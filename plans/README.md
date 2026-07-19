# Implementation Plans

**Third audit session (2026-07-18, planned at `a58f115`)**: a standard
`improve` run scoped to the ~30 commits landed since the deep audit's
`d40445e` baseline (the executed plans 009–023 themselves were the
unaudited surface). Four parallel audit agents (correctness+perf,
security, tests+debt, dx/deps/docs/direction); every tabled finding
re-verified against the code by the advisor. Suites green at HEAD
(`cargo test` 251 + 3 doc-tests, `npx vitest run` 62, both matching
TESTING_STRATEGY §0 exactly). Headline finding: a lost-wakeup race in
the plan-015 deadline heartbeat (plan 036 — drafted as 024, renumbered
because the concurrent proptest follow-up holds 024; despite the number
it is the P1 to run first). Plans 025–031 + 036 below; the operator
selected all eight. Note: master moved to `d926977` (plan 022's
deep-testing merges) while these were being written — every plan's
drift check + "recount, don't trust" count language covers that, but
executors should expect the §0 baselines to have grown past 251/62.
Uncommitted working-tree changes
(`plans/022`, `prototype/status-rail.html`,
`src/settings/preview-overlay.css`, untracked `testing/`) belong to a
concurrent session and were excluded from the audit.

**Status-rail redesign (2026-07-18, planned at `d926977`)**: plans
032–035, from the `prototype/status-rail.html` rev-3 review session with
the operator. The numbering 025–031 and 036 belongs to the third audit
session above (its heartbeat plan ceded 024 to the in-flight proptest
follow-up; an earlier draft of this redesign work called itself "unfiled
plan 024" — superseded). Operator decisions locked at planning: celebration =
shipped goal burst+ring **plus** a staggered accent ripple (goal signal
only); rich-manifest layout **A** (detail cells); markdown inline-only
(`` `code` ``/bold/italic/line-break) rendered as React nodes, never raw
HTML; queue-slider batch semantics with a 10-segment proportional
ceiling; **every** promotion starts expanded with a mid-rotation
auto-retract (reverses plan 008's High-only); the `/notify` subtitle-fold
contract is amended (035). Recommended order: 032 → 033 → 034 → 035.

**Architecture-review session (2026-07-18, planned at `d926977`)**: an
`improve-codebase-architecture` run over the commit-history hot spots
(rust core + frontend, two parallel explore agents). Five deepening
candidates were reported; the operator selected the mutate→wake→emit
protocol deepening and locked its design in a grilling session
(closure-based `apply`, queue+wake private to the module, `accept` with
the News-origin Connector gate, Engine-owned clock, the name "Engine",
sequencing after 036/025). Filed as plan 037; CONTEXT.md gained the
**Engine** term at filing. One filing-time correction to the grilled
sketch, forced by 036's waiter-under-lock shape: the rotation loop
moves *inside* the Engine (`spawn_rotation`) rather than calling
`apply`, which would self-wake and spin — details in the plan's Design
block. Remaining candidates (poll-loop driver, stylesheet
triplication, typed settings IPC client, second-Connector cost map)
are recorded in the session report only — re-raise via a future
`improve` run if wanted.

**Reconciled 2026-07-18 at `b43a7ca` (advisor verify pass)**: plans
001–008 + the two extra-session plans (004-test-notifications,
005-appearance-config) were re-verified against HEAD — every claimed
implementation found in code, suites green (`cargo test` 225 + 3
doc-tests, `npx vitest run` 62). Two outstanding operator gates, neither
blocking: plan 001's manual real-keypress check, and plan 007's first
green CI run (workflow changes need a push). Plan 005's credential file
(`~/.config/opencode/openrouter.key`) confirmed mode `0600`; the
repo-root `opencode.json` holds only a `{file:…}` reference.
All TODO plans (009–023) were drift-checked: every finding still present
and every quoted code excerpt confirmed byte-identical — only line
numbers shifted from the 001/008/appearance merges (queue.rs promote_next
now ~206, open_current_story ~725, spawn_heartbeat ~655, saveConfig
rebuild ~1117). Plan 020's excerpts were refreshed in place for the
six-command build.rs. Plan 022's dependency (008) is now DONE — it is
unblocked except for its own Step 0 decision gate. Plan 015 still waits
on 009.

Two `improve` sessions wrote the original plans against commit `d40445e`
(2026-07-17). Plans 005 and 006 were cold-reviewed and refreshed against
`b1981c9` later that day; their files contain the current baselines and gates.

- **Plans 001–003**: a `next`-invocation session (direction-only audit).
- **Plans 004–023**: a `deep`-invocation session (all nine audit
  categories: correctness, security, performance, tests, tech debt,
  dependencies, DX, docs, direction). Baseline verified green at planning
  time: `cargo test` 209 + 3 doc-tests, `npx vitest run` 60,
  `tsc --noEmit` clean.

Each executor: read the plan fully before starting, honor its STOP
conditions, run every verification gate, and update your row when done.
Plans that add/remove tests must update `docs/TESTING_STRATEGY.md` §0 —
counts live there and only there.

## Execution order & status

Recommended order below (dependencies + risk-ordering), not strict — most
plans are independent. P1s first.

| Plan | Title | Priority | Effort | Depends on | Status |
|------|-------|----------|--------|------------|--------|
| 005 | Verify relocated OpenRouter key + complete operator rotation | P1 | S | — | DONE |
| 006 | Prevent telegram transport errors from logging the bot token | P1 | S | — | DONE |
| 007 | Supply chain + CI: pin nspanel rev, `--locked`, audit scans, Linux web job, `sh -n` gate | P1 | S | — | DONE (2026-07-18; pending CI gate since satisfied — hardened workflow green on master across the day's pushes, latest at `7c84a02`) |
| 008 | Expanded semantics: auto-expand High, reset per item, idle no-op | P1 | S | — | DONE (`8ca01e3`, verified 2026-07-18 — rewritten from `b1981c9` to remove an unrelated plan-001 duplicate that leaked into that commit) |
| 009 | Validate live `slot-state` payloads + pin the event-name seam | P1 | S | — | DONE (`bb0f249`, implemented directly 2026-07-18 from `3c5cb90`; vitest 62→64, `cargo test` 234→235, tsc/clippy/fmt/build clean; file → `(done)`) |
| 010 | ESPN fetch hardening: gzip, 1 MiB cap, redirect limit, UA | P1 | S | — | DONE (`4381de4`, merged to master 2026-07-18; Step 4 GUI smoke owed to operator) |
| 011 | RSS robustness: `fetch_feed` wiremock tests, bounded entity decoder, streaming cap | P1 | M | — | DONE (`6b0bbc4`, merged to master 2026-07-18; `/improve execute` → reviewed APPROVE; file → `(done)`) |
| 012 | Open-story hardening: reap child, `open -u` normalized URL, tested scheme gate | P2 | S | — | DONE (`b7f58fd`, implemented directly 2026-07-18 from `586c943`; `cargo test` 234 + 3 doc-tests, clippy/fmt clean; Step 4 GUI smoke batched to end-of-run; file → `(done)`) |
| 013 | Boot-path config validation (warn-and-continue) | P2 | S | — | DONE (`cd97ace`, `/improve execute` → reviewed APPROVE; rebased onto 023+020 and landed alongside a fix to 020's compile break (`8a5d674`); all gates green — `cargo test` 243 + 3 doc-tests, clippy/fmt/tsc/vitest/build; boot smoke batched; file → `(done)`) |
| 014 | Test the log-rotation engine + eval-splice escaping | P2 | S | — | DONE (merged to master `b4da777` 2026-07-18 and pushed; `/improve execute` → reviewed APPROVE in worktree; file → `(done)`) |
| 015 | Deadline-based heartbeat (replace the 250 ms tick) | P2 | M | 009 (DONE) | DONE (`2cef7ad`, `/improve execute` → reviewed APPROVE after 1 revision (settings test-notif wake); cherry-picked to master 2026-07-18; `cargo test` 242 + 3 doc-tests; idle-CPU smoke batched to end-of-run; file → `(done)`) |
| 016 | Frontend lint/format gate (Biome) | P2 | S | — | DONE (`dab3f29`+`c6501bc`, fast-forwarded to master 2026-07-18 and pushed; `/improve execute` → STOP at the line-width gate resolved by operator (`lineWidth: 100`, churn 782→473), then reviewed APPROVE; file → `(done)`) |
| 023 | Goal celebration visible (review-log ranked list + redesign) | P2 | M | — | DONE (CSS-first: dropped lottie for a layered-radial confetti burst + `::before` ring + overshoot on `.rail-card.pulse-goal`, removed `lottie-react`/JSON/`GoalCelebration.tsx`; reduce-motion ⇒ nothing in ARCHITECTURE.md §4; `vitest` 64 / `tsc` / `vite build` green; landed on `master` 2026-07-18. Live dev-machine eyeball — Steps 1–3 + acceptance, incl. the burst-readability judgement in the review log — owed to operator) |
| 018 | Overlay idle-cost cut: transform-based news shader (rescoped 2026-07-18 — the lazy-lottie half went moot when 023 deleted lottie outright) | P2 | S | — | DONE (`7c84a02`, implemented directly 2026-07-18; verified same day — `background-position`→0/`translate3d`→2/`infinite`→1, cargo 249+3, vitest 66, tsc/build/clippy/fmt clean, CI green on master at the commit; file → `(done)`) |
| 022 | Deep-testing un-park decision + §9.1/§9.2 execution | P2 | L | decision gate | DONE — Step 0 operator decision = **EXECUTE §9** 2026-07-18; `/improve execute` → reviewed **APPROVE**; fast-forwarded to master `4cc3a4b` 2026-07-18 (worktree branch cleaned up); file → `(done)`. 3 commits: `5b51855` (§9.1/§9.2 docs retarget), `37ebfb8` (queue proptest suite + proptest dev-dep), `4cc3a4b` (§9.2 http burst cases). Pre-dispatch reconcile folded a kimi-authored, reviewer-verified §9.1 scoping into the plan (cap-bypass set on invariant 2, two-caps split on 6, enqueue_test/slot_state_if_changed pre-clears). Reviewer re-verified independently: **zero production-code changes** (queue.rs/http.rs purely additive, proptest lives in `#[cfg(test)] mod proptest_queue`), `cargo test --locked` **257 + 3 doc-tests** matching §0 exactly (queue 52→53, http 27→32), clippy/fmt `--locked` exit 0, proptest 256-case suite passes 2× no flakes (~0.35s, well under 2-min budget), invariant 2 correctly scoped to post-Enqueue-into-waiting (not gamed), `enqueue_at` uses production pause semantics. **No bug surfaced** (no `#[ignore]`d regression needed). One documented deviation approved on merit: §9.2's ttl-clamp case adapted to "extra `ttlSecs` wire field ignored → configured default applies" (there is no ttl wire field). Known follow-up: invariant 4's rotation_order rank tie-break is NOT exercised by the property suite (`with_rotation_order` left unset → FIFO only; the v6 rank tie-break stays covered by the example suite). **Operator action: review + merge the worktree branch to master.** Then file → `(done)`. |
| 017 | Justfile (one-command local verification) | P3 | S | 016 (soft) | DONE — executed + reviewer APPROVE 2026-07-18 (`/improve execute`). Pre-dispatch reconcile (pass 4): biome/016 landed, baked `npx biome ci .` into `check-web`, drift baseline → `4af5e8e`. Reviewer re-verified all 10 gates green independently (fmt/clippy --locked/cargo test 256+3/biome/tsc/npm audit/vitest 66/vite build/sh -n/swift build), both done-criteria greps pass, scope clean (justfile+AGENTS.md+CLAUDE.md only), `just` confirmed absent so manual-verify path used. `test-all` omits cargo-audit (binary absent — documented in recipe + commit). Commit `b3aa43d` fast-forwarded to master 2026-07-18 (worktree branch cleaned up); file → `(done)`. |
| 019 | Dead code removal: presentation channel, polling gates, no-op dispatch, scaffold | P3 | M | — | DONE — `/improve execute` 2026-07-18, reviewed **APPROVE**, fast-forwarded to master and **pushed** (`f2f3299..c376479`). 4 commits: A `647e1b4` (presentation channel), B `df25b0d` (pause gates), D `089bf33` (scaffold), C `c376479` (dispatch). Executor STOPPED at C first pass — correctly caught the plan's "sole `dispatch` caller" was wrong (2nd caller `settings.rs:684`, v5.1 `efa1bd2`); plan corrected (settings.rs → scope/Step C/drift list) and C re-run green. Reviewer re-verified ALL done-criteria independently: dead-symbol grep zero, no `dispatch(` callers left, `cargo test --locked` 251+3, clippy/fmt `--locked` exit 0, vitest 62, tsc/biome/vite build exit 0, index.html retitled, `capabilities/default.json` byte-identical, §0 counts reconciled (rust 256→251: poller 19→16, event 18→16; frontend 66→62), CONTEXT.md Polling Pause updated. Scope 100% in-plan (17 files). One documented deviation approved on merit: relocated `use crate::error::EventError` into event.rs `mod tests` (test at :353 still uses `MissingField`). Remaining manual check owed to operator: `npm run tauri dev` overlay boot (headless worktree couldn't). File → `(done)`. |
| 020 | Config defaults single-source (`get_default_config` invoke) | P3 | M | — | DONE (`9774930`, merged to master 2026-07-18; `/improve execute` → reviewed APPROVE after 1 REVISE; file → `(done)`) |
| 021 | Settings save polish: feed metadata, duplicate rejection, port pre-flight | P3 | M | — | DONE (`8c35f1e`, merged to master 2026-07-18; `/improve execute` → reviewed APPROVE; frontend gates green, rust structural-verified + macOS CI; file → `(done)`) |
| 036 | Heartbeat lost-wakeup: register the `Notified` waiter under the queue lock (renumbered from 024 — that number stays with the in-flight proptest plan) | P1 | S | — | DONE (`9373b84` on `exec/036-heartbeat-lost-wakeup`; `/improve execute` → reviewed APPROVE; cargo 258+3 doc-tests, clippy/fmt clean; `enable()` registered inside queue-lock block) — reconcile-verified 2026-07-19 at `7430b4b`: `enable()` at lib.rs:768 inside the lock before `next_deadline()`, single pinned `notified`, all gates green |
| 025 | ESPN streaming body cap + shared poller client/fetch helpers (`net.rs`) | P1 | M | — | DONE (`920d4e4`, merged to master 2026-07-19; `/improve execute` → reviewed APPROVE. Re-executed against master `7430b4b` after the 032–034 landings invalidated the first run (`5e838d4` on the retired pipeline branch — same content, stale base). Reviewer independently re-ran all done criteria: `cargo test --locked` 280 + 3 doc-tests (`net 4` row), clippy/fmt `--locked` exit 0, zero inline `Client::builder`/`response.bytes()` in pollers, `read_body_capped` 1× each, rss `mod tests` untouched, hunks fenced off the new live-match/status-state code) |
| 026 | Docs/DX truth pass: seven invoke commands, deadline-heartbeat prose, biome-ci wording, `just setup` | P2 | S | — | DONE (cherry-picked to master `727be48`+`9ee8ec2`+`8157287` 2026-07-18; `/improve execute` → reviewed APPROVE after 1 revision (v3.6 heartbeat prose cited §4.4, fixed to §4.3); worktree cleaned up; file → `(done)`) |
| 027 | Appearance section follows Reset; App.tsx unlisten unmount-guard | P3 | S | — | DONE (`51ca452` on `claude/tasks-27-30-blockers-fkencr`; `/improve execute` → reviewed **APPROVE**; `formGeneration` key remounts AppearanceSection on load/both-resets, App.tsx `appearance-changed` listener gains the `unmounted` guard + `.catch` mirroring `useSlotState`; new re-seed test pins the bug (`key={0}`→fail verified); tsc/vitest 98/vite build green; §0 frontend 97→98, settings form 13→14. Reviewer-independent re-run of all criteria. One documented deviation: `biome ci .` red on PRE-EXISTING out-of-scope files only (`lib/markdown.tsx:67`, `components/Track.tsx:20`, `lib/presentation.test.ts`) — byte-identical to master, not from this plan. Merged to master (fast-forward); file → `(done)`.) |
| 028 | Shared Event test builder (rust) + shared listen-mock harness (frontend) | P3 | M | 036, 025 (soft) | TODO |
| 029 | Pin CI actions to commit SHAs + dependabot for github-actions | P3 | S | — | TODO |
| 030 | SPIKE: OpenRouter news enrichment design doc (`docs/design/`) | P2 | M | — | TODO |
| 031 | SPIKE: live-match scoreboard Topic card design doc (`docs/design/`) | P2 | M | — | DONE (`d2dad13` on `exec/031-scoreboard-topic-spike`, researched at `339156a`, 2026-07-19; `/improve execute` → reviewed APPROVE. Deliverable: `docs/design/scoreboard-topic-card.md` (604 lines, all 10 sections w/ recommendation + rejected alternative, docs-only — zero source changes). Design spine: Topic = `espn:{league}:{match_id}`; Rotation = `Recurring` for the match with the full-time `MatchState` emitted `OneShot`-same-Topic so existing supersede/rotate-out retires the card for free; opt-in `espn_live_card` flag default `false`, not a default-behavior replacement. Reviewer independently verified the two load-bearing traces: (1) `batch_done += 1` at queue.rs:257 is unconditional, BEFORE the `Recurring` check at :258 — a real build-blocker the spike correctly surfaced (a cycling card would pin the 033 queue-slider near "complete"); (2) `enqueue_and_fan_out` clones the event (poller.rs:565) before `enqueue()` and offers on `Ok(())` regardless of supersession, so telegram gets every delta by construction. **§10 decisions RESOLVED 2026-07-19**: build the card opt-in (`espn_live_card` default `false`); defer multi-match (reuse today's tier sharing); reuse 8s `ttl_secs` for dwell; land the `queue.rs:257` `batch_done` fix as its own small plan FIRST. Splits into two build plans (A: counter fix, B: opt-in single-match card). The build plan inherits this doc's Topic-identity + rotation-kind verbatim; if 037 lands first, re-ground the enqueue/tick paths against the Engine module.) |
| 024 | Proptest rotation_order coverage (invariant 4 rank tie-break) | P3 | S | 022 (DONE) | DONE (`67e7ecf`, merged to master + pushed 2026-07-18; file → `(done)`) — `/improve execute` → reviewed **APPROVE**. Executor commit `cee5249` rebased cleanly onto master as `67e7ecf` (tests-only; zero overlap with the concurrent plans/ churn, FF push). Reviewer re-verified independently in the worktree: scope clean (only `queue.rs` +97/−30 and `docs/TESTING_STRATEGY.md`, all hunks inside `#[cfg(test)] mod proptest_queue`, production `best_index_in_tier` untouched, no Cargo/README change in the code commit); `predict_promoted` mirrors production strict-`<` min-rank/FIFO exactly (not left in Step 4's inverted state); property suite **5/5** (no `proptest-regressions`); full suite **257 + 3 doc-tests, 0 failed** (§0 unchanged); clippy `--locked --all-targets -D warnings` + `fmt --check` exit 0; re-confirmed on the rebased state before push. Executor used `prop_shuffle` (proptest 1.11.0). Deferred: a stale "leaves it unset" line in §9.1's operation-model para (`docs/TESTING_STRATEGY.md:717`) — tiny follow-up. Env note: executor cleared a sibling worktree's regenerable `target/` under a disk-full crisis — source intact. |
| 032 | Status Rail visual refresh: chip removal + accent edge, rounded default (16px), body prominence, inline-markdown body, celebration A+B ripple | P2 | M | — | DONE (`d6546e7`..`b5cf0f8` on `exec/032-status-rail-visual-refresh`; `/improve execute` → reviewed APPROVE; vitest 68, tsc/biome/vite clean, cargo 258+3; visual eyeball operator-owed) — reconcile-verified 2026-07-19 at `7430b4b`: zero TierCode refs, accent edge + ripple + reduced-motion in both CSS files, radius 16.0, markdown renderer 7 tests / no `dangerouslySetInnerHTML` usage, cmux sample has the code-span body; visual eyeball still operator-owed |
| 033 | Queue slider track (batch counters, slot-state `queueTotal`/`queueDone`) + auto-expand-all lifecycle with mid-rotation retract | P2 | M | 032 | DONE (`a134be4`+`eb4053d` on `exec/033-queue-slider-expand-all`; `/improve execute` → reviewed APPROVE; cargo 268+3 doc-tests, vitest 77, all gates green; 5-push expand→retract→rotate manual check operator-owed) — reconcile-verified 2026-07-19 at `7430b4b`: `window_expanded`/`auto_retract_armed` split in place, `queueTotal`/`queueDone` in snapshot tests, retract example suite present, proptest invariants 8+9 rewritten per the review (9 = min-of-two-deadlines, queue.rs:2250); 5-push manual check still operator-owed |
| 034 | Idle source-status rail: `status-state` channel, poller live-match surface, IdleView redesign | P2 | M | — (soft 032) | DONE (`5dd7567`+`3ecda02` on `exec/034-idle-source-status-rail`; `/improve execute` → reviewed APPROVE; cargo 276+3 doc-tests, vitest 97, all gates green; `capabilities/default.json` byte-identical; idle manual checks operator-owed) — reconcile-verified 2026-07-19 at `7430b4b`: `STATUS_STATE_EVENT` pinned by test (status.rs:136), status seed routed through `escape_for_eval_splice` (lib.rs:438), `useStatusState.ts`/IdleView chips present, capabilities file untouched, §0 counts (276+3 / 97) match live runs; idle manual checks still operator-owed |
| 035 | Rich relay manifest: `/notify` `subtitle`/`details[]`, CLI `--detail`, claude-code + cmux hook scripts, manifest layout A | P2 | M | — | DONE — merged to master `55d799e` (2026-07-19). `/improve execute`: reconciled `d926977`→`339156a` pre-exec (closed the `StatusRailCard.tsx` render-path gap 033 introduced + refreshed line refs), dispatched executor on branch `worktree-agent-afec8af1b9c0e033c` (`7222989` rust+wire / `b625ed8` cli+hooks / `a26becd` frontend / `e981d7a` docs), reviewed **APPROVE**. Reviewer re-ran every gate green on both the branch and the merged tree — cargo 289+3 doc-tests, clippy `-D warnings` clean, vitest 105, tsc + vite build clean, both hooks `sh -n`-clean and exit-0 fail-safe; scope clean. Merge resolved the one `docs/TESTING_STRATEGY.md` §0 count line (frontend → 105; `SettingsApp.tsx` auto-merged). Owed (operator): manual check (real permission prompt → Tool/Command/Project cells) + confirm `PermissionRequest` is a hook event in the installed Claude Code. Unblocks 037. |
| 037 | The Engine: one propagation module for every Slot mutation (`apply`/`read`/`accept`/`update_live_match`, private queue+wake+live, clock-agnostic queue, rotation loop inside) | P2 | L | 036 (DONE), 025 (DONE), 033 (DONE), 034 (DONE), 035 (DONE) | DONE — retargeted 2026-07-19 at `b8c554f` (review-plan pass; history in the plan file's Status section), executed on `exec/037-engine-v2` as `942c4d2`..`1e11337` off master `9b5bd62`; `/improve execute` → reviewed APPROVE: reviewer independently re-ran every done criterion — cargo 295+3 doc-tests matching the recomputed §0 row, property suite 5× green (no regressions file), clippy/fmt `--locked` exit 0, vitest 107 + tsc, all three seam greps + the retire/`Instant::now`/`too_many_arguments` greps clean. Deviations judged on merit: steps 3–5 landed as one atomic commit `0b9d2b5` (not independently green-able — AppState/engine construction forces simultaneous migration), `status.rs` got test-only signature edits (plan omission, mechanical), vitest count is 107 vs the plan's 105 (plan 040 Part A added 2 at the base). Merged to master at `6b53c32` (corrected 2026-07-19 at the 038 review-plan pass — the earlier "NOT merged" note predated the merge). Follow-ups: the pre-existing `biome ci` red on 3 committed frontend files (known since plan 027's done entry) still stands — and the main tree's `node_modules` lacks `@biomejs/biome`, so local `npx biome` silently resolves a stale 0.3.3 that passes vacuously; AGENTS.md "project state" paragraph predates 037. |
| 038 | batch_done must not count a Recurring requeue as "done" (`queue.rs`) | P2 | S | 035 (ordering), 037 (coord) | DONE 2026-07-19 — guarded both requeue arms (`rotate_out_if_elapsed`, `skip_visible`), reconciled the caps test, added a regression test, cargo test/clippy/fmt clean. |
| 039 | ESPN live-match scoreboard card (opt-in, single-match) — Topic `espn:{league}:{match_id}`, `Recurring` rotation, full-time `OneShot` retires it | P2 | M | 037 (DONE), 038 (DONE) | DONE 2026-07-19 — added `Config.espn_live_card` (default false) + settings toggle; `poller.rs`'s `make_event`/`diff_scoreboard`/`spawn_espn_poller` thread the flag and emit per-match Topics with `Recurring` in play / `OneShot` full-time on the same Topic; three new poller tests (flag-off regression pin, flag-on Topic/rotation shape, flag-on end-to-end slot collapse + connector fan-out); no `engine.rs`/`queue.rs` change needed. |
| 040 | Weather source + idle-rail ambient presence (football chip + weather chip/alerts) | P2 | S+M | — | TODO — filed 2026-07-19 from live-session product discussion. Part A (S): football presence chip when `enabled && !live` (IdleView only). Part B (M): `weather_poller.rs` in the ESPN/RSS mould — ambient idle chip + threshold alert cards, opt-in `weather_enabled` default `false`. Independent of 035/037/039. |
| 041 | ESPN event-card copy — label scoring plays with ESPN's own event-type text (goal/penalty/own-goal), matching how cards already self-describe | P3 | S | none (037 landed; 039 coordination is same-file-touch only) | DONE — landed 2026-07-19: `last_scoring_play` now prefixes the body with ESPN's own `detail_type.text` via a shared `labeled_detail_line` helper used by both card and scoring-play extractions, plus a structural `own_goal: bool` on `SbDetail` (parsed from ESPN's `ownGoal` boolean, same pattern as `red_card`) that short-circuits to an "Own Goal" label; three new poller tests (goal, penalty, synthesized own-goal), poller 22→25. Originally filed 2026-07-19 from operator feedback watching ENG–FRA live. Review-plan pass 1 (2026-07-19): found the fix is narrower than the original filing assumed — cards already self-describe (`last_card` already prefixes with ESPN's own `detail_type.text`); only the scoring-play path (`last_scoring_play`) discards that same field, and goals are `EventType::ScoreUpdate`, not `MatchState` as originally scoped. A fresh-context subagent cold-read then caught a fixture-ordering bug (`last_scoring_play` picks the LAST matching detail — the untouched UCL fixture yields "Penalty - Scored", not "Goal") and a false premise that `baseline(UCL)` works (that fixture is already final on first sighting, returns an empty snapshot) — Step 2 rewritten with verified working test code. Review-plan pass 2 (2026-07-20): reading the raw ESPN fixture JSON directly found structural `ownGoal`/`penaltyKick` booleans on every scoring-play detail, sitting unparsed right next to `redCard` (which already uses this exact technique). Eliminated the "documented best-guess" own-goal label entirely — Step 1 now adds `own_goal: bool` and derives the label from that structural fact, not from guessed text. Dropped the original filing's emoji prefixes (zero precedent anywhere in the codebase), flagged as a confirm-with-operator choice. |
| 042 | Live-match scorecard presentation (bigger, persistent while playing) — Option A "match mode" pin+enlarge vs Option B richer rotating card vs Option C hybrid | P2 | M–L (per option) | 039 (hard, unblocked not yet executed), 037 (DONE) | BLOCKED on an unresolved maintainer decision (Option A/B/C) — review-plan pass 2026-07-20 did NOT make that call, it's explicitly reserved for the maintainer, but grounded each option's real cost against live code: found `EventMeta.details`/`subtitle` (plan 035's wire fields) already carry arbitrary label/value cells with zero schema changes, de-risking Option B, but found those cells render ONLY when expanded (`Manifest.tsx:38`) so "readable when collapsed" is new frontend work under every option; found the overlay window is sized once at startup with no dynamic-resize path anywhere in the code, so Option A's "window grows" is genuinely new capability, not an adaptation; found per-side yellow/red card counts need new team-id parsing (`SbDetail`/`SbTeam` don't parse ESPN's `team.id` field at all today, verified it correctly correlates a real card to the scoring player's team) common to every option. |
| 043 | Richer live-match event coverage (fouls, offside, disallowed goals, subs) via ESPN's per-match play-by-play endpoint | P3 | M–L (contingent on Step 0) | 037 (DONE), 039 (soft, unblocked) | TODO, gated on a new Step 0 — review-plan pass 2026-07-20 fetched ESPN's real `summary?event={id}` endpoint for two actual matches (one finished, one upcoming) to check whether it contains the play-by-play/commentary timeline this plan's entire approach assumes exists. It did not, in either sample — no `commentary`/`keyEvents` key in either response. Neither sample was a genuinely live match though, so this doesn't disprove the premise (commentary may only populate during live play) — added a hard Step 0 requiring confirmation against a real live match before any fetch/parse code is written, with an explicit STOP path if the data turns out not to be there at all. Corrected coordination language (037 landed, no longer a wait; 039 unblocked). |

## Done

Completed in this session (2026-07-17), filed with a `(done)` suffix:

- [`001-wire-skip-and-open-settings-hotkeys(done).md`](./001-wire-skip-and-open-settings-hotkeys(done).md) — ⌃⇧] skip + ⌃⇧, open-settings hotkeys; code review approved, manual real-keypress verification pending.
- [`002-settings-animation-previews(done).md`](./002-settings-animation-previews(done).md) — Appearance section + static preview cards.
- [`003-kuma-webhook-recipe(done).md`](./003-kuma-webhook-recipe(done).md) — Uptime Kuma → notchtap webhook recipe (docs only); kuma-side verification not run, see `docs/recipes/kuma-webhook.md`'s status line.
- [`004-docs-truth-pass(done).md`](./004-docs-truth-pass(done).md) — agent-facing docs/comments synced to shipped reality (~14 stale claims fixed); executed via `/improve execute`, reviewed and merged into `master` at `0749235`.
- [`004-test-notifications(done).md`](./004-test-notifications(done).md) — per-source test-notification buttons.
- [`005-appearance-config(done).md`](./005-appearance-config(done).md) — card scale/radius/opacity presets with hot-apply.
- [`008-expanded-semantics-high-auto-expand(done).md`](./008-expanded-semantics-high-auto-expand(done).md) — auto-expand High at both promotion sites, per-item reset, idle no-op toggle; executed at `b1981c9`, rewritten to `8ca01e3` to strip a leaked plan-001 duplicate, done criteria re-verified 2026-07-18 (details in the plan's post-execution appendix).
- [`005-relocate-opencode-api-key(done).md`](./005-relocate-opencode-api-key(done).md) — OpenRouter key relocated, file mode locked down, key rotated, auth smoke check passed.
- [`006-redact-telegram-token-from-logs(done).md`](./006-redact-telegram-token-from-logs(done).md) — `reqwest::Error::without_url()` redaction at `notifier.rs:278` plus a production-path regression test, `cargo test` 225 + 3 doc-tests.
- [`007-supply-chain-and-ci-hardening(done).md`](./007-supply-chain-and-ci-hardening(done).md) — `tauri-nspanel` pinned to `rev` (dropped `branch`, which cargo rejects alongside `rev`), `--locked` on clippy/test, `rustsec/audit-check@v2.0.0` + `npm audit --audit-level=high`, web job moved to `ubuntu-latest`, `sh -n notchtap` gate added; all local gates green (`cargo test --locked` 225+3, `npx vitest run` 62, `npm audit` 0 vulns); one green CI run on push still pending.
- [`011-rss-robustness-and-fetch-feed-tests(done).md`](./011-rss-robustness-and-fetch-feed-tests(done).md) — `fetch_feed` wiremock characterization (304 / validator-persist ordering / size cap), `MAX_ENTITY_LEN`-bounded entity decoder, pre-truncated `sanitize`, and a streamed 1 MiB body cap replacing full buffering; executed 2026-07-18 via `/improve execute`, reviewed APPROVE, merged to master `6b0bbc4` (`cargo test` 232 + 3 doc-tests, `rss_poller` 21→28).
- [`012-open-story-hardening(done).md`](./012-open-story-hardening(done).md) — ⌃⇧O open-story: extracted the tested `openable_http_url` gate (full parse, http(s)-only, returns the normalized serialization), hand it to `open -u` so validated==executed, and reap the spawned child off-thread (no zombie per press); implemented directly 2026-07-18 at `b7f58fd` (`cargo test` 234 + 3 doc-tests, `lib` 6→8). Step 4 GUI smoke batched to end-of-run.
- [`009-validate-slot-state-event-path(done).md`](./009-validate-slot-state-event-path(done).md) — route live `slot-state` event payloads through `isValidSlotState` (not just the eval-planted global), `.catch` a dead listener registration, and pin the `SLOT_STATE_EVENT` name across the rust↔TS seam with a rust test; implemented directly 2026-07-18 at `3c5cb90` (vitest 62→64 slot-state hook 14→16, `cargo test` 234→235 event 17→18). Unblocks 015.
- [`010-espn-fetch-hardening(done).md`](./010-espn-fetch-hardening(done).md) — ESPN poller gets the RSS client posture (UA, `Policy::limited(3)`, 10 s timeout) plus a two-stage 1 MiB response cap and gzip for both pollers; merged to master `4381de4` 2026-07-18. Verified complete via `/improve execute` 2026-07-18 (no dispatch needed — already merged; all done criteria re-checked green at HEAD). Step 4 manual GUI smoke owed to operator (needs a Mac dev machine).
- [`015-deadline-based-heartbeat(done).md`](./015-deadline-based-heartbeat(done).md) — replaced the 250 ms polling heartbeat with deadline-based wakeups: `SingleSlotQueue::next_deadline`, a `spawn_heartbeat` that sleeps to the visible item's rotation deadline (or forever when idle) woken by an `Arc<Notify>`, and every queue-mutation site wakes it — the wake lives inside the shared `enqueue_and_emit` (covering `/notify` + Settings test-notifications by construction), both pollers, and the four lib handlers; executed via `/improve execute`, reviewed APPROVE after one revision (review caught that Settings test-notifications would never auto-dismiss without the wake), cherry-picked to master `2cef7ad` (`cargo test` 242 + 3 doc-tests: queue 47→52, http 26→27, lib 8→9). Idle-CPU smoke batched to end-of-run.
- [`020-config-defaults-single-source(done).md`](./020-config-defaults-single-source(done).md) — seventh settings command `get_default_config` returns `Config::default()`, deleting the hand-maintained frontend `DEFAULTS` mirror; the "Reset to defaults" button now reads the rust source of truth, with the advisory fetch isolated from the critical panel load. Executed 2026-07-18 via `/improve execute`, reviewed APPROVE after 1 REVISE, merged to master `9774930` (frontend gates green — vitest 64/64, tsc, vite build; `cargo fmt` clean; `settings` 38→39, total 242→243). `cargo test`/`cargo clippy` deferred to macOS CI — Linux can't compile the `smappservice-rs`→`objc2` dependency (CI runs the rust job on `macos-latest`). **Follow-up `8a5d674` (2026-07-18): `get_default_config` was non-generic over the tauri Runtime, so its own MockRuntime tests failed to compile (`E0308`) — the crate did not build under `cargo test`. Widened to `<R: tauri::Runtime>` and committed its missing autogen permission (`permissions/autogenerated/get_default_config.toml`, which 020 left untracked though the other six are tracked). The break was caught on the Mac dev machine, not CI; it had been masked at land time by a full disk making `cargo test` fail as `ENOSPC` noise.**
- [`023-goal-celebration-visible(done).md`](./023-goal-celebration-visible(done).md) — CSS-first goal celebration: dropped `lottie-react` + the JSON asset + `GoalCelebration.tsx`, rebuilt the moment as a layered-radial confetti burst (`.rail-card::after`) + expanding `::before` ring + punchier `goal-overshoot`, keyed on the existing `pulse-goal` class (play-once, ~620ms, `signal === "goal"` only); reduce-motion ⇒ deliberately nothing, recorded in `docs/ARCHITECTURE.md` §4; landed on master 2026-07-18 (`vitest` 64, `tsc`/`vite build` green). Live dev-machine eyeball (Steps 1–3 + acceptance, incl. burst readability) owed to operator; moots plan 018's lazy-lottie step.
- [`013-boot-config-validation(done).md`](./013-boot-config-validation(done).md) — boot now runs the loaded config through the settings window's `validate()` and logs each out-of-range value as a warning, continuing (malformed TOML still fails fast in `Config::load`) — the hand-edited config file gets the same range contract as the settings save path. Executed 2026-07-18 via `/improve execute`, reviewed APPROVE; rebased onto 023+020 and landed at `cd97ace` alongside a fix to 020's compile break (`8a5d674` — see below). All gates green (`cargo test` 243 + 3 doc-tests, clippy/fmt/tsc/vitest 64/build). Boot smoke (scratch-`HOME`, out-of-range config) owed to operator.
- [`021-settings-save-polish(done).md`](./021-settings-save-polish(done).md) — settings save flow hardened: `saveConfig` now matches feeds by a normalized `feedKey` (strip fragment + trailing slash) so cosmetic URL edits keep their `source`/`category` while the *typed* URL is preserved; `validate()` rejects duplicate feeds (`feed_key`-normalized, `HashSet`); and `save_config_and_relaunch` runs an extracted `preflight_port` best-effort bind before writing so a colliding port is caught in-window instead of bricking the relaunch. Executed 2026-07-18 via `/improve execute`, reviewed APPROVE (executor correctly deviated from the literal Step-1 snippet, which would have discarded the user's typed URL), cherry-picked onto master `8c35f1e` (`vitest` 64→66 settings-form 11→13, `tsc`/`fmt` green; rust `settings` 39→45, total 243→249 — verified structurally on Linux then by macOS CI). Best-effort port pre-flight keeps its TOCTOU window by design (boot-time fail-fast is the backstop).
- [`018-overlay-idle-cost-cuts(done).md`](./018-overlay-idle-cost-cuts(done).md) — `.news-shade::before` now drifts via a compositor-only `translate3d` on an oversized (`inset: -30%`) pseudo-element (+ `will-change: transform`) instead of animating `background-position` — WebKit no longer repaints the news card layer up to 60 fps while a news card is visible; gradients, z-order, and the reduced-motion override unchanged, clipping still guaranteed by `.rail-card`'s `overflow: hidden` (no new containment rules). Implemented directly 2026-07-18 at `7c84a02`; the plan file's own rescope (dropping the lazy-lottie half that 023 mooted) rides along in the same filing. Verified 2026-07-18: `background-position`→0, `translate3d`→2, `infinite`→1; `cargo test --locked` 249 + 3 doc-tests, `npx vitest run` 66, `tsc`/`vite build`/clippy/fmt clean, CI green on master at the commit. Live shader eyeball (paint-flashing, Step 2) owed to operator as with the sibling GUI smokes.
- [`014-logging-rotation-and-eval-splice-tests(done).md`](./014-logging-rotation-and-eval-splice-tests(done).md) — the two untested load-bearing pure surfaces now have suites: `logging.rs` grew a 4-case temp-dir module pinning the rotation engine (threshold boundary, backup+reset, cascade retention = current + exactly 3 backups with no `.4`, empty-file guard), and `lib.rs`'s eval-splice escaping is extracted as `escape_for_eval_splice` with BOTH `on_page_load` sites (slot-state + appearance — the review-plan pass caught the v5.1 duplicate) routed through it plus 3 pure tests (`</script>` can't survive, U+2028/29 escaped, JSON round-trip preserves data); TESTING_STRATEGY §5.1 narrowed per its own de-listing rule. Executed 2026-07-18 via `/improve execute` in worktree branch `exec/014-logging-tests` (`59c3d75`+`7607b7c`+`65c288a`), reviewed APPROVE, merged to master `b4da777` and pushed (`cargo test` 249→256 + 3 doc-tests, clippy/fmt clean).
- [`016-frontend-lint-biome(done).md`](./016-frontend-lint-biome(done).md) — Biome 2.5.4 (lint+format) with a CI gate (`npx biome ci .` after `npm ci` in the web job), `lint`/`lint:fix` scripts, and an AGENTS/CLAUDE commands line; config scoped to `src/**/*.ts(x)` + `vite.config.ts` (CSS deliberately excluded — hand-tuned one-liners), `preset: "recommended"` + `useExhaustiveDependencies: error`, and `lineWidth: 100` by operator decision after the first executor pass STOPped on 782-line churn at the default 80 (re-wrapped to 473). 16 suppressions, each with an inline reason: 12 a11y in SettingsApp (semantic-markup migration is its own future task), 2 noNonNullAssertion (biome's fix breaks tsc narrowing), 2 exhaustive-deps (deliberate re-trigger keys); 3 mechanical forEach fixes applied. Executed 2026-07-18 via `/improve execute` in worktree branch `exec/016-biome` (`dab3f29` tool+gate, `c6501bc` churn), reviewed APPROVE, fast-forwarded to master and pushed (`biome ci`/tsc/vitest 66/vite build all green).
- [`026-docs-dx-truth-pass(done).md`](./026-docs-dx-truth-pass(done).md) — agent-facing docs synced to the seven-command invoke surface (CLAUDE.md/AGENTS.md narrative + ipc sections, `ARCHITECTURE.md` §17 follow-on amendment keeping the historical four, `V5_TECHNICAL_SPEC.md` intro line), the v3.6 spec's rotation prose moved off the deleted 250 ms tick onto plan 015's deadline-heartbeat model (§4.3), `biome ci` documented as the enforcing gate (`biome check` = local dev), and an opt-in `just setup` (`npm ci`) recipe so `just test-all` works on a fresh clone. Executed 2026-07-18 via `/improve execute`, reviewed APPROVE after 1 revision (the new prose cited §4.4 — supersession — for the heartbeat; fixed to §4.3), cherry-picked to master `727be48`+`9ee8ec2`+`8157287` and pushed.
- [`036-heartbeat-lost-wakeup(done).md`](./036-heartbeat-lost-wakeup(done).md) — fixed plan 015's lost-wakeup race by registering the `Notified` waiter under the queue lock via `enable()`; added regression test for idle-park → enqueue → rotate-out; `cargo test` 258 + 3 doc-tests, clippy/fmt clean.
- [`032-status-rail-visual-refresh(done).md`](./032-status-rail-visual-refresh(done).md) — removed tier chip, added 3px accent edge, rounded default to 16px, raised body prominence, inline-markdown renderer (no `dangerouslySetInnerHTML`), goal-celebration A+B ripple; vitest 68, cargo 258+3, all gates green.
- [`033-queue-slider-and-expand-all(done).md`](./033-queue-slider-and-expand-all(done).md) — repurposed Track as queue slider with batch counters `queueTotal`/`queueDone`, expand-all with half-window auto-retract and manual-expand-only 3× extension; cargo 268+3, vitest 77, all gates green.
- [`034-idle-source-status-rail(done).md`](./034-idle-source-status-rail(done).md) — added `status-state` channel, poller live-match summary, idle source-status chips with widened rail card; heartbeat sole emitter, `capabilities/default.json` byte-identical; cargo 276+3, vitest 97, all gates green.
- [`027-frontend-reset-polish(done).md`](./027-frontend-reset-polish(done).md) — Appearance section remounts on Reset/Reset-to-defaults via a `formGeneration` key so its controls re-seed from `config.appearance` (were showing stale pre-reset values); `App.tsx` `appearance-changed` listener gains the `unmounted` unlisten-race guard + `.catch` mirroring `useSlotState`; one new re-seed test pins the bug (`key={0}`→fail verified). §0 frontend 97→98, settings form 13→14; tsc/vitest 98/vite build green. `/improve execute` → reviewed **APPROVE** (`51ca452`); merged to master via fast-forward. Note: repo-wide `biome ci .` is red on PRE-EXISTING out-of-scope files (`lib/markdown.tsx:67`, `components/Track.tsx:20`, `lib/presentation.test.ts`) unrelated to this plan.
- [`031-scoreboard-topic-spike(done).md`](./031-scoreboard-topic-spike(done).md) — live-match scoreboard Topic-card design doc (`docs/design/scoreboard-topic-card.md`, 604 lines, docs-only — zero source changes); Topic = `espn:{league}:{match_id}`, `Recurring` rotation with the full-time `MatchState` emitted `OneShot`-same-Topic so existing supersede/rotate-out retires the card, opt-in `espn_live_card` flag default `false`. `/improve execute` → reviewed **APPROVE** (`d2dad13`). §10 decisions resolved 2026-07-19: opt-in card (`espn_live_card` default `false`), defer multi-match, reuse 8s `ttl_secs`, land the `queue.rs:257` counter fix first as its own plan.
- [`035-rich-relay-manifest(done).md`](./035-rich-relay-manifest(done).md) — `/notify` gains optional `subtitle` + capped `details[]` ({label,value}, ≤8 pairs, label/value truncated server-side as the trust boundary), CLI `--subtitle` now posts as its own field (no longer folded into the body) + repeatable `--detail Label=Value` (first-`=` split), two new observational hooks (`hooks/notchtap-claude-hook.sh`, `notchtap-cmux-hook.sh` — exit 0, no stdout, fail-safe if jq/notchtap absent), and the manifest renders subtitle + one cell per detail pair (Layout A) threaded `StatusRailCard`→`Manifest`. `/improve execute`: reconciled to `339156a`, reviewed **APPROVE**, merged to master `55d799e` (cargo 289+3 doc-tests, vitest 105, clippy/tsc/vite build all green). Owed (operator): manual permission-prompt render check + confirm `PermissionRequest` is a hook event in the installed Claude Code. Unblocks 037.
- [`037-engine-propagation-module(done).md`](./037-engine-propagation-module(done).md) — the Engine (`src-tauri/src/engine.rs`): one propagation module for every Slot mutation — `apply`/`apply_blocking`/`read`/`read_blocking`/`accept`/`update_live_match`/`emit_current_blocking`/`emit_current_status_blocking`, with the queue, wake, and live-match handle private by construction (taken by value at `Engine::new`); the queue's enqueue interface is clock-agnostic (`now: Instant`, `enqueue_at` deleted); the rotation loop (formerly lib.rs `spawn_heartbeat`) moved inside as `spawn_rotation` preserving 036's enable-under-lock and 034's live-before-queue ordering; `enqueue_and_emit`/`enqueue_and_fan_out` deleted, every ingest (http, settings test notifications, both pollers) routed through `accept`, which also encodes the News-never-offered connector gate (first test coverage of that rule — a News test notification previously leaked to telegram). Two deliberate behavior changes, both pinned: News-gate (test `accept_offers_manual_but_never_news`) and QueueFull-no-wake (test `accept_queue_full_propagates_nothing`, named in commit `0b9d2b5`). `/improve execute` on `exec/037-engine-v2` (`942c4d2`..`1e11337`), reviewed **APPROVE** — cargo 295+3, vitest 107, clippy/fmt/tsc green, seam greps structural. NOT yet merged — operator's call; worktree `.claude/worktrees/exec-037-engine-v2`.

Status values: TODO | IN PROGRESS | DONE | BLOCKED (with one-line reason) | REJECTED (with one-line rationale)

## Dependency notes

- **005 relocation is already complete** — both OpenCode configs use an
  external file reference and the credential file is `0600`; do not move
  tooling config again. The remaining completion gate is operator-confirmed
  replacement-key use plus revocation of the old key.
- **006 has a red repository baseline outside its scope at `b1981c9`** — full
  fmt fails in `settings.rs`, and full clippy fails on four
  unrelated lints. Its reviewed plan uses targeted gates and must not absorb
  that cleanup. The Rust total changed concurrently while 006 was reviewed, so
  its count step records the clean execution baseline and increments it rather
  than hard-coding the original total.

- **004 first** — it corrects the project-state files every subsequent
  agent session reads. 014/017/019 touch some of the same doc lines;
  whoever lands second reconciles (noted in each plan).
- **015 after 009** — 008 is DONE (015's baseline already includes it);
  009's seam pin should exist before the emit path is reworked.
- **022 is blocked only by its Step 0 decision gate** — 008 is DONE; the
  property-test model's expanded invariants are already written into the
  plan. The operator must choose execute-§9 vs re-park.
- **018 is DONE (2026-07-18, `7c84a02`)** — it was rescoped earlier that
  day (`review-plan` pass at `ede063a`) after 023 deleted lottie
  outright, mooting the lazy-lottie half; only the transform-based news
  shader swap remained, and it is now landed and verified (see the Done
  section).
- **016 / 017** touch `.github/workflows/ci.yml` or mirror it — 007
  already landed there (`--locked`, audit scans, `ubuntu-latest`,
  `sh -n`); both plans' texts account for it.
- **012 vs 001** — both touch `lib.rs`'s shortcut area; textual-only
  interaction, reconcile by reading.
- **020 / 021 vs 002** — all touch `SettingsApp.tsx` in different
  regions; reconcile textually.
- Not planned but flagged MED-confidence in the deep audit:
  **slot-state emissions can be delivered out of order** (five emitters
  compute under the queue lock but emit after releasing it). Plan 015
  fixes the heartbeat's instance as a side effect; the mutation-site
  instances remain — investigate after 015 lands if blank/ghost cards
  are ever observed around rotation boundaries.

Third audit session (036, 025–031):

- **036 first among these** — it fixes a shipped P1 regression (plan
  015's lost wakeup); its heartbeat loop is also touched by nothing
  else here, so it lands cleanly. (Review-plan pass 2026-07-18: 036
  also comes before the rev-3 batch's 033 and 034 — 033's auto-retract
  rides the heartbeat wake, and 034's status channel makes the
  heartbeat the sole emitter, a design that is unsound while the
  lost-wakeup race exists. Both plans now carry the dependency.)
- **028 after 036 and 025 (soft)** — both add rust tests; landing the
  fixture consolidation first would force them to rebase over moved
  fixtures. 028 should also re-inventory after the 022/024-proptest
  merges (its Step 1 is grep-based for exactly this reason).
- **025 vs 030** — 030's design doc should reference `net::build_poll_client`
  if 025 has landed by then (noted inside 030).
- **026 vs 032–035** — 026 edits CLAUDE.md/AGENTS.md/spec prose; the
  redesign plans will touch adjacent doc regions when they land.
  Whoever lands second reconciles textually (the 004/014/017/019
  precedent).
- **027 vs 032** — both touch frontend presentation; 027 is
  settings-window + App.tsx listener only, 032 is overlay card CSS —
  no file overlap expected except possibly `App.tsx`; reconcile by
  reading.
- **029 needs network** (tag→SHA resolution) and its definitive gate is
  a green CI run on push — operator-owed, like 007's was.
- **030/031 are spikes**: deliverable is a design doc in
  `docs/design/`, zero production code. The *features* stay undecided
  until the operator reads the docs.

Architecture-review session (037):

- **037 strictly after 036 and 025** — it rewrites the heartbeat loop
  036 fixes (preserving `enable`-under-lock inside the Engine) and the
  poller call paths 025 reshapes; running it earlier forces double
  rebases of a P1 bug fix. It also mechanically edits the proptest
  harness — if 024 is still in flight, coordinate before dispatch.
  033/035 change queue/http production surfaces: whoever lands second
  reconciles textually (the 004/014/017/019 precedent). 037 subsumes
  neither the emit-after-unlock reordering (still deferred — but after
  037 it is a one-site fix) nor the rejected lib.rs file-split finding
  (see the plan's "Why this matters" for the distinction).

## Findings considered and rejected

From the deep session (2026-07-17) — recorded so they aren't re-audited:

- **queue.rs / settings.rs / SettingsApp.tsx size**: mostly tests or
  already-internally-layered; splitting adds files, not clarity.
- **lib.rs multi-responsibility split** (window/tray/shortcuts modules):
  real but M-effort refactor with hardware-sensitive code and no
  behavior payoff; deferred until it actually impedes a change.
- **EventMeta all-`Option` news fields on every Event**: documented as
  presentation-only accretion; revisit at a second meta-carrying source.
- **No `Notifier` trait**: explicitly recorded in CONTEXT.md as deferred
  until a second connector exists.
- **Dependency version lag**: none material — all deps current-generation
  for 2026 (react 19, vite 7, tauri 2.11, axum 0.8.9, etc.).
- **Unused deps / lucide-react authenticity / wiremock**: all verified
  fine.
- **`npm audit`**: 0 vulnerabilities at planning time (plan 007 adds the
  standing gate).
- **Pre-commit hooks**: CI + justfile (017) cover it with less machinery.
- **`.env.example`**: N/A — secrets deliberately live in `secrets.toml`,
  not env.
- **Advisory `min`/`max` props duplicating `validate` ranges**: accepted
  duplication (enforcement is server-side); a bounds-map export isn't
  worth the plumbing (see plan 020).
- **`Instant` deadlines freezing across system sleep**: judged
  working-as-intended (items resume remaining time at wake); document if
  it ever surprises.
- **Double tracing fmt layer in release / stdout with no consumer**:
  real but tiny; fold into any future logging.rs change.
- **500×300 window vs 270×38 idle pill compositing cost**: needs
  `powermetrics` measurement before any fix; not planned.
- **Notification titles in the plaintext log / log files not 0600**:
  accepted for a single-user machine; noted in plan 006's maintenance
  section.
- **Percent-encoding the ESPN league slug**: covered practically by
  plan 013's boot validation of the slug shape.
- **tauri-nspanel objc→objc2 upstream migration**: needs a network check;
  folded into plan 007's maintenance notes for the next deliberate bump.

From the earlier `next` session (kept from its index):

- **Generalize the `Notifier` seam for a second outbound connector**
  (e.g. ntfy/Pushover) — no doc names a specific wanted second connector
  today. Revisit if Telegram "proves insufficient" or a specific target
  is named.
- **Posture module (AirPods motion via `CMHeadphoneMotionManager`)** —
  weakest-grounded direction finding; needs a design spike defining the
  trigger heuristic before any build step.

Direction options surfaced in the deep session but not selected for
plans: **OpenRouter news enrichment** (the stored-but-unused key's first
feature — best-effort summary/category into `EventMeta`) and **a first
Recurring/Topic producer** (live-match scoreboard card superseding in
place — the supersession machinery currently has zero production
producers) and **a "what did I miss" history surface**. All grounded;
re-raise with `improve next` when wanted. *(Third-session update: the
first two are now spike plans 030/031; the history surface remains
unplanned — weakest grounding, needs a persistence decision.)*

From the third audit session (2026-07-18) — recorded so they aren't
re-audited:

- **`App.tsx` appearance-listener unlisten race**: real but near-zero
  impact (overlay root never unmounts) — folded into plan 027 as a
  consistency fix, not a standalone finding.
- **Wall-clock heartbeat test** (`heartbeat_rotates_out_via_deadline_
  sleep_not_polling` uses real ~1 s sleeps inside a 3 s timeout — the
  suite's one real-timer async test, contra §9.1's simulated-clock
  discipline): accepted as-is; injectable-clock refactor is
  disproportionate for one characterization test. Revisit only if it
  flakes in CI (noted in plan 036's maintenance section).
- **Poller spawn-signature param bundles** (8 positional args,
  `too_many_arguments` allows): already self-recorded as tech-debt in
  code comments; deliberately not absorbed into plan 025.
- **`prototype/status-rail.html` reconcile/hygiene**: deferred — the
  file was under active uncommitted edit by a concurrent session during
  the audit (and that session has since filed plans 032–035 around it);
  any hygiene verdict would have fought live work.
- **Upstream health of `tauri-nspanel` (git-rev pin) / `smappservice-rs`
  (0.1.x)**: still unchecked (needs network) — carried forward from the
  deep session, now twice-deferred; fold into the next deliberate
  dependency bump rather than a standalone plan.
- **Slot-state emit-after-unlock reordering**: re-examined; still no
  new failure mode beyond the known reordering, still symptom-free —
  remains deferred per the note above.

## What was not audited (deep session)

The Swift `notchtap-detect` source beyond structure; full git-history
secret scanning; live/hardware behavior (nspanel, notch geometry,
animation look — manual-checklist territory by the repo's own design);
Rust advisory database (cargo-audit not installed — plan 007 adds it);
upstream repo health for `tauri-nspanel`/`smappservice-rs` (needs
network).

## What was not audited (third session, 2026-07-18)

Scope was the `d40445e..a58f115` delta plus fresh eyes on its
surroundings — not a full re-sweep of code the deep session already
covered. Additionally not audited: the uncommitted working-tree changes
and untracked `testing/` scratch (a concurrent session's live work);
the `022` deep-testing merges (`5b51855`..`d926977`, landed mid-audit —
the concurrent session's own executor/review covered them); exhaustive
assert-quality reads of all 45 `settings.rs` and 28 `rss_poller.rs`
tests (sampled only); `SettingsApp.tsx`'s IPC-argument construction
beyond an injection-sink pass; live `cargo audit`/`npm outdated`
(CI gates the former; lockfiles read for the latter); everything on the
deep session's not-audited list above (unchanged).
