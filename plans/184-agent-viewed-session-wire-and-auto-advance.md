# Plan 184: Wire agent-viewed-session-changed to the display, then add auto-advance

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f810d58..HEAD -- src-tauri/src/lib.rs src-tauri/src/tabs.rs src/components/StatusRailCard.tsx src/components/TabBelowBlock.tsx src/useTabSelection.ts`
> If any of those files changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition. In particular: `plans/README.md`'s
> "eighth audit session" note (already on master) independently found the
> same missing-wire gap this plan's Part 1 fixes, filed under "Dead prefix
> pull actions" in its "verified but NOT planned" backlog. If someone has
> since turned that into its own numbered plan, treat this plan's Part 1 as
> superseded by it (do Part 1 from whichever plan lands first; do not do it
> twice) and proceed only with Part 2 here.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none (Part 2 depends on Part 1 landing first, within this plan)
- **Category**: bug fix (Part 1) + feature (Part 2)
- **Planned at**: commit `f810d58`, 2026-08-02
- **Spec**: `docs/superpowers/specs/2026-08-02-agent-session-auto-advance-design.md`

## Why this matters

**Part 1 — a real, independently-confirmed gap.** `PrefixAction::
NextSession`/`PreviousSession` (`src-tauri/src/prefix.rs`) already
mutates `tab_wire.viewed_session` and emits `agent-viewed-session-changed`
(`src-tauri/src/lib.rs`'s `handle_prefix_followup`), but
`StatusRailCard.tsx` never passes `viewedSessionIndex` into
`TabBelowBlock` — it stays `undefined`, so `AgentBelowBlock` always shows
the first session. Pressing the prefix + next/prev keys today changes
rust state nothing displays.

**Part 2 — the operator's actual request.** The Agent tab's session view
should automatically cycle through sessions on a timer (pausing on
hover), not just via manual keypress. This can only be built cleanly
once Part 1 exists — both the manual keys and the new auto-timer need to
drive the SAME `viewed_session` value through the SAME wire event, one
source of truth, matching this app's receive-only frontend architecture.

## Current state

`src-tauri/src/lib.rs:2129-2159` (`handle_prefix_followup`'s
`PreviousSession`/`NextSession` arm — unchanged by Part 1, extended by
Part 2):

```rust
        prefix::PrefixAction::PreviousSession | prefix::PrefixAction::NextSession => {
            // Spec §9: "ignored unless the agent tab is selected" — the
            // caller-side gate PrefixAction's own doc assigns here.
            let agent_selected = {
                let sel = tab_wire
                    .tabs
                    .selection
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                sel.selected() == Some(tabs::Tab::Agent)
            };
            if agent_selected {
                let count = tab_wire.agent_sessions.load(Ordering::Relaxed);
                if count > 0 {
                    let delta: isize = if action == prefix::PrefixAction::NextSession {
                        1
                    } else {
                        -1
                    };
                    let current = tab_wire.viewed_session.load(Ordering::Relaxed) as isize;
                    let next = (current + delta).rem_euclid(count as isize) as usize;
                    tab_wire.viewed_session.store(next, Ordering::Relaxed);
                    use tauri::Emitter;
                    if let Err(e) = app.emit(
                        "agent-viewed-session-changed",
                        serde_json::json!({ "index": next }),
                    ) {
                        tracing::error!("failed to emit agent-viewed-session-changed: {e}");
                    }
                }
            }
        }
```

`src-tauri/src/tabs.rs:163-197` (`TabWire`, full struct — the fields
this plan reads/extends):

```rust
#[derive(Debug)]
pub struct TabWire {
    pub agent_sessions: std::sync::atomic::AtomicUsize,
    pub news_charge: std::sync::Mutex<crate::news_charge::NewsCharge>,
    pub tabs: TabState,
    pub prefix: std::sync::Mutex<crate::prefix::PrefixState>,
    pub prefix_generation: std::sync::atomic::AtomicU64,
    pub viewed_session: std::sync::atomic::AtomicUsize,
    pub followups_registered: std::sync::atomic::AtomicBool,
    pub slot_occupied: std::sync::atomic::AtomicBool,
}
```

`src-tauri/src/lib.rs:463-464` (the hover latch this plan reads; note
its own `#[cfg(target_os = "macos")]` gate — hover tracking is AppKit-only,
which is why Part 2's new task must be gated the same way):

```rust
            #[cfg(target_os = "macos")]
            let was_hovered = Arc::new(StdMutex::new(false));
```

`src-tauri/src/queue.rs:769` — `pub fn is_paused(&self) -> bool`, read
via `engine.apply_blocking(|q, _now| q.is_paused())` (this exact idiom is
used throughout `lib.rs`, e.g. line 2218's `toggle_pause`).

`src/useTabSelection.ts` (full file, 82 lines) — the template Part 1's
new hook mirrors exactly:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Tab } from "./components/IconStrip";
import { TAB_ORDER } from "./components/IconStrip";

export type TabSelectionPayload = { selected: Tab | null };

const VALID_TABS: ReadonlySet<string> = new Set(TAB_ORDER);

export function isValidTabSelection(v: unknown): v is TabSelectionPayload {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  if (obj.selected === null) {
    return true;
  }
  return typeof obj.selected === "string" && VALID_TABS.has(obj.selected);
}

export function useTabSelection(): Tab | null {
  const [selected, setSelected] = useState<Tab | null>(null);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("tab-selection-changed", ({ payload }) =>
      setSelected(isValidTabSelection(payload) ? payload.selected : null),
    )
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("tab-selection-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return selected;
}
```

`src/components/TabBelowBlock.tsx:96-130` (the already-built,
already-correct receiving end — Part 1 does NOT modify this file, it
only supplies the missing caller):

```tsx
  viewedSessionIndex?: number;
  expanded?: boolean;
}) {
  if (!tabBelowBlockHandles(selected)) {
    return null;
  }

  switch (selected) {
    case "agent":
      return (
        <AgentBelowBlock
          sessions={agentSessions}
          viewedIndex={viewedSessionIndex}
          capturedAtMs={agentCapturedAtMs}
          nowMs={nowMs}
        />
      );
```

`src/components/StatusRailCard.tsx:1107-1112` (the mount site Part 1
DOES modify — currently omits `viewedSessionIndex` entirely):

```tsx
            <TabBelowBlock
              selected={pulledTab}
              status={status}
              agentSessions={agentSessions}
              agentCapturedAtMs={agentCapturedAtMs}
            />
```

## Commands you will need

Rust commands run from `src-tauri/`; frontend commands from repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass |
| Rust lints | `cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Type check | `npx tsc --noEmit` | exit 0 |
| Lint | `npx biome ci .` | exit 0 |

## Scope

**In scope**:
- `src/useAgentViewedSession.ts` (new)
- `src/useAgentViewedSession.test.ts` (new)
- `src/components/StatusRailCard.tsx` (Part 1: pass `viewedSessionIndex`
  into `TabBelowBlock`; no other change)
- `src/components/StatusRailCard.test.tsx` (new integration test)
- `src-tauri/src/tabs.rs` (Part 2: add `session_advanced: tokio::sync::
  Notify` to `TabWire`)
- `src-tauri/src/lib.rs` (Part 2: new pure gate function + new spawned
  task; Part 2 also adds `tab_wire.session_advanced.notify_one()` to the
  existing manual-advance branch shown in "Current state")

**Out of scope**:
- `TabBelowBlock.tsx`, `AgentBelowBlock.tsx`, `PositionBar.tsx` — already
  correct, need no changes.
- `AgentBoard.tsx` — this plan is scoped to the Agent tab on
  `StatusRailCard` only, not the automatic full-board surface.
- `engine.rs`'s `spawn_rotation` — unrelated rotation (which CARD is in
  the slot, not which session is viewed within the Agent tab).
- Any UI library addition (`shadcn`/`embla-carousel`) — explicitly
  rejected in the spec; not part of this plan.

## Git workflow

- Branch: `feat/agent-viewed-session-and-auto-advance`
- Commit style: conventional. Suggest two commits matching the two parts:
  `fix(agent): wire agent-viewed-session-changed to the display` then
  `feat(agent): auto-advance viewed session on a timer`.
- Open a PR when both parts are done and green.

## Steps

### Part 1 — wire the missing display

### Step 1: Write the failing hook test

Create `src/useAgentViewedSession.test.ts`, mirroring
`useTabSelection.test.ts`'s structure (read that file first for the
exact mocking pattern it uses for `@tauri-apps/api/event`'s `listen`):

```ts
import { describe, expect, it } from "vitest";
import { isValidAgentViewedSession } from "./useAgentViewedSession";

describe("isValidAgentViewedSession", () => {
  it("accepts a non-negative integer index", () => {
    expect(isValidAgentViewedSession({ index: 0 })).toBe(true);
    expect(isValidAgentViewedSession({ index: 3 })).toBe(true);
  });

  it("rejects a negative index", () => {
    expect(isValidAgentViewedSession({ index: -1 })).toBe(false);
  });

  it("rejects a non-integer index", () => {
    expect(isValidAgentViewedSession({ index: 1.5 })).toBe(false);
  });

  it("rejects a missing or malformed payload", () => {
    expect(isValidAgentViewedSession(null)).toBe(false);
    expect(isValidAgentViewedSession({})).toBe(false);
    expect(isValidAgentViewedSession({ index: "0" })).toBe(false);
  });
});
```

Also add the hook-behavior tests following `useTabSelection.test.ts`'s
own pattern exactly (valid payload updates state; malformed payload is
ignored, state stays at the default `0`; listener registration failure
calls `console.error`) — copy that file's test structure, not just this
validator table.

**Verify**: `npx vitest run useAgentViewedSession` → FAILS (module
doesn't exist yet).

### Step 2: Implement the hook

Create `src/useAgentViewedSession.ts`:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

/// The frontend half of the `agent-viewed-session-changed` channel —
/// mirrors `useTabSelection.ts`'s shape exactly (listen-only, strict
/// validator, dead-listener `console.error`, no boot seed since a
/// viewed-session index is only meaningful once sessions exist).
/// **Rust owns this value, not this hook** — both the manual prefix-key
/// cycling (`handle_prefix_followup` in `src-tauri/src/lib.rs`) and the
/// auto-advance timer (this plan's Part 2) write `tab_wire.viewed_session`
/// and emit this event; the frontend only ever renders what it's told.
export type AgentViewedSessionPayload = { index: number };

export function isValidAgentViewedSession(v: unknown): v is AgentViewedSessionPayload {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  return (
    typeof obj.index === "number" && Number.isInteger(obj.index) && obj.index >= 0
  );
}

export function useAgentViewedSession(): number {
  const [index, setIndex] = useState(0);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("agent-viewed-session-changed", ({ payload }) => {
      if (isValidAgentViewedSession(payload)) {
        setIndex(payload.index);
      }
    })
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("agent-viewed-session-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return index;
}
```

**Verify**: `npx vitest run useAgentViewedSession` → PASSES.

### Step 3: Wire the hook into StatusRailCard

In `src/components/StatusRailCard.tsx`: import `useAgentViewedSession`,
call it once near the component's other hook calls, and pass the result
as `viewedSessionIndex` in the `TabBelowBlock` call shown in "Current
state":

```tsx
            <TabBelowBlock
              selected={pulledTab}
              status={status}
              agentSessions={agentSessions}
              agentCapturedAtMs={agentCapturedAtMs}
              viewedSessionIndex={viewedSessionIndex}
            />
```

(`viewedSessionIndex` here is the local variable holding
`useAgentViewedSession()`'s return value — name it consistently with
the file's existing hook-call variable naming conventions nearby.)

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 4: Add the integration pin test

In `src/components/StatusRailCard.test.tsx`, add a test confirming the
hook's value actually reaches `PositionBar`/`AgentBelowBlock` — find an
existing test in that file that renders `<StatusRailCard>` with
`agentSessions` set to 2+ sessions and a selected agent tab, as a
starting template, then simulate the `agent-viewed-session-changed`
event firing (mock `@tauri-apps/api/event`'s `listen` the same way
`useTabSelection`'s own consuming tests do — grep the test file for how
`tab-selection-changed` is simulated in existing tests and copy that
pattern) and assert the displayed session content changes to match the
new index.

**Verify**: `npx vitest run StatusRailCard` → all pass, including the
new test.

### Part 2 — auto-advance

### Step 5: Add the Notify field to TabWire

In `src-tauri/src/tabs.rs`, add to the `TabWire` struct (after
`viewed_session`, before `followups_registered`):

```rust
    /// Fired whenever `viewed_session` changes for ANY reason (manual
    /// prefix-key cycling or the auto-advance timer below) — lets the
    /// auto-advance loop's wait reset on a manual advance instead of
    /// firing again moments later, per the spec's "manual navigation
    /// resets the auto-advance clock" requirement. Mirrors the
    /// `tokio::sync::Notify` pattern `engine.rs`'s own rotation loop
    /// already uses for "sleep until deadline, wake early on mutation."
    pub session_advanced: tokio::sync::Notify,
```

And in `TabWire::new()`, add `session_advanced: tokio::sync::Notify::new(),`
to the constructor (after `viewed_session: ...`).

**Verify**: `cargo test --locked` → all pass (no behavior change yet —
the field is unused so far; this will produce a harmless
"field never read"-adjacent situation only until Step 6 reads it. If
`cargo clippy` complains about it before Step 6 finishes, that's
expected and resolves once Step 6 lands — don't add `#[allow(dead_code)]`
for what's about to be used two steps later).

### Step 6: Notify on manual advance

In `src-tauri/src/lib.rs`'s `handle_prefix_followup`, in the
`PreviousSession | NextSession` arm shown in "Current state", add one
line right after the existing `tab_wire.viewed_session.store(next, ...)`
call (before the `emit` call):

```rust
                    tab_wire.viewed_session.store(next, Ordering::Relaxed);
                    tab_wire.session_advanced.notify_one();
                    use tauri::Emitter;
```

**Verify**: `cargo test --locked` → all pass.

### Step 7: Write the failing gate-function test

Add to `lib.rs`'s existing `#[cfg(test)]` region, near the other prefix
tests (find with `grep -n "mod tests" src-tauri/src/lib.rs`):

```rust
    #[test]
    fn should_auto_advance_session_requires_agent_tab_multiple_sessions_no_hover_no_pause() {
        assert!(should_auto_advance_session(Some(tabs::Tab::Agent), 3, false, false));
    }

    #[test]
    fn should_auto_advance_session_false_when_different_tab_selected() {
        assert!(!should_auto_advance_session(Some(tabs::Tab::Weather), 3, false, false));
    }

    #[test]
    fn should_auto_advance_session_false_when_no_tab_selected() {
        assert!(!should_auto_advance_session(None, 3, false, false));
    }

    #[test]
    fn should_auto_advance_session_false_with_one_or_zero_sessions() {
        assert!(!should_auto_advance_session(Some(tabs::Tab::Agent), 1, false, false));
        assert!(!should_auto_advance_session(Some(tabs::Tab::Agent), 0, false, false));
    }

    #[test]
    fn should_auto_advance_session_false_while_hovered() {
        assert!(!should_auto_advance_session(Some(tabs::Tab::Agent), 3, true, false));
    }

    #[test]
    fn should_auto_advance_session_false_while_paused() {
        assert!(!should_auto_advance_session(Some(tabs::Tab::Agent), 3, false, true));
    }
```

**Verify**: `cargo test --locked should_auto_advance_session` → FAILS
(function doesn't exist yet).

### Step 8: Implement the pure gate function

Add near the other prefix-region helpers in `lib.rs` (this function is
NOT `#[cfg(target_os = "macos")]`-gated — it takes plain values and has
no OS dependency, so it stays unit-testable on every platform, same
reasoning `docs/TESTING_STRATEGY.md` §4.4 gives for `presentation_mode`):

```rust
/// Pure decision, thin apply wrapper (house pattern — see
/// `watchdog_verdict` in plan 178 / `silence_should_flip`'s own comment).
/// Whether this tick should advance the Agent tab's viewed session
/// automatically: the Agent tab must be selected, there must be 2+
/// sessions to cycle between, the app must not be hovered (pause on
/// hover, matching the operator's own request), and not paused (matches
/// every other "the engine isn't delivering anything right now"
/// precedent in this codebase, e.g. `StatusDots`' pause handling).
fn should_auto_advance_session(
    tab_selected: Option<tabs::Tab>,
    session_count: usize,
    hovered: bool,
    paused: bool,
) -> bool {
    tab_selected == Some(tabs::Tab::Agent) && session_count > 1 && !hovered && !paused
}
```

**Verify**: `cargo test --locked should_auto_advance_session` → all 6
PASS.

### Step 9: Spawn the auto-advance task

Add near the other periodic-task spawns in `.setup()`
(`src-tauri/src/lib.rs`, alongside `spawn_silence_task`/
`poller::spawn_espn_poller`/etc. — same region "Current state" of plan
178 excerpts), gated `#[cfg(target_os = "macos")]` (it reads
`was_hovered`, itself macOS-only per the excerpt above):

```rust
            #[cfg(target_os = "macos")]
            {
                const SESSION_AUTO_ADVANCE_INTERVAL: std::time::Duration =
                    std::time::Duration::from_secs(6);
                let auto_advance_wire = tab_wire.clone();
                let auto_advance_app = app.handle().clone();
                let auto_advance_engine = engine.clone();
                let auto_advance_hovered = was_hovered.clone();
                tauri::async_runtime::spawn(async move {
                    use std::sync::atomic::Ordering;
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(SESSION_AUTO_ADVANCE_INTERVAL) => {}
                            _ = auto_advance_wire.session_advanced.notified() => {
                                // A manual advance just happened — restart the
                                // wait instead of also firing this tick.
                                continue;
                            }
                        }
                        let tab_selected = {
                            let sel = auto_advance_wire
                                .tabs
                                .selection
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            sel.selected()
                        };
                        let session_count = auto_advance_wire.agent_sessions.load(Ordering::Relaxed);
                        let hovered = *auto_advance_hovered.lock().unwrap_or_else(|e| e.into_inner());
                        let paused = auto_advance_engine.apply_blocking(|q, _now| q.is_paused());
                        if should_auto_advance_session(tab_selected, session_count, hovered, paused) {
                            let current = auto_advance_wire.viewed_session.load(Ordering::Relaxed) as isize;
                            let next = (current + 1).rem_euclid(session_count as isize) as usize;
                            auto_advance_wire.viewed_session.store(next, Ordering::Relaxed);
                            use tauri::Emitter;
                            if let Err(e) = auto_advance_app.emit(
                                "agent-viewed-session-changed",
                                serde_json::json!({ "index": next }),
                            ) {
                                tracing::error!(
                                    "failed to emit agent-viewed-session-changed (auto-advance): {e}"
                                );
                            }
                        }
                    }
                });
            }
```

**Verify**: `cargo test --locked` → all pass. `cargo clippy --locked
--all-targets -- -D warnings` → exit 0 (on macOS; on this Linux dev box
the whole block is compiled out by the `cfg` gate — expected, matches
the prefix-wiring fix's own precedent (`fix/prefix-wiring-linux-cfg-gate`,
already merged) — do NOT leave this block ungated the way that bug did).

### Step 10: Full verification, both stacks

Run all five commands from "Commands you will need". All must pass.

### Step 11: Commit and open PR

```bash
git add src/useAgentViewedSession.ts src/useAgentViewedSession.test.ts src/components/StatusRailCard.tsx src/components/StatusRailCard.test.tsx
git commit -m "fix(agent): wire agent-viewed-session-changed to the display"
git add src-tauri/src/tabs.rs src-tauri/src/lib.rs
git commit -m "feat(agent): auto-advance viewed session on a timer"
git push -u origin feat/agent-viewed-session-and-auto-advance
gh pr create --title "feat(agent): wire viewed-session display + auto-advance" --body "Implements docs/superpowers/specs/2026-08-02-agent-session-auto-advance-design.md. Part 1 fixes a real gap (agent-viewed-session-changed was emitted into the void — StatusRailCard never passed viewedSessionIndex to TabBelowBlock). Part 2 adds a rust-side auto-advance timer driving the same wire path, gated on Agent-tab-selected + 2plus sessions + not-hovered + not-paused, with manual prefix-key advances resetting the timer via a tokio::sync::Notify."
```

## Test plan

- `useAgentViewedSession.test.ts` — validator table + hook behavior
  (Step 1), mirroring `useTabSelection.test.ts`.
- `StatusRailCard.test.tsx` integration pin (Step 4) confirming the wire
  actually reaches the rendered session.
- Rust: 6 table tests for `should_auto_advance_session` (Step 7) covering
  every gate independently.
- No real-timer async test for the spawned loop itself — same rationale
  plan 178 gives for its own watchdog loop: the pure decision function
  carries the logic, the loop stays thin and untested by design.

## Done criteria

- [ ] `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` exit 0 (from `src-tauri/`)
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` all exit 0 (from repo root)
- [ ] `should_auto_advance_session` exists with 6 passing table tests
- [ ] `grep -n "viewedSessionIndex={viewedSessionIndex}" src/components/StatusRailCard.tsx` → match found
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated; note the "eighth audit
      session" backlog item this plan's Part 1 resolves, so it isn't
      re-planned separately

## STOP conditions

- Any "Current state" excerpt no longer matches the live code — a
  concurrent change landed (very possible: see the drift-check note
  about the audit session's own backlog item). Re-diff before proceeding.
- `tab_wire.tabs.selection()` or `was_hovered` are not reachable from a
  freshly-spawned async task the way this plan assumes (e.g. a
  restructuring moved them) — STOP and report rather than widening
  access ad hoc.
- Someone has already turned the "eighth audit session"'s "Dead prefix
  pull actions" backlog note into its own numbered plan and it has
  landed — do Part 2 only, referencing that plan's Part 1 instead of
  redoing it.

## Maintenance notes

- `SESSION_AUTO_ADVANCE_INTERVAL` (6s) has no existing precedent in this
  codebase to match — it's a fresh choice. If it feels too fast/slow
  once verified on hardware, retune the one constant; no other code
  depends on its exact value.
- The auto-advance loop is deliberately simpler than `queue.rs`'s
  hover-banking rotation (`hover_enter`/`hover_exit`/
  `rotate_out_if_elapsed`) — it has no visible countdown to preserve
  exact-elapsed-time for, just a skip-this-tick-if-hovered gate. If a
  future report says a long hover followed by an immediate advance the
  instant the cursor leaves feels janky, escalate to the banked-time
  pattern rather than tuning around it.
- On-hardware check (operator-owed): confirm auto-advance actually
  cycles sessions while idle on the Agent tab, pauses immediately on
  hover, resumes after hover ends, and a manual `⌃⇧[`/`⌃⇧]` press
  doesn't get immediately followed by an auto-advance jump.
