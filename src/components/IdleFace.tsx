import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { IDLE_GLANCE_MS, IDLE_REVEAL_MS, NOTCHTAP_EASE } from "../lib/constants";

const REVEAL_DELAY_MS = 4500;

type GazeName = "center" | "left" | "right" | "up";
const GAZE_OFFSETS = {
  center: { x: 0, y: 0 },
  left: { x: -3, y: 0 },
  right: { x: 3, y: 0 },
  up: { x: 0, y: -2 },
} satisfies Record<GazeName, { x: number; y: number }>;
const GAZE_SEQUENCE: GazeName[] = ["center", "left", "center", "right", "center", "up", "center"];

function randomBetween(minMs: number, maxMs: number): number {
  return minMs + Math.random() * (maxMs - minMs);
}

function useGazeCycle(active: boolean): GazeName {
  const [gaze, setGaze] = useState<GazeName>("center");

  useEffect(() => {
    if (!active) {
      setGaze("center");
      return;
    }
    let index = 0;
    let timeoutId: number;
    const step = () => {
      index = (index + 1) % GAZE_SEQUENCE.length;
      setGaze(GAZE_SEQUENCE[index]);
      timeoutId = window.setTimeout(step, randomBetween(6000, 11000));
    };
    timeoutId = window.setTimeout(step, randomBetween(6000, 11000));
    return () => window.clearTimeout(timeoutId);
  }, [active]);

  return gaze;
}

function useBlink(active: boolean): boolean {
  const [blinking, setBlinking] = useState(false);

  useEffect(() => {
    if (!active) {
      setBlinking(false);
      return;
    }
    let closeId: number;
    let openId: number;
    const scheduleNext = () => {
      closeId = window.setTimeout(
        () => {
          setBlinking(true);
          openId = window.setTimeout(() => {
            setBlinking(false);
            scheduleNext();
          }, 140);
        },
        randomBetween(6000, 12000),
      );
    };
    scheduleNext();
    return () => {
      window.clearTimeout(closeId);
      window.clearTimeout(openId);
    };
  }, [active]);

  return blinking;
}

export function IdleFace({ idle }: { idle: boolean }) {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    if (!idle) {
      setVisible(false);
      return;
    }
    const id = window.setTimeout(() => setVisible(true), REVEAL_DELAY_MS);
    return () => window.clearTimeout(id);
  }, [idle]);

  const gaze = useGazeCycle(visible);
  const blinking = useBlink(visible);
  const offset = GAZE_OFFSETS[gaze];

  return (
    <AnimatePresence>
      {visible ? (
        <motion.div
          className="idle-face"
          aria-hidden="true"
          initial={{ opacity: 0, scale: 0.92 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.85, transition: { duration: 0.1, ease: "easeOut" } }}
          transition={{ duration: IDLE_REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
        >
          <div
            className="idle-face-eyes"
            style={{
              transform: `translate(${offset.x}px, ${offset.y}px) scaleY(${blinking ? 0.12 : 1})`,
              transition: `transform ${IDLE_GLANCE_MS}ms cubic-bezier(${NOTCHTAP_EASE.join(", ")})`,
            }}
          >
            <span className="idle-face-eye" />
            <span className="idle-face-eye" />
          </div>
          <svg
            className="idle-face-mouth"
            viewBox="0 0 14 6"
            width="14"
            height="6"
            role="presentation"
          >
            <path d="M1 1.5 Q7 5.5 13 1.5" />
          </svg>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
