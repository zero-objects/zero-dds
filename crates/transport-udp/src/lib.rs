// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-udp`. Safety classification: **SAFE** (std-only).
//!
//! UDP/IP PSM-Implementation des `zerodds-transport::Transport`-Traits.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5** §9.6.1 — UDP/IP PSM Wire-Mapping.
//! - **DDSI-RTPS 2.5** §8.3.2 — Locator (Re-Export aus `zerodds-transport`).
//!
//! ## Implementiert (RC1)
//!
//! - UDPv4 Unicast Bind + Send + Recv via `std::net::UdpSocket`
//! - UDPv4 Multicast: `bind_multicast_v4` mit Group-Join +
//!   `SO_REUSEADDR`/`SO_REUSEPORT` + `set_multicast_ttl_v4` (DDSI-RTPS
//!   §9.6.1.4 SPDP/SEDP-Discovery-Pfad)
//! - Read-Timeout konfigurierbar via `with_timeout`
//! - Bind-Retry-Loop für CI-EADDRINUSE-Race (3× Backoff)
//! - Anwendungs-Layer-Fragmentation in `zerodds-rtps` (DATA_FRAG/NACK_FRAG,
//!   WP 1.2) übernimmt MTU-Handling — Transport selbst sendet Datagramme
//!   atomar mit `MAX_DATAGRAM_SIZE`-Cap.
//!
//! ## Bewusst nicht im Crate
//!
//! - **UDPv6**: Locator-Wire-Format unterstützt v6, aber Bind-API ist
//!   v4-spezifisch (`bind_v4`). v6 würde ein paralleles `bind_v6` +
//!   `Ipv6Addr`-Codepfad erfordern. Erweiterungspunkt — kein Layer-Break.
//! - **Async/Non-blocking**: Sync-Architektur ist gewählter Stil — DCPS
//!   nutzt eigene Tick-Scheduler, kein Tokio-Stack.
//! - **Pfad-MTU-Discovery**: Fragmentation läuft auf RTPS-Layer (WP 1.2).

#![cfg_attr(not(feature = "std"), no_std)]
// Default-Build hat KEIN unsafe; nur das Linux-`recvmmsg`-Modul
// (Feature `recvmmsg-batch`) braucht libc-FFI mit unsafe-Blocks.
// Mit aktiviertem Feature lockern wir `forbid` auf `deny` + lokales
// `allow(unsafe_code)` im recv_batch-Modul.
#![cfg_attr(not(feature = "recvmmsg-batch"), forbid(unsafe_code))]
#![cfg_attr(feature = "recvmmsg-batch", deny(unsafe_code))]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

/// Opt-2 (Spec `zerodds-zero-copy-1.0` §9): Linux-`recvmmsg`-Batch-Recv.
/// Nur kompiliert mit `recvmmsg-batch`-Feature aktiviert + auf Linux.
#[cfg(all(feature = "std", feature = "recvmmsg-batch", target_os = "linux"))]
pub mod recv_batch;
#[cfg(feature = "std")]
mod udp_transport;

#[cfg(feature = "std")]
pub use udp_transport::{MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportError};
