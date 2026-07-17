import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import type { SlotState } from "../useSlotState";
import { TierCode } from "./TierCode";
import { Stamp } from "./Stamp";
import { Track } from "./Track";
import { Manifest } from "./Manifest";
import { IdleView } from "./IdleView";
import { GoalCelebration } from "./GoalCelebration";

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

  const [pulse, setPulse] = useState<Pulse>(null);

  // Keyed on [currentId, currentSignal], never on priority — the actual
  // acceptance criterion this field exists for: a High-priority cmux
  // "needs input" alert (signal: "generic") must never play the goal
  // celebration. Not keyed on `expanded` either, so toggling the manual
  // hotkey on an already-visible item doesn't replay the burst.
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
              <TierCode priority={slot.priority} />
              <div className="copy">
                <div className="title">{slot.title}</div>
                <div className="body">{slot.body}</div>
              </div>
              <Stamp priority={slot.priority} signal={slot.signal} />
              <Track priority={slot.priority} />
            </div>
            <Manifest body={slot.body} eventType={slot.eventType} expanded={expanded} />
          </motion.div>
        )}
      </AnimatePresence>
      {showing && slot.signal === "goal" && <GoalCelebration />}
    </div>
  );
}
