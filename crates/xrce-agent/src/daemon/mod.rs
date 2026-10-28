// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-xrce-agentd` — the DDS-XRCE agent daemon.
//!
//! A process XRCE clients (eProsima Micro-XRCE-DDS Client 3.0.1,
//! `zerodds-xrce-client`) connect to over UDP/TCP/serial, which represents
//! them in the DDS data space through `zerodds-dcps`, per OMG DDS-XRCE 1.0
//! §7.3: "the XRCE Agent represents the XRCE client in the DDS data
//! space".
//!
//! ## Module map
//!
//! - [`access`] — the AccessController of §7.4: authorized ClientKeys,
//!   required client properties, per-client permissions for the object
//!   operations.
//! - [`handshake`] — re-exports `zerodds_xrce::handshake`'s
//!   `CLIENT_Representation`/`AGENT_Representation` codec (§7.8.2, Annex
//!   A) — moved there so `zerodds-xrce-client` can share it too.
//! - [`session`] — [`session::SessionEngine`], the pure protocol state
//!   machine: `Message` in, `Message`s out, no sockets. Built on this
//!   crate's [`crate::XrceAgent`] plus `zerodds-xrce`'s
//!   `ReliableStreamState`/`FragmentAssembler`/`ReadStream`. Talks to the
//!   DDS side only through the [`dds_port::DdsPort`] trait, so it is
//!   testable without a live `DcpsRuntime` — the replay test
//!   (`tests/replay_foreign_session.rs`) and the daemon binary both drive
//!   the same engine.
//! - [`dds_port`] — the [`dds_port::DdsPort`] trait plus
//!   [`dds_port::NullDdsPort`] (accepts every CREATE, drops every write —
//!   used by unit/replay tests) and [`dcps_bridge::RealDdsPort`] (the real
//!   `zerodds-dcps` bridge the daemon binary runs).
//! - [`dcps_bridge`] — [`dcps_bridge::RealDdsPort`].
//! - [`entity_repr`] — reads a CREATE's XML (attribute and Fast DDS
//!   element form) or binary representation into the names, container
//!   relations and QoS the DDS side is created with.
//! - [`definitions`] — the Domains, DomainParticipants and Applications of
//!   the configuration files (§9.3) and the XRCE objects inside a
//!   DomainParticipant, with the `ObjectId` of their names.
//! - [`config`] — the daemon's YAML configuration.
//! - [`cli`] — `--help`/`--version` text; `--config`/`--check-config`/
//!   `--set` are handled by `zerodds_dcps::cli::Configured`.
//! - [`transport`] — UDP/TCP/serial listener loops feeding the session
//!   engine.
//! - [`discovery`] — answers `GET_INFO` on the multicast discovery group
//!   (§11.2.4) with the daemon's listening locators.
//! - [`server`] — process wiring: SIGHUP reload, SIGTERM shutdown,
//!   systemd `sd_notify`.

pub mod access;
pub(crate) mod binary_forms;
pub mod cli;
pub mod config;
pub mod dcps_bridge;
pub mod dds_port;
pub mod definitions;
pub mod discovery;
pub mod entity_repr;
pub mod handshake;
pub mod profiles;
pub mod server;
pub mod session;
pub mod transport;
