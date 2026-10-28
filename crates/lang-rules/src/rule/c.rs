// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C rule catalogue. Spec §2.
//!
//! The same constructs the Rust catalogue covers, mapped onto C's
//! own tree. The rules are the same two shapes; what differs is the node
//! each construct becomes, which is the difference a diff of two
//! catalogues is supposed to show.
//!
//! Reading this against `rule/rust.rs` is the point. Four differences
//! are C's and are named where the materialiser acts on them:
//!
//! - A module becomes a **name prefix**, not a scope. C has no
//!   namespaces, so `::common::util::Location` is `common_util_Location`
//!   and every generated identifier carries its module path.
//! - A bitmask becomes an **integer and one constant per bit**, the same
//!   as in Rust, but the constants are `#define`s at file scope, so they
//!   carry the module prefix too.
//! - An enumeration becomes an **integer typedef and constants** rather
//!   than a C `enum`: an enumeration constant is an `int` (C11 §6.7.2.2)
//!   and `@bit_bound(32)` admits values an `int` cannot hold.
//! - An **interface has no rule at all**. See `coverage.rs`.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "c";

/// The name of the rule that maps a struct to a C struct.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a C field.
pub const MEMBER_TO_MEMBER: &str = "member-to-member";

/// The name of the rule that maps an enumeration to a C integer and its
/// constants.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to a constant.
pub const ENUMERATOR_TO_ENUMERATOR: &str = "enumerator-to-enumerator";

/// The name of the rule that maps a typedef to a C typedef.
pub const ALIAS_TO_TYPEDEF: &str = "alias-to-typedef";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_UNION: &str = "union-to-union";

/// The name of the rule that maps a union case to a union member.
pub const CASE_TO_MEMBER: &str = "case-to-member";

/// The name of the rule that maps a bitmask to an integer and its bits.
pub const BITMASK_TO_ENUM: &str = "bitmask-to-enum";

/// The name of the rule that maps a bit to a constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a `#define`.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to a typedef.
pub const NATIVE_TO_TYPEDEF: &str = "native-to-typedef";

/// Every C rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a
        // mismatch rather than reading an older file as if it were
        // this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_STRUCT, "Structure", "CStruct", "CStructCorr"),
            part_of(
                MEMBER_TO_MEMBER,
                ("Member", "Structure", "CStructCorr"),
                ("CField", "CStruct", "CFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "CEnum", "CEnumCorr"),
            part_of(
                ENUMERATOR_TO_ENUMERATOR,
                ("Enumerator", "Enumeration", "CEnumCorr"),
                ("CEnumerator", "CEnum", "CEnumeratorCorr"),
            ),
            one_to_one(ALIAS_TO_TYPEDEF, "Alias", "CAlias", "CAliasCorr"),
            one_to_one(UNION_TO_UNION, "Union", "CUnion", "CUnionCorr"),
            part_of(
                CASE_TO_MEMBER,
                ("Case", "Union", "CUnionCorr"),
                ("CCase", "CUnion", "CCaseCorr"),
            ),
            one_to_one(BITMASK_TO_ENUM, "Bitmask", "CBitmask", "CBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "CBitmaskCorr"),
                ("CBitConst", "CBitmask", "CBitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "CBitset", "CBitsetCorr"),
            one_to_one(VALUETYPE_TO_STRUCT, "Valuetype", "CValue", "CValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "CModule", "CModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "CConst", "CConstCorr"),
            one_to_one(EXCEPTION_TO_STRUCT, "Exception", "CError", "CErrorCorr"),
            one_to_one(NATIVE_TO_TYPEDEF, "Native", "CNative", "CNativeCorr"),
        ],
    }
}
