# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## project state

scaffolded and shipping. v1 (core engine, queue, one animation, cli
push), v2 (espn poller, cmux relay, animation table), v3 (outbound
connectors — telegram, `src-tauri/src/notifier.rs`), v4
(github + ci at `github.com/jainChetan81/notchtap`), and v3.6
(permanent rotating overlay — single-slot queue, priority tiers, the
"Status Rail" frontend redesign, global hotkeys for expand-toggle/
open-story/dismiss/pause-toggle) are done as of 2026-07-17. as of the
same date, the rss poller (`src-tauri/src/rss_poller.rs`), `NewsItem`
events with wire metadata (source/category/publishedAtMs), and the
status-rail news cards also landed. v5 (settings window / control
panel) is **done** — the invoke commands, config/secrets write paths,
`start_paused` kill switch, per-window command acl, and the settings
*page* itself (sidebar nav, rotation/priority group, shortcuts
cheatsheet) are all built and tested as of 2026-07-17. the same day,
`efa1bd2` (v5.1 appearance hot-apply + per-source test notifications)
added two more invoke commands — `send_test_notification` and
`set_appearance` (six commands at the time) — an Appearance section
with card-shape presets, and per-source test-notification buttons. v6
(commits `e1f1998`, `a693cf2`) added per-source priority, rotation-order
tie-break, and cmux origin naming. plan 020 (`9774930`, 2026-07-18)
added a seventh invoke command, `get_default_config` — seven invoke
commands total as of that date. decisions in `docs/ARCHITECTURE.md`
§17, plan in `docs/IMPLEMENTATION_PLAN.md` §4.5/§4.6, contract in
`docs/V5_TECHNICAL_SPEC.md`. the tauri/rust/web project lives at repo
root alongside
`docs/` — the docs folder isn't part of the app build. the test suite
exists and must stay green (`cargo test` from `src-tauri/`, `npx
vitest run` from repo root, all gated by ci) — current counts live in
`docs/TESTING_STRATEGY.md` §0 and only there. an uptime kuma →
notchtap webhook integration recipe (docs only, no source changes)
landed 2026-07-17 at `docs/recipes/kuma-webhook.md` — verified working
end-to-end against kuma v2.4.0. later landings the paragraph above
predates: plan 037 (`6b53c32`) introduced the Engine
(`src-tauri/src/engine.rs`) — one propagation module every Slot
mutation now flows through
(`apply`/`apply_blocking`/`read`/`accept`/`update_live_match`),
replacing the deleted `lib.rs` functions
`spawn_heartbeat`/`enqueue_and_emit`/`enqueue_and_fan_out`; every
ingest path (http, settings test notifications, both pollers) now
routes through `Engine::accept`, which also enforces the
news-never-to-telegram Connector rule structurally. plan 040 added the
weather source — a fifth `SourceKind` (`Weather`), an Open-Meteo
poller (`src-tauri/src/weather_poller.rs`) with an ambient idle-rail
chip and edge-triggered rain-incoming/hot/cold threshold alert cards,
opt-in via `weather_enabled` (default `false`). plans 039/041/042
added the opt-in (`espn_live_card`, default `false`) espn live-match
scoreboard card — one single-updating card per live match via Topic
supersession (`espn:{league}:{match_id}`), with per-side card counts
and a Clock detail line in its collapsed presentation, and scoring
plays labeled with espn's own event-type text (goal/penalty/own-goal).
plan 044 (`5c1ca36`) fixed a same-poll Topic-supersession ordering bug that
could permanently un-retire a finished match's card. plan 045 bumped the
`tauri-nspanel` git pin (13mo/39 commits behind) and added a compensating
`tauri_panel!` macro block for a confirmed API break the bump introduced.
plans 047/048 backfilled tests on newer modules and refactored the status
snapshot signature to named `StatusInputs` fields. `fb4acce` (unfiled) made
`Config::parse` self-heal a `rotation_order` missing a `SourceKind` variant by
appending it. plans 049-053 are docs-only design spikes in `docs/design/` —
deliverable is a design doc, zero production code, same precedent as 030/031.
plan 058 (`8743ce6`, 2026-07-20) added a `run` subcommand to the cli script.
plan 063 clamped the notch-mode idle rail to the cutout width and added the
shared `__NOTCHTAP_MODE__`/`__NOTCHTAP_CUTOUT_WIDTH__` boot-fact eval-splice
channel. the 064/066/067/070 hardening quartet added Topic supersede meta
freeze, cmux `ttl_secs` validation, rotation-order heal dedup, and `/notify`
ingest logging. plans 076/077/078 added the telegram connector health chip, an
in-app log viewer (Diagnostics settings section), and dropped the motion
library from the overlay bundle. the 080-085 UI batch added: news card
published-time meta + full-width expanded summary (080); the TTL progress bar
with `ttl_ms`/`remaining_ms` wire fields and the `SlotState::dedup_eq` rule —
continuously-varying wire fields must extend `dedup_eq`, never derived
`PartialEq` (081); weather-alert Meteocons art + mood backdrops (082);
football backend `EspnMeta`/crest cache/`espn_rich_events` + `assetProtocol`
scope (083); the live-match compact scorecard (084); and the `resting_state:
rail|notch` hide-when-idle flag (085). plans 086/087 spiked
(`docs/design/hover-cursor-tracking.md`) then shipped the hover primitive —
tracking area + rust-derived card rect + `hover-changed` event — but the hover
CONSUMER features (TTL-bar pause, weather peek, scorecard reveal, idle
expand-on-hover) are still unbuilt. plans 068/072/074 (landed 2026-07-21) were
mostly test backfill (`build_test_event`'s five per-source arms; weather
rain-lookahead minute-rounding and day-rollover boundaries), plus one small
defensive queue fix: a cross-tier Topic supersede now honors
`max_queued_per_tier` instead of skipping the cap check, dropping the fresh
content rather than evicting — this is **latent**: zero production behavior
change today, since no producer currently varies priority per Topic. plan 075
was a docs-only toolchain spike (TypeScript 7 / Vite 8 / Vitest 4 trial bump
in a throwaway worktree) — verdict was **GO**, but nothing was adopted:
`package.json` is untouched, adoption is a separate unwritten plan, and the
spike result lives in
`plans/done/075-frontend-toolchain-major-bump-spike(done).md`, not
`docs/design/`.
remaining open work: the
manual checklist rows in `docs/IMPLEMENTATION_PLAN.md` §6, and whatever
`plans/` holds.

`docs/archive/BLIND_REVIEW.md` and `docs/archive/CHANGES_SUMMARY.md` are
changelog/audit artifacts from the planning pass, not sources of
truth — the decisions they describe are already folded into the three
docs below. `docs/archive/V1_TECHNICAL_SPEC.md`,
`docs/archive/V2_TECHNICAL_SPEC.md`, and `docs/archive/V3_TECHNICAL_SPEC.md`
are likewise archived: those phases shipped, and `docs/V3_6_TECHNICAL_SPEC.md`
/ `docs/V5_TECHNICAL_SPEC.md` are the active working-draft specs now.

the dev machine is the mac mini (no notch), user `chetanjain`, home
`/Users/chetanjain`; the rust toolchain is installed. notch-mode
behaviour still needs per-change verification on the macbook.

## source of truth

`CONTEXT.md` at repo root is the glossary (ubiquitous language) —
terms like Promotion, Visible/Waiting, Paused, Presentation Mode. keep
code and doc edits consistent with it; it holds no implementation
details.

`docs/ARCHITECTURE.md` holds the locked decisions (scope phasing, tech
stack, cross-device behaviour, distribution model) — do not re-litigate
these without the user explicitly reopening them. `docs/IMPLEMENTATION_PLAN.md`
holds the phased build sequence and exit criteria for v1–v5.
`docs/TESTING_STRATEGY.md` holds the testing approach — frameworks, what's
tdd'd first vs written after, per-component test plan, and what's
deliberately left as manual-only verification. read all three before
starting implementation work.

`docs/V3_6_TECHNICAL_SPEC.md` and `docs/V5_TECHNICAL_SPEC.md` are v0
drafts that operationalize those three into code-level specifics for
the currently-active phases — exact file layout, struct/type shapes,
the `/notify` json schema, the `notchtap-detect` subprocess contract,
config/logging paths, error-to-status-code mapping. unlike
`ARCHITECTURE.md`, neither is locked — adjust them freely as
implementation surfaces friction. if a change there is actually a
*decision* change (a default, a scope boundary), make that edit in
`ARCHITECTURE.md` instead. the equivalent v1/v2/v3 specs are archived
at `docs/archive/` — those phases already shipped, so they're historical
records now, not active contracts (same status as `BLIND_REVIEW.md`/
`CHANGES_SUMMARY.md` above).

## commands (once scaffolded)

- `npm run tauri dev` — run the app in dev mode
- `npx tsc --noEmit` — typecheck the frontend
- `npx vite build` — build the frontend
- `npx biome check .` — frontend lint + format, local dev command
  (`npm run lint:fix` auto-applies); the enforcing gate CI and
  `just check-web` run is `npx biome ci .`
- `cargo build` (run from `src-tauri/`) — build the rust core; requires
  the rust toolchain (`rustup`) on the target mac
- `cargo test` (run from `src-tauri/`) — rust unit + integration tests
  (queue, event bus, `/notify` handler via axum/tower `oneshot`, notch/hud
  decision function)
- `npx vitest run` — frontend unit tests (visible-notification render
  state)
- `./notchtap --title "t" --body "b"` — manually trigger a notification
  against the local `/notify` endpoint (default `127.0.0.1:9789`,
  override via `--port` or `$NOTCHTAP_PORT`), for testing the
  queue/animation without a real event source. the cli is a committed
  shell script at repo root; besides flags, it also has a `run`
  subcommand (plan 058) — `notchtap run -- pnpm build` wraps a
  long-running command and pushes a completion card when it finishes
  (skipped for successful runs under `--min-secs`, default 15s; a
  failure always pushes)
- `just test-all` — one-command local verification mirroring
  `.github/workflows/ci.yml` exactly (see `justfile` at repo root for
  the full recipe list: `setup`, `dev`, `test-rust`, `check-rust`,
  `test-web`, `check-web`, `audit-web`, `build-web`, `check-cli`,
  `check-swift`). on a fresh clone, run `just setup` (`npm ci`) first —
  `test-all` does not install web deps for you.
  `just push "title" "body"` wraps the `./notchtap` cli call above.
  `just` is not installed on the dev machine yet — `brew install just`
  first.

`cargo test` and `npx vitest run` should both be clean before any phase
in `docs/IMPLEMENTATION_PLAN.md` is marked done — see that doc's §6 and
`docs/TESTING_STRATEGY.md` §7. there's no repo-wide coverage percentage
gate (see `docs/TESTING_STRATEGY.md` §6 for why) — the bar is "every example
case listed for the phase's components has a passing test," not a
coverage number. physical-hardware behaviour (notch geometry, hud
placement, animation look) stays a manual checklist by design —
`docs/TESTING_STRATEGY.md` §5 explains why those specific things aren't
worth automating.

## architecture (once scaffolded)

this is a tauri app: a rust core plus a react/ts webview ui, not
electron and not pure native swift (see `docs/ARCHITECTURE.md` §8 for why).

- **rust core** (`src-tauri/src/main.rs`) owns a local http listener on
  `127.0.0.1:9789` (`/notify`), a typed event bus, a fifo notification queue
  (capped concurrent visible items, per-item ttl), and window
  positioning. this is the only process that talks to the outside
  world (cli pushes, and in v2, the espn scoreboard poller and cmux's
  notification-command relay).
- **react/ts frontend** (`src/App.tsx`, `src/styles.css`) owns
  rendering only: it receives queued events via tauri's event system
  and renders them through an animation template. v1 has exactly one
  template (enter/hold/exit); v2 replaces this with a config table
  keyed by event type — that should stay a data change, not a new
  render path.
- **cross-device behaviour is a single runtime branch, not two
  builds.** the same compiled app runs on both the notch macbook and
  the notchless mac mini; a runtime check
  (`NSScreen.main?.safeAreaInsets.top > 0`) decides whether the window
  anchors over the notch cutout or floats as a top-center hud. do not
  fork this into separate build targets.
- **the swift↔rust boundary is a subprocess, not ffi.** the
  `NSScreen` check lives in a standalone swift cli (`notchtap-detect`)
  that prints json to stdout; the rust core shells out to it via
  `std::process::Command` and parses the result (`docs/ARCHITECTURE.md`
  §5). keep the pure decision logic (`fn presentation_mode
  (safe_area_top_inset: f64) -> Mode`) separate from that subprocess
  call — the function is unit-testable, the subprocess call is not
  (`docs/TESTING_STRATEGY.md` §4.4).
- **v1 has no approve/deny action.** notifications are display-only,
  auto-dismissed by ttl. do not add a "respond back into Codex"
  loop without reading `docs/ARCHITECTURE.md` §7 first — that requires
  Codex's own `PreToolUse`/`PermissionRequest` hooks, which is a
  deliberately separate, harder problem, out of scope until explicitly
  requested.

## naming

this project has no association with, and does not reference, any
third-party app's name, branding, or code. keep it that way in
identifiers, comments, and commit messages — use the product name
(`notchtap`), this repo's own name (`mac-notification-nudge`), or
generic terms (the engine, the cli, the notify endpoint).

## ipc & security (once scaffolded)

tauri v2 uses a capabilities/permissions system. the frontend in this app
is **receive-only** in v1 — it listens for a single custom event from
the rust core (`notification-promoted`, emitted exactly once per item
at promotion time, never at enqueue) and renders it. there are no
frontend-to-rust invoke commands in v1.

the `src-tauri/capabilities/default.json` should be locked down to the
minimum: one permission for the custom event channel, no file-system
access, no shell access, no network access from the frontend. the
frontend should not be able to trigger notifications — only display what
the rust core sends it.

**v5 amendment (rust side built 2026-07-17)**: the receive-only
rule above now applies to the *overlay* window (`main`), permanently.
the settings window (`settings` label, `src-tauri/src/settings.rs`)
has eleven
invoke commands (see `CLAUDE.md`'s receive-only section for the full
list), gated per-window. critical tauri v2 gotcha: app-
defined commands are allowed to **every** window by
default — the gate only exists with the `tauri_build::AppManifest::
commands` opt-in in `build.rs` plus a dedicated
`capabilities/settings.json`. never add a `#[tauri::command]` without
also adding it to that `build.rs` list; `default.json` must never
change. full contract: `docs/V5_TECHNICAL_SPEC.md` §2.

## rust error handling

- **library/internal modules** (queue, event bus, event types): use
  `thiserror` for structured, matchable error variants. tests should be
  able to assert `matches!(err, MyError::QueueFull)`.
- **application boundary** (main.rs, HTTP handlers, CLI entrypoint): use
  `anyhow` for ergonomic error propagation. the HTTP layer returns
  specific status codes (400 for malformed json, 429 for queue full,
  500 for unexpected), but the internal error type doesn't need to leak
  into every function signature.

this split is standard in the rust ecosystem and matches the testing
strategy: unit tests match on `thiserror` variants; integration tests
assert on HTTP status codes.
