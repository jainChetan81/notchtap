import React from "react";
import ReactDOM from "react-dom/client";
import { SettingsApp } from "./SettingsApp";
// plan 112: Tailwind v4 entry (no-preflight, shared-ui tokens, shadcn
// utilities) loads first so its @layer theme/utilities are established
// before any plain CSS. plan 111: shared card-shape stylesheet next,
// then this window's own residue (settings chrome + the Appearance
// preview's frame/override rules) — fixed order so settings-only
// declarations load after and win any specificity tie by source order.
import "./base.css";
import "../overlay-card.css";
import "./settings.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
