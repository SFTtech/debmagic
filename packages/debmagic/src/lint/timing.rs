//! Wall time for data initialized once and shared by every Rule.
//!
//! File trees, Debian control, Debian changelog, and tarball indexes are
//! computed on first use. That work is not part of the Rule that happens to
//! trigger it. [`init_once`] records it separately so a timings report can
//! show shared initialization apart from each Rule.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static STATE: RefCell<Option<Arc<TimingState>>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static WAITED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

struct TimingState {
    shared_nanos: AtomicU64,
}

/// Installed on the lint pool while Rules run, so shared initialization is
/// timed on every worker. Dropping it clears that state.
#[must_use = "dropping the session ends shared-initialization timing"]
pub(crate) struct TimingSession {
    state: Arc<TimingState>,
    pooled: bool,
}

impl TimingSession {
    /// Record shared initialization on every worker in the current rayon pool.
    ///
    /// Call this from inside that pool. The session's drop broadcasts the
    /// clear, so it must be dropped on a pool thread too.
    pub(crate) fn install_on_current_pool() -> Self {
        let session = Self::new(true);
        let state = Arc::clone(&session.state);
        rayon::broadcast(move |_| install_state(Arc::clone(&state)));
        session
    }

    #[cfg(test)]
    fn install_here() -> Self {
        let session = Self::new(false);
        session.install_on_this_thread();
        session
    }

    fn new(pooled: bool) -> Self {
        Self {
            state: Arc::new(TimingState {
                shared_nanos: AtomicU64::new(0),
            }),
            pooled,
        }
    }

    #[cfg(test)]
    fn install_on_this_thread(&self) {
        install_state(Arc::clone(&self.state));
    }

    /// Sum of outermost initializer spans. A thread that only waited is not
    /// included, and a nested initializer is included in its parent span.
    pub(crate) fn shared(&self) -> Duration {
        Duration::from_nanos(self.state.shared_nanos.load(Ordering::Acquire))
    }
}

impl Drop for TimingSession {
    fn drop(&mut self) {
        if self.pooled {
            rayon::broadcast(|_| clear_thread());
        } else {
            clear_thread();
        }
    }
}

fn install_state(state: Arc<TimingState>) {
    STATE.with(|slot| *slot.borrow_mut() = Some(state));
    ENABLED.with(|cell| cell.set(true));
    DEPTH.with(|cell| cell.set(0));
    WAITED.with(|cell| cell.set(Duration::ZERO));
}

fn clear_thread() {
    ENABLED.with(|cell| cell.set(false));
    STATE.with(|slot| *slot.borrow_mut() = None);
    DEPTH.with(|cell| cell.set(0));
    WAITED.with(|cell| cell.set(Duration::ZERO));
}

fn current_state() -> Option<Arc<TimingState>> {
    STATE.with(|slot| slot.borrow().clone())
}

/// Time this thread has spent inside outermost [`init_once`] calls since the
/// session was installed, including waits for another thread's initializer.
pub(crate) fn waited_on_this_thread() -> Duration {
    WAITED.with(|slot| slot.get())
}

struct ResetDepth(u32);

impl Drop for ResetDepth {
    fn drop(&mut self) {
        DEPTH.with(|cell| cell.set(self.0));
    }
}

/// `OnceLock::get_or_init`, timed as shared initialization when a
/// [`TimingSession`] is installed on this thread.
///
/// Nested calls count toward the outermost span only, so a File tree that
/// indexes a tarball is not added twice. The initializer's span is recorded
/// once; every caller, including those that waited, can subtract that wait
/// from its own Rule time.
pub(crate) fn init_once<T>(cell: &OnceLock<T>, init: impl FnOnce() -> T) -> &T {
    // The default lint path pays one thread-local read. The session state is
    // only consulted when a timings report is actually being collected.
    if !ENABLED.with(Cell::get) {
        return cell.get_or_init(init);
    }
    let Some(state) = current_state() else {
        return cell.get_or_init(init);
    };
    if let Some(value) = cell.get() {
        return value;
    }

    let start = Instant::now();
    let previous_depth = DEPTH.with(|cell| {
        let depth = cell.get();
        cell.set(depth + 1);
        depth
    });
    let _reset = ResetDepth(previous_depth);
    let mut ran_init = false;
    let value = cell.get_or_init(|| {
        ran_init = true;
        init()
    });
    if previous_depth == 0 {
        let elapsed = start.elapsed();
        WAITED.with(|slot| slot.set(slot.get().saturating_add(elapsed)));
        if ran_init {
            let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
            state.shared_nanos.fetch_add(nanos, Ordering::AcqRel);
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;
    use std::thread;
    use std::time::{Duration, Instant};

    use rayon::prelude::*;

    use super::*;

    #[test]
    fn init_once_without_a_session_initializes_once() {
        let cell = OnceLock::new();
        let mut calls = 0;
        assert_eq!(
            *init_once(&cell, || {
                calls += 1;
                7
            }),
            7
        );
        assert_eq!(
            *init_once(&cell, || {
                calls += 1;
                9
            }),
            7
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn nested_shared_init_is_counted_once() {
        let session = TimingSession::install_here();
        let outer = OnceLock::new();
        let inner = OnceLock::new();
        let wall_start = Instant::now();
        init_once(&outer, || {
            init_once(&inner, || {
                thread::sleep(Duration::from_millis(20));
            });
            thread::sleep(Duration::from_millis(20));
        });
        let wall = wall_start.elapsed();
        let shared = session.shared();
        assert_eq!(shared, waited_on_this_thread());
        assert!(
            shared <= wall,
            "nested init counted twice: shared {shared:?} wall {wall:?}"
        );
        assert!(
            wall.saturating_sub(shared) < Duration::from_millis(15),
            "shared {shared:?} dropped the initializer wall {wall:?}"
        );

        let again = session.shared();
        let _ = init_once(&outer, || panic!("already initialized"));
        let _ = init_once(&inner, || panic!("already initialized"));
        assert_eq!(session.shared(), again);
    }

    #[test]
    fn waiting_caller_does_not_add_shared_time() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("pool");
        pool.install(|| {
            let session = TimingSession::install_on_current_pool();
            let cell = OnceLock::new();
            let waits: Vec<Duration> = (0..2)
                .into_par_iter()
                .map(|_| {
                    let before = waited_on_this_thread();
                    init_once(&cell, || {
                        thread::sleep(Duration::from_millis(40));
                    });
                    waited_on_this_thread().saturating_sub(before)
                })
                .collect();
            let shared = session.shared();
            assert!(shared >= Duration::from_millis(30), "{shared:?}");
            if waits.iter().all(|wait| *wait >= Duration::from_millis(20)) {
                let sum: Duration = waits.iter().copied().sum();
                assert!(
                    shared + Duration::from_millis(5) < sum,
                    "shared {shared:?} includes the waiter; waits {waits:?}"
                );
            }
        });
    }
}
