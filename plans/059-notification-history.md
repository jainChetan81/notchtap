# Plan 059: SPIKE — persist and browse past notifications

## Status

- **Priority**: P3
- **Effort**: M (spike doc) / M-L (build)
- **Risk**: LOW-MED — new persistent storage of potentially sensitive
  content (permission-prompt bodies, cmux/agent output) is a privacy
  surface this app has never had before.
- **Depends on**: none (plan 049, the per-source-config-consolidation
  spike this originally soft-coordinated with, is DONE as of the
  2026-07-21 review — no coordination needed anymore)
- **Category**: direction
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  question ("do we hold past notification data? can I look it up in the
  control panel?") during a live UI walkthrough.

## Why this matters

Confirmed by direct read: **nothing is persisted today** (one narrow
exception, below). `logging.rs` writes only `tracing`-level diagnostic
logs (poll failures, bind errors) to
`~/Library/Logs/notchtap/notchtap.log` — not notification content, with
one exception: on a telegram send drop, `src-tauri/src/notifier.rs:281`
logs the event *title* (`title = %event.payload.title`) into that
diagnostic log — and since plan 077's Diagnostics section, that log is
browsable inside Settings via `get_recent_log_lines`
(`src-tauri/src/settings.rs:791`). So today, a connector failure can
already put a notification title on disk AND in the control-panel UI —
unplanned, connector-failure-only, but real. Plan 070's ingest logging
was checked and is content-clean (`engine.rs:189-192` logs only
id/origin/priority/error, never title/body).
`SettingsApp.tsx`'s sidebar (`SectionId`,
`src/settings/SettingsApp.tsx:95-104` — nine sections as of 2026-07-21,
including the newer `diagnostics`) has no History/Recent/Log tab. Once an item leaves the single-slot
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
   Note the posture is already not perfectly clean: dropped telegram
   sends log the title to the diagnostic log today (see "Why this
   matters") — deciding this plan is also implicitly deciding whether
   that existing narrow leak is acceptable or should be redacted the way
   plan 006 redacted the bot token from transport errors.
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
4. Independent of 1-3: keep or redact the EXISTING title-on-drop log
   line (`notifier.rs:281` — see "Why this matters")? Redaction is a
   one-line S-effort fix in plan 006's pattern; keeping it is also a
   defensible choice (it's the only clue when a telegram send silently
   drops) — but decide it explicitly rather than by omission.

## How this proceeds

This file is a decision memo, not an executor plan — there is nothing to
dispatch. When the operator answers the three questions above, the
advisor files the outcome as a NEW plan (next free number): a spike-shaped
plan (deliverable: a `docs/design/` doc, zero production code — the plan
030/031/086 shape) if the answers leave open design work, or a build plan
if they fully pin the design. If the answer to question 1 is "no
persistence at all," record that in `plans/README.md`'s
findings-considered-and-rejected section and retire this file — and
separately decide whether the existing notifier.rs title-on-drop log line
should be redacted (a one-line S-effort fix, filable on its own).

## Recommendation

Scope to the sources without sensitive-content risk first (ESPN, RSS,
weather) if this is pursued, and treat cmux/manual history as a separate,
explicitly-opt-in decision — don't default to persisting agent/hook
payloads just because the mechanism exists for other sources. This
mirrors how `weather_enabled`/`rss_enabled` already default off
(opt-in) rather than on.

## DECIDED — operator decision session, 2026-07-21

All four questions answered; this memo's decision phase is CLOSED:

1. **Persist ALL sources, including cmux/manual** — the operator
   explicitly accepted the privacy tradeoff, going broader than the
   recommendation above (which is retained for the record, not as the
   decision).
2. **Retention: size-capped + manual "Clear history" control** — keep
   the last N items / N MB (exact figure chosen at build-plan time),
   following `logging.rs`'s size-rotation precedent; no time-based
   pruning.
3. **Storage: JSONL, size-rotated** — append-only
   `~/.config/notchtap/history.jsonl` (path per the `settings.rs`/
   `notifier.rs` config-dir conventions), reusing/adapting
   `logging.rs`'s rotation pattern; no SQLite, no new dependency.
4. **Keep the title-on-drop log line** (`notifier.rs:281`) as-is — a
   deliberate debuggability tradeoff, consistent with choosing full
   history; decided explicitly, not by omission.

**Next step (deliberately NOT taken today — operator asked to wrap up
planning without starting new work)**: a future session files the build
plan at the next free plan number per "How this proceeds" — the answers
above fully pin the design, so it's a build plan, not a spike. Until
then this file stays in `plans/` as the decision of record.

## Maintenance notes

- If built, `docs/TESTING_STRATEGY.md` needs a new component-test row for
  whatever storage layer is chosen, following this repo's existing
  "write tests before/alongside, not after" convention.
- Any retention/rotation logic should follow `logging.rs`'s existing
  size-based rotation as the exemplar pattern, per this repo's stated
  convention of matching established patterns rather than inventing new
  ones.

**Review-plan pass (2026-07-21)**: Verified at HEAD `647f6d0`. Fixed the
drifted `SectionId` citation (`82-90` → `src/settings/SettingsApp.tsx:95-104`;
the sidebar grew to nine sections — football/diagnostics etc. — still no
History tab, so the core claim holds) and retired the plan-049
coordination note (049 is done). All other claims re-verified:
`SingleSlotQueue` (`src-tauri/src/queue.rs:48`), `rss_enabled`/
`weather_enabled` default false (`src-tauri/src/config.rs:484,500`
asserts), log dir `~/Library/Logs/notchtap` (`logging.rs:27-33`),
`DESIGN.html` manifest example present. Found the title-on-drop caveat
(notifier.rs), initially recorded only in this note.

**Review-plan pass 2 (2026-07-21, at `f084fa8`)**: all five cited source
files confirmed byte-unchanged since pass 1 (087's landing touched none
of them). Pass 1's caveat was material to the exact decision this memo
asks for, so it now lives in the body, upgraded with two verified facts:
plan 077's Diagnostics section makes the diagnostic log — and therefore
any dropped-send title — browsable in Settings via `get_recent_log_lines`
(`settings.rs:791`), and plan 070's ingest logging was checked
content-clean (`engine.rs:189-192`: id/origin/priority only). Also added
the "How this proceeds" section (decision → next-numbered spike/build
plan, or retirement + an optional one-line title-redaction fix), so the
memo states its own exit path. Still NEEDS-OPERATOR-DECISION — nothing
here is dispatchable.
