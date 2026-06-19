// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connection preface — RFC 9113 §3.4.
//!
//! Spec §3.5: the client preface is 24 bytes:
//! `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`.

use crate::error::Http2Error;

/// HTTP/2 client connection preface (RFC 9113 §3.4).
pub const CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Checks whether the first 24 bytes of the input match the
/// spec preface.
///
/// # Errors
/// `BadPreface` if the prefix does not match; `ShortFrameHeader` if
/// fewer than 24 bytes are available.
pub fn check_preface(input: &[u8]) -> Result<(), Http2Error> {
    if input.len() < CLIENT_PREFACE.len() {
        return Err(Http2Error::ShortFrameHeader);
    }
    if &input[..CLIENT_PREFACE.len()] != CLIENT_PREFACE {
        return Err(Http2Error::BadPreface);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn correct_preface_passes() {
        check_preface(CLIENT_PREFACE).unwrap();
    }

    #[test]
    fn preface_with_trailing_data_passes() {
        let mut buf = alloc::vec::Vec::new();
        buf.extend_from_slice(CLIENT_PREFACE);
        buf.extend_from_slice(&[0; 100]);
        check_preface(&buf).unwrap();
    }

    #[test]
    fn short_input_rejected() {
        assert_eq!(check_preface(b"PRI"), Err(Http2Error::ShortFrameHeader));
    }

    #[test]
    fn wrong_preface_rejected() {
        let mut bad = CLIENT_PREFACE.to_vec();
        bad[0] = b'X';
        assert_eq!(check_preface(&bad), Err(Http2Error::BadPreface));
    }

    #[test]
    fn preface_length_is_24_bytes() {
        assert_eq!(CLIENT_PREFACE.len(), 24);
    }
}
