//! Plan 171 (tab-notch redesign, slice A): the icon-strip SELECTION state
//! machine — pure, no AppKit types, no lock, no I/O, same discipline
//! `hover.rs` follows (`docs/TESTING_STRATEGY.md` §4.4).
//!
//! Not yet wired into `lib.rs`: the actual click-detection mechanism that
//! would call `TabSelection::select` is an open design question (see
//! `plans/171-tab-notch-redesign.md`'s own note) — whether it needs a new
//! native NSEvent monitor, or whether a plain webview `onClick` already
//! reaches the frontend once click-through is off on this app's
//! non-activating overlay panel, is unverified from a Linux dev
//! environment and needs one empirical check on real macOS hardware
//! before either is built. This module is ready for either answer; it
//! knows about neither AppKit nor tauri events.
//!
//! `#![allow(dead_code)]`: every item here is staged ahead of the real
//! caller that lands with Slice A's click-detection wiring above —
//! `cargo clippy --all-targets -D warnings` (the CI gate, justfile's
//! `check-rust`) has no exemption for a plain `pub fn` the way it does
//! for `#[cfg(test)]`-reached items, so this whole module reads as dead
//! until that caller exists. Remove this attribute the moment
//! `TabSelection` gets a real call site outside its own tests.
#![allow(dead_code)]

/// The five sources the icon strip can select, in the strip's fixed
/// left-to-right order (spec `docs/superpowers/specs/2026-08-02-tab-
/// notch-design.md` §6's table) — the SAME order `prefix+1..5` maps onto
/// (spec §9's keymap table: "1…5 select agent/football/music/weather/
/// news, strip order, left to right").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Agent,
    Football,
    Music,
    Weather,
    News,
}

impl Tab {
    /// Fixed strip order — the single source of truth every other "which
    /// index is which tab" mapping (icon-rect layout, `prefix+N`) reads
    /// from, so the order can never drift between the two call sites.
    pub const ORDER: [Tab; 5] = [
        Tab::Agent,
        Tab::Football,
        Tab::Music,
        Tab::Weather,
        Tab::News,
    ];

    /// `prefix+1`..`prefix+5` (spec §9) — `None` for anything outside
    /// that range, so the caller's "anything else: disarm, do nothing"
    /// rule (spec §9's keymap table, last row) falls out of a plain
    /// `if let Some(tab) = Tab::from_prefix_digit(k) { … }` at the call
    /// site rather than needing its own bounds check.
    pub fn from_prefix_digit(digit: u8) -> Option<Tab> {
        match digit {
            1 => Some(Tab::Agent),
            2 => Some(Tab::Football),
            3 => Some(Tab::Music),
            4 => Some(Tab::Weather),
            5 => Some(Tab::News),
            _ => None,
        }
    }
}

/// At most one selected tab, or none. Spec §2 decision 5 / §7: "max one,
/// or none", "remembered silently across hovers", "cleared, not
/// remembered" if its source stops being live. `Default` is `None`
/// selected — the empty state is the app's own launch state and a
/// first-class state per the spec, not a placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabSelection {
    selected: Option<Tab>,
}

impl TabSelection {
    pub fn selected(&self) -> Option<Tab> {
        self.selected
    }

    /// A click on `tab` (spec §2 decision 5): the SAME tab clicked again
    /// deselects; a different tab (or nothing previously selected) moves
    /// the selection to it. This is also what `prefix+1..5` drives (spec
    /// §9's keymap table: "the same key again deselects") — one rule,
    /// two callers, not two mechanisms; see `select` below.
    pub fn select(&mut self, tab: Tab) {
        self.selected = if self.selected == Some(tab) {
            None
        } else {
            Some(tab)
        };
    }

    pub fn deselect(&mut self) {
        self.selected = None;
    }

    /// Spec §2 decision 5 / §3's "a selection whose icon disappears is
    /// cleared, not remembered": call after every liveness change with a
    /// predicate answering "is the CURRENTLY selected tab still present"
    /// (weather/news are always present whenever the strip is up, per
    /// spec §6's table, so this is a no-op for them by construction —
    /// only agent/football/music can genuinely go present -> absent
    /// mid-selection). A no-op when nothing is selected.
    pub fn clear_if_gone(&mut self, is_present: impl FnOnce(Tab) -> bool) {
        if let Some(tab) = self.selected {
            if !is_present(tab) {
                self.selected = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_is_none() {
        assert_eq!(TabSelection::default().selected(), None);
    }

    #[test]
    fn selecting_a_tab_selects_it() {
        let mut s = TabSelection::default();
        s.select(Tab::Weather);
        assert_eq!(s.selected(), Some(Tab::Weather));
    }

    #[test]
    fn selecting_the_same_tab_again_deselects_it() {
        let mut s = TabSelection::default();
        s.select(Tab::Weather);
        s.select(Tab::Weather);
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn selecting_a_different_tab_moves_the_selection_not_adds_to_it() {
        let mut s = TabSelection::default();
        s.select(Tab::Weather);
        s.select(Tab::News);
        assert_eq!(s.selected(), Some(Tab::News));
    }

    #[test]
    fn deselect_clears_regardless_of_current_state() {
        let mut s = TabSelection::default();
        s.deselect();
        assert_eq!(s.selected(), None);
        s.select(Tab::Agent);
        s.deselect();
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn clear_if_gone_is_a_no_op_when_nothing_is_selected() {
        let mut s = TabSelection::default();
        s.clear_if_gone(|_| false);
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn clear_if_gone_clears_only_when_the_selected_tab_is_no_longer_present() {
        let mut s = TabSelection::default();
        s.select(Tab::Music);
        s.clear_if_gone(|tab| tab != Tab::Music); // Music is absent
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn clear_if_gone_leaves_the_selection_when_still_present() {
        let mut s = TabSelection::default();
        s.select(Tab::Music);
        s.clear_if_gone(|tab| tab == Tab::Music); // still present
        assert_eq!(s.selected(), Some(Tab::Music));
    }

    #[test]
    fn from_prefix_digit_maps_1_through_5_in_strip_order() {
        assert_eq!(Tab::from_prefix_digit(1), Some(Tab::Agent));
        assert_eq!(Tab::from_prefix_digit(2), Some(Tab::Football));
        assert_eq!(Tab::from_prefix_digit(3), Some(Tab::Music));
        assert_eq!(Tab::from_prefix_digit(4), Some(Tab::Weather));
        assert_eq!(Tab::from_prefix_digit(5), Some(Tab::News));
    }

    #[test]
    fn from_prefix_digit_rejects_anything_out_of_1_to_5() {
        assert_eq!(Tab::from_prefix_digit(0), None);
        assert_eq!(Tab::from_prefix_digit(6), None);
    }

    #[test]
    fn order_matches_from_prefix_digit_index_for_index() {
        for (i, tab) in Tab::ORDER.iter().enumerate() {
            assert_eq!(Tab::from_prefix_digit((i + 1) as u8), Some(*tab));
        }
    }
}
