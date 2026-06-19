// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 wire PSM (`formal/2019-09-01` §6) — XML as the
//! wire format for DDS topic data.
//!
//! Crate `zerodds-xml-wire`. Safety classification: **STANDARD**.
//!
//! Spec §6 specifies XML as an alternative wire representation for
//! topic samples (parallel to CDR). We provide:
//!
//! * [`codec`] — bidirectional XML↔CDR codec at the type-token level.
//! * [`xsd`] — XSD schema generation from IDL type definitions.
//! * [`parser`] — streaming XML parser (no DOM buffering).
//! * [`emitter`] — streaming XML emitter with XML 1.0 conformance.
//! * [`validator`] — schema validation against XSD.

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
