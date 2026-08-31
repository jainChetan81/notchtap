import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { type ActionError, describeActionError } from "@/lib/error";
import { isFunction } from "@/lib/guards";
import { cn } from "@/lib/utils";

export { type ActionError, describeActionError };

export type ActionState = "idle" | "pending" | "ok" | "error";

export interface ActionStatusValue {
  state: ActionState;
  message?: string;
  announce: boolean;
}

export interface RunOptions<T = unknown> {
  announce: boolean;
  okMessage?: string | ((result: T) => string);
  okClearMs?: number;
  showPending?: boolean;
  errorMessage?: (reason: ActionError) => string;
}

const DEFAULT_OK_CLEAR_MS = 2500;

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
      // SAFETY: isFunction is the runtime check that okMessage is callable — the cast only narrows the generic signature after that check.
      const message = isFunction(okMessage) ? (okMessage as (r: T) => string)(result) : okMessage;
      if (message) {
        applyStatus({ state: "ok", message, announce });
        clearTimerRef.current = window.setTimeout(() => {
          applyStatus({ state: "idle", announce: false });
        }, okClearMs);
      } else {
        applyStatus({ state: "idle", announce: false });
      }
      return result;
    } catch (reason: unknown) {
      // SAFETY: catch variable is unknown — narrowed via ActionError checks inside describeActionError/errorMessage.
      const message = errorMessage
        ? errorMessage(reason as ActionError)
        : describeActionError(reason as ActionError);
      applyStatus({ state: "error", message, announce });
      return undefined;
    }
  }

  return { status, run };
}

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
