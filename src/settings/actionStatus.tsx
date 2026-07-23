import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

// Shared visible-outcome mechanism (plan 108). Every operation that can
// silently fail — send-test, appearance hot-apply, history read/clear,
// diagnostics read, connector-health read, defaults fetch — reports
// through one instance of this hook, rendered through the ActionStatus
// component below. Two knobs are per-ATTEMPT, not per-component: `announce`
// (aria-live is for user-initiated attempts only — a passive mount read or
// a background poll must never speak up on its own) and `showPending`
// (high-frequency actions like a slider drag should never flicker a
// "working" state). `error` is sticky until the next attempt settles it;
// `ok` auto-clears.
export type ActionState = "idle" | "pending" | "ok" | "error";

export interface ActionStatusValue {
  state: ActionState;
  message?: string;
  announce: boolean;
}

export interface RunOptions {
  /** true only for a user-initiated attempt whose result should be announced via aria-live. */
  announce: boolean;
  /** message to show (and, if announce, speak) on success; omit to skip the ok phase entirely — a silent success. */
  okMessage?: string;
  /** ms before an ok status clears back to idle. */
  okClearMs?: number;
  /** whether to surface a pending status at all while the action is in flight (default true). */
  showPending?: boolean;
  /** derive the user-facing message from the rejection reason; defaults to describeActionError. */
  errorMessage?: (reason: unknown) => string;
}

const DEFAULT_OK_CLEAR_MS = 2500;

export function describeActionError(reason: unknown): string {
  if (Array.isArray(reason)) return reason.map(String).join(", ");
  if (typeof reason === "string") return reason;
  return "Something went wrong";
}

// `label` is optional and purely a debug/test seam: a genuine state
// transition (not a deduped repeat) logs once via console.debug. This is
// what makes the connector-health poll's transition-only behavior
// observable in tests without inventing a second render-tracking
// mechanism — see the done criteria in plan 108.
export function useActionStatus(label?: string) {
  const [status, setStatus] = useState<ActionStatusValue>({ state: "idle", announce: false });
  const clearTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (clearTimerRef.current !== null) window.clearTimeout(clearTimerRef.current);
    };
  }, []);

  function clearOkTimer() {
    if (clearTimerRef.current !== null) {
      window.clearTimeout(clearTimerRef.current);
      clearTimerRef.current = null;
    }
  }

  function applyStatus(next: ActionStatusValue) {
    setStatus((prev) => {
      // Dedup: an identical status is a no-op, not a new render — this is
      // what makes repeated identical poll failures collapse to a single
      // transition, and steady-state success polling stay silent.
      if (
        prev.state === next.state &&
        prev.message === next.message &&
        prev.announce === next.announce
      ) {
        return prev;
      }
      if (label) {
        console.debug(`[action-status:${label}]`, prev.state, "->", next.state);
      }
      return next;
    });
  }

  async function run<T>(action: () => Promise<T>, options: RunOptions): Promise<T | undefined> {
    const {
      announce,
      okMessage,
      okClearMs = DEFAULT_OK_CLEAR_MS,
      showPending = true,
      errorMessage,
    } = options;
    clearOkTimer();
    if (showPending) applyStatus({ state: "pending", announce });
    try {
      const result = await action();
      if (okMessage) {
        applyStatus({ state: "ok", message: okMessage, announce });
        clearTimerRef.current = window.setTimeout(() => {
          applyStatus({ state: "idle", announce: false });
        }, okClearMs);
      } else {
        applyStatus({ state: "idle", announce: false });
      }
      return result;
    } catch (reason) {
      const message = errorMessage ? errorMessage(reason) : describeActionError(reason);
      applyStatus({ state: "error", message, announce });
      return undefined;
    }
  }

  return { status, run };
}

// Renders the current ActionStatus. `announce` on the status value (set
// per-attempt by the caller of `run`, not hardcoded per component) decides
// whether this instance carries aria-live — never on a passive/pending
// render. `showPending` lets a high-frequency action (e.g. the appearance
// sliders) opt out of a "working" flicker entirely.
//
// plan 112 Step 3: markup/behavior (which element, when aria-live is
// present, dedup/announce policy) is unchanged from Plan 108 — only the
// per-state color moves to utilities, following the token table verbatim
// (pending -> text-muted-foreground, ok -> text-overlay-teal, error ->
// text-destructive). The two old ancestor-selector overrides
// (`.test-button-wrap .action-status` / `.settings-footer .action-status`,
// both just `margin-top: 0`, the wrap variant also `text-align: right`)
// have no single-element utility equivalent from inside this component, so
// callers in those two contexts pass the override through `className`;
// `cn` (clsx + tailwind-merge) resolves the conflicting `mt-*` in the
// caller's favor since it's applied last.
export function ActionStatus({
  status,
  className,
  showPending = true,
}: {
  status: ActionStatusValue;
  className?: string;
  showPending?: boolean;
}) {
  const stateClasses =
    status.state === "pending"
      ? "text-muted-foreground"
      : status.state === "ok"
        ? "text-overlay-teal"
        : "text-destructive";
  const classes = cn(
    "action-status",
    `is-${status.state}`,
    "mt-1.5 text-fs-secondary leading-[1.4]",
    stateClasses,
    className,
  );

  // Same mount/unmount pattern as SettingsApp.tsx's ErrorPanel (plan 126):
  // presence of `content` drives an AnimatePresence enter/exit instead of an
  // abrupt appear/disappear. Duration/ease come from the ancestor
  // MotionConfig (SettingsApp.tsx), not a local literal — same as
  // ErrorPanel. `live` mirrors the exact aria-live rule the two branches
  // below used to encode directly: pending never announces (even mid a
  // user-initiated attempt — it's a "working" flicker, not an outcome);
  // ok/error announce only when the triggering `run()` call opted in.
  let content: string | null = null;
  let live = false;
  if (status.state === "pending") {
    if (showPending) content = "Working…";
  } else if (status.message) {
    content = status.message;
    live = status.announce;
  }

  return (
    <AnimatePresence initial={false}>
      {content !== null ? (
        <motion.div
          className={classes}
          aria-live={live ? "polite" : undefined}
          initial={{ opacity: 0, y: -3 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -3 }}
        >
          {content}
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
