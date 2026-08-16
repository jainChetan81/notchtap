import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

// tailwind-merge's default config only recognizes Tailwind's own built-in
// scale names inside the "text-*" prefix (font sizes AND colors both start
// with "text-", so it disambiguates by matching known theme keys). It knows
// nothing about this app's settings-scoped `text-fs-caption/-secondary/
// -body/-title` utilities (bridged from base.css's `--fs-*` vars) — so by
// default it mis-files them as an ambiguous "text-*" token and silently
// DROPS one of two conflicting classes whenever a `text-fs-*` size utility
// and a `text-{color}` utility appear in the SAME cn() call. Teaching
// twMerge that `text-fs-*` belongs to the "font-size" class group (not
// "text-color") fixes this app-wide.
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
