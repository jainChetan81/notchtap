# Implementation Plans

Two `improve` sessions have written plans here, both against commit
`d40445e` (2026-07-17):

- **Plans 001–003**: a `next`-invocation session (direction-only audit).
- **Plans 004–023**: a `deep`-invocation session (all nine audit
  categories: correctness, security, performance, tests, tech debt,
  dependencies, DX, docs, direction). Baseline verified green at planning
  time: `cargo test` 209 + 3 doc-tests, `npx vitest run` 60,
  `tsc --noEmit` clean.

Each executor: read the plan fully before starting, honor its STOP
conditions, run every verification gate, and update your row when done.
Plans that add/remove tests must update `docs/TESTING_STRATEGY.md` §0 —
counts live there and only there.

## Execution order & status

Recommended order below (dependencies + risk-ordering), not strict — most
plans are independent. P1s first.

| Plan | Title | Priority | Effort | Depends on | Status |
|------|-------|----------|--------|------------|--------|
| 004 | Docs truth pass — agent-facing docs/comments match shipped reality | P1 | M | — | TODO |
| 005 | Relocate + rotate the OpenRouter key in `opencode.json` | P1 | S | — | TODO |
| 006 | Redact telegram bot token from transport-error logs | P1 | S | — | TODO |
| 007 | Supply chain + CI: pin nspanel rev, `--locked`, audit scans, Linux web job, `sh -n` gate | P1 | S | — | TODO |
| 008 | Expanded semantics: auto-expand High, reset per item, idle no-op | P1 | S | — | TODO |
| 009 | Validate live `slot-state` payloads + pin the event-name seam | P1 | S | — | TODO |
| 010 | ESPN fetch hardening: gzip, 1 MiB cap, redirect limit, UA | P1 | S | — | TODO |
| 011 | RSS robustness: `fetch_feed` wiremock tests, bounded entity decoder, streaming cap | P1 | M | — | TODO |
| 012 | Open-story hardening: reap child, `open -u` normalized URL, tested scheme gate | P2 | S | — | TODO |
| 013 | Boot-path config validation (warn-and-continue) | P2 | S | — | TODO |
| 014 | Test the log-rotation engine + eval-splice escaping | P2 | S | — | TODO |
| 015 | Deadline-based heartbeat (replace the 250 ms tick) | P2 | M | 008, 009 | TODO |
| 016 | Frontend lint/format gate (Biome) | P2 | S | 007 (soft) | TODO |
| 023 | Goal celebration visible (review-log ranked list + redesign) | P2 | M | — | TODO |
| 018 | Overlay idle-cost cuts: lazy lottie, transform-based news shader | P2 | S | 023 (soft) | TODO |
| 022 | Deep-testing un-park decision + §9.1/§9.2 execution | P2 | L | 008; decision gate | TODO |
| 017 | Justfile (one-command local verification) | P3 | S | 007/016 (soft) | TODO |
| 019 | Dead code removal: presentation channel, polling gates, no-op dispatch, scaffold | P3 | M | 004 (soft) | TODO |
| 020 | Config defaults single-source (`get_default_config` invoke) | P3 | M | — | TODO |
| 021 | Settings save polish: feed metadata, duplicate rejection, port pre-flight | P3 | M | — | TODO |
| 001  | Wire the two "planned" global hotkeys (⌃⇧] skip, ⌃⇧, open settings) | P2 | S | — | TODO |
| 003  | Uptime Kuma → notchtap webhook recipe (docs only) | P3 | S | — | TODO |

## Done

Completed in this session (2026-07-17), filed with a `(done)` suffix:

- [`002-settings-animation-previews(done).md`](./002-settings-animation-previews(done).md) — Appearance section + static preview cards.
- [`004-test-notifications(done).md`](./004-test-notifications(done).md) — per-source test-notification buttons.
- [`005-appearance-config(done).md`](./005-appearance-config(done).md) — card scale/radius/opacity presets with hot-apply.

Status values: TODO | IN PROGRESS | DONE | BLOCKED (with one-line reason) | REJECTED (with one-line rationale)

## Dependency notes

- **004 first** — it corrects the project-state files every subsequent
  agent session reads. 014/017/019 touch some of the same doc lines;
  whoever lands second reconciles (noted in each plan).
- **015 after 008 and 009** — 008 changes `promote_next` (015 quotes it);
  009's seam pin should exist before the emit path is reworked.
- **022 after 008** — the property-test model must encode 008's expanded
  semantics; 022 also has a mandatory operator decision gate (execute vs
  re-park) at Step 0.
- **018 after 023** if both run — 023 may drop lottie entirely, mooting
  018's Step 1.
- **007 / 016 / 017** all touch `.github/workflows/ci.yml` or mirror it —
  any order, trivial textual merges; 017's recipes must mirror whatever
  CI runs at execution time.
- **012 vs 001** — both touch `lib.rs`'s shortcut area; textual-only
  interaction, reconcile by reading.
- **020 / 021 vs 002** — all touch `SettingsApp.tsx` in different
  regions; reconcile textually.
- Not planned but flagged MED-confidence in the deep audit:
  **slot-state emissions can be delivered out of order** (five emitters
  compute under the queue lock but emit after releasing it). Plan 015
  fixes the heartbeat's instance as a side effect; the mutation-site
  instances remain — investigate after 015 lands if blank/ghost cards
  are ever observed around rotation boundaries.

## Findings considered and rejected

From the deep session (2026-07-17) — recorded so they aren't re-audited:

- **queue.rs / settings.rs / SettingsApp.tsx size**: mostly tests or
  already-internally-layered; splitting adds files, not clarity.
- **lib.rs multi-responsibility split** (window/tray/shortcuts modules):
  real but M-effort refactor with hardware-sensitive code and no
  behavior payoff; deferred until it actually impedes a change.
- **EventMeta all-`Option` news fields on every Event**: documented as
  presentation-only accretion; revisit at a second meta-carrying source.
- **No `Notifier` trait**: explicitly recorded in CONTEXT.md as deferred
  until a second connector exists.
- **Dependency version lag**: none material — all deps current-generation
  for 2026 (react 19, vite 7, tauri 2.11, axum 0.8.9, etc.).
- **Unused deps / lucide-react authenticity / wiremock**: all verified
  fine.
- **`npm audit`**: 0 vulnerabilities at planning time (plan 007 adds the
  standing gate).
- **Pre-commit hooks**: CI + justfile (017) cover it with less machinery.
- **`.env.example`**: N/A — secrets deliberately live in `secrets.toml`,
  not env.
- **Advisory `min`/`max` props duplicating `validate` ranges**: accepted
  duplication (enforcement is server-side); a bounds-map export isn't
  worth the plumbing (see plan 020).
- **`Instant` deadlines freezing across system sleep**: judged
  working-as-intended (items resume remaining time at wake); document if
  it ever surprises.
- **Double tracing fmt layer in release / stdout with no consumer**:
  real but tiny; fold into any future logging.rs change.
- **500×300 window vs 270×38 idle pill compositing cost**: needs
  `powermetrics` measurement before any fix; not planned.
- **Notification titles in the plaintext log / log files not 0600**:
  accepted for a single-user machine; noted in plan 006's maintenance
  section.
- **Percent-encoding the ESPN league slug**: covered practically by
  plan 013's boot validation of the slug shape.
- **tauri-nspanel objc→objc2 upstream migration**: needs a network check;
  folded into plan 007's maintenance notes for the next deliberate bump.

From the earlier `next` session (kept from its index):

- **Generalize the `Notifier` seam for a second outbound connector**
  (e.g. ntfy/Pushover) — no doc names a specific wanted second connector
  today. Revisit if Telegram "proves insufficient" or a specific target
  is named.
- **Posture module (AirPods motion via `CMHeadphoneMotionManager`)** —
  weakest-grounded direction finding; needs a design spike defining the
  trigger heuristic before any build step.

Direction options surfaced in the deep session but not selected for
plans: **OpenRouter news enrichment** (the stored-but-unused key's first
feature — best-effort summary/category into `EventMeta`) and **a first
Recurring/Topic producer** (live-match scoreboard card superseding in
place — the supersession machinery currently has zero production
producers) and **a "what did I miss" history surface**. All grounded;
re-raise with `improve next` when wanted.

## What was not audited (deep session)

The Swift `notchtap-detect` source beyond structure; full git-history
secret scanning; live/hardware behavior (nspanel, notch geometry,
animation look — manual-checklist territory by the repo's own design);
Rust advisory database (cargo-audit not installed — plan 007 adds it);
upstream repo health for `tauri-nspanel`/`smappservice-rs` (needs
network).
