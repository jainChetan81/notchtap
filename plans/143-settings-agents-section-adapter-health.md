# 143 — Settings Agents section + Adapter Health

> v7 ticket 11 of 13 (spec §4.6, §8, §10). Filed 2026-07-26.

**What to build:** the Agents section replacing the removed cmux tab,
plus per-runtime Adapter Health end to end. Settings needs to function;
overlay-grade visual polish not required here.

- New `agents/health.rs`: per-runtime snapshot (adapter version,
  available/partial/unavailable, declared capabilities, last accepted
  event time, last bounded error category, setup compatibility
  message) feeding the `adapterHealth` array in `agent-state` and the
  Settings section.
- Settings Agents section per §8: global enable, retention, stale
  threshold, informational toggle; event-kind Priority and Rotation
  controls; four adapter cards (enabled, health, last seen,
  capabilities, compatibility note, copyable setup snippet + exact
  target file, test event, uninstall instructions — §4.6: no silent
  editing of user provider config); preview fixtures for waiting
  permission, working-with-subagents, completed, failed, and multiple
  independent sessions; Agent in General's Rotation Order editor;
  Agent labels/filters in History and queue inspection; `⌃⇧A`
  cheat-sheet entry.
- Any new `#[tauri::command]` goes into `build.rs`,
  `capabilities/settings.json`, the settings-window label guard, and
  the command ACL tests TOGETHER (CLAUDE.md ipc rule). No new overlay
  command; `capabilities/default.json` unchanged.

**Blocked by:** 137 (config block + tab removal), 138 (helper + first
adapter for health/test-event reality), 141 (OpenCode snippet).
139/140 do NOT gate this — their cards may show
undetected/unavailable.

**Status:** ready-for-agent

- [ ] Adapter Health unit tests (state derivation, error categories,
      last-seen).
- [ ] Settings UI tests: section renders all controls, four cards,
      preview fixtures; test-event button round-trips.
- [ ] Command ACL tests updated in lockstep with any new command.
- [ ] Manual: setup snippet copied from Settings installs a working
      Claude Code hook.
- [ ] `cargo test` + `npx vitest run` green.
