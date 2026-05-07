// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Continuous-Read-Mode (Spec §8.4.14).
//!
//! `READ_DATA`-Submessages koennen einen `DataDeliveryControl` tragen, der
//! aus dem Single-Shot-Read einen kontinuierlichen Stream macht: der Agent
//! liefert solange `DATA`-Submessages, bis eines der Limits erreicht ist:
//!
//! - `max_samples`: maximale Anzahl Samples insgesamt.
//! - `max_elapsed_time`: harte Zeit-Obergrenze.
//! - `max_bytes_per_second`: Rate-Limit (Token-Bucket-aehnlich).
//!
//! Diese Datei modelliert den Reader-Mode-State, der von einem
//! Agent-Prozess (out-of-scope hier) konsumiert wird.

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::time::Duration;

use crate::object_id::ObjectId;

/// Delivery-Control (Spec §7.7.13).
///
/// `0` als Wert bedeutet "no limit" (Spec); wir mappen das auf `u16::MAX`
/// fuer max_samples / `Duration::MAX` fuer max_elapsed_time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeliveryControl {
    /// Maximale Anzahl Samples (`0` = unlimited).
    pub max_samples: u16,
    /// Harte Zeit-Obergrenze.
    pub max_elapsed_time: Duration,
    /// Rate-Cap in Bytes/s (`0` = unlimited).
    pub max_bytes_per_second: u32,
    /// Mindest-Pause zwischen Samples (ms). `0` = kein Pacing.
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
    /// Single-Shot Read: ein Sample, sofort.
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

/// Ein vom ReadStream produziertes Sample (entspricht spaeter dem
/// XCDR2-Body einer DATA-Submessage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSample {
    /// Application-Payload (XCDR2-encoded).
    pub bytes: Vec<u8>,
}

/// Pro-`READ_DATA`-Request gehaltener Stream-State.
#[derive(Debug, Clone)]
pub struct ReadStream {
    /// Subscriber-Object, dem der ReadStream zugeordnet ist.
    pub subscriber_handle: ObjectId,
    /// Topic-Object, das gelesen wird.
    pub topic_handle: ObjectId,
    /// Delivery-Control mit allen Limits.
    pub delivery_control: DeliveryControl,

    /// Start-Zeitpunkt (uptime-relativ).
    started_at: Duration,
    /// Letzter Tick.
    last_tick: Duration,
    /// Bisher gelieferte Samples.
    samples_delivered: u32,
    /// Token-Bucket: zum Tick verfuegbare Bytes.
    bytes_credit: u64,
    /// Wartende Samples, von der App-Schicht eingespielt.
    queue: VecDeque<PendingSample>,
    /// `true`, wenn der Stream finalized ist und nichts mehr liefert.
    finalized: bool,
}

impl ReadStream {
    /// Konstruktor.
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

    /// `true`, wenn der Stream abgeschlossen ist.
    #[must_use]
    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    /// Anzahl bereits gelieferter Samples.
    #[must_use]
    pub fn samples_delivered(&self) -> u32 {
        self.samples_delivered
    }

    /// App-Schicht reicht ein neues Sample ein.
    pub fn push_sample(&mut self, sample: PendingSample) {
        if !self.finalized {
            self.queue.push_back(sample);
        }
    }

    /// Anzahl wartender Samples (noch nicht ausgeliefert).
    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// Pull-Tick: liefert die Samples, die jetzt rate-konform ausgegeben
    /// werden duerfen. `now` ist uptime-relativ.
    pub fn pull_pending_samples(&mut self, now: Duration) -> Vec<PendingSample> {
        if self.finalized {
            return Vec::new();
        }
        // Time-Cap: max_elapsed_time ueberschritten?
        let elapsed = now.saturating_sub(self.started_at);
        if elapsed >= self.delivery_control.max_elapsed_time
            && self.delivery_control.max_elapsed_time > Duration::ZERO
        {
            self.finalized = true;
            return Vec::new();
        }

        // Token-Bucket nachfuellen.
        let dt = now.saturating_sub(self.last_tick);
        if self.delivery_control.max_bytes_per_second > 0 {
            let added = (u128::from(self.delivery_control.max_bytes_per_second)
                * u128::from(dt.as_millis() as u64)
                / 1000) as u64;
            self.bytes_credit = self.bytes_credit.saturating_add(added);
            // Cap auf 1s-Burst
            let burst_cap = u64::from(self.delivery_control.max_bytes_per_second);
            if self.bytes_credit > burst_cap {
                self.bytes_credit = burst_cap;
            }
        }

        // Pacing-Pause: noch nicht abgelaufen?
        if self.delivery_control.min_pace_period > Duration::ZERO
            && dt < self.delivery_control.min_pace_period
            && self.samples_delivered > 0
        {
            return Vec::new();
        }
        self.last_tick = now;

        let mut out = Vec::new();
        while let Some(front) = self.queue.front() {
            // max_samples-Cap?
            if self.delivery_control.max_samples > 0
                && self.samples_delivered >= u32::from(self.delivery_control.max_samples)
            {
                self.finalized = true;
                break;
            }
            // Rate-Cap: passt das naechste Sample?
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

            // Single-Shot ist nach 1 Sample fertig.
            if self.delivery_control.max_samples == 1 {
                self.finalized = true;
                break;
            }
            // Pacing nach jedem Sample, falls aktiv.
            if self.delivery_control.min_pace_period > Duration::ZERO {
                break;
            }
        }
        // max_samples erreicht?
        if self.delivery_control.max_samples > 0
            && self.samples_delivered >= u32::from(self.delivery_control.max_samples)
        {
            self.finalized = true;
        }
        out
    }

    /// Stoppt den Stream sofort (z.B. bei DELETE des Subscribers).
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
        // Nach finalize liefert nichts mehr
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
        // Bei 1s vergangen: 100 B Budget, 50 B Samples → 2 Samples
        let out = rs.pull_pending_samples(Duration::from_secs(1));
        assert_eq!(out.len(), 2);
        // Bei 2s: weitere 100 B → 2 mehr (insgesamt 4)
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
        // 0.5s spaeter → noch ok
        let out = rs.pull_pending_samples(Duration::from_millis(500));
        assert_eq!(out.len(), 1);
        assert!(!rs.is_finalized());
        // 2s spaeter → finalized, kein Sample mehr
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
        // Erster Tick: liefert 1 Sample
        let out = rs.pull_pending_samples(Duration::from_millis(1));
        assert_eq!(out.len(), 1);
        // 50ms spaeter → noch keine 100ms vergangen → kein Sample
        let out = rs.pull_pending_samples(Duration::from_millis(50));
        assert!(out.is_empty());
        // 200ms spaeter → naechstes Sample
        let out = rs.pull_pending_samples(Duration::from_millis(200));
        assert_eq!(out.len(), 1);
    }
}
