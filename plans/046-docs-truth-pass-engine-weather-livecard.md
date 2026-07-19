# Plan 046: docs truth pass — Engine/weather/live-card narrative, stale citations, justfile discoverability

> **Executor instructions**: Follow this plan step by step. This is a
> docs-only plan — no source code changes. Run every verification
> command and confirm the expected result before moving to the next
> step. If anything in "STOP conditions" occurs, stop and report — do
> not improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- CLAUDE.md AGENTS.md README.md docs/V3_6_TECHNICAL_SPEC.md docs/TESTING_STRATEGY.md docs/IMPLEMENTATION_PLAN.md`
> If any of these files changed since this plan was written, re-read the
> live content at the cited lines before editing — this plan quotes
> exact current text for several of its steps, and a docs edit is only
> as good as the text it's replacing being the text that's actually
> there.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW — prose-only edits, no behavior or code touched.
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

Six small, independently-cheap doc-accuracy gaps accumulated since the
last docs truth pass (plans 004/026), all touching adjacent or
overlapping files, bundled here in one plan following this repo's own
established precedent for grouping this class of finding (see plans
004, 026 in `plans/README.md`). The most consequential:
`CLAUDE.md`/`AGENTS.md`'s "project state" paragraph — the primary
onboarding narrative for any future agent session, and the file
`CLAUDE.md`'s own header says to read before implementation work —
stops at plan 020 (2026-07-18) and never mentions the Engine module
(plan 037, a major architectural change that deleted `lib.rs`'s
`spawn_heartbeat`/`enqueue_and_emit`/`enqueue_and_fan_out`), the weather
source (plan 040), or the ESPN live-match scoreboard card (plans
039/041/042) — 22+ shipped plans across three full audit/design
sessions the paragraph is silent on. The rest are smaller but real:
a working spec citing a deleted function, a stale "four sources" claim,
an off-by-one test count, missing manual-checklist rows for two shipped
opt-in features, and a one-command CI-mirror tool (`justfile`) with zero
discoverability from the human-facing `README.md`.

## Current state

All six sub-findings, with the exact current text and exact fix:

### 1. `CLAUDE.md` and `AGENTS.md` — stale project-state paragraph

`CLAUDE.md:5-39` (section header `## project state`) and
`AGENTS.md:5-37` (identical section, minus one sentence about the kuma
recipe that only `CLAUDE.md` has) both end with:

```
plan 020 (`9774930`, 2026-07-18)
added a seventh invoke command, `get_default_config` — seven invoke
commands total as of that date. decisions in `docs/ARCHITECTURE.md`
§17, plan in `docs/IMPLEMENTATION_PLAN.md` §4.5/§4.6, contract in
`docs/V5_TECHNICAL_SPEC.md`. the tauri/rust/web project lives at repo
root alongside `docs/` — the docs folder isn't part of the app build.
the test suite exists and must stay green (`cargo test` from
`src-tauri/`, `npx vitest run` from repo root, all gated by ci) —
current counts live in `docs/TESTING_STRATEGY.md` §0 and only there.
an uptime kuma → notchtap webhook integration recipe (docs only, no
source changes) landed 2026-07-17 at `docs/recipes/kuma-webhook.md` —
verified working end-to-end against kuma v2.4.0.
remaining open work: the manual checklist rows in
`docs/IMPLEMENTATION_PLAN.md` §6, and whatever `plans/` holds.
```

(`AGENTS.md`'s version is word-for-word identical except it omits the
kuma-recipe sentence — confirm this with `diff` before editing, per Step
1 below.)

Neither paragraph, nor any other section in either file, mentions:
plan 037 (the Engine — `src-tauri/src/engine.rs`, the module every Slot
mutation now flows through: `apply`/`apply_blocking`/`read`/`accept`/
`update_live_match`, replacing the deleted `lib.rs` functions
`spawn_heartbeat`/`enqueue_and_emit`/`enqueue_and_fan_out`), plan 040
(the weather source — a fifth `SourceKind`, ambient idle-rail chip,
rain/temp threshold alerts), or plans 039/041/042 (the opt-in ESPN
live-match scoreboard card and its richer presentation).

### 2. `docs/V3_6_TECHNICAL_SPEC.md:708` — cites a deleted function

```
- the heartbeat (`lib.rs`'s `spawn_heartbeat`): after `queue.tick(now)`,
  emit only if the slot state actually changed. Plan 015 replaced the
  original fixed-250ms-interval version of this loop with a
  deadline-sleep-plus-wake design (§4.3); the emission behavior here is
  unchanged by that.
```

`spawn_heartbeat` no longer exists anywhere in `src-tauri/src/lib.rs` —
confirmed via `grep -rn "fn spawn_heartbeat" src-tauri/src/`, zero
matches. Plan 037 moved this loop into `src-tauri/src/engine.rs` as
`Engine::spawn_rotation`; `lib.rs:321-323` carries an explicit comment:
`// plan 037: the rotation loop (formerly spawn_heartbeat) lives //
inside the Engine`.

### 3. `README.md:19-20` — "four sources", now five

```
- accepts pushes from four sources: the `notchtap` cli, cmux's
  notification relay (including claude code "agent needs input"
  alerts), an ESPN live-football poller, and an rss news poller
```

`src-tauri/src/event.rs:83-88`'s `SourceKind` enum has five variants:
`Football, News, Manual, Cmux, Weather` — the weather source (plan 040,
`src-tauri/src/weather_poller.rs`) pushes edge-triggered rain-incoming
and hot/cold alert cards through the same `Engine::accept` path as the
other four.

### 4. `docs/TESTING_STRATEGY.md:19` — rust test count off by one

```
| rust unit/integration | 326 tests — settings 46, queue 65, http 36, notifier 23, rss_poller 28, poller 30, event 19, config 19, weather_poller 13, presentation 11, lib 11, engine 10, status 7, logging 4, net 4 | `cargo test` from `src-tauri/` |
```

`notifier 23` is stale — `src-tauri/src/notifier.rs` has 22
`#[test]`/`#[tokio::test]` functions as of `f2cbae6` (recount, don't
trust this number either — it may have drifted further by execution
time). Every other sub-count in this row was independently spot-checked
and found accurate at the time this plan was written.

### 5. `docs/IMPLEMENTATION_PLAN.md` §6 — no manual-checklist rows for two shipped opt-in features

The "manual — physical hardware, not automatable" checklist
(`docs/IMPLEMENTATION_PLAN.md:754-803`) has zero rows mentioning
`weather_enabled` (plan 040 — does the rain-incoming/hot/cold alert
actually fire on real hardware with real Open-Meteo data) or
`espn_live_card` (plans 039/042 — does the opt-in live-match card
actually rotate/expand/retract correctly on real hardware). Confirmed
via `grep -in "weather\|espn_live_card\|live-match\|live match" docs/IMPLEMENTATION_PLAN.md`
— zero hits anywhere in the file, not just in §6.

### 6. `README.md` — `justfile` undiscoverable

`README.md` has zero occurrences of the word "just" (confirmed via
`grep -in "just" README.md` — no hits). `justfile` (repo root) exists
specifically as "one-command local verification mirroring
`.github/workflows/ci.yml` exactly" (its own header comment, and plan
017's stated purpose). `CLAUDE.md:111-120`/`AGENTS.md`'s equivalent
region already document it in detail, including the `just setup`
fresh-clone step and the "`just` is not installed on the dev machine yet
— `brew install just` first" caveat — but `README.md`'s own "quick
start"/"setup" sections (lines 56-88) never mention it, telling a human
reader to run `npm install`/`cargo test`/`npx vitest run` by hand
instead.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust test recount | `cd src-tauri && cargo test --locked 2>&1 \| tail -20` | prints the real per-module and total counts to reconcile §0 against |
| Confirm `spawn_heartbeat` is gone | `grep -rn "fn spawn_heartbeat" src-tauri/src/` | no matches |
| Confirm `SourceKind` has 5 variants | `grep -n "pub enum SourceKind" -A 6 src-tauri/src/event.rs` | shows `Football, News, Manual, Cmux, Weather` |
| Confirm no "just" in README | `grep -in "just" README.md` | no matches (before your edit) |

## Scope

**In scope** (the only files you should modify):
- `CLAUDE.md`
- `AGENTS.md`
- `README.md`
- `docs/V3_6_TECHNICAL_SPEC.md`
- `docs/TESTING_STRATEGY.md`
- `docs/IMPLEMENTATION_PLAN.md`

**Out of scope** (do NOT touch, even though they look related):
- Any file under `src/` or `src-tauri/src/` — this is a docs-only plan;
  if you find yourself wanting to change source to "make a doc claim
  true," stop and report instead (the direction of the fix here is
  always doc → matches code, never code → matches doc).
- `docs/ARCHITECTURE.md` and `docs/V5_TECHNICAL_SPEC.md` — already
  checked during this audit and found to correctly reference
  `Engine::accept`/current architecture; no changes needed there.
- `docs/archive/` — historical, deliberately not updated.
- `docs/design/*.md` — spike docs, stamped to their own research commit
  by design; not touched by a truth pass.

## Git workflow

- Branch: `advisor/046-docs-truth-pass` (or work directly if the
  operator dispatched you that way).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `docs: sync project-state narrative through plan 042, fix stale citations`.
  Consider one commit for the CLAUDE.md/AGENTS.md narrative update and a
  second for the smaller mechanical fixes (spec citation, README count,
  test count, checklist rows, justfile mention) — matches this repo's
  general preference for reviewable, logically-separated commits, but a
  single docs commit is also acceptable given the small total diff.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Extend `CLAUDE.md`'s and `AGENTS.md`'s project-state paragraph

First run `diff <(sed -n '5,39p' CLAUDE.md) <(sed -n '5,37p' AGENTS.md)`
to confirm exactly how the two paragraphs differ today (expected: only
the kuma-recipe sentence and possibly minor line-wrap differences) —
don't assume, verify, since your edit needs to land in both files
consistently.

Append to both files' `## project state` paragraph (after the existing
"remaining open work" sentence, or wherever reads most naturally without
breaking the paragraph's flow — use your judgment on exact placement,
but keep it in the same section, don't create a new heading), content
covering at minimum:

- **The Engine** (plan 037, merged `6b53c32`): one propagation module
  (`src-tauri/src/engine.rs`) through which every Slot mutation now
  flows (`apply`/`apply_blocking`/`read`/`accept`/`update_live_match`),
  replacing the former `lib.rs` functions `spawn_heartbeat`/
  `enqueue_and_emit`/`enqueue_and_fan_out` (all deleted). Every ingest
  path (http, settings test notifications, both pollers) now routes
  through `Engine::accept`, which also enforces the News-never-to-
  Telegram Connector rule structurally.
- **The weather source** (plan 040): a fifth push source
  (`SourceKind::Weather`), an Open-Meteo poller
  (`src-tauri/src/weather_poller.rs`) with an ambient idle-rail chip and
  edge-triggered rain-incoming/hot/cold threshold alert cards, opt-in
  via `Config.weather_enabled` (default `false`).
- **The ESPN live-match scoreboard card** (plans 039/041/042): an
  opt-in (`Config.espn_live_card`, default `false`) single-updating card
  per live match via Topic supersession (`espn:{league}:{match_id}`),
  with per-side card counts and a Clock detail line in its collapsed
  presentation, and scoring plays labeled with ESPN's own event-type
  text (goal/penalty/own-goal).

Match the existing paragraph's voice (lowercase sentence starts after
periods, present-tense "done as of" framing, inline commit-SHA
citations in backticks) — read the surrounding text once more before
writing so the new sentences don't stick out stylistically.

Update the closing "remaining open work" sentence if plan 043's status
(TODO, gated on a live-match Step 0 confirmation) is worth naming
explicitly — optional, use your judgment; the existing generic "and
whatever `plans/` holds" phrasing may already cover it adequately.

**Verify**: `grep -c "engine.rs\|Engine::accept\|weather_enabled\|espn_live_card" CLAUDE.md AGENTS.md`
→ at least one match per file for each of the four terms searched
(adjust the grep to however you actually phrased it, but confirm all
three shipped features are named in both files).

### Step 2: Retarget `docs/V3_6_TECHNICAL_SPEC.md:708`'s stale citation

Replace the `lib.rs`'s `spawn_heartbeat` citation with the current
location. Suggested replacement text (adapt to fit the surrounding
sentence, don't just paste verbatim if it reads awkwardly):

```
- the rotation loop (`engine.rs`'s `Engine::spawn_rotation` — moved
  here from `lib.rs`'s `spawn_heartbeat` by plan 037's Engine
  refactor): after `queue.tick(now)`, emit only if the slot state
  actually changed. Plan 015 replaced the original fixed-250ms-interval
  version of this loop with a deadline-sleep-plus-wake design (§4.3);
  the emission behavior here is unchanged by either that or the plan
  037 relocation.
```

**Verify**: `grep -n "spawn_heartbeat" docs/V3_6_TECHNICAL_SPEC.md` →
either no matches, or a match that explicitly frames it as historical
("formerly", "moved from") rather than as a live citation.

### Step 3: Fix `README.md`'s stale source count and add a `justfile` mention

- Line 19-20: change "four sources" to "five sources" and add weather to
  the list, e.g.: "accepts pushes from five sources: the `notchtap` cli,
  cmux's notification relay (including claude code "agent needs input"
  alerts), an ESPN live-football poller, an rss news poller, and an
  Open-Meteo weather poller (ambient idle-rail chip plus rain/
  temperature threshold alerts)." Adapt wording to fit the surrounding
  list style; don't just append a fragment.
- In the "quick start" or "setup" section (lines 56-88), add a short
  mention of `just test-all` (and `just setup` for a fresh clone) as the
  one-command way to run everything CI runs, cross-referencing
  `justfile` the way `CLAUDE.md`'s commands section already does. Keep
  it brief — a bullet or two, not a full recipe list (that already
  lives in `CLAUDE.md`/`justfile` itself).

**Verify**: `grep -in "five sources" README.md` → 1 match.
`grep -in "just" README.md` → at least 2 matches (the new mentions).

### Step 4: Fix `docs/TESTING_STRATEGY.md` §0's test count

Recount `notifier.rs`'s actual test count
(`grep -c '#\[test\]\|#\[tokio::test\]' src-tauri/src/notifier.rs` from
repo root, or run the full `cargo test --locked` from `src-tauri/` and
read its summary line) and correct both the `notifier` sub-count and the
row's total at `docs/TESTING_STRATEGY.md:19` to match. Do not trust the
number quoted in this plan (22) without re-running the count yourself —
recount, don't trust, per this doc's own stated discipline.

**Verify**: the `notifier` sub-count in the §0 table matches
`grep -c '#\[test\]\|#\[tokio::test\]' src-tauri/src/notifier.rs`'s
output exactly, and the row's total matches a live `cargo test --locked`
run's reported test count.

### Step 5: Add two rows to `docs/IMPLEMENTATION_PLAN.md` §6's manual checklist

In the "manual — physical hardware, not automatable" list
(`docs/IMPLEMENTATION_PLAN.md:754-803`), add two rows following the
existing rows' format (a `- [ ]` bullet, one sentence, referencing the
config flag and what to observe):

- A weather row: with `weather_enabled = true` and real coordinates
  configured, confirm the idle-rail ambient chip shows current
  conditions, and that a real rain-incoming/hot/cold threshold crossing
  produces exactly one alert card (edge-triggered, not repeated every
  poll).
- A live-match-card row: with `espn_live_card = true` during an actual
  live match, confirm the card updates in place through kickoff/goals/
  cards rather than stacking separate cards, and that it correctly
  retires (does not keep cycling) after full-time.

**Verify**: `grep -in "weather_enabled\|espn_live_card" docs/IMPLEMENTATION_PLAN.md`
→ at least one match each, inside §6's checklist section.

## Test plan

N/A — docs-only plan, no automated tests apply. The "verification" for
each step is a grep/diff confirming the specific textual claim, listed
inline per step above.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `grep -c "engine.rs\|weather_enabled\|espn_live_card" CLAUDE.md` ≥ 3,
      same for `AGENTS.md`
- [ ] `grep -n "spawn_heartbeat" docs/V3_6_TECHNICAL_SPEC.md` shows no
      live (non-historical) citation
- [ ] `grep -in "five sources" README.md` → 1 match
- [ ] `grep -in "just" README.md` → ≥ 2 matches
- [ ] `docs/TESTING_STRATEGY.md`'s `notifier` sub-count matches a live
      `grep -c '#\[test\]\|#\[tokio::test\]' src-tauri/src/notifier.rs`
      count, and the row total matches a live `cargo test --locked` run
- [ ] `grep -in "weather_enabled\|espn_live_card" docs/IMPLEMENTATION_PLAN.md`
      → at least one match each
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Any of the six "current state" excerpts above don't match the live
  file content — the docs have drifted further since `f2cbae6`; re-read
  the live text and adapt, but if the underlying claim (e.g. "notifier
  count is off by one") turns out to already be fixed, just skip that
  sub-step and note it in your report rather than re-doing already-done
  work.
- You find yourself wanting to change source code to make a doc's claim
  true, rather than changing the doc to match the code — that's a sign
  the "finding" was actually a real behavior gap, not a docs gap; report
  it instead of silently converting this into a code-change plan.
- `CLAUDE.md` and `AGENTS.md` have diverged in some way beyond the one
  known kuma-recipe-sentence difference — reconcile by reading both in
  full before editing, and keep whatever the intentional
  Claude-vs-Codex-specific differences are (don't force them fully
  identical if one has agent-specific content the other correctly
  lacks).

## Maintenance notes

- This is the fourth docs-truth-pass-style plan in this repo's history
  (after 004, 026, and this repo's general practice of folding small
  doc drift into whichever plan touches the same region). Future
  sessions should keep using this bundling pattern for cheap,
  independent doc-accuracy findings rather than writing one plan per
  one-line fix.
- `CLAUDE.md`/`AGENTS.md`'s project-state paragraph will drift again the
  moment the next non-trivial plan lands (weather/Engine/live-card were
  the last three "big enough to warrant a sentence" landings) — a
  reviewer merging any future architecturally-significant plan should
  consider whether this paragraph needs another append, rather than
  waiting for the gap to reach 20+ plans again before someone notices.
- A reviewer should scrutinize: that the CLAUDE.md/AGENTS.md addition
  reads as a natural continuation of the existing paragraph's voice (not
  a jarringly different style), that the V3_6 spec citation fix doesn't
  accidentally imply the emission behavior itself changed (it didn't —
  only its location), and that the test-count fix in Step 4 was actually
  recounted live rather than just incrementing/decrementing the number
  quoted in this plan by one (which itself may be stale by execution
  time).
