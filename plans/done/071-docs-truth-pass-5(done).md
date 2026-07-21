# Plan 071: Docs truth pass #5 — project-state narrative, test count, README table, justfile note

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 958c2f7..HEAD -- CLAUDE.md AGENTS.md docs/TESTING_STRATEGY.md README.md justfile`
> Expected: empty. If any of these changed since this plan was re-baselined,
> compare the "Current state" excerpts against the live files before
> proceeding; on a mismatch, treat it as a STOP condition.
>
> **The sequencing constraint is now satisfied — no need to wait for
> anything.** This plan was written to land LAST among the test-adding
> plans 068/072/074 because they move the counts Step 3 re-derives. All
> three landed on 2026-07-21 (`7382607`, `b883479`, `1d399fb`; merges
> `f00fec7`, `2d970ac`, `c3115f0`) and `958c2f7` refreshed
> `docs/TESTING_STRATEGY.md` §0 to match. Step 3 is therefore expected to
> be a pure verify-only no-op — but still re-derive the counts live, never
> from numbers quoted in this plan.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: nothing outstanding. The one ordering constraint (land
  after plans 068/072/074, whose tests move Step 3's counts) is satisfied
  — all three merged 2026-07-21. Still use a live count in Step 3, never
  a number quoted in this plan.
- **Category**: docs
- **Planned at**: commit `f6c2f46`, 2026-07-20; re-baselined to `0b316c8`,
  2026-07-21 (review-plan pass 2 — corrections folded into the body);
  re-baselined again to `958c2f7`, 2026-07-21 (execute-time reconcile —
  068/072/074 landed, §0 counts refreshed, two broken verification
  commands replaced; see the pass-3 note at the end)

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
   RESOLVED in passing** by the executors of plans 080-087 and then
   068/072/074, all of whom kept §0 current as they landed (it reads 428
   rust + 3 doc-tests / 183 frontend as of `958c2f7`, with per-plan
   attribution). Step 3 is retained as a verify-only gate rather than
   dropped, because this section has a documented recurring-drift history
   (plan 046 found real drift here; a later review-plan pass had to
   reverse a *false-positive* drift claim in the same plan) — precision
   matters here specifically, and a cheap re-count is the whole cost.

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
  `958c2f7` the rust row opens `| rust unit/integration | 428 tests —
  poller 55 (…) |`, followed by a doc-tests row (3) and a frontend row
  (183 tests), each with per-plan attribution parentheticals. Plans
  068/072/074 have already landed AND `958c2f7` already folded their
  tests into this row with attribution (settings 49→54, queue 71→72,
  weather_poller 19→21). **Every figure was re-verified live against
  `958c2f7` while reconciling this plan** — total 428, doc-tests 3,
  frontend 183, and all seventeen per-module counts match the table
  exactly. Step 3 is therefore expected to be a verify-only no-op:
  confirm and write nothing. Still re-derive it yourself with
  `cargo test --locked -- --list` grouped by module — never trust a
  number quoted in this plan and never `grep`-estimate; this section's
  own history (plan 046) is a documented cautionary tale about exactly
  that mistake. Note the listing includes the 3 doc-tests (they appear
  under an `src/…` prefix), so the raw `grep -c ': test$'` total is
  **431**, i.e. 428 + 3 — do not report 431 as the unit/integration
  count.

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
| Confirm doc files exist | `ls docs/design/ docs/recipes/` | 8 files in `docs/design/` (confirmed at `958c2f7`; re-confirm — more may land), `kuma-webhook.md` in `docs/recipes/` |

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
readable, not become a changelog.

The bullets below are in **landing order, not plan-number order** — plan
numbers reflect when a plan was *written*, not when it shipped, so
068/072/074/075 land after 087 here. That's correct; write the narrative
in the order given rather than "fixing" it into numeric sequence. Cover:
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
  the cli script (`README.md:33-35` already documents it)
- plan 063 — notch-mode idle rail clamps to the cutout width, plus the
  shared `__NOTCHTAP_MODE__`/`__NOTCHTAP_CUTOUT_WIDTH__` boot-fact
  eval-splice channel
- the 064/066/067/070 hardening quartet, one clause total (Topic
  supersede meta freeze, cmux `ttl_secs` validation, rotation-order heal
  dedup, `/notify` ingest logging). These four are the quartet — 068 is
  NOT one of them (it landed separately, see the next bullet); an
  earlier note in this file miscounted this group
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
- plans 068/072/074, one clause total, landed 2026-07-21 — mostly test
  backfill (`build_test_event`'s five per-source arms; the weather
  rain-lookahead minute-rounding and day-rollover boundaries), plus one
  small defensive queue fix: a cross-tier Topic supersede now honors
  `max_queued_per_tier` instead of skipping the cap check, dropping the
  fresh content rather than evicting. Flag that last one as **latent —
  zero production behavior change today**, since no producer currently
  varies priority per Topic; that framing matters so a future reader
  doesn't go hunting for a user-visible symptom that never existed.
  Follow the low-narrative-weight precedent set for 047/048 above —
  one clause for all three, not a paragraph
- plan 075 — a docs-only toolchain spike (TypeScript 7 / Vite 8 /
  Vitest 4 trial bump in a throwaway worktree). **Verdict was GO, but
  nothing was adopted** — `package.json` is untouched and adoption is a
  separate unwritten plan. Say both halves explicitly: the spike result
  lives in `plans/done/075-frontend-toolchain-major-bump-spike(done).md`,
  not in `docs/design/`, so it does not belong in the 049-053 spike
  clause above. One sentence is enough; the point is to stop a future
  session from either re-running the spike or assuming the bump shipped

Additionally, in BOTH files' commands section, fix the now-false cli
claim "flags only — there is no positional form" (`CLAUDE.md:118-119`,
`AGENTS.md:117-118`) — plan 058's `run` subcommand ended flags-only.
Reword to mention the `run` subcommand while keeping the rest of the
bullet intact. `README.md:33-35` documents the real behavior and is the
wording to match: `notchtap run -- pnpm build` wraps a long-running
command and pushes a completion card when it finishes.

⚠️ **The claim is line-wrapped across two lines** — "…flags only — there
is no" ends the first line, "positional form; the cli is a committed
shell script at repo root" begins the next. A single-line
`grep "there is no positional form"` therefore matches NOTHING and will
fool you into thinking the claim is already gone. Verify with
`grep -n "flags only\|positional" CLAUDE.md AGENTS.md`, which finds it in
both files today.

Apply the identical addition to both files — they've been kept in sync
until now (only the kuma-webhook gap broke that), so diverging them
further defeats the point of Step 2.

**Verify**: the two files hard-wrap at *different* columns, so a plain
line-by-line `diff` of these paragraphs reports ~25 differing lines even
when the prose is identical — it is pure wrap noise and tells you
nothing. Compare whitespace-normalized instead:

```
diff <(sed -n '/project state/,/^## source of truth/p' CLAUDE.md | tr '\n' ' ' | tr -s ' ' | fold -w 90) \
     <(sed -n '/project state/,/^## source of truth/p' AGENTS.md | tr '\n' ' ' | tr -s ' ' | fold -w 90)
```

Run this BEFORE editing to see your starting point: today the only
substantive difference is the kuma-webhook sentence that Step 2 adds
(everything after it merely shifts position because that sentence is
missing). After Step 1 + Step 2, this diff should be empty or show only
trailing-whitespace/fold artifacts. If it shows a real sentence present
in one file and absent from the other, you have diverged them — fix
before moving on.

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

**This step is expected to require NO edit.** It was already verified as
a no-op at `958c2f7` while reconciling this plan. Your job is to confirm
that independently, not to take it on faith — and not to invent a change
just to feel productive. Writing nothing here is the correct outcome.

Count live:

```
cd src-tauri
cargo test --locked -- --list 2>/dev/null | grep -c ': test$'
cargo test --locked -- --list 2>/dev/null | grep ': test$' \
  | sed 's/^\([a-z_]*\).*/\1/' | sort | uniq -c | sort -rn
```

Then `npx vitest run` from the repo root for the frontend total.

Expected, all three already confirmed against `958c2f7`:

| figure | expected | note |
|---|---|---|
| raw `grep -c` total | **431** | includes the 3 doc-tests, which list under an `src/…` prefix |
| rust unit/integration | **428** | 431 − 3; this is the number §0's rust row states |
| doc-tests | **3** | its own §0 row |
| frontend (vitest) | **183** | 13 files |

Per-module (the second command; `lib`'s 13 list under a `tests` prefix
because they live in a `mod tests` inside `lib.rs` — §0 labels that row
`lib`, which is correct): queue 72, poller 55, settings 54, http 36,
hover 30, rss_poller 28, notifier 26, config 23, event 22,
weather_poller 21, lib 13, presentation 11, engine 11, crests 8,
status 7, logging 7, net 4. These sum to 428.

If every figure matches, write nothing and move to Step 4. Only if
something genuinely differs (i.e. work landed after `958c2f7`) update
the row with live numbers and extend the attribution parentheticals in
the established per-plan style — then re-run the count and confirm what
you wrote matches, never transcribing from this plan or `grep`-estimating.

**Verify**: state in your report the four figures you actually observed, and whether you edited §0 (expected: no). If you did edit it, re-run the count afterward and confirm your written numbers match exactly.

### Step 4: Add the 3 missing README docs-table rows

Add rows for `AGENTS.md` (next to the existing `CLAUDE.md` row),
`docs/recipes/kuma-webhook.md`, and `docs/design/` (one summary row
covering the directory, not 8 individual rows — e.g. "spike/design docs
from `/improve` sessions — read alongside `plans/README.md`"), matching
the existing table's column format and terse description style.

**Verify**: `grep -c "AGENTS.md\|docs/recipes\|docs/design" README.md` — this returns **0 today** (measured at `958c2f7`; the table genuinely has none of the three), so after your edit it must return **at least 3**. The table has 12 rows now and should have 15.

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

**Verify**: `grep -c "see note below" justfile` → **0** (it returns **1** today). Then `grep -B1 -A2 "cargo-audit" justfile` to eyeball that the replacement note is self-contained and reads correctly.

## Test plan

No automated tests apply — this is a pure documentation pass. Verification
is the grep/diff checks embedded in each step above, plus the live
`cargo test --locked -- --list` / `npx vitest run` re-counts in Step 3.

## Done criteria

- [ ] CLAUDE.md and AGENTS.md's "later landings" clauses both mention plans 044/045/047/048, commit `fb4acce`, 058, 063, the 064/066/067/070 quartet, 068/072/074, 075, 076/077/078, the 080-085 batch (incl. the `dedup_eq` invariant), and 086/087 — and stay textually in sync with each other (Step 1's normalized diff check)
- [ ] Neither file still claims the cli is "flags only" / has "no positional form": `grep -c "positional form" CLAUDE.md AGENTS.md` → both **0** (each returns **1** today). ⚠️ Use exactly this pattern — the longer `"there is no positional form"` returns 0 *before any edit* because the sentence is line-wrapped, so it can never detect whether you did the work
- [ ] Both files mention the kuma-webhook recipe: `grep -c "kuma-webhook.md" CLAUDE.md AGENTS.md` → both ≥1 (today: CLAUDE 1, AGENTS **0**)
- [ ] `docs/TESTING_STRATEGY.md` §0's counts match a fresh live count exactly (Step 3) — expected outcome is **no edit at all**; 428 rust / 3 doc-tests / 183 frontend
- [ ] `README.md`'s docs table includes AGENTS.md, docs/recipes/, and docs/design/: `grep -c "AGENTS.md\|docs/recipes\|docs/design" README.md` → ≥3 (today **0**)
- [ ] `justfile`'s cargo-audit comment is self-contained, no dangling "see note below": `grep -c "see note below" justfile` → **0** (today **1**)
- [ ] No files outside the 5 listed in Scope modified, plus `plans/README.md` (`git status` — that status-row update is expected and is the standard bookkeeping exemption every plan in this index carries; everything else outside the 5 is out of scope)
- [ ] `plans/README.md` status row for 071 updated

## STOP conditions

- CLAUDE.md and AGENTS.md's project-state paragraphs have diverged more
  substantially than just the kuma-webhook sentence by the time you read
  them (i.e. someone edited one but not the other since planning) —
  reconcile by reading both in full before editing either, don't assume
  this plan's diff-based check from planning time still holds.
- The live test count differs from §0's recorded 428+3/183 AND no
  test-adding work landed after `958c2f7` to explain it — recount
  carefully rather than guessing. If something seems off (a module count
  went DOWN, or the per-module counts don't sum to the total), STOP and
  report rather than writing a number you're not confident in. Remember
  the raw listing total is 431 because doc-tests are included; that is
  expected, not a discrepancy.
- Any of the four "today" baselines in the done criteria doesn't match
  what you actually observe before editing — i.e. `positional form` is
  not 1/1, `kuma-webhook.md` is not 1/0, the README grep is not 0, or
  `see note below` is not 1 in the justfile. Each was measured at
  `958c2f7`; a mismatch means someone edited these files after this plan
  was reconciled, so re-read the live files before touching anything.

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

**Reconcile pass 3 (2026-07-21, at `958c2f7`, immediately pre-dispatch)**:
run as the `execute` precondition check, not a scheduled review. Drift
since `0b316c8` was one line in `docs/TESTING_STRATEGY.md` — the §0 count
refresh that shipped with 068/072/074. Three defects found and fixed,
two of which would have produced a false PASS:

1. **The cli-claim done criterion was vacuous.** It grepped for
   `"there is no positional form"`, but that sentence hard-wraps between
   "there is no" and "positional form", so the pattern matched nothing
   *before* any edit — an executor that did nothing would have passed it.
   Replaced with `"positional form"` (1 in each file today, must reach 0).
   Step 1 now carries an explicit warning about the wrap, since the trap
   catches a reader confirming the claim exists just as easily as one
   confirming it's gone.
2. **Step 1's sync-verify was noise.** CLAUDE.md and AGENTS.md wrap at
   different columns, so a plain `diff` of the project-state paragraphs
   reports ~25 differing lines when the prose is identical. Replaced with
   a whitespace-normalized comparison, plus the instruction to run it
   *before* editing to establish the baseline.
3. **Step 1 asserted "068 is still TODO".** It landed (`7382607`), as did
   072 (`b883479`) and 074 (`1d399fb`). The instruction is corrected and
   the narrative list gained two bullets: one for 068/072/074 (grouped as
   test backfill + the latent queue cap fix, following the 047/048
   low-weight precedent) and one for 075's GO-but-unadopted toolchain
   spike.

Also re-verified every remaining premise live rather than trusting the
prior pass: items 1/3/4/5 all still valid (AGENTS.md kuma count 0,
README docs-table refs 0 of 3, justfile's dangling "see note below"
present, both project-state paragraphs stale past 042), 8 files in
`docs/design/`, and the `run` subcommand documented at `README.md:33-35`.
Every done criterion now states its measured pre-edit baseline, so each
one can distinguish "done" from "not attempted" — that property was
missing from three of them.

---

## EXECUTED + APPROVED after one revision round (2026-07-21)

Dispatched to an executor subagent in an isolated worktree (branch
`worktree-agent-a3ab379cf4936c614`, commits `b6a5584` + `29dcd30`), then
reviewed. Diff is 4 files, +103/−7: `CLAUDE.md`, `AGENTS.md`, `README.md`,
`justfile`. `docs/TESTING_STRATEGY.md` was correctly NOT modified —
Step 3 was a genuine no-op, as predicted.

**Revision the reviewer required**: the first pass rewrote the cli bullet
as "`notchtap run -- pnpm build` wraps a long-running command and pushes
a completion card when it finishes" — true but incomplete. `notchtap:105`
early-exits without pushing when a run *succeeds* in under `run_min_secs`
(line 59, default 15). Since that bullet's stated purpose is "testing the
queue/animation without a real event source," a reader following it with
a fast command would see no card and reasonably conclude the app was
broken. Shipping a fresh inaccuracy into the file agents orient from is
precisely what this plan exists to prevent, so it went back rather than
through. Fixed by appending "(skipped for successful runs under
`--min-secs`, default 15s; a failure always pushes)" to both files —
wording that tracks the script's own line-47 comment.

**Reviewer's independent verification** (not taken from the executor's
report): all six machine-checkable criteria re-run in the worktree —
`positional form` 0/0, `kuma-webhook.md` 1/1, README refs 3, `see note
below` 0, §0 untouched. The two files are byte-identical once
whitespace-normalized (project-state section AND the edited cli bullet,
both exit 0). Every checkable claim in the new narrative was verified
against the repo rather than trusted: commit hashes `5c1ca36` ("fold a
same-poll card into the full-time event"), `8743ce6` ("notchtap run —
long-command finisher subcommand") and `fb4acce` ("heal a rotation_order
missing a source at load time") all exist and match their descriptions;
`plans/done/075-…(done).md`, `docs/design/hover-cursor-tracking.md` and
`docs/recipes/kuma-webhook.md` all exist; the justfile's new claim that
CI runs `rustsec/audit-check` is true (`.github/workflows/ci.yml:35`);
and `--min-secs` is the exact flag literal (`notchtap:69`). `git
merge-tree` confirmed a clean merge onto master.

**Notable**: the reconcile pass above was not bookkeeping — two of this
plan's verification commands would have produced a false PASS, and the
cli-claim criterion in particular could never have detected whether the
work was done. Worth remembering that a grep-based criterion over
hard-wrapped prose is only as good as the wrap points; prefer the
shortest distinctive substring that cannot straddle a line break.

**MERGED** into master 2026-07-21 and pushed, at the operator's explicit
instruction.
