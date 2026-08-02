import { AnimatePresence, motion, useReducedMotion } from "motion/react";
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
import { weatherArtFor } from "../lib/weatherArt";
import type { AgentSessionView } from "../useAgentState";
import { useExitChoreography } from "../useExitChoreography";
import type { EspnMeta, Priority, SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { EqBars } from "./EqBars";
import { FlankClock } from "./FlankClock";
import type { Tab } from "./IconStrip";
import { IconStrip } from "./IconStrip";
import { IdleFace } from "./IdleFace";
import { IdleHoverPeek, type PeekPreference } from "./IdleHoverPeek";
import { FootballHeroCard, NotificationBody } from "./NotificationBody";
import { TabBelowBlock, tabBelowBlockHandles } from "./TabBelowBlock";

// plan 084: the live scorecard's celebration classes — echoes the shipped
// pulse-goal/pulse-red discipline (keyed on [currentId, currentSignal],
// cleared on the ending keyframe's animationend) but scoped to the
// espn-structured-meta branch so a live-branch goal never stacks BOTH
// `pulse-goal` and `cele-goal` (see the `isLiveCard` gate below).
// `Celebration` (lib/presentation.ts) already IS the class-name union, so
// there's no separate translation table to keep in sync with Step 1's.
const CELEBRATION_END_ANIMATION: Record<NonNullable<Celebration>, string> = {
  "cele-goal": "cele-ring",
  "cele-yc": "cele-ring",
  "cele-rc": "red-strobe",
};

// plan 082: weather ALERT cards ride the plan-035 display-only `details`
// channel to carry condition + day/night art inputs — plan 096 later put
// `origin` on the slot-state wire, but weather art derivation has no
// reason to move off these markers (they carry condition/day-night, which
// `origin: "weather"` alone doesn't). Every pair whose label starts with
// "wx-" is a MARKER, never real content: it must be read to derive the
// mood/glyph art, then excluded from every place `details` is rendered as
// visible text (the marker-leak guard).
function isWxMarker(label: string): boolean {
  return label.startsWith("wx-");
}

// plan 120: exported so NotificationBody.tsx (src/components/) can import
// the shape rather than duplicating this two-field structural type — one
// definition, not two that can drift.
export type Detail = { label: string; value: string };

function visibleDetails(details: Detail[]): Detail[] {
  return details.filter((detail) => !isWxMarker(detail.label));
}

function weatherArtFromDetails(details: Detail[]) {
  const condition = details.find((detail) => detail.label === "wx-condition")?.value;
  if (condition === undefined) {
    return null;
  }
  const isDay = details.find((detail) => detail.label === "wx-is-day")?.value === "1";
  return weatherArtFor(condition, isDay);
}

// plan 12x (wave 2): mirrors shared-ui's `--ease-notchtap`
// (vendor/shared-ui/design/tokens.css) —
// motion's `transition.ease` takes a bezier array, not a CSS var, so this
// is the JS-side twin of that token for the showing-flavored content swap
// below (mirrors card-enter-showing/card-exit-showing's old
// `var(--ease-notchtap)`). 2026-07-23: the literal moved to
// animationTiming.ts's exported NOTCHTAP_EASE (imported above) with a
// token-parity guard test — no local copy here anymore. The idle-flavored
// swap (StatusDots) keeps motion's built-in "easeOut", matching the old
// card-enter-idle/card-exit-idle's plain `ease-out`.

type Pulse = "pulse-goal" | "pulse-red" | null;

// Which @keyframes name (styles.css) ends each pulse — the *only* place
// either duration lives is the CSS animation itself; clearing on
// animationend means there's no JS-side duration to keep in sync with it.
// plan 150 (Step 1): `pulse-goal` moved off `goal-overshoot` (the 1240ms
// SHELL keyframe) onto `ripple-out` — the LAST thing the goal celebration
// plays. The three `.cele-ripple` rings run 1440ms with 280ms/560ms
// stagger, so the family only truly finishes at ~2000ms; clearing on the
// shell's end tore ring 3 out at 62% of its life, mid-expansion (the
// `.cele-ripple` layer is mounted by the `pulse === "pulse-goal"` gate
// further down, so clearing the state unmounts the rings outright).
// Holding the class the extra ~760ms is safe: `goal-overshoot`/
// `goal-burst`/`goal-ring` are all one-shot, finite, and fill-mode-less
// (choreography.css) — once they end they revert to their base rules
// (`::after`/`::before` both rest at `opacity: 0`), and a class that
// merely stays applied never re-runs an animation.
const PULSE_END_ANIMATION: Record<NonNullable<Pulse>, string> = {
  "pulse-goal": "ripple-out",
  "pulse-red": "red-alert",
};

// plan 150 (Step 1): how many `.cele-ripple` rings the goal celebration
// mounts (the three <span>s below) — `ripple-out` therefore ends three
// times per celebration, staggered, and only the THIRD one means "the
// celebration is over". Kept next to the table above because the two are
// read together in `clearPulseWhenItsAnimationEnds`; the JSX below must
// mount exactly this many spans.
const RIPPLE_RING_COUNT = 3;

// plan 127 (Step 3, /improve-animations audit finding #3): the content
// swap's `exit` leg, as a motion `variants` function keyed on
// `isRotation` (below) — the exiting `motion.div` (the OLD `swapKey`)
// stops receiving fresh props from this component's own render the
// instant its key drops out of the JSX, so there is no other way to
// hand it the freshly-computed `isRotation` boolean than `AnimatePresence`'s
// own `custom` prop, which motion re-evaluates variant FUNCTIONS with for
// exiting children specifically (the documented mechanism the plan's own
// doc names). `initial`/`animate` don't need this: the ENTERING child is
// still live in the render tree, so they read `isRotation` directly from
// closure, no variants indirection needed — only `exit` is a variant
// label here, kept as a module-level constant (not per-render) since it
// depends on nothing but its `custom` argument.
// plan 129 (T3, deep-review fix): exported (test-only export, same
// precedent as `iconForBundleId` in IdleHoverPeek.tsx) so
// StatusRailCard.test.tsx can pin the two durations/ease directly
// against this object rather than only indirectly, through rendered
// motion output — jsdom/motion don't expose a committed animation's
// `transition` back onto the DOM the way plain CSS values are
// inspectable, so the variant function itself is the only place these
// three values are actually checkable.
// plan 146b: `custom` widened from a plain `isRotation` boolean to
// `{ isRotation, isInterrupt }` — a Priority Preemption handover is
// ALWAYS also a showing(A)->showing(B) rotation (`isRotation` true, see
// `isInterrupt`'s own doc further down for why), but it must NOT play
// the gentle rotation fade: it needs its own faster, sharper "yanked"
// exit (INTERRUPT_EXIT_MS + INTERRUPT_EASE, plus a small downward/scale
// pull so it reads as cut short, not merely quicker). `isInterrupt` is
// checked first and wins outright — the plain `isRotation` branch below
// only ever runs for an ordinary end-of-turn rotation.
export const contentExitVariants = {
  // review fix (/review-animations, fresh-agent pass): `reduceMotion` was
  // added to `custom` because Motion's own `MotionConfig
  // reducedMotion="user"` mechanism (main.tsx) never reaches this branch —
  // it keys its reduced-motion snap off `positionalKeys` (x/y/scale/…,
  // motion-dom's own `transformPropOrder`), which a raw `transform` STRING
  // target (below) never matches, confirmed by reading motion-dom's source
  // directly. So reduced motion has to be handled explicitly here instead
  // of relying on the library to catch it — dropping the `transform` field
  // entirely under reduced motion, keeping only the opacity fade, matching
  // the shape the non-interrupt branches below already use.
  exit: (custom: { isRotation: boolean; isInterrupt: boolean; reduceMotion: boolean }) => {
    if (custom.isInterrupt) {
      return custom.reduceMotion
        ? { opacity: 0, transition: { duration: INTERRUPT_EXIT_MS / 1000, ease: INTERRUPT_EASE } }
        : {
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

// plan 146b: rank map backing `isInterrupt`'s "strictly higher priority"
// check below — mirrors rust's `Priority` ordering (`event.rs`/`queue.rs`
// derive `Ord` with declaration order Low < Medium < High). Kept local
// to this file (not exported from useSlotState.ts) since nothing else
// needs a numeric Priority rank today.
const PRIORITY_RANK: Record<"low" | "medium" | "high", number> = {
  low: 0,
  medium: 1,
  high: 2,
};

// plan 146b: how much of the OUTGOING item's Rotation window must still
// plausibly remain, estimated client-side, before a showing(A)->
// showing(B) swap is treated as a genuine Priority Preemption rather
// than an ordinary end-of-turn Promotion. The wire carries no explicit
// "preempted" flag (queue.rs's `try_preempt_visible` is rust-internal —
// spec's deliberate "no wire change"), so this is inferred from the same
// ttl/remaining fields TtlBar.tsx already anchors locally: an ordinary
// rotation only ever happens once the outgoing item's countdown has
// actually run out (estimated remaining ~0, modulo the engine's own
// small tick-to-emission latency), while a preemption cuts the item off
// with real time still on the clock. 400ms comfortably clears that
// tick/emission jitter (the rotation loop wakes ~10ms past its own
// deadline — see engine.rs) without requiring a preemption to land in
// the very same instant as the interrupting enqueue to be recognized.
const INTERRUPT_MIN_REMAINING_MS = 400;

// Plan 171 (tab-notch redesign, slice K): which selections are served by
// `IdleHoverPeek`'s own shipped rendering rather than by a dedicated
// below-block component. Spec section 7's weather bullet ("the shipped
// card, unchanged") and section 11 ("`IdleHoverPeek.tsx` is untouched")
// together mean these two must reach that component, not a copy of it —
// see `TabBelowBlock.tsx`'s header for the full split.
function peekPreferenceFor(selected: Tab | null): PeekPreference {
  return selected === "football" || selected === "weather" ? selected : null;
}

// The empty session list every caller that doesn't participate in the
// tab feature (tests, the settings preview) gets by omission. A
// module-level constant rather than a `[]` default in the destructuring
// so it keeps a stable identity across renders — a fresh array literal
// per render would defeat any future memoization downstream.
const NO_AGENT_SESSIONS: AgentSessionView[] = [];

// Spec section 10: the overlay stays receive-only, so the DOM click on an
// icon decides nothing — rust's own native click monitor sees the same
// physical click and emits `tab-selection-changed`. The strip still wants
// a real `<button>` (for `icon-strip.css`'s `:active { scale(0.9) }`
// press feedback and for the accessible name), so it gets a handler that
// deliberately does nothing. Module-level so its identity is stable
// across renders. See `IconStrip`'s own `onSelect` doc: that component
// was written to be correct under either eventual answer to the
// click-detection question, and this is the rust-owned answer.
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
  // plan 085: the resting-state render choice. Optional, defaulting to
  // "rail" — every existing caller (tests, the settings preview) that
  // never passes it keeps today's idle rail, byte-identical.
  restingState?: "rail" | "notch";
  // plan 087: the hover primitive's one diagnostic consumer — a real
  // `hover-changed` event drives this in the shipped app; every other
  // caller (tests, the settings preview) that never passes it keeps
  // today's un-hovered render, byte-identical. Consuming features
  // (081/082/084/idle expanded-on-hover) are each their own follow-on
  // work — this prop only proves the signal arrives.
  hovered?: boolean;
  // Plan 171 (tab-notch redesign, slice K): the currently selected tab,
  // sourced from the `tab-selection-changed` channel in App.tsx
  // (`useTabSelection`) and threaded down exactly like `status` already
  // is — this component never listens for itself, so the settings
  // preview and every test render it with no tauri channel at all. RUST
  // owns the selection (spec section 10); this is display state only.
  selectedTab?: Tab | null;
  // The Agent Session snapshot backing the agent tab's below-block —
  // `useAgentState`'s own `sessions`/`capturedAtMs` pair, threaded from
  // App.tsx for the same reason `status` is. Empty by default, so every
  // existing caller renders byte-identically.
  agentSessions?: AgentSessionView[];
  agentCapturedAtMs?: number;
  // Plan 184 (Part 1): the Agent tab's viewed-session cursor —
  // `useAgentViewedSession`'s return value, sourced in App.tsx and
  // threaded down for the same reason `selectedTab` above is (this
  // component never listens for itself). Optional and undefined by
  // default so every existing caller (tests, the settings preview) keeps
  // rendering byte-identically; `TabBelowBlock`'s own `viewedSessionIndex`
  // prop already defaults an absent value to session 0.
  viewedSessionIndex?: number;
}) {
  const showing = slot.state === "showing";
  const currentId = showing ? slot.id : null;
  const currentSignal = showing ? slot.signal : null;
  const currentBody = showing ? slot.body : null;
  const news = showing && slot.eventType === "news_item";
  // plan 084: detect the live-match football branch by the structured
  // `espn` block's presence (POST-083 contract), never by string-sniffing
  // eventType/signal — off the LIVE slot, matching how `news`/`wxArt`
  // above are computed, so the pulse-vs-celebration gate below always
  // reflects the arriving item.
  const isLiveCard = showing && slot.espn !== undefined;

  const [pulse, setPulse] = useState<Pulse>(null);

  // plan 150 (Step 2): a render-independent mirror of `pulse`, so the
  // re-trigger effect below can ask "is the class I'm about to apply the
  // one that's already on the element?" WITHOUT taking `pulse` as a
  // dependency (which would make the effect re-run on its own writes).
  // Every write to `pulse` goes through `setPulseNow` so the two can't
  // drift.
  const pulseRef = useRef<Pulse>(null);
  const setPulseNow = useCallback((next: Pulse) => {
    pulseRef.current = next;
    setPulse(next);
  }, []);

  // plan 150 (Step 1): how many `ripple-out` animationend events this
  // celebration has seen so far — reset whenever a goal pulse is (re)armed
  // below, counted up in `clearPulseWhenItsAnimationEnds`.
  const rippleEndsSeenRef = useRef(0);

  // plan 127 (Step 3): backs `isRotation` below (computed right where
  // `swapKey` is, further down) — declared up here with the component's
  // other hooks, per this file's usual convention. `key` starts at
  // `undefined` (never equal to a real `swapKey` on the very first
  // render, guaranteeing the guard below never mistakes mount for a
  // same-key re-render); `isRotation`/`wasShowing` both start `false`
  // since nothing has ever "shown" yet.
  // plan 146b: `isInterrupt` and `anchor` added alongside. `anchor` is
  // THIS key's own countdown snapshot (priority + the wall-clock instant
  // it was last (re)anchored + the `remainingMs` it was anchored at) —
  // read back only once, at the render where a DIFFERENT key eventually
  // replaces this one, to estimate how much of ITS turn was actually left
  // at that instant (see `isInterrupt`'s own doc below for why that's the
  // only signal available). `null` until the first showing render sets it.
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

  // Keyed on [currentId, currentSignal], never on priority — the actual
  // acceptance criterion this field exists for: a High-priority agent
  // "needs input" alert (signal: "generic") must never play the goal
  // celebration. Not keyed on `expanded` either, so toggling the manual
  // hotkey on an already-visible item doesn't replay the burst.
  // biome-ignore lint/correctness/useExhaustiveDependencies: currentId is the deliberate re-trigger key documented above — a new item with the same signal must replay the pulse; dropping it would change that behavior.
  useEffect(() => {
    const nextPulse: Pulse =
      currentSignal === "goal" ? "pulse-goal" : currentSignal === "red_card" ? "pulse-red" : null;

    // plan 150 (Step 1): a fresh celebration starts its ring count over —
    // otherwise a second goal arriving mid-flight would inherit the first
    // one's partial tally and clear early.
    rippleEndsSeenRef.current = 0;

    // plan 150 (Step 2): the same-signal replay fix. Two goals in a row
    // (a NEW `currentId`, same `signal`) compute the SAME class string,
    // and `setPulse("pulse-goal")` while `pulse` is already "pulse-goal"
    // is a React `Object.is` state bailout — no re-render, so the DOM
    // `class` attribute is never rewritten and the CSS animations neither
    // restart nor replay: the second goal celebrated nothing. Clearing to
    // `null` and re-applying on the NEXT frame remounts the class (and
    // the `.cele-ripple` layer with it), which is what actually restarts a
    // CSS animation. The one blank frame this costs is ~16ms, invisible at
    // 60fps, and is the standard restart technique. Only taken when the
    // class is genuinely unchanged — a first goal (from `null`) or a
    // goal-after-red-card applies synchronously, exactly as before.
    if (nextPulse !== null && pulseRef.current === nextPulse) {
      setPulseNow(null);
      const frame = requestAnimationFrame(() => setPulseNow(nextPulse));
      return () => cancelAnimationFrame(frame);
    }

    setPulseNow(nextPulse);
  }, [currentId, currentSignal, setPulseNow]);

  const [liveCelebration, setLiveCelebration] = useState<Celebration>(null);

  // Same [currentId, currentSignal] re-trigger discipline as the pulse
  // effect above — `currentBody` is read inside (not a dependency) because
  // it always arrives paired with currentId/currentSignal on the same slot
  // object; the goal/penalty/own-goal split needs it (see
  // `footballEventKindFor`'s doc), but it can't independently change
  // without a new id, so it isn't a re-trigger key in its own right.
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
      // plan 150 (Step 1): `ripple-out` ends once per ring (three rings,
      // staggered 0/280/560ms) and they all bubble to this one handler —
      // only the LAST one means the goal celebration is actually over.
      // `pulse-red` has no ripple layer at all (the `.cele-ripple` mount
      // below is gated on `pulse === "pulse-goal"`), so its own end
      // keyframe still clears on the first arrival.
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

  // plan 082: weather ALERT cards carry their art derived from the live
  // slot's `wx-*` marker pairs — same live-`slot` basis as `news`/
  // `categoryClass` above (not `renderedSlot`), so the below-block's mood
  // updates in lockstep with every other live-slot-derived class, not
  // delayed by the 220ms content swap. `null` for every non-weather card,
  // so it renders byte-identical to today.
  const wxArt = showing ? weatherArtFromDetails(slot.details) : null;
  // plan 096, renamed by plan 137 (cmux relay superseded by the v7 Agent
  // Adapter layer, spec §7/§12): the agent accent's below-block hairline
  // gate — same live-slot basis as `news`/`wxArt` above, for the same
  // lockstep-with-below-block reason. Deliberately NOT part of
  // `cardClass` (the shell): the shell owns the priority accent channel
  // only, and origin must never share that channel (see the CSS comment
  // on `.below-block.agent-origin`).
  const agentOrigin = showing && slot.origin === "agent";

  // plan 120: `swapKey` also feeds the below-block's AnimatePresence
  // `key` directly (JSX further down), not just `useExitChoreography`'s
  // internal `useDelayedSwap` call — so it stays computed here too,
  // deliberately duplicating the identical one-line derivation the hook
  // now also does internally (same pattern this file already uses for
  // `showing` itself: cheap, pure, recomputed rather than threaded
  // across the hook boundary as an extra return value just for one JSX
  // consumer).
  const swapKey = showing ? slot.id : "idle";

  // plan 127 (Step 3, finding #3): whether the swap THAT LANDED THIS
  // `swapKey` was a same-slot rotation — showing(A)->showing(B) — rather
  // than a promotion (idle->showing) or an exit (showing->idle).
  // `wasShowingRef` holds the previous DISTINCT key's own showing-ness,
  // updated only on the render where `swapKey` actually changes (guarded
  // by comparing against the last key this ref saw) — deliberately NOT
  // unconditional every render: this component re-renders for reasons
  // that have nothing to do with the swap (e.g. the pulse/celebration
  // effects just above call `setPulse`/`setLiveCelebration` right after
  // mount, forcing an immediate second render with the SAME `swapKey`),
  // and an unconditional write would let that unrelated extra render
  // overwrite the ref before this render's own `isRotation` value is
  // even read back on a later actual key change — corrupting the history
  // the very next real transition depends on. Guarding on the key
  // ensures `isRotation` stays STABLE for a given key's entire mounted
  // lifetime (every same-key re-render — the queue-slider tick, the
  // pulse effect, ...) reads the SAME cached value AnimatePresence's
  // `custom` was actually given at the transition, never a stale
  // recomputation.
  // Mutated directly in the render body (not an effect) — the standard,
  // React-sanctioned "remember a previous render's value" idiom (same
  // shape as a hand-rolled `usePrevious`), safe here because the write is
  // deterministic given this render's own `swapKey`/`showing` and has no
  // visible side effect other than being read by a later render.
  // The three-way split falls out for free from just two booleans:
  // idle->showing has the previous key's `wasShowing` false (idle was
  // never "showing"), so `isRotation` is false; showing->idle has the
  // live `showing` itself false, so `isRotation` is false regardless of
  // history; only showing(A)->showing(B), where both are true, yields
  // true. Promotion and exit legs are therefore byte-identical to before
  // this plan — only the true showing->showing case changes at all.
  // plan 146b: `isInterrupt` — computed in the SAME guarded block, at the
  // SAME instant `isRotation` is (the render where `swapKey` actually
  // changes), for the same "must stay stable for the whole mounted
  // lifetime of this key" reason documented above. A Priority Preemption
  // is, structurally, ALWAYS a showing(A)->showing(B) rotation too (the
  // Slot never empties in between) — the wire gives no other signal that
  // distinguishes "cut short by something more important" from "finished
  // its turn, next item promoted," including when that next item happens
  // to outrank the one it replaced by ordinary priority-drain order (the
  // queue always promotes its highest-priority Waiting item, so a plain
  // priority INCREASE across a rotation is common and unremarkable on its
  // own). The two extra facts that jointly and only hold for a genuine
  // preemption: (1) the arriving item's priority is STRICTLY higher than
  // the outgoing one's (queue.rs's own `try_preempt_visible` contract —
  // equal or lower never preempts), and (2) the outgoing item's own
  // countdown, estimated from its last anchor, still had real time left
  // (an ordinary end-of-turn rotation only ever fires once that countdown
  // has actually run out). `previous.anchor` is the OUTGOING key's own
  // last-anchored snapshot — see the ref's declaration doc above and the
  // re-anchor block just below for how it's kept fresh across supersede
  // top-ups/manual-expand extensions on the same key.
  // 2026-08-02 (animation audit, finding 1): the arrival-pop marker. The
  // shell's bouncy `--ease-notchtap-pop` width curve used to live on
  // `.card-assembly`'s BASE rule (card-chrome.css), which meant it was
  // whatever played when no more-specific rule matched — including the
  // hover-out collapse that drops `.expanded`, so every un-hover
  // overshot and rebounded. The pop now lives on `.card-assembly.promoting`,
  // and this state is what scopes it to a genuine promotion entrance.
  //
  // Armed in the render body (below), NOT in an effect: the class has to
  // land in the SAME commit that flips the shell's width formula to the
  // showing geometry, because CSS keeps a running transition's original
  // timing function even if the rule underneath it changes mid-flight —
  // an effect would apply the class one commit too late to affect
  // anything. A render-phase `setState` on the component currently
  // rendering is React's own sanctioned mechanism for exactly this
  // (adjust state when a prop-derived key changes), and it's already
  // guarded by the same key-change check `isRotation` uses, so it can't
  // loop.
  const [promoting, setPromoting] = useState(false);

  // The disarm half: EXPAND_MS after each swap the entrance has settled,
  // so the pop must stop being the curve any LATER width change (a hover
  // flip, a manual expand toggle) resolves against. Keyed on `swapKey`
  // alone so each new swap restarts the window and the cleanup cancels
  // the previous one; `setPromoting(false)` while it's already false is a
  // React identity bailout, so the common idle case costs nothing.
  // A showing->idle exit doesn't wait for this timer at all — `swapKey`
  // flips to "idle" on that very render, and the render-body arm below
  // sets `false` synchronously, which is what guarantees `.promoting` and
  // `.exiting` can never sit on the shell together (card-chrome.css's
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

  // review fix (/review-animations): `motion/react`'s own reduced-motion
  // context (`MotionConfig reducedMotion="user"`, main.tsx) doesn't reach
  // the raw `transform` STRING targets below (Motion's reduced-motion gate
  // keys off `positionalKeys` — x/y/scale/… — which "transform" the string
  // never matches) — see `contentExitVariants`'s own doc for the full
  // "why". Read explicitly here instead and threaded through by hand.
  const reduceMotion = useReducedMotion() ?? false;

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
  // Each of the first three arms is LITERALLY the empty-guard its own
  // component already applies — `AgentBelowBlock`'s `sessions.length
  // === 0`, `MediaBelowBlock`'s `media === null`, and the hard-wired
  // `NO_NEWS_STORIES` that `TabBelowBlock` hands `NewsBelowBlock` (no
  // wire source exists for news story CONTENT yet — a recorded direction
  // option, deliberately not faked). Kept in lockstep on purpose: when a
  // news story wire lands, the `news` arm and that constant flip
  // TOGETHER, and this is the second of the two places to edit. The
  // final arm is `true` for football/weather, which have no below-block
  // at all — they are served by `IdleHoverPeek`'s own `prefer` rendering
  // (see `peekPreferenceFor`), so "has content" is not this predicate's
  // question for them.
  const pulledTabHasContent =
    pulledTab === "agent"
      ? agentSessions.length > 0
      : pulledTab === "music"
        ? (status?.media.current ?? null) !== null
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
  // RENDER something" — a tab whose source is empty (news today,
  // music with nothing playing, agent with no live session) degrades to
  // the ambient peek rather than to a blank shell.
  const peekOpen = tabPullOpen && !pulledBelowBlockOpen;

  // plan 091: the outer shell (`.card-assembly`) now owns ONLY geometry-
  // and-effects classes — priority accent, hover diagnostic, the goal/
  // red-card pulse and the live-match celebrations. `news-shade`/`wx-card`
  // (and their mood/texture riders) move to `belowBlockClass` below: they
  // are content presentation, not shell, and the below-block is the block
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
  // plan 091: below-block's own class list — the news/weather mood
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
    wxArt && "wx-card",
    wxArt?.moodClass,
    wxArt?.textureClass,
    agentOrigin && "agent-origin",
  ]
    .filter(Boolean)
    .join(" ");

  // plan 12x (wave 2): the swapped card BODY (everything inside
  // `.card-content`, below) now reads the LIVE `slot` directly, like
  // `news`/`wxArt`/`isLiveCard` above — there is no more `renderedSlot`
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
  // plan 082 marker-leak guard: every `wx-*` pair is a mood/glyph input,
  // never real content — strip it from `details` before it reaches EITHER
  // place details render as visible text (the collapsed loop below and
  // the expanded Manifest). Every non-weather card's `details` has no
  // `wx-*` labels, so this filter is a no-op there — byte-identical.
  const liveVisibleDetails = showing ? visibleDetails(slot.details) : [];

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
  // announce: a non-live-match card (news/generic/agent/weather ALERT —
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
          Plan 171 (slice K, spec section 4 — "rest is exactly the shell,
          the UNMODIFIED <IdleFace />, and the eq bars whenever audio is
          genuinely playing"): the face is now wrapped with `<EqBars>` in
          a `.rest-cluster` row, which takes over the face's own
          `grid-column: 2 / grid-row: 1` placement so the two sit side by
          side in the cutout instead of stacking in one cell — exactly
          the design source's own `.rest-cluster` (prototypes/tab-notch-
          rest-and-morph.html section 1c). `<IdleFace>` ITSELF is
          untouched, as spec section 4's correction requires: its grid
          declarations simply go inert as a flex child, and its
          `display: none` -> HUD `display: flex` gate still governs
          whether it paints at all. The cluster carries the same
          HUD-only gate for the same reason, so the eq bars can never
          paint over real notch hardware either. */}
      {idleFaceEligible && (
        <div className="rest-cluster" aria-hidden="true">
          <IdleFace idle={trueIdle} />
          {/* deliberately NARROWER than the music icon's own presence
              gate (`iconPresenceFor`, which keeps a paused track present
              but dim): the eq bars are a "sound is happening right now"
              indicator, so a paused transport collapses them to zero
              width. EqBars.tsx's own header comment reserves exactly
              this call. */}
          <EqBars playing={status?.media.current?.playing === true} />
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
          depends on that). `prefer` routes the football/weather
          selections into this component's OWN shipped rendering rather
          than a second copy of it — spec section 7's weather bullet
          ("the shipped card, unchanged") and section 11's explicit
          "IdleHoverPeek's mechanism is untouched". With nothing
          selected, both props are inert and this is byte-identical to
          before the plan. */}
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
            what the rounding law depends on. Football/weather are
            unaffected (the predicate is true for them; their wrapper
            mounts exactly as before and `TabBelowBlock` returns null for
            them as it always has). */}
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
              {/* plan 082: the condition glyph — a background-layer image,
              same z-order tier as .news-shade::before (behind
              .compact/.manifest, which the CSS below lifts to z-index 1).
              Live-slot-derived, like belowBlockClass's mood/texture
              classes above, so it never waits on the content swap. */}
              {wxArt && <img className="wx-icon" src={wxArt.glyphUrl} alt="" />}
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
              <AnimatePresence mode="wait" custom={{ isRotation, isInterrupt, reduceMotion }}>
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
                    // review fix (/review-animations): `reduceMotion` gates
                    // the `transform` field explicitly — see the hook's own
                    // doc above for why `MotionConfig` alone doesn't catch
                    // a raw `transform` string target.
                    initial={
                      enterAsPromotion && !reduceMotion
                        ? { opacity: 0, transform: "translateY(-4px)" }
                        : { opacity: 0 }
                    }
                    animate={
                      enterAsPromotion && !reduceMotion
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
