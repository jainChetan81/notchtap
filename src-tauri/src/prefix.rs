//! Plan 171 (tab-notch redesign, slice D): the tmux-style prefix keymap's
//! ARM/DISARM state machine — pure, no AppKit types, no
//! `tauri_plugin_global_shortcut` dependency, same discipline `tabs.rs`
//! follows (`docs/TESTING_STRATEGY.md` §4.4). Spec
//! `docs/superpowers/specs/2026-08-02-tab-notch-design.md` §9's table is
//! the source of truth this module encodes.
//!
//! **What this module does NOT do**: register or release any actual OS
//! key grab. Spec §9's own "Mechanism note" says the real wiring needs a
//! genuinely different mechanism from the app's existing seven
//! `tauri_plugin_global_shortcut` combos — the prefix itself registers
//! globally as normal, but the SEVEN follow-up keys (`1`…`5`, `[`, `]`,
//! `enter`/`o`, `p`, `esc`) would need to be registered only for the
//! live 2-second window (bare, unmodified global shortcuts for keys like
//! "1" or "Return" registered PERMANENTLY would fire on every ordinary
//! keystroke anywhere on the system — not what's wanted), which means
//! dynamic `register`/`unregister` calls plus a cancellable 2s timer
//! task. That's architecturally sound (the same already-proven
//! `tauri_plugin_global_shortcut` API the existing seven combos use, not
//! a new native mechanism like Slice A's click-detection question), but
//! it's a genuinely live, timing-sensitive integration this Linux dev
//! environment cannot exercise or verify at all — left for real-device
//! wiring, same "pure logic now, live grab/release manual-only" split
//! the plan's own Verification section already draws. This module knows
//! nothing about `tauri_plugin_global_shortcut`; it is ready to be
//! driven by whichever wiring lands.
//!
//! `#![allow(dead_code)]`: staged ahead of that caller, same situation as
//! `tabs.rs`/`news_charge.rs` (see those files' doc comments) — `cargo
//! clippy --locked --all-targets -D warnings` (the CI gate) has no
//! exemption for a plain unused `pub` item. Remove once the lib.rs
//! wiring calls `on_prefix`/`on_key` for real.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use crate::tabs::Tab;

/// Spec §12 open question 1's stated default.
pub const PREFIX_ARM_WINDOW: Duration = Duration::from_secs(2);

/// Armed or not, and since when — `Default` is `Disarmed`, the app's own
/// launch state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefixState {
    #[default]
    Disarmed,
    Armed {
        armed_at: Instant,
    },
}

/// The seven follow-up keys spec §9's table recognizes while armed, in
/// already-abstracted form — mapping a raw NSEvent/tauri key code to one
/// of these variants (e.g. both `Return` and `O` collapse to
/// `ExpandToggle`) is the caller's job, deliberately kept out of this
/// AppKit-free module. `Other` covers everything unmapped — spec §9's
/// last row: "disarm silently — never beep, never flash an error."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixKey {
    /// 1..5 — the caller passes the raw digit; out-of-range values are a
    /// no-op via `Tab::from_prefix_digit`'s own `None` case, not a panic.
    Digit(u8),
    BracketLeft,
    BracketRight,
    /// `enter` or `o` — spec §9: "the *only* expansion gesture that
    /// exists anywhere in this feature."
    ExpandToggle,
    Pause,
    /// `esc`, or the prefix combo pressed again while already armed.
    Disarm,
    Other,
}

/// What the caller should DO in response to a consumed key — this slice
/// documents which EXISTING mechanism each maps to (plan's own §0 ask);
/// the caller (lib.rs, once wired) is what actually calls it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixAction {
    /// prefix+1..5: `TabSelection::select` (tabs.rs) with the tab
    /// `Tab::from_prefix_digit` resolves the digit to — same toggle
    /// semantics a click would drive (spec §9: "same key again
    /// deselects").
    Select(Tab),
    /// prefix+`[` / prefix+`]`: previous/next agent session. Spec §9:
    /// "ignored unless the agent tab is selected" — this state machine
    /// doesn't hold the current selection, so it always emits these;
    /// the caller checks `TabSelection::selected() == Some(Tab::Agent)`
    /// before acting and no-ops otherwise, the same "caller holds the
    /// cross-cutting state, this machine only classifies the keystroke"
    /// split `TabSelection::clear_if_gone` already uses for liveness.
    PreviousSession,
    NextSession,
    /// prefix+enter / prefix+o: reuses the EXISTING `⌃⇧N` expand-toggle
    /// mechanism (`try_expand_board_for_hover`/`collapse_board_if_
    /// expanded` in lib.rs) — not a new toggle, the same one spec §9
    /// calls out as the feature's only expansion gesture.
    ExpandToggle,
    /// prefix+p: reuses the EXISTING pause the tray item and `⌃⇧P`
    /// already drive.
    TogglePause,
    /// esc, the prefix again, or anything unmapped: no side effect.
    NoOp,
}

impl PrefixState {
    /// Whether an armed window is CURRENTLY live — elapsed-aware, not a
    /// bare variant check, so a window that has silently timed out reads
    /// as disarmed even before anything calls `on_key`/`on_prefix` again
    /// to formally clear it.
    pub fn is_armed(&self, now: Instant) -> bool {
        matches!(*self, PrefixState::Armed { armed_at } if now.duration_since(armed_at) < PREFIX_ARM_WINDOW)
    }

    /// The global prefix combo fired. Two distinct cases spec §9's table
    /// draws out separately: if a PREVIOUS armed window is still live,
    /// this press IS "the prefix again" (the table's explicit disarm
    /// row) — not a re-arm, not an extension of the window. If disarmed,
    /// OR a previous window already timed out, this arms a fresh
    /// 2-second window starting now. Getting this distinction backwards
    /// (treating a stale Armed variant as still-toggleable) would leave
    /// a user who pauses too long between the prefix and their next
    /// press unable to ever arm again without pressing the prefix twice.
    pub fn on_prefix(&mut self, now: Instant) -> PrefixAction {
        if self.is_armed(now) {
            *self = PrefixState::Disarmed;
        } else {
            *self = PrefixState::Armed { armed_at: now };
        }
        PrefixAction::NoOp
    }

    /// Any other key seen while the temporary grab is active. Spec §9:
    /// "the next keystroke does exactly one thing and disarms
    /// immediately, whether or not it matched anything" — so this always
    /// transitions to `Disarmed`, match or no match. Defensively also a
    /// no-op (rather than acting on a stale key) if called while the
    /// window has already expired — the caller is expected to release
    /// its temporary grab the moment `is_armed` goes false, but a real
    /// key-grab release is an actual syscall/dispatch that can lag its
    /// logical deadline by a beat, and this guards that race rather than
    /// trusting it always wins.
    pub fn on_key(&mut self, now: Instant, key: PrefixKey) -> PrefixAction {
        if !self.is_armed(now) {
            *self = PrefixState::Disarmed;
            return PrefixAction::NoOp;
        }
        *self = PrefixState::Disarmed;
        match key {
            PrefixKey::Digit(d) => Tab::from_prefix_digit(d)
                .map(PrefixAction::Select)
                .unwrap_or(PrefixAction::NoOp),
            PrefixKey::BracketLeft => PrefixAction::PreviousSession,
            PrefixKey::BracketRight => PrefixAction::NextSession,
            PrefixKey::ExpandToggle => PrefixAction::ExpandToggle,
            PrefixKey::Pause => PrefixAction::TogglePause,
            PrefixKey::Disarm | PrefixKey::Other => PrefixAction::NoOp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_disarmed() {
        let now = Instant::now();
        assert!(!PrefixState::default().is_armed(now));
    }

    #[test]
    fn on_prefix_from_disarmed_arms_and_returns_no_op() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        let action = s.on_prefix(t0);
        assert_eq!(action, PrefixAction::NoOp);
        assert!(s.is_armed(t0));
    }

    #[test]
    fn armed_window_is_live_just_under_two_seconds() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert!(s.is_armed(t0 + Duration::from_millis(1999)));
    }

    #[test]
    fn armed_window_has_expired_at_exactly_two_seconds() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert!(!s.is_armed(t0 + Duration::from_secs(2)));
    }

    #[test]
    fn on_prefix_again_while_still_armed_disarms_with_no_side_effect() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        let action = s.on_prefix(t0 + Duration::from_millis(500));
        assert_eq!(action, PrefixAction::NoOp);
        assert!(!s.is_armed(t0 + Duration::from_millis(500)));
    }

    #[test]
    fn on_prefix_after_the_window_expired_arms_fresh_instead_of_toggling_off() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        // window expired by now
        s.on_prefix(t0 + Duration::from_secs(3));
        // if this were mistakenly treated as a toggle-off, the window
        // below would read as disarmed instead of freshly armed.
        assert!(s.is_armed(t0 + Duration::from_millis(3500)));
    }

    #[test]
    fn on_key_digit_maps_through_tab_from_prefix_digit() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        let action = s.on_key(t0 + Duration::from_millis(100), PrefixKey::Digit(3));
        assert_eq!(action, PrefixAction::Select(Tab::Music));
    }

    #[test]
    fn on_key_out_of_range_digit_is_a_no_op() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        let action = s.on_key(t0 + Duration::from_millis(100), PrefixKey::Digit(9));
        assert_eq!(action, PrefixAction::NoOp);
    }

    #[test]
    fn on_key_brackets_map_to_previous_and_next_session() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(10), PrefixKey::BracketLeft),
            PrefixAction::PreviousSession
        );

        s.on_prefix(t0 + Duration::from_millis(20));
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(30), PrefixKey::BracketRight),
            PrefixAction::NextSession
        );
    }

    #[test]
    fn on_key_expand_toggle_maps_through() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(10), PrefixKey::ExpandToggle),
            PrefixAction::ExpandToggle
        );
    }

    #[test]
    fn on_key_pause_maps_to_toggle_pause() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(10), PrefixKey::Pause),
            PrefixAction::TogglePause
        );
    }

    #[test]
    fn on_key_disarm_and_other_are_both_no_ops() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(10), PrefixKey::Disarm),
            PrefixAction::NoOp
        );

        s.on_prefix(t0 + Duration::from_millis(20));
        assert_eq!(
            s.on_key(t0 + Duration::from_millis(30), PrefixKey::Other),
            PrefixAction::NoOp
        );
    }

    #[test]
    fn on_key_always_disarms_regardless_of_whether_it_matched() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        s.on_key(t0 + Duration::from_millis(10), PrefixKey::Digit(1));
        assert!(!s.is_armed(t0 + Duration::from_millis(10)));
    }

    #[test]
    fn on_key_called_while_already_disarmed_is_a_defensive_no_op() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        let action = s.on_key(t0, PrefixKey::Digit(1));
        assert_eq!(action, PrefixAction::NoOp);
    }

    #[test]
    fn on_key_called_after_the_window_expired_is_a_no_op_not_a_stale_action() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        let action = s.on_key(t0 + Duration::from_secs(3), PrefixKey::Digit(1));
        assert_eq!(
            action,
            PrefixAction::NoOp,
            "a key arriving after the window closed must not fire a stale action"
        );
    }
}
