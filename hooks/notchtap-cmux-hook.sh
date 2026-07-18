#!/bin/sh
# notchtap-cmux-hook.sh — cmux notification hook -> notchtap (plan 035).
#
# OBSERVATIONAL ONLY, pass-through. cmux applies whatever JSON a hook returns
# on stdout (it can filter banners, keep/skip history, run sounds, stop later
# hooks); if a hook returns invalid JSON cmux falls back to defaults and posts
# a hook-failure alert. So this script ECHOES the input JSON back UNCHANGED
# FIRST — cmux's own notification behaviour is untouched — then fires a
# heads-up to notchtap in the background and exits 0. It never modifies the
# notification or the effects.
#
# stdin JSON (cmux docs): { "notification": {title, subtitle, body, ...},
#                           "context": {cwd, ...}, "effects": {...} }

set -u

input=$(cat)

# Pass-through: hand cmux back exactly what it gave us (valid JSON in ->
# valid JSON out), so its banner / sidebar / sound behaviour is unchanged.
printf '%s\n' "$input"

# Best-effort relay to notchtap; needs jq to parse and notchtap to deliver.
command -v jq >/dev/null 2>&1 || exit 0
command -v notchtap >/dev/null 2>&1 || exit 0

title=$(printf '%s' "$input" | jq -r '.notification.title // ""')
body=$(printf '%s' "$input" | jq -r '.notification.body // ""')
subtitle=$(printf '%s' "$input" | jq -r '.notification.subtitle // ""')
project=$(printf '%s' "$input" | jq -r '.context.cwd // ""')

# The CLI requires a non-empty title and body; fall back so a sparse cmux
# payload still produces a valid post rather than a silent CLI usage error.
[ -n "$title" ] || title="cmux"
[ -n "$body" ] || body="Workspace notification"

set -- --title "$title" --body "$body" --source cmux --priority high
[ -n "$subtitle" ] && set -- "$@" --subtitle "$subtitle"
[ -n "$project" ] && set -- "$@" --detail "Project=$project"

notchtap "$@" >/dev/null 2>&1 &
exit 0
