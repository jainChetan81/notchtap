use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::event::{Priority, SourceKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub default_ttl: u64,
    pub max_queued_per_tier: usize,
    pub detect_path: PathBuf,
    /// v5 master kill switch: launch with promotion paused (tray reads
    /// "Resume"). the tray toggle itself stays session-only.
    pub start_paused: bool,
    pub espn_enabled: bool,
    pub espn_leagues: Vec<String>,
    pub espn_poll_secs: u64,
    /// v6: was hardcoded `Priority::High` in `poller.rs`; now configurable
    /// per source (`docs/CONTEXT.md`'s Origin/Rotation Order glossary).
    pub espn_priority: Priority,
    /// v6: previously football silently reused `default_ttl` — now has its
    /// own rotation window like `rss_ttl_secs` already does for news.
    pub espn_ttl_secs: u64,
    /// plan 039: opt-in live-match card. default false — today's
    /// burst-of-one-shot-cards stays the default; when on, one live match
    /// collapses to a single updating card (Topic `espn:{league}:{match_id}`,
    /// `Recurring` while in play, `OneShot` full-time on the same Topic).
    pub espn_live_card: bool,
    /// default false — news is opt-in per machine; ambient sources must
    /// not default on top of the app's primary agent-notification purpose.
    pub rss_enabled: bool,
    /// Per-feed configuration uses TOML array tables:
    ///
    /// ```toml
    /// [[rss_feeds]]
    /// url = "https://feeds.feedburner.com/ndtvnews-top-stories"
    /// source = "NDTV"
    /// category = "politics"
    /// ```
    pub rss_feeds: Vec<RssFeedConfig>,
    pub rss_poll_secs: u64,
    /// v6: was hardcoded `Priority::Low` in `rss_poller.rs`.
    pub rss_priority: Priority,
    pub rss_ttl_secs: u64,
    pub rss_max_per_poll: usize,
    /// v6: the `/notify` fallback when a request omits its own `priority`
    /// (was the hardcoded `Priority::Medium` in `http.rs`). A request that
    /// sets `priority` explicitly still overrides this.
    pub manual_default_priority: Priority,
    /// v6.1: fallback priority for a `/notify` request that self-identifies
    /// as `source: "cmux"` (see `notchtap` CLI's `--source`/auto-detect) —
    /// same override rule as `manual_default_priority`. Defaults to `High`
    /// to match the documented cmux notification-command convention
    /// (`--priority high`), so a cmux push that omits `priority` resolves
    /// identically to before this field existed.
    pub cmux_priority: Priority,
    /// v6.1: rotation window for a cmux-originated push — previously
    /// indistinguishable from any other manual `/notify` caller, so it
    /// silently used `default_ttl`.
    pub cmux_ttl_secs: u64,
    /// v6/v6.1: same-tier promotion tie-break, checked before arrival
    /// order. Must be a permutation of all four `SourceKind` variants —
    /// enforced by `settings::validate`.
    pub rotation_order: Vec<SourceKind>,
    pub connectors: Connectors,
    pub appearance: Appearance,
}

/// `[connectors.*]` tables — non-secret on/off switches only; credentials
/// live in `secrets.toml` (v3 spec §4) so `config.toml` stays paste-safe.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Connectors {
    pub telegram: TelegramToggle,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramToggle {
    /// default off — v3 outbound is opt-in per machine
    pub enabled: bool,
}

/// `[appearance]` — overlay card styling. Serialized as its own table so
/// hand-edited `config.toml` can override one value without touching others.
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
pub struct RssFeedConfig {
    pub url: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

/// Bare-url ergonomics: a feed known only by url (tests, minimal
/// configs) gets no source/category metadata.
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
    // ARCHITECTURE.md §16 locks the three leagues
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
    8
}

fn default_espn_live_card() -> bool {
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

fn default_cmux_priority() -> Priority {
    Priority::High
}

fn default_cmux_ttl_secs() -> u64 {
    8
}

fn default_rotation_order() -> Vec<SourceKind> {
    // v6.1 review fix: Manual ranks ahead of Cmux — at default priorities
    // (Football/Cmux both High, Manual Medium, News Low) this never
    // actually breaks a tie, since Cmux and Manual don't share a tier
    // unless the user manually equalizes their priorities. Still, an
    // install already running both should see the pre-existing, more
    // established Manual path win any such tie by default, not the
    // newer Cmux origin.
    vec![
        SourceKind::Football,
        SourceKind::Manual,
        SourceKind::Cmux,
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
            rss_enabled: default_rss_enabled(),
            rss_feeds: default_rss_feeds(),
            rss_poll_secs: default_rss_poll_secs(),
            rss_priority: default_rss_priority(),
            rss_ttl_secs: default_rss_ttl_secs(),
            rss_max_per_poll: default_rss_max_per_poll(),
            manual_default_priority: default_manual_default_priority(),
            cmux_priority: default_cmux_priority(),
            cmux_ttl_secs: default_cmux_ttl_secs(),
            rotation_order: default_rotation_order(),
            connectors: Connectors::default(),
            appearance: default_appearance(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // spec §9 pins ~/.config/notchtap/config.toml. dirs::config_dir()
        // is wrong here: on macOS it resolves to ~/Library/Application Support.
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

    /// `~/.config/notchtap/` — the one directory config and secrets share
    /// (v5 settings write paths need it as a value, not a hardcode).
    pub fn dir_from_home(home: &std::path::Path) -> PathBuf {
        home.join(".config").join("notchtap")
    }

    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        let mut config: Config = toml::from_str(content)?;
        // v6.1 review fix: espn_ttl_secs/cmux_ttl_secs were split out of
        // the one shared default_ttl. serde's whole-struct #[serde(default)]
        // can't express "inherit sibling field X when absent" — only "use
        // Config::default()'s value" — so an install that had already
        // customized default_ttl before this split would otherwise see
        // football/cmux silently revert to the new fields' own hardcoded
        // default instead of keeping the value it actually configured.
        // Re-parsing as a raw table to see which keys the file itself set
        // (not what serde defaulted them to) lets us inherit the file's
        // effective default_ttl exactly where the old shared-field
        // behavior would have applied it.
        if let Ok(raw) = content.parse::<toml::Table>() {
            if !raw.contains_key("espn_ttl_secs") {
                config.espn_ttl_secs = config.default_ttl;
            }
            if !raw.contains_key("cmux_ttl_secs") {
                config.cmux_ttl_secs = config.default_ttl;
            }
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
        assert_eq!(c.espn_ttl_secs, 8);
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
        assert_eq!(c.manual_default_priority, Priority::Medium);
        assert_eq!(c.cmux_priority, Priority::High);
        assert_eq!(c.cmux_ttl_secs, 8);
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Cmux,
                SourceKind::News
            ]
        );
        assert_eq!(c.appearance.card_scale, 1.0);
        assert_eq!(c.appearance.card_radius, 16.0);
        assert_eq!(c.appearance.card_opacity, 0.9);
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
    fn telegram_connector_defaults_to_disabled() {
        // v3 exit criteria (IMPLEMENTATION_PLAN.md §3.1): outbound is
        // opt-in per machine — absent table means off.
        let c = Config::parse("").unwrap();
        assert!(!c.connectors.telegram.enabled);
    }

    #[test]
    fn telegram_connector_can_be_enabled() {
        let c = Config::parse("[connectors.telegram]\nenabled = true\n").unwrap();
        assert!(c.connectors.telegram.enabled);
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
        // v5 kill switch is opt-in: absent field means normal launch
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
            "espn_priority = \"medium\"\nespn_ttl_secs = 12\nrss_priority = \"high\"\nmanual_default_priority = \"low\"\ncmux_priority = \"low\"\ncmux_ttl_secs = 20\n",
        )
        .unwrap();
        assert_eq!(c.espn_priority, Priority::Medium);
        assert_eq!(c.espn_ttl_secs, 12);
        assert_eq!(c.rss_priority, Priority::High);
        assert_eq!(c.manual_default_priority, Priority::Low);
        assert_eq!(c.cmux_priority, Priority::Low);
        assert_eq!(c.cmux_ttl_secs, 20);
    }

    #[test]
    fn espn_and_cmux_ttl_inherit_a_customized_default_ttl_when_absent() {
        // v6.1 review fix: an install that already had `default_ttl = 20`
        // before espn_ttl_secs/cmux_ttl_secs existed must not silently
        // revert football/cmux to the new fields' own hardcoded default.
        let c = Config::parse("default_ttl = 20\n").unwrap();
        assert_eq!(c.default_ttl, 20);
        assert_eq!(c.espn_ttl_secs, 20);
        assert_eq!(c.cmux_ttl_secs, 20);
    }

    #[test]
    fn explicit_espn_or_cmux_ttl_is_not_overridden_by_inheritance() {
        let c = Config::parse("default_ttl = 20\nespn_ttl_secs = 5\ncmux_ttl_secs = 6\n").unwrap();
        assert_eq!(c.default_ttl, 20);
        assert_eq!(c.espn_ttl_secs, 5);
        assert_eq!(c.cmux_ttl_secs, 6);
    }

    #[test]
    fn absent_default_ttl_still_yields_the_shared_default_of_eight() {
        // no default_ttl in the file at all: default_ttl resolves to its
        // own default (8), and espn/cmux inherit that same resolved
        // value — identical to today's fresh-install behavior.
        let c = Config::parse("").unwrap();
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.espn_ttl_secs, 8);
        assert_eq!(c.cmux_ttl_secs, 8);
    }

    #[test]
    fn rotation_order_is_overridable() {
        let c = Config::parse("rotation_order = [\"news\", \"football\", \"cmux\", \"manual\"]\n")
            .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Cmux,
                SourceKind::Manual
            ]
        );
    }

    #[test]
    fn unknown_priority_or_source_kind_string_is_a_parse_error() {
        assert!(Config::parse("espn_priority = \"urgent\"").is_err());
        assert!(Config::parse("rotation_order = [\"telegram\"]").is_err());
    }
}
