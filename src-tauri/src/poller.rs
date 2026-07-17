use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::event::{
    emit_slot_state, Event, EventMeta, EventPayload, EventSignal, EventType, Priority,
    RotationSpec, SlotState, SourceKind,
};
use crate::queue::SingleSlotQueue;

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
    last_scoring_play: Option<String>, // "K. Havertz 6'"
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
        .map(detail_line)
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
        let line = detail_line(d);
        if line.is_empty() {
            kind
        } else {
            format!("{kind} — {line}")
        }
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

fn make_event(
    event_type: EventType,
    title: String,
    body: String,
    ttl_secs: u64,
    signal: EventSignal,
    priority: Priority,
) -> Event {
    Event {
        id: Uuid::new_v4(),
        event_type,
        // v6: configurable via `Config.espn_priority` (default `High`,
        // matching the v3.6-spec-§3.4 hardcoded behavior this replaces).
        // `signal` is presentation-only (icon/animation selection) and
        // deliberately doesn't affect this.
        priority,
        rotation: RotationSpec::OneShot { ttl_secs },
        // the poller has its own per-match dedup/eviction via Snapshot /
        // missed_polls already; it doesn't need the queue's topic
        // supersession mechanism in this pass (spec §3.4: no source here
        // constructs Recurring, and topic is a Recurring-adjacent concern).
        topic: None,
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
) -> (Vec<Event>, Snapshot) {
    let mut out = Vec::new();
    let mut next = Snapshot::new();

    for sb_event in &fetched.events {
        let v = view(sb_event);
        let final_now = v.snap.state == "post";

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

// ---------------------------------------------------------------------------
// per-league backoff (v2 spec §3): 30s → 60s → 120s → … cap 300s, reset on
// that league's first success. pure state machine, unit-tested directly.
// ---------------------------------------------------------------------------

const BACKOFF_BASE: Duration = Duration::from_secs(30);
const BACKOFF_CAP: Duration = Duration::from_secs(300);

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
// tray pause gate — pure per-tick decision for the fetch loop. the tray
// flips a shared AtomicBool; the loop asks the gate what that means for
// this tick. resuming re-baselines (snapshots cleared) so everything that
// happened while paused stays silent — same rule as first sighting.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct GateTick {
    pub poll: bool,
    pub rebaseline: bool,
}

#[derive(Debug)]
pub struct PauseGate {
    was_active: bool,
}

impl PauseGate {
    pub fn new() -> Self {
        // polling starts active; the tray flag also starts true
        Self { was_active: true }
    }

    pub fn tick(&mut self, active: bool) -> GateTick {
        let rebaseline = active && !self.was_active;
        self.was_active = active;
        GateTick {
            poll: active,
            rebaseline,
        }
    }
}

// ---------------------------------------------------------------------------
// fetch loop — deliberately thin and untested (v2 spec §3): everything below
// the "here is a response body" line is the tested surface above.
// ---------------------------------------------------------------------------

async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
    let url = format!("https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard");
    let body = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(body)
}

/// Enqueues each event and offers the accepted ones to every connector —
/// the poller-side twin of `http.rs`'s acceptance fan-out (plan §3:
/// "every accepted event goes to every enabled connector, always" — with
/// one recorded exception: rss/news events are overlay-only and never
/// offered, `IMPLEMENTATION_PLAN.md` §4.6).
/// Returns the promotions to emit. An earlier draft fanned out only from
/// the http handler, silently excluding score events from outbound
/// (caught in the 2026-07-16 v3 review).
pub(crate) fn enqueue_and_fan_out(
    queue: &mut SingleSlotQueue,
    connectors: &[crate::notifier::ConnectorHandle],
    events: Vec<Event>,
    context: &str,
) -> Option<SlotState> {
    for event in events {
        let accepted = event.clone();
        match queue.enqueue(event) {
            Ok(()) => {
                for connector in connectors {
                    connector.offer(&accepted);
                }
            }
            Err(e) => tracing::warn!(context, "espn event dropped: {e}"),
        }
    }
    // only one item can ever be visible now, so there's no batch of
    // "promotions" to report — just whatever the final slot state is after
    // this whole poll's events have been enqueued.
    queue.slot_state_if_changed()
}

// 8 args trips clippy 1.97's too_many_arguments; the spawn signature grew
// one handle per shipped phase (connectors v3, pause/active v5) — bundling
// them into a struct is tracked as tech-debt, not worth churning the call
// sites for a lint.
#[allow(clippy::too_many_arguments)]
pub fn spawn_espn_poller(
    app: tauri::AppHandle,
    queue: Arc<Mutex<SingleSlotQueue>>,
    connectors: Arc<Vec<crate::notifier::ConnectorHandle>>,
    leagues: Vec<String>,
    poll_secs: u64,
    ttl_secs: u64,
    priority: Priority,
    active: Arc<AtomicBool>,
) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let mut snapshots: HashMap<String, Snapshot> = HashMap::new();
        let mut backoffs: HashMap<String, Backoff> = HashMap::new();
        let mut gate = PauseGate::new();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs.max(5)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(?leagues, poll_secs, "espn poller started");

        loop {
            interval.tick().await;
            let t = gate.tick(active.load(Ordering::Relaxed));
            if t.rebaseline {
                // scores moved while we weren't looking; next poll is a
                // silent baseline, not a burst of stale notifications
                snapshots.clear();
                tracing::info!("espn polling resumed from tray; re-baselining");
            }
            if !t.poll {
                continue;
            }
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
                let (events, next) = diff_scoreboard(prev, &scoreboard, ttl_secs, league, priority);
                snapshots.insert(league.clone(), next);
                if events.is_empty() {
                    continue;
                }

                let slot_change = {
                    let mut q = queue.lock().await;
                    enqueue_and_fan_out(&mut q, &connectors, events, league)
                };
                if let Some(state) = slot_change {
                    emit_slot_state(&app, state);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SlotState,
    };
    use crate::notifier::ConnectorHandle;

    const USA: &str = include_str!("../tests/fixtures/scoreboard-usa.1.json");
    const UCL: &str = include_str!("../tests/fixtures/scoreboard-uefa.champions.json");

    fn score_event(title: &str) -> Event {
        Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::ScoreUpdate,
            priority: Priority::High,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Goal,
            origin: SourceKind::Football,
        }
    }

    #[test]
    fn poller_accepted_events_fan_out_and_rejected_do_not() {
        // regression for the 2026-07-16 v3 review finding: score events
        // must reach connectors the same as http pushes (plan §3). one
        // visible slot, no waiting room: first accepted, second rejected.
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let connector = ConnectorHandle::new("test", tx);
        let mut q = SingleSlotQueue::new(0);

        let slot_change = enqueue_and_fan_out(
            &mut q,
            &[connector],
            vec![score_event("accepted"), score_event("rejected")],
            "test.league",
        );

        match slot_change.expect("first event should have promoted") {
            SlotState::Showing { title, .. } => assert_eq!(title, "accepted"),
            SlotState::Empty => panic!("expected a Showing slot state"),
        }
        let fanned = rx.try_recv().expect("accepted score event must fan out");
        assert_eq!(fanned.payload.title, "accepted");
        assert!(rx.try_recv().is_err(), "rejected event must not fan out");
    }

    fn baseline(fixture: &str) -> (Snapshot, Scoreboard) {
        let sb = parse_scoreboard(fixture).unwrap();
        let (events, snap) = diff_scoreboard(&Snapshot::new(), &sb, 8, "usa.1", Priority::High);
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
    fn score_delta_emits_one_score_update_and_nothing_for_unchanged() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
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
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
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
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.body, "half-time");
        assert_eq!(events[0].signal, EventSignal::Halftime);
    }

    #[test]
    fn full_time_emits_and_evicts() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
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
        let (events, next) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
        assert!(events.is_empty());
        assert_eq!(next.len(), 4); // still tracked
        assert_eq!(next.get("761659").unwrap().missed_polls, 1);
    }

    #[test]
    fn goal_during_feed_blip_is_caught_on_reappearance() {
        let (snap, sb) = baseline(USA);

        // poll 1: entire feed transiently empty — everything carried
        let empty = parse_scoreboard("{}").unwrap();
        let (events, carried) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High);
        assert!(events.is_empty());
        assert_eq!(carried.len(), 4);

        // poll 2: feed back, one match scored during the blip
        let mut back = sb;
        back.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) = diff_scoreboard(&carried, &back, 8, "usa.1", Priority::High);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(next.get("761659").unwrap().missed_polls, 0); // reset
    }

    #[test]
    fn sustained_absence_evicts_after_threshold() {
        let (mut snap, _) = baseline(USA);
        let empty = parse_scoreboard("{}").unwrap();
        for i in 1..ABSENT_POLLS_BEFORE_EVICTION {
            let (events, next) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High);
            assert!(events.is_empty());
            assert_eq!(next.len(), 4, "still carried at miss {i}");
            snap = next;
        }
        // the miss that reaches the threshold evicts, silently
        let (events, next) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High);
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

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High);
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

        let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert_eq!(events[0].signal, EventSignal::RedCard);
    }

    #[test]
    fn goal_and_full_time_in_one_poll_emit_in_order() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[1].payload.body, "full-time");
    }

    #[test]
    fn malformed_and_empty_json_are_handled() {
        assert!(parse_scoreboard("{not json").is_err());
        let sb = parse_scoreboard("{}").unwrap();
        assert!(sb.events.is_empty());
        let (events, snap) = diff_scoreboard(&Snapshot::new(), &sb, 8, "usa.1", Priority::High);
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

    #[test]
    fn gate_polls_normally_while_active() {
        let mut g = PauseGate::new();
        for _ in 0..3 {
            assert_eq!(
                g.tick(true),
                GateTick {
                    poll: true,
                    rebaseline: false
                }
            );
        }
    }

    #[test]
    fn gate_skips_every_tick_while_paused() {
        let mut g = PauseGate::new();
        for _ in 0..3 {
            assert_eq!(
                g.tick(false),
                GateTick {
                    poll: false,
                    rebaseline: false
                }
            );
        }
    }

    #[test]
    fn gate_rebaselines_exactly_once_on_resume() {
        let mut g = PauseGate::new();
        g.tick(false);
        assert_eq!(
            g.tick(true),
            GateTick {
                poll: true,
                rebaseline: true
            }
        );
        // only the transition tick re-baselines, not every active tick
        assert_eq!(
            g.tick(true),
            GateTick {
                poll: true,
                rebaseline: false
            }
        );
    }
}
