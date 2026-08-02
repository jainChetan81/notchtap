import { cva, type VariantProps } from "class-variance-authority";
import { Slot } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  // press feedback (operator complaint, 2026-08-02: "shallow, devoid of
  // animation") is deliberately layered across three simultaneous channels
  // rather than one bare nudge — position (translate-y-px, kept from
  // before), depth (scale-[0.96], stronger than the 0.97 every other
  // pressable primitive in the settings window uses, since the button is
  // the primary tactile surface), and an inset shadow that reads as
  // "pressed in" rather than "lifted and dropped." None of this variant
  // table carries a resting `shadow-*` (verified: no variant below sets
  // one), so there's no existing elevation to invert on press — instead
  // the inset shadow borrows its exact offsets/opacity from this file's
  // own shadow vocabulary (`--shadow-selected: 0 1px 2px rgba(0, 0, 0,
  // 0.4)` in settings/base.css, used by Segmented.tsx's selected-pill
  // shadow) with `inset` prepended, so a press reads as that same
  // shadow flipped concave rather than a new value invented from
  // scratch. `box-shadow` was already in the transition list below
  // (plan 126); `transform` covers both translate and scale since
  // Tailwind composes them through shared CSS vars.
  // CodeRabbit review (PR #11): Tailwind v4 emits `scale-*`/`translate-*`
  // utilities as the standalone CSS `scale`/`translate` properties, not
  // `transform` (confirmed against the built CSS: `{scale:.97}`, not a
  // `transform:` shorthand) — so listing `transform` here transitioned
  // neither, and the whole "deepen the press feedback" change was
  // snapping instantly instead of animating. Fixed to the real property
  // names.
  "group/button inline-flex shrink-0 items-center justify-center rounded-lg border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap transition-[color,background-color,border-color,box-shadow,translate,scale] outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:not-aria-[haspopup]:translate-y-px active:scale-[0.96] active:shadow-[var(--shadow-pressed)] disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        // plan 112 Step 3: stock shadcn white-text-on-`--primary` fails
        // AA (computed ~3.3-3.65:1 vs the 4.5:1 floor Plan 109 set for
        // notchtap settings — `--primary` is the same #0a84ff accent
        // that pairing already failed against there). Upstream
        // `--primary-foreground` stays deliberately dark (5.56:1) —
        // it's meant for the undarkened `--primary` surface, where
        // light text would fail AA (white on raw #0a84ff is 3.26:1) —
        // so we don't touch that shared token. Instead this variant
        // darkens the background toward `--background` via `color-mix`
        // (same token-composition technique the `secondary` variant
        // below already uses for its hover state, not a new raw
        // palette literal) and pairs it with the local `text-foreground`
        // (light) so the app's one `default`-variant consumer (the
        // Save & Relaunch CTA) clears AA: rest ~5.4:1, hover ~4.7:1.
        default:
          "bg-[color-mix(in_oklch,var(--primary),var(--background)_25%)] text-foreground hover:bg-[color-mix(in_oklch,var(--primary),var(--background)_18%)]",
        outline:
          "border-border bg-background hover:bg-muted hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground dark:border-input dark:bg-input/30 dark:hover:bg-input/50",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] aria-expanded:bg-secondary aria-expanded:text-secondary-foreground",
        ghost:
          "hover:bg-muted hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground dark:hover:bg-muted/50",
        destructive:
          "bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/40 focus-visible:ring-destructive/20 dark:bg-destructive/20 dark:hover:bg-destructive/30 dark:focus-visible:ring-destructive/40",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default:
          "h-8 gap-1.5 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        xs: "h-6 gap-1 rounded-[min(var(--radius-md),10px)] px-2 text-xs in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-7 gap-1 rounded-[min(var(--radius-md),12px)] px-2.5 text-[0.8rem] in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: "h-9 gap-1.5 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        icon: "size-8",
        "icon-xs":
          "size-6 rounded-[min(var(--radius-md),10px)] in-data-[slot=button-group]:rounded-lg [&_svg:not([class*='size-'])]:size-3",
        "icon-sm":
          "size-7 rounded-[min(var(--radius-md),12px)] in-data-[slot=button-group]:rounded-lg",
        "icon-lg": "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot.Root : "button";

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
