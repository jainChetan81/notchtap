import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { IDLE_GLANCE_MS, IDLE_REVEAL_MS, NOTCHTAP_EASE } from "../lib/constants";

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
//
// COST NOTE: at rest, the overlay's main thread must stay asleep ("bare
// notch ≈ zero cost") — the cadence constants below and the eyes'
// CSS-transition approach (see useGazeCycle/useBlink and the eyes'
// `style` below) keep the wakeup rate low and drop the rAF loop
// entirely, without changing what the face LOOKS like doing.
const REVEAL_DELAY_MS = 4500;

// A short, hand-picked "looks around" loop rather than fully random
// targets — center is revisited between every glance so the motion always
// returns to a resting pose instead of drifting.
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

// Cycles through GAZE_SEQUENCE on a gentle loop (~6-11s between
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
      timeoutId = window.setTimeout(step, randomBetween(6000, 11000));
    };
    timeoutId = window.setTimeout(step, randomBetween(6000, 11000));
    return () => window.clearTimeout(timeoutId);
  }, [active]);

  return gaze;
}

// An occasional quick blink (scaleY dip on the eyes group), independent of
// the gaze loop above — same active-gated start/stop discipline. Gap
// 6000–12000ms; the open/close leg stays short (140ms).
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
          /* Reveal motion on the house vocabulary: 0.24s, NOTCHTAP_EASE,
             entrance scale 0.92 — the face's entrance reads in step with
             the rest of the overlay's motion. */
          initial={{ opacity: 0, scale: 0.92 }}
          animate={{ opacity: 1, scale: 1 }}
          /* Per-variant exit override: the file's contract above says a
             card/hover must hide the face immediately — "no fade delay on
             the way OUT, only on the way in." Without this, the reveal
             transition also governed exit, so the face lingered into
             every promotion/hover. 0.1s reads as instant without a
             one-frame hard cut. */
          exit={{ opacity: 0, scale: 0.85, transition: { duration: 0.1, ease: "easeOut" } }}
          /* Duration is lib/constants.ts's IDLE_REVEAL_MS — same
             value, now named rather than a bare literal. */
          transition={{ duration: IDLE_REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
        >
          {/* Plain element with a CSS `transition` on `transform` —
              browser-driven (composited-eligible) and self-ending, no
              rAF loop, no per-frame React/JS once the style is set. The
              blink's scaleY rides the SAME `transform` property (one
              translate+scaleY string), so one transition covers both
              glance and blink with a single declaration. The curve is
              lib/constants.ts's NOTCHTAP_EASE, interpolated via
              `NOTCHTAP_EASE.join(", ")` below — the imported array is
              the only place these four numbers are written; the
              duration is lib/constants.ts's IDLE_GLANCE_MS. */}
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
