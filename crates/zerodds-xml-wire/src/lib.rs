// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 Wire-PSM (`formal/2019-09-01` §6) — XML als
//! Wire-Format fuer DDS-Topic-Daten.
//!
//! Crate `zerodds-xml-wire`. Safety classification: **STANDARD**.
//!
//! Spec §6 spezifiziert XML als alternative Wire-Repraesentation fuer
//! Topic-Samples (parallel zu CDR). Wir liefern:
//!
//! * [`codec`] — Bidirektionaler XML↔CDR-Codec auf Type-Token-Ebene.
//! * [`xsd`] — XSD-Schema-Generation aus IDL-Type-Definitionen.
//! * [`parser`] — Streaming-XML-Parser (kein DOM-Buffering).
//! * [`emitter`] — Streaming-XML-Emitter mit XML 1.0 Conformance.
//! * [`validator`] — Schema-Validation gegen XSD.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod codec;
pub mod emitter;
pub mod parser;
pub mod validator;
pub mod xsd;

pub use codec::{CodecError, FieldKind, FieldValue, decode_xml, encode_to_xml};
pub use emitter::{EmitError, XmlEmitter};
pub use parser::{Event, ParseError, XmlParser};
pub use validator::{ValidationError, validate};
pub use xsd::{XsdGenerator, XsdType};
