import Foundation
import WidgetKit
import SwiftUI
import CortexShared
#if canImport(ActivityKit)
import ActivityKit
import AppIntents

// The lecture-recording Live Activity: a persistent Lock Screen banner + Dynamic Island that
// shows the running timer, a level-driven waveform, and (iOS 17+) Stop / Pause buttons that act
// in the background. The `RecordingAttributes` type is the one shared in CortexShared, so this
// renders the activity the app's RecordingController started.
@available(iOS 16.1, *)
struct RecordingLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RecordingAttributes.self) { context in
            LockScreenLiveView(context: context)
                .activityBackgroundTint(Color(hex: "#0e1813").opacity(0.92))
                .activitySystemActionForegroundColor(Color(hex: context.attributes.accentHex))
        } dynamicIsland: { context in
            let accent = Color(hex: context.attributes.accentHex)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    HStack(spacing: 6) {
                        PulseDot(color: context.state.isPaused ? .gray : Color(hex: "#ff5345"))
                        VStack(alignment: .leading, spacing: 0) {
                            Text("REC").font(.system(size: 9, weight: .bold, design: .monospaced)).foregroundStyle(accent)
                            Text(context.attributes.subjectName)
                                .font(.system(size: 11, weight: .semibold)).foregroundStyle(.white).lineLimit(1)
                        }
                    }
                }
                DynamicIslandExpandedRegion(.trailing) {
                    TimerText(context: context, size: 16).foregroundStyle(.white)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    HStack(spacing: 10) {
                        WaveBars(level: context.state.level, color: accent, bars: 13, height: 16)
                        Spacer()
                        if #available(iOS 17.0, *) {
                            Button(intent: ToggleRecordingPauseIntent()) {
                                Image(systemName: context.state.isPaused ? "play.fill" : "pause.fill")
                                    .font(.system(size: 13, weight: .bold))
                            }
                            .buttonStyle(.plain).tint(accent)
                            Button(intent: StopRecordingIntent()) {
                                Image(systemName: "stop.fill").font(.system(size: 13, weight: .bold))
                            }
                            .buttonStyle(.plain).tint(Color(hex: "#ff5345"))
                        }
                    }
                    .padding(.top, 2)
                }
            } compactLeading: {
                PulseDot(color: context.state.isPaused ? .gray : Color(hex: "#ff5345"))
            } compactTrailing: {
                TimerText(context: context, size: 13).foregroundStyle(accent)
            } minimal: {
                PulseDot(color: context.state.isPaused ? .gray : Color(hex: "#ff5345"))
            }
            .widgetURL(URL(string: "cortex://recording"))
            .keylineTint(accent)
        }
    }
}

@available(iOS 16.1, *)
private struct LockScreenLiveView: View {
    let context: ActivityViewContext<RecordingAttributes>
    var accent: Color { Color(hex: context.attributes.accentHex) }

    var body: some View {
        HStack(spacing: 14) {
            VStack(spacing: 4) {
                PulseDot(color: context.state.isPaused ? .gray : Color(hex: "#ff5345"), size: 11)
                Text(context.state.isPaused ? "PAUSED" : "REC")
                    .font(.system(size: 8, weight: .bold, design: .monospaced)).foregroundStyle(accent)
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(context.attributes.subjectName)
                    .font(.system(size: 14, weight: .semibold)).foregroundStyle(.white).lineLimit(1)
                WaveBars(level: context.state.level, color: accent, bars: 22, height: 16)
            }
            Spacer(minLength: 6)
            VStack(alignment: .trailing, spacing: 4) {
                TimerText(context: context, size: 22).foregroundStyle(.white)
                if #available(iOS 17.0, *) {
                    HStack(spacing: 8) {
                        Button(intent: ToggleRecordingPauseIntent()) {
                            Image(systemName: context.state.isPaused ? "play.fill" : "pause.fill")
                                .font(.system(size: 12, weight: .bold)).foregroundStyle(accent)
                                .frame(width: 30, height: 26)
                                .background(RoundedRectangle(cornerRadius: 7).fill(.white.opacity(0.12)))
                        }.buttonStyle(.plain)
                        Button(intent: StopRecordingIntent()) {
                            Image(systemName: "stop.fill")
                                .font(.system(size: 12, weight: .bold)).foregroundStyle(Color(hex: "#ff5345"))
                                .frame(width: 30, height: 26)
                                .background(RoundedRectangle(cornerRadius: 7).fill(.white.opacity(0.12)))
                        }.buttonStyle(.plain)
                    }
                }
            }
        }
        .padding(16)
    }
}

// MARK: - Shared bits

@available(iOS 16.1, *)
private struct TimerText: View {
    let context: ActivityViewContext<RecordingAttributes>
    var size: CGFloat
    var body: some View {
        if context.state.isPaused {
            Text("paused").font(.system(size: size * 0.7, weight: .medium, design: .monospaced))
        } else {
            // Count up from when recording effectively started (paused gaps folded in).
            Text(timerInterval: context.state.countUpFrom...Date(timeIntervalSinceNow: 60 * 60 * 12),
                 countsDown: false)
                .font(.system(size: size, weight: .semibold, design: .monospaced))
                .monospacedDigit()
        }
    }
}

private struct PulseDot: View {
    let color: Color
    var size: CGFloat = 9
    var body: some View {
        Circle().fill(color).frame(width: size, height: size)
            .shadow(color: color.opacity(0.7), radius: 4)
    }
}

/// A row of bars whose heights are seeded by the mic level (single value per update) plus a fixed
/// per-bar variation, so it reads as a waveform without needing a timer the Live Activity can't run.
private struct WaveBars: View {
    let level: Double
    let color: Color
    var bars: Int = 16
    var height: CGFloat = 16
    var body: some View {
        HStack(alignment: .center, spacing: 2.5) {
            ForEach(0..<bars, id: \.self) { i in
                let seed = (sin(Double(i) * 1.7) + 1) / 2          // 0…1 fixed shape
                let amp = max(0.12, min(1, level * (0.55 + seed * 0.9)))
                Capsule().fill(color.opacity(0.55 + amp * 0.45))
                    .frame(width: 2.5, height: max(2, height * amp))
            }
        }
        .frame(height: height)
    }
}
#endif
