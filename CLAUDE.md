# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## project state

scaffolded and shipping through v6. per-plan history is NOT duplicated
here — read `plans/done/` (filed, one file per plan) and `git log` for
what landed when. the notes below are only the things those sources
don't tell you.

- the docs folder isn't part of the app build; the tauri/rust/web
  project lives at repo root alongside `docs/`.
- the test suite must stay green (`cargo test` from `src-tauri/`,
  `npx vitest run` from repo root, all gated by ci). current test
  counts live in `docs/TESTING_STRATEGY.md` §0 and only there — don't
  restate them anywhere else.
- **`SlotState::dedup_eq` rule:** continuously-varying wire fields
  (e.g. `ttl_ms`/`remaining_ms`) must extend `dedup_eq` explicitly and
  must never rely on derived `PartialEq` — deriving it causes every
  tick to read as a content change.
- the hover primitive shipped (tracking area, rust-derived card rect,
  `hover-changed` event) and all four hover CONSUMER features shipped
  in plan 093: TTL-bar hover-pause, idle weather peek, scorecard
  reveal-on-hover, idle expand-on-hover.
- the frontend toolchain spike (TypeScript 7 / Vite 8 / Vitest 4)
  returned a **GO** verdict, but nothing was adopted — `package.json`
  is untouched and adoption is a separate unwritten plan. don't read
  the GO as "already done".
- remaining open work: the manual checklist rows in
  `docs/IMPLEMENTATION_PLAN.md` §6, and whatever `plans/` holds.

<!-- trimmed 2026-07-21: a plan-by-plan changelog (v1–v6, plans
037–087) lived here. it was reconstructible from plans/done/ and git
log, so it was cut to keep this file cheap to load every session. -->

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

standard invocations (`npm run tauri dev`, `npx tsc --noEmit`,
`npx vite build`, `cargo build`/`cargo test` from `src-tauri/`,
`npx vitest run`) are in `package.json` and the `justfile` — read those.
the non-obvious ones:

- `npx biome check .` is the local dev command (`npm run lint:fix`
  auto-applies), but the enforcing gate CI and `just check-web` run is
  `npx biome ci .` — they are not interchangeable.
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
  auto-dismissed by ttl. do not add a "respond back into claude code"
  loop without reading `docs/ARCHITECTURE.md` §7 first — that requires
  claude code's own `PreToolUse`/`PermissionRequest` hooks, which is a
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

**v5 settings window is the one exception, and it's opt-in-gated,
not default-safe.** tauri v2 grants app-defined commands to *every*
window by default — the settings window's seven invoke commands
(`get_config`, `get_default_config`, `get_secret_status`,
`save_config_and_relaunch`, `set_secret`, `send_test_notification`,
`set_appearance`) are scoped to it alone only because `src-tauri/build.rs`
opts into `tauri_build::AppManifest::commands(&[...])` (deny-by-default)
plus a dedicated `capabilities/settings.json`. never add a new
`#[tauri::command]` without adding it to that `build.rs` list —
otherwise it silently becomes callable from the overlay (`main`)
window too, breaking the receive-only guarantee above.
`capabilities/default.json` must never change. full contract:
`docs/V5_TECHNICAL_SPEC.md` §2.

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
