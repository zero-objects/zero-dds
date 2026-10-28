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

use zerodds_types::hash::{complete_serialized_size, minimal_serialized_size};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_information::TypeIdentifierWithSize;
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest, TypeIdentifierPair, TypeIdentifierTypeObjectPair,
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
    /// [`TypeIdentifierTypeObjectPair`] is inserted — the object *and*
    /// the identifier it was asked for under, which is what
    /// §7.6.3.3.3 puts in `TypeLookup_getTypes_Out::types`. Unknown
    /// hashes and non-hash identifiers (primitives, plain collections —
    /// which need no TypeObject) are skipped.
    ///
    /// `complete_to_minimal` stays empty: §7.6.3.3.4.2 asks for it only
    /// where a server answers a MINIMAL question with a COMPLETE object,
    /// and this server answers each question in the equivalence kind it
    /// was asked in.
    #[must_use]
    pub fn handle_get_types(&self, req: &GetTypesRequest) -> GetTypesReply {
        let mut types: Vec<TypeIdentifierTypeObjectPair> = Vec::with_capacity(req.type_ids.len());
        let mut complete_to_minimal: Vec<TypeIdentifierPair> = Vec::new();
        for ti in &req.type_ids {
            match ti {
                TypeIdentifier::EquivalenceHashMinimal(h) => {
                    if let Some(m) = self.registry.get_minimal(h) {
                        types.push(TypeIdentifierTypeObjectPair {
                            type_identifier: ti.clone(),
                            type_object: TypeObject::Minimal(m.clone()),
                        });
                    }
                }
                TypeIdentifier::EquivalenceHashComplete(h) => {
                    if let Some(c) = self.registry.get_complete(h) {
                        types.push(TypeIdentifierTypeObjectPair {
                            type_identifier: ti.clone(),
                            type_object: TypeObject::Complete(c.clone()),
                        });
                        // XTypes 1.3 §7.6.3.3.4.2: for every COMPLETE
                        // identifier answered, the pair naming its
                        // MINIMAL counterpart. A requester that resolved
                        // an endpoint's complete identity — RTI Connext
                        // 7.7.0 announces and asks for that form — needs
                        // this to reach the minimal object the
                        // assignability rules compare; an empty list
                        // left it with a complete object and no way to
                        // its minimal hash. Measured 2026-09-13 on the
                        // OMG dds-xtypes suite: RTI asked once, got the
                        // complete object, and never matched.
                        if let Some(minimal) = self.registry.minimal_hash_of_complete(h) {
                            complete_to_minimal.push(TypeIdentifierPair {
                                type_identifier1: ti.clone(),
                                type_identifier2: TypeIdentifier::EquivalenceHashMinimal(minimal),
                            });
                        }
                    }
                }
                TypeIdentifier::StronglyConnectedComponent(scc) => {
                    // §7.6.3.3: "If a TypeIdentifier was a SCCIdentifier,
                    // then the response shall treat the TypeObjects within
                    // the Strongly Connected Components atomically. Either
                    // include all in the reply or none." Every member goes
                    // in under the identifier of its own index, or the
                    // component is absent from the reply. The registry
                    // records a component in both forms; the form asked
                    // for is the form answered, and a COMPLETE answer
                    // carries the pairs that name each member's MINIMAL
                    // counterpart (§7.6.3.3.4.2).
                    use zerodds_types::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL};
                    let index_of =
                        |position: usize| i32::try_from(position + 1).unwrap_or(i32::MAX);
                    match scc.equivalence_kind {
                        EK_COMPLETE => {
                            let Some(members) = self.registry.component_members(&scc.hash) else {
                                continue;
                            };
                            if i32::try_from(members.len()).ok() != Some(scc.scc_length) {
                                continue;
                            }
                            let minimal_hash = self.registry.minimal_component_hash(&scc.hash);
                            for (position, member) in members.iter().enumerate() {
                                let mut id = *scc;
                                id.scc_index = index_of(position);
                                types.push(TypeIdentifierTypeObjectPair {
                                    type_identifier: TypeIdentifier::StronglyConnectedComponent(id),
                                    type_object: TypeObject::Complete(member.clone()),
                                });
                                if let Some(hash) = minimal_hash {
                                    let mut minimal = id;
                                    minimal.equivalence_kind = EK_MINIMAL;
                                    minimal.hash = hash;
                                    complete_to_minimal.push(TypeIdentifierPair {
                                        type_identifier1:
                                            TypeIdentifier::StronglyConnectedComponent(id),
                                        type_identifier2:
                                            TypeIdentifier::StronglyConnectedComponent(minimal),
                                    });
                                }
                            }
                        }
                        EK_MINIMAL => {
                            let Some(members) = self.registry.minimal_component_members(&scc.hash)
                            else {
                                continue;
                            };
                            if i32::try_from(members.len()).ok() != Some(scc.scc_length) {
                                continue;
                            }
                            for (position, member) in members.iter().enumerate() {
                                let mut id = *scc;
                                id.scc_index = index_of(position);
                                types.push(TypeIdentifierTypeObjectPair {
                                    type_identifier: TypeIdentifier::StronglyConnectedComponent(id),
                                    type_object: TypeObject::Minimal(member.clone()),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {
                    // Primitives / plain collections — no TypeObject.
                }
            }
        }
        // The closure, beside the objects asked for. §7.6.3.3.4.2 lets a
        // getTypes reply carry more than was requested, and a requester
        // that converts an object as it arrives needs the types the
        // object names by hash to be there already: RTI Connext 7.7.0
        // fails the whole reply ("failure creating TypeCode from
        // TypeIdentifier") when a member's type is not in its library,
        // and never asks again. Measured 2026-09-13 on the OMG
        // dds-xtypes suite, every struct with an enumerated, nested or
        // union member against RTI. Both forms of each dependency go in,
        // each once.
        // Dependencies first, the requested objects after them: a
        // requester converting entries in reply order needs a member's
        // type filed before the aggregate that names it. RTI's library
        // lookup failed on the aggregate when the closure followed it.
        let asked: Vec<TypeIdentifier> = types.iter().map(|t| t.type_identifier.clone()).collect();
        let requested = core::mem::take(&mut types);
        for ti in &asked {
            let dependencies = match ti {
                TypeIdentifier::EquivalenceHashMinimal(h)
                | TypeIdentifier::EquivalenceHashComplete(h) => {
                    self.registry.transitive_dependencies(h, 256)
                }
                // The types a component's members name by hash are part of
                // the answer for the component, as a hashed type's are.
                TypeIdentifier::StronglyConnectedComponent(scc) => {
                    let complete_hash = if scc.equivalence_kind
                        == zerodds_types::type_identifier::kinds::EK_COMPLETE
                    {
                        Some(scc.hash)
                    } else {
                        self.registry.complete_component_hash(&scc.hash)
                    };
                    let mut found = complete_hash
                        .map(|hash| self.registry.component_dependencies(&hash))
                        .unwrap_or_default();
                    found.truncate(256);
                    found
                }
                _ => continue,
            };
            for dep in dependencies {
                let complete = TypeIdentifier::EquivalenceHashComplete(dep);
                if !types.iter().any(|t| t.type_identifier == complete) {
                    if let Some(c) = self.registry.get_complete(&dep) {
                        types.push(TypeIdentifierTypeObjectPair {
                            type_identifier: complete.clone(),
                            type_object: TypeObject::Complete(c.clone()),
                        });
                        if let Some(minimal) = self.registry.minimal_hash_of_complete(&dep) {
                            complete_to_minimal.push(TypeIdentifierPair {
                                type_identifier1: complete,
                                type_identifier2: TypeIdentifier::EquivalenceHashMinimal(minimal),
                            });
                        }
                    }
                }
                let minimal = TypeIdentifier::EquivalenceHashMinimal(dep);
                if !types.iter().any(|t| t.type_identifier == minimal) {
                    if let Some(m) = self.registry.get_minimal(&dep) {
                        types.push(TypeIdentifierTypeObjectPair {
                            type_identifier: minimal,
                            type_object: TypeObject::Minimal(m.clone()),
                        });
                    }
                }
            }
        }
        for pair in requested {
            if !types
                .iter()
                .any(|t| t.type_identifier == pair.type_identifier)
            {
                types.push(pair);
            }
        }
        // Both forms of every type in the reply. A requester that asked
        // for the complete form and converts it to minimal as it arrives
        // needs each member type's MINIMAL object in its library to build
        // the member's TypeCode — RTI Connext 7.7.0 does exactly this and
        // fails otherwise ("createMinimalTypeObjectV2FromCompleteTypeObjectV2:
        // FAILED TO CONVERT", the member's minimal identifier "getting
        // TypeObjectV2 from library"), with nothing left to request because
        // the complete closure is already present. Measured 2026-09-13 on
        // the OMG dds-xtypes suite, every ZeroDDS type with an enumerated,
        // nested or union member against an RTI reader. §7.6.3.3.4.2 lets
        // the reply carry more than was asked, so the counterpart form of
        // each type rides along; a requester that needs only one ignores
        // the other.
        let counterparts: Vec<TypeIdentifierTypeObjectPair> = types
            .iter()
            .filter_map(|t| match &t.type_identifier {
                TypeIdentifier::EquivalenceHashComplete(h) => {
                    let m = self.registry.minimal_hash_of_complete(h)?;
                    let obj = self.registry.get_minimal(&m)?;
                    Some(TypeIdentifierTypeObjectPair {
                        type_identifier: TypeIdentifier::EquivalenceHashMinimal(m),
                        type_object: TypeObject::Minimal(obj.clone()),
                    })
                }
                TypeIdentifier::EquivalenceHashMinimal(_) => None,
                _ => None,
            })
            .collect();
        for pair in counterparts {
            if !types
                .iter()
                .any(|t| t.type_identifier == pair.type_identifier)
            {
                types.push(pair);
            }
        }
        GetTypesReply {
            types,
            complete_to_minimal,
        }
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

    /// Sorted, deduplicated list of all dependencies, each in the form
    /// of the identifier it was reached from: a complete object names
    /// complete hashes and a minimal one minimal (§7.3.4.5), so the
    /// dependencies of a COMPLETE root are COMPLETE identifiers. Every
    /// dependency used to be answered as MINIMAL, whichever form was
    /// asked; RTI Connext 7.7.0, which asks for the complete form, found
    /// nothing it could request ("there are no type identifiers to
    /// request and the type is not resolved") and failed the aggregate
    /// on its first member. Measured 2026-09-13 on the OMG dds-xtypes
    /// suite.
    fn collect_dependencies_sorted(
        &self,
        type_ids: &[TypeIdentifier],
    ) -> Vec<TypeIdentifierWithSize> {
        use alloc::collections::BTreeMap;
        let mut map: BTreeMap<(bool, EquivalenceHash), u32> = BTreeMap::new();

        let mut of_components: Vec<TypeIdentifierWithSize> = Vec::new();
        for ti in type_ids {
            let (root_hash, complete) = match ti {
                TypeIdentifier::EquivalenceHashMinimal(h) => (*h, false),
                TypeIdentifier::EquivalenceHashComplete(h) => (*h, true),
                TypeIdentifier::StronglyConnectedComponent(scc) => {
                    for dependent in self.component_dependents(scc) {
                        if !of_components.contains(&dependent) {
                            of_components.push(dependent);
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            for dep in self
                .registry
                .transitive_dependencies(&root_hash, MAX_TRANSITIVE_DEPS)
            {
                let size = self.estimate_size(&dep);
                map.entry((complete, dep)).or_insert(size);
            }
        }

        let mut sorted: Vec<TypeIdentifierWithSize> = map
            .into_iter()
            .map(|((complete, h), size)| TypeIdentifierWithSize {
                type_id: if complete {
                    TypeIdentifier::EquivalenceHashComplete(h)
                } else {
                    TypeIdentifier::EquivalenceHashMinimal(h)
                },
                typeobject_serialized_size: size,
            })
            .collect();
        // The dependencies of a component come after the hashed ones, in
        // the order the component announces them.
        for dependent in of_components {
            if !sorted.contains(&dependent) {
                sorted.push(dependent);
            }
        }
        sorted
    }

    /// What the component `scc` names depends on, in the form of `scc`:
    /// the hash identifiers of its members' types and the `SCCIdentifier`
    /// of every other component they reach (§7.6.3.3.4.1; §7.6.3.2.1 for
    /// the rule that a component is named as a whole). The component's own
    /// `SCCIdentifier` is not a dependency of itself.
    ///
    /// Empty when the registry does not hold the component in both forms.
    fn component_dependents(
        &self,
        scc: &zerodds_types::type_identifier::StronglyConnectedComponentId,
    ) -> Vec<TypeIdentifierWithSize> {
        use zerodds_types::type_identifier::kinds::EK_COMPLETE;
        let complete_hash = if scc.equivalence_kind == EK_COMPLETE {
            Some(scc.hash)
        } else {
            self.registry.complete_component_hash(&scc.hash)
        };
        let Some(complete_hash) = complete_hash else {
            return Vec::new();
        };
        let mut as_complete = *scc;
        as_complete.equivalence_kind = EK_COMPLETE;
        as_complete.hash = complete_hash;
        as_complete.scc_index = 1;
        let Ok(information) =
            zerodds_types::TypeInformation::from_component(&as_complete, &self.registry)
        else {
            return Vec::new();
        };
        let side = if scc.equivalence_kind == EK_COMPLETE {
            information.complete
        } else {
            information.minimal
        };
        side.dependent_typeids.into_iter().skip(1).collect()
    }

    /// Determines the serialized size of a registered type.
    /// If not in the registry: 0.
    ///
    /// This is the same `typeobject_serialized_size` the SEDP announcement
    /// carries, and it is measured the same way — over the DHEADER-delimited
    /// stream the EquivalenceHash is taken over, see
    /// `zerodds_types::hash::minimal_serialized_size`. A dependency announced
    /// four octets short here is the same defect as one announced four octets
    /// short in `PID_TYPE_INFORMATION`, only in the reply a peer reads after
    /// it already matched.
    fn estimate_size(&self, hash: &EquivalenceHash) -> u32 {
        if let Some(m) = self.registry.get_minimal(hash) {
            minimal_serialized_size(m).unwrap_or(0)
        } else if let Some(c) = self.registry.get_complete(hash) {
            complete_serialized_size(c).unwrap_or(0)
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

    /// §7.6.3.3.4.2: a COMPLETE identifier is answered with its object
    /// and the pair naming its MINIMAL hash.
    /// A COMPLETE root's dependencies come back as COMPLETE identifiers
    /// with the complete objects' sizes; the same type asked in the
    /// MINIMAL form gets MINIMAL ones. RTI Connext 7.7.0 asks in the
    /// complete form and requests exactly what this reply names.
    #[test]
    fn handle_get_type_dependencies_answers_in_the_form_asked() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        let mut server = TypeLookupServer::new();
        let inner = zerodds_types::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::tl::Inner")
                .extensibility(Extensibility::Appendable)
                .member(
                    "a",
                    TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32),
                    |m| m,
                )
                .build_complete(),
        );
        let inner_hash = zerodds_types::compute_complete_hash(&inner).expect("hash");
        let outer = zerodds_types::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::tl::Outer")
                .extensibility(Extensibility::Appendable)
                .member(
                    "i",
                    TypeIdentifier::EquivalenceHashComplete(inner_hash),
                    |m| m,
                )
                .build_complete(),
        );
        let outer_hash = zerodds_types::compute_complete_hash(&outer).expect("hash");
        server.registry.insert_complete(inner_hash, inner);
        server.registry.insert_complete(outer_hash, outer);
        server.registry.project_completes_to_minimal();

        let reply = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashComplete(outer_hash)],
            continuation_point: ContinuationPoint::default(),
        });
        assert_eq!(
            reply
                .dependent_typeids
                .iter()
                .map(|d| d.type_id.clone())
                .collect::<Vec<_>>(),
            alloc::vec![TypeIdentifier::EquivalenceHashComplete(inner_hash)]
        );
        assert!(reply.dependent_typeids[0].typeobject_serialized_size > 0);

        let outer_min = server
            .registry
            .minimal_hash_of_complete(&outer_hash)
            .expect("projected");
        let inner_min = server
            .registry
            .minimal_hash_of_complete(&inner_hash)
            .expect("projected");
        let reply = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(outer_min)],
            continuation_point: ContinuationPoint::default(),
        });
        assert_eq!(
            reply
                .dependent_typeids
                .iter()
                .map(|d| d.type_id.clone())
                .collect::<Vec<_>>(),
            alloc::vec![TypeIdentifier::EquivalenceHashMinimal(inner_min)]
        );
    }

    #[test]
    fn handle_get_types_pairs_a_complete_identifier_with_its_minimal_hash() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        let mut server = TypeLookupServer::new();
        let complete = TypeObjectBuilder::struct_type("::tl::Pair")
            .extensibility(Extensibility::Appendable)
            .member(
                "a",
                TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32),
                |m| m,
            )
            .build_complete();
        let complete = zerodds_types::CompleteTypeObject::Struct(complete);
        let hash = zerodds_types::compute_complete_hash(&complete).expect("hash");
        server.registry.insert_complete(hash, complete);
        server.registry.project_completes_to_minimal();
        let reply = server.handle_get_types(&GetTypesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashComplete(hash)],
        });
        // The complete object asked for, and its minimal counterpart
        // beside it (a requester converting complete to minimal needs
        // the minimal object in its library).
        assert_eq!(reply.types.len(), 2);
        assert!(reply.types.iter().any(|t| matches!(
            t.type_identifier,
            TypeIdentifier::EquivalenceHashComplete(h) if h == hash
        )));
        assert!(
            reply
                .types
                .iter()
                .any(|t| matches!(t.type_identifier, TypeIdentifier::EquivalenceHashMinimal(_)))
        );
        assert_eq!(reply.complete_to_minimal.len(), 1);
        let pair = &reply.complete_to_minimal[0];
        assert_eq!(
            pair.type_identifier1,
            TypeIdentifier::EquivalenceHashComplete(hash)
        );
        let TypeIdentifier::EquivalenceHashMinimal(minimal) = pair.type_identifier2 else {
            panic!("the second identifier names the minimal form");
        };
        assert!(server.registry.get_minimal(&minimal).is_some());
        assert_ne!(minimal, hash);
    }

    /// A requested aggregate brings the objects it names by hash: RTI
    /// converts a reply as it arrives and fails on a member type it
    /// does not hold yet.
    #[test]
    fn handle_get_types_carries_the_closure_of_what_was_asked() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        let mut server = TypeLookupServer::new();
        let inner = zerodds_types::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::tl::Inner")
                .extensibility(Extensibility::Final)
                .member(
                    "x",
                    TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32),
                    |m| m,
                )
                .build_complete(),
        );
        let inner_hash = zerodds_types::compute_complete_hash(&inner).expect("hash");
        let outer = zerodds_types::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::tl::Outer")
                .extensibility(Extensibility::Appendable)
                .member(
                    "inner",
                    TypeIdentifier::EquivalenceHashComplete(inner_hash),
                    |m| m,
                )
                .build_complete(),
        );
        let outer_hash = zerodds_types::compute_complete_hash(&outer).expect("hash");
        server.registry.insert_complete(inner_hash, inner);
        server.registry.insert_complete(outer_hash, outer);
        server.registry.project_completes_to_minimal();
        let reply = server.handle_get_types(&GetTypesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashComplete(outer_hash)],
        });
        let ids: Vec<&TypeIdentifier> = reply.types.iter().map(|t| &t.type_identifier).collect();
        assert!(ids.contains(&&TypeIdentifier::EquivalenceHashComplete(outer_hash)));
        assert!(
            ids.contains(&&TypeIdentifier::EquivalenceHashComplete(inner_hash)),
            "the member type's complete object rides along: {ids:?}"
        );
        // The dependency precedes the aggregate that names it.
        let pos = |id: &TypeIdentifier| ids.iter().position(|i| *i == id).expect("present");
        assert!(
            pos(&TypeIdentifier::EquivalenceHashComplete(inner_hash))
                < pos(&TypeIdentifier::EquivalenceHashComplete(outer_hash))
        );
        assert_eq!(
            reply.complete_to_minimal.len(),
            2,
            "one pair per complete object answered"
        );
        // Asked twice, carried once.
        assert_eq!(
            ids.iter()
                .filter(|i| ***i == TypeIdentifier::EquivalenceHashComplete(inner_hash))
                .count(),
            1
        );
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

    /// A dependency the reply names carries the same
    /// `typeobject_serialized_size` the SEDP announcement would carry for it:
    /// the length of the DHEADER-delimited stream its EquivalenceHash is
    /// taken over (XTypes 1.3 §7.6.3.2.1 and the Annex-B `EquivalenceHash`
    /// comment).
    ///
    /// That stream is now what `TypeObject::to_bytes_le` writes, so the
    /// two lengths are equal rather than four apart — this used to
    /// assert the difference, which was `encode_into` writing no DHEADER
    /// and `hash` adding one. The announcement is unchanged either way;
    /// what moved is where the header is written.
    #[test]
    fn a_dependency_is_announced_at_its_hashed_length() {
        let mut server = TypeLookupServer::new();
        let dep = sample_struct("::Dep");
        let dep_hash = zerodds_types::compute_minimal_hash(&dep).unwrap();
        let dep_len = zerodds_types::hash::minimal_hash_input(&dep).unwrap().len();
        let serialized = zerodds_types::TypeObject::Minimal(dep.clone())
            .to_bytes_le()
            .unwrap()
            .len();
        server.registry.insert_minimal(dep_hash, dep);

        let root = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Root")
                .member("d", TypeIdentifier::EquivalenceHashMinimal(dep_hash), |m| m)
                .build_minimal(),
        );
        let root_hash = zerodds_types::compute_minimal_hash(&root).unwrap();
        server.registry.insert_minimal(root_hash, root);

        let reply = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
            continuation_point: ContinuationPoint::default(),
        });
        let Some(entry) = reply
            .dependent_typeids
            .iter()
            .find(|d| d.type_id == TypeIdentifier::EquivalenceHashMinimal(dep_hash))
        else {
            panic!("the registered dependency is missing from the reply")
        };
        let announced = entry.typeobject_serialized_size as usize;

        assert_eq!(announced, dep_len);
        assert_eq!(dep_len, serialized);
        // And the stream really does open with the DHEADER the length
        // counts, so the equality above is not two numbers that are both
        // four short.
        let bytes = zerodds_types::hash::minimal_hash_input(
            server.registry.get_minimal(&dep_hash).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize,
            bytes.len() - 4
        );
    }

    #[test]
    fn page_size_zero_normalizes_to_one() {
        let server = TypeLookupServer::new().with_page_size(0);
        assert_eq!(server.page_size(), 1);
    }
}
