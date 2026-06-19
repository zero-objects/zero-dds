// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Configuration for the public `parse()` API (T5.5).
//!
//! [`ParserConfig`] combines the grammar selection ([`IdlVersion`]),
//! the strictness level ([`CompatMode`]) and vendor-specific extensions
//! ([`VendorExt`]). The default is OMG-compliant IDL 4.2.
//!
//! Vendor extensions are empty in phase 0 (hooks only). Concrete
//! RTI/OpenSplice deltas follow with Task 6.4 (`grammar::deltas`).

use crate::features::IdlFeatures;
use crate::grammar::IdlVersion;

/// Configuration for the top-level parser.
///
/// # Example
/// ```
/// use zerodds_idl::config::ParserConfig;
/// let cfg = ParserConfig::default();
/// assert_eq!(cfg.version, zerodds_idl::grammar::IdlVersion::V4_2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParserConfig {
    /// Which IDL spec version to parse. Default: V4_2.
    pub version: IdlVersion,
    /// How strictly the OMG grammar rule set is enforced.
    pub compat: CompatMode,
    /// Which vendor extension(s) are additionally active. Legacy hook;
    /// per-construct activation is now controlled by [`IdlFeatures`].
    pub vendor: VendorExt,
    /// Building-block profile (§2.3 + §9). Controls which spec sections
    /// (DDS-Basic, DDS-Extensible, Full with CORBA/CCM) are considered
    /// valid. The default `DdsExtensible` reflects the
    /// ZeroDDS use case (DDS stack with XTypes).
    pub profile: Profile,
    /// Feature-flag mask for per-construct activation. The default
    /// follows the `profile` field; the user can additionally toggle
    /// individual flags (e.g. CORBA template modules in the DDS-Extensible profile
    /// for DDS-RPC service templates).
    pub features: IdlFeatures,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::dds_extensible(),
        }
    }
}

impl ParserConfig {
    /// Convenience: strict OMG-IDL-4.2 configuration (== `Default`).
    #[must_use]
    pub const fn strict_4_2() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::dds_extensible(),
        }
    }

    /// Convenience: vendor-pragmatic configuration with a looser
    /// strict interpretation (e.g. empty `<member_list>` allowed).
    #[must_use]
    pub const fn pragmatic_4_2() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Pragmatic,
            vendor: VendorExt::None,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::dds_extensible(),
        }
    }

    /// Convenience: plain-DDS profile (§2.3a) — minimal DDS stack without
    /// XTypes extensions, without CORBA/CCM constructs. Strict.
    #[must_use]
    pub const fn plain_dds_4_2() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::DdsBasic,
            features: IdlFeatures::dds_basic(),
        }
    }

    /// Convenience: full profile — IDL 4.2 with all building blocks
    /// (incl. CORBA profiles, if activated in the grammar).
    #[must_use]
    pub const fn full_4_2() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::Full,
            features: IdlFeatures::corba_full(),
        }
    }

    /// OpenSplice legacy migration profile: CORBA-Full + legacy pragmas.
    /// For reference customers upgrading ancient OpenSplice IDLs.
    #[must_use]
    pub const fn opensplice_legacy() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Pragmatic,
            vendor: VendorExt::OpenSplice,
            profile: Profile::Full,
            features: IdlFeatures::opensplice_legacy(),
        }
    }

    /// OpenSplice modern profile.
    #[must_use]
    pub const fn opensplice_modern() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::OpenSplice,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::opensplice_modern(),
        }
    }

    /// RTI Connext profile.
    #[must_use]
    pub const fn rti_connext() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::Rti,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::rti_connext(),
        }
    }

    /// Cyclone DDS profile.
    #[must_use]
    pub const fn cyclonedds() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::cyclonedds(),
        }
    }

    /// FastDDS profile.
    #[must_use]
    pub const fn fastdds() -> Self {
        Self {
            version: IdlVersion::V4_2,
            compat: CompatMode::Strict,
            vendor: VendorExt::None,
            profile: Profile::DdsExtensible,
            features: IdlFeatures::fastdds(),
        }
    }
}

/// How strictly the parser interprets the OMG spec.
///
/// In phase 0 the setting has no behavioral effect yet — the grammar
/// is hard-coded vendor-pragmatic (empty member_list etc.). With T6.x
/// (`grammar::deltas`) `Strict` will turn off the pragmatic relaxations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompatMode {
    /// OMG-compliant with all `+`/`*` quantifier obligations.
    #[default]
    Strict,
    /// Vendor-pragmatic: empty lists where the spec requires `+`, soft
    /// disambiguation at points with common vendor extensions.
    Pragmatic,
}

/// Building-block profile (§2.3 + §9 — IDL 4.2 conformance).
///
/// IDL 4.2 is built modularly from building blocks. This axis
/// selects which sections are active:
///
/// - **`DdsBasic`** — minimal DDS profile: only `module`/`struct`/`union`/
///   `enum`/`typedef`/`const`/`exception` plus DDS annotations. No
///   `interface`, no `valuetype`, no XTypes map/bitset/bitmask, no
///   CORBA/CCM constructs. Spec-conformance target for lean
///   topic-type IDLs.
/// - **`DdsExtensible`** — DDS plus XTypes (§7.4.13: map/bitset/bitmask/
///   any), annotation defs, `interface` for DDS-RPC. Default for the
///   ZeroDDS use case.
/// - **`Full`** — all building blocks of IDL 4.2 incl. (future)
///   CORBA profiles (components/homes/template-modules). Today identical
///   to `DdsExtensible`, because CORBA constructs are not yet implemented
///   (see `idl-4.2.open.md` section 7).
///
/// The profile is *declarative* — the grammar itself is
/// monolithic and always accepts the `DdsExtensible` scope. The
/// profile selector will take effect with CORBA constructs
/// (selective production activation in the `GrammarComposer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Profile {
    /// Plain DDS — minimal DDS topic-type scope.
    DdsBasic,
    /// DDS + XTypes + DDS-RPC (default for DDS stacks).
    #[default]
    DdsExtensible,
    /// Full IDL 4.2 incl. (future) CORBA/CCM profiles.
    Full,
}

/// Active vendor extension for the parser run.
///
/// With the introduction of the feature-flag axis [`IdlFeatures`], this
/// enum is primarily a UI/routing hint; the actual activation of the
/// vendor constructs is controlled by the `features` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VendorExt {
    /// No vendor extension. Default.
    #[default]
    None,
    /// RTI Connext DDS.
    Rti,
    /// OpenSplice (modern or legacy — see `IdlFeatures::vendor_*`).
    OpenSplice,
    /// Cyclone DDS.
    CycloneDds,
    /// FastDDS (eProsima).
    FastDds,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn default_is_strict_4_2_no_vendor() {
        let cfg = ParserConfig::default();
        assert_eq!(cfg.version, IdlVersion::V4_2);
        assert_eq!(cfg.compat, CompatMode::Strict);
        assert_eq!(cfg.vendor, VendorExt::None);
    }

    #[test]
    fn strict_4_2_constructor_matches_default() {
        assert_eq!(ParserConfig::strict_4_2(), ParserConfig::default());
    }

    #[test]
    fn pragmatic_4_2_keeps_version_and_no_vendor() {
        let cfg = ParserConfig::pragmatic_4_2();
        assert_eq!(cfg.version, IdlVersion::V4_2);
        assert_eq!(cfg.compat, CompatMode::Pragmatic);
        assert_eq!(cfg.vendor, VendorExt::None);
    }

    #[test]
    fn config_is_copyable() {
        let cfg = ParserConfig::default();
        let copy = cfg;
        assert_eq!(cfg, copy);
    }

    #[test]
    fn compat_mode_default_is_strict() {
        assert_eq!(CompatMode::default(), CompatMode::Strict);
    }

    #[test]
    fn vendor_ext_default_is_none() {
        assert_eq!(VendorExt::default(), VendorExt::None);
    }

    #[test]
    fn config_can_be_customized_field_by_field() {
        let cfg = ParserConfig {
            version: IdlVersion::V4_0,
            compat: CompatMode::Pragmatic,
            vendor: VendorExt::Rti,
            profile: Profile::Full,
            features: IdlFeatures::corba_full(),
        };
        assert_eq!(cfg.version, IdlVersion::V4_0);
        assert_eq!(cfg.compat, CompatMode::Pragmatic);
        assert_eq!(cfg.vendor, VendorExt::Rti);
        assert_eq!(cfg.profile, Profile::Full);
        assert!(cfg.features.corba_components);
    }

    #[test]
    fn opensplice_legacy_constructor_enables_corba_and_pragmas() {
        let cfg = ParserConfig::opensplice_legacy();
        assert!(cfg.features.vendor_opensplice_legacy);
        assert!(cfg.features.corba_value_types_full);
        assert!(cfg.features.corba_components);
        assert_eq!(cfg.vendor, VendorExt::OpenSplice);
    }

    #[test]
    fn rti_connext_constructor() {
        let cfg = ParserConfig::rti_connext();
        assert!(cfg.features.vendor_rti);
        assert_eq!(cfg.vendor, VendorExt::Rti);
        assert!(!cfg.features.corba_components);
    }

    #[test]
    fn cyclonedds_constructor() {
        let cfg = ParserConfig::cyclonedds();
        assert!(cfg.features.vendor_cyclonedds);
        assert!(!cfg.features.vendor_rti);
    }

    #[test]
    fn fastdds_constructor() {
        let cfg = ParserConfig::fastdds();
        assert!(cfg.features.vendor_fastdds);
        assert!(!cfg.features.vendor_opensplice);
    }

    // -----------------------------------------------------------------
    // §2.3 + §9 — profile selector (§10 open-list)
    // -----------------------------------------------------------------

    #[test]
    fn default_profile_is_dds_extensible() {
        assert_eq!(ParserConfig::default().profile, Profile::DdsExtensible);
        assert_eq!(Profile::default(), Profile::DdsExtensible);
    }

    #[test]
    fn plain_dds_profile_constructor() {
        let cfg = ParserConfig::plain_dds_4_2();
        assert_eq!(cfg.profile, Profile::DdsBasic);
        assert_eq!(cfg.compat, CompatMode::Strict);
        assert_eq!(cfg.vendor, VendorExt::None);
    }

    #[test]
    fn full_profile_constructor() {
        let cfg = ParserConfig::full_4_2();
        assert_eq!(cfg.profile, Profile::Full);
    }

    #[test]
    fn strict_pragmatic_use_extensible_profile() {
        // strict_4_2 + pragmatic_4_2 must stay identical to the default profile
        // (no implicit upgrade to Full).
        assert_eq!(ParserConfig::strict_4_2().profile, Profile::DdsExtensible);
        assert_eq!(
            ParserConfig::pragmatic_4_2().profile,
            Profile::DdsExtensible
        );
    }
}
