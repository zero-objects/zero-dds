// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ParticipantBuiltinTopicData (DDSI-RTPS 2.5 §8.5.4.2).
//!
//! Inhalt der SPDP-Beacon-DATA-Submessage. Wird als PL_CDR_LE-encoded
//! ParameterList in der `serialized_payload` der DATA-Submessage
//! transportiert (mit 4-byte Encapsulation-Header).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::property_list::WirePropertyList;
use crate::wire_types::{Guid, Locator, ProtocolVersion, VendorId};

/// PL_CDR_LE Encapsulation-Kind (Spec §10.2).
pub const ENCAPSULATION_PL_CDR_LE: [u8; 2] = [0x00, 0x03];

/// `BuiltinEndpointSet`-Bitmask-Flags (DDSI-RTPS 2.5 §9.3.2.12,
/// Tabelle 9.4 + 9.5; DDS-Security 1.2 §7.4.7.1, Tabelle 8 fuer Bits
/// 16..27). Wird ueber den `PID_BUILTIN_ENDPOINT_SET` (0x0058) PID in
/// SPDP-`ParticipantBuiltinTopicData` als 32-Bit-Bitmaske ausgetauscht.
///
/// Bits 6..9 sind Spec-reserviert (historische Particpant-Proxy-
/// Features in DDSI 2.1 / Topics aus 2.5 sind hier nicht vergeben).
/// Bits 16..27 sind die Secure-Discovery-Endpoints aus DDS-Security.
/// Bits 28..29 sind die XTypes-Topics-Discovery-Endpoints. Bit 30..31
/// sind Spec-reserviert.
///
/// Cyclone DDS und Fast-DDS legen ihre SEDP-Proxies anhand dieser
/// Bits an — wenn ein Bit fehlt, baut der Peer den korrespondierenden
/// Reader/Writer nicht auf und das Endpoint-Discovery scheitert.
/// Daher MUESSEN wir alle Endpoints, die wir lokal anbieten, im
/// `builtin_endpoint_set` annoncieren.
pub mod endpoint_flag {
    // ---------------------------------------------------------------
    // Standard-Discovery-Endpoints (DDSI-RTPS 2.5 §9.3.2.12, Tab. 9.4)
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

    /// `PARTICIPANT_MESSAGE_DATA_WRITER` — sendet WLP-Heartbeats
    /// (`ParticipantMessageData`) im Topic
    /// `DCPSParticipantMessage` (Bit 10, RTPS 2.5 §8.4.13).
    pub const PARTICIPANT_MESSAGE_DATA_WRITER: u32 = 1 << 10;
    /// `PARTICIPANT_MESSAGE_DATA_READER` — empfaengt WLP-Heartbeats
    /// (Bit 11, RTPS 2.5 §8.4.13).
    pub const PARTICIPANT_MESSAGE_DATA_READER: u32 = 1 << 11;

    // ---------------------------------------------------------------
    // TypeLookup-Service (XTypes 1.3 §7.6.3.3.4)
    // ---------------------------------------------------------------

    /// `TYPE_LOOKUP_SERVICE_REQUEST_DATA_WRITER/READER` — TypeLookup-
    /// Request-Endpoint-Pair (Writer + Reader). XTypes 1.3 §7.6.3.3.4
    /// belegt Bit 12 für das Request-Pair.
    pub const TYPE_LOOKUP_REQUEST: u32 = 1 << 12;
    /// `TYPE_LOOKUP_SERVICE_REPLY_DATA_WRITER/READER` — TypeLookup-
    /// Reply-Endpoint-Pair (Writer + Reader). XTypes 1.3 §7.6.3.3.4
    /// belegt Bit 13 für das Reply-Pair.
    pub const TYPE_LOOKUP_REPLY: u32 = 1 << 13;

    // ---------------------------------------------------------------
    // DDS-Security 1.2 §7.4.7.1, Tab. 8 — Secure-Discovery-Endpoints
    // (Bits 16..27). Doc-Comments referenzieren die Spec-Konstanten-
    // Namen (`DISC_BUILTIN_ENDPOINT_*`) fuer Cross-Crate-Audits mit
    // dem `zerodds-security-rtps`-Crate.
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
    // Convenience-Bundles
    // ---------------------------------------------------------------

    /// Maske aller 12 Secure-Discovery-Bits (16..27). Wird vom DCPS-
    /// Runtime zugemixt, wenn das `security`-Feature aktiv ist.
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

    /// Maske aller Standard-Bits (0..5, 10..13) ohne Security
    /// und ohne SEDP-Topics. Repraesentiert die ZeroDDS-Standard-
    /// Discovery-Capability fuer einen Participant ohne
    /// `security`-Feature. Inkludiert TypeLookup-Service (Bits 12+13,
    /// XTypes 1.3 §7.6.3.3.4).
    ///
    /// SEDP-Topics-Endpoints (Bits 28/29) sind per RTPS 2.5 §8.5.4.4
    /// optional. Da ZeroDDS DCPSTopic-Samples synthetisch aus
    /// Publications/Subscriptions ableitet, annoncen wir die
    /// Topics-Capability nicht — Bits 28/29 wuerden Peers eine nicht
    /// existente Endpoint-Paarung versprechen. Vendors, die die
    /// nativen Topic-Endpoints selbst implementieren, koennen
    /// [`TOPICS_ANNOUNCER`]/[`TOPICS_DETECTOR`] zur Maske hinzumixen.
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
/// re-exportiert den Typ hier für Abwärtskompatibilität. Alle Call-
/// sites nutzen den qos-Typ.
pub use zerodds_qos::Duration;

/// SPDP-Discovered-Participant-Daten. Subset.
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
    /// Default-Multicast-Locator — User-Daten-Multicast.
    pub default_multicast_locator: Option<Locator>,
    /// Metatraffic-Unicast-Locator — wohin Peers SEDP-Unicast schicken.
    /// Fuer SEDP-Interop (z.B. Cyclone) unverzichtbar: Cyclone routet
    /// Publications/Subscriptions an genau diesen Locator nach match.
    pub metatraffic_unicast_locator: Option<Locator>,
    /// Metatraffic-Multicast-Locator — SPDP/SEDP-Multicast-Gruppe.
    pub metatraffic_multicast_locator: Option<Locator>,
    /// DDS-Domain-ID. Cyclone filtert Beacons aus anderen Domains,
    /// wenn die PID fehlt, wird i.d.R. Domain 0 angenommen.
    pub domain_id: Option<u32>,
    /// Bitmaske der verfuegbaren Builtin-Endpoints
    /// (siehe [`endpoint_flag`]).
    pub builtin_endpoint_set: u32,
    /// Wie lange der Participant ohne erneuten Beacon "lebendig" gilt.
    pub lease_duration: Duration,
    /// UserData-QoS am Participant (Spec §2.2.3.1) — opaque
    /// sequence<octet>, ueber SPDP propagiert.
    pub user_data: Vec<u8>,
    /// Property-Liste (`PID_PROPERTY_LIST`, 0x0059). Traeger fuer
    /// Security-Plugin-Klassen, Permissions-Tokens und ZeroDDS-
    /// Heterogeneous-Security-Caps (WP 4H-b). Leer bei Peers ohne
    /// Security-Announcements (Legacy-Kompatibilitaet).
    pub properties: WirePropertyList,
    /// Roher CDR-encoded `IdentityToken`-Blob (DDS-Security 1.2 §7.4.1.4
    /// Tab.16, `PID_IDENTITY_TOKEN`=0x1001). Wird vom DDS-Security-Layer
    /// (`zerodds_security::token::DataHolder`) geparst — RTPS reicht nur
    /// die Bytes durch, damit diese Crate transport-frei bleibt.
    /// `None` bei Legacy-Peers ohne Security-Annonce.
    pub identity_token: Option<Vec<u8>>,
    /// Roher CDR-encoded `PermissionsToken`-Blob (DDS-Security 1.2
    /// §7.4.1.5 Tab.17, `PID_PERMISSIONS_TOKEN`=0x1002).
    pub permissions_token: Option<Vec<u8>>,
    /// Roher CDR-encoded `IdentityStatusToken`-Blob (DDS-Security 1.2
    /// §7.4.1.6, `PID_IDENTITY_STATUS_TOKEN`=0x1006). Optional, traegt
    /// OCSP-Live-Status.
    pub identity_status_token: Option<Vec<u8>>,
    /// `ParticipantSecurityDigitalSignatureAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.11, `PID=0x1010`). Optional — `None` = Spec-Default
    /// (RSASSA-PSS + ECDSA-P256).
    pub sig_algo_info:
        Option<crate::security_algo_info::ParticipantSecurityDigitalSignatureAlgorithmInfo>,
    /// `ParticipantSecurityKeyEstablishmentAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.12, `PID=0x1011`). Optional — `None` = Spec-Default
    /// (DHE-MODP-2048 + ECDHE-CEUM-P256).
    pub kx_algo_info:
        Option<crate::security_algo_info::ParticipantSecurityKeyEstablishmentAlgorithmInfo>,
    /// `ParticipantSecuritySymmetricCipherAlgorithmInfo` (DDS-Security
    /// 1.2 §7.3.13, `PID=0x1012`). Optional — `None` = Spec-Default
    /// (AES128|AES256 supported, AES128 required).
    pub sym_cipher_algo_info:
        Option<crate::security_algo_info::ParticipantSecuritySymmetricCipherAlgorithmInfo>,
}

impl ParticipantBuiltinTopicData {
    /// Encoded zu PL_CDR_LE-Bytes (mit 4-byte Encapsulation-Header).
    /// Output ist direkt als `serialized_payload` einer DATA-
    /// Submessage verwendbar.
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

        // USER_DATA — opaque sequence<octet>, nur wenn gesetzt.
        if !self.user_data.is_empty() {
            if let Ok(v) = crate::publication_data::encode_octet_seq_le(&self.user_data) {
                params.push(Parameter::new(pid::USER_DATA, v));
            }
        }

        // IDENTITY_TOKEN / PERMISSIONS_TOKEN / IDENTITY_STATUS_TOKEN
        // (DDS-Security 1.2 §7.4.1.4-6). Caller hat den DataHolder
        // bereits CDR-encoded — wir reichen die Bytes durch.
        if let Some(blob) = self.identity_token.as_ref() {
            params.push(Parameter::new(pid::IDENTITY_TOKEN, blob.clone()));
        }
        if let Some(blob) = self.permissions_token.as_ref() {
            params.push(Parameter::new(pid::PERMISSIONS_TOKEN, blob.clone()));
        }
        if let Some(blob) = self.identity_status_token.as_ref() {
            params.push(Parameter::new(pid::IDENTITY_STATUS_TOKEN, blob.clone()));
        }

        // Algorithm-Info-PIDs (Spec §7.3.11-13, C3.5-Rest). Default
        // (None) wird NICHT geschickt — Empfaenger nutzt Spec-Default.
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

        // PROPERTY_LIST (optional — nur wenn nicht leer). Der Encoder
        // der PropertyList darf nicht fehlschlagen, wenn die Bytes
        // konform zu MAX_PROPERTIES sind; bei Ueberschreitung
        // schweigend auslassen, damit das SPDP-Beacon-Encoding nie
        // fehlschlaegt. Caller muss vor Befuellen Caps applizieren.
        if !self.properties.is_empty() {
            if let Ok(bytes) = self.properties.encode(true) {
                params.push(Parameter::new(pid::PROPERTY_LIST, bytes));
            }
        }

        // Encapsulation-Header: 4 byte (PL_CDR_LE + options=0).
        let mut out = Vec::new();
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        out
    }

    /// Decoded aus PL_CDR_LE-Bytes (mit Encapsulation-Header).
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` wenn Bytes zu kurz; PIDs ohne
    /// Spec-konforme Laenge werden ignoriert (forward-compat).
    pub fn from_pl_cdr_le(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        // Encapsulation-Header pruefen — wir akzeptieren PL_CDR_LE
        // (00 03) und PL_CDR_BE (00 02). Andere → Error.
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

        let default_unicast_locator = pl
            .find(pid::DEFAULT_UNICAST_LOCATOR)
            .and_then(|p| decode_locator(&p.value, little_endian));

        let default_multicast_locator = pl
            .find(pid::DEFAULT_MULTICAST_LOCATOR)
            .and_then(|p| decode_locator(&p.value, little_endian));

        let metatraffic_unicast_locator = pl
            .find(pid::METATRAFFIC_UNICAST_LOCATOR)
            .and_then(|p| decode_locator(&p.value, little_endian));

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

        // PROPERTY_LIST: leer wenn Peer keine Security-Announcements
        // schickt (Legacy-Kompatibilitaet); Decoder-Fehler fuehren zu
        // leerer Liste statt harter Ablehnung, damit ein boeser Peer
        // uns nicht per malformed PropertyList aus dem SPDP-Prozess
        // drueckt.
        let properties = pl
            .find(pid::PROPERTY_LIST)
            .and_then(|p| WirePropertyList::decode(&p.value, little_endian).ok())
            .unwrap_or_default();

        // Roh-Bytes der Tokens durchreichen (Parsing macht der Security-
        // Layer). Identity/Permissions/IdentityStatus sind optional;
        // Legacy-Peers schicken sie nicht.
        let identity_token = pl.find(pid::IDENTITY_TOKEN).map(|p| p.value.clone());
        let permissions_token = pl.find(pid::PERMISSIONS_TOKEN).map(|p| p.value.clone());
        let identity_status_token = pl.find(pid::IDENTITY_STATUS_TOKEN).map(|p| p.value.clone());

        // Algorithm-Info-PIDs (Spec §7.3.11-13, C3.5-Rest). Decode-
        // Fehler → silent None (forward-compat: ein Peer mit veraenderten
        // Wire-Formaten darf uns nicht aus dem SPDP druecken).
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
        })
    }
}

fn decode_locator(value: &[u8], little_endian: bool) -> Option<Locator> {
    if value.len() != Locator::WIRE_SIZE {
        return None;
    }
    if !little_endian {
        // Limit: BE-Locator nicht implementiert.
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
        }
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
        // Leere PropertyList → PID_PROPERTY_LIST soll NICHT in den
        // Bytes auftauchen (Abwaerts-Kompatibilitaet: Legacy-Peers
        // die den PID nicht kennen duerfen nicht verwirrt werden).
        let d = sample_data();
        assert!(d.properties.is_empty());
        let bytes = d.to_pl_cdr_le();
        // PID_PROPERTY_LIST = 0x0059 LE = 59 00 ; im Stream suchen
        // (naiv — reicht fuer diesen Test).
        let has_pid = bytes.windows(2).any(|w| w == [0x59, 0x00]);
        assert!(!has_pid, "leere properties muessen PID weglassen");
    }

    #[test]
    fn participant_data_legacy_peer_without_properties_parses_ok() {
        // Peer der keine PID_PROPERTY_LIST schickt → decoded.properties
        // ist leer. Dieses Scenario ist der Default für alle Legacy-
        // ZeroDDS-Peers + alle Cyclone/Fast-DDS ohne Security.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.properties.is_empty());
    }

    #[test]
    fn participant_data_identity_token_pid_roundtrip() {
        // PID_IDENTITY_TOKEN (0x1001) — opaker Wert (CDR-encoded
        // DataHolder, vom Security-Layer geparst). RTPS reicht ihn
        // byte-identisch durch.
        let mut d = sample_data();
        // Wert auf 4-byte aligned, weil ParameterList den PID-Wert
        // mit Zero-Padding bis zur 4-byte-Grenze auffuellt — die
        // parameter_length im PID-Header ist die gepaddete Laenge,
        // und der Decoder kann trailing zeros nicht von echtem Wert-
        // Inhalt unterscheiden. Der Security-Layer-Codec (DataHolder)
        // ignoriert trailing zeros durch sein Parser-Verhalten.
        d.identity_token = Some(vec![0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x02, 0x03, 0x04]);
        let bytes = d.to_pl_cdr_le();
        // PID-Tag 0x1001 LE = 01 10 muss im Stream auftauchen.
        let has_pid = bytes.windows(2).any(|w| w == [0x01, 0x10]);
        assert!(has_pid, "PID_IDENTITY_TOKEN fehlt im PL_CDR_LE-Stream");
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.identity_token, d.identity_token);
    }

    #[test]
    fn participant_data_permissions_token_pid_roundtrip() {
        let mut d = sample_data();
        d.permissions_token = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let bytes = d.to_pl_cdr_le();
        let has_pid = bytes.windows(2).any(|w| w == [0x02, 0x10]);
        assert!(has_pid, "PID_PERMISSIONS_TOKEN fehlt");
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.permissions_token, d.permissions_token);
    }

    #[test]
    fn participant_data_identity_status_token_pid_roundtrip() {
        let mut d = sample_data();
        d.identity_status_token = Some(vec![0x77, 0x88, 0x99, 0xAA]);
        let bytes = d.to_pl_cdr_le();
        let has_pid = bytes.windows(2).any(|w| w == [0x06, 0x10]);
        assert!(has_pid, "PID_IDENTITY_STATUS_TOKEN fehlt");
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.identity_status_token, d.identity_status_token);
    }

    #[test]
    fn participant_data_no_token_pids_in_legacy_announce() {
        // Default-Sample (kein Security) → keiner der drei Token-PIDs
        // taucht auf, damit Legacy-Peers nicht verwirrt werden.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        for pid_le in [[0x01u8, 0x10], [0x02, 0x10], [0x06, 0x10]] {
            let found = bytes.windows(2).any(|w| w == pid_le);
            assert!(
                !found,
                "Token-PID {pid_le:?} darf in Legacy nicht auftauchen"
            );
        }
    }

    #[test]
    fn participant_data_three_tokens_combined_roundtrip() {
        // Realistisches Security-Announce: alle drei Tokens
        // gleichzeitig.
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
        // PID 0x1010 LE = [0x10, 0x10] muss im Stream sein.
        assert!(
            bytes.windows(2).any(|w| w == [0x10, 0x10]),
            "PID 0x1010 fehlt im PL_CDR_LE-Stream"
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
            "PID 0x1011 fehlt"
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
            "PID 0x1012 fehlt"
        );
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.sym_cipher_algo_info, d.sym_cipher_algo_info);
    }

    #[test]
    fn participant_data_no_algo_info_in_legacy_announce() {
        // Default-Sample → keiner der drei Algo-Info-PIDs taucht auf.
        let d = sample_data();
        let bytes = d.to_pl_cdr_le();
        for pid_le in [[0x10u8, 0x10], [0x11, 0x10], [0x12, 0x10]] {
            assert!(
                !bytes.windows(2).any(|w| w == pid_le),
                "Algo-PID {pid_le:?} darf in Legacy nicht auftauchen"
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
        // Sanity: keine zwei Flags belegen den gleichen Bit.
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
        // Bit-Positionen muessen exakt der Spec-Tabelle entsprechen
        // (DDSI-RTPS 2.5 §9.3.2.12 + DDS-Security 1.2 §7.4.7.1).
        // Cyclone DDS und Fast-DDS verlassen sich auf diese exakten
        // Bits — Versatz um 1 Position bricht das Endpoint-Discovery.
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
        // ALL_SECURE muss exakt die 12 Bits 16..=27 setzen, kein Bit
        // mehr und kein Bit weniger (sonst leakt der Default-Build
        // Security-Bits in unsichere Peers).
        let mask = endpoint_flag::ALL_SECURE;
        for bit in 16u32..=27 {
            assert!(mask & (1u32 << bit) != 0, "bit {bit} fehlt in ALL_SECURE");
        }
        // Keine Bits ausserhalb 16..=27.
        let outside_mask: u32 = !((1u32 << 28) - (1u32 << 16));
        assert_eq!(
            mask & outside_mask,
            0,
            "ALL_SECURE darf nur Bits 16..27 setzen"
        );
    }

    #[test]
    fn endpoint_flag_all_standard_excludes_secure_bits() {
        // Default-Standard-Bundle darf KEINE Security-Bits enthalten.
        // Sonst leaken wir Secure-Endpoint-Promises in Peers, ohne
        // dass das `security`-Feature aktiv ist.
        assert_eq!(endpoint_flag::ALL_STANDARD & endpoint_flag::ALL_SECURE, 0);
    }

    #[test]
    fn endpoint_flag_roundtrip_through_pl_cdr() {
        // Encoder muss alle 16 Bits unverfaelscht ueber PL_CDR_LE
        // tragen — sonst verlieren Peers Secure-/WLP-/Topics-Bits.
        let combined = endpoint_flag::ALL_STANDARD | endpoint_flag::ALL_SECURE;
        let mut d = sample_data();
        d.builtin_endpoint_set = combined;
        let bytes = d.to_pl_cdr_le();
        let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.builtin_endpoint_set, combined);
    }
}
