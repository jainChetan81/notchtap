# Plan 086: SPIKE — hover enablement: dynamic cursor-event tracking on the overlay window

> **Executor instructions**: This is a decide/investigate-scoped plan,
> not a build plan — its deliverable is a WRITTEN RECOMMENDATION with an
> effort estimate (and, conditionally, a small proof-of-concept), not a
> shipped feature. Its outcome GATES the hover-halves of plans 081 (TTL
> hover-pause), 082 (weather peek), 084 (rail→scorecard hover reveal),
> and the locked idle expanded-on-hover state — those plans' hover steps
> stay frozen until this spike concludes. Follow the steps in order. If
> the investigation doesn't clearly favor one mechanism, STOP and
> present the options to the operator rather than picking one yourself —
> this is exactly the kind of judgment call this plan exists to surface,
> not resolve unilaterally (plan 073's precedent). Security and
> performance analysis is a required part of the deliverable: one
> candidate mechanism is a global event tap, which is a sensitive
> capability in a repo that locks the frontend down to two event
> permissions. When done, update the status row for this plan in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 4fb3af9..HEAD -- src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/capabilities/default.json docs/ARCHITECTURE.md`
> Any diff in `lib.rs` means the native-config line refs below shifted —
> re-read before investigating. A diff adding hover machinery anywhere
> means someone jumped the gate — reconcile before proceeding.
> (Baseline re-stamped `71e54a7` → `4fb3af9` on 2026-07-21: plan 063's
> merge added a +41-line block to `lib.rs`'s `on_page_load`, shifting
> `apply_overlay_native_config` and its comment to :544-574 — refreshed
> and re-verified by direct read.)

## Status

- **Priority**: P2 (its outcome unblocks four locked-but-frozen
  interaction designs)
- **Effort**: S (investigate; the POC branch adds S at most)
- **Risk**: LOW to investigate — read-only plus an optional throwaway
  POC; the DECISION it produces is high-leverage (a wrong pick either
  reintroduces the 2026-07-17 menu-bar-click-swallowing bug or signs
  the app up for an Input Monitoring permission prompt)
- **Depends on**: none. Gates: the hover-halves of 081/082/084 and the
  idle expanded-on-hover state (079's locked three-states design).
- **Category**: tech-debt / architecture (investigate)
- **Planned at**: commit `71e54a7`, 2026-07-20. **Review-plan pass 2
  (2026-07-21, against `4fb3af9`)**: the one code citation block
  re-verified — `apply_overlay_native_config` now lives at
  `lib.rs:544-574` (`set_ignore_cursor_events(true)` at :554, the
  2026-07-17-bug comment at :546-553, the `objc2_app_kit` use at :545),
  shifted +15 by 063's `on_page_load` block; fixed below. Capabilities
  file confirmed still exactly the two event permissions. Drift
  baseline re-stamped to `4fb3af9`.

## Why this matters

Every hover interaction in the locked redesign — idle weather peek,
football rail→compact reveal, TTL-bar hover-pause, the expanded-on-hover
idle state — is mocked and operator-approved in the prototypes, and
every one is blocked by the same line of code. Plan 079's consolidated
file documented the blocker while exploring hover mockups
(`plans/079-overlay-visual-revamp-consolidated.md`, item 17's
"bigger than previously scoped" note): **no hover or mouse interaction
works anywhere on the real shipped window today, and this is
deliberate, not an oversight.** The prototypes' footer says the same
("the hover *trigger* itself … is still separate interaction-model
work — not shown or implied as free here"). Until a mechanism is chosen
and costed, 081/082/084 can only ship their non-hover halves. This
spike exists to make that call deliberately, with the security and perf
trade-offs named up front, not discovered mid-build.

## Current state

- `src-tauri/src/lib.rs:544-574` — `apply_overlay_native_config` calls
  `window.set_ignore_cursor_events(true)` UNCONDITIONALLY (:554), on the whole
  overlay window. The comment (:546-553) records why: the window sits
  flush over the real menu bar at `NSStatusWindowLevel`, so without
  click-through it swallows clicks meant for other apps' menu-bar icons
  (the exact 2026-07-17 bug this line fixed). Safe today because the
  frontend is receive-only — every interaction is a global hotkey
  (⌃⇧N/⌃⇧O), never a click.
- Net effect (locked-design constraint, restated in
  `plans/frontend-ui-consolidated.html`'s Engineering constraints):
  any hover design needs real re-engineering — almost certainly
  tracking actual cursor position and toggling
  `ignore_cursor_events` on/off dynamically only while the cursor is
  within the card's current rendered bounds, or the icon-swallowing bug
  comes back.
- Building blocks already in the repo:
  - `objc2_app_kit` in use at `lib.rs:545` (NSWindow-level calls are an
    established pattern here).
  - The swift-subprocess precedent: `notchtap-detect` — a standalone
    swift CLI printing JSON to stdout, rust shells out and parses
    (`docs/ARCHITECTURE.md` §5). A `notchtap-cursor` helper is the
    same shape.
  - `tauri-nspanel` is already a dependency (pin bumped in plan 045) —
    its NSPanel behaviors are worth checking for mouse-related options
    before writing anything new.
  - Idle-CPU discipline is load-bearing history: the 250ms tick was
    deliberately replaced with deadline-based wakeups (plan 015) and
    news-shader repaints were cut for idle cost (plan 018). A polling
    mechanism must justify its wake rate against that history.
  - The repo's lockdown posture: `capabilities/default.json` = exactly
    `core:event:allow-listen`/`allow-unlisten`; frontend receive-only;
    no frontend network. A global event tap sits in visible tension
    with that posture and needs an explicit, honest treatment.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests (only if a POC touches code) | `cd src-tauri && cargo test --locked` | all pass |
| Clippy (POC only) | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Swift helper build (POC only, if the subprocess route is probed) | `swift build` in the helper's dir (follow `notchtap-detect`'s own build recipe — read it first) | exit 0 |

## Scope

**In scope**:
- Read-only investigation of the three mechanism families below,
  against the real code and (where the question is behavioral) the real
  dev machine.
- A written recommendation + effort estimate per the Deliverable
  section.
- ONE conditional small POC (Step 2) if it's cheap and decisive.
- The security/perf analysis — required, not optional.

**Out of scope**:
- Shipping hover to any plan (081/082/084, the idle expanded state) —
  they consume this spike's OUTPUT; nothing here lands in the app's
  render path.
- Re-deciding whether hover designs are desirable (locked already) —
  this spike answers HOW and AT WHAT COST, and may conclude "not worth
  it," but doesn't re-litigate the designs themselves.
- The lifecycle-pause engine work 081's Step 5 contemplates — even if
  this spike unblocks hover, that queue-deadline work is its own plan.
- Changing `apply_overlay_native_config`'s current behavior in any
  committed code (the POC, if any, is throwaway or behind a dev flag
  and called out as such).

## Mechanisms to investigate

(a) **Dynamic `ignore_cursor_events` toggling off a global cursor-
position watcher.** Two watcher sub-variants: (a1) rust-side polling of
the global cursor position (tauri/tao exposes cursor position; NSEvent
`mouseLocation` via the already-used `objc2_app_kit`) at a bounded
rate, arming click-through OFF only while the cursor is inside the
card's current rendered bounds plus hysteresis; (a2) a swift-subprocess
CGEvent tap (`notchtap-cursor`, ARCHITECTURE.md §5's precedent)
streaming mouse-moved events as JSON lines — event-driven, no polling,
but a *listen-only global tap on mouse-moved* typically triggers
macOS's Input Monitoring permission. Evaluate both wake-cost and
permission stories honestly.

(b) **`tauri-nspanel` capabilities.** The dep is already pinned and
loaded; check the pinned rev for panel-level mouse options (mouse
acceptance modes, tracking behavior) that could give enter/exit
detection without a global watcher. If the panel route can deliver
hover only inside the card bounds WITHOUT disabling click-through
elsewhere, it may be the smallest-diff answer — verify against the
actual pinned source, not docs.

(c) **Always-on tracking region.** An `NSTrackingArea` on the window's
content view (via `objc2_app_kit`) delivering mouse-entered/exited —
but tracking areas require the window to accept mouse events, which
conflicts with the unconditional click-through. Evaluate whether a
hybrid works: tracking region armed only in a small hover zone around
the card (click-through off there, on everywhere else), and what that
does to menu-bar icons the zone overlaps. If the zone must move as the
card resizes (idle 270px vs expanded 500px), cost that bookkeeping.

## Steps

### Step 1: Investigate

Read `lib.rs`'s full native-config function and its re-apply site, the
`tauri-nspanel` pinned rev's relevant source, and (for the tap variant)
what macOS permission a listen-only CGEvent tap on `mouseMoved`
actually triggers on the dev machine's macOS version. Answer, with
evidence:

1. Can enter/exit detection within the card's bounds be had WITHOUT a
   global watcher (mechanism b or c)? If yes, at what code cost, and
   does it survive the card's resize/reposition (idle ↔ showing ↔
   expanded widths)?
2. If a global watcher is required (mechanism a): what's the cheapest
   honest wake pattern — poll rate vs tap — measured against this
   repo's idle-CPU history (plans 015/018)? Is the tap's Input
   Monitoring prompt one-time and explainable, and is that acceptable
   for this app's posture?
3. Failure modes: what happens to hover when the cursor is already
   inside the card at toggle time, when the card rotates out from under
   the cursor, when a Space switch or fullscreen app changes the
   window's relevance? Any answer MUST NOT reintroduce the
   menu-bar-click-swallowing bug — for each mechanism, state precisely
   when click-through is off and why that's safe.
4. Effort estimate per viable mechanism (S/M/L, with the rust/swift/
   frontend split) and which you'd build.

**Verify**: no command — this is a reasoning step. The deliverable is
the written analysis itself.

### Step 2 (conditional): Small POC

Only if Step 1 has a clear front-runner AND a cheap decisive test
exists (e.g. a 50-line rust patch polling cursor position and logging
would-be hover transitions, or a minimal swift tap printing events):
build it, run it on the dev machine, and record whether enter/exit
detection tracks the real card bounds without perceptible lag and
without the menu-bar bug. Throwaway or dev-flagged — do NOT wire it
into any render path.

**Verify** (only if a POC lands): the POC builds/runs as described;
`cd src-tauri && cargo test --locked` still all pass if any committed
file changed (ideally none did).

### Step 3: Recommendation

Write the recommendation (a few paragraphs, in the completion report
and appended to this file): the chosen mechanism + effort estimate, or
"no clearly-best mechanism — options presented" (STOP), or "cost/
permission not worth it — hover stays frozen" (a legitimate outcome;
081/082/084 then ship non-hover-only permanently and the locked
idle expanded-on-hover state gets revisited by the operator). Include
the security analysis (permission prompts, what a tap can see, how the
chosen mechanism relates to the capabilities lockdown) and the perf
analysis (wake rate, idle cost) either way.

**Verify**: no command — the recommendation is the deliverable.

## Test plan

- Nothing to unit-test in an investigate spike (per
  `docs/TESTING_STRATEGY.md`'s treatment of decision work).
- If the POC touches committed code: the full rust suite re-run
  unchanged (`cargo test --locked`) is the verification nothing
  regressed.
- POC behavior verification is manual on the dev machine by
  construction (cursor/hover behavior is hardware-visual —
  TESTING_STRATEGY §5 territory).

## Done criteria

- [ ] Written analysis covering all three mechanism families with evidence (not docs-page claims — the pinned sources / dev-machine behavior)
- [ ] Security analysis: permission prompts, tap sensitivity, relation to the repo's capabilities lockdown posture
- [ ] Perf analysis: wake pattern vs the repo's idle-CPU history
- [ ] A clear recommendation + effort estimate, OR options presented to the operator per the STOP rule
- [ ] If a POC ran: its results recorded, and no committed code left behind unless explicitly meant to stay
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` updated with the outcome (the "No hover exists on the real window today" constraint row is the one to amend); `plans/README.md` row for 086 updated; 081/082/084's gated steps annotated with the verdict

## STOP conditions

- The investigation doesn't clearly favor one mechanism — present the
  options to the operator rather than picking one (073's exact rule).
- The front-runner requires a permission (Input Monitoring,
  Accessibility) whose prompt/privacy story can't be honestly justified
  against this repo's lockdown posture — don't wave it through; that's
  an operator decision.
- Any candidate reintroduces click-capture risk over the real menu bar
  (the 2026-07-17 bug) without a bounded, provable off-window — stop
  and say so; that bug's fix is not negotiable.
- You find yourself building hover into a render path (081/082/084's
  gated steps) as part of this spike — the spike informs, it doesn't
  ship.

## Maintenance notes

- Whatever this spike concludes rewrites a load-bearing constraint:
  `plans/frontend-ui-consolidated.html`'s "Engineering constraints"
  first row ("No hover exists on the real window today") and
  `plans/079-checklist.html`'s hover-gated items must both be amended
  with the verdict, and plans 081 (Step 5), 082 (weather-peek prep),
  084 (hover reveal) need their gated steps annotated "unblocked:
  mechanism X" or "permanently descoped."
- If the verdict is a mechanism with a permission prompt, the operator
  facing copy for that prompt (what to tell the user why notchtap wants
  it) becomes a required part of whichever build plan implements it —
  note it there when dispatching.
- The `notchtap-detect` precedent (`docs/ARCHITECTURE.md` §5) gained a
  sibling if the tap route wins — record the new subprocess contract in
  the same section at build time, mirroring how `notchtap-detect` is
  documented.
