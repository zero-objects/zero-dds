// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS SOAP-PSM — SOAP 1.2 + WSDL 1.1/2.0 + MTOM + WS-Addressing +
//! WS-Security 1.1.
//!
//! Crate `zerodds-soap`. Safety classification: **STANDARD**.
//!
//! Liefert einen produktionsreifen SOAP-Stack zur Anbindung
//! Java-2000er-Bestand-Clients und JEE-Web-Services an DDS-Topics.
//!
//! # Module
//!
//! * [`envelope`] — SOAP 1.2 Envelope/Header/Body (W3C SOAP 1.2 §5).
//! * [`fault`] — SOAP-Fault-Codes (`Sender`/`Receiver`/`MustUnderstand`/
//!   `VersionMismatch`/`DataEncodingUnknown`).
//! * [`wsdl`] — WSDL 1.1 (`http://schemas.xmlsoap.org/wsdl/`) + WSDL
//!   2.0 (`http://www.w3.org/ns/wsdl`)-Generation aus IDL-Service-Defs.
//! * [`mtom`] — MTOM/XOP-Attachments (W3C MTOM Rec.).
//! * [`addressing`] — WS-Addressing 1.0 (W3C 2006).
//! * [`security`] — WS-Security 1.1 (OASIS WSS-1.1) — UsernameToken,
//!   X.509-Token, Timestamp, BinarySecurityToken.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod addressing;
pub mod envelope;
pub mod fault;
pub mod mtom;
pub mod security;
pub mod wsdl;

pub use addressing::{AddressingHeaders, build_addressing_header};
pub use envelope::{Envelope, EnvelopeError, build_envelope, parse_envelope};
pub use fault::{Fault, FaultCode};
pub use mtom::{Attachment, MtomMessage, encode_mtom_packaging};
pub use security::{SecurityHeader, Timestamp, UsernameToken, X509Token};
pub use wsdl::{Operation, WsdlGenerator, WsdlVersion};
