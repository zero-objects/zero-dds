// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-java-omgdds`. Safety classification: **STANDARD**.
//!
//! Native Java DDS PSM (`org.omg.dds.*`) pure-Java implementation without
//! a JNI dependency. The actual Java source tree lives under
//! `crates/java-omgdds/java/src/main/java/org/omg/dds/*` (normative
//! OMG API) and `org/zerodds/internal/*` (implementation detail).
//!
//! Spec: OMG DDS Java PSM 1.0 (formal/2017-04-01).
//! Vendor spec: `docs/specs/zerodds-java-omgdds-1.0.md`.
//!
//! ## Layer position
//!
//! Layer 6 — PSMs / bindings (pure-Java path without JNI; codegen via
//! `zerodds-idl-java`).
//!
//! ## Architecture
//!
//! ```text
//! +------------------------------------------------------+
//! |  Java user code                                     |
//! |  import org.omg.dds.domain.DomainParticipantFactory; |
//! +------------------------------------------------------+
//!                  |
//!                  v
//! +------------------------------------------------------+
//! |  org.omg.dds.*  (23 Java files, 18 mvn tests green) |
//! |  - DomainParticipant, Topic<T>, Pub/Sub, DW/DR       |
//! |  - InProcessBus (single-JVM pub/sub)                |
//! |  - Xcdr2Codec (XTypes 1.3 §7.4 wire form)           |
//! +------------------------------------------------------+
//! ```
//!
//! Multi-JVM via a gRPC bridge is a v1.1 stretch (see vendor spec §5).
//!
//! ## Test
//!
//! - `mvn test` in `crates/java-omgdds/java/`: 18 tests green
//! - `cargo test -p zerodds-java-omgdds`: 1 smoke test

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

/// Version marker for the pure-Java implementation.
pub const SCAFFOLD_VERSION: &str = "1.0.0-rc.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_version_is_set() {
        assert_eq!(SCAFFOLD_VERSION, "1.0.0-rc.1");
    }
}
