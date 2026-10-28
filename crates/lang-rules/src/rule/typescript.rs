// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript rule catalogue. Spec §2.
//!
//! The same constructs the Java catalogue covers, mapped onto
//! TypeScript's own tree. The rules are the same two shapes; what
//! differs is the node each construct becomes, and the rule names say
//! so — `alias-to-type-alias` where Java has `alias-to-class`, because
//! TypeScript has a type alias and Java does not, and
//! `union-to-discriminated-union` where Java has `union-to-class`,
//! because a literal-tagged union is a type here and a class with a
//! `_d` field everywhere else.
//!
//! A struct becomes **two** nodes, `TsStruct` and `TsStructClass`, and
//! that is not a duplicate rule: an `interface` exists only in the type
//! space and leaves nothing behind at run time, so a caller that has to
//! *create* a value needs something instantiable. Emitting only the
//! class would make every consumer depend on the implementation, which
//! is the decision `crates/dlrl-codegen/src/ts.rs` records for the DLRL
//! objects and the one this catalogue follows.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "typescript";

/// Every TypeScript rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                "struct-to-interface",
                "Structure",
                "TsStruct",
                "TsStructCorr",
            ),
            // The instantiable half. A second rule and not a second
            // node inside the first: the class is what a caller
            // constructs and the interface is what it programs against,
            // and a catalogue that could not emit one without the other
            // could not answer for a consumer that wants only types.
            one_to_one(
                "struct-to-class",
                "Structure",
                "TsStructClass",
                "TsStructClassCorr",
            ),
            part_of(
                "member-to-property",
                ("Member", "Structure", "TsStructCorr"),
                ("TsProperty", "TsStruct", "TsPropertyCorr"),
            ),
            one_to_one(
                "enum-to-const-object",
                "Enumeration",
                "TsEnum",
                "TsEnumCorr",
            ),
            part_of(
                "enumerator-to-entry",
                ("Enumerator", "Enumeration", "TsEnumCorr"),
                ("TsEnumerator", "TsEnum", "TsEnumeratorCorr"),
            ),
            one_to_one("alias-to-type-alias", "Alias", "TsAlias", "TsAliasCorr"),
            one_to_one(
                "union-to-discriminated-union",
                "Union",
                "TsUnion",
                "TsUnionCorr",
            ),
            part_of(
                "case-to-variant",
                ("Case", "Union", "TsUnionCorr"),
                ("TsCase", "TsUnion", "TsCaseCorr"),
            ),
            one_to_one(
                "bitmask-to-const-object",
                "Bitmask",
                "TsBitmask",
                "TsBitmaskCorr",
            ),
            part_of(
                "bit-to-entry",
                ("BitValue", "Bitmask", "TsBitmaskCorr"),
                ("TsBitConst", "TsBitmask", "TsBitCorr"),
            ),
            one_to_one("bitset-to-interface", "Bitset", "TsBitset", "TsBitsetCorr"),
            one_to_one(
                "valuetype-to-interface",
                "Valuetype",
                "TsValue",
                "TsValueCorr",
            ),
            one_to_one("module-to-namespace", "Module", "TsModule", "TsModuleCorr"),
            one_to_one("const-to-const", "Const", "TsConst", "TsConstCorr"),
            one_to_one(
                "exception-to-interface",
                "Exception",
                "TsError",
                "TsErrorCorr",
            ),
            one_to_one(
                "interface-to-interface",
                "Interface",
                "TsInterface",
                "TsInterfaceCorr",
            ),
            one_to_one("native-to-type-alias", "Native", "TsNative", "TsNativeCorr"),
        ],
    }
}
