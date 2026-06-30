import WidgetKit
import SwiftUI
import AppIntents
import CortexShared

// ───────────────────────────────────────────────────────────────────────────
// LOCK SCREEN: RECORD  — accessoryCircular. Tap to start/stop a lecture recording
// straight from the Lock Screen (also usable on the Home Screen via StandBy).
// ───────────────────────────────────────────────────────────────────────────

struct LockRecordWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexLockRecord", provider: Provider()) { entry in
            LockRecordView(entry: entry)
        }
        .configurationDisplayName("Record (Lock Screen)")
        .description("Start a lecture recording from the Lock Screen.")
        .supportedFamilies([.accessoryCircular])
    }
}

struct LockRecordView: View {
    let entry: CortexEntry
    var rec: RecordingState { RecordingState.current }

    var body: some View {
        ZStack {
            AccessoryWidgetBackground()
            if rec.isRecording {
                // Live: pulsing square + tiny elapsed timer; tap stops.
                lockButton(stop: true) {
                    VStack(spacing: 1) {
                        Image(systemName: "stop.fill").font(.system(size: 15, weight: .bold))
                        if let s = rec.startedAt {
                            Text(s, style: .timer).font(.system(size: 8, weight: .medium, design: .monospaced))
                                .monospacedDigit().lineLimit(1).minimumScaleFactor(0.5)
                        }
                    }
                }
            } else {
                lockButton(stop: false) {
                    Image(systemName: "mic.fill").font(.system(size: 18, weight: .semibold))
                }
            }
        }
        .widgetAccentable()
    }

    @ViewBuilder
    private func lockButton<Content: View>(stop: Bool, @ViewBuilder _ label: () -> Content) -> some View {
        if #available(iOS 18.0, *), !stop {
            Button(intent: StartRecordingIntent()) { label() }.buttonStyle(.plain)
        } else if #available(iOS 17.0, *) {
            if stop {
                Button(intent: StopRecordingIntent()) { label() }.buttonStyle(.plain)
            } else {
                Button(intent: StartRecordingLaunchIntent()) { label() }.buttonStyle(.plain)
            }
        } else {
            label() // iOS 16: tap opens the app
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// LOCK SCREEN: NEXT-UP  — accessoryRectangular + accessoryInline. The next class
// or deadline as a glanceable one/two-liner.
// ───────────────────────────────────────────────────────────────────────────

struct LockNextUpWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexLockNextUp", provider: Provider()) { entry in
            LockNextUpView(entry: entry)
        }
        .configurationDisplayName("Next Up (Lock Screen)")
        .description("Your next class or deadline.")
        .supportedFamilies([.accessoryRectangular, .accessoryInline])
    }
}

struct LockNextUpView: View {
    @Environment(\.widgetFamily) private var family
    let entry: CortexEntry

    /// Prefer the soonest agenda event today; otherwise the nearest deadline.
    private var nextAgenda: AgendaItem? { entry.snapshot.agenda.sorted { $0.at < $1.at }.first }
    private var nextDeadline: DeadlineItem? { entry.snapshot.deadlines.sorted { $0.dueAt < $1.dueAt }.first }

    var body: some View {
        switch family {
        case .accessoryInline:
            inline
        default:
            rectangular
        }
    }

    private var inline: some View {
        Group {
            if let a = nextAgenda {
                Label { Text("\(a.title) · \(a.at, style: .time)") } icon: { Image(systemName: "calendar") }
            } else if let d = nextDeadline {
                Label { Text("\(d.title) · ") + Text(d.dueAt, style: .relative) } icon: { Image(systemName: "flag.checkered") }
            } else {
                Label("Cortex · all clear", systemImage: "checkmark.circle")
            }
        }
    }

    private var rectangular: some View {
        VStack(alignment: .leading, spacing: 2) {
            if let a = nextAgenda {
                HStack(spacing: 4) {
                    Image(systemName: "calendar").font(.system(size: 10))
                    Text("NEXT CLASS").font(.system(size: 9, weight: .semibold, design: .monospaced)).tracking(1)
                }
                .widgetAccentable()
                Text(a.title).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                Text("\(a.at, style: .time) · \(a.course)").font(.system(size: 11)).lineLimit(1)
            } else if let d = nextDeadline {
                HStack(spacing: 4) {
                    Image(systemName: d.kind == .exam ? "graduationcap.fill" : "flag.checkered").font(.system(size: 10))
                    Text(d.kind == .exam ? "NEXT EXAM" : "NEXT DUE")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced)).tracking(1)
                }
                .widgetAccentable()
                Text(d.title).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                HStack(spacing: 3) {
                    Text(d.dueAt, style: .relative).font(.system(size: 11, weight: .medium))
                    Text("· \(d.course)").font(.system(size: 11)).lineLimit(1)
                }
            } else {
                HStack(spacing: 4) {
                    Image(systemName: "checkmark.circle").font(.system(size: 11)).widgetAccentable()
                    Text("All caught up").font(.system(size: 14, weight: .semibold))
                }
                Text("No upcoming classes or deadlines").font(.system(size: 11)).lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
