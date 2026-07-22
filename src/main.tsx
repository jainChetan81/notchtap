import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// plan 114: shared-ui's design tokens (--font-sans, --font-mono,
// --ease-notchtap, etc.) first — same discipline settings/base.css
// already follows — so overlay-card.css and styles.css can reference
// them below instead of hand-copying literal values.
import "@chetanjain/shared-ui/design/tokens.css";
// plan 111: shared card-shape stylesheet next, then this window's own
// residue (window-level reset + the `.card-root` scoping rule) — fixed
// order so overlay-only declarations load after and win any specificity
// tie by source order, same as before the mirror-file split.
import "./overlay-card.css";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
