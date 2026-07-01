import AppIntents
import Foundation

// AppIntents that drive the native recorder from widgets + the Live Activity.
//
// The "record from the Home/Lock screen without opening the app" behaviour (à la Coconote) is
// `AudioRecordingIntent` (iOS 18+): the system runs its `perform()` in the APP's background
// process and grants the audio-session privileges needed to start capturing. On iOS 16.1–17 we
// fall back to an intent that opens the app to arm the mic, then records in the background.

// MARK: - Start (iOS 18 headless)

@available(iOS 18.0, *)
public struct StartRecordingIntent: AudioRecordingIntent {
    public static var title: LocalizedStringResource = "Record Lecture"
    public static var description = IntentDescription("Start recording a lecture in the background.")
    public init() {}
    public func perform() async throws -> some IntentResult {
        // Headless start requires mic permission already granted (a background intent can't prompt).
        // The widget routes to StartRecordingLaunchIntent when permission is missing.
        if RecordingController.shared.hasMicPermission {
            _ = RecordingController.shared.start()
        }
        return .result()
    }
}

// MARK: - Start via the app (opens Cortex — used to request mic permission, or on iOS 16–17)

@available(iOS 16.0, *)
public struct StartRecordingLaunchIntent: AppIntent {
    public static var title: LocalizedStringResource = "Record Lecture"
    public static var description = IntentDescription("Open Cortex and start recording a lecture.")
    // Open the app so we can prompt for mic permission (impossible from a background intent) and
    // activate the audio session pre-iOS 18.
    public static var openAppWhenRun: Bool = true
    public init() {}
    public func perform() async throws -> some IntentResult {
        let granted = await RecordingController.shared.ensureMicPermission()
        if granted { _ = RecordingController.shared.start() }
        return .result()
    }
}

// MARK: - Stop (works in the background from the Live Activity)

@available(iOS 16.0, *)
public struct StopRecordingIntent: AppIntent {
    public static var title: LocalizedStringResource = "Stop Recording"
    public static var description = IntentDescription("Stop recording and open Cortex to save it.")
    // Open the app so the user chooses the subject/topic to save into (no auto-assign).
    public static var openAppWhenRun: Bool = true
    public init() {}
    public func perform() async throws -> some IntentResult {
        _ = RecordingController.shared.stop()
        return .result()
    }
}

// MARK: - Pause / Resume toggle (background, from the Live Activity)

@available(iOS 16.2, *)
public struct ToggleRecordingPauseIntent: LiveActivityIntent {
    public static var title: LocalizedStringResource = "Pause or Resume Recording"
    public init() {}
    public func perform() async throws -> some IntentResult {
        let c = RecordingController.shared
        if c.isPaused { c.resume() } else { c.pause() }
        return .result()
    }
}
