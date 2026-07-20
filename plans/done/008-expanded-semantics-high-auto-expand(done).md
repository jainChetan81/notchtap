# Plan 008: Fix Expanded semantics — auto-expand High at promotion, reset per item, guard idle toggling

> **STATUS: DONE** — executed at commit `b1981c9` (2026-07-17), verified
> against done criteria on 2026-07-18, then **rewritten to `8ca01e3`** the
> same day: the executor's original commit silently bundled an unrelated
> duplicate of plan 001's `skip_visible`/hotkey feature (already correctly
> isolated on `advisor/001-skip-and-settings-hotkeys`) alongside this
> plan's actual fix. The duplicate content was stripped via interactive
> rebase; the plan-008 logic itself was unchanged and re-verified after
> the rewrite (see "Post-execution verification" at the bottom of this
> file, updated). Kept for the record per the repo's `(done)`-suffix
> convention.

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src-tauri/src/queue.rs src-tauri/src/lib.rs`
> On any change, compare the excerpts below against live code; mismatch = STOP.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

The spec (`docs/V3_6_TECHNICAL_SPEC.md`, and `CONTEXT.md`'s **Expanded**
entry) says: Expanded is "automatic for `High`-priority Notifications,
manual (global hotkey) for everything else", and the hotkey "is a no-op
while an automatically-Expanded `High` item is Visible." The code
implements the no-op guard but **never implemented the auto-expand**, so:

- High-priority items — ESPN goals and cmux "agent needs input", the
  app's marquee events — can *never* show the expanded Manifest view, and
  ⌃⇧N is dead while one is visible (the guard guards nothing).
- Separately, `expanded` is a queue-level flag that is never reset between
  items: expand one Medium card and every subsequent notification renders
  expanded (with the ~3× rotation window) until manually collapsed. It can
  even be toggled while the slot is Empty, invisibly arming expansion for
  whatever promotes next.

The fix makes `expanded` per-turn: set true at promotion when the promoted
item is High, reset at every promotion otherwise, and make the hotkey a
no-op when nothing is visible.

**Two call sites promote an item into `self.visible`, not one** —
`promote_next` (used by `tick()` / rotation) AND `enqueue_new`'s
`can_promote_now` fast path (used whenever `enqueue()` is called on an
empty, unpaused queue — see "Current state" below). The fast path is not
a rare corner: `src-tauri/src/http.rs:73`, the `/notify` HTTP handler that
every CLI push, ESPN goal, and cmux relay message goes through, calls
`queue.enqueue(event)` directly, as do `poller.rs:563` and
`rss_poller.rs:491`. A High item arriving while the slot is already empty
— the common case, since the slot is idle most of the time — takes this
fast path. **Both sites must set the flag or the auto-expand bug is only
half-fixed**: it would start working for High items that promote from the
Waiting tier via rotation, but stay broken for the far more common case of
a High item pushed straight into an idle queue.

## Current state

`src-tauri/src/queue.rs` — the queue struct (line ~53) has
`expanded: bool`, initialized `false` (line ~69). **Two places** promote
an item into `self.visible`, and neither touches `expanded`:

```rust
// queue.rs:205-214 — promotion from the Waiting tier (via tick()/rotation)
fn promote_next(&mut self, now: Instant) {
    if self.visible.is_some() || self.paused {
        return;
    }
    if let Some(mut item) = self.pop_highest_priority_waiting() {
        item.promoted_at = Some(now);
        item.extension_secs = 0;
        self.visible = Some(item);
    }
}
```

```rust
// queue.rs:123-146 — the enqueue-time fast path: fires whenever enqueue()
// is called on an empty, unpaused queue and skips promote_next entirely.
fn enqueue_new(
    &mut self,
    event: Event,
    now: Instant,
    bypass_pause_when_slot_empty: bool,
) -> Result<(), QueueError> {
    let tier = event.priority as usize;
    let can_promote_now = self.visible.is_none()
        && (bypass_pause_when_slot_empty || !self.paused)
        && self.all_tiers_empty();
    if !can_promote_now && self.waiting[tier].len() >= self.max_queued_per_tier {
        return Err(QueueError::QueueFull);
    }
    let mut item = QueueItem {
        event,
        enqueued_at: now,
        promoted_at: None,
        extension_secs: 0,
    };
    if can_promote_now {
        item.promoted_at = Some(now);
        self.visible = Some(item);   // <-- this line is the fast path
    } else {
        self.waiting[tier].push_back(item);
    }
    Ok(())
}
```

(`bypass_pause_when_slot_empty` and `enqueue_test`/`enqueue_with_options`
are unrelated v5.1-era plumbing for settings test-notifications — ignore
that parameter, just note that `can_promote_now` is the branch that
matters here.)

```rust
// queue.rs:315-317
pub fn toggle_expanded(&mut self) {
    self.expanded = !self.expanded;
}
```

The rotation window consults it (`queue.rs:194`):
`let window = item.event.rotation_window(self.expanded) + item.extension_secs;`
and it is emitted on the wire (`queue.rs:347`): `expanded: self.expanded,`.

`src-tauri/src/lib.rs:649-662` — the hotkey handler and its guard:

```rust
// v3.6 spec §7.1.1: the hotkey is a manual override for "everything else"
// (§3.6's wording) — it's a no-op while the current slot is already
// auto-expanded (High priority), not a forced-collapse of an automatic
// expand.
#[cfg(target_os = "macos")]
fn toggle_manual_expand<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let mut q = queue.blocking_lock();
    if q.current_priority() == Some(crate::event::Priority::High) {
        return;
    }
    q.toggle_expanded();
    ...
```

Existing tests that pin today's (wrong) behavior — you will UPDATE these,
not delete them:
- `lib.rs` test `toggle_manual_expand` High case (~line 712) asserts
  `!expanded` for High — after this plan, a visible High item is
  `expanded == true` *because of auto-expand*, and the hotkey remains a
  no-op (still can't collapse it). Rewrite the assertion accordingly.
- `queue.rs` tests around lines 1060-1090 (`toggle_expanded` behavior,
  `expanded_increases_rotation_window`) — keep passing; they use Medium
  items.

Repo conventions: queue logic is TDD'd, tests live in
`#[cfg(test)] mod tests` at the bottom of `queue.rs`, use simulated
`Instant`s (no sleeps), and assert precise state. Test counts live ONLY in
`docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Queue tests | `cargo test queue::` (from `src-tauri/`) | all pass |
| Hotkey tests | `cargo test --lib -- lib` or `cargo test toggle_manual` (from `src-tauri/`) | all pass |
| Full suite | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/queue.rs` (promotion/toggle logic + tests)
- `src-tauri/src/lib.rs` (only the `toggle_manual_expand` comment/tests if
  assertions change — the guard logic itself stays)
- `docs/TESTING_STRATEGY.md` §0 (queue/lib counts) and, if §4.10's example
  list enumerates expand cases, add the new ones there
- `plans/README.md` (status row)

**Out of scope**:
- `rotation_window` math, extension caps, supersession — untouched.
- The frontend (`src/`) — it renders `slot.expanded` from the wire and
  needs no change.
- `dismiss_current` / pause paths.

## Git workflow

- Current branch; commit style: `queue: auto-expand High at promotion, reset expanded per item, no-op toggle while idle`.
- Do NOT push.

## Steps

### Step 1 (tests first): Write the failing tests

In `queue.rs`'s test module, add:
1. `high_priority_immediate_enqueue_auto_expands` — on a **fresh, empty,
   unpaused** `SingleSlotQueue`, call `q.enqueue(event("h", Priority::High,
   8)).unwrap()` directly (do NOT call `tick()` first — this must exercise
   `enqueue_new`'s `can_promote_now` fast path, queue.rs:143-146, not
   `promote_next`). Assert `current_slot_state()` is
   `Showing { expanded: true, .. }`. This is the regression test for the
   more common of the two promotion paths (see "Why this matters") —
   without it, a fix that only touches `promote_next` passes CI while the
   real-world bug (a High item pushed via `/notify` into an idle queue)
   stays broken.
2. `high_priority_promoted_from_waiting_auto_expands` — get a High item
   into the Waiting tier (e.g. enqueue a Medium item first so the queue is
   non-empty and the slot is occupied, then enqueue the High item — it
   queues behind the visible Medium item since a higher-or-equal item
   doesn't preempt), dismiss or tick past the Medium item so `promote_next`
   (not the fast path) promotes the High item next. Assert
   `current_slot_state()` is `Showing { expanded: true, .. }`. This is the
   regression test for the `promote_next` path specifically, kept separate
   from test 1 so neither call site can regress without a test noticing.
3. `expanded_resets_when_next_item_promotes` — promote a Medium item,
   `toggle_expanded()` (now true), tick past its window so a second
   waiting Medium item promotes; assert the new visible state has
   `expanded: false`.
4. `auto_expanded_high_uses_expanded_rotation_window` — a promoted High
   item's rotation-out happens at the *expanded* window, not the base one
   (mirror the arithmetic style of the existing
   `expanded_increases_rotation_window` test).
5. `toggle_expanded_is_noop_while_slot_empty` — on an empty queue,
   `toggle_expanded()` then enqueue+promote a Medium item; assert it
   promotes with `expanded: false` (the idle press armed nothing).

**Verify**: `cargo test queue::` → the 5 new tests FAIL, everything else passes.
(Test 1 must fail with `expanded: false`, proving the fast path is the
gap — if it *passes* before you touch any implementation code, something
about the test is wrong; re-read it against the `enqueue_new` excerpt
above before continuing.)

### Step 2: Implement in `enqueue_new`, `promote_next`, and `toggle_expanded`

Both promotion call sites must set the flag from the **same** rule, so
factor it into one private helper rather than duplicating the line —
this matches the codebase's own stated preference for a single choke
point per behavior (the spec makes the identical argument for
`slot_state_if_changed` in `docs/V3_6_TECHNICAL_SPEC.md` around line
936-938: "exactly one emission rule in the codebase, not two"). Add:

```rust
// Expanded is per-turn (CONTEXT.md "Expanded"): automatic for High,
// cleared for everything else — a leftover manual expand must not leak
// onto the next item. Called from both promotion sites (promote_next and
// enqueue_new's immediate-promote fast path) so neither can drift from
// the other.
fn set_expanded_for_promotion(&mut self, priority: Priority) {
    self.expanded = priority == Priority::High;
}
```

Call it from `promote_next` (queue.rs:205-214), replacing the body with:

```rust
fn promote_next(&mut self, now: Instant) {
    if self.visible.is_some() || self.paused {
        return;
    }
    if let Some(mut item) = self.pop_highest_priority_waiting() {
        item.promoted_at = Some(now);
        item.extension_secs = 0;
        self.set_expanded_for_promotion(item.event.priority);
        self.visible = Some(item);
    }
}
```

And from `enqueue_new`'s fast path (queue.rs:123-146) — this is the line
that closes the gap described in "Why this matters":

```rust
if can_promote_now {
    item.promoted_at = Some(now);
    self.set_expanded_for_promotion(item.event.priority);
    self.visible = Some(item);
} else {
    self.waiting[tier].push_back(item);
}
```

(`Priority` is already in scope via the existing `use crate::event::{...,
Priority, ...}` at the top of the file — no new import needed.)

In `toggle_expanded`, guard idle:

```rust
pub fn toggle_expanded(&mut self) {
    if self.visible.is_none() {
        return;
    }
    self.expanded = !self.expanded;
}
```

Also check `rotate_out_if_elapsed` / dismiss paths: after this change the
flag is authoritative only while an item is visible, and both promotion
sites always rewrite it via the helper, so no reset is needed there —
confirm by reading, don't add redundant resets.

**Verify**: `cargo test queue::` → ALL pass, including the 5 new ones.

### Step 3: Reconcile the hotkey handler and its tests

`toggle_manual_expand`'s High guard is now *correct* (High really is
auto-expanded; the hotkey must not collapse it) — keep the logic, update
the comment to say the auto-expand exists (it now does, in
`set_expanded_for_promotion`, called from both `promote_next` and
`enqueue_new`). Update the lib.rs test that asserted `!expanded` for a
visible High item: with auto-expand, the expected state after promotion is
`expanded == true`, and after the hotkey fires it is STILL `true` (no-op
proven). Keep the Medium toggle test as-is.

**Verify**: `cargo test` (full, from `src-tauri/`) → all pass. `cargo clippy --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 4: Docs counts

Update `docs/TESTING_STRATEGY.md` §0 queue count (+5) and, if you changed
lib.rs test assertions, confirm the lib count is unchanged. If §4.10's
example-case list enumerates expand behavior, append the new cases in the
same bullet style.

**Verify**: `cargo test` still green.

## Test plan

Covered by Steps 1–3: five new queue tests (auto-expand via the immediate
fast path, auto-expand via `promote_next`, per-item reset, expanded window
for High, idle no-op) + one updated lib.rs hotkey assertion. Pattern
exemplars: `expanded_increases_rotation_window` (queue.rs) and
`toggle_manual_expand_flips_expanded_for_non_high_priority` (lib.rs).

## Done criteria

- [x] `cargo test` exits 0; the 5 named tests exist and pass
- [x] `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check` exit 0 *for in-scope files* (repo-wide gate fails on pre-existing issues in `poller.rs` / `rss_poller.rs` / `settings.rs` — outside this plan's scope; see verification note below)
- [x] `grep -n "fn set_expanded_for_promotion" src-tauri/src/queue.rs` → 1 hit
- [x] `grep -n "self.set_expanded_for_promotion" src-tauri/src/queue.rs` → 2 hits (`promote_next` and `enqueue_new`'s fast path)
- [x] `docs/TESTING_STRATEGY.md` §0 updated
- [x] No files outside scope modified
- [x] `plans/README.md` status row updated

## STOP conditions

- The excerpts above don't match the live code (drift).
- Making High auto-expand breaks a test that encodes a *documented*
  contrary decision you find in `docs/V3_6_TECHNICAL_SPEC.md` §7.1.1 —
  read that section before Step 2; if it explicitly says High should NOT
  auto-expand (contradicting CONTEXT.md), stop and report the doc
  conflict instead of picking a side.
- Any supersession/extension test starts failing — the interaction of
  per-item reset with `apply_fresh_content` was judged safe by reading;
  if reality disagrees, report rather than patching supersession.
- Test 1 (`high_priority_immediate_enqueue_auto_expands`) passes BEFORE
  you write any implementation code — that would mean the test isn't
  actually exercising `enqueue_new`'s fast path as intended (e.g. it
  accidentally goes through `tick()`/`promote_next` instead). Stop and
  re-check the test against the "Current state" `enqueue_new` excerpt
  rather than proceeding with a non-discriminating test.

## Maintenance notes

- `self.expanded` is now set exclusively through
  `set_expanded_for_promotion`, called from both `promote_next` and
  `enqueue_new`'s fast path — this is intentionally the only place either
  call site touches the flag. If a third promotion path is ever added
  (e.g. a future "priority preemption" feature that lets a Waiting High
  item interrupt a visible Low item), it must call the same helper or
  this bug reappears in a new form.
- The reduced-motion / visual side of expanded High cards is manual-
  checklist territory (macbook + mac mini eyeballs) — add "High card
  auto-expands, ⌃⇧N no-ops on it, next item starts collapsed" to the §6
  manual checklist run.
- Plan 022 (queue property tests), if executed, should encode
  "expanded is always false immediately after a non-High promotion" as an
  invariant, and should specifically generate cases that hit both
  promotion call sites (empty-queue enqueue and Waiting-tier rotation).
- Watch in review: the wire `expanded` field now changes meaning slightly
  (it can be true without any user action) — the frontend already renders
  it unconditionally, so no change there, but any future "user collapsed
  this" analytics-style logic must not assume expanded == user intent.

## Post-execution verification (advisor, 2026-07-18, updated after rebase)

Originally executed at `b1981c9` ("queue: auto-expand High at promotion,
reset expanded per item, no-op toggle while idle"). That commit was found
to also contain an unrelated, unauthorized duplicate of plan 001's
`skip_visible` queue method and its two hotkeys — already correctly
implemented and isolated on `advisor/001-skip-and-settings-hotkeys`
(`a48c398`/`ea42e3d`/`7660e8a`), never merged. The duplicate was removed
via `git rebase -i` (`edit` on `b1981c9`, replace the 3 touched files with
a version containing only this plan's hunks, amend, continue). The
plan-008 commit is now `8ca01e3`, with `78f6984` (plan 006, unrelated)
rebased on top unchanged in content, resolving one docs-count merge
conflict.

Re-verified against live code at `8ca01e3` (and the current tip):

- All 5 named tests exist and the full suite passes (219 lib tests + 3
  doc-tests at `8ca01e3` alone; 220 + 3 once `78f6984`'s notifier test is
  included on top — 0 failures either way).
- `set_expanded_for_promotion` defined once (`queue.rs:223`), called from
  exactly the two promotion sites (`queue.rs:145` fast path, `:213`
  `promote_next`) — matches the plan's single-choke-point requirement.
- `toggle_expanded` has the idle guard; lib.rs test renamed to
  `toggle_manual_expand_is_a_noop_while_high_priority_is_visible` and
  asserts auto-expand + hotkey no-op, per Step 3.
- `grep -c skip_visible src-tauri/src/queue.rs src-tauri/src/lib.rs` → 0
  in both, at `8ca01e3` and at the current tip — confirms the duplicate is
  fully gone and the plan-001 branch remains the sole owner of that
  feature, mergeable later without conflict.
- `docs/TESTING_STRATEGY.md` §0 counts corrected in the same commit
  (214→219, queue 38→43 — the original commit had incorrectly counted
  the duplicate's tests too: 224/47).
- Gate caveat unchanged: `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check` still fail repo-wide, but every failure is in
  `poller.rs`, `rss_poller.rs`, or `settings.rs` — files this plan never
  touches — confirmed present on the true baseline (`d6a9050`) before
  this plan's work even started. The in-scope files (`queue.rs`, `lib.rs`)
  are clean.
