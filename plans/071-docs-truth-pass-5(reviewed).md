# Plan 071: Docs truth pass #5 — project-state narrative, test count, README table, justfile note

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0b316c8..HEAD -- CLAUDE.md AGENTS.md docs/TESTING_STRATEGY.md README.md justfile`
> Expected: empty. If any of these changed since this plan was re-baselined,
> compare the "Current state" excerpts against the live files before
> proceeding; on a mismatch, treat it as a STOP condition. **This plan
> should land LAST** among the still-pending test-adding plans 068/072/074
> if any of them execute concurrently — they bump the counts Step 3
> re-derives live, never from numbers quoted in this plan.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none structurally, but see the drift-check note above —
  run this plan's Step 3 (test counts) AFTER any of the still-pending
  plans 068/072/074 that land first, using a live count, not the numbers
  in this plan
- **Category**: docs
- **Planned at**: commit `f6c2f46`, 2026-07-20; re-baselined to `0b316c8`,
  2026-07-21 (review-plan pass 2 — corrections folded into the body)

## Why this matters

This repo has a well-established, repeated pattern of "docs truth pass"
plans (004, 026, 046 — each caught real narrative/count drift after a
burst of landings). This is truth pass #5, closing four small, independently-
confirmed drift items found during the current audit:

1. **CLAUDE.md/AGENTS.md's "project state" paragraph is stale past plan
   042.** It names plans 037/039/040/041/042 as the most recent landings,
   but plans 044 (P1 bugfix), 045 (nspanel bump + compensating macro
   fix), 047 (test backfill), 048 (`StatusInputs` refactor), and the
   unfiled commit `fb4acce` (`rotation_order` self-heal) have all landed
   since and are absent from the narrative. An agent orienting from these
   files before touching `config.rs`, `poller.rs`, or the nspanel pin has
   no way to learn these landed — risking reverting a deliberate fix
   (e.g. "simplifying" the `rotation_order` heal away, or re-pinning
   nspanel to an older rev, both of which would reintroduce fixed bugs).

2. **`docs/TESTING_STRATEGY.md` §0's test count — original finding now
   RESOLVED in passing** by the executors of plans 080-087, who kept §0
   current as they landed (it reads 420 rust + 3 doc-tests / 183
   frontend as of `0b316c8`, with per-plan attribution). Step 3 is
   retained as a verify-only gate: re-derive the count live at execution
   time, because the still-pending plans 068/072/074 each add tests and
   this section has a documented recurring-drift history (plan 046 found
   real drift here; a later review-plan pass had to reverse a
   *false-positive* drift claim in the same plan) — precision matters
   here specifically.

3. **AGENTS.md is missing the kuma-webhook recipe mention** that
   CLAUDE.md already has (`CLAUDE.md:35-37`) — confirmed via a direct
   diff between the two files; it's the only non-cosmetic difference.
   Codex sessions (which read AGENTS.md) have no record this recipe
   exists, while Claude Code sessions do.

4. **README.md's project-docs table is missing three real, substantial
   things**: `AGENTS.md` itself, `docs/recipes/kuma-webhook.md`, and all
   8 files in `docs/design/` (spike docs from plans 030/031/049-053/086) —
   confirmed present on disk but absent from the table that's supposed to
   be a fresh clone's primary navigation aid.

5. **`justfile`'s `test-all` recipe has a dangling comment** — `# ...
   except cargo-audit — see note below`, but no note follows it anywhere
   in the 53-line file.

Bundling these matches this repo's own established practice (plans
004/014/017/019 explicitly coordinate on touching the same doc regions;
026/046 are themselves bundled multi-item truth passes) rather than
filing five near-trivial single-line plans.

## Current state

- `CLAUDE.md:1-45` and `AGENTS.md:1-56` — both have an identical
  "## project state" paragraph (verified via diff at planning time — the
  kuma-webhook sentence is the only difference) ending with:

  ```
  ...plans 039/041/042 added the opt-in (`espn_live_card`, default
  `false`) espn live-match scoreboard card — one single-updating card per
  live match via Topic supersession (`espn:{league}:{match_id}`), with
  per-side card counts and a Clock detail line in its collapsed
  presentation, and scoring plays labeled with espn's own event-type text
  (goal/penalty/own-goal).
  ```

  CLAUDE.md continues with a "remaining open work" sentence pointing at
  `docs/IMPLEMENTATION_PLAN.md` §6 and `plans/`; AGENTS.md has the
  equivalent. Both files' "later landings the paragraph above predates:"
  clause (CLAUDE.md:38, AGENTS.md's equivalent) currently stops at plan
  042 and needs extending.

- `docs/TESTING_STRATEGY.md` §0 table (starts ~line 15) — as of
  `0b316c8` the rust row opens `| rust unit/integration | 420 tests —
  poller 55 (…) |`, followed by a doc-tests row (3) and a frontend row
  (183 tests), each with per-plan attribution parentheticals. These are
  CURRENT (kept up to date by the plan 080-087 executors), so Step 3 is
  expected to be a verify-only no-op — **unless plans 068 (+~5
  `settings`), 072 (+~2 `queue`), or 074 (+3 `weather_poller`) land
  before you execute**, in which case the counts must be re-derived and
  the attribution sentence extended. Use `cargo test --locked -- --list`
  grouped by module as the source of truth, never a stale number from
  this plan or a `grep` estimate — this section's own history (plan 046)
  is a documented cautionary tale about exactly that mistake.

- `README.md:102-115` — the project-docs table currently lists
  `ARCHITECTURE.md`, `IMPLEMENTATION_PLAN.md`, `TESTING_STRATEGY.md`,
  `V3_6_TECHNICAL_SPEC.md`, `V5_TECHNICAL_SPEC.md`, the archived
  specs/audit artifacts, `CONTEXT.md`, `CLAUDE.md` — 12 rows total,
  missing `AGENTS.md`, `docs/recipes/`, `docs/design/`.

- `justfile:47-48`:

  ```
  # everything CI runs, locally (except cargo-audit — see note below)
  test-all: check-rust test-rust check-web audit-web test-web build-web check-cli check-swift
  ```

  No note about cargo-audit exists anywhere else in the file. The actual
  reason (from plan 017's done-entry in `plans/README.md`): the
  `cargo-audit` binary is absent on the dev machine, so `test-all` omits
  it while CI's `rustsec/audit-check` action runs it instead.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Live rust test count | `cd src-tauri && cargo test --locked -- --list 2>/dev/null \| grep -c ': test$'` (total), and per-module: `cargo test --locked -- --list 2>/dev/null \| grep '^config::' \| grep -c ': test$'` (repeat per module name) | exact current counts — use these, not any number quoted in this plan |
| Live frontend test count | `npx vitest run` (read the summary line) | current total, cross-check against `docs/TESTING_STRATEGY.md` §0's frontend row too if you're touching that row |
| Confirm doc files exist | `ls docs/design/ docs/recipes/` | 8 files in `docs/design/` (as of `0b316c8`; re-confirm — more may land), `kuma-webhook.md` in `docs/recipes/` |

## Scope

**In scope**:
- `CLAUDE.md`
- `AGENTS.md`
- `docs/TESTING_STRATEGY.md` (§0 table only)
- `README.md` (project-docs table only)
- `justfile` (one comment line)

**Out of scope**:
- Any other section of `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md`,
  `docs/V3_6_TECHNICAL_SPEC.md`, `docs/V5_TECHNICAL_SPEC.md` — not touched
  by this pass; a full audit already checked these against current code
  during this session and found them accurate.
- Any source code — this is a pure documentation pass, zero behavior
  change.

## Steps

### Step 1: Extend CLAUDE.md/AGENTS.md's "later landings" clause

In both files, extend the sentence that currently ends at plan 042 (find
it via `grep -n "plans 039/041/042" CLAUDE.md AGENTS.md`) with a compact
paragraph of new sentences, matching the doc's existing terse, lowercase,
plan-number-citing style. Group aggressively — the narrative should stay
readable, not become a changelog. Cover:
- plan 044 (`5c1ca36`) — fixed a same-poll Topic-supersession ordering
  bug that could permanently un-retire a finished match's card
- plan 045 — bumped the `tauri-nspanel` git pin (13mo/39 commits behind)
  and added a compensating `tauri_panel!` macro block for a confirmed API
  break the bump introduced
- plans 047/048 — test backfill and a `StatusInputs` named-field
  refactor (low-narrative-weight, one clause is enough for both)
- the unfiled commit `fb4acce` — `Config::parse` now self-heals a
  `rotation_order` missing a `SourceKind` variant by appending it (name
  it by commit hash, not a plan number, since it never got one)
- one clause noting plans 049-053 are docs-only design spikes in
  `docs/design/`, not shipped behavior (the existing doc already treats
  spikes this way for 030/031, match that precedent)
- plan 058 — landed 2026-07-20, commit `8743ce6`: a `run` subcommand on
  the cli script (`README.md:34` already documents it)
- plan 063 — notch-mode idle rail clamps to the cutout width, plus the
  shared `__NOTCHTAP_MODE__`/`__NOTCHTAP_CUTOUT_WIDTH__` boot-fact
  eval-splice channel
- the 064/066/067/070 hardening quartet, one clause total (Topic
  supersede meta freeze, cmux `ttl_secs` validation, rotation-order heal
  dedup, `/notify` ingest logging). **Do NOT include 068 here — it is
  still TODO** (an earlier note in this file miscounted the quartet)
- plans 076/077/078, one clause total (telegram connector health chip,
  in-app log viewer / Diagnostics settings section, motion library
  dropped from the overlay bundle)
- the 080-085 UI batch, 1-2 sentences total: news card published-time
  meta + full-width expanded summary (080), the TTL progress bar with
  the `ttl_ms`/`remaining_ms` wire fields and the `SlotState::dedup_eq`
  rule — a real contributor-facing invariant worth one explicit clause:
  continuously-varying wire fields must extend `dedup_eq`, never derived
  `PartialEq` (081), weather-alert Meteocons art + mood backdrops (082),
  football backend `EspnMeta`/crest cache/`espn_rich_events` +
  `assetProtocol` scope (083), the live-match compact scorecard (084),
  and the `resting_state: rail|notch` hide-when-idle flag (085)
- plans 086/087 — the hover spike (`docs/design/hover-cursor-tracking.md`)
  and the shipped hover primitive (tracking area + rust-derived card
  rect + `hover-changed` event, merged `0b316c8`); note the hover
  CONSUMER features (TTL-bar pause, weather peek, scorecard reveal, idle
  expand-on-hover) are still unbuilt

Additionally, in BOTH files' commands section, fix the now-false cli
claim "flags only — there is no positional form" (`CLAUDE.md:118-119`,
`AGENTS.md:117-118`) — plan 058's `run` subcommand ended flags-only.
Reword to mention the `run` subcommand while keeping the rest of the
bullet intact.

Apply the identical addition to both files — they've been kept in sync
until now (only the kuma-webhook gap broke that), so diverging them
further defeats the point of Step 2.

**Verify**: `diff <(sed -n '/project state/,/^##/p' CLAUDE.md) <(sed -n '/project state/,/^##/p' AGENTS.md)` — the only difference should be the header line ("Claude Code" vs "Codex") and any file-specific framing already present before this edit; the substantive landing narrative should match.

### Step 2: Add the missing kuma-webhook sentence to AGENTS.md

Confirm CLAUDE.md's existing sentence (`CLAUDE.md:35-37`):

```
an uptime kuma → notchtap webhook integration recipe (docs only, no
source changes) landed 2026-07-17 at `docs/recipes/kuma-webhook.md` —
verified working end-to-end against kuma v2.4.0.
```

Insert the identical sentence into AGENTS.md at the corresponding point
in its project-state paragraph (same relative position as in CLAUDE.md).

**Verify**: `grep -c "kuma-webhook.md" CLAUDE.md AGENTS.md` → both return at least 1.

### Step 3: Fix `docs/TESTING_STRATEGY.md` §0's count

Run `cd src-tauri && cargo test --locked -- --list` and count tests per
module (group by the `module::` prefix before each `test` line) plus the
grand total, and `npx vitest run` for the frontend total. Compare
against §0's recorded figures (420 rust + 3 doc-tests / 183 frontend as
of `0b316c8`). If they match exactly, this step is a verify-only no-op
— write nothing. If they differ (expected only if plans 068/072/074 or
newer work landed first), update the table rows with the live numbers
and extend the attribution parentheticals in the established per-plan
style already used throughout the row.

**Verify**: re-run `cargo test --locked -- --list` and diff your counted numbers against what you wrote in the table — they must match exactly, not be transcribed from this plan or estimated via `grep` (this section's own history shows both mistakes have happened before).

### Step 4: Add the 3 missing README docs-table rows

Add rows for `AGENTS.md` (next to the existing `CLAUDE.md` row),
`docs/recipes/kuma-webhook.md`, and `docs/design/` (one summary row
covering the directory, not 8 individual rows — e.g. "spike/design docs
from `/improve` sessions — read alongside `plans/README.md`"), matching
the existing table's column format and terse description style.

**Verify**: `grep -c "AGENTS.md\|docs/recipes\|docs/design" README.md` → at least 3 new matches beyond whatever was already there.

### Step 5: Fix the justfile's dangling comment

Either add the missing note or drop the dangling clause. Recommended
(shorter, keeps the info where a reader following the pointer would look
— right there in the file):

```
# everything CI runs, locally, except cargo-audit (binary isn't
# installed on the dev machine; CI's rustsec/audit-check action runs it
# instead — see .github/workflows/ci.yml's rust job)
test-all: check-rust test-rust check-web audit-web test-web build-web check-cli check-swift
```

**Verify**: `grep -A1 "except cargo-audit" justfile` → the note is self-contained on the same or next line, no more "see note below" pointing at nothing.

## Test plan

No automated tests apply — this is a pure documentation pass. Verification
is the grep/diff checks embedded in each step above, plus the live
`cargo test --locked -- --list` / `npx vitest run` re-counts in Step 3.

## Done criteria

- [ ] CLAUDE.md and AGENTS.md's "later landings" clauses both mention plans 044/045/047/048, commit `fb4acce`, 058, 063, the 064/066/067/070 quartet, 076/077/078, the 080-085 batch (incl. the `dedup_eq` invariant), and 086/087 — and stay textually in sync with each other (Step 1's diff check)
- [ ] Neither file still claims the cli is "flags only" / has "no positional form": `grep -c "there is no positional form" CLAUDE.md AGENTS.md` → both 0 (Step 1's cli-claim fix)
- [ ] Both files mention the kuma-webhook recipe (Step 2's grep check)
- [ ] `docs/TESTING_STRATEGY.md` §0's rust count matches a fresh `cargo test --locked -- --list` count exactly, re-verified after writing (Step 3)
- [ ] `README.md`'s docs table includes AGENTS.md, docs/recipes/, and docs/design/ (Step 4)
- [ ] `justfile`'s cargo-audit comment is self-contained, no dangling "see note below" (Step 5)
- [ ] No files outside the 5 listed in Scope modified, plus `plans/README.md` (`git status` — that status-row update is expected and is the standard bookkeeping exemption every plan in this index carries; everything else outside the 5 is out of scope)
- [ ] `plans/README.md` status row for 071 updated

## STOP conditions

- CLAUDE.md and AGENTS.md's project-state paragraphs have diverged more
  substantially than just the kuma-webhook sentence by the time you read
  them (i.e. someone edited one but not the other since planning) —
  reconcile by reading both in full before editing either, don't assume
  this plan's diff-based check from planning time still holds.
- The live test count differs from §0's recorded 420+3/183 AND none of
  plans 068/072/074 (or other test-adding work) landed to explain it —
  recount carefully rather than guessing; if something seems off (a
  module count went DOWN, or the total doesn't match the sum of
  per-module counts), STOP and report rather than writing a number
  you're not confident in.

## Maintenance notes

- This is truth-pass #5 in a now well-established series (004, 026, 046,
  and this one) — the next one will likely be needed after another burst
  of ~10 plans land without a dedicated docs pass. No need to schedule it
  proactively; the pattern of "audit finds it stale, files a bundled fix"
  has worked fine three times running.
- If `docs/TESTING_STRATEGY.md` §0 drifts again, prefer a live recount
  command over `grep`-based estimation every time — this section has now
  had two separate false-count incidents (plan 046's real drift, and a
  near-miss false-positive in the same plan's own review pass) from
  exactly that shortcut.

**Review-plan pass (2026-07-21, first pass, at `647f6d0`)**: item-by-item
recheck after plans 076/077/078/080-086 landed. Found items 1/3/4/5 still
valid (CLAUDE.md/AGENTS.md/justfile had zero diff from baseline), item 2
already satisfied by later executors, and the narrative scope grown.
Historical only — its findings were folded into the body by pass 2 below,
which also corrected two errors in it (it called the landed quartet
"064/066/067/068" — the fourth landed plan is 070, 068 is still TODO; and
it located the "flags only" cli claim in CLAUDE.md only — AGENTS.md:117-118
has the identical line).

**Review-plan pass 2 (2026-07-21, at `0b316c8`)**: plan 087 (hover
primitive) executed and merged mid-review (`a5a98b1`, merge `0b316c8`),
moving §0's recorded counts to 420 rust + 3 doc-tests / 183 frontend —
all figures re-checked against the live §0 text this pass. All of pass
1's corrections plus the 087 landing are now folded directly into the
body: drift baseline re-stamped `f6c2f46` → `0b316c8` (expected diff now
empty on all five in-scope files), Step 1's landing list is complete
through 087 and carries the cli-claim fix for both files, Step 3 is
rewritten around the verify-only expectation with 068/072/074 as the
only anticipated count-movers, Step 4/commands table say 8 design docs,
and the done criteria/STOP conditions match. The plan is self-contained
again: execute from the body alone; these notes are history, not
instructions.
