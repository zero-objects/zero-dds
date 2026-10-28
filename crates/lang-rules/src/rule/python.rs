// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python rule catalogue. Spec §2.
//!
//! The same constructs the Rust, C, C++ and Java catalogues cover,
//! mapped onto Python's own tree. The rules are the same two shapes;
//! what differs is the node each construct becomes, which is the
//! difference a diff of two catalogues is supposed to show.
//!
//! Two rules are named for what Python actually does rather than for
//! what the sibling catalogues do, because the target is not the same
//! shape:
//!
//! - `enum-to-intenum` and `bitmask-to-intflag`. Java has no rule of
//!   that name because a Java `enum` constant's identity is its ordinal;
//!   Python's `enum.IntEnum` member *is* an `int`, so the enumerator
//!   names and the values live in one declaration.
//! - `module-to-class`. Python's own unit of scope is the module, and
//!   one IDL file becomes one Python module, so a nested IDL module has
//!   to become something inside it. A class is the only construct Python
//!   has that nests and carries a name.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "python";

/// Every Python rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a
        // mismatch rather than reading an older file as if it were
        // this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                "struct-to-dataclass",
                "Structure",
                "PythonStruct",
                "PythonStructCorr",
            ),
            part_of(
                "member-to-field",
                ("Member", "Structure", "PythonStructCorr"),
                ("PythonField", "PythonStruct", "PythonFieldCorr"),
            ),
            one_to_one(
                "enum-to-intenum",
                "Enumeration",
                "PythonEnum",
                "PythonEnumCorr",
            ),
            part_of(
                "enumerator-to-member",
                ("Enumerator", "Enumeration", "PythonEnumCorr"),
                ("PythonEnumerator", "PythonEnum", "PythonEnumeratorCorr"),
            ),
            one_to_one(
                "alias-to-comment",
                "Alias",
                "PythonAlias",
                "PythonAliasCorr",
            ),
            one_to_one(
                "union-to-dataclass",
                "Union",
                "PythonUnion",
                "PythonUnionCorr",
            ),
            part_of(
                "case-to-field",
                ("Case", "Union", "PythonUnionCorr"),
                ("PythonCase", "PythonUnion", "PythonCaseCorr"),
            ),
            one_to_one(
                "bitmask-to-intflag",
                "Bitmask",
                "PythonBitmask",
                "PythonBitmaskCorr",
            ),
            part_of(
                "bit-to-member",
                ("BitValue", "Bitmask", "PythonBitmaskCorr"),
                ("PythonBitConst", "PythonBitmask", "PythonBitCorr"),
            ),
            one_to_one(
                "bitset-to-dataclass",
                "Bitset",
                "PythonBitset",
                "PythonBitsetCorr",
            ),
            one_to_one(
                "valuetype-to-dataclass",
                "Valuetype",
                "PythonValue",
                "PythonValueCorr",
            ),
            one_to_one(
                "module-to-class",
                "Module",
                "PythonModule",
                "PythonModuleCorr",
            ),
            one_to_one("const-to-final", "Const", "PythonConst", "PythonConstCorr"),
            one_to_one(
                "exception-to-dataclass",
                "Exception",
                "PythonError",
                "PythonErrorCorr",
            ),
            one_to_one(
                "interface-to-protocol",
                "Interface",
                "PythonInterface",
                "PythonInterfaceCorr",
            ),
            one_to_one(
                "native-to-comment",
                "Native",
                "PythonNative",
                "PythonNativeCorr",
            ),
        ],
    }
}
