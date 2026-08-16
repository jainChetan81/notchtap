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
  type Priority,
} from "../lib/presentation";
import type { AgentSessionState, AgentSessionView } from "../useAgentState";
import type { StatusState } from "../useStatusState";
import { FlankClock } from "./FlankClock";
import {
  AgentHeroCard,
  type Fact,
  type FactTone,
  MAX_VISIBLE_DETAIL_PAIRS,
} from "./NotificationBody";
import { StatusDots } from "./StatusDots";
import type { Detail } from "./StatusRailCard";

// The Agent Board's resting layout — one rich card for the
// highest-ranked session (`sessions[0]`, already Rust-ordered; this
// component does no sorting of its own) plus every other session as
// an individual compact row, never a "+N" collapse.
//
// Mounted inside the SAME `.card-assembly`/`.flank-left`/
// `.synthetic-cutout`/`.flank-right`/`.below-block` shell
// StatusRailCard.tsx uses (card-chrome.css), inheriting the cutout
// shape, rounding law, and Appearance controls. App.tsx swaps
// between the two components based on `presentationMode`.

// The board's local wall-clock ticks only as fast as the fastest
// label it drives can change: `elapsedLabel` is second-granular
// below 60s and minute-granular above it, so past a minute the
// one-second rate would re-render byte-identical output endlessly.
// The fast rate stays for the sub-minute window; 15s is well under
// the 60s boundary, so a label never looks more than a quarter
// minute stale.
const FAST_NOW_TICK_MS = 1000;
const SLOW_NOW_TICK_MS = 15_000;
// The granularity boundary in `elapsedLabel` itself — above this,
// the label only changes once a minute.
const SECOND_GRANULAR_BELOW_MS = 60_000;

// One shared const drives enter, exit, AND layout (sibling reflow);
// three hand-copied literals would invite desynced clocks — the same
// failure class as `dedup_eq` drift. CRITICALLY DAMPED (`bounce: 0`,
// no gesture momentum to preserve here) with a 0.35s settle.
// Exported so tests can pin these exact values without duplicating
// them (a second copy in the test would be the drift this exists to
// prevent).
export const ROW_TRANSITION = { type: "spring", bounce: 0, duration: 0.35 } as const;

// The hero's identity swap — played only when a DIFFERENT session
// becomes primary (keyed on `primary.id`), never when the same
// session just changes state (that morphs in place: dot colour
// transition + state tick). Same one-const discipline as
// `ROW_TRANSITION`, exported for the same reason.
// A short tween, not a spring: a content SWAP (out, then in via
// `mode="wait"`) has no momentum to preserve; overshoot would read as
// a wobble. 6px of travel gives the swap a direction.
export const HERO_SWAP_TRANSITION = { duration: 0.16, ease: NOTCHTAP_EASE } as const;

// The single grid cell both branches of the resting<->expanded swap
// share, so a sync overlap crossfades them IN PLACE instead of
// stacking them and pushing the card taller.
// Only the CELL is inline; the wrapper's own `display: grid` lives in
// agent-board.css (`.agent-board-swap`) because a CSS rule there also
// turns the wrapper OFF (`display: none` when empty, off the same
// `:has()` signal) — an inline `display` would out-specify that rule.
const SWAP_CELL_STYLE = { gridArea: "1 / 1" } as const;

/// Local wall-clock tick — re-renders the board so every row's
/// elapsed-in-state label stays live, WITHOUT rust publishing a
/// per-second `agent-state` event (a continuously-varying field must
/// never drive a wire emission). The interval is re-derived on every
/// render (a session crossing the 60s boundary re-evaluates it), but
/// the effect only re-subscribes when the interval actually changes.
function useNowTick(sessions: AgentSessionView[], capturedAtMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  const intervalMs = nowTickIntervalMs(sessions, capturedAtMs, now);
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

// Fast tick only while at least ONE session's live elapsed is
// still inside `elapsedLabel`'s second-granular window; slow
// otherwise. Exported so the test can pin the selection directly.
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
/// time has passed since, so a continuously-varying value never rides
/// the wire.
function liveElapsedMs(session: AgentSessionView, capturedAtMs: number, nowMs: number): number {
  return session.elapsedMs + Math.max(0, nowMs - capturedAtMs);
}

// The hero's per-state TITLE — prose naming what happened, not a
// `"<Runtime> — <State label>"` composition: the runtime name lives
// in the subtitle, so the title can be a sentence.
//
// Lives here, not in `lib/presentation.ts`, because it is HERO COPY,
// not state presentation: `agentStatePresentationFor`'s `label` (the
// short "Needs approval"/"Working" word rows still show) stays the
// shared lookup. Two waiting states deliberately share one string —
// "needs input" is what both mean to the operator. Exhaustive
// `Record`: a new `AgentSessionState` is a compile error here until
// this table names it.
const AGENT_HERO_TITLE = {
  waiting_for_permission: "Agent needs input",
  waiting_for_input: "Agent needs input",
  working: "Agent working",
  starting: "Agent starting",
  completed: "Agent turn completed",
  failed: "Agent session failed",
  stale: "Agent session stale",
} satisfies Record<AgentSessionState, string>;

// The risk values that earn the coloured `DESTRUCTIVE` tag on a
// permission request's tool pill. Any other risk value (e.g.
// "read-only") stays its own plain pill — the tag flags the
// dangerous case, it doesn't restate every risk level.
const TAGGED_RISKS = new Set(["destructive", "blocked"]);

function normalizedLabel(label: string): string {
  return label.trim().toLowerCase();
}

// A nonzero, parseable exit code. `"0"` (a clean exit reported on a
// failed session) and a non-numeric value both fall through untagged
// rather than being asserted as errors.
function isNonzeroExitValue(value: string): boolean {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) && parsed !== 0;
}

// The two tags the mock's proposal fixtures show (`Tool rm
// DESTRUCTIVE`, `Exit 1 ERROR`), derived ONLY from facts the session
// already carries — never synthesized from the state alone:
//
//   - `waiting_for_permission`: a `Risk` detail reading
//     destructive/blocked is folded INTO the `Tool` detail's pill as
//     its tag and dropped as a standalone pill. Without a tool pill to
//     fold into, or an unflagged risk, the details pass through
//     exactly as the adapter sent them.
//   - `failed`: an `Exit`/`Exit code` detail with a nonzero value gets
//     the `error` tag on its own pill.
//
// Every other state and label passes through untouched.
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

// Shared `AgentHeroCard` prop derivation for ANY session: the board's
// own resting-hero primary AND the tab-notch below-block's "viewed
// session" hero (a DIFFERENT session) both call this, so the identical
// props can't drift into a second copy of the logic.
export function agentHeroPropsFor(
  session: AgentSessionView,
  capturedAtMs: number,
  nowMs: number,
): {
  dotKey: string;
  pulse: boolean;
  title: string;
  subtitle: string;
  body: string | null;
  priority: Priority;
  facts: Fact[];
  factsTone: FactTone;
} {
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

// One row of the hover-expanded, scrollable session list — every
// retained session renders through this, in the same Rust-provided
// order, no re-sorting. Click-free: the transition history discloses
// on the row's OWN mouse-enter/leave — real pointer events genuinely
// land in the webview while expanded, because rust temporarily
// disables click-through for exactly this rect (`lib.rs`'s
// `try_expand_board_for_hover`); the wire's single card-level
// `hovered` signal can't distinguish which row the cursor is over.
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
  // The session's active subagent, one more meta chip alongside
  // cwd/host/clears-in — same "nothing renders when absent"
  // discipline. Falls back to `id` when the runtime hasn't supplied a
  // human `label`; the subagent's own state appears in parens only
  // when the runtime reports one.
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
      {/* A restrained extra line of small muted mono chips for wire
          fields (`project.cwd`, `host.name`, terminal retention) —
          same "nothing renders if absent" discipline as the detail
          cells below, no placeholder chips. */}
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
      {/* Capability-dependent detail cells — an adapter that never
          declared/observed a detail simply has an empty `details`
          array, so nothing renders, no placeholder cell. */}
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
  // `expanded` is the board's hover state: sourced from App.tsx's
  // `hover-changed`-driven `hovered` boolean (the same rust-emitted
  // one StatusRailCard's hover consumers use). Only meaningful while
  // this component is mounted — a `true` here means "the cursor is
  // over the Board", never another card; hovering the Board is what
  // flips resting rows into the expanded list.
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
  // The full title/subtitle/body/facts/priority derivation lives in
  // `agentHeroPropsFor` above — this board's own hero and the
  // tab-notch below-block's "viewed session" hero call the same
  // function.
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
      {/* `agent-origin` is the SHIPPED runtime wash + hairline every
          agent-origin notification card carries (card-chrome.css's
          `.below-block.agent-origin`/`::before` — a corner radial
          keyed to `--cat-deep`, no motion). The board's own
          `src-<runtime>` class already sets the `--cat`/`--cat-deep`
          pair that rule reads, so this is a class application, not
          new CSS. */}
      <div
        className={`below-block agent-board agent-origin ${primaryPresentation.className} ${agentRuntimeClass(primary.runtime)}`}
      >
        {/* The hero block below and the expanded list are disjoint:
            the hero for the PRIMARY session stays mounted in BOTH
            states, and the expanded list carries only the OTHER
            sessions (`rest`, never `sessions`) — each session appears
            exactly once at every N, and a hover never shrinks the
            card or swaps its content out. */}
        {/* The hero's identity swap is keyed on `primary.id` ONLY: a
            change in which session is primary plays the swap; a state
            change within the same session morphs in place (dot colour
            transition + state tick, agent-board.css). Keying on state
            too would re-animate the whole block on ordinary updates.
            `mode="wait"` because the hero is a single block — an
            overlap would double its height mid-flight. */}
        {/* The hero's INNER content renders through `AgentHeroCard`
            (NotificationBody.tsx) — the same masthead-row/Stamp/title/
            subtitle/body/fact-pill template every other origin's
            compact card uses. The OUTER `motion.div` (identity-swap
            animation, `agent-board-primary` class) is untouched: only
            what renders inside `.below-block` changes here, never the
            shell or the swap's own animation contract. */}
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
        {/* While `expanded`, the compact `rest` rows swap for a bounded,
            scrollable list of the SAME non-primary sessions in richer
            form (the primary stays in the hero above, both states).
            Rust has already grown the real window to a screen-bounded
            frame and opened pointer delivery for exactly this rect by
            the time this prop flips true (`try_expand_board_for_hover`),
            so the scroll container genuinely has room and receives
            wheel events. */}
        {/* The DEFAULT (sync) `AnimatePresence` mode inside a single-cell
            grid stack (`.agent-board-swap`, agent-board.css; both
            branches pinned to `gridArea: "1 / 1"`, the `SWAP_CELL_STYLE`
            above): the two lists overlap in place, and because a grid
            row is sized to the MAX of its items rather than their SUM,
            the container's height traces `max(outgoing, incoming)` — a
            single continuous size change instead of down-to-zero-then-up,
            regardless of which branch is taller.
            Both children KEEP their `height` spring (rather than
            opacity-only children with the grid carrying all the size):
            opacity-only lists would sit at natural heights during the
            overlap, jumping straight to `max(H_resting, H_expanded)`
            and snapping when the exit finished — two instant steps, no
            morph. With both heights animating on the SAME spring, the
            `max()` traces an animated path both ways.
            The two branches are deliberately NOT one component:
            `AgentRow` is a single nowrap line, `ExpandedAgentRow` is a
            multi-block card with meta/detail chips and its own hover
            history disclosure. Everything else — per-row add/remove
            transitions (`ROW_TRANSITION` + `layout="position"`), the
            inner `AnimatePresence initial={false}`, each branch's own
            `overflow: hidden` clipping, and the
            `.agent-board-expanded-scroll` max-height — is untouched. */}
        {/* One shared `rest.length > 0` guard over BOTH branches: with a
            single session there is nothing below the hero to show in
            either state, and hovering a one-session Board simply keeps
            the hero — a hover must never shrink or swap the card.
            The guard stays INSIDE `.agent-board-swap` rather than
            around it so a last row leaving still gets its exit
            animation (an unmounting wrapper would cut it). The empty
            wrapper is hidden by CSS off the same `:has()` signal
            `.agent-board`'s padding rule reads — otherwise it would
            still collect the parent flex `gap` and re-add the dead
            band under a one-session hero. */}
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
                  {/* `initial={false}` mirrors the outer `AnimatePresence` blocks in
                    this file — the whole board mounting (or `expanded`
                    flipping true for the first time) shouldn't
                    stagger-animate every already-present row in; only a
                    genuine per-session add/remove/reorder animates.
                    Default ("sync") mode, not "wait" — an exit and its
                    siblings' reflow play concurrently, so a removal
                    reads as one fluid motion, not two separate beats. */}
                  <AnimatePresence initial={false}>
                    {/* `rest`, NOT `sessions`: the primary session is the
                      hero above (in both states), so listing `sessions`
                      here would render it twice. */}
                    {rest.map((session) => (
                      // Share ONE transition (`ROW_TRANSITION`) for a
                      // departing row's collapse, an arriving row's
                      // expand, and `layout="position"`'s sibling reflow —
                      // the exit path mirrors exactly. Overflow hidden on
                      // THIS row (not the `.agent-board-expanded-scroll`
                      // container, which keeps its own `overflow-y: auto`)
                      // so the collapsing row doesn't spill during
                      // animation.
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
                {/* `initial={false}` — this block already fades/grows in as a
                  whole on its own first mount, so individual rows
                  inside shouldn't ALSO stagger-animate in on top of
                  that; only a genuine per-session add/remove/reorder
                  triggers `AgentRow`'s own enter/exit. Default
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
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}
