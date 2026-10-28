// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `AnnotationDescriptor` and `VerbatimTextDescriptor`. XTypes 1.3
//! §7.5.2.3 and Annex C.
//!
//! The annotations of a type or of a member are **read off the model**,
//! like the other descriptors of this module. The model keeps the
//! annotations XTypes defines in typed fields — an `@key` is
//! [`zerodds_ast::XTypesFacet::is_key`], not an entry in a list — and the
//! ones a user declared in `Facets::annotations`. §7.5.2.8's
//! `get_annotation` returns both, so [`annotations_of`] puts the first
//! kind back into the form a descriptor has: a built-in annotation type
//! and string values.
//!
//! ## `type`
//!
//! §7.5.2.3.4 types `AnnotationDescriptor::type` as a `DynamicType` of
//! kind `ANNOTATION_TYPE`. An `@annotation` declaration is not a
//! `TypeNode` in this model — a `DynamicType` is a view on one — so the
//! property is an [`AnnotationType`], a view on the declaration with the
//! kind [`TypeKind::Annotation`].

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_ast::{
    AnnotationDefNode, Ast, AutoId, ConstExprNode, Extensibility, Facets, TryConstruct,
    VerbatimDirective, VerbatimPlacement, XTypesFacet,
};
use zerodds_ast_core::Id;

use super::default_literal::default_expr_text;
use super::descriptor::{MemberDescriptor, MemberId, TryConstructKind, TypeKind};
use super::error::DynamicError;
use super::type_::DynamicType;

/// An annotation type: the declaration an annotation is an application
/// of. §7.5.2.3.4, kind `ANNOTATION_TYPE`.
#[derive(Clone)]
pub struct AnnotationType {
    ast: Arc<Ast>,
    id: Id<AnnotationDefNode>,
}

impl AnnotationType {
    /// A view on one annotation declaration of a model.
    #[must_use]
    pub const fn new(ast: Arc<Ast>, id: Id<AnnotationDefNode>) -> Self {
        Self { ast, id }
    }

    /// The annotation type of that name in the model, a built-in one
    /// (`key`, `id`, …) or a declared one. The name is a simple name
    /// without the `@`.
    #[must_use]
    pub fn find(ast: &Arc<Ast>, name: &str) -> Option<Self> {
        if let Some(id) = ast.builtin_annotation(name) {
            return Some(Self::new(Arc::clone(ast), id));
        }
        ast.annotation_defs()
            .find(|(_, def)| def.name == name)
            .map(|(id, _)| Self::new(Arc::clone(ast), id))
    }

    /// The declaration this is a view of.
    #[must_use]
    pub const fn id(&self) -> Id<AnnotationDefNode> {
        self.id
    }

    /// The model the declaration is in.
    #[must_use]
    pub const fn ast(&self) -> &Arc<Ast> {
        &self.ast
    }

    fn node(&self) -> Option<&AnnotationDefNode> {
        self.ast.annotation_def(self.id).ok()
    }

    /// `ANNOTATION_TYPE`.
    #[must_use]
    pub const fn kind(&self) -> TypeKind {
        TypeKind::Annotation
    }

    /// The name of the annotation type, without the `@`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.node()
            .map_or("<unknown annotation>", |node| node.name.as_str())
    }

    /// Whether this is one of the annotations XTypes and IDL define.
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.node()
            .is_some_and(|node| node.module == self.ast.builtin_module())
    }

    /// The attributes the annotation type defines: name and, where the
    /// declaration resolves it, type.
    ///
    /// A built-in annotation answers from the table of its parameters
    /// ([`builtin_parameters`]), because the model gives each one only an
    /// implicit `value` member with no type.
    #[must_use]
    pub fn attributes(&self) -> Vec<(String, ValueKind)> {
        let Some(node) = self.node() else {
            return Vec::new();
        };
        if self.is_builtin() {
            return builtin_parameters(&node.name)
                .iter()
                .map(|(name, kind)| ((*name).to_string(), *kind))
                .collect();
        }
        node.members
            .iter()
            .filter_map(|member| {
                let member = self.ast.annotation_member(*member).ok()?;
                let kind = member
                    .type_
                    .resolved()
                    .map_or(ValueKind::Any, |id| value_kind(&self.ast, id));
                Some((member.name.clone(), kind))
            })
            .collect()
    }

    /// Two annotation types are equal when they have the same name and
    /// the same attributes.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self.name() == other.name() && self.attributes() == other.attributes()
    }

    /// §7.5.2.6, Table 51, `ANNOTATION_TYPE`: "a 'member' in this sense
    /// has the same meaning as it does in the definition of aggregated
    /// types generally". The members of an annotation type are its
    /// attributes, in declaration order, with the member ids §7.5.2.5
    /// assigns to members without `@id`: the first is 0 and each
    /// following one is the previous plus one.
    #[must_use]
    pub fn members(&self) -> Vec<AnnotationMember> {
        let Some(node) = self.node() else {
            return Vec::new();
        };
        if self.is_builtin() {
            // The model gives a built-in annotation one implicit `value`
            // member with no type; the parameters are the table's.
            return builtin_parameters(&node.name)
                .iter()
                .enumerate()
                .map(|(index, (name, kind))| AnnotationMember {
                    ast: Arc::clone(&self.ast),
                    annotation: self.id,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                    name: (*name).to_string(),
                    kind: *kind,
                    type_id: None,
                    default: None,
                })
                .collect();
        }
        let mut out = Vec::new();
        for (index, member) in node.members.iter().enumerate() {
            let Ok(member) = self.ast.annotation_member(*member) else {
                continue;
            };
            let type_id = member.type_.resolved();
            out.push(AnnotationMember {
                ast: Arc::clone(&self.ast),
                annotation: self.id,
                index: u32::try_from(index).unwrap_or(u32::MAX),
                name: member.name.clone(),
                kind: type_id.map_or(ValueKind::Any, |id| value_kind(&self.ast, id)),
                type_id,
                default: member.default,
            });
        }
        out
    }

    /// `get_member_count()` of an annotation type.
    #[must_use]
    pub fn member_count(&self) -> u32 {
        u32::try_from(self.members().len()).unwrap_or(u32::MAX)
    }

    /// `get_member_by_index(index)`, as §7.5.2.8.7.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an index past the last member.
    pub fn get_member_by_index(&self, index: u32) -> Result<AnnotationMember, DynamicError> {
        self.members()
            .into_iter()
            .nth(index as usize)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`@{}` has no member at index {index}",
                    self.name()
                ))
            })
    }

    /// `get_member_by_name(name)`, as §7.5.2.8.6.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a name that is no attribute.
    pub fn get_member_by_name(&self, name: &str) -> Result<AnnotationMember, DynamicError> {
        self.members()
            .into_iter()
            .find(|member| member.name == name)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`@{}` has no member `{name}`",
                    self.name()
                ))
            })
    }

    /// `get_member(id)`, as §7.5.2.8.9.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an id no attribute has.
    pub fn get_member(&self, id: MemberId) -> Result<AnnotationMember, DynamicError> {
        self.members()
            .into_iter()
            .find(|member| member.id() == id)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`@{}` has no member with the id {id}",
                    self.name()
                ))
            })
    }

    /// `get_all_members_by_name()`, as §7.5.2.8.
    #[must_use]
    pub fn get_all_members_by_name(&self) -> BTreeMap<String, AnnotationMember> {
        self.members()
            .into_iter()
            .map(|member| (member.name.clone(), member))
            .collect()
    }

    /// `get_all_members()`, as §7.5.2.8.
    #[must_use]
    pub fn get_all_members(&self) -> BTreeMap<MemberId, AnnotationMember> {
        self.members()
            .into_iter()
            .map(|member| (member.id(), member))
            .collect()
    }
}

/// One member of an annotation type: an attribute. §7.5.2.6, Table 51.
#[derive(Clone)]
pub struct AnnotationMember {
    ast: Arc<Ast>,
    annotation: Id<AnnotationDefNode>,
    index: u32,
    name: String,
    kind: ValueKind,
    type_id: Option<Id<zerodds_ast::TypeNode>>,
    default: Option<Id<ConstExprNode>>,
}

impl AnnotationMember {
    /// `get_name()`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `get_id()`: the attribute's position, §7.5.2.5.
    #[must_use]
    pub const fn id(&self) -> MemberId {
        self.index
    }

    /// The attribute's position in the declaration.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// The annotation type the member belongs to.
    #[must_use]
    pub fn annotation_type(&self) -> AnnotationType {
        AnnotationType::new(Arc::clone(&self.ast), self.annotation)
    }

    /// `get_descriptor()`: name, id, type and `default_value` (§7.5.2.7).
    ///
    /// The type is the declared one. A built-in annotation's model has
    /// no type for its parameters (see [`AnnotationType::attributes`]),
    /// so the type is the one the value is written as in an
    /// [`AnnotationDescriptor`]: `Boolean`, `Int64`, `Float64`, `Char8`,
    /// or an unbounded `string` for text and for a value of any type.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] where a default does not evaluate.
    pub fn descriptor(&self) -> Result<MemberDescriptor, DynamicError> {
        use super::factory::DynamicTypeBuilderFactory;
        let type_ = match self.type_id {
            Some(id) => DynamicType::new(Arc::clone(&self.ast), id),
            None => {
                let factory = DynamicTypeBuilderFactory::get_instance();
                match self.kind {
                    ValueKind::Boolean => factory.get_primitive_type(TypeKind::Boolean)?,
                    ValueKind::Integer => factory.get_primitive_type(TypeKind::Int64)?,
                    ValueKind::Float => factory.get_primitive_type(TypeKind::Float64)?,
                    ValueKind::Character => factory.get_primitive_type(TypeKind::Char8)?,
                    ValueKind::Text | ValueKind::Any => factory
                        .create_string_type(super::LENGTH_UNLIMITED)?
                        .build()?,
                }
            }
        };
        let default_value = match self.default {
            Some(expr) => default_expr_text(&self.ast, expr, &self.name)?,
            None => String::new(),
        };
        Ok(MemberDescriptor {
            name: self.name.clone(),
            id: self.id(),
            type_,
            index: self.index,
            label: Vec::new(),
            is_default_label: false,
            try_construct_kind: TryConstructKind::Discard,
            is_key: false,
            is_optional: false,
            is_must_understand: false,
            is_shared: false,
            has_default_value: self.default.is_some(),
            default_value,
        })
    }

    /// `equals(other)`: same annotation type and equal descriptors.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self.annotation_type().equals(&other.annotation_type())
            && match (self.descriptor(), other.descriptor()) {
                (Ok(a), Ok(b)) => {
                    a.name == b.name
                        && a.id == b.id
                        && a.index == b.index
                        && a.default_value == b.default_value
                        && a.type_.equals(&b.type_)
                }
                _ => false,
            }
    }
}

impl core::fmt::Debug for AnnotationMember {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AnnotationMember({} id {})", self.name, self.index)
    }
}

impl PartialEq for AnnotationMember {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl core::fmt::Debug for AnnotationType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AnnotationType(@{})", self.name())
    }
}

impl PartialEq for AnnotationType {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

/// What a string value of an annotation attribute has to be, to be
/// "a well-formed string representation of an object of the type of the
/// attribute" (§7.5.2.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// `TRUE` or `FALSE`.
    Boolean,
    /// An integer literal.
    Integer,
    /// A floating-point literal.
    Float,
    /// One character.
    Character,
    /// Any text.
    Text,
    /// A literal of an enumeration or a type this module does not parse;
    /// any non-empty text.
    Any,
}

impl ValueKind {
    /// Whether `text` is a well-formed value of this kind. Strings and
    /// characters are not quoted, and no operator appears
    /// (§7.5.2.3.5).
    #[must_use]
    pub fn is_well_formed(self, text: &str) -> bool {
        match self {
            Self::Boolean => matches!(text, "TRUE" | "FALSE" | "true" | "false"),
            Self::Integer => {
                let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
                let parsed = if let Some(hex) = digits
                    .strip_prefix("0x")
                    .or_else(|| digits.strip_prefix("0X"))
                {
                    i128::from_str_radix(hex, 16).is_ok()
                } else {
                    digits.parse::<i128>().is_ok()
                };
                parsed && !digits.is_empty()
            }
            Self::Float => text.parse::<f64>().is_ok(),
            Self::Character => text.chars().count() == 1,
            Self::Text => true,
            Self::Any => !text.is_empty(),
        }
    }
}

pub(super) fn value_kind(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> ValueKind {
    use zerodds_ast::{PrimitiveKind as P, TypeKind as K};
    match ast.type_(id).map(|node| &node.kind) {
        Ok(K::Primitive(primitive)) => match primitive {
            P::Boolean => ValueKind::Boolean,
            P::Char | P::WChar => ValueKind::Character,
            P::Float32 | P::Float64 | P::Float128 => ValueKind::Float,
            _ => ValueKind::Integer,
        },
        Ok(K::String { .. }) => ValueKind::Text,
        _ => ValueKind::Any,
    }
}

/// The parameters of a built-in annotation, by name, as the clauses
/// that define them give them (§7.3.1.2, Annex B, IDL 4.2 §8.3).
#[must_use]
pub fn builtin_parameters(name: &str) -> &'static [(&'static str, ValueKind)] {
    use ValueKind::{Any, Boolean, Integer, Text};
    match name {
        "id" | "position" | "bit_bound" | "value" => &[("value", Integer)],
        "key"
        | "optional"
        | "must_understand"
        | "external"
        | "nested"
        | "non_serialized"
        | "ignore_literal_names"
        | "default_nested" => &[("value", Boolean)],
        "final" | "appendable" | "mutable" | "default_literal" | "service" | "oneway" | "ami" => {
            &[]
        }
        "extensibility" | "autoid" | "try_construct" | "data_representation" => &[("value", Text)],
        "hashid" | "unit" => &[("value", Text)],
        "default" | "min" | "max" => &[("value", Any)],
        "range" => &[("min", Any), ("max", Any)],
        "verbatim" => &[("language", Text), ("placement", Text), ("text", Text)],
        "topic" => &[("name", Text), ("platform", Text)],
        _ => &[],
    }
}

/// The application of an annotation type to a type or a member.
/// §7.5.2.3, Table 49.
///
/// Value semantics: it is cloned, compared and overwritten as a whole.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationDescriptor {
    type_: Option<AnnotationType>,
    value: BTreeMap<String, String>,
}

impl AnnotationDescriptor {
    /// A newly created descriptor: "this reference shall be nil"
    /// (§7.5.2.3.4) and no value is set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A descriptor of the application of `type_`.
    #[must_use]
    pub fn of_type(type_: AnnotationType) -> Self {
        Self {
            type_: Some(type_),
            value: BTreeMap::new(),
        }
    }

    /// §7.5.2.3.4 `type`.
    #[must_use]
    pub const fn annotation_type(&self) -> Option<&AnnotationType> {
        self.type_.as_ref()
    }

    /// Sets `type`.
    pub fn set_annotation_type(&mut self, type_: Option<AnnotationType>) {
        self.type_ = type_;
    }

    /// Annex C `get_value(inout value, in key)`: the value of one
    /// attribute.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an empty key;
    /// [`DynamicError::NoData`] when no value exists for the key
    /// (§7.5.2.1).
    pub fn get_value(&self, key: &str) -> Result<&str, DynamicError> {
        if key.is_empty() {
            return Err(DynamicError::bad_parameter("the key is nil"));
        }
        self.value
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| DynamicError::no_data(alloc::format!("no value for `{key}`")))
    }

    /// Annex C `get_all_value(inout value)`: every attribute that has a
    /// value. An attribute with none "shall be considered to have its
    /// default value" (§7.5.2.3.5).
    #[must_use]
    pub const fn get_all_value(&self) -> &BTreeMap<String, String> {
        &self.value
    }

    /// Annex C `set_value(in key, in value)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an empty key or value
    /// (§7.5.2.1: "if either argument is nil").
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), DynamicError> {
        if key.is_empty() || value.is_empty() {
            return Err(DynamicError::bad_parameter("the key or the value is nil"));
        }
        self.value.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// §7.5.2.3.1 `copy_from(other)`: overwrite this descriptor so that
    /// `equals(other)` holds. `other` is not changed.
    pub fn copy_from(&mut self, other: &Self) {
        self.clone_from(other);
    }

    /// §7.5.2.3.2 `equals(other)`: the types are equal and both have the
    /// same attributes with equal values.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        let types_equal = match (&self.type_, &other.type_) {
            (Some(a), Some(b)) => a.equals(b),
            (None, None) => true,
            _ => false,
        };
        types_equal && self.value == other.value
    }

    /// §7.5.2.3.3 `is_consistent()`: the type is a non-nil annotation
    /// type, every attribute named in `value` is one of its attributes,
    /// and every value is a well-formed string of the attribute's type.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        let Some(type_) = &self.type_ else {
            return false;
        };
        if type_.kind() != TypeKind::Annotation {
            return false;
        }
        let attributes = type_.attributes();
        self.value.iter().all(|(key, value)| {
            attributes
                .iter()
                .find(|(name, _)| name == key)
                .is_some_and(|(_, kind)| kind.is_well_formed(value))
        })
    }
}

/// `VerbatimTextDescriptor` (Annex C): a `@verbatim` text and where it
/// goes.
///
/// Annex C names `placement` and `text`. The model's directive also has
/// the target language, which `@verbatim(language = …)` selects (IDL 4.2
/// §8.3.5.1), so [`Self::language`] is carried too; `*` is every language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimTextDescriptor {
    /// Annex C `placement`, in the spelling of IDL 4.2 §8.3.5.1:
    /// `begin-file`, `before-declaration`, `begin-declaration`,
    /// `end-declaration`, `after-declaration`, `end-file`.
    pub placement: String,
    /// Annex C `text`.
    pub text: String,
    /// The language the text is for; `*` for all.
    pub language: String,
}

impl Default for VerbatimTextDescriptor {
    fn default() -> Self {
        Self {
            placement: String::new(),
            text: String::new(),
            language: String::from("*"),
        }
    }
}

impl VerbatimTextDescriptor {
    /// The descriptor of a directive of the model.
    #[must_use]
    pub fn of_directive(directive: &VerbatimDirective) -> Self {
        Self {
            placement: placement_text(directive.placement).to_string(),
            text: directive.text.clone(),
            language: directive.language.clone(),
        }
    }

    /// Annex C `copy_from(other)`.
    pub fn copy_from(&mut self, other: &Self) {
        self.clone_from(other);
    }

    /// Annex C `equals(other)`.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self == other
    }

    /// Annex C `is_consistent()`: the placement is one of the six
    /// spellings.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        PLACEMENTS.iter().any(|(_, name)| *name == self.placement)
    }

    /// The directive of the model this descriptor stands for.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when the placement is none of the
    /// six.
    pub fn to_directive(&self) -> Result<VerbatimDirective, DynamicError> {
        let placement = PLACEMENTS
            .iter()
            .find(|(_, name)| *name == self.placement)
            .map(|(placement, _)| *placement)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`{}` is no `@verbatim` placement",
                    self.placement
                ))
            })?;
        Ok(VerbatimDirective {
            language: self.language.clone(),
            placement,
            text: self.text.clone(),
        })
    }
}

const PLACEMENTS: [(VerbatimPlacement, &str); 6] = [
    (VerbatimPlacement::BeginFile, "begin-file"),
    (VerbatimPlacement::BeforeDeclaration, "before-declaration"),
    (VerbatimPlacement::BeginDeclaration, "begin-declaration"),
    (VerbatimPlacement::EndDeclaration, "end-declaration"),
    (VerbatimPlacement::AfterDeclaration, "after-declaration"),
    (VerbatimPlacement::EndFile, "end-file"),
];

fn placement_text(placement: VerbatimPlacement) -> &'static str {
    PLACEMENTS
        .iter()
        .find(|(known, _)| *known == placement)
        .map_or("before-declaration", |(_, name)| name)
}

/// What a facet applies, as strings: the name of the annotation type and
/// its attribute values.
type Applied = (&'static str, Vec<(&'static str, String)>);

fn flag(name: &'static str) -> Applied {
    (name, Vec::new())
}

fn valued(name: &'static str, key: &'static str, value: impl Into<String>) -> Applied {
    (name, alloc::vec![(key, value.into())])
}

fn expr_text(ast: &Ast, expr: Id<ConstExprNode>, at: &str) -> Option<String> {
    default_expr_text(ast, expr, at).ok()
}

/// The built-in annotations a facet records, as applications.
fn builtin_applications(ast: &Ast, facets: &Facets, at: &str) -> Vec<Applied> {
    let mut out = Vec::new();
    let Some(x) = facets.xtypes.as_ref() else {
        return out;
    };
    builtin_of_type(ast, x, at, &mut out);
    if x.is_key {
        out.push(flag("key"));
    }
    if x.is_optional {
        out.push(flag("optional"));
    }
    if x.is_external {
        out.push(flag("external"));
    }
    if x.is_must_understand {
        out.push(flag("must_understand"));
    }
    if x.is_non_serialized {
        out.push(flag("non_serialized"));
    }
    if x.is_default_literal {
        out.push(flag("default_literal"));
    }
    if let Some(behaviour) = x.try_construct {
        out.push(valued("try_construct", "value", behaviour_text(behaviour)));
    }
    if let Some(assignment) = &x.member_id {
        match &assignment.origin {
            zerodds_ast::MemberIdOrigin::Declared => {
                out.push(valued("id", "value", assignment.effective.to_string()));
            }
            zerodds_ast::MemberIdOrigin::Hashed { source_name } => {
                out.push(valued("hashid", "value", source_name.clone()));
            }
            _ => {}
        }
    }
    if let Some(expr) = x.default_value {
        if let Some(text) = expr_text(ast, expr, at) {
            out.push(valued("default", "value", text));
        }
    }
    if let Some(range) = x.range {
        let mut values = Vec::new();
        if let Some(min) = range.min.and_then(|e| expr_text(ast, e, at)) {
            values.push(("min", min));
        }
        if let Some(max) = range.max.and_then(|e| expr_text(ast, e, at)) {
            values.push(("max", max));
        }
        out.push(("range", values));
    }
    if let Some(unit) = &x.unit {
        out.push(valued("unit", "value", unit.clone()));
    }
    out
}

fn builtin_of_type(ast: &Ast, x: &XTypesFacet, at: &str, out: &mut Vec<Applied>) {
    let _ = (ast, at);
    match x.extensibility {
        Some(Extensibility::Final) => out.push(flag("final")),
        Some(Extensibility::Appendable) => out.push(flag("appendable")),
        Some(Extensibility::Mutable) => out.push(flag("mutable")),
        None => {}
    }
    if x.nested_explicit || x.is_nested {
        out.push(valued(
            "nested",
            "value",
            if x.is_nested { "TRUE" } else { "FALSE" },
        ));
    }
    match x.autoid {
        Some(AutoId::Sequential) => out.push(valued("autoid", "value", "SEQUENTIAL")),
        Some(AutoId::Hash) => out.push(valued("autoid", "value", "HASH")),
        None => {}
    }
    if let Some(bound) = x.bit_bound {
        out.push(valued("bit_bound", "value", bound.to_string()));
    }
    if x.ignore_literal_names {
        out.push(flag("ignore_literal_names"));
    }
    if let Some(topic) = &x.topic {
        let mut values = Vec::new();
        if let Some(name) = &topic.name {
            values.push(("name", name.clone()));
        }
        if let Some(platform) = &topic.platform {
            values.push(("platform", platform.clone()));
        }
        out.push(("topic", values));
    }
    if let Some(mask) = x.data_representation {
        let mut parts = Vec::new();
        if mask.xcdr1 {
            parts.push("XCDR1");
        }
        if mask.xcdr2 {
            parts.push("XCDR2");
        }
        if mask.xml {
            parts.push("XML");
        }
        out.push(valued("data_representation", "value", parts.join(" | ")));
    }
}

const fn behaviour_text(behaviour: TryConstruct) -> &'static str {
    behaviour.as_str()
}

/// Every annotation applied to a node, as descriptors: the built-in ones
/// the model keeps in typed fields, then the user-declared ones.
#[must_use]
pub fn annotations_of(ast: &Arc<Ast>, facets: &Facets, at: &str) -> Vec<AnnotationDescriptor> {
    let mut out = Vec::new();
    for (name, values) in builtin_applications(ast, facets, at) {
        let Some(type_) = AnnotationType::find(ast, name) else {
            continue;
        };
        let mut descriptor = AnnotationDescriptor::of_type(type_);
        for (key, value) in values {
            descriptor.value.insert(key.to_string(), value);
        }
        out.push(descriptor);
    }
    for applied in &facets.annotations {
        let type_ = applied
            .definition
            .resolved()
            .map(|id| AnnotationType::new(Arc::clone(ast), id));
        let mut descriptor = AnnotationDescriptor {
            type_,
            value: BTreeMap::new(),
        };
        for argument in &applied.arguments {
            let key = argument.name.as_deref().unwrap_or("value");
            if let Some(text) = expr_text(ast, argument.value, at) {
                descriptor.value.insert(key.to_string(), text);
            }
        }
        out.push(descriptor);
    }
    out
}

/// The `@verbatim` texts of a node, as descriptors.
#[must_use]
pub fn verbatim_texts_of(facets: &Facets) -> Vec<VerbatimTextDescriptor> {
    facets.xtypes.as_ref().map_or_else(Vec::new, |x| {
        x.verbatim
            .iter()
            .map(VerbatimTextDescriptor::of_directive)
            .collect()
    })
}
