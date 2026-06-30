import WidgetKit
import SwiftUI

/// Entry point for the CortexWidgets extension: registers all eight widgets plus the recording
/// Live Activity. The Live Activity is gated to iOS 16.1 (ActivityKit's floor).
@main
struct CortexWidgetBundle: WidgetBundle {
    @WidgetBundleBuilder
    var body: some Widget {
        QuickRecordWidget()
        NextDeadlineWidget()
        TodayAgendaWidget()
        StudyDashboardWidget()
        FlashcardsDueWidget()
        PomodoroWidget()
        LockRecordWidget()
        LockNextUpWidget()
        liveActivity
    }

    @WidgetBundleBuilder
    private var liveActivity: some Widget {
        if #available(iOS 16.1, *) {
            RecordingLiveActivity()
        }
    }
}
