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

// 2026-08-02 (operator: "the compact<->expanded transition reads badly"):
// the single grid cell both branches of the resting<->expanded swap share,
// so a sync overlap crossfades them IN PLACE instead of stacking them and
// pushing the card taller. See that swap's own comment in the JSX below
// for the full argument, and App.tsx's `SURFACE_CELL_STYLE` for the
// identical mechanism one level up.
//
// Only the CELL is inline; the wrapper's own `display: grid` lives in
// agent-board.css (`.agent-board-swap`) rather than here, because a CSS
// rule there also has to be able to turn the wrapper OFF (`display: none`
// while it is empty, off the same `:has()` signal the board's padding
// rule uses) — an inline `display` would out-specify that rule.
const SWAP_CELL_STYLE = { gridArea: "1 / 1" } as const;

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

// Plan 169 fidelity pass (2026-08-02): the hero's per-state TITLE — prose
// naming what happened, exactly as `prototype/agent-board.html`'s proposal
// section writes it. Deliberately NOT the old `"<Runtime> — <State
// label>"` composition: the runtime name moved down into the subtitle
// (`primarySubtitle` below), so the title is free to be a sentence rather
// than a pair of labels.
//
// Lives here, not in `lib/presentation.ts`, because it is HERO COPY, not
// state presentation: `agentStatePresentationFor`'s own `label` (the
// short "Needs approval"/"Working" word every compact row and expanded
// row still shows) is unchanged and remains the shared lookup. Two
// waiting states deliberately share one string — the mock's own fixture
// does the same, since "needs input" is what both mean to the operator.
// Exhaustive `Record` so a new `AgentSessionState` is a compile error
// here until this table names it, the same discipline every other closed
// table in this app follows.
const AGENT_HERO_TITLE: Record<AgentSessionState, string> = {
  waiting_for_permission: "Agent needs input",
  waiting_for_input: "Agent needs input",
  working: "Agent working",
  starting: "Agent starting",
  completed: "Agent turn completed",
  failed: "Agent session failed",
  stale: "Agent session stale",
};

// Plan 169 fidelity pass: the risk values that earn the mock's coloured
// `DESTRUCTIVE` tag on a permission request's tool pill. A risk detail
// with any other value (a runtime that reports e.g. "read-only") is left
// as its own plain pill — the tag exists to flag the dangerous case, not
// to restate every risk level.
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

// Plan 169 fidelity pass: the two tags the mock's proposal fixtures show
// (`Tool rm DESTRUCTIVE` on a permission request, `Exit 1 ERROR` on a
// failure), derived ONLY from facts the session already carries — no tag
// is ever synthesized from the state alone:
//
//   - `waiting_for_permission`: a `Risk` detail whose value reads
//     destructive/blocked is FOLDED INTO the `Tool` detail's pill as its
//     tag (one pill saying "this tool, and it's destructive", which is
//     what the mock draws) and dropped as a standalone pill. Without a
//     tool pill to fold into, or with a risk the table doesn't flag, the
//     details are left exactly as the adapter sent them.
//   - `failed`: an `Exit`/`Exit code` detail with a nonzero value gets
//     the `error` tag on its own pill.
//
// Every other state (and every other label) passes through untouched.
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

// Plan 171 (tab-notch redesign, slice F): extracted verbatim from this
// component's own resting-hero rendering (below) so the tab-notch
// below-block's "viewed session" hero — a DIFFERENT session than
// whichever one this board's own priority ranking makes `primary` — can
// derive the identical `AgentHeroCard` props without a second, drifting
// copy of this logic. `AgentBoard`'s render below now calls this too;
// nothing about its own output changed.
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
  // Plan 171 (slice F): the full title/subtitle/body/facts/priority
  // derivation now lives in `agentHeroPropsFor` above (extracted, not
  // duplicated) — this board's own hero and the tab-notch below-block's
  // "viewed session" hero call the same function.
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
      {/* Plan 169 fidelity pass: `agent-origin` is the SHIPPED runtime
          wash + hairline every agent-origin notification card already
          carries (card-chrome.css's `.below-block.agent-origin`/`::before`
          — a corner radial keyed to `--cat-deep`, no motion). The mock's
          hero draws exactly that (`origin-wash`), and the board's own
          `src-<runtime>` class already sets the `--cat`/`--cat-deep`
          pair the rule reads (source-identity.css), so this is a class
          application, not new CSS. */}
      <div
        className={`below-block agent-board agent-origin ${primaryPresentation.className} ${agentRuntimeClass(primary.runtime)}`}
      >
        {/* Plan 142 fix (operator feedback, 2026-07-27): the hero block
            below and the expanded list's own first row both used to
            render `sessions[0]` — at N=1 that put the identical session
            on screen twice. That was fixed by hiding the hero while
            expanded; operator feedback (2026-08-02) rejected the cure:
            hovering a one-session Board swapped its big hero for a single
            skinny row, so "compact looks bigger than on hover" — a hover
            must never shrink the card or swap its content out. The fix
            now runs the other way: the hero for the PRIMARY session stays
            mounted in BOTH states, and the expanded list carries only the
            OTHER sessions (`rest`, never `sessions`), which keeps each
            session on screen exactly once at every N. */}
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
            head/runtime/state-pill/project/summary/elapsed block this
            file used to hand-roll (that block's own stylesheet rules were
            trimmed as plan 169 step 9's mandated follow-up, 2026-08-02 —
            they had had no consumer since this swap). The OUTER
            `motion.div` (identity-swap
            animation, `agent-board-primary` class) is untouched — this
            plan changes what renders INSIDE `.below-block`, never the
            shell or the swap's own animation contract (Boundaries). */}
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
        {/* Plan 142 (spec §6.2 expanded): while `expanded`, the compact
            `rest` rows swap for a bounded, scrollable list of those SAME
            non-primary sessions in their richer form (the primary stays
            in the hero above, in both states) — rust has
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
        {/* 2026-08-02 animation audit (finding #4) + operator feedback the
            same day ("the compact<->expanded transition reads badly").
            This block used to run `mode="wait"`, which made the "morphs"
            claim above false: the resting rows had to collapse ALL THE
            WAY to height 0 before the expanded list was allowed to start
            growing from 0, so a hover played as two chained beats with a
            full pinch to nothing between them, at double the settle time.

            Fixed with the same technique App.tsx's surface swap uses —
            the DEFAULT (sync) `AnimatePresence` mode inside a single-cell
            grid stack (`.agent-board-swap`, agent-board.css; both
            branches pinned to `gridArea: "1 / 1"`). The two lists now
            overlap in place, and because a grid row is sized to the MAX
            of its items rather than their SUM, the container's height
            traces `max(outgoing, incoming)` — one continuous size change
            instead of down-to-zero-then-up.

            Why both children KEEP their `height` spring rather than
            simplifying to opacity-only children with the grid carrying
            the size: with opacity-only children both lists would sit at
            their natural heights for the whole overlap, so the row would
            jump straight to `max(H_resting, H_expanded)` on the swap
            frame and snap to the survivor when the exit finished — two
            instant steps, no morph at all. With both heights on the SAME
            spring (one config, started the same frame), `max()` traces a
            genuinely animated path in both directions. It is not
            perfectly monotonic — the two curves cross at
            `H1*H2/(H1+H2)`, a transient dip of a few px below the
            smaller height — but that is a small fraction of the travel,
            against a former pinch of 100% of it.

            The two branches are deliberately NOT converged into one
            component (the audit's original reason for deferring this):
            `AgentRow` is a single nowrap line, `ExpandedAgentRow` is a
            multi-block card with meta/detail chips and its own hover
            history disclosure. This is a transition fix only — the
            settled per-row add/remove animations (`ROW_TRANSITION` +
            `layout="position"` + the inner `AnimatePresence
            initial={false}`), each branch's own `overflow: hidden`
            clipping, and the `.agent-board-expanded-scroll` max-height
            cap are all untouched. */}
        {/* One shared `rest.length > 0` guard over BOTH branches: with a
            single session there is nothing below the hero to show in
            either state, so hovering a one-session Board simply keeps the
            hero (operator feedback, 2026-08-02 — hover must not shrink or
            swap the card).

            The guard stays INSIDE `.agent-board-swap` rather than around
            it, so a last row leaving still gets its exit animation (an
            unmounting wrapper would cut it). The wrapper is instead
            hidden by CSS while it holds nothing, off the same `:has()`
            signal `.agent-board`'s own padding rule already reads —
            otherwise an empty wrapper would still collect the parent
            flex `gap` and re-add the dead band under a one-session hero
            that the CONTENT-HUG pass just removed. */}
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
                    {/* `rest`, NOT `sessions`: the primary session is the
                      hero above (in both states), so listing `sessions`
                      here would render it twice — the same N=1 double
                      render plan 142 already fixed once, just from the
                      other side. */}
                    {rest.map((session) => (
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
              <motion.div
                key="resting"
                className="agent-board-rows"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={DISCLOSURE_SPRING}
                style={{ ...SWAP_CELL_STYLE, overflow: "hidden" }}
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
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}
