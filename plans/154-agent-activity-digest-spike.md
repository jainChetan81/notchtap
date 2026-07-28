# Plan 154: SPIKE — decide where a "what did my agents do" digest gets its data and where it renders

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **This is a SPIKE. It produces ONE document and writes NO code — not in
> `src/`, not in `src-tauri/`, not a scratch script at the repo root or in
> `/tmp`. No prototype, no scaffold, no "just the struct".** A fenced
> ```rust``` or ```ts``` block anywhere in the document you write is a
> failed spike; describe record shapes in prose or a markdown table.
>
> **You cannot ask the operator anything.** Every judgement you cannot
> ground in the repo becomes a written entry in the document's questions
> section.
>
> **Drift check (run first)**:
> `git diff --stat acdaeb0..HEAD -- src-tauri/src/history.rs src-tauri/src/engine.rs src-tauri/src/agents/ src-tauri/src/settings.rs src/settings/sections/HistorySection.tsx src/useAgentState.ts docs/ARCHITECTURE.md`
> These are the files this plan's reasoning depends on, not the files it
> writes — that inversion is deliberate for a spike. If any changed,
> re-verify the "Current state" excerpts before proceeding.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW (no code changes)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `acdaeb0`, 2026-07-28

## Why this matters

Since v7, Agent is the product's dominant Origin: four coding-agent
runtimes push session lifecycle and permission/input events all day. The
overlay is deliberately ephemeral — a card shows for a few seconds and is
gone, and the Agent Board evicts a finished session after 60 seconds. If
you were away from the machine, or Silenced, that activity is lost.

A "what did my agents do today" surface is the obvious complement, and it
is the one direction idea in this repo's backlog raised repeatedly and
never planned — `plans/README.md` records it as *"a 'what did I miss'
history surface… weakest grounding, needs a persistence decision"*.

That persistence decision is what this spike makes. Building on the wrong
data source is expensive to undo: option A costs nothing but
under-reports, option B adds a new on-disk file with its own privacy and
retention questions. Writing the comparison down first is cheap;
discovering it mid-build is not.

## Current state

### What is persisted today

`src-tauri/src/history.rs` is an append-only JSONL store. Its module doc
(`history.rs:1-5`) is explicit about the single writer:

```rust
//! Plan 088: append-only JSONL notification history, gated behind the
//! opt-in `history_enabled` config flag (default `false`, see
//! `config.rs`). The only writer is `Engine::accept`; the settings
//! window's `get_history`/`clear_history` invoke commands (plan 089,
//! `settings.rs`) are the read/clear surface.
```

The record shape (`history.rs:36-40`) is only two fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub recorded_at_ms: i64,
    pub event: Event,
}
```

The write is **one-shot only** — `src-tauri/src/engine.rs:327-331` gates
on `if matches!(to_offer.rotation, RotationSpec::OneShot { .. })` under a
comment reading `// plan 088: best-effort history append. ONE-SHOT ONLY`.
It sits *after* `enqueue_result?`, so a rejected enqueue never reaches it.

Storage posture: the config dir is pinned `0700` and the file `0600`, the
same posture as `secrets.toml`. Size-rotated at 5 MB, current plus two
backups (`history.rs:29-30`).

Default is `history_enabled = false` (`config.rs:449-451`). **The
operator has it enabled** in their live config.

### Three constraints on Option A that decide the whole comparison

**1. The read path is capped at 200 entries, current file only.**
`get_history` and `clear_history` both call `store.read_recent(200)`
(`src-tauri/src/settings.rs:1030` and `:1034`), and `read_recent`
(`history.rs:158`) reads **only `history.jsonl`, not the rotated `.1`/`.2`
backups** — its doc says "rotated backups stay out of scope". Those 200
records are shared across *all* origins (football, news, weather, manual,
agent). On a busy day a digest built on `get_history` unchanged may see
only a few hours of agent activity. It also cannot be date-ranged or
origin-filtered server-side. This materially changes both the Option A
analysis and the Step 4 answer about whether a new `#[tauri::command]` is
needed.

**2. History records *acceptance*, never *display*.** `HistoryEntry` has
no "was this seen?" field, and `Engine::accept` writes at acceptance —
not at promotion, not at dismissal, not at expiry. So the candidate
intent "show me what I missed while Silenced" is **unanswerable from
option A, and equally unanswerable from B and C as described below**,
because none of them record display outcome. If you choose that intent as
primary, you must say plainly that every option fails it and that a new
field would be required.

**3. A noteworthy Agent event lost to a full queue tier is not
recorded.** `QueueError::QueueFull` returns before the append
(`src-tauri/src/http.rs:521-523`). So "a noteworthy Agent event is
recorded" is really "is recorded if it won a queue slot".

### What that means for Agent coverage

Agent notifications are `RotationSpec::OneShot`
(`src-tauri/src/agents/notification.rs:235`), so a noteworthy Agent event
**is** recorded today (subject to constraint 3). Three categories are not:

1. **Non-noteworthy events.** `is_noteworthy`
   (`src-tauri/src/agents/notification.rs:92-102`) gates `Informational`
   and non-terminal `Failed` behind `policy.informational_notifications`.
   The operator has that **off**, so those never become notifications and
   never reach history.
2. **All board-only session lifecycle.** `Starting → Working →
   WaitingForInput → Completed` updates the Agent Registry and Board
   without a card per tick — the two-path design in
   `docs/ARCHITECTURE.md` §20 (*"session lifecycle/progress updates the
   separate Rust-owned Agent Registry and Agent Board without creating a
   card for every tick"*). None of it is persisted.
3. **Session-level facts.** Duration, final outcome, project, host,
   subagent activity, and the per-session transition history live only in
   memory. `AgentRegistry::tick`'s `retain`
   (`src-tauri/src/agents/registry.rs:325-333`) **discards the whole
   session**, transition list included, once `terminal_at` is older than
   `terminal_retention` (default **60 seconds**).

So today's history can tell you "a permission request happened at 14:02".
It cannot tell you "Claude Code ran for 40 minutes on project X, hit two
tool errors, and finished."

### Two known doc/code conflicts — do NOT treat as drift

- `docs/ARCHITECTURE.md:781` still says agent terminal retention
  "defaults to ten minutes". The code says **60 seconds**
  (`registry.rs:24-26`, `config.rs:252`), tightened 2026-07-27 by an
  operator decision. **The code is right; the doc is stale. This is
  known.** Do not stop on it.
- `src-tauri/build.rs:8` says "eighteen settings-window commands" while
  `settings_commands.rs` asserts **seventeen**. Also a known stale
  comment. Do not stop on it.

### The IPC cost of a new read path

Adding any new `#[tauri::command]` is a 4-place coordinated change:
`settings_commands.rs`'s list **and its
`assert_eq!(SETTINGS_COMMANDS.len(), 17)`**, `lib.rs`'s
`generate_handler!`, `capabilities/settings.json`, and `build.rs`'s
include. It is a security-load-bearing allowlist. Any option needing one
must say so and count the cost.

The existing read surface is `get_history` / `clear_history` plus
`src/settings/sections/HistorySection.tsx` (367 lines), whose own header
comment calls it *"a plain scannable list, not a card renderer"*.

### Constraints this spike must respect

From `docs/ARCHITECTURE.md` §20 (locked 2026-07-26) — quote both in the
document; they bound every option:

> **loopback structured ingestion**: Agent Adapters post a versioned,
> bounded normalized schema to `POST /agent/events`. raw hook payloads,
> prompts, tool output, secrets, and arbitrary executable actions never
> enter frontend IPC or persistence.

> **heads-up only**: Permission Requested and Input Required are
> high-priority heads-up states. notchtap never approves, rejects,
> replies, launches, supervises, or scrapes the runtime.

Also read `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §9 ("persistence
and privacy") before proposing any new on-disk file.

**One correction to carry into Option C's analysis**: the bounded
per-session transition history is **already published to the overlay** on
the live `agent-state` wire (`src-tauri/src/agents/model.rs:347`,
consumed at `src/useAgentState.ts:90`), as is `project.cwd`
(`src-tauri/src/agents/board.rs:128`, `src/useAgentState.ts:53`). So
Option C's privacy delta over B is "persist to disk what is already on
the IPC wire", **not** "expose something new". Do not assert that C has a
harder privacy story without making that argument on its merits.

### The structural exemplar

`docs/design/read-only-status-endpoint.md` is this repo's existing design
spike (plan 050). Read it before writing — it establishes the house
format: a status line declaring proposal-only with zero code written, a
"researched against commit" line, and an explicit attestation that the
`file:line` citations were verified. Match that register.

### Vocabulary (`CONTEXT.md` — use these words, do not invent synonyms)

**Agent Session**, **Agent Registry**, **Agent Board**, **Agent
Runtime**, **Terminal Retention**, **Origin**, **Silenced**.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Baseline rust suite | `cd src-tauri && cargo test --locked` | `0 failed` |
| Baseline frontend suite | `npx vitest run` | all pass |
| Record count | `wc -l ~/.config/notchtap/history.jsonl` | a number |
| Origin split | `jq -r '.event.origin' ~/.config/notchtap/history.jsonl \| sort \| uniq -c` | counts per origin |
| Agent runtime split | `jq -r 'select(.event.origin=="agent" or .event.origin=="cmux") \| .event.meta.agent.runtime // "none"' ~/.config/notchtap/history.jsonl \| sort \| uniq -c` | counts per runtime |
| Date range | `jq -r '.recorded_at_ms' ~/.config/notchtap/history.jsonl \| sort -n \| sed -n '1p;$p'` | two epoch-ms numbers |
| Agent record key paths | `jq -r 'select(.event.origin=="agent" or .event.origin=="cmux") \| [paths(scalars)\|join(".")] \| .[]' ~/.config/notchtap/history.jsonl \| sort \| uniq -c` | key paths + how often populated |

`jq` is at `/usr/bin/jq`. If `cargo` is not on PATH, prefix with
`PATH="$HOME/.cargo/bin:$PATH"`.

**Why `"cmux"` appears in those filters**: `SourceKind` carries
`#[serde(alias = "cmux")]` (`src-tauri/src/event.rs:106-107`), so records
written before plan 137 are serialized as `"origin":"cmux"` and
deserialize as `Agent`. Count and report the two separately — the split
tells you how far back agent coverage actually goes.

## Scope

**In scope** (the only files you may create or modify):

- `docs/design/agent-activity-digest.md` (create)
- `plans/README.md` (status row, plus **extending** the existing note —
  see Step 6)

**Out of scope** (do NOT touch):

- **Every file under `src/` and `src-tauri/`**, and any new file
  anywhere else.
- `~/.config/notchtap/history.jsonl` — read for measurement only; never
  write, move, clear, or rotate it.
- `~/.config/notchtap/secrets.toml` — never read it at all.
- `docs/ARCHITECTURE.md` — a decision entry follows the operator's
  decision, written by the build plan.
- Any new `#[tauri::command]`, and `src-tauri/capabilities/*`.

## Git workflow

- Branch: `advisor/154-agent-activity-digest-spike`
- Conventional commits, e.g.
  `docs(design): agent activity digest spike — data source and surface options`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 0: Record the baselines

**Verify**: `cd src-tauri && cargo test --locked` → `0 failed` (record the
pass count); `npx vitest run` → all pass (record the count).

### Step 1: Ground the problem in real data

Run the six measurement commands from the table above and record:
total records; the origin split; the agent/cmux split; the Agent Runtime
split; the date range the file spans; and which key paths an Agent record
populates.

**PRIVACY RULE — this is not optional and it governs your commands, not
just your output.**

`history.jsonl` contains real notification titles, bodies, project names
and paths from the operator's actual work. The sensitive fields are
`event.payload.title`, `event.payload.body`, and `event.meta.subtitle`
(the project name). `event.meta.agent.session_hash` is already a hash
(`src-tauri/src/agents/model.rs:165`) and is safe to count.

1. **Never run a command whose output includes a JSON *value* from this
   file.** That rules out `cat`, `head`, `tail`, `jq .`, `jq '.[]'`, and
   `grep` on the file. Every command in the table above projects to keys
   or aggregates only — use those. If you need a measurement they do not
   cover, construct it the same way (project to a key name or a count)
   and say in your report what you ran.
2. **The document, your commit message, and your final report contain
   aggregate counts and field names only.** No record, title, body,
   project name, path, or summary text.
3. **When you need to illustrate a record shape, use only these
   placeholder values**: `example-project`,
   `/Users/example/code/example-project`, `runtime-a`, `session-hash-abc`.
   Nothing else.

**Verify**: `grep -c '/Users/chetanjain' docs/design/agent-activity-digest.md`
→ `0`, and `grep -c 'example-project\|runtime-a' docs/design/agent-activity-digest.md`
→ at least `1` if you illustrated any shape at all.

**If the file does not exist or is empty**, or if the agent+cmux record
count is **zero**: do not stop. Write the document from the code instead,
open it with a line of the exact form
`> Grounding unavailable: <reason>` and say so in your report. Done
criterion 2 is then satisfied by that line instead of by measurements.

### Step 2: Create the document skeleton

Create `docs/design/agent-activity-digest.md` with **exactly these
headings, in this order**:

```markdown
# Agent activity digest — data source and surface spike

**Status**: design spike (plan 154) — proposal only, zero code written.
**Researched against**: commit acdaeb0
**Awaiting**: operator decision on data source and surface.

## Measurements

## Primary intent

## Data-source options

## Surface options

## Thin first slice

## Questions for the operator

## What this spike does not decide
```

Fill "Measurements" now from Step 1.

**Verify**: `grep -c '^## ' docs/design/agent-activity-digest.md` → `7`

### Step 3: Name the primary intent

Propose 2–3 concrete user intents and pick one. Candidates grounded in
this repo:

- **"What did I miss?"** — you were away or Silenced. **Note the trap
  from "Current state" constraint 2**: history records acceptance, never
  display, so no option answers this without a new field. If you pick it,
  say so explicitly.
- **"What happened today?"** — an end-of-day retrospective across
  sessions and runtimes.
- **"Is anything still waiting on me right now?"** — the Agent Board
  already answers this for live sessions. A digest that mostly duplicates
  the Board is a weak proposal; say so if you reach that conclusion.

Write the chosen one on its own line in exactly this form:

`**Primary intent**: <one sentence>`

Then one to two sentences per non-primary intent on why it is secondary.

**Verify**: `grep -c '^\*\*Primary intent\*\*: ' docs/design/agent-activity-digest.md` → `1`

### Step 4: Compare the three data-source options

Under "Data-source options", first a markdown table with **exactly these
columns**, one row per option:

`| option | what it can answer | what it cannot | code cost | new on-disk state | privacy delta | retention/migration questions |`

Then a `###` sub-section per option with the reasoning. Every cell
non-empty; no cell reading "TBD".

**Option A — reuse `history.jsonl`, filtered to Origin `Agent`.** Zero
new persistence, reuses `get_history`, gated by the existing
`history_enabled` flag. Under-reports exactly as "Current state"
documents — and note the 200-record, current-file-only read cap, which is
the binding constraint, not an afterthought.

**Option B — a new bounded agent-session journal.** One line per session
when it goes terminal or is evicted. Bounded by construction (per
session, not per event). Costs: a new file, a config flag, a
retention/rotation policy, a read path, and — if Settings is the surface
— a new `#[tauri::command]` and the 4-place parity change. The natural
write point is the same `AgentRegistry::tick` `retain` at
`registry.rs:325-333` that currently discards the session.

**Describe B's record contents in prose or a table only.** Do not write a
Rust struct, a `serde` derive, or a JSON schema — that is the prototype
this spike forbids.

**Option C — persist the full per-session transition history.** Richest.
Already bounded in memory at `MAX_TRANSITIONS_PER_SESSION`. Costs
everything B costs plus a larger file. Judge its privacy delta honestly
against the correction in "Current state" (the transition list and `cwd`
are already on the IPC wire), and judge whether the extra fidelity serves
your Step 3 primary intent or is just more data.

End with `**Recommended data source**: <A|B|C>` on its own line, plus one
paragraph of reasoning naming the option's main downside. If the honest
answer is "A is enough for the primary intent", say that — the cheapest
option winning is a good spike outcome.

**Verify**: `grep -c '^\*\*Recommended data source\*\*: ' docs/design/agent-activity-digest.md` → `1`

### Step 5: Compare the surface options

Under "Surface options", cover **exactly these three** (do not add a
fourth; if you think of one, it belongs in "Questions for the operator").
Each needs one line stating its IPC cost — including option 2:

1. **A new Settings section**, or an extension of
   `src/settings/sections/HistorySection.tsx`. Cheapest to build, but you
   must go looking for it. State whether it needs a new invoke command or
   can reuse `get_history` **given the 200-record cap**.
2. **A pushed card** at a scheduled time. Reuses the whole existing
   notification path (so: no new IPC), but a summary is a poor fit for a
   card that shows for seconds, and it collides with the Silent Period
   (default `00:00–10:00`, enabled by default). State how it would
   interact.
3. **An Agent Board / idle-surface panel.** The overlay is receive-only:
   `capabilities/default.json` grants only event listen/unlisten and must
   never change, so this must be fed by a Rust-published event, never an
   invoke.

End with `**Recommended surface**: <1|2|3>` on its own line.

**Verify**: `grep -c '^\*\*Recommended surface\*\*: ' docs/design/agent-activity-digest.md` → `1`

### Step 6: Thin slice, questions, closing section, and index

1. **Thin first slice** — the smallest thing delivering the Step 3
   primary intent, as **3–6 numbered steps** a future plan could be
   written from. Shippable on its own; must not require both B and C.
2. **Questions for the operator** — a numbered list of **exactly 4 to 6**,
   each answerable with a single choice. Include at minimum: retention
   (how long does agent activity persist?), default (on or off — note
   `history_enabled` ships `false`), and whether project paths are
   acceptable in a persisted digest given `cwd` is already published to
   the overlay today. **Every judgement you could not ground in the repo
   goes here** — state the assumption it rests on and mark it
   operator-confirmable.
3. **What this spike does not decide** — no code changed; the build needs
   its own plan after the operator answers.
4. **Index it**: `plans/README.md` already contains a note reading "153
   and 154 change **no application code** — each produces a decision
   document…". **Extend that existing sentence** with a pointer to
   `docs/design/agent-activity-digest.md`, the recommended option, and
   "awaiting operator decision". Do not add a competing section.

**Verify**: all of
- `grep -c 'agent-activity-digest' plans/README.md` → at least `1`
- `grep -cE '^\s*```(rust|ts|tsx)' docs/design/agent-activity-digest.md` → `0`
- `cd src-tauri && cargo test --locked` → `0 failed`, pass count equals Step 0's
- `npx vitest run` → pass count equals Step 0's

## Test plan

No application code changes, so no new application tests. Verification is
that the suites are **unchanged** against the Step 0 baselines. Record
both baselines and both final counts in your report. (Reporting them in
your report is fine — the "counts live only in `docs/TESTING_STRATEGY.md`
§0" rule governs committed files, and you are committing neither.)

## Done criteria

ALL must hold:

- [ ] `docs/design/agent-activity-digest.md` exists
- [ ] `grep -c '^## ' docs/design/agent-activity-digest.md` → `7`
- [ ] It records real measurements from the live history file, **or** opens with a `> Grounding unavailable:` line
- [ ] `grep -c '/Users/chetanjain' docs/design/agent-activity-digest.md` → `0`
- [ ] `grep -cE '^\s*```(rust|ts|tsx)' docs/design/agent-activity-digest.md` → `0`
- [ ] `grep -c '^\*\*Primary intent\*\*: ' docs/design/agent-activity-digest.md` → `1`
- [ ] `grep -c '^\*\*Recommended data source\*\*: ' docs/design/agent-activity-digest.md` → `1`
- [ ] `grep -c '^\*\*Recommended surface\*\*: ' docs/design/agent-activity-digest.md` → `1`
- [ ] The data-source table has all seven columns and three rows, no cell reading "TBD": `grep -c 'TBD' docs/design/agent-activity-digest.md` → `0`
- [ ] The Questions section has 4–6 numbered entries; the thin slice has 3–6 numbered steps (count by eye)
- [ ] `git status --porcelain -- src src-tauri` → **no output** (catches untracked new files, which `git diff` would not)
- [ ] `git status --porcelain` lists only `docs/design/agent-activity-digest.md` and `plans/README.md`
- [ ] `wc -l ~/.config/notchtap/history.jsonl` is **greater than or equal to** your Step 1 number (the app is running with history enabled, so it may legitimately grow — a *smaller* number means something truncated it and is a failure)
- [ ] `cd src-tauri && cargo test --locked` pass count matches Step 0
- [ ] `npx vitest run` pass count matches Step 0
- [ ] `plans/README.md` updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the
  excerpts — especially `history.rs`'s single-writer claim, the 200-record
  cap at `settings.rs:1030`, or `registry.rs`'s eviction `retain`.
  **The two known doc/code conflicts listed in "Current state" are NOT
  drift** — do not stop on those.
- The baseline suites in Step 0 do not pass.
- You conclude the digest requires the overlay to *invoke* something, or
  requires changing `capabilities/default.json`. Both are hard
  boundaries; report rather than designing around them.
- You are tempted to write a struct, a migration, a test, or a scratch
  script "to prove it works". That is the build plan, not this spike.
- A step's verification fails twice after a reasonable fix attempt.

Note: `plans/README.md` is already modified in the working tree and plans
152/153/155 are untracked when you start. Expected, not drift.

## Maintenance notes

- **The assumption most likely to be wrong later** is that Agent events
  reaching history is sufficient (option A). It depends on
  `informational_notifications` **and** on the 200-record read cap, both
  of which move. A digest built on A must say so in the UI, not just in
  the code.
- **If option B or C is chosen**, the write point at `registry.rs:325-333`
  is inside `AgentRegistry::tick`, which runs on a 5-second interval and
  holds the registry lock. A synchronous file write there is the obvious
  wrong answer; flag it for whoever writes the build plan.
- **Deliberately not in this spike**: any form of acting on agent
  activity (approving, replying, re-running). `ARCHITECTURE.md` §20 locks
  notchtap as heads-up only.
- **A reviewer should scrutinise**: that the privacy rule in Step 1 was
  actually followed — read the document for anything resembling a real
  project name or path, and check the executor's reported commands for
  any that would have printed values.
