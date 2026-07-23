# Plan 060: SPIKE — the overlay card visually merges with whatever's behind it in HUD mode

> **SUPERSEDED — do not execute.** Folded into 079 item 1 and shipped
> by plan 091 (2026-07). Kept for historical context; see
> plans/README.md.

> **Executor instructions**: Decision spike, not a ready-to-execute CSS
> fix — investigation below found a real cross-mode risk that blocks a
> blind "just add a margin" fix. Read "Current state" before proposing a
> change.

## Status

- **Priority**: P2
- **Effort**: S (decision) / S (build, once decided)
- **Risk**: MED for the naive fix (see below), LOW for the mode-aware
  fix
- **Depends on**: none strictly (this plan is independently decidable
  and buildable), but **soft-coordinates with plan 063** — see
  Maintenance notes; whichever of the two is built first determines the
  shape of a fact both need.
- **Category**: bug / direction
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  screenshot: the idle-rail chip row ("FOOTBALL · NEWS · WEATHER OFF ·
  CLEAR") rendering flush against Chrome's own tab strip at the very top
  of the screen, visually merging with it — no gap, no visual separation,
  looks like a rendering bug even though it isn't one.
- **Review-plan pass (2026-07-20)**: own read + a fresh-context
  subagent cold-read, cross-checked; the cross-check against plan 063
  (its "Coordinates with 063" note referencing this plan) found no
  overstatement on either side. Found and fixed: (1) two stale
  citations — `lib.rs:551` and `lib.rs:560-568`/`573-607` had drifted
  ~4-7 lines from repo churn since this plan was planned; corrected
  below to the current, individually-verified line numbers (the quoted
  text itself was always accurate — this was pure line-number drift,
  not a wrong claim). (2) A real tension: "Depends on: none" understated
  the sequencing coupling the Maintenance notes already described with
  063 — fixed to say so plainly instead of a bare "none" a skimming
  reader could take at face value. (3) A gap neither this plan nor 063
  specified: the two plans agree to "build one shared channel" but
  neither pinned down its actual shape, risking two independent builders
  inventing incompatible ones — added a concrete suggested shape below.
  (4) A previously-unraised risk, confirmed real by reading the actual
  boot sequence: the overlay window is `"visible": true` from launch
  (`tauri.conf.json`) and every existing boot-time fact
  (`__NOTCHTAP_SLOT_STATE__`, `__NOTCHTAP_APPEARANCE__`, etc.) arrives via
  `webview.eval` inside `.on_page_load` (`lib.rs:393-457`) — *after* the
  page can already paint. A new mode fact delivered the same way has the
  same window for a one-frame flash of the wrong corner/margin treatment
  before it arrives; added a mitigation (default to notch's existing
  flush look, not the new HUD treatment, so any flash is toward the
  already-correct state).

## Why this matters

On a notchless Mac (this operator's dev machine, HUD mode), the overlay
sits flush at the very top of the screen with sharp (non-rounded) top
corners, directly abutting whatever's rendered below the menu bar. The
screenshot shows this reading as broken — the card looks like it's part
of Chrome's window chrome rather than a distinct floating element.

## Current state — why this isn't a quick CSS tweak

Traced the full positioning stack:

- `src-tauri/src/lib.rs:555-556`: the window uses `NSStatusWindowLevel`
  specifically so it can render *above the menu bar itself* — the
  comment is explicit: *"Floating-tier levels cannot overlap the menu
  bar or appear over fullscreen Spaces; status level (25) can — required
  for the flush-to-top permanent overlay."* This is deliberate, not a
  bug: the whole point is to always be on top, even of the menu bar and
  fullscreen apps.
- `position_window` (`lib.rs:580-614`) sets Y to `0.0` for **both**
  notch-anchored and HUD-fallback (`position_top_center`, `lib.rs:567-575`)
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
   `presentation::Mode` to the frontend so HUD mode can get rounded top
   corners + a few px of top margin while notch mode stays flush?
   **Concrete shape, so this doesn't get built two incompatible ways
   between this plan and 063**: follow the existing `.on_page_load`
   eval-splice pattern exactly (`lib.rs:393-457` — the same call site
   that already sets `__NOTCHTAP_SLOT_STATE__`/`__NOTCHTAP_APPEARANCE__`),
   adding one more assignment, e.g.
   `window.__NOTCHTAP_MODE__ = "notch" | "hud";` — a plain string so
   plan 063's Option 1 can extend the *same* eval call with a second
   field later (e.g. `window.__NOTCHTAP_CUTOUT_WIDTH__ = <number>;`)
   without redesigning the channel. Frontend side: a new small hook
   (`src/usePresentationMode.ts`, matching the existing
   `useSlotState.ts`/`useStatusState.ts` file-per-hook convention) reading
   the global once at mount, mirroring `initialSlotState()`'s pattern in
   `useSlotState.ts:129-133` — read once, no listener needed, since the
   mode never changes after boot.
2. Or: is a universal change acceptable, accepting the "needs manual
   verification on the MacBook" risk explicitly, rather than building
   the mode-plumbing?
3. Or: is the current flush-top look actually fine/intentional even in
   HUD mode, and the real fix is narrower — e.g. just a stronger
   drop-shadow or a 1-2px hairline border for visual separation without
   changing the corner shape or position at all?

**Risk to account for regardless of which option is picked, if it
touches CSS driven by the new mode fact**: the overlay window is
`"visible": true` from launch and every existing boot-time fact arrives
via `.on_page_load` eval — *after* the page can already paint (confirmed
by reading the boot sequence directly, not assumed). A mode-driven CSS
class has the same one-frame window to render with the *default* (pre-
fact) styling before the real value lands. Whatever default the CSS
falls back to before `__NOTCHTAP_MODE__` is set should be **notch
mode's existing flush/sharp-corner look**, not the new HUD treatment —
that way any momentary flash happens in the mode already correctly
tuned (notch), never in the one being fixed, and never risks a
flash of the *wrong* shape in notch mode specifically (the case this
whole plan is careful not to regress).

## Recommendation

Option 1 (mode-aware) is the only choice that doesn't risk regressing an
already-shipped, deliberately-tuned notch-mode behavior. The plumbing is
small: `position_window` already knows the mode at the exact call site
that sets Y=0 — passing that same fact to the frontend (one boolean,
concrete shape above) is much cheaper than the risk of guessing wrong on
hardware nobody in this session can test against.

## Maintenance notes

- Whichever option is picked, a vitest test for the new hook/fact-read
  (mirroring `useSlotState.test.ts`'s coverage of `initialSlotState`) is
  in scope and cheap; the *visual correctness* of the resulting CSS
  (does HUD mode actually look right, does notch mode still look flush)
  is manual-only, same as every other hardware-dependent check in this
  batch — don't let a green test suite substitute for the MacBook
  smoke-check below.
- Whoever builds this must get a real MacBook (notch) smoke-check before
  calling it done — same discipline as plans 010/012/018/023/032/033/034,
  which all left a hardware-only manual check explicitly owed to the
  operator rather than claiming full verification from a notchless dev
  machine.
- **Coordinates directly with plan 063** (added 2026-07-20, during a
  review-plan pass on 063): both this plan and 063 independently need
  the same new mode-boolean fact ("am I in notch mode?") threaded from
  rust to the frontend — 063's idle-rail width cap needs it just as much
  as this plan's corner/margin treatment does. Whichever lands first
  should build ONE shared "presentation facts" boot-time channel (with
  room for 063's Option 1 to later add a numeric cutout-width alongside
  the boolean, if that option is chosen) rather than each inventing its
  own single-purpose one-shot fact.
