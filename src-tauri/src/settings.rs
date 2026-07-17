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

use serde::{Deserialize, Serialize};

use crate::config::Config;

// ---------------------------------------------------------------------------
// validation (pure, unit-tested — spec §3)
// ---------------------------------------------------------------------------

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

/// Returns the **booted** config (managed state) — "what is running",
/// which save-and-relaunch makes true of the file again after every save.
#[tauri::command]
pub fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Config>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    Ok(state.inner().clone())
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

/// Validate → atomic write → relaunch. The `Err` arm carries the whole
/// per-field message list for the form; on success the process is gone
/// before a reply could matter.
#[tauri::command]
pub fn save_config_and_relaunch(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Config>,
    config: Config,
) -> Result<(), Vec<String>> {
    ensure_settings_window(&window).map_err(|e| vec![e])?;
    let config = pin_uneditable_fields(config, state.inner());
    validate(&config)?;
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
            rss_enabled: true,
            rss_feeds: vec!["https://example.com/feed.xml".into()],
            rss_poll_secs: 120,
            rss_ttl_secs: 15,
            rss_max_per_poll: 5,
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
        let mut c = Config::default();
        c.port = 4242;
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
}
