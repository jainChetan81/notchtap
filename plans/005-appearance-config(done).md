# Plan 005: Appearance config with hot-apply

> Implemented 2026-07-17 as part of the appearance + test-notification
> work block; merged into the same commit as Plan 002/004.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW
- **Depends on**: Plan 002 (Appearance section/preview scaffold)
- **Category**: feature
- **Status**: DONE

## Why this matters

The Kuma-style ask: a settings panel where the user can tune shape/size of
the popover card. This is implemented as a small set of preset controls
(scale, radius, opacity) that write to `[appearance]` in `config.toml` and
hot-apply CSS variables to the overlay window without a relaunch.

## Current state

After implementation:
- `src-tauri/src/config.rs` has `[appearance]` with `card_scale`,
  `card_radius`, `card_opacity`.
- `src-tauri/src/settings.rs` exposes `set_appearance(scale, radius,
  opacity)` which validates, writes the config, updates the managed
  running state, and emits `appearance-changed` to the overlay.
- `src/settings/SettingsApp.tsx` renders segmented controls in the
  Appearance section; the static preview cards reflect the chosen values.
- `src/App.tsx` listens for `appearance-changed` and sets
  `--card-scale`, `--card-radius`, `--card-opacity` on the root element.

## Invoke contract

`invoke("set_appearance", { scale: 0.8-1.4, radius: 0-24, opacity: 0.5-1.0 })`

Pure preset values today; the config schema stores floats so continuous
sliders can be added later as a frontend-only change.

## Done criteria

- [x] `[appearance]` config table + validation tests
- [x] `set_appearance` command added to allowlist/capabilities
- [x] Overlay receives current appearance on load + live updates
- [x] Static preview cards in Appearance section reflect controls
- [x] All test suites green
