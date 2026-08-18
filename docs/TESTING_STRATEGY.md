# mac-notification-nudge — testing strategy

companion to `ARCHITECTURE.md` (decisions). this doc answers: what gets
an automated test, what doesn't, which framework, and what is
deliberately manual-only. it is the only testing doc.

---

## 0. status at a glance

counts live here and only here; other sections and docs point back
rather than repeating them.

| suite | size | where |
|---|---|---|
| rust unit/integration | 953 lib-crate tests + 3 integration-binary tests = 956 | `cargo test` from `src-tauri/` |
| rust doc-tests | 3 — public `queue`/`event` apis | same `cargo test` run |
| frontend | 748 tests across 39 test files | `npx vitest run` |
| ci | fmt, clippy `-D warnings` (`--locked`), cargo test (`--locked`), cargo-audit, npm audit, tsc, vitest, vite build, `sh -n` cli syntax check, swiftc compile check | every push + pr |

every example surface listed in §4 has passing coverage. the recurring
manual hardware checklist is §6 — never "done", re-run per relevant
change.

---

## 1. shape of the pyramid for this project

this project's pyramid is unusually bottom-heavy and has almost no
automated top layer, for a concrete reason: the highest-risk logic
(queue ordering, rotation expiry, event parsing) is pure and
deterministic, while the highest-visibility behaviour (notch-cutout
geometry on real hardware, animation timing) depends on things a test
runner can't see — real `NSScreen` data on two specific physical macs,
and rendered visual output. don't fight that; put the automation budget
where it pays off.

```
        /  manual only   \      2 physical machines, real windows,
       /  (§5, §6)        \     visual correctness — not automatable
      /------------------  \
     /   integration (some)  \   http layer, tauri command dispatch
    /------------------------  \
   /   unit tests (many, fast)   \  queue, registry, parsing, reducers
```

---

## 2. framework choices

| layer | framework | why |
|---|---|---|
| rust unit/integration | built-in `cargo test` (`#[test]`, `#[tokio::test]`) | no extra dependency; tauri already pulls in tokio, so async tests are free |
| rust http layer | `axum` + `tower`'s `ServiceExt::oneshot` | routes tested in-process — no real socket bind, no port cleanup, no flaky "address in use" |
| rust external-http decision surfaces | `wiremock` | only for fetch-path decision logic (redirects, size caps, 304s, fallback chains); parsers and delta logic stay pure functions against committed fixtures — never a live network call in any test |
| rust property tests | `proptest` (dev-dependency) | the queue's generated-adversary suite, §7 |
| frontend unit/component | `vitest` + `@testing-library/react` | shares the vite config; tests behaviour (what's rendered), not implementation details |
| rust doc-tests | built-in | the public `queue`/`event` apis carry runnable examples that double as documentation. not the coverage layer — keep them few and lifecycle-shaped |

---

## 3. where to actually do tdd

red-green-refactor is worth the discipline where the logic is pure,
deterministic, and wrong-by-default if untested:

- **single-slot priority queue** — tier-strict promotion, rotation
  (one-shot vs recurring), topic supersession, preemption, pause,
  silence gating
- **event bus / dispatch router** — event routing, malformed payload
  rejection
- **http handlers** (`/notify`, `/agent/events`) — parsing, validation,
  status codes, caps
- **notch/hud mode decision function** — the one piece of the native
  layer that's a pure function once isolated
- **agent registry** — state transitions, ordering, retention

tdd is **not** worth it, and shouldn't be forced, for:

- css animation timing/easing — write it, eyeball it, adjust
- the native swift `NSScreen` shim — nothing to assert without the
  physical screen
- tauri window creation/positioning calls — thin wrappers around a
  native api; a unit test would just re-assert the mock

---

## 4. component-by-component test plan

what each surface covers, and the standing rules per surface. current
counts: §0.

### 4.1 single-slot rotating queue (rust, `queue.rs`)

covers every state transition in the single-slot model:

- `tick`/rotation: tier-strict promotion order, rotation-order rank
  then FIFO within a tier, `OneShot` drops forever, `Recurring`
  requeues to the back of its **own** tier
- fast-path: a push with any tier non-empty never fast-path-promotes,
  even a `High` push arriving while only `Low` waits
- supersession: a visible-item supersede updates
  payload/priority/rotation and grants a capped extension only when
  remaining time is below the floor (`promoted_at` never mutated); a
  burst of rapid supersedes still rotates out at exactly
  `base_window + 6s`; a priority-changing supersede moves to the back
  of its *new* tier; cross-tier supersede respects the destination
  tier's cap
- per-tier cap: a full `Low` tier rejects a new `Low` push while a
  simultaneous `High` push is still accepted
- pause: gates promotion, not rotation; resume promotes on the next
  `tick`
- silence: Medium/Low buffer, High breaks through compact; preemption:
  strictly-higher preempts, the preempted card re-queues at its tier
  head with remaining time
- hover: `hover_enter`/`hover_exit` freeze `remaining_ms` for the whole
  hover session, banked time never grants more than the rotation
  window (also pinned as proptest properties, §7)
- expanded semantics: Medium/High promotions start expanded,
  auto-collapse at half window; Low and Breakthrough start compact
- `slot_state_if_changed`: suppresses a re-emit when nothing changed;
  a promotion, rotation-to-empty, or expand toggle always emits
- ttl-restart instrumentation: the `is_ttl_restart` predicate's decay/
  hover-gap/restart/jitter cases; an intentional skip-restart never
  warns, a genuine restart does

two wire-contract pins that guard real drift classes:

- **`Priority` ordering** (`event.rs`): `Low < Medium < High` pinned —
  the array-index promotion logic depends on declaration order matching
  `Ord`.
- **`SlotState` wire snapshot** (`event.rs`): a `serde_json::to_value`
  snapshot pins the exact camelCase field names —
  `#[serde(rename_all = "camelCase")]` on an enum renames only the
  variant tag, not struct-variant fields, and this test is what catches
  that class of drift. `dedup_eq` participation is pinned per
  continuously-varying field (`ttl_ms`/`remaining_ms`) and for
  `origin`/`agent_runtime`.

### 4.2 event bus / dispatch router (rust)

every event type routed; unknown `type` rejected, not silently
dropped; missing required fields rejected with a specific error, never
a panic.

### 4.3 `/notify` http handler (rust)

every response code path via `tower::oneshot`: valid POST → 200 and
forwarded; malformed json / wrong content-type → 400; oversized body →
413 (exact 64 KiB boundary pinned: at-limit accepted, one byte over
rejected); tier full → 429; paused → 202 + `{"status": "paused",
"queued": n}` and still 429 when full. the loopback-only bind address
is asserted directly (a security boundary, not just correctness).
subtitle/details round-trip with server-side caps (pair count, label/
value truncation, empty-label drop); a `{title, body}`-only payload
behaves byte-identically. `ttl` is wire-immutable: `NotifyRequest` has
no ttl field, an unrecognized `ttlSecs` on the wire is ignored and the
configured default applies (asserted via `next_deadline()`). burst
accounting: with a tier cap of 5, eight same-tier posts yield exactly
1 visible + 5 waiting + 2×429.

### 4.4 notch/hud mode decision (rust, `presentation.rs`)

the decision is a pure function —
`fn presentation_mode(safe_area_top_inset: f64) -> Mode` — tested with
`0.0` (hud) and positive (notch) without touching `NSScreen`. the
subprocess boundary (`notchtap-detect` via `std::process::Command`) is
split the same way: the spawn stays untested; everything downstream of
"here is a string of stdout" is covered — well-formed json parses,
malformed/truncated json and non-zero exit fall back to hud mode
explicitly (log, don't panic), and the binary-not-found path (the most
likely real failure) has a dedicated test.

### 4.5 frontend slot render state (react/ts)

`useSlotState` + `App.tsx`: renders `empty` as nothing; renders
`showing` with the right priority/expanded/source classes; a new
payload replaces content without an intermediate empty frame; listener
cleanup on unmount. the runtime payload validator rejects malformed
`slot-state` payloads field by field (missing/invalid `origin`,
`agentRuntime`, timing fields, espn block). the frontend renders every
promoted event it receives without enforcing any cap itself — cap and
promotion authority live rust-side.

### 4.6 animation rendering (css/react) — manual by design

no automated visual regression: the tooling cost (screenshot diffing,
baseline management) isn't justified for a personal tool whose
keyframe sets are trivially eyeball-able. re-evaluate only if per-type
styling starts regressing during unrelated css edits. string-level CSS
pins are the exception where a rule's *existence or byte-equality* is
load-bearing (celebration z-index stacking, exit-to-bare convergence,
transition-property lists, token wiring) — jsdom can't measure layout,
so those pin the stylesheet text instead.

### 4.7 espn football poller (rust, `poller.rs`)

the fetch loop stays thin and untested (§5.1); parsing and all delta
logic are pure functions against captured fixtures:

- score delta → one `ScoreUpdate` per changed match; unchanged matches
  emit nothing; status delta (pre→in, →halftime, →final) → one
  `MatchState`
- first sighting of a match is silent (no restart flood); a match gone
  final is evicted after the full-time event; a match merely absent
  from one poll is carried forward (a goal during the blip is caught on
  reappearance), sustained absence evicts
- malformed/empty json → no crash, no event; http timeout/5xx →
  per-league backoff, other leagues keep polling
- live-card mode: Topic/rotation shape, same-poll ordering never
  un-retires a just-finished card, `meta` (Clock/Cards) survives
  supersession
- crests: cache-miss/hit, one attempt per team, oversized/failed fetch,
  filename sanitization, and the `crest_url_allowed` allowlist (https +
  espncdn hosts only; non-https, wrong-host, suffix-spoofed, raw-ip,
  unparseable all rejected) plus the pin that a disallowed logo url
  never enters the id→logo map
- rich events: the summary→plays fallback chain via wiremock (404 and
  empty triggers), classification drops scoreboard-owned and
  unrecognized types, per-match (kind, clock) dedup
- never call the live espn endpoint from a test — fixtures only

### 4.8 rss news poller (rust, `rss_poller.rs`)

same shape as §4.7 — thin fetch loop, pure logic against real-shaped
fixtures:

- `SeenStore`: bounds (1k keys, oldest first), 7-day eviction, guid
  dedup, canonical-link fallback, cross-feed duplicate guard
- sanitize strips markup/entities without mangling plain text; a
  malformed/empty feed body emits nothing and never crashes
- first poll per feed is silent; subsequent polls emit only unseen
  items in feed order; `rss_max_per_poll` caps a poll's emissions
  without dropping the excess from `SeenStore`
- category/source metadata derivation with fallbacks; topic feeds
  (`expand_topic_url`) and the one-shot `search_once` path (wiremock)
- `fetch_feed`'s decision surface (304 short-circuit,
  validate-then-persist ordering, streamed size cap) is
  wiremock-tested; only the spawn loop stays thin-by-design

frontend: masthead render, clamped headline, category/age pills,
lookup-table cases with unknown-category and null-metadata fallbacks,
the news manifest layout.

### 4.9 settings window (rust + vitest)

the app's one frontend→rust invoke surface, so the suite's job is
twofold: pure-logic coverage, plus pinning the security boundary.

- pure, tdd'd first (`settings.rs`): `validate` — every rule's
  accept/reject boundary (port floor, ttl/poll/cap ranges, league and
  feed url shapes, prefix-shortcut validation incl. the full-BMP
  whitespace sweep, `[agents]` duration bounds, silence window
  parsing); config serialize→parse round-trip pinning the `Serialize`
  derive against field drift; submitted `detect_path` replaced with the
  booted value before a save
- temp-dir integration (never `$HOME`): atomic config write — result
  parses, temp file gone after rename, missing parent dir created
- `ensure_settings_window` accepts the `settings` label and rejects
  `main` (`tauri::test::mock_app()` + `WebviewWindowBuilder`)
- **security triple** (`settings_commands.rs`): the command count, the
  whole-permission-set compare against `capabilities/settings.json`,
  and the `generate_handler!` parse — a new command that misses any leg
  of `ARCHITECTURE.md` §9's contract fails a test, not a review
- frontend (vitest, `SettingsApp.test.tsx` + section suites): form
  round-trips from a mocked `get_config`; save rejection renders the
  error list; every action reports its outcome (announced errors
  deduplicate, successes clear); queue/history sections render, clear,
  and skip through their commands; real semantic HTML pins (fieldset/
  legend, lists, tables, `role=switch`)
- security boundary, pinned two ways: `capabilities/default.json`
  unchanged in any diff (review-level), and — manual, once per
  mechanism change — `invoke("get_config")` from the *main* window's
  devtools console is denied (§6 checklist)
- untested by design: lazy window creation and tray wiring (thin
  native glue); `app.restart()` (kills the process — nothing to assert
  from inside it)

### 4.10 agent adapters, registry, and Agent Board

code-level contract: `ARCHITECTURE.md` §10.

**rust (`src-tauri/src/agents/`)**: `adapter.rs` — wire parsing, every
cap's boundary/one-over/multibyte-safe-truncation case, enum
round-trips, control-character stripping, missing-identity rejection.
`registry.rs` — state-machine transitions (non-terminal tool failure
stays live, terminal never resurrects), stale timeout, retention purge,
urgency+FIFO ordering, duplicate/stale/higher sequence handling,
bounded LRU event IDs, history cap, reused-terminal-id suffixing.
`health.rs` — accepted/error recording, per-runtime compatibility
messages, snapshot reflection. `focus.rs` — Host recognition, the
code-owned scheme allowlist rejecting everything by default,
wire-supplied bundle IDs never trusted. `model.rs` — session-key
identity, urgency rank, terminal classification, `dedup_eq` ignoring
only clock-only changes. `notification.rs` — permission/input/failure
map to high one-shot, completed to medium, informational suppressed
unless policy-enabled, progress creates no card, session IDs hashed in
the signal. `expand.rs` — Board geometry formulas. `board.rs` — the
`agent-state` event name pin plus the pull-surface publish gates.
`providers/doctor.rs` — hook-file inspection per runtime, command
classification, render/health verdicts with no absolute home path in
output.

**`POST /agent/events` (`http.rs`)**: valid event → 202; registry
update confirmed independently of notification delivery; master switch
off skips every runtime; noteworthy events queue a card while routine
progress doesn't; a full tier still updates the registry (202, card
dropped); 400/413 and Host-header defense mirror `/notify`; every
per-field cap has a truncation case and an at-boundary kept-whole case.

**fail-open helper** (`tests/notchtap_agent_fail_open.rs`, integration
binary): the `notchtap-agent` hook exits 0 with empty stdout on
malformed stdin and when the loopback port is unreachable — the
observer guarantee that a stopped notchtap never blocks a real agent
session.

**provider fixtures** (`src-tauri/tests/fixtures/`): a redacted
native-payload fixture per documented lifecycle event per provider,
each exercised through the parser tests; every declared capability has
a fixture, unsupported events are dropped or mapped to Informational
explicitly, and secrets/raw tool output never survive into the
normalized event (never inferred from title/body wording). Codex's set
is deliberately smaller — its input-required/terminal-failure gaps are
declared, not fixtured.

**frontend**: `useAgentState` renders rust's ordered payload without
frontend-side lifecycle inference (validators reject malformed
sessions/subagents/tabSessions); `AgentBoard` — resting/expanded
precedence, per-session rendering, status icons, meta lines,
motion-vitals pins; the Settings Agents section — controls round-trip
config, adapter cards render mocked health, send-test invokes with the
card's own runtime; `hookEventParity.test.ts` pins `doctor.rs`'s hook
event lists against the Settings setup snippets per runtime
(region-scoped, because two runtimes share an event set). the OpenCode
plugin (`adapters/opencode/notchtap.test.ts`, included in the root
vitest run) covers its event mapping, port resolution, and delivery
against a mocked `fetch` — no real OpenCode runtime or network.

### 4.11 hover, tabs, prefix, click (rust + frontend)

- `hover.rs`: the card-rect width/height formulas per mode and state,
  the icon-strip rect geometry, and the lockstep pin holding the
  hovered flank width to the whole icon-count curve
- `tabs.rs` / `news_charge.rs` / `prefix.rs` / `click.rs`: selection
  and wire-label sets, the charge state machine, the arm/disarm/timeout
  machine, `watchdog_verdict`'s deadline table, and click hit-testing
  (present-list/rect zip, gap and out-of-band misses)
- frontend: tab-selection seam tests in `App`, `tabWireParity.test.ts`
  (the tab identity pin across `tabs.rs`/`lib.rs`/`IconStrip`/
  `iconPresence`), and `stripGeometryParity.test.ts` — a text-level pin
  holding `hover.rs`'s strip constants, `icon-strip.css`'s per-icon
  footprint, and `card-chrome.css`'s strip-visible `--cw` growth terms
  to the same numbers. cross-language parity pins like these are the
  standing pattern wherever rust and css/ts must agree on the same
  geometry or identity list.

### 4.12 history, status, engine, support modules

- `history.rs`: append/read round trip, missing-file-as-empty, last-n
  truncation, malformed-line skip, size-rotation, clear
- `status.rs`: snapshot serialization, derived-`PartialEq` change-guard
  behaviour (an ordinary content change emits once, repeats are silent)
- `engine.rs`: single-emit regression pins, accept→history recording
  rules (`Recurring` never recorded — a design tripwire), clear-waiting
  emit semantics
- `silence.rs`: window parse (incl. midnight-crossing), mute/skip/
  union/next-boundary
- `logging.rs` rotation engine, `net.rs` client posture, `crests.rs`,
  `lib.rs`'s pure hotkey/eval-splice/cutout helpers — each has its own
  small suite

---

## 5. what stays manual, and why

these aren't gaps to close later — they're inherent to what's being
tested:

- **notch-cutout anchoring on the actual macbook, hud rendering on the
  actual mac mini** — needs two specific physical machines; no ci
  runner reproduces this
- **animation look/feel** — subjective, visual, cheap to eyeball,
  expensive to automate for a one-person tool
- **real coding-agent lifecycle end-to-end** — provider hooks/plugins,
  Host behavior, and native application focusing require the installed
  runtimes. fixtures test normalization; only a real session proves
  installation and fail-open behavior.
- **expanded Agent Board pointer behavior** — tracking-area pointer
  delivery, wheel scrolling, menu-bar pass-through, and notch geometry
  are AppKit/hardware interactions. pure geometry is unit-tested; final
  behavior stays physical-machine manual.

### 5.1 modules with no test module, and why

silence is ambiguous — this list makes "untested by design" explicit,
so a missing `#[cfg(test)]` block is never mistaken for an oversight:

- **`lib.rs`** — partially tested: pure hotkey handlers, eval-splice
  escaping, and cutout helpers have suites; window/tray construction,
  heartbeat spawn, and the page-load gate stay untested thin
  orchestration of native apis. the logic they call is tested where it
  lives.
- **`login_item.rs`** — `SMAppService` shim; only observable against a
  real macos session.
- **`error.rs`** — `thiserror` declarations; variants asserted where
  produced.
- **poller fetch loops / `presentation.rs` subprocess spawn** — thin by
  design; everything downstream of "here is a response/stdout string"
  is the tested surface.
- **tracking-area / click-monitor / prefix registration plumbing** —
  AppKit callback wiring; the pure decision functions they feed are
  tested, the native delivery is §6 territory.

if a module on this list grows a real decision (a branch someone could
get wrong), it comes off the list and gets a test module in the same
change.

---

## 6. manual verification checklist

recurring, physical-hardware verification — re-run the relevant rows
per change; this list is never "done". `cargo test` and
`npx vitest run` must both be clean before any of this counts.

- [ ] manual push → visible animation, both machines
- [ ] startup log shows **notch** mode on the macbook — the hud
      fallback is silent by design, so this log line is the only tell
      that the detector worked
- [ ] mac mini build transferred via a quarantine-free method
      (`ARCHITECTURE.md` §13), and `notchtap-detect` built + symlinked
      on that machine too
- [ ] queue under load: push 5+ notifications rapidly, confirm exactly
      one item is ever Visible, the rest wait ordered by Priority tier
      → Rotation Order → arrival, and the Visible item
      rotation-dismisses into the next promotion
- [ ] tray: pause → new pushes answered `202` and buffered, nothing new
      renders, the Visible item finishes its own Rotation, nothing
      further promotes; resume → highest-priority Waiting item promotes
      immediately; quit exits the app
- [ ] notch-cutout anchoring looks correct on the macbook; hud
      placement looks correct on the mac mini
- [ ] the global expand hotkey toggles expand by real keypress; an
      auto-expanded card collapses on the first press
- [ ] the window survives a Spaces switch and stays visible over a
      fullscreen app (`NSWindowCollectionBehavior`)
- [ ] a live espn goal (`High` priority) preempts, renders, and rotates
      out correctly under the single-slot model
- [ ] hotkeys: ⌃⇧] with a Recurring item Visible requeues it (it
      returns after the queue laps) and promotes the next item; ⌃⇧]
      with a OneShot drops it; ⌃⇧, opens/focuses the settings window
      from any app — real keypresses, not the unit-tested pure handlers
- [ ] settings window (mac mini is enough): opens from the tray and
      re-focuses instead of duplicating; "Save & Relaunch" restarts the
      app with the change observably live; `start_paused = true` boots
      the app paused with the tray reading "Resume"
- [ ] `invoke("get_config")` from the *main* window's devtools console
      is denied — verifies the command acl actually gates (once per
      mechanism change)
- [ ] news: with `rss_enabled = true` against a live feed — first-poll
      silence, card rendering, the news manifest hotkey, ⌃⇧O opening
      the current story
- [ ] live-match card (`espn_live_card = true` during an actual live
      match): the card updates in place through kickoff/goals/cards
      rather than stacking, and retires after full-time
- [ ] silence: a Timed Mute buffers Medium/Low while a High push still
      breaks through compact; Skip ends today's Silent Period
- [ ] agents: real lifecycle smoke test per runtime (Claude Code,
      Codex, Kimi, OpenCode) against the actual CLIs — fixtures prove
      the parsing/registry contract, not that a real provider emits on
      schedule; T3 Code Host reporting + focus for at least two
      runtimes; ⌃⇧A focuses a known Host and fails quietly for an
      unknown one
- [ ] expanded Agent Board on hardware: tracking rect, wheel scroll,
      collapse-on-exit, menu-bar click pass-through
- [ ] icon strip on hardware: hover reveals the strip, a click selects
      the right tab at every live-source count, a pulled card shows
      compact with no countdown, and the prefix window arms/disarms
      without leaving a bare key grabbed

---

## 7. deep testing — the queue property suite

the example-based suite covers every listed transition; the property
suite adds *machine-generated adversaries* — random operation
interleavings checked against invariants after every step. it lives in
`src-tauri/src/queue.rs` (`mod proptest_queue`), driving a
`SingleSlotQueue` with a generated script of ops (`Enqueue` with
priority/rotation/topic/origin, `Tick`, `Dismiss`, `Skip`,
`ToggleExpanded`, `Pause`, `Resume`, silence ops) against a simulated
clock — no real sleeps. small generation bounds on purpose: proptest
shrinks failures toward minimal scripts.

the invariants, checked after every op:

1. at most one Visible item ever (structural; asserted as
   documentation)
2. per-tier waiting cap, checked only immediately after an `Enqueue`
   that lands in waiting — three documented bypasses legally exceed the
   cap (immediate-promote fast path; Recurring requeue; cross-tier
   Topic supersede). the harness distinguishes new-item from
   supersede-merge by total item count before/after.
3. no premature rotation: a `Tick` short of
   `promoted_at + window + extension` leaves the exact visible item
   unchanged; early removal only via explicit `Dismiss`/`Skip`
4. promotion picks the highest non-empty tier, minimum rotation-order
   rank within it, FIFO on a rank tie — the predictor mirrors the
   production `best_index_in_tier` exactly, with `rotation_order`
   generated per case
5. pause gates promotion, not aging; count conservation — accepted
   items always equal visible + waiting + dropped + dismissed +
   skipped-oneshot
6. supersession never creates a second item; a visible supersede's
   `extension_secs` never exceeds the cap
7. `slot_state_if_changed` never returns two consecutive equal states
8. expanded resets correctly on every promotion
9. `next_deadline()`, whenever `Some`, equals the visible item's
   `promoted_at + window + extension`

invariants 3, 4, 5, and 9 are the ones example-based tests can't
honestly claim — they quantify over *all* interleavings, plus hover
properties (a held card never rotates out; repeated hover cycles never
grant more than the rotation window).

**deliberately parked** (a decision with a trigger, not a gap):

- poller parse fuzz (`proptest` over `parse_scoreboard` junk/mutated
  fixtures) and frontend timing fuzz (`fast-check` over
  emit/clock-jump schedules) — pick up on the first parser or
  frontend-timing regression the example suites miss
- properties over `diff_scoreboard` *semantics* — the fixture cases are
  the spec; a property would re-encode the implementation. skip.
- mutation testing (`cargo-mutants`) — the "break it once, watch it
  fail" manual check buys most of the value at zero tooling cost.
- `test-cli.sh` for the `notchtap` script — only if the script grows
  beyond flag-parsing + curl; `sh -n` is the automated gate.

---

## 8. no global coverage percentage gate

resist tracking one repo-wide coverage number — it rewards testing
trivial getters and framework glue. the bar instead: every example
surface listed in §4 for a component has a passing test before work on
that component is called done.

## 9. running the suite

- `cargo test` (from `src-tauri/`) — all rust unit + integration
  tests, including the doc-tests
- `npx vitest run` (from repo root) — all frontend unit tests
- both must run clean before any work is called done
- ci runs the same two commands plus `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `npx tsc --noEmit`, `npx vite build`,
  audits, and a `swiftc` compile check — nothing ci-only; `just
  test-all` mirrors it locally
