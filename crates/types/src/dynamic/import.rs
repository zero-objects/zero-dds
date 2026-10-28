// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A type from one model, into the model of a builder.
//!
//! §7.5.2.2 hands a [`DynamicType`] to `create_type_copy`,
//! `create_sequence_type`, `create_array_type`, `create_map_type` and
//! `add_member`, and the type it names may come from any model: IDL
//! lowered elsewhere, a peer's `TypeObject`, another builder. A builder
//! owns one model and the handles in it are that model's, so the type has
//! to be **copied** in, with everything it reaches.
//!
//! ## What is copied
//!
//! The XTypes information: the kind and the name with its module path,
//! bounds and dimensions (evaluated), members with their ids, flags,
//! defaults, case labels and `@try_construct`, enumerators with their
//! values, bits with their positions, bitfields, extensibility, `@nested`,
//! `@autoid`, `@bit_bound`, `@unit`, `@range`, `@verbatim` and the
//! annotations a user declared with `@annotation`, whose declarations are
//! copied too.
//!
//! What is not: the CORBA, ROS 2, FlatData and vendor facets, which are
//! not part of the type system §7.5.2 describes and which carry handles
//! into the model they were written in, and the provenance of the
//! source file (the copy is a synthetic declaration).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{
    AnnotationDefNode, AnyId, AppliedAnnotation, AppliedArgument, Ast, Bound, CaseLabel, ConstExpr,
    ConstExprNode, ConstValue, Facets, Literal, MemberSpec, ModuleNode, RangeConstraint,
    SourceFacet, TypeKind as AstKind, TypeNode, XTypesFacet,
};
use zerodds_ast_core::{Id, Ref};

use super::builder::DynamicTypeBuilder;
use super::error::DynamicError;
use super::type_::DynamicType;

/// How deep a type is followed before it is taken for a cycle that the
/// memo did not catch. The memo catches every named type; this bounds a
/// chain of anonymous collections.
const MAX_DEPTH: usize = 64;

impl DynamicTypeBuilder {
    /// A copy of `type_` and everything it reaches, in this builder's
    /// model.
    ///
    /// A recursive type is copied once and its self-references point at
    /// the copy.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a reference the source model did
    /// not close; [`DynamicError::Unrepresentable`] for a kind the
    /// Dynamic Language Binding has no shape for (`valuetype`, `fixed`,
    /// `native`, …); [`DynamicError::Model`] where this model refuses an
    /// edit.
    pub fn import(&mut self, type_: &DynamicType) -> Result<Id<TypeNode>, DynamicError> {
        let mut importer = Importer {
            src: type_.ast(),
            memo: Vec::new(),
            defs: Vec::new(),
        };
        importer.type_(self, type_.id(), 0)
    }
}

impl DynamicTypeBuilder {
    /// The declaration of `annotation` in this builder's model: the
    /// built-in annotation of that name, or a copy of the declared one,
    /// made once per source declaration of an `import`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn import_annotation_type(
        &mut self,
        annotation: &super::annotation::AnnotationType,
    ) -> Result<Id<AnnotationDefNode>, DynamicError> {
        if let Some(existing) = self
            .ast
            .annotation_defs()
            .find(|(_, def)| {
                def.name == annotation.name() && def.module != self.ast.builtin_module()
            })
            .map(|(id, _)| id)
        {
            if !annotation.is_builtin() {
                return Ok(existing);
            }
        }
        let mut importer = Importer {
            src: annotation.ast(),
            memo: Vec::new(),
            defs: Vec::new(),
        };
        importer.definition(self, annotation.id())
    }
}

struct Importer<'a> {
    src: &'a Ast,
    memo: Vec<(Id<TypeNode>, Id<TypeNode>)>,
    defs: Vec<(Id<AnnotationDefNode>, Id<AnnotationDefNode>)>,
}

impl Importer<'_> {
    fn type_(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        id: Id<TypeNode>,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        if let Some((_, done)) = self.memo.iter().find(|(from, _)| *from == id) {
            return Ok(*done);
        }
        if depth > MAX_DEPTH {
            return Err(DynamicError::Recursive {
                what: String::from("a chain of collections"),
            });
        }
        let node = self.src.type_(id).map_err(|e| DynamicError::model(&e))?;
        let name = node.name.clone();
        let kind = node.kind.clone();
        let facets = node.facets.clone();
        let module = self.module_of(dst, id)?;
        match kind {
            AstKind::Primitive(primitive) => {
                let made = dst
                    .ast
                    .add_type(
                        dst.ast.root_module(),
                        primitive.as_str(),
                        AstKind::Primitive(primitive),
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.memo.push((id, made));
                Ok(made)
            }
            AstKind::String { width, bound } => {
                let bound = self.bound(dst, &bound)?;
                let made = dst
                    .ast
                    .add_type(
                        dst.ast.root_module(),
                        &name,
                        AstKind::String { width, bound },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.memo.push((id, made));
                Ok(made)
            }
            AstKind::Sequence { element, bound } => {
                let element = self.reference(dst, &element, &name, depth)?;
                let bound = self.bound(dst, &bound)?;
                let made = dst
                    .ast
                    .add_type(
                        dst.ast.root_module(),
                        &name,
                        AstKind::Sequence {
                            element: Ref::Resolved(element),
                            bound,
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                self.memo.push((id, made));
                Ok(made)
            }
            AstKind::Array { element, dims } => {
                let element = self.reference(dst, &element, &name, depth)?;
                let mut out = Vec::with_capacity(dims.len());
                for dim in dims {
                    out.push(self.expr(dst, dim)?);
                }
                let made = dst
                    .ast
                    .add_type(
                        dst.ast.root_module(),
                        &name,
                        AstKind::Array {
                            element: Ref::Resolved(element),
                            dims: out,
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                self.memo.push((id, made));
                Ok(made)
            }
            AstKind::Map { key, value, bound } => {
                let key = self.reference(dst, &key, &name, depth)?;
                let value = self.reference(dst, &value, &name, depth)?;
                let bound = self.bound(dst, &bound)?;
                let made = dst
                    .ast
                    .add_type(
                        dst.ast.root_module(),
                        &name,
                        AstKind::Map {
                            key: Ref::Resolved(key),
                            value: Ref::Resolved(value),
                            bound,
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                self.memo.push((id, made));
                Ok(made)
            }
            AstKind::Alias { aliased } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Alias {
                        aliased: Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                            zerodds_ast_core::ScopedName::absolute(Vec::new()),
                        )),
                    },
                    id,
                )?;
                let target = self.reference(dst, &aliased, &name, depth)?;
                dst.ast
                    .complete_type(
                        made,
                        AstKind::Alias {
                            aliased: Ref::Resolved(target),
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                Ok(made)
            }
            AstKind::Enumeration { enumerators } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Enumeration {
                        enumerators: Vec::new(),
                    },
                    id,
                )?;
                dst.ast
                    .complete_type(
                        made,
                        AstKind::Enumeration {
                            enumerators: Vec::new(),
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                for enumerator in enumerators {
                    let node = self
                        .src
                        .enumerator(enumerator)
                        .map_err(|e| DynamicError::model(&e))?;
                    let value = match node.value {
                        Some(expr) => Some(self.expr(dst, expr)?),
                        None => None,
                    };
                    let literal_facets = node.facets.clone();
                    let literal_name = node.name.clone();
                    let added = dst
                        .ast
                        .add_enumerator(made, &literal_name, value)
                        .map_err(|e| DynamicError::model(&e))?;
                    let fixed = self.facets(dst, &literal_facets)?;
                    let node_id = dst
                        .ast
                        .enumerator(added)
                        .map_err(|e| DynamicError::model(&e))?
                        .node_id;
                    if let Some(slot) = dst.ast.facets_mut(AnyId::Enumerator(added, node_id)) {
                        *slot = fixed;
                    }
                }
                Ok(made)
            }
            AstKind::Bitmask { bits } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Bitmask { bits: Vec::new() },
                    id,
                )?;
                dst.ast
                    .complete_type(made, AstKind::Bitmask { bits: Vec::new() })
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                for bit in bits {
                    let node = self
                        .src
                        .bit_value(bit)
                        .map_err(|e| DynamicError::model(&e))?;
                    let (bit_name, position, bit_facets) =
                        (node.name.clone(), node.position, node.facets.clone());
                    let added = dst
                        .ast
                        .add_bit_value(made, &bit_name, position)
                        .map_err(|e| DynamicError::model(&e))?;
                    let fixed = self.facets(dst, &bit_facets)?;
                    let node_id = dst
                        .ast
                        .bit_value(added)
                        .map_err(|e| DynamicError::model(&e))?
                        .node_id;
                    if let Some(slot) = dst.ast.facets_mut(AnyId::BitValue(added, node_id)) {
                        *slot = fixed;
                    }
                }
                Ok(made)
            }
            AstKind::Bitset { base, fields } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Bitset {
                        base: None,
                        fields: Vec::new(),
                    },
                    id,
                )?;
                let base = match base {
                    Some(base) => Some(Ref::Resolved(self.reference(dst, &base, &name, depth)?)),
                    None => None,
                };
                dst.ast
                    .complete_type(
                        made,
                        AstKind::Bitset {
                            base,
                            fields: Vec::new(),
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                for field in fields {
                    let node = self
                        .src
                        .bitfield(field)
                        .map_err(|e| DynamicError::model(&e))?;
                    let (field_name, bits, holder) =
                        (node.name.clone(), node.bits, node.holder_type.clone());
                    let holder = match holder {
                        Some(holder) => {
                            Some(Ref::Resolved(self.reference(dst, &holder, &name, depth)?))
                        }
                        None => None,
                    };
                    dst.ast
                        .add_bitfield(made, field_name.as_deref(), bits, holder)
                        .map_err(|e| DynamicError::model(&e))?;
                }
                Ok(made)
            }
            AstKind::Structure { base, members } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Structure {
                        base: None,
                        members: Vec::new(),
                    },
                    id,
                )?;
                let base = match base {
                    Some(base) => Some(Ref::Resolved(self.reference(dst, &base, &name, depth)?)),
                    None => None,
                };
                dst.ast
                    .complete_type(
                        made,
                        AstKind::Structure {
                            base,
                            members: Vec::new(),
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                for member in members {
                    let spec = self.member_spec(dst, member, &name, depth)?;
                    dst.ast
                        .add_member(made, spec)
                        .map_err(|e| DynamicError::model(&e))?;
                }
                Ok(made)
            }
            AstKind::Union {
                discriminator,
                cases,
                ..
            } => {
                let made = self.forward(
                    dst,
                    module,
                    &name,
                    AstKind::Union {
                        discriminator: Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                            zerodds_ast_core::ScopedName::absolute(Vec::new()),
                        )),
                        cases: Vec::new(),
                        default_case: None,
                    },
                    id,
                )?;
                let discriminator = self.reference(dst, &discriminator, &name, depth)?;
                dst.ast
                    .complete_type(
                        made,
                        AstKind::Union {
                            discriminator: Ref::Resolved(discriminator),
                            cases: Vec::new(),
                            default_case: None,
                        },
                    )
                    .map_err(|e| DynamicError::model(&e))?;
                self.copy_type_facets(dst, made, &facets)?;
                for case in cases {
                    let node = self.src.case(case).map_err(|e| DynamicError::model(&e))?;
                    let labels = node.labels.clone();
                    let member = node.member;
                    let mut out = Vec::with_capacity(labels.len());
                    for label in labels {
                        out.push(match label {
                            CaseLabel::Value(expr) => CaseLabel::Value(self.expr(dst, expr)?),
                            CaseLabel::Default => CaseLabel::Default,
                        });
                    }
                    let spec = self.member_spec(dst, member, &name, depth)?;
                    dst.ast
                        .add_case(made, out, spec)
                        .map_err(|e| DynamicError::model(&e))?;
                }
                Ok(made)
            }
            other => Err(DynamicError::Unrepresentable {
                what: name,
                kind: unrepresentable(&other),
            }),
        }
    }

    fn forward(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        module: Id<ModuleNode>,
        name: &str,
        placeholder: AstKind,
        from: Id<TypeNode>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let made = dst
            .ast
            .add_forward_type(module, name, placeholder)
            .map_err(|e| DynamicError::model(&e))?;
        self.memo.push((from, made));
        Ok(made)
    }

    /// The module a named type sits in, created in the destination.
    fn module_of(
        &self,
        dst: &mut DynamicTypeBuilder,
        id: Id<TypeNode>,
    ) -> Result<Id<ModuleNode>, DynamicError> {
        let node = self.src.type_(id).map_err(|e| DynamicError::model(&e))?;
        let Some(scoped) = self.src.scoped_name_of(AnyId::Type(id, node.node_id)) else {
            return Ok(dst.ast.root_module());
        };
        let segments = scoped.segments();
        if segments.len() <= 1 {
            return Ok(dst.ast.root_module());
        }
        let path: Vec<&str> = segments[..segments.len() - 1]
            .iter()
            .map(String::as_str)
            .collect();
        dst.module(&path)
    }

    fn reference(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        reference: &Ref<TypeNode>,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let target = reference
            .resolved()
            .ok_or_else(|| DynamicError::Unresolved { at: at.to_string() })?;
        self.type_(dst, target, depth + 1)
    }

    fn bound(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        bound: &Bound,
    ) -> Result<Bound, DynamicError> {
        match bound {
            Bound::Unbounded => Ok(Bound::Unbounded),
            Bound::Const(expr) => Ok(Bound::Const(self.expr(dst, *expr)?)),
        }
    }

    /// An expression, as the literal it evaluates to.
    fn expr(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        expr: Id<ConstExprNode>,
    ) -> Result<Id<ConstExprNode>, DynamicError> {
        let value = self
            .src
            .eval_const(expr)
            .map_err(|_| DynamicError::Unresolved {
                at: String::from("an expression that does not evaluate"),
            })?;
        let literal = match value {
            ConstValue::Boolean(v) => Literal::Boolean(v),
            ConstValue::Integer(v) => Literal::Integer(v),
            ConstValue::Float { mantissa, exponent } => Literal::Float { mantissa, exponent },
            ConstValue::Fixed { digits, scale } => Literal::Fixed { digits, scale },
            ConstValue::Char(v) => Literal::Char(v),
            ConstValue::WChar(v) => Literal::WChar(v),
            ConstValue::String(v) => Literal::String(v),
            ConstValue::WString(v) => Literal::WString(v),
            _ => {
                return Err(DynamicError::Unrepresentable {
                    what: String::from("an expression"),
                    kind: "a value no literal carries",
                });
            }
        };
        dst.ast
            .add_expr(ConstExpr::Literal(literal))
            .map_err(|e| DynamicError::model(&e))
    }

    fn member_spec(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        member: Id<zerodds_ast::MemberNode>,
        owner: &str,
        depth: usize,
    ) -> Result<MemberSpec, DynamicError> {
        let node = self
            .src
            .member(member)
            .map_err(|e| DynamicError::model(&e))?;
        let (name, type_, facets) = (node.name.clone(), node.type_.clone(), node.facets.clone());
        let at = format!("{owner}::{name}");
        let type_ = self.reference(dst, &type_, &at, depth)?;
        let mut spec = MemberSpec::new(&name, Ref::Resolved(type_));
        spec.facets = self.facets(dst, &facets)?;
        Ok(spec)
    }

    fn copy_type_facets(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        made: Id<TypeNode>,
        facets: &Facets,
    ) -> Result<(), DynamicError> {
        let fixed = self.facets(dst, facets)?;
        let any = dst
            .ast
            .any_of_type(made)
            .map_err(|e| DynamicError::model(&e))?;
        if let Some(slot) = dst.ast.facets_mut(any) {
            *slot = fixed;
        }
        Ok(())
    }

    /// The XTypes facet and the declared annotations of a node, with
    /// every handle replaced by one of the destination.
    fn facets(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        facets: &Facets,
    ) -> Result<Facets, DynamicError> {
        let mut out = Facets {
            source: SourceFacet::synthetic("copied into a DynamicTypeBuilder"),
            ..Facets::default()
        };
        if let Some(x) = &facets.xtypes {
            let mut x: XTypesFacet = x.clone();
            if let Some(expr) = x.default_value {
                x.default_value = Some(self.expr(dst, expr)?);
            }
            if let Some(range) = x.range {
                x.range = Some(RangeConstraint {
                    min: match range.min {
                        Some(expr) => Some(self.expr(dst, expr)?),
                        None => None,
                    },
                    max: match range.max {
                        Some(expr) => Some(self.expr(dst, expr)?),
                        None => None,
                    },
                });
            }
            x.element_annotations = self.applied_all(dst, &x.element_annotations)?;
            x.key_annotations = self.applied_all(dst, &x.key_annotations)?;
            x.discriminator_annotations = self.applied_all(dst, &x.discriminator_annotations)?;
            out.xtypes = Some(x);
        }
        out.annotations = self.applied_all(dst, &facets.annotations)?;
        Ok(out)
    }

    fn applied_all(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        list: &[AppliedAnnotation],
    ) -> Result<Vec<AppliedAnnotation>, DynamicError> {
        let mut out = Vec::with_capacity(list.len());
        for applied in list {
            let definition = match applied.definition.resolved() {
                Some(def) => Ref::Resolved(self.definition(dst, def)?),
                None => applied.definition.clone(),
            };
            let mut arguments = Vec::with_capacity(applied.arguments.len());
            for argument in &applied.arguments {
                arguments.push(AppliedArgument {
                    name: argument.name.clone(),
                    value: self.expr(dst, argument.value)?,
                });
            }
            out.push(AppliedAnnotation {
                name: applied.name.clone(),
                definition,
                arguments,
            });
        }
        Ok(out)
    }

    /// The destination's declaration of an annotation: the built-in one by
    /// name, a declared one copied once.
    fn definition(
        &mut self,
        dst: &mut DynamicTypeBuilder,
        def: Id<AnnotationDefNode>,
    ) -> Result<Id<AnnotationDefNode>, DynamicError> {
        if let Some((_, done)) = self.defs.iter().find(|(from, _)| *from == def) {
            return Ok(*done);
        }
        let node = self
            .src
            .annotation_def(def)
            .map_err(|e| DynamicError::model(&e))?;
        let name = node.name.clone();
        if node.module == self.src.builtin_module() {
            if let Some(builtin) = dst.ast.builtin_annotation(&name) {
                self.defs.push((def, builtin));
                return Ok(builtin);
            }
        }
        let members = node.members.clone();
        let made = dst
            .ast
            .add_annotation_def(dst.ast.root_module(), &name)
            .map_err(|e| DynamicError::model(&e))?;
        self.defs.push((def, made));
        for member in members {
            let node = self
                .src
                .annotation_member(member)
                .map_err(|e| DynamicError::model(&e))?;
            let (member_name, member_type, default) =
                (node.name.clone(), node.type_.clone(), node.default);
            let member_type = match member_type.resolved() {
                Some(id) => Ref::Resolved(self.type_(dst, id, 1)?),
                None => member_type,
            };
            let default = match default {
                Some(expr) => Some(self.expr(dst, expr)?),
                None => None,
            };
            dst.ast
                .add_annotation_member(made, &member_name, member_type, default)
                .map_err(|e| DynamicError::model(&e))?;
        }
        Ok(made)
    }
}

const fn unrepresentable(kind: &AstKind) -> &'static str {
    match kind {
        AstKind::Fixed { .. } => {
            "a fixed-point type, which the Dynamic Language Binding does not have"
        }
        AstKind::Valuetype { .. } | AstKind::ValueBox { .. } | AstKind::ValueBase => {
            "a value type, which the Dynamic Language Binding does not have"
        }
        _ => "a kind the Dynamic Language Binding has no shape for",
    }
}
