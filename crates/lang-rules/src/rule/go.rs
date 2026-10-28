// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Go rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Go's own
//! tree. Reading this against `rule/c.rs` is the point, because Go and C
//! answer three of the same questions differently and one the same way:
//!
//! - A module becomes a **name prefix**, as in C. Go's unit of scope is
//!   the package and a package is a directory, so nested IDL modules
//!   cannot nest inside one generated file; `::common::util::Location`
//!   is `Common_Util_Location`.
//! - A union becomes a **struct with a discriminator and one field per
//!   branch**, where C overlays the branches in a `union`. Go has no
//!   overlaid storage at all, so the branches sit side by side and `D`
//!   says which one carries the value.
//! - An **interface has a rule here** and none in C. A Go interface is a
//!   method set with names, which is exactly what an IDL interface
//!   declares; the C tree has no function-pointer type to write one
//!   with (`coverage.rs`).
//! - An enumeration becomes a **defined integer type and one constant
//!   per enumerator**, as in C and for a related reason: Go's `iota`
//!   numbers consecutively from zero, and `@value` admits gaps, so a
//!   generated `iota` block would renumber the wire.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "go";

/// The name of the rule that maps a struct to a Go struct type.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a Go field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a defined integer
/// type and its constants.
pub const ENUM_TO_TYPE: &str = "enum-to-type";

/// The name of the rule that maps an enumerator to a constant.
pub const ENUMERATOR_TO_CONST: &str = "enumerator-to-const";

/// The name of the rule that maps a typedef to a Go type alias.
pub const ALIAS_TO_ALIAS: &str = "alias-to-alias";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_STRUCT: &str = "union-to-struct";

/// The name of the rule that maps a union case to a field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an unsigned type and its
/// bits.
pub const BITMASK_TO_TYPE: &str = "bitmask-to-type";

/// The name of the rule that maps a bit to a constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a Go constant.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to an alias.
pub const NATIVE_TO_ALIAS: &str = "native-to-alias";

/// The name of the rule that maps an interface to a Go interface.
pub const INTERFACE_TO_INTERFACE: &str = "interface-to-interface";

/// Every Go rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_STRUCT, "Structure", "GoStruct", "GoStructCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "GoStructCorr"),
                ("GoField", "GoStruct", "GoFieldCorr"),
            ),
            one_to_one(ENUM_TO_TYPE, "Enumeration", "GoEnum", "GoEnumCorr"),
            part_of(
                ENUMERATOR_TO_CONST,
                ("Enumerator", "Enumeration", "GoEnumCorr"),
                ("GoEnumerator", "GoEnum", "GoEnumeratorCorr"),
            ),
            one_to_one(ALIAS_TO_ALIAS, "Alias", "GoAlias", "GoAliasCorr"),
            one_to_one(UNION_TO_STRUCT, "Union", "GoUnion", "GoUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "GoUnionCorr"),
                ("GoCase", "GoUnion", "GoCaseCorr"),
            ),
            one_to_one(BITMASK_TO_TYPE, "Bitmask", "GoBitmask", "GoBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "GoBitmaskCorr"),
                ("GoBitConst", "GoBitmask", "GoBitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "GoBitset", "GoBitsetCorr"),
            one_to_one(VALUETYPE_TO_STRUCT, "Valuetype", "GoValue", "GoValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "GoModule", "GoModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "GoConst", "GoConstCorr"),
            one_to_one(EXCEPTION_TO_STRUCT, "Exception", "GoError", "GoErrorCorr"),
            one_to_one(NATIVE_TO_ALIAS, "Native", "GoNative", "GoNativeCorr"),
            one_to_one(
                INTERFACE_TO_INTERFACE,
                "Interface",
                "GoInterface",
                "GoInterfaceCorr",
            ),
        ],
    }
}
