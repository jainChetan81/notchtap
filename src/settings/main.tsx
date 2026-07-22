import React from "react";
import ReactDOM from "react-dom/client";
import { SettingsApp } from "./SettingsApp";
// plan 111: shared card-shape stylesheet first, then this window's own
// residue (settings chrome + the Appearance preview's frame/override
// rules) — fixed order so settings-only declarations load after and win
// any specificity tie by source order, same discipline as main.tsx.
import "../overlay-card.css";
import "./settings.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
