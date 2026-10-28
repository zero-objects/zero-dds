// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lightweight CCM Profile restrictions — spec §13 (p. 273-280).
//!
//! §13 does not describe a smaller runtime; it describes which parts of
//! the extended model a Lightweight container refuses. Spec §13.1
//! (p. 273): "Operations that are termed 'disabled' in this conformance
//! point are still part of the associated interface, but implementations
//! may raise either BAD_OPERATION or NO_IMPLEMENT exceptions when they
//! are invoked." So the restriction is not an IDL edit — the equivalent
//! interface keeps every operation, and the repository IDs keep their
//! meaning — it is an answer the container gives at invocation time.
//!
//! This module is that answer. [`disabled_by`] maps an operation name to
//! the §13 subsection that disables it; [`rejection_for`] says which
//! exception the container raises, including the one §13.1 mandates
//! rather than permits:
//!
//! > "In the specific case of the disabled 'same_component' operation,
//! > on the Navigation interface, throwing an exception of NO_IMPLEMENT
//! > is required, with a minor code of IS_LIGHTWEIGHT_COMPONENT."
//!
//! The exclusions that are not operations — component categories
//! (§13.2), CIDL and its compositions (§13.2 / §13.4 / §13.10), the
//! segment multiplicity of the CIF metamodel (§13.4) — are the other
//! three entry points: [`category_supported`], [`composition_exclusion`]
//! and [`MAX_SEGMENTS_PER_IMPLEMENTATION`].

use crate::cidl::{Composition, CompositionCategory};

/// The §13 subsection that excludes a feature from the Lightweight
/// profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LwccmExclusion {
    /// §13.2 (p. 274) — persistence: primary keys, storage homes and
    /// types, entity container API types, the durable usage model.
    Persistence,
    /// §13.3 (p. 276) — introspection, navigation, and the type-specific
    /// port operations that are redundant with the generic ones.
    Introspection,
    /// §13.4 (p. 277) — segmentation, compositions, executor locators.
    Segmentation,
    /// §13.5 (p. 278) — transaction policies and the transaction
    /// operations of `CCMContext`.
    Transactions,
    /// §13.6 (p. 278) — security policy declarations and the caller
    /// credential operations of `CCMContext`.
    Security,
    /// §13.7 (p. 279) — the `Configurator` and `StandardConfigurator`
    /// interfaces and `HomeConfiguration::set_configurator`.
    Configurators,
    /// §13.8 (p. 279) — proxy homes and `ProxyHomeRegistration`.
    ProxyHomes,
    /// §13.9 (p. 279) — the `HomeFinder` interface, the
    /// `ComponentHomeFinder` initial reference, and finder operations.
    HomeFinders,
    /// §13.10 (p. 280) — CIDL itself, removed as a consequence of
    /// excluding persistence and segmentation.
    Cidl,
    /// §13.10 (p. 280) — direct access to the POA from a component
    /// implementation.
    PoaAccess,
}

impl LwccmExclusion {
    /// The §13 subsection that carries this exclusion.
    #[must_use]
    pub fn spec_section(self) -> &'static str {
        match self {
            Self::Persistence => "13.2",
            Self::Introspection => "13.3",
            Self::Segmentation => "13.4",
            Self::Transactions => "13.5",
            Self::Security => "13.6",
            Self::Configurators => "13.7",
            Self::ProxyHomes => "13.8",
            Self::HomeFinders => "13.9",
            Self::Cidl | Self::PoaAccess => "13.10",
        }
    }
}

/// The minor code §13.1 (p. 273) names for the disabled
/// `Navigation::same_component`.
///
/// The specification names the code and does not number it; a number
/// invented here would be indistinguishable from an assigned one, so
/// the name is what this carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LwccmMinorCode {
    /// `IS_LIGHTWEIGHT_COMPONENT`.
    IsLightweightComponent,
}

/// What a Lightweight container raises for a disabled operation —
/// spec §13.1 (p. 273).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LwccmRejection {
    /// `CORBA::BAD_OPERATION`. §13.1 permits either this or
    /// `NO_IMPLEMENT` for every disabled operation but one.
    BadOperation,
    /// `CORBA::NO_IMPLEMENT` with the given minor code. §13.1 requires
    /// this form for `same_component`.
    NoImplement(LwccmMinorCode),
}

/// The operations the §13.2 to §13.9 tables disable by name.
///
/// Every entry is one row of one table: the operation the row names and
/// the subsection the row belongs to.
const DISABLED_OPERATIONS: [(&str, LwccmExclusion); 22] = [
    // §13.2 (p. 275) — "Disable get_persistence operation from
    // CCM2Context interface".
    ("get_persistence", LwccmExclusion::Persistence),
    // §13.3 (p. 276) — Navigation.
    ("get_all_facets", LwccmExclusion::Introspection),
    ("get_named_facets", LwccmExclusion::Introspection),
    ("same_component", LwccmExclusion::Introspection),
    // §13.3 (p. 276) — Receptacles.
    ("get_connections", LwccmExclusion::Introspection),
    ("get_all_receptacles", LwccmExclusion::Introspection),
    ("get_named_receptacles", LwccmExclusion::Introspection),
    // §13.3 (p. 276) — Events.
    ("get_all_consumers", LwccmExclusion::Introspection),
    ("get_named_consumers", LwccmExclusion::Introspection),
    ("get_all_emitters", LwccmExclusion::Introspection),
    ("get_named_emitters", LwccmExclusion::Introspection),
    ("get_all_publishers", LwccmExclusion::Introspection),
    ("get_named_publishers", LwccmExclusion::Introspection),
    // §13.3 (p. 277) — CCMHome and CCMObject.
    ("get_component_def", LwccmExclusion::Introspection),
    ("get_home_def", LwccmExclusion::Introspection),
    ("get_primary_key", LwccmExclusion::Introspection),
    ("get_all_ports", LwccmExclusion::Introspection),
    // §13.5 (p. 278) — CCMContext.
    ("get_rollback_only", LwccmExclusion::Transactions),
    ("get_user_transaction", LwccmExclusion::Transactions),
    ("set_rollback_only", LwccmExclusion::Transactions),
    // §13.6 (p. 278) — CCMContext.
    //
    // `get_caller_principal` and `is_caller_in_role` are the two
    // operations the security row names. `is_caller_in_role` shares the
    // name space with nothing else, so both are exact matches.
    ("is_caller_in_role", LwccmExclusion::Security),
    ("get_caller_principal", LwccmExclusion::Security),
];

/// The operations that carry a §13-disabled name in addition to the
/// table entries above, with the subsection that disables them.
const DISABLED_SUPPLEMENTARY: [(&str, LwccmExclusion); 6] = [
    // §13.7 (p. 279) — "Disable the set_configurator operation in the
    // HomeConfiguration interface".
    ("set_configurator", LwccmExclusion::Configurators),
    // §13.8 (p. 279) — 9.4.1.3 `ProxyHomeRegistration` is removed.
    ("register_proxy_home", LwccmExclusion::ProxyHomes),
    // §13.9 (p. 279) — the `HomeFinder` operations of §6.8 and
    // `CCM2Context::get_home_registration`.
    ("find_home_by_component_type", LwccmExclusion::HomeFinders),
    ("find_home_by_home_type", LwccmExclusion::HomeFinders),
    ("find_home_by_name", LwccmExclusion::HomeFinders),
    ("get_home_registration", LwccmExclusion::HomeFinders),
];

/// The generic operations §13.3 keeps.
///
/// Spec §13.3 (p. 276), on `provide_facet`: "The generic version,
/// provide_facet, in the Navigation interface, is retained as it is
/// required for containers for deployment." The same reasoning keeps the
/// generic connect / disconnect of `Receptacles` and the generic event
/// routing of `Events` — those are what the type-specific operations are
/// declared redundant *with*.
///
/// Each of these carries a prefix that [`TYPE_SPECIFIC_PREFIXES`] would
/// otherwise catch, so the list is consulted first.
const RETAINED_GENERIC: [&str; 7] = [
    "provide_facet",
    "connect",
    "disconnect",
    "get_consumer",
    "subscribe",
    "unsubscribe",
    "connect_consumer",
];

/// The prefixes of the type-specific port operations §13.3 disables.
///
/// The equivalent interface names them after the port — `provide_stock`,
/// `connect_supplier`, `get_consumer_ticks` — so they cannot be listed,
/// only recognised.
const TYPE_SPECIFIC_PREFIXES: [&str; 8] = [
    "provide_",
    "connect_",
    "disconnect_",
    "get_connection_",
    "get_connections_",
    "subscribe_",
    "unsubscribe_",
    "get_consumer_",
];

/// §13.4 (p. 277) — "Restrict CIF metamodel to a single segment per
/// implementation": the multiplicity of the association between
/// `ComponentImplDef` and `SegmentDef` becomes 1:1.
pub const MAX_SEGMENTS_PER_IMPLEMENTATION: usize = 1;

/// Which §13 subsection disables `operation`, if any.
///
/// `None` means the Lightweight profile keeps the operation.
#[must_use]
pub fn disabled_by(operation: &str) -> Option<LwccmExclusion> {
    if let Some((_, exclusion)) = DISABLED_OPERATIONS
        .iter()
        .chain(DISABLED_SUPPLEMENTARY.iter())
        .find(|(name, _)| *name == operation)
    {
        return Some(*exclusion);
    }
    if RETAINED_GENERIC.contains(&operation) {
        return None;
    }
    if TYPE_SPECIFIC_PREFIXES
        .iter()
        .any(|prefix| operation.len() > prefix.len() && operation.starts_with(prefix))
    {
        return Some(LwccmExclusion::Introspection);
    }
    None
}

/// The exception a Lightweight container raises when `operation` is
/// invoked — spec §13.1 (p. 273).
///
/// `None` means the operation is not disabled and runs normally.
#[must_use]
pub fn rejection_for(operation: &str) -> Option<LwccmRejection> {
    let _ = disabled_by(operation)?;
    if operation == "same_component" {
        // §13.1 requires this exact form; the others are the vendor's
        // choice between BAD_OPERATION and NO_IMPLEMENT.
        return Some(LwccmRejection::NoImplement(
            LwccmMinorCode::IsLightweightComponent,
        ));
    }
    Some(LwccmRejection::BadOperation)
}

/// §13.1 (p. 273) — "Persistence (only session and service components
/// are supported)".
///
/// §13.2 (p. 275) states the same as a container API type restriction:
/// "Remove support for entity container API types. Only session
/// containers exist."
#[must_use]
pub fn category_supported(category: CompositionCategory) -> bool {
    matches!(
        category,
        CompositionCategory::Session | CompositionCategory::Service
    )
}

/// Which §13 subsection removes `composition` from a Lightweight
/// deployment.
///
/// Always `Some`. Spec §13.2 (p. 274) and §13.4 (p. 277) both carry the
/// row "Exclude support for CIDL — Chapter 7: remove", and §13.10
/// (p. 280) states the consequence: "CIDL is removed because
/// segmentation and persistence is removed, and thus the 'API type' does
/// not require specification." A composition whose category is itself
/// unsupported is reported under §13.2 instead, because that is the
/// narrower finding.
#[must_use]
pub fn composition_exclusion(composition: &Composition) -> Option<LwccmExclusion> {
    if !category_supported(composition.category) || composition.home_storage.is_some() {
        return Some(LwccmExclusion::Persistence);
    }
    Some(LwccmExclusion::Cidl)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn same_component_raises_no_implement_with_the_mandated_minor_code() {
        // §13.1 (p. 273): "In the specific case of the disabled
        // 'same_component' operation, on the Navigation interface,
        // throwing an exception of NO_IMPLEMENT is required, with a
        // minor code of IS_LIGHTWEIGHT_COMPONENT."
        assert_eq!(
            rejection_for("same_component"),
            Some(LwccmRejection::NoImplement(
                LwccmMinorCode::IsLightweightComponent
            ))
        );
    }

    #[test]
    fn every_other_disabled_operation_raises_bad_operation() {
        // §13.1 permits either exception for the rest.
        for (name, _) in DISABLED_OPERATIONS
            .iter()
            .chain(DISABLED_SUPPLEMENTARY.iter())
        {
            if *name == "same_component" {
                continue;
            }
            assert_eq!(
                rejection_for(name),
                Some(LwccmRejection::BadOperation),
                "{name} is disabled but answers nothing"
            );
        }
    }

    #[test]
    fn an_operation_no_table_names_is_not_disabled() {
        assert_eq!(disabled_by("configuration_complete_check"), None);
        assert_eq!(rejection_for("buy"), None);
    }

    // -----------------------------------------------------------------
    // §13.2 — persistence.
    // -----------------------------------------------------------------

    #[test]
    fn the_persistence_operations_are_disabled_by_thirteen_two_and_three() {
        // §13.2 (p. 275) disables `get_persistence` on CCM2Context;
        // §13.3 (p. 277) disables `get_primary_key` on CCMObject as
        // introspection.
        assert_eq!(
            disabled_by("get_persistence"),
            Some(LwccmExclusion::Persistence)
        );
        assert_eq!(
            disabled_by("get_primary_key"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(LwccmExclusion::Persistence.spec_section(), "13.2");
    }

    #[test]
    fn only_session_and_service_components_are_supported() {
        // §13.1 (p. 273): "Persistence (only session and service
        // components are supported)".
        assert!(category_supported(CompositionCategory::Session));
        assert!(category_supported(CompositionCategory::Service));
        assert!(!category_supported(CompositionCategory::Process));
        assert!(!category_supported(CompositionCategory::Entity));
    }

    #[test]
    fn an_entity_composition_is_excluded_as_persistence() {
        let entity = Composition {
            name: "OrderImpl".to_string(),
            category: CompositionCategory::Entity,
            home_executor: "OrderHomeExec".to_string(),
            home_storage: Some("OrderStorageHome".to_string()),
        };
        assert_eq!(
            composition_exclusion(&entity),
            Some(LwccmExclusion::Persistence)
        );
    }

    // -----------------------------------------------------------------
    // §13.4 — segmentation.
    // -----------------------------------------------------------------

    #[test]
    fn a_session_composition_is_still_excluded_because_cidl_is_gone() {
        // §13.2 and §13.4 both remove Chapter 7; §13.10 (p. 280) states
        // the consequence. A session composition is conformant as a
        // component category and still has no place in the profile as a
        // CIDL construct.
        let session = Composition {
            name: "TraderImpl".to_string(),
            category: CompositionCategory::Session,
            home_executor: "TraderHomeExec".to_string(),
            home_storage: None,
        };
        assert!(category_supported(session.category));
        assert_eq!(composition_exclusion(&session), Some(LwccmExclusion::Cidl));
        assert_eq!(LwccmExclusion::Cidl.spec_section(), "13.10");
    }

    #[test]
    fn the_cif_metamodel_holds_one_segment_per_implementation() {
        // §13.4 (p. 277): "restrict the multiplicity of the association
        // between ComponentImplDef and SegmentDef to 1:1".
        assert_eq!(MAX_SEGMENTS_PER_IMPLEMENTATION, 1);
        assert_eq!(LwccmExclusion::Segmentation.spec_section(), "13.4");
    }

    // -----------------------------------------------------------------
    // §13.5 / §13.6 — transactions and security.
    // -----------------------------------------------------------------

    #[test]
    fn the_three_transaction_operations_of_ccmcontext_are_disabled() {
        // §13.5 (p. 278): "Disable the get_rollback_only,
        // get_user_transaction, and set_rollback_only operations in the
        // CCMContext interface".
        for name in [
            "get_rollback_only",
            "get_user_transaction",
            "set_rollback_only",
        ] {
            assert_eq!(
                disabled_by(name),
                Some(LwccmExclusion::Transactions),
                "{name}"
            );
        }
        assert_eq!(LwccmExclusion::Transactions.spec_section(), "13.5");
    }

    #[test]
    fn the_two_credential_operations_of_ccmcontext_are_disabled() {
        // §13.6 (p. 278): "Disable the get_caller_principal and
        // is_caller_in_role operations in the CCMContext interface".
        assert_eq!(
            disabled_by("get_caller_principal"),
            Some(LwccmExclusion::Security)
        );
        assert_eq!(
            disabled_by("is_caller_in_role"),
            Some(LwccmExclusion::Security)
        );
        assert_eq!(LwccmExclusion::Security.spec_section(), "13.6");
    }

    #[test]
    fn get_ccm_home_survives_beside_the_disabled_ccmcontext_operations() {
        // §13.5 and §13.6 name four of the six CCMContext operations
        // (§9.3.2.1). The other two stay.
        assert_eq!(disabled_by("get_CCM_home"), None);
    }

    // -----------------------------------------------------------------
    // §13.8 / §13.9 — proxy homes and home finders.
    // -----------------------------------------------------------------

    #[test]
    fn register_proxy_home_is_disabled() {
        // §13.8 (p. 279) removes 9.4.1.3, the ProxyHomeRegistration
        // interface, whose one operation is register_proxy_home.
        assert_eq!(
            disabled_by("register_proxy_home"),
            Some(LwccmExclusion::ProxyHomes)
        );
        assert_eq!(LwccmExclusion::ProxyHomes.spec_section(), "13.8");
    }

    #[test]
    fn the_home_finder_operations_and_home_registration_are_disabled() {
        // §13.9 (p. 279) removes §6.8 — the three find_home_by_*
        // operations of HomeFinder — and CCM2Context::
        // get_home_registration.
        for name in [
            "find_home_by_component_type",
            "find_home_by_home_type",
            "find_home_by_name",
            "get_home_registration",
        ] {
            assert_eq!(
                disabled_by(name),
                Some(LwccmExclusion::HomeFinders),
                "{name}"
            );
        }
        assert_eq!(LwccmExclusion::HomeFinders.spec_section(), "13.9");
    }

    #[test]
    fn register_home_survives_because_only_the_proxy_form_is_excluded() {
        // §13.8 removes 9.4.1.3 (ProxyHomeRegistration), not 9.4.1.2
        // (HomeRegistration). `get_home_registration` is disabled by
        // §13.9 instead, and the two operations of HomeRegistration
        // itself are named by neither table.
        assert_eq!(disabled_by("register_home"), None);
        assert_eq!(disabled_by("unregister_home"), None);
    }

    // -----------------------------------------------------------------
    // §13.3 — type-specific versus generic.
    // -----------------------------------------------------------------

    #[test]
    fn the_type_specific_port_operations_are_disabled_and_the_generic_ones_are_not() {
        // §13.3 (p. 276) disables `provide_<name>` and keeps
        // `provide_facet`; the same split runs through the receptacle
        // and event operations.
        assert_eq!(
            disabled_by("provide_stock"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(disabled_by("provide_facet"), None);
        assert_eq!(
            disabled_by("connect_supplier"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(disabled_by("connect"), None);
        assert_eq!(disabled_by("disconnect"), None);
        assert_eq!(
            disabled_by("get_consumer_ticks"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(disabled_by("get_consumer"), None);
        assert_eq!(
            disabled_by("subscribe_ticks"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(disabled_by("subscribe"), None);
        assert_eq!(disabled_by("connect_consumer"), None);
    }

    #[test]
    fn both_connection_getters_are_disabled_the_generic_one_as_introspection() {
        // §13.3 (p. 276) disables `get_connection_<name>` and
        // `get_connections_<name>` in the equivalent interface and, one
        // row further down, the generic `get_connections` of
        // `Receptacles` as introspection.
        assert_eq!(
            disabled_by("get_connection_supplier"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(
            disabled_by("get_connections_supplier"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(
            disabled_by("get_connections"),
            Some(LwccmExclusion::Introspection)
        );
        assert_eq!(LwccmExclusion::Introspection.spec_section(), "13.3");
    }

    #[test]
    fn a_bare_prefix_is_not_an_operation_name() {
        // "provide_" is a prefix, not a port operation; a component with
        // an empty port name cannot be declared.
        assert_eq!(disabled_by("provide_"), None);
    }
}
