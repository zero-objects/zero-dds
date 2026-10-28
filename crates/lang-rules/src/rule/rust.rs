// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust rule catalogue. Spec §2.
//!
//! Each rule is as small as it can be while still standing alone, and
//! each states only what must be present. Application is monotone and
//! add-only: a rule creates nodes and links and removes nothing.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "rust";

/// The name of the rule that maps a struct to a Rust struct.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a Rust field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a Rust enum.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to a Rust variant.
pub const ENUMERATOR_TO_VARIANT: &str = "enumerator-to-variant";

/// The name of the rule that maps a typedef to a Rust type alias.
pub const ALIAS_TO_TYPE_ALIAS: &str = "alias-to-type-alias";

/// The name of the rule that maps a union to a Rust enum.
pub const UNION_TO_ENUM: &str = "union-to-enum";

/// The name of the rule that maps a union case to a variant.
pub const CASE_TO_VARIANT: &str = "case-to-variant";

/// The name of the rule that maps a bitmask to a Rust struct of bits.
pub const BITMASK_TO_STRUCT: &str = "bitmask-to-struct";

/// The name of the rule that maps a bit to a constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a Rust struct.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a Rust struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a Rust module.
pub const MODULE_TO_MOD: &str = "module-to-mod";

/// The name of the rule that maps a constant to a Rust constant.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a Rust struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps an interface to a Rust trait.
pub const INTERFACE_TO_TRAIT: &str = "interface-to-trait";

/// The name of the rule that maps a native declaration to an alias.
pub const NATIVE_TO_ALIAS: &str = "native-to-alias";

/// The data rules a CORBA target needs: [`catalogue`] without
/// [`INTERFACE_TO_TRAIT`] and [`VALUETYPE_TO_STRUCT`].
///
/// Those two are the DDS Rust PSM's answers for an interface and a
/// valuetype. The CORBA column (`zerodds-corba-rust`) emits its own for
/// both — an operation that crosses a wire returns a `Result`, a
/// valuetype is a trait plus a state carrier — and a user concatenates
/// the two columns, so with both rules in place one name is declared
/// twice and the result does not compile.
#[must_use]
pub fn corba_data_catalogue() -> RuleFile {
    let mut file = catalogue();
    file.rules
        .retain(|rule| rule.name != INTERFACE_TO_TRAIT && rule.name != VALUETYPE_TO_STRUCT);
    file
}

/// Every Rust rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a
        // mismatch rather than reading an older file as if it were
        // this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_STRUCT, "Structure", "RustStruct", "StructCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "StructCorr"),
                ("RustField", "RustStruct", "FieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "RustEnum", "EnumCorr"),
            part_of(
                ENUMERATOR_TO_VARIANT,
                ("Enumerator", "Enumeration", "EnumCorr"),
                ("RustVariant", "RustEnum", "VariantCorr"),
            ),
            one_to_one(ALIAS_TO_TYPE_ALIAS, "Alias", "RustTypeAlias", "AliasCorr"),
            one_to_one(UNION_TO_ENUM, "Union", "RustUnionEnum", "UnionCorr"),
            part_of(
                CASE_TO_VARIANT,
                ("Case", "Union", "UnionCorr"),
                ("RustCaseVariant", "RustUnionEnum", "CaseCorr"),
            ),
            one_to_one(BITMASK_TO_STRUCT, "Bitmask", "RustBitmask", "BitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "BitmaskCorr"),
                ("RustBitConst", "RustBitmask", "BitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "RustBitset", "BitsetCorr"),
            one_to_one(
                VALUETYPE_TO_STRUCT,
                "Valuetype",
                "RustValueStruct",
                "ValuetypeCorr",
            ),
            one_to_one(MODULE_TO_MOD, "Module", "RustModule", "ModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "RustConst", "ConstCorr"),
            one_to_one(
                EXCEPTION_TO_STRUCT,
                "Exception",
                "RustErrorStruct",
                "ExceptionCorr",
            ),
            one_to_one(
                INTERFACE_TO_TRAIT,
                "Interface",
                "RustTrait",
                "InterfaceCorr",
            ),
            one_to_one(NATIVE_TO_ALIAS, "Native", "RustNativeAlias", "NativeCorr"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corba_data_catalogue_is_the_catalogue_without_the_two_dds_answers() {
        let names = |file: &RuleFile| -> Vec<String> {
            file.rules.iter().map(|rule| rule.name.clone()).collect()
        };
        let all = names(&catalogue());
        let expected: Vec<String> = all
            .iter()
            .filter(|name| *name != INTERFACE_TO_TRAIT && *name != VALUETYPE_TO_STRUCT)
            .cloned()
            .collect();
        assert_eq!(names(&corba_data_catalogue()), expected);
        assert_eq!(all.len(), expected.len() + 2);
    }
}
