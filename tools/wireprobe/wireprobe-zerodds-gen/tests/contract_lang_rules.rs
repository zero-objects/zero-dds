// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-gen` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules`: `emit::encode::CodecOptions`
//! (`src/lib.rs`, `Args::codec_options`), whose one field carries the
//! `--map-length` axis to every column's codec walk, and
//! `emit::inherited_member_names` (`src/fill.rs`), with which the code that
//! fills a sample of a derived structure takes the base's members out of the
//! flat object it is given.
//!
//! Oracle: DDS-XTypes 1.3 §7.5.1.4.1.3 (a language with no inheritance of
//! its own binds the base as the first member `parent`, so a filler needs
//! the base and every member it has, the base's base's included), and the
//! two readings of a delimited `map` the clause gives (§7.4.1.1.4 by
//! equivalence to a sequence, a DHEADER and a count; rule (15) of
//! §7.4.3.5.3, a DHEADER and the pairs), of which the default is the first.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_lang_rules::emit::encode::CodecOptions;
use zerodds_lang_rules::emit::inherited_member_names;

fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl").ast;
    ast.resolve_all_by_name().unwrap();
    ast
}

#[test]
fn xtypes_7_5_1_4_1_3_a_derived_structure_names_its_base_and_every_inherited_member() {
    let ast = model(
        "struct Root { long r; };
         struct Middle : Root { long m; };
         struct Leaf : Middle { long l; };",
    );
    let id = |name: &str| ast.types().find(|(_, n)| n.name == name).unwrap().0;
    let (base, names) = inherited_member_names(&ast, id("Leaf")).unwrap();
    assert_eq!(base, id("Middle"));
    assert_eq!(names, ["r", "m"]);
    assert!(inherited_member_names(&ast, id("Root")).is_none());
}

#[test]
fn xtypes_7_4_1_1_4_the_default_map_reading_carries_the_count_beside_the_delimiter() {
    assert!(CodecOptions::default().map_length_with_delimiter);
}
