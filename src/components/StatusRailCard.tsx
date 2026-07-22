import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { SWAP_EXIT_MS } from "../animationTiming";
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
import { useDelayedSwap } from "../useDelayedSwap";
import type { EspnMeta, SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
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
  // above are computed (not the delayed-swap `renderedSlot`), so the
  // pulse-vs-celebration gate below always reflects the arriving item.
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
  // plan 078: the idle/showing swap (formerly AnimatePresence mode="wait")
  // freezes the outgoing item via useDelayedSwap while the CSS exit
  // animation runs, then swaps content and key together. Everything that
  // was inside the old swapped motion.div reads from `renderedSlot`;
  // only cardClass (outer div) and the pulse celebration stay on live
  // slot/status, exactly as they did before. Hoisted above `cardClass`
  // (plan 105) so `bare`, below, can feed into it — `renderedShowing`/
  // `exiting` are needed before the class list is built, not after.
  const swapKey = showing ? slot.id : "idle";
  const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, SWAP_EXIT_MS);
  const renderedShowing = renderedSlot.state === "showing";

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

  const renderedExpanded = renderedShowing && renderedSlot.expanded;
  const renderedNews = renderedShowing && renderedSlot.eventType === "news_item";
  const renderedNewsCategory = renderedNews ? categoryLabel(renderedSlot.category) : null;
  const renderedNewsAge = renderedNews ? ageLabel(renderedSlot.publishedAtMs, Date.now()) : null;
  // plan 082 marker-leak guard: every `wx-*` pair is a mood/glyph input,
  // never real content — strip it from `details` before it reaches EITHER
  // place details render as visible text (the collapsed loop below and
  // the expanded Manifest). Every non-weather card's `details` has no
  // `wx-*` labels, so this filter is a no-op there — byte-identical.
  const renderedVisibleDetails = renderedShowing ? visibleDetails(renderedSlot.details) : [];

  // plan 069 (folded into 078): memoized on the rendered slot so unrelated
  // re-renders don't re-tokenize the markdown.
  const bodyContent = useMemo(
    () => renderInlineMarkdown(renderedSlot.state === "showing" ? renderedSlot.body : ""),
    [renderedSlot],
  );

  // plan 084: the live-match branch reads off the DELAYED-SWAP value, like
  // every other piece of rendered content above — the outer shell's
  // isLiveCard (used only to gate the pulse/celebration classes) stays on
  // the live `slot` on purpose, so the two can briefly disagree during the
  // 220ms swap-exit window, exactly like `news` vs `renderedNews` above.
  const renderedEspn: EspnMeta | undefined = renderedShowing ? renderedSlot.espn : undefined;
  const isRenderedLiveCard = renderedEspn !== undefined;
  const renderedFootballKind =
    isRenderedLiveCard && renderedShowing
      ? footballEventKindFor(renderedSlot.signal, renderedSlot.body)
      : null;
  const renderedEventPresentation = renderedFootballKind
    ? eventKindPresentationFor(renderedFootballKind)
    : null;
  const renderedPillVariant =
    isRenderedLiveCard && renderedShowing ? livePillVariantFor(renderedSlot.signal) : "live";
  const renderedPillLabel =
    renderedPillVariant === "break" ? "Break" : renderedPillVariant === "final" ? "Final" : "Live";
  const renderedCardsClean =
    renderedEspn !== undefined &&
    renderedEspn.homeCards[0] === 0 &&
    renderedEspn.homeCards[1] === 0 &&
    renderedEspn.awayCards[0] === 0 &&
    renderedEspn.awayCards[1] === 0;

  // plan 091: the below-block mounts if and only if `renderedShowing` is
  // true (used directly in the JSX below, not through an extra alias, so
  // TypeScript's control-flow narrowing of `renderedSlot` inside that
  // branch stays exactly as reliable as every other `renderedShowing`
  // check already in this file) — which, thanks to useDelayedSwap
  // freezing `renderedSlot` at its pre-transition value for the whole
  // `exiting` window, already covers BOTH "currently showing" and
  // "exiting FROM showing back to idle" (renderedSlot stays showing-
  // flavored throughout that fade). It does NOT need `|| exiting` on top:
  // during the opposite transition (idle exiting INTO showing),
  // renderedSlot is still frozen idle-flavored — there is no below-block
  // content to fade out of idle, because idle never had any (flank-
  // right's dots below play that side of the transition instead, gated
  // on `!renderedShowing`, the mirror-image condition). Exit animations
  // still work: the existing `card-content` key/idle/swap-exit mechanics
  // are byte-identical, just relocated — `key={swapKey}` still forces a
  // fresh remount (and a replayed enter/exit) on every
  // showing(A)->showing(B) transition, since below-block itself doesn't
  // remount for that case (renderedShowing stays true throughout), only
  // its keyed child does.

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
      <div className="flank-right">
        {/* plan 091: StatusDots is idle-only furniture — mirrors the old
            IdleView's single `card-content` swap wrapper (same classes,
            same keyframes) so the dots fade out over the same 220ms
            window the old idle content did, on the idle->showing leg of
            the transition. No `key` needed: unlike below-block's content,
            there is only ever one "flavor" of dots, so a plain mount/
            unmount (no forced remount) is enough.
            plan 105 (Step C): also gated on `!bare` — the dots are rail
            furniture, not part of the "looks like a native notch" bare
            state. */}
        {!renderedShowing && !bare && (
          <div className={`card-content idle${exiting ? " swap-exit" : ""}`}>
            <StatusDots status={status} />
          </div>
        )}
      </div>
      {/* plan 093 (079 items 9/17/18): the idle hover-expanded state —
          gated on `renderedShowing` (not the live `showing`) for the same
          reason `StatusDots` above is: it must stay in step with the
          delayed-swap settle, not flicker on/off mid-transition. Driven
          by the live `hovered` prop, never CSS `:hover`. */}
      {!renderedShowing && <IdleHoverPeek status={status} hovered={hovered} />}
      {renderedShowing && (
        <div className={belowBlockClass}>
          {/* plan 082: the condition glyph — a background-layer image,
              same z-order tier as .news-shade::before (behind
              .compact/.manifest, which the CSS below lifts to z-index 1).
              Live-slot-derived, like belowBlockClass's mood/texture
              classes above, so it never waits on the delayed content
              swap. */}
          {wxArt && <img className="wx-icon" src={wxArt.glyphUrl} alt="" />}
          <div key={swapKey} className={`card-content${exiting ? " swap-exit" : ""}`}>
            {isRenderedLiveCard && renderedEspn !== undefined ? (
              // plan 084: the recurring live-match scorecard (POST-083 espn
              // meta) — sticky medium-priority presence, no full-expand
              // (operator lock). Deliberately ignores `renderedExpanded`:
              // even if the slot's `expanded` flag arrives true, there is
              // no manual-expand affordance for football, so this branch
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
                  <span className="chip chip-league">{renderedEspn.league}</span>
                  <span
                    className={`chip chip-live${renderedPillVariant === "live" ? "" : ` ${renderedPillVariant}`}`}
                  >
                    {renderedPillVariant !== "final" && <span className="live-dot" />}
                    {renderedPillLabel}
                  </span>
                  <span className="clock-pill">{renderedEspn.clock}</span>
                </div>
                <div className="score-row">
                  <div className="side">
                    <Crest abbrev={renderedEspn.homeAbbrev} path={renderedEspn.homeCrest} />
                  </div>
                  <span className="score">
                    {renderedEspn.homeScore}
                    <span className="dash">–</span>
                    {renderedEspn.awayScore}
                  </span>
                  <div className="side">
                    <Crest abbrev={renderedEspn.awayAbbrev} path={renderedEspn.awayCrest} />
                  </div>
                </div>
                <div
                  className={`event-line${renderedEventPresentation?.tintClass ? ` ${renderedEventPresentation.tintClass}` : ""}`}
                >
                  {renderedEventPresentation && (
                    <span className={renderedEventPresentation.iconClass} />
                  )}
                  {renderedSlot.body}
                </div>
                {!renderedCardsClean && (
                  <div className="cards-line">
                    {renderedEspn.homeAbbrev} {renderedEspn.homeCards[0]}Y
                    {renderedEspn.homeCards[1]}R · {renderedEspn.awayAbbrev}{" "}
                    {renderedEspn.awayCards[0]}Y{renderedEspn.awayCards[1]}R
                  </div>
                )}
              </div>
            ) : (
              <>
                <div className="compact">
                  <div className="copy">
                    {renderedNews ? (
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
                            {renderedSlot.source ?? "RSS"}
                          </div>
                          <Stamp
                            priority={renderedSlot.priority}
                            signal={renderedSlot.signal}
                            eventType={renderedSlot.eventType}
                          />
                        </div>
                        <div className="title headline">{renderedSlot.title}</div>
                        {(renderedNewsCategory !== null || renderedNewsAge !== null) && (
                          <div className="notif-meta-row">
                            {renderedNewsCategory !== null && (
                              <span className="chip chip-category">{renderedNewsCategory}</span>
                            )}
                            {renderedNewsAge !== null && (
                              <span className="notif-time-inline">{renderedNewsAge}</span>
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
                          <span className="notif-title">{renderedSlot.title}</span>
                          <div className="notif-header-badges">
                            {renderedSlot.origin === "cmux" && (
                              <span className="chip chip-cmux">Agent</span>
                            )}
                            <Stamp
                              priority={renderedSlot.priority}
                              signal={renderedSlot.signal}
                              eventType={renderedSlot.eventType}
                            />
                          </div>
                        </div>
                        {renderedSlot.subtitle !== null && (
                          <div className="notif-subtitle-row">
                            <span className="notif-subtitle">{renderedSlot.subtitle}</span>
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
                        {!renderedExpanded &&
                          renderedVisibleDetails.length > 0 &&
                          renderedVisibleDetails.map((detail) => (
                            <div key={`${detail.label}:${detail.value}`}>
                              <div className="detail-label">{detail.label}</div>
                              <div className="detail-value">{detail.value}</div>
                            </div>
                          ))}
                      </>
                    )}
                  </div>
                  {!renderedExpanded && (
                    <div className="compact-hint">
                      <kbd>⌃⇧N</kbd> more
                    </div>
                  )}
                  <Track total={renderedSlot.queueTotal} done={renderedSlot.queueDone} />
                </div>
                <Manifest
                  body={renderedSlot.body}
                  eventType={renderedSlot.eventType}
                  expanded={renderedExpanded}
                  source={renderedSlot.source}
                  category={renderedSlot.category}
                  publishedAtMs={renderedSlot.publishedAtMs}
                  hasLink={renderedSlot.link !== null}
                  subtitle={renderedSlot.subtitle}
                  details={renderedVisibleDetails}
                />
                {/* plan 100: last in DOM order within .below-block — the bar
                    is the card's floor, absolutely positioned to its bottom
                    edge (styles.css), clipped to the rounded corners by
                    .below-block's own overflow: hidden. */}
                <TtlBar
                  key={renderedSlot.id}
                  slotId={renderedSlot.id}
                  ttlMs={renderedSlot.ttlMs}
                  remainingMs={renderedSlot.remainingMs}
                  // plan 093: TTL hover-pause — this bar only ever mounts
                  // while `renderedShowing`, so `hovered` alone (the live
                  // cursor signal) is exactly "is THIS card hovered right
                  // now," no extra gating needed.
                  hoverPaused={hovered}
                />
              </>
            )}
          </div>
        </div>
      )}
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
