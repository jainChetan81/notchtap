import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

// Shared visible-outcome mechanism: every operation that can silently fail
// reports through one instance of this hook. `announce`/`showPending` are
// per-ATTEMPT knobs; `error` is sticky until the next attempt, `ok` auto-clears.
export type ActionState = "idle" | "pending" | "ok" | "error";

export interface ActionStatusValue {
  state: ActionState;
  message?: string;
  announce: boolean;
}

export interface RunOptions<T = unknown> {
  /** true only for a user-initiated attempt whose result should be announced via aria-live. */
  announce: boolean;
  /**
   * message to show (and, if announce, speak) on success; omit to skip the
   * ok phase entirely — a silent success. A function derives the message
   * from the resolved value once the invoke settles.
   */
  okMessage?: string | ((result: T) => string);
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

// `label` is a debug/test seam: a genuine state transition (not a deduped
// repeat) logs once via console.debug, making transition-only behavior
// observable in tests.
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
      // Dedup: an identical status is a no-op — repeated identical poll
      // failures collapse to one transition, steady-state success stays silent.
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

  async function run<T>(action: () => Promise<T>, options: RunOptions<T>): Promise<T | undefined> {
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
      const message = typeof okMessage === "function" ? okMessage(result) : okMessage;
      if (message) {
        applyStatus({ state: "ok", message, announce });
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

// Renders the current ActionStatus. `announce` (set per-attempt by the caller
// of `run`) decides whether this instance carries aria-live — never on a
// pending render. `showPending` lets high-frequency actions skip the flicker.
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

  // Presence of `content` drives an AnimatePresence enter/exit; duration/ease
  // come from the ancestor MotionConfig. Pending never announces (it's a
  // flicker, not an outcome); ok/error announce only when `run()` opted in.
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
