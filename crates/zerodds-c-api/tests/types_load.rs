// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_types_load`: the types a document defines, from IDL and from a DDS-XML type library
//! (`dds::core::xtypes::TypeProvider`, DDS-PSM-Cxx 1.0 §7.5).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CStr, CString};
use std::ptr;

use zerodds::topic_ffi::zerodds_string_free;
use zerodds::types_ffi::{zerodds_types_describe_idl, zerodds_types_load};

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;
const UNSUPPORTED: i32 = -13;

fn load(uri: &str) -> Result<serde_json::Value, i32> {
    let uri = CString::new(uri).unwrap();
    let mut out = ptr::null_mut();
    // SAFETY: NUL-terminated string, writable out pointer; the text is freed once.
    unsafe {
        let rc = zerodds_types_load(uri.as_ptr(), &mut out);
        if rc != OK {
            return Err(rc);
        }
        let json = serde_json::from_str(CStr::from_ptr(out).to_str().unwrap()).unwrap();
        zerodds_string_free(out);
        Ok(json)
    }
}

fn find<'a>(doc: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    doc["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("no type {name} in {doc}"))
}

#[test]
fn idl_types_are_described_with_members_keys_enumerators_and_aliases() {
    let idl = "module m { enum Color { RED, @value(5) GREEN }; typedef string<8> Label; \
               @final struct Track { @key long id; Label lbl; sequence<double, 4> xs; \
               @optional Color shade; long grid[2][3]; }; \
               union U switch (long) { case 1: case 2: long a; case 3: string b; }; \
               bitset Flags { bitfield<3> low; bitfield<5> high; }; };";
    let doc = load(&format!("data:text/plain,{}", idl.replace(' ', "%20"))).unwrap();

    let track = find(&doc, "m::Track");
    assert_eq!(track["kind"], "struct");
    assert_eq!(track["ext"], "final");
    assert_eq!(track["members"][0]["name"], "id");
    assert_eq!(track["members"][0]["key"], true);
    assert_eq!(
        track["members"][1]["type"],
        serde_json::json!({"k": "ref", "n": "m::Label"})
    );
    assert_eq!(track["members"][2]["type"]["k"], "seq");
    assert_eq!(track["members"][2]["type"]["bound"], 4);
    assert_eq!(track["members"][3]["optional"], true);
    assert_eq!(track["members"][4]["type"]["k"], "array");
    assert_eq!(
        track["members"][4]["type"]["dims"],
        serde_json::json!([2, 3])
    );
    assert_eq!(
        track["members"][4]["type"]["e"],
        serde_json::json!({"k": "prim", "n": "long"})
    );

    let color = find(&doc, "m::Color");
    assert_eq!(color["enumerators"][1]["name"], "GREEN");
    assert_eq!(color["enumerators"][1]["value"], 5);
    assert_eq!(find(&doc, "m::Label")["aliased"]["bound"], 8);
    assert_eq!(
        find(&doc, "m::U")["cases"][0]["labels"],
        serde_json::json!(["1", "2"])
    );
    assert_eq!(find(&doc, "m::Flags")["fields"][1]["bits"], 5);
}

#[test]
fn a_dds_xml_type_library_is_described_like_the_idl_it_stands_for() {
    // Only the spellings of DDS-XML 1.0 (`dds-xml_type_definitions_nonamespace.xsd`): `int32`,
    // `string`, `type="nonBasic"` with `nonBasicTypeName` for a named type, `extensibility`
    // from `final|append|mutable`.
    let xml = r#"<types><module name="m"><enum name="Color"><enumerator name="RED"/>
        <enumerator name="GREEN" value="5"/></enum>
        <struct name="Track" extensibility="final">
        <member name="id" type="int32" key="true"/>
        <member name="label" type="string" stringMaxLength="8"/>
        <member name="shade" type="nonBasic" nonBasicTypeName="m::Color"/>
        </struct></module></types>"#;
    let doc = load(&format!(
        "data:text/xml,{}",
        xml.replace([' ', '\n'], "%20")
    ))
    .unwrap();
    let track = find(&doc, "m::Track");
    assert_eq!(track["ext"], "final");
    assert_eq!(track["members"][0]["key"], true);
    assert_eq!(track["members"][1]["type"]["bound"], 8);
    assert_eq!(
        track["members"][2]["type"],
        serde_json::json!({"k": "ref", "n": "m::Color"})
    );
    assert_eq!(find(&doc, "m::Color")["enumerators"][1]["value"], 5);
}

#[test]
fn a_document_that_cannot_be_read_and_an_unknown_scheme_are_refused() {
    assert_eq!(load("gopher://example.org/t.idl").unwrap_err(), UNSUPPORTED);
    assert_eq!(
        load("file:///nonexistent/zerodds_types.idl").unwrap_err(),
        BAD_PARAMETER
    );
    let idl = CString::new("struct {").unwrap();
    let mut out = ptr::null_mut();
    // SAFETY: NUL-terminated string, writable out pointer.
    assert_eq!(
        unsafe { zerodds_types_describe_idl(idl.as_ptr(), &mut out) },
        BAD_PARAMETER
    );
}

#[test]
fn bitmasks_unions_with_a_default_and_enum_discriminators_are_described() {
    let idl = "module m { enum E { X, Y, Z }; \
               @bit_bound(16) bitmask Perm { READ, @position(3) WRITE, EXEC }; \
               union Wide switch (long) { case 1: long a; case 2: case 5: string b; default: double c; }; \
               union ByEnum switch (E) { case X: long a; case Y: string b; }; };";
    let doc = load(&format!("data:text/plain,{}", idl.replace(' ', "%20"))).unwrap();

    let perm = find(&doc, "m::Perm");
    assert_eq!(perm["kind"], "bitmask");
    assert_eq!(perm["bit_bound"], 16);
    assert_eq!(
        perm["flags"],
        serde_json::json!([
            {"name": "READ", "position": 0},
            {"name": "WRITE", "position": 3},
            {"name": "EXEC", "position": 4},
        ])
    );

    let wide = find(&doc, "m::Wide");
    assert_eq!(wide["cases"][0]["default"], false);
    assert_eq!(wide["cases"][1]["labels"], serde_json::json!(["2", "5"]));
    assert_eq!(wide["cases"][2]["default"], true);

    let by_enum = find(&doc, "m::ByEnum");
    assert_eq!(
        by_enum["disc"],
        serde_json::json!({"k": "ref", "n": "m::E"})
    );
    assert_eq!(by_enum["cases"][1]["labels"], serde_json::json!(["1"]));
}
