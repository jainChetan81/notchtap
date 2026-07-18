# Plan 035: Rich relay manifest — wire `subtitle`/`details`, CLI flags, claude-code + cmux hooks

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 339156a..HEAD -- notchtap src-tauri/src/event.rs src-tauri/src/queue.rs src-tauri/src/http.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src/useSlotState.ts src/components/StatusRailCard.tsx src/components/Manifest.tsx`
> On any change, re-verify the excerpts below; mismatch = STOP. (Baseline
> re-stamped d926977 → 339156a in the 2026-07-19 reconcile; the watch list
> now also covers the files plan 033 touched.)

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
- **Reviewed**: 2026-07-18 at `1add02e` (review-plan pass) — repo-side
  excerpts all re-verified (NotifyRequest shape, CLI fold at
  `notchtap:4-6`/:83, Manifest 2-cell generic / 3-cell news branches,
  exactly three exhaustive `EventMeta` literals at rss_poller.rs:332/:834
  + settings.rs:536, TS `SlotState` fields non-optional so the
  compiler-lists-fixtures claim holds); claude-hook contract re-verify
  STOP added (was cmux-only), git-status criterion added
- **Reconciled**: 2026-07-19 at `339156a` (`/improve execute` pre-flight,
  per closing-the-loop "don't hand a stale plan to an executor"). Plan 033
  landed since `d926977` and rewrote the shared surfaces this plan extends;
  every excerpt below was re-verified against `339156a`. Material changes:
  (1) `SlotState::Showing` now carries two **required** fields
  `queue_total`/`queue_done` (plan 033) that every full literal must set —
  `subtitle`/`details` join them, not replace the four EventMeta mirrors;
  (2) `src/components/StatusRailCard.tsx` **added to scope** — it is the
  component that renders `<Manifest>` (`:149`) and must thread the two new
  props (Manifest is *not* rendered from App.tsx); (3) line refs refreshed —
  `current_slot_state` is `queue.rs:480-510` (Showing literal `:493-507`),
  the three `EventMeta` literals are `rss_poller.rs:332` & `:812` +
  `settings.rs:536`, `EventMeta` is `event.rs:127-132`, `NotifyRequest` is
  `http.rs:110`; (4) both `.detail-value` CSS rules already exist
  (`styles.css:410`, `preview-overlay.css:350`) — extend, don't duplicate;
  (5) the generic Manifest Message cell now renders `renderInlineMarkdown`
  (plan 032) — the append target (the generic `.manifest-inner` div) is
  unchanged

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

## Current state (re-verified at `339156a`, 2026-07-19)

- `NotifyRequest` (`http.rs:110`) accepts `{title, body, priority?, signal,
  source?}` (title/body are `Option`, `signal` is `#[serde(default)]`); the
  `/notify` handler builds `meta: EventMeta::default()` (`http.rs:179`).
- `EventMeta` (`event.rs:127-132`): `source/category/published_at_ms/link`,
  all optional, presentation-only; derives `Default` + `#[serde(default)]`.
  Its three full literals are `rss_poller.rs:332` & `:812` + `settings.rs:536`.
- `SlotState::Showing` (`event.rs:146-164`) mirrors those four **and** carries
  two required plan-033 fields `queue_total: u32`/`queue_done: u32` (the enum
  has no `Default`, so every full literal must list all fields).
  `current_slot_state` (`queue.rs:480-510`, Showing literal `:493-507`) is the
  single passthrough. The other full `SlotState::Showing { … }` literals the
  compiler will flag: the two `event.rs` snapshot tests (`:277`, `:315`) and
  two test literals (`queue.rs:1696`, `lib.rs:1281`).
- CLI `notchtap`: flags only; `--subtitle` folds into the body as
  `"<subtitle> — <body>"` (`notchtap:83-85`); cmux auto-detect via
  `CMUX_NOTIFICATION_BODY` (`:91`).
- `Manifest.tsx` is rendered by `StatusRailCard.tsx:149`, which passes each
  prop from `slot`. Generic branch (`Manifest.tsx:71-84`): two cells —
  Message (now `renderInlineMarkdown(body)`, class `detail-value message`,
  plan 032) and Source/Control. News branch: three cells (unchanged).
- TS `SlotState` "showing" (`useSlotState.ts:27-44`) already has required
  `queueTotal`/`queueDone`, validated by `isNonNegativeInteger`. New required
  `subtitle`/`details` there make `tsc` list every inline showing fixture
  (`App.test.tsx`, `useSlotState.test.ts`, `StatusRailCard.test.tsx`,
  `SettingsApp.tsx` preview samples).
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
| CLI + hook syntax | `for f in notchtap hooks/notchtap-claude-hook.sh hooks/notchtap-cmux-hook.sh; do echo "== $f"; sh -n "$f" || echo "SYNTAX FAIL: $f"; done` | each file printed, no `SYNTAX FAIL` line (`sh -n f1 f2` only checks f1 — must loop) | 
| Typecheck/build | `npx tsc --noEmit && npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/event.rs` — `DetailItem { label, value }`,
  `EventMeta.subtitle/details`, `SlotState` fields, snapshot tests
- `src-tauri/src/queue.rs` — passthrough in `current_slot_state` (+ test)
- `src-tauri/src/rss_poller.rs`, `src-tauri/src/settings.rs` — the three
  exhaustive `EventMeta{…}` literals (2 rss, 1 test-notification) gain
  the new fields (the compiler lists them; those events leave them empty)
- `src-tauri/src/http.rs` — request fields + `sanitize_details` caps (+ tests)
- `notchtap` CLI — `--subtitle` as field, repeatable `--detail Label=Value`
- `hooks/notchtap-claude-hook.sh`, `hooks/notchtap-cmux-hook.sh` (new)
- `src/useSlotState.ts` — `SlotState` "showing" gains required
  `subtitle: string | null` + `details: {label,value}[]`, plus validation
- `src/components/Manifest.tsx` (variant A) — new `subtitle`/`details` props
- `src/components/StatusRailCard.tsx` — thread `subtitle={slot.subtitle}`
  and `details={slot.details}` into `<Manifest>` (`:149`). **Without this the
  new cells never reach the real render path** — and a test that renders a
  bare `<Manifest subtitle=… details=…/>` would still pass, so it is not
  evidence the feature works; test through `StatusRailCard`
- `src/styles.css` + `src/settings/preview-overlay.css` — add
  `overflow-wrap: anywhere;` to the **existing** `.detail-value` rules
  (`styles.css:410`; `.appearance-preview .detail-value` at
  `preview-overlay.css:350`); do NOT add a duplicate selector
- tests — **every inline TS `SlotState` showing fixture / preview-sample
  gains `subtitle`/`details`** (compiler-listed; the `as unknown as
  SlotState` invalid-payload casts in `useSlotState.test.ts` stay untouched —
  they exist to be rejected by the validator)
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

`DetailItem { label: String, value: String }` (Serialize/Deserialize/
PartialEq). `EventMeta` gains `subtitle: Option<String>`,
`details: Vec<DetailItem>` (both covered by its existing `#[serde(default)]`;
its three full literals — `rss_poller.rs:332` & `:812`, `settings.rs:536` —
each get `subtitle: None, details: Vec::new()`, compiler-flagged).
`SlotState::Showing` gains the same two (camelCase on the wire: `subtitle`,
`details`) **beside its existing required `queue_total`/`queue_done`** — so
every full literal must list them: `current_slot_state` (the passthrough —
copy from `item.event.meta`), the two `event.rs` snapshot tests (`:277`,
`:315`), and the two test literals (`queue.rs:1696`, `lib.rs:1281`, where
`None`/empty is fine). Update the two event.rs snapshot tests (`subtitle`
null-or-string, `details` array — note they already assert
`queueTotal`/`queueDone`). Queue test: an enqueued event's details appear in
`current_slot_state`.

### Step 2: `/notify` schema + caps

`NotifyRequest` gains `subtitle: Option<String>`,
`details: Option<Vec<DetailItem>>`. `sanitize_details`: drop empty
labels, take ≤ 8, truncate label/value (40/200 chars + `…`); subtitle ≤
120. Empty-string subtitle → `None`. Wire the sanitized values into the
`/notify` handler's `EventMeta`: it currently builds `EventMeta::default()`
at `http.rs:179` — set `subtitle`/`details` there (the other three
`EventMeta::default()` sites, `:513`/`:673`/`:765`, are tests — leave them,
`Default` now covers the new fields). Tests: full round-trip (POST with
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

`useSlotState.ts`: add required `subtitle: string | null` +
`details: {label,value}[]` to the `SlotState` "showing" type, and validate
in `isValidSlotState` (`subtitle` null-or-string; `details` an array whose
items are objects with string `label`/`value` — mirror the existing
`isNonNegativeInteger` helper style). `Manifest.tsx`: add `subtitle` +
`details` props; the generic branch appends, after the two existing cells,
a `Subtitle` cell when present, then one cell per detail pair
(`detail-label` = label, `detail-value` = value; render as plain text, not
markdown — per locked layout A, and details come from untrusted hook input).
**`StatusRailCard.tsx:149`: thread `subtitle={slot.subtitle}` and
`details={slot.details}` into `<Manifest>`** — the render path runs through
here; every inline showing fixture in the four test/preview files gains the
two fields (tsc lists them). Add `overflow-wrap: anywhere;` to the existing
`.detail-value` rule in `styles.css` (`:410`) and the
`.appearance-preview .detail-value` rule in `preview-overlay.css` (`:350`) —
no duplicate selectors. Tests (exercise the real path via `StatusRailCard`,
not a bare `<Manifest>`): validator cases; expanded generic card renders
subtitle + detail cells; collapsed card renders neither; a `details: "nope"`
payload falls back to empty.

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
- [ ] `git status --short` shows, beyond whatever was already dirty
      before your first edit (concurrent sessions share this checkout —
      snapshot it first, never revert/stage/commit those paths),
      modifications ONLY to in-scope files
- [ ] `plans/README.md` row updated

## STOP conditions

- Any hook requirement to return content to Claude Code (a decision, a
  modified payload) — that reopens the respond-back exclusion; stop
- cmux's hook contract drifted from the documented stdin-JSON/stdout-JSON
  shape — re-verify against cmux.com/docs/notifications before writing
  the script
- The Claude Code hook contract gets the same treatment (added at
  review — the plan-time draft guarded only cmux): before writing
  `notchtap-claude-hook.sh`, verify the event names this plan maps
  (`Notification`, `PermissionRequest`, `Stop`, `PostToolUse`) and the
  payload fields (`hook_event_name`, `tool_name`, `tool_input`, `message`,
  `cwd`) against the official Claude Code hooks docs
  (docs.anthropic.com → Claude Code → hooks). In particular, if
  `PermissionRequest` is not a documented hook event in the installed
  version, map permission prompts via the `Notification` event's
  message instead — report the substitution, don't invent fields.
- A third party asks for `details` to influence priority/rotation —
  details are display-only, forever; stop

## Maintenance notes

- The caps exist because the card lives in a fixed 500×300 window; if the
  window ever grows, revisit the numbers, not the mechanism.
- Telegram deliberately never learns about `details` — if a connector
  ever wants them, that's a formatting decision for that connector's own
  plan, not a given.
- The `justfile`'s `check-cli` recipe and CI's `sh -n` gate cover only
  `notchtap` today; extending them to `hooks/*.sh` is a sensible
  follow-up but out of this plan's scope — note it in the completion
  report rather than editing `justfile`/`ci.yml` here.
