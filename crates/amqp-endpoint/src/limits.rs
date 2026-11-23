// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Resource limits + DoS caps for the DDS-AMQP endpoint.
//!
//! Spec source: dds-amqp-1.0-beta1.pdf Annex A IDL Configuration
//! Schema (`ResourceLimits` struct) + §6.1 Implementation Note.

/// Spec Annex A — `ResourceLimits` from the IDL schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum number of concurrent connections.
    pub max_connections: u32,
    /// Maximum number of sessions per connection.
    pub max_sessions_per_connection: u32,
    /// Maximum number of links per session.
    pub max_links_per_session: u32,
    /// Maximum frame size (bytes).
    pub max_frame_size: u32,
    /// Idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        // Spec §6.1 Implementation Note: default caps should be
        // conservative so that operational hardening applies from day one.
        Self {
            max_connections: 1024,
            max_sessions_per_connection: 8,
            max_links_per_session: 64,
            max_frame_size: 65_536,
            idle_timeout_ms: 60_000,
        }
    }
}

/// The bounds of the nodes a listener holds messages in (OASIS AMQP 1.0
/// Part 3 §3.3). Not part of Annex A: the `<nodes>` element of the XML
/// configuration. A node here is a queue in the memory of the process; an
/// attach or a transfer beyond a bound is refused with
/// `amqp:resource-limit-exceeded` (Part 2 §2.8.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeLimits {
    /// Nodes created by an attach: by address and dynamic ones together.
    pub max_nodes: u32,
    /// Messages one node holds.
    pub max_messages: u32,
    /// Octets of message payload one node holds.
    pub max_octets: u64,
}

impl Default for NodeLimits {
    fn default() -> Self {
        Self {
            max_nodes: 1024,
            max_messages: 10_000,
            max_octets: 64 * 1024 * 1024,
        }
    }
}

/// The bounds of the transactions a listener's coordinator holds (OASIS
/// AMQP 1.0 Part 4). Not part of Annex A: the `<transactions>` element of
/// the XML configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxnLimits {
    /// Transactions one connection holds declared and not discharged. A
    /// declare beyond it is refused with `amqp:resource-limit-exceeded`.
    /// 0: the coordinator declares none.
    pub max_transactions: u32,
    /// How long a transaction may stay undischarged, in milliseconds; it
    /// is rolled back then, and its discharge is refused with
    /// `amqp:transaction:timeout` (Part 4 §4.5.8). 0: no time-out.
    pub timeout_ms: u64,
}

impl Default for TxnLimits {
    fn default() -> Self {
        Self {
            max_transactions: 64,
            timeout_ms: 300_000,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ddsamqp_7_10_defaults_match_spec_implementation_note() {
        let l = ResourceLimits::default();
        assert_eq!(l.max_connections, 1024);
        assert_eq!(l.max_sessions_per_connection, 8);
        assert_eq!(l.max_links_per_session, 64);
        assert_eq!(l.max_frame_size, 65_536);
        assert_eq!(l.idle_timeout_ms, 60_000);
    }
}
