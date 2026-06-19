// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Priority-Model + Threadpools/Lanes + Priority-Banded-Connections
//! (RT-CORBA §5.4.1, §5.7, §5.8).

use alloc::vec::Vec;

use crate::priority::Priority;

/// Priority-Model (RT-CORBA §5.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityModel {
    /// `SERVER_DECLARED` — the server determines the execution priority.
    ServerDeclared,
    /// `CLIENT_PROPAGATED` — the client priority travels along and applies at the server.
    ClientPropagated,
}

/// `PriorityModelPolicy` (RT-CORBA §5.4.1): model + (with `SERVER_DECLARED`) the
/// server-declared default priority. Advertised in the IOR as the TaggedComponent
/// `TAG_RT_CORBA_PRIORITY_MODEL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityModelPolicy {
    /// The priority model.
    pub model: PriorityModel,
    /// Server-declared priority (relevant with `SERVER_DECLARED`).
    pub server_priority: Priority,
}

impl PriorityModelPolicy {
    /// The priority a request actually gets at the server: the
    /// propagated client priority with `CLIENT_PROPAGATED`, otherwise the
    /// server-declared one.
    #[must_use]
    pub fn effective_priority(&self, propagated: Option<Priority>) -> Priority {
        match self.model {
            PriorityModel::ClientPropagated => propagated.unwrap_or(self.server_priority),
            PriorityModel::ServerDeclared => self.server_priority,
        }
    }
}

/// A threadpool **lane** (RT-CORBA §5.7): serves requests of one priority with
/// a fixed plus dynamically growing number of threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    /// Priority that this lane serves.
    pub priority: Priority,
    /// Statically maintained threads.
    pub static_threads: u32,
    /// Maximum number of additional dynamically creatable threads.
    pub dynamic_threads: u32,
}

/// A **threadpool** with lanes (RT-CORBA §5.7). Models structure + selection;
/// the actual thread spawning is the responsibility of the runtime integration.
#[derive(Debug, Clone)]
pub struct Threadpool {
    /// Lanes (one per served priority).
    pub lanes: Vec<Lane>,
    /// Stack size per thread in bytes (0 = platform default).
    pub stacksize: usize,
    /// Whether requests are buffered when a lane is full (instead of rejected).
    pub allow_request_buffering: bool,
    /// Max buffered requests (0 = unbounded when buffering is on).
    pub max_buffered_requests: u32,
}

impl Threadpool {
    /// Selects the lane for a request priority: the lane with the **highest
    /// priority ≤ `priority`** (the most demanding one that still covers the
    /// request). Returns `None` if no lane fits.
    #[must_use]
    pub fn lane_for(&self, priority: Priority) -> Option<&Lane> {
        self.lanes
            .iter()
            .filter(|l| l.priority <= priority)
            .max_by_key(|l| l.priority)
            .or_else(|| self.lanes.iter().min_by_key(|l| l.priority))
    }

    /// Total static thread capacity across all lanes.
    #[must_use]
    pub fn static_capacity(&self) -> u32 {
        self.lanes.iter().map(|l| l.static_threads).sum()
    }
}

/// `ThreadpoolPolicy` (RT-CORBA §5.7) — binds a threadpool to a POA.
#[derive(Debug, Clone)]
pub struct ThreadpoolPolicy {
    /// The associated threadpool.
    pub pool: Threadpool,
}

/// A priority band (RT-CORBA §5.8): an interval `[low, high]` to which a
/// dedicated connection is assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityBand {
    /// Lower band bound (inclusive).
    pub low: Priority,
    /// Upper band bound (inclusive).
    pub high: Priority,
}

impl PriorityBand {
    /// Whether `priority` falls into this band.
    #[must_use]
    pub fn contains(&self, priority: Priority) -> bool {
        self.low <= priority && priority <= self.high
    }
}

/// `PriorityBandedConnectionPolicy` (RT-CORBA §5.8): multiple bands, each with
/// its own connection — requests travel over the connection of their band
/// (prevents priority inversion on a shared connection).
#[derive(Debug, Clone)]
pub struct PriorityBandedConnectionPolicy {
    /// The configured bands.
    pub bands: Vec<PriorityBand>,
}

impl PriorityBandedConnectionPolicy {
    /// Index of the band that covers `priority` (= connection selection).
    #[must_use]
    pub fn band_for(&self, priority: Priority) -> Option<usize> {
        self.bands.iter().position(|b| b.contains(priority))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn p(v: i16) -> Priority {
        Priority::new(v).unwrap()
    }

    #[test]
    fn client_propagated_uses_propagated_priority() {
        let pol = PriorityModelPolicy {
            model: PriorityModel::ClientPropagated,
            server_priority: p(10),
        };
        assert_eq!(pol.effective_priority(Some(p(99))), p(99));
        assert_eq!(pol.effective_priority(None), p(10)); // fallback
    }

    #[test]
    fn server_declared_ignores_propagated() {
        let pol = PriorityModelPolicy {
            model: PriorityModel::ServerDeclared,
            server_priority: p(10),
        };
        assert_eq!(pol.effective_priority(Some(p(99))), p(10));
    }

    #[test]
    fn lane_selection_picks_highest_covering() {
        let pool = Threadpool {
            lanes: alloc::vec![
                Lane {
                    priority: p(0),
                    static_threads: 2,
                    dynamic_threads: 0
                },
                Lane {
                    priority: p(50),
                    static_threads: 4,
                    dynamic_threads: 2
                },
                Lane {
                    priority: p(90),
                    static_threads: 1,
                    dynamic_threads: 0
                },
            ],
            stacksize: 0,
            allow_request_buffering: true,
            max_buffered_requests: 0,
        };
        assert_eq!(pool.lane_for(p(60)).unwrap().priority, p(50));
        assert_eq!(pool.lane_for(p(90)).unwrap().priority, p(90));
        assert_eq!(pool.lane_for(p(10)).unwrap().priority, p(0));
        assert_eq!(pool.static_capacity(), 7);
    }

    #[test]
    fn priority_band_selection() {
        let pol = PriorityBandedConnectionPolicy {
            bands: alloc::vec![
                PriorityBand {
                    low: p(0),
                    high: p(32)
                },
                PriorityBand {
                    low: p(33),
                    high: p(66)
                },
                PriorityBand {
                    low: p(67),
                    high: p(99)
                },
            ],
        };
        assert_eq!(pol.band_for(p(10)), Some(0));
        assert_eq!(pol.band_for(p(50)), Some(1));
        assert_eq!(pol.band_for(p(80)), Some(2));
    }
}
