// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A bitmask as a union discriminator, over the two declarations the OMG
//! `dds-xtypes` suite pairs in `union_uint32_bitmask32` and
//! `union_uint32_bitmask16`.
//!
//! DDS-XTypes 1.3 §7.5.2.6 admits a bitmask in `<discriminator
//! type="nonBasic"/>`; IDL 4.2 §7.4.1.4.4.3 rule (50) admits none, which
//! is why `types/xml/unions_bitmask.xml` exists only as XML. The front
//! end has read it since the XML driver landed, and
//! `crates/xml-compose/tests/table_15_bitmask.rs` pins the assignability
//! verdict on it. What was missing below that is here:
//!
//! * the **wire** — a bitmask discriminator is written in the holder its
//!   `@bit_bound` names (§7.4.1.1.3.2), which is why the 32-bit pair has
//!   the same octets as a `uint32` discriminator and the 16-bit pair
//!   does not;
//! * the **read back** — `selector_of` turns the discriminator's value
//!   into the integer a case label is compared against, and a bitmask's
//!   value is a set of bit names, not an integer;
//! * the **sample** — the suite ships 49 samples as XML and 43 as JSON,
//!   and `data/xml/union_bitmask.xml` is the only one without a JSON
//!   twin that an active case names.
//!
//! Measured on the Linux test host, 2026-09-05, against the four vendors on the
//! host: no IDL front end accepts the construct — Cyclone `idlc`
//! "Invalid switch type 'bitmask'", `fastddsgen` "Not supported type
//! discriminator.", `opendds_idl` (TAO IDL FE 2.5.24, OpenDDS 3.34.0)
//! does not parse `bitmask` at all, `rtiddsgen` 4.7.0 stops at the
//! keyword — and RTI's XML front end, given the same file with an
//! `<enum>` in place of the `<bitmask>` as the control, answers
//! "discriminator type 'B_16' not found". So there is no vendor wire
//! form to follow and none to diverge from: what is pinned below is the
//! specification's own, and no dialect axis is involved.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::TypeNode;
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_omg_xtypes_test_main::data;
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_xml_compose::compose_source;

/// `types/xml/unions_bitmask.xml` verbatim, plus the `union_uint32` from
/// `types/xml/unions.xml` the suite pairs it against.
const SOURCE: &str = r#"<dds>
    <types>
      <module name="Test">
        <bitmask name="B_16" bitBound="16">
            <flag name="B_FLAG_1" value="0"/>
            <flag name="B_FLAG_2" value="1"/>
        </bitmask>
        <bitmask name="B_32" bitBound="32">
            <flag name="B_FLAG_1" value="0"/>
            <flag name="B_FLAG_2" value="1"/>
            <flag name="B_FLAG_3" value="2"/>
        </bitmask>
        <union name="union_uint32" ><discriminator type="uint32" />
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
        <union name="union_bitmask16" ><discriminator type="nonBasic" nonBasicTypeName="B_16"/>
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
        <union name="union_bitmask32" ><discriminator type="nonBasic" nonBasicTypeName="B_32"/>
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
      </module>
    </types>
</dds>"#;

/// `data/xml/union_bitmask.xml` verbatim — the file with no JSON twin.
const UNION_BITMASK_XML: &str = "<union_bitmask>\n  <discriminator>2</discriminator>\n  \
                                 <x1>128</x1>\n</union_bitmask>\n";

/// `data/xml/union_uint32.xml` verbatim, whose discriminator is written
/// in hexadecimal where the file above writes decimal.
const UNION_UINT32_XML: &str = "<union_primitives>\n  <discriminator>0x02</discriminator>\n  \
                                <x1>128</x1>\n</union_primitives>\n";

struct Model {
    ast: zerodds_ast::Ast,
}

impl Model {
    fn read() -> Self {
        let composed = compose_source("unions_bitmask.xml", SOURCE);
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        Self { ast: composed.ast }
    }

    fn type_(&self, name: &str) -> Id<TypeNode> {
        let segments = ["Test", name].map(str::to_owned).to_vec();
        self.ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .unwrap_or_else(|| panic!("the document declares {name}"))
    }

    /// The suite runs `@appendable` types under XCDR2, which is
    /// `DelimitedCdr`, and little-endian on every host it runs on.
    fn request(name: &str) -> ProbeRequest {
        ProbeRequest::new(
            &format!("Test::{name}"),
            Encoding::DelimitedCdr,
            Endianness::Little,
        )
    }
}

/// The 32-bit pair has one wire form, and the 16-bit pair has another.
///
/// §7.4.1.1.3.2 gives a bitmask the holder its `@bit_bound` names, so
/// `B_32` is four octets and `B_16` is two. That is the whole of why the
/// suite expects the first pair to communicate and the second not to,
/// and it is what makes the second refusal a wire fact rather than a
/// policy: a `uint32` writer's four octets are not a `B_16` reader's
/// two.
#[test]
fn a_bitmask_discriminator_is_as_wide_as_its_bit_bound() {
    let model = Model::read();
    let sample = serde_json::json!({"_d": 2, "x1": 128});

    let uint32 = zerodds_wire_codec::encode(&model.ast, &Model::request("union_uint32"), &sample)
        .expect("union_uint32 encodes");
    // DHEADER(6) | discriminator 2 as uint32 | x1 = 128 as int16.
    assert_eq!(uint32, [0x06, 0, 0, 0, 0x02, 0, 0, 0, 0x80, 0x00]);

    let named = serde_json::json!({"_d": ["B_FLAG_2"], "x1": 128});
    let bitmask32 =
        zerodds_wire_codec::encode(&model.ast, &Model::request("union_bitmask32"), &named)
            .expect("union_bitmask32 encodes");
    assert_eq!(
        bitmask32, uint32,
        "a 32-bit bitmask discriminator is the same four octets as uint32"
    );

    let bitmask16 =
        zerodds_wire_codec::encode(&model.ast, &Model::request("union_bitmask16"), &named)
            .expect("union_bitmask16 encodes");
    // DHEADER(4) | discriminator 2 as uint16 | x1 = 128 as int16.
    assert_eq!(bitmask16, [0x04, 0, 0, 0, 0x02, 0x00, 0x80, 0x00]);
}

/// The sample a `uint32` writer sends reads back through a 32-bit
/// bitmask discriminator.
///
/// The discriminator's value is the set of bit names — `read_bitmask`
/// hands back names and `write_bitmask` takes them — and the case label
/// it selects is an integer, so `selector_of` packs the set back into
/// the anchor. Without that arm the discriminator reaches the label
/// comparison as an array and the whole sample fails to decode, which is
/// the `DATA_NOT_CORRECT` the suite reported for both cases.
#[test]
fn a_uint32_writers_sample_reads_back_through_a_32_bit_bitmask() {
    let model = Model::read();
    let bytes = zerodds_wire_codec::encode(
        &model.ast,
        &Model::request("union_uint32"),
        &serde_json::json!({"_d": 2, "x1": 128}),
    )
    .expect("union_uint32 encodes");

    let read: serde_json::Value =
        zerodds_wire_codec::decode(&model.ast, &Model::request("union_bitmask32"), &bytes)
            .expect("union_bitmask32 decodes a uint32 writer's sample");
    assert_eq!(read, serde_json::json!({"_d": ["B_FLAG_2"], "x1": 128}));
}

/// A bit the bitmask does not declare is refused rather than dropped.
///
/// §7.4.1.1.3.2 puts a bit the type does not name outside the bitmask,
/// so a stream that sets one is a stream this type does not describe.
/// `B_32` declares positions 0, 1 and 2; position 3 is the value 8.
#[test]
fn an_undeclared_bit_in_the_discriminator_is_refused() {
    let model = Model::read();
    let bytes = zerodds_wire_codec::encode(
        &model.ast,
        &Model::request("union_uint32"),
        &serde_json::json!({"_d": 8, "x2": 7}),
    )
    .expect("union_uint32 encodes");
    let read: Result<serde_json::Value, _> =
        zerodds_wire_codec::decode(&model.ast, &Model::request("union_bitmask32"), &bytes);
    assert!(
        read.is_err(),
        "an undeclared bit must not read back as an empty set: {read:?}"
    );
}

/// The suite's XML sample for the bitmask unions is read, because the
/// JSON twin the client would rather have does not exist.
///
/// `data/xml/union_bitmask.xml` is the one of the suite's 35 samples
/// with no `data/json` file beside it. Read as XML it is text; completed
/// against the type it is the discriminator's bit names and an integer
/// branch — the shape `zerodds_wire_codec` encodes.
#[test]
fn the_bitmask_samples_xml_completes_to_the_codecs_value_contract() {
    let model = Model::read();
    let raw = data::from_xml(UNION_BITMASK_XML).expect("the sample parses");
    assert_eq!(raw, serde_json::json!({"_d": "2", "x1": "128"}));

    let completed = data::complete(&model.ast, model.type_("union_bitmask32"), &raw)
        .expect("the sample completes");
    assert_eq!(
        completed,
        serde_json::json!({"_d": ["B_FLAG_2"], "x1": 128})
    );
    // And it is the value the wire carries, not merely a value that
    // encodes: the same octets a `uint32` writer sends.
    let bytes =
        zerodds_wire_codec::encode(&model.ast, &Model::request("union_bitmask32"), &completed)
            .expect("the completed sample encodes");
    assert_eq!(bytes, [0x06, 0, 0, 0, 0x02, 0, 0, 0, 0x80, 0x00]);
}

/// The publisher's own sample, whose discriminator the same file writes
/// in hexadecimal.
///
/// `data/xml/union_uint32.xml` spells the value `0x02` where
/// `union_bitmask.xml` spells it `2`. Both are the value 2, and a reader
/// that took only one spelling would select no branch for the other.
#[test]
fn a_hexadecimal_discriminator_is_the_same_value_as_a_decimal_one() {
    let model = Model::read();
    let raw = data::from_xml(UNION_UINT32_XML).expect("the sample parses");
    assert_eq!(raw, serde_json::json!({"_d": "0x02", "x1": "128"}));

    let completed = data::complete(&model.ast, model.type_("union_uint32"), &raw)
        .expect("the sample completes");
    assert_eq!(completed, serde_json::json!({"_d": 2, "x1": 128}));
}

/// The XML sample layout is tried after the JSON one, not before.
///
/// Every sample but one has both, and the JSON is what this client has
/// always read; a fallback that overtook it would change 34 comparisons
/// to fix one.
#[test]
fn the_json_sample_is_tried_before_the_xml_one() {
    let paths = data::candidate_paths(Some("data"), Some("union_bitmask"));
    assert_eq!(
        paths[0],
        std::path::PathBuf::from("data/json/union_bitmask.json")
    );
    assert!(
        paths
            .iter()
            .any(|p| *p == std::path::PathBuf::from("data/xml/union_bitmask.xml")),
        "{paths:?}"
    );
}
