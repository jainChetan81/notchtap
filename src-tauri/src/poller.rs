use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::event::{emit_promoted, Event, EventPayload, EventType, Priority};
use crate::queue::NotificationQueue;

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
}

struct MatchView<'a> {
    id: &'a str,
    snap: MatchSnapshot,
    last_scoring_play: Option<String>, // "K. Havertz 6'"
    last_card: Option<String>,         // "Yellow Card — B. Saka 54'"
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
            let abbrev = c.team.as_ref().map(|t| t.abbreviation.clone()).unwrap_or_default();
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
        .map(|d| detail_line(d))
        .filter(|s| !s.is_empty());
    let last_card = details
        .iter()
        .rev()
        .find(|d| {
            d.detail_type
                .as_ref()
                .map(|t| t.text.contains("Card"))
                .unwrap_or(false)
        })
        .map(|d| {
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
        },
        last_scoring_play,
        last_card,
    }
}

fn matchup(s: &MatchSnapshot) -> String {
    // away-first, matching espn's "ARS @ PSG" orientation:
    // "ARS 1–1 PSG" (spec §3's title shape)
    format!(
        "{} {}–{} {}",
        s.away_abbrev, s.away_score, s.home_score, s.home_abbrev
    )
}

fn make_event(event_type: EventType, title: String, body: String, ttl_secs: u64) -> Event {
    Event {
        id: Uuid::new_v4(),
        event_type,
        priority: Priority::Normal,
        ttl_secs,
        payload: EventPayload { title, body },
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
pub fn diff_scoreboard(prev: &Snapshot, fetched: &Scoreboard, ttl_secs: u64) -> (Vec<Event>, Snapshot) {
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
                let title = matchup(&v.snap);

                if v.snap.home_score != old.home_score || v.snap.away_score != old.away_score {
                    let body = v.last_scoring_play.clone().unwrap_or_else(|| "goal".to_string());
                    out.push(make_event(EventType::ScoreUpdate, title.clone(), body, ttl_secs));
                }

                if old.state == "pre" && v.snap.state == "in" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "kickoff".to_string(),
                        ttl_secs,
                    ));
                }
                if v.snap.status_name == "STATUS_HALFTIME" && old.status_name != "STATUS_HALFTIME" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "half-time".to_string(),
                        ttl_secs,
                    ));
                }
                if final_now && old.state != "post" {
                    out.push(make_event(
                        EventType::MatchState,
                        title.clone(),
                        "full-time".to_string(),
                        ttl_secs,
                    ));
                }

                if v.snap.cards > old.cards {
                    let body = v.last_card.clone().unwrap_or_else(|| "card".to_string());
                    out.push(make_event(EventType::MatchState, title, body, ttl_secs));
                }

                if !final_now {
                    next.insert(v.id.to_string(), v.snap);
                }
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
// fetch loop — deliberately thin and untested (v2 spec §3): everything below
// the "here is a response body" line is the tested surface above.
// ---------------------------------------------------------------------------

async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
    let url =
        format!("https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard");
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

pub fn spawn_espn_poller(
    app: tauri::AppHandle,
    queue: Arc<Mutex<NotificationQueue>>,
    leagues: Vec<String>,
    poll_secs: u64,
    ttl_secs: u64,
) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
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
                let (events, next) = diff_scoreboard(prev, &scoreboard, ttl_secs);
                snapshots.insert(league.clone(), next);
                if events.is_empty() {
                    continue;
                }

                let promoted = {
                    let mut q = queue.lock().await;
                    for event in events {
                        if let Err(e) = q.enqueue(event) {
                            tracing::warn!(league, "espn event dropped: {e}");
                        }
                    }
                    q.take_promoted()
                };
                emit_promoted(&app, promoted);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const USA: &str = include_str!("../tests/fixtures/scoreboard-usa.1.json");
    const UCL: &str = include_str!("../tests/fixtures/scoreboard-uefa.champions.json");

    fn baseline(fixture: &str) -> (Snapshot, Scoreboard) {
        let sb = parse_scoreboard(fixture).unwrap();
        let (events, snap) = diff_scoreboard(&Snapshot::new(), &sb, 8);
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
        let (events, next) = diff_scoreboard(&snap, &sb, 8);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[0].payload.title, "TOR 0–1 MTL");
        assert_eq!(events[0].payload.body, "goal"); // no scoring play in feed
        assert_eq!(events[0].ttl_secs, 8);
        assert_eq!(next.len(), 4);
    }

    #[test]
    fn state_transitions_emit_match_state() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].status.status_type.state = "in".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert_eq!(events[0].payload.body, "kickoff");
    }

    #[test]
    fn halftime_is_detected_via_status_name() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "in".to_string();
        sb.events[0].status.status_type.name = "STATUS_HALFTIME".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.body, "half-time");
    }

    #[test]
    fn full_time_emits_and_evicts() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, next) = diff_scoreboard(&snap, &sb, 8);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.body, "full-time");
        assert!(!next.contains_key("761659")); // evicted
        assert_eq!(next.len(), 3);
    }

    #[test]
    fn absent_match_is_evicted_silently() {
        let (snap, mut sb) = baseline(USA);
        sb.events.remove(0);
        let (events, next) = diff_scoreboard(&snap, &sb, 8);
        assert!(events.is_empty());
        assert_eq!(next.len(), 3);
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

        let (events, _) = diff_scoreboard(&snap, &live, 8);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::MatchState));
        assert!(events[0].payload.body.contains("Card"));
    }

    #[test]
    fn goal_and_full_time_in_one_poll_emit_in_order() {
        let (mut snap, mut sb) = baseline(USA);
        snap.get_mut("761659").unwrap().state = "in".to_string();
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
        assert_eq!(events[1].payload.body, "full-time");
    }

    #[test]
    fn malformed_and_empty_json_are_handled() {
        assert!(parse_scoreboard("{not json").is_err());
        let sb = parse_scoreboard("{}").unwrap();
        assert!(sb.events.is_empty());
        let (events, snap) = diff_scoreboard(&Snapshot::new(), &sb, 8);
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
