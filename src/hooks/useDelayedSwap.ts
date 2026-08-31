import { useLayoutEffect, useRef, useState } from "react";

type SwapKey = string | number | symbol | null | undefined;

export type DelayedSwap<T> = { value: T; exiting: boolean };

export function useDelayedSwap<T>(value: T, key: SwapKey, exitDurationMs: number): DelayedSwap<T> {
  const [shown, setShown] = useState<{ key: SwapKey; value: T }>({ key, value });
  const [exiting, setExiting] = useState(false);

  const lastLiveValueRef = useRef(value);
  if (key === shown.key) {
    lastLiveValueRef.current = value;
  }

  const incomingRef = useRef<{ key: SwapKey; value: T }>({ key, value });
  incomingRef.current = { key, value };

  useLayoutEffect(() => {
    if (key === shown.key) {
      return;
    }
    setExiting(true);
    const id = window.setTimeout(() => {
      setShown(incomingRef.current);
      setExiting(false);
    }, exitDurationMs);
    return () => window.clearTimeout(id);
  }, [key, shown.key, exitDurationMs]);

  const liveValue = key === shown.key ? value : lastLiveValueRef.current;
  return { value: liveValue, exiting };
}
