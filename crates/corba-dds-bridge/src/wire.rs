// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire-Helpers — bringen die Bridge an konkrete CORBA-Wire-Crates
//! (`zerodds-corba-giop` + `zerodds-corba-ior`) heran.
//!
//! Die Bridge selbst lebt logisch oberhalb der Wire-Crates und mappt
//! Operation-Bytes auf DDS-Topics, aber Hosting braucht die Helpers
//! um:
//!
//! 1. **GIOP-Requests zu inspizieren** (Operation-Name + Body
//!    extrahieren ohne den ganzen Servant-Loop), und
//! 2. **`object_key` aus einer IOR zu ziehen** — fuer den Lookup,
//!    welche `BridgeRoute` einen eingehenden Request bedient.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_giop::{Message, decode_message};
use zerodds_corba_ior::{Ior, ProfileId};

/// Zusammenfassung einer GIOP-Request, wie sie der Bridge-Servant
/// fuer das DDS-Mapping benoetigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSummary {
    /// `request_id` aus dem GIOP-Header (bzw. der `RequestHeader`).
    pub request_id: u32,
    /// Operation-Name (vollqualifiziert).
    pub operation: String,
    /// Body-Bytes (das, was die Bridge auf das Topic publiziert).
    pub body: Vec<u8>,
}

/// Decodiert ein GIOP-Frame (Header + Body) und extrahiert
/// `request_id`/`operation`/`body` falls die Message ein
/// `Request`-Type ist. Andere Message-Types werden nicht in eine
/// `RequestSummary` gekapselt.
#[must_use]
pub fn decode_giop_request_bytes(frame: &[u8]) -> Option<RequestSummary> {
    let (msg, body) = decode_message(frame).ok()?;
    match msg {
        Message::Request(r) => Some(RequestSummary {
            request_id: r.request_id,
            operation: r.operation,
            body: body.to_vec(),
        }),
        _ => None,
    }
}

/// Extrahiert das `object_key` aus dem ersten `TAG_INTERNET_IOP`-
/// Profile einer IOR. Rueckgabe ist die fuer Bridge-Routes-Match
/// benoetigte `Vec<u8>`-Form. Liefert `None` falls die IOR kein
/// IIOP-Profile enthaelt oder die Decapsulation fehlschlaegt.
#[must_use]
pub fn object_key_from_ior(ior: &Ior) -> Option<Vec<u8>> {
    for prof in &ior.profiles {
        if prof.tag == ProfileId::InternetIop {
            if let Some(Ok(body)) = prof.as_iiop() {
                return Some(body.object_key);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_giop_request_bytes_rejects_non_request_frame() {
        // 12-Byte-Header eines bekannt-ungueltigen Frames.
        let bogus = [0u8; 12];
        assert!(decode_giop_request_bytes(&bogus).is_none());
    }

    #[test]
    fn object_key_from_empty_ior_is_none() {
        let ior = Ior::default();
        assert_eq!(object_key_from_ior(&ior), None);
    }
}
