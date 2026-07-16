## review log — v2 §2.1 espn poller (commit c1b1a55) — 2026-07-16

**setup**: 2 reviewers via PAL `consensus`. the skill's pinned pair
(`moonshotai/kimi-k2.7-code` for, `z-ai/glm-5.2` against) did **not
resolve** in the current registry (confirmed via `listmodels`; same
staleness the docs-review skill hit earlier today). substituted:
`openai/gpt-5.1-codex` (for) + `google/gemini-2.5-pro` (against) —
distinct from each other and from the executor (claude-fable-5).

### round 1

**executor's diff**: commit c1b1a55 as landed — new
`src-tauri/src/poller.rs` (tolerant serde structs verified against 5
captured fixtures; pure `diff_scoreboard(prev, fetched, ttl) ->
(events, next_snapshot)` with first-sighting-silent, score→ScoreUpdate
w/ scorer text, kickoff/HT/FT/cards→MatchState, eviction by
construction; per-league `Backoff` 30s→cap 300s; thin untested reqwest
fetch loop), config gains `espn_enabled`/`espn_leagues`/
`espn_poll_secs`, lib.rs config-gated spawn, vitest worktree exclude,
doc updates. 52 rust + 8 vitest tests green. full poller.rs and
neighbours attached to reviewers; executor pre-flagged six concerns
(a–f) including the transient-empty-feed eviction hole and missing
league label.

**reviewer 1 (openai/gpt-5.1-codex, for)**: **approve** (confidence
8/10) — "the poller meets the v2 §3 contract with safe failure
handling and well-tested diff logic; no blocking or panic risks."
&mut-across-await benign; error boundaries match CLAUDE.md; 13 tests
give strong regression protection; all six executor concerns rated
acceptable iterate-later trade-offs for a personal tool (incl.
transient-empty-feed re-baseline and unbounded `.text()`).

**reviewer 2 (google/gemini-2.5-pro, against)**: **needs-changes**
(confidence 9/10) — implementation "excellent… exemplary separation",
but two targeted changes strongly recommended before approval:
(1) **eviction robustness**: evict only on explicit `"post"` state,
never on mere absence from a poll — the current evict-if-absent makes
a transient empty-but-200 espn response silently drop live matches and
lose their in-window events (executor's concern f, promoted from
trade-off to required change); (2) **league context in the title**
(e.g. `"eng.1: ARS 1–0 CHE"`) to resolve same-abbreviation ambiguity
(concern d). VAR-correction and shootout semantics confirmed as
acceptable feature gaps, not bugs; unbounded `.text()` noted as a
minor dos vector.

**disagreement surfaced**: yes — approve (gpt-5.1-codex) vs
needs-changes (gemini-2.5-pro). the requested changes are not
mutually exclusive with the approve verdict; the disagreement is
purely about whether they gate the merge. handed to the user with the
executor's recommendation: apply both (cheap, and the eviction one
closes a real silent-loss hole the executor had flagged himself), with
absence-eviction bounded by a small consecutive-miss threshold rather
than kept forever (a match espn drops permanently — postponement —
must not pin the snapshot).

**user decision**: apply both fixes (the executor's recommendation) —
sided with gemini-2.5-pro's needs-changes on both points.

**model substitutions**: `moonshotai/kimi-k2.7-code` →
`openai/gpt-5.1-codex`; `z-ai/glm-5.2` → `google/gemini-2.5-pro`
(pinned slugs absent from PAL registry). stances preserved.

**action taken**: (1) eviction tightened — `MatchSnapshot` gains a
consecutive-miss counter; absent matches are carried forward and
evicted only after 10 straight misses (~5 min), with a `tracing::warn`
on eviction; explicit `"post"` still evicts immediately. a goal scored
during a feed blip now diffs against the carried snapshot on
reappearance (covered by a dedicated test). (2) titles league-tagged
via `league_label` (EPL / UCL / La Liga, slug fallback):
`"EPL: LIV 2–1 MUN"`. `diff_scoreboard` gains a `league` param; spec
§3 and TESTING §4.7 updated. tests 52 → 54, all green, zero warnings.
the accepted trade-offs (VAR correction body, shootout semantics,
unbounded `.text()`) remain recorded as such.

reviewed by: openai/gpt-5.1-codex (for) and google/gemini-2.5-pro
(against) — both completed; substitutions as recorded above.
