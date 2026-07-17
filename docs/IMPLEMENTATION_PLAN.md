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
  `notchtap --title "$CMUX_NOTIFICATION_TITLE" --subtitle "$CMUX_NOTIFICATION_SUBTITLE" --body "$CMUX_NOTIFICATION_BODY" --priority high`
- **`--priority high` added 2026-07-17** (v3.6 grilling session): cmux's
  own notification-command hook exposes no kind/type/urgency signal —
  confirmed against `cmux.com/docs/notifications`, which lists only
  the three variables above. rather than guess at urgency by
  pattern-matching title/body text (fragile, breaks silently if
  cmux's wording changes), every cmux-relayed notification is treated
  as `High` uniformly — matches actual usage (frequent, time-sensitive
  "claude code needs input"/"finished" alerts the user wants full,
  auto-expanded detail on, not partial attention). **this setting lives
  in cmux itself** (`settings > app > notification command`), not in
  this repo — update it on both machines, mac mini done, macbook still
  pending same as the base relay setup above.
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

decisions locked 2026-07-16 (grilling session; code-level contract in
`docs/V3_TECHNICAL_SPEC.md`):

- **the seam sits at acceptance, not promotion**: once a push passes
  validation and `enqueue` succeeds, it fans out to every connector.
  the queue (cap/ttl/pause/promotion) is a *display* concern owned by
  the overlay path alone — pausing the overlay must not silence the
  phone (that's when outbound matters most). paused pushes (`202`)
  fan out normally — acceptance succeeded; rejected pushes
  (`400`/`413`/`429`) reach no connector.
- **honest asymmetry**: the Notifier seam (`CONTEXT.md`) is
  outbound-only. the overlay is not a notifier; the queue keeps owning
  the http contract
  (200/202/429). a connector's outcome never influences the response
  to the pusher.
- **worker-per-connector**: each connector = one bounded mpsc channel
  (~64) + one long-lived worker task sending serially. acceptance does
  `try_send`; channel full → drop + warn. the guarantee is *bounded
  and non-blocking*, not *fresh*: acceptance is never delayed and the
  backlog is hard-capped at the channel depth, but under a degraded
  network up to a channel's worth of events can still be delivered
  late before drops kick in — accepted; the cap bounds the damage
  rather than eliminating staleness.
- **telegram first** (botfather bot + `sendMessage`, one rest call, no
  approval process). whatsapp/twilio demoted to "maybe later" — the
  twilio sandbox's 72h re-join and meta's template rules make it a
  poor fit for an always-on personal notifier; re-evaluate only if
  telegram proves insufficient. (reopens the earlier "whatsapp
  preferred" line, 2026-07-16.)
- **no routing, no presence-gating in v3**: every accepted event goes
  to every enabled connector, always — even while at the mac. a
  per-connector event-type filter and away-detection are both
  deliberate v3.5 candidates, not v3 scope.
- **failure semantics**: 10s send timeout; one retry after ~5s, then
  drop. formatting rejections (`400`) resend once as plain text
  instead of retrying the same broken payload.
- **message format**: per-event-type templates over telegram html mode
  (3-char escaping), unknown types fall back to generic — the same
  data-not-code move as the frontend's animation table.
- **secrets**: bot token + chat id in `~/.config/notchtap/secrets.toml`
  (checked for `600` perms), never in `config.toml`, never committed,
  never pasted into chat. no env vars — login-item launches don't
  inherit shell env. missing secrets = connector disables itself with
  a warning; the app runs overlay-only.

### 3.1 v3 exit criteria
- `cargo test` passes with the new notifier suite: `format_message`
  per type + escaping, `RetryDecision` rules, channel-full drop, and
  the acceptance fan-out test (accepted event reaches the channel,
  rejected doesn't, paused-202 does) — see `TESTING_STRATEGY.md` §4.9
- config gates are tested too: `connectors.telegram.enabled` parses
  with default `false` (outbound is opt-in per machine), and the
  secrets loader disables the connector on a missing file or non-`0600`
  permissions (unit tests against temp files)
- wiremock covers the send path (success / 400 / 5xx); no live
  telegram call in any test, ever
- manual (§6): one real end-to-end telegram message on the mac mini —
  ✅ verified 2026-07-17 (cli push arrived on the phone via the bot;
  connector-enabled overlay behaviour unchanged). the secrets-absent
  case was exercised throughout v1–v3 dev: every pre-secrets run
  logged "telegram connector disabled" and ran overlay-only

---

## 3.5. notch-morph nudge (ui polish)

decided 2026-07-16 via `prototype-notch-morph.html` (throwaway
prototype, four variants A–D, switchable live in a browser — not part
of the app build). the picked design isn't one variant but two,
selected by event type:

- **compact pill (variant C's shape)**: stays fixed at notch height,
  only widens horizontally into a pill — no vertical growth. used for
  terse/status content: `ScoreUpdate`, `MatchState`.
- **grow (variant A's motion)**: drops down and widens simultaneously,
  gaining height. used for richer content that needs multi-line body
  text: `Generic` (cmux relay, cli pushes).
- this composes with the existing v2.3 animation-type table (event
  type → animation) rather than replacing it — morph shape becomes
  another column keyed by event type, the same data-not-code move,
  not a new lookup mechanism. unknown types fall back to **grow**,
  mirroring v2.3's `generic` css fallback.
- **notch-precise geometry**: extend `notchtap-detect`'s swift shim
  beyond the existing safe-area-top presence check to also report the
  cutout's actual left/right bounds (`NSScreen.auxiliaryTopLeftArea`/
  `auxiliaryTopRightArea`). supersedes the "notch-precise window
  positioning" bullet in §5's deferred-polish list — promoted here
  into scoped work.
- **window anchoring**: the tauri window anchors to that reported
  cutout geometry in notch mode, replacing the current top-center
  placement.
- **hud fallback stays simple**: mac mini / no-notch machines keep a
  smaller "pill drop" (prototype's "mini" toggle) — there's no cutout
  to grow out of, so neither morph shape applies as-is.
- css-only animation (framer motion already declined,
  `ARCHITECTURE.md` §16); true-black background + matched corner radii
  so the boundary between hardware notch and window is invisible.
- scope: frontend css/animation-table changes + one swift-shim
  extension + window positioning. no queue/core/http changes — same
  non-goal boundary v2.3 kept (data change, not a new render path).

### 3.5.1 notch-morph exit criteria
- `npx vitest run` covers the event-type → morph-shape mapping:
  `ScoreUpdate`/`MatchState` → pill, `Generic` → grow, unknown types →
  grow (fallback)
- swift shim: `notchtap-detect` gains fixture coverage for cutout
  left/right/width reporting, alongside the existing safe-area-top
  cases (`presentation::tests`)
- manual (§6, physical hardware, macbook): the pill widens without any
  vertical jump; grow drops-then-widens with no visible seam against
  the real cutout; hud fallback still looks correct on the mac mini
- `prototype-notch-morph.html` deleted once the real implementation
  lands — it's throwaway by design, not meant to ship

---

## 3.6. permanent rotating overlay (replaces the transient display model)

decided 2026-07-17 via grilling session, inspired by trycrossbar.live
("the world cup, living in your macbook's notch" — toggle/expand/replay
hotkeys, content persists until dismissed rather than auto-expiring).
**this supersedes §3.5's pill/grow/mini shape system and the
multi-item stack entirely** — not an addition alongside them. an
earlier draft of this section proposed a second, additive window; that
was reopened and reversed 2026-07-17: the user wants one persistent
surface, not two display paradigms to keep in sync.

this is the biggest architectural change since v1: it reopens the
transient/TTL display assumption `ARCHITECTURE.md` and `CONTEXT.md`
have carried since v1. it does **not** reopen the receive-only ipc
boundary (`CLAUDE.md` "ipc & security") — that constraint is
deliberately preserved, see the expand-mechanism bullet below.

**what survives, reconfigured rather than rebuilt**: `queue.rs`'s
accept/buffer machinery — FIFO-capped `Waiting`, a heartbeat that
promotes on a timer — is not thrown away. what changes:

- **`max_concurrent` → 1**: exactly one `Visible` item at a time (the
  slot), not a 3-item stack. everything else sits `Waiting`.
- **promotion becomes priority-ordered, not pure fifo**: a new
  `priority: low | medium | high` field, decoupled from `EventType`
  (not every high-priority thing is a score — "a service is down" is
  high-priority and isn't `ScoreUpdate`/`MatchState`). higher-priority
  `Waiting` items are promoted next, ahead of older lower-priority
  ones. **ordering only — never live interruption**: the currently
  `Visible` item always finishes its own turn; a high-priority arrival
  jumps the `Waiting` line for the *next* promotion, it does not cut
  the current item off mid-display. avoids jarring half-finished exit
  animations and the open question of what "resume" would even mean
  for an interrupted item.
- **ttl becomes rotation duration**: ~8s base per item (reuses today's
  `default_ttl`), longer while an item is expanded. **low-priority
  items may be infinite-duration**: they don't expire on a clock, they
  keep recurring in rotation until superseded by fresher data for the
  same thing (e.g. a live score updates in place) or the underlying
  state naturally ends (match over). this is also how the
  "always something there" requirement is met — **not a special idle
  mode**, just one or more perpetual low-priority items (exact content
  tbd by the user later: candidates mentioned were current match
  score/time, a calendar event starting soon, order-arrival tracking,
  world clock) always sitting in `Waiting`, promoted whenever nothing
  higher-priority is pending. the architecture must stay generic
  enough to accept these later sources without a redesign — this
  section does not scope or build any of them now.
- **every source feeds this one queue**: the espn poller, the cmux
  relay, and cli pushes all produce events for the same
  priority-ordered queue — no separate transient path for "urgent"
  content. an urgent cmux "needs input" alert does not interrupt or
  bypass the slot; it's a high-priority item that gets promoted next,
  same as a goal.
- **cli gains `--priority low|medium|high`** (default: unspecified →
  treated as `medium`... **open detail, not blocking**: exact default
  and whether cmux's relay path needs its own default separately from
  raw cli pushes is an implementation-time call, not re-litigated
  here). the poller sets `high` on goals/cards itself.

**display**: single fixed-position slot, edge-flush to the top of the
screen, no padding/gap — crossbar-style, notch-integrated on notch
hardware (anchors to the cutout precisely, same geometry §3.5 already
built) — generic top-edge-flush on hud machines. replaces
`.notification.pill`/`.grow`/`.mini` and `getMorphShape` entirely; the
event-type → shape lookup table is gone, replaced by a single
slot renderer that reads whatever item the queue currently has
`Visible` plus its `priority`/expand state.

**expand — two independent triggers, ipc boundary preserved**:
- **automatic**, content-driven, for `priority: high` items — the slot
  grows to fit (same `max-height`/width-animation technique §3.5
  already built for the `grow` shape, just applied uniformly rather
  than per-event-type).
- **manual**, via a **macOS global hotkey** (exact combo tbd), for
  everything else ("news"). this stays consistent with `CLAUDE.md`'s
  locked receive-only frontend: the hotkey is registered and handled
  **rust-side** (global shortcut api, not a browser/webview keyboard
  handler), rust decides the resulting expand state, and pushes it to
  the frontend via the same one-way event pattern already used for
  `presentation-mode` — no new frontend-to-rust invoke command, no
  in-window click handler.

**window behaviour — new requirement, applies to the one window this
section leaves in place**: must stay visible over fullscreen apps and
follow the user across macOS Spaces, not just sit on bare desktop.
`window.set_always_on_top(true)` (already set) does not cover this —
needs the window's `NSWindowCollectionBehavior` to include
`canJoinAllSpaces` (and likely `fullScreenAuxiliary`), which tauri's
cross-platform api doesn't expose directly; needs a small macos-
specific call against the raw `NSWindow` handle. single physical
monitor scope — no multi-monitor window spawning.

**paused semantics carry over unchanged**: the tray's `Paused` state
still means promotion is frozen and new pushes still buffer into
`Waiting` — same contract as today (`CONTEXT.md`), just operating on
the single-slot queue instead of the 3-item stack.

**idle content — decided 2026-07-17, grilled separately from the rest of
this section**: a local date/time clock (`useClock.ts`), rendered by
`App.tsx` only when `useSlotState()` reports `empty`. **deliberately
bypasses the queue entirely** — it's computed client-side from the
webview's own `Date()`, not pushed as an `Event`/`Priority`/`Recurring`
item, so it does not exercise the evergreen-queue mechanism §4 of
`V3_6_TECHNICAL_SPEC.md` describes. that mechanism stays functionally
untested until a real backend-driven evergreen source (calendar, order
tracking) needs it. 30s refresh interval (display has no seconds, so
per-second ticking would be pure waste).

**explicitly deferred, not decided here**:
- exact global hotkey combination
- the longer-term content-source taxonomy (stocks, calendar, order
  tracking, claude code session status, etc.) beyond "the
  queue and priority model must be generic enough to accept them"
- `CONTEXT.md` glossary updates (this section introduces `Priority`,
  redefines `Visible` as singular, and probably retires "Promotion
  disabled while Paused, stack" language) — needed before
  implementation starts, not written speculatively here
- a code-level technical spec (mirroring `V3_TECHNICAL_SPEC.md`'s
  precedent) for the wire schema change, the `NSWindowCollectionBehavior`
  call, and the global-hotkey registration mechanism — this section is
  the architecture decision, not the implementation contract — ✅
  written 2026-07-17, see `docs/V3_6_TECHNICAL_SPEC.md` (HLD+LLD, plus
  a five-workstream parallel-agent breakdown for implementation)

### 3.6.1 v3.6 exit criteria

full detail and the workstream breakdown live in
`docs/V3_6_TECHNICAL_SPEC.md` §8/§10; summarized here to match the
§3.1/§3.5.1 pattern. **implementation landed 2026-07-17** on branch
`v3.6-rotating-overlay` (not yet merged to `master`) — automated
criteria below are verified; manual criteria need the physical
macbook and are still open:

- ✅ `cargo test` passes: the single-slot queue suite (never-interrupt,
  tier-strict promotion, fifo-within-tier, `Recurring` requeue-to-own-
  tier-back, `OneShot` drop-forever, topic supersession both
  visible/waiting, per-tier `429`), `Priority` ordering, and the
  `slot-state` change-guard emission tests — see
  `TESTING_STRATEGY.md` §4.10 for the full case list and current
  counts (§0)
- ✅ `npx vitest run` passes: `useSlotState` render tests (empty, showing,
  replace-without-empty-frame) and `App.tsx` against the new markup —
  `morphShape.test.ts` and the old `.stack`-based tests are deleted,
  not just left passing-by-accident
- ✅ `npx tsc --noEmit` / `npx vite build` clean against the rewritten
  frontend
- [ ] manual (physical hardware, per `TESTING_STRATEGY.md` §5/§4.10):
  the global hotkey toggles expand on the macbook; the window survives
  a Spaces switch and stays visible over a fullscreen app; a live espn
  goal still auto-expands and rotates out correctly under the new
  model — not yet run, tracked in §6's checklist too
- ✅ `CONTEXT.md` glossary updated per this doc's §3.6 note and
  `V3_6_TECHNICAL_SPEC.md` §2/§9 (single reviewable commit, not bundled
  into the code-focused spec) — landed 2026-07-17 alongside this entry

**implementation notes worth recording** (not decisions, just facts
that would otherwise only live in the branch's commit history):
- the rust backend was split across two coding agents (kimi, then
  Claude directly after kimi hit a billing quota limit mid-`queue.rs`)
  against the same written plan; the frontend was done by codex
  (gpt-5.6-sol) in one pass. all of it was independently re-verified
  (fresh `cargo build`/`cargo test`/`tsc`/`vitest`/`vite build` runs,
  not trusted from either agent's own self-report) before being
  recorded as done here.
- verification caught one real production bug before it shipped:
  `SlotState`'s camelCase rename didn't apply to struct-variant field
  names without `rename_all_fields` too — see `TESTING_STRATEGY.md`
  §4.10 for detail. this is exactly the failure mode the spec's own
  §5.2 "integration risk" note flagged in advance.

---

## 4. v4 — github, ci, expanded test suites — ✅ done 2026-07-16

all four exit criteria in §4.4 verified 2026-07-16: repo live at
`github.com/jainChetan81/notchtap` with force-push/deletion protection
on `master`; ci green on the default branch (rust 63, web 11, swift
compile — run 29502443487); a deliberately broken test on a pr branch
turned ci red (run 29503004557, pr #1, closed unmerged); ci jobs are
the same commands a dev runs locally.

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

## 4.5. v5 — settings window (control panel)

decisions locked 2026-07-17 (grilling session) in `ARCHITECTURE.md`
§17; code-level contract in `docs/V5_TECHNICAL_SPEC.md`. numbered §4.5
to avoid renumbering §5–§8 (same fractional-section precedent as
§3.5/§3.6). **steps 1–4 (all rust) landed 2026-07-17**; step 5 is
held per the sequencing note below.

the shape in one paragraph: a fourth tray item ("Settings…") opens a
second webview window (label `settings`) — the app's first
frontend→rust invoke surface, scoped to that window alone via the
`build.rs` command-acl opt-in + a dedicated capability file (the
overlay's `default.json` stays byte-for-byte). saving validates
rust-side, writes `config.toml` atomically, and relaunches the app
(no hot-reload, ever). secrets (openrouter api key now, telegram
folded in) are write-only across ipc into `secrets.toml` (0600) with
masked status display. a persisted `start_paused` flag becomes the
master kill switch.

build order (each step leaves the suite green):

1. **pure core first, tdd**: `settings.rs` — `validate`, `mask`,
   config `Serialize` + round-trip test, secrets-file merge logic
   (all tables optional; spec §3/§4)
2. **write paths**: atomic config write, secrets read-modify-write
   with `0600`, temp-dir integration tests
3. **ipc surface**: the four commands + label checks, `build.rs`
   `AppManifest::commands` opt-in, `capabilities/settings.json`
4. **window + tray**: lazy `WebviewWindowBuilder` from the new tray
   item; `start_paused` wiring in `lib.rs`/`build_tray`
5. **frontend page**: `settings.html` + `src/settings/`, vite
   multi-page input, form + vitest cases

**sequencing note (2026-07-17)**: steps 1–4 are rust-only and don't
touch `src/`; they proceed **now**, in parallel with the ui migration
(framer motion + lucide, `ARCHITECTURE.md` §4/§16 reversal). step 5
is **deliberately held** until that migration lands, so the settings
form is built once, on the new stack, instead of built plain and
restyled. interim consequence: the tray's "Settings…" item opens a
blank window until step 5 — accepted, not a bug.

### 4.5.1 v5 exit criteria

- `cargo test` passes with the new settings suite (`TESTING_STRATEGY.md`
  §4.11): every `validate` rule boundary, `mask`, config
  serialize→parse round-trip, secrets merge (preserve-other-table,
  malformed-file-errors-not-clobbers), atomic-write + `0600`
  integration cases against temp dirs
- `npx vitest run` passes with the settings-form cases (mocked
  `invoke`); `npx tsc --noEmit` / `npx vite build` clean with the
  multi-page config; overlay tests untouched and green
- `capabilities/default.json` is unchanged in the diff — reviewable
  proof the overlay stayed receive-only
- manual (§6): settings window opens from the tray; save relaunches
  with the change live; a pasted key lands in `secrets.toml` mode
  `0600` and shows masked; `start_paused = true` boots to "Resume";
  an `invoke("get_config")` from the *main* window's devtools is
  denied (the acl gate actually gates — run once, v4 §4.4 discipline)

---

## 4.6. v5 — news source (rss poller + status-rail news cards) — ✅ landed 2026-07-17

built via two delegated slices (rust wire metadata, frontend cards)
against a frozen `slot-state` contract, merged sequentially onto
`v3.6-rotating-overlay` the same day as the settings rust core.

- **rust** (`src-tauri/src/rss_poller.rs`): polls configured rss feeds
  (default NDTV top-stories) every `rss_poll_secs` (default 60, floor
  15); shared bounded `SeenStore` dedup (guid → canonical-link
  fallback, 1k keys / 7-day eviction, cross-feed duplicate guard);
  silent baseline on start and tray-resume; conditional GET
  (etag/last-modified, validators persisted only after a successful
  parse); per-feed `Backoff`; 1 MiB body cap; ≤3 redirects;
  `rss_max_per_poll` (default 10) as a replay bug-guard. events are
  `NewsItem` / `Priority::Low` / `OneShot { rss_ttl_secs }` /
  `topic: None` (every-headline decision — no supersession
  coalescing), overlay-only (never offered to connectors).
- **wire metadata**: `EventMeta { source, category, published_at_ms }`
  on `Event`, surfaced on `SlotState::Showing` as `source` /
  `category` / `publishedAtMs` (null for non-news). category derives
  from entry `<category>` tags via a keyword table, else the feed's
  configured default; source from `[[rss_feeds]]` config, else the
  parsed feed title.
- **config** (breaking, pre-release): `rss_feeds` is an array of
  tables — `[[rss_feeds]] url / source / category` — plus
  `rss_enabled` (default **false**, opt-in), `rss_poll_secs`,
  `rss_ttl_secs` (default 10), `rss_max_per_poll`. all panel-validated
  (§4.5's `validate`, extended same day).
- **frontend** (status rail): news branch keyed off
  `eventType === "news_item"` — masthead row (`{source} · Wire`),
  2-line clamped headline, category + age pills (staggered entry),
  category-hued gradient shader (`.news-shade`, sub-16%-alpha drift,
  reduced-motion aware), newspaper tier glyph, 3-column news manifest
  (summary / source + published / category + control). priority keeps
  tier column, stamp, and track — the two color systems never touch
  the same element. all lookups are typed tables in
  `lib/presentation.ts` (`stampFor` news → "Wire", `categoryClass`,
  `ageLabel`, `publishedLabel`), `assertNever`-guarded.

### 4.6.1 news source exit criteria

- [x] `cargo test` green with the rss_poller suite (20 cases: seen
  store bounds, dedup key fallbacks, sanitize incl. real-shaped ndtv
  fixtures, diff ordering/baseline/cap, cross-feed dedup, metadata
  derivation) — `TESTING_STRATEGY.md` §0 counts
- [x] `npx vitest run` green with the news-card cases (presentation
  tables, StatusRailCard news rendering incl. null-metadata fallbacks,
  slot-state validation of the three nullable fields)
- [x] tray gains "Pause News" (only when `rss_enabled`); resume
  re-baselines — no headline flood
- [x] manual, hud mode (mac mini, 2026-07-17): live-feed smoke run —
  poller started against a local rss fixture, first poll **200 →
  silent baseline**, second poll **304 Not Modified** (conditional GET
  path proven), item published after baseline fetched **200** and
  displayed within one poll cycle; high-priority cli push rendered
  instantly alongside. re-pointed at the real NDTV feed after the run.
- [ ] manual, notch mode: same checklist eyeballed on the macbook
  (geometry only — logic is device-independent)

---

## 5. explicitly deferred polish (not blocking any phase above)

- ~~notch-precise window positioning via the native swift shim~~ —
  promoted to scoped work in §3.5 (2026-07-16); no longer deferred
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

checked 2026-07-16 against `HEAD` = `b061577` (v1–v4); re-checked
2026-07-17 on branch `v3.6-rotating-overlay` (not yet merged) to cover
v3.6's rewritten queue/frontend — see `TESTING_STRATEGY.md` §4.10.

**automated — must pass:**
- [x] `npx tsc --noEmit` clean
- [x] `npx vite build` clean
- [x] `cargo build` clean (rust toolchain required — install via
      `rustup` if not already on the mac)
- [x] `cargo test` clean — queue, event bus, `/notify` handler, notch/hud
      decision function, and (as of 2026-07-17) the v3.6 single-slot
      queue/priority/rotation suite all covered (`TESTING_STRATEGY.md`
      §4.1–4.4, §4.10) — current counts live in `TESTING_STRATEGY.md`
      §0 and only there, not duplicated here
- [x] `npx vitest run` clean — frontend queue state covered
      (`TESTING_STRATEGY.md` §4.5), superseded 2026-07-17 by the v3.6
      `useSlotState`/`App.tsx` suite (§4.10) — counts in
      `TESTING_STRATEGY.md` §0

**manual — physical hardware, not automatable (`TESTING_STRATEGY.md` §5):**
- [ ] manual push → visible animation, both machines — mac mini side
      exercised repeatedly during dev; macbook side not yet confirmed
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
- [x] cmux relay (v2): trigger a real claude code "needs input" moment,
      confirm it surfaces without manual intervention — ✅ verified
      2026-07-16 on the mac mini, see §2.2
- [ ] notch-cutout anchoring looks correct on the macbook; hud placement
      looks correct on the mac mini
- [ ] v3.6: the global hotkey toggles expand on the macbook (manual
      trigger, `Medium`/`Low`-priority item Visible); pressing it while
      a `High`-priority item is auto-Expanded is a no-op
- [ ] v3.6: the window survives a Spaces switch and stays visible over
      a fullscreen app (`NSWindowCollectionBehavior`)
- [ ] v3.6: a live espn goal (`High` priority) auto-expands and rotates
      out correctly under the single-slot model
- [ ] v5 (once built, mac mini is enough — no notch dependency):
      settings window opens from the tray and re-focuses instead of
      duplicating; "Save & Relaunch" restarts the app with the change
      observably live (e.g. a new `default_ttl`); a pasted openrouter
      key lands in `secrets.toml` with mode `0600` and the panel shows
      only the masked status; `start_paused = true` boots the app
      paused with the tray reading "Resume"
- [ ] v5: `invoke("get_config")` from the *main* window's devtools
      console is denied — verifies the command acl actually gates
      (once, same discipline as v4's break-the-ci check)

---

## 7. open items — resolved 2026-07-16

- username / home path confirmed on the dev machine: `chetanjain` /
  `/Users/chetanjain`. the dev machine is the **mac mini** (`Mac16,10`,
  macos 26.5.1, no notch) — notch-mode verification waits for the
  macbook.
- the rust toolchain is **not installed** on the mac mini — install via
  rustup before §1.1. `swift` (6.2.4), `node` (22), `npm`, `jq`, and
  `curl` are all present; port `9789` is free.

---

## 8. future integration idea — kuma alert relay (not scoped, not committed)

surfaced 2026-07-17 while reviewing whether notchtap should integrate
with the user's separate "mac mini automation" home-lab project
(hermes agent, uptime kuma, wiz lights, media stack, etc. — an
unrelated repo). that review concluded almost every proposed
integration was low-value: notchtap is receive-only by design
(`ARCHITECTURE.md` §7 rules out any respond-back/approval loop), so it
cannot control or query anything in that stack, only display. most
"integration" ideas (a notchtap telegram bot, hook scripts pushing to
notchtap on failures, secrets-storage alignment) were rejected as
either redundant with what the hermes telegram bot already does, or
pure hygiene with no functional value. see the user's cowork project
memory for the full rejected list.

**the one idea judged genuinely worth keeping, not yet built**:
uptime kuma (already monitoring jellyfin/qbittorrent/sonarr/etc. on
the mini) has a generic "webhook" notification provider that supports
a custom json body template. pointed at notchtap's existing
`/notify` endpoint with a body of `{"title": "{{name}}", "body":
"{{msg}}"}`, a kuma monitor going down could push straight into
notchtap — no new service, no relay script, just one kuma notification
config entry.

**real constraints, not glossed over**:
- **loopback-only applies here too**: notchtap's `/notify` binds
  `127.0.0.1` only (§1.2, `ARCHITECTURE.md` §7 — deliberate, not an
  oversight). kuma runs on the mac mini, so this only reaches a
  notchtap instance running on **that same machine**, never the
  macbook over the tailnet. reaching the macbook would mean reopening
  the loopback-only decision — a real scope change, not a config
  tweak, and not proposed here.
- kuma's custom-body webhook has known bugs with template-variable
  substitution in some versions (github issues #3635, #4861 on
  louislam/uptime-kuma) — needs a manual smoke test before trusting it,
  not assumed to work first try.
- **open question, not resolved**: whether this is worth building at
  all depends on how often the user is actually looking at the mac
  mini's own screen when a monitor fires — if it's mostly headless for
  him, kuma's existing telegram alert already covers it and this adds
  nothing.

no timeline, no owner, not blocking any phase above — recorded here so
the idea isn't lost, same treatment as §2.4's posture module.
