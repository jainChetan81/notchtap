# Plan 177: Stop blank pulls — feed the agent tab the ungated sessions and fall back to the peek for empty tabs

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src-tauri/src/agents/board.rs src-tauri/src/agents/model.rs src/useAgentState.ts src/App.tsx src/components/StatusRailCard.tsx src/components/TabBelowBlock.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none (independent of 175/176; touches `StatusRailCard.tsx` in a different region than 176 — whoever lands second reconciles by reading)
- **Category**: bug
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Under the shipped default (`[agents] board_show_working = false`), the most
common agent state — a session that is Working but needs nothing — lights
the agent icon in the strip, lets the click monitor select it, and then
renders an **empty** below-block. Root cause: the icon's presence count and
the pull surface's data come from two different views of the registry.

- The icon count is **ungated**: `board.rs`'s `publish_if_changed` stores
  `tab_wire.agent_sessions` from the raw `ordered_states` slice
  (non-terminal, non-stale), per plan 171's spec §6 ("a session is
  genuinely running"). `tabs.rs::present_tabs` and the frontend's
  `iconPresence.ts` both read that count.
- The pull surface's data is **gated**: `gate_presence` replaces the
  published slice with `[]` when no session summons the board (the
  operator's 2026-08-02 decision: "agents that are merely working must not
  summon the board"), and `App.tsx` threads exactly that gated
  `agentState.sessions` into the tab surface.

Both decisions are correct for their own consumers; the pull surface is a
**user-initiated** view — the operator clicked the icon — so it should see
the ungated list. The presence gate governs autonomous board summoning,
not explicit pulls; this plan does not touch that gate's behaviour.

A second, smaller half: `StatusRailCard` closes the ambient peek whenever
a below-block-owning tab is selected, based only on WHICH tab — never on
whether the tab will render anything. News (no story wire exists yet, the
block is hard-wired empty) and music-with-nothing-playing therefore
collapse the hover to a blank shell. An empty tab must degrade to the
ambient peek, not to nothing.

## Current state

`src-tauri/src/agents/board.rs` — `publish_if_changed` (starting ~`:384`):

```rust
pub async fn publish_if_changed(&self, now: Instant) -> bool {
    let ungated = self.registry.ordered_states(now).await;
    // Plan 171: the agent icon counts LIVE sessions ... from the ungated
    // registry view, independent of the Board's own show-working presence gate.
    self.tab_wire.agent_sessions.store(
        ungated.iter().filter(|st| !st.state.is_terminal()
            && st.state != crate::agents::model::AgentSessionState::Stale).count(),
        std::sync::atomic::Ordering::Relaxed,
    );
    let states = self.gate_presence(ungated);
    let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
    let changed = match &guard.last {
        None => true,
        Some(prev) => !states_dedup_eq(prev, &states),
    };
    if !changed { return false; }
    guard.revision += 1;
    ...
    let snapshot = AgentStateSnapshot {
        revision,
        captured_at_ms: now_ms(),
        sessions: states.iter().map(|s| to_view(s, now)).collect(),
        adapter_health,
    };
    if let Err(e) = self.app.emit(AGENT_STATE_EVENT, &snapshot) { ... }
    true
}
```

Critical detail: `gate_presence(ungated)` is a **pure function of the
ungated slice** (all-or-nothing: `[]` unless some session
`summons_board()`), and the dedup currently compares **gated** slices. If
the snapshot starts carrying ungated data, the dedup must move to the
**ungated** slice — otherwise ungated-only changes (a Working session's
state advancing while the gate is closed) would never emit and the pull
surface would go stale. Deduping on ungated is sufficient for both because
the gated slice is derived from it deterministically.

`src/App.tsx:315-321` — the tab surface reads the gated list:

```tsx
// Plan 171 (slice K): the tab surface's three inputs.
// `agentState` is already read above for the Board's own
// presentation branch — the agent tab's below-block reads
// the same snapshot rather than a second subscription.
selectedTab={selectedTab}
agentSessions={agentState.sessions}
agentCapturedAtMs={agentState.capturedAtMs}
```

`src/components/AgentBelowBlock.tsx:70-72` — empty list renders nothing:

```tsx
if (sessions.length === 0) {
  return null;
}
```

`src/components/StatusRailCard.tsx:631-638` — the peek gate keys on tab
identity only:

```tsx
const pulledTab = tabPullOpen ? selectedTab : null;
const peekPreference = peekPreferenceFor(pulledTab);
// ... there is never more than one `.below-block` under the shell (the
// rounding law in card-chrome.css depends on that).
const peekOpen = tabPullOpen && !tabBelowBlockHandles(pulledTab);
```

`src/components/TabBelowBlock.tsx` — the three branches' empty cases:
agent → `null` (above); media → `null` when `status.media.current` is
null; news → hard-wired `NO_NEWS_STORIES` (module-level `[]`, "no wire
source exists for news story CONTENT … deliberately not faked"), and
`NewsBelowBlock` renders `null` for zero stories.

`src/useAgentState.ts` — the frontend listener/validator for the
`agent-state` snapshot (`sessions`, `capturedAtMs`, `adapterHealth`,
`revision`); it has a validation guard in the house
`isValidSlotState` style and a test file `src/useAgentState.test.ts`.

Repo conventions that apply:

- Wire snapshots are validated on the frontend before use; a malformed
  payload falls back wholesale (see `src/useAgentState.ts`'s existing
  guard). Extend the guard for any new field; never trust the payload.
- `SlotState::dedup_eq` rule (CLAUDE.md): publish suppression compares
  content fields explicitly; a clock-only tick must never emit. The board's
  `states_dedup_eq`/`AgentState::dedup_eq` (`model.rs`, handwritten, spec
  §2.3) already encode this — reuse them on the ungated slice, do not
  derive `PartialEq`.
- Vocabulary (`CONTEXT.md` / plan 171): "pull"/"pulled tab" for the
  user-initiated surface; "summons" for the board's autonomous presence.

## Commands you will need

Run rust commands from `src-tauri/`, web commands from the repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` | all pass |
| Rust lints | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint gate | `npx biome ci .` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/agents/board.rs`
- `src/useAgentState.ts` + `src/useAgentState.test.ts`
- `src/App.tsx`
- `src/components/StatusRailCard.tsx` + its test
- `src/components/TabBelowBlock.tsx` + its test (predicate only, if placed here)
- `docs/TESTING_STRATEGY.md` §0 (counts)

**Out of scope** (do NOT touch, even though they look related):
- `gate_presence` itself and `board_show_working` semantics — the
  operator's decision stands; the board's own presence behaviour must be
  byte-identical.
- `src-tauri/src/agents/registry.rs` — known recorded findings live there
  (F10/F11 in `plans/README.md`); do not fix them in passing.
- The news story wire — a recorded direction option, not this plan. News
  stays empty; it must *degrade to the peek*, not gain content.
- The news charge/`isCharged` consumption semantics.
- `capabilities/*.json`, `build.rs` — no new command, no capability change.

## Git workflow

- Branch: `advisor/177-blank-pull-surfaces`
- Commit style: conventional, e.g. `fix(agents): publish ungated sessions for the pull surface` and `fix(tabs): fall back to the ambient peek for empty tabs`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Move the board dedup to the ungated slice

In `board.rs::publish_if_changed`, compare `!states_dedup_eq(prev_ungated,
&ungated)` instead of the gated slices: store the last **ungated** slice in
`PublishState.last_ungated` (keep `last` holding the gated slice — `lib.rs`'s
hover primitive reads `last_session_count` from it; that consumer must keep
seeing exactly what the overlay's BOARD was last told). Update the doc
comment to explain: dedup on ungated is a superset trigger (gated is a pure
function of ungated), so board consumers see no behaviour change, while
ungated-only changes now emit for the tab surface.

Add/extend rust tests in `board.rs`'s `#[cfg(test)]` block: an
ungated-only change (Working session advances state with the gate closed)
now publishes; a clock-only tick still does not.

**Verify**: from `src-tauri/`, `cargo test --locked board` → all pass.

### Step 2: Carry the ungated sessions on the snapshot

Add `tab_sessions: Vec<AgentSessionView>` to `AgentStateSnapshot`
(serialized in the same casing as the existing fields — check the struct's
serde attributes and mirror them), filled with
`ungated.iter().map(|s| to_view(s, now)).collect()`. Keep `sessions` as
the gated list. Update the struct's doc comment: `sessions` = the Board
(summons-gated); `tab_sessions` = the pull surface (user-initiated).

**Verify**: `cargo test --locked` (from `src-tauri/`) → all pass;
`cargo clippy --locked --all-targets -- -D warnings` → exit 0.

### Step 3: Consume it on the frontend

In `src/useAgentState.ts`: extend the payload type and the validation
guard for `tabSessions` (same per-session validation the existing
`sessions` field gets — reuse the same session validator, not a copy), and
expose it from the hook with a safe `[]` fallback. Extend
`src/useAgentState.test.ts` with: a valid payload carrying `tabSessions`
parses; a payload with malformed `tabSessions` is rejected wholesale
(matching the file's existing rejection-test shape).

In `src/App.tsx:320`, thread `agentSessions={agentState.tabSessions}` into
the tab surface (the Board's own render above keeps reading
`agentState.sessions` — do not touch it). Update the slice-K comment.

**Verify**: `npx vitest run useAgentState` → all pass; `npx tsc --noEmit`
→ exit 0.

### Step 4: Peek fallback for empty tabs

In `src/components/StatusRailCard.tsx`, derive a content predicate and use
it in the peek gate:

```tsx
const pulledTabHasContent =
  pulledTab === "agent" ? agentSessions.length > 0
  : pulledTab === "music" ? status.media.current !== null
  : pulledTab === "news" ? false // no story wire yet — recorded direction option
  : pulledTab !== null; // football/weather are peek-preference tabs, not below-block tabs
const peekOpen = tabPullOpen && !(tabBelowBlockHandles(pulledTab) && pulledTabHasContent);
```

Match the file's actual prop names and the `tabBelowBlockHandles` set when
writing the real code — the sketch above is the shape, not paste-ready
text. Also gate the pulled-tab `motion.div` mount (the `pulledTab !== null`
condition) on the same predicate so an empty tab mounts nothing AND keeps
the peek — preserving the "one `.below-block` at a time" rounding law the
comment at `:634-637` names. Keep the predicate's three guards literally
identical to the three components' own empty-guards (agent
`sessions.length === 0`, media `current === null`, news hard-wired empty)
and say so in a comment, so a future content wire flips both together.

Extend `src/components/StatusRailCard.test.tsx`: selecting news keeps the
ambient peek mounted; selecting music with no track keeps the peek;
selecting agent with ungated sessions mounts the agent below-block.

**Verify**: `npx vitest run StatusRailCard` → all pass.

### Step 5: Full gates + counts

All five commands green; recount `docs/TESTING_STRATEGY.md` §0 from live
output.

## Test plan

- Rust (`board.rs` tests): ungated-only change publishes; clock-only tick
  does not; gated `sessions` field unchanged by the gate-closed publish;
  `tab_sessions` carries the working session.
- Frontend: `useAgentState` accept/reject for `tabSessions`;
  `StatusRailCard` fallback behaviour for the three empty-tab cases and
  the agent-with-sessions case. Model on each file's existing tests.

## Done criteria

- [ ] All five commands exit 0
- [ ] `grep -n "tab_sessions" src-tauri/src/agents/board.rs` → snapshot field + fill site
- [ ] `grep -n "tabSessions" src/useAgentState.ts src/App.tsx` → validator + threading
- [ ] New tests from the Test plan exist and pass
- [ ] Board presence behaviour unchanged: every pre-existing `board.rs` and `gate_presence` test passes unmodified (test edits allowed only for constructor/fixture arity)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated; `docs/TESTING_STRATEGY.md` §0 recounted

## STOP conditions

Stop and report back (do not improvise) if:

- `states_dedup_eq`/`PublishState` are structured differently than the
  excerpt (the dedup move has a hidden consumer).
- `lib.rs`'s `last_session_count` (hover-expand) turns out to read
  anything other than the gated `last` slice — the split in Step 1 would
  be wrong; report the actual consumer graph.
- Any existing board/gate test needs a **behavioural** (not fixture-arity)
  change to pass — that means this plan is altering the presence gate,
  which is out of scope.
- The `agent-state` payload has a size guard or schema pin somewhere this
  plan doesn't list (search `AGENT_STATE_EVENT` consumers first).

## Maintenance notes

- F11 in `plans/README.md` (no cap on registry sessions; re-serialize per
  event) gets marginally heavier with a second list on the wire — if F11
  is ever planned, `tab_sessions` doubles down on the same fix.
- When a news story wire lands (recorded direction option), flip the
  `news` arm of the content predicate and delete `NO_NEWS_STORIES` — the
  Step 4 comment marks the spot.
- Reviewer focus: the dedup move (Step 1) is the risky hunk — check no
  publish path now emits on clock-only ticks (the board's dedup rule).
