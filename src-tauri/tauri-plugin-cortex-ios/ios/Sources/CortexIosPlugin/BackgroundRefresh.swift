import Foundation
import BackgroundTasks

// Bridge to the Rust background-check entry point (src-tauri/src/notify.rs). The symbol is linked
// into the app binary from the Rust staticlib.
@_silgen_name("cortex_ios_background_refresh")
func cortex_ios_background_refresh()

/// Registers + schedules the iOS Background-App-Refresh task that polls Moodle for new grades and
/// announcements while the app is closed, and fires local notifications for anything new (the Rust
/// side does the sync + diff + notify). Identifier must match `BGTaskSchedulerPermittedIdentifiers`
/// in Info.plist (added by CI).
enum BackgroundRefresh {
    static let taskId = "study.cortex.app.refresh"
    private static var registered = false

    /// Register the task handler. Must happen early in launch (this is called from the plugin's
    /// `load`, which Tauri invokes during app launch). Guarded so it only registers once.
    static func registerIfNeeded() {
        guard !registered else { return }
        registered = true
        BGTaskScheduler.shared.register(forTaskWithIdentifier: taskId, using: nil) { task in
            guard let refresh = task as? BGAppRefreshTask else { task.setTaskCompleted(success: false); return }
            handle(refresh)
        }
        schedule()
    }

    /// Ask iOS to wake us again (no sooner than ~30 min; iOS decides the real cadence).
    static func schedule() {
        let req = BGAppRefreshTaskRequest(identifier: taskId)
        req.earliestBeginDate = Date(timeIntervalSinceNow: 30 * 60)
        try? BGTaskScheduler.shared.submit(req)
    }

    private static func handle(_ task: BGAppRefreshTask) {
        schedule() // chain the next refresh before doing work
        let work = DispatchWorkItem { cortex_ios_background_refresh() }
        task.expirationHandler = { work.cancel() }
        DispatchQueue.global(qos: .background).async {
            work.perform()
            task.setTaskCompleted(success: true)
        }
    }
}
