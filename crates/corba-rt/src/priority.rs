// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Priority` + `PriorityMapping` (RT-CORBA §5.3/§5.4).

/// CORBA priority (RT-CORBA §5.3): a platform-independent `short` in the range
/// `0..=32767` (negative values are reserved). Higher = more urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(i16);

/// Minimum valid CORBA priority.
pub const MIN_PRIORITY: i16 = 0;
/// Maximum valid CORBA priority.
pub const MAX_PRIORITY: i16 = 32767;

impl Priority {
    /// Constructs a priority; `None` if outside `0..=32767`.
    #[must_use]
    pub fn new(value: i16) -> Option<Self> {
        if (MIN_PRIORITY..=MAX_PRIORITY).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Constructs a priority, clamped to `0..=32767`.
    #[must_use]
    pub fn clamped(value: i32) -> Self {
        Self(value.clamp(MIN_PRIORITY as i32, MAX_PRIORITY as i32) as i16)
    }

    /// The raw `short` value.
    #[must_use]
    pub fn value(self) -> i16 {
        self.0
    }
}

/// Maps CORBA priorities to native (OS) priorities and back
/// (RT-CORBA §5.4 `PriorityMapping`).
pub trait PriorityMapping {
    /// CORBA priority → native priority. `None` if not mappable.
    fn to_native(&self, corba: Priority) -> Option<i32>;
    /// Native priority → CORBA priority. `None` if not mappable.
    fn to_corba(&self, native: i32) -> Option<Priority>;
}

/// Default linear mapping: `native = floor / divisor`-scaled onto a native
/// window `[native_min, native_max]` (e.g. POSIX `SCHED_FIFO` 1..99).
#[derive(Debug, Clone, Copy)]
pub struct LinearPriorityMapping {
    native_min: i32,
    native_max: i32,
}

impl LinearPriorityMapping {
    /// New linear mapping onto `[native_min, native_max]` (e.g. `(1, 99)`).
    #[must_use]
    pub fn new(native_min: i32, native_max: i32) -> Self {
        Self {
            native_min,
            native_max,
        }
    }
}

impl PriorityMapping for LinearPriorityMapping {
    fn to_native(&self, corba: Priority) -> Option<i32> {
        let span = i64::from(self.native_max - self.native_min);
        let scaled = i64::from(corba.value()) * span / i64::from(MAX_PRIORITY);
        Some(self.native_min + scaled as i32)
    }

    fn to_corba(&self, native: i32) -> Option<Priority> {
        if native < self.native_min || native > self.native_max {
            return None;
        }
        let span = i64::from(self.native_max - self.native_min);
        if span == 0 {
            return Priority::new(0);
        }
        let scaled = i64::from(native - self.native_min) * i64::from(MAX_PRIORITY) / span;
        Priority::new(scaled as i16)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn priority_range() {
        assert!(Priority::new(0).is_some());
        assert!(Priority::new(32767).is_some());
        assert!(Priority::new(-1).is_none());
        assert_eq!(Priority::clamped(99999).value(), 32767);
        assert_eq!(Priority::clamped(-5).value(), 0);
    }

    #[test]
    fn linear_mapping_endpoints() {
        let m = LinearPriorityMapping::new(1, 99);
        assert_eq!(m.to_native(Priority::new(0).unwrap()), Some(1));
        assert_eq!(m.to_native(Priority::new(32767).unwrap()), Some(99)); // native_min + span = 1 + 98
        // Middle ~50.
        let mid = m.to_native(Priority::new(16383).unwrap()).unwrap();
        assert!((49..=51).contains(&mid), "mid={mid}");
    }

    #[test]
    fn native_out_of_window_unmapped() {
        let m = LinearPriorityMapping::new(1, 99);
        assert!(m.to_corba(0).is_none());
        assert!(m.to_corba(100).is_none());
        assert!(m.to_corba(50).is_some());
    }
}
