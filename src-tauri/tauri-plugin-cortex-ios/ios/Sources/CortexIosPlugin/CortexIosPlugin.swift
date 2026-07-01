import Foundation
import Tauri
import UIKit
import WebKit
import WidgetKit
import AVFoundation
import CortexShared

/// Tauri iOS plugin: the JS↔Swift bridge for the native lecture recorder and widget snapshots.
/// JS calls `invoke('plugin:cortex-ios|start_recording', …)`; this forwards to the shared
/// `RecordingController` and streams `tick` / `state` events back so the existing Svelte recorder
/// UI (waveform + timer) keeps working unchanged while capture runs natively in the background.
class CortexIosPlugin: Plugin {

    override init() {
        super.init()
        NotificationCenter.default.addObserver(self, selector: #selector(onTick(_:)),
                                               name: .cortexRecordingTick, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(onState(_:)),
                                               name: .cortexRecordingState, object: nil)
    }

    /// Called as the webview/plugin loads (early in app launch) — register the Background-App-Refresh
    /// task here so iOS can wake us to check Moodle for new grades/announcements while closed.
    @objc public override func load(webview: WKWebView) {
        BackgroundRefresh.registerIfNeeded()
    }

    deinit { NotificationCenter.default.removeObserver(self) }

    // MARK: - Recorder commands

    struct StartArgs: Decodable { var subject: String?; var accent: String? }
    struct PathArgs: Decodable { var path: String }
    struct SnapshotArgs: Decodable { var json: String }

    @objc public func startRecording(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(StartArgs.self)
        let url = RecordingController.shared.start(subjectName: args.subject, accentHex: args.accent)
        invoke.resolve(["path": url?.path ?? "", "durationSec": 0])
    }

    @objc public func stopRecording(_ invoke: Invoke) throws {
        let dur = RecordingController.shared.elapsed
        let url = RecordingController.shared.stop()
        invoke.resolve(["path": url?.path ?? "", "durationSec": dur])
    }

    @objc public func pauseRecording(_ invoke: Invoke) throws {
        RecordingController.shared.pause()
        invoke.resolve(["ok": true])
    }

    @objc public func resumeRecording(_ invoke: Invoke) throws {
        RecordingController.shared.resume()
        invoke.resolve(["ok": true])
    }

    // MARK: - Microphone permission

    /// Current mic permission without prompting: "granted" | "denied" | "undetermined".
    /// Persists the flag so the record widgets pick the right (headless vs open-app) intent.
    @objc public func micPermissionStatus(_ invoke: Invoke) throws {
        let s = Self.micStatusString()
        AppGroup.setMicGranted(s == "granted")
        WidgetCenter.shared.reloadAllTimelines()
        invoke.resolve(["status": s])
    }

    /// Request mic permission. Shows the system prompt when undetermined; resolves with the result.
    /// (When already denied the system won't re-prompt — the caller opens Settings instead.)
    @objc public func requestMicPermission(_ invoke: Invoke) throws {
        let done: (Bool) -> Void = { granted in
            AppGroup.setMicGranted(granted)
            WidgetCenter.shared.reloadAllTimelines()
            invoke.resolve(["granted": granted, "status": granted ? "granted" : Self.micStatusString()])
        }
        if #available(iOS 17.0, *) {
            AVAudioApplication.requestRecordPermission(completionHandler: done)
        } else {
            AVAudioSession.sharedInstance().requestRecordPermission(done)
        }
    }

    /// Open this app's page in the Settings app (so the user can flip Microphone on after denying).
    @objc public func openAppSettings(_ invoke: Invoke) throws {
        DispatchQueue.main.async {
            if let url = URL(string: UIApplication.openSettingsURLString) {
                UIApplication.shared.open(url, options: [:], completionHandler: nil)
            }
        }
        invoke.resolve(["ok": true])
    }

    private static func micStatusString() -> String {
        if #available(iOS 17.0, *) {
            switch AVAudioApplication.shared.recordPermission {
            case .granted: return "granted"
            case .denied: return "denied"
            default: return "undetermined"
            }
        } else {
            switch AVAudioSession.sharedInstance().recordPermission {
            case .granted: return "granted"
            case .denied: return "denied"
            default: return "undetermined"
            }
        }
    }

    @objc public func recordingState(_ invoke: Invoke) throws {
        let c = RecordingController.shared
        invoke.resolve([
            "isRecording": c.isRecording,
            "isPaused": c.isPaused,
            "elapsed": c.elapsed,
            "fileName": RecordingState.current.fileName ?? "",
        ])
    }

    /// Read a finished recording's bytes so the existing JS `saveRecording` pipeline can ingest
    /// it unchanged (keeps the recorder save/review UI identical to desktop).
    @objc public func readRecordingBytes(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(PathArgs.self)
        let data = (try? Data(contentsOf: URL(fileURLWithPath: args.path))) ?? Data()
        invoke.resolve(["bytes": Array(data)])
    }

    /// List finished recordings sitting in the App Group inbox (e.g. a lecture recorded entirely
    /// from the Lock Screen while the app was closed). The app drains these on launch.
    @objc public func listInbox(_ invoke: Invoke) throws {
        var files: [[String: Any]] = []
        if let dir = AppGroup.inbox,
           let urls = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.fileSizeKey]) {
            let subject = RecordingState.current.subjectName
            for u in urls where u.pathExtension.lowercased() == "m4a" {
                let size = (try? u.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
                files.append(["path": u.path, "name": u.lastPathComponent, "subject": subject ?? "", "size": size])
            }
        }
        invoke.resolve(["files": files])
    }

    @objc public func deleteRecording(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(PathArgs.self)
        let ok = (try? FileManager.default.removeItem(atPath: args.path)) != nil
        invoke.resolve(["ok": ok])
    }

    // MARK: - Widgets

    @objc public func setWidgetSnapshot(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(SnapshotArgs.self)
        let ok = AppGroup.writeRaw(args.json, to: AppGroup.snapshotFile)
        WidgetCenter.shared.reloadAllTimelines()
        invoke.resolve(["ok": ok])
    }

    // MARK: - Controller → JS events

    @objc private func onTick(_ n: Notification) {
        trigger("tick", data: [
            "level": (n.userInfo?["level"] as? Double) ?? 0,
            "elapsed": (n.userInfo?["elapsed"] as? Double) ?? 0,
        ])
    }

    @objc private func onState(_ n: Notification) {
        // Pass a dictionary LITERAL to trigger (like onTick) so it coerces to Tauri's JSObject —
        // a pre-typed [String: Any] variable doesn't match. finishedFile is "" when absent.
        trigger("state", data: [
            "isRecording": (n.userInfo?["isRecording"] as? Bool) ?? false,
            "isPaused": (n.userInfo?["isPaused"] as? Bool) ?? false,
            "elapsed": (n.userInfo?["elapsed"] as? Double) ?? 0,
            "finishedFile": (n.userInfo?["finishedFile"] as? String) ?? "",
        ])
    }
}

@_cdecl("init_plugin_cortex_ios")
func initPlugin() -> Plugin {
    return CortexIosPlugin()
}
