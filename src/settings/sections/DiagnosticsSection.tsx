import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";

// plan 077: read-only tail of the active log file. Fetched on section-open
// (this component mounts only while the Diagnostics section is active), not
// on app load — the same advisory, isolated-from-panel-load pattern as
// get_default_config / get_connector_health. No live tail; the Refresh
// button re-invokes manually.
export function DiagnosticsSection() {
  const [logLines, setLogLines] = useState<string[] | null>(null);
  const { status, run } = useActionStatus("diagnostics");

  // `announce` is explicit per call, not a static prop: the mount-time read
  // below is passive (announce: false), the Refresh button's own call
  // further down is interactive (announce: true) — same operation, two
  // distinct attempt origins.
  function refresh(announce: boolean) {
    void run(() => settingsInvoke("get_recent_log_lines").then((fetched) => setLogLines(fetched)), {
      announce,
      showPending: false,
      errorMessage: () => "Couldn't read log lines",
    });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_recent_log_lines on every render.
  useEffect(() => {
    refresh(false);
  }, []);

  const logText =
    logLines === null
      ? "Loading…"
      : logLines.length === 0
        ? "No log lines yet."
        : logLines.join("\n");

  return (
    <SettingsGroup
      title="Recent log lines"
      description="The last 200 lines of ~/Library/Logs/notchtap/notchtap.log. Read-only; rotated backups are available via Console.app."
    >
      {/* plan 112 Step 3: was a raw inline style={{...}} (the sole
          CSS-in-JSX color literal the Step 0 inventory found —
          rgba(0,0,0,0.25)); ported to utilities, bg-black/25 reproduces
          the same composited color without a raw hex/rgb literal in
          code. fontSize stays a literal 11px (not the fs-body token) —
          it was already decoupled from the type-scale system before
          this migration, a fixed size for the monospace log viewer. */}
      <pre className="m-0 max-h-[320px] overflow-auto rounded-lg bg-black/25 p-3 font-mono text-[11px] leading-[1.5] whitespace-pre-wrap break-all select-text">
        {logText}
      </pre>
      <ActionStatus status={status} className="diagnostics-status" showPending={false} />
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="refresh-log-lines"
          name="Refresh"
          help="Re-read the log file. New lines appear as the app writes them."
        />
        <Button
          id="refresh-log-lines"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          onClick={() => refresh(true)}
        >
          Refresh
        </Button>
      </div>
    </SettingsGroup>
  );
}
