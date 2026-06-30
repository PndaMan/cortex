import SwiftUI
import WidgetKit
import CortexShared

// Reusable, themed building blocks shared by the Cortex widgets. Everything pulls colour from
// the live `CortexTheme` so widgets re-skin with the in-app palette.

/// The Cortex brand mark — a soft brain/leaf glyph rendered from SF Symbols, accent-tinted.
struct BrandMark: View {
    let theme: CortexTheme
    var size: CGFloat = 13
    var body: some View {
        Image(systemName: "brain.head.profile")
            .font(.system(size: size, weight: .semibold))
            .foregroundStyle(theme.accent)
    }
}

/// A small uppercase monospace eyebrow label, matching the app's `.page-title` chrome.
struct Eyebrow: View {
    let text: String
    let theme: CortexTheme
    var body: some View {
        Text(text.uppercased())
            .font(theme.mono(9, weight: .semibold))
            .tracking(1.2)
            .foregroundStyle(theme.fgMuted)
            .lineLimit(1)
    }
}

/// Progress ring used by the dashboard + pomodoro + flashcards widgets.
struct RingView: View {
    let progress: Double
    let color: Color
    let track: Color
    var lineWidth: CGFloat = 5
    var body: some View {
        ZStack {
            Circle().stroke(track, lineWidth: lineWidth)
            Circle()
                .trim(from: 0, to: max(0.001, min(1, progress)))
                .stroke(color, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                .rotationEffect(.degrees(-90))
        }
    }
}

/// The signature record button — accent gradient disc with a soft glow + centred dot/glyph.
struct RecordButton: View {
    let theme: CortexTheme
    var isRecording: Bool = false
    var diameter: CGFloat = 54
    var body: some View {
        ZStack {
            Circle()
                .fill(theme.accentGradient)
                .shadow(color: theme.accent.opacity(0.55), radius: 10, y: 2)
            if isRecording {
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.bg)
                    .frame(width: diameter * 0.3, height: diameter * 0.3)
            } else {
                Circle()
                    .fill(theme.bg)
                    .frame(width: diameter * 0.34, height: diameter * 0.34)
            }
        }
        .frame(width: diameter, height: diameter)
    }
}

/// A single deadline row: urgency dot, title, course, and a live relative countdown.
struct DeadlineRow: View {
    let item: DeadlineItem
    let theme: CortexTheme
    var compact: Bool = false
    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(theme.urgency(hoursAway: item.hoursAway))
                .frame(width: 7, height: 7)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title)
                    .font(theme.rounded(compact ? 12 : 13, weight: .semibold))
                    .foregroundStyle(theme.fgBright)
                    .lineLimit(1)
                if !compact {
                    Text(item.course)
                        .font(theme.mono(9))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 4)
            Text(item.dueAt, style: .relative)
                .font(theme.mono(compact ? 9 : 10, weight: .medium))
                .foregroundStyle(theme.urgency(hoursAway: item.hoursAway))
                .lineLimit(1)
                .multilineTextAlignment(.trailing)
        }
    }
}

/// Applies the Cortex surface background to a widget, respecting iOS 17's containerBackground.
extension View {
    @ViewBuilder
    func cortexContainer(_ theme: CortexTheme) -> some View {
        if #available(iOS 17.0, *) {
            self.containerBackground(theme.bg, for: .widget)
        } else {
            self.background(theme.bg)
        }
    }
}
