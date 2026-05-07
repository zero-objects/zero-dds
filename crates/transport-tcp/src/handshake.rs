// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ZeroDDS-TCP-Transport-Handshake.
//!
//! # Spec-Status
//!
//! **Es gibt keinen normativen OMG-Spec für einen TCP-PSM-Handshake.**
//! DDSI-RTPS 2.5 §9.4 normiert nur den Locator-Kind (`TCPv4`=4,
//! `TCPv6`=8) und §9.5 das Wire-Bytes-Mapping (RTPS-Header +
//! Submessages, identisch zum UDP-PSM). Vendor-spezifisch sind:
//! Cyclone's `ddsi_tcp` (kein Handshake, raw RTPS-Frames mit Length-
//! Prefix), FastDDS-TCPv4-Transport (eigene BindConnection-Submessages
//! 0x71/0x72), RTI-DDS-TCP (TLS-orientiert).
//!
//! Dieser Handshake ist Teil der **ZeroDDS-TCP-Transport-1.0**-Spec
//! (`docs/spec-coverage/zerodds-tcp-transport-1.0.md`) und ist
//! ZeroDDS-vendor-spezifisch.
//!
//! # Scope
//!
//! Beim Verbindungsaufbau tauschen Client und Server **vor** jedem
//! RTPS-Frame einen festen 16-Byte `BindConnection`-Request +
//! -Response aus. Der Handshake:
//!
//! 1. Verifiziert, dass der Peer ueberhaupt ein ZeroDDS-TCP-Transport
//!    ist und kein HTTP/TLS/Nonsense — Magic-Prefix klappert sofort.
//! 2. Alignt Protocol-Version, Vendor-Id, und einen
//!    Logical-Port-Claim.
//! 3. Erlaubt dem Server, die Verbindung mit einem Reason-Code
//!    abzulehnen (Version-mismatch, Port-Konflikt, Resource-Limit).
//!
//! # Cross-Vendor-Interop
//!
//! Cross-Vendor-TCP-Interop (FastDDS, RTI) erfordert vendor-spezifische
//! Compatibility-Modes. Cyclone-`ddsi_tcp`-Compat ist bereits über den
//! "raw RTPS-Frames"-Pfad abgedeckt (Handshake skippbar via
//! `TcpTransport::without_handshake`). FastDDS-/RTI-Compat sind
//! optionale Feature-Flags-Erweiterungspunkte; siehe
//! `zerodds-tcp-transport-1.0.md §6` für Wire-Format-Details.
//!
//! # Wire-Layout
//!
//! BindConnectionRequest (16 Byte, big-endian):
//!
//! ```text
//!   +---------+---------+---------+---------+
//!   |  'Z'   |  'D'    |  'D'    |  'S'    |   magic "ZDDS"
//!   +---------+---------+---------+---------+
//!   | v_major | v_minor |  vendor_id (2 B)  |
//!   +---------+---------+---------+---------+
//!   |               flags (u32)             |   reserved, = 0
//!   +---------+---------+---------+---------+
//!   |            logical_port (u32)         |
//!   +---------+---------+---------+---------+
//! ```
//!
//! BindConnectionResponse (16 Byte, big-endian):
//!
//! ```text
//!   +---------+---------+---------+---------+
//!   |  'Z'   |  'D'    |  'A'    |  status |   magic "ZDA" + status
//!   +---------+---------+---------+---------+
//!   | v_major | v_minor |  vendor_id (2 B)  |   server's version
//!   +---------+---------+---------+---------+
//!   |               flags (u32)             |   reserved, = 0
//!   +---------+---------+---------+---------+
//!   |            reason_code (u32)          |   0 bei Ok, sonst nach RejectReason
//!   +---------+---------+---------+---------+
//! ```
//!
//! Status im 4. Magic-Byte:
//! - `b'+'` (0x2B) = Accept
//! - `b'-'` (0x2D) = Reject
//!
//! # Fehlerpfade
//!
//! - Falscher Magic-Prefix → `HandshakeError::BadMagic`. Sender ist
//!   kein ZeroDDS-TCP — Connection dropen.
//! - Version-Mismatch → Server antwortet mit Reject/`VersionMismatch`.
//! - Reject aus irgend einem Grund → Client dropt die Connection und
//!   signalisiert dem Pool einen `backoff`, damit kein tight-loop.

use std::io::{Read, Write};

/// Handshake-Magic `ZDDS`.
pub const HANDSHAKE_MAGIC_REQUEST: [u8; 4] = *b"ZDDS";
/// Handshake-Response-Magic-Prefix `ZDA` + Accept-Byte.
pub const HANDSHAKE_MAGIC_ACCEPT: [u8; 4] = *b"ZDA+";
/// Handshake-Response-Magic-Prefix `ZDA` + Reject-Byte.
pub const HANDSHAKE_MAGIC_REJECT: [u8; 4] = *b"ZDA-";

/// Protocol-Version-Major, die dieser Transport fuehrt.
pub const TCP_PSM_VERSION_MAJOR: u8 = 1;
/// Protocol-Version-Minor.
pub const TCP_PSM_VERSION_MINOR: u8 = 0;

/// Maximaler Diff zwischen Peer- und lokaler Version, den wir noch
/// akzeptieren. `(0,0)` = exakte Matches only.
pub const ACCEPTED_VERSION_DIFF: (u8, u8) = (0, 0);

/// Fixed wire size beider Handshake-Frames.
pub const HANDSHAKE_WIRE_SIZE: usize = 16;

/// ZeroDDS-Vendor-Id (matches `zerodds_rtps::wire_types::VendorId::ZERODDS`
/// [0x01, 0x0F]). Hardcodiert, damit diese Crate nicht auf zerodds-rtps
/// verweisen muss nur fuer die Konstante.
pub const VENDOR_ID_ZERODDS: [u8; 2] = [0x01, 0x0F];

/// Bind-Connection-Request (§5.2.1.3, hier Fixed-Layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindConnectionRequest {
    /// Protocol-Major-Version des Senders.
    pub version_major: u8,
    /// Protocol-Minor-Version.
    pub version_minor: u8,
    /// Vendor-Id des Senders.
    pub vendor_id: [u8; 2],
    /// Reserved, muss 0 sein.
    pub flags: u32,
    /// DDS-Endpoint-Logical-Port, den dieser Sender auf dieser
    /// Connection befragen will. 0 = "keine Angabe, nutze default".
    pub logical_port: u32,
}

/// Bind-Connection-Response Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseStatus {
    /// Handshake akzeptiert.
    Accept,
    /// Handshake abgelehnt — `reason_code` traegt Begruendung.
    Reject(RejectReason),
}

/// Reject-Reasons. Numerische Werte sind stable wire-Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum RejectReason {
    /// Unbekannt / ueberreservierter Code.
    Unknown = 0,
    /// Peer-Version zu neu/alt.
    VersionMismatch = 1,
    /// Server hat Resource-Limit erreicht (MAX_PEERS).
    ResourceLimit = 2,
    /// Logical-Port-Konflikt (Port bereits gebunden).
    LogicalPortConflict = 3,
    /// Vendor-ID nicht akzeptiert (Policy).
    VendorNotAccepted = 4,
}

impl RejectReason {
    /// Decodes a wire code to a known `RejectReason`, returning
    /// `Unknown` for out-of-range values so the client can still
    /// proceed with a generic reject.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::VersionMismatch,
            2 => Self::ResourceLimit,
            3 => Self::LogicalPortConflict,
            4 => Self::VendorNotAccepted,
            _ => Self::Unknown,
        }
    }

    /// Wire code (u32).
    #[must_use]
    pub fn as_code(self) -> u32 {
        self as u32
    }
}

/// Bind-Connection-Response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindConnectionResponse {
    /// Accept/Reject.
    pub status: ResponseStatus,
    /// Version des Servers (Peer echoed bei Accept; bei Reject
    /// informativ, damit Client die Version kennt).
    pub version_major: u8,
    /// Minor-Version.
    pub version_minor: u8,
    /// Vendor-Id des Servers.
    pub vendor_id: [u8; 2],
    /// Reserved, muss 0 sein.
    pub flags: u32,
}

/// Fehler waehrend des Handshakes.
#[derive(Debug)]
#[non_exhaustive]
pub enum HandshakeError {
    /// I/O-Fehler beim Schreiben/Lesen.
    Io(std::io::Error),
    /// Peer sendete einen Frame ohne das erwartete Magic-Prefix.
    BadMagic {
        /// Tatsaechlich erhaltenes 4-Byte-Prefix (fuer Diagnostik).
        got: [u8; 4],
    },
    /// Peer-Version ausserhalb des akzeptierten Fensters.
    VersionMismatch {
        /// Peer-Version (major, minor).
        peer: (u8, u8),
        /// Unsere Version.
        local: (u8, u8),
    },
    /// Server sagte Reject.
    Rejected {
        /// Grund.
        reason: RejectReason,
    },
    /// Response-Magic weder Accept noch Reject.
    BadResponse {
        /// 4-Byte-Prefix.
        got: [u8; 4],
    },
}

impl core::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "handshake i/o error: {e}"),
            Self::BadMagic { got } => {
                write!(f, "handshake bad magic: {got:?}")
            }
            Self::VersionMismatch { peer, local } => {
                write!(
                    f,
                    "handshake version mismatch: peer {peer:?}, local {local:?}"
                )
            }
            Self::Rejected { reason } => write!(f, "handshake rejected: {reason:?}"),
            Self::BadResponse { got } => {
                write!(f, "handshake unexpected response magic: {got:?}")
            }
        }
    }
}

impl std::error::Error for HandshakeError {}

impl From<std::io::Error> for HandshakeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------
// Encoders/Decoders
// ---------------------------------------------------------------------

fn encode_request(req: &BindConnectionRequest) -> [u8; HANDSHAKE_WIRE_SIZE] {
    let mut buf = [0u8; HANDSHAKE_WIRE_SIZE];
    buf[..4].copy_from_slice(&HANDSHAKE_MAGIC_REQUEST);
    buf[4] = req.version_major;
    buf[5] = req.version_minor;
    buf[6..8].copy_from_slice(&req.vendor_id);
    buf[8..12].copy_from_slice(&req.flags.to_be_bytes());
    buf[12..16].copy_from_slice(&req.logical_port.to_be_bytes());
    buf
}

fn decode_request(
    buf: &[u8; HANDSHAKE_WIRE_SIZE],
) -> Result<BindConnectionRequest, HandshakeError> {
    let magic = [buf[0], buf[1], buf[2], buf[3]];
    if magic != HANDSHAKE_MAGIC_REQUEST {
        return Err(HandshakeError::BadMagic { got: magic });
    }
    let version_major = buf[4];
    let version_minor = buf[5];
    let vendor_id = [buf[6], buf[7]];
    let flags = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let logical_port = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
    Ok(BindConnectionRequest {
        version_major,
        version_minor,
        vendor_id,
        flags,
        logical_port,
    })
}

fn encode_response(resp: &BindConnectionResponse) -> [u8; HANDSHAKE_WIRE_SIZE] {
    let mut buf = [0u8; HANDSHAKE_WIRE_SIZE];
    let (magic, reason) = match resp.status {
        ResponseStatus::Accept => (HANDSHAKE_MAGIC_ACCEPT, 0u32),
        ResponseStatus::Reject(r) => (HANDSHAKE_MAGIC_REJECT, r.as_code()),
    };
    buf[..4].copy_from_slice(&magic);
    buf[4] = resp.version_major;
    buf[5] = resp.version_minor;
    buf[6..8].copy_from_slice(&resp.vendor_id);
    buf[8..12].copy_from_slice(&resp.flags.to_be_bytes());
    buf[12..16].copy_from_slice(&reason.to_be_bytes());
    buf
}

fn decode_response(
    buf: &[u8; HANDSHAKE_WIRE_SIZE],
) -> Result<BindConnectionResponse, HandshakeError> {
    let magic = [buf[0], buf[1], buf[2], buf[3]];
    let status = if magic == HANDSHAKE_MAGIC_ACCEPT {
        ResponseStatus::Accept
    } else if magic == HANDSHAKE_MAGIC_REJECT {
        let reason_code = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
        ResponseStatus::Reject(RejectReason::from_code(reason_code))
    } else {
        return Err(HandshakeError::BadResponse { got: magic });
    };
    Ok(BindConnectionResponse {
        status,
        version_major: buf[4],
        version_minor: buf[5],
        vendor_id: [buf[6], buf[7]],
        flags: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
    })
}

// ---------------------------------------------------------------------
// High-level roles
// ---------------------------------------------------------------------

/// Client-Side des Handshakes. Sendet Request, liest Response,
/// liefert die akzeptierte Server-Identitaet oder einen Error.
///
/// # Errors
/// [`HandshakeError`].
pub fn client_handshake<S: Read + Write>(
    stream: &mut S,
    logical_port: u32,
) -> Result<BindConnectionResponse, HandshakeError> {
    let req = BindConnectionRequest {
        version_major: TCP_PSM_VERSION_MAJOR,
        version_minor: TCP_PSM_VERSION_MINOR,
        vendor_id: VENDOR_ID_ZERODDS,
        flags: 0,
        logical_port,
    };
    let encoded = encode_request(&req);
    stream.write_all(&encoded)?;
    stream.flush()?;

    let mut buf = [0u8; HANDSHAKE_WIRE_SIZE];
    stream.read_exact(&mut buf)?;
    let resp = decode_response(&buf)?;
    match resp.status {
        ResponseStatus::Accept => Ok(resp),
        ResponseStatus::Reject(reason) => Err(HandshakeError::Rejected { reason }),
    }
}

/// Server-Side des Handshakes. Liest Request, prueft Version, sendet
/// Accept- oder Reject-Response. Bei Reject wird trotzdem Ok(...)
/// zurueckgegeben, damit der Caller entscheiden kann, ob er die
/// Verbindung droppet; der Server sollte in der Regel droppen.
///
/// # Errors
/// I/O-Fehler, Protokoll-Fehler beim Request.
pub fn server_handshake<S: Read + Write>(
    stream: &mut S,
) -> Result<(BindConnectionRequest, BindConnectionResponse), HandshakeError> {
    let mut buf = [0u8; HANDSHAKE_WIRE_SIZE];
    stream.read_exact(&mut buf)?;
    let req = decode_request(&buf)?;

    let local = (TCP_PSM_VERSION_MAJOR, TCP_PSM_VERSION_MINOR);
    let peer = (req.version_major, req.version_minor);
    let within_window = peer.0 == local.0
        && peer.0.abs_diff(local.0) <= ACCEPTED_VERSION_DIFF.0
        && peer.1.abs_diff(local.1) <= ACCEPTED_VERSION_DIFF.1;

    let resp = if within_window {
        BindConnectionResponse {
            status: ResponseStatus::Accept,
            version_major: local.0,
            version_minor: local.1,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        }
    } else {
        BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::VersionMismatch),
            version_major: local.0,
            version_minor: local.1,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        }
    };
    let encoded = encode_response(&resp);
    stream.write_all(&encoded)?;
    stream.flush()?;
    Ok((req, resp))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_request() -> BindConnectionRequest {
        BindConnectionRequest {
            version_major: TCP_PSM_VERSION_MAJOR,
            version_minor: TCP_PSM_VERSION_MINOR,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
            logical_port: 7400,
        }
    }

    #[test]
    fn request_roundtrip() {
        let req = sample_request();
        let bytes = encode_request(&req);
        assert_eq!(&bytes[..4], b"ZDDS");
        let back = decode_request(&bytes).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_rejects_wrong_magic() {
        let mut bytes = encode_request(&sample_request());
        bytes[0] = b'H';
        bytes[1] = b'T';
        bytes[2] = b'T';
        bytes[3] = b'P';
        match decode_request(&bytes) {
            Err(HandshakeError::BadMagic { got }) => assert_eq!(got, *b"HTTP"),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn response_roundtrip_accept() {
        let resp = BindConnectionResponse {
            status: ResponseStatus::Accept,
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        };
        let bytes = encode_response(&resp);
        assert_eq!(&bytes[..4], b"ZDA+");
        let back = decode_response(&bytes).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn response_roundtrip_reject() {
        let resp = BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::ResourceLimit),
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        };
        let bytes = encode_response(&resp);
        assert_eq!(&bytes[..4], b"ZDA-");
        let back = decode_response(&bytes).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn reject_reason_roundtrip_known_codes() {
        for r in [
            RejectReason::VersionMismatch,
            RejectReason::ResourceLimit,
            RejectReason::LogicalPortConflict,
            RejectReason::VendorNotAccepted,
        ] {
            assert_eq!(RejectReason::from_code(r.as_code()), r);
        }
    }

    #[test]
    fn reject_reason_unknown_code_maps_to_unknown() {
        assert_eq!(RejectReason::from_code(9999), RejectReason::Unknown);
    }

    // ---- client_handshake-Rejection-Pfade ----

    /// Paired-stream helper: emits a fixed server response and lets
    /// the client read it back while we capture what the client sent.
    fn run_client_against_server_bytes(
        server_response: [u8; HANDSHAKE_WIRE_SIZE],
    ) -> (Result<BindConnectionResponse, HandshakeError>, Vec<u8>) {
        struct PairedStream {
            in_buf: Vec<u8>,
            in_pos: usize,
            out_buf: Vec<u8>,
        }
        impl Read for PairedStream {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let remaining = &self.in_buf[self.in_pos..];
                let n = remaining.len().min(buf.len());
                buf[..n].copy_from_slice(&remaining[..n]);
                self.in_pos += n;
                Ok(n)
            }
        }
        impl Write for PairedStream {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.out_buf.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut s = PairedStream {
            in_buf: server_response.to_vec(),
            in_pos: 0,
            out_buf: Vec::new(),
        };
        let res = client_handshake(&mut s, 7400);
        (res, s.out_buf)
    }

    #[test]
    fn client_handshake_rejects_on_version_mismatch_reply() {
        let reject = encode_response(&BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::VersionMismatch),
            version_major: 2,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        });
        let (res, _) = run_client_against_server_bytes(reject);
        assert!(matches!(
            res,
            Err(HandshakeError::Rejected {
                reason: RejectReason::VersionMismatch
            })
        ));
    }

    #[test]
    fn client_handshake_rejects_on_resource_limit() {
        let reject = encode_response(&BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::ResourceLimit),
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        });
        let (res, _) = run_client_against_server_bytes(reject);
        assert!(matches!(
            res,
            Err(HandshakeError::Rejected {
                reason: RejectReason::ResourceLimit
            })
        ));
    }

    #[test]
    fn client_handshake_rejects_on_logical_port_conflict() {
        let reject = encode_response(&BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::LogicalPortConflict),
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        });
        let (res, _) = run_client_against_server_bytes(reject);
        assert!(matches!(
            res,
            Err(HandshakeError::Rejected {
                reason: RejectReason::LogicalPortConflict
            })
        ));
    }

    #[test]
    fn client_handshake_rejects_on_vendor_not_accepted() {
        let reject = encode_response(&BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::VendorNotAccepted),
            version_major: 1,
            version_minor: 0,
            vendor_id: [0xFF, 0xFF],
            flags: 0,
        });
        let (res, _) = run_client_against_server_bytes(reject);
        assert!(matches!(
            res,
            Err(HandshakeError::Rejected {
                reason: RejectReason::VendorNotAccepted
            })
        ));
    }

    #[test]
    fn client_handshake_errors_on_bad_response_magic() {
        // Server antwortet mit zufaelligen bytes (HTTP-Probe, TLS-Probe).
        let mut bad = [0u8; HANDSHAKE_WIRE_SIZE];
        bad[..4].copy_from_slice(b"HTTP");
        let (res, _) = run_client_against_server_bytes(bad);
        assert!(matches!(res, Err(HandshakeError::BadResponse { .. })));
    }

    #[test]
    fn full_roundtrip_accept_via_paired_cursors() {
        // Simulate a client/server exchange via two back-to-back buffers.
        let mut client_to_server: Vec<u8> = Vec::new();
        let mut server_to_client: Vec<u8> = Vec::new();

        // Client sends its request.
        let req = sample_request();
        client_to_server.extend_from_slice(&encode_request(&req));

        // Server reads request, writes response.
        let mut server_in = Cursor::new(&client_to_server);
        let mut server_out_buf = Vec::new();
        {
            struct DuplexCursor<'a, 'b> {
                inb: &'a mut Cursor<&'b Vec<u8>>,
                outb: &'a mut Vec<u8>,
            }
            impl Read for DuplexCursor<'_, '_> {
                fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                    self.inb.read(b)
                }
            }
            impl Write for DuplexCursor<'_, '_> {
                fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                    self.outb.write(b)
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            let mut duplex = DuplexCursor {
                inb: &mut server_in,
                outb: &mut server_out_buf,
            };
            let (parsed, resp) = server_handshake(&mut duplex).unwrap();
            assert_eq!(parsed, req);
            assert_eq!(resp.status, ResponseStatus::Accept);
        }
        server_to_client.append(&mut server_out_buf);

        // Client reads the response as part of a client_handshake call
        // (we replay its own request into the buffer to match the API).
        let mut full = Vec::new();
        full.extend_from_slice(&server_to_client);
        // client_handshake writes its own request first; since we've
        // already checked the response decode, we just decode_response
        // directly here to keep the test tight.
        let mut arr = [0u8; HANDSHAKE_WIRE_SIZE];
        arr.copy_from_slice(&full[..HANDSHAKE_WIRE_SIZE]);
        let resp = decode_response(&arr).unwrap();
        assert_eq!(resp.status, ResponseStatus::Accept);
    }

    #[test]
    fn server_rejects_version_mismatch() {
        let mut bytes_in = encode_request(&sample_request());
        bytes_in[4] = TCP_PSM_VERSION_MAJOR.wrapping_add(1); // bump major

        let mut transcript: Vec<u8> = Vec::new();
        transcript.extend_from_slice(&bytes_in);
        let mut in_cur = Cursor::new(&transcript);
        let mut out: Vec<u8> = Vec::new();
        struct DuplexCursor<'a, 'b> {
            inb: &'a mut Cursor<&'b Vec<u8>>,
            outb: &'a mut Vec<u8>,
        }
        impl Read for DuplexCursor<'_, '_> {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                self.inb.read(b)
            }
        }
        impl Write for DuplexCursor<'_, '_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.outb.write(b)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut dup = DuplexCursor {
            inb: &mut in_cur,
            outb: &mut out,
        };
        let (_req, resp) = server_handshake(&mut dup).unwrap();
        assert_eq!(
            resp.status,
            ResponseStatus::Reject(RejectReason::VersionMismatch)
        );
        assert_eq!(&out[..4], b"ZDA-");
    }
}
