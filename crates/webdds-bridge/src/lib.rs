// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-webdds-bridge` — the DDS-WEB 1.0 gateway daemon
//! `zerodds-webdds-bridged`.
//!
//! Crate `zerodds-webdds-bridge`. Safety classification: **STANDARD**.
//!
//! Spec: OMG Web-Enabled DDS 1.0, `formal/2014-12-01`. §7-§8.3 (the
//! WebDDS object model + REST PSM) are implemented as a library in
//! `zerodds-web`; §8.4 (the SIMPLE-WSDL-SOAP PSM) as a library in
//! `zerodds-soap`. Neither crate implements a `DdsBackend`, an HTTP
//! server, or a body codec — `zerodds-web`'s own crate doc says so in as
//! many words ("HTTP-Server-Implementation + XML-Body-Serialization sind
//! Caller-Layer"). This crate is that caller layer, and the one process
//! that serves both PSMs — REST under `/dds/rest1`, SOAP under
//! `/soap/<binding>`, WSDL under `/wsdl/<binding>` — because they are two
//! platform mappings of the *same* object model (Spec §1.5), not two
//! unrelated protocols.
//!
//! # Modules
//!
//! * [`types_registry`] — resolves the wire type behind a topic, from a
//!   config-named IDL file or from TypeLookup discovery, into a
//!   [`zerodds_dcps::serialized_type::SerializedTypeSupport`] plus a
//!   [`zerodds_types::dynamic::DynamicType`] reflective view.

pub mod contained_entities;
pub mod daemon;
pub mod dynamic_rpc;
pub mod dynxml;
pub mod gateway;
pub mod http1;
pub mod metadata;
pub mod qos_update;
pub mod rest_service;
pub mod rpc_dispatch;
pub mod soap_service;
pub mod types_registry;
pub mod wsdl_bridge;
pub mod xmlfields;
