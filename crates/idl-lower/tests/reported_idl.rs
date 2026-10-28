// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL from field reports, taken verbatim.
//!
//! Each fixture here is a paste from a bug report rather than something
//! written to exercise a rule. A rule-shaped fixture tests what the
//! author thought of; a reported one tests what somebody actually wrote.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Extensibility, TypeKind};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::lower;
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    let mut ast = lower(&parsed.syntax(), "reported.idl").ast;
    // A name written before its declaration is pending until this runs;
    // a consumer that skips it sees an incomplete model.
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.is_clean(),
        "the resolve pass broke references: {:?}",
        report.broken
    );
    ast
}

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

fn find(ast: &Ast, segments: &[&str]) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&scoped(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{segments:?} is not in the model"))
}

/// The IDL from GitHub issues #41 and #42, character for character
/// apart from the leading `#pragma once` the preprocessor consumes.
const ROBOT: &str = "\
module common {
    module util {
        struct Location {
            double x;
            double y;
            double z;
        };

        struct Orientation {
            double x;
            double y;
            double z;
        };
    };

    struct Robot {
        ::common::util::Location location;
        ::common::util::Orientation orientation;
    };
};
";

#[test]
fn a_fully_qualified_scoped_name_resolves() {
    // `::common::util::Location` — the root-anchored form of OMG IDL
    // 4.2 §7.5.2. Fast DDS's own generator rejects it:
    //
    //   gen.idl:17:8: error: '::' was not defined previously
    //
    // measured with fastddsgen on 2026-08-09. It is valid IDL, so it
    // resolves here.
    let ast = model(ROBOT);
    let robot = find(&ast, &["common", "Robot"]);
    let members = ast.members_of(robot);
    assert_eq!(members.len(), 2);

    let location = ast.member(members[0]).expect("location");
    let target = location
        .type_
        .resolved()
        .expect("`::common::util::Location` resolved");
    assert_eq!(
        ast.type_(target).expect("node").name,
        "Location",
        "the member points at the type the root-anchored name names"
    );
}

#[test]
fn an_unannotated_struct_is_appendable() {
    // XTypes 1.3 §7.3.1.2.1.8: "In the event that the representation of
    // a given type does not indicate the type's extensibility kind, the
    // type shall be considered appendable."
    //
    // The vendors do not agree, which is what issues #41 and #42 are.
    // Measured on 2026-08-09 for this exact IDL, XCDR2 little-endian:
    //
    //   cyclone  48 bytes, no delimiter anywhere  (final)
    //   rti      60 bytes, three DHEADERs         (appendable)
    //
    // Cyclone's own `idlc` says so in a warning it prints for this
    // file: "Currently the default extensibility for these types is
    // 'final', but this may change to 'appendable' in a future release
    // because that is the default in the DDS XTypes specification."
    //
    // The model follows the specification. `DefaultExtensibility` in
    // the lowering options is how a deployment talking to Cyclone
    // selects the other reading; §7.3.1.2.1.8 sanctions exactly that.
    let ast = model(ROBOT);
    for path in [
        &["common", "Robot"][..],
        &["common", "util", "Location"][..],
        &["common", "util", "Orientation"][..],
    ] {
        let id = find(&ast, path);
        assert_eq!(
            ast.effective_extensibility(id),
            Extensibility::Appendable,
            "{path:?} with no annotation"
        );
    }
}

#[test]
fn the_nested_structs_are_three_doubles_each() {
    let ast = model(ROBOT);
    for name in ["Location", "Orientation"] {
        let id = find(&ast, &["common", "util", name]);
        let members = ast.members_of(id);
        assert_eq!(members.len(), 3, "{name} member count");
        for member in members {
            let node = ast.member(*member).expect("member");
            let target = node.type_.resolved().expect("resolved");
            assert!(
                matches!(
                    ast.type_(target).expect("node").kind,
                    TypeKind::Primitive(zerodds_ast::PrimitiveKind::Float64)
                ),
                "{name}.{} is a double",
                node.name
            );
        }
    }
}
