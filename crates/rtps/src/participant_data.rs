// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ParticipantBuiltinTopicData (DDSI-RTPS 2.5 §8.5.4.2).
//!
//! Content of the SPDP beacon DATA submessage. Carried as a PL_CDR_LE-encoded
//! ParameterList in the `serialized_payload` of the DATA submessage
//! (with a 4-byte encapsulation header).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::property_list::WirePropertyList;
use crate::wire_types::{Guid, Locator, LocatorKind, ProtocolVersion, VendorId};

/// PL_CDR_LE Encapsulation-Kind (Spec §10.2).
pub const ENCAPSULATION_PL_CDR_LE: [u8; 2] = [0x00, 0x03];

/// `BuiltinEndpointSet` bitmask flags (DDSI-RTPS 2.5 §9.3.2.12,
/// table 9.4 + 9.5; DDS-Security 1.2 §7.5.1.4, Table 11 for bits
/// 16..27). Exchanged via the `PID_BUILTIN_ENDPOINT_SET` (0x0058) PID
/// in the SPDP `ParticipantBuiltinTopicData` as a 32-bit bitmask.
///
/// Bits 6..9 are spec-reserved (historical participant-proxy features
/// in DDSI 2.1 / topics from 2.5 are not assigned here). Bits 16..27
/// are the secure-discovery endpoints from DDS-Security. Bits 28..29
/// are the XTypes topics-discovery endpoints. Bits 30..31 are
/// spec-reserved.
///
/// Cyclone DDS and Fast-DDS create their SEDP proxies based on these
/// bits — if a bit is missing, the peer does not build the
/// corresponding reader/writer and endpoint discovery fails.
/// Therefore we MUST announce all endpoints we offer locally in the
/// `builtin_endpoint_set`.
pub mod endpoint_flag {
    // ---------------------------------------------------------------
    // Standard discovery endpoints (DDSI-RTPS 2.5 §9.3.2.12, tab. 9.4)
    // ---------------------------------------------------------------

    /// Participant SPDP Writer announce (Bit 0).
    pub const PARTICIPANT_ANNOUNCER: u32 = 1 << 0;
    /// Participant SPDP Reader detector (Bit 1).
    pub const PARTICIPANT_DETECTOR: u32 = 1 << 1;
    /// SEDP Publications Writer (Bit 2).
    pub const PUBLICATIONS_ANNOUNCER: u32 = 1 << 2;
    /// SEDP Publications Reader (Bit 3).
    pub const PUBLICATIONS_DETECTOR: u32 = 1 << 3;
    /// SEDP Subscriptions Writer (Bit 4).
    pub const SUBSCRIPTIONS_ANNOUNCER: u32 = 1 << 4;
    /// SEDP Subscriptions Reader (Bit 5).
    pub const SUBSCRIPTIONS_DETECTOR: u32 = 1 << 5;

    // ---------------------------------------------------------------
    // Writer-Liveliness-Protocol (DDSI-RTPS 2.5 §8.4.13, §9.3.2.12)
    // ---------------------------------------------------------------

    /// `PARTICIPANT_MESSAGE_DATA_WRITER` — sends WLP heartbeats
    /// (`ParticipantMessageData`) on the topic
    /// `DCPSParticipantMessage` (bit 10, RTPS 2.5 §8.4.13).
    pub const PARTICIPANT_MESSAGE_DATA_WRITER: u32 = 1 << 10;
    /// `PARTICIPANT_MESSAGE_DATA_READER` — receives WLP heartbeats
    /// (bit 11, RTPS 2.5 §8.4.13).
    pub const PARTICIPANT_MESSAGE_DATA_READER: u32 = 1 << 11;

    // ---------------------------------------------------------------
    // TypeLookup-Service (XTypes 1.3 §7.6.3.3.4)
    // ---------------------------------------------------------------

    /// TypeLookup request writer (bit 12), used by a querying client.
    pub const TYPE_LOOKUP_REQUEST_DATA_WRITER: u32 = 1 << 12;
    /// TypeLookup request reader (bit 13), used by a responding service.
    pub const TYPE_LOOKUP_REQUEST_DATA_READER: u32 = 1 << 13;
    /// TypeLookup reply writer (bit 14), used by a responding service.
    pub const TYPE_LOOKUP_REPLY_DATA_WRITER: u32 = 1 << 14;
    /// TypeLookup reply reader (bit 15), used by a querying client.
    pub const TYPE_LOOKUP_REPLY_DATA_READER: u32 = 1 << 15;
    /// Both TypeLookup request endpoints, not a single wire bit.
    pub const TYPE_LOOKUP_REQUEST: u32 =
        TYPE_LOOKUP_REQUEST_DATA_WRITER | TYPE_LOOKUP_REQUEST_DATA_READER;
    /// Both TypeLookup reply endpoints, not a single wire bit.
    pub const TYPE_LOOKUP_REPLY: u32 =
        TYPE_LOOKUP_REPLY_DATA_WRITER | TYPE_LOOKUP_REPLY_DATA_READER;

    // ---------------------------------------------------------------
    // DDS-Security 1.2 §7.5.1.4, Table 11 — secure-discovery endpoints
    // (bits 16..27). Doc comments reference the spec constant names
    // (`DISC_BUILTIN_ENDPOINT_*`) for cross-crate audits with the
    // `zerodds-security-rtps` crate.
    // ---------------------------------------------------------------

    /// `DISC_BUILTIN_ENDPOINT_PUBLICATIONS_SECURE_WRITER` — Secure
    /// SEDP Publications Writer (Bit 16, DDS-Security 1.2 §7.4.7.1).
    pub const PUBLICATIONS_SECURE_WRITER: u32 = 1 << 16;
    /// `DISC_BUILTIN_ENDPOINT_PUBLICATIONS_SECURE_READER` — Secure
    /// SEDP Publications Reader (Bit 17, DDS-Security 1.2 §7.4.7.1).
    pub const PUBLICATIONS_SECURE_READER: u32 = 1 << 17;
    /// `DISC_BUILTIN_ENDPOINT_SUBSCRIPTIONS_SECURE_WRITER` — Secure
    /// SEDP Subscriptions Writer (Bit 18, DDS-Security 1.2 §7.4.7.1).
    pub const SUBSCRIPTIONS_SECURE_WRITER: u32 = 1 << 18;
    /// `DISC_BUILTIN_ENDPOINT_SUBSCRIPTIONS_SECURE_READER` — Secure
    /// SEDP Subscriptions Reader (Bit 19, DDS-Security 1.2 §7.4.7.1).
    pub const SUBSCRIPTIONS_SECURE_READER: u32 = 1 << 19;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_SECURE_WRITER` — Secure
    /// WLP-Writer (Bit 20, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_MESSAGE_SECURE_WRITER: u32 = 1 << 20;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_SECURE_READER` — Secure
    /// WLP-Reader (Bit 21, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_MESSAGE_SECURE_READER: u32 = 1 << 21;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_STATELESS_MESSAGE_WRITER` —
    /// Auth-Stateless-Writer (Bit 22, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_STATELESS_MESSAGE_WRITER: u32 = 1 << 22;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_STATELESS_MESSAGE_READER` —
    /// Auth-Stateless-Reader (Bit 23, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_STATELESS_MESSAGE_READER: u32 = 1 << 23;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`
    /// — Crypto-KeyExchange-Writer (Bit 24, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER: u32 = 1 << 24;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER`
    /// — Crypto-KeyExchange-Reader (Bit 25, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER: u32 = 1 << 25;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_SECURE_WRITER` — DCPSParticipants-
    /// Secure-Writer (Bit 26, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_SECURE_WRITER: u32 = 1 << 26;
    /// `BUILTIN_ENDPOINT_PARTICIPANT_SECURE_READER` — DCPSParticipants-
    /// Secure-Reader (Bit 27, DDS-Security 1.2 §7.4.7.1).
    pub const PARTICIPANT_SECURE_READER: u32 = 1 << 27;

    // ---------------------------------------------------------------
    // XTypes-Topics-Discovery (DDSI-RTPS 2.5 §9.3.2.12)
    // ---------------------------------------------------------------

    /// `DISC_BUILTIN_ENDPOINT_TOPICS_ANNOUNCER` — Topics-Builtin-
    /// Topic-Announcer (Bit 28, RTPS 2.5 §9.3.2.12).
    pub const TOPICS_ANNOUNCER: u32 = 1 << 28;
    /// `DISC_BUILTIN_ENDPOINT_TOPICS_DETECTOR` — Topics-Builtin-
    /// Topic-Detector (Bit 29, RTPS 2.5 §9.3.2.12).
    pub const TOPICS_DETECTOR: u32 = 1 << 29;

    // ---------------------------------------------------------------
    // Convenience bundles
    // ---------------------------------------------------------------

    /// Mask of all 12 secure-discovery bits (16..27). Mixed in by the
    /// DCPS runtime when the `security` feature is active.
    pub const ALL_SECURE: u32 = PUBLICATIONS_SECURE_WRITER
        | PUBLICATIONS_SECURE_READER
        | SUBSCRIPTIONS_SECURE_WRITER
        | SUBSCRIPTIONS_SECURE_READER
        | PARTICIPANT_MESSAGE_SECURE_WRITER
        | PARTICIPANT_MESSAGE_SECURE_READER
        | PARTICIPANT_STATELESS_MESSAGE_WRITER
        | PARTICIPANT_STATELESS_MESSAGE_READER
        | PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
        | PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
        | PARTICIPANT_SECURE_WRITER
        | PARTICIPANT_SECURE_READER;

    /// Mask of all standard bits (0..5, 10..15) without security and
    /// without SEDP topics. Represents the ZeroDDS standard discovery
    /// capability for a participant without the `security` feature.
    /// Includes all four TypeLookup endpoints (bits 12..15, XTypes 1.3
    /// §7.6.3.3.4).
    ///
    /// SEDP topics endpoints (bits 28/29) are optional per RTPS 2.5
    /// §8.5.4.4. Since ZeroDDS derives DCPSTopic samples synthetically
    /// from publications/subscriptions, we do not announce the topics
    /// capability — bits 28/29 would promise peers a non-existent
    /// endpoint pairing. Vendors that implement the native topic
    /// endpoints themselves can mix
    /// [`TOPICS_ANNOUNCER`]/[`TOPICS_DETECTOR`] into the mask.
    pub const ALL_STANDARD: u32 = PARTICIPANT_ANNOUNCER
        | PARTICIPANT_DETECTOR
        | PUBLICATIONS_ANNOUNCER
        | PUBLICATIONS_DETECTOR
        | SUBSCRIPTIONS_ANNOUNCER
        | SUBSCRIPTIONS_DETECTOR
        | PARTICIPANT_MESSAGE_DATA_WRITER
        | PARTICIPANT_MESSAGE_DATA_READER
        | TYPE_LOOKUP_REQUEST
        | TYPE_LOOKUP_REPLY;
}

/// Duration_t (Spec §9.4.2.2): seconds + nanoseconds.
///
/// Canonical definition lives in [`zerodds_qos::Duration`]; RTPS
/// re-exports the type here for backward compatibility. All call sites
/// use the qos type.
pub use zerodds_qos::Duration;

/// SPDP discovered-participant data. Subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantBuiltinTopicData {
    /// GUID of the participant.
    pub guid: Guid,
    /// Protocol version (typically 2.5).
    pub protocol_version: ProtocolVersion,
    /// Vendor-Identifier.
    pub vendor_id: VendorId,
    /// Default unicast locators — where peers send user data. A list
    /// (DDSI-RTPS 2.5 §9.6.1: `PID_DEFAULT_UNICAST_LOCATOR` may repeat); on a
    /// multi-homed host every usable address is announced so a peer on any
    /// shared segment finds a reachable one (#27). Empty = none announced.
    /// Wire order preserved; exact duplicates removed on encode.
    pub default_unicast_locators: Vec<Locator>,
    /// Default multicast locators — user-data multicast groups.
    pub default_multicast_locators: Vec<Locator>,
    /// Metatraffic unicast locators — where peers send SEDP unicast.
    /// Indispensable for SEDP interop (e.g. Cyclone): Cyclone routes
    /// publications/subscriptions to these locators after a match. Announced
    /// as the full multi-homed set (#27).
    pub metatraffic_unicast_locators: Vec<Locator>,
    /// Metatraffic multicast locators — the SPDP/SEDP multicast groups.
    pub metatraffic_multicast_locators: Vec<Locator>,
    /// DDS domain ID. Cyclone filters beacons from other domains; if the
    /// PID is missing, domain 0 is usually assumed.
    pub domain_id: Option<u32>,
    /// DDSI-RTPS 2.5 §8.5.3.2 `domainTag`: participants of one domain with
    /// different tags do not discover each other. Empty is the default tag,
    /// and an absent `PID_DOMAIN_TAG` means it; a non-empty tag is sent.
    pub domain_tag: alloc::string::String,
    /// Bitmask of available builtin endpoints
    /// (see [`endpoint_flag`]).
    pub builtin_endpoint_set: u32,
    /// How long the participant is considered "alive" without a renewed
    /// beacon.
    pub lease_duration: Duration,
    /// UserData QoS at the participant (spec §2.2.3.1) — opaque
    /// `sequence<octet>`, propagated over SPDP.
    pub user_data: Vec<u8>,
    /// Property list (`PID_PROPERTY_LIST`, 0x0059). Carrier for
    /// security-plugin classes, permissions tokens and ZeroDDS
    /// heterogeneous-security caps (WP 4H-b). Empty for peers without
    /// security announcements (legacy compatibility).
    pub properties: WirePropertyList,
    /// Raw CDR-encoded `IdentityToken` blob (DDS-Security 1.2 §7.5.1.4 Table 10
    /// Tab.16, `PID_IDENTITY_TOKEN`=0x1001). Parsed by the DDS-Security
    /// layer (`zerodds_security::token::DataHolder`) — RTPS only passes
    /// the bytes through, so this crate stays transport-free.
    /// `None` for legacy peers without a security announce.
    pub identity_token: Option<Vec<u8>>,
    /// Raw CDR-encoded `PermissionsToken` blob (DDS-Security 1.2
    /// §7.5.1.4 Table 10, `PID_PERMISSIONS_TOKEN`=0x1002).
    pub permissions_token: Option<Vec<u8>>,
    /// Raw CDR-encoded `IdentityStatusToken` blob (DDS-Security 1.2
    /// §7.5.1.6 Table 14, `PID_IDENTITY_STATUS_TOKEN`=0x1006). Optional, carries
    /// the OCSP live status.
    pub identity_status_token: Option<Vec<u8>>,
    /// `ParticipantSecurityDigitalSignatureAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.11, `PID=0x1010`). Optional — `None` = spec default
    /// (RSASSA-PSS + ECDSA-P256).
    pub sig_algo_info:
        Option<crate::security_algo_info::ParticipantSecurityDigitalSignatureAlgorithmInfo>,
    /// `ParticipantSecurityKeyEstablishmentAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.12, `PID=0x1011`). Optional — `None` = spec default
    /// (DHE-MODP-2048 + ECDHE-CEUM-P256).
    pub kx_algo_info:
        Option<crate::security_algo_info::ParticipantSecurityKeyEstablishmentAlgorithmInfo>,
    /// `ParticipantSecuritySymmetricCipherAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.13, `PID=0x1012`). Optional — `None` = spec default
    /// (AES128|AES256 supported, AES128 required).
    pub sym_cipher_algo_info:
        Option<crate::security_algo_info::ParticipantSecuritySymmetricCipherAlgorithmInfo>,
    /// `available_builtin_endpoints_ext` (DDS-Security 1.2 §7.5.1.4
    /// Table 10, `PID=0x1007`, bits in [`builtin_endpoint_ext`]). Not sent
    /// when 0; a participant that sends none has 0 ("the receiver shall
    /// interpret the value of the member to be 0x00000000").
    pub available_builtin_endpoints_ext: u32,
    /// `ParticipantSecurityInfo` (DDS-Security 1.2 §7.5.1.4 Table 10,
    /// `PID_PARTICIPANT_SECURITY_INFO`=0x1005). Two attribute bitmasks at
    /// participant level. **Mandatory for cross-vendor interop**: foreign
    /// vendors (cyclone/FastDDS) classify a participant WITHOUT this PID as
    /// NON-SECURE and reject its endpoints. `None` = legacy/plain.
    pub participant_security_info:
        Option<crate::participant_security_info::ParticipantSecurityInfo>,
    /// `PID_EXPECTS_INLINE_QOS` (0x0043) in SPDP: the readers of this
    /// participant expect the writer's QoS in-line with every DATA
    /// (DDSI-RTPS 2.5 §8.5.3.2). Absent is `false` (Table 9.19), and
    /// this tree's participants announce nothing.
    pub expects_inline_qos: bool,
    /// `PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT` (0x0034,
    /// `ParticipantProxy::manualLivelinessCount`, DDSI-RTPS 2.5 Table 9.19):
    /// "When liveliness is asserted, the manualLivelinessCount is incremented
    /// and a new SPDPdiscoveredParticipantData is sent" (Table 8.78). `None`
    /// when the participant announces none.
    pub manual_liveliness_count: Option<i32>,
    /// `PID_ENTITY_NAME` (0x0062, DDSI-RTPS 2.5 Table 9.18, `EntityName_t`):
    /// the name of the participant. `None` when it announces none.
    pub entity_name: Option<alloc::string::String>,
    /// `PID_BUILTIN_ENDPOINT_QOS` (0x0077, `ParticipantProxy::builtinEndpointQos`)
    /// as the bitmask of [`builtin_endpoint_qos`]. `None` when the participant
    /// announces none: its builtin readers are then RELIABLE (§8.4.13.3).
    pub builtin_endpoint_qos: Option<u32>,
}

pub mod builtin_endpoint_ext {
    //! `available_builtin_endpoints_ext` (DDS-Security 1.2 §7.5.1.4
    //! Table 12, `PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT` 0x1007): the bits of
    //! the secure TypeLookup endpoints.

    /// `TypeLookupServiceRequestSecureWriter` (bit 0).
    pub const TYPE_LOOKUP_REQUEST_SECURE_WRITER: u32 = 1 << 0;
    /// `TypeLookupServiceRequestSecureReader` (bit 1).
    pub const TYPE_LOOKUP_REQUEST_SECURE_READER: u32 = 1 << 1;
    /// `TypeLookupServiceReplySecureWriter` (bit 2).
    pub const TYPE_LOOKUP_REPLY_SECURE_WRITER: u32 = 1 << 2;
    /// `TypeLookupServiceReplySecureReader` (bit 3).
    pub const TYPE_LOOKUP_REPLY_SECURE_READER: u32 = 1 << 3;
    /// All four secure TypeLookup bits.
    pub const ALL_TYPE_LOOKUP_SECURE: u32 = TYPE_LOOKUP_REQUEST_SECURE_WRITER
        | TYPE_LOOKUP_REQUEST_SECURE_READER
        | TYPE_LOOKUP_REPLY_SECURE_WRITER
        | TYPE_LOOKUP_REPLY_SECURE_READER;
}

/// `BuiltinEndpointQos_t` bits (DDSI-RTPS 2.5 §9.3.2.13).
pub mod builtin_endpoint_qos {
    /// `BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER` (`@position(0)`): the
    /// BuiltinParticipantMessageReader of the participant is BEST_EFFORT
    /// (§8.4.13.3).
    pub const BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER: u32 = 1 << 0;
}

impl ParticipantBuiltinTopicData {
    /// First announced default-unicast locator, if any. Ergonomic accessor
    /// for call sites that only need one representative locator (e.g. logging
    /// or a legacy single-destination path). Prefer iterating the full list
    /// for actual transmission (fan-out).
    #[must_use]
    pub fn primary_default_unicast_locator(&self) -> Option<Locator> {
        self.default_unicast_locators.first().copied()
    }

    /// First announced metatraffic-unicast locator, if any. See
    /// [`Self::primary_default_unicast_locator`].
    #[must_use]
    pub fn primary_metatraffic_unicast_locator(&self) -> Option<Locator> {
        self.metatraffic_unicast_locators.first().copied()
    }

    /// The full set of unicast locators a peer should use to reach this
    /// participant's metatraffic (SEDP): the announced metatraffic list, or —
    /// if none was announced — the default-unicast list (DDSI-RTPS 2.5
    /// §8.5.4.2 fallback). Returns **all** of them so the caller can fan out
    /// to every advertised destination (#27); the list is empty only when the
    /// participant announced no unicast locator at all.
    #[must_use]
    pub fn metatraffic_or_default_unicast_locators(&self) -> Vec<Locator> {
        if self.metatraffic_unicast_locators.is_empty() {
            self.default_unicast_locators.clone()
        } else {
            self.metatraffic_unicast_locators.clone()
        }
    }

    /// Encodes to PL_CDR_LE bytes (with a 4-byte encapsulation header).
    /// The output is directly usable as the `serialized_payload` of a DATA
    /// submessage.
    #[must_use]
    pub fn to_pl_cdr_le(&self) -> Vec<u8> {
        let mut params = ParameterList::new();

        // PROTOCOL_VERSION: 2 byte + 2 padding
        let mut pv = Vec::with_capacity(4);
        pv.extend_from_slice(&self.protocol_version.to_bytes());
        pv.extend_from_slice(&[0, 0]);
        params.push(Parameter::new(pid::PROTOCOL_VERSION, pv));

        // VENDOR_ID: 2 byte + 2 padding
        let mut vid = Vec::with_capacity(4);
        vid.extend_from_slice(&self.vendor_id.to_bytes());
        vid.extend_from_slice(&[0, 0]);
        params.push(Parameter::new(pid::VENDOR_ID, vid));

        // PARTICIPANT_GUID: 16 byte
        params.push(Parameter::new(
            pid::PARTICIPANT_GUID,
            self.guid.to_bytes().to_vec(),
        ));

        // Locator lists: one PID per locator, wire order preserved, exact
        // duplicates removed (24 byte each, LE).
        for (id, locs) in [
            (pid::DEFAULT_UNICAST_LOCATOR, &self.default_unicast_locators),
            (
                pid::DEFAULT_MULTICAST_LOCATOR,
                &self.default_multicast_locators,
            ),
            (
                pid::METATRAFFIC_UNICAST_LOCATOR,
                &self.metatraffic_unicast_locators,
            ),
            (
                pid::METATRAFFIC_MULTICAST_LOCATOR,
                &self.metatraffic_multicast_locators,
            ),
        ] {
            let mut seen: Vec<Locator> = Vec::new();
            for loc in locs {
                if seen.contains(loc) {
                    continue;
                }
                seen.push(*loc);
                params.push(Parameter::new(id, loc.to_bytes_le().to_vec()));
            }
        }

        // DOMAIN_ID (optional): 4 byte u32
        if let Some(dom) = self.domain_id {
            params.push(Parameter::new(pid::DOMAIN_ID, dom.to_le_bytes().to_vec()));
        }
        if !self.domain_tag.is_empty() {
            params.push(Parameter::new(
                pid::DOMAIN_TAG,
                cdr_string(&self.domain_tag, true),
            ));
        }

        // BUILTIN_ENDPOINT_SET: 4 byte u32
        params.push(Parameter::new(
            pid::BUILTIN_ENDPOINT_SET,
            self.builtin_endpoint_set.to_le_bytes().to_vec(),
        ));

        // LEASE_DURATION: 8 byte
        params.push(Parameter::new(
            pid::PARTICIPANT_LEASE_DURATION,
            self.lease_duration.to_bytes_le().to_vec(),
        ));

        // USER_DATA — opaque sequence<octet>, only if set.
        if !self.user_data.is_empty() {
            if let Ok(v) = crate::publication_data::encode_octet_seq_le(&self.user_data) {
                params.push(Parameter::new(pid::USER_DATA, v));
            }
        }

        // IDENTITY_TOKEN / PERMISSIONS_TOKEN / IDENTITY_STATUS_TOKEN
        // (DDS-Security 1.2 §7.5.1.4, Table 10). The caller has already
        // CDR-encoded the DataHolder — we pass the bytes through.
        if let Some(blob) = self.identity_token.as_ref() {
            params.push(Parameter::new(pid::IDENTITY_TOKEN, blob.clone()));
        }
        if let Some(blob) = self.permissions_token.as_ref() {
            params.push(Parameter::new(pid::PERMISSIONS_TOKEN, blob.clone()));
        }
        if let Some(blob) = self.identity_status_token.as_ref() {
            params.push(Parameter::new(pid::IDENTITY_STATUS_TOKEN, blob.clone()));
        }
        // PID_PARTICIPANT_SECURITY_INFO (0x1005, §7.4.1.6) — 8 Byte LE.
        // Without this PID, cyclone/FastDDS classify the participant as
        // non-secure and reject its endpoints.
        if let Some(psi) = self.participant_security_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_INFO,
                psi.to_wire_le(),
            ));
        }

        // Algorithm-info PIDs (spec §7.3.11-13, C3.5 remainder). The
        // default (None) is NOT sent — the receiver uses the spec default.
        if let Some(sig) = self.sig_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO,
                sig.to_bytes(true).to_vec(),
            ));
        }
        if let Some(kx) = self.kx_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO,
                kx.to_bytes(true).to_vec(),
            ));
        }
        if let Some(sym) = self.sym_cipher_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO,
                sym.to_bytes(true).to_vec(),
            ));
        }
        if self.available_builtin_endpoints_ext != 0 {
            params.push(Parameter::new(
                pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT,
                self.available_builtin_endpoints_ext.to_le_bytes().to_vec(),
            ));
        }
        // FALSE is the default (Table 9.19) and is left out.
        if self.expects_inline_qos {
            params.push(Parameter::new(
                pid::EXPECTS_INLINE_QOS,
                crate::publication_data::boolean_value(true),
            ));
        }
        if let Some(count) = self.manual_liveliness_count {
            params.push(Parameter::new(
                pid::PARTICIPANT_MANUAL_LIVELINESS_COUNT,
                count.to_le_bytes().to_vec(),
            ));
        }
        if let Some(name) = &self.entity_name {
            params.push(Parameter::new(pid::ENTITY_NAME, cdr_string(name, true)));
        }
        if let Some(qos) = self.builtin_endpoint_qos {
            params.push(Parameter::new(
                pid::BUILTIN_ENDPOINT_QOS,
                qos.to_le_bytes().to_vec(),
            ));
        }

        // PROPERTY_LIST (optional — only if non-empty). The PropertyList
        // encoder must not fail if the bytes conform to MAX_PROPERTIES;
        // on overflow silently omit it, so the SPDP-beacon encoding
        // never fails. The caller must apply caps before filling.
        if !self.properties.is_empty() {
            if let Ok(bytes) = self.properties.encode(true) {
                params.push(Parameter::new(pid::PROPERTY_LIST, bytes));
            }
        }

        // Encapsulation header: 4 byte (PL_CDR_LE + options=0).
        let mut out = Vec::new();
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        out
    }

    /// Encodes to PL_CDR_**BE** bytes (encapsulation 0x0002, all values
    /// big-endian). For the handshake `c.pdata` (DDS-Security §9.3.2.5.2:
    /// "CDR Big-Endian serialization of the ParticipantBuiltinTopicData") —
    /// cyclone/FastDDS deserialize c.pdata strictly as a BE ParameterList;
    /// LE yields "Deserialize parameter failed: payload too long".
    ///
    /// The credential tokens (identity_token/permissions_token/
    /// identity_status_token) are OMITTED here: they are LE-encoded
    /// DataHolder blobs and travel separately in the handshake as
    /// c.id/c.perm — Cyclone's c.pdata also does not contain them.
    #[must_use]
    pub fn to_pl_cdr_be(&self) -> Vec<u8> {
        let mut params = ParameterList::new();

        let mut pv = Vec::with_capacity(4);
        pv.extend_from_slice(&self.protocol_version.to_bytes());
        pv.extend_from_slice(&[0, 0]);
        params.push(Parameter::new(pid::PROTOCOL_VERSION, pv));

        let mut vid = Vec::with_capacity(4);
        vid.extend_from_slice(&self.vendor_id.to_bytes());
        vid.extend_from_slice(&[0, 0]);
        params.push(Parameter::new(pid::VENDOR_ID, vid));

        params.push(Parameter::new(
            pid::PARTICIPANT_GUID,
            self.guid.to_bytes().to_vec(),
        ));

        // One PID per locator, wire order preserved, exact duplicates removed
        // (a repeated locator carries no information and only bloats the beacon).
        for (id, locs) in [
            (pid::DEFAULT_UNICAST_LOCATOR, &self.default_unicast_locators),
            (
                pid::DEFAULT_MULTICAST_LOCATOR,
                &self.default_multicast_locators,
            ),
            (
                pid::METATRAFFIC_UNICAST_LOCATOR,
                &self.metatraffic_unicast_locators,
            ),
            (
                pid::METATRAFFIC_MULTICAST_LOCATOR,
                &self.metatraffic_multicast_locators,
            ),
        ] {
            let mut seen: Vec<Locator> = Vec::new();
            for loc in locs {
                if seen.contains(loc) {
                    continue;
                }
                seen.push(*loc);
                params.push(Parameter::new(id, loc.to_bytes_be().to_vec()));
            }
        }

        if let Some(dom) = self.domain_id {
            params.push(Parameter::new(pid::DOMAIN_ID, dom.to_be_bytes().to_vec()));
        }
        if !self.domain_tag.is_empty() {
            params.push(Parameter::new(
                pid::DOMAIN_TAG,
                cdr_string(&self.domain_tag, false),
            ));
        }

        params.push(Parameter::new(
            pid::BUILTIN_ENDPOINT_SET,
            self.builtin_endpoint_set.to_be_bytes().to_vec(),
        ));

        params.push(Parameter::new(
            pid::PARTICIPANT_LEASE_DURATION,
            self.lease_duration.to_bytes_be().to_vec(),
        ));

        if let Some(psi) = self.participant_security_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_INFO,
                psi.to_wire_be(),
            ));
        }
        if let Some(sig) = self.sig_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO,
                sig.to_bytes(false).to_vec(),
            ));
        }
        if let Some(kx) = self.kx_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO,
                kx.to_bytes(false).to_vec(),
            ));
        }
        if let Some(sym) = self.sym_cipher_algo_info.as_ref() {
            params.push(Parameter::new(
                pid::PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO,
                sym.to_bytes(false).to_vec(),
            ));
        }
        if self.available_builtin_endpoints_ext != 0 {
            params.push(Parameter::new(
                pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT,
                self.available_builtin_endpoints_ext.to_be_bytes().to_vec(),
            ));
        }
        // FALSE is the default (Table 9.19) and is left out.
        if self.expects_inline_qos {
            params.push(Parameter::new(
                pid::EXPECTS_INLINE_QOS,
                crate::publication_data::boolean_value(true),
            ));
        }
        if let Some(count) = self.manual_liveliness_count {
            params.push(Parameter::new(
                pid::PARTICIPANT_MANUAL_LIVELINESS_COUNT,
                count.to_be_bytes().to_vec(),
            ));
        }
        if let Some(name) = &self.entity_name {
            params.push(Parameter::new(pid::ENTITY_NAME, cdr_string(name, false)));
        }
        if let Some(qos) = self.builtin_endpoint_qos {
            params.push(Parameter::new(
                pid::BUILTIN_ENDPOINT_QOS,
                qos.to_be_bytes().to_vec(),
            ));
        }
        if !self.properties.is_empty() {
            if let Ok(bytes) = self.properties.encode(false) {
                params.push(Parameter::new(pid::PROPERTY_LIST, bytes));
            }
        }

        // Encapsulation-Header: PL_CDR_BE (0x0002) + options=0.
        let mut out = Vec::new();
        out.extend_from_slice(&[0x00, 0x02]);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&params.to_bytes(false));
        out
    }

    /// Decoded from PL_CDR_LE bytes (with encapsulation header).
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` if the bytes are too short; PIDs
    /// without a spec-conformant length are ignored (forward-compat).
    pub fn from_pl_cdr_le(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        // Check the encapsulation header — we accept PL_CDR_LE
        // (00 03) and PL_CDR_BE (00 02). Others → error.
        let little_endian = match &bytes[..2] {
            b if b == ENCAPSULATION_PL_CDR_LE => true,
            [0x00, 0x02] => false,
            other => {
                return Err(WireError::UnsupportedEncapsulation {
                    kind: [other[0], other[1]],
                });
            }
        };
        let pl = ParameterList::from_bytes(&bytes[4..], little_endian)?;

        let guid = pl
            .find(pid::PARTICIPANT_GUID)
            .and_then(|p| {
                if p.value.len() == 16 {
                    let mut g = [0u8; 16];
                    g.copy_from_slice(&p.value);
                    Some(Guid::from_bytes(g))
                } else {
                    None
                }
            })
            .ok_or(WireError::ValueOutOfRange {
                message: "PARTICIPANT_GUID missing or wrong length",
            })?;

        let protocol_version = pl
            .find(pid::PROTOCOL_VERSION)
            .and_then(|p| {
                if p.value.len() >= 2 {
                    let mut bs = [0u8; 2];
                    bs.copy_from_slice(&p.value[..2]);
                    Some(ProtocolVersion::from_bytes(bs))
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let vendor_id = pl
            .find(pid::VENDOR_ID)
            .and_then(|p| {
                if p.value.len() >= 2 {
                    let mut bs = [0u8; 2];
                    bs.copy_from_slice(&p.value[..2]);
                    Some(VendorId::from_bytes(bs))
                } else {
                    None
                }
            })
            .unwrap_or(VendorId::UNKNOWN);

        // Locator PIDs may appear zero, one, or many times (multi-homed peer,
        // DDSI-RTPS 2.5 §9.6.1). Retain ALL valid ones in wire order — no
        // routing/usability filtering here; that belongs in DCPS/transport.
        let decode_all = |id: u16| -> Vec<Locator> {
            pl.find_all(id)
                .filter_map(|p| decode_locator(&p.value, little_endian))
                .collect()
        };
        let default_unicast_locators = decode_all(pid::DEFAULT_UNICAST_LOCATOR);
        let default_multicast_locators = decode_all(pid::DEFAULT_MULTICAST_LOCATOR);
        let metatraffic_unicast_locators = decode_all(pid::METATRAFFIC_UNICAST_LOCATOR);
        let metatraffic_multicast_locators = decode_all(pid::METATRAFFIC_MULTICAST_LOCATOR);

        let domain_id = pl.find(pid::DOMAIN_ID).and_then(|p| {
            if p.value.len() == 4 {
                let mut bs = [0u8; 4];
                bs.copy_from_slice(&p.value);
                Some(if little_endian {
                    u32::from_le_bytes(bs)
                } else {
                    u32::from_be_bytes(bs)
                })
            } else {
                None
            }
        });

        // An unreadable tag is not the default tag: reading it as "" would
        // join this participant to the untagged domain it asked to leave.
        let domain_tag = match pl.find(pid::DOMAIN_TAG) {
            Some(p) => crate::publication_data::decode_bounded_cdr_string(
                &p.value,
                little_endian,
                crate::publication_data::STRING_256_MAX,
            )?,
            None => alloc::string::String::new(),
        };

        let builtin_endpoint_set = pl
            .find(pid::BUILTIN_ENDPOINT_SET)
            .and_then(|p| {
                if p.value.len() == 4 {
                    let mut bs = [0u8; 4];
                    bs.copy_from_slice(&p.value);
                    Some(if little_endian {
                        u32::from_le_bytes(bs)
                    } else {
                        u32::from_be_bytes(bs)
                    })
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let lease_duration = pl
            .find(pid::PARTICIPANT_LEASE_DURATION)
            .and_then(|p| {
                if p.value.len() == 8 {
                    let mut bs = [0u8; 8];
                    bs.copy_from_slice(&p.value);
                    // Duration_t in the list's byte order (§9.4.2.11).
                    Some(Duration::from_bytes(bs, little_endian))
                } else {
                    None
                }
            })
            .unwrap_or(Duration::from_secs(100));

        let user_data = pl
            .find(pid::USER_DATA)
            .and_then(|p| crate::publication_data::decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();

        // PROPERTY_LIST: empty if the peer sends no security
        // announcements (legacy compatibility); decoder errors lead to
        // an empty list instead of a hard rejection, so a malicious peer
        // cannot push us out of the SPDP process with a malformed
        // PropertyList.
        let properties = pl
            .find(pid::PROPERTY_LIST)
            .and_then(|p| WirePropertyList::decode(&p.value, little_endian).ok())
            .unwrap_or_default();

        // Pass the raw token bytes through (parsing is done by the
        // security layer). Identity/Permissions/IdentityStatus are
        // optional; legacy peers do not send them.
        let identity_token = pl.find(pid::IDENTITY_TOKEN).map(|p| p.value.clone());
        let permissions_token = pl.find(pid::PERMISSIONS_TOKEN).map(|p| p.value.clone());
        let identity_status_token = pl.find(pid::IDENTITY_STATUS_TOKEN).map(|p| p.value.clone());
        // PID_PARTICIPANT_SECURITY_INFO (0x1005) — decode error → silent None.
        let participant_security_info = pl.find(pid::PARTICIPANT_SECURITY_INFO).and_then(|p| {
            use crate::participant_security_info::ParticipantSecurityInfo;
            if little_endian {
                ParticipantSecurityInfo::from_le_bytes(&p.value).ok()
            } else {
                ParticipantSecurityInfo::from_be_bytes(&p.value).ok()
            }
        });

        // Algorithm-info PIDs (spec §7.3.11-13, C3.5 remainder). A decode
        // error → silent None (forward-compat: a peer with altered wire
        // formats must not push us out of SPDP).
        let sig_algo_info = pl
            .find(pid::PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO)
            .and_then(|p| {
                crate::security_algo_info::ParticipantSecurityDigitalSignatureAlgorithmInfo::from_bytes(
                    &p.value,
                    little_endian,
                )
                .ok()
            });
        let kx_algo_info = pl
            .find(pid::PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO)
            .and_then(|p| {
                crate::security_algo_info::ParticipantSecurityKeyEstablishmentAlgorithmInfo::from_bytes(
                    &p.value,
                    little_endian,
                )
                .ok()
            });
        let sym_cipher_algo_info = pl
            .find(pid::PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO)
            .and_then(|p| {
                crate::security_algo_info::ParticipantSecuritySymmetricCipherAlgorithmInfo::from_bytes(
                    &p.value,
                    little_endian,
                )
                .ok()
            });

        // Absent: 0. A value that is not 4 octets rejects the sample.
        let available_builtin_endpoints_ext = match pl.find(pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT) {
            Some(p) => {
                let raw: [u8; 4] = p.value.get(..4).and_then(|b| b.try_into().ok()).ok_or(
                    WireError::ValueOutOfRange {
                        message: "PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT shorter than 4 octets",
                    },
                )?;
                if little_endian {
                    u32::from_le_bytes(raw)
                } else {
                    u32::from_be_bytes(raw)
                }
            }
            None => 0,
        };

        Ok(Self {
            guid,
            protocol_version,
            vendor_id,
            default_unicast_locators,
            default_multicast_locators,
            metatraffic_unicast_locators,
            metatraffic_multicast_locators,
            domain_id,
            domain_tag,
            builtin_endpoint_set,
            lease_duration,
            user_data,
            properties,
            identity_token,
            permissions_token,
            identity_status_token,
            sig_algo_info,
            kx_algo_info,
            sym_cipher_algo_info,
            available_builtin_endpoints_ext,
            participant_security_info,
            expects_inline_qos: crate::publication_data::boolean_param(
                &pl,
                pid::EXPECTS_INLINE_QOS,
            )?,
            entity_name: match pl.find(pid::ENTITY_NAME) {
                Some(p) => Some(crate::publication_data::decode_cdr_string(
                    &p.value,
                    little_endian,
                )?),
                None => None,
            },
            manual_liveliness_count: four_octets(
                &pl,
                pid::PARTICIPANT_MANUAL_LIVELINESS_COUNT,
                "PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT is not 4 octets",
                little_endian,
            )?
            .map(|v| v as i32),
            builtin_endpoint_qos: four_octets(
                &pl,
                pid::BUILTIN_ENDPOINT_QOS,
                "PID_BUILTIN_ENDPOINT_QOS is not 4 octets",
                little_endian,
            )?,
        })
    }
}

/// The four-octet value of `id` in `pl`, in the byte order of the list; `None`
/// where the parameter is absent.
///
/// # Errors
/// `ValueOutOfRange` with `message` for a value of another length.
pub(crate) fn four_octets(
    pl: &ParameterList,
    id: u16,
    message: &'static str,
    little_endian: bool,
) -> Result<Option<u32>, WireError> {
    let Some(p) = pl.find(id) else {
        return Ok(None);
    };
    let bytes: [u8; 4] = p
        .value
        .as_slice()
        .try_into()
        .map_err(|_| WireError::ValueOutOfRange { message })?;
    Ok(Some(if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }))
}

/// `true` if a UDPv4 locator is plausibly reachable — i.e. NOT
/// link-local (169.254.0.0/16) or unspecified (0.0.0.0). Loopback (127.0.0.0/8)
/// counts as routable (same-host). Non-UDPv4 kinds (TCP/SHM/UDS/IPv6) are
/// not heuristically downgraded.
/// Heuristic: does this locator look like a routable destination? UDPv4
/// unspecified (0.0.0.0) and link-local (169.254/16) are treated as
/// non-routable; every other kind/address is assumed routable. Used by
/// DCPS/transport for usability ordering of a peer's announced locator list
/// (the RTPS decode path itself performs **no** filtering — it retains all).
#[must_use]
pub fn locator_looks_routable(loc: &Locator) -> bool {
    if loc.kind != LocatorKind::UdpV4 {
        return true;
    }
    let ip = loc.ipv4();
    let unspecified = ip == [0, 0, 0, 0];
    let link_local = ip[0] == 169 && ip[1] == 254;
    !(unspecified || link_local)
}

fn decode_locator(value: &[u8], little_endian: bool) -> Option<Locator> {
    if value.len() != Locator::WIRE_SIZE {
        return None;
    }
    let mut bs = [0u8; 24];
    bs.copy_from_slice(value);
    if little_endian {
        Locator::from_bytes_le(bs).ok()
    } else {
        Locator::from_bytes_be(bs).ok()
    }
}

/// A CDR `string` as a parameter value: length with the terminator, the
/// octets, the terminator, padded to four.
fn cdr_string(s: &str, little_endian: bool) -> Vec<u8> {
    let len = (s.len() as u32).saturating_add(1);
    let mut out = Vec::with_capacity(8 + s.len());
    out.extend_from_slice(&if little_endian {
        len.to_le_bytes()
    } else {
        len.to_be_bytes()
    });
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix};
    use alloc::vec;

    #[test]
    fn locator_looks_routable_classifies_addresses() {
        // The routability heuristic is a pure predicate now (DCPS orders the
        // retained list with it; the decode path itself never filters).
        assert!(locator_looks_routable(&Locator::udp_v4(
            [192, 168, 1, 10],
            7410
        )));
        // Loopback counts as routable (same-host operation).
        assert!(locator_looks_routable(&Locator::udp_v4(
            [127, 0, 0, 1],
            7410
        )));
        // link-local and unspecified are non-routable.
        assert!(!locator_looks_routable(&Locator::udp_v4(
            [169, 254, 1, 5],
            7410
        )));
        assert!(!locator_looks_routable(&Locator::udp_v4(
            [0, 0, 0, 0],
            7410
        )));
    }

    #[test]
    fn decode_retains_all_repeated_unicast_locators_in_order_le_and_be() {
        // A multi-homed peer lists every usable unicast locator. Decode keeps
        // ALL of them in wire order (no reduction), LE and BE alike (#27).
        let a = Locator::udp_v4([169, 254, 1, 5], 7410); // link-local FIRST
        let b = Locator::udp_v4([192, 168, 1, 10], 7410);
        let c = Locator::udp_v4([10, 0, 0, 7], 7410);
        for be in [false, true] {
            let mut src = sample_data();
            src.default_unicast_locators = vec![a, b, c];
            src.metatraffic_unicast_locators = vec![b, c];
            let bytes = if be {
                src.to_pl_cdr_be()
            } else {
                src.to_pl_cdr_le()
            };
            let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
            assert_eq!(
                decoded.default_unicast_locators,
                vec![a, b, c],
                "be={be}: all three retained, wire order preserved"
            );
            assert_eq!(decoded.metatraffic_unicast_locators, vec![b, c], "be={be}");
            // No routing filtering in decode: the link-local FIRST survives.
            assert_eq!(decoded.default_unicast_locators[0], a, "be={be}");
        }
    }

    #[test]
    fn encode_dedups_exact_duplicate_locators() {
        let b = Locator::udp_v4([192, 168, 1, 10], 7410);
        let mut src = sample_data();
        src.default_unicast_locators = vec![b, b, b];
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&src.to_pl_cdr_le()).unwrap();
        assert_eq!(
            decoded.default_unicast_locators,
            vec![b],
            "duplicates removed"
        );
    }

    #[test]
    fn decode_zero_locators_yields_empty_lists() {
        let mut src = sample_data();
        src.default_unicast_locators.clear();
        src.metatraffic_unicast_locators.clear();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&src.to_pl_cdr_le()).unwrap();
        assert!(decoded.default_unicast_locators.is_empty());
        assert!(decoded.metatraffic_unicast_locators.is_empty());
    }

    fn sample_data() -> ParticipantBuiltinTopicData {
        ParticipantBuiltinTopicData {
            guid: Guid::new(
                GuidPrefix::from_bytes([0xA, 0xB, 0xC, 0xD, 1, 2, 3, 4, 5, 6, 7, 8]),
                EntityId::PARTICIPANT,
            ),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locators: vec![Locator::udp_v4([192, 168, 1, 100], 7410)],
            default_multicast_locators: vec![Locator::udp_v4([239, 255, 0, 1], 7400)],
            metatraffic_unicast_locators: Vec::new(),
            metatraffic_multicast_locators: Vec::new(),
            domain_id: None,
            domain_tag: Default::default(),
            builtin_endpoint_set: endpoint_flag::PARTICIPANT_ANNOUNCER
                | endpoint_flag::PARTICIPANT_DETECTOR,
            lease_duration: Duration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            available_builtin_endpoints_ext: 0,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
        }
    }

    #[test]
    fn to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens() {
        use crate::participant_security_info::{ParticipantSecurityInfo, attrs, plugin_attrs};
        let mut d = sample_data();
        d.participant_security_info = Some(ParticipantSecurityInfo {
            participant_security_attributes: attrs::IS_VALID,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID,
            ..Default::default()
        });
        // Credential token set — must NOT appear in c.pdata.
        d.identity_token = Some(alloc::vec![0xAB; 32]);
        d.permissions_token = Some(alloc::vec![0xCD; 16]);
        // Not the 100 s a missing lease falls back to.
        d.lease_duration = Duration::from_secs(30);
        let be = d.to_pl_cdr_be();
        // PL_CDR_BE encapsulation (Spec §9.3.2.5.2: c.pdata is big-endian).
        assert_eq!(
            &be[..4],
            &[0x00, 0x02, 0x00, 0x00],
            "c.pdata must be PL_CDR_BE"
        );
        // Roundtrip (from_pl_cdr_le accepts BE via the encapsulation kind).
        let back = ParticipantBuiltinTopicData::from_pl_cdr_le(&be).unwrap();
        assert_eq!(back.guid, d.guid);
        assert_eq!(back.builtin_endpoint_set, d.builtin_endpoint_set);
        assert_eq!(back.participant_security_info, d.participant_security_info);
        // Duration_t in the list's byte order (§9.4.2.11).
        assert_eq!(back.lease_duration, Duration::from_secs(30));
        // Credential tokens were omitted (they travel via c.id/c.perm).
        assert!(
            back.identity_token.is_none(),
            "identity_token does not belong in c.pdata"
        );
        assert!(
            back.permissions_token.is_none(),
            "permissions_token does not belong in c.pdata"
        );
    }

    /// `PID_DOMAIN_TAG` (0x4014, DDSI-RTPS 2.5 §9.6.2.2.2): a CDR string,
    /// sent only when not empty, read back in both byte orders.
    #[test]
    fn domain_tag_goes_out_when_set_and_reads_back() {
        let mut d = sample_data();
        assert!(d.domain_tag.is_empty());
        let plain = d.to_pl_cdr_le();
        assert!(
            !plain.windows(2).any(|w| w == [0x14, 0x40]),
            "empty tag, no PID"
        );
        assert_eq!(
            ParticipantBuiltinTopicData::from_pl_cdr_le(&plain)
                .unwrap()
                .domain_tag,
            ""
        );

        d.domain_tag = "abc".into();
        let le = d.to_pl_cdr_le();
        let pid_and_value = [0x14, 0x40, 0x08, 0x00, 0x04, 0, 0, 0, b'a', b'b', b'c', 0];
        assert!(
            le.windows(pid_and_value.len()).any(|w| w == pid_and_value),
            "PID 0x4014, length 8, CDR string \"abc\""
        );
        assert_eq!(
            ParticipantBuiltinTopicData::from_pl_cdr_le(&le)
                .unwrap()
                .domain_tag,
            "abc"
        );
        let be = d.to_pl_cdr_be();
        assert_eq!(
            ParticipantBuiltinTopicData::from_pl_cdr_le(&be)
                .unwrap()
                .domain_tag,
            "abc"
        );
    }

    /// `PID_EXPECTS_INLINE_QOS` in SPDP (DDSI-RTPS 2.5 §8.5.3.2): TRUE goes
    /// out in both byte orders and reads back, FALSE is left out, and an
    /// octet other than 0 or 1 is refused.
    #[test]
    fn expects_inline_qos_goes_out_when_set_and_reads_back() {
        let mut d = sample_data();
        let plain = d.to_pl_cdr_le();
        assert!(
            !plain.windows(4).any(|w| w == [0x43, 0x00, 0x04, 0x00]),
            "FALSE, no PID"
        );
        assert!(
            !ParticipantBuiltinTopicData::from_pl_cdr_le(&plain)
                .unwrap()
                .expects_inline_qos
        );

        d.expects_inline_qos = true;
        let mut le = d.to_pl_cdr_le();
        let pid_and_value = [0x43, 0x00, 0x04, 0x00, 0x01, 0, 0, 0];
        let at = le
            .windows(pid_and_value.len())
            .position(|w| w == pid_and_value)
            .expect("PID 0x0043, length 4, TRUE");
        assert!(
            ParticipantBuiltinTopicData::from_pl_cdr_le(&le)
                .unwrap()
                .expects_inline_qos
        );
        let be = d.to_pl_cdr_be();
        assert!(
            be.windows(8)
                .any(|w| w == [0x00, 0x43, 0x00, 0x04, 0x01, 0, 0, 0]),
            "big-endian: PID and length swap, the octet does not"
        );
        assert!(
            ParticipantBuiltinTopicData::from_pl_cdr_le(&be)
                .unwrap()
                .expects_inline_qos
        );
        le[at + 4] = 2;
        assert!(ParticipantBuiltinTopicData::from_pl_cdr_le(&le).is_err());
    }

    /// DDSI-RTPS 2.5 Table 9.18, Table 9.19: `PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT`
    /// (0x0034, `Count_t`) and `PID_BUILTIN_ENDPOINT_QOS` (0x0077,
    /// `BuiltinEndpointQos_t`, §9.3.2.13) go out in both byte orders as four
    /// octets and read back; absent is `None`, and another length is refused.
    /// §10.3: "The receiver shall ignore the value of the
    /// representation_options", in both byte orders.
    #[test]
    fn the_receiver_ignores_the_representation_options_of_a_participant() {
        let d = sample_data();
        for mut bytes in [d.to_pl_cdr_le(), d.to_pl_cdr_be()] {
            assert_eq!(&bytes[2..4], &[0, 0], "the sender writes zero");
            let plain = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
            bytes[2] = 0x12;
            bytes[3] = 0x34;
            assert_eq!(
                ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap(),
                plain
            );
        }
    }

    /// Table 9.18 `PID_ENTITY_NAME`: the participant's name, in both byte
    /// orders.
    #[test]
    fn the_participant_name_goes_out_and_reads_back() {
        let mut d = sample_data();
        assert_eq!(d.entity_name, None);
        d.entity_name = Some("RTPSParticipant".into());
        let le = ParticipantBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le()).unwrap();
        assert_eq!(le.entity_name.as_deref(), Some("RTPSParticipant"));
        let be = ParticipantBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_be()).unwrap();
        assert_eq!(be.entity_name.as_deref(), Some("RTPSParticipant"));
        d.entity_name = None;
        let none = ParticipantBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le()).unwrap();
        assert_eq!(none.entity_name, None);
    }

    #[test]
    fn manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back() {
        let mut d = sample_data();
        let plain = d.to_pl_cdr_le();
        let read = ParticipantBuiltinTopicData::from_pl_cdr_le(&plain).unwrap();
        assert_eq!(
            (read.manual_liveliness_count, read.builtin_endpoint_qos),
            (None, None)
        );

        d.manual_liveliness_count = Some(0x0102_0304);
        d.builtin_endpoint_qos =
            Some(builtin_endpoint_qos::BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER);
        let le = d.to_pl_cdr_le();
        assert!(
            le.windows(8)
                .any(|w| w == [0x34, 0x00, 0x04, 0x00, 0x04, 0x03, 0x02, 0x01])
        );
        assert!(
            le.windows(8)
                .any(|w| w == [0x77, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00])
        );
        let be = d.to_pl_cdr_be();
        assert!(
            be.windows(8)
                .any(|w| w == [0x00, 0x34, 0x00, 0x04, 0x01, 0x02, 0x03, 0x04])
        );
        for bytes in [&le, &be] {
            let read = ParticipantBuiltinTopicData::from_pl_cdr_le(bytes).unwrap();
            assert_eq!(read.manual_liveliness_count, Some(0x0102_0304));
            assert_eq!(read.builtin_endpoint_qos, Some(1));
        }
        // A length of 8 for 0x0077 is no BuiltinEndpointQos_t.
        let mut bad = le.clone();
        let at = bad
            .windows(4)
            .position(|w| w == [0x77, 0x00, 0x04, 0x00])
            .unwrap();
        bad[at + 2] = 0x08;
        assert!(ParticipantBuiltinTopicData::from_pl_cdr_le(&bad).is_err());
    }

    /// DDSI-RTPS 2.5 Table 9.18: `PID_DOMAIN_TAG` is `string<256>`; a longer
    /// tag in an announcement is refused, not cut.
    #[test]
    fn a_domain_tag_longer_than_string_256_is_refused() {
        let mut d = sample_data();
        d.domain_tag = "x".repeat(256);
        let ok = ParticipantBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le()).unwrap();
        assert_eq!(ok.domain_tag.len(), 256);
        d.domain_tag = "x".repeat(257);
        assert!(ParticipantBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le()).is_err());
    }

    /// The must-understand bit is part of the id: a receiver validating
    /// must-understand PIDs accepts it.
    #[test]
    fn domain_tag_is_a_standard_must_understand_pid() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::DOMAIN_TAG,
            alloc::vec![1, 0, 0, 0, 0, 0, 0, 0],
        ));
        assert!(
            pl.validate_must_understand_in_data_pipeline(crate::wire_types::VendorId::ZERODDS)
                .is_ok()
        );
    }

    #[test]
    fn participant_security_info_pid_roundtrip() {
        // FU2 cross-vendor: PID_PARTICIPANT_SECURITY_INFO (0x1005) must
        // appear in the SPDP PL_CDR + roundtrip, otherwise cyclone/
        // FastDDS classify us as non-secure.
        use crate::participant_security_info::{ParticipantSecurityInfo, attrs, plugin_attrs};
        let mut d = sample_data();
        d.participant_security_info = Some(ParticipantSecurityInfo {
            participant_security_attributes: attrs::IS_VALID,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID,
            ..Default::default()
        });
        let bytes = d.to_pl_cdr_le();
        // PID 0x1005 LE = 05 10 must be in the stream.
        let has_pid = bytes.windows(2).any(|w| w == [0x05, 0x10]);
        assert!(
            has_pid,
            "PID_PARTICIPANT_SECURITY_INFO missing from PL_CDR_LE"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.participant_security_info,
            d.participant_security_info
        );
        assert!(decoded.participant_security_info.unwrap().is_valid());
    }

    #[test]
    fn duration_roundtrip_le() {
        let d = Duration {
            seconds: 30,
            fraction: 500_000_000,
        };
        assert_eq!(Duration::from_bytes_le(d.to_bytes_le()), d);
    }

    #[test]
    fn participant_data_roundtrip_full() {
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn participant_data_first_4_bytes_are_pl_cdr_le_encapsulation() {
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        assert_eq!(&bytes[..4], &[0x00, 0x03, 0x00, 0x00]);
    }

    #[test]
    fn participant_data_properties_roundtrip() {
        use crate::property_list::WireProperty;
        let mut d = sample_data();
        d.properties.push(WireProperty::new(
            "org.zerodds.sec.auth.plugin_class",
            "DDS:Auth:PKI-DH:1.2",
        ));
        d.properties.push(WireProperty::new(
            "org.zerodds.sec.offered_protection",
            "ENCRYPT",
        ));
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.properties, d.properties);
        assert_eq!(
            decoded.properties.get("org.zerodds.sec.offered_protection"),
            Some("ENCRYPT")
        );
    }

    #[test]
    fn participant_data_empty_properties_omits_pid() {
        // Empty PropertyList → PID_PROPERTY_LIST should NOT appear in
        // the bytes (backward compatibility: legacy peers that do not
        // know the PID must not be confused).
        let d = sample_data();
        assert!(d.properties.is_empty());
        let bytes = d.to_pl_cdr_le();
        // PID_PROPERTY_LIST = 0x0059 LE = 59 00 ; search in the stream
        // (naive — enough for this test).
        let has_pid = bytes.windows(2).any(|w| w == [0x59, 0x00]);
        assert!(!has_pid, "empty properties must omit the PID");
    }

    #[test]
    fn participant_data_legacy_peer_without_properties_parses_ok() {
        // A peer that sends no PID_PROPERTY_LIST → decoded.properties
        // is empty. This scenario is the default for all legacy
        // ZeroDDS peers + all Cyclone/Fast-DDS without security.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.properties.is_empty());
    }

    #[test]
    fn participant_data_identity_token_pid_roundtrip() {
        // PID_IDENTITY_TOKEN (0x1001) — opaque value (CDR-encoded
        // DataHolder, parsed by the security layer). RTPS passes it
        // through byte-identically.
        let mut d = sample_data();
        // Value 4-byte aligned, because the ParameterList pads the PID
        // value with zero-padding to the 4-byte boundary — the
        // parameter_length in the PID header is the padded length, and
        // the decoder cannot distinguish trailing zeros from real value
        // content. The security-layer codec (DataHolder) ignores
        // trailing zeros via its parser behavior.
        d.identity_token = Some(vec![0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x02, 0x03, 0x04]);
        let bytes = d.to_pl_cdr_le();
        // PID tag 0x1001 LE = 01 10 must appear in the stream.
        let has_pid = bytes.windows(2).any(|w| w == [0x01, 0x10]);
        assert!(
            has_pid,
            "PID_IDENTITY_TOKEN missing from the PL_CDR_LE stream"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.identity_token, d.identity_token);
    }

    #[test]
    fn participant_data_permissions_token_pid_roundtrip() {
        let mut d = sample_data();
        d.permissions_token = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let bytes = d.to_pl_cdr_le();
        let has_pid = bytes.windows(2).any(|w| w == [0x02, 0x10]);
        assert!(has_pid, "PID_PERMISSIONS_TOKEN missing");
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.permissions_token, d.permissions_token);
    }

    #[test]
    fn participant_data_identity_status_token_pid_roundtrip() {
        let mut d = sample_data();
        d.identity_status_token = Some(vec![0x77, 0x88, 0x99, 0xAA]);
        let bytes = d.to_pl_cdr_le();
        let has_pid = bytes.windows(2).any(|w| w == [0x06, 0x10]);
        assert!(has_pid, "PID_IDENTITY_STATUS_TOKEN missing");
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.identity_status_token, d.identity_status_token);
    }

    #[test]
    fn participant_data_no_token_pids_in_legacy_announce() {
        // Default sample (no security) → none of the three token PIDs
        // appears, so legacy peers are not confused.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        for pid_le in [[0x01u8, 0x10], [0x02, 0x10], [0x06, 0x10]] {
            let found = bytes.windows(2).any(|w| w == pid_le);
            assert!(!found, "token PID {pid_le:?} must not appear in legacy");
        }
    }

    #[test]
    fn participant_data_three_tokens_combined_roundtrip() {
        // Realistic security announce: all three tokens at once.
        let mut d = sample_data();
        d.identity_token = Some(vec![0x01; 64]);
        d.permissions_token = Some(vec![0x02; 32]);
        d.identity_status_token = Some(vec![0x03; 16]);
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, d);
    }

    // Algorithm-Info-PIDs (Spec §7.3.11-13, C3.5-Rest)

    #[test]
    fn participant_data_sig_algo_info_roundtrip() {
        let mut d = sample_data();
        d.sig_algo_info =
            Some(crate::security_algo_info::ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default());
        let bytes = d.to_pl_cdr_le();
        // PID 0x1010 LE = [0x10, 0x10] must be in the stream.
        assert!(
            bytes.windows(2).any(|w| w == [0x10, 0x10]),
            "PID 0x1010 missing from the PL_CDR_LE stream"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.sig_algo_info, d.sig_algo_info);
    }

    #[test]
    fn participant_data_kx_algo_info_roundtrip() {
        let mut d = sample_data();
        d.kx_algo_info =
            Some(crate::security_algo_info::ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default());
        let bytes = d.to_pl_cdr_le();
        assert!(
            bytes.windows(2).any(|w| w == [0x11, 0x10]),
            "PID 0x1011 missing"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.kx_algo_info, d.kx_algo_info);
    }

    #[test]
    fn participant_data_sym_cipher_algo_info_roundtrip() {
        let mut d = sample_data();
        d.sym_cipher_algo_info =
            Some(crate::security_algo_info::ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default());
        let bytes = d.to_pl_cdr_le();
        assert!(
            bytes.windows(2).any(|w| w == [0x12, 0x10]),
            "PID 0x1012 missing"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.sym_cipher_algo_info, d.sym_cipher_algo_info);
    }

    /// DDS-Security 1.2 §7.5.1.4 Table 12 (S. 71 to 72): the bits of
    /// `available_builtin_endpoints_ext`, `0x00000001 << n` for n = 0..3 in the
    /// order Request Writer, Request Reader, Reply Writer, Reply Reader.
    #[test]
    fn table_12_bits() {
        use super::builtin_endpoint_ext as ext;
        assert_eq!(ext::TYPE_LOOKUP_REQUEST_SECURE_WRITER, 0x0000_0001);
        assert_eq!(ext::TYPE_LOOKUP_REQUEST_SECURE_READER, 0x0000_0002);
        assert_eq!(ext::TYPE_LOOKUP_REPLY_SECURE_WRITER, 0x0000_0004);
        assert_eq!(ext::TYPE_LOOKUP_REPLY_SECURE_READER, 0x0000_0008);
        assert_eq!(ext::ALL_TYPE_LOOKUP_SECURE, 0x0000_000f);
    }

    /// DDS-Security 1.2 §7.5.1.4 Table 10, Table 12: the secure TypeLookup
    /// endpoints are bits 0..3 of `PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT`
    /// 0x1007; absent is 0 and is not sent.
    #[test]
    fn available_builtin_endpoints_ext_roundtrip() {
        use super::builtin_endpoint_ext as ext;
        let mut d = sample_data();
        assert_eq!(d.available_builtin_endpoints_ext, 0);
        let plain = d.to_pl_cdr_le();
        assert!(
            !plain.windows(2).any(|w| w == [0x07, 0x10]),
            "not sent at 0"
        );
        d.available_builtin_endpoints_ext = ext::ALL_TYPE_LOOKUP_SECURE;
        let bytes = d.to_pl_cdr_le();
        // PID 0x1007 LE = 07 10, length 4, value 0x0000000f (LE).
        assert!(
            bytes
                .windows(8)
                .any(|w| w == [0x07, 0x10, 0x04, 0x00, 0x0f, 0, 0, 0]),
            "PID 0x1007 with 0x0f missing"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.available_builtin_endpoints_ext, 0x0f);
        assert_eq!(ext::TYPE_LOOKUP_REPLY_SECURE_READER, 0x08);
        let be = d.to_pl_cdr_be();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&be).unwrap();
        assert_eq!(decoded.available_builtin_endpoints_ext, 0x0f);
    }

    #[test]
    fn participant_data_no_algo_info_in_legacy_announce() {
        // Default sample → none of the three algo-info PIDs appears.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        for pid_le in [[0x10u8, 0x10], [0x11, 0x10], [0x12, 0x10]] {
            assert!(
                !bytes.windows(2).any(|w| w == pid_le),
                "algo PID {pid_le:?} must not appear in legacy"
            );
        }
    }

    #[test]
    fn participant_data_all_three_algo_infos_combined() {
        let mut d = sample_data();
        d.sig_algo_info =
            Some(crate::security_algo_info::ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default());
        d.kx_algo_info =
            Some(crate::security_algo_info::ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default());
        d.sym_cipher_algo_info =
            Some(crate::security_algo_info::ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default());
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn participant_data_without_optional_locators() {
        let mut d = sample_data();
        d.default_unicast_locators.clear();
        d.default_multicast_locators.clear();
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.default_unicast_locators.is_empty());
        assert!(decoded.default_multicast_locators.is_empty());
    }

    #[test]
    fn participant_data_decode_rejects_unknown_encapsulation() {
        let mut bytes = vec![0x99, 0x99, 0, 0]; // unknown encap
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // sentinel
        let res = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnsupportedEncapsulation { kind: [0x99, 0x99] })
        ));
    }

    #[test]
    fn participant_data_decode_requires_guid_pid() {
        // Encapsulation + sentinel only.
        let bytes = vec![0x00, 0x03, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let res = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn endpoint_flags_have_distinct_bits() {
        // Sanity: no two flags occupy the same bit.
        let flags = [
            endpoint_flag::PARTICIPANT_ANNOUNCER,
            endpoint_flag::PARTICIPANT_DETECTOR,
            endpoint_flag::PUBLICATIONS_ANNOUNCER,
            endpoint_flag::PUBLICATIONS_DETECTOR,
            endpoint_flag::SUBSCRIPTIONS_ANNOUNCER,
            endpoint_flag::SUBSCRIPTIONS_DETECTOR,
            endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER,
            endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER,
            endpoint_flag::PUBLICATIONS_SECURE_WRITER,
            endpoint_flag::PUBLICATIONS_SECURE_READER,
            endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER,
            endpoint_flag::SUBSCRIPTIONS_SECURE_READER,
            endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER,
            endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER,
            endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER,
            endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER,
            endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            endpoint_flag::PARTICIPANT_SECURE_WRITER,
            endpoint_flag::PARTICIPANT_SECURE_READER,
            endpoint_flag::TOPICS_ANNOUNCER,
            endpoint_flag::TOPICS_DETECTOR,
        ];
        for (i, &a) in flags.iter().enumerate() {
            for &b in &flags[i + 1..] {
                assert_eq!(a & b, 0, "flag bits must be distinct");
            }
        }
    }

    #[test]
    fn endpoint_flag_bit_positions_match_spec() {
        // Bit positions must match the spec table exactly
        // (DDSI-RTPS 2.5 §9.3.2.12 + DDS-Security 1.2 §7.4.7.1).
        // Cyclone DDS and Fast-DDS rely on these exact bits — an offset
        // of 1 position breaks endpoint discovery.
        assert_eq!(endpoint_flag::PARTICIPANT_ANNOUNCER, 0x0000_0001);
        assert_eq!(endpoint_flag::PARTICIPANT_DETECTOR, 0x0000_0002);
        assert_eq!(endpoint_flag::PUBLICATIONS_ANNOUNCER, 0x0000_0004);
        assert_eq!(endpoint_flag::PUBLICATIONS_DETECTOR, 0x0000_0008);
        assert_eq!(endpoint_flag::SUBSCRIPTIONS_ANNOUNCER, 0x0000_0010);
        assert_eq!(endpoint_flag::SUBSCRIPTIONS_DETECTOR, 0x0000_0020);
        assert_eq!(endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER, 0x0000_0400);
        assert_eq!(endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER, 0x0000_0800);
        assert_eq!(endpoint_flag::PUBLICATIONS_SECURE_WRITER, 0x0001_0000);
        assert_eq!(endpoint_flag::PUBLICATIONS_SECURE_READER, 0x0002_0000);
        assert_eq!(endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER, 0x0004_0000);
        assert_eq!(endpoint_flag::SUBSCRIPTIONS_SECURE_READER, 0x0008_0000);
        assert_eq!(
            endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER,
            0x0010_0000
        );
        assert_eq!(
            endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER,
            0x0020_0000
        );
        assert_eq!(
            endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER,
            0x0040_0000
        );
        assert_eq!(
            endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER,
            0x0080_0000
        );
        assert_eq!(
            endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            0x0100_0000
        );
        assert_eq!(
            endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            0x0200_0000
        );
        assert_eq!(endpoint_flag::PARTICIPANT_SECURE_WRITER, 0x0400_0000);
        assert_eq!(endpoint_flag::PARTICIPANT_SECURE_READER, 0x0800_0000);
        assert_eq!(endpoint_flag::TOPICS_ANNOUNCER, 0x1000_0000);
        assert_eq!(endpoint_flag::TOPICS_DETECTOR, 0x2000_0000);
    }

    #[test]
    fn endpoint_flag_all_secure_covers_bits_16_to_27() {
        // ALL_SECURE must set exactly the 12 bits 16..=27, no bit more
        // and no bit less (otherwise the default build leaks security
        // bits into insecure peers).
        let mask = endpoint_flag::ALL_SECURE;
        for bit in 16u32..=27 {
            assert!(
                mask & (1u32 << bit) != 0,
                "bit {bit} missing from ALL_SECURE"
            );
        }
        // No bits outside 16..=27.
        let outside_mask: u32 = !((1u32 << 28) - (1u32 << 16));
        assert_eq!(
            mask & outside_mask,
            0,
            "ALL_SECURE may only set bits 16..27"
        );
    }

    #[test]
    fn endpoint_flag_all_standard_excludes_secure_bits() {
        // The default standard bundle must contain NO security bits.
        // Otherwise we leak secure-endpoint promises into peers without
        // the `security` feature being active.
        assert_eq!(endpoint_flag::ALL_STANDARD & endpoint_flag::ALL_SECURE, 0);
    }

    #[test]
    fn endpoint_flag_roundtrip_through_pl_cdr() {
        // The encoder must carry all 16 bits unaltered over PL_CDR_LE —
        // otherwise peers lose secure/WLP/topics bits.
        let combined = endpoint_flag::ALL_STANDARD | endpoint_flag::ALL_SECURE;
        let mut d = sample_data();
        d.builtin_endpoint_set = combined;
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.builtin_endpoint_set, combined);
    }
}
