import type * as React from "react";

import { cn } from "@/lib/utils";

// M16 consolidation: this repo had grown FOUR separate implementations
// of the same "small mono metadata chip" role — History's
// `history-meta-chip` / Queue's `queue-priority-tag`
// (rounded-full border-border px-[7px] py-0.5 font-[650]), About's
// tech-stack chip (rounded-full px-[9px] py-[3px], no weight), a shadcn
// `Badge` override in Connectors (rounded-[4px] px-[5px] py-[3px]), and
// Shortcuts' `shortcut-status` (rounded-[3px] px-1 py-0.5). One shape —
// radius/padding/weight/tracking — replaces all four call sites; the two
// genuinely semantic differences (a status chip reading as emphasized
// once "set"/"active", and free-form content wanting mixed case rather
// than an uppercase status word) stay as the `active`/`uppercase` props
// below rather than forking the shape again.
//
// Tone redesign: `active` alone only ever had one visual (a brighter-
// bordered neutral). Real status chips (adapter health, secret-saved)
// carry real meaning with more than one state, so `tone` gives them a
// real color — reusing the SAME five accent hues the overlay's priority/
// source-identity system already uses (`--overlay-*`, tokens.css), never
// a new palette invented for the settings window alone. `tone` and
// `active` are independent: `active` still governs the plain emphasized-
// neutral look for chips that only ever have one state worth
// highlighting (Shortcuts' wired/not, Queue's priority word); `tone`
// is for chips with a real positive/caution/critical status to report.
export type ChipTone = "neutral" | "positive" | "caution" | "critical" | "accent";

const TONE_CLASSES: Record<Exclude<ChipTone, "neutral">, string> = {
  // adapter available, secret saved, connector reachable
  positive: "border-overlay-green/45 bg-overlay-green/15 text-overlay-green",
  // adapter partial/stale, degraded but not down
  caution: "border-overlay-amber/45 bg-overlay-amber/15 text-overlay-amber",
  // adapter unavailable, last event errored
  critical: "border-overlay-coral/45 bg-overlay-coral/15 text-overlay-coral",
  // a non-status "this is set/on" emphasis (kept distinct from `active`'s
  // plain neutral so a caller can opt into color for a non-tri-state flag)
  accent: "border-overlay-teal/45 bg-overlay-teal/15 text-overlay-teal",
};

export function MetaChip({
  active = false,
  tone = "neutral",
  uppercase = false,
  dotColor,
  className,
  children,
  ...props
}: React.ComponentProps<"span"> & {
  /** Emphasized tone for a status chip that's "set"/"active" with only
   *  one state worth calling out — e.g. a shortcut that's actually
   *  wired up. Ignored when `tone` is anything but `"neutral"`. */
  active?: boolean;
  /** Real status color — reuses the overlay's own accent hues so a
   *  chip's meaning (good/degraded/down) reads at a glance instead of
   *  needing the label text read in full. Defaults to `"neutral"`
   *  (the plain bordered look, unaffected by `active`'s own styling). */
  tone?: ChipTone;
  /** Status-word chips (Connectors' secret status, Shortcuts' active/
   *  planned column) read as uppercase; free-form content chips
   *  (a history entry's source, a tech-stack name) don't. */
  uppercase?: boolean;
  /** Plan 147: leading colour swatch — a source/runtime/category
   *  identity dot pulled from `src/lib/sourceColors.ts`. Omitted by
   *  default (no swatch, no layout change). Independent of `tone`: a
   *  runtime-identity dot and a status tone answer different questions
   *  ("whose" vs. "how's it doing") and can appear together. */
  dotColor?: string;
}) {
  return (
    <span
      data-slot="meta-chip"
      data-tone={tone}
      className={cn(
        "meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground transition-colors duration-[150ms] ease-notchtap",
        uppercase && "tracking-[0.06em] uppercase",
        tone === "neutral" && active && "border-ring/40 bg-input/40 text-foreground",
        tone !== "neutral" && TONE_CLASSES[tone],
        className,
      )}
      {...props}
    >
      {dotColor ? (
        <span
          data-slot="meta-chip-dot"
          className="meta-chip-dot mr-[5px] inline-block size-[6px] rounded-full align-middle"
          style={{ background: dotColor }}
        />
      ) : null}
      {children}
    </span>
  );
}
