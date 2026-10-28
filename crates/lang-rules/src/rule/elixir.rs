// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Elixir's
//! own tree. Reading this against `rule/go.rs` is the point, because the
//! two answer four of the same questions and only one of them the same
//! way:
//!
//! - A module becomes a **module name segment**, where Go has to
//!   flatten. An Elixir module name is an atom with dotted segments —
//!   `Common.Util.Location` — and that nesting is the language's own, so
//!   `module-to-module` here is a real scope rule and `module-to-prefix`
//!   in C and Go is a name rule standing in for one.
//! - A union becomes a **struct with a discriminator and one nullable
//!   field per branch**, as in Go and for a different reason: Go has no
//!   overlaid storage, and Elixir has no tagged form that can hold a
//!   discriminator value no branch selects (IDL 4.2 §7.4.1.4.4.1 admits
//!   one).
//! - An **enumeration becomes a module**, not a type: Elixir has no
//!   enumeration construct at all. The module carries a `@type t` of
//!   atoms and one function per enumerator, with every value written
//!   out, because `@value` admits gaps.
//! - An **interface has a rule here** and none in C. An Elixir
//!   behaviour is a module of `@callback` declarations, which is exactly
//!   what an IDL interface declares.
//!
//! One rule has no counterpart in any sibling: `const-to-function`. A
//! bare module attribute is a compile-time binding and is gone
//! afterwards, so a constant that is only an attribute is invisible to
//! every other module. The rule creates the attribute and the zero-arity
//! function that reads it.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "elixir";

/// The name of the rule that maps a struct to a module with a
/// `defstruct`.
pub const STRUCT_TO_MODULE: &str = "struct-to-module";

/// The name of the rule that maps a member to a struct field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a module of atoms.
pub const ENUM_TO_MODULE: &str = "enum-to-module";

/// The name of the rule that maps an enumerator to a function.
pub const ENUMERATOR_TO_FUNCTION: &str = "enumerator-to-function";

/// The name of the rule that maps a typedef to a `@type` alias.
pub const ALIAS_TO_TYPE: &str = "alias-to-type";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_MODULE: &str = "union-to-module";

/// The name of the rule that maps a union case to a nullable field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to a module of bit
/// constants.
pub const BITMASK_TO_MODULE: &str = "bitmask-to-module";

/// The name of the rule that maps a bit to a function.
pub const BIT_TO_FUNCTION: &str = "bit-to-function";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_MODULE: &str = "bitset-to-module";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_MODULE: &str = "valuetype-to-module";

/// The name of the rule that maps an IDL module to a module name
/// segment.
pub const MODULE_TO_MODULE: &str = "module-to-module";

/// The name of the rule that maps a constant to an attribute and the
/// function that reads it.
pub const CONST_TO_FUNCTION: &str = "const-to-function";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_MODULE: &str = "exception-to-module";

/// The name of the rule that maps a native declaration to an opaque
/// `@type`.
pub const NATIVE_TO_TYPE: &str = "native-to-type";

/// The name of the rule that maps an interface to a behaviour.
pub const INTERFACE_TO_BEHAVIOUR: &str = "interface-to-behaviour";

/// Every Elixir rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_MODULE,
                "Structure",
                "ElixirStruct",
                "ElixirStructCorr",
            ),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "ElixirStructCorr"),
                ("ElixirField", "ElixirStruct", "ElixirFieldCorr"),
            ),
            one_to_one(
                ENUM_TO_MODULE,
                "Enumeration",
                "ElixirEnum",
                "ElixirEnumCorr",
            ),
            part_of(
                ENUMERATOR_TO_FUNCTION,
                ("Enumerator", "Enumeration", "ElixirEnumCorr"),
                ("ElixirEnumerator", "ElixirEnum", "ElixirEnumeratorCorr"),
            ),
            one_to_one(ALIAS_TO_TYPE, "Alias", "ElixirAlias", "ElixirAliasCorr"),
            one_to_one(UNION_TO_MODULE, "Union", "ElixirUnion", "ElixirUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "ElixirUnionCorr"),
                ("ElixirCase", "ElixirUnion", "ElixirCaseCorr"),
            ),
            one_to_one(
                BITMASK_TO_MODULE,
                "Bitmask",
                "ElixirBitmask",
                "ElixirBitmaskCorr",
            ),
            part_of(
                BIT_TO_FUNCTION,
                ("BitValue", "Bitmask", "ElixirBitmaskCorr"),
                ("ElixirBit", "ElixirBitmask", "ElixirBitCorr"),
            ),
            one_to_one(
                BITSET_TO_MODULE,
                "Bitset",
                "ElixirBitset",
                "ElixirBitsetCorr",
            ),
            one_to_one(
                VALUETYPE_TO_MODULE,
                "Valuetype",
                "ElixirValue",
                "ElixirValueCorr",
            ),
            one_to_one(
                MODULE_TO_MODULE,
                "Module",
                "ElixirModule",
                "ElixirModuleCorr",
            ),
            one_to_one(CONST_TO_FUNCTION, "Const", "ElixirConst", "ElixirConstCorr"),
            one_to_one(
                EXCEPTION_TO_MODULE,
                "Exception",
                "ElixirError",
                "ElixirErrorCorr",
            ),
            one_to_one(NATIVE_TO_TYPE, "Native", "ElixirNative", "ElixirNativeCorr"),
            one_to_one(
                INTERFACE_TO_BEHAVIOUR,
                "Interface",
                "ElixirBehaviour",
                "ElixirBehaviourCorr",
            ),
        ],
    }
}
