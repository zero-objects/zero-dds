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
}

/// Mapping-Profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// Basic-Mapping (Spec §7.7.1).
    Basic,
    /// Enhanced-Mapping (Spec §7.7.2).
    Enhanced,
}

/// Spec §7.7: Compatibility-Tabelle pro (Mapping, Evolution).
///
/// Returns `true` if the evolution is backward-compatible under the
/// given mapping (an old client can talk to a new service).
#[must_use]
pub fn is_compatible(mapping: Mapping, evolution: Evolution) -> bool {
    match (mapping, evolution) {
        // Basic mapping: every structural change is breaking
        // (Spec §7.7.1.x).
        (Mapping::Basic, _) => false,

        // Enhanced mapping (Spec §7.7.2.x): most
        // structural changes are compat-by-default.
        (Mapping::Enhanced, Evolution::AddOperation) => true,
        (Mapping::Enhanced, Evolution::RemoveOperation) => true, // with a caveat (§7.7.2.2)
        (Mapping::Enhanced, Evolution::ReorderOperations) => true,
        (Mapping::Enhanced, Evolution::ReorderBaseInterfaces) => true,
        (Mapping::Enhanced, Evolution::DuckTyping) => true,
        (Mapping::Enhanced, Evolution::AddRemoveParameter) => true,
        (Mapping::Enhanced, Evolution::ReorderParameters) => true,
        (Mapping::Enhanced, Evolution::AddRemoveReturnType) => true,

        // A change of type (even in enhanced) is breaking.
        (Mapping::Enhanced, Evolution::ChangeSignature) => false,
        (Mapping::Enhanced, Evolution::ChangeParameterType) => false,
        (Mapping::Enhanced, Evolution::ChangeReturnType) => false,
    }
}

/// Returns all evolutions that are **compatible** under the given
/// mapping.
#[must_use]
pub fn compatible_evolutions(mapping: Mapping) -> Vec<Evolution> {
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
    ]
    .iter()
    .copied()
    .filter(|e| is_compatible(mapping, *e))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Spec §7.7.1.1-3 basic mapping (all breaking) -------------

    #[test]
    fn basic_mapping_add_remove_operation_is_breaking() {
        // Spec §7.7.1.1.
        assert!(!is_compatible(Mapping::Basic, Evolution::AddOperation));
        assert!(!is_compatible(Mapping::Basic, Evolution::RemoveOperation));
    }

    #[test]
    fn basic_mapping_reorder_operations_is_breaking() {
        // Spec §7.7.1.2.
        assert!(!is_compatible(Mapping::Basic, Evolution::ReorderOperations));
        assert!(!is_compatible(
            Mapping::Basic,
            Evolution::ReorderBaseInterfaces
        ));
    }

    #[test]
    fn basic_mapping_change_signature_is_breaking() {
        // Spec §7.7.1.3.
        assert!(!is_compatible(Mapping::Basic, Evolution::ChangeSignature));
    }

    #[test]
    fn basic_mapping_has_no_compatible_evolutions() {
        assert!(compatible_evolutions(Mapping::Basic).is_empty());
    }

    // ---- Spec §7.7.2.1-5 Enhanced-Mapping (compat by default) -----

    #[test]
    fn enhanced_mapping_add_operation_is_compatible() {
        // Spec §7.7.2.1.
        assert!(is_compatible(Mapping::Enhanced, Evolution::AddOperation));
    }

    #[test]
    fn enhanced_mapping_remove_operation_is_compatible_with_caveat() {
        // Spec §7.7.2.2.
        assert!(is_compatible(Mapping::Enhanced, Evolution::RemoveOperation));
    }

    #[test]
    fn enhanced_mapping_reorder_operations_is_compatible() {
        // Spec §7.7.2.3.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderOperations
        ));
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderBaseInterfaces
        ));
    }

    #[test]
    fn enhanced_mapping_duck_typing_is_compatible() {
        // Spec §7.7.2.4.
        assert!(is_compatible(Mapping::Enhanced, Evolution::DuckTyping));
    }

    #[test]
    fn enhanced_mapping_add_remove_param_is_compatible() {
        // Spec §7.7.2.5.1.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveParameter
        ));
    }

    #[test]
    fn enhanced_mapping_reorder_params_is_compatible() {
        // Spec §7.7.2.5.2.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::ReorderParameters
        ));
    }

    #[test]
    fn enhanced_mapping_change_param_type_is_breaking() {
        // Spec §7.7.2.5.3 — type changes are breaking even in enhanced,
        // because the wire format is not compatible.
        assert!(!is_compatible(
            Mapping::Enhanced,
            Evolution::ChangeParameterType
        ));
    }

    #[test]
    fn enhanced_mapping_add_remove_return_type_is_compatible() {
        // Spec §7.7.2.5.4.
        assert!(is_compatible(
            Mapping::Enhanced,
            Evolution::AddRemoveReturnType
        ));
    }

    #[test]
    fn enhanced_mapping_change_return_type_is_breaking() {
        // Spec §7.7.2.5.5.
        assert!(!is_compatible(
            Mapping::Enhanced,
            Evolution::ChangeReturnType
        ));
    }

    #[test]
    fn enhanced_compatible_evolutions_includes_8_of_11() {
        let compats = compatible_evolutions(Mapping::Enhanced);
        // Add/Remove/Reorder-Op + Reorder-Bases + DuckTyping +
        // Add/Remove-Param + Reorder-Params + Add/Remove-Return = 8.
        assert_eq!(compats.len(), 8);
    }
}
