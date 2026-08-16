import { AnimatePresence, motion } from "motion/react";
import { DISCLOSURE_SPRING } from "../animationTiming";
import { useClock } from "../useClock";
import type { LiveMatchSummary, StatusState } from "../useStatusState";

// plan 093 (079 items 9/17/18, folded into one surface): the idle
// hover-expanded state — hovering the idle assembly (`hovered`, the hover
// primitive's prop, NEVER CSS `:hover` — the overlay window is
// click-through, only the rust tracking area knows the cursor) opens a
// `.below-block` beneath the flank row. Content precedence (item 3's
// design constraint — one below-block at a time): a live match
// (`status.football.live`) fills the content slot; with none available,
// the block still opens with the day-progress timeline alone (item 18's
// decision: "the timeline appears only in the idle expanded-on-hover
// view", read as unconditional on ambient data — the timeline lost every
// other home when 091 removed the old always-on idle view, so gating its
// one remaining home on football being configured would make it
// permanently unreachable for anyone without it).
//
// Mount lifecycle: the CONTENT below opens/closes on the `open` prop via
// `AnimatePresence` + `motion.div` (plan 12x — migrated off a hand-rolled
// mounted/closing `useState` + `setTimeout` state machine and matching CSS
// `@keyframes`, this codebase's animation-law now routes all animation
// through the `motion` library rather than hand-drawn keyframes or manual
// layout-prop tweening). `AnimatePresence` owns the exit window itself
// (`exit={...}` plays out before the node actually leaves the DOM) — no
// purpose-built timer needed here anymore. `IdleHoverPeek` ITSELF is now
// always mounted by StatusRailCard (plan 127, Step 2, finding #2) — only
// `open` (defaulting to `hovered` for standalone callers, see the prop's
// own doc below) gates this AnimatePresence child, so a promotion
// arriving mid-peek (which flips `open` false without ever unmounting
// this component) lets the exit actually play instead of the whole
// subtree — AnimatePresence included — being torn out synchronously by a
// parent-level conditional.
//
// This is also why the block is NOT always-mounted whenever ambient data
// exists: `.below-block`'s mere DOM presence is what 091's
// `:not(:has(.below-block))` rounding law keys off (untouched by this
// plan — the flanks-un-round-while-open behavior below falls out of that
// existing rule for free, exactly because this mounts only while open,
// including during `AnimatePresence`'s exit animation). Always-mounting
// whenever football happened to be configured would un-round the idle
// pill any time that ambient data exists, not just while actually
// hovered — a real, unwanted change to 091's shell behavior.

function ScorecardRevealContent({ live }: { live: LiveMatchSummary }) {
  return (
    <div className="idle-reveal-scorecard">
      <div className="sc-head">
        <span className="chip chip-live">
          <span className="live-dot" aria-hidden="true" />
          Live
        </span>
        <span className="chip clock-pill">{live.minute}</span>
      </div>
      <div className="idle-reveal-label">{live.label}</div>
    </div>
  );
}

// plan 093 (item 18): the day-progress timeline, relocated from the
// deleted `IdleView`/`.idle-view .timeline` (091 removed the old always-on
// idle view; the original CSS is gone too, so this rebuilds the same
// thin-line-plus-dot shape from that history rather than reusing dead
// selectors) into its new, sole home: this peek. `useClock`'s
// `dayProgress` is unchanged — same 0–100 "how far through the local day"
// value the old view read.
function PeekTimeline() {
  const { dayProgress } = useClock();
  return (
    <span
      className="idle-peek-timeline"
      style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
      aria-hidden="true"
    />
  );
}

// Plan 171 (tab-notch redesign, slice K): which source the caller wants
// this peek to show, overriding the ambient precedence chain below.
// `null` (the default, and what every pre-171 caller passes by omission)
// means "unchanged" — the shipped football > timeline chain,
// byte-identical to before this plan.
//
// Only the source the tab feature deliberately does NOT rebuild is
// listed: section 11 keeps this peek's own mechanism explicitly
// untouched, so selecting the football tab must reach THIS component's
// existing rendering rather than a second, drifting copy of it.
// Agent/news each got their own dedicated below-block component
// (`AgentBelowBlock`/`NewsBelowBlock`) and so are never routed here.
export type PeekPreference = "football" | null;

export function IdleHoverPeek({
  status,
  hovered,
  // plan 127 (Step 2, /improve-animations audit finding #2): the mount
  // gate used to be StatusRailCard's own `{!renderedShowing && <IdleHoverPeek
  // hovered={hovered} />}` conditional — which unmounted this WHOLE
  // component (AnimatePresence included) the instant a promotion arrived
  // mid-peek, tearing out up to 100px of content with zero animation
  // (React drops a component tree synchronously; an AnimatePresence
  // inside the removed subtree never gets the chance to run its own
  // `exit`). `open` is the new, dedicated mount-gate signal — sourced by
  // the caller from `!renderedShowing && hovered` exactly as before, but
  // now read INSIDE the always-mounted component's own AnimatePresence
  // condition, so a promotion flipping `open` false lets THIS
  // AnimatePresence play the existing `exit={...}` collapse for real
  // while the caller's card content enters above it, instead of
  // vanishing instantly. Defaults to `hovered` (this component's
  // original, pre-127 gate) so every standalone caller — this file's own
  // tests, the settings preview, any future direct usage — keeps
  // identical behavior without having to pass a redundant `open` prop
  // when there's no separate "peek should survive a promotion" caller to
  // coordinate with.
  open = hovered,
  // Plan 171 (slice K): see `PeekPreference`'s own doc above. Defaults to
  // `null` so every existing caller — this file's own tests, the settings
  // preview, StatusRailCard's no-selection path — keeps the shipped
  // precedence chain without passing anything.
  prefer = null,
}: {
  status?: StatusState;
  hovered: boolean;
  open?: boolean;
  prefer?: PeekPreference;
}) {
  const liveMatch = status?.football.live ?? null;

  // Plan 171 (slice K): a preference NARROWS to one source, it never
  // re-orders and falls through. A preferred source with no data yields
  // an empty content slot (the timeline alone), which is the same
  // "nothing to show" posture the un-preferred chain's own final `null`
  // arm already produces. With only football left as a peek-served
  // source, `prefer` currently narrows to the same source the ambient
  // chain would pick anyway — kept as a prop so the routing seam
  // (`peekPreferenceFor`, StatusRailCard.tsx) stays explicit.
  const live = prefer === null || prefer === "football" ? liveMatch : null;

  return (
    <AnimatePresence>
      {open ? (
        // plan 093: `height: 100` mirrors `hover.rs`'s `IDLE_PEEK_BELOW_BLOCK_H`
        // constant exactly — a real duplicated-constants pair (see that
        // constant's own doc comment). Any change to this height MUST
        // change that constant in the same commit.
        <motion.div
          className="below-block idle-peek"
          /* 2026-07-23 review fix (the "peek close pops ~25px" finding):
             `.idle-peek`'s CSS `padding: 12px 16px 13px` + border-box means
             a bare `height: 0` FLOORS at padding-top+bottom (25px) — the
             box could never reach zero, so the close visibly popped at
             unmount. Animating the vertical paddings in lockstep with the
             height (12/13 at rest, matching the stylesheet exactly, so
             there is zero visual change while open) lets the collapse
             genuinely reach 0. Horizontal padding stays CSS-owned (16px). */
          initial={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          animate={{ height: 100, opacity: 1, paddingTop: 12, paddingBottom: 13 }}
          exit={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          // plan 12x (wave 3): stiffer spring (420 -> 480) and damping
          // nudged up in step (34 -> 37), operator-feedback "snappier
          // overall" pass — keeping the same near-critically-damped feel
          // rather than trading the speed gain for extra bounce.
          // `height: 100` is untouched (rust's `IDLE_PEEK_BELOW_BLOCK_H`
          // pairing, out of scope for that pass).
          //
          // plan 148: those numbers now live in animationTiming.ts as
          // DISCLOSURE_SPRING, shared with AgentBoard's disclosures (they
          // were byte-identical hand-copies). The wave-3 pass's separate
          // `opacity: { duration: 0.15 }` override is gone with them —
          // see that constant's own doc for why a fixed opacity tween
          // desynced from the spring on interrupted hover flips.
          transition={DISCLOSURE_SPRING}
          style={{ overflow: "hidden" }}
        >
          <div className="peek-content">
            {/* precedence is football > timeline — one below-block at a
                time. */}
            {live !== null ? <ScorecardRevealContent live={live} /> : null}
            <PeekTimeline />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
