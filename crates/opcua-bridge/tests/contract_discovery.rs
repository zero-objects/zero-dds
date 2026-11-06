// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-discovery`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-discovery`, only with its `daemon`
//! feature (`src/dds_bridge.rs::resolve_discovered`): no path names the
//! crate; it locks `DcpsRuntime::type_lookup_server`, a
//! `type_lookup::TypeLookupServer`, and reads its public field `registry`
//! (`get_complete`, `transitive_dependencies`) to resolve a discovered
//! endpoint's complete type identifier, `None` when the registry does not
//! hold it.
//!
//! Oracle: the field's documented role ("Registry of locally known
//! TypeObjects") and DDS-XTypes 1.3 §7.3.4.5 (an object is filed under the
//! hash of its serialization). The complete TypeObject of `@final struct
//! Robot { unsigned long id; unsigned long label; }` below was written by
//! hand from Annex B; its MD5, computed with Python's `hashlib`, gives the
//! complete hash Cyclone DDS 11.0.1 announces
//! (`crates/types/tests/typeobject_serialized_size.rs`).
#![allow(clippy::unwrap_used)]

use zerodds_discovery::type_lookup::TypeLookupServer;
use zerodds_types::EquivalenceHash;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;

#[rustfmt::skip]
const ROBOT_COMPLETE: [u8; 84] = [
    0x50, 0x00, 0x00, 0x00, 0xf2, 0x51, 0x01, 0x00,
    0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00, 0x00, 0x00,
    0x34, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x03, 0x00, 0x00, 0x00, b'i', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x06, 0x00, 0x00, 0x00, b'l', b'a', b'b', b'e', b'l', 0x00, 0x00, 0x00,
];

const ROBOT_COMPLETE_HASH: [u8; 14] = [
    0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
];

#[test]
fn the_servers_registry_is_the_types_registry_the_bridge_reads() {
    fn registry(server: &TypeLookupServer) -> &TypeRegistry {
        &server.registry
    }
    let mut filled = TypeRegistry::new();
    filled
        .insert_object(TypeObject::from_bytes_le(&ROBOT_COMPLETE).unwrap())
        .unwrap();
    let server = TypeLookupServer::with_registry(filled);
    let hash = EquivalenceHash(ROBOT_COMPLETE_HASH);
    assert!(registry(&server).get_complete(&hash).is_some());
    assert!(
        registry(&server)
            .transitive_dependencies(&hash, zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES)
            .is_empty()
    );
}

#[test]
fn a_hash_the_registry_does_not_hold_resolves_to_nothing() {
    let server = TypeLookupServer::new();
    assert!(
        server
            .registry
            .get_complete(&EquivalenceHash(ROBOT_COMPLETE_HASH))
            .is_none()
    );
}
