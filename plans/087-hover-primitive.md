# Plan 087: Hover primitive — tracking area + rust-derived card rect + `hover-changed` event

> **Executor instructions**: This is a build plan for the ONE shared
> hover primitive that plan 086's spike chose. It ships **no user-visible
> hover feature** — it lands the mechanism (a tracking area on the
> existing overlay panel, a pure rect-derivation function, and a
> `hover-changed` event reaching the webview) and one CSS state class
> proving the signal arrives. The four features that consume it
> (081's TTL hover-pause, 082's weather peek, 084's rail→scorecard
> reveal, and the locked idle expanded-on-hover state) are each their
> own follow-on work — building any of them here is a STOP condition.
> `set_ignore_cursor_events(true)` must remain unconditional at both
> call sites; that is the whole point of the chosen mechanism. When
> done, update the status row for this plan in `plans/README.md` —
> unless a reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 647f6d0..HEAD -- src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/capabilities/default.json src-tauri/tauri.conf.json src/styles.css src/App.tsx src/components/StatusRailCard.tsx docs/design/hover-cursor-tracking.md`
> Baseline `647f6d0` is master with plans 080–086 ALL merged (084's
> scorecard CSS included — it appended +348 lines to `styles.css` at
> :862+, which is why the earlier `d42603a` baseline was re-stamped a
> second time), so this should be **empty or near-empty**. If you see a
> large diff, you are not on `647f6d0` — fix that first rather than
> trying to reconcile.
> **A diff in `capabilities/default.json` is a STOP condition** (see
> below). A diff in `docs/design/hover-cursor-tracking.md` means the
> spike was revised — re-read it before starting.
>
> **All line numbers below were re-verified against `647f6d0` by a
> second cold-read review on 2026-07-21** (pass 1 verified them at
> `d42603a`; pass 2 re-checked after 084's merge — `lib.rs`, `App.tsx`,
> and `styles.css:25-61` are all unshifted, because 084 only appended
> CSS). They are correct as written. Re-grep anyway if anything looks
> off — content matters more than line numbers.

## Status

- **Priority**: P2 (unblocks four separately-planned hover features;
  none of them can start until this lands)
- **Effort**: M — one pure rust function + its tests, one macro block,
  one event emission, one small frontend listener. The spike estimated
  S/M; this plan carries M because the rect table has more inputs than
  the spike's sketch accounted for (see the `--card-scale` note below).
- **Risk**: MED — touches `lib.rs`'s native window configuration, the
  one place a mistake resurrects the 2026-07-17 menu-bar-click-
  swallowing bug. The chosen mechanism is specifically designed so it
  *cannot* (click-through is never disabled), but the reviewer will
  check that line first.
- **Depends on**: 086 (DONE — `docs/design/hover-cursor-tracking.md`).
  The former soft-dependency ("land after 081/085 merge") is satisfied —
  080–086 are all in master at the `647f6d0` baseline. **Gates**: the
  hover-halves of 081, 082, 084, and the locked idle expanded-on-hover
  state.
- **Category**: tech-debt / architecture (spike → build)
- **Planned at**: commit `3de785a`, 2026-07-21. Written by the advisor
  directly from plan 086's reviewed spike output, at operator request.
  **Review-plan pass (2026-07-21, against `d42603a`)**: cold-read by a
  fresh-context agent. `tauri-nspanel`'s tracking-area support CONFIRMED
  in the vendored checkout at the pinned rev. All `lib.rs` citations were
  stale by ~13-14 lines (083's merge) and are refreshed. Three real gaps
  closed — see the cold-read section below. Drift baseline re-stamped
  `3de785a` → `d42603a`.
  **Review-plan pass 2 (2026-07-21, against `647f6d0`)**: second
  cold-read after 084's merge. All prior citations re-verified and still
  exact (084 appended CSS at `styles.css:862+`; nothing cited shifted);
  the vendored `tauri-nspanel` claims re-confirmed against the
  `a3122e8` checkout. Two REAL errors found and fixed: (1) the Step 2
  width table applied the notch clamp to ALL notch-mode states, but the
  only notch-scoped rule in `styles.css` is
  `:root[data-notchtap-mode="notch"] .rail-card.idle.status` — notch
  visible/expanded stay 400/500 and notch plain-idle stays 270; (2) the
  plan was silent on plan 085's `resting_state = "notch"`, under which
  the idle card renders NOTHING (`src/components/StatusRailCard.tsx:252`
  returns `null`) — the rect decision for that case is now pinned in
  Step 2 and guarded by a STOP condition. Drift baseline re-stamped
  `d42603a` → `647f6d0`. Step 3's stale `lib.rs:42-49` corrected to
  `:44-51`.

## Why this matters

Four separately-locked interaction designs — TTL hover-pause (081),
weather peek (082), rail→scorecard reveal (084), and idle
expanded-on-hover — are each blocked on the same missing primitive, and
plan 086's spike recommended building it **once** rather than four
times. The spike's decisive finding (`docs/design/hover-cursor-tracking.md`
§2, empirically verified with a throwaway Swift POC run twice at
production's exact `NSStatusWindowLevel`):

> with `ignoresMouseEvents = true`, the tracking area's
> `mouseEntered`/`mouseMoved`/`mouseExited` fire *normally* — but a real
> click at the identical location is **not** captured. macOS gates these
> two behaviors through independent mechanisms.

So hover is obtainable with **zero change** to `set_ignore_cursor_events
(true)`, no global cursor watcher, no CGEvent tap, no Input Monitoring
or Accessibility permission prompt, and no change to
`capabilities/default.json`. That combination is why this mechanism was
chosen over the two alternatives.

**Known risk, carried forward from the spike's review deliberately**:
the AppKit behavior above is *empirically observed, not contractually
documented by Apple*. The spike's "not undocumented surface" argument
covers `tauri-nspanel`'s API (shipped, two worked examples), not the
underlying platform behavior. The operator accepted this risk on
2026-07-21. The failure mode if a future macOS changes it is benign —
hover silently stops firing; the menu-bar bug cannot recur, because the
click-through call is never touched. **Do not "improve" this by
disabling click-through when hover seems not to work** — that trade was
explicitly rejected; see the STOP conditions.

## Current state

- `src-tauri/src/lib.rs:44-51` — the `tauri_nspanel::tauri_panel!`
  macro invocation declaring `OverlayPanel`, today carrying only a
  `config:` block (`can_become_key_window: true`,
  `can_become_main_window: false`). This is where the `with: {
  tracking_area: {...} }` block goes.
- `src-tauri/src/lib.rs:558-594` — `apply_overlay_native_config`, whose
  `window.set_ignore_cursor_events(true)` at :568 **stays exactly as
  it is**. Its comment (:560-567) records the 2026-07-17 bug it fixed.
  Called at two sites: `lib.rs:271` (initial setup) and `lib.rs:497`
  (re-apply). Neither changes.
- `src-tauri/Cargo.toml:40` — `tauri-nspanel` pinned at rev
  `a3122e894383aa068ec5365a42994e3ac94ba1b6` (`a3122e8`). The pinned rev
  already ships tracking-area support: `TrackingAreaOptions` builder at
  `src/builder.rs:244-330`, the macro-generated `add_tracking_area`
  helper at `src/panel.rs:659-696`, and the
  `on_mouse_entered`/`on_mouse_exited`/`on_mouse_moved`/
  `on_cursor_update` closures at `src/panel.rs:24-30` and `:72-79`.
  Two worked examples exist in that checkout:
  `examples/mouse_tracking/src-tauri/src/main.rs` and
  `examples/hover_activate/src-tauri/src/main.rs` — read both before
  writing the macro block.
- `src-tauri/capabilities/default.json` — exactly
  `core:event:allow-listen`/`allow-unlisten` for window `main`.
  **This file must not change.** A rust→webview `emit` rides the
  already-granted blanket listen permission (tauri v2 does not scope
  capabilities per event name), which is precisely why the spike chose
  an event over an invoke command.
- `src-tauri/tauri.conf.json` — the overlay window is a fixed
  **500×300**, `"resizable": false`. The window *frame* never changes
  size; only the CSS width *within* it changes. This is why
  `auto_resize` on the tracking area is irrelevant and why a rect
  derivation is needed at all.
- `src/styles.css:25-61` — the card width breakpoints (cold-read-verified
  UNSHIFTED twice, despite 081/082/084 all editing this file elsewhere —
  084 only appended, at :862+) the rect table must mirror:
  - `.rail-card` base — `calc(400px * var(--card-scale))` (:25)
  - `.rail-card.expanded` — `calc(500px * var(--card-scale))` (:39)
  - `.rail-card.idle` — `calc(270px * var(--card-scale))` (:44)
  - `.rail-card.idle.status` — `calc(460px * var(--card-scale))` (:52)
  - `:root[data-notchtap-mode="notch"] .rail-card.idle.status` — `calc(clamp(270px, var(--notchtap-cutout-width, 270px), 460px) * var(--card-scale))` (:60-61)

  **The notch clamp is scoped to `.idle.status` ONLY** — it is the sole
  `data-notchtap-mode` rule in the file (grep to confirm). In notch mode
  a visible card is still 400/500 and a plain (chipless) idle card is
  still 270. Plan 063 capped only the idle status rail. The Step 2 width
  table reflects this — do not "simplify" it back to a blanket
  notch-mode clamp.
- `src/components/StatusRailCard.tsx:244-253` — plan 085's
  `resting_state` escape hatch. When the config's `resting_state` is
  `"notch"` and the card has fully settled into idle, the component
  renders **nothing at all**:

  ```ts
  // plan 085: idle + resting_state "notch" → zero app-drawn pixels, ...
  if (!renderedShowing && !exiting && restingState === "notch") {
    return null;
  }
  ```

  So "idle" does not always mean "an idle rail is on screen." The rect
  decision for this case is pinned in Step 2 — read it; it is one of
  the two errors the second cold-read found.
- `src-tauri/src/lib.rs:611-616` — `cutout_width_js_value` (call site :484): rust is
  ALREADY the geometry authority for the cutout width and already
  pushes it to JS. The rect derivation reads the same input from the
  same source, in the same direction — no new IPC.
- Delivery precedent: `webview.emit("appearance-changed", &payload)`
  (`lib.rs:473`, `settings.rs:564`) with a matching `listen` in
  `src/App.tsx:47-55`, inside the `useEffect` at :39-70. `hover-changed` mirrors this shape exactly.
- `docs/design/hover-cursor-tracking.md` — the spike. §2 is the
  empirical result, §6 is the rect-derivation decision and the
  **rejected** alternative, §7 the recommendation, §8 the test strategy.
  Read §6 and §7 in full before Step 2.

## The `--card-scale` gap (pinned by the advisor, NOT in the spike)

The spike's §6 lists the breakpoints as "270/400/460/500, the notch-mode
clamp" — but **every one of those is multiplied by `var(--card-scale)`**
in `styles.css`, and `--card-scale` is user-configurable through the
Settings Appearance section (`applyAppearance` in `src/App.tsx` sets it
from the appearance payload). A rect table that ignores the scale will
be wrong for every user who is not at scale 1.0 — silently, and more
wrongly the further from 1.0 they go.

Rust already owns this value: it is the `scale` field on the same
`Appearance`/`Config` the `appearance-changed` payload is built from.
**The derivation function must take scale as an input and multiply.**
Say so explicitly in the completion report, and pin it with its own
test case (scale 0.8 and 1.25, not just 1.0).

## ⚠️ Cold-read review findings (2026-07-21) — the three gaps this plan had

A fresh-context agent reviewed this plan against live code and found
three places where it assumed knowledge an executor won't have. All
three are closed below. **Read this section before Step 2.**

It also CONFIRMED the plan's load-bearing claim by locating the vendored
source at `~/.cargo/git/checkouts/tauri-nspanel-*/a3122e8/`:
`TrackingAreaOptions` at `builder.rs:244`, `add_tracking_area` at
`panel.rs:659-696` (file is exactly 696 lines), and both example
programs, using the exact macro shape Step 3 sketches. The mechanism is
verified, not merely plausible.

### Gap 1 (BIGGEST RISK) — `has_status_chips` is NOT rust-owned today

This plan frames all six `active_card_rect` inputs as "state rust
already owns." **That is true for five of them and false for
`has_status_chips`.** The predicate it mirrors lives ONLY in TypeScript:

```ts
// src/useStatusState.ts:106 — statusRailActive()
status.football.enabled ||
status.news.enabled ||
status.football.live !== null ||
status.weather.enabled ||
status.weather.current !== null ||
status.waiting > 0 ||
status.paused
```

Rust has the *data* — `StatusState` at `src-tauri/src/status.rs:21-27`
(`paused`, `waiting`, `football`, `news`, `weather`) — but **no
equivalent predicate function.** You must port it.

Port it as a small pure function next to `active_card_rect` (e.g.
`fn status_rail_active(s: &StatusState) -> bool`), mirroring the seven
terms above **exactly** — the two easiest to miss are `waiting > 0` and
`paused`, which have nothing to do with sources being enabled. Give it
its own table-driven test with one case per term (each term alone → true;
all false → false). Add a comment on BOTH copies naming the other as its
mirror — this is now a duplicated predicate across the language seam,
same class of hazard as the rect table vs `styles.css`.

### Gap 2 — how state reaches the `panel_event!` closure

Step 3 says "built from current state" without saying how the AppKit
callback reaches the Engine/Config/StatusState. The upstream examples
capture nothing (they just print), so they don't answer it. **This repo
already has the pattern**, in the same file:

```rust
let engine = app_handle.state::<Engine>().inner().clone();          // lib.rs:423
let config = app_handle.state::<StdMutex<Config>>().lock().unwrap().clone();  // lib.rs:466
```

Clone an `AppHandle` into the closure (`let handle = app_handle.to_owned()`,
the upstream `mouse_tracking` example's own pattern) and pull state
through it per event. **Do not hold a lock across the callback** — read,
clone what you need, drop, then compute. The rect function is pure and
cheap; the lock scope should be one or two lines.

### Gap 3 — the coordinate-space flip, which nothing tests

`locationInWindow` is **bottom-left origin, y grows up** (AppKit). The
CSS card is laid out **top-down** in a window pinned flush to the
physical screen top. Get the flip wrong and the rect never matches the
cursor — and per this plan's own Test plan, the tracking-area handler
has **no automated coverage**, so a y-axis bug passes every rust,
vitest, clippy and build gate and surfaces only at the manual smoke
step, which can pass by accident on a large card.

Requirements, now mandatory:
1. Convert explicitly, in one named helper, with the formula in a
   comment: for a window of height `H`, a top-down rect at CSS y-offset
   `t` with height `h` occupies AppKit y from `H - t - h` to `H - t`.
   `H` is 300 (`tauri.conf.json`, fixed, non-resizable).
2. **Unit-test the conversion itself** — it is pure arithmetic, so there
   is no excuse for leaving it untested. At minimum: a rect at the very
   top of the window converts to the TOP in AppKit coords (high y, not
   low), and a point just inside vs just outside each edge classifies
   correctly. This is the test that catches an inverted axis; the
   existing "table of widths" tests cannot, because they only check the
   function against constants its own author chose.
3. In the manual smoke step, hover **near the card's top edge and again
   near its bottom edge** specifically. An inverted rect often still
   overlaps in the middle — the edges are where it shows.

### Gap 4 (minor) — `docs/ARCHITECTURE.md` has no obviously-right section

The done criterion says "one line where the window/native-config
behavior is already described." The cold read checked: **no such
description exists.** §6 ("always-on background behaviour") is closest
but never mentions `set_ignore_cursor_events`, `NSStatusWindowLevel`, or
the 2026-07-17 bug — that narrative lives only in `lib.rs` comments and
the spike doc. So: add a short new subsection under §6 rather than
hunting for a home, and say in your report where you put it.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |
| Frontend build | `npx vite build` | exit 0 |

The counts stamped at planning time (`3de785a`: rust 345 + 3 doc-tests,
frontend 124) predate the 081/082/083/084/085 merges and are certainly
low now — **recount from a clean run before you start, don't trust any
number written here**, and read the current figures from
`docs/TESTING_STRATEGY.md` §0 (which plan 085 corrected to be accurate
and 083/084 have since updated).

## Scope

**In scope**:
- `src-tauri/src/lib.rs` — the `with: { tracking_area: {...} }` block on
  the `OverlayPanel` macro; a `panel_event!` handler wiring
  `on_mouse_entered`/`on_mouse_exited`/`on_mouse_moved`; the
  `hover-changed` emission.
- A new pure function (suggest `src-tauri/src/hover.rs`, or a
  `lib.rs` submodule — match repo layout judgment; a separate module is
  preferred, mirroring how `presentation_mode`'s pure decision logic is
  kept apart from its subprocess call): `(mode, cutout_width, scale,
  visible, expanded, has_status_chips) -> Rect`, plus a
  `point_in_rect`-style helper. **No AppKit types in this function's
  signature** — plain numbers in, plain rect out, so it is unit-testable
  without a GUI (`docs/TESTING_STRATEGY.md` §4.4's stated pattern, the
  same discipline `presentation_mode` follows).
- `src/App.tsx` — a `hover-changed` listener setting one boolean state,
  applied as a CSS class on the card root (e.g. `.rail-card.hovered`).
  Follow the existing `appearance-changed` listener's shape exactly,
  including its `unmounted` guard and `.catch`.
- `src/styles.css` + `src/settings/preview-overlay.css` — ONE minimal,
  deliberately boring rule for `.hovered` (the spike's suggestion: a
  barely-perceptible brightness or border lift) proving the signal
  arrives end-to-end. Mirrored same commit per the mirror law. This is
  a diagnostic, not a design — 082/084 will replace it.
- Tests per the Test plan.

**Out of scope**:
- **Every actual hover feature**: TTL hover-pause (081 Step 5), weather
  peek (082), rail→scorecard reveal (084), idle expanded-on-hover. This
  plan lands the signal; they consume it.
- Any lifecycle/deadline pausing — that is queue-engine work and its
  own plan (081's Step 5 note is explicit that it must not be squeezed
  in).
- Changing `set_ignore_cursor_events`, either call site, or
  `capabilities/default.json`.
- The rejected `report_card_bounds` invoke command (spike §6) — do not
  build it, do not "just try it."
- Pixel-perfect rect precision. The spike explicitly recommends a
  CONSERVATIVE rect (slightly wider than the rendered card is fine and
  expected).

## Steps

### Step 1: Read the spike and the two upstream examples

Read `docs/design/hover-cursor-tracking.md` §§2, 6, 7, 8 in full. Then
locate the pinned `tauri-nspanel` checkout (cargo's git checkout dir,
rev `a3122e8`) and read `examples/mouse_tracking/src-tauri/src/main.rs`
and `examples/hover_activate/src-tauri/src/main.rs`, plus
`src/builder.rs:244-330` (`TrackingAreaOptions`) and
`src/panel.rs:659-696` (`add_tracking_area`).

**STOP** if the pinned rev does NOT expose tracking-area support as the
spike describes — that would mean the spike's central mechanism claim is
wrong about the actual dependency, and this plan's premise fails.

**Verify**: no command — report which files you read and confirm the
`TrackingAreaOptions` builder and the `on_mouse_*` closures exist at the
cited locations.

### Step 2: The pure rect-derivation function + its tests (TDD this one)

Write the function BEFORE the macro wiring — it is the only genuinely
new logic and it is fully testable without a window.

Signature (adapt names to repo style):

```rust
/// The screen-space rect, in window coordinates, currently covered by
/// the rendered card — the region where hover should count. Deliberately
/// CONSERVATIVE: it may be slightly wider than the true rendered edge
/// (the spike's §6 decision), never narrower.
///
/// Mirrors the width breakpoints in `src/styles.css:25-61`. Any change
/// to a card width there MUST change the table here — see the
/// duplicated-constants note in this function's tests.
pub fn active_card_rect(
    mode: Mode,
    cutout_width: f64,
    scale: f64,
    visible: bool,
    expanded: bool,
    has_status_chips: bool,
) -> Rect
```

Width selection, mirroring `styles.css` (the notch clamp applies ONLY
to the idle-with-chips state — see the Current state note; `mode` is
irrelevant to every other branch):
- `visible && expanded` (either mode) → `500.0`
- `visible` (either mode) → `400.0`
- idle + status chips, notch mode → `clamp(270.0, cutout_width, 460.0)`
- idle + status chips, hud mode → `460.0`
- idle, no chips (either mode) → `270.0`

**The `resting_state = "notch"` case (pinned by review pass 2 — the
plan was previously silent on it)**: under plan 085's flag, a
fully-idle card renders zero pixels
(`src/components/StatusRailCard.tsx:252` returns `null`). The decision:
**`resting_state` is deliberately NOT an input to `active_card_rect`,
and the idle branches above apply unchanged** — the rect targets the
card's *would-be* footprint. Rationale: the gated "idle
expanded-on-hover" consumer (079 item 17's full vision) exists
precisely to summon the rail by hovering the bare cutout/rest region,
so the primitive must keep reporting hover there even while nothing is
drawn; and the cost of a `hover-changed` while nothing renders is nil
(the `.hovered` class lands on a component that returned `null`). This
also keeps the function's inputs exactly the six listed. Record this
decision in the function's doc comment, citing
`StatusRailCard.tsx:244-253`. If, while wiring, you find this makes the
diagnostic smoke test impossible to pass (nothing visible to observe),
verify via the DOM-inspector/event log instead — do NOT add a
`resting_state` special case on your own; that changes what the four
consumers inherit and is the operator's call (STOP condition).

**Then multiply the chosen width by `scale`** (see the `--card-scale`
gap section above — this is the step most likely to be forgotten).
The card is horizontally centred in the fixed 500×300 window, so the
rect's x-origin is `(window_width - card_width) / 2.0`; height and
y-origin follow the card's rendered box — derive them the same
conservative way and document the choice.

Tests (table-driven, the `presentation_mode` test style):
- every branch above at scale 1.0
- **the same branches at scale 0.8 and 1.25** (the pinned gap)
- notch-mode idle+chips clamp at both bounds (cutout 200 → 270;
  cutout 600 → 460)
- **notch mode, `visible` → 400 regardless of cutout width** — the
  tripwire for the blanket-clamp error review pass 2 removed from this
  very table; the clamp is idle+chips-only
- a `point_in_rect` case just inside and just outside each edge
- one test asserting the table's raw widths equal named constants, with
  a comment naming `src/styles.css:25-61` as the other half of the
  duplicated-constants pair (per spike §6 — a named-constant assertion,
  NOT a live CSS parse)

**Verify**: `cd src-tauri && cargo test --locked` → all pass (new tests
included); `cargo clippy --locked --all-targets -- -D warnings` → exit 0;
`cargo fmt --check` → exit 0.

### Step 3: Tracking area + `hover-changed` emission

Extend the `OverlayPanel` macro (`lib.rs:44-51`) with the tracking-area
block, following the upstream examples:

```rust
panel!(OverlayPanel {
    config: { can_become_key_window: true, can_become_main_window: false },
    with: {
        tracking_area: {
            options: TrackingAreaOptions::new()
                .active_always()
                .mouse_entered_and_exited()
                .mouse_moved(),
            auto_resize: true
        }
    }
})
```

(`active_always` is required — the app is a non-activating accessory
panel and must track while unfocused.) Wire a `panel_event!` handler:
on `mouse_entered`/`mouse_moved`, compare the event's
`locationInWindow` against `active_card_rect(...)` built from current
state; on `mouse_exited`, treat as not-hovered. **Emit only on
transitions** — `hover-changed` fires when the boolean flips, never
per mouse-move — otherwise a moving cursor floods the webview with
events and violates the idle-cost discipline plans 015/018 established.
Payload: `{ "hovered": bool }`, emitted via `webview.emit`, same shape
as `appearance-changed`.

`set_ignore_cursor_events(true)` at `lib.rs:568` is NOT touched. Both
call sites (`:271`, `:497`) are NOT touched.

**Verify**: `cd src-tauri && cargo test --locked` → all pass; clippy →
exit 0; `cargo fmt --check` → exit 0; and confirm by grep that
`set_ignore_cursor_events` still appears exactly once, still
unconditional, still `true`.

### Step 4: Frontend listener + the one diagnostic CSS rule

`src/App.tsx`: a `hover-changed` listener holding one boolean state,
mirroring the `appearance-changed` listener's exact shape (including the
`unmounted` guard and `.catch` — see `App.tsx:47-55`). Pass it into
`StatusRailCard` (`src/components/StatusRailCard.tsx`) as a prop
(optional, defaulting to `false`, so every
existing caller and test is unaffected — plan 085's `restingState` prop
is the precedent to copy), applied as a `.hovered` class on the card
root. Add ONE minimal rule for `.rail-card.hovered` in `src/styles.css`
and mirror it in `src/settings/preview-overlay.css` in the same commit.

**Verify**: `npx vitest run` → all pass (new test: `hovered` prop toggles
the class; absent prop → no class, byte-identical to today);
`npx tsc --noEmit` → exit 0; `npx biome ci .` → exit 0; mirror grep —
`grep -c 'hovered' src/settings/preview-overlay.css` ≥ 1.

### Step 5: Full gate

**Verify**: every command in the Commands table exits 0.
`docs/TESTING_STRATEGY.md` §0 updated for the tests added (recount from
the CURRENT figures, per the Commands-table note).

## Test plan

- **Rust (cargo)**: the Step 2 table-driven suite for
  `active_card_rect` — every mode/state branch, three scales, both
  notch-clamp bounds, `point_in_rect` edges, and the named-constant
  assertion. This is the bulk of the testing and it needs no GUI.
- **Rust**: no test for the tracking-area wiring itself — it is AppKit
  callback plumbing, manual-only by `docs/TESTING_STRATEGY.md` §5, the
  same treatment `apply_overlay_native_config` already gets.
- **Frontend (vitest)**: the `hovered` prop toggles `.hovered`; absent
  prop renders byte-identically to today (regression pin). Do NOT try to
  simulate a real tracking-area event in jsdom.
- **Manual-only** (operator, TESTING_STRATEGY §5 — the acceptance that
  actually matters here):
  1. Move the cursor over a live card; confirm `.hovered` appears in the
     DOM inspector and disappears on exit.
  2. **Click a menu-bar icon that the overlay window's dead margin
     overlaps, and confirm it still responds.** This is the
     2026-07-17 regression check and it is the single most important
     manual step in this plan.
  3. Confirm hover still fires while another app is focused (the
     `active_always` requirement).
  4. Both presentation modes if both machines are reachable — the mac
     mini is HUD-only; notch-mode geometry is unverifiable there.
  5. With `resting_state = "notch"` set and the card idle (nothing
     rendered), confirm `hover-changed` still fires over the would-be
     footprint — observe via the event log / DOM inspector, since
     there are no pixels to watch (the Step 2 pinned decision).

## Done criteria

- [ ] `active_card_rect` (or equivalent) is a pure function with no AppKit types in its signature, unit-tested across every mode/state branch AND at ≥3 distinct `--card-scale` values, including the notch-mode `visible → 400` (unclamped) tripwire case
- [ ] The `resting_state = "notch"` decision (idle rect = would-be footprint, `resting_state` NOT an input) is recorded in the function's doc comment citing `src/components/StatusRailCard.tsx:244-253`
- [ ] `status_rail_active` ported to rust from `src/useStatusState.ts:106`, all seven terms, table-tested one case per term, with mirror comments on both copies (cold-read Gap 1)
- [ ] The AppKit y-flip lives in ONE named helper and has its own unit test proving top-of-window maps to HIGH AppKit y, plus inside/outside cases on each edge (cold-read Gap 3)
- [ ] Tracking area attached via the pinned `tauri-nspanel`'s own support; `hover-changed` emitted on TRANSITIONS only, never per mouse-move
- [ ] `src-tauri/src/lib.rs`'s `set_ignore_cursor_events(true)` is byte-identical to before this plan, at both call sites (`git diff` proves it)
- [ ] `src-tauri/capabilities/default.json` byte-identical (`git diff --exit-code` on that path)
- [ ] No new `#[tauri::command]` anywhere; `src-tauri/build.rs`'s command list unchanged
- [ ] Frontend `.hovered` class toggles from the real event; absent-prop path byte-identical (pinned by test)
- [ ] The diagnostic CSS rule mirrored in `src/settings/preview-overlay.css`, same commit
- [ ] `cargo test --locked`, clippy `-D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` all exit 0
- [ ] `docs/TESTING_STRATEGY.md` §0 updated from the CURRENT (recounted) baseline
- [ ] `docs/ARCHITECTURE.md` — one line recording the hover mechanism where the window/native-config behavior is already described (the spike's maintenance note asks for this)
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` updated: the hover constraint row moves from "mechanism chosen, primitive unbuilt" to "primitive shipped; the four consumers still unwired"

## STOP conditions

- **The pinned `tauri-nspanel` rev does not expose tracking-area
  support as spike §6 describes** — the plan's premise fails; stop and
  report rather than bumping the pin (a dependency bump is its own
  plan, with its own review).
- **You are about to change `set_ignore_cursor_events`, either of its
  call sites, or `capabilities/default.json`.** All three are hard
  boundaries. If hover appears not to fire and disabling click-through
  "would fix it," that is the 2026-07-17 bug waiting to happen and the
  exact trade the spike rejected — stop and report what you observed.
- **You are about to add a `#[tauri::command]`** (e.g. the rejected
  `report_card_bounds`) — spike §6 flagged that as a security-posture
  change reserved for the operator, not an engineering convenience.
- The tracking area fires but `locationInWindow` cannot be reconciled
  with the CSS-rendered card position (coordinate-space mismatch you
  cannot resolve conservatively) — stop and present what you measured;
  do not ship a rect you know is wrong.
- You find yourself building any of the four consuming features
  (TTL hover-pause, weather peek, scorecard reveal, idle
  expanded-on-hover) — this plan ships the signal only.
- Emission turns out to require per-mouse-move events to feel correct —
  stop and reassess against the idle-cost discipline (plans 015/018)
  before proceeding.
- **You find yourself adding `resting_state` as an input to
  `active_card_rect` or special-casing the idle rect when
  `resting_state = "notch"` renders nothing** — the pinned decision
  (Step 2) is that the rect targets the would-be footprint unchanged.
  Deviating changes what all four consumers inherit (especially idle
  expanded-on-hover, which depends on hover firing over the bare rest
  region) and is the operator's call, not an engineering judgment.

## Maintenance notes

- **The rect table and `src/styles.css:25-61` are a duplicated-constants
  pair.** Any future change to a card width (270/400/460/500, the
  notch-mode clamp bounds, or the `--card-scale` multiplication) must
  change both. The Step 2 named-constant test is the tripwire; keep it.
- The `--card-scale` multiplication is the subtlest part of this plan
  and is absent from the spike's own sketch — if a future reader
  wonders why the function takes `scale`, this is why.
- When the four consumers wire up, they read the `.hovered` class (or
  the state driving it), not the tracking area directly — keep the
  seam at that boundary so the mechanism stays swappable if the
  undocumented AppKit behavior ever regresses.
- If a future macOS DOES break the behavior spike §2 relies on, the
  symptom is "hover never fires," not a crash and never the menu-bar
  bug. The fallback options are spike §3's mechanisms (a) and (c),
  both of which cost a permission prompt or a capabilities change —
  re-open the spike's §7 recommendation rather than improvising.
- `docs/design/hover-cursor-tracking.md` is this plan's rationale of
  record, including the rejected alternative and the accepted
  undocumented-behavior risk. Point future readers there rather than
  re-deriving the decision.
