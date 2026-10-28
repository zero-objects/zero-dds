// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeInformation (XTypes 1.3 §7.6.3.2.2) — wrapper for the
//! discovery of strongly-hashed TypeObjects incl. transitive
//! dependencies.
//!
//! Wire-Format — the extensibility of each aggregate is part of it,
//! because it decides which headers stand in front of the fields
//! (§7.6.3.2.1):
//!
//! ```text
//! @extensibility(APPENDABLE) @nested
//! struct TypeIdentifierWithSize {              // DHEADER + body
//!     TypeIdentifier type_id;
//!     uint32         typeobject_serialized_size;
//! };
//!
//! @extensibility(APPENDABLE) @nested
//! struct TypeIdentifierWithDependencies {      // DHEADER + body
//!     TypeIdentifierWithSize       typeid_with_size;
//!     int32                        dependent_typeid_count; // -1 = unknown
//!     sequence<TypeIdentifierWithSize> dependent_typeids;  // DHEADER + count
//! };
//!
//! @extensibility(MUTABLE)
//! struct TypeInformation {                     // DHEADER + EMHEADER per member
//!     @id(0x1001) TypeIdentifierWithDependencies minimal;
//!     @id(0x1002) TypeIdentifierWithDependencies complete;
//! };
//! ```
//!
//! This is transmitted as the payload of `PID_TYPE_INFORMATION` (0x0075) in SEDP
//! publication/subscription announcements (T8).
//!
//! The dependencies describe transitively needed TypeObjects, which
//! the receiver can fetch via the TypeLookup service (T11..T15),
//! as long as it only knows the TypeIdentifier.

use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::framing::encode_mutable_member;
use crate::hash::{
    complete_serialized_size, compute_complete_hash, compute_minimal_hash, minimal_serialized_size,
};
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{decode_seq_appendable, encode_seq_appendable};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject};

/// The `@id` XTypes 1.3 §7.6.3.2.1 gives `TypeInformation::minimal`.
pub const MEMBER_ID_MINIMAL: u32 = 0x1001;

/// The `@id` XTypes 1.3 §7.6.3.2.1 gives `TypeInformation::complete`.
pub const MEMBER_ID_COMPLETE: u32 = 0x1002;

/// A `TypeIdentifierWithDependencies` that names no type: `TK_NONE`, no
/// object behind it, no dependencies. This is what an absent member of a
/// `TypeInformation` means (§7.6.3.2.1).
fn empty_dependencies() -> TypeIdentifierWithDependencies {
    TypeIdentifierWithDependencies::without_dependencies(TypeIdentifierWithSize {
        type_id: TypeIdentifier::None,
        typeobject_serialized_size: 0,
    })
}

/// TypeIdentifier + size of the serialized TypeObject (§7.6.3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithSize {
    /// Type reference (direct or strongly-hashed).
    pub type_id: TypeIdentifier,
    /// Size of the serialized TypeObject in bytes — **the DHEADER
    /// counts**.
    ///
    /// XTypes 1.3 §7.6.3.2.1 asks for "the serialized size of the
    /// associated TypeObject" and the Annex-B comment on `EquivalenceHash`
    /// builds `type_id` from the "first 14 bytes of MD5 of the serialized
    /// TypeObject using XCDR version 2 with Little Endian encoding". The two
    /// sentences name one serialization, so a peer that has the TypeObject
    /// under this hash and counts its octets must arrive at this number.
    /// Annex B declares `union TypeObject` `@extensibility(APPENDABLE)`:
    /// that serialization opens with a DHEADER. `TypeObject::to_bytes_le`
    /// includes that header, so its full length is the advertised size.
    ///
    /// Measured for `@extensibility(FINAL) struct Robot { uint32 id; uint32
    /// label; }`, whose MINIMAL and COMPLETE TypeObjects ZeroDDS builds
    /// byte-identically to the three: Cyclone DDS 11.0.1, Fast DDS 3.6.1 and
    /// OpenDDS 3.34.0 all announce 55 and 84, and RTI Connext 7.7.0 counts 84
    /// for the COMPLETE one. The serialization begins `33 00 00 00 f1` and
    /// `50 00 00 00 f2` — DHEADER 51 resp. 80, then the EquivalenceKind.
    ///
    /// Zero if `type_id` is a directly-identified primitive
    /// (no TypeObject needs to exist) or the size is unknown.
    pub typeobject_serialized_size: u32,
}

impl TypeIdentifierWithSize {
    /// Encode.
    ///
    /// XTypes 1.3 §7.6.3.2.1 declares `TypeIdentifierWithSize`
    /// `@extensibility(APPENDABLE) @nested`, so it goes on the wire as a
    /// `DHEADER` and a body — everywhere it appears, inside
    /// `PID_TYPE_INFORMATION` and inside a TypeLookup
    /// `getTypeDependencies` reply alike. A body written without the
    /// header parses as a `TypeIdentifier` whose discriminator is the
    /// first length byte, which is why Cyclone answers such a parameter
    /// with `DDS_RETCODE_BAD_PARAMETER` and drops the whole announcement.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.type_id.encode_into(w)?;
            w.write_u32(self.typeobject_serialized_size);
            Ok(())
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let type_id = TypeIdentifier::decode_from(r)?;
            let typeobject_serialized_size = r.read_u32()?;
            Ok(Self {
                type_id,
                typeobject_serialized_size,
            })
        })?)
    }
}

/// TypeIdentifier + dependencies (§7.6.3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithDependencies {
    /// Main type.
    pub typeid_with_size: TypeIdentifierWithSize,
    /// Number of transitive dependencies (can be `-1` = "unknown").
    ///
    /// When `dependent_typeids.len() == dependent_typeid_count`, the
    /// list is complete. With `-1` the receiver can fetch more via
    /// TypeLookup `getTypeDependencies`.
    pub dependent_typeid_count: i32,
    /// Transitive dependencies (only those the sender knows).
    pub dependent_typeids: Vec<TypeIdentifierWithSize>,
}

impl TypeIdentifierWithDependencies {
    /// Short constructor for types without known dependencies.
    #[must_use]
    pub fn without_dependencies(typeid_with_size: TypeIdentifierWithSize) -> Self {
        Self {
            typeid_with_size,
            dependent_typeid_count: 0,
            dependent_typeids: Vec::new(),
        }
    }

    /// Encode.
    ///
    /// XTypes 1.3 §7.6.3.2.1 declares `TypeIdentifierWithDependencies`
    /// `@extensibility(APPENDABLE) @nested`: a `DHEADER` and a body. The
    /// dependency list inside it is a `sequence` whose element is itself
    /// an aggregate, so XCDR2 §7.4.3.5.3 gives the sequence a `DHEADER`
    /// of its own before the element count — which is why
    /// `encode_seq_appendable` and not the bare length is written here.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.typeid_with_size.encode_into(w)?;
            w.write_u32(self.dependent_typeid_count as u32);
            encode_seq_appendable(w, &self.dependent_typeids, |w, t| t.encode_into(w))
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or a discriminator
    /// [`TypeIdentifier`] does not define.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let at = r.position();
            let typeid_with_size =
                TypeIdentifierWithSize::decode_from(r).map_err(|e| e.into_wire(at))?;
            let dependent_typeid_count = r.read_u32()? as i32;
            let dependent_typeids = decode_seq_appendable(r, |rr| {
                let at = rr.position();
                TypeIdentifierWithSize::decode_from(rr).map_err(|e| e.into_wire(at))
            })?;
            Ok(Self {
                typeid_with_size,
                dependent_typeid_count,
                dependent_typeids,
            })
        })?)
    }
}

/// TypeInformation (§7.6.3.2.2) — tuples `minimal` and `complete`
/// TypeIdentifier references. Transmitted as the payload of `PID_TYPE_INFORMATION`
/// (0x0075) in SEDP announcements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInformation {
    /// Minimal TypeIdentifier with dependencies.
    pub minimal: TypeIdentifierWithDependencies,
    /// Complete TypeIdentifier with dependencies.
    pub complete: TypeIdentifierWithDependencies,
}

impl TypeInformation {
    /// Builds TypeInformation from a minimal/complete pair, without
    /// transitive dependencies. Useful when the type is self-contained
    /// (all members are primitives / plain collections).
    ///
    /// # Errors
    /// `WireError` when serializing for size/hash.
    pub fn from_minimal_and_complete(
        minimal: &MinimalTypeObject,
        complete: &CompleteTypeObject,
    ) -> Result<Self, WireError> {
        let min_hash = compute_minimal_hash(minimal)?;
        let com_hash = compute_complete_hash(complete)?;
        // The size is the length of the same serialization the hash is taken
        // over — DHEADER included, see `hash::minimal_serialized_size`.
        let min_size = minimal_serialized_size(minimal)?;
        let com_size = complete_serialized_size(complete)?;
        Ok(Self {
            minimal: TypeIdentifierWithDependencies::without_dependencies(TypeIdentifierWithSize {
                type_id: TypeIdentifier::EquivalenceHashMinimal(min_hash),
                typeobject_serialized_size: min_size,
            }),
            complete: TypeIdentifierWithDependencies::without_dependencies(
                TypeIdentifierWithSize {
                    type_id: TypeIdentifier::EquivalenceHashComplete(com_hash),
                    typeobject_serialized_size: com_size,
                },
            ),
        })
    }

    /// Builds TypeInformation from a complete `TypeObject` and the
    /// registry that holds the types it depends on, with the dependencies
    /// the clause describes.
    ///
    /// DDS-XTypes 1.3 §7.6.3.2.1: "The field minimal.dependent_typeid_count
    /// shall contain the total number of other MINIMAL Hash TypeIdentifiers
    /// that correspond to data-types the TopicType depends on. This field
    /// may be set to -1 to indicate it is not being provided. The field
    /// minimal.dependent_typeids may contain some of the MINIMAL Hash
    /// TypeIdentifiers of the types the TopicType depends on, along with
    /// the serialized size of the respective TypeObjects", and the same
    /// for the complete side. The dependencies are followed transitively
    /// through the registry; each distinct type is listed once, in the
    /// order it is met.
    ///
    /// The count is `-1` — and the list holds only what was found — when
    /// the registry lacks a dependency, when a dependency does not project
    /// to its minimal form, or when the type reaches a strongly connected
    /// component: the number would otherwise claim a total it does not
    /// know.
    ///
    /// # Errors
    /// `WireError` when serializing for size or hash, or when the object
    /// itself does not project to its minimal form.
    pub fn from_registry(
        complete: &CompleteTypeObject,
        registry: &crate::resolve::TypeRegistry,
    ) -> Result<Self, WireError> {
        use crate::resolve::{DEFAULT_MAX_RESOLVE_DEPTH, collect_referenced_hashes};
        use crate::type_identifier::EquivalenceHash;
        use crate::type_object::TypeObject;
        use alloc::collections::BTreeSet;

        let minimal = complete
            .to_minimal(registry)
            .map_err(|_| WireError::OutOfRange {
                at: 0,
                expected: alloc::string::String::from(
                    "a complete TypeObject whose dependencies project to their minimal form",
                ),
            })?;
        let mut information = Self::from_minimal_and_complete(&minimal, complete)?;
        let own = compute_complete_hash(complete)?;

        let mut provided = true;
        let mut pending: Vec<EquivalenceHash> = match collect_referenced_hashes(
            &TypeObject::Complete(complete.clone()),
            DEFAULT_MAX_RESOLVE_DEPTH,
        ) {
            Ok(found) => found,
            Err(_) => {
                provided = false;
                Vec::new()
            }
        };
        pending.reverse();
        let mut met: BTreeSet<EquivalenceHash> = BTreeSet::new();
        let mut minimal_met: BTreeSet<EquivalenceHash> = BTreeSet::new();
        let mut minimal_deps: Vec<TypeIdentifierWithSize> = Vec::new();
        let mut complete_deps: Vec<TypeIdentifierWithSize> = Vec::new();
        while let Some(hash) = pending.pop() {
            if hash == own || !met.insert(hash) {
                continue;
            }
            let Some(object) = registry.get_complete(&hash) else {
                provided = false;
                continue;
            };
            let Ok(projected) = object.to_minimal(registry) else {
                provided = false;
                continue;
            };
            complete_deps.push(TypeIdentifierWithSize {
                type_id: TypeIdentifier::EquivalenceHashComplete(hash),
                typeobject_serialized_size: complete_serialized_size(object)?,
            });
            let minimal_hash = compute_minimal_hash(&projected)?;
            if minimal_met.insert(minimal_hash) {
                minimal_deps.push(TypeIdentifierWithSize {
                    type_id: TypeIdentifier::EquivalenceHashMinimal(minimal_hash),
                    typeobject_serialized_size: minimal_serialized_size(&projected)?,
                });
            }
            match collect_referenced_hashes(
                &TypeObject::Complete(object.clone()),
                DEFAULT_MAX_RESOLVE_DEPTH,
            ) {
                Ok(found) => pending.extend(found.into_iter().rev()),
                Err(_) => provided = false,
            }
        }

        let count = |list: &[TypeIdentifierWithSize]| {
            if provided {
                list.len().min(i32::MAX as usize) as i32
            } else {
                -1
            }
        };
        information.minimal.dependent_typeid_count = count(&minimal_deps);
        information.complete.dependent_typeid_count = count(&complete_deps);
        information.minimal.dependent_typeids = minimal_deps;
        information.complete.dependent_typeids = complete_deps;
        Ok(information)
    }

    /// Builds the TypeInformation of a topic type that belongs to a
    /// strongly connected component, from the registry that holds both
    /// forms of the component.
    ///
    /// DDS-XTypes 1.3 §7.6.3.2.1 puts the identifier of the TopicType in
    /// `typeid_with_size` of each side — here the type's own
    /// `TI_STRONGLY_CONNECTED_COMPONENT` identifier, `scc_index` naming it
    /// in the component, with the serialized size of its `TypeObject` —
    /// and says the list of dependent identifiers "shall not contain
    /// individual type identifiers for types belonging to Strongly
    /// Connected Component [...], instead it shall include the identifier
    /// of the whole Strongly-Connected Component (SCCIdentifier)". So the
    /// dependents are the `SCCIdentifier` (`scc_index = 0`) of the type's
    /// own component, which the receiver asks for to get every member, the
    /// SCCIdentifier of every other component the type reaches, and the
    /// hash identifiers of the other types, each once. The size of an
    /// `SCCIdentifier` is the sum of the sizes of its members' objects.
    ///
    /// `scc` is the COMPLETE identifier of the topic type; its minimal
    /// counterpart is the same position of the minimal component, which the
    /// registry filed when it built both.
    ///
    /// The counts are `-1`, and the lists hold what was found, when the
    /// registry lacks a dependency, when one does not project to its
    /// minimal form, or when an object has a shape this revision cannot
    /// enumerate.
    ///
    /// # Errors
    /// `WireError` when the component or its minimal form is not in the
    /// registry, when `scc` names a position outside it, or when
    /// serializing for size or hash fails.
    pub fn from_component(
        scc: &crate::type_identifier::StronglyConnectedComponentId,
        registry: &crate::resolve::TypeRegistry,
    ) -> Result<Self, WireError> {
        use crate::resolve::named_identifiers;
        use crate::type_identifier::{StronglyConnectedComponentId, kinds};
        use alloc::collections::BTreeSet;
        use alloc::string::String;

        let missing = |what: &str| WireError::OutOfRange {
            at: 0,
            expected: String::from(what),
        };
        let complete_members = registry
            .component_members(&scc.hash)
            .ok_or_else(|| missing("a strongly connected component the registry holds"))?;
        let minimal_hash = registry.minimal_component_hash(&scc.hash).ok_or_else(|| {
            missing("a strongly connected component the registry holds in its minimal form")
        })?;
        let minimal_members = registry
            .minimal_component_members(&minimal_hash)
            .ok_or_else(|| missing("the minimal members of the component"))?;
        let position = usize::try_from(scc.scc_index.saturating_sub(1))
            .ok()
            .filter(|position| {
                scc.scc_index >= 1
                    && *position < complete_members.len()
                    && complete_members.len() == minimal_members.len()
                    && i32::try_from(complete_members.len()).ok() == Some(scc.scc_length)
            })
            .ok_or_else(|| missing("an scc_index inside the component"))?;

        let component_id = |kind: u8, hash, index: i32| {
            TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
                equivalence_kind: kind,
                hash,
                scc_length: scc.scc_length,
                scc_index: index,
            })
        };
        let complete_size = |members: &[CompleteTypeObject]| -> Result<u32, WireError> {
            let mut total = 0_u32;
            for member in members {
                total = total.saturating_add(complete_serialized_size(member)?);
            }
            Ok(total)
        };
        let minimal_size = |members: &[MinimalTypeObject]| -> Result<u32, WireError> {
            let mut total = 0_u32;
            for member in members {
                total = total.saturating_add(minimal_serialized_size(member)?);
            }
            Ok(total)
        };

        let mut information = Self {
            minimal: TypeIdentifierWithDependencies::without_dependencies(TypeIdentifierWithSize {
                type_id: component_id(kinds::EK_MINIMAL, minimal_hash, scc.scc_index),
                typeobject_serialized_size: minimal_serialized_size(&minimal_members[position])?,
            }),
            complete: TypeIdentifierWithDependencies::without_dependencies(
                TypeIdentifierWithSize {
                    type_id: component_id(kinds::EK_COMPLETE, scc.hash, scc.scc_index),
                    typeobject_serialized_size: complete_serialized_size(
                        &complete_members[position],
                    )?,
                },
            ),
        };

        // The component's own SCCIdentifier first: it is how a receiver
        // gets every member of the component the topic type is in.
        let mut minimal_deps = alloc::vec![TypeIdentifierWithSize {
            type_id: component_id(kinds::EK_MINIMAL, minimal_hash, 0),
            typeobject_serialized_size: minimal_size(minimal_members)?,
        }];
        let mut complete_deps = alloc::vec![TypeIdentifierWithSize {
            type_id: component_id(kinds::EK_COMPLETE, scc.hash, 0),
            typeobject_serialized_size: complete_size(complete_members)?,
        }];

        let mut provided = true;
        let mut queue: Vec<&CompleteTypeObject> = complete_members.iter().collect();
        let mut met: BTreeSet<crate::type_identifier::EquivalenceHash> = BTreeSet::new();
        let mut met_components: BTreeSet<crate::type_identifier::EquivalenceHash> = BTreeSet::new();
        met_components.insert(scc.hash);
        let mut minimal_met: BTreeSet<crate::type_identifier::EquivalenceHash> = BTreeSet::new();
        while let Some(object) = queue.pop() {
            let Some(named) = named_identifiers(object) else {
                provided = false;
                continue;
            };
            for identifier in named {
                match identifier {
                    TypeIdentifier::EquivalenceHashComplete(hash) => {
                        if !met.insert(*hash) {
                            continue;
                        }
                        let Some(dependency) = registry.get_complete(hash) else {
                            provided = false;
                            continue;
                        };
                        let Ok(projected) = dependency.to_minimal(registry) else {
                            provided = false;
                            continue;
                        };
                        complete_deps.push(TypeIdentifierWithSize {
                            type_id: TypeIdentifier::EquivalenceHashComplete(*hash),
                            typeobject_serialized_size: complete_serialized_size(dependency)?,
                        });
                        let projected_hash = compute_minimal_hash(&projected)?;
                        if minimal_met.insert(projected_hash) {
                            minimal_deps.push(TypeIdentifierWithSize {
                                type_id: TypeIdentifier::EquivalenceHashMinimal(projected_hash),
                                typeobject_serialized_size: minimal_serialized_size(&projected)?,
                            });
                        }
                        queue.push(dependency);
                    }
                    TypeIdentifier::StronglyConnectedComponent(other) => {
                        if !met_components.insert(other.hash) {
                            continue;
                        }
                        let (Some(members), Some(other_minimal_hash)) = (
                            registry.component_members(&other.hash),
                            registry.minimal_component_hash(&other.hash),
                        ) else {
                            provided = false;
                            continue;
                        };
                        let Some(other_minimal) =
                            registry.minimal_component_members(&other_minimal_hash)
                        else {
                            provided = false;
                            continue;
                        };
                        let identifier_of = |kind: u8, hash| {
                            TypeIdentifier::StronglyConnectedComponent(
                                StronglyConnectedComponentId {
                                    equivalence_kind: kind,
                                    hash,
                                    scc_length: other.scc_length,
                                    scc_index: 0,
                                },
                            )
                        };
                        complete_deps.push(TypeIdentifierWithSize {
                            type_id: identifier_of(kinds::EK_COMPLETE, other.hash),
                            typeobject_serialized_size: complete_size(members)?,
                        });
                        minimal_deps.push(TypeIdentifierWithSize {
                            type_id: identifier_of(kinds::EK_MINIMAL, other_minimal_hash),
                            typeobject_serialized_size: minimal_size(other_minimal)?,
                        });
                        queue.extend(members.iter());
                    }
                    // A primitive, a string, `TK_NONE` (no base type), or a
                    // minimal hash inside a complete object: nothing to
                    // announce.
                    TypeIdentifier::None
                    | TypeIdentifier::Primitive(_)
                    | TypeIdentifier::String8Small { .. }
                    | TypeIdentifier::String8Large { .. }
                    | TypeIdentifier::String16Small { .. }
                    | TypeIdentifier::String16Large { .. }
                    | TypeIdentifier::EquivalenceHashMinimal(_) => {}
                    // A collection is followed to its leaves by
                    // `named_identifiers`, so what is left is an
                    // identifier this revision cannot enumerate.
                    _ => provided = false,
                }
            }
        }

        let count = |list: &[TypeIdentifierWithSize]| {
            if provided {
                list.len().min(i32::MAX as usize) as i32
            } else {
                -1
            }
        };
        information.minimal.dependent_typeid_count = count(&minimal_deps);
        information.complete.dependent_typeid_count = count(&complete_deps);
        information.minimal.dependent_typeids = minimal_deps;
        information.complete.dependent_typeids = complete_deps;
        Ok(information)
    }

    /// Adds a transitive dependency to both sides.
    pub fn add_dependency(
        &mut self,
        minimal_dep: TypeIdentifierWithSize,
        complete_dep: TypeIdentifierWithSize,
    ) {
        self.minimal.dependent_typeids.push(minimal_dep);
        self.minimal.dependent_typeid_count =
            self.minimal.dependent_typeids.len().min(i32::MAX as usize) as i32;
        self.complete.dependent_typeids.push(complete_dep);
        self.complete.dependent_typeid_count =
            self.complete.dependent_typeids.len().min(i32::MAX as usize) as i32;
    }

    /// Encode as XCDR2 bytes (without the encapsulation header — that is
    /// provided by the PID layer).
    ///
    /// XTypes 1.3 §7.6.3.2.1 declares `TypeInformation`
    /// `@extensibility(MUTABLE)`, so the wire form is PL_CDR2: a `DHEADER`
    /// over the member list, and inside it each member behind an
    /// `EMHEADER`. The two members carry the explicit ids the clause
    /// assigns them, [`MEMBER_ID_MINIMAL`] and [`MEMBER_ID_COMPLETE`], and
    /// not their ordinal position — a mutable member is addressed by id,
    /// and a peer reading them positionally would agree with this encoder
    /// only by accident.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            encode_mutable_member(w, MEMBER_ID_MINIMAL, |w| self.minimal.encode_into(w))?;
            encode_mutable_member(w, MEMBER_ID_COMPLETE, |w| self.complete.encode_into(w))
        })
    }

    /// Decode.
    ///
    /// A member id this clause does not define is skipped rather than
    /// refused. `TypeInformation` has exactly the two members of
    /// §7.6.3.2.1 today; a future revision that adds a third would
    /// otherwise cost the whole type identity of every endpoint that
    /// sends it, and the two members this reader wants are still in front
    /// of it.
    ///
    /// # Errors
    /// The stream ends inside this value, a discriminator the
    /// specification does not define, or a member whose `EMHEADER` names a
    /// length that runs past the member list.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let mut minimal = None;
            let mut complete = None;
            // Four bytes is the shortest thing an EMHEADER can be followed
            // by; anything less is the padding that ends a member list.
            while r.remaining() >= 4 {
                // `read_emheader` honours all eight length codes of
                // §7.4.3.4.2 and reports the member's extent, so a member
                // this reader does not want can still be stepped over
                // rather than guessed at.
                let header = r.read_emheader()?;
                let at = r.position();
                let body = r.read_bytes(header.extent)?;
                let mut inner =
                    Reader::with_max_align(body, r.endianness(), crate::framing::XCDR2_MAX_ALIGN);
                match header.member_id {
                    MEMBER_ID_MINIMAL => {
                        minimal = Some(
                            TypeIdentifierWithDependencies::decode_from(&mut inner)
                                .map_err(|e| e.into_wire(at))?,
                        );
                    }
                    MEMBER_ID_COMPLETE => {
                        complete = Some(
                            TypeIdentifierWithDependencies::decode_from(&mut inner)
                                .map_err(|e| e.into_wire(at))?,
                        );
                    }
                    _ => {}
                }
            }
            // A member the sender left out is `TK_NONE` on both counts,
            // which is what §7.6.3.2.1 means by an absent equivalence
            // form — and what `preferred_type_identifier` reads as "this
            // side carries no identity".
            Ok(Self {
                minimal: minimal.unwrap_or_else(empty_dependencies),
                complete: complete.unwrap_or_else(empty_dependencies),
            })
        })?)
    }

    /// Convenience: LE-Bytes.
    ///
    /// # Errors
    /// Encode.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, WireError> {
        let mut w = crate::framing::xcdr2_writer();
        self.encode_into(&mut w)?;
        Ok(w.finish())
    }

    /// Convenience: from LE bytes.
    ///
    /// # Errors
    /// Decode / Unknown-TypeKind.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, TypeCodecError> {
        let mut r = crate::framing::xcdr2_reader(bytes);
        Self::decode_from(&mut r)
    }

    /// Decode from CDR bytes with an explicit endianness. Used by the SEDP
    /// decode path, where the `PID_TYPE_INFORMATION` blob follows the
    /// endianness of the enclosing PL_CDR message (not always little-endian).
    ///
    /// # Errors
    /// Decode / Unknown-TypeKind.
    pub fn from_bytes(bytes: &[u8], endianness: Endianness) -> Result<Self, TypeCodecError> {
        // The byte order is the caller's; the alignment cap is not. This is
        // the one decode in this crate that reads a foreign peer's octets,
        // and a `PID_TYPE_INFORMATION` blob is XCDR2 whichever way round it
        // is written (XTypes 1.3 §7.6.3.2.1).
        let mut r = zerodds_wire::Reader::with_max_align(
            bytes,
            endianness,
            crate::framing::XCDR2_MAX_ALIGN,
        );
        Self::decode_from(&mut r)
    }

    /// Returns the `TypeIdentifier` best suited for cross-vendor
    /// assignability matching (issue #30).
    ///
    /// A standard `PID_TYPE_INFORMATION` blob from a non-ZeroDDS peer
    /// (Cyclone, FastDDS, RTI, …) carries a minimal and a complete
    /// `TypeIdentifierWithSize`. For match-time assignability we prefer the
    /// **minimal** equivalence hash — it is the identity peers resolve
    /// structurally through the TypeLookup service (XTypes 1.3 §7.6.3.3.4) —
    /// and fall back to the **complete** identifier when the minimal side is
    /// absent (`TypeIdentifier::None`).
    ///
    /// Returns `None` only when neither side carries a usable identifier;
    /// the caller MUST treat that as "no type identity available" and must
    /// NOT silently fall back to a name-only match without signalling it.
    #[must_use]
    pub fn preferred_type_identifier(&self) -> Option<TypeIdentifier> {
        let minimal = &self.minimal.typeid_with_size.type_id;
        if *minimal != TypeIdentifier::None {
            return Some(minimal.clone());
        }
        let complete = &self.complete.typeid_with_size.type_id;
        if *complete != TypeIdentifier::None {
            return Some(complete.clone());
        }
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::builder::{Extensibility, TypeObjectBuilder};
    use crate::type_identifier::{EquivalenceHash, PrimitiveKind};

    fn sample_type_info() -> TypeInformation {
        let b = TypeObjectBuilder::struct_type("::chat::Chatter")
            .extensibility(Extensibility::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m);
        let minimal = MinimalTypeObject::Struct(b.build_minimal());
        let complete = CompleteTypeObject::Struct(b.build_complete());
        TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap()
    }

    /// The `PID_TYPE_INFORMATION` value of a real CycloneDDS 0.10.2 SEDP
    /// publication (the `KeyedSeq` type of `ddsperf`), taken from
    /// `crates/rtps/tests/fixtures/cyclone/sedp_publication.hex`.
    ///
    /// This is the one test in this module whose bytes were not produced
    /// by this crate. Everything else here is a round trip, and a round
    /// trip agrees with itself no matter which framing it uses — which is
    /// how a `TypeInformation` written without its `DHEADER` and
    /// `EMHEADER`s passed for correct while no peer had ever read one.
    const CYCLONE_KEYEDSEQ_TYPE_INFORMATION: &[u8] = &[
        0x60, 0x00, 0x00, 0x00, // DHEADER: 96 octets of member list
        0x01, 0x10, 0x00, 0x40, // EMHEADER: LC=4, member id 0x1001 (minimal)
        0x28, 0x00, 0x00, 0x00, // NEXTINT: 40 octets
        0x24, 0x00, 0x00, 0x00, // DHEADER TypeIdentifierWithDependencies
        0x14, 0x00, 0x00, 0x00, // DHEADER TypeIdentifierWithSize
        0xf1, 0xfa, 0x04, 0x13, 0x69, 0x3f, 0x17, 0x17, // EK_MINIMAL + hash
        0x16, 0x33, 0x96, 0x2d, 0xcd, 0x81, 0xa2, 0x00, // hash tail + pad
        0x4c, 0x00, 0x00, 0x00, // typeobject_serialized_size = 76
        0x00, 0x00, 0x00, 0x00, // dependent_typeid_count = 0
        0x04, 0x00, 0x00, 0x00, // DHEADER of the dependency sequence
        0x00, 0x00, 0x00, 0x00, // sequence length = 0
        0x02, 0x10, 0x00, 0x40, // EMHEADER: LC=4, member id 0x1002 (complete)
        0x28, 0x00, 0x00, 0x00, // NEXTINT: 40 octets
        0x24, 0x00, 0x00, 0x00, //
        0x14, 0x00, 0x00, 0x00, //
        0xf2, 0xc6, 0xe6, 0x28, 0x5a, 0x68, 0xc8, 0xf6, // EK_COMPLETE + hash
        0xcd, 0x7c, 0x42, 0x03, 0xc4, 0x6c, 0xb2, 0x00, //
        0x7a, 0x00, 0x00, 0x00, // typeobject_serialized_size = 122
        0x00, 0x00, 0x00, 0x00, //
        0x04, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, //
    ];

    #[test]
    fn a_cyclone_type_information_decodes_to_its_two_hashes() {
        let ti = TypeInformation::from_bytes_le(CYCLONE_KEYEDSEQ_TYPE_INFORMATION)
            .expect("a captured Cyclone PID_TYPE_INFORMATION value");
        assert_eq!(
            ti.minimal.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([
                0xfa, 0x04, 0x13, 0x69, 0x3f, 0x17, 0x17, 0x16, 0x33, 0x96, 0x2d, 0xcd, 0x81, 0xa2
            ]))
        );
        assert_eq!(ti.minimal.typeid_with_size.typeobject_serialized_size, 76);
        assert_eq!(
            ti.complete.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
                0xc6, 0xe6, 0x28, 0x5a, 0x68, 0xc8, 0xf6, 0xcd, 0x7c, 0x42, 0x03, 0xc4, 0x6c, 0xb2
            ]))
        );
        assert_eq!(ti.complete.typeid_with_size.typeobject_serialized_size, 122);
        assert_eq!(ti.minimal.dependent_typeid_count, 0);
        assert!(ti.minimal.dependent_typeids.is_empty());
    }

    /// What ZeroDDS puts in `PID_TYPE_INFORMATION` has to be the octets
    /// Cyclone puts there, not merely something Cyclone can be talked into
    /// accepting: `ddsi_plist.c` normalises the value against the XCDR2
    /// descriptor of `DDS_XTypes_TypeInformation` before it reads a field,
    /// and a value that does not normalise is `DDS_RETCODE_BAD_PARAMETER`
    /// — which discards the whole endpoint announcement, not just the
    /// parameter.
    #[test]
    fn re_encoding_a_cyclone_type_information_reproduces_its_octets() {
        let ti = TypeInformation::from_bytes_le(CYCLONE_KEYEDSEQ_TYPE_INFORMATION)
            .expect("a captured Cyclone PID_TYPE_INFORMATION value");
        assert_eq!(
            ti.to_bytes_le().expect("the encode"),
            CYCLONE_KEYEDSEQ_TYPE_INFORMATION
        );
    }

    #[test]
    fn typeinformation_roundtrips() {
        let ti = sample_type_info();
        let bytes = ti.to_bytes_le().unwrap();
        let decoded = TypeInformation::from_bytes_le(&bytes).unwrap();
        assert_eq!(ti, decoded);
    }

    #[test]
    fn typeinformation_minimal_has_minimal_discriminator() {
        let ti = sample_type_info();
        assert!(matches!(
            ti.minimal.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashMinimal(_)
        ));
        assert!(matches!(
            ti.complete.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashComplete(_)
        ));
    }

    /// The announced size is the length of the serialization the hash is
    /// taken over, DHEADER included — see the field's own documentation.
    /// This test used to compare against `TypeObject::to_bytes_le`, which
    /// omits the DHEADER; that is the four-octet shortfall RTI Connext 7.7.0
    /// rejects, and it is asserted against here rather than for.
    #[test]
    fn typeinformation_size_matches_actual_typeobject_bytes() {
        let b = TypeObjectBuilder::struct_type("::X").member(
            "a",
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            |m| m,
        );
        let minimal = MinimalTypeObject::Struct(b.build_minimal());
        let complete = CompleteTypeObject::Struct(b.build_complete());
        let hashed_min_size = crate::hash::minimal_hash_input(&minimal).unwrap().len();
        let hashed_com_size = crate::hash::complete_hash_input(&complete).unwrap().len();
        // The serialized object *is* the hash input — one encoder, one
        // DHEADER. This used to assert a difference of four octets,
        // which was `TypeObject::encode_into` writing no DHEADER and
        // `hash` adding one; announcing the shorter number is what RTI
        // Connext 7.7.0 refused at discovery.
        let serialized_min_size = crate::type_object::TypeObject::Minimal(minimal.clone())
            .to_bytes_le()
            .unwrap()
            .len();
        let serialized_com_size = crate::type_object::TypeObject::Complete(complete.clone())
            .to_bytes_le()
            .unwrap()
            .len();
        assert_eq!(hashed_min_size, serialized_min_size);
        assert_eq!(hashed_com_size, serialized_com_size);

        let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
        assert_eq!(
            ti.minimal.typeid_with_size.typeobject_serialized_size,
            hashed_min_size as u32
        );
        assert_eq!(
            ti.complete.typeid_with_size.typeobject_serialized_size,
            hashed_com_size as u32
        );
    }

    #[test]
    fn add_dependency_updates_count_and_list() {
        let mut ti = sample_type_info();
        assert_eq!(ti.minimal.dependent_typeid_count, 0);
        let dep_min = TypeIdentifierWithSize {
            type_id: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0xAA; 14])),
            typeobject_serialized_size: 42,
        };
        let dep_com = TypeIdentifierWithSize {
            type_id: TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0xBB; 14])),
            typeobject_serialized_size: 84,
        };
        ti.add_dependency(dep_min.clone(), dep_com.clone());
        assert_eq!(ti.minimal.dependent_typeid_count, 1);
        assert_eq!(ti.minimal.dependent_typeids[0], dep_min);
        assert_eq!(ti.complete.dependent_typeid_count, 1);
        assert_eq!(ti.complete.dependent_typeids[0], dep_com);

        // Roundtrip with dependencies too
        let bytes = ti.to_bytes_le().unwrap();
        assert_eq!(TypeInformation::from_bytes_le(&bytes).unwrap(), ti);
    }

    #[test]
    fn preferred_type_identifier_prefers_minimal() {
        // A standard PID_TYPE_INFORMATION blob (as a non-ZeroDDS peer sends
        // it) yields the minimal equivalence hash for match-time
        // assignability (issue #30).
        let ti = sample_type_info();
        let id = ti.preferred_type_identifier().expect("minimal id present");
        assert_eq!(id, ti.minimal.typeid_with_size.type_id);
        assert!(matches!(id, TypeIdentifier::EquivalenceHashMinimal(_)));
    }

    #[test]
    fn preferred_type_identifier_falls_back_to_complete() {
        let mut ti = sample_type_info();
        ti.minimal.typeid_with_size.type_id = TypeIdentifier::None;
        let id = ti
            .preferred_type_identifier()
            .expect("complete id present as fallback");
        assert_eq!(id, ti.complete.typeid_with_size.type_id);
        assert!(matches!(id, TypeIdentifier::EquivalenceHashComplete(_)));
    }

    #[test]
    fn preferred_type_identifier_none_when_both_absent() {
        let mut ti = sample_type_info();
        ti.minimal.typeid_with_size.type_id = TypeIdentifier::None;
        ti.complete.typeid_with_size.type_id = TypeIdentifier::None;
        assert_eq!(ti.preferred_type_identifier(), None);
    }

    #[test]
    fn from_bytes_matches_from_bytes_le_for_little_endian() {
        let ti = sample_type_info();
        let bytes = ti.to_bytes_le().unwrap();
        let a = TypeInformation::from_bytes_le(&bytes).unwrap();
        let b = TypeInformation::from_bytes(&bytes, zerodds_wire::Endianness::Little).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, ti);
    }

    #[test]
    fn negative_dependent_typeid_count_roundtrips() {
        // -1 = "unknown/incomplete" (spec-compliant).
        let mut ti = sample_type_info();
        ti.minimal.dependent_typeid_count = -1;
        ti.complete.dependent_typeid_count = -1;
        let bytes = ti.to_bytes_le().unwrap();
        let decoded = TypeInformation::from_bytes_le(&bytes).unwrap();
        assert_eq!(decoded.minimal.dependent_typeid_count, -1);
        assert_eq!(decoded.complete.dependent_typeid_count, -1);
    }
}
