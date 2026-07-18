# Plan 019: Remove dead machinery — the presentation-mode frontend channel, the never-flipped polling gates, the no-op dispatch, and scaffold leftovers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> This plan DELETES code; the STOP conditions are strict — when in doubt,
> stop. When done, update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2f3299..HEAD -- src/presentationMode.ts src/presentationMode.test.ts src/useSlotState.ts src/useSlotState.test.ts src-tauri/src/lib.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/event.rs src-tauri/src/error.rs src-tauri/src/http.rs src-tauri/src/settings.rs index.html`
> On any change, re-verify each deletion target still matches the
> evidence below (especially: still zero non-test references). Also run
> `git status` — this repo hosts concurrent agent sessions; if any file
> above is already modified in the working tree, STOP and report rather
> than deleting around someone's in-flight edits.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED — deletions; each sub-part has its own verify gate
- **Depends on**: none — 004 (docs pass) is DONE; read what it wrote in
  CONTEXT.md before editing the "Polling Pause" entry in Part B
- **Category**: tech-debt
- **Planned at**: commit `d40445e`, 2026-07-17; **review-plan pass
  2026-07-18 at `cd64e19`** — the earlier `b43a7ca` baseline was stale
  (plan-014 eval-splice + logging merges moved every rust excerpt:
  lib.rs +362 lines). All excerpts and line refs below re-verified
  against `cd64e19`; deadness re-confirmed (`usePresentationMode`
  imported only by its test, zero `.store(` calls, dispatch still the
  same no-op at event.rs:163). New facts folded in: the `*_active`
  Options double as the spawn-gate condition in lib.rs (Step B now
  specifies the replacement), a new lib.rs slot-state comment references
  the presentation-mode block, and the UnknownType spec reference lives
  only in the archived V1 spec (no live-spec edit needed). **Reconcile
pass 2026-07-18 at `f2f3299`** (pre-execute): biome (plan 016) reformatted
the four frontend files — cosmetic only (import order, brace/object
wrapping); all rust deletion targets unchanged since `cd64e19`,
presentationMode still dead, only `useSlotState.test.ts` comment shifted
150→157.

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
`rg -n "usePresentationMode|__NOTCHTAP_MODE__" src` shows the only
consumers are `src/presentationMode.ts` and `src/presentationMode.test.ts`
themselves. Two prose comments reference the file and must be reworded
(not kept verbatim — the done-criteria grep would hit them):
`src/useSlotState.ts:54` ("the same race presentationMode.ts was already
built for") and `src/useSlotState.test.ts:157` ("mirrors
presentationMode's"). Rust side, `src-tauri/src/lib.rs:400-408` (inside
`on_page_load`, preceded by a "double-shielded" explainer comment at
394-399 that goes with it):

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

`PresentationModePayload` is at `lib.rs:605`. NOTE (new since the plan
was first written): the slot-state block right below (lib.rs:410-416)
carries a comment saying its shield mirrors "the presentation-mode
shield above" — after deleting the block, reword that comment so it
stands alone (the slot-state double-shield itself is ALIVE and stays;
only its cross-reference loses its antecedent). The `mode` value itself
is ALSO used for window positioning (`position_window` calls) — that use
stays.

**B.** `src-tauri/src/lib.rs:344-345` (with a v6 decision comment
attached at ~336-343 — trim its gate-specific sentences when deleting):

```rust
let espn_active = espn_enabled.then(|| Arc::new(AtomicBool::new(true)));
let rss_active = rss_enabled.then(|| Arc::new(AtomicBool::new(true)));
```

**Load-bearing subtlety**: the Options above are ALSO the spawn
condition — lib.rs:352 / lib.rs:365 read
`if let Some(espn_active) = espn_active { poller::spawn_espn_poller(…,
espn_active); }` (same for rss). Deleting the two lines naively would
delete the "only spawn when enabled" gate with them. The replacement is
`if espn_enabled { … }` / `if rss_enabled { … }` with the trailing
`active` argument dropped — the config-gating comments at 348-351 stay
true and stay put. Also remove the now-unused
`use std::sync::atomic::AtomicBool;` at `lib.rs:17`.

`rg -n "\.store\(" src-tauri/src` → zero hits (re-verified 2026-07-18).
Both spawn fns take `active: Arc<AtomicBool>` (`poller.rs:596`,
`rss_poller.rs:452`); both loops call
`gate.tick(active.load(Ordering::Relaxed))` and have a
`rebaseline { … continue; }` branch (`poller.rs:613-625`,
`rss_poller.rs:469-477`). `PauseGate` lives in `poller.rs:498-527`
(including its explainer comment) with 3 tests at ~poller.rs:960-1010
(`gate_*` test fns). Beware when trimming imports:
`rss_poller.rs:348-349` uses `std::cmp::Ordering` — only the
`std::sync::atomic::{AtomicBool, Ordering}` import (line 2 in both
pollers) goes.

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

(The live function body now carries a comment about `signal` being
presentation-only — it dies with the function.) There are **TWO**
production callers (an earlier version of this plan wrongly said one —
corrected 2026-07-18 after the executor found the second):
1. `src-tauri/src/http.rs:183`
   (`dispatch(event.clone()).map_err(HttpError::Event)?`); `dispatch`
   imported at `http.rs:17`; the doc comment at `http.rs:60-62`
   ("dispatch is the caller's responsibility …") must be reworded once
   the concept is gone.
2. `src-tauri/src/settings.rs:684`
   (`dispatch(event.clone()).map_err(|e| e.to_string())?;`) inside
   `send_test_notification`; `dispatch` imported at `settings.rs:22`.
   Structurally identical to the http.rs site — the `event.clone()`
   exists ONLY to feed `dispatch`; the real `event` is moved into
   `http::enqueue_and_emit(..., event, ...)` at `settings.rs:692`
   directly after, so dropping the dispatch line drops the clone with it
   and `event` flows unchanged into the enqueue. (Landed in `efa1bd2`,
   v5.1 per-source test notifications — predates this plan; a genuine
   original-audit miss, not concurrent drift.) Two dispatch tests in event.rs:
`generic_event_dispatches_ok` (~211) and `dispatch_accepts_all_variants`
(~257). `src-tauri/src/error.rs:11-15` has `EventError::UnknownType`
marked `#[allow(dead_code)]` with the comment "unconstructable until v2
adds event variants; the variant exists now so the §11 error→status
table is complete from day one" — v2 shipped and serde still rejects
unknown types before dispatch, so that rationale is resolved, not
contradicted, by deleting it. `EventError::MissingField` is genuinely
used (http.rs:157,160 title/body checks) and STAYS, so `HttpError::Event`
and its 400 mapping (http.rs:224) STAY. Spec check (done 2026-07-18):
`UnknownType` appears in the docs only in
`docs/archive/V1_TECHNICAL_SPEC.md` — archived, out of scope, leave it;
no live spec edit is needed.

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
| Rust suite | `cargo test --locked` (from `src-tauri/`) | all pass |
| Frontend suite | `npx vitest run && npx tsc --noEmit && npx biome ci . && npx vite build` | all pass |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

(`--locked` matches CI. If `cargo` is not found in a non-interactive
shell on this machine, prefix `PATH="$HOME/.cargo/bin:$PATH"`.)

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
  mapping table), `src-tauri/src/settings.rs` (the SECOND dispatch call
  at :684 + its `dispatch` import at :22 — see Current state C)
- `src/useSlotState.ts`, `src/useSlotState.test.ts` (comment-only
  rewording — see Current state A; no logic changes in either)
- `index.html`, `public/vite.svg`, `public/tauri.svg`,
  `src/assets/react.svg` (orphans only)
- `CONTEXT.md` **Polling Pause** entry (one clause noting the runtime
  gate machinery was removed, dated the day you execute; per-source
  control = spawn-time only) — plan 004's docs pass already rewrote this
  file; read the entry before editing it
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
- The slot-state double-shield block in lib.rs (~410 onward) and
  `escape_for_eval_splice` — alive, tested, and adjacent to Step A's
  deletion; only the cross-reference comment gets reworded.
- `docs/archive/**` (the V1 spec's UnknownType rows are historical
  record — never edit archives).

## Git workflow

- Current branch; one commit per part (A/B/C/D) so each deletion is
  independently revertable:
  `overlay: drop dead presentation-mode frontend channel`, 
  `pollers: remove unreachable runtime pause gate (boot-set since v6)`,
  `event: delete no-op dispatch gate`, `scaffold: retitle overlay page, drop template svgs`.
- **Stage in-scope files by name** (`git add <files>` per part) — never
  `git add -A`/`git add .`; concurrent agent sessions share this
  working tree, so re-check `git status` right before each commit and
  leave unrelated modifications alone.
- Do NOT push.

## Steps

### Step A: Presentation-mode channel

Re-verify deadness: `rg -n "usePresentationMode" src --glob '!*.test.*'`
→ hits only in `src/presentationMode.ts` itself (STOP on any other
file). Delete `src/presentationMode.ts` + `src/presentationMode.test.ts`.
In lib.rs, delete the eval/emit block quoted above WITH its explainer
comment (lib.rs:394-408), and `PresentationModePayload` (lib.rs:605) if
`rg` finds no other use. Then reword the three stranded comments so no
identifier of the deleted code survives (the done-criteria grep checks
this): `src/useSlotState.ts:54` and `src/useSlotState.test.ts:157` —
e.g. "the mode-delivery hook removed in plan 019 (see git history)" —
and lib.rs's slot-state comment (~410-416), which currently mirrors
"the presentation-mode shield above"; make it describe the slot-state
double-shield on its own terms. Comment-only edits; touch no logic.

**Verify**: `npx vitest run` (4 fewer tests, all pass), `npx tsc --noEmit`, `npx biome ci .`, `cargo test --locked`, clippy — all clean.

### Step B: Polling gates

In `lib.rs`: delete the two `*_active` lines (344-345, trimming the
gate-specific sentences of the comment attached to them) and convert the
spawn conditions from `if let Some(espn_active) = espn_active { … }` /
`if let Some(rss_active) = rss_active { … }` (lib.rs:352, 365) to
`if espn_enabled { … }` / `if rss_enabled { … }`, dropping the trailing
`active` argument from each spawn call — the "only spawn when enabled"
behavior must be preserved exactly. Remove `use
std::sync::atomic::AtomicBool;` (lib.rs:17). In both pollers: remove the
`active` param, the `gate` variable, the `gate.tick(...)` + rebaseline
branch (the loop body then starts at the backoff check), and the
now-unused `use std::sync::atomic::{AtomicBool, Ordering};` (line 2 in
each — do NOT touch rss_poller.rs's `std::cmp::Ordering` uses at
348-349). Delete `PauseGate` + its 3 tests from poller.rs and the
`PauseGate` half of rss_poller.rs:15's import. Update CONTEXT.md's
Polling Pause entry as scoped above.

**Verify**: `cargo test --locked` all pass; `cargo clippy --locked --all-targets -- -D warnings` → exit 0 (clippy will catch leftover unused imports); `rg -n "PauseGate|AtomicBool" src-tauri/src` → zero hits.

### Step C: dispatch

Delete `dispatch` + its two tests (`generic_event_dispatches_ok`,
`dispatch_accepts_all_variants`) from event.rs. Then remove BOTH
production call sites (STOP if `rg -n "\bdispatch\(" src-tauri/src` shows
any call site not covered here):
- **http.rs**: delete the `dispatch(event.clone()).map_err(HttpError::Event)?`
  line at 183 (and the `event.clone()` with it), drop `dispatch` from
  the http.rs:17 import, and reword the doc comment at http.rs:60-62
  ("dispatch is the caller's responsibility …") to describe what's still
  true (malformed `/notify` requests 400 via the title/body
  `MissingField` checks before entering the queue).
- **settings.rs**: delete the `dispatch(event.clone()).map_err(|e| e.to_string())?;`
  line at 684 (the clone goes with it — `event` then flows unchanged
  into `http::enqueue_and_emit(..., event, ...)` at 692), and drop
  `dispatch` from the `settings.rs:22` import (leave the rest of that
  `use crate::event::{…}` list intact — the other names stay used).
Remove `EventError::UnknownType` (+ its `#[allow(dead_code)]` and
comment) from error.rs. `HttpError::Event`
STAYS — `MissingField` still flows through it (http.rs:157,160) to the
400 mapping (http.rs:224). No spec edit: `UnknownType` appears only in
the archived V1 spec, which must not be touched.

**Verify**: `cargo test --locked` all pass (the http suite asserts every status code); `rg -n "fn dispatch|UnknownType" src-tauri/src` → zero hits (note: the pattern's `|` is unescaped — an escaped `\|` would match nothing and pass vacuously).

### Step D: Scaffold

`index.html`: title → `notchtap`, drop or replace the `/vite.svg` favicon
line. Delete the three svgs IF the orphan grep (Current state D) shows
zero references — list the grep output in your report.

**Verify**: `npx vite build` → exit 0; `npm run tauri dev` boots (or hand to operator).

### Step E: Counts

`docs/TESTING_STRATEGY.md` §0: subtract the deleted tests per module
(frontend −4 presentationMode tests; poller −3 PauseGate tests; event
−2 dispatch tests — re-count from your own diff before editing, these
were counted at `cd64e19`). Also §4.5 (frontend render-state
section) may reference presentationMode tests — point it at the git
history like other superseded entries do.

**Verify**: `cargo test 2>&1 | grep "test result"` totals match what §0 now claims.

## Test plan

Deletions only — no new tests. The full suite (rust + frontend + build)
green after each part is the gate. Count integrity per Step E.

## Done criteria

- [ ] `rg -n "presentationMode|PresentationModePayload|PauseGate|fn dispatch|UnknownType" src src-tauri/src` → zero hits (requires the comment rewording in Step A — the two useSlotState comments count)
- [ ] `grep -c "Tauri + React" index.html` prints 0 (grep exits 1 on zero matches — that IS the pass)
- [ ] `cargo test --locked`, clippy, fmt, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` all exit 0
- [ ] `git diff f2f3299..HEAD -- src-tauri/capabilities/default.json` → empty (locked rule)
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
- Any gate fails BEFORE your first deletion (pre-existing red baseline —
  plausible in this shared working tree): report which gate and its
  output; do not fix unrelated code and do not start deleting on a red
  baseline.

## Maintenance notes

- If notch-specific card styling is ever wanted, the frontend channel
  deleted in Step A is the thing to REBUILD (rust still knows the mode) —
  the git history of `presentationMode.ts` is the reference
  implementation; this deletion is reversible by revert.
- If a runtime polling toggle returns (settings hot-path), `PauseGate` +
  its tests are likewise one `git revert` away — note the commit hash in
  CONTEXT.md's updated entry.
