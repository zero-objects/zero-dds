// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup builtin endpoint GUIDs — XTypes 1.3 §7.6.3.3.4.
//!
//! ## Spec mapping
//!
//! - 4 builtin endpoints per participant: TL_SVC_REQUEST_{WRITER,READER}
//!   and TL_SVC_REPLY_{WRITER,READER}.
//! - QoS: Reliability=RELIABLE, Durability=VOLATILE, History=KEEP_LAST(1).
//! - Topic names: `dds.builtin.TOS.<HEX-GUID>` (service instance name).
//!
//! This module provides the GUIDs + service-instance-name formatter.
//! The instantiation of the reliable writer/reader pairs lives in the
//! DCPS layer (see `crates/dcps/src/runtime.rs` builtin-endpoint
//! spawn path).

use alloc::format;
use alloc::string::String;

use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

/// Topic-name prefix for TypeLookup service topics (§7.6.3.3.4).
pub const TYPELOOKUP_TOPIC_PREFIX: &str = "dds.builtin.TOS";

/// Builds the service instance name `dds.builtin.TOS.<HEX>` from
/// a `GuidPrefix` (§7.6.3.3.4 c).
///
/// The hex part is the full participant GUID: 24 hex prefix characters
/// followed by participant EntityId 000001c1, as in Cyclone's service client.
#[must_use]
pub fn format_service_instance_name(prefix: &GuidPrefix) -> String {
    zerodds_types::type_lookup::service_instance_name(&prefix.0)
}

/// Historical shortened variant. Not the standard TypeLookup service name.
#[must_use]
pub fn format_service_instance_name_short(prefix: &GuidPrefix) -> String {
    let mut hex = String::with_capacity(16);
    for b in prefix.0[..8].iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("{TYPELOOKUP_TOPIC_PREFIX}.{hex}")
}

/// Complete wiring of a participant's four builtin endpoint GUIDs
/// for the TypeLookup service.
///
/// QoS (§7.6.3.3.4): Reliability=RELIABLE, Durability=VOLATILE,
/// History=KEEP_LAST(1). Read by the DCPS runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLookupEndpoints {
    /// Own `GuidPrefix`.
    pub prefix: GuidPrefix,
}

impl TypeLookupEndpoints {
    /// Constructor.
    #[must_use]
    pub fn new(prefix: GuidPrefix) -> Self {
        Self { prefix }
    }

    /// GUID of the request writer (client outgoing → server incoming).
    #[must_use]
    pub fn request_writer(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REQ_WRITER)
    }

    /// GUID of the request reader (server incoming).
    #[must_use]
    pub fn request_reader(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REQ_READER)
    }

    /// GUID of the reply writer (server outgoing).
    #[must_use]
    pub fn reply_writer(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REPLY_WRITER)
    }

    /// GUID of the reply reader (client incoming).
    #[must_use]
    pub fn reply_reader(&self) -> Guid {
        Guid::new(self.prefix, EntityId::TL_SVC_REPLY_READER)
    }

    /// Service instance name for both topics (§7.6.3.3.4 c).
    #[must_use]
    pub fn service_instance_name(&self) -> String {
        format_service_instance_name(&self.prefix)
    }

    /// All four GUIDs as an array — for iterating during endpoint
    /// registration in the RTPS stack.
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
    fn service_instance_name_short_takes_first_8_bytes() {
        let prefix = GuidPrefix::from_bytes([
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
        ]);
        let name = format_service_instance_name_short(&prefix);
        assert_eq!(name, "dds.builtin.TOS.aabbccddeeff0011");
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
    fn xtypes_7_6_3_3_3_entity_ids_match_spec_constants() {
        let e = TypeLookupEndpoints::new(GuidPrefix::from_bytes([1; 12]));
        assert_eq!(e.request_writer().entity_id, EntityId::TL_SVC_REQ_WRITER);
        assert_eq!(e.request_reader().entity_id, EntityId::TL_SVC_REQ_READER);
        assert_eq!(e.reply_writer().entity_id, EntityId::TL_SVC_REPLY_WRITER);
        assert_eq!(e.reply_reader().entity_id, EntityId::TL_SVC_REPLY_READER);
    }
}
