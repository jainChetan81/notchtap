# Hover cursor tracking

## purpose

the overlay needs hover state for the idle peek, notification expansion,
rotation pause, the Agent Board, and the Icon Strip. the overlay window sits at
`NSStatusWindowLevel`, flush over the menu bar, so it must remain click-through.

## AppKit behaviour

an `NSTrackingArea` continues to deliver `mouseEntered`, `mouseMoved`, and
`mouseExited` while `ignoresMouseEvents` is true. click and drag dispatch remain
disabled. these behaviours use independent AppKit event paths.

notchtap therefore keeps `set_ignore_cursor_events(true)` unconditional. the
tracking area does not require Input Monitoring, Accessibility permission, a
global event monitor, or a polling loop.

## implementation

`src-tauri/src/lib.rs` attaches a `tauri-nspanel` tracking area to the existing
`OverlayPanel`. the handler converts AppKit coordinates into the card geometry
defined by `src-tauri/src/hover.rs`.

the native window has a fixed 500×300 frame. the rendered card occupies only
part of that frame in several states. rust therefore derives an active card
rectangle from:

- Presentation Mode and cutout geometry.
- card scale.
- Slot visibility and expansion state.
- idle-peek and hover-expansion latches.

the rectangle grows with revealed content. entering uses the compact rectangle,
and leaving uses the expanded rectangle. this hysteresis prevents the card from
collapsing beneath the pointer.

`src-tauri/src/hover.rs` also derives one hit rectangle per present Icon Strip
icon. `src-tauri/src/click.rs` uses those rectangles for native click handling.
the frontend remains receive-only.

rust emits `hover-changed` only when the boolean state changes. mouse movement
inside one state does not emit repeated events. `src/App.tsx` consumes the event
and passes the state into the overlay components.

## geometry contract

the constants in `src-tauri/src/hover.rs` mirror these sources:

- `src/styles.css` for card widths.
- `src/overlay/card-chrome.css` for flank geometry.
- `src/overlay/icon-strip.css` for icon size and spacing.
- `src/components/IdleHoverPeek.tsx` for idle-peek height.
- `src/App.tsx` for HUD cutout dimensions.

change each duplicated value and its source value in one commit. the named
geometry parity tests guard the fixed pairs.

showing and expanded content use conservative height estimates because their CSS
height is content-driven. the active rectangle must stay within painted content.

## security boundary

`src-tauri/capabilities/default.json` remains unchanged. hover state flows only
from rust to the overlay over the typed event channel. never add an overlay
invoke command for frontend-reported bounds. that would break the receive-only
boundary documented in `docs/ARCHITECTURE.md` §9.

the Icon Strip click path remains native. `src-tauri/src/click.rs` observes the
mouse event and publishes `tab-selection-changed`; the overlay never invokes
rust.

## verification

pure rust tests cover coordinate conversion, active card rectangles, hysteresis,
and Icon Strip geometry. frontend tests cover hover event consumption and the
resulting presentation states.

verify physical notch framing and menu-bar click-through on the macbook with the
manual checklist in `docs/TESTING_STRATEGY.md` §6.
