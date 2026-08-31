import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import type { AgentSessionState, AgentSessionView } from "../hooks/useAgentState";
import type { StatusState } from "../hooks/useStatusState";
import { DISCLOSURE_SPRING, NOTCHTAP_EASE } from "../lib/constants";
import {
  abbreviateHome,
  agentRuntimeClass,
  agentRuntimeLabel,
  agentStatePresentationFor,
  agentStatePriorityFor,
  elapsedLabel,
  type Priority,
} from "../lib/presentation";
import { FlankClock } from "./FlankClock";
import {
  AgentHeroCard,
  type Fact,
  type FactTone,
  MAX_VISIBLE_DETAIL_PAIRS,
} from "./NotificationBody";
import { StatusDots } from "./StatusDots";
import type { Detail } from "./StatusRailCard";

const FAST_NOW_TICK_MS = 1000;
const SLOW_NOW_TICK_MS = 15_000;
const SECOND_GRANULAR_BELOW_MS = 60_000;

export const ROW_TRANSITION = { type: "spring", bounce: 0, duration: 0.35 } as const;

export const HERO_SWAP_TRANSITION = { duration: 0.16, ease: NOTCHTAP_EASE } as const;

const SWAP_CELL_STYLE = { gridArea: "1 / 1" } as const;

function useNowTick(sessions: AgentSessionView[], capturedAtMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  const intervalMs = nowTickIntervalMs(sessions, capturedAtMs, now);
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

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

function liveElapsedMs(session: AgentSessionView, capturedAtMs: number, nowMs: number): number {
  return session.elapsedMs + Math.max(0, nowMs - capturedAtMs);
}

const AGENT_HERO_TITLE = {
  waiting_for_permission: "Agent needs input",
  waiting_for_input: "Agent needs input",
  working: "Agent working",
  starting: "Agent starting",
  completed: "Agent turn completed",
  failed: "Agent session failed",
  stale: "Agent session stale",
} satisfies Record<AgentSessionState, string>;

const TAGGED_RISKS = new Set(["destructive", "blocked"]);

function normalizedLabel(label: string): string {
  return label.trim().toLowerCase();
}

function isNonzeroExitValue(value: string): boolean {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) && parsed !== 0;
}

function heroFactTags(state: AgentSessionState, details: Detail[]): Fact[] {
  const facts: Fact[] = details.map((detail) => ({ ...detail }));
  if (state === "waiting_for_permission") {
    const riskIndex = facts.findIndex(
      (fact) =>
        normalizedLabel(fact.label) === "risk" && TAGGED_RISKS.has(normalizedLabel(fact.value)),
    );
    const toolIndex = facts.findIndex((fact) => normalizedLabel(fact.label) === "tool");
    if (riskIndex !== -1 && toolIndex !== -1) {
      facts[toolIndex] = {
        ...facts[toolIndex],
        tag: { text: facts[riskIndex].value, tone: "danger" },
      };
      facts.splice(riskIndex, 1);
    }
    return facts;
  }
  if (state === "failed") {
    const exitIndex = facts.findIndex((fact) => {
      const label = normalizedLabel(fact.label);
      return (label === "exit" || label === "exit code") && isNonzeroExitValue(fact.value);
    });
    if (exitIndex !== -1) {
      facts[exitIndex] = { ...facts[exitIndex], tag: { text: "error", tone: "danger" } };
    }
  }
  return facts;
}

export type AgentHeroProps = {
  dotKey: string;
  pulse: boolean;
  title: string;
  subtitle: string;
  body: string | null;
  priority: Priority;
  facts: Fact[];
  factsTone: FactTone;
};

export function agentHeroPropsFor(
  session: AgentSessionView,
  capturedAtMs: number,
  nowMs: number,
): AgentHeroProps {
  const presentation = agentStatePresentationFor(session.state);
  const projectName = session.project?.name ?? null;
  const elapsed = elapsedLabel(liveElapsedMs(session, capturedAtMs, nowMs));
  const priority = agentStatePriorityFor(session.state);
  const factsRaw: Fact[] = heroFactTags(session.state, session.details);
  if (session.state === "starting") {
    factsRaw.push({ label: "Session", value: elapsed });
  } else if (session.state === "completed") {
    factsRaw.push({ label: "Duration", value: elapsed });
  } else if (session.state === "stale") {
    factsRaw.push({ label: "Last seen", value: `${elapsed} ago` });
  }
  const facts = factsRaw.slice(0, MAX_VISIBLE_DETAIL_PAIRS);
  const factsTone: FactTone =
    session.state === "waiting_for_permission" || session.state === "failed" ? "danger" : "accent";
  const runtimeLabel = agentRuntimeLabel(session.runtime);
  const subtitle = projectName !== null ? `${runtimeLabel} · ${projectName}` : runtimeLabel;

  return {
    dotKey: session.state,
    pulse: presentation.pulse,
    title: AGENT_HERO_TITLE[session.state],
    subtitle,
    body: session.summary,
    priority,
    facts,
    factsTone,
  };
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
    <motion.div
      layout="position"
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: "auto", opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={ROW_TRANSITION}
      style={{ overflow: "hidden" }}
      className={`agent-row ${presentation.className} ${agentRuntimeClass(session.runtime)}`}
    >
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
  const cwd = session.project?.cwd ?? null;
  const showCwd = cwd !== null && cwd !== projectName;
  const hostName = session.host?.name ?? null;
  const clearsIn =
    session.retentionRemainingMs !== null ? elapsedLabel(session.retentionRemainingMs) : null;
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
  expanded = false,
}: {
  sessions: AgentSessionView[];
  capturedAtMs: number;
  status?: StatusState;
  expanded?: boolean;
}) {
  const nowMs = useNowTick(sessions, capturedAtMs);

  if (sessions.length === 0) {
    return null;
  }

  const [primary, ...rest] = sessions;
  const primaryPresentation = agentStatePresentationFor(primary.state);
  const heroProps = agentHeroPropsFor(primary, capturedAtMs, nowMs);

  return (
    <div
      className={`card-assembly expanded agent-board-shell ${heroProps.priority}`}
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
        className={`below-block agent-board agent-origin ${primaryPresentation.className} ${agentRuntimeClass(primary.runtime)}`}
      >
        <AnimatePresence initial={false} mode="wait">
          <motion.div
            key={primary.id}
            className="agent-board-primary"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={HERO_SWAP_TRANSITION}
          >
            <AgentHeroCard {...heroProps} />
          </motion.div>
        </AnimatePresence>
        <div className="agent-board-swap">
          <AnimatePresence initial={false}>
            {rest.length === 0 ? null : expanded ? (
              <motion.div
                key="expanded"
                className="agent-board-expanded-list"
                data-testid="agent-board-expanded-list"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={DISCLOSURE_SPRING}
                style={{ ...SWAP_CELL_STYLE, overflow: "hidden" }}
              >
                <div className="agent-board-expanded-scroll">
                  <AnimatePresence initial={false}>
                    {rest.map((session) => (
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
              <motion.div
                key="resting"
                className="agent-board-rows"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={DISCLOSURE_SPRING}
                style={{ ...SWAP_CELL_STYLE, overflow: "hidden" }}
              >
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
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}
