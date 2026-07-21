//! plan 083 workstream a: club crest fetch + on-disk cache.
//!
//! **Legal/scope rule (hard, not a style preference)**: crest PNGs are
//! runtime-cached by rust, NEVER committed to git — trademarked club
//! artwork, materially lower-risk as a fetched-at-runtime cache of a
//! feed-provided URL than as a vendored asset
//! (`docs/ARCHITECTURE.md`'s config/logging paths section documents this
//! cache dir's location and lifecycle). `.gitignore` has no rule for this
//! dir specifically because the dir is never created inside the repo
//! tree — it lives under `~/.config/notchtap/crests/`, alongside
//! `config.toml`/`secrets.toml` (`Config::dir_from_home`).
//!
//! Cache policy (deliberately v1-minimal, per plan 083's scope):
//! - fetch on cache miss only; a cache hit (on-disk PNG, which persists
//!   across restarts) never re-fetches.
//! - one fetch ATTEMPT per team per process lifetime — `should_fetch`
//!   marks a team "attempted" the first time it's asked about, whether
//!   the fetch that follows succeeds or fails, so a failing team isn't
//!   retried every poll; a restart clears the attempted-set and tries
//!   again.
//! - failures are silent-with-fallback (the caller sees `None` and the
//!   frontend renders the text-abbrev fallback) — never poller-fatal.
//! - no eviction: ESPN's watched leagues bound the team count to a
//!   couple dozen, trivial by any cache's standard.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Generous for a club crest PNG — real-world ESPN crests are a few KB.
const MAX_CREST_BYTES: usize = 256 * 1024;

/// Runtime cache of club crest PNGs, keyed by ESPN's numeric team id.
/// Cheaply `Clone`-able (an `Arc` around the shared attempted-set) so the
/// espn poller can hand a handle to each spawned background fetch task.
#[derive(Clone)]
pub struct CrestCache {
    pub(crate) dir: PathBuf,
    attempted: Arc<Mutex<HashSet<String>>>,
}

impl CrestCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            attempted: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Defensive sanitization of the team id used in the cache filename —
    /// ESPN ids are always plain numeric strings in every checked-in
    /// fixture, but this is untrusted feed input, not a trusted
    /// filesystem path, so it's filtered rather than trusted verbatim.
    pub(crate) fn path_for(&self, team_id: &str) -> PathBuf {
        let safe: String = team_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        self.dir.join(format!("{safe}.png"))
    }

    /// A cache hit's on-disk path, if this team's crest is already
    /// cached — pure filesystem check, no network, so this is safe to
    /// call from anywhere (including the pure/fixture-tested
    /// `diff_scoreboard`'s caller in the poll loop) and easy to test
    /// with a temp dir.
    pub fn cached_path(&self, team_id: &str) -> Option<PathBuf> {
        if team_id.is_empty() {
            return None;
        }
        let path = self.path_for(team_id);
        path.exists().then_some(path)
    }

    /// [`cached_path`] as a wire-ready `String` (the shape `EspnMeta.home_crest`/
    /// `away_crest` carry) — a raw absolute filesystem path, NOT yet a
    /// servable `asset://`/`crest://` URL (the frontend converts it via
    /// `convertFileSrc`, keeping this cache serving-route-agnostic).
    pub fn cached_path_string(&self, team_id: &str) -> Option<String> {
        self.cached_path(team_id)
            .map(|p| p.to_string_lossy().into_owned())
    }

    /// Returns `true` exactly once per team per process lifetime (the
    /// first time this team is asked about and it isn't already cached
    /// on disk) — the caller should schedule a fetch only when this
    /// returns `true`. Marks the team attempted immediately (before the
    /// fetch even starts) so two overlapping calls for the same team
    /// within one poll can't both schedule a fetch.
    pub fn should_fetch(&self, team_id: &str) -> bool {
        if team_id.is_empty() || self.cached_path(team_id).is_some() {
            return false;
        }
        let mut attempted = self.attempted.lock().unwrap_or_else(|e| e.into_inner());
        attempted.insert(team_id.to_string())
    }

    /// Fetch a team's crest and store it under the cache dir. Never
    /// poller-fatal: every failure (network, non-2xx, oversized body,
    /// filesystem) is logged and swallowed — the caller already has
    /// `None` on the wire for this team and the text-abbrev fallback
    /// renders regardless.
    pub async fn fetch_and_store(&self, client: &reqwest::Client, team_id: &str, url: &str) {
        if let Err(e) = self.try_fetch(client, team_id, url).await {
            tracing::warn!(team_id, url, "crest fetch failed: {e}");
        }
    }

    async fn try_fetch(
        &self,
        client: &reqwest::Client,
        team_id: &str,
        url: &str,
    ) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let response = client.get(url).send().await?.error_for_status()?;
        let bytes = crate::net::read_body_capped(response, MAX_CREST_BYTES).await?;
        write_atomic(&self.path_for(team_id), &bytes)
    }
}

/// Same-dir temp-file + rename atomic write (matches `settings.rs`'s
/// config/secrets write posture) — a crest fetch racing a read (the
/// frontend loading the same path) must never observe a partial PNG.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("png.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Test-local temp dir (no `tempfile` dev-dependency in this crate) —
/// a uuid-suffixed subdir of the OS temp dir, cleaned up on drop. `pub(crate)`
/// so `poller.rs`'s crest-patching tests (`patch_crests`) can share it
/// rather than reinventing their own.
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
        // simulates what a warm restart finds on disk: a PNG already
        // written by a previous process, no fetch involved this run.
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
    fn team_id_is_sanitized_in_the_cache_filename() {
        let (_dir, cache) = temp_cache();
        // a defensively-hostile team id must not escape the cache dir
        let path = cache.path_for("../../etc/passwd");
        assert_eq!(path.file_name().unwrap(), "etcpasswd.png");
        assert_eq!(path.parent().unwrap(), cache.dir);
    }
}
