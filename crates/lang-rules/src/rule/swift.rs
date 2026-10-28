// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Swift's
//! own tree. Reading this against `rule/go.rs` is the point, because the
//! two answer three of the same questions the same way and two of them
//! differently:
//!
//! - A module becomes a **name prefix**, as in Go and C. Swift's unit of
//!   namespacing is the *module*, which is a build-system target and not
//!   something a source file declares, so nested IDL modules cannot nest
//!   here; `::common::util::Location` is `common_util_Location`. The
//!   alternative — a caseless `enum` per module, which is Swift's own
//!   in-file namespacing idiom — is rejected for a measured reason,
//!   recorded in `crate::adapter::swift`.
//! - A union becomes a **struct with a discriminator and one optional
//!   field per branch**, as in Go. Swift *has* the shape Go lacks — an
//!   `enum` with associated values — and it is still the wrong one: an
//!   IDL discriminator may take a value no branch selects
//!   (IDL 4.2 §7.4.1.4.4.2.2, the implicit default), and an
//!   enum-with-associated-values has no state for that.
//! - An enumeration becomes a **Swift `enum` with its raw values written
//!   out**, where Go has to produce a defined integer type and a
//!   constant per enumerator. Swift's `enum` is a real nominal type with
//!   `RawValue`, so the enumeration is one declaration and not `1 + n`.
//! - A bitmask becomes an **`OptionSet`**, which is the standard
//!   library's own answer to a set of bit flags, where Go gets an
//!   unsigned type and a constant per bit.
//! - An **interface has a rule here**, as it does in Go and not in C: a
//!   Swift protocol is a set of method requirements with names, which is
//!   what an IDL interface declares.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "swift";

/// The name of the rule that maps a struct to a Swift struct.
pub const STRUCT_TO_STRUCT: &str = "struct-to-struct";

/// The name of the rule that maps a member to a stored property.
pub const MEMBER_TO_PROPERTY: &str = "member-to-property";

/// The name of the rule that maps an enumeration to a Swift enum.
pub const ENUM_TO_ENUM: &str = "enum-to-enum";

/// The name of the rule that maps an enumerator to a case.
pub const ENUMERATOR_TO_CASE: &str = "enumerator-to-case";

/// The name of the rule that maps a typedef to a Swift type alias.
pub const ALIAS_TO_TYPEALIAS: &str = "alias-to-typealias";

/// The name of the rule that maps a union to a discriminated struct.
pub const UNION_TO_STRUCT: &str = "union-to-struct";

/// The name of the rule that maps a union case to an optional property.
pub const CASE_TO_PROPERTY: &str = "case-to-property";

/// The name of the rule that maps a bitmask to an `OptionSet`.
pub const BITMASK_TO_OPTIONSET: &str = "bitmask-to-optionset";

/// The name of the rule that maps a bit to a `static let` flag.
pub const BIT_TO_FLAG: &str = "bit-to-flag";

/// The name of the rule that maps a bitset to a struct of fields.
pub const BITSET_TO_STRUCT: &str = "bitset-to-struct";

/// The name of the rule that maps a valuetype to a struct.
pub const VALUETYPE_TO_STRUCT: &str = "valuetype-to-struct";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a Swift `let`.
pub const CONST_TO_LET: &str = "const-to-let";

/// The name of the rule that maps an exception to a struct.
pub const EXCEPTION_TO_STRUCT: &str = "exception-to-struct";

/// The name of the rule that maps a native declaration to a type alias.
pub const NATIVE_TO_TYPEALIAS: &str = "native-to-typealias";

/// The name of the rule that maps an interface to a Swift protocol.
pub const INTERFACE_TO_PROTOCOL: &str = "interface-to-protocol";

/// Every Swift rule.
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
                "SwiftStruct",
                "SwiftStructCorr",
            ),
            part_of(
                MEMBER_TO_PROPERTY,
                ("Member", "Structure", "SwiftStructCorr"),
                ("SwiftProperty", "SwiftStruct", "SwiftPropertyCorr"),
            ),
            one_to_one(ENUM_TO_ENUM, "Enumeration", "SwiftEnum", "SwiftEnumCorr"),
            part_of(
                ENUMERATOR_TO_CASE,
                ("Enumerator", "Enumeration", "SwiftEnumCorr"),
                ("SwiftCase", "SwiftEnum", "SwiftCaseCorr"),
            ),
            one_to_one(
                ALIAS_TO_TYPEALIAS,
                "Alias",
                "SwiftTypealias",
                "SwiftTypealiasCorr",
            ),
            one_to_one(UNION_TO_STRUCT, "Union", "SwiftUnion", "SwiftUnionCorr"),
            part_of(
                CASE_TO_PROPERTY,
                ("Case", "Union", "SwiftUnionCorr"),
                ("SwiftBranch", "SwiftUnion", "SwiftBranchCorr"),
            ),
            one_to_one(
                BITMASK_TO_OPTIONSET,
                "Bitmask",
                "SwiftOptionSet",
                "SwiftOptionSetCorr",
            ),
            part_of(
                BIT_TO_FLAG,
                ("BitValue", "Bitmask", "SwiftOptionSetCorr"),
                ("SwiftFlag", "SwiftOptionSet", "SwiftFlagCorr"),
            ),
            one_to_one(BITSET_TO_STRUCT, "Bitset", "SwiftBitset", "SwiftBitsetCorr"),
            one_to_one(
                VALUETYPE_TO_STRUCT,
                "Valuetype",
                "SwiftValue",
                "SwiftValueCorr",
            ),
            one_to_one(MODULE_TO_PREFIX, "Module", "SwiftModule", "SwiftModuleCorr"),
            one_to_one(CONST_TO_LET, "Const", "SwiftLet", "SwiftLetCorr"),
            one_to_one(
                EXCEPTION_TO_STRUCT,
                "Exception",
                "SwiftError",
                "SwiftErrorCorr",
            ),
            one_to_one(
                NATIVE_TO_TYPEALIAS,
                "Native",
                "SwiftNative",
                "SwiftNativeCorr",
            ),
            one_to_one(
                INTERFACE_TO_PROTOCOL,
                "Interface",
                "SwiftProtocol",
                "SwiftProtocolCorr",
            ),
        ],
    }
}
