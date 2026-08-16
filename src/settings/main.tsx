import React from "react";
import ReactDOM from "react-dom/client";
import { applyAnimationTiming } from "../applyAnimationTiming";
import { SettingsApp } from "./SettingsApp";
// Import order is fixed and load-bearing: base.css (Tailwind entry,
// no-preflight) first so its @layer theme/utilities are established; then
// overlay-card.css so its unlayered rules still win any specificity tie by
// source order over base.css's layered content. This window is down to
// these two real stylesheets.
import "./base.css";
import "../overlay-card.css";

// Must run before render so the Appearance gallery's `var(--swap-exit-ms,
// ...)` etc. resolve to real JS-sourced values, not CSS fallbacks, on first
// paint — an undefined custom property inside a `transition:` shorthand
// invalidates the whole shorthand. Same required call at the real overlay
// entry (main.tsx).
applyAnimationTiming();

// SAFETY: settings.html statically mounts `<div id="root">`, so getElementById returns it.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
