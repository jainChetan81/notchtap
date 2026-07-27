# 141 — OpenCode TypeScript plugin

> v7 ticket 9 of 13 (spec §4.5). Filed 2026-07-26.

**What to build:** the OpenCode adapter as a TS plugin
(`adapters/opencode/notchtap.ts`), since OpenCode's lifecycle surface
is a plugin event bus, not command hooks.

- Listen for `permission.asked`, `permission.replied`,
  `session.created`, `session.updated`, `session.status`,
  `session.idle`, `session.error`, `session.deleted`,
  `tool.execute.before`, `tool.execute.after`; post the same schema v1
  to the loopback endpoint.
- Network behavior, caps, sanitization, and fail-open semantics match
  the Rust helper exactly (≤750 ms, never blocks the session, no
  secrets/prompts/full commands, bounded diagnostics).
- Capability declaration per the §1 matrix row: explicit-input and
  completed are session/status-derived; subagents NOT declared until
  verified; session error → failed.
- Vitest coverage in the repo suite (plugin code is testable pure
  functions + a thin bus binding).

**Blocked by:** 134 (endpoint). Independent of the Rust helper — can
run in parallel with 138–140.

**Status:** ready-for-agent

- [ ] Fixture test per bus event → normalized schema v1 (or dropped).
- [ ] Sanitization + fail-open tests mirroring ticket 138's.
- [ ] No subagent capability declared.
- [ ] Manual: real OpenCode session smoke via the documented plugin
      install path.
- [ ] `npx vitest run` + typecheck + lint green.
