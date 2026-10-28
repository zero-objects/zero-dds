// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Continuous-Read-Mode (Spec §8.4.14).
//!
//! `READ_DATA` submessages can carry a `DataDeliveryControl` that
//! turns the single-shot read into a continuous stream: the agent
//! delivers `DATA` submessages until one of the limits is reached:
//!
//! - `max_samples`: maximum total number of samples
//!   ([`MAX_SAMPLES_UNLIMITED`] for none, [`MAX_SAMPLES_ZERO`] cancels).
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

/// `MAX_SAMPLES_ZERO` (§7.8.5.1 Table 9, §8.3.5.9.2): "shall be used to
/// cancel the currently active read operation without sending any more
/// samples".
pub const MAX_SAMPLES_ZERO: u16 = 0;
/// `MAX_SAMPLES_UNLIMITED` (§7.8.5.1 Table 9, §8.3.5.9.2): no limit on the
/// number of samples.
pub const MAX_SAMPLES_UNLIMITED: u16 = 0xFFFF;

/// Delivery control (Spec §7.7.13, §8.3.5.9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeliveryControl {
    /// Maximum number of samples; [`MAX_SAMPLES_UNLIMITED`] for no limit,
    /// [`MAX_SAMPLES_ZERO`] ends the read before it delivers anything.
    pub max_samples: u16,
    /// Hard time upper bound; `Duration::MAX` for
    /// `MAX_ELAPSED_TIME_UNLIMITED`.
    pub max_elapsed_time: Duration,
    /// Rate cap in bytes/s (`0` = unlimited).
    pub max_bytes_per_second: u32,
    /// Minimum pause between samples (ms). `0` = no pacing.
    pub min_pace_period: Duration,
}

impl Default for DeliveryControl {
    fn default() -> Self {
        Self {
            max_samples: MAX_SAMPLES_UNLIMITED,
            max_elapsed_time: Duration::MAX,
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
    /// The `SampleInfoFlags` of the sample (instance, view and sample state,
    /// §7.7.1, Annex A); `0` for an alive sample that is read for the first
    /// time of a known instance.
    pub state: u8,
    /// The source timestamp of the sample (seconds since the Unix epoch and
    /// nanoseconds), when the DataReader gave one; `None` leaves the offset to
    /// the time of delivery.
    pub source_time: Option<(i32, u32)>,
}

impl PendingSample {
    /// A sample with only its payload: alive, no source timestamp.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            state: 0,
            source_time: None,
        }
    }
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
    /// A read without `delivery_control`: one read call.
    once: bool,
}

impl ReadStream {
    /// A read with `delivery_control`. With `max_samples`
    /// [`MAX_SAMPLES_ZERO`] it is finalized from the start.
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
            finalized: delivery_control.max_samples == MAX_SAMPLES_ZERO,
            once: false,
        }
    }

    /// A read without `delivery_control` (§8.3.5.9.2): "the Agent shall call
    /// the read operation just once. As a result the only data returned will
    /// be the one already in the DDS DataReader cache." The first
    /// [`Self::pull_pending_samples`] returns every sample pushed before it,
    /// and the stream is finalized.
    #[must_use]
    pub fn once(subscriber_handle: ObjectId, topic_handle: ObjectId, now: Duration) -> Self {
        Self {
            once: true,
            ..Self::new(
                subscriber_handle,
                topic_handle,
                DeliveryControl::default(),
                now,
            )
        }
    }

    /// `true` for a read without `delivery_control`.
    #[must_use]
    pub fn is_once(&self) -> bool {
        self.once
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

    /// How many more samples the stream takes at `now`, so that the caller
    /// takes from the DataReader no sample the stream will not deliver: the
    /// rest of `max_samples` minus what waits; one at a time, when nothing
    /// waits, under a pace period or a rate cap (the size of the next sample
    /// is not known before it is taken); every sample for a read without
    /// `delivery_control`.
    #[must_use]
    pub fn capacity(&self, now: Duration) -> usize {
        if self.finalized {
            return 0;
        }
        if self.once {
            return usize::MAX;
        }
        let queued = self.queue.len();
        let dc = &self.delivery_control;
        let mut cap = if dc.max_samples == MAX_SAMPLES_UNLIMITED {
            usize::MAX
        } else {
            (usize::from(dc.max_samples))
                .saturating_sub(self.samples_delivered as usize)
                .saturating_sub(queued)
        };
        if dc.min_pace_period > Duration::ZERO {
            let paced = self.samples_delivered > 0
                && now.saturating_sub(self.last_tick) < dc.min_pace_period;
            cap = if paced {
                0
            } else {
                cap.min(1usize.saturating_sub(queued))
            };
        } else if dc.max_bytes_per_second > 0 {
            cap = cap.min(1usize.saturating_sub(queued));
        }
        cap
    }

    /// Samples taken for the stream and not delivered, once it is finalized
    /// (by `max_elapsed_time` while a sample waited for the rate or the pace).
    #[must_use]
    pub fn undelivered(&self) -> usize {
        if self.finalized { self.queue.len() } else { 0 }
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
        if self.once {
            self.finalized = true;
            let out: Vec<PendingSample> = self.queue.drain(..).collect();
            self.samples_delivered = self
                .samples_delivered
                .saturating_add(u32::try_from(out.len()).unwrap_or(u32::MAX));
            return out;
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

        let limited = self.delivery_control.max_samples != MAX_SAMPLES_UNLIMITED;
        let limit = u32::from(self.delivery_control.max_samples);
        let mut out = Vec::new();
        while let Some(front) = self.queue.front() {
            if limited && self.samples_delivered >= limit {
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
            // Pacing after each sample, if active.
            if self.delivery_control.min_pace_period > Duration::ZERO {
                break;
            }
        }
        if limited && self.samples_delivered >= limit {
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

    /// §8.3.5.9.2: without `delivery_control` the read is called once and
    /// returns what is in the cache; a sample after it is not delivered.
    /// It returned the first of two and waited for later ones.
    #[test]
    fn a_read_without_delivery_control_returns_the_cache_once() {
        let mut rs = ReadStream::once(s_id(), t_id(), Duration::ZERO);
        assert_eq!(rs.capacity(Duration::ZERO), usize::MAX);
        rs.push_sample(PendingSample::new(alloc::vec![1, 2]));
        rs.push_sample(PendingSample::new(alloc::vec![3, 4]));
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 2);
        assert!(rs.is_finalized());
        assert_eq!(rs.capacity(Duration::from_millis(2)), 0);
        rs.push_sample(PendingSample::new(alloc::vec![5]));
        assert!(rs.pull_pending_samples(Duration::from_millis(2)).is_empty());
    }

    /// §7.8.5.1 Table 9: `MAX_SAMPLES_ZERO` delivers nothing,
    /// `MAX_SAMPLES_UNLIMITED` has no limit. 0 was taken as no limit and
    /// 0xFFFF as 65 535.
    #[test]
    fn max_samples_zero_ends_the_read_and_unlimited_has_no_limit() {
        let zero = DeliveryControl {
            max_samples: MAX_SAMPLES_ZERO,
            ..Default::default()
        };
        let mut rs = ReadStream::new(s_id(), t_id(), zero, Duration::ZERO);
        assert!(rs.is_finalized());
        assert_eq!(rs.capacity(Duration::ZERO), 0);
        rs.push_sample(PendingSample::new(alloc::vec![1]));
        assert!(rs.pull_pending_samples(Duration::from_millis(1)).is_empty());

        let mut rs = ReadStream::new(s_id(), t_id(), DeliveryControl::default(), Duration::ZERO);
        assert_eq!(rs.delivery_control.max_samples, MAX_SAMPLES_UNLIMITED);
        for i in 0..70_000u32 {
            rs.push_sample(PendingSample::new(alloc::vec![i as u8]));
        }
        assert_eq!(
            rs.pull_pending_samples(Duration::from_millis(1)).len(),
            70_000
        );
        assert!(!rs.is_finalized());
    }

    /// The caller takes no more from the DataReader than the stream
    /// delivers: the rest of `max_samples`, one at a time under a rate cap.
    #[test]
    fn the_capacity_is_what_the_stream_still_delivers() {
        let dc = DeliveryControl {
            max_samples: 3,
            ..Default::default()
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        assert_eq!(rs.capacity(Duration::ZERO), 3);
        rs.push_sample(PendingSample::new(alloc::vec![1]));
        assert_eq!(rs.capacity(Duration::ZERO), 2);
        assert_eq!(rs.pull_pending_samples(Duration::ZERO).len(), 1);
        assert_eq!(rs.capacity(Duration::ZERO), 2);

        let rated = DeliveryControl {
            max_bytes_per_second: 100,
            ..Default::default()
        };
        let mut rs = ReadStream::new(s_id(), t_id(), rated, Duration::ZERO);
        assert_eq!(rs.capacity(Duration::ZERO), 1);
        rs.push_sample(PendingSample::new(alloc::vec![0; 50]));
        assert_eq!(rs.capacity(Duration::ZERO), 0);
    }

    #[test]
    fn max_samples_cap_enforced() {
        let dc = DeliveryControl {
            max_samples: 3,
            ..Default::default()
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for i in 0..10 {
            rs.push_sample(PendingSample::new(alloc::vec![i as u8]));
        }
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 3);
        assert!(rs.is_finalized());
    }

    #[test]
    fn rate_limit_partitions_samples_over_time() {
        let dc = DeliveryControl {
            max_samples: MAX_SAMPLES_UNLIMITED,
            max_elapsed_time: Duration::MAX,
            max_bytes_per_second: 100, // 100 B/s
            min_pace_period: Duration::ZERO,
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for _ in 0..5 {
            rs.push_sample(PendingSample::new(alloc::vec![0u8; 50]));
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
            max_samples: MAX_SAMPLES_UNLIMITED,
            max_elapsed_time: Duration::from_secs(1),
            max_bytes_per_second: 0,
            min_pace_period: Duration::ZERO,
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        rs.push_sample(PendingSample::new(alloc::vec![1]));
        // 0.5s later → still ok
        let out = rs.pull_pending_samples(Duration::from_millis(500));
        assert_eq!(out.len(), 1);
        assert!(!rs.is_finalized());
        // 2s later → finalized, no sample anymore
        rs.push_sample(PendingSample::new(alloc::vec![2]));
        let out = rs.pull_pending_samples(Duration::from_secs(2));
        assert!(out.is_empty());
        assert!(rs.is_finalized());
    }

    #[test]
    fn stop_finalizes_immediately() {
        let mut rs = ReadStream::new(s_id(), t_id(), DeliveryControl::default(), Duration::ZERO);
        rs.push_sample(PendingSample::new(alloc::vec![1]));
        rs.stop();
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert!(out.is_empty());
        assert!(rs.is_finalized());
    }

    #[test]
    fn pacing_throttles_per_period() {
        let dc = DeliveryControl {
            max_samples: MAX_SAMPLES_UNLIMITED,
            max_elapsed_time: Duration::MAX,
            max_bytes_per_second: 0,
            min_pace_period: Duration::from_millis(100),
        };
        let mut rs = ReadStream::new(s_id(), t_id(), dc, Duration::ZERO);
        for _ in 0..5 {
            rs.push_sample(PendingSample::new(alloc::vec![1]));
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
