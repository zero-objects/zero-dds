// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL parser feature flags (spec-completeness via feature gating).
//!
//! Goal: spec-completeness against OMG IDL 4.2. All constructs (DDS profile,
//! XTypes, CORBA, CCM, template modules, vendor extensions) are present in the
//! default grammar as productions — but are activatable via an
//! [`IdlFeatures`] bool flag. On use of a construct whose
//! feature is OFF, the validator returns an
//! `Error::FeatureNotEnabled`.
//!
//! # Profile vs. features
//!
//! [`super::config::Profile`] is a high-level profile (`DdsBasic`,
//! `DdsExtensible`, `Full`, `OpenSpliceLegacy`, ...). Each profile
//! maps to a default `IdlFeatures` mask via
//! [`IdlFeatures::for_profile`]. The user can additionally toggle individual
//! feature bits (e.g. `DdsExtensible` + `corba_template_modules`
//! active for DDS-RPC templates).

pub mod gate;

/// Feature-flag set for the IDL parser.
///
/// The default is DDS-Extensible (Plain DDS + XTypes + DDS-RPC), each
/// without CORBA/CCM constructs and without vendor extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdlFeatures {
    // ========================================================================
    // §7.4.5 — Value Types Full
    // ========================================================================
    /// `valuetype` with `state_member`, `init_dcl`, `factory` body
    /// (Spec §7.4.5 Rules 100-109). Box/Forward are always active.
    pub corba_value_types_full: bool,
    /// `abstract valuetype`, `custom valuetype`, `truncatable`
    /// (Spec §7.4.5 Rules 125-132).
    pub corba_value_types_extras: bool,

    // ========================================================================
    // §7.4.6 — Repository-IDs
    // ========================================================================
    /// `typeid` and `typeprefix` top-level directives (rules 111-117).
    pub corba_repository_ids: bool,
    /// `import` Top-Level-Direktive (Rules 115-117).
    pub corba_import: bool,

    // ========================================================================
    // §7.4.7 — CORBA interface extensions
    // ========================================================================
    /// `local interface` (Rule 119).
    pub corba_local_interface: bool,
    /// `abstract interface`.
    pub corba_abstract_interface: bool,
    /// `: Object` as base.
    pub corba_object_base: bool,
    /// `oneway` as a prefix before an op (instead of an `@oneway` annotation).
    pub corba_oneway_op: bool,
    /// `context (...)` Clause (Rule 123/124).
    pub corba_context: bool,

    // ========================================================================
    // §7.4.9 — Components/Homes/Ports
    // ========================================================================
    /// `component`/`provides`/`uses` (Rules 133+).
    pub corba_components: bool,
    /// `home`/`manages`/`primarykey`/`factory`/`finder` (Rules 145+).
    pub corba_homes: bool,
    /// `eventtype`/`emits`/`publishes`/`consumes`.
    pub corba_eventtypes: bool,
    /// `porttype`/`port`/`mirrorport`/`connector`.
    pub corba_ports: bool,

    // ========================================================================
    // §7.4.12 — Template Modules
    // ========================================================================
    /// `module M<typename T> { ... };` and template instantiation.
    pub corba_template_modules: bool,

    // ========================================================================
    // §7.4.1.3 — Native-Type
    // ========================================================================
    /// `native <ident>;` (Rule 61). Implemented; on by default,
    /// can be turned off for the plain-DDS profile.
    pub corba_native: bool,

    // ========================================================================
    // §7.3 — preprocessor stage 2
    // ========================================================================
    /// Function-like macros + token pasting `##` + `#if`/`#elif` with
    /// expression evaluation.
    pub preprocessor_full: bool,
    /// `#warning` and `#line` directives.
    pub preprocessor_warning_line: bool,

    // ========================================================================
    // Vendor extensions
    // ========================================================================
    /// RTI Connext: `@RTI_*` annotations, `keylist` directive,
    /// `//@key` legacy comments.
    pub vendor_rti: bool,
    /// OpenSplice (modern, post-2010): `#pragma keylist` plus
    /// more modern `@key` annotations.
    pub vendor_opensplice: bool,
    /// OpenSplice legacy (pre-2010): `#pragma keylist`,
    /// `#pragma DCPS_DATA_TYPE`, `#pragma DCPS_DATA_KEY`,
    /// `#pragma cats`, `#pragma genequality`, CORBA-style IDLs.
    /// Migration use case for reference customers.
    pub vendor_opensplice_legacy: bool,
    /// Cyclone DDS: `#pragma keylist`, `idlc`-specific annotations.
    pub vendor_cyclonedds: bool,
    /// FastDDS (eProsima): `@RTPS_*`-Annotations, `@key`-Variants.
    pub vendor_fastdds: bool,
}

impl Default for IdlFeatures {
    /// Default: DDS-Extensible profile (Plain DDS + XTypes + DDS-RPC,
    /// native type, no CORBA/CCM, no vendor extensions).
    fn default() -> Self {
        Self::dds_extensible()
    }
}

impl IdlFeatures {
    /// All feature flags off — strict plain-DDS profile without
    /// any extension.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            corba_value_types_full: false,
            corba_value_types_extras: false,
            corba_repository_ids: false,
            corba_import: false,
            corba_local_interface: false,
            corba_abstract_interface: false,
            corba_object_base: false,
            corba_oneway_op: false,
            corba_context: false,
            corba_components: false,
            corba_homes: false,
            corba_eventtypes: false,
            corba_ports: false,
            corba_template_modules: false,
            corba_native: false,
            preprocessor_full: false,
            preprocessor_warning_line: false,
            vendor_rti: false,
            vendor_opensplice: false,
            vendor_opensplice_legacy: false,
            vendor_cyclonedds: false,
            vendor_fastdds: false,
        }
    }

    /// All feature flags on — full IDL 4.2 incl. CORBA/CCM and
    /// all vendor extensions active. For migration use cases
    /// and spec-compliance audits.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            corba_value_types_full: true,
            corba_value_types_extras: true,
            corba_repository_ids: true,
            corba_import: true,
            corba_local_interface: true,
            corba_abstract_interface: true,
            corba_object_base: true,
            corba_oneway_op: true,
            corba_context: true,
            corba_components: true,
            corba_homes: true,
            corba_eventtypes: true,
            corba_ports: true,
            corba_template_modules: true,
            corba_native: true,
            preprocessor_full: true,
            preprocessor_warning_line: true,
            vendor_rti: true,
            vendor_opensplice: true,
            vendor_opensplice_legacy: true,
            vendor_cyclonedds: true,
            vendor_fastdds: true,
        }
    }

    /// Plain DDS — minimal DDS topic-type scope. Native type off,
    /// no CORBA/CCM, no vendor extensions.
    #[must_use]
    pub const fn dds_basic() -> Self {
        Self::none()
    }

    /// DDS + XTypes + DDS-RPC: native type active, everything else
    /// CORBA-specific off, no vendor extensions. Default for the
    /// ZeroDDS use case.
    #[must_use]
    pub const fn dds_extensible() -> Self {
        let mut f = Self::none();
        f.corba_native = true;
        f
    }

    /// CORBA-Full: all CORBA/CCM constructs active (value types full,
    /// repository IDs, components, homes, ports, template modules,
    /// all interface extensions). No vendor extensions.
    #[must_use]
    pub const fn corba_full() -> Self {
        Self {
            corba_value_types_full: true,
            corba_value_types_extras: true,
            corba_repository_ids: true,
            corba_import: true,
            corba_local_interface: true,
            corba_abstract_interface: true,
            corba_object_base: true,
            corba_oneway_op: true,
            corba_context: true,
            corba_components: true,
            corba_homes: true,
            corba_eventtypes: true,
            corba_ports: true,
            corba_template_modules: true,
            corba_native: true,
            preprocessor_full: true,
            preprocessor_warning_line: true,
            vendor_rti: false,
            vendor_opensplice: false,
            vendor_opensplice_legacy: false,
            vendor_cyclonedds: false,
            vendor_fastdds: false,
        }
    }

    /// §9.2.1 plain-CORBA profile: Core, Any, Interfaces-Basic,
    /// Interfaces-Full, Value-Types, CORBA-Specific-Interfaces,
    /// CORBA-Specific-Value-Types — 7 building blocks.
    #[must_use]
    pub const fn plain_corba() -> Self {
        Self {
            corba_value_types_full: true,
            corba_value_types_extras: true,
            corba_repository_ids: true,
            corba_import: true,
            corba_local_interface: true,
            corba_abstract_interface: true,
            corba_object_base: true,
            corba_oneway_op: true,
            corba_context: true,
            corba_components: false,
            corba_homes: false,
            corba_eventtypes: false,
            corba_ports: false,
            corba_template_modules: false,
            corba_native: true,
            preprocessor_full: true,
            preprocessor_warning_line: false,
            vendor_rti: false,
            vendor_opensplice: false,
            vendor_opensplice_legacy: false,
            vendor_cyclonedds: false,
            vendor_fastdds: false,
        }
    }

    /// §9.2.2 minimum-CORBA profile: 4 building blocks (Core,
    /// Interfaces-Basic, Interfaces-Full, CORBA-Specific-Interfaces).
    /// No Any, no value types.
    #[must_use]
    pub const fn minimum_corba() -> Self {
        Self {
            corba_value_types_full: false,
            corba_value_types_extras: false,
            corba_repository_ids: true,
            corba_import: true,
            corba_local_interface: true,
            corba_abstract_interface: false,
            corba_object_base: true,
            corba_oneway_op: true,
            corba_context: true,
            corba_components: false,
            corba_homes: false,
            corba_eventtypes: false,
            corba_ports: false,
            corba_template_modules: false,
            corba_native: true,
            preprocessor_full: true,
            preprocessor_warning_line: false,
            vendor_rti: false,
            vendor_opensplice: false,
            vendor_opensplice_legacy: false,
            vendor_cyclonedds: false,
            vendor_fastdds: false,
        }
    }

    /// §9.2.3 CCM profile: 9 building blocks
    /// (Plain-CORBA + Components-Basic + CCM-Specific).
    #[must_use]
    pub const fn ccm_profile() -> Self {
        let mut f = Self::plain_corba();
        f.corba_components = true;
        f.corba_homes = true;
        f.corba_eventtypes = true;
        f
    }

    /// §9.2.4 IDL3+ profile (CCM with Generic Interaction Support):
    /// 11 building blocks (CCM + Components-Ports/Connectors +
    /// Template-Modules).
    #[must_use]
    pub const fn ccm_with_gis() -> Self {
        let mut f = Self::ccm_profile();
        f.corba_ports = true;
        f.corba_template_modules = true;
        f
    }

    /// §9.3.3 RPC-over-DDS profile: 7 BBs/groups (Core, Extended-Data-
    /// Types, Anonymous, Interfaces-Basic, Annotations, General-Purpose,
    /// Interfaces-Group). More complex than pure DDS, because the interface BB
    /// is active.
    #[must_use]
    pub const fn rpc_over_dds() -> Self {
        let mut f = Self::dds_extensible();
        f.corba_oneway_op = true;
        f.preprocessor_full = true;
        f
    }

    /// OpenSplice legacy profile: CORBA-Full + OpenSplice legacy pragmas +
    /// preprocessor stage 2. Migration use case for a reference customer
    /// upgrading an old OpenSplice variant.
    #[must_use]
    pub const fn opensplice_legacy() -> Self {
        let mut f = Self::corba_full();
        f.vendor_opensplice_legacy = true;
        f.vendor_opensplice = true;
        f
    }

    /// OpenSplice modern profile: DDS-Extensible + OpenSplice pragmas.
    #[must_use]
    pub const fn opensplice_modern() -> Self {
        let mut f = Self::dds_extensible();
        f.vendor_opensplice = true;
        f.preprocessor_full = true;
        f
    }

    /// RTI Connext profile: DDS-Extensible + RTI annotations and
    /// the `keylist` pragma.
    #[must_use]
    pub const fn rti_connext() -> Self {
        let mut f = Self::dds_extensible();
        f.vendor_rti = true;
        f.preprocessor_full = true;
        f
    }

    /// Cyclone DDS profile: DDS-Extensible + Cyclone pragmas.
    #[must_use]
    pub const fn cyclonedds() -> Self {
        let mut f = Self::dds_extensible();
        f.vendor_cyclonedds = true;
        f
    }

    /// FastDDS profile: DDS-Extensible + FastDDS annotations.
    #[must_use]
    pub const fn fastdds() -> Self {
        let mut f = Self::dds_extensible();
        f.vendor_fastdds = true;
        f
    }

    /// Activate a vendor profile in addition to the current mask
    /// (additive).
    #[must_use]
    pub const fn with_vendor_rti(mut self) -> Self {
        self.vendor_rti = true;
        self
    }

    /// Additively activate `vendor_opensplice_legacy`.
    #[must_use]
    pub const fn with_opensplice_legacy(mut self) -> Self {
        self.vendor_opensplice_legacy = true;
        self.vendor_opensplice = true;
        self
    }

    /// Additively activate `vendor_cyclonedds`.
    #[must_use]
    pub const fn with_cyclonedds(mut self) -> Self {
        self.vendor_cyclonedds = true;
        self
    }

    /// Additively activate `vendor_fastdds`.
    #[must_use]
    pub const fn with_fastdds(mut self) -> Self {
        self.vendor_fastdds = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dds_extensible() {
        assert_eq!(IdlFeatures::default(), IdlFeatures::dds_extensible());
    }

    #[test]
    fn none_disables_everything() {
        let f = IdlFeatures::none();
        assert!(!f.corba_value_types_full);
        assert!(!f.corba_components);
        assert!(!f.corba_native);
        assert!(!f.vendor_rti);
        assert!(!f.preprocessor_full);
    }

    #[test]
    fn all_enables_everything() {
        let f = IdlFeatures::all();
        assert!(f.corba_value_types_full);
        assert!(f.corba_components);
        assert!(f.corba_template_modules);
        assert!(f.vendor_rti);
        assert!(f.vendor_opensplice_legacy);
        assert!(f.preprocessor_full);
    }

    #[test]
    fn dds_basic_has_no_corba() {
        let f = IdlFeatures::dds_basic();
        assert!(!f.corba_value_types_full);
        assert!(!f.corba_native);
        assert!(!f.corba_components);
    }

    #[test]
    fn dds_extensible_has_native_only() {
        let f = IdlFeatures::dds_extensible();
        assert!(f.corba_native);
        assert!(!f.corba_value_types_full);
        assert!(!f.corba_components);
        assert!(!f.vendor_rti);
    }

    #[test]
    fn corba_full_has_all_corba_no_vendor() {
        let f = IdlFeatures::corba_full();
        assert!(f.corba_value_types_full);
        assert!(f.corba_components);
        assert!(f.corba_template_modules);
        assert!(f.preprocessor_full);
        assert!(!f.vendor_rti);
        assert!(!f.vendor_opensplice_legacy);
    }

    #[test]
    fn opensplice_legacy_includes_corba_and_legacy_pragmas() {
        let f = IdlFeatures::opensplice_legacy();
        assert!(f.corba_value_types_full);
        assert!(f.corba_components);
        assert!(f.vendor_opensplice_legacy);
        assert!(f.vendor_opensplice);
    }

    #[test]
    fn opensplice_modern_is_extensible_plus_pragmas() {
        let f = IdlFeatures::opensplice_modern();
        assert!(f.corba_native);
        assert!(f.vendor_opensplice);
        assert!(!f.vendor_opensplice_legacy);
        assert!(!f.corba_components);
    }

    #[test]
    fn rti_connext_profile() {
        let f = IdlFeatures::rti_connext();
        assert!(f.corba_native);
        assert!(f.vendor_rti);
        assert!(!f.vendor_opensplice);
    }

    #[test]
    fn cyclonedds_profile() {
        let f = IdlFeatures::cyclonedds();
        assert!(f.corba_native);
        assert!(f.vendor_cyclonedds);
        assert!(!f.vendor_rti);
    }

    #[test]
    fn fastdds_profile() {
        let f = IdlFeatures::fastdds();
        assert!(f.corba_native);
        assert!(f.vendor_fastdds);
        assert!(!f.vendor_opensplice);
    }

    #[test]
    fn additive_with_vendor_rti() {
        let f = IdlFeatures::dds_extensible().with_vendor_rti();
        assert!(f.vendor_rti);
        assert!(f.corba_native);
    }

    #[test]
    fn additive_with_opensplice_legacy_sets_both() {
        let f = IdlFeatures::dds_extensible().with_opensplice_legacy();
        assert!(f.vendor_opensplice);
        assert!(f.vendor_opensplice_legacy);
    }

    // §9 profile constructors

    #[test]
    fn plain_corba_profile_matches_spec() {
        // §9.2.1: 7 BBs (Core, Any, Interfaces-Basic+Full,
        // Value-Types, CORBA-Specific-Interfaces+Value-Types).
        let f = IdlFeatures::plain_corba();
        assert!(f.corba_value_types_full);
        assert!(f.corba_repository_ids);
        assert!(f.corba_native);
        assert!(!f.corba_components);
        assert!(!f.corba_homes);
        assert!(!f.corba_template_modules);
    }

    #[test]
    fn minimum_corba_profile_matches_spec() {
        // §9.2.2: 4 BBs (Core, Interfaces-Basic+Full,
        // CORBA-Specific-Interfaces). No Any, no value types.
        let f = IdlFeatures::minimum_corba();
        assert!(!f.corba_value_types_full);
        assert!(!f.corba_value_types_extras);
        assert!(f.corba_repository_ids);
        assert!(f.corba_oneway_op);
        assert!(!f.corba_components);
    }

    #[test]
    fn ccm_profile_matches_spec() {
        // §9.2.3: Plain-CORBA + Components-Basic + CCM-Specific.
        let f = IdlFeatures::ccm_profile();
        assert!(f.corba_value_types_full);
        assert!(f.corba_components);
        assert!(f.corba_homes);
        assert!(f.corba_eventtypes);
        assert!(!f.corba_ports);
        assert!(!f.corba_template_modules);
    }

    #[test]
    fn ccm_with_gis_profile_matches_spec() {
        // §9.2.4 (IDL3+): CCM + Ports/Connectors + Template-Modules.
        let f = IdlFeatures::ccm_with_gis();
        assert!(f.corba_components);
        assert!(f.corba_ports);
        assert!(f.corba_template_modules);
    }

    #[test]
    fn rpc_over_dds_profile_matches_spec() {
        // §9.3.3: DDS-Extensible + Interfaces-Basic-Annotations.
        let f = IdlFeatures::rpc_over_dds();
        assert!(f.corba_native);
        assert!(f.corba_oneway_op);
        assert!(!f.corba_components);
    }

    #[test]
    fn plain_dds_profile_matches_spec_exactly() {
        // §9.3.1: Plain-DDS = Core + Anonymous Types only.
        let f = IdlFeatures::dds_basic();
        assert!(!f.corba_native);
        assert!(!f.corba_components);
        assert!(!f.corba_value_types_full);
        assert!(!f.vendor_rti);
        assert!(!f.vendor_cyclonedds);
    }

    #[test]
    fn dds_extensible_excludes_corba_components() {
        // §9.3.2: DDS-Extensible activates annotation groups, but
        // no CORBA components.
        let f = IdlFeatures::dds_extensible();
        assert!(f.corba_native);
        assert!(!f.corba_components);
        assert!(!f.corba_homes);
        assert!(!f.corba_template_modules);
    }
}
