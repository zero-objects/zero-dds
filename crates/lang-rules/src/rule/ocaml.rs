// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto OCaml's
//! own tree. Reading this against `rule/go.rs` is the point, because
//! OCaml answers three of the same questions differently:
//!
//! - A module becomes a **name prefix**, as in C and Go — but for a
//!   reason neither of those has. OCaml *does* have nested modules and
//!   they would be the better spelling; what rules them out is that a
//!   `type a = … and b = …` group cannot cross a `module … struct … end`
//!   boundary, and IDL puts no restriction on where a type cycle may
//!   close. `crate::adapter::ocaml` carries the argument.
//! - An enumeration becomes a **variant and a pair of conversions**.
//!   Every other column emits a defined integer type and one constant
//!   per enumerator, because in those languages an enumerator *is* a
//!   number. An OCaml variant constructor carries no number at all, so
//!   the numbering — which is the wire format (DDS-XTypes 1.3 §7.4.3.5.3)
//!   — has to be written as two functions or it is lost.
//! - A union becomes a **record with a discriminator and one `option`
//!   field per branch** and never an OCaml variant, although a variant
//!   is the shape a reader would expect. An IDL discriminator may take a
//!   value that selects no branch (IDL 4.2 §7.4.1.4.4.1, and
//!   DDS-XTypes 1.3 §7.4.1.1.5.2 for what is then written), and a
//!   variant has no value for "the discriminator is 7 and no branch
//!   matches".
//! - An **interface has a rule here**, as it does in Go: an OCaml class
//!   type is a set of named method signatures, which is what an IDL
//!   interface declares.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "ocaml";

/// The name of the rule that maps a struct to a record type.
pub const STRUCT_TO_RECORD: &str = "struct-to-record";

/// The name of the rule that maps a member to a record field.
pub const MEMBER_TO_FIELD: &str = "member-to-field";

/// The name of the rule that maps an enumeration to a variant type.
pub const ENUM_TO_VARIANT: &str = "enum-to-variant";

/// The name of the rule that maps an enumerator to a constructor.
pub const ENUMERATOR_TO_CONSTRUCTOR: &str = "enumerator-to-constructor";

/// The name of the rule that maps a typedef to a type abbreviation.
pub const ALIAS_TO_ABBREVIATION: &str = "alias-to-abbreviation";

/// The name of the rule that maps a union to a discriminated record.
pub const UNION_TO_RECORD: &str = "union-to-record";

/// The name of the rule that maps a union case to an optional field.
pub const CASE_TO_FIELD: &str = "case-to-field";

/// The name of the rule that maps a bitmask to an integer abbreviation
/// and its bits.
pub const BITMASK_TO_ABBREVIATION: &str = "bitmask-to-abbreviation";

/// The name of the rule that maps a bit to a `let` binding.
pub const BIT_TO_LET: &str = "bit-to-let";

/// The name of the rule that maps a bitset to a record of fields.
pub const BITSET_TO_RECORD: &str = "bitset-to-record";

/// The name of the rule that maps a valuetype to a record.
pub const VALUETYPE_TO_RECORD: &str = "valuetype-to-record";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a `let` binding.
pub const CONST_TO_LET: &str = "const-to-let";

/// The name of the rule that maps an exception to a record.
pub const EXCEPTION_TO_RECORD: &str = "exception-to-record";

/// The name of the rule that maps a native declaration to an opaque
/// abbreviation.
pub const NATIVE_TO_ABBREVIATION: &str = "native-to-abbreviation";

/// The name of the rule that maps an interface to a class type.
pub const INTERFACE_TO_CLASS_TYPE: &str = "interface-to-class-type";

/// Every OCaml rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_RECORD,
                "Structure",
                "OCamlRecord",
                "OCamlRecordCorr",
            ),
            part_of(
                MEMBER_TO_FIELD,
                ("Member", "Structure", "OCamlRecordCorr"),
                ("OCamlField", "OCamlRecord", "OCamlFieldCorr"),
            ),
            one_to_one(
                ENUM_TO_VARIANT,
                "Enumeration",
                "OCamlVariant",
                "OCamlVariantCorr",
            ),
            part_of(
                ENUMERATOR_TO_CONSTRUCTOR,
                ("Enumerator", "Enumeration", "OCamlVariantCorr"),
                ("OCamlConstructor", "OCamlVariant", "OCamlConstructorCorr"),
            ),
            one_to_one(
                ALIAS_TO_ABBREVIATION,
                "Alias",
                "OCamlAlias",
                "OCamlAliasCorr",
            ),
            one_to_one(UNION_TO_RECORD, "Union", "OCamlUnion", "OCamlUnionCorr"),
            part_of(
                CASE_TO_FIELD,
                ("Case", "Union", "OCamlUnionCorr"),
                ("OCamlCase", "OCamlUnion", "OCamlCaseCorr"),
            ),
            one_to_one(
                BITMASK_TO_ABBREVIATION,
                "Bitmask",
                "OCamlBitmask",
                "OCamlBitmaskCorr",
            ),
            part_of(
                BIT_TO_LET,
                ("BitValue", "Bitmask", "OCamlBitmaskCorr"),
                ("OCamlBitLet", "OCamlBitmask", "OCamlBitCorr"),
            ),
            one_to_one(BITSET_TO_RECORD, "Bitset", "OCamlBitset", "OCamlBitsetCorr"),
            one_to_one(
                VALUETYPE_TO_RECORD,
                "Valuetype",
                "OCamlValue",
                "OCamlValueCorr",
            ),
            one_to_one(MODULE_TO_PREFIX, "Module", "OCamlModule", "OCamlModuleCorr"),
            one_to_one(CONST_TO_LET, "Const", "OCamlConst", "OCamlConstCorr"),
            one_to_one(
                EXCEPTION_TO_RECORD,
                "Exception",
                "OCamlError",
                "OCamlErrorCorr",
            ),
            one_to_one(
                NATIVE_TO_ABBREVIATION,
                "Native",
                "OCamlNative",
                "OCamlNativeCorr",
            ),
            one_to_one(
                INTERFACE_TO_CLASS_TYPE,
                "Interface",
                "OCamlInterface",
                "OCamlInterfaceCorr",
            ),
        ],
    }
}
