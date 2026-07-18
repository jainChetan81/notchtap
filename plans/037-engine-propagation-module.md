# Plan 037: the Engine — one propagation module for every Slot mutation

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report — do not
> improvise. When done, update this plan's status row in
> `plans/README.md` — unless a reviewer dispatched you and said they
> maintain the index.
>
> **Dependency gate (run BEFORE the drift check — a hard positive
> check, because the drift check alone cannot detect a missing
> dependency)**: this plan's excerpts describe the codebase AFTER plans
> 036 and 025 have landed. Verify both:
> 1. `plans/README.md` status rows for **036** and **025** read DONE.
> 2. `rg -n "enable\(\)" src-tauri/src/lib.rs` hits inside
>    `spawn_heartbeat`'s lock block (036's waiter-under-lock shape).
> If either check fails, STOP and report "dependencies not landed" —
> do NOT execute against pre-dependency code. In particular, porting
> today's pre-036 heartbeat loop "verbatim" into the Engine would
> perpetuate the P1 lost-wakeup race 036 exists to fix.
>
> **Drift check (run second)**:
> `git diff --stat d926977..HEAD -- src-tauri/src/lib.rs src-tauri/src/http.rs src-tauri/src/queue.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src-tauri/src/event.rs CONTEXT.md`
> Drift from plans **036**, **025**, and **024** (proptest
> rotation_order) is EXPECTED — reconcile the excerpts below against
> their landed shapes before starting. Drift from **033/035** (queue
> slot-state counters, `/notify` subtitle contract) is a STOP:
> re-verify every excerpt against the live code and update this plan
> first. Line numbers are indicative; locate every target with the `rg`
> command given. Run `git status` first — if any in-scope file is dirty
> in the working tree, STOP and report rather than layering onto
> someone's in-flight edit.
>
> **Filing note (operator)**: this plan was filed together with the
> CONTEXT.md Engine entry and the plans/README.md row, uncommitted.
> Commit that filing before dispatching an executor — otherwise the
> dirty-tree gate above fires immediately.

## Status

> **BLOCKED 2026-07-19 — do not execute this plan as written.** Plans
> 032–034 landed on master while an executor was mid-run from
> `4ad4be5`, rewriting queue.rs/lib.rs/http.rs underneath it: 033
> reversed the High-only auto-expand (invariant 8 below is stale and
> the queue gained batch counters), the heartbeat now carries the
> live-match handle + StatusState emission (plan 034 — `spawn_rotation`'s
> port target changed shape), and 035's `/notify` contract changes are
> still inbound. The design decisions (closure-based `apply`, private
> queue+wake, `accept` with the News gate, Engine-owned clock,
> rotation loop inside the Engine) remain operator-locked and valid.
> A partial reference implementation (Steps 1–2 against the old base)
> is preserved on branch `exec/037-engine`. **Next action: after 035
> lands, run `/improve review-plan` on this file to retarget every
> excerpt, the op model, and the invariant list at settled master —
> then re-execute.**

- **Priority**: P2
- **Effort**: L
- **Risk**: MEDIUM — a wide but mechanical refactor across every caller
  of the queue. Semantics must NOT change (one deliberate exception,
  documented below); the plan-022 proptest suite and the full example
  suite are the safety net.
- **Depends on**: **036 (hard)** — this plan rewrites the heartbeat
  loop 036 fixes and must preserve its `Notified::enable`-under-lock
  shape; **025 (hard)** — 025 reshapes the poller fetch paths this plan
  migrates; running 037 first forces 025 into a painful rebase;
  **024 (soft)** — 024 extends the `mod proptest_queue` harness whose
  enqueue call sites this plan mechanically edits (if 024 is still IN
  PROGRESS, coordinate before dispatch); **033/035 (ordering only)** —
  they change queue/http production surfaces; whoever lands second
  reconciles textually (repo precedent).
- **Category**: tech debt / architecture
- **Planned at**: commit `d926977`, 2026-07-18. Design decided in an
  operator-confirmed architecture-review grilling session (interface
  shape, read path, fan-out rule, clock ownership, name, sequencing —
  each an explicit operator selection). CONTEXT.md gained the
  **Engine** term in the same filing. **Review-plan pass 2026-07-18**
  (advisor + independent cold-read against live code at `d926977`+
  filing): added the dependency gate (the drift check alone cannot
  detect that 036/025 haven't landed — and they had NOT at review
  time), the missed `on_page_load` re-emit site and its
  `emit_current_blocking` migration, the full managed-state/setup
  wiring list (Engine construction point, `app.manage(engine)`,
  `state::<Engine>()` retrieval in `on_page_load`, by-value queue in
  `Engine::new`), the new `send_test_notification` signature, the
  CONTEXT.md scope listing, and the split poller verify commands.

## Why this matters

Every queue mutation in the codebase must follow the same protocol:
lock → mutate → `slot_state_if_changed()` → unlock →
`wake.notify_waiters()` → `emit_slot_state`. Miss the wake and the
deadline heartbeat (plan 015) oversleeps or wedges; miss the emit and
the overlay shows a stale Slot. Today that protocol is **enforced by
convention at ~8 hand-written sites**; exactly one of them —
`http::enqueue_and_emit` — encodes it structurally (its own doc
comment: "a mutation without a wake is structurally impossible to
express here") and is regression-tested.

Evidence the convention has real cost:

- Plan 015 had to add the wake at every site by hand, and its review
  caught a missed one (Settings test-notifications never auto-
  dismissing).
- Plan 036 is a shipped P1 bug in exactly this protocol's territory,
  and its maintenance note pleads for the convention: "Any future
  queue-mutation site must keep the contract: lock → mutate → unlock →
  `notify_waiters()` … a mutation that bypasses the queue lock would
  reopen the race." This plan turns that plea into structure: after it,
  a mutation that skips the protocol does not compile, because nothing
  outside the Engine can reach the queue.

**This is not the rejected "lib.rs multi-responsibility split"** (see
plans/README.md "Findings considered and rejected"): that was a
file-organization split with no behavior payoff. This plan builds a
seam with three payoffs the rejection didn't have: (1) the wake/emit
contract becomes structural, (2) the queue becomes fully deterministic
(clock injected through its whole interface — the proptest suite's
long-standing want), (3) the two ingest paths (espn's
`enqueue_and_fan_out` vs rss's inline loop) collapse into one that
encodes the Connector rule. The rejection's own un-park trigger —
"deferred until it actually impedes a change" — has fired: 036.

Two known deferred findings get a locality point (not a fix) here:

- **Slot-state emit-after-unlock reordering** (plans/README.md
  dependency notes, twice-deferred, symptom-free): after this plan all
  emit-after-unlock sites are ONE site (`Engine::apply`'s tail). If
  ghost/blank cards ever appear, the fix is a one-place change.
- **Poller spawn-signature bundles** (8 positional args,
  `too_many_arguments`): partially relieved — queue+wake+connectors+
  app_handle collapse into one `Engine` parameter.

## Current state

The mutation sites (locate each with
`rg -n "blocking_lock|lock\(\)\.await" src-tauri/src`):

| Site | Where | Lock style |
|---|---|---|
| `enqueue_and_emit` | `http.rs:69` (the model; tested at `http.rs:737`) | async |
| `send_test_notification` | `settings.rs:691` → calls `enqueue_and_emit` | async |
| `enqueue_and_fan_out` | `poller.rs:~524` (espn; offers to Connectors) | async |
| rss inline enqueue loop | `rss_poller.rs:~508` (deliberately no offer) | async |
| `toggle_pause` | `lib.rs:~584` (also sets tray label; resume+tick) | blocking |
| `toggle_manual_expand` | `lib.rs:~725` | blocking |
| `dismiss_current` | `lib.rs:~746` | blocking |
| `skip_current` | `lib.rs:~765` | blocking |
| `spawn_heartbeat` | `lib.rs:~684` (in its post-036 shape — the dependency gate above guarantees you never see the pre-036 racy loop) | async |

Read-only sites (no propagation today, correctly):
`open_current_story` (`lib.rs:~794`, reads `current_link`), the
`/notify` handler's `is_paused`/`total_waiting` response reads
(`http.rs:~200`).

One more site that is neither a mutation nor a pure read — easy to
miss, and Step 6's greps WILL flag it if left unmigrated: the
**webview-reload re-emit** in `on_page_load` (`lib.rs:~394-403`,
find with `rg -n "current_slot_state" src-tauri/src/lib.rs`). On page
load it does `blocking_lock()` → `current_slot_state()` → an
UNCONDITIONAL `emit_slot_state` (plus the eval-splice global seed).
The dedup (`slot_state_if_changed`) is deliberately bypassed: a
freshly reloaded webview has no state, so it must be re-sent even if
unchanged. This site migrates to `Engine::emit_current_blocking`
(see Design) — a read+emit with NO wake (nothing mutated).

The clock split (`rg -n "Instant::now" src-tauri/src/queue.rs`):

```rust
pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError> {
    self.enqueue_with_options(event, Instant::now(), false)   // queue.rs:87
}
pub fn enqueue_test(&mut self, event: Event) -> Result<(), QueueError> {
    self.enqueue_with_options(event, Instant::now(), true)    // queue.rs:95
}
fn enqueue_at(&mut self, event: Event, now: Instant) -> ...   // queue.rs:101, private, test-only
```

`tick`/`dismiss_visible`/`skip_visible` already take `now` from the
caller. `emit_slot_state` lives at `event.rs:169`.

**A latent Connector-rule violation this plan fixes**: CONTEXT.md's
Connector entry says Connectors receive every accepted Event *except
News*. But `send_test_notification` builds test Events with real
Origins — including `SourceKind::News` (`settings.rs:543`,
`build_test_event`) — and routes them through `enqueue_and_emit`, which
offers to ALL Connectors. Today the News test button leaks a News Event
to telegram. See the deliberate behavior change below.

## Design (operator-locked — do not re-litigate)

New module `src-tauri/src/engine.rs`:

```rust
/// The Engine (CONTEXT.md): the one module through which every Slot
/// mutation flows. Owns the queue, the heartbeat wake, the app handle,
/// and the Connectors — all private. Nothing outside this module can
/// lock the queue or touch the wake, so the mutate→wake→emit protocol
/// is structural, not conventional.
pub struct Engine<R: tauri::Runtime = tauri::Wry> {
    queue: Arc<Mutex<SingleSlotQueue>>,        // PRIVATE
    wake: Arc<tokio::sync::Notify>,            // PRIVATE — never escapes
    app: tauri::AppHandle<R>,
    connectors: Arc<Vec<ConnectorHandle>>,
}
// Clone impl by hand (Arc clones + AppHandle clone), like AppState's.

impl<R: tauri::Runtime> Engine<R> {
    /// Propagating mutation — async callers (http, settings, pollers).
    /// Reads Instant::now() ONCE (the system's only wall-clock read for
    /// queue time), locks, runs `f`, captures slot_state_if_changed,
    /// unlocks, notify_waiters, emits. Emit stays after unlock — the
    /// known deferred reordering (plans/README.md); now one site.
    pub async fn apply<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T;

    /// Propagating mutation — main-thread callers (tray, hotkeys).
    /// Same body with blocking_lock; keeps toggle_pause's
    /// debug_assert!(Handle::try_current().is_err()) guard.
    pub fn apply_blocking<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T;

    /// Non-propagating reads — no wake, no emit, & not &mut.
    pub async fn read<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T;
    pub fn read_blocking<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T;

    /// The one ingest path (replaces enqueue_and_emit AND rss's inline
    /// loop AND espn's enqueue_and_fan_out): enqueue via apply, then
    /// Connector fan-out. The CONTEXT.md Connector rule is encoded
    /// HERE: an Event whose origin is SourceKind::News is never
    /// offered. QueueFull returns early — no wake, no offer (today's
    /// enqueue_and_emit semantics).
    pub async fn accept(&self, event: Event, bypass_pause_when_slot_empty: bool)
        -> Result<(), QueueError>;

    /// The rotation loop (formerly lib.rs spawn_heartbeat), moved
    /// inside the Engine so `wake` never escapes. MUST preserve plan
    /// 036's shape: the pinned Notified is enable()d while holding the
    /// queue lock, before next_deadline() is read.
    pub fn spawn_rotation(&self);

    /// Webview-reload re-emit (the on_page_load site): reads
    /// current_slot_state and emits it UNCONDITIONALLY (dedup
    /// deliberately bypassed — a fresh webview has no state), returns
    /// it for the eval-splice global seed. No wake: nothing mutated.
    pub fn emit_current_blocking(&self) -> SlotState;
}
```

**`Engine::new` takes the queue BY VALUE** —
`Engine::new(queue: SingleSlotQueue, app: tauri::AppHandle<R>,
connectors: Arc<Vec<ConnectorHandle>>)` — wrapping it in the Arc and
creating the wake internally. By construction, no code outside
`engine.rs` can ever hold the queue Arc or the wake: there is nothing
to alias. (`run()` still builds the bare `SingleSlotQueue` with
`with_rotation_order` + the `start_paused` pause, as today at
`lib.rs:~104-112`, then moves it into `setup` for `Engine::new` —
see Step 5's wiring list.)

**Why the rotation loop moves inside (a filing-time correction to the
grilling session's sketch)**: post-036 the heartbeat arms its waiter
under the queue lock. If the heartbeat ran through `apply`, apply's own
`notify_waiters()` would fire after the closure armed the waiter — the
loop would wake itself every iteration and spin. The rotation loop is
the *consumer* of the wake, not a producer, so it gets its own private
loop inside the Engine (lock → tick → emit-if-changed → arm → read
deadline → unlock → sleep/await; NO notify). This also makes `wake`
fully private — the strongest form of the seam. The operator has been
shown this correction.

**Queue goes clock-agnostic**: `enqueue(&mut self, event, now)` and
`enqueue_test(&mut self, event, now)` take `now` like `tick` already
does; private `enqueue_at` is deleted (its signature IS the new public
one). Zero `Instant::now()` calls remain in queue.rs production code.

**Deliberate behavior change (the only one)**: a News-origin test
notification (Settings → News → "Send test") is no longer offered to
telegram — it becomes overlay-only, matching CONTEXT.md's Connector
rule. Name this in the commit message. Every other path keeps today's
observable behavior exactly, with one deemed-unobservable ordering
note: today `enqueue_and_emit` runs wake → connector offers → emit
(`http.rs:87-93`); `accept` (enqueue via `apply`, then fan-out) runs
wake → emit → offers. The two sinks are independent (webview event vs
mpsc channel), so the swap has no observable effect — do not contort
`accept` to preserve the old interleaving.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass (recount against §0 at HEAD — do not trust stale totals) |
| Property suite | `cargo test --locked queue_invariants_hold_under_any_op_script` | 1 passed, run 5× |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend (unchanged) | `npx vitest run && npx tsc --noEmit` | all pass |
| Full local gate | `just test-all` | all green |

(If `cargo: command not found`, prefix `PATH="$HOME/.cargo/bin:$PATH"`.)

## Scope

**In scope**:
- `src-tauri/src/engine.rs` (new) + registration in `lib.rs`
- `src-tauri/src/queue.rs` — enqueue/enqueue_test signatures + the
  mechanical test/proptest-harness updates that follow (NO semantic
  queue changes)
- `src-tauri/src/http.rs` — AppState holds an `Engine<R>`; delete
  `enqueue_and_emit`; migrate handler + tests
- `src-tauri/src/settings.rs` — `send_test_notification` → `accept`
- `src-tauri/src/poller.rs`, `src-tauri/src/rss_poller.rs` — spawn fns
  take `Engine<R>`; delete `enqueue_and_fan_out`; rss inline loop →
  `accept`
- `src-tauri/src/lib.rs` — `run()` builds ONE Engine; handlers migrate;
  `spawn_heartbeat` moves into `engine.rs::spawn_rotation`
- `docs/TESTING_STRATEGY.md` §0 counts (+ any §9.1 line that names
  `enqueue` signatures)
- `CONTEXT.md` — one edit only: the Engine entry's "plan 037 —
  planned" parenthetical becomes a landed note
- `plans/README.md` (status row)

**Out of scope**:
- Fixing the emit-after-unlock ordering (stays deferred; add the
  pointer comment in `apply`, nothing more)
- Any queue-semantics change; any SlotState/wire change
- The poller loop-skeleton dedup (a separate candidate; 025 owns the
  fetch-path shape)
- Frontend code entirely

## Git workflow

- Branch: `advisor/037-engine-propagation` (or per operator dispatch).
- Commit sequence (each independently green):
  1. `queue: take now at the interface — clock-agnostic enqueue`
  2. `engine: apply/read/accept + private rotation loop`
  3. `engine: migrate /notify + test notifications to accept`
  4. `engine: migrate pollers; delete enqueue_and_fan_out`
  5. `engine: migrate tray/hotkey handlers; retire lib spawn_heartbeat`
  6. `docs: §0 counts + plans index for 037`
- Do NOT push.

## Steps

### Step 1: clock-agnostic queue

Change `enqueue`/`enqueue_test` to take `now: Instant`; delete
`enqueue_at`; update every queue.rs test call site mechanically
(the tests already hold a synthetic `t0`). The proptest harness's
enqueue op gains the harness clock it already tracks.

**Verify**: `cargo test --locked` (from `src-tauri/`) → all pass;
property suite 5× green; `rg -n "Instant::now" src-tauri/src/queue.rs`
→ matches only inside `#[cfg(test)]` (as a base instant), none in
production code.

### Step 2: engine.rs

Create the module per the Design block: `apply`/`apply_blocking`/
`read`/`read_blocking`/`accept`/`spawn_rotation`/
`emit_current_blocking`, plus
`Engine::new(queue: SingleSlotQueue, app, connectors)` — by-value
queue, wake created internally (nothing else can hold either). Port
`spawn_heartbeat`'s post-036 body into `spawn_rotation` VERBATIM in
shape — the `Notified::enable` under the lock, the 10 ms grace, the
`select!` on the pinned future. (The dependency gate at the top of
this plan guarantees the post-036 shape is what you find in lib.rs.)

Engine tests (in `engine.rs`'s `#[cfg(test)]`, `mock_app` pattern from
lib.rs; private-field access makes the wake assertable):
1. `apply_wakes_and_emits` — port of `enqueue_and_emit_wakes_the_heartbeat`
   (http.rs:737) generalized to `apply` (register a waiter on the
   private wake, apply a mutation, assert the waiter fires).
2. `read_never_wakes` — register a waiter, `read`, assert timeout.
3. `accept_offers_manual_but_never_news` — fake-connector channel
   pattern from `http.rs::test_connector`: a Manual event lands on the
   rx, a News event does not.
4. `accept_queue_full_propagates_nothing` — fill a tier, accept →
   `Err(QueueFull)`, no wake, no offer.
5. `rotation_loop_parked_idle_wakes_on_accept` — port/adopt the
   idle-park regression test that plan 036 added to lib.rs (named
   `heartbeat_parked_idle_wakes_on_enqueue_and_rotates_out` in 036's
   Step 2; it exists only once 036 has landed — the dependency gate
   guarantees that) against `spawn_rotation` + `accept`.
6. `emit_current_returns_and_emits` — seed a visible item, call
   `emit_current_blocking` twice, assert both calls emit (dedup
   bypassed) and return the same `Showing` state.

**Verify**: `cargo test --locked engine::` → all pass, 3 consecutive
runs (the rotation test uses real timers; see 036's flake gate).

### Step 3: migrate ingest (http + settings)

`AppState` drops `queue`/`wake`/`connectors` (and `app_handle` if no
remaining use — `rg` first) in favor of `engine: Engine<R>`; scalar
config fields stay. `notify_handler` calls `engine.accept(event,
false).await`; the 200-vs-202 read becomes
`engine.read(|q| (q.is_paused(), q.total_waiting())).await`. Delete
`enqueue_and_emit` (its doc-comment guarantee now lives on the Engine
itself — move the prose).

`send_test_notification` (`settings.rs:~672`) today receives the
queue/wake/connectors via **tauri managed state** — three
`tauri::State<'_, Arc<…>>` parameters. Its new signature drops all
three (and the now-unused `app: tauri::AppHandle` — `rg` its remaining
uses in the fn first) in favor of ONE managed-state parameter:

```rust
#[tauri::command]
pub async fn send_test_notification(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
    engine: tauri::State<'_, Engine>,   // Engine<R = Wry> default makes this concrete
    source: SourceKind,
) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let config = state.inner().lock().unwrap().clone();
    let event = build_test_event(&config, source);
    engine.accept(event, true).await.map_err(|e| e.to_string())
}
```

(The `app.manage(engine.clone())` that backs this parameter is wired
in Step 5 — the two steps meet at the same managed type.)

Migrate http.rs tests: `test_state`/`test_state_with_connectors`
(`http.rs:~241-249`) build an Engine from a real `SingleSlotQueue` (by
value now) + `MockRuntime` handle. Heads-up on the ripple: five test
assertions around `http.rs:~534-605` read
`state.queue.lock().await.current_priority()` directly — they become
`state.engine.read(|q| q.current_priority()).await` (mechanical). The
wake-regression test has already moved to engine.rs (Step 2.1) —
remove the http copy.

**Verify**: `cargo test --locked http::` → all pass;
`rg -n "enqueue_and_emit" src-tauri/` → zero matches.

### Step 4: migrate pollers

`spawn_espn_poller`/`spawn_rss_poller` take `engine: Engine<R>` in
place of queue+wake+connectors+app_handle (coordinate with 025's landed
signatures). Each accepted event goes through `engine.accept(ev,
false)`; delete `enqueue_and_fan_out` and rss's inline
lock/enqueue/notify/emit block. The espn/rss offer difference is now
automatic (Origin gate) — no flags at the call sites.

**Verify**: `cargo test --locked poller::` → all pass, then
`cargo test --locked rss_poller::` → all pass (two invocations —
cargo accepts only one positional test filter).

### Step 5: migrate lib.rs

**Wiring — follow this exactly; the Engine can only be constructed
where an `AppHandle` exists, which is inside `setup`:**

1. `run()` keeps building the bare `SingleSlotQueue` as today
   (`with_rotation_order` + `start_paused` pause, `lib.rs:~104-112`)
   but does NOT wrap it in an Arc. Delete the `Arc::new(Mutex::new(…))`
   (`lib.rs:~113`), the `wake` creation (`lib.rs:~117`), and the whole
   named-alias block (`setup_queue`/`rss_queue`/`page_load_queue`/
   `hotkey_queue`/`*_wake`, `lib.rs:~167-178`). The bare queue moves
   into the `setup` closure.
2. At the TOP of `setup`:
   `let engine = Engine::new(queue, app.handle().clone(), connectors.clone());`
   After this line, `run()` holds no queue binding at all — by-value
   construction makes a retained alias a compile error, not a
   convention.
3. Replace the three managed-state lines
   `app.manage(queue.clone()); app.manage(wake.clone());
   app.manage(connectors.clone());` (`lib.rs:~201-206`) with ONE:
   `app.manage(engine.clone());` — this is what Step 3's
   `tauri::State<'_, Engine>` parameter resolves to. Do NOT leave the
   old manage calls in place: a managed queue Arc is reachable from
   every command and silently defeats the seam while all of Step 6's
   greps still pass.
4. Consumers, all inside `setup` or later: pollers get `engine.clone()`
   (Step 4), `build_tray`/hotkey registration take `&Engine<R>` /
   clones, `engine.spawn_rotation()` replaces the `spawn_heartbeat`
   call, and the `on_page_load` closure captures NOTHING queue-related
   — it retrieves the engine at page-load time via
   `webview.app_handle().state::<Engine>()` (the closure is built
   before `setup` runs, so it cannot capture the Engine directly; a
   second `Engine::new` there would create a second wake that no
   rotation loop waits on — the exact stall class 015/036 fixed —
   which is why retrieval-via-managed-state is the required shape).
   The page-load re-emit + eval-splice seed use
   `engine.emit_current_blocking()` (Design block).

Handlers become:

```rust
fn skip_current<R: tauri::Runtime>(engine: &Engine<R>) {
    engine.apply_blocking(|q, now| q.skip_visible(now));
}
```

`toggle_pause` keeps its tray-label logic at the caller, driven by the
closure's return value (the Engine never touches menus):

```rust
let now_paused = engine.apply_blocking(|q, now| {
    if q.is_paused() { q.resume(); q.tick(now); false }
    else { q.pause(); true }
});
let _ = pause_item.set_text(if now_paused { "Resume" } else { "Pause" });
```

`open_current_story` uses `read_blocking`. Delete `spawn_heartbeat`
(now `engine.spawn_rotation()`). Migrate the lib.rs handler tests to
construct an Engine.

**Verify**: `cargo test --locked` (full) → all pass;
`cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check`
→ exit 0.

### Step 6: the seam sweep (the point of the plan)

Prove the protocol is now structural:

- `rg -n "blocking_lock|\.lock\(\)\.await" src-tauri/src --glob '!engine.rs'`
  → zero matches on the queue (other mutexes, e.g. the managed
  `StdMutex<Config>`, are fine — eyeball the hits).
- `rg -n "notify_waiters|Notify::new" src-tauri/src --glob '!engine.rs'`
  → zero matches.
- `rg -n "emit_slot_state" src-tauri/src --glob '!engine.rs' --glob '!event.rs'`
  → zero matches.

**Verify**: all three greps clean; full suite + gates green; run the
property suite 5×.

### Step 7: reconcile docs

`docs/TESTING_STRATEGY.md` §0: recount per module (queue count
unchanged in intent; http loses the moved wake test; engine module is
new; lib loses the heartbeat test if it moved). Fix any §9.1 sentence
that names the old `enqueue(event)` signature. `plans/README.md`:
status row.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` totals
match §0 exactly; `npx vitest run` count unchanged.

## Test plan

- New: the six engine.rs tests (Step 2) — the protocol itself finally
  has a direct test surface.
- Moved, not lost: the wake-regression test (http→engine), the two
  heartbeat tests (lib→engine).
- Everything else stays green UNCHANGED in behavior: the 53 queue
  example tests + proptest suite (signature-only edits), 30-odd http
  tests, poller/rss/settings/lib suites.
- The deliberate News-test-notification change gets an explicit
  assertion (Step 2 test 3 covers the rule; if settings.rs has a test
  pinning the old offer-News behavior, update it and say so in the
  commit).

## Done criteria

- [ ] `cd src-tauri && cargo test --locked` exits 0; totals match §0
- [ ] Property suite passes 5× consecutively; no `proptest-regressions`
      file appears
- [ ] clippy `--locked -D warnings` + `cargo fmt --check` exit 0
- [ ] `npx vitest run` + `npx tsc --noEmit` green, counts unchanged
- [ ] Step 6's three seam greps return zero out-of-engine matches
- [ ] `rg -n "enqueue_and_emit|enqueue_and_fan_out|fn spawn_heartbeat" src-tauri/` → zero
- [ ] `rg -n "Instant::now" src-tauri/src/queue.rs` → test-module only
- [ ] CONTEXT.md Engine entry's "plan 037 — planned" parenthetical
      updated to note it landed
- [ ] `plans/README.md` status row updated

## STOP conditions

- The dependency gate fails (036 or 025 not DONE, or no
  `enable()`-under-lock in `spawn_heartbeat`) — report "dependencies
  not landed"; do not execute against pre-dependency code.
- A property-test or example-test failure you cannot attribute to a
  mechanical migration mistake — the refactor may have changed queue
  semantics; minimize, commit `#[ignore]`d, report.
- Preserving 036's enable-under-lock shape inside `spawn_rotation`
  proves impossible without changing wake semantics — report; do not
  weaken the race fix to fit the seam.
- Any migration step seems to need the queue Arc or the wake exposed
  outside `engine.rs` — that is the design being violated; stop and
  report what needs it.
- 033/035 (or anything else) landed changes to queue/http surfaces the
  excerpts don't match — reconcile the plan text first (drift check).
- The rotation-loop test is flaky (fails any of 3 consecutive runs).

## Maintenance notes

- **Every future queue-mutation site must go through
  `Engine::apply`/`accept`** — reviewers should treat an out-of-engine
  `lock` on the queue as a defect. The Step 6 greps are re-runnable as
  a review check.
- The emit-after-unlock ordering gap now lives at exactly one site
  (`apply`'s tail) — if ghost/blank cards are ever observed around
  rotation boundaries, fix it there (hold the lock through emit, or
  sequence-number the states) and delete the plans/README deferred
  note.
- 024's predictor duplication note still stands; this plan does not
  change ranking.
- A future second Connector still edits config/secrets/construction
  (see the architecture review's candidate 5) — `accept`'s fan-out loop
  itself needs no change.
