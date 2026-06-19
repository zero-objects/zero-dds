// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Continuous-Read-Mode (Spec §8.4.14).
//!
//! `READ_DATA` submessages can carry a `DataDeliveryControl` that
//! turns the single-shot read into a continuous stream: the agent
//! delivers `DATA` submessages until one of the limits is reached:
//!
//! - `max_samples`: maximum total number of samples.
//! - `max_elapsed_time`: hard time upper bound.
//! - `max_bytes_per_second`: rate limit (token-bucket-like).
//!
//! This file models the reader-mode state, which is consumed by an
//! agent process (out of scope here).

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::time::Duration;

use crate::object_id::ObjectId;

/// Delivery control (Spec §7.7.13).
///
/// A value of `0` means "no limit" (spec); we map that to `u16::MAX`
/// for max_samples / `Duration::MAX` for max_elapsed_time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeliveryControl {
    /// Maximum number of samples (`0` = unlimited).
    pub max_samples: u16,
    /// Hard time upper bound.
    pub max_elapsed_time: Duration,
    /// Rate cap in bytes/s (`0` = unlimited).
    pub max_bytes_per_second: u32,
    /// Minimum pause between samples (ms). `0` = no pacing.
    pub min_pace_period: Duration,
}

impl Default for DeliveryControl {
    fn default() -> Self {
        Self {
            max_samples: 0,
            max_elapsed_time: Duration::MAX,
            max_bytes_per_second: 0,
            min_pace_period: Duration::ZERO,
        }
    }
}

impl DeliveryControl {
    /// Single-shot read: one sample, immediately.
    #[must_use]
    pub fn single_shot() -> Self {
        Self {
            max_samples: 1,
            max_elapsed_time: Duration::ZERO,
            max_bytes_per_second: 0,
            min_pace_period: Duration::ZERO,
        }
    }
}

/// A sample produced by the ReadStream (corresponds later to the
/// XCDR2 body of a DATA submessage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSample {
    /// Application payload (XCDR2-encoded).
    pub bytes: Vec<u8>,
}

/// Stream state held per `READ_DATA` request.
#[derive(Debug, Clone)]
pub struct ReadStream {
    /// Subscriber object the ReadStream is associated with.
    pub subscriber_handle: ObjectId,
    /// Topic object being read.
    pub topic_handle: ObjectId,
    /// Delivery control with all limits.
    pub delivery_control: DeliveryControl,

    /// Start time (uptime-relative).
    started_at: Duration,
    /// Last tick.
    last_tick: Duration,
    /// Samples delivered so far.
    samples_delivered: u32,
    /// Token bucket: bytes available at the tick.
    bytes_credit: u64,
    /// Waiting samples, fed in by the app layer.
    queue: VecDeque<PendingSample>,
    /// `true` when the stream is finalized and delivers nothing more.
    finalized: bool,
}

impl ReadStream {
    /// Constructor.
    #[must_use]
    pub fn new(
        subscriber_handle: ObjectId,
        topic_handle: ObjectId,
        delivery_control: DeliveryControl,
        now: Duration,
    ) -> Self {
        Self {
            subscriber_handle,
            topic_handle,
            delivery_control,
            started_at: now,
            last_tick: now,
            samples_delivered: 0,
            bytes_credit: 0,
            queue: VecDeque::new(),
            finalized: false,
        }
    }

    /// `true` when the stream is finished.
    #[must_use]
    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    /// Number of samples already delivered.
    #[must_use]
    pub fn samples_delivered(&self) -> u32 {
        self.samples_delivered
    }

    /// The app layer submits a new sample.
    pub fn push_sample(&mut self, sample: PendingSample) {
        if !self.finalized {
            self.queue.push_back(sample);
        }
    }

    /// Number of waiting samples (not yet delivered).
    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// Pull tick: returns the samples that may now be emitted in a
    /// rate-conformant way. `now` is uptime-relative.
    pub fn pull_pending_samples(&mut self, now: Duration) -> Vec<PendingSample> {
        if self.finalized {
            return Vec::new();
        }
        // Time cap: max_elapsed_time exceeded?
        let elapsed = now.saturating_sub(self.started_at);
        if elapsed >= self.delivery_control.max_elapsed_time
            && self.delivery_control.max_elapsed_time > Duration::ZERO
        {
            self.finalized = true;
            return Vec::new();
        }

        // Refill the token bucket.
        let dt = now.saturating_sub(self.last_tick);
        if self.delivery_control.max_bytes_per_second > 0 {
            let added = (u128::from(self.delivery_control.max_bytes_per_second)
                * u128::from(dt.as_millis() as u64)
                / 1000) as u64;
            self.bytes_credit = self.bytes_credit.saturating_add(added);
            // Cap to a 1s burst
            let burst_cap = u64::from(self.delivery_control.max_bytes_per_second);
            if self.bytes_credit > burst_cap {
                self.bytes_credit = burst_cap;
            }
        }

        // Pacing pause: not yet elapsed?
        if self.delivery_control.min_pace_period > Duration::ZERO
            && dt < self.delivery_control.min_pace_period
            && self.samples_delivered > 0
        {
            return Vec::new();
        }
        self.last_tick = now;

        let mut out = Vec::new();
        while let Some(front) = self.queue.front() {
            // max_samples cap?
            if self.delivery_control.max_samples > 0
                && self.samples_delivered >= u32::from(self.delivery_control.max_samples)
            {
                self.finalized = true;
                break;
            }
            // Rate cap: does the next sample fit?
            if self.delivery_control.max_bytes_per_second > 0 {
                let need = front.bytes.len() as u64;
                if self.bytes_credit < need {
                    break;
                }
                self.bytes_credit -= need;
            }
            let Some(sample) = self.queue.pop_front() else {
                break;
            };
            out.push(sample);
            self.samples_delivered = self.samples_delivered.saturating_add(1);

            // Single-shot is done after 1 sample.
            if self.delivery_control.max_samples == 1 {
                self.finalized = true;
                break;
            }
            // Pacing after each sample, if active.
            if self.delivery_control.min_pace_period > Duration::ZERO {
                break;
            }
        }
        // max_samples reached?
        if self.delivery_control.max_samples > 0
            && self.samples_delivered >= u32::from(self.delivery_control.max_samples)
        {
            self.finalized = true;
        }
        out
    }

    /// Stops the stream immediately (e.g. on DELETE of the subscriber).
    pub fn stop(&mut self) {
        self.finalized = true;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::object_kind::ObjectKind;

    fn s_id() -> ObjectId {
        ObjectId::new(0x10, ObjectKind::Subscriber).unwrap()
    }
    fn t_id() -> ObjectId {
        ObjectId::new(0x11, ObjectKind::Topic).unwrap()
    }

    #[test]
    fn single_shot_delivers_one_then_finalizes() {
        let mut rs = ReadStream::new(
            s_id(),
            t_id(),
            DeliveryControl::single_shot(),
            Duration::ZERO,
        );
        rs.push_sample(PendingSample {
            bytes: alloc::vec![1, 2],
        });
        rs.push_sample(PendingSample {
            bytes: alloc::vec![3, 4],
        });
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 1);
        assert!(rs.is_finalized());
        // After finalize, nothing more is delivered
        let out = rs.pull_pending_samples(Duration::from_millis(2));
        assert!(out.is_empty());
    }

    #[test]
    fn max_samples_cap_enforced() {
        let dc = DeliveryControl {
            max_samples: 3,
            ..Default::default()
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for i in 0..10 {
            rs.push_sample(PendingSample {
                bytes: alloc::vec![i as u8],
            });
        }
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 3);
        assert!(rs.is_finalized());
    }

    #[test]
    fn rate_limit_partitions_samples_over_time() {
        let dc = DeliveryControl {
            max_samples: 0,
            max_elapsed_time: Duration::MAX,
            max_bytes_per_second: 100, // 100 B/s
            min_pace_period: Duration::ZERO,
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for _ in 0..5 {
            rs.push_sample(PendingSample {
                bytes: alloc::vec![0u8; 50],
            });
        }
        // At 1s elapsed: 100 B budget, 50 B samples → 2 samples
        let out = rs.pull_pending_samples(Duration::from_secs(1));
        assert_eq!(out.len(), 2);
        // At 2s: another 100 B → 2 more (4 in total)
        let out = rs.pull_pending_samples(Duration::from_secs(2));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn max_elapsed_time_finalizes() {
        let dc = DeliveryControl {
            max_samples: 0,
            max_elapsed_time: Duration::from_secs(1),
            max_bytes_per_second: 0,
            min_pace_period: Duration::ZERO,
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        rs.push_sample(PendingSample {
            bytes: alloc::vec![1],
        });
        // 0.5s later → still ok
        let out = rs.pull_pending_samples(Duration::from_millis(500));
        assert_eq!(out.len(), 1);
        assert!(!rs.is_finalized());
        // 2s later → finalized, no sample anymore
        rs.push_sample(PendingSample {
            bytes: alloc::vec![2],
        });
        let out = rs.pull_pending_samples(Duration::from_secs(2));
        assert!(out.is_empty());
        assert!(rs.is_finalized());
    }

    #[test]
    fn stop_finalizes_immediately() {
        let mut rs = ReadStream::new(s_id(), t_id(), DeliveryControl::default(), Duration::ZERO);
        rs.push_sample(PendingSample {
            bytes: alloc::vec![1],
        });
        rs.stop();
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert!(out.is_empty());
        assert!(rs.is_finalized());
    }

    #[test]
    fn pacing_throttles_per_period() {
        let dc = DeliveryControl {
            max_samples: 0,
            max_elapsed_time: Duration::MAX,
            max_bytes_per_second: 0,
            min_pace_period: Duration::from_millis(100),
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for _ in 0..5 {
            rs.push_sample(PendingSample {
                bytes: alloc::vec![1],
            });
        }
        // First tick: delivers 1 sample
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 1);
        // 50ms later → no 100ms elapsed yet → no sample
        let out = rs.pull_pending_samples(Duration::from_millis(50));
        assert!(out.is_empty());
        // 200ms later → next sample
        let out = rs.pull_pending_samples(Duration::from_millis(200));
        assert_eq!(out.len(), 1);
    }
}
