# Plan 097: Fix the hover-latch desync on hotkey dismiss/skip, the supersede top-up's hover blind spot, and the unclamped appearance load path

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. The reviewer maintains `plans/README.md` — do
> not edit it.
>
> **Worktree preflight (run before anything else)**: agent worktrees can
> branch from a stale HEAD. Run `git log --oneline master ^HEAD`; if it
> prints anything, run `git merge --ff-only master` and confirm it
> succeeds before starting.
>
> **Drift check (run second)**: `git diff --stat 0056f38..HEAD -- src-tauri/src/lib.rs src-tauri/src/queue.rs src-tauri/src/config.rs src-tauri/src/settings.rs`
> If any of these changed since `0056f38`, compare the "Current state"
> excerpts below against the live code before proceeding; on a mismatch,
> treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (touches the hover seam and the queue's time math)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `0056f38`, 2026-07-21

## Why this matters

Plan 093 shipped a TTL hover-pause with the invariant "a card must NEVER
rotate out while under the cursor." An audit found two holes and one
robustness gap:

1. **Hotkey desync**: dismissing/skipping the visible card by hotkey
   promotes the next card with its hover state reset, but the AppKit-side
   `was_hovered` latch never resets (no mouse event fired). The
   transitions-only gate then swallows the re-enter, so the new card is
   NOT hover-paused even though the cursor sits on it — and the webview
   still renders stale `hovered: true` UI (paused TTL bar) while the card
   is actually rotating.
2. **Top-up blind spot**: on topic supersede, the extension math uses raw
   `now - promoted_at`, ignoring banked hover-pause time, so a
   previously-hovered card looks closer to expiry than it is and gets up
   to 6 bonus seconds.
3. **Unclamped load**: appearance bounds (`card_scale` 0.8–1.4 etc.) are
   enforced only on the settings save path. A hand-edited `config.toml`
   with `card_scale = 0.0` boots unclamped and silently breaks hover
   geometry (degenerate hover rect) plus card rendering.

## Current state

- `src-tauri/src/lib.rs` — Tauri setup + AppKit glue. Relevant sites:
  - `lib.rs:340` (inside the macOS tracking-area setup block):
    ```rust
    let was_hovered = Arc::new(StdMutex::new(false));
    ```
    Captured (cloned) by three mouse-event closures at `:346`, `:371`,
    `:394`, each computing `hovered` and calling
    `emit_hover_changed_if_transitioned(&engine, &app_handle, &was_hovered, hovered)`.
  - `lib.rs:849-877` — the gate:
    ```rust
    fn emit_hover_changed_if_transitioned(
        engine: &Engine,
        app_handle: &tauri::AppHandle,
        was_hovered: &StdMutex<bool>,
        hovered: bool,
    ) {
        {
            let mut last = was_hovered.lock().unwrap();
            if *last == hovered {
                return;
            }
            *last = hovered;
        }
        if let Some(webview) = app_handle.get_webview_window("main") {
            let _ = webview.emit("hover-changed", &serde_json::json!({ "hovered": hovered }));
        }
        engine.apply_blocking(|q, now| {
            if hovered {
                q.hover_enter(now);
            } else {
                q.hover_exit(now);
            }
        });
    }
    ```
  - `lib.rs:420-455` — the global-shortcut handler. The dismiss arm
    (`:432-435`) calls `dismiss_current(&engine_for_handler)`; the skip
    arm (`:443-446`) calls `skip_current(&engine_for_handler)`. The
    closure also has `app` in scope (it calls `open_settings_window(app)`
    at `:453`).
  - `lib.rs:1017-1028` — `dismiss_current` / `skip_current` are thin
    `engine.apply_blocking` wrappers over `q.dismiss_visible(now)` /
    `q.skip_visible(now)`.
- `src-tauri/src/queue.rs` — the single-slot queue.
  - `:399-405` — promotion resets hover fields (this is correct; the bug
    is only the lib.rs latch):
    ```rust
    fn set_expanded_for_promotion(&mut self) {
        self.expanded = true;
        self.window_expanded = false;
        self.auto_retract_armed = true;
        self.hover_started_at = None;
        self.hover_paused_total = Duration::ZERO;
    }
    ```
  - `:330-337` — `hover_frozen_rotation_elapsed(&self, promoted_at, now)`
    returns `raw_elapsed - (hover_paused_total + in_flight)`, saturating.
  - `:346-348` — `hover_adjusted_promoted_at(&self, promoted_at)` returns
    `promoted_at + self.hover_paused_total`.
  - `:451-467` — the buggy top-up:
    ```rust
    fn top_up_visible_remaining_time(&mut self, now: Instant) {
        let Some(item) = &mut self.visible else {
            return;
        };
        let Some(promoted_at) = item.promoted_at else {
            return;
        };
        let base_window = item.event.rotation_window(self.window_expanded);
        let effective_window = base_window + item.extension_secs;
        let elapsed = now.saturating_duration_since(promoted_at).as_secs();
        let remaining = effective_window.saturating_sub(elapsed);
        if remaining < MIN_REMAINING_ON_SUPERSEDE_SECS {
            let deficit = MIN_REMAINING_ON_SUPERSEDE_SECS - remaining;
            let room = MAX_EXTENSION_ON_SUPERSEDE_SECS.saturating_sub(item.extension_secs);
            item.extension_secs += deficit.min(room);
        }
    }
    ```
    Note `now.saturating_duration_since(promoted_at)` — every OTHER
    deadline consumer (`next_deadline` at `:594`, `remaining_ms` at
    `:670`) anchors at `hover_adjusted_promoted_at` instead.
  - Hover test exemplars live at `queue.rs:2211-2290` (e.g.
    `visible_item_does_not_rotate_out_while_hover_held_past_its_window`).
    Match their construction style.
- `src-tauri/src/config.rs`:
  - `:139-146` — `pub struct Appearance { card_scale: f64, card_radius: f64, card_opacity: f64 }`
    with `default_card_*` serde defaults.
  - `:419-476` — `Config::parse` already self-heals `rotation_order`
    (dedupe + append missing) after deserializing. Appearance fields are
    NOT validated or clamped here.
  - Heal-test exemplars: `config.rs:783`
    (`rotation_order_missing_a_source_is_healed_by_appending_it`), `:803`.
- `src-tauri/src/settings.rs:219-241` — `validate_appearance` with the
  literal ranges `0.8..=1.4`, `0.0..=24.0`, `0.5..=1.0` (save path only).

Repo conventions: comments explain constraints and cite plan numbers
("plan 093: …"). Errors in queue/config are structural; no `unwrap` on
runtime-reachable fallible paths. Time math uses `saturating_*`.

## Commands you will need

Run cargo with the PATH prefix — the toolchain isn't on the default PATH:

| Purpose | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test` | all pass |
| Lint | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings` | exit 0 |
| Format | `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check` | exit 0 |
| Build | `PATH="$HOME/.cargo/bin:$PATH" cargo build` | exit 0 |

No frontend commands needed — this plan is rust-only.

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/lib.rs`
- `src-tauri/src/queue.rs`
- `src-tauri/src/config.rs`
- `src-tauri/src/settings.rs` (only to re-point `validate_appearance` at
  shared range constants — no behavior change)

**Out of scope** (do NOT touch):
- `docs/TESTING_STRATEGY.md` §0 test counts — plan 099 reconciles them
  after this merges. Do not update counts anywhere.
- `src-tauri/src/hover.rs`, `event.rs`, `history.rs` — unrelated.
- `src/` (frontend) — the webview needs no change; the fix re-drives the
  existing `hover-changed` event.
- `src-tauri/capabilities/*`, `src-tauri/build.rs` — no new commands here;
  these files must not change.

## Git workflow

- Branch: your dispatched worktree branch.
- Conventional commits, e.g.
  `fix(overlay): reset hover latch on hotkey dismiss/skip (plan 097)`.
- Do NOT push.

## Steps

### Step 1: Reset the hover latch when dismiss/skip hotkeys replace the card

In `src-tauri/src/lib.rs`:

1. Hoist the `was_hovered` declaration (currently `lib.rs:340`, inside
   the tracking-area setup block) up to the enclosing setup scope so it
   is in scope for BOTH the tracking-area block and the global-shortcut
   registration block (`:410-465`). Keep the existing clones into the
   three mouse closures unchanged.
2. Clone `was_hovered` (and an `AppHandle` — the shortcut closure already
   has `app`; use `app.clone()`/`app_handle` as the existing closures do)
   into the shortcut handler closure.
3. In the DISMISS arm, after `dismiss_current(&engine_for_handler);`, add:
   `emit_hover_changed_if_transitioned(&engine_for_handler, <app_handle>, &was_hovered, false);`
   Do the same in the SKIP arm after `skip_current(...)`.
   This resets the latch, emits `hovered: false` to the webview (clearing
   the stale paused-UI), and calls `q.hover_exit` (a harmless no-op —
   promotion already reset the queue's hover fields).
4. Add a short comment at the call sites: content under the cursor was
   replaced without any mouse event, so the latch must be forced back to
   "not hovered"; the next real mouse move re-enters. Note the accepted
   residual: a perfectly stationary cursor stays un-paused until it moves
   1px (cite plan 097).

Both blocks are `#[cfg(target_os = "macos")]`; keep the hoisted
declaration under the same cfg so non-macos builds stay clean.

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings` → exit 0.

### Step 2: Queue-side regression test for hotkey-dismiss-mid-hover

In `queue.rs`'s test module (near the hover tests at `:2211-2290`), add
`dismiss_while_hover_held_leaves_next_items_rotation_unfrozen`:
enqueue two items, promote the first, `hover_enter(now)`, advance past
the first item's window (assert it does NOT rotate — frozen), then
`dismiss_visible(now)`, assert the second item is visible, then advance
past the second item's full window WITHOUT any `hover_enter` and assert
it rotates out on schedule (its deadline was not inherited/frozen). If an
existing test already covers exactly this promotion-reset path, extend it
rather than duplicating — but the dismiss-mid-hover entry point must be
exercised.

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo test dismiss_while_hover` → 1+ tests pass.

### Step 3: Anchor the supersede top-up at hover-adjusted elapsed time

In `queue.rs:451-467`, compute elapsed via the hover-aware helper.
Borrow order matters — `hover_frozen_rotation_elapsed` takes `&self`, so
call it BEFORE taking the `&mut` borrow of `self.visible`:

```rust
fn top_up_visible_remaining_time(&mut self, now: Instant) {
    let Some(promoted_at) = self.visible.as_ref().and_then(|i| i.promoted_at) else {
        return;
    };
    let elapsed = self.hover_frozen_rotation_elapsed(promoted_at, now).as_secs();
    let Some(item) = &mut self.visible else {
        return;
    };
    let base_window = item.event.rotation_window(self.window_expanded);
    let effective_window = base_window + item.extension_secs;
    let remaining = effective_window.saturating_sub(elapsed);
    // ... unchanged from here
}
```

Add a comment: the top-up's notion of "remaining" must match the real
rotation deadline, which discounts banked and in-flight hover time
(`hover_adjusted_promoted_at` / plan 093); using raw elapsed here
over-granted extensions to previously-hovered cards (plan 097).

Add two tests next to the existing supersede/top-up tests (locate with
`grep -n "supersede\|top_up\|extension" src-tauri/src/queue.rs`):
- banked hover time: promote, hover for longer than
  `MIN_REMAINING_ON_SUPERSEDE_SECS`, exit hover, advance so RAW elapsed
  makes remaining look < MIN but hover-adjusted remaining is ample;
  supersede; assert `extension_secs` did NOT grow.
- unhovered behavior unchanged: replicate an existing top-up-grants case
  and assert the grant still happens with no hover involved.

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo test top_up` (and the two new test names) → pass.

### Step 4: Clamp appearance bounds on config load

1. In `config.rs`, define shared range constants near the `Appearance`
   struct (`:139`):
   ```rust
   pub const CARD_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.8..=1.4;
   pub const CARD_RADIUS_RANGE: std::ops::RangeInclusive<f64> = 0.0..=24.0;
   pub const CARD_OPACITY_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.0;
   ```
2. At the end of `Config::parse` (after the rotation_order heal, before
   `Ok(config)`), self-heal each appearance field, matching the heal
   style already in that function: if the value is non-finite
   (`!v.is_finite()`), replace with the field's default
   (`default_card_scale()` etc.); otherwise clamp into the range
   (`v.clamp(*RANGE.start(), *RANGE.end())`). Comment it as the
   load-path twin of `settings::validate_appearance` (which guards the
   save path only), citing plan 097.
3. In `settings.rs:219-241`, replace the three literal ranges in
   `validate_appearance` with the new shared constants so the two paths
   cannot drift. No message-text changes.
4. Tests in `config.rs`'s test module, modeled on
   `rotation_order_missing_a_source_is_healed_by_appending_it` (`:783`):
   - `appearance_out_of_range_is_clamped_on_load`: toml with
     `card_scale = 0.0`, `card_radius = 99.0`, `card_opacity = 2.0` →
     parses Ok with `0.8`, `24.0`, `1.0`.
   - `appearance_non_finite_falls_back_to_defaults`: toml with
     `card_scale = nan` → parses with the default scale (compare against
     `Appearance::default().card_scale`; do not hardcode).
   - in-range values pass through untouched.

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo test appearance` → new tests pass.

## Test plan

Summarized from the steps: one queue regression test for
dismiss-mid-hover (Step 2), two top-up hover tests (Step 3), three
config clamp tests (Step 4). The lib.rs latch fix itself is AppKit glue
with no headless test seam — it is covered by compile + clippy plus the
manual smoke row added below; say so plainly in your report rather than
inventing a mock harness.

## Done criteria

ALL must hold (run from `src-tauri/` with the PATH prefix):

- [ ] `cargo test` exits 0; the new tests named in Steps 2–4 exist and pass
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `grep -n "saturating_duration_since(promoted_at)" src/queue.rs` shows NO hit inside `top_up_visible_remaining_time`
- [ ] `grep -c "0.8..=1.4" src/settings.rs` → 0 (literals replaced by shared constants)
- [ ] `git status` shows no modified files outside the in-scope list
- [ ] `git diff master -- src-tauri/capabilities src-tauri/build.rs` is empty

## STOP conditions

Stop and report back (do not improvise) if:

- The shortcut-registration block and the tracking-area block turn out
  NOT to share an enclosing scope where `was_hovered` can be hoisted
  (e.g. they live in different functions).
- `emit_hover_changed_if_transitioned`'s signature can't be called from
  the shortcut closure without changing its signature or threading new
  generics through `Engine`.
- The borrow restructuring in Step 3 still fails to compile after the
  prescribed reordering.
- Any existing hover/rotation proptest fails after your change.
- The "Current state" excerpts don't match the live code.

## Maintenance notes

- The stationary-cursor residual (card replaced under a motionless
  cursor stays un-paused until 1px of movement) is accepted; if it ever
  matters, the fix is querying `NSEvent.mouseLocation` at promotion —
  out of scope here.
- The expand-toggle hotkey has a milder cousin of this bug (collapsing a
  card can shrink the rect out from under the cursor, leaving a stale
  hover-pause until the mouse moves). Deliberately deferred: it
  over-pauses rather than under-pauses, and self-corrects on movement.
- Reviewer: scrutinize the Step 3 borrow reorder for behavior identity in
  the no-hover case, and confirm Step 1 emits for BOTH dismiss and skip
  arms (not just dismiss).
- Manual smoke (operator's batched hour): hover a card, hotkey-dismiss,
  confirm the next card's TTL bar is NOT shown paused and rotates
  normally; wiggle the cursor and confirm it pauses.
