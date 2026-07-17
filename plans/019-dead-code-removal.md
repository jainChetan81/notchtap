# Plan 019: Remove dead machinery — the presentation-mode frontend channel, the never-flipped polling gates, the no-op dispatch, and scaffold leftovers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> This plan DELETES code; the STOP conditions are strict — when in doubt,
> stop. When done, update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src/presentationMode.ts src/presentationMode.test.ts src-tauri/src/lib.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/event.rs src-tauri/src/error.rs src-tauri/src/http.rs index.html`
> On any change, re-verify each deletion target still matches the
> evidence below (especially: still zero non-test references).

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED — deletions; each sub-part has its own verify gate
- **Depends on**: plan 004 (docs pass) recommended first — CONTEXT.md's
  "Polling Pause" entry needs updating alongside part B
- **Category**: tech-debt
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

Four clusters of code that cannot run (or do nothing) in production make
every reader — human or agent — reason about machinery that doesn't
exist:

- **A. The presentation-mode frontend channel is dead on both sides of
  IPC**: `usePresentationMode` is imported ONLY by its own test; yet the
  rust core still evals `window.__NOTCHTAP_MODE__` and emits
  `presentation-mode` on every page load. (Rust-side `mode` remains
  genuinely used for window positioning — only the frontend delivery is
  dead.)
- **B. The polling-pause gates can never fire**: `espn_active`/
  `rss_active` are `AtomicBool::new(true)` with zero `.store()` calls in
  the codebase (the v6 comment says "set once at boot … never flipped
  again") — so `PauseGate::tick`'s pause/re-baseline branches in both
  poller loops are unreachable, yet each loop threads the Arc and carries
  ~10 lines of re-baseline handling.
- **C. `event::dispatch` is a no-op acceptance gate**: it matches all
  four `EventType` variants and returns `Ok(())` unconditionally; its
  error variant `EventError::UnknownType` is `#[allow(dead_code)]` and
  unconstructable (serde already rejects unknown types). The hot `/notify`
  handler pays an `event.clone()` for a check that cannot fail.
- **D. Scaffold leftovers**: `index.html` still titled
  "Tauri + React + Typescript" with a `/vite.svg` favicon; orphaned
  template svgs.

## Current state

**A.** `src/presentationMode.ts` (47 lines) exports `usePresentationMode`;
`rg -n "presentationMode|usePresentationMode" src` shows the only
importers are `src/presentationMode.test.ts` and a prose mention in a
`useSlotState.ts` comment. Rust side, `src-tauri/src/lib.rs:313-321`
(inside `on_page_load`):

```rust
{
    use tauri::Emitter;
    let mode_str = match mode {
        presentation::Mode::Notch => "notch",
        presentation::Mode::Hud => "hud",
    };
    let _ = webview.eval(format!("window.__NOTCHTAP_MODE__ = '{mode_str}';"));
    let _ = webview.emit("presentation-mode", PresentationModePayload { mode });
}
```

(`PresentationModePayload` is defined somewhere in lib.rs — find with
`rg -n "PresentationModePayload" src-tauri/src`.) The `mode` value itself
is ALSO used for window positioning (`position_window` calls) — that use
stays.

**B.** `src-tauri/src/lib.rs:260-261`:

```rust
let espn_active = espn_enabled.then(|| Arc::new(AtomicBool::new(true)));
let rss_active = rss_enabled.then(|| Arc::new(AtomicBool::new(true)));
```

`rg -n "\.store\(" src-tauri/src` → zero hits. Both spawn fns take
`active: Arc<AtomicBool>`; both loops call
`gate.tick(active.load(Ordering::Relaxed))` and have an
`if t.rebaseline { … continue; }` branch (`poller.rs:597-606`,
`rss_poller.rs:443-452`). `PauseGate` lives in `poller.rs` (~lines
502-527) with its own tests.

**C.** `src-tauri/src/event.rs:163-172`:

```rust
pub fn dispatch(event: Event) -> Result<(), EventError> {
    match event.event_type {
        EventType::Generic
        | EventType::ScoreUpdate
        | EventType::MatchState
        | EventType::NewsItem => Ok(()),
    }
}
```

Sole production caller: `src-tauri/src/http.rs:~143`
(`dispatch(event.clone()).map_err(HttpError::Event)?`).
`src-tauri/src/error.rs:10-14` has `EventError::UnknownType` marked
`#[allow(dead_code)]`; `EventError::MissingField` is genuinely used
(http.rs title/body checks) and STAYS.

**D.** `index.html:5,7`: `<link rel="icon" … href="/vite.svg" />`,
`<title>Tauri + React + Typescript</title>`. Check orphans:
`rg -n "tauri.svg|react.svg|vite.svg" src index.html settings.html` —
delete `public/vite.svg`, `public/tauri.svg`, `src/assets/react.svg` only
if nothing references them (vite serves `public/` by URL, so grep both
html files and all of `src/`).

Decision context the executor must honor: CONTEXT.md's **Polling Pause**
entry documents the state as boot-set ("v6: no longer tray-toggleable —
set once at boot"). Part B's deletion is consistent with that decision
(the *gate machinery* for runtime flipping is what's dead, not the
boot-time enable/disable, which lives in whether the poller spawns at
all). `PauseGate` itself may still be judged worth keeping if a runtime
toggle is plausibly coming back — this plan REMOVES it fully; the STOP
conditions cover discovering contrary intent.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cargo test` (from `src-tauri/`) | all pass |
| Frontend suite | `npx vitest run && npx tsc --noEmit && npx vite build` | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- Delete: `src/presentationMode.ts`, `src/presentationMode.test.ts`
- `src-tauri/src/lib.rs` (the eval/emit block A; `PresentationModePayload`
  if now unused; the two `*_active` lines and spawn-arg threading)
- `src-tauri/src/poller.rs` (PauseGate + its tests + loop branches;
  `active` param), `src-tauri/src/rss_poller.rs` (same + its
  `use crate::poller::{Backoff, PauseGate}` import trim)
- `src-tauri/src/event.rs` (dispatch + its tests), `src-tauri/src/error.rs`
  (UnknownType variant), `src-tauri/src/http.rs` (the dispatch call +
  `HttpError::Event` arm if now unconstructable — check its status-code
  mapping table)
- `index.html`, `public/vite.svg`, `public/tauri.svg`,
  `src/assets/react.svg` (orphans only)
- `CONTEXT.md` **Polling Pause** entry (one clause noting the runtime
  gate machinery was removed 2026-07-17; per-source control = spawn-time
  only) — coordinate with plan 004
- `docs/TESTING_STRATEGY.md` §0 (counts drop: presentationMode 4 frontend
  tests, PauseGate ~3 rust tests, dispatch tests) 
- `plans/README.md` (status row)

**Out of scope**:
- `presentation.rs` (the rust Mode detection — fully alive), the
  positioning code, `Backoff` (alive, keep), `EventError::MissingField`,
  the `capabilities/default.json` (even though `presentation-mode` emit
  disappears, the capability file must stay byte-identical per the
  locked rule — emitting permissions are rust-side anyway; do NOT touch
  it).

## Git workflow

- Current branch; one commit per part (A/B/C/D) so each deletion is
  independently revertable:
  `overlay: drop dead presentation-mode frontend channel`, 
  `pollers: remove unreachable runtime pause gate (boot-set since v6)`,
  `event: delete no-op dispatch gate`, `scaffold: retitle overlay page, drop template svgs`.
- Do NOT push.

## Steps

### Step A: Presentation-mode channel

Re-verify deadness: `rg -n "usePresentationMode" src --glob '!*.test.*'`
→ zero hits (STOP if any). Delete `src/presentationMode.ts` +
`src/presentationMode.test.ts`. In lib.rs, delete the eval/emit block
quoted above and `PresentationModePayload` if `rg` finds no other use.
Keep the `useSlotState.ts` comment that *mentions* presentationMode only
if it still reads sensibly — otherwise trim the reference (comment-only
edit).

**Verify**: `npx vitest run` (4 fewer tests, all pass), `npx tsc --noEmit`, `cargo test`, clippy — all clean.

### Step B: Polling gates

In `lib.rs`, delete the two `*_active` lines and stop passing `active`
to `spawn_espn_poller`/`spawn_rss_poller`. In both pollers: remove the
`active` param, the `gate` variable, the `let t = gate.tick(...)` +
rebaseline branch (the loop body starts at the backoff check), and the
now-unused imports (`AtomicBool`, `Ordering`, `PauseGate`). Delete
`PauseGate` + its tests from poller.rs. Update CONTEXT.md's Polling Pause
entry as scoped above.

**Verify**: `cargo test` all pass; `cargo clippy --all-targets -- -D warnings` → exit 0 (clippy will catch leftover unused imports); `rg -n "PauseGate|AtomicBool" src-tauri/src` → zero hits.

### Step C: dispatch

Delete `dispatch` + its tests from event.rs; delete the
`dispatch(event.clone()).map_err(HttpError::Event)?` line from http.rs
(and the `event.clone()` with it); remove `EventError::UnknownType` from
error.rs. Then check `HttpError::Event`: if `EventError::MissingField`
still flows through it (it does — title/body validation), the variant
STAYS; only confirm the match arms still compile. The spec's
error→status table (`docs/V3_6_TECHNICAL_SPEC.md` §11 or nearby) may
reference UnknownType — if so, update that row (working-draft spec).

**Verify**: `cargo test` all pass (http suite exercises every status code — 26 tests); `rg -n "dispatch\|UnknownType" src-tauri/src` → zero hits.

### Step D: Scaffold

`index.html`: title → `notchtap`, drop or replace the `/vite.svg` favicon
line. Delete the three svgs IF the orphan grep (Current state D) shows
zero references — list the grep output in your report.

**Verify**: `npx vite build` → exit 0; `npm run tauri dev` boots (or hand to operator).

### Step E: Counts

`docs/TESTING_STRATEGY.md` §0: subtract the deleted tests per module
(frontend −4; poller −(PauseGate tests count — count them before
deleting); event −(dispatch tests)). Also §4.5 (frontend render-state
section) may reference presentationMode tests — point it at the git
history like other superseded entries do.

**Verify**: `cargo test 2>&1 | grep "test result"` totals match what §0 now claims.

## Test plan

Deletions only — no new tests. The full suite (rust + frontend + build)
green after each part is the gate. Count integrity per Step E.

## Done criteria

- [ ] `rg -n "presentationMode|PresentationModePayload|PauseGate|fn dispatch|UnknownType" src src-tauri/src` → zero hits
- [ ] `grep -c "Tauri + React" index.html` → 0
- [ ] `cargo test`, clippy, fmt, `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] `git diff d40445e..HEAD -- src-tauri/capabilities/default.json` → empty (locked rule)
- [ ] §0 counts reconciled; CONTEXT.md Polling Pause updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- Any deadness re-verify grep returns a production reference — the audit
  evidence has drifted; report.
- You find a doc/comment stating a runtime polling toggle is planned to
  RETURN (e.g. a settings hot-toggle design) — Part B would then be
  wrong; skip B, do the rest, report.
- Removing the dispatch call changes any HTTP status behavior (an http
  test fails) — report; the premise "cannot fail" would be wrong.
- The earlier session's plan 001 (wire-skip hotkey) or 002 (animation
  previews) landed and touches these regions — reconcile by reading
  before deleting.

## Maintenance notes

- If notch-specific card styling is ever wanted, the frontend channel
  deleted in Step A is the thing to REBUILD (rust still knows the mode) —
  the git history of `presentationMode.ts` is the reference
  implementation; this deletion is reversible by revert.
- If a runtime polling toggle returns (settings hot-path), `PauseGate` +
  its tests are likewise one `git revert` away — note the commit hash in
  CONTEXT.md's updated entry.
