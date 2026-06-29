// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS-OPCUA Gateway 1.0 — Type-System + AddressSpace Mapping.
//!
//! Crate `zerodds-opcua-gateway`. Safety classification: **STANDARD**.
//! Spec `formal/2020-01-01` (`internal/standards/cache/omg/dds-opcua-1.0.pdf`).
//!
//! # Scope
//!
//! We implement the **type-system-mapping** layer from Spec §8.2
//! (OPC-UA → DDS) + §9.2 (DDS → OPC-UA) as a pure-Rust no_std+alloc
//! library:
//!
//! * `BuiltinTypeKind` enum with all 25 OPC-UA built-in type IDs
//!   (Spec §8.2 Tab 8.1).
//! * Primitive-Type-Mapping (Boolean, SByte/Byte, Int16/UInt16,
//!   Int32/UInt32, Int64/UInt64, Float, Double, String) — Spec Tab 8.1.
//! * `NodeId` with all 4 identifier kinds (Numeric, String, Guid,
//!   Opaque) — Spec §8.2.2 Tab 8.2.
//! * `ExpandedNodeId` with namespace URI + server index — Spec §8.2.2.
//! * `NodeClass` enum (8 values) — Spec §8.3.1 Tab 8.3.
//! * `StatusCode` + `Guid` + `ByteString` + `LocalizedText` +
//!   `QualifiedName` models — Spec Tab 8.2.
//! * `BodyEncoding` + `ExtensionObject` + `Variant` + `DataValue` —
//!   Spec Tab 8.2.
//! * AddressSpace-Mapping: DDS Domain → OPC-UA ObjectNode + DDS Topic
//!   → OPC-UA ObjectNode + DDS Sample → OPC-UA Variable — Spec §9.3.
//!
//! # What is not covered
//!
//! * **OPC-UA Binary Wire-Encoding** (Spec OPCUA-06 Mappings) —
//!   the caller layer (typically an external OPC-UA stack like `opcua` or
//!   `open62541`).
//! * **Subscription Service Set Behavior** (Spec §8.4.3) — runtime
//!   logic; the mapping model is exposed, the actual polling/
//!   notification is the caller's.
//! * **Historical Access** (Spec §8.3.4 + §9.3.4) — requires a
//!   historical-server backend.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod address_space;
pub mod conformance;
pub mod data_value;
pub mod dds_to_ua;
pub mod historical;
pub mod node_id;
pub mod service_sets;
pub mod subscription_mapping;
pub mod types;

#[cfg(feature = "std")]
pub mod xml;

pub use address_space::{
    DomainNode, InstanceNode, SampleVariable, TopicNode, build_sample_instance,
    mangle_topic_node_browse_name,
};
pub use conformance::{
    ConformancePoint, DECLARED_CONFORMANCE, declares, is_multi_point_conformant,
};
pub use data_value::{DataValue, ExtensionObject, Variant, VariantValue};
pub use node_id::{ExpandedNodeId, NodeId, NodeIdentifier};
pub use types::{
    BuiltinTypeKind, ByteString, Guid, LocalizedText, NodeClass, QualifiedName, StatusCode,
    map_primitive_to_dds,
};

#[cfg(feature = "std")]
pub use xml::{
    BridgeDef, ConnectionDirection, GatewayConfig, OpcuaXmlError, OpcuaXmlError as XmlError,
    UaConnection, parse_gateway_config,
};
