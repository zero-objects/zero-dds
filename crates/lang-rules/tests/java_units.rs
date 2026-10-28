// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated Java text is cut into compilation units that each hold
//! one public top-level class (JLS §7.6): `ZdTypes`, `ZdServices` and
//! `ZdCodec`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_lang_cst::Language;
use zerodds_lang_rules::emit::interface_rust::rpc_layout_key;
use zerodds_lang_rules::generate::{java_units, source};

fn text(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    source(&ast, Language::Java, rpc_layout_key()).unwrap_or_else(|error| panic!("{error}"))
}

fn public_top_level_classes(unit: &str) -> usize {
    unit.lines()
        .filter(|line| line.starts_with("public ") && line.contains(" class "))
        .count()
}

#[test]
fn an_interface_gives_three_units_with_one_public_class_each() {
    let generated = text("module m { interface I { long f(in long a); }; };");
    let units = java_units(&generated);
    let classes: Vec<&str> = units.iter().map(|unit| unit.class).collect();
    assert_eq!(classes, ["ZdTypes", "ZdServices", "ZdCodec"], "{generated}");
    for unit in &units {
        assert_eq!(public_top_level_classes(&unit.text), 1, "{}", unit.text);
        assert!(
            unit.text.contains(&format!("class {} {{", unit.class)),
            "{}",
            unit.text
        );
    }
    // The Javadoc above `ZdServices` goes with it and not with `ZdTypes`.
    let types = &units[0].text;
    assert!(!types.trim_end().ends_with("*/"), "{types}");
    assert!(
        units[1].text.trim_start().starts_with("/**"),
        "{}",
        units[1].text
    );
}

#[test]
fn a_model_without_an_interface_has_no_services_unit() {
    let generated = text("struct S { long a; };");
    let classes: Vec<&str> = java_units(&generated).iter().map(|u| u.class).collect();
    assert_eq!(classes, ["ZdTypes", "ZdCodec"], "{generated}");
}
