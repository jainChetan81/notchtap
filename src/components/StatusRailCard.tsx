import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { CONTENT_EXIT_MS, NOTCHTAP_EASE, SWAP_EXIT_MS } from "../animationTiming";
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
import { presentationFacts } from "../lib/presentationFacts";
import { weatherArtFor } from "../lib/weatherArt";
import { useDelayedSwap } from "../useDelayedSwap";
import type { EspnMeta, SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import { IdleFace } from "./IdleFace";
import { IdleHoverPeek } from "./IdleHoverPeek";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import { StatusDots } from "./StatusDots";
import { Track } from "./Track";
import { TtlBar } from "./TtlBar";

// plan 084: the recurring live-match scorecard's crest — a filesystem path
// on the wire (083 workstream a), never a ready `asset://` URL, so every
// render must go through `convertFileSrc` itself. `onError` is defense in
// depth for a cache entry that's gone stale on disk between poll and
// render; the `broken` flag is deliberately sticky (not re-tried) so a
// permanently-404ing path doesn't flash between the two states forever.
function Crest({ abbrev, path }: { abbrev: string; path: string | null }) {
  const [broken, setBroken] = useState(false);
  const src = !broken && path !== null ? convertFileSrc(path) : null;
  return (
    <span className="crest">
      {src !== null ? <img src={src} alt="" onError={() => setBroken(true)} /> : abbrev}
    </span>
  );
}

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

type Detail = { label: string; value: string };

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
  const bodyContent = useMemo(
    () => renderInlineMarkdown(showing ? slot.body : ""),
    [slot, showing],
  );

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

  return (
    <div
      className={cardClass}
      role={showing ? "status" : undefined}
      aria-live={showing ? "polite" : undefined}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
      <div className="flank-left">
        {/* plan 105 (Step C): bare mode draws no clock — CSS alone can't
            hide it (the flanks going transparent still leaves text
            painted), so this is a real render-time gate, unlike the
            synthetic-cutout's "always render, CSS decides" idiom below. */}
        {!bare && <FlankClock />}
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
        {/* plan 091: StatusDots is idle-only furniture — mounted whenever
            idle-flavored content should be visible, so the dots fade out
            over the same SWAP_EXIT_MS window the old idle content did, on
            the idle->showing leg of the transition. No `key` needed:
            unlike below-block's content, there is only ever one "flavor"
            of dots, so a plain mount/unmount (no forced remount) is
            enough — and no `AnimatePresence` either, since this node's
            OWN mount/unmount is already externally timed by the
            `renderedShowing`/`bare` condition it's wrapped in (kept
            `useDelayedSwap`, plan 107's geometry timer).
            plan 12x (wave 2): the fade itself is now a `motion.div`
            controlled by `exiting`, replacing the old CSS `.swap-exit`
            class + `card-enter-idle`/`card-exit-idle` keyframes.
            plan 105 (Step C): also gated on `!bare` — the dots are rail
            furniture, not part of the "looks like a native notch" bare
            state. */}
        {!renderedShowing && !bare && (
          <motion.div
            className="card-content idle"
            initial={{ opacity: 0 }}
            animate={{ opacity: exiting ? 0 : 1 }}
            transition={{ duration: SWAP_EXIT_MS / 1000, ease: "easeOut" }}
          >
            <StatusDots status={status} />
          </motion.div>
        )}
      </div>
      {/* plan 093 (079 items 9/17/18): the idle hover-expanded state —
          gated on `renderedShowing` (not the live `showing`) for the same
          reason `StatusDots` above is: it must stay in step with the
          delayed-swap settle, not flicker on/off mid-transition. Driven
          by the live `hovered` prop, never CSS `:hover`. */}
      {!renderedShowing && <IdleHoverPeek status={status} hovered={hovered} />}
      <AnimatePresence>
        {belowBlockOpen && (
          <motion.div
            className={belowBlockClass}
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
              `AnimatePresence`'s job now. */}
            <AnimatePresence mode="wait">
              {showing && (
                <motion.div
                  key={swapKey}
                  className="card-content"
                  initial={{ opacity: 0, y: -4 }}
                  animate={{ opacity: 1, y: 0 }}
                  // plan 12x (wave 3): the exit variant carries its OWN
                  // `transition` (overriding the shared one below, motion's
                  // documented per-variant override mechanism) — see the
                  // wrapper's own doc comment above for why this must match
                  // CONTENT_EXIT_MS, not SWAP_EXIT_MS, on the exit side only.
                  exit={{
                    opacity: 0,
                    transition: { duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE },
                  }}
                  transition={{ duration: SWAP_EXIT_MS / 1000, ease: NOTCHTAP_EASE }}
                >
                  {isLiveCard && liveEspn !== undefined ? (
                    // plan 084: the recurring live-match scorecard (POST-083 espn
                    // meta) — sticky medium-priority presence, no full-expand
                    // (operator lock). Deliberately ignores `expanded`: even if
                    // the slot's `expanded` flag arrives true, there is no
                    // manual-expand affordance for football, so this branch
                    // always renders this same compact scorecard rather than
                    // switching to a richer layout. No `Track` (a batch-position
                    // slider is meaningless for a single recurring presence —
                    // prototype lock) and no `TtlBar` either: the bar's
                    // countdown-to-rotation framing would visually contradict
                    // "sticky" (see plan 084's report for the reasoning). No
                    // generic `<Stamp>` — the live chip above already carries
                    // that role (Live/Break/Final) with more precision.
                    <div className="notif-block">
                      <div className="sc-head">
                        <span className="chip chip-league">{liveEspn.league}</span>
                        <span
                          className={`chip chip-live${pillVariant === "live" ? "" : ` ${pillVariant}`}`}
                        >
                          {pillVariant !== "final" && <span className="live-dot" />}
                          {pillLabel}
                        </span>
                        <span className="clock-pill">{liveEspn.clock}</span>
                      </div>
                      <div className="score-row">
                        <div className="side">
                          <Crest abbrev={liveEspn.homeAbbrev} path={liveEspn.homeCrest} />
                        </div>
                        <span className="score">
                          {liveEspn.homeScore}
                          <span className="dash">–</span>
                          {liveEspn.awayScore}
                        </span>
                        <div className="side">
                          <Crest abbrev={liveEspn.awayAbbrev} path={liveEspn.awayCrest} />
                        </div>
                      </div>
                      <div
                        className={`event-line${eventPresentation?.tintClass ? ` ${eventPresentation.tintClass}` : ""}`}
                      >
                        {eventPresentation && <span className={eventPresentation.iconClass} />}
                        {slot.body}
                      </div>
                      {!cardsClean && (
                        <div className="cards-line">
                          {liveEspn.homeAbbrev} {liveEspn.homeCards[0]}Y{liveEspn.homeCards[1]}R ·{" "}
                          {liveEspn.awayAbbrev} {liveEspn.awayCards[0]}Y{liveEspn.awayCards[1]}R
                        </div>
                      )}
                    </div>
                  ) : (
                    <>
                      <div className="compact">
                        <div className="copy">
                          {news ? (
                            // plan 092 (item 19 + 080 carry-forward): the shipped
                            // news layout stays screenshot-faithful (masthead,
                            // headline, WIRE stamp, news-shade, track) — only the
                            // Stamp badge's position (now inline with the
                            // masthead, `.masthead-row`) and the pills' visual
                            // vocabulary (chip-converged, item 10) change. Age
                            // moves out of the meta row entirely into the plain
                            // `.notif-time-inline` slot (Decision 5 — same
                            // ageLabel computation/thresholds, new location).
                            // plan 110 (Step C): the redundant `.pub-meta`
                            // "published HH:MM" node is gone — the compact row
                            // now carries exactly one time expression (the
                            // relative age above). The expanded Manifest's own
                            // "published HH:MM" segment is untouched (its own
                            // pinned test lives in StatusRailCard.test.tsx).
                            <>
                              <div className="masthead-row">
                                <div className="masthead">
                                  <span className="dot" />
                                  {slot.source ?? "RSS"}
                                </div>
                                <Stamp
                                  priority={slot.priority}
                                  signal={slot.signal}
                                  eventType={slot.eventType}
                                />
                              </div>
                              <div className="title headline">{slot.title}</div>
                              {(newsCategory !== null || newsAge !== null) && (
                                <div className="notif-meta-row">
                                  {newsCategory !== null && (
                                    <span className="chip chip-category">{newsCategory}</span>
                                  )}
                                  {newsAge !== null && (
                                    <span className="notif-time-inline">{newsAge}</span>
                                  )}
                                </div>
                              )}
                            </>
                          ) : (
                            // plan 092 (item 19, this plan's core): the general
                            // card's header row (title + the badge cluster) +
                            // subtitle row (plan 035's `subtitle`, surfaced in
                            // compact for the first time) + full-width clamped
                            // body. There is no inline-time value here (no
                            // non-news event carries a publishedAtMs), so the
                            // subtitle row's time slot simply never renders.
                            // plan 096: the badge cluster is the priority Stamp
                            // PLUS the cmux chip, conditional on `origin` (now on
                            // the wire — 092 deferred this exact spot pending
                            // that wire change).
                            <>
                              <div className="notif-header-row">
                                <span className="notif-title">{slot.title}</span>
                                <div className="notif-header-badges">
                                  {slot.origin === "cmux" && (
                                    <span className="chip chip-cmux">Agent</span>
                                  )}
                                  <Stamp
                                    priority={slot.priority}
                                    signal={slot.signal}
                                    eventType={slot.eventType}
                                  />
                                </div>
                              </div>
                              {slot.subtitle !== null && (
                                <div className="notif-subtitle-row">
                                  <span className="notif-subtitle">{slot.subtitle}</span>
                                </div>
                              )}
                              <div className="notif-body">{bodyContent}</div>
                              {/* plan 042: collapsed scorecard cells (Clock,
                                per-side Cards) — only a live-match card with
                                `espn_live_card` on populates `details`, so
                                every other card renders exactly as before.
                                Same detail-label/detail-value classes as the
                                expanded Manifest view; collapsed-only, so the
                                pairs never render twice when expanded. */}
                              {!expanded &&
                                liveVisibleDetails.length > 0 &&
                                liveVisibleDetails.map((detail) => (
                                  <div key={`${detail.label}:${detail.value}`}>
                                    <div className="detail-label">{detail.label}</div>
                                    <div className="detail-value">{detail.value}</div>
                                  </div>
                                ))}
                            </>
                          )}
                        </div>
                        {!expanded && (
                          <div className="compact-hint">
                            <kbd>⌃⇧N</kbd> more
                          </div>
                        )}
                        <Track total={slot.queueTotal} done={slot.queueDone} />
                      </div>
                      <Manifest
                        body={slot.body}
                        eventType={slot.eventType}
                        expanded={expanded}
                        source={slot.source}
                        category={slot.category}
                        publishedAtMs={slot.publishedAtMs}
                        hasLink={slot.link !== null}
                        subtitle={slot.subtitle}
                        details={liveVisibleDetails}
                      />
                      {/* plan 100: last in DOM order within .below-block — the bar
                        is the card's floor, absolutely positioned to its bottom
                        edge (styles.css), clipped to the rounded corners by
                        .below-block's own overflow: hidden. */}
                      <TtlBar
                        key={slot.id}
                        slotId={slot.id}
                        ttlMs={slot.ttlMs}
                        remainingMs={slot.remainingMs}
                        // plan 093: TTL hover-pause — this bar only ever mounts
                        // while `showing`, so `hovered` alone (the live cursor
                        // signal) is exactly "is THIS card hovered right now,"
                        // no extra gating needed.
                        hoverPaused={hovered}
                      />
                    </>
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        )}
      </AnimatePresence>
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
