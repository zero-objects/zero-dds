// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-java-omgdds`. Safety classification: **STANDARD**.
//!
//! Native Java-DDS-PSM (`org.omg.dds.*`) Pure-Java-Implementation ohne
//! JNI-Dependency. Der eigentliche Java-Source-Tree lebt unter
//! `crates/java-omgdds/java/src/main/java/org/omg/dds/*` (normative
//! OMG-API) und `org/zerodds/internal/*` (Implementation-Detail).
//!
//! Spec: OMG DDS-Java-PSM 1.0 (formal/2017-04-01).
//! Vendor-Spec: `docs/specs/zerodds-java-omgdds-1.0.md`.
//!
//! ## Schichten-Position
//!
//! Layer 6 — PSMs / Bindings (Pure-Java-Pfad ohne JNI; Codegen via
//! `zerodds-idl-java`).
//!
//! ## Architektur
//!
//! ```text
//! +------------------------------------------------------+
//! |  Java-User-Code                                      |
//! |  import org.omg.dds.domain.DomainParticipantFactory; |
//! +------------------------------------------------------+
//!                  |
//!                  v
//! +------------------------------------------------------+
//! |  org.omg.dds.*  (23 Java-Files, 18 mvn-Tests gruen)  |
//! |  - DomainParticipant, Topic<T>, Pub/Sub, DW/DR       |
//! |  - InProcessBus (Single-JVM Pub-Sub)                 |
//! |  - Xcdr2Codec (XTypes 1.3 §7.4 Wire-Form)            |
//! +------------------------------------------------------+
//! ```
//!
//! Multi-JVM via gRPC-Bridge ist v1.1-Stretch (siehe Vendor-Spec §5).
//!
//! ## Test
//!
//! - `mvn test` in `crates/java-omgdds/java/`: 18 Tests grün
//! - `cargo test -p zerodds-java-omgdds`: 1 Smoke-Test

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

/// Versions-Marker fuer die Pure-Java-Implementation.
pub const SCAFFOLD_VERSION: &str = "1.0.0-rc.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_version_is_set() {
        assert_eq!(SCAFFOLD_VERSION, "1.0.0-rc.1");
    }
}
