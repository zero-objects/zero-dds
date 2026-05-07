// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC-Discovery-Extensions (Spec §7.6.2.x).
//!
//! `PublicationBuiltinTopicDataExt` und `SubscriptionBuiltinTopicDataExt`
//! erweitern die Standard-DCPS-Discovery-Daten um RPC-Service-
//! Identitaet (Service-Name, Mapping-Profil, Topic-Aliases fuer
//! Inheritance).
//!
//! # Spec-Mapping
//!
//! * **§7.6.2.1.1** [`PublicationBuiltinTopicDataExt`] — extended
//!   Publication-Data mit RPC-Felder.
//! * **§7.6.2.1.2** [`SubscriptionBuiltinTopicDataExt`] — analog.
//! * **§7.6.2.2.1** [`client_matches_service`] — Client-Matching-Helper.
//! * **§7.6.2.2.2** [`service_matches_client`] — Service-Matching-Helper.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Spec §7.6.2.1.1 Extension der Standard-PublicationBuiltinTopicData.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublicationBuiltinTopicDataExt {
    /// Service-Name aus IDL-`@service`-Annotation.
    pub service_name: String,
    /// Mapping-Profil ("Basic" oder "Enhanced").
    pub mapping_profile: ServiceMappingProfile,
    /// Topic-Aliases fuer Interface-Inheritance (Spec §7.5.1.2.6).
    pub topic_aliases: Vec<String>,
}

/// Spec §7.6.2.1.2 Extension der Standard-SubscriptionBuiltinTopicData.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionBuiltinTopicDataExt {
    /// Service-Name aus IDL-`@service`-Annotation.
    pub service_name: String,
    /// Mapping-Profil.
    pub mapping_profile: ServiceMappingProfile,
    /// Topic-Aliases fuer Interface-Inheritance.
    pub topic_aliases: Vec<String>,
}

/// Service-Mapping-Profil (Spec §2.1 + §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceMappingProfile {
    /// Basic-Mapping (default).
    #[default]
    Basic,
    /// Enhanced-Mapping mit X-Types-Aliases.
    Enhanced,
}

/// Spec §7.6.2.2.1: Client-Side-Matching via extended publication data.
///
/// Client matched einen Service wenn:
/// 1. Service-Name uebereinstimmt.
/// 2. Mapping-Profil kompatibel (Enhanced akzeptiert Basic-Subset).
#[must_use]
pub fn client_matches_service(
    client_pub_data: &PublicationBuiltinTopicDataExt,
    service_sub_data: &SubscriptionBuiltinTopicDataExt,
) -> bool {
    if client_pub_data.service_name != service_sub_data.service_name {
        return false;
    }
    profile_compatible(
        client_pub_data.mapping_profile,
        service_sub_data.mapping_profile,
    )
}

/// Spec §7.6.2.2.2: Service-Side-Matching analog.
#[must_use]
pub fn service_matches_client(
    service_pub_data: &PublicationBuiltinTopicDataExt,
    client_sub_data: &SubscriptionBuiltinTopicDataExt,
) -> bool {
    if service_pub_data.service_name != client_sub_data.service_name {
        return false;
    }
    profile_compatible(
        service_pub_data.mapping_profile,
        client_sub_data.mapping_profile,
    )
}

/// Profile-Kompatibilitaet: gleicher Profile-Typ matched immer; Basic
/// und Enhanced sind nicht direkt cross-kompatibel (Spec §2.1: "must
/// use the same Service Mapping").
fn profile_compatible(a: ServiceMappingProfile, b: ServiceMappingProfile) -> bool {
    a == b
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn pub_data(name: &str, profile: ServiceMappingProfile) -> PublicationBuiltinTopicDataExt {
        PublicationBuiltinTopicDataExt {
            service_name: name.into(),
            mapping_profile: profile,
            topic_aliases: Vec::new(),
        }
    }

    fn sub_data(name: &str, profile: ServiceMappingProfile) -> SubscriptionBuiltinTopicDataExt {
        SubscriptionBuiltinTopicDataExt {
            service_name: name.into(),
            mapping_profile: profile,
            topic_aliases: Vec::new(),
        }
    }

    #[test]
    fn client_matches_service_with_same_name_and_profile() {
        let p = pub_data("Calc", ServiceMappingProfile::Basic);
        let s = sub_data("Calc", ServiceMappingProfile::Basic);
        assert!(client_matches_service(&p, &s));
    }

    #[test]
    fn client_does_not_match_service_with_different_name() {
        let p = pub_data("Calc", ServiceMappingProfile::Basic);
        let s = sub_data("Other", ServiceMappingProfile::Basic);
        assert!(!client_matches_service(&p, &s));
    }

    #[test]
    fn client_does_not_match_service_with_different_profile() {
        // Spec §2.1: Client+Service muessen dasselbe Mapping nutzen.
        let p = pub_data("Calc", ServiceMappingProfile::Basic);
        let s = sub_data("Calc", ServiceMappingProfile::Enhanced);
        assert!(!client_matches_service(&p, &s));
    }

    #[test]
    fn service_matches_client_symmetric() {
        let p = pub_data("Calc", ServiceMappingProfile::Enhanced);
        let s = sub_data("Calc", ServiceMappingProfile::Enhanced);
        assert!(service_matches_client(&p, &s));
    }

    #[test]
    fn topic_aliases_propagated_in_extended_data() {
        let p = PublicationBuiltinTopicDataExt {
            service_name: "Inherited".into(),
            mapping_profile: ServiceMappingProfile::Enhanced,
            topic_aliases: alloc::vec!["BaseInterface_Request".into()],
        };
        assert_eq!(p.topic_aliases.len(), 1);
        assert_eq!(p.topic_aliases[0], "BaseInterface_Request");
    }
}
