// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lifecycle sync — spec-behavior mapping between CORBA POA state and
//! DDS discovery.
//!
//! A CORBA object becomes active with `activate_object` and inactive
//! with `deactivate_object`. On the DDS side this corresponds to a
//! `register_instance` / `unregister_instance` (spec OMG DDS 1.4
//! §2.2.2.2.1). The `LifecycleSync` propagates these events
//! bidirectionally.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use std::sync::Mutex;

/// Lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// A CORBA object was activated; we register the corresponding
    /// instance on the DDS side.
    CorbaActivated {
        /// Repository ID.
        repository_id: alloc::string::String,
        /// Object key.
        object_key: Vec<u8>,
    },
    /// A CORBA object was deactivated; we unregister the instance.
    CorbaDeactivated {
        /// Repository ID.
        repository_id: alloc::string::String,
        /// Object key.
        object_key: Vec<u8>,
    },
    /// A DDS reader discovered an instance; we can activate a
    /// CORBA forwarder servant.
    DdsInstanceDiscovered {
        /// Topic.
        topic: alloc::string::String,
        /// Instance handle (caller-layer type).
        instance_handle: u64,
    },
    /// A DDS reader reports `NOT_ALIVE_DISPOSED` — the corresponding
    /// forwarder servant is deactivated.
    DdsInstanceDisposed {
        /// Topic.
        topic: alloc::string::String,
        /// Instance handle.
        instance_handle: u64,
    },
}

/// Lifecycle sync — collects events and delivers them to the caller.
#[derive(Debug, Default)]
pub struct LifecycleSync {
    queue: Mutex<Vec<LifecycleEvent>>,
    instance_handles: Mutex<BTreeMap<(alloc::string::String, u64), bool>>,
}

impl LifecycleSync {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Notify hook for incoming events.
    pub fn notify(&self, event: LifecycleEvent) {
        if let LifecycleEvent::DdsInstanceDiscovered {
            topic,
            instance_handle,
        } = &event
        {
            if let Ok(mut h) = self.instance_handles.lock() {
                h.insert((topic.clone(), *instance_handle), true);
            }
        }
        if let LifecycleEvent::DdsInstanceDisposed {
            topic,
            instance_handle,
        } = &event
        {
            if let Ok(mut h) = self.instance_handles.lock() {
                h.insert((topic.clone(), *instance_handle), false);
            }
        }
        if let Ok(mut q) = self.queue.lock() {
            q.push(event);
        }
    }

    /// Drains all pending events.
    #[must_use]
    pub fn drain(&self) -> Vec<LifecycleEvent> {
        self.queue
            .lock()
            .ok()
            .map(|mut q| core::mem::take(&mut *q))
            .unwrap_or_default()
    }

    /// Number of pending events.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Checks whether a DDS instance is currently alive.
    #[must_use]
    pub fn is_dds_instance_alive(&self, topic: &str, handle: u64) -> bool {
        self.instance_handles
            .lock()
            .map(|h| {
                h.get(&(topic.to_string(), handle))
                    .copied()
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn notify_and_drain_round_trip() {
        let s = LifecycleSync::new();
        s.notify(LifecycleEvent::CorbaActivated {
            repository_id: "IDL:demo/Echo:1.0".into(),
            object_key: alloc::vec![1],
        });
        s.notify(LifecycleEvent::CorbaDeactivated {
            repository_id: "IDL:demo/Echo:1.0".into(),
            object_key: alloc::vec![1],
        });
        assert_eq!(s.pending(), 2);
        let events = s.drain();
        assert_eq!(events.len(), 2);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn dds_instance_alive_tracking() {
        let s = LifecycleSync::new();
        s.notify(LifecycleEvent::DdsInstanceDiscovered {
            topic: "T".into(),
            instance_handle: 42,
        });
        assert!(s.is_dds_instance_alive("T", 42));
        s.notify(LifecycleEvent::DdsInstanceDisposed {
            topic: "T".into(),
            instance_handle: 42,
        });
        assert!(!s.is_dds_instance_alive("T", 42));
    }

    #[test]
    fn unknown_instance_handle_is_not_alive() {
        let s = LifecycleSync::new();
        assert!(!s.is_dds_instance_alive("T", 99));
    }

    #[test]
    fn multiple_topics_tracked_independently() {
        let s = LifecycleSync::new();
        s.notify(LifecycleEvent::DdsInstanceDiscovered {
            topic: "T1".into(),
            instance_handle: 1,
        });
        s.notify(LifecycleEvent::DdsInstanceDiscovered {
            topic: "T2".into(),
            instance_handle: 1,
        });
        s.notify(LifecycleEvent::DdsInstanceDisposed {
            topic: "T1".into(),
            instance_handle: 1,
        });
        assert!(!s.is_dds_instance_alive("T1", 1));
        assert!(s.is_dds_instance_alive("T2", 1));
    }
}
