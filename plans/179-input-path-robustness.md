# Plan 179: Input-path robustness — live scale in the click monitor, poison-tolerant AppKit locks, tab emit under its lock

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src-tauri/src/click.rs src-tauri/src/lib.rs src-tauri/src/status.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none (independent of 175-178; touches `lib.rs` in different regions than 178 — whoever lands second reconciles by reading)
- **Category**: bug
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Three small, verified defects in the plan-171 input path, batched because
they share files and a theme (the native input path must not trust boot
state or panic across FFI):

1. **The click monitor freezes `card_scale` at boot.** The monitor closure
   captures the boot config's scale; the Appearance settings hot-apply
   scale changes at runtime (`set_appearance`). After a scale change, the
   hover hit-test (which re-reads config per event) and the webview both
   move; the click rects keep the old scale until relaunch — clicks miss
   visible icons with nothing in the logs.
2. **Three lock sites reachable from AppKit callbacks use `.unwrap()`.**
   House policy in the same code paths is poison tolerance
   (`unwrap_or_else(|e| e.into_inner())`) — the new click monitor and the
   slot-state listener both do it, on the SAME mutexes. A poisoning panic
   elsewhere (the `Config` mutex is held across `write_config_atomic`)
   would make every later mouse event panic **inside an objc callback**,
   which is UB-adjacent unwinding across FFI rather than a clean crash.
3. **`tab-selection-changed` is emitted after its dedup lock is dropped,
   with two writer threads.** The click/prefix path (AppKit main thread)
   and the engine's status loop (tokio worker) both call the emit helper.
   Unlock-then-emit is safe with one writer (the `hover-changed` precedent
   it copied) but with two, an interleave can put the OLDER payload on the
   wire LAST — and the mismatch sticks, because the next call compares
   against the newer guard value and returns early.

## Current state

`src-tauri/src/lib.rs:748` — the boot-frozen scale (inside `setup`, the
`#[cfg(target_os = "macos")]` block that builds `ClickMonitorParams`):

```rust
let monitor_scale = config.appearance.card_scale;
```

`src-tauri/src/click.rs` — `ClickMonitorParams` carries `scale: f64`
(struct field), destructured at `:84` and used in the handler at
`:121-128`:

```rust
let rects = crate::hover::icon_strip_rects(
    mode,
    cutout_width,
    cutout_height,
    scale,
    present.len(),
    real_window_height,
);
```

The handler already reads other live state poison-tolerantly, e.g.
`click.rs:104`:

```rust
if !*was_hovered.lock().unwrap_or_else(|e| e.into_inner()) {
```

The live-read precedent, `src-tauri/src/lib.rs:1445-1450`
(`hover_point_is_over_card`) — note it is ALSO one of the `.unwrap()`
offenders (defect 2):

```rust
let scale = app_handle
    .state::<StdMutex<Config>>()
    .lock()
    .unwrap()
    .appearance
    .card_scale;
```

The other two `.unwrap()` sites reachable from AppKit callbacks:
`lib.rs:550` (`let hover_latched = *was_hovered.lock().unwrap();` inside
the tracking-area closure) and `lib.rs:1521`
(`let mut last = was_hovered.lock().unwrap();` in
`emit_hover_changed_if_transitioned`, called from those closures).

`src-tauri/src/status.rs:295-313` — the emit-after-unlock (defect 3):

```rust
pub fn emit_tab_selection_if_transitioned<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    last: &std::sync::Mutex<Option<crate::tabs::Tab>>,
    selected: Option<crate::tabs::Tab>,
) {
    use tauri::Emitter;
    {
        let mut guard = last.lock().unwrap_or_else(|e| e.into_inner());
        if *guard == selected {
            return;
        }
        *guard = selected;
    }
    let payload = serde_json::json!({
        "selected": selected.map(crate::tabs::Tab::wire_label),
    });
    if let Err(e) = app.emit("tab-selection-changed", payload) {
        tracing::error!("failed to emit tab-selection-changed: {e}");
    }
}
```

Its two writers: `engine.rs:480` (status loop, tokio worker) and
`lib.rs:2204` (`apply_tab_select`, reached from `click.rs:134` on the
AppKit main thread and from the prefix keymap). `app.emit` is documented
in this repo as a synchronous, non-blocking post (`engine.rs:226`'s
comment), so emitting under the mutex adds no new blocking edge; the same
mutex is already the only lock this function takes.

## Commands you will need

Run from `src-tauri/` (frontend untouched).

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` | all pass |
| Rust lints | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --locked --all-targets -- -D warnings` | exit 0 |

`npx vitest run` (repo root) must still pass, unchanged.

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/click.rs`
- `src-tauri/src/lib.rs` (only: the `ClickMonitorParams` construction in
  `setup`, and the three `.unwrap()` lock sites listed above)
- `src-tauri/src/status.rs` (only `emit_tab_selection_if_transitioned`)
- `docs/TESTING_STRATEGY.md` §0 (counts, if changed)

**Out of scope** (do NOT touch, even though they look related):
- Every other `.lock().unwrap()` in the codebase — only the three sites
  reachable from AppKit callbacks are in scope; a repo-wide sweep is a
  different (unselected) change.
- `emit_hover_changed_if_transitioned`'s unlock-then-emit shape — single
  writer, safe, documented; leave it (fixing only its `.unwrap()`).
- The prefix watchdog region (plan 178's territory).
- `catch_unwind` wrappers around monitor bodies — considered and NOT
  included (bigger design question about what state is safe after a
  caught panic); do not add one.

## Git workflow

- Branch: `advisor/179-input-path-robustness`
- Commit style: conventional, e.g. `fix(click): read card_scale live in the monitor` (one commit per defect is fine)
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Live scale in the click monitor

Remove the `scale: f64` field from `ClickMonitorParams` (`click.rs`), and
the `monitor_scale` capture at `lib.rs:748`. Inside the handler closure,
read the scale where the rects are computed, exactly the way
`hover_point_is_over_card` does but poison-tolerantly:

```rust
let scale = app
    .state::<std::sync::Mutex<crate::config::Config>>()
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .appearance
    .card_scale;
```

(Match the real types in the file — `StdMutex<Config>` naming, the
`app` handle already captured by the closure. Confirm the managed-state
type by reading how `lib.rs:1445` obtains it.) Update the struct's doc
comment: geometry inputs that can change at runtime are read per-event
(scale), fixed ones stay captured (window number, mode, cutout).

**Verify**: `cargo test --locked` → all pass;
`grep -n "scale" src-tauri/src/click.rs` → no struct field, one live read.

### Step 2: Poison-tolerant locks at the three AppKit-reachable sites

Change `.lock().unwrap()` to
`.lock().unwrap_or_else(|e| e.into_inner())` at exactly:

- `lib.rs:1445-1450` (config read in `hover_point_is_over_card`)
- `lib.rs:550` (`was_hovered` read in the tracking-area closure)
- `lib.rs:1521` (`was_hovered` in `emit_hover_changed_if_transitioned`)

No other semantic change. One-line comment at the first site naming the
policy ("poison-tolerant: a panic elsewhere must not turn every later
mouse event into a panic inside an objc callback" — matching
`board.rs`'s existing phrasing).

**Verify**: from `src-tauri/`:
`grep -n "lock().unwrap()" src/lib.rs` → none of the three listed lines
remain (other files/lines untouched); `cargo clippy --locked --all-targets
-- -D warnings` → exit 0.

### Step 3: Emit `tab-selection-changed` under its lock

In `status.rs::emit_tab_selection_if_transitioned`, move the payload build
and `app.emit` INSIDE the mutex scope (delete the inner block's closing
brace accordingly), so wire order is a total order under that lock.
Rewrite the doc comment paragraph that likens it to
`emit_hover_changed_if_transitioned`: state that hover has a single writer
(unlock-then-emit safe there) while tab selection has two (click/prefix on
the main thread, engine loop on a tokio worker), and that `app.emit` is a
synchronous non-blocking post (`engine.rs:226`), so holding the lock across
it is deliberate.

**Verify**: `cargo test --locked status` → all pass.

### Step 4: Full gates

Both cargo commands green; `npx vitest run` from the repo root green and
unchanged; recount `docs/TESTING_STRATEGY.md` §0 only if a count moved.

## Test plan

- Existing suites must stay green — these are behaviour-preserving except
  under the exact failure interleavings, which are not deterministically
  testable without real threads/timers (the repo's accepted-exception list
  allows no new real-timer tests).
- Add one small unit test if cheap: `emit_tab_selection_if_transitioned`
  called twice with the same value emits once (pin the dedup while you are
  in the file). Use the existing test-app harness in `status.rs`'s test
  module if one exists; skip if the module has no emit-capturing harness —
  do not build new test infrastructure for this.

## Done criteria

- [ ] `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` exit 0 (from `src-tauri/`)
- [ ] `ClickMonitorParams` has no `scale` field; the handler reads config per event
- [ ] The three listed `.unwrap()` lock sites are poison-tolerant
- [ ] `status.rs`: `app.emit("tab-selection-changed", ...)` executes inside the `last` mutex scope
- [ ] `npx vitest run` exits 0 (unchanged)
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Reading the `Config` state inside the click handler would deadlock
  (i.e. you find any path where the click handler is invoked while the
  config mutex is held by the same thread — search `install_click_monitor`
  callers and `set_appearance` for main-thread lock holds first; at
  planning time none existed).
- The three `.unwrap()` sites don't match the quoted lines (drift).
- Emitting under the lock in Step 3 creates a lock-order edge with any
  OTHER lock taken inside `app.emit`'s listeners (search for rust-side
  `listen` handlers touching the same mutex; at planning time the only
  listeners are in the webview).

## Maintenance notes

- Any NEW lock site inside an AppKit callback (tracking area, click
  monitor, future region routing) must use the poison-tolerant idiom —
  reviewer should grep for `.lock().unwrap()` in diffs touching
  `click.rs`/`hover.rs`/the monitor closures.
- If a third writer for tab selection ever appears, the under-lock emit
  keeps ordering correct with no further change.
- Deferred deliberately: `catch_unwind` at the FFI boundary (see Out of
  scope) — re-raise only with a concrete panic report.
