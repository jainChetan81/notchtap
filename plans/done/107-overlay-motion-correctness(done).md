# Plan 107: Overlay motion correctness — celebration never obscures content, compact→idle as one state machine, TTL-restart instrumentation

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat 870cdeb..HEAD -- src/components src/styles.css src/settings/preview-overlay.css src-tauri/src/lib.rs src-tauri/src/engine.rs`
> — on content mismatch with the excerpts below, STOP.

## Status

- **Priority**: P2 (A and B are the two highest-ranked confirmed
  findings of the 2026-07-22 external UI review; C arms the one open
  bug suspect with data)
- **Effort**: M
- **Risk**: MED (B touches the promotion/idle render path; A touches a
  celebration the operator has already tuned twice)
- **Depends on**: 106 (soft — docs only, no file overlap). **Must land
  before 111** (111 deletes the CSS mirror this plan still edits under
  the mirror law).
- **Category**: bug + polish + instrumentation
- **Planned at**: commit `870cdeb`, 2026-07-22

## Why this matters

- **A.** The goal celebration's burst paints ON TOP of the card text at
  up to 0.95-opacity yellow. Verified stacking: `.card-assembly::after`
  is positioned with `z-index: 0`; `.card-content` (styles.css:457) is
  non-positioned and in-flow, so the positioned pseudo-element wins
  paint order. The defending comment (styles.css:260-262) only argues
  the burst sits over the *background* — the text case was never
  considered. For ~the first half of a 1240ms animation the goal you
  most want to read is the thing you can't.
- **B.** The compact→idle exit has a known, code-acknowledged geometry
  race — the ledger calls it "grows before shrinking; corners pop in
  late". The external review independently found the same mechanism
  the CSS comments already describe. No repro needed; the cause is
  understood (see below).
- **C.** The TTL timer "sometimes restarts" (operator report,
  unconfirmed). Rather than keep waiting for the operator to notice
  WHEN, log the signature so the next occurrence self-documents.

## Current state (verified 2026-07-22 at `870cdeb`)

- **Celebration** (`src/styles.css`, mirrored in
  `src/settings/preview-overlay.css`):
  - `:278-294` `.card-assembly::after`: `inset: -60px; z-index: 0;
    opacity: 0`, bright core `radial-gradient(circle at 50% 50%,
    rgba(255, 224, 130, 0.95), transparent 40%)` + outer
    `rgba(255,107,87,0.55)` glow.
  - `:301` `animation: goal-burst 1240ms ease-out`; `@keyframes
    goal-burst` (`:338-355`) hits `opacity: 1` at 35%, scale 0.5→1.7.
    Overshoot + ring also 1240ms (`:297`, `:320`).
  - `:392-401` `.cele-ripple span`: `ripple-out 1440ms ease-out`,
    three spans, delays 0/280/560ms.
  - `.card-assembly` has `isolation: isolate`; `.card-content`
    (`:457`) is NOT positioned and carries no z-index.
  - These durations are plan 100's deliberate 2× slowdown — an
    explicit operator request. **Do not shorten total durations.**
- **Compact→idle race** (`src/components/StatusRailCard.tsx:191-193`):
  `swapKey = showing ? slot.id : "idle"`;
  `useDelayedSwap(slot, swapKey, 220)`;
  `renderedShowing = renderedSlot.state === "showing"`. The shell
  class (`.idle` vs priority class) is computed off the LIVE `showing`
  flag; `.below-block` mount/unmount is gated on the DELAYED
  `renderedShowing`/`exiting`. `styles.css:143-157` documents the
  mismatch verbatim ("that class is computed off the LIVE `showing`
  flag… which flips a render ahead of `renderedShowing`") and works
  around the corner-rounding half with a `:has(.below-block)` hack.
  The width half is NOT worked around: during exit the shell snaps to
  idle width while content is still fading (or vice versa on some
  paths) — the visible "grows before shrinking".
- **TTL suspect**: no instrumentation exists. The wire carries
  `remaining_ms`/`ttl_ms` (excluded from `dedup_eq` per the
  CLAUDE.md rule). Plan 097 already fixed one real restart-adjacent
  bug (supersede top-up using raw elapsed); the operator's report
  post-dates that build, so something else may remain — or not.

## Commands you will need

`npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` /
`npx vite build` from the worktree root; `cargo test --locked`,
`cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt
--check` from `src-tauri/` (Step C only).

## Scope

**In scope**: `src/styles.css`, `src/settings/preview-overlay.css`
(mirror law — every CSS change lands in both), `src/components/
StatusRailCard.tsx`, its test file, `src/useDelayedSwap.ts` (or
wherever the hook lives) IF Step B needs a rendered-state export it
doesn't have, `src-tauri/src/lib.rs` and/or `engine.rs`/`queue.rs`
for Step C's queue-owned sample state, pure predicate, emission-boundary
wiring, warning, and focused tests (instrumentation only—no queue
behavior or wire-shape change), `docs/TESTING_STRATEGY.md` §0 (counts,
last).

**Out of scope**: celebration total durations (plan 100's 2× stands);
`.chip-live .live-dot`; every hover feature; `capabilities/`,
`build.rs`; the Settings window.

## Steps

### Step A: celebration must never obscure text
Contract: at every frame of the celebration, the card's title/body
remain readable. Achieve it by **stacking + opacity**, not by
shortening plan 100's timings:
1. Make `.card-content` a stacking participant above the burst:
   `position: relative; z-index: 1;`. **(Cold-read correction,
   2026-07-22)**: the mirror file has NO `.card-content` rule today
   (`grep -c "card-content" src/settings/preview-overlay.css` → 0),
   and every selector in the mirror is prefixed `.appearance-preview`
   (its scoping convention — e.g. the burst lives at
   `.appearance-preview .card-assembly::after`, mirror `:145`, and
   `goal-burst` at mirror `:202`; the mirror's line numbers never
   match styles.css's, locate by selector). So: edit the existing
   `.card-content` rule in `styles.css` (`:457`), and CREATE
   `.appearance-preview .card-content { position: relative;
   z-index: 1; }` in `preview-overlay.css` with a comment noting it
   was born here (plan 107) because the preview renders card text
   over the mirrored burst too.
2. Lower the burst's peak: bright-core alpha 0.95 → ~0.55 and cap the
   `goal-burst` keyframe at `opacity: 0.7` instead of 1 (tune by eye
   in the preview; state your final values in the report).
3. Keep: scale overshoot, ring, ripples, all durations/delays.
4. Update the styles.css:260-262 comment to state the full contract
   (burst above surface, below content).
**Verify (corrected at review round 2)**: `position: relative` alone
is NOT the guarantee — with `z-index: auto` the `::after`
pseudo-element still paints after (above) an earlier positioned
sibling in tree order; **`z-index: 1` is the decisive declaration**.
jsdom can't compute cascade from stylesheets, so the pin is
string-level against BOTH CSS files: a test (or the
StatusRailCard.test.tsx suite) reads `src/styles.css` and
`src/settings/preview-overlay.css` and asserts (per file, matching
that file's selector form — bare `.card-content` in styles.css, the
`.appearance-preview`-prefixed rule created in step 1 in the mirror)
that the rule block contains BOTH `position: relative` and
`z-index: 1`, and that the celebration `::after` keeps `z-index: 0`
— plus the DOM test that `.card-content` is present during a
goal-celebration render. Cross-reference the CSS comment to this
test by name.

### Step B: compact→idle as one directional state machine
**Corrected at plan review (2026-07-22)** — the original step assumed
the delayed swap passes promotion content through immediately; it
does NOT. Verified: `useDelayedSwap` (`src/useDelayedSwap.ts:20-30`)
delays EVERY key change by `exitDurationMs`, direction-blind. So
today's ENTERING choreography is: shell grows immediately (live
`showing`), content mounts 220ms later — that lead is existing
shipped behavior and stays. Only the EXIT direction is broken.

Directional contract:
- **Entrance (idle→showing)**: geometry keys off live `showing` —
  grows on the same render the promotion arrives (unchanged from
  today; content follows 220ms later as it does now).
- **Exit (showing→idle)**: ALL geometry-driving state holds its
  rendered value until the delayed content actually unmounts (the
  220ms swap), THEN transitions to idle — no early shrink-to-grow,
  no late corner pop. **(Widened at review round 2)**: "geometry"
  is not just the showing/idle shell class — it includes the
  EXPANDED state and any priority-sized classes. If only the shell
  class is held, an expanded card still collapses in two visible
  stages (expanded→compact when the live slot empties, then
  compact→idle at the swap). During exit, expanded/priority classes
  must derive from the RENDERED slot (`renderedSlot.expanded` etc.),
  so an expanded card exits as an expanded card in one motion.
  Check what drives the expanded class today before assuming which
  way it currently falls — start at the card-class construction
  adjacent to the swap wiring (`StatusRailCard.tsx:185-193`, where
  `swapKey`/`renderedShowing` are computed) — and pin whichever
  direction with a test.

Implementation, in order of preference:
1. **Preferred: pure derived state, hook untouched** — drive geometry
   from `showing || renderedShowing` (true when EITHER the live slot
   shows or delayed content is still mounted), and choose the geometry
   source explicitly: `geometrySlot = showing ? slot : renderedSlot`.
   Entrance therefore reads priority/expanded from the new LIVE slot;
   exit reads them from the still-mounted RENDERED slot. Derive the
   shell priority class and `expanded` class from `geometrySlot`, never
   from the boolean alone. This yields exactly
   the directional contract above with zero hook changes; plan 078's
   tested swap semantics stay byte-identical. (This is why the
   reviewer-suggested hook restructuring is NOT the first move: the
   asymmetry we want is derivable at the call site, and the hook is
   shared with `IdleHoverPeek` — a directional parameter would tax an
   unrelated consumer.)
2. **Fallback, authorized if (1) proves insufficient**: add an
   explicit directional option to `useDelayedSwap` (e.g.
   `swapImmediatelyWhen(prevKey, nextKey)`) with its own hook tests.
   Restructuring the hook is IN scope on this path — the old STOP
   condition ("report the design instead") is deleted; report the
   fallback choice and why (1) fell short.
3. Remove the now-unneeded `:has(.below-block)` rounding workaround in
   both CSS files ONLY if the new keying makes it redundant; if it
   still serves the entering direction, keep it and say why.
4. The `.bare` mode gating (plan 105) keys off delayed state already —
   keep it consistent; its tests must stay green unmodified.
**Verify**: tests — (1) simulate showing→idle: while the exit
animation is live (before the 220ms swap), the assembly still carries
the showing-geometry class; after the swap it carries `.idle`;
(2) idle→showing promotion applies showing geometry on the SAME
render the promotion arrives (pinning today's width-leads-content
entrance — content still mounts after the swap delay, and that is
correct); (3) an EXPANDED card's showing→idle exit keeps its
expanded class until the swap (the two-stage collapse is the bug
this pins); (4) plan 105 bare-mode tests untouched and green.

**Browser motion gate (UI evidence, not jsdom)**: in headless Chromium
at a fixed viewport, inject deterministic expanded and compact slots,
then drive each to idle while sampling `.card-assembly` and
`.below-block` bounding rects/classes at 0/110/220/400ms. Width must
move monotonically from its showing endpoint to idle—no reversal and
no intermediate value outside the two endpoints—and the expanded
class must survive through the 220ms content swap. Save the trace plus
a four-frame contact sheet to the scratchpad/report. For Step A, freeze
the goal burst at its peak frame and save a 2× screenshot plus computed
z-index evidence showing content above the burst. These browser checks
are mandatory on the dev machine; physical-notch feel remains the
operator smoke check.

### Step C: TTL-restart instrumentation (cheap, log-only)
**Corrected at plan review (2026-07-22)** — the original predicate
(`new_remaining > prev_remaining + slack`) misses the canonical
reported pattern: emissions are SPARSE (`remaining_ms` is excluded
from `dedup_eq`, so ticks don't emit), and a buggy reset back to the
full TTL is NOT numerically greater than the previous emission's
value. Example: emit at 8000ms remaining → 5s pass silently → buggy
reset re-emits 8000ms. `8000 > 8000 + slack` is false, yet the
frontend visibly jumps ~3000→8000. The comparison must be against
the ELAPSED-ADJUSTED expectation, not the raw previous value.

Predicate (pure, unit-tested — following the pure-decision-vs-boundary
split that CLAUDE.md cites via TESTING_STRATEGY §4.4; §4.4 itself is
the presentation.rs case of that pattern, not a TTL section — cite
the PATTERN, not the section, in code comments):
`fn is_ttl_restart(prev_ttl_ms, prev_remaining_ms,
elapsed_since_prev_emission_ms, hover_held_delta_ms, new_ttl_ms,
new_remaining_ms, slack_ms) -> bool`.

First, a changed total window (`new_ttl_ms != prev_ttl_ms`) is NOT a
restart: manual expansion and a legitimate Topic-supersede top-up both
change `ttl_ms`, so treating either as suspicious would create the
detector's most likely false positive. With an unchanged window,
restart iff `new_remaining > prev_remaining - (elapsed -
hover_held_delta) + slack` (all subtraction saturating; slack ~500ms
covers scheduling jitter).

**State placement is fixed, not open-ended.** `SingleSlotQueue`
already owns `last_emitted: Option<SlotState>` and all hover accounting;
add one adjacent queue-owned `TtlEmissionSample` field containing item
id, ttl/remaining, emission `Instant`, and the cumulative hover-held
total as of that sample. Do NOT use a process-global static or an
engine-side map: both duplicate queue identity/lifetime, complicate
parallel tests, and can leak samples across Engine instances. Add a
small queue helper that returns cumulative hover-held time at an
injected `now` (banked total plus the in-flight session when present),
so the detector subtracts only the DELTA since the prior sample.

Observe every attempted slot-state wire emission, including the
unconditional `emit_current_blocking` page-load re-emit, not just
`slot_state_if_changed`; otherwise a reload-triggered jump is invisible
to the instrument. Parameterize the sampling helper with `Instant` so
tests are deterministic and the state + timestamp are captured from
one clock read. Reset the sample without warning when the item id
changes or the state becomes Empty.

Boundary of the evidence: this Rust warning detects a BACKEND WIRE
jump. A visible restart with no warning implicates frontend remount/
re-anchor behavior instead; state that explicitly near the detector
and in the implementation report rather than claiming it can observe
a React-only restart.
On detection, `tracing::warn!` one line with: item id, prev/new
remaining, elapsed, hover context, whether a supersede happened this
tick (omit if not cheaply available). No behavior change, no config.
**Verify**: pure-function tests — (a) normal decay after 5s
(8000→~3000) → no warn; (b) hover-held 5s then re-emit 8000 → no
warn; (c) the canonical pattern (8000, 5s elapsed unhovered, re-emit
8000) → WARN — this case is the whole point and must be red without
the elapsed adjustment; (d) small jitter within slack → no warn;
(e) a legitimate supersede/manual-expand change where `ttl_ms` grows
→ no warn; (f) a new item id → sample reset, no warn. Plus queue-driven
tests that a hover-pause sequence never warns and an unconditional
page-load re-emit participates in sampling without warning.
`cargo test --locked` green.

### Step D: gates + §0
All frontend gates + all rust gates → clean. Update
`docs/TESTING_STRATEGY.md` §0 to observed counts with attribution.

## Done criteria

- [ ] `.card-content` stacks above the celebration burst in both CSS
      files (bare rule in styles.css; `.appearance-preview`-prefixed
      rule CREATED in the mirror); peak burst opacity reduced; all
      plan-100 durations byte-unchanged — baseline counts per file
      (cold-read verified): `1240ms` ×3, `1440ms` ×1, `920ms` ×1 in
      EACH of styles.css and preview-overlay.css; identical after
- [ ] Step B's four tests pass; entrance geometry applies on the
      promotion render (today's width-leads-content entrance pinned,
      not removed); plan 105 bare tests unmodified
- [ ] TTL predicate is a pure tested function taking ttl, elapsed +
      hover-held delta; queue-owned sample state covers changed emits
      AND page-load re-emits; test (c) — reset-to-initial after silent
      elapsed — passes and is red without the elapsed adjustment;
      changed ttl/new id/hover pause do not trigger it
- [ ] Mirror law held (`preview-overlay.css` diff mirrors `styles.css`)
- [ ] All gates clean; §0 matches observed counts; only in-scope files

## STOP conditions

- Step B: BOTH the derived-state approach and the directional hook
  fallback fail to produce the contract (report both attempts; do not
  invent a third mechanism unreviewed).
- The rounding workaround removal changes any entering-direction test.
- Step C cannot route every slot-state emission attempt through the
  queue-owned sampling helper without changing the public wire contract
  or introducing a second source of queue truth (report the uncovered
  path; do not fall back to a global static/map).
- Content mismatch against the excerpts above.

## Maintenance notes

- Step A/B have deterministic headless-browser evidence in addition to
  unit tests. Physical-notch feel is still operator-owed because the dev
  machine is notchless; that smoke check judges polish, not correctness
  of the stacking/directional contracts already pinned above.
- If the operator's TTL restart recurs post-107, the warn line turns
  the anecdote into a timestamped, context-carrying repro. If it
  never fires again across a week of real use, 097 probably was the
  fix and the suspect can be closed.
- Step A deliberately spends none of plan 100's timing budget; if the
  operator still finds the celebration too loud after the opacity
  cut, the next lever is the `-60px` inset (smaller bloom), not
  duration.
