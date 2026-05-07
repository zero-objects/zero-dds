// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DefinitionKind — Spec §10.5.2.
//!
//! Alle 26 IR-Definition-Kinds.

/// Alle Spec-§10.5.2 DefinitionKind-Werte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DefinitionKind {
    /// `dk_none = 0`.
    None = 0,
    /// `dk_all = 1`.
    All = 1,
    /// `dk_Attribute = 2`.
    Attribute = 2,
    /// `dk_Constant = 3`.
    Constant = 3,
    /// `dk_Exception = 4`.
    Exception = 4,
    /// `dk_Interface = 5`.
    Interface = 5,
    /// `dk_Module = 6`.
    Module = 6,
    /// `dk_Operation = 7`.
    Operation = 7,
    /// `dk_Typedef = 8`.
    Typedef = 8,
    /// `dk_Alias = 9`.
    Alias = 9,
    /// `dk_Struct = 10`.
    Struct = 10,
    /// `dk_Union = 11`.
    Union = 11,
    /// `dk_Enum = 12`.
    Enum = 12,
    /// `dk_Primitive = 13`.
    Primitive = 13,
    /// `dk_String = 14`.
    String = 14,
    /// `dk_Sequence = 15`.
    Sequence = 15,
    /// `dk_Array = 16`.
    Array = 16,
    /// `dk_Repository = 17`.
    Repository = 17,
    /// `dk_Wstring = 18`.
    WString = 18,
    /// `dk_Fixed = 19`.
    Fixed = 19,
    /// `dk_Value = 20`.
    Value = 20,
    /// `dk_ValueBox = 21`.
    ValueBox = 21,
    /// `dk_ValueMember = 22`.
    ValueMember = 22,
    /// `dk_Native = 23`.
    Native = 23,
    /// `dk_AbstractInterface = 24`.
    AbstractInterface = 24,
    /// `dk_LocalInterface = 25`.
    LocalInterface = 25,
}

impl DefinitionKind {
    /// Roher Diskriminanten-Wert.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn well_known_definition_kinds_match_spec() {
        // Spec §10.5.2: Table 10-1.
        assert_eq!(DefinitionKind::None.as_u32(), 0);
        assert_eq!(DefinitionKind::Module.as_u32(), 6);
        assert_eq!(DefinitionKind::Interface.as_u32(), 5);
        assert_eq!(DefinitionKind::ValueBox.as_u32(), 21);
        assert_eq!(DefinitionKind::AbstractInterface.as_u32(), 24);
        assert_eq!(DefinitionKind::LocalInterface.as_u32(), 25);
    }

    #[test]
    fn at_least_26_kinds_modeled() {
        let all = [
            DefinitionKind::None,
            DefinitionKind::All,
            DefinitionKind::Attribute,
            DefinitionKind::Constant,
            DefinitionKind::Exception,
            DefinitionKind::Interface,
            DefinitionKind::Module,
            DefinitionKind::Operation,
            DefinitionKind::Typedef,
            DefinitionKind::Alias,
            DefinitionKind::Struct,
            DefinitionKind::Union,
            DefinitionKind::Enum,
            DefinitionKind::Primitive,
            DefinitionKind::String,
            DefinitionKind::Sequence,
            DefinitionKind::Array,
            DefinitionKind::Repository,
            DefinitionKind::WString,
            DefinitionKind::Fixed,
            DefinitionKind::Value,
            DefinitionKind::ValueBox,
            DefinitionKind::ValueMember,
            DefinitionKind::Native,
            DefinitionKind::AbstractInterface,
            DefinitionKind::LocalInterface,
        ];
        assert_eq!(all.len(), 26);
    }
}
