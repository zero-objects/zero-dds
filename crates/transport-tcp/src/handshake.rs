// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ZeroDDS TCP transport handshake.
//!
//! # Spec status
//!
//! No OMG specification defines a TCP mapping of RTPS, and so none
//! defines a handshake: DDSI-RTPS 2.5 names the locator kinds UDPv4 and
//! UDPv6 (§9.3.2.1), maps messages to UDP/IP (§9.5) and leaves the range
//! 3 to `0x01ffffff` of locator kinds to the vendors (§9.3.2.4). Every
//! vendor opens a TCP connection in its own way.
//!
//! This handshake is part of the **ZeroDDS TCP Transport 1.0** spec
//! (`docs/spec-coverage/zerodds-tcp-transport-1.0.md` §3) and is
//! ZeroDDS-specific. It belongs to the dialect `zerodds` of the transport
//! (`TcpDialect::ZeroDds`, the default). A transport of the dialect `rtps`
//! (`TcpDialect::Rtps`, §2.5) has no handshake: its connections begin
//! with the first RTPS message.
//!
//! # Scope
//!
//! On connection setup in the dialect `zerodds`, client and server
//! exchange a fixed 16-byte `BindConnection` request + response **before**
//! any RTPS frame. The handshake:
//!
//! 1. Verifies that the peer is actually a ZeroDDS TCP transport and
//!    not HTTP/TLS/nonsense — the magic prefix rattles immediately.
//! 2. Aligns protocol version, vendor id, and a logical-port claim.
//! 3. Lets the server reject the connection with a reason code. The
//!    server holds a request against its [`ServerPolicy`] in a fixed
//!    order ([`decide`]): version, reserved flags, vendor id, peer limit,
//!    logical-port claim; the first check that fails names the reason.
//!
//! # Cross-vendor interop
//!
//! In the dialect `zerodds` the handshake is mandatory on both roles:
//! `accept_one` runs `server_handshake_with` before the first frame, so a
//! peer that does not begin with it, one of another vendor or a transport
//! of the dialect `rtps`, is refused with `HandshakeError::BadMagic`. A
//! transport has one dialect, set per transport
//! (`transport.tcp_dialect`); a listener does not tell the two apart at
//! the first octet (§4.2). The dialects of other vendors are open items;
//! see `docs/spec-coverage/zerodds-tcp-transport-1.0.open.md`.
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
//!   |            reason_code (u32)          |   0 on Ok, otherwise per RejectReason
//!   +---------+---------+---------+---------+
//! ```
//!
//! Status in the 4th magic byte:
//! - `b'+'` (0x2B) = Accept
//! - `b'-'` (0x2D) = Reject
//!
//! # Error paths
//!
//! - Wrong magic prefix → `HandshakeError::BadMagic`. The sender is
//!   not ZeroDDS TCP — drop the connection.
//! - Version mismatch → the server answers with Reject/`VersionMismatch`.
//! - A request with a bit of the reserved `flags` set → the server answers
//!   with Reject/`ReservedFlagsSet`; a response with one set → the client
//!   fails with `HandshakeError::ReservedFlags` (§3.5).
//! - Reject for any reason → the client drops the connection and
//!   signals the pool a `backoff`, so no tight loop.

use std::io::{Read, Write};

/// Handshake magic `ZDDS`.
pub const HANDSHAKE_MAGIC_REQUEST: [u8; 4] = *b"ZDDS";
/// Handshake response magic prefix `ZDA` + accept byte.
pub const HANDSHAKE_MAGIC_ACCEPT: [u8; 4] = *b"ZDA+";
/// Handshake response magic prefix `ZDA` + reject byte.
pub const HANDSHAKE_MAGIC_REJECT: [u8; 4] = *b"ZDA-";

/// Protocol major version that this transport carries.
pub const TCP_PSM_VERSION_MAJOR: u8 = 1;
/// Protocol minor version.
pub const TCP_PSM_VERSION_MINOR: u8 = 0;

/// Maximum diff between the peer and local version we still accept.
/// `(0,0)` = exact matches only.
pub const ACCEPTED_VERSION_DIFF: (u8, u8) = (0, 0);

/// Fixed wire size of both handshake frames.
pub const HANDSHAKE_WIRE_SIZE: usize = 16;

/// ZeroDDS vendor id: `zerodds_rtps::wire_types::VendorId::ZERODDS`,
/// `[0x01, 0xF0]`. A hand-written copy here once read `[0x01, 0x0F]`, Fast
/// DDS's OMG vendor id; `tests/contract_rtps.rs` holds the two together.
pub const VENDOR_ID_ZERODDS: [u8; 2] = zerodds_rtps::wire_types::VendorId::ZERODDS.0;

/// Bind-connection request (`zerodds-tcp-transport-1.0` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindConnectionRequest {
    /// Protocol major version of the sender.
    pub version_major: u8,
    /// Protocol minor version.
    pub version_minor: u8,
    /// Vendor id of the sender.
    pub vendor_id: [u8; 2],
    /// Reserved, must be 0.
    pub flags: u32,
    /// The logical port the sender claims on this connection (§3.6): the
    /// port of its own listener, which is the port of the locator it
    /// announces. 0 = no claim. A value above 65535 is no port of a TCP
    /// listener and is refused.
    pub logical_port: u32,
}

/// Bind-connection response status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseStatus {
    /// Handshake accepted.
    Accept,
    /// Handshake rejected — `reason_code` carries the rationale.
    Reject(RejectReason),
}

/// Reject reasons. Numeric values are stable wire codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum RejectReason {
    /// Unknown / over-reserved code.
    Unknown = 0,
    /// Peer version too new/old.
    VersionMismatch = 1,
    /// Server reached its resource limit (MAX_PEERS).
    ResourceLimit = 2,
    /// Logical-port conflict (port already bound).
    LogicalPortConflict = 3,
    /// Vendor id not accepted (policy).
    VendorNotAccepted = 4,
    /// The request has a bit of the reserved `flags` set (§3.5).
    ReservedFlagsSet = 5,
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
            5 => Self::ReservedFlagsSet,
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
    /// Version of the server (the peer echoes it on Accept; on Reject
    /// informative, so the client knows the version).
    pub version_major: u8,
    /// Minor version.
    pub version_minor: u8,
    /// Vendor id of the server.
    pub vendor_id: [u8; 2],
    /// Reserved, must be 0.
    pub flags: u32,
}

/// Error during the handshake.
#[derive(Debug)]
#[non_exhaustive]
pub enum HandshakeError {
    /// I/O error on write/read.
    Io(std::io::Error),
    /// Peer sent a frame without the expected magic prefix.
    BadMagic {
        /// The 4-byte prefix actually received (for diagnostics).
        got: [u8; 4],
    },
    /// Peer version outside the accepted window.
    VersionMismatch {
        /// Peer version (major, minor).
        peer: (u8, u8),
        /// Our version.
        local: (u8, u8),
    },
    /// Server said Reject.
    Rejected {
        /// Reason.
        reason: RejectReason,
    },
    /// Response magic neither Accept nor Reject.
    BadResponse {
        /// 4-byte prefix.
        got: [u8; 4],
    },
    /// The response has a bit of the reserved `flags` set (§3.5): the
    /// server means something by it that this version does not know.
    ReservedFlags {
        /// The `flags` word received.
        got: u32,
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
            Self::ReservedFlags { got } => {
                write!(f, "handshake response with reserved flags set: {got:#010x}")
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

/// Client side of the handshake. Sends the request, reads the
/// response, returns the accepted server identity or an error.
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
    // §3.5: a bit this version does not know is not ignored, whatever the
    // status beside it.
    if resp.flags != 0 {
        return Err(HandshakeError::ReservedFlags { got: resp.flags });
    }
    match resp.status {
        ResponseStatus::Accept => Ok(resp),
        ResponseStatus::Reject(reason) => Err(HandshakeError::Rejected { reason }),
    }
}

/// What the accepting side holds a request against, beside the version
/// and the reserved flags (§3.3). The default accepts every vendor, is
/// below its peer limit and grants every claim.
#[derive(Default)]
pub struct ServerPolicy<'a> {
    /// The vendor ids a peer may carry; `None` accepts every vendor.
    pub accepted_vendors: Option<&'a [[u8; 2]]>,
    /// The listener reads as many connections as it may (§3.7).
    pub at_peer_limit: bool,
    /// Called with the logical port a request claims, never with 0 and
    /// never with a value above 65535, once every other check has passed;
    /// says whether the claim is granted (§3.6). `None` grants every
    /// claim of a port up to 65535.
    pub claim: Option<&'a mut dyn FnMut(u32) -> bool>,
}

impl core::fmt::Debug for ServerPolicy<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ServerPolicy")
            .field("accepted_vendors", &self.accepted_vendors)
            .field("at_peer_limit", &self.at_peer_limit)
            .field("claim", &self.claim.is_some())
            .finish()
    }
}

/// Reads the request of §3.1.
///
/// # Errors
/// I/O error; `BadMagic` for a peer that does not begin with `ZDDS`.
pub fn read_request<S: Read>(stream: &mut S) -> Result<BindConnectionRequest, HandshakeError> {
    let mut buf = [0u8; HANDSHAKE_WIRE_SIZE];
    stream.read_exact(&mut buf)?;
    decode_request(&buf)
}

/// Whether a request of version `peer` is taken.
fn version_accepted(peer: (u8, u8)) -> bool {
    let local = (TCP_PSM_VERSION_MAJOR, TCP_PSM_VERSION_MINOR);
    peer.0 == local.0
        && peer.0.abs_diff(local.0) <= ACCEPTED_VERSION_DIFF.0
        && peer.1.abs_diff(local.1) <= ACCEPTED_VERSION_DIFF.1
}

/// The answer to a request (§3.3). The checks run in a fixed order and
/// the first that fails names the reason:
///
/// 1. the version (`VersionMismatch`): the fields behind it mean what
///    this version says only if it matches;
/// 2. the reserved flags (`ReservedFlagsSet`, §3.5);
/// 3. the vendor id against `policy.accepted_vendors`
///    (`VendorNotAccepted`);
/// 4. the peer limit (`ResourceLimit`, §3.7);
/// 5. the logical-port claim (`LogicalPortConflict`, §3.6). It is last,
///    because a granted claim is held: a request refused for another
///    reason has claimed nothing. A claim above 65535 is refused with the
///    same reason before the policy is asked: it names no port of a TCP
///    listener, and the frames of the connection would carry a source no
///    send can reach.
pub fn decide(req: &BindConnectionRequest, policy: &mut ServerPolicy<'_>) -> ResponseStatus {
    if !version_accepted((req.version_major, req.version_minor)) {
        return ResponseStatus::Reject(RejectReason::VersionMismatch);
    }
    if req.flags != 0 {
        return ResponseStatus::Reject(RejectReason::ReservedFlagsSet);
    }
    if let Some(accepted) = policy.accepted_vendors
        && !accepted.contains(&req.vendor_id)
    {
        return ResponseStatus::Reject(RejectReason::VendorNotAccepted);
    }
    if policy.at_peer_limit {
        return ResponseStatus::Reject(RejectReason::ResourceLimit);
    }
    if req.logical_port > u32::from(u16::MAX) {
        return ResponseStatus::Reject(RejectReason::LogicalPortConflict);
    }
    if req.logical_port != 0
        && let Some(claim) = policy.claim.as_mut()
        && !claim(req.logical_port)
    {
        return ResponseStatus::Reject(RejectReason::LogicalPortConflict);
    }
    ResponseStatus::Accept
}

/// Writes the response of §3.2 with `status`, this side's version and
/// vendor id, and no flag.
///
/// # Errors
/// I/O error.
pub fn write_response<S: Write>(
    stream: &mut S,
    status: ResponseStatus,
) -> Result<BindConnectionResponse, HandshakeError> {
    let resp = BindConnectionResponse {
        status,
        version_major: TCP_PSM_VERSION_MAJOR,
        version_minor: TCP_PSM_VERSION_MINOR,
        vendor_id: VENDOR_ID_ZERODDS,
        flags: 0,
    };
    stream.write_all(&encode_response(&resp))?;
    stream.flush()?;
    Ok(resp)
}

/// Server side of the handshake with the default [`ServerPolicy`]: reads
/// the request, checks the version and the reserved flags, sends an
/// Accept or Reject response. On Reject, Ok(...) is still returned so the
/// caller can decide whether to drop the connection; the server should
/// normally drop.
///
/// # Errors
/// I/O error, protocol error on the request.
pub fn server_handshake<S: Read + Write>(
    stream: &mut S,
) -> Result<(BindConnectionRequest, BindConnectionResponse), HandshakeError> {
    server_handshake_with(stream, &mut ServerPolicy::default())
}

/// Server side of the handshake: [`read_request`], [`decide`] against
/// `policy`, [`write_response`]. On Reject, Ok(...) is still returned; the
/// caller drops the connection (§3.4).
///
/// # Errors
/// I/O error, protocol error on the request.
pub fn server_handshake_with<S: Read + Write>(
    stream: &mut S,
    policy: &mut ServerPolicy<'_>,
) -> Result<(BindConnectionRequest, BindConnectionResponse), HandshakeError> {
    let req = read_request(stream)?;
    let resp = write_response(stream, decide(&req, policy))?;
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
    fn tcp_3_1_request_roundtrip() {
        let req = sample_request();
        let bytes = encode_request(&req);
        assert_eq!(&bytes[..4], b"ZDDS");
        let back = decode_request(&bytes).unwrap();
        assert_eq!(back, req);
    }

    /// §3.1 layout, byte for byte: magic `ZDDS`, major, minor, vendor id,
    /// flags (u32 BE), logical port (u32 BE) — 16 bytes, nothing else.
    #[test]
    fn tcp_3_1_request_wire_layout_is_sixteen_bytes_big_endian() {
        let req = BindConnectionRequest {
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
            logical_port: 7400,
        };
        let bytes = encode_request(&req);
        assert_eq!(bytes.len(), HANDSHAKE_WIRE_SIZE);
        assert_eq!(
            bytes,
            [
                b'Z', b'D', b'D', b'S', // magic
                0x01, 0x00, // version 1.0
                0x01, 0xF0, // vendor id
                0x00, 0x00, 0x00, 0x00, // flags, reserved
                0x00, 0x00, 0x1C, 0xE8, // logical port 7400, big-endian
            ]
        );
    }

    /// §3.2 layout, byte for byte, for both statuses. The reject variant
    /// carries the reason code in the last four bytes, big-endian.
    #[test]
    fn tcp_3_2_response_wire_layout_is_sixteen_bytes_big_endian() {
        let accept = BindConnectionResponse {
            status: ResponseStatus::Accept,
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        };
        assert_eq!(
            encode_response(&accept),
            [
                b'Z', b'D', b'A', b'+', // magic + accept
                0x01, 0x00, 0x01, 0xF0, // version + vendor id
                0x00, 0x00, 0x00, 0x00, // flags
                0x00, 0x00, 0x00, 0x00, // reason code 0 on accept
            ]
        );

        let reject = BindConnectionResponse {
            status: ResponseStatus::Reject(RejectReason::LogicalPortConflict),
            ..accept
        };
        assert_eq!(
            encode_response(&reject),
            [
                b'Z', b'D', b'A', b'-', // magic + reject
                0x01, 0x00, 0x01, 0xF0, // version + vendor id
                0x00, 0x00, 0x00, 0x00, // flags
                0x00, 0x00, 0x00, 0x03, // reason code 3
            ]
        );
    }

    /// A stream that yields `input` and keeps what is written to it.
    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Duplex {
        fn new(input: &[u8]) -> Self {
            Self {
                input: Cursor::new(input.to_vec()),
                output: Vec::new(),
            }
        }
    }

    impl Read for Duplex {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(b)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.output.write(b)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The response octets of a server that reads `req` under `policy`.
    fn server_answer(req: &BindConnectionRequest, policy: &mut ServerPolicy<'_>) -> Vec<u8> {
        let mut stream = Duplex::new(&encode_request(req));
        server_handshake_with(&mut stream, policy).unwrap();
        stream.output
    }

    /// The codec carries the `flags` word as it is; the check is in the
    /// roles (§3.5).
    #[test]
    fn the_codec_carries_the_flags_word() {
        let req = BindConnectionRequest {
            flags: 0xDEAD_BEEF,
            ..sample_request()
        };
        let bytes = encode_request(&req);
        assert_eq!(&bytes[8..12], &0xDEAD_BEEFu32.to_be_bytes());
        let back = decode_request(&bytes).unwrap();
        assert_eq!(back.flags, 0xDEAD_BEEF);
    }

    /// §3.5: a request with reserved flags set is answered `ZDA-` with
    /// reason code 5, and the claim it carries is not looked at.
    #[test]
    fn tcp_3_5_server_rejects_a_request_with_reserved_flags_set() {
        let req = BindConnectionRequest {
            flags: 0x8000_0001,
            ..sample_request()
        };
        let claims = core::cell::Cell::new(0u32);
        let mut claim = |_port: u32| {
            claims.set(claims.get() + 1);
            true
        };
        let mut policy = ServerPolicy {
            claim: Some(&mut claim),
            ..ServerPolicy::default()
        };
        let out = server_answer(&req, &mut policy);
        assert_eq!(out.len(), HANDSHAKE_WIRE_SIZE);
        assert_eq!(&out[..4], b"ZDA-");
        assert_eq!(out[8..12], [0, 0, 0, 0], "the response sets no flag");
        assert_eq!(out[12..16], 5u32.to_be_bytes());
        assert_eq!(claims.get(), 0);
        // The same request without the flags is accepted and claims.
        let out = server_answer(&sample_request(), &mut policy);
        assert_eq!(&out[..4], b"ZDA+");
        assert_eq!(claims.get(), 1);
    }

    /// §3.5: the client fails on a response with reserved flags set,
    /// Accept or Reject, and takes the same response without them.
    #[test]
    fn tcp_3_5_client_drops_a_response_with_reserved_flags_set() {
        for status in [
            ResponseStatus::Accept,
            ResponseStatus::Reject(RejectReason::ResourceLimit),
        ] {
            let with_flags = encode_response(&BindConnectionResponse {
                status,
                version_major: 1,
                version_minor: 0,
                vendor_id: VENDOR_ID_ZERODDS,
                flags: 0x0000_0100,
            });
            let (res, _) = run_client_against_server_bytes(with_flags);
            match res {
                Err(HandshakeError::ReservedFlags { got }) => assert_eq!(got, 0x0000_0100),
                other => panic!("expected ReservedFlags, got {other:?}"),
            }
        }
        let clean = encode_response(&BindConnectionResponse {
            status: ResponseStatus::Accept,
            version_major: 1,
            version_minor: 0,
            vendor_id: VENDOR_ID_ZERODDS,
            flags: 0,
        });
        let (res, sent) = run_client_against_server_bytes(clean);
        assert!(res.is_ok());
        assert_eq!(sent[8..12], [0, 0, 0, 0], "the client sets no flag");
    }

    /// §3.5: each of the 32 bits alone is refused, in both directions.
    #[test]
    fn tcp_3_5_each_single_flag_bit_is_rejected() {
        for bit in 0..32 {
            let flags = 1u32 << bit;
            let req = BindConnectionRequest {
                flags,
                ..sample_request()
            };
            assert_eq!(
                decide(&req, &mut ServerPolicy::default()),
                ResponseStatus::Reject(RejectReason::ReservedFlagsSet),
                "request bit {bit}"
            );
            let resp = encode_response(&BindConnectionResponse {
                status: ResponseStatus::Accept,
                version_major: 1,
                version_minor: 0,
                vendor_id: VENDOR_ID_ZERODDS,
                flags,
            });
            let (res, _) = run_client_against_server_bytes(resp);
            assert!(
                matches!(res, Err(HandshakeError::ReservedFlags { got }) if got == flags),
                "response bit {bit}: {res:?}"
            );
        }
    }

    /// §3.3: a vendor id that is not on the list is answered `ZDA-` with
    /// reason code 4.
    #[test]
    fn tcp_3_3_server_rejects_vendor_not_accepted() {
        let accepted = [[0x01, 0x10], [0x01, 0x0F]];
        let mut policy = ServerPolicy {
            accepted_vendors: Some(&accepted),
            ..ServerPolicy::default()
        };
        let out = server_answer(&sample_request(), &mut policy);
        assert_eq!(&out[..4], b"ZDA-");
        assert_eq!(out[12..16], 4u32.to_be_bytes());
        // An empty list accepts no vendor.
        let mut none = ServerPolicy {
            accepted_vendors: Some(&[]),
            ..ServerPolicy::default()
        };
        assert_eq!(
            decide(&sample_request(), &mut none),
            ResponseStatus::Reject(RejectReason::VendorNotAccepted)
        );
    }

    /// §3.3: a vendor on the list is accepted, and so is every vendor
    /// without a list.
    #[test]
    fn tcp_3_3_server_accepts_a_listed_vendor() {
        let accepted = [[0x01, 0x10], VENDOR_ID_ZERODDS];
        let mut policy = ServerPolicy {
            accepted_vendors: Some(&accepted),
            ..ServerPolicy::default()
        };
        let out = server_answer(&sample_request(), &mut policy);
        assert_eq!(&out[..4], b"ZDA+");
        assert_eq!(out[12..16], [0, 0, 0, 0]);
        let other = BindConnectionRequest {
            vendor_id: [0x01, 0x10],
            ..sample_request()
        };
        assert_eq!(decide(&other, &mut policy), ResponseStatus::Accept);
        let unknown = BindConnectionRequest {
            vendor_id: [0xAB, 0xCD],
            ..sample_request()
        };
        assert_eq!(
            decide(&unknown, &mut policy),
            ResponseStatus::Reject(RejectReason::VendorNotAccepted)
        );
        assert_eq!(
            decide(&unknown, &mut ServerPolicy::default()),
            ResponseStatus::Accept
        );
    }

    /// §3.3: a request that fails every check is refused for the version;
    /// with each check repaired in turn the next one names the reason,
    /// and the claim is asked for only when all others have passed.
    #[test]
    fn tcp_3_3_decide_checks_in_the_stated_order() {
        let accepted = [VENDOR_ID_ZERODDS];
        let asked = core::cell::Cell::new(0u32);
        let mut refuse = |_port: u32| {
            asked.set(asked.get() + 1);
            false
        };
        let mut policy = ServerPolicy {
            accepted_vendors: Some(&accepted),
            at_peer_limit: true,
            claim: Some(&mut refuse),
        };
        let mut req = BindConnectionRequest {
            version_major: 2,
            version_minor: 0,
            vendor_id: [0xAB, 0xCD],
            flags: 1,
            logical_port: 7400,
        };
        let reason = |req: &BindConnectionRequest, policy: &mut ServerPolicy<'_>| match decide(
            req, policy,
        ) {
            ResponseStatus::Reject(reason) => Some(reason),
            ResponseStatus::Accept => None,
        };
        assert_eq!(
            reason(&req, &mut policy),
            Some(RejectReason::VersionMismatch)
        );
        req.version_major = 1;
        assert_eq!(
            reason(&req, &mut policy),
            Some(RejectReason::ReservedFlagsSet)
        );
        req.flags = 0;
        assert_eq!(
            reason(&req, &mut policy),
            Some(RejectReason::VendorNotAccepted)
        );
        req.vendor_id = VENDOR_ID_ZERODDS;
        assert_eq!(reason(&req, &mut policy), Some(RejectReason::ResourceLimit));
        assert_eq!(asked.get(), 0, "no claim is taken for a refused request");
        policy.at_peer_limit = false;
        assert_eq!(
            reason(&req, &mut policy),
            Some(RejectReason::LogicalPortConflict)
        );
        assert_eq!(asked.get(), 1);
        // No claim: the callback is not asked.
        req.logical_port = 0;
        assert_eq!(reason(&req, &mut policy), None);
        assert_eq!(asked.get(), 1);
    }

    /// §3.6: a claim above 65535 names no port of a TCP listener. The
    /// response is `ZDA-` with reason code 3, with a policy that grants
    /// every claim and with one that is asked; that one is not asked.
    /// 65535 is a port and is granted.
    #[test]
    fn tcp_3_6_a_claim_above_65535_is_refused() {
        let beyond = BindConnectionRequest {
            logical_port: 65_536,
            ..sample_request()
        };
        let out = server_answer(&beyond, &mut ServerPolicy::default());
        assert_eq!(&out[..4], b"ZDA-");
        assert_eq!(out[4..8], [1, 0, 0x01, 0xF0]);
        assert_eq!(out[8..12], [0, 0, 0, 0]);
        assert_eq!(out[12..16], 3u32.to_be_bytes());

        let asked = core::cell::Cell::new(0u32);
        let mut grant = |_port: u32| {
            asked.set(asked.get() + 1);
            true
        };
        let mut policy = ServerPolicy {
            claim: Some(&mut grant),
            ..ServerPolicy::default()
        };
        let highest = BindConnectionRequest {
            logical_port: u32::MAX,
            ..sample_request()
        };
        for req in [&beyond, &highest] {
            assert_eq!(
                decide(req, &mut policy),
                ResponseStatus::Reject(RejectReason::LogicalPortConflict)
            );
        }
        assert_eq!(
            asked.get(),
            0,
            "the policy is not asked for what is no port"
        );
        let last = BindConnectionRequest {
            logical_port: 65_535,
            ..sample_request()
        };
        assert_eq!(decide(&last, &mut policy), ResponseStatus::Accept);
        assert_eq!(asked.get(), 1);
        let out = server_answer(&last, &mut ServerPolicy::default());
        assert_eq!(&out[..4], b"ZDA+");
    }

    #[test]
    fn tcp_3_1_request_rejects_wrong_magic() {
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
    fn tcp_3_2_response_roundtrip_accept() {
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
    fn tcp_3_2_response_roundtrip_reject() {
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
    fn tcp_3_3_reject_reason_roundtrip_known_codes() {
        for r in [
            RejectReason::VersionMismatch,
            RejectReason::ResourceLimit,
            RejectReason::LogicalPortConflict,
            RejectReason::VendorNotAccepted,
            RejectReason::ReservedFlagsSet,
        ] {
            assert_eq!(RejectReason::from_code(r.as_code()), r);
        }
        assert_eq!(RejectReason::ReservedFlagsSet.as_code(), 5);
        assert_eq!(RejectReason::from_code(0), RejectReason::Unknown);
    }

    #[test]
    fn tcp_3_3_reject_reason_unknown_code_maps_to_unknown() {
        assert_eq!(RejectReason::from_code(9999), RejectReason::Unknown);
    }

    // ---- client_handshake rejection paths ----

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
    fn tcp_3_3_client_handshake_rejects_on_resource_limit() {
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
    fn tcp_3_3_client_handshake_rejects_on_logical_port_conflict() {
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
    fn tcp_3_3_client_handshake_rejects_on_vendor_not_accepted() {
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
    fn tcp_3_2_client_handshake_errors_on_bad_response_magic() {
        // Server answers with random bytes (HTTP probe, TLS probe).
        let mut bad = [0u8; HANDSHAKE_WIRE_SIZE];
        bad[..4].copy_from_slice(b"HTTP");
        let (res, _) = run_client_against_server_bytes(bad);
        assert!(matches!(res, Err(HandshakeError::BadResponse { .. })));
    }

    #[test]
    fn tcp_3_4_full_roundtrip_accept_via_paired_cursors() {
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
    fn tcp_3_3_server_rejects_version_mismatch() {
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
