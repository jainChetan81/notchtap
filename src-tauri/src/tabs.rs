//! The Icon Strip's SELECTION state machine — pure, no AppKit types, no
//! lock, no I/O, same discipline `hover.rs` follows
//! (`docs/TESTING_STRATEGY.md` §4.4). `click.rs`'s NSEvent monitor is the
//! click path, `lib.rs` owns the live `Arc<TabState>`, and the engine's
//! status loop drives `clear_if_gone` off the same presence snapshot the
//! strip renders from — every caller calls in, never the other way.

/// The three sources the Icon Strip can select, in the strip's fixed
/// left-to-right order — the SAME
/// order `prefix+1..3` maps onto: digits select in strip order, left to
/// right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Agent,
    Football,
    News,
}

impl Tab {
    /// Fixed strip order — the single source of truth every other "which
    /// index is which tab" mapping (icon-rect layout, `prefix+N`) reads
    /// from, so the order can never drift between the two call sites.
    pub const ORDER: [Tab; 3] = [Tab::Agent, Tab::Football, Tab::News];

    /// `prefix+1`..`prefix+3` — `None` for anything outside that range,
    /// so the caller's "anything else: disarm, do nothing" rule falls
    /// out of a plain `if let Some(tab) = Tab::from_prefix_digit(k) { … }`
    /// at the call site rather than needing its own bounds check.
    pub fn from_prefix_digit(digit: u8) -> Option<Tab> {
        match digit {
            1 => Some(Tab::Agent),
            2 => Some(Tab::Football),
            3 => Some(Tab::News),
            _ => None,
        }
    }
}

/// At most one selected Tab, or none: remembered silently across hovers,
/// and CLEARED (not remembered) if its source stops being live.
/// `Default` is `None` selected — the empty state is the app's own launch
/// state and a first-class state, not a placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabSelection {
    selected: Option<Tab>,
}

impl TabSelection {
    pub fn selected(&self) -> Option<Tab> {
        self.selected
    }

    /// A click on `tab`: the SAME tab clicked again deselects; a
    /// different tab moves the selection
    /// to it. `prefix+1..3` drives this same method — one rule, two
    /// callers, not two mechanisms.
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

    /// A selection whose icon disappears is cleared, not remembered:
    /// call after every liveness change with a predicate answering "is
    /// the CURRENTLY selected tab still present". News is always present
    /// whenever the strip is up, so this is a no-op for it by
    /// construction — only agent/football can genuinely go present ->
    /// absent mid-selection. A no-op when nothing is selected.
    pub fn clear_if_gone(&mut self, is_present: impl FnOnce(Tab) -> bool) {
        if let Some(tab) = self.selected {
            if !is_present(tab) {
                self.selected = None;
            }
        }
    }
}

impl Tab {
    /// The wire token `tab-selection-changed` carries — the closed set:
    /// `"agent" | "football" | "news"`.
    pub fn wire_label(self) -> &'static str {
        match self {
            Tab::Agent => "agent",
            Tab::Football => "football",
            Tab::News => "news",
        }
    }
}

/// Which tabs are PRESENT given the current ambient state: news always,
/// agent/football only while genuinely live. Returned in `Tab::ORDER`
/// order — the same order
/// `hover::icon_strip_rects` lays boxes out in, so a caller can zip the
/// two index-for-index (that pairing is the whole click hit-test).
pub fn present_tabs(state: &crate::status::StatusState) -> Vec<Tab> {
    Tab::ORDER
        .iter()
        .copied()
        .filter(|tab| match tab {
            Tab::Agent => state.agent.active_sessions > 0,
            Tab::Football => state.football.live.is_some(),
            Tab::News => true,
        })
        .collect()
}

/// The one shared, live tab-state bundle (`lib.rs` owns the `Arc`): the
/// selection itself, the last selection actually emitted over the wire
/// (`tab-selection-changed` fires on transitions only, mirroring
/// `emit_hover_changed_if_transitioned`'s discipline), and the presence
/// snapshot the click monitor hit-tests against — written by the
/// engine's status loop from the SAME `StatusState` the frontend renders
/// the strip from, so both sides always derive geometry from one source.
#[derive(Debug, Default)]
pub struct TabState {
    pub selection: std::sync::Mutex<TabSelection>,
    pub last_emitted: std::sync::Mutex<Option<Tab>>,
    pub presence: std::sync::Mutex<Vec<Tab>>,
}

/// The ONE shared wire bundle (`lib.rs` owns the `Arc`): the click
/// monitor, the Prefix keymap, the engine's status loop, the Agent
/// Board's session-count mirror, and the rss poller's charge feed all
/// read/write through this rather than each holding its own handle soup
/// — one `Engine::new` param instead of four.
#[derive(Debug)]
pub struct TabWire {
    /// Live Agent Session count, mirrored by
    /// `AgentBoardPublisher::publish_if_changed` (which already recomputes
    /// on every registry change) — the status loop reads it instead of
    /// taking a second async registry lock inside the queue's own
    /// critical section.
    pub agent_sessions: std::sync::atomic::AtomicUsize,
    /// The news-charge state machine (`news_charge.rs`), fed by
    /// `rss_poller.rs` (`item_landed`/`cycle_end`) and cleared by the
    /// selection paths (`visit` on selecting the news tab).
    pub news_charge: std::sync::Mutex<crate::news_charge::NewsCharge>,
    /// Selection + emission + presence — see [`TabState`].
    pub tabs: TabState,
    /// The Prefix keymap's arm/disarm state machine (`prefix.rs`) plus
    /// the generation counter its cancellable disarm timer checks — a
    /// timer only acts if no later arm/consume bumped the generation out
    /// from under it.
    pub prefix: std::sync::Mutex<crate::prefix::PrefixState>,
    pub prefix_generation: std::sync::atomic::AtomicU64,
    /// When the NEWEST arm happened. The watchdog stays
    /// generation-BLIND by design (see `followups_registered` below), so
    /// this instant — not the generation counter — is what tells an older
    /// watchdog "a newer legitimate window is still inside its own budget,
    /// sleep again" instead of force-releasing grabs that window still
    /// needs. `None` until the first arm; never cleared on disarm, because
    /// a stale arm instant reads as "long past its deadline" anyway, which
    /// is exactly the release verdict we want.
    pub last_arm_at: std::sync::Mutex<Option<std::time::Instant>>,
    /// The agent below-block's VIEWED session index (`prefix-[` /
    /// `prefix-]` cycling) — wraps modulo the live session
    /// count; emitted to the frontend as `agent-viewed-session-changed`.
    pub viewed_session: std::sync::atomic::AtomicUsize,
    /// Fired whenever `viewed_session` changes for ANY reason (manual
    /// prefix-key cycling or the auto-advance timer below) — lets the
    /// auto-advance loop's wait reset on a manual advance instead of
    /// firing again moments later, so manual navigation resets the
    /// auto-advance clock. Mirrors the
    /// `tokio::sync::Notify` pattern `engine.rs`'s own rotation loop
    /// already uses for "sleep until deadline, wake early on mutation."
    pub session_advanced: tokio::sync::Notify,
    /// TRUE whenever the bare follow-up keys are currently grabbed
    /// system-wide. The watchdog reads ONLY this — never the generation
    /// counter — so a wedged runtime, a lost timer, or a panic that
    /// unwound past the normal release path still gets caught.
    pub followups_registered: std::sync::atomic::AtomicBool,
    /// Whether a pushed card currently occupies the Slot — mirrored at
    /// every `emit_slot_state` site so the click monitor (a sync
    /// main-thread AppKit callback that cannot await the queue) can gate
    /// on "the strip is actually what's on screen".
    pub slot_occupied: std::sync::atomic::AtomicBool,
}

impl TabWire {
    pub fn new(news_batch_size: usize) -> Self {
        Self {
            agent_sessions: std::sync::atomic::AtomicUsize::new(0),
            news_charge: std::sync::Mutex::new(crate::news_charge::NewsCharge::new(
                news_batch_size,
            )),
            tabs: TabState::default(),
            prefix: std::sync::Mutex::new(crate::prefix::PrefixState::Disarmed),
            prefix_generation: std::sync::atomic::AtomicU64::new(0),
            last_arm_at: std::sync::Mutex::new(None),
            viewed_session: std::sync::atomic::AtomicUsize::new(0),
            session_advanced: tokio::sync::Notify::new(),
            followups_registered: std::sync::atomic::AtomicBool::new(false),
            slot_occupied: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for TabWire {
    /// Test-friendly default; production (`lib.rs`) passes the real
    /// configured batch size (`rss_max_per_poll` — a "full batch" is
    /// exactly what one poll cycle is allowed to land).
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::status::{FootballStatus, NewsStatus, StatusState};

    fn base_state() -> StatusState {
        StatusState {
            paused: false,
            waiting: 0,
            agent: crate::status::AgentStatus { active_sessions: 0 },
            football: FootballStatus {
                enabled: true,
                live: None,
            },
            news: NewsStatus {
                enabled: true,
                charge_fraction: 0.0,
                charge_count: 0,
                is_charged: false,
            },
        }
    }

    #[test]
    fn wire_labels_are_the_pinned_closed_set() {
        let labels: Vec<&str> = Tab::ORDER.iter().map(|t| t.wire_label()).collect();
        assert_eq!(labels, ["agent", "football", "news"]);
    }

    #[test]
    fn news_is_always_present_even_with_nothing_live() {
        assert_eq!(present_tabs(&base_state()), vec![Tab::News]);
    }

    #[test]
    fn agent_presence_follows_active_session_count() {
        let mut s = base_state();
        s.agent.active_sessions = 1;
        assert_eq!(present_tabs(&s), vec![Tab::Agent, Tab::News]);
    }

    #[test]
    fn presence_preserves_strip_order_when_everything_is_live() {
        let mut s = base_state();
        s.agent.active_sessions = 2;
        s.football.live = Some(crate::status::LiveMatchSummary {
            label: "A 1-0 B".to_string(),
            minute: "45'".to_string(),
        });
        assert_eq!(present_tabs(&s), vec![Tab::Agent, Tab::Football, Tab::News]);
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
        s.select(Tab::News);
        assert_eq!(s.selected(), Some(Tab::News));
    }

    #[test]
    fn selecting_the_same_tab_again_deselects_it() {
        let mut s = TabSelection::default();
        s.select(Tab::News);
        s.select(Tab::News);
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn selecting_a_different_tab_moves_the_selection_not_adds_to_it() {
        let mut s = TabSelection::default();
        s.select(Tab::Football);
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
        s.select(Tab::Football);
        s.clear_if_gone(|tab| tab != Tab::Football); // Football is absent
        assert_eq!(s.selected(), None);
    }

    #[test]
    fn clear_if_gone_leaves_the_selection_when_still_present() {
        let mut s = TabSelection::default();
        s.select(Tab::Football);
        s.clear_if_gone(|tab| tab == Tab::Football); // still present
        assert_eq!(s.selected(), Some(Tab::Football));
    }

    #[test]
    fn from_prefix_digit_maps_1_through_3_in_strip_order() {
        assert_eq!(Tab::from_prefix_digit(1), Some(Tab::Agent));
        assert_eq!(Tab::from_prefix_digit(2), Some(Tab::Football));
        assert_eq!(Tab::from_prefix_digit(3), Some(Tab::News));
    }

    #[test]
    fn from_prefix_digit_rejects_anything_out_of_1_to_3() {
        assert_eq!(Tab::from_prefix_digit(0), None);
        assert_eq!(Tab::from_prefix_digit(4), None);
        assert_eq!(Tab::from_prefix_digit(5), None);
        assert_eq!(Tab::from_prefix_digit(6), None);
    }

    #[test]
    fn order_matches_from_prefix_digit_index_for_index() {
        for (i, tab) in Tab::ORDER.iter().enumerate() {
            assert_eq!(Tab::from_prefix_digit((i + 1) as u8), Some(*tab));
        }
    }
}
