//! Tracks how many items have landed since the news icon was last visited and whether a full batch
//! has accumulated by a poll-cycle boundary.
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

    pub fn item_landed(&mut self) {
        self.items_since_visit += 1;
    }

    /// Call once per poll-cycle boundary (`rss_poller.rs`'s `interval.tick()`, after every source
    /// in that tick has been diffed).
    pub fn cycle_end(&mut self) {
        if self.items_since_visit >= self.batch_size {
            self.charged = true;
        }
    }

    /// The news icon being visited (selected, or otherwise acknowledged) — cleared, not remembered.
    pub fn visit(&mut self) {
        self.items_since_visit = 0;
        self.charged = false;
    }

    pub fn fill(&self) -> f32 {
        (self.items_since_visit as f32 / self.batch_size as f32).min(1.0)
    }

    pub fn is_charged(&self) -> bool {
        self.charged
    }

    /// The literal count badge — items landed since the last visit, uncapped (unlike `fill`, which
    /// clamps for the animation).
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

        c.cycle_end();
        assert!(c.is_charged());

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
