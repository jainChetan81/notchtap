# notchtap — v1 technical spec (v0 draft)

this is the concrete, code-level contract for v1 (`IMPLEMENTATION_PLAN.md`
§1). `ARCHITECTURE.md` holds the *locked* decisions (scope, stack,
defaults, distribution model) — this doc doesn't re-decide any of
those. what it adds is everything `ARCHITECTURE.md` left at the
"design decision" level: exact struct/type shapes, file layout, json
schemas, function signatures. that's a **v0 draft**, not locked —
unlike `ARCHITECTURE.md`, adjust anything here freely the moment
implementation disagrees with it. if a *decision* changes (a default
value, a scope boundary), that edit belongs in `ARCHITECTURE.md`
instead, per usual.

read `ARCHITECTURE.md` §§1–7 and `TESTING_STRATEGY.md` §4.1–4.5 first —
this doc assumes both.

**queue authority, resolved**: `ARCHITECTURE.md` §2's diagram draws the
notification queue inside the rust core engine; `IMPLEMENTATION_PLAN.md`
§1.3 separately describes queue-like behavior (fifo, cap, ttl-dismiss)
under its frontend bullet. that's a pre-existing ambiguity between the
two upstream docs, not a decision either one actually made explicitly.
this doc resolves it: **rust's `NotificationQueue` (§4) is the sole
source of truth** for cap enforcement, fifo ordering, and
promotion/eviction. the frontend (§8) is not a second queue — it never
decides what's visible or when something's evicted; it only renders
whatever rust chooses to emit, driving its own enter/hold/exit
animation clock off the `ttlSecs` value each event carries. see §8 for
the reasoning on why that's still safe (both sides share the same ttl
value, so no separate "evict" signal is needed).

---

## 0. scope

v1 only: engine + fifo queue + one css animation + cli push (manual
and cmux-relayed, same endpoint). no espn poller, no animation table,
no posture module, no outbound connectors — those are v2/v3, not
covered here.

---

## 1. project layout

after `npm create tauri-app@latest` (react + typescript template):

```
src-tauri/
  Cargo.toml
  capabilities/
    default.json
  src/
    main.rs        — two-line binary shim calling notchtap_lib::run()
                      (tauri template convention)
    lib.rs          — the actual entrypoint wiring: tracing init,
                      config load, axum server + tauri app boot,
                      window setup, tray (§6), 250ms promotion
                      heartbeat (§4)
    http.rs         — axum router, POST /notify handler
    event.rs        — Event, EventType, dispatch/routing
    queue.rs        — NotificationQueue (fifo, cap, ttl)
    presentation.rs — Mode enum, presentation_mode() pure fn,
                      notchtap-detect subprocess call + json parse
    config.rs        — Config struct, load from
                      ~/.config/notchtap/config.toml
    error.rs        — thiserror variants (queue/event/config) +
                      IntoResponse impl for the http boundary
    logging.rs       — tracing-subscriber + rotating file appender setup

src/
  App.tsx            — tauri event listener, renders visible notifications
  useVisibleNotifications.ts — render state: enter/hold/exit per item
  styles.css          — the one v1 animation (keyframes)

notchtap              — cli push script (shell, jq + curl — §12),
                         committed executable at repo root

notchtap-detect/      — standalone swift package (separate build,
                         see §5). not part of the cargo/npm build graph.
```

`notchtap-detect` is its own swift package at repo root, built
independently (`swift build -c release`) **on each target machine**
and invoked via an absolute path (`detect_path` config field, §9) —
the rust core never links or embeds it, only shells out
(`ARCHITECTURE.md` §5). do not rely on `PATH` lookup: see §5.

---

## 2. data model

```rust
// event.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub priority: Priority,
    pub ttl_secs: u64,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic, // v1's only variant. v2 adds ScoreUpdate, PostureAlert, etc.
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Normal, // v1 always sends this — the field exists so v2 sources
            // (e.g. espn) don't need a queue/event schema migration.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub title: String,
    pub body: String,
}
```

the `EventType`/`Priority` enums exist in v1 with exactly one variant
each, deliberately — this is what lets `event.rs`'s dispatch/routing
logic be unit-tested against "unknown type" today (`TESTING_STRATEGY.md`
§4.2) without v2's variants existing yet.

```rust
// queue.rs
pub struct QueueItem {
    pub event: Event,
    pub enqueued_at: Instant,        // when it entered `waiting` — fifo
                                      // ordering only, never used for ttl
    pub promoted_at: Option<Instant>, // when it moved into `visible`;
                                       // None while still waiting
}

pub struct NotificationQueue {
    visible: VecDeque<QueueItem>, // len <= max_concurrent
    waiting: VecDeque<QueueItem>, // len <= max_queued
    max_concurrent: usize,
    max_queued: usize,
    paused: bool, // starts false; in-memory only, never persisted
}
```

**ttl clock**: `ARCHITECTURE.md` §3 defines ttl as "time from
enter-complete to exit-start" — i.e. it starts once the item is
actually visible and done animating in, not when it was first
enqueued. an item sitting in `waiting` must not burn its ttl before
ever being shown. `promoted_at` is set the instant an item moves
`waiting` → `visible`; `expire_and_promote` measures ttl from
`promoted_at`, not `enqueued_at`. v1 doesn't track "enter animation
finished" as a distinct server-side event — `promoted_at` is used as
an approximation of enter-complete, which is accurate to within
`enter_duration` (300ms, negligible against an 8s ttl). `enqueued_at`
exists purely for fifo tie-breaking among `waiting` items.

```rust
// presentation.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Notch,
    Hud,
}
```

---

## 3. http api

single route, per `ARCHITECTURE.md` §7 / `IMPLEMENTATION_PLAN.md` §1.2:

`POST /notify` on `127.0.0.1:9789`, bound loopback-only.

**request body**:

```json
{ "title": "string, required", "body": "string, required" }
```

no `type`/`priority`/`ttl` fields accepted from the client in v1 — the
http handler always constructs `EventType::Generic`,
`Priority::Normal`, and `ttl_secs` from `Config::default_ttl`. this
keeps the wire contract exactly matching the cli (`notchtap --title
<t> --body <b>`, §12) and the cmux relay's three env vars
(title/subtitle/body — the cli folds a non-empty subtitle into `body`,
no separate wire field).

**responses**:

| status | condition |
|---|---|
| `200` | accepted while running normally — body `{"status": "accepted"}` |
| `202` | accepted while paused — buffered into `waiting`, promotion deferred until resume; body `{"status": "paused", "queued": <waiting count>}` |
| `400` | malformed json, or missing `title`/`body` |
| `429` | queue at `max_queued` (50) — enforced identically while paused |
| `500` | unexpected internal error |

**startup ordering (listener-ready gate)**: tauri events are
transient — an emit with no registered listener is silently dropped.
a `/notify` accepted before the webview has registered its
`notification-promoted` listener would return `200` with nothing ever
rendered — exactly at the first-launch moment the pipe gets tested
(2026-07-16 consensus review's top pipe-reliability risk). rule: bind
the axum listener only **after** the main webview has loaded — wire
the bind to tauri's page-load callback / ready event in `main.rs`.
before that point the cli gets connection-refused, an honest visible
failure, instead of a silent 200-drop. this keeps the frontend
strictly receive-only: no frontend→rust "ready" invoke is needed,
the page-load signal is observable from the rust side.

**body size limit**: set axum's `DefaultBodyLimit` explicitly small
(64 KB — a title+body never legitimately approaches it); oversized
bodies get axum's built-in `413`. loopback is not an auth boundary
against other local processes (`ARCHITECTURE.md` §7), so don't accept
unbounded input.

these map directly to `TESTING_STRATEGY.md` §4.3's example cases —
each row above should have exactly one integration test via
`tower::ServiceExt::oneshot`, **except** the loopback-bind-address
case: `oneshot` calls the router in-process and never binds a real
socket, so it can't prove the listener refuses non-loopback
connections. that one case needs a separate test that actually calls
`TcpListener::bind(("127.0.0.1", port))` and asserts a bind to
`0.0.0.0`/a non-loopback address is not what's happening — a real,
if minimal, integration test distinct from the `oneshot` suite.

**internal validation** (closes the testability gap for
`TESTING_STRATEGY.md` §4.2's "unknown type"/"missing field" cases,
which v1's http wire format can't exercise directly since it never
accepts a `type` field from the client):

```rust
// event.rs
pub fn dispatch(event: Event) -> Result<(), EventError>;
```

`TESTING_STRATEGY.md` §4.2's cases are unit tests against `dispatch`
directly — constructing `Event { event_type: EventType::Generic, .. }`
and (once v2 adds variants) an out-of-range type — not integration
tests through the http layer, since v1's `/notify` handler always
constructs `EventType::Generic` itself and has no way to receive an
"unknown" type from a client.

---

## 4. queue engine

```rust
impl NotificationQueue {
    pub fn new(max_concurrent: usize, max_queued: usize) -> Self;

    /// enqueues; promotes to `visible` immediately only if there's a
    /// free slot, the queue is not paused, *and* `waiting` is empty —
    /// a new push must never jump ahead of older waiting items (fifo).
    /// otherwise appends to `waiting` with `promoted_at = None`.
    /// while paused it always appends to `waiting`, even with a free
    /// visible slot. errors if `waiting` is already at `max_queued` —
    /// paused or not.
    pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError>;

    /// called from the 250ms heartbeat (see below): removes any
    /// `visible` item whose ttl has elapsed since `promoted_at`,
    /// then — unless paused — promotes the oldest `waiting` items into
    /// freed slots, setting their `promoted_at = Some(now)`. expiry
    /// runs even while paused; only promotion is gated.
    pub fn expire_and_promote(&mut self, now: Instant);

    /// pause/resume (tray, §6). pause disables promotion only. the
    /// caller of `resume` must immediately follow with
    /// `expire_and_promote(now)` so un-pausing promotes without
    /// waiting for the next tick.
    pub fn pause(&mut self);
    pub fn resume(&mut self);
    pub fn is_paused(&self) -> bool;

    pub fn visible(&self) -> &VecDeque<QueueItem>;
}
```

defaults come from `ARCHITECTURE.md` §3 (`ttl=8s`, `max_concurrent=3`,
`max_queued=50`) — read from `Config` at startup, not hardcoded.

**promotion heartbeat**: `main.rs` runs a tokio `interval` that calls
`expire_and_promote(Instant::now())` every **250ms**. an earlier draft
allowed "lazily on access" as an alternative — that was broken: access
only happens when a new push arrives, so once pushes stop, a waiting
item would never be promoted and visible items would never expire
server-side. worst-case expiry lateness (~250ms) is invisible against
an 8s ttl and 300ms animations.

**emit rule**: exactly one `notification-promoted` event (§7) is
emitted per item, at the moment it's promoted `waiting` → `visible` —
whether that promotion happened inside `enqueue` (free slot, not
paused), on a heartbeat tick, or on resume. enqueue into `waiting` is
silent; the frontend never hears about waiting items.

**pause semantics** (`ARCHITECTURE.md` §3): pause gates promotion and
nothing else. visible items keep aging out on their normal ttl, waiting
items accumulate (still capped at `max_queued`), and the http layer
answers `202` instead of `200` (§3). in-memory only — the app always
launches unpaused.
`max_queued` bounds the `waiting` list only — visible items never
count against it (so up to 3 + 50 items can exist at once).

---

## 5. presentation mode / notch detection

**pure decision function** (`TESTING_STRATEGY.md` §4.4 — unit-tested
directly, no native call involved):

```rust
pub fn presentation_mode(safe_area_top_inset: f64) -> Mode {
    if safe_area_top_inset > 0.0 { Mode::Notch } else { Mode::Hud }
}
```

**`notchtap-detect` subprocess contract** (the untestable boundary
that feeds the function above):

- invocation: `Command::new(&config.detect_path).output()`, no args —
  an absolute path (default `/usr/local/bin/notchtap-detect`,
  overridable via `detect_path` in the config file, §9). never a bare
  `PATH` lookup: gui/login-item-launched apps get a minimal
  environment (`/usr/bin:/bin:/usr/sbin:/sbin`), so
  `Command::new("notchtap-detect")` would hit the binary-missing
  fallback on every real launch while working fine from a terminal —
  the worst kind of silent divergence (2026-07-16 consensus review)
- stdout (well-formed case):
  ```json
  { "safe_area_top_inset": 0.0 }
  ```
- exit code `0` on success; non-zero on any internal swift-side failure
- **fallback rule** (applies to malformed json, non-zero exit, *and*
  binary-not-found-on-`PATH`): log a warning via `tracing`, fall back
  to `Mode::Hud`, never panic. this one fallback rule covers all three
  failure cases from `TESTING_STRATEGY.md` §4.4 — implement it once at
  the call site that wraps the subprocess call, not duplicated per
  failure type.
- **startup logging**: log the resolved mode and the raw inset at
  `info` level as one of the first startup lines. the hud fallback is
  deliberately silent-safe, which means a broken or missing detector
  on the macbook would still "pass" v1's visible-notification exit
  criterion in hud mode — this log line is the only thing that makes
  that failure visible (2026-07-16 consensus review). the manual
  checklist (`IMPLEMENTATION_PLAN.md` §6) verifies it on the macbook.

**build & install** (was left open in an earlier draft of this doc —
resolved here since it's a prerequisite for `IMPLEMENTATION_PLAN.md`
§1.2's exit criteria, not a detail that can wait until then):

- `notchtap-detect/Package.swift` — a minimal swift-tools-version 5.9
  executable package, single target `notchtap-detect`, no external
  dependencies (just `Foundation`/`AppKit` for `NSScreen`)
- build: `cd notchtap-detect && swift build -c release`
- output binary: `notchtap-detect/.build/release/notchtap-detect`
- install (v1, manual step — not yet scripted, run on **each** target
  machine):
  `ln -sf "$(pwd)/notchtap-detect/.build/release/notchtap-detect" /usr/local/bin/notchtap-detect`
  — the symlink target must match whatever `detect_path` the config
  resolves to (default `/usr/local/bin/notchtap-detect`). a build
  script (`justfile`/`Makefile` recipe, per `CLAUDE.md`) that
  automates this is a natural v1-polish follow-up, not a blocker.

---

## 6. window & background setup

`ARCHITECTURE.md` §6 locks three things as v1 day-one, not deferred
polish: `LSUIElement = true` (no dock icon), login-item registration
via `SMAppService.mainApp.register()` (macOS 13+), and always-on-top.
none of these were operationalized anywhere in an earlier draft of
this doc — that's fixed here.

- **`LSUIElement`**: tauri's macOS bundler merges a partial
  `src-tauri/Info.plist` into the generated app bundle's `Info.plist`
  at build time. create that file with:
  ```xml
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
    <key>LSUIElement</key>
    <true/>
  </dict>
  </plist>
  ```
  the plist key only applies to the bundled `.app` — `tauri dev` runs
  an unbundled binary where it has no effect. so *also* set the policy
  programmatically in `main.rs` setup:
  `app.set_activation_policy(tauri::ActivationPolicy::Accessory)`.
  both together cover dev and release; without the programmatic call,
  dock-icon behaviour would be unverifiable until final packaging.
- **login item**: call `SMAppService.mainApp.register()` in-process
  via the `smappservice` crate (the `smappservice-rs` project), once
  at startup in `main.rs`. an earlier draft said to use
  `tauri-plugin-autostart` "because it wraps SMAppService" — that was
  factually wrong (caught in the 2026-07-16 consensus review,
  `docs/review-logs/`): the plugin actually uses AppleScript (System
  Events permission popups) or a LaunchAgent plist, neither of which
  is the `SMAppService` mechanism `ARCHITECTURE.md` §6 locks. calling
  the api in-process means `SMAppService.mainApp` resolves to the
  running app bundle correctly. two caveats: (1) registration is only
  meaningful when running as the bundled `.app` — in `tauri dev`
  (unbundled binary) skip it with an info-level log line, don't
  error; (2) if the crate proves broken at implementation time, the
  fallback is a small swift helper embedded in the app bundle (same
  subprocess pattern as `notchtap-detect`) — not the autostart
  plugin.
- **always-on-top**: set on the main window at creation, via the
  `WebviewWindowBuilder`:
  ```rust
  // main.rs, window setup
  WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
      .always_on_top(true)
      // ...other builder calls (size, position, decorations)
      .build()?;
  ```
- **tray (quit + pause)**: `ARCHITECTURE.md` §6 locks a menu-bar tray
  with exactly two items. build it with tauri's tray api
  (`TrayIconBuilder`; needs the `tray-icon` feature on the `tauri`
  dependency). "pause" calls `NotificationQueue::pause` and flips its
  own label to "resume"; selecting it again calls `resume()` followed
  immediately by `expire_and_promote` (§4) so un-pausing promotes
  without waiting for the next tick. "quit" exits the app. default
  tauri icon until the real-icon polish item (`IMPLEMENTATION_PLAN.md`
  §4).
- **macOS 13+ enforcement**: set
  `bundle.macOS.minimumSystemVersion = "13.0"` in `tauri.conf.json`.
  this is enforced by launchservices/gatekeeper at the os level — the
  app won't launch on macOS 12 at all, so there's no runtime version
  check or custom error message to write in rust.

---

## 7. tauri ↔ frontend contract

- rust emits a single custom event: `notification-promoted` — exactly
  one emit per item, at the moment of promotion (§4's emit rule);
  enqueue is silent. (earlier drafts named this `notification-received`;
  renamed because "received" describes enqueue — precisely the moment
  that does *not* emit.)
- wire payload is a distinct, purpose-built struct — not `Event`
  serialized directly — so the rust↔ts field naming is explicit rather
  than incidental:
  ```rust
  // event.rs (or a small dedicated wire.rs)
  #[derive(Debug, Clone, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct NotificationPayload {
      pub id: Uuid,
      pub title: String,
      pub body: String,
      pub ttl_secs: u64, // serializes as "ttlSecs"
  }
  ```
- ts side (matches the json produced above):
  ```ts
  type NotificationPayload = {
    id: string;
    title: string;
    body: string;
    ttlSecs: number;
  };
  ```
- `src-tauri/capabilities/default.json` — locked to the minimum per
  `ARCHITECTURE.md` §14 / `CLAUDE.md`'s ipc & security section:
  ```json
  {
    "$schema": "../gen/schemas/desktop-schema.json",
    "identifier": "default",
    "windows": ["main"],
    "permissions": ["core:event:allow-listen", "core:event:allow-unlisten"]
  }
  ```
  no `core:path:*`, `core:fs:*`, `shell:*`, or `http:*` permissions —
  the frontend never invokes back into rust in v1. listen/unlisten
  only, not `core:event:default` (which would also grant emit to a
  receive-only frontend).
- `tauri.conf.json` sets a csp with `connect-src` limited to tauri's
  ipc endpoints (`ipc: http://ipc.localhost`), so the webview cannot
  `fetch()` `/notify` or any other network target (`ARCHITECTURE.md`
  §14). the dev csp additionally allows vite's hmr websocket and
  injected styles. one check at
  scaffold time: confirm `core:event:default` actually grants the
  frontend `listen` operation under the tauri version the scaffold
  resolves — don't assume the permission name is sufficient
  (2026-07-16 consensus review).

---

## 8. frontend render state

```ts
type VisibleNotification = NotificationPayload & {
  phase: "enter" | "hold" | "exit";
};

// useVisibleNotifications.ts
function useVisibleNotifications(): VisibleNotification[];
```

- listens for `notification-promoted` via `@tauri-apps/api/event`
- enter → hold transition after `enter_duration` (300ms, from
  `ARCHITECTURE.md` §3)
- hold → exit transition at `ttlSecs` elapsed
- exit → removed from state after `exit_duration` (300ms)
- **this is a render-state list, not a queue** — see the "queue
  authority" note near the top of this doc. the frontend never decides
  cap, eviction, or promotion; rust (§4) already made that decision
  before emitting the event. the frontend's only job is running the
  enter/hold/exit animation clock for whatever it's told to show, using
  the `ttlSecs` value it was given. both sides derive their timing from
  the same number, so no separate "now remove this" signal from rust
  is needed for the frontend to know when to animate out. (the hook and
  type were named `useNotificationQueue`/`QueuedNotification` in
  earlier drafts — renamed so the names stop contradicting this exact
  rule.)

---

## 9. config file

`~/.config/notchtap/config.toml`, read once at startup
(`ARCHITECTURE.md` §10):

```toml
port = 9789
default_ttl = 8
max_concurrent = 3
max_queued = 50
detect_path = "/usr/local/bin/notchtap-detect"
```

```rust
// config.rs
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_ttl")]
    pub default_ttl: u64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_max_queued")]
    pub max_queued: usize,
    #[serde(default = "default_detect_path")]
    pub detect_path: PathBuf, // absolute; see §5 for why no PATH lookup
}
```

missing file → use all defaults (don't error; the file is optional
convenience, not required setup). malformed toml → fail fast at
startup with a clear message (this is a boot-time config error, not a
runtime one worth an explicit fallback).

---

## 10. logging

`tracing` + `tracing-appender` rolling file, per `ARCHITECTURE.md` §11:

- path: `~/Library/Logs/notchtap/notchtap.log`
- rotate at 10MB, keep 3 backups
- level: `info` in release builds, `debug` in dev (`cfg!(debug_assertions)`)
- **frontend errors are out of v1 scope.** an earlier draft floated a
  `log_frontend_error` tauri command as a one-off exception to
  "frontend never invokes rust," but left it framed as both "the one
  exception" and "not required" in the same breath — confusing, and
  not needed for any v1 exit criterion. cut entirely for v1; the
  frontend stays strictly receive-only, no exceptions, matching
  `ARCHITECTURE.md` §14 exactly. if react error-boundary logging is
  wanted later, it's a v2+ addition with its own capabilities entry.

---

## 11. error types

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("queue is full")]
    QueueFull,
}

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("unknown event type: {0}")]
    UnknownType(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}
```

http handler (`http.rs`, `anyhow` boundary per `CLAUDE.md`'s rust
error handling section) maps these to status codes:

| error | status |
|---|---|
| json parse failure | `400` |
| `EventError::MissingField` / `UnknownType` | `400` |
| `QueueError::QueueFull` | `429` |
| anything else (`anyhow::Error`) | `500` |

---

## 12. cli entrypoint

**resolved: shell script** (was the last open item in §15; closed in
the 2026-07-16 grilling session). `notchtap` is a committed, executable
shell script at repo root — not a rust `[[bin]]`. json is built with
`jq -n --arg` (safe escaping for quotes/newlines in relayed bodies) and
posted with `curl`; both ship with macos on the target machines.

contract — flags only, no positional form (`ARCHITECTURE.md` §7):

```
notchtap --title <title> --body <body> [--subtitle <s>] [--port <p>]
  → POST http://127.0.0.1:<port>/notify
    body: {"title": "<title>", "body": "<body>"}
```

- a non-empty `--subtitle` folds into the posted body as
  `"<subtitle> — <body>"`; absent or empty, the body passes through
  untouched. the server never sees a subtitle field.
- port resolution: `--port` flag → `$NOTCHTAP_PORT` env var → `9789`.
  the script never reads `config.toml`.
- exits non-zero on connection failure or any non-2xx response, so a
  relay caller (cmux) surfaces failures instead of swallowing them.

per `IMPLEMENTATION_PLAN.md` §1.4/§1.5 this must exist and be tested
manually against a running server as part of v1's exit criteria.

---

## 13. dependencies

**`src-tauri/Cargo.toml`** (names only, no versions pinned here —
use whatever `cargo add` resolves at scaffold time):

- `axum`, `tokio`, `tower` (dev-dep, `features = ["util"]`)
- `serde`, `serde_json`
- `thiserror`, `anyhow`
- `tracing`, `tracing-subscriber`, `tracing-appender`
- `toml`
- `dirs` (resolving `~/.config`, `~/Library/Logs`)
- `uuid` (`v4` feature)
- `tauri` (already added by the scaffold; enable the `tray-icon`
  feature for §6's tray)
- `smappservice` (§6 — login-item registration via `SMAppService`;
  replaces the earlier `tauri-plugin-autostart` entry, which doesn't
  actually use `SMAppService` — see §6)

v2-only, not needed for v1: `wiremock`, `reqwest` (poller).

**`package.json`** (frontend): whatever the tauri react+ts template
scaffolds, plus dev-dependencies `vitest`, `@testing-library/react`,
`@testing-library/jest-dom` (`TESTING_STRATEGY.md` §2).

---

## 14. crosswalk to the test plan

each module above maps to a `TESTING_STRATEGY.md` §4 section — use
this to confirm nothing built here is missing its test coverage before
calling v1 "done" (`IMPLEMENTATION_PLAN.md` §1.5):

| this doc | `TESTING_STRATEGY.md` |
|---|---|
| §4 queue engine | §4.1 |
| §2 `EventType`/dispatch, §3 `dispatch()` | §4.2 |
| §3 http api (`oneshot` cases + the separate real-bind test) | §4.3 |
| §5 `presentation_mode` + subprocess parsing | §4.4 |
| §8 frontend render state | §4.5 |
| css animation itself | §4.6 (manual only, not automated) |
| §6 window & background setup (always-on-top, login item survives a
  restart, `LSUIElement` hides the dock icon, tray pause/resume/quit
  ui — the underlying queue pause logic is automated in §4.1) | not in `TESTING_STRATEGY.md`
  today — manual-only by the same logic as §4.6 (physical, visual,
  not worth automating for a personal tool); worth folding into
  `IMPLEMENTATION_PLAN.md` §6's manual checklist alongside the
  existing notch/hud placement checks |

---

## 15. what's still genuinely open

nothing. the last open item — the cli implementation choice — was
resolved to a shell script in the 2026-07-16 grilling session (§12),
which also locked the tray (§6), pause semantics (§3/§4), the
promotion heartbeat and emit rule (§4), the `notification-promoted`
rename (§7), and the frontend render-state rename (§8). resolved terms
live in `CONTEXT.md` at repo root.

earlier items (`notchtap-detect` build/install, the
`log_frontend_error` question) were resolved during a `docs-review`
pass the same day — see §5, §6, and §10 respectively, and
`docs/review-logs/2026-07-16-v1-technical-spec.md` for the full review
that surfaced them.
