//! The now-playing ambient source (plan 104): a supervised, long-lived
//! `mediaremote-adapter` `stream` child (vendored, SHA-pinned —
//! `src-tauri/vendor/mediaremote-adapter/VENDORED.md`), feeding
//! `StatusState`'s ambient media row exactly like `weather_poller.rs`
//! feeds the weather chip. The mechanism and its risk profile are
//! recorded in full in `docs/design/now-playing-adapter.md` (plan 103's
//! spike) — this module is that spike's GO turned into a spec (§8).
//!
//! **Ambient-only** (this plan's non-negotiable decision 3): media never
//! becomes an `Event`/card. No `SourceKind` variant, no queue
//! interaction — this module only ever calls `engine.update_now_playing`,
//! the same one-way push `weather_poller.rs` uses via
//! `engine.update_weather`.
//!
//! **A materially different producer lifecycle than every other poller in
//! this repo**: `espn`/`rss`/`weather` are all "wake on a timer, fetch,
//! parse, sleep" loops around a short-lived request. This is a held-open
//! child process streaming newline-delimited JSON diffs — closer to a
//! log-tailer than a poller (`docs/design/now-playing-adapter.md` §8's
//! own framing). The supervision shape (restart-on-exit with backoff,
//! treating a closed stdout as the failure signal) is genuinely new
//! surface area for this codebase, which is why it gets its own pure,
//! unit-tested state machine (`Supervisor`) rather than reusing
//! `poller::Backoff` — that type's `on_success` resets on every good
//! response, appropriate for a poll that either fails or succeeds each
//! tick; this module's schedule is plan 104's own spec (5s → 10s → 30s →
//! 60s, reset only after 5 minutes of continuous healthy runtime), a
//! deliberately slower-to-forgive reset for a crash-looping child.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::engine::Engine;
use crate::status::NowPlayingSummary;

/// The entitlement trick this whole feature rests on (`docs/design/
/// now-playing-adapter.md` §2) depends on the HOST PROCESS being the
/// real, Apple-signed `/usr/bin/perl` — `codesign -dv` on that exact
/// binary is what carries the `com.apple.perl` identifier
/// `mediaremoted`'s allowlist check waves through. Resolving "perl" via
/// `$PATH` instead (e.g. a Homebrew perl) would silently break the
/// mechanism entirely — this MUST stay a hardcoded absolute path, never
/// `Command::new("perl")`.
const SYSTEM_PERL: &str = "/usr/bin/perl";

/// Backoff schedule (plan 104's own spec, distinct from `poller::Backoff`'s
/// doubling-with-immediate-reset shape — see this module's doc comment
/// for why). Capped at the last entry once exhausted.
const BACKOFF_SCHEDULE_SECS: [u64; 4] = [5, 10, 30, 60];

/// How long a single child run must survive before its NEXT restart gets
/// the floor backoff again, instead of continuing to escalate.
const HEALTHY_RESET_SECS: u64 = 5 * 60;

/// The restart-backoff state machine, decoupled from any real subprocess
/// so it's unit-testable the same way `presentation::presentation_mode`
/// is (`docs/design/now-playing-adapter.md` §11's own suggestion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Supervisor {
    attempt: usize,
}

impl Supervisor {
    /// The delay (seconds) before the next restart attempt, advancing the
    /// schedule index by one each call (capped at the schedule's last
    /// entry) — call this once per child-exit event, before sleeping.
    pub fn next_backoff_secs(&mut self) -> u64 {
        let secs = BACKOFF_SCHEDULE_SECS[self.attempt.min(BACKOFF_SCHEDULE_SECS.len() - 1)];
        if self.attempt < BACKOFF_SCHEDULE_SECS.len() - 1 {
            self.attempt += 1;
        }
        secs
    }

    /// Resets the schedule to its floor — call this only after a child
    /// instance ran for at least `HEALTHY_RESET_SECS`, never on a bare
    /// successful line (unlike `poller::Backoff::on_success`).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

/// The three-condition gate (plan 104 Step 4): both config flags true AND
/// the two expected adapter files exist under the configured directory.
/// A pure function over already-performed `Path::exists()` reads, so the
/// gate logic itself is unit-testable without touching the filesystem.
pub fn should_spawn(
    now_playing_enabled: bool,
    now_playing_adapter_enabled: bool,
    pl_exists: bool,
    framework_exists: bool,
) -> bool {
    now_playing_enabled && now_playing_adapter_enabled && pl_exists && framework_exists
}

/// Whether pushing `next` to the ambient channel is warranted given
/// `previous` — decision 5's compare-before-push discipline (CLAUDE.md's
/// `SlotState::dedup_eq` lesson, plan 081: never let a per-line/per-tick
/// read drive a wire emission when nothing actually changed), pulled out
/// of the IO loop below so it's directly unit-testable.
fn changed(previous: &Option<NowPlayingSummary>, next: &Option<NowPlayingSummary>) -> bool {
    previous != next
}

/// Wall-clock epoch millis at receipt — same technique as
/// `history.rs::now_ms` (that function is private to its own module, so
/// this mirrors it rather than reaching across crate-internal privacy for
/// one line; `history.rs` isn't in this plan's scope).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Secs (f64, the adapter's own `elapsedTime`/`duration` unit) → ms (u64),
/// saturating: `NaN` or a negative value collapses to 0 (no defined
/// order/meaning), `+inf` (or any value overflowing `u64`) clamps to
/// `u64::MAX` rather than wrapping or panicking on the `as` cast.
fn secs_to_ms(secs: f64) -> u64 {
    if secs.is_nan() || secs <= 0.0 {
        return 0;
    }
    let ms = secs * 1000.0;
    if ms >= u64::MAX as f64 {
        u64::MAX
    } else {
        ms as u64
    }
}

/// Partial mirror of the adapter's real payload shape
/// (`docs/design/now-playing-adapter.md` §5/§5c, this plan's own Step 2
/// live probe) — only the fields this feature reads, every one optional
/// with `#[serde(default)]` so an unmodeled extra key or a payload
/// omitting a field this diff didn't change never fails the parse.
/// `elapsedTime`/`duration` are floating-point SECONDS on the wire, not
/// ms. `parentApplicationBundleIdentifier` is the app-identifying field
/// when present (103 §5c: a Safari `<audio>` session's own
/// `bundleIdentifier` is the process-internal `com.apple.WebKit.GPU`,
/// not `com.apple.Safari`) — prefer it, falling back to
/// `bundleIdentifier` only when it's absent.
#[derive(Debug, Default, Deserialize)]
struct RawPayload {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    playing: Option<bool>,
    #[serde(default, rename = "elapsedTime")]
    elapsed_time: Option<f64>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default, rename = "bundleIdentifier")]
    bundle_identifier: Option<String>,
    #[serde(default, rename = "parentApplicationBundleIdentifier")]
    parent_application_bundle_identifier: Option<String>,
}

impl RawPayload {
    /// An empty payload object (`{}`) — the observed connection-time /
    /// no-session shape (this plan's Step 2 live probe:
    /// `{"type":"data","diff":false,"payload":{}}` on `stream` connect,
    /// matching `docs/design/now-playing-adapter.md` §5's own finding).
    fn is_all_absent(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.playing.is_none()
            && self.elapsed_time.is_none()
            && self.duration.is_none()
            && self.bundle_identifier.is_none()
            && self.parent_application_bundle_identifier.is_none()
    }
}

/// The `stream` wire envelope (§5, §8): `{"type":"data","diff":bool,
/// "payload":{...}}`. `payload` is `#[serde(default)]`-optional: a
/// missing/`null` payload key degrades to "no session," the same as an
/// empty `{}` object — no such line was observed live in this plan's own
/// probing (only `{}` was), but the adapter's `get` mode DOES print a
/// bare top-level `null` when nothing is playing (§5b), so a stream line
/// shaped that way someday must degrade the same way, not panic.
#[derive(Debug, Deserialize)]
struct StreamLine {
    #[serde(default)]
    payload: Option<RawPayload>,
}

/// The pure diff-application function (plan 104 Step 4): merges one raw
/// adapter stdout line into `state`. `stream`'s default mode sends DIFF
/// lines — only changed keys are present — so an absent field means
/// "carry the previous value forward," which is why this MERGES rather
/// than replaces. `title` is required after the merge: no title (a fresh
/// session with no title field yet, or a payload that never established
/// one) is treated as no session at all, matching the adapter's own
/// `mandatoryPayloadKeys` always including `title`
/// (`docs/design/now-playing-adapter.md` §5d). A malformed line (invalid
/// JSON — e.g. the adapter's own occasional stderr-bound diagnostics,
/// §5a's `duration: nan` warning, never land on stdout, but a defensive
/// parse failure here must not be fatal either) is silently ignored,
/// leaving `state` untouched.
pub fn apply_event(state: &mut Option<NowPlayingSummary>, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    let parsed: Result<Option<StreamLine>, _> = serde_json::from_str(trimmed);
    let Ok(parsed) = parsed else {
        // malformed line: ignored, not fatal (Step 4's spec)
        return;
    };
    let Some(stream_line) = parsed else {
        // bare `null` — treated the same as an empty/absent payload.
        *state = None;
        return;
    };
    let Some(payload) = stream_line.payload else {
        *state = None;
        return;
    };
    if payload.is_all_absent() {
        *state = None;
        return;
    }

    let previous = state.take();
    let title = payload
        .title
        .or_else(|| previous.as_ref().map(|p| p.title.clone()));
    let Some(title) = title else {
        // no title after merge: not a session (Step 4's spec).
        *state = None;
        return;
    };

    let artist = payload
        .artist
        .or_else(|| previous.as_ref().and_then(|p| p.artist.clone()));
    let album = payload
        .album
        .or_else(|| previous.as_ref().and_then(|p| p.album.clone()));
    let playing = payload
        .playing
        .unwrap_or_else(|| previous.as_ref().map(|p| p.playing).unwrap_or(false));
    let elapsed_ms = payload
        .elapsed_time
        .map(secs_to_ms)
        .unwrap_or_else(|| previous.as_ref().map(|p| p.elapsed_ms).unwrap_or(0));
    let duration_ms = match payload.duration {
        Some(secs) => Some(secs_to_ms(secs)),
        None => previous.as_ref().and_then(|p| p.duration_ms),
    };
    let app_bundle_id = payload
        .parent_application_bundle_identifier
        .or(payload.bundle_identifier)
        .or_else(|| previous.as_ref().and_then(|p| p.app_bundle_id.clone()));

    *state = Some(NowPlayingSummary {
        title,
        artist,
        album,
        playing,
        elapsed_ms,
        duration_ms,
        captured_at_ms: now_ms(),
        app_bundle_id,
    });
}

/// The supervised streaming child. Entry point called unconditionally
/// from `lib.rs`'s `setup` (mirrors `weather_poller`'s config-gated call
/// shape, except the gate — both config flags AND the two adapter files
/// existing — lives inside this function rather than at the call site,
/// since the file-existence half of the gate needs filesystem IO this
/// module already owns).
pub fn spawn_now_playing_poller(
    engine: Engine,
    now_playing_enabled: bool,
    now_playing_adapter_enabled: bool,
    adapter_dir: PathBuf,
) {
    let pl_path = adapter_dir.join("bin").join("mediaremote-adapter.pl");
    let framework_path = adapter_dir.join("MediaRemoteAdapter.framework");
    let pl_exists = pl_path.exists();
    let framework_exists = framework_path.exists();

    if !should_spawn(
        now_playing_enabled,
        now_playing_adapter_enabled,
        pl_exists,
        framework_exists,
    ) {
        // Clean degrade, never a startup error (mirrors `detect_path`'s
        // own missing-binary tolerance) — log once so a user who enabled
        // the feature but never ran `just build-media-adapter` has a
        // trail to follow, without spamming on every restart (there is
        // no restart here: the task simply never spawns).
        if now_playing_enabled && now_playing_adapter_enabled {
            tracing::warn!(
                dir = %adapter_dir.display(),
                pl_exists,
                framework_exists,
                "now-playing enabled but the adapter isn't installed at the configured path \
                 — not spawning (run `just build-media-adapter`)"
            );
        }
        return;
    }

    tauri::async_runtime::spawn(async move {
        tracing::info!(dir = %adapter_dir.display(), "now-playing adapter poller started");
        let mut supervisor = Supervisor::default();
        loop {
            let started_at = Instant::now();
            if let Err(error) = run_stream_once(&pl_path, &framework_path, &engine).await {
                tracing::warn!("now-playing adapter stream error: {error}");
            }
            // The child is gone either way (error or a clean stdout
            // close) — clear the ambient state so a lost/restarting
            // child never leaves a stale "still playing" row on screen.
            engine.update_now_playing(None);

            if started_at.elapsed() >= Duration::from_secs(HEALTHY_RESET_SECS) {
                supervisor.reset();
            }
            let backoff_secs = supervisor.next_backoff_secs();
            tracing::info!(
                backoff_secs,
                "now-playing adapter child exited; restarting after backoff"
            );
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
        }
    });
}

/// One child lifetime: spawn, read diff lines until stdout closes (the
/// failure signal — `docs/design/now-playing-adapter.md` §8 — treated
/// the same whether the child also exited nonzero or is still exiting),
/// pushing each CHANGED summary through `engine.update_now_playing`.
/// `kill_on_drop(true)` covers the main-path clean shutdown (app exit
/// drops the `Child`); this function's own loop exit (stdout closed)
/// covers the restart path via the caller's supervision loop.
async fn run_stream_once(
    pl_path: &Path,
    framework_path: &Path,
    engine: &Engine,
) -> anyhow::Result<()> {
    let mut child = Command::new(SYSTEM_PERL)
        .arg(pl_path)
        .arg(framework_path)
        .arg("stream")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("adapter child had no stdout"))?;
    let mut lines = BufReader::new(stdout).lines();

    let mut current: Option<NowPlayingSummary> = None;
    while let Some(line) = lines.next_line().await? {
        let before = current.clone();
        apply_event(&mut current, &line);
        if changed(&before, &current) {
            engine.update_now_playing(current.clone());
        }
    }

    // stdout closed: reap the child so it never lingers as a zombie, then
    // return — the caller's loop treats this the same as an error return.
    let _ = child.wait().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(title: &str) -> NowPlayingSummary {
        NowPlayingSummary {
            title: title.to_string(),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            playing: true,
            elapsed_ms: 1000,
            duration_ms: Some(200_000),
            captured_at_ms: 0,
            app_bundle_id: Some("com.apple.Safari".to_string()),
        }
    }

    // --- gating (pure function over the three conditions) ---

    #[test]
    fn should_spawn_requires_all_four_conditions() {
        assert!(should_spawn(true, true, true, true));
        assert!(!should_spawn(false, true, true, true));
        assert!(!should_spawn(true, false, true, true));
        assert!(!should_spawn(true, true, false, true));
        assert!(!should_spawn(true, true, true, false));
        assert!(!should_spawn(false, false, false, false));
    }

    // --- change-only emission (decision 5) ---

    #[test]
    fn changed_detects_a_real_transition_and_ignores_a_repeat() {
        assert!(changed(&None, &Some(summary("t"))));
        assert!(!changed(&Some(summary("t")), &Some(summary("t"))));
        assert!(changed(&Some(summary("t")), &Some(summary("u"))));
        assert!(changed(&Some(summary("t")), &None));
        assert!(!changed(&None, &None));
    }

    // --- ms conversion ---

    #[test]
    fn secs_to_ms_converts_and_saturates() {
        assert_eq!(secs_to_ms(1.5), 1500);
        assert_eq!(secs_to_ms(0.0), 0);
        assert_eq!(secs_to_ms(-5.0), 0);
        assert_eq!(secs_to_ms(f64::NAN), 0);
        assert_eq!(secs_to_ms(f64::INFINITY), u64::MAX);
    }

    // --- backoff schedule ---

    #[test]
    fn supervisor_escalates_then_caps_at_the_schedule_ceiling() {
        let mut s = Supervisor::default();
        assert_eq!(s.next_backoff_secs(), 5);
        assert_eq!(s.next_backoff_secs(), 10);
        assert_eq!(s.next_backoff_secs(), 30);
        assert_eq!(s.next_backoff_secs(), 60);
        // stays at the ceiling for any further failures
        assert_eq!(s.next_backoff_secs(), 60);
        assert_eq!(s.next_backoff_secs(), 60);
    }

    #[test]
    fn supervisor_reset_returns_to_the_floor() {
        let mut s = Supervisor::default();
        s.next_backoff_secs();
        s.next_backoff_secs();
        s.next_backoff_secs();
        assert_eq!(s.next_backoff_secs(), 60);
        s.reset();
        assert_eq!(s.next_backoff_secs(), 5);
    }

    // --- apply_event: fresh session ---

    #[test]
    fn apply_event_establishes_a_fresh_session() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":false,"payload":{"title":"Midnight City","artist":"M83","album":"Hurry Up, We're Dreaming","playing":true,"elapsedTime":1.5,"duration":243.0,"bundleIdentifier":"app.zen-browser.zen"}}"#,
        );
        let s = state.expect("expected a session");
        assert_eq!(s.title, "Midnight City");
        assert_eq!(s.artist.as_deref(), Some("M83"));
        assert_eq!(s.album.as_deref(), Some("Hurry Up, We're Dreaming"));
        assert!(s.playing);
        assert_eq!(s.elapsed_ms, 1500);
        assert_eq!(s.duration_ms, Some(243_000));
        assert_eq!(s.app_bundle_id.as_deref(), Some("app.zen-browser.zen"));
    }

    // --- apply_event: parentApplicationBundleIdentifier preferred (103 §5c) ---

    #[test]
    fn apply_event_prefers_parent_application_bundle_identifier() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":false,"payload":{"title":"t","playing":true,"bundleIdentifier":"com.apple.WebKit.GPU","parentApplicationBundleIdentifier":"com.apple.Safari"}}"#,
        );
        assert_eq!(
            state.unwrap().app_bundle_id.as_deref(),
            Some("com.apple.Safari")
        );
    }

    #[test]
    fn apply_event_falls_back_to_bundle_identifier_when_parent_absent() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":false,"payload":{"title":"t","playing":true,"bundleIdentifier":"app.zen-browser.zen"}}"#,
        );
        assert_eq!(
            state.unwrap().app_bundle_id.as_deref(),
            Some("app.zen-browser.zen")
        );
    }

    // --- apply_event: diff merge (a later diff line only carries changed keys) ---

    #[test]
    fn apply_event_merges_a_diff_line_over_the_previous_session() {
        let mut state = Some(summary("Midnight City"));
        // a diff carrying only playing + elapsedTime — title/artist/album
        // must survive from the previous state.
        apply_event(
            &mut state,
            r#"{"type":"data","diff":true,"payload":{"playing":false,"elapsedTime":42.0}}"#,
        );
        let s = state.expect("session must survive a partial diff");
        assert_eq!(s.title, "Midnight City");
        assert_eq!(s.artist.as_deref(), Some("Artist"));
        assert!(!s.playing);
        assert_eq!(s.elapsed_ms, 42_000);
    }

    // --- apply_event: artist-less session ---

    #[test]
    fn apply_event_accepts_a_session_with_no_artist() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":false,"payload":{"title":"Some Video","playing":true}}"#,
        );
        let s = state.expect("title alone is a valid session");
        assert_eq!(s.title, "Some Video");
        assert_eq!(s.artist, None);
        assert_eq!(s.album, None);
    }

    // --- apply_event: session-end clears (the observed connection-time /
    // end-of-session empty-payload shape) ---

    #[test]
    fn apply_event_empty_payload_clears_an_existing_session() {
        let mut state = Some(summary("Midnight City"));
        apply_event(&mut state, r#"{"type":"data","diff":false,"payload":{}}"#);
        assert_eq!(state, None);
    }

    #[test]
    fn apply_event_bare_null_clears_an_existing_session() {
        let mut state = Some(summary("Midnight City"));
        apply_event(&mut state, "null");
        assert_eq!(state, None);
    }

    #[test]
    fn apply_event_missing_payload_key_clears_an_existing_session() {
        let mut state = Some(summary("Midnight City"));
        apply_event(&mut state, r#"{"type":"data","diff":false}"#);
        assert_eq!(state, None);
    }

    #[test]
    fn apply_event_diff_that_never_established_a_title_yields_no_session() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":true,"payload":{"playing":true,"elapsedTime":3.0}}"#,
        );
        assert_eq!(state, None);
    }

    // --- apply_event: malformed line ignored, not fatal ---

    #[test]
    fn apply_event_malformed_line_is_ignored_and_state_is_unchanged() {
        let mut state = Some(summary("Midnight City"));
        apply_event(&mut state, "not json at all {{{");
        assert_eq!(state, Some(summary("Midnight City")));

        let mut empty_state: Option<NowPlayingSummary> = None;
        apply_event(&mut empty_state, "");
        assert_eq!(empty_state, None);
    }

    // --- apply_event: ms conversion end-to-end through a real payload ---

    #[test]
    fn apply_event_converts_fractional_seconds_to_milliseconds() {
        let mut state = None;
        apply_event(
            &mut state,
            r#"{"type":"data","diff":false,"payload":{"title":"t","playing":true,"elapsedTime":9.854145,"duration":829.981}}"#,
        );
        let s = state.unwrap();
        assert_eq!(s.elapsed_ms, 9854);
        assert_eq!(s.duration_ms, Some(829_981));
    }
}
