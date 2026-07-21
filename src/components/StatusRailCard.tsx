import { useEffect, useMemo, useState } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import { ageLabel, categoryClass, categoryLabel, publishedLabel } from "../lib/presentation";
import { weatherArtFor } from "../lib/weatherArt";
import { useDelayedSwap } from "../useDelayedSwap";
import type { SlotState } from "../useSlotState";
import { type StatusState, statusRailActive } from "../useStatusState";
import { IdleView } from "./IdleView";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import { Track } from "./Track";
import { TtlBar } from "./TtlBar";

// plan 082: weather ALERT cards ride the plan-035 display-only `details`
// channel to carry condition + day/night art inputs — `origin` is not on
// the slot-state wire, and this plan deliberately doesn't add it. Every
// pair whose label starts with "wx-" is a MARKER, never real content: it
// must be read to derive the mood/glyph art, then excluded from every
// place `details` is rendered as visible text (the marker-leak guard).
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
}: {
  slot: SlotState;
  status?: StatusState;
  // plan 085: the resting-state render choice. Optional, defaulting to
  // "rail" — every existing caller (tests, the settings preview) that
  // never passes it keeps today's idle rail, byte-identical.
  restingState?: "rail" | "notch";
}) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const news = showing && slot.eventType === "news_item";

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

  function clearPulseWhenItsAnimationEnds(event: React.AnimationEvent<HTMLDivElement>) {
    if (pulse && event.animationName === PULSE_END_ANIMATION[pulse]) {
      setPulse(null);
    }
  }

  const expanded = showing && slot.expanded;
  // plan 034: the idle card widens only while the status rail has chips
  // to show — the plain clock idle keeps its narrow width. `status` is
  // optional: the settings preview renders the card without one.
  const statusRail = !showing && status !== undefined && statusRailActive(status);
  // plan 082: weather ALERT cards carry their art derived from the live
  // slot's `wx-*` marker pairs — same live-`slot` basis as `news`/
  // `categoryClass` above (not `renderedSlot`), so the outer shell's mood
  // updates in lockstep with every other outer-shell class. `null` for
  // every non-weather card, so it renders byte-identical to today.
  const wxArt = showing ? weatherArtFromDetails(slot.details) : null;
  const cardClass = [
    "rail-card",
    showing ? slot.priority : "idle",
    statusRail && "status",
    expanded && "expanded",
    pulse,
    news && "news-shade",
    news && categoryClass(slot.category),
    wxArt && "wx-card",
    wxArt?.moodClass,
    wxArt?.textureClass,
  ]
    .filter(Boolean)
    .join(" ");

  // plan 078: the idle/showing swap (formerly AnimatePresence mode="wait")
  // freezes the outgoing item via useDelayedSwap while the CSS exit
  // animation runs, then swaps content and key together. Everything that
  // was inside the old swapped motion.div reads from `renderedSlot`;
  // only cardClass (outer div) and the pulse celebration stay on live
  // slot/status, exactly as they did before.
  const swapKey = showing ? slot.id : "idle";
  const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, 220);
  const renderedShowing = renderedSlot.state === "showing";
  const renderedExpanded = renderedShowing && renderedSlot.expanded;
  const renderedNews = renderedShowing && renderedSlot.eventType === "news_item";
  const renderedNewsCategory = renderedNews ? categoryLabel(renderedSlot.category) : null;
  const renderedNewsAge = renderedNews ? ageLabel(renderedSlot.publishedAtMs, Date.now()) : null;
  const renderedNewsPublished = renderedNews
    ? publishedLabel(renderedSlot.publishedAtMs, Date.now())
    : null;
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

  // plan 085: idle + resting_state "notch" → zero app-drawn pixels, not a
  // narrower/emptied shell — the outer `.rail-card` div itself must not
  // mount (it carries background/shadow/priority-accent styling that a
  // bare native notch must never show). Gated on the delayed-swap-settled
  // state (`renderedShowing`/`exiting`), not the live `showing` flag, so a
  // still-exiting prior card finishes its normal exit animation exactly as
  // it does in "rail" mode; only once the swap has fully settled into idle
  // does notch mode hide the card. Every `showing` path is unaffected.
  if (!renderedShowing && !exiting && restingState === "notch") {
    return null;
  }

  return (
    <div
      className={cardClass}
      role={showing ? "status" : undefined}
      aria-live={showing ? "polite" : undefined}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
      {/* plan 082: the condition glyph — a background-layer image, same
          z-order tier as .news-shade::before (behind .compact/.manifest,
          which the CSS below lifts to z-index 1). Live-slot-derived, like
          the mood/texture classes on the outer div above, so it never
          waits on the delayed content swap. */}
      {wxArt && <img className="wx-icon" src={wxArt.glyphUrl} alt="" />}
      <div
        key={swapKey}
        className={`card-content${!renderedShowing ? " idle" : ""}${exiting ? " swap-exit" : ""}`}
      >
        {!renderedShowing ? (
          <IdleView status={status} />
        ) : (
          <>
            <div className="compact">
              <div className="copy">
                {renderedNews ? (
                  <>
                    <div className="masthead">
                      <span className="dot" />
                      {renderedSlot.source ?? "RSS"}
                    </div>
                    <div className="title headline">{renderedSlot.title}</div>
                    {(renderedNewsCategory !== null || renderedNewsAge !== null) && (
                      <div className="pills">
                        {renderedNewsCategory !== null && (
                          <span className="pill category">{renderedNewsCategory}</span>
                        )}
                        {renderedNewsAge !== null && (
                          <span className="pill age">{renderedNewsAge}</span>
                        )}
                        {renderedNewsPublished !== null && (
                          <span className="pub-meta">published {renderedNewsPublished}</span>
                        )}
                      </div>
                    )}
                  </>
                ) : (
                  <>
                    <div className="title">{renderedSlot.title}</div>
                    <div className="body">{bodyContent}</div>
                    {/* plan 042: collapsed scorecard cells (Clock, per-side
                        Cards) — only a live-match card with `espn_live_card`
                        on populates `details`, so every other card renders
                        exactly as before. Same detail-label/detail-value
                        classes as the expanded Manifest view; collapsed-only,
                        so the pairs never render twice when expanded. */}
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
              <Stamp
                priority={renderedSlot.priority}
                signal={renderedSlot.signal}
                eventType={renderedSlot.eventType}
              />
              {!renderedExpanded && (
                <div className="compact-hint">
                  <kbd>⌃⇧N</kbd> more
                </div>
              )}
              <Track total={renderedSlot.queueTotal} done={renderedSlot.queueDone} />
            </div>
            <TtlBar
              key={renderedSlot.id}
              slotId={renderedSlot.id}
              ttlMs={renderedSlot.ttlMs}
              remainingMs={renderedSlot.remainingMs}
            />
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
          </>
        )}
      </div>
      {/* the goal celebration is plan 023's pure-CSS confetti burst +
          ring on `.rail-card.pulse-goal`'s ::after/::before PLUS plan
          032's ripple: three staggered concentric accent rings, mounted
          only while the goal pulse is live and unmounted by the same
          animationend path that clears the burst (goal-signal only,
          one-shot — never keyed on priority). */}
      {pulse === "pulse-goal" && (
        <div className="cele-ripple" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
      )}
    </div>
  );
}
