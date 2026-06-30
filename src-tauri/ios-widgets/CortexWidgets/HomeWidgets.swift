import WidgetKit
import SwiftUI
import AppIntents
import CortexShared

// ───────────────────────────────────────────────────────────────────────────
// 1) QUICK RECORD  — systemSmall. Tap the disc to start a lecture recording.
//    iOS 18: starts in the background (AudioRecordingIntent) without opening the app.
//    iOS 17: opens the app to arm the mic, then records in the background.
//    While recording, the widget flips to a live state with a Stop button.
// ───────────────────────────────────────────────────────────────────────────

struct QuickRecordWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexQuickRecord", provider: Provider()) { entry in
            QuickRecordView(entry: entry)
                .cortexContainer(entry.theme)
        }
        .configurationDisplayName("Quick Record")
        .description("Start recording a lecture with one tap.")
        .supportedFamilies([.systemSmall])
    }
}

struct QuickRecordView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var rec: RecordingState { RecordingState.current }

    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Eyebrow(text: rec.isRecording ? "Recording" : "Record", theme: theme)
                Spacer()
                BrandMark(theme: theme)
            }
            Spacer(minLength: 0)
            if rec.isRecording {
                if let started = rec.startedAt {
                    Text(started, style: .timer)
                        .font(theme.mono(20, weight: .semibold))
                        .foregroundStyle(theme.fgBright)
                        .monospacedDigit()
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                }
                RecordStopControl(theme: theme)
            } else {
                RecordStartControl(theme: theme, subject: entry.snapshot.activeSubject)
                Text(entry.snapshot.activeSubject ?? "Tap to record")
                    .font(theme.mono(9))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
    }
}

/// The start button — chooses the correct intent per iOS version, deep-links on iOS 16.
struct RecordStartControl: View {
    let theme: CortexTheme
    let subject: String?
    var body: some View {
        if #available(iOS 18.0, *) {
            Button(intent: StartRecordingIntent()) { RecordButton(theme: theme) }
                .buttonStyle(.plain)
        } else if #available(iOS 17.0, *) {
            Button(intent: StartRecordingLaunchIntent()) { RecordButton(theme: theme) }
                .buttonStyle(.plain)
        } else {
            RecordButton(theme: theme) // iOS 16: tapping the widget opens the app
        }
    }
}

struct RecordStopControl: View {
    let theme: CortexTheme
    var body: some View {
        if #available(iOS 17.0, *) {
            Button(intent: StopRecordingIntent()) {
                RecordButton(theme: theme, isRecording: true)
            }
            .buttonStyle(.plain)
        } else {
            RecordButton(theme: theme, isRecording: true)
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 2) NEXT DEADLINE — systemSmall. Countdown to the nearest assignment/exam.
// ───────────────────────────────────────────────────────────────────────────

struct NextDeadlineWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexNextDeadline", provider: Provider()) { entry in
            NextDeadlineView(entry: entry).cortexContainer(entry.theme)
        }
        .configurationDisplayName("Next Deadline")
        .description("Counts down to your nearest deadline.")
        .supportedFamilies([.systemSmall])
    }
}

struct NextDeadlineView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var next: DeadlineItem? { entry.snapshot.deadlines.sorted { $0.dueAt < $1.dueAt }.first }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Eyebrow(text: next?.kind == .exam ? "Next Exam" : "Next Due", theme: theme)
                Spacer()
                Image(systemName: next?.kind == .exam ? "graduationcap.fill" : "flag.checkered")
                    .font(.system(size: 11)).foregroundStyle(theme.accent)
            }
            Spacer(minLength: 0)
            if let d = next {
                Text(d.dueAt, style: .relative)
                    .font(theme.mono(22, weight: .bold))
                    .foregroundStyle(theme.urgency(hoursAway: d.hoursAway))
                    .lineLimit(1).minimumScaleFactor(0.5)
                Text(d.title)
                    .font(theme.rounded(13, weight: .semibold))
                    .foregroundStyle(theme.fgBright).lineLimit(2)
                Text(d.course)
                    .font(theme.mono(9)).foregroundStyle(theme.fgMuted).lineLimit(1)
            } else {
                Spacer()
                Text("All caught up").font(theme.rounded(15, weight: .semibold)).foregroundStyle(theme.fgBright)
                Text("No upcoming deadlines").font(theme.mono(9)).foregroundStyle(theme.fgMuted)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 3) TODAY'S AGENDA — systemMedium. Today's classes / calendar events.
// ───────────────────────────────────────────────────────────────────────────

struct TodayAgendaWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexTodayAgenda", provider: Provider()) { entry in
            TodayAgendaView(entry: entry).cortexContainer(entry.theme)
        }
        .configurationDisplayName("Today's Agenda")
        .description("Your classes and events for today.")
        .supportedFamilies([.systemMedium])
    }
}

struct TodayAgendaView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var items: [AgendaItem] { Array(entry.snapshot.agenda.sorted { $0.at < $1.at }.prefix(4)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Eyebrow(text: "Today", theme: theme)
                Spacer()
                Text(Date(), style: .date).font(theme.mono(9)).foregroundStyle(theme.fgMuted)
                BrandMark(theme: theme, size: 12)
            }
            if items.isEmpty {
                Spacer()
                HStack { Spacer()
                    VStack(spacing: 4) {
                        Image(systemName: "checkmark.circle").font(.system(size: 20)).foregroundStyle(theme.accent)
                        Text("Nothing scheduled").font(theme.rounded(13, weight: .semibold)).foregroundStyle(theme.fgBright)
                    }
                    Spacer() }
                Spacer()
            } else {
                VStack(spacing: 7) {
                    ForEach(items) { it in
                        HStack(spacing: 9) {
                            Text(it.at, style: .time)
                                .font(theme.mono(11, weight: .semibold))
                                .foregroundStyle(theme.accent).frame(width: 58, alignment: .leading)
                            Rectangle().fill(theme.accent.opacity(0.5)).frame(width: 2, height: 22)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(it.title).font(theme.rounded(12, weight: .semibold))
                                    .foregroundStyle(theme.fgBright).lineLimit(1)
                                Text(it.course).font(theme.mono(8)).foregroundStyle(theme.fgMuted).lineLimit(1)
                            }
                            Spacer(minLength: 0)
                        }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(14)
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 4) STUDY DASHBOARD — systemLarge. Streak + deadlines + subject progress board.
// ───────────────────────────────────────────────────────────────────────────

struct StudyDashboardWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexStudyDashboard", provider: Provider()) { entry in
            StudyDashboardView(entry: entry).cortexContainer(entry.theme)
        }
        .configurationDisplayName("Study Dashboard")
        .description("Streak, deadlines and subject progress at a glance.")
        .supportedFamilies([.systemLarge])
    }
}

struct StudyDashboardView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var snap: WidgetSnapshot { entry.snapshot }

    var body: some View {
        VStack(alignment: .leading, spacing: 11) {
            HStack(alignment: .center, spacing: 12) {
                BrandMark(theme: theme, size: 16)
                Text("Cortex").font(theme.mono(15, weight: .bold)).foregroundStyle(theme.fgBright)
                Spacer()
                HStack(spacing: 5) {
                    Image(systemName: "flame.fill").font(.system(size: 13)).foregroundStyle(theme.warn)
                    Text("\(snap.streak)").font(theme.mono(15, weight: .bold)).foregroundStyle(theme.fgBright)
                    Text("day streak").font(theme.mono(9)).foregroundStyle(theme.fgMuted)
                }
            }
            Divider().overlay(theme.fgMuted.opacity(0.3))

            // stat strip
            HStack(spacing: 10) {
                StatChip(value: "\(snap.deadlines.count)", label: "due soon", color: theme.accent, theme: theme)
                StatChip(value: "\(snap.flashcardsDue)", label: "cards due", color: theme.warn, theme: theme)
                StatChip(value: "\(snap.agenda.count)", label: "today", color: theme.fg, theme: theme)
            }

            Eyebrow(text: "Upcoming", theme: theme)
            VStack(spacing: 6) {
                ForEach(Array(snap.deadlines.sorted { $0.dueAt < $1.dueAt }.prefix(3))) { d in
                    DeadlineRow(item: d, theme: theme, compact: true)
                }
                if snap.deadlines.isEmpty {
                    Text("No deadlines — nice.").font(theme.mono(10)).foregroundStyle(theme.fgMuted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            Eyebrow(text: "Subjects", theme: theme)
            VStack(spacing: 7) {
                ForEach(Array(snap.subjects.prefix(3))) { s in
                    HStack(spacing: 8) {
                        Text(s.name).font(theme.rounded(11, weight: .medium))
                            .foregroundStyle(theme.fg).lineLimit(1).frame(width: 92, alignment: .leading)
                        ProgressBar(progress: s.progress, color: Color(hex: s.accentHex), track: theme.surface)
                        Text("\(Int(s.progress * 100))%").font(theme.mono(9)).foregroundStyle(theme.fgMuted)
                            .frame(width: 30, alignment: .trailing)
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
    }
}

struct StatChip: View {
    let value: String; let label: String; let color: Color; let theme: CortexTheme
    var body: some View {
        VStack(spacing: 2) {
            Text(value).font(theme.mono(18, weight: .bold)).foregroundStyle(color)
            Text(label).font(theme.mono(8)).foregroundStyle(theme.fgMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 10).fill(theme.surface))
    }
}

struct ProgressBar: View {
    let progress: Double; let color: Color; let track: Color
    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(track)
                Capsule().fill(color).frame(width: max(4, geo.size.width * max(0, min(1, progress))))
            }
        }
        .frame(height: 6)
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 5) FLASHCARDS DUE — systemSmall. Spaced-repetition cards waiting for review.
// ───────────────────────────────────────────────────────────────────────────

struct FlashcardsDueWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexFlashcardsDue", provider: Provider()) { entry in
            FlashcardsDueView(entry: entry).cortexContainer(entry.theme)
        }
        .configurationDisplayName("Cards Due")
        .description("Flashcards ready for review today.")
        .supportedFamilies([.systemSmall])
    }
}

struct FlashcardsDueView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var due: Int { entry.snapshot.flashcardsDue }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Eyebrow(text: "Review", theme: theme)
                Spacer()
                Image(systemName: "rectangle.on.rectangle.angled")
                    .font(.system(size: 12)).foregroundStyle(theme.accent)
            }
            Spacer(minLength: 0)
            Text("\(due)")
                .font(theme.mono(40, weight: .bold))
                .foregroundStyle(due > 0 ? theme.fgBright : theme.fgMuted)
                .lineLimit(1).minimumScaleFactor(0.5)
            Text(due == 1 ? "card due" : "cards due")
                .font(theme.rounded(13, weight: .semibold)).foregroundStyle(theme.fg)
            Text(due > 0 ? "Tap to start reviewing" : "You're all caught up")
                .font(theme.mono(8)).foregroundStyle(theme.fgMuted).lineLimit(1)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .widgetURL(URL(string: "cortex://review"))
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 6) POMODORO / FOCUS — systemSmall. Mirrors the in-app focus timer.
// ───────────────────────────────────────────────────────────────────────────

struct PomodoroWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CortexPomodoro", provider: Provider()) { entry in
            PomodoroView(entry: entry).cortexContainer(entry.theme)
        }
        .configurationDisplayName("Focus Timer")
        .description("Your Pomodoro focus session.")
        .supportedFamilies([.systemSmall])
    }
}

struct PomodoroView: View {
    let entry: CortexEntry
    var theme: CortexTheme { entry.theme }
    var pomo: PomodoroState? { entry.snapshot.pomodoro }

    var ringColor: Color {
        guard let p = pomo else { return theme.fgMuted }
        return p.phase == "work" ? theme.accent : Color(hex: "#63b07a")
    }

    var body: some View {
        ZStack {
            RingView(progress: pomo?.progress ?? 0, color: ringColor, track: theme.surface, lineWidth: 7)
                .padding(6)
            VStack(spacing: 2) {
                Image(systemName: pomo?.running == true ? "leaf.fill" : "leaf")
                    .font(.system(size: 14)).foregroundStyle(ringColor)
                Text(pomo?.mmss ?? "25:00")
                    .font(theme.mono(19, weight: .bold)).foregroundStyle(theme.fgBright)
                    .monospacedDigit().lineLimit(1).minimumScaleFactor(0.6)
                Text(pomo?.label ?? "Ready")
                    .font(theme.mono(7)).foregroundStyle(theme.fgMuted).lineLimit(1)
            }
        }
        .padding(12)
        .widgetURL(URL(string: "cortex://focus"))
    }
}
