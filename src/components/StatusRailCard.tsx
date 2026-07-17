import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import type { SlotState } from "../useSlotState";
import { TierCode } from "./TierCode";
import { Stamp } from "./Stamp";
import { Track } from "./Track";
import { Manifest } from "./Manifest";
import { IdleView } from "./IdleView";
import { GoalCelebration } from "./GoalCelebration";
import { ageLabel, categoryClass, categoryLabel } from "../lib/presentation";

type Pulse = "pulse-goal" | "pulse-red" | null;
const PULSE_DURATION_MS: Record<NonNullable<Pulse>, number> = {
  "pulse-goal": 620,
  "pulse-red": 920, // two 460ms strobe cycles
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
  useEffect(() => {
    if (currentSignal === "goal" || currentSignal === "red_card") {
      const next: Pulse = currentSignal === "goal" ? "pulse-goal" : "pulse-red";
      setPulse(next);
      const timeout = window.setTimeout(() => setPulse(null), PULSE_DURATION_MS[next]);
      return () => window.clearTimeout(timeout);
    }
    setPulse(null);
  }, [currentId, currentSignal]);

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
    <div className={cardClass} role="status" aria-live="polite">
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
              <TierCode priority={slot.priority} eventType={slot.eventType} />
              <div className="copy">
                {slot.eventType === "news_item" ? (
                  <>
                    <div className="masthead">
                      <span className="dot" />
                      {slot.source ?? "RSS"} · Wire
                    </div>
                    <div className="title headline">{slot.title}</div>
                    {(newsCategory !== null || newsAge !== null) && (
                      <div className="pills">
                        {newsCategory !== null && (
                          <motion.span
                            className="pill category"
                            initial={{ opacity: 0, y: 3 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
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
                    <div className="body">{slot.body}</div>
                  </>
                )}
              </div>
              <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
              <Track priority={slot.priority} />
            </div>
            <Manifest
              body={slot.body}
              eventType={slot.eventType}
              expanded={expanded}
              source={slot.source}
              category={slot.category}
              publishedAtMs={slot.publishedAtMs}
            />
          </motion.div>
        )}
      </AnimatePresence>
      {showing && slot.signal === "goal" && <GoalCelebration />}
    </div>
  );
}
