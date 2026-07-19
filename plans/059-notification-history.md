# Plan 059: SPIKE — persist and browse past notifications

## Status

- **Priority**: P3
- **Effort**: M (spike doc) / M-L (build)
- **Risk**: LOW-MED — new persistent storage of potentially sensitive
  content (permission-prompt bodies, cmux/agent output) is a privacy
  surface this app has never had before.
- **Depends on**: none (touches the Settings sidebar, so soft-coordinate
  with plan 049 if that per-source-config-consolidation work is also
  in flight)
- **Category**: direction
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  question ("do we hold past notification data? can I look it up in the
  control panel?") during a live UI walkthrough.

## Why this matters

Confirmed by direct read: **nothing is persisted today.** `logging.rs`
writes only `tracing`-level diagnostic logs (poll failures, bind errors)
to `~/Library/Logs/notchtap/notchtap.log` — not notification content.
`SettingsApp.tsx`'s sidebar (`SectionId`, `SettingsApp.tsx:82-90`) has no
History/Recent/Log tab. Once an item leaves the single-slot
`SingleSlotQueue` (dismissed or rotated out), nothing about it survives
anywhere. An operator who glances away and misses a card has no way to
recover what it said.

## Current state — what this would require

1. **Storage**: nothing in this app writes structured data today beyond
   config (`~/.config/notchtap/config.toml`) and diagnostic logs. A
   history feature needs an actual store — likely SQLite (a new
   dependency) or an append-only JSONL file (simpler, no new dependency,
   matches this repo's existing preference for plain-file config over
   databases).
2. **What gets stored**: the full `Event` struct (title/body/source/etc.)
   as it already flows through the queue — this is the same data already
   in memory at promotion time, just needs a write-out hook.
3. **Privacy surface — the real design question**: this app already
   handles genuinely sensitive content by design — cmux/Claude-Code
   permission-prompt bodies (`git push origin master`, tool names, project
   paths — see the plan 035 manifest example in `DESIGN.html`), agent
   output, whatever a `/notify` caller sends. Today that content is
   **ephemeral by construction** — it exists on screen briefly, then
   nothing. Persisting it changes the app's privacy posture materially:
   a history file becomes a target (disk access, backups, another app
   reading it) in a way "it was on screen for 8 seconds" never was.
4. **UI surface**: a new Settings sidebar section (History), a list view,
   probably a retention/clear-all control given point 3.

## Decision needed (operator)

1. Is persisting notification content (including cmux/hook payloads)
   actually wanted, given the privacy tradeoff above — or does this only
   make sense for a subset of sources (e.g. ESPN/weather/news, which are
   already public data, excluding cmux/manual which can carry
   command-line/project-path content)?
2. Retention: how long, is there a cap, is there a manual "clear
   history" control (there should be, at minimum)?
3. Storage mechanism: SQLite vs. JSONL — SQLite is more queryable
   (useful for a "what happened last Tuesday" search) but a new
   dependency; JSONL matches this repo's plain-file convention but needs
   its own rotation logic (this repo already has that pattern —
   `logging.rs`'s size-rotating appender — that could be reused/adapted).

## Recommendation

Scope to the sources without sensitive-content risk first (ESPN, RSS,
weather) if this is pursued, and treat cmux/manual history as a separate,
explicitly-opt-in decision — don't default to persisting agent/hook
payloads just because the mechanism exists for other sources. This
mirrors how `weather_enabled`/`rss_enabled` already default off
(opt-in) rather than on.

## Maintenance notes

- If built, `docs/TESTING_STRATEGY.md` needs a new component-test row for
  whatever storage layer is chosen, following this repo's existing
  "write tests before/alongside, not after" convention.
- Any retention/rotation logic should follow `logging.rs`'s existing
  size-based rotation as the exemplar pattern, per this repo's stated
  convention of matching established patterns rather than inventing new
  ones.
