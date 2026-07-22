import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// plan 111: shared card-shape stylesheet first, then this window's own
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
