import Foundation
import AVFoundation
#if canImport(ActivityKit)
import ActivityKit
#endif
#if canImport(WidgetKit)
import WidgetKit
#endif

/// Names the controller broadcasts so the Tauri plugin (when the app is foregrounded) can forward
/// ticks/state to the Svelte UI, keeping the existing recorder UI visually identical.
public extension Notification.Name {
    static let cortexRecordingTick = Notification.Name("cortex.recording.tick")
    static let cortexRecordingState = Notification.Name("cortex.recording.state")
}

/// Snapshot of recording state persisted to the App Group so the Live Activity + record widgets
/// always reflect reality even after a relaunch.
public struct RecordingState: Codable {
    public var isRecording: Bool
    public var isPaused: Bool
    public var startedAt: Date?
    public var subjectName: String?
    public var fileName: String?
    public init(isRecording: Bool = false, isPaused: Bool = false, startedAt: Date? = nil,
                subjectName: String? = nil, fileName: String? = nil) {
        self.isRecording = isRecording; self.isPaused = isPaused; self.startedAt = startedAt
        self.subjectName = subjectName; self.fileName = fileName
    }
    public static var current: RecordingState {
        AppGroup.read(RecordingState.self, from: AppGroup.recordingFile) ?? RecordingState()
    }
}

/// Single owner of the native lecture recorder. Lives in the APP process. Reachable from:
///  • the Tauri plugin (JS start/stop while the recorder screen is open), and
///  • the AppIntents fired by the Home/Lock-screen record button and the Live Activity Stop
///    button — `AudioRecordingIntent` runs these in the app's background process, so the same
///    singleton + audio session is used whether or not the UI is open. That is what lets
///    recording start from the Home/Lock screen WITHOUT bringing the app forward.
public final class RecordingController: NSObject, AVAudioRecorderDelegate {
    public static let shared = RecordingController()

    public private(set) var isRecording = false
    public private(set) var isPaused = false
    private var recorder: AVAudioRecorder?
    private var meterTimer: Timer?
    private var startedAt = Date()
    private var pausedAccumulated: TimeInterval = 0
    private var pauseStartedAt: Date?
    private var subjectName = "Lecture"
    private var accentHex = ThemeColors.osakaJade.accent
    private var currentFile: URL?
    private var lastActivityPush = Date(timeIntervalSince1970: 0)

    #if canImport(ActivityKit)
    @available(iOS 16.1, *)
    private var activity: Activity<RecordingAttributes>? {
        get { _activity as? Activity<RecordingAttributes> }
        set { _activity = newValue }
    }
    private var _activity: Any?
    #endif

    // MARK: - Public control

    /// Begin a recording. Safe to call from a background AppIntent. Returns the file URL or nil.
    @discardableResult
    public func start(subjectName: String? = nil, accentHex: String? = nil) -> URL? {
        if isRecording { return currentFile }
        if let s = subjectName, !s.isEmpty { self.subjectName = s }
        if let a = accentHex, !a.isEmpty { self.accentHex = a }

        let session = AVAudioSession.sharedInstance()
        do {
            // `.record` + `.spokenAudio` + the `audio` UIBackgroundMode is what keeps capture
            // alive when the screen locks or the app is backgrounded.
            try session.setCategory(.record, mode: .spokenAudio, options: [])
            try session.setActive(true, options: [])
        } catch {
            NSLog("[Cortex] audio session activate failed: \(error)")
            return nil
        }

        let stamp = Self.fileStamp()
        guard let dir = AppGroup.inbox ?? recordsFallbackDir() else { return nil }
        let url = dir.appendingPathComponent("lecture-\(stamp).m4a")
        let settings: [String: Any] = [
            AVFormatIDKey: Int(kAudioFormatMPEG4AAC),
            AVSampleRateKey: 16_000,            // matches the Whisper pipeline's preferred rate
            AVNumberOfChannelsKey: 1,
            AVEncoderAudioQualityKey: AVAudioQuality.medium.rawValue,
        ]
        do {
            let rec = try AVAudioRecorder(url: url, settings: settings)
            rec.delegate = self
            rec.isMeteringEnabled = true
            guard rec.record() else { return nil }
            recorder = rec
        } catch {
            NSLog("[Cortex] recorder init failed: \(error)")
            return nil
        }

        isRecording = true
        isPaused = false
        startedAt = Date()
        pausedAccumulated = 0
        pauseStartedAt = nil
        currentFile = url

        startMetering()
        startLiveActivity()
        persistState()
        reloadWidgets()
        postState()
        return url
    }

    /// Stop + finalize. Returns the finished file URL (in the App Group inbox) or nil.
    @discardableResult
    public func stop() -> URL? {
        guard isRecording else { return nil }
        let url = currentFile
        recorder?.stop()
        recorder = nil
        meterTimer?.invalidate(); meterTimer = nil
        isRecording = false
        isPaused = false
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        endLiveActivity()
        persistState()        // isRecording = false
        reloadWidgets()
        postState(finishedFile: url)
        return url
    }

    public func pause() {
        guard isRecording, !isPaused else { return }
        recorder?.pause()
        isPaused = true
        pauseStartedAt = Date()
        updateLiveActivity(force: true)
        persistState(); reloadWidgets(); postState()
    }

    public func resume() {
        guard isRecording, isPaused else { return }
        if let p = pauseStartedAt { pausedAccumulated += Date().timeIntervalSince(p) }
        pauseStartedAt = nil
        recorder?.record()
        isPaused = false
        updateLiveActivity(force: true)
        persistState(); reloadWidgets(); postState()
    }

    public func toggle() {
        if isRecording {
            if isPaused { resume() } else { pause() }
        } else {
            _ = start()
        }
    }

    /// Seconds of audio captured so far (excludes paused gaps).
    public var elapsed: TimeInterval {
        guard isRecording else { return 0 }
        let extra = isPaused ? (pauseStartedAt.map { Date().timeIntervalSince($0) } ?? 0) : 0
        return Date().timeIntervalSince(startedAt) - pausedAccumulated - extra
    }

    // MARK: - Metering

    private func startMetering() {
        meterTimer?.invalidate()
        // Schedule on the MAIN run loop explicitly: when start() is invoked from a background
        // AppIntent (Home/Lock-screen record button) there is no run loop on the calling thread,
        // so `Timer.scheduledTimer` would never fire. The app process stays alive via the `audio`
        // background mode, so the main run loop keeps running and ticking the meter.
        let timer = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in
            guard let self, let rec = self.recorder, self.isRecording, !self.isPaused else { return }
            rec.updateMeters()
            // dBFS (-160…0) → 0…1 with a gentle curve so quiet rooms still show a baseline.
            // averagePower returns Float; widen to Double so `level` matches the Double APIs.
            let db = Double(rec.averagePower(forChannel: 0))
            let norm = max(0, min(1, (db + 55) / 55))
            let level = pow(norm, 1.4)
            NotificationCenter.default.post(name: .cortexRecordingTick, object: nil,
                userInfo: ["level": level, "elapsed": self.elapsed])
            // The Live Activity clock uses Text(timerInterval:) and updates itself; only push
            // a level update every ~2s to stay inside ActivityKit's update budget.
            if Date().timeIntervalSince(self.lastActivityPush) > 2 {
                self.lastActivityPush = Date()
                self.updateLiveActivity(level: level)
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        meterTimer = timer
    }

    // MARK: - Live Activity

    private func startLiveActivity() {
        #if canImport(ActivityKit)
        guard #available(iOS 16.1, *) else { return }
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        let attrs = RecordingAttributes(subjectName: subjectName, accentHex: accentHex)
        let state = RecordingAttributes.State(startedAt: startedAt, isPaused: false, level: 0, pausedAccumulated: 0)
        do {
            if #available(iOS 16.2, *) {
                activity = try Activity.request(attributes: attrs,
                    content: .init(state: state, staleDate: nil), pushType: nil)
            } else {
                activity = try Activity.request(attributes: attrs, contentState: state, pushType: nil)
            }
        } catch {
            NSLog("[Cortex] Live Activity start failed: \(error)")
        }
        #endif
    }

    private func updateLiveActivity(level: Double = 0, force: Bool = false) {
        #if canImport(ActivityKit)
        guard #available(iOS 16.1, *), let activity else { return }
        let state = RecordingAttributes.State(startedAt: startedAt, isPaused: isPaused,
            level: level, pausedAccumulated: pausedAccumulated)
        Task {
            if #available(iOS 16.2, *) {
                await activity.update(.init(state: state, staleDate: nil))
            } else {
                await activity.update(using: state)
            }
        }
        #endif
    }

    private func endLiveActivity() {
        #if canImport(ActivityKit)
        guard #available(iOS 16.1, *), let activity else { return }
        let final = RecordingAttributes.State(startedAt: startedAt, isPaused: true,
            level: 0, pausedAccumulated: pausedAccumulated)
        Task {
            if #available(iOS 16.2, *) {
                await activity.end(.init(state: final, staleDate: nil), dismissalPolicy: .immediate)
            } else {
                await activity.end(using: final, dismissalPolicy: .immediate)
            }
        }
        self.activity = nil
        #endif
    }

    // MARK: - Persistence / widgets / events

    private func persistState() {
        let s = RecordingState(isRecording: isRecording, isPaused: isPaused,
            startedAt: isRecording ? startedAt : nil,
            subjectName: subjectName, fileName: currentFile?.lastPathComponent)
        AppGroup.write(s, to: AppGroup.recordingFile)
    }

    private func reloadWidgets() {
        #if canImport(WidgetKit)
        WidgetCenter.shared.reloadAllTimelines()
        #endif
    }

    private func postState(finishedFile: URL? = nil) {
        var info: [String: Any] = ["isRecording": isRecording, "isPaused": isPaused, "elapsed": elapsed]
        if let f = finishedFile { info["finishedFile"] = f.path }
        NotificationCenter.default.post(name: .cortexRecordingState, object: nil, userInfo: info)
    }

    private func recordsFallbackDir() -> URL? {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
        return docs
    }

    private static func fileStamp() -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyyMMdd-HHmmss"
        return f.string(from: Date())
    }

    // AVAudioRecorderDelegate
    public func audioRecorderEncodeErrorDidOccur(_ recorder: AVAudioRecorder, error: Error?) {
        NSLog("[Cortex] recorder encode error: \(String(describing: error))")
    }
}
