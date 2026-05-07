// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bidirectional-GIOP — Spec §15.9.
//!
//! Bidirectional-GIOP erlaubt einem Server, Requests an den Client
//! ueber dieselbe TCP-Connection zurueckzuschicken. Aushandlung
//! geschieht via `BiDirIIOPServiceContext` mit Tag `BI_DIR_IIOP = 5`,
//! transportiert in der ersten Request-Message des Clients.
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
//! Der Server speichert die Listen-Points und nutzt sie, wenn er
//! spaeter selbst ein Object referenziert, das auf dem Client lebt.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::profile_body::CdrError;

/// IOP-Service-Context-Tag fuer Bidirectional-GIOP (Spec §15.9 +
/// §13.7).
pub const IIOP_BI_DIR_TAG: u32 = 5;

/// Ein einzelner Listen-Point — Host + Port.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BiDirIiopListenPoint {
    /// Host-Name oder IP-Adresse.
    pub host: String,
    /// TCP-Port.
    pub port: u16,
}

/// Gesamter `BiDirIIOPServiceContext`-Inhalt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BiDirIiopServiceContext {
    /// Liste der Listen-Points.
    pub listen_points: Vec<BiDirIiopListenPoint>,
}

impl BiDirIiopServiceContext {
    /// CDR-Encode in einen `BufferWriter`.
    ///
    /// # Errors
    /// Buffer-Schreibfehler oder Length-Overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        let n = u32::try_from(self.listen_points.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        for lp in &self.listen_points {
            w.write_string(&lp.host)?;
            w.write_u16(lp.port)?;
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
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
        // Spec §15.9 + §13.7: Tag-Wert = 5.
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
}
