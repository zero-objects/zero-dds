// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Service-Evolution-Compatibility-Regeln (Spec §7.7).
//!
//! Pro Mapping (Basic + Enhanced) definiert die Spec, welche
//! Service-Evolutionen (add/remove/reorder operation, change
//! signature, etc.) backward-compatible sind. Dieses Modul kodiert
//! diese Regeln als Const-Tabellen und liefert Helper-Funktionen,
//! die ein Codegen-/Audit-Tool gegen evolved Service-Definitions
//! prueft.

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
    /// Spec §7.7.1.3 — Operation-Signatur aendert sich.
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
/// Liefert `true` wenn die Evolution unter dem gegebenen Mapping
/// backward-compatible ist (Old-Client kann mit New-Service reden).
#[must_use]
pub fn is_compatible(mapping: Mapping, evolution: Evolution) -> bool {
    match (mapping, evolution) {
        // Basic-Mapping: jede Strukturaenderung ist breaking
        // (Spec §7.7.1.x).
        (Mapping::Basic, _) => false,

        // Enhanced-Mapping (Spec §7.7.2.x): die meisten
        // Strukturaenderungen sind compat-by-default.
        (Mapping::Enhanced, Evolution::AddOperation) => true,
        (Mapping::Enhanced, Evolution::RemoveOperation) => true, // mit Caveat (§7.7.2.2)
        (Mapping::Enhanced, Evolution::ReorderOperations) => true,
        (Mapping::Enhanced, Evolution::ReorderBaseInterfaces) => true,
        (Mapping::Enhanced, Evolution::DuckTyping) => true,
        (Mapping::Enhanced, Evolution::AddRemoveParameter) => true,
        (Mapping::Enhanced, Evolution::ReorderParameters) => true,
        (Mapping::Enhanced, Evolution::AddRemoveReturnType) => true,

        // Aenderung des Typs (auch in Enhanced) ist breaking.
        (Mapping::Enhanced, Evolution::ChangeSignature) => false,
        (Mapping::Enhanced, Evolution::ChangeParameterType) => false,
        (Mapping::Enhanced, Evolution::ChangeReturnType) => false,
    }
}

/// Liefert alle Evolutionen, die unter dem gegebenen Mapping
/// **kompatibel** sind.
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

    // ---- Spec §7.7.1.1-3 Basic-Mapping (alle breaking) -------------

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
        // Spec §7.7.2.5.3 — Type-Aenderungen sind auch in Enhanced
        // breaking, weil das Wire-Format nicht kompatibel ist.
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
