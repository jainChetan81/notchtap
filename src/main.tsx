import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyAnimationTiming } from "./applyAnimationTiming";
// Import order is fixed and load-bearing: shared-ui design tokens first
// (same discipline as settings/base.css) so overlay-card.css and styles.css
// reference them below instead of hand-copying literals; then the shared
// card-shape stylesheet, then this window's own residue (window-level reset
// + `.card-root`) last so overlay-only declarations win any specificity tie
// by source order.
import "@chetanjain/shared-ui/design/tokens.css";
import "./notchtap-tokens.css";
import "./overlay-card.css";
import "./styles.css";

// Must run before render so overlay-card.css's `var(--swap-exit-ms, ...)`
// etc. resolve to the real JS-sourced values on first paint, not the CSS
// fallback. Required at BOTH entry points (here and settings/main.tsx).
applyAnimationTiming();

// SAFETY: index.html statically mounts `<div id="root">`, so getElementById returns it.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
