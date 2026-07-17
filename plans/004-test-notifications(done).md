# Plan 004: Per-source test notifications in the settings window

> Implemented 2026-07-17 as part of the appearance + test-notification
> work block; merged into the same commit as Plan 002/005.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW
- **Depends on**: Plan 002 (Appearance section now provides a home for the
  manual test button)
- **Category**: feature
- **Status**: DONE

## Why this matters

The overlay supports four sources (`football`, `news`, `cmux`, `manual`).
Without a way to preview each render path, tuning appearance or verifying
a new Kuma-style webhook is guesswork. This plan adds one gated invoke
command and small test buttons inside each settings section.

## Current state

After implementation: `src-tauri/src/settings.rs` exposes
`send_test_notification(source)`; `src-tauri/build.rs` and
`src-tauri/capabilities/settings.json` include it; and
`src/settings/SettingsApp.tsx` has a `TestButtonRow` in General, Football,
News, and Cmux sections plus the Appearance section.

## Invoke contract

`invoke("send_test_notification", { source: "football" | "news" | "cmux" | "manual" })`

- Builds a canned per-source payload with a timestamped body.
- Uses the running config's per-source priority and TTL.
- Enqueues through the same engine path as `/notify`.
- Bypasses pause only when the visible slot is empty; otherwise queues
  normally behind the visible item.

## Done criteria

- [x] Command added to `build.rs` allowlist + `capabilities/settings.json`
- [x] Rust unit tests cover paused-but-empty promotion and visible-item
      queueing
- [x] Frontend tests cover enabled navigation and per-source button presence
- [x] All test suites green
