//! Monotonic instrumentation clock. Browser hosts supply nanoseconds; this
//! clock measures work and never supplies an accounting date or a figure.
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn ratio_monotonic_nanos() -> u64;
}

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub fn now() -> Self {
        Self(unsafe { ratio_monotonic_nanos() })
    }
    pub fn elapsed(self) -> std::time::Duration {
        std::time::Duration::from_nanos(Self::now().0.saturating_sub(self.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::Add<std::time::Duration> for Instant {
    type Output = Self;
    fn add(self, duration: std::time::Duration) -> Self {
        Self(
            self.0
                .saturating_add(u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)),
        )
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::Sub<std::time::Duration> for Instant {
    type Output = Self;
    fn sub(self, duration: std::time::Duration) -> Self {
        Self(
            self.0
                .saturating_sub(u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)),
        )
    }
}
