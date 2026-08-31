import React from "react";
import ReactDOM from "react-dom/client";
import { applyAnimationTiming } from "../applyAnimationTiming";
import { SettingsApp } from "./SettingsApp";
// Import order is load-bearing: base establishes layers before unlayered card rules.
import "./base.css";
import "../overlay-card.css";

applyAnimationTiming();

// SAFETY: settings.html statically mounts `<div id="root">`, so getElementById returns it.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
