// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto D's own
//! tree. Reading this against `rule/c.rs` and `rule/go.rs` is the point,
//! because D answers three of their questions differently:
//!
//! - A module becomes a **name prefix**, as in C and Go. D has real
//!   modules, but the correspondence between a module and a file is
//!   one-to-one (D language specification, "Modules"), so the nested IDL
//!   modules of one IDL file cannot each be a D module in one generated
//!   file; `::common::util::Location` is `Common_Util_Location`.
//! - A union becomes a **struct with a discriminator and an anonymous
//!   `union` member**, which is the C column's answer and not the Go
//!   column's: D has overlaid storage and Go does not.
//! - An enumeration becomes **one `enum` declaration with its base type
//!   and every value written out**, where C and Go need a type
//!   declaration and one constant per enumerator. D's `enum` is a named
//!   scope, so `Mode.IDLE` does not collide with anything and the
//!   enumerators need no module prefix.
//! - An **interface has a rule here**, as it does in Go and not in C. A
//!   D `interface` is a method set with names, which is what an IDL
//!   interface declares.
//!
//! A bitmask still becomes an alias and a run of constants rather than a
//! D `enum`: an `enum` is a closed set of named values and a bitmask
//! value is any combination of its bits, so the constants are manifest
//! constants of the alias's type (DDS-XTypes 1.3 §7.4.1.1.3.2).

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "d";

/// The name of the rule that maps a struct to a D struct.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a D field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a D enum.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to an enum member.
pub const ENUMERATOR_TO_MEMBER: &str = "enumerator-to-member";

/// The name of the rule that maps a typedef to a D alias.
pub const ALIAS_TO_ALIAS: &str = "alias-to-alias";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_STRUCT: &str = "union-to-struct";

/// The name of the rule that maps a union case to a field of the
/// anonymous union.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an alias and its bits.
pub const BITMASK_TO_ALIAS: &str = "bitmask-to-alias";

/// The name of the rule that maps a bit to a manifest constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a manifest constant.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to an alias.
pub const NATIVE_TO_ALIAS: &str = "native-to-alias";

/// The name of the rule that maps an interface to a D interface.
pub const INTERFACE_TO_INTERFACE: &str = "interface-to-interface";

/// Every D rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_STRUCT, "Structure", "DStruct", "DStructCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "DStructCorr"),
                ("DField", "DStruct", "DFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "DEnum", "DEnumCorr"),
            part_of(
                ENUMERATOR_TO_MEMBER,
                ("Enumerator", "Enumeration", "DEnumCorr"),
                ("DEnumMember", "DEnum", "DEnumMemberCorr"),
            ),
            one_to_one(ALIAS_TO_ALIAS, "Alias", "DAlias", "DAliasCorr"),
            one_to_one(UNION_TO_STRUCT, "Union", "DUnion", "DUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "DUnionCorr"),
                ("DCase", "DUnion", "DCaseCorr"),
            ),
            one_to_one(BITMASK_TO_ALIAS, "Bitmask", "DBitmask", "DBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "DBitmaskCorr"),
                ("DBitConst", "DBitmask", "DBitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "DBitset", "DBitsetCorr"),
            one_to_one(VALUETYPE_TO_STRUCT, "Valuetype", "DValue", "DValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "DModule", "DModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "DConst", "DConstCorr"),
            one_to_one(EXCEPTION_TO_STRUCT, "Exception", "DError", "DErrorCorr"),
            one_to_one(NATIVE_TO_ALIAS, "Native", "DNative", "DNativeCorr"),
            one_to_one(
                INTERFACE_TO_INTERFACE,
                "Interface",
                "DInterface",
                "DInterfaceCorr",
            ),
        ],
    }
}
