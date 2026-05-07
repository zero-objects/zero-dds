// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DdsType` — der Trait, den User-Typen erfuellen muessen, um ueber
//! DDS verschickt zu werden.
//!
//! # Usage
//!
//! User-Typen erfuellen den Trait entweder per Hand oder ueber
//! die Codegen-Pipeline `zerodds-idl-rust` (IDL → Rust mit
//! abgeleitetem `DdsType`-Impl). Die Encoder-/Decoder-Paerchen
//! folgen XCDR2-Konvention (siehe `zerodds-cdr`); der Trait bleibt
//! transport- und qos-agnostisch.
//!
//! # Interop-Hinweis
//!
//! Fuer Interop mit Cyclone/Fast-DDS MUSS der `TYPE_NAME` exakt mit
//! dem Remote-Topic-Typnamen uebereinstimmen (strict equality). Das
//! IDL-Type-Namespacing (z.B. `std_msgs::msg::String`) muss
//! beruecksichtigt werden.

extern crate alloc;
use alloc::vec::Vec;

pub use zerodds_cdr::{KEY_HASH_LEN, PlainCdr2BeKeyHolder, compute_key_hash};

/// XTypes 1.3 §7.4.5 Struct-Extensibility-Kind. Wire-relevante
/// Information fuer den Sample-Encoder; gleicht den IDL-Annotationen
/// `@final` / `@appendable` / `@mutable`.
///
/// Spec: `zerodds-xcdr2-rust` §2 referenziert das als
/// `ExtensibilityKind`; der Implementations-Name `Extensibility` und
/// der spec-aligned Alias [`ExtensibilityKind`] sind identisch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Extensibility {
    /// `@final`: tight-packed body, kein Header.
    Final = 0,
    /// `@appendable`: 4-byte DHEADER + body, forward-compatible.
    Appendable = 1,
    /// `@mutable`: pro Member ein EMHEADER + Body.
    Mutable = 2,
}

/// Spec-aligned Alias: `zerodds-xcdr2-rust` §2 referenziert die
/// Extensibility-Enum unter dem Namen `ExtensibilityKind`. Wir
/// behalten `Extensibility` als Implementations-Name; beide sind via
/// Alias identisch.
pub type ExtensibilityKind = Extensibility;

/// Typ, der via DDS published/subscribed werden kann.
pub trait DdsType: Sized {
    /// Vollqualifizierter Topic-Type-Name (z.B. `"std_msgs::String"`).
    /// Muss exakt zum Peer-Type-Namen passen (strict matching).
    const TYPE_NAME: &'static str;

    /// XTypes 1.3 §7.4.5 Struct-Extensibility-Kind. Default `Final`
    /// fuer Backwards-Kompat zu pre-`EXTENSIBILITY`-Codegen-Outputs.
    /// Spec: zerodds-xcdr2-rust §2.3.
    const EXTENSIBILITY: Extensibility = Extensibility::Final;

    /// `true` wenn der Topic-Type **keyed** ist (mindestens ein Member
    /// mit `@key`-Annotation). Default `false` — Caller (proc-macro)
    /// ueberschreibt fuer keyed Types und implementiert auch
    /// [`Self::encode_key_holder_be`].
    ///
    /// Spec: XTypes 1.3 §7.6.8 (KeyHash-Pflicht fuer keyed Topics).
    ///
    /// Hinweis (`zerodds-xcdr2-rust` §11 Errata): die Spec referenziert
    /// dieses Feld als `IS_KEYED`. Wir behalten `HAS_KEY` fuer
    /// Source-Kompat zu Pre-1.0 Code; der spec-aligned Alias
    /// [`Self::IS_KEYED`] gibt jederzeit denselben Wert.
    const HAS_KEY: bool = false;

    /// Spec-aligned Alias fuer [`Self::HAS_KEY`].
    /// `zerodds-xcdr2-rust` §2 referenziert das als `IS_KEYED`.
    const IS_KEYED: bool = Self::HAS_KEY;

    /// Maximale Groesse des PLAIN_CDR2-BE-KeyHolder-Streams in Bytes
    /// (XTypes 1.3 §7.6.8.4 Step 5). `None` = nicht keyed oder
    /// unbounded (MD5-Pfad). `Some(n)` mit `n <= 16` = zero-pad-Pfad.
    const KEY_HOLDER_MAX_SIZE: Option<usize> = None;

    /// `true` wenn der Type mit `@nested` annotiert ist (XTypes 1.3
    /// §7.4.6.3.5). Nested-Types sind nur als Member anderer Types
    /// gedacht und MUESSEN nicht als DDS-Topic-Type registriert
    /// werden. `DomainParticipant::create_topic` lehnt registration
    /// von nested-Types mit `PreconditionNotMet` ab.
    const IS_NESTED: bool = false;

    /// XTypes 1.3 §7.3.4.2 — TypeIdentifier des Types fuer XTypes-aware
    /// Discovery + Compatibility-Matching. Default `TypeIdentifier::None`
    /// signalisiert "type-id nicht bereitgestellt; Reader-Writer-Match
    /// faellt zurueck auf reinen `type_name`-Vergleich (DDS 1.4 §2.2.3
    /// Default-Path)".
    ///
    /// idl-rust Codegen emittiert hier den passenden TypeIdentifier:
    /// - Primitive `int32` → `TypeIdentifier::Primitive(PrimitiveKind::Int32)`,
    /// - String `string<N>` → `TypeIdentifier::String8Small{ bound }`,
    /// - Composite struct → `TypeIdentifier::EquivalenceHash` (sobald
    ///   die TypeRegistry-Lookup live ist).
    ///
    /// Sobald beide Seiten (Writer + Reader) einen TypeIdentifier
    /// liefern, ruft der Subscriber-Match-Pfad
    /// [`zerodds_types::type_matcher::TypeMatcher::match_types`] auf
    /// (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier = zerodds_types::TypeIdentifier::None;

    /// Serialisiert `self` in den XCDR2-Payload, der in einer
    /// DATA-Submessage als `serialized_payload` gesendet wird.
    /// Default-Endianness: Little-Endian (RTPS 2.5 §10.5
    /// `RepresentationIdentifier = CDR2_LE = 0x0010`).
    ///
    /// # Errors
    /// CDR-Encoder-Fehler (Buffer-Overflow etc.).
    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError>;

    /// Big-Endian-Variante von [`Self::encode`]. Default-Implementation
    /// delegiert auf [`Self::encode`] (kein Byte-Swap), da generischer
    /// BE-Re-Encode ohne Type-Reflection nicht moeglich ist. Codegen
    /// ueberschreibt das fuer Strukturen, die echt BE auf die Wire
    /// gehen sollen. Spec: zerodds-xcdr2-rust §2.4.
    ///
    /// # Errors
    /// CDR-Encoder-Fehler.
    fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode(out)
    }

    /// Deserialisiert einen XCDR2-Payload. Der Caller stellt sicher,
    /// dass `bytes` den vollen Sample-Payload enthaelt.
    ///
    /// # Errors
    /// CDR-Decoder-Fehler (Truncation, unerwartete Bytes, etc.).
    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError>;

    /// Serialisiert die `@key`-Member-Werte im **PLAIN_CDR2-BE**-Format
    /// in den uebergebenen [`PlainCdr2BeKeyHolder`]. Reihenfolge: nach
    /// `member_id` aufsteigend (XTypes 1.3 §7.6.8.3.1.b).
    ///
    /// **Default-Implementation**: leerer Schreibvorgang. Keyed Types
    /// MUESSEN das ueberschreiben.
    ///
    /// Wird vom DcpsRuntime im Sample-Encode-Pfad aufgerufen, um
    /// PID_KEY_HASH in die Inline-QoS zu schreiben.
    fn encode_key_holder_be(&self, _holder: &mut PlainCdr2BeKeyHolder) {
        // Default: kein Key. Keyed Types ueberschreiben.
    }

    /// Liefert den Wert eines Feldpfads (dotted, z.B. `"a.b"`) als
    /// `zerodds_sql_filter::Value` fuer SQL-Filter-Evaluation in
    /// QueryCondition / ContentFilteredTopic. Default: `None` (kein
    /// Feld erreichbar — der Filter denied dann jedes Sample, sofern
    /// es einen Feldzugriff enthaelt).
    ///
    /// Spec: DDS 1.4 §B.2.1 (Filter Expressions) iVm. §2.2.2.5.9
    /// (QueryCondition) und §2.2.2.3.5 (ContentFilteredTopic).
    /// Generierte IDL-Stubs ueberschreiben das per Field.
    #[must_use]
    fn field_value(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
        None
    }

    /// Berechnet den 16-Byte KeyHash dieser Instanz nach XTypes 1.3
    /// §7.6.8.4. `None` wenn `HAS_KEY = false`.
    ///
    /// Default-Implementation nutzt [`Self::encode_key_holder_be`] +
    /// [`Self::KEY_HOLDER_MAX_SIZE`] und delegiert an
    /// [`compute_key_hash`].
    #[must_use]
    fn compute_key_hash(&self) -> Option<[u8; KEY_HASH_LEN]> {
        if !Self::HAS_KEY {
            return None;
        }
        let mut holder = PlainCdr2BeKeyHolder::new();
        self.encode_key_holder_be(&mut holder);
        let max = Self::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
        Some(compute_key_hash(holder.as_bytes(), max))
    }

    /// Spec-aligned Alias fuer [`Self::compute_key_hash`].
    /// `zerodds-xcdr2-rust` §2.5 nutzt den Namen `key_hash`; der
    /// Implementations-Name behaelt `compute_key_hash` aus
    /// historischer Kompat. Beide liefern denselben Wert.
    #[must_use]
    fn key_hash(&self) -> Option<[u8; KEY_HASH_LEN]> {
        self.compute_key_hash()
    }
}

/// `RowAccess`-Adapter fuer einen `DdsType`-Sample-Wert. Wird vom
/// DataReader in `read_w_condition`/`take_w_condition` und vom
/// `ContentFilteredTopic`-Filter benutzt.
pub struct DdsTypeRow<'a, T: DdsType> {
    /// Inneres Sample, dessen Felder per [`DdsType::field_value`]
    /// abgefragt werden.
    pub sample: &'a T,
}

impl<'a, T: DdsType> DdsTypeRow<'a, T> {
    /// Konstruktor.
    #[must_use]
    pub fn new(sample: &'a T) -> Self {
        Self { sample }
    }
}

impl<T: DdsType> zerodds_sql_filter::RowAccess for DdsTypeRow<'_, T> {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        self.sample.field_value(path)
    }
}

/// Platzhalter-Error fuer DdsType::encode. In v1.3 wird das auf
/// `zerodds_cdr::EncodeError` re-exported, sobald wir den CDR-Layer
/// in DCPS-Sicht stabilisiert haben.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// Buffer-Overflow oder feldspezifischer Wertebereichs-Fehler.
    Invalid {
        /// Statische Beschreibung.
        what: &'static str,
    },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid { what } => write!(f, "encode error: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

impl From<zerodds_cdr::EncodeError> for EncodeError {
    fn from(e: zerodds_cdr::EncodeError) -> Self {
        // zerodds-cdr-Fehler werden als opaker `Invalid`-Wrap weitergegeben.
        // Das ist ausreichend fuer DdsType-Caller, die nur „encoding hat
        // nicht geklappt"-Information brauchen — die detaillierte
        // Fehlerstruktur lebt im cdr-Layer und wird via Display
        // serialisiert wenn ein Caller die Fehlermeldung loggt.
        let _ = e;
        Self::Invalid {
            what: "zerodds_cdr encode error",
        }
    }
}

/// Platzhalter-Error fuer DdsType::decode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Truncation oder Wertebereich out-of-range.
    Invalid {
        /// Statische Beschreibung.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid { what } => write!(f, "decode error: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

impl From<zerodds_cdr::DecodeError> for DecodeError {
    fn from(e: zerodds_cdr::DecodeError) -> Self {
        let _ = e;
        Self::Invalid {
            what: "zerodds_cdr decode error",
        }
    }
}

// ---------------------------------------------------------------------
// DdsAny — IDL `any` Type-Erasure (XCDR2 §7.4.4.7)
//
// Wire-Format: TypeIdentifier-Header (CDR-String) + Payload-Bytes.
// Pure-Rust ohne externe Crate-Dep; Volle Type-Erasure via String-Tag.

/// IDL-`any` als Type-Erasure-Wrapper. Traegt einen Type-Identifier-
/// String (z.B. `"std_msgs::Header"`) plus die Payload-Bytes.
///
/// Konsumenten-Pattern: man prueft `type_name`, deserialisiert die
/// `payload` mit dem konkreten DdsType.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DdsAny {
    /// Voll-qualifizierter Type-Name (entspricht `DdsType::TYPE_NAME`).
    pub type_name: alloc::string::String,
    /// XCDR2-Payload-Bytes des wraped Werts.
    pub payload: Vec<u8>,
}

impl DdsAny {
    /// Konstruiert ein `DdsAny` aus einem `DdsType`-Wert.
    ///
    /// # Errors
    /// `EncodeError` bei Encode-Fehler.
    pub fn pack<T: DdsType>(value: &T) -> Result<Self, EncodeError> {
        let mut payload = Vec::new();
        value.encode(&mut payload)?;
        Ok(Self {
            type_name: alloc::string::String::from(T::TYPE_NAME),
            payload,
        })
    }

    /// Versucht das Wrapped als `T` zu entpacken.
    ///
    /// # Errors
    /// `DecodeError::Invalid` wenn `T::TYPE_NAME != self.type_name`
    /// oder Decode-Fehler.
    pub fn unpack<T: DdsType>(&self) -> Result<T, DecodeError> {
        if self.type_name != T::TYPE_NAME {
            return Err(DecodeError::Invalid {
                what: "DdsAny: type-name mismatch",
            });
        }
        T::decode(&self.payload)
    }
}

impl zerodds_cdr::CdrEncode for DdsAny {
    fn encode(
        &self,
        w: &mut zerodds_cdr::BufferWriter,
    ) -> core::result::Result<(), zerodds_cdr::EncodeError> {
        // Type-name als CDR-String + payload-bytes mit u32-length-prefix.
        w.write_string(&self.type_name)?;
        let payload_len = u32::try_from(self.payload.len()).map_err(|_| {
            zerodds_cdr::EncodeError::ValueOutOfRange {
                message: "DdsAny: payload > u32::MAX",
            }
        })?;
        w.write_u32(payload_len)?;
        w.write_bytes(&self.payload)?;
        Ok(())
    }
}

impl zerodds_cdr::CdrDecode for DdsAny {
    fn decode(
        r: &mut zerodds_cdr::BufferReader<'_>,
    ) -> core::result::Result<Self, zerodds_cdr::DecodeError> {
        let type_name = r.read_string()?;
        let payload_len = r.read_u32()? as usize;
        let payload = r.read_bytes(payload_len)?.to_vec();
        Ok(Self { type_name, payload })
    }
}

// ---------------------------------------------------------------------
// Built-in `DdsType` fuer &[u8]/Vec<u8>-Payloads
//
// Viele ROS-Use-Cases und Interop-Tests brauchen "roh durchreichen".
// Ein `BytesPayload`-Newtype mit festem Type-Name erlaubt das.
// ---------------------------------------------------------------------

/// Ein opaker Raw-Byte-Payload mit konfigurierbarem Type-Name (per
/// `impl` von `BytesPayload<T>` oder via newtype).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBytes {
    /// Payload-Bytes (werden 1:1 auf die Wire gelegt, kein CDR-Framing).
    pub data: Vec<u8>,
}

impl RawBytes {
    /// Konstruktor.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl DdsType for RawBytes {
    const TYPE_NAME: &'static str = "zerodds::RawBytes";

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            data: bytes.to_vec(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn raw_bytes_roundtrip() {
        let orig = RawBytes::new(vec![1, 2, 3, 4, 5]);
        let mut buf = Vec::new();
        orig.encode(&mut buf).unwrap();
        let back = RawBytes::decode(&buf).unwrap();
        assert_eq!(back, orig);
    }

    #[test]
    fn raw_bytes_type_name_is_namespaced() {
        assert_eq!(RawBytes::TYPE_NAME, "zerodds::RawBytes");
    }

    // ---- .B: keyed types + KeyHash ----

    /// Test-Fixture: keyed Topic mit @key u32 id (max 4 byte → zero-pad).
    struct SmallKeyed {
        id: u32,
    }

    impl DdsType for SmallKeyed {
        const TYPE_NAME: &'static str = "test::SmallKeyed";
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

        fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            out.extend_from_slice(&self.id.to_le_bytes());
            Ok(())
        }
        fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            if bytes.len() < 4 {
                return Err(DecodeError::Invalid {
                    what: "truncated SmallKeyed",
                });
            }
            let id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(Self { id })
        }
        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_u32(self.id);
        }
    }

    #[test]
    fn small_keyed_produces_zero_padded_keyhash() {
        let s = SmallKeyed { id: 0x1122_3344 };
        let key = s.compute_key_hash().expect("keyed");
        assert_eq!(&key[0..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&key[4..16], &[0u8; 12]);
    }

    #[test]
    fn non_keyed_returns_none_for_keyhash() {
        let r = RawBytes::new(vec![1, 2, 3]);
        assert_eq!(r.compute_key_hash(), None);
    }

    #[test]
    fn keyed_two_instances_have_distinct_hashes() {
        let a = SmallKeyed { id: 1 };
        let b = SmallKeyed { id: 2 };
        assert_ne!(a.compute_key_hash(), b.compute_key_hash());
    }

    /// Test-Fixture: keyed Topic mit unbounded @key string (MD5-Pfad).
    struct LargeKeyed {
        topic: alloc::string::String,
    }

    impl DdsType for LargeKeyed {
        const TYPE_NAME: &'static str = "test::LargeKeyed";
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = None; // unbounded → MD5

        fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            out.extend_from_slice(self.topic.as_bytes());
            Ok(())
        }
        fn decode(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            Err(DecodeError::Invalid {
                what: "test fixture",
            })
        }
        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_string(&self.topic);
        }
    }

    #[test]
    fn large_keyed_produces_md5_hashed_keyhash() {
        let s = LargeKeyed {
            topic: alloc::string::String::from("hello"),
        };
        let key = s.compute_key_hash().expect("keyed");
        // 16 byte deterministic hash, ungleich zero
        assert_ne!(key, [0u8; 16]);
        // Idempotent
        let key2 = s.compute_key_hash().expect("keyed");
        assert_eq!(key, key2);
    }

    #[test]
    fn spec_aligned_aliases_match_implementation_names() {
        // zerodds-xcdr2-rust §11 Errata.
        assert_eq!(
            <RawBytes as DdsType>::IS_KEYED,
            <RawBytes as DdsType>::HAS_KEY
        );
        fn is_keyed<T: DdsType>() -> bool {
            T::IS_KEYED
        }
        assert!(is_keyed::<SmallKeyed>());
        let s = SmallKeyed { id: 0xABCD };
        assert_eq!(s.key_hash(), s.compute_key_hash());
    }

    #[test]
    fn extensibility_default_is_final() {
        assert_eq!(<RawBytes as DdsType>::EXTENSIBILITY, Extensibility::Final);
        // ExtensibilityKind alias is the same type.
        let _: ExtensibilityKind = Extensibility::Mutable;
    }

    #[test]
    fn encode_be_default_delegates_to_encode() {
        let r = RawBytes::new(vec![1, 2, 3]);
        let mut le = Vec::new();
        let mut be = Vec::new();
        r.encode(&mut le).unwrap();
        r.encode_be(&mut be).unwrap();
        assert_eq!(le, be);
    }

    #[test]
    fn keyed_member_order_matters() {
        // Hypothetisch: zwei Members mit verschiedener Reihenfolge wuerden
        // unterschiedliche Hashes ergeben. Wir verifizieren das mit einem
        // Mock-Type, der zwei Felder in Reverse-Order schreibt.
        struct A {
            x: u32,
            y: u32,
        }
        impl DdsType for A {
            const TYPE_NAME: &'static str = "test::A";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(8);
            fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> Result<Self, DecodeError> {
                Err(DecodeError::Invalid { what: "stub" })
            }
            fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
                holder.write_u32(self.x);
                holder.write_u32(self.y);
            }
        }
        struct B {
            x: u32,
            y: u32,
        }
        impl DdsType for B {
            const TYPE_NAME: &'static str = "test::B";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(8);
            fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> Result<Self, DecodeError> {
                Err(DecodeError::Invalid { what: "stub" })
            }
            fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
                holder.write_u32(self.y);
                holder.write_u32(self.x);
            }
        }
        let a = A { x: 1, y: 2 };
        let b = B { x: 1, y: 2 };
        assert_ne!(a.compute_key_hash(), b.compute_key_hash());
    }
}
