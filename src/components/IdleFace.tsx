import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { NOTCHTAP_EASE } from "../animationTiming";

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
// COST NOTE (plan 125, /improve-animations audit finding #1, HIGH):
// once revealed, this component used to re-arm a JS timer every
// 1.5-2.5s (gaze) and 3-6s (blink) forever, plus drive the eyes with a
// `motion` spring (a per-frame rAF loop while it settles) — on the
// Mac mini (`idleFaceEligible`) that meant the overlay's main thread
// never slept longer than ~2.5s, 24/7, in exactly the "bare notch ≈
// zero cost" resting state the app is supposed to have. The cadence
// constants below and the eyes' CSS-transition approach (see
// useGazeCycle/useBlink and the eyes' `style` below) exist to cut that
// wakeup rate roughly 4x and drop the rAF loop entirely, without
// changing what the face LOOKS like doing.
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

// Cycles through GAZE_SEQUENCE on a gentle loop (~6-11s between
// glances) while `active`; resets to center (and stops scheduling) the
// instant `active` goes false, so the face never keeps ticking in the
// background once it's hidden.
//
// plan 125: was 1500-2500ms — a glance every ~8s still reads as calm
// (arguably calmer than the old fidgety ~2s cadence), while cutting
// the idle-cost wakeup rate roughly 4x (see the file-header COST NOTE).
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
// the gaze loop above — same active-gated start/stop discipline.
//
// plan 125: the gap between blinks was 3000-6000ms — widened to
// 6000-12000ms alongside the gaze cadence above, same idle-cost
// rationale. The blink's own open/close leg (140ms) is untouched; only
// how often a blink gets scheduled changed.
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
          /* plan 125 (character finding #11): scale 0.85 -> 0.92 and the
             transition's duration/ease moved onto the house vocabulary
             (300ms UI ceiling -> 0.24s here, NOTCHTAP_EASE instead of
             motion's built-in "easeOut") — the old 0.5s/easeOut/0.85 was
             both slower than every other reveal in the app and used a
             different curve, so the face's entrance read as slightly out
             of place next to the rest of the overlay's motion. */
          initial={{ opacity: 0, scale: 0.92 }}
          animate={{ opacity: 1, scale: 1 }}
          /* Per-variant exit override: the file's contract above says a
             card/hover must hide the face immediately — "no fade delay on
             the way OUT, only on the way in." Without this, the reveal
             transition also governed exit, so the face lingered into
             every promotion/hover (2026-07-23 review finding). 0.1s reads
             as instant without a one-frame hard cut. Left exactly as-is
             (0.85/easeOut/0.1s) — this override is a documented review
             fix, not part of plan 125's scope. */
          exit={{ opacity: 0, scale: 0.85, transition: { duration: 0.1, ease: "easeOut" } }}
          transition={{ duration: 0.24, ease: NOTCHTAP_EASE }}
        >
          {/* plan 125 (perf finding #1): was a `motion.div` animating via
              a `{ type: "spring", stiffness: 140, damping: 16 }` spring —
              damping ratio ~0.68, visibly wobblier than the app's house
              spring (IdleHoverPeek's 480/37, ~0.84), and springs run a
              main-thread rAF loop for every glance/blink on top of the
              gaze/blink timers themselves. A plain element with a CSS
              `transition` on `transform` is browser-driven (composited-
              eligible) and self-ending — no rAF loop, no per-frame
              React/JS involvement once the style is set. The blink's
              scaleY rides the SAME `transform` property (folded into one
              translate+scaleY string) so one transition covers both
              glance and blink with a single declaration. 200ms /
              cubic-bezier(0.22, 1, 0.36, 1) is the house curve
              (animationTiming.ts's NOTCHTAP_EASE, [0.22, 1, 0.36, 1]) —
              written as a literal here rather than interpolated because
              CSS transition shorthand needs a string, and animationTiming
              only exports the numeric array form (plan 127 owns adding a
              string export, if one turns out to be worth it). */}
          <div
            className="idle-face-eyes"
            style={{
              transform: `translate(${offset.x}px, ${offset.y}px) scaleY(${blinking ? 0.12 : 1})`,
              transition: "transform 200ms cubic-bezier(0.22, 1, 0.36, 1)",
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
