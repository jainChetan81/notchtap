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

/// R7/C10 fix (2026-07-25): the longest stretch `run_stream_once`'s read
/// loop will wait for ANY line (not just a content change) before
/// deciding the child is wedged rather than merely quiet. 90s is
/// deliberately generous — the adapter's own `stream` mode has no
/// heartbeat/keepalive (`docs/design/now-playing-adapter.md` — nothing
/// idle-signals beyond the payload itself), so "no media playing at
/// all" and "the adapter silently hung" look IDENTICAL from here; this
/// bound only needs to be long enough that a real (if boring) idle
/// stretch doesn't trip it constantly, not long enough to rule out
/// genuine wedging quickly. On expiry the child is killed and this
/// function returns exactly the way a clean stdout close already does
/// (`Ok(())`, no distinct error variant) — so a run that goes idle
/// reconnects through the SAME restart/backoff schedule every other
/// exit already uses, rather than a bespoke path that could escalate
/// (or reset) differently and thrash.
const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(90);

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
///
/// `captured_at_ms` (CLAUDE.md's `dedup_eq` rule: a continuously-varying
/// wire field must extend the change-comparison explicitly and must
/// never ride a derived/full `PartialEq`) is deliberately EXCLUDED here
/// — it's stamped fresh on every applied line (`apply_event`, `now_ms()`)
/// regardless of whether anything else changed, so comparing it would
/// make an identical resync (adapter reconnect resending the exact same
/// title/artist/.../just a newer timestamp) read as "changed" and fire a
/// spurious `engine.update_now_playing` call. The field itself is still
/// carried on `NowPlayingSummary` and still stamped — only THIS
/// change-detection gate ignores it.
fn changed(previous: &Option<NowPlayingSummary>, next: &Option<NowPlayingSummary>) -> bool {
    match (previous, next) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(p), Some(n)) => {
            p.title != n.title
                || p.artist != n.artist
                || p.album != n.album
                || p.playing != n.playing
                || p.elapsed_ms != n.elapsed_ms
                || p.duration_ms != n.duration_ms
                || p.app_bundle_id != n.app_bundle_id
        }
    }
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

/// L-sec6 fix (2026-07-25): before exec'ing `pl_path` as the real,
/// Apple-signed `/usr/bin/perl` (the entitlement trick this whole
/// feature rests on — see `SYSTEM_PERL`'s own doc comment), refuse to
/// spawn if the script itself OR its containing directory is group- or
/// world-writable. Same defensive posture as the secrets loader
/// (`settings.rs` writes its own files/dirs `0o600`/`0o700`): a writable
/// script or directory on a shared/multi-user machine means another
/// local account could plant or swap the file this process is about to
/// exec as itself. Checked fresh on every spawn attempt (this is called
/// from `run_stream_once`, which the supervision loop re-enters on every
/// restart) rather than once at startup, since permissions can change
/// out from under a long-lived running app. Does NOT change path
/// resolution — `pl_path` is still whatever `adapter_dir` resolved to;
/// this only gates whether the exec is allowed to happen.
///
/// A `metadata()` failure (e.g. the file vanished between `should_spawn`'s
/// existence check and now) is NOT treated as a violation here — the
/// `Command::spawn()` call right after this check will fail loudly with
/// its own `ENOENT`, which is already routed through the normal
/// warn+backoff path, so there is no separate silent-failure mode to
/// guard against.
#[cfg(unix)]
fn adapter_permission_violation(pl_path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    fn group_or_world_writable(meta: &std::fs::Metadata) -> bool {
        meta.mode() & 0o022 != 0
    }

    let file_meta = std::fs::metadata(pl_path).ok()?;
    if group_or_world_writable(&file_meta) {
        return Some(format!("{} is group/world-writable", pl_path.display()));
    }
    let dir = pl_path.parent()?;
    let dir_meta = std::fs::metadata(dir).ok()?;
    if group_or_world_writable(&dir_meta) {
        return Some(format!("{} is group/world-writable", dir.display()));
    }
    None
}

#[cfg(not(unix))]
fn adapter_permission_violation(_pl_path: &Path) -> Option<String> {
    None
}

/// One child lifetime: spawn, read diff lines until stdout closes (the
/// failure signal — `docs/design/now-playing-adapter.md` §8 — treated
/// the same whether the child also exited nonzero or is still exiting),
/// pushing each CHANGED summary through `engine.update_now_playing`.
/// `kill_on_drop(true)` covers the main-path clean shutdown (app exit
/// drops the `Child`); this function's own loop exit (stdout closed, or
/// the inactivity watchdog firing — R7/C10 fix, below) covers the
/// restart path via the caller's supervision loop.
async fn run_stream_once(
    pl_path: &Path,
    framework_path: &Path,
    engine: &Engine,
) -> anyhow::Result<()> {
    if let Some(violation) = adapter_permission_violation(pl_path) {
        tracing::warn!(
            pl_path = %pl_path.display(),
            violation,
            "now-playing adapter script or its directory is group/world-writable — refusing to spawn"
        );
        anyhow::bail!("unsafe adapter permissions: {violation}");
    }

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
    loop {
        // R7/C10 fix: an unbounded `lines.next_line().await` parks this
        // supervisor forever if the child is alive but silently wedged
        // (never closes stdout, never prints another line) — the row
        // stays stuck on whatever was last applied. This 90s bound (see
        // `INACTIVITY_TIMEOUT`'s doc) treats "no line at all in that
        // window" as wedged and restarts, same as any other exit —
        // genuine "nothing is playing" silence is expected to recur
        // after the restart too, which is fine (cheap, and NOT routed
        // through a different/escalating path — see the const's doc for
        // why that matters).
        let next_line = match tokio::time::timeout(INACTIVITY_TIMEOUT, lines.next_line()).await {
            Ok(read_result) => read_result?,
            Err(_elapsed) => {
                tracing::warn!(
                    timeout_secs = INACTIVITY_TIMEOUT.as_secs(),
                    "now-playing adapter produced no output within the inactivity window — \
                     treating as wedged, restarting"
                );
                let _ = child.start_kill();
                break;
            }
        };
        let Some(line) = next_line else {
            break; // stdout closed
        };
        let before = current.clone();
        apply_event(&mut current, &line);
        if changed(&before, &current) {
            engine.update_now_playing(current.clone());
        }
    }

    // stdout closed (or the watchdog above killed the child): reap it so
    // it never lingers as a zombie, then return — the caller's loop
    // treats this the same as an error return.
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

    #[test]
    fn changed_ignores_a_captured_at_ms_only_difference() {
        // `dedup_eq` regression (CLAUDE.md): an adapter resync that
        // resends identical content with a fresh `captured_at_ms`
        // timestamp must NOT read as a change — otherwise every
        // reconnect (or, worse, every line) fires a spurious
        // `engine.update_now_playing` push.
        let mut older = summary("t");
        older.captured_at_ms = 1_000;
        let mut newer = summary("t");
        newer.captured_at_ms = 2_000;
        assert_ne!(older, newer, "sanity: the two summaries are not literally equal");
        assert!(
            !changed(&Some(older), &Some(newer)),
            "captured_at_ms alone must never trip change-detection"
        );
    }

    #[test]
    fn changed_still_detects_a_real_field_change_even_with_captured_at_ms_moving_too() {
        // the exclusion must not become "ignore everything" — a genuine
        // content change alongside the (always-different) timestamp must
        // still be reported.
        let mut before = summary("t");
        before.captured_at_ms = 1_000;
        before.elapsed_ms = 1_000;
        let mut after = summary("t");
        after.captured_at_ms = 2_000;
        after.elapsed_ms = 5_000;
        assert!(changed(&Some(before), &Some(after)));
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

    // --- L-sec6: refuse to spawn against a group/world-writable script
    // or containing directory ---

    #[cfg(unix)]
    mod adapter_permission_tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn temp_adapter_dir() -> PathBuf {
            std::env::temp_dir().join(format!(
                "notchtap-now-playing-test-{}",
                uuid::Uuid::new_v4()
            ))
        }

        #[test]
        fn none_when_script_and_dir_are_locked_down() {
            let dir = temp_adapter_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let pl = dir.join("mediaremote-adapter.pl");
            std::fs::write(&pl, b"#!/usr/bin/perl\n").unwrap();
            // 0o755 (rwxr-xr-x): no GROUP or WORLD write bit set.
            std::fs::set_permissions(&pl, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

            assert_eq!(adapter_permission_violation(&pl), None);
            std::fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn flags_a_world_writable_script() {
            let dir = temp_adapter_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let pl = dir.join("mediaremote-adapter.pl");
            std::fs::write(&pl, b"#!/usr/bin/perl\n").unwrap();
            std::fs::set_permissions(&pl, std::fs::Permissions::from_mode(0o777)).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

            let violation = adapter_permission_violation(&pl);
            assert!(violation.is_some(), "a world-writable script must be refused");
            assert!(violation.unwrap().contains("mediaremote-adapter.pl"));
            std::fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn flags_a_group_writable_containing_directory() {
            let dir = temp_adapter_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let pl = dir.join("mediaremote-adapter.pl");
            std::fs::write(&pl, b"#!/usr/bin/perl\n").unwrap();
            std::fs::set_permissions(&pl, std::fs::Permissions::from_mode(0o755)).unwrap();
            // 0o775: group write bit set on the DIRECTORY, not the file.
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();

            let violation = adapter_permission_violation(&pl);
            assert!(
                violation.is_some(),
                "a group-writable containing directory must be refused even if the script itself is locked down"
            );
            std::fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn none_when_the_script_does_not_exist() {
            // dir deliberately never created — `metadata()` fails, and
            // that must defer to `Command::spawn()`'s own ENOENT rather
            // than double-reporting a permissions violation.
            let dir = temp_adapter_dir();
            let pl = dir.join("mediaremote-adapter.pl");
            assert_eq!(adapter_permission_violation(&pl), None);
        }
    }
}
