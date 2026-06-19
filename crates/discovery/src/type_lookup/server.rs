// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup service server side (XTypes 1.3 §7.6.3.3.4).
//!
//! Processes [`GetTypesRequest`] and [`GetTypeDependenciesRequest`]
//! against a local [`TypeRegistry`] and produces replies. Pagination
//! of dependency replies via [`ContinuationPoint`] (§7.6.3.3.3).
//!
//! Spec mapping (§7.6.3.3.4):
//! - Operation `getTypes(type_ids)` → [`TypeLookupServer::handle_get_types`]
//! - Operation `getTypeDependencies(type_ids, continuation_point)` →
//!   [`TypeLookupServer::handle_get_type_dependencies`]
//!
//! Pagination constants:
//! - [`TypeLookupServer::DEFAULT_DEPENDENCY_PAGE_SIZE`] = 100 dependencies
//!   per reply. Configurable via [`TypeLookupServer::with_page_size`].

use alloc::vec::Vec;

use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_information::TypeIdentifierWithSize;
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest, ReplyTypeObject,
};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// Server-side TypeLookup service (responder).
///
/// Holds a reference to a [`TypeRegistry`] and answers RPC requests
/// from its content. Stateless across requests (each request carries
/// its own `ContinuationPoint`).
#[derive(Debug, Clone)]
pub struct TypeLookupServer {
    /// Registry of locally known TypeObjects.
    pub registry: TypeRegistry,
    /// Maximum number of dependencies per `GetTypeDependenciesReply`.
    page_size: usize,
}

impl TypeLookupServer {
    /// Default page size for dependency replies (§7.6.3.3.4 allows any
    /// choice; 100 is a compromise between round trips and reply size).
    pub const DEFAULT_DEPENDENCY_PAGE_SIZE: usize = 100;

    /// Constructs a server over an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: TypeRegistry::new(),
            page_size: Self::DEFAULT_DEPENDENCY_PAGE_SIZE,
        }
    }

    /// Constructs a server over an existing registry.
    #[must_use]
    pub fn with_registry(registry: TypeRegistry) -> Self {
        Self {
            registry,
            page_size: Self::DEFAULT_DEPENDENCY_PAGE_SIZE,
        }
    }

    /// Sets the page size for dependency pagination.
    /// `page_size = 0` is raised to 1 (one iteration per reply).
    #[must_use]
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.max(1);
        self
    }

    /// Currently configured page size.
    #[must_use]
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Answers `getTypes(type_ids)`.
    ///
    /// For each known `EquivalenceHashMinimal/Complete`, the matching
    /// [`ReplyTypeObject`] is inserted. Unknown hashes and non-hash
    /// identifiers (primitives, plain collections — which need no
    /// TypeObject) are skipped.
    #[must_use]
    pub fn handle_get_types(&self, req: &GetTypesRequest) -> GetTypesReply {
        let mut types: Vec<ReplyTypeObject> = Vec::with_capacity(req.type_ids.len());
        for ti in &req.type_ids {
            match ti {
                TypeIdentifier::EquivalenceHashMinimal(h) => {
                    if let Some(m) = self.registry.get_minimal(h) {
                        types.push(ReplyTypeObject::Minimal(m.clone()));
                    }
                }
                TypeIdentifier::EquivalenceHashComplete(h) => {
                    if let Some(c) = self.registry.get_complete(h) {
                        types.push(ReplyTypeObject::Complete(c.clone()));
                    }
                }
                _ => {
                    // Primitives / plain collections — no TypeObject.
                }
            }
        }
        GetTypesReply { types }
    }

    /// Answers `getTypeDependencies(type_ids, continuation_point)`.
    ///
    /// Collects the transitively known dependencies for all requested
    /// hashes. If the overall list contains more than `page_size`
    /// entries, it is split: the reply returns the first `page_size`
    /// and a continuation point that marks the offset for the next
    /// iteration.
    ///
    /// Continuation-point encoding (§7.6.3.3.3): we encode the offset
    /// as 4-byte LE in the first 4 bytes (rest = 0). This is an
    /// implementation choice — the spec only says "opaque, max 32
    /// bytes". External peers treat the CP as a black box and send it
    /// back unchanged.
    #[must_use]
    pub fn handle_get_type_dependencies(
        &self,
        req: &GetTypeDependenciesRequest,
    ) -> GetTypeDependenciesReply {
        // Aggregated sorted dependency list — deterministic across all
        // iterations (otherwise the client could see the same
        // dependency twice or not at all between two calls).
        let all = self.collect_dependencies_sorted(&req.type_ids);

        let offset = decode_continuation_offset(&req.continuation_point);
        if offset >= all.len() {
            // No further dependencies — empty reply with empty CP.
            return GetTypeDependenciesReply {
                dependent_typeids: Vec::new(),
                continuation_point: ContinuationPoint::default(),
            };
        }

        let end = (offset + self.page_size).min(all.len());
        let page = all[offset..end].to_vec();
        let continuation_point = if end < all.len() {
            encode_continuation_offset(end)
        } else {
            ContinuationPoint::default()
        };

        GetTypeDependenciesReply {
            dependent_typeids: page,
            continuation_point,
        }
    }

    /// Sorted, deduplicated list of all dependencies.
    fn collect_dependencies_sorted(
        &self,
        type_ids: &[TypeIdentifier],
    ) -> Vec<TypeIdentifierWithSize> {
        use alloc::collections::BTreeMap;
        let mut map: BTreeMap<EquivalenceHash, u32> = BTreeMap::new();

        for ti in type_ids {
            let root_hash = match ti {
                TypeIdentifier::EquivalenceHashMinimal(h)
                | TypeIdentifier::EquivalenceHashComplete(h) => *h,
                _ => continue,
            };
            for dep in self
                .registry
                .transitive_dependencies(&root_hash, MAX_TRANSITIVE_DEPS)
            {
                let size = self.estimate_size(&dep);
                map.entry(dep).or_insert(size);
            }
        }

        map.into_iter()
            .map(|(h, size)| TypeIdentifierWithSize {
                type_id: TypeIdentifier::EquivalenceHashMinimal(h),
                typeobject_serialized_size: size,
            })
            .collect()
    }

    /// Determines the serialized size of a registered type.
    /// If not in the registry: 0.
    fn estimate_size(&self, hash: &EquivalenceHash) -> u32 {
        if let Some(m) = self.registry.get_minimal(hash) {
            TypeObject::Minimal(m.clone())
                .to_bytes_le()
                .map(|b| u32::try_from(b.len()).unwrap_or(0))
                .unwrap_or(0)
        } else if let Some(c) = self.registry.get_complete(hash) {
            TypeObject::Complete(c.clone())
                .to_bytes_le()
                .map(|b| u32::try_from(b.len()).unwrap_or(0))
                .unwrap_or(0)
        } else {
            0
        }
    }
}

impl Default for TypeLookupServer {
    fn default() -> Self {
        Self::new()
    }
}

/// DoS cap for transitive dependency resolution (within a single
/// server call). Prevents a malicious peer with a large type graph
/// from inflating a single reply without bound.
const MAX_TRANSITIVE_DEPS: usize = 4_096;

/// Encodes a pagination offset as an 8-byte continuation point.
/// Format: `[offset_le_u64; 8 bytes]`. The rest of MAX_LEN stays
/// unused.
fn encode_continuation_offset(offset: usize) -> ContinuationPoint {
    let off64 = u64::try_from(offset).unwrap_or(u64::MAX);
    let bytes = off64.to_le_bytes();
    ContinuationPoint(bytes.to_vec())
}

/// Decodes a pagination offset.
/// Empty or too-short CPs are interpreted as offset 0
/// (= "first iteration").
fn decode_continuation_offset(cp: &ContinuationPoint) -> usize {
    if cp.0.len() < 8 {
        return 0;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&cp.0[..8]);
    usize::try_from(u64::from_le_bytes(buf)).unwrap_or(usize::MAX)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_types::builder::TypeObjectBuilder;
    use zerodds_types::{MinimalTypeObject, PrimitiveKind};

    fn sample_struct(name: &str) -> MinimalTypeObject {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type(name)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
                .build_minimal(),
        )
    }

    #[test]
    fn handle_get_types_unknown_returns_empty() {
        let server = TypeLookupServer::new();
        let req = GetTypesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                [0xAA; 14]
            ))],
        };
        let reply = server.handle_get_types(&req);
        assert!(reply.types.is_empty());
    }

    #[test]
    fn handle_get_types_skips_primitives() {
        let server = TypeLookupServer::new();
        let req = GetTypesRequest {
            type_ids: alloc::vec![TypeIdentifier::Primitive(PrimitiveKind::Int32)],
        };
        let reply = server.handle_get_types(&req);
        assert!(reply.types.is_empty());
    }

    #[test]
    fn pagination_offset_encoding_roundtrip() {
        let cp = encode_continuation_offset(123_456);
        assert_eq!(decode_continuation_offset(&cp), 123_456);
        let cp = encode_continuation_offset(0);
        assert_eq!(decode_continuation_offset(&cp), 0);
    }

    #[test]
    fn pagination_truncates_at_page_size() {
        let mut server = TypeLookupServer::new().with_page_size(3);
        // Build a struct with 5 hash member refs (not in registry).
        let mut builder = TypeObjectBuilder::struct_type("::Root");
        let dep_hashes: alloc::vec::Vec<EquivalenceHash> = (0..5u8)
            .map(|i| {
                let mut b = [0u8; 14];
                b[0] = i;
                EquivalenceHash(b)
            })
            .collect();
        for (i, h) in dep_hashes.iter().enumerate() {
            builder = builder.member(
                alloc::format!("m{i}").as_str(),
                TypeIdentifier::EquivalenceHashMinimal(*h),
                |m| m,
            );
        }
        let root = MinimalTypeObject::Struct(builder.build_minimal());
        let root_hash = zerodds_types::compute_minimal_hash(&root).unwrap();
        server.registry.insert_minimal(root_hash, root);

        // First page → 3 deps + non-empty CP.
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
            continuation_point: ContinuationPoint::default(),
        };
        let reply = server.handle_get_type_dependencies(&req);
        assert_eq!(reply.dependent_typeids.len(), 3);
        assert!(!reply.continuation_point.0.is_empty());

        // Second page → 2 deps + empty CP.
        let req2 = GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
            continuation_point: reply.continuation_point.clone(),
        };
        let reply2 = server.handle_get_type_dependencies(&req2);
        assert_eq!(reply2.dependent_typeids.len(), 2);
        assert!(reply2.continuation_point.0.is_empty());
    }

    #[test]
    fn handle_get_type_dependencies_empty_when_no_deps() {
        let mut server = TypeLookupServer::new();
        let m = sample_struct("::Empty");
        let h = zerodds_types::compute_minimal_hash(&m).unwrap();
        server.registry.insert_minimal(h, m);
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(h)],
            continuation_point: ContinuationPoint::default(),
        };
        let reply = server.handle_get_type_dependencies(&req);
        assert!(reply.dependent_typeids.is_empty());
        assert!(reply.continuation_point.0.is_empty());
    }

    #[test]
    fn page_size_zero_normalizes_to_one() {
        let server = TypeLookupServer::new().with_page_size(0);
        assert_eq!(server.page_size(), 1);
    }
}
