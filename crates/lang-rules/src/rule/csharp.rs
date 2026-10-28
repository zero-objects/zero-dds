// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# rule catalogue. Spec §2.
//!
//! The same constructs the Java catalogue covers, mapped onto C#'s own
//! tree. The rules are the same two shapes; the diff against
//! [`crate::rule::java`] is the whole point of the rules being data, and
//! four of its lines are where C# and Java genuinely part company:
//!
//! | construct | Java | C# |
//! |---|---|---|
//! | `module` | `module-to-class`, nested in one outer class | `module-to-namespace` |
//! | `enum` | `enum-to-class`, `int` constants | `enum-to-enum`, a real `enum : int` |
//! | `bitmask` | `bitmask-to-class`, integer constants | `bitmask-to-flags-enum` |
//! | `typedef` | `alias-to-comment` | `alias-to-using` |
//!
//! Each of the four is a C# form Java has no counterpart for. Everything
//! else maps the same way and the rule names say so.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "csharp";

/// The rule a struct's declaration comes from.
pub const STRUCT_TO_CLASS: &str = "struct-to-class";

/// The rule a struct member's field comes from.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// Every C# rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_CLASS,
                "Structure",
                "CSharpClass",
                "CSharpClassCorr",
            ),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "CSharpClassCorr"),
                ("CSharpField", "CSharpClass", "CSharpFieldCorr"),
            ),
            // A real `enum`, which the Java column could not use: a Java
            // enum constant's identity is its ordinal, and a C# enum
            // member's is the integer it is given — so `@value(-1)` and
            // a value the reader's IDL never listed both hold.
            one_to_one(
                "enum-to-enum",
                "Enumeration",
                "CSharpEnum",
                "CSharpEnumCorr",
            ),
            part_of(
                "enumerator-to-member",
                ("Enumerator", "Enumeration", "CSharpEnumCorr"),
                ("CSharpEnumMember", "CSharpEnum", "CSharpEnumMemberCorr"),
            ),
            // `using X = T;` — C# 12 aliases any type, arrays included,
            // so a typedef has a form here where Java had only a comment.
            one_to_one("alias-to-using", "Alias", "CSharpAlias", "CSharpAliasCorr"),
            one_to_one("union-to-class", "Union", "CSharpUnion", "CSharpUnionCorr"),
            part_of(
                "case-to-field",
                ("Case", "Union", "CSharpUnionCorr"),
                ("CSharpCase", "CSharpUnion", "CSharpCaseCorr"),
            ),
            // `[Flags] enum X : ulong` — the language's own form for a
            // set of bits, typed rather than a bare integer.
            one_to_one(
                "bitmask-to-flags-enum",
                "Bitmask",
                "CSharpBitmask",
                "CSharpBitmaskCorr",
            ),
            part_of(
                "bit-to-member",
                ("BitValue", "Bitmask", "CSharpBitmaskCorr"),
                ("CSharpBitMember", "CSharpBitmask", "CSharpBitCorr"),
            ),
            one_to_one(
                "bitset-to-class",
                "Bitset",
                "CSharpBitset",
                "CSharpBitsetCorr",
            ),
            one_to_one(
                "valuetype-to-class",
                "Valuetype",
                "CSharpValue",
                "CSharpValueCorr",
            ),
            // A namespace and not a nested type: C# lets a file declare
            // many top-level types and does not tie the file's name to
            // one of them, so the scope IDL declares is a scope here too.
            one_to_one(
                "module-to-namespace",
                "Module",
                "CSharpNamespace",
                "CSharpNamespaceCorr",
            ),
            one_to_one("const-to-const", "Const", "CSharpConst", "CSharpConstCorr"),
            one_to_one(
                "exception-to-class",
                "Exception",
                "CSharpError",
                "CSharpErrorCorr",
            ),
            one_to_one(
                "interface-to-interface",
                "Interface",
                "CSharpInterface",
                "CSharpInterfaceCorr",
            ),
            one_to_one(
                "native-to-alias",
                "Native",
                "CSharpNative",
                "CSharpNativeCorr",
            ),
        ],
    }
}
