import { useEffect, useRef } from "react";
import { prefersReducedMotion } from "../prefersReducedMotion";

// plan 081: the thin rotation-countdown bar every showing card carries.
// Deliberately NOT React state per frame — the prototype
// (prototype/notch-states.html §4's ttlTick) mutates the fill node's
// style.width directly via rAF, and this component follows that: a
// re-render per animation frame would be needless work for a value React
// never needs to read back.
//
// Anchoring: `remainingMs` is a snapshot taken server-side at emission
// time (queue.rs's current_slot_state) — by the time this component
// receives it, some of that time has already elapsed in flight. Anchoring
// `deadline = performance.now() + remainingMs` on receipt (mount, or
// whenever remainingMs/ttlMs/slotId changes) and counting down locally
// from there is the honest way to track it without needing wall-clock
// sync between rust and the webview.
export function TtlBar({
  slotId,
  ttlMs,
  remainingMs,
  hoverPaused = false,
}: {
  slotId: string;
  ttlMs: number;
  remainingMs: number;
  // plan 093: 081's deferred half. Freezes the countdown for as long as
  // the caller reports the card hovered — driven by `App.tsx`'s
  // `hover-changed`-sourced `hovered` prop (never CSS `:hover`, per the
  // hover primitive's own rule), NOT by re-anchoring off a fresh
  // `remainingMs`: the rust side never re-emits `slot-state` purely for a
  // hover transition (`SlotState::dedup_eq` excludes `remaining_ms`,
  // CLAUDE.md's own rule, so a hover-only mutation is never wire-visible
  // here) — this prop is the ONLY signal this component gets. The actual
  // rotation-deadline hold lives rust-side (`queue.rs`'s
  // `hover_started_at`/`hover_paused_total`); this is purely the LOCAL
  // visual mirror of that hold, decoupled from it but kept honest because
  // both sides pause/resume off the same tracking-area transition.
  hoverPaused?: boolean;
}) {
  const fillRef = useRef<HTMLDivElement | null>(null);
  // Read inside the rAF loop below without re-running the anchoring
  // effect on every hoverPaused flip (see that effect's own dependency
  // array) — a ref, not a second effect, so pause/resume never resets
  // `deadline`.
  const hoverPausedRef = useRef(hoverPaused);
  // 2026-07-23 review fix (Performance finding — rAF loop kept running
  // while hover-paused): `frameIdRef` is the shared handshake between the
  // anchoring effect below and the pause-edge effect right after it.
  // `null` means "no frame currently in flight" — either genuinely
  // bailed out on a hover pause, or expired, or not yet armed. `resumeRef`
  // holds a closure (set up fresh by the anchoring effect, on every
  // slotId/ttlMs/remainingMs re-anchor) that re-arms exactly one frame;
  // the pause-edge effect calls it on the paused->unpaused transition,
  // and only when frameIdRef is actually null, so it never double-
  // schedules a frame that's already in flight.
  const frameIdRef = useRef<number | null>(null);
  const resumeRef = useRef<(() => void) | null>(null);

  // Bail/resume plumbing: while hoverPaused is true, the anchoring
  // effect's tick() loop paints the frozen value once and then stops
  // requesting new frames entirely (see that effect below) — no more
  // per-frame work for as long as the pause lasts, not just a frozen
  // number. This effect's only job is noticing the FALLING edge
  // (paused -> not paused) and re-arming exactly one frame to resume,
  // via the resumeRef closure the anchoring effect maintains.
  useEffect(() => {
    const wasPaused = hoverPausedRef.current;
    hoverPausedRef.current = hoverPaused;
    if (wasPaused && !hoverPaused && frameIdRef.current === null) {
      resumeRef.current?.();
    }
  }, [hoverPaused]);

  // Re-anchors on mount and on every slotId/ttlMs/remainingMs change: a
  // new promotion (slotId changes) or a same-id re-emit with a fresh
  // remainingMs (supersede top-up, manual expand) both need the countdown
  // to restart from the new numbers, not keep counting from the old ones.
  // Deliberately NOT keyed on hoverPaused — see the ref above; a hover
  // toggle must pause/resume in place, never restart the countdown.
  // biome-ignore lint/correctness/useExhaustiveDependencies: slotId isn't read in the body, but it's the deliberate re-anchor trigger documented above (same pattern as StatusRailCard's currentId) — a new promotion must restart the countdown even when ttlMs/remainingMs happen to coincide. hoverPaused is excluded on purpose too (see the comment above the effect).
  useEffect(() => {
    const fill = fillRef.current;
    if (!fill) {
      return;
    }

    // Idle-CPU discipline (plans 015/018): under prefers-reduced-motion, a
    // CSS rule alone can't stop a rAF loop, so the loop itself is gated in
    // JS — render a static, un-scaled fill and never arm the loop.
    const reducedMotion = prefersReducedMotion();

    if (reducedMotion || ttlMs <= 0) {
      fill.style.transform = "scaleX(1)";
      resumeRef.current = null;
      return;
    }

    const deadline = performance.now() + remainingMs;
    // plan 093: total time already spent paused (banked once a pause
    // ends) plus, while a pause is currently open, the in-flight duration
    // since it started — the same freeze-via-subtraction technique
    // queue.rs's `hover_frozen_rotation_elapsed` uses rust-side, kept
    // independent here since this component never round-trips a fresh
    // remainingMs to resync against (see the `hoverPaused` prop doc).
    let pausedAccumMs = 0;
    let pauseStartedAt: number | null = hoverPausedRef.current ? performance.now() : null;
    let cancelled = false;

    // 2026-07-23 review fix (Performance finding): `.ttl-fill` sits under
    // `.card-assembly`'s `filter: drop-shadow` — mutating a layout
    // property (`width`) every frame forced a re-layout/re-rasterize of
    // that whole filtered group. Animating `transform: scaleX(fraction)`
    // instead (CSS: `transform-origin: left`, full-width base) is
    // visually identical at this bar's 2px height and stays
    // compositor-only.
    //
    // While paused, this still paints the frozen value on the FIRST tick
    // after the pause begins (so the bar visibly holds at the right
    // position, not whatever it happened to be mid-frame), then bails —
    // no `requestAnimationFrame` call, so no more per-frame work at all
    // until the pause-edge effect above calls `resumeRef.current()`.
    // Also stops permanently once `remaining` reaches 0 (expired), same
    // idle-CPU discipline as the reduced-motion early return above.
    function tick() {
      if (cancelled) {
        return;
      }
      const fillEl = fillRef.current;
      if (!fillEl) {
        return;
      }
      const nowPaused = hoverPausedRef.current;
      if (nowPaused && pauseStartedAt === null) {
        pauseStartedAt = performance.now();
      } else if (!nowPaused && pauseStartedAt !== null) {
        pausedAccumMs += performance.now() - pauseStartedAt;
        pauseStartedAt = null;
      }
      const effectiveNow = pauseStartedAt ?? performance.now();
      const remaining = Math.max(0, deadline + pausedAccumMs - effectiveNow);
      const pct = Math.min(100, (remaining / ttlMs) * 100);
      fillEl.style.transform = `scaleX(${pct / 100})`;
      if (remaining <= 0) {
        frameIdRef.current = null; // stop permanently once expired
        return;
      }
      if (nowPaused) {
        frameIdRef.current = null; // bail — resumeRef re-arms on unpause
        return;
      }
      frameIdRef.current = requestAnimationFrame(tick);
    }

    resumeRef.current = () => {
      if (cancelled || frameIdRef.current !== null) {
        return;
      }
      frameIdRef.current = requestAnimationFrame(tick);
    };

    frameIdRef.current = requestAnimationFrame(tick);

    return () => {
      cancelled = true;
      resumeRef.current = null;
      if (frameIdRef.current !== null) {
        cancelAnimationFrame(frameIdRef.current);
        frameIdRef.current = null;
      }
    };
  }, [slotId, ttlMs, remainingMs]);

  return (
    <div className="ttl-bar">
      <div className="ttl-fill" ref={fillRef} />
    </div>
  );
}
