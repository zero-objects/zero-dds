// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua rule catalogue. Spec §2.
//!
//! The same constructs the other catalogues cover, mapped onto Lua's
//! tree. Reading this against `rule/go.rs` is the point, because the two
//! answer four of the same questions differently:
//!
//! - A module becomes a **nested table** where Go's becomes a name
//!   prefix. A Go package is a directory and cannot nest inside one file;
//!   a Lua table nests, so `::common::util::Location` stays
//!   `M.common.util.Location` and the IDL scope survives the mapping.
//! - A struct becomes a **descriptor and a constructor**, not a type.
//!   Lua declares nothing, so there is no node for the shape itself —
//!   what the rule creates is the table that describes it and the
//!   function that builds one.
//! - An enumeration becomes **two maps and an ordered list**, where Go
//!   gets a defined integer type and one constant per enumerator. Lua
//!   has no constant and no defined type; a name-to-value map, its
//!   inverse and the declaration order are the three facts an enumerator
//!   carries, and each is a table entry.
//! - A **typedef has a rule here and declares nothing**. Lua has no alias
//!   to write, so what the rule creates is an entry in the descriptor
//!   table naming the target — which is the only place a `typedef` is
//!   recoverable from the generated file at all.

use crate::rule::shape::{one_to_one, part_of};
use seesaw_tgg::rules::format::RuleFile;

/// The catalogue's own name, which a serialised rule file carries.
const CATALOGUE_NAME: &str = "lua";

/// The name of the rule that maps a struct to a descriptor table.
pub const STRUCT_TO_TABLE: &str = "struct-to-table";

/// The name of the rule that maps a member to a descriptor entry.
pub const MEMBER_TO_ENTRY: &str = "member-to-entry";

/// The name of the rule that maps an enumeration to a value table.
pub const ENUM_TO_TABLE: &str = "enum-to-table";

/// The name of the rule that maps an enumerator to a value entry.
pub const ENUMERATOR_TO_ENTRY: &str = "enumerator-to-entry";

/// The name of the rule that maps a typedef to a descriptor entry
/// naming its target.
pub const ALIAS_TO_ENTRY: &str = "alias-to-entry";

/// The name of the rule that maps a union to a descriptor table.
pub const UNION_TO_TABLE: &str = "union-to-table";

/// The name of the rule that maps a union case to a branch entry.
pub const CASE_TO_ENTRY: &str = "case-to-entry";

/// The name of the rule that maps a bitmask to a value table.
pub const BITMASK_TO_TABLE: &str = "bitmask-to-table";

/// The name of the rule that maps a bit to a value entry.
pub const BIT_TO_ENTRY: &str = "bit-to-entry";

/// The name of the rule that maps a bitset to a descriptor table.
pub const BITSET_TO_TABLE: &str = "bitset-to-table";

/// The name of the rule that maps a valuetype to a descriptor table.
pub const VALUETYPE_TO_TABLE: &str = "valuetype-to-table";

/// The name of the rule that maps a module to a nested table.
pub const MODULE_TO_TABLE: &str = "module-to-table";

/// The name of the rule that maps a constant to a module field.
pub const CONST_TO_FIELD: &str = "const-to-field";

/// The name of the rule that maps an exception to a descriptor table.
pub const EXCEPTION_TO_TABLE: &str = "exception-to-table";

/// The name of the rule that maps a native declaration to an opaque
/// descriptor.
pub const NATIVE_TO_TABLE: &str = "native-to-table";

/// The name of the rule that maps an interface to a table of operation
/// descriptors.
pub const INTERFACE_TO_TABLE: &str = "interface-to-table";

/// Every Lua rule.
#[must_use]
pub fn catalogue() -> RuleFile {
    RuleFile {
        // The format version the engine expects; it reports a mismatch
        // rather than reading an older file as if it were this one.
        format: 3,
        name: CATALOGUE_NAME.to_string(),
        rules: vec![
            one_to_one(STRUCT_TO_TABLE, "Structure", "LuaStruct", "LuaStructCorr"),
            part_of(
                MEMBER_TO_ENTRY,
                ("Member", "Structure", "LuaStructCorr"),
                ("LuaMember", "LuaStruct", "LuaMemberCorr"),
            ),
            one_to_one(ENUM_TO_TABLE, "Enumeration", "LuaEnum", "LuaEnumCorr"),
            part_of(
                ENUMERATOR_TO_ENTRY,
                ("Enumerator", "Enumeration", "LuaEnumCorr"),
                ("LuaEnumerator", "LuaEnum", "LuaEnumeratorCorr"),
            ),
            one_to_one(ALIAS_TO_ENTRY, "Alias", "LuaAlias", "LuaAliasCorr"),
            one_to_one(UNION_TO_TABLE, "Union", "LuaUnion", "LuaUnionCorr"),
            part_of(
                CASE_TO_ENTRY,
                ("Case", "Union", "LuaUnionCorr"),
                ("LuaCase", "LuaUnion", "LuaCaseCorr"),
            ),
            one_to_one(BITMASK_TO_TABLE, "Bitmask", "LuaBitmask", "LuaBitmaskCorr"),
            part_of(
                BIT_TO_ENTRY,
                ("BitValue", "Bitmask", "LuaBitmaskCorr"),
                ("LuaBitEntry", "LuaBitmask", "LuaBitCorr"),
            ),
            one_to_one(BITSET_TO_TABLE, "Bitset", "LuaBitset", "LuaBitsetCorr"),
            one_to_one(VALUETYPE_TO_TABLE, "Valuetype", "LuaValue", "LuaValueCorr"),
            one_to_one(MODULE_TO_TABLE, "Module", "LuaModule", "LuaModuleCorr"),
            one_to_one(CONST_TO_FIELD, "Const", "LuaConst", "LuaConstCorr"),
            one_to_one(EXCEPTION_TO_TABLE, "Exception", "LuaError", "LuaErrorCorr"),
            one_to_one(NATIVE_TO_TABLE, "Native", "LuaNative", "LuaNativeCorr"),
            one_to_one(
                INTERFACE_TO_TABLE,
                "Interface",
                "LuaInterface",
                "LuaInterfaceCorr",
            ),
        ],
    }
}
