// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-opcua-bridge` — the OMG DDS-OPCUA Gateway 1.0 as a daemon on
//! `zerodds-dcps`.
//!
//! Crate `zerodds-opcua-bridge`. Safety classification: **STANDARD**.
//!
//! The four `no_std + alloc` OPC-UA crates
//! ([`zerodds_opcua_gateway`], [`zerodds_opcua_uacp`], [`zerodds_opcua_server`],
//! [`zerodds_opcua_pubsub`]) implement the OMG DDS-OPCUA Gateway 1.0 type
//! system and AddressSpace mapping, the Part 6 `opc.tcp` framing and
//! SecureChannel, the Part 4 Client/Server service set, and the Part 14
//! (UADP) PubSub wire stack — as libraries, with no DDS runtime, no
//! network listener loop and no configuration file. This crate is the
//! part that was missing: a running process.
//!
//! # What the daemon does
//!
//! - **DDS → OPC UA (Gateway 1.0 §9):** DDS topics — named in
//!   [`daemon::config`]'s `opcua.gateway_config` (the Spec §10 XML this
//!   daemon reads via [`zerodds_opcua_gateway::xml::parse_gateway_config`])
//!   — appear in this daemon's own `opc.tcp` [`zerodds_opcua_server::Server`]
//!   AddressSpace: one `ObjectNode` per DDS domain, one per topic, and one
//!   `Variable` per sample field, kept live by a DDS reader pump
//!   ([`address_space_build`]). A write to a `ua_to_dds_connection` node from
//!   any OPC-UA client goes back into DDS.
//! - **OPC UA → DDS (Gateway 1.0 §8):** the servers named in
//!   `opcua.client.servers` are browsed and subscribed as an OPC-UA client
//!   ([`ua_client`]); their monitored nodes are written into the configured
//!   DDS topics.
//! - **Part 14 PubSub:** the `opcua.pubsub.connections` named in the config
//!   run a [`zerodds_opcua_pubsub::daemon::Publisher`]/[`zerodds_opcua_pubsub::daemon::Subscriber`]
//!   pair per connection over UDP multicast ([`pubsub_glue`]), publishing
//!   DataSets built from DDS topics and writing received DataSets back into
//!   DDS.
//!
//! Types are resolved through the reflective layer
//! ([`zerodds-dcps::serialized_type::SerializedTypeSupport`], fed by a
//! `CompleteTypeObject` this daemon builds from an operator-supplied `.idl`
//! file — [`idl`]) rather than a compiled Rust type, the same primitive
//! `crates/xrce-agent`'s daemon and `crates/routing-service` use for a topic
//! whose Rust type is not linked in.
//!
//! # Module map
//!
//! - [`dynamic_type`] / [`dynamic_value`] — `zerodds_types::dynamic` ↔ the
//!   OPC-UA gateway's own symbolic type/value model
//!   ([`zerodds_opcua_gateway::dds_to_ua::walker::DdsType`] /
//!   [`zerodds_opcua_gateway::data_value::Variant`]).
//! - [`idl`] — compiles an operator-supplied `.idl` file and builds the
//!   [`zerodds-dcps::serialized_type::SerializedTypeSupport`] for a named
//!   type in it.
//! - [`dds_bridge`] — the DDS-side port: participant/topic/reader/writer
//!   registration against a real [`zerodds-dcps::runtime::DcpsRuntime`], plus
//!   SEDP-discovered topic enumeration.
//! - [`address_space_build`] — Gateway 1.0 §9.3 AddressSpace materialization
//!   (`DomainNode`/`TopicNode`/instance nodes) over a live DDS reader.
//! - [`ua_client`] — the OPC-UA client side of Gateway 1.0 §8.
//! - [`pubsub_glue`] — the Part 14 UADP Publisher/Subscriber wiring.
//! - [`daemon`] — the process itself: CLI, YAML config, SIGHUP/SIGTERM,
//!   systemd `Type=notify`, observability.
//!
//! Feature `daemon` (default-on) gates the whole crate: the reflective
//! type layer, the DDS port, the AddressSpace/PubSub glue and the process
//! itself. Without it the crate is empty — there is no other consumer of
//! these modules; unlike the four library crates it wires together, this
//! one is the product, not a component.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "daemon")]
pub mod address_space_build;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "daemon")]
pub mod dds_bridge;
#[cfg(feature = "daemon")]
pub mod dynamic_type;
#[cfg(feature = "daemon")]
pub mod dynamic_value;
#[cfg(feature = "daemon")]
pub mod idl;
#[cfg(feature = "daemon")]
pub mod pubsub_glue;
#[cfg(feature = "daemon")]
pub mod ua_client;
