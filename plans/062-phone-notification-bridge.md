# Plan 062: SPIKE — bridge phone notifications into notchtap

## Status

- **Priority**: P3
- **Effort**: M (spike doc) / L (build, platform-dependent)
- **Risk**: MED — the network-exposure question is a locked architectural
  boundary, not a style choice.
- **Depends on**: none
- **Category**: direction / security
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  question ("can we hook up phone notifications?") during a live UI
  walkthrough; answered inline in that conversation, filed here for the
  real design work if pursued.

## Why this matters

Every current input source (ESPN, RSS, cmux, `/notify`) is either a
poller this app initiates, or a local process posting to it. Nothing
captures notifications generated on a *different device*. This would be
a genuinely new capability, not an extension of an existing one.

## Current state — the two separable problems

1. **Capturing the phone's notifications.**
   - **Android**: feasible — `NotificationListenerService` is a public,
     documented API; a small companion app (or existing tools like
     Tasker/MacroDroid) can read notifications and forward them via a
     webhook.
   - **iOS**: no public API exists for reading another app's
     notifications. Apple's own Continuity notification mirroring
     (iPhone → Mac notifications, same iCloud account) is the closest
     built-in behavior, but it's a private, unhookable OS feature — this
     app cannot intercept or relay it.

2. **Getting the event into notchtap.** `src-tauri/src/http.rs:141-144`
   hardcodes the `/notify` listener to `127.0.0.1` — verified directly in
   code, with an explicit comment: *"this is the single place a bind
   happens, and it is hardcoded to 127.0.0.1 — no config field can widen
   it"* — and a dedicated test (`listener_binds_loopback_only`,
   `http.rs:415-418`) pins that. A phone on the same Wi-Fi network cannot
   reach this endpoint today, by design (`ARCHITECTURE.md` §7 is the
   cited rationale).

## Decision needed (operator)

1. **Platform scope**: Android-only (the realistic path), or is iOS
   in-scope despite the much harder/less clean capture story (e.g. via
   Shortcuts personal automations, which only cover specific triggerable
   events, not arbitrary notifications)?
2. **Network exposure**: is loosening the loopback-only bind acceptable
   at all, and if so, how — widen to LAN-only with some auth token, or
   keep it loopback and relay through a cloud intermediary (mirroring
   `docs/recipes/kuma-webhook.md`'s existing local-webhook pattern, which
   itself still targets `127.0.0.1` from another *local* process)?
3. **Trust boundary**: a phone-originated event is fundamentally less
   trusted input than the current sources (all either local or
   this-app-initiated polls) — does it need its own priority ceiling,
   rate limit, or content sanitization beyond what `/notify` already
   does?

## Recommendation

If pursued: Android companion app → local relay process on the same Mac
(not the phone reaching notchtap directly) → existing `/notify` endpoint,
unchanged. This keeps the loopback-only boundary intact (the relay runs
on the same machine, same as any other CLI caller) and avoids the harder
"widen the network boundary" decision entirely. iOS support would need a
separate, much more constrained design (or may not be feasible without
Apple platform capabilities this app doesn't have).

## Maintenance notes

- This is the one item in this batch that touches a security-relevant
  locked decision (`ARCHITECTURE.md` §7) rather than a UI/design choice —
  treat the network-exposure question with the same weight as any other
  boundary change in this codebase, not as a quick add.
