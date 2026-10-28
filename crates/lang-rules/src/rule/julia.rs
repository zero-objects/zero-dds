// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Julia's
//! own tree. Reading this against `rule/go.rs` is the point, because the
//! two agree on three answers and differ on three:
//!
//! - A module becomes a **name prefix**, as in Go and C, and for a reason
//!   that is neither of theirs. Julia's `module` nests properly and would
//!   be the right form; a module body is one contiguous block, and a
//!   second `module A … end` in the same file rebinds the name rather
//!   than extending the first. Declarations have to be ordered by
//!   dependency here, which in general interleaves two IDL modules, so
//!   the scope goes into the name. `zerodds_lang_rules::adapter::julia`
//!   carries the measurement.
//! - A union becomes a **struct with a discriminator and one
//!   `Union{T, Nothing}` field per branch**, and not a Julia `Union` type,
//!   although Julia has one and it is a real type. An IDL discriminator
//!   may take a value no branch selects (IDL 4.2 §7.4.1.4.4.1), and a
//!   `Union{A, B}` value is one of its arms with nothing else to say.
//! - An **interface has a rule here** and none in C. Julia has no
//!   interface construct at all; what it has is an abstract type and a
//!   set of functions declared before anything implements them, which is
//!   the language's own answer and is what this rule produces.
//! - An enumeration becomes an **`@enum` with its base and every value
//!   written out**, where Go writes a defined integer type and one
//!   constant per enumerator. `@enum` is in `Base` and needs no import;
//!   the values are written because `@value` admits gaps and `@enum`
//!   numbers from zero when they are omitted.
//! - A bitmask becomes a **`const` alias to an unsigned integer and one
//!   shifted constant per bit**, and never an `@enum`: an `@enum` value
//!   is one member, and a bitmask holds several at once.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "julia";

/// The name of the rule that maps a struct to a Julia struct.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to an `@enum`.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to an `@enum` member.
pub const ENUMERATOR_TO_MEMBER: &str = "enumerator-to-member";

/// The name of the rule that maps a typedef to a `const` binding.
pub const ALIAS_TO_CONST: &str = "alias-to-const";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_STRUCT: &str = "union-to-struct";

/// The name of the rule that maps a union case to a field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an unsigned alias.
pub const BITMASK_TO_CONST: &str = "bitmask-to-const";

/// The name of the rule that maps a bit to a shifted constant.
pub const BIT_TO_CONST: &str = "bit-to-const";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a `const` binding.
pub const CONST_TO_CONST: &str = "const-to-const";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to a `const`
/// binding.
pub const NATIVE_TO_CONST: &str = "native-to-const";

/// The name of the rule that maps an interface to an abstract type.
pub const INTERFACE_TO_ABSTRACT: &str = "interface-to-abstract";

/// Every Julia rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_STRUCT,
                "Structure",
                "JuliaStruct",
                "JuliaStructCorr",
            ),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "JuliaStructCorr"),
                ("JuliaField", "JuliaStruct", "JuliaFieldCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "JuliaEnum", "JuliaEnumCorr"),
            part_of(
                ENUMERATOR_TO_MEMBER,
                ("Enumerator", "Enumeration", "JuliaEnumCorr"),
                ("JuliaEnumerator", "JuliaEnum", "JuliaEnumeratorCorr"),
            ),
            one_to_one(ALIAS_TO_CONST, "Alias", "JuliaAlias", "JuliaAliasCorr"),
            one_to_one(UNION_TO_STRUCT, "Union", "JuliaUnion", "JuliaUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "JuliaUnionCorr"),
                ("JuliaCase", "JuliaUnion", "JuliaCaseCorr"),
            ),
            one_to_one(
                BITMASK_TO_CONST,
                "Bitmask",
                "JuliaBitmask",
                "JuliaBitmaskCorr",
            ),
            part_of(
                BIT_TO_CONST,
                ("BitValue", "Bitmask", "JuliaBitmaskCorr"),
                ("JuliaBitConst", "JuliaBitmask", "JuliaBitCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "JuliaBitset", "JuliaBitsetCorr"),
            one_to_one(
                VALUETYPE_TO_STRUCT,
                "Valuetype",
                "JuliaValue",
                "JuliaValueCorr",
            ),
            one_to_one(MODULE_TO_PREFIX, "Module", "JuliaModule", "JuliaModuleCorr"),
            one_to_one(CONST_TO_CONST, "Const", "JuliaConst", "JuliaConstCorr"),
            one_to_one(
                EXCEPTION_TO_STRUCT,
                "Exception",
                "JuliaError",
                "JuliaErrorCorr",
            ),
            one_to_one(NATIVE_TO_CONST, "Native", "JuliaNative", "JuliaNativeCorr"),
            one_to_one(
                INTERFACE_TO_ABSTRACT,
                "Interface",
                "JuliaInterface",
                "JuliaInterfaceCorr",
            ),
        ],
    }
}
