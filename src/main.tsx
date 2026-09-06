import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyAnimationTiming } from "./applyAnimationTiming";
// Import order is load-bearing: tokens, shared card CSS, then overlay overrides.
import "./notchtap-tokens.css";
import "./overlay-card.css";
import "./styles.css";

applyAnimationTiming();

// SAFETY: index.html statically mounts `<div id="root">`, so getElementById returns it.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
