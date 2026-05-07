// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Naming Conventions — Spec §9.2 Tabellen 9.7/9.11/9.15/9.16/9.27.
//!
//! Spec-normative Naming-Regeln:
//!
//! * Struct-DataType: `<StructTypeName>DataType` (Tab 9.15).
//! * Struct-VariableType: `<StructTypeName>VariableType` (Tab 9.16).
//! * Union-DataType: `<UnionTypeName>DataType` (Tab 9.20).
//! * Enum-DataType: `<EnumName>DataType` (Tab 9.7).
//! * Bitmask-DataType: `<BitmaskTypeName>DataType` (Tab 9.11).
//! * Array-of-Struct-Element-Variable: `<StructTypeName>_<index>`
//!   (Tab 9.27, mit `_`-Trennung pro Dimension fuer Multi-Dim).
//! * Bitmask-Gap: `UndefinedPosition_<PositionNumber>` (Tab 9.11).

use alloc::format;
use alloc::string::String;

/// Spec Tab 9.15/9.7/9.11 — DataType-Name fuer einen Struct/Enum/
/// Bitmask/Union-Type aus dem DDS-Type-Namen.
#[must_use]
pub fn data_type_name(dds_type_name: &str) -> String {
    format!("{dds_type_name}DataType")
}

/// Spec Tab 9.16 — VariableType-Name fuer einen Struct/Union-Type
/// (`<TypeName>VariableType`). Wird auch in §9.2.4.2 fuer Union-
/// Variable-Types genutzt.
#[must_use]
pub fn variable_type_name(dds_type_name: &str) -> String {
    format!("{dds_type_name}VariableType")
}

/// Spec Tab 9.27 — Element-BrowseName fuer das `<index>`-te Element
/// eines Arrays/Sequence von Structures/Unions. Multi-Dim-Indizes
/// werden mit `_` getrennt (z.B. Position [1][2][3] → `_1_2_3`).
///
/// `indices` ist die Position pro Dimension (Outer-First).
#[must_use]
pub fn array_element_browse_name(struct_type_name: &str, indices: &[u32]) -> String {
    let mut s = String::from(struct_type_name);
    for i in indices {
        s.push('_');
        s.push_str(&format!("{i}"));
    }
    s
}

/// Spec Tab 9.11 — `UndefinedPosition_<N>` Eintrag fuer Bitmask-
/// Position ohne explizit definiertes Bitflag.
#[must_use]
pub fn undefined_bitfield_position(position: u32) -> String {
    format!("UndefinedPosition_{position}")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn struct_data_type_appends_suffix() {
        assert_eq!(data_type_name("ShapeType"), "ShapeTypeDataType");
        assert_eq!(data_type_name("Foo"), "FooDataType");
    }

    #[test]
    fn struct_variable_type_appends_suffix() {
        assert_eq!(variable_type_name("ShapeType"), "ShapeTypeVariableType");
    }

    #[test]
    fn array_element_single_dim_uses_underscore() {
        // Spec §9.2.5 Array-Element: <StructTypeName>_<index>.
        assert_eq!(array_element_browse_name("ShapeType", &[3]), "ShapeType_3");
    }

    #[test]
    fn array_element_multi_dim_concatenates_with_underscore() {
        // Spec §9.2.5: Multi-Dim Position [1][2][3] → `_1_2_3`.
        assert_eq!(
            array_element_browse_name("ShapeType", &[1, 2, 3]),
            "ShapeType_1_2_3"
        );
    }

    #[test]
    fn array_element_zero_dim_returns_bare_name() {
        // Edge-Case: keine Indizes -> nur Type-Name.
        assert_eq!(array_element_browse_name("ShapeType", &[]), "ShapeType");
    }

    #[test]
    fn undefined_position_uses_decimal_index() {
        // Spec §9.2.3.2.2 Tab 9.11: `UndefinedPosition_<PositionNumber>`.
        assert_eq!(undefined_bitfield_position(1), "UndefinedPosition_1");
        assert_eq!(undefined_bitfield_position(42), "UndefinedPosition_42");
    }
}
