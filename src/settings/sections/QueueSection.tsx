import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { QueueItemSummary } from "../types";
import { PRIORITY_LABELS } from "../types";

// plan 121: read-only visibility into the WAITING items behind the
// visible card (the queue_total/queue_done dots on the overlay are the
// only prior visibility into this — no list, no way to act on it), plus
// two controls: Skip current (dismiss the visible card now, promoting
// the next waiting item — routes through skip_visible's existing
// semantics) and Clear queue (drop every waiting item; the visible card
// is untouched and finishes its normal ttl/rotation). Same fetch-on-open
// + manual Refresh shape as DiagnosticsSection. Titles are UNTRUSTED
// wire data — rendered as plain text only, same rule as History's
// link-as-literal-text precedent (no dangerouslySetInnerHTML, no <a
// href>, no markdown rendering).
export function QueueSection() {
  const [items, setItems] = useState<QueueItemSummary[] | null>(null);
  const loadStatus = useActionStatus("queue-load");
  const skipStatus = useActionStatus("queue-skip");
  const clearStatus = useActionStatus("queue-clear");

  // `announce` is explicit per call, not a static prop — same split as
  // DiagnosticsSection's own `refresh`: the mount-time read is passive,
  // the Refresh button's call is a user-initiated attempt.
  function refresh(announce: boolean) {
    void loadStatus.run(() => settingsInvoke("get_queue").then((fetched) => setItems(fetched)), {
      announce,
      showPending: false,
      errorMessage: () => "Couldn't load the queue",
    });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_queue on every render.
  useEffect(() => {
    refresh(false);
  }, []);

  async function handleSkipClick() {
    await skipStatus.run(() => settingsInvoke("skip_current"), {
      announce: true,
      okMessage: "Skipped",
      errorMessage: () => "Couldn't skip the current notification",
    });
    refresh(false);
  }

  async function handleClearClick() {
    await clearStatus.run(() => settingsInvoke("clear_queue"), {
      announce: true,
      okMessage: "Queue cleared",
      errorMessage: () => "Couldn't clear the queue",
    });
    refresh(false);
  }

  return (
    <SettingsGroup
      title="Waiting notifications"
      description="What's queued behind the visible card, and controls to skip or clear it."
    >
      <ActionStatus status={loadStatus.status} className="queue-load-status" showPending={false} />
      {items === null ? (
        // plan 124 (F6): a failed mount fetch used to leave `items` at
        // `null` forever, so "Loading…" rendered underneath the sticky
        // ActionStatus error above it with no way to tell the two apart at
        // a glance. `loadStatus.status.state` is the same signal the
        // ActionStatus banner above already reads — reusing it here (not a
        // second error flag) means this can never disagree with the
        // banner about whether the last attempt failed.
        loadStatus.status.state === "error" ? (
          <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">
            Couldn't load the queue — Refresh to retry.
          </p>
        ) : (
          <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">Loading…</p>
        )
      ) : items.length === 0 ? (
        <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">Queue is empty.</p>
      ) : (
        <ul className="queue-list flex flex-col py-1 pb-[11px]">
          {items.map((item, index) => (
            <li
              // biome-ignore lint/suspicious/noArrayIndexKey: `get_queue` returns plain summaries with no id — this list is always replaced wholesale by the next refresh() (never reordered or spliced in place), so the index is a stable-enough positional identity for one fetched snapshot.
              key={`${index}:${item.title}`}
              className="queue-row grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border-t border-border/60 py-2 first:border-t-0"
            >
              <span className="queue-title min-w-0 text-fs-body text-foreground [overflow-wrap:anywhere]">
                {item.title}
              </span>
              <span className="queue-priority-tag min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground">
                {PRIORITY_LABELS[item.priority]}
              </span>
            </li>
          ))}
        </ul>
      )}
      {/* plan 124 (F1): the control this section's own top-of-file comment
          ("fetch-on-open + manual Refresh") always claimed but never
          rendered — `refresh(announce)` already existed, this just wires
          a button to it, following DiagnosticsSection's own Refresh row
          (`../sections/DiagnosticsSection.tsx`) exactly: `refresh(true)`
          (announced — the plan-108 user-initiated rule), never a static
          prop. The mount-time fetch above stays `refresh(false)`. */}
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="refresh-queue"
          name="Refresh"
          help="Re-fetch the waiting list. Not live — this is a manual pull."
        />
        <Button
          id="refresh-queue"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          onClick={() => refresh(true)}
        >
          Refresh
        </Button>
      </div>
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="skip-current"
          name="Skip current"
          help="Dismiss the visible card now and promote the next waiting item."
        />
        <Button
          id="skip-current"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={skipStatus.status.state === "pending"}
          onClick={() => void handleSkipClick()}
        >
          Skip current
        </Button>
      </div>
      <ActionStatus status={skipStatus.status} className="queue-skip-status" />
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="clear-queue"
          name="Clear queue"
          help="Drops every waiting notification. The visible card is unaffected and finishes its normal turn."
        />
        <Button
          id="clear-queue"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={clearStatus.status.state === "pending"}
          onClick={() => void handleClearClick()}
        >
          Clear queue
        </Button>
      </div>
      <ActionStatus status={clearStatus.status} className="queue-clear-status" />
    </SettingsGroup>
  );
}
