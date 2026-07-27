# 138 — `notchtap-agent` helper + Claude Code adapter

> v7 ticket 6 of 13 (spec §4.1, §4.2). Filed 2026-07-26.

**What to build:** the shared hook-delivery binary plus the first real
adapter. After this ticket, a real Claude Code session on this machine
drives the Agent Board and produces permission/completion cards.

- New Rust binary target `notchtap-agent` installed with the app:
  `hook claude-code|codex|kimi`, `test <runtime>`, `status`.
- `hook` reads ONE native JSON payload from stdin, normalizes via a
  pure per-provider parser, posts schema v1 to the loopback port
  (`NOTCHTAP_PORT` override honored), exits.
- Delivery rules per §4.1: ≤750 ms connect/read timeout; fail open
  (provider sessions never blocked); exit 0 on delivery failure after
  a bounded diagnostic to notchtap's adapter log (never stdout); no
  decision JSON or native-event mutation; no daemon, shell
  interpolation, or `jq` dependency.
- Claude Code parser (§4.2) for `SessionStart`, `SessionEnd`,
  `PermissionRequest`, `Notification`, `Stop`, `StopFailure`,
  `PostToolUse`, `PostToolUseFailure`, `SubagentStart`,
  `SubagentStop`, with committed REDACTED fixtures for every event.
  `Notification` maps to a waiting state only for documented
  permission/idle-input notifications; generic ones are
  `Informational`; wording is never parsed to infer state.
- Capability declaration matching the §1 matrix row (InputRequired is
  notification-derived). Do not ship the adapter unless declaration
  and fixture suite agree (§14).
- Sanitization per §3.2: safe tool name, basename, short human
  summary; never secrets, prompts, raw tool I/O, env values, or full
  command lines.
- Codex/Kimi subcommands may exist as stubs that print "not yet
  supported" — their parsers are tickets 139/140.

**Blocked by:** 134 (endpoint must exist to post to).

**Status:** ready-for-agent

- [ ] Fixture test per §4.2 hook event proving normalized output
      (kind, state, capabilities, sanitized fields).
- [ ] Sanitization tests: a fixture with a fake secret/prompt/full
      command line never emits it.
- [ ] Fail-open test: unreachable port → exit 0, diagnostic in adapter
      log, nothing on stdout.
- [ ] `notchtap-agent test claude-code` posts a visible test event.
- [ ] Manual: hooks installed per the setup snippet in a real Claude
      Code session → board + cards update live.
- [ ] `cargo test` green; `just check-cli`-style lint for any script
      glue.
