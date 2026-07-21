# Plan 099: Docs and dead-code truth sweep — make every living document agree with shipped reality

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. The reviewer maintains `plans/README.md` — do
> not edit it (its historical session narratives are exempt from this
> sweep by design).
>
> **Worktree preflight (run before anything else)**: agent worktrees can
> branch from a stale HEAD. Run `git log --oneline master ^HEAD`; if it
> prints anything, run `git merge --ff-only master` and confirm it
> succeeds before starting. Then run `npm ci` (fresh worktrees have no
> node_modules).
>
> **Drift check (run second)**: this plan depends on plans 097 and 098
> being MERGED to master first. Verify:
> `git log --oneline master | head -5` must show both a plan 097 and a
> plan 098 merge/commit. If either is absent, STOP — the §0 count
> reconcile (Step 8) would write wrong numbers.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (docs, comments, and one dead-code deletion; no behavior)
- **Depends on**: plans/097-hover-seam-and-config-load-fixes.md, plans/098-below-block-opacity.md (both merged)
- **Category**: docs / tech-debt
- **Planned at**: commit `0056f38`, 2026-07-21

## Why this matters

An audit after the 088–096 wave found the living documentation layer
lagging shipped reality — including one flatly false line in `CLAUDE.md`
(loaded into every future session), a tracker HTML deck stamped
"re-verified 2026-07-21 EOD" that predates every 2026-07-21 merge, seven
suspended plans that read as actionable TODOs though all were superseded
and shipped, and a dead exported function whose doc block describes a
deleted rust mirror. Executors and future sessions steer by these files;
each stale line is a future wrong decision.

Operator decisions (2026-07-21): prototype HTML files get a
**historical banner only** (no content rework); the two `plans/*.html`
decks get fully truth-updated.

## Current state

Shipped reality these edits must reflect (all merged to master
2026-07-21): plan 088/089 notification history (backend + Settings
section), 091 cutout card-assembly, 092 general card + chip language,
093 ALL FOUR hover consumers (TTL-bar hover-pause, idle weather peek,
scorecard reveal-on-hover, idle expand/peek-on-hover), 094 app icon,
095 MediaRemote spike closed with a NO-GO verdict
(`docs/design/now-playing-mediaremote.md`), 096 origin-on-the-wire +
cmux accent. The 079 ledger (19 items) is fully resolved and filed done.

The stale sites, with excerpts from the current files:

1. `CLAUDE.md:22-25`:
   ```
   - the hover primitive shipped (tracking area, rust-derived card rect,
     `hover-changed` event) but every hover CONSUMER feature is still
     unbuilt: TTL-bar pause, weather peek, scorecard reveal, idle
     expand-on-hover.
   ```
   False — all four consumers shipped in plan 093.
2. `src/useStatusState.ts:101-122` — exported `statusRailActive` has NO
   production caller (grep: only its own file, comments, and
   `useStatusState.test.ts`). Its doc block claims a rust mirror
   (`plan 087: mirrored in rust as hover::status_rail_active`) that plan
   093 deleted, and a 270px/460px idle-width split that plan 091
   collapsed. The companion comment at `:36-38` ("statusRailActive is
   false on it, so the idle card keeps its plain-clock form") leans on
   the same deleted behavior.
   Tests: `src/useStatusState.test.ts:5` imports it; `:178-212` is a
   `describe("statusRailActive", ...)` block.
3. `src/lib/presentation.ts:91-93`:
   ```ts
   // Celebration values are the exact CSS class names StatusRailCard.tsx
   // applies to the outer `.rail-card` (styles.css/preview-overlay.css) —
   ```
   `.rail-card` was deleted by plan 091; celebrations now land on
   `.card-assembly`.
4. `src-tauri/src/history.rs:59`:
   ```rust
   /// `dir/history.jsonl`, defaults: 5MB cap, current + one `.1` backup.
   ```
   Wrong — `DEFAULT_MAX_FILES = 2` (`:29`) keeps `.1` AND `.2` (the
   `clear()` loop `1..=max_files` confirms two backups).
5. `src-tauri/src/hover.rs:59-67` — the height-constant comment
   contradicts itself: `:59` ends "…staying safely under what a real card
   of that kind renders — err" while the constants at `:66-67` are
   labeled "conservative estimate". Read `:50-68` and rewrite the prose
   so it consistently says the constants UNDER-estimate real card height
   (the hover rect claims less than the card, never more). Do not change
   the constant values.
6. `docs/design/hover-cursor-tracking.md:336-337`:
   ```
   - `cargo test`/`npx vitest run` should stay at the current counts
     (345 + 3 doc-tests / 124) until an actual build plan lands the
   ```
   Stale point-in-time counts stated as current — rewrite to "(counts as
   of this spike; current counts live in `docs/TESTING_STRATEGY.md` §0)".
7. `plans/suspended/*.md` (7 files) — each still opens with an
   actionable Status (e.g. `043-richer-match-events.md:21`:
   "**TODO — Step 0 CONFIRMED, ready for Step 1.**") and none contains
   the word "superseded" (grep-verified). Supersession map:
   - 043 richer match events → superseded by 079 item 6a, shipped in plan 083
   - 054 app icon → superseded by 079 item 12, shipped in plan 094
   - 055 in-card pause control → superseded by 079 item 11, shipped in plan 092
   - 056 live-match scorecard redesign → superseded by 079 items 3/5/6, shipped in plans 083/084
   - 057 paid sports API evaluation → closed by 079 item 15 decision (stay on ESPN); no successor
   - 060 HUD top treatment → superseded by 079 item 1, shipped in plan 091
   - 069 memoize inline markdown → superseded by and shipped in plan 078
8. `plans/frontend-ui-consolidated.html` — stamped
   "Active (re-verified 2026-07-21 EOD)" at `:165` yet reflects the
   pre-088–096 world. Known-false lines: `:168` (079 row: items
   "8/10/11/12/14 still operator decisions, 1/2 locked-but-unbuilt, 19
   the biggest gap"), `:170` + `:214` + `:221` (089 "Filed, TODO"/"○"),
   `:130/:135/:136/:143-146/:150` (hover consumers "still unwired", icon
   "Tauri scaffold defaults", now-playing "spike to be filed", history
   section TODO), `:137-140` (items 11/14/2/10 "○ open"), `:216-224`
   ("Next moves" listing already-shipped work).
9. `plans/079-checklist.html` — stale badges/notes: `:50` (weather peek
   "hasn't wired it yet"), `:51` (hover trigger "still open"), `:60`
   (now-playing "Spike plan to be filed … no build commitment yet" — it
   was filed AND closed NO-GO by plan 095), `:61` (app icon "○ open"),
   `:62` (TTL hover-pause "this consumer just hasn't wired it yet"),
   `:68` (reveal-on-hover "four consumers still unwired").
10. `prototype/*.html` — 6 files (`control-panel`, `football-card`,
    `news-card`, `notch-states`, `status-rail`, `weather-art`):
    historical design mockups; the shipped implementation deliberately
    diverged (CSS-grid `.card-assembly` vs their absolute positioning).
11. `docs/TESTING_STRATEGY.md` §0 (`:16` region) — the ONLY place test
    counts live; will be stale after plans 097/098 and this plan's test
    deletion.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Rust tests | `cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo test` | all pass; note final counts |
| Frontend tests | `npx vitest run` (repo root) | all pass; note final count |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint (CI gate) | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | success |
| Rust lint | `cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `CLAUDE.md`
- `src/useStatusState.ts`, `src/useStatusState.test.ts`
- `src/lib/presentation.ts` (comment only)
- `src-tauri/src/history.rs` (comment only)
- `src-tauri/src/hover.rs` (comment only)
- `docs/design/hover-cursor-tracking.md`
- `plans/suspended/` — all 7 files (banner insertion only)
- `plans/frontend-ui-consolidated.html`, `plans/079-checklist.html`
- `prototype/*.html` (banner insertion only)
- `docs/TESTING_STRATEGY.md` (§0 counts only)

**Out of scope** (do NOT touch):
- `plans/README.md` (reviewer-maintained), `plans/done/**` (historical
  record), `docs/review-logs/**` (archival), `DESIGN.html` (its known
  stale spots are already recorded in the consolidated deck).
- `src/components/StatusRailCard.test.tsx:335-338` — its `.rail-card`
  mention is a correct HISTORICAL note about the collapsed plan-034
  split; leave it.
- Any rust/TS behavior. The only code change is deleting
  `statusRailActive` + its tests; everything else is comments/docs.

## Git workflow

- Branch: your dispatched worktree branch.
- One or two conventional commits, e.g.
  `docs: truth sweep — CLAUDE.md, decks, suspended banners, stale comments (plan 099)`
  and `refactor(frontend): drop orphaned statusRailActive (plan 099)`.
- Do NOT push.

## Steps

### Step 1: Fix the false CLAUDE.md bullet
Replace the `CLAUDE.md:22-25` bullet with one stating: the hover
primitive AND all four consumers (TTL-bar hover-pause, idle weather
peek, scorecard reveal, idle expand-on-hover) shipped — plan 093; keep
the bullet style of its neighbors.
**Verify**: `grep -n "still\s*unbuilt" CLAUDE.md` → no match.

### Step 2: Delete `statusRailActive` and its stale narrative
In `src/useStatusState.ts`: delete the function (`:112-122`) and its doc
block (`:101-111`); trim the `:36-38` fallback comment so it no longer
references `statusRailActive` or the plain-clock width behavior. In
`src/useStatusState.test.ts`: remove the import usage at `:5` and the
`describe("statusRailActive", ...)` block (`:178-212`).
**Verify**: `grep -rn "statusRailActive\|status_rail_active" src/ src-tauri/` → no matches; `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0.

### Step 3: Comment corrections
- `src/lib/presentation.ts:91-93`: `.rail-card` → `.card-assembly` (and
  keep the "no separate translation table" point).
- `src-tauri/src/history.rs:59`: "current + one `.1` backup" →
  "current + two backups (`.1`, `.2`)".
- `src-tauri/src/hover.rs:50-68`: reconcile the prose per Current-state
  item 5. Constants unchanged.
**Verify**: `grep -n "rail-card" src/lib/presentation.ts` → no match; `grep -n "one \`.1\` backup" src-tauri/src/history.rs` → no match; `cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo build` → exit 0.

### Step 4: Spike-doc count rephrase
Apply Current-state item 6 to `docs/design/hover-cursor-tracking.md:336-337`.
**Verify**: `grep -n "345 + 3" docs/design/hover-cursor-tracking.md` → the remaining text (if any) is explicitly framed as a point-in-time snapshot deferring to §0.

### Step 5: Superseded banners on the 7 suspended plans
Immediately under the H1 of each `plans/suspended/*.md`, insert a
blockquote banner, e.g. for 043:
`> **SUPERSEDED — do not execute.** Folded into 079 item 6a and shipped by plan 083 (2026-07). Kept for historical context; see plans/README.md.`
Use the per-file map in Current-state item 7 (057's says "closed by
decision — stay on ESPN — no successor plan"). Do not edit anything
below the banner.
**Verify**: `grep -Lil "SUPERSEDED" plans/suspended/*.md` → no output (every file matches).

### Step 6: Truth-update the two living decks
`plans/frontend-ui-consolidated.html`: correct every line in
Current-state item 8 — 079 row: all 19 items resolved, plan retired to
done; 089: DONE/MERGED; hover consumers/icon/now-playing/history: shipped
(093/094/095-NO-GO/088+089); replace "Next moves" with the actual
remaining work: operator's batched manual smoke hour, deferred
BrightData token rotation (065), toolchain-adoption plan still unwritten.
Update the `:165` stamp to "re-verified 2026-07-21 (post-096)".
`plans/079-checklist.html`: flip the six stale badges/notes per
Current-state item 9 (mark them shipped with their plan numbers; item
16's note → "spike run and closed NO-GO, plan 095"). Update its header
stamp the same way.
**Verify**: `grep -n "Filed, TODO\|still unwired\|hasn't wired\|to be filed" plans/frontend-ui-consolidated.html plans/079-checklist.html` → no matches.

### Step 7: Historical banners on the prototypes
At the top of `<body>` in each of the 6 `prototype/*.html` files, insert
one identical banner div:
`<div style="background:#7a2d2d;color:#fff;padding:6px 12px;font:12px/1.4 monospace">HISTORICAL PROTOTYPE — design exploration that preceded implementation. The shipped overlay deliberately diverged (CSS-grid .card-assembly — see src/styles.css). Not a reference for current behavior.</div>`
No other edits to these files.
**Verify**: `grep -l "HISTORICAL PROTOTYPE" prototype/*.html | wc -l` → 6.

### Step 8: Reconcile §0 test counts
Run both suites (commands table), then update the counts in
`docs/TESTING_STRATEGY.md` §0 to the numbers you observed (rust tests +
doc-tests, frontend tests). Do not restate counts anywhere else.
**Verify**: the §0 numbers equal the just-observed suite output exactly.

## Test plan

No new tests. Step 2 deletes one describe block — the frontend count
drops accordingly and Step 8 records the new totals. Full gates run at
the end (Done criteria) to prove the sweep changed no behavior.

## Done criteria

ALL must hold:

- [ ] All per-step greps above return the stated results
- [ ] `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` (from `src-tauri/`) clean
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` clean
- [ ] §0 counts match the observed suite output
- [ ] `git status` shows no modified files outside the in-scope list

## STOP conditions

Stop and report back (do not improvise) if:

- Plans 097/098 are not both on master (drift-check gate).
- `statusRailActive` turns out to HAVE a production caller your grep
  finds outside test files.
- Any excerpt in Current state no longer matches the live file (line
  drift beyond ±5 lines is fine to re-locate by content; content
  mismatch is not).
- A deck line you're told to correct appears to be TRUE against master
  (e.g. you cannot find the shipped feature it claims is missing) —
  report, don't guess.

## Maintenance notes

- The two decks self-describe as living trackers; whoever files a future
  plan should update them in the same commit or accept they'll rot again.
  If nobody wants that duty, demote them to dated snapshots like the
  prototypes.
- `CLAUDE.md` deliberately stays thin (no per-plan history) — this sweep
  fixes its one false claim, it does not add new status lines.
- Reviewer: check Step 2 didn't disturb `useStatusState`'s validator
  logic (pure deletion), and spot-check three suspended banners and both
  deck stamps.
