use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub default_ttl: u64,
    pub max_queued_per_tier: usize,
    pub detect_path: PathBuf,
    pub espn_enabled: bool,
    pub espn_leagues: Vec<String>,
    pub espn_poll_secs: u64,
    /// default false — news is opt-in per machine; ambient sources must
    /// not default on top of the app's primary agent-notification purpose.
    pub rss_enabled: bool,
    pub rss_feeds: Vec<String>,
    pub rss_poll_secs: u64,
    pub rss_ttl_secs: u64,
    pub rss_max_per_poll: usize,
    pub connectors: Connectors,
}

/// `[connectors.*]` tables — non-secret on/off switches only; credentials
/// live in `secrets.toml` (v3 spec §4) so `config.toml` stays paste-safe.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Connectors {
    pub telegram: TelegramToggle,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct TelegramToggle {
    /// default off — v3 outbound is opt-in per machine
    pub enabled: bool,
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

fn default_rss_enabled() -> bool {
    false
}

fn default_rss_feeds() -> Vec<String> {
    vec!["https://feeds.feedburner.com/ndtvnews-top-stories".to_string()]
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

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            default_ttl: default_ttl(),
            max_queued_per_tier: default_max_queued_per_tier(),
            detect_path: default_detect_path(),
            espn_enabled: default_espn_enabled(),
            espn_leagues: default_espn_leagues(),
            espn_poll_secs: default_espn_poll_secs(),
            rss_enabled: default_rss_enabled(),
            rss_feeds: default_rss_feeds(),
            rss_poll_secs: default_rss_poll_secs(),
            rss_ttl_secs: default_rss_ttl_secs(),
            rss_max_per_poll: default_rss_max_per_poll(),
            connectors: Connectors::default(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // spec §9 pins ~/.config/notchtap/config.toml. dirs::config_dir()
        // is wrong here: on macOS it resolves to ~/Library/Application Support.
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        let path = home.join(".config").join("notchtap").join("config.toml");

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config at {:?}: {}", path, e))?;
        Self::parse(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse config at {:?}: {}", path, e))
    }

    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
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
        assert!(!c.rss_enabled);
        assert_eq!(
            c.rss_feeds,
            ["https://feeds.feedburner.com/ndtvnews-top-stories"]
        );
        assert_eq!(c.rss_poll_secs, 60);
        assert_eq!(c.rss_ttl_secs, 10);
        assert_eq!(c.rss_max_per_poll, 10);
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
            "rss_enabled = true\nrss_feeds = [\"https://example.com/feed\"]\nrss_poll_secs = 120\n",
        )
        .unwrap();
        assert!(c.rss_enabled);
        assert_eq!(c.rss_feeds, ["https://example.com/feed"]);
        assert_eq!(c.rss_poll_secs, 120);
        assert_eq!(c.rss_ttl_secs, 10);
        assert_eq!(c.rss_max_per_poll, 10);
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
    fn malformed_toml_is_an_error() {
        assert!(Config::parse("port = \"not a number\"").is_err());
    }
}
