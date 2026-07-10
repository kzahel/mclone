//! Target-neutral monotonic time used by scene/runtime orchestration.
//!
//! The clock source is supplied by the host. Native hosts use
//! [`system_monotonic_clock`]; browser hosts can project their cadence source
//! (for example `requestAnimationFrame`) into [`MonotonicInstant`] without
//! exposing browser APIs to shared scene code.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// An ordered point on a host-supplied monotonic timeline.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicInstant {
    nanos: u64,
}

impl MonotonicInstant {
    pub const ZERO: Self = Self { nanos: 0 };

    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    pub fn from_millis_f64(millis: f64) -> Self {
        if !millis.is_finite() || millis <= 0.0 {
            return Self::ZERO;
        }
        Self::from_nanos((millis * 1_000_000.0).min(u64::MAX as f64) as u64)
    }

    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_nanos(self.nanos.saturating_sub(earlier.nanos))
    }

    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let nanos = u64::try_from(duration.as_nanos()).ok()?;
        self.nanos.checked_add(nanos).map(Self::from_nanos)
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        Self::from_nanos(self.nanos.saturating_add(nanos))
    }
}

/// Source for a host's monotonic timeline.
pub trait MonotonicClock: Send + Sync {
    fn now(&self) -> MonotonicInstant;
}

/// Cloneable host clock capability retained by shared orchestration.
#[derive(Clone)]
pub struct MonotonicClockHandle {
    inner: Arc<dyn MonotonicClock>,
}

impl MonotonicClockHandle {
    pub fn new(clock: impl MonotonicClock + 'static) -> Self {
        Self {
            inner: Arc::new(clock),
        }
    }

    pub fn now(&self) -> MonotonicInstant {
        self.inner.now()
    }

    pub fn elapsed_since(&self, earlier: MonotonicInstant) -> Duration {
        self.now().saturating_duration_since(earlier)
    }

    pub fn deadline_after(&self, duration: Duration) -> MonotonicDeadline {
        MonotonicDeadline {
            clock: self.clone(),
            at: self.now().saturating_add(duration),
        }
    }
}

impl fmt::Debug for MonotonicClockHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MonotonicClockHandle")
            .finish_non_exhaustive()
    }
}

/// A deadline which retains the clock needed to evaluate it.
#[derive(Clone, Debug)]
pub struct MonotonicDeadline {
    clock: MonotonicClockHandle,
    at: MonotonicInstant,
}

impl MonotonicDeadline {
    pub fn at(clock: MonotonicClockHandle, at: MonotonicInstant) -> Self {
        Self { clock, at }
    }

    pub const fn instant(&self) -> MonotonicInstant {
        self.at
    }

    pub fn is_reached(&self) -> bool {
        self.clock.now() >= self.at
    }

    pub fn remaining(&self) -> Duration {
        self.at.saturating_duration_since(self.clock.now())
    }

    pub fn earlier(self, other: Self) -> Self {
        debug_assert!(
            Arc::ptr_eq(&self.clock.inner, &other.clock.inner),
            "cannot compare deadlines from different monotonic clocks"
        );
        if self.at <= other.at { self } else { other }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct SystemMonotonicClock {
    origin: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl MonotonicClock for SystemMonotonicClock {
    fn now(&self) -> MonotonicInstant {
        let nanos = u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX);
        MonotonicInstant::from_nanos(nanos)
    }
}

/// Native system monotonic clock adapter.
#[cfg(not(target_arch = "wasm32"))]
pub fn system_monotonic_clock() -> MonotonicClockHandle {
    MonotonicClockHandle::new(SystemMonotonicClock {
        origin: std::time::Instant::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Clone, Default)]
    struct ManualClock {
        nanos: Arc<AtomicU64>,
    }

    impl ManualClock {
        fn advance(&self, duration: Duration) {
            self.nanos.fetch_add(
                u64::try_from(duration.as_nanos()).unwrap(),
                Ordering::Relaxed,
            );
        }
    }

    impl MonotonicClock for ManualClock {
        fn now(&self) -> MonotonicInstant {
            MonotonicInstant::from_nanos(self.nanos.load(Ordering::Relaxed))
        }
    }

    #[test]
    fn instants_order_and_saturate_without_wall_clock_semantics() {
        let earlier = MonotonicInstant::from_nanos(4);
        let later = MonotonicInstant::from_nanos(9);
        assert!(earlier < later);
        assert_eq!(
            later.saturating_duration_since(earlier),
            Duration::from_nanos(5)
        );
        assert_eq!(earlier.saturating_duration_since(later), Duration::ZERO);
    }

    #[test]
    fn deadline_uses_the_supplied_clock() {
        let manual = ManualClock::default();
        let clock = MonotonicClockHandle::new(manual.clone());
        let deadline = clock.deadline_after(Duration::from_millis(5));
        assert!(!deadline.is_reached());
        assert_eq!(deadline.remaining(), Duration::from_millis(5));
        manual.advance(Duration::from_millis(5));
        assert!(deadline.is_reached());
        assert_eq!(deadline.remaining(), Duration::ZERO);
    }
}
