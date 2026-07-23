import { useMemo } from "react";
import { SWAP_EXIT_MS } from "./animationTiming";
import { presentationFacts } from "./lib/presentationFacts";
import { useDelayedSwap } from "./useDelayedSwap";
import type { SlotState } from "./useSlotState";

// Plan 120: extracted verbatim from StatusRailCard.tsx (`:196-332` at
// 2a840c4) — the showing<->idle exit-choreography state machine. Every
// comment below moved with the code it documents (load-bearing design
// record, not filler). `showing` is recomputed internally from `slot`
// (matching StatusRailCard.tsx:126 exactly) rather than threaded in as a
// separate parameter, so the two can never drift apart across the hook
// boundary.
export function useExitChoreography(
  slot: SlotState,
  restingState: "rail" | "notch",
  hovered: boolean,
) {
  const showing = slot.state === "showing";

  // plan 078: originally the idle/showing content swap itself (freezing
  // the outgoing item via useDelayedSwap while a matching CSS exit
  // animation ran). plan 12x (wave 2) moved the content swap onto real
  // `AnimatePresence` (JSX below), which owns its own freeze — so this
  // hook is kept now for exactly one reason: plan 107's GEOMETRY
  // choreography (`geometryPriority`/`expanded`/`bare` immediately
  // below, and the StatusDots/IdleHoverPeek/idle-face mount gates
  // further down) must NOT move into motion (that plan's contract), and
  // still needs a literal, fake-timer-steppable JS exit window to hold
  // the outer shell's classes through. Hoisted above `cardClass` (plan
  // 105) so `bare`, below, can feed into it — `renderedShowing`/
  // `exiting` are needed before the class list is built, not after.
  const swapKey = showing ? slot.id : "idle";
  const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, SWAP_EXIT_MS);
  const renderedShowing = renderedSlot.state === "showing";

  // plan 11x: the below-block's own open/close signal — see
  // CONTENT_EXIT_MS's doc (animationTiming.ts) for why this differs from
  // `renderedShowing` on the EXIT side. ENTRANCE deliberately still
  // reads `renderedShowing` (unchanged: promotions wait the same 220ms
  // they always have, preserving the width-leads-content choreography).
  // EXIT deliberately does NOT add any JS-side delay on top of the live
  // `showing` flag going false — `belowBlockOpen` drops immediately, and
  // the below-block `motion.div`'s OWN `exit` transition (JSX below,
  // CONTENT_EXIT_MS long) is what supplies the close's actual duration.
  // Gating this on a SECOND delayed-swap timer here as well would double
  // that wait (React removes the child only once `belowBlockOpen` goes
  // false, THEN AnimatePresence's exit animation runs on top of that) —
  // confirmed empirically via a headless-Chrome timeline probe before
  // landing this line.
  const belowBlockOpen = showing && renderedShowing;

  // plan 107 Step B: the outer shell's geometry (priority accent class +
  // expanded width class) must not snap to idle the instant live
  // `showing` goes false — during the 220ms delayed-swap exit above,
  // below-block content is still showing-flavored (`renderedSlot`/
  // `renderedShowing`), and the shell's width formula has to stay in
  // lockstep with it or the card visibly shrinks to idle width WHILE the
  // old content is still fading out (the "grows before shrinking" exit
  // race the 105 ledger already named — entrance was always fine, see
  // below). Entrance (idle->showing): `showing` is live-true on the very
  // render the promotion arrives, so `slot.priority`/`slot.expanded`
  // apply immediately — unchanged from before this plan. Exit
  // (showing->idle): once `showing` goes false, fall back to
  // `renderedSlot` for as long as IT is still showing-flavored
  // (`renderedShowing`) — useDelayedSwap freezes that value for the
  // whole exit window, so priority/expanded stay put until the swap
  // actually completes. Only once both are false (the swap has settled)
  // does geometry become idle. Pinned by StatusRailCard.test.tsx's
  // "compact->idle geometry" describe block.
  const geometryPriority = showing
    ? slot.priority
    : renderedShowing
      ? renderedSlot.priority
      : "idle";
  const expanded = showing ? slot.expanded : renderedShowing && renderedSlot.expanded;

  // 2026-07-23 review fix (wave B, Task 1 — "one overlapping collapse"):
  // true ONLY during the genuine showing->idle exit's freeze window, never
  // during entrance. `exiting` (from useDelayedSwap, above) alone isn't
  // enough for this — its key changes on BOTH legs (an idle->showing
  // promotion re-keys `swapKey` too, so `exiting` goes briefly true right
  // after a promotion as well), and this class must never fire there. The
  // extra `!showing` pins it to the exit leg only: on entrance `showing`
  // is live-true from the very first render (width-leads-content, kept
  // untouched by this plan), so `shellExiting` is false throughout.
  // Drives the shell's `.exiting` CSS class (overlay-card.css): width
  // shrinks to the idle formula and the flank corners start rounding
  // IMMEDIATELY (t=0 of the exit) instead of waiting out the whole
  // SWAP_EXIT_MS geometry-class freeze above — that freeze still holds
  // `geometryPriority`/`expanded` (untouched; still needed for the accent
  // color and other content that reads them), but width/corner-round no
  // longer wait on it. Previously: content fade (0-105ms) -> corner round
  // (105-210ms, keyed off the below-block's DOM removal) -> width shrink
  // (175-495ms, keyed off the geometry-class flip) — three chained acts,
  // a ~70ms dead gap, ~495ms tail. Now all three start at t=0 and finish
  // by ~175ms. See overlay-card.css's own comment on
  // `.card-assembly.exiting` for the full geometry/timing.
  const shellExiting = !showing && renderedShowing;

  // plan 105 (Step C): narrows plan 085's original "zero app-drawn
  // pixels" promise to "zero app-drawn pixels *until hovered*" — the old
  // `return null` here made the mode a dead end: nothing painted AND
  // nothing was hoverable, so the peek could never be revealed once you
  // were in it. Gated on the delayed-swap-settled state
  // (`renderedShowing`/`exiting`), not the live `showing` flag, so a
  // still-exiting prior card finishes its normal exit animation exactly
  // as it does in "rail" mode; only once the swap has fully settled into
  // idle does notch mode go bare. Every `showing`/`exiting` path is
  // unaffected (identical to today). Hover detection itself is
  // `resting_state`-agnostic (src-tauri/src/hover.rs never reads it), so
  // the tracking area — and therefore `hovered` — already works
  // correctly here; the assembly only needs to keep mounting.
  const bare = restingState === "notch" && !renderedShowing && !exiting;

  // plan 123: kills the "box, then rounded shape pops in" exit artifact —
  // full mechanism in overlay-card.css's own `.exiting.exit-to-bare`
  // comment. `shellExiting` alone (above) drives IDENTICAL exit
  // choreography regardless of resting-mode look, converging on the WIDE
  // idle/rail geometry either way — correct for `restingState === "rail"`
  // (there is no bare shape to converge on), wrong for `restingState ===
  // "notch"` (the shell was always going to land on `.bare`'s cutout-only
  // geometry the instant the swap settles, so shrinking to the wide rail
  // width first, then jumping to cutout width at the class flip, is
  // exactly the discrete pop the operator flagged). This flag narrows
  // `shellExiting` to "exiting AND about to land on `.bare`, not `.idle`"
  // so overlay-card.css can converge the shell's width/flank paint/cutout
  // radii on the BARE geometry DURING the exit window instead of at the
  // flip. Rail mode never sets `bare` true (its own doc, above, pins that
  // to `restingState === "notch"`), so `exitToBare` is always false there
  // and the plain `.exiting` rule — and every rail-mode exit test — stays
  // byte-identical.
  //
  // plan 124 (F2, review fix): narrowed further with `&& !hovered`. Bug
  // (verified with the pointer resting on the card at settle): a hovered
  // exit used to still land on `exitToBare` true, so the shell converged
  // on `.bare`'s cutout-only geometry during the window — but the very
  // next render is `bare && hovered` (below), which flips two more CSS
  // rules on top of that: `.bare:has(.idle-peek)` re-widens `--cw` back to
  // the full idle formula (overlay-card.css:165-167) and `.bare.hovered`
  // repaints the flanks opaque again. Net effect: a 175ms shrink to
  // cutout width immediately followed by a ~320ms rebound back out to
  // idle width — a visible wobble the exit-to-bare mechanism exists
  // specifically to prevent. `!hovered` routes a hovered exit back onto
  // the plain `.exiting` rule instead (wide/idle-shaped `--cw`, the same
  // rule rail mode always used) — that convergence target was seamless
  // pre-123 and is exactly where a hovered bare-notch settle needs to
  // land anyway (hover already forces the idle-width `--cw` via
  // `.bare.hovered`/`:has(.idle-peek)`, so there is no second hop). A
  // hover arriving or leaving mid-window flips this flag on a live
  // render, and CSS transitions retarget continuously from whatever the
  // computed value currently is — no snap, no replay from a fixed start.
  const exitToBare = shellExiting && restingState === "notch" && !hovered;

  // 2026-07-23 (operator minimal-notch spec, Task 1.2/1.3): whether the
  // rail's painted chrome (flank paint, clock, dots) should be showing.
  // `bare` alone used to gate FlankClock, and `!renderedShowing && !bare`
  // gated StatusDots — meaning a genuine notification hid the dots (idle-
  // only furniture) and bare-hover had nothing to reveal but the peek
  // below. The operator's spec wants the OPPOSITE on both counts:
  // hovering a bare (minimal) notch should expand it into the full idle
  // rail (clock + cutout + dots), and an arriving notification should
  // keep that same rail visible above the compact/expanded card rather
  // than hiding it — one continuous shape, never a detached slab +
  // floating card. `bare` is already false the instant a promotion
  // starts (see its own doc: `!exiting` flips first) and stays false for
  // the whole showing/exiting window, so `railRevealed` is true
  // throughout that window on its own — the `hovered` half of the OR
  // only ever matters while genuinely bare.
  const railRevealed = !bare || hovered;

  // idle face: true idle only — not while a card is showing OR still
  // exiting (the delayed-swap window), and not while hovered (the hover
  // primitive's live prop, never CSS `:hover`, matching every other hover
  // consumer above). Deliberately keyed on `renderedShowing`/`exiting`
  // (the same delayed-swap-settled basis StatusDots/IdleHoverPeek already
  // use just below), not the live `showing` flag alone, so the face
  // doesn't flash back on mid-exit before the swap actually settles.
  const trueIdle = !showing && !renderedShowing && !exiting && !hovered;

  // 2026-07-23 review fix: `.idle-face` is CSS-hidden (`display: none`)
  // for the ENTIRE lifetime of a real notch-hardware device — see
  // overlay-card.css's `:root[data-notchtap-mode="hud"] .card-root
  // .idle-face` rule, the only thing that ever flips it to `display:
  // flex`. That gate is the boot-time device mode (`presentationFacts`),
  // NOT `restingState`/`bare` (a user-chosen idle-LOOK preference that's
  // orthogonal to whether hardware notch pixels exist — `.bare` never
  // touches `.idle-face`'s grid cell). Read once — like App.tsx's own
  // `presentationFacts()` call, this reflects a boot-time global that
  // never changes for the process's lifetime, so there's nothing to
  // resubscribe to. On real notch hardware this keeps `<IdleFace>`
  // unmounted entirely, so its reveal-delay timer and the gaze/blink
  // `setTimeout` loops inside it never arm in the first place — mirrors
  // how `FlankClock` below is conditionally rendered rather than always
  // mounted-but-hidden.
  const idleFaceEligible = useMemo(() => presentationFacts().mode !== "notch", []);

  return {
    renderedSlot,
    exiting,
    renderedShowing,
    belowBlockOpen,
    geometryPriority,
    expanded,
    shellExiting,
    bare,
    exitToBare,
    railRevealed,
    trueIdle,
    idleFaceEligible,
  };
}
