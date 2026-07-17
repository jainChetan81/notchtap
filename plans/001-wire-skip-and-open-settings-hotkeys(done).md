# Plan 001: Wire the two "planned" global hotkeys (⌃⇧] skip, ⌃⇧, open settings)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat d6a9050..HEAD -- src-tauri/src/lib.rs src-tauri/src/queue.rs src/settings/SettingsApp.tsx src/settings/SettingsApp.test.tsx src/settings/settings.css docs/IMPLEMENTATION_PLAN.md docs/TESTING_STRATEGY.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on
> a mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction (feature — closing a stated-but-undelivered gap)
- **Planned at**: commit `d6a9050`, 2026-07-17
- **Revised**: 2026-07-17, after a grilling session — the original draft
  mapped skip to `dismiss_current` verbatim; that was reversed (see
  "Design decisions" below) and four internal defects were fixed (test-scope
  contradictions, a stale UI footnote the flip would leave behind, a
  vacuous replacement assertion, and undisclosed registration-failure /
  manual-verification caveats).
- **Revised (2)**: 2026-07-17, after a cold-read review by a zero-context
  executor: `settings.css` and `plans/README.md` added to the in-scope
  list (Step 5 and the executor preamble already required editing them —
  the scope list contradicted the steps); drift-check path list synced to
  the scope; the false "exactly the commands CI runs" claim corrected;
  the `cargo test skip` done criterion replaced (two pre-existing tests
  also match "skip"); a baseline-recording Step 0 added; a second stale
  copy string (the Shortcuts section *description*) added to Step 5;
  Step 6's scoped assertion made primary; Step 7's insertion point and
  diff verification made exact.
- **Revised (3)**: 2026-07-17, reconciled before execution after unrelated
  v5.1 and docs work changed every in-scope file since the original stamp.
  The hotkey/queue/shortcuts seams remain structurally unchanged; line
  anchors and the drift SHA were refreshed to `d6a9050`. Added the required
  `docs/TESTING_STRATEGY.md` §0 count update (+5 Rust tests), matching the
  repository's current plan-index convention.
- **Revised (4)**: 2026-07-17, after execution exposed two baseline gate
  failures under Rust 1.97.0 that Step 0 had not checked. At untouched
  `d6a9050`, `cargo fmt --check` reports pre-existing formatting drift in
  `lib.rs`, `queue.rs`, and out-of-scope `settings.rs`; Clippy reports four
  pre-existing errors in out-of-scope `poller.rs`, `rss_poller.rs`, and
  `settings.rs`. The executor must format every newly added hunk, prove the
  remaining output is baseline-identical, and must not widen scope to repair
  those unrelated failures. This documented baseline exception replaces the
  impossible requirement that both full-repo commands exit 0 in this plan.

## Why this matters

The settings window's Shortcuts page (`src/settings/SettingsApp.tsx`) already
advertises two hotkeys — `⌃⇧]` "Skip to the next waiting item" and `⌃⇧,`
"Open settings" — labeled `status: "planned"` and rendered in the UI as
"planned · not implemented" (`SettingsApp.tsx:806`). This is the product
telling the user, in the shipped UI, that these exist and don't work yet.
The four *active* hotkeys (`⌃⇧N` expand, `⌃⇧O` open story, `⌃⇧X` dismiss,
`⌃⇧P` pause) are all wired using one consistent, well-tested pattern in
`src-tauri/src/lib.rs`. This plan closes the gap by following that exact
pattern for the two remaining combos, and flipping their `status` to
`"active"` in the settings UI once wired.

### Design decisions (locked by the maintainer 2026-07-17 — do not improvise on these)

1. **Skip is NOT dismiss.** Skip means "end the Visible item's turn *now*,
   exactly as if its Rotation window had elapsed naturally" — a
   **Recurring** item requeues to the back of its own Priority tier's
   Waiting line (it comes back after a lap), a **OneShot** drops, and the
   next Waiting item is Promoted immediately. This mirrors the queue's
   natural rotation-out (`rotate_out_if_elapsed`, `queue.rs:191-203`) and
   is deliberately different from dismiss (`dismiss_visible`,
   `queue.rs:300-303`), whose contract is "dropped, not requeued: 'get rid
   of this' means gone" (that function's own doc comment). An earlier draft
   of this plan aliased skip to dismiss; that was rejected in review because
   (a) it made `⌃⇧]` and `⌃⇧X` literally identical — two hotkeys, one
   behavior — and (b) pressing "skip" on a Recurring item (e.g. a live match
   card) would have destroyed it permanently, contradicting what the word
   "skip" promises. This plan therefore adds one small new queue method,
   `skip_visible` (Step 1), with tests. The vocabulary here is
   `CONTEXT.md`'s: **Slot**, **Visible**, **Waiting**, **Promotion**,
   **Rotation**, **Recurring** — use these terms in doc comments.
2. **"Open settings" hotkey** calls the exact same `open_settings_window`
   function the tray's "Settings…" menu item already calls
   (`lib.rs:604-623`) — lazy-create-or-focus, no new logic, no new tests
   (the function is unchanged; its tray call is untested today by the
   repo's own explicit choice, and this plan doesn't reopen that).

### Honest limits of this plan (stated up front, not discovered later)

- **The done criteria cannot prove the hotkeys work.** Every gate below is
  a compile/test/grep check. You could swap the two function calls between
  the two new handler branches and every automated gate would still pass.
  `docs/TESTING_STRATEGY.md` §5 keeps actual OS keypresses manual by
  design (hardware-dependent). Step 7 therefore adds the two combos to the
  manual verification checklist in `docs/IMPLEMENTATION_PLAN.md` §6 as a
  **required** step of this plan — a human on the dev machine must press
  both combos once before this feature is believed working.
- **Each global-shortcut registration is a launch-failure risk.** The
  registration calls end in `?` inside `.setup()` — if any other app on
  the machine already holds one of these combos, `register` fails, setup
  errors, and **the whole app fails to launch**. This is a pre-existing
  property of the four current hotkeys (this plan follows the established
  pattern and does not change it), but adding two more combos widens the
  surface, and `⌃⇧]`/`⌃⇧,` sit closer to window-manager territory
  (Rectangle, BetterTouchTool) than the existing letter keys. If this ever
  bites, the fix is downgrading `?` to a `tracing::warn!` for *all six*
  registrations — a separate, deliberate change, not part of this plan.

## Current state

- `src-tauri/src/queue.rs` — owns the single-slot queue; this plan adds
  one method (`skip_visible`) next to `dismiss_visible` plus tests.
- `src-tauri/src/lib.rs` — owns all four active hotkey consts, the plugin
  registration, the handler match arms, and the handler functions. This
  plan adds two consts, two registrations, two dispatch branches, one new
  thin handler (`skip_current`), and one test.
- `src/settings/SettingsApp.tsx` — the `shortcuts` array (lines 182-189),
  the `ShortcutsSection` component (lines 797-813), a footnote at line 811
  that must be deleted, and the Shortcuts section *description* at line 147
  ("…available now and planned next.") that goes stale with the flip —
  both handled in Step 5.
- `src/settings/settings.css` — holds the `.shortcut-footnote` rule
  (line 799) that becomes dead when the footnote is deleted (Step 5).

The queue's natural rotation-out — the semantics skip must reproduce
(`queue.rs:191-203`):

```rust
fn rotate_out_if_elapsed(&mut self, now: Instant) {
    let Some(item) = &self.visible else { return };
    let promoted_at = item.promoted_at.expect("visible items have promoted_at");
    let window = item.event.rotation_window(self.expanded) + item.extension_secs;
    if now.duration_since(promoted_at).as_secs() < window {
        return;
    }
    let item = self.visible.take().expect("checked Some above");
    if let RotationSpec::Recurring { .. } = item.event.rotation {
        let tier = item.event.priority as usize;
        self.waiting[tier].push_back(item);
    }
}
```

The dismiss contract skip must NOT copy (`queue.rs:294-303`):

```rust
/// Manually dismiss the current visible item, if any, and promote the
/// next waiting item immediately — mirrors what `tick` does on natural
/// rotation-out, but caller-triggered rather than TTL-triggered. Unlike
/// a natural rotation-out, a dismissed Recurring item is dropped, not
/// requeued: "get rid of this" means gone, not "back after a lap
/// through the other tiers."
pub fn dismiss_visible(&mut self, now: Instant) {
    self.visible = None;
    self.promote_next(now);
}
```

Note: a requeued item's stale `promoted_at`/`extension_secs` are reset at
its next Promotion (`promote_next`, `queue.rs:205-213`, sets
`promoted_at = Some(now); extension_secs = 0;`) — so `skip_visible` does
not need to touch those fields, same as `rotate_out_if_elapsed` doesn't.

Exact current hotkey consts (`lib.rs:35-48`):

```rust
// placeholder combo — v3.6 spec §7.1 explicitly defers "exact global hotkey
// combination" as an open detail; isolated to one constant.
#[cfg(target_os = "macos")]
const EXPAND_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyN);
#[cfg(target_os = "macos")]
const OPEN_STORY_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyO);
#[cfg(target_os = "macos")]
const DISMISS_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyX);
#[cfg(target_os = "macos")]
const PAUSE_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyP);
```

Registration + dispatch, inside `.setup()` (`lib.rs:199-254`, abbreviated —
read the full block in the file before editing; this excerpt elides some
surrounding closure capture lines and is reformatted (the real code is
rustfmt-wrapped across more lines — content is what matters, not layout):

```rust
#[cfg(target_os = "macos")]
{
    let hotkey_queue_for_handler = hotkey_queue.clone();
    let pause_item_for_handler = pause_item.clone();
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if *shortcut == Shortcut::new(EXPAND_TOGGLE_SHORTCUT.0, EXPAND_TOGGLE_SHORTCUT.1) {
                        toggle_manual_expand(app, &hotkey_queue_for_handler);
                    } else if *shortcut == Shortcut::new(OPEN_STORY_SHORTCUT.0, OPEN_STORY_SHORTCUT.1) {
                        open_current_story(app, &hotkey_queue_for_handler);
                    } else if *shortcut == Shortcut::new(DISMISS_SHORTCUT.0, DISMISS_SHORTCUT.1) {
                        dismiss_current(app, &hotkey_queue_for_handler);
                    } else if *shortcut == Shortcut::new(PAUSE_TOGGLE_SHORTCUT.0, PAUSE_TOGGLE_SHORTCUT.1) {
                        toggle_pause(app, &hotkey_queue_for_handler, &pause_item_for_handler);
                    }
                }
            })
            .build(),
    )?;
    app.global_shortcut().register(Shortcut::new(EXPAND_TOGGLE_SHORTCUT.0, EXPAND_TOGGLE_SHORTCUT.1))?;
    app.global_shortcut().register(Shortcut::new(OPEN_STORY_SHORTCUT.0, OPEN_STORY_SHORTCUT.1))?;
    app.global_shortcut().register(Shortcut::new(DISMISS_SHORTCUT.0, DISMISS_SHORTCUT.1))?;
    app.global_shortcut().register(Shortcut::new(PAUSE_TOGGLE_SHORTCUT.0, PAUSE_TOGGLE_SHORTCUT.1))?;
}
```

`dismiss_current` (`lib.rs:664-675`) — the structural template for the new
`skip_current` (Step 3):

```rust
#[cfg(target_os = "macos")]
fn dismiss_current<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let mut q = queue.blocking_lock();
    q.dismiss_visible(Instant::now());
    if let Some(state) = q.slot_state_if_changed() {
        drop(q);
        emit_slot_state(app, state);
    }
}
```

`open_settings_window` (`lib.rs:601-623`) — the function `⌃⇧,` will call. It
takes only `app: &tauri::AppHandle<R>`, no queue argument:

```rust
fn open_settings_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }
    match tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("notchtap settings")
    .inner_size(480.0, 600.0)
    .build()
    {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!("settings window failed to open: {e}"),
    }
}
```

Note `open_settings_window` is **not** `#[cfg(target_os = "macos")]`-gated
today. Do not add a `#[cfg]` to it, and do not change its signature or
behavior — the tray depends on it as-is.

Queue test fixtures to reuse (all already exist in `queue.rs`'s
`mod tests`): `event(title, priority, ttl_secs)` (`queue.rs:381`, builds a
OneShot), `recurring_event(title, priority, display_secs)` (`queue.rs:398`),
`visible_title(&q)` (`queue.rs:442`), `waiting_titles(&q, tier)`
(`queue.rs:446`). The dismiss test block to model skip's tests on is
`queue.rs:972-1016` (four tests: clears-and-promotes, noop-when-empty,
drops-recurring, respects-paused).

`lib.rs`'s test module (`#[cfg(all(test, target_os = "macos"))] mod tests`,
`lib.rs:704-817`) has an `event(priority)` helper that builds a **OneShot**
event (`RotationSpec::OneShot { ttl_secs: 8 }`) — the new `skip_current`
test (Step 4) needs a Recurring event; build it inline with
`RotationSpec::Recurring { display_secs: 8 }` (see Step 4's exact code).

Frontend shortcuts table (`src/settings/SettingsApp.tsx:182-189`):

```ts
const shortcuts = [
  { keys: "⌃⇧N", action: "Expand or collapse the slot (manual)", status: "active" },
  { keys: "⌃⇧O", action: "Open the current story's link", status: "active" },
  { keys: "⌃⇧X", action: "Dismiss the visible notification now", status: "active" },
  { keys: "⌃⇧P", action: "Pause or resume promotion", status: "active" },
  { keys: "⌃⇧]", action: "Skip to the next waiting item", status: "planned" },
  { keys: "⌃⇧,", action: "Open settings", status: "planned" },
] as const;
```

And the footnote that becomes stale once no row is "planned"
(`SettingsApp.tsx:811`):

```tsx
<p className="shortcut-footnote">Planned key combinations are placeholders and may change before implementation.</p>
```

The corresponding vitest assertion that must change
(`SettingsApp.test.tsx:85`):

```ts
expect(await screen.findAllByText("planned · not implemented")).toHaveLength(2);
```

**Repo conventions this plan must follow**: every hotkey combo constant has
a one-line comment explaining why that key was picked
(`docs/V3_6_TECHNICAL_SPEC.md` §7.1.2's precedent: avoid combos already
registered and avoid common macOS `⌘`-based shortcuts). `⌃⇧]` and `⌃⇧,`
were already chosen and shipped in the settings UI — implement exactly
those two, do not choose different ones. Pure queue logic is TDD'd first
(`docs/TESTING_STRATEGY.md` §3) — Step 1 writes the queue tests alongside
the method, before any lib.rs wiring.

## Commands you will need

| Purpose        | Command                                              | Expected on success |
|----------------|-------------------------------------------------------|---------------------|
| Rust test      | `cargo test` (from `src-tauri/`)                      | exit 0; pass count increases by exactly the 5 new tests this plan adds (4 queue + 1 lib) over the Step 0 baseline |
| Rust fmt check | `cargo fmt --check` (from `src-tauri/`)               | only the baseline `d6a9050` diffs remain; no new-hunk diff |
| Rust lint      | `cargo clippy --all-targets -- -D warnings` (from `src-tauri/`) | only the four baseline Rust 1.97 errors remain; no new error |
| Rust build     | `cargo build` (from `src-tauri/`)                     | exit 0 |
| TS typecheck   | `npx tsc --noEmit` (repo root)                        | exit 0 |
| Frontend tests | `npx vitest run` (repo root)                          | exit 0, all pass |
| Frontend build | `npx vite build` (repo root)                          | exit 0 |

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, and `cargo test` in its rust job, and
`npm ci` + `npx tsc --noEmit` + `npx vitest run` + `npx vite build` in its
web job (plus a `swift build` of `notchtap-detect/`, which this plan never
touches). `cargo build` is not in CI (clippy `--all-targets` covers
compilation there) — it's in this table as a fast local gate for Steps 2-3,
not a CI mirror.

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/queue.rs` — one new method (`skip_visible`) next to
  `dismiss_visible`, plus 4 tests in the existing `mod tests`.
- `src-tauri/src/lib.rs` — two new consts, two registrations, two dispatch
  branches, one new thin handler (`skip_current`), one test.
- `src/settings/SettingsApp.tsx` — flip both entries' `status` to
  `"active"`; delete the stale footnote; update the stale section
  description (Step 5).
- `src/settings/settings.css` — delete the orphaned `.shortcut-footnote`
  rule (line 799) in the same step.
- `src/settings/SettingsApp.test.tsx` — replace the "planned" count
  assertion with a positive "active" count assertion.
- `docs/IMPLEMENTATION_PLAN.md` — one new row in §6's manual verification
  checklist (Step 7 — **required**, not optional).
- `docs/TESTING_STRATEGY.md` — update §0's canonical Rust total and
  queue/lib breakdown for the five new tests (Step 7).
- `plans/README.md` — the status row for this plan only (per the executor
  preamble), nothing else in that file.

**Out of scope** (do NOT touch, even though they look related):
- `dismiss_visible` / `dismiss_current` — unchanged. Skip is a sibling,
  not a replacement; both hotkeys ship, with different semantics.
- `rotate_out_if_elapsed` — do NOT refactor it to share code with the new
  `skip_visible` (e.g. extracting a common `end_turn` helper). The
  duplication is ~4 lines and intentional for this plan: `skip_visible`'s
  early-return shape differs (no elapsed-window check), and refactoring
  the tick path is risk this small feature doesn't need to carry. Note it
  in the new method's comment instead (see Step 1's exact code).
- `src-tauri/capabilities/default.json` / `settings.json` — hotkey
  registration happens entirely rust-side (`app.global_shortcut()`), same
  as the four existing hotkeys; per `docs/V3_6_TECHNICAL_SPEC.md` §7.1 this
  needs **no** new capability entries. Do not add any.
- `src-tauri/build.rs` — unrelated to hotkeys (it gates the settings-window
  *invoke commands*, not global shortcuts). Do not touch.
- Any change to `open_settings_window`'s existing signature or behavior.
- The `?` on the six `register(...)` calls — the launch-failure fragility
  described in "Honest limits" above is real but pre-existing; changing the
  error handling for all six registrations is a separate decision, not
  this plan's.

## Git workflow

- Branch: `advisor/001-skip-and-settings-hotkeys` — create it **before
  Step 0** (the baseline run belongs on the branch too).
- Three commits, one per logical step, matching the repo's terse,
  colon-prefixed style (closest precedent in `git log --oneline`:
  `overlay: dismiss-now (⌃⇧X) and pause-toggle (⌃⇧P) global shortcuts`):
  1. `queue: skip_visible — end the turn like a natural rotation-out`
     (Step 1)
  2. `overlay: skip (⌃⇧]) and open-settings (⌃⇧,) global shortcuts`
     (Steps 2-4)
  3. `settings: shortcuts table all-active — copy cleanup, manual checklist row`
     (Steps 5-7)
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 0: Record the baseline

Before touching any file, run both suites and record their pass counts —
the later "+5 / +0" checks are relative to these numbers, and nothing else
in this plan tells you what they were:

- `cargo test` (from `src-tauri/`) → note every `N passed` result line.
  The output has multiple result lines (per-target unit/integration tests
  plus a doc-test line); record them all — the "+5" invariant applies to
  the lib target's unit-test line (where `queue.rs` and `lib.rs` tests
  live), and the doc-test line must stay unchanged.
- `npx vitest run` (repo root) → note the total "Tests: N passed" line.
  This number must be unchanged at the end (the plan modifies an existing
  assertion, adds no frontend test).
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` (from
  `src-tauri/`) are known-red at `d6a9050` under Rust 1.97.0. Preserve their
  complete baseline output for comparison: fmt reports only existing drift
  in `lib.rs`, `queue.rs`, and `settings.rs`; Clippy reports exactly three
  `too_many_arguments` errors (`poller.rs:spawn_espn_poller`,
  `rss_poller.rs:diff_feed`, `rss_poller.rs:spawn_rss_poller`) and one
  `field_reassign_with_default` error (`settings.rs` test code).

**Verify**: both test commands exit 0 before you start. The two known-red
Rust quality gates must match the failures enumerated above; any additional
baseline error is a STOP condition.

### Step 1: Add `skip_visible` to the queue, tests first

In `src-tauri/src/queue.rs`, immediately after `dismiss_visible`
(`queue.rs:300-303`), add:

```rust
/// Skip the Visible item: end its turn now, exactly as if its Rotation
/// window had elapsed naturally — a Recurring item requeues to the back
/// of its own Priority tier's Waiting line, a OneShot drops — then
/// promote the next Waiting item immediately. Contrast with
/// [`Self::dismiss_visible`], which drops a Recurring item outright:
/// skip means "not now, come back later", dismiss means "gone".
/// The requeue arm deliberately mirrors (not shares — see the plan that
/// added this) `rotate_out_if_elapsed`'s: stale `promoted_at` /
/// `extension_secs` on the requeued item are reset at its next
/// Promotion, so neither needs touching here.
pub fn skip_visible(&mut self, now: Instant) {
    if let Some(item) = self.visible.take() {
        if let RotationSpec::Recurring { .. } = item.event.rotation {
            let tier = item.event.priority as usize;
            self.waiting[tier].push_back(item);
        }
    }
    self.promote_next(now);
}
```

Then add these 4 tests to `queue.rs`'s existing `mod tests`, placed
directly after `dismiss_visible_respects_paused` (`queue.rs:1006-1016`),
reusing the existing fixtures (`event`, `recurring_event`, `visible_title`,
`waiting_titles`):

```rust
#[test]
fn skip_visible_requeues_recurring_to_back_of_own_tier_and_promotes_next() {
    // the exact case that distinguishes skip from dismiss: a Recurring
    // item survives a skip (dismiss_visible_drops_recurring_item_rather_
    // than_requeue proves dismiss destroys it)
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(recurring_event("recur", Priority::Medium, 8))
        .unwrap();
    q.enqueue(event("next", Priority::Medium, 8)).unwrap();

    q.skip_visible(Instant::now());

    assert_eq!(visible_title(&q), Some("next"));
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["recur"]);
}

#[test]
fn skip_visible_drops_oneshot_and_promotes_next() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(event("a", Priority::Medium, 8)).unwrap();
    q.enqueue(event("b", Priority::Medium, 8)).unwrap();

    q.skip_visible(Instant::now());

    assert_eq!(visible_title(&q), Some("b"));
    assert_eq!(q.total_waiting(), 0);
}

#[test]
fn skip_visible_is_noop_when_nothing_visible() {
    let mut q = SingleSlotQueue::new(50);

    q.skip_visible(Instant::now());

    assert!(q.visible.is_none());
    assert_eq!(q.total_waiting(), 0);
}

#[test]
fn skip_visible_respects_paused() {
    // paused: the recurring item still requeues, but nothing promotes
    // (Promotion is frozen — CONTEXT.md's Paused contract)
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(recurring_event("recur", Priority::Medium, 8))
        .unwrap();
    q.pause();

    q.skip_visible(Instant::now());

    assert!(q.visible.is_none());
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["recur"]);
}
```

**Verify**: `cargo test skip_visible` (from `src-tauri/`) → 4 passed.
Then `cargo test` → exit 0, lib-target unit-test pass count is Step 0's
baseline + 4.

### Step 2: Add the two new shortcut constants

In `src-tauri/src/lib.rs`, immediately after `PAUSE_TOGGLE_SHORTCUT`
(line 48), add:

```rust
// ⌃⇧] / ⌃⇧, — chosen (and already shipped in the settings UI's shortcut
// table) to avoid the four combos above and common macOS ⌘-based
// shortcuts, same rule as ⌃⇧X/⌃⇧P (v3.6 spec §7.1.2).
#[cfg(target_os = "macos")]
const SKIP_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::BracketRight);
#[cfg(target_os = "macos")]
const OPEN_SETTINGS_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::Comma);
```

`Code::BracketRight` and `Code::Comma` are both valid variants of the
`keyboard_types::Code` enum re-exported by `tauri_plugin_global_shortcut`
(confirmed present in the `keyboard-types 0.7.0` crate this project already
depends on via `Cargo.lock`). Note these are **physical-key** codes (DOM
`code` semantics) — on non-ANSI keyboard layouts the physical key may not
print `]`/`,`. The four existing hotkeys share this property; both target
machines use US layouts, accepted.

**Verify**: `cargo build` (from `src-tauri/`) → exit 0, no "no variant
named `BracketRight`/`Comma`" error.

### Step 3: Add `skip_current`, then register and dispatch both shortcuts

1. In `lib.rs`, directly after `dismiss_current` (`lib.rs:665-675`), add
   the thin handler — identical shape, calling the new queue method:

```rust
// ⌃⇧]: end the Visible item's turn as if its Rotation elapsed (Recurring
// requeues, OneShot drops) — deliberately different from ⌃⇧X's dismiss,
// which drops a Recurring item outright. See SingleSlotQueue::skip_visible.
#[cfg(target_os = "macos")]
fn skip_current<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let mut q = queue.blocking_lock();
    q.skip_visible(Instant::now());
    if let Some(state) = q.slot_state_if_changed() {
        drop(q);
        emit_slot_state(app, state);
    }
}
```

2. Add two more `else if` branches to the `.with_handler(...)` closure,
   after the existing `PAUSE_TOGGLE_SHORTCUT` branch (order doesn't affect
   behavior — each branch is mutually exclusive on shortcut identity — but
   this keeps the visual order matching the settings UI's table):

```rust
} else if *shortcut == Shortcut::new(SKIP_SHORTCUT.0, SKIP_SHORTCUT.1) {
    skip_current(app, &hotkey_queue_for_handler);
} else if *shortcut == Shortcut::new(OPEN_SETTINGS_SHORTCUT.0, OPEN_SETTINGS_SHORTCUT.1) {
    open_settings_window(app);
}
```

3. Add two more `register` calls after the existing four:

```rust
app.global_shortcut()
    .register(Shortcut::new(SKIP_SHORTCUT.0, SKIP_SHORTCUT.1))?;
app.global_shortcut()
    .register(Shortcut::new(OPEN_SETTINGS_SHORTCUT.0, OPEN_SETTINGS_SHORTCUT.1))?;
```

`open_settings_window(app)` takes `app: &tauri::AppHandle<R>` — the handler
closure's `app` parameter is already that type (see how
`toggle_manual_expand(app, ...)` is called in the same closure). No wrapper
function is needed for the settings hotkey.

Before moving on, manually re-read all six branches and confirm each const
uses a distinct `Code` value (`KeyN`, `KeyO`, `KeyX`, `KeyP`,
`BracketRight`, `Comma`) and each branch calls the function its combo
promises — the automated gates below **cannot catch** a swapped pair of
calls or a duplicated `Code` (the branches are independent equality checks,
not an exhaustive match; clippy `-D warnings` will not flag an unreachable
`else if` here).

**Verify**: `cargo build` (from `src-tauri/`) → exit 0.

### Step 4: Add the `skip_current` unit test

In `lib.rs`'s existing `#[cfg(all(test, target_os = "macos"))] mod tests`,
after `dismiss_current_is_noop_when_slot_already_empty` (`lib.rs:781-792`),
add one test covering the skip-vs-dismiss distinction at the handler level
(the pure queue permutations are already covered by Step 1's four tests —
don't duplicate them here; this one test proves the handler calls the
*skip* path, not the dismiss path):

```rust
#[test]
fn skip_current_requeues_recurring_and_promotes_next() {
    let app = tauri::test::mock_app();
    let mut inner = SingleSlotQueue::new(50);
    let mut recurring = event(Priority::Medium);
    recurring.rotation = RotationSpec::Recurring { display_secs: 8 };
    let recurring_id = recurring.id;
    inner.enqueue(recurring).unwrap();
    let next = event(Priority::Medium);
    let next_id = next.id;
    inner.enqueue(next).unwrap();
    let queue = Arc::new(Mutex::new(inner));

    skip_current(&app.handle().clone(), &queue);

    let mut q = queue.blocking_lock();
    match q.current_slot_state() {
        SlotState::Showing { id, .. } => assert_eq!(id, next_id),
        SlotState::Empty => panic!("expected Showing"),
    }
    // the skipped Recurring item survived — this is what distinguishes
    // skip_current from dismiss_current (whose test proves the drop)
    assert_eq!(q.total_waiting(), 1);
    // and it comes back: skip the next item too and the recurring one
    // promotes again
    q.skip_visible(Instant::now());
    match q.current_slot_state() {
        SlotState::Showing { id, .. } => assert_eq!(id, recurring_id),
        SlotState::Empty => panic!("expected recurring item to return"),
    }
}
```

The test module's existing imports (`lib.rs:706-710`) already include
`RotationSpec` and `SlotState` — confirm before adding; if `RotationSpec`
is missing from that `use` line, add it there rather than using a full
path inline.

**Verify**: `cargo test skip_current` (from `src-tauri/`) → 1 passed.
Then `cargo test` → exit 0, lib-target unit-test count is Step 0's
baseline + 5.
Then run `cargo fmt --check` and manually format every diff touching a line
added by this plan. Re-run it and confirm only the pre-existing `d6a9050`
diffs remain. Run `cargo clippy --all-targets -- -D warnings` and confirm it
reports only the four pre-existing errors recorded in Step 0, with no error
at or caused by a line added by this plan. Do not edit unrelated baseline
code to make either full-repo command green.

### Step 5: Flip both entries to `"active"`, fix the two stale copy strings

In `src/settings/SettingsApp.tsx`:

1. Change the two `shortcuts` entries:

```ts
{ keys: "⌃⇧]", action: "Skip to the next waiting item", status: "active" },
{ keys: "⌃⇧,", action: "Open settings", status: "active" },
```

2. Delete the footnote line (`SettingsApp.tsx:811`) — with zero "planned"
   rows it refers to nothing and would actively mislead:

```tsx
<p className="shortcut-footnote">Planned key combinations are placeholders and may change before implementation.</p>
```

3. Update the Shortcuts section description (`SettingsApp.tsx:147`), which
   also references planned shortcuts and goes stale with the flip:

```ts
// before:
description: "A reference for the global controls available now and planned next.",
// after:
description: "A reference for the global controls available while notchtap runs.",
```

   (No test asserts this string — verified; only the General section's
   description is asserted in `SettingsApp.test.tsx`.)

4. Delete the now-orphaned `.shortcut-footnote` rule in
   `src/settings/settings.css` (line 799) — nothing else uses that class.

5. **Leave the ternary at lines 805-806 in place**
   (`shortcut.status === "active" ? "active" : "planned · not implemented"`).
   With an all-`"active"` const array its false branch is currently dead,
   but it is the rendering affordance for any *future* planned shortcut —
   deleting it would force the next planned-shortcut author to re-invent
   it. tsc will not error on the comparison (the literal types overlap).
   If the repo's lint setup ever flags the dead branch, that's the moment
   to revisit — not now.

**Verify**: `npx tsc --noEmit` (repo root) → exit 0. Then
`grep -n "shortcut-footnote" src/settings/settings.css src/settings/SettingsApp.tsx`
→ **no output, exit code 1** (grep exits 1 on zero matches — that exit
code is the success signal here, not a failure).

### Step 6: Replace the vitest assertion with a positive one

In `src/settings/SettingsApp.test.tsx:85`, replace:

```ts
expect(await screen.findAllByText("planned · not implemented")).toHaveLength(2);
```

with:

```ts
const shortcutTable = screen.getByRole("table", { name: "Keyboard shortcuts" });
expect(within(shortcutTable).getAllByText("active")).toHaveLength(6);
expect(screen.queryAllByText("planned · not implemented")).toHaveLength(0);
```

The positive assertion is the load-bearing one — it fails if a row goes
missing, if a flip didn't happen, or if the section broke entirely (an
absence-only assertion would pass in all three of those failure modes).
The absence check rides along as documentation of the flip. Scoping with
`within` (already imported in this file, line 2) is deliberate: it pins the
count to the shortcut table's own six status spans (the `role="table"
aria-label="Keyboard shortcuts"` element, `SettingsApp.tsx:800`), so an
unrelated "active" text node elsewhere in the window can never inflate the
count. No `await`/`find*` is needed — the preceding line in this test
(`findByText("Expand or collapse the slot (manual)")`) has already waited
for the section to render. This stays inside the existing
`it("renders sidebar navigation and switches among available sections", ...)`
test — do not extract it into a new test, and do not remove that preceding
line, which must stay.

**Verify**: `npx vitest run` (repo root) → exit 0, all tests pass
including the modified assertions.

### Step 7 (REQUIRED): Update canonical counts and add the manual keypress row

First update `docs/TESTING_STRATEGY.md` §0's Rust suite row. The reconciled
baseline at `d6a9050` is 214 total Rust tests, with queue 38 and lib (hotkey)
5. Change only that row to 219 total, queue 42, and lib (hotkey) 6; every
other module count, the 3 doc-tests, and the 62 frontend tests stay unchanged.

Then add the manual keypress row to `docs/IMPLEMENTATION_PLAN.md` §6.

The automated gates cannot prove the hotkeys work (see "Honest limits").
In `docs/IMPLEMENTATION_PLAN.md` §6's manual checklist, insert **after the
last of the three consecutive v3.6 rows** (the "a live espn goal …
single-slot model" bullet, around lines 787-788 — do not wedge the new
bullet between the v3.6 rows), add:

```markdown
- [ ] hotkeys (plans/001): ⌃⇧] with a Recurring item Visible requeues it
      (it returns after the queue laps) and promotes the next item; ⌃⇧]
      with a OneShot Visible drops it; ⌃⇧, opens/focuses the settings
      window from any app; both verified by real keypress on the dev
      machine (TESTING_STRATEGY.md §5 — not automatable)
```

Do not restructure anything else in that checklist.

**Verify**: `git diff --numstat docs/IMPLEMENTATION_PLAN.md` → exactly
`5	0	docs/IMPLEMENTATION_PLAN.md` (5 lines added — the bullet above is
5 physical lines — 0 deleted, nothing else changed in that file). If you
already committed earlier steps, diff against the pre-plan commit instead:
`git diff --numstat d6a9050 -- docs/IMPLEMENTATION_PLAN.md` → same result.
Also verify `git diff --numstat d6a9050 -- docs/TESTING_STRATEGY.md` reports
exactly `1\t1\tdocs/TESTING_STRATEGY.md`, and the edited row says 219 total,
queue 42, lib (hotkey) 6.

## Test plan

- **New Rust tests (5 total)**: 4 queue tests in `queue.rs` (Step 1 —
  recurring-requeues-and-promotes, oneshot-drops-and-promotes,
  noop-when-empty, respects-paused; modeled on the `dismiss_visible` test
  block at `queue.rs:972-1016`) + 1 handler test in `lib.rs` (Step 4 —
  proves the hotkey path requeues rather than drops, i.e. that
  `skip_current` is wired to `skip_visible` and not accidentally to
  `dismiss_visible`).
- **No test for `open_settings_window`** — the function is unchanged, and
  its existing tray call is untested by the repo's own explicit choice
  (thin window-creation call). Wiring a second caller to it doesn't change
  that calculus.
- **Frontend**: the existing sidebar-navigation test's assertion is
  strengthened in place (Step 6), not duplicated into a new test.
- Full-suite: `cargo test` (from `src-tauri/`) → Step 0 baseline + 5 on
  the lib target's unit-test line, doc-tests unchanged; `npx vitest run`
  (repo root) → same count as the Step 0 baseline, all green. The
  invariant is "+5 Rust, +0 frontend, nothing broken" relative to your own
  recorded baseline — not any absolute number from a doc.
- **Canonical count**: `docs/TESTING_STRATEGY.md` §0 changes from 214 to
  219 Rust tests, queue 38→42 and lib (hotkey) 5→6; doc-test/frontend counts
  are unchanged.
- **Manual (required, human-only)**: Step 7's checklist row — real
  keypresses on the dev machine. This plan is not "verified working"
  until that row is checked; the automated done criteria below only prove
  the wiring compiles and nothing regressed.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo fmt --check` (from `src-tauri/`) has no diff in any line added
      by this plan; its remaining output is baseline-identical drift in
      `lib.rs`, `queue.rs`, and `settings.rs`
- [ ] `cargo clippy --all-targets -- -D warnings` (from `src-tauri/`) has no
      new diagnostic; its remaining output is exactly the three baseline
      `too_many_arguments` errors and one `field_reassign_with_default` error
- [ ] `cargo build` (from `src-tauri/`) exits 0
- [ ] `cargo test` (from `src-tauri/`) exits 0; lib-target unit-test pass
      count = Step 0 baseline + 5; doc-test count unchanged
- [ ] `cargo test skip_visible` (from `src-tauri/`) → 4 passed;
      `cargo test skip_current` → 1 passed (these filters are exact — do
      NOT use plain `cargo test skip`, which also matches two pre-existing
      tests: `gate_skips_every_tick_while_paused` in `poller.rs` and
      `diff_feed_skips_title_that_sanitizes_to_empty` in `rss_poller.rs`)
- [ ] `npx tsc --noEmit` (repo root) exits 0
- [ ] `npx vitest run` (repo root) exits 0, pass count = Step 0 baseline
      (no new frontend tests, none lost)
- [ ] `npx vite build` (repo root) exits 0
- [ ] `grep -n '"planned"' src/settings/SettingsApp.tsx` → no output,
      exit code 1
- [ ] `grep -n "shortcut-footnote" src/settings/SettingsApp.tsx src/settings/settings.css`
      → no output, exit code 1
- [ ] `grep -n "planned next" src/settings/SettingsApp.tsx` → no output,
      exit code 1 (the section description was updated in Step 5)
- [ ] `grep -n "SKIP_SHORTCUT\|OPEN_SETTINGS_SHORTCUT" src-tauri/src/lib.rs`
      shows both consts defined, both registered, both dispatched
- [ ] `grep -n "skip_visible" src-tauri/src/queue.rs` shows the method and
      its 4 tests; `grep -n "skip_current" src-tauri/src/lib.rs` shows the
      handler, its dispatch branch, and its test
- [ ] `git diff --numstat d6a9050 -- docs/IMPLEMENTATION_PLAN.md` →
      `5	0	docs/IMPLEMENTATION_PLAN.md` (the one checklist bullet, nothing
      else)
- [ ] `git diff --numstat d6a9050 -- docs/TESTING_STRATEGY.md` →
      `1	1	docs/TESTING_STRATEGY.md`; §0 says 219 total Rust tests,
      queue 42, lib (hotkey) 6, with doc-test/frontend counts unchanged
- [ ] No files outside the in-scope list are modified (`git status --short`
      plus `git diff --stat d6a9050` — every touched path appears in this
      plan's Scope section)
- [ ] `plans/README.md` status row for `001` updated to `DONE (manual
      keypress verification pending)` — it becomes plain `DONE` only after
      Step 7's checklist row is checked by a human on the dev machine

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the cited `lib.rs`/`queue.rs` line ranges doesn't match the
  excerpts in "Current state" (the codebase has drifted since this plan was
  written) — small formatting differences are fine, but if the
  hotkey-handling closure has been restructured (e.g. moved to a match
  statement, or the handler functions renamed) or `dismiss_visible`/
  `rotate_out_if_elapsed` have changed shape, STOP.
- `Code::BracketRight` or `Code::Comma` do not exist in the vendored
  `keyboard_types` version (verify via `cargo build`'s error if Step 2
  fails) — do not substitute a different combo without asking; the settings
  UI text (`⌃⇧]`, `⌃⇧,`) is a public-facing promise already made.
- Step 1's tests reveal `skip_visible`'s intended semantics conflict with
  some queue invariant this plan didn't anticipate (e.g. a supersession or
  per-tier-cap interaction that makes "requeue on skip" wrong) — report
  the conflict; do not weaken the tests to make them pass.
- Any of the six shortcut consts end up sharing a `Code` value after your
  edit — this would silently make one hotkey unreachable, and no automated
  gate catches it.
- A verification command other than the two explicitly documented
  Rust-1.97 baseline exceptions fails twice after a reasonable fix attempt;
  or either exception gains a new diff/diagnostic attributable to this plan.

## Maintenance notes

- **Skip vs dismiss is now a deliberate behavioral pair** — `⌃⇧]` requeues
  Recurring, `⌃⇧X` drops it. If either semantic is ever changed, change the
  settings UI's action text in the same commit (the two tables — `lib.rs`
  consts and `SettingsApp.tsx` `shortcuts` — are independently maintained,
  no shared source of truth; any hotkey change must touch both).
- **Registration fragility**: six global combos now each carry the
  pre-existing "another app holds this combo → notchtap fails to launch"
  risk (the `?` on `register`). If a launch failure is ever traced to
  this, the fix is downgrading all six registrations to `tracing::warn!` —
  one deliberate change, not per-hotkey patches.
- If a seventh hotkey is added, follow this plan's const → register →
  dispatch-branch → thin-handler pattern again; at ~10 branches consider
  replacing the `if`/`else if` chain with a lookup table.
- The skipped-item requeue reuses `rotate_out_if_elapsed`'s logic by
  **mirroring, not sharing** (see Step 1's comment) — if the natural
  rotation-out's requeue rules ever change (e.g. a new `RotationSpec`
  variant), `skip_visible` must be updated in lockstep; grep for
  `skip_visible` from any change to `rotate_out_if_elapsed`.
- `TESTING_STRATEGY.md` §5 keeps actual OS keypresses manual forever —
  Step 7's checklist row is that discipline applied to these two combos;
  don't let the row be checked off by anything other than a real keypress.
</content>
