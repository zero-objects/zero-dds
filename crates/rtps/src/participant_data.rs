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
/// table 9.4 + 9.5; DDS-Security 1.2 §7.4.7.1, table 8 for bits
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

    /// `TYPE_LOOKUP_SERVICE_REQUEST_DATA_WRITER/READER` — the TypeLookup
    /// request endpoint pair (writer + reader). XTypes 1.3 §7.6.3.3.4
    /// assigns bit 12 to the request pair.
    pub const TYPE_LOOKUP_REQUEST: u32 = 1 << 12;
    /// `TYPE_LOOKUP_SERVICE_REPLY_DATA_WRITER/READER` — the TypeLookup
    /// reply endpoint pair (writer + reader). XTypes 1.3 §7.6.3.3.4
    /// assigns bit 13 to the reply pair.
    pub const TYPE_LOOKUP_REPLY: u32 = 1 << 13;

    // ---------------------------------------------------------------
    // DDS-Security 1.2 §7.4.7.1, tab. 8 — secure-discovery endpoints
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

    /// Mask of all standard bits (0..5, 10..13) without security and
    /// without SEDP topics. Represents the ZeroDDS standard discovery
    /// capability for a participant without the `security` feature.
    /// Includes the TypeLookup service (bits 12+13, XTypes 1.3
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
    /// GUID des Participants.
    pub guid: Guid,
    /// Protokoll-Version (typisch 2.5).
    pub protocol_version: ProtocolVersion,
    /// Vendor-Identifier.
    pub vendor_id: VendorId,
    /// Default-Unicast-Locator — wohin Peers User-Daten schicken.
    pub default_unicast_locator: Option<Locator>,
    /// Default multicast locator — user-data multicast.
    pub default_multicast_locator: Option<Locator>,
    /// Metatraffic unicast locator — where peers send SEDP unicast.
    /// Indispensable for SEDP interop (e.g. Cyclone): Cyclone routes
    /// publications/subscriptions to exactly this locator after a match.
    pub metatraffic_unicast_locator: Option<Locator>,
    /// Metatraffic multicast locator — the SPDP/SEDP multicast group.
    pub metatraffic_multicast_locator: Option<Locator>,
    /// DDS domain ID. Cyclone filters beacons from other domains; if the
    /// PID is missing, domain 0 is usually assumed.
    pub domain_id: Option<u32>,
    /// Bitmask of available builtin endpoints
    /// (see [`endpoint_flag`]).
    pub builtin_endpoint_set: u32,
    /// How long the participant is considered "alive" without a renewed
    /// beacon.
    pub lease_duration: Duration,
    /// UserData QoS at the participant (spec §2.2.3.1) — opaque
    /// sequence<octet>, propagated over SPDP.
    pub user_data: Vec<u8>,
    /// Property list (`PID_PROPERTY_LIST`, 0x0059). Carrier for
    /// security-plugin classes, permissions tokens and ZeroDDS
    /// heterogeneous-security caps (WP 4H-b). Empty for peers without
    /// security announcements (legacy compatibility).
    pub properties: WirePropertyList,
    /// Raw CDR-encoded `IdentityToken` blob (DDS-Security 1.2 §7.4.1.4
    /// Tab.16, `PID_IDENTITY_TOKEN`=0x1001). Parsed by the DDS-Security
    /// layer (`zerodds_security::token::DataHolder`) — RTPS only passes
    /// the bytes through, so this crate stays transport-free.
    /// `None` for legacy peers without a security announce.
    pub identity_token: Option<Vec<u8>>,
    /// Raw CDR-encoded `PermissionsToken` blob (DDS-Security 1.2
    /// §7.4.1.5 Tab.17, `PID_PERMISSIONS_TOKEN`=0x1002).
    pub permissions_token: Option<Vec<u8>>,
    /// Raw CDR-encoded `IdentityStatusToken` blob (DDS-Security 1.2
    /// §7.4.1.6, `PID_IDENTITY_STATUS_TOKEN`=0x1006). Optional, carries
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
    /// `ParticipantSecurityInfo` (DDS-Security 1.2 §7.4.1.6,
    /// `PID_PARTICIPANT_SECURITY_INFO`=0x1005). Two attribute bitmasks at
    /// participant level. **Mandatory for cross-vendor interop**: foreign
    /// vendors (cyclone/FastDDS) classify a participant WITHOUT this PID as
    /// NON-SECURE and reject its endpoints. `None` = legacy/plain.
    pub participant_security_info:
        Option<crate::participant_security_info::ParticipantSecurityInfo>,
}

impl ParticipantBuiltinTopicData {
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

        // DEFAULT_UNICAST_LOCATOR (optional): 24 byte
        if let Some(loc) = self.default_unicast_locator {
            params.push(Parameter::new(
                pid::DEFAULT_UNICAST_LOCATOR,
                loc.to_bytes_le().to_vec(),
            ));
        }

        // DEFAULT_MULTICAST_LOCATOR (optional): 24 byte
        if let Some(loc) = self.default_multicast_locator {
            params.push(Parameter::new(
                pid::DEFAULT_MULTICAST_LOCATOR,
                loc.to_bytes_le().to_vec(),
            ));
        }

        // METATRAFFIC_UNICAST_LOCATOR (optional): 24 byte
        if let Some(loc) = self.metatraffic_unicast_locator {
            params.push(Parameter::new(
                pid::METATRAFFIC_UNICAST_LOCATOR,
                loc.to_bytes_le().to_vec(),
            ));
        }

        // METATRAFFIC_MULTICAST_LOCATOR (optional): 24 byte
        if let Some(loc) = self.metatraffic_multicast_locator {
            params.push(Parameter::new(
                pid::METATRAFFIC_MULTICAST_LOCATOR,
                loc.to_bytes_le().to_vec(),
            ));
        }

        // DOMAIN_ID (optional): 4 byte u32
        if let Some(dom) = self.domain_id {
            params.push(Parameter::new(pid::DOMAIN_ID, dom.to_le_bytes().to_vec()));
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
        // (DDS-Security 1.2 §7.4.1.4-6). The caller has already
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
                psi.to_le_bytes().to_vec(),
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

        for (id, loc) in [
            (pid::DEFAULT_UNICAST_LOCATOR, self.default_unicast_locator),
            (
                pid::DEFAULT_MULTICAST_LOCATOR,
                self.default_multicast_locator,
            ),
            (
                pid::METATRAFFIC_UNICAST_LOCATOR,
                self.metatraffic_unicast_locator,
            ),
            (
                pid::METATRAFFIC_MULTICAST_LOCATOR,
                self.metatraffic_multicast_locator,
            ),
        ] {
            if let Some(loc) = loc {
                params.push(Parameter::new(id, loc.to_bytes_be().to_vec()));
            }
        }

        if let Some(dom) = self.domain_id {
            params.push(Parameter::new(pid::DOMAIN_ID, dom.to_be_bytes().to_vec()));
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
                psi.to_be_bytes().to_vec(),
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

        // Unicast locators may be announced multiple times (multi-homed peer):
        // decode all and prefer the routable one instead of blindly the first (M-1).
        let default_unicast_locator = pick_routable_locator(
            pl.find_all(pid::DEFAULT_UNICAST_LOCATOR)
                .filter_map(|p| decode_locator(&p.value, little_endian)),
        );

        let default_multicast_locator = pl
            .find(pid::DEFAULT_MULTICAST_LOCATOR)
            .and_then(|p| decode_locator(&p.value, little_endian));

        let metatraffic_unicast_locator = pick_routable_locator(
            pl.find_all(pid::METATRAFFIC_UNICAST_LOCATOR)
                .filter_map(|p| decode_locator(&p.value, little_endian)),
        );

        let metatraffic_multicast_locator = pl
            .find(pid::METATRAFFIC_MULTICAST_LOCATOR)
            .and_then(|p| decode_locator(&p.value, little_endian));

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
                    Some(Duration::from_bytes_le(bs))
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

        Ok(Self {
            guid,
            protocol_version,
            vendor_id,
            default_unicast_locator,
            default_multicast_locator,
            metatraffic_unicast_locator,
            metatraffic_multicast_locator,
            domain_id,
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
            participant_security_info,
        })
    }
}

/// `true` if a UDPv4 locator is plausibly reachable — i.e. NOT
/// link-local (169.254.0.0/16) or unspecified (0.0.0.0). Loopback (127.0.0.0/8)
/// counts as routable (same-host). Non-UDPv4 kinds (TCP/SHM/UDS/IPv6) are
/// not heuristically downgraded.
fn locator_looks_routable(loc: &Locator) -> bool {
    if loc.kind != LocatorKind::UdpV4 {
        return true;
    }
    let ip = loc.ipv4();
    let unspecified = ip == [0, 0, 0, 0];
    let link_local = ip[0] == 169 && ip[1] == 254;
    !(unspecified || link_local)
}

/// Picks from several announced locators (multi-homed peer, DDSI-RTPS
/// §8.5.3.2 / §9.6.1.1: a `*_UNICAST_LOCATOR` PID may appear multiple times)
/// the most likely reachable one: a plausibly routable one ([`locator_looks_routable`])
/// is preferred, otherwise the first announced. Fixes the misroute where a
/// non-routable FIRST locator (link-local listed first) sent the reverse SPDP/
/// SEDP/VolatileSecure reply to an unreachable target.
fn pick_routable_locator(candidates: impl Iterator<Item = Locator>) -> Option<Locator> {
    let mut first = None;
    let mut best = None;
    for loc in candidates {
        if first.is_none() {
            first = Some(loc);
        }
        if best.is_none() && locator_looks_routable(&loc) {
            best = Some(loc);
        }
    }
    best.or(first)
}

fn decode_locator(value: &[u8], little_endian: bool) -> Option<Locator> {
    if value.len() != Locator::WIRE_SIZE {
        return None;
    }
    if !little_endian {
        // Limitation: BE locator not implemented.
        return None;
    }
    let mut bs = [0u8; 24];
    bs.copy_from_slice(value);
    Locator::from_bytes_le(bs).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix};
    use alloc::vec;

    #[test]
    fn pick_routable_prefers_non_link_local() {
        // Regression M-1: a multi-homed peer may list the link-local
        // locator FIRST. pick_routable_locator must still choose the
        // routable one, otherwise the reverse-discovery reply goes to
        // 169.254.x.x into the void.
        let link_local = Locator::udp_v4([169, 254, 1, 5], 7410);
        let routable = Locator::udp_v4([192, 168, 1, 10], 7410);
        assert_eq!(
            pick_routable_locator([link_local, routable].into_iter()),
            Some(routable)
        );
        // Only link-local → fall back to the first (better than nothing).
        assert_eq!(
            pick_routable_locator([link_local].into_iter()),
            Some(link_local)
        );
        // unspecified is downgraded too.
        let unspec = Locator::udp_v4([0, 0, 0, 0], 7410);
        assert_eq!(
            pick_routable_locator([unspec, routable].into_iter()),
            Some(routable)
        );
        // Loopback counts as routable (same-host operation).
        let loopback = Locator::udp_v4([127, 0, 0, 1], 7410);
        assert_eq!(
            pick_routable_locator([loopback].into_iter()),
            Some(loopback)
        );
        assert_eq!(pick_routable_locator(core::iter::empty()), None);
    }

    fn sample_data() -> ParticipantBuiltinTopicData {
        ParticipantBuiltinTopicData {
            guid: Guid::new(
                GuidPrefix::from_bytes([0xA, 0xB, 0xC, 0xD, 1, 2, 3, 4, 5, 6, 7, 8]),
                EntityId::PARTICIPANT,
            ),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: Some(Locator::udp_v4([192, 168, 1, 100], 7410)),
            default_multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
            metatraffic_unicast_locator: None,
            metatraffic_multicast_locator: None,
            domain_id: None,
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
            participant_security_info: None,
        }
    }

    #[test]
    fn to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens() {
        use crate::participant_security_info::{ParticipantSecurityInfo, attrs, plugin_attrs};
        let mut d = sample_data();
        d.participant_security_info = Some(ParticipantSecurityInfo {
            participant_security_attributes: attrs::IS_VALID,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID,
        });
        // Credential token set — must NOT appear in c.pdata.
        d.identity_token = Some(alloc::vec![0xAB; 32]);
        d.permissions_token = Some(alloc::vec![0xCD; 16]);
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
        // Credential-Token wurden weggelassen (reisen via c.id/c.perm).
        assert!(
            back.identity_token.is_none(),
            "identity_token does not belong in c.pdata"
        );
        assert!(
            back.permissions_token.is_none(),
            "permissions_token does not belong in c.pdata"
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
            "dds.sec.auth.plugin_class",
            "DDS:Auth:PKI-DH:1.2",
        ));
        d.properties.push(WireProperty::new(
            "zerodds.sec.offered_protection",
            "ENCRYPT",
        ));
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.properties, d.properties);
        assert_eq!(
            decoded.properties.get("zerodds.sec.offered_protection"),
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
        d.default_unicast_locator = None;
        d.default_multicast_locator = None;
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.default_unicast_locator.is_none());
        assert!(decoded.default_multicast_locator.is_none());
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
        // Encapsulation + nur Sentinel.
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
