//! Settings window backend. Write paths are atomic (same-dir temp file + rename) because a
//! half-written `config.toml` is a bricked boot given `Config::load`'s fail-fast rule.

use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use chrono::Local;

use crate::config::{
    Appearance, Config, RestingState, CARD_OPACITY_RANGE, CARD_RADIUS_RANGE, CARD_SCALE_RANGE,
};
use crate::engine::Engine;
use crate::event::{
    AgentSignal, DetailItem, Event, EventMeta, EventPayload, EventSignal, EventType, RotationSpec,
    SourceKind,
};
use tauri::Manager;

fn feed_key(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            parsed.to_string().trim_end_matches('/').to_string()
        }
        Err(_) => url.trim().to_string(),
    }
}

/// SSRF guard: true for a literal loopback/link-local/private-network ip, `localhost`, or any
/// `.local` domain.
fn feed_host_is_internal(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_link_local() || v4.is_private(),
            std::net::IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => v4.is_loopback() || v4.is_link_local() || v4.is_private(),
                None => {
                    let first = v6.segments()[0];
                    v6.is_loopback() || (first & 0xffc0) == 0xfe80 || (first & 0xfe00) == 0xfc00
                }
            },
        };
    }
    let domain = host.trim_end_matches('.').to_ascii_lowercase();
    domain == "localhost" || domain.ends_with(".local")
}

/// Every rule violated contributes one human-readable message — the settings form renders the whole
/// list, not just the first failure.
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
    if !(1..=3600).contains(&c.agent_ttl_secs) {
        errors.push(format!(
            "agent_ttl_secs must be 1–3600 seconds (got {})",
            c.agent_ttl_secs
        ));
    }
    // A zero stale THRESHOLD is never meaningful — it marks every Agent Session Stale on the first
    // tick, including one that is actively Working, which empties the Agent Board with no error.
    if !(1..=86400).contains(&c.agents.stale_after_secs) {
        errors.push(format!(
            "agents.stale_after_secs must be 1–86400 seconds (got {}) — 0 marks every Agent Session Stale on the first board tick",
            c.agents.stale_after_secs
        ));
    }
    if !(0..=86400).contains(&c.agents.terminal_retention_secs) {
        errors.push(format!(
            "agents.terminal_retention_secs must be 0–86400 seconds (got {})",
            c.agents.terminal_retention_secs
        ));
    }
    if !(0..=86400).contains(&c.agents.stale_retention_secs) {
        errors.push(format!(
            "agents.stale_retention_secs must be 0–86400 seconds (got {})",
            c.agents.stale_retention_secs
        ));
    }
    for league in &c.espn_leagues {
        // leagues feed straight into an ESPN scoreboard url path segment (poller.rs) — beyond
        // "non-empty, no whitespace".
        let valid = !league.is_empty()
            && league
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if !valid {
            errors.push(format!(
                "league {league:?} is invalid — entries must be non-empty and match ^[A-Za-z0-9._-]+$ (letters, digits, '.', '_', '-' only)"
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
        let parsed = if feed.url.chars().any(char::is_whitespace) {
            None
        } else {
            reqwest::Url::parse(&feed.url).ok()
        };
        let scheme_and_host_ok = parsed
            .as_ref()
            .map(|u| (u.scheme() == "http" || u.scheme() == "https") && u.host_str().is_some())
            .unwrap_or(false);
        if !scheme_and_host_ok {
            errors.push(format!(
                "feed {:?} is invalid — entries must be full http(s) urls with a host and no whitespace",
                feed.url
            ));
        } else if let Some(parsed) = &parsed {
            // M2 SSRF guard: the rss poller fetches this url from the rust core itself
            // (server-side).
            if feed_host_is_internal(parsed) {
                errors.push(format!(
                    "feed {:?} is invalid — loopback, link-local, and private-network hosts are not allowed",
                    feed.url
                ));
            }
        }
    }
    if c.rss_enabled && c.rss_feeds.is_empty() {
        errors.push("rss_enabled is on but rss_feeds is empty — add a feed or disable".into());
    }

    {
        let mut seen_keys = std::collections::HashSet::new();
        for feed in &c.rss_feeds {
            if !seen_keys.insert(feed_key(&feed.url)) {
                errors.push(format!("duplicate rss feed: {}", feed.url));
            }
        }
    }

    // rotation_order must be a permutation of all four SourceKind variants — the ui is a fixed
    // 4-row reorder list, never add/remove, so any other shape means the ipc caller bypassed it.
    let expected_sources = [
        crate::event::SourceKind::Football,
        crate::event::SourceKind::Manual,
        crate::event::SourceKind::News,
        crate::event::SourceKind::Agent,
    ];
    let is_permutation = c.rotation_order.len() == expected_sources.len()
        && expected_sources
            .iter()
            .all(|source| c.rotation_order.contains(source));
    if !is_permutation {
        errors.push(
            "rotation_order must contain each of football, manual, news, and agent exactly once"
                .into(),
        );
    }

    if let Err(mut appearance_errors) = validate_appearance(&c.appearance) {
        errors.append(&mut appearance_errors);
    }

    // `prefix_shortcut`'s doc comment (config.rs) has the full rationale — this is the save-time
    // backstop, mirroring the frontend's own inline `isValidPrefixShortcut`
    if !is_valid_prefix_shortcut(&c.prefix_shortcut) {
        errors.push(format!(
            "prefix_shortcut must be \"⌃⇧\" followed by one more key name with no whitespace (got {:?})",
            c.prefix_shortcut
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_valid_prefix_shortcut(value: &str) -> bool {
    const PREFIX: &str = "⌃⇧";
    match value.strip_prefix(PREFIX) {
        Some(rest) => {
            let key_chars = rest.chars().count();
            (1..=24).contains(&key_chars) && !rest.chars().any(char::is_whitespace)
        }
        None => false,
    }
}

// ranges live in `config::CARD_*_RANGE` so this save-path check and `Config::parse`'s load-path
// self-heal can never drift apart.
pub fn validate_appearance(a: &Appearance) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if !CARD_SCALE_RANGE.contains(&a.card_scale) {
        errors.push(format!("card_scale must be 0.8–1.4 (got {})", a.card_scale));
    }
    if !CARD_RADIUS_RANGE.contains(&a.card_radius) {
        errors.push(format!(
            "card_radius must be 0.0–24.0 (got {})",
            a.card_radius
        ));
    }
    if !CARD_OPACITY_RANGE.contains(&a.card_opacity) {
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

// write paths (atomic; integration-tested against temp dirs, never $HOME) A fresh.
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
        let _ = std::fs::remove_file(tmp);
    }
    attempt
}

/// Create `dir` (config/history share `Config::dir_from_home`) and pin it to `0700`:
/// `create_dir_all` only applies the umask-derived default.
fn ensure_config_dir(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Serialize the whole config and atomically replace `config.toml` in `dir`. Same-dir temp file +
/// rename — rename across filesystems isn't atomic, and a torn `config.toml` is a bricked boot.
pub fn write_config_atomic(dir: &Path, config: &Config) -> anyhow::Result<()> {
    ensure_config_dir(dir)?;
    let serialized = toml::to_string_pretty(config)?;
    write_then_rename(
        &unique_tmp(dir, "config.toml"),
        &dir.join("config.toml"),
        &serialized,
        Some(0o600),
    )
}

fn notchtap_config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| Config::dir_from_home(&h))
        .ok_or_else(|| "could not determine home directory".to_string())
}

fn ensure_settings_window<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    if window.label() == "settings" {
        Ok(())
    } else {
        Err("settings commands are settings-window-only".to_string())
    }
}

/// IPC payload for `appearance-changed`: sent to the overlay whenever the user updates card
/// styling, or any other overlay-behavior field on the appearance channel.
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
                agent: None,
            },
            signal: EventSignal::Generic,
            origin: SourceKind::News,
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
        SourceKind::Agent => Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::AgentEvent,
            priority: config.agent_priority,
            rotation: RotationSpec::OneShot {
                ttl_secs: config.agent_ttl_secs,
            },
            topic: None,
            payload: EventPayload {
                title: "Test · agent event".into(),
                body: "This is how agent notifications look".into(),
            },
            meta: EventMeta {
                subtitle: Some("notchtap".into()),
                details: vec![DetailItem {
                    label: "Tool".into(),
                    value: "Bash".into(),
                }],
                agent: Some(AgentSignal {
                    runtime: "claude-code".into(),
                    kind: "completed".into(),
                    session_hash: "preview".into(),
                    summary: None,
                }),
                ..EventMeta::default()
            },
            signal: EventSignal::Generic,
            origin: SourceKind::Agent,
        },
    }
}

#[tauri::command]
pub fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    let config = state
        .inner()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(config)
}

/// Serves Config::default() so the frontend never mirrors defaults — the "Reset to defaults" source
/// of truth is config.rs.
#[tauri::command]
pub fn get_default_config<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    Ok(Config::default())
}

/// Pin it server-side to the booted value so the ipc surface enforces that rule regardless of what
/// the webview submits.
pub fn pin_uneditable_fields(mut submitted: Config, booted: &Config) -> Config {
    submitted.detect_path = booted.detect_path.clone();
    submitted
}

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

/// Validate → atomic write → relaunch.
#[tauri::command]
pub fn save_config_and_relaunch(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, StdMutex<Config>>,
    config: Config,
) -> Result<(), Vec<String>> {
    ensure_settings_window(&window).map_err(|e| vec![e])?;

    let mut managed = state.inner().lock().unwrap_or_else(|e| e.into_inner());
    let booted = managed.clone();
    let config = pin_uneditable_fields(config, &booted);
    validate(&config)?;
    preflight_port(config.port, booted.port).map_err(|e| vec![e])?;
    let dir = notchtap_config_dir().map_err(|e| vec![e])?;
    write_config_atomic(&dir, &config)
        .map_err(|e| vec![format!("could not write config.toml: {e}")])?;
    *managed = config;
    drop(managed);

    tracing::info!("config saved from settings window — relaunching");
    app.restart();
}

#[tauri::command]
pub async fn send_test_notification(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    engine: tauri::State<'_, Engine>,
    source: SourceKind,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let config = state
        .inner()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let event = build_test_event(&config, source);
    engine.accept(event, true).await.map_err(|e| e.to_string())
}

/// On-the-go news search: expands `query` via the SAME `rss_poller::expand_topic_url` a configured
/// topic line uses (one shared path, no fork), fetches it ONCE.
#[tauri::command]
pub async fn search_news_now(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    engine: tauri::State<'_, Engine>,
    seen: tauri::State<'_, StdMutex<crate::rss_poller::SeenStore>>,
    in_flight: tauri::State<'_, std::sync::atomic::AtomicBool>,
    query: String,
) -> Result<usize, String> {
    ensure_settings_window(&window)?;
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("query must not be empty".to_string());
    }

    if in_flight.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("already searching".to_string());
    }
    // Resets the flag on every exit path — the early-return error arms above included, since a
    // fetch error must not wedge the flag permanently "in flight".
    struct ResetInFlight<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for ResetInFlight<'_> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _reset = ResetInFlight(in_flight.inner());

    let (ttl_secs, max_per_poll, priority) = {
        let config = state.inner().lock().unwrap_or_else(|e| e.into_inner());
        (
            config.rss_ttl_secs,
            config.rss_max_per_poll,
            config.rss_priority,
        )
    };
    let url = crate::rss_poller::expand_topic_url(trimmed);
    let client = crate::net::build_poll_client().map_err(|e| e.to_string())?;
    let events = crate::rss_poller::search_once(
        &client,
        seen.inner(),
        &url,
        trimmed,
        max_per_poll,
        ttl_secs,
        priority,
    )
    .await
    .map_err(|e| e.to_string())?;

    let mut enqueued = 0usize;
    for event in events {
        if let Err(e) = engine.accept(event, false).await {
            tracing::warn!(query = %trimmed, "search_news_now event dropped: {e}");
        } else {
            enqueued += 1;
        }
    }
    Ok(enqueued)
}

#[tauri::command]
pub async fn get_recent_log_lines(window: tauri::WebviewWindow) -> Result<Vec<String>, String> {
    ensure_settings_window(&window)?;
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
    let mut managed = state.inner().lock().unwrap_or_else(|e| e.into_inner());
    let mut config = managed.clone();
    config.appearance = appearance.clone();
    write_config_atomic(&dir, &config).map_err(|e| format!("could not write config.toml: {e}"))?;
    managed.appearance = appearance;
    drop(managed);
    broadcast_appearance_change(&app, &config);
    Ok(())
}

// A fresh instance per call is a real race: `HistoryStore`'s serialization is an instance-level
// `Mutex`, so a second instance over the same file shares no lock with the engine's.
#[tauri::command]
pub async fn get_history(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<Vec<crate::history::HistoryEntry>, String> {
    ensure_settings_window(&window)?;
    match engine.history_store() {
        Some(store) => store.read_recent(200).map_err(|e| e.to_string()),
        None => {
            let dir = notchtap_config_dir()?;
            let store = crate::history::HistoryStore::new(dir).map_err(|e| e.to_string())?;
            store.read_recent(200).map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub async fn clear_history(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    match engine.history_store() {
        Some(store) => store.clear().map_err(|e| e.to_string()),
        None => {
            let dir = notchtap_config_dir()?;
            let store = crate::history::HistoryStore::new(dir).map_err(|e| e.to_string())?;
            store.clear().map_err(|e| e.to_string())
        }
    }
}

// Titles/bodies inside `QueueItemSummary` are UNTRUSTED wire data (same rule as History's
// link-as-literal-text precedent) — the frontend must render them as plain text only.
#[tauri::command]
pub async fn get_queue(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<Vec<crate::queue::QueueItemSummary>, String> {
    ensure_settings_window(&window)?;
    Ok(engine.read(|q| q.waiting_summaries()).await)
}

#[tauri::command]
pub async fn clear_queue(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<usize, String> {
    ensure_settings_window(&window)?;
    Ok(engine.apply(|q, _now| q.clear_waiting()).await)
}

#[tauri::command]
pub async fn skip_current(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    engine.apply(|q, now| q.skip_visible(now)).await;
    Ok(())
}

#[tauri::command]
pub async fn get_about_info(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    started_at: tauri::State<'_, std::time::Instant>,
) -> Result<crate::about::AboutInfo, String> {
    ensure_settings_window(&window)?;
    Ok(crate::about::gather_about_info(&app, *started_at.inner()))
}

/// the Agents section's four adapter cards read Adapter Health through this command — a live
/// [`crate::agents::health::HealthTracker::snapshot`] read.
#[tauri::command]
pub fn get_agent_health(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    health: tauri::State<'_, std::sync::Arc<crate::agents::health::HealthTracker>>,
) -> Result<Vec<crate::agents::board::AdapterHealthView>, String> {
    ensure_settings_window(&window)?;
    let runtimes = state
        .inner()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .agents
        .runtimes;
    let snapshot = health
        .inner()
        .snapshot(&runtimes, std::time::Instant::now());
    Ok(snapshot
        .iter()
        .map(crate::agents::board::health_to_view)
        .collect())
}

#[tauri::command]
pub async fn send_agent_test_event(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    registry: tauri::State<'_, crate::agents::registry::AgentRegistryHandle>,
    board: tauri::State<'_, crate::agents::board::AgentBoardPublisher>,
    engine: tauri::State<'_, Engine>,
    health: tauri::State<'_, std::sync::Arc<crate::agents::health::HealthTracker>>,
    runtime: String,
) -> Result<(), String> {
    ensure_settings_window(&window)?;

    const KNOWN_RUNTIMES: [&str; 4] = ["claude-code", "codex", "kimi", "opencode"];
    if !KNOWN_RUNTIMES.contains(&runtime.as_str()) {
        return Err(format!(
            "unknown runtime {runtime:?} — expected one of {}",
            KNOWN_RUNTIMES.join(", ")
        ));
    }

    let event_id = uuid::Uuid::new_v4().to_string();
    let session_id = format!("settings-test-{event_id}");
    let occurred_at_ms = chrono::Utc::now().timestamp_millis();
    let body = serde_json::json!({
        "schemaVersion": 1,
        "eventId": event_id,
        "runtime": runtime,
        "sessionId": session_id,
        "occurredAtMs": occurred_at_ms,
        "nativeEvent": "settings test event",
        "kind": "completed",
        "state": "completed",
        "summary": format!("Test event from Settings — {runtime} session completed"),
        "capabilities": ["session_lifecycle", "completion"],
        "terminal": true,
    })
    .to_string();

    let parsed = crate::agents::adapter::parse_wire_event(body.as_bytes())
        .map_err(|e| format!("could not build test event: {e}"))?;
    let event = parsed.event;
    let kind = event.kind;
    let terminal = event.terminal;
    let session_key = event.session_key.clone();
    let summary = event.summary.clone();
    let project_name = event.project.as_ref().and_then(|p| p.name.clone());
    let details = event.details.clone();

    health
        .inner()
        .record_accepted(session_key.runtime, occurred_at_ms);

    let now = std::time::Instant::now();
    registry.inner().apply_event(event, now).await;
    board.inner().publish_if_changed(now).await;

    let (agent_ttl_secs, policy) = {
        let config = state.inner().lock().unwrap_or_else(|e| e.into_inner());
        (
            config.agent_ttl_secs,
            crate::agents::notification::NotificationPolicy {
                informational_notifications: config.agents.informational_notifications,
                completion_notifications: config.agents.completion_notifications,
                permission_priority: config.agents.permission_priority,
                input_priority: config.agents.input_priority,
                failure_priority: config.agents.failure_priority,
                completion_priority: config.agents.completion_priority,
            },
        )
    };

    if let Some(notification) = crate::agents::notification::build_notification(
        &session_key,
        kind,
        terminal,
        crate::agents::notification::NotificationContent {
            summary: summary.as_deref(),
            project_name: project_name.as_deref(),
            details: &details,
        },
        agent_ttl_secs,
        &policy,
    ) {
        engine
            .inner()
            .accept(notification, true)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn default_config_validates_clean() {
        assert!(validate(&Config::default()).is_ok());
    }

    #[test]
    fn prefix_shortcut_accepts_the_shipped_default_and_the_existing_combo_family() {
        assert!(is_valid_prefix_shortcut("⌃⇧Space"));
        assert!(is_valid_prefix_shortcut("⌃⇧N"));
    }

    #[test]
    fn prefix_shortcut_rejects_missing_prefix_empty_key_and_embedded_whitespace() {
        assert!(!is_valid_prefix_shortcut("Space")); // no ⌃⇧ prefix at all
        assert!(!is_valid_prefix_shortcut("⌃⇧")); // prefix with no key name
        assert!(!is_valid_prefix_shortcut("")); // empty
        assert!(!is_valid_prefix_shortcut("⌃⇧ Space")); // whitespace right after the prefix
        assert!(!is_valid_prefix_shortcut("⌃⇧Sp ace")); // whitespace inside the key name
        assert!(!is_valid_prefix_shortcut("⇧⌃Space")); // glyphs in the wrong order
    }

    #[test]
    fn prefix_shortcut_whitespace_table_matches_the_ts_mirror() {
        assert!(is_valid_prefix_shortcut("⌃⇧K"));
        assert!(is_valid_prefix_shortcut("⌃⇧Space"));
        assert!(is_valid_prefix_shortcut("⌃⇧K\u{FEFF}"));
        assert!(is_valid_prefix_shortcut(&format!("⌃⇧{}", "K".repeat(24))));

        assert!(!is_valid_prefix_shortcut("⌃⇧K L"));
        assert!(!is_valid_prefix_shortcut("⌃⇧K\u{0085}"));
        assert!(!is_valid_prefix_shortcut("⌃⇧"));
        assert!(!is_valid_prefix_shortcut(&format!("⌃⇧{}", "K".repeat(25))));
    }

    #[test]
    fn prefix_shortcut_rejects_exactly_the_unicode_white_space_code_points() {
        for code in 0u32..=0xFFFF {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            assert_eq!(
                is_valid_prefix_shortcut(&format!("⌃⇧K{c}")),
                !c.is_whitespace(),
                "U+{code:04X} disagrees with char::is_whitespace"
            );
        }
    }

    #[test]
    fn prefix_shortcut_boundary_validates_through_the_whole_config() {
        let mut c = Config {
            prefix_shortcut: "not-a-combo".to_string(),
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.prefix_shortcut = "⌃⇧Space".to_string();
        assert!(validate(&c).is_ok());
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
    fn agent_ttl_boundaries() {
        let mut c = Config {
            agent_ttl_secs: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.agent_ttl_secs = 1;
        assert!(validate(&c).is_ok());
        c.agent_ttl_secs = 3600;
        assert!(validate(&c).is_ok());
        c.agent_ttl_secs = 3601;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn agents_stale_after_boundaries() {
        let mut c = Config::default();
        c.agents.stale_after_secs = 0;
        assert!(validate(&c).is_err());
        c.agents.stale_after_secs = 1;
        assert!(validate(&c).is_ok());
        c.agents.stale_after_secs = 86400;
        assert!(validate(&c).is_ok());
        c.agents.stale_after_secs = 86401;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn agents_retention_boundaries() {
        let mut c = Config::default();
        c.agents.terminal_retention_secs = 0;
        assert!(validate(&c).is_ok());
        c.agents.terminal_retention_secs = 86400;
        assert!(validate(&c).is_ok());
        c.agents.terminal_retention_secs = 86401;
        assert!(validate(&c).is_err());

        let mut c = Config::default();
        c.agents.stale_retention_secs = 0;
        assert!(validate(&c).is_ok());
        c.agents.stale_retention_secs = 86400;
        assert!(validate(&c).is_ok());
        c.agents.stale_retention_secs = 86401;
        assert!(validate(&c).is_err());
    }

    #[test]
    fn agents_duration_violations_are_all_reported_together() {
        let mut c = Config::default();
        c.agents.stale_after_secs = 0;
        c.agents.terminal_retention_secs = 86401;
        c.agents.stale_retention_secs = 86401;
        assert_eq!(validate(&c).unwrap_err().len(), 3);
    }

    #[test]
    fn rotation_order_must_be_a_permutation() {
        use crate::event::SourceKind;

        let mut c = Config {
            rotation_order: vec![SourceKind::Football, SourceKind::Manual],
            ..Config::default()
        };
        assert!(validate(&c).is_err());

        c.rotation_order = vec![
            SourceKind::Football,
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::Agent,
        ];
        assert!(validate(&c).is_err());

        c.rotation_order = vec![
            SourceKind::News,
            SourceKind::Football,
            SourceKind::Agent,
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
    fn league_entries_must_match_the_allowed_character_set() {
        for junk in ["../etc/passwd", "eng.1/../../x", "eng?1", "eng#1"] {
            let c = Config {
                espn_leagues: vec![junk.into()],
                ..Config::default()
            };
            assert!(validate(&c).is_err(), "{junk:?} must be rejected");
        }
        let c = Config {
            espn_leagues: vec!["usa_1-league.2".into()],
            ..Config::default()
        };
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

    #[test]
    fn appearance_changed_payload_carries_resting_state_from_config() {
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
        assert_eq!(json["resting_state"], serde_json::json!("notch"));
    }

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

    #[test]
    fn feed_urls_reject_loopback_link_local_and_private_hosts() {
        for internal in [
            "http://127.0.0.1/feed",
            "http://127.0.0.1:8080/feed",
            "http://169.254.169.254/x", // cloud metadata endpoint
            "http://10.0.0.5/feed",
            "http://172.16.0.1/feed",
            "http://192.168.1.1/feed",
            "http://localhost/feed",
            "http://LOCALHOST/feed",
            "http://printer.local/feed",
            "http://[::1]/feed",
        ] {
            let c = Config {
                rss_feeds: vec![internal.into()],
                ..Config::default()
            };
            assert!(validate(&c).is_err(), "{internal:?} must be rejected");
        }

        let c = Config {
            rss_feeds: vec!["https://example.com/feed".into()],
            ..Config::default()
        };
        assert!(validate(&c).is_ok());
    }

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
            agent_priority: crate::event::Priority::High,
            agent_ttl_secs: 9,
            rotation_order: vec![
                crate::event::SourceKind::News,
                crate::event::SourceKind::Manual,
                crate::event::SourceKind::Agent,
                crate::event::SourceKind::Football,
            ],
            ..Config::default()
        };

        let serialized = toml::to_string_pretty(&original).unwrap();
        let reparsed = Config::parse(&serialized).unwrap();
        assert_eq!(original, reparsed);
    }

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("notchtap-settings-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn config_write_is_atomic_parseable_and_creates_the_dir() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir(); // deliberately not created — the writer must
        let c = Config {
            port: 4242,
            ..Default::default()
        };
        write_config_atomic(&dir, &c).unwrap();

        let path = dir.join("config.toml");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        let reparsed = Config::parse(&on_disk).unwrap();
        assert_eq!(reparsed.port, 4242);

        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "config.toml must not be world-readable");
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "config dir must not be world-readable");

        assert!(
            no_tmp_leftovers(&dir),
            "temp files must be gone after the rename"
        );
        std::fs::remove_dir_all(&dir).ok();
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
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(preflight_port(port, port).is_ok());
    }

    #[test]
    fn preflight_port_rejects_a_port_held_by_a_live_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let held_port = listener.local_addr().unwrap().port();
        let booted_port = held_port - 1;
        let err = preflight_port(held_port, booted_port).unwrap_err();
        assert!(err.contains(held_port.to_string().as_str()), "{err:?}");
    }

    #[test]
    fn preflight_port_accepts_a_free_port() {
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

    #[test]
    fn build_test_event_football_uses_espn_config() {
        use crate::event::{EventType, Priority, RotationSpec, SourceKind};

        let config = Config {
            espn_priority: Priority::High,
            espn_ttl_secs: 42,
            rss_priority: Priority::Low,
            agent_priority: Priority::Low,
            manual_default_priority: Priority::Low,
            ..Config::default()
        };
        let event = build_test_event(&config, SourceKind::Football);
        assert_eq!(event.event_type, EventType::ScoreUpdate);
        assert_eq!(event.priority, Priority::High);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 42 });
        assert_eq!(event.origin, SourceKind::Football);
    }

    #[test]
    fn build_test_event_news_uses_rss_config() {
        use crate::event::{EventType, Priority, RotationSpec, SourceKind};

        let config = Config {
            rss_priority: Priority::Low,
            rss_ttl_secs: 17,
            espn_priority: Priority::High,
            agent_priority: Priority::High,
            manual_default_priority: Priority::High,
            ..Config::default()
        };
        let event = build_test_event(&config, SourceKind::News);
        assert_eq!(event.event_type, EventType::NewsItem);
        assert_eq!(event.priority, Priority::Low);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 17 });
        assert_eq!(event.origin, SourceKind::News);
        assert!(event.meta.espn.is_none());
    }

    #[test]
    fn build_test_event_agent_uses_agent_config() {
        use crate::event::{EventType, Priority, RotationSpec, SourceKind};

        let config = Config {
            agent_priority: Priority::High,
            agent_ttl_secs: 23,
            espn_priority: Priority::Low,
            rss_priority: Priority::Low,
            manual_default_priority: Priority::Low,
            ..Config::default()
        };
        let event = build_test_event(&config, SourceKind::Agent);
        assert_eq!(event.event_type, EventType::AgentEvent);
        assert_eq!(event.priority, Priority::High);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 23 });
        assert_eq!(event.origin, SourceKind::Agent);
    }

    #[test]
    fn build_test_event_manual_uses_default_ttl_and_manual_priority() {
        use crate::event::{EventType, Priority, RotationSpec, SourceKind};

        let config = Config {
            manual_default_priority: Priority::Low,
            default_ttl: 99,
            espn_priority: Priority::High,
            rss_priority: Priority::High,
            agent_priority: Priority::High,
            ..Config::default()
        };
        let event = build_test_event(&config, SourceKind::Manual);
        assert_eq!(event.event_type, EventType::Generic);
        assert_eq!(event.priority, Priority::Low);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 99 });
        assert_eq!(event.origin, SourceKind::Manual);
    }
}
