// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup Builtin-Endpoint-GUIDs — XTypes 1.3 §7.6.3.3.4.
//!
//! ## Spec-Mapping
//!
//! - 4 Builtin-Endpoints pro Participant: TL_SVC_REQUEST_{WRITER,READER}
//!   und TL_SVC_REPLY_{WRITER,READER}.
//! - QoS: Reliability=RELIABLE, Durability=VOLATILE, History=KEEP_LAST(1).
//! - Topic-Names: `dds.builtin.TOS.<HEX-GUID>` (Service-Instance-Name).
//!
//! Dieses Modul liefert die GUIDs + Service-Instance-Name-Formatter.
//! Die Instantiierung der Reliable-Writer/Reader-Pairs liegt im
//! DCPS-Layer (siehe `crates/dcps/src/runtime.rs` Builtin-Endpoint-
//! Spawn-Pfad).

use alloc::format;
use alloc::string::String;

use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

/// Topic-Name-Praefix fuer TypeLookup-Service-Topics (§7.6.3.3.4).
pub const TYPELOOKUP_TOPIC_PREFIX: &str = "dds.builtin.TOS";

/// Bildet den Service-Instance-Namen `dds.builtin.TOS.<HEX>` aus
/// einem `GuidPrefix` (§7.6.3.3.4 c).
///
/// Der Hex-Anteil ist **24 Hex-Chars** (12 Bytes GuidPrefix). Die
/// Spec spricht von "16-hex" was sich auf den 8-byte-RPC-Service-
/// Instance-Identifier-Anteil bezieht; in unserer Implementation
/// nutzen wir den vollen 12-byte GuidPrefix als eindeutigen
/// Participant-Identifier — kompatibel mit Cyclone DDS' Wahl.
#[must_use]
pub fn format_service_instance_name(prefix: &GuidPrefix) -> String {
    let mut hex = String::with_capacity(24);
    for b in prefix.0.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("{TYPELOOKUP_TOPIC_PREFIX}.{hex}")
}

/// Strikt-spec-treue Variante: nur die ersten 8 Bytes des Prefix
/// werden als Hex codiert (16 Hex-Chars). Wird von einigen
/// Implementierungen als "Service-Instance-Identifier" benutzt.
#[must_use]
pub fn format_service_instance_name_short(prefix: &GuidPrefix) -> String {
    let mut hex = String::with_capacity(16);
    for b in prefix.0[..8].iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("{TYPELOOKUP_TOPIC_PREFIX}.{hex}")
}

/// Vollstaendiges Wiring der vier Builtin-Endpoint-GUIDs eines
/// Participants fuer den TypeLookup-Service.
///
/// QoS (§7.6.3.3.4): Reliability=RELIABLE, Durability=VOLATILE,
/// History=KEEP_LAST(1). Wird vom DCPS-Runtime gelesen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLookupEndpoints {
    /// Eigener `GuidPrefix`.
    pub prefix: GuidPrefix,
}

impl TypeLookupEndpoints {
    /// Konstruktor.
    #[must_use]
    pub fn new(prefix: GuidPrefix) -> Self {
        Self { prefix }
    }

    /// GUID des Request-Writers (Client-Outgoing → Server-Incoming).
    #[must_use]
    pub fn request_writer(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REQ_WRITER)
    }

    /// GUID des Request-Readers (Server-Incoming).
    #[must_use]
    pub fn request_reader(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REQ_READER)
    }

    /// GUID des Reply-Writers (Server-Outgoing).
    #[must_use]
    pub fn reply_writer(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REPLY_WRITER)
    }

    /// GUID des Reply-Readers (Client-Incoming).
    #[must_use]
    pub fn reply_reader(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REPLY_READER)
    }

    /// Service-Instance-Name fuer beide Topics (§7.6.3.3.4 c).
    #[must_use]
    pub fn service_instance_name(&self) -> String {
        format_service_instance_name(&self.prefix)
    }

    /// Alle vier GUIDs als Array — fuer Iterieren bei Endpoint-
    /// Registrierung im RTPS-Stack.
    #[must_use]
    pub fn all_guids(&self) -> [Guid; 4] {
        [
            self.request_writer(),
            self.request_reader(),
            self.reply_writer(),
            self.reply_reader(),
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn service_instance_name_format() {
        let prefix = GuidPrefix::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ]);
        let name = format_service_instance_name(&prefix);
        assert_eq!(name, "dds.builtin.TOS.0102030405060708090a0b0c");
        assert!(name.starts_with(TYPELOOKUP_TOPIC_PREFIX));
    }

    #[test]
    fn service_instance_name_short_takes_first_8_bytes() {
        let prefix = GuidPrefix::from_bytes([
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
        ]);
        let name = format_service_instance_name_short(&prefix);
        assert_eq!(name, "dds.builtin.TOS.aabbccddeeff0011");
    }

    #[test]
    fn endpoints_have_distinct_entity_ids() {
        let e = TypeLookupEndpoints::new(GuidPrefix::from_bytes([0; 12]));
        let g = e.all_guids();
        for i in 0..g.len() {
            for j in (i + 1)..g.len() {
                assert_ne!(g[i].entity_id, g[j].entity_id);
            }
        }
    }

    #[test]
    fn endpoints_share_prefix() {
        let prefix = GuidPrefix::from_bytes([0xAA; 12]);
        let e = TypeLookupEndpoints::new(prefix);
        for g in e.all_guids() {
            assert_eq!(g.prefix, prefix);
        }
    }

    #[test]
    fn entity_ids_match_spec_constants() {
        let e = TypeLookupEndpoints::new(GuidPrefix::from_bytes([1; 12]));
        assert_eq!(e.request_writer().entity_id, EntityId::TL_SVC_REQ_WRITER);
        assert_eq!(e.request_reader().entity_id, EntityId::TL_SVC_REQ_READER);
        assert_eq!(e.reply_writer().entity_id, EntityId::TL_SVC_REPLY_WRITER);
        assert_eq!(e.reply_reader().entity_id, EntityId::TL_SVC_REPLY_READER);
    }

    #[test]
    fn service_instance_name_via_endpoints() {
        let prefix = GuidPrefix::from_bytes([0x00; 12]);
        let e = TypeLookupEndpoints::new(prefix);
        assert_eq!(
            e.service_instance_name(),
            format_service_instance_name(&prefix)
        );
    }
}
