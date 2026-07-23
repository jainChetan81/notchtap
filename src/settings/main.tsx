import React from "react";
import ReactDOM from "react-dom/client";
import { applyAnimationTiming } from "../applyAnimationTiming";
import { SettingsApp } from "./SettingsApp";
// plan 112: Tailwind v4 entry (no-preflight, shared-ui tokens, shadcn
// utilities) loads first so its @layer theme/utilities are established
// before any plain CSS. plan 111: shared card-shape stylesheet next —
// fixed order so overlay-card.css's unlayered rules still win any
// specificity tie by source order over base.css's layered content.
// Step 5: the old per-window settings stylesheet is gone — every rule
// it still carried (the settings-scoped base reset, the Appearance
// preview frame chrome, the shared mono-font list, `.section-stack`)
// is now inside base.css itself, so this window is down to its two
// real stylesheets.
import "./base.css";
import "../overlay-card.css";

// 2026-07-23 review fix (wave C, CSS custom-property injection): the
// Appearance gallery renders this same overlay-card.css — skipping this
// call here would leave its `var(--swap-exit-ms, ...)` etc. resolving
// only to CSS fallbacks, silently diverging from the real overlay. See
// applyAnimationTiming.ts's own doc: an undefined custom property inside
// a `transition:` shorthand invalidates the WHOLE shorthand, which is
// exactly the bug class this call prevents here.
applyAnimationTiming();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
