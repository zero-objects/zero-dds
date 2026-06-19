// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bidirectional GIOP — Spec §15.9.
//!
//! Bidirectional GIOP allows a server to send requests back to the
//! client over the same TCP connection. Negotiation happens via
//! `BiDirIIOPServiceContext` with tag `BI_DIR_IIOP = 5`, carried in
//! the client's first request message.
//!
//! ```text
//! struct BiDirIIOPServiceContext {
//!     sequence<BiDirIIOPListenPoint> listen_points;
//! };
//!
//! struct BiDirIIOPListenPoint {
//!     string         host;
//!     unsigned short port;
//! };
//! ```
//!
//! The server stores the listen points and uses them when it later
//! references an object that lives on the client.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::profile_body::CdrError;

/// IOP service-context tag for bidirectional GIOP (Spec §15.9 +
/// §13.7).
pub const IIOP_BI_DIR_TAG: u32 = 5;

/// A single listen point — host + port.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BiDirIiopListenPoint {
    /// Host name or IP address.
    pub host: String,
    /// TCP port.
    pub port: u16,
}

/// The full `BiDirIIOPServiceContext` content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BiDirIiopServiceContext {
    /// List of listen points.
    pub listen_points: Vec<BiDirIiopListenPoint>,
}

impl BiDirIiopServiceContext {
    /// CDR-encodes into a `BufferWriter`.
    ///
    /// # Errors
    /// Buffer write error or length overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        let n = u32::try_from(self.listen_points.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        for lp in &self.listen_points {
            w.write_string(&lp.host)?;
            w.write_u16(lp.port)?;
        }
        Ok(())
    }

    /// CDR-decodes.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, CdrError> {
        let n = r.read_u32()? as usize;
        let mut listen_points = Vec::with_capacity(n.min(32));
        for _ in 0..n {
            let host = r.read_string()?;
            let port = r.read_u16()?;
            listen_points.push(BiDirIiopListenPoint { host, port });
        }
        Ok(Self { listen_points })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn bidir_tag_value_matches_spec() {
        // Spec §15.9 + §13.7: tag value = 5.
        assert_eq!(IIOP_BI_DIR_TAG, 5);
    }

    #[test]
    fn empty_context_round_trip() {
        let c = BiDirIiopServiceContext::default();
        let mut w = BufferWriter::new(Endianness::Big);
        c.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(BiDirIiopServiceContext::decode(&mut r).unwrap(), c);
    }

    #[test]
    fn multi_listen_point_round_trip() {
        let c = BiDirIiopServiceContext {
            listen_points: alloc::vec![
                BiDirIiopListenPoint {
                    host: "client-a.lab".into(),
                    port: 8080,
                },
                BiDirIiopListenPoint {
                    host: "10.0.0.42".into(),
                    port: 7000,
                },
            ],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        c.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = BiDirIiopServiceContext::decode(&mut r).unwrap();
        assert_eq!(decoded, c);
    }

    #[test]
    fn bidir_sc_byte_identical_to_omniorb() {
        // Cross-ORB conformance (§15.8): byte-identical to omniORB 4.3.3, which
        // marshals the same BiDirIIOPServiceContext struct via cdrEncapsulationStream
        // (capture on the Linux test host, clearMemory=1, little-endian).
        // listen_points = [{ "client.local", 5555 }].
        let c = BiDirIiopServiceContext {
            listen_points: alloc::vec![BiDirIiopListenPoint {
                host: "client.local".into(),
                port: 5555,
            }],
        };
        // ServiceContext encapsulation = byte-order octet (1=LE) + struct body.
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(1).unwrap();
        c.encode(&mut w).unwrap();
        let hex: alloc::string::String = w
            .into_bytes()
            .iter()
            .map(|b| alloc::format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "01000000010000000d000000636c69656e742e6c6f63616c0000b315"
        );
    }
}
