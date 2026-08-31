import type * as React from "react";

import { cn } from "@/lib/utils";

export type ChipTone = "neutral" | "positive" | "caution" | "critical" | "accent";

const TONE_CLASSES = {
  positive: "border-overlay-green/45 bg-overlay-green/15 text-overlay-green",
  caution: "border-overlay-amber/45 bg-overlay-amber/15 text-overlay-amber",
  critical: "border-overlay-coral/45 bg-overlay-coral/15 text-overlay-coral",
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
  active?: boolean;
  tone?: ChipTone;
  uppercase?: boolean;
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
