import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
} from "../animationTiming";
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
import type { AgentSessionView } from "../useAgentState";
import { useExitChoreography } from "../useExitChoreography";
import type { EspnMeta, Priority, SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import type { Tab } from "./IconStrip";
import { IconStrip } from "./IconStrip";
import { IdleFace } from "./IdleFace";
import { IdleHoverPeek, type PeekPreference } from "./IdleHoverPeek";
import { FootballHeroCard, NotificationBody } from "./NotificationBody";
import { TabBelowBlock, tabBelowBlockHandles } from "./TabBelowBlock";

// Which keyframe (styles.css) ends each live-branch celebration — cleared on
// animationend, scoped to the espn branch so a live goal never stacks both
// `pulse-goal` and `cele-goal` (see the `isLiveCard` gate below).
const CELEBRATION_END_ANIMATION: Record<NonNullable<Celebration>, string> = {
  "cele-goal": "cele-ring",
  "cele-yc": "cele-ring",
  "cele-rc": "red-strobe",
};

// Exported so NotificationBody.tsx imports the shape — one definition, not two.
export type Detail = { label: string; value: string };

type Pulse = "pulse-goal" | "pulse-red" | null;

// Which @keyframes name (styles.css) ends each pulse — the CSS animation is the
// only place either duration lives; clearing on animationend keeps no JS-side
// copy. `pulse-goal` clears on `ripple-out`, the LAST thing the goal celebration
// plays (the staggered rings outlive the shell keyframe).
const PULSE_END_ANIMATION: Record<NonNullable<Pulse>, string> = {
  "pulse-goal": "ripple-out",
  "pulse-red": "red-alert",
};

// How many `.cele-ripple` rings the goal celebration mounts — `ripple-out` ends
// once per ring and only the last means the celebration is over; the JSX below
// must mount exactly this many spans.
const RIPPLE_RING_COUNT = 3;

// Content-swap exit leg. An exiting motion.div stops receiving props once its
// key leaves the JSX, so AnimatePresence's `custom` prop is the only channel
// that can hand it `{ isRotation, isInterrupt }`; `isInterrupt` wins outright —
// a Priority Preemption is structurally also a rotation but plays the sharper
// "yanked" exit. Test-only export: jsdom/motion don't expose a committed
// animation's `transition`, so this object is the only place the values are
// checkable (StatusRailCard.test.tsx).
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

// Mirrors rust's `Priority` ordering (`event.rs`/`queue.rs` derive `Ord`,
// declaration order Low < Medium < High).
const PRIORITY_RANK: Record<"low" | "medium" | "high", number> = {
  low: 0,
  medium: 1,
  high: 2,
};

// The wire carries no explicit "preempted" flag (queue.rs's `try_preempt_visible`
// is rust-internal), so a preemption is inferred: an ordinary rotation fires with
// ~0 remaining, a preemption cuts the item off with real time left. 400ms clears
// the engine's tick/emission jitter (engine.rs wakes ~10ms past its deadline).
const INTERRUPT_MIN_REMAINING_MS = 400;

// Selections served by `IdleHoverPeek`'s own rendering rather than a dedicated
// below-block component — see `TabBelowBlock.tsx`'s header for the split.
function peekPreferenceFor(selected: Tab | null): PeekPreference {
  return selected === "football" ? selected : null;
}

// Module-level (not a `[]` destructuring default) so callers omitting the prop
// get a stable identity across renders.
const NO_AGENT_SESSIONS: AgentSessionView[] = [];

// The overlay is receive-only: rust's native click monitor sees the same
// physical click and emits `tab-selection-changed`, so the DOM click decides
// nothing — the button keeps a handler only for `:active` press feedback and
// the accessible name. Module-level for a stable identity.
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
  // Driven by rust's `hover-changed` event in the shipped app; never CSS :hover.
  hovered?: boolean;
  // Rust owns the selection (`tab-selection-changed` channel, threaded from
  // App.tsx); this component never listens for itself — display state only.
  selectedTab?: Tab | null;
  // `useAgentState`'s snapshot pair, threaded from App.tsx like `status` is.
  agentSessions?: AgentSessionView[];
  agentCapturedAtMs?: number;
  // The Agent tab's viewed-session cursor (`useAgentViewedSession`), threaded
  // from App.tsx; `TabBelowBlock` defaults an absent value to session 0.
  viewedSessionIndex?: number;
}) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const currentBody = showing ? slot.body : null;
  const news = showing && slot.eventType === "news_item";
  // Live-match branch detected by the structured `espn` block's presence, never
  // by string-sniffing eventType/signal.
  const isLiveCard = showing && slot.espn !== undefined;

  const [pulse, setPulse] = useState<Pulse>(null);

  // Render-independent mirror of `pulse` so the re-trigger effect can check the
  // applied class without taking `pulse` as a dependency; every write goes
  // through `setPulseNow` so the two can't drift.
  const pulseRef = useRef<Pulse>(null);
  const setPulseNow = useCallback((next: Pulse) => {
    pulseRef.current = next;
    setPulse(next);
  }, []);

  // `ripple-out` animationend events seen this celebration — reset when a goal
  // pulse (re)arms, counted up in `clearPulseWhenItsAnimationEnds`.
  const rippleEndsSeenRef = useRef(0);

  // Backs `isRotation`/`isInterrupt` below. `key` starts `undefined` so mount
  // never reads as a same-key re-render; `anchor` is this key's own countdown
  // snapshot, read back once when a different key replaces it to estimate how
  // much of its turn was left at that instant.
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

  // Keyed on [currentId, currentSignal], never on priority — a High-priority
  // non-football alert must never play the goal celebration — and not on
  // `expanded`, so the manual hotkey doesn't replay the burst.
  // biome-ignore lint/correctness/useExhaustiveDependencies: currentId is the deliberate re-trigger key documented above — a new item with the same signal must replay the pulse; dropping it would change that behavior.
  useEffect(() => {
    const nextPulse: Pulse =
      currentSignal === "goal" ? "pulse-goal" : currentSignal === "red_card" ? "pulse-red" : null;

    // A fresh celebration starts its ring count over.
    rippleEndsSeenRef.current = 0;

    // Same-signal replay: re-applying an identical class is a React Object.is
    // bailout — no re-render, no CSS restart. Clearing to null and re-applying
    // next frame remounts the class, which is what actually restarts a CSS
    // animation; only taken when the class is genuinely unchanged.
    if (nextPulse !== null && pulseRef.current === nextPulse) {
      setPulseNow(null);
      const frame = requestAnimationFrame(() => setPulseNow(nextPulse));
      return () => cancelAnimationFrame(frame);
    }

    setPulseNow(nextPulse);
  }, [currentId, currentSignal, setPulseNow]);

  const [liveCelebration, setLiveCelebration] = useState<Celebration>(null);

  // Same [currentId, currentSignal] re-trigger discipline as the pulse effect —
  // `currentBody` arrives paired with the id on the same slot object and can't
  // change without a new one, so it isn't a re-trigger key in its own right.
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
      // `ripple-out` ends once per ring, all bubbling here — only the last one
      // means the goal is over. `pulse-red` has no ripple layer, so it clears
      // on the first arrival.
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

  // Agent accent's below-block hairline gate — deliberately NOT part of
  // `cardClass`: the shell owns the priority accent channel only, and origin
  // never shares it (see the CSS on `.below-block.agent-origin`).
  const agentOrigin = showing && slot.origin === "agent";

  // Also feeds the below-block's AnimatePresence `key` directly, deliberately
  // duplicating the hook's identical internal derivation.
  const swapKey = showing ? slot.id : "idle";

  // `promoting` scopes the bouncy `--ease-notchtap-pop` width curve to a genuine
  // promotion entrance (`.card-assembly.promoting`, card-chrome.css). Armed in
  // the render body, NOT an effect: the class must land in the SAME commit that
  // flips the shell's width formula, because CSS keeps a running transition's
  // original timing function even if the rule underneath changes mid-flight.
  const [promoting, setPromoting] = useState(false);

  // Disarm: EXPAND_MS after each swap the entrance has settled, so the pop stops
  // being the curve later width changes (hover flip, manual expand) resolve
  // against. A showing->idle exit disarms synchronously in the render body,
  // guaranteeing `.promoting` and `.exiting` never coexist (card-chrome.css's
  // `.promoting` rule leans on that).
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
    // the arrival-pop arm (see `promoting`'s own doc above). Same
    // condition `enterAsPromotion` names further down — a genuine
    // idle->showing promotion, or a Priority Preemption's incoming card
    // (which enters as a promotion too) — but computed off THIS render's
    // fresh locals, since the derived `enterAsPromotion` below reads the
    // ref that was only just written. An ordinary same-tier rotation and
    // every non-showing key (the exit to "idle") both disarm it, so the
    // class is only ever on the shell during a real arrival's width grow.
    setPromoting(showing && (!isRotationNow || isInterruptNow));
  } else if (showing && wasShowingRef.current.anchor?.remainingMs !== slot.remainingMs) {
    // plan 146b: re-anchor THIS key's own countdown snapshot whenever its
    // remainingMs actually changes without the key itself changing — a
    // topic supersede's top-up or a manual-expand extension (same
    // re-anchor triggers TtlBar.tsx's own effect uses: `[slotId, ttlMs,
    // remainingMs]`). Deliberately does NOT touch `isRotation`/
    // `isInterrupt`/`wasShowing` — those are pinned for this key's whole
    // mounted lifetime (see the guard's own doc above); only the anchor
    // used to judge the NEXT key's swap needs to stay current.
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
  // plan 146b: the entering child's own animation should treat a Priority
  // Preemption's incoming card exactly like an ordinary promotion (the
  // full slide-in), never like the lighter same-tier rotation, even
  // though `isRotation` is structurally true for it too (see
  // `isInterrupt`'s own doc above). Named once here so every consumer in
  // the JSX below (initial/animate/duration/rotation-swap class/data
  // attribute) reads the same derived value.
  const enterAsPromotion = !isRotation || isInterrupt;

  // plan 120: the showing<->idle exit-choreography state machine —
  // extracted to src/useExitChoreography.ts (see that file for every
  // comment documenting each of these values; moved verbatim, not
  // rewritten). `renderedSlot`/`exiting` (the hook's own intermediate
  // values) are NOT destructured here — every downstream consumer that
  // used to read them directly
  // (geometryPriority/expanded/shellExiting/bare/trueIdle) is itself now
  // a hook output, so nothing in this file needs the raw pair anymore;
  // destructuring them unused would trip tsconfig's `noUnusedLocals`.
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

  // ---- plan 171 (tab-notch redesign, slice K) ----------------------
  // The icon strip's five tiers, derived from the ambient status wire by
  // one pure table (`lib/iconPresence.ts`, spec section 6). Memoized on
  // `status` alone because it depends on nothing else and `status`
  // changes only on a genuine wire emission, unlike this component's own
  // per-tick re-renders.
  const iconPresence = useMemo(() => iconPresenceFor(status), [status]);
  // plan 175: how many of those five tiers actually render as
  // `.is-present`, fed to the shell as `--present-icons` so the two
  // strip-visible `--cw` formulas (card-chrome.css) can grow the flanks
  // with the strip instead of painting a flat 85px one the rust hit-test
  // (`hover.rs::hovered_right_flank_width`) had already outgrown. Derived
  // from the SAME `iconPresence` table IconStrip renders from — never a
  // second presence predicate — and `state !== "hidden"` is verbatim
  // IconStrip's own `is-present` condition (`iconClass`), so the count
  // and the DOM can't disagree. It also equals what rust's
  // `tabs::present_tabs` returns for the same status: both sides call a
  // paused-but-loaded track present, which is the one case where the
  // frontend's finer three-tier reading could have diverged.
  const presentIconCount = useMemo(
    () => Object.values(iconPresence).filter((state) => state !== "hidden").length,
    [iconPresence],
  );
  const newsWaitingCount = status?.news.chargeCount ?? 0;

  // The IDLE-and-hovered branch. Everything tab-related below hangs off
  // this one boolean, which is what keeps the push path untouched: a
  // Showing card renders exactly as it did before this plan regardless
  // of what is selected (spec section 7's closing rule — "tab selection
  // decides what the notch shows when the operator goes looking; it
  // never decides what the notch is allowed to tell them").
  // `renderedShowing`, not the live `showing`, for the same
  // delayed-swap-settle reason every other idle-flavored mount gate in
  // this file uses.
  const tabPullOpen = !renderedShowing && hovered;
  // Spec section 7's "none" page falls out of this, rather than being
  // built as its own case: with nothing selected, `TabBelowBlock`
  // returns null and `IdleHoverPeek` keeps its shipped ambient chain.
  const pulledTab = tabPullOpen ? selectedTab : null;
  const peekPreference = peekPreferenceFor(pulledTab);
  // Plan 177: whether the pulled tab actually has anything to draw.
  // Each of the first two arms is LITERALLY the empty-guard its own
  // component already applies — `AgentBelowBlock`'s `sessions.length
  // === 0`, and the hard-wired `NO_NEWS_STORIES` that `TabBelowBlock`
  // hands `NewsBelowBlock` (no wire source exists for news story CONTENT
  // yet — a recorded direction option, deliberately not faked). Kept in
  // lockstep on purpose: when a news story wire lands, the `news` arm
  // and that constant flip TOGETHER, and this is the second of the two
  // places to edit. The final arm is `true` for football, which has no
  // below-block at all — it is served by `IdleHoverPeek`'s own `prefer`
  // rendering (see `peekPreferenceFor`), so "has content" is not this
  // predicate's question for it.
  const pulledTabHasContent =
    pulledTab === "agent"
      ? agentSessions.length > 0
      : pulledTab === "news"
        ? false
        : pulledTab !== null;
  // Whether a real `.below-block` is about to mount for the pulled tab.
  // Both this and the peek below hang off it, which is what guarantees
  // exactly one of the two is ever on screen.
  const pulledBelowBlockOpen = tabBelowBlockHandles(pulledTab) && pulledTabHasContent;
  // The peek stays open for no-selection (its shipped ambient behavior,
  // spec section 11's explicit non-goal) and for the two selections it
  // itself serves; the three with their own below-block close it, so
  // there is never more than one `.below-block` under the shell (the
  // rounding law in card-chrome.css depends on that). Plan 177: "close
  // it" now means "close it for a below-block that will actually
  // RENDER something" — a tab whose source is empty (news today, agent
  // with no live session) degrades to the ambient peek rather than to a
  // blank shell.
  const peekOpen = tabPullOpen && !pulledBelowBlockOpen;

  // plan 091: the outer shell (`.card-assembly`) now owns ONLY geometry-
  // and-effects classes — priority accent, hover diagnostic, the goal/
  // red-card pulse and the live-match celebrations. `news-shade` (and
  // its mood/texture riders) moves to `belowBlockClass` below: it is
  // content presentation, not shell, and the below-block is the block
  // that actually carries that content now (Step 4's ownership split).
  // The old idle/idle-status width split (plan 034) is gone — the new
  // idle has one width formula regardless of status chips (Geometry
  // contract point 5), so there is no more "status" class to compute here.
  const cardClass = [
    "card-assembly",
    geometryPriority,
    expanded && "expanded",
    // 2026-08-02 (animation audit, finding 1): the transient arrival-pop
    // marker — see `promoting`'s own doc above, and card-chrome.css's
    // `.card-assembly.promoting` rule for what it actually changes (the
    // width transition's timing function, for the entrance window only).
    promoting && "promoting",
    // the hover modifier, off the live `hovered` prop — never CSS
    // `:hover`, since the overlay window is click-through and never
    // receives real pointer events. It no longer scales the shell (the
    // "breathing" rule was removed 2026-08-02 — choreography.css's own
    // note); what it still drives is the bare-notch rail reveal
    // (`.bare.hovered`, card-chrome.css), and, via `expanded` above,
    // hover-expand on a showing card.
    hovered && "hovered",
    // plan 105 (Step C): the bare-notch modifier — transparent flanks,
    // cutout-width-only shell (styles.css), so the mode reads as the
    // native notch until hovered.
    bare && "bare",
    // 2026-07-23 review fix (wave B, Task 1): see `shellExiting`'s own
    // doc above — drives the immediate width-shrink + corner-round start
    // on the true showing->idle exit leg only.
    shellExiting && "exiting",
    // plan 123: see `exitToBare`'s own doc above — only ever paired with
    // `exiting` (never appears alone), so it's a pure narrowing modifier,
    // not a separate state; `restingState === "rail"` never sets it.
    exitToBare && "exit-to-bare",
    // plan 084: `pulse`/`cele-*` are mutually exclusive, never stacked —
    // the live-match branch (structured espn meta) plays its own
    // `cele-goal`/`cele-yc`/`cele-rc`; every other football-signal card
    // (flag-off path, or a non-espn source that happens to share a
    // signal) keeps the shipped pulse-goal/pulse-red exactly as before.
    !isLiveCard && pulse,
    isLiveCard && liveCelebration,
  ]
    .filter(Boolean)
    .join(" ");
  // plan 091: below-block's own class list — the news mood
  // presentation, still derived off the LIVE slot (not `renderedSlot`) for
  // the same "no delayed-swap lag" reason the comment above always gave;
  // only WHERE these classes attach moved (below-block, not the shell).
  const belowBlockClass = [
    "below-block",
    news && "news-shade",
    news && categoryClass(slot.category),
    // 2026-07-24: the generic branch's compact masthead now reuses news's
    // `.masthead .dot` markup (see NotificationBody.tsx), whose color
    // reads the same `--cat`/`--cat-deep` custom properties `categoryClass`
    // sets — without a class here they're unset for every non-news card,
    // leaving the dot invisible.
    // Plan 147: the flat `cat-generic` fallback is replaced by
    // `sourceClass`, which resolves a per-origin (and, for agent origin,
    // per-runtime) identity colour (source-identity.css's `.src-*`
    // classes) instead of one shared neutral gray — news keeps
    // `categoryClass` above untouched. Gated on `showing` too (not just
    // `!news`) purely so `slot.origin`/`slot.agentRuntime` type-narrow;
    // `belowBlockOpen` (below) never mounts this block while idle, so the
    // fallback string was already inert there — no behavior change.
    !news && showing && slot.origin !== "news" && sourceClass(slot.origin, slot.agentRuntime),
    agentOrigin && "agent-origin",
  ]
    .filter(Boolean)
    .join(" ");

  // plan 12x (wave 2): the swapped card BODY (everything inside
  // `.card-content`, below) now reads the LIVE `slot` directly, like
  // `news`/`isLiveCard` above — there is no more `renderedSlot`
  // stand-in for content. `AnimatePresence` (in the JSX below) is what
  // now supplies the "outgoing content stays frozen through its own
  // exit" behavior: an exiting `motion.div` keeps whatever it last
  // rendered (captured automatically once its parent stops including
  // it), so freezing content by hand is no longer this component's job.
  // `renderedSlot`/`renderedShowing`/`exiting` (from the STILL-KEPT
  // `useDelayedSwap` above) are now scoped to ONE job only: the plan-107
  // GEOMETRY choreography (`geometryPriority`/`expanded`/`bare` above,
  // and the below-block/StatusDots mount gates below) — never content.
  const newsCategory = news ? categoryLabel(slot.category) : null;
  const newsAge = news ? ageLabel(slot.publishedAtMs, Date.now()) : null;
  const liveVisibleDetails = showing ? slot.details : [];

  // plan 069 (folded into 078; re-scoped to live `slot` in wave 2): memoized
  // so unrelated re-renders don't re-tokenize the markdown.
  // 2026-07-23: dependency narrowed from the whole `slot` object to
  // `currentBody` (the actual string fed into `renderInlineMarkdown`,
  // already computed above) — mirrors Manifest.tsx's own `[body]`
  // dependency. `slot` changes on every wire tick (queue counters, TTL
  // countdowns, etc.), which was re-tokenizing this markdown on every one
  // of those emissions even though the body text itself hadn't changed.
  const bodyContent = useMemo(() => renderInlineMarkdown(currentBody ?? ""), [currentBody]);

  // plan 084: the live-match branch — `isLiveCard` (above) already reads
  // the live slot, so it doubles as both the outer shell's accent gate
  // AND the content branch selector below; there is no more a separate
  // delayed/live pair that could "briefly disagree" (wave 2 dropped that
  // window entirely — see the comment above).
  const liveEspn: EspnMeta | undefined = showing ? slot.espn : undefined;
  // plan 170: the OLD `footballKind` local (fed only the now-deleted
  // `eventPresentation` derivative — the `.event-line` icon+tint prop
  // `LiveMatchScorecard` took, which `FootballHeroCard` has no equivalent
  // slot for, see that component's own doc) is gone too — `tsc` confirmed
  // it dead once `eventPresentation` was removed. The celebration effect
  // above (`setLiveCelebration`, ~line 333) is UNAFFECTED: it calls
  // `footballEventKindFor`/`eventKindPresentationFor` itself, on its own
  // local `kind`, never reading this variable.
  const pillVariant = showing && isLiveCard ? livePillVariantFor(slot.signal) : "live";
  const pillLabel = pillVariant === "break" ? "Break" : pillVariant === "final" ? "Final" : "Live";
  const cardsClean =
    liveEspn !== undefined &&
    liveEspn.homeCards[0] === 0 &&
    liveEspn.homeCards[1] === 0 &&
    liveEspn.awayCards[0] === 0 &&
    liveEspn.awayCards[1] === 0;

  // plan 091: the below-block mounts if and only if `renderedShowing` is
  // true — which, thanks to `useDelayedSwap` freezing `renderedSlot` at
  // its pre-transition value for the whole `exiting` window, already
  // covers BOTH "currently showing" and "exiting FROM showing back to
  // idle". It does NOT need `|| exiting` on top: during the opposite
  // transition (idle exiting INTO showing), renderedSlot is still frozen
  // idle-flavored — there is no below-block content to fade out of idle,
  // because idle never had any (flank-right's dots below play that side
  // of the transition instead, gated on `!renderedShowing`, the
  // mirror-image condition).
  // plan 12x (wave 2): this wrapper's PRESENCE stays on the kept
  // `renderedShowing` (unchanged from plan 091/107) so the swapped
  // content's own `AnimatePresence`, just inside it, always has a parent
  // that outlives its exit animation — `belowBlockClass` itself is still
  // computed off the LIVE slot above, so the mood/texture classes keep
  // updating in lockstep with `slot`, not delayed by the content swap,
  // exactly as before.
  // plan 11x: PRESENCE itself now reads `belowBlockOpen`, not
  // `renderedShowing` directly — identical to `renderedShowing` for
  // every case above (entrance, steady showing, same-id/rotation
  // swaps — see `belowBlockOpen`'s own doc above for why those are
  // unaffected), except the true showing->idle close, which now settles
  // CONTENT_EXIT_MS after `showing` goes false instead of the full
  // SWAP_EXIT_MS. The wrapper is a `motion.div` now (was a plain `div`)
  // so that close can fade rather than snap: `initial={false}` skips any
  // enter animation (below-block still appears at full opacity on the
  // very render it mounts, byte-identical to the old plain `div`), so
  // only ITS OWN exit (below-block visibly clearing) is new — the inner
  // `AnimatePresence`'s content swap just below is completely untouched
  // (same duration, same easing, both directions), so entrance content
  // fade-in and same-priority rotation fades are unaffected; the outer
  // fade's job is only to make sure nothing is left visible when this
  // wrapper actually unmounts CONTENT_EXIT_MS later, so overlay-card.css's
  // `:not(:has(.below-block))` flank corner-round (which can only safely
  // start once the below-block is truly gone — see that rule's ROUNDING
  // LAW comment) begins right after, not a further ~330ms late.
  //
  // plan 12x (wave 3, operator-feedback polish pass): the wrapper's own
  // `exit` variant (JSX below) now also animates `height` (auto -> 0,
  // motion measures the rendered box itself — same implicit-from-value
  // read it already relied on for `opacity`), not opacity alone. Before
  // this, the wrapper kept its full showing-height for the entire
  // CONTENT_EXIT_MS fade, then vanished outright the instant it
  // unmounted — an abrupt height "pop," not a collapse. `.below-block`'s
  // own `overflow: hidden` (overlay-card.css) both clips the shrinking
  // content and gives it an automatic min-height of 0 (CSS Grid: a
  // non-visible-overflow item's auto min-size is 0), so the surrounding
  // `.card-assembly` grid row — and the whole card — shrinks in step,
  // frame by frame, rather than jumping straight to its post-below-block
  // height on unmount. Paired with the inner `AnimatePresence`'s own
  // `exit` fix just below, this also fixes the pre-pop content reflow:
  // previously the INNER content swap's `exit` variant (`{opacity: 0, y:
  // -4}` over SWAP_EXIT_MS) kept animating past this wrapper's own
  // shorter CONTENT_EXIT_MS close, so the wrapper unmounted the whole
  // subtree mid-flight through the inner fade+shift — a visible jump
  // right before the pop. The inner exit is now `{opacity: 0}` (no
  // y-shift — nothing to reflow) over CONTENT_EXIT_MS (matching this
  // wrapper exactly), so the two finish in lockstep: content quietly
  // fades in place while the box collapses around it, one motion, never
  // cut off mid-animation. The inner `animate` (entrance) transition is
  // untouched — still SWAP_EXIT_MS — so promotions and same-priority
  // rotations keep their existing feel; only the true close changed.

  // plan 127 (Step 5, /improve-animations audit finding #6): `role`/
  // `aria-live` moved OFF the card root (see below-block's own JSX for
  // where they landed) — this used to sit on `.card-assembly` itself,
  // which also encloses FlankClock (a 30s-ticking clock) and, for a
  // live-match card, the scorecard's own constantly-updating minute/
  // score chrome, both of which would re-announce to assistive tech on
  // every routine wire tick, not just genuine new-notification arrivals.
  // `liveRegionActive` gates the region to exactly the case that should
  // announce: a non-live-match card (news/generic/agent —
  // stable title/body text that only changes on a genuine new item or
  // rotation) that's actually mounted.
  //
  // plan 129 (K2, deep-review fix): the wave above landed this on
  // `belowBlockOpen && !isLiveCard` and put the attributes on the
  // AnimatePresence-keyed `motion.div` that carries `belowBlockClass` —
  // that node MOUNTS at t=175ms (`belowBlockOpen` lags `showing` by the
  // exit-choreography settle) in the SAME commit as the title/body
  // content already inside it. A live region inserted already-populated
  // is the canonical unreliable ARIA pattern — most screen readers only
  // pick up mutations to an already-established region, not a region
  // that arrives pre-filled. Rotations happened to still announce only
  // because `mode="wait"` delays the swapped child's own mount past the
  // point where the region (on the old placement) had already flipped
  // on with the FIRST item's content — the region existed early only by
  // accident of a later render, not by design. The fix: gate on
  // `showing` (this component's own live boolean, flips the instant the
  // slot enters/leaves "showing" — t=0, no exit-choreography lag)
  // instead of `belowBlockOpen`, and move the attributes onto a NEW,
  // always-mounted static wrapper one level up (see the JSX below) that
  // exists before AND after the AnimatePresence-keyed content node ever
  // mounts — so the attribute-flip and the content-mount are back in
  // two different commits, exactly like the pre-127 root-level pattern
  // this doc describes above (`role`/`aria-live` flip at t=0, content
  // arrives later into an already-established region). `isLiveCard` is
  // itself derived off the live `slot`, gated on `showing` — stays in
  // sync with it; no staleness window between the two.
  const liveRegionActive = showing && !isLiveCard;

  return (
    <div
      className={cardClass}
      // plan 175: the icon-count term card-chrome.css's two strip-visible
      // `--cw` rules read. Always set (not gated on `tabPullOpen`), because
      // the shell's width must already know the strip's size on the very
      // render hover lands — the growth is what the width transition
      // animates, and a value arriving one render late would make the
      // flank snap. Harmless in every non-strip state: no other `--cw`
      // formula references it. Same `as React.CSSProperties` custom-property
      // idiom PositionBar/IdleHoverPeek already use.
      style={{ "--present-icons": presentIconCount } as React.CSSProperties}
      onAnimationEnd={clearPulseWhenItsAnimationEnds}
    >
      {/* 2026-07-23 review fix (wave B, Task 2 — real concave S-curve):
          the top "gill" corners — real DOM siblings of the flanks, NOT
          `.card-assembly::before`/`::after` (both already claimed by the
          goal-celebration burst/ring — see those rules in
          overlay-card.css) and NOT pseudo-elements on the flanks
          themselves (`.flank-left`/`.flank-right` have `overflow:
          hidden`, which would clip anything anchored outside their own
          box — these gills must poke a few px past the flank's outer
          edge to read as a flare, so they need `.card-assembly` itself,
          which has no overflow rule, as their positioning ancestor).
          Always rendered ("always render, CSS decides" idiom, matches
          `.synthetic-cutout` below); pure decoration
          (`aria-hidden` + CSS `pointer-events: none`). See
          overlay-card.css's own comment for the concave-fillet geometry. */}
      <span className="notch-gill notch-gill-left" aria-hidden="true" />
      <span className="notch-gill notch-gill-right" aria-hidden="true" />
      <div className="flank-left">
        {/* plan 105 (Step C): bare mode draws no clock — CSS alone can't
            hide it (the flanks going transparent still leaves text
            painted), so this is a real render-time gate, unlike the
            synthetic-cutout's "always render, CSS decides" idiom below.
            2026-07-23 (operator minimal-notch spec, Task 1.2): gate moved
            from `!bare` to `railRevealed` — a bare-hover now mounts the
            clock too (expanding the minimal notch into the full idle
            rail), fading in via `AnimatePresence`/`motion` rather than
            popping, coordinated with the width growth CSS already drives
            on `.card-assembly.bare:has(.idle-peek)` (overlay-card.css).
            Every other case (`!bare` was already true) is unaffected —
            `railRevealed` is true throughout showing/exiting exactly as
            `!bare` was, so this is a pure addition, not a behavior
            change, off bare-idle.
            plan 124 (F3, review fix): `&& !exitToBare` added. During the
            exit-to-bare window (`useExitChoreography.ts`'s `exitToBare`
            doc has the full mechanism) the flank paint itself animates to
            transparent over overlay-card.css's `.exiting.exit-to-bare`
            rule, but this mount was gated on `railRevealed` alone — true
            for that entire window (bare is false throughout showing/
            exiting) — so the clock stayed mounted fully opaque while its
            background faded out from under it: white text sitting on a
            see-through flank mid-window. Unmounting it the instant
            `exitToBare` goes true lets its own 260ms exit fade (below)
            overlap the flank's fade instead of lagging a full render
            behind AnimatePresence's own exit trigger. Every other case is
            unaffected: `exitToBare` is always false in rail mode (its own
            doc pins that), so this is a pure narrowing on the notch-mode
            exit leg only. */}
        <AnimatePresence>
          {railRevealed && !exitToBare && (
            <motion.span
              key="flank-clock"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              // plan 127 (Step 1, /improve-animations audit findings #4/
              // #9): was a hand-typed `{ duration: 0.26, ease: "easeOut" }`
              // — now single-sourced off REVEAL_MS (the same reveal/paint
              // coordination duration the flank background/padding fade
              // and `.track span` background fade use, overlay-card.css)
              // and NOTCHTAP_EASE (matching the flank paint's own
              // `--ease-notchtap`, which this fade is coupled to — both
              // read the exact same bare<->hovered/reveal trigger, so
              // they should ease identically, not one bezier and one
              // built-in "easeOut").
              transition={{ duration: REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              <FlankClock />
            </motion.span>
          )}
        </AnimatePresence>
      </div>
      {/* plan 091: the notch cutout itself — real hardware empty space in
          notch mode (nothing painted here), an app-drawn pure-#000 block
          in HUD mode (`:root[data-notchtap-mode="hud"] .synthetic-cutout`,
          styles.css). Always rendered; CSS alone decides whether it
          paints, so there is no mode branch in this component (Decision
          6 — "no mode branch" in the shape itself). */}
      <div className="synthetic-cutout" aria-hidden="true" />
      {/* the idle face — purely additive decoration in the same grid cell
          as .synthetic-cutout above (grid-column 2 / grid-row 1,
          overlay-card.css); it owns none of the geometry/swap machinery,
          only reads it via `trueIdle`. Gated on `idleFaceEligible`
          (2026-07-23 review fix): CSS never paints it on real notch
          hardware, so it's not rendered at all there — otherwise its
          internal reveal/gaze/blink timers would run forever for a node
          that can never be seen.
          Plan 171 (slice K): the face sits in a `.rest-cluster` row,
          which takes over the face's own `grid-column: 2 / grid-row: 1`
          placement. The shipped `.rest-cluster` is authoritative.
          `<IdleFace>` ITSELF is untouched: its grid declarations simply
          go inert as a flex child, and its `display: none` -> HUD
          `display: flex` gate still governs whether it paints at all.
          The cluster carries the same HUD-only gate for the same
          reason. */}
      {idleFaceEligible && (
        <div className="rest-cluster" aria-hidden="true">
          <IdleFace idle={trueIdle} />
        </div>
      )}
      <div className="flank-right">
        {/* Plan 171 (tab-notch redesign, slice K — spec section 2
            decision 1 and section 6): the status dots LEAVE this surface.
            Rest is bare (shell + face + eq bars, nothing else), and the
            right flank's one job on hover is now the icon strip. The
            `<StatusDots>` COMPONENT and its own tests are deliberately
            untouched and NOT deleted — `AgentBoard.tsx` still mounts it
            (its own rail is a different surface this spec doesn't
            reach), so removing it here orphans nothing.

            Mount gate, per spec section 5 ("the icon strip arrives
            INSIDE the right flank") and section 2 decision 2: the strip
            is in the DOM whenever this surface is idle, and
            `icon-strip.css` alone decides when it becomes VISIBLE — a
            `visibility: hidden` + `opacity: 0` + `pointer-events: none`
            baseline that only lifts under `.card-assembly.hovered`, with
            the strip's own opacity leg staggered ICON_STRIP_STAGGER_MS
            behind the flank's black paint so no glyph is ever visible
            against the desktop. That discipline is entirely CSS-owned,
            so there is no `AnimatePresence` here (the dots' old one is
            gone with them): adding a JS opacity tween on top would fight
            the stylesheet's own staggered fade rather than complement
            it. `!renderedShowing` is the one thing CSS can't express —
            a hovered SHOWING card also carries `.hovered`, and the strip
            must not appear over a real notification (spec section 7:
            "hover always shows the selected tab's card" is an IDLE
            gesture; a pushed card is unaffected by selection). Same
            `!exitToBare` narrowing FlankClock above documents. */}
        {railRevealed && !exitToBare && !renderedShowing && (
          <IconStrip
            {...iconPresence}
            newsCharge={status?.news.chargeFraction ?? 0}
            newsCharged={status?.news.isCharged ?? false}
            // spec section 12 open question 5's shipped default is "ship
            // both" the fill and the badge — but a zero count is nothing
            // to announce, so `null` (which omits the badge entirely,
            // per IconStrip's own prop doc) is the right rendering of
            // "no items waiting", not a literal `0`.
            newsCount={newsWaitingCount > 0 ? newsWaitingCount : null}
            selected={selectedTab}
            // RUST owns selection (spec section 10): the same physical
            // click that lands on this button is also seen by rust's own
            // native click monitor, which decides what it selected and
            // emits `tab-selection-changed` back. So the DOM side is
            // purely presentational — it exists for the `:active`
            // press-scale feedback and the accessible name, not to
            // decide anything. Passing a real handler here would create
            // a second, divergent copy of "what's selected"; passing
            // none at all would lose the press feedback that makes the
            // click feel registered. Hence a deliberate no-op.
            onSelect={noopSelect}
          />
        )}
      </div>
      {/* plan 093 (079 items 9/17/18): the idle hover-expanded state —
          `open` is gated on `renderedShowing` (not the live `showing`)
          for the same reason `StatusDots` above is: it must stay in step
          with the delayed-swap settle, not flicker on/off mid-transition.
          Driven by the live `hovered` prop, never CSS `:hover`.
          plan 127 (Step 2, finding #2): ALWAYS rendered now — the old
          `{!renderedShowing && <IdleHoverPeek .../>}` conditional
          unmounted this component (its internal AnimatePresence
          included) the instant a promotion arrived mid-peek, tearing out
          up to 100px of content with zero animation. The mount gate
          moved INSIDE IdleHoverPeek as its own `open` prop (same
          `!renderedShowing && hovered` condition, just evaluated one
          level deeper) so a promotion now lets IdleHoverPeek's own exit
          animation play while the card content mounts above it, instead
          of both changes landing as one unanimated swap. See
          IdleHoverPeek.tsx's own doc on `open` for the full mechanism.
          Plan 171 (slice K): `open` is now `peekOpen` — the same
          `!renderedShowing && hovered` condition it always was, narrowed
          by "and the selected tab isn't one with its own below-block",
          so only ever ONE `.below-block` sits under the shell at a time
          (the `:not(:has(.below-block))` rounding law in card-chrome.css
          depends on that). `prefer` routes the football selection into
          this component's OWN shipped rendering rather than a second
          copy of it — spec section 11's explicit "IdleHoverPeek's
          mechanism is untouched". With nothing selected, both props are
          inert and this is byte-identical to before the plan. */}
      <IdleHoverPeek status={status} hovered={hovered} open={peekOpen} prefer={peekPreference} />
      {/* Plan 171 (slice K), spec section 7: the selection-driven
          below-block. Mounted OUTSIDE the live-region wrapper below on
          purpose — that region announces genuine new NOTIFICATIONS
          (`liveRegionActive` is gated on `showing`), and a pulled card is
          by definition something the operator went looking for, not
          something arriving unannounced. Rendering it here also keeps
          the push path byte-identical: `tabPullOpen` is false for the
          whole life of a Showing card, so this is simply absent then. */}
      {/* Animation lock-down (2026-08-03): keyed on the selected tab so
          switching icons CROSS-FADES rather than jump-cutting — this is
          the one moment the whole feature is about, and every other
          content swap in this file is animated. `mode="wait"` matches the
          rotation swap's own discipline (one card on stage at a time).
          Emphasis is below-cutout content only, per animationTiming.ts's
          standing law — the shell itself is untouched. Durations are the
          rotation tokens, not new literals (spec section 13). */}
      <AnimatePresence mode="wait" initial={false}>
        {/* Plan 177: `pulledTabHasContent` joins the mount gate so an
            empty tab mounts NOTHING here and keeps the ambient peek
            above instead — one surface at a time either way, which is
            what the rounding law depends on. Football is unaffected (the
            predicate is true for it; its wrapper mounts exactly as
            before and `TabBelowBlock` returns null for it as it always
            has). */}
        {pulledTab !== null && pulledTabHasContent && (
          <motion.div
            // plan 176: the placement class below (card-chrome.css owns
            // the rule) is what spans this wrapper across the shell's row
            // 2, the same cell every other below-block occupies — it
            // carries placement and box behaviour only, never chrome. It
            // has to live HERE, on the animating element, rather than on
            // the `.below-block` each `TabBelowBlock` branch renders:
            // that block is a GRANDCHILD
            // of the `.card-assembly` grid, and grid placement only
            // reaches direct items — so without a class on this wrapper
            // the whole pulled card was auto-placed into the left flank
            // column and rendered squeezed (zero-width, in bare notch
            // mode). The live-region wrapper below solves the same problem
            // the opposite way (`display: contents`, keeping ITS animating
            // child the grid item); that inversion is unavailable here,
            // because a `display: contents` box would erase this element's
            // own opacity/y animation.
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
      {/* plan 129 (K2, deep-review fix): this `display: contents` div is
          the ACTUAL live-region wrapper now — `liveRegionActive`'s own
          doc (above) has the full mechanism. It is a plain, always-
          mounted static element (never conditionally rendered, unlike
          everything it wraps), so `role`/`aria-live` land in the DOM on
          the same render `showing` itself flips, before the
          AnimatePresence-keyed `belowBlockOpen` content below ever
          mounts — a real screen reader sees an EMPTY, already-live
          region first, then a mutation into it ~175ms later, never a
          region that arrives pre-populated. `display: contents` means
          this node contributes nothing to layout or the box tree (no
          new flex/grid item, no new stacking context) — `.card-assembly`
          (a CSS grid) still sees straight through to the `.below-block`
          motion.div as its direct box-participating child, and
          `overlay-card.css`'s `:not(:has(.below-block))` flank-rounding
          law still matches/misses exactly as before, since `.below-block`
          itself stays on the same node, just with one more non-
          participating ancestor between it and `.card-assembly`. */}
      <div
        style={{ display: "contents" }}
        role={liveRegionActive ? "status" : undefined}
        aria-live={liveRegionActive ? "polite" : undefined}
      >
        <AnimatePresence>
          {belowBlockOpen && (
            <motion.div
              className={belowBlockClass}
              // plan 127 (Step 5, finding #6): this used to be the STATIC
              // live-region wrapper `liveRegionActive`'s doc refers to —
              // it mounts once per showing session (gated on
              // `belowBlockOpen`, not `swapKey`) and stays mounted through
              // every same-session rotation, so it was NOT the
              // AnimatePresence-keyed node (the inner `motion.div key=
              // {swapKey}` just below) that remounts per swap and would
              // re-announce its ENTIRE content as a brand-new region on
              // every rotation rather than reporting a content update
              // within a stable one. Title/body text changes from a
              // rotation still reach assistive tech exactly as before —
              // aria-live watches for DOM mutations anywhere in its
              // subtree, not just at its own root — so this was a strict
              // narrowing of WHAT can trigger an announcement (excludes
              // FlankClock/StatusDots, structurally outside this wrapper,
              // and the live-match branch via `liveRegionActive`'s own
              // `!isLiveCard` gate), never a loss of the rotation-announce
              // behavior itself.
              //
              // plan 129 (K2): the ROLE/ARIA-LIVE ATTRIBUTES themselves
              // moved OFF this node — this node still mounts at
              // t=175ms (`belowBlockOpen` lags `showing`), so leaving the
              // attributes here reintroduces the exact pre-populated-
              // region bug this comment above used to describe as fixed.
              // The narrowing-of-what-can-announce rationale above still
              // holds (rotations still reach assistive tech via DOM
              // mutation inside the now-outer live region, live-match
              // chrome is still excluded via `liveRegionActive`'s
              // `!isLiveCard` gate) — only WHICH element carries the
              // attributes changed, to the static wrapper just above.
              initial={false}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE }}
            >
              {/* plan 12x (wave 2): the actual content-swap animation — was a
              hand-rolled `useDelayedSwap` freeze + CSS
              `card-enter-showing`/`card-exit-showing` keyframes, now real
              `AnimatePresence mode="wait"`. Keyed on the LIVE `swapKey`
              (unchanged: `slot.id` while showing) — a same-id update
              (e.g. a queue-counter tick) re-renders this SAME node in
              place, no key change, no exit/enter replay, no remount
              (pinned by the "updates the queue slider... without
              remounting" test). A genuine id change (showing(A)->
              showing(B)) — or `showing` itself going false, which yields
              no child at all here — drops the old key; `AnimatePresence`
              freezes whatever that child last rendered and plays its
              `exit` variant, and `mode="wait"` holds any new child back
              until that finishes. That's exactly what the old freeze
              timer did, now framework-owned — which is also why content
              below reads the LIVE `slot` directly (see the comment above
              `newsCategory`) rather than a frozen stand-in: the freeze is
              `AnimatePresence`'s job now.
              plan 127 (Step 3, finding #3): `custom={{ isRotation,
              isInterrupt }}` on this `AnimatePresence` is what lets the
              EXITING child (the OLD `swapKey`, already dropped out of the
              JSX below by the time this render commits) still learn
              whether ITS OWN removal is a rotation (or, per plan 146b, a
              Priority Preemption's interrupt) — see `contentExitVariants`'
              own doc for why this is the only channel available for that.
              Promotion (idle->showing) and exit (showing->idle) always
              pass `{ isRotation: false, isInterrupt: false }`, so this
              AnimatePresence's behavior for those two legs is unchanged.
              plan 146b: the ENTERING child's own `initial`/`animate` below
              additionally check `isInterrupt` — a preemption's incoming
              card is a genuine new Promotion taking the Slot (spec: "the
              new card enters with the normal enter animation"), so it
              must use the slide-in promotion entrance even though
              `isRotation` is (structurally) also true for it, never the
              lighter opacity-only rotation entrance. `enterAsPromotion`
              names that combined condition once so every prop below
              reads it consistently. */}
              <AnimatePresence mode="wait" custom={{ isRotation, isInterrupt }}>
                {showing && (
                  <motion.div
                    key={swapKey}
                    // item 3 (rotation de-noise): `rotation-swap` (only on a
                    // ordinary same-slot rotation, never a promotion OR an
                    // interrupt) gates off the news chips' own `pill-enter`
                    // replay (news-category.css) — see that rule's own doc
                    // for why. Plain string concatenation, not the
                    // array-join idiom this file uses elsewhere
                    // (`cardClass`/`belowBlockClass`), since there are only
                    // ever these two fixed tokens.
                    className={
                      isRotation && !isInterrupt ? "card-content rotation-swap" : "card-content"
                    }
                    // plan 127 (Step 3): a showing->showing rotation skips
                    // the y-slide entirely (opacity-only) — the slide is
                    // the part of the ceremony that reads as repetitive on
                    // a ~10s cadence; the idle->showing promotion keeps its
                    // slide, byte-identical to before this plan.
                    // plan 146b: `enterAsPromotion` (declared just above the
                    // JSX return, doc there) routes a Priority Preemption's
                    // incoming card through this same slide-in branch, not
                    // the rotation's opacity-only one — see this
                    // AnimatePresence's own doc comment above.
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
                    // `data-rotation-swap`/`data-interrupt-swap` are real DOM
                    // attributes, not pure decoration: they're how the test
                    // suite pins this leg-detection logic (motion's own
                    // transition/variant props aren't otherwise inspectable
                    // from rendered output in jsdom) — see
                    // StatusRailCard.test.tsx's "same-slot rotation" and
                    // "Priority Preemption interrupt" describe blocks.
                    data-rotation-swap={isRotation && !isInterrupt}
                    data-interrupt-swap={isInterrupt}
                    // plan 12x (wave 3, exit) / plan 127 (Step 3, rotation
                    // split): the exit variant carries its OWN `transition`
                    // (overriding the shared one below, motion's documented
                    // per-variant override mechanism) — see the wrapper's
                    // own doc comment above for why the non-rotation exit
                    // must match CONTENT_EXIT_MS, not SWAP_EXIT_MS.
                    // `exit="exit"` (a variant label, not an inline object)
                    // is what lets `contentExitVariants`' function read the
                    // AnimatePresence-supplied `custom` above — see that
                    // constant's own doc.
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
      {/* the goal celebration is plan 023's pure-CSS confetti burst +
          ring on `.card-assembly.pulse-goal`'s ::after/::before PLUS plan
          032's ripple: three staggered concentric accent rings, mounted
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
