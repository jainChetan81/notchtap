# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## project state

scaffolded and shipping. v1 (core engine, queue, one animation, cli
push), v2 (espn poller, cmux relay, animation table), v3 (outbound
connectors — telegram, `src-tauri/src/notifier.rs`), and v4
(github + ci at `github.com/jainChetan81/notchtap`) are done as of
2026-07-16. the tauri/rust/web project lives at repo root alongside
`docs/` — the docs folder isn't part of the app build. the test suite
exists and must stay green (`cargo test` from `src-tauri/`, `npx
vitest run` from repo root, all gated by ci) — current counts live in
`docs/TESTING_STRATEGY.md` §0 and only there.

`docs/archive/BLIND_REVIEW.md` and `docs/archive/CHANGES_SUMMARY.md` are
changelog/audit artifacts from the planning pass, not sources of
truth — the decisions they describe are already folded into the three
docs below.

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
holds the phased build sequence and exit criteria for v1/v2/v3.
`docs/TESTING_STRATEGY.md` holds the testing approach — frameworks, what's
tdd'd first vs written after, per-component test plan, and what's
deliberately left as manual-only verification. read all three before
starting implementation work.

`docs/V1_TECHNICAL_SPEC.md` is a v0 draft that operationalizes those
three into code-level specifics for v1 only — exact file layout,
struct/type shapes, the `/notify` json schema, the `notchtap-detect`
subprocess contract, config/logging paths, error-to-status-code
mapping. unlike `ARCHITECTURE.md`, it isn't locked — adjust it freely
as implementation surfaces friction. if a change there is actually a
*decision* change (a default, a scope boundary), make that edit in
`ARCHITECTURE.md` instead.

## commands (once scaffolded)

- `npm run tauri dev` — run the app in dev mode
- `npx tsc --noEmit` — typecheck the frontend
- `npx vite build` — build the frontend
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
  queue/animation without a real event source. flags only — there is no
  positional form; the cli is a committed shell script at repo root

consider adding a `justfile` (or `Makefile`) after scaffold with
recipes: `dev`, `test-rust`, `test-web`, `test-all`, `build`, `push
"title" "body"`. this prevents "oops i ran `vitest` from `src-tauri/`"
errors.

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
