import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  CONTENT_EXIT_MS,
  NOTCHTAP_EASE,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SWAP_EXIT_MS,
} from "../animationTiming";
import { renderInlineMarkdown } from "../lib/markdown";
import {
  ageLabel,
  type Celebration,
  categoryClass,
  categoryLabel,
  eventKindPresentationFor,
  footballEventKindFor,
  livePillVariantFor,
} from "../lib/presentation";
import { weatherArtFor } from "../lib/weatherArt";
import { useExitChoreography } from "../useExitChoreography";
import type { EspnMeta, SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import { IdleFace } from "./IdleFace";
import { IdleHoverPeek } from "./IdleHoverPeek";
import { LiveMatchScorecard } from "./LiveMatchScorecard";
import { NotificationBody } from "./NotificationBody";
import { StatusDots } from "./StatusDots";

// plan 084: the live scorecard's celebration classes — echoes the shipped
// pulse-goal/pulse-red discipline (keyed on [currentId, currentSignal],
// cleared on the ending keyframe's animationend) but scoped to the
// espn-structured-meta branch so a live-branch goal never stacks BOTH
// `pulse-goal` and `cele-goal` (see the `isLiveCard` gate below).
// `Celebration` (lib/presentation.ts) already IS the class-name union, so
// there's no separate translation table to keep in sync with Step 1's.
const CELEBRATION_END_ANIMATION: Record<NonNullable<Celebration>, string> = {
  "cele-goal": "cele-ring",
  "cele-yc": "cele-ring",
  "cele-rc": "red-strobe",
};

// plan 082: weather ALERT cards ride the plan-035 display-only `details`
// channel to carry condition + day/night art inputs — plan 096 later put
// `origin` on the slot-state wire, but weather art derivation has no
// reason to move off these markers (they carry condition/day-night, which
// `origin: "weather"` alone doesn't). Every pair whose label starts with
// "wx-" is a MARKER, never real content: it must be read to derive the
// mood/glyph art, then excluded from every place `details` is rendered as
// visible text (the marker-leak guard).
function isWxMarker(label: string): boolean {
  return label.startsWith("wx-");
}

// plan 120: exported so NotificationBody.tsx (src/components/) can import
// the shape rather than duplicating this two-field structural type — one
// definition, not two that can drift.
export type Detail = { label: string; value: string };

function visibleDetails(details: Detail[]): Detail[] {
  return details.filter((detail) => !isWxMarker(detail.label));
}

function weatherArtFromDetails(details: Detail[]) {
  const condition = details.find((detail) => detail.label === "wx-condition")?.value;
  if (condition === undefined) {
    return null;
  }
  const isDay = details.find((detail) => detail.label === "wx-is-day")?.value === "1";
  return weatherArtFor(condition, isDay);
}

// plan 12x (wave 2): mirrors shared-ui's `--ease-notchtap`
// (vendor/shared-ui/design/tokens.css: `cubic-bezier(.22, 1, .36, 1)`) —
// motion's `transition.ease` takes a bezier array, not a CSS var, so this
// is the JS-side twin of that token for the showing-flavored content swap
// below (mirrors card-enter-showing/card-exit-showing's old
// `var(--ease-notchtap)`). 2026-07-23: the literal moved to
// animationTiming.ts's exported NOTCHTAP_EASE (imported above) with a
// token-parity guard test — no local copy here anymore. The idle-flavored
// swap (StatusDots) keeps motion's built-in "easeOut", matching the old
// card-enter-idle/card-exit-idle's plain `ease-out`.

type Pulse = "pulse-goal" | "pulse-red" | null;

// Which @keyframes name (styles.css) ends each pulse — the *only* place
// either duration lives is the CSS animation itself; clearing on
// animationend means there's no JS-side duration to keep in sync with it.
const PULSE_END_ANIMATION: Record<NonNullable<Pulse>, string> = {
  "pulse-goal": "goal-overshoot",
  "pulse-red": "red-alert",
};

// plan 127 (Step 3, /improve-animations audit finding #3): the content
// swap's `exit` leg, as a motion `variants` function keyed on
// `isRotation` (below) — the exiting `motion.div` (the OLD `swapKey`)
// stops receiving fresh props from this component's own render the
// instant its key drops out of the JSX, so there is no other way to
// hand it the freshly-computed `isRotation` boolean than `AnimatePresence`'s
// own `custom` prop, which motion re-evaluates variant FUNCTIONS with for
// exiting children specifically (the documented mechanism the plan's own
// doc names). `initial`/`animate` don't need this: the ENTERING child is
// still live in the render tree, so they read `isRotation` directly from
// closure, no variants indirection needed — only `exit` is a variant
// label here, kept as a module-level constant (not per-render) since it
// depends on nothing but its `custom` argument.
// plan 129 (T3, deep-review fix): exported (test-only export, same
// precedent as `iconForBundleId` in IdleHoverPeek.tsx) so
// StatusRailCard.test.tsx can pin the two durations/ease directly
// against this object rather than only indirectly, through rendered
// motion output — jsdom/motion don't expose a committed animation's
// `transition` back onto the DOM the way plain CSS values are
// inspectable, so the variant function itself is the only place these
// three values are actually checkable.
export const contentExitVariants = {
  exit: (isRotation: boolean) =>
    isRotation
      ? { opacity: 0, transition: { duration: ROTATION_EXIT_MS / 1000, ease: NOTCHTAP_EASE } }
      : { opacity: 0, transition: { duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE } },
};

export function StatusRailCard({
  slot,
  status,
  restingState = "rail",
  hovered = false,
}: {
  slot: SlotState;
  status?: StatusState;
  // plan 085: the resting-state render choice. Optional, defaulting to
  // "rail" — every existing caller (tests, the settings preview) that
  // never passes it keeps today's idle rail, byte-identical.
  restingState?: "rail" | "notch";
  // plan 087: the hover primitive's one diagnostic consumer — a real
  // `hover-changed` event drives this in the shipped app; every other
  // caller (tests, the settings preview) that never passes it keeps
  // today's un-hovered render, byte-identical. Consuming features
  // (081/082/084/idle expanded-on-hover) are each their own follow-on
  // work — this prop only proves the signal arrives.
  hovered?: boolean;
}) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const currentBody = showing ? slot.body : null;
  const news = showing && slot.eventType === "news_item";
  // plan 084: detect the live-match football branch by the structured
  // `espn` block's presence (POST-083 contract), never by string-sniffing
  // eventType/signal — off the LIVE slot, matching how `news`/`wxArt`
  // above are computed, so the pulse-vs-celebration gate below always
  // reflects the arriving item.
  const isLiveCard = showing && slot.espn !== undefined;

  const [pulse, setPulse] = useState<Pulse>(null);

  // plan 127 (Step 3): backs `isRotation` below (computed right where
  // `swapKey` is, further down) — declared up here with the component's
  // other hooks, per this file's usual convention. `key` starts at
  // `undefined` (never equal to a real `swapKey` on the very first
  // render, guaranteeing the guard below never mistakes mount for a
  // same-key re-render); `isRotation`/`wasShowing` both start `false`
  // since nothing has ever "shown" yet.
  const wasShowingRef = useRef<{ key: unknown; isRotation: boolean; wasShowing: boolean }>({
    key: undefined,
    isRotation: false,
    wasShowing: false,
  });

  // Keyed on [currentId, currentSignal], never on priority — the actual
  // acceptance criterion this field exists for: a High-priority cmux
  // "needs input" alert (signal: "generic") must never play the goal
  // celebration. Not keyed on `expanded` either, so toggling the manual
  // hotkey on an already-visible item doesn't replay the burst.
  // biome-ignore lint/correctness/useExhaustiveDependencies: currentId is the deliberate re-trigger key documented above — a new item with the same signal must replay the pulse; dropping it would change that behavior.
  useEffect(() => {
    if (currentSignal === "goal") {
      setPulse("pulse-goal");
    } else if (currentSignal === "red_card") {
      setPulse("pulse-red");
    } else {
      setPulse(null);
    }
  }, [currentId, currentSignal]);

  const [liveCelebration, setLiveCelebration] = useState<Celebration>(null);

  // Same [currentId, currentSignal] re-trigger discipline as the pulse
  // effect above — `currentBody` is read inside (not a dependency) because
  // it always arrives paired with currentId/currentSignal on the same slot
  // object; the goal/penalty/own-goal split needs it (see
  // `footballEventKindFor`'s doc), but it can't independently change
  // without a new id, so it isn't a re-trigger key in its own right.
  // biome-ignore lint/correctness/useExhaustiveDependencies: currentBody isn't a re-trigger key (see comment above) — only currentId/currentSignal decide whether to replay.
  useEffect(() => {
    if (!isLiveCard || currentSignal === null || currentBody === null) {
      setLiveCelebration(null);
      return;
    }
    const kind = footballEventKindFor(currentSignal, currentBody);
    setLiveCelebration(kind ? eventKindPresentationFor(kind).celebration : null);
  }, [currentId, currentSignal, isLiveCard]);

  function clearPulseWhenItsAnimationEnds(event: React.AnimationEvent<HTMLDivElement>) {
    if (pulse && event.animationName === PULSE_END_ANIMATION[pulse]) {
      setPulse(null);
    }
    if (liveCelebration && event.animationName === CELEBRATION_END_ANIMATION[liveCelebration]) {
      setLiveCelebration(null);
    }
  }

  // plan 082: weather ALERT cards carry their art derived from the live
  // slot's `wx-*` marker pairs — same live-`slot` basis as `news`/
  // `categoryClass` above (not `renderedSlot`), so the below-block's mood
  // updates in lockstep with every other live-slot-derived class, not
  // delayed by the 220ms content swap. `null` for every non-weather card,
  // so it renders byte-identical to today.
  const wxArt = showing ? weatherArtFromDetails(slot.details) : null;
  // plan 096: the cmux accent's below-block hairline gate — same live-slot
  // basis as `news`/`wxArt` above, for the same lockstep-with-below-block
  // reason. Deliberately NOT part of `cardClass` (the shell): the shell
  // owns the priority accent channel only, and origin must never share
  // that channel (see the CSS comment on `.below-block.cmux-origin`).
  const cmuxOrigin = showing && slot.origin === "cmux";

  // plan 120: `swapKey` also feeds the below-block's AnimatePresence
  // `key` directly (JSX further down), not just `useExitChoreography`'s
  // internal `useDelayedSwap` call — so it stays computed here too,
  // deliberately duplicating the identical one-line derivation the hook
  // now also does internally (same pattern this file already uses for
  // `showing` itself: cheap, pure, recomputed rather than threaded
  // across the hook boundary as an extra return value just for one JSX
  // consumer).
  const swapKey = showing ? slot.id : "idle";

  // plan 127 (Step 3, finding #3): whether the swap THAT LANDED THIS
  // `swapKey` was a same-slot rotation — showing(A)->showing(B) — rather
  // than a promotion (idle->showing) or an exit (showing->idle).
  // `wasShowingRef` holds the previous DISTINCT key's own showing-ness,
  // updated only on the render where `swapKey` actually changes (guarded
  // by comparing against the last key this ref saw) — deliberately NOT
  // unconditional every render: this component re-renders for reasons
  // that have nothing to do with the swap (e.g. the pulse/celebration
  // effects just above call `setPulse`/`setLiveCelebration` right after
  // mount, forcing an immediate second render with the SAME `swapKey`),
  // and an unconditional write would let that unrelated extra render
  // overwrite the ref before this render's own `isRotation` value is
  // even read back on a later actual key change — corrupting the history
  // the very next real transition depends on. Guarding on the key
  // ensures `isRotation` stays STABLE for a given key's entire mounted
  // lifetime (every same-key re-render — the queue-slider tick, the
  // pulse effect, ...) reads the SAME cached value AnimatePresence's
  // `custom` was actually given at the transition, never a stale
  // recomputation.
  // Mutated directly in the render body (not an effect) — the standard,
  // React-sanctioned "remember a previous render's value" idiom (same
  // shape as a hand-rolled `usePrevious`), safe here because the write is
  // deterministic given this render's own `swapKey`/`showing` and has no
  // visible side effect other than being read by a later render.
  // The three-way split falls out for free from just two booleans:
  // idle->showing has the previous key's `wasShowing` false (idle was
  // never "showing"), so `isRotation` is false; showing->idle has the
  // live `showing` itself false, so `isRotation` is false regardless of
  // history; only showing(A)->showing(B), where both are true, yields
  // true. Promotion and exit legs are therefore byte-identical to before
  // this plan — only the true showing->showing case changes at all.
  if (wasShowingRef.current.key !== swapKey) {
    wasShowingRef.current = {
      key: swapKey,
      isRotation: showing && wasShowingRef.current.wasShowing,
      wasShowing: showing,
    };
  }
  const isRotation = wasShowingRef.current.isRotation;

  // plan 120: the showing<->idle exit-choreography state machine —
  // extracted to src/useExitChoreography.ts (see that file for every
  // comment documenting each of these values; moved verbatim, not
  // rewritten). `renderedSlot`/`exiting` (the hook's own intermediate
  // values) are NOT destructured here — every downstream consumer that
  // used to read them directly
  // (geometryPriority/expanded/shellExiting/bare/trueIdle) is itself now
  // a hook output, so nothing in this file needs the raw pair anymore;
  // destructuring them unused would trip tsconfig's `noUnusedLocals`.
  const {
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
  } = useExitChoreography(slot, restingState, hovered);

  // plan 091: the outer shell (`.card-assembly`) now owns ONLY geometry-
  // and-effects classes — priority accent, hover diagnostic, the goal/
  // red-card pulse and the live-match celebrations. `news-shade`/`wx-card`
  // (and their mood/texture riders) move to `belowBlockClass` below: they
  // are content presentation, not shell, and the below-block is the block
  // that actually carries that content now (Step 4's ownership split).
  // The old idle/idle-status width split (plan 034) is gone — the new
  // idle has one width formula regardless of status chips (Geometry
  // contract point 5), so there is no more "status" class to compute here.
  const cardClass = [
    "card-assembly",
    geometryPriority,
    expanded && "expanded",
    // drives the hover "breathing" (scale + deeper drop-shadow,
    // overlay-card.css `.card-assembly.hovered`) off the live `hovered`
    // prop — never CSS `:hover`, since the overlay window is
    // click-through and never receives real pointer events.
    hovered && "hovered",
    // plan 105 (Step C): the bare-notch modifier — transparent flanks,
    // cutout-width-only shell (styles.css), so the mode reads as the
    // native notch until hovered.
    bare && "bare",
    // 2026-07-23 review fix (wave B, Task 1): see `shellExiting`'s own
    // doc above — drives the immediate width-shrink + corner-round start
    // on the true showing->idle exit leg only.
    shellExiting && "exiting",
    // plan 123: see `exitToBare`'s own doc above — only ever paired with
    // `exiting` (never appears alone), so it's a pure narrowing modifier,
    // not a separate state; `restingState === "rail"` never sets it.
    exitToBare && "exit-to-bare",
    // plan 084: `pulse`/`cele-*` are mutually exclusive, never stacked —
    // the live-match branch (structured espn meta) plays its own
    // `cele-goal`/`cele-yc`/`cele-rc`; every other football-signal card
    // (flag-off path, or a non-espn source that happens to share a
    // signal) keeps the shipped pulse-goal/pulse-red exactly as before.
    !isLiveCard && pulse,
    isLiveCard && liveCelebration,
  ]
    .filter(Boolean)
    .join(" ");
  // plan 091: below-block's own class list — the news/weather mood
  // presentation, still derived off the LIVE slot (not `renderedSlot`) for
  // the same "no delayed-swap lag" reason the comment above always gave;
  // only WHERE these classes attach moved (below-block, not the shell).
  const belowBlockClass = [
    "below-block",
    news && "news-shade",
    news && categoryClass(slot.category),
    wxArt && "wx-card",
    wxArt?.moodClass,
    wxArt?.textureClass,
    cmuxOrigin && "cmux-origin",
  ]
    .filter(Boolean)
    .join(" ");

  // plan 12x (wave 2): the swapped card BODY (everything inside
  // `.card-content`, below) now reads the LIVE `slot` directly, like
  // `news`/`wxArt`/`isLiveCard` above — there is no more `renderedSlot`
  // stand-in for content. `AnimatePresence` (in the JSX below) is what
  // now supplies the "outgoing content stays frozen through its own
  // exit" behavior: an exiting `motion.div` keeps whatever it last
  // rendered (captured automatically once its parent stops including
  // it), so freezing content by hand is no longer this component's job.
  // `renderedSlot`/`renderedShowing`/`exiting` (from the STILL-KEPT
  // `useDelayedSwap` above) are now scoped to ONE job only: the plan-107
  // GEOMETRY choreography (`geometryPriority`/`expanded`/`bare` above,
  // and the below-block/StatusDots mount gates below) — never content.
  const newsCategory = news ? categoryLabel(slot.category) : null;
  const newsAge = news ? ageLabel(slot.publishedAtMs, Date.now()) : null;
  // plan 082 marker-leak guard: every `wx-*` pair is a mood/glyph input,
  // never real content — strip it from `details` before it reaches EITHER
  // place details render as visible text (the collapsed loop below and
  // the expanded Manifest). Every non-weather card's `details` has no
  // `wx-*` labels, so this filter is a no-op there — byte-identical.
  const liveVisibleDetails = showing ? visibleDetails(slot.details) : [];

  // plan 069 (folded into 078; re-scoped to live `slot` in wave 2): memoized
  // so unrelated re-renders don't re-tokenize the markdown.
  // 2026-07-23: dependency narrowed from the whole `slot` object to
  // `currentBody` (the actual string fed into `renderInlineMarkdown`,
  // already computed above) — mirrors Manifest.tsx's own `[body]`
  // dependency. `slot` changes on every wire tick (queue counters, TTL
  // countdowns, etc.), which was re-tokenizing this markdown on every one
  // of those emissions even though the body text itself hadn't changed.
  const bodyContent = useMemo(() => renderInlineMarkdown(currentBody ?? ""), [currentBody]);

  // plan 084: the live-match branch — `isLiveCard` (above) already reads
  // the live slot, so it doubles as both the outer shell's accent gate
  // AND the content branch selector below; there is no more a separate
  // delayed/live pair that could "briefly disagree" (wave 2 dropped that
  // window entirely — see the comment above).
  const liveEspn: EspnMeta | undefined = showing ? slot.espn : undefined;
  const footballKind = showing && isLiveCard ? footballEventKindFor(slot.signal, slot.body) : null;
  const eventPresentation = footballKind ? eventKindPresentationFor(footballKind) : null;
  const pillVariant = showing && isLiveCard ? livePillVariantFor(slot.signal) : "live";
  const pillLabel = pillVariant === "break" ? "Break" : pillVariant === "final" ? "Final" : "Live";
  const cardsClean =
    liveEspn !== undefined &&
    liveEspn.homeCards[0] === 0 &&
    liveEspn.homeCards[1] === 0 &&
    liveEspn.awayCards[0] === 0 &&
    liveEspn.awayCards[1] === 0;

  // plan 091: the below-block mounts if and only if `renderedShowing` is
  // true — which, thanks to `useDelayedSwap` freezing `renderedSlot` at
  // its pre-transition value for the whole `exiting` window, already
  // covers BOTH "currently showing" and "exiting FROM showing back to
  // idle". It does NOT need `|| exiting` on top: during the opposite
  // transition (idle exiting INTO showing), renderedSlot is still frozen
  // idle-flavored — there is no below-block content to fade out of idle,
  // because idle never had any (flank-right's dots below play that side
  // of the transition instead, gated on `!renderedShowing`, the
  // mirror-image condition).
  // plan 12x (wave 2): this wrapper's PRESENCE stays on the kept
  // `renderedShowing` (unchanged from plan 091/107) so the swapped
  // content's own `AnimatePresence`, just inside it, always has a parent
  // that outlives its exit animation — `belowBlockClass` itself is still
  // computed off the LIVE slot above, so the mood/texture classes keep
  // updating in lockstep with `slot`, not delayed by the content swap,
  // exactly as before.
  // plan 11x: PRESENCE itself now reads `belowBlockOpen`, not
  // `renderedShowing` directly — identical to `renderedShowing` for
  // every case above (entrance, steady showing, same-id/rotation
  // swaps — see `belowBlockOpen`'s own doc above for why those are
  // unaffected), except the true showing->idle close, which now settles
  // CONTENT_EXIT_MS after `showing` goes false instead of the full
  // SWAP_EXIT_MS. The wrapper is a `motion.div` now (was a plain `div`)
  // so that close can fade rather than snap: `initial={false}` skips any
  // enter animation (below-block still appears at full opacity on the
  // very render it mounts, byte-identical to the old plain `div`), so
  // only ITS OWN exit (below-block visibly clearing) is new — the inner
  // `AnimatePresence`'s content swap just below is completely untouched
  // (same duration, same easing, both directions), so entrance content
  // fade-in and same-priority rotation fades are unaffected; the outer
  // fade's job is only to make sure nothing is left visible when this
  // wrapper actually unmounts CONTENT_EXIT_MS later, so overlay-card.css's
  // `:not(:has(.below-block))` flank corner-round (which can only safely
  // start once the below-block is truly gone — see that rule's ROUNDING
  // LAW comment) begins right after, not a further ~330ms late.
  //
  // plan 12x (wave 3, operator-feedback polish pass): the wrapper's own
  // `exit` variant (JSX below) now also animates `height` (auto -> 0,
  // motion measures the rendered box itself — same implicit-from-value
  // read it already relied on for `opacity`), not opacity alone. Before
  // this, the wrapper kept its full showing-height for the entire
  // CONTENT_EXIT_MS fade, then vanished outright the instant it
  // unmounted — an abrupt height "pop," not a collapse. `.below-block`'s
  // own `overflow: hidden` (overlay-card.css) both clips the shrinking
  // content and gives it an automatic min-height of 0 (CSS Grid: a
  // non-visible-overflow item's auto min-size is 0), so the surrounding
  // `.card-assembly` grid row — and the whole card — shrinks in step,
  // frame by frame, rather than jumping straight to its post-below-block
  // height on unmount. Paired with the inner `AnimatePresence`'s own
  // `exit` fix just below, this also fixes the pre-pop content reflow:
  // previously the INNER content swap's `exit` variant (`{opacity: 0, y:
  // -4}` over SWAP_EXIT_MS) kept animating past this wrapper's own
  // shorter CONTENT_EXIT_MS close, so the wrapper unmounted the whole
  // subtree mid-flight through the inner fade+shift — a visible jump
  // right before the pop. The inner exit is now `{opacity: 0}` (no
  // y-shift — nothing to reflow) over CONTENT_EXIT_MS (matching this
  // wrapper exactly), so the two finish in lockstep: content quietly
  // fades in place while the box collapses around it, one motion, never
  // cut off mid-animation. The inner `animate` (entrance) transition is
  // untouched — still SWAP_EXIT_MS — so promotions and same-priority
  // rotations keep their existing feel; only the true close changed.

  // plan 127 (Step 5, /improve-animations audit finding #6): `role`/
  // `aria-live` moved OFF the card root (see below-block's own JSX for
  // where they landed) — this used to sit on `.card-assembly` itself,
  // which also encloses FlankClock (a 30s-ticking clock) and, for a
  // live-match card, the scorecard's own constantly-updating minute/
  // score chrome, both of which would re-announce to assistive tech on
  // every routine wire tick, not just genuine new-notification arrivals.
  // `liveRegionActive` gates the region to exactly the case that should
  // announce: a non-live-match card (news/generic/cmux/weather ALERT —
  // stable title/body text that only changes on a genuine new item or
  // rotation) that's actually mounted.
  //
  // plan 129 (K2, deep-review fix): the wave above landed this on
  // `belowBlockOpen && !isLiveCard` and put the attributes on the
  // AnimatePresence-keyed `motion.div` that carries `belowBlockClass` —
  // that node MOUNTS at t=175ms (`belowBlockOpen` lags `showing` by the
  // exit-choreography settle) in the SAME commit as the title/body
  // content already inside it. A live region inserted already-populated
  // is the canonical unreliable ARIA pattern — most screen readers only
  // pick up mutations to an already-established region, not a region
  // that arrives pre-filled. Rotations happened to still announce only
  // because `mode="wait"` delays the swapped child's own mount past the
  // point where the region (on the old placement) had already flipped
  // on with the FIRST item's content — the region existed early only by
  // accident of a later render, not by design. The fix: gate on
  // `showing` (this component's own live boolean, flips the instant the
  // slot enters/leaves "showing" — t=0, no exit-choreography lag)
  // instead of `belowBlockOpen`, and move the attributes onto a NEW,
  // always-mounted static wrapper one level up (see the JSX below) that
  // exists before AND after the AnimatePresence-keyed content node ever
  // mounts — so the attribute-flip and the content-mount are back in
  // two different commits, exactly like the pre-127 root-level pattern
  // this doc describes above (`role`/`aria-live` flip at t=0, content
  // arrives later into an already-established region). `isLiveCard` is
  // itself derived off the live `slot`, gated on `showing` — stays in
  // sync with it; no staleness window between the two.
  const liveRegionActive = showing && !isLiveCard;

  return (
    <div className={cardClass} onAnimationEnd={clearPulseWhenItsAnimationEnds}>
      {/* 2026-07-23 review fix (wave B, Task 2 — real concave S-curve):
          the top "gill" corners — real DOM siblings of the flanks, NOT
          `.card-assembly::before`/`::after` (both already claimed by the
          goal-celebration burst/ring — see those rules in
          overlay-card.css) and NOT pseudo-elements on the flanks
          themselves (`.flank-left`/`.flank-right` have `overflow:
          hidden`, which would clip anything anchored outside their own
          box — these gills must poke a few px past the flank's outer
          edge to read as a flare, so they need `.card-assembly` itself,
          which has no overflow rule, as their positioning ancestor).
          Always rendered ("always render, CSS decides" idiom, matches
          `.synthetic-cutout` below); pure decoration
          (`aria-hidden` + CSS `pointer-events: none`). See
          overlay-card.css's own comment for the concave-fillet geometry. */}
      <span className="notch-gill notch-gill-left" aria-hidden="true" />
      <span className="notch-gill notch-gill-right" aria-hidden="true" />
      <div className="flank-left">
        {/* plan 105 (Step C): bare mode draws no clock — CSS alone can't
            hide it (the flanks going transparent still leaves text
            painted), so this is a real render-time gate, unlike the
            synthetic-cutout's "always render, CSS decides" idiom below.
            2026-07-23 (operator minimal-notch spec, Task 1.2): gate moved
            from `!bare` to `railRevealed` — a bare-hover now mounts the
            clock too (expanding the minimal notch into the full idle
            rail), fading in via `AnimatePresence`/`motion` rather than
            popping, coordinated with the width growth CSS already drives
            on `.card-assembly.bare:has(.idle-peek)` (overlay-card.css).
            Every other case (`!bare` was already true) is unaffected —
            `railRevealed` is true throughout showing/exiting exactly as
            `!bare` was, so this is a pure addition, not a behavior
            change, off bare-idle.
            plan 124 (F3, review fix): `&& !exitToBare` added. During the
            exit-to-bare window (`useExitChoreography.ts`'s `exitToBare`
            doc has the full mechanism) the flank paint itself animates to
            transparent over overlay-card.css's `.exiting.exit-to-bare`
            rule, but this mount was gated on `railRevealed` alone — true
            for that entire window (bare is false throughout showing/
            exiting) — so the clock stayed mounted fully opaque while its
            background faded out from under it: white text sitting on a
            see-through flank mid-window. Unmounting it the instant
            `exitToBare` goes true lets its own 260ms exit fade (below)
            overlap the flank's fade instead of lagging a full render
            behind AnimatePresence's own exit trigger. Every other case is
            unaffected: `exitToBare` is always false in rail mode (its own
            doc pins that), so this is a pure narrowing on the notch-mode
            exit leg only. */}
        <AnimatePresence>
          {railRevealed && !exitToBare && (
            <motion.span
              key="flank-clock"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              // plan 127 (Step 1, /improve-animations audit findings #4/
              // #9): was a hand-typed `{ duration: 0.26, ease: "easeOut" }`
              // — now single-sourced off REVEAL_MS (the same reveal/paint
              // coordination duration the flank background/padding fade
              // and `.track span` background fade use, overlay-card.css)
              // and NOTCHTAP_EASE (matching the flank paint's own
              // `--ease-notchtap`, which this fade is coupled to — both
              // read the exact same bare<->hovered/reveal trigger, so
              // they should ease identically, not one bezier and one
              // built-in "easeOut").
              transition={{ duration: REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              <FlankClock />
            </motion.span>
          )}
        </AnimatePresence>
      </div>
      {/* plan 091: the notch cutout itself — real hardware empty space in
          notch mode (nothing painted here), an app-drawn pure-#000 block
          in HUD mode (`:root[data-notchtap-mode="hud"] .synthetic-cutout`,
          styles.css). Always rendered; CSS alone decides whether it
          paints, so there is no mode branch in this component (Decision
          6 — "no mode branch" in the shape itself). */}
      <div className="synthetic-cutout" aria-hidden="true" />
      {/* the idle face — purely additive decoration in the same grid cell
          as .synthetic-cutout above (grid-column 2 / grid-row 1,
          overlay-card.css); it owns none of the geometry/swap machinery,
          only reads it via `trueIdle`. Gated on `idleFaceEligible`
          (2026-07-23 review fix): CSS never paints it on real notch
          hardware, so it's not rendered at all there — otherwise its
          internal reveal/gaze/blink timers would run forever for a node
          that can never be seen. */}
      {idleFaceEligible && <IdleFace idle={trueIdle} />}
      <div className="flank-right">
        {/* plan 091: StatusDots was idle-only furniture — mounted whenever
            idle-flavored content should be visible, fading out over the
            SWAP_EXIT_MS delayed-swap window on the idle->showing leg (plan
            105 Step C added the `!bare` half: rail furniture, not part of
            the "looks like a native notch" bare state).
            2026-07-23 (operator minimal-notch spec, Task 1.2 + 1.3,
            operator-requested behavior change): gate is now `railRevealed`
            alone — the `!renderedShowing` half is REMOVED. The operator's
            spec (`⟨time⟩⟨minimal⟩⟨dots⟩` above `⟨compact⟩` above
            `⟨expanded⟩`) wants the dots to stay visible as constant rail
            furniture THROUGH a showing notification, not hide the instant
            one is promoted — "the top row stays the full rail" for both
            configs. `railRevealed` already covers the bare-hover reveal
            (Task 1.2) for free, since it's `!bare || hovered` and `bare`
            is always false while genuinely showing/exiting. Now wrapped in
            `AnimatePresence` (previously unnecessary — the old `!bare`-
            gated mount only ever toggled off `bare`, which never combined
            with `exiting`'s manual opacity dance below) so mount/unmount
            still animates smoothly on the one remaining toggle: bare
            <-> bare-hovered. Steady rail mode (`bare` always false) never
            triggers this AnimatePresence exit/enter at all — the node
            just stays mounted across idle<->showing, so the dots read as
            one continuous shape, not a fade replay on every promotion.
            plan 124 (F3, review fix): `&& !exitToBare` added — same
            mismatch and same fix as FlankClock's own doc just above:
            `railRevealed` alone stayed true through the whole exit-to-bare
            window (bare is false throughout showing/exiting), so the dots
            sat fully opaque while the flank painted underneath them faded
            to transparent. Unmounting on `exitToBare` lets this node's own
            260ms exit fade overlap the flank's fade instead of trailing
            it. Rail mode is unaffected (`exitToBare` always false there —
            "steady rail mode" above still never triggers this
            AnimatePresence exit/enter). */}
        <AnimatePresence>
          {railRevealed && !exitToBare && (
            <motion.div
              key="status-dots"
              className="card-content idle"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              // plan 127 (Step 1, /improve-animations audit findings #4/
              // #9): was a hand-typed `{ duration: 0.26, ease: "easeOut" }`
              // — now single-sourced off REVEAL_MS (the same reveal/paint
              // coordination duration the flank background/padding fade
              // and `.track span` background fade use, overlay-card.css)
              // and NOTCHTAP_EASE (matching the flank paint's own
              // `--ease-notchtap`, which this fade is coupled to — both
              // read the exact same bare<->hovered/reveal trigger, so
              // they should ease identically, not one bezier and one
              // built-in "easeOut").
              transition={{ duration: REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              <StatusDots status={status} />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      {/* plan 093 (079 items 9/17/18): the idle hover-expanded state —
          `open` is gated on `renderedShowing` (not the live `showing`)
          for the same reason `StatusDots` above is: it must stay in step
          with the delayed-swap settle, not flicker on/off mid-transition.
          Driven by the live `hovered` prop, never CSS `:hover`.
          plan 127 (Step 2, finding #2): ALWAYS rendered now — the old
          `{!renderedShowing && <IdleHoverPeek .../>}` conditional
          unmounted this component (its internal AnimatePresence
          included) the instant a promotion arrived mid-peek, tearing out
          up to 100px of content with zero animation. The mount gate
          moved INSIDE IdleHoverPeek as its own `open` prop (same
          `!renderedShowing && hovered` condition, just evaluated one
          level deeper) so a promotion now lets IdleHoverPeek's own exit
          animation play while the card content mounts above it, instead
          of both changes landing as one unanimated swap. See
          IdleHoverPeek.tsx's own doc on `open` for the full mechanism. */}
      <IdleHoverPeek status={status} hovered={hovered} open={!renderedShowing && hovered} />
      {/* plan 129 (K2, deep-review fix): this `display: contents` div is
          the ACTUAL live-region wrapper now — `liveRegionActive`'s own
          doc (above) has the full mechanism. It is a plain, always-
          mounted static element (never conditionally rendered, unlike
          everything it wraps), so `role`/`aria-live` land in the DOM on
          the same render `showing` itself flips, before the
          AnimatePresence-keyed `belowBlockOpen` content below ever
          mounts — a real screen reader sees an EMPTY, already-live
          region first, then a mutation into it ~175ms later, never a
          region that arrives pre-populated. `display: contents` means
          this node contributes nothing to layout or the box tree (no
          new flex/grid item, no new stacking context) — `.card-assembly`
          (a CSS grid) still sees straight through to the `.below-block`
          motion.div as its direct box-participating child, and
          `overlay-card.css`'s `:not(:has(.below-block))` flank-rounding
          law still matches/misses exactly as before, since `.below-block`
          itself stays on the same node, just with one more non-
          participating ancestor between it and `.card-assembly`. */}
      <div
        style={{ display: "contents" }}
        role={liveRegionActive ? "status" : undefined}
        aria-live={liveRegionActive ? "polite" : undefined}
      >
        <AnimatePresence>
          {belowBlockOpen && (
            <motion.div
              className={belowBlockClass}
              // plan 127 (Step 5, finding #6): this used to be the STATIC
              // live-region wrapper `liveRegionActive`'s doc refers to —
              // it mounts once per showing session (gated on
              // `belowBlockOpen`, not `swapKey`) and stays mounted through
              // every same-session rotation, so it was NOT the
              // AnimatePresence-keyed node (the inner `motion.div key=
              // {swapKey}` just below) that remounts per swap and would
              // re-announce its ENTIRE content as a brand-new region on
              // every rotation rather than reporting a content update
              // within a stable one. Title/body text changes from a
              // rotation still reach assistive tech exactly as before —
              // aria-live watches for DOM mutations anywhere in its
              // subtree, not just at its own root — so this was a strict
              // narrowing of WHAT can trigger an announcement (excludes
              // FlankClock/StatusDots, structurally outside this wrapper,
              // and the live-match branch via `liveRegionActive`'s own
              // `!isLiveCard` gate), never a loss of the rotation-announce
              // behavior itself.
              //
              // plan 129 (K2): the ROLE/ARIA-LIVE ATTRIBUTES themselves
              // moved OFF this node — this node still mounts at
              // t=175ms (`belowBlockOpen` lags `showing`), so leaving the
              // attributes here reintroduces the exact pre-populated-
              // region bug this comment above used to describe as fixed.
              // The narrowing-of-what-can-announce rationale above still
              // holds (rotations still reach assistive tech via DOM
              // mutation inside the now-outer live region, live-match
              // chrome is still excluded via `liveRegionActive`'s
              // `!isLiveCard` gate) — only WHICH element carries the
              // attributes changed, to the static wrapper just above.
              initial={false}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              {/* plan 082: the condition glyph — a background-layer image,
              same z-order tier as .news-shade::before (behind
              .compact/.manifest, which the CSS below lifts to z-index 1).
              Live-slot-derived, like belowBlockClass's mood/texture
              classes above, so it never waits on the content swap. */}
              {wxArt && <img className="wx-icon" src={wxArt.glyphUrl} alt="" />}
              {/* plan 12x (wave 2): the actual content-swap animation — was a
              hand-rolled `useDelayedSwap` freeze + CSS
              `card-enter-showing`/`card-exit-showing` keyframes, now real
              `AnimatePresence mode="wait"`. Keyed on the LIVE `swapKey`
              (unchanged: `slot.id` while showing) — a same-id update
              (e.g. a queue-counter tick) re-renders this SAME node in
              place, no key change, no exit/enter replay, no remount
              (pinned by the "updates the queue slider... without
              remounting" test). A genuine id change (showing(A)->
              showing(B)) — or `showing` itself going false, which yields
              no child at all here — drops the old key; `AnimatePresence`
              freezes whatever that child last rendered and plays its
              `exit` variant, and `mode="wait"` holds any new child back
              until that finishes. That's exactly what the old freeze
              timer did, now framework-owned — which is also why content
              below reads the LIVE `slot` directly (see the comment above
              `newsCategory`) rather than a frozen stand-in: the freeze is
              `AnimatePresence`'s job now.
              plan 127 (Step 3, finding #3): `custom={isRotation}` on this
              `AnimatePresence` is what lets the EXITING child (the OLD
              `swapKey`, already dropped out of the JSX below by the time
              this render commits) still learn whether ITS OWN removal is
              a rotation — see `contentExitVariants`' own doc for why this
              is the only channel available for that. Promotion
              (idle->showing) and exit (showing->idle) always pass
              `isRotation: false`, so this AnimatePresence's behavior for
              those two legs is unchanged. */}
              <AnimatePresence mode="wait" custom={isRotation}>
                {showing && (
                  <motion.div
                    key={swapKey}
                    className="card-content"
                    // plan 127 (Step 3): a showing->showing rotation skips
                    // the y-slide entirely (opacity-only) — the slide is
                    // the part of the ceremony that reads as repetitive on
                    // a ~10s cadence; the idle->showing promotion keeps its
                    // slide, byte-identical to before this plan.
                    initial={isRotation ? { opacity: 0 } : { opacity: 0, y: -4 }}
                    animate={isRotation ? { opacity: 1 } : { opacity: 1, y: 0 }}
                    // `data-rotation-swap` is a real DOM attribute, not
                    // pure decoration: it's how the test suite pins this
                    // leg-detection logic (motion's own transition/variant
                    // props aren't otherwise inspectable from rendered
                    // output in jsdom) — see StatusRailCard.test.tsx's
                    // "same-slot rotation" describe block.
                    data-rotation-swap={isRotation}
                    // plan 12x (wave 3, exit) / plan 127 (Step 3, rotation
                    // split): the exit variant carries its OWN `transition`
                    // (overriding the shared one below, motion's documented
                    // per-variant override mechanism) — see the wrapper's
                    // own doc comment above for why the non-rotation exit
                    // must match CONTENT_EXIT_MS, not SWAP_EXIT_MS.
                    // `exit="exit"` (a variant label, not an inline object)
                    // is what lets `contentExitVariants`' function read the
                    // AnimatePresence-supplied `custom` above — see that
                    // constant's own doc.
                    variants={contentExitVariants}
                    exit="exit"
                    transition={{
                      duration: (isRotation ? ROTATION_ENTER_MS : SWAP_EXIT_MS) / 1000,
                      ease: NOTCHTAP_EASE,
                    }}
                  >
                    {isLiveCard && liveEspn !== undefined ? (
                      <LiveMatchScorecard
                        liveEspn={liveEspn}
                        pillVariant={pillVariant}
                        pillLabel={pillLabel}
                        eventPresentation={eventPresentation}
                        cardsClean={cardsClean}
                        body={slot.body}
                      />
                    ) : (
                      <NotificationBody
                        news={news}
                        slot={slot}
                        newsCategory={newsCategory}
                        newsAge={newsAge}
                        bodyContent={bodyContent}
                        expanded={expanded}
                        liveVisibleDetails={liveVisibleDetails}
                        hovered={hovered}
                      />
                    )}
                  </motion.div>
                )}
              </AnimatePresence>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      {/* the goal celebration is plan 023's pure-CSS confetti burst +
          ring on `.card-assembly.pulse-goal`'s ::after/::before PLUS plan
          032's ripple: three staggered concentric accent rings, mounted
          only while the goal pulse is live and unmounted by the same
          animationend path that clears the burst (goal-signal only,
          one-shot — never keyed on priority). */}
      {!isLiveCard && pulse === "pulse-goal" && (
        <div className="cele-ripple" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
      )}
    </div>
  );
}
