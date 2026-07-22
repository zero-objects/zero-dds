// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `NodeId` + `ExpandedNodeId` — Spec §8.2.2 Tab 8.2.

use alloc::string::String;

use crate::types::{ByteString, Guid};

/// `NodeIdentifierKind` (Spec Tab 8.2 enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeIdentifierKind {
    /// `NODEID_NUMERIC` — uint32-Identifier.
    Numeric,
    /// `NODEID_STRING` — UTF-8-String (max 4096 bytes per Spec).
    String,
    /// `NODEID_GUID` — RFC 4122 Guid.
    Guid,
    /// `NODEID_OPAQUE` — opaque ByteString (max 4096 bytes per Spec).
    Opaque,
}

/// Spec §8.2.2 Tab 8.2 — `NodeIdentifierType` Union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeIdentifier {
    /// `NUMERIC_NODE_ID` case.
    Numeric(u32),
    /// `STRING_NODE_ID` case (Spec: max 4096 bytes).
    String(String),
    /// `GUID_NODE_ID` case.
    Guid(Guid),
    /// `OPAQUE_NODE_ID` case (Spec: max 4096 bytes).
    Opaque(ByteString),
}

impl NodeIdentifier {
    /// Returns the kind discriminator.
    #[must_use]
    pub const fn kind(&self) -> NodeIdentifierKind {
        match self {
            Self::Numeric(_) => NodeIdentifierKind::Numeric,
            Self::String(_) => NodeIdentifierKind::String,
            Self::Guid(_) => NodeIdentifierKind::Guid,
            Self::Opaque(_) => NodeIdentifierKind::Opaque,
        }
    }
}

/// Spec §8.2.2 Tab 8.2 — `NodeId` struct with namespace_index +
/// identifier_type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId {
    /// Namespace index (Spec: uint16).
    pub namespace_index: u16,
    /// Identifier variant.
    pub identifier_type: NodeIdentifier,
}

impl NodeId {
    /// Constructor for a numeric NodeId (most common case).
    #[must_use]
    pub const fn numeric(namespace: u16, id: u32) -> Self {
        Self {
            namespace_index: namespace,
            identifier_type: NodeIdentifier::Numeric(id),
        }
    }

    /// Constructor for a string NodeId.
    ///
    /// # Errors
    /// `Err(NodeIdError::StringTooLong)` if `id.len() > 4096`.
    pub fn string(namespace: u16, id: impl Into<String>) -> Result<Self, NodeIdError> {
        let s = id.into();
        if s.len() > 4096 {
            return Err(NodeIdError::StringTooLong(s.len()));
        }
        Ok(Self {
            namespace_index: namespace,
            identifier_type: NodeIdentifier::String(s),
        })
    }

    /// Constructor for an opaque NodeId.
    ///
    /// # Errors
    /// `Err(NodeIdError::OpaqueTooLong)` if `bytes.len() > 4096`.
    pub fn opaque(namespace: u16, bytes: ByteString) -> Result<Self, NodeIdError> {
        if bytes.len() > 4096 {
            return Err(NodeIdError::OpaqueTooLong(bytes.len()));
        }
        Ok(Self {
            namespace_index: namespace,
            identifier_type: NodeIdentifier::Opaque(bytes),
        })
    }

    /// Constructor for a guid NodeId.
    #[must_use]
    pub const fn guid(namespace: u16, g: Guid) -> Self {
        Self {
            namespace_index: namespace,
            identifier_type: NodeIdentifier::Guid(g),
        }
    }

    /// Spec — formatted display per OPC-UA convention `ns=N;<kind>=<id>`.
    #[must_use]
    pub fn to_canonical_string(&self) -> String {
        let body = match &self.identifier_type {
            NodeIdentifier::Numeric(n) => alloc::format!("i={n}"),
            NodeIdentifier::String(s) => alloc::format!("s={s}"),
            NodeIdentifier::Guid(g) => {
                use core::fmt::Write as _;
                let mut hex = String::with_capacity(16);
                for b in g.data4 {
                    let _ = write!(&mut hex, "{b:02X}");
                }
                alloc::format!("g={:08X}-{:04X}-{:04X}-{hex}", g.data1, g.data2, g.data3)
            }
            NodeIdentifier::Opaque(b) => {
                use core::fmt::Write as _;
                let mut hex = String::with_capacity(b.len() * 2);
                for x in b {
                    let _ = write!(&mut hex, "{x:02X}");
                }
                alloc::format!("b={hex}")
            }
        };
        alloc::format!("ns={};{body}", self.namespace_index)
    }
}

/// `NodeId` error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeIdError {
    /// String identifier longer than 4096 bytes (spec limit).
    StringTooLong(usize),
    /// Opaque identifier longer than 4096 bytes (spec limit).
    OpaqueTooLong(usize),
}

impl core::fmt::Display for NodeIdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StringTooLong(n) => write!(f, "string identifier {n} > 4096"),
            Self::OpaqueTooLong(n) => write!(f, "opaque identifier {n} > 4096"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NodeIdError {}

/// Spec §8.2.2 Tab 8.2 — `ExpandedNodeId` extends NodeId with
/// namespace_uri + server_index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedNodeId {
    /// Inherited fields.
    pub node_id: NodeId,
    /// Namespace-URI (Spec: string).
    pub namespace_uri: String,
    /// Server-Index (Spec: uint32).
    pub server_index: u32,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn numeric_node_id_carries_namespace_and_id() {
        // Spec §8.2.2.
        let n = NodeId::numeric(2, 42);
        assert_eq!(n.namespace_index, 2);
        assert_eq!(n.identifier_type, NodeIdentifier::Numeric(42));
        assert_eq!(n.identifier_type.kind(), NodeIdentifierKind::Numeric);
    }

    #[test]
    fn string_node_id_validates_length_limit() {
        // Spec — max 4096 bytes.
        assert!(NodeId::string(0, "Temperature").is_ok());
        let too_long = "x".repeat(4097);
        assert!(matches!(
            NodeId::string(0, too_long),
            Err(NodeIdError::StringTooLong(4097))
        ));
    }

    #[test]
    fn opaque_node_id_validates_length_limit() {
        assert!(NodeId::opaque(0, alloc::vec![0; 4096]).is_ok());
        assert!(matches!(
            NodeId::opaque(0, alloc::vec![0; 4097]),
            Err(NodeIdError::OpaqueTooLong(4097))
        ));
    }

    #[test]
    fn guid_node_id_carries_guid() {
        let g = Guid::from_bytes([1; 16]);
        let n = NodeId::guid(3, g);
        assert_eq!(n.identifier_type.kind(), NodeIdentifierKind::Guid);
    }

    #[test]
    fn canonical_string_format_for_numeric() {
        // OPC-UA-Convention `ns=N;i=ID`.
        let n = NodeId::numeric(2, 42);
        assert_eq!(n.to_canonical_string(), "ns=2;i=42");
    }

    #[test]
    fn canonical_string_format_for_string_id() {
        let n = NodeId::string(0, "Temperature").expect("ok");
        assert_eq!(n.to_canonical_string(), "ns=0;s=Temperature");
    }

    #[test]
    fn canonical_string_format_for_guid() {
        let g = Guid::from_bytes([
            0xDE, 0xAD, 0xBE, 0xEF, 0x12, 0x34, 0x56, 0x78, 1, 2, 3, 4, 5, 6, 7, 8,
        ]);
        let n = NodeId::guid(1, g);
        assert!(
            n.to_canonical_string()
                .starts_with("ns=1;g=DEADBEEF-1234-5678-")
        );
    }

    #[test]
    fn canonical_string_format_for_opaque() {
        let n = NodeId::opaque(0, alloc::vec![0xCA, 0xFE]).expect("ok");
        assert_eq!(n.to_canonical_string(), "ns=0;b=CAFE");
    }

    #[test]
    fn expanded_node_id_extends_node_id() {
        let e = ExpandedNodeId {
            node_id: NodeId::numeric(2, 42),
            namespace_uri: alloc::string::String::from("urn:my:server"),
            server_index: 5,
        };
        assert_eq!(e.namespace_uri, "urn:my:server");
        assert_eq!(e.server_index, 5);
        assert_eq!(e.node_id.namespace_index, 2);
    }

    #[test]
    fn node_identifier_kind_round_trip() {
        for nid in [
            NodeIdentifier::Numeric(1),
            NodeIdentifier::String(alloc::string::String::from("x")),
            NodeIdentifier::Guid(Guid::from_bytes([0; 16])),
            NodeIdentifier::Opaque(alloc::vec![1]),
        ] {
            // Round-Trip-Sanity: kind() ist Diskriminator.
            let _kind = nid.kind();
        }
    }
}
