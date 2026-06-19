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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec_implementation_note() {
        let l = ResourceLimits::default();
        assert_eq!(l.max_connections, 1024);
        assert_eq!(l.max_sessions_per_connection, 8);
        assert_eq!(l.max_links_per_session, 64);
        assert_eq!(l.max_frame_size, 65_536);
        assert_eq!(l.idle_timeout_ms, 60_000);
    }
}
