// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA Component Model 4.0 (CCM).
//!
//! Crate `zerodds-corba-ccm`. Safety classification: **STANDARD**.
//! Spec OMG CCM 4.0 (`formal/2006-04-01`) §6 + §13 (Lightweight Profile).
//!
//! Full CCM container stack with component model, home model,
//! CIDL data model, CIF (Component Implementation Framework),
//! container runtime, ORB extensions, persistent-state-service stub,
//! Time PSM and TimerEventService including a CosEventService adapter
//! (feature `cos-event`).
//!
//! ## Example
//!
//! ```
//! use zerodds_corba_ccm::conformance::CCM_CONFORMANCE_BASIC_LEVEL;
//! assert_eq!(CCM_CONFORMANCE_BASIC_LEVEL, "OMG-CCM-4.0:Basic");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod cidl;
pub mod cif;
pub mod component_def;
pub mod context;
pub mod dynamic_api;
pub mod home;
pub mod orb_extensions;
pub mod port;

#[cfg(feature = "std")]
pub mod container;
#[cfg(all(feature = "std", feature = "cos-event"))]
pub mod cos_event_bridge;
#[cfg(feature = "std")]
pub mod lifecycle;
#[cfg(feature = "std")]
pub mod orb_core;
#[cfg(feature = "std")]
pub mod pss;
#[cfg(feature = "std")]
pub mod time_psm;
#[cfg(feature = "std")]
pub mod timer;

pub use cidl::{Composition, HomeExecutor, StorageHome, StorageType};
pub use cif::{ComponentExecutor, ExecutorLocator, KeyedExecutor, SessionExecutor};
pub use component_def::{
    AttributeDef, ComponentDef, EventSinkDef, EventSourceDef, FacetDef, ReceptacleDef,
};
pub use context::ComponentContext;
pub use home::{HomeDef, HomeFinder};
pub use port::{ConnectionId, EventStream, PortRegistry};

#[cfg(feature = "std")]
pub use container::{Container, ContainerError, ContainerType, LifecycleState};
#[cfg(all(feature = "std", feature = "cos-event"))]
pub use cos_event_bridge::EventChannelTimerCallback;
#[cfg(feature = "std")]
pub use timer::{TimerEventService, TimerHandle, TimerKind};

// ---------------------------------------------------------------------------
// OMG CCM 4.0 §2 conformance markers
// ---------------------------------------------------------------------------

/// OMG CCM 4.0 §2 conformance levels — formal identifier per
/// vendor conformance path. Tooling can consume these strings as
/// capability markers.
pub mod conformance {
    /// §2 item 4 — Basic Level non-Java (IDL extensions + CIDL +
    /// container + XML D&C, ORB prerequisite). Active once
    /// the CIDL AST + container-runtime lifecycle are wired in.
    pub const CCM_CONFORMANCE_BASIC_LEVEL: &str = "OMG-CCM-4.0:Basic";

    /// §2 item 5 — Basic Level Java (EJB1.1 + java-to-IDL).
    /// Cross-ref `crates/corba-ccm-ejb/`.
    pub const CCM_CONFORMANCE_BASIC_LEVEL_JAVA: &str = "OMG-CCM-4.0:BasicJava";

    /// §2 item 7 — Lightweight CCM Profile (subset per §13).
    pub const LIGHTWEIGHT_CCM_LEVEL: &str = "OMG-CCM-4.0:Lightweight";

    /// §6.12 / §13.10 — restriction marker for the lightweight filter.
    pub const LWCCM_RESTRICTIONS_ENFORCED: &str = "OMG-CCM-4.0:LwCCM-Restrictions";

    /// §13.3 — Lightweight Profile: filter active, no introspection,
    /// no generic navigation, no type-specific generic ops.
    pub const LWCCM_FILTER_ACTIVE: &str = "OMG-CCM-4.0:LwCCM-Filter";

    // -----------------------------------------------------------------
    // CORBA 3.3 Part 3 — CCM-related conformance markers
    // -----------------------------------------------------------------

    /// CORBA 3.3 Part 3 §6.13 — CCM Conformance Requirements
    /// (doc marker; follows §2 once the IDL subset is covered).
    pub const CORBA_PART3_6_13_CCM_CONFORMANCE: &str = "OMG-CORBA-3.3-P3:6.13-CCM-Conformance";

    /// CORBA 3.3 Part 3 §7 — Generic Interaction Support
    /// (Simple Connectors §7.2 covered; templated connectors §7.3/§7.4
    /// addressable as a codegen extension).
    pub const CORBA_PART3_7_GENERIC_INTERACTION: &str = "OMG-CORBA-3.3-P3:7-Generic-Interaction";

    /// CORBA 3.3 Part 3 §14 — Lightweight CCM Profile constants/
    /// doc marker. The code supports the full model + LwCCM filter;
    /// this marker highlights the formal LwCCM conformance level.
    pub const CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE: &str = "OMG-CORBA-3.3-P3:14-LwCCM-Profile";

    /// CORBA 3.3 Part 2 §10.6 — CSIv2 Conformance Levels.
    /// The spec lists three levels (0=ServerAuth, 1=ClientAuthN+Server,
    /// 2=ClientAuthZ identity assertion). ZeroDDS covers all three
    /// via `crates/corba-csiv2/`.
    pub const CORBA_PART2_10_6_CSIV2_LEVEL_0: &str = "OMG-CORBA-3.3-P2:10.6-CSIv2-L0";
    /// CSIv2 Level 1 (client authentication in addition).
    pub const CORBA_PART2_10_6_CSIV2_LEVEL_1: &str = "OMG-CORBA-3.3-P2:10.6-CSIv2-L1";
    /// CSIv2 Level 2 (identity assertion + authorization).
    pub const CORBA_PART2_10_6_CSIV2_LEVEL_2: &str = "OMG-CORBA-3.3-P2:10.6-CSIv2-L2";

    // -----------------------------------------------------------------
    // CCM 4.0 §2 optional conformance markers (items 3 + 8)
    // -----------------------------------------------------------------

    /// CCM 4.0 §2 item 3 — Optional Extended Level. The ZeroDDS stub
    /// layer addresses persistent state (`crates/ccm-pss`) and the
    /// configurator interface; the full Extended Level is not
    /// certifiable without a vendor ORB.
    pub const CCM_OPTIONAL_EXTENDED_LEVEL: &str = "OMG-CCM-4.0:Optional-Extended";

    /// CCM 4.0 §2 item 8 — ORB vendor. ZeroDDS provides
    /// `corba-ccm::orb_core::Orb` as a configuration-layer stub
    /// for component-specific extensions on the ORB.
    pub const CCM_ORB_VENDOR_STUB: &str = "OMG-CCM-4.0:ORB-Vendor-Stub";
}

#[cfg(test)]
mod conformance_tests {
    use super::conformance::*;

    #[test]
    fn ccm_conformance_basic_level_marker_matches_spec() {
        assert_eq!(CCM_CONFORMANCE_BASIC_LEVEL, "OMG-CCM-4.0:Basic");
    }

    #[test]
    fn ccm_conformance_basic_level_java_marker_matches_spec() {
        assert_eq!(CCM_CONFORMANCE_BASIC_LEVEL_JAVA, "OMG-CCM-4.0:BasicJava");
    }

    #[test]
    fn lightweight_ccm_level_marker_matches_spec() {
        assert_eq!(LIGHTWEIGHT_CCM_LEVEL, "OMG-CCM-4.0:Lightweight");
    }

    #[test]
    fn lwccm_restrictions_marker_matches_spec() {
        assert_eq!(
            LWCCM_RESTRICTIONS_ENFORCED,
            "OMG-CCM-4.0:LwCCM-Restrictions"
        );
    }

    #[test]
    fn lwccm_filter_marker_matches_spec() {
        assert_eq!(LWCCM_FILTER_ACTIVE, "OMG-CCM-4.0:LwCCM-Filter");
    }

    #[test]
    fn all_markers_are_distinct() {
        let markers = [
            CCM_CONFORMANCE_BASIC_LEVEL,
            CCM_CONFORMANCE_BASIC_LEVEL_JAVA,
            LIGHTWEIGHT_CCM_LEVEL,
            LWCCM_RESTRICTIONS_ENFORCED,
            LWCCM_FILTER_ACTIVE,
            CORBA_PART3_6_13_CCM_CONFORMANCE,
            CORBA_PART3_7_GENERIC_INTERACTION,
            CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE,
            CORBA_PART2_10_6_CSIV2_LEVEL_0,
            CORBA_PART2_10_6_CSIV2_LEVEL_1,
            CORBA_PART2_10_6_CSIV2_LEVEL_2,
        ];
        let mut seen = std::collections::HashSet::new();
        for m in markers {
            assert!(seen.insert(m), "duplicate marker: {m}");
        }
    }

    #[test]
    fn corba_part3_6_13_marker_namespace() {
        assert!(CORBA_PART3_6_13_CCM_CONFORMANCE.starts_with("OMG-CORBA-3.3-P3:"));
    }

    #[test]
    fn corba_part3_7_marker_namespace() {
        assert!(CORBA_PART3_7_GENERIC_INTERACTION.starts_with("OMG-CORBA-3.3-P3:"));
    }

    #[test]
    fn corba_part3_14_marker_namespace() {
        assert!(CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE.starts_with("OMG-CORBA-3.3-P3:"));
    }

    #[test]
    fn csiv2_level_markers_match_spec() {
        assert_eq!(
            CORBA_PART2_10_6_CSIV2_LEVEL_0,
            "OMG-CORBA-3.3-P2:10.6-CSIv2-L0"
        );
        assert_eq!(
            CORBA_PART2_10_6_CSIV2_LEVEL_1,
            "OMG-CORBA-3.3-P2:10.6-CSIv2-L1"
        );
        assert_eq!(
            CORBA_PART2_10_6_CSIV2_LEVEL_2,
            "OMG-CORBA-3.3-P2:10.6-CSIv2-L2"
        );
    }
}
