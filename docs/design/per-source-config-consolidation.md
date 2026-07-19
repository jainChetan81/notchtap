# Per-source config consolidation — design spike

> **Status**: design proposal, not a build plan. Zero production code
> changes accompany this document.
>
> **Researched against**: commit `f2cbae6` (declared by plan 049 as its
> grounding commit; every `file:line` citation below was re-verified
> line-by-line against that tree, 2026-07-19).
>
> **Terminology**: follows `CONTEXT.md` — **Origin** (which source
> produced an Event, `CONTEXT.md:32-35`), **Poller** (an internal event
> source that repeatedly checks an external service, `CONTEXT.md:122-125`),
> **Connector** (an outbound sink configured globally, not per-source,
> `CONTEXT.md:111-116`).

## §0 Inventory: what is actually duplicated

`SourceKind` has exactly five variants — `Football`, `News`, `Manual`,
`Cmux`, `Weather` (`src-tauri/src/event.rs:83-89`). There is no sixth
source in the tree; the "no forcing function yet" premise holds.

`Config` is a single flat struct (`src-tauri/src/config.rs:7-88`). Its 25
per-source fields, tabulated per source:

| Source | Fields (config.rs lines) | Count |
|---|---|---|
| espn (Football) | `espn_enabled`, `espn_leagues`, `espn_poll_secs`, `espn_priority`, `espn_ttl_secs`, `espn_live_card` (`config.rs:17-30`) | 6 |
| rss (News) | `rss_enabled`, `rss_feeds`, `rss_poll_secs`, `rss_priority`, `rss_ttl_secs`, `rss_max_per_poll` (`config.rs:33-47`) | 6 |
| manual | `manual_default_priority` (`config.rs:51`) | 1 |
| cmux | `cmux_priority`, `cmux_ttl_secs` (`config.rs:58-62`) | 2 |
| weather | `weather_enabled` … `weather_priority` (`config.rs:65-81`) | 10 |

Which candidate "common" fields each source actually has:

| Field | espn | rss | manual | cmux | weather |
|---|---|---|---|---|---|
| `enabled` | ✓ | ✓ | — (push Origin, no Poller) | — | ✓ |
| `poll_secs` | ✓ | ✓ | — | — | ✓ |
| `priority` | ✓ | ✓ | ✓ (`manual_default_priority`) | ✓ | ✓ |
| `ttl_secs` | ✓ | ✓ | — (uses `default_ttl`, `settings.rs:605`) | ✓ | — (uses `default_ttl`, `settings.rs:621`) |

The only field all five Origins share is `priority`. `enabled` and
`poll_secs` exist only on the three **Poller**-backed sources; `ttl_secs`
on three of five. This is the honest version of "every source repeats the
same shape": the triad is real for Poller sources, thinner for push
Origins.

Genuinely source-specific fields: espn's `espn_leagues` +
`espn_live_card`, rss's `rss_feeds` + `rss_max_per_poll`, weather's
`weather_lat`/`weather_lon`/`weather_units`/`weather_rain_threshold_pct`/
`weather_rain_lookahead_mins`/`weather_temp_hot_c`/`weather_temp_cold_c`
— 11 of the 25 per-source fields.

The repeated scaffolding per field:

- **34** private `fn default_*()` free functions in `config.rs`
  (`rg -c "^fn default_" src-tauri/src/config.rs` → 34), of which 25 are
  per-source: espn ×6 (`config.rs:160-187`), rss ×6
  (`config.rs:189-195, 269-287`), manual ×1 (`config.rs:197-199`),
  cmux ×2 (`config.rs:201-207`), weather ×10 (`config.rs:209-247`).
- One manual line per field in `impl Default for Config`
  (`config.rs:309-347`; weather alone is `config.rs:332-341`).
- A per-source assertion block in the defaults test (weather's is
  `config.rs:433-442` inside `empty_toml_yields_all_defaults`,
  `config.rs:403-456`) plus a per-source TOML override test (weather's
  `weather_fields_are_overridable`, `config.rs:618-634`).
- A hand-written per-source block in `settings::validate`
  (`src-tauri/src/settings.rs:49-211`): espn `settings.rs:70-92`, rss
  `settings.rs:93-129` plus duplicate-feed rejection
  `settings.rs:169-179`, weather `settings.rs:131-168`.
- A dedicated settings-window section component per source:
  `FootballSection` (`src/settings/SettingsApp.tsx:698-769`),
  `NewsSection` (`SettingsApp.tsx:771-844`), `CmuxSection`
  (`SettingsApp.tsx:846-882`), `WeatherSection`
  (`SettingsApp.tsx:884-993`), wired in at `SettingsApp.tsx:1490-1511`;
  plus 25 per-source lines in the TypeScript mirror `interface Config`
  (`SettingsApp.tsx:41-65`).

Consumers that read these fields today (the migration blast radius):

- `lib.rs` destructures all 25 per-source fields into locals
  (`src-tauri/src/lib.rs:124-148`), passes the three `*_enabled` flags
  into `Engine::new` (`lib.rs:200-207`), and gates/launches the three
  pollers with the locals (`lib.rs:330-367`).
- Poller entry points take the fields as positional arguments:
  `spawn_espn_poller` (`src-tauri/src/poller.rs:725-732`),
  `spawn_rss_poller` (`src-tauri/src/rss_poller.rs:425-432`),
  `spawn_weather_poller` (`src-tauri/src/weather_poller.rs:227-237`).
- `http.rs`'s `AppState` carries `default_ttl`,
  `manual_default_priority`, `cmux_priority`, `cmux_ttl_secs`
  (`src-tauri/src/http.rs:27-37`), consumed in the source-fallback
  resolution (`http.rs:173-177`).
- `settings::build_test_event` reads per-source priority/ttl fields for
  all five Origins (`settings.rs:542-633`, field reads at
  `settings.rs:548, 550, 564, 566, 587, 589, 603, 619`).

`config.rs` is 661 lines total; the per-source pattern accounts for
roughly 400 of them (fields + defaults + `Default` impl + tests).

## §1 Shared shape

**Recommendation**: per-source sub-structs embedding one shared
`SourceConfig`, with source-specific fields staying beside it:

```rust
// illustrative only — lives in this doc, not the repo
pub struct SourceConfig {
    pub enabled: bool,          // Poller sources only; see below
    pub priority: Priority,     // the one field all five Origins share
    pub poll_secs: Option<u64>, // None for push Origins (manual/cmux)
    pub ttl_secs: Option<u64>,  // None = fall back to default_ttl
}

pub struct EspnConfig {
    #[serde(flatten)]
    pub common: SourceConfig,
    pub leagues: Vec<String>,
    pub live_card: bool,
}

pub struct Config {
    // …global fields unchanged…
    pub espn: EspnConfig,
    pub rss: RssConfig,       // common + feeds + max_per_poll
    pub manual: ManualConfig, // priority only — see honesty note below
    pub cmux: CmuxConfig,     // priority + ttl_secs
    pub weather: WeatherConfig, // common + lat/lon/units/4 thresholds
}
```

`Config` becomes five sub-struct fields (`config.rs:17-81` collapses
from ~65 lines to ~5). Each sub-struct gets its own `impl Default`
replacing the 25 per-source `default_*()` free functions — the
`fn default_*()` pattern itself survives only for genuinely global
fields. `validate()` calls one `validate_*()` per source (§3).
`SettingsApp.tsx`'s `interface Config` (`SettingsApp.tsx:35-73`) nests
identically; each section receives `config.espn` instead of the whole
`config` (§4).

Honesty note: `ManualConfig` is a single-field struct
(`manual_default_priority` today, `config.rs:51`) and `CmuxConfig` is
two fields — for the push Origins the "shared shape" is nearly empty.
The win concentrates on the three Poller sources.

**Rejected alternative — `HashMap<SourceKind, SourceConfig>` replacing
the flat fields entirely**: loses static typing for the 11
source-specific fields (they would need a parallel per-source escape
hatch anyway, recreating the duplication it removed); makes
`Config::default()` and serde defaults awkward (a map can't express
"espn defaults enabled, rss defaults disabled" as naturally as typed
structs, cf. `config.rs:160-162` vs `config.rs:189-191`); and turns every
settings-IPC access into a lookup the TypeScript mirror cannot type-check
— the `get_config`/`save_config_and_relaunch` contract
(`settings.rs:637-713`) ships `Config` whole, so its shape *is* the IPC
schema.

**Rejected alternative — macro-generated fields** (`define_source!`
expanding fields + defaults + `Default` lines): keeps the flat struct,
so it solves only the `config.rs` third of the problem — `validate()`
blocks and the five UI sections stay exactly as hand-written; and the
macro must encode per-source quirks (`manual_` has no `enabled`,
weather's thresholds, the `default_ttl` inheritance shim at
`config.rs:373-395`) as special cases, which is worse to read than the
current honest repetition.

## §2 TOML wire-format compatibility

**This is the crux constraint.** Today's wire format is flat keys:
`espn_enabled = true`, `weather_poll_secs = 900` at the top level of
`~/.config/notchtap/config.toml`. The `#[serde(default)]` whole-struct
attribute (`config.rs:8`) plus per-field defaults makes every key
optional.

A nested `pub espn: EspnConfig` with derive-based serde serializes as an
`[espn]` **table**, not flat keys. There is no derive-native way to make
a sub-struct read/write prefixed flat keys: `#[serde(flatten)]` inlines
the inner field *names unchanged* (five embedded `SourceConfig`s would
collide on `enabled`/`priority`), and serde offers no per-instance
prefix. So the §1 shape **breaks every existing user's config file**
unless one of two things is added:

1. **A parse-time migration shim** (recommended): extend the existing
   precedent in `Config::parse` (`config.rs:373-395`), which already
   re-parses the raw TOML table to preserve back-compat across the
   `espn_ttl_secs`/`cmux_ttl_secs` schema split (`config.rs:386-393`).
   The shim would detect legacy flat keys (`espn_enabled`, …), lift them
   into the new nested shape, and — because the settings window rewrites
   the whole file on save via `write_config_atomic`
   (`settings.rs:416-425`) — the file migrates on first save after
   upgrade. This is one more block in a function that already exists for
   exactly this purpose. Risk: the shim must map ~25 legacy keys, an
   order of magnitude more than the current two-key shim, and the
   hand-maintained key list is itself a new duplication vector.
2. **Hand-written `Deserialize`/`Serialize` for `Config`**: keeps flat
   keys forever with the nested Rust shape. Rejected — it replaces
   derive-generated code with the largest hand-written serde impl in the
   codebase, for a cosmetic wire-format preference, and every future
   field must be added in two more places.

**Answer to the plan's question**: the proposed shape does *not*
serialize to today's flat keys; it requires a migration shim modeled on
`config.rs:373-395`. Wire-compatible-without-shim is only achievable by
keeping the flat Rust fields (i.e., not doing the consolidation), or by
the rejected hand-written serde impl.

**Rejected alternative — break the wire format with no shim**: existing
installs' `espn_enabled = false` (etc.) would silently revert to defaults
on upgrade because `#[serde(default)]` treats unknown flat keys as
ignorable and missing `[espn]` tables as defaulted. A user who disabled a
source would have it re-enabled by an app update — the exact class of
silent-regression the `config.rs:375-385` comment block was written to
prevent. Not acceptable.

## §3 Validation

**Recommendation**: keep validation as plain per-source free functions —
`validate_espn(&EspnConfig)`, `validate_rss(&RssConfig)`,
`validate_weather(&WeatherConfig)` — called from `validate()`
(`settings.rs:49-211`), exactly following the existing
`validate_appearance` precedent (`settings.rs:213-235`, invoked at
`settings.rs:202-204`). Source-specific checks (rain-threshold 0–100 at
`settings.rs:149-154`, the hot>cold cross-field rule at
`settings.rs:163-168`, feed-url and duplicate checks at
`settings.rs:111-129, 169-179`) stay next to the source's own fields,
inside the source's own function. Cross-source rules that remain global
(`rotation_order` permutation, `settings.rs:181-200`; port/ttl caps,
`settings.rs:52-69`) stay in `validate()` directly. The shared
`SourceConfig` range checks (poll 5–3600, ttl 1–3600 — today repeated at
`settings.rs:70-81, 93-104, 143-148`) become one helper each source
function calls, which is where the consolidation actually pays off here.

**Rejected alternative — a `Validate` trait with `fn validate(&self) ->
Result<(), Vec<String>>` per source**: five implementors with
heterogeneous checks (weather's cross-field rule, rss's two-phase
url-then-duplicate pass) share no call pattern beyond the signature, so
the trait buys one loop iteration in `validate()` at the cost of
indirection every reader must jump through. The repo already chose the
plain-function style for `validate_appearance`; consistency wins.

## §4 Settings UI

**Recommendation**: leave the five section components hand-written;
change only their prop shape (pass `config.espn` instead of `config`)
and the TypeScript `Config` interface nesting (`SettingsApp.tsx:35-73`).
The sections already share all their actual duplication through the
declarative controls — `ToggleControl`, `NumberControl`,
`PriorityToggle`, `TestButtonRow`, `SettingsGroup` (composed identically
in `FootballSection`, `SettingsApp.tsx:710-767`, and `WeatherSection`,
`SettingsApp.tsx:896-990`). What remains per-section is genuinely
per-source: espn's leagues `TextareaControl` with its `leaguesText`
local state (`SettingsApp.tsx:719-726`), rss's feeds textarea and
`max_per_poll`, weather's seven threshold controls. A shared "source
section" abstraction would have to parameterize exactly the things that
differ. The shape change pays off on the Rust side; the UI gain is
limited to not repeating the `espn_` prefix in every `patchConfig` call
(`SettingsApp.tsx:717, 735, 745, 753, 760`).

**Rejected alternative — a data-driven `SourceSection` component
rendering controls from a per-source descriptor**: the descriptors would
need to express control type, min/max/unit, help copy, local-state
escapes (leagues/feeds textareas), and per-source test buttons — i.e. a
small form-schema engine to save ~100 lines of readable JSX across five
sections. The repo's settings UI deliberately favors explicit composition
(five near-identical section functions, `SettingsApp.tsx:698-993`); a
schema engine is a new abstraction layer for its own sake.

## §5 Migration cost for the five existing sources

File-by-file blast radius, against the verified call sites in §0:

- **`src-tauri/src/config.rs`** (661 lines): rewrite the 25 field
  declarations (`config.rs:17-81`) into five sub-structs; replace the 25
  per-source `default_*()` fns (`config.rs:160-287`) with five `Default`
  impls; rewrite `impl Default for Config` (`config.rs:309-347`); extend
  the `parse()` shim (`config.rs:373-395`) with the ~25-key legacy
  mapping (§2); rewrite most of the 260-line test module
  (`config.rs:398-661`) since every assertion path changes
  (`c.weather_lat` → `c.weather.lat`). Estimate: ~450 of 661 lines
  touched.
- **`src-tauri/src/settings.rs`**: split `validate()` (`settings.rs:
  49-211`) per §3; update `build_test_event`'s ten field reads
  (`settings.rs:548-619`); update its test module's per-source mutation
  sites (e.g. `settings.rs:826-943`). Estimate: ~200 lines touched.
- **`src/settings/SettingsApp.tsx`**: re-nest `interface Config`
  (`SettingsApp.tsx:35-73`); re-point ~50 control bindings and
  `patchConfig` calls across the four sections (`SettingsApp.tsx:
  698-993`); update `SettingsApp.test.tsx` fixtures (per-source keys at
  `src/settings/SettingsApp.test.tsx:12-83`). Estimate: ~150 lines
  touched.
- **`src-tauri/src/lib.rs`**: the 25-local destructure
  (`lib.rs:124-148`), `Engine::new` call (`lib.rs:200-207`), and three
  poller spawns (`lib.rs:330-367`) re-point to sub-struct paths — or,
  better, the destructure shrinks to passing sub-structs whole (see §8).
  Estimate: ~40 lines touched.
- **Pollers**: optionally narrow signatures to take their sub-struct
  instead of positional args (`poller.rs:725-732`,
  `rss_poller.rs:425-432`, `weather_poller.rs:227-237`) — recommended,
  since `spawn_weather_poller` already takes ten positional parameters
  (`weather_poller.rs:227-237`). Estimate: ~30 lines touched.
- **`src-tauri/src/http.rs`**: `AppState` fields and fallback resolution
  (`http.rs:27-47, 173-177`) re-point to `cmux`/`manual` sub-structs.
  Estimate: ~20 lines touched.

Everything changes in one PR: the TOML wire shim, the IPC shape
(`Config` crosses the wire whole in `get_config` /
`save_config_and_relaunch`, `settings.rs:637-713`), the UI, and every
consumer — there is no incremental landing order, because the IPC
contract couples the Rust struct to the TypeScript interface.

## §6 What doesn't get simpler

The 11 source-specific fields (§0) keep: per-field defaults (now inside
their sub-struct's `Default` impl — same count of code, better
location), per-field validation (§3 — the checks are irreducibly
per-source), and per-field UI controls (§4). The
`default_ttl`-inheritance shim (`config.rs:373-395`) survives in some
form regardless — it encodes a user-data guarantee, not a shape. The
`rotation_order` permutation rule (`settings.rs:181-200`,
`config.rs:249-267`) is global and untouched. Realistic elimination:
~25 `default_*()` fns and ~25 `Default`-impl lines collapse into five
`Default` impls (net −30 to −40 lines in `config.rs`), the shared
poll/ttl range checks dedupe (net −15 lines in `settings.rs`), and the
`espn_`/`rss_`/`weather_` prefixes disappear. The duplication that
remains — per-source tests, per-source validation bodies, per-source UI —
is the majority of today's per-source line count. **A shared shape
eliminates maybe a third of the per-addition tax, not most of it.**

## §7 Trigger condition

**Recommendation: do this only if and when a sixth source is seriously
proposed.** Today the cost (§5: ~890 lines across six files plus a
wire-format shim, landed atomically) is certain and immediate; the
benefit (smoother addition of a source that does not exist and is not
planned — `SourceKind` has exactly five variants, `event.rs:83-89`) is
speculative. The per-addition tax for a hypothetical sixth source under
today's flat pattern is well-understood and tested — plan 040 paid it
for weather and landed cleanly. If a sixth source *is* proposed, do the
consolidation as part of that source's plan (the shim and the new
source's nested config land together, and the new source never exists in
flat form). The rejected posture — "migrate proactively regardless" —
spends an L-sized, wire-format-touching change to save an unscheduled
M-sized one, and every week it sits unmerged it drifts against active
development on the very files it rewrites.

## §8 Build estimate

**L** (contingent on §1's sub-struct shape + §2's migration shim). File
list: `src-tauri/src/config.rs` (shape + shim + tests),
`src-tauri/src/settings.rs` (validate split, `build_test_event`, tests),
`src/settings/SettingsApp.tsx` + `SettingsApp.test.tsx` (interface,
sections, fixtures), `src-tauri/src/lib.rs` (destructure, spawns),
`src-tauri/src/http.rs` (`AppState`), `src-tauri/src/poller.rs`,
`src-tauri/src/rss_poller.rs`, `src-tauri/src/weather_poller.rs`
(signatures), plus `CONTEXT.md`/`ARCHITECTURE.md` decision entries.
No new dependencies; the risk is concentrated in the shim (user-data
facing) and the atomic IPC/UI coupling. Confidence: high that §2's
compatibility question is the crux and is answerable as stated; medium
on the exact struct shape — nailing `Option<u64>` vs. per-source
`ttl_secs` presence and the shim's key-mapping ergonomics wants a
throwaway prototype before the build plan is written.

## §9 Open questions for the maintainer

1. **Is TOML back-compat a hard requirement with zero user-visible
   change, or is a one-time automatic migration on first save (§2,
   option 1) acceptable?** The whole shape decision hangs on this: §1's
   sub-structs are only viable with the shim. (The repo's own precedent —
   `config.rs:373-395` — suggests migration-with-preservation is the
   accepted pattern, but that shim preserved the *flat* format; this one
   changes it.)
2. **Is a sixth source actually anticipated?** §7's trigger condition is
   only worth its cost if the answer is plausibly yes. If no source is on
   any horizon, this doc should be filed as reference (its §0 inventory
   is still the correct checklist for hand-copying a new source's config
   triad) and the consolidation deferred.
3. **`CONTEXT.md` drift**: the **Origin** glossary entry still lists
   `Football | News | Manual | Cmux` (`CONTEXT.md:32-35`) — `Weather`
   (plan 040) was never added. Worth a one-line fix regardless of this
   spike's outcome; noted here because this doc quotes the glossary
   verbatim.
4. **Should the pollers' positional-argument signatures be narrowed to
   sub-structs as part of the same change, or left alone?** Bundling is
   cheaper (the call sites in `lib.rs:330-367` are touched anyway) but
   widens the diff; `spawn_weather_poller`'s ten parameters
   (`weather_poller.rs:227-237`) are the strongest argument for bundling.
5. **`Option<u64>` vs. absent fields in `SourceConfig`**: `ttl_secs`
   exists on only three of five sources today (weather and manual fall
   back to `default_ttl`, `settings.rs:605, 621`). Should the shared
   shape model that as `Option` (uniform struct, per-source meaning) or
   should `ttl_secs` stay outside `SourceConfig` on the three sources
   that have it (no false uniformity)? Leaning: keep it outside — the
   `Option` would invite a sixth source to inherit a knob whose fallback
   semantics (`config.rs:373-395`'s inheritance) are per-source history.
