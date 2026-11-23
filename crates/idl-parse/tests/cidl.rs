// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CIDL grammar, CORBA 3.3 Part 3 §8.3, and the PSDL declarations it
//! contains, Persistent State Service 2.0 §3.2.3.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_cst::CstKind;
use zerodds_idl_parse::{Dialect, Parse, parse_with};

const EVERY_PRODUCTION: &str = include_str!("fixtures/cidl/every_production.cdl");

fn cidl() -> Dialect {
    Dialect {
        cidl: true,
        ..Dialect::ALL_VENDORS
    }
}

fn parse_cidl(source: &str) -> Parse {
    parse_with(source, cidl())
}

fn kinds(parse: &Parse) -> Vec<CstKind> {
    parse
        .syntax()
        .descendants()
        .iter()
        .map(|node| node.kind())
        .collect()
}

fn count(parse: &Parse, kind: CstKind) -> usize {
    kinds(parse).iter().filter(|k| **k == kind).count()
}

#[test]
fn corba_p3_8_3_every_production_parses() {
    let parsed = parse_cidl(EVERY_PRODUCTION);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.text(), EVERY_PRODUCTION, "the round trip");
    let found = kinds(&parsed);
    for kind in [
        CstKind::AbstractStorageTypeDecl,
        CstKind::AbstractStorageHomeDecl,
        CstKind::StorageTypeDecl,
        CstKind::StorageHomeDecl,
        CstKind::OfClause,
        CstKind::ImplementsClause,
        CstKind::StateDecl,
        CstKind::StorageRefType,
        CstKind::KeyDecl,
        CstKind::PsdlFactoryDecl,
        CstKind::StoreDirective,
        CstKind::RefRepDirective,
        CstKind::PrimaryKeyDecl,
        CstKind::CompositionDecl,
        CstKind::HomeExecutorDef,
        CstKind::HomeImplDecl,
        CstKind::StorageHomeBinding,
        CstKind::PersistenceDecl,
        CstKind::ExecutorDef,
        CstKind::SegmentDef,
        CstKind::FacetDecl,
        CstKind::FeatureDelegationSpec,
        CstKind::FeatureDelegation,
        CstKind::DelegationSpec,
        CstKind::Delegation,
        CstKind::AbstractSpec,
        CstKind::ProxyHomeDef,
    ] {
        assert!(found.contains(&kind), "no {kind:?} in the tree");
    }
    assert_eq!(
        count(&parsed, CstKind::CompositionDecl),
        4,
        "the four categories"
    );
    assert_eq!(count(&parsed, CstKind::SegmentDef), 2);
    assert_eq!(count(&parsed, CstKind::FeatureDelegation), 2);
    // Two storage home delegations, one executor delegation, one home
    // delegation.
    assert_eq!(count(&parsed, CstKind::Delegation), 4);
    assert_eq!(count(&parsed, CstKind::DelegationSpec), 3);
    // `abstract storagetype Ledger;` and `abstract storagehome
    // LedgerHome;` are forward declarations, `storagetype LedgerImpl;`
    // too: three nodes of the kind have a body, two do not.
    assert_eq!(count(&parsed, CstKind::AbstractStorageTypeDecl), 3);
    assert_eq!(count(&parsed, CstKind::StorageHomeDecl), 3);
}

#[test]
fn corba_p3_8_2_1_keywords_are_identifiers_outside_cidl_and_escapable_inside() {
    let source = "struct state { long key; long entity; string scope; };";
    let idl = parse_with(source, Dialect::ALL_VENDORS);
    assert!(
        !idl.has_errors(),
        "IDL keeps the words: {:?}",
        idl.diagnostics()
    );
    assert_eq!(count(&idl, CstKind::CompositionDecl), 0);

    // In CIDL every one of them is a keyword.
    let in_cidl = parse_cidl(source);
    assert!(in_cidl.has_errors(), "`state` is a keyword in CIDL");
    let message = &in_cidl.diagnostics()[0].message;
    assert!(
        message.contains("keyword") || message.contains("expected"),
        "{message}"
    );

    // §8.2.1: "unless escaped with a leading underscore".
    let escaped = parse_cidl("struct _state { long _key; long _entity; string _scope; };");
    assert!(!escaped.has_errors(), "{:?}", escaped.diagnostics());

    // The spelling is the table's: `bindsto` is an identifier, `bindsTo`
    // the keyword.
    let spelling = parse_cidl("struct S { long bindsto; };");
    assert!(!spelling.has_errors(), "{:?}", spelling.diagnostics());
    assert!(parse_cidl("struct S { long bindsTo; };").has_errors());
}

#[test]
fn corba_p3_8_14_both_spellings_of_a_feature_delegation_parse() {
    // Production (20) writes `delegatesTo storage`; the text of §8.14
    // names the keywords `delegatesTo`, `abstract` and `storagetype`.
    for spelling in ["storage", "abstract storagetype"] {
        let source = format!(
            "composition process C {{ home executor H {{ implements M::H; bindsTo M::S;
               manages E {{ delegatesTo {spelling} (a : b, c : d) }}; }}; }};"
        );
        let parsed = parse_cidl(&source);
        assert!(
            !parsed.has_errors(),
            "{spelling}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(
            count(&parsed, CstKind::FeatureDelegationSpec),
            1,
            "{spelling}"
        );
        assert_eq!(count(&parsed, CstKind::FeatureDelegation), 2, "{spelling}");
    }
}

#[test]
fn corba_p3_9_2_9_1_the_schematic_segment_form_parses_too() {
    // §9.2.9.1 gives `provides ( a, b );` and `};` after a segment.
    let parsed = parse_cidl(
        "composition entity C { home executor H { implements M::H;
           manages E { segment S { storedOn M::SH; provides (a, b); }; }; }; };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(count(&parsed, CstKind::FacetDecl), 1);
    let facet = parsed
        .syntax()
        .descendants()
        .into_iter()
        .find(|n| n.kind() == CstKind::FacetDecl)
        .unwrap();
    let declarators = facet
        .children()
        .into_iter()
        .filter(|n| n.kind() == CstKind::Declarator)
        .count();
    assert_eq!(declarators, 2);
}

#[test]
fn corba_p3_8_5_a_composition_needs_its_category_and_its_home_executor() {
    let no_category =
        parse_cidl("composition C { home executor H { implements M::H; manages E; }; };");
    assert!(no_category.has_errors());
    assert!(
        no_category.diagnostics()[0]
            .message
            .contains("life cycle category"),
        "{:?}",
        no_category.diagnostics()
    );
    let no_home = parse_cidl("composition session C { proxy home P { abstract (a); }; };");
    assert!(no_home.has_errors());
    assert!(
        no_home
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("home executor")),
        "{:?}",
        no_home.diagnostics()
    );
    // §9.2.7's catalogs are PSS 1.0 and in neither grammar.
    let catalog = parse_cidl(
        "composition entity C { uses catalog { K store; }; home executor H {
           implements M::H; manages E; }; };",
    );
    assert!(catalog.has_errors());
    // The round trip holds for a broken file too.
    assert_eq!(
        catalog.text(),
        "composition entity C { uses catalog { K store; }; home executor H {
           implements M::H; manages E; }; };"
    );
}

#[test]
fn pss_3_2_5_10_an_anonymous_sequence_is_no_state_type() {
    let parsed = parse_cidl("abstract storagetype T { state sequence<long> s; };");
    assert!(parsed.has_errors());
    assert!(
        parsed.diagnostics()[0].spec_reference.contains("§3.2.5.10"),
        "{:?}",
        parsed.diagnostics()
    );
    let named =
        parse_cidl("typedef sequence<long> Longs; abstract storagetype T { state Longs s; };");
    assert!(!named.has_errors(), "{:?}", named.diagnostics());
}

#[test]
fn pss_3_2_5_4_const_is_for_a_storage_type_operation() {
    let home = parse_cidl("abstract storagehome H of T { long f() const; };");
    assert!(home.has_errors());
    let type_ = parse_cidl("abstract storagetype T { long f() const; };");
    assert!(!type_.has_errors(), "{:?}", type_.diagnostics());
}

/// The diagnostics of `parse` citing `clause`.
fn citing<'a>(parse: &'a Parse, clause: &str) -> Vec<&'a zerodds_idl_parse::Diagnostic> {
    parse
        .diagnostics()
        .iter()
        .filter(|d| d.spec_reference.contains(clause))
        .collect()
}

/// A composition whose home executor body is `body`.
fn home_executor(body: &str) -> String {
    format!("composition entity C {{ home executor H {{ {body} }}; }};")
}

#[test]
fn corba_p3_8_6_each_place_takes_only_its_kinds_of_delegation() {
    // Production (8): the home executor body takes `delegatesTo abstract
    // storagehome` and `delegatesTo executor`; (15): the executor body
    // `delegatesTo storage` (`abstract storagetype` in the text of
    // §8.14); (33): the proxy home `delegatesTo home`.
    let cases = [
        (
            home_executor(
                "implements M::H; bindsTo M::S; manages E; delegatesTo storage (balance: b);",
            ),
            "`delegatesTo storage` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        (
            home_executor(
                "implements M::H; manages E; delegatesTo abstract storagetype (balance: b);",
            ),
            "`delegatesTo storage` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        (
            home_executor("implements M::H; manages E; delegatesTo home (close_all: wipe);"),
            "`delegatesTo home` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        (
            home_executor("implements M::H; manages E { delegatesTo executor (bogus) };"),
            "`delegatesTo executor` does not belong in the executor body",
            "§8.10 (15)",
        ),
        (
            home_executor(
                "implements M::H; manages E { delegatesTo abstract storagehome (open: create); };",
            ),
            "`delegatesTo abstract storagehome` does not belong in the executor body",
            "§8.10 (15)",
        ),
        (
            home_executor("implements M::H; manages E { delegatesTo home (open) };"),
            "`delegatesTo home` does not belong in the executor body",
            "§8.10 (15)",
        ),
        (
            "composition session C { home executor H { implements M::H; manages E; };
               proxy home P { delegatesTo storage (balance: zzz); }; };"
                .to_string(),
            "`delegatesTo storage` does not belong in a proxy home",
            "§8.18 (33)",
        ),
        (
            "composition session C { home executor H { implements M::H; manages E; };
               proxy home P { delegatesTo executor (place); }; };"
                .to_string(),
            "`delegatesTo executor` does not belong in a proxy home",
            "§8.18 (33)",
        ),
        (
            "composition session C { home executor H { implements M::H; manages E; };
               proxy home P { delegatesTo abstract storagehome (sweep: wipe); }; };"
                .to_string(),
            "`delegatesTo abstract storagehome` does not belong in a proxy home",
            "§8.18 (33)",
        ),
    ];
    for (source, message, clause) in cases {
        let parsed = parse_cidl(&source);
        let found = citing(&parsed, clause);
        assert_eq!(found.len(), 1, "{source}: {:?}", parsed.diagnostics());
        assert!(found[0].message.contains(message), "{source}: {found:?}");
        assert_eq!(parsed.diagnostics().len(), 1, "{source}");
        // The delegation is kept in an error node, never as a member of
        // the place it does not belong in.
        for place in parsed.syntax().descendants() {
            if matches!(
                place.kind(),
                CstKind::HomeExecutorDef | CstKind::ExecutorDef | CstKind::ProxyHomeDef
            ) {
                assert!(
                    place.children().iter().all(|member| !matches!(
                        member.kind(),
                        CstKind::FeatureDelegationSpec | CstKind::DelegationSpec
                    )),
                    "{source}: a delegation is a member of {:?}",
                    place.kind()
                );
            }
        }
        assert_eq!(count(&parsed, CstKind::Error), 1, "{source}");
        assert_eq!(parsed.text(), source, "the round trip");
    }
}

#[test]
fn corba_p3_8_6_the_home_executor_body_is_read_in_the_order_of_production_8() {
    let cases = [
        // A second `bindsTo` and `manages`, `implements` after `manages`.
        (
            "manages E1; bindsTo M::T; implements M::H; bindsTo M::S; manages E2;",
            vec![
                "`bindsTo` after `manages`",
                "`implements` after `manages`",
                "`bindsTo` after `manages`",
                "a second `manages`",
            ],
        ),
        (
            "implements M::H; implements M::G; manages E;",
            vec!["a second `implements`"],
        ),
        (
            "implements M::H; storedOn M::S; bindsTo M::S; manages E;",
            vec!["`bindsTo` after `storedOn`"],
        ),
        (
            "implements M::H; manages E; abstract (a); delegatesTo executor (open);",
            vec!["`delegatesTo executor` after `abstract`"],
        ),
        (
            "implements M::H; manages E; delegatesTo executor (open);
             delegatesTo abstract storagehome (close: wipe);",
            vec!["`delegatesTo abstract storagehome` after `delegatesTo executor`"],
        ),
        (
            "implements M::H; manages E; abstract (a); abstract (b);",
            vec!["a second `abstract`"],
        ),
        // `implements` and `manages` are required.
        ("implements M::H;", vec!["has no `manages`"]),
        ("bindsTo M::S; manages E;", vec!["has no `implements`"]),
        ("", vec!["has no `implements`", "has no `manages`"]),
    ];
    for (body, expected) in cases {
        let source = home_executor(body);
        let parsed = parse_cidl(&source);
        let found = citing(&parsed, "§8.6 (8)");
        let messages: Vec<&str> = found.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages.len(), expected.len(), "{body}: {messages:?}");
        for (message, wanted) in messages.iter().zip(&expected) {
            assert!(message.contains(wanted), "{body}: {messages:?}");
        }
        assert_eq!(parsed.text(), source, "the round trip");
    }
    // A member out of its place stays in the tree, in an error node: one
    // `bindsTo` and one `manages` remain members of the body.
    let parsed = parse_cidl(&home_executor(
        "manages E1; bindsTo M::T; implements M::H; bindsTo M::S; manages E2;",
    ));
    let body = parsed
        .syntax()
        .descendants()
        .into_iter()
        .find(|n| n.kind() == CstKind::HomeExecutorDef)
        .unwrap();
    let members: Vec<CstKind> = body.children().iter().map(|n| n.kind()).collect();
    assert_eq!(
        members,
        vec![
            CstKind::ExecutorDef,
            CstKind::Error,
            CstKind::Error,
            CstKind::Error,
            CstKind::Error,
        ],
        "only the first `manages` is in its place"
    );
    // The order of production (8), every member once, is no error.
    let fine = parse_cidl(&home_executor(
        "implements M::H; bindsTo M::S; storedOn M::S; manages E;
         delegatesTo abstract storagehome (open: create); delegatesTo executor (make);
         abstract (find_by_primary_key);",
    ));
    assert!(!fine.has_errors(), "{:?}", fine.diagnostics());
}

#[test]
fn corba_p3_8_10_an_executor_body_a_segment_and_a_proxy_home_are_not_empty() {
    for (source, clause) in [
        (
            home_executor("implements M::H; manages E { };"),
            "§8.10 (14)",
        ),
        (
            home_executor("implements M::H; manages E { segment S { } };"),
            "§8.11 (16)",
        ),
        (
            "composition session C { home executor H { implements M::H; manages E; };
               proxy home P { }; };"
                .to_string(),
            "§8.18 (32)",
        ),
    ] {
        let parsed = parse_cidl(&source);
        assert_eq!(
            citing(&parsed, clause).len(),
            1,
            "{source}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(parsed.diagnostics().len(), 1, "{source}");
    }
    // Without a body, an executor is monolithic: `manages E;`.
    let monolithic = parse_cidl(&home_executor("implements M::H; manages E;"));
    assert!(!monolithic.has_errors(), "{:?}", monolithic.diagnostics());
}
