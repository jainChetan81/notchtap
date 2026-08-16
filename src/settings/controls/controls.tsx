import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch as UiSwitch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { ActionStatus, describeActionError, useActionStatus } from "../actionStatus";
import { settingsInvoke } from "../ipc";
import type { TestSource } from "../types";

// Shared row shell for every control kind; `first:border-t-0` gives every row
// but the first in its group a top divider.
export const CONTROL_ROW =
  "control-row grid min-h-[58px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-border/60 py-2.5 first:border-t-0";

// shadcn Card shell. gap-0/py-0/ring-0 strip Card's own spacing/ring defaults
// so they don't double up with the explicit padding below.
export function SettingsGroup({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <Card
      // Card's default `text-sm` is inherited, so it would silently reach the
      // Appearance preview subtree, which relies on the browser's 16px/normal
      // baseline; `text-base leading-[normal]` restores exactly that baseline.
      className="gap-0 overflow-hidden rounded-md border border-border bg-card py-0 text-base leading-[normal] ring-0"
    >
      <CardHeader
        // CardHeader's `[.border-b]:pb-(--card-spacing)` rule keys on the
        // literal "border-b" token and re-widens padding-bottom; the trailing
        // `!` forces this pb to win despite that rule's higher specificity.
        className="gap-[5px] border-b border-border/60 px-[13px] pt-3 pb-[11px]!"
      >
        <CardTitle className="text-fs-body leading-[1.25] font-[640] text-foreground">
          {title}
        </CardTitle>
        {description ? (
          <CardDescription className="text-fs-secondary leading-[1.45] text-muted-foreground">
            {description}
          </CardDescription>
        ) : null}
      </CardHeader>
      <CardContent className="px-[13px]">{children}</CardContent>
    </Card>
  );
}

export function ControlCopy({
  htmlFor,
  name,
  help,
}: {
  htmlFor: string;
  name: string;
  help: string;
}) {
  return (
    <div className="control-copy min-w-0">
      {/* id lets a sibling fieldset (Segmented's labelled form) point
          aria-labelledby back here — fieldset isn't a labelable element,
          so <label for> alone can't associate with it. */}
      <label
        className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground"
        id={`${htmlFor}-label`}
        htmlFor={htmlFor}
      >
        {name}
      </label>
      <span className="control-help mt-[3px] block text-fs-secondary leading-[1.4] text-muted-foreground">
        {help}
      </span>
    </div>
  );
}

export function NumberControl({
  id,
  name,
  help,
  value,
  min,
  max,
  unit,
  step,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  /** HTML `step` attribute. Defaults to `1` (integer fields); pass
   *  `"any"` for decimal fields (e.g. latitude/longitude) so a partial
   *  value like `12.5` isn't flagged as a `stepMismatch`. */
  step?: number | "any";
  onChange: (value: number) => void;
}) {
  // Local raw-string mirror of `value`: a controlled numeric value coerces a
  // cleared field to 0 and snaps an in-progress "12." back to "12". Keeping
  // the in-progress text in state lets the user type without being fought.
  const [raw, setRaw] = useState(() => String(value));

  useEffect(() => {
    setRaw(String(value));
  }, [value]);

  return (
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <div className="number-field relative w-24 flex-none">
        <Input
          id={id}
          type="number"
          min={min}
          max={max}
          step={step ?? 1}
          value={raw}
          inputMode="numeric"
          onChange={(event) => {
            const next = event.currentTarget.value;
            setRaw(next);
            // Empty or a bare sign/decimal point mid-entry: don't propagate
            // until the input reads as a real number.
            if (next === "" || next === "-" || next === "." || next === "-.") {
              return;
            }
            const parsed = Number(next);
            if (!Number.isNaN(parsed)) {
              onChange(parsed);
            }
          }}
          onBlur={() => {
            // Leaving the field on an invalid/empty value restores the
            // last-committed value rather than leaving the box blank.
            if (raw === "" || Number.isNaN(Number(raw))) {
              setRaw(String(value));
            }
          }}
          className={cn(
            "h-[31px] rounded-sm border-input bg-input/20 text-right font-mono text-fs-body font-[650] text-foreground",
            unit ? "pr-10" : "pr-2.5",
          )}
        />
        {unit ? (
          <span className="unit pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-fs-caption font-bold tracking-[0.05em] text-muted-foreground">
            {unit}
          </span>
        ) : null}
      </div>
    </div>
  );
}

// shadcn Switch plus a visually-hidden Label (ControlCopy renders the visible
// name); `screen.getByLabelText` resolves it via the label[for] association.
export function ToggleControl({
  id,
  name,
  help,
  label,
  checked,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Label htmlFor={id} className="sr-only">
        {label}
      </Label>
      <UiSwitch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

export function TextareaControl({
  id,
  name,
  help,
  value,
  caption,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: string;
  caption: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="textarea-control border-t border-border/60 pt-[11px] pb-3 first:border-t-0">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Textarea
        id={id}
        spellCheck={false}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        className="mt-2 min-h-[73px] resize-y rounded-md border-input bg-input/20 px-2.5 py-2 font-mono text-fs-secondary font-[560] leading-[1.55] text-foreground"
      />
      <div className="field-caption mt-[5px] text-fs-caption font-bold tracking-[0.08em] text-muted-foreground uppercase">
        {caption}
      </div>
    </div>
  );
}

export function TestButton({ source }: { source: TestSource }) {
  const { status, run } = useActionStatus("send-test");
  const pending = status.state === "pending";

  async function send() {
    await run(() => settingsInvoke("send_test_notification", { source }), {
      announce: true,
      okMessage: "Queued",
      errorMessage: (reason) => {
        // Errors surface inline; the console line helps a dev watching too.
        console.error("send_test_notification failed:", reason);
        return describeActionError(reason);
      },
    });
  }

  return (
    <div className="test-button-wrap flex flex-col items-end gap-0.5">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="text-fs-secondary"
        disabled={pending}
        onClick={() => void send()}
      >
        {pending ? "Sending…" : "Send test notification"}
      </Button>
      <ActionStatus status={status} className="test-button-status mt-0 text-right" />
    </div>
  );
}

export function TestButtonRow({
  name,
  help,
  source,
}: {
  name: string;
  help: string;
  source: TestSource;
}) {
  return (
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={name.replace(/\s+/g, "-")} name={name} help={help} />
      <TestButton source={source} />
    </div>
  );
}
