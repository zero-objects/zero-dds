// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The XTypes description of the topic types of the Enhanced Service Mapping
//! (DDS-RPC 1.0 §7.5.1.2): the `CompleteTypeObject` of `${I}_Request` and
//! `${I}_Reply` and of every type they name.
//!
//! The structures are those of [`interface::enhanced_service`]: `${I}_Request`
//! and `${I}_Reply` are `@Choice @Autoid` — a mutable structure whose
//! members are all optional, with the HASH of the member name as member id
//! (§7.5.1.2.1) — and `In`, `Out` and `Result` are structures of the member
//! kinds [`EnhancedMemberType`] lists. A declared exception is the
//! structure of its members, named by its scoped name.
//!
//! With these objects an endpoint announces a `TypeIdentifier` and a peer
//! matches by assignability (XTypes 1.3 §7.6.3.3) and not by the type name,
//! which is what the evolution rules of §7.7.2.5 and the topic aliases of
//! §7.4.3 rely on.

use std::collections::BTreeSet;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::from_ast::{
    Form, complete_type_object, dependency_type_objects, type_identifier,
};
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_DEPTH, TypeRegistry, collect_referenced_hashes};
use zerodds_types::type_information::TypeInformation;
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, PrimitiveKind, TypeIdentifier, TypeObject, compute_hash,
};

use crate::adapter::interface::{
    self, EnhancedMemberType, EnhancedService, EnhancedStruct, Raise, Service,
};

/// `dds::rpc::RemoteExceptionCode_t` (§7.5.1.1.1).
const REMOTE_EXCEPTION_NAME: &str = "dds::rpc::RemoteExceptionCode_t";
const REMOTE_EXCEPTION_LITERALS: [&str; 6] = [
    "REMOTE_EX_OK",
    "REMOTE_EX_UNSUPPORTED",
    "REMOTE_EX_INVALID_ARGUMENT",
    "REMOTE_EX_OUT_OF_RESOURCES",
    "REMOTE_EX_UNKNOWN_OPERATION",
    "REMOTE_EX_UNKNOWN_EXCEPTION",
];

/// The objects of one topic type, in the form the runtime reads
/// (`TypeObject::to_bytes_le`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicTypeObjects {
    /// The object of the topic type.
    pub object: Vec<u8>,
    /// Every object it names, directly or through another.
    pub dependencies: Vec<Vec<u8>>,
    /// The `PID_TYPE_INFORMATION` payload of the topic type (XTypes 1.3
    /// §7.6.3.2.1, XCDR2 little-endian): `None` when a dependency has no
    /// minimal form.
    pub information: Option<Vec<u8>>,
    /// For the root (index 0) and each dependency (index 1 and on): the
    /// indexes of the objects it names directly.
    pub references: Vec<Vec<usize>>,
    /// The MINIMAL form (XTypes 1.3 §7.3.4.6) of the root and of each
    /// dependency, once each: `information` announces their identifiers,
    /// and a participant answers a TypeLookup request for any identifier it
    /// announced (§7.6.3.3.4). Empty when an object has no minimal form.
    pub minimal: Vec<Vec<u8>>,
    /// For each object of `minimal`: the indexes of the objects it names
    /// directly, counted over the root, the dependencies and then `minimal`.
    pub minimal_references: Vec<Vec<usize>>,
}

/// The objects of the two topic types of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTypeObjects {
    /// `${I}_Request`.
    pub request: TopicTypeObjects,
    /// `${I}_Reply`.
    pub reply: TopicTypeObjects,
}

/// Collects objects once each, by their equivalence hash.
#[derive(Default)]
struct Collected {
    seen: BTreeSet<[u8; 14]>,
    objects: Vec<TypeObject>,
}

impl Collected {
    fn add(&mut self, object: TypeObject) -> Result<(), String> {
        let hash = compute_hash(&object).map_err(|e| format!("hash: {e:?}"))?;
        if self.seen.insert(hash.0) {
            self.objects.push(object);
        }
        Ok(())
    }
}

fn scoped(ast: &Ast, module: Id<zerodds_ast::ModuleNode>, name: &str) -> Result<String, String> {
    let mut parts: Vec<String> = interface::module_chain(ast, module)
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    parts.push(name.to_string());
    Ok(parts.join("::"))
}

fn complete(object: CompleteTypeObject) -> TypeObject {
    TypeObject::Complete(object)
}

fn hash_identifier(object: &TypeObject) -> Result<TypeIdentifier, String> {
    let hash: EquivalenceHash = compute_hash(object).map_err(|e| format!("hash: {e:?}"))?;
    Ok(TypeIdentifier::EquivalenceHashComplete(hash))
}

/// The members of the declared exception `raise`, as a structure of its own.
fn exception_object(
    ast: &Ast,
    raise: &Raise,
    collected: &mut Collected,
) -> Result<TypeIdentifier, String> {
    let node = ast
        .exception(raise.id)
        .map_err(|e| format!("exception `{}`: {e:?}", raise.name))?;
    let mut builder = TypeObjectBuilder::struct_type(raise.scoped_name.clone())
        .extensibility(Extensibility::Final);
    for member_id in &node.members {
        let member = ast
            .member(*member_id)
            .map_err(|e| format!("exception `{}`: {e:?}", raise.name))?;
        let type_id = member.type_.resolved().ok_or_else(|| {
            format!(
                "exception `{}`: `{}` is unresolved",
                raise.name, member.name
            )
        })?;
        let identifier = idl_type(ast, type_id, collected)?;
        builder = builder.member(member.name.clone(), identifier, |m| m);
    }
    let object = complete(CompleteTypeObject::Struct(builder.build_complete()));
    let identifier = hash_identifier(&object)?;
    collected.add(object)?;
    Ok(identifier)
}

/// The identifier of an IDL type, its object and the objects it names added.
fn idl_type(
    ast: &Ast,
    id: Id<TypeNode>,
    collected: &mut Collected,
) -> Result<TypeIdentifier, String> {
    let identifier = type_identifier(ast, id, Form::Complete).map_err(|e| format!("{e:?}"))?;
    // A fully descriptive identifier (a primitive, a string, a sequence)
    // has no object of its own; its dependencies are still collected.
    if let Ok(object) = complete_type_object(ast, id) {
        collected.add(complete(object_of(object)))?;
    }
    for dependency in dependency_type_objects(ast, id).map_err(|e| format!("{e:?}"))? {
        collected.add(complete(object_of(dependency)))?;
    }
    Ok(identifier)
}

fn object_of(object: CompleteTypeObject) -> CompleteTypeObject {
    object
}

/// One structure of the mapping, built after the structures it names.
fn enhanced_struct(
    ast: &Ast,
    module: Id<zerodds_ast::ModuleNode>,
    spec: &EnhancedStruct,
    built: &std::collections::BTreeMap<String, TypeIdentifier>,
    collected: &mut Collected,
) -> Result<TypeObject, String> {
    let name = scoped(ast, module, &spec.name)?;
    let extensibility = if spec.choice || spec.mutable {
        Extensibility::Mutable
    } else {
        Extensibility::Final
    };
    let mut builder = TypeObjectBuilder::struct_type(name).extensibility(extensibility);
    for member in &spec.members {
        let identifier = match &member.type_ {
            EnhancedMemberType::Idl(id) => idl_type(ast, *id, collected)?,
            EnhancedMemberType::Dummy => TypeIdentifier::Primitive(PrimitiveKind::Byte),
            EnhancedMemberType::Struct(name) => built
                .get(name)
                .cloned()
                .ok_or_else(|| format!("structure `{name}` is used before it is built"))?,
            EnhancedMemberType::Exception(raise) => exception_object(ast, raise, collected)?,
            EnhancedMemberType::RemoteException => {
                let mut en = TypeObjectBuilder::enum_type(REMOTE_EXCEPTION_NAME);
                for (value, literal) in REMOTE_EXCEPTION_LITERALS.iter().enumerate() {
                    en = en.literal(*literal, value as i32);
                }
                let object = complete(CompleteTypeObject::Enumerated(en.build_complete()));
                let identifier = hash_identifier(&object)?;
                collected.add(object)?;
                identifier
            }
        };
        let member_id = member.member_id;
        let choice = spec.choice;
        builder = builder.member(member.name.clone(), identifier, move |m| {
            let m = m.id(member_id);
            if choice { m.optional() } else { m }
        });
    }
    Ok(complete(CompleteTypeObject::Struct(
        builder.build_complete(),
    )))
}

/// The objects of `${I}_Request` and `${I}_Reply` of `service`.
///
/// # Errors
/// A description of the type that has no `TypeObject`: a parameter type
/// XTypes 1.3 §7.3 has no `TypeIdentifier` for, or one in a recursion.
pub fn service_type_objects(
    ast: &Ast,
    service: &Service,
    model: &EnhancedService,
) -> Result<ServiceTypeObjects, String> {
    let mut collected = Collected::default();
    let mut built: std::collections::BTreeMap<String, TypeIdentifier> =
        std::collections::BTreeMap::new();
    for operation in &model.operations {
        // `Result` names `Out`, so `Out` comes first.
        for spec in [
            &operation.in_struct,
            &operation.out_struct,
            &operation.result_struct,
        ] {
            let object = enhanced_struct(ast, operation.module, spec, &built, &mut collected)?;
            built.insert(spec.name.clone(), hash_identifier(&object)?);
            collected.add(object)?;
        }
    }
    let request = enhanced_struct(ast, service.module, &model.request, &built, &mut collected)?;
    let reply = enhanced_struct(ast, service.module, &model.reply, &built, &mut collected)?;
    let bytes = |object: &TypeObject| object.to_bytes_le().map_err(|e| format!("encode: {e:?}"));
    let topic = |root: &TypeObject| -> Result<TopicTypeObjects, String> {
        let own = compute_hash(root).map_err(|e| format!("hash: {e:?}"))?.0;
        let mut ordered: Vec<&TypeObject> = vec![root];
        for object in &collected.objects {
            if compute_hash(object).map_err(|e| format!("hash: {e:?}"))?.0 != own {
                ordered.push(object);
            }
        }
        let mut hashes: Vec<[u8; 14]> = Vec::new();
        for object in &ordered {
            hashes.push(compute_hash(object).map_err(|e| format!("hash: {e:?}"))?.0);
        }
        let mut references = Vec::new();
        for object in &ordered {
            let found = collect_referenced_hashes(object, DEFAULT_MAX_RESOLVE_DEPTH)
                .map_err(|e| format!("references: {e:?}"))?;
            let mut indexes: Vec<usize> = Vec::new();
            for hash in found {
                if let Some(at) = hashes.iter().position(|h| *h == hash.0) {
                    if !indexes.contains(&at) {
                        indexes.push(at);
                    }
                }
            }
            references.push(indexes);
        }
        let mut registry = TypeRegistry::new();
        for object in &ordered {
            if let TypeObject::Complete(complete) = object {
                let hash = compute_hash(object).map_err(|e| format!("hash: {e:?}"))?;
                registry.insert_complete(hash, complete.clone());
            }
        }
        let information = match root {
            TypeObject::Complete(complete) => TypeInformation::from_registry(complete, &registry)
                .ok()
                .and_then(|info| info.to_bytes_le().ok()),
            _ => None,
        };
        let mut dependencies = Vec::new();
        for object in ordered.iter().skip(1) {
            dependencies.push(bytes(object)?);
        }
        // The minimal objects name minimal hashes only, so their references
        // are looked up among them, past the complete objects.
        let mut minimal_objects: Vec<TypeObject> = Vec::new();
        let mut minimal_hashes: Vec<[u8; 14]> = Vec::new();
        for object in &ordered {
            let TypeObject::Complete(complete) = object else {
                continue;
            };
            let Ok(projected) = complete.to_minimal(&registry) else {
                minimal_objects.clear();
                minimal_hashes.clear();
                break;
            };
            let projected = TypeObject::Minimal(projected);
            let hash = compute_hash(&projected)
                .map_err(|e| format!("hash: {e:?}"))?
                .0;
            if !minimal_hashes.contains(&hash) {
                minimal_hashes.push(hash);
                minimal_objects.push(projected);
            }
        }
        let mut minimal = Vec::new();
        let mut minimal_references = Vec::new();
        for object in &minimal_objects {
            minimal.push(bytes(object)?);
            let found = collect_referenced_hashes(object, DEFAULT_MAX_RESOLVE_DEPTH)
                .map_err(|e| format!("references: {e:?}"))?;
            let mut indexes: Vec<usize> = Vec::new();
            for hash in found {
                if let Some(at) = minimal_hashes.iter().position(|h| *h == hash.0) {
                    let at = ordered.len() + at;
                    if !indexes.contains(&at) {
                        indexes.push(at);
                    }
                }
            }
            minimal_references.push(indexes);
        }
        Ok(TopicTypeObjects {
            object: bytes(root)?,
            dependencies,
            information,
            references,
            minimal,
            minimal_references,
        })
    };
    Ok(ServiceTypeObjects {
        request: topic(&request)?,
        reply: topic(&reply)?,
    })
}

/// Standard Base64 (RFC 4648, with padding).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(char::from(ALPHABET[(n >> 18) as usize & 63]));
        out.push(char::from(ALPHABET[(n >> 12) as usize & 63]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[(n >> 6) as usize & 63])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[n as usize & 63])
        } else {
            '='
        });
    }
    out
}

/// The longest string constant a Java class file holds is 65535 octets.
const JAVA_CONSTANT_LIMIT: usize = 60_000;

/// One topic type as the Java runtime's `TypeDescription.of(information,
/// objects, references)` takes it: Base64 text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaDescription {
    /// The `PID_TYPE_INFORMATION` payload.
    pub information: String,
    /// The objects, separated by `;`.
    pub objects: String,
    /// The indexes each object names, `,` within and `;` between the lists.
    pub references: String,
}

impl JavaDescription {
    fn of(topic: &TopicTypeObjects) -> Option<Self> {
        let information = base64(topic.information.as_ref()?);
        // The minimal objects follow the complete ones, so the runtime
        // serves the identifiers of both forms the information announces.
        let objects = std::iter::once(&topic.object)
            .chain(topic.dependencies.iter())
            .chain(topic.minimal.iter())
            .map(|bytes| base64(bytes))
            .collect::<Vec<_>>()
            .join(";");
        let references = topic
            .references
            .iter()
            .chain(topic.minimal_references.iter())
            .map(|list| {
                list.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join(";");
        (information.len() <= JAVA_CONSTANT_LIMIT
            && objects.len() <= JAVA_CONSTANT_LIMIT
            && references.len() <= JAVA_CONSTANT_LIMIT)
            .then_some(Self {
                information,
                objects,
                references,
            })
    }
}

/// The request and the reply description for the Java runtime, or `None`
/// when either has no `PID_TYPE_INFORMATION` payload or does not fit a
/// string constant (the topics are then matched by type name).
#[must_use]
pub fn java_descriptions(
    objects: &ServiceTypeObjects,
) -> Option<(JavaDescription, JavaDescription)> {
    Some((
        JavaDescription::of(&objects.request)?,
        JavaDescription::of(&objects.reply)?,
    ))
}
