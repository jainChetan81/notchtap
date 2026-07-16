## docs-review log — V1_TECHNICAL_SPEC.md implementation-readiness — 2026-07-16

**setup**: 2 reviewers via PAL `consensus`. Pinned slugs from the `consensus-research` plugin
(`anthropic/claude-sonnet-5`, `openai/gpt-5.6-sol`) do not exist in this PAL instance's
OpenRouter registry (confirmed via `listmodels` — newest available are `claude-sonnet-4.5`
and `gpt-5.2`/`gpt-5.2-pro`). User approved substituting nearest equivalents, same stance
assignment: `openai/gpt-5.2` (for) + `anthropic/claude-sonnet-4.5` (against).

### round 1

**executor's document**: `docs/V1_TECHNICAL_SPEC.md` (417 lines) — full text passed to both
reviewers via `relevant_files`, alongside `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md`,
`docs/TESTING_STRATEGY.md` for cross-reference. Reviewed for internal consistency with the
three upstream docs, sufficiency to start coding without further design decisions, and gaps
against `IMPLEMENTATION_PLAN.md` §1.5's v1 exit criteria.

**reviewer 1 (openai/gpt-5.2, for)**: **needs-changes** —
1. **Queue-ownership contradiction**: `ARCHITECTURE.md`'s system diagram (§2) places the
   notification queue inside the rust core engine; `IMPLEMENTATION_PLAN.md` §1.3 separately
   describes "notification queue: fifo, cap at 3, ttl-based auto-dismiss" under the *frontend*
   heading. `V1_TECHNICAL_SPEC.md` specified a full `NotificationQueue` in rust (§4) *and* a
   frontend queue hook with its own enter/hold/exit+ttl state (§7) without stating which one is
   authoritative — an authoritative-state conflict, not mere duplication.
2. **TTL semantics ambiguity**: `ARCHITECTURE.md` §3 defines ttl as "time from enter-complete to
   exit-start," but the spec's `QueueItem` only stores `enqueued_at`, leaving unclear whether the
   clock starts at raw enqueue (wrong per the locked definition — items waiting in the queue
   would lose ttl before ever being shown) or at promotion to visible.
3. **Missing v1 day-one background requirements**: `LSUIElement`, `SMAppService.mainApp.register()`
   login-item registration, and always-on-top are locked as v1 day-one in `ARCHITECTURE.md` §6 /
   `IMPLEMENTATION_PLAN.md` §4, but nowhere operationalized in the spec's file layout or signatures.
   Also flagged: `ttl_secs` (rust snake_case) vs `ttlSecs` (ts camelCase) has no stated serde
   rename; the HTTP layer drops `type`/`priority`/`ttl` entirely, leaving no internal
   validation function signature to exercise `TESTING_STRATEGY.md` §4.2's "unknown type" test
   case; and `oneshot`-based handler tests don't reconcile with §4.3's requirement for a
   dedicated loopback-bind-address test (`oneshot` never binds a real socket).

**reviewer 2 (anthropic/claude-sonnet-4.5, against)**: **needs-changes** —
1. **Missing v1 day-one background requirements** (same finding as reviewer 1, arrived at
   independently): zero mention of `LSUIElement`, `SMAppService` registration, or
   `setAlwaysOnTop` anywhere in the spec, despite `ARCHITECTURE.md` §6 locking all three as
   v1 day-one and `IMPLEMENTATION_PLAN.md` §4 explicitly resolving an earlier draft's attempt
   to defer them. Called this a **blocking** gap against the exit criterion "produces a visible
   animated notification" — a window with neither always-on-top nor login-item registration
   fails that criterion the moment another app takes focus or the machine restarts.
2. **`notchtap-detect` build/install unspecified**: the spec's own §14 punts this as "still
   genuinely open," but §1.2's exit criteria require the notch/hud check to actually work on
   both machines — meaning the binary must be built and on `PATH` *before* that phase's testing
   can happen. No `Package.swift` stub, build command, output path, or install method specified.
   This is a hard blocker, not a deferrable detail.
3. **macOS 13+ enforcement missing**: `ARCHITECTURE.md` §6 locks macOS 13+ (driven by
   `SMAppService` availability), but the spec never specifies build-time (`tauri.conf.json`) or
   runtime enforcement, or what happens on macOS 12.
   Also flagged (non-blocking): confusing wording around the CLI entrypoint ("not load-bearing"
   when the exit criteria actually require it working) and around `log_frontend_error` (called
   "the one exception" then immediately "not required" — should be added properly or cut).
   Notably did **not** flag the queue-ownership issue reviewer 1 raised — reviewer 2 validated
   both the rust queue (§4) and frontend queue (§7) against their respective
   `TESTING_STRATEGY.md` sections independently, without cross-checking for a single source of
   truth between them.

**disagreement surfaced**: yes — reviewer 1 found a queue-ownership authority conflict; reviewer
2 missed it entirely. I re-verified this myself against the actual file contents rather than
taking either reviewer's word for it: confirmed `ARCHITECTURE.md` §2's diagram places
"notification queue (fifo, max n concurrent, ttl per item)" inside the "core engine (rust)" box,
and `IMPLEMENTATION_PLAN.md` §1.3 separately lists "notification queue: fifo, cap at 3 concurrent
visible items, ttl-based auto-dismiss" under its "frontend" subsection. This is a real,
pre-existing tension in the upstream locked docs themselves (not introduced by the technical
spec) — the spec inherited it by fully specifying logic on both sides without resolving which
one governs. Reviewer 1's finding stands as confirmed, not just asserted.

**user decision**: not surfaced as a user choice — treated as a confirmed finding (independently
verified against source files) rather than an unresolved model disagreement, and fixed directly.
Both reviewers agreed on the verdict (needs-changes) and on 2 of 3 top-line issues, so no
conflicting-verdict escalation was needed per the skill's step 3.

**model substitutions**: `openai/gpt-5.2` for `openai/gpt-5.6-sol`; `anthropic/claude-sonnet-4.5`
for `anthropic/claude-sonnet-5` — pinned slugs don't exist in this registry (see setup above).
User approved before the round ran.

**action taken**: `docs/V1_TECHNICAL_SPEC.md` updated to address all 3 shared/confirmed findings
plus the 5 secondary points raised (see diff): resolved queue authority (rust is sole decision-maker,
frontend is a pure renderer keyed off ttl it's given — new explicit statement), added a
window/background-setup section (`Info.plist` merge for `LSUIElement`, `tauri-plugin-autostart`
for `SMAppService` registration, `.always_on_top(true)`, `tauri.conf.json`
`bundle.macOS.minimumSystemVersion` for macOS 13+ enforcement), expanded the `notchtap-detect`
section with a concrete build/install path, added an explicit `camelCase` serde rename for the
frontend-facing payload struct, added a `dispatch()` function signature so §4.2's "unknown type"
case is unit-testable directly, distinguished `oneshot` handler tests from a separate real-bind
loopback test, tightened the CLI entrypoint wording, and removed `log_frontend_error` from v1
scope (pushed to a "future" note) rather than leaving it half-specified. `docs/V1_TECHNICAL_SPEC.md`
§14 ("what's still open") trimmed to just the CLI implementation choice, which both reviewers
treated as genuinely non-blocking.
