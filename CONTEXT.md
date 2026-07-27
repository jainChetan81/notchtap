# notchtap — ubiquitous language

glossary only. no implementation details — those live in
`docs/V3_6_TECHNICAL_SPEC.md` / `docs/V5_TECHNICAL_SPEC.md` /
`docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` (the v1/v2/v3
equivalents shipped; their specs were removed at repo close-out
(2026-07-23), retrievable via `git log -- docs/archive/`). decisions live
in `docs/ARCHITECTURE.md`.

## terms

- **Event** — one incoming push (title + body, plus type/priority/
  rotation assigned by the engine). the unit that flows through the
  system.
- **Notification** — an Event being (or waiting to be) displayed.
  every Notification is an Event; "Notification" is the word for the
  display-side view of it.
- **Slot** — the single Visible position (v3.6; replaces the 3-item
  "Visible... ordered as a stack" model). there is never more than one
  Notification on screen at a time.
- **Visible** — the Notification currently occupying the Slot, if any
  (at most one, see **Slot**).
- **Waiting** — Notifications accepted but not yet shown, ordered
  **within their own Priority tier** by Rotation Order first and
  arrival order (FIFO) as the tie-break (v3.6: Low/Medium/High are
  three separate lines, not one; v6 added Rotation Order ahead of pure
  FIFO). capped per tier (`max_queued_per_tier`); pushes beyond a
  tier's own cap are rejected, independent of the other two tiers.
- **Priority** — `Low | Medium | High` on every Event (v3.6),
  independent of `EventType` — not every high-priority thing is a
  score. governs Promotion order and **Preemption**: higher-priority
  Waiting items are promoted next, and a strictly-higher-priority
  arrival cuts the currently-Visible item short — the preempted card
  re-queues at the head of its own tier with its remaining turn
  intact, and shows again once every higher-priority card has
  finished. equal priority never preempts; it waits its turn.
  (pre-silence contract was no-interruption ever; rewritten with the
  Silenced work, 2026-07-27.)
- **Origin** — which source category produced an Event (v6): `Football |
  News | Manual | Agent | Weather`. orthogonal to Priority and `EventType` —
  a source's Origin
  never changes, but its Priority is user-configurable per source. the
  only thing Origin governs is Rotation Order.
- **Agent Runtime** — the coding agent that produced an Event whose Origin
  is **Agent**, initially `Claude Code | Codex | Kimi | OpenCode`. Runtime
  identifies the producer for presentation and runtime-specific policy; it
  does not create a separate Origin or Rotation Order category.
- **Agent Adapter** — the heads-up-only bridge from one Agent Runtime's
  lifecycle hooks into notchtap. an Agent Adapter translates the runtime's
  native event into an Agent Event; notchtap does not launch, supervise, or
  scrape the runtime, and the adapter never answers on the user's behalf.
- **Agent Adapter Capability** — one kind of structured information or
  behavior an Agent Adapter can provide, such as completion, failure,
  permission requests, progress, tool details, subagents, or opening the
  originating session. adapters declare their capabilities; partial support
  is explicit, and unsupported information is omitted rather than invented.
- **Agent Event Kind** — the runtime-independent meaning assigned by an
  Agent Adapter: `Permission Requested | Input Required | Completed |
  Failed | Informational`. runtime-native event names remain diagnostic
  detail, not presentation branches.
- **Agent Session** — one active coding-agent session reported by an Agent
  Adapter. its continuously updated state can appear on the idle surface
  without producing a Notification for every progress tick; only noteworthy
  Agent Events enter the Slot. every session has an independent identity and
  history, even when multiple sessions share an Agent Runtime or project;
  histories are never merged. identity is the Agent Runtime plus its native
  session identifier; an adapter may provide a degraded process/start-time
  fallback, but a project path alone is never a session identity.
- **Agent Session State** — the runtime-independent lifecycle of an Agent
  Session: `Starting | Working | Waiting For Permission | Waiting For Input |
  Completed | Failed | Stale`. adapters translate native lifecycle names into
  these states. Informational is an Agent Event Kind, not a Session State;
  Stale means reporting ended without a clean terminal event.
- **Agent Registry** — the in-process store of every live and recently
  terminal Agent Session, keyed by Agent Runtime plus native session
  identity. it applies Agent Events to advance Session State, enforces
  Agent Session Order and Terminal Retention, and is the source the Agent
  Board and Origin::Agent Notifications both read from; it updates
  independently of whether the corresponding Notification can enter the
  Slot.
- **Adapter Health** — the presented status of one Agent Adapter:
  available, partial, or unavailable, alongside its declared Agent Adapter
  Capabilities, last-accepted-event time, and any compatibility note. it
  reflects whether the adapter is actually delivering events, not whether
  the underlying Agent Runtime happens to be running.
- **Agent Host** — the application presenting an Agent Session, such as T3
  Code, a terminal, or an IDE. Host is optional presentation and open/focus
  metadata; it is not part of Agent Session identity or an Origin.
- **Agent Session Order** — urgency first, then arrival order among sessions
  at equal urgency. a session needing input or permission ranks ahead of a
  passive Working session; equal-urgency sessions remain FIFO.
- **Terminal Retention** — how long a terminal Agent Session remains on the
  Agent Board before moving to its history: configurable, default 10 minutes.
  Failed and Stale rank ahead of Completed during this grace period. Waiting
  states do not expire as ordinary Notifications; they remain until the
  runtime reports a new state or the session becomes Stale.
- **Agent Board** — the idle presentation of active Agent Sessions. its
  resting state shows the highest-ranked session richly and represents other
  sessions individually; hover/expand grows it into a screen-bounded,
  scrollable list of every active session. noteworthy Agent Events still use
  the Slot, after which presentation returns to the Agent Board.
- **Rotation Order** — the configured tie-break (v6) among Waiting
  Notifications that share a Priority tier: a ranking over Origin,
  checked before arrival order. it never overrides Priority — a
  higher-Priority arrival still promotes ahead of a lower-Priority one
  regardless of Rotation Order.
- **Promotion** — the moment the highest-priority Waiting Notification
  moves into the Slot. the engine's decision alone; the frontend never
  promotes.
- **Engine** — the one module through which every change to the Slot
  and the Waiting lines flows (plan 037 — landed 2026-07-19 as
  `src-tauri/src/engine.rs`: the queue, the wake, and the live-match
  handle are private to it, so its guarantees are structural, not a
  convention spread across code paths). only the Engine
  Promotes; a change applied through it can never miss a Rotation
  deadline or fail to publish the resulting Slot change to the overlay
  — by construction, not by discipline. accepted Events reach
  Connectors through it, and it enforces the Connector rule that News
  never leaves the machine (see **Connector**).
- **Rotation** — how long a Notification stays Visible, measured from
  Promotion (not from arrival); replaces the old TTL concept (v3.6).
  extended (see **Expanded**) while the Slot is grown. config file keys
  retain `*ttl*` names for file compatibility; the domain term is
  Rotation.
- **Recurring** — a Rotation kind that requeues to the back of its own
  Priority tier's Waiting line after its turn, instead of being
  dropped (v3.6). bounded by supersession or the underlying state
  naturally ending, not a clock. the alternative kind, one-shot, is
  today's plain drop-forever-after-Rotation behaviour.
- **Topic** — the supersession identity carried by a Recurring Event
  (v3.6). a fresh Event sharing a Topic updates the existing
  Notification in place — Waiting or Visible — rather than adding a
  new one; a Visible supersede can grant a small, capped Rotation
  extension if remaining time was already low, but never mutates when
  it was first promoted.
- **Expanded** — a Slot's optional grown state (v3.6; plan 033 made it
  universal, the Silenced work carved out two exceptions): a Medium or
  High Promotion starts Expanded and auto-collapses at half the base
  Rotation window; a Low Promotion and a Breakthrough Promotion start
  compact (the manual expand hotkey still grows either on demand) — the grown first
  half of the turn is display-only and never extends the Rotation. only
  a manual expand (global hotkey) extends the Rotation window, and any
  hotkey press disarms the auto-collapse — a press on an auto-Expanded
  card collapses it.
- **Paused** — engine state in which Promotion is disabled. pushes are
  still accepted and buffered into Waiting (caller is told the app is
  paused); an already-Visible Notification finishes its natural
  Rotation and exits. Resuming re-enables Promotion immediately;
  nothing is dropped. (v3.6: gates the single Slot, same contract,
  formerly gated a 3-item cap. v5: the tray toggle stays session-only,
  but the persisted `start_paused` config flag — the **Kill Switch** —
  makes the app *launch* Paused.)
- **Silenced** — engine state in which Promotion is priority-gated:
  Medium/Low pushes buffer into Waiting exactly as under Paused, but a
  High Event still promotes (**Breakthrough**). display-layer only —
  Connectors are untouched, pollers keep running, and the overlay's
  idle surface (clock, weather, Agent Board) behaves as normal. ends by
  schedule, timer, or Skip; the backlog then drains under normal
  Rotation. distinct from Paused, which is absolute and sits above
  Silenced — a Paused engine shows nothing, Breakthrough included.
- **Silent Period** — a Silenced span that starts and ends on the
  daily schedule (default 00:00–10:00, one window per day, local wall
  clock). **Skip** — the tray action that ends today's Silent Period
  early; the schedule re-arms at the next window start.
- **Timed Mute** — a Silenced span started manually from the tray with
  a fixed duration (the "meeting mode": preset lengths, auto-resume
  when the timer ends). independent of the Silent Period; overlapping
  silences union — the engine is Silenced until the last one ends.
- **Breakthrough** — the promotion of a High-priority Event while
  Silenced. any High Event qualifies, whatever its Origin; the card
  renders compact (no auto-Expanded opening), and after its Rotation
  ends the engine returns to Silenced.
- **Polling Pause** — historical: a Poller-level state (per source) in
  which the Poller stopped checking its external service; no Events
  were produced and changes during the pause were never seen. distinct
  from Paused: Paused buffers and drops nothing, a Polling Pause
  observed nothing. (v6: no longer tray-toggleable — set once at boot
  from `espn_enabled`/`rss_enabled`; per-source control lives entirely
  in the Settings Window now. 2026-07-18, plan 019: the runtime
  pause/resume gate machinery — unreachable in production since v6
  made this boot-only — was deleted; a source now either spawns at
  boot or doesn't, and there is no live pause/resume or
  re-baseline-on-resume left to describe.)
- **Presentation Mode** — how the window anchors: **Notch** (over the
  macbook's notch cutout) or **HUD** (floating top-center, on
  notchless machines). decided at runtime, never at build time.
- **Settings Window** — the second webview window (v5), opened from
  the tray, where config and secrets are edited. the one window
  allowed to invoke commands into the engine; the overlay never is.
  saving always relaunches the app — there is no hot-reload.
- **notchtap** — the product: the always-on engine + overlay app, and
  the name of the CLI that pushes to it.
- **notchtap-detect** — the standalone swift helper that reports
  screen safe-area geometry so the engine can pick a Presentation
  Mode.
- **Relay** — an external tool forwarding its own notifications into
  notchtap. a Relay is heads-up only: it can never answer back into the
  tool that raised the alert.
- **Connector** — an outbound sink that would receive every accepted
  Event *except News items, which are overlay-only by design* — see
  `IMPLEMENTATION_PLAN.md` §4.6 — and forward the rest off the
  machine, best-effort. a Connector observes acceptance, not
  Promotion: the queue's display rules (cap, Rotation, Paused) never
  apply to it, and its failures never affect the pusher's response.
  telegram shipped as the first (and, so far, only) Connector in v3,
  then was removed 2026-07-27 by operator decision; the generic
  `ConnectorHandle` fan-out framework remains with zero connectors,
  kept for plan 128's Tavily connector.
- **Notifier** — the outbound half of notchtap as a whole: the seam
  through which accepted Events leave the machine. Connectors are its
  members; the overlay is not one. a seam, not a code interface —
  earlier drafts said "the Notifier trait," but no trait exists (and
  none is needed until a second Connector does).
- **Poller** — an internal event source that repeatedly checks an
  external service (espn in v2) and turns observed *changes* into
  Events. a Poller emits deltas only: the first sighting of a match is
  silent, and repetition of an unchanged fact never produces an Event.
- **Score Update** — an Event produced when a watched match's score
  changes (a goal).
- **Match State** — an Event produced when a watched match's phase
  changes: kickoff, half-time, full-time (and cards, where reported).
