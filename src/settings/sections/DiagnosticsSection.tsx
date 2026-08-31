import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";

export function DiagnosticsSection() {
  const [logLines, setLogLines] = useState<string[] | null>(null);
  const { status, run } = useActionStatus("diagnostics");

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
