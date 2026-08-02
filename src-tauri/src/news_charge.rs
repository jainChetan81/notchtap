//! Plan 171 (tab-notch redesign, slice B): the news icon's CHARGE state
//! machine — pure, no I/O, same discipline `tabs.rs` follows
//! (`docs/TESTING_STRATEGY.md` §4.4). Tracks how many items have landed
//! since the news icon was last visited and whether a full batch has
//! accumulated by a poll-cycle boundary, per spec
//! `docs/superpowers/specs/2026-08-02-tab-notch-design.md` §8 / open
//! question 4's default (ship both the fill level and the count badge).
//!
//! Mirrors `weather_poller.rs`'s `WeatherAlertState` edge-trigger
//! discipline: `charged` is not live "is a full batch sitting there right
//! now" arithmetic re-evaluated on every read — `cycle_end()` sets it
//! once, at the moment a cycle closes with the batch full, and only
//! `visit()` clears it. A charge earned on one cycle survives however
//! many further cycles pass without a visit, the same way
//! `WeatherAlertState::rain_fired` stays `true` until the condition
//! clears rather than the next poll silently unfiring it.
//!
//! **Design decision, recorded rather than left implicit**: "charged"
//! means cycle-ended AND the batch is FULL (`items_since_visit >=
//! batch_size`), not cycle-ended with merely *any* items waiting. This
//! follows the mission brief's own literal wording — "glowing when the
//! user-defined batch is ready" — over the looser "anything landed"
//! reading. If that reading turns out wrong once this is on real
//! hardware next to the actual RSS cadence, the fix is a single
//! comparison in `cycle_end` below, not a restructure.
//!
//! Not yet wired into `rss_poller.rs`'s poll loop or into `StatusState`
//! (`status.rs`): the wire shape this feeds (`StatusState`'s icon-
//! presence extension) is Slice A's call per the plan's own §0 cross-
//! slice contract note, and Slice A's remaining scope is itself gated on
//! the Mac Mini click-detection hand-off — wiring this in ahead of that
//! risks inventing a shape Slice A/K's integration then has to unwind.
//! This module knows about neither `rss_poller.rs` nor `StatusState`; it
//! is ready to be driven by whichever one ends up calling it.
//!
//! `#![allow(dead_code)]`: same staged-ahead-of-its-caller situation as
//! `tabs.rs` — `cargo clippy --all-targets -D warnings` (the CI gate,
//! justfile's `check-rust`) has no exemption for a plain `pub fn` the way
//! it does for `#[cfg(test)]`-reached items. Remove this attribute the
//! moment `NewsCharge` gets a real call site outside its own tests.
#![allow(dead_code)]

/// Tracks landed-since-visit count and the edge-triggered "charged" flag
/// for the news icon. `batch_size` is clamped to at least 1 at
/// construction — a misconfigured `0` would otherwise make every
/// `fill()` call divide by zero and every cycle instantly "full".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewsCharge {
    items_since_visit: usize,
    batch_size: usize,
    charged: bool,
}

impl NewsCharge {
    pub fn new(batch_size: usize) -> Self {
        Self {
            items_since_visit: 0,
            batch_size: batch_size.max(1),
            charged: false,
        }
    }

    /// Call once per item landed by the poller, as items are found —
    /// mid-cycle, before `cycle_end` closes it out. Does not itself
    /// decide charged; only `cycle_end` evaluates the batch threshold, so
    /// the icon never flashes charged mid-cycle on a partial count.
    pub fn item_landed(&mut self) {
        self.items_since_visit += 1;
    }

    /// Call once per poll-cycle boundary (`rss_poller.rs`'s
    /// `interval.tick()`, after every source in that tick has been
    /// diffed). Sets `charged` when the accumulated count has reached the
    /// batch size; leaves an already-`true` flag untouched on a cycle
    /// that lands nothing new, the same "stays fired until it clears"
    /// persistence `WeatherAlertState` uses.
    pub fn cycle_end(&mut self) {
        if self.items_since_visit >= self.batch_size {
            self.charged = true;
        }
    }

    /// The news icon being visited (selected, or otherwise acknowledged)
    /// — spec §7/§8: "cleared, not remembered". Resets both the count and
    /// the charge, re-arming the edge trigger for the next batch.
    pub fn visit(&mut self) {
        self.items_since_visit = 0;
        self.charged = false;
    }

    /// `0.0..=1.0`, clamped — the interior fill level the icon's charging
    /// animation reads (`icon-strip.css`'s `.charge` transform, plan
    /// 171's `NEWS_CHARGE_STEP_MS` token). Never exceeds `1.0` even once
    /// `items_since_visit` overshoots `batch_size` (a cycle can land more
    /// than one batch's worth at once).
    pub fn fill(&self) -> f32 {
        (self.items_since_visit as f32 / self.batch_size as f32).min(1.0)
    }

    pub fn is_charged(&self) -> bool {
        self.charged
    }

    /// The literal count badge (spec §12 open question 5's "ship both"
    /// default) — items landed since the last visit, uncapped (unlike
    /// `fill`, which clamps for the animation).
    pub fn count(&self) -> usize {
        self.items_since_visit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_uncharged() {
        let c = NewsCharge::new(5);
        assert_eq!(c.count(), 0);
        assert_eq!(c.fill(), 0.0);
        assert!(!c.is_charged());
    }

    #[test]
    fn items_landing_raise_the_fill_fraction_but_do_not_charge_mid_cycle() {
        let mut c = NewsCharge::new(5);
        c.item_landed();
        c.item_landed();
        c.item_landed();
        assert_eq!(c.count(), 3);
        assert_eq!(c.fill(), 0.6);
        assert!(
            !c.is_charged(),
            "charging is decided at cycle_end, not on arrival"
        );
    }

    #[test]
    fn cycle_end_with_a_partial_batch_does_not_charge() {
        let mut c = NewsCharge::new(5);
        c.item_landed();
        c.item_landed();
        c.item_landed();
        c.cycle_end();
        assert!(!c.is_charged());
        assert_eq!(c.fill(), 0.6);
    }

    #[test]
    fn cycle_end_with_a_full_batch_charges() {
        let mut c = NewsCharge::new(5);
        for _ in 0..5 {
            c.item_landed();
        }
        c.cycle_end();
        assert!(c.is_charged());
        assert_eq!(c.fill(), 1.0);
    }

    #[test]
    fn overshooting_the_batch_size_in_one_cycle_still_charges_and_clamps_fill() {
        let mut c = NewsCharge::new(5);
        for _ in 0..8 {
            c.item_landed();
        }
        c.cycle_end();
        assert!(c.is_charged());
        assert_eq!(c.count(), 8, "the literal badge is uncapped");
        assert_eq!(c.fill(), 1.0, "the animation fraction clamps at 1.0");
    }

    #[test]
    fn charged_state_persists_across_further_cycles_without_a_visit() {
        let mut c = NewsCharge::new(3);
        for _ in 0..3 {
            c.item_landed();
        }
        c.cycle_end();
        assert!(c.is_charged());

        // a later cycle with nothing new lands must not silently unfire it
        c.cycle_end();
        assert!(c.is_charged());

        // nor does a cycle that lands one more item
        c.item_landed();
        c.cycle_end();
        assert!(c.is_charged());
    }

    #[test]
    fn visit_clears_count_and_charge() {
        let mut c = NewsCharge::new(3);
        for _ in 0..3 {
            c.item_landed();
        }
        c.cycle_end();
        assert!(c.is_charged());

        c.visit();
        assert_eq!(c.count(), 0);
        assert_eq!(c.fill(), 0.0);
        assert!(!c.is_charged());
    }

    #[test]
    fn a_fresh_batch_can_charge_again_after_a_visit() {
        let mut c = NewsCharge::new(2);
        c.item_landed();
        c.item_landed();
        c.cycle_end();
        assert!(c.is_charged());
        c.visit();

        c.item_landed();
        c.cycle_end();
        assert!(!c.is_charged(), "only one item landed since the visit");

        c.item_landed();
        c.cycle_end();
        assert!(c.is_charged(), "the second item completes a fresh batch");
    }

    #[test]
    fn visit_with_nothing_landed_is_a_harmless_no_op() {
        let mut c = NewsCharge::new(4);
        c.visit();
        assert_eq!(c.count(), 0);
        assert!(!c.is_charged());
    }

    #[test]
    fn zero_batch_size_is_clamped_to_one_to_avoid_a_divide_by_zero() {
        let mut c = NewsCharge::new(0);
        assert_eq!(c.fill(), 0.0);
        c.item_landed();
        assert_eq!(c.fill(), 1.0);
        c.cycle_end();
        assert!(c.is_charged());
    }
}
