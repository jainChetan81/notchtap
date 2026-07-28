# Agent activity digest — data source and surface spike

**Status**: design spike (plan 154) — proposal only, zero code written.
**Researched against**: commit `acdaeb0` (2026-07-28). Every `file:line`
citation below was verified by reading that file at that commit; every
number in "Measurements" came from a key-projecting or aggregating
command run against the live `history.jsonl`, never from reading a
record.

## Measurements

Measured 2026-07-28 against the operator's live
`~/.config/notchtap/history.jsonl` (`history_enabled = true` in their
config; the shipped default is `false`, `config.rs:449-451`). Aggregates
and field names only — no titles, bodies, project names, or paths were
read out of the file, and none appear below.

**Volume and span**

| measure | value |
|---|---|
| total records | 2445 |
| file size | 1,841,078 bytes of the 5,242,880-byte rotation threshold (`history.rs:29`) |
| rotated backups present | none — rotation has never fired on this machine |
| oldest record | 2026-07-21 21:55 |
| newest record | 2026-07-28 08:17 |
| span | ~6.4 days |

**Origin split** (`event.origin`)

| origin | records | share |
|---|---|---|
| `news` | 2003 | 81.9% |
| `cmux` | 224 | 9.2% |
| `agent` | 149 | 6.1% |
| `manual` | 61 | 2.5% |
| `football` | 7 | 0.3% |
| `weather` | 1 | <0.1% |

`cmux` and `agent` are the same Origin on the wire — `SourceKind` carries
`#[serde(alias = "cmux")]` (`event.rs:106-107`), so pre-plan-137 lines
deserialize as `Agent`. Combined Agent share: **373 records, 15.3%**.

The split matters more than the sum. All 224 `cmux`-serialized records
carry **no `event.meta.agent` object at all** — no runtime, no session
hash, no kind. Records that are actually usable as Agent data begin at
the first `agent`-serialized line: **2026-07-27 14:33**, i.e. just under
**18 hours** of usable history at measurement time.

**Agent Runtime split** (`event.meta.agent.runtime`, agent+cmux rows)

| runtime | records |
|---|---|
| `claude-code` | 139 |
| `kimi` | 6 |
| `codex` | 2 |
| `opencode` | 2 |
| absent (the 224 cmux-era rows) | 224 |

**Agent Event kind split** (`event.meta.agent.kind`, `agent` rows only)

| kind | records |
|---|---|
| `completed` | 137 |
| `informational` | 8 |
| `permission_requested` | 3 |
| `failed` | 1 |

The 8 `informational` rows all landed inside a 140-second window on
2026-07-27 16:13, while the operator's config now reads
`informational_notifications = false`. Read as a brief flag flip, not as
steady-state coverage. Steady state is what the other three rows show:
completions dominate, permission requests are rare in the record even
though they are the highest-urgency thing the product renders.

**Which keys an Agent record actually populates** (of 373 agent+cmux rows)

| key path | populated |
|---|---|
| `recorded_at_ms`, `event.id`, `event.origin`, `event.event_type`, `event.priority`, `event.signal`, `event.rotation.kind`, `event.rotation.ttl_secs`, `event.payload.title`, `event.payload.body` | 373 (all) |
| `event.meta.subtitle` (project name) | 305 |
| `event.meta.agent.kind` / `.runtime` / `.sessionHash` / `.summary` | 149 each |
| `event.meta.details.0.*` (tool detail pairs) | **1** |

Two findings sit in that last table. First, `event.rotation.kind` is
`one_shot` on every Agent row, confirming the one-shot write gate
(`engine.rs:327-331`) never excludes Agent traffic. Second, the details
array — the tool/subagent colour a retrospective would most want — is
present on exactly **one** record in 373. Whatever a digest shows about
"what the session did", it will not come from `details`.

**The 200-record read window, measured**

`get_history` returns `read_recent(200)` (`settings.rs:1030`), and
`read_recent` reads only `history.jsonl`, never the `.1`/`.2` backups
(`history.rs:158-176`). Projecting the last 200 lines:

| measure | value |
|---|---|
| `agent` records in the window | 42 |
| `news` records in the window | 158 |
| window span | 2026-07-27 20:12 → 2026-07-28 08:17 |
| window duration | **12.1 hours** |

So the existing read surface, unchanged, would show a digest **half a
day** of Agent activity, 79% of its budget spent on News. This is the
single most load-bearing measurement in this document.

## Primary intent

**Primary intent**: at the end of a working day, see which Agent Sessions ran, on which projects and Agent Runtimes, and how each one ended.

Two intents were considered and rejected as primary.

*"What did I miss while I was away or Silenced?"* is rejected because
**no option on the table can answer it.** `HistoryEntry` is
`{recorded_at_ms, event}` (`history.rs:36-40`) and `Engine::accept`
writes at acceptance — not at promotion, not at dismissal, not at
expiry. Nothing anywhere records whether a card was actually *seen*.
Options B and C inherit the same blindness: the Agent Registry tracks
session state, not display outcome. Answering this intent requires a new
"was this displayed" field on the persisted record and a write at
promotion time, which is a strictly larger change than any option here.
It is a legitimate future feature; it is not what this spike can decide.

*"Is anything waiting on me right now?"* is rejected because the Agent
Board already answers it, richly and live, and does so as the overlay's
resting state (`ARCHITECTURE.md` §20, "Agent Board"). A digest that
re-answered it would duplicate a shipped surface with staler data.

## Data-source options

| option | what it can answer | what it cannot | code cost | new on-disk state | privacy delta | retention/migration questions |
|---|---|---|---|---|---|---|
| **A** — reuse `history.jsonl`, filter to Origin `Agent` | when a session ended, its Agent Runtime, its project (82% of rows), its outcome kind, its one-line summary | session duration; anything suppressed by `informational_notifications`; all board-only lifecycle; anything dropped by a full queue tier; anything older than the read window | smallest — a read-count change in `settings.rs` plus frontend grouping; **no new `#[tauri::command]`** | none | none — reads a file the settings window already reads | none new; inherits 5 MB size rotation and the "current file only" read, so "how many days" is a side effect of total notification volume, not a policy |
| **B** — a new bounded Agent Session journal, one line per session at eviction | everything A answers, plus duration, first/last state, transition count, host, subagent presence — and it covers sessions that never produced a card | still not "was it seen"; still nothing about tool output (never ingested) | medium — a write hook, a store, a config flag, a read path, and (if Settings is the surface) a new command plus the four-place allowlist change | yes, one file (or a new record kind inside the existing store) | small and arguably negative: it can persist strictly less per session than A does today, since A persists a free-text title and body | real and unavoidable: retention unit (days? sessions?), rotation policy, default on/off, and a first-write migration decision if it shares `history.jsonl` |
| **C** — B plus the full per-session transition list | everything B answers, plus the shape of the session over time (how long it sat in `WaitingForInput`, how many `Working` re-entries) | same blind spots as B | B's cost plus a materially larger file and a UI that has to summarise 50-entry arrays | yes, larger | **none over B** — the transition list and `project.cwd` are already published to the overlay on the live `agent-state` wire (`model.rs:347`, `board.rs:128`, consumed at `useAgentState.ts:53` and `:90`) | B's questions plus a per-session cap interacting with the file cap |

### Option A — reuse the existing history store

A is real coverage, not a fig leaf. Agent notifications are
`RotationSpec::OneShot` (`agents/notification.rs:235`), the write gate
accepts exactly one-shot events (`engine.rs:327-331`), and the
measurement confirms every Agent row in the live file is `one_shot`. The
kind split shows 137 of 149 usable rows are `completed` — which is to
say the file already contains, in substance, a list of the day's
finished sessions with Agent Runtime and project attached.

Three gaps, in descending order of how much they hurt the primary
intent.

**The read window is the binding constraint.** 200 records shared across
all Origins bought 12.1 hours on the measured file, and News alone took
158 of those 200 slots. "What happened today" needs a full day and
survives a busy News day. The fix is small and does not touch the
allowlist: `get_history` is already an allowed command
(`settings_commands.rs`, seventeen names, count asserted), so raising or
parameterising its `read_recent(200)` argument is an edit inside
`settings.rs`, not a new capability. It does need a judgement call about
payload size — the whole file is 1.8 MB, so an unbounded read is not
free, and an Origin filter or a since-timestamp argument is the
better-shaped version of the same change.

**There is no duration.** `recorded_at_ms` is the moment the completion
was accepted. Nothing records when the session started, so "Claude Code
ran 40 minutes on that project" is unavailable. A digest under A reads
as a timeline of endings, not a set of sessions.

**Coverage is conditional on notification policy.** `is_noteworthy`
(`agents/notification.rs:92-102`) gates `Informational` and non-terminal
`Failed` behind `policy.informational_notifications`, which the operator
has off; those events never become notifications and so never reach
history. A full queue tier is worse: `QueueError::QueueFull` returns
before the append (`http.rs:520-523`), so a permission request lost to a
busy queue is lost from the digest too — precisely the event most worth
recovering. Both gaps are silent. A digest built on A should say what it
is a digest *of* (accepted notifications), not imply completeness.

### Option B — a bounded Agent Session journal

The natural write point already exists and currently throws the data
away: `AgentRegistry::tick`'s `retain` (`registry.rs:325-333`) drops the
whole session once `terminal_at` is older than `terminal_retention`
(60 seconds, `registry.rs:24-26`). One line written just before that
drop turns a discarded in-memory session into a durable row.

Contents, in prose — deliberately not a struct, and deliberately a
subset of what the Registry holds: the session's own key hash (already a
non-reversible hash, `model.rs:160-170`), the Agent Runtime label, the
first-seen and terminal wall-clock stamps (from which duration is a
subtraction), the final state, a count of transitions, the sanitised
one-line summary, the project name, optionally the project working
directory, the host name, and whether a subagent was seen. Nothing else.
Illustratively: project name `example-project`, working directory
`/Users/example/code/example-project`, runtime `runtime-a`, session
`session-hash-abc`.

The spec already sanctions this shape. `V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
§9 says that when `history_enabled` is on, notchtap may "persist only the
normalized/sanitized session summary and bounded transitions **through
the existing history ownership boundary**", and forbids raw hook JSON,
prompts, model responses, tool stdout/stderr, environment variables,
secrets, complete shell commands, and Host launch data. B as described
stays inside that list. The phrase "through the existing history
ownership boundary" is a genuine steer away from a wholly separate file
and toward a second record kind inside `history.rs`'s store — which has
a compatibility wrinkle worth naming rather than deciding here: today's
reader `filter_map`s unparseable lines away (`history.rs:168-172`), so an
unrecognised record kind degrades to "skipped", not "crash", but the
existing settings History list would need to know to ignore it.

B's honest cost is not the write. It is that every one of the four
allowlist places (`settings_commands.rs` and its
`assert_eq!(SETTINGS_COMMANDS.len(), 17)`, `lib.rs`'s
`generate_handler!`, `capabilities/settings.json`, `build.rs`) has to
move together for a new read command, and that list is
security-load-bearing: `build.rs`'s opt-in is the only thing keeping
settings commands off the overlay window.

B's payoff is the one thing A structurally cannot give: sessions instead
of endings. Duration, final state, and "this session existed at all even
though it never produced a card" are session facts, and only a
session-shaped record holds them.

### Option C — persist the full transition list

C is B plus `AgentSession.history`, already bounded in memory at
`MAX_TRANSITIONS_PER_SESSION = 50` (`registry.rs:21`).

The privacy argument against C does not hold up, and this document
declines to make it. The bounded transition list is already on the live
`agent-state` wire (`model.rs:347`, typed frontend-side at
`useAgentState.ts:90`), as is `project.cwd` (`board.rs:128`,
`useAgentState.ts:53`). C therefore writes to disk what already crosses
IPC every publish. The delta over B is durability, not exposure — a real
consideration, but a different one, and one B shares.

The argument against C is proportionality. The primary intent is an
end-of-day retrospective: which sessions ran, where, and how they ended.
A 50-entry state-transition array per session answers a question nobody
in this repo has asked ("how many times did it re-enter `Working`?"),
multiplies file size by roughly the transition count, and hands the UI a
summarisation problem it would solve by collapsing the array back down
to the counts B already stores. C is the right shape for a debugging or
profiling feature. It is over-built for a digest.

**Recommended data source**: A

A already contains, for the primary intent, the substance of the answer:
137 of 149 usable Agent rows are completions carrying Agent Runtime and
project, written under a flag the operator has on. It needs no new file,
no new retention policy, no migration, no allowlist change, and — because
it persists nothing new — no new privacy decision. The cheapest option
genuinely clearing the bar is the correct spike outcome, and it also
de-risks B: shipping A first tells the operator whether a digest is a
thing they open twice a week or twice a year, before anyone funds a
journal.

Its main downside, stated plainly: **A cannot show how long anything
took, and it under-reports silently.** A session that finished is in the
file; a session that was suppressed by notification policy, or dropped by
a full queue tier, or never noteworthy at all, is simply absent with no
marker saying so. If the operator's answer to "what happened today" needs
durations, or needs to be trustworthy as a *complete* record rather than
a record of accepted notifications, A is the wrong foundation and B is
the answer — the write hook at `registry.rs:325-333` is waiting either
way.

## Surface options

### 1. A Settings section (new, or an extension of `HistorySection`)

IPC cost: **none, given the read-count change described under Option A.**
`get_history` is already one of the seventeen allowlisted commands
(`settings_commands.rs`), so an Agent-filtered, day-grouped view reuses
it. Widening the 200-record cap — or better, giving the command an
Origin filter or a since-timestamp argument — changes that command's
signature, not the allowlist, and the parity tests
(`settings_json_permissions_match_exactly`,
`generate_handler_registers_exactly_the_canonical_commands`) stay green
untouched. If instead the digest were built on Option B with its own
store, this is where the four-place change would land.

`HistorySection.tsx` already renders Origin-coloured rows and calls
itself "a plain scannable list, not a card renderer" (its own header
comment). A digest is a grouped, aggregated view — daily headers, per-runtime
and per-project counts — which is a different job. Extending that
component risks turning a deliberately plain list into a card renderer;
a sibling section reading the same command is the cleaner shape.

The cost of this surface is discoverability: the operator has to open
Settings and go looking. For an end-of-day retrospective that is
acceptable, and it is the only one of the three surfaces where "show me
yesterday too" is even expressible.

### 2. A pushed card on a schedule

IPC cost: **none.** It reuses the whole existing notification path — a
Rust-side scheduler builds an `Event` and calls `Engine::accept`, exactly
as the pollers do.

It is still the wrong surface. A digest is a list; a card holds a title,
a body, and a few detail pairs for a handful of seconds. Compressing a
day of sessions into that either loses the detail that makes it a digest
or produces a card nobody can read in time.

The Silent Period makes it worse. `[silence]` ships **enabled** with a
default window of `00:00–10:00` (`config.rs:617-619`, pinned by
`silence_defaults_to_enabled_with_the_overnight_window`). An end-of-day
digest naturally wants to fire late; late is inside the default silence
window. The digest would be silenced by default on a default install,
which is a confusing first-run experience. Any scheduled-card version
needs an explicit answer for what happens when its fire time falls inside
silence — deliver late, drop, or bypass — and "bypass" would make a
low-urgency summary the one thing that ignores a Silent Period, which is
backwards.

There is a narrow good version: a card as a *pointer* ("14 sessions today
across 3 projects") whose value is the prompt to go look, with the real
digest at surface 1. That is an add-on to surface 1, not an alternative.

### 3. An Agent Board / idle-surface panel

IPC cost: **must be a Rust-published event; an invoke is impossible.**
`capabilities/default.json` grants the `main` window exactly
`core:event:allow-listen` and `core:event:allow-unlisten` and must never
change (`ARCHITECTURE.md` §20, "overlay security remains locked"). So the
digest payload would ride an event alongside or inside `agent-state`
(`board.rs:26`, emitted at `board.rs:376`).

That is buildable, and it is the most beautiful version — the overlay is
where this product's visual quality lives. It is also the most expensive
and the most confusable. The Board's job is *live* sessions ranked by
urgency; a retrospective is a different mode, and mixing yesterday's
completed sessions into the surface that means "these need you now"
undermines the Board's one clear meaning. It would need its own trigger,
its own layout, and its own dismissal, all inside a receive-only window
that cannot ask for data on demand — every filter or date change becomes
a Rust-side republish.

**Recommended surface**: 1

## Thin first slice

Smallest shippable thing delivering the primary intent. Requires Option
A only; needs neither B nor C.

1. Give `get_history` a bounded, explicit read budget suited to a day —
   an Origin filter and/or a since-timestamp argument on the existing
   command (`settings.rs:1023-1037`), not a new command, with a hard
   upper bound so the payload stays sane against a 1.8 MB file.
2. Add an "Agent activity" section to the settings window that calls it,
   filtered to Origin `Agent`, grouped by calendar day (newest first).
3. Within each day, group by Agent Runtime and project (`event.meta.agent.runtime`,
   `event.meta.subtitle`), showing per-group counts by Agent Event kind —
   the four kinds measured above, with `completed` and
   `permission_requested` the ones worth eye-catching.
4. Render each row as time plus outcome plus the existing
   `event.meta.agent.summary`; reuse `HistorySection`'s Origin colour
   tokens so the two sections read as one system.
5. Gate the whole section on `history_enabled` and show an explicit
   empty state when it is off, saying that nothing is being recorded —
   the flag ships `false`, so a silent blank panel would be the default
   experience.
6. Put a one-line honesty note in the section itself: this lists accepted
   notifications, not every session; suppressed and queue-dropped events
   are not shown.

## Questions for the operator

1. **Is duration worth funding Option B?** The recommendation assumes
   "which sessions, where, how they ended" is enough, and that "how long
   each took" is a nice-to-have. If duration is actually the point of the
   feature, the recommendation flips to B. *(Assumption: the retrospective
   is about outcomes, not time budgeting. Operator-confirmable.)*
2. **How long should Agent activity persist?** Today the answer is an
   accident: the file rotates at 5 MB and reads only the current file, so
   depth is a by-product of total notification volume — currently ~35% of
   the threshold after 6.4 days, mostly News. Choose one: leave it
   volume-driven (cheapest); add a day-based prune; or, under B, give the
   journal its own retention. *(Assumption: nobody has yet needed
   more than a week of lookback. Operator-confirmable.)*
3. **On or off by default?** `history_enabled` ships `false`
   (`config.rs:449-451`) and you have it on. Should the digest section
   appear-but-empty when the flag is off, appear only when on, or should
   the digest ship with its own separate flag? *(Assumption: reuse
   `history_enabled` rather than add a flag. Operator-confirmable.)*
4. **Are project working directories acceptable in a persisted digest?**
   `project.cwd` is already published to the overlay every
   `agent-state` tick (`board.rs:128`), and the project *name* is already
   persisted today as `event.meta.subtitle` on 305 of 373 Agent rows. The
   open question is only whether the full path should additionally go to
   disk under Option B. Options: name only; name plus path; path behind
   its own flag. *(Assumption: name only is sufficient for a digest.
   Operator-confirmable.)*
5. **Should the digest count what it could not record?** A digest can
   silently omit policy-suppressed and queue-dropped events, or it can
   show a footer count of them — but the second needs a counter that does
   not exist today, and the queue-full path returns before any recording
   (`http.rs:520-523`). Silent omission plus a wording note is what the
   thin slice proposes. *(Assumption: an honest sentence beats a new
   counter. Operator-confirmable.)*
6. **Is a fourth surface wanted that this spike was scoped out of
   proposing** — a plain text or JSON export, or a CLI subcommand that
   prints the day's Agent activity to a terminal? The existing
   integrations are terminal-centric, so it is a plausible fit, but it was
   outside the three surfaces this spike was asked to compare.
   *(Assumption: GUI-first. Operator-confirmable.)*

## What this spike does not decide

No code was written and no application file was touched — this document
is the entire output. The rust and frontend suites were run before and
after and are unchanged.

Not decided here: whether the feature is built at all; the exact
argument shape of the widened `get_history` read; the section's visual
design; and, if the operator answers question 1 with "duration matters",
the record shape and retention policy of the Option B journal. Each of
those belongs in a build plan written after the questions above are
answered.

Deliberately out of scope, and worth stating so a future reader does not
mistake it for an oversight: the "what did I miss while Silenced"
intent. It needs a display-outcome field that no option here provides
(see "Primary intent"), and it should be planned as its own change to the
write path, not folded into a digest.
