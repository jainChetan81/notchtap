# notchtap — ubiquitous language

glossary only. no implementation details — those live in
`docs/V3_6_TECHNICAL_SPEC.md` / `docs/V5_TECHNICAL_SPEC.md` (the v1/v2/v3
equivalents shipped and are archived at `docs/archive/`). decisions live
in `docs/ARCHITECTURE.md` and `docs/adr/`.

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
  score. governs Promotion order only: higher-priority Waiting items
  are promoted next, but a Priority arrival never interrupts the
  currently-Visible item — it always finishes its own turn.
- **Origin** — which source produced an Event (v6): `Football | News |
  Manual | Cmux`. orthogonal to Priority and `EventType` — a source's Origin
  never changes, but its Priority is user-configurable per source. the
  only thing Origin governs is Rotation Order.
- **Rotation Order** — the configured tie-break (v6) among Waiting
  Notifications that share a Priority tier: a ranking over Origin,
  checked before arrival order. it never overrides Priority — a
  higher-Priority arrival still promotes ahead of a lower-Priority one
  regardless of Rotation Order.
- **Promotion** — the moment the highest-priority Waiting Notification
  moves into the Slot. the engine's decision alone; the frontend never
  promotes.
- **Rotation** — how long a Notification stays Visible, measured from
  Promotion (not from arrival); replaces the old TTL concept (v3.6).
  extended (see **Expanded**) while the Slot is grown.
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
- **Expanded** — a Slot's optional grown state (v3.6): automatic for
  `High`-priority Notifications, manual (global hotkey) for everything
  else. never both triggers on the same item — the hotkey is a no-op
  while an automatically-Expanded `High` item is Visible.
- **Paused** — engine state in which Promotion is disabled. pushes are
  still accepted and buffered into Waiting (caller is told the app is
  paused); an already-Visible Notification finishes its natural
  Rotation and exits. Resuming re-enables Promotion immediately;
  nothing is dropped. (v3.6: gates the single Slot, same contract,
  formerly gated a 3-item cap. v5: the tray toggle stays session-only,
  but the persisted `start_paused` config flag — the **Kill Switch** —
  makes the app *launch* Paused.)
- **Polling Pause** — a Poller-level state (per source) in which the
  Poller stops checking its external service; no Events are produced
  and changes during the pause are never seen. distinct from Paused:
  Paused buffers and drops nothing, a Polling Pause observes nothing.
  resuming re-baselines silently, like a first sighting. (v6: no
  longer tray-toggleable — set once at boot from `espn_enabled`/
  `rss_enabled`; per-source control lives entirely in the Settings
  Window now.)
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
- **Relay** — an external tool (cmux in v2) forwarding its own
  notifications into notchtap. a Relay is heads-up only: it can never
  answer back into the tool that raised the alert.
- **Connector** — an outbound sink (telegram in v3) that receives
  every accepted Event and forwards it off the machine, best-effort.
  a Connector observes acceptance, not Promotion: the queue's display
  rules (cap, TTL, Paused) never apply to it, and its failures never
  affect the pusher's response.
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
