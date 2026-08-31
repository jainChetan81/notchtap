//! The Engine: the one module through which every Slot mutation flows.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::QueueError;
use crate::event::{emit_slot_state, Event, RotationSpec, SlotState};
use crate::history::HistoryStore;
use crate::queue::SingleSlotQueue;
use crate::status::{
    emit_status_state, status_state_if_changed, LiveMatchSummary, StatusInputs, StatusState,
};

/// Deliberately NOT `derive(Clone)`: that would add an unneeded `T: Clone` bound on the derive
/// itself (the manual impl below only ever clones the `Arc`, never `T`).
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
    /// The handle is written independently of the queue lock; nobody holds both at the same time
    /// (callers pass `&self.wake`, never the queue).
    fn update(&self, value: Option<T>, wake: &tokio::sync::Notify) {
        let changed = {
            // poison-tolerant (codebase convention, see settings.rs): a panic while a poller holds
            // this lock must not wedge every other ambient-channel caller behind a poisoned Mutex.
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Read/clone/drop the handle — same lock discipline every caller already follows: read the
    /// ambient handles BEFORE locking the queue, so nobody ever holds both locks at once.
    fn snapshot(&self) -> Option<T> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

pub struct Engine<R: tauri::Runtime = tauri::Wry> {
    queue: Arc<Mutex<SingleSlotQueue>>,
    wake: Arc<tokio::sync::Notify>,
    app: tauri::AppHandle<R>,
    live: AmbientSlot<LiveMatchSummary>,
    espn_enabled: bool,
    rss_enabled: bool,
    history: Option<Arc<HistoryStore>>,
    tab_wire: Arc<crate::tabs::TabWire>,
}

impl<R: tauri::Runtime> Clone for Engine<R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            wake: self.wake.clone(),
            app: self.app.clone(),
            live: self.live.clone(),
            espn_enabled: self.espn_enabled,
            rss_enabled: self.rss_enabled,
            history: self.history.clone(),
            tab_wire: self.tab_wire.clone(),
        }
    }
}

impl<R: tauri::Runtime> Engine<R> {
    // Over clippy's default 7-argument threshold. A named-field params
    // struct (what `StatusInputs` does for `StatusState::snapshot`) would
    // reshape all 9 call sites, so this stays a targeted allow — same
    // precedent as `SlotState`'s `large_enum_variant` allow in event.rs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        queue: SingleSlotQueue,
        app: tauri::AppHandle<R>,
        espn_enabled: bool,
        rss_enabled: bool,
        history: Option<Arc<HistoryStore>>,
        tab_wire: Arc<crate::tabs::TabWire>,
    ) -> Self {
        Self {
            queue: Arc::new(Mutex::new(queue)),
            wake: Arc::new(tokio::sync::Notify::new()),
            app,
            live: AmbientSlot::new(),
            espn_enabled,
            rss_enabled,
            history,
            tab_wire,
        }
    }

    pub fn history_store(&self) -> Option<Arc<HistoryStore>> {
        self.history.clone()
    }

    pub async fn apply<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T {
        let now = Instant::now();
        let mut q = self.queue.lock().await;
        let out = f(&mut q, now);
        let slot_change = q.slot_state_if_changed();
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            self.tab_wire.slot_occupied.store(
                !matches!(state, SlotState::Empty),
                std::sync::atomic::Ordering::Relaxed,
            );
            emit_slot_state(&self.app, state);
        }
        out
    }

    /// Propagating mutation — main-thread callers (tray, hotkeys).
    pub fn apply_blocking<T>(&self, f: impl FnOnce(&mut SingleSlotQueue, Instant) -> T) -> T {
        debug_assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "tray/hotkey handlers must arrive off the tokio runtime; blocking_lock would deadlock"
        );
        let now = Instant::now();
        let mut q = self.queue.blocking_lock();
        let out = f(&mut q, now);
        let slot_change = q.slot_state_if_changed();
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            self.tab_wire.slot_occupied.store(
                !matches!(state, SlotState::Empty),
                std::sync::atomic::Ordering::Relaxed,
            );
            emit_slot_state(&self.app, state);
        }
        out
    }

    /// Non-propagating read — no wake, no emit, & not &mut.
    pub async fn read<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T {
        let q = self.queue.lock().await;
        f(&q)
    }

    /// Non-propagating read — main-thread callers.
    pub fn read_blocking<T>(&self, f: impl FnOnce(&SingleSlotQueue) -> T) -> T {
        let q = self.queue.blocking_lock();
        f(&q)
    }

    pub async fn accept(
        &self,
        event: Event,
        bypass_pause_when_slot_empty: bool,
    ) -> Result<(), QueueError> {
        let recorded = event.clone();
        let now = Instant::now();
        {
            let mut q = self.queue.lock().await;
            let enqueue_result = if bypass_pause_when_slot_empty {
                q.enqueue_test(event, now)
            } else {
                q.enqueue(event, now)
            };
            if let Err(ref e) = enqueue_result {
                tracing::warn!(id = %recorded.id, origin = ?recorded.origin, error = ?e, "accept: enqueue rejected");
            }
            enqueue_result?;
            tracing::debug!(id = %recorded.id, origin = ?recorded.origin, priority = ?recorded.priority, "accept: enqueued");
            // emit under the lock, same rationale as `apply` above (see its doc comment).
            let slot_change = q.slot_state_if_changed();
            self.wake.notify_waiters();
            if let Some(state) = slot_change {
                self.tab_wire.slot_occupied.store(
                    !matches!(state, SlotState::Empty),
                    std::sync::atomic::Ordering::Relaxed,
                );
                emit_slot_state(&self.app, state);
            }
        }
        // A write failure must never fail an accept: the notification already promoted.
        if let Some(store) = &self.history {
            if matches!(recorded.rotation, RotationSpec::OneShot { .. }) {
                if let Err(e) = store.append(&recorded) {
                    tracing::warn!(id = %recorded.id, origin = ?recorded.origin, error = %e, "history append failed");
                }
            }
        }
        Ok(())
    }

    /// Compares to the new summary, stores it, and wakes the rotation loop ONLY if it changed, via
    /// the shared `AmbientSlot::update`. Not `apply`/`accept`: it never touches the queue.
    pub fn update_live_match(&self, summary: Option<LiveMatchSummary>) {
        self.live.update(summary, &self.wake);
    }

    /// The rotation loop lives inside the Engine so `wake` never escapes. A small grace addition
    /// avoids sub-ms re-loops at the edge.
    pub fn spawn_rotation(&self) {
        let app = self.app.clone();
        let queue = self.queue.clone();
        let wake = self.wake.clone();
        let live = self.live.clone();
        let espn_enabled = self.espn_enabled;
        let rss_enabled = self.rss_enabled;
        let tab_wire = self.tab_wire.clone();
        tauri::async_runtime::spawn(async move {
            let mut last_status: Option<StatusState> = None;
            loop {
                // Arm the wake waiter *while holding the queue lock*: every mutation site locks
                // the queue before mutating and calls `notify_waiters()` after unlocking.
                let notified = wake.notified();
                tokio::pin!(notified);
                let deadline = {
                    // read/clone/drop the live-match handle BEFORE locking the queue — nobody
                    // holds both at the same time.
                    let live_summary = live.snapshot();
                    let mut q = queue.lock().await;
                    q.tick(Instant::now());
                    if let Some(state) = q.slot_state_if_changed() {
                        tab_wire.slot_occupied.store(
                            !matches!(state, SlotState::Empty),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        emit_slot_state(&app, state);
                    }
                    let news_charge = {
                        let c = tab_wire
                            .news_charge
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        (c.fill(), c.count(), c.is_charged())
                    };
                    let status = StatusState::snapshot(
                        &q,
                        StatusInputs {
                            live: live_summary,
                            espn_enabled,
                            rss_enabled,
                            agent_sessions: tab_wire
                                .agent_sessions
                                .load(std::sync::atomic::Ordering::Relaxed),
                            news_charge,
                        },
                    );
                    let present = crate::tabs::present_tabs(&status);
                    {
                        let mut p = tab_wire
                            .tabs
                            .presence
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        *p = present.clone();
                    }
                    let selected_now = {
                        let mut sel = tab_wire
                            .tabs
                            .selection
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        sel.clear_if_gone(|t| present.contains(&t));
                        sel.selected()
                    };
                    crate::status::emit_tab_selection_if_transitioned(
                        &app,
                        &tab_wire.tabs.last_emitted,
                        selected_now,
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

    /// Non-emitting read of the current `SlotState` for the on_page_load boot-shield site
    /// (`lib.rs`).
    pub fn current_slot_state_blocking(&self) -> SlotState {
        let mut q = self.queue.blocking_lock();
        q.current_slot_state_for_emission(Instant::now())
    }

    #[cfg(test)]
    pub fn emit_current_blocking(&self) -> SlotState {
        let state = self.current_slot_state_blocking();
        emit_slot_state(&self.app, state.clone());
        state
    }

    pub fn status_snapshot_blocking(&self) -> StatusState {
        let live_summary = self.live.snapshot();
        let q = self.queue.blocking_lock();
        StatusState::snapshot(
            &q,
            StatusInputs {
                live: live_summary,
                espn_enabled: self.espn_enabled,
                rss_enabled: self.rss_enabled,
                agent_sessions: self
                    .tab_wire
                    .agent_sessions
                    .load(std::sync::atomic::Ordering::Relaxed),
                news_charge: {
                    let c = self
                        .tab_wire
                        .news_charge
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    (c.fill(), c.count(), c.is_charged())
                },
            },
        )
    }

    #[cfg(test)]
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
            true,
            true,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
        )
    }

    #[tokio::test]
    async fn apply_wakes_and_emits() {
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
    async fn accept_queue_full_propagates_nothing() {
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(1),
            app.handle().clone(),
            true,
            true,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
        );

        engine.accept(event(Priority::Medium), false).await.unwrap();
        engine.accept(event(Priority::Medium), false).await.unwrap();

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
    }

    #[tokio::test]
    async fn rotation_loop_parked_idle_wakes_on_accept() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.spawn_rotation();

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

        let mut long_lived = event(Priority::Medium);
        long_lived.rotation = RotationSpec::OneShot { ttl_secs: 30 };
        engine.accept(long_lived, false).await.unwrap();
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            1,
            "accept() itself must have emitted exactly once"
        );

        engine.spawn_rotation();

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
    fn current_slot_state_blocking_returns_without_emitting() {
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
        emit_count.store(0, Ordering::SeqCst);

        let state = engine.current_slot_state_blocking();
        assert!(matches!(state, SlotState::Showing { .. }));
        assert_eq!(
            emit_count.load(Ordering::SeqCst),
            0,
            "current_slot_state_blocking must not emit — the caller controls when/if it does"
        );

        let emitted = engine.emit_current_blocking();
        assert_eq!(emitted, state);
        assert_eq!(emit_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn emit_current_status_returns_and_emits() {
        let app = tauri::test::mock_app();
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            true,
            false,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
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
            true,
            true,
            Some(store.clone()),
            std::sync::Arc::new(crate::tabs::TabWire::default()),
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
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let store =
            Arc::new(crate::history::HistoryStore::with_limits(&dir, 5 * 1024 * 1024, 2).unwrap());
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            true,
            true,
            Some(store.clone()),
            std::sync::Arc::new(crate::tabs::TabWire::default()),
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

    #[tokio::test]
    async fn history_store_returns_a_clone_of_the_same_arc_the_accept_path_writes_through() {
        let app = tauri::test::mock_app();
        let dir = history_temp_dir();
        let store =
            Arc::new(crate::history::HistoryStore::with_limits(&dir, 5 * 1024 * 1024, 2).unwrap());
        let engine = Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            true,
            true,
            Some(store.clone()),
            std::sync::Arc::new(crate::tabs::TabWire::default()),
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

    #[tokio::test]
    async fn clear_queue_apply_emits_a_fresh_slot_state_for_the_progress_dots() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);

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
