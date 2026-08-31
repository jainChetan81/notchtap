import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Tab } from "../components/IconStrip";
import { TAB_ORDER } from "../components/IconStrip";

type UnparsedValue = string | number | boolean | null | UnparsedObject | UnparsedValue[];
type UnparsedObject = { [key: string]: UnparsedValue };

export type TabSelectionPayload = { selected: Tab | null };

const VALID_TABS: ReadonlySet<string> = new Set(TAB_ORDER);

export function isValidTabSelection(v: unknown): v is TabSelectionPayload {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const obj = v as UnparsedObject;
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
        console.error("tab-selection-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return selected;
}
