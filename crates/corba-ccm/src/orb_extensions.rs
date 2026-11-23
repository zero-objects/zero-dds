// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 ORB extensions:
//! - §16 Portable Interceptors (Part 1), re-exported from
//!   [`crate::portable_interceptor`]
//! - §17 CORBA Messaging (Part 1)
//! - Part 2 §8 Inter-ORB Bridges
//! - Part 2 §9.8/§9.9 BiDirectional GIOP
//!
//! ZeroDDS has no full ORB; these modules provide a configuration
//! + data-model layer as a stub for migration tooling.

#[cfg(feature = "std")]
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

// ===========================================================================
// §16 Portable Interceptors
// ===========================================================================

// The portable interceptors live in `crate::portable_interceptor`; the
// names stay reachable here.
#[cfg(feature = "std")]
pub use crate::portable_interceptor::{
    ClientRequestInterceptor, CurrentError, DuplicateName, InitializerError, InterceptionPoint,
    InterceptorError, InterceptorRegistry, InvalidSlot, IorInfo, IorInterceptor, OrbInitInfo,
    OrbInitializer, Outcome, PiCurrent, Policy, PolicyError, PolicyFactory, RegistrationError,
    ReplyStatus, RequestException, RequestInfo, ServerRequestInterceptor, ServiceContextInjector,
    register_orb_initializer,
};

// ===========================================================================
// §17 CORBA Messaging (Part 1)
// ===========================================================================

/// Spec §17 — messaging policy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagingPolicy {
    /// `RebindPolicy` — rebinding behavior (spec §17.3).
    Rebind,
    /// `SyncScopePolicy` — synchronization scope (spec §17.4).
    SyncScope,
    /// `RequestPriorityPolicy` — request priority (spec §17.5).
    RequestPriority,
    /// `ReplyPriorityPolicy`.
    ReplyPriority,
    /// `RoutingPolicy` — TII (time-independent invocation) (spec §17.7).
    Routing,
    /// `MaxHopsPolicy`.
    MaxHops,
    /// `RequestStartTimePolicy` / `RequestEndTimePolicy`.
    RequestTime,
    /// `ReplyStartTimePolicy` / `ReplyEndTimePolicy`.
    ReplyTime,
    /// `RelativeRoundtripTimeoutPolicy`.
    RelativeRoundtripTimeout,
    /// `RoutingTypeRange`.
    RoutingTypeRange,
}

impl MessagingPolicy {
    /// Spec §17 — wire value (`PolicyType` is u32, vendor-defined).
    /// Follows the order in the OMG `Messaging.idl` (formal/2011-11-02
    /// §B.5.1, policy types 23..32).
    #[must_use]
    pub const fn policy_type(self) -> u32 {
        match self {
            Self::Rebind => 23,
            Self::SyncScope => 24,
            Self::RequestPriority => 25,
            Self::ReplyPriority => 26,
            Self::Routing => 30,
            Self::MaxHops => 32,
            Self::RequestTime => 27,
            Self::ReplyTime => 28,
            Self::RelativeRoundtripTimeout => 31,
            Self::RoutingTypeRange => 33,
        }
    }
}

/// Spec §17.1 — AMI (Asynchronous Method Invocation) reply-handler style.
/// `ReplyHandlerStyle` marks the code-generator path for the
/// stub code (callback vs. polling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmiReplyHandler {
    /// Callback style — reply-handler object.
    Callback,
    /// Polling style — poller object.
    Polling,
}

/// Spec §17.5 — `AmiReplySink` trait for asynchronous reply processing.
///
/// Adapter trait that makes the three AMI callback methods from the spec IDL
/// (`handle_reply` / `handle_excep` / `handle_other`) concrete.
/// The `dispatch_async_reply` function maps a GIOP reply
/// onto the matching callback method.
pub trait AmiReplySink: Send + Sync {
    /// Spec §17.5.1 — `handle_reply(...)` for `NoException`.
    fn handle_reply(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_excep(...)` for user/system exception.
    fn handle_excep(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_other(...)` for LocationForward etc.
    fn handle_other(&self, request_id: u32, body: &[u8]);
}

/// Spec §17.5 — maps a `corba_giop::Reply` onto the matching
/// AMI callback method. This is the bridge between the GIOP wire and
/// the `AmiReplySink` surface.
pub fn dispatch_async_reply<S: AmiReplySink + ?Sized>(sink: &S, reply: &zerodds_corba_giop::Reply) {
    use zerodds_corba_giop::ReplyStatusType;
    match reply.reply_status {
        ReplyStatusType::NoException => sink.handle_reply(reply.request_id, &reply.body),
        ReplyStatusType::UserException | ReplyStatusType::SystemException => {
            sink.handle_excep(reply.request_id, &reply.body);
        }
        ReplyStatusType::LocationForward
        | ReplyStatusType::LocationForwardPerm
        | ReplyStatusType::NeedsAddressingMode => {
            sink.handle_other(reply.request_id, &reply.body);
        }
    }
}

/// Spec §17.7 — persistent request store for time-independent
/// invocations (TII). In-memory; the backing layer may be on disk.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct PersistentRequestStore {
    inner: std::sync::Mutex<BTreeMap<u32, PersistentRequestEntry>>,
}

/// Entry in the persistent request store.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentRequestEntry {
    /// Body of the original request (CDR-encoded).
    pub body: Vec<u8>,
    /// Spec §17.7 — deadline for the request (epoch seconds).
    pub deadline_secs: u64,
}

#[cfg(feature = "std")]
impl PersistentRequestStore {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §17.7 — stores a request for asynchronous
    /// processing.
    pub fn add(&self, request_id: u32, body: Vec<u8>, deadline_secs: u64) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(
                request_id,
                PersistentRequestEntry {
                    body,
                    deadline_secs,
                },
            );
        }
    }

    /// Spec §17.7 — retrieves a request (removes it from the store).
    #[must_use]
    pub fn poll(&self, request_id: u32) -> Option<PersistentRequestEntry> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut g| g.remove(&request_id))
    }

    /// Spec §17.7 — removes all entries with `deadline_secs < now`.
    /// Returns the request_ids of the expired entries.
    pub fn timeout_expired(&self, now_secs: u64) -> Vec<u32> {
        let Ok(mut g) = self.inner.lock() else {
            return Vec::new();
        };
        let expired: Vec<u32> = g
            .iter()
            .filter(|(_, e)| e.deadline_secs < now_secs)
            .map(|(k, _)| *k)
            .collect();
        for k in &expired {
            g.remove(k);
        }
        expired
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ===========================================================================
// Part 2 §8 Inter-ORB Bridges
// ===========================================================================

/// Spec Part 2 §8 — bridge mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeMode {
    /// `Inline-Bridge` — embedded in an ORB process.
    Inline,
    /// `Request-Level-Bridge` — a separate bridge process via DSI/DII.
    RequestLevel,
}

/// Spec Part 2 §8 — bridge configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Bridge mode.
    pub mode: BridgeMode,
    /// Source ORB identifier (e.g. `"corba"`).
    pub source_orb: String,
    /// Target ORB identifier (e.g. `"dds"`).
    pub target_orb: String,
}

// ===========================================================================
// Part 2 §9.8/§9.9 Bi-Directional GIOP
// ===========================================================================

/// Spec Part 2 §9.8 — BiDir policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiDirPolicy {
    /// `NORMAL` — standard IIOP, no BiDir.
    Normal,
    /// `BOTH` — the server can also send outgoing requests over the same
    /// connection (callback style).
    Both,
}

/// Spec Part 2 §9.9 — bidirectional service context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiDirServiceContext {
    /// Spec §9.9.1 — listen points (the client's host:port pairs).
    pub listen_points: Vec<(String, u16)>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // §17 Messaging
    #[test]
    fn messaging_policies_distinct() {
        assert_ne!(MessagingPolicy::Rebind, MessagingPolicy::SyncScope);
    }

    #[test]
    fn ami_reply_handler_distinct() {
        assert_ne!(AmiReplyHandler::Callback, AmiReplyHandler::Polling);
    }

    // Part 2 §8 Bridges
    #[test]
    fn bridge_modes_distinct() {
        assert_ne!(BridgeMode::Inline, BridgeMode::RequestLevel);
    }

    #[test]
    fn bridge_config_construct() {
        let c = BridgeConfig {
            mode: BridgeMode::RequestLevel,
            source_orb: "corba".into(),
            target_orb: "dds".into(),
        };
        assert_eq!(c.source_orb, "corba");
    }

    // Part 2 §9.8/§9.9 BiDir
    #[test]
    fn bidir_policy_distinct() {
        assert_ne!(BiDirPolicy::Normal, BiDirPolicy::Both);
    }

    #[test]
    fn bidir_service_context_listen_points() {
        let sc = BiDirServiceContext {
            listen_points: alloc::vec![("client.example".into(), 8080)],
        };
        assert_eq!(sc.listen_points.len(), 1);
    }

    // §17 Messaging — Policy-Type-Wire
    #[test]
    fn messaging_policy_wire_values_match_omg_messaging_idl() {
        assert_eq!(MessagingPolicy::Rebind.policy_type(), 23);
        assert_eq!(MessagingPolicy::SyncScope.policy_type(), 24);
        assert_eq!(MessagingPolicy::Routing.policy_type(), 30);
        assert_eq!(MessagingPolicy::RelativeRoundtripTimeout.policy_type(), 31);
    }

    // §17 AMI — Reply-Dispatch
    struct RecordingSink {
        replies: alloc::sync::Arc<std::sync::Mutex<Vec<(u32, &'static str)>>>,
    }
    impl AmiReplySink for RecordingSink {
        fn handle_reply(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "reply"));
            }
        }
        fn handle_excep(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "excep"));
            }
        }
        fn handle_other(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "other"));
            }
        }
    }

    #[test]
    fn ami_handler_handles_no_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 42,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(42_u32, "reply")]);
    }

    #[test]
    fn ami_handler_handles_user_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 7,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(7_u32, "excep")]);
    }

    // §17.7 persistent request store for TII
    #[cfg(feature = "std")]
    #[test]
    fn persistent_request_store_add_poll_timeout() {
        let s = PersistentRequestStore::new();
        s.add(1, alloc::vec![0xAA], 100);
        s.add(2, alloc::vec![0xBB], 50);
        s.add(3, alloc::vec![0xCC], 200);
        assert_eq!(s.len(), 3);

        // Poll retrieves an entry (removes it from the store).
        let e1 = s.poll(1).expect("present");
        assert_eq!(e1.body, alloc::vec![0xAA]);
        assert_eq!(e1.deadline_secs, 100);
        assert!(s.poll(1).is_none());
        assert_eq!(s.len(), 2);

        // Timeout @ now=120 → request 2 (deadline=50) is expired.
        let expired = s.timeout_expired(120);
        assert_eq!(expired, alloc::vec![2]);
        assert_eq!(s.len(), 1);

        // Request 3 (deadline=200) stays in.
        assert!(s.poll(3).is_some());
    }
}
