# Plan 085: Hide-when-idle option — `resting_state: rail|notch` config flag

> **Executor instructions**: This is a small build plan for the locked
> "cheap half" of plan 079 item 17 (locked 2026-07-20, mocked at
> `prototype/notch-states.html` §6): a config flag choosing the
> overlay's RESTING state — today's time+dots idle rail (`rail`,
> default, zero behavior change) or the collapsed bare-notch state
> (`notch`, render nothing while idle). Promotions, rotation, expand,
> TTL, and every other lifecycle behavior are UNCHANGED — this is a
> render choice on the idle branch only, no interaction-model work.
> Explicitly OUT of scope: hover-reveal when nothing is showing — that's
> the expensive half, gated on plan 086 like all hover work. Follow the
> steps in order. When done, update the status row for this plan in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 3de785a..HEAD -- src-tauri/src/config.rs src-tauri/src/settings.rs src-tauri/src/lib.rs src/App.tsx src/useSlotState.ts src/components/StatusRailCard.tsx src/settings/SettingsApp.tsx prototype/notch-states.html`
> Expect `useSlotState.ts` possibly to differ from 3de785a by 081/083's
> diff, `src-tauri/src/lib.rs` possibly by 081's test-literal update,
> and `StatusRailCard.tsx` possibly by 082/084's diffs if those landed
> first (085 is independent of 081-084 but shares the file) — anything
> MORE is drift to reconcile. Any diff in the
> prototype is a STOP condition. Baseline `3de785a` already INCLUDES
> plan 080 (merged 2026-07-21 as `d21d689`) — the StatusRailCard.tsx
> refs below are post-080. (Baseline history: `71e54a7` → `4fb3af9` for
> 063's merge — App.tsx refs at :30-53, lib.rs added to the path list —
> then → `3de785a` for 080's merge. All refreshed and re-verified by
> direct read.)

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW — a config enum + one render branch; default preserves
  shipped behavior exactly
- **Depends on**: 080 (lands after the news card so the idle-branch
  edit doesn't rebase against it). Can land any time after that —
  independent of 081-084 and of 086.
- **Category**: direction (locked 2026-07-20 — "Hide-when-idle option
  (079 item 17, cheap half)", `plans/frontend-ui-consolidated.html`
  Locked decisions) → build
- **Planned at**: commit `71e54a7`, 2026-07-20. **Review-plan pass 2
  (2026-07-21, against `4fb3af9`)**: citations re-verified
  (StatusRailCard.tsx :56-67/:101-103, useStatusState.ts :106-116,
  config.rs :65/:205-245/:373-395/:620-635, useSlotState.ts :51-60,
  settings.rs :533/:549-556, SettingsApp.tsx :634, notch-states.html
  :202-206/:424/:442-448) — three stale/wrong refs fixed (App.tsx seed
  read `:20-23` → `:30-34` and listener `:27-42` → `:36-53`, shifted by
  063's `presentationFacts` effect; `validate()` `:76-108` → `:49+` —
  the cited range was mid-function; `set_appearance` `:791` → `:792`).
  One scope gap closed: `src-tauri/src/lib.rs` (the
  `__NOTCHTAP_APPEARANCE__` seed block) added to in-scope and the
  drift paths — without the seed the flag never reaches a fresh boot
  on the relaunch apply path. Drift baseline re-stamped to `4fb3af9`,
  then again to `3de785a` when plan 080 merged mid-review (the idle
  branch moved to `StatusRailCard.tsx:104-106`).

## Why this matters

Locked 2026-07-20 (`prototype/notch-states.html` §6, lines 423-449):
some users want the screen back — the resting state becomes the bare
native notch (indistinguishable from hardware) instead of the
time+dots rail, while promotions/event expansions/rotate-out all
behave identically. The prototype's own engineering note (:442-448)
scopes it precisely: "this is the cheap half of plan 079 item 17. It
needs no hover detection (resting state is a render choice), only a
config flag read by the overlay. The expensive half — revealing the
rail/card on hover when nothing is showing — is the same
cursor-tracking work every other hover needs" (plan 086). So this plan
is deliberately tiny: one enum, one toggle, one render branch.

## Current state

- `src/components/StatusRailCard.tsx:104-106` — the idle branch:
  `!renderedShowing ? <IdleView status={status} /> : …`. `IdleView`
  (`src/components/IdleView.tsx`) renders the clock, the source-status
  chips (gated by `statusRailActive`, `useStatusState.ts:106-116`), and
  the day-progress timeline. The card's idle width classes are keyed at
  `StatusRailCard.tsx:56-67` (`.rail-card.idle[.status]`).
- `src-tauri/src/config.rs` — flat config fields with per-field
  `default_*` fns (e.g. `weather_enabled` :65, defaults :205-245) and a
  parse-heal/migration precedent (:373-395) for files predating new
  keys — a new field with a `#[serde(default = "…")]` fn rides that
  machinery; `settings.rs`'s `validate()` (:49+) covers range-like
  fields (an enum needs no range check — follow the nearest bool/enum
  precedent).
- Delivery channel precedent (the overlay learns appearance-ish state
  this way today): `window.__NOTCHTAP_APPEARANCE__` seed (declared
  `src/useSlotState.ts:51-60`, read at `src/App.tsx:30-34`) plus the
  `appearance-changed` event listener (`src/App.tsx:36-53`), emitted
  rust-side from `set_appearance` (`settings.rs:792`,
  `AppearanceChangedPayload` `settings.rs:533`). The seed itself is
  built in `src-tauri/src/lib.rs:445-460` from the app's `Config`
  mutex. Either this channel
  extended with `resting_state`, or the slot-state seed/event — pick
  the appearance channel (it already exists for exactly this class of
  overlay-behavior state and hot-applies without touching the
  receive-only rule: it's rust→webview emit, no invoke from the
  overlay).
- Settings UI: `GeneralSection` (`src/settings/SettingsApp.tsx:634`,
  "Control startup, the local listener, and how notifications
  rotate") — the toggle goes here per the filing decision; follow the
  section's existing `ToggleControl`/help-text patterns, and state its
  apply semantics (relaunch vs hot-apply) honestly in the help copy
  matching whatever Step 3 wires.
- Prototype (`prototype/notch-states.html` §6): resting `notch` =
  render NOTHING beyond the cutout itself (the `.hw-notch.solo` state,
  section 1, lines 200-208) — not a minimized rail, not a narrower
  card: nothing.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/config.rs` — `resting_state: RestingState` enum
  (`rail` default | `notch`), serde-serialized as the string values
  above, with its `default_*` fn and parse-heal behavior for
  pre-existing config files.
- `src-tauri/src/settings.rs` — whatever `validate()`/DTO/save-path
  touch the new field needs (likely none beyond the Config DTO
  mirroring — check how `weather_enabled`-class bools flow and match).
- Delivery to the overlay: extend the appearance channel (seed
  `__NOTCHTAP_APPEARANCE__` + `appearance-changed` payload) with
  `resting_state`, and make the settings save path emit it (verify the
  exact emit site — `set_appearance` today, or the save flow; note the
  choice and its apply semantics in the completion report).
- `src-tauri/src/lib.rs` — the `__NOTCHTAP_APPEARANCE__` seed block
  (:445-460): REQUIRED, not optional. The General-section save path
  relaunches the app, and a fresh boot learns the flag ONLY from this
  seed — extending just the event payload would boot every relaunch
  into `rail` regardless of config until the next settings save.
  Construction caveat (pinned at review-plan pass 2): the payload's
  `From<&Appearance>` impl (settings.rs:539-547) sees only the
  `Appearance` struct, but `resting_state` is a top-level `Config`
  field — construct the widened payload from `Config` at both emit
  sites (the seed already locks the full `Config` at lib.rs:447-453;
  the broadcast path at settings.rs:549-556 needs the same access)
  rather than through that `From` impl.
- `src/App.tsx` (+ the `__NOTCHTAP_APPEARANCE__` global declaration in
  `src/useSlotState.ts`) — accept + apply the new field.
- `src/components/StatusRailCard.tsx` — the idle branch renders nothing
  when `resting_state === "notch"` (and the card shell gets no idle
  width/chrome in that state — see Step 4's care point).
- `src/settings/SettingsApp.tsx` — General-section toggle.
- Tests per the Test plan.

**Out of scope**:
- Hover-reveal of the rail/card when nothing shows (plan 086 territory
  — the prototype §6's "hovering the bare notch still opens the peeks"
  sentence is NOT implemented here; the peek itself is also 086-gated).
- Any lifecycle change: promotions, rotation, expand toggle, TTL bar
  (081), supersession — all untouched.
- The idle rail's CONTENT (time+dots redesign is the future 079-shape
  plan; 063's wrap mechanics stay as shipped).
- HUD-mode vs notch-mode differences — the locked shape is
  unconditional across modes; the flag means the same thing on both.

## Steps

### Step 1: Config field

`config.rs`: `RestingState` enum (`Rail` default, `Notch`),
serde-snake_case (`"rail"`/`"notch"`), `pub resting_state: RestingState`
on `Config` with `#[serde(default = "default_resting_state")]`, the
default fn, `Config::default()` entry, and a parse test: a config file
with no `resting_state` key parses to `Rail` (heal-by-default, same
shape as the file's existing missing-key tests, e.g. :626-631's
pattern); `resting_state = "notch"` round-trips.

**Verify**: `cd src-tauri && cargo test --locked` → all pass;
`cargo clippy --locked --all-targets -- -D warnings` → exit 0;
`cargo fmt --check` → exit 0.

### Step 2: Delivery channel

Extend `AppearanceChangedPayload` (`settings.rs:533`) and the
`__NOTCHTAP_APPEARANCE__` seed construction with `resting_state`, and
wire the emit so saving the new toggle reaches the overlay (preferred:
the save path emits `appearance-changed` with the full payload, making
the toggle hot-apply like scale/radius/opacity; if the General
section's save flow is relaunch-only today, relaunch-applied is
acceptable — but then say so in the toggle's help text and the
completion report). Keep the payload backward-tolerant: an old seed
without the field means `rail` (frontend defaults, Step 3).

**Verify**: `cd src-tauri && cargo test --locked` → all pass (update/
extend the `set_appearance`-adjacent tests for the widened payload).

### Step 3: Overlay render branch

`src/useSlotState.ts`: extend the `__NOTCHTAP_APPEARANCE__` global
type with `resting_state?: "rail" | "notch"`. `src/App.tsx`: hold it
alongside the other appearance vars (state or a ref — match how the
file treats scale/radius/opacity) and pass down. `StatusRailCard.tsx`:
when idle AND `resting_state === "notch"`, render nothing — the
cleanest honest shape is the card shell itself not mounting its idle
content AND carrying no `.rail-card.idle` box (a rendered-but-empty
black rounded box over the bare notch is exactly the wrong look; the
prototype's `notch` state has zero app-drawn pixels). Watch the shell:
`.rail-card` carries background/shadow/priority accents — in the
notch-resting state none of that may paint. Simplest correct shape:
early-return `null` (or a zero-size fragment) for the idle+notch
  combination, keeping every `showing` path byte-identical. The
  appearance change itself (rail↔notch toggle while running) must
  re-render without a reload — it's React state driven by the App.tsx
  `appearance-changed` listener extended in Step 2.

**Verify**: `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0.

### Step 4: Settings toggle

`SettingsApp.tsx` `GeneralSection`: a `ToggleControl` — e.g. name "Hide
overlay when idle", help "Resting state shows the bare notch instead of
the clock and status dots. Notifications, rotation, and shortcuts are
unaffected." — mapping checked=`resting_state === "notch"` onto
`patchConfig`. Place it near the startup/overlay-behavior controls,
following the section's existing copy tone.

**Verify**: `npx vitest run` → all pass (settings-form tests for the
new toggle's render + patch payload); `npx tsc --noEmit` → exit 0;
`npx biome ci .` → exit 0.

### Step 5: Full gate

**Verify**: every command in the Commands table exits 0.

## Test plan

- **Cargo**: config parse/heal tests from Step 1; the widened
  appearance payload serialization from Step 2.
- **Vitest**: idle + `resting_state: "notch"` → no idle content
  rendered and no `.rail-card.idle` box (assert the shell is
  absent/zero-content, not just empty-looking); idle + `"rail"` (and +
  field-absent) → today's IdleView renders unchanged (regression pin);
  `showing` + `"notch"` → card renders normally (promotions unaffected
  — the key acceptance test); settings-form test for the toggle's
  patch payload.
- **Manual-only** (operator, TESTING_STRATEGY §5): on the real overlay —
  toggle on, idle shows bare notch; a `/notify` push still promotes,
  rotates, expands via ⌃⇧N; toggle off restores the rail; both modes
  (MacBook notch + mac mini HUD) if both machines are reachable.

## Done criteria

- [ ] `resting_state` enum in config with `rail` default; pre-existing config files parse unchanged (pinned by test)
- [ ] Overlay learns the flag via the appearance channel (seed + event); missing field = `rail`
- [ ] Idle + `notch` renders nothing (zero app-drawn pixels, no shell chrome); all `showing` paths byte-identical
- [ ] General-section toggle with honest apply-semantics copy
- [ ] `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` all exit 0
- [ ] No lifecycle diff: rotation/supersede/expand behavior untouched (`git diff` shows no `queue.rs`/`engine.rs`/`poller.rs` changes)
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated (hide-when-idle cheap half → shipped; hover-reveal half stays 086-gated); `plans/README.md` row for 085 updated

## STOP conditions

- **Prototype drift**: `prototype/notch-states.html` §6 differs from
  this plan's description — stop and re-confirm the approved resting
  state.
- You find yourself writing hover-detection, cursor tracking, or
  peek-reveal code — that's 086-gated; stop.
- The appearance channel turns out NOT to be extensible without
  touching the receive-only/capability posture (it is emit-only
  rust→webview, so this shouldn't happen — if it does, stop and use
  the slot-state seed route instead, noting why).
- Hiding the idle card regresses the swap/animation machinery (plan
  078's `useDelayedSwap` assumes an idle branch exists — if a null
  idle branch breaks its keying, stop and re-shape rather than
  special-casing the hook).

## Maintenance notes

- Update `plans/079-checklist.html` and
  `plans/frontend-ui-consolidated.html`: the "Hide-when-idle option"
  locked-decision entry → shipped (cheap half); the "Hide-when-idle /
  reveal-on-hover" open-question entry keeps its expensive half open,
  still gated on 086.
- This is the first overlay-behavior (not just cosmetic) field on the
  appearance channel — if a second one ever appears (e.g. a future
  pause-indicator toggle, 079 item 11), this plan's wiring is the
  template.
- The future 079 3-block shape plan changes what "idle" renders
  entirely (time+dots flanks); the `notch` branch's "render nothing"
  semantics survive it unchanged — call this out in that plan's
  Current-state section when it's written.
