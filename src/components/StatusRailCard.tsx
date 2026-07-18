import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import { ageLabel, categoryClass, categoryLabel } from "../lib/presentation";
import type { SlotState } from "../useSlotState";
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

export function StatusRailCard({ slot }: { slot: SlotState }) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const news = showing && slot.eventType === "news_item";
  const newsCategory = news ? categoryLabel(slot.category) : null;
  const newsAge = news ? ageLabel(slot.publishedAtMs, Date.now()) : null;

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
  const cardClass = [
    "rail-card",
    showing ? slot.priority : "idle",
    expanded && "expanded",
    pulse,
    news && "news-shade",
    news && categoryClass(slot.category),
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={cardClass}
      role={showing ? "status" : undefined}
      aria-live={showing ? "polite" : undefined}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
      <AnimatePresence mode="wait" initial={false}>
        {!showing ? (
          <motion.div
            key="idle"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.22 }}
          >
            <IdleView />
          </motion.div>
        ) : (
          <motion.div
            key={slot.id}
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
          >
            <div className="compact">
              <div className="copy">
                {slot.eventType === "news_item" ? (
                  <>
                    <div className="masthead">
                      <span className="dot" />
                      {slot.source ?? "RSS"}
                    </div>
                    <div className="title headline">{slot.title}</div>
                    {(newsCategory !== null || newsAge !== null) && (
                      <div className="pills">
                        {newsCategory !== null && (
                          <motion.span
                            className="pill category"
                            initial={{ opacity: 0, y: 3 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{
                              duration: 0.22,
                              ease: [0.22, 1, 0.36, 1],
                            }}
                          >
                            {newsCategory}
                          </motion.span>
                        )}
                        {newsAge !== null && (
                          <motion.span
                            className="pill age"
                            initial={{ opacity: 0, y: 3 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{
                              duration: 0.22,
                              delay: 0.07,
                              ease: [0.22, 1, 0.36, 1],
                            }}
                          >
                            {newsAge}
                          </motion.span>
                        )}
                      </div>
                    )}
                  </>
                ) : (
                  <>
                    <div className="title">{slot.title}</div>
                    <div className="body">{renderInlineMarkdown(slot.body)}</div>
                  </>
                )}
              </div>
              <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
              {!expanded && <div className="compact-hint">⌃⇧N more</div>}
              <Track priority={slot.priority} />
            </div>
            <Manifest
              body={slot.body}
              eventType={slot.eventType}
              expanded={expanded}
              source={slot.source}
              category={slot.category}
              publishedAtMs={slot.publishedAtMs}
              hasLink={slot.link !== null}
            />
          </motion.div>
        )}
      </AnimatePresence>
      {/* The goal celebration is pure CSS now (plan 023): the confetti
          burst + ring live on `.rail-card.pulse-goal`'s ::after/::before,
          driven by the pulse state above — no separate element to mount. */}
    </div>
  );
}
