use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;
use uuid::Uuid;

use crate::engine::Engine;
use crate::event::{
    Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SourceKind,
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
    #[serde(default)]
    pub abbreviation: String,
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
    pub cards: usize,
    /// consecutive polls this match has been absent from its league's
    /// feed. reset to 0 whenever it appears; evicted at
    /// ABSENT_POLLS_BEFORE_EVICTION (review fix, 2026-07-16: a
    /// transient empty-but-valid espn response must not silently drop
    /// live matches and lose their in-window events).
    pub missed_polls: usize,
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
    let mut home_score = 0u32;
    let mut away_score = 0u32;

    if let Some(comp) = comp {
        for c in &comp.competitors {
            let abbrev = c
                .team
                .as_ref()
                .map(|t| t.abbreviation.clone())
                .unwrap_or_default();
            let score = c
                .score
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            match c.home_away.as_str() {
                "home" => {
                    home_abbrev = abbrev;
                    home_score = score;
                }
                "away" => {
                    away_abbrev = abbrev;
                    away_score = score;
                }
                _ => {}
            }
        }
    }

    let details = comp.map(|c| c.details.as_slice()).unwrap_or(&[]);
    let cards = details
        .iter()
        .filter(|d| {
            d.detail_type
                .as_ref()
                .map(|t| t.text.contains("Card"))
                .unwrap_or(false)
        })
        .count();
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
            cards,
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

                if v.snap.home_score != old.home_score || v.snap.away_score != old.away_score {
                    let body = v
                        .last_scoring_play
                        .clone()
                        .unwrap_or_else(|| "goal".to_string());
                    out.push(make_event(
                        EventType::ScoreUpdate,
                        title.clone(),
                        body,
                        ttl_secs,
                        EventSignal::Goal,
                        priority,
                        card_topic(&topic, false),
                    ));
                }

                if old.state == "pre" && v.snap.state == "in" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "kickoff".to_string(),
                        ttl_secs,
                        EventSignal::Kickoff,
                        priority,
                        card_topic(&topic, false),
                    ));
                }
                if v.snap.status_name == "STATUS_HALFTIME" && old.status_name != "STATUS_HALFTIME" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "half-time".to_string(),
                        ttl_secs,
                        EventSignal::Halftime,
                        priority,
                        card_topic(&topic, false),
                    ));
                }
                if final_now && old.state != "post" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "full-time".to_string(),
                        ttl_secs,
                        EventSignal::Fulltime,
                        priority,
                        card_topic(&topic, true),
                    ));
                }

                if v.snap.cards > old.cards {
                    let body = v.last_card.clone().unwrap_or_else(|| "card".to_string());
                    let signal = if v.last_card_is_red {
                        EventSignal::RedCard
                    } else {
                        EventSignal::YellowCard
                    };
                    out.push(make_event(
                        EventType::MatchState,
                        title,
                        body,
                        ttl_secs,
                        signal,
                        priority,
                        card_topic(&topic, false),
                    ));
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

/// plan 037: ingest goes through `Engine::accept` — the one shared path
/// that enqueues with the mutate→wake→emit protocol and then fans
/// accepted events out to every connector (plan §3: "every accepted
/// event goes to every enabled connector, always" — with one recorded
/// exception: rss/news events are overlay-only and never offered,
/// `IMPLEMENTATION_PLAN.md` §4.6 — a rule `accept` encodes via the
/// origin gate, so no per-caller flag is needed here).
pub fn spawn_espn_poller(
    engine: Engine,
    leagues: Vec<String>,
    poll_secs: u64,
    ttl_secs: u64,
    priority: Priority,
    espn_live_card: bool,
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

                let prev = snapshots.entry(league.clone()).or_default();
                let (events, next) = diff_scoreboard(
                    prev,
                    &scoreboard,
                    ttl_secs,
                    league,
                    priority,
                    espn_live_card,
                );
                snapshots.insert(league.clone(), next);
                if events.is_empty() {
                    continue;
                }

                for event in events {
                    if let Err(e) = engine.accept(event, false).await {
                        tracing::warn!(league, "espn event dropped: {e}");
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
    use crate::notifier::ConnectorHandle;
    use crate::queue::SingleSlotQueue;
    use std::sync::Arc;

    const USA: &str = include_str!("../tests/fixtures/scoreboard-usa.1.json");
    const UCL: &str = include_str!("../tests/fixtures/scoreboard-uefa.champions.json");

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
            true,
            true,
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
        // pretend we saw this match live with one fewer card
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
        v_snap.cards -= 1;
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
        v_snap.cards -= 1;
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
            true,
            true,
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
}
