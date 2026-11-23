// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MIOP — GIOP request messages over UDP multicast, CORBA 3.3 Part 2
//! §11.1.
//!
//! ```text
//! module MIOP {
//!     typedef sequence<octet, 252> UniqueId;
//!     struct PacketHeader_1_0 {
//!         char           magic[4];          // "MIOP"
//!         octet          hdr_version;       // 0x10 = 1.0
//!         octet          flags;             // bit 0 little endian, bit 1 stop
//!         unsigned short packet_length;     // octets of GIOP data in this packet
//!         unsigned long  packet_number;     // 0 .. n-1
//!         unsigned long  number_of_packets; // n, or 0 when not used
//!         UniqueId       id;                // the same in every packet of a collection
//!     };                                    // GIOP data from the next 8-octet boundary
//! };
//! ```
//!
//! A GIOP request (with its fragments) is sent as one packet collection:
//! every packet but the last carries the same number of octets, the last
//! has the stop flag. A receiver strips the headers and concatenates the
//! data of a collection — packets may arrive in any order — and drops a
//! collection whose stop packet arrives while packets are missing or
//! that stays incomplete beyond a timeout (§11.1.6.2). The header fields
//! are in the byte order of the endian flag; alignment counts from the
//! first octet of the packet.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use zerodds_wire::{Endianness, Reader, Writer};

/// `"MIOP"` (§11.1.4.1.1).
pub const MIOP_MAGIC: [u8; 4] = *b"MIOP";
/// `hdr_version` of MIOP 1.0: major in the high, minor in the low nibble.
pub const MIOP_HEADER_VERSION_1_0: u8 = 0x10;
/// The longest `UniqueId` (§11.1.4.1.7).
pub const MAX_UNIQUE_ID_LEN: usize = 252;
/// Flag bit 0: the header is little endian.
pub const FLAG_LITTLE_ENDIAN: u8 = 0x01;
/// Flag bit 1: the last packet of a collection.
pub const FLAG_STOP: u8 = 0x02;
/// The largest UDP payload over IPv4 on a 1500-octet Ethernet frame.
pub const DEFAULT_MAX_DATAGRAM: usize = 1472;
/// The largest collection a receiver assembles (as the largest GIOP
/// message of `framing`).
pub const DEFAULT_MAX_COLLECTION: usize = 16 * 1024 * 1024;
/// How many incomplete collections a receiver keeps at a time.
pub const DEFAULT_MAX_PENDING: usize = 256;
/// How many octets the incomplete collections of a receiver hold
/// together, each packet counted with [`PACKET_OVERHEAD`]: four
/// collections of the largest size.
pub const DEFAULT_MAX_PENDING_BYTES: usize = 4 * DEFAULT_MAX_COLLECTION;
/// What a receiver counts for one packet it holds besides its data: its
/// entry in the collection and its buffer. A packet without data costs
/// memory too, so the limit of [`DEFAULT_MAX_PENDING_BYTES`] also bounds
/// a stream of empty packets.
pub const PACKET_OVERHEAD: usize = 64;

/// Why a packet or a collection was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiopError {
    /// Shorter than the fixed part of the header and the id length.
    TooShort,
    /// The magic is not `"MIOP"`.
    InvalidMagic([u8; 4]),
    /// `hdr_version` other than 1.0.
    UnsupportedVersion(u8),
    /// One of the six reserved flag bits is set.
    ReservedFlags(u8),
    /// A `UniqueId` longer than 252 octets.
    UniqueIdTooLong(usize),
    /// `packet_length` differs from the GIOP data the datagram carries.
    LengthMismatch {
        /// `packet_length`.
        declared: u16,
        /// Octets after the header.
        carried: usize,
    },
    /// `packet_number` not below `number_of_packets`.
    PacketNumberOutOfRange {
        /// `packet_number`.
        number: u32,
        /// `number_of_packets`.
        of: u32,
    },
    /// A packet whose number, length, byte order or count contradicts the
    /// packets already received for its collection; the collection is
    /// dropped.
    Inconsistent(&'static str),
    /// The same packet number twice in one collection.
    DuplicatePacket(u32),
    /// The stop packet arrived while packets were missing; the collection
    /// is dropped (§11.1.6.2).
    Incomplete {
        /// Packets received.
        received: usize,
        /// Packets the collection has: the stop packet's number plus one,
        /// up to 2^32.
        expected: u64,
    },
    /// The collection grew beyond the receiver's limit; dropped.
    CollectionTooLarge(usize),
    /// A message that cannot be sent: empty, or a datagram size that
    /// leaves no room for data after the header.
    CannotPacketize(&'static str),
}

impl fmt::Display for MiopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MIOP: {self:?}")
    }
}

impl core::error::Error for MiopError {}

/// `MIOP::PacketHeader_1_0` (§11.1.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    /// The byte order of the header (flag bit 0).
    pub endianness: Endianness,
    /// The stop flag: the last packet of the collection (flag bit 1).
    pub stop: bool,
    /// Octets of GIOP data in this packet, the header not counted.
    pub packet_length: u16,
    /// The number of this packet, from 0.
    pub packet_number: u32,
    /// The number of packets of the collection; 0 when not used.
    pub number_of_packets: u32,
    /// The `UniqueId` of the collection.
    pub id: Vec<u8>,
}

fn round8(n: usize) -> usize {
    (n + 7) & !7
}

impl PacketHeader {
    /// The octets of the header up to the 8-octet boundary where the GIOP
    /// data starts.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        round8(20 + self.id.len())
    }

    /// Encodes the header and the padding to the data.
    ///
    /// # Errors
    /// `UniqueIdTooLong` for an id over 252 octets.
    pub fn encode(&self) -> Result<Vec<u8>, MiopError> {
        if self.id.len() > MAX_UNIQUE_ID_LEN {
            return Err(MiopError::UniqueIdTooLong(self.id.len()));
        }
        let mut w = Writer::new(self.endianness);
        w.write_bytes(&MIOP_MAGIC);
        w.write_u8(MIOP_HEADER_VERSION_1_0);
        let mut flags = 0;
        if self.endianness == Endianness::Little {
            flags |= FLAG_LITTLE_ENDIAN;
        }
        if self.stop {
            flags |= FLAG_STOP;
        }
        w.write_u8(flags);
        w.write_u16(self.packet_length);
        w.write_u32(self.packet_number);
        w.write_u32(self.number_of_packets);
        // The id length fits u32: it is at most 252.
        w.write_u32(self.id.len() as u32);
        w.write_bytes(&self.id);
        let mut out = w.finish();
        out.resize(self.encoded_len(), 0);
        Ok(out)
    }

    /// Decodes the header of `datagram` and returns it with the GIOP data
    /// the packet carries.
    ///
    /// # Errors
    /// [`MiopError`] for a datagram that is not an MIOP 1.0 packet.
    pub fn decode(datagram: &[u8]) -> Result<(Self, &[u8]), MiopError> {
        if datagram.len() < 20 {
            return Err(MiopError::TooShort);
        }
        if datagram[..4] != MIOP_MAGIC {
            let mut m = [0u8; 4];
            m.copy_from_slice(&datagram[..4]);
            return Err(MiopError::InvalidMagic(m));
        }
        if datagram[4] != MIOP_HEADER_VERSION_1_0 {
            return Err(MiopError::UnsupportedVersion(datagram[4]));
        }
        let flags = datagram[5];
        if flags & !(FLAG_LITTLE_ENDIAN | FLAG_STOP) != 0 {
            return Err(MiopError::ReservedFlags(flags));
        }
        let endianness = if flags & FLAG_LITTLE_ENDIAN != 0 {
            Endianness::Little
        } else {
            Endianness::Big
        };
        let mut r = Reader::new(datagram, endianness);
        r.skip_to(6).map_err(|_| MiopError::TooShort)?;
        let short = |_| MiopError::TooShort;
        let packet_length = r.read_u16().map_err(short)?;
        let packet_number = r.read_u32().map_err(short)?;
        let number_of_packets = r.read_u32().map_err(short)?;
        let id_len = r.read_u32().map_err(short)? as usize;
        if id_len > MAX_UNIQUE_ID_LEN {
            return Err(MiopError::UniqueIdTooLong(id_len));
        }
        let id = r.read_bytes(id_len).map_err(short)?.to_vec();
        let data_start = round8(20 + id_len);
        if datagram.len() < data_start {
            return Err(MiopError::TooShort);
        }
        let data = &datagram[data_start..];
        if data.len() != usize::from(packet_length) {
            return Err(MiopError::LengthMismatch {
                declared: packet_length,
                carried: data.len(),
            });
        }
        if number_of_packets != 0 && packet_number >= number_of_packets {
            return Err(MiopError::PacketNumberOutOfRange {
                number: packet_number,
                of: number_of_packets,
            });
        }
        Ok((
            Self {
                endianness,
                stop: flags & FLAG_STOP != 0,
                packet_length,
                packet_number,
                number_of_packets,
                id,
            },
            data,
        ))
    }
}

/// Splits `message` (a GIOP request and its fragments) into the packets
/// of one collection with the id `id`, each packet at most
/// `max_datagram` octets: all packets but the last carry the same number
/// of octets, the last has the stop flag, `number_of_packets` is set.
///
/// # Errors
/// `CannotPacketize` for an empty message or a datagram size that leaves
/// no room for data; `UniqueIdTooLong`.
pub fn packetize(
    message: &[u8],
    id: &[u8],
    max_datagram: usize,
    endianness: Endianness,
) -> Result<Vec<Vec<u8>>, MiopError> {
    if message.is_empty() {
        return Err(MiopError::CannotPacketize("empty message"));
    }
    if id.len() > MAX_UNIQUE_ID_LEN {
        return Err(MiopError::UniqueIdTooLong(id.len()));
    }
    let header_len = round8(20 + id.len());
    let chunk = max_datagram
        .checked_sub(header_len)
        .filter(|c| *c > 0)
        .ok_or(MiopError::CannotPacketize(
            "no room for data after the header",
        ))?
        .min(usize::from(u16::MAX));
    let count = message.len().div_ceil(chunk);
    let number_of_packets =
        u32::try_from(count).map_err(|_| MiopError::CannotPacketize("more than 2^32 packets"))?;
    let mut out = Vec::with_capacity(count);
    for (i, data) in message.chunks(chunk).enumerate() {
        let header = PacketHeader {
            endianness,
            stop: i + 1 == count,
            // A chunk is at most u16::MAX octets.
            packet_length: data.len() as u16,
            packet_number: i as u32,
            number_of_packets,
            id: id.to_vec(),
        };
        let mut packet = header.encode()?;
        packet.extend_from_slice(data);
        out.push(packet);
    }
    Ok(out)
}

/// One collection being assembled.
#[derive(Debug)]
struct Collection<T> {
    packets: BTreeMap<u32, Vec<u8>>,
    endianness: Endianness,
    /// `packet_length` of the packets before the last.
    full_length: Option<u16>,
    /// The number of the stop packet, when it arrived.
    last: Option<u32>,
    /// `number_of_packets`, when the sender set it.
    count: Option<u32>,
    /// Octets of GIOP data.
    bytes: usize,
    /// What the collection holds: its data and [`PACKET_OVERHEAD`] per
    /// packet.
    held: usize,
    /// When the last packet of the collection arrived (§11.1.6.2: a
    /// collection times out when no further packets are incoming).
    last_packet: T,
}

/// What a receiver counted: every packet and collection it did not
/// deliver is in one of these counters (§11.1.6.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReassemblyStats {
    /// Collections delivered.
    pub delivered: u64,
    /// Datagrams that are no MIOP 1.0 packet.
    pub malformed: u64,
    /// Packets that repeat one already received.
    pub duplicates: u64,
    /// Collections dropped because their stop packet came with packets
    /// missing, or a packet contradicted them.
    pub incomplete: u64,
    /// Collections dropped because they stayed incomplete beyond the
    /// timeout or the receiver had too many pending, by number or by the
    /// octets they hold.
    pub timed_out: u64,
    /// Collections dropped for their size.
    pub too_large: u64,
}

/// Assembles packet collections (§11.1.3, §11.1.6.2), keyed by the
/// sender and the `UniqueId`. `T` is the clock (an `Instant`).
#[derive(Debug)]
pub struct Reassembler<K: Ord + Clone, T: Copy + Ord> {
    pending: BTreeMap<(K, Vec<u8>), Collection<T>>,
    max_collection: usize,
    max_pending: usize,
    max_pending_bytes: usize,
    /// What the pending collections hold together.
    held: usize,
    stats: ReassemblyStats,
}

impl<K: Ord + Clone, T: Copy + Ord> Reassembler<K, T> {
    /// A reassembler that assembles collections of at most
    /// `max_collection` octets and keeps at most `max_pending` incomplete
    /// ones (the one that has gone longest without a packet goes first,
    /// counted as timed out), holding at most
    /// [`DEFAULT_MAX_PENDING_BYTES`] together
    /// ([`Self::with_max_pending_bytes`]).
    #[must_use]
    pub fn new(max_collection: usize, max_pending: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            max_collection,
            max_pending: max_pending.max(1),
            max_pending_bytes: DEFAULT_MAX_PENDING_BYTES,
            held: 0,
            stats: ReassemblyStats::default(),
        }
    }

    /// The octets the incomplete collections may hold together, each
    /// packet counted with [`PACKET_OVERHEAD`]. When a packet takes them
    /// beyond, the other collections go, the one longest without a
    /// packet first, counted as timed out; a collection that alone holds
    /// more is dropped as too large.
    #[must_use]
    pub fn with_max_pending_bytes(mut self, max_pending_bytes: usize) -> Self {
        self.max_pending_bytes = max_pending_bytes;
        self
    }

    /// What the incomplete collections hold together: their data and
    /// [`PACKET_OVERHEAD`] per packet.
    #[must_use]
    pub fn pending_bytes(&self) -> usize {
        self.held
    }

    /// Drops the collection `key`, if pending.
    fn drop_collection(&mut self, key: &(K, Vec<u8>)) -> Option<Collection<T>> {
        let c = self.pending.remove(key)?;
        self.held -= c.held;
        Some(c)
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> ReassemblyStats {
        self.stats
    }

    /// The number of incomplete collections.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Takes one datagram from `sender` received at `now`. Returns the
    /// GIOP octets of a collection it completes.
    ///
    /// # Errors
    /// [`MiopError`] for a datagram that is not a packet, a duplicate, a
    /// packet that contradicts its collection, a stop packet with packets
    /// missing, or a collection over the size limit; the last three drop
    /// the collection.
    pub fn accept(
        &mut self,
        sender: &K,
        datagram: &[u8],
        now: T,
    ) -> Result<Option<Vec<u8>>, MiopError> {
        let (header, data) = match PacketHeader::decode(datagram) {
            Ok(x) => x,
            Err(e) => {
                self.stats.malformed += 1;
                return Err(e);
            }
        };
        let key = (sender.clone(), header.id.clone());
        if !self.pending.contains_key(&key) && self.pending.len() >= self.max_pending {
            let oldest = self
                .pending
                .iter()
                .min_by_key(|(_, c)| c.last_packet)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                self.drop_collection(&k);
                self.stats.timed_out += 1;
            }
        }
        let c = self
            .pending
            .entry(key.clone())
            .or_insert_with(|| Collection {
                packets: BTreeMap::new(),
                endianness: header.endianness,
                full_length: None,
                last: None,
                count: None,
                bytes: 0,
                held: 0,
                last_packet: now,
            });
        if let Err(e) = Self::check(c, &header) {
            if matches!(e, MiopError::DuplicatePacket(_)) {
                self.stats.duplicates += 1;
            } else {
                self.drop_collection(&key);
                self.stats.incomplete += 1;
            }
            return Err(e);
        }
        if !header.stop {
            c.full_length = Some(header.packet_length);
        }
        if header.number_of_packets != 0 {
            c.count = Some(header.number_of_packets);
        }
        if header.stop {
            c.last = Some(header.packet_number);
        }
        let cost = data.len() + PACKET_OVERHEAD;
        c.bytes += data.len();
        c.held += cost;
        c.last_packet = now;
        c.packets.insert(header.packet_number, data.to_vec());
        let (bytes, held, last, received) = (c.bytes, c.held, c.last, c.packets.len());
        self.held += cost;
        if bytes > self.max_collection || held > self.max_pending_bytes {
            self.drop_collection(&key);
            self.stats.too_large += 1;
            return Err(MiopError::CollectionTooLarge(bytes));
        }
        if let Some(last) = last {
            // §11.1.6.2: the stop packet is the end; what is missing then
            // will not be waited for.
            let expected = u64::from(last) + 1;
            if received as u64 != expected {
                self.drop_collection(&key);
                self.stats.incomplete += 1;
                return Err(MiopError::Incomplete { received, expected });
            }
            let c = self.drop_collection(&key).map(|c| c.packets);
            let mut out = Vec::new();
            for (_, data) in c.into_iter().flatten() {
                out.extend_from_slice(&data);
            }
            self.stats.delivered += 1;
            return Ok(Some(out));
        }
        // The collection stays pending. What all pending collections hold
        // stays within the limit: the others go, the one longest without
        // a packet first.
        while self.held > self.max_pending_bytes {
            let oldest = self
                .pending
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, c)| c.last_packet)
                .map(|(k, _)| k.clone());
            let Some(k) = oldest else {
                break;
            };
            self.drop_collection(&k);
            self.stats.timed_out += 1;
        }
        Ok(None)
    }

    fn check(c: &Collection<T>, h: &PacketHeader) -> Result<(), MiopError> {
        if c.packets.contains_key(&h.packet_number) {
            return Err(MiopError::DuplicatePacket(h.packet_number));
        }
        if h.endianness != c.endianness {
            return Err(MiopError::Inconsistent(
                "byte order differs within a collection",
            ));
        }
        if let Some(n) = c.count
            && h.number_of_packets != 0
            && h.number_of_packets != n
        {
            return Err(MiopError::Inconsistent(
                "number_of_packets differs within a collection",
            ));
        }
        // The number of packets, from this packet or an earlier one: a
        // packet that does not set it is still numbered below it.
        let count = c
            .count
            .or((h.number_of_packets != 0).then_some(h.number_of_packets));
        if let Some(n) = count
            && h.packet_number >= n
        {
            return Err(MiopError::PacketNumberOutOfRange {
                number: h.packet_number,
                of: n,
            });
        }
        // A count that arrives after packets without one holds for them.
        if c.count.is_none() && h.number_of_packets != 0 {
            let n = h.number_of_packets;
            if c.packets.keys().next_back().is_some_and(|m| *m >= n) {
                return Err(MiopError::Inconsistent(
                    "a packet number at or beyond number_of_packets",
                ));
            }
            if c.last.is_some_and(|l| u64::from(l) + 1 != u64::from(n)) {
                return Err(MiopError::Inconsistent("the stop packet is not packet n-1"));
            }
        }
        if h.stop {
            if c.last.is_some() {
                return Err(MiopError::Inconsistent("a second stop packet"));
            }
            if c.packets.keys().any(|n| *n > h.packet_number) {
                return Err(MiopError::Inconsistent("a packet after the stop packet"));
            }
            if let Some(n) = count
                && u64::from(h.packet_number) + 1 != u64::from(n)
            {
                return Err(MiopError::Inconsistent("the stop packet is not packet n-1"));
            }
            if let Some(full) = c.full_length
                && h.packet_length > full
            {
                return Err(MiopError::Inconsistent(
                    "the last packet is longer than the others",
                ));
            }
        } else {
            if let Some(last) = c.last
                && h.packet_number > last
            {
                return Err(MiopError::Inconsistent("a packet after the stop packet"));
            }
            if let Some(full) = c.full_length
                && h.packet_length != full
            {
                return Err(MiopError::Inconsistent(
                    "packets before the last differ in length",
                ));
            }
        }
        Ok(())
    }

    /// Drops the collections whose last packet arrived before `deadline`
    /// (§11.1.6.2: a partial collection with no further packets incoming
    /// is timed out; one whose packets still arrive is kept, however long
    /// it takes) and returns how many.
    pub fn expire(&mut self, deadline: T) -> usize {
        let before = self.pending.len();
        let mut released = 0;
        self.pending.retain(|_, c| {
            let keep = c.last_packet >= deadline;
            if !keep {
                released += c.held;
            }
            keep
        });
        self.held -= released;
        let dropped = before - self.pending.len();
        self.stats.timed_out += dropped as u64;
        dropped
    }
}

#[cfg(feature = "std")]
pub use net::*;

#[cfg(feature = "std")]
mod net {
    use super::{
        DEFAULT_MAX_COLLECTION, DEFAULT_MAX_DATAGRAM, DEFAULT_MAX_PENDING, MiopError, Reassembler,
        ReassemblyStats, packetize,
    };
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicU64, Ordering};
    use std::io;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
    use std::time::{Duration, Instant};
    use zerodds_wire::Endianness;

    /// A 16-octet `UniqueId` unique for the internet as a whole
    /// (§11.1.4.1.7): eight octets that differ between processes and
    /// starts — the process id, the start time and the first thread,
    /// through the random keys of the standard hasher — and a counter of
    /// the process.
    #[must_use]
    pub fn unique_id() -> Vec<u8> {
        use std::hash::{BuildHasher, Hasher};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        static SEED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        let seed = *SEED.get_or_init(|| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u32(std::process::id());
            if let Ok(t) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                h.write_u128(t.as_nanos());
            }
            h.write(alloc::format!("{:?}", std::thread::current().id()).as_bytes());
            h.finish()
        });
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut id = Vec::with_capacity(16);
        id.extend_from_slice(&seed.to_be_bytes());
        id.extend_from_slice(&n.to_be_bytes());
        id
    }

    /// Sends GIOP request messages as MIOP packet collections (§11.1.7).
    #[derive(Debug)]
    pub struct MiopSender {
        socket: UdpSocket,
        max_datagram: usize,
        endianness: Endianness,
    }

    impl MiopSender {
        /// A sender for groups of the address family of `family` whose
        /// datagrams cross at most `ttl` routers (IP_MULTICAST_TTL,
        /// IPV6_MULTICAST_HOPS; §11.1.6.1); it receives its own packets
        /// when a receiver on this host has joined the group.
        ///
        /// # Errors
        /// Socket errors.
        pub fn new(family: IpAddr, ttl: u32) -> io::Result<Self> {
            let domain = if family.is_ipv4() {
                socket2::Domain::IPV4
            } else {
                socket2::Domain::IPV6
            };
            let socket =
                socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
            if family.is_ipv4() {
                socket.set_multicast_ttl_v4(ttl)?;
                socket.set_multicast_loop_v4(true)?;
                socket.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0).into())?;
            } else {
                socket.set_multicast_hops_v6(ttl)?;
                socket.set_multicast_loop_v6(true)?;
                socket.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0).into())?;
            }
            Ok(Self {
                socket: socket.into(),
                max_datagram: DEFAULT_MAX_DATAGRAM,
                endianness: Endianness::Big,
            })
        }

        /// The largest datagram this sender writes (default 1472 octets).
        #[must_use]
        pub fn with_max_datagram(mut self, max_datagram: usize) -> Self {
            self.max_datagram = max_datagram;
            self
        }

        /// The byte order of the packet headers (default big endian).
        #[must_use]
        pub fn with_endianness(mut self, endianness: Endianness) -> Self {
            self.endianness = endianness;
            self
        }

        /// Sends `message` (a GIOP request and its fragments) to `group`
        /// as one collection with a fresh [`unique_id`]; returns the
        /// number of packets.
        ///
        /// # Errors
        /// `InvalidInput` for a message that cannot be packetized; socket
        /// errors.
        pub fn send(&self, group: SocketAddr, message: &[u8]) -> io::Result<usize> {
            let packets = packetize(message, &unique_id(), self.max_datagram, self.endianness)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            for p in &packets {
                self.socket.send_to(p, group)?;
            }
            Ok(packets.len())
        }
    }

    /// The interface a group is joined on.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum MulticastInterface {
        /// The interface the system chooses.
        #[default]
        Default,
        /// An IPv4 interface by its address (IPv4 groups only).
        V4(Ipv4Addr),
        /// An IPv6 interface by its index (IPv6 groups only).
        V6Index(u32),
    }

    /// A UDP socket bound to `port` that has joined `group` (§11.1.5) on
    /// `interface`. Several sockets of this host may join the same group
    /// and port.
    ///
    /// # Errors
    /// `InvalidInput` when `group` is not a multicast address or
    /// `interface` is of the other address family; socket errors.
    pub fn join_group(
        group: IpAddr,
        port: u16,
        interface: MulticastInterface,
    ) -> io::Result<UdpSocket> {
        if !group.is_multicast() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MIOP group address is not a multicast address",
            ));
        }
        let wrong_family = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the interface is of the other address family than the group",
            )
        };
        let domain = if group.is_ipv4() {
            socket2::Domain::IPV4
        } else {
            socket2::Domain::IPV6
        };
        let socket =
            socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
        socket.set_reuse_port(true)?;
        match group {
            IpAddr::V4(g) => {
                let iface = match interface {
                    MulticastInterface::Default => Ipv4Addr::UNSPECIFIED,
                    MulticastInterface::V4(a) => a,
                    MulticastInterface::V6Index(_) => return Err(wrong_family()),
                };
                // Bound to the group: datagrams of other groups on the
                // same port are not received here (Unix).
                #[cfg(unix)]
                socket.bind(&SocketAddr::new(IpAddr::V4(g), port).into())?;
                #[cfg(not(unix))]
                socket.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())?;
                socket.join_multicast_v4(&g, &iface)?;
            }
            IpAddr::V6(g) => {
                let index = match interface {
                    MulticastInterface::Default => 0,
                    MulticastInterface::V6Index(i) => i,
                    MulticastInterface::V4(_) => return Err(wrong_family()),
                };
                socket.set_only_v6(true)?;
                #[cfg(unix)]
                socket.bind(&SocketAddr::new(IpAddr::V6(g), port).into())?;
                #[cfg(not(unix))]
                socket.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port).into())?;
                socket.join_multicast_v6(&g, index)?;
            }
        }
        Ok(socket.into())
    }

    /// What one call of [`MiopReceiver::receive`] produced.
    #[derive(Debug)]
    pub enum Received {
        /// A complete collection: the GIOP octets and their sender.
        Collection(Vec<u8>, SocketAddr),
        /// A datagram that was refused, with its sender; the
        /// [`ReassemblyStats`] count it.
        Refused(MiopError, SocketAddr),
        /// Nothing arrived within the timeout.
        Idle,
    }

    /// Receives packet collections of the groups its socket joined.
    #[derive(Debug)]
    pub struct MiopReceiver {
        socket: UdpSocket,
        reassembler: Reassembler<SocketAddr, Instant>,
        collection_timeout: Duration,
    }

    impl MiopReceiver {
        /// A receiver on `group:port` (see [`join_group`]) that drops a
        /// collection still incomplete after `collection_timeout`.
        ///
        /// # Errors
        /// As [`join_group`].
        pub fn join(
            group: IpAddr,
            port: u16,
            interface: MulticastInterface,
            collection_timeout: Duration,
        ) -> io::Result<Self> {
            Ok(Self::from_socket(
                join_group(group, port, interface)?,
                collection_timeout,
            ))
        }

        /// A receiver on a socket that joined its groups.
        #[must_use]
        pub fn from_socket(socket: UdpSocket, collection_timeout: Duration) -> Self {
            Self {
                socket,
                reassembler: Reassembler::new(DEFAULT_MAX_COLLECTION, DEFAULT_MAX_PENDING),
                collection_timeout,
            }
        }

        /// The socket, e.g. to join further groups.
        #[must_use]
        pub fn socket(&self) -> &UdpSocket {
            &self.socket
        }

        /// The counters of delivered and dropped packets and collections.
        #[must_use]
        pub fn stats(&self) -> ReassemblyStats {
            self.reassembler.stats()
        }

        /// Waits at most `wait` for one datagram and assembles it; drops
        /// the collections that timed out.
        ///
        /// # Errors
        /// Socket errors other than a timeout.
        pub fn receive(&mut self, wait: Duration) -> io::Result<Received> {
            let now = Instant::now();
            if let Some(deadline) = now.checked_sub(self.collection_timeout) {
                self.reassembler.expire(deadline);
            }
            self.socket
                .set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
            let mut buf = alloc::vec![0u8; 65_536];
            let (n, from) = match self.socket.recv_from(&mut buf) {
                Ok(x) => x,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(Received::Idle);
                }
                Err(e) => return Err(e),
            };
            Ok(
                match self.reassembler.accept(&from, &buf[..n], Instant::now()) {
                    Ok(Some(giop)) => Received::Collection(giop, from),
                    Ok(None) => Received::Idle,
                    Err(e) => Received::Refused(e, from),
                },
            )
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn header(id: &[u8], number: u32, of: u32, stop: bool, len: u16) -> PacketHeader {
        PacketHeader {
            endianness: Endianness::Big,
            stop,
            packet_length: len,
            packet_number: number,
            number_of_packets: of,
            id: id.to_vec(),
        }
    }

    #[test]
    fn corba_p2_11_1_4_packet_header_layout_and_alignment() {
        let h = PacketHeader {
            endianness: Endianness::Big,
            stop: true,
            packet_length: 0x0102,
            packet_number: 3,
            number_of_packets: 4,
            id: alloc::vec![0xAA; 5],
        };
        let bytes = h.encode().unwrap();
        assert_eq!(&bytes[..4], b"MIOP");
        assert_eq!(bytes[4], 0x10); // 1.0
        assert_eq!(bytes[5], FLAG_STOP); // big endian, stop
        assert_eq!(&bytes[6..8], &[1, 2]);
        assert_eq!(&bytes[8..12], &[0, 0, 0, 3]);
        assert_eq!(&bytes[12..16], &[0, 0, 0, 4]); // sub-total 16 octets
        assert_eq!(&bytes[16..20], &[0, 0, 0, 5]); // id length
        assert_eq!(&bytes[20..25], &[0xAA; 5]);
        // GIOP data on the 8-octet boundary after the id: 25 -> 32.
        assert_eq!(bytes.len(), 32);
        assert_eq!(h.encoded_len(), 32);
        let mut packet = bytes.clone();
        packet.extend_from_slice(&[7u8; 0x0102]);
        let (back, data) = PacketHeader::decode(&packet).unwrap();
        assert_eq!(back, h);
        assert_eq!(data.len(), 0x0102);

        let le = PacketHeader {
            endianness: Endianness::Little,
            stop: false,
            packet_length: 0,
            id: alloc::vec![1; 12],
            ..h
        };
        let lb = le.encode().unwrap();
        assert_eq!(lb[5], FLAG_LITTLE_ENDIAN);
        assert_eq!(&lb[8..12], &[3, 0, 0, 0]);
        assert_eq!(lb.len(), 32); // 20 + 12 = 32
        assert_eq!(PacketHeader::decode(&lb).unwrap().0, le);
    }

    #[test]
    fn corba_p2_11_1_4_malformed_packets_are_refused() {
        let good = {
            let mut p = header(&[1, 2], 0, 1, true, 3).encode().unwrap();
            p.extend_from_slice(&[9, 9, 9]);
            p
        };
        assert!(PacketHeader::decode(&good).is_ok());
        let mut magic = good.clone();
        magic[0] = b'G';
        assert!(matches!(
            PacketHeader::decode(&magic),
            Err(MiopError::InvalidMagic(_))
        ));
        let mut version = good.clone();
        version[4] = 0x11;
        assert_eq!(
            PacketHeader::decode(&version).unwrap_err(),
            MiopError::UnsupportedVersion(0x11)
        );
        let mut reserved = good.clone();
        reserved[5] |= 0x04;
        assert!(matches!(
            PacketHeader::decode(&reserved),
            Err(MiopError::ReservedFlags(_))
        ));
        let mut longer = good.clone();
        longer.push(0);
        assert!(matches!(
            PacketHeader::decode(&longer),
            Err(MiopError::LengthMismatch { .. })
        ));
        let mut out_of_range = good.clone();
        out_of_range[11] = 1; // packet 1 of 1
        assert!(matches!(
            PacketHeader::decode(&out_of_range),
            Err(MiopError::PacketNumberOutOfRange { number: 1, of: 1 })
        ));
        let mut long_id = good.clone();
        long_id[19] = 253;
        assert!(matches!(
            PacketHeader::decode(&long_id),
            Err(MiopError::UniqueIdTooLong(253))
        ));
        assert_eq!(
            PacketHeader::decode(&good[..10]).unwrap_err(),
            MiopError::TooShort
        );
        assert!(header(&[0; 253], 0, 1, true, 0).encode().is_err());
    }

    #[test]
    fn corba_p2_11_1_3_packetize_splits_into_equal_packets_and_a_short_last() {
        let message: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let id = [5u8; 16];
        let packets = packetize(&message, &id, 300, Endianness::Little).unwrap();
        // header 20 + 16 = 36 -> 40; 260 octets of data per packet.
        assert_eq!(packets.len(), 4);
        let mut joined = Vec::new();
        for (i, p) in packets.iter().enumerate() {
            assert!(p.len() <= 300);
            let (h, data) = PacketHeader::decode(p).unwrap();
            assert_eq!(h.packet_number, i as u32);
            assert_eq!(h.number_of_packets, 4);
            assert_eq!(h.id, id);
            assert_eq!(h.stop, i == 3);
            assert_eq!(h.endianness, Endianness::Little);
            assert_eq!(usize::from(h.packet_length), if i < 3 { 260 } else { 220 });
            joined.extend_from_slice(data);
        }
        assert_eq!(joined, message);
        // A single packet when it fits.
        assert_eq!(
            packetize(&message, &id, 2000, Endianness::Big)
                .unwrap()
                .len(),
            1
        );
        assert!(packetize(&[], &id, 300, Endianness::Big).is_err());
        assert!(packetize(&message, &id, 40, Endianness::Big).is_err());
    }

    #[test]
    fn corba_p2_11_1_3_collection_reassembles_in_any_order() {
        let message: Vec<u8> = (0..5000u32).map(|i| (i % 253) as u8).collect();
        let packets = packetize(&message, b"id-1", 600, Endianness::Big).unwrap();
        assert!(packets.len() > 5);
        let mut r: Reassembler<u8, u32> = Reassembler::new(1 << 20, 8);
        // Every packet before the stop packet, in reverse order, then
        // the stop packet.
        let (last, rest) = packets.split_last().unwrap();
        for p in rest.iter().rev() {
            assert_eq!(r.accept(&1, p, 0).unwrap(), None);
        }
        assert_eq!(r.accept(&1, last, 0).unwrap(), Some(message.clone()));
        assert_eq!(r.pending(), 0);
        assert_eq!(r.stats().delivered, 1);
        // Two senders with the same id do not mix.
        let other: Vec<u8> = alloc::vec![1; 700];
        let po = packetize(&other, b"id-1", 600, Endianness::Big).unwrap();
        assert_eq!(r.accept(&1, &packets[0], 1).unwrap(), None);
        assert_eq!(r.accept(&2, &po[0], 1).unwrap(), None);
        assert_eq!(r.accept(&2, &po[1], 1).unwrap(), Some(other));
        assert_eq!(r.pending(), 1);
    }

    #[test]
    fn corba_p2_11_1_6_2_a_stop_packet_with_packets_missing_drops_the_collection() {
        let message = alloc::vec![3u8; 3000];
        let packets = packetize(&message, b"x", 600, Endianness::Big).unwrap();
        let mut r: Reassembler<u8, u32> = Reassembler::new(1 << 20, 8);
        r.accept(&1, &packets[0], 0).unwrap();
        let err = r.accept(&1, packets.last().unwrap(), 0).unwrap_err();
        assert!(
            matches!(err, MiopError::Incomplete { received: 2, .. }),
            "{err:?}"
        );
        assert_eq!(r.pending(), 0);
        assert_eq!(r.stats().incomplete, 1);
        // The packets that come late start a collection the stop packet
        // will not end: it times out.
        r.accept(&1, &packets[1], 5).unwrap();
        assert_eq!(r.expire(6), 1);
        assert_eq!(r.stats().timed_out, 1);
    }

    #[test]
    fn corba_p2_11_1_6_2_the_timeout_counts_from_the_last_packet_of_a_collection() {
        let message = alloc::vec![5u8; 6000];
        let packets = packetize(&message, b"slow", 600, Endianness::Big).unwrap();
        assert!(packets.len() >= 10);
        let mut r: Reassembler<u8, u32> = Reassembler::new(1 << 20, 8);
        // One packet every 5 ticks with a timeout of 10 ticks: the
        // collection is older than the timeout long before it completes,
        // but its last packet never is.
        let timeout = 10;
        let mut delivered = None;
        for (i, p) in packets.iter().enumerate() {
            let now = 5 * i as u32;
            assert_eq!(r.expire(now.saturating_sub(timeout)), 0, "packet {i}");
            delivered = r.accept(&1, p, now).unwrap();
        }
        assert_eq!(delivered, Some(message.clone()));
        assert_eq!(r.stats().timed_out, 0);
        // A collection whose packets stop: timed out once the timeout has
        // passed since its last packet, not before.
        r.accept(&1, &packets[0], 100).unwrap();
        r.accept(&1, &packets[1], 108).unwrap();
        // At 117 the collection started more than the timeout ago, but
        // its last packet did not.
        assert_eq!(r.expire(117 - timeout), 0);
        assert_eq!(r.expire(119 - timeout), 1);
        assert_eq!(r.stats().timed_out, 1);
        // With too many pending, the one longest without a packet goes,
        // not the one started first.
        let mut few: Reassembler<u8, u32> = Reassembler::new(1 << 20, 2);
        let a = packetize(&message, b"a", 600, Endianness::Big).unwrap();
        let b = packetize(&message, b"b", 600, Endianness::Big).unwrap();
        let c = packetize(&message, b"c", 600, Endianness::Big).unwrap();
        few.accept(&1, &a[0], 0).unwrap();
        few.accept(&1, &b[0], 1).unwrap();
        few.accept(&1, &a[1], 2).unwrap();
        few.accept(&1, &c[0], 3).unwrap();
        assert_eq!(few.stats().timed_out, 1);
        // `a` is still there: its next packet continues it.
        few.accept(&1, &a[2], 4).unwrap();
        assert_eq!(few.pending(), 2);
        assert_eq!(few.stats().timed_out, 1);
        assert_eq!(few.stats().duplicates, 0);
        // `b` was dropped: its next packet starts a new collection, which
        // evicts the one longest without a packet (`c`).
        few.accept(&1, &b[1], 5).unwrap();
        assert_eq!(few.stats().timed_out, 2);
    }

    #[test]
    fn corba_p2_11_1_6_2_duplicates_and_contradictions_are_reported() {
        let message = alloc::vec![3u8; 3000];
        let packets = packetize(&message, b"y", 600, Endianness::Big).unwrap();
        let mut r: Reassembler<u8, u32> = Reassembler::new(1 << 20, 8);
        r.accept(&1, &packets[0], 0).unwrap();
        assert_eq!(
            r.accept(&1, &packets[0], 0).unwrap_err(),
            MiopError::DuplicatePacket(0)
        );
        assert_eq!(r.stats().duplicates, 1);
        assert_eq!(r.pending(), 1);
        // A packet of another length before the last.
        let mut odd = header(b"y", 1, packets.len() as u32, false, 10)
            .encode()
            .unwrap();
        odd.extend_from_slice(&[0; 10]);
        assert!(matches!(
            r.accept(&1, &odd, 0),
            Err(MiopError::Inconsistent(_))
        ));
        assert_eq!(r.pending(), 0);
        // A collection over the size limit.
        let mut small: Reassembler<u8, u32> = Reassembler::new(1000, 8);
        small.accept(&1, &packets[0], 0).unwrap();
        assert!(matches!(
            small.accept(&1, &packets[1], 0),
            Err(MiopError::CollectionTooLarge(_))
        ));
        assert_eq!(small.stats().too_large, 1);
        // Too many pending: the oldest goes.
        let mut few: Reassembler<u8, u32> = Reassembler::new(1 << 20, 2);
        for (i, id) in [b"a", b"b", b"c"].iter().enumerate() {
            let p = packetize(&message, *id, 600, Endianness::Big).unwrap();
            few.accept(&1, &p[0], i as u32).unwrap();
        }
        assert_eq!(few.pending(), 2);
        assert_eq!(few.stats().timed_out, 1);
        assert!(matches!(
            r.accept(&1, b"GIOP-not-miop-at-all", 0),
            Err(MiopError::InvalidMagic(_))
        ));
        assert_eq!(r.stats().malformed, 1);
    }

    /// A packet of `len` octets of data.
    fn packet(id: &[u8], number: u32, of: u32, stop: bool, len: u16) -> Vec<u8> {
        let mut p = header(id, number, of, stop, len).encode().unwrap();
        p.resize(p.len() + usize::from(len), 7);
        p
    }

    /// §11.1.4: `packet_number` is below `number_of_packets`. A packet
    /// that leaves `number_of_packets` at 0 is numbered below the count an
    /// earlier packet of its collection set, and a count that comes after
    /// packets without one holds for them too. The highest packet number
    /// as a stop packet is a collection of 2^32 packets, counted without
    /// overflow.
    #[test]
    fn corba_p2_11_1_4_a_packet_number_at_or_beyond_the_count_of_its_collection_is_refused() {
        let mut r: Reassembler<u8, u64> = Reassembler::new(1 << 20, 4);
        assert_eq!(r.accept(&1, &packet(b"n", 0, 5, false, 8), 0), Ok(None));
        assert_eq!(
            r.accept(&1, &packet(b"n", u32::MAX, 0, true, 8), 0),
            Err(MiopError::PacketNumberOutOfRange {
                number: u32::MAX,
                of: 5
            })
        );
        assert_eq!(r.pending(), 0);
        assert_eq!(r.stats().incomplete, 1);
        // The same before the stop packet.
        assert_eq!(r.accept(&1, &packet(b"n", 0, 5, false, 8), 1), Ok(None));
        assert_eq!(
            r.accept(&1, &packet(b"n", 5, 0, false, 8), 1),
            Err(MiopError::PacketNumberOutOfRange { number: 5, of: 5 })
        );
        // A count after packets beyond it, and after a stop packet that
        // is not packet n-1.
        assert_eq!(r.accept(&1, &packet(b"m", 6, 0, false, 8), 2), Ok(None));
        assert!(matches!(
            r.accept(&1, &packet(b"m", 0, 5, false, 8), 2),
            Err(MiopError::Inconsistent(_))
        ));
        assert_eq!(r.accept(&1, &packet(b"s", 1, 0, false, 8), 3), Ok(None));
        assert!(matches!(
            r.accept(&1, &packet(b"s", 2, 0, true, 4), 3),
            Err(MiopError::Incomplete {
                received: 2,
                expected: 3
            })
        ));
        assert_eq!(r.accept(&1, &packet(b"t", 1, 0, false, 8), 4), Ok(None));
        assert_eq!(r.accept(&1, &packet(b"t", 0, 0, false, 8), 4), Ok(None));
        assert!(matches!(
            r.accept(&1, &packet(b"t", 2, 4, true, 4), 4),
            Err(MiopError::Inconsistent(_))
        ));
        // A stop packet with the highest number and no count.
        assert_eq!(
            r.accept(&1, &packet(b"u", u32::MAX, 0, true, 8), 5),
            Err(MiopError::Incomplete {
                received: 1,
                expected: 1 << 32
            })
        );
        // Packets numbered below the count still assemble.
        let mut ok: Reassembler<u8, u64> = Reassembler::new(1 << 20, 4);
        assert_eq!(ok.accept(&1, &packet(b"v", 0, 2, false, 8), 0), Ok(None));
        assert_eq!(
            ok.accept(&1, &packet(b"v", 1, 0, true, 4), 0)
                .unwrap()
                .map(|m| m.len()),
            Some(12)
        );
    }

    /// What incomplete collections hold together is bounded: the
    /// collections longest without a packet go first, a collection that
    /// alone holds more is too large, and empty packets count with their
    /// overhead.
    #[test]
    fn corba_p2_11_1_6_2_the_pending_collections_are_bounded_in_octets() {
        let limit = 10 * (1000 + PACKET_OVERHEAD);
        let mut r: Reassembler<u8, u64> =
            Reassembler::new(1 << 20, 256).with_max_pending_bytes(limit);
        // Twenty collections of one 1000-octet packet each, from senders
        // the receiver cannot tell apart from the group's.
        for i in 0..20u8 {
            assert_eq!(
                r.accept(&i, &packet(b"g", 0, 3, false, 1000), u64::from(i)),
                Ok(None)
            );
            assert!(r.pending_bytes() <= limit, "{}", r.pending_bytes());
        }
        assert_eq!(r.pending(), 10);
        assert_eq!(r.stats().timed_out, 10);
        // The newest ten are left: the next packet of sender 10 continues
        // its collection, that of sender 0 starts a new one.
        assert_eq!(
            r.accept(&10, &packet(b"g", 1, 3, false, 1000), 30),
            Ok(None)
        );
        assert_eq!(r.stats().timed_out, 11);
        assert!(r.pending_bytes() <= limit);
        // The stop packet completes it: nothing else goes.
        assert_eq!(
            r.accept(&10, &packet(b"g", 2, 3, true, 10), 31)
                .unwrap()
                .map(|m| m.len()),
            Some(2010)
        );
        assert_eq!(r.stats().timed_out, 11);
        assert_eq!(r.pending(), 8);
        assert_eq!(r.pending_bytes(), 8 * (1000 + PACKET_OVERHEAD));
        // A collection that alone goes beyond the limit.
        let mut one: Reassembler<u8, u64> =
            Reassembler::new(1 << 20, 256).with_max_pending_bytes(limit);
        let mut refused = None;
        for n in 0..12 {
            if let Err(e) = one.accept(&1, &packet(b"big", n, 0, false, 1000), 0) {
                refused = Some((n, e));
                break;
            }
        }
        assert_eq!(refused, Some((10, MiopError::CollectionTooLarge(11_000))));
        assert_eq!(one.pending(), 0);
        assert_eq!(one.pending_bytes(), 0);
        assert_eq!(one.stats().too_large, 1);
        // Empty packets: each costs its overhead, so a stream of them
        // ends at the limit as well.
        let mut empty: Reassembler<u8, u64> =
            Reassembler::new(1 << 20, 256).with_max_pending_bytes(limit);
        let mut taken = 0u32;
        while empty
            .accept(&1, &packet(b"e", taken, 0, false, 0), 0)
            .is_ok()
        {
            taken += 1;
            assert!(taken < 1000, "empty packets are not bounded");
        }
        assert_eq!(taken as usize, limit / PACKET_OVERHEAD);
        // The timeout gives back what the dropped collections held.
        let mut t: Reassembler<u8, u64> = Reassembler::new(1 << 20, 256);
        t.accept(&1, &packet(b"x", 0, 2, false, 100), 0).unwrap();
        t.accept(&2, &packet(b"x", 0, 2, false, 100), 5).unwrap();
        assert_eq!(t.pending_bytes(), 2 * (100 + PACKET_OVERHEAD));
        assert_eq!(t.expire(3), 1);
        assert_eq!(t.pending_bytes(), 100 + PACKET_OVERHEAD);
    }
}
