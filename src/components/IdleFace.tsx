import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

// The idle face: a minimal, notchtap-branded bit of personality that fades
// into the CENTER of the idle rail (the `.synthetic-cutout` grid cell —
// StatusRailCard.tsx renders this as a sibling of that cell, same
// grid-column/row) after a few seconds of continuous true-idle, then
// glances softly around while it's up. Purely decorative: `aria-hidden`
// and `pointer-events: none` (overlay-card.css) so it can never shadow the
// rust-derived hover tracking area or the real notch hardware in notch
// mode (CSS alone decides visibility there, mirroring `.synthetic-cutout`
// itself — see the mode-gated rule in overlay-card.css).
//
// Gating is the caller's job: StatusRailCard passes `idle` as
// `!showing && !renderedShowing && !exiting && !hovered` — a card
// showing/exiting, or a hover, must hide the face immediately (no fade
// delay on the way OUT, only on the way in).
const REVEAL_DELAY_MS = 4500;

// A short, hand-picked "looks around" loop rather than fully random
// targets — center is revisited between every glance so the motion always
// returns to a resting pose instead of drifting.
type GazeName = "center" | "left" | "right" | "up";
const GAZE_OFFSETS: Record<GazeName, { x: number; y: number }> = {
  center: { x: 0, y: 0 },
  left: { x: -3, y: 0 },
  right: { x: 3, y: 0 },
  up: { x: 0, y: -2 },
};
const GAZE_SEQUENCE: GazeName[] = ["center", "left", "center", "right", "center", "up", "center"];

function randomBetween(minMs: number, maxMs: number): number {
  return minMs + Math.random() * (maxMs - minMs);
}

// Cycles through GAZE_SEQUENCE on a gentle loop (~1.5-2.5s between
// glances) while `active`; resets to center (and stops scheduling) the
// instant `active` goes false, so the face never keeps ticking in the
// background once it's hidden.
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
      timeoutId = window.setTimeout(step, randomBetween(1500, 2500));
    };
    timeoutId = window.setTimeout(step, randomBetween(1500, 2500));
    return () => window.clearTimeout(timeoutId);
  }, [active]);

  return gaze;
}

// An occasional quick blink (scaleY dip on the eyes group), independent of
// the gaze loop above — same active-gated start/stop discipline.
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
        randomBetween(3000, 6000),
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
  // The delayed reveal: a timer that (re)starts every time `idle` flips
  // true, and is cancelled — instantly hiding the face via AnimatePresence
  // below — the moment `idle` flips false again (a card showing, or a
  // hover). The cleanup below is what gives "resets whenever idle breaks"
  // for free: a new effect run on `idle` changing always tears down the
  // previous timeout first.
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
          initial={{ opacity: 0, scale: 0.85 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.85 }}
          transition={{ duration: 0.5, ease: "easeOut" }}
        >
          <motion.div
            className="idle-face-eyes"
            animate={{ x: offset.x, y: offset.y, scaleY: blinking ? 0.12 : 1 }}
            transition={{ type: "spring", stiffness: 140, damping: 16 }}
          >
            <span className="idle-face-eye" />
            <span className="idle-face-eye" />
          </motion.div>
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
