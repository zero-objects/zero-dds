// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Service-evolution compatibility rules (Spec §7.7).
//!
//! Per mapping (basic + enhanced), the spec defines which
//! service evolutions (add/remove/reorder operation, change
//! signature, etc.) are backward-compatible. This module encodes
//! these rules as const tables and provides helper functions that
//! a codegen/audit tool checks against evolved service definitions.

extern crate alloc;

use alloc::vec::Vec;

/// Service-Evolution-Operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evolution {
    /// Spec §7.7.1.1 / §7.7.2.1.
    AddOperation,
    /// Spec §7.7.1.1 / §7.7.2.2.
    RemoveOperation,
    /// Spec §7.7.1.2 / §7.7.2.3.
    ReorderOperations,
    /// Spec §7.7.1.2.
    ReorderBaseInterfaces,
    /// Spec §7.7.1.3 — operation signature changes.
    ChangeSignature,
    /// Spec §7.7.2.4 — Duck-Typing.
    DuckTyping,
    /// Spec §7.7.2.5.1.
    AddRemoveParameter,
    /// Spec §7.7.2.5.2.
    ReorderParameters,
    /// Spec §7.7.2.5.3.
    ChangeParameterType,
    /// Spec §7.7.2.5.4.
    AddRemoveReturnType,
    /// Spec §7.7.2.5.5.
    ChangeReturnType,
    /// Spec §7.7.2.5.6.
    AddRemoveException,
}

/// Mapping profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// Basic-Mapping (Spec §7.7.1).
    Basic,
    /// Enhanced-Mapping (Spec §7.7.2).
    Enhanced,
}

/// Spec §7.7: Compatibility table per (mapping, evolution).
///
/// Returns `true` if the evolution is backward-compatible under the
/// given mapping (an old client can talk to a new service).
///
/// Basic (§7.7.1): adding or removing an operation is defined behaviour
/// (§7.7.1.1 — the opposite side is oblivious or answers
/// `REMOTE_EX_UNSUPPORTED`), and reordering operations or base
/// interfaces has no impact (§7.7.1.2). A signature change is not
/// supported (§7.7.1.3). The Enhanced-only evolutions do not apply.
///
/// Enhanced (§7.7.2): adding/removing an operation and reordering are
/// compatible; a parameter or return-type add/remove/reorder breaks
/// compatibility unless the operation is `@Mutable` (§7.7.2.5.1/.2/.4:
/// the `In` and `Out` structures are `@final`, and "it is, however,
/// possible to mark an operation @mutable"), which `mutable` says; a
/// *type* change is allowed and governed by the XTypes assignability
/// rules (§7.7.2.5.3/.5); adding/removing an exception is supported
/// (§7.7.2.5.6) whatever `mutable` says, since the `Result` structure is
/// a mutable `@Choice` in any case.
///
/// `mutable` is whether the evolved operation carries `@Mutable`. The
/// Basic mapping ignores the annotation (its structures are `@final`
/// whatever is written), so it does not read the argument.
#[must_use]
pub fn is_compatible(mapping: Mapping, evolution: Evolution, mutable: bool) -> bool {
    match (mapping, evolution) {
        // §7.7.1.1 — adding/removing an operation is defined behaviour.
        (Mapping::Basic, Evolution::AddOperation | Evolution::RemoveOperation) => true,
        // §7.7.1.2 — reordering has no impact.
        (Mapping::Basic, Evolution::ReorderOperations | Evolution::ReorderBaseInterfaces) => true,
        // §7.7.1.3 — the Basic profile supports no signature change, and
        // the Enhanced-only evolutions do not apply to it.
        (Mapping::Basic, _) => false,

        // §7.7.2.1-.4.
        (
            Mapping::Enhanced,
            Evolution::AddOperation
            | Evolution::RemoveOperation
            | Evolution::ReorderOperations
            | Evolution::ReorderBaseInterfaces
            | Evolution::DuckTyping,
        ) => true,
        // §7.7.2.5.3 and §7.7.2.5.5 — a type change is allowed.
        (Mapping::Enhanced, Evolution::ChangeParameterType | Evolution::ChangeReturnType) => true,
        // §7.7.2.5.6 — adding/removing an exception is supported.
        (Mapping::Enhanced, Evolution::AddRemoveException) => true,
        // §7.7.2.5.1/.2/.4 — parameter and return-type add/remove/reorder
        // break compatibility under the default `@final`; an operation
        // marked `@Mutable` has `@mutable` `In` and `Out` structures
        // and allows them.
        (
            Mapping::Enhanced,
            Evolution::AddRemoveParameter
            | Evolution::ReorderParameters
            | Evolution::AddRemoveReturnType,
        ) => mutable,
        // §7.7.1.3's signature change is a Basic rule.
        (Mapping::Enhanced, Evolution::ChangeSignature) => false,
    }
}

/// Returns all evolutions that are **compatible** under the given
/// mapping, for an operation with or without `@Mutable` (see
/// [`is_compatible`]).
#[must_use]
pub fn compatible_evolutions(mapping: Mapping, mutable: bool) -> Vec<Evolution> {
    [
        Evolution::AddOperation,
        Evolution::RemoveOperation,
        Evolution::ReorderOperations,
        Evolution::ReorderBaseInterfaces,
        Evolution::ChangeSignature,
        Evolution::DuckTyping,
        Evolution::AddRemoveParameter,
        Evolution::ReorderParameters,
        Evolution::ChangeParameterType,
        Evolution::AddRemoveReturnType,
        Evolution::ChangeReturnType,
        Evolution::AddRemoveException,
    ]
    .iter()
    .copied()
    .filter(|e| is_compatible(mapping, *e, mutable))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Spec §7.7.1 Basic mapping -------------------------------

    #[test]
    fn basic_mapping_add_remove_operation_is_compatible() {
        // §7.7.1.1: a client with fewer operations is oblivious; a client
        // with more gets REMOTE_EX_UNSUPPORTED. Defined behaviour, not a
        // break.
        assert!(is_compatible(
            Mapping::Basic,
            Evolution::AddOperation,
            false
        ));
        assert!(is_compatible(
            Mapping::Basic,
            Evolution::RemoveOperation,
            false
        ));
    }

    #[test]
    fn basic_mapping_reorder_operations_is_compatible() {
        // §7.7.1.2: "Reordering operations has no impact on the semantics
        // of the interface [...]. Reordering base interfaces has no
        // impact."
        assert!(is_compatible(
            Mapping::Basic,
            Evolution::ReorderOperations,
            false
        ));
        assert!(is_compatible(
            Mapping::Basic,
            Evolution::ReorderBaseInterfaces,
            false
        ));
    }

    #[test]
    fn basic_mapping_change_signature_is_breaking() {
        // §7.7.1.3.
        assert!(!is_compatible(
            Mapping::Basic,
            Evolution::ChangeSignature,
            false
        ));
        assert!(!is_compatible(
            Mapping::Basic,
            Evolution::AddRemoveParameter,
            false
        ));
        assert!(!is_compatible(
            Mapping::Basic,
            Evolution::ChangeReturnType,
            false
        ));
    }

    #[test]
    fn basic_mapping_has_four_compatible_evolutions() {
        let compats = compatible_evolutions(Mapping::Basic, false);
        assert_eq!(compats.len(), 4, "{compats:?}");
        assert!(compats.contains(&Evolution::AddOperation));
        assert!(compats.contains(&Evolution::RemoveOperation));
        assert!(compats.contains(&Evolution::ReorderOperations));
        assert!(compats.contains(&Evolution::ReorderBaseInterfaces));
    }

    // ---- Spec §7.7.2 Enhanced mapping ----------------------------

    #[test]
    fn enhanced_mapping_add_operation_is_compatible() {
        // Spec §7.7.2.1.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddOperation,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_remove_operation_is_compatible() {
        // Spec §7.7.2.2.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::RemoveOperation,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_reorder_operations_is_compatible() {
        // Spec §7.7.2.3.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderOperations,
            false
        ));
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderBaseInterfaces,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_duck_typing_is_compatible() {
        // Spec §7.7.2.4.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::DuckTyping,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_add_remove_param_is_breaking() {
        // §7.7.2.5.1: adding and removing parameters break compatibility;
        // "@final annotation on the In and Out structures enforce this
        // restriction". Without @Mutable; see the tests below for the marked operation.
        assert!(!is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveParameter,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_reorder_params_is_breaking() {
        // §7.7.2.5.2.
        assert!(!is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderParameters,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_change_param_type_is_compatible() {
        // §7.7.2.5.3: "Changing the type of the parameters is allowed by
        // this specification."
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ChangeParameterType,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_add_remove_return_type_is_breaking() {
        // §7.7.2.5.4.
        assert!(!is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveReturnType,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_change_return_type_is_compatible() {
        // §7.7.2.5.5.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ChangeReturnType,
            false
        ));
    }

    #[test]
    fn enhanced_mapping_add_remove_exception_is_compatible() {
        // §7.7.2.5.6.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveException,
            false
        ));
    }

    #[test]
    fn enhanced_compatible_evolutions_includes_8_of_12() {
        let compats = compatible_evolutions(Mapping::Enhanced, false);
        // Add/Remove-Op + Reorder-Ops + Reorder-Bases + DuckTyping +
        // Change-Param-Type + Change-Return-Type + Add/Remove-Exception
        // = 8.
        assert_eq!(compats.len(), 8, "{compats:?}");
    }

    // ---- `@Mutable` (§7.7.2.5.1, .2, .4) ----------------------------

    #[test]
    fn a_mutable_operation_allows_parameter_changes() {
        // §7.7.2.5.1: "It is, however, possible to mark an operation
        // @mutable to allow the In and Out structures to be mutable and
        // thereby supporting extensions."
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveParameter,
            true
        ));
        // §7.7.2.5.2: "It is, however, possible to mark an operation
        // @mutable to allow reordering of parameters."
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderParameters,
            true
        ));
        // §7.7.2.5.4: "It is, however, possible to annotate an operation
        // @Mutable to allow [...] addition or removal of return type".
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveReturnType,
            true
        ));
    }

    #[test]
    fn mutable_changes_nothing_that_was_compatible_or_never_was() {
        // The evolutions that did not depend on `@Mutable` do not start to.
        for evolution in [
            Evolution::AddOperation,
            Evolution::RemoveOperation,
            Evolution::ReorderOperations,
            Evolution::ReorderBaseInterfaces,
            Evolution::DuckTyping,
            Evolution::ChangeParameterType,
            Evolution::ChangeReturnType,
            Evolution::AddRemoveException,
            Evolution::ChangeSignature,
        ] {
            assert_eq!(
                is_compatible(Mapping::Enhanced, evolution, true),
                is_compatible(Mapping::Enhanced, evolution, false),
                "{evolution:?}"
            );
        }
        // The Basic mapping ignores the annotation.
        for evolution in [
            Evolution::AddRemoveParameter,
            Evolution::ReorderParameters,
            Evolution::AddRemoveReturnType,
            Evolution::ChangeSignature,
        ] {
            assert!(
                !is_compatible(Mapping::Basic, evolution, true),
                "{evolution:?}"
            );
        }
    }

    #[test]
    fn a_mutable_operation_has_eleven_compatible_enhanced_evolutions() {
        // The eight above plus the three that `@Mutable` opens; the
        // Basic signature change stays out.
        let compats = compatible_evolutions(Mapping::Enhanced, true);
        assert_eq!(compats.len(), 11, "{compats:?}");
        assert!(compats.contains(&Evolution::AddRemoveParameter));
        assert!(compats.contains(&Evolution::ReorderParameters));
        assert!(compats.contains(&Evolution::AddRemoveReturnType));
        assert!(!compats.contains(&Evolution::ChangeSignature));
    }
}
