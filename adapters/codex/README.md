# Codex adapter (v7 ticket 7 of 13)

Like the Claude Code adapter, this is not a plugin — it's the shared
`notchtap-agent` binary (`src-tauri/src/bin/notchtap_agent.rs`, built and
installed with the app) invoked as a Codex **command hook**. See
`docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §4.1/§4.3 and
`src-tauri/src/agents/providers/codex.rs`'s module doc for the full
delivery/parsing contract and the one known doc-vs-spec gap (Codex's
documented hook surface has no structural failure signal, so this
adapter never emits a Codex `failed` event).

This file is the copyable setup snippet plan 143 (Settings' Agents
section) surfaces to the user, plus the exact target file to add it to.

## target file

`~/.codex/hooks.json` (user-level) — Codex also reads a `<repo>/.codex/hooks.json`
per-project and a TOML equivalent (`~/.codex/config.toml` /
`<repo>/.codex/config.toml`) if you'd rather keep hooks alongside your
other Codex config; the JSON form is shown below since it merges most
predictably with an existing file. notchtap never edits this file itself
(spec §4.6: "v7 does not silently edit a user's global provider
configuration") — copy the snippet below in yourself.

## snippet

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "PermissionRequest": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "SubagentStart": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "SubagentStop": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "PreToolUse": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ],
    "PostToolUse": [
      { "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }
    ]
  }
}
```

Every event uses the exact same command — `notchtap-agent hook codex`
reads the native JSON payload from stdin and reads the actual event name
back out of that payload itself, so no event needs its own distinct
command line or matcher (an unmatched/empty `matcher` runs the hook for
every invocation of that event, which is what's wanted here).

If `hooks.json` already has a `hooks` block (e.g. from another tool),
merge these eight keys into it rather than replacing the file — each key
is independent and order doesn't matter. If a key already exists for one
of these events, append a new entry to that key's array instead of
overwriting it.

Codex's documented lifecycle surface also has `PreCompact`/`PostCompact`/
`UserPromptSubmit` hooks — deliberately omitted above, since none of the
three carries permission/completion/failure/tool meaning the Agent Board
renders (see `codex.rs`'s own doc for why).

## verifying it worked

```sh
notchtap-agent test codex
```

posts one synthetic event so you can see it land on the Agent Board
without needing a real Codex session. `notchtap-agent status` reports
whether anything is listening on notchtap's loopback port.

## what this does *not* do

- it never answers a permission prompt, never blocks your session, and
  never mutates the native hook event (spec §4.1: fail-open, ≤750ms,
  "no decision JSON... or native-event mutation");
- it never forwards prompts, model output, raw tool input/output,
  environment values, secrets, or full command lines — see
  `src-tauri/src/agents/providers/codex.rs`'s module doc for the exact
  sanitization rules;
- a failure to reach notchtap (app not running, wrong port) is silent to
  Codex — check `~/Library/Logs/notchtap/notchtap-agent.log` for a
  bounded diagnostic if events aren't showing up.

## known capability gap

Codex has no documented terminal-failure or explicit-input-required hook
event today — this adapter declares only `session_lifecycle`,
`permission_requests`, `completion`, `tool_details`, and `subagents`
(never `failure`/`input_required`). Settings' Agents section reports
Codex as `partial` for exactly this reason, not because anything here is
broken.

## port override

notchtap's loopback listener defaults to port 9789. If you've changed it
(`config.toml`'s `port`), set `NOTCHTAP_PORT` in the environment Codex's
hook commands run in, matching the `notchtap` CLI's own `NOTCHTAP_PORT`
convention.
