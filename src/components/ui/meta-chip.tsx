import type * as React from "react";

import { cn } from "@/lib/utils";

// `tone` carries multi-state health meaning. `active` carries a plain
// emphasized state for controls with one state worth highlighting.
export type ChipTone = "neutral" | "positive" | "caution" | "critical" | "accent";

const TONE_CLASSES = {
  // adapter available
  positive: "border-overlay-green/45 bg-overlay-green/15 text-overlay-green",
  // adapter partial/stale, degraded but not down
  caution: "border-overlay-amber/45 bg-overlay-amber/15 text-overlay-amber",
  // adapter unavailable, last event errored
  critical: "border-overlay-coral/45 bg-overlay-coral/15 text-overlay-coral",
  // a non-status "this is set/on" emphasis (kept distinct from `active`'s
  // plain neutral so a caller can opt into color for a non-tri-state flag)
  accent: "border-overlay-teal/45 bg-overlay-teal/15 text-overlay-teal",
} satisfies Record<Exclude<ChipTone, "neutral">, string>;

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
  /** Status-word chips read as uppercase; free-form content chips
   *  (a history entry's source, a tech-stack name) don't. */
  uppercase?: boolean;
  /** Leading colour swatch — a source/runtime/category
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
