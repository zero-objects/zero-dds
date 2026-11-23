// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message compression of `zerodds-grpc-bridge-1.0` §4.4: a
//! Length-Prefixed-Message with the Compressed-Flag 1 is compressed with
//! the `grpc-encoding` of its call (gRPC over HTTP/2, "Message-Encoding";
//! gRPC Compression).
//!
//! `identity`, `gzip` (RFC 1952) and `deflate` (the zlib format of RFC
//! 1950, as HTTP's `deflate` content coding, RFC 9110 §8.4.1.2, and
//! gRPC core's `deflate`). Inflating stops at a bound, so a small message
//! cannot expand beyond `max_recv_message_size`. A gzip message is the
//! series of its members (RFC 1952 §2.2), inflated one after the other; an
//! octet after the last member or after the zlib stream is corrupt, never
//! dropped.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::bufread::{MultiGzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};

/// The encodings of the daemon, as `grpc-accept-encoding` names them.
pub const ACCEPT_ENCODING: &str = "identity,deflate,gzip";

/// A message encoding the daemon speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// No compression.
    Identity,
    /// gzip, RFC 1952.
    Gzip,
    /// zlib, RFC 1950.
    Deflate,
}

impl Encoding {
    /// The encoding a `grpc-encoding` value names; `None` for one the
    /// daemon does not speak. Absent is `identity`.
    #[must_use]
    pub fn of(name: Option<&str>) -> Option<Self> {
        match name.map(str::trim) {
            None | Some("identity") => Some(Self::Identity),
            Some("gzip") => Some(Self::Gzip),
            Some("deflate") => Some(Self::Deflate),
            Some(_) => None,
        }
    }

    /// The name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
        }
    }
}

/// Why a compressed message was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionError {
    /// The inflated message would exceed the bound (spec §3
    /// `max_recv_message_size`: `RESOURCE_EXHAUSTED`).
    TooLarge {
        /// The bound in octets.
        limit: usize,
    },
    /// The octets are not a stream of the encoding.
    Corrupt(String),
}

impl core::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLarge { limit } => {
                write!(f, "the inflated message exceeds {limit} octets")
            }
            Self::Corrupt(e) => write!(f, "the compressed message does not inflate: {e}"),
        }
    }
}

impl std::error::Error for CompressionError {}

/// The message of `bytes` compressed with `encoding`, inflated to at most
/// `limit` octets.
///
/// # Errors
/// [`CompressionError::TooLarge`] above `limit`,
/// [`CompressionError::Corrupt`] for octets that do not inflate.
pub fn decompress(
    encoding: Encoding,
    bytes: &[u8],
    limit: usize,
) -> Result<Vec<u8>, CompressionError> {
    match encoding {
        Encoding::Identity => {
            if bytes.len() > limit {
                return Err(CompressionError::TooLarge { limit });
            }
            Ok(bytes.to_vec())
        }
        // RFC 1952 §2.2: a gzip file is a series of members; the decoder
        // reads the next member's header after each member and fails on
        // octets that do not begin one.
        Encoding::Gzip => {
            let mut d = MultiGzDecoder::new(bytes);
            let out = inflate(&mut d, limit)?;
            nothing_after(d.get_ref(), "gzip member")?;
            Ok(out)
        }
        Encoding::Deflate => {
            let mut d = ZlibDecoder::new(bytes);
            let out = inflate(&mut d, limit)?;
            nothing_after(d.get_ref(), "zlib stream")?;
            Ok(out)
        }
    }
}

/// Reads `decoder` to the end, at most `limit` octets.
fn inflate(decoder: impl Read, limit: usize) -> Result<Vec<u8>, CompressionError> {
    let mut out = Vec::new();
    let bound = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    decoder
        .take(bound)
        .read_to_end(&mut out)
        .map_err(|e| CompressionError::Corrupt(e.to_string()))?;
    if out.len() > limit {
        return Err(CompressionError::TooLarge { limit });
    }
    Ok(out)
}

/// The input a decoder left after the end of `what`: corrupt unless empty.
fn nothing_after(rest: &[u8], what: &str) -> Result<(), CompressionError> {
    if rest.is_empty() {
        return Ok(());
    }
    Err(CompressionError::Corrupt(format!(
        "{} octets after the end of the {what}",
        rest.len()
    )))
}

/// `message` compressed with `encoding`.
///
/// # Errors
/// An I/O error of the encoder (writing into memory does not fail).
pub fn compress(encoding: Encoding, message: &[u8]) -> std::io::Result<Vec<u8>> {
    match encoding {
        Encoding::Identity => Ok(message.to_vec()),
        Encoding::Gzip => {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write_all(message)?;
            e.finish()
        }
        Encoding::Deflate => {
            let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
            e.write_all(message)?;
            e.finish()
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// §4.4: gzip and deflate round trip; gzip starts with the RFC 1952
    /// magic, deflate with a zlib header (RFC 1950 §2.2: CMF 0x78, the
    /// header a multiple of 31).
    #[test]
    fn grpcbridge_4_4_gzip_and_deflate_round_trip() {
        let msg: Vec<u8> = (0..10_000u32).map(|i| (i % 7) as u8).collect();
        let gz = compress(Encoding::Gzip, &msg).unwrap();
        assert_eq!(&gz[..2], &[0x1f, 0x8b]);
        assert!(gz.len() < msg.len());
        assert_eq!(decompress(Encoding::Gzip, &gz, msg.len()).unwrap(), msg);
        let z = compress(Encoding::Deflate, &msg).unwrap();
        assert_eq!(z[0], 0x78);
        assert_eq!(u16::from_be_bytes([z[0], z[1]]) % 31, 0);
        assert_eq!(decompress(Encoding::Deflate, &z, msg.len()).unwrap(), msg);
        assert_eq!(compress(Encoding::Identity, &msg).unwrap(), msg);
        // A gzip stream is not a zlib stream.
        assert!(matches!(
            decompress(Encoding::Deflate, &gz, msg.len()),
            Err(CompressionError::Corrupt(_))
        ));
    }

    /// §3 `max_recv_message_size` on the inflated size: one octet above the
    /// bound is refused, the bound itself passes.
    #[test]
    fn grpcbridge_4_4_an_inflated_size_above_the_limit_is_refused() {
        let msg = vec![0u8; 100_000];
        let gz = compress(Encoding::Gzip, &msg).unwrap();
        assert!(gz.len() < 1_000);
        assert_eq!(
            decompress(Encoding::Gzip, &gz, 99_999),
            Err(CompressionError::TooLarge { limit: 99_999 })
        );
        assert_eq!(
            decompress(Encoding::Gzip, &gz, 100_000).unwrap().len(),
            100_000
        );
        assert!(decompress(Encoding::Identity, &msg, 99_999).is_err());
    }

    /// §4.4, RFC 1952 §2.2: a gzip message of two members inflates to both,
    /// the bound counting both; octets after the last member, a member cut
    /// short and octets after the zlib stream are corrupt.
    #[test]
    fn grpcbridge_4_4_a_gzip_message_of_two_members_is_read_whole() {
        let a = vec![1u8; 600];
        let b: Vec<u8> = (0..600u32).map(|i| (i % 251) as u8).collect();
        let mut two = compress(Encoding::Gzip, &a).unwrap();
        let second = compress(Encoding::Gzip, &b).unwrap();
        two.extend_from_slice(&second);
        let whole = decompress(Encoding::Gzip, &two, 1_200).unwrap();
        assert_eq!(whole.len(), 1_200);
        assert_eq!(whole[..600], a[..]);
        assert_eq!(whole[600..], b[..]);
        assert_eq!(
            decompress(Encoding::Gzip, &two, 1_199),
            Err(CompressionError::TooLarge { limit: 1_199 })
        );
        // Octets after the last member that begin no member.
        let mut tail = compress(Encoding::Gzip, &a).unwrap();
        tail.extend_from_slice(b"junk");
        assert!(matches!(
            decompress(Encoding::Gzip, &tail, 1_200),
            Err(CompressionError::Corrupt(_))
        ));
        // A second member cut short.
        let mut cut = compress(Encoding::Gzip, &a).unwrap();
        cut.extend_from_slice(&second[..second.len() / 2]);
        assert!(matches!(
            decompress(Encoding::Gzip, &cut, 1_200),
            Err(CompressionError::Corrupt(_))
        ));
        // Octets after the zlib stream.
        let mut z = compress(Encoding::Deflate, &a).unwrap();
        z.extend_from_slice(&[0, 0, 0]);
        assert_eq!(
            decompress(Encoding::Deflate, &z, 1_200),
            Err(CompressionError::Corrupt(
                "3 octets after the end of the zlib stream".into()
            ))
        );
    }

    /// The `grpc-encoding` names: absent is identity, an unknown one is
    /// `None`.
    #[test]
    fn grpcbridge_4_4_an_unknown_encoding_is_named() {
        assert_eq!(Encoding::of(None), Some(Encoding::Identity));
        assert_eq!(Encoding::of(Some("gzip")), Some(Encoding::Gzip));
        assert_eq!(Encoding::of(Some("deflate")), Some(Encoding::Deflate));
        assert_eq!(Encoding::of(Some("snappy")), None);
        assert_eq!(Encoding::Gzip.name(), "gzip");
        assert!(
            ACCEPT_ENCODING
                .split(',')
                .all(|e| Encoding::of(Some(e)).is_some())
        );
    }
}
