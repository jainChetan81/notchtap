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
> dependency)**: this plan's excerpts describe the codebase as it
> stands after plans 025, 033, 034, 035, 036 all landed (they have —
> `plans/README.md` shows all five DONE as of `b8c554f`, 2026-07-19).
> Verify both:
> 1. `plans/README.md` status rows for **025**, **033**, **034**,
>    **035**, **036** all read DONE.
> 2. `rg -n "enable\(\)" src-tauri/src/lib.rs` hits inside
>    `spawn_heartbeat`'s lock block (036's waiter-under-lock shape —
>    `enable()` called on the pinned `notified` before `q.next_deadline()`
>    is read, itself inside the block that holds the queue lock).
> If either check fails, STOP and report "dependencies not landed" —
> do NOT execute against pre-dependency code.
>
> **Drift check (run second)**:
> `git diff --stat b8c554f..HEAD -- src-tauri/src/lib.rs src-tauri/src/http.rs src-tauri/src/queue.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src-tauri/src/event.rs src-tauri/src/status.rs CONTEXT.md`
> `b8c554f` is this retargeting pass's baseline — every excerpt and line
> number below was re-verified against the live code at that commit. If
> this returns anything, one of the eight files changed after the
> retarget: open each changed file, re-diff its excerpt against this
> plan's "Current state"/Design/Steps text, and update the plan before
> proceeding. This is expected to be empty on a same-day dispatch, but
> not on a delayed one — treat a nonempty diff as a required
> re-reconciliation, not a blocker to work around. Line numbers
> throughout this plan are indicative; locate every target with the
> `rg` command given alongside it. Run `git status` first — if any
> in-scope file is dirty in the working tree, STOP and report rather
> than layering onto someone's in-flight edit.
>
> **Filing note (operator)**: this retargeting pass (2026-07-19)
> rewrote this plan file in place — no other file changed. Commit the
> rewrite before dispatching an executor, so the dirty-tree gate above
> doesn't fire on the plan file itself sitting uncommitted next to a
> clean `src-tauri/`.

## Status

**DONE 2026-07-19** — executed via `/improve execute` on branch
`exec/037-engine-v2` (commits `942c4d2`..`1e11337`, based on master
`9b5bd62`), reviewed APPROVE: cargo 295 + 3 doc-tests (matches the
recomputed TESTING_STRATEGY §0 row), property suite 5× green,
clippy/fmt `--locked` exit 0, vitest 107 + tsc, all seam greps clean.
Not yet merged to master — merge is the operator's call. The
retargeting history below is preserved for the record.

**RETARGETED 2026-07-19** — the `/improve review-plan` pass that was
promised when this plan was filed BLOCKED (see prior status text,
preserved below for history). All five dependencies (025, 033, 034,
035, 036) are now DONE on master (`b8c554f`). This pass re-verified
every excerpt in the plan against live code at that commit and found
real drift beyond line numbers — most importantly, **plan 034 (idle
source-status rail) landed after this plan's design was authored and
fused a second concern into the exact code this plan restructures**:
`spawn_heartbeat` is no longer just a rotation loop — it is also the
sole `StatusState` emitter, reading a `live: Arc<StdMutex<Option<LiveMatchSummary>>>`
handle the espn poller writes, and that poller write triggers its own
`wake.notify_waiters()` call **outside any queue mutation**. Both facts
are new since this plan's Design block was written and change what
"port `spawn_heartbeat` verbatim" and "wake never escapes engine.rs"
mean in practice. The fixes are folded into the Design and Steps below
as explicit, marked amendments (same treatment the original filing gave
the "rotation loop moves inside" correction) — the five operator-locked
decisions themselves (closure-based `apply`, private queue+wake,
`accept` with the News gate, Engine-owned clock, rotation loop inside
the Engine) are unchanged and still govern. This plan is now believed
executable as written; re-run `git status`/the drift check above before
dispatch regardless, since more time has passed since `b8c554f`.

> Prior status (2026-07-19, now resolved): **BLOCKED — do not execute
> this plan as written.** Plans 032–034 landed on master while an
> executor was mid-run from `4ad4be5`, rewriting queue.rs/lib.rs/http.rs
> underneath it: 033 reversed the High-only auto-expand (the queue
> gained batch counters), the heartbeat now carries the live-match
> handle + StatusState emission (plan 034 — `spawn_rotation`'s port
> target changed shape), and 035's `/notify` contract changes were
> still inbound. A partial reference implementation (Steps 1–2 against
> the old base) is preserved on branch `exec/037-engine` — **do not
> reuse it**: Step 1 below now covers more call sites than that branch's
> version did (see the "clock-agnostic queue" amendment), and Step 2's
> Design grew two fields and two methods since that branch was cut.

- **Priority**: P2
- **Effort**: L
- **Risk**: MEDIUM — a wide but mechanical refactor across every caller
  of the queue, now confirmed to also touch the status-state emission
  path plan 034 added. Semantics must NOT change (two deliberate
  exceptions, both documented below); the plan-022 proptest suite and
  the full example suite are the safety net.
- **Depends on**: **036 (hard, DONE)** — this plan rewrites the
  heartbeat loop 036 fixes and must preserve its `Notified::enable`-
  under-lock shape; **025 (hard, DONE)** — 025 reshaped the poller
  fetch paths (`net::build_poll_client`) this plan migrates;
  **033 (DONE)**, **034 (DONE)**, **035 (DONE)** — all three changed
  queue/http/lib production surfaces this plan touches; this
  retargeting pass reconciled all three textually. **024 (soft)** — if
  024 is still IN PROGRESS, coordinate before dispatch (it extends the
  `mod proptest_queue` harness whose enqueue call sites this plan
  mechanically edits).
- **Category**: tech debt / architecture
- **Planned at**: commit `d926977`, 2026-07-18. Design decided in an
  operator-confirmed architecture-review grilling session (interface
  shape, read path, fan-out rule, clock ownership, name, sequencing —
  each an explicit operator selection). CONTEXT.md gained the
  **Engine** term in the same filing.
  **Review-plan pass 1 (2026-07-18)**, against live code at `d926977`+
  filing: added the dependency gate, the missed `on_page_load` re-emit
  site and its `emit_current_blocking` migration, the full
  managed-state/setup wiring list, the new `send_test_notification`
  signature, the CONTEXT.md scope listing, and the split poller verify
  commands.
  **Review-plan pass 2 — this one (2026-07-19)**, retargeted at
  `b8c554f` after 025/033/034/035/036 all landed: re-verified every
  excerpt against live code (not just line-number drift); found and
  folded in two structural amendments — (1) `spawn_heartbeat` (now
  `spawn_rotation`) must also own the plan-034 live-match handle,
  `espn_enabled`/`rss_enabled` flags, and the `StatusState`
  snapshot/emit/dedup logic, since 034 fused that into the exact loop
  this plan relocates; (2) the espn poller's live-match-summary-changed
  wake (poller.rs, added by 034) is a wake call site **outside any
  queue mutation** that this plan's original Design didn't anticipate
  and Step 6's seam sweep would otherwise flag — resolved with a new
  narrow `Engine::update_live_match` method. Also corrected: the
  `enqueue`/`enqueue_test` signature change in Step 1 breaks
  compilation at every non-queue.rs call site until those sites also
  gain a `now` argument (the original Step 1 text implied queue.rs was
  the only file it touched — it isn't, by necessity); the queue's own
  `batch_total`/`batch_done` counters (plan 033) and the current
  `invariant 8` proptest check ("every promotion starts expanded") were
  checked and found NOT to need any change from this plan — they were
  flagged as a risk in the prior BLOCKED status but the live code
  already reflects 033's universal-expand behavior correctly; and the
  `send_test_notification`/`build_test_event` line numbers, the five
  `http.rs` `current_priority()` test assertions (now at lines 597,
  615, 634, 651, 668 — plus three more `current_slot_state()` reads at
  1045, 1073, 1094 the original text missed), and every other cited
  line number were re-pointed at live code.

## Why this matters

Every queue mutation in the codebase must follow the same protocol:
lock → mutate → `slot_state_if_changed()` → unlock →
`wake.notify_waiters()` → `emit_slot_state`. Miss the wake and the
deadline heartbeat (plan 015) oversleeps or wedges; miss the emit and
the overlay shows a stale Slot. Today that protocol is **enforced by
convention at ~9 hand-written sites** (see "Current state" below);
exactly one of them — `http::enqueue_and_emit` — encodes it
structurally (its own doc comment: "a mutation without a wake is
structurally impossible to express here") and is regression-tested.

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
- **New evidence found during this retargeting pass**: plan 034 added
  a TENTH wake site (poller.rs, the live-match-summary-changed wake)
  that touches neither the queue nor `enqueue_and_emit`'s shared path —
  it was free to reach directly into the raw `wake` handle because
  nothing stopped it. That is exactly the class of site this plan is
  meant to make impossible; it is folded into the Engine below via
  `update_live_match` rather than left as an 11th hand-written
  exception.

**This is not the rejected "lib.rs multi-responsibility split"** (see
plans/README.md "Findings considered and rejected"): that was a
file-organization split with no behavior payoff. This plan builds a
seam with payoffs the rejection didn't have: (1) the wake/emit contract
becomes structural — now covering the status-state channel too, not
just slot-state, (2) the queue becomes fully deterministic (clock
injected through its whole interface — the proptest suite's
long-standing want), (3) the two ingest paths (espn's
`enqueue_and_fan_out` vs rss's inline loop) collapse into one that
encodes the Connector rule. The rejection's own un-park trigger —
"deferred until it actually impedes a change" — has fired twice now:
036, and the live-match wake finding above.

Two known deferred findings get a locality point (not a fix) here:

- **Slot-state emit-after-unlock reordering** (plans/README.md
  dependency notes, twice-deferred, symptom-free): after this plan all
  emit-after-unlock sites are ONE site (`Engine::apply`'s tail). If
  ghost/blank cards ever appear, the fix is a one-place change.
- **Poller spawn-signature bundles** (`#[allow(clippy::too_many_arguments)]`
  on both `spawn_espn_poller` — 9 args — and `spawn_rss_poller` — 8
  args): **fully relieved, not just partially** now that `live` also
  collapses into the Engine (see the Design amendment below) —
  `spawn_espn_poller` drops to `engine, leagues, poll_secs, ttl_secs,
  priority` (5 args) and `spawn_rss_poller` to `engine, feeds,
  poll_secs, ttl_secs, max_per_poll, priority` (6 args), both under
  clippy's default 7-arg threshold. Delete both `#[allow(...)]`
  attributes in Step 4.

## Current state

The mutation sites (locate each with
`rg -n "blocking_lock|lock\(\)\.await" src-tauri/src`):

| Site | Where | Lock style |
|---|---|---|
| `enqueue_and_emit` | `http.rs:69` (the model; wake regression test at `http.rs:800`) | async |
| `send_test_notification` | `settings.rs:674` → calls `enqueue_and_emit` | async |
| `enqueue_and_fan_out` | `poller.rs:557` (espn; offers to Connectors) | async |
| rss inline enqueue loop | `rss_poller.rs:~487-501` (deliberately no offer) | async |
| `toggle_pause` | `lib.rs:623` (also sets tray label; resume+tick) | blocking |
| `toggle_manual_expand` | `lib.rs:793` | blocking |
| `dismiss_current` | `lib.rs:811` | blocking |
| `skip_current` | `lib.rs:830` | blocking |
| `spawn_heartbeat` | `lib.rs:723` (post-036 AND post-034 shape — see the Design amendment; the dependency gate above guarantees you never see the pre-036 racy loop) | async |

**A tenth site the original filing missed** (found during this
retargeting pass): the espn poller's live-match-summary-changed wake,
`poller.rs:~660-677` inside `spawn_espn_poller`'s loop, after
`enqueue_and_fan_out` returns for the pass — it locks `live` (a plain
`StdMutex<Option<LiveMatchSummary>>`, *not* the queue), compares
against the previous summary, and calls `wake.notify_waiters()`
directly if it changed. This is neither a queue mutation (`live` isn't
the queue) nor read-only (it does call `notify_waiters()`) — it doesn't
fit `apply`/`read`/`accept` as designed. See the Design amendment for
`Engine::update_live_match`, which this site migrates to.

Read-only sites (no propagation today, correctly):
`open_current_story` (`lib.rs:859`, reads `current_link`), the
`/notify` handler's `is_paused`/`total_waiting` response reads
(`http.rs:258-261`).

One more site that is neither a mutation nor a pure read — easy to
miss, and Step 6's greps WILL flag it if left unmigrated: the
**webview-reload re-emit** in `on_page_load` (`lib.rs:~416-423`, find
with `rg -n "current_slot_state" src-tauri/src/lib.rs`). On page load
it does `blocking_lock()` → `current_slot_state()` → an UNCONDITIONAL
`emit_slot_state` (plus the eval-splice global seed). The dedup
(`slot_state_if_changed`) is deliberately bypassed: a freshly reloaded
webview has no state, so it must be re-sent even if unchanged. This
site migrates to `Engine::emit_current_blocking` (see Design) — a
read+emit with NO wake (nothing mutated).

**A second, structurally identical re-emit the original filing also
missed** (it postdates `d926977` — plan 034 added it the same day):
immediately after the slot-state block, `on_page_load` does the exact
same dance for `StatusState` (`lib.rs:~425-443`) — lock discipline note
in the code itself: "the live-match handle is read/cloned/dropped
BEFORE the queue lock (nobody holds both at once), same as the
heartbeat." This migrates to a new `Engine::emit_current_status_blocking`
(see Design amendment) — same shape as `emit_current_blocking`, over
`StatusState` instead of `SlotState`, still no wake.

**A third site inside `on_page_load` this plan must also touch**: the
`server_once.call_once` closure (`lib.rs:~481-492`) constructs the
axum `http::AppState` struct literal — `queue`, `wake: queue_wake`,
`connectors`, `app_handle`, plus the scalar config fields. This is
WHERE `AppState`'s `engine: Engine<R>` field (Step 3) actually gets
populated at runtime; it's easy to migrate Step 3's struct *definition*
and Step 5's *managed-state* wiring while missing this one *construction
site*, which lives inside `on_page_load`, not `setup`. It is exactly
why `on_page_load` must retrieve the Engine via
`webview.app_handle().state::<Engine>()` rather than capturing it — see
Step 5.

The clock split (`rg -n "Instant::now" src-tauri/src/queue.rs`):

```rust
pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError> {
    self.enqueue_with_options(event, Instant::now(), false)   // queue.rs:110-112
}
pub fn enqueue_test(&mut self, event: Event) -> Result<(), QueueError> {
    self.enqueue_with_options(event, Instant::now(), true)    // queue.rs:118-120
}
#[cfg(test)]
fn enqueue_at(&mut self, event: Event, now: Instant) -> ...   // queue.rs:122-126, private, test-only
```

`tick`/`dismiss_visible`/`skip_visible` already take `now` from the
caller. `emit_slot_state` lives at `event.rs:205`.

**`enqueue`/`enqueue_test` are called outside queue.rs too** — this
matters for Step 1's sequencing (see the amendment there):
`http.rs:81,83` (inside `enqueue_and_emit`), `http.rs:566,726` (two
tests building a queue directly), `poller.rs:565` (inside
`enqueue_and_fan_out`), `rss_poller.rs:492` (the inline loop), and
eight `lib.rs` handler-test call sites (`rg -n "\.enqueue\(" src-tauri/src/lib.rs`
→ lines 930, 961, 992, 995, 1028, 1031, 1065, 1163). None of these pass
a `now` today (they rely on `enqueue`'s internal `Instant::now()`), so
changing the signature breaks all of them at once.

**A latent Connector-rule violation this plan fixes**: CONTEXT.md's
Connector entry says Connectors receive every accepted Event *except*
News. But `send_test_notification` builds test Events with real
Origins — including `SourceKind::News` (`settings.rs:505`,
`build_test_event`) — and routes them through `enqueue_and_emit`, which
offers to ALL Connectors. Today the News test button leaks a News Event
to telegram. See the deliberate behavior change below. (Checked during
this retargeting pass: no existing test pins the old offer-News
behavior — `rg -n "fn.*test_notification" src-tauri/src/settings.rs`
finds only the command itself, no unit test exercising its connector
fan-out — so Step 2's new `accept_offers_manual_but_never_news` test is
the first coverage of this rule, not a replacement for an existing one.)

## Design (operator-locked — do not re-litigate; two amendments below
## extend it to cover code that landed after the lock, they don't
## reopen it)

New module `src-tauri/src/engine.rs`:

```rust
use crate::status::{LiveMatchSummary, StatusState};

/// The Engine (CONTEXT.md): the one module through which every Slot
/// mutation flows. Owns the queue, the heartbeat wake, the app handle,
/// the Connectors, and (plan 034's territory, folded in by this
/// retargeting pass) the live-match handle and source-enabled flags the
/// rotation loop needs to also be the sole StatusState emitter — all
/// private. Nothing outside this module can lock the queue or touch the
/// wake, so the mutate→wake→emit protocol is structural, not
/// conventional.
pub struct Engine<R: tauri::Runtime = tauri::Wry> {
    queue: Arc<Mutex<SingleSlotQueue>>,             // PRIVATE
    wake: Arc<tokio::sync::Notify>,                  // PRIVATE — never escapes
    app: tauri::AppHandle<R>,
    connectors: Arc<Vec<ConnectorHandle>>,
    live: Arc<StdMutex<Option<LiveMatchSummary>>>,   // NEW (amendment 1)
    espn_enabled: bool,                              // NEW (amendment 1)
    rss_enabled: bool,                                // NEW (amendment 1)
}
// Clone impl by hand (Arc clones + AppHandle clone + bool copies), like
// AppState's.

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
    /// enqueue_and_emit semantics). NOTE (amendment, this retargeting
    /// pass): today's espn/rss poller paths wake unconditionally even
    /// on QueueFull (a harmless no-op wakeup, not a behavior a test
    /// pins — see "Current state"); `accept` does not replicate that,
    /// matching `enqueue_and_emit`'s existing early-return instead. See
    /// "Deliberate behavior changes" below.
    pub async fn accept(&self, event: Event, bypass_pause_when_slot_empty: bool)
        -> Result<(), QueueError>;

    /// NEW (amendment 1). Replaces the espn poller's direct
    /// `live.lock().unwrap()` + `wake.notify_waiters()` dance
    /// (`poller.rs:~660-677` today) — the one wake site this plan's
    /// original Design didn't cover because plan 034 landed after the
    /// design was locked. Locks `live`, compares to the new summary,
    /// stores it, and wakes the rotation loop ONLY if it changed —
    /// same compare-then-store shape `status_state_if_changed`
    /// (status.rs) already uses for the analogous StatusState guard.
    /// Not `apply`/`accept`: it never touches the queue.
    pub fn update_live_match(&self, summary: Option<LiveMatchSummary>);

    /// The rotation loop (formerly lib.rs spawn_heartbeat), moved
    /// inside the Engine so `wake` never escapes. MUST preserve plan
    /// 036's shape (`Notified::enable` under the lock, before
    /// `next_deadline()` is read) AND plan 034's shape (read/clone/drop
    /// `live` BEFORE locking the queue — never both locks at once —
    /// then compute `StatusState::snapshot`, dedup against a
    /// `last_status` local owned by this loop's task, emit on change).
    /// See the Design amendment below for why both survive verbatim.
    pub fn spawn_rotation(&self);

    /// Webview-reload re-emit (the on_page_load slot-state site): reads
    /// current_slot_state and emits it UNCONDITIONALLY (dedup
    /// deliberately bypassed — a fresh webview has no state), returns
    /// it for the eval-splice global seed. No wake: nothing mutated.
    pub fn emit_current_blocking(&self) -> SlotState;

    /// NEW (amendment 1). The on_page_load StatusState twin of
    /// `emit_current_blocking` — same shape, over StatusState instead
    /// of SlotState, using the Engine's own `live`/`espn_enabled`/
    /// `rss_enabled`. Also bypasses the rotation loop's `last_status`
    /// dedup (that guard belongs to the loop's task, not to this
    /// method) for the same reload reason. No wake.
    pub fn emit_current_status_blocking(&self) -> StatusState;
}
```

**`Engine::new` takes the queue BY VALUE, plus the two new scalar
flags** — `Engine::new(queue: SingleSlotQueue, app: tauri::AppHandle<R>,
connectors: Arc<Vec<ConnectorHandle>>, espn_enabled: bool, rss_enabled:
bool)` — wrapping the queue in the Arc and creating BOTH the wake and
the `live` handle internally (amendment 1: `live` used to be
constructed in `run()` at `lib.rs:125` and cloned three ways —
`heartbeat_live`/`espn_live`/`page_load_live` — all three clones
disappear along with `wake`'s, for the same "nothing to alias" reason).
By construction, no code outside `engine.rs` can ever hold the queue
Arc, the wake, or the live-match handle: there is nothing to alias.
(`run()` still builds the bare `SingleSlotQueue` with
`with_rotation_order` + the `start_paused` pause, as today at
`lib.rs:110-116`, then moves it into `setup` for `Engine::new` — see
Step 5's wiring list.)

**Why the rotation loop moves inside (the original filing-time
correction, still valid)**: post-036 the heartbeat arms its waiter
under the queue lock. If the heartbeat ran through `apply`, apply's own
`notify_waiters()` would fire after the closure armed the waiter — the
loop would wake itself every iteration and spin. The rotation loop is
the *consumer* of the wake, not a producer, so it gets its own private
loop inside the Engine (lock → tick → emit-if-changed → arm → read
deadline → unlock → sleep/await; NO notify from this path). This also
makes `wake` fully private — the strongest form of the seam.

**Amendment 1 — why the rotation loop must ALSO own StatusState now
(new this retargeting pass, 2026-07-19)**: `spawn_heartbeat`'s live
shape at `lib.rs:723-783` is not the simple slot-state-only loop the
original Design assumed — plan 034 made the same loop the SOLE
StatusState emitter, because "every mutation already reaches it" (the
plan-015 shared Notify is exactly the signal StatusState recomputation
should ride too). Concretely, each loop pass now also: reads/clones/
drops `live` BEFORE locking the queue (lock-discipline comment in the
code: "nobody holds both at the same time"), computes
`StatusState::snapshot(&q, live_summary, espn_enabled, rss_enabled)`
under the SAME queue lock as the slot-state tick, and emits it through
a `last_status` dedup local scoped to the loop's own spawned task (not
a struct field — status.rs's own comment: "the heartbeat is the sole
emitter, so there is exactly one guard and no second writer can desync
it"). Since `spawn_rotation` is a verbatim port of this loop, it must
carry all of that — which is why `Engine` gains the `live`/
`espn_enabled`/`rss_enabled` fields and `Engine::new` gains the two
bool parameters. This is additive to the locked design (the rotation
loop was already going to move inside `engine.rs`; it now carries
slightly more state when it does), not a re-litigation of it.

**Amendment 2 — the espn poller's live-match wake (new this
retargeting pass)**: with `live` and `wake` both now Engine-private,
the espn poller can no longer do what it does today
(`poller.rs:~660-677`: lock `live` directly, compare, store, and call
`wake.notify_waiters()` itself). `Engine::update_live_match` (Design,
above) replaces that block one-for-one — same compare-then-store-then-
maybe-wake shape, just behind a method instead of three raw handles.

**Queue goes clock-agnostic**: `enqueue(&mut self, event, now)` and
`enqueue_test(&mut self, event, now)` take `now` like `tick` already
does; private `enqueue_at` is deleted (its signature IS the new public
one). Zero `Instant::now()` calls remain in queue.rs production code.
Every non-queue.rs call site of `enqueue`/`enqueue_test` (listed in
"Current state" above) needs a `now` argument added at the SAME time
this signature changes, or the crate stops compiling — see Step 1's
amendment for exactly which sites and what to pass them.

**Deliberate behavior changes (two, both named in the commit
message)**:
1. A News-origin test notification (Settings → News → "Send test") is
   no longer offered to telegram — it becomes overlay-only, matching
   CONTEXT.md's Connector rule. Every other path keeps today's
   observable behavior exactly, with one deemed-unobservable ordering
   note: today `enqueue_and_emit` runs wake → connector offers → emit
   (`http.rs:87-93`); `accept` (enqueue via `apply`, then fan-out) runs
   wake → emit → offers. The two sinks are independent (webview event
   vs mpsc channel), so the swap has no observable effect — do not
   contort `accept` to preserve the old interleaving.
2. **(New, found this retargeting pass)** The espn and rss pollers
   currently call `wake.notify_waiters()` unconditionally per accepted-
   or-rejected event, including on `QueueFull` (`poller.rs:652`,
   `rss_poller.rs:499` — both outside any `if let Ok(...)` guard).
   `accept`'s early-return-on-`QueueFull` (matching `enqueue_and_emit`)
   drops this redundant wake for the poller paths. No test pins the old
   behavior (`rg -n "QueueFull" src-tauri/src/poller.rs
   src-tauri/src/rss_poller.rs` → zero matches), and the effect is
   unobservable: a wake with no resulting slot-state or status-state
   change is a no-op cycle for the rotation loop (it recomputes,
   confirms nothing changed via the dedup guards, and re-arms) — it
   never reaches the overlay. Name this in the commit message for
   Step 4 alongside the News-gate change.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass — baseline at `b8c554f` is 289 + 3 doc-tests; recount against `docs/TESTING_STRATEGY.md` §0 at your actual HEAD, don't trust this number if time has passed |
| Property suite | `cargo test --locked queue_invariants_hold_under_any_op_script` | 1 passed, run 5× |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend (unchanged) | `npx vitest run && npx tsc --noEmit` | all pass — baseline at `b8c554f` is 105 vitest tests |
| Full local gate | `just test-all` | all green |

(If `cargo: command not found`, prefix `PATH="$HOME/.cargo/bin:$PATH"`.)

## Scope

**In scope**:
- `src-tauri/src/engine.rs` (new) + registration in `lib.rs`
- `src-tauri/src/queue.rs` — enqueue/enqueue_test signatures + the
  mechanical test/proptest-harness updates that follow (NO semantic
  queue changes)
- `src-tauri/src/http.rs` — AppState holds an `Engine<R>`; delete
  `enqueue_and_emit`; migrate handler + tests. Step 1 also makes a
  transitional one-line edit here (see Step 1) that Step 3 then
  supersedes — that's expected, not scope creep.
- `src-tauri/src/settings.rs` — `send_test_notification` → `accept`
- `src-tauri/src/poller.rs`, `src-tauri/src/rss_poller.rs` — spawn fns
  take `Engine<R>`; delete `enqueue_and_fan_out`; rss inline loop →
  `accept`; the espn poller's live-match write → `update_live_match`.
  Same transitional Step-1 note as http.rs applies here.
- `src-tauri/src/lib.rs` — `run()` builds ONE Engine; handlers migrate;
  `spawn_heartbeat` moves into `engine.rs::spawn_rotation`; both
  `on_page_load` re-emit blocks (slot-state AND status-state) migrate;
  the `server_once.call_once` `AppState` construction migrates. Step 1
  also makes a transitional edit to the 8 handler-test call sites here.
- `docs/TESTING_STRATEGY.md` §0 counts (+ any §9.1 line that names
  `enqueue` signatures)
- `CONTEXT.md` — one edit only: the Engine entry's "plan 037 —
  planned" parenthetical becomes a landed note
- `plans/README.md` (status row)

**Out of scope**:
- `src-tauri/src/status.rs` itself — Engine only *consumes*
  `LiveMatchSummary`/`StatusState`/`status_state_if_changed`/
  `emit_status_state` from it; no change to that module's own types or
  logic.
- Fixing the emit-after-unlock ordering (stays deferred; add the
  pointer comment in `apply`, nothing more)
- Any queue-semantics change; any SlotState/StatusState/wire change
- The poller loop-skeleton dedup (a separate candidate; 025 owns the
  fetch-path shape)
- Frontend code entirely

## Git workflow

- Branch: `advisor/037-engine-propagation` (or per operator dispatch).
- Commit sequence (each independently green):
  1. `queue: take now at the interface — clock-agnostic enqueue`
     (touches queue.rs plus the transitional non-queue.rs call sites —
     see Step 1)
  2. `engine: apply/read/accept/update_live_match + private rotation loop`
  3. `engine: migrate /notify + test notifications to accept`
  4. `engine: migrate pollers; delete enqueue_and_fan_out; note the
     QueueFull-wake behavior change`
  5. `engine: migrate tray/hotkey/on_page_load handlers; retire lib
     spawn_heartbeat`
  6. `docs: §0 counts + plans index for 037`
- Do NOT push.

## Steps

### Step 1: clock-agnostic queue

Change `enqueue`/`enqueue_test` to take `now: Instant`; delete
`enqueue_at`; update every queue.rs test call site mechanically (the
tests already hold a synthetic `t0`, or can pass `Instant::now()`
inline where they don't). The proptest harness's `apply_enqueue`
(`queue.rs:2130`, `self.q.enqueue_at(event, self.now)`) renames to
`self.q.enqueue(event, self.now)` — trivial, it already tracks its own
clock.

**Amendment (this retargeting pass): this step is NOT queue.rs-only.**
The signature change breaks compilation at every other call site until
they also pass a `now`. Rather than reorder the plan around that (which
would force Engine to exist before the queue can compile — circular),
add a transitional `Instant::now()` argument at each of these sites in
THIS step, and leave a `// TODO(engine): routed through Engine::apply/accept
in plan 037 step N` comment at each — Steps 3, 4, and 5 delete these
comments along with the transitional call as they migrate each site to
`Engine`. Locate them all with `rg -n "\.enqueue\(|\.enqueue_test\(" src-tauri/src --glob '!queue.rs'`;
as of `b8c554f` that's:
- `http.rs:81,83` (inside `enqueue_and_emit`) → `Instant::now()`
- `http.rs:566,726` (two tests building a queue directly) → `Instant::now()`
- `poller.rs:565` (inside `enqueue_and_fan_out`) → `Instant::now()`
- `rss_poller.rs:492` (the inline loop) → `Instant::now()`
- `lib.rs:930,961,992,995,1028,1031,1065,1163` (handler-test setup,
  each builds a raw `SingleSlotQueue`) → `Instant::now()`

This is safe: `enqueue`/`enqueue_test` already called `Instant::now()`
internally today, so passing it explicitly at the call site is a no-op
behavior change, purely mechanical, and every one of these call sites
is deleted or rewritten by a later step anyway (Steps 3–5 route them
through `Engine::apply`/`accept` instead, which reads `Instant::now()`
exactly once per operation — the "Engine-owned clock" locked design
decision is about the END state after Steps 2–5, not about Step 1 in
isolation).

**Verify**: `cargo test --locked` (from `src-tauri/`, the FULL suite —
not `queue::` alone, since this step necessarily touches http.rs/
poller.rs/rss_poller.rs/lib.rs too) → all pass; property suite 5×
green; `rg -n "Instant::now" src-tauri/src/queue.rs` → matches only
inside `#[cfg(test)]` (as a base instant), none in production code.

### Step 2: engine.rs

Create the module per the Design block (including both amendments):
`apply`/`apply_blocking`/`read`/`read_blocking`/`accept`/
`update_live_match`/`spawn_rotation`/`emit_current_blocking`/
`emit_current_status_blocking`, plus `Engine::new(queue:
SingleSlotQueue, app, connectors, espn_enabled: bool, rss_enabled:
bool)` — by-value queue, wake AND live created internally (nothing else
can hold either). Port `spawn_heartbeat`'s live body
(`lib.rs:723-783` — the post-036-AND-post-034 shape) into
`spawn_rotation` VERBATIM in shape: the `Notified::enable` under the
lock, the `live`-before-queue lock ordering, the `StatusState::snapshot`
+ `status_state_if_changed` + `emit_status_state` sequence with its own
task-local `last_status`, the 10 ms grace, the `select!` on the pinned
future. (The dependency gate at the top of this plan guarantees the
post-036 shape is what you find in lib.rs; this retargeting pass
confirmed the post-034 shape is there too, at the same location.)

Engine tests (in `engine.rs`'s `#[cfg(test)]`, `mock_app` pattern from
lib.rs — used throughout lib.rs's own handler tests, e.g. `lib.rs:928`;
private-field access makes the wake and `live` assertable):
1. `apply_wakes_and_emits` — port of `enqueue_and_emit_wakes_the_heartbeat`
   (`http.rs:800`) generalized to `apply` (register a waiter on the
   private wake, apply a mutation, assert the waiter fires).
2. `read_never_wakes` — register a waiter, `read`, assert timeout.
3. `accept_offers_manual_but_never_news` — fake-connector channel
   pattern from `http.rs::test_connector` (`http.rs:327`): a Manual
   event lands on the rx, a News event does not.
4. `accept_queue_full_propagates_nothing` — fill a tier, accept →
   `Err(QueueFull)`, no wake, no offer.
5. `rotation_loop_parked_idle_wakes_on_accept` — port/adopt the
   idle-park regression test plan 036 added to lib.rs (it exists in the
   live tree now — locate it with `rg -n "parked_idle_wakes" src-tauri/src/lib.rs`)
   against `spawn_rotation` + `accept`.
6. `emit_current_returns_and_emits` — seed a visible item, call
   `emit_current_blocking` twice, assert both calls emit (dedup
   bypassed) and return the same `Showing` state.
7. **NEW (amendment 1)** `emit_current_status_returns_and_emits` — same
   shape as test 6, over `emit_current_status_blocking`/`StatusState`:
   seed `live`/flags via the test harness's `Engine::new`, call twice,
   assert both emit and return the same snapshot.
8. **NEW (amendment 1)** `update_live_match_wakes_only_on_change` —
   mirrors `status.rs`'s own `status_state_if_changed` test pattern
   (`status.rs:~160-174`): call `update_live_match(Some(a))` (wakes),
   call it again with the same value (does NOT wake — assert via
   timeout), call it with a different value (wakes again).

**Verify**: `cargo test --locked engine::` → all pass, 3 consecutive
runs (the rotation test uses real timers; see 036's flake gate).

### Step 3: migrate ingest (http + settings)

`AppState` (`http.rs:25-43`) drops `queue`/`wake`/`connectors` in favor
of `engine: Engine<R>`. Also drop `app_handle`: checked during this
retargeting pass (`rg -n "app_handle" src-tauri/src/http.rs`) — its
only production read is passing `&state.app_handle` into
`enqueue_and_emit` (`http.rs:251`) for `emit_slot_state`, which Step 3
deletes; nothing else in http.rs reads `AppState.app_handle` once that
call is gone. Scalar config fields stay (`default_ttl`,
`manual_default_priority`, `cmux_priority`, `cmux_ttl_secs`).
`notify_handler` (`http.rs:192`) calls `engine.accept(event,
false).await`; the 200-vs-202 read (`http.rs:258-261`) becomes
`engine.read(|q| (q.is_paused(), q.total_waiting())).await`. Delete
`enqueue_and_emit` (`http.rs:69-95`; its doc-comment guarantee now
lives on the Engine itself — move the prose).

`send_test_notification` (`settings.rs:674`) today receives the
queue/wake/connectors via **tauri managed state** — three
`tauri::State<'_, Arc<…>>` parameters (`settings.rs:678-680`), plus an
unused-after-migration `app: tauri::AppHandle` (`settings.rs:676`).
Its new signature drops all four in favor of ONE managed-state
parameter:

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
(`http.rs:304-323`, building `AppState { queue: Arc::new(Mutex::new(queue)),
wake: Arc::new(Notify::new()), ..., app_handle: app.handle().clone(),
connectors: Arc::new(connectors) }` today) build an `Engine::new(queue,
app.handle().clone(), Arc::new(connectors), espn_enabled, rss_enabled)`
instead — pick a fixed `espn_enabled`/`rss_enabled` (e.g. both `true`,
matching neither test's assertions since none of these tests inspect
`StatusState`) for the two new required params. Heads-up on the ripple: eight test
assertions read `state.queue.lock().await` directly and need
migrating to `state.engine.read(...)`/`state.engine.apply(...)` — five
read `current_priority()` (`http.rs:597,615,634,651,668`, e.g.
`state.queue.lock().await.current_priority()` →
`state.engine.read(|q| q.current_priority()).await`) and three read
`current_slot_state()` (`http.rs:1045,1073,1094`, same treatment). The
wake-regression test has already moved to engine.rs (Step 2.1) — remove
the http copy (`http.rs:799-839`, `enqueue_and_emit_wakes_the_heartbeat`).

**Verify**: `cargo test --locked http::` → all pass;
`rg -n "enqueue_and_emit" src-tauri/` → zero matches.

### Step 4: migrate pollers

`spawn_espn_poller` (`poller.rs:585-676`) drops `app`, `queue`, `wake`,
`connectors`, `live` in favor of a single `engine: Engine<R>`, leaving
`(engine, leagues, poll_secs, ttl_secs, priority)` — 5 args, down from
9. `spawn_rss_poller` (`rss_poller.rs:428`) drops `app`, `queue`,
`wake` the same way, leaving `(engine, feeds, poll_secs, ttl_secs,
max_per_poll, priority)` — 6 args, down from 8. Delete the
`#[allow(clippy::too_many_arguments)]` above both (`poller.rs:584`,
`rss_poller.rs:427`) — both are now under the default 7-arg threshold.

Each accepted event goes through `engine.accept(ev, false)`; delete
`enqueue_and_fan_out` (`poller.rs:557-578`) and rss's inline
lock/enqueue/notify/emit block (`rss_poller.rs:487-501`). The espn/rss
offer difference is now automatic (Origin gate inside `accept`) — no
flags at the call sites. The espn poller's live-match-summary refresh
(`poller.rs:~660-677`, after the per-league loop) becomes
`engine.update_live_match(summary)` — deletes the direct `live.lock()`/
`wake.notify_waiters()` pair; the lock-discipline comment about queue
vs. live ordering moves into `Engine::update_live_match`'s doc comment
(already written in Step 2) and can be deleted here.

Name the QueueFull-wake behavior change (see "Deliberate behavior
changes" #2) in this step's commit message.

**Verify**: `cargo test --locked poller::` → all pass, then
`cargo test --locked rss_poller::` → all pass (two invocations —
cargo accepts only one positional test filter); `cargo clippy --locked
--all-targets -- -D warnings` → exit 0 (confirms the removed
`#[allow(...)]` attributes are no longer needed).

### Step 5: migrate lib.rs

**Wiring — follow this exactly; the Engine can only be constructed
where an `AppHandle` exists, which is inside `setup`:**

1. `run()` (`lib.rs:71`) keeps building the bare `SingleSlotQueue` as
   today (`with_rotation_order` + the `start_paused` pause,
   `lib.rs:110-116`) but does NOT wrap it in an Arc. Delete the
   `Arc::new(Mutex::new(…))` (`lib.rs:116`), the `wake` creation
   (`lib.rs:120`), the `live_match` creation (`lib.rs:125` — amendment
   1: this collapses into Engine too now), and the whole named-alias
   block: `setup_queue`/`rss_queue`/`page_load_queue`/`hotkey_queue`,
   `setup_wake`/`espn_wake`/`rss_wake`/`http_wake`/`tray_wake`/
   `hotkey_wake`, AND (amendment 1) `heartbeat_live`/`espn_live`/
   `page_load_live` — the whole block at `lib.rs:175-189`. Keep the
   `page_load_connectors`/`poller_connectors` clones (`lib.rs:172-173`)
   only if `accept`'s fan-out still needs them threaded that way after
   Step 4 — it doesn't (they route through `engine.accept`), so delete
   those two too. The bare queue moves into the `setup` closure.
2. At the TOP of `setup` (`lib.rs:206`):
   `let engine = Engine::new(queue, app.handle().clone(), connectors.clone(), espn_enabled, rss_enabled);`
   After this line, `run()` holds no queue/wake/live binding at all —
   by-value construction makes a retained alias a compile error, not a
   convention.
3. Replace the three managed-state lines `app.manage(queue.clone());
   app.manage(wake.clone()); app.manage(connectors.clone());`
   (`lib.rs:212,216,217`) with ONE: `app.manage(engine.clone());` —
   this is what Step 3's `tauri::State<'_, Engine>` parameter resolves
   to. Do NOT leave the old manage calls in place: a managed queue Arc
   is reachable from every command and silently defeats the seam while
   all of Step 6's greps still pass.
4. Consumers, all inside `setup` or later: `build_tray` (`lib.rs:666`)
   and the hotkey registration closure (`lib.rs:258-321`) take
   `&Engine<R>` / clones instead of separate `queue`/`wake` params;
   `toggle_pause`/`toggle_manual_expand`/`dismiss_current`/
   `skip_current` (`lib.rs:623,793,811,830`) each collapse their
   `queue`/`wake` params into one `engine: &Engine<R>`; `spawn_heartbeat`
   (`lib.rs:723-783`) is deleted — `engine.spawn_rotation()`
   (`lib.rs:353-360`'s call site) replaces it; `spawn_espn_poller`/
   `spawn_rss_poller` call sites (`lib.rs:367-389`) pass `engine.clone()`
   instead of the old queue/wake/connectors/live bundle (drop the
   `espn_live`/`live` argument entirely — Step 4 already dropped it
   from the callee signature).
5. The `on_page_load` closure (`lib.rs:394` onward) captures NOTHING
   queue-related — it retrieves the engine at page-load time via
   `webview.app_handle().state::<Engine>()` (the closure is built
   before `setup` runs, so it cannot capture the Engine directly; a
   second `Engine::new` there would create a second wake AND a second
   `live` that no rotation loop waits on or writes to — the exact
   stall/desync class 015/036 fixed — which is why retrieval-via-
   managed-state is the required shape). BOTH re-emit blocks migrate:
   the slot-state block (`lib.rs:416-423`) to
   `engine.emit_current_blocking()`, and the status-state block
   (`lib.rs:425-443`) to `engine.emit_current_status_blocking()`
   (Design amendment 1). The `server_once.call_once` closure
   (`lib.rs:481-492`) — which constructs `http::AppState` for the axum
   server — is where `AppState`'s new `engine` field actually gets
   populated: `AppState { engine: webview.app_handle().state::<Engine>().inner().clone(), default_ttl, manual_default_priority, cmux_priority, cmux_ttl_secs }`
   (drop `queue`/`wake`/`connectors`/`app_handle` from the literal —
   they're gone from the struct per Step 3).

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

`open_current_story` (`lib.rs:859`) uses `read_blocking`. Migrate the
lib.rs handler tests (`lib.rs:920-1250`, the eight sites listed in
Step 1's amendment) to construct an `Engine` instead of a raw queue —
these are the same eight call sites Step 1 gave a transitional
`Instant::now()`; delete that transitional argument now that they go
through `Engine`'s own clock.

**Verify**: `cargo test --locked` (full) → all pass;
`cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check`
→ exit 0.

### Step 6: the seam sweep (the point of the plan)

Prove the protocol is now structural:

- `rg -n "blocking_lock|\.lock\(\)\.await" src-tauri/src --glob '!engine.rs'`
  → zero matches on the queue OR on `live` (other mutexes, e.g. the
  managed `StdMutex<Config>`, are fine — eyeball the hits).
- `rg -n "notify_waiters|Notify::new" src-tauri/src --glob '!engine.rs'`
  → zero matches. (This is the grep that would have caught the
  poller.rs live-match wake this retargeting pass found — if it
  doesn't come back clean, `update_live_match` migration in Step 4 was
  missed or incomplete.)
- `rg -n "emit_slot_state|emit_status_state" src-tauri/src --glob '!engine.rs' --glob '!event.rs' --glob '!status.rs'`
  → zero matches.

**Verify**: all three greps clean; full suite + gates green; run the
property suite 5×.

### Step 7: reconcile docs

`docs/TESTING_STRATEGY.md` §0: recount per module (queue count
unchanged in intent; http loses the moved wake test; engine module is
new — has 8 tests per Step 2; lib loses the heartbeat test(s) if they
moved). The baseline going into this plan is 289 rust + 3 doc-tests /
105 vitest (confirmed at `b8c554f`) — recompute the actual post-migration
total, don't just copy that number forward. Fix any §9.1 sentence that
names the old `enqueue(event)` signature. `plans/README.md`: status
row.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` totals
match §0 exactly; `npx vitest run` count unchanged (105).

## Test plan

- New: the eight engine.rs tests (Step 2, six original + two from
  amendment 1) — the protocol itself finally has a direct test surface,
  now covering the status-state path too.
- Moved, not lost: the wake-regression test (http→engine), the
  heartbeat/rotation regression test (lib→engine).
- Everything else stays green UNCHANGED in behavior: the queue example
  tests + proptest suite (signature-only edits), the http/poller/
  rss_poller/settings/lib suites (counts per `docs/TESTING_STRATEGY.md`
  §0 at `b8c554f`: queue 64, http 37, poller 19, rss_poller 28,
  settings 45, lib 13).
- The two deliberate behavior changes each get explicit coverage: the
  News-gate change via Step 2 test 3 (`accept_offers_manual_but_never_news`);
  the QueueFull-wake change needs no new test (nothing pinned the old
  behavior — confirmed during this retargeting pass) but must be named
  in Step 4's commit message per "Deliberate behavior changes" above.

## Done criteria

- [ ] `cd src-tauri && cargo test --locked` exits 0; totals match §0
- [ ] Property suite passes 5× consecutively; no `proptest-regressions`
      file appears
- [ ] clippy `--locked -D warnings` + `cargo fmt --check` exit 0
- [ ] `npx vitest run` + `npx tsc --noEmit` green, counts unchanged (105)
- [ ] Step 6's three seam greps return zero out-of-engine matches
      (queue lock, wake/Notify, AND both emit functions)
- [ ] `rg -n "enqueue_and_emit|enqueue_and_fan_out|fn spawn_heartbeat" src-tauri/` → zero
- [ ] `rg -n "Instant::now" src-tauri/src/queue.rs` → test-module only
- [ ] `rg -n "too_many_arguments" src-tauri/src/poller.rs src-tauri/src/rss_poller.rs`
      → zero matches on `spawn_espn_poller`/`spawn_rss_poller` (the
      `rss_poller.rs:259`-area one, if still present, belongs to a
      different function — confirm before treating it as a leftover)
- [ ] CONTEXT.md Engine entry's "plan 037 — planned" parenthetical
      updated to note it landed
- [ ] `plans/README.md` status row updated

## STOP conditions

- The dependency gate fails (025/033/034/035/036 not all DONE, or no
  `enable()`-under-lock in `spawn_heartbeat`) — report "dependencies
  not landed"; do not execute against pre-dependency code.
- The drift check (against `b8c554f`) is nonempty and the changed
  file's excerpts in this plan no longer match live code after you've
  tried to reconcile them yourself — STOP and report the mismatch
  rather than guessing.
- A property-test or example-test failure you cannot attribute to a
  mechanical migration mistake — the refactor may have changed queue
  semantics; minimize, commit `#[ignore]`d, report.
- Preserving 036's enable-under-lock shape OR 034's live-before-queue
  lock ordering inside `spawn_rotation` proves impossible without
  changing wake or lock semantics — report; do not weaken either race
  fix to fit the seam.
- Any migration step seems to need the queue Arc, the wake, or the
  `live` handle exposed outside `engine.rs` — that is the design being
  violated; stop and report what needs it. (This is exactly the
  category of gap this retargeting pass found and fixed for the espn
  poller's live-match wake — if you find a similar site this plan
  didn't anticipate, treat it the same way: don't let it reach into
  `engine.rs`'s private state; report it so the plan can grow a narrow
  method for it, the way `update_live_match` was added.)
- 033/034/035 (or anything else) landed changes to queue/http/lib
  surfaces the excerpts don't match — reconcile the plan text first
  (drift check).
- The rotation-loop test is flaky (fails any of 3 consecutive runs).

## Maintenance notes

- **Every future queue-mutation site must go through
  `Engine::apply`/`accept`** — reviewers should treat an out-of-engine
  `lock` on the queue as a defect. The Step 6 greps are re-runnable as
  a review check.
- **Every future non-queue state that needs to wake the rotation loop
  should get its own narrow Engine method, following
  `update_live_match`'s precedent** — not a raw `wake` handle passed
  around. This is the lesson this retargeting pass encoded: plan 034
  added exactly such a site (the live-match summary) after the Engine's
  original design was locked, and it was free to reach the raw `wake`
  handle only because nothing stopped it.
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
