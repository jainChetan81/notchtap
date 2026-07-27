import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import {
  abbreviateHome,
  agentRuntimeClass,
  agentRuntimeLabel,
  agentStatePresentationFor,
  elapsedLabel,
} from "../lib/presentation";
import type { AgentSessionView } from "../useAgentState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import { StatusDots } from "./StatusDots";

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

const NOW_TICK_MS = 1000;

/// Local wall-clock tick — re-renders the board once a second so every
/// row's elapsed-in-state label stays live, WITHOUT rust publishing a
/// per-second `agent-state` event (CLAUDE.md's `dedup_eq` rule: a
/// continuously-varying field must never drive a wire emission). Mirrors
/// `useClock`'s own "local interval, not a wire tick" shape.
function useNowTick(intervalMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
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
    <div className={`agent-row ${presentation.className} ${agentRuntimeClass(session.runtime)}`}>
      <span className={`agent-dot ${presentation.pulse ? "pulse" : ""}`} aria-hidden="true" />
      <span className="agent-runtime-tick" aria-hidden="true" />
      <span className="agent-row-runtime">{agentRuntimeLabel(session.runtime)}</span>
      {projectName && <span className="agent-row-project">{projectName}</span>}
      <span className="agent-row-state">{presentation.label}</span>
      <span className="agent-row-elapsed">
        {elapsedLabel(liveElapsedMs(session, capturedAtMs, nowMs))}
      </span>
    </div>
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
        <span className={`agent-dot ${presentation.pulse ? "pulse" : ""}`} aria-hidden="true" />
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
            transition={{
              type: "spring",
              stiffness: 480,
              damping: 37,
              opacity: { duration: 0.15 },
            }}
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
  const nowMs = useNowTick(NOW_TICK_MS);

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

  return (
    <div className="card-assembly expanded agent-board-shell" data-testid="agent-board">
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
        {!expanded && (
          <div className="agent-board-primary">
            <div className={`agent-board-primary-head ${agentRuntimeClass(primary.runtime)}`}>
              <span
                className={`agent-dot large ${primaryPresentation.pulse ? "pulse" : ""}`}
                aria-hidden="true"
              />
              <span className="agent-runtime-tick" aria-hidden="true" />
              <span className="agent-board-runtime">{agentRuntimeLabel(primary.runtime)}</span>
              <span className="agent-board-state-pill">{primaryPresentation.label}</span>
            </div>
            {primaryProjectName && <div className="agent-board-project">{primaryProjectName}</div>}
            {primary.summary && <div className="agent-board-summary">{primary.summary}</div>}
            <div className="agent-board-elapsed">{primaryElapsed}</div>
          </div>
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
              transition={{
                type: "spring",
                stiffness: 420,
                damping: 38,
                opacity: { duration: 0.15 },
              }}
              style={{ overflow: "hidden" }}
            >
              <div className="agent-board-expanded-scroll">
                {sessions.map((session, index) => (
                  <motion.div
                    key={session.id}
                    initial={{ opacity: 0, y: -6 }}
                    animate={{ opacity: 1, y: 0 }}
                    // plan 142: a light per-row stagger (organic, not
                    // mechanical — capped so a 30-session board doesn't
                    // take seconds to finish entering) mirrors the
                    // "rows animate in staggered/organically" guideline;
                    // exit skips the stagger (collapsing should feel
                    // immediate, not trickle out row by row).
                    transition={{ delay: Math.min(index, 8) * 0.02, duration: 0.16 }}
                  >
                    <ExpandedAgentRow session={session} capturedAtMs={capturedAtMs} nowMs={nowMs} />
                  </motion.div>
                ))}
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
                transition={{
                  type: "spring",
                  stiffness: 420,
                  damping: 38,
                  opacity: { duration: 0.15 },
                }}
                style={{ overflow: "hidden" }}
              >
                {rest.map((session) => (
                  <AgentRow
                    key={session.id}
                    session={session}
                    capturedAtMs={capturedAtMs}
                    nowMs={nowMs}
                  />
                ))}
              </motion.div>
            )
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
