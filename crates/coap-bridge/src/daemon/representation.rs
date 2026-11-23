// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The representations of a topic's samples (spec §4.5): the five forms a
//! CoAP payload takes, the Content-Format numbers they go under, and the
//! conversion between them through the topic's type.
//!
//! A topic with an `idl_file` has its type: every form can be read into a
//! value of that type and written from it (`zerodds-wire-codec`, the
//! encodings of DDS-XTypes 1.3 §7.4). A topic without one has none: its
//! samples pass through as they are, in the form the encapsulation header
//! of a DDS sample names (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).

use std::string::{String, ToString};
use std::sync::Arc;
use std::vec::Vec;

use serde_json::Value;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_types::dynamic::descriptor::ExtensibilityKind;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_wire_codec::{Emit, ProbeRequest};

/// A form a CoAP payload of a topic takes (spec §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// XCDR2, little endian (`application/x-dds-cdr2; bo=le`).
    Cdr2Le,
    /// XCDR2, big endian (`application/x-dds-cdr2; bo=be`).
    Cdr2Be,
    /// XCDR1, little endian (`application/x-dds-cdr1; bo=le`).
    Cdr1Le,
    /// The sample as JSON (`application/json`).
    Json,
    /// The sample as CBOR (`application/cbor`, RFC 8949 §6.2 from the JSON
    /// value).
    Cbor,
}

impl Form {
    /// The five forms, in the order the catalog names them.
    pub const ALL: [Form; 5] = [
        Form::Cdr2Le,
        Form::Cdr2Be,
        Form::Cdr1Le,
        Form::Json,
        Form::Cbor,
    ];

    /// The form of a DDS sample's body by the representation identifier of
    /// its encapsulation header; `None` for one no Content-Format carries
    /// (XCDR1 big endian, XML, a vendor's identifier).
    #[must_use]
    pub fn of_identifier(identifier: [u8; 2]) -> Option<Self> {
        if identifier[0] != 0x00 {
            return None;
        }
        match identifier[1] {
            0x07 | 0x09 | 0x0b => Some(Form::Cdr2Le),
            0x06 | 0x08 | 0x0a => Some(Form::Cdr2Be),
            0x01 | 0x03 => Some(Form::Cdr1Le),
            _ => None,
        }
    }
}

/// `content_format:` (spec §3, §4.5): the Content-Format number of each
/// form. The defaults are those of §4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentFormats {
    /// `content_format.cdr2_le_id`, 65000.
    pub cdr2_le: u16,
    /// `content_format.cdr2_be_id`, 65001.
    pub cdr2_be: u16,
    /// `content_format.cdr1_le_id`, 65002.
    pub cdr1_le: u16,
    /// `content_format.json_id`, 50.
    pub json: u16,
    /// `content_format.cbor_id`, 60.
    pub cbor: u16,
}

impl Default for ContentFormats {
    fn default() -> Self {
        Self {
            cdr2_le: crate::content_format::XCDR2_LE,
            cdr2_be: crate::content_format::XCDR2_BE,
            cdr1_le: crate::content_format::XCDR1_LE,
            json: crate::content_format::JSON,
            cbor: crate::content_format::CBOR,
        }
    }
}

impl ContentFormats {
    /// The number `form` goes under.
    #[must_use]
    pub const fn number(&self, form: Form) -> u16 {
        match form {
            Form::Cdr2Le => self.cdr2_le,
            Form::Cdr2Be => self.cdr2_be,
            Form::Cdr1Le => self.cdr1_le,
            Form::Json => self.json,
            Form::Cbor => self.cbor,
        }
    }

    /// The form a Content-Format number names; `None` for one of no form.
    #[must_use]
    pub fn form(&self, number: u64) -> Option<Form> {
        Form::ALL
            .into_iter()
            .find(|f| u64::from(self.number(*f)) == number)
    }

    /// Whether two forms share a number (a configuration that cannot be).
    #[must_use]
    pub fn has_duplicates(&self) -> bool {
        let numbers = Form::ALL.map(|f| self.number(f));
        (0..numbers.len()).any(|i| numbers[i + 1..].contains(&numbers[i]))
    }
}

/// The extensibility of a type (DDS-XTypes 1.3 §7.2.2.4.4.4.4): it decides
/// the encoding of a sample and its representation identifier (Table 60).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extensibility {
    /// `@final`.
    Final,
    /// `@appendable`, the default of an unannotated aggregate.
    Appendable,
    /// `@mutable`.
    Mutable,
}

impl Extensibility {
    /// From the `extensibility:` of a topic.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "final" => Some(Self::Final),
            "appendable" => Some(Self::Appendable),
            "mutable" => Some(Self::Mutable),
            _ => None,
        }
    }

    /// The runtime's name of it, for the writer's header.
    #[must_use]
    pub const fn for_repr(self) -> zerodds_types::qos::ExtensibilityForRepr {
        match self {
            Self::Final => zerodds_types::qos::ExtensibilityForRepr::Final,
            Self::Appendable => zerodds_types::qos::ExtensibilityForRepr::Appendable,
            Self::Mutable => zerodds_types::qos::ExtensibilityForRepr::Mutable,
        }
    }

    /// The encoding and byte order of `form` for a type of this
    /// extensibility (DDS-XTypes 1.3 §7.4.2, §7.4.3: XCDR1 has no
    /// delimited form, an appendable type is plain there); `None` for the
    /// forms that are no CDR.
    #[must_use]
    pub const fn encoding(self, form: Form) -> Option<(Encoding, Endianness)> {
        let endianness = match form {
            Form::Cdr2Be => Endianness::Big,
            Form::Cdr2Le | Form::Cdr1Le => Endianness::Little,
            Form::Json | Form::Cbor => return None,
        };
        let encoding = match (form, self) {
            (Form::Cdr1Le, Self::Mutable) => Encoding::PlCdr1,
            (Form::Cdr1Le, _) => Encoding::PlainCdr1,
            (_, Self::Final) => Encoding::PlainCdr2,
            (_, Self::Appendable) => Encoding::DelimitedCdr,
            (_, Self::Mutable) => Encoding::PlCdr2,
        };
        Some((encoding, endianness))
    }
}

/// The encoding and byte order a representation identifier names
/// (DDS-XTypes 1.3 Table 60); `None` for one of no CDR form.
#[must_use]
pub fn encoding_of(identifier: [u8; 2]) -> Option<(Encoding, Endianness)> {
    if identifier[0] != 0x00 {
        return None;
    }
    let encoding = match identifier[1] & !0x01 {
        0x00 => Encoding::PlainCdr1,
        0x02 => Encoding::PlCdr1,
        0x06 => Encoding::PlainCdr2,
        0x08 => Encoding::DelimitedCdr,
        0x0a => Encoding::PlCdr2,
        _ => return None,
    };
    let endianness = if identifier[1] & 0x01 == 0 {
        Endianness::Big
    } else {
        Endianness::Little
    };
    Some((encoding, endianness))
}

/// The type of a topic, from its `idl_file`.
#[derive(Clone)]
pub struct TopicType {
    ty: DynamicType,
    keyed: bool,
    /// The IDL the type came from, its `#include`s expanded: what
    /// `GET /schema/<slug>` returns (spec §5.2).
    idl: Arc<str>,
}

impl core::fmt::Debug for TopicType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TopicType")
            .field("type", &self.ty.qualified_name())
            .field("keyed", &self.keyed)
            .finish_non_exhaustive()
    }
}

impl TopicType {
    /// Reads `idl_file` (its `#include`s from its own directory) and finds
    /// `type_name` in it.
    ///
    /// # Errors
    /// The file does not read or compile, or declares no such type.
    pub fn load(idl_file: &str, type_name: &str) -> Result<Self, String> {
        let dir = std::path::Path::new(idl_file)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let loader = zerodds_idl_compose::PathLoader::new(vec![dir]);
        let composed = zerodds_idl_compose::compose(idl_file, &loader)
            .map_err(|e| format!("{idl_file}: {e}"))?;
        let text = match &composed.expanded {
            Some(expanded) => expanded.text.clone(),
            None => std::fs::read_to_string(idl_file).map_err(|e| format!("{idl_file}: {e}"))?,
        };
        Self::of(composed, type_name, text).map_err(|e| format!("{idl_file}: {e}"))
    }

    /// `type_name` in the IDL text `source`.
    ///
    /// # Errors
    /// The text does not compile, or declares no such type.
    pub fn from_idl(source: &str, type_name: &str) -> Result<Self, String> {
        let composed = zerodds_idl_compose::compose_source_with(
            "<idl>",
            source,
            &zerodds_idl_compose::PathLoader::bare(),
            zerodds_idl_compose::ComposeOptions::default(),
        );
        Self::of(composed, type_name, source.to_string())
    }

    fn of(
        composed: zerodds_idl_compose::Composed,
        type_name: &str,
        idl: String,
    ) -> Result<Self, String> {
        if composed.failed() {
            let messages: Vec<String> = composed
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect();
            return Err(messages.join("; "));
        }
        let ast = Arc::new(composed.ast);
        let ty = DynamicType::find(&ast, type_name)
            .map_err(|_| format!("declares no type {type_name}"))?;
        let keyed = ty
            .all_members_by_name()
            .values()
            .any(|m| m.descriptor().is_ok_and(|d| d.is_key));
        Ok(Self {
            ty,
            keyed,
            idl: Arc::from(idl),
        })
    }

    /// Whether the type has a key member: its samples belong to instances.
    #[must_use]
    pub fn keyed(&self) -> bool {
        self.keyed
    }

    /// The IDL the type came from.
    #[must_use]
    pub fn idl(&self) -> &str {
        &self.idl
    }

    /// The type's extensibility.
    #[must_use]
    pub fn extensibility(&self) -> Extensibility {
        match self.ty.extensibility() {
            Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
            Some(ExtensibilityKind::Final) => Extensibility::Final,
            _ => Extensibility::Appendable,
        }
    }

    fn request(&self, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
        ProbeRequest::new(&self.ty.qualified_name(), encoding, endianness)
    }

    /// The value of a DDS sample's `body`, read in the encoding and byte
    /// order its representation identifier names.
    ///
    /// # Errors
    /// An identifier of no CDR form, or a body that is no sample of the
    /// type.
    pub fn read_body(&self, body: &[u8], identifier: [u8; 2]) -> Result<Value, String> {
        let (encoding, endianness) = encoding_of(identifier).ok_or_else(|| {
            format!(
                "the representation {:02x}{:02x} is no CDR",
                identifier[0], identifier[1]
            )
        })?;
        zerodds_wire_codec::decode::<Value>(
            self.ty.ast(),
            &self.request(encoding, endianness),
            body,
        )
        .map_err(|e| e.to_string())
    }

    /// The value of a CoAP payload in `form`.
    ///
    /// # Errors
    /// A payload that is no sample of the type in that form.
    pub fn read(&self, form: Form, payload: &[u8]) -> Result<Value, String> {
        match form {
            Form::Json => serde_json::from_slice(payload).map_err(|e| format!("JSON: {e}")),
            Form::Cbor => {
                ciborium::from_reader::<Value, _>(payload).map_err(|e| format!("CBOR: {e}"))
            }
            _ => {
                let (encoding, endianness) =
                    self.extensibility().encoding(form).ok_or("no CDR form")?;
                zerodds_wire_codec::decode::<Value>(
                    self.ty.ast(),
                    &self.request(encoding, endianness),
                    payload,
                )
                .map_err(|e| e.to_string())
            }
        }
    }

    /// `value` as a CoAP payload in `form`.
    ///
    /// # Errors
    /// A value that is no sample of the type.
    pub fn write(&self, form: Form, value: &Value) -> Result<Vec<u8>, String> {
        match form {
            Form::Json => serde_json::to_vec(value).map_err(|e| e.to_string()),
            Form::Cbor => {
                let mut out = Vec::new();
                ciborium::into_writer(value, &mut out).map_err(|e| format!("CBOR: {e}"))?;
                Ok(out)
            }
            _ => {
                let (encoding, endianness) =
                    self.extensibility().encoding(form).ok_or("no CDR form")?;
                zerodds_wire_codec::encode(
                    self.ty.ast(),
                    &self.request(encoding, endianness),
                    value,
                )
                .map_err(|e| e.to_string())
            }
        }
    }

    /// The key holder of the instance `value` names: its key members as
    /// PLAIN_CDR2 big endian (DDS-XTypes 1.3 §7.6.8), what a dispose
    /// carries as `serializedKey` (DDSI-RTPS 2.5 §9.4.2.11).
    ///
    /// # Errors
    /// A value that is no sample of the type.
    pub fn key_holder(&self, value: &Value) -> Result<Vec<u8>, String> {
        let request = self.request(Encoding::PlainCdr2, Endianness::Big);
        zerodds_wire_codec::encode::key_holder(self.ty.ast(), &request, value)
            .map_err(|e| e.to_string())
    }

    /// The KeyHash of the instance `value` names (DDSI-RTPS 2.5 §9.6.4.8).
    ///
    /// # Errors
    /// A value that is no sample of the type.
    pub fn key_hash(&self, value: &Value) -> Result<[u8; 16], String> {
        let mut request = self.request(Encoding::PlainCdr2, Endianness::Big);
        request.emit = Emit::KeyHash;
        let hash = zerodds_wire_codec::encode(self.ty.ast(), &request, value)
            .map_err(|e| e.to_string())?;
        hash.try_into()
            .map_err(|_| "a KeyHash is sixteen octets".to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const IDL: &str = "@appendable struct Reading { @key long id; double value; string unit; };";

    /// Spec §4.5: the five forms and their numbers; a configured number
    /// names its form, a number of no form none.
    #[test]
    fn coapbridge_4_5_the_five_forms_and_their_numbers() {
        let formats = ContentFormats::default();
        assert_eq!(
            Form::ALL.map(|f| formats.number(f)),
            [65000, 65001, 65002, 50, 60]
        );
        for form in Form::ALL {
            assert_eq!(formats.form(u64::from(formats.number(form))), Some(form));
        }
        assert_eq!(formats.form(0), None);
        assert!(!formats.has_duplicates());
        let clash = ContentFormats {
            cbor: 50,
            ..ContentFormats::default()
        };
        assert!(clash.has_duplicates());
    }

    /// DDS-XTypes 1.3 Table 60: the identifier of a DDS sample names the
    /// form it can go under without a type; XCDR1 big endian and XML have
    /// none.
    #[test]
    fn coapbridge_4_5_the_identifier_names_the_form() {
        for (second, form) in [
            (0x07, Some(Form::Cdr2Le)),
            (0x09, Some(Form::Cdr2Le)),
            (0x0b, Some(Form::Cdr2Le)),
            (0x06, Some(Form::Cdr2Be)),
            (0x08, Some(Form::Cdr2Be)),
            (0x0a, Some(Form::Cdr2Be)),
            (0x01, Some(Form::Cdr1Le)),
            (0x03, Some(Form::Cdr1Le)),
            (0x00, None),
            (0x02, None),
            (0x04, None),
        ] {
            assert_eq!(Form::of_identifier([0x00, second]), form, "{second:#04x}");
        }
        assert_eq!(Form::of_identifier([0x80, 0x07]), None);
    }

    /// DDS-XTypes 1.3 §7.4.2, §7.4.3: an appendable type is DELIMITED_CDR
    /// under XCDR2 and plain under XCDR1, a mutable one PL_CDR2 and PL_CDR.
    #[test]
    fn xtypes_7_4_the_encoding_of_a_form_follows_the_extensibility() {
        use Extensibility::{Appendable, Final, Mutable};
        assert_eq!(
            Appendable.encoding(Form::Cdr2Le),
            Some((Encoding::DelimitedCdr, Endianness::Little))
        );
        assert_eq!(
            Appendable.encoding(Form::Cdr1Le),
            Some((Encoding::PlainCdr1, Endianness::Little))
        );
        assert_eq!(
            Mutable.encoding(Form::Cdr2Be),
            Some((Encoding::PlCdr2, Endianness::Big))
        );
        assert_eq!(
            Mutable.encoding(Form::Cdr1Le),
            Some((Encoding::PlCdr1, Endianness::Little))
        );
        assert_eq!(
            Final.encoding(Form::Cdr2Le),
            Some((Encoding::PlainCdr2, Endianness::Little))
        );
        assert_eq!(Final.encoding(Form::Json), None);
    }

    /// Spec §4.5: a sample written in one form reads back as the same value
    /// from every other; the XCDR2 forms of an appendable type start with
    /// the DHEADER (DDS-XTypes 1.3 §7.4.2), the XCDR1 form does not, and a
    /// double is aligned to 4 under XCDR2 and to 8 under XCDR1 (§7.4.1.1.1,
    /// §7.4.3.2).
    #[test]
    fn coapbridge_4_5_every_form_carries_the_same_sample() {
        let ty = TopicType::from_idl(IDL, "Reading").unwrap();
        assert!(ty.keyed());
        assert_eq!(ty.extensibility(), Extensibility::Appendable);
        let value = json!({ "id": 7, "value": 1.5, "unit": "V" });
        let le = ty.write(Form::Cdr2Le, &value).unwrap();
        assert_eq!(&le[..4], &[18, 0, 0, 0], "DHEADER: 18 octets follow");
        assert_eq!(&le[4..8], &[7, 0, 0, 0]);
        assert_eq!(&le[8..16], &1.5f64.to_le_bytes(), "XCDR2: the double at 4");
        let be = ty.write(Form::Cdr2Be, &value).unwrap();
        assert_eq!(&be[..4], &[0, 0, 0, 18]);
        let cdr1 = ty.write(Form::Cdr1Le, &value).unwrap();
        assert_eq!(&cdr1[..4], &[7, 0, 0, 0], "XCDR1: no DHEADER");
        assert_eq!(
            &cdr1[8..16],
            &1.5f64.to_le_bytes(),
            "XCDR1: the double at 8"
        );
        for form in Form::ALL {
            let payload = ty.write(form, &value).unwrap();
            assert_eq!(ty.read(form, &payload).unwrap(), value, "{form:?}");
        }
        assert_eq!(ty.read_body(&le, [0x00, 0x09]).unwrap(), value);
        assert_eq!(ty.read_body(&be, [0x00, 0x08]).unwrap(), value);
        assert_eq!(ty.read_body(&cdr1, [0x00, 0x01]).unwrap(), value);
        assert!(ty.read_body(&le, [0x00, 0x04]).is_err(), "XML is no CDR");
        assert_eq!(
            ty.key_hash(&value).unwrap(),
            [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        // DDS-XTypes 1.3 §7.6.8: the holder of `@key long id = 7`, big
        // endian, before the padding of the KeyHash.
        assert_eq!(ty.key_holder(&value).unwrap(), [0, 0, 0, 7]);
    }

    /// RFC 8949 §6.2: the CBOR of the JSON value — a map of text keys, the
    /// integer as a CBOR integer, the double as a float.
    #[test]
    fn rfc8949_6_2_the_cbor_form_is_the_json_value_in_cbor() {
        let ty = TopicType::from_idl(IDL, "Reading").unwrap();
        let cbor = ty
            .write(Form::Cbor, &json!({ "id": 7, "value": 1.5, "unit": "V" }))
            .unwrap();
        assert_eq!(cbor[0], 0xa3, "a map of three pairs");
        assert_eq!(&cbor[1..4], &[0x62, b'i', b'd']);
        assert_eq!(cbor[4], 0x07, "the integer 7");
        assert!(ty.read(Form::Cbor, &[0xff]).is_err());
        assert!(ty.read(Form::Json, b"{\"id\":").is_err());
        assert!(
            ty.read(Form::Cdr2Le, &[1, 2]).is_err(),
            "too short for the type"
        );
    }

    /// Spec §5.2: the IDL of `/schema/<slug>` is the text the type came
    /// from; a type the text does not declare is refused.
    #[test]
    fn coapbridge_5_2_the_schema_is_the_idl_of_the_type() {
        let ty = TopicType::from_idl(IDL, "Reading").unwrap();
        assert_eq!(ty.idl(), IDL);
        assert!(TopicType::from_idl(IDL, "Missing").is_err());
        assert!(TopicType::from_idl("struct {", "X").is_err());
    }
}
