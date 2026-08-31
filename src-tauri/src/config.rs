use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::event::{Priority, SourceKind};
use crate::silence::Window;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub default_ttl: u64,
    pub max_queued_per_tier: usize,
    pub detect_path: PathBuf,
    pub start_paused: bool,
    pub espn_enabled: bool,
    pub espn_leagues: Vec<String>,
    pub espn_poll_secs: u64,
    pub espn_priority: Priority,
    pub espn_ttl_secs: u64,
    pub espn_live_card: bool,
    pub espn_rich_events: bool,
    /// default false — news is opt-in per machine; ambient sources must not default on top of the
    /// app's primary agent-notification purpose.
    pub rss_enabled: bool,
    pub rss_feeds: Vec<RssFeedConfig>,
    #[serde(default)]
    pub rss_topics: Vec<String>,
    pub rss_poll_secs: u64,
    pub rss_priority: Priority,
    pub rss_ttl_secs: u64,
    pub rss_max_per_poll: usize,
    pub manual_default_priority: Priority,
    pub agent_priority: Priority,
    pub agent_ttl_secs: u64,
    #[serde(default)]
    pub agents: AgentsConfig,
    /// Must be a permutation of all four `SourceKind` variants — enforced by `settings::validate`.
    #[serde(
        deserialize_with = "lenient_rotation_order",
        default = "default_rotation_order"
    )]
    pub rotation_order: Vec<SourceKind>,
    pub appearance: Appearance,
    #[serde(default = "default_resting_state")]
    pub resting_state: RestingState,
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,
    #[serde(default)]
    pub silence: SilenceConfig,
    #[serde(default = "default_prefix_shortcut")]
    pub prefix_shortcut: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestingState {
    Rail,
    Notch,
}

/// shared bounds for the `[appearance]` fields, so the save path (`settings::validate_appearance`)
/// and the load path (`Config::parse`'s self-heal, below) can never drift apart.
pub const CARD_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.8..=1.4;
pub const CARD_RADIUS_RANGE: std::ops::RangeInclusive<f64> = 0.0..=24.0;
pub const CARD_OPACITY_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default = "default_card_scale")]
    pub card_scale: f64,
    #[serde(default = "default_card_radius")]
    pub card_radius: f64,
    #[serde(default = "default_card_opacity")]
    pub card_opacity: f64,
}

impl Default for Appearance {
    fn default() -> Self {
        default_appearance()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub enabled: bool,
    pub terminal_retention_secs: u64,
    pub stale_after_secs: u64,
    pub stale_retention_secs: u64,
    pub informational_notifications: bool,
    pub completion_notifications: bool,
    pub board_show_working: bool,
    pub permission_priority: Priority,
    pub input_priority: Priority,
    pub failure_priority: Priority,
    pub completion_priority: Priority,
    pub runtimes: AgentRuntimesConfig,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            terminal_retention_secs: 60,
            stale_after_secs: 300,
            stale_retention_secs: 600,
            informational_notifications: false,
            completion_notifications: true,
            board_show_working: false,
            permission_priority: Priority::High,
            input_priority: Priority::High,
            failure_priority: Priority::High,
            completion_priority: Priority::Medium,
            runtimes: AgentRuntimesConfig::default(),
        }
    }
}

/// All four default to `true`: a runtime the user hasn't touched is never silently disabled.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentRuntimesConfig {
    pub claude_code: AgentRuntimeToggle,
    pub codex: AgentRuntimeToggle,
    pub kimi: AgentRuntimeToggle,
    pub opencode: AgentRuntimeToggle,
}

impl AgentRuntimesConfig {
    pub fn runtime_enabled(&self, runtime: crate::agents::model::AgentRuntime) -> bool {
        use crate::agents::model::AgentRuntime;
        match runtime {
            AgentRuntime::ClaudeCode => self.claude_code.enabled,
            AgentRuntime::Codex => self.codex.enabled,
            AgentRuntime::Kimi => self.kimi.enabled,
            AgentRuntime::OpenCode => self.opencode.enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentRuntimeToggle {
    pub enabled: bool,
}

impl Default for AgentRuntimeToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RssFeedConfig {
    pub url: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

impl From<&str> for RssFeedConfig {
    fn from(url: &str) -> Self {
        Self {
            url: url.to_string(),
            source: None,
            category: None,
        }
    }
}

fn default_port() -> u16 {
    9789
}

fn default_ttl() -> u64 {
    8
}

fn default_max_queued_per_tier() -> usize {
    50
}

fn default_detect_path() -> PathBuf {
    PathBuf::from("/usr/local/bin/notchtap-detect")
}

fn default_espn_enabled() -> bool {
    true
}

fn default_espn_leagues() -> Vec<String> {
    vec![
        "eng.1".to_string(),
        "uefa.champions".to_string(),
        "esp.1".to_string(),
    ]
}

fn default_espn_poll_secs() -> u64 {
    30
}

fn default_espn_priority() -> Priority {
    Priority::High
}

fn default_espn_ttl_secs() -> u64 {
    15
}

fn default_espn_live_card() -> bool {
    false
}

fn default_espn_rich_events() -> bool {
    false
}

fn default_rss_enabled() -> bool {
    false
}

fn default_rss_priority() -> Priority {
    Priority::Low
}

fn default_manual_default_priority() -> Priority {
    Priority::Medium
}

fn default_agent_priority() -> Priority {
    Priority::High
}

fn default_agent_ttl_secs() -> u64 {
    8
}

fn default_resting_state() -> RestingState {
    RestingState::Rail
}

fn default_prefix_shortcut() -> String {
    "⌃⇧Space".to_string()
}

fn default_history_enabled() -> bool {
    false
}

fn lenient_rotation_order<'de, D>(deserializer: D) -> Result<Vec<SourceKind>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let raw: Vec<String> = Vec::deserialize(deserializer)?;
    Ok(raw
        .iter()
        .filter_map(|s| {
            SourceKind::deserialize(serde::de::value::StrDeserializer::<D::Error>::new(s)).ok()
        })
        .collect())
}

fn default_rotation_order() -> Vec<SourceKind> {
    // Manual ranks ahead of Agent — at default priorities (Football/Agent both High, Manual
    // Medium, News Low) this never actually breaks a tie.
    vec![
        SourceKind::Football,
        SourceKind::Manual,
        SourceKind::Agent,
        SourceKind::News,
    ]
}

fn default_rss_feeds() -> Vec<RssFeedConfig> {
    vec![RssFeedConfig {
        url: "https://feeds.feedburner.com/ndtvnews-top-stories".to_string(),
        source: Some("NDTV".to_string()),
        category: None,
    }]
}

fn default_rss_poll_secs() -> u64 {
    60
}

fn default_rss_ttl_secs() -> u64 {
    10
}

fn default_rss_max_per_poll() -> usize {
    10
}

fn default_card_scale() -> f64 {
    1.0
}

fn default_card_radius() -> f64 {
    16.0
}

fn default_card_opacity() -> f64 {
    0.9
}

fn default_appearance() -> Appearance {
    Appearance {
        card_scale: default_card_scale(),
        card_radius: default_card_radius(),
        card_opacity: default_card_opacity(),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            default_ttl: default_ttl(),
            max_queued_per_tier: default_max_queued_per_tier(),
            detect_path: default_detect_path(),
            start_paused: false,
            espn_enabled: default_espn_enabled(),
            espn_leagues: default_espn_leagues(),
            espn_poll_secs: default_espn_poll_secs(),
            espn_priority: default_espn_priority(),
            espn_ttl_secs: default_espn_ttl_secs(),
            espn_live_card: default_espn_live_card(),
            espn_rich_events: default_espn_rich_events(),
            rss_enabled: default_rss_enabled(),
            rss_feeds: default_rss_feeds(),
            rss_topics: Vec::new(),
            rss_poll_secs: default_rss_poll_secs(),
            rss_priority: default_rss_priority(),
            rss_ttl_secs: default_rss_ttl_secs(),
            rss_max_per_poll: default_rss_max_per_poll(),
            manual_default_priority: default_manual_default_priority(),
            agent_priority: default_agent_priority(),
            agent_ttl_secs: default_agent_ttl_secs(),
            agents: AgentsConfig::default(),
            rotation_order: default_rotation_order(),
            appearance: default_appearance(),
            resting_state: default_resting_state(),
            history_enabled: default_history_enabled(),
            silence: SilenceConfig::default(),
            prefix_shortcut: default_prefix_shortcut(),
        }
    }
}

/// `enabled`/`window` feed `silence::SilenceController::new` at boot (`lib.rs`'s wiring); Skip and
/// Timed Mutes are session-only tray state, never persisted here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SilenceConfig {
    pub enabled: bool,
    pub window: Window,
}

impl Default for SilenceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window: Window::parse("00:00-10:00").expect("default silence window must parse"),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        let path = Self::dir_from_home(&home).join("config.toml");

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config at {:?}: {}", path, e))?;
        Self::parse(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse config at {:?}: {}", path, e))
    }

    /// `~/.config/notchtap/` — the one directory config and secrets share (settings write paths
    /// need it as a value, not a hardcode).
    pub fn dir_from_home(home: &std::path::Path) -> PathBuf {
        home.join(".config").join("notchtap")
    }

    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        let mut config: Config = toml::from_str(content)?;
        if let Ok(raw) = content.parse::<toml::Table>() {
            if !raw.contains_key("espn_ttl_secs") && raw.contains_key("default_ttl") {
                config.espn_ttl_secs = config.default_ttl;
            }
            if raw.contains_key("agent_ttl_secs") {
            } else if let Some(legacy) = raw
                .get("cmux_ttl_secs")
                .and_then(|v| v.clone().try_into::<u64>().ok())
            {
                config.agent_ttl_secs = legacy;
            } else {
                config.agent_ttl_secs = config.default_ttl;
            }
            if !raw.contains_key("agent_priority") {
                if let Some(legacy) = raw
                    .get("cmux_priority")
                    .and_then(|v| v.clone().try_into::<Priority>().ok())
                {
                    config.agent_priority = legacy;
                }
            }
        }
        let mut seen: Vec<SourceKind> = Vec::new();
        config.rotation_order.retain(|s| {
            if seen.contains(s) {
                false
            } else {
                seen.push(*s);
                true
            }
        });
        for source in default_rotation_order() {
            if !config.rotation_order.contains(&source) {
                config.rotation_order.push(source);
            }
        }
        if !config.appearance.card_scale.is_finite() {
            config.appearance.card_scale = default_card_scale();
        } else {
            config.appearance.card_scale = config
                .appearance
                .card_scale
                .clamp(*CARD_SCALE_RANGE.start(), *CARD_SCALE_RANGE.end());
        }
        if !config.appearance.card_radius.is_finite() {
            config.appearance.card_radius = default_card_radius();
        } else {
            config.appearance.card_radius = config
                .appearance
                .card_radius
                .clamp(*CARD_RADIUS_RANGE.start(), *CARD_RADIUS_RANGE.end());
        }
        if !config.appearance.card_opacity.is_finite() {
            config.appearance.card_opacity = default_card_opacity();
        } else {
            config.appearance.card_opacity = config
                .appearance
                .card_opacity
                .clamp(*CARD_OPACITY_RANGE.start(), *CARD_OPACITY_RANGE.end());
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toml_yields_all_defaults() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.port, 9789);
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.max_queued_per_tier, 50);
        assert_eq!(
            c.detect_path,
            PathBuf::from("/usr/local/bin/notchtap-detect")
        );
        assert!(c.espn_enabled);
        assert_eq!(c.espn_leagues, ["eng.1", "uefa.champions", "esp.1"]);
        assert_eq!(c.espn_poll_secs, 30);
        assert_eq!(c.espn_priority, Priority::High);
        assert_eq!(c.espn_ttl_secs, 15);
        assert!(!c.rss_enabled);
        assert_eq!(
            c.rss_feeds,
            [RssFeedConfig {
                url: "https://feeds.feedburner.com/ndtvnews-top-stories".to_string(),
                source: Some("NDTV".to_string()),
                category: None,
            }]
        );
        assert_eq!(c.rss_poll_secs, 60);
        assert_eq!(c.rss_priority, Priority::Low);
        assert_eq!(c.rss_ttl_secs, 10);
        assert_eq!(c.rss_max_per_poll, 10);
        assert!(c.rss_topics.is_empty());
        assert_eq!(c.manual_default_priority, Priority::Medium);
        assert_eq!(c.agent_priority, Priority::High);
        assert_eq!(c.agent_ttl_secs, 8);
        assert!(c.agents.enabled);
        assert_eq!(c.agents.terminal_retention_secs, 60);
        assert_eq!(c.agents.stale_after_secs, 300);
        assert_eq!(c.agents.stale_retention_secs, 600);
        assert!(!c.agents.informational_notifications);
        assert!(c.agents.completion_notifications);
        assert!(!c.agents.board_show_working);
        assert_eq!(c.agents.permission_priority, Priority::High);
        assert_eq!(c.agents.input_priority, Priority::High);
        assert_eq!(c.agents.failure_priority, Priority::High);
        assert_eq!(c.agents.completion_priority, Priority::Medium);
        assert!(c.agents.runtimes.claude_code.enabled);
        assert!(c.agents.runtimes.codex.enabled);
        assert!(c.agents.runtimes.kimi.enabled);
        assert!(c.agents.runtimes.opencode.enabled);
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Agent,
                SourceKind::News
            ]
        );
        assert_eq!(c.appearance.card_scale, 1.0);
        assert_eq!(c.appearance.card_radius, 16.0);
        assert_eq!(c.appearance.card_opacity, 0.9);
        assert_eq!(c.resting_state, RestingState::Rail);
        assert!(!c.espn_rich_events);
        assert!(!c.history_enabled);
        assert_eq!(c.prefix_shortcut, "⌃⇧Space");
    }

    #[test]
    fn prefix_shortcut_round_trips_through_parse() {
        let c = Config::parse("prefix_shortcut = \"⌃⇧X\"\n").unwrap();
        assert_eq!(c.prefix_shortcut, "⌃⇧X");
    }

    #[test]
    fn prefix_shortcut_absent_from_file_falls_back_to_the_shipped_default() {
        let c = Config::parse("port = 4321\n").unwrap();
        assert_eq!(c.prefix_shortcut, "⌃⇧Space");
        assert_eq!(c.port, 4321);
    }

    #[test]
    fn completion_notifications_defaults_to_true_for_a_config_predating_the_key() {
        let legacy = Config::parse(
            "[agents]\nenabled = true\ninformational_notifications = false\ncompletion_priority = \"low\"\n",
        )
        .unwrap();
        assert!(legacy.agents.completion_notifications);
        assert_eq!(legacy.agents.completion_priority, Priority::Low);

        let off = Config::parse("[agents]\ncompletion_notifications = false\n").unwrap();
        assert!(!off.agents.completion_notifications);
    }

    #[test]
    fn board_show_working_defaults_to_false_including_for_a_config_predating_the_key() {
        let legacy = Config::parse(
            "[agents]\nenabled = true\nstale_after_secs = 120\ncompletion_priority = \"low\"\n",
        )
        .unwrap();
        assert!(!legacy.agents.board_show_working);
        assert_eq!(legacy.agents.stale_after_secs, 120);

        let on = Config::parse("[agents]\nboard_show_working = true\n").unwrap();
        assert!(on.agents.board_show_working);
    }

    #[test]
    fn espn_rich_events_defaults_to_false_and_is_overridable() {
        let default = Config::parse("").unwrap();
        assert!(!default.espn_rich_events);

        let on = Config::parse("espn_rich_events = true\n").unwrap();
        assert!(on.espn_rich_events);
    }

    #[test]
    fn resting_state_defaults_to_rail_and_is_overridable() {
        let healed = Config::parse("").unwrap();
        assert_eq!(healed.resting_state, RestingState::Rail);

        let notch = Config::parse("resting_state = \"notch\"\n").unwrap();
        assert_eq!(notch.resting_state, RestingState::Notch);

        let rail = Config::parse("resting_state = \"rail\"\n").unwrap();
        assert_eq!(rail.resting_state, RestingState::Rail);
    }

    #[test]
    fn history_enabled_defaults_to_false_and_is_overridable() {
        let healed = Config::parse("").unwrap();
        assert!(!healed.history_enabled);

        let on = Config::parse("history_enabled = true\n").unwrap();
        assert!(on.history_enabled);
    }

    #[test]
    fn espn_fields_are_overridable() {
        let c = Config::parse("espn_enabled = false\nespn_leagues = [\"usa.1\"]\n").unwrap();
        assert!(!c.espn_enabled);
        assert_eq!(c.espn_leagues, ["usa.1"]);
        assert_eq!(c.espn_poll_secs, 30);
    }

    #[test]
    fn rss_fields_are_overridable() {
        let c = Config::parse(
            "rss_enabled = true\nrss_poll_secs = 120\n\n[[rss_feeds]]\nurl = \"https://example.com/feed\"\nsource = \"Example News\"\ncategory = \"world\"\n",
        )
        .unwrap();
        assert!(c.rss_enabled);
        assert_eq!(
            c.rss_feeds,
            [RssFeedConfig {
                url: "https://example.com/feed".to_string(),
                source: Some("Example News".to_string()),
                category: Some("world".to_string()),
            }]
        );
        assert_eq!(c.rss_poll_secs, 120);
        assert_eq!(c.rss_ttl_secs, 10);
        assert_eq!(c.rss_max_per_poll, 10);
    }

    #[test]
    fn rss_topics_default_empty_and_overridable() {
        let default = Config::parse("").unwrap();
        assert!(default.rss_topics.is_empty());

        let c = Config::parse("rss_topics = [\"aston villa transfers\", \"formula 1\"]\n").unwrap();
        assert_eq!(
            c.rss_topics,
            ["aston villa transfers".to_string(), "formula 1".to_string()]
        );
    }

    #[test]
    fn rss_feed_tables_parse_with_and_without_optional_keys() {
        let c = Config::parse(
            r#"
[[rss_feeds]]
url = "https://example.com/with-meta"
source = "Example"
category = "tech"

[[rss_feeds]]
url = "https://example.com/without-meta"
"#,
        )
        .unwrap();

        assert_eq!(
            c.rss_feeds,
            [
                RssFeedConfig {
                    url: "https://example.com/with-meta".to_string(),
                    source: Some("Example".to_string()),
                    category: Some("tech".to_string()),
                },
                RssFeedConfig {
                    url: "https://example.com/without-meta".to_string(),
                    source: None,
                    category: None,
                },
            ]
        );
    }

    #[test]
    fn partial_toml_keeps_defaults_for_missing_fields() {
        let c = Config::parse("port = 1234\n").unwrap();
        assert_eq!(c.port, 1234);
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.max_queued_per_tier, 50);
    }

    #[test]
    fn unknown_top_level_table_is_ignored_not_a_parse_error() {
        let c = Config::parse("port = 1234\n[unknown_section.nested]\nenabled = true\n").unwrap();
        assert_eq!(c.port, 1234);
    }

    #[test]
    fn appearance_fields_use_toml_table_and_partial_defaults() {
        let full = Config::parse(
            "[appearance]\ncard_scale = 1.2\ncard_radius = 12.0\ncard_opacity = 0.75\n",
        )
        .unwrap();
        assert_eq!(full.appearance.card_scale, 1.2);
        assert_eq!(full.appearance.card_radius, 12.0);
        assert_eq!(full.appearance.card_opacity, 0.75);

        let partial = Config::parse("[appearance]\ncard_scale = 0.9\n").unwrap();
        assert_eq!(partial.appearance.card_scale, 0.9);
        assert_eq!(partial.appearance.card_radius, 16.0);
        assert_eq!(partial.appearance.card_opacity, 0.9);
    }

    #[test]
    fn start_paused_defaults_to_false() {
        let c = Config::parse("").unwrap();
        assert!(!c.start_paused);
    }

    #[test]
    fn start_paused_is_overridable() {
        let c = Config::parse("start_paused = true\n").unwrap();
        assert!(c.start_paused);
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(Config::parse("port = \"not a number\"").is_err());
    }

    #[test]
    fn per_source_priority_and_ttl_are_overridable() {
        let c = Config::parse(
            "espn_priority = \"medium\"\nespn_ttl_secs = 12\nrss_priority = \"high\"\nmanual_default_priority = \"low\"\nagent_priority = \"low\"\nagent_ttl_secs = 20\n",
        )
        .unwrap();
        assert_eq!(c.espn_priority, Priority::Medium);
        assert_eq!(c.espn_ttl_secs, 12);
        assert_eq!(c.rss_priority, Priority::High);
        assert_eq!(c.manual_default_priority, Priority::Low);
        assert_eq!(c.agent_priority, Priority::Low);
        assert_eq!(c.agent_ttl_secs, 20);
    }

    #[test]
    fn espn_and_agent_ttl_inherit_a_customized_default_ttl_when_absent() {
        let c = Config::parse("default_ttl = 20\n").unwrap();
        assert_eq!(c.default_ttl, 20);
        assert_eq!(c.espn_ttl_secs, 20);
        assert_eq!(c.agent_ttl_secs, 20);
    }

    #[test]
    fn explicit_espn_or_agent_ttl_is_not_overridden_by_inheritance() {
        let c = Config::parse("default_ttl = 20\nespn_ttl_secs = 5\nagent_ttl_secs = 6\n").unwrap();
        assert_eq!(c.default_ttl, 20);
        assert_eq!(c.espn_ttl_secs, 5);
        assert_eq!(c.agent_ttl_secs, 6);
    }

    #[test]
    fn absent_default_ttl_still_yields_the_shared_default_of_eight() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.espn_ttl_secs, 15);
        assert_eq!(c.agent_ttl_secs, 8);
    }

    #[test]
    fn legacy_cmux_priority_and_ttl_alias_to_agent_fields_when_new_keys_absent() {
        let c = Config::parse("cmux_priority = \"low\"\ncmux_ttl_secs = 20\n").unwrap();
        assert_eq!(c.agent_priority, Priority::Low);
        assert_eq!(c.agent_ttl_secs, 20);
    }

    #[test]
    fn new_agent_keys_win_over_legacy_cmux_keys_when_both_present() {
        let c = Config::parse(
            "cmux_priority = \"low\"\nagent_priority = \"high\"\ncmux_ttl_secs = 5\nagent_ttl_secs = 30\n",
        )
        .unwrap();
        assert_eq!(c.agent_priority, Priority::High);
        assert_eq!(c.agent_ttl_secs, 30);
    }

    #[test]
    fn legacy_cmux_ttl_alias_is_not_overridden_by_default_ttl_inheritance() {
        let c = Config::parse("default_ttl = 99\ncmux_ttl_secs = 6\n").unwrap();
        assert_eq!(c.default_ttl, 99);
        assert_eq!(c.agent_ttl_secs, 6);
    }

    #[test]
    fn full_legacy_config_migrates_and_reserializes_with_new_names_only() {
        let legacy = "cmux_priority = \"low\"\ncmux_ttl_secs = 42\nrotation_order = [\"football\", \"manual\", \"weather\", \"cmux\", \"news\"]\n";
        let c = Config::parse(legacy).unwrap();
        assert_eq!(c.agent_priority, Priority::Low);
        assert_eq!(c.agent_ttl_secs, 42);
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Agent,
                SourceKind::News,
            ]
        );

        let reserialized = toml::to_string_pretty(&c).unwrap();
        assert!(!reserialized.contains("cmux"), "{reserialized}");
        assert!(reserialized.contains("agent_priority"));
        assert!(reserialized.contains("agent_ttl_secs"));

        let reparsed = Config::parse(&reserialized).unwrap();
        assert_eq!(reparsed, c);
        let reserialized_again = toml::to_string_pretty(&reparsed).unwrap();
        assert_eq!(reserialized_again, reserialized);
    }

    #[test]
    fn double_migration_is_a_no_op() {
        let legacy = "cmux_priority = \"high\"\ncmux_ttl_secs = 11\n";
        let once = Config::parse(legacy).unwrap();
        let reserialized = toml::to_string_pretty(&once).unwrap();
        let twice = Config::parse(&reserialized).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn espn_ttl_defaults_to_15_when_default_ttl_untouched() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.espn_ttl_secs, 15);
        assert_eq!(c.default_ttl, default_ttl());

        let c = Config::parse("default_ttl = 30\n").unwrap();
        assert_eq!(c.espn_ttl_secs, 30);
    }

    #[test]
    fn rotation_order_is_overridable() {
        let c = Config::parse("rotation_order = [\"news\", \"football\", \"agent\", \"manual\"]\n")
            .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Agent,
                SourceKind::Manual,
            ]
        );
    }

    #[test]
    fn legacy_cmux_rotation_entry_is_rewritten_in_place_to_agent() {
        let c = Config::parse("rotation_order = [\"news\", \"football\", \"cmux\", \"manual\"]\n")
            .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Agent,
                SourceKind::Manual,
            ]
        );
    }

    #[test]
    fn rotation_order_missing_a_source_is_healed_by_appending_it() {
        let c = Config::parse("rotation_order = [\"news\", \"football\", \"manual\"]\n").unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Agent,
            ]
        );
    }

    #[test]
    fn rotation_order_with_duplicate_and_missing_sources_heals_to_exactly_four() {
        let c =
            Config::parse("rotation_order = [\"football\", \"football\", \"manual\"]\n").unwrap();
        assert_eq!(c.rotation_order.len(), 4);
        let mut sorted = c.rotation_order.clone();
        sorted.sort_by_key(|s| format!("{s:?}"));
        let mut expected = vec![
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::News,
            SourceKind::Agent,
        ];
        expected.sort_by_key(|s| format!("{s:?}"));
        assert_eq!(sorted, expected);
        assert_eq!(c.rotation_order[0], SourceKind::Football);
        assert_eq!(c.rotation_order[1], SourceKind::Manual);
    }

    #[test]
    fn rotation_order_containing_removed_weather_origin_boots_instead_of_crashing() {
        let c = Config::parse(
            "rotation_order = [\"football\", \"manual\", \"weather\", \"agent\", \"news\"]\n",
        )
        .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Agent,
                SourceKind::News,
            ]
        );
        assert!(crate::settings::validate(&c).is_ok());
    }

    #[test]
    fn rotation_order_of_entirely_unknown_names_heals_to_the_full_default() {
        let c = Config::parse("rotation_order = [\"weather\", \"pigeon\"]\n").unwrap();
        assert_eq!(c.rotation_order, default_rotation_order());
    }

    #[test]
    fn unknown_priority_string_is_a_parse_error() {
        assert!(Config::parse("espn_priority = \"urgent\"").is_err());
    }

    #[test]
    fn appearance_out_of_range_is_clamped_on_load() {
        let c = Config::parse(
            "[appearance]\ncard_scale = 0.0\ncard_radius = 99.0\ncard_opacity = 2.0\n",
        )
        .unwrap();
        assert_eq!(c.appearance.card_scale, *CARD_SCALE_RANGE.start());
        assert_eq!(c.appearance.card_radius, *CARD_RADIUS_RANGE.end());
        assert_eq!(c.appearance.card_opacity, *CARD_OPACITY_RANGE.end());
    }

    #[test]
    fn appearance_non_finite_falls_back_to_defaults() {
        let c = Config::parse("[appearance]\ncard_scale = nan\n").unwrap();
        assert_eq!(c.appearance.card_scale, Appearance::default().card_scale);
    }

    #[test]
    fn appearance_in_range_values_pass_through_untouched() {
        let c = Config::parse(
            "[appearance]\ncard_scale = 1.1\ncard_radius = 12.0\ncard_opacity = 0.75\n",
        )
        .unwrap();
        assert_eq!(c.appearance.card_scale, 1.1);
        assert_eq!(c.appearance.card_radius, 12.0);
        assert_eq!(c.appearance.card_opacity, 0.75);
    }

    #[test]
    fn default_silence_window_parses() {
        assert!(crate::silence::Window::parse("00:00-10:00").is_ok());
    }

    #[test]
    fn silence_defaults_to_enabled_with_the_overnight_window() {
        let c = Config::parse("").unwrap();
        assert!(c.silence.enabled);
        assert_eq!(
            c.silence.window,
            crate::silence::Window::parse("00:00-10:00").unwrap()
        );
    }

    #[test]
    fn silence_window_is_overridable() {
        let c = Config::parse("[silence]\nwindow = \"23:00-07:30\"\n").unwrap();
        assert_eq!(
            c.silence.window,
            crate::silence::Window::parse("23:00-07:30").unwrap()
        );
        assert!(c.silence.enabled);
    }

    #[test]
    fn silence_can_be_disabled() {
        let c = Config::parse("[silence]\nenabled = false\n").unwrap();
        assert!(!c.silence.enabled);
        assert_eq!(
            c.silence.window,
            crate::silence::Window::parse("00:00-10:00").unwrap()
        );
    }

    #[test]
    fn malformed_silence_window_is_a_parse_error() {
        assert!(Config::parse("[silence]\nwindow = \"garbage\"\n").is_err());
        assert!(Config::parse("[silence]\nwindow = \"25:00-10:00\"\n").is_err());
        assert!(Config::parse("[silence]\nwindow = \"10:00-10:00\"\n").is_err());
    }
}
