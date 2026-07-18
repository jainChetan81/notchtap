# Plan 035: Rich relay manifest — wire `subtitle`/`details`, CLI flags, claude-code + cmux hooks

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d926977..HEAD -- notchtap src-tauri/src/http.rs src-tauri/src/event.rs src/components/Manifest.tsx`
> On any change, re-verify the excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (extends the locked `/notify` wire contract and adds two
  new ingest paths — both heads-up-only, but they touch ARCHITECTURE §7's
  cli-contract wording)
- **Depends on**: none
- **Category**: wire/ui
- **Planned at**: commit `d926977`, 2026-07-18, from the prototype's
  section-5 design review. Layout variant **A (detail cells)** chosen by
  the operator over B (full-width rows) and C (terminal block).

## Decisions locked (operator, 2026-07-18)

1. **Data sources**: a Claude Code hook *and* a cmux `notifications.hooks`
   entry — both chosen. They give us real structure the plain
   notification-command never had: `hook_event_name`, `tool_name`,
   `tool_input`, `cwd` (claude) and workspace title/subtitle/`cwd` (cmux).
2. **Wire amendment (ARCHITECTURE §7)**: `--subtitle` becomes a
   first-class optional wire field instead of being folded into the body
   CLI-side, and `/notify` gains optional `details: [{label, value}]`.
   The "server never learns subtitle exists" line is amended by this
   plan; existing callers keep working (subtitle was always optional).
3. **Manifest layout A**: subtitle and each detail render as ordinary
   cells in the manifest's existing 2-col grid — no new visual language.
4. **Caps** (display safety inside the fixed 500×300 window): subtitle ≤
   120 chars, ≤ 8 detail pairs, label ≤ 40 chars, value ≤ 200 chars —
   truncated server-side with an ellipsis; the hooks truncate earlier.
5. **Hooks are observational only**: they post and exit 0 immediately
   (the post runs backgrounded). They never write a decision to stdout —
   the respond-back loop stays out of scope per ARCHITECTURE §7.

## Why this matters

cmux's notification-command hands us exactly three plain strings; the
operator wants the expanded card to show *what* is being asked
(permission: which tool, which command; errors: what failed). That data
exists only in the two hook payloads above — so the card gets a real
information hierarchy instead of "Agent needs input" for everything.

## Current state (verified at `d926977`)

- `NotifyRequest` (`http.rs`) accepts `{title, body, priority?, signal,
  source?}`; `EventMeta::default()` is set at the handler.
- `EventMeta` (`event.rs:127-134`): `source/category/published_at_ms/link`,
  all optional, presentation-only. `SlotState::Showing` mirrors them;
  `queue.rs:385-402` (`current_slot_state`) is the single passthrough.
- CLI `notchtap`: flags only; `--subtitle` folds into the body as
  `"<subtitle> — <body>"`; cmux auto-detect via `CMUX_NOTIFICATION_BODY`.
- `Manifest.tsx` generic branch: two cells (Message, Source/Control);
  news branch: three cells.
- cmux hook JSON (verified against cmux.com/docs/notifications):
  `{notification:{title,subtitle,body}, context:{cwd,…}, effects:{…}}`
  on stdin; a hook must emit valid JSON on stdout or cmux falls back to
  default behaviour. Claude Code hook JSON: `hook_event_name`, `tool_name`,
  `tool_input`, `message`, `cwd`, `session_id`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test` from `src-tauri/` | all pass |
| Frontend tests | `npx vitest run` | all pass |
| CLI + hook syntax | `sh -n notchtap hooks/*.sh` | exit 0 |
| Typecheck/build | `npx tsc --noEmit && npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/event.rs` — `DetailItem { label, value }`,
  `EventMeta.subtitle/details`, `SlotState` fields, snapshot tests
- `src-tauri/src/queue.rs` — passthrough in `current_slot_state` (+ test)
- `src-tauri/src/http.rs` — request fields + `sanitize_details` caps (+ tests)
- `notchtap` CLI — `--subtitle` as field, repeatable `--detail Label=Value`
- `hooks/notchtap-claude-hook.sh`, `hooks/notchtap-cmux-hook.sh` (new)
- `src/useSlotState.ts` validation; `src/components/Manifest.tsx` (variant A);
  `src/styles.css` + `src/settings/preview-overlay.css`
  (`.detail-value { overflow-wrap: anywhere; }`); tests
- `docs/ARCHITECTURE.md` §7 cli-contract paragraph (subtitle amendment +
  hook paths), `docs/V3_6_TECHNICAL_SPEC.md` wire schema,
  `docs/TESTING_STRATEGY.md` §4.3 cases + §0 counts

**Out of scope**:
- Any respond-back/decision path from a hook (hard out)
- Connectors: telegram formatting ignores `subtitle`/`details` (its input
  is title/body — unchanged)
- A new `SourceKind` for the claude hook: it posts `source: "cmux"` —
  the existing "agent relay" origin class (priority/rotation settings
  already apply); a distinct origin is its own future plan if ever wanted
- The user's own `~/.claude/settings.json` / `cmux.json` edits (operator
  step, snippets below)

## Git workflow

- Current branch; commits: rust+wire, CLI+hooks, frontend, docs. Do NOT push.

## Steps

### Step 1: Rust plumbing

`DetailItem` (Serialize/Deserialize/PartialEq). `EventMeta` gains
`subtitle: Option<String>`, `details: Vec<DetailItem>` (both
`#[serde(default)]`-covered). `SlotState::Showing` gains the same two
(camelCase: `subtitle`, `details`). `current_slot_state` passes them
through. Update the two event.rs snapshot tests (`subtitle` null-or-string,
`details` array). Queue test: an enqueued event's details appear in
`current_slot_state`.

### Step 2: `/notify` schema + caps

`NotifyRequest` gains `subtitle: Option<String>`,
`details: Option<Vec<DetailItem>>`. `sanitize_details`: drop empty
labels, take ≤ 8, truncate label/value (40/200 chars + `…`); subtitle ≤
120. Empty-string subtitle → `None`. Tests: full round-trip (POST with
both fields → `current_slot_state` shows them), caps applied, absent
fields → `None`/empty (back-compat).

### Step 3: CLI

`--subtitle` stops folding and posts the field (header comment + usage
updated). New repeatable `--detail Label=Value` (≤ 8; must contain `=`;
label non-empty — else `usage`). jq builds the `details` array with
`--arg` per pair (value may itself contain `=` — split on the *first*
`=`). `sh -n` stays clean.

### Step 4: Hook scripts (`hooks/`)

`notchtap-claude-hook.sh`: reads the hook JSON on stdin; maps
`hook_event_name` — `Notification` (title "Claude Code", body=message,
priority high, subtitle "Notification"), `PermissionRequest` (body
`Permission requested: <tool>`, subtitle "Permission request", priority
high, details Tool/Command-or-File/Project), `Stop` (body "Session
complete", priority medium), `PostToolUse` with matcher `Task` (body
"Agent finished", priority medium). `Command` comes from
`tool_input.command` (Bash), `File` from `tool_input.file_path`
(Edit/Write), else a ≤200-char compact `tool_input`. Project = `cwd`.
Posts via the `notchtap` CLI (PATH lookup; quietly exit 0 if absent),
**backgrounded**, then `exit 0` — never blocks, never answers.

`notchtap-cmux-hook.sh`: reads cmux's hook JSON; **echoes it unchanged
to stdout first** (cmux applies whatever the hook returns; pass-through
keeps cmux's own behaviour); then posts
`--title <notification.title> --body <notification.body> --source cmux --priority high`
plus `--subtitle` when present and `--detail "Project=<cwd>"` when
present, backgrounded, `exit 0`.

### Step 5: Frontend

`useSlotState.ts`: `subtitle` null-or-string; `details` an array whose
items have string `label`/`value`. `Manifest.tsx` generic branch appends,
after the two existing cells: a `Subtitle` cell when present, then one
cell per detail pair (`detail-label` = label, `detail-value` = value).
`.detail-value { overflow-wrap: anywhere; }` in both CSS files. Tests:
validator cases; expanded generic card renders subtitle + detail cells;
collapsed card renders neither; a `details: "nope"` payload falls back
to empty.

### Step 6: Docs

ARCHITECTURE §7: amend the subtitle-fold sentence (subtitle is now a
first-class optional field) and add a short paragraph for the two hook
paths (heads-up only, backgrounded, exit 0 — the respond-back rule is
unchanged). V3_6 spec wire schema + TESTING_STRATEGY §4.3 (new cases) +
§0 counts.

### Step 7: Operator config (manual, report the snippets)

`~/.claude/settings.json` hooks block (Notification, PermissionRequest,
Stop, PostToolUse matcher `Task` → `hooks/notchtap-claude-hook.sh`) and
`cmux.json` `notifications.hooks` entry → `hooks/notchtap-cmux-hook.sh`.
Both scripts must be on PATH-resolvable absolute paths in the configs.

## Test plan

- rust: DetailItem serde, slot-state snapshots, queue passthrough,
  sanitize caps, back-compat absence
- frontend: validator + Manifest render cases
- `sh -n` both hooks + CLI
- §0 counts updated
- Manual (operator): a real Claude Code permission prompt renders the
  expanded card with Tool/Command/Project cells

## Done criteria

- [ ] Old wire payloads (no subtitle/details) behave byte-identically
- [ ] Caps enforced server-side (9 pairs → 8; 500-char value → 200+`…`)
- [ ] CLI `--subtitle` no longer folds (manifest shows its own cell)
- [ ] Both hooks `sh -n`-clean, exit 0 without notchtap on PATH
- [ ] `cargo test`, `npx vitest run`, `npx tsc --noEmit`,
      `npx vite build` green
- [ ] `plans/README.md` row updated

## STOP conditions

- Any hook requirement to return content to Claude Code (a decision, a
  modified payload) — that reopens the respond-back exclusion; stop
- cmux's hook contract drifted from the documented stdin-JSON/stdout-JSON
  shape — re-verify against cmux.com/docs/notifications before writing
  the script
- A third party asks for `details` to influence priority/rotation —
  details are display-only, forever; stop

## Maintenance notes

- The caps exist because the card lives in a fixed 500×300 window; if the
  window ever grows, revisit the numbers, not the mechanism.
- Telegram deliberately never learns about `details` — if a connector
  ever wants them, that's a formatting decision for that connector's own
  plan, not a given.
