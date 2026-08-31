use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_CREST_BYTES: usize = 256 * 1024;

/// A crest URL is fetchable only if it is https and points at ESPN's own CDN — the URL arrives from
/// a network feed, so it is untrusted input.
pub fn crest_url_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    match parsed.host_str() {
        Some(host) => host == "espncdn.com" || host.ends_with(".espncdn.com"),
        None => false,
    }
}

#[derive(Clone)]
pub struct CrestCache {
    pub(crate) dir: PathBuf,
    attempted: Arc<Mutex<HashSet<String>>>,
}

impl CrestCache {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            attempted: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Defensive sanitization of the team id used in the cache filename — ESPN ids are always plain
    /// numeric strings in every checked-in fixture, but this is untrusted feed input.
    pub(crate) fn path_for(&self, team_id: &str) -> PathBuf {
        let safe: String = team_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        self.dir.join(format!("{safe}.png"))
    }

    /// A cache hit's on-disk path, if this team's crest is already cached — pure filesystem check,
    /// no network.
    pub fn cached_path(&self, team_id: &str) -> Option<PathBuf> {
        if team_id.is_empty() {
            return None;
        }
        let path = self.path_for(team_id);
        path.exists().then_some(path)
    }

    /// [`cached_path`] as a wire-ready `String` (the shape `EspnMeta.home_crest`/ `away_crest`
    /// carry) — a raw absolute filesystem path.
    pub fn cached_path_string(&self, team_id: &str) -> Option<String> {
        self.cached_path(team_id)
            .map(|p| p.to_string_lossy().into_owned())
    }

    /// Returns `true` exactly once per team per process lifetime (the first time this team is asked
    /// about and it isn't already cached on disk).
    pub fn should_fetch(&self, team_id: &str) -> bool {
        if team_id.is_empty() || self.cached_path(team_id).is_some() {
            return false;
        }
        let mut attempted = self.attempted.lock().unwrap_or_else(|e| e.into_inner());
        attempted.insert(team_id.to_string())
    }

    /// Fetch a team's crest and store it under the cache dir. Never poller-fatal: every failure
    /// (network, non-2xx, oversized body, filesystem) is logged and swallowed.
    pub async fn fetch_and_store(&self, client: &reqwest::Client, team_id: &str, url: &str) {
        if let Err(e) = self.try_fetch(client, team_id, url).await {
            tracing::warn!(team_id, url, "crest fetch failed: {e}");
        }
    }

    async fn try_fetch(
        &self,
        _client: &reqwest::Client,
        team_id: &str,
        url: &str,
    ) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let client = crest_client()?;
        let response = client.get(url).send().await?.error_for_status()?;
        let bytes = crate::net::read_body_capped(response, MAX_CREST_BYTES).await?;
        write_atomic(&self.path_for(team_id), &bytes)
    }
}

/// A client dedicated to crest fetches, with a redirect policy stricter than the shared poll
/// client's (`net::build_poll_client`): every hop — not just the initial URL.
fn crest_client() -> reqwest::Result<reqwest::Client> {
    crate::net::client_builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 3 {
                return attempt.error("too many redirects");
            }
            let url = attempt.url();
            if crate::net::host_is_blocked(url) || !crest_url_allowed(url.as_str()) {
                return attempt.error("redirect target is not an allowed espncdn host");
            }
            attempt.follow()
        }))
        .build()
}

/// Same-dir temp-file + rename atomic write (matches `settings.rs`'s config/secrets write posture).
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("png.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::CrestCache;
    use std::path::PathBuf;

    pub(crate) struct TempCacheDir(PathBuf);
    impl Drop for TempCacheDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn temp_cache() -> (TempCacheDir, CrestCache) {
        let dir =
            std::env::temp_dir().join(format!("notchtap-crest-test-{}", uuid::Uuid::new_v4()));
        let cache = CrestCache::new(dir.clone());
        (TempCacheDir(dir), cache)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::temp_cache;
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn cache_miss_reports_none_and_schedules_a_fetch() {
        let (_dir, cache) = temp_cache();
        assert_eq!(cache.cached_path("160"), None);
        assert_eq!(cache.cached_path_string("160"), None);
        assert!(
            cache.should_fetch("160"),
            "a never-seen team must be scheduled for fetch"
        );
    }

    #[test]
    fn empty_team_id_is_never_scheduled() {
        let (_dir, cache) = temp_cache();
        assert!(!cache.should_fetch(""));
        assert_eq!(cache.cached_path(""), None);
    }

    #[test]
    fn second_call_for_the_same_team_this_run_is_not_rescheduled() {
        let (_dir, cache) = temp_cache();
        assert!(cache.should_fetch("160"));
        assert!(
            !cache.should_fetch("160"),
            "one fetch attempt per team per process lifetime"
        );
    }

    #[test]
    fn a_warm_cache_hit_on_disk_is_never_rescheduled() {
        let (_dir, cache) = temp_cache();
        std::fs::create_dir_all(&cache.dir).unwrap();
        std::fs::write(cache.path_for("160"), b"not a real png, just bytes").unwrap();

        assert!(cache.cached_path("160").is_some());
        assert!(
            !cache.should_fetch("160"),
            "a cache hit (warm restart) must never schedule a refetch"
        );
    }

    #[tokio::test]
    async fn successful_fetch_writes_the_file_and_becomes_a_cache_hit() {
        let (_dir, cache) = temp_cache();
        let server = MockServer::start().await;
        let png_bytes = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        Mock::given(method("GET"))
            .and(path("/160.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes.clone()))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let url = format!("{}/160.png", server.uri());
        cache.fetch_and_store(&client, "160", &url).await;

        let cached = cache.cached_path("160").expect("fetch should have cached");
        assert_eq!(std::fs::read(cached).unwrap(), png_bytes);
        assert!(!cache.should_fetch("160"), "now a cache hit");
    }

    #[tokio::test]
    async fn failed_fetch_leaves_no_cache_entry_and_never_panics() {
        let (_dir, cache) = temp_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/404.png"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let url = format!("{}/404.png", server.uri());
        cache.fetch_and_store(&client, "999", &url).await;

        assert_eq!(cache.cached_path("999"), None);
    }

    #[tokio::test]
    async fn redirect_to_a_non_espncdn_host_is_rejected_and_leaves_no_cache_entry() {
        let (_dir, cache) = temp_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/redirect.png"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("Location", "https://evil.com/x.png"),
            )
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let url = format!("{}/redirect.png", server.uri());
        cache.fetch_and_store(&client, "222", &url).await;

        assert_eq!(cache.cached_path("222"), None);
    }

    #[tokio::test]
    async fn redirect_to_a_loopback_or_private_host_is_rejected() {
        let (_dir, cache) = temp_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/redirect.png"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "http://169.254.169.254/latest/meta-data"),
            )
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let url = format!("{}/redirect.png", server.uri());
        cache.fetch_and_store(&client, "223", &url).await;

        assert_eq!(cache.cached_path("223"), None);
    }

    #[tokio::test]
    async fn oversized_body_is_rejected_and_leaves_no_cache_entry() {
        let (_dir, cache) = temp_cache();
        let server = MockServer::start().await;
        let oversized = vec![0u8; MAX_CREST_BYTES + 100];
        Mock::given(method("GET"))
            .and(path("/big.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(oversized))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let url = format!("{}/big.png", server.uri());
        cache.fetch_and_store(&client, "111", &url).await;

        assert_eq!(cache.cached_path("111"), None);
    }

    #[test]
    fn crest_url_allowed_accepts_espncdn_https_hosts() {
        assert!(crest_url_allowed(
            "https://a.espncdn.com/i/teamlogos/soccer/500/360.png"
        ));
        assert!(crest_url_allowed("https://espncdn.com/x.png"));
    }

    #[test]
    fn crest_url_allowed_rejects_non_https_and_non_espncdn_hosts() {
        assert!(
            !crest_url_allowed("http://a.espncdn.com/x.png"),
            "non-https scheme"
        );
        assert!(!crest_url_allowed("https://evil.com/x.png"), "wrong host");
        assert!(
            !crest_url_allowed("https://espncdn.com.evil.com/x.png"),
            "suffix-spoofed host must not pass the ends_with check"
        );
        assert!(
            !crest_url_allowed("https://127.0.0.1/x.png"),
            "raw ip is not the espncdn host"
        );
        assert!(!crest_url_allowed("not a url"), "unparseable url");
    }

    #[test]
    fn team_id_is_sanitized_in_the_cache_filename() {
        let (_dir, cache) = temp_cache();
        let path = cache.path_for("../../etc/passwd");
        assert_eq!(path.file_name().unwrap(), "etcpasswd.png");
        assert_eq!(path.parent().unwrap(), cache.dir);
    }
}
