// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Zig's own
//! tree. Reading this against `rule/go.rs` and `rule/c.rs` is the point,
//! because Zig answers four of the same questions differently:
//!
//! - A module becomes a **name prefix**, as in C and Go. A Zig file is a
//!   struct and does not nest (Zig language reference, "struct"), so
//!   nested IDL modules cannot nest inside one generated file;
//!   `::common::util::Location` is `common_util_Location`. Unlike Go,
//!   nothing is capitalised on the way: Zig has no export-by-case rule,
//!   so a rename here would be a rename with no cause.
//! - An enumeration becomes **one declaration**, not a type and a run of
//!   constants. Zig's `enum(i32) { … }` holds its variants, so
//!   `enum-to-enum` creates the enumerators inside the type it created —
//!   where C and Go both have to put them at file scope and prefix them
//!   with the module to keep two enumerations apart.
//! - A **bitmask still needs file-scope constants**, because its Zig
//!   form is an unsigned integer and not an enumeration; `bit-to-const`
//!   is therefore a rule and `enumerator-to-variant` is a different one,
//!   although the IDL constructs look alike.
//! - An **interface becomes a struct of function pointers**. Zig can
//!   write one — `*const fn (t: T) void` — where the C tree cannot
//!   (`coverage.rs`), so this catalogue has the rule and C's does not.
//!   It is not a Zig `union(enum)`-style vtable and it carries no
//!   calling convention; `adapter/zig.rs` records the measurement that
//!   decided that.
//!
//! A union becomes a **struct with a discriminator and one field per
//! branch**, as in Go, and *not* Zig's own tagged union. That is the
//! load-bearing decision of this column and the adapter's documentation
//! carries the reason.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "zig";

/// The name of the rule that maps a struct to a Zig struct type.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a Zig field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a Zig enum type.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to a variant of that
/// type.
///
/// A variant and not a file-scope constant: Zig's enumeration holds its
/// own names, so `Mode.IDLE` is reachable without either a prefix or a
/// second declaration.
pub const ENUMERATOR_TO_VARIANT: &str = "enumerator-to-variant";

/// The name of the rule that maps a typedef to a Zig declaration.
pub const ALIAS_TO_CONST: &str = "alias-to-const";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_STRUCT: &str = "union-to-struct";

/// The name of the rule that maps a union case to a field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an unsigned type and its
/// bits.
pub const BITMASK_TO_INT: &str = "bitmask-to-int";

/// The name of the rule that maps a bit to a file-scope constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a Zig constant.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to an opaque
/// pointer.
pub const NATIVE_TO_OPAQUE: &str = "native-to-opaque";

/// The name of the rule that maps an interface to a struct of function
/// pointers.
pub const INTERFACE_TO_STRUCT: &str = "interface-to-struct";

/// Every Zig rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_STRUCT, "Structure", "ZigStruct", "ZigStructCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "ZigStructCorr"),
                ("ZigField", "ZigStruct", "ZigFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "ZigEnum", "ZigEnumCorr"),
            part_of(
                ENUMERATOR_TO_VARIANT,
                ("Enumerator", "Enumeration", "ZigEnumCorr"),
                ("ZigVariant", "ZigEnum", "ZigVariantCorr"),
            ),
            one_to_one(ALIAS_TO_CONST, "Alias", "ZigAlias", "ZigAliasCorr"),
            one_to_one(UNION_TO_STRUCT, "Union", "ZigUnion", "ZigUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "ZigUnionCorr"),
                ("ZigCase", "ZigUnion", "ZigCaseCorr"),
            ),
            one_to_one(BITMASK_TO_INT, "Bitmask", "ZigBitmask", "ZigBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "ZigBitmaskCorr"),
                ("ZigBitConst", "ZigBitmask", "ZigBitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "ZigBitset", "ZigBitsetCorr"),
            one_to_one(VALUETYPE_TO_STRUCT, "Valuetype", "ZigValue", "ZigValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "ZigModule", "ZigModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "ZigConst", "ZigConstCorr"),
            one_to_one(EXCEPTION_TO_STRUCT, "Exception", "ZigError", "ZigErrorCorr"),
            one_to_one(NATIVE_TO_OPAQUE, "Native", "ZigNative", "ZigNativeCorr"),
            one_to_one(
                INTERFACE_TO_STRUCT,
                "Interface",
                "ZigInterface",
                "ZigInterfaceCorr",
            ),
        ],
    }
}
