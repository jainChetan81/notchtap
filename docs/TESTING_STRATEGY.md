# mac-notification-nudge — testing strategy

companion to `ARCHITECTURE.md` (decisions) and `IMPLEMENTATION_PLAN.md`
(build sequence). this doc answers: what gets an automated test, what
doesn't, which framework, in what order tests get written — and, since
the 2026-07-16 merge, what is already done vs what is left. it absorbed
`DEEP_TESTING_PLAN.md` (now deleted) as §9; this is the only testing
doc.

---

## 0. status at a glance (2026-07-17) — done vs left

**done — built, green, ci-gated** (counts live here and only here;
other sections point back rather than repeating them):

| suite | size | where |
|---|---|---|
| rust unit/integration | 249 tests — settings 45, queue 52, http 27, notifier 23, rss_poller 28, poller 19, event 18, config 17, presentation 11, lib (hotkey) 9 | `cargo test` from `src-tauri/` |
| rust doc-tests | 3 — public `queue`/`event` apis | same `cargo test` run |
| frontend | 66 tests — presentation tables 14, slot-state hook 16, StatusRailCard 14, settings form 13, App render 5, presentation mode 4 | `npx vitest run` |
| ci (v4) | fmt, clippy `-D warnings` (`--locked`), cargo test (`--locked`), cargo-audit, npm audit, tsc, vitest, vite build, `sh -n` cli syntax check, swiftc compile check | every push + pr |

every example case listed in §4 for v1/v2/v3 components has a passing
test; the v4 §4.3 expansion (exhaustive status codes, queue edge
interleavings, sweep timing) is in; the v3 notifier suite (§4.9 —
telegram) landed 2026-07-16; the v3.6 single-slot rotating overlay
suite (§4.10) landed 2026-07-17, superseding §4.1's 3-item-cap queue
example cases; the v5 settings-window rust suite (§4.11), the v5
rss poller + status-rail news card suite (§4.12), and the v5.1
appearance/test-notification additions (queue test-promotion,
`send_test_notification`, `set_appearance`, AppearanceSection vitest)
all landed 2026-07-17 — §4.12's manual live-feed check is the only
piece still open, tracked in its own section.

**left — each is a decision with an owner section, not a gap:**

| item | status | where |
|---|---|---|
| deep testing work order (proptest invariants, http burst, poller fuzz, frontend timing fuzz) | **parked 2026-07-16** — un-park triggers in §8 | §9 (the full, implementation-ready plan) |
| ~~outbound connector tests~~ | **landed with v3 (telegram)** 2026-07-16 | §4.9 |
| ~~single-slot rotating overlay tests~~ | **landed with v3.6** 2026-07-17 (branch `v3.6-rotating-overlay`, not yet merged) | §4.10 |
| ~~v5 rss poller + news-card suite~~ | **landed 2026-07-17** (see §0) | §4.12 |
| v5 settings-window suite | **landed 2026-07-17, rust side and ui side both** (see §0) — validate/mask/round-trip/merge/write paths, plus the settings form + vitest cases (`IMPLEMENTATION_PLAN.md` §4.5 step 5) | §4.11, `V5_TECHNICAL_SPEC.md` §7 |
| manual checks not yet run (v3.6 hotkey keypress + Spaces/fullscreen survival, v5 news live-feed check) | needs the macbook + (for news) `rss_enabled = true` | §4.10, §4.12, §5, `IMPLEMENTATION_PLAN.md` §3.6.1/§4.6.1/§6 |
| `test-cli.sh` for the `notchtap` script | only if the script grows | §8 |
| manual hardware checklist | recurring per change — never "done" | §5, `IMPLEMENTATION_PLAN.md` §6 |

---

## 1. shape of the pyramid for this project

this project's pyramid is unusually bottom-heavy and has almost no
automated top layer, for a concrete reason: the highest-risk logic
(queue ordering, ttl expiry, event parsing) is pure and deterministic,
while the highest-visibility behaviour (notch-cutout geometry on real
hardware, css animation timing) depends on things a test runner can't
see — real `NSScreen` data on two specific physical macs, and rendered
visual output. don't fight that; put the automation budget where it
pays off.

```
        /  manual only   \      2 physical machines, real windows,
       /  (§5, checklist)  \     visual correctness — not automatable
      /------------------  \
     /   integration (some)  \   http layer, tauri command dispatch
    /------------------------  \
   /   unit tests (many, fast)   \  queue, event bus, parsing, reducers
```

---

## 2. framework choices

| layer | framework | why |
|---|---|---|
| rust unit/integration | built-in `cargo test` (`#[test]`, `#[tokio::test]`) | no extra dependency needed; idiomatic default; tauri already pulls in tokio, so async tests are free |
| rust http layer | `axum` + `tower`'s `ServiceExt::oneshot` (dev-dependency: `tower = { features = ["util"] }`) | lets the `/notify` route be tested in-process — no real socket bind, no port cleanup, no flaky "address in use" failures. this is also why axum is the pick over `tiny_http`: `IMPLEMENTATION_PLAN.md` §1.2 didn't pin an http crate — closing that gap here, in favour of axum specifically for this testability property |
| rust external http mocking (v2 espn poller) | none — dropped 2026-07-16 (was `wiremock`) | the poller design (v2 spec §3) keeps the fetch loop thin and untested; parsing, delta logic, and backoff are pure functions tested directly against captured fixture files, so nothing needs an http mock |
| frontend unit/component | `vitest` + `@testing-library/react` | vitest is free — the tauri react template already runs on vite, so it shares config and is fast; testing-library tests behaviour (what's rendered) not implementation details |
| rust doc-tests | built-in (`cargo test` runs them) | added 2026-07-16 — the public `queue`/`event` apis carry runnable examples that double as documentation. these are *not* the coverage layer (the `#[cfg(test)]` modules are); a doc-test exists to keep the documented usage honest, so keep them few and lifecycle-shaped, not exhaustive |
| deep testing, when un-parked (§9) | `proptest` (rust), `fast-check` (web, droppable) | dev-dependencies only; see §9 for the full rationale per section |

---

## 3. where to actually do tdd

red-green-refactor (write the failing test first) is worth the
discipline where the logic is pure, deterministic, and wrong-by-default
if untested. that's a short, specific list:

- **single-slot priority queue** (v3.6, formerly "notification queue" —
  fifo ordering, cap-3 enforcement, ttl-based expiry; see §4.10) —
  tier-strict promotion, fast-path never-jump, rotation (one-shot vs
  recurring), topic supersession with a capped extension
- **event bus / dispatch router** — event type routing, malformed
  payload rejection
- **`/notify` http handler** — request parsing, validation, status
  codes
- **notch/hud mode decision function** — see §4, this is the one piece
  of the native layer that's actually a pure function once isolated

tdd is **not** worth it, and shouldn't be forced, for:

- css animation timing/easing — write the animation, eyeball it, adjust
- the native swift `NSScreen` shim itself — there's nothing to assert
  against without the physical screen
- tauri window creation/positioning calls — thin wrappers around a
  native api; a unit test here would just be re-asserting the mock

---

## 4. component-by-component test plan

each subsection notes its status; "done" means every example case
listed has a passing test (the §6 bar), not that the component is
frozen.

### 4.1 notification queue (rust) — superseded by §4.10 (v3.6, 2026-07-17)

kept for historical record: this section describes the pre-v3.6
3-item-cap, pure-fifo `NotificationQueue`. that type no longer exists
— `queue.rs` now holds `SingleSlotQueue` (single slot, priority-tiered
waiting, rotation instead of ttl). the current, accurate example-case
list is §4.10; don't add new cases here.

- **type**: unit, tdd
- **coverage target**: every state transition (~100% branch coverage —
  small, deterministic module, this is the core value of the app)
- **example cases**:
  - enqueue 1 item → visible immediately
  - enqueue 4 items with cap=3 → 4th stays queued, not visible
  - item ttl expires → removed from visible set, next queued item
    promoted
  - queue empty → no-op, no panic
  - two items with identical ttl expire in enqueue order (fifo
    tie-break)
  - enqueue the 51st item while 50 are already waiting (`max_queued`,
    locked in `ARCHITECTURE.md` §3) → rejected, queue state unchanged,
    matches the `429` asserted at the http layer in §4.3 below
  - paused + free visible slot + enqueue → item lands in `waiting`,
    not `visible` (pause disables promotion even inside enqueue)
  - paused + visible item's ttl elapses → item removed on the next
    `expire_and_promote`, freed slot stays empty (expiry runs while
    paused; promotion doesn't)
  - resume followed by `expire_and_promote` → buffered items promote
    fifo into the free slots
  - `max_queued` enforced identically while paused (51st waiting item
    rejected)
  - (v4 expansion) burst-at-cap fifo/ttl accounting, exact expiry
    boundary (`>=` semantics), pause/resume interleavings with
    exactly-once promotion, resume promotes only up to cap
- property-based invariants on top of these: §9.1, parked

### 4.2 event bus / dispatch router (rust) — ✅ done

- **type**: unit, tdd
- **coverage target**: ~100% — every event type + malformed-input path
- **example cases**:
  - well-formed `generic` event → routed to queue
  - unknown event `type` field → rejected, not silently dropped
  - missing required field (`title` or `body`) → rejected with a
    specific error, not a panic

### 4.3 `/notify` http handler (rust) — ✅ done

- **type**: integration, tdd, via `tower::ServiceExt::oneshot`
- **coverage target**: every response code path
  (200/202/400/413/429 + method-not-allowed all asserted)
- **example cases**:
  - valid POST body → 200 with `{"status": "accepted"}`, event
    forwarded to bus
  - malformed json → 400, no crash
  - wrong content-type → 400
  - request from anything other than loopback — confirm the listener
    is bound to `127.0.0.1` only (this is a security boundary, not just
    a correctness one — worth a dedicated test asserting the bind
    address, not just handler behaviour)
  - queue already at `max_queued` (50, locked in `ARCHITECTURE.md` §3)
    → `429`, not `500` or a silently dropped event
  - queue paused → `202` with `{"status": "paused", "queued": <n>}`,
    event buffered into `waiting`, not dropped — and still `429` when
    full while paused
- burst accounting and exact 413/ttl boundaries: §9.2, parked

### 4.4 notch/hud mode decision (rust, `presentation.rs`) — ✅ done

- **type**: unit, tdd
- **coverage target**: 100% — cheap, and it's the one native-adjacent
  decision that's actually testable
- **example cases**: isolated as a pure function —
  `fn presentation_mode(safe_area_top_inset: f64) -> Mode` — so the
  test can pass in `0.0` (mac mini → hud) and a positive value
  (macbook → notch) without touching `NSScreen` at all. the actual
  `NSScreen.main?.safeAreaInsets.top` call stays a thin, untested
  boundary that feeds this function — don't let the untestable native
  call and the testable decision logic live in the same function.

**subprocess boundary (`notchtap-detect`)**: `ARCHITECTURE.md` §5 locks
the swift↔rust integration as a standalone cli (`notchtap-detect`)
invoked via `std::process::Command`, printing json to stdout. that json
parsing step is a second testable unit distinct from the pure decision
function above — the subprocess call itself (spawning `notchtap-detect`)
stays untested, same reasoning as `NSScreen`, but everything downstream
of "here is a string of stdout" is fair game:

- **type**: unit, tdd
- **coverage target**: every parse/failure path
- **example cases**:
  - well-formed json on stdout → parsed into the expected struct
  - malformed/truncated json → handled explicitly (fall back to hud
    mode, log, don't panic), not an unwrap
  - non-zero exit code from `notchtap-detect` → same explicit fallback,
    not a panic
  - binary not found on `PATH` → same explicit fallback — this is the
    most likely real-world failure (a fresh macos install, or the shim
    not yet built) and the one most worth a dedicated test

### 4.5 frontend visible-notification render state (react/ts) — ✅ done

- **type**: unit (vitest), tdd
- **coverage target**: every transition in the enter → hold → exit
  lifecycle, plus cap/ttl interaction with the rust-side queue's
  contract
- **example cases**:
  - receiving a tauri event adds an item to visible state
  - item past ttl removes itself from visible state
  - (v2 hardening) an item whose wall-clock deadline has passed is
    removed by the 1s sweep even if its setTimeout timers never fired —
    simulates system sleep / webview timer throttling (v2 spec §6.1);
    multi-item and not-yet-due sweep cases included (v4 expansion)
  - the frontend renders every `notification-promoted` event it
    receives without enforcing any cap itself — cap and promotion
    authority live rust-side (spec §8's queue-authority resolution); a
    4th visible item can only appear because rust promoted it. (an
    earlier draft of this case read "4th concurrent item does not
    render until a slot frees" — that predates the queue-authority
    resolution and described a frontend-side cap that must not exist.)
- generated emit/clock-jump schedules: §9.4, parked

### 4.6 animation rendering (css/react) — manual by design

- **type**: manual only
- **why no automated visual regression**: the tooling cost (screenshot
  diffing, baseline management) isn't justified for a single generic
  template on a personal tool. revisit only if v2's per-event-type
  animation table (§4 of `ARCHITECTURE.md`) grows large enough that
  regressions become hard to eyeball.
- **revisit trigger evaluated 2026-07-16**: v2's animation table landed
  (three event types: `generic`, `score_update`, `match_state`). three
  keyframe sets are still trivially eyeball-able, so the decision
  stands. re-evaluate if the table reaches ~6+ types or per-type
  styling starts regressing during unrelated css edits.
- **manual check**: covered by `IMPLEMENTATION_PLAN.md` §6 checklist

### 4.7 espn scoreboard poller (v2, rust) — ✅ done

- **type**: unit. the fetch loop stays thin and untested (§5.1);
  parsing and all delta logic live in pure functions
  (`parse_scoreboard`, `diff_scoreboard(prev, fetched)`) tested
  directly against captured fixture files — no http mocking (wiremock
  dropped 2026-07-16, see §2). not tdd-first (the external api shape
  was observed before tests asserted against it, per
  `IMPLEMENTATION_PLAN.md` §2.1)
- **example cases**:
  - well-formed scoreboard response → normalized `score-update` event
  - score delta against the snapshot → one `ScoreUpdate` per changed
    match; unchanged matches emit nothing
  - status delta (pre→in, in→halftime, →final) → one `MatchState`
  - first sighting of a match → no event (silent baseline, no restart
    flood)
  - match gone final → snapshot entry evicted immediately (after the
    full-time event)
  - match merely absent from a poll → carried forward, not evicted; a
    goal scored during the blip is still caught on reappearance;
    sustained absence (10 consecutive misses) evicts
  - malformed/empty json → no crash, no event emitted
  - http timeout / 5xx from espn → per-league backoff, no event
    emitted, no crash; the other leagues keep polling
  - never call the live espn endpoint from a test — fixtures only
- parse fuzz beyond hand-picked malformed fixtures: §9.3, parked

### 4.8 cmux relay ingestion (v2) — manual by design, live-verified

- **type**: manual (revised 2026-07-16 — this section originally
  planned rust unit tests for env-var parsing, written before the cli
  was locked as a flags-only shell script. there is no rust env-var
  parsing to test: cmux's notification command passes
  `$CMUX_NOTIFICATION_*` through shell expansion into `notchtap`'s
  flags, and the fold/empty-subtitle logic lives in the script.)
- an optional automated `test-cli.sh` is tracked in the v2 spec §8 —
  add it only if the script grows
- **example checks (manual)**:
  - all three env vars present → notification shows
    "subtitle — body" folded correctly
  - empty/unset `CMUX_NOTIFICATION_SUBTITLE` → body passes through
    untouched, no stray separator
  - end-to-end: live-verified 2026-07-16 on the mac mini (real claude
    code "needs input" alert surfaced through the overlay)

### 4.9 outbound connectors (v3 — telegram; whatsapp/twilio demoted)

(rewritten 2026-07-16 when v3 locked telegram-first — see
`IMPLEMENTATION_PLAN.md` §3; the old whatsapp/twilio framing predated
that decision. the no-live-calls rule is unchanged and applies to any
future connector too.)

- **type**: unit (pure fns, tdd) + `wiremock` integration (send path)
  + http-layer fan-out cases in §4.3's suite
- **rule**: no test ever sends a real telegram message. wiremock only.
- **pure, tdd'd first**:
  - `format_message` per event type + a nasty-characters escaping case
    (`<b>`, `&`, underscores, backticks in the body)
  - `escape_html` ampersand-first (no double-escaping)
  - `on_send_failure` — every arm: first transient → retry, first 400 →
    plain resend, fatal → drop, any second failure → drop
  - `ConnectorHandle::offer` — drop-on-full, never blocks
  - secrets loader against temp files (never `$HOME`): valid `0600`
    loads; missing file, non-`0600` perms, malformed toml each yield
    their specific `SecretsError` variant
  - config gate: `[connectors.telegram] enabled` parses, defaults to
    `false`
- **wiremock (send path)**:
  - 200 → exactly one request, html `parse_mode` present
  - 400 → exactly one plain-text resend, `parse_mode` absent
  - 5xx → exactly one retry, then drop
  - 401 (fatal) → no retry at all
- **http fan-out (in §4.3's suite)**: accepted push lands in a test
  connector channel; 429-rejected push does not; paused `202` push
  **does** (acceptance succeeded — v3 spec §1)

### 4.10 single-slot rotating overlay (v3.6 — priority queue, rotation, hotkey expand)

landed 2026-07-17 (branch `v3.6-rotating-overlay`, code-level contract
in `docs/V3_6_TECHNICAL_SPEC.md`). supersedes §4.1's notification-queue
example-case list — same file (`queue.rs`), renamed type
(`SingleSlotQueue`), materially different model (one slot, not three;
priority tiers, not pure fifo; rotation, not ttl).

- **type**: unit, written alongside the implementation against the
  frozen types in `V3_6_TECHNICAL_SPEC.md` §3/§4
- **coverage target**: every state transition in the single-slot
  model — tick/rotation, tier-strict promotion, fast-path,
  supersession (including the hard extension cap), pause/resume, and
  the `slot_state_if_changed` change-guard
- **example cases** (`queue.rs`, see §0 for the current count):
  - `tick`: never-interrupt (a `High` enqueue while something is
    Visible does not promote until the Visible item's own rotation
    elapses), tier-strict promotion order, fifo within a tier,
    `OneShot` drops forever, `Recurring` requeues to the back of its
    **own** tier
  - fast-path: a push with any tier non-empty never fast-path-promotes,
    even a `High` push arriving while only `Low` is waiting — the
    3-tier generalization of the old `fast_path_never_jumps_waiting_items`
  - supersession: a visible-item supersede updates
    payload/priority/rotation and grants a capped extension only when
    remaining time is already below the 2s floor (`promoted_at` is
    never mutated — only `extension_secs`); a burst of 25 rapid
    below-floor supersedes still rotates the item out at exactly
    `base_window + 6s`, never later, regardless of how many land; a
    same-tier waiting supersede keeps its queue position; a
    priority-changing supersede moves to the back of its *new* tier
    (not its old one, not the front)
  - per-tier cap: a full `Low` tier rejects a new `Low` push while a
    simultaneous `High` push is still accepted (`max_queued_per_tier`,
    independent per tier — a `Low` burst can't starve `High`'s own
    waiting room)
  - pause/resume: pause gates promotion, not rotation (an already-
    Visible item still ages out while paused); resume promotes
    immediately on the next `tick`, not the next heartbeat
   - `slot_state_if_changed`: suppresses a re-emit when nothing changed
     between two ticks; an actual promotion, rotation-to-empty, or
     expand toggle always emits
   - **expanded semantics** (plan 008, 2026-07-17 — `queue.rs`):
     automatic for `High` on both promotion call sites (the
     `enqueue_new` immediate-promote fast path, and `promote_next` via
     `tick`/rotation), reset to `false` for every non-`High` promotion
     (a leftover manual expand from the previous item never leaks onto
     the next one), the expanded rotation window applies to an
     auto-expanded `High` item exactly as it does to a manually-toggled
     one, and `toggle_expanded` is a no-op while the slot is Empty (an
     idle press arms nothing for whatever promotes next)
- **`Priority` ordering** (`event.rs`): `Low < Medium < High` pinned by
  a dedicated test — the array-index promotion logic in `queue.rs`
  depends on declaration order matching `Ord`, so a rustfmt/refactor
  reorder would silently break promotion without this test
- **`SlotState` wire contract** (`event.rs`): a `serde_json::to_value`
  snapshot test on `SlotState::Showing` pins the exact camelCase field
  names. this caught a real bug during implementation:
  `#[serde(rename_all = "camelCase")]` on the enum only renames the
  variant *tag* ("showing"), not fields inside the struct variant
  (`event_type` stayed `event_type` instead of becoming `eventType`)
  — needs the additional `rename_all_fields = "camelCase"` attribute
  too. flagged here because it's exactly the drift
  `V3_6_TECHNICAL_SPEC.md` §5.2's "integration risk" note warned
  about, and the snapshot test caught it on the first real run rather
  than shipping a frontend that silently never renders anything.
- **hotkey no-op branch** (`lib.rs`): `toggle_manual_expand`'s pure
  decision (no-op while a `High`-priority item is Visible — because
  it's already auto-expanded, per plan 008 — toggles otherwise) is
  unit-tested directly against a `SingleSlotQueue` and a
  `tauri::test::mock_app()` handle, bypassing the actual OS hotkey —
  same split as §4.4's subprocess boundary
- **frontend** (`useSlotState.ts` + `App.tsx`, 10 of the 14 total
  frontend tests): renders `empty` as nothing; renders `showing` with
  the right priority/expanded classes; a new `slot-state` payload
  replaces content directly, without an intermediate empty frame;
  listener cleanup on unmount
- **cli** (`notchtap --priority low|medium|high`): manual only, same
  as the rest of the script (§4.8) — `sh -n` syntax check is the only
  automated gate

**manual-only, not automatable** (extends §5):
- the global hotkey keypress actually toggling expand on real
  hardware — the pure decision logic above is unit-tested; os-level
  registration and keypress delivery are not
- the window surviving a Spaces switch and staying visible over a
  fullscreen app (`NSWindowCollectionBehavior`)
- a live espn goal auto-expanding (`High` priority) and rotating out
  correctly under the new single-slot model, on the macbook

### 4.11 settings window (v5 — rust side landed 2026-07-17; form held for the ui migration)

code-level contract in `V5_TECHNICAL_SPEC.md` (§7 is the source this
section mirrors); build sequence in `IMPLEMENTATION_PLAN.md` §4.5.
this is the app's first frontend→rust invoke surface, so the suite's
job is twofold: the usual pure-logic coverage, plus pinning the
security boundary (the overlay must stay receive-only).

- **type**: unit (pure fns, tdd) + temp-dir integration (write paths)
  + vitest (form) — no new frameworks, no new dependencies
- **pure, tdd'd first** (`settings.rs`):
  - `validate` — every rule's accept/reject boundary (`port` 1024
    floor, `default_ttl` 1..=3600, `max_queued_per_tier` 1..=1000,
    `espn_poll_secs` 5..=3600, league entries non-empty/no-whitespace,
    empty league list rejected only when `espn_enabled = true`)
  - `mask` — long value → `set (…last4)`, short value → `set` (no
    partial leak), boundary length
  - config serialize→parse round-trip — a non-default `Config`
    through `toml::to_string_pretty` then `Config::parse` compares
    equal; pins the new `Serialize` derive against field drift (same
    spirit as §4.10's `SlotState` snapshot test)
  - secrets merge — setting the openrouter key preserves an existing
    telegram table and vice versa; a malformed existing file yields
    an error, never a clobber; `SecretField` covers exactly the three
    allowed fields
- **temp-dir integration** (never `$HOME` — same rule as §4.9's
  secrets-loader tests):
  - atomic config write: result parses via `Config::parse`, the temp
    file is gone after rename, a missing parent dir is created
  - secrets write: resulting file is mode `0600` and loads through
    the existing `load_secrets`
- **2026-07-17 review-round additions**:
  - malformed secrets containing a sentinel never echo that material
    through either the settings-facing error or the connector error's
    `Display`
  - unknown top-level tables and unknown fields inside known secret
    tables survive a write
  - leading/trailing clipboard whitespace is trimmed before secret
    validation and storage; interior whitespace remains invalid
  - a stale permissive fixed-name temp file is never reused or written
    into
  - submitted `detect_path` is replaced with the booted value before a
    config save
  - rss feed validation requires a fully parsed http(s) url with a host,
    not just a matching prefix
  - `ensure_settings_window` accepts the `settings` label and rejects
    `main`, using `tauri::test::mock_app()` + `WebviewWindowBuilder`
- **frontend (vitest, small)**: form renders values from a mocked
  `get_config`; a mocked `save_config_and_relaunch` rejection renders
  the error list. `@tauri-apps/api/core`'s `invoke` is mocked — no
  webview in ci. overlay tests untouched.
- **security boundary, pinned two ways**:
  - automated: `capabilities/default.json` unchanged in the diff
    (review-level check, called out in the v5 exit criteria)
  - manual, once: `invoke("get_config")` from the *main* window's
    devtools console is denied — verifies the `build.rs`
    `AppManifest::commands` opt-in + per-window capability actually
    gate (v4 §4.4's "does the gate gate" discipline)
- **untested by design** (extends §5.1's list): lazy settings-window
  creation and tray-item wiring remain thin native glue, but window
  construction is now partially covered by the label-gate test's mock
  `WebviewWindowBuilder`; `app.restart()` remains untested (kills the
  process — nothing to assert from inside it); the openrouter key's
  *use* remains untested because no consumer exists until the first ai
  feature

### 4.12 rss news poller + status-rail news cards (v5 news — landed 2026-07-17)

code-level detail lives with the feature, not a separate spec; build
sequence in `IMPLEMENTATION_PLAN.md` §4.6. `rss_feeds`/`rss_enabled`/
`rss_poll_secs`/`rss_ttl_secs`/`rss_max_per_poll` validation rules live
in `settings.rs` and are already covered by §4.11 (folded into that
suite's count the same day, not duplicated here) — this section is the
poller and the frontend render path only.

- **type**: unit (pure fns, tdd) — same shape as §4.7's espn poller:
  the fetch loop stays thin and untested (§5.1), parsing/dedup/diff
  logic are pure functions tested directly against fixtures, including
  real-shaped ndtv captures. no live rss fetch in any test.
- **example cases** (`rss_poller.rs`, see §0 for the current count):
  - `SeenStore`: bounds enforcement (1k keys, oldest evicted first),
    7-day eviction, guid dedup, canonical-link fallback when a guid is
    absent, cross-feed duplicate guard (the same story from two feeds
    only surfaces once)
  - sanitize: strips markup/entities from real-shaped ndtv fixture
    items without mangling plain text
  - diff/baseline: first poll per feed is silent (no restart flood,
    same rule as §4.7's espn baseline); subsequent polls emit only
    unseen items in feed order; `rss_max_per_poll` caps a single poll's
    emissions (replay bug-guard) without dropping the excess from
    `SeenStore` (they're still marked seen, not re-offered next poll)
  - metadata derivation: category from entry `<category>` tags via the
    keyword table, falling back to the feed's configured default;
    source from `[[rss_feeds]]` config, falling back to the parsed
    feed title
  - malformed/empty feed body → no crash, no event emitted (same
    failure-mode contract as §4.7)
- **frontend (vitest, part of the frontend total — see §0 for current
  counts — presentation tables and `StatusRailCard` cover the news
  branch)**: masthead render
  (`{source} · Wire`), 2-line clamped headline, category + age pill
  content, `stampFor`/`categoryClass`/`ageLabel`/`publishedLabel`
  lookup-table cases including unknown-category fallback, null-metadata
  fallback (non-news `SlotState` items render with no source/category/
  publishedAtMs and don't crash the news branch), the news manifest's
  3-column layout
- **untested by design** (extends §5.1): the category-hued gradient
  shader's visual output and reduced-motion behaviour — same
  eyeball-only reasoning as §4.6; `fetch_feed`'s decision surface (304
  short-circuit, validator-persist-only-after-success ordering, the
  content-length/streamed size cap) is wiremock-tested (`rss_poller.rs`'s
  `fetch_feed_tests`), so only the spawn loop itself stays thin-by-design,
  same as §4.7's espn poller
- **manual, not yet run**: `rss_enabled = true` against the live ndtv
  feed — first-poll silence, masthead/shader/pill rendering, the news
  manifest hotkey, a `High`-priority push preempting a queued headline;
  tracked in `IMPLEMENTATION_PLAN.md` §4.6.1 and §6

---

## 5. what stays manual, and why (maps to `IMPLEMENTATION_PLAN.md` §6)

these aren't gaps to close later — they're inherent to what's being
tested:

- **notch-cutout anchoring on the actual macbook, hud rendering on the
  actual mac mini** — needs two specific physical machines; no ci
  runner reproduces this
- **animation look/feel** — subjective, visual, cheap to eyeball,
  expensive to automate for a one-person tool
- **cmux "agent needs input" end-to-end** — needs a real cmux session
  raising a real claude code permission prompt; can't be synthesized
  without also faking cmux itself, which would test the fake, not the
  integration

### 5.1 modules with no test module, and why (recorded 2026-07-16)

silence is ambiguous — this list makes "untested by design" explicit
per module, so a missing `#[cfg(test)]` block is never mistaken for an
oversight:

- **`lib.rs`** — partially tested: the pure hotkey handlers
  (`toggle_manual_expand`, `dismiss_current`, `toggle_pause`) have their
  own suite, §4.10. the rest — window, tray construction, heartbeat
  spawn, page-load gate — stays untested by design: thin orchestration
  of native apis; §3's "don't test thin wrappers" rule. the logic it
  calls (queue, emit rule) is tested where it lives.
- **`logging.rs`** — file-appender setup and rotation glue. filesystem
  side effects, no decision logic worth asserting.
- **`login_item.rs`** — `SMAppService` registration shim. only
  observable against a real macos session; manual checklist territory.
- **`error.rs`** — `thiserror` declarations only; the variants are
  asserted where they're produced (queue, event, http tests).
- **`poller.rs` fetch loop** (`fetch_league` + the spawn loop) —
  deliberately thin per v2 spec §3; everything downstream of "here is
  a response body string" (parse, diff, backoff decisions) is the
  tested surface.
- **`presentation.rs` subprocess spawn** — the `std::process::Command`
  call to `notchtap-detect`; §4.4 already covers why only the parse +
  fallback paths downstream of it are tested.

if a module on this list grows a real decision (a branch someone could
get wrong), it comes off the list and gets a test module in the same
change.

---

## 6. no global coverage percentage gate

resist tracking one repo-wide coverage number — it rewards testing
trivial getters and framework glue, and this repo has almost none of
that to begin with. instead, each phase's exit criteria
(`IMPLEMENTATION_PLAN.md` §6) should require: every example case listed
in §4 above for that phase's components has a passing test before the
phase is called done.

## 7. running the suite

- `cargo test` (from `src-tauri/`) — all rust unit + integration tests,
  including the doc-tests on the public `queue`/`event` apis
- `npx vitest run` (from repo root) — all frontend unit tests
- both should run clean before any phase in `IMPLEMENTATION_PLAN.md` is
  marked complete — this is now also reflected in that doc's §6
- ci (v4) runs the same two commands plus `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `npx tsc --noEmit`, `npx vite build`,
  and a `swiftc` compile check — nothing ci-only

---

## 8. planned / deliberately-not-yet (as of 2026-07-16)

tracked here so "not done" is a decision with a trigger, not a gap:

- **deep testing (§9)** — the one genuine rigor upgrade available; the
  full implementation-ready work order is §9 below. reviewed and
  **parked 2026-07-16**: the example-based suite covers every listed
  transition and the extra rigor wasn't judged worth the work yet.
  **trigger to un-park**: first queue regression the example cases
  miss, the next queue-semantics change (e.g. a priority lane), or the
  user asking for it. when picked up, §9 is the work order — don't
  re-plan.
- ~~**v3 connector tests**~~ — landed with v3 (telegram, not
  twilio/whatsapp — that demotion is in `IMPLEMENTATION_PLAN.md` §3);
  see §4.9. the no-live-calls rule held: wiremock only.
- **`test-cli.sh` for the `notchtap` script** — v2 spec §8, add only if
  the script grows beyond flag-parsing + one curl.
- **visual regression for animations** — §4.6's trigger, re-evaluated
  and declined 2026-07-16; see there.

---

## 9. deep testing work order — parked, implementation-ready

**status: parked 2026-07-16 — reviewed and accepted, deliberately not
scheduled.** nothing in this section is implemented. un-park triggers
are in §8. when picked up, follow §9.6's build order and review gates
as written. (formerly the standalone `DEEP_TESTING_PLAN.md`, merged
here 2026-07-16 so the testing story lives in one document.)

like `V3_6_TECHNICAL_SPEC.md`, this section is not locked: adjust freely
as implementation surfaces friction; fold any *decision* changes back
into §1–§8.

**stale as of 2026-07-17 — retarget before un-parking**: §9.1's
`Op`/invariant design below (`NotificationQueue`, `max_concurrent`,
`visible().len()`, `expire_and_promote`) targets the pre-v3.6 queue
shape, which no longer exists (`queue.rs` now holds `SingleSlotQueue`
— see §4.10). the *properties* (cap, bound, exactly-once promotion,
fifo, paused-ticks-silent, rejection-leaves-state-untouched,
no-premature-rotation) still make sense conceptually, but the `Op`
enum, field names, and the fifo-within-a-single-queue framing (I4)
need reworking for three priority tiers, `tick`/rotation naming, and
supersession before this is implementation-ready again. whoever
un-parks §9 does that retarget pass first, as its own step before
9.6's build order.

### 9.0 what "deep" means here, and what it doesn't

the existing suite is example-based: every listed transition has a
hand-written case. deep testing adds *machine-generated adversaries* —
random operation interleavings and malformed inputs — checked against
*invariants* (properties that must hold after every step, no matter the
sequence). it finds the interleavings nobody thought to write.

explicitly **not** in this section (unchanged from §5):

- notch geometry, hud placement, animation look — physical/manual
- live espn or twilio calls — fixtures and mocks only, in ci and locally
- visual regression — §4.6's trigger was re-evaluated and declined
- coverage percentage gates — still banned (§6's reasoning stands)

### 9.1 queue property tests (rust, `proptest`) — the core

**file**: `src-tauri/src/queue.rs`, new `#[cfg(test)] mod proptests`
alongside the existing example-based `mod tests` (same module so it can
keep using `expire_visible_for_test`).

**dependency**: `proptest = "1"` under `[dev-dependencies]` in
`src-tauri/Cargo.toml`. dev-only — no shipped-binary impact.

**the operation model** — drive a `NotificationQueue` with a generated
script of operations against a simulated clock (a `start: Instant` plus
an accumulated `Duration` — never real sleeps):

```rust
enum Op {
    Enqueue { ttl_secs: u64 },   // ttl in 1..=10
    Pause,
    Resume,
    Advance { ms: u64 },          // 0..=3000, clock moves, no tick
    Tick,                         // expire_and_promote(simulated now)
}
```

generator shape: `vec(any_op(), 0..120)` operations, with queue
parameters themselves generated per case: `max_concurrent in 1..=5`,
`max_queued in 1..=10`. small bounds on purpose — proptest shrinks
failures toward minimal scripts, and small state spaces shrink better.

each `Enqueue` carries a fresh uuid; the harness records, per op, what
the queue accepted vs rejected, and drains `take_promoted()` after
*every* op (not just ticks — the enqueue fast-path promotes too).

**the invariants (checked after every single op)**:

- **I1 — cap**: `visible().len() <= max_concurrent`, always.
- **I2 — bound**: `waiting().len() <= max_queued`, always.
- **I3 — exactly-once promotion**: no event id ever appears in the
  drained `take_promoted` stream twice. at script end, run the drain
  protocol (resume + advance past max ttl + tick, repeated until
  empty); then every *accepted* id has appeared exactly once, every
  *rejected* id never.
- **I4 — fifo**: the concatenated promotion stream is exactly the
  accepted-enqueue order (promotion never reorders; it's a prefix
  relation at every step and equality at drain-end).
- **I5 — paused ticks are silent**: a `Tick` executed while paused
  yields an empty `take_promoted` drain.
- **I6 — rejection leaves state untouched**: on `QueueError::QueueFull`,
  `visible`/`waiting` lengths and contents (by id) are identical before
  and after the failed call.
- **I7 — no premature expiry**: an item never leaves `visible` before
  its ttl has elapsed on the simulated clock (cross-check the removal
  against `promoted_at + ttl <= now`).

I3 and I4 are the ones example-based tests can't honestly claim — they
quantify over *all* interleavings of pause/resume/expiry windows,
including the freed-slot-between-ticks window the fast-path guards
against (`queue.rs` `can_promote_now`).

**expected size and cost**: one proptest
(`#[test] fn queue_invariants_hold_under_any_op_script()`) with the
default 256 cases, plus proptest's persisted-failure regression file
(`src-tauri/proptest-regressions/`, committed — that's the point: a
found counterexample becomes a permanent regression test). runtime
target: well under 1s; all clock math is simulated.

**exit criteria**:

- the property passes 256 cases locally and in ci
- deliberately breaking the queue (e.g. inverting the
  `waiting.is_empty()` fast-path guard, or letting promotion run while
  paused) makes the property fail with a small shrunk script — verify
  both mutations once, then revert. this is the "does the gate actually
  gate" check, same discipline as v4 §4.4.
- the existing example-based queue tests stay untouched and green —
  the property *supplements* them (readable spec cases stay readable),
  it does not replace them

### 9.2 http layer — burst and boundary integration cases

**file**: `src-tauri/src/http.rs`, extending the existing `mod tests`.
no new dependencies (tower `oneshot`, as everywhere in §4.3).

- **burst accounting**: with `max_concurrent = 3`, `max_queued = 50`,
  fire 60 sequential posts at the router. assert exactly 53 succeed
  (200) and 7 are rejected (429), and the queue ends at 3 visible + 50
  waiting. this is the http-visible face of queue invariants I1/I2.
- **paused burst accounting**: same, paused: 50× 202 then 10× 429,
  nothing visible.
- **boundary body sizes**: a body exactly at the 413 limit passes; one
  byte over is rejected. (the limit exists and is tested today only
  with a grossly oversized body — pin the exact boundary.)
- **ttl clamping/normalization** at the handler: whatever the v1 spec
  §7 says about absent/zero/absurd `ttlSecs` values gets one case each
  (absent → `default_ttl` is presumably covered; add `0` and
  `10_000_000` explicitly, asserting the documented behaviour — check
  the spec first, and if the behaviour is *undocumented*, that's a spec
  gap to fix in `archive/V1_TECHNICAL_SPEC.md` §7 before writing the
  test — archived, but still the doc of record for v1's `ttlSecs`
  behaviour).

true concurrency (simultaneous in-flight requests) is deliberately not
simulated: the queue sits behind a mutex, so interleaving reduces to
ordering — which §9.1 already covers exhaustively.

**exit criteria**: each case above written and green; the existing http
tests untouched.

### 9.3 poller robustness — parse fuzz (rust, `proptest`)

**file**: `src-tauri/src/poller.rs`, new `#[cfg(test)] mod proptests`.
reuses the §9.1 dependency.

- **`parse_scoreboard` never panics**: feed arbitrary strings
  (including non-utf8-boundary junk via `\PC*` and truncated prefixes of
  a real fixture) — the result is `Ok` or `Err`, never a panic. this
  hardens the "undocumented public endpoint changes shape without
  notice" failure mode (`IMPLEMENTATION_PLAN.md` §2.1) beyond the
  hand-picked malformed fixtures.
- **fixture-mutation fuzz**: take the committed real fixture, apply
  generated structural mutations (delete a random key, null a random
  value, retype a number to a string), and assert `parse_scoreboard` +
  `diff_scoreboard` combined never panic and never emit an event with an
  empty title. this catches the "half-changed payload shape" case that
  pure junk strings don't reach.

**exit criteria**: both properties green at 256 cases; no live network
anywhere (unchanged rule).

### 9.4 frontend deep timing tests (vitest + `fast-check`)

**file**: `src/useVisibleNotifications.test.tsx` (or a sibling
`useVisibleNotifications.property.test.tsx` if the file gets long).

**dependency**: `fast-check` as a devDependency (the vitest-ecosystem
proptest equivalent; integrates with fake timers cleanly).

- **no immortal cards**: for a generated sequence of
  (emit, advance-fake-clock) steps — including advances that skip past
  deadlines in one jump, simulating sleep/timer-throttling — after
  advancing past every emitted item's deadline **plus one sweep
  interval**, visible state is empty. this generalizes the hand-written
  sweep cases (v2 spec §6.1) to arbitrary schedules.
- **no duplicate renders**: duplicate ids across the generated emits
  never yield two simultaneous cards with the same id.
- **phase monotonicity**: a card's phase only ever moves
  enter → hold → exit (never backwards) across any advance schedule.

scope guard: the hook remains rendered through the real
`renderHook`/fake-timer harness the existing tests use — no new render
infrastructure. if fast-check + fake timers fight each other in
practice, fall back to a seeded hand-rolled fuzz loop (a plain test
generating 100 random schedules from a fixed seed) — the invariants
matter, the framework doesn't.

**exit criteria**: three properties green; the existing frontend tests
untouched; `npx vitest run` stays under ~5s.

### 9.5 deliberately still out (even when §9 is un-parked)

- **proptest on `diff_scoreboard` semantics** (beyond §9.3's
  never-panic): the delta logic's *meaning* (which transitions emit
  what) is spec-by-example; the fixture cases are the spec. a property
  here would just re-encode the implementation. skip.
- **doc-tests beyond queue/event**: the other pub-worthy surfaces
  (`http::router`, `poller::parse_scoreboard`) are internal; making
  them pub just for doc-tests inverts the §2 doc-test rule (few,
  lifecycle-shaped, on genuinely public api). skip.
- **mutation testing (`cargo-mutants`)**: interesting, but the §9.1 /
  v4-style "break it once, watch it fail" manual check buys most of the
  value at zero tooling cost for a one-person repo. note as a future
  idea only.

### 9.6 build order and review gates

1. §9.1 queue proptest (highest value, pure, no new test infra beyond
   the dep) → review the shrunk-failure ergonomics before proceeding
2. §9.2 http burst/boundary cases (no deps, quick)
3. §9.3 poller fuzz (reuses proptest)
4. §9.4 frontend fast-check (new dep on the web side — last, so a
   decision to drop it doesn't block the rust work)

each step lands with `cargo fmt --check`, `cargo clippy -- -D warnings`,
`cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npx vite build`
all green — same gates as ci, no exceptions. as each section lands,
update §0's status table and the per-component pointers in §4.
