# Plan 104: Now-playing ambient — vendored mediaremote-adapter stream → StatusState → idle-peek media row

> **Executor instructions**: Follow this plan step by step; run every
> verification command. On any STOP condition, stop and report. The
> reviewer maintains `plans/README.md` — do not edit it. This plan
> implements the GO from `docs/design/now-playing-adapter.md` (plan
> 103) under its two conditions — READ THAT DOC FIRST, especially §5
> (event shapes with real payloads), §7 (fragility) and §8 (the
> integration sketch this plan turns into a spec).
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it
> prints anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat e1f9d29..HEAD -- src-tauri/src src/`
> — on content mismatch with the excerpts below, STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (new long-lived-child supervision surface; mitigated by hard gating + clean degrade)
- **Depends on**: plan 103 (merged — the GO + evidence)
- **Category**: direction/feature
- **Planned at**: commit `e1f9d29`, 2026-07-21

## Why this matters

Operator asked for it explicitly after the 103 GO, and a live check
confirmed the full real-world case on this machine: Apple Music, a
YouTube tab in Zen, and Apple TV all register; MediaRemote exposes ONE
current session (most recently interacted — observed live:
`com.apple.TV`, title/album/elapsed/duration/playing all present). The
card design is `prototype/system-stats.html` option D: a now-playing
row in the idle hover-peek.

## Non-negotiable design decisions (from 103's GO conditions + advisor)

1. **Vendored, SHA-pinned, never fetched at build/run time**: upstream
   `ungive/mediaremote-adapter` at commit
   `3ac3d4bdf862c7b5399b4fba4df5689f5c38609a` (BSD-3-Clause; code-inspected
   clean in 103 §3).
2. **Two independent config gates**: `now_playing_enabled` (user
   feature toggle, settings UI, default `false` — weather's opt-in
   convention) AND `now_playing_adapter_enabled` (kill switch, default
   `true`, config-file only, deliberately NOT in the settings UI). The
   child spawns only when BOTH are true.
3. **Ambient-only — NO SourceKind change.** 103 §8 sketched a sixth
   `SourceKind::NowPlaying` following 095; do NOT add it. Media never
   makes alert cards in v1, and a new SourceKind ripples into
   `rotation_order` permutation validation, the settings reorder
   widget, and the per-arm `build_test_event` tests for zero benefit.
   If a future plan wants "track changed" alert cards, it adds the
   variant then. (Advisor decision, 2026-07-21.)
4. **No new `#[tauri::command]`.** Config fields flow through the
   existing `get_config`/`save_config_and_relaunch` path.
   `src-tauri/build.rs`, `src-tauri/capabilities/*` MUST NOT change.
5. **Emission discipline (the plan-081 lesson):** never let
   continuously-varying playback time drive per-second status
   emissions. `current` carries a SNAPSHOT (`elapsed_ms`, `duration_ms`,
   `captured_at_ms`, `playing`); the frontend derives live progress
   locally (as `TtlBar` does). Status updates fire only on discrete
   adapter events (diff lines).
6. **Artwork: deferred.** v1 shows a glyph keyed off the app bundle id
   (see Step 7); base64 artwork over the wire is a follow-up.

## Current state (the patterns to mirror — open each before coding)

- **Runtime-path convention**: `src-tauri/src/config.rs:13/:194-195` —
  `detect_path: PathBuf`, default `/usr/local/bin/notchtap-detect`;
  the binary is built/installed out of band (`justfile:45`
  `check-swift`). Mirror this: `now_playing_adapter_dir: PathBuf`,
  default `/usr/local/lib/notchtap/mediaremote-adapter` (expected to
  contain `bin/mediaremote-adapter.pl` + `MediaRemoteAdapter.framework`).
  Missing/invalid dir at spawn time = log once, don't spawn — clean
  degrade, never an error surface.
- **Ambient status shape**: `src-tauri/src/status.rs:26/:46-108` —
  `WeatherStatus { enabled, current: Option<WeatherSummary> }` and the
  `StatusInputs` assembly. Media mirrors this exactly:
  `MediaStatus { enabled, current: Option<NowPlayingSummary> }`.
  Match weather's serde casing exactly (the TS side sees camelCase,
  e.g. `tempDisplay`).
- **Ambient producer**: `src-tauri/src/weather_poller.rs` — how a
  producer pushes ambient data into the engine/status path (plan 073's
  `AmbientSlot`). Media's producer differs in lifecycle (long-lived
  streaming child, not a timer) but pushes through the same mechanism.
- **Poller task spawn/gating**: how `weather_poller`/`poller` tasks are
  conditionally started from `lib.rs` — mirror the gating for the new
  task.
- **Frontend status validation**: `src/useStatusState.ts:54-99` —
  per-field validators (`isValidWeatherSummary` etc.), defense-in-depth
  on every field. Add `isValidNowPlaying` in the same style, wired into
  `isValidStatusState` and `FALLBACK_STATUS`.
- **Peek composition**: `src/components/IdleHoverPeek.tsx` — current
  precedence: football scorecard > weather scene > timeline fallback.
  Media row slots BETWEEN football and weather (actively-chosen media
  outranks ambient temperature; live match still outranks everything).
- **Adapter invocation** (verified live on this machine, 2026-07-21):
  `perl <dir>/bin/mediaremote-adapter.pl <dir>/MediaRemoteAdapter.framework stream`
  (default diff mode), newline-delimited JSON on stdout. Payload fields
  observed: `title`, `album`, `artist` (may be absent), `playing`,
  `elapsedTime` (f64 secs), `duration` (f64 secs), `timestamp`,
  `bundleIdentifier`, `parentApplicationBundleIdentifier` (may be
  absent; when present it is the app-identifying one — 103 §8), plus
  extras to ignore. `get` and `test` subcommands exist (`test` exits 0
  when entitled — use it as the spawn-time preflight).

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Rust tests | `cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo test` | all pass |
| Rust lint | `... cargo clippy --all-targets -- -D warnings` | exit 0 |
| Rust fmt | `... cargo fmt --check` | exit 0 |
| Frontend | `npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` / `npx vite build` | all clean |
| Adapter build (Step 2 recipe) | `just build-media-adapter` (or the recipe's raw commands) | framework built |

## Scope

**In scope**:
- `src-tauri/vendor/mediaremote-adapter/**` (new — vendored tree + `VENDORED.md`)
- `justfile` (one new recipe)
- `src-tauri/src/config.rs`, `status.rs`, `lib.rs`, new `now_playing.rs`
- `src-tauri/src/settings.rs` ONLY if weather's toggle row requires a
  per-field mirror there (follow whatever `weather_enabled` needed)
- `src/useStatusState.ts`, `src/components/IdleHoverPeek.tsx`,
  `src/styles.css`, `src/settings/preview-overlay.css` (mirror law),
  `src/settings/SettingsApp.tsx` (one toggle row, weather's pattern)
- test files colocated with all of the above
- `docs/TESTING_STRATEGY.md` §0 (final counts)

**Out of scope (hard)**:
- `src-tauri/build.rs`, `src-tauri/capabilities/**` — MUST be
  byte-untouched (done criterion).
- `src-tauri/src/event.rs` — no SourceKind change (decision 3).
- `queue.rs`, `hover.rs`, rotation logic — media makes no cards.
- Artwork transport.

## Steps

### Step 1: Vendor the adapter
`git clone` upstream into a temp dir OUTSIDE the repo, `git checkout
3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`, verify with `git rev-parse HEAD`,
then copy `bin/ src/ include/ scripts/ CMakeLists.txt Makefile LICENSE`
into `src-tauri/vendor/mediaremote-adapter/` (NO `.git`, NO `build/`).
Add `VENDORED.md`: upstream URL, the pinned SHA, license note, "frozen
— never update without a reviewed plan; inspected in
docs/design/now-playing-adapter.md §3", and the one-line build/install
instruction (Step 2's recipe).
**Verify**: `ls src-tauri/vendor/mediaremote-adapter/bin/mediaremote-adapter.pl` exists; `grep 3ac3d4b src-tauri/vendor/mediaremote-adapter/VENDORED.md` → hit; no `.git` dir inside.

### Step 2: Build/install recipe
Add `build-media-adapter` to the `justfile` (match its recipe style):
cmake-configure + build the vendored tree into
`src-tauri/vendor/mediaremote-adapter/build/` (git-ignore that path),
then install `MediaRemoteAdapter.framework` and `bin/` to
`/usr/local/lib/notchtap/mediaremote-adapter/` (`mkdir -p` + `cp -R`,
no sudo — STOP if not writable). Run it once to prove it works, then
`perl /usr/local/lib/notchtap/mediaremote-adapter/bin/mediaremote-adapter.pl /usr/local/lib/notchtap/mediaremote-adapter/MediaRemoteAdapter.framework test`.
**Verify**: exit 0 from `test` (this machine is entitled — 103 proved it).
Add the `build/` ignore to the repo's `.gitignore` (scoped:
`src-tauri/vendor/mediaremote-adapter/build/`).

### Step 3: Config fields
`now_playing_enabled: bool` default `false`;
`now_playing_adapter_enabled: bool` default `true`;
`now_playing_adapter_dir: PathBuf` default
`/usr/local/lib/notchtap/mediaremote-adapter` — serde defaults +
`Config::default()` + tests, all mirroring `weather_enabled`/`detect_path`
precedents (config.rs). No validation ranges needed (bools + path).
**Verify**: `cargo test config` green with new default tests
(`now_playing_disabled_by_default`, overrides round-trip).

### Step 4: `now_playing.rs` — the supervised streaming child
New module owning the child process. Requirements (from 103 §8):
- Spawn ONLY when `now_playing_enabled && now_playing_adapter_enabled`
  and the `.pl` + `.framework` paths exist (else one warn-level log,
  no task).
- `tokio::process::Command` (the repo's async runtime is already
  tokio), `kill_on_drop(true)`, stdout piped; read lines, parse JSON.
- **Pure diff-application function**, separately unit-testable:
  `fn apply_event(state: &mut Option<NowPlayingSummary>, line: &str)`
  — diff lines merge into current state; a payload signaling no
  session clears it (probe the adapter's actual "session ended" shape
  during Step 2's test run — likely `null`/empty-object line; document
  what you observed in a comment).
- Map fields: `title` (required — no title after merge = treat as no
  session), `artist`/`album` optional, `playing`,
  `elapsed_ms`/`duration_ms` (secs f64 → ms u64, saturating),
  `captured_at_ms` (wall clock at receipt — `history.rs::now_ms`
  precedent), `app_bundle_id` = `parentApplicationBundleIdentifier`
  falling back to `bundleIdentifier`.
- Push state changes through the same ambient path weather uses; emit
  only when the summary CHANGED (compare before push — decision 5).
- Supervision: on child exit OR stdout close, restart with backoff
  5s → 10s → 30s → 60s (cap), reset backoff after 5 min healthy.
  Clean shutdown on app exit (kill_on_drop covers the main path).
- Tests: `apply_event` cases (fresh session, diff merge, artist-less,
  session-end clears, malformed line ignored-not-fatal, ms conversion),
  gating logic (pure function over the three conditions), change-only
  emission.
**Verify**: `cargo test now_playing` → all new tests pass.

### Step 5: StatusState wiring
`status.rs`: add `MediaStatus { enabled, current: Option<NowPlayingSummary> }`
to `StatusState` + `StatusInputs`, weather's shape/casing exactly.
Update the status assembly + its tests (enabled-gate off ⇒
`current: None` even if a session exists — same rule weather's gate
follows; check how weather handles this and match).
**Verify**: `cargo test status` green; a serialization test pins the
wire casing of every new field (camelCase like weather's).

### Step 6: Frontend — validator + peek row + styles
- `useStatusState.ts`: `media` in `FALLBACK_STATUS`
  (`{ enabled: false, current: null }`), `isValidNowPlaying` (every
  field checked; `current === null` allowed), wired into
  `isValidStatusState`.
- `IdleHoverPeek.tsx`: media row between football and weather
  (precedence: football > media > weather > timeline). Layout per
  `prototype/system-stats.html` option D: app glyph (Step 7), bold
  title — artist (single line, ellipsis), ▶/⏸ state, thin progress
  bar + m:ss remaining-or-elapsed. Progress derives LOCALLY:
  `elapsedMs + (playing ? now - capturedAtMs : 0)`, clamped to
  `durationMs`; rAF or 1s interval while the peek is open only
  (`TtlBar.tsx` is the reduced-motion + cleanup pattern — honor
  `prefers-reduced-motion` by rendering static progress).
- `styles.css` + `preview-overlay.css` (mirror law): `.media-row` in
  the peek's existing vocabulary — chips/gauge sizing consistent with
  `.idle-peek` rows; no new colors beyond existing tokens.
- `SettingsApp.tsx`: one "Now playing" toggle in the sources section,
  weather's row as the pattern. The kill switch does NOT appear in UI.
- Tests (vitest): validator accept/reject/missing-field; peek renders
  media row when present; football outranks media; media outranks
  weather; paused state renders ⏸; no-media renders nothing extra;
  settings toggle round-trips into saved config (weather's test as
  pattern).
**Verify**: `npx vitest run` green; `npx tsc --noEmit`; `npx biome ci .`.

### Step 7: App glyph map
Tiny TS map in the peek component: bundle id containing `Music` → ♪,
`TV` → tv glyph, browsers (`Safari`/`zen`/`Chrome`/`firefox`,
case-insensitive substring) → globe, fallback → ▶. Pure function +
3-case test. (Text glyphs, not images — artwork is deferred.)

### Step 8: Final gates + §0
Full rust + frontend gates (commands table). Update
`docs/TESTING_STRATEGY.md` §0: new totals + per-category attributions
(config, status, the new now_playing module, slot… whichever moved),
sums must match observed output exactly.

## Done criteria

- [ ] All step verifications pass; full gates clean
- [ ] `git diff master -- src-tauri/build.rs src-tauri/capabilities` → empty
- [ ] `grep -rn "SourceKind" src-tauri/src/event.rs | wc -l` unchanged vs master
- [ ] `grep -rn "now_playing_adapter_enabled" src/settings/SettingsApp.tsx` → no hits (kill switch not in UI)
- [ ] Vendored tree present, SHA recorded, no `.git`/`build` committed (`git status` clean of them)
- [ ] §0 matches observed counts
- [ ] Only in-scope files modified

## STOP conditions

- Upstream clone unavailable or `git rev-parse HEAD` ≠ the pinned SHA.
- `/usr/local/lib` not writable without sudo.
- The adapter `test` subcommand fails on this machine (entitlement
  regressed since 103 — that's a finding, not something to work around).
- Weather's ambient-push path can't accommodate a second producer
  without refactoring `engine.rs`/`AmbientSlot` beyond additive use.
- Any needed change to `build.rs`/`capabilities/`/`event.rs`.
- The stream's session-end shape can't be determined empirically in
  Step 2/4 probing — report what the adapter actually emits instead of
  guessing.

## Maintenance notes

- The GO is conditional and revocable: if a macOS update silences the
  adapter, flip `now_playing_adapter_enabled = false` in config (no
  rebuild) — and 103's maintenance note says treat the GO as void, not
  silently re-spawn-looping. The backoff cap (60s) bounds the waste if
  nobody flips it.
- The vendored tree is frozen at the inspected SHA; bumping it requires
  a new inspection pass (103 §3's checklist) in a reviewed plan.
- Operator-owed after merge: run `just build-media-adapter` once on
  each machine, then enable the toggle in Settings.
- Follow-ups deliberately deferred: artwork transport; "track changed"
  alert cards (needs the SourceKind variant); mic-in-use "on a call"
  indicator (different primitive entirely).
