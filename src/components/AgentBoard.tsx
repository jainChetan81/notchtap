import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { DISCLOSURE_SPRING, NOTCHTAP_EASE } from "../animationTiming";
import {
  abbreviateHome,
  agentRuntimeClass,
  agentRuntimeLabel,
  agentStatePresentationFor,
  agentStatePriorityFor,
  elapsedLabel,
} from "../lib/presentation";
import type { AgentSessionView } from "../useAgentState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import { AgentHeroCard, MAX_VISIBLE_DETAIL_PAIRS } from "./NotificationBody";
import { StatusDots } from "./StatusDots";
import type { Detail } from "./StatusRailCard";

// Plan 136 (v7 ticket 4 of 13, spec §6.2 resting): the Agent Board's
// resting layout — one rich card for the highest-ranked session
// (`sessions[0]`, already Rust-ordered per spec §2.2 — this component
// does no sorting of its own) plus every other session as an individual
// compact row, NEVER a "+N" collapse (spec's own explicit rule).
//
// Mounted inside the SAME `.card-assembly`/`.flank-left`/
// `.synthetic-cutout`/`.flank-right`/`.below-block` shell markup
// StatusRailCard.tsx uses (card-chrome.css) — a deliberate choice to
// inherit that shell's notch/HUD cutout shape, rounding law, and
// Appearance controls (`--card-scale`/`--card-radius`/`--card-opacity`)
// for free rather than hand-rolling a second card shape. This is its own
// top-level component (not a mode grafted into StatusRailCard's already
// elaborate showing<->idle exit choreography) — `App.tsx` swaps between
// the two based on `lib/presentation.ts::presentationMode`.

// Plan 149 (adaptive tick): the board's local wall-clock only needs to
// tick as fast as the fastest label it drives can actually CHANGE.
// `elapsedLabel` (lib/presentation.ts) is second-granular below 60s and
// MINUTE-granular above it — so past a minute, 59 of every 60 one-second
// re-renders produced byte-identical output (~3,600 pointless full-board
// re-renders an hour, all day, on a surface nobody is looking at). The
// fast rate is kept for the sub-minute window (where every second is a
// visible change) and the slow rate takes over otherwise; 15s is well
// under the 60s minute boundary, so a label can never look more than a
// quarter-minute stale.
const FAST_NOW_TICK_MS = 1000;
const SLOW_NOW_TICK_MS = 15_000;
// The granularity boundary in `elapsedLabel` itself — above this, the
// label only changes once a minute.
const SECOND_GRANULAR_BELOW_MS = 60_000;

// Operator feedback (plan 147 follow-up, 2026-07-27): session rows used
// to pop in/out of the DOM with no exit animation, so a removal (retention
// expiry, stale eviction, a session dropping out of the snapshot) made
// siblings jump instead of gliding into the vacated space. Apple's
// "Designing Fluid Interfaces": these rows carry no gesture momentum to
// preserve on exit (nothing here is a flung/dragged object), so the spring
// is CRITICALLY DAMPED (`bounce: 0` — no overshoot), with a settle time
// (`duration: 0.35`) fast enough to read as a direct response to the
// underlying state change rather than a lingering flourish. One shared
// const drives enter, exit, AND layout (sibling reflow) — this is the
// same failure class CLAUDE.md calls out for `dedup_eq`ish drift bugs:
// three hand-copied transition literals invite desynced clocks, one
// const can't.
// Exported so the test file can pin these exact values without
// duplicating them (a second hand-copied literal in the test would be
// exactly the "desynced clocks" drift risk this const exists to avoid).
export const ROW_TRANSITION = { type: "spring", bounce: 0, duration: 0.35 } as const;

// Plan 149: the hero's IDENTITY swap — played only when a DIFFERENT
// session becomes primary (keyed on `primary.id`), never when the same
// session merely changes state (that morphs in place: the dot's colour
// transition + state tick, agent-board.css). Same one-const discipline
// as ROW_TRANSITION above, and exported for the same reason (the test
// pins this object rather than hand-copying its numbers).
// A short tween, not a spring: this is a content SWAP (out, then in via
// `mode="wait"`), not a physical object being moved, so there's no
// momentum to preserve and an overshoot would read as a wobble on a
// block of text. 6px of travel — enough to give the swap a direction
// (old content leaves upward, new content arrives from below) without
// reading as a slide.
export const HERO_SWAP_TRANSITION = { duration: 0.16, ease: NOTCHTAP_EASE } as const;

/// Local wall-clock tick — re-renders the board so every row's
/// elapsed-in-state label stays live, WITHOUT rust publishing a
/// per-second `agent-state` event (CLAUDE.md's `dedup_eq` rule: a
/// continuously-varying field must never drive a wire emission). Mirrors
/// `useClock`'s own "local interval, not a wire tick" shape.
///
/// Plan 149: the rate is now ADAPTIVE rather than a flat 1s — see
/// `nowTickIntervalMs` below for the why. The interval is re-derived on
/// every render (so a session crossing the 60s boundary, or a new
/// session arriving, re-evaluates it), but the effect only re-subscribes
/// when the chosen interval actually changes.
function useNowTick(sessions: AgentSessionView[], capturedAtMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  const intervalMs = nowTickIntervalMs(sessions, capturedAtMs, now);
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

/// Plan 149: fast tick only while at least ONE session's live elapsed is
/// still inside `elapsedLabel`'s second-granular window; slow otherwise.
/// Exported so the test can pin the selection directly rather than
/// inferring it from fake-timer render counts alone.
export function nowTickIntervalMs(
  sessions: AgentSessionView[],
  capturedAtMs: number,
  nowMs: number,
): number {
  const anySecondGranular = sessions.some(
    (session) => liveElapsedMs(session, capturedAtMs, nowMs) < SECOND_GRANULAR_BELOW_MS,
  );
  return anySecondGranular ? FAST_NOW_TICK_MS : SLOW_NOW_TICK_MS;
}

/// `session.elapsedMs` is a snapshot as of `capturedAtMs` (the wire
/// anchor) — the live value is that snapshot plus however much wall
/// time has passed since, same `elapsedMs + (Date.now() - capturedAtMs)`
/// shape `NowPlayingSummary` already established (StatusDots/IdleHoverPeek
/// media rendering).
function liveElapsedMs(session: AgentSessionView, capturedAtMs: number, nowMs: number): number {
  return session.elapsedMs + Math.max(0, nowMs - capturedAtMs);
}

function AgentRow({
  session,
  capturedAtMs,
  nowMs,
}: {
  session: AgentSessionView;
  capturedAtMs: number;
  nowMs: number;
}) {
  const presentation = agentStatePresentationFor(session.state);
  const projectName = session.project?.name ?? null;
  return (
    // `layout="position"` (not the full `layout` prop) — this row's own
    // height is already explicitly driven by `initial`/`animate`/`exit`
    // below, so layout only needs to smooth the sibling REFLOW (position),
    // not fight that explicit height animation for the same property.
    <motion.div
      layout="position"
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: "auto", opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={ROW_TRANSITION}
      style={{ overflow: "hidden" }}
      className={`agent-row ${presentation.className} ${agentRuntimeClass(session.runtime)}`}
    >
      {/* `key={session.state}`: see `agent-board.css`'s bounded-pulse
          rule — the breathe/tick animations are BOUNDED, so they only
          replay if the span genuinely remounts. Keying on the state
          makes every state change (and only a state change) restart
          them, which is precisely what "this just changed" should mean. */}
      <span
        key={session.state}
        className={`agent-dot ${presentation.pulse ? "pulse" : ""}`}
        aria-hidden="true"
      />
      <span className="agent-runtime-tick" aria-hidden="true" />
      <span className="agent-row-runtime">{agentRuntimeLabel(session.runtime)}</span>
      {projectName && <span className="agent-row-project">{projectName}</span>}
      <span className="agent-row-state">{presentation.label}</span>
      <span className="agent-row-elapsed">
        {elapsedLabel(liveElapsedMs(session, capturedAtMs, nowMs))}
      </span>
    </motion.div>
  );
}

// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): one row of the
// hover-expanded, scrollable session list — every retained session
// renders through this, in the same Rust-provided order the resting
// board already trusts without re-sorting. Click-free: the per-row
// bounded transition history discloses on the row's OWN mouse-enter/
// leave (real pointer events genuinely land in the webview while
// expanded — rust temporarily disables click-through for exactly this
// rect, `lib.rs`'s `try_expand_board_for_hover`), not a rust-sourced
// `hover-changed` boolean the way the rest of this receive-only overlay
// gates hover — there is exactly one card-level `hovered` signal on the
// wire, and it can't distinguish which row the cursor is over.
function ExpandedAgentRow({
  session,
  capturedAtMs,
  nowMs,
}: {
  session: AgentSessionView;
  capturedAtMs: number;
  nowMs: number;
}) {
  const [historyOpen, setHistoryOpen] = useState(false);
  const presentation = agentStatePresentationFor(session.state);
  const projectName = session.project?.name ?? null;
  const hasHistory = session.history.length > 0;
  // `cwd` only earns its own line when it says something `projectName`
  // doesn't already — an adapter that sets both to the same directory
  // name shouldn't get a redundant second line.
  const cwd = session.project?.cwd ?? null;
  const showCwd = cwd !== null && cwd !== projectName;
  const hostName = session.host?.name ?? null;
  // Only non-null for terminal sessions (`AgentSession::to_state`,
  // model.rs) — a live/stale session has nothing "clearing," so this
  // doubles as the terminal-only guard, no separate state check needed.
  const clearsIn =
    session.retentionRemainingMs !== null ? elapsedLabel(session.retentionRemainingMs) : null;
  // Plan 147 wave 2: the session's active subagent, rendered as one more
  // meta chip alongside cwd/host/clears-in — same "nothing renders when
  // absent" discipline. Falls back to `id` when the runtime hasn't (yet)
  // supplied a human `label`, and appends the subagent's own state in
  // parens only when the runtime reports one.
  const subagentChip =
    session.subagent !== null
      ? `subagent: ${session.subagent.label ?? session.subagent.id}${
          session.subagent.state ? ` (${session.subagent.state})` : ""
        }`
      : null;
  const hasMeta = showCwd || hostName !== null || clearsIn !== null || subagentChip !== null;
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: a purely supplementary hover disclosure (recent transition history), not a control — nothing here is keyboard-reachable in this receive-only, mouse-only overlay (no focusable elements or click handlers exist anywhere in this app; see CLAUDE.md's ipc/security section).
    <div
      className={`agent-expanded-row ${presentation.className} ${agentRuntimeClass(session.runtime)}`}
      data-testid="agent-expanded-row"
      onMouseEnter={() => setHistoryOpen(true)}
      onMouseLeave={() => setHistoryOpen(false)}
    >
      <div className="agent-expanded-row-head">
        {/* state-keyed for the bounded pulse/tick restart — same reason
            as `AgentRow`'s own dot above. */}
        <span
          key={session.state}
          className={`agent-dot ${presentation.pulse ? "pulse" : ""}`}
          aria-hidden="true"
        />
        <span className="agent-runtime-tick" aria-hidden="true" />
        <span className="agent-row-runtime">{agentRuntimeLabel(session.runtime)}</span>
        {projectName && <span className="agent-row-project">{projectName}</span>}
        <span className="agent-row-state">{presentation.label}</span>
        <span className="agent-row-elapsed">
          {elapsedLabel(liveElapsedMs(session, capturedAtMs, nowMs))}
        </span>
      </div>
      {session.summary && <div className="agent-expanded-row-summary">{session.summary}</div>}
      {/* Plan 146 follow-up (operator feedback, 2026-07-27): a restrained
          extra line of small muted mono chips for wire fields the row
          never surfaced before (`project.cwd`, `host.name`, terminal
          retention) — same "nothing renders if absent" discipline the
          detail cells below already follow, no placeholder chips. */}
      {hasMeta && (
        <div className="agent-expanded-row-meta">
          {showCwd && <span className="agent-expanded-meta-item">{abbreviateHome(cwd)}</span>}
          {hostName !== null && <span className="agent-expanded-meta-item">{hostName}</span>}
          {clearsIn !== null && (
            <span className="agent-expanded-meta-item">clears in {clearsIn}</span>
          )}
          {subagentChip !== null && (
            <span className="agent-expanded-meta-item">{subagentChip}</span>
          )}
        </div>
      )}
      {/* Capability-dependent detail cells (spec §6.2's own words) — an
          adapter that never declared/observed a detail simply has an
          empty `details` array; there is no placeholder cell to omit,
          nothing renders at all. */}
      {session.details.length > 0 && (
        <div className="agent-expanded-row-details">
          {session.details.map((detail) => (
            <span key={detail.label} className="agent-expanded-detail">
              <span className="agent-expanded-detail-label">{detail.label}</span>
              <span className="agent-expanded-detail-value">{detail.value}</span>
            </span>
          ))}
        </div>
      )}
      <AnimatePresence initial={false}>
        {historyOpen && hasHistory && (
          <motion.div
            className="agent-expanded-history"
            data-testid="agent-expanded-history"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={DISCLOSURE_SPRING}
            style={{ overflow: "hidden" }}
          >
            <ul className="agent-expanded-history-list">
              {/* oldest first, exactly as rust sent it (spec's own "no
                  sorting" rule extends to per-row history) */}
              {session.history.map((transition, index) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: transitions carry no stable identity of their own (state can repeat across entries) — index is stable for a given snapshot, all a receive-only list needs.
                <li key={index} className="agent-expanded-history-entry">
                  <span className="agent-expanded-history-state">
                    {agentStatePresentationFor(transition.state).label}
                  </span>
                  <span className="agent-expanded-history-elapsed">
                    {elapsedLabel(transition.elapsedMs)} ago
                  </span>
                </li>
              ))}
            </ul>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function AgentBoard({
  sessions,
  capturedAtMs,
  status,
  // Plan 142: sourced from `App.tsx`'s existing `hover-changed`-driven
  // `hovered` state — the SAME rust-emitted boolean StatusRailCard's own
  // hover consumers already use (`hover.rs`'s tracking-area primitive),
  // now also threaded here. Only meaningful while this component is
  // mounted at all (i.e. `presentationMode === "board"`), so a `true`
  // here always means "the cursor is over the Board," never some other
  // card.
  expanded = false,
}: {
  sessions: AgentSessionView[];
  capturedAtMs: number;
  // Optional, same convention as StatusRailCard's own `status` prop —
  // every caller that never passes one (tests) still renders the flank
  // dots in their "nothing to report" dim state.
  status?: StatusState;
  expanded?: boolean;
}) {
  const nowMs = useNowTick(sessions, capturedAtMs);

  // Defense in depth: `App.tsx` only mounts this component when
  // `presentationMode` already found at least one session, but a
  // same-render race (the board's own last session clearing between
  // that decision and this render) should degrade to nothing rather
  // than crash on `sessions[0]`.
  if (sessions.length === 0) {
    return null;
  }

  const [primary, ...rest] = sessions;
  const primaryPresentation = agentStatePresentationFor(primary.state);
  const primaryProjectName = primary.project?.name ?? null;
  const primaryElapsed = elapsedLabel(liveElapsedMs(primary, capturedAtMs, nowMs));
  // Plan 169 (step 6): the priority-tier mapping that feeds the hero's
  // Stamp/accent-stripe/fact-pill tone — see `agentStatePriorityFor`'s
  // own doc (lib/presentation.ts) for why this table exists and how the
  // seven states sort into low/medium/high.
  const primaryPriority = agentStatePriorityFor(primary.state);
  // Plan 169: fact pills for the templated hero — the same declared
  // `session.details` ExpandedAgentRow already renders (capability-
  // dependent, adapter-provided: tool/risk, progress, exit code, ...),
  // plus one synthesized elapsed-in-state fact for the three states
  // where "how long" is itself the single most useful extra fact
  // (starting/completed/stale) — the other four states either need no
  // extra time fact (waiting_for_input) or already surface something
  // more actionable via their own details (waiting_for_permission's
  // tool/risk, working's progress, failed's exit code). Capped at the
  // SAME MAX_VISIBLE_DETAIL_PAIRS the generic branch's own pills already
  // respect (NotificationBody.tsx) — one shared limit, not a second
  // hand-copied one.
  const primaryFactsRaw: Detail[] = [...primary.details];
  if (primary.state === "starting") {
    primaryFactsRaw.push({ label: "Session", value: primaryElapsed });
  } else if (primary.state === "completed") {
    primaryFactsRaw.push({ label: "Duration", value: primaryElapsed });
  } else if (primary.state === "stale") {
    primaryFactsRaw.push({ label: "Last seen", value: `${primaryElapsed} ago` });
  }
  const primaryFacts = primaryFactsRaw.slice(0, MAX_VISIBLE_DETAIL_PAIRS);
  // Plan 169 (Target table): waiting_for_permission and failed are the
  // two states the table marks "(danger tone)" on their fact pills —
  // mirrors `AGENT_STATE_PRESENTATION`'s own "agent-waiting"/
  // "agent-failed" alarm grouping (lib/presentation.ts), not a guess at
  // per-detail content: there is no wire field that says "this fact is
  // dangerous," but the session's STATE already carries that signal.
  const primaryFactsDanger =
    primary.state === "waiting_for_permission" || primary.state === "failed";

  return (
    <div
      className={`card-assembly expanded agent-board-shell ${primaryPriority}`}
      data-testid="agent-board"
    >
      <span className="notch-gill notch-gill-left" aria-hidden="true" />
      <span className="notch-gill notch-gill-right" aria-hidden="true" />
      <div className="flank-left">
        <FlankClock />
      </div>
      <div className="synthetic-cutout" aria-hidden="true" />
      <div className="flank-right">
        <div className="card-content idle">
          <StatusDots status={status} />
        </div>
      </div>
      <div
        className={`below-block agent-board ${primaryPresentation.className} ${agentRuntimeClass(primary.runtime)}`}
      >
        {/* Plan 142 fix (operator feedback, 2026-07-27): the hero block
            below and the expanded list's own first row both used to
            render `sessions[0]` — at N=1 that put the identical session
            on screen twice. The hero is the RESTING-only summary for the
            top session; while `expanded`, it's replaced entirely by the
            expanded list (which already includes the primary session as
            its first `ExpandedAgentRow`, so `primary.details`/`history`
            stay reachable there instead). */}
        {/* Plan 149: the hero used to swap in a single frame when a
            DIFFERENT session took the top rank — five lines of content
            teleporting, with nothing distinguishing "a new session is
            now primary" from "the same session changed state". Keyed on
            `primary.id` ONLY, so an identity change plays the swap while
            a state change within the same session morphs in place (the
            dot's colour transition + state tick, agent-board.css) —
            keying on state too would make an ordinary state change
            re-animate the whole block, which is exactly the noise this
            is meant to remove. `mode="wait"` because the hero is a
            single block: an overlap would double its height mid-flight,
            and a 160ms out-then-in reads cleanly as one swap. */}
        {/* Plan 169: the hero's INNER content now renders through
            `AgentHeroCard` (NotificationBody.tsx) — the same masthead-row/
            Stamp/title/subtitle/body/fact-pill template every other
            origin's compact card already uses, replacing the bespoke
            `.agent-board-primary-head`/`.agent-board-runtime`/
            `.agent-board-state-pill`/`.agent-board-project`/
            `.agent-board-summary`/`.agent-board-elapsed` block this file
            used to hand-roll. The OUTER `motion.div` (identity-swap
            animation, `agent-board-primary` class) is untouched — this
            plan changes what renders INSIDE `.below-block`, never the
            shell or the swap's own animation contract (Boundaries). */}
        {!expanded && (
          <AnimatePresence initial={false} mode="wait">
            <motion.div
              key={primary.id}
              className="agent-board-primary"
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={HERO_SWAP_TRANSITION}
            >
              <AgentHeroCard
                dotKey={primary.state}
                pulse={primaryPresentation.pulse}
                title={`${agentRuntimeLabel(primary.runtime)} — ${primaryPresentation.label}`}
                subtitle={primaryProjectName}
                body={primary.summary}
                priority={primaryPriority}
                facts={primaryFacts}
                factsDanger={primaryFactsDanger}
              />
            </motion.div>
          </AnimatePresence>
        )}
        {/* Plan 142 (spec §6.2 expanded): while `expanded`, the hero +
            compact `rest`-only rows swap for a bounded, scrollable list
            of EVERY retained session (primary included) — rust has
            already grown the real window to
            `agents::expand::expanded_board_frame`'s screen-bounded frame
            and opened pointer delivery for exactly this rect by the time
            this prop flips true (`try_expand_board_for_hover`), so the
            scroll container below genuinely has room to scroll and
            genuinely receives wheel events. `AnimatePresence`/`motion`
            morphs between the two shapes rather than a hard swap, same
            guideline every other shipped expand/collapse in this app
            already follows (the failure class to avoid is desynced
            clocks, not a specific library — `IdleHoverPeek.tsx`'s own
            spring is the precedent this mirrors). */}
        <AnimatePresence initial={false} mode="wait">
          {expanded ? (
            <motion.div
              key="expanded"
              className="agent-board-expanded-list"
              data-testid="agent-board-expanded-list"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={DISCLOSURE_SPRING}
              style={{ overflow: "hidden" }}
            >
              <div className="agent-board-expanded-scroll">
                {/* `initial={false}` mirrors the two outer `AnimatePresence`
                    blocks in this file (the resting/expanded swap above,
                    the per-row history disclosure in `ExpandedAgentRow`) —
                    the whole board mounting (or `expanded` flipping true
                    for the first time) shouldn't stagger-animate every
                    already-present row in; only a genuine per-session
                    add/remove/reorder after that should animate. Default
                    ("sync") mode, not "wait" — an exit and its siblings'
                    reflow must play concurrently, not sequentially, or a
                    removal reads as two separate beats instead of one
                    fluid motion. */}
                <AnimatePresence initial={false}>
                  {sessions.map((session) => (
                    // Row exit/enter/reflow share ONE transition
                    // (`ROW_TRANSITION`, see its own comment above) — same
                    // spring for a departing row's collapse, an arriving
                    // row's expand, and `layout="position"`'s reflow of
                    // everything in between, per the "mirror the exit path
                    // exactly" spatial-consistency rule. Overflow hidden on
                    // THIS row (not the `.agent-board-expanded-scroll`
                    // container, which keeps its own `overflow-y: auto`)
                    // so the collapsing row doesn't spill during animation.
                    <motion.div
                      key={session.id}
                      layout="position"
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={ROW_TRANSITION}
                      style={{ overflow: "hidden" }}
                    >
                      <ExpandedAgentRow
                        session={session}
                        capturedAtMs={capturedAtMs}
                        nowMs={nowMs}
                      />
                    </motion.div>
                  ))}
                </AnimatePresence>
              </div>
            </motion.div>
          ) : (
            rest.length > 0 && (
              <motion.div
                key="resting"
                className="agent-board-rows"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={DISCLOSURE_SPRING}
                style={{ overflow: "hidden" }}
              >
                {/* `initial={false}` (matching the outer swap this block
                    lives inside, and `ExpandedAgentRow`'s inner list above)
                    — this block itself already fades/grows in as a whole
                    on first mount, so the individual rows inside it
                    shouldn't ALSO stagger-animate in on top of that; only
                    a genuine per-session add/remove/reorder afterward
                    should trigger `AgentRow`'s own enter/exit. Default
                    ("sync") mode so an exit and the resulting sibling
                    reflow (via `AgentRow`'s `layout="position"`) play
                    concurrently, same reasoning as the expanded list. */}
                <AnimatePresence initial={false}>
                  {rest.map((session) => (
                    <AgentRow
                      key={session.id}
                      session={session}
                      capturedAtMs={capturedAtMs}
                      nowMs={nowMs}
                    />
                  ))}
                </AnimatePresence>
              </motion.div>
            )
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
