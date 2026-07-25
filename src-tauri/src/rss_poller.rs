use std::collections::{HashSet, VecDeque};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use reqwest::header::{IF_MODIFIED_SINCE, IF_NONE_MATCH};
use tauri::Manager;
use uuid::Uuid;

use crate::config::RssFeedConfig;
use crate::engine::Engine;
use crate::event::{
    Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SourceKind,
};
use crate::poller::Backoff;

const TITLE_MAX_CHARS: usize = 120;
const BODY_MAX_CHARS: usize = 240;
// plan 130: matches http.rs's SUBTITLE_MAX_CHARS (same fixed-window
// display-safety rationale) — the topic label rides the same subtitle
// slot a `/notify` caller's rich-relay subtitle uses.
const TOPIC_SUBTITLE_MAX_CHARS: usize = 120;
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

/// Expands a plain-language topic ("aston villa transfers") into a
/// Google News query-feed URL (plan 130 Step 1). The `hl`/`gl`/`ceid`
/// triple is required, not decorative: Google News's RSS search
/// endpoint is a documented quirk here — omitting any of the three
/// yields empty or inconsistent results. Shared verbatim by the
/// continuous poller's topic list (`merge_feed_sources`, below) and the
/// one-shot `search_once` — a query typed into "Search now" expands
/// exactly the same way as a configured topic line, one path, no fork.
pub(crate) fn expand_topic_url(topic: &str) -> String {
    let mut url =
        reqwest::Url::parse("https://news.google.com/rss/search").expect("static url must parse");
    url.query_pairs_mut()
        .append_pair("q", topic)
        .append_pair("hl", "en-US")
        .append_pair("gl", "US")
        .append_pair("ceid", "US:en");
    url.to_string()
}

/// One poll unit: either a configured feed (`topic: None`) or a
/// topic-expanded query feed (`topic: Some(label)`). `diff_feed` uses
/// the label to stamp `meta.subtitle` on every event it produces from
/// this source — a plain feed keeps today's no-subtitle behavior
/// unchanged.
pub(crate) struct PollSource {
    pub(crate) config: RssFeedConfig,
    pub(crate) topic: Option<String>,
}

/// Merges configured feeds with topic-expanded query feeds into ONE
/// poll list (plan 130 Step 1): feeds first, then topics in configured
/// order — an operator reading their own config top-to-bottom sees the
/// same order reflected in poll sequence. Each topic line is trimmed;
/// an empty (or whitespace-only) line is skipped rather than expanding
/// to a nonsense query.
pub(crate) fn merge_feed_sources(feeds: &[RssFeedConfig], topics: &[String]) -> Vec<PollSource> {
    let mut sources: Vec<PollSource> = feeds
        .iter()
        .cloned()
        .map(|config| PollSource {
            config,
            topic: None,
        })
        .collect();

    for topic in topics {
        let trimmed = topic.trim();
        if trimmed.is_empty() {
            continue;
        }
        sources.push(PollSource {
            config: RssFeedConfig {
                url: expand_topic_url(trimmed),
                source: None,
                category: None,
            },
            topic: Some(trimmed.to_string()),
        });
    }

    sources
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
        // Whitespace-like named entities decode to a plain ASCII space (not
        // U+00A0/U+2002/etc) so the whitespace-collapse pass in `sanitize`
        // merges runs of them like any other space — Google News RSS in
        // particular emits `&nbsp;&nbsp;` between title and source.
        "nbsp" | "ensp" | "emsp" | "thinsp" => Some(' '),
        "ndash" => Some('–'),
        "mdash" => Some('—'),
        "hellip" => Some('…'),
        "lsquo" => Some('‘'),
        "rsquo" => Some('’'),
        "ldquo" => Some('“'),
        "rdquo" => Some('”'),
        "middot" => Some('·'),
        "bull" => Some('•'),
        "copy" => Some('©'),
        "reg" => Some('®'),
        "trade" => Some('™'),
        "deg" => Some('°'),
        "plusmn" => Some('±'),
        "times" => Some('×'),
        "laquo" => Some('«'),
        "raquo" => Some('»'),
        "euro" => Some('€'),
        "pound" => Some('£'),
        "cent" => Some('¢'),
        "sect" => Some('§'),
        "para" => Some('¶'),
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

// Longest entity text `decode_entity` recognizes is a numeric form like
// `#x10FFFF` (8 chars); 10 leaves slack without reopening the O(n^2) scan.
const MAX_ENTITY_LEN: usize = 10;

fn decode_entities(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '&' {
            // The window must hold up to MAX_ENTITY_LEN entity chars PLUS
            // the terminating `;` itself, hence +2 (not +1): an entity of
            // exactly MAX_ENTITY_LEN chars has its `;` at relative offset
            // MAX_ENTITY_LEN, which needs a window of MAX_ENTITY_LEN + 1
            // elements past the `&` to be visible to `.position()`.
            let window_end = (index + 2 + MAX_ENTITY_LEN).min(chars.len());
            if let Some(relative_end) = chars[index + 1..window_end]
                .iter()
                .position(|ch| *ch == ';')
            {
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
    // Output is truncated to max_chars below; stripping/decoding never
    // lengthens text, so a bounded prefix is behavior-identical and keeps
    // hostile multi-hundred-KB fields from costing full-length passes.
    let bounded: String = text.chars().take(max_chars * 8).collect();
    let decoded = decode_entities(&strip_html_tags(&bounded));
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

/// True when a feed's URL is a Google News RSS endpoint — including the
/// topic-expanded query feeds `expand_topic_url` builds, which always
/// point at `news.google.com`. Google News is the ONE source whose
/// `<description>` is a documented, fixed shape (`title + source name`,
/// nothing else); an arbitrary configured feed offers no such guarantee,
/// so this gates the title-prefix-plus-short-tail rule in
/// `body_is_redundant` below.
fn is_google_news_feed(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|parsed| parsed.host_str() == Some("news.google.com"))
        .unwrap_or(false)
}

/// Detects a body that adds nothing over the title, so `diff_feed` can
/// suppress it instead of rendering a duplicate of the headline back at
/// the reader. Two rules are universally safe: (a) an empty body, and
/// (b) a body fully contained in the title. A third rule — body equals
/// title plus a short (<24 char) tail — is gated by `allow_title_prefix_tail`:
/// it's only valid for Google News, whose `<description>` is always
/// `title + source name` verbatim (see `is_google_news_feed`). Applied to
/// an arbitrary configured feed, that rule would wrongly erase a genuine
/// short summary like title "Earthquake hits city" / body "Earthquake
/// hits city; 12 dead".
fn body_is_redundant(title: &str, body: &str, allow_title_prefix_tail: bool) -> bool {
    fn normalize(text: &str) -> String {
        text.chars()
            .filter(|ch| ch.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }

    let title = normalize(title);
    let body = normalize(body);

    if body.is_empty() {
        return true;
    }
    if title.contains(&body) {
        return true;
    }
    if allow_title_prefix_tail {
        if let Some(remainder) = body.strip_prefix(title.as_str()) {
            if !title.is_empty() && remainder.chars().count() < 24 {
                return true;
            }
        }
    }
    false
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
// 9 args (plan 130 added `topic`) trips clippy 1.97's too_many_arguments —
// this is the pure, exhaustively-tested core and every argument is a
// distinct test axis; bundling them would only obscure the test call
// sites.
#[allow(clippy::too_many_arguments)]
pub fn diff_feed(
    seen: &mut SeenStore,
    feed: &feed_rs::model::Feed,
    feed_config: &RssFeedConfig,
    baseline: bool,
    max_per_poll: usize,
    ttl_secs: u64,
    priority: Priority,
    now: Instant,
    // plan 130: `Some(label)` for a topic-expanded source — stamped onto
    // every event's `meta.subtitle`. `None` for a plain configured feed,
    // which keeps the pre-130 no-subtitle behavior byte-identical.
    topic: Option<&str>,
) -> Vec<Event> {
    let mut candidates = Vec::new();
    let source = derive_source(feed_config.source.as_deref(), feed);
    let google_news = is_google_news_feed(&feed_config.url);

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
        let mut body = sanitize(
            entry
                .summary
                .as_ref()
                .map(|summary| summary.content.as_str())
                .unwrap_or_default(),
            BODY_MAX_CHARS,
        );
        // Google News RSS `<description>` is just the title (and source)
        // repeated, not a real summary; suppress it rather than echo the
        // headline back in the body slot. The frontend falls back to the
        // headline in the expanded panel when body is empty. The
        // title-prefix-plus-short-tail rule only fires for Google News
        // feeds (`google_news`) — see `body_is_redundant`'s doc comment.
        if body_is_redundant(&title, &body, google_news) {
            body = String::new();
        }
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
            priority,
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
                // plan 035: rss items carry no details. plan 130: a
                // topic-derived item's subtitle carries the topic label
                // that produced it; a plain configured feed still has
                // none.
                subtitle: topic.map(|label| sanitize(label, TOPIC_SUBTITLE_MAX_CHARS)),
                details: Vec::new(),
                // plan 083: espn-only field; rss never populates it.
                espn: None,
            },
            origin: SourceKind::News,
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

    let body = crate::net::read_body_capped(response, MAX_FEED_BYTES).await?;

    let feed = feed_rs::parser::parse(&body[..])?;
    state.etag = etag;
    state.last_modified = last_modified;
    Ok(Some(feed))
}

/// L-sec2 fix: a feed's `config.url` is operator-supplied and may embed a
/// token in its query string (a private feed URL, an API key param,
/// etc.) — logging it verbatim would put a secret into a world-readable
/// log file. Prefers the feed's own `source` label when the operator set
/// one; otherwise falls back to just the URL's host (query string,
/// userinfo, and path all stripped) so the log line still identifies
/// WHICH feed failed without leaking whatever the query string carries.
fn feed_log_ref(config: &RssFeedConfig) -> String {
    if let Some(source) = config.source.as_deref().filter(|s| !s.trim().is_empty()) {
        return source.to_string();
    }
    reqwest::Url::parse(&config.url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "<unparseable feed url>".to_string())
}

// plan 037: ingest goes through `Engine::accept`, same as the espn
// poller — rss's deliberately offer-less inline loop is subsumed by
// accept's origin gate (News events are never offered to connectors).
//
// plan 130: `topics` merges with `feeds` (via `merge_feed_sources`) into
// ONE poll list — same SeenStore, same TTL/priority/max-per-poll, same
// News tier as a configured feed. `app_handle` reaches the
// `StdMutex<SeenStore>` tauri manages as app state (`lib.rs`'s
// `.setup()`), the SAME instance `settings::search_news_now`'s one-shot
// search dedups against — sharing an app-managed value rather than
// threading a new field through `Engine::new` (whose ~dozen call sites,
// mostly test doubles, would otherwise all need updating for a poller
// this deliberately isn't unit-tested — see this module's own top
// comment referenced from `weather_poller.rs`).
#[allow(clippy::too_many_arguments)]
pub fn spawn_rss_poller(
    engine: Engine,
    app_handle: tauri::AppHandle,
    feeds: Vec<crate::config::RssFeedConfig>,
    topics: Vec<String>,
    poll_secs: u64,
    ttl_secs: u64,
    max_per_poll: usize,
    priority: Priority,
) {
    tauri::async_runtime::spawn(async move {
        let client = match crate::net::build_poll_client() {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("rss poller could not build http client: {error}");
                return;
            }
        };
        let sources = merge_feed_sources(&feeds, &topics);
        let mut states: Vec<FeedState> = sources.iter().map(|_| FeedState::default()).collect();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs.max(15)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(
            feed_count = feeds.len(),
            topic_count = topics.len(),
            poll_secs,
            "rss poller started"
        );

        loop {
            interval.tick().await;

            for (source, state) in sources.iter().zip(&mut states) {
                let now = Instant::now();
                if !state.backoff.ready(now) {
                    continue;
                }

                let feed = match fetch_feed(&client, &source.config.url, state).await {
                    Ok(Some(feed)) => {
                        state.backoff.on_success();
                        feed
                    }
                    Ok(None) => {
                        state.backoff.on_success();
                        continue;
                    }
                    Err(error) => {
                        // L-sec2: never log the full feed url (it may
                        // embed a token) — `feed_log_ref` reduces it to
                        // the operator's `source` label or the bare host.
                        tracing::warn!(feed = %feed_log_ref(&source.config), "rss poll failed: {error}");
                        state.backoff.on_failure(now);
                        continue;
                    }
                };

                let events = {
                    let seen_state = app_handle.state::<StdMutex<SeenStore>>();
                    // R6: poison-tolerant — a panic while holding this
                    // lock elsewhere must not permanently kill the rss
                    // poller task, same convention as settings.rs/crests.rs.
                    let mut seen = seen_state.lock().unwrap_or_else(|e| e.into_inner());
                    diff_feed(
                        &mut seen,
                        &feed,
                        &source.config,
                        state.baseline,
                        max_per_poll,
                        ttl_secs,
                        priority,
                        now,
                        source.topic.as_deref(),
                    )
                };
                state.baseline = false;

                for event in events {
                    if let Err(error) = engine.accept(event, false).await {
                        tracing::warn!(feed = %feed_log_ref(&source.config), "rss event dropped: {error}");
                    }
                }
            }
        }
    });
}

/// One-shot fetch+diff for an ad-hoc search (`settings::search_news_now`,
/// plan 130 Step 3). The caller is expected to have built `url` via the
/// SAME `expand_topic_url` the continuous poller's topic list uses (one
/// shared path, no fork — see `search_news_now`'s own body) and to pass
/// the exact (trimmed) query back in as `topic_label`, stamped onto
/// every returned event's `meta.subtitle` the same way a configured
/// topic line's label is. Does a single fresh GET (a throwaway
/// `FeedState` — no etag/last-modified persists across calls, so a
/// repeated search always re-fetches rather than 304ing forever), and
/// dedups through the SAME `seen` store the poller loop above shares
/// via app-managed state — a story the background poller already
/// showed won't be re-enqueued by a search, and vice versa.
pub async fn search_once(
    client: &reqwest::Client,
    seen: &StdMutex<SeenStore>,
    url: &str,
    topic_label: &str,
    max_per_poll: usize,
    ttl_secs: u64,
    priority: Priority,
) -> anyhow::Result<Vec<Event>> {
    let mut state = FeedState::default();
    let feed = fetch_feed(client, url, &mut state)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no response"))?;
    let feed_config = RssFeedConfig {
        url: url.to_string(),
        source: None,
        category: None,
    };
    let now = Instant::now();
    // R6: poison-tolerant, same convention as settings.rs/crests.rs and
    // the poller loop's own lock above.
    let mut guard = seen.lock().unwrap_or_else(|e| e.into_inner());
    Ok(diff_feed(
        &mut guard,
        &feed,
        &feed_config,
        false,
        max_per_poll,
        ttl_secs,
        priority,
        now,
        Some(topic_label),
    ))
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

    fn google_news_feed_config() -> RssFeedConfig {
        RssFeedConfig {
            url: "https://news.google.com/rss/search?q=rodri".to_string(),
            source: None,
            category: None,
        }
    }

    // --- L-sec2: `feed_log_ref` must never leak the full feed url (which
    // may embed a token in its query string) into a log line ---

    #[test]
    fn feed_log_ref_prefers_the_operator_supplied_source_label() {
        let config = feed_config(Some("My Private Feed"), None);
        assert_eq!(feed_log_ref(&config), "My Private Feed");
    }

    #[test]
    fn feed_log_ref_falls_back_to_the_bare_host_with_no_source_label() {
        let mut config = feed_config(None, None);
        config.url = "https://example.com/feed?token=super-secret-value".to_string();
        let logged = feed_log_ref(&config);
        assert_eq!(logged, "example.com");
        assert!(
            !logged.contains("super-secret-value"),
            "the query string (and any token in it) must never appear in the log-safe ref"
        );
    }

    #[test]
    fn feed_log_ref_treats_a_blank_source_label_as_absent() {
        let config = feed_config(Some("   "), None);
        let logged = feed_log_ref(&config);
        assert_eq!(
            logged, "example.com",
            "a whitespace-only source must fall through to the host"
        );
    }

    #[test]
    fn feed_log_ref_never_panics_on_an_unparseable_url() {
        let mut config = feed_config(None, None);
        config.url = "not a url".to_string();
        assert_eq!(feed_log_ref(&config), "<unparseable feed url>");
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
    fn sanitize_decodes_named_entities_and_leaves_unknown_ones_verbatim() {
        assert_eq!(sanitize("Title&nbsp;&nbsp;Source", 100), "Title Source");
        assert_eq!(sanitize("Wait&mdash;really&hellip;", 100), "Wait—really…");
        assert_eq!(
            sanitize("Unknown &xyzzy; entity", 100),
            "Unknown &xyzzy; entity"
        );
    }

    #[test]
    fn sanitize_truncates_multibyte_text_on_a_char_boundary() {
        assert_eq!(sanitize("éclair", 2), "éc…");
        assert_eq!(sanitize("📰news", 1), "📰…");
    }

    #[test]
    fn ampersand_flood_without_semicolons_is_linear() {
        // Hostile input: no ';' anywhere, so the pre-cap decoder would
        // rescan the remainder of the string for every '&'. The bounded
        // window + bounded sanitize prefix make this structurally linear
        // rather than O(n^2) — this test asserts on output content (not
        // wall time), but it would still hang the old implementation.
        let flood = "&".repeat(100_000);
        let result = sanitize(&flood, 240);
        assert!(result.starts_with("&&&"));
        // truncated output is max_chars plus a single trailing ellipsis char
        assert!(result.chars().count() <= 241);
    }

    #[test]
    fn entities_still_decode_at_boundaries() {
        // ordinary named entity, unaffected by the window bound
        assert_eq!(sanitize("&amp;", 10), "&");

        // entity text exactly MAX_ENTITY_LEN (10) chars — "#x0001F600" —
        // still fits the search window and must decode to the
        // grinning-face emoji U+1F600.
        assert_eq!(sanitize("&#x0001F600;", 10), "\u{1F600}");

        // one char longer (11, "#x00001F600") pushes the ';' outside the
        // window: the literal text must pass through unchanged. This is
        // the pair that actually fails if the window arithmetic regresses.
        assert_eq!(sanitize("&#x00001F600;", 20), "&#x00001F600;");

        // existing-behavior regression guard, copied from
        // sanitize_decodes_numeric_entities_and_collapses_whitespace.
        assert_eq!(sanitize("A&#39;B &#x1F4F0;", 100), "A'B 📰");
    }

    #[test]
    fn body_is_redundant_when_equal_after_normalization() {
        // rules (a)/(b) are universal — the flag must not matter here.
        assert!(body_is_redundant(
            "Real Madrid win the league",
            "Real Madrid, win the league!",
            false
        ));
        assert!(body_is_redundant(
            "Real Madrid win the league",
            "Real Madrid, win the league!",
            true
        ));
    }

    #[test]
    fn body_is_redundant_when_body_is_a_subset_of_title() {
        // rule (b) is universal — the flag must not matter here.
        assert!(body_is_redundant(
            "Real Madrid win the league - full report",
            "Real Madrid win the league",
            false
        ));
        assert!(body_is_redundant(
            "Real Madrid win the league - full report",
            "Real Madrid win the league",
            true
        ));
    }

    #[test]
    fn body_is_redundant_when_title_followed_by_short_source_tail_and_flag_set() {
        assert!(body_is_redundant(
            "Real Madrid win the league",
            "Real Madrid win the league  Yahoo Sports",
            true
        ));
    }

    #[test]
    fn body_is_not_redundant_when_title_followed_by_short_tail_and_flag_unset() {
        // rule (c) — title-prefix-plus-short-tail — is Google-News-only.
        // For a generic feed a short genuine tail must survive.
        assert!(!body_is_redundant(
            "Real Madrid win the league",
            "Real Madrid win the league  Yahoo Sports",
            false
        ));
    }

    #[test]
    fn body_is_not_redundant_when_title_followed_by_a_real_paragraph() {
        assert!(!body_is_redundant(
            "Real Madrid win the league",
            "Real Madrid win the league after a dramatic final matchday \
             that saw three separate title contenders drop points",
            true
        ));
    }

    #[test]
    fn body_is_not_redundant_when_unrelated_to_title() {
        assert!(!body_is_redundant(
            "Real Madrid win the league",
            "Weather forecast calls for rain across the region tomorrow",
            true
        ));
    }

    #[test]
    fn is_google_news_feed_matches_only_the_news_google_com_host() {
        assert!(is_google_news_feed(
            "https://news.google.com/rss/search?q=formula+1"
        ));
        assert!(is_google_news_feed(&expand_topic_url("aston villa")));
        assert!(!is_google_news_feed("https://example.com/feed"));
        assert!(!is_google_news_feed(
            "https://notnews.google.com/rss/search?q=x"
        ));
        assert!(!is_google_news_feed("not a url"));
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

    // --- plan 130 Step 1: topic expansion + merge ---

    #[test]
    fn expand_topic_url_shape_encodes_the_query_and_carries_the_locale_triple() {
        let url = expand_topic_url("aston villa transfers");
        assert_eq!(
            url,
            "https://news.google.com/rss/search?q=aston+villa+transfers&hl=en-US&gl=US&ceid=US%3Aen"
        );
    }

    #[test]
    fn expand_topic_url_percent_encodes_reserved_query_characters() {
        let url = expand_topic_url("cmd & control?");
        assert!(url.starts_with("https://news.google.com/rss/search?q="));
        assert!(url.contains("cmd+%26+control%3F"));
        assert!(!url.contains(' '));
    }

    #[test]
    fn merge_feed_sources_puts_feeds_first_then_topics_in_order() {
        let feeds = vec![
            RssFeedConfig {
                url: "https://example.com/a.xml".to_string(),
                source: Some("A".to_string()),
                category: None,
            },
            RssFeedConfig {
                url: "https://example.com/b.xml".to_string(),
                source: None,
                category: None,
            },
        ];
        let topics = vec!["aston villa transfers".to_string(), "formula 1".to_string()];

        let merged = merge_feed_sources(&feeds, &topics);

        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].config.url, "https://example.com/a.xml");
        assert_eq!(merged[0].topic, None);
        assert_eq!(merged[1].config.url, "https://example.com/b.xml");
        assert_eq!(merged[1].topic, None);
        assert_eq!(
            merged[2].config.url,
            expand_topic_url("aston villa transfers")
        );
        assert_eq!(merged[2].topic, Some("aston villa transfers".to_string()));
        assert_eq!(merged[3].config.url, expand_topic_url("formula 1"));
        assert_eq!(merged[3].topic, Some("formula 1".to_string()));
    }

    #[test]
    fn merge_feed_sources_trims_and_skips_empty_topic_lines() {
        let topics = vec![
            "  aston villa transfers  ".to_string(),
            "".to_string(),
            "   ".to_string(),
            "formula 1".to_string(),
        ];

        let merged = merge_feed_sources(&[], &topics);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].topic, Some("aston villa transfers".to_string()));
        assert_eq!(
            merged[0].config.url,
            expand_topic_url("aston villa transfers")
        );
        assert_eq!(merged[1].topic, Some("formula 1".to_string()));
    }

    #[test]
    fn merge_feed_sources_with_no_feeds_and_no_topics_is_empty() {
        assert!(merge_feed_sources(&[], &[]).is_empty());
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
            Priority::Low,
            now,
            None,
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
            Priority::Low,
            now,
            None,
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
            Priority::Low,
            now,
            None,
        );
        let events = diff_feed(
            &mut seen,
            &changed,
            &feed_config(None, None),
            false,
            10,
            17,
            Priority::Low,
            now,
            None,
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.event_type, EventType::NewsItem);
        assert_eq!(event.priority, Priority::Low);
        assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 17 });
        assert_eq!(event.topic, None);
        assert_eq!(event.payload.title, "Second & Latest");
        assert_eq!(event.payload.body, "Details");
        assert_eq!(event.meta.subtitle, None);
    }

    #[test]
    fn diff_feed_suppresses_google_news_style_redundant_body() {
        let feed = parse_feed(
            r##"<item><guid>rodri</guid><title>Real Madrid make final decision on Rodri's transfer - Yahoo Sports</title><link>https://example.com/rodri</link><description><![CDATA[<a href="https://news.google.com/rss/articles/x">Real Madrid make final decision on Rodri's transfer - Yahoo Sports</a>&nbsp;&nbsp;<font color="#6f6f6f">Yahoo Sports</font>]]></description></item>"##,
        );
        let mut seen = SeenStore::default();
        let events = diff_feed(
            &mut seen,
            &feed,
            &google_news_feed_config(),
            false,
            10,
            10,
            Priority::Low,
            Instant::now(),
            None,
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(
            event.payload.title,
            "Real Madrid make final decision on Rodri's transfer - Yahoo Sports"
        );
        assert_eq!(event.payload.body, "");
    }

    #[test]
    fn diff_feed_keeps_short_genuine_summary_on_a_non_google_news_feed() {
        // Regression for the external-review finding: rule (c)
        // (title-prefix-plus-short-tail) must NOT fire for an arbitrary
        // configured feed — only Google News's `<description>` is
        // guaranteed to be title+source with no real content.
        let feed = parse_feed(
            r#"<item><guid>quake</guid><title>Earthquake hits city</title><link>https://example.com/quake</link><description>Earthquake hits city; 12 dead</description></item>"#,
        );
        let mut seen = SeenStore::default();
        let events = diff_feed(
            &mut seen,
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Priority::Low,
            Instant::now(),
            None,
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.payload.title, "Earthquake hits city");
        assert_eq!(event.payload.body, "Earthquake hits city; 12 dead");
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
            Priority::Low,
            Instant::now(),
            None,
        );

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].meta,
            EventMeta {
                source: Some("Configured News".to_string()),
                category: Some("tech".to_string()),
                published_at_ms: Some(1_704_067_200_000),
                link: Some(entry_link),
                subtitle: None,
                details: Vec::new(),
                espn: None,
            }
        );
    }

    #[test]
    fn diff_feed_topic_source_stamps_the_topic_as_subtitle() {
        let feed = parse_feed(
            r#"<item><guid>story</guid><title>Aston Villa sign new midfielder</title><link>https://example.com/story</link></item>"#,
        );

        let events = diff_feed(
            &mut SeenStore::default(),
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Priority::Low,
            Instant::now(),
            Some("aston villa transfers"),
        );

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].meta.subtitle,
            Some("aston villa transfers".to_string())
        );
    }

    #[test]
    fn diff_feed_topic_subtitle_is_truncated_like_any_other_field() {
        let feed = parse_feed(
            r#"<item><guid>story</guid><title>Story</title><link>https://example.com/story</link></item>"#,
        );
        let long_topic = "x".repeat(TOPIC_SUBTITLE_MAX_CHARS + 10);

        let events = diff_feed(
            &mut SeenStore::default(),
            &feed,
            &feed_config(None, None),
            false,
            10,
            10,
            Priority::Low,
            Instant::now(),
            Some(&long_topic),
        );

        assert_eq!(events.len(), 1);
        let subtitle = events[0].meta.subtitle.as_ref().unwrap();
        assert_eq!(subtitle.chars().count(), TOPIC_SUBTITLE_MAX_CHARS + 1); // +1 for the ellipsis
        assert!(subtitle.ends_with('…'));
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
            Priority::Low,
            Instant::now(),
            None,
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
            Priority::Low,
            Instant::now(),
            None,
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
            Priority::Low,
            Instant::now(),
            None,
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
            Priority::Low,
            Instant::now(),
            None,
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
                Priority::Low,
                now,
                None,
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
            Priority::Low,
            now,
            None,
        )
        .is_empty());
    }

    // --- wiremock: fetch_feed's decision surface (304 / validator
    // ordering / size cap). no live rss fetch, ever. ---

    mod fetch_feed_tests {
        use super::*;
        use wiremock::matchers::{header, header_regex, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const VALID_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test News</title>
    <link>https://example.com</link>
    <description>Fixture feed</description>
    <item><guid>a</guid><title>First</title><link>https://example.com/a</link></item>
  </channel>
</rss>"#;

        fn client() -> reqwest::Client {
            reqwest::Client::new()
        }

        #[tokio::test]
        async fn not_modified_returns_none_and_preserves_state() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(ResponseTemplate::new(304))
                .mount(&server)
                .await;

            let mut state = FeedState::default();
            let url = format!("{}/feed", server.uri());
            let result = fetch_feed(&client(), &url, &mut state).await.unwrap();

            assert!(result.is_none());
            assert_eq!(state.etag, None);
            assert_eq!(state.last_modified, None);
        }

        #[tokio::test]
        async fn validators_not_persisted_on_parse_failure() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("ETag", "\"abc123\"")
                        .insert_header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")
                        .set_body_raw("not xml", "text/xml"),
                )
                .mount(&server)
                .await;

            let mut state = FeedState::default();
            let url = format!("{}/feed", server.uri());
            let result = fetch_feed(&client(), &url, &mut state).await;

            assert!(result.is_err());
            // the bug-guard: a failed parse must NOT persist validators, or
            // the next poll would 304 forever and silently never retry.
            assert_eq!(state.etag, None);
            assert_eq!(state.last_modified, None);
        }

        #[tokio::test]
        async fn validators_persisted_on_success() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("ETag", "\"abc123\"")
                        .insert_header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")
                        .set_body_raw(VALID_FEED, "application/rss+xml"),
                )
                .mount(&server)
                .await;

            let mut state = FeedState::default();
            let url = format!("{}/feed", server.uri());
            let result = fetch_feed(&client(), &url, &mut state).await.unwrap();

            assert!(result.is_some());
            assert_eq!(state.etag.as_deref(), Some("\"abc123\""));
            assert_eq!(
                state.last_modified.as_deref(),
                Some("Wed, 21 Oct 2015 07:28:00 GMT")
            );
        }

        #[tokio::test]
        async fn oversized_content_length_rejected() {
            let server = MockServer::start().await;
            let big_body = vec![b'x'; MAX_FEED_BYTES + 1];
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(big_body))
                .mount(&server)
                .await;

            let mut state = FeedState::default();
            let url = format!("{}/feed", server.uri());
            let error = fetch_feed(&client(), &url, &mut state).await.unwrap_err();

            assert!(error.to_string().contains("1 MiB"));
            assert_eq!(state.etag, None);
            assert_eq!(state.last_modified, None);
        }

        #[tokio::test]
        async fn conditional_headers_sent_when_state_has_validators() {
            let server = MockServer::start().await;
            // `header()` splits comma-separated header values by design (for
            // multi-value headers like cache-control), which mismatches an
            // HTTP-date's internal comma. `header_regex` compares the raw
            // value instead.
            Mock::given(method("GET"))
                .and(path("/feed"))
                .and(header("If-None-Match", "\"etag-value\""))
                .and(header_regex(
                    "If-Modified-Since",
                    "^Wed, 21 Oct 2015 07:28:00 GMT$",
                ))
                .respond_with(ResponseTemplate::new(304))
                .mount(&server)
                .await;

            let mut state = FeedState {
                etag: Some("\"etag-value\"".to_string()),
                last_modified: Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
                ..FeedState::default()
            };
            let url = format!("{}/feed", server.uri());
            let result = fetch_feed(&client(), &url, &mut state).await.unwrap();

            // no `.and(header(..))` matcher registered without validators, so
            // a match here proves the request actually carried both headers.
            assert!(result.is_none());
        }
    }

    // --- plan 130 Step 3: search_once's fetch-once/dedup/subtitle
    // contract, same wiremock-not-live-fetch discipline as
    // fetch_feed_tests above. ---

    mod search_once_tests {
        use super::*;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const VALID_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test News</title>
    <link>https://example.com</link>
    <description>Fixture feed</description>
    <item><guid>a</guid><title>Aston Villa sign new midfielder</title><link>https://example.com/a</link></item>
  </channel>
</rss>"#;

        fn client() -> reqwest::Client {
            reqwest::Client::new()
        }

        #[tokio::test]
        async fn fetches_once_and_stamps_the_topic_label_as_subtitle() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(VALID_FEED, "application/rss+xml"),
                )
                .mount(&server)
                .await;

            let url = format!("{}/feed", server.uri());
            let seen = StdMutex::new(SeenStore::default());
            let events = search_once(
                &client(),
                &seen,
                &url,
                "aston villa transfers",
                10,
                10,
                Priority::Low,
            )
            .await
            .unwrap();

            assert_eq!(events.len(), 1);
            assert_eq!(
                events[0].meta.subtitle,
                Some("aston villa transfers".to_string())
            );
        }

        #[tokio::test]
        async fn dedups_against_a_seen_store_a_prior_call_already_populated() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(VALID_FEED, "application/rss+xml"),
                )
                .mount(&server)
                .await;

            let url = format!("{}/feed", server.uri());
            let seen = StdMutex::new(SeenStore::default());

            let first = search_once(
                &client(),
                &seen,
                &url,
                "aston villa transfers",
                10,
                10,
                Priority::Low,
            )
            .await
            .unwrap();
            assert_eq!(first.len(), 1);

            // Same story, second search (poller-shared SeenStore semantics):
            // already-seen keys never re-emit, mirroring the poller loop's
            // own tick-over-tick behavior.
            let second = search_once(
                &client(),
                &seen,
                &url,
                "aston villa transfers",
                10,
                10,
                Priority::Low,
            )
            .await
            .unwrap();
            assert!(second.is_empty());
        }

        #[tokio::test]
        async fn propagates_a_fetch_error_as_err() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/feed"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&server)
                .await;

            let url = format!("{}/feed", server.uri());
            let seen = StdMutex::new(SeenStore::default());
            let result =
                search_once(&client(), &seen, &url, "formula 1", 10, 10, Priority::Low).await;

            assert!(result.is_err());
        }
    }
}
