//! Pure, clock-free scheduling for **Silenced** (plan 146a; `CONTEXT.md`'s
//! Silenced/Silent Period/Timed Mute/Skip glossary entries).
//!
//! This module never reads the system clock — every function here takes
//! "now" as a plain value and returns a plain value, mirroring
//! `presentation::presentation_mode`'s "pure decision function, subprocess
//! call lives elsewhere" split (`docs/TESTING_STRATEGY.md` §4.4). The
//! engine-side wiring (a live `SilenceController` driven by
//! `chrono::Local::now()`, a tokio timer sleeping until `next_boundary`,
//! and the tray menu calling `start_mute`/`cancel_mute`/
//! `skip_current_window`) is a different executor's file surface
//! (`lib.rs`) — this module only has to make that wiring easy.
//!
//! ## Time representation
//!
//! Two units, both plain integers:
//!
//! - [`Minute`] (`u16`, `0..1440`) — a minute-of-day, local wall-clock
//!   time-of-day only (no date). This is what a [`Window`] is expressed
//!   and compared in.
//! - [`AbsoluteMinute`] (`u64`) — an ever-increasing minute counter the
//!   caller supplies for "now" and for mute/skip deadlines. The one
//!   contract: `absolute_minute % 1440` MUST equal the actual local
//!   minute-of-day (`local_hour * 60 + local_minute`), and it must
//!   increase by exactly `1440` every local midnight — i.e. it behaves
//!   like "days-since-some-fixed-point * 1440 + minute-of-day", not a
//!   UTC epoch counter (UTC epoch minutes modulo 1440 do not line up with
//!   *local* midnight for any timezone offset that isn't a whole number
//!   of days). [`absolute_minute`] below does this conversion from a
//!   `chrono::NaiveDateTime` (feed it `chrono::Local::now().naive_local()`)
//!   so callers don't have to hand-roll the arithmetic.
//!
//! ## Intended call pattern
//!
//! ```ignore
//! // once, at startup, from Config:
//! let mut controller = SilenceController::new(config.silence.enabled, config.silence.window);
//!
//! // on every tick / promotion decision:
//! let now = silence::absolute_minute(chrono::Local::now().naive_local());
//! if controller.is_silenced(now) { /* gate Medium/Low, still promote High */ }
//!
//! // to sleep instead of poll:
//! if let Some(boundary) = controller.next_boundary(now) {
//!     let wake_in_minutes = boundary.saturating_sub(now);
//!     // schedule a timer for `wake_in_minutes` out, then re-evaluate
//! }
//!
//! // tray actions:
//! controller.start_mute(30, now);   // "Mute 30m"
//! controller.cancel_mute();         // "Cancel mute"
//! controller.skip_current_window(now); // "Skip today"
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

/// A minute-of-day: `0` is local midnight, `1439` is 23:59. Never `1440` —
/// callers normalize via `% 1440`.
pub type Minute = u16;

/// Minutes in a day — the modulus every minute-of-day computation wraps on.
const MINUTES_PER_DAY: u16 = 1440;

/// An ever-increasing minute counter (`days * 1440 + minute_of_day`) the
/// caller supplies as "now" and that mute/skip deadlines are stored in. See
/// the module doc for the contract this must satisfy.
pub type AbsoluteMinute = u64;

/// Converts a local wall-clock date+time into an [`AbsoluteMinute`]. Pure —
/// takes the datetime as a value, never reads the clock itself. Callers
/// feed it `chrono::Local::now().naive_local()` (or any other source of
/// local wall-clock time); DST is handled implicitly because this only
/// ever looks at the wall-clock fields (`hour`/`minute`/day count), never
/// at a UTC offset.
pub fn absolute_minute(local: chrono::NaiveDateTime) -> AbsoluteMinute {
    use chrono::{Datelike, Timelike};
    let days = i64::from(local.date().num_days_from_ce());
    let minute_of_day = i64::from(local.time().hour()) * 60 + i64::from(local.time().minute());
    // `num_days_from_ce` is negative only for dates before 0001-01-01, which
    // never occurs for a running process's wall clock; the cast is safe.
    (days * i64::from(MINUTES_PER_DAY) + minute_of_day) as AbsoluteMinute
}

fn minute_of_day(now: AbsoluteMinute) -> Minute {
    (now % AbsoluteMinute::from(MINUTES_PER_DAY)) as Minute
}

/// Minutes from minute-of-day `from` until the next occurrence (possibly
/// tomorrow) of minute-of-day `target`, strictly greater than zero — i.e.
/// `from == target` resolves to "tomorrow" (`1440`), never "now". That
/// "equal means next day, not now" rule is exactly what
/// [`SilenceController::skip_current_window`] needs: a skip issued at the
/// exact instant a window starts must re-arm tomorrow, not immediately.
fn minutes_until_next(from: Minute, target: Minute) -> u16 {
    if target > from {
        target - from
    } else {
        MINUTES_PER_DAY - from + target
    }
}

/// Errors parsing a `"HH:MM-HH:MM"` silence window string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WindowParseError {
    #[error("malformed silence window {0:?}: expected \"HH:MM-HH:MM\" (24h)")]
    Malformed(String),
    #[error("silence window {0:?} has an out-of-range time (hours 0-23, minutes 0-59)")]
    OutOfRange(String),
    #[error("silence window {0:?} has equal start and end times, which is not a valid window")]
    ZeroLength(String),
}

/// A daily silence window, `[start, end)` in local wall-clock time-of-day.
/// May cross midnight (`start > end`, e.g. `"23:00-08:00"`) — the default
/// window (`"00:00-10:00"`) doesn't, but the type supports it since the
/// config format allows any two times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    start: Minute,
    end: Minute,
}

impl Window {
    /// Parses `"HH:MM-HH:MM"`. `start == end` is rejected — that would be
    /// either a zero-length window (nothing to silence) or, if read as
    /// "wrap all the way around", a 24-hour window, which the format has
    /// no unambiguous way to express, so it's simply invalid.
    pub fn parse(s: &str) -> Result<Self, WindowParseError> {
        let (start_str, end_str) = s
            .split_once('-')
            .ok_or_else(|| WindowParseError::Malformed(s.to_string()))?;
        let start = parse_hhmm(start_str, s)?;
        let end = parse_hhmm(end_str, s)?;
        if start == end {
            return Err(WindowParseError::ZeroLength(s.to_string()));
        }
        Ok(Self { start, end })
    }

    pub fn start_minute(&self) -> Minute {
        self.start
    }

    pub fn end_minute(&self) -> Minute {
        self.end
    }

    /// Half-open `[start, end)`: the window is active starting exactly at
    /// `start` and stops being active exactly at `end` (the boundary
    /// minute itself is NOT in the window). Handles midnight-crossing
    /// windows (`start > end`) exactly.
    pub fn in_window(&self, minute: Minute) -> bool {
        if self.start < self.end {
            minute >= self.start && minute < self.end
        } else {
            // start > end: the window is everything from `start` through
            // midnight up to (not including) `end` the next day.
            minute >= self.start || minute < self.end
        }
    }
}

fn parse_hhmm(part: &str, original: &str) -> Result<Minute, WindowParseError> {
    let (h, m) = part
        .split_once(':')
        .ok_or_else(|| WindowParseError::Malformed(original.to_string()))?;
    let h: u16 = h
        .parse()
        .map_err(|_| WindowParseError::Malformed(original.to_string()))?;
    let m: u16 = m
        .parse()
        .map_err(|_| WindowParseError::Malformed(original.to_string()))?;
    if h > 23 || m > 59 {
        return Err(WindowParseError::OutOfRange(original.to_string()));
    }
    Ok(h * 60 + m)
}

impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}-{:02}:{:02}",
            self.start / 60,
            self.start % 60,
            self.end / 60,
            self.end % 60
        )
    }
}

/// String round-trip so `Window` can sit directly in `config.toml` as
/// `window = "00:00-10:00"` — `Config::parse`'s `toml::from_str` fails with
/// a `toml::de::Error` wrapping [`WindowParseError`]'s message when the
/// string doesn't parse, the same "reject at deserialization" contract
/// `Priority`/`Units`/`SourceKind` already follow for unknown values.
impl TryFrom<String> for Window {
    type Error = WindowParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Window::parse(&value)
    }
}

impl From<Window> for String {
    fn from(window: Window) -> Self {
        window.to_string()
    }
}

impl Serialize for Window {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Window {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Window::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// The pure Silenced state machine: a configured schedule plus session-only
/// skip/mute state. Holds no timers — [`Self::next_boundary`] tells the
/// caller how long it can sleep before the verdict could change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilenceController {
    schedule_enabled: bool,
    window: Window,
    /// Set by [`Self::skip_current_window`] to the absolute minute of the
    /// window's next start; the skip is in effect for every `now` strictly
    /// before that boundary, then re-arms on its own (no explicit clear).
    skip_rearm_at: Option<AbsoluteMinute>,
    /// Set by [`Self::start_mute`] to the absolute minute the mute ends.
    mute_until: Option<AbsoluteMinute>,
}

impl SilenceController {
    pub fn new(schedule_enabled: bool, window: Window) -> Self {
        Self {
            schedule_enabled,
            window,
            skip_rearm_at: None,
            mute_until: None,
        }
    }

    fn mute_active(&self, now: AbsoluteMinute) -> bool {
        self.mute_until.is_some_and(|until| now < until)
    }

    fn skip_covers(&self, now: AbsoluteMinute) -> bool {
        self.skip_rearm_at.is_some_and(|rearm| now < rearm)
    }

    fn schedule_active(&self, now: AbsoluteMinute) -> bool {
        self.schedule_enabled && self.window.in_window(minute_of_day(now)) && !self.skip_covers(now)
    }

    /// The union of "the Silent Period is active and not skipped" and "a
    /// Timed Mute is running" — Silenced per `CONTEXT.md`'s definition.
    pub fn is_silenced(&self, now: AbsoluteMinute) -> bool {
        self.mute_active(now) || self.schedule_active(now)
    }

    /// Starts (or extends/replaces) a Timed Mute lasting `duration_minutes`
    /// from `now`. A second call before the first mute ends simply resets
    /// the deadline — there's no stacking, matching the tray's "one active
    /// mute at a time" preset UI.
    pub fn start_mute(&mut self, duration_minutes: u64, now: AbsoluteMinute) {
        self.mute_until = Some(now + duration_minutes);
    }

    /// Cancels a running Timed Mute early. A no-op if none is running.
    pub fn cancel_mute(&mut self) {
        self.mute_until = None;
    }

    /// Ends today's Silent Period early. Session-only: it re-arms
    /// automatically at the window's next start (per `CONTEXT.md`'s Skip
    /// entry), which is exactly the boundary this records. Skipping while
    /// not in the window (or with the schedule disabled) is harmless — it
    /// just pre-arms a no-op suppression that expires at the next start
    /// with nothing having changed.
    pub fn skip_current_window(&mut self, now: AbsoluteMinute) {
        let distance = minutes_until_next(minute_of_day(now), self.window.start_minute());
        self.skip_rearm_at = Some(now + u64::from(distance));
    }

    /// The next absolute minute worth re-evaluating [`Self::is_silenced`]
    /// at — `None` if nothing is scheduled to change it (schedule
    /// disabled and no mute running). Conservative: when a mute and the
    /// schedule overlap, this may return a boundary where the verdict
    /// does not actually flip (e.g. a window end still covered by a
    /// longer mute); callers sleep until this instant, recompute, and
    /// sleep again — a spurious wake is harmless, a missed flip is not.
    pub fn next_boundary(&self, now: AbsoluteMinute) -> Option<AbsoluteMinute> {
        let mute_boundary = self.mute_until.filter(|&until| now < until);

        let schedule_boundary = if self.schedule_enabled {
            if self.skip_covers(now) {
                // Suppressed until the skip re-arms; nothing else about the
                // schedule matters before then (the window's own end, if
                // any occurs first, doesn't change an already-false
                // verdict).
                self.skip_rearm_at
            } else {
                let m = minute_of_day(now);
                let target = if self.window.in_window(m) {
                    self.window.end_minute()
                } else {
                    self.window.start_minute()
                };
                Some(now + u64::from(minutes_until_next(m, target)))
            }
        } else {
            None
        };

        match (mute_boundary, schedule_boundary) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Window::parse ----

    #[test]
    fn parses_a_simple_window() {
        let w = Window::parse("00:00-10:00").unwrap();
        assert_eq!(w.start_minute(), 0);
        assert_eq!(w.end_minute(), 600);
    }

    #[test]
    fn parses_a_midnight_crossing_window() {
        let w = Window::parse("23:00-08:00").unwrap();
        assert_eq!(w.start_minute(), 23 * 60);
        assert_eq!(w.end_minute(), 8 * 60);
    }

    #[test]
    fn rejects_malformed_strings() {
        assert!(matches!(
            Window::parse("not a window"),
            Err(WindowParseError::Malformed(_))
        ));
        assert!(matches!(
            Window::parse("10:00"),
            Err(WindowParseError::Malformed(_))
        ));
        assert!(matches!(
            Window::parse("10-11:00"),
            Err(WindowParseError::Malformed(_))
        ));
        assert!(matches!(
            Window::parse("aa:00-10:00"),
            Err(WindowParseError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_out_of_range_times() {
        assert!(matches!(
            Window::parse("24:00-10:00"),
            Err(WindowParseError::OutOfRange(_))
        ));
        assert!(matches!(
            Window::parse("10:60-11:00"),
            Err(WindowParseError::OutOfRange(_))
        ));
    }

    #[test]
    fn rejects_equal_start_and_end() {
        assert!(matches!(
            Window::parse("10:00-10:00"),
            Err(WindowParseError::ZeroLength(_))
        ));
    }

    #[test]
    fn displays_as_the_canonical_hhmm_string() {
        let w = Window::parse("00:00-10:00").unwrap();
        assert_eq!(w.to_string(), "00:00-10:00");
        let w = Window::parse("9:05-17:30").unwrap();
        assert_eq!(w.to_string(), "09:05-17:30");
    }

    // ---- Window::in_window boundary inclusivity ----

    #[test]
    fn simple_window_includes_start_excludes_end() {
        let w = Window::parse("00:00-10:00").unwrap();
        assert!(w.in_window(0)); // start: inclusive
        assert!(w.in_window(1));
        assert!(w.in_window(599));
        assert!(!w.in_window(600)); // end: exclusive
        assert!(!w.in_window(1439));
    }

    #[test]
    fn midnight_crossing_window_includes_start_excludes_end() {
        let w = Window::parse("23:00-08:00").unwrap();
        assert!(w.in_window(23 * 60)); // start: inclusive
        assert!(w.in_window(23 * 60 + 30));
        assert!(w.in_window(0)); // through midnight
        assert!(w.in_window(7 * 60 + 59));
        assert!(!w.in_window(8 * 60)); // end: exclusive
        assert!(!w.in_window(12 * 60));
        assert!(!w.in_window(22 * 60 + 59));
    }

    // ---- absolute_minute ----

    #[test]
    fn absolute_minute_conversion_matches_wall_clock() {
        use chrono::NaiveDate;
        let dt = NaiveDate::from_ymd_opt(2026, 7, 27)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap();
        let am = absolute_minute(dt);
        assert_eq!(minute_of_day(am), 9 * 60 + 30);
        // exactly one day later is exactly 1440 minutes later
        let next_day = NaiveDate::from_ymd_opt(2026, 7, 28)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap();
        assert_eq!(absolute_minute(next_day) - am, 1440);
    }

    // ---- SilenceController: schedule only ----

    fn default_controller() -> SilenceController {
        SilenceController::new(true, Window::parse("00:00-10:00").unwrap())
    }

    #[test]
    fn silenced_inside_the_window_not_outside() {
        let c = default_controller();
        assert!(c.is_silenced(0));
        assert!(c.is_silenced(9 * 60 + 59));
        assert!(!c.is_silenced(10 * 60));
        assert!(!c.is_silenced(23 * 60));
    }

    #[test]
    fn disabled_schedule_never_silences_and_has_no_boundary() {
        let c = SilenceController::new(false, Window::parse("00:00-10:00").unwrap());
        assert!(!c.is_silenced(0));
        assert!(!c.is_silenced(9 * 60));
        assert_eq!(c.next_boundary(0), None);
    }

    #[test]
    fn midnight_crossing_schedule_silences_across_the_wrap() {
        let c = SilenceController::new(true, Window::parse("23:00-08:00").unwrap());
        assert!(c.is_silenced(23 * 60));
        assert!(c.is_silenced(0)); // day 1, just past midnight
        assert!(c.is_silenced(1440)); // day 2, exactly midnight, still in window
        assert!(!c.is_silenced(8 * 60));
        assert!(!c.is_silenced(12 * 60));
    }

    // ---- Timed Mute ----

    #[test]
    fn mute_silences_outside_the_schedule_window() {
        let mut c = default_controller();
        assert!(!c.is_silenced(12 * 60));
        c.start_mute(60, 12 * 60);
        assert!(c.is_silenced(12 * 60));
        assert!(c.is_silenced(12 * 60 + 59));
        assert!(!c.is_silenced(13 * 60)); // deadline reached: mute over
    }

    #[test]
    fn mute_outlasting_the_schedule_window_keeps_silencing() {
        // mute starts at 09:00 for 2h, schedule window ends at 10:00 —
        // silenced must continue past the schedule boundary to 11:00.
        let mut c = default_controller();
        c.start_mute(120, 9 * 60);
        assert!(c.is_silenced(9 * 60 + 30)); // schedule-covered too
        assert!(c.is_silenced(10 * 60 + 30)); // schedule ended, mute still running
        assert!(!c.is_silenced(11 * 60)); // mute deadline reached
    }

    #[test]
    fn cancel_mute_ends_it_immediately() {
        let mut c = default_controller();
        c.start_mute(60, 12 * 60);
        assert!(c.is_silenced(12 * 60 + 5));
        c.cancel_mute();
        assert!(!c.is_silenced(12 * 60 + 5));
    }

    #[test]
    fn cancel_mute_is_a_no_op_when_nothing_is_running() {
        let mut c = default_controller();
        c.cancel_mute();
        assert!(!c.is_silenced(12 * 60));
    }

    #[test]
    fn union_of_overlapping_mute_and_schedule_is_silenced() {
        // mute starting inside the schedule window still reads as
        // silenced (union, not exclusive)
        let mut c = default_controller();
        c.start_mute(30, 5 * 60);
        assert!(c.is_silenced(5 * 60 + 10));
    }

    // ---- Skip ----

    #[test]
    fn skip_suppresses_the_current_window() {
        let mut c = default_controller();
        assert!(c.is_silenced(5 * 60));
        c.skip_current_window(5 * 60);
        assert!(!c.is_silenced(5 * 60));
        assert!(!c.is_silenced(9 * 60 + 59)); // stays suppressed through the rest of today's window
    }

    #[test]
    fn skip_rearms_at_the_next_window_start() {
        let mut c = default_controller();
        c.skip_current_window(5 * 60);
        // next start is tomorrow at 00:00, i.e. absolute minute 1440
        assert!(!c.is_silenced(1440 - 1));
        assert!(c.is_silenced(1440));
        assert!(c.is_silenced(1440 + 5 * 60));
    }

    #[test]
    fn skip_issued_exactly_at_window_start_rearms_the_next_day_not_immediately() {
        let mut c = default_controller();
        c.skip_current_window(0); // exactly at start
        assert!(!c.is_silenced(0));
        assert!(!c.is_silenced(600));
        assert!(c.is_silenced(1440)); // re-armed the next day, not "now"
    }

    #[test]
    fn skip_does_not_suppress_a_mute() {
        let mut c = default_controller();
        c.skip_current_window(5 * 60);
        c.start_mute(30, 5 * 60);
        assert!(c.is_silenced(5 * 60 + 10)); // mute still applies
    }

    // ---- next_boundary ----

    #[test]
    fn next_boundary_from_outside_the_window_is_its_start() {
        let c = default_controller();
        assert_eq!(c.next_boundary(23 * 60), Some(1440)); // next midnight
    }

    #[test]
    fn next_boundary_from_inside_the_window_is_its_end() {
        let c = default_controller();
        assert_eq!(c.next_boundary(0), Some(600));
        assert_eq!(c.next_boundary(300), Some(600));
    }

    #[test]
    fn next_boundary_prefers_the_sooner_of_mute_and_schedule() {
        let mut c = default_controller();
        // inside the window (ends at 600); mute ends sooner, at 100
        c.start_mute(100, 0);
        assert_eq!(c.next_boundary(0), Some(100));

        // mute outlasts the schedule window: the window end (600) comes
        // back first — a conservative wake where the verdict stays
        // silenced (mute still running) — and re-evaluating from there
        // yields the mute deadline.
        let mut c2 = default_controller();
        c2.start_mute(1000, 0);
        assert_eq!(c2.next_boundary(0), Some(600));
        assert!(c2.is_silenced(600));
        assert_eq!(c2.next_boundary(600), Some(1000));
    }

    #[test]
    fn next_boundary_with_only_a_mute_running_outside_the_window() {
        let mut c = default_controller();
        c.start_mute(30, 12 * 60);
        assert_eq!(c.next_boundary(12 * 60), Some(12 * 60 + 30));
    }

    #[test]
    fn next_boundary_while_skipped_is_the_rearm_point_not_the_window_end() {
        let mut c = default_controller();
        c.skip_current_window(0);
        // window would naturally end at 600, but skip suppresses until the
        // next start at 1440 — that's the real next boundary.
        assert_eq!(c.next_boundary(0), Some(1440));
    }

    #[test]
    fn next_boundary_is_none_when_nothing_is_active_or_scheduled() {
        let c = SilenceController::new(false, Window::parse("00:00-10:00").unwrap());
        assert_eq!(c.next_boundary(0), None);
    }

    // ---- Window serde round-trip (used directly by config.rs) ----

    #[test]
    fn window_serializes_and_deserializes_as_its_canonical_string() {
        let w = Window::parse("00:00-10:00").unwrap();
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(json, "\"00:00-10:00\"");
        let round_tripped: Window = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, w);
    }

    #[test]
    fn window_deserialize_rejects_malformed_strings() {
        let result: Result<Window, _> = serde_json::from_str("\"garbage\"");
        assert!(result.is_err());
    }
}
