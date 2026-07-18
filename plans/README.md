# Implementation Plans

**Reconciled 2026-07-18 at `b43a7ca` (advisor verify pass)**: plans
001–008 + the two extra-session plans (004-test-notifications,
005-appearance-config) were re-verified against HEAD — every claimed
implementation found in code, suites green (`cargo test` 225 + 3
doc-tests, `npx vitest run` 62). Two outstanding operator gates, neither
blocking: plan 001's manual real-keypress check, and plan 007's first
green CI run (workflow changes need a push). Plan 005's credential file
(`~/.config/opencode/openrouter.key`) confirmed mode `0600`; the
repo-root `opencode.json` holds only a `{file:…}` reference.
All TODO plans (009–023) were drift-checked: every finding still present
and every quoted code excerpt confirmed byte-identical — only line
numbers shifted from the 001/008/appearance merges (queue.rs promote_next
now ~206, open_current_story ~725, spawn_heartbeat ~655, saveConfig
rebuild ~1117). Plan 020's excerpts were refreshed in place for the
six-command build.rs. Plan 022's dependency (008) is now DONE — it is
unblocked except for its own Step 0 decision gate. Plan 015 still waits
on 009.

Two `improve` sessions wrote the original plans against commit `d40445e`
(2026-07-17). Plans 005 and 006 were cold-reviewed and refreshed against
`b1981c9` later that day; their files contain the current baselines and gates.

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
| 005 | Verify relocated OpenRouter key + complete operator rotation | P1 | S | — | DONE |
| 006 | Prevent telegram transport errors from logging the bot token | P1 | S | — | DONE |
| 007 | Supply chain + CI: pin nspanel rev, `--locked`, audit scans, Linux web job, `sh -n` gate | P1 | S | — | DONE (2026-07-18, CI run pending push) |
| 008 | Expanded semantics: auto-expand High, reset per item, idle no-op | P1 | S | — | DONE (`8ca01e3`, verified 2026-07-18 — rewritten from `b1981c9` to remove an unrelated plan-001 duplicate that leaked into that commit) |
| 009 | Validate live `slot-state` payloads + pin the event-name seam | P1 | S | — | DONE (`bb0f249`, implemented directly 2026-07-18 from `3c5cb90`; vitest 62→64, `cargo test` 234→235, tsc/clippy/fmt/build clean; file → `(done)`) |
| 010 | ESPN fetch hardening: gzip, 1 MiB cap, redirect limit, UA | P1 | S | — | DONE (`4381de4`, merged to master 2026-07-18; Step 4 GUI smoke owed to operator) |
| 011 | RSS robustness: `fetch_feed` wiremock tests, bounded entity decoder, streaming cap | P1 | M | — | DONE (`6b0bbc4`, merged to master 2026-07-18; `/improve execute` → reviewed APPROVE; file → `(done)`) |
| 012 | Open-story hardening: reap child, `open -u` normalized URL, tested scheme gate | P2 | S | — | DONE (`b7f58fd`, implemented directly 2026-07-18 from `586c943`; `cargo test` 234 + 3 doc-tests, clippy/fmt clean; Step 4 GUI smoke batched to end-of-run; file → `(done)`) |
| 013 | Boot-path config validation (warn-and-continue) | P2 | S | — | TODO |
| 014 | Test the log-rotation engine + eval-splice escaping | P2 | S | — | TODO |
| 015 | Deadline-based heartbeat (replace the 250 ms tick) | P2 | M | 009 | TODO |
| 016 | Frontend lint/format gate (Biome) | P2 | S | — | TODO |
| 023 | Goal celebration visible (review-log ranked list + redesign) | P2 | M | — | TODO |
| 018 | Overlay idle-cost cuts: lazy lottie, transform-based news shader | P2 | S | 023 (soft) | TODO |
| 022 | Deep-testing un-park decision + §9.1/§9.2 execution | P2 | L | decision gate | TODO |
| 017 | Justfile (one-command local verification) | P3 | S | 016 (soft) | TODO |
| 019 | Dead code removal: presentation channel, polling gates, no-op dispatch, scaffold | P3 | M | — | TODO |
| 020 | Config defaults single-source (`get_default_config` invoke) | P3 | M | — | TODO |
| 021 | Settings save polish: feed metadata, duplicate rejection, port pre-flight | P3 | M | — | TODO |

## Done

Completed in this session (2026-07-17), filed with a `(done)` suffix:

- [`001-wire-skip-and-open-settings-hotkeys(done).md`](./001-wire-skip-and-open-settings-hotkeys(done).md) — ⌃⇧] skip + ⌃⇧, open-settings hotkeys; code review approved, manual real-keypress verification pending.
- [`002-settings-animation-previews(done).md`](./002-settings-animation-previews(done).md) — Appearance section + static preview cards.
- [`003-kuma-webhook-recipe(done).md`](./003-kuma-webhook-recipe(done).md) — Uptime Kuma → notchtap webhook recipe (docs only); kuma-side verification not run, see `docs/recipes/kuma-webhook.md`'s status line.
- [`004-docs-truth-pass(done).md`](./004-docs-truth-pass(done).md) — agent-facing docs/comments synced to shipped reality (~14 stale claims fixed); executed via `/improve execute`, reviewed and merged into `master` at `0749235`.
- [`004-test-notifications(done).md`](./004-test-notifications(done).md) — per-source test-notification buttons.
- [`005-appearance-config(done).md`](./005-appearance-config(done).md) — card scale/radius/opacity presets with hot-apply.
- [`008-expanded-semantics-high-auto-expand(done).md`](./008-expanded-semantics-high-auto-expand(done).md) — auto-expand High at both promotion sites, per-item reset, idle no-op toggle; executed at `b1981c9`, rewritten to `8ca01e3` to strip a leaked plan-001 duplicate, done criteria re-verified 2026-07-18 (details in the plan's post-execution appendix).
- [`005-relocate-opencode-api-key(done).md`](./005-relocate-opencode-api-key(done).md) — OpenRouter key relocated, file mode locked down, key rotated, auth smoke check passed.
- [`006-redact-telegram-token-from-logs(done).md`](./006-redact-telegram-token-from-logs(done).md) — `reqwest::Error::without_url()` redaction at `notifier.rs:278` plus a production-path regression test, `cargo test` 225 + 3 doc-tests.
- [`007-supply-chain-and-ci-hardening(done).md`](./007-supply-chain-and-ci-hardening(done).md) — `tauri-nspanel` pinned to `rev` (dropped `branch`, which cargo rejects alongside `rev`), `--locked` on clippy/test, `rustsec/audit-check@v2.0.0` + `npm audit --audit-level=high`, web job moved to `ubuntu-latest`, `sh -n notchtap` gate added; all local gates green (`cargo test --locked` 225+3, `npx vitest run` 62, `npm audit` 0 vulns); one green CI run on push still pending.
- [`011-rss-robustness-and-fetch-feed-tests(done).md`](./011-rss-robustness-and-fetch-feed-tests(done).md) — `fetch_feed` wiremock characterization (304 / validator-persist ordering / size cap), `MAX_ENTITY_LEN`-bounded entity decoder, pre-truncated `sanitize`, and a streamed 1 MiB body cap replacing full buffering; executed 2026-07-18 via `/improve execute`, reviewed APPROVE, merged to master `6b0bbc4` (`cargo test` 232 + 3 doc-tests, `rss_poller` 21→28).
- [`012-open-story-hardening(done).md`](./012-open-story-hardening(done).md) — ⌃⇧O open-story: extracted the tested `openable_http_url` gate (full parse, http(s)-only, returns the normalized serialization), hand it to `open -u` so validated==executed, and reap the spawned child off-thread (no zombie per press); implemented directly 2026-07-18 at `b7f58fd` (`cargo test` 234 + 3 doc-tests, `lib` 6→8). Step 4 GUI smoke batched to end-of-run.
- [`009-validate-slot-state-event-path(done).md`](./009-validate-slot-state-event-path(done).md) — route live `slot-state` event payloads through `isValidSlotState` (not just the eval-planted global), `.catch` a dead listener registration, and pin the `SLOT_STATE_EVENT` name across the rust↔TS seam with a rust test; implemented directly 2026-07-18 at `3c5cb90` (vitest 62→64 slot-state hook 14→16, `cargo test` 234→235 event 17→18). Unblocks 015.

Status values: TODO | IN PROGRESS | DONE | BLOCKED (with one-line reason) | REJECTED (with one-line rationale)

## Dependency notes

- **005 relocation is already complete** — both OpenCode configs use an
  external file reference and the credential file is `0600`; do not move
  tooling config again. The remaining completion gate is operator-confirmed
  replacement-key use plus revocation of the old key.
- **006 has a red repository baseline outside its scope at `b1981c9`** — full
  fmt fails in `settings.rs`, and full clippy fails on four
  unrelated lints. Its reviewed plan uses targeted gates and must not absorb
  that cleanup. The Rust total changed concurrently while 006 was reviewed, so
  its count step records the clean execution baseline and increments it rather
  than hard-coding the original total.

- **004 first** — it corrects the project-state files every subsequent
  agent session reads. 014/017/019 touch some of the same doc lines;
  whoever lands second reconciles (noted in each plan).
- **015 after 009** — 008 is DONE (015's baseline already includes it);
  009's seam pin should exist before the emit path is reworked.
- **022 is blocked only by its Step 0 decision gate** — 008 is DONE; the
  property-test model's expanded invariants are already written into the
  plan. The operator must choose execute-§9 vs re-park.
- **018 after 023** if both run — 023 may drop lottie entirely, mooting
  018's Step 1.
- **016 / 017** touch `.github/workflows/ci.yml` or mirror it — 007
  already landed there (`--locked`, audit scans, `ubuntu-latest`,
  `sh -n`); both plans' texts account for it.
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
