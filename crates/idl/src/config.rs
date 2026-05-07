// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Konfiguration fuer die Public-`parse()`-API (T5.5).
//!
//! [`ParserConfig`] kombiniert die Grammar-Auswahl ([`IdlVersion`]),
//! den Strenge-Grad ([`CompatMode`]) und Vendor-spezifische Erweiterungen
//! ([`VendorExt`]). Default ist OMG-konform IDL 4.2.
//!
//! Vendor-Extensions sind in Phase 0 leer (Hooks nur). Konkrete
//! RTI-/OpenSplice-Deltas folgen mit Task 6.4 (`grammar::deltas`).

use crate::features::IdlFeatures;
use crate::grammar::IdlVersion;

/// Konfiguration fuer den Top-Level-Parser.
///
/// # Beispiel
/// ```
/// use zerodds_idl::config::ParserConfig;
/// let cfg = ParserConfig::default();
/// assert_eq!(cfg.version, zerodds_idl::grammar::IdlVersion::V4_2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParserConfig {
    /// Welche IDL-Spec-Version geparst werden soll. Default: V4_2.
    pub version: IdlVersion,
    /// Wie streng wird das OMG-Grammatik-Regelwerk durchgesetzt.
    pub compat: CompatMode,
    /// Welche Vendor-Extension(s) zusaetzlich aktiv sind. Legacy-Hook;
    /// pro-Konstrukt-Aktivierung steuert jetzt [`IdlFeatures`].
    pub vendor: VendorExt,
    /// Building-Block-Profil (§2.3 + §9). Steuert welche Spec-Sektionen
    /// (DDS-Basic, DDS-Extensible, Full mit CORBA/CCM) als gueltig
    /// angesehen werden. Default `DdsExtensible` reflektiert den
    /// ZeroDDS-Use-Case (DDS-Stack mit XTypes).
    pub profile: Profile,
    /// Feature-Flag-Maske fuer pro-Konstrukt-Aktivierung. Default
    /// folgt dem `profile`-Feld; User kann zusaetzlich einzelne Flags
    /// togglen (z.B. CORBA-Template-Modules in DDS-Extensible-Profil
    /// fuer DDS-RPC-Service-Templates).
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
    /// Convenience: strikte OMG-IDL-4.2-Konfiguration (== `Default`).
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

    /// Convenience: vendor-pragmatische Konfiguration mit lockerer
    /// Strict-Auslegung (z.B. leere `<member_list>` zugelassen).
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

    /// Convenience: Plain-DDS-Profil (§2.3a) — minimal DDS-Stack ohne
    /// XTypes-Erweiterungen, ohne CORBA/CCM-Konstrukte. Strict.
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

    /// Convenience: Full-Profil — IDL 4.2 mit allen Building-Blocks
    /// (inkl. CORBA-Profile, falls in der Grammar aktiviert).
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

    /// OpenSplice-Legacy-Migration-Profil: CORBA-Full + Legacy-Pragmas.
    /// Fuer Referenz-Kunden, die uralte OpenSplice-IDLs upgraden.
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

    /// OpenSplice-Modern-Profil.
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

    /// RTI-Connext-Profil.
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

    /// Cyclone-DDS-Profil.
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

    /// FastDDS-Profil.
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

/// Wie streng der Parser die OMG-Spec auslegt.
///
/// In Phase 0 hat das Setting noch keinen Verhaltens-Effekt — die Grammar
/// ist hardgecodet vendor-pragmatisch (leere member_list etc.). Mit T6.x
/// (`grammar::deltas`) wird `Strict` die Pragmatik-Lockerungen abdrehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompatMode {
    /// OMG-konform mit allen `+`/`*`-Quantor-Pflichten.
    #[default]
    Strict,
    /// Vendor-pragmatisch: leere Listen wo Spec `+` fordert, weiche
    /// Disambiguation an Stellen mit ueblichen Vendor-Erweiterungen.
    Pragmatic,
}

/// Building-Block-Profil (§2.3 + §9 — IDL 4.2 Conformance).
///
/// IDL 4.2 ist modular aus Building-Blocks aufgebaut. Diese Achse
/// selektiert welche Sektionen aktiv sind:
///
/// - **`DdsBasic`** — minimales DDS-Profil: nur `module`/`struct`/`union`/
///   `enum`/`typedef`/`const`/`exception` plus DDS-Annotations. Kein
///   `interface`, kein `valuetype`, kein XTypes-Map/Bitset/Bitmask, keine
///   CORBA/CCM-Konstrukte. Spec-Konformitaets-Ziel fuer schlanke
///   Topic-Type-IDLs.
/// - **`DdsExtensible`** — DDS plus XTypes (§7.4.13: map/bitset/bitmask/
///   any), Annotation-Defs, `interface` fuer DDS-RPC. Default fuer den
///   ZeroDDS-Use-Case.
/// - **`Full`** — alle Building-Blocks der IDL 4.2 inkl. (zukuenftiger)
///   CORBA-Profile (Components/Homes/Template-Modules). Heute identisch
///   zu `DdsExtensible`, weil CORBA-Konstrukte noch nicht implementiert
///   sind (siehe `idl-4.2.open.md` Sektion 7).
///
/// Profil ist *deklarativ* — die Grammar selbst ist
/// monolithisch und akzeptiert immer den `DdsExtensible`-Umfang. Der
/// Profil-Selektor wird mit CORBA-Konstrukten greifen
/// (selektive Production-Aktivierung im `GrammarComposer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Profile {
    /// Plain-DDS — minimaler DDS-Topic-Type-Umfang.
    DdsBasic,
    /// DDS + XTypes + DDS-RPC (Default fuer DDS-Stacks).
    #[default]
    DdsExtensible,
    /// Full IDL 4.2 inkl. (zukuenftiger) CORBA/CCM-Profile.
    Full,
}

/// Aktive Vendor-Extension fuer den Parser-Lauf.
///
/// Mit der Einfuehrung der Feature-Flag-Achse [`IdlFeatures`] ist dieser
/// Enum primaer ein UI-/Routing-Hint; die eigentliche Aktivierung der
/// Vendor-Konstrukte steuert das `features`-Feld.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VendorExt {
    /// Keine Vendor-Erweiterung. Default.
    #[default]
    None,
    /// RTI Connext DDS.
    Rti,
    /// OpenSplice (modern oder legacy — siehe `IdlFeatures::vendor_*`).
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
    // §2.3 + §9 — Profile-Selektor (§10 Open-List)
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
        // strict_4_2 + pragmatic_4_2 muessen identisch zu Default-Profil
        // bleiben (kein implizites Upgrade auf Full).
        assert_eq!(ParserConfig::strict_4_2().profile, Profile::DdsExtensible);
        assert_eq!(
            ParserConfig::pragmatic_4_2().profile,
            Profile::DdsExtensible
        );
    }
}
