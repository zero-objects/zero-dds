// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-derive`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-derive`: the two derives it
//! re-exports. Their expansion names this crate's `codec` (`DdsTypeInfo`,
//! `DdsSerialize`, `DdsDeserialize`, `DdsMember`, `TypeDesc`,
//! `MemberDesc`, `EnumeratorDesc`, `HashId`, `AutoId`, `With`,
//! `with_field_value`, `default_struct`), `WireError`, `WireRepr`,
//! `IdlOut`, `Written`, `Encapsulation`, `idl`, `ModelError` and
//! `__private` (`dcps`, `types`, `wire`, the codec glue, `has_key`); a
//! struct's attributes become the description, a field-less enum's
//! variants the enumerators, `#[wire(idl = …)]` the `WireRepr` impl.
//!
//! Oracle: the IDL annotation each attribute stands for, DDS-XTypes 1.3
//! §7.2.2.4.4.4 (extensibility), §7.3.1.2.1 (`@key`, `@id`, `@hashid`,
//! `@autoid`, `@nested`), IDL 4.2 `@bit_bound` and `@value`.

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_codec::codec::{
    AutoId, DdsTypeInfo, EnumeratorDesc, Extensibility, HashId, MemberDesc, TypeKind, ValueDesc,
};
use zerodds_codec::{DdsMember, DdsType, WireRepr};

#[derive(WireRepr, Debug, Clone, PartialEq)]
#[wire(idl = "string")]
struct Code(String);

impl core::fmt::Display for Code {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for Code {
    type Error = String;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        if text.starts_with('C') {
            Ok(Self(text.into()))
        } else {
            Err(format!("a code starting with C, not {text:?}"))
        }
    }
}

#[derive(DdsType, Debug, Clone, Copy, PartialEq)]
#[dds(name = "x::Shade", bit_bound = 16)]
enum Shade {
    #[dds(name = "DARK")]
    Dark,
    #[dds(name = "LIGHT", value = 7)]
    Light,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "x::Item", extensibility = "mutable", autoid = "hash", nested)]
struct Item {
    #[dds(key, id = 3)]
    id: u32,
    #[dds(name = "label", bound = 10)]
    text: String,
    #[dds(hashid = "h")]
    hashed: i8,
    #[dds(optional)]
    code: Option<Code>,
    shade: Shade,
}

#[test]
fn xtypes_7_3_1_2_1_the_attributes_are_the_annotations() {
    let layout = <Item as DdsTypeInfo>::LAYOUT;
    assert_eq!(layout.name, "x::Item");
    let TypeKind::Struct {
        extensibility,
        autoid,
        nested,
        members,
        values,
        count,
    } = layout.kind
    else {
        panic!("a struct");
    };
    assert_eq!(extensibility, Extensibility::Mutable);
    assert_eq!(autoid, Some(AutoId::Hash));
    assert!(nested);
    assert_eq!(count, 5);
    assert_eq!(
        members,
        [
            MemberDesc::new("id").key().id(3),
            MemberDesc::new("label").bound(10),
            MemberDesc::new("hashed").hash_id(HashId::Text("h")),
            MemberDesc::new("code"),
            MemberDesc::new("shade"),
        ]
    );
    assert!(matches!(values(3), Some(ValueDesc::Optional { .. })));
    assert!(matches!(values(4), Some(ValueDesc::Type { .. })));
    assert!(values(5).is_none());
}

#[test]
fn idl_4_2_a_field_less_enum_is_an_enumeration() {
    let ValueDesc::Type { desc } = <Shade as DdsMember>::DESC else {
        panic!("a described type");
    };
    assert_eq!(desc.name, "x::Shade");
    let TypeKind::Enum {
        enumerators,
        bit_bound,
        extensibility,
    } = desc.kind
    else {
        panic!("an enumeration");
    };
    assert_eq!(bit_bound, Some(16));
    assert_eq!(extensibility, None, "unwritten, the IDL default");
    assert_eq!(
        enumerators,
        [
            EnumeratorDesc {
                name: "DARK",
                value: None
            },
            EnumeratorDesc {
                name: "LIGHT",
                value: Some(7)
            },
        ]
    );
    assert_eq!(<Shade as DdsMember>::default_member().unwrap(), Shade::Dark);
}

#[test]
fn issue_33_derive_wire_repr_reads_through_try_from_and_writes_through_display() {
    assert_eq!(Code::from_wire("C12").unwrap(), Code("C12".into()));
    let error = Code::from_wire("X").unwrap_err();
    assert!(
        error.to_string().contains("a code starting with C"),
        "{error}"
    );
    let item = Item {
        id: 1,
        text: "t".into(),
        hashed: -1,
        code: Some(Code("C1".into())),
        shade: Shade::Light,
    };
    let mut bytes = Vec::new();
    zerodds_codec::encode(&item, zerodds_codec::Encapsulation::Xcdr2Le, &mut bytes).unwrap();
    let back: Item = zerodds_codec::decode(&bytes, zerodds_codec::Encapsulation::Xcdr2Le).unwrap();
    assert_eq!(back, item);
    let idl = Item::idl().unwrap();
    assert!(idl.contains("@key @id(3) unsigned long id;"), "{idl}");
    assert!(idl.contains("@optional string code;"), "{idl}");
    assert!(idl.contains("::x::Shade shade;"), "{idl}");
}
