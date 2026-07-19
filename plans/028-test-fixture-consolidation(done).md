# Plan 028: One shared Event test builder (rust) and one shared listen-mock harness (frontend)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src-tauri/src src/App.test.tsx src/useSlotState.test.ts src/useStatusState.test.ts`
> If in-scope files changed since this plan was written, re-run the Step-1
> inventory rather than trusting this plan's counts; the *approach* is
> drift-proof, the counts are not.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW (test-only; the suites themselves are the safety net)
- **Depends on**: both original soft deps (036, 025) are now DONE
  (036 `9373b84`, 025 `920d4e4`, both on master as of `12eaefe`
  2026-07-19) — this plan is fully unblocked on its dependencies.
  **Active coordination hazard**: plan 037 ("The Engine", L) is
  BLOCKED but has a live partial branch `exec/037-engine` that already
  rewrote `queue.rs` (clock-agnostic enqueue) and added `engine.rs`;
  once 035 lands it will be retargeted and re-executed against a fresh
  master, touching the same `queue.rs` test surface this plan sweeps.
  Do NOT run this plan while 037's re-execution is in flight — check
  037's status row first; if it has moved off BLOCKED, coordinate
  ordering with the operator rather than racing it.
- **Category**: tests / tech-debt
- **Planned at**: commit `a58f115`, 2026-07-18
- **Reviewed**: 2026-07-18 at `4281d2c` (review-plan pass) — Event
  struct, both frontend harness excerpts, queue.rs helper cluster
  (429–483) and beforeEach resets all re-verified verbatim. Known
  drift folded in: plan 022's `mod proptest_queue` (queue.rs ~1429)
  and http.rs burst tests landed after planning — the proptest
  module's generated-values `build_event` is now explicitly out of
  scope, and the drift-check hit on those two files documented as
  expected, not a STOP
- **Re-reviewed**: 2026-07-19 at `7430b4b` — plans 024/032/033/034/036
  all landed since the first review, so the drift check now hits
  `queue.rs`, `http.rs`, `lib.rs`, `poller.rs`, `event.rs`, the new
  `status.rs`, and both frontend test files: ALL expected, none a
  STOP. Folded in: (a) a THIRD copy of the frontend harness landed in
  `src/useStatusState.test.ts` (plan 034) — now in scope; (b)
  `status.rs` has a new in-scope fixture helper (`generic_event`,
  ~line 178); (c) queue.rs helper cluster moved to ~537–591,
  `mod proptest_queue` to ~1812, its `build_event` to ~1928 (exclusion
  unchanged); (d) Event struct itself re-verified UNCHANGED at
  event.rs:5-19 (the event.rs drift is `SlotState`, not `Event`);
  (e) baseline counts: `7430b4b` had cargo 276+3 doc-tests / vitest 97,
  but plan 025 has since landed (`920d4e4`, adds `net`'s 4 rust tests
  → cargo 280+3; vitest still 97) — record your own fresh baseline at
  the commit you actually start from, do not trust either number

## Why this matters

Roughly 25 test call-sites across 9 rust modules (18-across-8 at
planning time; plans 033/034 added more, including a whole new
`status.rs` helper) spell out full
`Event { ... }` struct literals, most behind near-identical private
helpers (`fn event(...)`, `make_event`, `build_test_event`,
`recurring_event`, …) that each executor session rolled independently.
Every field added to `Event` (recent history: `origin`, `meta`,
`signal`) forces a shotgun edit across all of them, and the helpers
have already diverged on defaults (some hardcode
`Priority::Medium`, some take it as a param, some hardcode
`OneShot { ttl_secs: 8 }`). The frontend has the parallel problem in
miniature — and it is actively multiplying: `App.test.tsx` and
`useSlotState.test.ts` carry a verbatim copy of the same
`vi.mock("@tauri-apps/api/event")` + `handlers[]` + `emit()` harness,
and plan 034 (landed 2026-07-19-review) pasted a THIRD copy into
`useStatusState.test.ts` — exactly the copy-forward this plan exists
to stop. Consolidating both means the next `Event` field is a
one-file test change, not eight.

## Current state

- The `Event` shape (`src-tauri/src/event.rs:5-20`) — every fixture
  must populate all of:

  ```rust
  pub struct Event {
      pub id: Uuid,
      pub event_type: EventType,
      pub priority: Priority,
      pub rotation: RotationSpec,
      pub topic: Option<String>,
      pub payload: EventPayload,
      #[serde(default)]
      pub meta: EventMeta,
      pub signal: EventSignal,
      pub origin: SourceKind,
  }
  ```

- A representative duplicated literal (`src-tauri/src/lib.rs`, inside
  the heartbeat test):

  ```rust
  let short_lived = Event {
      id: uuid::Uuid::new_v4(),
      event_type: EventType::Generic,
      priority: Priority::Medium,
      rotation: RotationSpec::OneShot { ttl_secs: 1 },
      topic: None,
      payload: EventPayload { title: "t".to_string(), body: "b".to_string() },
      meta: EventMeta::default(),
      signal: EventSignal::Generic,
      origin: SourceKind::Manual,
  };
  ```

- Known helper clusters (line numbers verified at `7430b4b` but still
  re-inventory in Step 1): `queue.rs` (`event`, `recurring_event`,
  `topic_event`, `event_from` at lines 537–591, ~13 literals),
  `settings.rs` (`build_test_event` at 505 + inline, ~5 literals),
  `http.rs` (~4 inline literals), `lib.rs` (~4 test literals),
  `status.rs` (`generic_event` at ~178, ~2 literals — new module from
  plan 034), `notifier.rs` (~2), `poller.rs` tests (~5, but its
  `make_event` at ~297 is PRODUCTION — see below),
  `event.rs`/`rss_poller.rs` test modules (~1–3 each).
- **Known drift, already accounted for**: plans 022/024 added
  `#[cfg(test)] mod proptest_queue` to `queue.rs` (at ~line 1812 as of
  `7430b4b`) and burst tests to `http.rs`; plans 032/033/034/036 then
  touched `queue.rs`, `lib.rs`, `poller.rs`, `event.rs`, added
  `status.rs`, and extended both frontend test files — the drift check
  WILL light up on all of these; that alone is not a STOP (STOP only
  if the excerpts in this section don't match what you find). The
  proptest module has its own `fn build_event(spec: &EnqueueSpec)`
  (~line 1928): that is a *generated-values* mapper, not a
  fixed-default fixture — see the exclusion in Scope.
- **Production constructors are NOT fixtures**: `poller.rs::make_event`
  (~line 284, used by `diff_scoreboard`), the event construction in
  `rss_poller.rs::diff_feed` (~line 320), and the one in `http.rs`'s
  notify handler are production code — they must not be touched. Only
  literals inside `#[cfg(test)] mod tests` blocks are in scope.
- Frontend duplication — the identical harness at the top of ALL THREE
  of `src/App.test.tsx:6-22`, `src/useSlotState.test.ts:6-22`, and
  `src/useStatusState.test.ts:6-22` (the third types its payload
  `unknown` instead of `SlotState` and its hook listens for
  `"status-state"` — same harness, third copy):

  ```ts
  type Handler = (event: { payload: SlotState }) => void;
  const handlers: Handler[] = [];

  vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn((_name: string, handler: Handler) => {
      handlers.push(handler);
      return Promise.resolve(() => {});
    }),
  }));

  function emit(payload: SlotState) {
    act(() => {
      handlers.forEach((handler) => {
        handler({ payload });
      });
    });
  }
  ```

  Caveat this harness hides: `useSlotState`'s real listener registers
  for `"slot-state"`, `App.tsx` also registers an
  `"appearance-changed"` listener, and (since plan 034)
  `useStatusState` registers for `"status-state"` — the current mock
  pushes ALL handlers registered in a file into one array and `emit`
  calls all of them with one payload shape. It happens to work because
  each handler ignores malformed payloads. The shared harness should
  keep handlers **per event name** and `emit(name, payload)` to remove
  that latent trap; adapt all three test files to pass the event name
  (`"slot-state"` in App/useSlotState tests, `"status-state"` in
  useStatusState tests).
- Conventions: rust tests in `#[cfg(test)] mod tests` per file;
  frontend tests in `src/*.test.ts(x)` with vitest + testing-library;
  biome governs `src/**/*.ts(x)`; counts live in
  `docs/TESTING_STRATEGY.md` §0 (this plan should NOT change any count
  — it moves fixtures, it does not add or remove tests).

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Rust tests | `cd src-tauri && cargo test --locked` | same pass count as before this plan |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust fmt | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | same pass count as before |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint | `npx biome ci .` | exit 0 |

Record the exact baseline counts (`cargo test` summary lines, vitest
summary) BEFORE changing anything — "same count after" is the core
done criterion.

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/event.rs` (add the `#[cfg(test)]` fixtures module; its
  own tests may switch to it)
- Test modules (`#[cfg(test)] mod tests` blocks ONLY) in:
  `src-tauri/src/queue.rs`, `http.rs`, `lib.rs`, `settings.rs`,
  `notifier.rs`, `poller.rs`, `rss_poller.rs`, `status.rs`
- `src/test-support/tauriEventMock.ts` (create)
- `src/App.test.tsx`, `src/useSlotState.test.ts`,
  `src/useStatusState.test.ts`
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- ANY production (non-`#[cfg(test)]`) code path — especially
  `poller.rs::make_event` and `diff_feed`'s event construction.
- Test *assertions* and test *logic* — this is a fixture swap; if a
  test's meaning would change, leave that site alone and note it.
- `src/settings/SettingsApp.test.tsx` — it mocks IPC, not the event
  channel; different harness, out of scope.
- `queue.rs`'s `mod proptest_queue` (~line 1429, from plan 022) —
  its `build_event(spec: &EnqueueSpec)` (~line 1545) populates every
  field from proptest-generated values; delegating it to a
  fixed-default builder adds indirection without removing duplication,
  and touching the property-suite semantics is exactly the kind of
  silent meaning-change this plan forbids. Leave the whole module
  alone; only the *example*-test helpers/literals in `mod tests` are
  in scope.
- `docs/TESTING_STRATEGY.md` — counts must not change; nothing to
  update.

## Git workflow

- Branch: `advisor/028-test-fixture-consolidation`.
- Two commits: `tests(rust): shared event fixture builder` and
  `tests(web): shared tauri event-mock harness` (repo style:
  lowercase `area: summary`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Inventory the actual fixture sites

```
cd src-tauri && grep -n "Event {" src/*.rs
```

For each hit, note whether it is inside a `#[cfg(test)] mod tests`
block (in scope) or production code (out of scope). Write the list
into your working notes; it drives Steps 3's mechanical sweep.

**Verify**: the in-scope list covers ≥ 15 sites across ≥ 6 files (if it
is dramatically smaller, the codebase drifted — STOP).

### Step 2: Add the shared builder in `event.rs`

At the bottom of `src-tauri/src/event.rs` (before or after the existing
`mod tests`), add:

```rust
/// Shared test fixture builder (plan 028): the ONE place tests build
/// `Event`s, so a new field is a one-file test change. Production code
/// must never use this — `#[cfg(test)]` enforces it.
#[cfg(test)]
pub(crate) mod test_fixtures {
    use super::*;

    pub(crate) fn event(title: &str) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    pub(crate) fn with_priority(mut e: Event, priority: Priority) -> Event {
        e.priority = priority;
        e
    }

    pub(crate) fn with_rotation(mut e: Event, rotation: RotationSpec) -> Event {
        e.rotation = rotation;
        e
    }

    pub(crate) fn with_topic(mut e: Event, topic: &str) -> Event {
        e.topic = Some(topic.to_string());
        e
    }

    // add with_origin / with_signal / with_body / with_meta the same way
    // ONLY as the sweep in Step 3 actually needs them — no speculative API.
}
```

(Adjust default field values to whatever the *majority* of existing
helpers use — inspect the queue.rs helpers first; the point is that
per-site divergence becomes explicit `with_*` calls.)

**Verify**: `cd src-tauri && cargo build` → compiles (the module is
test-only; `cargo test --locked event::` still green).

### Step 3: Sweep the rust test modules

File by file (commit-worthy checkpoints — run the file's tests after
each): replace each local helper's *body* with delegation to
`crate::event::test_fixtures` (keeping thin local wrappers where a
module's tests call them dozens of times is fine and keeps the diff
small — e.g. `queue.rs`'s `fn event(...)` can become a one-line call
into the shared builder), and replace naked inline literals with
builder calls. Where a literal sets unusual fields, express them via
`with_*` combinators so the intent is visible.

Rules:
- If a test asserts on a value the old helper hardcoded (e.g. a
  specific ttl), pass that value explicitly at the call site — never
  change the assertion.
- Do not chase zero-literal purity: a site testing serde/shape of
  `Event` itself (e.g. in `event.rs` tests) may legitimately keep its
  literal.

**Verify** after each file: `cd src-tauri && cargo test --locked <module>::`
→ same pass count for that module as baseline. After the sweep:
`cargo test --locked` → same total as baseline; `cargo clippy --locked
--all-targets -- -D warnings` and `cargo fmt --check` → exit 0.

### Step 4: Extract the frontend harness

Create `src/test-support/tauriEventMock.ts`:

```ts
// Shared tauri event-channel mock (plan 028): one harness for every
// test that feeds the overlay listeners. Handlers are kept per event
// name — the previous per-file copies lumped all listeners into one
// array and emitted slot-state payloads at every registered handler.
import { vi } from "vitest";

type Handler = (event: { payload: unknown }) => void;
const handlersByName = new Map<string, Handler[]>();

export const listen = vi.fn((name: string, handler: Handler) => {
  const list = handlersByName.get(name) ?? [];
  list.push(handler);
  handlersByName.set(name, list);
  return Promise.resolve(() => {});
});

export function emitTo(name: string, payload: unknown) {
  for (const handler of handlersByName.get(name) ?? []) {
    handler({ payload });
  }
}

export function resetHandlers() {
  handlersByName.clear();
}
```

Then in ALL THREE of `App.test.tsx`, `useSlotState.test.ts`, and
`useStatusState.test.ts`:
- `vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));`
  (vitest accepts a factory returning the module promise; if the
  relative path differs, adjust — all three test files sit in `src/`).
- Replace the local `emit` with a thin wrapper that keeps the existing
  `act()` semantics and the file's event name:
  `const emit = (payload: SlotState) => act(() => emitTo("slot-state", payload));`
  in App/useSlotState tests, and
  `const emit = (payload: unknown) => act(() => emitTo("status-state", payload));`
  in useStatusState tests (that file deliberately emits malformed
  payloads, so keep `unknown`).
- Call `resetHandlers()` in the existing `beforeEach` (each file
  already resets its `handlers` array there — mirror that).

**Verify**: `npx vitest run` → same total pass count as baseline;
`npx tsc --noEmit` and `npx biome ci .` → exit 0.

## Test plan

No new tests, no removed tests — the entire suite at its baseline count
IS the test plan. Both counts (rust + frontend) must match baseline
exactly; a differing count means a test was dropped or duplicated in
the sweep.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cd src-tauri && cargo test --locked` exits 0 with EXACTLY the baseline pass count
- [ ] `npx vitest run` exits 0 with EXACTLY the baseline pass count
- [ ] `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npx tsc --noEmit`, `npx biome ci .` all exit 0
- [ ] `grep -rn "vi.mock(\"@tauri-apps/api/event\"" src/ | wc -l` → 3, all one-line factories importing the shared module (no inline `handlers[]` array remains in any test file)
- [ ] In-scope rust test modules build their events via `test_fixtures` (spot-check: `grep -c "test_fixtures" src-tauri/src/queue.rs` ≥ 1, same for http.rs, settings.rs, status.rs)
- [ ] `git diff` contains no hunk outside `#[cfg(test)]` blocks in rust files (review the diff explicitly for this)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Any test's pass/fail status or count changes and the cause isn't an
  obvious fixture mistake you can revert.
- A "fixture" turns out to be load-bearing production code (it isn't
  inside `#[cfg(test)]`) — leave it and report.
- The `vi.mock` factory-import pattern fails under this vitest version
  — report the error rather than inlining the module twice again.
- Plan 037's status row has moved off BLOCKED (its `exec/037-engine`
  branch rewrites the same `queue.rs` test surface this plan sweeps) —
  coordinate ordering with the operator instead of racing a concurrent
  executor through the same modules. (036 and 025 are already DONE, so
  they no longer gate this plan.)

## Maintenance notes

- Future `Event` fields: update `test_fixtures::event` (+ a `with_*` if
  tests need to vary it) and the compiler finds everything else.
- Reviewer should scan the diff for accidental semantic changes:
  every replaced literal's non-default fields must reappear as explicit
  `with_*`/argument values at the same site.
- The frontend harness's per-name handler map removes a latent trap
  (appearance handler receiving slot-state payloads); if a future test
  needs to exercise `appearance-changed`, `emitTo("appearance-changed", …)`
  now exists for free.
