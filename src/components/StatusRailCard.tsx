import { useEffect, useMemo, useState } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import { ageLabel, categoryClass, categoryLabel, publishedLabel } from "../lib/presentation";
import { useDelayedSwap } from "../useDelayedSwap";
import type { SlotState } from "../useSlotState";
import { type StatusState, statusRailActive } from "../useStatusState";
import { IdleView } from "./IdleView";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import { Track } from "./Track";

type Pulse = "pulse-goal" | "pulse-red" | null;

// Which @keyframes name (styles.css) ends each pulse — the *only* place
// either duration lives is the CSS animation itself; clearing on
// animationend means there's no JS-side duration to keep in sync with it.
const PULSE_END_ANIMATION: Record<NonNullable<Pulse>, string> = {
  "pulse-goal": "goal-overshoot",
  "pulse-red": "red-alert",
};

export function StatusRailCard({ slot, status }: { slot: SlotState; status?: StatusState }) {
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
  const cardClass = [
    "rail-card",
    showing ? slot.priority : "idle",
    statusRail && "status",
    expanded && "expanded",
    pulse,
    news && "news-shade",
    news && categoryClass(slot.category),
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

  // plan 069 (folded into 078): memoized on the rendered slot so unrelated
  // re-renders don't re-tokenize the markdown.
  const bodyContent = useMemo(
    () => renderInlineMarkdown(renderedSlot.state === "showing" ? renderedSlot.body : ""),
    [renderedSlot],
  );

  return (
    <div
      className={cardClass}
      role={showing ? "status" : undefined}
      aria-live={showing ? "polite" : undefined}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
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
                      renderedSlot.details.length > 0 &&
                      renderedSlot.details.map((detail) => (
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
            <Manifest
              body={renderedSlot.body}
              eventType={renderedSlot.eventType}
              expanded={renderedExpanded}
              source={renderedSlot.source}
              category={renderedSlot.category}
              publishedAtMs={renderedSlot.publishedAtMs}
              hasLink={renderedSlot.link !== null}
              subtitle={renderedSlot.subtitle}
              details={renderedSlot.details}
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
