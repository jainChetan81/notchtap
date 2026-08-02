# Plan 181: Post-171 docs truth — five stale claims that survived the truth pass, one of them user-visible

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src/settings/sections/ShortcutsSection.tsx src-tauri/src/prefix.rs src-tauri/src/lib.rs src/components/IconStrip.tsx src/components/NewsBatchHeader.tsx src/components/TabBelowBlock.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: 178, 179, 180 (soft — they edit neighbouring regions of `lib.rs`/`ShortcutsSection.tsx`; land this LAST and reconcile by reading)
- **Category**: docs
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Commit `0e9b212` (2026-08-02) was plan 171's post-merge docs truth pass,
but five load-bearing claims survived it, and one is **user-visible in the
Settings window**: the Prefix keybinding group tells the operator the
feature is "not yet wired to a live key grab" when it has been live since
`82a4598` — actively discouraging use of (and bug reports against) a
feature that grabs system-wide keys. The others are comments a future
contributor reads immediately before touching the exact code they
misdescribe: `lib.rs`'s click-through comment asserts the opposite of the
locked §22 decision, and `prefix.rs`'s header describes a pre-wiring world
with a wrong key count. Stale docs that are actively wrong are worse than
missing ones.

This plan changes prose only — zero behaviour. All five claims were
verified against live code on 2026-08-02.

## Current state

**Site 1 — user-visible.** `src/settings/sections/ShortcutsSection.tsx:131`:

```tsx
description="A tmux-style prefix for the tab-notch icon strip (plan 171). Not yet wired to a live key grab — this is the configurable value only."
```

Reality: `lib.rs:945` registers the prefix combo, `lib.rs:1952-1964`
registers the eleven bare follow-ups for the armed window,
`handle_prefix_fire`/`handle_prefix_followup` dispatch.

**Site 2.** `src-tauri/src/prefix.rs:8-33` — the module header:

- "**What this module does NOT do**: register or release any actual OS key
  grab … left for real-device wiring" — the wiring exists (above).
- "the SEVEN follow-up keys" — there are eleven
  (`lib.rs:1952` `[(Code, PrefixKey); 11]`; `enter`/`o` both map to
  ExpandToggle, plus `p` and `esc`).
- "`#![allow(dead_code)]`: staged ahead of that caller … Remove once the
  lib.rs wiring calls `on_prefix`/`on_key` for real" — no
  `#![allow(dead_code)]` exists anywhere in the file today, and the
  callers are live.

**Site 3.** `src-tauri/src/lib.rs:1290-1300` — the comment on
`set_ignore_cursor_events(true)` in `apply_overlay_native_config`:

```rust
// ... safe unconditionally: the
// frontend is receive-only and has no click handlers anywhere — every
// interaction is a global hotkey (⌃⇧N/⌃⇧O), never a click.
window.set_ignore_cursor_events(true)?;
```

Reality: `docs/ARCHITECTURE.md` §22 records that the toggle is no longer
unconditionally true — a conditional (around `lib.rs:1581-1599`) opens
cursor events while the strip is hoverable, and `click.rs` observes clicks.
The `true` at THIS site is still correct as the **boot default**; only the
"safe unconditionally / no click handlers anywhere" rationale is stale.

**Site 4.** Two "still open" asides about a resolved question:
`src/components/IconStrip.tsx:63-71` (the `onSelect` prop doc: "see
plans/171-tab-notch-redesign.md's slice A note on the still-open
click-detection mechanism question") and
`src/components/NewsBatchHeader.tsx:26-32` ("real click detection is Slice
A's still-open Mac-Mini hand-off"). The mechanism is decided and shipped:
the `NSEvent` local monitor (`click.rs:1-13`, `ARCHITECTURE.md` §22). What
remains true: the news nav buttons and media transport buttons have no
click ROUTING yet (recorded in `plans/README.md` as awaiting selection) —
the mechanism itself is no longer open.

**Site 5.** `src/components/TabBelowBlock.tsx:96-104` — the
`viewedSessionIndex` prop doc:

```
There is NO `agent-viewed-session-changed` event on the
wire at this commit (grepped `src/` and `src-tauri/src/` before
writing this) and slice D's `prefix.rs` is not wired to
`tauri_plugin_global_shortcut` yet, so nothing can move it today
```

Reality: rust emits `agent-viewed-session-changed` (`lib.rs:2160-2166`)
and the prefix IS wired; what's missing is the frontend listener/threading
(recorded in `plans/README.md`'s current session block as an unplanned
finding). Half the sentence is now false; the conclusion ("it defaults to
the first session") is still true.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` (from `src-tauri/`) | all pass |
| Rust lints | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --locked --all-targets -- -D warnings` (from `src-tauri/`) | exit 0 |
| Frontend tests | `npx vitest run` (repo root) | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint gate | `npx biome ci .` | exit 0 |

Note: `src/settings/hookEventParity.test.ts` and (if plans 175/180 landed)
the new text-level parity tests read some of these files as text — if a
parity test fails after your edit, you changed a pinned region; restore
the pinned text and put your edit elsewhere in the comment.

## Scope

**In scope** (the only files you should modify — prose/comments only):
- `src/settings/sections/ShortcutsSection.tsx` (the `description` string)
- `src-tauri/src/prefix.rs` (module header comment)
- `src-tauri/src/lib.rs` (the `apply_overlay_native_config` comment only)
- `src/components/IconStrip.tsx` (the `onSelect` doc comment)
- `src/components/NewsBatchHeader.tsx` (the posture comment)
- `src/components/TabBelowBlock.tsx` (the `viewedSessionIndex` doc comment)

**Out of scope** (do NOT touch):
- Any executable code — this plan is prose-only; a diff hunk that changes
  a non-comment, non-string line is a bug in your execution.
- `docs/ARCHITECTURE.md`, `README.md`, `AGENTS.md`, `CONTEXT.md` — checked
  during the audit; their plan-171 content is correct.
- The recorded F3 docs findings from the 2026-07-28 audit
  (`plans/README.md`) — a separate, already-recorded docs backlog; do not
  fold it in here.

## Git workflow

- Branch: `advisor/181-post-171-docs-truth`
- Commit style: `docs: correct five stale plan-171 claims (settings copy, prefix header, click-through rationale)`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: The Settings copy

Rewrite the `description` at `ShortcutsSection.tsx:131` to describe live
behaviour, e.g.: "A tmux-style prefix for the tab-notch icon strip: press
it, then within 2 seconds a follow-up key (1-5 select a tab, [ ] cycle
agent sessions, enter/o expand, p pause, esc cancel)." Keep it one
sentence-pair, matching the neighbouring groups' tone.

**Verify**: `npx vitest run Shortcuts SettingsApp` → pass (a copy
assertion may exist; if a test pinned the old string, update the
assertion to the new string — that is an allowed test edit).

### Step 2: The prefix.rs header

Rewrite the header's stale block (`prefix.rs:8-33`): state that the module
stays pure (no AppKit, no plugin dependency) and that the LIVE wiring is
`lib.rs`'s `PREFIX_FOLLOWUPS` (eleven bare grabs) +
`handle_prefix_fire`/`handle_prefix_followup` + the watchdog. Delete the
"What this module does NOT do … left for real-device wiring" paragraph and
the `#![allow(dead_code)]` paragraph entirely. Correct "SEVEN" to eleven
(or drop the count and point at `PREFIX_FOLLOWUPS`). Keep the spec §9
citation and the purity rationale.

**Verify**: from `src-tauri/`, `cargo clippy --locked --all-targets -- -D warnings` → exit 0.

### Step 3: The click-through rationale

Rewrite `lib.rs`'s comment above `set_ignore_cursor_events(true)`
(`:1290-1299`): keep the 2026-07-17 menu-bar history, then state that
`true` is the BOOT DEFAULT, no longer an invariant — the hover tracking
conditionally opens cursor events for the strip (name the function that
does it, found near `lib.rs:1581-1599`) and `ARCHITECTURE.md` §22 holds
the decision. Delete "safe unconditionally … no click handlers anywhere".

**Verify**: `grep -n "no click handlers anywhere" src-tauri/src/lib.rs` →
no match.

### Step 4: The two "still open" asides and the TabBelowBlock claim

- `IconStrip.tsx` `onSelect` doc: mechanism resolved — clicks are observed
  by the rust `NSEvent` monitor (`click.rs`), the prop remains a
  presentational callback for tests; keep the "correct under either
  answer" history out, state the settled shape.
- `NewsBatchHeader.tsx`: same correction; keep what is still true (these
  nav buttons have no click ROUTING yet — that gap is recorded in
  `plans/README.md`).
- `TabBelowBlock.tsx` `viewedSessionIndex` doc: the wire event
  `agent-viewed-session-changed` EXISTS (`lib.rs`) but has no frontend
  consumer yet (recorded gap); the prop still defaults to the first
  session until that lands. Delete the "grepped before writing this"
  parenthetical.

**Verify**: `grep -rn "still-open\|still open" src/components/IconStrip.tsx src/components/NewsBatchHeader.tsx` → no match;
`grep -n "There is NO" src/components/TabBelowBlock.tsx` → no match.

### Step 5: Full gates

All five commands green. No test-count change expected (Step 1's possible
assertion update changes no count); if a count moved anyway, STOP — you
changed behaviour somewhere.

## Test plan

No new tests — prose-only. The verification greps in each step are the
machine checks; the suites prove no behaviour moved.

## Done criteria

- [ ] All five commands exit 0
- [ ] The four greps in Steps 3-4 return no matches
- [ ] `grep -n "Not yet wired" src/settings/sections/ShortcutsSection.tsx` → no match
- [ ] `grep -n "SEVEN follow-up" src-tauri/src/prefix.rs` → no match
- [ ] `git diff --stat` shows only the six in-scope files
- [ ] Every changed hunk is a comment or string literal (review your own diff)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Any excerpt in "Current state" no longer matches (another docs pass may
  have landed).
- A parity/text-pin test fails after an edit and you cannot restore the
  pinned region without leaving the stale claim in place — report the
  conflict.
- You find the Step 3 conditional (cursor-events opening) no longer exists
  where described — the comment fix would then be wrong too; report what
  the live mechanism is.

## Maintenance notes

- The repo's docs-truth discipline: every future feature-wiring commit
  should sweep the "not yet wired"-style staging comments it obsoletes —
  this plan is the cleanup for plan 171's staged slices.
- Reviewer focus: confirm zero executable-code hunks in the diff.
