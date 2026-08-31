//! Prefix-key state machine with a watchdog that releases stranded system-wide grabs.

use std::time::{Duration, Instant};

use crate::tabs::Tab;

/// How long one arm stays live before it lapses on its own.
pub const PREFIX_ARM_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefixState {
    #[default]
    Disarmed,
    Armed {
        armed_at: Instant,
    },
}

/// `Other` covers everything unmapped: disarm silently, never beep, never flash an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixKey {
    Digit(u8),
    BracketLeft,
    BracketRight,
    ExpandToggle,
    Pause,
    Disarm,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixAction {
    Select(Tab),
    PreviousSession,
    NextSession,
    ExpandToggle,
    TogglePause,
    NoOp,
}

impl PrefixState {
    /// Whether an armed window is CURRENTLY live — elapsed-aware, not a bare variant check.
    pub fn is_armed(&self, now: Instant) -> bool {
        matches!(*self, PrefixState::Armed { armed_at } if now.duration_since(armed_at) < PREFIX_ARM_WINDOW)
    }

    /// The global prefix combo fired.
    pub fn on_prefix(&mut self, now: Instant) -> PrefixAction {
        if self.is_armed(now) {
            *self = PrefixState::Disarmed;
        } else {
            *self = PrefixState::Armed { armed_at: now };
        }
        PrefixAction::NoOp
    }

    /// Any other key seen while the temporary grab is active. Defensively also a no-op (rather than
    /// acting on a stale key) if called while the window has already expired.
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
        s.on_prefix(t0 + Duration::from_secs(3));
        assert!(s.is_armed(t0 + Duration::from_millis(3500)));
    }

    #[test]
    fn on_key_digit_maps_through_tab_from_prefix_digit() {
        let t0 = Instant::now();
        let mut s = PrefixState::default();
        s.on_prefix(t0);
        let action = s.on_key(t0 + Duration::from_millis(100), PrefixKey::Digit(3));
        assert_eq!(action, PrefixAction::Select(Tab::News));
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
