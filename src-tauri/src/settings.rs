//! v5 settings window backend (`docs/V5_TECHNICAL_SPEC.md`). Owns the
//! app's only invoke commands — all four are settings-window-scoped:
//! gated declaratively by the `build.rs` `AppManifest::commands` opt-in +
//! `capabilities/settings.json`, and defensively by [`ensure_settings_window`]
//! (the tauri acl does not protect against scope bugs in handlers, so the
//! label check stays even though the acl should make it unreachable).
//!
//! Everything decision-shaped in here is a pure function (validate, mask,
//! merge); the commands are thin wrappers. Write paths are atomic
//! (same-dir temp file + rename) because a half-written `config.toml` is
//! a bricked boot given `Config::load`'s fail-fast rule.

use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::config::{Appearance, Config, RestingState};
use crate::engine::Engine;
use crate::event::{
    Event, EventMeta, EventPayload, EventSignal, EventType, RotationSpec, SourceKind,
};
use tauri::Manager;

// ---------------------------------------------------------------------------
// validation (pure, unit-tested — spec §3)
// ---------------------------------------------------------------------------

/// Normalized match key for a feed url (plan 021 — mirrors the frontend's
/// `feedKey` in `SettingsApp.tsx`): clear the fragment and trim a single
/// trailing slash so a cosmetic variant (trailing "/", a `#anchor`) is
/// recognized as the same feed for duplicate rejection. Falls back to the
/// trimmed raw string on parse failure — malformed urls already get their
/// own error from the per-feed parse check above, so this loop only needs
/// "close enough" grouping for parse failures, not correctness.
fn feed_key(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            parsed.to_string().trim_end_matches('/').to_string()
        }
        Err(_) => url.trim().to_string(),
    }
}

/// Every rule violated contributes one human-readable message — the
/// settings form renders the whole list, not just the first failure.
pub fn validate(c: &Config) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if c.port < 1024 {
        errors.push(format!(
            "port must be 1024–65535 (got {}) — privileged ports fail to bind at boot",
            c.port
        ));
    }
    if !(1..=3600).contains(&c.default_ttl) {
        errors.push(format!(
            "default_ttl must be 1–3600 seconds (got {})",
            c.default_ttl
        ));
    }
    if !(1..=1000).contains(&c.max_queued_per_tier) {
        errors.push(format!(
            "max_queued_per_tier must be 1–1000 (got {})",
            c.max_queued_per_tier
        ));
    }
    if !(5..=3600).contains(&c.espn_poll_secs) {
        errors.push(format!(
            "espn_poll_secs must be 5–3600 (got {}) — below 5s is abuse of a free endpoint",
            c.espn_poll_secs
        ));
    }
    if !(1..=3600).contains(&c.espn_ttl_secs) {
        errors.push(format!(
            "espn_ttl_secs must be 1–3600 seconds (got {})",
            c.espn_ttl_secs
        ));
    }
    if !(1..=3600).contains(&c.cmux_ttl_secs) {
        errors.push(format!(
            "cmux_ttl_secs must be 1–3600 seconds (got {})",
            c.cmux_ttl_secs
        ));
    }
    for league in &c.espn_leagues {
        if league.is_empty() || league.chars().any(char::is_whitespace) {
            errors.push(format!(
                "league {league:?} is invalid — entries must be non-empty with no whitespace"
            ));
        }
    }
    if c.espn_enabled && c.espn_leagues.is_empty() {
        errors
            .push("espn_enabled is on but espn_leagues is empty — add a league or disable".into());
    }
    if !(5..=3600).contains(&c.rss_poll_secs) {
        errors.push(format!(
            "rss_poll_secs must be 5–3600 (got {})",
            c.rss_poll_secs
        ));
    }
    if !(1..=3600).contains(&c.rss_ttl_secs) {
        errors.push(format!(
            "rss_ttl_secs must be 1–3600 (got {})",
            c.rss_ttl_secs
        ));
    }
    if !(1..=100).contains(&c.rss_max_per_poll) {
        errors.push(format!(
            "rss_max_per_poll must be 1–100 (got {})",
            c.rss_max_per_poll
        ));
    }
    for feed in &c.rss_feeds {
        // full parse, not a prefix check (2026-07-17 review): "https://"
        // alone or a host-less url would pass a starts_with test and then
        // fail on every poll. whitespace is rejected explicitly because
        // the url parser is lenient enough to percent-encode some of it.
        let parsed_ok = !feed.url.chars().any(char::is_whitespace)
            && reqwest::Url::parse(&feed.url)
                .map(|u| (u.scheme() == "http" || u.scheme() == "https") && u.host_str().is_some())
                .unwrap_or(false);
        if !parsed_ok {
            errors.push(format!(
                "feed {:?} is invalid — entries must be full http(s) urls with a host and no whitespace",
                feed.url
            ));
        }
    }
    if c.rss_enabled && c.rss_feeds.is_empty() {
        errors.push("rss_enabled is on but rss_feeds is empty — add a feed or disable".into());
    }

    if !(-90.0..=90.0).contains(&c.weather_lat) {
        errors.push(format!(
            "weather_lat must be -90.0–90.0 (got {})",
            c.weather_lat
        ));
    }
    if !(-180.0..=180.0).contains(&c.weather_lon) {
        errors.push(format!(
            "weather_lon must be -180.0–180.0 (got {})",
            c.weather_lon
        ));
    }
    if !(5..=3600).contains(&c.weather_poll_secs) {
        errors.push(format!(
            "weather_poll_secs must be 5–3600 (got {})",
            c.weather_poll_secs
        ));
    }
    if c.weather_rain_threshold_pct > 100 {
        errors.push(format!(
            "weather_rain_threshold_pct must be 0–100 (got {})",
            c.weather_rain_threshold_pct
        ));
    }
    if !(5..=120).contains(&c.weather_rain_lookahead_mins) {
        errors.push(format!(
            "weather_rain_lookahead_mins must be 5–120 (got {})",
            c.weather_rain_lookahead_mins
        ));
    }
    // cross-field check: the hot threshold must sit strictly above the
    // cold one, or the temp alert would fire on every poll.
    if c.weather_temp_hot_c <= c.weather_temp_cold_c {
        errors.push(format!(
            "weather_temp_hot_c must be greater than weather_temp_cold_c (got {} <= {})",
            c.weather_temp_hot_c, c.weather_temp_cold_c
        ));
    }
    {
        // Duplicate feeds double the poll's network work per tick even
        // though the SeenStore hides the duplicate notifications — reject
        // rather than silently pay that cost (plan 021).
        let mut seen_keys = std::collections::HashSet::new();
        for feed in &c.rss_feeds {
            if !seen_keys.insert(feed_key(&feed.url)) {
                errors.push(format!("duplicate rss feed: {}", feed.url));
            }
        }
    }

    // rotation_order must be a permutation of all five SourceKind variants
    // — the ui is a fixed 5-row reorder list, never add/remove, so any
    // other shape means the ipc caller bypassed it.
    let expected_sources = [
        crate::event::SourceKind::Football,
        crate::event::SourceKind::Manual,
        crate::event::SourceKind::Weather,
        crate::event::SourceKind::News,
        crate::event::SourceKind::Cmux,
    ];
    let is_permutation = c.rotation_order.len() == expected_sources.len()
        && expected_sources
            .iter()
            .all(|source| c.rotation_order.contains(source));
    if !is_permutation {
        errors.push(
            "rotation_order must contain each of football, manual, weather, news, and cmux exactly once"
                .into(),
        );
    }

    if let Err(mut appearance_errors) = validate_appearance(&c.appearance) {
        errors.append(&mut appearance_errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_appearance(a: &Appearance) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if !(0.8..=1.4).contains(&a.card_scale) {
        errors.push(format!("card_scale must be 0.8–1.4 (got {})", a.card_scale));
    }
    if !(0.0..=24.0).contains(&a.card_radius) {
        errors.push(format!(
            "card_radius must be 0.0–24.0 (got {})",
            a.card_radius
        ));
    }
    if !(0.5..=1.0).contains(&a.card_opacity) {
        errors.push(format!(
            "card_opacity must be 0.5–1.0 (got {})",
            a.card_opacity
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// secret masking (pure, unit-tested — spec §4)
// ---------------------------------------------------------------------------

/// `"set (…a1b2)"` for values of 8+ chars, plain `"set"` below that —
/// short values would leak most of themselves through their own tail.
/// The full value never crosses ipc outbound; this string is all the
/// settings window ever sees.
pub fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() >= 8 {
        let last4: String = chars[chars.len() - 4..].iter().collect();
        format!("set (…{last4})")
    } else {
        "set".to_string()
    }
}

// ---------------------------------------------------------------------------
// secrets document (read-modify-write target — spec §4)
// ---------------------------------------------------------------------------

/// The whole-file shape of `secrets.toml` as the settings writer sees it:
/// every table and field optional, so setting one field never demands the
/// others exist. The telegram *connector* keeps its own strict loader
/// (`notifier::load_secrets`) — incomplete telegram secrets disable the
/// connector at boot with a warning, same as always.
///
/// The `extra` maps (2026-07-17 review) preserve unknown tables and
/// unknown fields inside known tables across a read-modify-write —
/// without them, serde would silently drop anything this struct doesn't
/// model, and "setting one key deletes a hand-added table" would violate
/// the never-clobber rule this module promises.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SecretsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramTable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<OpenRouterTable>,
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TelegramTable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterTable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

/// The three settable fields — a closed set, so the ipc surface can never
/// be steered at an arbitrary toml path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretField {
    OpenrouterApiKey,
    TelegramBotToken,
    TelegramChatId,
}

/// Masked per-field status for the settings form (spec §2) — presence and
/// completeness in one shape, never a value.
#[derive(Debug, PartialEq, Serialize)]
pub struct SecretStatus {
    pub openrouter_api_key: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
}

/// Wire shape for the telegram connector's delivery health (plan 076).
/// `ConnectorHealth` keeps `Instant`s internal; this DTO converts them to
/// elapsed-milliseconds-since-now at the boundary (`status.rs` has no
/// existing Instant-to-wire precedent to mirror — its only `Instant`
/// uses are in tests). camelCase on the wire, matching status.rs's
/// convention.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorHealthDto {
    /// ms since the last send attempt, success or failure
    pub last_attempt_ms: Option<i64>,
    /// ms since the last confirmed delivery
    pub last_success_ms: Option<i64>,
    pub consecutive_failures: u32,
}

// ---------------------------------------------------------------------------
// pure merge + status (unit-tested against in-memory docs)
// ---------------------------------------------------------------------------

/// Sets exactly one field, materializing its table if needed and touching
/// nothing else — "set openrouter preserves telegram" is the tested
/// contract (spec §7).
pub fn merge_secret(doc: &mut SecretsDoc, field: SecretField, value: String) {
    match field {
        SecretField::OpenrouterApiKey => {
            doc.openrouter.get_or_insert_with(Default::default).api_key = Some(value);
        }
        SecretField::TelegramBotToken => {
            doc.telegram.get_or_insert_with(Default::default).bot_token = Some(value);
        }
        SecretField::TelegramChatId => {
            doc.telegram.get_or_insert_with(Default::default).chat_id = Some(value);
        }
    }
}

pub fn secret_status(doc: &SecretsDoc) -> SecretStatus {
    SecretStatus {
        openrouter_api_key: doc
            .openrouter
            .as_ref()
            .and_then(|t| t.api_key.as_deref())
            .map(mask),
        telegram_bot_token: doc
            .telegram
            .as_ref()
            .and_then(|t| t.bot_token.as_deref())
            .map(mask),
        telegram_chat_id: doc
            .telegram
            .as_ref()
            .and_then(|t| t.chat_id.as_deref())
            .map(mask),
    }
}

/// Secret values are validated for shape only (spec §4): non-empty, no
/// whitespace. Nothing reads the openrouter key in v5 — it waits for the
/// first ai feature.
pub fn validate_secret_value(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("secret value must not be empty".into());
    }
    if value.chars().any(char::is_whitespace) {
        return Err("secret value must not contain whitespace".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// write paths (atomic; integration-tested against temp dirs, never $HOME)
// ---------------------------------------------------------------------------

/// A fresh, never-before-existing temp path in `dir` (2026-07-17 review):
/// a *fixed* temp name could pre-exist with permissive permissions, and
/// `OpenOptions::mode` only applies at creation — writing a secret into a
/// stale world-readable temp file would void the 0600 guarantee. Unique
/// name + `create_new` makes creation (and therefore the mode) certain.
fn unique_tmp(dir: &Path, base: &str) -> PathBuf {
    dir.join(format!("{base}.tmp.{}", uuid::Uuid::new_v4()))
}

fn write_then_rename(
    tmp: &Path,
    dest: &Path,
    contents: &str,
    mode: Option<u32>,
) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let attempt = (|| -> anyhow::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        if let Some(mode) = mode {
            options.mode(mode);
        }
        let mut f = options.open(tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(tmp, dest)?;
        Ok(())
    })();
    if attempt.is_err() {
        // best-effort: don't leave a half-written temp file behind
        let _ = std::fs::remove_file(tmp);
    }
    attempt
}

/// Serialize the whole config and atomically replace `config.toml` in
/// `dir`. Same-dir temp file + rename — rename across filesystems isn't
/// atomic, and a torn `config.toml` is a bricked boot. Known, accepted
/// loss (spec §3): hand-written comments in the file don't survive.
pub fn write_config_atomic(dir: &Path, config: &Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let serialized = toml::to_string_pretty(config)?;
    write_then_rename(
        &unique_tmp(dir, "config.toml"),
        &dir.join("config.toml"),
        &serialized,
        None,
    )
}

/// Atomically replace `secrets.toml` in `dir`, mode `0600` from the first
/// byte — the temp file is `create_new` with the final permissions, so
/// there is no window where secret content sits with any other mode.
pub fn write_secrets_atomic(dir: &Path, doc: &SecretsDoc) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let serialized = toml::to_string_pretty(doc)?;
    write_then_rename(
        &unique_tmp(dir, "secrets.toml"),
        &dir.join("secrets.toml"),
        &serialized,
        Some(0o600),
    )
}

/// Load the secrets doc for a read-modify-write. Missing file → empty doc
/// (first key ever pasted creates it). **Malformed file → hard error**,
/// never a clobber — the user may have hand-edited it wrong, and silently
/// overwriting would destroy whatever they meant to keep (spec §4).
///
/// The parse error itself is deliberately withheld (2026-07-17 review):
/// `toml::de::Error`'s Display can echo the offending source line — which
/// in this file is secret material — straight across ipc.
pub fn load_secrets_doc(dir: &Path) -> Result<SecretsDoc, String> {
    let path = dir.join("secrets.toml");
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).map_err(|_| {
            "secrets.toml is malformed — fix or delete it by hand \
             (parse detail withheld: it could echo secret material)"
                .to_string()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SecretsDoc::default()),
        Err(e) => Err(format!("secrets.toml unreadable: {e}")),
    }
}

/// Serializes every secrets read-modify-write (2026-07-17 review): two
/// concurrent `set_secret` invocations would otherwise both read the old
/// doc and one field would silently overwrite the other. In-process only —
/// the app has no single-instance enforcement, but two running copies
/// already fail loudly at the port bind, so a file lock is unearned.
static SECRETS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The one full set-a-secret path the command wraps: trim → validate →
/// load → merge → atomic 0600 write, all under [`SECRETS_LOCK`]. The trim
/// (2026-07-17 review) forgives the trailing newline every clipboard
/// paste carries — rejecting it as "contains whitespace" would make the
/// most common paste fail confusingly.
pub fn set_secret_in(dir: &Path, field: SecretField, value: String) -> Result<(), String> {
    let value = value.trim().to_string();
    validate_secret_value(&value)?;
    let _guard = SECRETS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut doc = load_secrets_doc(dir)?;
    merge_secret(&mut doc, field, value);
    write_secrets_atomic(dir, &doc).map_err(|e| format!("could not write secrets.toml: {e}"))
}

fn notchtap_config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| Config::dir_from_home(&h))
        .ok_or_else(|| "could not determine home directory".to_string())
}

// ---------------------------------------------------------------------------
// invoke commands (thin; scope-checked; untested by design — the logic
// they call is tested above, `TESTING_STRATEGY.md` §4.11)
// ---------------------------------------------------------------------------

/// Defense-in-depth behind the acl (spec §2): app commands would be
/// window-agnostic without the `build.rs` opt-in, and a future
/// `generate_handler` edit that forgets that list must fail closed here.
fn ensure_settings_window<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    if window.label() == "settings" {
        Ok(())
    } else {
        Err("settings commands are settings-window-only".to_string())
    }
}

/// IPC payload for `appearance-changed`: sent to the overlay whenever the
/// user updates card styling, or any other overlay-behavior field on the
/// appearance channel. The field names stay camelCase-free; the frontend's
/// listener mirrors this shape directly.
///
/// plan 085: `resting_state` widened this beyond pure card styling — it's a
/// top-level `Config` field, not part of `Appearance`, so this payload is
/// always built from the whole `Config` (`from_config`), never from
/// `Appearance` alone. That matters even for a pure appearance-only change
/// (`set_appearance`): the emitted event must still carry the *current*
/// `resting_state`, or the frontend's tracked value would fall back to its
/// default on every unrelated scale/radius/opacity tweak.
#[derive(Clone, serde::Serialize)]
pub struct AppearanceChangedPayload {
    pub scale: f64,
    pub radius: f64,
    pub opacity: f64,
    pub resting_state: RestingState,
}

impl AppearanceChangedPayload {
    pub fn from_config(config: &Config) -> Self {
        Self {
            scale: config.appearance.card_scale,
            radius: config.appearance.card_radius,
            opacity: config.appearance.card_opacity,
            resting_state: config.resting_state,
        }
    }
}

fn broadcast_appearance_change<R: tauri::Runtime>(app: &tauri::AppHandle<R>, config: &Config) {
    use tauri::Emitter;
    let payload = AppearanceChangedPayload::from_config(config);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("appearance-changed", &payload);
    }
}

fn timestamp_body() -> String {
    format!("Test · sent {}", Local::now().format("%H:%M:%S"))
}

fn build_test_event(config: &Config, source: SourceKind) -> Event {
    let now_ms = Local::now().timestamp_millis();
    match source {
        SourceKind::Football => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::ScoreUpdate,
            priority: config.espn_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.espn_ttl_secs,
            },
            topic: None,
            payload: EventPayload {
                title: "Test score update".into(),
                body: timestamp_body(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Goal,
            origin: SourceKind::Football,
        },
        SourceKind::News => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::NewsItem,
            priority: config.rss_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.rss_ttl_secs,
            },
            topic: None,
            payload: EventPayload {
                title: "Test news headline".into(),
                body: timestamp_body(),
            },
            meta: EventMeta {
                source: Some("Settings".into()),
                category: Some("preview".into()),
                published_at_ms: Some(now_ms),
                link: None,
                subtitle: None,
                details: Vec::new(),
                espn: None,
            },
            signal: EventSignal::Generic,
            origin: SourceKind::News,
        },
        SourceKind::Cmux => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: config.cmux_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.cmux_ttl_secs,
            },
            topic: None,
            payload: EventPayload {
                title: "Test · agent relay".into(),
                body: "This is how cmux alerts look".into(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Cmux,
        },
        SourceKind::Manual => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: config.manual_default_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.default_ttl,
            },
            topic: None,
            payload: EventPayload {
                title: "Test notification".into(),
                body: timestamp_body(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        },
        SourceKind::Weather => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: config.weather_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.default_ttl,
            },
            topic: None,
            payload: EventPayload {
                title: "Test weather alert".into(),
                body: timestamp_body(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Weather,
        },
    }
}

/// Returns the **booted** config (managed state) — "what is running",
/// which save-and-relaunch makes true of the file again after every save.
#[tauri::command]
pub fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    let config = state.inner().lock().unwrap().clone();
    Ok(config)
}

/// Serves Config::default() so the frontend never mirrors defaults
/// (plan 020) — the "Reset to defaults" source of truth is config.rs.
#[tauri::command]
pub fn get_default_config<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    Ok(Config::default())
}

#[tauri::command]
pub fn get_secret_status(window: tauri::WebviewWindow) -> Result<SecretStatus, String> {
    ensure_settings_window(&window)?;
    let dir = notchtap_config_dir()?;
    let doc = load_secrets_doc(&dir)?;
    Ok(secret_status(&doc))
}

/// The panel never edits `detect_path` (ARCHITECTURE.md §17: file-only) —
/// but the ui not *showing* a field is not a boundary (2026-07-17 review:
/// it's an executed subprocess path, the one config field with code-exec
/// consequences). Pin it server-side to the booted value so the ipc
/// surface enforces what the spec states, regardless of what the webview
/// submits.
pub fn pin_uneditable_fields(mut submitted: Config, booted: &Config) -> Config {
    submitted.detect_path = booted.detect_path.clone();
    submitted
}

/// Best-effort pre-flight (plan 021): the relaunched app `exit(1)`s on a
/// taken port with no UI — catch the common collision before writing. A
/// race remains possible (port taken between check and relaunch); this
/// narrows the window, it doesn't close it — accepted. The `new != booted`
/// guard matters: the app itself holds the booted port, so binding it
/// would false-positive against our own listener.
pub fn preflight_port(new: u16, booted: u16) -> Result<(), String> {
    if new != booted {
        if let Err(e) = std::net::TcpListener::bind(("127.0.0.1", new)) {
            return Err(format!(
                "port {new} is not bindable right now ({e}) — pick another or free it first"
            ));
        }
    }
    Ok(())
}

/// Validate → atomic write → relaunch. The `Err` arm carries the whole
/// per-field message list for the form; on success the process is gone
/// before a reply could matter.
#[tauri::command]
pub fn save_config_and_relaunch(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, StdMutex<Config>>,
    config: Config,
) -> Result<(), Vec<String>> {
    ensure_settings_window(&window).map_err(|e| vec![e])?;
    let booted = state.inner().lock().unwrap().clone();
    let config = pin_uneditable_fields(config, &booted);
    validate(&config)?;
    preflight_port(config.port, booted.port).map_err(|e| vec![e])?;
    let dir = notchtap_config_dir().map_err(|e| vec![e])?;
    write_config_atomic(&dir, &config)
        .map_err(|e| vec![format!("could not write config.toml: {e}")])?;
    tracing::info!("config saved from settings window — relaunching");
    app.restart();
}

#[tauri::command]
pub fn set_secret(
    window: tauri::WebviewWindow,
    field: SecretField,
    value: String,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let dir = notchtap_config_dir()?;
    set_secret_in(&dir, field, value)
}

#[tauri::command]
pub async fn send_test_notification(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    engine: tauri::State<'_, Engine>,
    source: SourceKind,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let config = state.inner().lock().unwrap().clone();
    let event = build_test_event(&config, source);
    // plan 037: Engine::accept performs the enqueue with the one
    // mutate→wake→emit protocol (a test notification pushed from the
    // Settings window rotates out on schedule — plan 015's review
    // follow-up — by construction now, not convention).
    engine.accept(event, true).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_connector_health(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<ConnectorHealthDto, String> {
    ensure_settings_window(&window)?;
    let health = engine.telegram_health();
    let elapsed_ms =
        |t: std::time::Instant| i64::try_from(t.elapsed().as_millis()).unwrap_or(i64::MAX);
    Ok(ConnectorHealthDto {
        last_attempt_ms: health.last_attempt.map(elapsed_ms),
        last_success_ms: health.last_success.map(elapsed_ms),
        consecutive_failures: health.consecutive_failures,
    })
}

#[tauri::command]
pub async fn get_recent_log_lines(window: tauri::WebviewWindow) -> Result<Vec<String>, String> {
    ensure_settings_window(&window)?;
    // plan 077: no content-based redaction layer here on purpose —
    // notifier.rs's token redaction (plan 006, `e.without_url()`) already
    // keeps secrets out of the log file itself, so the file is safe to
    // surface read-only in the settings window.
    crate::logging::read_recent_lines(200).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_appearance(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, StdMutex<Config>>,
    scale: f64,
    radius: f64,
    opacity: f64,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let appearance = Appearance {
        card_scale: scale,
        card_radius: radius,
        card_opacity: opacity,
    };
    validate_appearance(&appearance).map_err(|errors| errors.join("; "))?;

    let dir = notchtap_config_dir()?;
    let mut config = state.inner().lock().unwrap().clone();
    config.appearance = appearance.clone();
    write_config_atomic(&dir, &config).map_err(|e| format!("could not write config.toml: {e}"))?;
    {
        let mut managed = state.inner().lock().unwrap();
        managed.appearance = appearance.clone();
    }
    broadcast_appearance_change(&app, &config);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // --- validate: every rule's accept/reject boundary ---

    #[test]
    fn default_config_validates_clean() {
        assert!(validate(&Config::default()).is_ok());
    }

    #[test]
    fn privileged_port_is_rejected_at_the_boundary() {
        let mut c = Config {
            port: 1023,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.port = 1024;
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn ttl_boundaries() {
        let mut c = Config {
            default_ttl: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.default_ttl = 1;
        assert!(validate(&c).is_ok());
        c.default_ttl = 3600;
        assert!(validate(&c).is_ok());
        c.default_ttl = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn queue_cap_boundaries() {
        let mut c = Config {
            max_queued_per_tier: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.max_queued_per_tier = 1;
        assert!(validate(&c).is_ok());
        c.max_queued_per_tier = 1000;
        assert!(validate(&c).is_ok());
        c.max_queued_per_tier = 1001;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn espn_ttl_boundaries() {
        let mut c = Config {
            espn_ttl_secs: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.espn_ttl_secs = 1;
        assert!(validate(&c).is_ok());
        c.espn_ttl_secs = 3600;
        assert!(validate(&c).is_ok());
        c.espn_ttl_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn cmux_ttl_boundaries() {
        let mut c = Config {
            cmux_ttl_secs: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.cmux_ttl_secs = 1;
        assert!(validate(&c).is_ok());
        c.cmux_ttl_secs = 3600;
        assert!(validate(&c).is_ok());
        c.cmux_ttl_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn rotation_order_must_be_a_permutation() {
        use crate::event::SourceKind;

        // missing entries
        let mut c = Config {
            rotation_order: vec![SourceKind::Football, SourceKind::Manual],
            ..Config::default()
        };
        assert!(validate(&c).is_err());

        // duplicate entry (still length 5, but News is missing)
        c.rotation_order = vec![
            SourceKind::Football,
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::Weather,
            SourceKind::Cmux,
        ];
        assert!(validate(&c).is_err());

        // correct permutation, any order
        c.rotation_order = vec![
            SourceKind::News,
            SourceKind::Football,
            SourceKind::Cmux,
            SourceKind::Weather,
            SourceKind::Manual,
        ];
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn poll_interval_boundaries() {
        let mut c = Config {
            espn_poll_secs: 4,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.espn_poll_secs = 5;
        assert!(validate(&c).is_ok());
        c.espn_poll_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn weather_field_ranges_validate() {
        let mut c = Config {
            weather_lat: 91.0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.weather_lat = -90.0;
        assert!(validate(&c).is_ok());
        c.weather_lon = 181.0;
        assert!(validate(&c).is_err());
        c.weather_lon = -180.0;
        assert!(validate(&c).is_ok());
        c.weather_poll_secs = 4;
        assert!(validate(&c).is_err());
        c.weather_poll_secs = 900;
        assert!(validate(&c).is_ok());
        c.weather_rain_threshold_pct = 101;
        assert!(validate(&c).is_err());
        c.weather_rain_threshold_pct = 100;
        assert!(validate(&c).is_ok());
        c.weather_rain_lookahead_mins = 4;
        assert!(validate(&c).is_err());
        c.weather_rain_lookahead_mins = 121;
        assert!(validate(&c).is_err());
        c.weather_rain_lookahead_mins = 30;
        assert!(validate(&c).is_ok());
        // cross-field: hot must sit strictly above cold
        c.weather_temp_hot_c = 14.0;
        c.weather_temp_cold_c = 14.0;
        assert!(validate(&c).is_err());
        c.weather_temp_hot_c = 36.0;
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn league_entries_must_be_nonempty_and_whitespace_free() {
        let mut c = Config {
            espn_leagues: vec!["eng.1".into(), "".into()],
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.espn_leagues = vec!["eng 1".into()];
        assert!(validate(&c).is_err());
        c.espn_leagues = vec!["eng.1".into()];
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn empty_league_list_rejected_only_while_espn_enabled() {
        let mut c = Config {
            espn_leagues: vec![],
            espn_enabled: true,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.espn_enabled = false;
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn multiple_violations_accumulate() {
        let c = Config {
            port: 80,
            default_ttl: 0,
            espn_poll_secs: 1,
            ..Config::default()
        };
        let errors = validate(&c).unwrap_err();
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn appearance_boundaries_accepted() {
        assert!(validate_appearance(&Appearance {
            card_scale: 0.8,
            card_radius: 0.0,
            card_opacity: 0.5,
        })
        .is_ok());
        assert!(validate_appearance(&Appearance {
            card_scale: 1.4,
            card_radius: 24.0,
            card_opacity: 1.0,
        })
        .is_ok());
    }

    #[test]
    fn appearance_rejects_out_of_range_values() {
        let low = validate_appearance(&Appearance {
            card_scale: 0.79,
            card_radius: -0.1,
            card_opacity: 0.49,
        })
        .unwrap_err();
        assert_eq!(low.len(), 3);

        let high = validate_appearance(&Appearance {
            card_scale: 1.41,
            card_radius: 24.1,
            card_opacity: 1.01,
        })
        .unwrap_err();
        assert_eq!(high.len(), 3);
    }

    // --- appearance-changed payload (plan 085 widened it with resting_state) ---

    #[test]
    fn appearance_changed_payload_carries_resting_state_from_config() {
        // plan 085: the payload is built from the whole Config, not just
        // Appearance — a pure appearance change (set_appearance) must still
        // report the config's actual resting_state, not a default.
        let mut config = Config {
            resting_state: crate::config::RestingState::Notch,
            ..Config::default()
        };
        config.appearance.card_scale = 1.2;
        let payload = AppearanceChangedPayload::from_config(&config);
        assert_eq!(payload.scale, 1.2);
        assert_eq!(payload.resting_state, crate::config::RestingState::Notch);

        config.resting_state = crate::config::RestingState::Rail;
        let payload = AppearanceChangedPayload::from_config(&config);
        assert_eq!(payload.resting_state, crate::config::RestingState::Rail);
    }

    #[test]
    fn appearance_changed_payload_serializes_resting_state_as_snake_case_string() {
        let config = Config {
            resting_state: crate::config::RestingState::Notch,
            ..Config::default()
        };
        let payload = AppearanceChangedPayload::from_config(&config);
        let json = serde_json::to_value(&payload).unwrap();
        // no camelCase rename on this payload (the frontend listener
        // mirrors the shape directly) — snake_case wire field, snake_case
        // value, matching the frontend's `"rail" | "notch"` union exactly.
        assert_eq!(json["resting_state"], serde_json::json!("notch"));
    }

    // --- rss rules (v5 news backend, folded into the panel 2026-07-17) ---

    #[test]
    fn rss_poll_interval_boundaries() {
        let mut c = Config {
            rss_poll_secs: 4,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.rss_poll_secs = 5;
        assert!(validate(&c).is_ok());
        c.rss_poll_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn rss_ttl_boundaries() {
        let mut c = Config {
            rss_ttl_secs: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.rss_ttl_secs = 1;
        assert!(validate(&c).is_ok());
        c.rss_ttl_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn rss_max_per_poll_boundaries() {
        let mut c = Config {
            rss_max_per_poll: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.rss_max_per_poll = 1;
        assert!(validate(&c).is_ok());
        c.rss_max_per_poll = 100;
        assert!(validate(&c).is_ok());
        c.rss_max_per_poll = 101;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn rss_feeds_must_be_http_urls_without_whitespace() {
        let mut c = Config {
            rss_feeds: vec!["ftp://example.com/feed".into()],
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.rss_feeds = vec!["https://example.com/a feed".into()];
        assert!(validate(&c).is_err());
        c.rss_feeds = vec!["https://example.com/feed.xml".into()];
        assert!(validate(&c).is_ok());
        c.rss_feeds = vec!["http://example.com/feed.xml".into()];
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn rss_feeds_require_a_real_parsed_host_not_just_a_prefix() {
        // 2026-07-17 review: a prefix check let "https://" and host-less
        // urls through to fail on every poll instead of at save time.
        // note "https:///x" is NOT a rejectable case: the whatwg parser
        // skips extra slashes after a special scheme and yields host "x".
        for junk in ["https://", "notaurl", "http://["] {
            let c = Config {
                rss_feeds: vec![junk.into()],
                ..Config::default()
            };
            assert!(validate(&c).is_err(), "{junk:?} must be rejected");
        }
    }

    #[test]
    fn empty_feed_list_rejected_only_while_rss_enabled() {
        let mut c = Config {
            rss_feeds: vec![],
            rss_enabled: true,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.rss_enabled = false;
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn exact_duplicate_feed_rejected() {
        let c = Config {
            rss_feeds: vec![
                "https://example.com/feed.xml".into(),
                "https://example.com/feed.xml".into(),
            ],
            ..Config::default()
        };
        let errors = validate(&c).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("duplicate rss feed")),
            "{errors:?}"
        );
    }

    #[test]
    fn trailing_slash_variant_duplicate_feed_rejected() {
        let c = Config {
            rss_feeds: vec![
                "https://example.com/feed.xml".into(),
                "https://example.com/feed.xml/".into(),
            ],
            ..Config::default()
        };
        let errors = validate(&c).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("duplicate rss feed")),
            "{errors:?}"
        );
    }

    #[test]
    fn genuinely_different_feeds_accepted() {
        let c = Config {
            rss_feeds: vec![
                "https://example.com/world.xml".into(),
                "https://example.com/tech.xml".into(),
            ],
            ..Config::default()
        };
        assert!(validate(&c).is_ok());
    }

    // --- mask ---

    #[test]
    fn long_values_mask_to_last_four() {
        assert_eq!(mask("sk-or-v1-abcda1b2"), "set (…a1b2)");
    }

    #[test]
    fn exactly_eight_chars_still_masks() {
        assert_eq!(mask("abcda1b2"), "set (…a1b2)");
    }

    #[test]
    fn short_values_never_leak_their_tail() {
        assert_eq!(mask("abcdefg"), "set");
        assert_eq!(mask("x"), "set");
    }

    // --- config round-trip: pins the Serialize derive against drift ---

    #[test]
    fn non_default_config_survives_serialize_then_parse() {
        let original = Config {
            port: 9999,
            default_ttl: 12,
            max_queued_per_tier: 7,
            start_paused: true,
            espn_enabled: false,
            espn_leagues: vec!["usa.1".into()],
            espn_poll_secs: 60,
            espn_priority: crate::event::Priority::Medium,
            espn_ttl_secs: 20,
            rss_enabled: true,
            rss_feeds: vec!["https://example.com/feed.xml".into()],
            rss_poll_secs: 120,
            rss_priority: crate::event::Priority::High,
            rss_ttl_secs: 15,
            rss_max_per_poll: 5,
            manual_default_priority: crate::event::Priority::Low,
            cmux_priority: crate::event::Priority::High,
            cmux_ttl_secs: 9,
            rotation_order: vec![
                crate::event::SourceKind::News,
                crate::event::SourceKind::Manual,
                crate::event::SourceKind::Cmux,
                crate::event::SourceKind::Football,
                crate::event::SourceKind::Weather,
            ],
            connectors: crate::config::Connectors {
                telegram: crate::config::TelegramToggle { enabled: true },
            },
            ..Config::default()
        };

        let serialized = toml::to_string_pretty(&original).unwrap();
        let reparsed = Config::parse(&serialized).unwrap();
        assert_eq!(original, reparsed);
    }

    // --- secrets merge (pure) ---

    #[test]
    fn setting_openrouter_preserves_telegram() {
        let mut doc = SecretsDoc {
            telegram: Some(TelegramTable {
                bot_token: Some("tok".into()),
                chat_id: Some("42".into()),
                ..Default::default()
            }),
            openrouter: None,
            ..Default::default()
        };
        merge_secret(&mut doc, SecretField::OpenrouterApiKey, "sk-or-key1".into());
        assert_eq!(
            doc.telegram.as_ref().unwrap().bot_token.as_deref(),
            Some("tok")
        );
        assert_eq!(
            doc.telegram.as_ref().unwrap().chat_id.as_deref(),
            Some("42")
        );
        assert_eq!(
            doc.openrouter.as_ref().unwrap().api_key.as_deref(),
            Some("sk-or-key1")
        );
    }

    #[test]
    fn setting_telegram_token_preserves_openrouter_and_chat_id() {
        let mut doc = SecretsDoc {
            telegram: Some(TelegramTable {
                bot_token: None,
                chat_id: Some("42".into()),
                ..Default::default()
            }),
            openrouter: Some(OpenRouterTable {
                api_key: Some("sk-or-key1".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        merge_secret(&mut doc, SecretField::TelegramBotToken, "newtok".into());
        assert_eq!(
            doc.telegram.as_ref().unwrap().bot_token.as_deref(),
            Some("newtok")
        );
        assert_eq!(
            doc.telegram.as_ref().unwrap().chat_id.as_deref(),
            Some("42")
        );
        assert_eq!(
            doc.openrouter.as_ref().unwrap().api_key.as_deref(),
            Some("sk-or-key1")
        );
    }

    #[test]
    fn secret_field_deserializes_exactly_three_names() {
        for (name, expected) in [
            ("\"openrouter_api_key\"", SecretField::OpenrouterApiKey),
            ("\"telegram_bot_token\"", SecretField::TelegramBotToken),
            ("\"telegram_chat_id\"", SecretField::TelegramChatId),
        ] {
            let parsed: SecretField = serde_json::from_str(name).unwrap();
            assert_eq!(parsed, expected);
        }
        assert!(serde_json::from_str::<SecretField>("\"detect_path\"").is_err());
    }

    #[test]
    fn secret_values_must_be_nonempty_and_whitespace_free() {
        assert!(validate_secret_value("").is_err());
        assert!(validate_secret_value("has space").is_err());
        assert!(validate_secret_value("has\ttab").is_err());
        assert!(validate_secret_value("sk-or-v1-fine").is_ok());
    }

    // --- status masking over the doc ---

    #[test]
    fn status_reports_per_field_masked_presence() {
        let doc = SecretsDoc {
            telegram: Some(TelegramTable {
                bot_token: Some("longtoken1234".into()),
                chat_id: None,
                ..Default::default()
            }),
            openrouter: None,
            ..Default::default()
        };
        let status = secret_status(&doc);
        assert_eq!(status.telegram_bot_token.as_deref(), Some("set (…1234)"));
        assert_eq!(status.telegram_chat_id, None);
        assert_eq!(status.openrouter_api_key, None);
    }

    // --- write paths (temp dirs, never $HOME) ---

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("notchtap-settings-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn config_write_is_atomic_parseable_and_creates_the_dir() {
        let dir = temp_dir(); // deliberately not created — the writer must
        let c = Config {
            port: 4242,
            ..Default::default()
        };
        write_config_atomic(&dir, &c).unwrap();

        let on_disk = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        let reparsed = Config::parse(&on_disk).unwrap();
        assert_eq!(reparsed.port, 4242);
        assert!(
            no_tmp_leftovers(&dir),
            "temp files must be gone after the rename"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn secrets_write_is_0600_and_loads_via_the_strict_loader() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        set_secret_in(&dir, SecretField::TelegramBotToken, "tok12345".into()).unwrap();
        set_secret_in(&dir, SecretField::TelegramChatId, "42".into()).unwrap();

        let path = dir.join("secrets.toml");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // the v3 connector's own loader accepts what the v5 writer wrote
        let secrets = crate::notifier::load_secrets(&path).unwrap();
        assert_eq!(secrets.bot_token, "tok12345");
        assert_eq!(secrets.chat_id, "42");
        assert!(no_tmp_leftovers(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn second_secret_write_preserves_the_first_on_disk() {
        let dir = temp_dir();
        set_secret_in(&dir, SecretField::TelegramBotToken, "tok12345".into()).unwrap();
        set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-key9".into()).unwrap();

        let doc = load_secrets_doc(&dir).unwrap();
        assert_eq!(doc.telegram.unwrap().bot_token.as_deref(), Some("tok12345"));
        assert_eq!(
            doc.openrouter.unwrap().api_key.as_deref(),
            Some("sk-or-key9")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_existing_secrets_error_and_are_never_clobbered() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secrets.toml");
        let garbage = "this is [not valid toml";
        std::fs::write(&path, garbage).unwrap();

        let err =
            set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-key9".into()).unwrap_err();
        assert!(err.contains("malformed"), "got: {err}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            garbage,
            "a malformed file must survive untouched, never be clobbered"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_secrets_file_yields_an_empty_doc() {
        let dir = temp_dir();
        assert_eq!(load_secrets_doc(&dir).unwrap(), SecretsDoc::default());
    }

    fn no_tmp_leftovers(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .all(|e| !e.file_name().to_string_lossy().contains(".tmp."))
            })
            .unwrap_or(true)
    }

    // --- 2026-07-17 review round: sentinel leak, unknown-key preservation,
    // --- trim, detect_path pinning, stale-tmp safety, label gate ---

    #[test]
    fn malformed_secrets_error_never_echoes_secret_material() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // malformed line that CONTAINS the secret — toml::de::Error's
        // Display would echo this source line if formatted
        std::fs::write(
            dir.join("secrets.toml"),
            "[telegram]\nbot_token = \"SENTINEL-hunter2",
        )
        .unwrap();

        let err = load_secrets_doc(&dir).unwrap_err();
        assert!(!err.contains("SENTINEL"), "leaked secret in: {err}");
        assert!(!err.contains("hunter2"), "leaked secret in: {err}");

        let err2 =
            set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-new1".into()).unwrap_err();
        assert!(!err2.contains("SENTINEL"), "leaked secret in: {err2}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_tables_and_fields_survive_a_secret_write() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("secrets.toml"),
            "[telegram]\nbot_token = \"tok12345\"\nwebhook = \"keep-me\"\n\n[future_service]\napi_key = \"also-keep-me\"\n",
        )
        .unwrap();

        set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-new1".into()).unwrap();

        let on_disk = std::fs::read_to_string(dir.join("secrets.toml")).unwrap();
        assert!(
            on_disk.contains("keep-me"),
            "unknown telegram field dropped:\n{on_disk}"
        );
        assert!(
            on_disk.contains("future_service"),
            "unknown table dropped:\n{on_disk}"
        );
        assert!(on_disk.contains("also-keep-me"));
        assert!(on_disk.contains("tok12345"));
        assert!(on_disk.contains("sk-or-new1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn secret_values_are_trimmed_before_validation_and_storage() {
        // the trailing newline every clipboard paste carries must not be
        // rejected as "contains whitespace" — and must not be stored
        let dir = temp_dir();
        set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-key9\n".into()).unwrap();
        let doc = load_secrets_doc(&dir).unwrap();
        assert_eq!(
            doc.openrouter.unwrap().api_key.as_deref(),
            Some("sk-or-key9")
        );
        // interior whitespace is still rejected
        assert!(set_secret_in(&dir, SecretField::OpenrouterApiKey, "bad key".into()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stale_permissive_tmp_files_are_never_written_into() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // attacker/leftover file at a plausible fixed temp name, world-readable
        let stale = dir.join("secrets.toml.tmp");
        std::fs::write(&stale, "old junk").unwrap();
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o644)).unwrap();

        set_secret_in(&dir, SecretField::OpenrouterApiKey, "sk-or-key9".into()).unwrap();

        // the stale file was never touched — no secret ever entered it
        assert_eq!(std::fs::read_to_string(&stale).unwrap(), "old junk");
        let mode = std::fs::metadata(dir.join("secrets.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_path_is_pinned_to_the_booted_value() {
        let booted = Config::default();
        let submitted = Config {
            detect_path: PathBuf::from("/tmp/evil-binary"),
            port: 9999,
            ..Config::default()
        };
        let pinned = pin_uneditable_fields(submitted, &booted);
        assert_eq!(
            pinned.detect_path, booted.detect_path,
            "detect_path must not be ipc-editable"
        );
        assert_eq!(pinned.port, 9999, "editable fields must pass through");
    }

    #[test]
    fn preflight_port_never_trips_when_the_submitted_port_is_unchanged() {
        // Bind an ephemeral port ourselves and make it BOTH the booted and
        // submitted value — this simulates the app's own listener already
        // holding the booted port. The `new != booted` guard must skip the
        // bind attempt entirely, so this must pass even though the port is
        // held right now.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(preflight_port(port, port).is_ok());
    }

    #[test]
    fn preflight_port_rejects_a_port_held_by_a_live_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let held_port = listener.local_addr().unwrap().port();
        // distinct booted port (ephemeral ports are always well above 1)
        // so the `new != booted` guard doesn't skip the check
        let booted_port = held_port - 1;
        let err = preflight_port(held_port, booted_port).unwrap_err();
        assert!(err.contains(held_port.to_string().as_str()), "{err:?}");
    }

    #[test]
    fn preflight_port_accepts_a_free_port() {
        // Bind-then-drop to get a free ephemeral port number, then confirm
        // preflight can still bind it (it dropped the listener already).
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let free_port = probe.local_addr().unwrap().port();
        drop(probe);
        assert!(preflight_port(free_port, free_port.wrapping_add(1)).is_ok());
    }

    #[test]
    fn ensure_settings_window_gates_on_the_window_label() {
        let app = tauri::test::mock_app();
        let settings = tauri::WebviewWindowBuilder::new(
            app.handle(),
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .build()
        .unwrap();
        assert!(ensure_settings_window(&settings).is_ok());

        let main = tauri::WebviewWindowBuilder::new(
            app.handle(),
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .build()
        .unwrap();
        assert!(
            ensure_settings_window(&main).is_err(),
            "the overlay window must be refused even if the acl were misconfigured"
        );
    }

    #[test]
    fn get_default_config_gates_on_window_label_and_returns_config_default() {
        let app = tauri::test::mock_app();
        let settings = tauri::WebviewWindowBuilder::new(
            app.handle(),
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .build()
        .unwrap();
        let returned = get_default_config(settings).unwrap();
        assert_eq!(returned, Config::default());

        let main = tauri::WebviewWindowBuilder::new(
            app.handle(),
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .build()
        .unwrap();
        assert!(
            get_default_config(main).is_err(),
            "the overlay window must be refused even if the acl were misconfigured"
        );
    }
}
