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

// everything is defaulted so a missing field degrades to "no delta", never a parse error.

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
    #[serde(rename = "displayClock", default)]
    pub display_clock: String,
}

#[derive(Debug, Deserialize)]
pub struct SbStatusType {
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SbCompetition {
    #[serde(default)]
    pub competitors: Vec<SbCompetitor>,
    #[serde(default)]
    pub details: Vec<SbDetail>,
}

#[derive(Debug, Deserialize)]
pub struct SbCompetitor {
    #[serde(rename = "homeAway", default)]
    pub home_away: String,
    #[serde(default)]
    pub score: Option<String>,
    #[serde(default)]
    pub team: Option<SbTeam>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SbTeam {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub abbreviation: String,
    /// Detail-level card `team` objects reuse this struct and never carry `logo` — defaults to
    /// `None` there, harmlessly.
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SbDetail {
    #[serde(rename = "type", default)]
    pub detail_type: Option<SbDetailType>,
    #[serde(rename = "scoringPlay", default)]
    pub scoring_play: bool,
    #[serde(rename = "redCard", default)]
    pub red_card: bool,
    #[serde(rename = "ownGoal", default)]
    pub own_goal: bool,
    #[serde(default)]
    pub clock: Option<SbClock>,
    #[serde(rename = "athletesInvolved", default)]
    pub athletes: Vec<SbAthlete>,
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

pub type Snapshot = HashMap<String, MatchSnapshot>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    pub home_abbrev: String,
    pub away_abbrev: String,
    pub home_score: u32,
    pub away_score: u32,
    pub state: String,
    pub status_name: String,
    /// Never diffed — clock advance alone emits no queue event.
    pub display_clock: String,
    pub home_cards: (u32, u32),
    pub away_cards: (u32, u32),
    /// reset to 0 whenever it appears; evicted at ABSENT_POLLS_BEFORE_EVICTION so a transient
    /// empty-but-valid espn response can't silently drop live matches.
    pub missed_polls: usize,
}

impl MatchSnapshot {
    /// sum of all four per-side counters — the card event-emission gate's comparison value.
    pub fn total_cards(&self) -> u32 {
        self.home_cards.0 + self.home_cards.1 + self.away_cards.0 + self.away_cards.1
    }
}

const ABSENT_POLLS_BEFORE_EVICTION: usize = 10;

struct MatchView<'a> {
    id: &'a str,
    snap: MatchSnapshot,
    last_scoring_play: Option<String>, // "Goal — K. Havertz 6'"
    last_card: Option<String>,         // "Yellow Card — B. Saka 54'"
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
    match league {
        "eng.1" => "EPL",
        "uefa.champions" => "UCL",
        "esp.1" => "La Liga",
        _ => league,
    }
}

fn matchup(league: &str, s: &MatchSnapshot) -> String {
    format!(
        "{}: {} {}–{} {}",
        league_label(league),
        s.away_abbrev,
        s.away_score,
        s.home_score,
        s.home_abbrev
    )
}

/// `team.logo` is untrusted feed input (SSRF hardening): a URL failing `crest_url_allowed` is
/// filtered out here, at the map-building side.
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
                if crate::crests::crest_url_allowed(logo) {
                    out.insert(team.id.clone(), logo.clone());
                } else {
                    let rejected_host = reqwest::Url::parse(logo)
                        .ok()
                        .and_then(|u| u.host_str().map(str::to_string))
                        .unwrap_or_else(|| "<unparseable>".to_string());
                    tracing::debug!(
                        team_id = %team.id,
                        rejected_host,
                        "crest logo url rejected by allowlist — not fetched"
                    );
                }
            }
        }
    }
    out
}

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

enum CardTopic {
    Off,
    Live(String),
    FullTime(String),
}

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
        priority,
        rotation,
        topic,
        payload: EventPayload { title, body },
        meta: EventMeta::default(),
        signal,
        origin: SourceKind::Football,
    }
}

fn diff_match(
    league: &str,
    v: &MatchView,
    old: Option<&MatchSnapshot>,
    ttl_secs: u64,
    priority: Priority,
    espn_live_card: bool,
) -> (Vec<Event>, Option<MatchSnapshot>) {
    let mut out = Vec::new();
    let final_now = v.snap.state == "post";
    let topic = espn_live_card.then(|| format!("espn:{league}:{}", v.id));

    match old {
        None => {
            let entry = (!final_now).then(|| v.snap.clone());
            (out, entry)
        }
        Some(old) => {
            let title = matchup(league, &v.snap);
            let meta = if topic.is_some() {
                let mut details = vec![DetailItem {
                    label: "Clock".to_string(),
                    value: v.snap.display_clock.clone(),
                }];
                let (home_y, home_r) = v.snap.home_cards;
                let (away_y, away_r) = v.snap.away_cards;
                if home_y + home_r + away_y + away_r > 0 {
                    details.push(DetailItem {
                        label: "Cards".to_string(),
                        value: format!(
                            "{} {}Y{}R · {} {}Y{}R",
                            v.snap.away_abbrev, away_y, away_r, v.snap.home_abbrev, home_y, home_r
                        ),
                    });
                }
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

            let entry = (!final_now).then(|| v.snap.clone());
            (out, entry)
        }
    }
}

/// Carries forward any `prev` match absent from this poll's fetched feed: evict only after
/// sustained absence, never on one missing poll — no events are emitted for absent matches.
fn carry_forward_absent(prev: &Snapshot, fetched: &Scoreboard, next: &mut Snapshot, league: &str) {
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
}

#[cfg(test)]
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
        let old = prev.get(v.id);
        let (events, entry) = diff_match(league, &v, old, ttl_secs, priority, espn_live_card);
        out.extend(events);
        if let Some(entry) = entry {
            next.insert(v.id.to_string(), entry);
        }
    }

    carry_forward_absent(prev, fetched, &mut next, league);

    (out, next)
}

/// The idle rail's live-match chip, computed over the poll loop's whole snapshot map (every watched
/// league).
pub fn live_match_summary(snapshots: &HashMap<String, Snapshot>) -> Option<LiveMatchSummary> {
    let mut leagues: Vec<(&String, &Snapshot)> = snapshots.iter().collect();
    leagues.sort_by(|a, b| a.0.cmp(b.0));
    for (_league, snapshot) in leagues {
        let mut matches: Vec<(&String, &MatchSnapshot)> = snapshot.iter().collect();
        matches.sort_by(|a, b| a.0.cmp(b.0));
        if let Some((_id, m)) = matches.into_iter().find(|(_, m)| m.state == "in") {
            return Some(LiveMatchSummary {
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

// Goal/penalty/own-goal/yellow/red already flow from the scoreboard feed above and are NEVER
// re-emitted here (see `classify_rich_type`).
#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    #[serde(default)]
    pub commentary: Vec<CommentaryEntry>,
    #[serde(default, rename = "keyEvents")]
    pub key_events: Vec<KeyEventEntry>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichEventKind {
    Foul,
    Offside,
    VarCheck,
    Substitution,
}

fn classify_rich_type(raw: &str) -> Option<RichEventKind> {
    match raw {
        "foul" => Some(RichEventKind::Foul),
        "offside" => Some(RichEventKind::Offside),
        "var-check" | "var-review" | "video-review" => Some(RichEventKind::VarCheck),
        "substitution" | "sub" => Some(RichEventKind::Substitution),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichEventCandidate {
    pub kind: RichEventKind,
    pub text: String,
    pub clock: String,
}

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

fn is_empty_summary(resp: &SummaryResponse) -> bool {
    resp.commentary.is_empty() && resp.key_events.is_empty()
}

/// Dedup key: (kind, clock) — a re-poll re-fetching the same event must not re-emit it.
fn dedup_key(candidate: &RichEventCandidate) -> String {
    format!("{:?}|{}", candidate.kind, candidate.clock)
}

fn filter_new(
    seen: &HashSet<String>,
    candidates: Vec<RichEventCandidate>,
) -> Vec<RichEventCandidate> {
    candidates
        .into_iter()
        .filter(|c| !seen.contains(&dedup_key(c)))
        .collect()
}

fn evict_rich_seen(
    rich_seen: &mut HashMap<(String, String), HashSet<String>>,
    snapshots: &HashMap<String, Snapshot>,
) {
    rich_seen.retain(|(league, id), _| {
        snapshots
            .get(league)
            .map(|snap| snap.contains_key(id))
            .unwrap_or(false)
    });
}

/// Builds the emitted `Event` for one informational candidate — a one-shot (never a Topic/Recurring
/// card; the sticky live-match card is the existing Topic machinery's job).
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

async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse("https://site.api.espn.com/apis/site/v2/sports/soccer")?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("espn scoreboard base url cannot-be-a-base"))?
        .push(league)
        .push("scoreboard");
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_SCOREBOARD_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

const ESPN_SUMMARY_BASE: &str = "https://site.api.espn.com/apis/site/v2/sports/soccer";
const ESPN_CORE_BASE: &str = "https://sports.core.api.espn.com/v2/sports/soccer/leagues";
const MAX_RICH_EVENT_BYTES: usize = 512 * 1024;

async fn fetch_summary(
    client: &reqwest::Client,
    base: &str,
    league: &str,
    event_id: &str,
) -> anyhow::Result<String> {
    // `event_id` comes off the feed we just fetched (not directly attacker-controlled, but still
    // untrusted), interpolated into a URL.
    let mut url = reqwest::Url::parse(base)?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("espn summary base url cannot-be-a-base"))?
        .push(league)
        .push("summary");
    url.query_pairs_mut().append_pair("event", event_id);
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_RICH_EVENT_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

async fn fetch_plays_page(
    client: &reqwest::Client,
    base: &str,
    league: &str,
    event_id: &str,
    page: u32,
) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(base)?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("espn plays base url cannot-be-a-base"))?
        .push(league)
        .push("events")
        .push(event_id)
        .push("competitions")
        .push(event_id)
        .push("plays");
    url.query_pairs_mut().append_pair("page", &page.to_string());
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = crate::net::read_body_capped(response, MAX_RICH_EVENT_BYTES).await?;
    Ok(String::from_utf8(bytes)?)
}

/// Fetches the NEWEST page only, never backfilling — one request to learn `pageCount`, a second to
/// the last page only when there's more than one.
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
        // per-match dedup key sets for the richer event feed — keyed by (league, match id) so a
        // match id collision across leagues can't happen.
        let mut rich_seen: HashMap<(String, String), HashSet<String>> = HashMap::new();
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
                        tracing::warn!(league, "espn poll failed: {e}");
                        backoff.on_failure(now);
                        continue;
                    }
                };

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

                let prev_snapshot = snapshots.entry(league.clone()).or_default().clone();
                let mut next_snapshot = Snapshot::new();
                for sb_event in &scoreboard.events {
                    let v = view(sb_event);
                    let old = prev_snapshot.get(v.id);
                    let (mut match_events, tentative_entry) =
                        diff_match(league, &v, old, ttl_secs, priority, espn_live_card);
                    patch_crests(&mut match_events, league, &team_ids, &crests);

                    let mut all_accepted = true;
                    for event in match_events {
                        if let Err(e) = engine.accept(event, false).await {
                            tracing::warn!(league, match_id = %v.id, "espn event dropped: {e}");
                            all_accepted = false;
                        }
                    }
                    let committed_entry = if all_accepted {
                        tentative_entry
                    } else {
                        old.cloned()
                    };
                    if let Some(entry) = committed_entry {
                        next_snapshot.insert(v.id.to_string(), entry);
                    }
                }
                carry_forward_absent(&prev_snapshot, &scoreboard, &mut next_snapshot, league);
                snapshots.insert(league.clone(), next_snapshot);

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
                            let seen = rich_seen
                                .entry((league.clone(), match_id.clone()))
                                .or_default();
                            for candidate in filter_new(seen, candidates) {
                                let key = dedup_key(&candidate);
                                let event =
                                    make_rich_event(league, &snap, &candidate, ttl_secs, priority);
                                match engine.accept(event, false).await {
                                    Ok(()) => {
                                        seen.insert(key);
                                    }
                                    Err(e) => {
                                        tracing::warn!(league, match_id, "rich event dropped: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if espn_rich_events {
                evict_rich_seen(&mut rich_seen, &snapshots);
            }

            let summary = live_match_summary(&snapshots);
            engine.update_live_match(summary);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{test_fixtures, EventType, Priority, SlotState};
    use crate::queue::SingleSlotQueue;
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
    async fn poller_accepted_events_enter_the_slot_and_rejected_do_not() {
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(0),
            app.handle().clone(),
            true,
            true,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
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
    fn team_logo_url_parses_from_the_real_fixture() {
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
    fn team_logos_filters_out_non_espncdn_or_non_https_logo_urls() {
        let json = r#"{
            "events": [{
                "id": "1",
                "status": {"type": {"state": "pre"}},
                "competitions": [{
                    "competitors": [
                        {
                            "homeAway": "home",
                            "team": {"id": "1", "logo": "https://a.espncdn.com/x.png"}
                        },
                        {
                            "homeAway": "away",
                            "team": {"id": "2", "logo": "http://evil.com/x.png"}
                        }
                    ]
                }]
            }]
        }"#;
        let sb = parse_scoreboard(json).unwrap();
        let logos = team_logos(&sb);
        assert_eq!(
            logos.get("1").map(String::as_str),
            Some("https://a.espncdn.com/x.png"),
            "allowed espncdn https url must enter the map"
        );
        assert_eq!(
            logos.get("2"),
            None,
            "disallowed http/non-espncdn url must be filtered out"
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

        let empty = parse_scoreboard("{}").unwrap();
        let (events, carried) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert_eq!(carried.len(), 4);

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
        let (events, next) = diff_scoreboard(&snap, &empty, 8, "usa.1", Priority::High, false);
        assert!(events.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn new_card_emits_match_state_with_detail() {
        let sb = parse_scoreboard(UCL).unwrap();
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
        assert_eq!(events[0].signal, EventSignal::YellowCard);
    }

    #[test]
    fn red_card_emits_red_card_signal() {
        let sb = parse_scoreboard(UCL).unwrap();
        let mut v_snap = view(&sb.events[0]).snap;
        v_snap.state = "in".to_string();
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
        assert_eq!(snap.total_cards(), baseline_total - 1);
    }

    #[test]
    fn goal_body_names_the_event() {
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

    fn live_cycle_events(espn_live_card: bool) -> Vec<Event> {
        let (mut snap, mut sb) = baseline(USA);
        let mut out = Vec::new();

        sb.events[0].status.status_type.state = "in".to_string();
        let (events, next) =
            diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);
        snap = next;

        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let (events, next) =
            diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);
        snap = next;

        sb.events[0].status.status_type.state = "post".to_string();
        let (events, _) = diff_scoreboard(&snap, &sb, 8, "usa.1", Priority::High, espn_live_card);
        out.extend(events);

        out
    }

    #[test]
    fn live_card_off_keeps_one_shot_topicless_events() {
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
        for event in &events[..2] {
            assert_eq!(event.topic.as_deref(), Some(topic));
            assert_eq!(event.rotation, RotationSpec::Recurring { display_secs: 8 });
        }
        assert_eq!(events[2].payload.body, "full-time");
        assert_eq!(events[2].topic.as_deref(), Some(topic));
        assert_eq!(events[2].rotation, RotationSpec::OneShot { ttl_secs: 8 });
    }

    #[test]
    fn live_card_off_keeps_meta_default() {
        let events = live_cycle_events(false);
        assert_eq!(events.len(), 3);
        for event in &events {
            assert_eq!(event.meta, EventMeta::default());
        }
    }

    #[test]
    fn live_card_on_attaches_clock_and_per_side_cards() {
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
        assert_eq!(espn.home_crest, None);
        assert_eq!(espn.away_crest, None);
    }

    #[test]
    fn live_card_on_clean_match_omits_cards_cell() {
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
    async fn live_card_cycle_collapses_to_one_slot() {
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(0),
            app.handle().clone(),
            true,
            true,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
        );

        for event in live_cycle_events(true) {
            engine.accept(event, false).await.unwrap();
        }

        match engine.read(|q| q.current_slot_state()).await {
            SlotState::Showing { body, .. } => assert_eq!(body, "full-time"),
            SlotState::Empty => panic!("expected a Showing slot state"),
        }
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
        let first_poll = filter_new(&seen, vec![candidate.clone()]);
        assert_eq!(first_poll.len(), 1, "first sighting must pass through");
        seen.insert(dedup_key(&candidate));
        let second_poll = filter_new(&seen, vec![candidate]);
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
        assert_eq!(filter_new(&seen, vec![first.clone()]).len(), 1);
        seen.insert(dedup_key(&first));
        assert_eq!(
            filter_new(&seen, vec![second]).len(),
            1,
            "a genuinely different event (different clock) must not be deduped"
        );
    }

    #[test]
    fn evict_rich_seen_does_not_wipe_other_leagues() {
        let mut rich_seen: HashMap<(String, String), HashSet<String>> = HashMap::new();
        rich_seen.insert(
            ("usa.1".to_string(), "AAA".to_string()),
            HashSet::from(["Foul|10'".to_string()]),
        );
        rich_seen.insert(
            ("esp.1".to_string(), "BBB".to_string()),
            HashSet::from(["Offside|20'".to_string()]),
        );

        let live_match = |clock: &str| MatchSnapshot {
            home_abbrev: "A".to_string(),
            away_abbrev: "B".to_string(),
            home_score: 0,
            away_score: 0,
            state: "in".to_string(),
            status_name: String::new(),
            display_clock: clock.to_string(),
            home_cards: (0, 0),
            away_cards: (0, 0),
            missed_polls: 0,
        };

        let mut snapshots: HashMap<String, Snapshot> = HashMap::new();
        let mut usa_snap = Snapshot::new();
        usa_snap.insert("AAA".to_string(), live_match("10'"));
        snapshots.insert("usa.1".to_string(), usa_snap);
        let mut esp_snap = Snapshot::new();
        esp_snap.insert("BBB".to_string(), live_match("20'"));
        snapshots.insert("esp.1".to_string(), esp_snap);

        evict_rich_seen(&mut rich_seen, &snapshots);

        assert!(
            rich_seen.contains_key(&("usa.1".to_string(), "AAA".to_string())),
            "usa.1's dedup state must survive a tick where esp.1 is also live"
        );
        assert!(
            rich_seen.contains_key(&("esp.1".to_string(), "BBB".to_string())),
            "esp.1's dedup state must survive a tick where usa.1 is also live"
        );
    }

    #[test]
    fn evict_rich_seen_evicts_matches_absent_from_every_snapshot() {
        let mut rich_seen: HashMap<(String, String), HashSet<String>> = HashMap::new();
        rich_seen.insert(
            ("usa.1".to_string(), "GONE".to_string()),
            HashSet::from(["Foul|10'".to_string()]),
        );
        let snapshots: HashMap<String, Snapshot> = HashMap::new();

        evict_rich_seen(&mut rich_seen, &snapshots);

        assert!(
            rich_seen.is_empty(),
            "a match absent everywhere must be evicted"
        );
    }

    #[test]
    fn diff_match_delta_is_reproducible_when_not_committed() {
        let (snap, mut sb) = baseline(USA);
        sb.events[0].competitions[0].competitors[0].score = Some("1".to_string());
        let old = snap.get("761659").unwrap();
        let v = view(&sb.events[0]);

        let (events1, entry1) = diff_match("usa.1", &v, Some(old), 8, Priority::High, false);
        assert_eq!(events1.len(), 1);
        assert_eq!(events1[0].payload.body, "goal");
        assert!(entry1.is_some());

        let (events2, entry2) = diff_match("usa.1", &v, Some(old), 8, Priority::High, false);
        assert_eq!(
            events2.len(),
            1,
            "a delta dropped by QueueFull must re-emit next tick, not vanish"
        );
        assert_eq!(events2[0].payload.body, events1[0].payload.body);
        assert_eq!(
            entry2, entry1,
            "the re-diff must land on the same snapshot entry"
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

    #[tokio::test]
    async fn poll_rich_events_uses_summary_when_it_succeeds_and_never_touches_plays() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/usa.1/summary"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SUMMARY_LIVE))
            .expect(1)
            .mount(&server)
            .await;

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
