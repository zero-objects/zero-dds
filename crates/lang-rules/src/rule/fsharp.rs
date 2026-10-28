// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# rule catalogue. Spec §2.
//!
//! The same constructs `rule/zig.rs` and `rule/go.rs` cover, mapped onto
//! F#'s own tree. Reading this beside `rule/zig.rs` is the point, because
//! the two agree on every rule's *shape* — one struct becomes one
//! aggregate, one member becomes one field, one union case becomes one
//! field of the discriminated aggregate — and differ only in which ghost
//! type each rule names; `crates/lang-rules/src/adapter/fsharp.rs`
//! carries why the aggregate itself is a class and not Zig's `struct` or
//! F#'s own discriminated union.
//!
//! - A module becomes a **name prefix**, as in Zig, C and Go. F#'s
//!   `module rec` does not nest the way IDL's `module` can, so
//!   `::common::util::Location` is `common_util_Location`.
//! - An enumeration becomes **one declaration** holding its cases, as in
//!   Zig's `enum(i32) { … }` — F#'s CLI enum syntax holds its cases the
//!   same way, so `enum-to-enum` and `enumerator-to-variant` read exactly
//!   as Zig's do.
//! - A **bitmask still needs file-scope constants**, because its F# form
//!   is a type abbreviation and not an enumeration — `bit-to-const` is a
//!   rule of its own, as it is in Zig.
//! - An **interface becomes a class of function-valued fields**, F#'s
//!   answer to Zig's "struct of function pointers": a first-class
//!   function value is a real F# field the way a function pointer is a
//!   real Zig one.
//!
//! A union becomes a **class with a discriminator and one field per
//! branch**, as in Zig's struct-not-tagged-union and Go's struct, and not
//! F#'s own discriminated union — the load-bearing decision, and
//! `crates/lang-rules/src/adapter/fsharp.rs`'s documentation carries the
//! reason.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "fsharp";

/// The name of the rule that maps a struct to an F# class type.
pub const STRUCT_TO_CLASS: &str = "struct-to-class";

/// The name of the rule that maps a member to a class field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to an F# enum type.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to a case of that type.
pub const ENUMERATOR_TO_VARIANT: &str = "enumerator-to-variant";

/// The name of the rule that maps a typedef to an F# type abbreviation.
pub const ALIAS_TO_ABBREVIATION: &str = "alias-to-abbreviation";

/// The name of the rule that maps a union to a discriminated class.
pub const UNION_TO_CLASS: &str = "union-to-class";

/// The name of the rule that maps a union case to a field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an unsigned type
/// abbreviation and its bits.
pub const BITMASK_TO_INT: &str = "bitmask-to-int";

/// The name of the rule that maps a bit to a file-scope constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a class of fields.
pub const BITSET_TO_CLASS: &str = "bitset-to-class";

/// The name of the rule that maps a valuetype to a class.
pub const VALUETYPE_TO_CLASS: &str = "valuetype-to-class";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to an F# `let` binding.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a class.
pub const EXCEPTION_TO_CLASS: &str = "exception-to-class";

/// The name of the rule that maps a native declaration to an opaque
/// mapping.
pub const NATIVE_TO_OPAQUE: &str = "native-to-opaque";

/// The name of the rule that maps an interface to a class of
/// function-valued fields.
pub const INTERFACE_TO_CLASS: &str = "interface-to-class";

/// Every F# rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_CLASS, "Structure", "FsClass", "FsClassCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "FsClassCorr"),
                ("FsField", "FsClass", "FsFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "FsEnum", "FsEnumCorr"),
            part_of(
                ENUMERATOR_TO_VARIANT,
                ("Enumerator", "Enumeration", "FsEnumCorr"),
                ("FsVariant", "FsEnum", "FsVariantCorr"),
            ),
            one_to_one(ALIAS_TO_ABBREVIATION, "Alias", "FsAlias", "FsAliasCorr"),
            one_to_one(UNION_TO_CLASS, "Union", "FsUnion", "FsUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "FsUnionCorr"),
                ("FsCase", "FsUnion", "FsCaseCorr"),
            ),
            one_to_one(BITMASK_TO_INT, "Bitmask", "FsBitmask", "FsBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "FsBitmaskCorr"),
                ("FsBitConst", "FsBitmask", "FsBitCorr"),
            ),
            one_to_one(BITSET_TO_CLASS, "Bitset", "FsBitset", "FsBitsetCorr"),
            one_to_one(VALUETYPE_TO_CLASS, "Valuetype", "FsValue", "FsValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "FsModule", "FsModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "FsConst", "FsConstCorr"),
            one_to_one(EXCEPTION_TO_CLASS, "Exception", "FsError", "FsErrorCorr"),
            one_to_one(NATIVE_TO_OPAQUE, "Native", "FsNative", "FsNativeCorr"),
            one_to_one(
                INTERFACE_TO_CLASS,
                "Interface",
                "FsInterface",
                "FsInterfaceCorr",
            ),
        ],
    }
}
