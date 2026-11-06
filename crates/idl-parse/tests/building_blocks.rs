// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The building blocks of OMG IDL 4.2 §7.4, their relationships (§7.4.16,
//! Figure 7-2) and the profiles of clause 9, each held to the sentences of
//! the specification that state them.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_parse::{
    AnnotationGroup, BuildingBlock as B, DiagnosticCode, IdlProfile, outside_profile, parse,
};

/// The diagnostics a profile reports for `source`.
fn outside(source: &str, profile: IdlProfile) -> Vec<String> {
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "{source:?} does not parse: {:?}",
        parsed.diagnostics()
    );
    outside_profile(&parsed.syntax(), profile)
        .into_iter()
        .inspect(|d| assert_eq!(d.code, DiagnosticCode::OutsideProfile))
        .map(|d| d.message)
        .collect()
}

fn assert_inside(source: &str, profile: IdlProfile) {
    let found = outside(source, profile);
    assert!(
        found.is_empty(),
        "{source:?} under {}: {found:?}",
        profile.name()
    );
}

fn assert_outside(source: &str, profile: IdlProfile, block_or_group: &str) {
    let found = outside(source, profile);
    assert!(
        found.iter().any(|message| message.contains(block_or_group)),
        "{source:?} under {} should be outside by {block_or_group}: {found:?}",
        profile.name()
    );
}

// -- §7.4.16 Relationships between the building blocks -------------------------

/// Figure 7-2 as the "Dependencies with other Building Blocks" subclauses
/// state it: each block's closure is what its subclause lists, directly and
/// "Transitively".
#[test]
fn idl_7_4_16_each_building_block_relies_on_what_its_subclause_lists() {
    let mut expected: Vec<(B, Vec<B>)> = vec![
        // §7.4.1.2: "the root for all other building blocks and requires
        // no other ones."
        (B::CoreDataTypes, vec![]),
        // §7.4.2.2, §7.4.3.2: "relies on Building Block Core Data Types."
        (B::Any, vec![B::CoreDataTypes]),
        (B::InterfacesBasic, vec![B::CoreDataTypes]),
        // §7.4.4.2: "complements Building Block Interfaces – Basic.
        // Transitively, it relies on Building Block Core Data Types."
        (
            B::InterfacesFull,
            vec![B::InterfacesBasic, B::CoreDataTypes],
        ),
        // §7.4.5.2: "requires Building Block Interfaces – Basic.
        // Transitively, it relies on Building Block Core Data Types."
        (B::ValueTypes, vec![B::InterfacesBasic, B::CoreDataTypes]),
        // §7.4.6.2: "based on Building Block Interfaces – Full.
        // Transitively, it relies on Building Block Interfaces – Basic and
        // Building Block Core Data Types."
        (
            B::CorbaSpecificInterfaces,
            vec![B::InterfacesFull, B::InterfacesBasic, B::CoreDataTypes],
        ),
        // §7.4.7.2: "based on Building Block Value Types and complements
        // Building Block CORBA-Specific – Interfaces. Transitively, it
        // relies on Building Block Interfaces – Full, Building Block
        // Interfaces – Basic and Building Block Core Data Types."
        (
            B::CorbaSpecificValueTypes,
            vec![
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::InterfacesFull,
                B::InterfacesBasic,
                B::CoreDataTypes,
            ],
        ),
        // §7.4.8.2: "complements Building Block Interfaces – Basic.
        // Transitively, it relies on Building Block Core Data Types."
        (
            B::ComponentsBasic,
            vec![B::InterfacesBasic, B::CoreDataTypes],
        ),
        // §7.4.9.2: "complements Building Block Components – Basic.
        // Transitively, it relies on Building Block Interfaces – Basic and
        // Building Block Core Data Types."
        (
            B::ComponentsHomes,
            vec![B::ComponentsBasic, B::InterfacesBasic, B::CoreDataTypes],
        ),
        // §7.4.10.2: "relies on Building Block Components – Basic and
        // Building Block CORBA-Specific – Value Types. Transitively, it
        // relies on Building Block CORBA-Specific – Interfaces, Building
        // Block Value Types, Building Block Interfaces – Full, Building
        // Block Interfaces – Basic and Building Block Core Data Types."
        (
            B::CcmSpecific,
            vec![
                B::ComponentsBasic,
                B::CorbaSpecificValueTypes,
                B::CorbaSpecificInterfaces,
                B::ValueTypes,
                B::InterfacesFull,
                B::InterfacesBasic,
                B::CoreDataTypes,
            ],
        ),
        // §7.4.11.2: "relies on the Building Block Components – Basic.
        // Transitively, it relies on Building Block Interfaces – Basic and
        // Building Block Core Data Types."
        (
            B::ComponentsPortsAndConnectors,
            vec![B::ComponentsBasic, B::InterfacesBasic, B::CoreDataTypes],
        ),
        // §7.4.12.2: "relies only on Building Block Core Data Types".
        (B::TemplateModules, vec![B::CoreDataTypes]),
        // §7.4.13.2: "complements the Building Block Core Data Types."
        (B::ExtendedDataTypes, vec![B::CoreDataTypes]),
        // §7.4.14.2, §7.4.15.2: "relies on Building Block Core Data Types."
        (B::AnonymousTypes, vec![B::CoreDataTypes]),
        (B::Annotations, vec![B::CoreDataTypes]),
    ];
    assert_eq!(expected.len(), B::ALL.len());
    for (block, relies) in &mut expected {
        relies.push(*block);
        relies.sort();
        assert_eq!(block.closure(), *relies, "{}", block.name());
    }
}

// -- clause 9 Profiles ---------------------------------------------------------

/// Every profile is the list of building blocks its clause gives, closed
/// under the lattice (a profile holds what each of its blocks relies on),
/// and a profile with a group of annotations includes the Annotations block
/// (§8.1: "Any profile that includes such a group of annotations must
/// include the Building Block Annotations").
#[test]
fn idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice() {
    let lists: [(IdlProfile, &[B]); 7] = [
        (
            IdlProfile::PlainCorba,
            &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
            ],
        ),
        (
            IdlProfile::MinimumCorba,
            &[
                B::CoreDataTypes,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::CorbaSpecificInterfaces,
            ],
        ),
        (
            IdlProfile::Ccm,
            &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
                B::ComponentsBasic,
                B::CcmSpecific,
            ],
        ),
        (
            IdlProfile::CcmWithGenericInteractionSupport,
            &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
                B::ComponentsBasic,
                B::CcmSpecific,
                B::ComponentsPortsAndConnectors,
                B::TemplateModules,
            ],
        ),
        (IdlProfile::PlainDds, &[B::CoreDataTypes, B::AnonymousTypes]),
        (
            IdlProfile::ExtensibleDds,
            &[
                B::CoreDataTypes,
                B::ExtendedDataTypes,
                B::AnonymousTypes,
                B::Annotations,
            ],
        ),
        (
            IdlProfile::RpcOverDds,
            &[
                B::CoreDataTypes,
                B::ExtendedDataTypes,
                B::AnonymousTypes,
                B::InterfacesBasic,
                B::Annotations,
            ],
        ),
    ];
    for (profile, list) in lists {
        assert_eq!(profile.building_blocks(), list, "{}", profile.name());
        for block in profile.building_blocks() {
            for needed in block.closure() {
                assert!(
                    profile.includes(needed),
                    "{} holds {} and not {}, which it relies on",
                    profile.name(),
                    block.name(),
                    needed.name()
                );
            }
        }
        if !profile.annotation_groups().is_empty() {
            assert!(profile.includes(B::Annotations), "{}", profile.name());
        }
        assert_eq!(IdlProfile::from_name(profile.as_str()), Some(profile));
    }
    // §9.3.2 and §9.3.3 name their groups.
    assert_eq!(
        IdlProfile::ExtensibleDds.annotation_groups(),
        [
            AnnotationGroup::GeneralPurpose,
            AnnotationGroup::DataModeling,
            AnnotationGroup::DataImplementation,
            AnnotationGroup::CodeGeneration,
        ]
    );
    assert_eq!(
        IdlProfile::RpcOverDds.annotation_groups(),
        [AnnotationGroup::GeneralPurpose, AnnotationGroup::Interfaces]
    );
}

/// §9.2.1: Core Data Types, Any, Interfaces – Basic and – Full, Value Types,
/// and the two CORBA-Specific blocks — no anonymous types (a `string`
/// written at a member is a template type, rule (216)), no components, no
/// extended data types, no annotations.
#[test]
fn idl_9_2_1_the_plain_corba_profile() {
    let profile = IdlProfile::PlainCorba;
    assert_inside(
        "typedef string Name;
         module M {
             interface Base { void op(in long a); attribute any value; };
             local interface Local { const long K = 1; };
             abstract interface Abstract { };
             valuetype V supports Base { public Name n; factory make(in long a); };
             abstract valuetype AV { };
             custom valuetype CV { public long c; };
             valuetype Box long;
             typeid V \"IDL:M/V:1.0\";
         };",
        profile,
    );
    assert_outside("struct S { string s; };", profile, "Anonymous Types");
    assert_outside("struct S { long a[3]; };", profile, "Anonymous Types");
    assert_outside("component C { };", profile, "Components – Basic");
    assert_outside("typedef map<long, long> M;", profile, "Extended Data-Types");
    assert_outside("struct S { int8 x; };", profile, "Extended Data-Types");
    assert_outside("@final struct S { long x; };", profile, "Annotations");
}

/// §9.2.2: "As opposed to Plain CORBA Profile, it does not embed Any nor
/// Valuetypes."
#[test]
fn idl_9_2_2_the_minimum_corba_profile() {
    let profile = IdlProfile::MinimumCorba;
    assert_inside(
        "interface I { typedef long T; T op(in long a) context (\"x\"); };
         local interface L { };",
        profile,
    );
    assert_outside("interface I { void op(in any a); };", profile, "Any");
    assert_outside("valuetype V { public long a; };", profile, "Value Types");
    assert_outside(
        "abstract valuetype V { };",
        profile,
        "CORBA-Specific – Value Types",
    );
}

/// §9.2.3: the CORBA blocks with Components – Basic and CCM-Specific. The
/// clause does not list Components – Homes, so a `home` is outside it,
/// while CCM-Specific's rules (162) to (165) extend the home header;
/// recorded as written.
#[test]
fn idl_9_2_3_the_ccm_profile() {
    let profile = IdlProfile::Ccm;
    assert_inside(
        "interface I { };
         eventtype E { public long v; };
         component C supports I {
             provides I facet;
             uses multiple I receptacles;
             emits E outbound;
             consumes E sink;
             attribute long a;
         };",
        profile,
    );
    assert_outside(
        "component C { }; home H manages C { };",
        profile,
        "Components – Homes",
    );
    assert_outside(
        "porttype P { provides Object f; };",
        profile,
        "Ports and Connectors",
    );
}

/// §9.2.4: the CCM profile with Ports and Connectors and Template Modules.
#[test]
fn idl_9_2_4_the_ccm_with_generic_interaction_support_profile() {
    let profile = IdlProfile::CcmWithGenericInteractionSupport;
    assert_inside(
        "interface I { };
         porttype P { provides I f; uses I r; };
         connector X { port P p; mirrorport P m; };
         module T<typename X> { typedef X Same; };
         module T<long> L;",
        profile,
    );
    assert_outside("struct S { uint16 v; };", profile, "Extended Data-Types");
}

/// §9.3.1: "Building Block Core Data Types; Building Block Anonymous
/// Types."
#[test]
fn idl_9_3_1_the_plain_dds_profile() {
    let profile = IdlProfile::PlainDds;
    assert_inside(
        "module M {
             enum Color { RED, GREEN };
             struct S {
                 string name;
                 sequence<long> values;
                 long matrix[2][3];
                 Color c;
             };
             union U switch (long) { case 1: string text; default: double d; };
             typedef sequence<sequence<S> > Nested;
         };",
        profile,
    );
    assert_outside("struct S { any a; };", profile, "Any");
    assert_outside(
        "struct S { map<long, string> m; };",
        profile,
        "Extended Data-Types",
    );
    assert_outside("@key struct S { long a; };", profile, "Annotations");
    assert_outside("interface I { };", profile, "Interfaces – Basic");
    assert_outside(
        "struct B { long a; }; struct S : B { long b; };",
        profile,
        "Extended Data-Types",
    );
}

/// §9.3.2: Core, Extended Data-Types, Anonymous Types, Annotations and four
/// groups: General Purpose, Data Modeling, Data Implementation, Code
/// Generation — not Units and Ranges, not Interfaces.
#[test]
fn idl_9_3_2_the_extensible_dds_profile() {
    let profile = IdlProfile::ExtensibleDds;
    assert_inside(
        "@annotation Mine { long v default 1; };
         @mutable struct S {
             @key @id(1) long a;
             @optional map<uint32, string> m;
             @external int8 e;
         };
         @bit_bound(8) bitmask F { @position(0) one, two };
         bitset B { bitfield<3> x; };
         @Mine @verbatim(language=\"*\", text=\"x\") struct T { };",
        profile,
    );
    assert_outside(
        "struct S { @range(min=0, max=1) long a; };",
        profile,
        "Units and Ranges",
    );
    assert_outside(
        "struct S { @default(1) long a; };",
        profile,
        "Units and Ranges",
    );
    assert_outside("@service interface I { };", profile, "Interfaces – Basic");
}

/// §9.3.3: Core, Extended Data-Types, Anonymous Types, Interfaces – Basic,
/// Annotations, and the groups General Purpose and Interfaces.
#[test]
fn idl_9_3_3_the_rpc_over_dds_profile() {
    let profile = IdlProfile::RpcOverDds;
    assert_inside(
        "@service interface Calculator {
             @oneway void reset();
             long add(in long a, in long b);
             attribute uint32 count;
         };
         @final struct Point { long x; long y; };",
        profile,
    );
    assert_outside("struct S { @key long a; };", profile, "Data Modeling");
    assert_outside("valuetype V { public long a; };", profile, "Value Types");
    assert_outside(
        "interface I { struct S { long a; }; };",
        profile,
        "Interfaces – Full",
    );
}
