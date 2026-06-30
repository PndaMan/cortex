import Foundation

/// The data contract written by the Cortex app (JS → plugin → App Group) and read by every
/// widget. Keep field names in sync with `src/lib/widgets.ts` `buildWidgetSnapshot()`.
/// Everything is optional-tolerant: a stale or partial snapshot still renders a sensible widget.
public struct WidgetSnapshot: Codable {
    public var updatedAt: Date
    public var theme: ThemeColors
    public var deadlines: [DeadlineItem]
    public var agenda: [AgendaItem]
    public var subjects: [SubjectProgress]
    public var streak: Int
    public var flashcardsDue: Int
    public var pomodoro: PomodoroState?
    public var activeSubject: String?

    public init(updatedAt: Date = Date(), theme: ThemeColors = .osakaJade,
                deadlines: [DeadlineItem] = [], agenda: [AgendaItem] = [],
                subjects: [SubjectProgress] = [], streak: Int = 0, flashcardsDue: Int = 0,
                pomodoro: PomodoroState? = nil, activeSubject: String? = nil) {
        self.updatedAt = updatedAt; self.theme = theme; self.deadlines = deadlines
        self.agenda = agenda; self.subjects = subjects; self.streak = streak
        self.flashcardsDue = flashcardsDue; self.pomodoro = pomodoro; self.activeSubject = activeSubject
    }

    /// Loaded from the App Group, or a designed placeholder when nothing has been written yet
    /// (fresh install, unsigned simulator without the entitlement, etc.).
    public static var current: WidgetSnapshot {
        AppGroup.read(WidgetSnapshot.self, from: AppGroup.snapshotFile) ?? .placeholder
    }

    public static let placeholder = WidgetSnapshot(
        deadlines: [
            DeadlineItem(id: "p1", title: "Problem Set 7", course: "Linear Algebra",
                         dueAt: Date().addingTimeInterval(3 * 3600), kind: .deadline),
            DeadlineItem(id: "p2", title: "Midterm", course: "Organic Chemistry",
                         dueAt: Date().addingTimeInterval(36 * 3600), kind: .exam),
        ],
        agenda: [
            AgendaItem(id: "a1", title: "Algorithms — Lecture 12", at: Date().addingTimeInterval(2 * 3600), course: "CS 240"),
            AgendaItem(id: "a2", title: "Lab: Titration", at: Date().addingTimeInterval(5 * 3600), course: "Chem 110"),
        ],
        subjects: [
            SubjectProgress(id: "s1", name: "Linear Algebra", progress: 0.72, accentHex: "#2dd5b7"),
            SubjectProgress(id: "s2", name: "Organic Chem", progress: 0.41, accentHex: "#e5c736"),
            SubjectProgress(id: "s3", name: "Algorithms", progress: 0.88, accentHex: "#63b07a"),
        ],
        streak: 12, flashcardsDue: 23,
        pomodoro: PomodoroState(running: false, phase: "work", remainingSec: 1500, progress: 0, label: "Focus 1 of 4"),
        activeSubject: "Linear Algebra")
}

public struct DeadlineItem: Codable, Identifiable {
    public enum Kind: String, Codable { case deadline, exam, announcement }
    public var id: String
    public var title: String
    public var course: String
    public var dueAt: Date
    public var kind: Kind
    public init(id: String, title: String, course: String, dueAt: Date, kind: Kind) {
        self.id = id; self.title = title; self.course = course; self.dueAt = dueAt; self.kind = kind
    }
    /// Hours from now (can be negative). Drives urgency colouring.
    public var hoursAway: Double { dueAt.timeIntervalSinceNow / 3600 }
}

public struct AgendaItem: Codable, Identifiable {
    public var id: String
    public var title: String
    public var at: Date
    public var course: String
    public init(id: String, title: String, at: Date, course: String) {
        self.id = id; self.title = title; self.at = at; self.course = course
    }
}

public struct SubjectProgress: Codable, Identifiable {
    public var id: String
    public var name: String
    public var progress: Double // 0…1
    public var accentHex: String
    public init(id: String, name: String, progress: Double, accentHex: String) {
        self.id = id; self.name = name; self.progress = progress; self.accentHex = accentHex
    }
}

public struct PomodoroState: Codable {
    public var running: Bool
    public var phase: String     // work | break | long
    public var remainingSec: Int
    public var progress: Double  // 0…1 elapsed
    public var label: String
    public init(running: Bool, phase: String, remainingSec: Int, progress: Double, label: String) {
        self.running = running; self.phase = phase; self.remainingSec = remainingSec
        self.progress = progress; self.label = label
    }
    public var mmss: String {
        let s = max(0, remainingSec)
        return String(format: "%02d:%02d", s / 60, s % 60)
    }
}

/// The live in-app palette, mirrored so widgets match whatever theme the user picked.
public struct ThemeColors: Codable {
    public var bg: String
    public var surface: String
    public var fg: String
    public var fgBright: String
    public var fgMuted: String
    public var accent: String
    public var warn: String
    public var err: String
    public init(bg: String, surface: String, fg: String, fgBright: String,
                fgMuted: String, accent: String, warn: String, err: String) {
        self.bg = bg; self.surface = surface; self.fg = fg; self.fgBright = fgBright
        self.fgMuted = fgMuted; self.accent = accent; self.warn = warn; self.err = err
    }
    enum CodingKeys: String, CodingKey {
        case bg, surface, fg, fgBright, fgMuted, accent, warn, err
    }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        bg = (try? c.decode(String.self, forKey: .bg)) ?? ThemeColors.osakaJade.bg
        surface = (try? c.decode(String.self, forKey: .surface)) ?? ThemeColors.osakaJade.surface
        fg = (try? c.decode(String.self, forKey: .fg)) ?? ThemeColors.osakaJade.fg
        fgBright = (try? c.decode(String.self, forKey: .fgBright)) ?? ThemeColors.osakaJade.fgBright
        fgMuted = (try? c.decode(String.self, forKey: .fgMuted)) ?? ThemeColors.osakaJade.fgMuted
        accent = (try? c.decode(String.self, forKey: .accent)) ?? ThemeColors.osakaJade.accent
        warn = (try? c.decode(String.self, forKey: .warn)) ?? ThemeColors.osakaJade.warn
        err = (try? c.decode(String.self, forKey: .err)) ?? ThemeColors.osakaJade.err
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(bg, forKey: .bg); try c.encode(surface, forKey: .surface)
        try c.encode(fg, forKey: .fg); try c.encode(fgBright, forKey: .fgBright)
        try c.encode(fgMuted, forKey: .fgMuted); try c.encode(accent, forKey: .accent)
        try c.encode(warn, forKey: .warn); try c.encode(err, forKey: .err)
    }

    /// Cortex default palette (Osaka Jade) — used until the app writes the live theme.
    public static let osakaJade = ThemeColors(
        bg: "#0e1813", surface: "#111c18", fg: "#c1c497", fgBright: "#f6f5dd",
        fgMuted: "#8a9a7e", accent: "#2dd5b7", warn: "#e5c736", err: "#ff5345")
}
