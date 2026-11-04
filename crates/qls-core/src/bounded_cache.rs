//! A cache for values that are cheap to return and expensive to refresh
//! (GPU inventory, engine device lists, version strings).
//!
//! The property that matters for the tokio runtime: **readers never block
//! behind a slow refresh.** The old shape (one mutex over the whole cache,
//! refresh held while a subprocess ran) froze every other caller for the
//! duration of the query — measured on 2026-11-02: an `nvidia-smi` stall
//! during a GPU changeover pinned all four runtime worker threads, the
//! service went silent for eight minutes, and no request-timeout fired
//! (timers are futures, they cannot run on a blocked thread). Here the
//! last known value sits behind its own mutex that readers take alone; the
//! refresh lock is held only while the query itself runs, and each refresher
//! re-checks freshness after acquiring it, so concurrent refreshers collapse
//! into one query.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub struct BoundedCache<T: Clone> {
    last: Mutex<T>,
    stamp: Mutex<Option<Instant>>,
    refresh_lock: Mutex<()>,
}

impl<T: Clone> BoundedCache<T> {
    /// `init` is the value served until the first refresh completes.
    pub fn new(init: T) -> Self {
        Self {
            last: Mutex::new(init),
            stamp: Mutex::new(None),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Returns a clone of the last known value without waiting on a refresh
    /// in flight. Never blocks beyond a plain mutex hand-off.
    pub fn get(&self) -> T {
        self.lock_last().clone()
    }

    /// Serves the cached value when `is_fresh(&last)` is true; otherwise
    /// (including the first time) runs `fresh` to produce a new value, stores
    /// it, and returns it. Concurrent callers serialize on the refresh lock
    /// and each re-checks `is_fresh` after acquiring it, so only the first of
    /// a burst queries.
    pub fn refresh_if_stale(
        &self,
        is_fresh: impl Fn(&T) -> bool,
        fresh: impl FnOnce() -> T,
    ) -> T {
        let _held = self.lock_refresh();
        if is_fresh(&self.lock_last()) {
            return self.get();
        }
        let v = fresh();
        *self.lock_stamp() = Some(Instant::now());
        *self.lock_last() = v.clone();
        v
    }

    /// How long ago the last refresh completed, if one has.
    pub fn age(&self) -> Option<Duration> {
        match *self.lock_stamp() {
            Some(t) => Some(t.elapsed()),
            None => None,
        }
    }

    /// Overwrites the value and stamps it as fresh now.
    pub fn reset(&self, v: T) {
        *self.lock_stamp() = Some(Instant::now());
        *self.lock_last() = v;
    }

    fn lock_last(&self) -> MutexGuard<'_, T> {
        self.last.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_stamp(&self) -> MutexGuard<'_, Option<Instant>> {
        self.stamp.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_refresh(&self) -> MutexGuard<'_, ()> {
        self.refresh_lock.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn new_cache_queries_immediately() {
        let cache = BoundedCache::new(0i64);
        let calls = AtomicUsize::new(0);
        let v = cache.refresh_if_stale(|t: &i64| *t != 0, || {
            calls.fetch_add(1, Ordering::SeqCst);
            42
        });
        assert_eq!(v, 42);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(cache.age().is_some());
    }

    #[test]
    fn fresh_value_is_not_recomputed() {
        let cache = BoundedCache::new(42i64);
        let calls = AtomicUsize::new(0);
        let v = cache.refresh_if_stale(|t: &i64| *t != 0, || {
            calls.fetch_add(1, Ordering::SeqCst);
            99
        });
        assert_eq!(v, 42, "a fresh value is served from the cache");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "no query while fresh");
    }

    #[test]
    fn stale_value_is_refreshed_once_per_window() {
        let cache = BoundedCache::new(0i64);
        let calls = Arc::new(AtomicUsize::new(0));
        // only the sentinel value 99 is "fresh": the first call queries for it,
        // the second must be served from the cache without querying
        let c = calls.clone();
        let v = cache.refresh_if_stale(|t: &i64| *t == 99, || {
            c.fetch_add(1, Ordering::SeqCst);
            99
        });
        assert_eq!(v, 99);
        let c = calls.clone();
        let v = cache.refresh_if_stale(|t: &i64| *t == 99, || {
            c.fetch_add(1, Ordering::SeqCst);
            100
        });
        assert_eq!(v, 99, "fresh: the old value is served, not the query's result");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "exactly one query across both calls");
    }

    #[test]
    fn reader_never_waits_on_a_slow_refresh() {
        let cache = Arc::new(BoundedCache::new(String::from("old")));
        let slow = cache.clone();
        let handle = std::thread::spawn(move || {
            slow.refresh_if_stale(|t: &String| t.is_empty(), || {
                std::thread::sleep(Duration::from_millis(1500));
                String::from("new")
            })
        });
        // Let the refresher take its lock and start the slow query, then read.
        std::thread::sleep(Duration::from_millis(200));
        let t0 = Instant::now();
        let v = cache.get();
        assert!(
            t0.elapsed() < Duration::from_millis(400),
            "get() blocked on the in-flight refresh: {:?}",
            t0.elapsed()
        );
        assert_eq!(v, "old", "get() returns the last known value, not a partial refresh");
        assert_eq!(handle.join().unwrap(), "new");
        assert_eq!(cache.get(), "new");
    }

    #[test]
    fn concurrent_refreshers_run_the_query_once() {
        let cache = Arc::new(BoundedCache::new(0i64));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = cache.clone();
            let calls = calls.clone();
            handles.push(std::thread::spawn(move || {
                // everyone after the winner sees 42 (fresh) and skips the query
                c.refresh_if_stale(|t: &i64| *t != 0, || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(300));
                    42
                })
            }));
        }
        for h in handles {
            assert_eq!(h.join().unwrap(), 42);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "only the refresher that wins the lock queries; the rest re-check and skip"
        );
        assert_eq!(cache.get(), 42);
    }

    #[test]
    fn reset_overwrites_and_restarts_the_window() {
        let cache = BoundedCache::new(0i64);
        cache.reset(7);
        assert_eq!(cache.get(), 7);
        let age = cache.age().expect("reset stamps now");
        assert!(age.as_millis() < 1000);
    }
}
