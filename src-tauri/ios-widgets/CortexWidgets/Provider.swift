import WidgetKit
import SwiftUI
import CortexShared

/// One timeline entry carries the whole study snapshot the app last wrote to the App Group.
struct CortexEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot
    var theme: CortexTheme { CortexTheme(snapshot.theme) }
}

/// Shared provider for every static Cortex widget. The data is pushed by the app
/// (`WidgetCenter.reloadAllTimelines()` after each snapshot write), so the timeline is short:
/// render now, ask the system to refresh in 15 minutes as a safety net. Countdowns inside the
/// views use `Text(_:style:)` / `Text(timerInterval:)`, which tick on their own without reloads.
struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> CortexEntry {
        CortexEntry(date: Date(), snapshot: .placeholder)
    }

    func getSnapshot(in context: Context, completion: @escaping (CortexEntry) -> Void) {
        let snap = context.isPreview ? .placeholder : WidgetSnapshot.current
        completion(CortexEntry(date: Date(), snapshot: snap))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<CortexEntry>) -> Void) {
        let snap = WidgetSnapshot.current
        let entry = CortexEntry(date: Date(), snapshot: snap)
        let refresh = Date().addingTimeInterval(15 * 60)
        completion(Timeline(entries: [entry], policy: .after(refresh)))
    }
}
