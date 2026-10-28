// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 rule catalogue.
//!
//! Almost `rule::ada`'s catalogue, over `Ada83…` ghost kinds instead of
//! `Ada…` ones. Two rules differ and one is missing entirely:
//!
//! - **`ENUM_TO_CONSTANTS`** replaces `enum-to-type`. RM83 3.5.1 gives
//!   an enumeration type real literals, and this column could declare
//!   one — but `'Enum_Rep`/`'Enum_Val`, which Ada 2012's column needs to
//!   cross between a literal and its representation-clause code, are
//!   GNAT-implemented attributes documented against later Ada, and
//!   nothing here confirms they answer under `-gnat83`. `adapter::ada83`
//!   materialises an enumerator exactly as it would a bitmask's bit: a
//!   named `Long_Integer` constant, with no enumeration type and no
//!   representation clause at all.
//! - There is **no `interface-to-…` rule**. Ada 83 has no interface
//!   type, no tagged type, no `abstract` — `adapter::ada83` refuses an
//!   IDL `interface` by name before any rule of this catalogue would be
//!   consulted for it, so no ghost kind is declared for one.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "ada83";

/// The name of the rule that maps a struct to an Ada record type.
pub const STRUCT_TO_RECORD: &str = "struct-to-record";

/// The name of the rule that maps a member to a record component.
pub const MEMBER_TO_COMPONENT: &str = "member-to-component";

/// The name of the rule that maps an enumeration to a family of named
/// constants — see the module documentation for why this replaces an
/// enumeration type and a representation clause.
pub const ENUM_TO_CONSTANTS: &str = "enum-to-constants";

/// The name of the rule that maps an enumerator to its constant.
pub const ENUMERATOR_TO_CONSTANT: &str = "enumerator-to-constant";

/// The name of the rule that maps a typedef to an Ada subtype.
pub const ALIAS_TO_SUBTYPE: &str = "alias-to-subtype";

/// The name of the rule that maps a union to a discriminated record.
pub const UNION_TO_RECORD: &str = "union-to-record";

/// The name of the rule that maps a union case to a variant arm.
pub const CASE_TO_VARIANT: &str = "case-to-variant";

/// The name of the rule that maps a bitmask to its bits' constants.
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

/// Every Ada 83 rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(
                STRUCT_TO_RECORD,
                "Structure",
                "Ada83Record",
                "Ada83RecordCorr",
            ),
            part_of(
                MEMBER_TO_COMPONENT,
                ("Member", "Structure", "Ada83RecordCorr"),
                ("Ada83Component", "Ada83Record", "Ada83ComponentCorr"),
            ),
            one_to_one(
                ENUM_TO_CONSTANTS,
                "Enumeration",
                "Ada83Enum",
                "Ada83EnumCorr",
            ),
            part_of(
                ENUMERATOR_TO_CONSTANT,
                ("Enumerator", "Enumeration", "Ada83EnumCorr"),
                ("Ada83EnumConst", "Ada83Enum", "Ada83EnumConstCorr"),
            ),
            one_to_one(
                ALIAS_TO_SUBTYPE,
                "Alias",
                "Ada83Subtype",
                "Ada83SubtypeCorr",
            ),
            one_to_one(UNION_TO_RECORD, "Union", "Ada83Union", "Ada83UnionCorr"),
            part_of(
                CASE_TO_VARIANT,
                ("Case", "Union", "Ada83UnionCorr"),
                ("Ada83Case", "Ada83Union", "Ada83CaseCorr"),
            ),
            one_to_one(
                BITMASK_TO_TYPE,
                "Bitmask",
                "Ada83Bitmask",
                "Ada83BitmaskCorr",
            ),
            part_of(
                BIT_TO_CONSTANT,
                ("BitValue", "Bitmask", "Ada83BitmaskCorr"),
                ("Ada83BitConst", "Ada83Bitmask", "Ada83BitCorr"),
            ),
            one_to_one(BITSET_TO_RECORD, "Bitset", "Ada83Bitset", "Ada83BitsetCorr"),
            one_to_one(
                VALUETYPE_TO_RECORD,
                "Valuetype",
                "Ada83Value",
                "Ada83ValueCorr",
            ),
            one_to_one(MODULE_TO_PREFIX, "Module", "Ada83Module", "Ada83ModuleCorr"),
            one_to_one(CONST_TO_CONSTANT, "Const", "Ada83Const", "Ada83ConstCorr"),
            one_to_one(
                EXCEPTION_TO_RECORD,
                "Exception",
                "Ada83Error",
                "Ada83ErrorCorr",
            ),
            one_to_one(NATIVE_TO_TYPE, "Native", "Ada83Native", "Ada83NativeCorr"),
        ],
    }
}
