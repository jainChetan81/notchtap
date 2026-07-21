# Design spike: hover enablement — dynamic cursor-event tracking on the overlay window

> **Status**: design spike (plan 086), zero production code changes.
> Researched against commit `3de785a` (worktree reset target; drift check
> `git diff --stat 4fb3af9..HEAD -- src-tauri/src/lib.rs
> src-tauri/Cargo.toml src-tauri/capabilities/default.json
> docs/ARCHITECTURE.md` is clean at this commit — confirmed). All
> `file:line` citations below were read fresh at this commit; the
> `tauri-nspanel` citations are against the exact pinned rev in
> `src-tauri/Cargo.toml:37` (`a3122e894383aa068ec5365a42994e3ac94ba1b6`),
> read from the local git cache at
> `~/.cargo/git/checkouts/tauri-nspanel-cab3955568b3504c/a3122e8/`, not
> from the project's docs/README.
>
> **A throwaway Swift AppKit POC was built and run** to resolve the one
> empirical question this spike turned on (see §2). It lives outside
> this repo (`/private/tmp/.../scratchpad/tracking_poc.swift`, this
> session's scratchpad) and was never added to the tauri build, any
> Cargo target, or any render path — it is not part of this commit and
> nothing here depends on it surviving.

## 1. Why this matters

Every hover interaction in the locked redesign — idle weather peek
(plan 082), football rail→compact reveal (plan 084), TTL-bar
hover-pause (plan 081), and the expanded-on-hover idle state — is
mocked and approved in the prototypes, and every one is blocked by one
line: `src-tauri/src/lib.rs:554`,
`window.set_ignore_cursor_events(true)?;`, called unconditionally.
The comment above it (`lib.rs:546-553`) records why: the overlay window
sits at `NSStatusWindowLevel` flush over the real menu bar (not just in
notch mode — see §4's geometry note), and without click-through it
swallows clicks meant for other apps' menu-bar tray icons — the exact
2026-07-17 bug this line fixed. This spike's job is to find a mechanism
that gives real hover-in/hover-out detection scoped to the currently
rendered card, without ever reopening that bug, and to cost it honestly
against this app's security and idle-CPU posture.

## 2. The decisive empirical test

The plan's own framing treated "does a tracking mechanism require
disabling click-through" as the open, load-bearing question — mechanism
family (c) explicitly worried that "tracking areas require the window to
accept mouse events, conflicting with the unconditional click-through."
**That assumption is wrong, verified empirically on this machine.**

**Method**: a standalone Swift/AppKit program (no Tauri, no Rust)
created a borderless, non-activating `NSPanel` — first at `.floating`
level, then re-run at `NSWindow.Level(rawValue: 25)`
(`NSStatusWindowLevel`, matching `lib.rs:573` exactly), positioned flush
to the physical top of the screen (matching `position_window`'s
"y stays 0.0" placement rule). It attached an `NSTrackingArea`
(`.activeAlways, .mouseEnteredAndExited, .mouseMoved, .inVisibleRect`)
to its content view and overrode `mouseEntered`/`mouseExited`/
`mouseMoved`/`mouseDown`. `CGEventPost` then synthesized real
mouse-moved and mouse-click events at the window's screen coordinates,
first with `panel.ignoresMouseEvents = true`, then again with it set to
`false` on the *same* window (isolating that one flag as the variable).

**Result** (macOS 26.5.2, this Mac mini, both panel levels — reproduced
twice):

```
PHASE 1: ignoresMouseEvents = true
EVENT mouseEntered
EVENT mouseMoved
EVENT mouseMoved
EVENT mouseExited
PHASE 2: ignoresMouseEvents = false
EVENT mouseEntered
EVENT mouseMoved
EVENT mouseDown (CLICK WAS CAPTURED BY THIS WINDOW)
EVENT mouseExited
RESULT enteredWhileIgnoring=true exitedWhileIgnoring=true
       clickCapturedWhileIgnoring=false
       enteredWhileAccepting=true clickCapturedWhileAccepting=true
```

**Reading this**: with `ignoresMouseEvents = true`, the tracking area's
`mouseEntered`/`mouseMoved`/`mouseExited` fire *normally* — but a real
click (`mouseDown`) at the identical location is **not** captured
(`clickCapturedWhileIgnoring = false`). With `ignoresMouseEvents =
false`, both fire, including the click, as the control case. This shows
macOS gates these two behaviors through independent mechanisms: click/
drag dispatch (what the 2026-07-17 fix needed to suppress) is gated by
`ignoresMouseEvents`; tracking-area enter/exit/move notifications are
not. **A hover primitive can be built on this window with zero change
to `set_ignore_cursor_events(true)` and zero risk of resurrecting the
menu-bar bug**, because the exact call that fixed it is untouched and
independently re-verified to still block clicks in the presence of an
active tracking area.

## 3. Mechanism-by-mechanism verdict

**(b) `tauri-nspanel`'s built-in tracking-area support — the answer.**
The pinned rev already ships this, fully built, with two worked
examples (`examples/mouse_tracking/src-tauri/src/main.rs`,
`examples/hover_activate/src-tauri/src/main.rs`) — it is not
alpha/undocumented surface, it is a shipped feature of the exact
dependency version this repo already depends on
(`src-tauri/Cargo.toml:37`). The macro block:

```rust
panel!(MyPanel {
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

(`TrackingAreaOptions` builder: `src/builder.rs:244-330` in the pinned
checkout; the macro's generated `add_tracking_area` helper:
`src/panel.rs:659-696`) attaches a real `NSTrackingArea` to the panel's
content view and exposes `on_mouse_entered`/`on_mouse_exited`/
`on_mouse_moved`/`on_cursor_update` closures on the event handler
(`src/panel.rs:24-30`, `72-79`). This repo's overlay window is already a
`tauri_panel!`-defined panel (`OverlayPanel`, `lib.rs:42-49`) built the
same way — extending its macro invocation with a `with: { tracking_area:
{...} }` block is additive to code that already exists, not new
infrastructure.

**(c) Hand-rolled `NSTrackingArea` via `objc2_app_kit` directly.**
Mechanically identical to (b) (same AppKit primitive; `objc2_app_kit` is
already a direct dependency, `Cargo.toml:36`, already used at
`lib.rs:545`), but strictly more code for the same result, since (b)
already wraps it with less unsafe surface. Not recommended over (b) —
no reason to hand-roll what the pinned dependency ships.

**(a) A global cursor watcher (polling or `CGEventTap`).**
Not needed. §2 shows the window can learn about its own hover state
without watching the whole desktop. Building (a) anyway would mean
carrying either a periodic poll loop (fighting the plan's own cited
idle-CPU discipline, §5) or a global event tap (a materially bigger
capability — see §4) to solve a problem (b) already solves for free,
locally, with the dependency already in the tree. **Rejected** for that
reason alone, independent of its own permission cost (documented in §4
for completeness, since the plan asks for it, but it is moot to this
recommendation).

## 4. Security analysis

**No new sensitive capability is requested.** The recommended
mechanism (b) is local to the app's own already-open window — it
observes mouse position only when the cursor is physically over
notchtap's own on-screen pixels, the same scope any ordinary click
target already has. It requires:
- No `CGEventTap` — no Input Monitoring/Accessibility permission
  prompt. (For completeness, since the plan asks: a system-wide
  `CGEventTap` on any event type, including mouse-only, has required
  Input Monitoring trust since macOS Catalina — this is documented
  Apple platform behavior, not independently re-verified in this spike
  since mechanism (a) turned out to be unnecessary.)
- No `NSEvent.addGlobalMonitorForEvents` — global monitors watch every
  window on the desktop, not just this app's; even though mouse-only
  global monitors have not historically required extra permission
  (only keyboard monitoring does), "global" is a strictly bigger
  capability than this app needs and was correctly avoided.
- **No change to `src-tauri/capabilities/default.json`.** The hover
  signal flows rust→frontend as a new custom event (e.g.
  `hover-changed`), emitted via `webview.emit(...)` — the exact
  mechanism `appearance-changed` (`lib.rs:459`) and
  `notification-promoted` already use. Tauri v2's capability model
  scopes the `event` plugin's `listen`/`unlisten` *commands*
  (`core:event:allow-listen`/`allow-unlisten`, already granted), not
  individual event *names* — there is no per-event-name permission to
  loosen. The frontend stays receive-only exactly as CLAUDE.md's ipc
  section requires; **this spike found no mechanism that requires
  touching the locked default capability set**, and specifically
  rejected the one design (frontend reports its own rendered card
  bounds back to rust via a new invoke command — see §5) that would
  have required exactly that.
- `set_ignore_cursor_events(true)` (`lib.rs:554`) is **never made
  conditional in this design** — it is called exactly as today, at both
  existing sites (`lib.rs:258` setup, `lib.rs:483` page-load re-apply),
  unconditionally, forever. §2's POC is the direct evidence that this
  is sufficient: click capture stays off regardless of tracking-area
  state.
- **Geometry caveat, not a permission concern**: the fixed OS window is
  500×300 (`tauri.conf.json`'s `"width": 500, "height": 300,
  "resizable": false`) at every Presentation Mode, while the
  CSS-rendered card is one of several narrower widths (270/400/460/500,
  further clamped to cutout width in notch mode — `src/styles.css:39-61`).
  The window's true bounds extend beyond the rendered card into menu-bar
  icon territory in **both** modes (not just HUD — the 500px-wide window
  is centered on the cutout, which is typically narrower than 500px, so
  even in notch mode the window's dead margin overlaps real menu-bar
  icons; this is exactly why `lib.rs:554` is unconditional rather than
  notch-only). §5's design derives the *active hover rect* from state
  rust already owns, specifically so a hover signal is never asserted
  for a screen region wider than what is actually rendered.

## 5. Perf analysis

`NSTrackingArea` enter/exited/moved delivery is event-driven at the
window-server level — it is not a polling loop and not a periodic
timer. It produces **zero wakeups while the cursor is not over the
window**, which is the overwhelming majority of a 24/7 background app's
runtime — a materially better idle-cost story than either historical
target this repo has already cut: the 250ms heartbeat tick (plan 015,
replaced with deadline-based wakeups — this repo's own the precedent
for "sub-second repeating timers defeat App Nap") or the CSS shader
repaint cost (plan 018). This mechanism doesn't need a plan-015-style
redesign because it was never a timer to begin with — it inherits the
OS's own hit-testing/event-dispatch cost model, the same one every
ordinary clickable UI element in every other app already pays only
while genuinely hovered.

While actively hovered, `mouseMoved` firing rate is bounded to exactly
the span of time a user's cursor is voluntarily over the window's small
on-screen footprint — comparable to normal UI event-handling cost in any
app, not a background cost. The rust-side handler work per event is a
point-in-rect comparison against a small, already-known state table
(§6) — O(1), no `String` clones, no allocation — notably *cheaper* than
the per-tick `slot_state_if_changed()` comparison plan 015 flagged as a
real cost (cloning every visible card field 4×/second) before that plan
replaced the timer entirely.

## 6. The remaining engineering question: mapping "somewhere in the fixed window" to "over the rendered card"

`add_tracking_area` (`panel.rs:659-696` in the pinned checkout) attaches
the tracking area to the panel's **content view**, at that view's
current `bounds` — i.e., the whole fixed 500×300 window, not any
particular CSS-rendered sub-rect. `auto_resize: true` only sets an
`NSAutoresizingMaskOptions` mask on the content view (so if the *window
frame* changes size, its subviews stretch) — irrelevant here since
`tauri.conf.json` has `"resizable": false` and the window frame never
changes; what changes is the CSS width *within* that fixed frame
(idle 270/400px vs expanded 500px vs notch-mode's `clamp(270px,
--notchtap-cutout-width, 460px)`, `src/styles.css:38-61`). The tracking
area, on its own, cannot distinguish "cursor over the visible card" from
"cursor over the dead margin around it" — that distinction has to be
computed in the `on_mouse_moved`/`on_mouse_entered` handler, comparing
the event's `locationInWindow` against a known-current "active card
rect."

**Two ways to get that rect, one recommended:**

- **RECOMMENDED — rust derives it from state it already owns.** The
  Engine already tracks exactly what a rect-derivation function needs:
  whether an item is visible, whether it's expanded
  (`toggle_manual_expand`, the `expanded` flag threaded through queue
  state), the Presentation Mode (`notch`/`hud`), and the cutout width
  (already computed and pushed to JS today via
  `cutout_width_js_value`, `lib.rs:597-604` — rust is already the
  geometry authority for that one input, one direction). A small
  rust-side table mirroring `styles.css`'s breakpoints (270/400/460/500,
  the notch-mode clamp) lets the handler compute a conservative
  (deliberately not pixel-perfect — slightly wider than the exact
  rendered edge is fine) active-rect per state, with **zero new IPC
  surface**. Cost: a duplicated-constants drift risk between this table
  and `styles.css`, mitigated by a comment cross-referencing both files
  and a unit test asserting the table's widths match named constants
  (not a live CSS parse — keep it simple).
- **REJECTED — frontend reports its own rendered bounds to rust** via a
  new invoke command (e.g. `report_card_bounds`). More precise (no
  conservative-strip overshoot), but it requires adding a new
  `#[tauri::command]` reachable from the **main** overlay window —
  exactly the boundary CLAUDE.md's ipc section locks down
  ("`capabilities/default.json` must never change... the frontend
  should not be able to trigger [anything] — only display what the rust
  core sends it"). That is a real, opt-in security-posture change (a
  new frontend→rust write channel that doesn't exist today), not a
  free engineering choice — this spike treats it the same way plan 073
  treated its own open call: **flagged for the maintainer, not decided
  here.** The conservative-strip approach avoids the question entirely
  and is recommended for that reason, not merely because it's simpler.

## 7. Recommendation

**Chosen mechanism: (b), `tauri-nspanel`'s built-in tracking-area
support on the existing `OverlayPanel`, with hover-zone geometry derived
rust-side from already-owned Engine/Presentation-Mode state (§6's
recommended option).** `set_ignore_cursor_events(true)` stays
unconditional and untouched at both existing call sites. No new
capability, no permission prompt, no global watcher, no polling loop.

**Effort: S/M.**
- `src-tauri/src/lib.rs`: extend the `OverlayPanel` `tauri_panel!`
  invocation (`lib.rs:42-49`) with a `with: { tracking_area: {...} }`
  block (mirrors `examples/mouse_tracking`/`examples/hover_activate`
  almost verbatim) and a `panel_event!` handler wiring
  `on_mouse_entered`/`on_mouse_exited`/`on_mouse_moved` to a small
  rect-derivation function + a `webview.emit("hover-changed", ...)`
  call, same shape as the existing `appearance-changed` emission.
- A small, pure, unit-testable rust function: `(mode, cutout_width,
  visible, expanded) -> Rect` — this is the one genuinely new piece of
  logic, and it's exactly the kind of pure decision function
  `TESTING_STRATEGY.md` already prefers (like `presentation_mode`) —
  no GUI test needed, only value-in/value-out assertions.
- No Swift subprocess needed (unlike the `notchtap-detect` precedent) —
  this rides entirely on the already-pinned Rust dependency
  (`tauri-nspanel`) and the already-used `objc2_app_kit` crate. Rust/
  Swift/frontend split: **all rust, plus a small frontend listener**
  (a `hover-changed` event handler toggling a CSS state class) —
  wiring the *four* actual hover features (081/082/084/idle
  expanded-on-hover) that would consume that class is explicitly
  **out of scope for this spike** and should be scoped as their own
  (now unblocked) build steps.

**One item is a genuine, maintainer-level open call, not resolved
here**: conservative hover-strip precision vs. pixel-perfect tracking
(§6). This spike recommends the conservative strip as the default
(avoids any capability change) but flags that if a future card layout
makes the strip's overshoot cosmetically noticeable on real hardware,
revisiting toward the rejected bounds-reporting alternative is a UX/
security trade the maintainer should make explicitly, not something
this spike should default into.

This is a clear-favorite outcome, not an options-presented STOP: the
empirical test in §2 collapsed what looked like a three-way,
security-sensitive trade-off into "the dependency already in the tree
already does this, for free, without touching the one line that
matters."

## 8. Test strategy

- A pure unit test suite for the rect-derivation function (§6/§7):
  table-driven over `(mode, cutout_width, visible, expanded)` →
  expected `Rect`, matching the existing style of `presentation_mode`'s
  tests (`TESTING_STRATEGY.md` §4.4's stated pattern for exactly this
  kind of pure decision function).
- No new GUI/hardware test — `TESTING_STRATEGY.md` §5 already treats
  notch/HUD physical-window behavior as manual-only by design; this
  spike's own POC (§2) is the one-time empirical unblock, not a
  permanent automated check. A build plan should still include a manual
  smoke step: move the mouse over a live card on real hardware, confirm
  a hover class toggles in the DOM inspector, confirm a menu-bar icon
  directly overlapped by the window's dead margin still responds to a
  real click.
- `cargo test`/`npx vitest run` should stay at the current counts
  (345 + 3 doc-tests / 124) until an actual build plan lands the
  rect-derivation function and its tests — this spike adds no committed
  code.

## Maintenance notes

- **The rect-derivation table vs. `styles.css`'s breakpoints is a
  duplicated-constants risk.** Any future change to a card width in
  `styles.css` (270/400/460/500, or the notch-mode clamp bounds) needs a
  matching update to the rust-side table, or the hover zone silently
  drifts from the rendered card. Cross-reference both files in comments;
  consider (not required at S/M scope) a shared constants source if this
  proves to drift in practice.
- **`capabilities/default.json` stays locked.** If a future pass wants
  pixel-perfect hover bounds via a frontend-reported-bounds invoke
  command (§6's rejected alternative), that is an explicit reopening of
  the "frontend is receive-only" boundary CLAUDE.md documents as
  load-bearing — treat it with the same weight as any other
  capabilities-file change, not as an incidental part of a hover-polish
  pass.
- **The POC script is not part of this repo.** It was written and run
  from this session's scratchpad directory, never touched
  `src-tauri/Cargo.toml`, any build file, or any render path, and is not
  included in this commit. A future build plan implementing §7 should
  treat this doc's §2 findings as the empirical grounding and does not
  need to re-run or resurrect that script, though re-verifying on the
  actual notch macbook (this spike ran on the notchless Mac mini) is
  still worthwhile before considering notch-mode behavior fully proven —
  the window-level/click-through mechanics tested here are
  mode-independent, but real hardware confirmation of notch-mode framing
  has not been done by this spike.
