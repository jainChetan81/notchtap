# Plan 071: Docs truth pass #5 — project-state narrative, test count, README table, justfile note

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- CLAUDE.md AGENTS.md docs/TESTING_STRATEGY.md README.md justfile`
> If any of these changed since this plan was written, compare the
> "Current state" excerpts against the live files before proceeding; on a
> mismatch, treat it as a STOP condition. **This plan should land LAST**
> among plans 064-070 if any of them land first — several of those bump
> test counts that this plan's Step 2 needs to re-derive live, not from the
> numbers quoted below (which are the pre-064..070 baseline).

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none structurally, but see the drift-check note above —
  run this plan's Step 2 (test counts) AFTER any of plans 064/066/067/068
  have landed, using a live count, not the numbers in this plan
- **Category**: docs
- **Planned at**: commit `f6c2f46`, 2026-07-20

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

2. **`docs/TESTING_STRATEGY.md` §0's rust test count is off by one.**
   The table says 331 total / config 19; a live count is 332 / config 20
   — the extra test is `config::tests::rotation_order_missing_a_source_is_healed_by_appending_it`,
   added in `fb4acce` and never folded back into this doc. This section
   has a documented recurring-drift history (plan 046 found real drift
   here; a later review-plan pass had to reverse a *false-positive* drift
   claim in the same plan) — precision matters here specifically.

3. **AGENTS.md is missing the kuma-webhook recipe mention** that
   CLAUDE.md already has (`CLAUDE.md:35-37`) — confirmed via a direct
   diff between the two files; it's the only non-cosmetic difference.
   Codex sessions (which read AGENTS.md) have no record this recipe
   exists, while Claude Code sessions do.

4. **README.md's project-docs table is missing three real, substantial
   things**: `AGENTS.md` itself, `docs/recipes/kuma-webhook.md`, and all
   7 files in `docs/design/` (spike docs from plans 030/031/049-053) —
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

- `docs/TESTING_STRATEGY.md:15` — the §0 table row (rust unit/integration
  count):

  ```
  | rust unit/integration | 331 tests — settings 46, queue 65, http 36, notifier 23, rss_poller 28, poller 32, event 19, config 19, weather_poller 16, presentation 11, lib 11, engine 10, status 7, logging 4, net 4 | `cargo test` from `src-tauri/` |
  ```

  Live at planning time (`cargo test --locked -- --list`, grouped by
  module): 332 total, config 20 — every other per-module count matches
  live exactly. **Re-run this count yourself before editing** — if plans
  064/066/067/068 have landed by the time you execute this, the real
  total will be higher than 332 and several other per-module counts will
  have changed too (064 adds 3 to `queue`, 066 adds 2-3 to `settings`,
  067 adds 1 to `config` on top of the `fb4acce` one, 068 adds 5 to
  `settings`). Use `cargo test --locked -- --list` grouped by module as
  the source of truth, never a stale number from this plan or a `grep`
  estimate — this section's own history (plan 046) is a documented
  cautionary tale about exactly that mistake.

- `README.md:97-113` — the project-docs table currently lists
  `ARCHITECTURE.md`, `IMPLEMENTATION_PLAN.md`, `TESTING_STRATEGY.md`,
  `V3_6_TECHNICAL_SPEC.md`, `V5_TECHNICAL_SPEC.md`, three archived specs,
  `CONTEXT.md`, `CLAUDE.md` — 9 rows, missing `AGENTS.md`,
  `docs/recipes/`, `docs/design/`.

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
| Confirm doc files exist | `ls docs/design/ docs/recipes/` | 7 files in `docs/design/`, `kuma-webhook.md` in `docs/recipes/` (re-confirm counts — more may have landed) |

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
it via `grep -n "plans 039/041/042" CLAUDE.md AGENTS.md`) with 2-3 more
sentences, matching the doc's existing terse, lowercase, plan-number-
citing style. Cover:
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
  `docs/design/`, not shipped behavior (lower narrative priority — the
  existing doc already treats spikes this way for 030/031, match that
  precedent)
- plan 058 (`notchtap run`) HAS landed since this plan was written —
  DONE 2026-07-20, commit `8743ce6` (a `run` subcommand on the cli
  script; `README.md:34` already documents it). Mention it as landed.
  (This bullet originally said the opposite — "filed but still TODO, no
  code yet" — corrected in the 2026-07-21 review-plan pass; see the
  note at the end of this file.)

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
grand total. Update the §0 table row with the live numbers. Add one
short sentence after the table (matching the existing per-landing-note
style already there for plans 037/039/040/042) attributing the count
change since the last truth pass to whichever of plans 064/066/067/068
have landed, plus `fb4acce`'s test. If none of 064/066/067/068 have
landed yet when you run this step, the only change is `fb4acce`'s single
test (331→332, config 19→20) — attribute it to that commit by hash.

**Verify**: re-run `cargo test --locked -- --list` and diff your counted numbers against what you wrote in the table — they must match exactly, not be transcribed from this plan or estimated via `grep` (this section's own history shows both mistakes have happened before).

### Step 4: Add the 3 missing README docs-table rows

Add rows for `AGENTS.md` (next to the existing `CLAUDE.md` row),
`docs/recipes/kuma-webhook.md`, and `docs/design/` (one summary row
covering the directory, not 7 individual rows — e.g. "spike/design docs
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

- [ ] CLAUDE.md and AGENTS.md's "later landings" clauses both mention plans 044/045/047/048 and commit `fb4acce`, and stay textually in sync with each other (Step 1's diff check)
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
- The live test count doesn't match your expectations after landing
  064/066/067/068 (if they've landed) — recount carefully rather than
  guessing; if something seems off (a module count went DOWN, or the
  total doesn't match the sum of per-module counts), STOP and report
  rather than writing a number you're not confident in.

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

**Review-plan pass (2026-07-21)**: item-by-item recheck against live
docs at HEAD `647f6d0`, after plans 076/077/078/080–086 + 083/084 all
landed on top of this plan's `f6c2f46` baseline. Drift-check ground
truth: `git diff --stat f6c2f46..HEAD` on the five in-scope files shows
CLAUDE.md, AGENTS.md, and justfile with **zero** diff (items 1/3/5
premises intact), README.md +3 lines (058's `notchtap run` usage,
outside the docs table), and `docs/TESTING_STRATEGY.md` heavily
rewritten (expected — see item 2). Per item:

1. **Item 1 (narrative extension) — STILL VALID, scope grown.**
   044/045/047/048/`fb4acce` remain absent from both files (grep: zero
   hits in either). New drift to fold into the same Step-1 edit: plan
   058 landed (`8743ce6`, bullet above corrected); plan 063 landed
   (notch-mode idle-rail clamp + the shared
   `__NOTCHTAP_MODE__`/`__NOTCHTAP_CUTOUT_WIDTH__` boot-fact channel);
   the 064/066/067/068 bugfix quartet; plan 080 (news card meta +
   full-width expanded summary); 081 (TTL bar — `ttl_ms`/`remaining_ms`
   wire fields and the `SlotState::dedup_eq` rule, a real
   contributor-facing invariant); 082 (weather-alert art); 083
   (football backend — `EspnMeta`, crest cache + `assetProtocol` scope
   in `tauri.conf.json`, `espn_rich_events`); 084 (live-match
   scorecard); 085 (`resting_state` rail|notch hide-when-idle); 086
   spike DONE (docs-only → `docs/design/hover-cursor-tracking.md`) with
   follow-up plan 087 filed. Also: CLAUDE.md's commands section still
   says the cli is "flags only — there is no positional form", which
   058's `run` subcommand has made inaccurate — fold that one-line fix
   in too.
2. **Item 2 (§0 rust count) — ALREADY SATISFIED by later plan
   executors, in passing.** §0 now reads 390 rust + 3 doc-tests / 181
   frontend, dated 2026-07-21, with per-plan attribution through
   083/084. **Live-verified this pass**: `cargo test --locked -- --list`
   → 390 exactly, every per-module figure in the table matches the live
   grouped count (queue 71, poller 55, settings 49, http 36,
   rss_poller 28, notifier 26, config 23, event 22, weather_poller 19,
   lib 13, presentation 11, engine 11, crests 8, status 7, logging 7,
   net 4; sum 390) + 3 doc-tests; `npx vitest run` → 181/181 passed.
   Step 3's *procedure* (re-derive live) stays correct, but its
   contingency numbers (331→332, config 19→20) and its attribution
   list (064/066/067/068 + `fb4acce`) are obsolete — if executed today
   Step 3 is a verify-only no-op unless further plans land first.
3. **Item 3 (AGENTS.md kuma sentence) — STILL VALID.** `grep -c kuma`:
   CLAUDE.md 3, AGENTS.md 0. The two files' project-state paragraphs
   otherwise still differ only by reflow — Step 2 applies as written.
4. **Item 4 (README docs table) — STILL VALID.** Zero table rows for
   AGENTS.md, `docs/recipes/`, or `docs/design/` (grep confirms). One
   count update: `docs/design/` is now **8** files, not 7 —
   `hover-cursor-tracking.md` landed with plan 086; the single
   summary-row approach in Step 4 absorbs this without change.
5. **Item 5 (justfile dangling comment) — STILL VALID.** The
   `# ... except cargo-audit — see note below` line is still at
   `justfile:47` with no note anywhere in the file.

Net: 4 of 5 items survive unchanged (1/3/4/5); item 2 is done but its
verify-live step should still be run at execution time. The plan
remains executable as written given the corrections above; the drift
check's STOP on `docs/TESTING_STRATEGY.md` / README.md diffs is
explained by this note and is not a reason to abort.
