// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicTypeBuilderFactory` and `DynamicTypeBuilder`. XTypes 1.3
//! §7.5.2.2 and §7.5.2.9, Tables 48 and 56.
//!
//! [`super::DynamicTypeBuilder`] is a factory over **one shared model**
//! (see its module note for why); §7.5.2 has the factory create one
//! builder per type, each a transitional state of a single type. This
//! module is that shape on top: [`DynamicTypeBuilderFactory`] creates
//! [`TypeBuilder`]s, a [`TypeBuilder`] owns a model of its own, and a type
//! named by a [`DynamicType`] argument is **copied in** with everything it
//! reaches ([`super::DynamicTypeBuilder::import`]). [`TypeBuilder::build`]
//! hands out a type over a snapshot of the model, so the builder can go
//! on being edited without the types it built changing under their
//! holders.
//!
//! ## Which operations
//!
//! `create_type_w_uri` and `create_type_w_document` read an XML Type
//! Representation, which this crate does not parse; `zerodds-xml-compose`
//! provides them.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_ast::{
    AnnotationDefNode, Ast, AutoId, ConstExpr, Extensibility, Literal, MemberIdAssignment,
    MemberIdOrigin, TryConstruct, TypeKind as AstKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

use crate::resolve::TypeRegistry;
use crate::type_object::TypeObject;

use super::annotation::{AnnotationDescriptor, AnnotationType, ValueKind, value_kind};
use super::builder::{DynamicTypeBuilder, NewMember};
use super::descriptor::{MEMBER_ID_INVALID, MemberDescriptor, MemberId, TypeDescriptor, TypeKind};
use super::error::DynamicError;
use super::to_ast::ModelFromTypeObjects;
use super::type_::DynamicType;

/// The factory of §7.5.2.2: "logically a singleton (although it need not
/// technically be a singleton in practice)". It holds no state, so every
/// instance behaves as every other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DynamicTypeBuilderFactory;

impl DynamicTypeBuilderFactory {
    /// Table 48 `get_instance`.
    #[must_use]
    pub const fn get_instance() -> Self {
        Self
    }

    /// §7.5.2.2.9 `delete_instance`: there is nothing to reclaim, so it
    /// succeeds; `get_instance` is legal afterwards (§7.5.2.2.11).
    ///
    /// # Errors
    /// None: `RETCODE_ERROR` is for "an implementation-specific reason",
    /// and there is none.
    pub const fn delete_instance() -> Result<(), DynamicError> {
        Ok(())
    }

    /// §7.5.2.2.12 `get_primitive_type`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a kind that is not a primitive.
    pub fn get_primitive_type(&self, kind: TypeKind) -> Result<DynamicType, DynamicError> {
        if !kind.is_primitive() {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "{} is no primitive type kind",
                kind.as_str()
            )));
        }
        let mut builder = DynamicTypeBuilder::new();
        let id = builder.primitive(kind)?;
        builder.finish_as(id)
    }

    /// §7.5.2.2.6 `create_type`: a builder for the type `descriptor`
    /// describes.
    ///
    /// The conventional way to create structured, enumerated and alias
    /// types (the clause); the other kinds work too, except an annotation
    /// type: an `@annotation` declaration is not a `TypeNode` of this
    /// model, and [`Self::create_annotation_type`] builds it.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an inconsistent descriptor
    /// ("this operation shall fail and return a nil value");
    /// [`DynamicError::Unrepresentable`] for an annotation type.
    pub fn create_type(&self, descriptor: &TypeDescriptor) -> Result<TypeBuilder, DynamicError> {
        descriptor
            .is_consistent()
            .map_err(|error| DynamicError::bad_parameter(error.to_string()))?;
        if descriptor.name.is_empty() && !descriptor.kind.is_primitive() {
            // Anonymous collections and strings carry a spelling, not a
            // name; every other kind needs one.
            let anonymous = matches!(
                descriptor.kind,
                TypeKind::String8
                    | TypeKind::String16
                    | TypeKind::Sequence
                    | TypeKind::Array
                    | TypeKind::Map
            );
            if !anonymous {
                return Err(DynamicError::bad_parameter(
                    "a type of this kind needs a name",
                ));
            }
        }
        let mut inner = DynamicTypeBuilder::new();
        let root = declare_descriptor(&mut inner, descriptor)?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.6 `create_type` for the kind `ANNOTATION_TYPE`
    /// (§7.5.2.3.4, Annex C): a builder for an annotation type named
    /// `name`, to which [`AnnotationTypeBuilder::add_member`] adds the
    /// attributes. [`AnnotationTypeBuilder::build`] gives the
    /// [`AnnotationType`] an [`AnnotationDescriptor`] is an application of,
    /// and [`TypeBuilder::apply_annotation`] applies it.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an empty name or the name of a
    /// built-in annotation.
    pub fn create_annotation_type(
        &self,
        name: &str,
    ) -> Result<AnnotationTypeBuilder, DynamicError> {
        if name.is_empty() {
            return Err(DynamicError::bad_parameter(
                "an annotation type needs a name",
            ));
        }
        let mut inner = DynamicTypeBuilder::new();
        if inner.model().builtin_annotation(name).is_some() {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "`@{name}` is a built-in annotation"
            )));
        }
        let root = inner.ast.root_module();
        let id = inner
            .ast
            .add_annotation_def(root, name)
            .map_err(|error| DynamicError::model(&error))?;
        Ok(AnnotationTypeBuilder { inner, id })
    }

    /// §7.5.2.2.7 `create_type_copy`: a builder with a copy of the state
    /// of `type_`. Changes to the builder do not reach `type_`.
    ///
    /// # Errors
    /// As [`DynamicTypeBuilder::import`].
    pub fn create_type_copy(&self, type_: &DynamicType) -> Result<TypeBuilder, DynamicError> {
        let mut inner = DynamicTypeBuilder::new();
        let root = inner.import(type_)?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.8 `create_type_w_type_object`: a builder for a type
    /// identical to the one `type_object` describes. The registry holds
    /// the objects it names.
    ///
    /// # Errors
    /// [`DynamicError::MinimalHasNoNames`] for a minimal object; the
    /// errors of [`ModelFromTypeObjects::add_type_object`].
    pub fn create_type_w_type_object(
        &self,
        type_object: &TypeObject,
        registry: &TypeRegistry,
    ) -> Result<TypeBuilder, DynamicError> {
        let mut model = ModelFromTypeObjects::new(registry);
        let root = model.add_type_object(type_object)?;
        Ok(TypeBuilder {
            inner: DynamicTypeBuilder::over(model.into_model()),
            root,
        })
    }

    /// §7.5.2.2.5 `create_string_type`. `LENGTH_UNLIMITED` is unbounded.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn create_string_type(&self, bound: u32) -> Result<TypeBuilder, DynamicError> {
        let mut inner = DynamicTypeBuilder::new();
        let root = inner.string(false, Some(bound))?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.5 `create_wstring_type`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn create_wstring_type(&self, bound: u32) -> Result<TypeBuilder, DynamicError> {
        let mut inner = DynamicTypeBuilder::new();
        let root = inner.string(true, Some(bound))?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.4 `create_sequence_type`.
    ///
    /// # Errors
    /// As [`DynamicTypeBuilder::import`].
    pub fn create_sequence_type(
        &self,
        element_type: &DynamicType,
        bound: u32,
    ) -> Result<TypeBuilder, DynamicError> {
        let mut inner = DynamicTypeBuilder::new();
        let element = inner.import(element_type)?;
        let root = inner.sequence(element, Some(bound))?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.1 `create_array_type`: one bound per dimension, outermost
    /// first.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for no bound (the clause: "If this
    /// argument is nil, the operation shall fail with
    /// RETCODE_BAD_PARAMETER"); as [`DynamicTypeBuilder::import`].
    pub fn create_array_type(
        &self,
        element_type: &DynamicType,
        bound: &[u32],
    ) -> Result<TypeBuilder, DynamicError> {
        if bound.is_empty() {
            return Err(DynamicError::bad_parameter("an array needs a bound"));
        }
        let mut inner = DynamicTypeBuilder::new();
        let element = inner.import(element_type)?;
        let root = inner.array(element, bound)?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.3 `create_map_type`.
    ///
    /// # Errors
    /// As [`DynamicTypeBuilder::import`].
    pub fn create_map_type(
        &self,
        key_element_type: &DynamicType,
        element_type: &DynamicType,
        bound: u32,
    ) -> Result<TypeBuilder, DynamicError> {
        let mut inner = DynamicTypeBuilder::new();
        let key = inner.import(key_element_type)?;
        let value = inner.import(element_type)?;
        let root = inner.map(key, value, Some(bound))?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.2 `create_bitmask_type`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a bound outside 1 to 64.
    pub fn create_bitmask_type(&self, bound: u32) -> Result<TypeBuilder, DynamicError> {
        let bound = u16::try_from(bound)
            .ok()
            .filter(|bound| (1..=64).contains(bound))
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "a bitmask bound of {bound} is outside 1 to 64"
                ))
            })?;
        let mut inner = DynamicTypeBuilder::new();
        let root = inner.bitmask("", Some(bound))?;
        Ok(TypeBuilder { inner, root })
    }

    /// §7.5.2.2.10 `delete_type`: the type is dropped when its last
    /// holder drops it, so there is nothing to do and nothing can already
    /// be deleted; the call succeeds, as it does for the primitive types
    /// and for a nil type.
    ///
    /// # Errors
    /// None.
    pub fn delete_type(&self, type_: DynamicType) -> Result<(), DynamicError> {
        drop(type_);
        Ok(())
    }
}

/// The declaration of the type a descriptor describes, in `b`.
fn declare_descriptor(
    b: &mut DynamicTypeBuilder,
    descriptor: &TypeDescriptor,
) -> Result<Id<TypeNode>, DynamicError> {
    let element = |b: &mut DynamicTypeBuilder| -> Result<Id<TypeNode>, DynamicError> {
        let element = descriptor
            .element_type
            .as_ref()
            .ok_or_else(|| DynamicError::bad_parameter("the descriptor has no element type"))?;
        b.import(element)
    };
    let bound = descriptor.bound.first().copied();
    let id = match descriptor.kind {
        kind if kind.is_primitive() => return b.primitive(kind),
        TypeKind::String8 => b.string(false, bound)?,
        TypeKind::String16 => b.string(true, bound)?,
        TypeKind::Sequence => {
            let element = element(b)?;
            b.sequence(element, bound)?
        }
        TypeKind::Array => {
            let element = element(b)?;
            b.array(element, &descriptor.bound)?
        }
        TypeKind::Map => {
            let key = descriptor
                .key_element_type
                .as_ref()
                .ok_or_else(|| DynamicError::bad_parameter("the descriptor has no key type"))?;
            let key = b.import(key)?;
            let value = element(b)?;
            b.map(key, value, bound)?
        }
        TypeKind::Alias => {
            let target = descriptor
                .base_type
                .as_ref()
                .ok_or_else(|| DynamicError::bad_parameter("an alias needs a base type"))?;
            let target = b.import(target)?;
            named(
                b,
                &descriptor.name,
                AstKind::Alias {
                    aliased: Ref::Resolved(target),
                },
            )?
        }
        TypeKind::Structure => {
            let base = match &descriptor.base_type {
                Some(base) => Some(Ref::Resolved(b.import(base)?)),
                None => None,
            };
            named(
                b,
                &descriptor.name,
                AstKind::Structure {
                    base,
                    members: Vec::new(),
                },
            )?
        }
        TypeKind::Union => {
            let discriminator = descriptor
                .discriminator_type
                .as_ref()
                .ok_or_else(|| DynamicError::bad_parameter("a union needs a discriminator type"))?;
            let discriminator = b.import(discriminator)?;
            named(
                b,
                &descriptor.name,
                AstKind::Union {
                    discriminator: Ref::Resolved(discriminator),
                    cases: Vec::new(),
                    default_case: None,
                },
            )?
        }
        TypeKind::Enumeration => named(
            b,
            &descriptor.name,
            AstKind::Enumeration {
                enumerators: Vec::new(),
            },
        )?,
        TypeKind::Bitmask => named(b, &descriptor.name, AstKind::Bitmask { bits: Vec::new() })?,
        TypeKind::Bitset => {
            let base = match &descriptor.base_type {
                Some(base) => Some(Ref::Resolved(b.import(base)?)),
                None => None,
            };
            named(
                b,
                &descriptor.name,
                AstKind::Bitset {
                    base,
                    fields: Vec::new(),
                },
            )?
        }
        TypeKind::Annotation => {
            return Err(DynamicError::Unrepresentable {
                what: descriptor.name.clone(),
                kind: "an annotation type (create_annotation_type builds it)",
            });
        }
        TypeKind::NoType => {
            return Err(DynamicError::Unrepresentable {
                what: descriptor.name.clone(),
                kind: "a kind that is no type of this model",
            });
        }
        _ => {
            return Err(DynamicError::Unrepresentable {
                what: descriptor.name.clone(),
                kind: "a kind this build has no declaration for",
            });
        }
    };
    let any = b
        .ast
        .any_of_type(id)
        .map_err(|error| DynamicError::model(&error))?;
    if let Some(facets) = b.ast.facets_mut(any) {
        let x = facets.xtypes_mut();
        if let Some(extensibility) = descriptor.extensibility_kind {
            if matches!(
                descriptor.kind,
                TypeKind::Structure | TypeKind::Union | TypeKind::Enumeration | TypeKind::Bitmask
            ) {
                x.extensibility = Some(Extensibility::from(extensibility));
            }
        }
        if descriptor.is_nested {
            x.is_nested = true;
            x.nested_explicit = true;
        }
        if let Some(bit_bound) = descriptor.bit_bound {
            x.bit_bound = Some(bit_bound);
        }
        if matches!(descriptor.kind, TypeKind::Bitmask) {
            if let Some(bound) = descriptor.bound.first() {
                x.bit_bound = u16::try_from(*bound).ok();
            }
        }
    }
    Ok(id)
}

/// A named type in the module its qualified name gives.
fn named(
    b: &mut DynamicTypeBuilder,
    qualified: &str,
    kind: AstKind,
) -> Result<Id<TypeNode>, DynamicError> {
    let segments: Vec<&str> = qualified
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect();
    let Some((name, path)) = segments.split_last() else {
        return Err(DynamicError::bad_parameter("a type needs a name"));
    };
    let module = b.module(path)?;
    b.ast
        .add_type(module, name, kind)
        .map_err(|error| DynamicError::model(&error))
}

/// A type in progress. §7.5.2.9, Table 56.
///
/// "A DynamicTypeBuilder object represents a transitional state of a
/// particular type defined according to the Type System. It is used to
/// instantiate concrete DynamicType objects."
pub struct TypeBuilder {
    inner: DynamicTypeBuilder,
    root: Id<TypeNode>,
}

impl TypeBuilder {
    /// §7.5.2.9 `build`: the type as it stands, over a snapshot of the
    /// model. Later edits do not reach it.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the snapshot cannot be taken.
    pub fn build(&self) -> Result<DynamicType, DynamicError> {
        let bytes = self
            .inner
            .model()
            .to_snapshot()
            .map_err(|error| DynamicError::model(&error))?;
        let ast = Ast::from_snapshot(&bytes).map_err(|error| DynamicError::model(&error))?;
        Ok(DynamicType::new(Arc::new(ast), self.root))
    }

    /// The handle of the type in this builder's model.
    #[must_use]
    pub const fn id(&self) -> Id<TypeNode> {
        self.root
    }

    /// §7.5.2.9.8 `get_descriptor`.
    ///
    /// # Errors
    /// As [`Self::build`] and [`DynamicType::descriptor`].
    pub fn get_descriptor(&self) -> Result<TypeDescriptor, DynamicError> {
        self.build()?.descriptor()
    }

    /// §7.5.2.9.17 `get_name`: the fully qualified name.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_name(&self) -> Result<String, DynamicError> {
        Ok(self.build()?.qualified_name())
    }

    /// §7.5.2.9.12 `get_kind`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_kind(&self) -> Result<TypeKind, DynamicError> {
        Ok(self.build()?.kind())
    }

    /// §7.5.2.9.16 `get_member_count`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_member_count(&self) -> Result<u32, DynamicError> {
        Ok(self.build()?.member_count())
    }

    /// The type as it stands, for the lookups below and for any other
    /// query of a type.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn current(&self) -> Result<DynamicType, DynamicError> {
        self.build()
    }

    /// §7.5.2.9.13 `get_member`: the member with that id.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an id no member has.
    pub fn get_member(&self, id: MemberId) -> Result<super::DynamicTypeMember, DynamicError> {
        self.build()?.get_member(id)
    }

    /// §7.5.2.9.15 `get_member_by_name`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a name no member has.
    pub fn get_member_by_name(&self, name: &str) -> Result<super::DynamicTypeMember, DynamicError> {
        self.build()?.get_member_by_name(name)
    }

    /// §7.5.2.9.14 `get_member_by_index`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an index past the last member.
    pub fn get_member_by_index(
        &self,
        index: u32,
    ) -> Result<super::DynamicTypeMember, DynamicError> {
        self.build()?.get_member_by_index(index)
    }

    /// §7.5.2.9.3 `member_by_name`, whole.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn all_members_by_name(
        &self,
    ) -> Result<alloc::collections::BTreeMap<String, super::DynamicTypeMember>, DynamicError> {
        Ok(self.build()?.all_members_by_name())
    }

    /// §7.5.2.9.2 `member`, whole.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn all_members_by_id(
        &self,
    ) -> Result<alloc::collections::BTreeMap<MemberId, super::DynamicTypeMember>, DynamicError>
    {
        Ok(self.build()?.all_members_by_id())
    }

    /// Annex C `DynamicTypeBuilder::get_all_members_by_name`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_all_members_by_name(
        &self,
    ) -> Result<alloc::collections::BTreeMap<String, super::DynamicTypeMember>, DynamicError> {
        self.all_members_by_name()
    }

    /// Annex C `DynamicTypeBuilder::get_all_members`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_all_members(
        &self,
    ) -> Result<alloc::collections::BTreeMap<MemberId, super::DynamicTypeMember>, DynamicError>
    {
        self.all_members_by_id()
    }

    /// §7.5.2.9.10 `get_annotation_count`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn get_annotation_count(&self) -> Result<u32, DynamicError> {
        Ok(self.build()?.annotation_count())
    }

    /// §7.5.2.9.11 `get_annotation`.
    ///
    /// # Errors
    /// As [`Self::build`]; [`DynamicError::BadParameter`] past the end.
    pub fn get_annotation(&self, index: u32) -> Result<AnnotationDescriptor, DynamicError> {
        self.build()?.annotation(index)
    }

    /// §7.5.2.9.9 `equals`: the type as it stands equals `other`.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn equals(&self, other: &DynamicType) -> Result<bool, DynamicError> {
        Ok(self.build()?.equals(other))
    }

    /// §7.5.2.9.4 `add_member` (Table 55): the descriptor has to describe
    /// what the kind of the type takes.
    ///
    /// - A structure or union member: a consistent member. A union member
    ///   carries its labels or is the default one.
    /// - An enumeration literal: a descriptor whose type is the
    ///   enumeration itself. The literal's value is
    ///   [`MemberDescriptor::default_value`] where it is not empty and
    ///   positional otherwise.
    /// - A bitmask flag: a Boolean descriptor. The flag's position is
    ///   [`MemberDescriptor::default_value`] where it is not empty, and
    ///   has to be inside the bound.
    /// - An alias: as for the type the alias chain ends at; a base type
    ///   that takes no member is `RETCODE_PRECONDITION_NOT_MET`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] where the descriptor does not
    /// satisfy the constraint of the kind;
    /// [`DynamicError::PreconditionNotMet`] for an alias whose base type
    /// takes no member.
    pub fn add_member(&mut self, descriptor: &MemberDescriptor) -> Result<(), DynamicError> {
        let target = self.target()?;
        let kind = self.kind_of(target)?;
        match kind {
            TypeKind::Structure | TypeKind::Union | TypeKind::Annotation => {
                descriptor
                    .is_consistent_for_add(kind)
                    .map_err(|error| DynamicError::bad_parameter(error.to_string()))?;
                let member = self.new_member(descriptor)?;
                let index = descriptor.index as usize;
                if kind == TypeKind::Union {
                    self.inner.add_case_at(
                        target,
                        index,
                        &descriptor.label,
                        descriptor.is_default_label,
                        &member,
                    )
                } else if kind == TypeKind::Structure {
                    self.inner.add_member_at(target, index, &member)
                } else {
                    Err(DynamicError::Unrepresentable {
                        what: descriptor.name.clone(),
                        kind: "a member of an annotation type, which is no type of this model",
                    })
                }
            }
            TypeKind::Enumeration => {
                if descriptor.type_.kind() != TypeKind::Enumeration {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "`{}` is not a literal with the type of the enumeration",
                        descriptor.name
                    )));
                }
                let value = self.literal_value(descriptor, i32::MIN.into(), i32::MAX.into())?;
                let value = value.map(i32::try_from).transpose().map_err(|_| {
                    DynamicError::bad_parameter("an enumerator value outside 32 bits")
                })?;
                self.inner
                    .add_enumerator(target, &descriptor.name, value, false)
            }
            TypeKind::Bitmask => {
                if descriptor.type_.kind() != TypeKind::Boolean {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "`{}` is not a Boolean flag",
                        descriptor.name
                    )));
                }
                let bound = self
                    .build()?
                    .descriptor()?
                    .bound
                    .first()
                    .copied()
                    .unwrap_or(32);
                let position = self.literal_value(descriptor, 0, i64::from(bound) - 1)?;
                let position = position.map(u16::try_from).transpose().map_err(|_| {
                    DynamicError::bad_parameter("a flag position outside the bound")
                })?;
                self.inner.add_bit(target, &descriptor.name, position)
            }
            other => Err(DynamicError::precondition_not_met(alloc::format!(
                "a member cannot be added to {}",
                other.as_str()
            ))),
        }
    }

    /// §7.5.2.9.5 `apply_annotation` to the type.
    ///
    /// The built-in annotations the model keeps in typed fields are set
    /// there; a declared annotation is copied in with its declaration.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an inconsistent descriptor or a
    /// built-in annotation that does not apply to a type;
    /// [`DynamicError::Unrepresentable`] for a built-in annotation this
    /// does not set.
    pub fn apply_annotation(
        &mut self,
        descriptor: &AnnotationDescriptor,
    ) -> Result<(), DynamicError> {
        let any = self
            .inner
            .model()
            .any_of_type(self.root)
            .map_err(|error| DynamicError::model(&error))?;
        self.apply(any, descriptor)
    }

    /// §7.5.2.9.6 `apply_annotation_to_member`.
    ///
    /// # Errors
    /// As [`Self::apply_annotation`]; [`DynamicError::BadParameter`] when
    /// the type has no member of that id.
    pub fn apply_annotation_to_member(
        &mut self,
        member_id: MemberId,
        descriptor: &AnnotationDescriptor,
    ) -> Result<(), DynamicError> {
        let target = self.target()?;
        let ast = self.inner.model();
        let node = ast
            .type_(target)
            .map_err(|error| DynamicError::model(&error))?;
        let candidates: Vec<Id<zerodds_ast::MemberNode>> = match &node.kind {
            AstKind::Union { cases, .. } => cases
                .iter()
                .filter_map(|case| ast.case(*case).ok().map(|case| case.member))
                .collect(),
            _ => ast.all_members_of(target),
        };
        let found = candidates.iter().enumerate().find_map(|(index, member)| {
            let node = ast.member(*member).ok()?;
            let id = node
                .facets
                .xtypes
                .as_ref()
                .and_then(zerodds_ast::XTypesFacet::effective_member_id)
                .unwrap_or_else(|| u32::try_from(index).unwrap_or(u32::MAX));
            (id == member_id).then(|| ast.any_of_member(*member))
        });
        let any = found
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!("no member with the id {member_id}"))
            })?
            .map_err(|error| DynamicError::model(&error))?;
        self.apply(any, descriptor)
    }

    fn target(&self) -> Result<Id<TypeNode>, DynamicError> {
        let mut current = self.root;
        for _ in 0..64 {
            let node = self
                .inner
                .model()
                .type_(current)
                .map_err(|error| DynamicError::model(&error))?;
            let AstKind::Alias { aliased } = &node.kind else {
                return Ok(current);
            };
            current = aliased.resolved().ok_or_else(|| DynamicError::Unresolved {
                at: node.name.clone(),
            })?;
        }
        Err(DynamicError::Recursive {
            what: String::from("an alias chain"),
        })
    }

    fn kind_of(&self, id: Id<TypeNode>) -> Result<TypeKind, DynamicError> {
        // The kind of the node, through the view every other query uses.
        let bytes = self
            .inner
            .model()
            .to_snapshot()
            .map_err(|error| DynamicError::model(&error))?;
        let ast = Ast::from_snapshot(&bytes).map_err(|error| DynamicError::model(&error))?;
        Ok(DynamicType::new(Arc::new(ast), id).kind())
    }

    fn new_member(&mut self, descriptor: &MemberDescriptor) -> Result<NewMember, DynamicError> {
        let type_ = self.inner.import(&descriptor.type_)?;
        let mut member = NewMember::new(&descriptor.name, type_);
        if descriptor.id != MEMBER_ID_INVALID {
            member = member.with_id(descriptor.id);
        }
        member.is_key = descriptor.is_key;
        member.is_optional = descriptor.is_optional;
        member.is_must_understand = descriptor.is_must_understand;
        member.is_external = descriptor.is_shared;
        let behaviour = TryConstruct::from(descriptor.try_construct_kind);
        if behaviour != TryConstruct::ABSENT {
            member.try_construct = Some(descriptor.try_construct_kind);
        }
        if !descriptor.default_value.is_empty() {
            member = member.with_default(&descriptor.default_value);
        }
        Ok(member)
    }

    /// The value text of an enumerator or a flag, within `[low, high]`.
    fn literal_value(
        &self,
        descriptor: &MemberDescriptor,
        low: i64,
        high: i64,
    ) -> Result<Option<i64>, DynamicError> {
        if descriptor.default_value.is_empty() {
            return Ok(None);
        }
        let value: i64 = descriptor.default_value.parse().map_err(|_| {
            DynamicError::bad_parameter(alloc::format!(
                "`{}` is no value of `{}`",
                descriptor.default_value,
                descriptor.name
            ))
        })?;
        if value < low || value > high {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "the value {value} of `{}` is outside {low} to {high}",
                descriptor.name
            )));
        }
        Ok(Some(value))
    }

    fn apply(
        &mut self,
        any: zerodds_ast::AnyId,
        descriptor: &AnnotationDescriptor,
    ) -> Result<(), DynamicError> {
        if !descriptor.is_consistent() {
            return Err(DynamicError::bad_parameter(
                "the annotation descriptor is not consistent",
            ));
        }
        let Some(annotation) = descriptor.annotation_type() else {
            return Err(DynamicError::bad_parameter("the annotation type is nil"));
        };
        let name = annotation.name().to_string();
        if annotation.is_builtin() {
            return self.apply_builtin(any, &name, descriptor);
        }
        let definition = self.inner.import_annotation_type(annotation)?;
        let attributes = annotation.attributes();
        let mut arguments = Vec::new();
        for (key, text) in descriptor.get_all_value() {
            let kind = attributes
                .iter()
                .find(|(attribute, _)| attribute == key)
                .map_or(ValueKind::Any, |(_, kind)| *kind);
            let value = self.inner.literal_expr(kind, text)?;
            arguments.push(zerodds_ast::AppliedArgument {
                name: Some(key.clone()),
                value,
            });
        }
        let applied = zerodds_ast::AppliedAnnotation {
            name,
            definition: Ref::Resolved(definition),
            arguments,
        };
        let facets = self.inner.facets_of(any)?;
        facets.annotations.push(applied);
        Ok(())
    }

    fn apply_builtin(
        &mut self,
        any: zerodds_ast::AnyId,
        name: &str,
        descriptor: &AnnotationDescriptor,
    ) -> Result<(), DynamicError> {
        let value = descriptor.get_value("value").ok();
        let truth = |text: Option<&str>| text.is_none_or(|t| t.eq_ignore_ascii_case("true"));
        let facets = self.inner.facets_of(any)?;
        let x = facets.xtypes_mut();
        match name {
            "final" => x.extensibility = Some(Extensibility::Final),
            "appendable" => x.extensibility = Some(Extensibility::Appendable),
            "mutable" => x.extensibility = Some(Extensibility::Mutable),
            "extensibility" => {
                x.extensibility = Some(match value {
                    Some("FINAL") => Extensibility::Final,
                    Some("APPENDABLE") => Extensibility::Appendable,
                    Some("MUTABLE") => Extensibility::Mutable,
                    _ => {
                        return Err(DynamicError::bad_parameter(
                            "`@extensibility` needs FINAL, APPENDABLE or MUTABLE",
                        ));
                    }
                });
            }
            "nested" => {
                x.is_nested = truth(value);
                x.nested_explicit = true;
            }
            "autoid" => {
                x.autoid = Some(match value {
                    Some("HASH") => AutoId::Hash,
                    Some("SEQUENTIAL") => AutoId::Sequential,
                    _ => {
                        return Err(DynamicError::bad_parameter(
                            "`@autoid` needs HASH or SEQUENTIAL",
                        ));
                    }
                });
            }
            "bit_bound" => {
                x.bit_bound =
                    Some(value.and_then(|v| v.parse::<u16>().ok()).ok_or_else(|| {
                        DynamicError::bad_parameter("`@bit_bound` needs a number")
                    })?);
            }
            "key" => {
                x.is_key = truth(value);
                // §7.2.2.4.4.4.8: a key member is must-understand.
                if x.is_key {
                    x.is_must_understand = true;
                }
            }
            "optional" => x.is_optional = truth(value),
            "external" => x.is_external = truth(value),
            "must_understand" => x.is_must_understand = truth(value),
            "non_serialized" => x.is_non_serialized = truth(value),
            "default_literal" => x.is_default_literal = true,
            "ignore_literal_names" => x.ignore_literal_names = truth(value),
            "id" => {
                let id = value
                    .and_then(|v| v.parse::<u32>().ok())
                    .ok_or_else(|| DynamicError::bad_parameter("`@id` needs a number"))?;
                x.member_id = Some(MemberIdAssignment {
                    effective: id,
                    origin: MemberIdOrigin::Declared,
                });
            }
            "hashid" => {
                x.member_id = Some(MemberIdAssignment {
                    effective: 0,
                    origin: MemberIdOrigin::Hashed {
                        source_name: value.unwrap_or_default().to_string(),
                    },
                });
            }
            "try_construct" => {
                x.try_construct = Some(match value {
                    Some("DISCARD") => TryConstruct::Discard,
                    Some("USE_DEFAULT") => TryConstruct::UseDefault,
                    Some("TRIM") => TryConstruct::Trim,
                    _ => {
                        return Err(DynamicError::bad_parameter(
                            "`@try_construct` needs DISCARD, USE_DEFAULT or TRIM",
                        ));
                    }
                });
            }
            "unit" => x.unit = value.map(ToString::to_string),
            other => {
                return Err(DynamicError::Unrepresentable {
                    what: other.to_string(),
                    kind: "a built-in annotation this builder does not set",
                });
            }
        }
        Ok(())
    }
}

impl DynamicTypeBuilder {
    pub(super) fn facets_of(
        &mut self,
        any: zerodds_ast::AnyId,
    ) -> Result<&mut zerodds_ast::Facets, DynamicError> {
        self.ast
            .facets_mut(any)
            .ok_or_else(|| DynamicError::bad_parameter("the node has no facets"))
    }

    /// A literal expression of the kind an annotation attribute has.
    pub(super) fn literal_expr(
        &mut self,
        kind: ValueKind,
        text: &str,
    ) -> Result<Id<zerodds_ast::ConstExprNode>, DynamicError> {
        let literal = match kind {
            ValueKind::Boolean => Literal::Boolean(text.eq_ignore_ascii_case("true")),
            ValueKind::Integer => Literal::Integer(text.parse::<i128>().map_err(|_| {
                DynamicError::bad_parameter(alloc::format!("`{text}` is no integer"))
            })?),
            ValueKind::Float => {
                let (mantissa, exponent) = match text.split_once(['e', 'E']) {
                    Some((mantissa, exponent)) => (mantissa, Some(exponent.to_string())),
                    None => (text, None),
                };
                Literal::Float {
                    mantissa: mantissa.to_string(),
                    exponent,
                }
            }
            ValueKind::Character => Literal::Char(
                text.chars()
                    .next()
                    .ok_or_else(|| DynamicError::bad_parameter("an empty character"))?,
            ),
            ValueKind::Text | ValueKind::Any => Literal::String(text.to_string()),
        };
        self.ast
            .add_expr(ConstExpr::Literal(literal))
            .map_err(|error| DynamicError::model(&error))
    }
}

/// An annotation type in progress: the counterpart of [`TypeBuilder`] for
/// the kind `ANNOTATION_TYPE`. See
/// [`DynamicTypeBuilderFactory::create_annotation_type`].
pub struct AnnotationTypeBuilder {
    inner: DynamicTypeBuilder,
    id: Id<AnnotationDefNode>,
}

impl AnnotationTypeBuilder {
    /// The name of the annotation type.
    #[must_use]
    pub fn get_name(&self) -> String {
        self.inner
            .model()
            .annotation_def(self.id)
            .map_or_else(|_| String::new(), |def| def.name.clone())
    }

    /// Adds the attribute `name` of type `type_`, with the default
    /// `default` where one is given (an attribute without one has to be
    /// given a value where the annotation is applied).
    ///
    /// The type of an attribute is one an annotation can hold: a
    /// primitive, a string or an enumeration (IDL 4.2 §8.3).
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a name that is empty or already
    /// an attribute, a type of another kind, or a default that is not a
    /// well-formed value of the type.
    pub fn add_member(
        &mut self,
        name: &str,
        type_: &DynamicType,
        default: Option<&str>,
    ) -> Result<(), DynamicError> {
        if name.is_empty() {
            return Err(DynamicError::bad_parameter("an attribute needs a name"));
        }
        let taken = self
            .inner
            .model()
            .annotation_def(self.id)
            .map_err(|error| DynamicError::model(&error))?
            .members
            .iter()
            .any(|member| {
                self.inner
                    .model()
                    .annotation_member(*member)
                    .is_ok_and(|member| member.name == name)
            });
        if taken {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "`{name}` is already an attribute of `@{}`",
                self.get_name()
            )));
        }
        let resolved = type_.resolve_alias()?;
        let allowed = resolved.kind().is_primitive()
            || matches!(
                resolved.kind(),
                TypeKind::String8 | TypeKind::String16 | TypeKind::Enumeration
            );
        if !allowed {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "an attribute of an annotation is a primitive, a string or an enumeration, not {}",
                resolved.kind().as_str()
            )));
        }
        let imported = self.inner.import(&resolved)?;
        let default = match default {
            Some(text) => {
                let kind = value_kind(self.inner.model(), imported);
                if !kind.is_well_formed(text) {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "`{text}` is no well-formed default for `{name}`"
                    )));
                }
                Some(self.inner.literal_expr(kind, text)?)
            }
            None => None,
        };
        self.inner
            .ast
            .add_annotation_member(self.id, name, Ref::Resolved(imported), default)
            .map_err(|error| DynamicError::model(&error))?;
        Ok(())
    }

    /// §7.5.2.9.4 `add_member` for the kind `ANNOTATION_TYPE` (Table 55):
    /// "The member descriptor must describe a consistent annotation type
    /// member. If the descriptor does not satisfy these constraints, the
    /// operation shall fail with RETCODE_BAD_PARAMETER." The member of an
    /// annotation type has a name, a type and optionally a default
    /// (`default_value`); it is none of key, optional, shared or must
    /// understand, and has no labels.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], as [`Self::add_member`] and for a
    /// descriptor that sets any of the properties an annotation member does
    /// not have.
    pub fn add_member_descriptor(
        &mut self,
        descriptor: &MemberDescriptor,
    ) -> Result<(), DynamicError> {
        if descriptor.is_key
            || descriptor.is_optional
            || descriptor.is_shared
            || descriptor.is_must_understand
            || descriptor.is_default_label
            || !descriptor.label.is_empty()
        {
            return Err(DynamicError::bad_parameter(
                "an annotation member is none of key, optional, shared, must understand and has no labels",
            ));
        }
        let default =
            (!descriptor.default_value.is_empty()).then_some(descriptor.default_value.as_str());
        self.add_member(&descriptor.name, &descriptor.type_, default)
    }

    /// The annotation type, over a snapshot of the model: the builder can
    /// go on being edited without the types it built changing.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the snapshot cannot be taken.
    pub fn build(&self) -> Result<AnnotationType, DynamicError> {
        let bytes = self
            .inner
            .model()
            .to_snapshot()
            .map_err(|error| DynamicError::model(&error))?;
        let ast = Ast::from_snapshot(&bytes).map_err(|error| DynamicError::model(&error))?;
        Ok(AnnotationType::new(Arc::new(ast), self.id))
    }
}
