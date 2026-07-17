# Plan 009: Validate live `slot-state` event payloads and pin the event name across the rust↔TS seam

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src/useSlotState.ts src/useSlotState.test.ts src-tauri/src/event.rs src-tauri/src/http.rs`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug / tests
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

The overlay's single input is the `slot-state` tauri event. The frontend
has a careful per-field validator, `isValidSlotState` — but it is applied
only to the eval-planted startup global, NOT to live event payloads, which
are stored raw. The file's own comment claims the payload "is validated
rather than trusted blindly", which is true for one of the two paths. A
rust-side field rename/addition (exactly the drift the code's own comments
warn about) arriving via emit renders `undefined` fields instead of
falling back to empty. The `listen()` registration promise also has no
`.catch` — if registration rejects, the overlay silently freezes on its
initial state forever, with nothing in any log.

Separately, nothing pins the event-name string across the seam: rust emits
`"slot-state"` (`event.rs`) and TS listens for `"slot-state"`
(`useSlotState.ts`) as two unrelated literals. A rename on either side
compiles clean, passes all 272 tests, and ships an overlay that never
updates.

## Current state

`src/useSlotState.ts:90-107` (the hook):

```ts
export function useSlotState(): SlotState {
  const [slot, setSlot] = useState<SlotState>(initialSlotState);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<SlotState>("slot-state", ({ payload }) => setSlot(payload)).then((fn) => {
      if (unmounted) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return slot;
}
```

`isValidSlotState` (same file, lines ~60-88) validates every field of a
"showing" payload (id/title/body strings, expanded boolean, eventType/
priority/signal against the const arrays, source/category/publishedAtMs/
link null-or-typed). `initialSlotState()` (lines ~84-88) is its only
caller.

`src-tauri/src/event.rs:174-181` (the single emit path):

```rust
pub fn emit_slot_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: SlotState) {
    use tauri::Emitter;
    if let Err(e) = app.emit("slot-state", &state) {
        tracing::error!("failed to emit slot-state: {e}");
    }
}
```

Existing tests to model on:
- `src/useSlotState.test.ts` — 14 tests; the malformed-payload suite
  (lines ~141-207) plants garbage in `window.__NOTCHTAP_SLOT_STATE__` and
  asserts fallback to empty. Its mock/event plumbing shows how the tauri
  `listen` is mocked in this repo (read the top of the file before
  writing).
- `src-tauri/src/http.rs:~220` uses `tauri::test::mock_app()` — the mock
  runtime is available (Cargo.toml dev-deps enable tauri's `test`
  feature) if you choose the rust-side listener-drain approach in Step 3.

Conventions: frontend tests are vitest + @testing-library; rust event
constants would live in `event.rs` next to `emit_slot_state`. Counts live
ONLY in `docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` (repo root) | all pass |
| Typecheck | `npx tsc --noEmit` (repo root) | exit 0 |
| Rust tests | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src/useSlotState.ts`, `src/useSlotState.test.ts`
- `src-tauri/src/event.rs` (event-name constant), and the one string
  literal in `src-tauri/src/lib.rs`'s page-load block ONLY if it also
  hardcodes the name (check: `rg -n '"slot-state"' src-tauri/src`)
- `docs/TESTING_STRATEGY.md` §0 counts
- `plans/README.md` (status row)

**Out of scope**:
- The eval-planted global / double-shield mechanism in `lib.rs` — working
  as designed (plan 014 tests its escaping separately).
- `presentationMode.ts` — a different (dead) channel; plan 019 owns it.
- Changing the wire payload shape in any way.

## Git workflow

- Current branch; commit style: `overlay: validate live slot-state payloads, log dead listener, pin event name`.
- Do NOT push.

## Steps

### Step 1 (tests first): malformed-payload-via-event tests

In `src/useSlotState.test.ts`, add two tests mirroring the existing
global-path malformed suite but delivering the payload through the mocked
`listen` callback instead of the global:
1. a well-tagged-but-incomplete object (`{state:"showing", id:"x"}` and
   nothing else) delivered via the event → hook returns
   `{state:"empty"}`;
2. a fully valid `showing` payload delivered via the event → hook returns
   it unchanged (guards against over-strict validation — reuse the valid
   fixture the existing tests use, including the news/null-metadata
   variant if one exists).

**Verify**: `npx vitest run` → the new incomplete-object test FAILS (current code stores it raw), the valid-payload test passes.

### Step 2: Route the listener through the validator + catch registration failure

In `useSlotState.ts`:

```ts
listen<unknown>("slot-state", ({ payload }) =>
  setSlot(isValidSlotState(payload) ? payload : { state: "empty" }),
)
  .then((fn) => { /* unchanged unlisten bookkeeping */ })
  .catch((error) => {
    // A dead listener means a permanently frozen overlay — make it loud
    // in the webview console since the overlay can't write to the file log.
    console.error("slot-state listener failed to register", error);
  });
```

Update the file's header comment so the "validated rather than trusted"
claim names BOTH paths. Keep the fallback exactly `{ state: "empty" }` —
same as the global path.

**Verify**: `npx vitest run` → all pass, including Step 1's tests. `npx tsc --noEmit` → exit 0.

### Step 3: Pin the seam

In `src-tauri/src/event.rs`, hoist the name:

```rust
/// The one event channel into the overlay — the frontend listens for
/// exactly this string (src/useSlotState.ts). Change both together.
pub const SLOT_STATE_EVENT: &str = "slot-state";
```

Use it in `emit_slot_state` (and in any other rust-side `"slot-state"`
literal found by `rg -n '"slot-state"' src-tauri/src`). Then add a rust
test that pins the value: `assert_eq!(SLOT_STATE_EVENT, "slot-state");`
with a comment pointing at `useSlotState.ts` — a rename now fails a test
naming the file that must change in lockstep. Optionally (only if quick):
in an existing `http.rs` mock-app test, register
`app.listen(SLOT_STATE_EVENT, ...)` and assert a driven promotion
delivers a payload — if the mock runtime does not deliver events to
listeners, skip this without ceremony (the constant + pin test already
close the gap; note the skip in your report).

**Verify**: `cargo test` → all pass; `rg -n '"slot-state"' src-tauri/src` → only the `const` definition line (and the pin test's expected literal).

### Step 4: Counts

`docs/TESTING_STRATEGY.md` §0: frontend +2, event +1 (adjust to what you
actually added).

**Verify**: full gates — `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `npx tsc --noEmit`, `npx vitest run`, `npx vite build` → all clean.

## Test plan

- 2 new vitest cases in `src/useSlotState.test.ts` (event-path malformed →
  empty; event-path valid → stored), modeled on the existing global-path
  malformed suite in the same file.
- 1 rust pin test for `SLOT_STATE_EVENT` in `event.rs`'s test module.
- Optional listener-drain integration test in `http.rs` (skip allowed —
  see Step 3).

## Done criteria

- [ ] `grep -c "isValidSlotState(payload)" src/useSlotState.ts` → 1
- [ ] `grep -c "\.catch" src/useSlotState.ts` → ≥1
- [ ] `grep -c "SLOT_STATE_EVENT" src-tauri/src/event.rs` → ≥2 (const + use)
- [ ] `npx vitest run` exits 0 with the 2 new tests
- [ ] `cargo test` exits 0 with the pin test
- [ ] All lint/format/typecheck gates exit 0
- [ ] `docs/TESTING_STRATEGY.md` §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- The `listen` mocking pattern in `useSlotState.test.ts` doesn't support
  delivering a payload to the registered callback (read the existing
  tests first) — report the mocking gap rather than inventing a new mock
  layer.
- You find a comment or review-log entry stating the event-path
  non-validation was a *deliberate* decision (the audit found commit
  `0ab0da4` "shallow slot-state validation" nearby but no such statement)
  — surface it before changing behavior.

## Maintenance notes

- Any new field on `SlotState::Showing` (rust) now has THREE lockstep
  sites: the rust struct, `isValidSlotState`, and the `EVENT_TYPES`-style
  const arrays — the existing regression-test comment at the top of
  `useSlotState.ts` explains the drill; reviewers should check all three
  on any wire change.
- If plan 015 (heartbeat rework) changes emission cadence, these
  validators are unaffected — shape-only.
