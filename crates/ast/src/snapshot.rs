// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Model snapshot. Spec §9.
//!
//! The whole model serialises to a deterministic binary form and loads
//! back with every `NodeId` preserved. Determinism follows from the
//! arena's ascending-slot iteration order rather than from a
//! serialisation library's field ordering, which is why a snapshot hash
//! can key a build cache.
//!
//! Every node kind the model holds round-trips: modules, expressions,
//! constants, types and their parts, interfaces with their operations,
//! parameters, attributes and exceptions, the deployment layer, the
//! annotation declarations, and the facet payload on all of them.

use crate::error::AstError;
use crate::facet::Facets;
use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::{Bound, CharWidth, Completeness, PrimitiveKind, TypeKind};
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{AstCoreError, Id, IdSpace, NodeId, Ref, SnapshotReader, SnapshotWriter};

/// Tag numbers for `TypeKind`, part of the snapshot format.
mod tag {
    pub const PRIMITIVE: u8 = 1;
    pub const FIXED: u8 = 2;
    pub const STRING: u8 = 3;
    pub const SEQUENCE: u8 = 4;
    pub const ARRAY: u8 = 5;
    pub const MAP: u8 = 6;
    pub const STRUCTURE: u8 = 7;
    pub const UNION: u8 = 8;
    pub const ENUMERATION: u8 = 9;
    pub const BITMASK: u8 = 10;
    pub const BITSET: u8 = 11;
    pub const ALIAS: u8 = 12;
    pub const NATIVE: u8 = 13;
    pub const VALUETYPE: u8 = 14;
    pub const VALUE_BOX: u8 = 15;
    pub const OBJECT_REF: u8 = 16;
    pub const ANY: u8 = 17;
    pub const VALUE_BASE: u8 = 18;
    pub const ANNOTATION_TYPE: u8 = 19;
}

impl Ast {
    /// Serialise the model.
    ///
    /// # Errors
    /// [`AstError::Core`] when a name or payload exceeds what the length
    /// prefix can express.
    pub fn to_snapshot(&self) -> Result<Vec<u8>, AstError> {
        let mut w = SnapshotWriter::with_header();

        // Identity high-water marks, so a loaded model continues where
        // this one left off rather than minting duplicates.
        w.u64(self.model_ids.next_counter());
        w.u64(self.builtin_ids.next_counter());
        w.u64(self.synthetic_ids.next_counter());
        w.u64(self.sequence);

        // Modules, in ascending slot order.
        w.u32(count(self.modules.slot_count())?);
        w.u32(count(self.modules.len())?);
        for (id, module) in self.modules.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(module.node_id);
            w.str(&module.name)?;
            match module.parent {
                Some(parent) => {
                    w.bool(true);
                    w.u32(parent.slot());
                    w.u32(parent.generation());
                }
                None => w.bool(false),
            }
            write_facets(&mut w, &module.facets)?;
        }

        // Expressions before the types that reference them.
        w.u32(count(self.exprs.slot_count())?);
        w.u32(count(self.exprs.len())?);
        for (id, expr) in self.exprs.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(expr.node_id);
            write_expr(&mut w, &expr.expr)?;
            write_facets(&mut w, &expr.facets)?;
        }

        // Types.
        w.u32(count(self.types.slot_count())?);
        w.u32(count(self.types.len())?);
        for (id, node) in self.types.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.module.slot());
            w.u32(node.module.generation());
            write_str_opt(&mut w, node.scope.as_deref())?;
            w.bool(node.completeness == Completeness::Complete);
            write_kind(&mut w, &node.kind)?;
            write_facets(&mut w, &node.facets)?;
        }

        // Members, after the types they belong to.
        w.u32(count(self.members.slot_count())?);
        w.u32(count(self.members.len())?);
        for (id, member) in self.members.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(member.node_id);
            w.str(&member.name)?;
            write_type_ref(&mut w, &member.type_)?;
            // A member is owned by a type or by an exception; encoding
            // only the type case loses every exception member.
            match member.parent {
                AnyId::Type(id, _) => {
                    w.u8(1);
                    w.u32(id.slot());
                    w.u32(id.generation());
                }
                AnyId::Exception(id, _) => {
                    w.u8(2);
                    w.u32(id.slot());
                    w.u32(id.generation());
                }
                _ => w.u8(0),
            }
            write_facets(&mut w, &member.facets)?;
        }

        // Cases, enumerators, bit values and bitfields, each after the
        // type that owns them.
        w.u32(count(self.cases.slot_count())?);
        w.u32(count(self.cases.len())?);
        for (id, case) in self.cases.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(case.node_id);
            w.u32(case.parent.slot());
            w.u32(case.parent.generation());
            w.u32(case.member.slot());
            w.u32(case.member.generation());
            w.u32(count(case.labels.len())?);
            for label in &case.labels {
                match label {
                    crate::CaseLabel::Default => w.u8(0),
                    crate::CaseLabel::Value(expr) => {
                        w.u8(1);
                        w.u32(expr.slot());
                        w.u32(expr.generation());
                    }
                }
            }
            write_facets(&mut w, &case.facets)?;
        }

        w.u32(count(self.enumerators.slot_count())?);
        w.u32(count(self.enumerators.len())?);
        for (id, node) in self.enumerators.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            match node.value {
                Some(expr) => {
                    w.bool(true);
                    w.u32(expr.slot());
                    w.u32(expr.generation());
                }
                None => w.bool(false),
            }
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.bit_values.slot_count())?);
        w.u32(count(self.bit_values.len())?);
        for (id, node) in self.bit_values.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            match node.position {
                Some(position) => {
                    w.bool(true);
                    w.u16(position);
                }
                None => w.bool(false),
            }
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.bitfields.slot_count())?);
        w.u32(count(self.bitfields.len())?);
        for (id, node) in self.bitfields.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            match node.name.as_deref() {
                Some(name) => {
                    w.bool(true);
                    w.str(name)?;
                }
                None => w.bool(false),
            }
            w.u16(node.bits);
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_type_ref_opt(&mut w, node.holder_type.as_ref())?;
            write_facets(&mut w, &node.facets)?;
        }

        // Constants, after the expressions they point at.
        w.u32(count(self.constants.slot_count())?);
        w.u32(count(self.constants.len())?);
        for (id, node) in self.constants.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.module.slot());
            w.u32(node.module.generation());
            write_str_opt(&mut w, node.scope.as_deref())?;
            w.u32(node.expr.slot());
            w.u32(node.expr.generation());
            write_type_ref(&mut w, &node.type_)?;
            write_facets(&mut w, &node.facets)?;
        }

        // Exceptions before the operations that raise them.
        w.u32(count(self.exceptions.slot_count())?);
        w.u32(count(self.exceptions.len())?);
        for (id, node) in self.exceptions.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.module.slot());
            w.u32(node.module.generation());
            write_str_opt(&mut w, node.scope.as_deref())?;
            write_handles(&mut w, &node.members)?;
            write_facets(&mut w, &node.facets)?;
        }

        // Interfaces, then their operations, parameters and attributes.
        w.u32(count(self.interfaces.slot_count())?);
        w.u32(count(self.interfaces.len())?);
        for (id, node) in self.interfaces.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.module.slot());
            w.u32(node.module.generation());
            w.u8(interface_kind_tag(node.kind));
            w.bool(node.completeness == Completeness::Complete);
            w.u32(count(node.bases.len())?);
            for base in &node.bases {
                write_ref(&mut w, base)?;
            }
            write_handles(&mut w, &node.operations)?;
            write_handles(&mut w, &node.attributes)?;
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.operations.slot_count())?);
        w.u32(count(self.operations.len())?);
        for (id, node) in self.operations.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_type_ref_opt(&mut w, node.return_type.as_ref())?;
            w.bool(node.oneway);
            write_handles(&mut w, &node.params)?;
            w.u32(count(node.raises.len())?);
            for raise in &node.raises {
                write_ref(&mut w, raise)?;
            }
            w.u32(count(node.contexts.len())?);
            for context in &node.contexts {
                w.str(context)?;
            }
            write_any_parent(&mut w, node.parent);
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.params.slot_count())?);
        w.u32(count(self.params.len())?);
        for (id, node) in self.params.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u8(direction_tag(node.direction));
            write_type_ref(&mut w, &node.type_)?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.attributes.slot_count())?);
        w.u32(count(self.attributes.len())?);
        for (id, node) in self.attributes.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_type_ref(&mut w, &node.type_)?;
            w.bool(node.readonly);
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            w.u32(count(node.get_raises.len())?);
            for raise in &node.get_raises {
                write_ref(&mut w, raise)?;
            }
            w.u32(count(node.set_raises.len())?);
            for raise in &node.set_raises {
                write_ref(&mut w, raise)?;
            }
            write_facets(&mut w, &node.facets)?;
        }

        // Annotation declarations and their members.
        w.u32(count(self.annotation_defs.slot_count())?);
        w.u32(count(self.annotation_defs.len())?);
        for (id, node) in self.annotation_defs.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.module.slot());
            w.u32(node.module.generation());
            write_handles(&mut w, &node.members)?;
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.annotation_members.slot_count())?);
        w.u32(count(self.annotation_members.len())?);
        for (id, node) in self.annotation_members.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_type_ref(&mut w, &node.type_)?;
            w.bool(node.is_value_member);
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            match node.default {
                Some(expr) => {
                    w.bool(true);
                    w.u32(expr.slot());
                    w.u32(expr.generation());
                }
                None => w.bool(false),
            }
            write_facets(&mut w, &node.facets)?;
        }

        // The deployment layer.
        w.u32(count(self.libraries.slot_count())?);
        w.u32(count(self.libraries.len())?);
        for (id, node) in self.libraries.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.domains.slot_count())?);
        w.u32(count(self.domains.len())?);
        for (id, node) in self.domains.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.domain_id);
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.qos_profiles.slot_count())?);
        w.u32(count(self.qos_profiles.len())?);
        for (id, node) in self.qos_profiles.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_ref_opt(&mut w, node.base.as_ref())?;
            write_policies(&mut w, &node.policies)?;
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.topics.slot_count())?);
        w.u32(count(self.topics.len())?);
        for (id, node) in self.topics.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_type_ref(&mut w, &node.type_)?;
            write_ref_opt(&mut w, node.qos.as_ref())?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.participants.slot_count())?);
        w.u32(count(self.participants.len())?);
        for (id, node) in self.participants.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            write_ref(&mut w, &node.domain)?;
            write_ref_opt(&mut w, node.qos.as_ref())?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_facets(&mut w, &node.facets)?;
        }

        w.u32(count(self.endpoints.slot_count())?);
        w.u32(count(self.endpoints.len())?);
        for (id, node) in self.endpoints.iter() {
            w.u32(id.slot());
            w.u32(id.generation());
            w.node_id(node.node_id);
            w.str(&node.name)?;
            w.bool(node.role == crate::EndpointRole::DataWriter);
            write_ref(&mut w, &node.topic)?;
            write_ref_opt(&mut w, node.qos.as_ref())?;
            w.u32(node.parent.slot());
            w.u32(node.parent.generation());
            write_facets(&mut w, &node.facets)?;
        }

        Ok(w.finish())
    }

    /// Load a model from a snapshot.
    ///
    /// Every `NodeId` is preserved and the indices are rebuilt, so the
    /// loaded model is usable immediately.
    ///
    /// # Errors
    /// [`AstError::Core`] with [`AstCoreError::MalformedSnapshot`] on a
    /// wrong magic, an unknown version, truncation or an out-of-range
    /// handle. Never panics; this is a fuzz target.
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, AstError> {
        let mut r = SnapshotReader::with_header(bytes)?;
        let mut ast = Self::empty_for_load();

        ast.model_ids.set_next_counter(r.u64()?);
        ast.builtin_ids.set_next_counter(r.u64()?);
        ast.synthetic_ids.set_next_counter(r.u64()?);
        ast.sequence = r.u64()?;

        let module_slots = bounded_count(&mut r, 1)?;
        let module_count = bounded_count(&mut r, min_size::MODULE)?;
        for _ in 0..module_count {
            let slot = bounded_slot(&mut r, module_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let parent = if r.bool()? {
                Some(Id::from_parts(r.u32()?, r.u32()?)?)
            } else {
                None
            };
            let facets = read_facets(&mut r)?;
            let id = ast.modules.insert_at(
                slot,
                generation,
                crate::ModuleNode {
                    node_id,
                    name,
                    parent,
                    children: Vec::new(),
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Module(id, node_id));
        }

        let expr_slots = bounded_count(&mut r, 1)?;
        let expr_count = bounded_count(&mut r, min_size::EXPR)?;
        for _ in 0..expr_count {
            let slot = bounded_slot(&mut r, expr_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let expr = read_expr(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.exprs.insert_at(
                slot,
                generation,
                crate::ConstExprNode {
                    node_id,
                    expr,
                    facets,
                },
            )?;
            let _ = ast
                .by_node_id
                .insert(node_id, AnyId::ConstExpr(id, node_id));
        }

        let type_slots = bounded_count(&mut r, 1)?;
        let type_count = bounded_count(&mut r, min_size::TYPE)?;
        for _ in 0..type_count {
            let slot = bounded_slot(&mut r, type_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let module = Id::from_parts(r.u32()?, r.u32()?)?;
            let scope = read_str_opt(&mut r)?;
            let completeness = if r.bool()? {
                Completeness::Complete
            } else {
                Completeness::Forward
            };
            let kind = read_kind(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.types.insert_at(
                slot,
                generation,
                crate::TypeNode {
                    node_id,
                    name,
                    module,
                    scope,
                    kind,
                    completeness,
                    facets,
                },
            )?;
            let any = AnyId::Type(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(m) = ast.modules.get_mut(module) {
                m.children.push(any);
            }
        }

        let member_slots = bounded_count(&mut r, 1)?;
        let member_count = bounded_count(&mut r, min_size::MEMBER)?;
        for _ in 0..member_count {
            let slot = bounded_slot(&mut r, member_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let type_ = read_type_ref(&mut r)?;
            let owner_tag = r.u8()?;
            let owner = if owner_tag == 0 {
                None
            } else {
                Some((r.u32()?, r.u32()?))
            };
            let facets = read_facets(&mut r)?;

            let parent = match (owner_tag, owner) {
                (1, Some((slot, generation))) => {
                    let id: Id<crate::TypeNode> = Id::from_parts(slot, generation)?;
                    AnyId::Type(id, ast.types.get(id)?.node_id)
                }
                (2, Some((slot, generation))) => {
                    // Exceptions are written after members, so the
                    // owner's identity is not known yet; it is filled in
                    // by the fix-up pass once that section is read.
                    let id: Id<crate::ExceptionNode> = Id::from_parts(slot, generation)?;
                    AnyId::Exception(id, NodeId::from_parts(IdSpace::MODEL, 0))
                }
                _ => {
                    return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a member's owning type or exception",
                    }));
                }
            };

            let id = ast.members.insert_at(
                slot,
                generation,
                crate::MemberNode {
                    node_id,
                    name,
                    type_,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Member(id, node_id));
        }

        // Cases, enumerators, bit values, bitfields.
        let case_slots = bounded_count(&mut r, 1)?;
        let case_count = bounded_count(&mut r, 24)?;
        for _ in 0..case_count {
            let slot = bounded_slot(&mut r, case_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let member = Id::from_parts(r.u32()?, r.u32()?)?;
            let label_count = bounded_count(&mut r, 1)? as usize;
            let mut labels = Vec::new();
            for _ in 0..label_count {
                labels.push(match r.u8()? {
                    0 => crate::CaseLabel::Default,
                    1 => crate::CaseLabel::Value(Id::from_parts(r.u32()?, r.u32()?)?),
                    _ => {
                        return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                            offset: r.offset(),
                            expected: "case label tag 0 or 1",
                        }));
                    }
                });
            }
            let facets = read_facets(&mut r)?;
            let id = ast.cases.insert_at(
                slot,
                generation,
                crate::CaseNode {
                    node_id,
                    labels,
                    member,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Case(id, node_id));
        }

        let enum_slots = bounded_count(&mut r, 1)?;
        let enum_count = bounded_count(&mut r, 24)?;
        for _ in 0..enum_count {
            let slot = bounded_slot(&mut r, enum_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let value = read_handle_opt(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.enumerators.insert_at(
                slot,
                generation,
                crate::EnumeratorNode {
                    node_id,
                    name,
                    value,
                    parent,
                    facets,
                },
            )?;
            let _ = ast
                .by_node_id
                .insert(node_id, AnyId::Enumerator(id, node_id));
        }

        let bit_slots = bounded_count(&mut r, 1)?;
        let bit_count = bounded_count(&mut r, 24)?;
        for _ in 0..bit_count {
            let slot = bounded_slot(&mut r, bit_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let position = if r.bool()? { Some(r.u16()?) } else { None };
            let facets = read_facets(&mut r)?;
            let id = ast.bit_values.insert_at(
                slot,
                generation,
                crate::BitValueNode {
                    node_id,
                    name,
                    position,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::BitValue(id, node_id));
        }

        let field_slots = bounded_count(&mut r, 1)?;
        let field_count = bounded_count(&mut r, 24)?;
        for _ in 0..field_count {
            let slot = bounded_slot(&mut r, field_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = if r.bool()? { Some(r.string()?) } else { None };
            let bits = r.u16()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let holder_type = read_ref_opt(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.bitfields.insert_at(
                slot,
                generation,
                crate::BitfieldNode {
                    node_id,
                    name,
                    bits,
                    holder_type,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Bitfield(id, node_id));
        }

        // Constants.
        let const_slots = bounded_count(&mut r, 1)?;
        let const_count = bounded_count(&mut r, 24)?;
        for _ in 0..const_count {
            let slot = bounded_slot(&mut r, const_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let module = Id::from_parts(r.u32()?, r.u32()?)?;
            let scope = read_str_opt(&mut r)?;
            let expr = Id::from_parts(r.u32()?, r.u32()?)?;
            let type_ = read_type_ref(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.constants.insert_at(
                slot,
                generation,
                crate::ConstNode {
                    node_id,
                    name,
                    module,
                    scope,
                    type_,
                    expr,
                    facets,
                },
            )?;
            let any = AnyId::Const(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(m) = ast.modules.get_mut(module) {
                m.children.push(any);
            }
        }

        // Exceptions.
        let exc_slots = bounded_count(&mut r, 1)?;
        let exc_count = bounded_count(&mut r, 24)?;
        for _ in 0..exc_count {
            let slot = bounded_slot(&mut r, exc_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let module = Id::from_parts(r.u32()?, r.u32()?)?;
            let scope = read_str_opt(&mut r)?;
            let members = read_handles(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.exceptions.insert_at(
                slot,
                generation,
                crate::ExceptionNode {
                    node_id,
                    name,
                    module,
                    scope,
                    members,
                    facets,
                },
            )?;
            let any = AnyId::Exception(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(m) = ast.modules.get_mut(module) {
                m.children.push(any);
            }
        }

        // Interfaces, operations, parameters, attributes.
        let iface_slots = bounded_count(&mut r, 1)?;
        let iface_count = bounded_count(&mut r, 24)?;
        for _ in 0..iface_count {
            let slot = bounded_slot(&mut r, iface_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let module = Id::from_parts(r.u32()?, r.u32()?)?;
            let kind = interface_kind_from_tag(r.u8()?, &r)?;
            let completeness = if r.bool()? {
                Completeness::Complete
            } else {
                Completeness::Forward
            };
            let base_count = bounded_count(&mut r, 1)? as usize;
            let mut bases = Vec::new();
            for _ in 0..base_count {
                bases.push(read_ref(&mut r)?);
            }
            let operations = read_handles(&mut r)?;
            let attributes = read_handles(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.interfaces.insert_at(
                slot,
                generation,
                crate::InterfaceNode {
                    node_id,
                    name,
                    module,
                    kind,
                    bases,
                    operations,
                    attributes,
                    completeness,
                    facets,
                },
            )?;
            let any = AnyId::Interface(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(m) = ast.modules.get_mut(module) {
                m.children.push(any);
            }
        }

        let op_slots = bounded_count(&mut r, 1)?;
        let op_count = bounded_count(&mut r, 24)?;
        for _ in 0..op_count {
            let slot = bounded_slot(&mut r, op_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let return_type = read_ref_opt(&mut r)?;
            let oneway = r.bool()?;
            let params = read_handles(&mut r)?;
            let raise_count = bounded_count(&mut r, 1)? as usize;
            let mut raises = Vec::new();
            for _ in 0..raise_count {
                raises.push(read_ref(&mut r)?);
            }
            let context_count = bounded_count(&mut r, 4)? as usize;
            let mut contexts = Vec::new();
            for _ in 0..context_count {
                contexts.push(r.string()?);
            }
            let parent = read_any_parent(&mut r, &ast)?;
            let facets = read_facets(&mut r)?;
            let id = ast.operations.insert_at(
                slot,
                generation,
                crate::OperationNode {
                    node_id,
                    name,
                    return_type,
                    params,
                    raises,
                    oneway,
                    contexts,
                    parent,
                    facets,
                },
            )?;
            let _ = ast
                .by_node_id
                .insert(node_id, AnyId::Operation(id, node_id));
        }

        let param_slots = bounded_count(&mut r, 1)?;
        let param_count = bounded_count(&mut r, 24)?;
        for _ in 0..param_count {
            let slot = bounded_slot(&mut r, param_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let direction = direction_from_tag(r.u8()?, &r)?;
            let type_ = read_type_ref(&mut r)?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let facets = read_facets(&mut r)?;
            let id = ast.params.insert_at(
                slot,
                generation,
                crate::ParamNode {
                    node_id,
                    name,
                    direction,
                    type_,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Param(id, node_id));
        }

        let attr_slots = bounded_count(&mut r, 1)?;
        let attr_count = bounded_count(&mut r, 24)?;
        for _ in 0..attr_count {
            let slot = bounded_slot(&mut r, attr_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let type_ = read_type_ref(&mut r)?;
            let readonly = r.bool()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let get_count = bounded_count(&mut r, 1)? as usize;
            let mut get_raises = Vec::new();
            for _ in 0..get_count {
                get_raises.push(read_ref(&mut r)?);
            }
            let set_count = bounded_count(&mut r, 1)? as usize;
            let mut set_raises = Vec::new();
            for _ in 0..set_count {
                set_raises.push(read_ref(&mut r)?);
            }
            let facets = read_facets(&mut r)?;
            let id = ast.attributes.insert_at(
                slot,
                generation,
                crate::AttributeNode {
                    node_id,
                    name,
                    type_,
                    readonly,
                    get_raises,
                    set_raises,
                    parent,
                    facets,
                },
            )?;
            let _ = ast
                .by_node_id
                .insert(node_id, AnyId::Attribute(id, node_id));
        }

        // Annotations.
        let adef_slots = bounded_count(&mut r, 1)?;
        let adef_count = bounded_count(&mut r, 24)?;
        for _ in 0..adef_count {
            let slot = bounded_slot(&mut r, adef_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let module = Id::from_parts(r.u32()?, r.u32()?)?;
            let members = read_handles(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.annotation_defs.insert_at(
                slot,
                generation,
                crate::AnnotationDefNode {
                    node_id,
                    name,
                    module,
                    members,
                    facets,
                },
            )?;
            let any = AnyId::AnnotationDef(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(m) = ast.modules.get_mut(module) {
                m.children.push(any);
            }
        }

        let amem_slots = bounded_count(&mut r, 1)?;
        let amem_count = bounded_count(&mut r, 24)?;
        for _ in 0..amem_count {
            let slot = bounded_slot(&mut r, amem_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let type_ = read_type_ref(&mut r)?;
            let is_value_member = r.bool()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let default = read_handle_opt(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.annotation_members.insert_at(
                slot,
                generation,
                crate::AnnotationMemberNode {
                    node_id,
                    name,
                    type_,
                    default,
                    is_value_member,
                    parent,
                    facets,
                },
            )?;
            let _ = ast
                .by_node_id
                .insert(node_id, AnyId::AnnotationMember(id, node_id));
        }

        // Deployment.
        let lib_slots = bounded_count(&mut r, 1)?;
        let lib_count = bounded_count(&mut r, 24)?;
        for _ in 0..lib_count {
            let slot = bounded_slot(&mut r, lib_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let facets = read_facets(&mut r)?;
            let id = ast.libraries.insert_at(
                slot,
                generation,
                crate::LibraryNode {
                    node_id,
                    name,
                    children: Vec::new(),
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Library(id, node_id));
        }

        let dom_slots = bounded_count(&mut r, 1)?;
        let dom_count = bounded_count(&mut r, 24)?;
        for _ in 0..dom_count {
            let slot = bounded_slot(&mut r, dom_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let domain_id = r.u32()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let facets = read_facets(&mut r)?;
            let id = ast.domains.insert_at(
                slot,
                generation,
                crate::DomainNode {
                    node_id,
                    name,
                    domain_id,
                    topics: Vec::new(),
                    parent,
                    facets,
                },
            )?;
            let any = AnyId::Domain(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(l) = ast.libraries.get_mut(parent) {
                l.children.push(any);
            }
        }

        let qos_slots = bounded_count(&mut r, 1)?;
        let qos_count = bounded_count(&mut r, 24)?;
        for _ in 0..qos_count {
            let slot = bounded_slot(&mut r, qos_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let base = read_ref_opt(&mut r)?;
            let policies = read_policies(&mut r)?;
            let facets = read_facets(&mut r)?;
            let id = ast.qos_profiles.insert_at(
                slot,
                generation,
                crate::QosProfileNode {
                    node_id,
                    name,
                    base,
                    policies,
                    parent,
                    facets,
                },
            )?;
            let any = AnyId::QosProfile(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(l) = ast.libraries.get_mut(parent) {
                l.children.push(any);
            }
        }

        let topic_slots = bounded_count(&mut r, 1)?;
        let topic_count = bounded_count(&mut r, 24)?;
        for _ in 0..topic_count {
            let slot = bounded_slot(&mut r, topic_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let type_ = read_type_ref(&mut r)?;
            let qos = read_ref_opt(&mut r)?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let facets = read_facets(&mut r)?;
            let id = ast.topics.insert_at(
                slot,
                generation,
                crate::TopicNode {
                    node_id,
                    name,
                    type_,
                    qos,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Topic(id, node_id));
            if let Ok(d) = ast.domains.get_mut(parent) {
                d.topics.push(id);
            }
        }

        let part_slots = bounded_count(&mut r, 1)?;
        let part_count = bounded_count(&mut r, 24)?;
        for _ in 0..part_count {
            let slot = bounded_slot(&mut r, part_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let domain = read_ref(&mut r)?;
            let qos = read_ref_opt(&mut r)?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let facets = read_facets(&mut r)?;
            let id = ast.participants.insert_at(
                slot,
                generation,
                crate::ParticipantNode {
                    node_id,
                    name,
                    domain,
                    qos,
                    endpoints: Vec::new(),
                    parent,
                    facets,
                },
            )?;
            let any = AnyId::Participant(id, node_id);
            let _ = ast.by_node_id.insert(node_id, any);
            if let Ok(l) = ast.libraries.get_mut(parent) {
                l.children.push(any);
            }
        }

        let ep_slots = bounded_count(&mut r, 1)?;
        let ep_count = bounded_count(&mut r, 24)?;
        for _ in 0..ep_count {
            let slot = bounded_slot(&mut r, ep_slots)?;
            let generation = r.u32()?;
            let node_id = r.node_id()?;
            let name = r.string()?;
            let role = if r.bool()? {
                crate::EndpointRole::DataWriter
            } else {
                crate::EndpointRole::DataReader
            };
            let topic = read_ref(&mut r)?;
            let qos = read_ref_opt(&mut r)?;
            let parent = Id::from_parts(r.u32()?, r.u32()?)?;
            let facets = read_facets(&mut r)?;
            let id = ast.endpoints.insert_at(
                slot,
                generation,
                crate::EndpointNode {
                    node_id,
                    name,
                    role,
                    topic,
                    qos,
                    parent,
                    facets,
                },
            )?;
            let _ = ast.by_node_id.insert(node_id, AnyId::Endpoint(id, node_id));
            if let Ok(p) = ast.participants.get_mut(parent) {
                p.endpoints.push(id);
            }
        }

        r.expect_end()?;

        // Members owned by an exception were loaded before the
        // exceptions were, so their owner identity is a placeholder
        // until now. Repairing it here rather than reordering the
        // sections keeps the format's dependency order simple.
        let member_ids: Vec<Id<crate::MemberNode>> = ast.members.ids().collect();
        for member in member_ids {
            let Ok(node) = ast.members.get(member) else {
                continue;
            };
            let AnyId::Exception(owner, _) = node.parent else {
                continue;
            };
            let Ok(owner_node) = ast.exceptions.get(owner) else {
                continue;
            };
            let owner_id = owner_node.node_id;
            if let Ok(node) = ast.members.get_mut(member) {
                node.parent = AnyId::Exception(owner, owner_id);
            }
        }

        // Members owned by an exception were read before the exceptions
        // were, so their owner identity is still a placeholder. Filling
        // it in here keeps the format's section order simple; the
        // alternative is writing exceptions before members, which would
        // put them ahead of the types their members reference.
        let member_ids: Vec<Id<crate::MemberNode>> = ast.members.ids().collect();
        for member in member_ids {
            let Ok(node) = ast.members.get(member) else {
                continue;
            };
            let AnyId::Exception(owner, _) = node.parent else {
                continue;
            };
            let Ok(owner_node) = ast.exceptions.get(owner) else {
                continue;
            };
            let owner_id = owner_node.node_id;
            if let Ok(node) = ast.members.get_mut(member) {
                node.parent = AnyId::Exception(owner, owner_id);
            }
        }

        // A struct's member list was written inside its kind, so the
        // handles are already in place; only the indices need building —
        // the waiter index too, or a pending reference read back would
        // never resolve.
        ast.reindex();
        let nodes: Vec<AnyId> = ast.by_node_id.values().copied().collect();
        for any in nodes {
            ast.index_pending(any);
        }
        Ok(ast)
    }
}

fn count(value: usize) -> Result<u32, AstError> {
    u32::try_from(value).map_err(|_| AstError::Core(AstCoreError::Exhausted))
}

/// Read an element count and check it against what the buffer can
/// actually hold.
///
/// Without this a crafted snapshot naming four billion entries makes the
/// loader reserve memory for them before discovering the bytes are not
/// there — found by the fuzz target as an out-of-memory abort rather
/// than the typed error the spec requires. `min_entry` is the smallest
/// number of bytes one entry can occupy.
fn bounded_count(r: &mut SnapshotReader<'_>, min_entry: usize) -> Result<u32, AstError> {
    let declared = r.u32()?;
    let needed = (declared as usize).saturating_mul(min_entry);
    if needed > r.remaining() {
        return Err(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "an element count the remaining buffer can hold",
        }));
    }
    Ok(declared)
}

/// Read a slot index and check it against the section's declared slot
/// capacity.
///
/// Without this a crafted snapshot naming slot 4294967295 makes the
/// arena grow its slot vector to that index before failing — the second
/// out-of-memory path the fuzz target found. The capacity itself is
/// bounded against the remaining buffer by `bounded_count`.
fn bounded_slot(r: &mut SnapshotReader<'_>, capacity: u32) -> Result<u32, AstError> {
    let slot = r.u32()?;
    if slot >= capacity {
        return Err(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a slot index within the section's declared capacity",
        }));
    }
    Ok(slot)
}

/// Smallest byte size of one entry, per section. Slot, generation and
/// identity alone account for most of it.
mod min_size {
    /// slot + generation + node id + name length + parent flag
    pub const MODULE: usize = 4 + 4 + 16 + 4 + 1;
    /// slot + generation + node id + expression tag
    pub const EXPR: usize = 4 + 4 + 16 + 1;
    /// slot + generation + node id + name length + module handle +
    /// completeness + kind tag
    pub const TYPE: usize = 4 + 4 + 16 + 4 + 8 + 1 + 1;
    /// slot + generation + node id + name length + reference tag +
    /// parent flag + facet flag
    pub const MEMBER: usize = 4 + 4 + 16 + 4 + 1 + 1 + 1;
    /// slot + generation
    pub const HANDLE: usize = 8;
    /// reference tag, for a list of references
    pub const REFERENCE: usize = 1;
}

fn write_type_ref(
    w: &mut SnapshotWriter,
    reference: &Ref<crate::TypeNode>,
) -> Result<(), AstError> {
    write_ref(w, reference)
}

fn read_type_ref(r: &mut SnapshotReader<'_>) -> Result<Ref<crate::TypeNode>, AstError> {
    read_ref(r)
}

fn write_bound(w: &mut SnapshotWriter, bound: Bound) {
    match bound {
        Bound::Unbounded => w.u8(0),
        Bound::Const(id) => {
            w.u8(1);
            w.u32(id.slot());
            w.u32(id.generation());
        }
    }
}

fn read_bound(r: &mut SnapshotReader<'_>) -> Result<Bound, AstError> {
    Ok(match r.u8()? {
        0 => Bound::Unbounded,
        1 => Bound::Const(Id::from_parts(r.u32()?, r.u32()?)?),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "bound tag 0 or 1",
            }));
        }
    })
}

fn write_handles<T>(w: &mut SnapshotWriter, handles: &[Id<T>]) -> Result<(), AstError> {
    w.u32(count(handles.len())?);
    for handle in handles {
        w.u32(handle.slot());
        w.u32(handle.generation());
    }
    Ok(())
}

fn read_handles<T>(r: &mut SnapshotReader<'_>) -> Result<Vec<Id<T>>, AstError> {
    let len = bounded_count(r, min_size::HANDLE)? as usize;
    let mut out = Vec::new();
    for _ in 0..len {
        out.push(Id::from_parts(r.u32()?, r.u32()?)?);
    }
    Ok(out)
}

fn write_kind(w: &mut SnapshotWriter, kind: &TypeKind) -> Result<(), AstError> {
    match kind {
        TypeKind::Primitive(p) => {
            w.u8(tag::PRIMITIVE);
            w.u8(primitive_tag(*p));
        }
        TypeKind::Fixed { digits, scale } => {
            w.u8(tag::FIXED);
            w.u8(*digits);
            w.u8(*scale);
        }
        TypeKind::String { width, bound } => {
            w.u8(tag::STRING);
            w.u8(u8::from(matches!(width, CharWidth::WChar)));
            write_bound(w, *bound);
        }
        TypeKind::Sequence { element, bound } => {
            w.u8(tag::SEQUENCE);
            write_type_ref(w, element)?;
            write_bound(w, *bound);
        }
        TypeKind::Array { element, dims } => {
            w.u8(tag::ARRAY);
            write_type_ref(w, element)?;
            write_handles(w, dims)?;
        }
        TypeKind::Map { key, value, bound } => {
            w.u8(tag::MAP);
            write_type_ref(w, key)?;
            write_type_ref(w, value)?;
            write_bound(w, *bound);
        }
        TypeKind::Structure { base, members } => {
            w.u8(tag::STRUCTURE);
            match base {
                Some(base) => {
                    w.bool(true);
                    write_type_ref(w, base)?;
                }
                None => w.bool(false),
            }
            write_handles(w, members)?;
        }
        TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } => {
            w.u8(tag::UNION);
            write_type_ref(w, discriminator)?;
            write_handles(w, cases)?;
            // The `default:` case was dropped on the way out and read
            // back as `None`, so a union with a default arm loaded as one
            // without: the arm was still in `cases`, but nothing said
            // which one it was.
            write_handle_opt(w, *default_case);
        }
        TypeKind::Enumeration { enumerators } => {
            w.u8(tag::ENUMERATION);
            write_handles(w, enumerators)?;
        }
        TypeKind::Bitmask { bits } => {
            w.u8(tag::BITMASK);
            write_handles(w, bits)?;
        }
        TypeKind::Bitset { base, fields } => {
            w.u8(tag::BITSET);
            write_handles(w, fields)?;
            // A bitset's inherited base was dropped the same way.
            write_type_ref_opt(w, base.as_ref())?;
        }
        TypeKind::Alias { aliased } => {
            w.u8(tag::ALIAS);
            write_type_ref(w, aliased)?;
        }
        TypeKind::Valuetype {
            kind,
            bases,
            state,
            initializers,
            supported,
        } => {
            w.u8(tag::VALUETYPE);
            w.u8(valuetype_tag(*kind));
            w.u32(count(bases.len())?);
            for base in bases {
                write_type_ref(w, base)?;
            }
            write_handles(w, state)?;
            write_handles(w, initializers)?;
            w.u32(count(supported.len())?);
            for interface in supported {
                write_ref(w, interface)?;
            }
        }
        TypeKind::ValueBox { boxed } => {
            w.u8(tag::VALUE_BOX);
            write_type_ref(w, boxed)?;
        }
        TypeKind::ObjectRef { interface } => {
            w.u8(tag::OBJECT_REF);
            match interface {
                Some(interface) => {
                    w.bool(true);
                    write_ref(w, interface)?;
                }
                None => w.bool(false),
            }
        }
        TypeKind::Any => w.u8(tag::ANY),
        TypeKind::ValueBase => w.u8(tag::VALUE_BASE),
        TypeKind::AnnotationType { definition } => {
            w.u8(tag::ANNOTATION_TYPE);
            write_handle_opt(w, Some(*definition));
        }
        TypeKind::Native => w.u8(tag::NATIVE),
    }
    Ok(())
}

fn valuetype_tag(kind: crate::ValuetypeKind) -> u8 {
    match kind {
        crate::ValuetypeKind::Regular => 0,
        crate::ValuetypeKind::Abstract => 1,
        crate::ValuetypeKind::Custom => 2,
        crate::ValuetypeKind::Truncatable => 3,
    }
}

fn read_kind(r: &mut SnapshotReader<'_>) -> Result<TypeKind, AstError> {
    Ok(match r.u8()? {
        tag::PRIMITIVE => TypeKind::Primitive(primitive_from_tag(r.u8()?, r)?),
        tag::FIXED => TypeKind::Fixed {
            digits: r.u8()?,
            scale: r.u8()?,
        },
        tag::STRING => TypeKind::String {
            width: if r.u8()? == 0 {
                CharWidth::Char
            } else {
                CharWidth::WChar
            },
            bound: read_bound(r)?,
        },
        tag::SEQUENCE => TypeKind::Sequence {
            element: read_type_ref(r)?,
            bound: read_bound(r)?,
        },
        tag::ARRAY => TypeKind::Array {
            element: read_type_ref(r)?,
            dims: read_handles(r)?,
        },
        tag::MAP => TypeKind::Map {
            key: read_type_ref(r)?,
            value: read_type_ref(r)?,
            bound: read_bound(r)?,
        },
        tag::STRUCTURE => TypeKind::Structure {
            base: if r.bool()? {
                Some(read_type_ref(r)?)
            } else {
                None
            },
            members: read_handles(r)?,
        },
        tag::UNION => TypeKind::Union {
            discriminator: read_type_ref(r)?,
            cases: read_handles(r)?,
            default_case: read_handle_opt(r)?,
        },
        tag::ENUMERATION => TypeKind::Enumeration {
            enumerators: read_handles(r)?,
        },
        tag::BITMASK => TypeKind::Bitmask {
            bits: read_handles(r)?,
        },
        tag::BITSET => {
            let fields = read_handles(r)?;
            TypeKind::Bitset {
                fields,
                base: read_ref_opt(r)?,
            }
        }
        tag::ALIAS => TypeKind::Alias {
            aliased: read_type_ref(r)?,
        },
        tag::NATIVE => TypeKind::Native,
        tag::VALUETYPE => {
            let tag = r.u8()?;
            let kind =
                crate::ValuetypeKind::ALL
                    .get(tag as usize)
                    .copied()
                    .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a known valuetype modifier tag",
                    }))?;
            let base_count = bounded_count(r, min_size::REFERENCE)? as usize;
            let mut bases = Vec::new();
            for _ in 0..base_count {
                bases.push(read_type_ref(r)?);
            }
            let state = read_handles(r)?;
            let initializers = read_handles(r)?;
            let supported_count = bounded_count(r, min_size::REFERENCE)? as usize;
            let mut supported = Vec::new();
            for _ in 0..supported_count {
                supported.push(read_ref(r)?);
            }
            TypeKind::Valuetype {
                kind,
                bases,
                state,
                initializers,
                supported,
            }
        }
        tag::VALUE_BOX => TypeKind::ValueBox {
            boxed: read_type_ref(r)?,
        },
        tag::OBJECT_REF => TypeKind::ObjectRef {
            interface: if r.bool()? { Some(read_ref(r)?) } else { None },
        },
        tag::ANY => TypeKind::Any,
        tag::VALUE_BASE => TypeKind::ValueBase,
        tag::ANNOTATION_TYPE => TypeKind::AnnotationType {
            definition: read_handle_opt(r)?.ok_or(AstError::Core(
                AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "an annotation type's declaration handle",
                },
            ))?,
        },
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known type kind tag",
            }));
        }
    })
}

fn primitive_tag(kind: PrimitiveKind) -> u8 {
    match kind {
        PrimitiveKind::Boolean => 0,
        PrimitiveKind::Octet => 1,
        PrimitiveKind::Char => 2,
        PrimitiveKind::WChar => 3,
        PrimitiveKind::Int8 => 4,
        PrimitiveKind::UInt8 => 5,
        PrimitiveKind::Int16 => 6,
        PrimitiveKind::UInt16 => 7,
        PrimitiveKind::Int32 => 8,
        PrimitiveKind::UInt32 => 9,
        PrimitiveKind::Int64 => 10,
        PrimitiveKind::UInt64 => 11,
        PrimitiveKind::Float32 => 12,
        PrimitiveKind::Float64 => 13,
        PrimitiveKind::Float128 => 14,
    }
}

fn primitive_from_tag(tag: u8, r: &SnapshotReader<'_>) -> Result<PrimitiveKind, AstError> {
    PrimitiveKind::ALL
        .get(tag as usize)
        .copied()
        .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known primitive tag",
        }))
}

fn write_expr(w: &mut SnapshotWriter, expr: &crate::ConstExpr) -> Result<(), AstError> {
    match expr {
        crate::ConstExpr::Literal(crate::Literal::Integer(value)) => {
            w.u8(0);
            w.i128(*value);
        }
        crate::ConstExpr::Literal(crate::Literal::Boolean(value)) => {
            w.u8(1);
            w.bool(*value);
        }
        crate::ConstExpr::Binary { op, lhs, rhs } => {
            w.u8(2);
            w.u8(binary_tag(*op));
            w.u32(lhs.slot());
            w.u32(lhs.generation());
            w.u32(rhs.slot());
            w.u32(rhs.generation());
        }
        crate::ConstExpr::Unary { op, operand } => {
            w.u8(3);
            w.u8(unary_tag(*op));
            w.u32(operand.slot());
            w.u32(operand.generation());
        }
        crate::ConstExpr::Literal(crate::Literal::Float { mantissa, exponent }) => {
            w.u8(4);
            w.str(mantissa)?;
            write_str_opt(w, exponent.as_deref())?;
        }
        crate::ConstExpr::Literal(crate::Literal::Fixed { digits, scale }) => {
            w.u8(5);
            w.str(digits)?;
            w.u8(*scale);
        }
        crate::ConstExpr::Literal(crate::Literal::Char(value)) => {
            w.u8(6);
            w.u32(*value as u32);
        }
        crate::ConstExpr::Literal(crate::Literal::WChar(value)) => {
            w.u8(7);
            w.u32(*value);
        }
        crate::ConstExpr::Literal(crate::Literal::String(value)) => {
            w.u8(8);
            w.str(value)?;
        }
        crate::ConstExpr::Literal(crate::Literal::WString(value)) => {
            w.u8(9);
            w.u32(count(value.len())?);
            for point in value {
                w.u32(*point);
            }
        }
        crate::ConstExpr::Literal(crate::Literal::Enumerator(reference)) => {
            w.u8(10);
            write_ref(w, reference)?;
        }
        crate::ConstExpr::NameRef(reference) => {
            w.u8(11);
            write_ref(w, reference)?;
        }
    }
    Ok(())
}

fn read_expr(r: &mut SnapshotReader<'_>) -> Result<crate::ConstExpr, AstError> {
    Ok(match r.u8()? {
        0 => crate::ConstExpr::Literal(crate::Literal::Integer(r.i128()?)),
        1 => crate::ConstExpr::Literal(crate::Literal::Boolean(r.bool()?)),
        2 => {
            let op = binary_from_tag(r.u8()?, r)?;
            let lhs = Id::from_parts(r.u32()?, r.u32()?)?;
            let rhs = Id::from_parts(r.u32()?, r.u32()?)?;
            crate::ConstExpr::Binary { op, lhs, rhs }
        }
        3 => {
            let op = unary_from_tag(r.u8()?, r)?;
            let operand = Id::from_parts(r.u32()?, r.u32()?)?;
            crate::ConstExpr::Unary { op, operand }
        }
        4 => crate::ConstExpr::Literal(crate::Literal::Float {
            mantissa: r.string()?,
            exponent: read_str_opt(r)?,
        }),
        5 => crate::ConstExpr::Literal(crate::Literal::Fixed {
            digits: r.string()?,
            scale: r.u8()?,
        }),
        6 => {
            let value = r.u32()?;
            let ch =
                char::from_u32(value).ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a character literal's code point",
                }))?;
            crate::ConstExpr::Literal(crate::Literal::Char(ch))
        }
        7 => crate::ConstExpr::Literal(crate::Literal::WChar(r.u32()?)),
        8 => crate::ConstExpr::Literal(crate::Literal::String(r.string()?)),
        9 => {
            let n = bounded_count(r, 4)? as usize;
            let mut points = Vec::new();
            for _ in 0..n {
                points.push(r.u32()?);
            }
            crate::ConstExpr::Literal(crate::Literal::WString(points))
        }
        10 => crate::ConstExpr::Literal(crate::Literal::Enumerator(read_ref(r)?)),
        11 => crate::ConstExpr::NameRef(read_ref(r)?),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known expression tag",
            }));
        }
    })
}

fn binary_tag(op: crate::node::constant::BinaryOp) -> u8 {
    use crate::node::constant::BinaryOp as B;
    match op {
        B::Or => 0,
        B::Xor => 1,
        B::And => 2,
        B::Shl => 3,
        B::Shr => 4,
        B::Add => 5,
        B::Sub => 6,
        B::Mul => 7,
        B::Div => 8,
        B::Mod => 9,
    }
}

fn binary_from_tag(
    tag: u8,
    r: &SnapshotReader<'_>,
) -> Result<crate::node::constant::BinaryOp, AstError> {
    crate::node::constant::BinaryOp::ALL
        .get(tag as usize)
        .copied()
        .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known binary operator tag",
        }))
}

fn unary_tag(op: crate::UnaryOp) -> u8 {
    match op {
        crate::UnaryOp::Plus => 0,
        crate::UnaryOp::Minus => 1,
        crate::UnaryOp::Not => 2,
    }
}

fn unary_from_tag(tag: u8, r: &SnapshotReader<'_>) -> Result<crate::UnaryOp, AstError> {
    crate::UnaryOp::ALL
        .get(tag as usize)
        .copied()
        .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known unary operator tag",
        }))
}

// -- generic reference and facet helpers ---------------------------------

/// A reference: tag 1 and the handle when resolved, tag 2 and the lookup
/// key when pending, tag 3 and the cause when broken.
///
/// A pending reference keeps its key so that a model saved half resolved
/// loads half resolved — still waiting on the same name, and resolved by
/// the declaration that arrives later — rather than as a broken reference
/// with an empty name. The key is the frontend's, and it is written as the
/// frontend left it.
fn write_ref<T>(w: &mut SnapshotWriter, reference: &Ref<T>) -> Result<(), AstError> {
    match reference {
        Ref::Resolved(id) => {
            w.u8(1);
            w.u32(id.slot());
            w.u32(id.generation());
        }
        Ref::Pending(key) => {
            w.u8(2);
            write_pending_key(w, key)?;
        }
        Ref::Broken(cause) => {
            w.u8(3);
            write_break_cause(w, cause)?;
        }
    }
    Ok(())
}

fn write_pending_key(
    w: &mut SnapshotWriter,
    key: &zerodds_ast_core::PendingRef,
) -> Result<(), AstError> {
    use zerodds_ast_core::{PendingRef, RosInterfaceKind};
    match key {
        PendingRef::ScopedName(name) => {
            w.u8(0);
            w.bool(name.is_absolute());
            write_strings(w, name.segments())?;
        }
        PendingRef::TypeIdentifier(identifier) => {
            w.u8(1);
            for octet in identifier.0 {
                w.u8(octet);
            }
        }
        PendingRef::RosTypeName(name) => {
            w.u8(2);
            w.str(&name.package)?;
            w.u8(match name.kind {
                RosInterfaceKind::Msg => 0,
                RosInterfaceKind::Srv => 1,
                RosInterfaceKind::Action => 2,
            });
            w.str(&name.name)?;
        }
        PendingRef::RepositoryId(id) => {
            w.u8(3);
            w.str(&id.0)?;
        }
        PendingRef::XmlRef(key) => {
            w.u8(4);
            w.str(&key.0)?;
        }
    }
    Ok(())
}

fn write_break_cause(
    w: &mut SnapshotWriter,
    cause: &zerodds_ast_core::BreakCause,
) -> Result<(), AstError> {
    use zerodds_ast_core::BreakCause;
    match cause {
        BreakCause::NotFound { what } => {
            w.u8(0);
            write_pending_key(w, what)?;
        }
        BreakCause::Ambiguous { what, candidates } => {
            w.u8(1);
            write_pending_key(w, what)?;
            w.u32(count(candidates.len())?);
            for candidate in candidates {
                w.node_id(*candidate);
            }
        }
        BreakCause::KindMismatch { expected, found } => {
            w.u8(2);
            w.str(expected)?;
            w.str(found)?;
        }
        BreakCause::IllegalCycle { through } => {
            w.u8(3);
            w.node_id(*through);
        }
    }
    Ok(())
}

fn write_ref_opt<T>(w: &mut SnapshotWriter, reference: Option<&Ref<T>>) -> Result<(), AstError> {
    match reference {
        Some(reference) => {
            w.bool(true);
            write_ref(w, reference)?;
        }
        None => w.bool(false),
    }
    Ok(())
}

fn write_type_ref_opt(
    w: &mut SnapshotWriter,
    reference: Option<&Ref<crate::TypeNode>>,
) -> Result<(), AstError> {
    write_ref_opt(w, reference)
}

fn read_ref<T>(r: &mut SnapshotReader<'_>) -> Result<Ref<T>, AstError> {
    Ok(match r.u8()? {
        1 => Ref::Resolved(Id::from_parts(r.u32()?, r.u32()?)?),
        2 => Ref::Pending(read_pending_key(r)?),
        3 => Ref::Broken(read_break_cause(r)?),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "reference tag 1, 2 or 3",
            }));
        }
    })
}

fn read_pending_key(r: &mut SnapshotReader<'_>) -> Result<zerodds_ast_core::PendingRef, AstError> {
    use zerodds_ast_core::{
        PendingRef, RepositoryId, RosInterfaceKind, RosTypeName, ScopedName, TypeIdentifierKey,
        XmlRefKey,
    };
    Ok(match r.u8()? {
        0 => {
            let absolute = r.bool()?;
            let segments = read_strings(r)?;
            PendingRef::ScopedName(if absolute {
                ScopedName::absolute(segments)
            } else {
                ScopedName::relative(segments)
            })
        }
        1 => {
            let mut identifier = [0u8; 16];
            for octet in &mut identifier {
                *octet = r.u8()?;
            }
            PendingRef::TypeIdentifier(TypeIdentifierKey(identifier))
        }
        2 => {
            let package = r.string()?;
            let kind = match r.u8()? {
                0 => RosInterfaceKind::Msg,
                1 => RosInterfaceKind::Srv,
                2 => RosInterfaceKind::Action,
                _ => {
                    return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a ROS interface kind tag 0, 1 or 2",
                    }));
                }
            };
            let name = r.string()?;
            PendingRef::RosTypeName(RosTypeName {
                package,
                kind,
                name,
            })
        }
        3 => PendingRef::RepositoryId(RepositoryId(r.string()?)),
        4 => PendingRef::XmlRef(XmlRefKey(r.string()?)),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a lookup key tag 0 to 4",
            }));
        }
    })
}

/// The names `BreakCause::KindMismatch` carries, as the resolver writes
/// them: every node kind's `as_str`, and the three phrases of
/// `crate::resolve`. The cause holds `&'static str`, so a name read back
/// has to be one of these; anything else is refused rather than leaked.
fn kind_mismatch_name(text: &str) -> Option<&'static str> {
    const PHRASES: [&str; 3] = [
        "a different kind of declaration",
        "a declaration the reference accepts",
        "nothing",
    ];
    PHRASES
        .iter()
        .copied()
        .chain(crate::NodeKind::ALL.iter().map(|kind| kind.as_str()))
        .find(|known| *known == text)
}

fn read_break_cause(r: &mut SnapshotReader<'_>) -> Result<zerodds_ast_core::BreakCause, AstError> {
    use zerodds_ast_core::BreakCause;
    Ok(match r.u8()? {
        0 => BreakCause::NotFound {
            what: read_pending_key(r)?,
        },
        1 => {
            let what = read_pending_key(r)?;
            let n = bounded_count(r, 16)? as usize;
            let mut candidates = Vec::new();
            for _ in 0..n {
                candidates.push(r.node_id()?);
            }
            BreakCause::Ambiguous { what, candidates }
        }
        2 => {
            let name = |r: &mut SnapshotReader<'_>| -> Result<&'static str, AstError> {
                let text = r.string()?;
                kind_mismatch_name(&text).ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a kind name the resolver writes",
                }))
            };
            let expected = name(r)?;
            let found = name(r)?;
            BreakCause::KindMismatch { expected, found }
        }
        3 => BreakCause::IllegalCycle {
            through: r.node_id()?,
        },
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a break cause tag 0 to 3",
            }));
        }
    })
}

fn read_ref_opt<T>(r: &mut SnapshotReader<'_>) -> Result<Option<Ref<T>>, AstError> {
    Ok(if r.bool()? { Some(read_ref(r)?) } else { None })
}

fn write_handle_opt<T>(w: &mut SnapshotWriter, handle: Option<Id<T>>) {
    match handle {
        Some(id) => {
            w.bool(true);
            w.u32(id.slot());
            w.u32(id.generation());
        }
        None => w.bool(false),
    }
}

fn read_handle_opt<T>(r: &mut SnapshotReader<'_>) -> Result<Option<Id<T>>, AstError> {
    Ok(if r.bool()? {
        Some(Id::from_parts(r.u32()?, r.u32()?)?)
    } else {
        None
    })
}

fn write_any_parent(w: &mut SnapshotWriter, parent: AnyId) {
    // Only the kinds that can own an operation are recorded; anything
    // else would be a model the edit surface cannot build.
    match parent {
        AnyId::Interface(id, _) => {
            w.u8(1);
            w.u32(id.slot());
            w.u32(id.generation());
        }
        AnyId::Type(id, _) => {
            w.u8(2);
            w.u32(id.slot());
            w.u32(id.generation());
        }
        _ => w.u8(0),
    }
}

fn read_any_parent(r: &mut SnapshotReader<'_>, ast: &Ast) -> Result<AnyId, AstError> {
    match r.u8()? {
        1 => {
            let id: Id<crate::InterfaceNode> = Id::from_parts(r.u32()?, r.u32()?)?;
            let node_id = ast.interfaces.get(id)?.node_id;
            Ok(AnyId::Interface(id, node_id))
        }
        2 => {
            let id: Id<crate::TypeNode> = Id::from_parts(r.u32()?, r.u32()?)?;
            let node_id = ast.types.get(id)?.node_id;
            Ok(AnyId::Type(id, node_id))
        }
        _ => Err(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "an operation's owning interface or valuetype",
        })),
    }
}

fn interface_kind_tag(kind: crate::InterfaceKind) -> u8 {
    use crate::InterfaceKind as K;
    match kind {
        K::Interface => 0,
        K::Local => 1,
        K::Abstract => 2,
        K::Component => 3,
        K::Home => 4,
        K::Connector => 5,
        K::PortType => 6,
    }
}

fn interface_kind_from_tag(
    tag: u8,
    r: &SnapshotReader<'_>,
) -> Result<crate::InterfaceKind, AstError> {
    crate::InterfaceKind::ALL
        .get(tag as usize)
        .copied()
        .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known interface kind tag",
        }))
}

fn direction_tag(direction: crate::ParamDirection) -> u8 {
    use crate::ParamDirection as D;
    match direction {
        D::In => 0,
        D::Out => 1,
        D::InOut => 2,
    }
}

fn direction_from_tag(tag: u8, r: &SnapshotReader<'_>) -> Result<crate::ParamDirection, AstError> {
    crate::ParamDirection::ALL
        .get(tag as usize)
        .copied()
        .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known parameter direction tag",
        }))
}

/// QoS policies, as an opaque count-prefixed list of kind tags.
///
/// The policy payloads are values a frontend re-derives from the source
/// it parsed; the snapshot records which policies a profile declared so
/// the shape survives, and a loader that needs the values re-reads the
/// DDS-XML. Recorded explicitly rather than dropped silently, so a
/// consumer can tell the difference.
fn write_policies(w: &mut SnapshotWriter, policies: &[crate::QosPolicy]) -> Result<(), AstError> {
    w.u32(count(policies.len())?);
    for policy in policies {
        w.u8(policy_tag(policy.kind())?);
    }
    Ok(())
}

/// The snapshot tag of a QoS policy kind.
///
/// # Errors
/// [`AstCoreError::Exhausted`] when the kind is absent from `ALL`, which
/// means a variant was added without extending the table. Writing a
/// stand-in tag would put a different policy in the snapshot and only
/// surface when someone loaded it, on another machine, later.
fn policy_tag(kind: crate::QosPolicyKind) -> Result<u8, AstError> {
    tag_of(crate::QosPolicyKind::ALL.iter().position(|c| *c == kind))
}

/// A variant's index in its `ALL` table, as the tag it is written as.
fn tag_of(position: Option<usize>) -> Result<u8, AstError> {
    position
        .and_then(|index| u8::try_from(index).ok())
        .ok_or(AstError::Core(AstCoreError::Exhausted))
}

fn read_policies(r: &mut SnapshotReader<'_>) -> Result<Vec<crate::QosPolicy>, AstError> {
    let len = bounded_count(r, 1)? as usize;
    let mut out = Vec::new();
    for _ in 0..len {
        let tag = r.u8()?;
        let kind = crate::QosPolicyKind::ALL
            .get(tag as usize)
            .copied()
            .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known QoS policy tag",
            }))?;
        out.push(empty_policy(kind));
    }
    Ok(out)
}

/// A policy of the given kind with every field unset.
///
/// Unset rather than defaulted: the snapshot did not carry the values,
/// and a defaulted field would claim the profile wrote something it did
/// not. Spec §3.7.
fn empty_policy(kind: crate::QosPolicyKind) -> crate::QosPolicy {
    use crate::QosPolicy as P;
    use crate::QosPolicyKind as K;
    match kind {
        K::UserData => P::UserData { value: Vec::new() },
        K::TopicData => P::TopicData { value: Vec::new() },
        K::GroupData => P::GroupData { value: Vec::new() },
        K::Durability => P::Durability { kind: None },
        K::DurabilityService => P::DurabilityService {
            service_cleanup_delay: None,
            history_kind: None,
            history_depth: None,
            max_samples: None,
            max_instances: None,
            max_samples_per_instance: None,
        },
        K::Presentation => P::Presentation {
            access_scope: None,
            coherent_access: None,
            ordered_access: None,
        },
        K::Deadline => P::Deadline { period: None },
        K::LatencyBudget => P::LatencyBudget { duration: None },
        K::Ownership => P::Ownership { kind: None },
        K::OwnershipStrength => P::OwnershipStrength { value: None },
        K::Liveliness => P::Liveliness {
            kind: None,
            lease_duration: None,
        },
        K::TimeBasedFilter => P::TimeBasedFilter {
            minimum_separation: None,
        },
        K::Partition => P::Partition { name: Vec::new() },
        K::Reliability => P::Reliability {
            kind: None,
            max_blocking_time: None,
        },
        K::TransportPriority => P::TransportPriority { value: None },
        K::Lifespan => P::Lifespan { duration: None },
        K::DestinationOrder => P::DestinationOrder { kind: None },
        K::History => P::History {
            kind: None,
            depth: None,
        },
        K::ResourceLimits => P::ResourceLimits {
            max_samples: None,
            max_instances: None,
            max_samples_per_instance: None,
        },
        K::EntityFactory => P::EntityFactory {
            autoenable_created_entities: None,
        },
        K::WriterDataLifecycle => P::WriterDataLifecycle {
            autodispose_unregistered_instances: None,
        },
        K::ReaderDataLifecycle => P::ReaderDataLifecycle {
            autopurge_nowriter_samples_delay: None,
            autopurge_disposed_samples_delay: None,
        },
        K::DataRepresentation => P::DataRepresentation { value: Vec::new() },
        K::TypeConsistencyEnforcement => P::TypeConsistencyEnforcement {
            kind: None,
            ignore_sequence_bounds: None,
            ignore_string_bounds: None,
            ignore_member_names: None,
            prevent_type_widening: None,
            force_type_validation: None,
        },
    }
}

/// The full facet payload.
///
/// Every facet round-trips: the XTypes annotations layout reads, the
/// CORBA repository identity, the ROS 2 naming and role, the DDS-XML
/// provenance, the FlatData properties, the vendor directives and the
/// source origin.
fn write_facets(w: &mut SnapshotWriter, facets: &Facets) -> Result<(), AstError> {
    // Source, always present.
    match &facets.source.origin {
        crate::SourceOrigin::File { path } => {
            w.u8(0);
            w.str(path)?;
        }
        crate::SourceOrigin::Synthetic { reason } => {
            w.u8(1);
            w.str(reason)?;
        }
        crate::SourceOrigin::Builtin => w.u8(2),
    }
    match facets.source.span {
        Some(span) => {
            w.bool(true);
            w.u32(span.start);
            w.u32(span.end);
        }
        None => w.bool(false),
    }
    w.u32(count(facets.source.macro_chain.len())?);
    for expansion in &facets.source.macro_chain {
        w.str(&expansion.name)?;
        match expansion.invoked_at {
            Some(span) => {
                w.bool(true);
                w.u32(span.start);
                w.u32(span.end);
            }
            None => w.bool(false),
        }
        match &expansion.path {
            Some(path) => {
                w.bool(true);
                w.str(path)?;
            }
            None => w.bool(false),
        }
    }
    w.u32(count(facets.source.doc_comments.len())?);
    for comment in &facets.source.doc_comments {
        w.str(comment)?;
    }

    // XTypes.
    match facets.xtypes.as_ref() {
        Some(x) => {
            w.bool(true);
            w.u8(match x.extensibility {
                None => 0,
                Some(crate::Extensibility::Final) => 1,
                Some(crate::Extensibility::Appendable) => 2,
                Some(crate::Extensibility::Mutable) => 3,
            });
            w.bool(x.is_key);
            w.bool(x.is_optional);
            w.bool(x.is_external);
            w.bool(x.is_must_understand);
            w.bool(x.is_non_serialized);
            w.bool(x.is_nested);
            w.bool(x.is_default_literal);
            w.bool(x.discriminator_is_key);
            match x.default_nested {
                None => w.u8(0),
                Some(false) => w.u8(1),
                Some(true) => w.u8(2),
            }
            for behaviour in [
                x.try_construct,
                x.element_try_construct,
                x.key_try_construct,
                x.discriminator_try_construct,
            ] {
                w.u8(try_construct_tag(behaviour));
            }
            w.bool(x.discriminator_is_key);
            match x.member_id.as_ref() {
                Some(assignment) => {
                    w.bool(true);
                    w.u32(assignment.effective);
                    w.u8(match &assignment.origin {
                        crate::MemberIdOrigin::Declared => 0,
                        crate::MemberIdOrigin::Hashed { .. } => 1,
                        crate::MemberIdOrigin::AutoSequential => 2,
                        crate::MemberIdOrigin::AutoHash => 3,
                        crate::MemberIdOrigin::PreviousPlusOne => 4,
                    });
                    if let crate::MemberIdOrigin::Hashed { source_name } = &assignment.origin {
                        w.str(source_name)?;
                    }
                }
                None => w.bool(false),
            }
            match x.bit_bound {
                Some(bound) => {
                    w.bool(true);
                    w.u16(bound);
                }
                None => w.bool(false),
            }
            write_handle_opt(w, x.default_value);
            match x.range.as_ref() {
                Some(range) => {
                    w.bool(true);
                    write_handle_opt(w, range.min);
                    write_handle_opt(w, range.max);
                }
                None => w.bool(false),
            }
            match x.unit.as_deref() {
                Some(unit) => {
                    w.bool(true);
                    w.str(unit)?;
                }
                None => w.bool(false),
            }
            w.u32(count(x.verbatim.len())?);
            for directive in &x.verbatim {
                w.str(&directive.language)?;
                w.u8(verbatim_tag(directive.placement)?);
                w.str(&directive.text)?;
            }
            w.u8(match x.autoid {
                None => 0,
                Some(crate::AutoId::Sequential) => 1,
                Some(crate::AutoId::Hash) => 2,
            });
            match x.topic.as_ref() {
                Some(topic) => {
                    w.bool(true);
                    write_str_opt(&mut *w, topic.name.as_deref())?;
                    write_str_opt(&mut *w, topic.platform.as_deref())?;
                }
                None => w.bool(false),
            }
            match x.data_representation {
                Some(mask) => {
                    w.bool(true);
                    w.bool(mask.xcdr1);
                    w.bool(mask.xcdr2);
                    w.bool(mask.xml);
                }
                None => w.bool(false),
            }
            w.bool(x.ignore_literal_names);
            w.bool(x.nested_explicit);
            w.bool(x.element_is_external);
            w.bool(x.key_is_external);
        }
        None => w.bool(false),
    }

    // CORBA.
    match facets.corba.as_ref() {
        Some(c) => {
            w.bool(true);
            match c.repository_id.as_ref() {
                Some(id) => {
                    w.bool(true);
                    w.str(&id.0)?;
                }
                None => w.bool(false),
            }
            match c.native_mapping.as_deref() {
                Some(mapping) => {
                    w.bool(true);
                    w.str(mapping)?;
                }
                None => w.bool(false),
            }
            w.u8(match c.repository_id_origin {
                None => 0,
                Some(crate::RepositoryIdOrigin::Derived) => 1,
                Some(crate::RepositoryIdOrigin::PragmaPrefix) => 2,
                Some(crate::RepositoryIdOrigin::PragmaVersion) => 3,
                Some(crate::RepositoryIdOrigin::PragmaId) => 4,
            });
            write_component_role(w, c.component_role.as_ref())?;
            w.u32(count(c.template_params.len())?);
            for param in &c.template_params {
                w.str(&param.name)?;
                w.u8(template_param_tag(param.kind)?);
                write_type_ref_opt(w, param.constraint.as_ref())?;
            }
            w.u32(count(c.template_args.len())?);
            for arg in &c.template_args {
                match arg {
                    crate::facet::corba::TemplateArg::Type(reference) => {
                        w.u8(0);
                        write_type_ref(w, reference)?;
                    }
                    crate::facet::corba::TemplateArg::Const(expr) => {
                        w.u8(1);
                        write_handle_opt(w, Some(*expr));
                    }
                }
            }
            w.bool(c.is_service);
            w.u32(count(c.service_arguments.len())?);
            for argument in &c.service_arguments {
                write_str_opt(w, argument.name.as_deref())?;
                write_handle_opt(w, Some(argument.value));
            }
            w.bool(c.is_ami);
            match c.manages.as_ref() {
                Some(manages) => {
                    w.bool(true);
                    write_ref(w, &manages.component)?;
                    write_type_ref_opt(w, manages.primary_key.as_ref())?;
                }
                None => w.bool(false),
            }
            write_strings(w, &c.imports)?;
            write_strings(w, &c.repository_directives)?;
            w.u32(count(c.supports.len())?);
            for interface in &c.supports {
                write_ref(w, interface)?;
            }
            w.u8(match c.state_visibility {
                None => 0,
                Some(crate::StateVisibility::Public) => 1,
                Some(crate::StateVisibility::Private) => 2,
            });
            w.bool(c.is_event_type);
        }
        None => w.bool(false),
    }

    // ROS 2.
    match facets.ros2.as_ref() {
        Some(rf) => {
            w.bool(true);
            w.bool(rf.was_constant);
            write_handle_opt(w, rf.default_value);
            match rf.ros_name.as_ref() {
                Some(name) => {
                    w.bool(true);
                    w.str(&name.package)?;
                    w.u8(match name.kind {
                        zerodds_ast_core::RosInterfaceKind::Msg => 0,
                        zerodds_ast_core::RosInterfaceKind::Srv => 1,
                        zerodds_ast_core::RosInterfaceKind::Action => 2,
                    });
                    w.str(&name.name)?;
                }
                None => w.bool(false),
            }
            w.u8(match rf.bounded_origin {
                None => 0,
                Some(crate::RosBoundedOrigin::BoundedArray) => 1,
                Some(crate::RosBoundedOrigin::FixedArray) => 2,
                Some(crate::RosBoundedOrigin::BoundedString) => 3,
                Some(crate::RosBoundedOrigin::UnboundedArray) => 4,
            });
            w.u8(match rf.interface_role {
                None => 0,
                Some(crate::RosInterfaceRole::Msg) => 1,
                Some(crate::RosInterfaceRole::Request) => 2,
                Some(crate::RosInterfaceRole::Response) => 3,
                Some(crate::RosInterfaceRole::Goal) => 4,
                Some(crate::RosInterfaceRole::Result) => 5,
                Some(crate::RosInterfaceRole::Feedback) => 6,
            });
        }
        None => w.bool(false),
    }

    // DDS-XML.
    match facets.xml.as_ref() {
        Some(x) => {
            w.bool(true);
            w.str(&x.element)?;
            w.u32(count(x.library_path.len())?);
            for segment in &x.library_path {
                w.str(segment)?;
            }
            w.u32(count(x.attributes.len())?);
            for (name, value) in &x.attributes {
                w.str(name)?;
                w.str(value)?;
            }
            w.u32(count(x.unresolved_idrefs.len())?);
            for key in &x.unresolved_idrefs {
                w.str(&key.0)?;
            }
            match x.xsd_type.as_deref() {
                Some(xsd) => {
                    w.bool(true);
                    w.str(xsd)?;
                }
                None => w.bool(false),
            }
        }
        None => w.bool(false),
    }

    // FlatData.
    match facets.flatdata.as_ref() {
        Some(f) => {
            w.bool(true);
            w.bool(f.is_plain);
            w.bool(f.explicit_fixed_layout);
            match f.loan_alignment {
                Some(align) => {
                    w.bool(true);
                    w.u8(align);
                }
                None => w.bool(false),
            }
        }
        None => w.bool(false),
    }

    // Vendor directives, in full: normalising or dropping them loses
    // exactly what a round trip back to that vendor's dialect needs.
    match facets.vendor.rti.as_ref() {
        Some(rti) => {
            w.bool(true);
            write_strings(w, &rti.copy_directives)?;
            write_bool_opt(w, rti.resolve_name);
            write_bool_opt(w, rti.top_level);
            write_strings(w, &rti.keylist)?;
            write_pairs(w, &rti.unrecognised)?;
        }
        None => w.bool(false),
    }
    match facets.vendor.open_dds.as_ref() {
        Some(open_dds) => {
            w.bool(true);
            w.bool(open_dds.dcps_data_type);
            write_strings(w, &open_dds.dcps_data_keys)?;
            write_bool_opt(w, open_dds.supports_zero_copy);
            write_pairs(w, &open_dds.unrecognised)?;
        }
        None => w.bool(false),
    }
    match facets.vendor.fast_dds.as_ref() {
        Some(fast_dds) => {
            w.bool(true);
            write_strings(w, &fast_dds.xtypes_extensions)?;
            write_bool_opt(w, fast_dds.plain_data_type);
            write_pairs(w, &fast_dds.unrecognised)?;
        }
        None => w.bool(false),
    }
    match facets.vendor.core_dx.as_ref() {
        Some(core_dx) => {
            w.bool(true);
            write_strings(w, &core_dx.key_fields)?;
            write_pairs(w, &core_dx.unrecognised)?;
        }
        None => w.bool(false),
    }

    // User-declared annotations, which have no typed field to fall back
    // on: dropping one loses the declaration outright.
    w.u32(count(facets.annotations.len())?);
    for applied in &facets.annotations {
        w.str(&applied.name)?;
        write_ref(w, &applied.definition)?;
        w.u32(count(applied.arguments.len())?);
        for argument in &applied.arguments {
            write_str_opt(w, argument.name.as_deref())?;
            write_handle_opt(w, Some(argument.value));
        }
    }

    Ok(())
}

fn write_strings(w: &mut SnapshotWriter, values: &[String]) -> Result<(), AstError> {
    w.u32(count(values.len())?);
    for value in values {
        w.str(value)?;
    }
    Ok(())
}

fn write_pairs(w: &mut SnapshotWriter, pairs: &[(String, String)]) -> Result<(), AstError> {
    w.u32(count(pairs.len())?);
    for (name, value) in pairs {
        w.str(name)?;
        w.str(value)?;
    }
    Ok(())
}

fn write_bool_opt(w: &mut SnapshotWriter, value: Option<bool>) {
    match value {
        None => w.u8(0),
        Some(false) => w.u8(1),
        Some(true) => w.u8(2),
    }
}

fn write_str_opt(w: &mut SnapshotWriter, value: Option<&str>) -> Result<(), AstError> {
    match value {
        Some(text) => {
            w.bool(true);
            w.str(text)?;
        }
        None => w.bool(false),
    }
    Ok(())
}

/// The snapshot tag of a template parameter kind.
///
/// # Errors
/// As [`policy_tag`]. The old fallback was `0`, which is a valid tag —
/// an unknown kind read back as `TypeName` and nothing said so.
fn template_param_tag(kind: crate::TemplateParamKind) -> Result<u8, AstError> {
    tag_of(
        crate::TemplateParamKind::ALL
            .iter()
            .position(|c| *c == kind),
    )
}

/// A component-model role, written as a tag plus its own fields.
fn write_component_role(
    w: &mut SnapshotWriter,
    role: Option<&crate::ComponentRole>,
) -> Result<(), AstError> {
    let Some(role) = role else {
        w.u8(0);
        return Ok(());
    };
    match role {
        crate::ComponentRole::Facet { name, interface } => {
            w.u8(1);
            w.str(name)?;
            write_ref(w, interface)?;
        }
        crate::ComponentRole::Receptacle {
            name,
            interface,
            multiple,
        } => {
            w.u8(2);
            w.str(name)?;
            write_ref(w, interface)?;
            w.bool(*multiple);
        }
        crate::ComponentRole::EventSource {
            name,
            event,
            publisher,
        } => {
            w.u8(3);
            w.str(name)?;
            write_type_ref(w, event)?;
            w.bool(*publisher);
        }
        crate::ComponentRole::EventSink { name, event } => {
            w.u8(4);
            w.str(name)?;
            write_type_ref(w, event)?;
        }
        crate::ComponentRole::HomeFactory => w.u8(5),
        crate::ComponentRole::HomeFinder => w.u8(6),
        crate::ComponentRole::PortTypeRole => w.u8(7),
        crate::ComponentRole::ConnectorRole => w.u8(8),
    }
    Ok(())
}

/// The snapshot tag of a `@verbatim` placement.
///
/// # Errors
/// As [`policy_tag`]. The old fallback was `0`, a valid tag: an unknown
/// placement read back as `BeginFile`.
fn verbatim_tag(placement: crate::VerbatimPlacement) -> Result<u8, AstError> {
    tag_of(
        crate::VerbatimPlacement::ALL
            .iter()
            .position(|c| *c == placement),
    )
}

/// The tag for one `@try_construct` slot. Zero is absence, which is not
/// `DISCARD`: an absent annotation and an explicit `@try_construct
/// (DISCARD)` behave alike but a regenerated declaration has to say
/// which was written. DDS-XTypes 1.3 §7.3.1.2.1.12.
const fn try_construct_tag(behaviour: Option<crate::TryConstruct>) -> u8 {
    match behaviour {
        None => 0,
        Some(crate::TryConstruct::Discard) => 1,
        Some(crate::TryConstruct::UseDefault) => 2,
        Some(crate::TryConstruct::Trim) => 3,
    }
}

fn read_try_construct(r: &mut SnapshotReader<'_>) -> Result<Option<crate::TryConstruct>, AstError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(crate::TryConstruct::Discard)),
        2 => Ok(Some(crate::TryConstruct::UseDefault)),
        3 => Ok(Some(crate::TryConstruct::Trim)),
        _ => Err(AstError::Core(AstCoreError::MalformedSnapshot {
            offset: r.offset(),
            expected: "a known try-construct tag",
        })),
    }
}

fn read_facets(r: &mut SnapshotReader<'_>) -> Result<Facets, AstError> {
    let mut facets = Facets::synthetic("loaded from a snapshot");

    facets.source.origin = match r.u8()? {
        0 => crate::SourceOrigin::File { path: r.string()? },
        1 => crate::SourceOrigin::Synthetic {
            reason: r.string()?,
        },
        2 => crate::SourceOrigin::Builtin,
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known source-origin tag",
            }));
        }
    };
    facets.source.span = if r.bool()? {
        Some(crate::ByteSpan {
            start: r.u32()?,
            end: r.u32()?,
        })
    } else {
        None
    };
    let expansions = bounded_count(r, 5)? as usize;
    for _ in 0..expansions {
        let name = r.string()?;
        let invoked_at = if r.bool()? {
            Some(crate::ByteSpan {
                start: r.u32()?,
                end: r.u32()?,
            })
        } else {
            None
        };
        let path = if r.bool()? { Some(r.string()?) } else { None };
        facets.source.macro_chain.push(crate::MacroExpansion {
            name,
            invoked_at,
            path,
        });
    }
    let comments = bounded_count(r, 4)? as usize;
    for _ in 0..comments {
        facets.source.doc_comments.push(r.string()?);
    }

    if r.bool()? {
        let x = facets.xtypes_mut();
        x.extensibility = match r.u8()? {
            0 => None,
            1 => Some(crate::Extensibility::Final),
            2 => Some(crate::Extensibility::Appendable),
            3 => Some(crate::Extensibility::Mutable),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known extensibility tag",
                }));
            }
        };
        x.is_key = r.bool()?;
        x.is_optional = r.bool()?;
        x.is_external = r.bool()?;
        x.is_must_understand = r.bool()?;
        x.is_non_serialized = r.bool()?;
        x.is_nested = r.bool()?;
        x.is_default_literal = r.bool()?;
        x.discriminator_is_key = r.bool()?;
        x.default_nested = match r.u8()? {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known @default_nested tag",
                }));
            }
        };
        x.try_construct = read_try_construct(r)?;
        x.element_try_construct = read_try_construct(r)?;
        x.key_try_construct = read_try_construct(r)?;
        x.discriminator_try_construct = read_try_construct(r)?;
        x.discriminator_is_key = r.bool()?;
        if r.bool()? {
            let effective = r.u32()?;
            let origin_tag = r.u8()?;
            let origin = match origin_tag {
                0 => crate::MemberIdOrigin::Declared,
                1 => crate::MemberIdOrigin::Hashed {
                    source_name: r.string()?,
                },
                2 => crate::MemberIdOrigin::AutoSequential,
                3 => crate::MemberIdOrigin::AutoHash,
                4 => crate::MemberIdOrigin::PreviousPlusOne,
                _ => {
                    return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a known member-id origin tag",
                    }));
                }
            };
            x.member_id = Some(crate::MemberIdAssignment { effective, origin });
        }
        if r.bool()? {
            x.bit_bound = Some(r.u16()?);
        }
        x.default_value = read_handle_opt(r)?;
        if r.bool()? {
            x.range = Some(crate::RangeConstraint {
                min: read_handle_opt(r)?,
                max: read_handle_opt(r)?,
            });
        }
        if r.bool()? {
            x.unit = Some(r.string()?);
        }
        let directives = bounded_count(r, 6)? as usize;
        for _ in 0..directives {
            let language = r.string()?;
            let tag = r.u8()?;
            let placement = crate::VerbatimPlacement::ALL
                .get(tag as usize)
                .copied()
                .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known verbatim placement tag",
                }))?;
            let text = r.string()?;
            x.verbatim.push(crate::VerbatimDirective {
                language,
                placement,
                text,
            });
        }
        x.autoid = match r.u8()? {
            0 => None,
            1 => Some(crate::AutoId::Sequential),
            2 => Some(crate::AutoId::Hash),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known autoid tag",
                }));
            }
        };
        if r.bool()? {
            x.topic = Some(crate::TopicAnnotation {
                name: read_str_opt(r)?,
                platform: read_str_opt(r)?,
            });
        }
        if r.bool()? {
            x.data_representation = Some(crate::DataRepresentationMask {
                xcdr1: r.bool()?,
                xcdr2: r.bool()?,
                xml: r.bool()?,
            });
        }
        x.ignore_literal_names = r.bool()?;
        x.nested_explicit = r.bool()?;
        x.element_is_external = r.bool()?;
        x.key_is_external = r.bool()?;
    }

    if r.bool()? {
        let c = facets.corba_mut();
        if r.bool()? {
            c.repository_id = Some(zerodds_ast_core::RepositoryId(r.string()?));
        }
        if r.bool()? {
            c.native_mapping = Some(r.string()?);
        }
        c.repository_id_origin = match r.u8()? {
            0 => None,
            1 => Some(crate::RepositoryIdOrigin::Derived),
            2 => Some(crate::RepositoryIdOrigin::PragmaPrefix),
            3 => Some(crate::RepositoryIdOrigin::PragmaVersion),
            4 => Some(crate::RepositoryIdOrigin::PragmaId),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known repository-id origin tag",
                }));
            }
        };
        c.component_role = read_component_role(r)?;
        let params = bounded_count(r, 8)? as usize;
        for _ in 0..params {
            let name = r.string()?;
            let tag = r.u8()?;
            let kind = crate::TemplateParamKind::ALL
                .get(tag as usize)
                .copied()
                .ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known template-parameter kind tag",
                }))?;
            let constraint = read_ref_opt(r)?;
            c.template_params.push(crate::TemplateParam {
                name,
                kind,
                constraint,
            });
        }
        let args = bounded_count(r, 8)? as usize;
        for _ in 0..args {
            let arg = match r.u8()? {
                0 => crate::facet::corba::TemplateArg::Type(read_type_ref(r)?),
                1 => {
                    let expr = read_handle_opt(r)?.ok_or(AstError::Core(
                        AstCoreError::MalformedSnapshot {
                            offset: r.offset(),
                            expected: "a constant template argument's expression",
                        },
                    ))?;
                    crate::facet::corba::TemplateArg::Const(expr)
                }
                _ => {
                    return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a known template-argument tag",
                    }));
                }
            };
            c.template_args.push(arg);
        }
        c.is_service = r.bool()?;
        let service_argument_count = bounded_count(r, 5)? as usize;
        for _ in 0..service_argument_count {
            let argument_name = read_str_opt(r)?;
            let value =
                read_handle_opt(r)?.ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a `@service` argument's expression",
                }))?;
            c.service_arguments.push(crate::AppliedArgument {
                name: argument_name,
                value,
            });
        }
        c.is_ami = r.bool()?;
        if r.bool()? {
            c.manages = Some(crate::HomeManages {
                component: read_ref(r)?,
                primary_key: read_ref_opt(r)?,
            });
        }
        c.imports = read_strings(r)?;
        c.repository_directives = read_strings(r)?;
        let supports = bounded_count(r, 8)? as usize;
        for _ in 0..supports {
            c.supports.push(read_ref(r)?);
        }
        c.state_visibility = match r.u8()? {
            0 => None,
            1 => Some(crate::StateVisibility::Public),
            2 => Some(crate::StateVisibility::Private),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known state-visibility tag",
                }));
            }
        };
        c.is_event_type = r.bool()?;
    }

    if r.bool()? {
        let rf = facets.ros2_mut();
        rf.was_constant = r.bool()?;
        rf.default_value = read_handle_opt(r)?;
        if r.bool()? {
            let package = r.string()?;
            let kind = match r.u8()? {
                0 => zerodds_ast_core::RosInterfaceKind::Msg,
                1 => zerodds_ast_core::RosInterfaceKind::Srv,
                2 => zerodds_ast_core::RosInterfaceKind::Action,
                _ => {
                    return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                        offset: r.offset(),
                        expected: "a known ROS interface kind tag",
                    }));
                }
            };
            let name = r.string()?;
            rf.ros_name = Some(zerodds_ast_core::RosTypeName {
                package,
                kind,
                name,
            });
        }
        rf.bounded_origin = match r.u8()? {
            0 => None,
            1 => Some(crate::RosBoundedOrigin::BoundedArray),
            2 => Some(crate::RosBoundedOrigin::FixedArray),
            3 => Some(crate::RosBoundedOrigin::BoundedString),
            4 => Some(crate::RosBoundedOrigin::UnboundedArray),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known ROS bounded-origin tag",
                }));
            }
        };
        rf.interface_role = match r.u8()? {
            0 => None,
            1 => Some(crate::RosInterfaceRole::Msg),
            2 => Some(crate::RosInterfaceRole::Request),
            3 => Some(crate::RosInterfaceRole::Response),
            4 => Some(crate::RosInterfaceRole::Goal),
            5 => Some(crate::RosInterfaceRole::Result),
            6 => Some(crate::RosInterfaceRole::Feedback),
            _ => {
                return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "a known ROS interface-role tag",
                }));
            }
        };
    }

    if r.bool()? {
        let x = facets.xml_mut();
        x.element = r.string()?;
        let segments = bounded_count(r, 4)? as usize;
        for _ in 0..segments {
            x.library_path.push(r.string()?);
        }
        let attributes = bounded_count(r, 8)? as usize;
        for _ in 0..attributes {
            let name = r.string()?;
            let value = r.string()?;
            x.attributes.push((name, value));
        }
        let idrefs = bounded_count(r, 4)? as usize;
        for _ in 0..idrefs {
            x.unresolved_idrefs
                .push(zerodds_ast_core::XmlRefKey(r.string()?));
        }
        if r.bool()? {
            x.xsd_type = Some(r.string()?);
        }
    }

    if r.bool()? {
        let f = facets.flatdata_mut();
        f.is_plain = r.bool()?;
        f.explicit_fixed_layout = r.bool()?;
        if r.bool()? {
            f.loan_alignment = Some(r.u8()?);
        }
    }

    if r.bool()? {
        facets.vendor.rti = Some(crate::RtiFacet {
            copy_directives: read_strings(r)?,
            resolve_name: read_bool_opt(r)?,
            top_level: read_bool_opt(r)?,
            keylist: read_strings(r)?,
            unrecognised: read_pairs(r)?,
        });
    }
    if r.bool()? {
        facets.vendor.open_dds = Some(crate::OpenDdsFacet {
            dcps_data_type: r.bool()?,
            dcps_data_keys: read_strings(r)?,
            supports_zero_copy: read_bool_opt(r)?,
            unrecognised: read_pairs(r)?,
        });
    }
    if r.bool()? {
        facets.vendor.fast_dds = Some(crate::FastDdsFacet {
            xtypes_extensions: read_strings(r)?,
            plain_data_type: read_bool_opt(r)?,
            unrecognised: read_pairs(r)?,
        });
    }
    if r.bool()? {
        facets.vendor.core_dx = Some(crate::CoreDxFacet {
            key_fields: read_strings(r)?,
            unrecognised: read_pairs(r)?,
        });
    }

    let applied_count = bounded_count(r, 10)? as usize;
    for _ in 0..applied_count {
        let name = r.string()?;
        let definition = read_ref(r)?;
        let argument_count = bounded_count(r, 5)? as usize;
        let mut arguments = Vec::new();
        for _ in 0..argument_count {
            let argument_name = read_str_opt(r)?;
            let value =
                read_handle_opt(r)?.ok_or(AstError::Core(AstCoreError::MalformedSnapshot {
                    offset: r.offset(),
                    expected: "an annotation argument's expression",
                }))?;
            arguments.push(crate::AppliedArgument {
                name: argument_name,
                value,
            });
        }
        facets.annotations.push(crate::AppliedAnnotation {
            name,
            definition,
            arguments,
        });
    }

    Ok(facets)
}

fn read_strings(r: &mut SnapshotReader<'_>) -> Result<Vec<String>, AstError> {
    let n = bounded_count(r, 4)? as usize;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(r.string()?);
    }
    Ok(out)
}

fn read_pairs(r: &mut SnapshotReader<'_>) -> Result<Vec<(String, String)>, AstError> {
    let n = bounded_count(r, 8)? as usize;
    let mut out = Vec::new();
    for _ in 0..n {
        let name = r.string()?;
        let value = r.string()?;
        out.push((name, value));
    }
    Ok(out)
}

fn read_bool_opt(r: &mut SnapshotReader<'_>) -> Result<Option<bool>, AstError> {
    Ok(match r.u8()? {
        0 => None,
        1 => Some(false),
        2 => Some(true),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known optional-boolean tag",
            }));
        }
    })
}

fn read_str_opt(r: &mut SnapshotReader<'_>) -> Result<Option<String>, AstError> {
    Ok(if r.bool()? { Some(r.string()?) } else { None })
}

/// A component-model role, read back by its tag.
fn read_component_role(
    r: &mut SnapshotReader<'_>,
) -> Result<Option<crate::ComponentRole>, AstError> {
    Ok(match r.u8()? {
        0 => None,
        1 => Some(crate::ComponentRole::Facet {
            name: r.string()?,
            interface: read_ref(r)?,
        }),
        2 => Some(crate::ComponentRole::Receptacle {
            name: r.string()?,
            interface: read_ref(r)?,
            multiple: r.bool()?,
        }),
        3 => Some(crate::ComponentRole::EventSource {
            name: r.string()?,
            event: read_type_ref(r)?,
            publisher: r.bool()?,
        }),
        4 => Some(crate::ComponentRole::EventSink {
            name: r.string()?,
            event: read_type_ref(r)?,
        }),
        5 => Some(crate::ComponentRole::HomeFactory),
        6 => Some(crate::ComponentRole::HomeFinder),
        7 => Some(crate::ComponentRole::PortTypeRole),
        8 => Some(crate::ComponentRole::ConnectorRole),
        _ => {
            return Err(AstError::Core(AstCoreError::MalformedSnapshot {
                offset: r.offset(),
                expected: "a known component-role tag",
            }));
        }
    })
}

impl Ast {
    /// A model with no root and no built-ins, for the snapshot loader to
    /// fill from the file's own content.
    fn empty_for_load() -> Self {
        let mut ast = Self::empty_public();
        ast.modules = zerodds_ast_core::Arena::new();
        ast.annotation_defs = zerodds_ast_core::Arena::new();
        ast.annotation_members = zerodds_ast_core::Arena::new();
        ast.by_node_id.clear();
        ast.model_ids = zerodds_ast_core::IdAllocator::new(IdSpace::MODEL);
        ast.builtin_ids = zerodds_ast_core::IdAllocator::new(IdSpace::BUILTIN);
        ast.synthetic_ids = zerodds_ast_core::IdAllocator::new(IdSpace::SYNTHETIC);
        ast
    }
}

/// The `NodeId` a fresh model gives its root module, so a loaded model
/// can be told apart from one built through the API.
pub const ROOT_MODULE_ID: NodeId = NodeId::from_parts(IdSpace::MODEL, 1);
