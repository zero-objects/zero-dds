// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ rule catalogue. Spec §2.
//!
//! The same constructs the Rust catalogue covers, mapped onto C++'s
//! own tree. The rules are the same two shapes; what differs is the node
//! each construct becomes, which is the difference a diff of two
//! catalogues is supposed to show.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "cpp";

/// Every C++ rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a
        // mismatch rather than reading an older file as if it were
        // this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one("struct-to-class", "Structure", "CppStruct", "CppStructCorr"),
            part_of(
                "member-to-member",
                ("Member", "Structure", "CppStructCorr"),
                ("CppField", "CppStruct", "CppFieldCorr"),
            ),
            one_to_one("enum-to-enum", "Enumeration", "CppEnum", "CppEnumCorr"),
            part_of(
                "enumerator-to-enumerator",
                ("Enumerator", "Enumeration", "CppEnumCorr"),
                ("CppEnumerator", "CppEnum", "CppEnumeratorCorr"),
            ),
            one_to_one("alias-to-using", "Alias", "CppAlias", "CppAliasCorr"),
            one_to_one("union-to-class", "Union", "CppUnion", "CppUnionCorr"),
            part_of(
                "case-to-member",
                ("Case", "Union", "CppUnionCorr"),
                ("CppCase", "CppUnion", "CppCaseCorr"),
            ),
            one_to_one("bitmask-to-enum", "Bitmask", "CppBitmask", "CppBitmaskCorr"),
            part_of(
                "bit-to-const",
                ("BitValue", "Bitmask", "CppBitmaskCorr"),
                ("CppBitConst", "CppBitmask", "CppBitCorr"),
            ),
            one_to_one("bitset-to-class", "Bitset", "CppBitset", "CppBitsetCorr"),
            one_to_one(
                "valuetype-to-class",
                "Valuetype",
                "CppValue",
                "CppValueCorr",
            ),
            one_to_one(
                "module-to-namespace",
                "Module",
                "CppModule",
                "CppModuleCorr",
            ),
            one_to_one("const-to-const", "Const", "CppConst", "CppConstCorr"),
            one_to_one(
                "exception-to-class",
                "Exception",
                "CppError",
                "CppErrorCorr",
            ),
            one_to_one(
                "interface-to-class",
                "Interface",
                "CppInterface",
                "CppInterfaceCorr",
            ),
            one_to_one("native-to-using", "Native", "CppNative", "CppNativeCorr"),
        ],
    }
}
