# 147 — Source identity colours, Claude/Kimi card parity, weather TTL unification

> Spec (PRD) from the 2026-07-27 trim-and-sync session. Uses the
> glossary terms in `CONTEXT.md` (Promotion, Visible, Slot, Agent
> Board, Rotation). Brand hexes were verified against the vendors'
> published palettes on 2026-07-27.

**Status:** implemented 2026-07-27 (three waves: sonnet executors +
one kimi delegation for the weather-TTL half — kimi's second task, the
settings swatches, hit its provider quota mid-run and was re-run on
sonnet. Gates 911 lib rust / 529 vitest green. Manual checks pending:
colour look on real hardware, Appearance preview, macbook notch pass)

## Problem Statement

Every notchtap card looks the same until you read it. A Claude Code
permission request, a Kimi completion, a football goal, and a news item
share one visual identity, so the operator has to read the masthead to
know what interrupted them — the one thing a glanceable overlay should
never require. The Settings panel gives no hint of any source identity
either.

Separately, the Claude Code and Kimi adapters carefully parse tool
names, file paths, error types, and the project directory — and then
the notification card drops all of it, rendering only a templated title
and a one-line summary. The card that was iterated on for days can
show a subtitle and detail cells; agent events just never fill them.

And one timing outlier: weather alerts hardcode an 8-second TTL while
every other source reads its window from config — the unified-engine
promise ("same engine, config-only differences") is one field short.

## Solution

Give every source a fixed, recognisable colour drawn from its real
identity: agent runtimes use their vendors' brand colours (Claude Code
terracotta, Codex OpenAI green, Kimi mono white, OpenCode purple), news
keeps its per-section colours and gains a science section, football/
weather/manual get stable identities from the existing overlay palette.
The colour rides the card's existing content paint channel (masthead
dot, chip, manifest label, agent hairline, a quiet identity wash), the
Agent Board's runtime labels, and appears as swatches throughout the
Settings panel so the mapping is learnable. Priority keeps its own
separate paint channel untouched.

Agent cards become as rich as the data already parsed: project name as
the subtitle, tool/path/error details as detail cells, and the board's
expanded rows finally show the subagent the registry already stores.
Weather's TTL moves into config with the same inheritance idiom as
every other source.

## User Stories

1. As the operator, I want each agent runtime's card tinted with that vendor's brand colour, so that I know which agent wants me without reading the title.
2. As the operator, I want Claude Code cards in Anthropic's terracotta, so that the most-used runtime has the most familiar identity.
3. As the operator, I want Kimi cards in Moonshot's mono white treatment, so that Kimi's restraint-is-the-brand identity survives on a dark card.
4. As the operator, I want Codex cards in OpenAI green and OpenCode cards in their TUI purple, so that the secondary runtimes are still distinguishable at a glance.
5. As the operator, I want an agent card with an unknown runtime to fall back to the existing amber identity, so that a future runtime never renders unstyled.
6. As the operator, I want news cards to keep their per-section colours, so that politics/tech/sports/business/world remain distinguishable as today.
7. As the operator, I want a science news section with its own colour, so that science items stop masquerading as tech.
8. As the operator, I want football, weather, and manual pushes to each have a stable colour, so that every Promotion is identifiable before it is readable.
9. As the operator, I want priority colour and source colour on separate visual channels, so that a High card and a Claude card never fight over the same pixel.
10. As the operator, I want the Agent Board's runtime labels coloured by runtime while state accents stay semantic, so that "who" and "what state" read independently.
11. As the operator, I want the same colours as swatches in Settings (Agents adapter cards, News section legend, Football/Weather sections, History origins), so that the mapping is learnable without trial and error.
12. As the operator, I want the Appearance preview to demonstrate the colours immediately, so that I can see the system without triggering real events.
13. As the operator, I want a Claude Code or Kimi permission card to show the tool name and file path as detail cells, so that I can judge an approval without switching windows.
14. As the operator, I want the project name as the card's subtitle, so that with several sessions running I know which repo is asking.
15. As the operator, I want failure cards to carry the error type detail, so that a failed turn tells me what kind of failure at a glance.
16. As the operator, I want the board's expanded rows to show a running subagent, so that the capability chip the board already renders refers to something visible.
17. As the operator, I want old payloads without the new fields to render exactly as today, so that nothing breaks during rollout.
18. As the operator, I want weather alert duration in config like every other source, so that "same engine, config-only differences" is actually true.
19. As the operator, I want the Settings weather preview to use the real weather TTL, so that the preview stops lying when defaults diverge.
20. As the operator, I want the colours to be fixed constants rather than settings, so that the identity system stays consistent and the overlay stays receive-only.
21. As a pusher, I want the `/notify` contract unchanged, so that colour identity is invisible to the API.

## Implementation Decisions

- **Paint channel**: source identity rides the existing category triple
  (`--cat`/`--cat-deep`/`--cat-pill`) on the card's content block. The
  priority accent channel is untouched; the "origin and priority never
  share a paint channel" law and the shell-class purity pin stay
  binding. The TTL bar stays priority-coloured (documented boundary).
- **Class scheme**: news keeps `cat-<section>` classes and gains
  `cat-science`; non-news origins get `src-<origin>` classes; agent
  cards with a known runtime get `src-<runtime-token>` (runtime names
  in identifiers are the sanctioned v7 naming exception). A pure
  resolver maps (origin, runtime) → class, twinned beside the existing
  runtime-label table.
- **Colour table** (fixed constants, one new overlay CSS chunk; house
  `color-mix` derivations): Claude Code `#D97757` terracotta; Codex
  `#10A37F`; Kimi mono white with washes held to ~10% so restraint
  reads as the brand; OpenCode `#9d7cd8` (their peach primary rejected
  as too close to Claude's terracotta); unknown-runtime agent = amber;
  football = overlay green; weather = overlay amber; manual = overlay
  blue; science = `#f2a2c8` pink (the only clearly free hue region).
  Brand hexes are app-side literals, not shared vendor tokens.
- **Wire change**: the promoted Slot payload gains an optional agent
  runtime token, populated from the event's agent metadata at
  projection time (currently dropped there). It is static per item, so
  it participates normally in `dedup_eq`. The frontend validates it
  against the closed runtime set; an unknown token rejects the payload,
  same discipline as origin.
- **Card treatment**: the free consumers of the category triple
  (masthead dot, category chip, manifest label) recolour automatically.
  The agent hairline changes from fixed amber to the source colour.
  Agent cards gain a static radial identity wash — no animation, no
  reduced-motion variants (permanent non-goal).
- **Board coexistence contract**: state accents (dot, pill, pulse)
  remain state-keyed and untouched; rows additionally carry the runtime
  class, and only the runtime label (plus a small square runtime tick,
  square vs the round state dot) reads the identity colour.
- **Parity threading**: the HTTP agent-events handler threads the
  already-parsed project name and detail list into the notification
  builder — project name as subtitle, details as detail cells. The
  data is already sanitized and capped upstream; the card already
  renders both fields. The board view additionally maps the stored
  subagent into the wire view, rendered in the expanded row's meta
  line under the render-nothing-if-absent discipline.
- **Science section**: the RSS category keyword table retargets
  "science" from tech to science (plus physics/space/health keywords);
  frontend category tables and CSS gain the section.
- **Weather TTL**: a new `weather_ttl_secs` config field, serde default
  8, deliberately NOT inheriting `default_ttl` — the historical
  behaviour was a hardcoded 8 regardless of `default_ttl`, and the
  default preserves that exactly. Threaded through the poller spawn;
  the Settings preview arm reads it (fixing an existing mismatch).
- **Settings swatches**: the shared meta chip gains an optional colour
  dot; a TS colour table (the CSS table's twin) feeds swatches in the
  Agents adapter cards, a News category legend, Football/Weather
  section rows, and History origin text.
- **No new invoke commands**; colours are not configurable; the
  overlay capability file is untouched; the `/notify` contract is
  unchanged.
- cmux relay retirement is already complete (machine settings +
  architecture doc); only a final docs grep-sweep rides along.

## Testing Decisions

- Good tests assert externally visible behaviour: the class present on
  the card's content block for a given (origin, runtime), the wire
  payload shape, what the promoted card's subtitle/details say — never
  which CSS file supplied a variable.
- Seams (ratified 2026-07-27, all existing except the last): the
  queue's Slot projection tests (runtime present for agent events,
  absent otherwise, serialization pin); the card class assertions on
  the content block (shell purity pins stay green and unmodified); the
  pure presentation-table tests (resolver totality across origins ×
  runtimes, science section); the notification-builder unit seam
  (subtitle/details threading, absent-field fallback); config parsing
  tests (weather TTL default, override, no inheritance). The one NEW
  seam: a string-level parity test pinning every TS colour-table hex
  to the overlay CSS, same register as the existing CSS-mirror test.
- Board tests pin the coexistence contract: a row carries state class
  and runtime class simultaneously; subagent chip renders when present
  and not otherwise.
- Settings tests cover the chip's colour dot and each swatch surface;
  preview fixtures gain a runtime and a science item so the Appearance
  preview exercises the system.
- Manual-only (per the testing strategy): how the colours actually
  look on hardware, notch-mode on the macbook, and the identity wash's
  visual quality.

## Out of Scope

- User-configurable colours or themes for sources.
- Colouring the TTL bar or any priority-channel element by source.
- Animated identity treatments (drift, shimmer) on the agent wash.
- Any change to Codex/OpenCode hook capabilities (provider limits).
- New notification content beyond the already-parsed fields.
- Reduced-motion variants (permanent non-goal).
- Per-feed custom colours for news; sections only.

## Further Notes

- OpenCode purple sits near the world-section lavender; accepted and
  documented in the CSS since a news card and an OpenCode card never
  compete on the same surface.
- Kimi's parser field names are an assumption copied from Claude
  Code's documented payloads (Moonshot publishes no per-event field
  tables); if real payloads differ, Kimi degrades to generic summaries
  silently. Capturing one real payload and diffing the fixtures is a
  cheap follow-up outside this spec.
- Execution as three executor waves (wire/parity/foundation →
  card/board/settings → docs + gates) per the captured plan.
