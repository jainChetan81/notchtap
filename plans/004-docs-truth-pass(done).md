# Plan 004: Make every agent-facing doc and load-bearing comment match shipped reality

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat d4eac73..HEAD -- AGENTS.md CLAUDE.md README.md CONTEXT.md docs/ARCHITECTURE.md docs/IMPLEMENTATION_PLAN.md docs/V3_6_TECHNICAL_SPEC.md docs/TESTING_STRATEGY.md src-tauri/src/lib.rs src-tauri/src/queue.rs src-tauri/src/poller.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.
>
> **Also run `git status` before Step 1**, separately from the drift
> check above — the drift check only diffs against committed history, so
> it will not see uncommitted edits already sitting in the working tree.
> This repo's tree is sometimes shared by concurrent sessions (per prior
> experience). If `git status` shows uncommitted modifications to any
> in-scope file that you did not just make, STOP and report — another
> session may be actively editing it — rather than overwriting or basing
> your edit on a file mid-change.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `d4eac73`, 2026-07-17 (refreshed twice: a
  `review-plan` pass moved the baseline `d40445e` → `efa1bd2`; an
  `execute` precondition check moved it `efa1bd2` → `d4eac73`. See
  "Current state" item 1 and the "Execute-time reconciliation" note in
  Maintenance notes.)

## Why this matters

This repo is developed almost entirely through AI-agent sessions, and
`AGENTS.md`/`CLAUDE.md`/`CONTEXT.md` are the first files every session
reads. A deep audit found ~12 places where those files (and load-bearing
code comments) describe a *superseded* state of the project — including
AGENTS.md telling agents that the settings page "is the next open item"
when it shipped days ago. Each stale claim is a prompt for wasted or wrong
work. This plan is one editing pass that makes the documented state true
again. It changes prose and comments only — zero behavior.

## Current state

House style for all docs in this repo: lowercase prose, dense, decision-
trail-preserving (superseded decisions are kept and marked, not deleted).
Match it.

The stale claims, each verified against the code at `d40445e` and
re-verified against `efa1bd2` during a `review-plan` pass (see the
"Review-plan refresh log" in Maintenance notes):

1. **`AGENTS.md` lines ~9–23** ("project state" paragraph) says v5's
   settings *page* "was held on the ui migration and is the next open
   item". Reality: commits `c662e73` and `1ed2a23` landed the page and its
   redesign on 2026-07-17; `docs/IMPLEMENTATION_PLAN.md:568` says "**all
   five steps landed 2026-07-17**". `CLAUDE.md`'s equivalent paragraph WAS
   updated (it says v5 is "**done**" and lists the page contents) — the two
   files have diverged. Neither mentions the v6 work (commits `e1f1998`,
   `a693cf2`: per-source priority, rotation-order tie-break, cmux origin).
   **Update (this plan was originally planned at `d40445e`; one more
   commit, `efa1bd2` "feat: appearance config hot-apply + per-source test
   notifications", has since landed — still 2026-07-17)**: `efa1bd2` added
   two more invoke commands (`send_test_notification`, `set_appearance`,
   six total now, up from four), an Appearance section with card-shape
   presets, and per-source test-notification buttons. AGENTS.md's
   paragraph was partially updated for this (it now says "six invoke
   commands") but the "is the next open item" sentence is untouched and
   still wrong — confirmed still present verbatim at AGENTS.md:21 as of
   `efa1bd2`. CLAUDE.md's paragraph was NOT updated for `efa1bd2` at all —
   it still describes only the original four invoke commands and, as
   before, omits v6. **The rewritten paragraph in Step 1 must cover
   `efa1bd2` too, not just v5/v5-news/v5.1-open-story/v6** — otherwise
   this plan reproduces the exact staleness pattern it exists to fix.
2. **`AGENTS.md` line ~192**: a trailing section heading
   `## Imported Claude Cowork project instructions` — check whether any
   content follows it in the file; if it is empty, delete the heading.
3. **`CONTEXT.md` lines 5–6**: "decisions live in `docs/ARCHITECTURE.md`
   and `docs/adr/`" — `docs/adr/` does not exist (verify: `ls docs/`).
4. **`CONTEXT.md` Connector entry (lines ~93–97)**: says a Connector
   "receives every accepted Event" and that display rules ("cap, TTL,
   Paused") never apply to it. Two problems: (a) rss/news events are
   deliberately never offered to connectors
   (`src-tauri/src/rss_poller.rs` enqueues without a connector offer;
   `docs/IMPLEMENTATION_PLAN.md:643` records "overlay-only (never offered
   to connectors)") — the glossary states the opposite rule; (b) "TTL"
   contradicts the glossary's own Rotation entry (lines 44–46: "replaces
   the old TTL concept").
5. **`src-tauri/src/lib.rs`, doc comment on `open_settings_window`**
   (was at lines ~573–575 when this plan was first drafted at `d40445e`;
   `efa1bd2` inserted ~30 lines earlier in `run()` for the appearance
   hot-apply emit, so as of `efa1bd2` this comment is at **lines ~604–606**
   — use `grep -n "doesn't exist and the window opens blank" src-tauri/src/lib.rs`
   to find it rather than trusting either line number):
   ```
   /// until IMPLEMENTATION_PLAN.md §4.5's step 5 lands (held for the ui
   /// migration), `settings.html` doesn't exist and the window opens blank —
   /// accepted interim state, documented there.
   ```
   `settings.html` exists at repo root and is fully built.
6. **`src-tauri/src/queue.rs:75–77`** doc comment on
   `with_rotation_order` says "validated as a permutation of all three
   `SourceKind` variants" — there are four (`event.rs`: Football, News,
   Manual, Cmux).
7. **`src-tauri/src/poller.rs:547–552`** doc comment on
   `enqueue_and_fan_out` says "every accepted event goes to every enabled
   connector, always" — false since v5 news (see item 4a).
8. **`README.md:22–24`** "what it does" still describes the retired
   v1–v3 model: "FIFO queue, TTL auto-dismiss", "v3: outbound connectors
   (WhatsApp via Twilio, Telegram)". WhatsApp/Twilio never shipped and was
   explicitly demoted (`docs/ARCHITECTURE.md` §7). The shipped model is a
   permanent single-slot rotating overlay: priority tiers, rotation-order
   tie-break, news cards, settings window, global hotkeys. `README.md:38`
   heading "quick start (once scaffolded)" is also stale (it's built).
9. **`README.md`** has no mention of: the four global hotkeys (⌃⇧N
   expand-toggle, ⌃⇧X dismiss-now, ⌃⇧P pause-toggle, ⌃⇧O open story), the
   log location, or real setup steps (rust toolchain via `rustup`;
   building + symlinking `notchtap-detect` — the config default is
   `/usr/local/bin/notchtap-detect`, see `src-tauri/src/config.rs:132-133`
   (function `default_detect_path`); `jq` + `curl` needed by the
   `notchtap` CLI script; and the first-run behavior: if
   `~/.config/notchtap/config.toml` is absent the app runs with all
   defaults and never creates the file — only a settings-window save
   creates it — see `src-tauri/src/config.rs:267-282` (`Config::load`)).
   *(Corrected during a `review-plan` pass: the original draft cited
   `config.rs:113-115` and `:227-242` for these two facts — both wrong at
   the commit this plan was planned against, not a drift artifact. If
   citing these in the README, verify the line numbers again with
   `grep -n "fn default_detect_path\|fn load" src-tauri/src/config.rs`
   in case the file has moved further by execution time.)*
10. **`docs/ARCHITECTURE.md` §11 (lines ~423–429)**: gives the log path
    as `~/.local/share/notchtap/logs/notchtap.log` "(or
    `~/Library/Logs/notchtap/` on macos)". The code uses ONLY
    `~/Library/Logs/notchtap/notchtap.log` (`src-tauri/src/logging.rs:30`,
    10 MB × 3 rotation). §11 also promises "any react error boundary catch
    or animation failure logs back to the same file via a tauri command" —
    no such command or error boundary exists, and the overlay is
    permanently receive-only (§14/§17), so the promised mechanism is now
    *forbidden* by a locked rule. State reality: frontend errors are
    devtools-only by design.
11. **`docs/IMPLEMENTATION_PLAN.md` §5 (line ~717)**: still lists
    "click-through window (`set_ignore_cursor_events`)" as deferred
    polish — it shipped (`src-tauri/src/lib.rs:418`). Strike it like the
    line above it (`~~...~~` + a note), leaving only the real app icon
    deferred.
12. **`docs/IMPLEMENTATION_PLAN.md` §6 (lines ~762–766)**: unchecked
    manual rows still verify the superseded model: "confirm fifo + cap-3 +
    ttl-dismiss all hold" and "visible items still age out … promote
    fifo". Rewrite in v3.6 single-slot terms (one visible item; waiting
    ordered priority tier → rotation order → arrival; rotation-dismiss),
    mirroring the correct v3.6 rows already at lines ~777–783.
13. **`docs/V3_6_TECHNICAL_SPEC.md` §3.3 (lines ~229–245)**: the
    `NotifyRequest` snippet shows only `{title, body, priority}` and says
    "`priority` absent → `Priority::Medium`". The real struct is at
    **`src-tauri/src/http.rs:98–111`** (confirmed during a `review-plan`
    pass — the original draft cited `:68–81`, which is actually the
    `enqueue_and_emit` function body a few lines above the struct, not
    the struct itself; `http.rs:92–96` also has the `RequestSource` enum
    the struct's `source` field uses). The real struct also has
    `signal: EventSignal` (serde-default) and `source:
    Option<RequestSource>`, and the absent-priority fallback is
    per-source config (`manual_default_priority` / `cmux_priority`) since
    v6. Also nothing documents the v5.1 `link` field on the outbound
    `SlotState::Showing` wire. This spec is a working draft — editing it
    is the designed mechanism.
14. **`docs/TESTING_STRATEGY.md`** violates its own "counts live in §0
    and only here" rule (line ~13) in the body: line ~344 says
    "(`queue.rs`, 27 tests)" (§0 says 36); line ~489 "(`rss_poller.rs`,
    20 tests)" (§0 says 21); lines ~41–42 say "rss_poller 20 above" and
    "the settings 34 above" (§0 says 21/36); lines ~507–508 "part of the
    48 total … StatusRailCard 12" (§0 says 60 total, StatusRailCard 14).
    Replace each in-body number with a pointer to §0 ("see §0"). Also
    §5.1's `lib.rs` entry (lines ~547–550) presents it as a module with
    no test module — it has had 5 hotkey tests since v3.6; reword the
    entry to scope it to the wiring that remains untested ("partially
    tested — hotkey handlers in §4.10; the window/tray/heartbeat wiring
    stays untested by design"). Line ~42's "form + vitest cases held with
    §4.5 step 5" is stale (they landed — 9 tests in
    `src/settings/SettingsApp.test.tsx`); mark landed.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests still green (comment-only edits) | `cargo test` (run from `src-tauri/`) | all pass, 0 failed |
| Frontend unchanged | `npx vitest run` (repo root) | all pass |
| Grep gates | see per-step verifies | as stated |

## Scope

**In scope** (the only files you may modify):
- `AGENTS.md`, `CLAUDE.md`, `README.md`, `CONTEXT.md`
- `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md`,
  `docs/V3_6_TECHNICAL_SPEC.md`, `docs/TESTING_STRATEGY.md`
- `src-tauri/src/lib.rs`, `src-tauri/src/queue.rs`,
  `src-tauri/src/poller.rs` — **comment text only, never code**
- `plans/README.md` (status row)

**Out of scope** (do NOT touch):
- Any executable code, test, or config value — this plan changes zero
  behavior. If a doc fix seems to require a code change, STOP.
- `docs/archive/**` and `docs/review-logs/**` — historical records,
  deliberately frozen.
- `docs/V5_TECHNICAL_SPEC.md` — spot-checked accurate; leave it.
- The §0 counts table in TESTING_STRATEGY.md itself — it is correct;
  only the in-body repetitions are wrong.

## Git workflow

- **Work on `master`.** (Corrected during `execute` dispatch: the plan
  originally said `v3.6-rotating-overlay`, which was the checked-out
  branch when this plan was first drafted. Since then, `master` has
  diverged 3 commits ahead of `v3.6-rotating-overlay` —
  `git merge-base master v3.6-rotating-overlay` = `a693cf2`, which is
  `v3.6-rotating-overlay`'s tip; `master` continued with `d40445e`,
  `efa1bd2`, `d4eac73` on top, none of which reached the feature branch.
  `master` is this repo's actual current branch and its stated default
  branch for PRs — confirm with `git branch --show-current` before
  starting; if it prints anything other than `master`, STOP and report,
  don't guess which branch is intended.)
- Commit message style (match `git log`): lowercase `area: summary`, e.g.
  `docs: sync agents/claude project state, fix stale paths and superseded-model text`.
- Do NOT push.

## Steps

### Step 1: Rewrite the project-state paragraph in AGENTS.md and CLAUDE.md

Write one accurate paragraph and use it in BOTH files (adjusting only the
first line's tool naming). It must state: v1–v5 shipped; v3.6 permanent
rotating overlay done (single-slot queue, priority tiers, Status Rail
frontend, global hotkeys ⌃⇧N/⌃⇧X/⌃⇧P/⌃⇧O); v5 settings window done
including the settings page (sidebar nav, rotation/priority group,
shortcuts cheatsheet); v5 news (rss poller, NewsItem wire metadata, news
cards) done; v5.1 open-story done; v6 per-source priority + rotation-order
tie-break + cmux origin done (commits `e1f1998`, `a693cf2`); **v5.1
appearance hot-apply + per-source test notifications done (commit
`efa1bd2`) — six settings-window invoke commands total (adds
`send_test_notification`, `set_appearance`), an Appearance section with
card-shape presets, per-source test-notification buttons**; remaining
open work = manual checklist rows in `docs/IMPLEMENTATION_PLAN.md` §6 and
whatever `plans/` holds. Delete AGENTS.md's empty trailing
`## Imported Claude Cowork project instructions` heading **only if** no
content follows it in the file (confirmed empty — last line of the file —
when this plan was refreshed against `efa1bd2`; re-check with `tail -3
AGENTS.md` since content could theoretically be appended later).

**Verify**: `grep -c "next open item" AGENTS.md` → `0`; `grep -c "e1f1998\|rotation-order\|rotation order" AGENTS.md` → ≥1; `grep -c "send_test_notification\|set_appearance\|six invoke" AGENTS.md CLAUDE.md` → ≥1 for each file; same paragraph present in CLAUDE.md.

### Step 2: Fix CONTEXT.md

Remove the `docs/adr/` pointer (line ~6). In the **Connector** entry:
change "receives every accepted Event" to note the news exception
("every accepted Event *except News items, which are overlay-only by
design* — see `IMPLEMENTATION_PLAN.md` §4.6") and change "cap, TTL,
Paused" to "cap, Rotation, Paused". Add one sentence to the **Rotation**
entry blessing the config keys: "config file keys retain `*ttl*` names
for file compatibility; the domain term is Rotation."

**Verify**: `grep -c "docs/adr" CONTEXT.md` → `0`; `grep -c "except News" CONTEXT.md` → `1`.

### Step 3: Fix the three stale code comments

- `src-tauri/src/lib.rs` (find with `grep -n "doesn't exist and the window
  opens blank" src-tauri/src/lib.rs` — was ~lines 573–575 at the original
  `d40445e` baseline, ~lines 604–606 as of `efa1bd2`, may have moved
  again by execution time): delete the "until …step 5 lands …
  `settings.html` doesn't exist" sentence from the `open_settings_window`
  doc comment; keep the rest.
- `src-tauri/src/queue.rs` (~line 76): "all three `SourceKind` variants"
  → "all four `SourceKind` variants".
- `src-tauri/src/poller.rs` (~lines 547–552): soften "every accepted
  event goes to every enabled connector, always" to cite the news
  exception ("…always — with one recorded exception: rss/news events are
  overlay-only and never offered, `IMPLEMENTATION_PLAN.md` §4.6").

**Verify**: `cargo test` from `src-tauri/` → all pass (comment-only). `grep -c "doesn't exist and the window opens blank" src-tauri/src/lib.rs` → `0`.

### Step 4: README refresh

Rewrite "what it does" to the shipped model (single visible slot, priority
tiers + rotation-order tie-break, sources: cli push, cmux relay, espn
scores, rss news; telegram outbound; settings window; the four global
hotkeys as a small table with the note that they are *global* grabs).
Drop "WhatsApp via Twilio". Retitle "quick start (once scaffolded)" →
"quick start". Add a short **setup** subsection: rustup; `swift build -c
release` in `notchtap-detect/` + symlink to `/usr/local/bin/notchtap-detect`
(or set `detect_path` in config); `brew install jq`; optional symlink for
the `notchtap` script; the first-run config sentence from Current state
item 9; and "logs: `~/Library/Logs/notchtap/notchtap.log` (10 MB × 3)".

**Verify**: `grep -c "Twilio\|max_concurrent\|FIFO queue" README.md` → `0`; `grep -c "Library/Logs" README.md` → ≥1.

### Step 5: ARCHITECTURE.md §11 and IMPLEMENTATION_PLAN.md §5/§6

Apply Current-state items 10, 11, 12 exactly as described. Keep the
repo's supersession style: strike/annotate rather than silently rewrite
where the old text records a decision (e.g. §11's frontend-log-bridge
line becomes "superseded 2026-07-17: never built; the overlay is
receive-only (§14/§17), so frontend errors are devtools-only by design").

**Verify**: `grep -c "local/share/notchtap" docs/ARCHITECTURE.md` → `0`; `grep -n "cap-3" docs/IMPLEMENTATION_PLAN.md` → no hits in §6's unchecked rows (hits inside historical/superseded prose are fine — check context of any remaining hit).

### Step 6: V3_6 spec §3.3 and TESTING_STRATEGY body counts

Apply Current-state items 13 and 14. For the spec, show the real struct
(copy the shape from `src-tauri/src/http.rs:98–111` — verify you have the
right span with `grep -n "struct NotifyRequest" -A 13 src-tauri/src/http.rs`
before copying; the `rotation`/`topic`-never-accepted-from-the-wire
comment already in the spec's surrounding prose stays as is) and point
the priority-default sentence at the per-source config fields. For
TESTING_STRATEGY, replace every in-body count with "see §0" and fix §5.1's
lib.rs entry per item 14.

**Verify**: `grep -n "27 tests\|20 tests\|the 48 total\|StatusRailCard 12\|rss_poller 20\|settings 34" docs/TESTING_STRATEGY.md` → no output.

## Test plan

No new tests — this plan is prose/comments only. The gates are the greps
above plus `cargo test` + `npx vitest run` green (proving no code was
accidentally touched).

## Done criteria

- [ ] All step verifies pass as stated
- [ ] `cargo test` (from `src-tauri/`) exits 0
- [ ] `npx vitest run` exits 0
- [ ] `git diff --stat` shows only in-scope files
- [ ] `plans/README.md` status row updated

## STOP conditions

- Any listed excerpt is not found where described (the file drifted).
- A fix appears to require changing executable code, a test assertion, or
  a config value.
- You find the AGENTS.md trailing section is NOT empty — report its
  contents instead of deleting.
- `git status` (run before Step 1, per the drift-check note above) shows
  uncommitted changes to an in-scope file that you didn't just make —
  another session may be mid-edit on it.

## Maintenance notes

- The AGENTS.md/CLAUDE.md pair is a standing drift hazard: they were
  updated independently once already. Consider (future, not this plan)
  making one file canonical and the other a one-line pointer.
- Plan 014 (logging rotation tests) removes `logging.rs` from
  TESTING_STRATEGY §5.1 entirely, and plan 017 (justfile) rewrites
  AGENTS.md's "consider adding a justfile" paragraph — if those land
  first, reconcile wording rather than duplicating.
- **Review-plan refresh log (this session)**: re-verified every
  "Current state" item against commit `efa1bd2` (one commit past the
  original `d40445e` baseline) plus the working tree. All findings still
  held; three things were fixed: (1) Step 1's paragraph now also covers
  `efa1bd2`'s appearance/test-notification work, which had landed after
  the original draft and would otherwise have been missed by this very
  anti-staleness plan; (2) the `http.rs:68–81` citation in item 13/Step 6
  was wrong even at the original baseline (pointed at `enqueue_and_emit`,
  not the `NotifyRequest` struct) — corrected to `:98–111`; (3) the two
  `config.rs` line citations in item 9 were also wrong at baseline
  (`:113-115`/`:227-242` → `:132-133`/`:267-282`). If this plan sits
  unexecuted long enough to drift again, a cheap re-verification pass is:
  re-run the drift-check `git diff --stat` above, then for each file it
  flags, re-run the specific `grep -n` shown next to that file's item in
  "Current state" rather than trusting any hardcoded line number.
- **Execute-time reconciliation (this session)**: one more commit,
  `d4eac73` ("fix: set_appearance invoke arg mismatch, cmux branding
  leak, stale docs"), landed between the review-plan pass and dispatch —
  a concurrent session fixing the `set_appearance` frontend/backend key
  mismatch this plan's items don't touch. It re-touched two in-scope
  files: `docs/TESTING_STRATEGY.md` (only the out-of-scope §0 table:
  `settings 41→38`, `config 16→17` — the four in-body stale numbers item
  14 targets are untouched) and `src-tauri/src/lib.rs` (deduplicated an
  `AppearanceChangedPayload` struct literal into a `From` impl, upstream
  of and unrelated to the `open_settings_window` doc comment). Verified
  via `grep -n` that every item-6/7/14 target string is still present
  verbatim and `open_settings_window`'s comment is still at line 605 —
  no further plan changes needed, only the baseline SHA bump above.
- **Branch-mismatch STOP and fix (this session)**: the first dispatch
  STOPPED at the drift check — its worktree checked out
  `v3.6-rotating-overlay` per the (then-)stated Git workflow, landing on
  `a693cf2`, three commits behind the `d4eac73` baseline this plan was
  actually verified against (all verification happened in the main tree,
  which was on `master`). On `a693cf2`, `efa1bd2`'s appearance/test-
  notification commands don't exist in the code at all (`grep -rn
  "send_test_notification\|set_appearance" src-tauri/src/` returns
  nothing), so Step 1's paragraph as specified would have asserted
  something false. Root cause: `master` and `v3.6-rotating-overlay` have
  diverged — `master` picked up 3 commits the feature branch didn't.
  Fixed by re-pinning the Git workflow section to `master`. If `master`
  and `v3.6-rotating-overlay` are later reconciled (merged either
  direction), this note is stale and can be deleted.
- **Execute review verdict (this session): APPROVED.** A third dispatch
  (after pushing local `master` to `origin` to fix the branch-mismatch
  above) completed all 6 steps and committed `7a0cb83` on top of
  `d4eac73`, on worktree branch `worktree-agent-adff3b07938ec8c4b`
  (path: `.claude/worktrees/agent-adff3b07938ec8c4b`). Reviewer
  independently re-verified: every step's grep verify, `cargo test`
  (214 + 3 doc-tests, 0 failed), `npx vitest run` (62 passed), and
  `git diff --stat d4eac73..HEAD` scope (exactly the 11 in-scope files,
  nothing else). Spot-checked several content claims against live code
  (`event.rs`'s `SlotState::Showing.link` field, `config.rs`'s
  `manual_default_priority`/`cmux_priority`) — accurate. Not merged —
  merging onto `master` is the operator's decision.
