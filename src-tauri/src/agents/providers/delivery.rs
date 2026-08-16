//! Impure half of the hook helper — posts an already-built schema-v1
//! body to loopback `POST /agent/events`. Timeout at most 750 ms total;
//! fail open: every failure becomes a bounded [`String`] reason, never
//! a panic/exit — the caller (`src/bin/notchtap_agent.rs`) logs it and
//! always exits 0. `NOTCHTAP_PORT` overrides the default 9789, which is
//! deliberately duplicated from `config.rs::default_port` and the
//! `notchtap` CLI script (neither module is `pub` from here).

use std::time::Duration;

use serde_json::Value;

/// Must match `config.rs::default_port` and the `notchtap` CLI's
/// `${NOTCHTAP_PORT:-9789}` fallback.
pub const DEFAULT_PORT: u16 = 9789;

/// Bounds the whole request (connect + send + read) via `reqwest`'s
/// per-request `.timeout()`.
pub const DELIVERY_TIMEOUT: Duration = Duration::from_millis(750);

/// Resolves the target port: `$NOTCHTAP_PORT` if parseable as `u16`,
/// else [`DEFAULT_PORT`]. An unparseable value silently falls back —
/// this helper is fail-open end to end, including config resolution.
pub fn resolve_port() -> u16 {
    std::env::var("NOTCHTAP_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// What happened when a body was posted. Every reqwest error path is
/// caught and turned into [`Failed`], never a panic.
///
/// [`Failed`]: DeliveryOutcome::Failed
#[derive(Debug)]
pub enum DeliveryOutcome {
    Delivered,
    /// A bounded, human-readable reason, logged by the caller via
    /// `diagnostics::log_diagnostic`.
    Failed(String),
}

/// Posts `body` to `http://127.0.0.1:{port}/agent/events`. Keep the URL
/// a loopback literal, never a hostname — `http.rs`'s
/// `check_loopback_host` requires a loopback-literal `Host` header.
pub async fn deliver(body: Value, port: u16) -> DeliveryOutcome {
    let client = match reqwest::Client::builder().timeout(DELIVERY_TIMEOUT).build() {
        Ok(client) => client,
        Err(e) => return DeliveryOutcome::Failed(format!("client build failed: {e}")),
    };

    let url = format!("http://127.0.0.1:{port}/agent/events");
    match client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                DeliveryOutcome::Delivered
            } else {
                DeliveryOutcome::Failed(format!("http {status}"))
            }
        }
        Err(e) => DeliveryOutcome::Failed(format!("request failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn default_port_matches_the_repo_wide_9789_default() {
        assert_eq!(DEFAULT_PORT, 9789);
    }

    #[test]
    fn resolve_port_falls_back_to_default_when_unset() {
        // Never mutate `NOTCHTAP_PORT` here — tests in this crate share
        // one process, so env writes race other tests.
        if std::env::var("NOTCHTAP_PORT").is_err() {
            assert_eq!(resolve_port(), DEFAULT_PORT);
        }
    }

    #[tokio::test]
    async fn deliver_to_an_unreachable_port_fails_open() {
        // Bind then immediately drop a listener to get a genuinely free
        // port with nothing behind it for the actual POST.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let outcome = deliver(serde_json::json!({"schemaVersion": 1}), port).await;
        assert!(matches!(outcome, DeliveryOutcome::Failed(_)));
    }
}
