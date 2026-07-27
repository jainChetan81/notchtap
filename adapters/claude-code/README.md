# Claude Code adapter (v7 ticket 6 of 13)

notchtap's Claude Code adapter is not a plugin — it's the shared
`notchtap-agent` binary (`src-tauri/src/bin/notchtap_agent.rs`, built and
installed with the app) invoked as a Claude Code **command hook**. There
is nothing to install beyond pointing Claude Code's hook config at that
binary; see `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §4.1/§4.2 for
the full delivery/parsing contract this implements.

This file is the copyable setup snippet plan 143 (Settings' Agents
section) surfaces to the user, plus the exact target file to add it to.

## target file

`~/.claude/settings.json` (user-level) or a project's `.claude/settings.json`
— either works; Claude Code merges them. notchtap never edits this file
itself (spec §4.6: "v7 does not silently edit a user's global provider
configuration") — copy the snippet below in yourself.

## snippet

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "PermissionRequest": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "StopFailure": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "PostToolUse": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "PostToolUseFailure": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "SubagentStart": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ],
    "SubagentStop": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }
    ]
  }
}
```

Every event uses the exact same command — `notchtap-agent hook
claude-code` reads the native JSON payload from stdin and reads the
actual event name back out of that payload's own `hook_event_name`
field, so no event needs its own distinct command line or matcher.

If `settings.json` already has a `hooks` block (e.g. from another tool),
merge these ten keys into it rather than replacing the file — each key
is independent and order doesn't matter. If a key already exists for
one of these events, append a new entry to that key's array instead of
overwriting it; Claude Code runs every hook registered for an event.

## verifying it worked

```sh
notchtap-agent test claude-code
```

posts one synthetic event so you can see it land on the Agent Board
without needing a real Claude Code session. `notchtap-agent status`
reports whether anything is listening on notchtap's loopback port.

## what this does *not* do

- it never answers a permission prompt, never blocks your session, and
  never mutates the native hook event (spec §4.1: fail-open, ≤750ms,
  "no decision JSON... or native-event mutation");
- it never forwards prompts, model output, raw tool input/output,
  environment values, secrets, or full command lines — see
  `src-tauri/src/agents/providers/claude_code.rs`'s module doc for the
  exact sanitization rules;
- a failure to reach notchtap (app not running, wrong port) is silent
  to Claude Code — check `~/Library/Logs/notchtap/notchtap-agent.log`
  for a bounded diagnostic if events aren't showing up.

## port override

notchtap's loopback listener defaults to port 9789. If you've changed
it (`config.toml`'s `port`), set `NOTCHTAP_PORT` in the environment
Claude Code's hook commands run in, matching the `notchtap` CLI's own
`NOTCHTAP_PORT` convention.
