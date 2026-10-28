// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin rule catalogue. Spec §2.
//!
//! The same constructs [`crate::rule::zig`] and [`crate::rule::java`]
//! cover, mapped onto Kotlin's own tree. Two of the four decisions that
//! module's documentation flags for Zig apply here too, and one does
//! not:
//!
//! - A module becomes a **name prefix**, as in Zig, C and Go: a Kotlin
//!   file holds one package, so nested IDL modules cannot nest inside
//!   it either. `crates/lang-rules/src/adapter/kotlin.rs`'s
//!   documentation gives the reason a probe compiling one file wants
//!   that.
//! - A bitmask still needs file-scope constants beside its `typealias`,
//!   exactly as Zig's does: `bitmask-to-alias` and `bit-to-const` are
//!   two rules for the reason the Zig catalogue's are.
//! - An enumeration becomes an `object` of constants and **not** a
//!   type-and-constants pair the way C's and Go's do, nor Zig's own
//!   `enum(i32)`. `enum-to-object` creates the enumerators inside the
//!   `object` it created, which is what lets
//!   `crate::adapter::kotlin::declare` reach them without a second
//!   declaration and without the module prefix an enumerator at file
//!   scope would need.
//!
//! An interface becomes a **class of function-typed properties**, as
//! Zig's becomes a struct of function pointers, and a union becomes a
//! **class with a discriminator and one property per branch**, as in
//! every column but Zig's own tagged-union alternative it rejected —
//! `crate::adapter::kotlin`'s documentation carries the reason once and
//! does not repeat it here.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "kotlin";

/// The name of the rule that maps a struct to a Kotlin data class.
pub const STRUCT_TO_CLASS: &str = "struct-to-class";

/// The name of the rule that maps a member to a Kotlin property.
pub const MEMBER_TO_PROPERTY: &str = "member-to-property";

/// The name of the rule that maps an enumeration to a Kotlin `object`.
pub const ENUM_TO_OBJECT: &str = "enum-to-object";

/// The name of the rule that maps an enumerator to a `const val` of
/// that `object`.
pub const ENUMERATOR_TO_CONSTANT: &str = "enumerator-to-constant";

/// The name of the rule that maps a typedef to a Kotlin `typealias`.
pub const ALIAS_TO_ALIAS: &str = "alias-to-alias";

/// The name of the rule that maps a union to a discriminated data
/// class.
pub const UNION_TO_CLASS: &str = "union-to-class";

/// The name of the rule that maps a union case to a property.
pub const CASE_TO_PROPERTY: &str = "case-to-property";

/// The name of the rule that maps a bitmask to a `typealias` and its
/// bits.
pub const BITMASK_TO_ALIAS: &str = "bitmask-to-alias";

/// The name of the rule that maps a bit to a file-scope `const val`.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a data class of
/// properties.
pub const BITSET_TO_CLASS: &str = "bitset-to-class";

/// The name of the rule that maps a valuetype to a data class.
pub const VALUETYPE_TO_CLASS: &str = "valuetype-to-class";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a Kotlin `const val`.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a data class.
pub const EXCEPTION_TO_CLASS: &str = "exception-to-class";

/// The name of the rule that maps a native declaration to a
/// `typealias`.
pub const NATIVE_TO_ALIAS: &str = "native-to-alias";

/// The name of the rule that maps an interface to a class of
/// function-typed properties.
pub const INTERFACE_TO_CLASS: &str = "interface-to-class";

/// Every Kotlin rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_CLASS,
                "Structure",
                "KotlinStruct",
                "KotlinStructCorr",
            ),
            part_of(
                MEMBER_TO_PROPERTY,
                ("Member", "Structure", "KotlinStructCorr"),
                ("KotlinProperty", "KotlinStruct", "KotlinPropertyCorr"),
            ),
            one_to_one(
                ENUM_TO_OBJECT,
                "Enumeration",
                "KotlinEnum",
                "KotlinEnumCorr",
            ),
            part_of(
                ENUMERATOR_TO_CONSTANT,
                ("Enumerator", "Enumeration", "KotlinEnumCorr"),
                ("KotlinConstant", "KotlinEnum", "KotlinConstantCorr"),
            ),
            one_to_one(ALIAS_TO_ALIAS, "Alias", "KotlinAlias", "KotlinAliasCorr"),
            one_to_one(UNION_TO_CLASS, "Union", "KotlinUnion", "KotlinUnionCorr"),
            part_of(
                CASE_TO_PROPERTY,
                ("Case", "Union", "KotlinUnionCorr"),
                ("KotlinCase", "KotlinUnion", "KotlinCaseCorr"),
            ),
            one_to_one(
                BITMASK_TO_ALIAS,
                "Bitmask",
                "KotlinBitmask",
                "KotlinBitmaskCorr",
            ),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "KotlinBitmaskCorr"),
                ("KotlinBitConst", "KotlinBitmask", "KotlinBitCorr"),
            ),
            one_to_one(
                BITSET_TO_CLASS,
                "Bitset",
                "KotlinBitset",
                "KotlinBitsetCorr",
            ),
            one_to_one(
                VALUETYPE_TO_CLASS,
                "Valuetype",
                "KotlinValue",
                "KotlinValueCorr",
            ),
            one_to_one(
                MODULE_TO_PREFIX,
                "Module",
                "KotlinModule",
                "KotlinModuleCorr",
            ),
            one_to_one(CONST_TO_CONST, "Const", "KotlinConst", "KotlinConstCorr"),
            one_to_one(
                EXCEPTION_TO_CLASS,
                "Exception",
                "KotlinError",
                "KotlinErrorCorr",
            ),
            one_to_one(
                NATIVE_TO_ALIAS,
                "Native",
                "KotlinNative",
                "KotlinNativeCorr",
            ),
            one_to_one(
                INTERFACE_TO_CLASS,
                "Interface",
                "KotlinInterface",
                "KotlinInterfaceCorr",
            ),
        ],
    }
}
