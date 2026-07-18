#!/bin/sh
# notchtap-claude-hook.sh — Claude Code hook -> notchtap (plan 035).
#
# OBSERVATIONAL ONLY. Reads a Claude Code hook payload as JSON on stdin,
# posts a heads-up notification via the `notchtap` CLI (backgrounded), and
# exits 0 WITHOUT writing anything to stdout. Staying silent means "no
# decision" (Claude Code hooks docs), so the real permission prompt still
# shows — this hook never answers Claude Code, never returns a decision or a
# modified payload (that respond-back loop stays out of scope, ARCHITECTURE
# §7). If jq or notchtap is missing it does nothing and still exits 0, so it
# can never block the session.
#
# Wired events (operator maps these in ~/.claude/settings.json):
#   Notification       -> body = .message                      (priority high)
#   PermissionRequest  -> "Permission requested: <tool>" + details Tool/
#                         Command-or-File/Project              (priority high)
#   Stop               -> "Session complete"                  (priority medium)
#   PostToolUse (Task) -> "Agent finished"                    (priority medium)
#
# PermissionRequest is a real, documented Claude Code hook event (it carries
# the common fields plus tool_name/tool_input, same as PreToolUse). `Command`
# comes from tool_input.command (Bash), `File` from tool_input.file_path
# (Edit/Write), else a compact tool_input; `Project` is cwd.

set -u

input=$(cat)

# jq parses the payload and notchtap delivers it — without either, do nothing.
command -v jq >/dev/null 2>&1 || exit 0
command -v notchtap >/dev/null 2>&1 || exit 0

event=$(printf '%s' "$input" | jq -r '.hook_event_name // ""')

title="Claude Code"
subtitle=""
priority=""
body=""
# up to three detail pairs; empty value => the pair is skipped when building
detail_tool=""
detail2_label=""
detail2_value=""
detail_project=""

case "$event" in
  Notification)
    subtitle="Notification"
    priority="high"
    body=$(printf '%s' "$input" | jq -r '.message // "Needs your attention"')
    ;;
  PermissionRequest)
    subtitle="Permission request"
    priority="high"
    detail_tool=$(printf '%s' "$input" | jq -r '.tool_name // ""')
    [ -n "$detail_tool" ] || detail_tool="a tool"
    body="Permission requested: $detail_tool"
    # Command (Bash) wins over File (Edit/Write); otherwise a compact
    # tool_input, truncated to 200 chars as a courtesy (the server caps too).
    cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""')
    file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""')
    if [ -n "$cmd" ]; then
      detail2_label="Command"
      detail2_value="$cmd"
    elif [ -n "$file" ]; then
      detail2_label="File"
      detail2_value="$file"
    else
      detail2_label="Input"
      detail2_value=$(printf '%s' "$input" | jq -r '(.tool_input // {}) | tostring' | cut -c1-200)
    fi
    detail_project=$(printf '%s' "$input" | jq -r '.cwd // ""')
    ;;
  Stop)
    priority="medium"
    body="Session complete"
    ;;
  PostToolUse)
    # only the Task matcher is wired by the operator, so any PostToolUse we
    # are handed is an agent (sub-task) completion.
    priority="medium"
    body="Agent finished"
    ;;
  *)
    # unmapped event — nothing to show, succeed silently
    exit 0
    ;;
esac

[ -n "$body" ] || exit 0

# Build the argument vector incrementally so values with spaces stay single
# args. A detail arg is "Label=Value"; the CLI splits on the FIRST '=', so a
# value that itself contains '=' (e.g. FOO=bar make) survives intact.
set -- --title "$title" --body "$body"
[ -n "$subtitle" ] && set -- "$@" --subtitle "$subtitle"
[ -n "$priority" ] && set -- "$@" --priority "$priority"
[ -n "$detail_tool" ] && set -- "$@" --detail "Tool=$detail_tool"
[ -n "$detail2_value" ] && set -- "$@" --detail "$detail2_label=$detail2_value"
[ -n "$detail_project" ] && set -- "$@" --detail "Project=$detail_project"

# Fire and forget: backgrounded so the hook returns immediately, output
# discarded, exit 0 unconditionally. Never blocks, never answers.
notchtap "$@" >/dev/null 2>&1 &
exit 0
