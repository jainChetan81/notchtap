# Plan 060: SPIKE — the overlay card visually merges with whatever's behind it in HUD mode

> **Executor instructions**: Decision spike, not a ready-to-execute CSS
> fix — investigation below found a real cross-mode risk that blocks a
> blind "just add a margin" fix. Read "Current state" before proposing a
> change.

## Status

- **Priority**: P2
- **Effort**: S (decision) / S (build, once decided)
- **Risk**: MED for the naive fix (see below), LOW for the mode-aware
  fix
- **Depends on**: none
- **Category**: bug / direction
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  screenshot: the idle-rail chip row ("FOOTBALL · NEWS · WEATHER OFF ·
  CLEAR") rendering flush against Chrome's own tab strip at the very top
  of the screen, visually merging with it — no gap, no visual separation,
  looks like a rendering bug even though it isn't one.

## Why this matters

On a notchless Mac (this operator's dev machine, HUD mode), the overlay
sits flush at the very top of the screen with sharp (non-rounded) top
corners, directly abutting whatever's rendered below the menu bar. The
screenshot shows this reading as broken — the card looks like it's part
of Chrome's window chrome rather than a distinct floating element.

## Current state — why this isn't a quick CSS tweak

Traced the full positioning stack:

- `src-tauri/src/lib.rs:551`: the window uses `NSStatusWindowLevel`
  specifically so it can render *above the menu bar itself* — the
  comment is explicit: *"Floating-tier levels cannot overlap the menu
  bar or appear over fullscreen Spaces; status level (25) can — required
  for the flush-to-top permanent overlay."* This is deliberate, not a
  bug: the whole point is to always be on top, even of the menu bar and
  fullscreen apps.
- `position_window` (`lib.rs:573-607`) sets Y to `0.0` for **both**
  notch-anchored and HUD-fallback (`position_top_center`, `lib.rs:560-568`)
  paths. For notch mode, the comment is explicit this is deliberate:
  *"y stays 0.0 deliberately: the cards sit flush with the screen top,
  inside the notch band."*
- `.rail-card` (`src/styles.css:22-31`) has `border-radius: 0 0
  var(--card-radius) var(--card-radius)` — **sharp top corners, rounded
  bottom only** — which is exactly the shape that makes a card blend
  into the top of the screen instead of reading as a floating rectangle.
  This is almost certainly intentional for notch mode (the card visually
  extends the notch cutout downward), which is also why it looks wrong
  in HUD mode (there's no cutout for it to extend).
- **The frontend has zero awareness of notch-vs-HUD mode.** Confirmed by
  grep: no file under `src/` (frontend) references the presentation mode
  at all. `presentation::Mode` (rust, `src-tauri/src/presentation.rs`)
  never crosses the IPC boundary to the webview. This means the exact
  same CSS renders identically in both modes today — there is no hook to
  make HUD mode look different from notch mode without adding one.

**The risk**: a universal CSS change (e.g. adding `margin-top` and
rounding the top corners on `.rail-card` unconditionally) would fix the
HUD-mode collision but *also* change the flush-notch look for notch-mode
users — and `CLAUDE.md` explicitly flags that "notch-mode behaviour still
needs per-change verification on the macbook," which isn't available on
this dev machine. Shipping an unverified visual change to notch mode
on a hunch is exactly the kind of mistake that caution exists to prevent.

## Decision needed (operator)

1. Is a mode-aware fix worth the (small) plumbing cost — thread
   `presentation::Mode` to the frontend (e.g. a data attribute set once
   at boot via the existing `slot-state`/`status-state` eval-splice
   pattern, or a dedicated one-shot event) so HUD mode can get rounded
   top corners + a few px of top margin while notch mode stays flush?
2. Or: is a universal change acceptable, accepting the "needs manual
   verification on the MacBook" risk explicitly, rather than building
   the mode-plumbing?
3. Or: is the current flush-top look actually fine/intentional even in
   HUD mode, and the real fix is narrower — e.g. just a stronger
   drop-shadow or a 1-2px hairline border for visual separation without
   changing the corner shape or position at all?

## Recommendation

Option 1 (mode-aware) is the only choice that doesn't risk regressing an
already-shipped, deliberately-tuned notch-mode behavior. The plumbing is
small: `position_window` already knows the mode at the exact call site
that sets Y=0 — passing that same fact to the frontend (one boolean) is
much cheaper than the risk of guessing wrong on hardware nobody in this
session can test against.

## Maintenance notes

- Whoever builds this must get a real MacBook (notch) smoke-check before
  calling it done — same discipline as plans 010/012/018/023/032/033/034,
  which all left a hardware-only manual check explicitly owed to the
  operator rather than claiming full verification from a notchless dev
  machine.
