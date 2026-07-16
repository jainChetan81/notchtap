# notchtap — implementation plan

this project builds a macos-only background utility that shows animated,
notch-anchored (or hud-style, on non-notch machines) push notifications,
fed by a local cli/http endpoint. it originated from wanting a
customisable equivalent of a closed-source reference app the user found
on his mac — no code, ip, or branding from that app is used or referenced
anywhere in this plan or the resulting codebase. this is an independent,
clean-room build.

architecture decisions (stack, distribution model, cross-device
behaviour) are already locked — see `ARCHITECTURE.md` in this folder for
the full rationale. this document is the *execution* plan: what gets
built, in what order, and how each phase is verified. `TESTING_STRATEGY.md`
holds the testing approach in full detail (frameworks, tdd scope,
per-component test plan) — §6 below only summarizes it.

---

## 0. ground rules

- macOS only. no windows/linux target, ever.
- same codebase runs unmodified on both target machines (a macbook with
  a notch, a mac mini without one). only window placement branches at
  runtime.
- tauri (rust core + react/ts webview ui), with a small native swift
  shim for notch geometry and window-level flags. not electron, not
  pure native swift — see architecture doc §8 for reasoning.
- no app store distribution. no paid apple developer account required
  for personal use on the user's own two machines.

---

## 1. v1 — core engine, queue, one animation, cli push

**goal**: prove the pipe end to end. a terminal command produces a
visible, animated notification on both target machines.

### 1.1 project scaffold
- prerequisite: install the rust toolchain via rustup (confirmed not
  present on the mac mini as of 2026-07-16 — see §7)
- `npm create tauri-app@latest` — react + typescript template
- confirm `npm run tauri dev` opens a blank window before writing any
  feature code

### 1.2 rust core (`src-tauri/src/main.rs`)
- http listener: **axum**, not `tiny_http` or a hand-rolled listener —
  pinned specifically because axum routes can be tested in-process via
  `tower::ServiceExt::oneshot` (no real socket bind needed in tests),
  and tauri already requires tokio, so this adds no new async runtime.
  see `TESTING_STRATEGY.md` §2 for the full reasoning.
- listens on `127.0.0.1:9789/notify` (loopback only, no external
  exposure — this bind restriction is itself a test case, not just a
  config detail, see `TESTING_STRATEGY.md` §4.3)
- request body → typed event → enqueued; the frontend is notified via
  the `notification-promoted` tauri event (`Manager`/`Emitter` apis),
  emitted exactly once per item at promotion time — never at enqueue
  (spec §4/§7)
- promotion heartbeat: a 250ms tokio interval drives
  `expire_and_promote` (ttl expiry + promotion — spec §4)
- tray icon (tauri tray api): **pause** (pushes buffered with a `202`
  response, promotion disabled; resume promotes immediately) and
  **quit** — spec §6, `ARCHITECTURE.md` §3/§6
- bind the `/notify` listener only **after** the main webview has
  loaded (tauri page-load callback) — tauri events are transient, so a
  `200` accepted before the frontend's listener registers would render
  nothing; connection-refused during startup is the honest failure
  mode (see `V1_TECHNICAL_SPEC.md` §3, startup ordering)
- window positioning: top-center on launch
- the notch/hud mode check (§5 of `ARCHITECTURE.md`) should be written
  as an isolated pure function taking the safe-area inset as a
  parameter, not reading `NSScreen` inline — see `TESTING_STRATEGY.md`
  §4.4 for why this split matters for testability

### 1.3 frontend (`src/App.tsx`, `src/styles.css`)
- notification queue: fifo, cap at 3 concurrent visible items, ttl-based
  auto-dismiss
- one css animation template: enter → hold → exit
- no per-event-type branching yet — that's v2

### 1.4 cli push mechanism
- `notchtap --title "t" --body "b"` — a committed shell script (jq +
  curl) at repo root posting to the `/notify` endpoint (port: `--port`
  → `$NOTCHTAP_PORT` → `9789`). flags only, no positional form; a
  non-empty `--subtitle` folds into the body cli-side (spec §12)
- this is the manual trigger; the cmux-relayed trigger point (see v2 §2.2)
  hits the same endpoint, no separate code path

### 1.5 v1 exit criteria
- `npx tsc --noEmit` and `npx vite build` run clean
- `cargo build` / `cargo tauri dev` compiles and runs (unverified in any
  prior session — this is the first real checkpoint)
- `cargo test` passes — every example case in `TESTING_STRATEGY.md` §4.1–4.4
  (queue, event bus, `/notify` handler, notch/hud decision function) has
  a written, passing test
- `npx vitest run` passes — every example case in `TESTING_STRATEGY.md`
  §4.5 (frontend queue state) has a written, passing test
- `notchtap --title "test" --body "body"` produces a visible animated
  notification on the macbook
- same build, run on the mac mini, produces a visible hud notification
  (even if not yet notch-precise on the macbook — that's deferred, see
  §5)

---

## 2. v2 — external triggers, real content, animation variety

decisions locked in `ARCHITECTURE.md` §16 (leagues, trigger scope,
css keyframes, hardening carry-over); code-level contract in
`docs/V2_TECHNICAL_SPEC.md`. build order: 2.0 → 2.3 → 2.1 (the
animation table lands before the poller so it's testable with plain
`notchtap`/`curl` pushes, no espn dependency).

### 2.0 v1 hardening carry-over (first, small, independent)
- frontend wall-clock deadline sweep (stale cards after sleep/timer
  throttling), `app_handle.exit(1)` in the server task, runtime-thread
  guard before the tray's `blocking_lock` — v2 spec §6, from
  `docs/review-logs/2026-07-16-v1-implementation-review.md`

### 2.1 espn live football scores
- poll `site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard`
  (public, keyless) for `espn_leagues` (default: `eng.1`,
  `uefa.champions`, `esp.1`) every `espn_poll_secs` (default 30s)
- new event types: `ScoreUpdate` (goal) and `MatchState`
  (kickoff/half-time/full-time — plus cards if the payload carries
  them), feeding the same v1 queue. trigger scope is "everything espn
  reports" (`ARCHITECTURE.md` §16)
- delta detection via an in-memory per-match snapshot; first sighting
  of a match is silent (no restart flood). all diff logic in a pure
  `diff_scoreboard` function, fixture-tested (v2 spec §3)
- **observe the real payload shape before writing fixture tests**
  (`TESTING_STRATEGY.md` §4.7 orders it this way)
- **failure mode is a requirement, not an afterthought**: this is an
  undocumented public endpoint, best-effort — no sla, no notice before
  it changes shape or goes down. the poller must fail gracefully: no
  crash, log a warning, skip that poll cycle (or back off), and never
  take the rest of the app down with it. see `TESTING_STRATEGY.md` §4.7
  for the corresponding fixture-based test cases (malformed response,
  http timeout/5xx)

### 2.2 cmux notification relay — ✅ verified early (2026-07-16)
- **already working on the mac mini**: during v1's live test a real
  claude code "needs input" alert surfaced through the overlay via
  cmux's notification command — this section's integration work is
  done there. remaining: configure the same one setting on the
  macbook.
- cmux (terminal) has a built-in "settings > app > notification command"
  hook that fires on every notification it raises, including claude
  code / copilot cli / opencode "agent needs input" alerts — documented
  at cmux.com/docs/notifications
- point that setting at the same `/notify` endpoint from v1 §1.4:
  `notchtap --title "$CMUX_NOTIFICATION_TITLE" --subtitle "$CMUX_NOTIFICATION_SUBTITLE" --body "$CMUX_NOTIFICATION_BODY"`
- no custom claude code hook needed — this is a heads-up relay, not an
  approval gate. it does not let the ui answer back into claude code's
  permission prompt. that would need claude code's own
  `PreToolUse`/`PermissionRequest` hooks — explicitly out of scope for
  this project.

### 2.3 animation variety
- replace the single v1 template with a config table: event type →
  animation (e.g. score-update = bounce, cmux-generic = simple slide,
  posture-alert = shake)
- css keyframes only — framer motion declined (`ARCHITECTURE.md` §16).
  the table is the stylesheet keyed by event type: the wire payload
  gains an `eventType` field, the notification div's class becomes
  `notification ${eventType} ${phase}`, unknown types fall back to
  `generic` (v2 spec §2/§5)
- lands **before** the poller — verifiable end-to-end by hand-pushing
  events, no espn needed

### 2.4 posture module (future, not in v2 scope)
- confirmed feasible via `CMHeadphoneMotionManager` (apple's public
  coremotion api, reads airpods motion data on-device)
- **not part of v2** — a tracked idea with no committed timeline. if
  picked up later it's a new event source feeding the same v1 queue, no
  core rework

### 2.5 v2 exit criteria
- `cargo test` passes — every example case in `TESTING_STRATEGY.md`
  §4.7 (espn poller: fixture-based `diff_scoreboard` + failure modes,
  no live api calls in tests) has a written, passing test. §4.8 (cmux
  env-var handling) is reduced by the flags-only cli — the shell
  expands the env vars; the script's fold/empty-subtitle behaviour
  stays manually verified (v2 spec §7/§8)
- `npx vitest run` passes — including the new wall-clock deadline
  sweep case (v2 spec §6.1)
- the three hardening fixes (§2.0) are in and `cargo test` /
  `npx vitest run` stay clean
- a live espn score change produces a distinct animation from a cmux
  relay event
- cmux "agent needs input" alerts visibly surface through the app
  without any claude code hook configuration — ✅ already verified on
  the mac mini (2026-07-16); re-verify once on the macbook

---

## 3. v3 — outbound connectors

- whatsapp via twilio (preferred over baileys — ban risk — or meta
  cloud api — heavier setup)
- telegram and other connectors follow the same notifier interface, no
  core rework
- any api keys (twilio, etc.) go in a local env var / secret file —
  never committed, never pasted into chat

---

## 4. v4 — github, ci, expanded test suites

added 2026-07-16. everything here is repo/automation infrastructure —
it changes no product behaviour. distribution/deployment is
deliberately **not** part of v4: the install story stays
`ARCHITECTURE.md` §9 (build locally per machine) until the user
reopens it.

### 4.1 github hosting
- repo lives at `github.com/jainChetan81/notchtap` (public — chosen
  2026-07-16; the docs deliberately contain no secrets, and the code
  binds to loopback only). the remote is the backup and collaboration
  surface; the two dev machines keep building locally.
- secrets hygiene carries over from §3: nothing in the repo history
  may contain a key or token. `.gitignore` already covers build
  output and agent worktrees.

### 4.2 ci — github actions
- one workflow on `macos-latest`, triggered on push + pull request,
  jobs mirroring the local gates (§6's automated section — ci runs
  exactly what a dev runs, nothing extra to memorize):
  - rust: `cargo fmt --check`, `cargo clippy -- -D warnings`,
    `cargo test` (from `src-tauri/`)
  - web: `npx tsc --noEmit`, `npx vitest run`, `npx vite build`
  - swift shim: `swiftc` compile check of `notchtap-detect` (build
    only — its behaviour is hardware-dependent and stays manual per
    `TESTING_STRATEGY.md` §5)
- no live network calls in ci — same rule as the test suite (espn is
  fixture-tested; wiremock covers the http paths)
- cache cargo + npm dependencies; keep the workflow under ~10 min
- clippy/fmt become gates here for the first time — fix what they
  flag in a dedicated commit before enabling `-D warnings`

### 4.3 expanded test suites
- raise the automated floor beyond the per-phase example cases:
  - http integration: exercise every documented status code
    (200/202/400/413/429/500) against a real axum stack via tower
    `oneshot` — some already exist; make the set exhaustive
  - queue/property edge: burst-at-cap, ttl-boundary, pause/resume
    interleavings currently only covered by the manual checklist rows
    that *can* be simulated without hardware
  - frontend: wall-clock deadline sweep timing cases (v2 spec §6.1)
    beyond the happy path
- what stays manual stays manual: notch geometry, hud placement,
  animation look, real cmux/espn end-to-end (`TESTING_STRATEGY.md`
  §5's reasoning is unchanged by ci)

### 4.4 v4 exit criteria
- repo on github, default branch protected against force-push
- ci green on the default branch, running all §4.2 jobs
- a deliberately broken test pushed to a branch turns ci red (verify
  the gate actually gates, once)
- local commands and ci run the same thing — no ci-only scripts

---

## 5. explicitly deferred polish (not blocking any phase above)

- notch-precise window positioning via the native swift shim
  (`NSScreen.auxiliaryTopLeftArea`/`NSScreen.auxiliaryTopRightArea`) — v1 ships
  with a top-center window that isn't yet notch-cutout-aware
- click-through window (`set_ignore_cursor_events`)
- real app icon (`npm run tauri icon <path>`)

note: `LSUIElement = true` + `SMAppService.mainApp.register()` (login
item, no dock icon) is **not** deferred — `ARCHITECTURE.md` §6 locks
this as a v1 day-one requirement, along with always-on-top. an earlier
draft of this list had it here as deferred polish; that was a
contradiction with §6, caught during review, and resolved in favour of
§6 since a background app that doesn't survive a restart or sits behind
other windows isn't validating the pipe either.

---

## 6. verification checklist (run before calling any phase "done")

full detail in `TESTING_STRATEGY.md`. summary:

**automated — must pass:**
- [ ] `npx tsc --noEmit` clean
- [ ] `npx vite build` clean
- [ ] `cargo build` clean (rust toolchain required — install via
      `rustup` if not already on the mac)
- [ ] `cargo test` clean — queue, event bus, `/notify` handler, notch/hud
      decision function all covered (`TESTING_STRATEGY.md` §4.1–4.4)
- [ ] `npx vitest run` clean — frontend queue state covered
      (`TESTING_STRATEGY.md` §4.5)

**manual — physical hardware, not automatable (`TESTING_STRATEGY.md` §5):**
- [ ] manual push → visible animation, both machines
- [ ] startup log shows **notch** mode on the macbook — the hud
      fallback is silent by design (`V1_TECHNICAL_SPEC.md` §5), so this
      log line is the only tell that the detector actually worked
- [ ] mac mini build transferred via a quarantine-free method
      (`ARCHITECTURE.md` §9), and `notchtap-detect` built + symlinked
      on that machine too (`V1_TECHNICAL_SPEC.md` §5)
- [ ] queue behaviour under load: push 5+ notifications rapidly, confirm
      fifo + cap-3 + ttl-dismiss all hold
- [ ] tray: pause → new pushes answered `202` and buffered, nothing new
      renders, visible items still age out; resume → buffered items
      promote fifo immediately; quit exits the app
- [ ] tray: pause football scores (v2, `espn_enabled = true` only) →
      poller log shows no new fetches after the next tick; resume →
      first poll re-baselines silently (no burst of stale score
      alerts), and a subsequent real score change still surfaces; item
      absent when `espn_enabled = false`
- [ ] cmux relay (v2): trigger a real claude code "needs input" moment,
      confirm it surfaces without manual intervention
- [ ] notch-cutout anchoring looks correct on the macbook; hud placement
      looks correct on the mac mini

---

## 7. open items — resolved 2026-07-16

- username / home path confirmed on the dev machine: `chetanjain` /
  `/Users/chetanjain`. the dev machine is the **mac mini** (`Mac16,10`,
  macos 26.5.1, no notch) — notch-mode verification waits for the
  macbook.
- the rust toolchain is **not installed** on the mac mini — install via
  rustup before §1.1. `swift` (6.2.4), `node` (22), `npm`, `jq`, and
  `curl` are all present; port `9789` is free.
