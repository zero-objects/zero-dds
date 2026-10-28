// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java rule catalogue. Spec §2.
//!
//! The same constructs the Rust catalogue covers, mapped onto Java's
//! own tree. The rules are the same two shapes; what differs is the node
//! each construct becomes, which is the difference a diff of two
//! catalogues is supposed to show.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "java";

/// Every Java rule.
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
                "struct-to-class",
                "Structure",
                "JavaStruct",
                "JavaStructCorr",
            ),
            part_of(
                "member-to-field",
                ("Member", "Structure", "JavaStructCorr"),
                ("JavaField", "JavaStruct", "JavaFieldCorr"),
            ),
            one_to_one("enum-to-enum", "Enumeration", "JavaEnum", "JavaEnumCorr"),
            part_of(
                "enumerator-to-constant",
                ("Enumerator", "Enumeration", "JavaEnumCorr"),
                ("JavaEnumerator", "JavaEnum", "JavaEnumeratorCorr"),
            ),
            one_to_one("alias-to-class", "Alias", "JavaAlias", "JavaAliasCorr"),
            one_to_one("union-to-class", "Union", "JavaUnion", "JavaUnionCorr"),
            part_of(
                "case-to-field",
                ("Case", "Union", "JavaUnionCorr"),
                ("JavaCase", "JavaUnion", "JavaCaseCorr"),
            ),
            one_to_one(
                "bitmask-to-class",
                "Bitmask",
                "JavaBitmask",
                "JavaBitmaskCorr",
            ),
            part_of(
                "bit-to-const",
                ("BitValue", "Bitmask", "JavaBitmaskCorr"),
                ("JavaBitConst", "JavaBitmask", "JavaBitCorr"),
            ),
            one_to_one("bitset-to-class", "Bitset", "JavaBitset", "JavaBitsetCorr"),
            one_to_one(
                "valuetype-to-class",
                "Valuetype",
                "JavaValue",
                "JavaValueCorr",
            ),
            one_to_one(
                "module-to-package",
                "Module",
                "JavaModule",
                "JavaModuleCorr",
            ),
            one_to_one("const-to-const", "Const", "JavaConst", "JavaConstCorr"),
            one_to_one(
                "exception-to-class",
                "Exception",
                "JavaError",
                "JavaErrorCorr",
            ),
            one_to_one(
                "interface-to-interface",
                "Interface",
                "JavaInterface",
                "JavaInterfaceCorr",
            ),
            one_to_one("native-to-class", "Native", "JavaNative", "JavaNativeCorr"),
        ],
    }
}
