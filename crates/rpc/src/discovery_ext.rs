// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC discovery extensions (Spec §7.6.2.x).
//!
//! `PublicationBuiltinTopicDataExt` and `SubscriptionBuiltinTopicDataExt`
//! extend the standard DCPS discovery data with RPC service
//! identity (service name, mapping profile, topic aliases for
//! inheritance).
//!
//! # Spec mapping
//!
//! * **§7.6.2.1.1** [`PublicationBuiltinTopicDataExt`] — extended
//!   publication data with RPC fields.
//! * **§7.6.2.1.2** [`SubscriptionBuiltinTopicDataExt`] — analogous.
//! * **§7.6.2.2.1** [`client_matches_service`] — client-matching helper.
//! * **§7.6.2.2.2** [`service_matches_client`] — service-matching helper.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Spec §7.6.2.1.1 extension of the standard PublicationBuiltinTopicData.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublicationBuiltinTopicDataExt {
    /// Service name from the IDL `@service` annotation.
    pub service_name: String,
    /// Mapping profile ("Basic" or "Enhanced").
    pub mapping_profile: ServiceMappingProfile,
    /// Topic aliases for interface inheritance (Spec §7.5.1.2.6).
    pub topic_aliases: Vec<String>,
}

/// Spec §7.6.2.1.2 extension of the standard SubscriptionBuiltinTopicData.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionBuiltinTopicDataExt {
    /// Service name from the IDL `@service` annotation.
    pub service_name: String,
    /// Mapping profile.
    pub mapping_profile: ServiceMappingProfile,
    /// Topic aliases for interface inheritance.
    pub topic_aliases: Vec<String>,
}

/// Service mapping profile (Spec §2.1 + §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceMappingProfile {
    /// Basic mapping (default).
    #[default]
    Basic,
    /// Enhanced mapping with X-Types aliases.
    Enhanced,
}

/// Spec §7.6.2.2.1: client-side matching via extended publication data.
///
/// A client matches a service if:
/// 1. The service name matches.
/// 2. The mapping profile is compatible (Enhanced accepts the Basic subset).
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

/// Spec §7.6.2.2.2: service-side matching analogously.
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

/// Profile compatibility: the same profile type always matches; Basic
/// and Enhanced are not directly cross-compatible (Spec §2.1: "must
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
        // Spec §2.1: client + service must use the same mapping.
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
