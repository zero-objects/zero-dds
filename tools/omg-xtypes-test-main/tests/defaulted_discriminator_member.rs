// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a union's member becomes when `@try_construct(use_default)`
//! defaulted its discriminator, on both values of the axis that decides
//! it.
//!
//! The shape is the OMG `dds-xtypes` suite's `tryc_union_enum_disc_2`,
//! copied from `types/xml/try_construct.xml`: a writer declaring
//! `union_disc_enum_1` over `E1` — four enumerators — sends `VAL3` with
//! `x3 = 3`, and a reader declaring `union_disc_enum_2_default` over
//! `E2` — three enumerators, `VAL1` carrying `defaultLiteral="true"` —
//! cannot represent `VAL3` and defaults its discriminator to `VAL1`.
//!
//! Table 19's `UNION_TYPE` row then says what `x1` is: "m1 is set to its
//! default value according to its type", which is
//! [`DefaultedDiscriminatorMember::TypeDefault`], the default here, and
//! what RTI Connext 7.7.0 and OpenDDS 3.34.0 were both measured handing
//! back. The suite compares against
//! `data/json/tryconstruct/union_x1.json` — `{"x1":3}` — which is the
//! writer's member value under the reader's member name, and only
//! [`DefaultedDiscriminatorMember::StreamValue`] produces it.
//!
//! Both are pinned here, so that a change to either one is a failure and
//! not a silent move of the default.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast_layout::{DefaultedDiscriminatorMember, Dialect, Encoding, Endianness};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_xml_compose::compose_source;

/// The four declarations of `types/xml/try_construct.xml` that
/// `tryc_union_enum_disc_2` names, verbatim.
const SOURCE: &str = r#"<dds>
    <types>
      <module name="Test">
        <enum name="E1" bitBound="32" extensibility="appendable">
            <enumerator name="VAL0" value="0"/>
            <enumerator name="VAL1" value="1"/>
            <enumerator name="VAL2" value="2"/>
            <enumerator name="VAL3" value="3"/>
        </enum>
        <enum name="E2" bitBound="32" extensibility="appendable">
            <enumerator name="VAL0" value="0"/>
            <enumerator name="VAL1" value="1" defaultLiteral="true"/>
            <enumerator name="VAL2" value="2"/>
        </enum>
        <union name="union_disc_enum_1"> <discriminator type="nonBasic" nonBasicTypeName="E1"/>
            <case><caseDiscriminator value="VAL0" /><member name="x0"   type="int32"/></case>
            <case><caseDiscriminator value="VAL1" /><member name="x1"   type="int32"/></case>
            <case><caseDiscriminator value="VAL2" /><member name="x2"   type="int32"/></case>
            <case><caseDiscriminator value="VAL3" /><member name="x3"   type="int32"/></case>
        </union>
        <union name="union_disc_enum_2_default"> <discriminator type="nonBasic" nonBasicTypeName="E2" tryConstruct="use_default"/>
            <case><caseDiscriminator value="VAL0" /><member name="x0"   type="int32"/></case>
            <case><caseDiscriminator value="VAL1" /><member name="x1"   type="int32"/></case>
            <case><caseDiscriminator value="VAL2" /><member name="x2"   type="int32"/></case>
        </union>
        <!-- Not the suite's. The same shape with an unbounded case, which
             is the one an undelimited union cannot read out of octets the
             writer meant for something else. -->
        <union name="union_disc_enum_2_default_string"> <discriminator type="nonBasic" nonBasicTypeName="E2" tryConstruct="use_default"/>
            <case><caseDiscriminator value="VAL0" /><member name="x0"   type="int32"/></case>
            <case><caseDiscriminator value="VAL1" /><member name="s1"   type="string"/></case>
            <case><caseDiscriminator value="VAL2" /><member name="x2"   type="int32"/></case>
        </union>
      </module>
    </types>
</dds>"#;

/// The model, and the writer's octets: the suite runs `@appendable`
/// types under XCDR2 — `DelimitedCdr` — and little-endian on every host
/// it runs on.
fn writers_sample() -> (zerodds_ast::Ast, Vec<u8>) {
    let composed = compose_source("try_construct.xml", SOURCE);
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let request = ProbeRequest::new(
        "Test::union_disc_enum_1",
        Encoding::DelimitedCdr,
        Endianness::Little,
    );
    // `data/json/tryconstruct/union_x3.json` is `{"x3":3}`; the
    // discriminator the suite's own client derives from it is `VAL3`.
    let bytes = zerodds_wire_codec::encode(
        &composed.ast,
        &request,
        &serde_json::json!({"_d": "VAL3", "x3": 3}),
    )
    .expect("union_disc_enum_1 encodes");
    // DHEADER(8) | E1::VAL3 as int32 | x3 = 3 as int32.
    assert_eq!(bytes, [8, 0, 0, 0, 3, 0, 0, 0, 3, 0, 0, 0]);
    (composed.ast, bytes)
}

/// The reader's request, on one value of the axis.
fn reader(policy: DefaultedDiscriminatorMember) -> ProbeRequest {
    let mut request = ProbeRequest::new(
        "Test::union_disc_enum_2_default",
        Encoding::DelimitedCdr,
        Endianness::Little,
    );
    request.defaulted_discriminator_member = policy;
    request
}

/// The default: Table 19's member step, and what RTI and OpenDDS hand
/// back.
#[test]
fn the_default_gives_the_member_its_type_default() {
    let (ast, bytes) = writers_sample();
    let read: serde_json::Value = zerodds_wire_codec::decode(
        &ast,
        &reader(DefaultedDiscriminatorMember::TypeDefault),
        &bytes,
    )
    .expect("the defaulted discriminator reads back");
    assert_eq!(read, serde_json::json!({"_d": "VAL1", "x1": 0}));
}

/// The axis switched on: the writer's octets read as the member the
/// defaulted discriminator selects, which is what the suite compares
/// against.
#[test]
fn the_axis_keeps_the_writers_member_value() {
    let (ast, bytes) = writers_sample();
    let read: serde_json::Value = zerodds_wire_codec::decode(
        &ast,
        &reader(DefaultedDiscriminatorMember::StreamValue),
        &bytes,
    )
    .expect("the defaulted discriminator reads back");
    assert_eq!(read, serde_json::json!({"_d": "VAL1", "x1": 3}));
}

/// A request that named no axis value is the one that keeps the two
/// measured peers.
///
/// Written as its own assertion rather than left to the test above: a
/// change that moved the default would still leave both explicit values
/// working, and this is what fails when it does. Every dialect answers
/// the same, because no dialect names a peer that was measured keeping
/// the member.
#[test]
fn no_dialect_departs_from_the_clause_here() {
    assert_eq!(
        ProbeRequest::new(
            "Test::union_disc_enum_2_default",
            Encoding::DelimitedCdr,
            Endianness::Little,
        )
        .defaulted_discriminator_member,
        DefaultedDiscriminatorMember::TypeDefault,
        "the shipped default is the clause's reading"
    );
    for dialect in Dialect::ALL {
        assert_eq!(
            dialect.defaulted_discriminator_member(),
            DefaultedDiscriminatorMember::TypeDefault,
            "{} was measured keeping the writer's member",
            dialect.as_str()
        );
    }
}

/// An undelimited union: the default refuses it, the axis reads a
/// fixed-extent case out of it, and neither invents a length.
///
/// Under XCDR1 an `@appendable` union has no DHEADER and no sentinel, so
/// nothing states where the writer's member ends. `TypeDefault` has to
/// step over it and cannot — guessing the length would assemble the rest
/// of the sample out of the wrong offsets — so it refuses by name.
/// `StreamValue` does not step over anything: it reads the octets as
/// *this* type's case, and an `int32` case takes four octets because its
/// own type says four. That is the whole of why
/// `tryc_union_enum_disc_2` passes under XCDR1 as well as XCDR2.
#[test]
fn an_undelimited_union_refuses_the_step_over_and_reads_a_fixed_case() {
    let (ast, bytes) = undelimited_writers_sample();

    let mut default = ProbeRequest::new(
        "Test::union_disc_enum_2_default",
        Encoding::PlainCdr1,
        Endianness::Little,
    );
    default.defaulted_discriminator_member = DefaultedDiscriminatorMember::TypeDefault;
    let error = zerodds_wire_codec::decode::<serde_json::Value>(&ast, &default, &bytes)
        .expect_err("nothing states where the writer's member ends");
    assert!(
        format!("{error:?}").contains("undelimited union"),
        "{error:?}"
    );

    let mut stream = default.clone();
    stream.defaulted_discriminator_member = DefaultedDiscriminatorMember::StreamValue;
    let read: serde_json::Value = zerodds_wire_codec::decode(&ast, &stream, &bytes)
        .expect("a fixed-extent case is read out of the writer's octets");
    assert_eq!(read, serde_json::json!({"_d": "VAL1", "x1": 3}));
}

/// An unbounded case in an undelimited union is stepped over, not read.
///
/// `StreamValue` reads the octets as this type's case, and a `string`
/// case would take its length out of octets that were never a length —
/// a four-octet member could become a read of four billion. Nothing
/// states where the union ends, so nothing would catch it either. The
/// bound is the case's own extent, and where the case has none the value
/// falls back to the step-over, which is the refusal the default makes.
#[test]
fn an_unbounded_case_has_no_bound_in_an_undelimited_union() {
    let (ast, bytes) = undelimited_writers_sample();
    let mut request = ProbeRequest::new(
        "Test::union_disc_enum_2_default_string",
        Encoding::PlainCdr1,
        Endianness::Little,
    );
    request.defaulted_discriminator_member = DefaultedDiscriminatorMember::StreamValue;
    let error = zerodds_wire_codec::decode::<serde_json::Value>(&ast, &request, &bytes)
        .expect_err("an unbounded case in an undelimited union is not read");
    assert!(
        format!("{error:?}").contains("undelimited union"),
        "{error:?}"
    );
}

/// The writer's sample under XCDR1, where an `@appendable` union is
/// `PlainCdr1` and carries no framing of its own.
fn undelimited_writers_sample() -> (zerodds_ast::Ast, Vec<u8>) {
    let composed = compose_source("try_construct.xml", SOURCE);
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let writer = ProbeRequest::new(
        "Test::union_disc_enum_1",
        Encoding::PlainCdr1,
        Endianness::Little,
    );
    let bytes = zerodds_wire_codec::encode(
        &composed.ast,
        &writer,
        &serde_json::json!({"_d": "VAL3", "x3": 3}),
    )
    .expect("union_disc_enum_1 encodes under XCDR1");
    // No DHEADER: the discriminator and the member, and nothing that
    // states where either ends.
    assert_eq!(bytes, [3, 0, 0, 0, 3, 0, 0, 0]);
    (composed.ast, bytes)
}
