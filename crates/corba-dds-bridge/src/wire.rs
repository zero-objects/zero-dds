// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire helpers — connect the bridge to the concrete CORBA wire crates
//! (`zerodds-corba-giop` + `zerodds-corba-ior`).
//!
//! The bridge itself lives logically above the wire crates and maps
//! operation bytes onto DDS topics, but hosting needs the helpers to:
//!
//! 1. **inspect GIOP requests** (extract the operation name + body
//!    without the full servant loop), and
//! 2. **pull the `object_key` out of an IOR** — for the lookup of
//!    which `BridgeRoute` serves an incoming request.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_giop::{Message, decode_message};
use zerodds_corba_ior::{Ior, ProfileId};

/// Summary of a GIOP request, as the bridge servant needs it for the
/// DDS mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSummary {
    /// `request_id` from the GIOP header (i.e. the `RequestHeader`).
    pub request_id: u32,
    /// Operation name (fully qualified).
    pub operation: String,
    /// Body bytes (what the bridge publishes to the topic).
    pub body: Vec<u8>,
}

/// Decodes a GIOP frame (header + body) and extracts
/// `request_id`/`operation`/`body` if the message is a `Request`
/// type. Other message types are not wrapped into a
/// `RequestSummary`.
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

/// Extracts the `object_key` from the first `TAG_INTERNET_IOP`
/// profile of an IOR. The return value is the `Vec<u8>` form needed
/// for bridge-route matching. Returns `None` if the IOR contains no
/// IIOP profile or the decapsulation fails.
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
        // 12-byte header of a known-invalid frame.
        let bogus = [0u8; 12];
        assert!(decode_giop_request_bytes(&bogus).is_none());
    }

    #[test]
    fn object_key_from_empty_ior_is_none() {
        let ior = Ior::default();
        assert_eq!(object_key_from_ior(&ior), None);
    }
}
