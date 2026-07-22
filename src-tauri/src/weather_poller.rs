//! The weather source (plan 040 Part B): polls Open-Meteo (keyless, no
//! auth — same client posture as the other pollers) and drives two
//! cleanly separated mechanisms, mirroring football's ambient-vs-card
//! split:
//!
//! - **Ambient**: every successful poll updates the idle-rail weather
//!   chip via `engine.update_weather` with an already-display-formatted
//!   [`WeatherSummary`]. Never an `Event`, never the queue.
//! - **Cards**: threshold alerts (rain-incoming + hot/cold temperature)
//!   are ordinary `accept()`-routed `Event`s with
//!   `origin: SourceKind::Weather`, edge-triggered (Design decision 5):
//!   an alert fires once on crossing INTO alert territory, stays silent
//!   while the condition holds, and re-arms only after it clears.
//!
//! No severe-weather category exists — Open-Meteo delivers only numeric
//! data and WMO codes, no warnings feed.
//!
//! The pure heart (`diff_weather`, `condition_word`) is fixture-tested
//! against `tests/fixtures/open-meteo-bangalore.json` with no network;
//! the outer spawn loop follows the same convention as
//! `spawn_espn_poller`/`spawn_rss_poller` (not unit-tested).

use std::time::Duration;

use chrono::{NaiveDateTime, Timelike};
use serde::Deserialize;
use uuid::Uuid;

use crate::engine::Engine;
use crate::event::{
    DetailItem, Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec,
    SourceKind, Units,
};
use crate::poller::Backoff;
use crate::status::WeatherSummary;

/// Open-Meteo forecast responses are tiny (one day of hourly data);
/// 64 KiB is generous headroom over the ~2 KiB real payload.
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Alert cards reuse the app's default rotation window shape — weather
/// has no dedicated ttl config field (plan 040's config surface), so the
/// standard 8s one-shot window applies.
const WEATHER_ALERT_TTL_SECS: u64 = 8;

#[derive(Debug, Clone, Deserialize)]
pub struct OpenMeteoResponse {
    pub current: OpenMeteoCurrent,
    #[serde(default)]
    pub hourly: Option<OpenMeteoHourly>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenMeteoCurrent {
    pub time: String,
    pub temperature_2m: f64,
    pub weather_code: u8,
    /// Day/night, straight from Open-Meteo (plan 082) — the frontend must
    /// never derive this from the user's wall clock + lat/lon, so this is
    /// the one authoritative source. Open-Meteo returns this as an
    /// INTEGER `1`/`0`, not a JSON boolean — `u8` (not `bool`) is
    /// load-bearing: a `bool` field would deserialize a hand-written
    /// fixture (if written as `true`/`false`) but fail on the real
    /// `1`/`0` wire shape, and only real polling would surface that.
    /// `#[serde(default)]` heals a response/fixture that predates this
    /// field to `0` (night) rather than failing to parse.
    #[serde(default)]
    pub is_day: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenMeteoHourly {
    pub time: Vec<String>,
    #[serde(default)]
    pub precipitation_probability: Vec<u8>,
}

/// "Already fired for this occurrence" state per alert type, carried
/// across polls — the same shape `poller.rs`'s `Snapshot` carries
/// forward to avoid re-emitting a kickoff every poll. `true` means the
/// condition held at the last poll (alert already sent); it flips back
/// to `false` only when the condition clears, which is what re-arms the
/// edge trigger.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WeatherAlertState {
    pub rain_fired: bool,
    pub hot_fired: bool,
    pub cold_fired: bool,
}

/// WMO weather-code → condition word (Design decision 3). Presentation-
/// only: this never feeds any alert threshold. Unknown codes fall back
/// to "—" rather than panicking or dropping the chip.
pub fn condition_word(code: u8) -> &'static str {
    match code {
        0 => "Clear",
        1..=3 => "Cloudy",
        45 | 48 => "Fog",
        51..=67 => "Rain",
        71..=77 => "Snow",
        80..=82 => "Showers",
        95..=99 => "Storm",
        _ => "—",
    }
}

/// The rain-lookahead read. Open-Meteo's `precipitation_probability` is
/// hourly-resolution (not sub-hourly), so a "30-minute lookahead" is
/// interpreted as: take the hourly entry whose `time` is closest to
/// (poll time + lookahead), rounding to the nearest hour — the finest
/// resolution the data actually offers, and still meaningfully
/// forward-looking (plan 040, Design decision 8). The poll time comes
/// from the response's own `current.time`, so this stays pure and
/// fixture-testable (no wall clock).
fn lookahead_rain_probability(response: &OpenMeteoResponse, lookahead_mins: u16) -> Option<u8> {
    let hourly = response.hourly.as_ref()?;
    let poll_time = NaiveDateTime::parse_from_str(&response.current.time, "%Y-%m-%dT%H:%M").ok()?;
    let target = poll_time + chrono::Duration::minutes(i64::from(lookahead_mins));
    let hour_start = target.with_minute(0)?.with_second(0)?.with_nanosecond(0)?;
    let rounded = if target.minute() >= 30 {
        hour_start + chrono::Duration::hours(1)
    } else {
        hour_start
    };
    let target_str = rounded.format("%Y-%m-%dT%H:%M").to_string();
    let index = hourly.time.iter().position(|t| *t == target_str)?;
    hourly.precipitation_probability.get(index).copied()
}

/// Pure parse/diff/alert-state heart of the weather poller. The summary
/// always updates (ambient); the returned events are non-empty only when
/// a threshold newly crosses (edge-triggered). Temperature thresholds
/// are always compared in Celsius (Design decision 3): when the operator
/// displays Fahrenheit the response arrives pre-converted by Open-Meteo,
/// so the comparison converts back to Celsius — display units never
/// change alert semantics.
#[allow(clippy::too_many_arguments)]
pub fn diff_weather(
    response: &OpenMeteoResponse,
    prev: WeatherAlertState,
    units: Units,
    rain_threshold_pct: u8,
    rain_lookahead_mins: u16,
    temp_hot_c: f64,
    temp_cold_c: f64,
    ttl_secs: u64,
    priority: Priority,
) -> (WeatherSummary, Vec<Event>, WeatherAlertState) {
    let condition = condition_word(response.current.weather_code);
    let is_day = response.current.is_day;
    let summary = WeatherSummary {
        temp_display: format!("{:.0}°", response.current.temperature_2m),
        condition: condition.to_string(),
        // plan 110 (Step B): `is_day == 1` — production traffic reaches
        // this point only after `fetch_forecast` has already validated
        // `is_day` is 0 or 1 (`validate_is_day` below), so this is a
        // straight boolean read, not a fallback/default. Fixture-driven
        // unit tests below call `diff_weather` directly (bypassing that
        // validation) specifically to pin the 0->false/1->true mapping in
        // isolation.
        is_day: is_day == 1,
    };

    let mut events = Vec::new();
    let mut next = prev;

    // Rain-incoming: a missing/unparseable lookahead read leaves the
    // fired flag untouched — transient bad data must not re-arm an alert
    // that is still legitimately holding.
    if let Some(probability) = lookahead_rain_probability(response, rain_lookahead_mins) {
        let crossed = probability >= rain_threshold_pct;
        if crossed && !prev.rain_fired {
            events.push(alert_event(
                "Rain expected soon".to_string(),
                format!("{probability}% chance of rain within ~{rain_lookahead_mins} min"),
                ttl_secs,
                priority,
                condition,
                is_day,
            ));
        }
        next.rain_fired = crossed;
    }

    let temp_c = match units {
        Units::Celsius => response.current.temperature_2m,
        Units::Fahrenheit => (response.current.temperature_2m - 32.0) * 5.0 / 9.0,
    };

    let hot = temp_c >= temp_hot_c;
    if hot && !prev.hot_fired {
        events.push(alert_event(
            "High temperature".to_string(),
            format!(
                "{:.0}° — above your {:.0}°C hot threshold",
                response.current.temperature_2m, temp_hot_c
            ),
            ttl_secs,
            priority,
            condition,
            is_day,
        ));
    }
    next.hot_fired = hot;

    let cold = temp_c <= temp_cold_c;
    if cold && !prev.cold_fired {
        events.push(alert_event(
            "Low temperature".to_string(),
            format!(
                "{:.0}° — below your {:.0}°C cold threshold",
                response.current.temperature_2m, temp_cold_c
            ),
            ttl_secs,
            priority,
            condition,
            is_day,
        ));
    }
    next.cold_fired = cold;

    (summary, events, next)
}

// plan 082: `condition`/`is_day` ride along as `meta.details` pairs under
// the `wx-` label prefix — plan 035's existing display-only details
// channel, reused rather than adding `origin` to the wire (the slot-state
// wire has no `origin` field and this plan doesn't add one). The
// frontend derives the card's mood/glyph from these two pairs, then
// EXCLUDES every `wx-`-prefixed pair from its own visible rendering (both
// the collapsed detail-cell loop and the expanded Manifest) — see
// StatusRailCard.tsx's marker-leak guard. `is_day` is carried as the
// literal "0"/"1" string Open-Meteo's integer already is, not "true"/
// "false" — the frontend compares against the string "1" directly.
fn alert_event(
    title: String,
    body: String,
    ttl_secs: u64,
    priority: Priority,
    condition: &str,
    is_day: u8,
) -> Event {
    Event {
        id: Uuid::new_v4(),
        event_type: EventType::Generic,
        priority,
        rotation: RotationSpec::OneShot { ttl_secs },
        topic: None,
        payload: EventPayload { title, body },
        meta: EventMeta {
            details: vec![
                DetailItem {
                    label: "wx-condition".to_string(),
                    value: condition.to_string(),
                },
                DetailItem {
                    label: "wx-is-day".to_string(),
                    value: is_day.to_string(),
                },
            ],
            ..EventMeta::default()
        },
        signal: EventSignal::Generic,
        origin: SourceKind::Weather,
    }
}

fn forecast_url(lat: f64, lon: f64, units: Units) -> String {
    let units_param = match units {
        // Celsius is Open-Meteo's default — omit the param entirely.
        Units::Celsius => "",
        Units::Fahrenheit => "&temperature_unit=fahrenheit",
    };
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,is_day&hourly=precipitation_probability&forecast_days=1{units_param}"
    )
}

// plan 037: ingest goes through `Engine::accept`, same as the espn/rss
// pollers. The ambient summary goes through `engine.update_weather` —
// the two mechanisms never conflate "current conditions" with "a card".
#[allow(clippy::too_many_arguments)]
pub fn spawn_weather_poller(
    engine: Engine,
    lat: f64,
    lon: f64,
    units: Units,
    poll_secs: u64,
    rain_threshold_pct: u8,
    rain_lookahead_mins: u16,
    temp_hot_c: f64,
    temp_cold_c: f64,
    priority: Priority,
) {
    tauri::async_runtime::spawn(async move {
        let client = match crate::net::build_poll_client() {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("weather poller could not build http client: {error}");
                return;
            }
        };
        let url = forecast_url(lat, lon, units);
        let mut backoff = Backoff::default();
        let mut alert_state = WeatherAlertState::default();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs.max(15)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(lat, lon, poll_secs, "weather poller started");

        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            if !backoff.ready(now) {
                continue;
            }

            let response = match fetch_forecast(&client, &url).await {
                Ok(response) => {
                    backoff.on_success();
                    response
                }
                Err(error) => {
                    tracing::warn!("weather poll failed: {error}");
                    backoff.on_failure(now);
                    continue;
                }
            };

            let (summary, events, next_state) = diff_weather(
                &response,
                alert_state,
                units,
                rain_threshold_pct,
                rain_lookahead_mins,
                temp_hot_c,
                temp_cold_c,
                WEATHER_ALERT_TTL_SECS,
                priority,
            );
            alert_state = next_state;

            engine.update_weather(Some(summary));
            for event in events {
                if let Err(error) = engine.accept(event, false).await {
                    tracing::warn!("weather alert dropped: {error}");
                }
            }
        }
    });
}

/// plan 110 (Step B): Open-Meteo's `is_day` is documented as strictly 0/1
/// (`OpenMeteoCurrent::is_day`'s own doc comment) — anything else means
/// the upstream contract broke. Pure and unit-testable on its own,
/// deliberately NOT folded into `diff_weather` (that function stays a
/// fixture-driven pure transform with no `Result`, called both by
/// production polling AFTER this gate and directly by unit tests that
/// want to exercise the 0/1 mapping in isolation without also wiring up
/// this validation).
fn validate_is_day(raw: u8) -> anyhow::Result<()> {
    if raw == 0 || raw == 1 {
        Ok(())
    } else {
        anyhow::bail!("unexpected is_day value: {raw} (expected 0 or 1)")
    }
}

async fn fetch_forecast(client: &reqwest::Client, url: &str) -> anyhow::Result<OpenMeteoResponse> {
    let response = client.get(url).send().await?;
    if response.status() != reqwest::StatusCode::OK {
        anyhow::bail!("unexpected http status {}", response.status());
    }
    let body = crate::net::read_body_capped(response, MAX_RESPONSE_BYTES).await?;
    let parsed: OpenMeteoResponse = serde_json::from_slice(&body)?;
    // plan 110 (Step B): an out-of-contract is_day value is treated as a
    // malformed response — the SAME `Err` branch in the outer poll loop
    // (spawn_weather_poller) that already handles an unparseable body or
    // a non-200 status: logged, backed off, and the previous ambient
    // summary is left in place rather than repainting with a coerced or
    // wrong day/night guess.
    validate_is_day(parsed.current.is_day)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BANGALORE: &str = include_str!("../tests/fixtures/open-meteo-bangalore.json");

    fn fixture() -> OpenMeteoResponse {
        serde_json::from_str(BANGALORE).unwrap()
    }

    fn default_args() -> (Units, u8, u16, f64, f64, u64, Priority) {
        (Units::Celsius, 60, 30, 36.0, 14.0, 8, Priority::Medium)
    }

    fn diff(
        response: &OpenMeteoResponse,
        prev: WeatherAlertState,
    ) -> (WeatherSummary, Vec<Event>, WeatherAlertState) {
        let (units, rain_pct, lookahead, hot_c, cold_c, ttl, priority) = default_args();
        diff_weather(
            response, prev, units, rain_pct, lookahead, hot_c, cold_c, ttl, priority,
        )
    }

    // --- WMO-code mapping (pure, no fixture) ---

    #[test]
    fn condition_word_covers_the_locked_ranges() {
        assert_eq!(condition_word(0), "Clear");
        assert_eq!(condition_word(1), "Cloudy");
        assert_eq!(condition_word(2), "Cloudy");
        assert_eq!(condition_word(3), "Cloudy");
        assert_eq!(condition_word(45), "Fog");
        assert_eq!(condition_word(48), "Fog");
        assert_eq!(condition_word(51), "Rain");
        assert_eq!(condition_word(61), "Rain");
        assert_eq!(condition_word(67), "Rain");
        assert_eq!(condition_word(71), "Snow");
        assert_eq!(condition_word(77), "Snow");
        assert_eq!(condition_word(80), "Showers");
        assert_eq!(condition_word(82), "Showers");
        assert_eq!(condition_word(95), "Storm");
        assert_eq!(condition_word(99), "Storm");
    }

    #[test]
    fn condition_word_falls_back_outside_known_ranges() {
        assert_eq!(condition_word(4), "—");
        assert_eq!(condition_word(44), "—");
        assert_eq!(condition_word(50), "—");
        assert_eq!(condition_word(70), "—");
        assert_eq!(condition_word(83), "—");
        assert_eq!(condition_word(100), "—");
    }

    // --- ambient summary (fixture) ---

    #[test]
    fn fixture_parses_into_the_expected_summary() {
        let (summary, events, _) = diff(&fixture(), WeatherAlertState::default());
        // 26.7°C rounds to the nearest integer at format time ("27°"),
        // weather_code 3 maps to "Cloudy", and the fixture's is_day: 1
        // (06:30 local, daytime) carries through as `true`.
        assert_eq!(
            summary,
            WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
            }
        );
        // the real fixture's near-term rain probabilities are all under
        // the 60% threshold and 26.7°C is between 14/36 — no alerts.
        assert!(events.is_empty());
    }

    // --- is_day -> WeatherSummary.is_day (plan 110 Step B) ---

    #[test]
    fn is_day_zero_carries_as_false_on_the_ambient_summary() {
        let mut response = fixture();
        response.current.is_day = 0;
        let (summary, _, _) = diff(&response, WeatherAlertState::default());
        assert!(!summary.is_day);
    }

    #[test]
    fn is_day_one_carries_as_true_on_the_ambient_summary() {
        let mut response = fixture();
        response.current.is_day = 1;
        let (summary, _, _) = diff(&response, WeatherAlertState::default());
        assert!(summary.is_day);
    }

    // --- validate_is_day: the malformed-response gate (plan 110 Step B) ---

    #[test]
    fn validate_is_day_accepts_zero_and_one() {
        assert!(validate_is_day(0).is_ok());
        assert!(validate_is_day(1).is_ok());
    }

    #[test]
    fn validate_is_day_rejects_any_other_numeric_value() {
        assert!(validate_is_day(2).is_err());
        assert!(validate_is_day(255).is_err());
    }

    // --- day/night (plan 082): is_day crosses the wire as Open-Meteo's
    // own integer 1/0, never a JSON boolean. This test proves the real
    // wire shape parses — the fixture is written with the API's actual
    // integer form ("is_day": 1), not a hand-authored `true`. ---

    #[test]
    fn fixture_parses_is_day_as_the_api_real_integer_form() {
        let response = fixture();
        // 06:30 in the fixture's location-local time — daytime.
        assert_eq!(response.current.is_day, 1);
    }

    #[test]
    fn missing_is_day_field_defaults_to_zero_rather_than_failing_to_parse() {
        // A response/fixture predating this field must still parse
        // (`#[serde(default)]`) — healed to 0 (night), not a hard error.
        let json =
            r#"{"current":{"time":"2026-07-19T06:30","temperature_2m":26.7,"weather_code":3}}"#;
        let response: OpenMeteoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.current.is_day, 0);
    }

    // --- wx-* marker pairs (plan 082): the alert's meta.details carries
    // condition + is_day so the frontend can derive mood/glyph art. These
    // are plan 035's existing display-only details channel, reused under
    // the `wx-` label prefix — the frontend is responsible for filtering
    // them out of its own visible rendering (StatusRailCard.tsx). ---

    #[test]
    fn alert_event_carries_wx_condition_and_wx_is_day_detail_pairs() {
        let crossed = fixture_with_rain_probability(75);
        let (_, events, _) = diff(&crossed, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        let details = &events[0].meta.details;
        assert_eq!(details.len(), 2);
        assert_eq!(details[0].label, "wx-condition");
        assert_eq!(details[0].value, "Cloudy");
        assert_eq!(details[1].label, "wx-is-day");
        assert_eq!(details[1].value, "1");
    }

    // --- rain-incoming: the four edge-trigger cases ---

    fn fixture_with_rain_probability(probability: u8) -> OpenMeteoResponse {
        // poll time is 06:30; +30min lookahead rounds to 07:00, which is
        // hourly index 7 (16% in the real fixture). Mutating that one
        // entry is the same technique poller.rs uses for scenarios no
        // real fixture contains.
        let mut response = fixture();
        response.hourly.as_mut().unwrap().precipitation_probability[7] = probability;
        response
    }

    #[test]
    fn rain_below_threshold_fires_nothing() {
        let (_, events, state) = diff(&fixture(), WeatherAlertState::default());
        assert!(events.is_empty());
        assert!(!state.rain_fired);
    }

    #[test]
    fn rain_crossing_threshold_fires_exactly_once() {
        let crossed = fixture_with_rain_probability(75);
        let (_, events, state) = diff(&crossed, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].origin, SourceKind::Weather);
        assert_eq!(events[0].priority, Priority::Medium);
        assert_eq!(events[0].payload.title, "Rain expected soon");
        assert!(state.rain_fired);
    }

    #[test]
    fn rain_still_crossed_on_next_poll_fires_no_second_event() {
        let crossed = fixture_with_rain_probability(75);
        let (_, _, state) = diff(&crossed, WeatherAlertState::default());
        let (_, events, state2) = diff(&crossed, state);
        assert!(events.is_empty());
        assert!(state2.rain_fired);
    }

    #[test]
    fn rain_clearing_rearms_and_recrossing_fires_again() {
        let crossed = fixture_with_rain_probability(75);
        let (_, _, fired) = diff(&crossed, WeatherAlertState::default());
        // clears: real fixture (16% at the lookahead hour) re-arms
        let (_, events, rearmed) = diff(&fixture(), fired);
        assert!(events.is_empty());
        assert!(!rearmed.rain_fired);
        // re-crosses: a second event, proving the alert re-armed
        let (_, events, _) = diff(&crossed, rearmed);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "Rain expected soon");
    }

    // --- temperature thresholds: the same four-case shape, hot & cold ---

    fn fixture_with_temp(temp: f64) -> OpenMeteoResponse {
        let mut response = fixture();
        response.current.temperature_2m = temp;
        response
    }

    #[test]
    fn temp_between_thresholds_fires_nothing() {
        let (_, events, state) = diff(&fixture(), WeatherAlertState::default());
        assert!(events.is_empty());
        assert!(!state.hot_fired);
        assert!(!state.cold_fired);
    }

    #[test]
    fn hot_crossing_fires_once_then_stays_silent_then_rearms() {
        let hot = fixture_with_temp(37.5);
        let (_, events, fired) = diff(&hot, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "High temperature");
        assert!(fired.hot_fired);

        let (_, events, holding) = diff(&hot, fired);
        assert!(events.is_empty());
        assert!(holding.hot_fired);

        let (_, events, rearmed) = diff(&fixture(), holding);
        assert!(events.is_empty());
        assert!(!rearmed.hot_fired);

        let (_, events, _) = diff(&hot, rearmed);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "High temperature");
    }

    #[test]
    fn cold_crossing_fires_once_then_stays_silent_then_rearms() {
        let cold = fixture_with_temp(12.0);
        let (_, events, fired) = diff(&cold, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "Low temperature");
        assert!(fired.cold_fired);

        let (_, events, holding) = diff(&cold, fired);
        assert!(events.is_empty());
        assert!(holding.cold_fired);

        let (_, events, rearmed) = diff(&fixture(), holding);
        assert!(events.is_empty());
        assert!(!rearmed.cold_fired);

        let (_, events, _) = diff(&cold, rearmed);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "Low temperature");
    }

    // --- inclusive-threshold boundaries: the comparisons are `>=`/`<=`,
    // so the exact threshold value itself must fire (plan 047) ---

    #[test]
    fn hot_boundary_at_exactly_threshold_fires() {
        let at_threshold = fixture_with_temp(36.0);
        let (_, events, fired) = diff(&at_threshold, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "High temperature");
        assert!(fired.hot_fired);
    }

    #[test]
    fn cold_boundary_at_exactly_threshold_fires() {
        let at_threshold = fixture_with_temp(14.0);
        let (_, events, fired) = diff(&at_threshold, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "Low temperature");
        assert!(fired.cold_fired);
    }

    #[test]
    fn rain_boundary_at_exactly_threshold_fires() {
        let at_threshold = fixture_with_rain_probability(60);
        let (_, events, state) = diff(&at_threshold, WeatherAlertState::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "Rain expected soon");
        assert!(state.rain_fired);
    }

    #[test]
    fn fahrenheit_display_still_compares_thresholds_in_celsius() {
        // 100°F == 37.8°C — over the 36°C hot threshold even though the
        // display number reads below it.
        let fahrenheit = fixture_with_temp(100.0);
        let (summary, events, _) = diff_weather(
            &fahrenheit,
            WeatherAlertState::default(),
            Units::Fahrenheit,
            60,
            30,
            36.0,
            14.0,
            8,
            Priority::Medium,
        );
        assert_eq!(summary.temp_display, "100°");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload.title, "High temperature");
    }

    #[test]
    fn missing_hourly_data_never_panics_and_preserves_alert_state() {
        let mut response = fixture();
        response.hourly = None;
        let fired = WeatherAlertState {
            rain_fired: true,
            hot_fired: false,
            cold_fired: false,
        };
        let (_, events, state) = diff(&response, fired);
        assert!(events.is_empty());
        // a transient missing read must not re-arm a holding alert
        assert!(state.rain_fired);
    }

    #[test]
    fn lookahead_rounds_to_the_nearest_hour() {
        // 06:30 + 30min = 07:00 exactly → index 7 (16% in the fixture).
        assert_eq!(lookahead_rain_probability(&fixture(), 30), Some(16));
        // 06:30 + 60min = 07:30 → rounds to 08:00 → index 8 (22%).
        assert_eq!(lookahead_rain_probability(&fixture(), 60), Some(22));
        // 06:30 + 120min = 08:30 → minute 30 rounds up to 09:00 →
        // index 9 (33%).
        assert_eq!(lookahead_rain_probability(&fixture(), 120), Some(33));
    }

    #[test]
    fn lookahead_rounds_down_just_under_the_boundary() {
        // 06:30 + 59min = 07:29 — minute 29 is just under the >=30 rounding
        // threshold, so this must round DOWN to 07:00 (index 7, 16%), not
        // up to 08:00 (index 8, 22%). The existing
        // lookahead_rounds_to_the_nearest_hour test only exercises the
        // round-UP side of this boundary (minute == 30) and an exact-hour
        // target (minute == 0) — this is the missing round-DOWN case.
        assert_eq!(lookahead_rain_probability(&fixture(), 59), Some(16));
    }

    fn fixture_with_late_poll_time_and_next_day_hour() -> OpenMeteoResponse {
        // mirrors fixture_with_rain_probability's technique: mutate the
        // already-deserialized struct rather than editing the JSON fixture,
        // since the base fixture has no next-day hourly entry to round into.
        let mut response = fixture();
        response.current.time = "2026-07-19T23:45".to_string();
        let hourly = response.hourly.as_mut().unwrap();
        hourly.time.push("2026-07-20T00:00".to_string());
        hourly.precipitation_probability.push(42);
        response
    }

    #[test]
    fn lookahead_rolls_over_to_next_day_at_midnight() {
        // poll_time 23:45 + 30min lookahead = next-day 00:15, minute 15 < 30,
        // rounds DOWN to next-day 00:00 — only found if the date component
        // actually rolled over. If only the hour wrapped in place (23→00 on
        // the SAME day), the produced string would be "2026-07-19T00:00",
        // which IS in this fixture — it's the hourly index-0 entry, with
        // probability 2% — so a buggy lookup would still succeed, returning
        // Some(2). That's exactly why the assertion below checks the
        // specific pushed value (42), not just `is_some()`: an `is_some()`
        // assertion would pass under precisely that bug.
        let response = fixture_with_late_poll_time_and_next_day_hour();
        assert_eq!(lookahead_rain_probability(&response, 30), Some(42));
    }
}
