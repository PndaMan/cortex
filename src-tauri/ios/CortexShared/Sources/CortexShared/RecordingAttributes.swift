import Foundation
#if canImport(ActivityKit)
import ActivityKit

/// Attributes for the lecture-recording Live Activity (Lock Screen + Dynamic Island).
/// Defined ONCE here in CortexShared so the app (which starts/updates the activity) and the
/// widget extension (which renders it) reference the exact same type — required for matching.
@available(iOS 16.1, *)
public struct RecordingAttributes: ActivityAttributes {
    public typealias ContentState = State

    /// Static, set when recording starts.
    public var subjectName: String
    public var accentHex: String

    public init(subjectName: String, accentHex: String) {
        self.subjectName = subjectName
        self.accentHex = accentHex
    }

    /// Dynamic, updated as recording runs.
    public struct State: Codable, Hashable {
        public var startedAt: Date
        public var isPaused: Bool
        /// Mic level 0…1 for the little waveform pulse.
        public var level: Double
        /// Total paused time so the Lock Screen `timer` text stays accurate across pauses.
        public var pausedAccumulated: TimeInterval

        public init(startedAt: Date, isPaused: Bool = false, level: Double = 0, pausedAccumulated: TimeInterval = 0) {
            self.startedAt = startedAt; self.isPaused = isPaused
            self.level = level; self.pausedAccumulated = pausedAccumulated
        }

        /// The Date the system should count up from (so SwiftUI `Text(timerInterval:)` is exact).
        public var countUpFrom: Date {
            startedAt.addingTimeInterval(pausedAccumulated)
        }
    }
}
#endif
