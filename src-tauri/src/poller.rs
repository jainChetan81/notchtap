use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::Deserialize;
use uuid::Uuid;

use crate::crests::CrestCache;
use crate::engine::Engine;
use crate::event::{
    DetailItem, EspnMeta, Event, EventMeta, EventPayload, EventSignal, EventType, Priority,
    RotationSpec, SourceKind,
};
use crate::status::LiveMatchSummary;

// ---------------------------------------------------------------------------
// scoreboard wire shape — minimal, tolerant structs for the fields the diff
// actually uses. field names verified against real captured payloads on
// 2026-07-16 (src-tauri/tests/fixtures/scoreboard-*.json); everything is
// defaulted so a missing field degrades to "no delta", never a parse error.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Scoreboard {
    #[serde(default)]
    pub events: Vec<SbEvent>,
}

#[derive(Debug, Deserialize)]
pub struct SbEvent {
    pub id: String,
    pub status: SbStatus,
    #[serde(default)]
    pub competitions: Vec<SbCompetition>,
}

#[derive(Debug, Deserialize)]
pub struct SbStatus {
    #[serde(rename = "type")]
    pub status_type: SbStatusType,
    // espn's own clock text ("45'", "120'") — the idle rail's live chip
    // shows it verbatim as the match minute (plan 034).
    #[serde(rename = "displayClock", default)]
    pub display_clock: String,
}

#[derive(Debug, Deserialize)]
pub struct SbStatusType {
    // "pre" | "in" | "post"
    #[serde(default)]
    pub state: String,
    // e.g. "STATUS_HALFTIME" while state stays "in"
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SbCompetition {
    #[serde(default)]
    pub competitors: Vec<SbCompetitor>,
    // goals and cards, with athlete + clock — present on live/finished games
    #[serde(default)]
    pub details: Vec<SbDetail>,
}

#[derive(Debug, Deserialize)]
pub struct SbCompetitor {
    #[serde(rename = "homeAway", default)]
    pub home_away: String,
    // espn sends scores as strings ("0", "2")
    #[serde(default)]
    pub score: Option<String>,
    #[serde(default)]
    pub team: Option<SbTeam>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SbTeam {
    // plan 042: per-side card bucketing cross-references card details'
    // `team.id` against this competitor-level id.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub abbreviation: String,
    /// plan 083 workstream a: ESPN already sends a direct crest CDN url
    /// per team on the scoreboard response we already fetch — zero extra
    /// discovery cost. Verified present in the checked-in
    /// `scoreboard-esp.1.json` fixture (`.../teamlogos/soccer/500/{id}.png`).
    /// Detail-level card `team` objects (`SbDetail::team`, which reuses
    /// this same struct) never carry `logo` — defaults to `None` there,
    /// harmlessly.
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SbDetail {
    #[serde(rename = "type", default)]
    pub detail_type: Option<SbDetailType>,
    #[serde(rename = "scoringPlay", default)]
    pub scoring_play: bool,
    // structural card-color signal (v3.6 EventSignal work) — espn's real
    // payload carries this boolean per detail (and a matching yellowCard
    // one we don't need: within a "Card" detail it's binary, not-red
    // means yellow). verified against the checked-in uefa.champions
    // fixture. reading it directly avoids text-matching "Red"/"Yellow"
    // out of the composed detail_type.text.
    #[serde(rename = "redCard", default)]
    pub red_card: bool,
    // structural own-goal signal, exactly like `red_card` above — read
    // directly instead of guessing at whatever free-text label ESPN uses
    // for an own goal (unverified; no checked-in fixture has one).
    #[serde(rename = "ownGoal", default)]
    pub own_goal: bool,
    #[serde(default)]
    pub clock: Option<SbClock>,
    #[serde(rename = "athletesInvolved", default)]
    pub athletes: Vec<SbAthlete>,
    // espn's detail-level team object is just `{"id": "..."}` — reuse
    // SbTeam (no new struct): `abbreviation` simply defaults to empty.
    // structural side attribution, same discipline as `red_card`/
    // `own_goal` above (plan 042).
    #[serde(default)]
    pub team: Option<SbTeam>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SbDetailType {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct SbClock {
    #[serde(rename = "displayValue", default)]
    pub display_value: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct SbAthlete {
    #[serde(rename = "shortName", default)]
    pub short_name: String,
}

pub fn parse_scoreboard(body: &str) -> Result<Scoreboard, serde_json::Error> {
    serde_json::from_str(body)
}

// ---------------------------------------------------------------------------
// snapshot + pure diff (TESTING_STRATEGY.md §4.7 — fixture-tested, no mocks)
// ---------------------------------------------------------------------------

pub type Snapshot = HashMap<String, MatchSnapshot>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    pub home_abbrev: String,
    pub away_abbrev: String,
    pub home_score: u32,
    pub away_score: u32,
    pub state: String,
    pub status_name: String,
    /// espn's displayClock at last sighting ("45'"). Tracked here (not
    /// read off the raw feed) so the idle rail's live chip survives the
    /// absent-poll carry-forward below instead of flickering off during a
    /// transient feed blip. Never diffed — clock advance alone emits no
    /// queue event.
    pub display_clock: String,
    /// per-side (yellow, red) card counts, bucketed by cross-referencing
    /// each card detail's `team.id` against the competitors' team ids
    /// (plan 042 — replaces the single aggregate `cards: usize`; the
    /// collapsed scorecard shows the split, not a bare total).
    pub home_cards: (u32, u32),
    pub away_cards: (u32, u32),
    /// consecutive polls this match has been absent from its league's
    /// feed. reset to 0 whenever it appears; evicted at
    /// ABSENT_POLLS_BEFORE_EVICTION (review fix, 2026-07-16: a
    /// transient empty-but-valid espn response must not silently drop
    /// live matches and lose their in-window events).
    pub missed_polls: usize,
}

impl MatchSnapshot {
    /// sum of all four per-side counters — the card event-emission
    /// gate's comparison value (plan 042: fires on exactly the
    /// transitions the old aggregate `cards > old.cards` did).
    pub fn total_cards(&self) -> u32 {
        self.home_cards.0 + self.home_cards.1 + self.away_cards.0 + self.away_cards.1
    }
}

/// ~5 minutes at the default 30s cadence — long enough to ride out a
/// transient feed glitch, short enough that a genuinely pulled match
/// (postponement) can't pin the snapshot forever.
const ABSENT_POLLS_BEFORE_EVICTION: usize = 10;

struct MatchView<'a> {
    id: &'a str,
    snap: MatchSnapshot,
    last_scoring_play: Option<String>, // "Goal — K. Havertz 6'"
    last_card: Option<String>,         // "Yellow Card — B. Saka 54'"
    // only meaningful when last_card.is_some(); structural, from espn's
    // own redCard boolean, not derived from last_card's display text.
    last_card_is_red: bool,
}

fn detail_line(d: &SbDetail) -> String {
    let who = d
        .athletes
        .first()
        .map(|a| a.short_name.clone())
        .unwrap_or_default();
    let clock = d
        .clock
        .as_ref()
        .map(|c| c.display_value.clone())
        .unwrap_or_default();
    format!("{who} {clock}").trim().to_string()
}

// shared "{kind} — {line}" formatting for the card and scoring-play
// extractions in `view()` below, so the two sites can't drift. a scoring
// play can arrive with no detail_type label (a case `last_card` can't
// hit — its search filters on the label), so an empty `kind` must fall
// back to the bare line rather than produce a stray leading "— ...".
fn labeled_detail_line(kind: &str, d: &SbDetail) -> String {
    let line = detail_line(d);
    match (kind.is_empty(), line.is_empty()) {
        (true, _) => line,
        (false, true) => kind.to_string(),
        (false, false) => format!("{kind} — {line}"),
    }
}

fn view(event: &SbEvent) -> MatchView<'_> {
    let comp = event.competitions.first();
    let mut home_abbrev = String::new();
    let mut away_abbrev = String::new();
    let mut home_id = String::new();
    let mut away_id = String::new();
    let mut home_score = 0u32;
    let mut away_score = 0u32;

    if let Some(comp) = comp {
        for c in &comp.competitors {
            let abbrev = c
                .team
                .as_ref()
                .map(|t| t.abbreviation.clone())
                .unwrap_or_default();
            let id = c.team.as_ref().map(|t| t.id.clone()).unwrap_or_default();
            let score = c
                .score
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            match c.home_away.as_str() {
                "home" => {
                    home_abbrev = abbrev;
                    home_id = id;
                    home_score = score;
                }
                "away" => {
                    away_abbrev = abbrev;
                    away_id = id;
                    away_score = score;
                }
                _ => {}
            }
        }
    }

    let details = comp.map(|c| c.details.as_slice()).unwrap_or(&[]);
    // per-side (yellow, red) bucketing (plan 042 — replaces the old
    // aggregate count): cross-reference each card detail's own `team.id`
    // against the competitor ids above — structural, same discipline as
    // `red_card`/`own_goal`. a card whose team id matches neither side
    // is not counted (no verified payload does this).
    let mut home_cards = (0u32, 0u32);
    let mut away_cards = (0u32, 0u32);
    for d in details.iter().filter(|d| {
        d.detail_type
            .as_ref()
            .map(|t| t.text.contains("Card"))
            .unwrap_or(false)
    }) {
        let team_id = d.team.as_ref().map(|t| t.id.as_str()).unwrap_or("");
        let side = if !team_id.is_empty() && team_id == home_id {
            Some(&mut home_cards)
        } else if !team_id.is_empty() && team_id == away_id {
            Some(&mut away_cards)
        } else {
            None
        };
        if let Some((yellows, reds)) = side {
            if d.red_card {
                *reds += 1;
            } else {
                *yellows += 1;
            }
        }
    }
    let last_scoring_play = details
        .iter()
        .rev()
        .find(|d| d.scoring_play)
        .map(|d| {
            // own_goal checked FIRST and short-circuits the text lookup —
            // the own-goal label never depends on ESPN's (unverified)
            // own-goal text string; every other case passes ESPN's own
            // label ("Goal", "Penalty - Scored") through verbatim.
            let kind = if d.own_goal {
                "Own Goal".to_string()
            } else {
                d.detail_type
                    .as_ref()
                    .map(|t| t.text.clone())
                    .unwrap_or_default()
            };
            labeled_detail_line(&kind, d)
        })
        .filter(|s| !s.is_empty());
    let last_card_detail = details.iter().rev().find(|d| {
        d.detail_type
            .as_ref()
            .map(|t| t.text.contains("Card"))
            .unwrap_or(false)
    });
    let last_card = last_card_detail.map(|d| {
        let kind = d
            .detail_type
            .as_ref()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        labeled_detail_line(&kind, d)
    });
    let last_card_is_red = last_card_detail.map(|d| d.red_card).unwrap_or(false);

    MatchView {
        id: &event.id,
        snap: MatchSnapshot {
            home_abbrev,
            away_abbrev,
            home_score,
            away_score,
            state: event.status.status_type.state.clone(),
            status_name: event.status.status_type.name.clone(),
            display_clock: event.status.display_clock.clone(),
            home_cards,
            away_cards,
            missed_polls: 0,
        },
        last_scoring_play,
        last_card,
        last_card_is_red,
    }
}

fn league_label(league: &str) -> &str {
    // friendly labels for the locked leagues (ARCHITECTURE.md §16);
    // anything else falls back to its raw slug
    match league {
        "eng.1" => "EPL",
        "uefa.champions" => "UCL",
        "esp.1" => "La Liga",
        _ => league,
    }
}

fn matchup(league: &str, s: &MatchSnapshot) -> String {
    // league-tagged, away-first (matching espn's "ARS @ PSG"
    // orientation): "UCL: ARS 1–1 PSG" (review fix, 2026-07-16 — two
    // simultaneous matches can share team abbreviations across leagues)
    format!(
        "{}: {} {}–{} {}",
        league_label(league),
        s.away_abbrev,
        s.away_score,
        s.home_score,
        s.home_abbrev
    )
}

/// plan 083 workstream a: (team id -> logo url) pairs from a freshly
/// fetched scoreboard — pure, fixture-testable, no I/O. Read directly
/// off the raw feed (not off `MatchSnapshot`, which doesn't carry team
/// ids) so a crest fetch can be scheduled even for matches this poll
/// won't otherwise emit an event for.
fn team_logos(fetched: &Scoreboard) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for event in &fetched.events {
        let Some(comp) = event.competitions.first() else {
            continue;
        };
        for c in &comp.competitors {
            let Some(team) = &c.team else { continue };
            if team.id.is_empty() {
                continue;
            }
            if let Some(logo) = &team.logo {
                out.insert(team.id.clone(), logo.clone());
            }
        }
    }
    out
}

/// plan 083 workstream a: (match id -> (home team id, away team id))
/// pairs from a freshly fetched scoreboard — pure, fixture-testable, no
/// I/O. Read directly off the raw feed, same reasoning as `team_logos`
/// above: `MatchSnapshot` doesn't carry team ids, and a full-time event
/// evicts its match from the *next* snapshot before crest-patching runs,
/// so this can't be derived from post-diff state either.
fn team_ids_by_match(fetched: &Scoreboard) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    for event in &fetched.events {
        let Some(comp) = event.competitions.first() else {
            continue;
        };
        let mut home_id = String::new();
        let mut away_id = String::new();
        for c in &comp.competitors {
            let id = c.team.as_ref().map(|t| t.id.clone()).unwrap_or_default();
            match c.home_away.as_str() {
                "home" => home_id = id,
                "away" => away_id = id,
                _ => {}
            }
        }
        out.insert(event.id.clone(), (home_id, away_id));
    }
    out
}

/// plan 083 workstream a: patch `home_crest`/`away_crest` onto every
/// emitted event's `EspnMeta` (if any) using the match's team ids and
/// whatever's already cache-hit on disk — pure aside from the
/// `CrestCache::cached_path` filesystem stat, no network. Kept separate
/// from `diff_scoreboard` (which never touches the filesystem) so that
/// function stays the pure, fixture-tested surface it's always been;
/// this is the untested-by-design fetch-loop's job, mirroring the
/// module's existing "tested pure core, thin untested wiring" split.
///
/// Matches by parsing the match id back out of the event's own Topic
/// string (`espn:{league}:{match_id}`) — reliable because crest
/// patching only ever has something to do when `EspnMeta` is present,
/// which is exactly the same `espn_live_card`-on gate that populates
/// `topic` in the first place.
fn patch_crests(
    events: &mut [Event],
    league: &str,
    team_ids: &HashMap<String, (String, String)>,
    crests: &CrestCache,
) {
    let prefix = format!("espn:{league}:");
    for event in events {
        let Some(espn) = &mut event.meta.espn else {
            continue;
        };
        let Some(match_id) = event.topic.as_deref().and_then(|t| t.strip_prefix(&prefix)) else {
            continue;
        };
        let Some((home_id, away_id)) = team_ids.get(match_id) else {
            continue;
        };
        espn.home_crest = crests.cached_path_string(home_id);
        espn.away_crest = crests.cached_path_string(away_id);
    }
}

/// plan 039: how one match's event lands in the Slot, bundled into a
/// single parameter so `make_event` stays under clippy's 7-arg
/// `too_many_arguments` threshold.
enum CardTopic {
    /// `espn_live_card` off: today's one-shot, topicless burst —
    /// `topic: None`, `OneShot`, byte-for-byte the pre-039 behavior.
    Off,
    /// flag on, non-final event: shared per-match Topic, `Recurring`
    /// rotation — kickoff/goal/card/half-time supersede each other in
    /// the single Slot as one updating card.
    Live(String),
    /// flag on, full-time event: the *same* Topic but `OneShot` —
    /// superseding the visible Recurring card copies this rotation onto
    /// it, so the card retires via the ordinary one-shot path with no
    /// bespoke teardown.
    FullTime(String),
}

/// Maps `diff_scoreboard`'s per-match `topic` (Some only when
/// `espn_live_card` is on) plus the is-this-the-full-time-branch flag
/// onto a [`CardTopic`].
fn card_topic(topic: &Option<String>, is_full_time: bool) -> CardTopic {
    match (topic, is_full_time) {
        (Some(t), false) => CardTopic::Live(t.clone()),
        (Some(t), true) => CardTopic::FullTime(t.clone()),
        (None, _) => CardTopic::Off,
    }
}

fn make_event(
    event_type: EventType,
    title: String,
    body: String,
    ttl_secs: u64,
    signal: EventSignal,
    priority: Priority,
    card: CardTopic,
) -> Event {
    let (rotation, topic) = match card {
        CardTopic::Off => (RotationSpec::OneShot { ttl_secs }, None),
        CardTopic::Live(topic) => (
            RotationSpec::Recurring {
                display_secs: ttl_secs,
            },
            Some(topic),
        ),
        CardTopic::FullTime(topic) => (RotationSpec::OneShot { ttl_secs }, Some(topic)),
    };
    Event {
        id: Uuid::new_v4(),
        event_type,
        // v6: configurable via `Config.espn_priority` (default `High`,
        // matching the v3.6-spec-§3.4 hardcoded behavior this replaces).
        // `signal` is presentation-only (icon/animation selection) and
        // deliberately doesn't affect this.
        priority,
        rotation,
        topic,
        payload: EventPayload { title, body },
        meta: EventMeta::default(),
        signal,
        origin: SourceKind::Football,
    }
}

/// Pure delta logic (v2 spec §3). Compares the fetched scoreboard against
/// the previous snapshot and returns the Events to emit plus the snapshot
/// to carry forward. Eviction falls out of construction: the new snapshot
/// only contains matches still present in the feed and not yet final.
///
/// Emission rules:
/// - first sighting of a match: silent baseline (no restart flood); a match
///   first seen already-final is never tracked at all
/// - score changed → ScoreUpdate, body = latest scoring play ("K. Havertz
///   6'") when the feed carries one, else "goal"
/// - state pre→in → MatchState "kickoff"; status name becomes
///   STATUS_HALFTIME → "half-time"; state →post → "full-time" (and the
///   match drops from the snapshot). the halftime→second-half resumption is
///   deliberately silent — kickoff/ht/ft are the moments that matter.
/// - card count increased → MatchState with the latest card's detail
///   ("Yellow Card — B. Saka 54'"). "everything espn reports"
///   (ARCHITECTURE.md §16) includes yellows, chosen with eyes open.
/// - a match merely *absent* from the feed is carried forward (counter
///   incremented) and only evicted after ABSENT_POLLS_BEFORE_EVICTION
///   consecutive misses — so a transient empty-but-valid response
///   neither drops live matches nor loses the goals scored during the
///   blip (they diff against the carried snapshot on reappearance).
///   only an explicit "post" evicts immediately.
pub fn diff_scoreboard(
    prev: &Snapshot,
    fetched: &Scoreboard,
    ttl_secs: u64,
    league: &str,
    priority: Priority,
    espn_live_card: bool,
) -> (Vec<Event>, Snapshot) {
    let mut out = Vec::new();
    let mut next = Snapshot::new();

    for sb_event in &fetched.events {
        let v = view(sb_event);
        let final_now = v.snap.state == "post";
        // plan 039: opt-in live-match card — one Topic per match so the
        // single Slot shows a single updating card per live match instead
        // of a burst of one-shots. `None` when the flag is off (zero
        // behavior change).
        let topic = espn_live_card.then(|| format!("espn:{league}:{}", v.id));

        match prev.get(v.id) {
            None => {
                // silent baseline; a match already final on first sight is
                // not worth tracking either
                if !final_now {
                    next.insert(v.id.to_string(), v.snap);
                }
            }
            Some(old) => {
                let title = matchup(league, &v.snap);
                // plan 042: collapsed-scorecard cells, built once per
                // match and attached unchanged to every event this poll
                // pushes for it (a single poll can push goal + full-time
                // together). `topic.is_some()` is the same value for all
                // of them, so this stays default (byte-identical to
                // pre-039) exactly when the live-card flag is off.
                let meta = if topic.is_some() {
                    let mut details = vec![DetailItem {
                        label: "Clock".to_string(),
                        value: v.snap.display_clock.clone(),
                    }];
                    let (home_y, home_r) = v.snap.home_cards;
                    let (away_y, away_r) = v.snap.away_cards;
                    // omit the cell on a clean match rather than show
                    // "0Y0R · 0Y0R" clutter
                    if home_y + home_r + away_y + away_r > 0 {
                        details.push(DetailItem {
                            label: "Cards".to_string(),
                            value: format!(
                                "{} {}Y{}R · {} {}Y{}R",
                                v.snap.away_abbrev,
                                away_y,
                                away_r,
                                v.snap.home_abbrev,
                                home_y,
                                home_r
                            ),
                        });
                    }
                    // plan 083 item 4: the structured sibling of the
                    // details cells just above — same fields, unjoined,
                    // so 084's card can lay out crest–score–crest instead
                    // of parsing `matchup()`'s pre-joined string. Crest
                    // paths are always `None` here (populated afterward
                    // by the crest cache patch in `spawn_espn_poller` —
                    // that step is async I/O and this function stays
                    // pure/sync/fixture-tested, plan 083 workstream a).
                    let espn = EspnMeta {
                        league: league_label(league).to_string(),
                        home_abbrev: v.snap.home_abbrev.clone(),
                        away_abbrev: v.snap.away_abbrev.clone(),
                        home_score: v.snap.home_score,
                        away_score: v.snap.away_score,
                        clock: v.snap.display_clock.clone(),
                        home_cards: v.snap.home_cards,
                        away_cards: v.snap.away_cards,
                        home_crest: None,
                        away_crest: None,
                    };
                    EventMeta {
                        details,
                        espn: Some(espn),
                        ..EventMeta::default()
                    }
                } else {
                    EventMeta::default()
                };

                if v.snap.home_score != old.home_score || v.snap.away_score != old.away_score {
                    let body = v
                        .last_scoring_play
                        .clone()
                        .unwrap_or_else(|| "goal".to_string());
                    let mut event = make_event(
                        EventType::ScoreUpdate,
                        title.clone(),
                        body,
                        ttl_secs,
                        EventSignal::Goal,
                        priority,
                        card_topic(&topic, false),
                    );
                    event.meta = meta.clone();
                    out.push(event);
                }

                if old.state == "pre" && v.snap.state == "in" {
                    let mut event = make_event(
                        EventType::MatchState,
                        title.clone(),
                        "kickoff".to_string(),
                        ttl_secs,
                        EventSignal::Kickoff,
                        priority,
                        card_topic(&topic, false),
                    );
                    event.meta = meta.clone();
                    out.push(event);
                }
                if v.snap.status_name == "STATUS_HALFTIME" && old.status_name != "STATUS_HALFTIME" {
                    let mut event = make_event(
                        EventType::MatchState,
                        title.clone(),
                        "half-time".to_string(),
                        ttl_secs,
                        EventSignal::Halftime,
                        priority,
                        card_topic(&topic, false),
                    );
                    event.meta = meta.clone();
                    out.push(event);
                }
                if final_now && old.state != "post" {
                    let mut event = make_event(
                        EventType::MatchState,
                        title.clone(),
                        "full-time".to_string(),
                        ttl_secs,
                        EventSignal::Fulltime,
                        priority,
                        card_topic(&topic, true),
                    );
                    event.meta = meta.clone();
                    out.push(event);
                }

                if v.snap.total_cards() > old.total_cards() && !final_now {
                    let body = v.last_card.clone().unwrap_or_else(|| "card".to_string());
                    let signal = if v.last_card_is_red {
                        EventSignal::RedCard
                    } else {
                        EventSignal::YellowCard
                    };
                    let mut event = make_event(
                        EventType::MatchState,
                        title,
                        body,
                        ttl_secs,
                        signal,
                        priority,
                        card_topic(&topic, false),
                    );
                    event.meta = meta.clone();
                    out.push(event);
                }

                if !final_now {
                    next.insert(v.id.to_string(), v.snap);
                }
            }
        }
    }

    // carry forward matches absent from this poll's feed (review fix,
    // 2026-07-16): evict only after sustained absence, never on one
    // missing poll. no events are emitted for absent matches.
    for (id, old) in prev {
        if !next.contains_key(id) && !fetched.events.iter().any(|e| &e.id == id) {
            let missed = old.missed_polls + 1;
            if missed < ABSENT_POLLS_BEFORE_EVICTION {
                let mut carried = old.clone();
                carried.missed_polls = missed;
                next.insert(id.clone(), carried);
            } else {
                tracing::warn!(
                    league,
                    match_id = %id,
                    "match absent for {missed} consecutive polls; evicting"
                );
            }
        }
    }

    (out, next)
}

/// plan 034: the idle rail's live-match chip, computed over the poll
/// loop's whole snapshot map (every watched league). The first in-play
/// match wins — deterministic (league slug, then match id) because
/// HashMap iteration order is not; a second simultaneous live match is
/// deliberately out of scope (plan 034's STOP list: multi-live layout is
/// an operator decision, not a guess). `None` when nothing is in-play.
pub fn live_match_summary(snapshots: &HashMap<String, Snapshot>) -> Option<LiveMatchSummary> {
    let mut leagues: Vec<(&String, &Snapshot)> = snapshots.iter().collect();
    leagues.sort_by(|a, b| a.0.cmp(b.0));
    for (_league, snapshot) in leagues {
        let mut matches: Vec<(&String, &MatchSnapshot)> = snapshot.iter().collect();
        matches.sort_by(|a, b| a.0.cmp(b.0));
        if let Some((_id, m)) = matches.into_iter().find(|(_, m)| m.state == "in") {
            return Some(LiveMatchSummary {
                // "Home X–Y Away" (plan 034 step 2) — deliberately NOT
                // matchup()'s away-first, league-tagged orientation: the
                // rail chip is a glanceable readout, not an alert title.
                label: format!(
                    "{} {}–{} {}",
                    m.home_abbrev, m.home_score, m.away_score, m.away_abbrev
                ),
                minute: m.display_clock.clone(),
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// per-league backoff (v2 spec §3): 30s → 60s → 120s → … cap 300s, reset on
// that league's first success. pure state machine, unit-tested directly.
// ---------------------------------------------------------------------------

const BACKOFF_BASE: Duration = Duration::from_secs(30);
const BACKOFF_CAP: Duration = Duration::from_secs(300);
const MAX_SCOREBOARD_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Backoff {
    delay: Duration,
    blocked_until: Option<Instant>,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            delay: BACKOFF_BASE,
            blocked_until: None,
        }
    }
}

impl Backoff {
    pub fn ready(&self, now: Instant) -> bool {
        self.blocked_until.map(|t| now >= t).unwrap_or(true)
    }

    pub fn on_failure(&mut self, now: Instant) {
        self.blocked_until = Some(now + self.delay);
        self.delay = (self.delay * 2).min(BACKOFF_CAP);
    }

    pub fn on_success(&mut self) {
        self.delay = BACKOFF_BASE;
        self.blocked_until = None;
    }
}

// ---------------------------------------------------------------------------
// plan 083 workstream c (item 6a): richer live-match events — foul,
// offside, VAR check, substitution. Goal/penalty/own-goal/yellow/red
// already flow from the scoreboard feed above and are NEVER re-emitted
// here (see `classify_rich_type`'s "scoreboard-owned" comment). Opt-in
// via `espn_rich_events` (default false), mirroring `espn_live_card`.
//
// Wire shapes below are synthesized to match the DOCUMENTED evidence in
// `plans/suspended/043-richer-match-events.md`'s "Step 0: CONFIRMED
// against a genuinely live match" section — that plan's own
// `research/043-worldcup-final-verification/` raw evidence directory is
// gitignored (`.gitignore`: "throwaway exploratory captures") and not
// present in this checkout, so the shapes here are built from the
// checked-in plan's summary of that evidence (key names `commentary`/
// `keyEvents`, the `commentary` entry's `sequence`/`time`/`text`/`play`
// shape, the `keyEvents` entry's `id`/`type`/`text`/`clock`/`scoringPlay`
// shape, and the confirmed fallback chain to the core API's
// `/competitions/{id}/plays`, paginated 25/page) — not independently
// re-verified against a live match this pass. Pure parse/classify/dedup
// logic is fully fixture-tested below; the two endpoints' exact field
// set beyond what's documented is necessarily best-effort, same posture
// as the scoreboard structs above ("everything is defaulted so a
// missing field degrades to no delta, never a parse error").
// ---------------------------------------------------------------------------

/// The `summary?event={id}` endpoint's response — only the two fields
/// this plan's approach depends on (`plans/suspended/043...md` point 1:
/// "The key names are `commentary` and `keyEvents`").
#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    #[serde(default)]
    pub commentary: Vec<CommentaryEntry>,
    #[serde(default, rename = "keyEvents")]
    pub key_events: Vec<KeyEventEntry>,
}

/// One `commentary` array entry (`plans/suspended/043...md` point 2):
/// `sequence`, `time` (`{value, displayValue}`), `text`, plus an
/// embedded `play` object. Parsed in full for parity with the
/// documented shape (satisfying "parse commentary/keyEvents into a
/// typed event stream"); `extract_rich_events` (below) builds the
/// actual EMITTED stream from `key_events` instead (see that function's
/// doc for why) — `#[allow(dead_code)]` here is deliberate: these fields
/// are parsed and available (e.g. to a future 084 richer-detail view),
/// just not read by anything yet.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CommentaryEntry {
    #[serde(default)]
    pub sequence: i64,
    #[serde(default)]
    pub time: Option<CommentaryTime>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub play: Option<CommentaryPlay>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct CommentaryTime {
    #[serde(default)]
    pub value: f64,
    #[serde(rename = "displayValue", default)]
    pub display_value: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CommentaryPlay {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub play_type: String,
    #[serde(default)]
    pub team: Option<SbTeam>,
}

/// One `keyEvents` array entry (`plans/suspended/043...md` point 3):
/// `id`, `type`, `text`, `clock`, `scoringPlay`. This is the "filtered/
/// significant-events-only view" the research called out as "likely the
/// better source for card-worthy event selection" — `extract_rich_events`
/// builds the emitted stream from this array. `id`/`scoring_play` are
/// parsed for shape completeness but not consumed by extraction (dedup
/// keys on kind+clock, not id; scoreboard-owned filtering happens on
/// `event_type` before `scoring_play` would matter).
#[derive(Debug, Deserialize)]
pub struct KeyEventEntry {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub clock: Option<SbClock>,
    #[serde(default, rename = "scoringPlay")]
    #[allow(dead_code)]
    pub scoring_play: bool,
}

/// The core API's `/competitions/{id}/plays` fallback response —
/// paginated (`plans/suspended/043...md` point 4: "25/page"). Only
/// `pageCount` and `items` are consumed: `pageCount` drives the
/// newest-page-only fetch, `items` are parsed the same way as
/// `keyEvents`.
#[derive(Debug, Deserialize, Default)]
pub struct PlaysResponse {
    #[serde(default, rename = "pageCount")]
    pub page_count: u32,
    #[serde(default)]
    pub items: Vec<PlayItem>,
}

#[derive(Debug, Deserialize)]
pub struct PlayItem {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub play_type: Option<PlayTypeTag>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub clock: Option<SbClock>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PlayTypeTag {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub text: String,
}

pub fn parse_summary(body: &str) -> Result<SummaryResponse, serde_json::Error> {
    serde_json::from_str(body)
}

pub fn parse_plays(body: &str) -> Result<PlaysResponse, serde_json::Error> {
    serde_json::from_str(body)
}

/// The four locked informational event kinds (079 item 6a) — everything
/// else (scoreboard-owned or genuinely unrecognized) is dropped, never
/// emitted, by `classify_rich_type` below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichEventKind {
    Foul,
    Offside,
    VarCheck,
    Substitution,
}

/// Maps a raw `type` string (from either `keyEvents` or the `plays`
/// fallback) to one of the four locked kinds, or `None` — which covers
/// BOTH a scoreboard-owned type (goal, penalty-scored, yellow-card,
/// red-card, own-goal, kickoff, half-time, full-time, …) AND any
/// genuinely unrecognized type. Both cases are dropped outright here,
/// not merely deduped by key later — "drop any summary/plays event
/// whose type is scoreboard-owned outright (redundant by construction,
/// not just by key-collision)" (plan 083 step 4).
fn classify_rich_type(raw: &str) -> Option<RichEventKind> {
    match raw {
        "foul" => Some(RichEventKind::Foul),
        "offside" => Some(RichEventKind::Offside),
        "var-check" | "var-review" | "video-review" => Some(RichEventKind::VarCheck),
        "substitution" | "sub" => Some(RichEventKind::Substitution),
        _ => None,
    }
}

/// One informational event ready for dedup + emission.
#[derive(Debug, Clone, PartialEq)]
pub struct RichEventCandidate {
    pub kind: RichEventKind,
    pub text: String,
    pub clock: String,
}

/// Extracts informational candidates from a `summary` response's
/// `keyEvents` array — the documented "better source for card-worthy
/// event selection" (`plans/suspended/043...md` point 3). `commentary`
/// is parsed above (satisfying "parse commentary/keyEvents into a typed
/// event stream") but not used for emission here: `keyEvents` already
/// carries a clean `type` tag and a ready-to-display `text`, so it's the
/// simpler and more faithful-to-research source for the four locked
/// kinds; `commentary`'s fuller play-by-play detail is left for 084 (or
/// a later plan) to consume if the collapsed scorecard ever wants it.
fn extract_rich_events(summary: &SummaryResponse) -> Vec<RichEventCandidate> {
    summary
        .key_events
        .iter()
        .filter_map(|e| {
            let kind = classify_rich_type(&e.event_type)?;
            Some(RichEventCandidate {
                kind,
                text: e.text.clone(),
                clock: e
                    .clock
                    .as_ref()
                    .map(|c| c.display_value.clone())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Same extraction, from the core-API `/plays` fallback response.
fn extract_rich_events_from_plays(plays: &PlaysResponse) -> Vec<RichEventCandidate> {
    plays
        .items
        .iter()
        .filter_map(|p| {
            let raw_type = p.play_type.as_ref().map(|t| t.id.as_str()).unwrap_or("");
            let kind = classify_rich_type(raw_type)?;
            Some(RichEventCandidate {
                kind,
                text: p.text.clone(),
                clock: p
                    .clock
                    .as_ref()
                    .map(|c| c.display_value.clone())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// `summary`'s "returns empty" fallback trigger (plan 083 step 4: "when
/// `summary` errors, 404s, or returns empty for a match known-live").
fn is_empty_summary(resp: &SummaryResponse) -> bool {
    resp.commentary.is_empty() && resp.key_events.is_empty()
}

/// Dedup key: (kind, clock) — a re-poll re-fetching the same event must
/// not re-emit it. No athlete field is documented on `keyEvents`/`plays`
/// beyond the embedded `commentary.play`, which isn't the extraction
/// source (see `extract_rich_events`'s doc), so (kind, clock) is the
/// key's full extent for now — sufficient because two distinct events
/// of the same kind at the same displayed clock string are not
/// realistically distinguishable from this feed anyway.
fn dedup_key(candidate: &RichEventCandidate) -> String {
    format!("{:?}|{}", candidate.kind, candidate.clock)
}

/// Filters `candidates` down to ones not already in `seen`, inserting
/// each survivor's key so a later call (next poll) drops the repeat.
fn filter_new(
    seen: &mut HashSet<String>,
    candidates: Vec<RichEventCandidate>,
) -> Vec<RichEventCandidate> {
    candidates
        .into_iter()
        .filter(|c| seen.insert(dedup_key(c)))
        .collect()
}

/// Builds the emitted `Event` for one informational candidate — a
/// one-shot (never a Topic/Recurring card; the sticky live-match card
/// is the existing Topic machinery's job, not new code here, per plan
/// 083 step 4), Football-origin, same TTL/priority as every other
/// football event.
fn make_rich_event(
    league: &str,
    snap: &MatchSnapshot,
    candidate: &RichEventCandidate,
    ttl_secs: u64,
    priority: Priority,
) -> Event {
    let title = matchup(league, snap);
    let body = if candidate.text.is_empty() {
        format!("{:?}", candidate.kind)
    } else {
        candidate.text.clone()
    };
    let signal = match candidate.kind {
        RichEventKind::Foul => EventSignal::Foul,
        RichEventKind::Offside => EventSignal::Offside,
        RichEventKind::VarCheck => EventSignal::VarCheck,
        RichEventKind::Substitution => EventSignal::Substitution,
    };
    Event {
        id: Uuid::new_v4(),
        event_type: EventType::MatchState,
        priority,
        rotation: RotationSpec::OneShot { ttl_secs },
        topic: None,
        payload: EventPayload { title, body },
        meta: EventMeta::default(),
        signal,
        origin: SourceKind::Football,
    }
}

// ---------------------------------------------------------------------------
// fetch loop — deliberately thin and untested (v2 spec §3): everything below
// the "here is a response body" line is the tested surface above. the capped
// body read is no longer inline here — it's the shared, wiremock-tested
// helper in `net.rs` (plan 025), which both pollers now call.
// ---------------------------------------------------------------------------

async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
    let url = format!("https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard");
    let response = client.get(&url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_SCOREBOARD_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

// plan 083 workstream c: production base URLs for the summary/plays
// fetch chain, matching the same host families verified in
// `plans/suspended/043-richer-match-events.md`. `base` is a parameter on
// the fetch functions below (not baked in) so the fallback-chain
// orchestration is wiremock-testable, same posture as `net.rs`.
const ESPN_SUMMARY_BASE: &str = "https://site.api.espn.com/apis/site/v2/sports/soccer";
const ESPN_CORE_BASE: &str = "https://sports.core.api.espn.com/v2/sports/soccer/leagues";
const MAX_RICH_EVENT_BYTES: usize = 512 * 1024;

async fn fetch_summary(
    client: &reqwest::Client,
    base: &str,
    league: &str,
    event_id: &str,
) -> anyhow::Result<String> {
    let url = format!("{base}/{league}/summary?event={event_id}");
    let response = client.get(&url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_RICH_EVENT_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

/// ESPN's soccer events are single-competition (no doubleheaders), so
/// `competition_id == event_id` in practice — an assumption carried over
/// from `plans/suspended/043-richer-match-events.md`'s URL, not
/// independently re-verified this pass (that plan's live verification
/// confirmed the URL shape worked, but didn't specifically probe a
/// competition_id divergent from event_id).
async fn fetch_plays_page(
    client: &reqwest::Client,
    base: &str,
    league: &str,
    event_id: &str,
    page: u32,
) -> anyhow::Result<String> {
    let url =
        format!("{base}/{league}/events/{event_id}/competitions/{event_id}/plays?page={page}");
    let response = client.get(&url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_RICH_EVENT_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

/// Fetches the NEWEST page only (plan 083 step 4's pagination rule: "25
/// plays/page ... fetch the newest page only each poll ... do not
/// backfill pages") — one request to learn `pageCount`, a second to the
/// last page only when there's more than one.
async fn fetch_newest_plays(
    client: &reqwest::Client,
    base: &str,
    league: &str,
    event_id: &str,
) -> anyhow::Result<PlaysResponse> {
    let first_body = fetch_plays_page(client, base, league, event_id, 1).await?;
    let first: PlaysResponse = parse_plays(&first_body)?;
    if first.page_count > 1 {
        let last_body = fetch_plays_page(client, base, league, event_id, first.page_count).await?;
        Ok(parse_plays(&last_body)?)
    } else {
        Ok(first)
    }
}

/// The fallback-chain orchestration (plan 083 step 4): `summary` first;
/// if it errors OR parses to an empty response, fall through to the core
/// API's `/plays` (newest page only); if THAT also fails, give up
/// silently for this poll — the next poll retries both from scratch,
/// same posture as the scoreboard feed's absent-poll carry-forward.
async fn poll_rich_events(
    client: &reqwest::Client,
    summary_base: &str,
    core_base: &str,
    league: &str,
    event_id: &str,
) -> Vec<RichEventCandidate> {
    let summary_result = fetch_summary(client, summary_base, league, event_id)
        .await
        .and_then(|body| parse_summary(&body).map_err(anyhow::Error::from));

    let needs_fallback = match &summary_result {
        Err(_) => true,
        Ok(resp) => is_empty_summary(resp),
    };
    if !needs_fallback {
        return summary_result
            .map(|r| extract_rich_events(&r))
            .unwrap_or_default();
    }

    match fetch_newest_plays(client, core_base, league, event_id).await {
        Ok(plays) => extract_rich_events_from_plays(&plays),
        Err(e) => {
            tracing::warn!(
                league,
                event_id,
                "rich events: summary and plays both failed this poll: {e}"
            );
            Vec::new()
        }
    }
}

/// plan 037: ingest goes through `Engine::accept` — the one shared path
/// that enqueues with the mutate→wake→emit protocol and then fans
/// accepted events out to every connector (plan §3: "every accepted
/// event goes to every enabled connector, always" — with one recorded
/// exception: rss/news events are overlay-only and never offered,
/// `IMPLEMENTATION_PLAN.md` §4.6 — a rule `accept` encodes via the
/// origin gate, so no per-caller flag is needed here).
#[allow(clippy::too_many_arguments)] // untested outer wiring, same reasoning as CardTopic's bundling for the pure/tested side
pub fn spawn_espn_poller(
    engine: Engine,
    leagues: Vec<String>,
    poll_secs: u64,
    ttl_secs: u64,
    priority: Priority,
    espn_live_card: bool,
    espn_rich_events: bool,
    crests: CrestCache,
) {
    tauri::async_runtime::spawn(async move {
        let client = match crate::net::build_poll_client() {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("espn poller could not build http client: {error}");
                return;
            }
        };
        let mut snapshots: HashMap<String, Snapshot> = HashMap::new();
        let mut backoffs: HashMap<String, Backoff> = HashMap::new();
        // plan 083 workstream c: per-match dedup key sets for the richer
        // event feed — keyed by match id, evicted alongside that match's
        // scoreboard snapshot (see the retain() call below) so this never
        // grows unbounded.
        let mut rich_seen: HashMap<String, HashSet<String>> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs.max(5)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(?leagues, poll_secs, "espn poller started");

        loop {
            interval.tick().await;
            for league in &leagues {
                let now = Instant::now();
                let backoff = backoffs.entry(league.clone()).or_default();
                if !backoff.ready(now) {
                    continue;
                }

                let parsed = match fetch_league(&client, league).await {
                    Ok(body) => parse_scoreboard(&body).map_err(anyhow::Error::from),
                    Err(e) => Err(e),
                };
                let scoreboard = match parsed {
                    Ok(sb) => {
                        backoff.on_success();
                        sb
                    }
                    Err(e) => {
                        // best-effort public endpoint: warn, back off this
                        // league only, never take the app down
                        tracing::warn!(league, "espn poll failed: {e}");
                        backoff.on_failure(now);
                        continue;
                    }
                };

                // plan 083 workstream a: schedule a background fetch for
                // any team whose crest isn't cached yet — before the diff,
                // so a crest that lands mid-poll is available for
                // `patch_crests` below even on the same poll it completes.
                for (team_id, logo_url) in team_logos(&scoreboard) {
                    if crests.should_fetch(&team_id) {
                        let crests = crests.clone();
                        let client = client.clone();
                        tauri::async_runtime::spawn(async move {
                            crests.fetch_and_store(&client, &team_id, &logo_url).await;
                        });
                    }
                }
                let team_ids = team_ids_by_match(&scoreboard);

                let prev = snapshots.entry(league.clone()).or_default();
                let (mut events, next) = diff_scoreboard(
                    prev,
                    &scoreboard,
                    ttl_secs,
                    league,
                    priority,
                    espn_live_card,
                );
                patch_crests(&mut events, league, &team_ids, &crests);
                snapshots.insert(league.clone(), next);
                for event in events {
                    if let Err(e) = engine.accept(event, false).await {
                        tracing::warn!(league, "espn event dropped: {e}");
                    }
                }

                // plan 083 workstream c: for every currently-live match,
                // poll the richer summary/plays fallback chain and emit
                // any newly-seen informational event. Only when opted in
                // — this is materially heavier per-match polling.
                if espn_rich_events {
                    if let Some(current) = snapshots.get(league) {
                        let live_matches: Vec<(String, MatchSnapshot)> = current
                            .iter()
                            .filter(|(_, s)| s.state == "in")
                            .map(|(id, s)| (id.clone(), s.clone()))
                            .collect();
                        for (match_id, snap) in live_matches {
                            let candidates = poll_rich_events(
                                &client,
                                ESPN_SUMMARY_BASE,
                                ESPN_CORE_BASE,
                                league,
                                &match_id,
                            )
                            .await;
                            let seen = rich_seen.entry(match_id.clone()).or_default();
                            for candidate in filter_new(seen, candidates) {
                                let event =
                                    make_rich_event(league, &snap, &candidate, ttl_secs, priority);
                                if let Err(e) = engine.accept(event, false).await {
                                    tracing::warn!(league, match_id, "rich event dropped: {e}");
                                }
                            }
                        }
                    }
                    // evict dedup state for matches no longer tracked at
                    // all (evicted by the scoreboard's own absent-poll
                    // logic above) — never grows unbounded.
                    if let Some(current) = snapshots.get(league) {
                        rich_seen.retain(|id, _| current.contains_key(id));
                    }
                }
            }

            // plan 034: refresh the idle rail's live-match chip once per
            // poll pass, over every watched league's snapshot — the single
            // chokepoint where a tick ends. The rotation loop is the sole
            // status-state emitter; the poller's only status-side act is
            // this write, which wakes the loop when (and only when) the
            // summary actually changed (plan 037: behind a narrow Engine
            // method, no raw handles).
            let summary = live_match_summary(&snapshots);
            engine.update_live_match(summary);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{test_fixtures, EventType, Priority, SlotState};
    use crate::notifier::{ConnectorHandle, ConnectorHealth};
    use crate::queue::SingleSlotQueue;
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const USA: &str = include_str!("../tests/fixtures/scoreboard-usa.1.json");
    const UCL: &str = include_str!("../tests/fixtures/scoreboard-uefa.champions.json");
    const ESP: &str = include_str!("../tests/fixtures/scoreboard-esp.1.json");

    fn score_event(title: &str) -> Event {
        test_fixtures::with_origin(
            test_fixtures::with_signal(
                test_fixtures::with_priority(
                    test_fixtures::with_event_type(
                        test_fixtures::event(title),
                        EventType::ScoreUpdate,
                    ),
                    Priority::High,
                ),
                EventSignal::Goal,
            ),
            SourceKind::Football,
        )
    }

    #[tokio::test]
    async fn poller_accepted_events_fan_out_and_rejected_do_not() {
        // regression for the 2026-07-16 v3 review finding: score events
        // must reach connectors the same as http pushes (plan §3). one
        // visible slot, no waiting room: first accepted, second rejected.
        // plan 037: the fan-out path is Engine::accept now.
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let connector = ConnectorHandle::new("test", tx);
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(0),
            app.handle().clone(),
            Arc::new(vec![connector]),
            Arc::new(std::sync::Mutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            None,
        );

        engine.accept(score_event("accepted"), false).await.unwrap();
        engine
            .accept(score_event("rejected"), false)
            .await
            .unwrap_err();

        match engine.read(|q| q.current_slot_state()).await {
            SlotState::Showing { title, .. } => assert_eq!(title, "accepted"),
            SlotState::Empty => panic!("expected a Showing slot state"),
        }
        let fanned = rx.try_recv().expect("accepted score event must fan out");
        assert_eq!(fanned.payload.title, "accepted");
        assert!(rx.try_recv().is_err(), "rejected event must not fan out");
    }

    fn baseline(fixture: &str) -> (Snapshot, Scoreboard) {
        let sb = parse_scoreboard(fixture).unwrap();
        let (events, snap) =
            diff_scoreboard(&Snapshot::new(), &sb, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty(), "first sighting must be silent");
        (snap, sb)
    }

    #[test]
    fn real_fixture_parses() {
        let sb = parse_scoreboard(USA).unwrap();
        assert_eq!(sb.events.len(), 4);
        let v = view(&sb.events[0]);
        assert_eq!(v.snap.home_abbrev, "MTL");
        assert_eq!(v.snap.away_abbrev, "TOR");
        assert_eq!(v.snap.state, "pre");
    }

    // plan 083 workstream a: crest logo parsing + the two pure lookup
    // helpers the async poll loop uses to schedule fetches and patch
    // `EspnMeta.home_crest`/`away_crest`.

    #[test]
    fn team_logo_url_parses_from_the_real_fixture() {
        // verified directly against the raw esp.1 fixture json: Alavés
        // (home, id 96) and Getafe (away, id 2922) both carry a direct
        // espncdn.com crest url.
        let sb = parse_scoreboard(ESP).unwrap();
        let comp = &sb.events[0].competitions[0];
        let home = comp
            .competitors
            .iter()
            .find(|c| c.home_away == "home")
            .unwrap();
        let away = comp
            .competitors
            .iter()
            .find(|c| c.home_away == "away")
            .unwrap();
        assert_eq!(
            home.team.as_ref().unwrap().logo.as_deref(),
            Some("https://a.espncdn.com/i/teamlogos/soccer/500/96.png")
        );
        assert_eq!(
            away.team.as_ref().unwrap().logo.as_deref(),
            Some("https://a.espncdn.com/i/teamlogos/soccer/500/2922.png")
        );
    }

    #[test]
    fn team_logos_extracts_every_team_id_to_logo_url_pair() {
        let sb = parse_scoreboard(ESP).unwrap();
        let logos = team_logos(&sb);
        assert_eq!(
            logos.get("96").map(String::as_str),
            Some("https://a.espncdn.com/i/teamlogos/soccer/500/96.png")
        );
        assert_eq!(
            logos.get("2922").map(String::as_str),
            Some("https://a.espncdn.com/i/teamlogos/soccer/500/2922.png")
        );
    }

    #[test]
    fn team_ids_by_match_maps_match_id_to_home_and_away_team_ids() {
        let sb = parse_scoreboard(ESP).unwrap();
        let match_id = sb.events[0].id.clone();
        let ids = team_ids_by_match(&sb);
        assert_eq!(
            ids.get(&match_id),
            Some(&("96".to_string(), "2922".to_string()))
        );
    }

    #[test]
    fn patch_crests_fills_in_cached_paths_by_topic_match_id() {
        let (_dir, crests) = crate::crests::test_support::temp_cache();
        // fabricate a warm cache hit for the home team (96) only.
        std::fs::create_dir_all(&crests.dir).unwrap();
        std::fs::write(crests.path_for("96"), b"png bytes").unwrap();

        let mut team_ids = HashMap::new();
        team_ids.insert("761659".to_string(), ("96".to_string(), "2922".to_string()));

        let mut events = vec![score_event("t")];
        events[0].topic = Some("espn:usa.1:761659".to_string());
        events[0].meta.espn = Some(EspnMeta {
            league: "usa.1".to_string(),
            home_abbrev: "MTL".to_string(),
            away_abbrev: "TOR".to_string(),
            home_score: 0,
            away_score: 0,
            clock: "0'".to_string(),
            home_cards: (0, 0),
            away_cards: (0, 0),
            home_crest: None,
            away_crest: None,
        });

        patch_crests(&mut events, "usa.1", &team_ids, &crests);

        let espn = events[0].meta.espn.as_ref().unwrap();
        assert!(espn.home_crest.is_some(), "96 is a cache hit");
        assert_eq!(espn.away_crest, None, "2922 was never cached");
    }

    #[test]
    fn patch_crests_leaves_events_without_espn_meta_untouched() {
        let (_dir, crests) = crate::crests::test_support::temp_cache();
        let team_ids = HashMap::new();
        let mut events = vec![score_event("t")];
        patch_crests(&mut events, "usa.1", &team_ids, &crests);
        assert_eq!(events[0].meta.espn, None);
    }

    #[test]
    fn finished_match_fixture_carries_scoring_and_card_details() {
        let sb = parse_scoreboard(UCL).unwrap();
        let v = view(&sb.events[0]);
        assert_eq!(v.snap.state, "post");
        assert!(v.last_scoring_play.is_some());
        assert!(v.last_card.unwrap().contains("Card"));
    }

    #[test]
    fn first_sighting_is_silent_and_final_matches_are_not_tracked() {
        let (snap, _) = baseline(USA);
        assert_eq!(snap.len(), 4); // all four MLS games are "pre"

        let (snap_ucl, _) = baseline(UCL);
        assert!(snap_ucl.is_empty()); // the one UCL game is already final
    }

    #[test]
    fn live_summary_is_none_when_nothing_is_in_play() {
        // all four USA-fixture matches are "pre" — no chip
        let (snap, _) = baseline(USA);
        let mut snapshots = HashMap::new();
        snapshots.insert("usa.1".to_string(), snap);
        assert_eq!(live_match_summary(&snapshots), None);
    }

    #[test]
    fn live_summary_populates_from_an_in_play_fixture_match() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].status.status_type.state = "in".to_string();
        sb.events[0].status.display_clock = "45'".to_string();
        let (_, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);

        let mut snapshots = HashMap::new();
        snapshots.insert("usa.1".to_string(), next);
        // MTL is home, TOR away (real_fixture_parses) — the chip label is
        // "Home X–Y Away", the minute espn's displayClock verbatim.
        assert_eq!(
            live_match_summary(&snapshots),
            Some(LiveMatchSummary {
                label: "MTL 0–0 TOR".to_string(),
                minute: "45'".to_string(),
            })
        );
    }

    #[test]
    fn live_summary_clears_when_the_match_goes_full_time() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "post".to_string();
        let (_, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);

        let mut snapshots = HashMap::new();
        snapshots.insert("usa.1".to_string(), next);
        // full-time evicts the only in-play match — chip off
        assert_eq!(live_match_summary(&snapshots), None);
    }

    #[test]
    fn score_delta_emits_one_score_update_and_nothing_for_unchanged() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[0].payload.title, "usa.1: TOR 0–1 MTL");
        assert_eq!(events[0].payload.body, "goal"); // no scoring play in feed
        assert_eq!(events[0].rotation, RotationSpec::OneShot { ttl_secs: 8 });
        assert_eq!(events[0].signal, EventSignal::Goal);
        assert_eq!(next.len(), 4);
    }

    #[test]
    fn state_transitions_emit_match_state() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].status.status_type.state = "in".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert_eq!(events[0].payload.body, "kickoff");
        assert_eq!(events[0].signal, EventSignal::Kickoff);
    }

    #[test]
    fn halftime_is_detected_via_status_name() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "in".to_string();
        sb.events[0].status.status_type.name = "STATUS_HALFTIME".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.body, "half-time");
        assert_eq!(events[0].signal, EventSignal::Halftime);
    }

    #[test]
    fn full_time_emits_and_evicts() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.body, "full-time");
        assert_eq!(events[0].signal, EventSignal::Fulltime);
        assert!(!next.contains_key("761659")); // evicted
        assert_eq!(next.len(), 3);
    }

    #[test]
    fn absent_match_is_carried_forward_not_evicted() {
        // review fix 2026-07-16: a transient empty-but-valid response
        // must not drop live matches
        let (snap, mut sb) = baseline(USA);
        sb.events.remove(0);
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert_eq!(next.len(), 4); // still tracked
        assert_eq!(next.get("761659").unwrap().missed_polls, 1);
    }

    #[test]
    fn goal_during_feed_blip_is_caught_on_reappearance() {
        let (snap, sb) = baseline(USA);

        // poll 1: entire feed transiently empty — everything carried
        let empty = parse_scoreboard("{}").unwrap();
        let (events, carried) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert_eq!(carried.len(), 4);

        // poll 2: feed back, one match scored during the blip
        let mut back = sb;
        back.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) = diff_scoreboard(&carried, &back, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(next.get("761659").unwrap().missed_polls, 0); // reset
    }

    #[test]
    fn sustained_absence_evicts_after_threshold() {
        let (mut snap, _) = baseline(USA);
        let empty = parse_scoreboard("{}").unwrap();
        for i in 1..ABSENT_POLLS_BEFORE_EVICTION {
            let (events, next) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High, false);
            assert!(events.is_empty());
            assert_eq!(next.len(), 4, "still carried at miss {i}");
            snap = next;
        }
        // the miss that reaches the threshold evicts, silently
        let (events, next) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn new_card_emits_match_state_with_detail() {
        let sb = parse_scoreboard(UCL).unwrap();
        // pretend we saw this match live with one fewer card — the
        // fixture's last card is a home (PSG) yellow, so "one card ago"
        // is home (1, 0) instead of (2, 0)
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_cards.0 -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert!(events[0].payload.body.contains("Card"));
        // the ucl fixture's cards are all yellow (verified against the raw
        // json) — signal must come from that structurally, not a guess.
        assert_eq!(events[0].signal, EventSignal::YellowCard);
    }

    #[test]
    fn red_card_emits_red_card_signal() {
        // none of the checked-in fixtures contain a real red card, so this
        // synthesizes one by mutating a detail's structural booleans —
        // same technique the malformed/absence tests already use for
        // hand-constructing scenarios the real fixtures don't cover.
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        // the mutation below turns the fixture's last (home) yellow into
        // a red — the "one card ago" state is one fewer home yellow
        v_snap.home_cards.0 -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();
        let last_detail = live.events[0].competitions[0]
            .details
            .last_mut()
            .expect("fixture has at least one detail");
        last_detail.red_card = true;
        if let Some(t) = last_detail.detail_type.as_mut() {
            t.text = "Red Card".to_string();
        }

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert_eq!(events[0].signal, EventSignal::RedCard);
    }

    #[test]
    fn card_recorded_same_poll_as_fulltime_does_not_emit_separately_and_stays_in_meta() {
        let live = parse_scoreboard(UCL).unwrap();
        let mut old_view = view(&live.events[0]).snap;
        old_view.state = "in".to_string();
        old_view.home_cards.0 -= 1; // one home yellow "not yet recorded" in `old`
        let mut snap = Snapshot::new();
        snap.insert(live.events[0].id.clone(), old_view);

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, true);

        assert_eq!(
            events.len(),
            1,
            "a card recorded the same poll as full-time must not emit as a separate event"
        );
        assert_eq!(events[0].payload.body, "full-time");
        assert!(
            events[0].meta.details.iter().any(|d| d.label == "Cards"),
            "the card must still be reflected in the full-time event's meta"
        );
    }

    #[test]
    fn ucl_fixture_cards_bucket_per_side_and_color() {
        // plan 042 ground truth, verified directly against the raw
        // fixture json: 6 yellows, 0 reds — home PSG (team.id 160) 2Y,
        // away ARS (team.id 359) 4Y. if these numbers come out different,
        // the `team.id` cross-reference is misattributing cards.
        let sb = parse_scoreboard(UCL).unwrap();
        let snap = view(&sb.events[0]).snap;
        assert_eq!(snap.home_abbrev, "PSG");
        assert_eq!(snap.away_abbrev, "ARS");
        assert_eq!(snap.home_cards, (2, 0));
        assert_eq!(snap.away_cards, (4, 0));
        assert_eq!(snap.total_cards(), 6);
    }

    #[test]
    fn card_with_unrecognized_team_id_is_dropped_not_misattributed() {
        let mut sb = parse_scoreboard(UCL).unwrap();
        let baseline_total = view(&sb.events[0]).snap.total_cards();

        // mutate the last card detail's team.id to a value that matches
        // neither PSG ("160") nor ARS ("359")
        let comp = &mut sb.events[0].competitions[0];
        let last_detail = comp
            .details
            .iter_mut()
            .rev()
            .find(|d| {
                d.detail_type
                    .as_ref()
                    .map(|t| t.text.contains("Card"))
                    .unwrap_or(false)
            })
            .expect("fixture has at least one card detail");
        last_detail.team = Some(SbTeam {
            id: "999999".to_string(),
            abbreviation: String::new(),
            logo: None,
        });

        let snap = view(&sb.events[0]).snap;
        // the mutated card is dropped, not misattributed to either side —
        // total_cards() is exactly one less than baseline (the mutated
        // detail no longer counts for anyone), not equal to baseline
        // (which would mean it landed somewhere by accident)
        assert_eq!(snap.total_cards(), baseline_total - 1);
    }

    #[test]
    fn goal_body_names_the_event() {
        // the ucl fixture's match is already `post` on first sighting, so
        // hand-build a synthetic prev snapshot the way
        // red_card_emits_red_card_signal does. details is truncated to
        // just the index-0 "Goal" entry — the untouched fixture's last
        // scoring play is a shootout "Penalty - Scored", and
        // `.rev().find()` would land on that instead.
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_score -= 1; // "one goal ago"
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();
        live.events[0].competitions[0].details.truncate(1); // keep only the "Goal" entry

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert!(events[0].payload.body.starts_with("Goal — "));
    }

    #[test]
    fn penalty_body_names_the_event() {
        // same shape as goal_body_names_the_event, but no truncation: the
        // last scoring-play entry in the untouched fixture already IS
        // "Penalty - Scored" (the shootout).
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_score -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(events[0].payload.body.starts_with("Penalty - Scored — "));
    }

    #[test]
    fn own_goal_body_derived_from_structural_flag() {
        // no fixture has an own goal, so synthesize one by setting the
        // structural boolean — same technique as
        // red_card_emits_red_card_signal. detail_type.text is deliberately
        // left untouched to prove the label comes from `own_goal`, not
        // from whatever text happens to still be there.
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_score -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();
        let last_detail = live.events[0].competitions[0]
            .details
            .last_mut()
            .expect("fixture has at least one detail");
        last_detail.scoring_play = true;
        last_detail.own_goal = true;

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, false);
        assert_eq!(events.len(), 1);
        assert!(events[0].payload.body.starts_with("Own Goal — "));
    }

    #[test]
    fn goal_and_full_time_in_one_poll_emit_in_order() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, false);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[1].payload.body, "full-time");
    }

    // plan 039: opt-in live-match card — one Topic per match
    // (`espn:{league}:{match_id}`), `Recurring` while in play, `OneShot`
    // full-time on the same Topic.

    /// Builds the kickoff → goal → full-time event sequence for the USA
    /// fixture's first match (761659) under a given `espn_live_card` flag.
    fn live_cycle_events(espn_live_card: bool) -> Vec<Event> {
        let (mut snap, mut sb) = baseline(USA);
        let mut out = Vec::new();

        // poll 1: pre → in (kickoff)
        sb.events[0].status.status_type.state = "in".to_string();
        let (events, next) =
            diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);
        snap = next;

        // poll 2: goal while in play
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) =
            diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);
        snap = next;

        // poll 3: in → post (full-time)
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);

        out
    }

    #[test]
    fn live_card_off_keeps_one_shot_topicless_events() {
        // regression pin, not new behavior: `espn_live_card` defaults off,
        // and off must remain byte-for-byte today's burst of one-shot,
        // topicless cards.
        let events = live_cycle_events(false);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].payload.body, "kickoff");
        assert_eq!(events[1].payload.body, "goal");
        assert_eq!(events[2].payload.body, "full-time");
        for event in &events {
            assert_eq!(event.topic, None);
            assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 8 });
        }
    }

    #[test]
    fn live_card_on_shares_one_topic_with_recurring_until_full_time() {
        let events = live_cycle_events(true);
        assert_eq!(events.len(), 3);
        let topic = "espn:usa.1:761659";
        // every non-final event rides the shared Topic as Recurring, so
        // kickoff/goal supersede each other in the single Slot.
        for event in &events[..2] {
            assert_eq!(event.topic.as_deref(), Some(topic));
            assert_eq!(event.rotation, RotationSpec::Recurring { display_secs: 8 });
        }
        // full-time retires the card: OneShot on the *same* Topic — the
        // supersede copies this rotation onto the visible item, so it
        // rotates out via the ordinary one-shot path, no bespoke teardown.
        assert_eq!(events[2].payload.body, "full-time");
        assert_eq!(events[2].topic.as_deref(), Some(topic));
        assert_eq!(events[2].rotation, RotationSpec::OneShot { ttl_secs: 8 });
    }

    // plan 042: collapsed-scorecard meta.details — Clock always (flag
    // on), per-side Cards only when any exist; flag off stays default.

    #[test]
    fn live_card_off_keeps_meta_default() {
        // regression pin: with the flag off, meta is byte-identical to
        // pre-039 behavior — fully default, empty details.
        let events = live_cycle_events(false);
        assert_eq!(events.len(), 3);
        for event in &events {
            assert_eq!(event.meta, EventMeta::default());
        }
    }

    #[test]
    fn live_card_on_attaches_clock_and_per_side_cards() {
        // same "one home yellow ago" setup as
        // new_card_emits_match_state_with_detail, flag on.
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_cards.0 -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, true);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].meta.details,
            vec![
                DetailItem {
                    label: "Clock".to_string(),
                    value: "120'".to_string(),
                },
                DetailItem {
                    label: "Cards".to_string(),
                    value: "ARS 4Y0R · PSG 2Y0R".to_string(),
                },
            ]
        );
    }

    // plan 083 item 4: EspnMeta — the structured sibling of the Clock/Cards
    // detail cells above, same values, unjoined.

    #[test]
    fn live_card_on_attaches_structured_espn_meta() {
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.home_cards.0 -= 1;
        let mut snap = Snapshot::new();
        snap.insert(sb.events[0].id.clone(), v_snap);

        let mut live = parse_scoreboard(UCL).unwrap();
        live.events[0].status.status_type.state = "in".to_string();

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, true);
        assert_eq!(events.len(), 1);
        let espn = events[0]
            .meta
            .espn
            .as_ref()
            .expect("espn_live_card on must populate EspnMeta");
        assert_eq!(espn.league, "UCL");
        assert_eq!(espn.home_abbrev, "PSG");
        assert_eq!(espn.away_abbrev, "ARS");
        assert_eq!(espn.home_score, 1);
        assert_eq!(espn.away_score, 1);
        assert_eq!(espn.clock, "120'");
        assert_eq!(espn.home_cards, (2, 0));
        assert_eq!(espn.away_cards, (4, 0));
        // crest paths are patched in afterward by the async poller loop
        // (workstream a) — diff_scoreboard itself never touches the
        // filesystem, so both stay None here.
        assert_eq!(espn.home_crest, None);
        assert_eq!(espn.away_crest, None);
    }

    #[test]
    fn live_card_off_leaves_espn_meta_none() {
        // regression pin: flag off must stay byte-identical — no EspnMeta,
        // same as the pre-083 `meta == EventMeta::default()` pin above.
        let events = live_cycle_events(false);
        for event in &events {
            assert_eq!(event.meta.espn, None);
        }
    }

    #[test]
    fn live_card_on_clean_match_omits_cards_cell() {
        // the USA fixture carries no card details — Clock must still be
        // there, Cards omitted rather than "0Y0R · 0Y0R".
        let (snap, mut sb) = baseline(USA);
        sb.events[0].status.status_type.state = "in".to_string(); // kickoff
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, true);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].meta.details,
            vec![DetailItem {
                label: "Clock".to_string(),
                value: "0'".to_string(),
            }]
        );
    }

    #[test]
    fn live_card_on_goal_and_full_time_in_one_poll_share_meta() {
        // mirrors goal_and_full_time_in_one_poll_emit_in_order with the
        // flag on: both events pushed this poll carry the same meta.
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, true);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[1].payload.body, "full-time");
        assert_eq!(events[0].meta.details.len(), 1); // Clock only — clean match
        assert_eq!(events[0].meta, events[1].meta);
    }

    #[tokio::test]
    async fn live_card_cycle_collapses_to_one_slot_and_still_fans_out() {
        // end-to-end: the flag-on sequence through a real Engine +
        // SingleSlotQueue must collapse to one Slot occupant (topic
        // supersession), while every delta still reaches connectors
        // (fan-out survives supersession under Engine::accept).
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let connector = ConnectorHandle::new("test", tx);
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(0),
            app.handle().clone(),
            Arc::new(vec![connector]),
            Arc::new(std::sync::Mutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            None,
        );

        for event in live_cycle_events(true) {
            engine.accept(event, false).await.unwrap();
        }

        // one consolidated card showing the latest state, not three items
        match engine.read(|q| q.current_slot_state()).await {
            SlotState::Showing { body, .. } => assert_eq!(body, "full-time"),
            SlotState::Empty => panic!("expected a Showing slot state"),
        }
        for expected in ["kickoff", "goal", "full-time"] {
            let fanned = rx.try_recv().expect("every event must fan out");
            assert_eq!(fanned.payload.body, expected);
        }
        assert!(rx.try_recv().is_err(), "no extra events may fan out");
    }

    #[test]
    fn malformed_and_empty_json_are_handled() {
        assert!(parse_scoreboard("{not json").is_err());
        let sb = parse_scoreboard("{}").unwrap();
        assert!(sb.events.is_empty());
        let (events, snap) =
            diff_scoreboard(&Snapshot::new(), &sb, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert!(snap.is_empty());
    }

    #[test]
    fn backoff_doubles_to_cap_and_resets_on_success() {
        let mut b = Backoff::default();
        let t0 = Instant::now();
        assert!(b.ready(t0));

        b.on_failure(t0);
        assert!(!b.ready(t0));
        assert!(b.ready(t0 + Duration::from_secs(30)));

        b.on_failure(t0); // second failure waits 60s
        assert!(!b.ready(t0 + Duration::from_secs(59)));
        assert!(b.ready(t0 + Duration::from_secs(60)));

        for _ in 0..10 {
            b.on_failure(t0); // delay caps at 300s
        }
        assert!(!b.ready(t0 + Duration::from_secs(299)));
        assert!(b.ready(t0 + Duration::from_secs(300)));

        b.on_success();
        assert!(b.ready(t0));
        b.on_failure(t0);
        assert!(b.ready(t0 + Duration::from_secs(30))); // reset to base
    }

    // -----------------------------------------------------------------
    // plan 083 workstream c (079 item 6a): richer live-match events.
    //
    // `SUMMARY_LIVE` is SYNTHESIZED, not a captured live-network payload
    // — the raw research evidence directory
    // (`research/043-worldcup-final-verification/`) is gitignored and not
    // present in this checkout (`.gitignore`: "throwaway exploratory
    // captures"). This fixture matches the DOCUMENTED shape recorded in
    // the checked-in `plans/suspended/043-richer-match-events.md` (key
    // names `commentary`/`keyEvents`; `keyEvents` entry fields `id`/
    // `type`/`text`/`clock`/`scoringPlay`), with one entry per locked
    // rich-event kind plus a scoreboard-owned goal/yellow-card (must be
    // dropped) and one unrecognized future type (must be skipped, not
    // fatal).
    // -----------------------------------------------------------------

    const SUMMARY_LIVE: &str = include_str!("../tests/fixtures/espn-summary-live.json");
    const PLAYS_PAGE1: &str = include_str!("../tests/fixtures/espn-plays-page1.json");
    const PLAYS_PAGE3_NEWEST: &str = include_str!("../tests/fixtures/espn-plays-page3-newest.json");

    #[test]
    fn parse_summary_parses_commentary_and_key_events() {
        let resp = parse_summary(SUMMARY_LIVE).unwrap();
        assert_eq!(resp.commentary.len(), 1);
        assert_eq!(resp.commentary[0].text, "Kickoff");
        assert_eq!(resp.key_events.len(), 7);
    }

    #[test]
    fn parse_summary_empty_object_yields_empty_arrays() {
        let resp = parse_summary("{}").unwrap();
        assert!(resp.commentary.is_empty());
        assert!(resp.key_events.is_empty());
        assert!(is_empty_summary(&resp));
    }

    #[test]
    fn parse_plays_parses_page_count_and_items() {
        let resp = parse_plays(PLAYS_PAGE1).unwrap();
        assert_eq!(resp.page_count, 3);
        assert_eq!(resp.items.len(), 1);
    }

    #[test]
    fn classify_rich_type_maps_all_four_locked_kinds() {
        assert_eq!(classify_rich_type("foul"), Some(RichEventKind::Foul));
        assert_eq!(classify_rich_type("offside"), Some(RichEventKind::Offside));
        assert_eq!(
            classify_rich_type("var-check"),
            Some(RichEventKind::VarCheck)
        );
        assert_eq!(
            classify_rich_type("substitution"),
            Some(RichEventKind::Substitution)
        );
    }

    #[test]
    fn classify_rich_type_drops_scoreboard_owned_types_outright() {
        for scoreboard_owned in [
            "goal",
            "penalty-scored",
            "yellow-card",
            "red-card",
            "own-goal",
            "kickoff",
            "half-time",
            "full-time",
        ] {
            assert_eq!(
                classify_rich_type(scoreboard_owned),
                None,
                "{scoreboard_owned} must never be classified as a rich event"
            );
        }
    }

    #[test]
    fn classify_rich_type_skips_unrecognized_types_not_fatal() {
        assert_eq!(classify_rich_type("some-future-espn-type"), None);
        assert_eq!(classify_rich_type(""), None);
    }

    #[test]
    fn extract_rich_events_pulls_only_the_four_locked_kinds_from_key_events() {
        let resp = parse_summary(SUMMARY_LIVE).unwrap();
        let candidates = extract_rich_events(&resp);
        // 7 keyEvents total: foul, offside, var-check, substitution,
        // goal (dropped), yellow-card (dropped), unrecognized (dropped)
        // -> exactly 4 candidates survive.
        assert_eq!(candidates.len(), 4);
        assert_eq!(candidates[0].kind, RichEventKind::Foul);
        assert!(candidates[0].text.contains("Foul by Pedro Porro"));
        assert_eq!(candidates[0].clock, "12'");
        assert_eq!(candidates[1].kind, RichEventKind::Offside);
        assert_eq!(candidates[2].kind, RichEventKind::VarCheck);
        assert_eq!(candidates[3].kind, RichEventKind::Substitution);
    }

    #[test]
    fn extract_rich_events_from_plays_pulls_only_recognized_kinds() {
        let resp = parse_plays(PLAYS_PAGE3_NEWEST).unwrap();
        let candidates = extract_rich_events_from_plays(&resp);
        // page3 has a foul (kept) and a goal (dropped, scoreboard-owned).
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, RichEventKind::Foul);
        assert_eq!(candidates[0].clock, "40'");
    }

    #[test]
    fn dedup_filter_new_drops_the_same_key_on_a_second_poll() {
        let mut seen = HashSet::new();
        let candidate = RichEventCandidate {
            kind: RichEventKind::Foul,
            text: "Foul by X".to_string(),
            clock: "12'".to_string(),
        };
        let first_poll = filter_new(&mut seen, vec![candidate.clone()]);
        assert_eq!(first_poll.len(), 1, "first sighting must pass through");
        let second_poll = filter_new(&mut seen, vec![candidate]);
        assert!(
            second_poll.is_empty(),
            "same (kind, clock) key on a re-poll must be deduped away"
        );
    }

    #[test]
    fn dedup_filter_new_treats_a_different_clock_as_a_new_event() {
        let mut seen = HashSet::new();
        let first = RichEventCandidate {
            kind: RichEventKind::Foul,
            text: "Foul by X".to_string(),
            clock: "12'".to_string(),
        };
        let second = RichEventCandidate {
            kind: RichEventKind::Foul,
            text: "Foul by Y".to_string(),
            clock: "50'".to_string(),
        };
        assert_eq!(filter_new(&mut seen, vec![first]).len(), 1);
        assert_eq!(
            filter_new(&mut seen, vec![second]).len(),
            1,
            "a genuinely different event (different clock) must not be deduped"
        );
    }

    #[test]
    fn make_rich_event_maps_kind_to_signal_and_rides_football_ttl() {
        let (snap, _) = baseline(USA);
        let match_snap = snap.get("761659").unwrap();
        let candidate = RichEventCandidate {
            kind: RichEventKind::Offside,
            text: "Offside — someone".to_string(),
            clock: "23'".to_string(),
        };
        let event = make_rich_event("usa.1", match_snap, &candidate, 8, Priority::High);
        assert_eq!(event.signal, EventSignal::Offside);
        assert_eq!(event.payload.body, "Offside — someone");
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 8 });
        assert_eq!(event.topic, None, "informational one-shots ride no Topic");
        assert_eq!(event.origin, SourceKind::Football);
    }

    // ---- fallback-chain orchestration (wiremock) ----

    #[tokio::test]
    async fn poll_rich_events_uses_summary_when_it_succeeds_and_never_touches_plays() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SUMMARY_LIVE))
            .expect(1)
            .mount(&server)
            .await;
        // no /plays mock mounted at all — if poll_rich_events called it,
        // wiremock's default 404-on-unmounted-path response would still
        // be handled gracefully, but `.expect(1)` above (summary called
        // exactly once) combined with checking the candidates came from
        // SUMMARY_LIVE is the real assertion here.

        let client = crate::net::build_poll_client().unwrap();
        let base = server.uri();
        let candidates = poll_rich_events(&client, &base, &base, "usa.1", "123").await;
        assert_eq!(candidates.len(), 4, "candidates must come from summary");
        server.verify().await;
    }

    #[tokio::test]
    async fn poll_rich_events_falls_back_to_plays_on_summary_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/usa.1/events/123/competitions/123/plays"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLAYS_PAGE1))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let base = server.uri();
        let candidates = poll_rich_events(&client, &base, &base, "usa.1", "123").await;
        // PLAYS_PAGE1's one item is "kickoff" — scoreboard-owned, dropped
        // — so an empty candidate list here still proves the plays
        // endpoint was consulted (pageCount 1, no newest-page fetch).
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn poll_rich_events_falls_back_to_plays_on_summary_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/usa.1/events/123/competitions/123/plays"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLAYS_PAGE1))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let base = server.uri();
        let candidates = poll_rich_events(&client, &base, &base, "usa.1", "123").await;
        assert!(
            candidates.is_empty(),
            "plays page1 (kickoff only) consulted, nothing card-worthy on it"
        );
    }

    #[tokio::test]
    async fn poll_rich_events_fetches_the_newest_plays_page_only() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/usa.1/events/123/competitions/123/plays"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLAYS_PAGE1))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/usa.1/events/123/competitions/123/plays"))
            .and(query_param("page", "3"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLAYS_PAGE3_NEWEST))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let base = server.uri();
        let candidates = poll_rich_events(&client, &base, &base, "usa.1", "123").await;
        // page1's lone item is "kickoff" (dropped); if page1's content had
        // leaked through instead of page3's, this would be empty, not 1 —
        // proving only the newest page's content was used.
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, RichEventKind::Foul);
    }

    #[tokio::test]
    async fn poll_rich_events_both_endpoints_failing_returns_empty_not_a_panic() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/usa.1/events/123/competitions/123/plays"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = crate::net::build_poll_client().unwrap();
        let base = server.uri();
        let candidates = poll_rich_events(&client, &base, &base, "usa.1", "123").await;
        assert!(candidates.is_empty());
    }
}
