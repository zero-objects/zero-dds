// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Nim's own
//! tree. Reading this against `rule/go.rs` is the point, because Nim and
//! Go answer four of the same questions and only two the same way:
//!
//! - A module becomes a **name prefix**, as in Go and C. Nim's unit of
//!   scope is the module and a module is a file, with no syntax for
//!   nesting one inside another, so `::common::util::Location` is
//!   `common_util_Location`.
//! - A union becomes an **object with a discriminator and one
//!   `Option[T]` per branch**, where Go writes a plain field per branch.
//!   Nim has an object variant — a `case` inside an object — and this
//!   column does not use it; `adapter/nim.rs` carries the measurement
//!   that decides it.
//! - An enumeration becomes **one `enum` declaration with every ordinal
//!   written out**, where Go and C need a defined integer type and one
//!   constant per enumerator. Nim's enum fields take explicit ordinals,
//!   which is what `@value` needs — with the constraint that they must
//!   ascend, so an IDL enumeration whose values do not is refused by
//!   name.
//! - An **interface has a rule here**, as in Go and not in C, but it
//!   reaches a different shape: Nim has no interface construct at all,
//!   so an IDL interface becomes an object of procedural fields.
//! - A bitmask becomes an **unsigned alias and one constant per bit**,
//!   exactly as in Go, and for a reason that is Nim's own: Nim has a
//!   `set[T]`, and a `set` is bounded at 2^16 elements and orders its
//!   bits the way the compiler laid the base type out, not the way the
//!   declaration did.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "nim";

/// The name of the rule that maps a struct to a Nim object.
pub const STRUCT_TO_OBJECT: &str = "struct-to-object";

/// The name of the rule that maps a member to a Nim field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a Nim enum.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to an enum field.
pub const ENUMERATOR_TO_FIELD: &str = "enumerator-to-field";

/// The name of the rule that maps a typedef to a Nim type alias.
pub const ALIAS_TO_ALIAS: &str = "alias-to-alias";

/// The name of the rule that maps a union to a discriminated object.
pub const UNION_TO_OBJECT: &str = "union-to-object";

/// The name of the rule that maps a union case to an optional field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an unsigned alias and its
/// bits.
pub const BITMASK_TO_ALIAS: &str = "bitmask-to-alias";

/// The name of the rule that maps a bit to a constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to an object of fields.
pub const BITSET_TO_OBJECT: &str = "bitset-to-object";

/// The name of the rule that maps a valuetype to an object.
pub const VALUETYPE_TO_OBJECT: &str = "valuetype-to-object";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a Nim constant.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to an object.
pub const EXCEPTION_TO_OBJECT: &str = "exception-to-object";

/// The name of the rule that maps a native declaration to an alias.
pub const NATIVE_TO_ALIAS: &str = "native-to-alias";

/// The name of the rule that maps an interface to an object of
/// procedural fields.
pub const INTERFACE_TO_OBJECT: &str = "interface-to-object";

/// Every Nim rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_OBJECT, "Structure", "NimObject", "NimObjectCorr"),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "NimObjectCorr"),
                ("NimField", "NimObject", "NimFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "NimEnum", "NimEnumCorr"),
            part_of(
                ENUMERATOR_TO_FIELD,
                ("Enumerator", "Enumeration", "NimEnumCorr"),
                ("NimEnumField", "NimEnum", "NimEnumFieldCorr"),
            ),
            one_to_one(ALIAS_TO_ALIAS, "Alias", "NimAlias", "NimAliasCorr"),
            one_to_one(UNION_TO_OBJECT, "Union", "NimUnion", "NimUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "NimUnionCorr"),
                ("NimCase", "NimUnion", "NimCaseCorr"),
            ),
            one_to_one(BITMASK_TO_ALIAS, "Bitmask", "NimBitmask", "NimBitmaskCorr"),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "NimBitmaskCorr"),
                ("NimBitConst", "NimBitmask", "NimBitCorr"),
            ),
            one_to_one(BITSET_TO_OBJECT, "Bitset", "NimBitset", "NimBitsetCorr"),
            one_to_one(VALUETYPE_TO_OBJECT, "Valuetype", "NimValue", "NimValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "NimModule", "NimModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "NimConst", "NimConstCorr"),
            one_to_one(EXCEPTION_TO_OBJECT, "Exception", "NimError", "NimErrorCorr"),
            one_to_one(NATIVE_TO_ALIAS, "Native", "NimNative", "NimNativeCorr"),
            one_to_one(
                INTERFACE_TO_OBJECT,
                "Interface",
                "NimInterface",
                "NimInterfaceCorr",
            ),
        ],
    }
}
