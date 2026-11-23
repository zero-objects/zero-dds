// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lowering a CIDL file: the compositions and their rules, CORBA 3.3
//! Part 3 §8, and the PSDL declarations, Persistent State Service 2.0
//! §3.2.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::cidl::{CompositionCategory, StorageKind};
use zerodds_idl_lower::{LoweredCidl, Options, lower_cidl};
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, Dialect, parse_with};

fn cidl() -> Dialect {
    Dialect {
        cidl: true,
        ..Dialect::ALL_VENDORS
    }
}

/// Parse (which must succeed) and lower a CIDL file.
fn lowered(source: &str) -> LoweredCidl {
    let parsed = parse_with(source, cidl());
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower_cidl(
        &parsed.syntax(),
        "test.cdl",
        Options {
            dialect: cidl(),
            ..Options::default()
        },
    )
}

fn errors(lowered: &LoweredCidl) -> Vec<&Diagnostic> {
    lowered
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .collect()
}

/// The diagnostics citing `clause`.
fn citing<'a>(lowered: &'a LoweredCidl, clause: &str) -> Vec<&'a Diagnostic> {
    lowered
        .diagnostics
        .iter()
        .filter(|d| d.spec_reference.contains(clause))
        .collect()
}

/// The IDL and PSDL every composition below implements and binds to.
const BANK: &str = "
module Bank {
    exception Overdrawn {};
    exception Closed {};
    interface Teller { void serve(); };
    interface Auditor : Teller {};
    interface Inspector : Auditor {};
    interface Log {};
    typedef sequence<Teller> Tellers;
    typedef sequence<Auditor> Auditors;
    eventtype Moved { public long amount; };
    eventtype BigMoved : Moved {};
    valuetype AccountKey { public string accno; };
    component Account {
        provides Teller teller;
        provides Log log;
        provides Teller spare;
        attribute long balance;
        attribute Auditor checker;
        uses Auditor backup;
        uses multiple Teller clerks;
        emits BigMoved moved;
        publishes Moved ticker;
        consumes Moved incoming;
    };
    home AccountHome manages Account primarykey AccountKey {
        factory open(in string accno, in long balance);
        finder lookup(in string accno);
        long count(in Auditor who, out Teller found) raises (Overdrawn);
        void close_all();
    };
    component Kiosk { provides Teller desk; attribute long visits; };
    home KioskHome manages Kiosk {
        factory place(in string where);
        void sweep();
    };
};
module Store {
    abstract storagetype LedgerBase {
        state long version;
    };
    abstract storagetype Ledger : LedgerBase {
        state string accno;
        state long balance;
        state Bank::Teller auditor;
        state Bank::Teller backup;
        state Bank::Tellers clerks;
        state Bank::Moved moved;
        state long ticker;
        state short narrow;
        state Bank::Auditor strict;
        state Bank::Auditors strict_clerks;
        state Bank::BigMoved big;
        state string name;
    };
    abstract storagehome LedgerHome of Ledger {
        key accno;
        factory create(accno, balance);
        factory create_named(name, balance);
        long tally(in Bank::Teller who, out Bank::Auditor found);
        long tally_narrow(in Bank::Inspector who, out Bank::Log found);
        short tally_short(in Bank::Teller who, out Bank::Auditor found);
        long tally_raising(in Bank::Teller who, out Bank::Auditor found) raises (Bank::Closed);
        void wipe();
        void wipe_twice(in long n);
    };
};
";

/// `BANK` and one composition.
fn with(composition: &str) -> String {
    format!("{BANK}\nmodule Impl {{\n{composition}\n}};\n")
}

/// An entity composition over `Bank::AccountHome` and
/// `Store::LedgerHome` with `body` in its home executor after `bindsTo`.
fn entity(executor: &str, rest: &str) -> String {
    with(&format!(
        "composition entity AccountImpl {{
            home executor AccountHomeImpl {{
                implements Bank::AccountHome;
                bindsTo Store::LedgerHome;
                manages AccountExec {executor};
                {rest}
            }};
        }};"
    ))
}

#[test]
fn corba_p3_8_a_composition_with_every_part_lowers_without_a_diagnostic() {
    let lowered = lowered(&with(
        "composition entity AccountImpl {
            home executor AccountHomeImpl {
                implements Bank::AccountHome;
                bindsTo Store::LedgerHome;
                storedOn Store::LedgerHome;
                manages AccountExec {
                    segment Audit { storedOn Store::LedgerHome; provides facet log; }
                    segment Spare { provides facet spare; }
                    delegatesTo storage (balance: balance, checker: auditor,
                        backup: backup, clerks: clerks, moved: moved, ticker: ticker)
                };
                delegatesTo abstract storagehome (open: create, count: tally, close_all: wipe);
                abstract (find_by_primary_key);
            };
            proxy home AccountProxy {
                delegatesTo home (count, close_all);
                abstract (lookup);
            };
        };",
    ));
    assert!(errors(&lowered).is_empty(), "{:?}", lowered.diagnostics);
    let composition = &lowered.cidl.compositions[0];
    assert_eq!(composition.name, "Impl::AccountImpl");
    assert_eq!(composition.category, CompositionCategory::Entity);
    let he = &composition.home_executor;
    assert_eq!(he.name, "AccountHomeImpl");
    assert_eq!(he.implements, "Bank::AccountHome");
    assert_eq!(he.binds_to.as_deref(), Some("Store::LedgerHome"));
    assert_eq!(he.stored_on.as_deref(), Some("Store::LedgerHome"));
    assert_eq!(he.executor.name, "AccountExec");
    assert_eq!(he.executor.feature_delegations.len(), 6);
    assert_eq!(he.storage_home_delegations.len(), 3);
    assert_eq!(he.storage_home_delegations[2].target_name(), "wipe");
    assert_eq!(
        he.abstract_operations,
        vec!["find_by_primary_key".to_string()]
    );
    let proxy = composition.proxy_home.as_ref().unwrap();
    assert_eq!(proxy.name, "AccountProxy");
    assert_eq!(proxy.home_delegations.len(), 2);
    // The PSDL declarations are in the model as CIDL sees them.
    let kinds: Vec<(String, StorageKind)> = lowered
        .cidl
        .storage
        .iter()
        .map(|s| (s.name.clone(), s.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("Store::LedgerBase".into(), StorageKind::AbstractStorageType),
            ("Store::Ledger".into(), StorageKind::AbstractStorageType),
            ("Store::LedgerHome".into(), StorageKind::AbstractStorageHome),
        ]
    );
    let home = &lowered.cidl.storage[2];
    assert_eq!(home.keys[0].members, vec!["accno".to_string()]);
    assert_eq!(
        home.factories[0].members,
        vec!["accno".to_string(), "balance".into()]
    );
    assert_eq!(home.of.as_deref(), Some("Store::Ledger"));
    assert_eq!(
        lowered.cidl.storage[1].bases,
        vec!["Store::LedgerBase".to_string()]
    );
}

#[test]
fn corba_p3_8_11_segments_get_ids() {
    let lowered = lowered(&entity(
        "{ segment Audit { provides facet log; } segment Spare { provides facet spare; } }",
        "",
    ));
    assert!(errors(&lowered).is_empty(), "{:?}", lowered.diagnostics);
    let composition = &lowered.cidl.compositions[0];
    let ids: Vec<(String, i16)> = composition
        .home_executor
        .executor
        .segments
        .iter()
        .map(|s| (s.name.clone(), s.id))
        .collect();
    assert_eq!(
        ids,
        vec![("Audit".to_string(), 1), ("Spare".to_string(), 2)]
    );
    // §8.11: the component segment is zero and has every other facet.
    assert_eq!(composition.segment_of_facet("log"), 1);
    assert_eq!(composition.segment_of_facet("spare"), 2);
    assert_eq!(composition.segment_of_facet("teller"), 0);
}

#[test]
fn corba_p3_8_13_a_facet_in_two_segments_is_refused() {
    let lowered = lowered(&entity(
        "{ segment A { provides facet log; } segment B { provides facet log, nothing; } }",
        "",
    ));
    let found = citing(&lowered, "§8.13");
    assert_eq!(found.len(), 2, "{:?}", lowered.diagnostics);
    assert!(
        found
            .iter()
            .all(|d| d.code == DiagnosticCode::CompositionRule)
    );
    assert!(
        found[0]
            .message
            .contains("already declared in the segment `A`")
    );
    assert!(found[1].message.contains("no facet `nothing`"));
}

#[test]
fn corba_p3_8_5_1_categories_are_checked_through_the_compiler() {
    // Table 8.3: a session composition binds to no storage home, and its
    // home declares neither a primary key nor a finder.
    let session = lowered(&with(
        "composition session S { home executor H {
            implements Bank::AccountHome; bindsTo Store::LedgerHome; manages E; }; };",
    ));
    let found = citing(&session, "Table 8.3");
    assert_eq!(found.len(), 3, "{:?}", session.diagnostics);
    // Table 8.4: a process composition's home has no primary key.
    let process = lowered(&with(
        "composition process P { home executor H { implements Bank::AccountHome; manages E; }; };",
    ));
    assert_eq!(citing(&process, "Table 8.4").len(), 1);
    // Table 8.5: an entity composition's home has one.
    let entity = lowered(&with(
        "composition entity N { home executor H { implements Bank::KioskHome; manages E; }; };",
    ));
    assert_eq!(citing(&entity, "Table 8.5").len(), 1);
    // A keyless home without finders is a session's or a service's.
    for category in ["session", "service"] {
        let fine = lowered(&with(&format!(
            "composition {category} K {{ home executor H {{ implements Bank::KioskHome;
                manages E; }}; }};"
        )));
        assert!(
            errors(&fine).is_empty(),
            "{category}: {:?}",
            fine.diagnostics
        );
    }
    // An executor body holds a member (production (14)): `manages E { }`
    // is no monolithic executor but a syntax error.
    let empty = parse_with(
        &with(
            "composition session K { home executor H { implements Bank::KioskHome;
                manages E { }; }; };",
        ),
        cidl(),
    );
    let found: Vec<_> = empty
        .diagnostics()
        .iter()
        .filter(|d| d.spec_reference.contains("§8.10 (14)"))
        .collect();
    assert_eq!(found.len(), 1, "{:?}", empty.diagnostics());
}

#[test]
fn corba_p3_8_7_names_denote_a_home_and_abstract_storage_homes() {
    let not_a_home = lowered(&with(
        "composition session S { home executor H { implements Bank::Teller; manages E; }; };",
    ));
    assert!(
        citing(&not_a_home, "§8.7")
            .iter()
            .any(|d| d.message.contains("not a component home")),
        "{:?}",
        not_a_home.diagnostics
    );
    let unknown = lowered(&with(
        "composition session S { home executor H { implements Bank::Nobody; manages E; }; };",
    ));
    assert!(
        unknown
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnresolvedName),
        "{:?}",
        unknown.diagnostics
    );
    let not_storage = lowered(&with(
        "composition process P { home executor H { implements Bank::KioskHome;
            bindsTo Store::Ledger; storedOn Bank::Teller; manages E; }; };",
    ));
    let found = citing(&not_storage, "§8.8, §8.9, §8.12");
    assert_eq!(found.len(), 2, "{:?}", not_storage.diagnostics);
    assert!(
        found
            .iter()
            .all(|d| d.message.contains("not an abstract storage home"))
    );
}

#[test]
fn corba_p3_8_14_feature_delegation_needs_a_binding_and_a_compatible_member() {
    // Without `bindsTo` there is no storage to delegate to.
    let unbound = lowered(&with(
        "composition process P { home executor H { implements Bank::KioskHome;
            manages E { delegatesTo storage (visits: balance) }; }; };",
    ));
    assert!(
        citing(&unbound, "§8.14")
            .iter()
            .any(|d| d.message.contains("binds to no abstract storage home")),
        "{:?}",
        unbound.diagnostics
    );

    // Table 8.6, row by row: (feature, member, compatible).
    let rows = [
        // attribute of a non-reference type: identical.
        ("balance", "balance", true),
        ("balance", "narrow", false),
        // attribute of an object reference type: identical or a base.
        ("checker", "auditor", true),
        ("checker", "strict", true),
        ("checker", "balance", false),
        // simplex receptacle: its interface or a base of it.
        ("backup", "backup", true),
        ("backup", "strict", true),
        ("backup", "clerks", false),
        // multiplex receptacle: a sequence of a compatible type.
        ("clerks", "clerks", true),
        ("clerks", "strict_clerks", false),
        ("clerks", "backup", false),
        // emitter: its event type or a base of it.
        ("moved", "moved", true),
        ("moved", "big", true),
        ("moved", "balance", false),
        // publisher: long.
        ("ticker", "ticker", true),
        ("ticker", "narrow", false),
    ];
    for (feature, member, compatible) in rows {
        let lowered = lowered(&entity(
            &format!("{{ delegatesTo storage ({feature}: {member}) }}"),
            "",
        ));
        let found = citing(&lowered, "Table 8.6");
        assert_eq!(
            found.is_empty(),
            compatible,
            "{feature}: {member}: {:?}",
            lowered.diagnostics
        );
        if !compatible {
            assert_eq!(found[0].code, DiagnosticCode::TypeMismatch);
        }
        assert!(
            errors(&lowered).len() == usize::from(!compatible),
            "{feature}: {member}: {:?}",
            lowered.diagnostics
        );
    }
    // The feature is a stateful one and the member exists.
    let names = lowered(&entity(
        "{ delegatesTo storage (incoming: moved, teller: auditor, balance: nothing) }",
        "",
    ));
    let found = citing(&names, "§8.14");
    assert_eq!(found.len(), 3, "{:?}", names.diagnostics);
}

#[test]
fn corba_p3_8_15_storage_home_delegation_signature_rules() {
    let case = |delegations: &str| {
        lowered(&entity(
            "",
            &format!("delegatesTo abstract storagehome ({delegations});"),
        ))
    };
    // Compatible: a factory to a factory with the parameters' types, an
    // `in` reference to a base, an `out` reference to a derived type.
    let fine = case("open: create, count: tally, close_all: wipe");
    assert!(errors(&fine).is_empty(), "{:?}", fine.diagnostics);
    // A default target of the same name: `wipe` is no home operation,
    // and `close_all` no storage home operation.
    let same_name = case("close_all");
    assert!(
        citing(&same_name, "§8.15")
            .iter()
            .any(|d| d.message.contains("no operation `close_all`")),
        "{:?}",
        same_name.diagnostics
    );
    for (delegations, expected) in [
        // A factory delegates to a factory.
        ("open: wipe", "not a factory"),
        // A return type that is no reference is the same type.
        ("count: tally_short", "return different types"),
        // `in` reference: the storage home's parameter is the home's or
        // a base, not a derived one; `out`: the home's or a derived one,
        // not an unrelated one.
        ("count: tally_narrow", "parameter 1"),
        ("count: tally_narrow", "parameter 2"),
        // Exceptions of the storage home operation are the home
        // operation's.
        ("count: tally_raising", "raises `Bank::Closed`"),
        // Equal numbers of parameters.
        ("close_all: wipe_twice", "takes 0 parameters"),
    ] {
        let lowered = case(delegations);
        let found = citing(&lowered, "§8.15");
        assert!(
            found.iter().any(|d| d.message.contains(expected)),
            "{delegations}: {:?}",
            lowered.diagnostics
        );
    }
    let narrow = case("count: tally_narrow");
    assert_eq!(
        citing(&narrow, "§8.15").len(),
        2,
        "both parameters: {:?}",
        narrow.diagnostics
    );
    // One target per home operation and one home operation per target.
    let twice = case("count: tally, close_all: tally");
    assert!(
        citing(&twice, "§8.15")
            .iter()
            .any(|d| d.message.contains("target of more than one delegation")),
        "{:?}",
        twice.diagnostics
    );
    // No delegation to the storage home without a binding.
    let unbound = lowered(&with(
        "composition process P { home executor H { implements Bank::KioskHome;
            manages E; delegatesTo abstract storagehome (sweep: wipe); }; };",
    ));
    assert!(
        citing(&unbound, "§8.15")
            .iter()
            .any(|d| d.message.contains("binds to none")),
        "{:?}",
        unbound.diagnostics
    );
    // A key's finders are operations of the storage home: `find_by_accno`
    // takes the key's state member, as `lookup` takes a string, returns
    // the storage type, which a finder's component is not, and raises
    // `CosPersistentState::NotFound`, which `lookup` does not.
    let finder = case("lookup: find_by_accno");
    let found = citing(&finder, "§8.15");
    assert_eq!(found.len(), 2, "{:?}", finder.diagnostics);
    assert!(found[0].message.contains("returns neither"));
    assert!(
        found[1]
            .message
            .contains("raises `CosPersistentState::NotFound`")
    );
}

/// PSS 2.0 §3.2.5.7: a key `k` declares `S find_by_k(…) raises
/// (CosPersistentState::NotFound)` and `ref<S> find_ref_by_k(…)`. The
/// rule of §8.15 on exceptions reads the first; the second returns no IDL
/// type.
#[test]
fn corba_p3_8_15_a_key_finder_raises_not_found_and_its_ref_finder_returns_a_ref() {
    let source = format!(
        "{BANK}
        module CosPersistentState {{ exception NotFound {{}}; }};
        module Desk {{
            home LedgerDesk manages Bank::Kiosk {{
                long find_quietly(in string accno);
                long find_loudly(in string accno) raises (CosPersistentState::NotFound);
            }};
            composition process DeskImpl {{
                home executor DeskHome {{
                    implements Desk::LedgerDesk;
                    bindsTo Store::LedgerHome;
                    manages DeskExec;
                    delegatesTo abstract storagehome (DELEGATIONS);
                }};
            }};
        }};
        "
    );
    let case = |delegations: &str| lowered(&source.replace("DELEGATIONS", delegations));
    let raising = |l: &LoweredCidl| {
        citing(l, "§8.15")
            .iter()
            .filter(|d| d.message.contains("raises `CosPersistentState::NotFound`"))
            .count()
    };
    // Both return `long`, not `Store::Ledger`: that rule fires for both;
    // the exception rule only where the home operation lacks NotFound.
    let quiet = case("find_quietly: find_by_accno");
    assert_eq!(raising(&quiet), 1, "{:?}", quiet.diagnostics);
    let loud = case("find_loudly: find_by_accno");
    assert_eq!(raising(&loud), 0, "{:?}", loud.diagnostics);
    for lowered in [&quiet, &loud] {
        assert!(
            citing(lowered, "§8.15")
                .iter()
                .any(|d| d.message.contains("return different types")),
            "{:?}",
            lowered.diagnostics
        );
    }
    // `find_ref_by_accno` raises nothing and returns `ref<Store::Ledger>`.
    let by_ref = case("find_quietly: find_ref_by_accno");
    let found = citing(&by_ref, "§8.15");
    assert_eq!(found.len(), 1, "{:?}", by_ref.diagnostics);
    assert!(
        found[0].message.contains("returns `ref<…>`"),
        "{:?}",
        by_ref.diagnostics
    );
}

#[test]
fn corba_p3_8_16_only_factories_delegate_to_the_executor() {
    let fine = lowered(&entity(
        "",
        "delegatesTo executor (open: open_account, create);",
    ));
    assert!(errors(&fine).is_empty(), "{:?}", fine.diagnostics);
    let not_a_factory = lowered(&entity("", "delegatesTo executor (count, close_all);"));
    let found = citing(&not_a_factory, "§8.16");
    assert_eq!(found.len(), 2, "{:?}", not_a_factory.diagnostics);
    assert!(found[0].message.contains("not a factory operation"));
    let both = lowered(&entity(
        "",
        "delegatesTo abstract storagehome (open: create); delegatesTo executor (open);",
    ));
    assert!(
        citing(&both, "§8.16").iter().any(|d| d
            .message
            .contains("both to the abstract storage home and to the executor")),
        "{:?}",
        both.diagnostics
    );
}

#[test]
fn corba_p3_8_17_abstract_operations_are_home_operations() {
    let lowered = lowered(&entity("", "abstract (count, remove, nothing);"));
    let found = citing(&lowered, "§8.17");
    assert_eq!(found.len(), 1, "{:?}", lowered.diagnostics);
    assert!(found[0].message.contains("no operation `nothing`"));
}

#[test]
fn corba_p3_8_18_proxy_home_delegates_to_home() {
    let fine = lowered(&with(
        "composition session S { home executor H { implements Bank::KioskHome; manages E; };
            proxy home P { delegatesTo home (sweep, place: place); abstract (create); }; };",
    ));
    assert!(errors(&fine).is_empty(), "{:?}", fine.diagnostics);
    let wrong = lowered(&with(
        "composition session S { home executor H { implements Bank::KioskHome; manages E; };
            proxy home P { delegatesTo home (sweep: nothing); abstract (sweep, gone); }; };",
    ));
    let found = citing(&wrong, "§8.18");
    // `nothing` and `gone` name no operation, and `sweep` is delegated
    // and abstract at once. A proxy home's executor delegation is a
    // syntax error (`corba_p3_8_6_a_misplaced_member_that_reaches_the_lowering_is_reported`).
    assert_eq!(found.len(), 3, "{:?}", wrong.diagnostics);
}

#[test]
fn corba_p3_9_2_5_the_toon_example_of_the_specification() {
    // §9.2.5, Example 1, with the IDL it imports written out.
    let lowered = lowered(
        "module LooneyToons {
            interface Bird { void fly(); };
            component Toon { provides Bird bird; };
            home ToonTown manages Toon {};
        };
        module MerryMelodies {
            // this is the composition:
            composition session ToonImpl {
                home executor ToonTownImpl {
                    implements LooneyToons::ToonTown;
                    manages ToonSessionImpl;
                };
            };
        };",
    );
    assert!(errors(&lowered).is_empty(), "{:?}", lowered.diagnostics);
    let toon = &lowered.cidl.compositions[0];
    assert_eq!(toon.name, "MerryMelodies::ToonImpl");
    assert_eq!(toon.home_executor.implements, "LooneyToons::ToonTown");
    assert_eq!(toon.home_executor.executor.name, "ToonSessionImpl");
    assert!(!toon.is_segmented());
}

#[test]
fn corba_p3_8_5_a_composition_name_is_a_name_of_its_module() {
    let lowered = lowered(&with(
        "interface Desk {};
         composition session Desk { home executor H { implements Bank::KioskHome; manages E; }; };
         composition session Twice { home executor H { implements Bank::KioskHome; manages E; }; };
         composition session Twice { home executor H { implements Bank::KioskHome; manages E; }; };",
    ));
    let found: Vec<_> = lowered
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::DuplicateName)
        .collect();
    assert_eq!(found.len(), 2, "{:?}", lowered.diagnostics);
}

#[test]
fn corba_p3_8_2_1_an_identifier_differing_from_a_cidl_keyword_in_case_collides() {
    let source = "struct State { long Key; };";
    let in_cidl = lowered(source);
    let collisions: Vec<_> = in_cidl
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::KeywordCollision)
        .collect();
    assert_eq!(collisions.len(), 2, "{:?}", in_cidl.diagnostics);
    // In IDL they are ordinary identifiers.
    let parsed = parse_with(source, Dialect::ALL_VENDORS);
    let idl = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(idl.diagnostics.is_empty(), "{:?}", idl.diagnostics);
    // The escape turns the check off.
    let escaped = lowered("struct _State { long _Key; };");
    assert!(errors(&escaped).is_empty(), "{:?}", escaped.diagnostics);
}

#[test]
fn pss_3_2_5_psdl_names_and_members_are_checked() {
    let lowered = lowered(
        "module P {
            interface I {};
            abstract storagetype T { state long a; state long b; };
            abstract storagetype U : I { state long c; };
            abstract storagehome H of T {
                key k(a, missing);
                key dup(a, a);
                factory f(b, nope);
            };
            abstract storagehome W of I {};
            storagetype S implements T ref(a, zz) {
                stores b as Nowhere;
            };
            storagehome SH of S primary key nokey {};
            abstract storagetype V { state ref<I> r; };
        };",
    );
    let messages: Vec<&str> = lowered
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    for expected in [
        "names `missing`",
        "names `a` twice",
        "names `nope`",
        "`ref(zz)` names no state member",
        "`Nowhere` names no declaration",
        "`primary key nokey` names no key",
    ] {
        assert!(
            messages.iter().any(|m| m.contains(expected)),
            "{expected}: {messages:?}"
        );
    }
    // A base of an abstract storage type, the type of an abstract storage
    // home, and the type behind `ref<…>` are abstract storage types.
    let kinds = lowered
        .diagnostics
        .iter()
        .filter(|d| {
            d.message
                .contains("is an abstract storagetype, and this names something else")
        })
        .count();
    assert_eq!(kinds, 3, "{messages:?}");
}

/// `green` with each error node replaced by the nodes and tokens it
/// holds: the tree a parser that did not check where a member stands
/// would have built.
fn without_error_nodes(
    green: &std::sync::Arc<zerodds_idl_cst::GreenNode>,
) -> std::sync::Arc<zerodds_idl_cst::GreenNode> {
    use zerodds_idl_cst::{CstKind, GreenChild, GreenNode};
    let mut children = Vec::new();
    for child in green.children() {
        match child {
            GreenChild::Node(node) if node.kind() == CstKind::Error => {
                children.extend(without_error_nodes(node).children().iter().cloned());
            }
            GreenChild::Node(node) => children.push(GreenChild::Node(without_error_nodes(node))),
            GreenChild::Token(token) => children.push(GreenChild::Token(token.clone())),
        }
    }
    std::sync::Arc::new(GreenNode::new(green.kind(), children))
}

/// The parser keeps a member out of its place in an error node
/// (`zerodds-idl-parse` tests `corba_p3_8_6_*`). One that reaches the
/// lowering in its place anyway is reported, never read as a member of
/// the wrong kind or dropped.
#[test]
fn corba_p3_8_6_a_misplaced_member_that_reaches_the_lowering_is_reported() {
    for (composition, expected, clause) in [
        // A feature delegation in the home executor body.
        (
            "manages E; delegatesTo storage (balance: nosuchmember);",
            "`delegatesTo storage (balance: nosuchmember)` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        // A home delegation in the home executor body: no storage home
        // delegation.
        (
            "manages E; delegatesTo home (close_all: wipe);",
            "`delegatesTo home (close_all: wipe)` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        // A second `bindsTo` and a second `manages`.
        (
            "manages E1; bindsTo Store::Ledger; manages E2;",
            "`bindsTo Store::Ledger;` does not belong in the home executor body",
            "§8.6 (8)",
        ),
        // An executor delegation in the executor body.
        (
            "manages E { delegatesTo executor (bogus) };",
            "`delegatesTo executor (bogus)` does not belong in the executor body",
            "§8.10 (15)",
        ),
    ] {
        let source = with(&format!(
            "composition entity AccountImpl {{ home executor AccountHomeImpl {{
                implements Bank::AccountHome; bindsTo Store::LedgerHome; {composition} }}; }};"
        ));
        let parsed = parse_with(&source, cidl());
        assert!(parsed.has_errors(), "the parser reports it: {composition}");
        let tree = std::sync::Arc::new(zerodds_idl_cst::SyntaxNode::root(without_error_nodes(
            parsed.green(),
        )));
        assert_eq!(tree.text(), source, "the same text");
        let lowered = lower_cidl(
            &tree,
            "test.cdl",
            Options {
                dialect: cidl(),
                ..Options::default()
            },
        );
        let found = citing(&lowered, clause);
        assert!(
            found
                .iter()
                .any(|d| d.message.contains(expected) && d.code == DiagnosticCode::CompositionRule),
            "{composition}: {:?}",
            lowered.diagnostics
        );
        let he = &lowered.cidl.compositions[0].home_executor;
        assert!(he.storage_home_delegations.is_empty(), "{composition}");
        assert!(he.executor_delegations.is_empty(), "{composition}");
        assert!(he.executor.feature_delegations.is_empty(), "{composition}");
        assert_eq!(
            he.binds_to.as_deref(),
            Some("Store::LedgerHome"),
            "{composition}"
        );
    }
    // A storage delegation in a proxy home.
    let source = with(
        "composition session S { home executor H { implements Bank::KioskHome; manages E; };
            proxy home P { delegatesTo storage (visits: balance); }; };",
    );
    let parsed = parse_with(&source, cidl());
    assert!(parsed.has_errors());
    let tree = std::sync::Arc::new(zerodds_idl_cst::SyntaxNode::root(without_error_nodes(
        parsed.green(),
    )));
    let lowered = lower_cidl(
        &tree,
        "test.cdl",
        Options {
            dialect: cidl(),
            ..Options::default()
        },
    );
    assert!(
        citing(&lowered, "§8.18 (33), (34)")
            .iter()
            .any(|d| d.message.contains("does not belong in a proxy home")),
        "{:?}",
        lowered.diagnostics
    );
    let proxy = lowered.cidl.compositions[0].proxy_home.as_ref().unwrap();
    assert!(proxy.home_delegations.is_empty());
}

#[test]
fn pss_3_2_5_4_a_const_operation_is_const_in_the_model() {
    let lowered = lowered(
        "module P {
            abstract storagetype T {
                state long a;
                long total(in long since) const;
                void reset();
            };
        };",
    );
    assert!(errors(&lowered).is_empty(), "{:?}", lowered.diagnostics);
    let operations: Vec<(String, bool)> = lowered.cidl.storage[0]
        .operations
        .iter()
        .map(|o| (o.name.clone(), o.is_const))
        .collect();
    assert_eq!(
        operations,
        vec![("total".to_string(), true), ("reset".to_string(), false)]
    );
}
