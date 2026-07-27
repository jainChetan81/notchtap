# 139 — Codex adapter

> v7 ticket 7 of 13 (spec §4.3). Filed 2026-07-26.

**What to build:** `notchtap-agent hook codex` becomes real. A real
Codex session drives the board and cards, with honestly declared
capability gaps.

- Pure Codex parser + redacted fixtures for the documented lifecycle
  events: `SessionStart`, `SessionEnd`, `PermissionRequest`, `Stop`,
  `SubagentStart`, `SubagentStop`, `PreToolUse`, `PostToolUse`, and
  failure variants available in the installed Codex version.
- The legacy top-level `notify` command is NOT used (single
  user-global slot, poor contract). Absence of explicit InputRequired
  and terminal-failure hooks remains a DECLARED capability gap per the
  §1 matrix (failed = "tool failure; terminal failure is partial") —
  never inferred from wording.
- Same sanitization, fail-open, and delivery rules as ticket 138.

**Blocked by:** 138 (shared helper).

**Status:** ready-for-agent

- [ ] Fixture test per supported native event; capability declaration
      and fixture suite agree.
- [ ] Declared-gap test: no InputRequired ever emitted from Codex
      input.
- [ ] Manual: real Codex session smoke — start, tool use, permission,
      stop all reflected.
- [ ] `cargo test` green.
