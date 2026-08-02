import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Tab } from "./components/IconStrip";
import { TAB_ORDER } from "./components/IconStrip";

// Plan 171 (tab-notch redesign, slice K — spec
// `docs/superpowers/specs/2026-08-02-tab-notch-design.md` section 10):
// the frontend half of the `tab-selection-changed` channel. Duplicates
// `useStatusState.ts`'s delivery discipline exactly — a strict
// validator, a listener, a dead-listener `console.error` — on a third,
// listen-only channel.
//
// **Rust owns selection, not this hook and not the DOM.** Spec section
// 10 is explicit: the overlay stays receive-only for commands, so a
// click on an icon is detected rust-side (the native click monitor
// adjacent to `hover.rs`'s own tracking area), rust decides which tab
// that click selected, and rust emits the transition here. There is no
// `invoke()` and no `#[tauri::command]` anywhere on this path — the
// frontend's job stays exactly what it is everywhere else in this app:
// render what rust says, never decide.
//
// Deliberately NO eval-planted boot seed (unlike `useStatusState`'s
// `window.__NOTCHTAP_STATUS_STATE__`): selection is only ever meaningful
// while the overlay is alive, and rust emits on transitions only, so
// there is no "value at page load" to seed — same reasoning App.tsx's
// own `hover-changed` listener documents for hover.

/// The wire payload. `selected: null` is a real, expected value (spec
/// section 7's "none" page — deselecting the current tab), never an
/// error.
export type TabSelectionPayload = { selected: Tab | null };

// Closed-set validation against the SAME `TAB_ORDER` the strip itself
// renders (IconStrip.tsx), not a second hand-typed literal union that
// could drift from it — adding a sixth tab there makes it valid here for
// free, and mistyping one makes it invalid on both sides at once.
const VALID_TABS: ReadonlySet<string> = new Set(TAB_ORDER);

export function isValidTabSelection(v: unknown): v is TabSelectionPayload {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  if (obj.selected === null) {
    return true;
  }
  return typeof obj.selected === "string" && VALID_TABS.has(obj.selected);
}

export function useTabSelection(): Tab | null {
  const [selected, setSelected] = useState<Tab | null>(null);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("tab-selection-changed", ({ payload }) =>
      // A malformed payload falls back to "nothing selected" whole — the
      // same all-off posture `useStatusState`'s FALLBACK_STATUS takes,
      // and the one state that can never render wrong data (spec section
      // 7's "none" page shows no below-block at all).
      setSelected(isValidTabSelection(payload) ? payload.selected : null),
    )
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        // A dead listener means a permanently stuck selection — make it
        // loud in the webview console since the overlay can't write to
        // the file log.
        console.error("tab-selection-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return selected;
}
