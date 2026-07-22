# Plan 106: Docs & prototype truth sync (post-external-review drift)

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`.
>
> **Drift check**: this plan is docs/prototype-only. If any cited line
> below no longer contains the quoted text, re-locate it by grep before
> editing; if the *claim* itself no longer holds (e.g. someone already
> fixed "seven"), skip that step and report it.

## Status

- **Priority**: P2 (three "source of truth" docs state a false command
  count; two specs describe behavior plan 033 reversed)
- **Effort**: S
- **Risk**: LOW (no code)
- **Depends on**: none — do first, everything else in the 106–111 batch
  is code
- **Category**: docs truth pass
- **Planned at**: commit `870cdeb`, 2026-07-22

## Why this matters

An external UI review (2026-07-22, verified line-by-line by four audit
subagents before this filing) found the docs have drifted from shipped
reality in ways that would misdirect the next agent:

1. **Seven vs eleven invoke commands.** `src-tauri/build.rs:9-19` and
   `capabilities/settings.json` correctly list **eleven** commands (a
   verified 1:1:1 match against the `#[tauri::command]` defs in
   `settings.rs` — no security gap), but three docs still say seven.
2. **High-only auto-expand is gone** (plan 033 made expansion
   universal; the hotkey now collapses), but two docs still describe
   the old High-only + hotkey-no-op contract.
3. **Prototype pages contradict themselves** — plan 099 added
   HISTORICAL banners, but adjacent prose still claims "currently
   shipped" / "current state", and the consolidated deck still carries
   095's NO-GO ("No build planned") for now-playing even though
   103 (GO) → 104/105 shipped it.

## Current state (verified 2026-07-22 at `870cdeb`)

- `CLAUDE.md:179` — "the settings window's seven invoke commands
  (`get_config`, `get_default_config`, `get_secret_status`,
  `save_config_and_relaunch`, `set_secret`, `send_test_notification`,
  `set_appearance`)". Missing: `clear_history`, `get_connector_health`,
  `get_history`, `get_recent_log_lines`.
- `docs/V5_TECHNICAL_SPEC.md:19` — "seven invoke commands".
- `docs/ARCHITECTURE.md:571` — "seven invoke commands total as of that
  date".
- `docs/V3_6_TECHNICAL_SPEC.md:978` (§7.1.1; the section header is at
  `:973`) — comment "// no-op while the current slot is already
  auto-expanded (High priority)" with a `Priority::High` guard.
  Reality (`src-tauri/src/lib.rs:1057-1068`): the guard is deleted,
  every promotion starts expanded (`queue.rs:399-405`
  `set_expanded_for_promotion` unconditional), the hotkey always
  flips. **(Cold-read, 2026-07-22): the same reversed contract
  appears in THREE more V3_6 locations outside §7.1.1** —
  `:999` ("this no-op-while-High behavior is this spec's reading"),
  `:1086` ("the pure no-op-while-High branch of
  `toggle_manual_expand` should still be unit-tested"), and `:1185`
  ("pure no-op-while-High branch unit-tested directly"). Fixing only
  §7.1.1 leaves Step 2's verify failing on these.
- `docs/IMPLEMENTATION_PLAN.md:784` (manual checklist; the row starts
  at `:782`) — "pressing it while a High-priority item is
  auto-Expanded is a no-op".
- **(added at review round 2, 2026-07-22)**
  `docs/TESTING_STRATEGY.md:537` — the "**hotkey no-op branch**"
  test-plan row still describes the plan-008 contract ("no-op while
  a High-priority item is Visible — because it's already
  auto-expanded, per plan 008"). Same drift, third location.
- `prototype/status-rail.html:547` — HISTORICAL PROTOTYPE banner;
  shortly after (`:552`) "This page mirrors the currently shipped
  overlay — src/styles.css is copied verbatim… nothing on this page is
  a proposal anymore."
- `prototype/control-panel.html` — the `<h1>` (`:92`) says "Control
  panel — the 9-section window, current state". **(Cold-read
  correction)**: the `<title>` (`:7`) says only "control panel — 9
  sections, with connector health + diagnostics" — it has the stale
  COUNT but NOT the words "current state"; fix the count/framing
  there, don't hunt for words it doesn't contain. `SettingsApp.tsx`'s
  `navigation` array (`:183-192`) has **ten** sections (history was
  the tenth, plan 089).
- `plans/frontend-ui-consolidated.html` (now-playing row, ~`:144`) —
  ends "…verdict NO-GO… No build planned." Plans 103 (GO), 104, 105
  are DONE/MERGED (`ad78ecb`, `10a07ed`).

## Scope

**In scope**: `CLAUDE.md`, `AGENTS.md`, `docs/V5_TECHNICAL_SPEC.md`,
`docs/ARCHITECTURE.md`, `docs/V3_6_TECHNICAL_SPEC.md`,
`docs/IMPLEMENTATION_PLAN.md`, `prototype/status-rail.html`,
`prototype/control-panel.html`, `plans/frontend-ui-consolidated.html`.

**Out of scope**: `plans/README.md` (reviewer-owned); all code; all
other prototypes (099's operator decision stands: banner only, no
rework — this plan only removes *contradictions*, it does not refresh
prototype content); `docs/TESTING_STRATEGY.md` EXCEPT the single
`:537` hotkey row named in Step 2 (§0 counts untouchable).

## Steps

### Step 1: eleven commands
Governing rule (added at plan review, 2026-07-22): **fix
present-tense claims; never rewrite dated/historical statements** —
a sentence that was true when written and is anchored to its date
stays as written, with a current-truth amendment appended nearby.
1. `CLAUDE.md:179` — present-tense claim WITH an enumerated list:
   replace "seven invoke commands" with "eleven invoke commands" and
   extend the list to the full set: `clear_history, get_config,
   get_connector_health, get_default_config, get_history,
   get_recent_log_lines, get_secret_status, save_config_and_relaunch,
   set_secret, send_test_notification, set_appearance`.
   `docs/V5_TECHNICAL_SPEC.md:19` — **(cold-read correction)** this
   is a compressed prose summary line with NO list ("…seven invoke
   commands, config validation + atomic write +…"): bump the count
   word only; do NOT graft a command list into a summary sentence.
2. `AGENTS.md:249` ("has seven") — present-tense: → eleven, same
   list or a pointer to CLAUDE.md's. **`AGENTS.md:27`** ("added a
   seventh invoke command, `get_default_config`") is a true
   historical record of that addition — leave it byte-untouched.
3. `docs/ARCHITECTURE.md:571` — this paragraph is an explicitly
   dated 2026-07-18 amendment, and seven WAS correct on that date.
   Do NOT edit the number inside it (that would falsify the dated
   record). Instead append a new dated amendment after it: "as of
   2026-07-22 the count is eleven (history ×2, connector-health,
   log-lines added); `src-tauri/build.rs` is the authority."
**Verify**: `grep -rn "seven" CLAUDE.md AGENTS.md docs/ARCHITECTURE.md docs/V5_TECHNICAL_SPEC.md`
→ every surviving hit is one of the two permitted historical
statements (AGENTS.md:27, the dated ARCHITECTURE.md paragraph) or
unrelated to command counts; list the survivors in your report.

### Step 2: auto-expand truth
- `V3_6_TECHNICAL_SPEC.md` §7.1.1 (`:978` comment + surrounding
  description): rewrite the `toggle_manual_expand` description to
  current reality — universal auto-expand at promotion (plan 033),
  hotkey always flips (first press on a fresh card collapses), no
  High guard. Add a one-line "(plan 008's High-only contract;
  reversed by plan 033)" historical note rather than deleting the
  history.
- **(Cold-read addition — without these the verify below fails)**
  `V3_6_TECHNICAL_SPEC.md:999` ("no-op-while-High behavior is this
  spec's reading"), `:1086` and `:1185` (both describe unit-testing
  the "pure no-op-while-High branch"): update each to the toggle
  semantics — the tested branch is the pure always-flip toggle
  (which IS what `lib.rs` unit-tests today); keep the plan-033
  reversal attribution consistent with §7.1.1's note.
- `IMPLEMENTATION_PLAN.md:784` (row starts `:782`): rewrite the
  checklist row to test the CURRENT contract: "the global hotkey
  collapses an auto-expanded card and re-expands on second press".
  Keep it unchecked.
- `docs/TESTING_STRATEGY.md:537` ("hotkey no-op branch" row, added at
  review round 2): rewrite to the shipped toggle semantics (always
  flips; plan-033 universal expand), renaming the bolded row label to
  match (e.g. "**hotkey toggle branch**"). Touch ONLY this row — §0
  counts and everything else in that file stay byte-identical (§0 is
  reconciled once at the end of the whole batch, not here).
**Verify (cold-read: enumerated-survivors form, no judgment calls)**:
`grep -rn "no-op-while-High\|is a no-op" docs/V3_6_TECHNICAL_SPEC.md docs/IMPLEMENTATION_PLAN.md docs/TESTING_STRATEGY.md`
→ the only surviving hits are (a) explicitly-labeled plan-008
HISTORY notes added by this step, and (b) hits unrelated to the
expand hotkey — enumerate every survivor in your report with its
classification. Baseline today: load-bearing hits at V3_6
`:978/:999/:1086/:1185`, IMPLEMENTATION_PLAN `:784`,
TESTING_STRATEGY `:537-538` — after this step, none of those six
describe the current contract. (`TESTING_STRATEGY:520`'s
"`toggle_expanded` is a no-op while the slot is Empty" is a TRUE
statement about the empty-slot branch, not the High guard — it
survives untouched; classify it as (b).)

### Step 3: prototype contradictions
- `status-rail.html:552`: reword to past tense — the page mirrored the
  overlay as of its last sync commit; the banner above it is the
  authority. Do not re-sync the CSS.
- `control-panel.html`: the `<h1>` (`:92`) "9-section… current
  state" → historical framing ("the 9-section window as of its era;
  production now has 10 — see the app"). The `<title>` (`:7`) has
  only the stale count ("9 sections") — fix the count/framing there;
  it does not contain "current state".
- `frontend-ui-consolidated.html` now-playing row: append the
  correction — 103 re-spiked with mediaremote-adapter → GO; built as
  104, polished in 105, both merged 2026-07-22. Keep the 095 NO-GO
  text as history.
**Verify**: `grep -n "current state" prototype/control-panel.html` →
only historical framing; `grep -n "No build planned" plans/frontend-ui-consolidated.html`
→ the line now carries the 103/104/105 correction.

### Step 4: gates
Docs-only, but run `npx vitest run` and `cargo test --locked` (from
`src-tauri/`) once to prove no accidental code touch:
`git diff --stat` must show only the nine in-scope files plus
`docs/TESTING_STRATEGY.md` (whose diff must be the single Step-2
hotkey row).

## Done criteria

(corrected at review round 2 — the old first criterion contradicted
Step 1's preserve-dated-history rule)
- [ ] `grep -rn "seven" CLAUDE.md AGENTS.md docs/ARCHITECTURE.md docs/V5_TECHNICAL_SPEC.md`
      → every surviving command-count hit is one of the two permitted
      historical statements (AGENTS.md:27; ARCHITECTURE.md's dated
      2026-07-18 paragraph, now followed by the new dated amendment),
      each listed in the report — zero PRESENT-TENSE seven-claims
      survive
- [ ] All eleven command names appear in CLAUDE.md's list
- [ ] V3_6 §7.1.1 + IMPLEMENTATION_PLAN checklist +
      TESTING_STRATEGY:537 row describe universal expand /
      hotkey-collapse
- [ ] The three prototype/deck contradictions carry corrections
- [ ] `git diff --stat` touches only in-scope files;
      TESTING_STRATEGY diff is the one hotkey row only

## STOP conditions

- A cited claim is already fixed (report, skip).
- Fixing a doc line would require deciding a *behavior* question
  (nothing here should — this is transcription, not design).

## Maintenance notes

- The command list will drift again; CLAUDE.md's real rule ("never add
  a `#[tauri::command]` without adding it to build.rs") is the part
  that matters and is already stated — this plan just fixes the count
  and list around it.
