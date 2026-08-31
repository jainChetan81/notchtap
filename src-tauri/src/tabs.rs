//! `click.rs`'s NSEvent monitor is the click path, `lib.rs` owns the live `Arc<TabState>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Agent,
    Football,
    News,
}

impl Tab {
    pub const ORDER: [Tab; 3] = [Tab::Agent, Tab::Football, Tab::News];

    /// `prefix+1`..`prefix+3` — `None` for anything outside that range, so the caller's "anything
    /// else: disarm.
    pub fn from_prefix_digit(digit: u8) -> Option<Tab> {
        match digit {
            1 => Some(Tab::Agent),
            2 => Some(Tab::Football),
            3 => Some(Tab::News),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabSelection {
    selected: Option<Tab>,
}

impl TabSelection {
    pub fn selected(&self) -> Option<Tab> {
        self.selected
    }

    /// A click on `tab`: the SAME tab clicked again deselects; a different tab moves the selection
    /// to it.
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

    /// Clears selection when the selected tab is no longer live.
    pub fn clear_if_gone(&mut self, is_present: impl FnOnce(Tab) -> bool) {
        if let Some(tab) = self.selected {
            if !is_present(tab) {
                self.selected = None;
            }
        }
    }
}

impl Tab {
    /// The wire token `tab-selection-changed` carries — the closed set: `"agent" | "football" |
    /// "news"`.
    pub fn wire_label(self) -> &'static str {
        match self {
            Tab::Agent => "agent",
            Tab::Football => "football",
            Tab::News => "news",
        }
    }
}

/// Which tabs are PRESENT given the current ambient state: news always, agent/football only while
/// genuinely live.
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

#[derive(Debug, Default)]
pub struct TabState {
    pub selection: std::sync::Mutex<TabSelection>,
    pub last_emitted: std::sync::Mutex<Option<Tab>>,
    pub presence: std::sync::Mutex<Vec<Tab>>,
}

#[derive(Debug)]
pub struct TabWire {
    pub agent_sessions: std::sync::atomic::AtomicUsize,
    pub news_charge: std::sync::Mutex<crate::news_charge::NewsCharge>,
    pub tabs: TabState,
    pub prefix: std::sync::Mutex<crate::prefix::PrefixState>,
    pub prefix_generation: std::sync::atomic::AtomicU64,
    /// `None` until the first arm; never cleared on disarm, because a stale arm instant reads as
    /// "long past its deadline" anyway, which is exactly the release verdict we want.
    pub last_arm_at: std::sync::Mutex<Option<std::time::Instant>>,
    pub viewed_session: std::sync::atomic::AtomicUsize,
    /// Fired whenever `viewed_session` changes for ANY reason (manual prefix-key cycling or the
    /// auto-advance timer below).
    pub session_advanced: tokio::sync::Notify,
    /// TRUE whenever the bare follow-up keys are currently grabbed system-wide.
    pub followups_registered: std::sync::atomic::AtomicBool,
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
    fn from_prefix_digit_rejects_anything_out_of_1_to_3() {
        assert_eq!(Tab::from_prefix_digit(0), None);
        assert_eq!(Tab::from_prefix_digit(4), None);
        assert_eq!(Tab::from_prefix_digit(5), None);
        assert_eq!(Tab::from_prefix_digit(6), None);
    }
}
