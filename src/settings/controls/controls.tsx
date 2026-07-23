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

// plan 112 Step 4 (General): shared row shell for every control kind
// (toggle, number, priority/units fieldset, the diagnostics/history
// footer-style rows). Old settings.css used an adjacent-sibling selector
// (".control-row + .control-row") so only a row PRECEDED BY another row
// got a top divider; `first-child:border-t-0` reproduces the same
// visible result (every row but the first in its group gets the
// divider) without depending on sibling order in the stylesheet. This
// single className is reused at every ".control-row" call site across
// the settings window (control-row is a shared layout idiom, not a
// per-section one) — migrated once, here, rather than duplicated at each
// of the ten sections that render one.
export const CONTROL_ROW =
  "control-row grid min-h-[58px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-border/60 py-2.5 first:border-t-0";

// plan 112 Step 4 (General): shadcn Card replaces the old
// .settings-group/.group-heading/.group-controls box. gap-0/py-0/ring-0
// strip Card's own spacing/ring defaults (they'd otherwise double up
// with the explicit padding below); the border-bottom divider between
// heading and controls is the only piece Card's own subcomponents don't
// give for free, so it's added directly on CardHeader.
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
      // Card's own default className carries `text-sm` (14px/20px
      // line-height) — harmless for the title/description text below
      // (both set their own explicit text-fs-* size), but it's an
      // INHERITED property, so left alone it silently reaches every
      // descendant that doesn't set its own font-size, including the
      // Appearance section's Plan 111 preview subtree nested inside this
      // same Card (`.appearance-preview`/`.preview-stage`/`.card-root`
      // never declared their own font-size — they relied on inheriting
      // the browser's 16px/normal default, same as before this
      // migration). `text-base leading-[normal]` restores exactly that
      // inherited baseline so the preview subtree's computed styles stay
      // byte-identical (caught by the settings_capture.js preview-
      // equivalence harness before this fix landed; plan 115 renamed
      // this from the equivalent `text-[16px]` arbitrary onto the
      // `text-base` scale utility — 16px either way, pixel-identical).
      className="gap-0 overflow-hidden rounded-md border border-border bg-card py-0 text-base leading-[normal] ring-0"
    >
      <CardHeader
        // CardHeader's own default className carries a self-triggering
        // `[.border-b]:pb-(--card-spacing)` rule keyed on the literal
        // presence of the "border-b" token — adding it for the divider
        // below silently re-widens padding-bottom to Card's own 16px
        // spacing unit, fighting the `pb-[11px]` needed to match the old
        // `.group-heading { padding: 12px 13px 11px }`. The trailing `!`
        // forces this pb to win regardless of that rule's higher
        // selector specificity (caught by the settings_capture.js
        // preview-equivalence harness — it grew "Card shape"'s box by
        // enough to shift the Appearance preview gallery below it).
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
      {/* id lets a sibling <fieldset role=group> (the Segmented control's
          labelled form) point aria-labelledby back at this same visible
          text — <label for> alone doesn't associate with a fieldset,
          since fieldset isn't a "labelable" HTML element. */}
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
  // Local raw-string mirror of `value` (2026-07-23 review): a plain
  // `value={value} onChange={(e) => onChange(Number(e.target.value))}`
  // pair fights the user on two fronts — `Number("")` coerces a
  // cleared field straight to `0`, and a controlled numeric `value`
  // snaps an in-progress decimal like `"12."` back to `"12"` on every
  // keystroke because `String(12) !== "12."`. Keeping the input's own
  // in-progress text in state (and only reconciling it with the
  // external `value` when that value actually changes, e.g. Reset)
  // lets the user clear-and-retype or type a trailing `.`/leading `-`
  // without the control fighting back.
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
            // Empty (clearing to retype) or a bare sign/decimal point
            // mid-entry: don't coerce to 0 and don't propagate yet —
            // leave the last-committed config value alone until the
            // input reads as a real number.
            if (next === "" || next === "-" || next === "." || next === "-.") {
              return;
            }
            const parsed = Number(next);
            if (!Number.isNaN(parsed)) {
              onChange(parsed);
            }
          }}
          onBlur={() => {
            // Leaving the field on an invalid/empty in-progress value
            // (e.g. the user cleared it and clicked away) restores the
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

// plan 112 Step 4 (General): the bespoke checkbox+track Switch is gone —
// role="switch"/aria-checked shadcn Switch (radix-ui's real <button
// type="button" role="switch">) plus a visually-hidden shadcn Label
// (ControlCopy already renders the visible name for this row) replace
// it. This is the plan's one authorized behavioral change: `checked` on
// an HTMLInputElement becomes `aria-checked` on a native button —
// `screen.getByLabelText` still resolves it via the label[for] ->
// button-id association (button is a labelable element), so accessible
// name is unchanged; only the test assertions that read `.checked`
// needed updating (see SettingsApp.test.tsx).
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
    // plan 112 Step 4 (Football): the shared control-row divider rhythm
    // landed in General's commit already — this section's own turn is
    // the textarea + caption styling below (Football is the first
    // section that actually renders one; News reuses the same
    // component unchanged).
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
        // Errors are surfaced inline now, but the console line costs
        // nothing and helps a dev watching the console too.
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
