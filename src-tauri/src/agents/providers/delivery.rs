//! Plan 138 (spec §4.1): the impure half of the hook helper — posting an
//! already-built schema-v1 body to the loopback `POST /agent/events`
//! endpoint under the delivery rules:
//!
//! - connect+read timeout at most 750 ms total;
//! - fail open: every failure mode (no listener, timeout, non-2xx,
//!   client-build failure) is reported back as a bounded [`String`]
//!   reason, never a panic/exit — the caller (`src/bin/notchtap_agent.rs`)
//!   is the one that decides to log it via `diagnostics::log_diagnostic`
//!   and always exits 0 regardless;
//! - `NOTCHTAP_PORT` remains the explicit port override, default 9789
//!   (matches `config.rs::default_port` and the `notchtap` CLI script's
//!   own `${NOTCHTAP_PORT:-9789}` resolution — this is the third place
//!   that same default/override pair is now written, by design: this
//!   binary can't reach `config.rs` any more than it can reach
//!   `http.rs`, since neither is `pub`, and duplicating one `u16`
//!   literal is a smaller risk than widening those modules' visibility
//!   for it).

use std::time::Duration;

use serde_json::Value;

/// Matches `config.rs::default_port` / the `notchtap` CLI's own
/// `${NOTCHTAP_PORT:-9789}` fallback — see this module's top doc for why
/// it's redefined here rather than imported.
pub const DEFAULT_PORT: u16 = 9789;

/// Spec §4.1: "connect/read timeout at most 750 ms". `reqwest`'s
/// per-request `.timeout()` bounds the whole request (connect + send +
/// read), which is a tighter guarantee than the spec's "at most" floor
/// requires.
pub const DELIVERY_TIMEOUT: Duration = Duration::from_millis(750);

/// Resolves the target port: `$NOTCHTAP_PORT` (any valid `u16`) if set
/// and parseable, else [`DEFAULT_PORT`]. An unparseable `NOTCHTAP_PORT`
/// (empty, non-numeric, out of `u16` range) silently falls back rather
/// than erroring — this helper is fail-open end to end, including its
/// own config resolution.
pub fn resolve_port() -> u16 {
    std::env::var("NOTCHTAP_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// What happened when a body was posted. Never constructed from a panic
/// — every reqwest error path is caught and turned into [`Failed`].
///
/// [`Failed`]: DeliveryOutcome::Failed
#[derive(Debug)]
pub enum DeliveryOutcome {
    Delivered,
    /// A bounded, human-readable reason — the caller logs this via
    /// `diagnostics::log_diagnostic`, so it's already short by
    /// construction (reqwest error `Display` impls and an HTTP status
    /// code are both naturally short).
    Failed(String),
}

/// Posts `body` to `http://127.0.0.1:{port}/agent/events`. Loopback
/// literal by construction (never a hostname) — `http.rs`'s
/// `check_loopback_host` requires the `Host` header be a loopback
/// literal, which `reqwest` derives correctly from this URL on its own.
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
        // Doesn't touch `NOTCHTAP_PORT` — assumes the test env doesn't
        // already export a valid override (matches this repo's existing
        // env-var test posture; no other test in this crate mutates
        // `NOTCHTAP_PORT`).
        if std::env::var("NOTCHTAP_PORT").is_err() {
            assert_eq!(resolve_port(), DEFAULT_PORT);
        }
    }

    // fail-open: nothing is listening on this ephemeral port, deliver()
    // must resolve to `Failed(..)`, never panic/hang past the timeout.
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
