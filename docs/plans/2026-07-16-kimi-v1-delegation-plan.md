# plan: implement notchtap v1 (phase 1)

## what this is

implement v1 of "notchtap" — a macOS-only tauri app (rust core +
react/ts webview) that shows animated notifications fed by a local http
endpoint — exactly as specified by the four docs in
`/tmp/kimi-delegation-tmp/notchtap-v1/work/docs/` plus the glossary at
`work/CONTEXT.md` and the project rules in `work/CLAUDE.md`. these are
current as of 2026-07-16 and are the complete, authoritative contract.
`docs/V1_TECHNICAL_SPEC.md` is the code-level spec — follow it
section by section. where the spec and this plan differ, the spec wins;
where the spec is silent, this plan wins; if both are silent and the
choice matters, stop and log it rather than guessing (see "blocked
protocol" below).

## boundary (absolute)

- everything you do happens inside `/tmp/kimi-delegation-tmp/notchtap-v1/`.
  the project you build lives at
  `/tmp/kimi-delegation-tmp/notchtap-v1/work/` (this becomes the future
  repo root — app at root alongside `docs/`, per `work/CLAUDE.md`).
- never read or write any path outside that directory. no `$HOME`
  files, no `~/.config`, no `~/Library`, no `/usr/local/bin`, no other
  repos. (build tools writing to their own caches — npm, cargo
  registry — is fine; that's implicit tool behavior, not you reaching
  out.)
- network access IS allowed and required: npm registry and crates.io
  for dependency resolution. nothing else.
- dependency installs ARE allowed, but only what
  `docs/V1_TECHNICAL_SPEC.md` §13 lists plus whatever the tauri
  scaffold itself brings. do not add other crates/packages.

## toolchain facts (verified on this machine, don't re-derive)

- node v22.22.2, npm 10.9.7
- rust: stable via rustup at `$HOME/.cargo/bin` — already on your PATH
  when launched. invoke as plain `cargo`.
- swift 6.2.4 (xcode toolchain present), arm64, macOS 26.5
- `jq` 1.7.1 and `curl` present (for the cli script §12)
- `npm run tauri dev` / any GUI launch is FORBIDDEN in this run — you
  are headless; a GUI would hang you. verification is builds + tests
  only. same for `npm run tauri build` (no bundling in this run).

## order of operations

### 1. read the contract
read fully, in this order: `work/docs/ARCHITECTURE.md`,
`work/docs/IMPLEMENTATION_PLAN.md` (§1 is the phase you're building),
`work/docs/TESTING_STRATEGY.md`, `work/docs/V1_TECHNICAL_SPEC.md`,
`work/CONTEXT.md`, `work/CLAUDE.md`.

### 2. scaffold
- scaffold a tauri v2 react+typescript app non-interactively. attempt:
  `cd /tmp/kimi-delegation-tmp/notchtap-v1/work && npm create tauri-app@latest notchtap-scaffold -- --template react-ts --manager npm --yes`
  if the flags differ in the current create-tauri-app, check
  `npm create tauri-app@latest -- --help` and adapt — but it MUST stay
  non-interactive (an interactive prompt will hang forever). use
  identifier `com.chetanjain.notchtap` if an identifier flag exists.
- move the scaffold's entire contents (including dotfiles) up into
  `work/` so the app lives at `work/` root next to `docs/`, then remove
  the empty scaffold dir. if a filename collides (e.g. README.md),
  keep the existing `work/` file and note the collision in result.md.
- `cd work && npm install`
- `git init` in `work/`, `git add -A`, commit as `scaffold baseline`.
  (this exists so the reviewer can diff your work against the pristine
  scaffold. commit again at the end as `v1 implementation`.)
- prove the baseline compiles before writing feature code:
  `npx tsc --noEmit` and `npx vite build` from `work/`, and
  `cargo build` from `work/src-tauri/`. all three must pass on the
  untouched scaffold first.

### 3. rust core (`work/src-tauri/src/`)
implement per `V1_TECHNICAL_SPEC.md`, module for module (§1 layout):
`main.rs`, `http.rs`, `event.rs`, `queue.rs`, `presentation.rs`,
`config.rs`, `error.rs`, `logging.rs`. the non-negotiables, all from
the spec:

- axum `POST /notify` on `127.0.0.1:<port>` (default 9789), loopback
  only. responses exactly per §3's table: 200 `{"status":"accepted"}`,
  202 `{"status":"paused","queued":n}` while paused, 400, 429, 500.
  `DefaultBodyLimit` 64 KB (oversize → axum's built-in 413).
- listener-ready gate (§3): bind axum only AFTER the main webview has
  loaded (tauri page-load callback / ready event). no frontend→rust
  invokes — the signal is observed rust-side.
- `NotificationQueue` (§4): fifo, `max_concurrent`/`max_queued`,
  ttl from `promoted_at` (never `enqueued_at`), `pause()`/`resume()`/
  `is_paused()` — pause gates promotion only, expiry keeps running,
  enqueue-while-paused always goes to `waiting`, 429 enforced
  identically while paused. resume's caller immediately calls
  `expire_and_promote`.
- 250ms tokio interval heartbeat in `main.rs` calling
  `expire_and_promote(Instant::now())` (§4). no "lazily on access"
  path.
- emit rule (§4/§7): exactly one `notification-promoted` event per
  item, at promotion, wherever promotion happens (enqueue fast-path,
  heartbeat tick, resume). enqueue into waiting is silent. payload is
  the dedicated `NotificationPayload` struct (camelCase, `ttlSecs`).
- `presentation_mode(f64) -> Mode` pure fn + `notchtap-detect`
  subprocess call via `Command::new(&config.detect_path)` (absolute
  path from config, default `/usr/local/bin/notchtap-detect`, NEVER a
  bare PATH lookup). one fallback rule at the call site: any failure →
  warn-log + `Mode::Hud`, never panic. log resolved mode + raw inset
  at info level at startup (§5).
- config (§9): `~/.config/notchtap/config.toml` semantics — missing
  file = all defaults, malformed = fail fast. `port`, `default_ttl`,
  `max_concurrent`, `max_queued`, `detect_path` with serde defaults.
  IMPORTANT: unit tests must construct `Config` values directly and
  must never read or write the real `~/.config` or `~/Library` —
  nothing in `cargo test` may touch `$HOME`.
- logging (§10): `tracing` + `tracing-appender`, rolling file config
  pointed at `~/Library/Logs/notchtap/` in the app code path only —
  again, tests must not initialize file logging.
- tray (§6): tauri `TrayIconBuilder` (`tray-icon` feature), exactly
  two items — pause (label flips to resume; wired to queue
  pause/resume + immediate `expire_and_promote` on resume) and quit.
  default scaffold icon.
- window & background (§6): `src-tauri/Info.plist` with
  `LSUIElement=true`; ALSO `app.set_activation_policy(ActivationPolicy::Accessory)`
  in setup; `always_on_top(true)` on the main window;
  `bundle.macOS.minimumSystemVersion = "13.0"` in `tauri.conf.json`.
- login item (§6): `smappservice` crate calling
  `SMAppService.mainApp.register()` once at startup; skip with an
  info log when not running as a bundled .app. FALLBACK RULE: if
  `cargo add smappservice` fails or the crate doesn't compile, do NOT
  substitute tauri-plugin-autostart or AppleScript — isolate the call
  in its own small module, stub it with a clear `// TODO` + info log,
  and record exactly what failed in result.md.
- errors (§11): `thiserror` for `QueueError`/`EventError` internals,
  `anyhow` at the http/main boundary, status mapping per §11's table.
- capabilities (§7): `capabilities/default.json` with exactly
  `["core:event:default"]` for window `main`. verify the frontend
  `listen` actually works under that permission (the vitest tests mock
  the api, so verify by checking the tauri docs/schema for the
  resolved tauri version — if `core:event:default` doesn't cover
  `listen`, add the minimal event-listen permission and note it in
  result.md).

### 4. rust tests
per `TESTING_STRATEGY.md` §4.1–4.4 — every listed example case gets a
test, including the new ones (pause cases in §4.1, the 202 case in
§4.3, the real-bind loopback test distinct from the `oneshot` suite).
http handler tests via `tower::ServiceExt::oneshot` (add `tower` with
`util` feature as dev-dependency). tests must assert on `thiserror`
variants with `matches!` where the strategy says so.
`cargo test` from `work/src-tauri/` must pass, all tests, no ignored.

### 5. frontend (`work/src/`)
per spec §8: `useVisibleNotifications.ts` (render-state list, NOT a
queue — enter 300ms → hold `ttlSecs` → exit 300ms → removed),
`App.tsx` renders the visible stack, `styles.css` holds the one
enter/hold/exit keyframe animation. listens for
`notification-promoted` via `@tauri-apps/api/event`.
tests per `TESTING_STRATEGY.md` §4.5 with vitest +
`@testing-library/react` + `@testing-library/jest-dom` (add as
dev-dependencies; mock `@tauri-apps/api/event`, use fake timers for
ttl). `npx vitest run` from `work/` must pass.

### 6. cli script
`work/notchtap` — committed executable shell script per spec §12:
flags only (`--title`, `--body`, required; `--subtitle`, `--port`,
optional), jq -n --arg for json, curl POST, subtitle folds into body
as `"<subtitle> — <body>"`, port resolution `--port` → `$NOTCHTAP_PORT`
→ 9789, non-zero exit on connection failure or non-2xx. `chmod +x`.
test it against a dummy local server if convenient (e.g. `nc` or a
tiny node http server on a high port, inside the work dir) or at
minimum shell-parse it (`sh -n`) and verify the jq expression output.

### 7. swift detector
`work/notchtap-detect/` — swift package per spec §5:
swift-tools-version 5.9, single executable target `notchtap-detect`,
Foundation/AppKit only, prints `{"safe_area_top_inset": <f64>}` to
stdout, exit 0. build with `swift build -c release` and RUN the built
binary once — it must print valid json (this machine has no notch
context in headless mode; any valid json + exit 0 is a pass). do NOT
install/symlink it anywhere outside the work dir.

### 8. final verification (all must be clean, in this order)
from `work/src-tauri/`: `cargo build`, then `cargo test`.
from `work/`: `npx tsc --noEmit`, `npx vite build`, `npx vitest run`.
`work/notchtap-detect`: binary exists and prints valid json.
then `git add -A && git commit -m "v1 implementation"` in `work/`.

## out of scope — do not do

- running the GUI (`tauri dev`) or bundling (`tauri build`)
- installing anything to `/usr/local/bin` or registering login items
  for real (the code path exists; executing it against the real OS
  does not happen in this run)
- touching the real repo at `~/Desktop/code/mac-notification-nudge`
- editing the docs in `work/docs/` (they're the contract, read-only
  for you) — exception: if implementation genuinely contradicts
  `V1_TECHNICAL_SPEC.md`, leave the doc alone and record the
  divergence in result.md instead
- adding dependencies beyond spec §13 + scaffold defaults
- deleting or rewriting `work/CONTEXT.md`, `work/CLAUDE.md`,
  `work/README.md`

## blocked protocol

if you hit something the plan and spec don't cover — a missing file, a
failing test you can't fix within the spec's constraints, a
contradiction between steps, create-tauri-app refusing every
non-interactive path — STOP. write what blocked you to
`/tmp/kimi-delegation-tmp/notchtap-v1/result.md` with `Status: BLOCKED`
as the first line, and end output with the sentinel per your launch
instructions. do not guess around a contradiction.

## result.md (required, at /tmp/kimi-delegation-tmp/notchtap-v1/result.md)

first line: `Status: DONE` (or BLOCKED/FAILED). then: files created or
modified (paths), every command run for verification with its outcome,
anything skipped or stubbed (especially the smappservice fallback if
taken), any divergence from the spec and why, test counts
(rust + vitest).
