# Plan 153: Produce a keep/cut/demote decision matrix for the four ambient sources

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **This plan produces ONE document and changes NO application code.** You
> may create exactly two files' worth of changes (see Scope). No new file
> anywhere else — not in `src/`, not in `src-tauri/`, not a scratch script
> at the repo root or in `/tmp`. All measurement is shell one-liners.
>
> **You cannot ask the operator anything.** Every question you would want
> to ask becomes a written entry in the document's questions section.
>
> **Drift check (run first)**:
> `git diff --stat acdaeb0..HEAD -- src-tauri/src/config.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/weather_poller.rs src-tauri/src/now_playing.rs src-tauri/src/crests.rs src-tauri/src/event.rs docs/ARCHITECTURE.md docs/TESTING_STRATEGY.md`
> If any changed since this plan was written, re-verify the "Current state"
> figures below against the live code before proceeding.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: LOW (no code changes)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `acdaeb0`, 2026-07-28

## Why this matters

notchtap began as a coding-agent notifier. Over v2–v6 it accreted four
ambient sources — football scores, RSS news, weather, and now-playing
media — each with its own poller, config block, Settings section, and
overlay rendering. v7 then refocused the product on coding agents
(`docs/ARCHITECTURE.md:741`: *"v7 restores the product's original
coding-agent focus"*).

The operator has stated the problem directly: *"we need to trim the
weight now, so many features we have and they are not in sync."* That
observation has not been acted on, and nothing records a decision either
way. Meanwhile every one of those sources is real, working, tested code
that the operator has switched **on** in their live config — so "delete
it" is not obviously right either.

This plan cuts nothing. It produces the missing artefact: a per-source,
evidence-backed cost/benefit matrix and a recommendation, so the operator
makes four small decisions instead of one vague one.

## Current state

### The four sources under review

| source | Origin | rust module(s) | shipped default | operator's live setting |
|---|---|---|---|---|
| Football (ESPN) | `SourceKind::Football` | `poller.rs`, `crests.rs` | `espn_enabled = true` (`config.rs:346-348`) | on |
| News (RSS) | `SourceKind::News` | `rss_poller.rs` | `rss_enabled = false` (`config.rs:381-383`) | on |
| Weather | `SourceKind::Weather` | `weather_poller.rs` | `weather_enabled = false` (`config.rs:401-403`) | on |
| Now Playing | **none — see below** | `now_playing.rs` | `now_playing_enabled = false` (`config.rs:453-455`) | on |

The "live setting" column is from the operator's own
`~/.config/notchtap/config.toml` (keys `espn_enabled`, `rss_enabled`,
`weather_enabled`, `now_playing_enabled` — all `true`). You may read that
file to confirm. **You must never read `~/.config/notchtap/secrets.toml`.**
Step 2's "authenticated or not" question is answered from `config.rs` and
the poller source only.

**Now Playing has no `SourceKind` variant.** The enum
(`src-tauri/src/event.rs:88-...`) is exactly `Football | News | Manual |
Weather | Agent`. Now Playing is an *ambient* surface, not a queued
Origin — it never becomes a Notification. So for Now Playing, Step 2.7's
"Origin token" search and Step 4's "enum variant + exhaustive matches"
line **do not apply**. Its equivalent removal surface is: the
`now_playing_*` config keys, the now-playing fields on the status wire
(`src-tauri/src/status.rs`), `src/components/IdleHoverPeek.tsx`'s media
row, and the vendored adapter under
`src-tauri/vendor/mediaremote-adapter/`. Record that substitution
explicitly in the document rather than leaving a blank row.

### Line counts at `acdaeb0` (`wc -l`, tests included)

```
2806  src-tauri/src/poller.rs        (football)
1822  src-tauri/src/rss_poller.rs    (news)
1272  src-tauri/src/weather_poller.rs
 805  src-tauri/src/now_playing.rs
 401  src-tauri/src/crests.rs        (football team crests)
```

**`#[cfg(test)]` occurrences, verified — read this before Step 2.1.**
Two files have **two** occurrences, and taking the first gives a badly
wrong answer:

| file | `#[cfg(test)]` at | which one starts the test module |
|---|---|---|
| `poller.rs` | 791, **1553** | 1553. The one at 791 is a test-only *helper fn* (`pub fn diff_scoreboard`) with ~760 lines of production code after it. |
| `rss_poller.rs` | 761 | 761 |
| `weather_poller.rs` | 634 | 634 |
| `now_playing.rs` | 478 | 478 |
| `crests.rs` | 197, **217** | 217. The one at 197 is `pub(crate) mod test_support`. |

Always use the **last** occurrence, and report how many there were.

### Per-module test counts already recorded

`docs/TESTING_STRATEGY.md` §0 records these (buried inside one very long
table cell): poller 56, rss_poller 28, weather_poller 29, now_playing 16,
crests 10. §0's header pins them to commit `9ca81f9` (2026-07-26) while
HEAD is `acdaeb0`, so **a mismatch against a live run is expected, not an
emergency** — record both numbers and move on. Do not treat it as drift.

### Decisions already locked — respect, do not re-litigate

`docs/ARCHITECTURE.md` §18 (ESPN live-match card, locked 2026-07-19),
§19 (weather source, locked 2026-07-19), §20 (agent integrations, locked
2026-07-26).

"Locked" means the *design* is settled, not that the feature is
permanent — the Telegram connector was removed a week after shipping. A
recommendation to change *how* one of these works (rather than whether to
keep it) is out of scope; flag it in the document if a verdict would
collide with one.

### What is NOT under review

Origin `Agent` (the product's purpose), Origin `Manual` (the `./notchtap`
CLI push and `notchtap run` — the generic ingestion surface everything
else is measured against), the notification queue, the overlay, the
Settings window, silence/preemption, and the Agent Board.

### The removal precedent

The Telegram connector removal (commit `22ba3a0`, 2026-07-27) is the
template for the Step 4 checklists. Find it with
`grep -n "Telegram connector removed" plans/README.md` and read that
paragraph. The shape: worker + config block + secret fields + one
`#[tauri::command]` removed; the *generic* framework underneath kept for
a future consumer; `ARCHITECTURE.md` given a dated reversal note; the
historical plan files left as filed history, not rewritten.

### The default asymmetry — record it, do not fix it

`espn_enabled` ships `true` while the other three ship `false`.
`config.rs:145-147` states a convention, attached to `now_playing_enabled`:

> Default `false` — same opt-in convention as `weather_enabled`/
> `rss_enabled`: ambient sources never default on top of the app's
> primary agent-notification purpose.

Quote those three lines in the document. Note that the comment cites
weather and RSS as its precedent and says nothing about ESPN — so record
this as "an inconsistency worth an operator decision", **not** as "ESPN
violates a documented repo-wide policy". Do not change any default.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Line count | `wc -l src-tauri/src/<file>.rs` | a number |
| Find test module start | `grep -n '#\[cfg(test)\]' src-tauri/src/<file>.rs` | one or two line numbers |
| Test count per module | `cd src-tauri && cargo test --locked --lib -- --list \| grep -c '^<module>::'` | a number |
| Config fields owned | `grep -c 'pub <prefix>_' src-tauri/src/config.rs` | a number |
| Cross-file reach | `grep -rln '<term>' src src-tauri/src src-tauri/tests` | a file list |
| Baseline rust suite | `cd src-tauri && cargo test --locked` | `0 failed` |
| Baseline frontend suite | `npx vitest run` | all pass |

**Do NOT use `cargo test --locked <module>`.** It substring-matches test
*paths*, so `cargo test poller` also counts every `rss_poller::` and
`weather_poller::` test, and `cargo test crests` also counts `poller.rs`'s
`patch_crests*` tests. The anchored `--list` form above is the correct
one: `^poller::` does not match `rss_poller::`.

If `cargo` is not on PATH, prefix with `PATH="$HOME/.cargo/bin:$PATH"`.
A cold `src-tauri/target/` makes the first `cargo` command take several
minutes; this plan runs it twice.

## Scope

**In scope** (the only files you may create or modify):

- `docs/design/source-scope-review.md` (create)
- `plans/README.md` (status row, plus **extending** the existing note —
  see Step 6)

**Out of scope** (do NOT touch):

- **Every file under `src/` and `src-tauri/`.** This plan removes,
  disables, deprecates and refactors nothing. Not one line.
- **Any new file anywhere else** — no scratch scripts, no `/tmp` helpers.
- `docs/ARCHITECTURE.md` — a decision entry is written *after* the
  operator decides, by a follow-up plan.
- `src-tauri/src/config.rs` defaults — including the ESPN asymmetry.
- `~/.config/notchtap/secrets.toml` — never read it.
- Other `docs/design/*.md` — prior art to read, not files to edit.

## Git workflow

- Branch: `advisor/153-source-scope-review`
- Conventional commits, e.g.
  `docs(design): source scope review — keep/cut matrix for the four ambient sources`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 0: Record the baselines

Run both suites and write the numbers down; you will re-run them at the
end and they must match.

**Verify**: `cd src-tauri && cargo test --locked` → `0 failed` (record the
pass count); `npx vitest run` → all pass (record the count).

### Step 1: Read the prior art

Read these before measuring anything:

- `CONTEXT.md` — the glossary. Note the exact definitions of **Origin**,
  **Rotation Order**, **Recurring**, **Topic**, **Ambient**. Use these
  words in the document.
- `docs/ARCHITECTURE.md` §18 and §19.
- `docs/design/scoreboard-topic-card.md`, `news-ambient-status.md`,
  `now-playing-adapter.md`, `now-playing-mediaremote.md`,
  `per-source-config-consolidation.md`. (All five exist; if one is
  missing, that is a STOP condition.)
- The Telegram removal paragraph in `plans/README.md` (locate with the
  grep in "Current state") and that file's
  "## Findings considered and rejected" section, so you do not re-raise
  something already declined.

### Step 2: Create the document skeleton

Create `docs/design/source-scope-review.md` with **exactly these
headings, in this order**. Every later step fills one of them. Do not
invent additional `##` headings; use `###` freely underneath.

```markdown
# Source scope review — keep/cut/demote for the four ambient sources

**Status**: decision document (plan 153) — recommendations only, zero code changed.
**Researched against**: commit acdaeb0
**Awaiting**: operator decision on four verdicts.

## Prior art consulted

## Cost table

## What each source gives

## If cut — removal checklists

## Verdicts

## Findings

## Questions for the operator

## What this document does not decide
```

Fill "Prior art consulted" now: one bullet per file from Step 1, each
with one line on what it settled. If a file did not exist, say so rather
than inventing its contents.

**Verify**: `grep -c '^## ' docs/design/source-scope-review.md` → `8`

### Step 3: Measure, and fill the Cost table

The "Cost table" section must contain a markdown table with one row per
source and **exactly these columns**:

`| source | rust lines (total) | rust lines (non-test) | `#[cfg(test)]` count | tests | config fields | settings file | overlay files | external dependency | files touched if removed |`

Gather each by running commands, never by estimating:

1. **Total lines** — `wc -l`.
2. **Non-test lines** — subtract the **last** `#[cfg(test)]` line number
   (see the table in "Current state"), and put the occurrence count in
   its own column.
3. **Tests** — the anchored `--list` command. Also record §0's number for
   that module (from "Current state") in parentheses; a mismatch is
   expected and is not drift.
4. **Config fields** — count `pub <prefix>_*` **fields on the struct**,
   not raw grep hits. `grep -n 'pub espn_' src-tauri/src/config.rs` will
   also match nothing outside the struct, but `grep -n 'espn_'` matches
   ~50 lines of doc comments, `default_*` fns and tests. Use the `pub `
   form and list the field names.
5. **Settings file** — which file(s) under `src/settings/sections/`.
   Note: Football/News/Weather each have their own section file; **Now
   Playing does not** — its controls live in `GeneralSection.tsx`, and
   `now_playing_adapter_enabled` is deliberately not in the UI at all
   (`config.rs:153-161`). Write that, not "n/a".
6. **Overlay files** — components under `src/components/` and
   stylesheets under `src/overlay/`. **Attribution rule for shared
   assets**: several files serve multiple sources
   (`StatusRailCard.tsx`, `StatusDots.tsx`, `card-chrome.css`,
   `choreography.css`, `status-dots.css`, `source-identity.css`,
   `idle-peek.css`). List a file under a source **only if that file would
   be deleted were the source removed**. Shared files go in a separate
   "shared, not attributable" list below the table, named once.
7. **External dependency** — this cell is prose, not a number: which
   third-party service, poll interval, authenticated or not, plus any
   out-of-band build step. Note especially that Now Playing requires a
   **vendored framework built by hand** (`just build-media-adapter`,
   `src-tauri/vendor/mediaremote-adapter/`) — a maintenance cost the
   other three do not have.
8. **Files touched if removed** — `grep -rln` for that source's search
   terms across `src`, `src-tauri/src` and `src-tauri/tests`. Use these
   exact terms: Football → `Football|espn_|espn`; News → `News|rss_`;
   Weather → `Weather|weather_`; Now Playing → `now_playing|nowPlaying`.
   Report the file count, and name the non-obvious ones (rotation-order
   default, history labels, source-identity colours, preview fixtures).

Every cell must hold a measured number, a named file, or prose for
columns 9 — never "several", "a few", or a range.

**Verify**: `grep -c '^| ' docs/design/source-scope-review.md` → at least
`5` (header + separator + four source rows).

### Step 4: Fill "What each source gives"

Per source, 2–4 sentences grounded in something checkable:

- What the user actually sees — **name the component that renders it**.
- That the operator has it enabled in their live config (per "Current
  state"; cite `~/.config/notchtap/config.toml`).
- Whether a locked `ARCHITECTURE.md` decision or a design doc records why
  it exists.
- Whether it is load-bearing for anything else — check specifically
  whether the Recurring/Topic supersession machinery in
  `src-tauri/src/queue.rs` and the live-match scorecard
  (`src/components/LiveMatchScorecard.tsx`) exist *only* because of
  football.

Where the deciding factor is whether the operator actually looks at it,
say so explicitly and add it to "Questions for the operator" — do not
guess.

**Verify**: `grep -c '\.tsx\|\.rs' ` over that section returns at least
one citation per source (inspect by eye; each of the four sub-sections
must name at least one file path).

### Step 5: Fill "If cut — removal checklists"

One `###` sub-section per source, each a markdown checklist a future
executor could follow. Cover at minimum:

- Rust modules deleted, and any shared helper that becomes dead.
- Config keys removed, **and the migration question**: what happens when
  a user's existing `~/.config/notchtap/config.toml` still contains them?
  Determine the answer by reading how `config.rs` handles unknown keys —
  do not assume.
- The `SourceKind` variant and every exhaustive `match` over it (`grep`
  the variant to enumerate). **Skip this line for Now Playing** and use
  the substitution named in "Current state".
- `default_rotation_order` (`config.rs:486`).
- Settings section removal and any `#[tauri::command]` that becomes
  unused. **If a command would be removed, the 4-place parity must change
  together**: `settings_commands.rs`'s list *and* its
  `assert_eq!(SETTINGS_COMMANDS.len(), 17)`, `lib.rs`'s
  `generate_handler!`, `capabilities/settings.json`, `build.rs`. This is
  a security-load-bearing allowlist.
- Frontend components, stylesheets, presentation tables, source-identity
  colours, preview fixtures.
- Tests deleted, and the `docs/TESTING_STRATEGY.md` §0 recount that
  follows.
- Docs that assert the feature exists.

End each checklist with a line of the exact form
`**Removal effort: S**` (or `M` / `L`) plus one sentence of
justification.

These checklists describe hypothetical future work. **Write them in the
past-conditional or as noun phrases** ("`LiveMatchScorecard.tsx` and its
test would be deleted"), never as imperatives — an imperative checklist
is the single most likely thing to make a reader start executing.

**Verify**: `grep -c '^\*\*Removal effort: ' docs/design/source-scope-review.md` → `4`

### Step 6: Fill "Verdicts", "Findings", "Questions", and the closing section

**Verdicts** — one per source, each on its own line in exactly this
form so it can be counted:

`**Verdict: KEEP** — Football. <2–4 sentences of reasoning.>`

The four allowed values, all phrased as *recommendations to the
operator*, never as actions to take:

- **KEEP** — the cost is proportionate; recommend leaving it alone.
- **DEMOTE** — recommend keeping the code but shipping it off by
  default, and/or recommend grouping it under a single "Ambient sources"
  Settings heading. Cheapest verdict; often the right one.
- **CUT** — recommend removal; the Step 5 checklist is the work.
- **NEEDS OPERATOR INPUT** — the deciding factor is usage, which the code
  cannot tell you.

Put the four verdict *definitions* in "What this document does not
decide", not next to the verdicts, so the count stays clean.

**Findings** — at minimum the `espn_enabled = true` asymmetry, written
per the "Current state" guidance (quote `config.rs:145-147`; frame it as
an inconsistency for the operator, not a policy violation).

**Questions for the operator** — a numbered list of **exactly 4 to 6**,
each answerable with yes/no or a single choice. Every judgement you could
not ground in the repo belongs here.

**What this document does not decide** — state plainly that no code
changed, that each verdict is a recommendation pending operator sign-off,
that a CUT verdict needs its own follow-up plan, and list the four
verdict definitions.

**Verify**: `grep -c '^\*\*Verdict: ' docs/design/source-scope-review.md` → `4`

### Step 7: Index the review, and re-verify the baselines

`plans/README.md` **already contains** a note reading "153 and 154 change
**no application code** — each produces a decision document…". **Extend
that existing sentence** with a pointer to
`docs/design/source-scope-review.md` and the words "awaiting operator
decision". Do not add a competing section.

Then re-run both suites.

**Verify**: all of
- `grep -c 'source-scope-review' plans/README.md` → at least `1`
- `cd src-tauri && cargo test --locked` → `0 failed`, pass count equals Step 0's
- `npx vitest run` → pass count equals Step 0's
- `git status --porcelain -- src src-tauri` → **no output**

## Test plan

No application code changes, so no new application tests. Verification is
that the suites are **unchanged** against the Step 0 baselines. Record
both baselines and both final counts in your report.

## Done criteria

ALL must hold:

- [ ] `docs/design/source-scope-review.md` exists
- [ ] `grep -c '^## ' docs/design/source-scope-review.md` → `8`
- [ ] `grep -c '^\*\*Verdict: ' docs/design/source-scope-review.md` → `4`
- [ ] `grep -c '^\*\*Removal effort: ' docs/design/source-scope-review.md` → `4`
- [ ] The Cost table has one row per source and no cell reading "several", "a few", "TBD", or a range: `grep -c 'several\|a few\|TBD' docs/design/source-scope-review.md` → `0`
- [ ] The Questions section has 4–6 numbered entries (count by eye)
- [ ] `grep -c 'espn_enabled' docs/design/source-scope-review.md` → at least `1`
- [ ] `git status --porcelain -- src src-tauri` → **no output** (catches untracked new files, which `git diff` would not)
- [ ] `git status --porcelain` lists only `docs/design/source-scope-review.md` and `plans/README.md`
- [ ] `cd src-tauri && cargo test --locked` pass count matches Step 0
- [ ] `npx vitest run` pass count matches Step 0
- [ ] `grep -c 'source-scope-review' plans/README.md` → at least `1`

## STOP conditions

Stop and report back (do not improvise) if:

- A measurement contradicts "Current state" by more than **±10 lines** on
  a line count, or by any amount on a `#[cfg(test)]` occurrence count or
  a config default value.
- The baseline suites in Step 0 do not pass, or `src-tauri` does not
  build. Two Done criteria become unreachable; report rather than
  proceeding.
- One of the five `docs/design/` files listed in Step 1 does not exist.
- You conclude a source cannot be removed without also removing something
  in the "What is NOT under review" list. That is an important finding —
  report it before finishing the document.
- You find yourself editing a file under `src/` or `src-tauri/`, creating
  a scratch script, or changing a default.
- A step's verification fails twice after a reasonable fix attempt.

Note: `plans/README.md` is already modified in the working tree and
plans 152/154/155 are untracked when you start. That is expected, not
drift.

## Maintenance notes

- **This document goes stale the moment a verdict is acted on.** Whoever
  executes a CUT must update its status line to point at the plan that
  did it, the way `plans/README.md` records the Telegram removal.
- **The DEMOTE verdict is the one to scrutinise in review.** It is cheap
  and reversible, which makes it easy to over-apply; a source used every
  day should get KEEP, not a default flip the operator has to undo.
- **Deliberately not in this plan**: consolidating the four Settings
  sections into one "Ambient" section. `docs/design/per-source-config-consolidation.md`
  covers adjacent ground — read and cite it, but implementing it should
  follow the keep/cut decisions, not precede them.
