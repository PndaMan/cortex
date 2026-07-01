import Foundation

/// Shared App Group plumbing. The non-sandboxed Tauri app writes JSON snapshots into the
/// App Group container; the sandboxed widget extension reads them. This is the only data
/// bridge between the two processes (WidgetKit forbids a live IPC channel).
///
/// The group id is derived from the bundle id so it stays correct if the identifier ever
/// changes. It MUST match the `group.study.cortex.app` group registered in the Apple
/// Developer portal and added to BOTH targets' entitlements (the CI inject script writes it).
public enum AppGroup {
    /// The App Group identifier. Keep in sync with the portal + entitlements + CI script.
    public static let id = "group.study.cortex.app"

    /// File the app writes with the latest study snapshot (deadlines, agenda, streak, …).
    public static let snapshotFile = "widget-snapshot.json"
    /// File describing the live recording state (used by the Live Activity / record widgets).
    public static let recordingFile = "recording-state.json"
    /// Folder where finished recordings are dropped for the app to ingest on next launch.
    public static let inboxDir = "RecordingInbox"
    /// "1"/"0" mic-permission flag so the record widgets can pick a headless vs open-app intent.
    public static let micFile = "mic-granted"

    /// Whether the app currently has microphone permission (persisted by the app/plugin).
    public static var micGranted: Bool {
        guard let url = container?.appendingPathComponent(micFile),
              let s = try? String(contentsOf: url, encoding: .utf8) else { return false }
        return s.trimmingCharacters(in: .whitespacesAndNewlines) == "1"
    }

    public static func setMicGranted(_ granted: Bool) {
        writeRaw(granted ? "1" : "0", to: micFile)
    }

    /// Root of the shared container, or nil if the entitlement is missing (e.g. unsigned sim).
    public static var container: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: id)
    }

    /// Inbox for finished/in-progress recordings — the APP'S OWN Documents sandbox, NOT the App
    /// Group container. Documents is always writable whether the app is App-Store-signed or
    /// sideloaded (LiveContainer), and the record AppIntent runs in the app's process so it shares
    /// this sandbox. Recording into the shared App-Group container is unreliable when signed — that
    /// was why capture failed on TestFlight but worked on the unsigned sideload. The App Group is
    /// still used for the widget snapshot / recording-state / mic flag (small JSON), not audio.
    public static var inbox: URL? {
        guard let base = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
        else { return nil }
        let dir = base.appendingPathComponent(inboxDir, isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    // MARK: - Generic JSON helpers (atomic write, tolerant read)

    public static func read<T: Decodable>(_ type: T.Type, from file: String) -> T? {
        guard let url = container?.appendingPathComponent(file),
              let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder.cortex.decode(T.self, from: data)
    }

    @discardableResult
    public static func write<T: Encodable>(_ value: T, to file: String) -> Bool {
        guard let url = container?.appendingPathComponent(file),
              let data = try? JSONEncoder.cortex.encode(value) else { return false }
        do {
            try data.write(to: url, options: .atomic)
            return true
        } catch {
            return false
        }
    }

    /// Write a raw JSON string straight through (used by the Tauri plugin which already
    /// has the snapshot serialized on the JS side — no re-encode, no schema drift).
    @discardableResult
    public static func writeRaw(_ json: String, to file: String) -> Bool {
        guard let url = container?.appendingPathComponent(file),
              let data = json.data(using: .utf8) else { return false }
        return (try? data.write(to: url, options: .atomic)) != nil
    }
}

extension JSONDecoder {
    static var cortex: JSONDecoder {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .secondsSince1970
        return d
    }
}
extension JSONEncoder {
    static var cortex: JSONEncoder {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .secondsSince1970
        return e
    }
}
