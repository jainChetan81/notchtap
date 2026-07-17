//! Outbound connectors (v3 spec). The `Notifier` seam is outbound-only:
//! the overlay is not an implementation, and a connector's outcome never
//! influences the http response. Fan-out happens at acceptance — after
//! `enqueue` returns Ok — via [`ConnectorHandle::offer`], which never
//! blocks (bounded channel, drop + warn when full).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::event::{Event, EventType};

pub const CHANNEL_CAP: usize = 64;
pub const RETRY_DELAY: Duration = Duration::from_secs(5);
pub const SEND_TIMEOUT: Duration = Duration::from_secs(10);
pub const TELEGRAM_API_BASE: &str = "https://api.telegram.org";

/// One spawned connector: a name for logs and the sending half of its
/// bounded channel. Cheap to clone into the http state.
#[derive(Clone)]
pub struct ConnectorHandle {
    name: &'static str,
    tx: mpsc::Sender<Event>,
}

impl ConnectorHandle {
    pub fn new(name: &'static str, tx: mpsc::Sender<Event>) -> Self {
        Self { name, tx }
    }

    /// Called at acceptance. Never blocks: on a full channel the event is
    /// dropped with a warning — bounded-and-non-blocking is the guarantee,
    /// not freshness (IMPLEMENTATION_PLAN.md §3).
    pub fn offer(&self, event: &Event) {
        match self.tx.try_send(event.clone()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(
                    connector = self.name,
                    "channel full — outbound event dropped"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::warn!(
                    connector = self.name,
                    "worker gone — outbound event dropped"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// retry policy (pure, unit-tested)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// network error / timeout / 5xx — worth one retry
    Transient,
    /// 400 — telegram rejected the formatting; resend once as plain text
    BadRequest,
    /// any other 4xx (401/403/404…) — config-level, retrying can't help
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    RetryAfter(Duration),
    ResendPlain,
    Drop,
}

/// v3 spec §2: at most one retry (transient) or one plain-text resend
/// (formatting rejection); any failure after that — or a fatal error at
/// any point — drops the event. `attempt` is 0-based. `retry_after` is
/// the delay a `RetryAfter` decision carries — the caller sleeps exactly
/// the carried value (2026-07-16 review: an earlier draft returned a
/// const here while the worker slept a different config value, so the
/// unit-tested number wasn't the one used).
pub fn on_send_failure(attempt: u32, kind: FailureKind, retry_after: Duration) -> RetryDecision {
    if attempt >= 1 {
        return RetryDecision::Drop;
    }
    match kind {
        FailureKind::Transient => RetryDecision::RetryAfter(retry_after),
        FailureKind::BadRequest => RetryDecision::ResendPlain,
        FailureKind::Fatal => RetryDecision::Drop,
    }
}

// ---------------------------------------------------------------------------
// message formatting (pure, unit-tested)
// ---------------------------------------------------------------------------

/// Telegram HTML mode needs exactly three escapes (v3 spec §3 — the whole
/// reason HTML mode beats MarkdownV2's 18-character surface). `&` first.
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Per-event-type templates — data-not-code, mirroring the frontend's
/// animation table. Title/body are escaped before substitution.
pub fn format_message(event: &Event) -> String {
    let title = escape_html(&event.payload.title);
    let body = escape_html(&event.payload.body);
    let icon = match event.event_type {
        EventType::ScoreUpdate => "⚽ ",
        EventType::MatchState => "🕐 ",
        EventType::Generic => "🤖 ",
    };
    format!("{icon}<b>{title}</b>\n{body}")
}

/// The plain-text fallback for a 400 resend: raw title/body, no html, no
/// escaping, `parse_mode` omitted entirely by the sender.
pub fn format_plain(event: &Event) -> String {
    format!("{}\n{}", event.payload.title, event.payload.body)
}

// ---------------------------------------------------------------------------
// secrets (v3 spec §4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramSecrets {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Deserialize)]
struct SecretsFile {
    // optional since v5: secrets.toml may hold other tables (e.g.
    // [openrouter]) without a [telegram] one — see V5_TECHNICAL_SPEC.md §4
    telegram: Option<TelegramSecrets>,
}

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("secrets file not found at {0}")]
    Missing(PathBuf),
    #[error("secrets file must be mode 0600, found {0:o} — refusing to use it")]
    BadPermissions(u32),
    #[error("secrets file unreadable: {0}")]
    Unreadable(std::io::Error),
    #[error("secrets file malformed: {0}")]
    Malformed(#[from] toml::de::Error),
    #[error("secrets file has no [telegram] table")]
    MissingTable,
}

pub fn default_secrets_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("notchtap").join("secrets.toml"))
}

/// Missing / wrong perms / malformed all disable the connector (the caller
/// warns and runs overlay-only) — same-build-everywhere: a machine without
/// secrets just has no outbound.
pub fn load_secrets(path: &Path) -> Result<TelegramSecrets, SecretsError> {
    use std::os::unix::fs::PermissionsExt;

    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SecretsError::Missing(path.to_path_buf()));
        }
        Err(e) => return Err(SecretsError::Unreadable(e)),
    };

    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(SecretsError::BadPermissions(mode));
    }

    let content = std::fs::read_to_string(path).map_err(SecretsError::Unreadable)?;
    let parsed: SecretsFile = toml::from_str(&content)?;
    parsed.telegram.ok_or(SecretsError::MissingTable)
}

// ---------------------------------------------------------------------------
// telegram worker (thin loop; the decisions it consults are tested above,
// the send path is covered by the wiremock tests below)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WorkerConfig {
    pub api_base: String,
    pub secrets: TelegramSecrets,
    pub retry_delay: Duration,
}

/// Builds the telegram connector: returns the handle for the http state
/// and the worker future for the caller to spawn on its runtime.
pub fn telegram_connector(
    secrets: TelegramSecrets,
    api_base: String,
    retry_delay: Duration,
) -> (
    ConnectorHandle,
    impl std::future::Future<Output = ()> + Send,
) {
    let (tx, rx) = mpsc::channel(CHANNEL_CAP);
    let cfg = WorkerConfig {
        api_base,
        secrets,
        retry_delay,
    };
    (ConnectorHandle::new("telegram", tx), run_worker(rx, cfg))
}

async fn run_worker(mut rx: mpsc::Receiver<Event>, cfg: WorkerConfig) {
    let client = match reqwest::Client::builder().timeout(SEND_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("telegram worker: cannot build http client: {e}");
            return;
        }
    };
    tracing::info!("telegram connector worker started");
    while let Some(event) = rx.recv().await {
        send_with_policy(&client, &cfg, &event).await;
    }
}

async fn send_with_policy(client: &reqwest::Client, cfg: &WorkerConfig, event: &Event) {
    let mut attempt: u32 = 0;
    let mut plain = false;
    loop {
        match send_once(client, cfg, event, plain).await {
            Ok(()) => return,
            Err(kind) => match on_send_failure(attempt, kind, cfg.retry_delay) {
                RetryDecision::RetryAfter(delay) => {
                    tracing::warn!(?kind, attempt, "telegram send failed — retrying");
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                RetryDecision::ResendPlain => {
                    tracing::warn!(attempt, "telegram rejected formatting — resending plain");
                    plain = true;
                    attempt += 1;
                }
                RetryDecision::Drop => {
                    tracing::warn!(?kind, attempt, title = %event.payload.title,
                        "telegram send dropped after failure");
                    return;
                }
            },
        }
    }
}

async fn send_once(
    client: &reqwest::Client,
    cfg: &WorkerConfig,
    event: &Event,
    plain: bool,
) -> Result<(), FailureKind> {
    let url = format!("{}/bot{}/sendMessage", cfg.api_base, cfg.secrets.bot_token);
    let mut body = serde_json::json!({
        "chat_id": cfg.secrets.chat_id,
        "text": if plain { format_plain(event) } else { format_message(event) },
    });
    if !plain {
        body["parse_mode"] = serde_json::json!("HTML");
    }

    let response = client.post(&url).json(&body).send().await.map_err(|e| {
        // timeouts, connect failures, dns — all transient per v3 spec §6
        tracing::debug!("telegram request error: {e}");
        FailureKind::Transient
    })?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else if status.is_server_error() {
        Err(FailureKind::Transient)
    } else if status == reqwest::StatusCode::BAD_REQUEST {
        Err(FailureKind::BadRequest)
    } else {
        Err(FailureKind::Fatal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventPayload, EventSignal, Priority, RotationSpec};
    use uuid::Uuid;

    fn event(event_type: EventType, title: &str, body: &str) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type,
            priority: Priority::Medium,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: body.to_string(),
            },
            signal: EventSignal::Generic,
        }
    }

    // --- retry policy: every arm ---

    #[test]
    fn first_transient_failure_retries_with_the_carried_delay() {
        // the decision carries the delay the caller passed in — the value
        // slept is the value tested (2026-07-16 review finding)
        let delay = Duration::from_millis(123);
        assert_eq!(
            on_send_failure(0, FailureKind::Transient, delay),
            RetryDecision::RetryAfter(delay)
        );
    }

    #[test]
    fn first_bad_request_resends_plain() {
        assert_eq!(
            on_send_failure(0, FailureKind::BadRequest, RETRY_DELAY),
            RetryDecision::ResendPlain
        );
    }

    #[test]
    fn fatal_drops_immediately() {
        assert_eq!(
            on_send_failure(0, FailureKind::Fatal, RETRY_DELAY),
            RetryDecision::Drop
        );
    }

    #[test]
    fn second_failure_always_drops() {
        for kind in [
            FailureKind::Transient,
            FailureKind::BadRequest,
            FailureKind::Fatal,
        ] {
            assert_eq!(on_send_failure(1, kind, RETRY_DELAY), RetryDecision::Drop);
            assert_eq!(on_send_failure(2, kind, RETRY_DELAY), RetryDecision::Drop);
        }
    }

    // --- formatting: per type + nasty characters ---

    #[test]
    fn score_update_uses_ball_template() {
        let msg = format_message(&event(EventType::ScoreUpdate, "GOAL", "1-0"));
        assert_eq!(msg, "⚽ <b>GOAL</b>\n1-0");
    }

    #[test]
    fn match_state_uses_clock_template() {
        let msg = format_message(&event(EventType::MatchState, "FT", "done"));
        assert_eq!(msg, "🕐 <b>FT</b>\ndone");
    }

    #[test]
    fn generic_uses_robot_template() {
        let msg = format_message(&event(EventType::Generic, "claude", "needs input"));
        assert_eq!(msg, "🤖 <b>claude</b>\nneeds input");
    }

    #[test]
    fn nasty_characters_are_escaped() {
        let msg = format_message(&event(
            EventType::Generic,
            "a <b>bold</b> claim",
            "x & y < z > w _under_ `tick`",
        ));
        // <, >, & escaped; markdown characters left alone (html mode)
        assert_eq!(
            msg,
            "🤖 <b>a &lt;b&gt;bold&lt;/b&gt; claim</b>\nx &amp; y &lt; z &gt; w _under_ `tick`"
        );
    }

    #[test]
    fn ampersand_is_escaped_first_not_double_escaped() {
        assert_eq!(escape_html("&lt;"), "&amp;lt;");
    }

    #[test]
    fn plain_fallback_is_unescaped_raw_text() {
        let msg = format_plain(&event(EventType::Generic, "a <b> & c", "body"));
        assert_eq!(msg, "a <b> & c\nbody");
    }

    // --- offer: drop-on-full, never blocks ---

    #[tokio::test]
    async fn offer_drops_when_channel_full_without_blocking() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = ConnectorHandle::new("test", tx);
        handle.offer(&event(EventType::Generic, "first", "kept"));
        handle.offer(&event(EventType::Generic, "second", "dropped"));

        let received = rx.try_recv().unwrap();
        assert_eq!(received.payload.title, "first");
        assert!(rx.try_recv().is_err()); // second was dropped, not queued
    }

    // --- secrets loading (temp files, never $HOME) ---

    fn temp_secrets(content: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("notchtap-test-{}.toml", Uuid::new_v4()));
        std::fs::write(&path, content).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    const VALID_SECRETS: &str = "[telegram]\nbot_token = \"tok\"\nchat_id = \"42\"\n";

    #[test]
    fn valid_0600_secrets_load() {
        let path = temp_secrets(VALID_SECRETS, 0o600);
        let secrets = load_secrets(&path).unwrap();
        assert_eq!(secrets.bot_token, "tok");
        assert_eq!(secrets.chat_id, "42");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_is_a_missing_error() {
        let err = load_secrets(Path::new("/nonexistent/notchtap-secrets.toml")).unwrap_err();
        assert!(matches!(err, SecretsError::Missing(_)));
    }

    #[test]
    fn world_readable_secrets_are_refused() {
        let path = temp_secrets(VALID_SECRETS, 0o644);
        let err = load_secrets(&path).unwrap_err();
        assert!(matches!(err, SecretsError::BadPermissions(0o644)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn malformed_secrets_are_an_error() {
        let path = temp_secrets("[telegram]\nbot_token = 12\n", 0o600);
        let err = load_secrets(&path).unwrap_err();
        assert!(matches!(err, SecretsError::Malformed(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn openrouter_only_file_is_missing_table_not_malformed() {
        // v5: secrets.toml may legitimately hold only an [openrouter]
        // table — the telegram connector disables itself cleanly
        let path = temp_secrets("[openrouter]\napi_key = \"sk-or-x\"\n", 0o600);
        let err = load_secrets(&path).unwrap_err();
        assert!(matches!(err, SecretsError::MissingTable));
        std::fs::remove_file(&path).ok();
    }

    // --- wiremock: the send path. no live telegram call, ever. ---

    mod send_path {
        use super::*;
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn test_cfg(base: String) -> WorkerConfig {
            WorkerConfig {
                api_base: base,
                secrets: TelegramSecrets {
                    bot_token: "tok".to_string(),
                    chat_id: "42".to_string(),
                },
                retry_delay: Duration::from_millis(10),
            }
        }

        fn client() -> reqwest::Client {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap()
        }

        #[tokio::test]
        async fn success_sends_html_message_once() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/bottok/sendMessage"))
                .and(body_partial_json(serde_json::json!({
                    "chat_id": "42",
                    "parse_mode": "HTML",
                })))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;

            let cfg = test_cfg(server.uri());
            send_with_policy(
                &client(),
                &cfg,
                &event(EventType::ScoreUpdate, "GOAL", "1-0"),
            )
            .await;
            // .expect(1) verifies on drop: exactly one request, html mode
        }

        #[tokio::test]
        async fn bad_request_resends_exactly_once_as_plain() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/bottok/sendMessage"))
                .respond_with(ResponseTemplate::new(400))
                .up_to_n_times(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/bottok/sendMessage"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;

            let cfg = test_cfg(server.uri());
            send_with_policy(&client(), &cfg, &event(EventType::Generic, "t", "b")).await;

            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 2);
            let second: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
            assert!(
                second.get("parse_mode").is_none(),
                "plain resend must omit parse_mode"
            );
            assert_eq!(second["text"], "t\nb");
        }

        #[tokio::test]
        async fn server_error_retries_exactly_once_then_drops() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/bottok/sendMessage"))
                .respond_with(ResponseTemplate::new(500))
                .expect(2) // initial + exactly one retry, then drop
                .mount(&server)
                .await;

            let cfg = test_cfg(server.uri());
            send_with_policy(&client(), &cfg, &event(EventType::Generic, "t", "b")).await;
        }

        #[tokio::test]
        async fn unauthorized_drops_without_retry() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/bottok/sendMessage"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&server)
                .await;

            let cfg = test_cfg(server.uri());
            send_with_policy(&client(), &cfg, &event(EventType::Generic, "t", "b")).await;
        }
    }
}
