//! Loopback event delivery with a 750 ms timeout and bounded fail-open results.

use std::time::Duration;

use serde_json::Value;

/// Must match `config.rs::default_port` and the `notchtap` CLI's `${NOTCHTAP_PORT:-9789}` fallback.
pub const DEFAULT_PORT: u16 = 9789;

/// Bounds the whole request (connect + send + read) via `reqwest`'s per-request `.timeout()`.
pub const DELIVERY_TIMEOUT: Duration = Duration::from_millis(750);

/// Falls back to [`DEFAULT_PORT`] when `NOTCHTAP_PORT` is absent or invalid.
pub fn resolve_port() -> u16 {
    std::env::var("NOTCHTAP_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Every reqwest error path is caught and turned into [`Failed`], never a panic.
#[derive(Debug)]
pub enum DeliveryOutcome {
    Delivered,
    Failed(String),
}

/// Posts `body` to `http://127.0.0.1:{port}/agent/events`. Keep the URL a loopback literal, never a
/// hostname — `http.rs`'s `check_loopback_host` requires a loopback-literal `Host` header.
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


    #[tokio::test]
    async fn deliver_to_an_unreachable_port_fails_open() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let outcome = deliver(serde_json::json!({"schemaVersion": 1}), port).await;
        assert!(matches!(outcome, DeliveryOutcome::Failed(_)));
    }
}
