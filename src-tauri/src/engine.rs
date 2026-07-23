//! The Engine (CONTEXT.md): the one module through which every Slot
//! mutation flows. Every queue mutation in the codebase follows one
//! protocol — lock → mutate → `slot_state_if_changed()` → unlock →
//! `wake.notify_waiters()` → `emit_slot_state` — and this module makes
//! that protocol structural rather than conventional: the queue, the
//! wake, and the live-match handle are all private, so a mutation that
//! skips the protocol does not compile, because nothing outside this
//! module can reach the queue (plan 037).

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::QueueError;
use crate::event::{emit_slot_state, Event, RotationSpec, SlotState, SourceKind};
use crate::history::HistoryStore;
use crate::notifier::{ConnectorHandle, ConnectorHealth};
use crate::queue::SingleSlotQueue;
use crate::status::{
    emit_status_state, status_state_if_changed, LiveMatchSummary, NowPlayingSummary, StatusInputs,
    StatusState, WeatherSummary,
};

/// A single ambient status side-channel: an `Arc<Mutex<Option<T>>>` plus
/// the compare-then-store-then-wake-only-on-change update shape and the
/// read/clone/drop snapshot shape, both previously hand-duplicated once
/// per channel (plan 073 generalization — see `update_live_match`'s doc
/// comment for the history this replaces). Parameterized by the summary
/// type so `live`/`weather` (and any future ambient channel) share one
/// implementation instead of copying the ~10-line lock/compare/store/wake
/// block per channel. Deliberately NOT `derive(Clone)`: that would add an
/// unneeded `T: Clone` bound on the derive itself (the manual impl below
/// only ever clones the `Arc`, never `T`).
struct AmbientSlot<T> {
    inner: Arc<StdMutex<Option<T>>>,
}

impl<T> AmbientSlot<T> {
    fn new() -> Self {
        Self {
            inner: Arc::new(StdMutex::new(None)),
        }
    }
}

impl<T> Clone for AmbientSlot<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone + PartialEq> AmbientSlot<T> {
    /// Locks, compares to the new value, stores it, and wakes `wake`
    /// ONLY if it changed — the same compare-then-store-then-wake shape
    /// `update_live_match`/`update_weather` each used to implement
    /// inline. The handle is written independently of the queue lock;
    /// nobody holds both at the same time (callers pass `&self.wake`,
    /// never the queue).
    fn update(&self, value: Option<T>, wake: &tokio::sync::Notify) {
        let changed = {
            let mut guard = self.inner.lock().unwrap();
            if *guard == value {
                false
            } else {
                *guard = value;
                true
            }
        };
        if changed {
            wake.notify_waiters();
        }
    }

    /// Read/clone/drop the handle — same lock discipline every caller
    /// already follows: read the ambient handles BEFORE locking the
    /// queue, so nobody ever holds both locks at once.
    fn snapshot(&self) -> Option<T> {
        self.inner.lock().unwrap().clone()
    }
}

/// Owns the queue, the heartbeat wake, the app handle, the Connectors,
/// and (plan 034's territory, folded into plan 037) the live-match handle
/// and source-enabled flags the rotation loop needs to also be the sole
/// StatusState emitter — all private. Nothing outside this module can
/// lock the queue or touch the wake, so the mutate→wake→emit protocol is
/// structural, not conventional.
pub struct Engine<R: tauri::Runtime = tauri::Wry> {
    queue: Arc<Mutex<SingleSlotQueue>>,
    wake: Arc<tokio::sync::Notify>,
    app: tauri::AppHandle<R>,
    connectors: Arc<Vec<ConnectorHandle>>,
    telegram_health: Arc<StdMutex<ConnectorHealth>>,
    live: AmbientSlot<LiveMatchSummary>,
    weather: AmbientSlot<WeatherSummary>,
    /// plan 104: the now-playing ambient summary — same `AmbientSlot`
    /// shape as `live`/`weather`, fed by `now_playing.rs`'s supervised
    /// streaming child instead of a timer-polled http fetch.
    now_playing: AmbientSlot<NowPlayingSummary>,
    espn_enabled: bool,
    rss_enabled: bool,
    weather_enabled: bool,
    /// plan 104: the panel-editable half of the two-gate design
    /// (`config.rs`'s `now_playing_enabled` doc comment) — gates
    /// `MediaStatus.enabled`/`current` in the status assembly exactly like
    /// `weather_enabled` gates `WeatherStatus`.
    now_playing_enabled: bool,
    /// plan 088: `None` when history is disabled (the default) — the
    /// `accept` hook is then a no-op and behavior is byte-identical to
    /// pre-088. `Some` when the operator opted in and the store opened
    /// successfully. Injected rather than constructed here so the hook is
    /// testable against a temp dir instead of the operator's real config
    /// directory.
    history: Option<Arc<HistoryStore>>,
}

// Clone by hand (Arc clones + AppHandle clone + bool copies), like
// AppState's — derived Clone would needlessly require `R: Clone`.
impl<R: tauri::Runtime> Clone for Engine<R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            wake: self.wake.clone(),
            app: self.app.clone(),
            connectors: self.connectors.clone(),
            telegram_health: self.telegram_health.clone(),
            live: self.live.clone(),
            weather: self.weather.clone(),
            now_playing: self.now_playing.clone(),
            espn_enabled: self.espn_enabled,
            rss_enabled: self.rss_enabled,
            weather_enabled: self.weather_enabled,
            now_playing_enabled: self.now_playing_enabled,
            history: self.history.clone(),
        }
    }
}

impl<R: tauri::Runtime> Engine<R> {
    /// Takes the queue BY VALUE and creates BOTH the wake and the
    /// live-match handle internally — by construction, no code outside
    /// this module can ever hold the queue Arc, the wake, or the
    /// live-match handle: there is nothing to alias.
    // plan 088 pushed this to 8 positional params (over clippy's default
    // 7-arg threshold) by adding `history` as the last one. A named-field
    // params struct (the fix `StatusInputs` used for a similar overflow,
    // plans 047/048) is a bigger surface change than this plan's scope —
    // it would touch every one of the 9 call sites' shape, not just add
    // an argument — so the minimal, in-scope fix is this allow, matching
    // the codebase's existing precedent of allowing a targeted clippy
    // lint with a comment (see `SlotState`'s `large_enum_variant` allow
    // in event.rs) rather than restructuring around it.
    // plan 104 pushed this to 9 positional params by adding
    // `now_playing_enabled` alongside `weather_enabled` — same accepted
    // tradeoff plan 088's own comment above this allow already explains
    // for `history`: a named-field params struct would touch every one
    // of this method's call sites' shape, which is a bigger surface
    // change than either plan's scope.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        queue: SingleSlotQueue,
        app: tauri::AppHandle<R>,
        connectors: Arc<Vec<ConnectorHandle>>,
        telegram_health: Arc<StdMutex<ConnectorHealth>>,
        espn_enabled: bool,
        rss_enabled: bool,
        weather_enabled: bool,
        now_playing_enabled: bool,
        history: Option<Arc<HistoryStore>>,
    ) -> Self {
        Self {
            queue: Arc::new(Mutex::new(queue)),
            wake: Arc::new(tokio::sync::Notify::new()),
            app,
            connectors,
            telegram_health,
            live: AmbientSlot::new(),
            weather: AmbientSlot::new(),
            now_playing: AmbientSlot::new(),
            espn_enabled,
            rss_enabled,
            weather_enabled,
            now_playing_enabled,
            history,
        }
    }

    /// Read accessor for the telegram connector's delivery health
    /// (plan 076): the notifier worker writes through the shared `Arc`,
    /// the settings window's `get_connector_health` command reads it
    /// here. Unlike `live`/`weather` the `Arc` is passed in — the worker
    /// lives outside `Engine` and needs a writable clone (same shape as
    /// `connectors`). Clones the guarded value out; `ConnectorHealth` is
    /// `Copy`.
    pub fn telegram_health(&self) -> ConnectorHealth {
        *self.telegram_health.lock().unwrap()
    }

    /// Read accessor for the shared history store, mirroring
    /// `telegram_health`'s pattern: the settings window's
    /// `get_history`/`clear_history` commands (`settings.rs`) call this
    /// to reach the SAME `Arc<HistoryStore>` the accept path in `accept`
    /// (above) appends through, rather than opening a second
    /// `HistoryStore` over the same file. Two instances would mean two
    /// independent instance-level locks guarding one file — no mutual
    /// exclusion between an accept-path append and a settings-window
    /// clear. `None` when history is disabled (the default), matching
    /// `self.history`'s own meaning. Cheap: an `Option<Arc<_>>` clone.
    pub fn history_store(&self) -> Option<Arc<HistoryStore>> {
        self.history.clone()
    }

    /// Propagating mutation — async callers (http, settings, pollers).
    /// Reads Instant::now() ONCE (the system's only wall-clock read for
    /// queue time), locks, runs `f`, captures slot_state_if_changed,
    /// unlocks, notify_waiters, emits. Emit stays after unlock — the
    /// known deferred reordering (plans/README.md); now one site.
    // The seam this doc comment predicted before it had a second caller
    // (`accept` was the only async mutation): plan 121's `clear_queue`/
    // `skip_current` settings commands are the next async mutation sites,
    // walking through this same door rather than hand-rolling their own
    // lock/mutate/wake/emit — no bespoke Engine wrapper method needed for
    // either, `apply` IS the wrapper.
    pub async fn apply<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T {
        let now = Instant::now();
        let (out, slot_change) = {
            let mut q = self.queue.lock().await;
            let out = f(&mut q, now);
            (out, q.slot_state_if_changed())
        };
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            emit_slot_state(&self.app, state);
        }
        out
    }

    /// Propagating mutation — main-thread callers (tray, hotkeys).
    /// Same body with blocking_lock; keeps toggle_pause's
    /// debug_assert!(Handle::try_current().is_err()) guard.
    pub fn apply_blocking<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T {
        // menu events and global-shortcut handlers arrive on the main
        // thread, outside the tokio runtime, so a blocking lock is safe
        // here
        debug_assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "tray/hotkey handlers must arrive off the tokio runtime; blocking_lock would deadlock"
        );
        let now = Instant::now();
        let (out, slot_change) = {
            let mut q = self.queue.blocking_lock();
            let out = f(&mut q, now);
            (out, q.slot_state_if_changed())
        };
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            emit_slot_state(&self.app, state);
        }
        out
    }

    /// Non-propagating read — no wake, no emit, & not &mut.
    pub async fn read<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T {
        let q = self.queue.lock().await;
        f(&q)
    }

    /// Non-propagating read — main-thread callers. No wake, no emit.
    pub fn read_blocking<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T {
        let q = self.queue.blocking_lock();
        f(&q)
    }

    /// The one ingest path (replaces http's old shared enqueue helper,
    /// rss's inline loop, AND espn's old fan-out helper): enqueue with the
    /// mutate→wake→emit protocol, then Connector fan-out. The CONTEXT.md
    /// Connector rule is encoded HERE: an Event whose origin is
    /// SourceKind::News is never offered. QueueFull returns early — no
    /// wake, no offer (the old shared helper's semantics): a malformed
    /// `/notify` request never reaches this function at all (the
    /// title/body `MissingField` checks in `notify_handler` return a 400
    /// before an `Event` is even constructed), and every accepted event
    /// is fanned out, emitted, and used to wake the rotation loop —
    /// routed through this one shared method rather than duplicated at
    /// each caller, so a mutation without a wake is structurally
    /// impossible to express here.
    pub async fn accept(
        &self,
        event: Event,
        bypass_pause_when_slot_empty: bool,
    ) -> Result<(), QueueError> {
        let to_offer = event.clone();
        let now = Instant::now();
        let slot_change = {
            let mut q = self.queue.lock().await;
            let enqueue_result = if bypass_pause_when_slot_empty {
                q.enqueue_test(event, now)
            } else {
                q.enqueue(event, now)
            };
            if let Err(ref e) = enqueue_result {
                tracing::warn!(id = %to_offer.id, origin = ?to_offer.origin, error = ?e, "accept: enqueue rejected");
            }
            enqueue_result?;
            tracing::debug!(id = %to_offer.id, origin = ?to_offer.origin, priority = ?to_offer.priority, "accept: enqueued");
            q.slot_state_if_changed()
        };
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            emit_slot_state(&self.app, state);
        }
        if to_offer.origin != SourceKind::News {
            for connector in self.connectors.iter() {
                connector.offer(&to_offer);
            }
        }
        // plan 088: best-effort history append. ONE-SHOT ONLY —
        // `Recurring` is the ambient live-scoreboard card, which
        // topic-supersedes on every poll cycle; recording it would bury
        // the discrete notifications this feature exists to recover under
        // ~100 near-identical score updates per match. A write failure
        // must never fail an accept: the notification already promoted.
        // Log id/origin only — never title/body, matching this function's
        // existing content-clean logging.
        if let Some(store) = &self.history {
            if matches!(to_offer.rotation, RotationSpec::OneShot { .. }) {
                if let Err(e) = store.append(&to_offer) {
                    tracing::warn!(id = %to_offer.id, origin = ?to_offer.origin, error = %e, "history append failed");
                }
            }
        }
        Ok(())
    }

    /// Replaces the espn poller's direct `live.lock().unwrap()` +
    /// `wake.notify_waiters()` dance (poller.rs, plan 034): compares to
    /// the new summary, stores it, and wakes the rotation loop ONLY if it
    /// changed — via the shared `AmbientSlot::update` (plan 073; was
    /// hand-written inline here and in `update_weather` until then). Not
    /// `apply`/`accept`: it never touches the queue.
    pub fn update_live_match(&self, summary: Option<LiveMatchSummary>) {
        self.live.update(summary, &self.wake);
    }

    /// The weather twin of `update_live_match` (plan 040 Part B): the
    /// ambient weather summary the weather poller folds into the idle
    /// rail. Same `AmbientSlot::update` call, same shape, just a
    /// different summary type.
    pub fn update_weather(&self, summary: Option<WeatherSummary>) {
        self.weather.update(summary, &self.wake);
    }

    /// The now-playing twin of `update_weather` (plan 104): the ambient
    /// media summary `now_playing.rs`'s supervised streaming child pushes
    /// on every changed adapter diff line. Same `AmbientSlot::update`
    /// call, same shape, just a different summary type and producer
    /// lifecycle (a held-open child, not a timer).
    pub fn update_now_playing(&self, summary: Option<NowPlayingSummary>) {
        self.now_playing.update(summary, &self.wake);
    }

    /// The rotation loop (formerly lib.rs spawn_heartbeat), moved inside
    /// the Engine so `wake` never escapes. It is the *consumer* of the
    /// wake, not a producer, so it gets its own private loop (lock →
    /// tick → emit-if-changed → arm → read deadline → unlock →
    /// sleep/await; NO notify from this path — running it through
    /// `apply` would wake the loop itself every iteration and spin).
    ///
    /// Deadline-based (plan 015): sleeps until the visible item's
    /// rotation deadline (or forever when idle) and is woken by any
    /// queue mutation. A small grace addition avoids sub-ms re-loops at
    /// the edge. plan 036: the wake waiter is armed *while holding the
    /// queue lock* (`Notified::enable`), so a wake landing between
    /// unlock and park can never be lost. plan 034: the loop is also the
    /// SOLE status-state emitter — every mutation already reaches it, so
    /// each pass recomputes the StatusState under the same queue lock as
    /// the slot-state tick and emits only when it differs from the
    /// previous pass (`last_status`).
    pub fn spawn_rotation(&self) {
        let app = self.app.clone();
        let queue = self.queue.clone();
        let wake = self.wake.clone();
        let live = self.live.clone();
        let weather = self.weather.clone();
        let now_playing = self.now_playing.clone();
        let espn_enabled = self.espn_enabled;
        let rss_enabled = self.rss_enabled;
        let weather_enabled = self.weather_enabled;
        let now_playing_enabled = self.now_playing_enabled;
        tauri::async_runtime::spawn(async move {
            let mut last_status: Option<StatusState> = None;
            loop {
                // Arm the wake waiter *while holding the queue lock* (plan
                // 036): every mutation site locks the queue before mutating
                // and calls `notify_waiters()` after unlocking, so a waiter
                // registered under the lock can never miss a mutation this
                // iteration's `next_deadline()` didn't already see.
                let notified = wake.notified();
                tokio::pin!(notified);
                let deadline = {
                    // plan 034 lock discipline: read/clone/drop the
                    // live-match handle BEFORE locking the queue — nobody
                    // holds both at the same time.
                    let live_summary = live.snapshot();
                    let weather_summary = weather.snapshot();
                    let now_playing_summary = now_playing.snapshot();
                    let mut q = queue.lock().await;
                    q.tick(Instant::now());
                    if let Some(state) = q.slot_state_if_changed() {
                        emit_slot_state(&app, state);
                    }
                    let status = StatusState::snapshot(
                        &q,
                        StatusInputs {
                            live: live_summary,
                            espn_enabled,
                            rss_enabled,
                            weather: weather_summary,
                            weather_enabled,
                            media: now_playing_summary,
                            now_playing_enabled,
                        },
                    );
                    if let Some(changed) = status_state_if_changed(&mut last_status, status) {
                        emit_status_state(&app, changed);
                    }
                    notified.as_mut().enable();
                    q.next_deadline()
                };
                match deadline {
                    Some(at) => {
                        tokio::select! {
                            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(at + Duration::from_millis(10))) => {}
                            _ = notified.as_mut() => {}
                        }
                    }
                    None => notified.await,
                }
            }
        });
    }

    /// Webview-reload re-emit (the on_page_load slot-state site): reads
    /// current_slot_state and emits it UNCONDITIONALLY (dedup
    /// deliberately bypassed — a freshly reloaded webview has no state,
    /// so it must be re-sent even if unchanged), returns it for the
    /// eval-splice global seed. No wake: nothing mutated.
    ///
    /// plan 107 Step C: routes through `current_slot_state_for_emission`
    /// (not the plain `current_slot_state`) so this one wire-emission
    /// path that bypasses `slot_state_if_changed`'s dedup gate still
    /// feeds the TTL-restart sampler — see that method's doc.
    pub fn emit_current_blocking(&self) -> SlotState {
        let state = {
            let mut q = self.queue.blocking_lock();
            q.current_slot_state_for_emission(Instant::now())
        };
        emit_slot_state(&self.app, state.clone());
        state
    }

    /// Non-emitting read of the current `StatusState` — same inputs as
    /// `emit_current_status_blocking`, no wire event. Originally added
    /// for plan 087's hover tracking-area handler (`lib.rs`), which
    /// needed this on every mouse-move to derive
    /// `hover::status_rail_active`, the idle-card WIDTH formula's
    /// `has_status_chips` input — that need went away when plan 091
    /// collapsed the idle/idle-status width split (there is no wider
    /// idle variant to pick anymore), and plan 093's y-span rect no
    /// longer needs `StatusState` either (its `idle_peek_open` input is
    /// hover hysteresis, not ambient-data availability — see
    /// `hover::active_card_rect`'s doc). This non-emitting shape (no
    /// re-emitting `status-state` on every read, which would flood the
    /// webview and defeat `hover-changed`'s own transitions-only idle-cost
    /// discipline, plans 015/018) is kept as `pub` for its current real
    /// caller, `emit_current_status_blocking` below. Lock discipline
    /// matches that caller: live/weather locked and dropped before the
    /// queue lock.
    pub fn status_snapshot_blocking(&self) -> StatusState {
        let live_summary = self.live.snapshot();
        let weather_summary = self.weather.snapshot();
        let now_playing_summary = self.now_playing.snapshot();
        let q = self.queue.blocking_lock();
        StatusState::snapshot(
            &q,
            StatusInputs {
                live: live_summary,
                espn_enabled: self.espn_enabled,
                rss_enabled: self.rss_enabled,
                weather: weather_summary,
                weather_enabled: self.weather_enabled,
                media: now_playing_summary,
                now_playing_enabled: self.now_playing_enabled,
            },
        )
    }

    /// The on_page_load StatusState twin of `emit_current_blocking` —
    /// same shape, over StatusState instead of SlotState, using the
    /// Engine's own `live`/`espn_enabled`/`rss_enabled`. Also bypasses
    /// the rotation loop's `last_status` dedup (that guard belongs to
    /// the loop's task, not to this method) for the same reload reason.
    /// No wake. Lock discipline: the live-match handle is read/cloned/
    /// dropped BEFORE the queue lock (nobody holds both at once), same
    /// as the rotation loop.
    pub fn emit_current_status_blocking(&self) -> StatusState {
        let state = self.status_snapshot_blocking();
        emit_status_state(&self.app, state.clone());
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QueueError;
    use crate::event::{test_fixtures, Priority, RotationSpec, SLOT_STATE_EVENT};
    use crate::status::STATUS_STATE_EVENT;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn event(priority: Priority) -> Event {
        test_fixtures::with_priority(test_fixtures::event("t"), priority)
    }

    fn live_summary(minute: &str) -> LiveMatchSummary {
        LiveMatchSummary {
            label: "Arsenal 2–0 Chelsea".to_string(),
            minute: minute.to_string(),
        }
    }

    fn test_engine(app: &tauri::App<tauri::test::MockRuntime>) -> Engine<tauri::test::MockRuntime> {
        Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            None,
        )
    }

    /// a connector whose receiving end the test holds, so fan-out can be
    /// asserted without any worker or network (pattern from http.rs's
    /// test_connector)
    fn test_connector() -> (ConnectorHandle, tokio::sync::mpsc::Receiver<Event>) {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        (ConnectorHandle::new("test", tx), rx)
    }

    #[tokio::test]
    async fn apply_wakes_and_emits() {
        // Generalized port of http.rs's old wake regression test
        // (plan 015 review follow-up): registering the waiter *before* the
        // call via `enable()` proves the wake fires — `notify_waiters()`
        // only wakes tasks already parked, it never queues a permit for a
        // future `.notified()` call.
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        engine
            .apply(|q, now| q.enqueue(event(Priority::Medium), now).unwrap())
            .await;

        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("apply must wake the rotation loop");
    }

    #[tokio::test]
    async fn read_never_wakes() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        let waiting = engine.read(|q| q.total_waiting()).await;
        assert_eq!(waiting, 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), notified)
                .await
                .is_err(),
            "a read must never wake the rotation loop"
        );
    }

    #[tokio::test]
    async fn accept_offers_manual_but_never_news() {
        // The CONTEXT.md Connector rule, encoded in `accept`: every accepted
        // event is offered EXCEPT News-origin ones. First coverage of this
        // rule — before plan 037, a News test notification leaked to
        // telegram via the old shared enqueue path's offer-all loop.
        let app = tauri::test::mock_app();
        let (connector, mut rx) = test_connector();
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(vec![connector]),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            None,
        );

        engine.accept(event(Priority::Medium), false).await.unwrap();
        let offered = rx
            .try_recv()
            .expect("manual event must reach the connector");
        assert_eq!(offered.payload.title, "t");

        let mut news = event(Priority::Medium);
        news.origin = SourceKind::News;
        engine.accept(news, false).await.unwrap();
        assert!(
            rx.try_recv().is_err(),
            "News-origin events are never offered to connectors"
        );
    }

    #[tokio::test]
    async fn accept_queue_full_propagates_nothing() {
        let app = tauri::test::mock_app();
        let (connector, mut rx) = test_connector();
        let engine = Engine::new(
            SingleSlotQueue::new(1),
            app.handle().clone(),
            Arc::new(vec![connector]),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            None,
        );

        // fill the Medium tier: first promotes into the visible slot,
        // second takes the one waiting slot
        engine.accept(event(Priority::Medium), false).await.unwrap();
        engine.accept(event(Priority::Medium), false).await.unwrap();
        let _ = rx.try_recv();
        let _ = rx.try_recv();

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        let err = engine
            .accept(event(Priority::Medium), false)
            .await
            .unwrap_err();
        assert!(matches!(err, QueueError::QueueFull));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), notified)
                .await
                .is_err(),
            "QueueFull must not wake the rotation loop"
        );
        assert!(rx.try_recv().is_err(), "QueueFull must not fan out");
    }

    #[tokio::test]
    async fn rotation_loop_parked_idle_wakes_on_accept() {
        // plan 036 regression, ported from lib.rs's
        // heartbeat_parked_idle_wakes_on_enqueue_and_rotates_out: with the
        // queue empty the rotation loop parks with no fallback timer, so an
        // accept's `notify_waiters()` is its ONLY chance to learn about the
        // new item. Exercises the fixed path end-to-end: idle-park → accept
        // wakes → item rotates out on its deadline.
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.spawn_rotation();

        // Give the loop time to finish its first iteration and reach the
        // idle park before the accept arrives.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut short_lived = event(Priority::Medium);
        short_lived.rotation = RotationSpec::OneShot { ttl_secs: 1 };
        engine.accept(short_lived, false).await.unwrap();
        assert!(matches!(
            engine.read(|q| q.current_slot_state()).await,
            SlotState::Showing { .. }
        ));

        let rotated = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if engine.read(|q| q.current_slot_state()).await == SlotState::Empty {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        assert!(
            rotated.is_ok(),
            "expected the idle-parked rotation loop to wake on accept and rotate the item out within 3s"
        );
    }

    #[tokio::test]
    async fn rotation_loop_rotates_out_via_deadline_sleep_not_polling() {
        // plan 015, ported from lib.rs's
        // heartbeat_rotates_out_via_deadline_sleep_not_polling: the rotation
        // loop sleeps until the visible item's rotation deadline (or forever
        // when idle) instead of polling a fixed 250ms interval — this proves
        // the sleep still fires and rotates the item out once its window
        // elapses, driven purely by the deadline (plus apply's wake after
        // the enqueue, the same wake every mutation path performs).
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.spawn_rotation();

        let mut short_lived = event(Priority::Medium);
        short_lived.rotation = RotationSpec::OneShot { ttl_secs: 1 };
        engine
            .apply(|q, now| q.enqueue(short_lived, now).unwrap())
            .await;
        assert!(matches!(
            engine.read(|q| q.current_slot_state()).await,
            SlotState::Showing { .. }
        ));

        let rotated = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if engine.read(|q| q.current_slot_state()).await == SlotState::Empty {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        assert!(
            rotated.is_ok(),
            "expected the item to rotate out via the deadline-based rotation loop within 3s"
        );
    }

    // plan 081 REQUIRED regression test — the tripwire for attempt 1's
    // finding. The rotation loop above (the `loop` in `spawn_rotation`) is
    // woken by `notify_waiters()` after EVERY mutation, then independently
    // re-locks the queue and calls `q.tick` + `q.slot_state_if_changed()` a
    // SECOND time, downstream of the mutation site's (`accept`'s) own
    // emit — this is the shipped plan-036/037 protocol, working as
    // designed. `SlotState::Showing::remaining_ms` is computed from
    // `Instant::now()` inside `current_slot_state`, so the two calls a few
    // ms apart always differed on that field alone — attempt 1 measured
    // exactly 2 emissions for 1 `accept()` before `SlotState::dedup_eq`
    // (which deliberately excludes `remaining_ms`, see its doc comment in
    // event.rs) existed. With the dedup split in place, this must be
    // exactly 1: the rotation loop's recheck sees an unchanged `ttl_ms` and
    // no other differing field, so it dedupes and does not re-emit.
    //
    // If a future change makes this assert !=1 again, it means either the
    // dedup split broke, or a new non-deduping emitter was introduced
    // elsewhere — do not "fix" this test by loosening the assertion.
    //
    // Ordering note: `accept()` runs BEFORE `spawn_rotation()` here,
    // deliberately — `tauri::async_runtime::spawn` schedules onto Tauri's
    // own (real, separate) async runtime, not this test's `#[tokio::test]`
    // one, so a rotation loop already running against an *empty* queue
    // races the test's own setup for real OS-scheduling reasons having
    // nothing to do with this regression: its first iteration sees
    // `last_emitted == None` and unconditionally emits a startup `Empty`
    // state, and under real system load there is no fixed settle delay
    // that reliably drains that emission before the observation window
    // starts (a fixed `sleep` + counter reset was tried and measured
    // flaky under a full-suite-load run). Doing the one `accept()` call
    // FIRST means the queue already holds the visible item by the time
    // `spawn_rotation()` starts, so the loop's first iteration is itself
    // the "downstream recheck" this test exists to exercise — no empty
    // queue, no startup emission, nothing to race.
    #[tokio::test]
    async fn one_accept_emits_exactly_one_slot_state_despite_live_remaining_ms() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let emit_count = Arc::new(AtomicUsize::new(0));
        {
            use tauri::Listener;
            let counter = emit_count.clone();
            app.handle().listen(SLOT_STATE_EVENT, move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        // long ttl so the item is still visible (and no rotation-out
        // occurs) during the observation window below.
        let mut long_lived = event(Priority::Medium);
        long_lived.rotation = RotationSpec::OneShot { ttl_secs: 30 };
        engine.accept(long_lived, false).await.unwrap();
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            1,
            "accept() itself must have emitted exactly once"
        );

        // only now start the rotation loop — its first iteration is the
        // downstream recheck under test.
        engine.spawn_rotation();

        // give the rotation loop's recheck time to run.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            1,
            "expected exactly one slot-state emission per accept() — the rotation \
             loop's post-wake recheck must dedupe via remaining_ms-exclusive comparison"
        );
    }

    #[test]
    fn emit_current_returns_and_emits() {
        // The webview-reload re-emit: dedup deliberately bypassed, so two
        // back-to-back calls both emit AND return the same state (a fresh
        // webview has no state and must be re-sent even if unchanged).
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let emit_count = Arc::new(AtomicUsize::new(0));
        {
            use tauri::Listener;
            let counter = emit_count.clone();
            app.handle().listen(SLOT_STATE_EVENT, move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        engine.apply_blocking(|q, now| q.enqueue(event(Priority::Medium), now).unwrap());
        // the promotion itself emitted once via apply_blocking; count only
        // the re-emits from here on
        emit_count.store(0, Ordering::SeqCst);

        let first = engine.emit_current_blocking();
        let second = engine.emit_current_blocking();
        assert!(matches!(first, SlotState::Showing { .. }));
        assert_eq!(first, second);
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            2,
            "dedup is bypassed: both calls emit"
        );
    }

    #[test]
    fn emit_current_status_returns_and_emits() {
        // StatusState twin of emit_current_returns_and_emits (plan 034's
        // second on_page_load re-emit block): same bypass, over the
        // Engine's own live/flags.
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            false,
            false,
            false,
            None,
        );
        engine.update_live_match(Some(live_summary("45'")));

        let emit_count = Arc::new(AtomicUsize::new(0));
        {
            use tauri::Listener;
            let counter = emit_count.clone();
            app.handle().listen(STATUS_STATE_EVENT, move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        let first = engine.emit_current_status_blocking();
        let second = engine.emit_current_status_blocking();
        assert_eq!(first, second);
        assert_eq!(first.football.live, Some(live_summary("45'")));
        assert!(first.football.enabled);
        assert!(!first.news.enabled);
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            2,
            "dedup is bypassed: both calls emit"
        );
    }

    #[tokio::test]
    async fn update_live_match_wakes_only_on_change() {
        // Mirrors status.rs's status_state_if_changed test pattern: store
        // on change wakes; re-store of the same value does not.
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_live_match(Some(live_summary("45'")));
        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("a new live-match summary must wake the rotation loop");

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_live_match(Some(live_summary("45'")));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), notified)
                .await
                .is_err(),
            "an unchanged summary must not wake"
        );

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_live_match(Some(live_summary("60'")));
        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("a changed summary must wake again");
    }

    fn weather_summary(temp: &str) -> WeatherSummary {
        WeatherSummary {
            temp_display: temp.to_string(),
            condition: "Cloudy".to_string(),
            is_day: true,
        }
    }

    #[tokio::test]
    async fn update_weather_wakes_only_on_change() {
        // The weather twin of update_live_match_wakes_only_on_change
        // (plan 040 Part B): store-on-change wakes; re-store does not.
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_weather(Some(weather_summary("27°")));
        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("a new weather summary must wake the rotation loop");

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_weather(Some(weather_summary("27°")));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), notified)
                .await
                .is_err(),
            "an unchanged summary must not wake"
        );

        let notified = engine.wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        engine.update_weather(Some(weather_summary("28°")));
        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("a changed summary must wake again");
    }

    // plan 088: history hook tests. `with_limits` in a fresh temp dir per
    // test — never the real config dir (see history.rs's own temp_dir()
    // helper for why a shared dir is unsafe here too).
    fn history_temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "notchtap-enginehistorytest-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn accept_records_one_shot_event_to_history() {
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let store =
            Arc::new(crate::history::HistoryStore::with_limits(&dir, 5 * 1024 * 1024, 2).unwrap());
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            Some(store.clone()),
        );

        let mut one_shot = event(Priority::Medium);
        one_shot.rotation = RotationSpec::OneShot { ttl_secs: 8 };
        engine.accept(one_shot, false).await.unwrap();

        let entries = store.read_recent(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event.payload.title, "t");
    }

    #[tokio::test]
    async fn accept_does_not_record_recurring_event() {
        // THE TRIPWIRE for the core design decision: `Recurring` events
        // (the ambient live-scoreboard card) must never be written to
        // history — if someone later "simplifies" the gate away, this
        // test must fail.
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let store =
            Arc::new(crate::history::HistoryStore::with_limits(&dir, 5 * 1024 * 1024, 2).unwrap());
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            Some(store.clone()),
        );

        let mut recurring = event(Priority::Medium);
        recurring.rotation = RotationSpec::Recurring { display_secs: 8 };
        engine.accept(recurring, false).await.unwrap();

        let entries = store.read_recent(10).unwrap();
        assert!(
            entries.is_empty(),
            "Recurring events must never be recorded to history"
        );
    }

    #[tokio::test]
    async fn accept_with_history_disabled_writes_nothing() {
        // Engine built with `None` (test_engine's default) — the store is
        // never even constructed, so there is no directory `accept` could
        // possibly write to. Assert the field directly (tests share
        // module scope with private fields) AND that an unrelated temp
        // dir stays empty, as a belt-and-suspenders regression pin: if a
        // future change made the `None` path fall back to constructing a
        // default store somewhere, this would catch the field flipping
        // to `Some` even before any file shows up.
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let engine = test_engine(&app);
        assert!(
            engine.history.is_none(),
            "test_engine must build with history disabled"
        );

        engine.accept(event(Priority::Medium), false).await.unwrap();

        assert!(
            !dir.join("history.jsonl").exists(),
            "history_enabled off must create no history.jsonl anywhere, \
             let alone in this unrelated temp dir"
        );
    }

    // --- history_store(): the accessor settings.rs's get_history/
    // clear_history commands now route through instead of opening a
    // second `HistoryStore` (real review finding — two instances over
    // one file share no lock, so a settings-window clear could interleave
    // unguarded with an accept-path append). `Arc::ptr_eq` is the actual
    // assertion that matters: same allocation means same instance-level
    // `Mutex`, i.e. one lock guarding the file, not two.

    #[tokio::test]
    async fn history_store_returns_a_clone_of_the_same_arc_the_accept_path_writes_through() {
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let store =
            Arc::new(crate::history::HistoryStore::with_limits(&dir, 5 * 1024 * 1024, 2).unwrap());
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            Arc::new(StdMutex::new(ConnectorHealth::default())),
            true,
            true,
            false,
            false,
            Some(store.clone()),
        );

        let accessed = engine
            .history_store()
            .expect("history_store() must return Some when the engine was built with a store");
        assert!(
            Arc::ptr_eq(&accessed, &store),
            "history_store() must hand back the SAME Arc<HistoryStore> the accept path holds, \
             not a clone of a freshly-opened instance — a distinct instance would carry its own \
             Mutex and reintroduce the unguarded race this accessor exists to close"
        );

        // And it is genuinely the live instance: an append made directly
        // through the accessor's handle is visible to a read through the
        // original `store` binding, and vice versa.
        accessed
            .append(&test_fixtures::event("via-accessor"))
            .unwrap();
        let entries = store.read_recent(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event.payload.title, "via-accessor");
    }

    #[tokio::test]
    async fn history_store_is_none_when_history_is_disabled() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        assert!(engine.history_store().is_none());
    }

    // plan 121: settings-window Queue section — `clear_waiting`, called
    // through `apply` (no bespoke Engine method needed; see `apply`'s own
    // doc comment above), must reach the overlay as a fresh slot-state
    // emit so the visible card's progress dots update. This is
    // `apply`'s existing `slot_state_if_changed`-gated emit, exercised
    // here specifically for a mutation that changes `queue_total`
    // without touching `visible` at all — the case `apply_wakes_and_emits`
    // above doesn't cover.
    #[tokio::test]
    async fn clear_queue_apply_emits_a_fresh_slot_state_for_the_progress_dots() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

        // one promotes to visible, one is left WAITING.
        engine.accept(event(Priority::Medium), false).await.unwrap();
        engine.accept(event(Priority::Medium), false).await.unwrap();

        let emit_count = Arc::new(AtomicUsize::new(0));
        {
            use tauri::Listener;
            let counter = emit_count.clone();
            app.handle().listen(SLOT_STATE_EVENT, move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        let dropped = engine.apply(|q, _now| q.clear_waiting()).await;
        assert_eq!(dropped, 1);
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            1,
            "clear_waiting changing queue_total (batch_total) while a card is still visible \
             must produce exactly one fresh slot-state emit — the settings window's Clear \
             queue action has no other way to update the overlay's progress dots"
        );
    }

    // The companion case: nothing visible, nothing left waiting after the
    // clear — `current_slot_state()` was `Empty` before and stays `Empty`
    // after, so there is genuinely nothing for the wire to say and `apply`
    // correctly emits nothing. Paused so the second enqueue stays WAITING
    // rather than promoting into `visible` (mirrors
    // `pause_sends_enqueues_to_waiting_even_with_free_slot` in queue.rs).
    #[tokio::test]
    async fn clear_queue_apply_emits_nothing_when_nothing_was_ever_visible() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply(|q, _now| q.pause()).await;

        engine.accept(event(Priority::Medium), false).await.unwrap();

        let emit_count = Arc::new(AtomicUsize::new(0));
        {
            use tauri::Listener;
            let counter = emit_count.clone();
            app.handle().listen(SLOT_STATE_EVENT, move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        let dropped = engine.apply(|q, _now| q.clear_waiting()).await;
        assert_eq!(dropped, 1);
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            0,
            "Empty -> Empty is not a slot-state change; get_queue's own refetch (not a wire \
             emit) is how the settings window learns the list is now empty"
        );
    }
}
