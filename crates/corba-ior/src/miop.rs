// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The MIOP object model — CORBA 3.3 Part 2 §11.2: the UIPMC profile, the
//! group components, group references and their `corbaloc:miop` form.
//!
//! ```text
//! module MIOP {
//!     typedef GIOP::Version Version;
//!     typedef string Address;
//!     struct UIPMC_ProfileBody {          // TAG_UIPMC = 3
//!         Version miop_version;
//!         Address the_address;            // class D IPv4, IPv6 multicast or alias
//!         short   the_port;
//!         sequence<IOP::TaggedComponent> components;
//!     };
//! };
//! module PortableGroup {
//!     struct GroupInfo {                  // TAG_GROUP = 39
//!         Version               component_version;
//!         GroupDomainId         group_domain_id;          // string
//!         ObjectGroupId         object_group_id;          // unsigned long long
//!         ObjectGroupRefVersion object_group_ref_version; // unsigned long
//!     };
//!     typedef sequence<octet> GroupIIOPProfile;           // TAG_GROUP_IIOP = 40
//! };
//! ```
//!
//! A UIPMC profile has no object key: the `GroupInfo` component names the
//! group, and the object adapter maps it to its object ids (§11.2.4). A
//! group reference (§11.2.3) carries a UIPMC profile, an IIOP profile of
//! a gateway, or both.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_corba_iiop::profile_body::CdrError;
use zerodds_corba_iiop::{IiopProfileBody, IiopVersion, TaggedComponent};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::error::{IorError, IorResult};
use crate::ior::Ior;
use crate::profile_tags::ProfileId;
use crate::tagged_profile::TaggedProfile;

/// `IOP::TAG_GROUP` (§11.2.2.4).
pub const TAG_GROUP: u32 = 39;
/// `IOP::TAG_GROUP_IIOP` (§11.2.2.4).
pub const TAG_GROUP_IIOP: u32 = 40;
/// The version of the UIPMC profile and of the group component: 1.0.
pub const MIOP_VERSION_1_0: IiopVersion = IiopVersion::V1_0;
/// The group domain of a group created without one (§11.2.6.8.7).
pub const DEFAULT_GROUP_DOMAIN: &str = "DefaultGroupDomain";

fn begin(endianness: Endianness) -> Writer {
    let mut w = Writer::new(endianness);
    w.write_u8(match endianness {
        Endianness::Big => 0,
        Endianness::Little => 1,
    });
    w
}

fn open(bytes: &[u8]) -> Result<Reader<'_>, CdrError> {
    let endianness = match bytes.first() {
        Some(0) => Endianness::Big,
        Some(1) => Endianness::Little,
        Some(_) => return Err(CdrError::InvalidEndianness),
        None => return Err(CdrError::Truncated),
    };
    let mut r = Reader::new(bytes, endianness);
    r.read_u8()?;
    Ok(r)
}

/// `MIOP::UIPMC_ProfileBody` (§11.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UipmcProfileBody {
    /// `miop_version`, 1.0.
    pub miop_version: IiopVersion,
    /// `the_address`: a class D IPv4 address, an IPv6 multicast address
    /// or an alias for one.
    pub the_address: String,
    /// `the_port`.
    pub the_port: u16,
    /// `components`; a group reference has a `TAG_GROUP` component.
    pub components: Vec<TaggedComponent>,
}

impl UipmcProfileBody {
    /// A 1.0 profile for `address:port` with `components`.
    #[must_use]
    pub fn new(address: impl Into<String>, port: u16, components: Vec<TaggedComponent>) -> Self {
        Self {
            miop_version: MIOP_VERSION_1_0,
            the_address: address.into(),
            the_port: port,
            components,
        }
    }

    /// Encodes the body as the CDR encapsulation that is the
    /// `profile_data` of a `TAG_UIPMC` profile.
    ///
    /// # Errors
    /// `Overflow` for a component list or component longer than a CDR
    /// sequence holds.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        let mut w = begin(endianness);
        w.write_u8(self.miop_version.major);
        w.write_u8(self.miop_version.minor);
        w.write_string(&self.the_address);
        w.write_u16(self.the_port);
        w.write_u32(u32::try_from(self.components.len()).map_err(|_| CdrError::Overflow)?);
        for c in &self.components {
            w.write_u32(c.tag);
            w.write_u32(u32::try_from(c.component_data.len()).map_err(|_| CdrError::Overflow)?);
            w.write_bytes(&c.component_data);
        }
        Ok(w.finish())
    }

    /// Decodes the `profile_data` of a `TAG_UIPMC` profile.
    ///
    /// # Errors
    /// A truncated or inconsistent encapsulation.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        let mut r = open(bytes)?;
        let major = r.read_u8()?;
        let minor = r.read_u8()?;
        let the_address = r.read_string()?;
        let the_port = r.read_u16()?;
        let n = r.read_u32()? as usize;
        if n > r.remaining() / 8 {
            return Err(CdrError::Truncated);
        }
        let mut components = Vec::with_capacity(n);
        for _ in 0..n {
            let tag = r.read_u32()?;
            let len = r.read_u32()? as usize;
            if len > r.remaining() {
                return Err(CdrError::Truncated);
            }
            components.push(TaggedComponent {
                tag,
                component_data: r.read_bytes(len)?.to_vec(),
            });
        }
        Ok(Self {
            miop_version: IiopVersion::new(major, minor),
            the_address,
            the_port,
            components,
        })
    }

    /// The `TAG_UIPMC` profile of this body.
    ///
    /// # Errors
    /// As [`Self::encode_encapsulation`].
    pub fn to_tagged_profile(&self, endianness: Endianness) -> Result<TaggedProfile, CdrError> {
        Ok(TaggedProfile {
            tag: ProfileId::Uipmc,
            profile_data: self.encode_encapsulation(endianness)?,
        })
    }

    /// The `GroupInfo` of the `TAG_GROUP` component; `None` without one.
    ///
    /// # Errors
    /// A `TAG_GROUP` component that does not decode.
    pub fn group_info(&self) -> Result<Option<GroupInfo>, CdrError> {
        self.components
            .iter()
            .find(|c| c.tag == TAG_GROUP)
            .map(|c| GroupInfo::decode_encapsulation(&c.component_data))
            .transpose()
    }

    /// The IIOP profile of the `TAG_GROUP_IIOP` component, which serves
    /// the operations of `CORBA::Object` and two-way calls (§11.2.2.4.2);
    /// `None` without one.
    ///
    /// # Errors
    /// A `TAG_GROUP_IIOP` component that does not decode.
    pub fn group_iiop_profile(&self) -> Result<Option<IiopProfileBody>, CdrError> {
        self.components
            .iter()
            .find(|c| c.tag == TAG_GROUP_IIOP)
            .map(|c| IiopProfileBody::decode_encapsulation(&c.component_data))
            .transpose()
    }
}

/// `PortableGroup::GroupInfo`, the body of `TAG_GROUP` (§11.2.2.4.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupInfo {
    /// `component_version`, 1.0.
    pub component_version: IiopVersion,
    /// `group_domain_id`: the scope of the group id.
    pub group_domain_id: String,
    /// `object_group_id`: the 64-bit id of the group.
    pub object_group_id: u64,
    /// `object_group_ref_version`; 0 when not used.
    pub object_group_ref_version: u32,
}

impl GroupInfo {
    /// A 1.0 group component.
    #[must_use]
    pub fn new(group_domain_id: impl Into<String>, object_group_id: u64, ref_version: u32) -> Self {
        Self {
            component_version: MIOP_VERSION_1_0,
            group_domain_id: group_domain_id.into(),
            object_group_id,
            object_group_ref_version: ref_version,
        }
    }

    /// Encodes the struct as a CDR encapsulation.
    #[must_use]
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Vec<u8> {
        let mut w = begin(endianness);
        w.write_u8(self.component_version.major);
        w.write_u8(self.component_version.minor);
        w.write_string(&self.group_domain_id);
        w.write_u64(self.object_group_id);
        w.write_u32(self.object_group_ref_version);
        w.finish()
    }

    /// Decodes the encapsulation of a `TAG_GROUP` component.
    ///
    /// # Errors
    /// A truncated encapsulation.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        let mut r = open(bytes)?;
        let major = r.read_u8()?;
        let minor = r.read_u8()?;
        let group_domain_id = r.read_string()?;
        let object_group_id = r.read_u64()?;
        let object_group_ref_version = r.read_u32()?;
        Ok(Self {
            component_version: IiopVersion::new(major, minor),
            group_domain_id,
            object_group_id,
            object_group_ref_version,
        })
    }

    /// The `TAG_GROUP` component.
    #[must_use]
    pub fn to_component(&self, endianness: Endianness) -> TaggedComponent {
        TaggedComponent {
            tag: TAG_GROUP,
            component_data: self.encode_encapsulation(endianness),
        }
    }
}

/// The `TAG_GROUP_IIOP` component carrying `iiop` (§11.2.2.4.2).
///
/// # Errors
/// As `IiopProfileBody::encode_encapsulation`.
pub fn group_iiop_component(
    iiop: &IiopProfileBody,
    endianness: Endianness,
) -> Result<TaggedComponent, CdrError> {
    Ok(TaggedComponent {
        tag: TAG_GROUP_IIOP,
        component_data: iiop.encode_encapsulation(endianness)?,
    })
}

/// The profiles of a group reference, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupProfiles {
    /// The UIPMC profiles, in the order of the IOR.
    pub uipmc: Vec<UipmcProfileBody>,
    /// The IIOP profiles (of a gateway), in the order of the IOR.
    pub iiop: Vec<IiopProfileBody>,
    /// The `GroupInfo` of every profile that carries one, in order.
    pub groups: Vec<GroupInfo>,
}

/// The decoded profiles of `ior` (§11.2.3).
///
/// # Errors
/// A UIPMC or IIOP profile or a group component that does not decode.
pub fn group_profiles(ior: &Ior) -> IorResult<GroupProfiles> {
    let mut out = GroupProfiles {
        uipmc: Vec::new(),
        iiop: Vec::new(),
        groups: Vec::new(),
    };
    for p in &ior.profiles {
        match p.tag {
            ProfileId::Uipmc => {
                let body = UipmcProfileBody::decode_encapsulation(&p.profile_data)?;
                if let Some(g) = body.group_info()? {
                    out.groups.push(g);
                }
                out.uipmc.push(body);
            }
            ProfileId::InternetIop => {
                let body = IiopProfileBody::decode_encapsulation(&p.profile_data)?;
                if let Some(c) = body.components.iter().find(|c| c.tag == TAG_GROUP) {
                    out.groups
                        .push(GroupInfo::decode_encapsulation(&c.component_data)?);
                }
                out.iiop.push(body);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `true` when `ior` is a group reference: one of its profiles carries a
/// `TAG_GROUP` component (§11.2.3). A reference whose profiles do not
/// decode is no group reference.
#[must_use]
pub fn is_group_reference(ior: &Ior) -> bool {
    group_profiles(ior).is_ok_and(|g| !g.groups.is_empty())
}

/// `CORBA::Object::is_equivalent` for references that may be group
/// references (§11.2.6.10): two references that are not group
/// references are equivalent when their type ids and profiles are
/// identical; a group reference is never equivalent to one that is not;
/// two group references are equivalent when they have as many profiles
/// and the group components of their profiles are identical.
#[must_use]
pub fn is_equivalent(a: &Ior, b: &Ior) -> bool {
    match (group_profiles(a), group_profiles(b)) {
        (Ok(ga), Ok(gb)) => match (ga.groups.is_empty(), gb.groups.is_empty()) {
            (true, true) => a == b,
            (false, false) => a.profiles.len() == b.profiles.len() && ga.groups == gb.groups,
            _ => false,
        },
        _ => a == b,
    }
}

/// `CORBA::Object::hash` following [`is_equivalent`] (§11.2.6.10): a value
/// in `0..=maximum` that equivalent references share.
#[must_use]
pub fn hash(ior: &Ior, maximum: u32) -> u32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    match group_profiles(ior) {
        Ok(g) if !g.groups.is_empty() => {
            feed(&(ior.profiles.len() as u64).to_be_bytes());
            for group in &g.groups {
                feed(group.group_domain_id.as_bytes());
                feed(&group.object_group_id.to_be_bytes());
                feed(&group.object_group_ref_version.to_be_bytes());
                feed(&[group.component_version.major, group.component_version.minor]);
            }
        }
        _ => {
            feed(ior.type_id.as_bytes());
            for p in &ior.profiles {
                feed(&p.tag.as_u32().to_be_bytes());
                feed(&p.profile_data);
            }
        }
    }
    if maximum == u32::MAX {
        (h % (u64::from(u32::MAX) + 1)) as u32
    } else {
        (h % (u64::from(maximum) + 1)) as u32
    }
}

/// `true` for a class D IPv4 address (224.0.0.0 to 239.255.255.255) or an
/// IPv6 multicast address (ff00::/8).
#[must_use]
pub fn is_multicast_address(address: &str) -> bool {
    if let Ok(v4) = address.parse::<core::net::Ipv4Addr>() {
        return v4.is_multicast();
    }
    address
        .parse::<core::net::Ipv6Addr>()
        .is_ok_and(|v6| v6.is_multicast())
}

fn bad_url(what: impl Into<String>) -> IorError {
    IorError::InvalidCorbalocAddress(what.into())
}

/// `<major>.<minor>`.
fn parse_dotted(s: &str) -> IorResult<IiopVersion> {
    let (maj, min) = s
        .split_once('.')
        .ok_or_else(|| bad_url(alloc::format!("version `{s}`")))?;
    let major = digits(maj)
        .and_then(|d| d.parse::<u8>().ok())
        .ok_or_else(|| bad_url(alloc::format!("version `{s}`")))?;
    let minor = digits(min)
        .and_then(|d| d.parse::<u8>().ok())
        .ok_or_else(|| bad_url(alloc::format!("version `{s}`")))?;
    Ok(IiopVersion::new(major, minor))
}

/// `s` when it is one or more decimal digits: the integer parsers of
/// the standard library also take a leading `+`, which the URL grammar
/// (§11.2.7, RFC 2396) does not.
fn digits(s: &str) -> Option<&str> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
}

/// `[<major>.<minor>@]rest` with the default 1.0.
fn split_version(s: &str) -> IorResult<(IiopVersion, &str)> {
    match s.split_once('@') {
        Some((v, rest)) => Ok((parse_dotted(v)?, rest)),
        None => Ok((IiopVersion::V1_0, s)),
    }
}

/// `host:port`, an IPv6 host in brackets.
fn split_host_port(s: &str) -> IorResult<(String, u16)> {
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, p) = rest
            .split_once(']')
            .ok_or_else(|| bad_url(alloc::format!("`{s}` lacks `]`")))?;
        let p = p
            .strip_prefix(':')
            .ok_or_else(|| bad_url(alloc::format!("`{s}` lacks the port")))?;
        (h, p)
    } else {
        s.rsplit_once(':')
            .ok_or_else(|| bad_url(alloc::format!("`{s}` lacks the port")))?
    };
    if host.is_empty() {
        return Err(bad_url(alloc::format!("`{s}` has no host")));
    }
    let port = digits(port)
        .and_then(|p| p.parse::<u16>().ok())
        .ok_or_else(|| bad_url(alloc::format!("port of `{s}`")))?;
    Ok((host.to_string(), port))
}

fn decode_key(s: &str) -> IorResult<Vec<u8>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Two hex digits (RFC 2396 §2.4.1); `from_str_radix` alone
            // would also take `+` and one digit.
            let hex = s
                .get(i + 1..i + 3)
                .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| bad_url(alloc::format!("`%` escape in `{s}`")))?;
            let v = u8::from_str_radix(hex, 16)
                .map_err(|_| bad_url(alloc::format!("`%{hex}` in `{s}`")))?;
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

/// `<group_id> = <component version>-<group_domain_id>-<object_group_id>[-<ref version>]`.
///
/// The domain may contain `-`; the numeric fields at the end are the
/// group id and, when there are two of them, the reference version.
fn parse_group_id(s: &str) -> IorResult<GroupInfo> {
    let bad = || bad_url(alloc::format!("group id `{s}`"));
    let (version, rest) = s.split_once('-').ok_or_else(bad)?;
    let component_version = parse_dotted(version)?;
    let fields: Vec<&str> = rest.split('-').collect();
    let numeric = |f: &str| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit());
    let n = fields.len();
    let (domain_end, group, ref_version) =
        if n >= 3 && numeric(fields[n - 1]) && numeric(fields[n - 2]) {
            (n - 2, fields[n - 2], Some(fields[n - 1]))
        } else if n >= 2 && numeric(fields[n - 1]) {
            (n - 1, fields[n - 1], None)
        } else {
            return Err(bad());
        };
    let group_domain_id = fields[..domain_end].join("-");
    if group_domain_id.is_empty() {
        return Err(bad());
    }
    let object_group_id = group.parse::<u64>().map_err(|_| bad())?;
    let object_group_ref_version = match ref_version {
        Some(v) => v.parse::<u32>().map_err(|_| bad())?,
        None => 0,
    };
    Ok(GroupInfo {
        component_version,
        group_domain_id,
        object_group_id,
        object_group_ref_version,
    })
}

/// Parses a `corbaloc` URL with an MIOP address (§11.2.7) into a group
/// reference:
///
/// ```text
/// corbaloc:miop:[<major>.<minor>@]<group_id>/<multicast address>:<port>
///          [;iiop:[<major>.<minor>@]<host>:<port>/<object key>]
///          [,iiop:[<major>.<minor>@]<host>:<port>]*[/<object key>]
/// <group_id> = <major>.<minor>-<group_domain_id>-<object_group_id>[-<ref version>]
/// ```
///
/// The MIOP address becomes a UIPMC profile with the `TAG_GROUP`
/// component and, after `;`, the `TAG_GROUP_IIOP` component; each further
/// `iiop` address becomes an IIOP profile of a gateway with the object
/// key at the end of the URL. An IPv6 multicast address is written in
/// brackets. Without a reference version the component carries 0. The
/// type id of the reference is empty: a URL does not carry one.
///
/// # Errors
/// `InvalidUrlScheme` for a URL that is not `corbaloc:` with an MIOP
/// address; `InvalidCorbalocAddress` for a malformed address, an address
/// that is not multicast, a missing port, a second MIOP address or IIOP
/// addresses without an object key.
pub fn parse_miop_corbaloc(url: &str) -> IorResult<Ior> {
    let payload = url
        .strip_prefix("corbaloc:")
        .ok_or_else(|| IorError::InvalidUrlScheme(url.into()))?;
    let mut uipmc = None;
    let mut gateways: Vec<(IiopVersion, String, u16)> = Vec::new();
    let mut key: Option<Vec<u8>> = None;
    let addrs: Vec<&str> = payload.split(',').collect();
    let last = addrs.len() - 1;
    for (i, addr) in addrs.iter().enumerate() {
        if let Some(miop) = addr.strip_prefix("miop:") {
            if uipmc.is_some() {
                return Err(bad_url("a second miop address"));
            }
            uipmc = Some(parse_miop_address(miop)?);
        } else if let Some(iiop) = addr.strip_prefix("iiop:") {
            let iiop = if i == last {
                match iiop.split_once('/') {
                    Some((a, k)) => {
                        key = Some(decode_key(k)?);
                        a
                    }
                    None => iiop,
                }
            } else {
                iiop
            };
            let (version, host_port) = split_version(iiop)?;
            let (host, port) = split_host_port(host_port)?;
            gateways.push((version, host, port));
        } else {
            return Err(bad_url(alloc::format!("address `{addr}`")));
        }
    }
    let uipmc = uipmc.ok_or_else(|| IorError::InvalidUrlScheme(url.into()))?;
    if !gateways.is_empty() && key.is_none() {
        return Err(bad_url("iiop addresses without an object key"));
    }
    let mut profiles = alloc::vec![uipmc.to_tagged_profile(Endianness::Big)?];
    for (version, host, port) in gateways {
        let body = IiopProfileBody::new(version, host, port, key.clone().unwrap_or_default());
        profiles.push(TaggedProfile::iiop(&body, Endianness::Big)?);
    }
    Ok(Ior::new(String::new(), profiles))
}

/// `<version><group_id>/<multicast address>:<port>[;<group_iiop>]`.
fn parse_miop_address(s: &str) -> IorResult<UipmcProfileBody> {
    let (main, group_iiop) = match s.split_once(';') {
        Some((m, g)) => (m, Some(g)),
        None => (s, None),
    };
    let (miop_version, rest) = split_version(main)?;
    let (group_id, multicast) = rest
        .split_once('/')
        .ok_or_else(|| bad_url(alloc::format!("`{s}` lacks the multicast address")))?;
    let group = parse_group_id(group_id)?;
    let (address, port) = split_host_port(multicast)?;
    if !is_multicast_address(&address) {
        return Err(bad_url(alloc::format!(
            "`{address}` is not a multicast address"
        )));
    }
    let mut components = alloc::vec![group.to_component(Endianness::Big)];
    if let Some(g) = group_iiop {
        let iiop = g
            .strip_prefix("iiop:")
            .ok_or_else(|| bad_url(alloc::format!("group iiop `{g}`")))?;
        let (version, rest) = split_version(iiop)?;
        let (host_port, k) = rest
            .split_once('/')
            .ok_or_else(|| bad_url(alloc::format!("group iiop `{g}` lacks the object key")))?;
        let (host, port) = split_host_port(host_port)?;
        let body = IiopProfileBody::new(version, host, port, decode_key(k)?);
        components.push(group_iiop_component(&body, Endianness::Big)?);
    }
    Ok(UipmcProfileBody {
        miop_version,
        the_address: address,
        the_port: port,
        components,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn group_ior() -> Ior {
        let gateway =
            IiopProfileBody::new(IiopVersion::V1_2, "gw.example".into(), 2809, b"gw".to_vec());
        let oma = IiopProfileBody::new(
            IiopVersion::V1_2,
            "oma.example".into(),
            1234,
            b"oma".to_vec(),
        );
        let uipmc = UipmcProfileBody::new(
            "225.1.1.8",
            5000,
            alloc::vec![
                GroupInfo::new("Shop", 7, 0).to_component(Endianness::Big),
                group_iiop_component(&oma, Endianness::Big).unwrap(),
            ],
        );
        Ior::new(
            "IDL:Shop/Till:1.0".into(),
            alloc::vec![
                uipmc.to_tagged_profile(Endianness::Big).unwrap(),
                TaggedProfile::iiop(&gateway, Endianness::Big).unwrap(),
            ],
        )
    }

    #[test]
    fn corba_p2_11_2_2_uipmc_profile_round_trips() {
        let body = UipmcProfileBody::new(
            "225.1.1.8",
            50_000,
            alloc::vec![GroupInfo::new("Dom", 1, 2).to_component(Endianness::Little)],
        );
        let be = body.encode_encapsulation(Endianness::Big).unwrap();
        // byte order, version 1.0, string length 10 at offset 4.
        assert_eq!(&be[..8], &[0, 1, 0, 0, 0, 0, 0, 10]);
        assert_eq!(&be[8..18], b"225.1.1.8\0");
        // the_port as two octets after alignment to 2: offset 18.
        assert_eq!(&be[18..20], &50_000u16.to_be_bytes());
        assert_eq!(UipmcProfileBody::decode_encapsulation(&be).unwrap(), body);
        let le = body.encode_encapsulation(Endianness::Little).unwrap();
        assert_eq!(UipmcProfileBody::decode_encapsulation(&le).unwrap(), body);
        let p = body.to_tagged_profile(Endianness::Big).unwrap();
        assert_eq!(p.tag.as_u32(), 3);
        assert_eq!(
            body.group_info().unwrap(),
            Some(GroupInfo::new("Dom", 1, 2))
        );
        // A component count beyond the octets.
        let mut bad = be.clone();
        let n = bad.len();
        bad.truncate(n - 3);
        assert!(UipmcProfileBody::decode_encapsulation(&bad).is_err());
    }

    #[test]
    fn corba_p2_11_2_2_4_group_info_layout() {
        let g = GroupInfo::new("D", 0x0102_0304_0506_0708, 9);
        let bytes = g.encode_encapsulation(Endianness::Big);
        assert_eq!(
            bytes,
            alloc::vec![
                0, 1, 0, 0, // byte order, version 1.0, padding
                0, 0, 0, 2, b'D', 0, // domain
                0, 0, 0, 0, 0, 0, // padding to 16
                1, 2, 3, 4, 5, 6, 7, 8, // object_group_id
                0, 0, 0, 9, // object_group_ref_version
            ]
        );
        assert_eq!(GroupInfo::decode_encapsulation(&bytes).unwrap(), g);
        assert_eq!(g.to_component(Endianness::Big).tag, 39);
    }

    #[test]
    fn corba_p2_11_2_3_a_group_reference_and_its_profiles() {
        let ior = group_ior();
        assert!(is_group_reference(&ior));
        let g = group_profiles(&ior).unwrap();
        assert_eq!(g.uipmc.len(), 1);
        assert_eq!(g.iiop.len(), 1);
        assert_eq!(g.groups, alloc::vec![GroupInfo::new("Shop", 7, 0)]);
        let oma = g.uipmc[0].group_iiop_profile().unwrap().unwrap();
        assert_eq!(oma.host, "oma.example");
        assert_eq!(oma.object_key, b"oma");
        let plain = Ior::new(
            "IDL:X:1.0".into(),
            alloc::vec![
                TaggedProfile::iiop(
                    &IiopProfileBody::new(IiopVersion::V1_2, "h".into(), 1, b"k".to_vec()),
                    Endianness::Big
                )
                .unwrap()
            ],
        );
        assert!(!is_group_reference(&plain));
    }

    #[test]
    fn corba_p2_11_2_6_10_is_equivalent_and_hash_of_group_references() {
        let a = group_ior();
        // Same group, other gateway and other type id: equivalent.
        let mut b = group_ior();
        b.type_id = String::new();
        b.profiles[1] = TaggedProfile::iiop(
            &IiopProfileBody::new(IiopVersion::V1_2, "other".into(), 1, b"x".to_vec()),
            Endianness::Little,
        )
        .unwrap();
        assert!(is_equivalent(&a, &b));
        assert_eq!(hash(&a, 1000), hash(&b, 1000));
        assert!(hash(&a, 1000) <= 1000);
        // Another number of profiles.
        let mut c = group_ior();
        c.profiles.pop();
        assert!(!is_equivalent(&a, &c));
        // Another group id.
        let mut d = group_ior();
        d.profiles[0] = UipmcProfileBody::new(
            "225.1.1.8",
            5000,
            alloc::vec![GroupInfo::new("Shop", 8, 0).to_component(Endianness::Big)],
        )
        .to_tagged_profile(Endianness::Big)
        .unwrap();
        assert!(!is_equivalent(&a, &d));
        // A group reference and a plain one.
        let mut plain = group_ior();
        plain.profiles.remove(0);
        assert!(!is_equivalent(&a, &plain));
        assert!(is_equivalent(&plain, &plain.clone()));
    }

    #[test]
    fn corba_p2_11_2_7_corbaloc_miop_url_parses() {
        let ior = parse_miop_corbaloc(
            "corbaloc:miop:1.0@1.0-MyLIttleDomin-1/225.1.1.8:5000;iiop:1.1@oma_host:1234/object_key,iiop:1.2@gateway_host:1234/object_key",
        )
        .unwrap();
        let g = group_profiles(&ior).unwrap();
        assert_eq!(g.uipmc.len(), 1);
        let u = &g.uipmc[0];
        assert_eq!(u.miop_version, IiopVersion::V1_0);
        assert_eq!(u.the_address, "225.1.1.8");
        assert_eq!(u.the_port, 5000);
        assert_eq!(g.groups, alloc::vec![GroupInfo::new("MyLIttleDomin", 1, 0)]);
        let oma = u.group_iiop_profile().unwrap().unwrap();
        assert_eq!(
            (oma.iiop_version, oma.host.as_str(), oma.port),
            (IiopVersion::V1_1, "oma_host", 1234)
        );
        assert_eq!(oma.object_key, b"object_key");
        assert_eq!(g.iiop.len(), 1);
        assert_eq!(g.iiop[0].host, "gateway_host");
        assert_eq!(g.iiop[0].iiop_version, IiopVersion::V1_2);
        assert_eq!(g.iiop[0].object_key, b"object_key");
        assert!(ior.type_id.is_empty());

        // Without versions: 1.0; a domain with dashes and a reference
        // version; an IPv6 group.
        let v6 = parse_miop_corbaloc("corbaloc:miop:1.0-my-dom-42-3/[ff15::1:2]:7000").unwrap();
        let g = group_profiles(&v6).unwrap();
        assert_eq!(g.groups, alloc::vec![GroupInfo::new("my-dom", 42, 3)]);
        assert_eq!(g.uipmc[0].the_address, "ff15::1:2");
        assert_eq!(g.uipmc[0].the_port, 7000);
        assert_eq!(v6.profiles.len(), 1);
    }

    #[test]
    fn corba_p2_11_2_7_malformed_miop_urls_are_refused() {
        for url in [
            "corbaloc:miop:1.0-D-1/10.0.0.1:5000", // not multicast
            "corbaloc:miop:1.0-D-1/225.1.1.1",     // no port
            "corbaloc:miop:1.0-D-x/225.1.1.1:1",   // group id not numeric
            "corbaloc:miop:D-1/225.1.1.1:1",       // no component version
            "corbaloc:miop:1.0--1/225.1.1.1:1",    // empty domain
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,miop:1.0-D-2/225.1.1.2:1",
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:host:1", // gateway without key
            "corbaloc:miop:1.0-D-1/225.1.1.1:1;iiop:h:1",    // group iiop without key
            "corbaloc:miop:1.0-D-1/[ff15::1:1",              // no `]`
            "corbaloc:iiop:host:1/key",                      // no miop address
            "corbaname:miop:1.0-D-1/225.1.1.1:1",
            // RFC 2396 §2.4.1: an escape is `%` and two hex digits; the
            // numbers of §11.2.7 are digits without a sign.
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:h:1/%+1",
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:h:1/%-1",
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:h:1/%1",
            "corbaloc:miop:1.0-D-1/225.1.1.1:+1",
            "corbaloc:miop:+1.0-D-1/225.1.1.1:1",
            "corbaloc:miop:1.+0-D-1/225.1.1.1:1",
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:h:+1/k",
            "corbaloc:miop:1.0-D-1/225.1.1.1:1,iiop:+1.2@h:1/k",
        ] {
            assert!(parse_miop_corbaloc(url).is_err(), "{url}");
        }
    }

    #[test]
    fn corba_p2_11_2_2_2_multicast_addresses() {
        assert!(is_multicast_address("224.0.0.0"));
        assert!(is_multicast_address("239.255.255.255"));
        assert!(!is_multicast_address("240.0.0.1"));
        assert!(!is_multicast_address("192.168.1.1"));
        assert!(is_multicast_address("FF01:0:0:0:0:0:0:1"));
        assert!(!is_multicast_address("fe80::1"));
        assert!(!is_multicast_address("group.alias"));
    }
}
