// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Simple Endpoint Discovery Protocol (SEDP) — DDSI-RTPS 2.5 §8.5.4.
//!
//! SEDP sendet `PublicationBuiltinTopicData` /
//! `SubscriptionBuiltinTopicData` über Reliable-Builtin-Endpoints,
//! damit Participants ihre DataWriter und DataReader gegenseitig
//! entdecken können.
//!
//! ## Module
//!
//! - [`cache`] — `DiscoveredEndpointsCache`.
//! - [`reader`] — SEDP-Publication/Subscription-Reader.
//! - [`writer`] — SEDP-Publication/Subscription-Writer.
//! - [`stack`] — integrierte SEDP-State-Machine
//!   (Participant-Lifecycle → SEDP-Proxy-Wiring).

pub mod cache;
pub mod reader;
pub mod stack;
pub mod writer;

pub use cache::{
    CacheCaps, DiscoveredEndpointsCache, DiscoveredPublication, DiscoveredSubscription,
};
pub use reader::{
    SEDP_READER_MAX_SAMPLES, SedpPublicationsReader, SedpReaderError, SedpSubscriptionsReader,
};
pub use stack::{SedpEvents, SedpStack};
pub use writer::{
    SEDP_DEFAULT_DEPTH, SEDP_HEARTBEAT_PERIOD, SedpPublicationsWriter, SedpSubscriptionsWriter,
};
