// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP ProfileBody — Spec §15.7.2 (all 4 versions 1.0/1.1/1.2/1.3).
//!
//! ```text
//! struct Version {
//!     octet major;
//!     octet minor;
//! };
//!
//! struct ProfileBody_1_0 {
//!     Version            iiop_version;     // 1.0
//!     string             host;
//!     unsigned short     port;
//!     sequence<octet>    object_key;
//! };
//!
//! struct ProfileBody_1_1 {  // also 1.2, 1.3
//!     Version                   iiop_version;
//!     string                    host;
//!     unsigned short            port;
//!     sequence<octet>           object_key;
//!     sequence<TaggedComponent> components;
//! };
//! ```
//!
//! `TaggedComponent` is a simple `(tag, encapsulation_octets)`
//! container; its content depends on the tag (Spec §13.6.6).
//! We keep the content opaque — `crates/corba-ior/` decodes the
//! standard 32 tags.
//!
//! The ProfileBody is stored inside an `IOR.profiles[i].profile_data`
//! `sequence<octet>` as a CDR encapsulation (Spec §13.6.3
//! Tab 13-3): first an `octet` with the endianness, then the body in
//! that endianness.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

/// IIOP version (`major.minor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IiopVersion {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

impl IiopVersion {
    /// IIOP 1.0.
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// IIOP 1.1.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// IIOP 1.2.
    pub const V1_2: Self = Self { major: 1, minor: 2 };
    /// IIOP 1.3.
    pub const V1_3: Self = Self { major: 1, minor: 3 };
    /// IIOP 1.4 (CORBA 3.3 Part 2 §9.7: based on GIOP 1.0 to 1.4).
    pub const V1_4: Self = Self { major: 1, minor: 4 };
    /// The highest version this ORB processes: a profile it publishes may
    /// name any version up to this one (Part 2 §9.7.2, "the minor version
    /// in the TAG_INTERNET_IOP profile signals the highest minor revision
    /// of GIOP supported by the server"). Which version the profiles of a
    /// server carry is the server's setting.
    pub const MAX: Self = Self::V1_4;

    /// Constructor.
    #[must_use]
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// `true` when the profile's host may be an IPv6 address (Part 2
    /// §7.9 Table 7.2, "IPV6 addresses in IOR": version 1.4).
    #[must_use]
    pub const fn allows_ipv6_host(self) -> bool {
        self.major > 1 || self.minor >= 4
    }

    /// The version a profile with `host` is written in: `self`, or 1.4
    /// when `host` is an IPv6 address, with or without the brackets of
    /// the URL form, and `self` is below 1.4 (Part 2 §7.9 Table 7.2:
    /// "IPV6 addresses in IOR" belong to version 1.4).
    #[must_use]
    pub fn for_host(self, host: &str) -> Self {
        if !self.allows_ipv6_host()
            && unbracketed_host(host)
                .parse::<core::net::Ipv6Addr>()
                .is_ok()
        {
            Self::V1_4
        } else {
            self
        }
    }

    /// `true` if the version has a `components` sequence in the body
    /// (Spec §15.7.2: from IIOP 1.1 onward).
    #[must_use]
    pub const fn has_components(self) -> bool {
        if self.major > 1 {
            true
        } else {
            self.minor >= 1
        }
    }
}

/// `host` without the brackets an IPv6 address takes in an object URL
/// (`[::1]`, Part 2 §7.6.10.3); any other host as it is.
#[must_use]
pub fn unbracketed_host(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
}

/// Tagged component — Spec §13.6.6.
///
/// `tag` is an `unsigned long` (e.g. `TAG_ORB_TYPE = 0`,
/// `TAG_CODE_SETS = 1`, `TAG_POLICIES = 2`, …).
/// `component_data` is a CDR-encapsulated octet stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedComponent {
    /// Tag ID.
    pub tag: u32,
    /// Encapsulation bytes (length+data or endianness+body).
    pub component_data: Vec<u8>,
}

/// IIOP ProfileBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IiopProfileBody {
    /// IIOP version.
    pub iiop_version: IiopVersion,
    /// Host name or IP address as a string (Spec §15.7.2:
    /// "string identifying the host").
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Object key (POA-encoded).
    pub object_key: Vec<u8>,
    /// `components` — empty in IIOP 1.0; from 1.1 onward it holds all
    /// optional TaggedComponents.
    pub components: Vec<TaggedComponent>,
}

impl IiopProfileBody {
    /// Constructor with the minimal set of required fields.
    #[must_use]
    pub fn new(version: IiopVersion, host: String, port: u16, object_key: Vec<u8>) -> Self {
        Self {
            iiop_version: version,
            host,
            port,
            object_key,
            components: Vec::new(),
        }
    }

    /// The profile a server publishes for its endpoint `host`:`port`
    /// (Part 2 §7.9, §9.7.2): the `host` field holds the address without
    /// the brackets of the URL form (`[::1]` is written `::1`; the
    /// brackets belong to the object URL syntax of §7.6.10.3, the profile
    /// names the host), and the version is `version` or, for an IPv6
    /// address, 1.4 ([`IiopVersion::for_host`]).
    #[must_use]
    pub fn for_endpoint(version: IiopVersion, host: &str, port: u16, object_key: Vec<u8>) -> Self {
        let host = unbracketed_host(host);
        Self::new(version.for_host(host), host.into(), port, object_key)
    }

    /// CDR-encodes the ProfileBody (without the encapsulation wrapper —
    /// the caller wraps it in `crates/corba-ior/` when needed).
    ///
    /// # Errors
    /// Buffer write error or length overflow;
    /// [`CdrError::ComponentsNotInVersion`] for an IIOP 1.0 body that
    /// holds components: `ProfileBody_1_0` has no place for them
    /// (CORBA 3.3 Part 2 §7.6.6, Table 7.2 "IOR components in IIOP
    /// profile" from 1.1), and they are not left out silently.
    pub fn encode(&self, w: &mut Writer) -> Result<(), CdrError> {
        if !self.iiop_version.has_components() && !self.components.is_empty() {
            return Err(CdrError::ComponentsNotInVersion);
        }
        w.write_u8(self.iiop_version.major);
        w.write_u8(self.iiop_version.minor);
        w.write_string(&self.host);
        w.write_u16(self.port);
        // sequence<octet> object_key.
        let n = u32::try_from(self.object_key.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n);
        w.write_bytes(&self.object_key);
        // components (from IIOP 1.1 onward).
        if self.iiop_version.has_components() {
            let cn = u32::try_from(self.components.len()).map_err(|_| CdrError::Overflow)?;
            w.write_u32(cn);
            for c in &self.components {
                w.write_u32(c.tag);
                let cd = u32::try_from(c.component_data.len()).map_err(|_| CdrError::Overflow)?;
                w.write_u32(cd);
                w.write_bytes(&c.component_data);
            }
        }
        Ok(())
    }

    /// CDR-decodes.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, CdrError> {
        let major = r.read_u8()?;
        let minor = r.read_u8()?;
        let iiop_version = IiopVersion::new(major, minor);
        let host = r.read_string()?;
        let port = r.read_u16()?;
        let key_len = r.read_u32()? as usize;
        let object_key = r.read_bytes(key_len)?.to_vec();
        let components = if iiop_version.has_components() {
            let cn = r.read_u32()? as usize;
            let mut out = Vec::with_capacity(cn.min(64));
            for _ in 0..cn {
                let tag = r.read_u32()?;
                let cd_len = r.read_u32()? as usize;
                let component_data = r.read_bytes(cd_len)?.to_vec();
                out.push(TaggedComponent {
                    tag,
                    component_data,
                });
            }
            out
        } else {
            Vec::new()
        };
        Ok(Self {
            iiop_version,
            host,
            port,
            object_key,
            components,
        })
    }

    /// Encodes the ProfileBody as a CDR encapsulation (Spec §15.3.3):
    /// `octet endianness + body in that endianness`. This is the
    /// form in which the ProfileBody is stored in
    /// `IOR.profiles[i].profile_data`.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        // The byte-order octet goes into the **same** writer, so every
        // alignment inside the body is counted from the start of the
        // encapsulation because that is where it starts. This used to
        // write the octet into a `Vec`, build a second writer, and tell
        // that writer its byte 0 was byte 1 — the same shape as the
        // buffer `corba-giop` had that was told its byte 0 was byte 12.
        // §15.3.3 says the encapsulation *is* the octet plus the body;
        // a coordinate a buffer is given is a coordinate that can be
        // given wrongly, and one it has cannot be.
        let mut w = Writer::new(endianness);
        // Byte-order octet: bit 0 is 0 for big-endian, 1 for little.
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        self.encode(&mut w)?;
        Ok(w.finish())
    }

    /// Decodes a CDR encapsulation (see `encode_encapsulation`).
    ///
    /// # Errors
    /// Buffer read error or invalid endianness byte.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        if bytes.is_empty() {
            return Err(CdrError::Truncated);
        }
        let endianness = match bytes[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            _ => return Err(CdrError::InvalidEndianness),
        };
        // The reader is over the whole encapsulation, and the
        // byte-order octet is read out of it rather than sliced off.
        // Alignment then counts from the encapsulation's own byte 0,
        // which is what §15.3.3 means, with no origin handed over.
        let mut r = Reader::new(bytes, endianness);
        r.read_u8()?;
        Self::decode(&mut r)
    }
}

/// Local CDR error type (wraps the `zerodds-cdr` errors and additionally
/// covers length-overflow + truncation cases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdrError {
    /// What the wire runtime refused. `zerodds-cdr` had a separate
    /// encode and decode error and this carried both; `zerodds-wire`
    /// has one, and two variants over one type is a `From` the
    /// compiler cannot choose between.
    Wire(zerodds_wire::WireError),
    /// Sequence length exceeds `u32::MAX`.
    Overflow,
    /// Truncated encapsulation (too few bytes).
    Truncated,
    /// Endianness octet is neither 0 nor 1.
    InvalidEndianness,
    /// An IIOP 1.0 profile body with components, which only IIOP 1.1
    /// and later carry.
    ComponentsNotInVersion,
}

impl From<zerodds_wire::WireError> for CdrError {
    fn from(error: zerodds_wire::WireError) -> Self {
        Self::Wire(error)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn iiop_version_constants_match_spec() {
        assert_eq!(IiopVersion::V1_0, IiopVersion::new(1, 0));
        assert_eq!(IiopVersion::V1_3, IiopVersion::new(1, 3));
        assert_eq!(IiopVersion::V1_4, IiopVersion::new(1, 4));
        assert_eq!(IiopVersion::MAX, IiopVersion::V1_4);
    }

    /// Table 7.2 "IPV6 addresses in IOR": 1.4 only. A 1.4 profile with an
    /// IPv6 host round-trips with the host as written.
    #[test]
    fn corba_p2_7_9_ipv6_host_in_a_1_4_profile() {
        assert!(!IiopVersion::V1_2.allows_ipv6_host());
        assert!(!IiopVersion::V1_3.allows_ipv6_host());
        assert!(IiopVersion::V1_4.allows_ipv6_host());
        let body = IiopProfileBody::new(IiopVersion::V1_4, "fe80::1:2".into(), 2809, b"k".to_vec());
        let bytes = body.encode_encapsulation(Endianness::Little).unwrap();
        let back = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(back, body);
    }

    /// Table 7.2 "IPV6 addresses in IOR": the profile a server publishes
    /// for an IPv6 host is IIOP 1.4 whatever version it asks for, and its
    /// `host` holds the address without the brackets of the URL form; a
    /// DNS name or an IPv4 address keeps the version asked for.
    #[test]
    fn corba_p2_7_9_an_endpoint_profile_for_an_ipv6_host_is_1_4_without_brackets() {
        for host in ["::1", "[::1]"] {
            for version in [IiopVersion::V1_0, IiopVersion::V1_2, IiopVersion::V1_4] {
                let body = IiopProfileBody::for_endpoint(version, host, 2809, b"k".to_vec());
                assert_eq!(body.iiop_version, IiopVersion::V1_4, "{host} {version:?}");
                assert_eq!(body.host, "::1", "{host}");
                assert_eq!((body.port, &body.object_key[..]), (2809, &b"k"[..]));
            }
        }
        for host in ["host.example", "192.0.2.7"] {
            let body = IiopProfileBody::for_endpoint(IiopVersion::V1_2, host, 1, b"k".to_vec());
            assert_eq!(
                (body.iiop_version, &body.host[..]),
                (IiopVersion::V1_2, host)
            );
        }
        // A bracketed text that is no IPv6 address is not raised.
        assert_eq!(IiopVersion::V1_2.for_host("[host]"), IiopVersion::V1_2);
        assert_eq!(unbracketed_host("[::1"), "[::1");
    }

    #[test]
    fn components_only_from_iiop_1_1() {
        // Spec §15.7.2: components sequence only from IIOP 1.1 onward.
        assert!(!IiopVersion::V1_0.has_components());
        assert!(IiopVersion::V1_1.has_components());
        assert!(IiopVersion::V1_2.has_components());
        assert!(IiopVersion::V1_3.has_components());
    }

    /// Table 7.2 has no components in IIOP 1.0: a 1.0 body with
    /// components is refused, not encoded without them; the same
    /// components encode under 1.1.
    #[test]
    fn corba_p2_7_9_iiop_1_0_profile_with_components_is_refused() {
        let mut body = sample_v1_0();
        body.components.push(TaggedComponent {
            tag: 1, // TAG_CODE_SETS
            component_data: alloc::vec![0, 0, 0, 0],
        });
        let mut w = Writer::new(Endianness::Big);
        assert_eq!(body.encode(&mut w), Err(CdrError::ComponentsNotInVersion));
        assert_eq!(
            body.encode_encapsulation(Endianness::Little),
            Err(CdrError::ComponentsNotInVersion)
        );
        body.iiop_version = IiopVersion::V1_1;
        let bytes = body.encode_encapsulation(Endianness::Big).unwrap();
        let back = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(back.components, body.components);
        // A 1.0 body without components still encodes.
        let plain = sample_v1_0();
        let back = IiopProfileBody::decode_encapsulation(
            &plain.encode_encapsulation(Endianness::Big).unwrap(),
        )
        .unwrap();
        assert_eq!(back, plain);
    }

    fn sample_v1_0() -> IiopProfileBody {
        IiopProfileBody::new(
            IiopVersion::V1_0,
            "example.com".into(),
            7777,
            alloc::vec![0xab, 0xcd],
        )
    }

    fn sample_v1_2_with_components() -> IiopProfileBody {
        IiopProfileBody {
            iiop_version: IiopVersion::V1_2,
            host: "10.0.0.1".into(),
            port: 9999,
            object_key: alloc::vec![1, 2, 3, 4],
            components: alloc::vec![
                TaggedComponent {
                    tag: 0, // TAG_ORB_TYPE
                    component_data: alloc::vec![0xde, 0xad],
                },
                TaggedComponent {
                    tag: 1, // TAG_CODE_SETS
                    component_data: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
                },
            ],
        }
    }

    #[test]
    fn round_trip_v1_0_be() {
        let p = sample_v1_0();
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        // Encapsulation starts with endianness byte 0 (BE).
        assert_eq!(bytes[0], 0);
    }

    #[test]
    fn round_trip_v1_0_le() {
        let p = sample_v1_0();
        let bytes = p.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        assert_eq!(bytes[0], 1);
    }

    #[test]
    fn round_trip_v1_2_with_components() {
        let p = sample_v1_2_with_components();
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        assert_eq!(decoded.components.len(), 2);
    }

    #[test]
    fn v1_0_does_not_emit_components_field() {
        // Spec §15.7.2: IIOP 1.0 has no components sequence on the wire:
        // the encapsulation ends with the object key. A body with
        // components is refused (see
        // `corba_p2_7_9_iiop_1_0_profile_with_components_is_refused`).
        let p = sample_v1_0();
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        assert_eq!(bytes.last_chunk::<2>(), Some(&[0xab, 0xcd]));
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert!(decoded.components.is_empty());
    }

    #[test]
    fn invalid_endianness_byte_is_diagnostic() {
        let bytes = alloc::vec![0xff, 0, 0];
        let err = IiopProfileBody::decode_encapsulation(&bytes).unwrap_err();
        assert!(matches!(err, CdrError::InvalidEndianness));
    }

    #[test]
    fn empty_encapsulation_is_truncated() {
        let err = IiopProfileBody::decode_encapsulation(&[]).unwrap_err();
        assert!(matches!(err, CdrError::Truncated));
    }
}
