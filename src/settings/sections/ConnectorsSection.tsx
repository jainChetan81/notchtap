import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { ActionStatusValue } from "../actionStatus";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { SettingsGroup, ToggleControl } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { Config, ConnectorHealthDto, SecretField, SecretStatus } from "../types";

const secretRows: ReadonlyArray<{
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
}> = [
  {
    field: "openrouter_api_key",
    id: "openrouter-key",
    label: "OpenRouter API key",
    placeholder: "Enter a new key",
  },
  {
    field: "telegram_bot_token",
    id: "telegram-token",
    label: "Telegram bot token",
    placeholder: "Enter a replacement token",
  },
  {
    field: "telegram_chat_id",
    id: "telegram-chat-id",
    label: "Telegram chat ID",
    placeholder: "Enter a new chat ID",
  },
];

function SecretRow({
  field,
  id,
  label,
  placeholder,
  status,
  onSaved,
}: {
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
  status: string | null;
  onSaved: () => Promise<void>;
}) {
  const [value, setValue] = useState("");
  const { status: actionStatus, run } = useActionStatus("secret-save");
  const saving = actionStatus.state === "pending";

  async function saveSecret() {
    await run(
      async () => {
        await settingsInvoke("set_secret", { field, value });
        setValue("");
        await onSaved();
      },
      {
        announce: true,
        errorMessage: (reason) =>
          typeof reason === "string" ? reason : "secret could not be saved",
      },
    );
  }

  return (
    <div className="secret-row border-t border-border/60 py-[11px] pb-3 first:border-t-0">
      <div className="secret-meta mb-[7px] flex items-center justify-between gap-2.5">
        <label
          className="secret-label block text-fs-body leading-[1.3] font-[590] text-foreground"
          htmlFor={id}
        >
          {label}
        </label>
        <Badge
          aria-live="polite"
          variant="outline"
          className={cn(
            // plan 115: rounded-[4px] is intentionally off-scale (no
            // --radius-* rung is 4px; --radius-sm is 6px) — left as a
            // literal arbitrary value rather than shifting the visible
            // corner radius.
            "status-chip h-auto flex-none rounded-[4px] border-input px-[5px] py-[3px] font-mono text-fs-caption font-bold tracking-[0.06em] text-muted-foreground uppercase",
            status && "is-set border-ring/40 bg-input/40 text-foreground",
          )}
        >
          {status ?? "unset"}
        </Badge>
      </div>
      <div className="secret-controls grid grid-cols-[minmax(0,1fr)_auto] gap-[7px]">
        <Input
          id={id}
          type="password"
          autoComplete="new-password"
          placeholder={placeholder}
          value={value}
          onChange={(event) => setValue(event.currentTarget.value)}
          className="secret-input h-[31px] rounded-sm border-input bg-input/20 font-mono text-fs-secondary font-[560] text-foreground"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          aria-label={`Save ${label}`}
          disabled={saving || value.trim().length === 0}
          onClick={() => void saveSecret()}
        >
          {saving ? "Saving…" : "Save"}
        </Button>
      </div>
      <ActionStatus status={actionStatus} className="secret-error" />
    </div>
  );
}

function formatDeliveryAgo(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.floor(hours / 24)} d ago`;
}

// Read-only delivery-health line for the Telegram connector (plan 076) —
// advisory, like the SecretStatus fetch: a failed/unknown fetch renders
// nothing rather than an error.
function ConnectorHealthLine({ health }: { health: ConnectorHealthDto | null }) {
  if (!health) return null;
  let text: string;
  if (health.consecutiveFailures > 0) {
    text = `${health.consecutiveFailures} consecutive failure${health.consecutiveFailures === 1 ? "" : "s"} — check your bot token`;
  } else if (health.lastSuccessMs !== null) {
    text = `Last delivered: ${formatDeliveryAgo(health.lastSuccessMs)}`;
  } else {
    text = "No deliveries yet.";
  }
  return (
    <div className="relaunch-note mt-[-2px] mb-[11px] text-fs-caption tracking-[0.06em] text-muted-foreground uppercase">
      {text}
    </div>
  );
}

export function ConnectorsSection({
  config,
  secretStatus,
  connectorHealth,
  connectorHealthStatus,
  patchConfig,
  refreshSecretStatus,
}: {
  config: Config;
  secretStatus: SecretStatus | null;
  connectorHealth: ConnectorHealthDto | null;
  connectorHealthStatus: ActionStatusValue;
  patchConfig: (patch: Partial<Config>) => void;
  refreshSecretStatus: () => Promise<void>;
}) {
  return (
    <div className="section-stack">
      <SettingsGroup title="Telegram">
        <ToggleControl
          id="telegram-enabled"
          name="Enable connector"
          help="Forward every accepted event after Save & Relaunch."
          label="Enable Telegram connector"
          checked={config.connectors.telegram.enabled}
          onChange={(enabled) => patchConfig({ connectors: { telegram: { enabled } } })}
        />
        <div className="relaunch-note mt-[-2px] mb-[11px] text-fs-caption tracking-[0.06em] text-muted-foreground uppercase">
          Config change · applied after relaunch
        </div>
        <ConnectorHealthLine health={connectorHealth} />
        {/* Transition-only (plan 108): renders only on an ok<->failed flip,
            never aria-live — a passive setInterval poll must never chant. */}
        <ActionStatus
          status={connectorHealthStatus}
          className="connector-health-status"
          showPending={false}
        />
      </SettingsGroup>

      <SettingsGroup
        title="Write-only keys"
        description="Values never come back across IPC. Status reveals only whether a value is set and, when safe, its masked suffix."
      >
        {secretRows.map((row) => (
          <SecretRow
            key={row.field}
            {...row}
            status={secretStatus?.[row.field] ?? null}
            onSaved={refreshSecretStatus}
          />
        ))}
      </SettingsGroup>
    </div>
  );
}
