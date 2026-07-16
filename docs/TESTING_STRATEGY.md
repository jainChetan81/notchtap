# mac-notification-nudge — testing strategy

companion to `ARCHITECTURE.md` (decisions) and `IMPLEMENTATION_PLAN.md`
(build sequence). this doc answers: what gets an automated test, what
doesn't, which framework, and in what order tests get written.

no test suite exists yet — this is the strategy claude code should
implement against once §1.1 of `IMPLEMENTATION_PLAN.md` is scaffolded.

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
| rust external http mocking (v2 espn poller) | `wiremock` (dev-dependency) | async-native, works directly with `reqwest`, lets fixture responses (success/malformed/timeout) be asserted without hitting the real espn endpoint from tests |
| frontend unit/component | `vitest` + `@testing-library/react` | vitest is free — the tauri react template already runs on vite, so it shares config and is fast; testing-library tests behaviour (what's rendered) not implementation details |

---

## 3. where to actually do tdd

red-green-refactor (write the failing test first) is worth the
discipline where the logic is pure, deterministic, and wrong-by-default
if untested. that's a short, specific list:

- **notification queue** — fifo ordering, cap-3 enforcement (4th item
  waits), ttl-based expiry
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

### 4.1 notification queue (rust)

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

### 4.2 event bus / dispatch router (rust)

- **type**: unit, tdd
- **coverage target**: ~100% — every event type + malformed-input path
- **example cases**:
  - well-formed `generic` event → routed to queue
  - unknown event `type` field → rejected, not silently dropped
  - missing required field (`title` or `body`) → rejected with a
    specific error, not a panic

### 4.3 `/notify` http handler (rust)

- **type**: integration, tdd, via `tower::ServiceExt::oneshot`
- **coverage target**: every response code path
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

### 4.4 notch/hud mode decision (rust or ts, wherever the check lands)

- **type**: unit, tdd
- **coverage target**: 100% — cheap, and it's the one native-adjacent
  decision that's actually testable
- **example cases**: isolate this as a pure function —
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

### 4.5 frontend visible-notification render state (react/ts)

- **type**: unit (vitest), tdd
- **coverage target**: every transition in the enter → hold → exit
  lifecycle, plus cap/ttl interaction with the rust-side queue's
  contract
- **example cases**:
  - receiving a tauri event adds an item to visible state
  - item past ttl removes itself from visible state
  - the frontend renders every `notification-promoted` event it
    receives without enforcing any cap itself — cap and promotion
    authority live rust-side (spec §8's queue-authority resolution); a
    4th visible item can only appear because rust promoted it. (an
    earlier draft of this case read "4th concurrent item does not
    render until a slot frees" — that predates the queue-authority
    resolution and described a frontend-side cap that must not exist.)

### 4.6 animation rendering (css/react)

- **type**: manual only, v1
- **why no automated visual regression**: the tooling cost (screenshot
  diffing, baseline management) isn't justified for a single generic
  template on a personal tool. revisit only if v2's per-event-type
  animation table (§4 of `ARCHITECTURE.md`) grows large enough that
  regressions become hard to eyeball.
- **manual check**: covered by `IMPLEMENTATION_PLAN.md` §5 checklist

### 4.7 espn scoreboard poller (v2, rust)

- **type**: unit, with `wiremock` fixtures — not tdd-first (external
  api shape needs to be observed before tests can assert against it),
  write tests once the real response shape is confirmed
- **example cases**:
  - well-formed scoreboard response → normalized `score-update` event
  - malformed/empty json → no crash, no event emitted
  - http timeout / 5xx from espn → backoff, no event emitted, no crash
  - never call the live espn endpoint from a test — fixtures only

### 4.8 cmux relay ingestion (v2)

- **type**: unit, tdd
- **coverage target**: 100% — small, pure env-var parsing
- **example cases**:
  - all three env vars present → forwarded correctly
  - missing `CMUX_NOTIFICATION_BODY` → handled explicitly, not a panic
  - empty-string values → handled explicitly

### 4.9 whatsapp/twilio outbound (v3)

- **type**: unit, mocked http client only
- **rule**: no test ever sends a real whatsapp message or makes a real
  twilio api call. mock the http client boundary; assert on the
  request that *would* have been sent.

---

## 5. what stays manual, and why (maps to `IMPLEMENTATION_PLAN.md` §5)

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

---

## 6. no global coverage percentage gate

resist tracking one repo-wide coverage number — it rewards testing
trivial getters and framework glue, and this repo has almost none of
that to begin with. instead, each phase's exit criteria
(`IMPLEMENTATION_PLAN.md` §5) should require: every example case listed
in §4 above for that phase's components has a passing test before the
phase is called done.

## 7. running the suite (once scaffolded)

- `cargo test` (from `src-tauri/`) — all rust unit + integration tests
- `npx vitest run` (from repo root) — all frontend unit tests
- both should run clean before any phase in `IMPLEMENTATION_PLAN.md` is
  marked complete — this is now also reflected in that doc's §5
