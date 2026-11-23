// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TCP protocol of Fast DDS, the dialect `fastdds`
//! (`docs/spec-coverage/zerodds-tcp-transport-1.0.md` §6.1).
//!
//! Fast DDS calls it RTCP (`src/cpp/rtps/transport/tcp/` of Fast DDS:
//! `RTCPHeader.h`, `RTCPMessageManager.cpp`, `TCPControlMessage.cpp`). The
//! layouts below were read in the source of Fast DDS 3.6.1 and checked
//! against octets Fast DDS 3.6.1 wrote on the Linux test host
//! (`tests/data/fastdds_3_6_1_rtcp_*.hex`, captured with
//! `tests/data/fastdds_3_6_1_rtcp_capture.py`).
//!
//! Every message on the connection, RTPS or control, has a 14-octet
//! header in front:
//!
//! ```text
//! 0      4          8          12      14
//! +------+----------+----------+-------+----------------------- ...
//! | RTCP | length   | crc      | port  | RTPS message or control message
//! +------+----------+----------+-------+----------------------- ...
//! ```
//!
//! `length` counts the header and what follows it, `crc` is the sum of the
//! octets behind the header with the carry added back in (one's
//! complement), `port` is the logical port the message is for. Fast DDS
//! writes the three fields in the order of its host and reads them in
//! the order of its host, without a flag; this module writes and reads
//! them little-endian, the order of every host Fast DDS runs on (x86-64,
//! AArch64). Port 0 is a control message:
//!
//! ```text
//! 0      1       2        4                 16
//! +------+-------+--------+-----------------+--------+------------------
//! | kind | flags | length | transaction id  | [code] | [payload]
//! +------+-------+--------+-----------------+--------+------------------
//! ```
//!
//! `length` counts the 16 octets of this header and what follows it. A
//! response (`0xE1` to `0xE4`) carries a four-octet response code. A
//! payload is a two-octet encapsulation id, a four-octet length and that
//! many octets of CDR data, which begin with their own encapsulation
//! header (`00 01 00 00`, little-endian CDR).
//!
//! A connection begins with a BindConnectionRequest of the connecting
//! side, which names its protocol version (1.0), its vendor id and the
//! locator of its listener; the accepting side answers with its own
//! locator. Before the first RTPS message to a logical port the sending
//! side asks the other with an OpenLogicalPortRequest whether that port
//! is open there. Both sides send and ask on the one connection.
//!
//! A Fast DDS locator of kind 4 (TCPv4) or 8 (TCPv6) holds two ports in
//! its `port` field: the physical port, the TCP port of the listener, in
//! the lower 16 bits, and the logical port in the upper 16. In a TCPv4
//! locator the octets 8 to 11 of the address are the WAN address, the
//! address the host is reached at from outside its network, and 12 to 15
//! the address in its network.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

extern crate alloc;
use alloc::vec::Vec;

use crate::framing::{FramingError, MAX_FRAME_SIZE, read_exact_or_eof, read_head};

/// The four octets each message begins with.
pub const MAGIC: [u8; 4] = *b"RTCP";

/// Length of the header in front of each message.
pub const TCP_HEADER_LEN: usize = 14;

/// Length of the header of a control message.
pub const CONTROL_HEADER_LEN: usize = 16;

/// The protocol version a BindConnectionRequest names; Fast DDS refuses
/// another with `IncompatibleVersion`.
pub const PROTOCOL_VERSION: [u8; 2] = [1, 0];

/// The largest RTPS message a `send` writes: Fast DDS reads a message into
/// a buffer of its `maxMessageSize`, 65500 by default, and drops a longer
/// one whole.
pub const MAX_MESSAGE_SIZE: usize = 65_500;

/// `LOCATOR_KIND_TCPv4` of Fast DDS.
pub const LOCATOR_KIND_TCPV4: i32 = 4;

/// `LOCATOR_KIND_TCPv6` of Fast DDS.
pub const LOCATOR_KIND_TCPV6: i32 = 8;

/// How long a logical port the peer refused is refused to a `send`
/// without asking again.
pub const REFUSED_PORT_HOLD: Duration = Duration::from_secs(1);

/// The kinds of control messages (`TCPCPMKind` of Fast DDS).
pub mod kind {
    /// BindConnectionRequest: the first message of a connection.
    pub const BIND_CONNECTION_REQUEST: u8 = 0xD1;
    /// BindConnectionResponse.
    pub const BIND_CONNECTION_RESPONSE: u8 = 0xE1;
    /// OpenLogicalPortRequest: is this logical port open at the peer.
    pub const OPEN_LOGICAL_PORT_REQUEST: u8 = 0xD2;
    /// OpenLogicalPortResponse.
    pub const OPEN_LOGICAL_PORT_RESPONSE: u8 = 0xE2;
    /// CheckLogicalPortsRequest: which of these logical ports are open.
    pub const CHECK_LOGICAL_PORT_REQUEST: u8 = 0xD3;
    /// CheckLogicalPortsResponse.
    pub const CHECK_LOGICAL_PORT_RESPONSE: u8 = 0xE3;
    /// KeepAliveRequest.
    pub const KEEP_ALIVE_REQUEST: u8 = 0xD4;
    /// KeepAliveResponse.
    pub const KEEP_ALIVE_RESPONSE: u8 = 0xE4;
    /// LogicalPortIsClosedRequest: a logical port is no longer open.
    pub const LOGICAL_PORT_IS_CLOSED_REQUEST: u8 = 0xD5;
    /// UnbindConnectionRequest: the sender closes the connection.
    pub const UNBIND_CONNECTION_REQUEST: u8 = 0xD6;
}

/// The response codes (`ResponseCode` of Fast DDS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseCode {
    /// 0.
    Ok,
    /// 1: the message was not understood.
    BadRequest,
    /// 2: the connection was bound already.
    ExistingConnection,
    /// 3: the logical port is not open.
    InvalidPort,
    /// 4: the locator of a KeepAliveRequest is not the connection's.
    UnknownLocator,
    /// 5: another protocol version.
    IncompatibleVersion,
    /// 6: the request cannot be served now.
    ServerError,
    /// A value Fast DDS does not define.
    Other(u32),
}

impl ResponseCode {
    /// The value on the wire.
    #[must_use]
    pub fn as_u32(self) -> u32 {
        match self {
            Self::Ok => 0,
            Self::BadRequest => 1,
            Self::ExistingConnection => 2,
            Self::InvalidPort => 3,
            Self::UnknownLocator => 4,
            Self::IncompatibleVersion => 5,
            Self::ServerError => 6,
            Self::Other(v) => v,
        }
    }

    /// The code of a value on the wire.
    #[must_use]
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => Self::Ok,
            1 => Self::BadRequest,
            2 => Self::ExistingConnection,
            3 => Self::InvalidPort,
            4 => Self::UnknownLocator,
            5 => Self::IncompatibleVersion,
            6 => Self::ServerError,
            other => Self::Other(other),
        }
    }
}

/// The header in front of each message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpHeader {
    /// Octets of the header and what follows it.
    pub length: u32,
    /// The sum of the octets behind the header ([`crc`]).
    pub crc: u32,
    /// The logical port the message is for; 0 for a control message.
    pub logical_port: u16,
}

impl TcpHeader {
    /// The header of `body` for `logical_port`.
    ///
    /// # Errors
    /// `FrameTooLarge` if `body` with the header is longer than a `u32`
    /// counts.
    pub fn of(body: &[u8], logical_port: u16) -> Result<Self, FramingError> {
        let length = u32::try_from(body.len() + TCP_HEADER_LEN).map_err(|_| {
            FramingError::FrameTooLarge {
                announced: u32::MAX,
            }
        })?;
        Ok(Self {
            length,
            crc: crc(body),
            logical_port,
        })
    }

    /// The 14 octets on the wire.
    #[must_use]
    pub fn encode(&self) -> [u8; TCP_HEADER_LEN] {
        let mut out = [0u8; TCP_HEADER_LEN];
        out[..4].copy_from_slice(&MAGIC);
        out[4..8].copy_from_slice(&self.length.to_le_bytes());
        out[8..12].copy_from_slice(&self.crc.to_le_bytes());
        out[12..].copy_from_slice(&self.logical_port.to_le_bytes());
        out
    }

    /// The header in `octets`.
    ///
    /// # Errors
    /// [`RtcpError::NotRtcp`] if they do not begin with `RTCP`,
    /// [`RtcpError::LengthTooSmall`] if the length is below the header's.
    pub fn decode(octets: &[u8; TCP_HEADER_LEN]) -> Result<Self, RtcpError> {
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&octets[..4]);
        if magic != MAGIC {
            return Err(RtcpError::NotRtcp { got: magic });
        }
        let field = |range: core::ops::Range<usize>| {
            let mut v = [0u8; 4];
            v.copy_from_slice(&octets[range]);
            u32::from_le_bytes(v)
        };
        let length = field(4..8);
        if (length as usize) < TCP_HEADER_LEN {
            return Err(RtcpError::LengthTooSmall { announced: length });
        }
        Ok(Self {
            length,
            crc: field(8..12),
            logical_port: u16::from_le_bytes([octets[12], octets[13]]),
        })
    }
}

/// The checksum Fast DDS puts into the header (`RTCPMessageManager::addToCRC`):
/// the octets added up in 32 bits, a carry out of the top bit added back
/// in.
#[must_use]
pub fn crc(data: &[u8]) -> u32 {
    data.iter().fold(0u32, |sum, &octet| {
        let (next, carry) = sum.overflowing_add(u32::from(octet));
        next.wrapping_add(u32::from(carry))
    })
}

/// A locator as RTCP carries it: kind, port and address, 24 octets of
/// CDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireLocator {
    /// [`LOCATOR_KIND_TCPV4`] or [`LOCATOR_KIND_TCPV6`].
    pub kind: i32,
    /// Physical port in the lower 16 bits, logical port in the upper 16.
    pub port: u32,
    /// The address; for TCPv4 the WAN address in octets 8 to 11 and the
    /// address in the host's network in 12 to 15.
    pub address: [u8; 16],
}

impl WireLocator {
    /// The locator of a socket address, logical port 0.
    #[must_use]
    pub fn of_socket(addr: SocketAddr) -> Self {
        let mut address = [0u8; 16];
        let kind = match addr.ip() {
            IpAddr::V4(v4) => {
                address[12..].copy_from_slice(&v4.octets());
                LOCATOR_KIND_TCPV4
            }
            IpAddr::V6(v6) => {
                address = v6.octets();
                LOCATOR_KIND_TCPV6
            }
        };
        Self {
            kind,
            port: u32::from(addr.port()),
            address,
        }
    }

    /// The physical port: the TCP port of the listener.
    #[must_use]
    pub fn physical_port(&self) -> u16 {
        (self.port & 0xFFFF) as u16
    }

    /// The logical port.
    #[must_use]
    pub fn logical_port(&self) -> u16 {
        (self.port >> 16) as u16
    }

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.kind.to_le_bytes());
        out.extend_from_slice(&self.port.to_le_bytes());
        out.extend_from_slice(&self.address);
    }
}

/// The payload of a BindConnectionRequest (`ConnectionRequest_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionRequest {
    /// Major and minor protocol version.
    pub protocol_version: [u8; 2],
    /// The vendor id of the connecting side.
    pub vendor_id: [u8; 2],
    /// The locator of the connecting side's listener.
    pub transport_locator: WireLocator,
}

/// A control message read from the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlMessage {
    /// The kind ([`kind`]); also one this module does not know.
    pub kind: u8,
    /// The transaction id a response repeats.
    pub transaction_id: [u8; 12],
    /// The response code of a response.
    pub code: Option<ResponseCode>,
    /// The CDR data of the payload, with its encapsulation header.
    pub payload: Option<Vec<u8>>,
}

/// What a control message carries that this module cannot take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadControl {
    /// The kind of the message, for the answer.
    pub kind: u8,
    /// Its transaction id, for the answer.
    pub transaction_id: [u8; 12],
}

/// Whether a kind is a response, which carries a code.
fn is_response(kind: u8) -> bool {
    matches!(
        kind,
        kind::BIND_CONNECTION_RESPONSE
            | kind::OPEN_LOGICAL_PORT_RESPONSE
            | kind::CHECK_LOGICAL_PORT_RESPONSE
            | kind::KEEP_ALIVE_RESPONSE
    )
}

/// The flags Fast DDS writes for a kind (`RTCPMessageManager::fillHeaders`):
/// `0x02` little-endian, `0x04` payload, `0x08` a response is wanted; by the
/// kind, also where the message has no payload.
fn flags_of(kind: u8) -> u8 {
    match kind {
        kind::BIND_CONNECTION_REQUEST
        | kind::OPEN_LOGICAL_PORT_REQUEST
        | kind::CHECK_LOGICAL_PORT_REQUEST
        | kind::KEEP_ALIVE_REQUEST => 0x0E,
        kind::UNBIND_CONNECTION_REQUEST => 0x02,
        _ => 0x06,
    }
}

impl ControlMessage {
    /// The control message in `body`, the octets behind the TCP header.
    ///
    /// # Errors
    /// [`BadControl`] if `body` is shorter than the control header, its
    /// length is not that of `body`, or the payload does not fit into it:
    /// Fast DDS answers such a message with its kind and `BadRequest`.
    pub fn decode(body: &[u8]) -> Result<Self, Option<BadControl>> {
        if body.len() < CONTROL_HEADER_LEN {
            return Err(None);
        }
        let kind = body[0];
        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&body[4..16]);
        let bad = BadControl {
            kind,
            transaction_id,
        };
        let length = usize::from(u16::from_le_bytes([body[2], body[3]]));
        if length != body.len() {
            return Err(Some(bad));
        }
        let mut rest = &body[CONTROL_HEADER_LEN..];
        let code = if is_response(kind) {
            let Some((code, after)) = rest.split_first_chunk::<4>() else {
                return Err(Some(bad));
            };
            rest = after;
            Some(ResponseCode::from_u32(u32::from_le_bytes(*code)))
        } else {
            None
        };
        let payload = if rest.is_empty() {
            None
        } else {
            let Some((head, data)) = rest.split_first_chunk::<6>() else {
                return Err(Some(bad));
            };
            let len = u32::from_le_bytes([head[2], head[3], head[4], head[5]]) as usize;
            if data.len() != len {
                return Err(Some(bad));
            }
            Some(data.to_vec())
        };
        Ok(Self {
            kind,
            transaction_id,
            code,
            payload,
        })
    }

    /// The whole message on the wire, TCP header in front.
    ///
    /// # Errors
    /// `FrameTooLarge` if the message is longer than its length field
    /// counts.
    pub fn encode(&self) -> Result<Vec<u8>, FramingError> {
        let mut body = Vec::with_capacity(64);
        body.push(self.kind);
        body.push(flags_of(self.kind));
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(&self.transaction_id);
        if let Some(code) = self.code {
            body.extend_from_slice(&code.as_u32().to_le_bytes());
        }
        if let Some(data) = &self.payload {
            let len = u32::try_from(data.len()).map_err(|_| FramingError::FrameTooLarge {
                announced: u32::MAX,
            })?;
            // CDR_LE, the encapsulation id Fast DDS writes in front.
            body.extend_from_slice(&1u16.to_le_bytes());
            body.extend_from_slice(&len.to_le_bytes());
            body.extend_from_slice(data);
        }
        let length = u16::try_from(body.len()).map_err(|_| FramingError::FrameTooLarge {
            announced: u32::try_from(body.len()).unwrap_or(u32::MAX),
        })?;
        body[2..4].copy_from_slice(&length.to_le_bytes());
        let mut out = Vec::with_capacity(TCP_HEADER_LEN + body.len());
        out.extend_from_slice(&TcpHeader::of(&body, 0)?.encode());
        out.extend_from_slice(&body);
        Ok(out)
    }
}

/// CDR data of a payload: the encapsulation header of little-endian CDR
/// and `body`.
fn cdr(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    out.extend_from_slice(body);
    out
}

/// A reader of CDR data with its encapsulation header: big-endian for
/// `00 00`, little-endian for `00 01`; alignment from the first octet
/// behind the header.
struct CdrReader<'a> {
    data: &'a [u8],
    pos: usize,
    little: bool,
}

impl<'a> CdrReader<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let (head, body) = data.split_first_chunk::<4>()?;
        let little = match head[..2] {
            [0x00, 0x01] => true,
            [0x00, 0x00] => false,
            _ => return None,
        };
        Some(Self {
            data: body,
            pos: 0,
            little,
        })
    }

    fn take(&mut self, n: usize, align: usize) -> Option<&'a [u8]> {
        self.pos = self.pos.div_ceil(align) * align;
        let out = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(out)
    }

    fn u16(&mut self) -> Option<u16> {
        let b: [u8; 2] = self.take(2, 2)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }

    fn u32(&mut self) -> Option<u32> {
        let b: [u8; 4] = self.take(4, 4)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }

    fn octets<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N, 1)?.try_into().ok()
    }

    fn locator(&mut self) -> Option<WireLocator> {
        Some(WireLocator {
            kind: self.u32()? as i32,
            port: self.u32()?,
            address: self.octets::<16>()?,
        })
    }
}

/// The CDR data of a ConnectionRequest.
#[must_use]
pub fn encode_connection_request(request: &ConnectionRequest) -> Vec<u8> {
    let mut body = Vec::with_capacity(28);
    body.extend_from_slice(&request.protocol_version);
    body.extend_from_slice(&request.vendor_id);
    request.transport_locator.encode(&mut body);
    cdr(&body)
}

/// The ConnectionRequest in CDR data.
#[must_use]
pub fn decode_connection_request(data: &[u8]) -> Option<ConnectionRequest> {
    let mut r = CdrReader::new(data)?;
    Some(ConnectionRequest {
        protocol_version: r.octets::<2>()?,
        vendor_id: r.octets::<2>()?,
        transport_locator: r.locator()?,
    })
}

/// The CDR data of a locator (BindConnectionResponse, KeepAliveRequest).
#[must_use]
pub fn encode_locator(locator: &WireLocator) -> Vec<u8> {
    let mut body = Vec::with_capacity(24);
    locator.encode(&mut body);
    cdr(&body)
}

/// The locator in CDR data.
#[must_use]
pub fn decode_locator(data: &[u8]) -> Option<WireLocator> {
    CdrReader::new(data)?.locator()
}

/// The CDR data of a logical port (OpenLogicalPortRequest,
/// LogicalPortIsClosedRequest).
#[must_use]
pub fn encode_port(port: u16) -> Vec<u8> {
    cdr(&port.to_le_bytes())
}

/// The logical port in CDR data.
#[must_use]
pub fn decode_port(data: &[u8]) -> Option<u16> {
    CdrReader::new(data)?.u16()
}

/// The CDR data of a sequence of logical ports (CheckLogicalPorts).
#[must_use]
pub fn encode_ports(ports: &[u16]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + 2 * ports.len());
    body.extend_from_slice(&u32::try_from(ports.len()).unwrap_or(u32::MAX).to_le_bytes());
    for port in ports {
        body.extend_from_slice(&port.to_le_bytes());
    }
    cdr(&body)
}

/// The sequence of logical ports in CDR data.
#[must_use]
pub fn decode_ports(data: &[u8]) -> Option<Vec<u16>> {
    let mut r = CdrReader::new(data)?;
    let n = r.u32()? as usize;
    // Two octets each: a count above what the data holds is no sequence.
    if n > data.len() / 2 {
        return None;
    }
    (0..n).map(|_| r.u16()).collect()
}

/// One message read from a connection: the TCP header and what follows.
#[derive(Debug)]
pub struct Message {
    /// The header.
    pub header: TcpHeader,
    /// The RTPS message or control message behind it.
    pub body: Vec<u8>,
}

/// Reads one message. A stream that ends before its first octet is
/// `Closed`; one that ends inside it `UnexpectedEof`.
///
/// # Errors
/// [`RtcpError::Framing`] for the end of the stream and I/O errors,
/// [`RtcpError::NotRtcp`] and [`RtcpError::LengthTooSmall`] for a header
/// that is none, `FrameTooLarge` for a message above
/// [`MAX_FRAME_SIZE`], [`RtcpError::CrcMismatch`] for a checksum that is
/// not 0 and not the sum of the body. Fast DDS writes 0 when its
/// `calculate_crc` is off.
pub fn read_message<R: Read>(r: &mut R) -> Result<Message, RtcpError> {
    let mut head = [0u8; TCP_HEADER_LEN];
    read_head(r, &mut head[..4]).map_err(RtcpError::Framing)?;
    if head[..4] != MAGIC {
        let mut got = [0u8; 4];
        got.copy_from_slice(&head[..4]);
        return Err(RtcpError::NotRtcp { got });
    }
    read_exact_or_eof(r, &mut head[4..]).map_err(RtcpError::Framing)?;
    read_after_head(&head, r)
}

/// Reads the rest of a message whose 14 header octets are `head`.
///
/// # Errors
/// As [`read_message`].
pub fn read_after_head<R: Read>(
    head: &[u8; TCP_HEADER_LEN],
    r: &mut R,
) -> Result<Message, RtcpError> {
    let header = TcpHeader::decode(head)?;
    let body_len = header.length - TCP_HEADER_LEN as u32;
    if body_len > MAX_FRAME_SIZE {
        return Err(RtcpError::Framing(FramingError::FrameTooLarge {
            announced: body_len,
        }));
    }
    let mut body = alloc::vec![0u8; body_len as usize];
    read_exact_or_eof(r, &mut body).map_err(RtcpError::Framing)?;
    if header.crc != 0 {
        let computed = crc(&body);
        if computed != header.crc {
            return Err(RtcpError::CrcMismatch {
                stated: header.crc,
                computed,
            });
        }
    }
    Ok(Message { header, body })
}

/// The RTPS message `data` for `logical_port`, TCP header in front.
///
/// # Errors
/// `FrameTooLarge` above [`MAX_MESSAGE_SIZE`].
pub fn data_message(logical_port: u16, data: &[u8]) -> Result<Vec<u8>, FramingError> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(FramingError::FrameTooLarge {
            announced: u32::try_from(data.len()).unwrap_or(u32::MAX),
        });
    }
    let mut out = Vec::with_capacity(TCP_HEADER_LEN + data.len());
    out.extend_from_slice(&TcpHeader::of(data, logical_port)?.encode());
    out.extend_from_slice(data);
    Ok(out)
}

/// Why a connection of the dialect `fastdds` failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum RtcpError {
    /// The end of the stream, an I/O error or a message above the limit.
    Framing(FramingError),
    /// A message did not begin with `RTCP`. Fast DDS skips octets until
    /// the next `RTCP`; a stream has no other way to find the next message,
    /// and this side closes the connection.
    NotRtcp {
        /// The four octets found.
        got: [u8; 4],
    },
    /// The length of a header is below the header's own.
    LengthTooSmall {
        /// The length stated.
        announced: u32,
    },
    /// The checksum of a message is not the sum of its octets.
    CrcMismatch {
        /// The checksum in the header.
        stated: u32,
        /// The sum of the octets.
        computed: u32,
    },
    /// The first message of an accepted connection is no
    /// BindConnectionRequest.
    NoBindRequest,
    /// An RTPS message arrived before the connection was bound.
    DataBeforeBind,
    /// A control message shorter than its 16-octet header.
    ShortControl {
        /// The octets behind the TCP header.
        len: usize,
    },
    /// The peer answered the BindConnectionRequest with this code.
    BindRefused(ResponseCode),
    /// This side refused the peer's BindConnectionRequest with this code.
    BindDeclined(ResponseCode),
    /// The peer refused to open this logical port (`InvalidPort`, or
    /// another code), or did so within [`REFUSED_PORT_HOLD`].
    PortRefused {
        /// The logical port.
        port: u16,
        /// The code of the refusal.
        code: ResponseCode,
    },
    /// No answer within the budget.
    Timeout,
    /// The connection is closed.
    Closed,
}

impl core::fmt::Display for RtcpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Framing(e) => write!(f, "{e}"),
            Self::NotRtcp { got } => write!(f, "rtcp message does not begin with RTCP: {got:?}"),
            Self::LengthTooSmall { announced } => {
                write!(f, "rtcp header states {announced} octets, less than itself")
            }
            Self::CrcMismatch { stated, computed } => write!(
                f,
                "rtcp checksum {stated:#010x} is not the sum {computed:#010x} of the message"
            ),
            Self::NoBindRequest => f.write_str("rtcp connection without a BindConnectionRequest"),
            Self::DataBeforeBind => f.write_str("rtps message before the rtcp bind"),
            Self::ShortControl { len } => {
                write!(
                    f,
                    "rtcp control message of {len} octets, less than its header"
                )
            }
            Self::BindRefused(code) => write!(f, "rtcp bind refused by the peer: {code:?}"),
            Self::BindDeclined(code) => write!(f, "rtcp bind of the peer declined: {code:?}"),
            Self::PortRefused { port, code } => {
                write!(f, "rtcp logical port {port} refused by the peer: {code:?}")
            }
            Self::Timeout => f.write_str("rtcp peer did not answer in time"),
            Self::Closed => f.write_str("rtcp connection closed"),
        }
    }
}

impl std::error::Error for RtcpError {}

/// What a side of a connection is: the logical ports it has open, the
/// locator of its listener and its vendor id.
#[derive(Debug, Clone)]
pub(crate) struct Own {
    /// The logical ports an OpenLogicalPortRequest is granted for.
    pub ports: Vec<u16>,
    /// The physical port of this side's listener.
    pub listener_port: u16,
    /// The vendor id of the BindConnectionRequest.
    pub vendor_id: [u8; 2],
}

/// Counts of a transport's connections of the dialect `fastdds`.
#[derive(Debug, Default)]
pub(crate) struct Counters {
    /// RTPS messages for a logical port this side has not open; each was
    /// answered with a LogicalPortIsClosedRequest.
    pub closed_port_messages: AtomicU64,
    /// Responses to no request of this side.
    pub unmatched_responses: AtomicU64,
    /// Logical ports a peer refused to open.
    pub refused_ports: AtomicU64,
    /// Control messages answered with `BadRequest`.
    pub bad_requests: AtomicU64,
}

#[derive(Debug, Default)]
struct LinkState {
    established: bool,
    closed: bool,
    next_transaction: u32,
    /// The transaction of this side's BindConnectionRequest and its
    /// answer.
    bind: Option<([u8; 12], Option<ResponseCode>)>,
    /// Logical ports the peer has open for this side.
    opened: BTreeSet<u16>,
    /// OpenLogicalPortRequests of this side not yet answered.
    asked: BTreeMap<[u8; 12], u16>,
    /// Logical ports the peer refused, with the time and the code.
    refused: BTreeMap<u16, (Instant, ResponseCode)>,
}

/// One connection of the dialect `fastdds`, shared by the thread that
/// reads it and the senders that write on it.
#[derive(Debug)]
pub(crate) struct Link {
    writer: Mutex<TcpStream>,
    state: Mutex<LinkState>,
    cv: Condvar,
    own: Own,
    /// Whether this side accepted the connection.
    accepted: bool,
}

/// What the read loop does after a message.
enum Flow {
    Next,
    End,
}

impl Link {
    fn new(stream: &TcpStream, own: Own, accepted: bool) -> std::io::Result<Self> {
        Ok(Self {
            writer: Mutex::new(stream.try_clone()?),
            state: Mutex::new(LinkState::default()),
            cv: Condvar::new(),
            own,
            accepted,
        })
    }

    /// The stream the link writes on.
    pub(crate) fn with_stream<T>(&self, f: impl FnOnce(&TcpStream) -> T) -> Option<T> {
        self.writer.lock().ok().map(|s| f(&s))
    }

    fn transaction(&self, st: &mut LinkState) -> [u8; 12] {
        st.next_transaction = st.next_transaction.wrapping_add(1);
        let mut id = [0u8; 12];
        id[..4].copy_from_slice(&st.next_transaction.to_le_bytes());
        id
    }

    fn write(&self, octets: &[u8]) -> Result<(), RtcpError> {
        let mut stream = self.writer.lock().map_err(|_| RtcpError::Closed)?;
        stream
            .write_all(octets)
            .map_err(|e| RtcpError::Framing(FramingError::Io(e)))
    }

    fn write_control(
        &self,
        kind: u8,
        transaction_id: [u8; 12],
        code: Option<ResponseCode>,
        payload: Option<Vec<u8>>,
    ) -> Result<(), RtcpError> {
        let message = ControlMessage {
            kind,
            transaction_id,
            code,
            payload,
        }
        .encode()
        .map_err(RtcpError::Framing)?;
        self.write(&message)
    }

    /// The locator of this side, as a BindConnectionRequest or response
    /// names it: the local address of the connection, the port of this
    /// side's listener, no WAN address.
    fn own_locator(&self, local: SocketAddr) -> WireLocator {
        WireLocator::of_socket(SocketAddr::new(local.ip(), self.own.listener_port))
    }

    /// Opens the connection `stream` this side made: the
    /// BindConnectionRequest and its answer, before `deadline`. Returns
    /// the link and the reader the thread that reads the connection goes
    /// on with.
    pub(crate) fn connect(
        stream: &TcpStream,
        own: Own,
        counters: &Counters,
        deadline: Instant,
    ) -> Result<(Self, std::io::BufReader<TcpStream>), RtcpError> {
        let io = |e: std::io::Error| RtcpError::Framing(FramingError::Io(e));
        let local = stream.local_addr().map_err(io)?;
        let link = Self::new(stream, own, false).map_err(io)?;
        let mut reader = std::io::BufReader::new(stream.try_clone().map_err(io)?);
        let request = ConnectionRequest {
            protocol_version: PROTOCOL_VERSION,
            vendor_id: link.own.vendor_id,
            transport_locator: link.own_locator(local),
        };
        let id = {
            let mut st = link.state.lock().map_err(|_| RtcpError::Closed)?;
            let id = link.transaction(&mut st);
            st.bind = Some((id, None));
            id
        };
        link.write_control(
            kind::BIND_CONNECTION_REQUEST,
            id,
            None,
            Some(encode_connection_request(&request)),
        )?;
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(RtcpError::Timeout)?;
            stream.set_read_timeout(Some(left)).map_err(io)?;
            let message = read_message(&mut reader).map_err(|e| match e {
                RtcpError::Framing(FramingError::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    RtcpError::Timeout
                }
                other => other,
            })?;
            if message.header.logical_port != 0 {
                return Err(RtcpError::DataBeforeBind);
            }
            if let Flow::End = link.control(&message.body, counters)? {
                return Err(RtcpError::Closed);
            }
            let answer = link
                .state
                .lock()
                .map_err(|_| RtcpError::Closed)?
                .bind
                .and_then(|(_, answer)| answer);
            match answer {
                None => {}
                Some(ResponseCode::Ok | ResponseCode::ExistingConnection) => break,
                Some(code) => return Err(RtcpError::BindRefused(code)),
            }
        }
        stream.set_read_timeout(None).map_err(io)?;
        Ok((link, reader))
    }

    /// Opens the connection `stream` this side accepted, whose first four
    /// octets, `RTCP`, have been read: the peer's BindConnectionRequest
    /// before `deadline` and the answer. `decide` gives the code for the
    /// request; a refused request is answered with its code and ends in
    /// [`RtcpError::BindDeclined`]. Returns the link, the reader and the
    /// request.
    pub(crate) fn accept(
        stream: &TcpStream,
        own: Own,
        deadline: Instant,
        decide: impl FnOnce(&ConnectionRequest) -> ResponseCode,
    ) -> Result<(Self, std::io::BufReader<TcpStream>, ConnectionRequest), RtcpError> {
        let io = |e: std::io::Error| RtcpError::Framing(FramingError::Io(e));
        let local = stream.local_addr().map_err(io)?;
        let mut reader = std::io::BufReader::new(stream.try_clone().map_err(io)?);
        let left = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(RtcpError::Timeout)?;
        stream.set_read_timeout(Some(left)).map_err(io)?;
        let mut head = [0u8; TCP_HEADER_LEN];
        head[..4].copy_from_slice(&MAGIC);
        let read = read_exact_or_eof(&mut reader, &mut head[4..])
            .map_err(RtcpError::Framing)
            .and_then(|()| read_after_head(&head, &mut reader));
        let message = read.map_err(|e| match e {
            RtcpError::Framing(FramingError::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                RtcpError::Timeout
            }
            other => other,
        })?;
        if message.header.logical_port != 0 {
            return Err(RtcpError::DataBeforeBind);
        }
        let control =
            ControlMessage::decode(&message.body).map_err(|_| RtcpError::NoBindRequest)?;
        if control.kind != kind::BIND_CONNECTION_REQUEST {
            return Err(RtcpError::NoBindRequest);
        }
        let request = control
            .payload
            .as_deref()
            .and_then(decode_connection_request)
            .ok_or(RtcpError::NoBindRequest)?;
        let code = if request.protocol_version == PROTOCOL_VERSION {
            decide(&request)
        } else {
            ResponseCode::IncompatibleVersion
        };
        let link = Self::new(stream, own, true).map_err(io)?;
        link.write_control(
            kind::BIND_CONNECTION_RESPONSE,
            control.transaction_id,
            Some(code),
            Some(encode_locator(&WireLocator::of_socket(local))),
        )?;
        if code != ResponseCode::Ok {
            return Err(RtcpError::BindDeclined(code));
        }
        link.state
            .lock()
            .map_err(|_| RtcpError::Closed)?
            .established = true;
        stream.set_read_timeout(None).map_err(io)?;
        Ok((link, reader, request))
    }

    /// Reads the connection until it ends: control messages are answered,
    /// each RTPS message for an open logical port of this side goes to
    /// `deliver` with that port. Returns `Ok` for the regular end (the
    /// peer closed the connection between messages or sent an
    /// UnbindConnectionRequest).
    pub(crate) fn run<R: Read>(
        &self,
        reader: &mut R,
        counters: &Counters,
        mut deliver: impl FnMut(u16, Vec<u8>),
    ) -> Result<(), RtcpError> {
        let result = loop {
            let message = match read_message(reader) {
                Ok(m) => m,
                Err(RtcpError::Framing(FramingError::Closed)) => break Ok(()),
                Err(e) => break Err(e),
            };
            if message.header.logical_port == 0 {
                match self.control(&message.body, counters) {
                    Ok(Flow::Next) => continue,
                    Ok(Flow::End) => break Ok(()),
                    Err(e) => break Err(e),
                }
            }
            let port = message.header.logical_port;
            if self.own.ports.contains(&port) {
                deliver(port, message.body);
            } else {
                // Fast DDS drops such a message with a log line. Here the
                // peer is told that the port is closed, which makes Fast
                // DDS ask for it again, and the message is counted.
                counters
                    .closed_port_messages
                    .fetch_add(1, Ordering::Relaxed);
                let id = match self.state.lock() {
                    Ok(mut st) => self.transaction(&mut st),
                    Err(_) => break Err(RtcpError::Closed),
                };
                if let Err(e) = self.write_control(
                    kind::LOGICAL_PORT_IS_CLOSED_REQUEST,
                    id,
                    None,
                    Some(encode_port(port)),
                ) {
                    break Err(e);
                }
            }
        };
        self.mark_closed();
        result
    }

    /// Takes one control message.
    fn control(&self, body: &[u8], counters: &Counters) -> Result<Flow, RtcpError> {
        let message = match ControlMessage::decode(body) {
            Ok(m) => m,
            Err(Some(bad)) => {
                counters.bad_requests.fetch_add(1, Ordering::Relaxed);
                self.write_control(
                    bad.kind,
                    bad.transaction_id,
                    Some(ResponseCode::BadRequest),
                    None,
                )?;
                return Ok(Flow::Next);
            }
            // Shorter than a control header: nothing to answer to.
            Err(None) => return Err(RtcpError::ShortControl { len: body.len() }),
        };
        let id = message.transaction_id;
        let mut st = self.state.lock().map_err(|_| RtcpError::Closed)?;
        let established = st.established;
        match message.kind {
            kind::BIND_CONNECTION_REQUEST => {
                drop(st);
                // The connection is bound once; Fast DDS answers a second
                // request so.
                let code = if established {
                    ResponseCode::ExistingConnection
                } else {
                    ResponseCode::ServerError
                };
                self.write_control(kind::BIND_CONNECTION_RESPONSE, id, Some(code), None)?;
            }
            kind::BIND_CONNECTION_RESPONSE => match st.bind {
                Some((asked, None)) if asked == id && !self.accepted => {
                    let code = message.code.unwrap_or(ResponseCode::BadRequest);
                    st.bind = Some((asked, Some(code)));
                    st.established =
                        matches!(code, ResponseCode::Ok | ResponseCode::ExistingConnection);
                    self.cv.notify_all();
                }
                _ => {
                    counters.unmatched_responses.fetch_add(1, Ordering::Relaxed);
                }
            },
            kind::OPEN_LOGICAL_PORT_REQUEST => {
                drop(st);
                let port = message.payload.as_deref().and_then(decode_port);
                let code = match port {
                    None => ResponseCode::BadRequest,
                    Some(_) if !established => ResponseCode::ServerError,
                    Some(port) if port != 0 && self.own.ports.contains(&port) => ResponseCode::Ok,
                    Some(_) => ResponseCode::InvalidPort,
                };
                self.write_control(kind::OPEN_LOGICAL_PORT_RESPONSE, id, Some(code), None)?;
            }
            kind::OPEN_LOGICAL_PORT_RESPONSE => match st.asked.remove(&id) {
                Some(port) => {
                    match message.code {
                        Some(ResponseCode::Ok) => {
                            st.refused.remove(&port);
                            st.opened.insert(port);
                        }
                        code => {
                            counters.refused_ports.fetch_add(1, Ordering::Relaxed);
                            st.refused.insert(
                                port,
                                (Instant::now(), code.unwrap_or(ResponseCode::BadRequest)),
                            );
                        }
                    }
                    self.cv.notify_all();
                }
                None => {
                    counters.unmatched_responses.fetch_add(1, Ordering::Relaxed);
                }
            },
            kind::CHECK_LOGICAL_PORT_REQUEST => {
                drop(st);
                match message.payload.as_deref().and_then(decode_ports) {
                    None => self.write_control(
                        kind::CHECK_LOGICAL_PORT_RESPONSE,
                        id,
                        Some(ResponseCode::BadRequest),
                        None,
                    )?,
                    Some(_) if !established => self.write_control(
                        kind::CHECK_LOGICAL_PORT_RESPONSE,
                        id,
                        Some(ResponseCode::ServerError),
                        None,
                    )?,
                    Some(asked) => {
                        let open: Vec<u16> = asked
                            .into_iter()
                            .filter(|port| self.own.ports.contains(port))
                            .collect();
                        self.write_control(
                            kind::CHECK_LOGICAL_PORT_RESPONSE,
                            id,
                            Some(ResponseCode::Ok),
                            Some(encode_ports(&open)),
                        )?;
                    }
                }
            }
            kind::KEEP_ALIVE_REQUEST => {
                drop(st);
                // Fast DDS compares the logical port of the request's
                // locator with the one of the connection's locator, which
                // is a physical locator (logical port 0).
                let code = match message.payload.as_deref().and_then(decode_locator) {
                    None => ResponseCode::BadRequest,
                    Some(_) if !established => ResponseCode::ServerError,
                    Some(locator) if locator.logical_port() == 0 => ResponseCode::Ok,
                    Some(_) => ResponseCode::UnknownLocator,
                };
                self.write_control(kind::KEEP_ALIVE_RESPONSE, id, Some(code), None)?;
            }
            kind::LOGICAL_PORT_IS_CLOSED_REQUEST => {
                // The next send to the port asks for it again.
                if let Some(port) = message.payload.as_deref().and_then(decode_port) {
                    st.opened.remove(&port);
                    st.refused.remove(&port);
                } else {
                    drop(st);
                    counters.bad_requests.fetch_add(1, Ordering::Relaxed);
                    self.write_control(message.kind, id, Some(ResponseCode::BadRequest), None)?;
                }
            }
            kind::UNBIND_CONNECTION_REQUEST => {
                return Ok(Flow::End);
            }
            kind::CHECK_LOGICAL_PORT_RESPONSE | kind::KEEP_ALIVE_RESPONSE => {
                // This side asks neither.
                counters.unmatched_responses.fetch_add(1, Ordering::Relaxed);
            }
            other => {
                drop(st);
                // Fast DDS answers an unknown kind with that kind and
                // BadRequest.
                counters.bad_requests.fetch_add(1, Ordering::Relaxed);
                self.write_control(other, id, Some(ResponseCode::BadRequest), None)?;
            }
        }
        Ok(Flow::Next)
    }

    /// Writes the RTPS message `data` for the logical port `port` of the
    /// peer. A port the peer has not opened for this side is asked for
    /// first, and the answer awaited until `budget` has passed.
    pub(crate) fn send(&self, port: u16, data: &[u8], budget: Duration) -> Result<(), RtcpError> {
        let message = data_message(port, data).map_err(RtcpError::Framing)?;
        let deadline = Instant::now() + budget;
        let mut st = self.state.lock().map_err(|_| RtcpError::Closed)?;
        loop {
            if st.closed {
                return Err(RtcpError::Closed);
            }
            if st.opened.contains(&port) {
                break;
            }
            if let Some(&(at, code)) = st.refused.get(&port) {
                if at.elapsed() < REFUSED_PORT_HOLD {
                    return Err(RtcpError::PortRefused { port, code });
                }
                st.refused.remove(&port);
            }
            if !st.asked.values().any(|asked| *asked == port) {
                let id = self.transaction(&mut st);
                st.asked.insert(id, port);
                drop(st);
                self.write_control(
                    kind::OPEN_LOGICAL_PORT_REQUEST,
                    id,
                    None,
                    Some(encode_port(port)),
                )?;
                st = self.state.lock().map_err(|_| RtcpError::Closed)?;
                continue;
            }
            let left = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(RtcpError::Timeout)?;
            st = self
                .cv
                .wait_timeout(st, left)
                .map_err(|_| RtcpError::Closed)?
                .0;
        }
        drop(st);
        self.write(&message)
    }

    fn mark_closed(&self) {
        if let Ok(mut st) = self.state.lock() {
            st.closed = true;
        }
        self.cv.notify_all();
    }

    /// Ends the connection: an UnbindConnectionRequest, as Fast DDS sends
    /// it when its transport closes, then the shutdown.
    pub(crate) fn unbind(&self) {
        let id = match self.state.lock() {
            Ok(mut st) => self.transaction(&mut st),
            Err(_) => [0u8; 12],
        };
        let _ = self.write_control(kind::UNBIND_CONNECTION_REQUEST, id, None, None);
        self.shutdown();
    }

    /// Ends the connection without an UnbindConnectionRequest.
    pub(crate) fn shutdown(&self) {
        self.mark_closed();
        let _ = self.with_stream(|s| s.shutdown(std::net::Shutdown::Both));
    }

    /// Whether the link is closed.
    pub(crate) fn is_closed(&self) -> bool {
        self.state.lock().map(|st| st.closed).unwrap_or(true)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cyclone_stream::tests::unhex;
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::Arc;

    /// What a Fast DDS 3.6.1 participant (`fastdds_shapes sub … tcp 0
    /// 127.0.0.1:<port>`, no listening port) wrote first on its
    /// connection: the BindConnectionRequest.
    const FASTDDS_BIND_REQUEST: &str =
        include_str!("../tests/data/fastdds_3_6_1_rtcp_bind_request.hex");

    /// What a Fast DDS 3.6.1 listener (`fastdds_shapes sub … tcp 47201`,
    /// domain 7) answered to a BindConnectionRequest and to the
    /// OpenLogicalPortRequests for 9160 (its metatraffic unicast port) and
    /// for 1.
    const FASTDDS_LISTENER_ANSWERS: &str =
        include_str!("../tests/data/fastdds_3_6_1_rtcp_listener_answers.hex");

    fn own(ports: &[u16], listener_port: u16) -> Own {
        Own {
            ports: ports.to_vec(),
            listener_port,
            vendor_id: [0x01, 0xF0],
        }
    }

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    #[test]
    fn tcp_6_1_rtcp_header_layout() {
        let octets = unhex(FASTDDS_BIND_REQUEST);
        let head: [u8; TCP_HEADER_LEN] = octets[..TCP_HEADER_LEN].try_into().unwrap();
        let header = TcpHeader::decode(&head).unwrap();
        assert_eq!(header.length as usize, octets.len());
        assert_eq!(header.logical_port, 0);
        assert_eq!(header.crc, crc(&octets[TCP_HEADER_LEN..]));
        // The encoder writes what Fast DDS wrote.
        assert_eq!(
            TcpHeader::of(&octets[TCP_HEADER_LEN..], 0)
                .unwrap()
                .encode(),
            head
        );
    }

    #[test]
    fn tcp_6_1_the_crc_adds_the_carry_back_in() {
        assert_eq!(crc(&[]), 0);
        assert_eq!(crc(&[1, 2, 3]), 6);
        // 0xFFFF_FFFF + 1 overflows: the carry comes back as 1.
        let ones = vec![0xFFu8; 0x0101_0102];
        let plain: u64 = ones.iter().map(|b| u64::from(*b)).sum();
        let folded = (plain & 0xFFFF_FFFF) + (plain >> 32);
        assert_eq!(u64::from(crc(&ones)), folded);
    }

    #[test]
    fn tcp_6_1_bind_connection_request_layout() {
        let octets = unhex(FASTDDS_BIND_REQUEST);
        let control = ControlMessage::decode(&octets[TCP_HEADER_LEN..]).unwrap();
        assert_eq!(control.kind, kind::BIND_CONNECTION_REQUEST);
        assert_eq!(octets[TCP_HEADER_LEN + 1], 0x0E, "flags of a request");
        assert_eq!(control.code, None);
        let request = decode_connection_request(control.payload.as_deref().unwrap()).unwrap();
        assert_eq!(request.protocol_version, PROTOCOL_VERSION);
        assert_eq!(request.vendor_id, [0x01, 0x0F], "eProsima");
        assert_eq!(request.transport_locator.kind, LOCATOR_KIND_TCPV4);
        assert_eq!(&request.transport_locator.address[12..], &[127, 0, 0, 1]);
        assert_eq!(request.transport_locator.logical_port(), 0);
        // This module writes the same octets for the same request.
        let again = ControlMessage {
            kind: kind::BIND_CONNECTION_REQUEST,
            transaction_id: control.transaction_id,
            code: None,
            payload: Some(encode_connection_request(&request)),
        }
        .encode()
        .unwrap();
        assert_eq!(again, octets);
    }

    #[test]
    fn tcp_6_1_the_listener_answers_of_fast_dds_decode() {
        let octets = unhex(FASTDDS_LISTENER_ANSWERS);
        let mut cursor = std::io::Cursor::new(&octets);
        let bind = read_message(&mut cursor).unwrap();
        let bind = ControlMessage::decode(&bind.body).unwrap();
        assert_eq!(bind.kind, kind::BIND_CONNECTION_RESPONSE);
        assert_eq!(bind.code, Some(ResponseCode::Ok));
        assert_eq!(bind.transaction_id[0], 7);
        let locator = decode_locator(bind.payload.as_deref().unwrap()).unwrap();
        assert_eq!(locator.physical_port(), 47201);
        assert_eq!(locator.kind, LOCATOR_KIND_TCPV4);
        let open = ControlMessage::decode(&read_message(&mut cursor).unwrap().body).unwrap();
        assert_eq!(open.kind, kind::OPEN_LOGICAL_PORT_RESPONSE);
        assert_eq!(open.code, Some(ResponseCode::Ok));
        assert_eq!(open.payload, None);
        let refused = ControlMessage::decode(&read_message(&mut cursor).unwrap().body).unwrap();
        assert_eq!(refused.code, Some(ResponseCode::InvalidPort));
        assert_eq!(refused.transaction_id[0], 9);
        // This module writes Fast DDS's answers octet for octet.
        let mut again = ControlMessage {
            kind: kind::BIND_CONNECTION_RESPONSE,
            transaction_id: bind.transaction_id,
            code: Some(ResponseCode::Ok),
            payload: Some(encode_locator(&locator)),
        }
        .encode()
        .unwrap();
        for answer in [&open, &refused] {
            again.extend(answer.encode().unwrap());
        }
        assert_eq!(again, octets);
        assert!(matches!(
            read_message(&mut cursor),
            Err(RtcpError::Framing(FramingError::Closed))
        ));
    }

    #[test]
    fn tcp_6_1_a_bad_crc_is_refused() {
        let mut octets = data_message(7410, b"RTPS-message").unwrap();
        // A checksum of 0 is one Fast DDS writes without `calculate_crc`.
        octets[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_message(&mut std::io::Cursor::new(&octets)).is_ok());
        octets[8..12].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(
            read_message(&mut std::io::Cursor::new(&octets)),
            Err(RtcpError::CrcMismatch { stated: 1, .. })
        ));
    }

    #[test]
    fn tcp_6_1_a_stream_without_rtcp_is_refused() {
        assert!(matches!(
            read_message(&mut std::io::Cursor::new(b"RTPS\x02\x05\x01\x0f..........")),
            Err(RtcpError::NotRtcp { got }) if &got == b"RTPS"
        ));
        let mut short = data_message(1, b"x").unwrap();
        short[4..8].copy_from_slice(&13u32.to_le_bytes());
        assert!(matches!(
            read_message(&mut std::io::Cursor::new(&short)),
            Err(RtcpError::LengthTooSmall { announced: 13 })
        ));
        let cut = data_message(1, b"abcdef").unwrap();
        assert!(matches!(
            read_message(&mut std::io::Cursor::new(&cut[..cut.len() - 1])),
            Err(RtcpError::Framing(FramingError::UnexpectedEof))
        ));
    }

    #[test]
    fn tcp_6_1_a_control_message_of_a_wrong_length_is_answered_bad_request() {
        let (client, server) = pair();
        let counters = Counters::default();
        let link = Link::new(&server, own(&[7410], 1), true).unwrap();
        link.state.lock().unwrap().established = true;
        let mut message = ControlMessage {
            kind: kind::OPEN_LOGICAL_PORT_REQUEST,
            transaction_id: [5; 12],
            code: None,
            payload: Some(encode_port(7410)),
        }
        .encode()
        .unwrap();
        // The control length states two octets more than there are.
        let stated = u16::from_le_bytes([message[16], message[17]]) + 2;
        message[16..18].copy_from_slice(&stated.to_le_bytes());
        let body = message[TCP_HEADER_LEN..].to_vec();
        assert!(matches!(link.control(&body, &counters), Ok(Flow::Next)));
        // Fast DDS answers with the kind of the request, its transaction
        // id and the code BadRequest behind the control header.
        let answer = read_message(&mut &client).unwrap().body;
        assert_eq!(answer[0], kind::OPEN_LOGICAL_PORT_REQUEST);
        assert_eq!(&answer[4..16], &[5; 12]);
        assert_eq!(&answer[16..], &1u32.to_le_bytes());
        assert_eq!(counters.bad_requests.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tcp_6_1_the_locator_port_splits_into_physical_and_logical() {
        let locator = WireLocator {
            kind: LOCATOR_KIND_TCPV4,
            port: (7410 << 16) | 5100,
            address: [0; 16],
        };
        assert_eq!(locator.physical_port(), 5100);
        assert_eq!(locator.logical_port(), 7410);
        let of_socket = WireLocator::of_socket("10.1.2.3:5100".parse().unwrap());
        assert_eq!(of_socket.port, 5100);
        assert_eq!(&of_socket.address[12..], &[10, 1, 2, 3]);
        assert_eq!(&of_socket.address[..12], &[0; 12]);
        let v6 = WireLocator::of_socket("[2001:db8::1]:5100".parse().unwrap());
        assert_eq!(v6.kind, LOCATOR_KIND_TCPV6);
    }

    #[test]
    fn tcp_6_1_the_cdr_payloads_round_trip() {
        assert_eq!(decode_port(&encode_port(7410)), Some(7410));
        assert_eq!(
            decode_ports(&encode_ports(&[7410, 7412])),
            Some(vec![7410, 7412])
        );
        assert_eq!(decode_ports(&encode_ports(&[])), Some(vec![]));
        // A count above what the data holds is no sequence.
        let mut long = encode_ports(&[1]);
        long[4..8].copy_from_slice(&1000u32.to_le_bytes());
        assert_eq!(decode_ports(&long), None);
        // Big-endian CDR is read as such.
        assert_eq!(
            decode_port(&[0x00, 0x00, 0x00, 0x00, 0x1C, 0xF2]),
            Some(7410)
        );
        assert_eq!(decode_port(&[0x00, 0x02, 0x00, 0x00, 0x1C, 0xF2]), None);
    }

    /// Two links over loopback, bound to each other as a connecting and
    /// an accepting side.
    fn bound_links(
        client_ports: &[u16],
        server_ports: &[u16],
    ) -> (Arc<Link>, Arc<Link>, Arc<Counters>, Arc<Counters>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let server_ports = server_ports.to_vec();
        let accepting = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut first = [0u8; 4];
            (&stream).read_exact(&mut first).unwrap();
            assert_eq!(first, MAGIC);
            let (link, reader, request) = Link::accept(
                &stream,
                own(&server_ports, addr.port()),
                Instant::now() + Duration::from_secs(2),
                |_| ResponseCode::Ok,
            )
            .unwrap();
            assert_eq!(request.vendor_id, [0x01, 0xF0]);
            // The connecting side names the port of its listener.
            assert_eq!(request.transport_locator.physical_port(), 4711);
            (Arc::new(link), reader)
        });
        let stream = TcpStream::connect(addr).unwrap();
        let client_counters = Arc::new(Counters::default());
        let (client, client_reader) = Link::connect(
            &stream,
            own(client_ports, 4711),
            &client_counters,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        let (server, server_reader) = accepting.join().unwrap();
        let client = Arc::new(client);
        let server_counters = Arc::new(Counters::default());
        for (link, mut reader, counters) in [
            (
                Arc::clone(&client),
                client_reader,
                Arc::clone(&client_counters),
            ),
            (
                Arc::clone(&server),
                server_reader,
                Arc::clone(&server_counters),
            ),
        ] {
            std::thread::spawn(move || {
                let _ = link.run(&mut reader, &counters, |_, _| {});
            });
        }
        (client, server, client_counters, server_counters)
    }

    #[test]
    fn tcp_6_1_open_logical_port_exchange() {
        let (client, server, client_counters, _) = bound_links(&[7411], &[7410]);
        // An open port: asked once, then written to.
        client.send(7410, b"first", Duration::from_secs(2)).unwrap();
        client
            .send(7410, b"second", Duration::from_secs(2))
            .unwrap();
        assert!(client.state.lock().unwrap().opened.contains(&7410));
        // A port the peer has not open is refused with InvalidPort, and
        // refused again at once within the hold.
        assert!(matches!(
            client.send(7412, b"x", Duration::from_secs(2)),
            Err(RtcpError::PortRefused {
                port: 7412,
                code: ResponseCode::InvalidPort
            })
        ));
        assert!(matches!(
            client.send(7412, b"x", Duration::from_secs(2)),
            Err(RtcpError::PortRefused { port: 7412, .. })
        ));
        assert_eq!(client_counters.refused_ports.load(Ordering::Relaxed), 1);
        // The accepting side asks the connecting one for its port.
        server
            .send(7411, b"answer", Duration::from_secs(2))
            .unwrap();
        assert!(matches!(
            server.send(7410, b"x", Duration::from_secs(2)),
            Err(RtcpError::PortRefused { port: 7410, .. })
        ));
    }

    #[test]
    fn tcp_6_1_the_messages_reach_the_reader_with_their_port() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let accepting = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut first = [0u8; 4];
            (&stream).read_exact(&mut first).unwrap();
            let (link, mut reader, _) = Link::accept(
                &stream,
                own(&[7410], addr.port()),
                Instant::now() + Duration::from_secs(2),
                |_| ResponseCode::Ok,
            )
            .unwrap();
            let counters = Counters::default();
            let ended = link.run(&mut reader, &counters, |port, data| {
                tx.send((port, data)).unwrap();
            });
            (
                ended.is_ok(),
                counters.closed_port_messages.load(Ordering::Relaxed),
            )
        });
        let stream = TcpStream::connect(addr).unwrap();
        let counters = Counters::default();
        let (client, mut reader) = Link::connect(
            &stream,
            own(&[], 1),
            &counters,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        let client = Arc::new(client);
        let reading = {
            let client = Arc::clone(&client);
            std::thread::spawn(move || {
                let counters = Counters::default();
                let _ = client.run(&mut reader, &counters, |_, _| {});
            })
        };
        client.send(7410, b"one", Duration::from_secs(2)).unwrap();
        client.send(7410, b"two", Duration::from_secs(2)).unwrap();
        // A message for a port not open there, written past the request
        // as by a sender that takes the port for opened: it is counted and
        // answered with LogicalPortIsClosed, which takes the port from the
        // opened ones of the sender.
        client.state.lock().unwrap().opened.insert(7999);
        let stray = data_message(7999, b"stray").unwrap();
        client.write(&stray).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while client.state.lock().unwrap().opened.contains(&7999) {
            assert!(
                Instant::now() < deadline,
                "no LogicalPortIsClosedRequest for 7999 reached the sender"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(client.state.lock().unwrap().opened.contains(&7410));
        client.unbind();
        let (ended_regularly, closed_port) = accepting.join().unwrap();
        assert!(ended_regularly, "an unbind is the regular end");
        assert_eq!(closed_port, 1);
        let got: Vec<_> = rx.try_iter().collect();
        assert_eq!(got, vec![(7410, b"one".to_vec()), (7410, b"two".to_vec())]);
        reading.join().unwrap();
    }

    #[test]
    fn tcp_6_1_a_closed_logical_port_is_reopened() {
        let (client, server, _, _) = bound_links(&[], &[7410]);
        client.send(7410, b"one", Duration::from_secs(2)).unwrap();
        // The accepting side says the port is closed.
        let id = {
            let mut st = server.state.lock().unwrap();
            server.transaction(&mut st)
        };
        server
            .write_control(
                kind::LOGICAL_PORT_IS_CLOSED_REQUEST,
                id,
                None,
                Some(encode_port(7410)),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while client.state.lock().unwrap().opened.contains(&7410) {
            assert!(Instant::now() < deadline, "the port stays opened");
            std::thread::sleep(Duration::from_millis(5));
        }
        // The next send asks for it again and is granted.
        client.send(7410, b"two", Duration::from_secs(2)).unwrap();
        assert!(client.state.lock().unwrap().opened.contains(&7410));
    }

    #[test]
    fn tcp_6_1_keep_alive_and_check_logical_ports_are_answered() {
        let ask = |kind: u8, tx: u8, payload: Vec<u8>| {
            ControlMessage {
                kind,
                transaction_id: [tx; 12],
                code: None,
                payload: Some(payload),
            }
            .encode()
            .unwrap()
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let accepting = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut first = [0u8; 4];
            (&stream).read_exact(&mut first).unwrap();
            let (link, mut reader, _) = Link::accept(
                &stream,
                own(&[7410, 7412], addr.port()),
                Instant::now() + Duration::from_secs(2),
                |_| ResponseCode::Ok,
            )
            .unwrap();
            let counters = Counters::default();
            let _ = link.run(&mut reader, &counters, |_, _| {});
            counters.bad_requests.load(Ordering::Relaxed)
        });
        let mut stream = TcpStream::connect(addr).unwrap();
        let request = ConnectionRequest {
            protocol_version: PROTOCOL_VERSION,
            vendor_id: [0x01, 0x0F],
            transport_locator: WireLocator::of_socket("127.0.0.1:9".parse().unwrap()),
        };
        stream
            .write_all(
                &ControlMessage {
                    kind: kind::BIND_CONNECTION_REQUEST,
                    transaction_id: [0; 12],
                    code: None,
                    payload: Some(encode_connection_request(&request)),
                }
                .encode()
                .unwrap(),
            )
            .unwrap();
        let bound = ControlMessage::decode(&read_message(&mut stream).unwrap().body).unwrap();
        assert_eq!(bound.code, Some(ResponseCode::Ok));
        stream
            .write_all(&ask(
                kind::KEEP_ALIVE_REQUEST,
                1,
                encode_locator(&WireLocator::of_socket("127.0.0.1:9".parse().unwrap())),
            ))
            .unwrap();
        let mut logical = WireLocator::of_socket("127.0.0.1:9".parse().unwrap());
        logical.port |= 7410 << 16;
        stream
            .write_all(&ask(kind::KEEP_ALIVE_REQUEST, 2, encode_locator(&logical)))
            .unwrap();
        stream
            .write_all(&ask(
                kind::CHECK_LOGICAL_PORT_REQUEST,
                3,
                encode_ports(&[7410, 7411, 7412]),
            ))
            .unwrap();
        stream.write_all(&ask(0xD9, 4, encode_port(1))).unwrap();
        let answers: Vec<ControlMessage> = (0..3)
            .map(|_| ControlMessage::decode(&read_message(&mut stream).unwrap().body).unwrap())
            .collect();
        // An unknown kind is answered with that kind, its transaction id
        // and BadRequest.
        let unknown = read_message(&mut stream).unwrap().body;
        assert_eq!(unknown[0], 0xD9);
        assert_eq!(&unknown[4..16], &[4; 12]);
        assert_eq!(&unknown[16..], &1u32.to_le_bytes());
        assert_eq!(answers[0].kind, kind::KEEP_ALIVE_RESPONSE);
        assert_eq!(answers[0].code, Some(ResponseCode::Ok));
        assert_eq!(answers[1].code, Some(ResponseCode::UnknownLocator));
        assert_eq!(answers[2].kind, kind::CHECK_LOGICAL_PORT_RESPONSE);
        assert_eq!(answers[2].code, Some(ResponseCode::Ok));
        assert_eq!(
            decode_ports(answers[2].payload.as_deref().unwrap()),
            Some(vec![7410, 7412])
        );
        stream.shutdown(std::net::Shutdown::Both).unwrap();
        assert_eq!(accepting.join().unwrap(), 1);
    }

    #[test]
    fn tcp_6_1_a_bind_of_another_version_is_refused() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let accepting = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut first = [0u8; 4];
            (&stream).read_exact(&mut first).unwrap();
            Link::accept(
                &stream,
                own(&[7410], addr.port()),
                Instant::now() + Duration::from_secs(2),
                |_| ResponseCode::Ok,
            )
            .map(|_| ())
        });
        let mut stream = TcpStream::connect(addr).unwrap();
        let request = ConnectionRequest {
            protocol_version: [2, 0],
            vendor_id: [0x01, 0x0F],
            transport_locator: WireLocator::of_socket("127.0.0.1:9".parse().unwrap()),
        };
        stream
            .write_all(
                &ControlMessage {
                    kind: kind::BIND_CONNECTION_REQUEST,
                    transaction_id: [3; 12],
                    code: None,
                    payload: Some(encode_connection_request(&request)),
                }
                .encode()
                .unwrap(),
            )
            .unwrap();
        let answer = ControlMessage::decode(&read_message(&mut stream).unwrap().body).unwrap();
        assert_eq!(answer.code, Some(ResponseCode::IncompatibleVersion));
        assert!(matches!(
            accepting.join().unwrap(),
            Err(RtcpError::BindDeclined(ResponseCode::IncompatibleVersion))
        ));
    }

    #[test]
    fn tcp_6_1_a_bind_without_an_answer_times_out() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_held, _) = listener.accept().unwrap();
        let started = Instant::now();
        let result = Link::connect(
            &stream,
            own(&[], 1),
            &Counters::default(),
            Instant::now() + Duration::from_millis(300),
        );
        assert!(matches!(result, Err(RtcpError::Timeout)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
