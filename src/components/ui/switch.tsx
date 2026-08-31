import { Switch as SwitchPrimitive } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

function Switch({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default";
}) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      data-size={size}
      className={cn(
        "peer group/switch relative inline-flex shrink-0 items-center rounded-full border border-transparent p-0 transition-[color,background-color,border-color,box-shadow,scale] duration-150 ease-out outline-none after:absolute after:-inset-x-1 after:-inset-y-1.5 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-[size=default]:h-[22px] data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50 active:scale-[0.97] active:shadow-[var(--shadow-pressed)]",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "pointer-events-none block translate-x-px rounded-full bg-background ring-0 transition-transform duration-150 ease-out",
          "group-data-[size=default]/switch:size-[18px] group-data-[size=default]/switch:data-checked:translate-x-[15px]",
          "group-data-[size=sm]/switch:size-3 group-data-[size=sm]/switch:data-checked:translate-x-[13px]",
          "dark:data-checked:bg-background dark:data-unchecked:bg-muted-foreground",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
