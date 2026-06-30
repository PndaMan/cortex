import Foundation
import Tauri
import UIKit
import WidgetKit
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
        var data: [String: Any] = [
            "isRecording": (n.userInfo?["isRecording"] as? Bool) ?? false,
            "isPaused": (n.userInfo?["isPaused"] as? Bool) ?? false,
            "elapsed": (n.userInfo?["elapsed"] as? Double) ?? 0,
        ]
        if let f = n.userInfo?["finishedFile"] as? String { data["finishedFile"] = f }
        trigger("state", data: data)
    }
}

@_cdecl("init_plugin_cortex_ios")
func initPlugin() -> Plugin {
    return CortexIosPlugin()
}
