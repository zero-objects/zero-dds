// SPDX-License-Identifier: Apache-2.0
//! Shared strict TypeInformation and TypeObject dependency closure.
use std::collections::BTreeSet;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;

pub(crate) type StandardMetadata = (Vec<u8>, Vec<zerodds_types::type_object::TypeObject>);

pub(crate) fn standard_metadata(ast: &Ast, id: Id<TypeNode>) -> Result<StandardMetadata, String> {
    use zerodds_types::resolve::{
        DEFAULT_MAX_RESOLVE_DEPTH, DEFAULT_MAX_RESOLVE_NODES, TypeRegistry,
        collect_referenced_hashes,
    };
    use zerodds_types::type_object::TypeObject;
    use zerodds_types::{TypeIdentifier, TypeIdentifierWithSize, TypeInformation};
    let mut registry = TypeRegistry::new();
    // Unrepresentable unrelated types may be present in a module. Only the
    // actual closure below decides availability; every referenced object must
    // be in the registry. Never use its error-swallowing convenience walkers.
    let _unrepresented = zerodds_types::from_ast::register_all(ast, &mut registry)
        .map_err(|e| format!("type registry: {e:?}"))?;
    let minimal = zerodds_types::from_ast::minimal_type_object(ast, id)
        .map_err(|e| format!("minimal object: {e:?}"))?;
    let complete = zerodds_types::from_ast::complete_type_object(ast, id)
        .map_err(|e| format!("complete object: {e:?}"))?;
    let mut info = TypeInformation::from_minimal_and_complete(&minimal, &complete)
        .map_err(|e| format!("type information: {e:?}"))?;
    let mut objects = Vec::new();
    for (is_complete, root, output) in [
        (false, TypeObject::Minimal(minimal), &mut info.minimal),
        (true, TypeObject::Complete(complete), &mut info.complete),
    ] {
        let root_hash =
            zerodds_types::hash::compute_hash(&root).map_err(|e| format!("root hash: {e:?}"))?;
        let mut seen = BTreeSet::from([root_hash]);
        let mut pending = collect_referenced_hashes(&root, DEFAULT_MAX_RESOLVE_DEPTH)
            .map_err(|e| format!("root dependencies: {e:?}"))?;
        objects.push(root);
        while let Some(hash) = pending.pop() {
            if !seen.insert(hash) {
                continue;
            }
            if seen.len() > DEFAULT_MAX_RESOLVE_NODES {
                return Err("type dependency closure exceeds node limit".into());
            }
            let (object, type_id, size) = if is_complete {
                let object = registry
                    .get_complete(&hash)
                    .ok_or_else(|| format!("missing complete dependency {hash:?}"))?;
                (
                    TypeObject::Complete(object.clone()),
                    TypeIdentifier::EquivalenceHashComplete(hash),
                    zerodds_types::hash::complete_serialized_size(object),
                )
            } else {
                let object = registry
                    .get_minimal(&hash)
                    .ok_or_else(|| format!("missing minimal dependency {hash:?}"))?;
                (
                    TypeObject::Minimal(object.clone()),
                    TypeIdentifier::EquivalenceHashMinimal(hash),
                    zerodds_types::hash::minimal_serialized_size(object),
                )
            };
            output.dependent_typeids.push(TypeIdentifierWithSize {
                type_id,
                typeobject_serialized_size: size.map_err(|e| format!("dependency size: {e:?}"))?,
            });
            pending.extend(
                collect_referenced_hashes(&object, DEFAULT_MAX_RESOLVE_DEPTH)
                    .map_err(|e| format!("transitive dependencies: {e:?}"))?,
            );
            objects.push(object);
        }
        output.dependent_typeid_count = i32::try_from(output.dependent_typeids.len())
            .map_err(|_| "dependency count overflow")?;
    }
    let bytes = info
        .to_bytes_le()
        .map_err(|e| format!("TypeInformation encode: {e:?}"))?;
    let decoded = TypeInformation::from_bytes_le(&bytes)
        .map_err(|e| format!("TypeInformation decode: {e:?}"))?;
    if decoded != info {
        return Err("TypeInformation round trip changed metadata".into());
    }
    Ok((bytes, objects))
}
