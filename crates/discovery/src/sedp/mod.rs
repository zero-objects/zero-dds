// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Simple Endpoint Discovery Protocol (SEDP) — DDSI-RTPS 2.5 §8.5.4.
//!
//! SEDP sends `PublicationBuiltinTopicData` /
//! `SubscriptionBuiltinTopicData` over reliable builtin endpoints so
//! that participants can discover each other's DataWriters and
//! DataReaders.
//!
//! ## Modules
//!
//! - [`cache`] — `DiscoveredEndpointsCache`.
//! - [`reader`] — SEDP publication/subscription reader.
//! - [`writer`] — SEDP publication/subscription writer.
//! - [`stack`] — integrated SEDP state machine
//!   (participant lifecycle → SEDP proxy wiring).

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
