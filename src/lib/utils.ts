import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

// plan 112 Step 4: tailwind-merge's default config only recognizes
// Tailwind's own built-in scale names inside the "text-*" prefix (font
// sizes AND colors both start with "text-", so it disambiguates by
// matching known theme keys). It has no way to know about this app's
// settings-scoped `text-fs-caption/-secondary/-body/-title` utilities
// (bridged from base.css's own `--fs-*` vars, Plan 109's type floor) —
// so by default it mis-files them as an unrecognized/ambiguous "text-*"
// token and, worse, silently DROPS one of the two conflicting classes
// whenever a `text-fs-*` size utility and a `text-{color}` utility
// appear in the SAME cn() call (e.g. ActionStatus's
// `cn("text-fs-secondary ...", stateClasses)` — caught by the Step 4
// preview-equivalence harness while restyling SettingsGroup, but the
// same silent drop already affected every already-merged Step 3
// ActionStatus/heading call site that combines a `text-fs-*` size with
// a color utility). Teaching twMerge that `text-fs-*` belongs to the
// "font-size" class group (not "text-color") fixes this app-wide,
// rather than working around it utility-call-by-utility-call.
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [{ text: ["fs-caption", "fs-secondary", "fs-body", "fs-title"] }],
    },
  },
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
