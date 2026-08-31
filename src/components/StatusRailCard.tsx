import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AgentSessionView } from "../hooks/useAgentState";
import { useExitChoreography } from "../hooks/useExitChoreography";
import type { EspnMeta, Priority, SlotState } from "../hooks/useSlotState";
import type { StatusState } from "../hooks/useStatusState";
import {
  CONTENT_EXIT_MS,
  EXPAND_MS,
  INTERRUPT_EASE,
  INTERRUPT_EXIT_MS,
  NOTCHTAP_EASE,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SWAP_EXIT_MS,
} from "../lib/constants";
import { iconPresenceFor } from "../lib/iconPresence";
import { renderInlineMarkdown } from "../lib/markdown";
import {
  ageLabel,
  type Celebration,
  categoryClass,
  categoryLabel,
  eventKindPresentationFor,
  footballEventKindFor,
  livePillVariantFor,
  sourceClass,
} from "../lib/presentation";
import { FlankClock } from "./FlankClock";
import type { Tab } from "./IconStrip";
import { IconStrip } from "./IconStrip";
import { IdleFace } from "./IdleFace";
import { IdleHoverPeek, type PeekPreference } from "./IdleHoverPeek";
import { FootballHeroCard, NotificationBody } from "./NotificationBody";
import { TabBelowBlock, tabBelowBlockHandles } from "./TabBelowBlock";

const CELEBRATION_END_ANIMATION = {
  "cele-goal": "cele-ring",
  "cele-yc": "cele-ring",
  "cele-rc": "red-strobe",
} satisfies Record<NonNullable<Celebration>, string>;

// Exported so NotificationBody.tsx imports the shape — one definition, not two.
export type Detail = { label: string; value: string };

type Pulse = "pulse-goal" | "pulse-red" | null;

const PULSE_END_ANIMATION = {
  "pulse-goal": "ripple-out",
  "pulse-red": "red-alert",
} satisfies Record<NonNullable<Pulse>, string>;

const RIPPLE_RING_COUNT = 3;

export const contentExitVariants = {
  exit: (custom: { isRotation: boolean; isInterrupt: boolean }) => {
    if (custom.isInterrupt) {
      return {
        opacity: 0,
        transform: "translateY(8px) scale(0.96)",
        transition: { duration: INTERRUPT_EXIT_MS / 1000, ease: INTERRUPT_EASE },
      };
    }
    return custom.isRotation
      ? { opacity: 0, transition: { duration: ROTATION_EXIT_MS / 1000, ease: NOTCHTAP_EASE } }
      : { opacity: 0, transition: { duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE } };
  },
};

const PRIORITY_RANK = {
  low: 0,
  medium: 1,
  high: 2,
} satisfies Record<"low" | "medium" | "high", number>;

const INTERRUPT_MIN_REMAINING_MS = 400;

function peekPreferenceFor(selected: Tab | null): PeekPreference {
  return selected === "football" ? selected : null;
}

const NO_AGENT_SESSIONS: AgentSessionView[] = [];

const noopSelect = (_tab: Tab): void => {};

export function StatusRailCard({
  slot,
  status,
  restingState = "rail",
  hovered = false,
  selectedTab = null,
  agentSessions = NO_AGENT_SESSIONS,
  agentCapturedAtMs = 0,
  viewedSessionIndex,
}: {
  slot: SlotState;
  status?: StatusState;
  restingState?: "rail" | "notch";
  hovered?: boolean;
  selectedTab?: Tab | null;
  agentSessions?: AgentSessionView[];
  agentCapturedAtMs?: number;
  viewedSessionIndex?: number;
}) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const currentBody = showing ? slot.body : null;
  const news = showing && slot.eventType === "news_item";
  const isLiveCard = showing && slot.espn !== undefined;

  const [pulse, setPulse] = useState<Pulse>(null);

  const pulseRef = useRef<Pulse>(null);
  const setPulseNow = useCallback((next: Pulse) => {
    pulseRef.current = next;
    setPulse(next);
  }, []);

  const rippleEndsSeenRef = useRef(0);

  const wasShowingRef = useRef<{
    key: unknown;
    isRotation: boolean;
    isInterrupt: boolean;
    wasShowing: boolean;
    anchor: { priority: Priority; anchoredAt: number; remainingMs: number } | null;
  }>({
    key: undefined,
    isRotation: false,
    isInterrupt: false,
    wasShowing: false,
    anchor: null,
  });

  // biome-ignore lint/correctness/useExhaustiveDependencies: currentId is the deliberate re-trigger — same signal on a new notification must replay the pulse
  useEffect(() => {
    const nextPulse: Pulse =
      currentSignal === "goal" ? "pulse-goal" : currentSignal === "red_card" ? "pulse-red" : null;

    rippleEndsSeenRef.current = 0;

    if (nextPulse !== null && pulseRef.current === nextPulse) {
      setPulseNow(null);
      const frame = requestAnimationFrame(() => setPulseNow(nextPulse));
      return () => cancelAnimationFrame(frame);
    }

    setPulseNow(nextPulse);
  }, [currentId, currentSignal, setPulseNow]);

  const [liveCelebration, setLiveCelebration] = useState<Celebration>(null);

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
      const isGoal = pulse === "pulse-goal";
      if (isGoal) {
        rippleEndsSeenRef.current += 1;
      }
      if (!isGoal || rippleEndsSeenRef.current >= RIPPLE_RING_COUNT) {
        setPulseNow(null);
      }
    }
    if (liveCelebration && event.animationName === CELEBRATION_END_ANIMATION[liveCelebration]) {
      setLiveCelebration(null);
    }
  }

  const agentOrigin = showing && slot.origin === "agent";

  const swapKey = showing ? slot.id : "idle";

  const [promoting, setPromoting] = useState(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: swapKey is the deliberate re-arm trigger, not a value read in the body — same shape as the pulse effect's own currentId dependency above.
  useEffect(() => {
    const timer = window.setTimeout(() => setPromoting(false), EXPAND_MS);
    return () => window.clearTimeout(timer);
  }, [swapKey]);

  if (wasShowingRef.current.key !== swapKey) {
    const previous = wasShowingRef.current;
    const isRotationNow = showing && previous.wasShowing;
    let isInterruptNow = false;
    if (isRotationNow && previous.anchor) {
      const elapsedMs = performance.now() - previous.anchor.anchoredAt;
      const estimatedRemainingMs = previous.anchor.remainingMs - elapsedMs;
      isInterruptNow =
        PRIORITY_RANK[slot.priority] > PRIORITY_RANK[previous.anchor.priority] &&
        estimatedRemainingMs > INTERRUPT_MIN_REMAINING_MS;
    }
    wasShowingRef.current = {
      key: swapKey,
      isRotation: isRotationNow,
      isInterrupt: isInterruptNow,
      wasShowing: showing,
      anchor: showing
        ? { priority: slot.priority, anchoredAt: performance.now(), remainingMs: slot.remainingMs }
        : null,
    };
    setPromoting(showing && (!isRotationNow || isInterruptNow));
  } else if (showing && wasShowingRef.current.anchor?.remainingMs !== slot.remainingMs) {
    wasShowingRef.current = {
      ...wasShowingRef.current,
      anchor: {
        priority: slot.priority,
        anchoredAt: performance.now(),
        remainingMs: slot.remainingMs,
      },
    };
  }
  const isRotation = wasShowingRef.current.isRotation;
  const isInterrupt = wasShowingRef.current.isInterrupt;
  // The entering child's own animation treats a Priority Preemption's
  // incoming card like an ordinary promotion (the full slide-in), never
  // the lighter same-tier rotation, even though `isRotation` is
  // structurally true for it too (see `isInterrupt`'s doc above). Named
  // once here so every consumer below (initial/animate/duration/
  // rotation-swap class/data attribute) reads the same derived value.
  const enterAsPromotion = !isRotation || isInterrupt;

  // The showing<->idle exit-choreography state machine lives in
  // src/useExitChoreography.ts (that file documents each value). The
  // hook's own intermediates `renderedSlot`/`exiting` are NOT
  // destructured here — every consumer
  // (geometryPriority/expanded/shellExiting/bare/trueIdle) is itself a
  // hook output now; destructuring them unused would trip `noUnusedLocals`.
  const {
    renderedShowing,
    belowBlockOpen,
    geometryPriority,
    expanded,
    shellExiting,
    bare,
    exitToBare,
    railRevealed,
    trueIdle,
    idleFaceEligible,
  } = useExitChoreography(slot, restingState, hovered);

  // The icon strip's five tiers, derived from the ambient status wire by
  // one pure table (`lib/iconPresence.ts`). Memoized on `status` alone
  // because it depends on nothing else and `status` changes only on a
  // genuine wire emission, unlike this component's own per-tick
  // re-renders.
  const iconPresence = useMemo(() => iconPresenceFor(status), [status]);
  const presentIconCount = useMemo(
    () => Object.values(iconPresence).filter((state) => state !== "hidden").length,
    [iconPresence],
  );
  const newsWaitingCount = status?.news.chargeCount ?? 0;

  const tabPullOpen = !renderedShowing && hovered;
  const pulledTab = tabPullOpen ? selectedTab : null;
  const peekPreference = peekPreferenceFor(pulledTab);
  const pulledTabHasContent =
    pulledTab === "agent"
      ? agentSessions.length > 0
      : pulledTab === "news"
        ? false
        : pulledTab !== null;
  const pulledBelowBlockOpen = tabBelowBlockHandles(pulledTab) && pulledTabHasContent;
  const peekOpen = tabPullOpen && !pulledBelowBlockOpen;

  const cardClass = [
    "card-assembly",
    geometryPriority,
    expanded && "expanded",
    promoting && "promoting",
    hovered && "hovered",
    bare && "bare",
    shellExiting && "exiting",
    exitToBare && "exit-to-bare",
    !isLiveCard && pulse,
    isLiveCard && liveCelebration,
  ]
    .filter(Boolean)
    .join(" ");
  const belowBlockClass = [
    "below-block",
    news && "news-shade",
    news && categoryClass(slot.category),
    !news && showing && slot.origin !== "news" && sourceClass(slot.origin, slot.agentRuntime),
    agentOrigin && "agent-origin",
  ]
    .filter(Boolean)
    .join(" ");

  const newsCategory = news ? categoryLabel(slot.category) : null;
  const newsAge = news ? ageLabel(slot.publishedAtMs, Date.now()) : null;
  const liveVisibleDetails = showing ? slot.details : [];

  const bodyContent = useMemo(() => renderInlineMarkdown(currentBody ?? ""), [currentBody]);

  const liveEspn: EspnMeta | undefined = showing ? slot.espn : undefined;
  const pillVariant = showing && isLiveCard ? livePillVariantFor(slot.signal) : "live";
  const pillLabel = pillVariant === "break" ? "Break" : pillVariant === "final" ? "Final" : "Live";
  const cardsClean =
    liveEspn !== undefined &&
    liveEspn.homeCards[0] === 0 &&
    liveEspn.homeCards[1] === 0 &&
    liveEspn.awayCards[0] === 0 &&
    liveEspn.awayCards[1] === 0;

  const liveRegionActive = showing && !isLiveCard;

  return (
    <div
      className={cardClass}
      // SAFETY: CSS custom property valid for this component; React.CSSProperties lacks index signature.
      style={{ "--present-icons": presentIconCount } as React.CSSProperties}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
      <span className="notch-gill notch-gill-left" aria-hidden="true" />
      <span className="notch-gill notch-gill-right" aria-hidden="true" />
      <div className="flank-left">
        <AnimatePresence>
          {railRevealed && !exitToBare && (
            <motion.span
              key="flank-clock"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              <FlankClock />
            </motion.span>
          )}
        </AnimatePresence>
      </div>
      <div className="synthetic-cutout" aria-hidden="true" />

      {idleFaceEligible && (
        <div className="rest-cluster" aria-hidden="true">
          <IdleFace idle={trueIdle} />
        </div>
      )}
      <div className="flank-right">
        {railRevealed && !exitToBare && !renderedShowing && (
          <IconStrip
            {...iconPresence}
            newsCharge={status?.news.chargeFraction ?? 0}
            newsCharged={status?.news.isCharged ?? false}
            newsCount={newsWaitingCount > 0 ? newsWaitingCount : null}
            selected={selectedTab}
            onSelect={noopSelect}
          />
        )}
      </div>
      <IdleHoverPeek status={status} hovered={hovered} open={peekOpen} prefer={peekPreference} />
      <AnimatePresence mode="wait" initial={false}>
        {pulledTab !== null && pulledTabHasContent && (
          <motion.div
            className="tab-below-slot"
            key={pulledTab}
            initial={{ opacity: 0, y: -4 }}
            animate={{
              opacity: 1,
              y: 0,
              transition: {
                duration: ROTATION_ENTER_MS / 1000,
                ease: NOTCHTAP_EASE,
              },
            }}
            exit={{
              opacity: 0,
              y: -2,
              transition: {
                duration: ROTATION_EXIT_MS / 1000,
                ease: NOTCHTAP_EASE,
              },
            }}
          >
            <TabBelowBlock
              selected={pulledTab}
              status={status}
              agentSessions={agentSessions}
              agentCapturedAtMs={agentCapturedAtMs}
              viewedSessionIndex={viewedSessionIndex}
            />
          </motion.div>
        )}
      </AnimatePresence>
      <div
        style={{ display: "contents" }}
        role={liveRegionActive ? "status" : undefined}
        aria-live={liveRegionActive ? "polite" : undefined}
      >
        <AnimatePresence>
          {belowBlockOpen && (
            <motion.div
              className={belowBlockClass}
              initial={false}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              <AnimatePresence mode="wait" custom={{ isRotation, isInterrupt }}>
                {showing && (
                  <motion.div
                    key={swapKey}
                    className={
                      isRotation && !isInterrupt ? "card-content rotation-swap" : "card-content"
                    }
                    initial={
                      enterAsPromotion
                        ? { opacity: 0, transform: "translateY(-4px)" }
                        : { opacity: 0 }
                    }
                    animate={
                      enterAsPromotion
                        ? { opacity: 1, transform: "translateY(0px)" }
                        : { opacity: 1 }
                    }
                    data-rotation-swap={isRotation && !isInterrupt}
                    data-interrupt-swap={isInterrupt}
                    variants={contentExitVariants}
                    exit="exit"
                    transition={{
                      duration: (enterAsPromotion ? SWAP_EXIT_MS : ROTATION_ENTER_MS) / 1000,
                      ease: NOTCHTAP_EASE,
                    }}
                  >
                    {isLiveCard && liveEspn !== undefined ? (
                      <FootballHeroCard
                        title={slot.body}
                        priority={slot.priority}
                        signal={slot.signal}
                        eventType={slot.eventType}
                        liveEspn={liveEspn}
                        pillVariant={pillVariant}
                        pillLabel={pillLabel}
                        cardsClean={cardsClean}
                      />
                    ) : (
                      <NotificationBody
                        news={news}
                        slot={slot}
                        newsCategory={newsCategory}
                        newsAge={newsAge}
                        bodyContent={bodyContent}
                        expanded={expanded}
                        liveVisibleDetails={liveVisibleDetails}
                        hovered={hovered}
                      />
                    )}
                  </motion.div>
                )}
              </AnimatePresence>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      {/* The goal celebration: pure-CSS confetti burst +
          ring on `.card-assembly.pulse-goal`'s ::after/::before PLUS
          ripple: three staggered concentric accent rings, mounted
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
