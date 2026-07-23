//! Idle source-status rail (plan 034): one combined `status-state` event
//! answering the idle card's "what's happening / what's next" — the boot
//! source gates, the queue depth behind the empty slot, and the one live
//! watched football match. Delivery duplicates the slot-state pattern
//! exactly: the rust core emits on change and plants
//! `window.__NOTCHTAP_STATUS_STATE__` on page load (lib.rs). The overlay
//! stays receive-only — this is a listen-only channel, no invoke.

use serde::Serialize;

use crate::queue::SingleSlotQueue;

/// The status channel into the overlay — the frontend listens for exactly
/// this string (`src/useStatusState.ts`). Change both together.
pub const STATUS_STATE_EVENT: &str = "status-state";

/// camelCase on the wire so the TS `StatusState` type mirrors this shape
/// exactly (same convention as `SlotState`, event.rs).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusState {
    pub paused: bool,
    pub waiting: usize,
    pub football: FootballStatus,
    pub news: NewsStatus,
    pub weather: WeatherStatus,
    pub media: MediaStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootballStatus {
    pub enabled: bool,
    /// `None` when no watched match is in-play (serializes as `null`).
    pub live: Option<LiveMatchSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMatchSummary {
    /// "Home X–Y Away" (poller.rs builds it from the tracked snapshot).
    pub label: String,
    /// espn's own clock text ("45'"), carried verbatim.
    pub minute: String,
}

/// plan 040 Part B: the ambient weather chip's data, carried already
/// display-formatted by the poller — `temp_display` "27°" (units applied
/// server-side by Open-Meteo via its `temperature_unit` query param) and
/// `condition` the WMO-code word ("Cloudy"). The frontend concatenates
/// them (`{tempDisplay} {condition}`), same shape as football's
/// `{live.label} · {live.minute}`.
///
/// plan 110 (Step B): `is_day` rides along too — Open-Meteo's own day/
/// night flag (already parsed by `weather_poller.rs` for the alert card's
/// `wx-is-day` marker, plan 082) now also reaches the ambient channel, so
/// the idle hover-peek's mood art no longer has to guess from the wall
/// clock. Plain `bool` here (unlike `OpenMeteoCurrent.is_day: u8`,
/// documented there): this struct is the wire's OWN presentation shape,
/// not a raw API passthrough, and every other field on it is already
/// display-formatted the same way. This field is NOT continuously
/// varying (flips at most twice a day, CLAUDE.md's `SlotState::dedup_eq`
/// rule doesn't apply) — it belongs in ordinary derived `PartialEq`, and
/// a flip is a genuine content change the change-guard below must repaint
/// on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherSummary {
    pub temp_display: String,
    pub condition: String,
    pub is_day: bool,
    /// plan 122: the rain-chance display for the idle hover-peek, already
    /// floor-filtered against `weather_rain_threshold_pct` by
    /// `weather_poller.rs::diff_weather` — `None` covers both "no
    /// lookahead read" and "below the operator's floor" in one value, so
    /// the frontend's job is reduced to a presence check
    /// (`IdleHoverPeek.tsx`). Like `is_day` above, this is NOT
    /// `SlotState::dedup_eq` territory: `WeatherSummary` never rides a
    /// `SlotState`, its own change-guard is `status_state_if_changed`
    /// below (ordinary derived `PartialEq`), and this field changes at
    /// most once per poll — never per tick — so a value change is a
    /// genuine content change the derived `PartialEq` must catch.
    pub rain_pct: Option<u8>,
    /// plan 131: today's high/low, same `"{:.0}°"` format as
    /// `temp_display` — `weather_poller.rs::diff_weather` reads these off
    /// Open-Meteo's `daily` block (`forecast_days=1`, so index 0 is
    /// always "today"). `None` when the `daily` block is missing or
    /// malformed — never fails the whole summary for a missing forecast.
    /// Same non-`dedup_eq` reasoning as `rain_pct`/`is_day` above: this
    /// changes at most once per poll, so it belongs in the ordinary
    /// derived `PartialEq` this struct already uses.
    pub today_high_display: Option<String>,
    pub today_low_display: Option<String>,
    /// plan 131: the minimal forecast strip — exactly 3 points at +2h/
    /// +4h/+6h from the poll's current hour (nearest hourly slots;
    /// `weather_poller.rs::build_outlook` skips a point rather than
    /// fabricating one when its target hour falls outside the fetched
    /// window). Empty when hourly data is missing/malformed, same
    /// "degrade to nothing, never fail the summary" discipline as
    /// `rain_pct`. Same non-`dedup_eq` reasoning as the other fields on
    /// this struct: changes at most once per poll.
    pub outlook: Vec<OutlookPoint>,
}

/// plan 131: one point on the minimal forecast strip. `hour_label` is a
/// local "HH:MM" string (`weather_poller.rs::build_outlook`, local
/// because the request now carries `timezone=auto`) — the frontend
/// renders it verbatim, never reformats a `Date`. `condition` is the
/// same `condition_word()` mapping `WeatherSummary.condition` already
/// uses, so `IdleHoverPeek.tsx` can feed it straight into the existing
/// `weatherArtFor(condition, isDay)` lookup without a second table.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlookPoint {
    pub hour_label: String,
    pub temp_display: String,
    pub condition: String,
    pub is_day: bool,
}

/// "News paused" in the idle rail means `enabled == false`: the polling
/// gates are boot-config since v6, so there is no runtime poll pause to
/// report beyond the gate itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsStatus {
    pub enabled: bool,
}

/// plan 040 Part B: mirrors `FootballStatus` exactly — `enabled` is the
/// boot-config gate (`Config.weather_enabled`), `current` is `None`
/// until the first successful poll (serializes as `null`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherStatus {
    pub enabled: bool,
    pub current: Option<WeatherSummary>,
}

/// plan 104: the ambient now-playing snapshot. Unlike `WeatherSummary`
/// (already display-formatted, never re-derived client-side), this DOES
/// carry a raw snapshot (`elapsed_ms`/`duration_ms`/`captured_at_ms`) —
/// the plan-081 emission-discipline lesson (CLAUDE.md's own
/// `SlotState::dedup_eq` rule): playback position must never drive a
/// per-second wire emission, so the frontend derives LIVE progress
/// locally from this snapshot (`TtlBar.tsx`'s own pattern), and this
/// struct only changes on a genuine adapter diff event, never a tick.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingSummary {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub playing: bool,
    pub elapsed_ms: u64,
    pub duration_ms: Option<u64>,
    /// Wall-clock epoch millis at the moment `now_playing.rs` received
    /// this snapshot — the anchor the frontend re-derives elapsed-since
    /// from, the same role `SlotState`'s emission timing plays for
    /// `TtlBar`.
    pub captured_at_ms: i64,
    /// `parentApplicationBundleIdentifier` when present, else
    /// `bundleIdentifier` (`now_playing.rs`'s `apply_event` — 103 §5c: the
    /// raw `bundleIdentifier` can be a process-internal helper, e.g.
    /// Safari's own `<audio>` sessions report `com.apple.WebKit.GPU`).
    /// Used only to key the peek row's app glyph (Step 7) — never shown
    /// verbatim.
    pub app_bundle_id: Option<String>,
}

/// plan 104: mirrors `WeatherStatus` exactly — `enabled` is the boot-config
/// gate (`Config.now_playing_enabled`), `current` is `None` until the
/// adapter child reports a session (or the feature/kill-switch gate is
/// off at all, or the child was never spawned/installed).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    pub enabled: bool,
    pub current: Option<NowPlayingSummary>,
}

/// Named-field inputs for [`StatusState::snapshot`] — replaces five
/// positional bool/Option arguments (three same-typed `bool`s, two
/// same-shaped `Option`s) that a future call-site edit could transpose
/// without a compile error. Construct with field names, not
/// positionally, at every call site.
pub struct StatusInputs {
    pub live: Option<LiveMatchSummary>,
    pub espn_enabled: bool,
    pub rss_enabled: bool,
    pub weather: Option<WeatherSummary>,
    pub weather_enabled: bool,
    /// plan 104: named `media` (not `now_playing`) to match the
    /// `StatusState.media`/`MediaStatus` field it feeds — the ambient
    /// snapshot `now_playing.rs`'s supervised child pushes.
    pub media: Option<NowPlayingSummary>,
    pub now_playing_enabled: bool,
}

impl StatusState {
    /// Recomputed from the live handles on every heartbeat pass; cheap
    /// (two queue reads + a clone) so the change-guard below is what keeps
    /// the channel silent at steady state, not any caching here.
    pub fn snapshot(queue: &SingleSlotQueue, inputs: StatusInputs) -> Self {
        Self {
            paused: queue.is_paused(),
            waiting: queue.total_waiting(),
            football: FootballStatus {
                enabled: inputs.espn_enabled,
                live: inputs.live,
            },
            news: NewsStatus {
                enabled: inputs.rss_enabled,
            },
            weather: WeatherStatus {
                enabled: inputs.weather_enabled,
                current: inputs.weather,
            },
            // plan 104: explicitly gated (unlike weather's assembly,
            // which relies structurally on the poller never being spawned
            // while disabled) — belt-and-suspenders against a stale
            // `AmbientSlot` value ever reaching the wire while the user's
            // current config says the feature is off.
            media: MediaStatus {
                enabled: inputs.now_playing_enabled,
                current: if inputs.now_playing_enabled {
                    inputs.media
                } else {
                    None
                },
            },
        }
    }
}

/// The change-guard. Unlike `slot_state_if_changed` (queue-owned), the
/// previous state is a `last_status` local in the heartbeat task — the
/// heartbeat is the sole emitter, so there is exactly one guard and no
/// second writer can desync it (plan 034 step 3).
pub fn status_state_if_changed(
    last: &mut Option<StatusState>,
    next: StatusState,
) -> Option<StatusState> {
    if last.as_ref() == Some(&next) {
        None
    } else {
        *last = Some(next.clone());
        Some(next)
    }
}

/// The single emit path, mirroring `emit_slot_state`: emit failure is
/// logged, never propagated — by this point the state has already changed,
/// so failing the caller would misreport the underlying mutation.
pub fn emit_status_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: StatusState) {
    use tauri::Emitter;
    if let Err(e) = app.emit(STATUS_STATE_EVENT, &state) {
        tracing::error!("failed to emit status-state: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{test_fixtures, Event};

    fn live_summary() -> LiveMatchSummary {
        LiveMatchSummary {
            label: "Arsenal 2–0 Chelsea".to_string(),
            minute: "45'".to_string(),
        }
    }

    fn status(live: Option<LiveMatchSummary>) -> StatusState {
        StatusState {
            paused: false,
            waiting: 3,
            football: FootballStatus {
                enabled: true,
                live,
            },
            news: NewsStatus { enabled: true },
            weather: WeatherStatus {
                enabled: false,
                current: None,
            },
            media: MediaStatus {
                enabled: false,
                current: None,
            },
        }
    }

    fn weather_summary() -> WeatherSummary {
        WeatherSummary {
            temp_display: "27°".to_string(),
            condition: "Cloudy".to_string(),
            is_day: true,
            rain_pct: Some(40),
            today_high_display: Some("30°".to_string()),
            today_low_display: Some("22°".to_string()),
            outlook: vec![outlook_point()],
        }
    }

    fn outlook_point() -> OutlookPoint {
        OutlookPoint {
            hour_label: "15:00".to_string(),
            temp_display: "29°".to_string(),
            condition: "Rain".to_string(),
            is_day: true,
        }
    }

    fn now_playing_summary() -> NowPlayingSummary {
        NowPlayingSummary {
            title: "Midnight City".to_string(),
            artist: Some("M83".to_string()),
            album: Some("Hurry Up, We're Dreaming".to_string()),
            playing: true,
            elapsed_ms: 1500,
            duration_ms: Some(243_000),
            captured_at_ms: 1_753_000_000_000,
            app_bundle_id: Some("app.zen-browser.zen".to_string()),
        }
    }

    #[test]
    fn status_state_event_name_is_pinned() {
        // The frontend listens for exactly this literal
        // (src/useStatusState.ts). A rename on either side compiles clean
        // and passes every other test, shipping a rail that never updates
        // — same reasoning as SLOT_STATE_EVENT's pin in event.rs.
        assert_eq!(STATUS_STATE_EVENT, "status-state");
    }

    #[test]
    fn serializes_camel_case_with_live_match() {
        let json = serde_json::to_value(status(Some(live_summary()))).unwrap();
        assert_eq!(json["paused"], false);
        assert_eq!(json["waiting"], 3);
        assert_eq!(json["football"]["enabled"], true);
        assert_eq!(json["football"]["live"]["label"], "Arsenal 2–0 Chelsea");
        assert_eq!(json["football"]["live"]["minute"], "45'");
        assert_eq!(json["news"]["enabled"], true);
    }

    #[test]
    fn serializes_live_as_null_when_nothing_in_play() {
        let json = serde_json::to_value(status(None)).unwrap();
        assert!(json["football"]["live"].is_null());
        assert!(json["weather"]["current"].is_null());
    }

    #[test]
    fn serializes_weather_summary_camel_case() {
        let mut s = status(None);
        s.weather = WeatherStatus {
            enabled: true,
            current: Some(weather_summary()),
        };
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(json["weather"]["enabled"], true);
        assert_eq!(json["weather"]["current"]["tempDisplay"], "27°");
        assert_eq!(json["weather"]["current"]["condition"], "Cloudy");
        // plan 110 (Step B): the wire carries `isDay` (camelCase), never
        // the rust-side `is_day` spelling — a serialize-shape regression
        // here would silently break the frontend's runtime guard, which
        // checks for `isDay` specifically (useStatusState.ts).
        assert_eq!(json["weather"]["current"]["isDay"], true);
        assert!(json["weather"]["current"].get("is_day").is_none());
        // plan 122: same camelCase discipline for the new field.
        assert_eq!(json["weather"]["current"]["rainPct"], 40);
        assert!(json["weather"]["current"].get("rain_pct").is_none());
        // plan 131: same camelCase discipline for the forecast-strip fields.
        assert_eq!(json["weather"]["current"]["todayHighDisplay"], "30°");
        assert_eq!(json["weather"]["current"]["todayLowDisplay"], "22°");
        assert!(json["weather"]["current"]
            .get("today_high_display")
            .is_none());
        assert!(json["weather"]["current"]
            .get("today_low_display")
            .is_none());
        let outlook = json["weather"]["current"]["outlook"].as_array().unwrap();
        assert_eq!(outlook.len(), 1);
        assert_eq!(outlook[0]["hourLabel"], "15:00");
        assert_eq!(outlook[0]["tempDisplay"], "29°");
        assert_eq!(outlook[0]["condition"], "Rain");
        assert_eq!(outlook[0]["isDay"], true);
        assert!(outlook[0].get("hour_label").is_none());
    }

    // plan 131: pins `today_high_display`/`today_low_display: None`'s wire
    // SHAPE, not just its value — same precedent as `rain_pct`'s own null
    // pin above (plan 124 R2). A future `skip_serializing_if` on either
    // field would make it vanish entirely rather than serialize as an
    // explicit `null`, and the frontend's validator rejects a MISSING key
    // differently than an explicit `null` one.
    #[test]
    fn today_high_low_none_serializes_as_explicit_null_keys_not_missing_ones() {
        let mut s = status(None);
        s.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        let json = serde_json::to_value(s).unwrap();
        assert!(json["weather"]["current"].get("todayHighDisplay").is_some());
        assert!(json["weather"]["current"]["todayHighDisplay"].is_null());
        assert!(json["weather"]["current"].get("todayLowDisplay").is_some());
        assert!(json["weather"]["current"]["todayLowDisplay"].is_null());
        // an empty outlook still serializes as an array, never a missing key.
        assert_eq!(json["weather"]["current"]["outlook"], serde_json::json!([]));
    }

    // plan 124 R2: pins `rain_pct: None`'s wire SHAPE, not just its value —
    // same precedent as `serializes_live_as_null_when_nothing_in_play`
    // (plan 104). The key assertion is `.get("rainPct").is_some()`: a
    // future `#[serde(skip_serializing_if = "Option::is_none")]` added to
    // `rain_pct` would make this field vanish from the JSON entirely on
    // `None` rather than serialize as an explicit `null`, and the
    // frontend's runtime validator (`useStatusState.ts`) rejects a
    // MISSING key differently than an explicit `null` one — this guards
    // against that regression landing silently.
    #[test]
    fn rain_pct_none_serializes_as_an_explicit_null_key_not_a_missing_one() {
        let mut s = status(None);
        s.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        let json = serde_json::to_value(s).unwrap();
        assert!(
            json["weather"]["current"].get("rainPct").is_some(),
            "rainPct key must be PRESENT (as an explicit null) even when rain_pct is None"
        );
        assert!(json["weather"]["current"]["rainPct"].is_null());
    }

    #[test]
    fn change_guard_emits_once_then_stays_silent_until_a_real_change() {
        let mut last = None;
        // first sighting always emits (the page-load seed's dual-path
        // shield relies on the heartbeat's first pass emitting too)
        assert_eq!(
            status_state_if_changed(&mut last, status(None)),
            Some(status(None))
        );
        // identical recompute: silent
        assert_eq!(status_state_if_changed(&mut last, status(None)), None);
        // any field change (here: a match goes live) emits again
        assert_eq!(
            status_state_if_changed(&mut last, status(Some(live_summary()))),
            Some(status(Some(live_summary())))
        );
        assert_eq!(
            status_state_if_changed(&mut last, status(Some(live_summary()))),
            None
        );
    }

    // plan 110 (Step B): `is_day` is not continuously-varying — this pins
    // that a lone day/night flip is treated as an ordinary content change
    // by the SAME derived-`PartialEq` guard above (no `dedup_eq`-style
    // special case needed, no tick-storm from re-polls that don't change
    // it either).
    #[test]
    fn is_day_flip_emits_once_then_stays_silent() {
        let mut last = None;
        let mut day = status(None);
        day.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        // first sighting emits
        assert_eq!(
            status_state_if_changed(&mut last, day.clone()),
            Some(day.clone())
        );
        // identical weather (including is_day): silent
        assert_eq!(status_state_if_changed(&mut last, day.clone()), None);

        let mut night = day.clone();
        night.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: false,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        // is_day alone flipping emits once
        assert_eq!(
            status_state_if_changed(&mut last, night.clone()),
            Some(night.clone())
        );
        // repeating the flipped value: silent again
        assert_eq!(status_state_if_changed(&mut last, night), None);
    }

    // plan 122: `rain_pct` is the same shape of field as `is_day` above —
    // changes at most once per poll, never per tick — so it belongs in
    // the SAME ordinary derived-`PartialEq` guard, no `dedup_eq`-style
    // special case. This test pins that a lone `rain_pct` change (with
    // every other field held constant) is itself a real content change.
    #[test]
    fn rain_pct_change_emits_once_then_stays_silent() {
        let mut last = None;
        let mut no_rain = status(None);
        no_rain.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        // first sighting emits
        assert_eq!(
            status_state_if_changed(&mut last, no_rain.clone()),
            Some(no_rain.clone())
        );
        // identical weather (including rain_pct: None): silent
        assert_eq!(status_state_if_changed(&mut last, no_rain.clone()), None);

        let mut rain = no_rain.clone();
        rain.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: Some(75),
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        // rain_pct alone crossing into Some(_) emits once
        assert_eq!(
            status_state_if_changed(&mut last, rain.clone()),
            Some(rain.clone())
        );
        // repeating the same rain_pct value: silent again
        assert_eq!(status_state_if_changed(&mut last, rain), None);
    }

    // plan 131: `outlook` is the same shape of field as `rain_pct` above —
    // changes at most once per poll, never per tick — so a lone outlook
    // change (every other field held constant) must be caught by the SAME
    // ordinary derived-`PartialEq` guard, no `dedup_eq`-style special case.
    #[test]
    fn outlook_change_emits_once_then_stays_silent() {
        let mut last = None;
        let mut no_outlook = status(None);
        no_outlook.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: Vec::new(),
            }),
        };
        // first sighting emits
        assert_eq!(
            status_state_if_changed(&mut last, no_outlook.clone()),
            Some(no_outlook.clone())
        );
        // identical weather (including an empty outlook): silent
        assert_eq!(status_state_if_changed(&mut last, no_outlook.clone()), None);

        let mut with_outlook = no_outlook.clone();
        with_outlook.weather = WeatherStatus {
            enabled: true,
            current: Some(WeatherSummary {
                temp_display: "27°".to_string(),
                condition: "Cloudy".to_string(),
                is_day: true,
                rain_pct: None,
                today_high_display: None,
                today_low_display: None,
                outlook: vec![outlook_point()],
            }),
        };
        // outlook alone going from empty to populated emits once
        assert_eq!(
            status_state_if_changed(&mut last, with_outlook.clone()),
            Some(with_outlook.clone())
        );
        // repeating the same outlook value: silent again
        assert_eq!(status_state_if_changed(&mut last, with_outlook), None);
    }

    fn generic_event() -> Event {
        test_fixtures::event("t")
    }

    #[test]
    fn snapshot_reads_pause_and_waiting_from_the_queue() {
        let mut queue = SingleSlotQueue::new(50);
        queue
            .enqueue(generic_event(), std::time::Instant::now())
            .unwrap(); // unpaused: promotes
        queue.pause();

        // one item visible, nothing waiting, paused, no live match
        let snap = StatusState::snapshot(
            &queue,
            StatusInputs {
                live: None,
                espn_enabled: true,
                rss_enabled: false,
                weather: None,
                weather_enabled: false,
                media: None,
                now_playing_enabled: false,
            },
        );
        assert!(snap.paused);
        assert_eq!(snap.waiting, 0);
        assert!(snap.football.enabled);
        assert_eq!(snap.football.live, None);
        assert!(!snap.news.enabled);
        assert!(!snap.weather.enabled);
        assert_eq!(snap.weather.current, None);
        assert!(!snap.media.enabled);
        assert_eq!(snap.media.current, None);

        // paused pushes buffer instead of promoting (v5 semantics)
        queue
            .enqueue(generic_event(), std::time::Instant::now())
            .unwrap();
        assert_eq!(
            StatusState::snapshot(
                &queue,
                StatusInputs {
                    live: None,
                    espn_enabled: true,
                    rss_enabled: false,
                    weather: None,
                    weather_enabled: false,
                    media: None,
                    now_playing_enabled: false,
                },
            )
            .waiting,
            1
        );
    }

    #[test]
    fn snapshot_carries_weather_summary_and_gate() {
        let queue = SingleSlotQueue::new(50);
        let snap = StatusState::snapshot(
            &queue,
            StatusInputs {
                live: None,
                espn_enabled: true,
                rss_enabled: false,
                weather: Some(weather_summary()),
                weather_enabled: true,
                media: None,
                now_playing_enabled: false,
            },
        );
        assert!(snap.weather.enabled);
        assert_eq!(snap.weather.current, Some(weather_summary()));
    }

    #[test]
    fn snapshot_carries_media_summary_and_gate() {
        let queue = SingleSlotQueue::new(50);
        let snap = StatusState::snapshot(
            &queue,
            StatusInputs {
                live: None,
                espn_enabled: true,
                rss_enabled: false,
                weather: None,
                weather_enabled: false,
                media: Some(now_playing_summary()),
                now_playing_enabled: true,
            },
        );
        assert!(snap.media.enabled);
        assert_eq!(snap.media.current, Some(now_playing_summary()));
    }

    // plan 104: the explicit belt-and-suspenders gate — a session sitting
    // in `inputs.media` must not reach the wire while the config-level
    // toggle is off, even though in practice the poller never spawns
    // (and so never populates the AmbientSlot) while disabled.
    #[test]
    fn snapshot_hides_media_current_when_the_gate_is_off_even_if_a_session_exists() {
        let queue = SingleSlotQueue::new(50);
        let snap = StatusState::snapshot(
            &queue,
            StatusInputs {
                live: None,
                espn_enabled: true,
                rss_enabled: false,
                weather: None,
                weather_enabled: false,
                media: Some(now_playing_summary()),
                now_playing_enabled: false,
            },
        );
        assert!(!snap.media.enabled);
        assert_eq!(snap.media.current, None);
    }

    #[test]
    fn serializes_media_summary_camel_case() {
        let mut s = status(None);
        s.media = MediaStatus {
            enabled: true,
            current: Some(now_playing_summary()),
        };
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(json["media"]["enabled"], true);
        assert_eq!(json["media"]["current"]["title"], "Midnight City");
        assert_eq!(json["media"]["current"]["artist"], "M83");
        assert_eq!(
            json["media"]["current"]["album"],
            "Hurry Up, We're Dreaming"
        );
        assert_eq!(json["media"]["current"]["playing"], true);
        assert_eq!(json["media"]["current"]["elapsedMs"], 1500);
        assert_eq!(json["media"]["current"]["durationMs"], 243_000);
        assert_eq!(
            json["media"]["current"]["capturedAtMs"],
            1_753_000_000_000i64
        );
        assert_eq!(
            json["media"]["current"]["appBundleId"],
            "app.zen-browser.zen"
        );
    }

    #[test]
    fn serializes_media_current_as_null_when_absent() {
        let json = serde_json::to_value(status(None)).unwrap();
        assert!(json["media"]["current"].is_null());
    }
}
