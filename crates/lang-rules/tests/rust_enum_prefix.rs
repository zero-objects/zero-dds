// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! GitHub #61: an enumeration's name cut off its enumerators' Rust names
//! (`zerodds_build::Config::strip_enum_prefix`, `zerodds-idlc
//! --strip-enum-prefix`), where a valid identifier is left.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_lang_rules::adapter::cst::{RustEnumPrefixScope, enumerator_ident, strip_enum_prefix};

#[test]
fn the_name_is_matched_without_regard_to_case_and_underscores() {
    assert_eq!(strip_enum_prefix("MyEnum", "MY_ENUM_A"), Some("A"));
    assert_eq!(strip_enum_prefix("MyEnum", "MYENUM_B"), Some("B"));
    assert_eq!(strip_enum_prefix("my_enum", "MyEnum_Value"), Some("Value"));
    assert_eq!(strip_enum_prefix("Color", "COLOR__RED"), Some("RED"));
}

#[test]
fn the_name_ends_at_an_underscore_or_a_case_change() {
    assert_eq!(strip_enum_prefix("Color", "ColorRed"), Some("Red"));
    // No boundary after `COLOR`: `COLORRED` is one word.
    assert_eq!(strip_enum_prefix("Color", "COLORRED"), None);
    // `Colorful` does not begin with the word `Color`.
    assert_eq!(strip_enum_prefix("Color", "Colorful"), None);
}

#[test]
fn it_is_cut_only_where_a_valid_identifier_is_left() {
    // Nothing left.
    assert_eq!(strip_enum_prefix("MyEnum", "MY_ENUM"), None);
    assert_eq!(strip_enum_prefix("MyEnum", "MY_ENUM_"), None);
    // A leading digit.
    assert_eq!(strip_enum_prefix("MyEnum", "MY_ENUM_1"), None);
    // Another prefix.
    assert_eq!(strip_enum_prefix("MyEnum", "OTHER_A"), None);
}

#[test]
fn the_rust_name_is_the_idl_name_unless_the_switch_is_set() {
    assert_eq!(enumerator_ident("MyEnum", "MY_ENUM_A"), "MY_ENUM_A");
    {
        let _strip = RustEnumPrefixScope::enter(true);
        assert_eq!(enumerator_ident("MyEnum", "MY_ENUM_A"), "A");
        assert_eq!(enumerator_ident("MyEnum", "MY_ENUM_1"), "MY_ENUM_1");
        // What is left is escaped like any other name.
        assert_eq!(enumerator_ident("Kind", "KIND_type"), "r#type");
    }
    assert_eq!(enumerator_ident("MyEnum", "MY_ENUM_A"), "MY_ENUM_A");
}
