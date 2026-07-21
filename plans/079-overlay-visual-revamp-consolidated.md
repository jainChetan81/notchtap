# Plan 079: Full overlay-card visual revamp — consolidated decision session (supersedes 043, 054, 055, 056, 057, 060)

> **Executor instructions**: This is a pure decision-gathering plan — no
> code changes described anywhere in this file, **except** the confirmed
> backend data-fetch work inherited from plan 043 (see item 6a below),
> which is ready to execute independently of the visual decisions. Do not
> execute any *display* work here until every item in "Decision needed"
> below has been resolved by the operator in a dedicated `/grilling`
> session against this file. This plan supersedes plans 043, 054, 055,
> 056, 057, and 060 — do not execute those files separately; their
> content is folded into the topic list below.

## Status

- **Priority**: P2 (matches the higher-priority items it absorbs; the
  overlay's visual language is seen constantly, unlike a one-off spike)
- **Effort**: L (decision session) — larger than any single folded plan;
  M–L per surface once decisions land (build effort tracked per-surface
  once this session produces a locked decision record, same pattern as
  plan 063's `/grilling` session today)
- **Risk**: LOW-MED — visual-only, but touches every card a user sees
  constantly (idle rail, live-match card, news card, cmux relay)
- **Depends on**: 042 (DONE — current collapsed scorecard baseline),
  063 (ships separately and first — this plan's "before" baseline is
  063's shipped result, not a redo of its width/wrap mechanics)
- **Soft-coordinates with**: 059 (notification history — a privacy/
  retention decision, not a visual one; if it's ever built, its browse
  view would want this plan's visual language)
- **Category**: direction

## Data-source context folded in from plan 057

Plan 057 ("evaluate a paid sports API, e.g. Sportmonks, as an ESPN
alternative") is superseded into this file too — not because it's a
visual decision itself, but because several topics below (3, 4, 6, 13)
assume ESPN as the data source, and ESPN's endpoint is undocumented/
best-effort (the same fact that's why plan 043 needed a fallback chain,
not a single request). No paid provider has ever been evaluated in this
repo. See item 15.

## Why this matters

Filed 2026-07-20 from a `/grilling` session on plan 063 (idle-rail
notch-overlap bug) that organically expanded into a full redesign
conversation once the operator shared a reference image (bold
typography, team badges, a match/weather panel, an action-button row).
Rather than let that conversation's decisions live only in a chat
transcript, or re-litigate the same "how should notchtap's overlay
actually look" question separately across plans 054 (app icon), 055
(pause control), 056 (scorecard visual — the closest existing match to
today's discussion), and 060 (HUD-mode visual merge), this plan
consolidates all of them into one place. One dedicated session should
answer the full topic list below once, covering every surface: the idle
clock/status rail, the live-match scorecard, news cards, weather
(today's chip plus a possible future ambient background), cmux-relayed
cards, and the app icon.

**Explicitly out of scope — already decided elsewhere**: plan 063's
notch-mode width-cap/wrap mechanics were fully grilled to a locked
decision the same day this plan was filed, and ship as their own narrow
bug fix using today's *existing* visual style (no restyle). This plan's
"current state" baseline, once 063 lands, is 063's shipped result — this
plan does not reopen 063's already-settled width/wrap/floor numbers.

## Session-0 record — decisions already locked (2026-07-20, from the 063 `/grilling` session that produced this plan)

These came up during today's conversation and don't need to be re-asked:

- The overlay frontend is **receive-only** — no clickable buttons on the
  card in any redesign (`docs/ARCHITECTURE.md`'s receive-only law,
  reconfirmed today against the reference image's button row). Any
  interactivity (including 055's pause-visibility question) must be
  decided as an explicit, deliberate exception, not a quiet one.
- Real team crest **images** are not currently fetchable by the frontend
  (no frontend network access, `docs/ARCHITECTURE.md`) and would need
  rust-side fetch-and-embed plumbing plus a licensing question about
  real club logos — not resolved today, listed as an open item below.
- `SbTeam` (`src-tauri/src/poller.rs:76-82`) already parses `id` +
  `abbreviation`; `matchup()` (`poller.rs:358-370`) already computes
  structured away/home abbrev+score internally before joining them into
  one string. Sending those fields separately instead of pre-joined is
  the only "new" backend fact-gathering this topic needs — the
  computation already exists.
- `default_weather_poll_secs()` is 900s / 15 min
  (`src-tauri/src/config.rs:225-227`) — current weather condition data
  is already this fresh; only condition-based *background art* is
  missing, not the underlying data.
- The green accent (`#7fe08d`) is reserved for "something is live right
  now" (`.src-chip.live`, the pulsing live-dot) — any typography refresh
  should keep that meaning exclusive, not spend it on the plain clock.
- **Inherited from plan 043 (richer live-match event coverage)**: Step 0
  is CONFIRMED — independent overnight research (Hermes + Kimi,
  cross-checked, raw evidence in
  `research/043-worldcup-final-verification/`) polled ESPN's real World
  Cup Final (event `760517`) across the whole match and confirmed its
  `summary` endpoint's `commentary`/`keyEvents` fields both exist and
  grow monotonically (9→115→144 and 1→29→41 entries across
  9'/90'+8'/108'). A real gap found beyond the original ask: `summary`
  returned empty/404 on 2 of 6 polls even while the match was live, so
  the fetch needs a fallback chain (core API
  `/competitions/{id}/plays`, confirmed working when `summary` wasn't)
  rather than a single endpoint with retries. This backend fetch work is
  **ready to execute independently** of any display decision below — see
  item 6a.

## Running decision record (updated live as this session's `/grilling` continues)

- **Item 1 (card shape) — LOCKED, 2026-07-20, superseded twice more since
  the paragraph below was first written (kept for history; see the final
  shape after it)**: neither original option. A continuous/attached
  shape: a "neck" (the real notch in notch mode; a synthetic app-drawn
  cap in HUD mode, matching fill, zero gap) flows directly into the card
  body below, rounded-bottom only — same family as today's shape, just
  composed with a neck in *both* modes now. Reference: confirmed via
  search that real macOS notch-utility apps
  ([Boring Notch](https://theboring.name/),
  [NotchNook](https://www.macworld.com/article/2406934/notfhnook-macbook-dynamic-island-widgets-files-tray.html))
  do exactly this — neither draws literally under the physical notch
  (macOS reserves that pixel region), they extend a matching black shape
  continuously outward from it instead. An initial "floating with a
  gap" version was tried and explicitly rejected — the gap read as
  disconnected ("hanging").
  - **Final shape (2026-07-20, after two more operator-photo rounds)**:
    the notch's own rectangle is a **permanent cutout** — never touched
    by app content in any state, not even the neck concept above. Built
    as three plain blocks (not clip-path, which silently clips
    overflowing children like the clock pill first tried): `.flank-left`
    / `.flank-right` sit either side of the cutout at the notch's own
    height, `.below-block` (when present) carries everything under that
    row, full width. Rounding is minimal and precise: the cutout's own
    base corners stay perfectly **square** (a plain sharp bite, per a
    real-photo correction — rounding the cutout's corners independently
    of the flanking blocks' corners left a visible gap where the two
    curves didn't meet); rounding exists **only** at the two true outer
    ends of the whole visible shape (whichever block is actually at the
    bottom — the flank blocks in the idle state, `.below-block` when one
    exists). Card fill is pure `#000`, matching the real hardware notch
    exactly (confirmed via a real macOS photo — not a semi-transparent
    near-black). This shape is unconditional across both modes now — no
    remaining notch-mode/HUD-mode branch in the mockup at all.
  - **[2026-07-21 review-plan pass: decision LOCKED but BUILD STILL
    OPEN.** Verified at HEAD `647f6d0`: no `.flank-left`/`.flank-right`/
    `.below-block` anywhere in `src/styles.css` — the shipped overlay
    still uses the pre-redesign rounded-card language; the cutout shape
    exists only in `prototype/notch-states.html`. None of plans 080-086
    built it, and no build plan for it has been filed yet.]
  - **New idle content (supersedes item 2's original framing)**: not
    day+time+text-pills. Just the time on one flanking side, three
    status dots (Football=green/News=red/Weather=yellow, glow when
    active, dim flat when disabled) on the other — operator-confirmed as
    "nailed." The original idle-view's day-progress **timeline slider**
    (`.idle-view .timeline`, `styles.css`) has no home in this new layout
    yet — open, see item 18 below.
  - **Three states, not one**: collapsed (bare cutout, no card at all —
    the future hide-when-idle resting state, item 17), idle (time+dots,
    above), and expanded-on-hover — which itself dropped a first attempt
    at dense per-source status rows (too slow to parse) in favor of a
    simple flat weather-mood background scene (three example moods
    built: rainy night, overcast rain, sunny — CSS gradients + blob
    shapes, static per operator instruction, real SVG/image art to come
    later, explicitly not Lottie for now) with just temp+condition
    overlaid.
  - **Scope check against `DESIGN.html`'s real component table (§03),
    2026-07-20**: everything above only covers the **idle state** and
    **one specific card** (the live-match scorecard, crests+score+clock
    pill in the flanking row). The general **compact card** used for
    every other notification — `.compact::before` (the 3px priority
    accent edge), `.stamp` (status word), `.compact-hint` (⌃⇧N hint),
    `.track` (queue-progress slider) — and the entire **expanded**
    `.manifest` view have **not** been touched by the new shape at all.
    Added as items 18-19 below so this doesn't get lost.
- **Item 3 (team identification) — LOCKED, 2026-07-20**: real club
  crests, not flags or text badges. **[2026-07-21: BUILT — plan 083
  workstream a (`59bceda`, `src-tauri/src/crests.rs` fetch/cache/serve)
  plus plan 084 display (`ff9a312`, `Crest` in
  `src/components/StatusRailCard.tsx` with text-abbrev fallback).]**
  Checked a real ESPN fixture
  (`src-tauri/tests/fixtures/scoreboard-esp.1.json`): flags don't apply
  semantically since this app's 3 configured leagues (`league_label` in
  `poller.rs`) are all club competitions, not national teams. ESPN's
  response for every one of those leagues already includes a direct
  `team.logo` URL — no new data source needed, just parsing one more
  field already present in a response already being fetched. Rust still
  needs to fetch-and-embed that URL (frontend has no network access);
  the *discovery* cost, not the plumbing cost, is what dropped to zero.
- **Item 15 (data source) — LOCKED, 2026-07-20**: stick with ESPN. Plan
  057's evaluation is closed, not deferred further.
- **New reference target**: NotchNook's interaction language (hover-to-
  expand from a minimal idle state, now-playing-style layout) is now the
  explicit visual/interaction reference for the rest of this session —
  not to replace notchtap (NotchNook has no plugin/API story for custom
  notification sources — searched, found none — so it can't ingest the
  `/notify` endpoint, ESPN scores, RSS news, weather, Telegram, or the
  cmux relay that's this app's actual reason for existing), but its
  *shell* conventions are worth matching much more closely than the
  first two mockup rounds did.

## Effort triage (2026-07-20, operator asked for a build-cost breakdown before continuing)

Grouped by actual implementation cost, not decision complexity — a few
"easy to decide" items (e.g. item 17) are among the biggest to build:

- **Small — CSS/component restyle only, no new plumbing**: item 1 (card
  shape, locked above), item 2 (idle typography), item 5's pure layout
  (bigger score element, clock pill, gradient edge — assuming the data
  behind it already exists), item 10 (bottom-row pills), item 11 (pause
  indicator), item 14 (staleness tweaks).
- **Medium — real but bounded new backend work**: item 3 (real crests —
  rust gets the `team.logo` URL for free per the lock above, but still
  needs to fetch the image bytes, embed them since the frontend has no
  network access, decide caching so the same team's logo isn't refetched
  every poll, and a text-abbreviation fallback if the fetch fails), item
  4 (live-match wire shape — a small rust type change), item 6 (event
  icon set — a handful of new icon assets + mapping logic).
- **Big — flagged explicitly, not to be casually folded into a restyle
  pass**: item 16 (now-playing/media controls) is the bigger of the two
  big items — macOS has no public API for this, so it needs a new Swift
  helper subprocess (same architectural pattern as `notchtap-detect`,
  `docs/ARCHITECTURE.md` §5) talking to the undocumented `MediaRemote`
  framework, comparable in scope to building the ESPN poller was. Item
  17 (hide-when-idle/reveal-on-hover) is a real interaction-model change
  (hover detection, window resize-on-hover, reconciling with "does a
  High-priority alert still force itself visible while hidden") — not a
  style question.
- **Operator direction (2026-07-20)**: proceed with prototyping the
  small/medium items now (continuous notch shape, crest-based
  scorecard, typography). Items 16 and 17 are deferred as their own
  future build efforts, not kept in the active mockup-iteration loop.

## Decision needed (operator) — remaining topics for the dedicated session

2. Idle clock/day-row typography scale (bold time, day-of-week
   treatment). **[2026-07-21: decision effectively LOCKED by item 1's
   "time+dots" idle record above ("nailed"); BUILD STILL OPEN —
   `src/components/IdleView.tsx` at HEAD `647f6d0` still renders the
   old text-pill rail + timeline; only plan 063's width cap (`4fb3af9`)
   touched the shipped idle view.]**
4. Live-match wire shape: confirm structured fields (league, home/away
   abbrev, home/away score, minute) over one joined string, once 043's
   real data shape is available to design against. **[2026-07-21:
   SHIPPED — plan 083 workstream b (`512e6e9`) put structured
   `EspnMeta` on the wire; plan 084 (`ff9a312`) consumes it in
   `StatusRailCard.tsx` (gates the live branch on `slot.espn`, never
   string-sniffing).]**
5. Score prominence — a bigger, visually distinct score element vs.
   today's inline title text. **[2026-07-21: SHIPPED — plan 084
   (`ff9a312`): crest–score–crest row, `.score-row`/`.score` at
   `src/styles.css:950-997`.]**
6. Event icon set for scoring plays / cards / fouls / offside — the
   *display* half of what plan 043 fed into this file; without richer
   event data, this stays limited to goal/card/penalty/own-goal (today's
   `poller.rs` extraction). **[2026-07-21: SHIPPED — display via plan
   084 (latest-event line, CSS-shape event icons + event tint); the
   richer event data arrived via plan 083 workstream c (`9dad2b5`).]**
   - **6a. The backend half is not a design decision** — the fetch/parse
     work described in the Session-0 record above (fallback chain
     against ESPN's `summary`/`plays` endpoints) is confirmed and can be
     executed independently, any time, ahead of or in parallel with this
     session's display decisions. Whoever picks this up should treat 6a
     as its own execution step, not wait on items 1-14. **[2026-07-21:
     SHIPPED — plan 083 workstream c (`9dad2b5`), richer ESPN match
     events with the confirmed summary→plays fallback chain.]**
7. **News card** presentation — any restyle beyond today's
   masthead/category-pill treatment (flagged in-scope per the operator's
   "everything UI-related" direction). **[2026-07-21: SHIPPED — plan
   080 (`d21d689`): compact `.pub-meta` published-time
   (`src/styles.css:712`) + full-width expanded `.manifest-block`
   summary (`:745`). Note: styled within today's pre-cutout visual
   language — revisit only when/if item 1's shape is built.]**
8. **Cmux-relayed** notification presentation — any surface-specific
   visual treatment. **[2026-07-21: STILL OPEN — nothing landed;
   operator decision needed (does cmux get any treatment beyond the
   general compact card of item 19, or none).]**
9. **Weather**: today's chip styling, plus condition-based *background*
   art (starfield for clear night, rain, snow, hot/cold, etc.) —
   deliberately deferred out of plan 063 today; this is the open-ended
   one (condition bucket list, day/night variants, static vs. animated,
   asset sourcing) and may itself need splitting into its own follow-up
   spike rather than a single-sitting answer. **[2026-07-21: PARTIALLY
   SHIPPED — plan 082 (`7d48062`): 12 vendored Meteocons (MIT, Bas
   Milius) at `src/assets/weather/` + NOTICE, the condition→(glyph,
   mood, day/night) table in `src/lib/weatherArt.ts`, and CSS mood
   gradients + glyph on the rain/hot/cold ALERT cards
   (`src/styles.css:787-860`). Remaining: idle-chip styling, the
   expanded ambient background scene (hover-gated, now on plan 087),
   and assigning the spare buckets — `extreme-rain`/`wind`/`hail` are
   vendored but unassigned.]**
10. Bottom-row / status-pill final shape — today's direction (inert
    pills carrying real existing data: News/Weather/Queued, restyled
    bigger/bolder) applied consistently across every card state, not
    just idle. **[2026-07-21: STILL OPEN — no pill restyle landed;
    decide together with items 2/18/19 since they share the idle/compact
    surfaces.]**
11. **In-card pause discoverability** (plan 055) — pause already exists
    (hotkey + tray + boot checkbox); does it get a visible *static*
    indicator (not a button, per the receive-only law above), and if so
    what does it look like. **[2026-07-21: STILL OPEN — operator
    decision needed. If built, plan 085's appearance-channel wiring is
    the named template (see its maintenance notes).]**
12. **App icon/branding** (plan 054) — `src-tauri/icons/` is still the
    Tauri scaffold default; needs a visual-identity direction before any
    artwork is generated. Different surface (the app icon, not the
    in-app card) but folded in for one-stop visual-identity discussion.
    **[2026-07-21: STILL OPEN — `src-tauri/icons/` is still the
    scaffold set; operator direction needed before any artwork.]**
13. **Asset sourcing / licensing**, resolved once for every surface that
    wants non-text art: real club crests/flags, weather-condition art,
    app icon artwork — where do assets come from, and is using real
    team branding acceptable scope for this app at all (worth a direct
    answer, not an assumption). **[2026-07-21: PARTIALLY RESOLVED in
    practice — weather art: Meteocons, MIT, vendored with NOTICE (plan
    082); crests: runtime fetch-and-cache under
    `~/.config/notchtap/crests/`, never committed to the repo (plan
    083's trademark stance). Remaining: app-icon artwork sourcing,
    together with item 12.]**
14. Staleness/age indicators — confirm whether today's shipped
    `ageLabel`/pill treatment (plan 032) extends into the new visual
    language as-is or needs its own restyle pass. **[2026-07-21: STILL
    OPEN — plan 032's treatment carried forward untouched by 080-086;
    decide with/after item 19. Adjacent but distinct: plan 081
    (`ae50ca1`, `TtlBar.tsx`) added a per-card TTL/rotation progress
    bar — a new locked visual element this file's topic list predates.]**
15. ~~**Data source**~~ — **LOCKED**, see running decision record above.
16. **Now-playing / media controls** — a genuinely new source type
    (alongside Football/News/Weather/Cmux today), directly inspired by
    NotchNook. Real caveat before committing: macOS has no public,
    documented API for system-wide now-playing metadata — NotchNook and
    similar apps lean on the undocumented, semi-private `MediaRemote`
    framework. Buildable, but worth deciding with that dependency risk
    named up front, not discovered mid-build. **[2026-07-21: STILL OPEN
    — deliberately deferred per the effort-triage note; operator
    decision: commit to a MediaRemote Swift helper as its own build
    effort, or drop the source type.]** **[2026-07-21 operator
    decision session: DECIDED — commit to a SPIKE (086-shape,
    investigate-first, not a build commitment): evaluate the
    MediaRemote Swift-helper approach following the `notchtap-detect`
    subprocess precedent (ARCHITECTURE.md §5), with the
    undocumented-framework fragility as the spike's central question.
    The advisor was recommending drop; operator chose to investigate
    first. Spike plan to be filed at the next free number in a future
    session — deliberately NOT filed today (operator asked to wrap up
    planning without starting new work).]**
17. **Hide-when-idle, reveal-on-hover** — not a visual decision, a real
    interaction-model change. Today's overlay is explicitly architected
    as a "permanent rotating overlay" (`CLAUDE.md`'s v3.6 description) —
    always visible, no idle-hide state. Switching to hover-to-reveal
    means deciding whether that permanence guarantee still holds, and if
    not, what replaces it (e.g. does a High-priority alert still force
    itself visible even while "hidden"?) — needs its own explicit
    decision, not an assumption that it comes free with a restyle.
    - **Bigger than previously scoped (found 2026-07-20, checking the
      real code while exploring hover mockups)**: `apply_overlay_native_config`
      (`src-tauri/src/lib.rs:530-540`) calls
      `window.set_ignore_cursor_events(true)` **unconditionally, on the
      entire window** — not scoped to the notch area, everywhere. The
      comment explains why: without it, the window (which sits flush
      over the real menu bar) swallows clicks meant for other apps'
      menu-bar icons underneath it (the exact 2026-07-17 bug this line
      fixed). Net effect: **no hover or mouse interaction works
      anywhere on the real shipped window today**, not just over the
      notch's dead zone — this is deliberate, not an oversight. Any
      hover-to-reveal design (including the state-model demo built this
      session) needs real re-engineering to make live: almost certainly
      tracking actual cursor position and toggling
      `ignore_cursor_events` on/off dynamically only while the cursor is
      within the card's current rendered bounds, or the original
      icon-swallowing bug comes back. This is additive to, not a
      replacement for, the effort-triage note above — item 17 is bigger
      than "big," it touches this specific native-config function
      directly.
    - **[2026-07-21: PARTIALLY SHIPPED / DE-RISKED.** Cheap half shipped
      via plan 085 (`725040b`): `resting_state: rail|notch` config flag
      (`src-tauri/src/config.rs:100-101`), collapsed bare-notch resting
      state selectable from Settings. The hover blocker above was
      resolved by plan 086's spike (`4ec98fe`,
      `docs/design/hover-cursor-tracking.md`): tracking-area events
      still fire while `set_ignore_cursor_events(true)` stays
      unconditional — no dynamic toggling needed, the 2026-07-17
      icon-swallowing fix survives untouched. **Plan 087 SHIPPED
      (2026-07-21, `a5a98b1`, merge `0b316c8`, filed done)**: the one
      shared mechanism exists — `hover::active_card_rect` +
      `status_rail_active` (`src-tauri/src/hover.rs`), the
      tracking-area block, the transitions-only `hover-changed` event,
      and a diagnostic `.hovered` class. The reveal-on-hover feature
      itself is now unblocked but UNBUILT, with one carried-forward
      limitation every consumer must handle: 087's rect is
      **x-constrained only** — its vertical span is the full 300px
      window, so ~240px of empty space below the card currently
      registers as hovered. This item's idle expanded-on-hover (and
      the other three consumers: 081's TTL hover-pause, 082's weather
      peek, 084's rail→scorecard reveal) must each add a height
      constraint to the rect before shipping its hover half — the
      y-flip helper (`css_top_down_to_appkit_y`) is already built and
      tested for exactly that.]
18. **Timeline/day-progress slider** — today's idle view has a thin
    horizontal line with a moving dot showing progress through the day
    (`.idle-view .timeline`, `styles.css`). The new time+dots idle layout
    dropped it entirely with nowhere obvious for it to live. Decide:
    bring it back (where?), fold it into the expanded state instead, or
    drop it for good. **[2026-07-21: STILL OPEN — `.idle-view
    .timeline` is still shipped and live (`src/styles.css:574`,
    rendered by `IdleView.tsx`) because the new idle layout hasn't been
    built. Plan 081's TTL bar is per-card rotation timing, not this
    day-progress slider — it does not resolve this item.]**
    **[2026-07-21 operator decision session: DECIDED — fold into the
    hover-expanded idle state (087's primitive makes it reachable):
    the timeline appears only in the idle expanded-on-hover view, not
    the resting time+dots rail, preserving the locked minimal idle.
    Lands with the items-1+2 idle build, not as its own plan. Note
    087's carried-forward y-span limitation applies to that build like
    every other hover consumer.]**
19. **The general compact card + expanded manifest view** — everything
    built so far covers only idle and the live-match scorecard. The
    accent edge (`.compact::before`, priority color), the stamp word
    (`.stamp`), the `⌃⇧N` hint (`.compact-hint`), the queue-progress
    slider (`.track`), and the entire expanded `.manifest` detail grid
    all still use today's pre-redesign visual language and haven't been
    reconciled with the new notch-cutout shape at all. This is the
    biggest remaining gap — most notifications a user actually sees
    (not live matches) go through this path, not the one path that's
    been mocked up. **[2026-07-21: STILL OPEN — re-verified at HEAD
    `647f6d0`: `.compact::before` (`src/styles.css:342`), `.stamp`
    (`:391`), `.compact-hint` (`:401`), `.track` (`:433`), `.manifest`
    (`:519`) are all unchanged pre-redesign language. Plan 080 restyled
    only the news manifest's internals *within* that language; plan 084
    built only the football path. Still the biggest remaining gap; its
    revamp is gated on item 1's shape being built first.]**

## Recommendation

Run one dedicated `/grilling` session against this file covering the
remaining topics (2, 4-14, 16-19 — items 1/3/15 are already locked, see
above), in the same style as today's plan 063 session (one decision at a
time, recommendation-first, facts looked up rather than asked). Produce
a single locked decision record before any execution plan gets written —
do not start building any one surface (e.g. just the scorecard) ahead of
the others, since several topics (asset sourcing, bottom-row shape) are
shared decisions that apply across multiple cards and would need
re-litigating per-surface otherwise. Item 17 (hide-when-idle) is worth
resolving early, since a "yes" answer changes the baseline every other
item's mockups get built against. Item 19 (the general compact card +
manifest view) is the single biggest remaining gap by user-facing
impact — most real notifications go through that path, not the
live-match path that's been the focus of the mockups so far.

## Maintenance notes

- Plans 043, 054, 055, 056, 057, 060 are marked `SUPERSEDED by 079` in
  `plans/README.md` — their content is fully represented in this file
  (043's confirmed backend research lives in the Session-0 record and
  item 6a above; 057's evaluation question lives in item 15); do not
  execute those files separately.
- Plan 059 is **not** superseded — persisting/browsing notification
  history is a privacy/retention decision first; only its eventual
  browse-view styling would draw on this plan.
- Plan 077 (Settings-window log panel) is deliberately **not** part of
  this consolidation — it's a different surface (the capability-gated
  Settings window, not the receive-only overlay card) with its own
  visual context.
- Plan 065 (hardcoded BrightData API token) is unrelated to this plan
  entirely — a live credential exposure needing operator rotation, not a
  design decision. Flagged here only so it doesn't get lost in the
  shuffle of "combine everything."

---

**Review-plan pass (2026-07-21)**: statuses above verified against live
code at HEAD `647f6d0` and annotated per item (bracketed **[2026-07-21:
...]** notes). Scoreboard — SHIPPED: items 3, 4, 5, 6/6a (plans 083/084:
`59bceda`, `512e6e9`, `9dad2b5`, `ff9a312`) and item 7 (plan 080,
`d21d689`). PARTIALLY SHIPPED: item 9 (plan 082, `7d48062` — alert-card
art only), item 13 (weather + crest licensing settled by
implementation; app icon open), item 17 (plan 085 `725040b` cheap half;
plan 086 spike `4ec98fe` de-risked hover; plan 087 filed TODO). LOCKED
BUT UNBUILT: items 1 and 2 — the cutout shape and time+dots idle exist
only in `prototype/notch-states.html`; no build plan filed. STILL
OPEN / OPERATOR DECISION: items 8 (cmux treatment), 10 (pill shape), 11
(pause indicator), 12 (app icon direction), 13-remainder (icon artwork
sourcing), 14 (staleness restyle) — items 16 and 18 were DECIDED in the
2026-07-21 operator decision session (16: MediaRemote spike to be filed,
not dropped; 18: timeline folds into hover-expanded idle, rides the
items-1+2 build — see the bracketed decision records on those items).
Same session also resolved decisions outside this file: 072's Step 0
(reject-on-full, ratifying what the executor shipped), 059's four
history questions (full-source JSONL history — see that plan), and 065
(deferred by operator). Item 19 remains the
biggest user-facing gap, mechanically gated on item 1's build. Item 15
stays LOCKED (ESPN). One vocabulary gap: plan 081's TTL bar
(`ae50ca1`) is a locked visual element this file's topic list predates
— noted under item 14. Mechanical follow-ups needing no new decisions:
(a) a build plan for items 1+2 (shape + new idle), (b) execute plan
087 then the four hover consumers, (c) an item-19 revamp plan once (a)
lands. Recommendation: keep this file as the single tracking ledger
(re-filing would orphan the Session-0 record and lock history); the
annotations above make it current. Companion trackers
`plans/079-checklist.html` / `plans/frontend-ui-consolidated.html`
were out of scope for this pass and may lag these annotations.

**Review-plan pass 2 (2026-07-21, at `958c2f7`)**: delta since the
pass above — plan 087 moved from "filed TODO" to **SHIPPED** (merge
`0b316c8`; item 17's bracket updated in place with the carried-forward
y-span limitation every hover consumer must close). Follow-up (b)
above is therefore half done: the primitive exists; the four consumer
wirings (081 hover-pause, 082 weather peek, 084 scorecard reveal,
item 17's own idle expand) are the remaining halves, each needing a
height constraint per the limitation note. One new cross-plan coupling
worth knowing before item 1/2's build plan is written:
`src-tauri/src/hover.rs:27-33` hardcodes rust mirrors of the CURRENT
card widths (270/400/460/500, cited to `styles.css` lines) — the
cutout-shape rebuild (item 1) and new idle layout (item 2) will change
those widths, so their build plan MUST scope updating `hover.rs`'s
constants (and its `active_card_rect` mode branches) in the same
commit, or hover geometry silently diverges from the redesigned cards.
Also verified this pass: plans 068/072/074 (test coverage + a queue
cap guard) merged the same day touch nothing visual — no annotation
impact. `plans/079-checklist.html` was updated by the 087 filing
session and is currently IN SYNC with these annotations, not lagging.
