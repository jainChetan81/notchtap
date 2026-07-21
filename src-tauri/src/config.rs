use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::event::{Priority, SourceKind, Units};

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
    /// plan 083 workstream c: opt-in richer match events (foul, offside,
    /// VAR check, substitution) via ESPN's `summary`/`plays` endpoints —
    /// default false, mirroring `espn_live_card`'s opt-in-gated pattern
    /// exactly. This is materially more per-match polling than the
    /// scoreboard feed alone, so it must stay opt-in.
    pub espn_rich_events: bool,
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
    /// plan 040 Part B: weather source (Open-Meteo, keyless). default
    /// false — ambient sources are opt-in per machine, same rule as rss.
    pub weather_enabled: bool,
    /// Raw coordinates the operator sets once — no geocoding, no
    /// city-name lookup, no second API dependency.
    pub weather_lat: f64,
    pub weather_lon: f64,
    /// Display units only: Open-Meteo converts server-side via its
    /// `temperature_unit` query param; alert thresholds below are always
    /// stored/compared in Celsius regardless of this field.
    pub weather_units: Units,
    pub weather_poll_secs: u64,
    pub weather_rain_threshold_pct: u8,
    pub weather_rain_lookahead_mins: u16,
    pub weather_temp_hot_c: f64,
    pub weather_temp_cold_c: f64,
    /// Medium by default: bracketed by espn (High — live sports is
    /// urgent) and rss (Low — news is ambient).
    pub weather_priority: Priority,
    /// v6/v6.1: same-tier promotion tie-break, checked before arrival
    /// order. Must be a permutation of all five `SourceKind` variants —
    /// enforced by `settings::validate`.
    pub rotation_order: Vec<SourceKind>,
    pub connectors: Connectors,
    pub appearance: Appearance,
    /// plan 085: the overlay's RESTING (idle) render choice — the cheap
    /// half of plan 079 item 17. `Rail` (default) is today's time+dots
    /// idle rail, zero behavior change. `Notch` renders nothing while
    /// idle (the bare native notch) — a render choice only, no hover
    /// detection; every `showing` path (promotions, rotation, expand,
    /// TTL) is unaffected either way.
    #[serde(default = "default_resting_state")]
    pub resting_state: RestingState,
    /// plan 088 (from plan 059's operator decision): persist accepted
    /// one-shot notifications to `~/.config/notchtap/history.jsonl` for
    /// later browsing. Defaults to `false` like every other opt-in surface
    /// here (`rss_enabled`, `weather_enabled`, `espn_live_card`,
    /// `espn_rich_events`) — this one writes notification CONTENT to disk,
    /// including cmux payloads, so off-by-default is load-bearing, not
    /// stylistic.
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,
    /// plan 104: user feature toggle for the ambient now-playing peek row.
    /// Default `false` — same opt-in convention as `weather_enabled`/
    /// `rss_enabled`: ambient sources never default on top of the app's
    /// primary agent-notification purpose. The panel-editable half of the
    /// two-gate design (`docs/design/now-playing-adapter.md`'s GO
    /// conditions) — the child process spawns only when this AND
    /// `now_playing_adapter_enabled` are both true.
    #[serde(default = "default_now_playing_enabled")]
    pub now_playing_enabled: bool,
    /// plan 104: the kill switch, deliberately separate from the feature
    /// toggle above and deliberately NOT exposed in the settings UI
    /// (`docs/design/now-playing-adapter.md` §7 risk #1: if Apple closes
    /// the `com.apple.*` MediaRemote oversight this adapter relies on, the
    /// failure degrades silently to "no data," indistinguishable from
    /// "nothing playing" — this flag is the config-file-only escape hatch
    /// so an operator can mute a dead feature without losing every other
    /// setting or waiting for a rebuild). Default `true`: once a user
    /// opts into `now_playing_enabled`, the adapter runs unless someone
    /// has explicitly muted it here by hand.
    #[serde(default = "default_now_playing_adapter_enabled")]
    pub now_playing_adapter_enabled: bool,
    /// plan 104: mirrors `detect_path`'s runtime-path convention exactly —
    /// built/installed out of band (`justfile`'s `build-media-adapter`
    /// recipe), never by the rust core itself. Expected to contain
    /// `bin/mediaremote-adapter.pl` + `MediaRemoteAdapter.framework`. Like
    /// `detect_path`, this is pinned server-side and never editable via
    /// the settings panel (`settings::pin_uneditable_fields`) — it names
    /// an executed subprocess path, not a display preference.
    #[serde(default = "default_now_playing_adapter_dir")]
    pub now_playing_adapter_dir: PathBuf,
}

/// See [`Config::resting_state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestingState {
    Rail,
    Notch,
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

/// plan 097: shared bounds for the `[appearance]` fields, so the save path
/// (`settings::validate_appearance`) and the load path (`Config::parse`'s
/// self-heal, below) can never drift apart.
pub const CARD_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.8..=1.4;
pub const CARD_RADIUS_RANGE: std::ops::RangeInclusive<f64> = 0.0..=24.0;
pub const CARD_OPACITY_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.0;

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
    // operator decision 2026-07-21: a scoreline needs longer on screen
    // than a generic alert; still configurable via espn_ttl_secs.
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

fn default_cmux_priority() -> Priority {
    Priority::High
}

fn default_cmux_ttl_secs() -> u64 {
    8
}

fn default_weather_enabled() -> bool {
    false
}

fn default_weather_lat() -> f64 {
    0.0
}

fn default_weather_lon() -> f64 {
    0.0
}

fn default_weather_units() -> Units {
    Units::Celsius
}

fn default_weather_poll_secs() -> u64 {
    900
}

fn default_weather_rain_threshold_pct() -> u8 {
    60
}

fn default_weather_rain_lookahead_mins() -> u16 {
    30
}

fn default_weather_temp_hot_c() -> f64 {
    36.0
}

fn default_weather_temp_cold_c() -> f64 {
    14.0
}

fn default_weather_priority() -> Priority {
    Priority::Medium
}

fn default_resting_state() -> RestingState {
    RestingState::Rail
}

fn default_history_enabled() -> bool {
    false
}

fn default_now_playing_enabled() -> bool {
    false
}

fn default_now_playing_adapter_enabled() -> bool {
    true
}

/// plan 104 revision (reviewer 2026-07-22): the original
/// `/usr/local/lib/notchtap/mediaremote-adapter` default requires
/// root-owned `/usr/local/lib` on a stock macOS install — verified live
/// on this exact machine (`mkdir -p` there fails with `Permission
/// denied`, no sudo), which means an operator's very first
/// `just build-media-adapter` run would fail before ever reaching the
/// adapter itself. `~/Library/Application Support/notchtap/` is the
/// macOS-conventional, user-writable location, resolved the same way
/// `Config::load`/`settings::notchtap_config_dir` already resolve home
/// (`dirs::home_dir()`, not a raw env lookup — this repo's one
/// home-resolution idiom, mirrored here rather than reimplemented).
/// Falls back to the old `/usr/local/lib` path only if home can't be
/// determined at all, so this default (like every other `default_*` fn
/// in this file) stays infallible and never empty.
fn default_now_playing_adapter_dir() -> PathBuf {
    dirs::home_dir()
        .map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("notchtap")
                .join("mediaremote-adapter")
        })
        .unwrap_or_else(|| PathBuf::from("/usr/local/lib/notchtap/mediaremote-adapter"))
}

fn default_rotation_order() -> Vec<SourceKind> {
    // v6.1 review fix: Manual ranks ahead of Cmux — at default priorities
    // (Football/Cmux both High, Manual Medium, News Low) this never
    // actually breaks a tie, since Cmux and Manual don't share a tier
    // unless the user manually equalizes their priorities. Still, an
    // install already running both should see the pre-existing, more
    // established Manual path win any such tie by default, not the
    // newer Cmux origin.
    // plan 040 Part B: Weather sits right after Manual — it shares
    // Manual's Medium tier by default, and the more established Manual
    // path wins that tie.
    vec![
        SourceKind::Football,
        SourceKind::Manual,
        SourceKind::Weather,
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
            espn_rich_events: default_espn_rich_events(),
            rss_enabled: default_rss_enabled(),
            rss_feeds: default_rss_feeds(),
            rss_poll_secs: default_rss_poll_secs(),
            rss_priority: default_rss_priority(),
            rss_ttl_secs: default_rss_ttl_secs(),
            rss_max_per_poll: default_rss_max_per_poll(),
            manual_default_priority: default_manual_default_priority(),
            cmux_priority: default_cmux_priority(),
            cmux_ttl_secs: default_cmux_ttl_secs(),
            weather_enabled: default_weather_enabled(),
            weather_lat: default_weather_lat(),
            weather_lon: default_weather_lon(),
            weather_units: default_weather_units(),
            weather_poll_secs: default_weather_poll_secs(),
            weather_rain_threshold_pct: default_weather_rain_threshold_pct(),
            weather_rain_lookahead_mins: default_weather_rain_lookahead_mins(),
            weather_temp_hot_c: default_weather_temp_hot_c(),
            weather_temp_cold_c: default_weather_temp_cold_c(),
            weather_priority: default_weather_priority(),
            rotation_order: default_rotation_order(),
            connectors: Connectors::default(),
            appearance: default_appearance(),
            resting_state: default_resting_state(),
            history_enabled: default_history_enabled(),
            now_playing_enabled: default_now_playing_enabled(),
            now_playing_adapter_enabled: default_now_playing_adapter_enabled(),
            now_playing_adapter_dir: default_now_playing_adapter_dir(),
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
        //
        // plan 101: the espn arm is conditional on the file ALSO having
        // customized default_ttl — the inherit exists only for configs
        // that customized the old shared default_ttl; a config that never
        // touched it gets espn's own default (15) instead of silently
        // re-inheriting the generic default. The cmux arm stays
        // unconditional (its default intentionally tracks default_ttl).
        if let Ok(raw) = content.parse::<toml::Table>() {
            if !raw.contains_key("espn_ttl_secs") && raw.contains_key("default_ttl") {
                config.espn_ttl_secs = config.default_ttl;
            }
            if !raw.contains_key("cmux_ttl_secs") {
                config.cmux_ttl_secs = config.default_ttl;
            }
        }
        // heal a `rotation_order` written before a `SourceKind` variant
        // existed (e.g. an install from before plan 040 Part B added
        // `weather`): `settings::validate` requires a permutation of all
        // five variants, but the settings UI's rotation-order list is a
        // fixed reorder-only widget (it just renders whatever's already in
        // the array) with no way for the user to add a missing one back —
        // so a stale array fails validation on every save, permanently,
        // with no in-UI escape hatch short of "Reset to defaults" (which
        // also discards every other customized setting). Dedupe any
        // pre-existing duplicate entries first (keeping the first
        // occurrence) — a duplicate-plus-missing array would otherwise
        // grow past five elements and fail the same permutation check.
        // Append whatever's missing, preserving the file's existing
        // relative order for everything it already had; this self-heals
        // for any future newly-added source too, not just this one.
        // dedupe first (keep first occurrence — a malformed hand-edited config
        // might repeat a source; without this, a duplicate-plus-missing array
        // would grow past 5 elements and fail `validate`'s permutation check
        // forever, the same lockout this heal exists to prevent). `SourceKind`
        // doesn't derive `Hash`, so this tracks "seen" sources in a small `Vec`
        // rather than a `HashSet` — negligible at this size.
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
        // plan 097: the load-path twin of `settings::validate_appearance`,
        // which only guards the settings-save path. A hand-edited
        // `config.toml` (e.g. `card_scale = 0.0`) would otherwise boot
        // unclamped, silently producing a degenerate hover rect plus
        // broken card rendering. Non-finite values (NaN, +/-inf — can't
        // occur through the settings UI, but a hand-edited TOML can
        // express `nan`/`inf`) fall back to the field's own default rather
        // than clamping, since clamping a NaN is a no-op in IEEE 754 and
        // would silently let it through.
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
        assert_eq!(c.manual_default_priority, Priority::Medium);
        assert_eq!(c.cmux_priority, Priority::High);
        assert_eq!(c.cmux_ttl_secs, 8);
        assert!(!c.weather_enabled);
        assert_eq!(c.weather_lat, 0.0);
        assert_eq!(c.weather_lon, 0.0);
        assert_eq!(c.weather_units, Units::Celsius);
        assert_eq!(c.weather_poll_secs, 900);
        assert_eq!(c.weather_rain_threshold_pct, 60);
        assert_eq!(c.weather_rain_lookahead_mins, 30);
        assert_eq!(c.weather_temp_hot_c, 36.0);
        assert_eq!(c.weather_temp_cold_c, 14.0);
        assert_eq!(c.weather_priority, Priority::Medium);
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::Football,
                SourceKind::Manual,
                SourceKind::Weather,
                SourceKind::Cmux,
                SourceKind::News
            ]
        );
        assert_eq!(c.appearance.card_scale, 1.0);
        assert_eq!(c.appearance.card_radius, 16.0);
        assert_eq!(c.appearance.card_opacity, 0.9);
        assert_eq!(c.resting_state, RestingState::Rail);
        assert!(!c.espn_rich_events);
        assert!(!c.history_enabled);
        assert!(!c.now_playing_enabled);
        assert!(c.now_playing_adapter_enabled);
        // plan 104 revision: user-writable default — pin the suffix, not
        // the whole absolute path, since the leading component is this
        // test-runner's own $HOME (CI/local machines differ).
        assert!(c
            .now_playing_adapter_dir
            .ends_with("Library/Application Support/notchtap/mediaremote-adapter"));
    }

    #[test]
    fn espn_rich_events_defaults_to_false_and_is_overridable() {
        // plan 083 workstream c: mirrors espn_live_card's opt-in pattern —
        // default off, this heavier per-match feed must not turn on for
        // an install that hasn't opted in.
        let default = Config::parse("").unwrap();
        assert!(!default.espn_rich_events);

        let on = Config::parse("espn_rich_events = true\n").unwrap();
        assert!(on.espn_rich_events);
    }

    #[test]
    fn resting_state_defaults_to_rail_and_is_overridable() {
        // plan 085: a config file predating this field (or one that simply
        // never sets it) heals to `rail` — zero behavior change by default.
        let healed = Config::parse("").unwrap();
        assert_eq!(healed.resting_state, RestingState::Rail);

        let notch = Config::parse("resting_state = \"notch\"\n").unwrap();
        assert_eq!(notch.resting_state, RestingState::Notch);

        let rail = Config::parse("resting_state = \"rail\"\n").unwrap();
        assert_eq!(rail.resting_state, RestingState::Rail);
    }

    #[test]
    fn now_playing_disabled_by_default() {
        // plan 104: ambient sources are opt-in per machine — same rule as
        // weather_enabled/rss_enabled. A config file predating this field
        // (or one that simply never sets it) heals to `false`.
        let c = Config::parse("").unwrap();
        assert!(!c.now_playing_enabled);
        // the kill switch defaults `true`: once a user opts into the
        // feature, the adapter runs unless someone has explicitly muted
        // it in config.toml by hand.
        assert!(c.now_playing_adapter_enabled);
    }

    #[test]
    fn now_playing_fields_are_overridable() {
        let c = Config::parse(
            "now_playing_enabled = true\nnow_playing_adapter_enabled = false\nnow_playing_adapter_dir = \"/opt/mediaremote-adapter\"\n",
        )
        .unwrap();
        assert!(c.now_playing_enabled);
        assert!(!c.now_playing_adapter_enabled);
        assert_eq!(
            c.now_playing_adapter_dir,
            PathBuf::from("/opt/mediaremote-adapter")
        );
    }

    #[test]
    fn history_enabled_defaults_to_false_and_is_overridable() {
        // plan 088: a config file predating this field (or one that simply
        // never sets it) heals to `false` — off-by-default, matching every
        // other opt-in surface, since this one writes notification CONTENT
        // to disk.
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
        // own default (8), and cmux inherits that same resolved value —
        // identical to today's fresh-install behavior. plan 101: espn no
        // longer inherits here — with default_ttl untouched, espn gets
        // its own default (15) instead.
        let c = Config::parse("").unwrap();
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.espn_ttl_secs, 15);
        assert_eq!(c.cmux_ttl_secs, 8);
    }

    #[test]
    fn espn_ttl_defaults_to_15_when_default_ttl_untouched() {
        // plan 101: espn's own default (15) applies when the file never
        // customized default_ttl — the generic default itself must not
        // have moved.
        let c = Config::parse("").unwrap();
        assert_eq!(c.espn_ttl_secs, 15);
        assert_eq!(c.default_ttl, default_ttl());

        // but the heal still honors a customized shared default_ttl.
        let c = Config::parse("default_ttl = 30\n").unwrap();
        assert_eq!(c.espn_ttl_secs, 30);
    }

    #[test]
    fn weather_fields_are_overridable() {
        let c = Config::parse(
            "weather_enabled = true\nweather_lat = 12.97\nweather_lon = 77.59\nweather_units = \"fahrenheit\"\nweather_poll_secs = 300\nweather_rain_threshold_pct = 80\nweather_rain_lookahead_mins = 60\nweather_temp_hot_c = 40.0\nweather_temp_cold_c = 10.0\nweather_priority = \"high\"\n",
        )
        .unwrap();
        assert!(c.weather_enabled);
        assert_eq!(c.weather_lat, 12.97);
        assert_eq!(c.weather_lon, 77.59);
        assert_eq!(c.weather_units, Units::Fahrenheit);
        assert_eq!(c.weather_poll_secs, 300);
        assert_eq!(c.weather_rain_threshold_pct, 80);
        assert_eq!(c.weather_rain_lookahead_mins, 60);
        assert_eq!(c.weather_temp_hot_c, 40.0);
        assert_eq!(c.weather_temp_cold_c, 10.0);
        assert_eq!(c.weather_priority, Priority::High);
    }

    #[test]
    fn unknown_units_string_is_a_parse_error() {
        assert!(Config::parse("weather_units = \"kelvin\"").is_err());
    }

    #[test]
    fn rotation_order_is_overridable() {
        let c = Config::parse(
            "rotation_order = [\"news\", \"football\", \"cmux\", \"manual\", \"weather\"]\n",
        )
        .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Cmux,
                SourceKind::Manual,
                SourceKind::Weather,
            ]
        );
    }

    #[test]
    fn rotation_order_missing_a_source_is_healed_by_appending_it() {
        // a config written before `weather` existed (e.g. pre-plan-040):
        // the settings UI's rotation-order list can't add a missing source
        // back on its own, so `Config::parse` must heal it at load time or
        // every save attempt fails `validate`'s permutation check forever.
        let c = Config::parse("rotation_order = [\"news\", \"football\", \"cmux\", \"manual\"]\n")
            .unwrap();
        assert_eq!(
            c.rotation_order,
            [
                SourceKind::News,
                SourceKind::Football,
                SourceKind::Cmux,
                SourceKind::Manual,
                SourceKind::Weather,
            ]
        );
    }

    #[test]
    fn rotation_order_with_duplicate_and_missing_sources_heals_to_exactly_five() {
        // a malformed config: `football` appears twice, `news` and `cmux`
        // are both missing entirely. The heal must both dedupe the
        // duplicate AND append the two missing sources, landing at exactly
        // 5 unique entries — not 6, which would still fail `validate`'s
        // permutation check.
        let c = Config::parse(
            "rotation_order = [\"football\", \"football\", \"manual\", \"weather\"]\n",
        )
        .unwrap();
        assert_eq!(c.rotation_order.len(), 5);
        let mut sorted = c.rotation_order.clone();
        sorted.sort_by_key(|s| format!("{s:?}"));
        let mut expected = vec![
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::Weather,
            SourceKind::News,
            SourceKind::Cmux,
        ];
        expected.sort_by_key(|s| format!("{s:?}"));
        assert_eq!(sorted, expected);
        // first-occurrence order preserved for what the file already had:
        // football (deduped to one), manual, weather stay in that relative
        // order; only the appended news/cmux go at the end.
        assert_eq!(c.rotation_order[0], SourceKind::Football);
        assert_eq!(c.rotation_order[1], SourceKind::Manual);
        assert_eq!(c.rotation_order[2], SourceKind::Weather);
    }

    #[test]
    fn unknown_priority_or_source_kind_string_is_a_parse_error() {
        assert!(Config::parse("espn_priority = \"urgent\"").is_err());
        assert!(Config::parse("rotation_order = [\"telegram\"]").is_err());
    }

    // plan 097: `validate_appearance` (settings.rs) only guards the
    // settings-save path — a hand-edited `config.toml` bypasses it
    // entirely. `Config::parse` must clamp out-of-range appearance values
    // at load time too, or a degenerate `card_scale = 0.0` boots a broken
    // hover rect and card rendering.
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

    // plan 097: a non-finite value can't be expressed through the settings
    // UI (only a hand-edited TOML can write `nan`), and clamping a NaN is
    // a no-op in IEEE 754 (`NaN.clamp(lo, hi)` stays NaN) — so non-finite
    // values fall back to the field's own default instead.
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
}
