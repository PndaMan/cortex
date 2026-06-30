import SwiftUI

/// Bridges the in-app CSS palette (mirrored into `ThemeColors`) to SwiftUI. Widgets read the
/// live theme out of the snapshot so they re-skin exactly when the user changes palette in the
/// app — no widget-only colours, matching the "visual continuity is non-negotiable" rule.
public struct CortexTheme {
    public let colors: ThemeColors
    public init(_ colors: ThemeColors) { self.colors = colors }

    /// Pull the theme from the latest snapshot (falls back to Osaka Jade).
    public static var live: CortexTheme { CortexTheme(WidgetSnapshot.current.theme) }

    public var bg: Color { Color(hex: colors.bg) }
    public var surface: Color { Color(hex: colors.surface) }
    public var fg: Color { Color(hex: colors.fg) }
    public var fgBright: Color { Color(hex: colors.fgBright) }
    public var fgMuted: Color { Color(hex: colors.fgMuted) }
    public var accent: Color { Color(hex: colors.accent) }
    public var warn: Color { Color(hex: colors.warn) }
    public var err: Color { Color(hex: colors.err) }

    /// Urgency colour for a deadline: red within 6h, amber within 24h, accent otherwise.
    public func urgency(hoursAway: Double) -> Color {
        if hoursAway <= 6 { return err }
        if hoursAway <= 24 { return warn }
        return accent
    }

    /// Cortex headings are JetBrains Mono; widgets fall back to the system monospace
    /// (custom fonts in widget extensions are unreliable, so the system mono keeps the
    /// "technical" feel without bundling a font into the extension).
    public func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
    public func rounded(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .rounded)
    }

    /// The signature soft-glow gradient used behind the brand mark / record button.
    public var accentGradient: LinearGradient {
        LinearGradient(colors: [accent, accent.opacity(0.55)],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }
}

public extension Color {
    /// Parse `#rrggbb` / `#rrggbbaa` / `#rgb` hex strings (the format the app mirrors).
    init(hex raw: String) {
        var s = raw.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        if s.count == 3 { s = s.map { "\($0)\($0)" }.joined() }
        var v: UInt64 = 0
        Scanner(string: s).scanHexInt64(&v)
        let r, g, b, a: Double
        if s.count == 8 {
            r = Double((v >> 24) & 0xff) / 255; g = Double((v >> 16) & 0xff) / 255
            b = Double((v >> 8) & 0xff) / 255;  a = Double(v & 0xff) / 255
        } else {
            r = Double((v >> 16) & 0xff) / 255; g = Double((v >> 8) & 0xff) / 255
            b = Double(v & 0xff) / 255;         a = 1
        }
        self.init(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}
