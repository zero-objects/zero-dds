// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`TopicMeta`] — everything a restarted service needs to resume serving a
//! topic without waiting for a writer or reader to be (re-)discovered.

use alloc::string::String;

use crate::contract::Contract;

/// Everything [`crate::DurabilityStore::set_topic_meta`] persists for a topic
/// when it is first served, and [`crate::DurabilityStore::topic_meta`]
/// returns on a later lookup — typically after a process restart, so the
/// service can resume serving the topic from the store alone (`ADR 0009`,
/// `zerodds-durability-service` module docs, "Startup-sync").
///
/// Deliberately does NOT carry a wire representation / byte order: those
/// travel on [`crate::DurabilitySample`] itself (`representation`,
/// `big_endian`), and every sample [`crate::DurabilityStore::replay_for_topic`]
/// returns already carries its own — so a replay writer primed from the store
/// re-declares the right encapsulation per sample regardless of what is (or
/// is not) in this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicMeta {
    /// The application/IDL type name the topic is served under (SEDP
    /// `type_name`, DDSI-RTPS 2.5 §8.4.2.1).
    pub type_name: String,
    /// Whether the topic is keyed (DDSI-RTPS 2.5 §9.3.1.2 entityKind).
    pub keyed: bool,
    /// The retention contract in force for the topic (`zerodds-durability-service`
    /// module docs, "The retention contract").
    pub contract: Contract,
}
