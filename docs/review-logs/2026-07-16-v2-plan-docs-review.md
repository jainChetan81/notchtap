## docs-review log — v2 plan docs (V2_TECHNICAL_SPEC + ARCHITECTURE §16 + IMPLEMENTATION_PLAN §2) — 2026-07-16

**setup**: 2 reviewers via PAL `consensus`. the skill's pinned pair
(`openai/gpt-5.6-sol` for, `anthropic/claude-sonnet-5` against) did
**not resolve** in the registry (confirmed via `listmodels` — the
staleness the skill's own open-question note anticipated). substituted
nearest same-family equivalents, same stances: `openai/gpt-5.2` (for)
+ `anthropic/claude-opus-4.5` (against). reviewer models differ from
each other and from the executor (claude-fable-5); no same-model
warnings.

### round 1

**executor's document**: the v2 planning set as committed at `3e548fb`
— `docs/V2_TECHNICAL_SPEC.md` (v0 draft: poller with pure
`diff_scoreboard`, EventType gains ScoreUpdate/MatchState, wire
payload gains eventType, css-only animation table, config additions,
3 hardening fixes, testing crosswalk), ARCHITECTURE.md §16 (locked v2
decisions), IMPLEMENTATION_PLAN.md §2 (build order 2.0→2.3→2.1, cmux
marked verified), CONTEXT.md glossary additions. full V2 spec inlined
verbatim in the reviewer prompt (lesson from the v1 implementation
review, where `relevant_files` didn't reach one reviewer); files also
attached. executor's own pre-review findings: TESTING_STRATEGY.md not
updated to carry the crosswalk's promised cases; snapshot eviction
unspecified; espn-on-by-default upgrade behavior unstated; backoff
scope ambiguous.

**reviewer 1 (openai/gpt-5.2, for)**: **needs-changes** (confidence
8/10). top reasons: (1) testing docs internally inconsistent — v2
crosswalk cites a "new §4.5 case" absent from TESTING_STRATEGY.md, and
§4.8 still plans rust unit tests for env-var parsing that the
flags-only shell cli made obsolete; (2) "animation table fully
testable with notchtap/curl" contradicts the generic-only `/notify`
contract — no way to hand-produce score_update/match_state types;
suggested a dev-only injection path as one option; (3) lib.rs vs
main.rs entrypoint drift between v1 spec layout and v2 spec. also
flagged: backoff global-vs-per-league ambiguity, snapshot eviction
missing, espn_enabled=true = new background network behavior on
upgrade, deterministic delta ordering unspecified.

**reviewer 2 (anthropic/claude-opus-4.5, against)**: **needs-changes**
(confidence 8/10). top reason (labelled CRITICAL): the crosswalk's
"new §4.5 case" doesn't exist in TESTING_STRATEGY.md — implementation
would either skip the test (violating v2 exit criteria) or write it
ad-hoc. minor: the "§2.1's first implementation step" cross-reference
is confusing; debug_assert guard is dev-only (no production
protection); espn_enabled=true default noted. explicitly endorsed:
scope discipline, security posture (receive-only preserved, no new
capabilities), backward-compatible wire change, cards-as-MatchState
pragmatism, observe-first fixture rule.

**disagreement surfaced**: no verdict disagreement — both
needs-changes with compatible change lists. one finding-level
disagreement: gpt-5.2 called the "testable with curl before espn"
claim false; opus-4.5 endorsed the build order as "testable without
ESPN". executor sided with gpt-5.2 (verifiably right: `/notify`
constructs EventType::Generic unconditionally), and rejected its
dev-injector suggestion as speculative for a personal tool — the doc
now states honestly that automated coverage is espn-free (vitest
synthesizes any eventType) but the visual eyeball waits for the
poller. on finding (3), the direction was inverted: the actual code
uses lib.rs (main.rs is a 2-line shim) — the *v1 spec* was stale, not
the v2 spec; fixed in V1_TECHNICAL_SPEC.md §1.

**user decision**: not needed — verdicts agreed.

**model substitutions**: `openai/gpt-5.6-sol` → `openai/gpt-5.2`
(pinned slug not in registry); `anthropic/claude-sonnet-5` →
`anthropic/claude-opus-4.5` (same). stances preserved.

**action taken**: all findings applied as doc edits —
TESTING_STRATEGY.md §4.5 gains the wall-clock deadline-sweep case,
§4.7 rewritten around pure `diff_scoreboard` with delta/eviction/
per-league-backoff cases, §4.8 rewritten as manual for the shell-cli
reality; V2_TECHNICAL_SPEC.md build-order rationale corrected
(automated vs visual testability split), per-league backoff +
snapshot eviction + deterministic ordering added to §3, upgrade-
behavior note added to §4, debug_assert dev-only caveat in §6.3,
§8 cross-reference clarified, crosswalk tense fixed;
V1_TECHNICAL_SPEC.md §1 layout corrected (main.rs shim / lib.rs
wiring).

reviewed by: openai/gpt-5.2 (for) and anthropic/claude-opus-4.5
(against) — both completed; substitutions from the skill's pinned
pair as recorded above.
