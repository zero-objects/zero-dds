// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Ada's own
//! tree. Reading this against `rule/go.rs` and `rule/c.rs` is the point,
//! because Ada answers four of the same questions differently:
//!
//! - A module becomes a **name prefix**, as in C and Go. Ada packages do
//!   nest, and nesting them would be the better Ada form — but a
//!   declarative part is contiguous, and an IDL module may be reopened
//!   (IDL 4.2 §7.4.2), so a dependency order over the whole model can
//!   need declarations from two modules interleaved. Flattening is what
//!   makes every such order writable.
//! - A union becomes **one record with a discriminant and a variant
//!   part**, where Go writes a struct with every branch beside every
//!   other and C overlays them. Ada 2012 RM 3.8.1 puts the branches
//!   inside the type and makes the discriminant part of the value, so
//!   the branch that is live is not a convention the codec has to keep.
//! - A typedef becomes a **subtype** and not a derived type. RM 3.2.2:
//!   a subtype is another name for a type, RM 3.4: a derived type is a
//!   new one. IDL 4.2 §7.4.1.4.3 says an alias "does not define a new
//!   type", so the first is the mapping and the second would make
//!   `Distance` and `Interfaces.Integer_32` two types the compiler keeps
//!   apart.
//! - An enumeration becomes **a type declaration and a representation
//!   clause**, which is one rule producing two declarations. Ada numbers
//!   enumeration literals 0, 1, 2 … in declaration order (RM 3.5.1) and
//!   `@value` admits gaps, so without the clause of RM 13.4 the wire
//!   numbering is simply gone. That is the single most important thing
//!   this column does and `adapter::ada` carries the detail.
//! - An **interface has a rule here** and none in C. Ada 2005 has real
//!   interface types (RM 3.9.4): a type with no components whose
//!   primitive operations are abstract, which is exactly what an IDL
//!   interface declares.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "ada";

/// The name of the rule that maps a struct to an Ada record type.
pub const STRUCT_TO_RECORD: &str = "struct-to-record";

/// The name of the rule that maps a member to a record component.
pub const MEMBER_TO_COMPONENT: &str = "member-to-component";

/// The name of the rule that maps an enumeration to an enumeration type
/// and its representation clause.
pub const ENUM_TO_TYPE: &str = "enum-to-type";

/// The name of the rule that maps an enumerator to a literal and its
/// code in the representation clause.
pub const ENUMERATOR_TO_LITERAL: &str = "enumerator-to-literal";

/// The name of the rule that maps a typedef to an Ada subtype.
pub const ALIAS_TO_SUBTYPE: &str = "alias-to-subtype";

/// The name of the rule that maps a union to a discriminated record.
pub const UNION_TO_RECORD: &str = "union-to-record";

/// The name of the rule that maps a union case to a variant arm.
pub const CASE_TO_VARIANT: &str = "case-to-variant";

/// The name of the rule that maps a bitmask to a derived modular type
/// and its bits.
pub const BITMASK_TO_TYPE: &str = "bitmask-to-type";

/// The name of the rule that maps a bit to a constant.
pub const BIT_TO_CONSTANT: &str = "bit-to-constant";

/// The name of the rule that maps a bitset to a record of fields.
pub const BITSET_TO_RECORD: &str = "bitset-to-record";

/// The name of the rule that maps a valuetype to a record.
pub const VALUETYPE_TO_RECORD: &str = "valuetype-to-record";

/// The name of the rule that maps a module to a name prefix.
pub const MODULE_TO_PREFIX: &str = "module-to-prefix";

/// The name of the rule that maps a constant to a deferred-free Ada
/// constant object.
pub const CONST_TO_CONSTANT: &str = "const-to-constant";

/// The name of the rule that maps an exception to a record.
pub const EXCEPTION_TO_RECORD: &str = "exception-to-record";

/// The name of the rule that maps a native declaration to a derived
/// type.
pub const NATIVE_TO_TYPE: &str = "native-to-type";

/// The name of the rule that maps an interface to an Ada interface type.
pub const INTERFACE_TO_INTERFACE: &str = "interface-to-interface";

/// Every Ada rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_RECORD, "Structure", "AdaRecord", "AdaRecordCorr"),
            part_of(
                MEMBER_TO_COMPONENT,
                ("Member", "Structure", "AdaRecordCorr"),
                ("AdaComponent", "AdaRecord", "AdaComponentCorr"),
            ),
            one_to_one(ENUM_TO_TYPE, "Enumeration", "AdaEnum", "AdaEnumCorr"),
            part_of(
                ENUMERATOR_TO_LITERAL,
                ("Enumerator", "Enumeration", "AdaEnumCorr"),
                ("AdaLiteral", "AdaEnum", "AdaLiteralCorr"),
            ),
            one_to_one(ALIAS_TO_SUBTYPE, "Alias", "AdaSubtype", "AdaSubtypeCorr"),
            one_to_one(UNION_TO_RECORD, "Union", "AdaUnion", "AdaUnionCorr"),
            part_of(
                CASE_TO_VARIANT,
                ("Case", "Union", "AdaUnionCorr"),
                ("AdaCase", "AdaUnion", "AdaCaseCorr"),
            ),
            one_to_one(BITMASK_TO_TYPE, "Bitmask", "AdaBitmask", "AdaBitmaskCorr"),
            part_of(
                BIT_TO_CONSTANT,
                ("BitValue", "Bitmask", "AdaBitmaskCorr"),
                ("AdaBitConst", "AdaBitmask", "AdaBitCorr"),
            ),
            one_to_one(BITSET_TO_RECORD, "Bitset", "AdaBitset", "AdaBitsetCorr"),
            one_to_one(VALUETYPE_TO_RECORD, "Valuetype", "AdaValue", "AdaValueCorr"),
            one_to_one(MODULE_TO_PREFIX, "Module", "AdaModule", "AdaModuleCorr"),
            one_to_one(CONST_TO_CONSTANT, "Const", "AdaConst", "AdaConstCorr"),
            one_to_one(EXCEPTION_TO_RECORD, "Exception", "AdaError", "AdaErrorCorr"),
            one_to_one(NATIVE_TO_TYPE, "Native", "AdaNative", "AdaNativeCorr"),
            one_to_one(
                INTERFACE_TO_INTERFACE,
                "Interface",
                "AdaInterface",
                "AdaInterfaceCorr",
            ),
        ],
    }
}
