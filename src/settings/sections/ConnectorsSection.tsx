import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { MetaChip } from "@/components/ui/meta-chip";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { SecretField, SecretStatus } from "../types";

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
        <MetaChip aria-live="polite" uppercase active={!!status} className="status-chip flex-none">
          {status ?? "unset"}
        </MetaChip>
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

export function ConnectorsSection({
  secretStatus,
  refreshSecretStatus,
}: {
  secretStatus: SecretStatus | null;
  refreshSecretStatus: () => Promise<void>;
}) {
  return (
    <div className="section-stack">
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
