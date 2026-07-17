use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{IF_MODIFIED_SINCE, IF_NONE_MATCH};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::RssFeedConfig;
use crate::event::{
    emit_slot_state, Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec,
};
use crate::poller::{Backoff, PauseGate};
use crate::queue::SingleSlotQueue;

const TITLE_MAX_CHARS: usize = 120;
const BODY_MAX_CHARS: usize = 240;
const MAX_FEED_BYTES: usize = 1024 * 1024;
const CATEGORY_KEYWORDS: &[(&str, &str)] = &[
    ("politic", "politics"),
    ("election", "politics"),
    ("parliament", "politics"),
    ("tech", "tech"),
    ("science", "tech"),
    ("gadget", "tech"),
    ("sport", "sports"),
    ("cricket", "sports"),
    ("football", "sports"),
    ("business", "business"),
    ("econom", "business"),
    ("market", "business"),
    ("profit", "business"),
    ("world", "world"),
    ("global", "world"),
    ("international", "world"),
];

/// Bounded, process-local memory of stories observed across every configured
/// feed. Insertion order is retained separately from membership so eviction
/// is deterministic and does not depend on hash iteration order.
#[derive(Default)]
pub struct SeenStore {
    keys: HashSet<String>,
    insertion_order: VecDeque<(String, Instant)>,
}

impl SeenStore {
    pub const MAX_KEYS: usize = 1_000;
    pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

    pub fn insert(&mut self, key: String, now: Instant) {
        while let Some((oldest_key, inserted_at)) = self.insertion_order.front() {
            let expired = now
                .checked_duration_since(*inserted_at)
                .map(|age| age > Self::MAX_AGE)
                .unwrap_or(false);
            if !expired {
                break;
            }
            self.keys.remove(oldest_key);
            self.insertion_order.pop_front();
        }

        if !self.keys.insert(key.clone()) {
            return;
        }
        self.insertion_order.push_back((key, now));

        while self.keys.len() > Self::MAX_KEYS {
            if let Some((oldest_key, _)) = self.insertion_order.pop_front() {
                self.keys.remove(&oldest_key);
            }
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.keys.contains(key)
    }
}

fn dedup_key(guid: Option<&str>, link: Option<&str>) -> Option<String> {
    if let Some(guid) = guid.filter(|value| !value.trim().is_empty()) {
        return Some(guid.to_string());
    }

    link.map(canonical_link).filter(|value| !value.is_empty())
}

/// Produces the deliberately small canonical form required for RSS dedup.
/// This is not a general URL normalizer: it handles ordinary absolute HTTP(S)
/// links, lowercases only the scheme and authority, removes query/fragment,
/// and removes at most one trailing slash. Percent-encoding, default ports,
/// dot-segments, and relative URLs are intentionally left untouched.
fn canonical_link(url: &str) -> String {
    let trimmed = url.trim();
    let cut_at = [trimmed.find('?'), trimmed.find('#')]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(trimmed.len());
    let without_suffix = &trimmed[..cut_at];

    let mut canonical = if let Some(scheme_end) = without_suffix.find("://") {
        let scheme = &without_suffix[..scheme_end];
        let after_scheme = &without_suffix[scheme_end + 3..];
        let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        let path = &after_scheme[authority_end..];
        format!(
            "{}://{}{}",
            scheme.to_lowercase(),
            authority.to_lowercase(),
            path
        )
    } else {
        without_suffix.to_string()
    };

    if canonical.ends_with('/') {
        canonical.pop();
    }
    canonical
}

fn strip_html_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' if !in_tag => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let value = if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()
            } else if let Some(decimal) = entity.strip_prefix('#') {
                decimal.parse::<u32>().ok()
            } else {
                None
            }?;
            char::from_u32(value)
        }
    }
}

fn decode_entities(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '&' {
            if let Some(relative_end) = chars[index + 1..].iter().position(|ch| *ch == ';') {
                let end = index + 1 + relative_end;
                let entity: String = chars[index + 1..end].iter().collect();
                if let Some(decoded) = decode_entity(&entity) {
                    out.push(decoded);
                    index = end + 1;
                    continue;
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }

    out
}

fn sanitize(text: &str, max_chars: usize) -> String {
    let decoded = decode_entities(&strip_html_tags(text));
    let mut collapsed = String::with_capacity(decoded.len());
    let mut pending_space = false;

    for ch in decoded.chars() {
        if ch.is_whitespace() {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(ch);
        }
    }

    let mut chars = collapsed.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn derive_category(entry_categories: &[String], feed_default: Option<&str>) -> Option<String> {
    let entry_categories: Vec<String> = entry_categories
        .iter()
        .map(|category| category.to_lowercase())
        .collect();

    CATEGORY_KEYWORDS
        .iter()
        .find_map(|(keyword, category)| {
            entry_categories
                .iter()
                .any(|term| term.contains(keyword))
                .then(|| (*category).to_string())
        })
        .or_else(|| feed_default.map(str::to_lowercase))
}

fn derive_source(configured_source: Option<&str>, feed: &feed_rs::model::Feed) -> Option<String> {
    configured_source.map(str::to_string).or_else(|| {
        feed.title.as_ref().and_then(|title| {
            let title = title.content.trim();
            (!title.is_empty()).then(|| title.to_string())
        })
    })
}

/// Pure set-difference and event-building heart of the RSS poller. All new
/// keys enter the shared store before baseline/display filtering, so skipped
/// or rate-limited stories cannot replay on a later tick.
pub fn diff_feed(
    seen: &mut SeenStore,
    feed: &feed_rs::model::Feed,
    feed_config: &RssFeedConfig,
    baseline: bool,
    max_per_poll: usize,
    ttl_secs: u64,
    now: Instant,
) -> Vec<Event> {
    let mut candidates = Vec::new();
    let source = derive_source(feed_config.source.as_deref(), feed);

    for (order, entry) in feed.entries.iter().enumerate() {
        let guid = (!entry.id.trim().is_empty()).then_some(entry.id.as_str());
        let link = entry.links.first().map(|link| link.href.as_str());
        let Some(key) = dedup_key(guid, link) else {
            tracing::debug!("rss entry skipped: no guid or link");
            continue;
        };
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key, now);

        if baseline {
            continue;
        }

        let title = sanitize(
            entry
                .title
                .as_ref()
                .map(|title| title.content.as_str())
                .unwrap_or_default(),
            TITLE_MAX_CHARS,
        );
        if title.is_empty() {
            continue;
        }
        let body = sanitize(
            entry
                .summary
                .as_ref()
                .map(|summary| summary.content.as_str())
                .unwrap_or_default(),
            BODY_MAX_CHARS,
        );
        let published = entry
            .published
            .as_ref()
            .or(entry.updated.as_ref())
            .map(|date| date.timestamp_millis());
        let entry_categories: Vec<String> = entry
            .categories
            .iter()
            .map(|category| category.term.clone())
            .collect();
        let category = derive_category(&entry_categories, feed_config.category.as_deref());
        let event = Event {
            id: Uuid::new_v4(),
            event_type: EventType::NewsItem,
            priority: Priority::Low,
            rotation: RotationSpec::OneShot { ttl_secs },
            topic: None,
            // news has no football signal; Generic keeps the frontend's
            // signature moments (goal/red card) exclusive to real signals
            signal: EventSignal::Generic,
            payload: EventPayload { title, body },
            meta: EventMeta {
                source: source.clone(),
                category,
                published_at_ms: published,
                link: link.map(str::to_string),
            },
        };
        candidates.push((published, order, event));
    }

    candidates.sort_by(|left, right| match (left.0, right.0) {
        (Some(left_date), Some(right_date)) => left_date
            .cmp(&right_date)
            .then_with(|| left.1.cmp(&right.1)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.1.cmp(&right.1),
    });

    if candidates.len() > max_per_poll {
        let dropped = candidates.len() - max_per_poll;
        tracing::warn!(dropped, max_per_poll, "rss poll display cap reached");
        candidates.drain(..dropped);
    }

    candidates.into_iter().map(|(_, _, event)| event).collect()
}

struct FeedState {
    etag: Option<String>,
    last_modified: Option<String>,
    backoff: Backoff,
    baseline: bool,
}

impl Default for FeedState {
    fn default() -> Self {
        Self {
            etag: None,
            last_modified: None,
            backoff: Backoff::default(),
            baseline: true,
        }
    }
}

async fn fetch_feed(
    client: &reqwest::Client,
    url: &str,
    state: &mut FeedState,
) -> anyhow::Result<Option<feed_rs::model::Feed>> {
    let mut request = client.get(url);
    if let Some(etag) = &state.etag {
        request = request.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = &state.last_modified {
        request = request.header(IF_MODIFIED_SINCE, last_modified);
    }

    let response = request.send().await?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    if response.status() != reqwest::StatusCode::OK {
        anyhow::bail!("unexpected http status {}", response.status());
    }

    // read validators now, but only persist them after a successful parse:
    // storing them on a failure path would make the next poll 304 and
    // silently never retry an oversized/unparseable response.
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    if response
        .content_length()
        .is_some_and(|length| length > MAX_FEED_BYTES as u64)
    {
        anyhow::bail!("response body exceeds 1 MiB");
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_FEED_BYTES {
        anyhow::bail!("response body exceeds 1 MiB");
    }

    let feed = feed_rs::parser::parse(&bytes[..])?;
    state.etag = etag;
    state.last_modified = last_modified;
    Ok(Some(feed))
}

pub fn spawn_rss_poller(
    app: tauri::AppHandle,
    queue: Arc<Mutex<SingleSlotQueue>>,
    feeds: Vec<crate::config::RssFeedConfig>,
    poll_secs: u64,
    ttl_secs: u64,
    max_per_poll: usize,
    active: Arc<AtomicBool>,
) {
    tauri::async_runtime::spawn(async move {
        let client = match reqwest::Client::builder()
            .user_agent("notchtap/0.1 (+https://github.com/jainChetan81/notchtap)")
            .redirect(reqwest::redirect::Policy::limited(3))
            .timeout(Duration::from_secs(10))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("rss poller could not build http client: {error}");
                return;
            }
        };
        let mut states: Vec<FeedState> = feeds.iter().map(|_| FeedState::default()).collect();
        let mut seen = SeenStore::default();
        let mut gate = PauseGate::new();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs.max(15)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(?feeds, poll_secs, "rss poller started");

        loop {
            interval.tick().await;
            let tick = gate.tick(active.load(Ordering::Relaxed));
            if tick.rebaseline {
                for state in &mut states {
                    state.baseline = true;
                }
                tracing::info!("rss polling resumed from tray; re-baselining");
            }
            if !tick.poll {
                continue;
            }

            for (feed_config, state) in feeds.iter().zip(&mut states) {
                let now = Instant::now();
                if !state.backoff.ready(now) {
                    continue;
                }

                let feed = match fetch_feed(&client, &feed_config.url, state).await {
                    Ok(Some(feed)) => {
                        state.backoff.on_success();
                        feed
                    }
                    Ok(None) => {
                        state.backoff.on_success();
                        continue;
                    }
                    Err(error) => {
                        tracing::warn!(feed = %feed_config.url, "rss poll failed: {error}");
                        state.backoff.on_failure(now);
                        continue;
                    }
                };

                let events = diff_feed(
                    &mut seen,
                    &feed,
                    feed_config,
                    state.baseline,
                    max_per_poll,
                    ttl_secs,
                    now,
                );
                state.baseline = false;

                for event in events {
                    let slot_change = {
                        let mut queue = queue.lock().await;
                        if let Err(error) = queue.enqueue(event) {
                            tracing::warn!(feed = %feed_config.url, "rss event dropped: {error}");
                        }
                        queue.slot_state_if_changed()
                    };
                    if let Some(slot_state) = slot_change {
                        emit_slot_state(&app, slot_state);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_feed(items: &str) -> feed_rs::model::Feed {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test News</title>
    <link>https://example.com</link>
    <description>Fixture feed</description>
    {items}
  </channel>
</rss>"#
        );
        feed_rs::parser::parse(xml.as_bytes()).unwrap()
    }

    fn feed_config(source: Option<&str>, category: Option<&str>) -> RssFeedConfig {
        RssFeedConfig {
            url: "https://example.com/feed".to_string(),
            source: source.map(str::to_string),
            category: category.map(str::to_string),
        }
    }

    #[test]
    fn seen_store_contains_and_insert_basics() {
        let mut seen = SeenStore::default();
        let now = Instant::now();
        assert!(!seen.contains("story"));
        seen.insert("story".to_string(), now);
        assert!(seen.contains("story"));
        seen.insert("story".to_string(), now + Duration::from_secs(1));
        assert_eq!(seen.keys.len(), 1);
    }

    #[test]
    fn seen_store_evicts_oldest_at_cap() {
        let mut seen = SeenStore::default();
        let now = Instant::now();
        for index in 0..SeenStore::MAX_KEYS {
            seen.insert(format!("story-{index}"), now);
        }
        seen.insert("newest".to_string(), now);
        assert!(!seen.contains("story-0"));
        assert!(seen.contains("story-1"));
        assert!(seen.contains("newest"));
        assert_eq!(seen.keys.len(), SeenStore::MAX_KEYS);
    }

    #[test]
    fn seen_store_evicts_keys_older_than_max_age() {
        let mut seen = SeenStore::default();
        let now = Instant::now();
        seen.insert("old".to_string(), now);
        seen.insert(
            "fresh".to_string(),
            now + SeenStore::MAX_AGE + Duration::from_secs(1),
        );
        assert!(!seen.contains("old"));
        assert!(seen.contains("fresh"));
    }

    #[test]
    fn dedup_key_prefers_nonempty_guid_then_falls_back_to_link() {
        assert_eq!(
            dedup_key(
                Some(" guid-verbatim "),
                Some("https://EXAMPLE.com/story/?x=1")
            ),
            Some(" guid-verbatim ".to_string())
        );
        assert_eq!(
            dedup_key(
                Some("  "),
                Some(" https://EXAMPLE.com/Story/?utm_source=rss#top ")
            ),
            Some("https://example.com/Story".to_string())
        );
        assert_eq!(dedup_key(None, None), None);
        assert_eq!(dedup_key(Some(""), Some("   ")), None);
    }

    #[test]
    fn canonical_link_normalizes_only_the_locked_components() {
        assert_eq!(
            canonical_link(" HTTPS://News.Example.COM/World/Story/?utm_source=rss#section "),
            "https://news.example.com/World/Story"
        );
        assert_eq!(
            canonical_link("http://EXAMPLE.com/CaseSensitive"),
            "http://example.com/CaseSensitive"
        );
    }

    #[test]
    fn sanitize_cleans_real_shaped_ndtv_content() {
        let title = sanitize("NDTV &amp; World", TITLE_MAX_CHARS);
        let body = sanitize(
            r#"<img src="lead.jpg">  Read <a href="/story">full story</a> &amp; updates"#,
            BODY_MAX_CHARS,
        );
        assert_eq!(title, "NDTV & World");
        assert_eq!(body, "Read full story & updates");
    }

    #[test]
    fn sanitize_decodes_numeric_entities_and_collapses_whitespace() {
        assert_eq!(
            sanitize(
                "  A&#39;B  &#x1F4F0;\n\t C &quot;D&quot; &apos;E&apos; ",
                100
            ),
            "A'B 📰 C \"D\" 'E'"
        );
    }

    #[test]
    fn sanitize_truncates_multibyte_text_on_a_char_boundary() {
        assert_eq!(sanitize("éclair", 2), "éc…");
        assert_eq!(sanitize("📰news", 1), "📰…");
    }

    #[test]
    fn category_derivation_uses_entry_tag_hit() {
        assert_eq!(
            derive_category(&["Science".to_string()], Some("world")),
            Some("tech".to_string())
        );
    }

    #[test]
    fn category_derivation_uses_keyword_table_order() {
        assert_eq!(
            derive_category(&["Tech and Parliament".to_string()], None),
            Some("politics".to_string())
        );
    }

    #[test]
    fn category_derivation_falls_back_to_feed_default() {
        assert_eq!(
            derive_category(&["Culture".to_string()], Some("BUSINESS")),
            Some("business".to_string())
        );
    }

    #[test]
    fn category_derivation_returns_none_without_match_or_default() {
        assert_eq!(derive_category(&["Culture".to_string()], None), None);
    }

    #[test]
    fn source_fallback_chain_prefers_config_then_feed_title_then_none() {
        let mut feed = parse_feed("");
        feed.title.as_mut().unwrap().content = "  Test News  ".to_string();

        assert_eq!(
            derive_source(Some("Configured Source"), &feed),
            Some("Configured Source".to_string())
        );
        assert_eq!(derive_source(None, &feed), Some("Test News".to_string()));

        feed.title = None;
        assert_eq!(derive_source(None, &feed), None);
    }

    #[test]
    fn diff_feed_baseline_records_without_emitting() {
        let feed = parse_feed(
            r#"<item><guid>a</guid><title>First</title><link>https://example.com/a</link></item>"#,
        );
        let now = Instant::now();
        let mut seen = SeenStore::default();
        assert!(diff_feed(
            &mut seen,
            &feed,
            &feed_config(None, None),
            true,
            10,
            10,
            now
        )
        .is_empty());
        assert!(seen.contains("a"));
        assert!(diff_feed(
            &mut seen,
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            now
        )
        .is_empty());
    }

    #[test]
    fn diff_feed_new_story_has_locked_event_semantics() {
        let baseline = parse_feed(
            r#"<item><guid>a</guid><title>First</title><link>https://example.com/a</link></item>"#,
        );
        let changed = parse_feed(
            r#"
<item><guid>a</guid><title>First</title><link>https://example.com/a</link></item>
<item><guid>b</guid><title>Second &amp;amp; Latest</title><link>https://example.com/b</link><description><![CDATA[<p>Details</p>]]></description></item>
"#,
        );
        let now = Instant::now();
        let mut seen = SeenStore::default();
        diff_feed(
            &mut seen,
            &baseline,
            &feed_config(None, None),
            true,
            10,
            17,
            now,
        );
        let events = diff_feed(
            &mut seen,
            &changed,
            &feed_config(None, None),
            false,
            10,
            17,
            now,
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.event_type, EventType::NewsItem);
        assert_eq!(event.priority, Priority::Low);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 17 });
        assert_eq!(event.topic, None);
        assert_eq!(event.payload.title, "Second & Latest");
        assert_eq!(event.payload.body, "Details");
    }

    #[test]
    fn diff_feed_events_carry_source_category_published_at_and_link_meta() {
        let feed = parse_feed(
            r#"
<item>
  <guid>story</guid>
  <title>Science story</title>
  <link>HTTPS://News.Example.com/Story/?utm_source=one#top</link>
  <category>Science</category>
  <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
</item>
"#,
        );
        let entry_link = feed.entries[0].links[0].href.clone();
        let events = diff_feed(
            &mut SeenStore::default(),
            &feed,
            &feed_config(Some("Configured News"), Some("world")),
            false,
            10,
            10,
            Instant::now(),
        );

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].meta,
            EventMeta {
                source: Some("Configured News".to_string()),
                category: Some("tech".to_string()),
                published_at_ms: Some(1_704_067_200_000),
                link: Some(entry_link),
            }
        );
    }

    #[test]
    fn diff_feed_guid_without_link_carries_no_link() {
        let feed = parse_feed(r#"<item><guid>story</guid><title>Story</title></item>"#);

        let events = diff_feed(
            &mut SeenStore::default(),
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Instant::now(),
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].meta.link, None);
    }

    #[test]
    fn diff_feed_sorts_oldest_first_and_undated_last() {
        let feed = parse_feed(
            r#"
<item><guid>new</guid><title>Newest</title><pubDate>Wed, 03 Jan 2024 00:00:00 GMT</pubDate></item>
<item><guid>undated</guid><title>Undated</title></item>
<item><guid>old</guid><title>Oldest</title><pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate></item>
<item><guid>middle</guid><title>Middle</title><pubDate>Tue, 02 Jan 2024 00:00:00 GMT</pubDate></item>
"#,
        );
        let events = diff_feed(
            &mut SeenStore::default(),
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Instant::now(),
        );
        let titles: Vec<_> = events
            .iter()
            .map(|event| event.payload.title.as_str())
            .collect();
        assert_eq!(titles, ["Oldest", "Middle", "Newest", "Undated"]);
    }

    #[test]
    fn diff_feed_cap_keeps_newest_and_records_every_key() {
        let feed = parse_feed(
            r#"
<item><guid>old</guid><title>Oldest</title><pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate></item>
<item><guid>middle</guid><title>Middle</title><pubDate>Tue, 02 Jan 2024 00:00:00 GMT</pubDate></item>
<item><guid>new</guid><title>Newest</title><pubDate>Wed, 03 Jan 2024 00:00:00 GMT</pubDate></item>
"#,
        );
        let mut seen = SeenStore::default();
        let events = diff_feed(
            &mut seen,
            &feed,
            &feed_config(None, None),
            false,
            2,
            10,
            Instant::now(),
        );
        let titles: Vec<_> = events
            .iter()
            .map(|event| event.payload.title.as_str())
            .collect();
        assert_eq!(titles, ["Middle", "Newest"]);
        assert!(seen.contains("old"));
        assert!(seen.contains("middle"));
        assert!(seen.contains("new"));
    }

    #[test]
    fn diff_feed_skips_title_that_sanitizes_to_empty() {
        let feed = parse_feed(
            r#"<item><guid>empty</guid><title><![CDATA[<img src="only.jpg">]]></title></item>"#,
        );
        let mut seen = SeenStore::default();
        let events = diff_feed(
            &mut seen,
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Instant::now(),
        );
        assert!(events.is_empty());
        assert!(seen.contains("empty"));
    }

    #[test]
    fn diff_feed_shared_store_dedups_canonical_link_across_feeds() {
        let mut first = parse_feed(
            r#"<item><guid>first-feed-id</guid><title>Story</title><link>HTTPS://News.Example.com/Story/?utm_source=one</link></item>"#,
        );
        let mut second = parse_feed(
            r#"<item><guid>second-feed-id</guid><title>Same story</title><link>https://news.example.com/Story#top</link></item>"#,
        );
        // feed-rs synthesizes ids when a source omits guid. Clear the parsed
        // ids to exercise the contract's canonical-link fallback directly.
        first.entries[0].id.clear();
        second.entries[0].id.clear();

        let now = Instant::now();
        let mut seen = SeenStore::default();
        assert_eq!(
            diff_feed(
                &mut seen,
                &first,
                &feed_config(None, None),
                false,
                10,
                10,
                now,
            )
            .len(),
            1
        );
        assert!(diff_feed(
            &mut seen,
            &second,
            &feed_config(None, None),
            false,
            10,
            10,
            now
        )
        .is_empty());
    }
}
