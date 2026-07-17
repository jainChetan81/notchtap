# Plan 023: Close the invisible goal-celebration bug — work the review log's ranked list, then redesign the moment

> **Executor instructions**: Follow this plan step by step. This plan is
> partly diagnostic (Steps 1–3 are live-session checks on the dev
> machine) and partly visual (final look needs human eyeballs) — an
> automated executor can do Steps 0 and 4–5 but must hand Steps 1–3 and
> the acceptance check to the operator. Run every verification command
> and confirm the expected result before moving on. If anything in "STOP
> conditions" occurs, stop and report. When done, update this plan's
> status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src/styles.css src/components/GoalCelebration.tsx src/components/StatusRailCard.tsx src/assets/lottie/goal-celebration.json docs/review-logs/2026-07-17-goal-burst-bug.md`
> On any change, re-read the review log — it may have been updated with
> findings from another session; reconcile before starting.

## Status

- **Priority**: P2
- **Effort**: M (S if hypothesis 1 — stale HMR — explains everything)
- **Risk**: LOW (presentation layer only; the signal wire path is
  verified working)
- **Depends on**: none. Plan 018 (lazy lottie) touches the same
  component — do THIS plan first if both run in one session.
- **Category**: bug / direction
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

Goals are the marquee High-priority moment the ESPN poller exists for
(`espn_priority` defaults to `high`), and the celebration — radial burst,
scale overshoot, lottie confetti — currently pays its full render cost
while delivering **zero visible effect**. The maintainer's own review log,
`docs/review-logs/2026-07-17-goal-burst-bug.md`, documents the live bug,
one fix already landed (z-index/`isolation` — commit `5277c55`), a ranked
hypothesis list explicitly labeled "next session works this list
top-down", and written acceptance criteria. This plan is that next
session, made executable.

## Current state

Read `docs/review-logs/2026-07-17-goal-burst-bug.md` in full before
starting — it is the primary source. Summary of its state:

- **Symptom**: `./notchtap --title "GOAL" --body "..." --priority high
  --signal goal` shows the coral High card correctly but no burst, no
  overshoot, no confetti (mac mini, HUD mode, dev build).
- **Verified**: the signal reaches the frontend end-to-end (this is
  presentation-only); the z-index bug WAS real and was fixed
  (`.rail-card::after` and `.goal-celebration` sat at `z-index: -1`
  behind an opaque card background; now `z-index: 0` +
  `isolation: isolate`); the fix was re-tested only via vite HMR and
  still showed nothing — so either HMR never applied it, or a second
  suppressor exists. macOS Reduce Motion was checked: off.
- **The log's ranked hypotheses** (work top-down):
  - (1) HMR never applied the fix — restart `npm run tauri dev` and
    re-test before trusting ANY css conclusion from the prior session.
  - (2) the `pulse-goal` class is never applied — verify in devtools that
    the card element gains `pulse-goal` for ~620 ms on a goal push
    (`StatusRailCard` keys the pulse effect on `[currentId,
    currentSignal]` — see `src/components/StatusRailCard.tsx:~33-52`:
    goal → `setPulse("pulse-goal")`, cleared by `onAnimationEnd` matching
    `PULSE_END_ANIMATION`).
  - (3) lottie mount/autoplay — confirm `GoalCelebration` mounts and
    plays (react devtools/console).
  - (4) paint-order edge with `overflow: hidden` + isolation —
    screenshot computed styles before touching css again.
  - (0 — applies even once fixed) **the moment is under-designed**:
    `goal-celebration.json` is 6 particles, 120×120, 24 frames @ 30 fps =
    0.8 s — near-invisible on a dark 400 px card even when correctly
    stacked. Likely resolution is a redesign: bigger/longer particle
    field, or drop lottie for a scaled-up css burst alone.
- **Acceptance (from the log, verbatim intent)**: with reduce-motion OFF:
  a goal push shows burst + overshoot + confetti once, ~620 ms, never
  looping; a red card shows the two-pulse strobe; with reduce-motion ON:
  whatever fallback is chosen — including deliberately nothing — is
  **documented in `docs/ARCHITECTURE.md`**.

Relevant files: `src/styles.css` (`.rail-card::after` burst,
`pulse-goal`/`pulse-red` keyframes — find with
`rg -n "pulse-goal|rail-card::after|goal-celebration" src/styles.css`),
`src/components/GoalCelebration.tsx` (lottie wrapper, plays once, never
loops), `src/components/StatusRailCard.tsx` (pulse state machine + the
conditional celebration mount), `src/assets/lottie/goal-celebration.json`
(must remain self-authored/bundled — the file's comment records the
no-CDN, no-third-party-asset rule; any replacement asset must honor it).

Devtools access (from the log): right-click the overlay → Inspect (dev
builds), or Safari → Develop → the notchtap webview.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Dev app | `npm run tauri dev` (repo root) | overlay + logs |
| Goal push | `./notchtap --title "GOAL" --body "test" --priority high --signal goal` | exit 0, card shows |
| Red-card push | `./notchtap --title "RED" --body "test" --priority high --signal red_card` | exit 0 |
| Frontend gates | `npx vitest run && npx tsc --noEmit && npx vite build` | all pass |

## Scope

**In scope**:
- `src/styles.css` (burst/pulse rules), `src/components/GoalCelebration.tsx`,
  `src/components/StatusRailCard.tsx` (pulse wiring only if a bug is found
  there), `src/assets/lottie/goal-celebration.json` (replace/redesign —
  self-authored only), `src/components/StatusRailCard.test.tsx` (pulse
  assertions if wiring changes)
- `docs/ARCHITECTURE.md` (the reduce-motion fallback decision — required
  by the log's acceptance)
- `docs/review-logs/2026-07-17-goal-burst-bug.md` (append the resolution —
  matches how this repo's review logs are used)
- `plans/README.md` (status row)

**Out of scope**:
- The signal wire path (rust `EventSignal`, queue, emit) — verified
  working.
- `.news-shade` and every non-goal animation (plan 018's territory).
- Making the celebration loop or exceed ~1 s — the "never obnoxious"
  comments in GoalCelebration.tsx record that decision.

## Git workflow

- Current branch; commit style: `overlay: goal celebration visible — <root cause> + redesigned burst` (one commit per real change; diagnostics produce no commits).
- Do NOT push.

## Steps

### Step 0: Re-verify current behavior cold (no HMR doubt)

Kill any running dev instance. `npm run tauri dev` fresh. Fire the goal
push. Record exactly what is visible (card? overshoot? burst? confetti?).

**Verify**: written observation in the report. If everything already
works (the z-index fix + a cold start was the whole bug — hypothesis 1),
skip to Step 4 (the moment is still under-designed per hypothesis 0) or,
if the operator judges the current look sufficient, go straight to
Step 5's documentation and close.

### Step 1–3: Work the log's hypotheses top-down (operator, devtools)

Exactly as the log prescribes (see Current state): (2) confirm
`pulse-goal` lands on the card element for ~620 ms; (3) confirm
`GoalCelebration` mounts and the lottie plays; (4) if all pass but
nothing is visible, capture computed styles/stacking of
`.rail-card::after` and `.goal-celebration` and fix the one CSS suppressor
found. Each finding → smallest possible fix → re-test cold (no HMR
conclusions — the log was burned by that once).

**Verify**: after this step, the EXISTING assets are visibly rendering
(however faintly) on a goal push, or the precise suppressor is fixed and
committed with a one-line cause note.

### Step 4: Redesign the moment (hypothesis 0)

The current asset is objectively too small/short (6 particles, 120×120,
0.8 s). Two sanctioned directions (operator picks based on Step 0–3
findings):
- **CSS-first**: scale up the existing `.rail-card::after` radial burst
  (size/opacity/duration to ~620 ms), add the scale overshoot on the
  card, and drop the lottie layer entirely (then plan 018's lazy-load
  step becomes moot — note it in the index); or
- **Bigger lottie**: author a fuller-field particle JSON (self-made,
  e.g. via a lottie editor — the no-third-party-asset rule applies),
  ~40+ particles, card-sized (≥400×80 coverage), ≤1 s, play-once.

Either way: keep red-card's two-pulse strobe working (`pulse-red`), keep
play-once/never-loop, and keep the celebration keyed to
`signal === "goal"` only.

**Verify**: `npx vitest run && npx tsc --noEmit && npx vite build` → all
pass (adjust StatusRailCard tests only if wiring changed); goal push on
the dev machine shows an unmissable ~620 ms celebration; red-card push
shows the strobe; a plain high-priority push (no signal) shows NEITHER.

### Step 5: Record the decisions

- `docs/ARCHITECTURE.md`: add the reduce-motion decision the log's
  acceptance requires (e.g. "reduce-motion ON → burst and confetti are
  suppressed, the card still pulses once / or deliberately nothing —"
  whichever the operator picks). Wire the chosen behavior via the
  existing `@media (prefers-reduced-motion: reduce)` pattern in
  styles.css if not already covered.
- Append the resolution (root cause, fix, redesign summary) to
  `docs/review-logs/2026-07-17-goal-burst-bug.md`.

**Verify**: `grep -c "reduce" docs/ARCHITECTURE.md` → ≥1 new mention in the celebration context; review log updated.

## Test plan

- Existing `StatusRailCard.test.tsx` pulse-related tests stay green
  (adjust only for deliberate wiring changes).
- No new automated tests for the visuals — animation look is manual-only
  by recorded design (`docs/TESTING_STRATEGY.md` §5). The acceptance
  criteria in the review log are the manual test script; run them
  verbatim on the dev machine (mac mini); the macbook eyeball joins the
  standing §6 hardware checklist.

## Done criteria

- [ ] Goal push → visible burst + overshoot + confetti (or the chosen
      css-only equivalent), once, ~620 ms, on a cold-started dev build
- [ ] Red-card push → two-pulse strobe; signal-less push → no celebration
- [ ] Reduce-motion fallback decided AND documented in ARCHITECTURE.md
- [ ] Review log updated with root cause + resolution
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] `plans/README.md` status row updated (note for plan 018 if lottie
      was dropped)

## STOP conditions

- Steps 1–3 exhaust the log's hypotheses and the suppressor is still
  unfound — append your observations to the review log (that's the
  log's own protocol) and stop; do not start speculative CSS rewrites.
- The redesign requires a third-party lottie asset — forbidden by the
  recorded no-external-asset rule; report.
- No operator/dev-machine available for the live steps — mark BLOCKED
  (this plan cannot complete headless).

## Maintenance notes

- If the css-first direction wins, remove lottie-react + the JSON from
  the bundle in the same change (Cargo of plan 018's Step 1 disappears;
  update that plan's status/scope note in the index).
- Any future celebration must keep: play-once, ≤~1 s, keyed on signal
  (never priority alone), reduce-motion fallback per the new
  ARCHITECTURE.md line.
