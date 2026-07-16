import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type NotificationPayload = {
  id: string;
  title: string;
  body: string;
  ttlSecs: number;
};

export type VisibleNotification = NotificationPayload & {
  phase: "enter" | "hold" | "exit";
};

export const ENTER_DURATION_MS = 300;
export const EXIT_DURATION_MS = 300;

// This is a render-state list, not a queue (spec §8): rust already decided
// what is visible before emitting `notification-promoted`. The only job
// here is running each item's enter → hold → exit animation clock off the
// ttlSecs value it arrived with.
export function useVisibleNotifications(): VisibleNotification[] {
  const [items, setItems] = useState<VisibleNotification[]>([]);
  const timers = useRef<Map<string, number[]>>(new Map());

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;

    const setPhase = (id: string, phase: VisibleNotification["phase"]) => {
      setItems((current) =>
        current.map((item) => (item.id === id ? { ...item, phase } : item)),
      );
    };

    const remove = (id: string) => {
      setItems((current) => current.filter((item) => item.id !== id));
      timers.current.delete(id);
    };

    listen<NotificationPayload>("notification-promoted", ({ payload }) => {
      setItems((current) => [...current, { ...payload, phase: "enter" }]);

      const holdAt = window.setTimeout(() => {
        setPhase(payload.id, "hold");
      }, ENTER_DURATION_MS);
      const exitAt = window.setTimeout(() => {
        setPhase(payload.id, "exit");
      }, ENTER_DURATION_MS + payload.ttlSecs * 1000);
      const removeAt = window.setTimeout(() => {
        remove(payload.id);
      }, ENTER_DURATION_MS + payload.ttlSecs * 1000 + EXIT_DURATION_MS);

      timers.current.set(payload.id, [holdAt, exitAt, removeAt]);
    }).then((fn) => {
      if (unmounted) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    const pendingTimers = timers.current;
    return () => {
      unmounted = true;
      unlisten?.();
      for (const ids of pendingTimers.values()) {
        ids.forEach((t) => window.clearTimeout(t));
      }
      pendingTimers.clear();
    };
  }, []);

  return items;
}
