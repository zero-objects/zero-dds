// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The IDL rules the model cannot express structurally. Spec §6.
//!
//! The model's own invariants run at every commit and are not repeated
//! here: this is what is legal to build and illegal to write.
//!
//! Every diagnostic carries a byte range, a code and the paragraph it
//! comes from. A rule with no paragraph is ZeroDDS policy and says so.

use crate::lower::Lowering;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast::{AnyId, Bound, MemberNode, PrimitiveKind, TryConstruct, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_idl_cst::Span;
use zerodds_idl_parse::DiagnosticCode;

/// The largest number of digits a `fixed` may have. IDL 4.2 §7.4.1.4.4.
const MAX_FIXED_DIGITS: u8 = 31;

/// How a forbidden type was reached from a key member.
///
/// Only the diagnostic's wording depends on it: "`m` is a `map`" reads
/// as a mistake in one line, and "the key of `m` reaches a `map`" tells
/// the reader to look at another declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyReach {
    /// The member's own type, after aliases.
    Direct,
    /// Through a collection element or a nested type's own key.
    Nested,
}

/// How far a `typedef` chain is followed before it is taken for a cycle.
/// The cycle itself is a resolution fault with its own report.
const MAX_ALIAS_DEPTH: usize = 64;

impl Lowering<'_> {
    /// Check every rule §6 states, over the finished model.
    ///
    /// Runs after completion, because a rule about member ids needs the
    /// derived ones to exist.
    pub(crate) fn validate(&mut self) {
        let types: Vec<Id<TypeNode>> = self.ast.types().map(|(id, _)| id).collect();
        for id in types {
            self.validate_type(id);
        }
        self.annotation_member_types();
        self.forward_declarations_are_defined();
    }

    fn validate_type(&mut self, id: Id<TypeNode>) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let span = self.span_of(AnyId::Type(id, node.node_id));
        self.external_is_a_member_annotation(id, span);
        self.topic_is_on_a_topic_type(id, span);
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        match &node.kind {
            TypeKind::Union { discriminator, .. } => {
                let discriminator = discriminator.clone();
                self.union_rules(id, &discriminator, span);
                self.union_member_rules(id);
                let empty = self.ast.cases_of(id).is_empty();
                self.body_is_not_empty(
                    empty,
                    span,
                    "a `union` needs at least one `case`",
                    "OMG IDL 4.2 §7.4.1.3 rule (52)",
                );
            }
            TypeKind::Enumeration { enumerators } => {
                let empty = enumerators.is_empty();
                let enumerators = enumerators.clone();
                self.body_is_not_empty(
                    empty,
                    span,
                    "an `enum` needs at least one enumerator",
                    "OMG IDL 4.2 §7.4.1.3 rule (57)",
                );
                self.enumerator_values_fit_int32(&enumerators, span);
                self.enumerator_values_are_unique(&enumerators, span);
                self.extensibility_is_final_or_appendable(id, "enum", span);
            }
            TypeKind::Bitmask { bits } => {
                let empty = bits.is_empty();
                self.body_is_not_empty(
                    empty,
                    span,
                    "a `bitmask` needs at least one bit value",
                    "OMG IDL 4.2 §7.4.13.3 rule (204)",
                );
                self.extensibility_is_final_or_appendable(id, "bitmask", span);
            }
            TypeKind::Fixed { digits, scale } => {
                let (digits, scale) = (*digits, *scale);
                self.fixed_rules(digits, scale, span);
            }
            TypeKind::String { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a bound");
            }
            TypeKind::Sequence { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a bound");
                self.trim_is_defined(id, span, "a sequence element");
            }
            TypeKind::Map { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a map bound");
                self.trim_is_defined(id, span, "a map value");
            }
            // Array dimensions and duplicate default cases are
            // structural invariants the model refuses outright; checking
            // them again here would give one rule two answers.
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let derives = matches!(node.kind, TypeKind::Structure { base: Some(_), .. });
                self.member_rules(id);
                if derives {
                    self.inheritance_rules(id, span);
                }
            }
            _ => {}
        }
    }

    /// `@external` written on a type declaration is refused by name.
    ///
    /// DDS-XTypes 1.3 Table 21 applies `@external` to members; on a
    /// `typedef`, a `struct` or any other type declaration the model has
    /// no consumer for it — the layout planner, the codec walks and the
    /// language adapters all read it off the member. Before this rule,
    /// `@external typedef long Handle;` lowered without a diagnostic and
    /// the flag sat on the alias node where nothing ever read it — an
    /// annotation that vanished between the source and every consumer,
    /// which is the silent-substitution class this project refuses.
    fn external_is_a_member_annotation(&mut self, id: Id<TypeNode>, span: Span) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let external = node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|xtypes| xtypes.is_external);
        if !external {
            return;
        }
        let what = node.kind.as_str();
        let message = alloc::format!(
            "`@external` applies to the members of an aggregated type, not to a {what} \
             declaration; nothing downstream reads it here, so it is refused rather than \
             silently dropped"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            span,
            &message,
            "DDS-XTypes 1.3 Table 21",
        );
    }

    /// An enumerated type is `@final` or `@appendable`, never `@mutable`.
    ///
    /// DDS-XTypes 1.3 Table 12 (§7.2.3): "Enumerated Types: ENUMERATION_TYPE,
    /// BITMASK_TYPE — These types may be final or appendable on a
    /// type-by-type basis." §7.2.2.4.1 repeats it for enumerations. A
    /// `@mutable` enumeration reached the TypeObject as an `IS_MUTABLE`
    /// bit in the enumeration's flag word, a kind of enumeration the
    /// specification does not have.
    fn extensibility_is_final_or_appendable(&mut self, id: Id<TypeNode>, what: &str, span: Span) {
        let mutable = self
            .ast
            .type_(id)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .is_some_and(|xtypes| {
                xtypes.extensibility == Some(zerodds_ast::Extensibility::Mutable)
            });
        if mutable {
            let message = alloc::format!(
                "an `{what}` is `@final` or `@appendable`; `@mutable` applies to structures and \
                 unions"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidAnnotation,
                span,
                &message,
                "DDS-XTypes 1.3 §7.2.2.4.1, Table 12",
            );
        }
    }

    /// The value of an enumerated literal is an Int32.
    ///
    /// DDS-XTypes 1.3 Table 4: an enumerated literal has "a name and an
    /// Int32 value". `@value` takes any integer constant, and the
    /// TypeObject writes `CommonEnumeratedLiteral.value` as an `int32`
    /// (§7.3.4.5.5), so a value outside the range has no representation
    /// and `zerodds_types::from_ast` reports `OutOfRange` for it.
    fn enumerator_values_fit_int32(
        &mut self,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        span: Span,
    ) {
        for id in enumerators {
            let Ok(node) = self.ast.enumerator(*id) else {
                continue;
            };
            let name = node.name.clone();
            let Some(value) = self.ast.effective_enumerator_value(*id) else {
                continue;
            };
            if i32::try_from(value).is_err() {
                let message = alloc::format!(
                    "the enumerator `{name}` has the value `{value}`, which is no Int32"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "DDS-XTypes 1.3 Table 4, §7.2.2.4.1",
                );
            }
        }
    }

    /// The literals of an enumeration have unique values.
    ///
    /// DDS-XTypes 1.3 Table 4: an enumerated literal has "a name and an
    /// Int32 value that are each unique within that type". Two literals
    /// of one value reached the TypeObject as two literals the Table 18
    /// rule ("any literals that have the same value in T1 and T2 also have
    /// the same name") can not tell apart.
    fn enumerator_values_are_unique(
        &mut self,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        span: Span,
    ) {
        let mut seen: Vec<(i128, String)> = Vec::new();
        for id in enumerators {
            let Ok(node) = self.ast.enumerator(*id) else {
                continue;
            };
            let name = node.name.clone();
            let Some(value) = self.ast.effective_enumerator_value(*id) else {
                continue;
            };
            if let Some((_, first)) = seen.iter().find(|(v, _)| *v == value) {
                let message = alloc::format!(
                    "the enumerators `{first}` and `{name}` both have the value `{value}`"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "DDS-XTypes 1.3 Table 4, §7.2.2.4.1",
                );
            } else {
                seen.push((value, name));
            }
        }
    }

    /// The type of an annotation member is one §7.2.2.6 permits.
    ///
    /// "The types permitted for an AnnotationParameter are: Primitive
    /// types, String types of Char8 or Char16 elements, Enumerated types."
    /// An alias resolves to its target first.
    fn annotation_member_types(&mut self) {
        let defs: Vec<_> = self
            .ast
            .annotation_defs()
            .map(|(id, node)| (id, node.node_id, node.name.clone(), node.members.clone()))
            .collect();
        for (id, node_id, name, members) in defs {
            let at = self.span_of(AnyId::AnnotationDef(id, node_id));
            for member in members {
                let Ok(node) = self.ast.annotation_member(member) else {
                    continue;
                };
                let member_name = node.name.clone();
                let Some(target) = node.type_.resolved() else {
                    continue;
                };
                if !self.is_legal_annotation_member_type(target, 0) {
                    let found = self
                        .ast
                        .type_(target)
                        .map_or("an unknown type", |node| node.kind.as_str());
                    let message = alloc::format!(
                        "`@{name}`'s member `{member_name}` is {found}; an annotation member is a \
                         primitive, a string or wstring, or an enumeration"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidAnnotation,
                        at,
                        &message,
                        "DDS-XTypes 1.3 §7.2.2.6",
                    );
                }
            }
        }
    }

    fn is_legal_annotation_member_type(&self, target: Id<TypeNode>, depth: usize) -> bool {
        if depth > MAX_ALIAS_DEPTH {
            return false;
        }
        let Ok(node) = self.ast.type_(target) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::String { .. } => true,
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .is_some_and(|target| self.is_legal_annotation_member_type(target, depth + 1)),
            _ => false,
        }
    }

    /// `@topic` is written on a type that may be a topic type.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.16: "The @topic annotation can only be
    /// used on types that are legal as DDS Topic types, see 7.6.2", and
    /// §7.6.2: "The only types that may be associated with a DDS Topic
    /// are the Aggregated Types" — structures and unions.
    fn topic_is_on_a_topic_type(&mut self, id: Id<TypeNode>, span: Span) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        if node
            .facets
            .xtypes
            .as_ref()
            .is_none_or(|xtypes| xtypes.topic.is_none())
        {
            return;
        }
        if matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
        ) {
            return;
        }
        let what = node.kind.as_str();
        let message = alloc::format!(
            "`@topic` applies to the aggregated types that DDS allows as topic types, not to a \
             {what} declaration"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            span,
            &message,
            "DDS-XTypes 1.3 §7.3.1.2.1.16, §7.6.2",
        );
    }

    // -- §6: bodies the grammar makes one-or-more ------------------------

    /// A repetition IDL 4.2 writes as one-or-more, written as none.
    ///
    /// Three rules say it and all three were read here as zero-or-more:
    /// (52) `<switch_body> ::= <case>+`, (57)'s `<enumerator> { ","
    /// <enumerator> }*` and (204)'s `<bit_value> { "," <bit_value> }*`.
    /// `enum E { };` reached the model as an enumeration with no value
    /// it could ever hold and nothing said about it; all seven foreign
    /// front ends measured refuse the file.
    ///
    /// Two empty bodies next door are legal and stay legal, which is why
    /// this is a check per rule and not a rule about empty bodies: (198)
    /// adds `"struct" <identifier> "{" "}"` to Extended Data-Types
    /// outright, and (200) writes a bitset's body as `<bitfield>*`.
    fn body_is_not_empty(&mut self, empty: bool, at: Span, message: &str, spec: &'static str) {
        if empty {
            self.diagnose_at(DiagnosticCode::MissingToken, at, message, spec);
        }
    }

    // -- §6: unions ------------------------------------------------------

    /// A discriminator must be integral, boolean, character or an enum,
    /// and every case label must be unique. IDL 4.2 §7.4.1.4.4.3.
    fn union_rules(&mut self, id: Id<TypeNode>, discriminator: &Ref<TypeNode>, span: Span) {
        let Some(target) = discriminator.resolved() else {
            // An unresolved discriminator is a resolution failure, which
            // the resolve pass reports; reporting it again here would
            // say the same thing twice.
            return;
        };
        if !self.is_legal_discriminator(target) {
            let found = self
                .ast
                .type_(target)
                .map_or("an unknown type", |node| node.kind.as_str());
            let message = alloc::format!(
                "a union discriminator must be an integer, boolean, character or enum, not {found}"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidDiscriminator,
                span,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4.3",
            );
        }

        // Labels are unique across the whole union.
        let mut seen: Vec<i128> = Vec::new();
        for case in self.ast.cases_of(id).to_vec() {
            let Ok(node) = self.ast.case(case) else {
                continue;
            };
            let labels = node.labels.clone();
            let at = self.span_of(AnyId::Case(case, node.node_id));
            for label in labels {
                match label {
                    // A second `default:` is a structural invariant the
                    // model already refuses.
                    zerodds_ast::CaseLabel::Default => {}
                    zerodds_ast::CaseLabel::Value(expr) => {
                        let Ok(value) = self.ast.eval_integral(expr) else {
                            continue;
                        };
                        if seen.contains(&value) {
                            let message =
                                alloc::format!("the case label `{value}` appears more than once");
                            self.diagnose_at(
                                DiagnosticCode::InvalidCaseLabel,
                                at,
                                &message,
                                "OMG IDL 4.2 §7.4.1.4.4.3",
                            );
                        } else {
                            seen.push(value);
                        }
                        self.label_is_in_range(target, value, at);
                    }
                }
            }
        }
    }

    fn is_legal_discriminator(&self, target: Id<TypeNode>) -> bool {
        let Ok(node) = self.ast.type_(target) else {
            return false;
        };
        match &node.kind {
            TypeKind::Enumeration { .. } => true,
            TypeKind::Primitive(kind) => matches!(
                kind,
                PrimitiveKind::Boolean
                    | PrimitiveKind::Char
                    | PrimitiveKind::WChar
                    | PrimitiveKind::Octet
                    | PrimitiveKind::Int8
                    | PrimitiveKind::UInt8
                    | PrimitiveKind::Int16
                    | PrimitiveKind::UInt16
                    | PrimitiveKind::Int32
                    | PrimitiveKind::UInt32
                    | PrimitiveKind::Int64
                    | PrimitiveKind::UInt64
            ),
            // A typedef to a legal discriminator is one too.
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .is_some_and(|target| self.is_legal_discriminator(target)),
            _ => false,
        }
    }

    /// A label outside the discriminator's range can never be selected.
    fn label_is_in_range(&mut self, discriminator: Id<TypeNode>, value: i128, at: Span) {
        let Ok(node) = self.ast.type_(discriminator) else {
            return;
        };
        let range = match &node.kind {
            TypeKind::Primitive(kind) => primitive_range(*kind),
            TypeKind::Enumeration { enumerators } => {
                // An enum label must name one of its own enumerators.
                let enumerators = enumerators.clone();
                let known = enumerators.iter().any(|enumerator| {
                    self.ast.effective_enumerator_value(*enumerator) == Some(value)
                });
                if !known {
                    let message = alloc::format!(
                        "the case label `{value}` is not one of the discriminator's enumerators"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidCaseLabel,
                        at,
                        &message,
                        "OMG IDL 4.2 §7.4.1.4.4.3",
                    );
                }
                return;
            }
            TypeKind::Alias { aliased } => {
                if let Some(target) = aliased.resolved() {
                    self.label_is_in_range(target, value, at);
                }
                return;
            }
            _ => return,
        };
        let Some((low, high)) = range else {
            return;
        };
        if value < low || value > high {
            let message = alloc::format!(
                "the case label `{value}` is outside the discriminator's range {low}..={high}"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidCaseLabel,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4.3",
            );
        }
    }

    // -- §6: members -----------------------------------------------------

    /// `@key` must not be `@optional`, and two members must not end up
    /// with the same id.
    fn member_rules(&mut self, id: Id<TypeNode>) {
        let members = self.ast.members_of(id).to_vec();
        let mut seen: Vec<(u32, String, &'static str)> = Vec::new();

        for member in members {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let name = node.name.clone();
            let declared_ref = node.type_.clone();
            let at = self.span_of(member_any);
            // `try_construct` is checked before the facet is unwrapped:
            // a member with no annotations of its own can still inherit
            // `TRIM` from the alias it is declared with, and skipping it
            // here let `@try_construct(TRIM) typedef long L;` through.
            self.member_trim_is_defined(&declared_ref, member_any, &name, at);
            let Some(xtypes) = self.ast.facets(member_any).and_then(|f| f.xtypes.clone()) else {
                continue;
            };

            if xtypes.is_key && xtypes.is_optional {
                let message = alloc::format!("`{name}` cannot be both `@key` and `@optional`");
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.8, §7.3.1.2.1.3",
                );
            }
            if xtypes.is_optional && xtypes.default_value.is_some() {
                let message = alloc::format!(
                    "`{name}` is `@optional` and has a `@default`; an optional member has no \
                     explicit default value"
                );
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.9",
                );
            }
            if xtypes.is_key && xtypes.is_non_serialized {
                let message =
                    alloc::format!("`{name}` cannot be both `@key` and `@non_serialized`");
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.4.4.2",
                );
            }
            if xtypes.is_key {
                self.key_member_type_rules(member, &name, at);
            }

            // The model refuses a commit that would collide, but ids are
            // written onto members after they exist, so a collision
            // between the last two members reaches no commit. Both paths
            // word it the same way and the duplicate is dropped, so one
            // fact stays one diagnostic.
            if let Some(assignment) = xtypes.member_id {
                let origin = assignment.origin.as_str();
                if let Some((_, other, other_origin)) = seen
                    .iter()
                    .find(|(id, _, _)| *id == assignment.effective)
                    .cloned()
                {
                    let message = crate::lower::member_id_collision(
                        &name,
                        origin,
                        &other,
                        other_origin,
                        assignment.effective,
                    );
                    self.diagnose_at(
                        DiagnosticCode::DuplicateMemberId,
                        at,
                        &message,
                        "DDS-XTypes 1.3 §7.3.1.2.1.1",
                    );
                } else {
                    seen.push((assignment.effective, name.clone(), origin));
                }
            }
        }
    }

    // -- DDS-XTypes 1.3 §7.2.2.4.5: inheritance of structures ---------------

    /// The three conditions under which a structure may derive from
    /// another: its extensibility is unspecified or the base's, no member
    /// repeats a name or member id of the base chain, and it defines no key
    /// fields of its own.
    ///
    /// Measured before these rules (`zerodds-idlc check`): all three were
    /// accepted without a diagnostic. Cyclone DDS 11.0.1 refuses a key in a
    /// derived structure ("Derived struct is not allowed to expand on
    /// keys"), and so does Fast DDS; every ZeroDDS column of the
    /// `key-in-base-struct` corpus row wrote bytes for it.
    fn inheritance_rules(&mut self, id: Id<TypeNode>, span: Span) {
        const SPEC: &str = "DDS-XTypes 1.3 §7.2.2.4.5";
        let Some(base) = self.ast.base_of(id) else {
            return;
        };
        let name = self
            .ast
            .type_(id)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let base_name = self
            .ast
            .type_(base)
            .map(|n| n.name.clone())
            .unwrap_or_default();

        let declared = self
            .ast
            .type_(id)
            .ok()
            .and_then(|n| n.facets.extensibility());
        let inherited = self.ast.effective_extensibility(base);
        if let Some(declared) = declared.filter(|d| *d != inherited) {
            let message = alloc::format!(
                "`{name}` is {} and derives from `{base_name}`, which is {}; a derived \
                 structure states its base's extensibility or none",
                extensibility_word(declared),
                extensibility_word(inherited),
            );
            self.diagnose_at(DiagnosticCode::InvalidInheritance, span, &message, SPEC);
        }

        // Name and member id of every member the base chain contributes.
        let inherited_members: Vec<(String, Option<u32>)> = self
            .ast
            .all_members_of(base)
            .iter()
            .filter_map(|m| {
                let node = self.ast.member(*m).ok()?;
                let id = self
                    .ast
                    .facets(AnyId::Member(*m, node.node_id))
                    .and_then(|f| f.xtypes.as_ref())
                    .and_then(|x| x.member_id.as_ref())
                    .map(|a| a.effective);
                Some((node.name.clone(), id))
            })
            .collect();

        for member in self.ast.members_of(id).to_vec() {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let member_name = node.name.clone();
            let at = self.span_of(member_any);
            let xtypes = self.ast.facets(member_any).and_then(|f| f.xtypes.clone());

            if inherited_members
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(&member_name))
            {
                let message = alloc::format!(
                    "`{member_name}` is declared in `{name}` and inherited from `{base_name}`; \
                     the members of a structure and of all its bases share one set of names"
                );
                self.diagnose_at(DiagnosticCode::DuplicateName, at, &message, SPEC);
            }
            if let Some(effective) = xtypes
                .as_ref()
                .and_then(|x| x.member_id.as_ref())
                .map(|a| a.effective)
            {
                if let Some((other, _)) = inherited_members
                    .iter()
                    .find(|(_, other_id)| *other_id == Some(effective))
                {
                    let message = alloc::format!(
                        "`{member_name}` of `{name}` has member id {effective}, which `{other}` \
                         inherited from `{base_name}` already has"
                    );
                    self.diagnose_at(DiagnosticCode::DuplicateMemberId, at, &message, SPEC);
                }
            }
            if xtypes.as_ref().is_some_and(|x| x.is_key) {
                let message = alloc::format!(
                    "`{member_name}` is a key of `{name}`, which derives from `{base_name}`; a \
                     derived structure defines no key fields, its keys are its root base's"
                );
                self.diagnose_at(DiagnosticCode::InvalidInheritance, at, &message, SPEC);
            }
        }
    }

    // -- §6: try_construct -----------------------------------------------

    /// `TRIM` on a member, where nothing at that position can be
    /// trimmed.
    ///
    /// An array member's annotation governs its **elements** rather than
    /// the array: Table 17's `ARRAY_TYPE` row knows no member-level
    /// behaviour at all, and an array whose bounds must match the
    /// sender's has no length to trim. Every other member is checked
    /// against its own type.
    fn member_trim_is_defined(
        &mut self,
        declared_ref: &Ref<TypeNode>,
        member: AnyId,
        name: &str,
        at: Span,
    ) {
        // The *unresolved* reference, so that a `TRIM` written on the
        // alias the member is declared with is seen. `refuse_trim`
        // resolves the chain again for the type it names.
        let declared = declared_ref.resolved();
        let written = self
            .ast
            .facets(member)
            .and_then(|f| f.xtypes.as_ref())
            .and_then(|x| x.try_construct);
        let array = declared
            .and_then(|id| self.through_aliases(id))
            .filter(|id| {
                matches!(
                    self.ast.type_(*id).map(|node| &node.kind),
                    Ok(TypeKind::Array { .. })
                )
            });
        if let Some(array) = array {
            if self.ast.element_try_construct(array, written) == TryConstruct::Trim {
                let element = match self.ast.type_(array).map(|node| node.kind.clone()) {
                    Ok(TypeKind::Array { element, .. }) => element.resolved(),
                    _ => None,
                };
                self.refuse_trim(element, at, "an array element");
            }
            return;
        }
        if self.ast.effective_try_construct(written, declared) == TryConstruct::Trim {
            let what = alloc::format!("the member `{name}`");
            self.refuse_trim(declared, at, &what);
        }
    }

    /// `TRIM` on a collection's element, where the element is something
    /// that cannot be trimmed.
    fn trim_is_defined(&mut self, collection: Id<TypeNode>, at: Span, what: &str) {
        if self.ast.element_try_construct(collection, None) != TryConstruct::Trim {
            return;
        }
        let element = match &self.ast.type_(collection).map(|node| node.kind.clone()) {
            Ok(TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. }) => {
                element.resolved()
            }
            Ok(TypeKind::Map { value, .. }) => value.resolved(),
            _ => None,
        };
        self.refuse_trim(element, at, what);
    }

    /// Report `TRIM` where Table 11 does not define it.
    ///
    /// Table 11: "This option only applies to elements or members of
    /// type string, wide string, sequence, or map. The behavior when
    /// applied to other element/member types is unspecified and may be
    /// treated as an error."
    ///
    /// Treated as an error, and not quietly as `DISCARD`. A declaration
    /// that asks for `TRIM` on a `long` is asking for something no
    /// receiver can do, and the two implementations that accept it —
    /// RTI, which drops the annotation entirely, and OpenDDS, which
    /// generates its `DISCARD` branch — give the author the opposite of
    /// what was written without saying so. Cyclone refuses it in the
    /// front end and names the reason;
    /// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`
    /// has the message.
    ///
    /// An unresolved type is not refused: the resolve pass reports it,
    /// and guessing here would put a second diagnostic on one fault.
    fn refuse_trim(&mut self, target: Option<Id<TypeNode>>, at: Span, what: &str) {
        let Some(target) = target else {
            return;
        };
        let Some(resolved) = self.through_aliases(target) else {
            return;
        };
        let Ok(node) = self.ast.type_(resolved) else {
            return;
        };
        if TryConstruct::trim_is_defined_for(&node.kind) {
            return;
        }
        let kind = node.kind.as_str();
        let message = alloc::format!(
            "`@try_construct(TRIM)` on {what} of type `{kind}` is not defined: only a \
             string, wstring, sequence or map can be trimmed"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            at,
            &message,
            "DDS-XTypes 1.3 Table 11",
        );
    }

    /// The type an alias chain ends at. §7.2.4.4.3 resolves an alias to
    /// its base type for every rule in the clause.
    pub(crate) fn through_aliases(&self, start: Id<TypeNode>) -> Option<Id<TypeNode>> {
        let mut current = start;
        // Bounded: a `typedef` cycle is a separate fault with its own
        // report, and spinning here would hide it behind a hang.
        for _ in 0..MAX_ALIAS_DEPTH {
            let node = self.ast.type_(current).ok()?;
            let TypeKind::Alias { aliased } = &node.kind else {
                return Some(current);
            };
            current = aliased.resolved()?;
        }
        None
    }

    // -- §7.2.2.4.4.4.8: what may be part of a key ------------------------

    /// A `map` may not be part of a key, however deeply it is reached.
    ///
    /// §7.2.2.4.4.4.8: "Members of type map cannot be included as part
    /// of the key." Nothing in the tree enforced it, and a `map` inside
    /// a key holder is not a byte question but an *instance* question:
    /// the holder is what the KeyHash is computed over, and two samples
    /// whose maps hold the same pairs in a different order hash
    /// differently, so one instance gets two handles.
    ///
    /// # Where the walk goes, and where it stops
    ///
    /// The clause says which members are part of the key, so the walk
    /// follows exactly that:
    ///
    /// * an alias is transparent — "Aliases to one of the previous
    ///   types can also be used as key members";
    /// * an array or sequence carries its elements in — "If a member of
    ///   type array or sequence is marked as a key member of an
    ///   aggregated type T, all the elements in the array or sequence
    ///   shall be considered part of the key of T";
    /// * an aggregate carries only its **own** key members in — "In the
    ///   event that the type K of a key member of a given type T itself
    ///   defines key members, only the key of K, and not any other of
    ///   its members, shall be considered part of the key of T. This
    ///   relationship is recursive".
    ///
    /// It stops at an aggregate that declares no key member of its own.
    /// Read with §7.6.8 Step 1 — "Otherwise do not remove any members" —
    /// every member of such a type is in the key, which would make a
    /// `map` anywhere inside it a refusal here. That reading is the open
    /// question in `docs/OPEN-ITEMS.md` ("A keyed member whose type
    /// declares no `@key` of its own"), it is not what
    /// `zerodds_ast_layout::keyhash::collect_key_members` implements, and
    /// no vendor has been asked. Refusing on a reading this project does
    /// not itself follow would reject IDL whose key holds no map at all.
    ///
    /// # What is not refused
    ///
    /// The clause also lists the types a key *may* include — "primitive,
    /// aggregation, enumeration, bitmask, array, and sequence" — and
    /// `string` is not among them. Its own Example 2 four pages later is
    /// `@key string<12> label`, `ShapeType`'s key in the OMG interop test
    /// is `@key string<128> color`, and Cyclone, RTI and Fast DDS all
    /// accept a string key. The list is incomplete, not exhaustive, and
    /// only the sentence that forbids something is enforced.
    fn key_member_type_rules(&mut self, member: Id<MemberNode>, name: &str, at: Span) {
        let Some(target) = self
            .ast
            .member(member)
            .ok()
            .and_then(|node| node.type_.resolved())
        else {
            return;
        };
        let mut seen = Vec::new();
        let Some(reached) = self.map_in_key(target, &mut seen) else {
            return;
        };
        let message = if reached == KeyReach::Direct {
            alloc::format!("`{name}` is a `map`, which cannot be part of a key")
        } else {
            alloc::format!("the key of `{name}` reaches a `map`, which cannot be part of a key")
        };
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            at,
            &message,
            "DDS-XTypes 1.3 §7.2.2.4.4.4.8",
        );
    }

    /// Whether the key of `target` reaches a `map`, and how directly.
    ///
    /// `seen` is the cycle guard: a keyed member whose type reaches
    /// itself through a sequence is legal IDL, and a walk without one
    /// would not return.
    fn map_in_key(&self, target: Id<TypeNode>, seen: &mut Vec<Id<TypeNode>>) -> Option<KeyReach> {
        if seen.contains(&target) {
            return None;
        }
        seen.push(target);
        let node = self.ast.type_(target).ok()?;
        match &node.kind {
            TypeKind::Map { .. } => Some(KeyReach::Direct),
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .and_then(|next| self.map_in_key(next, seen)),
            TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => element
                .resolved()
                .and_then(|next| self.map_in_key(next, seen))
                .map(|_| KeyReach::Nested),
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let members = self.ast.all_members_of(target);
                for member in &members {
                    let Ok(member_node) = self.ast.member(*member) else {
                        continue;
                    };
                    if !member_node.facets.is_key() {
                        continue;
                    }
                    let Some(member_type) = member_node.type_.resolved() else {
                        continue;
                    };
                    if self.map_in_key(member_type, seen).is_some() {
                        return Some(KeyReach::Nested);
                    }
                }
                None
            }
            // A union's key is its discriminator and nothing else
            // (§7.2.2.4.4.4.8), and a discriminator is integral,
            // character, boolean or an enumeration — never a map.
            _ => None,
        }
    }

    /// In a union only the discriminator may be a key, and no union
    /// member may be optional.
    ///
    /// Table 21 lists `@key` against "Structure Members, Union
    /// discriminator" and `@optional` against "Structure Members" alone;
    /// §7.2.2.4.4.4.7 says it in prose — "Union members, including the
    /// discriminator, shall never be optional."
    ///
    /// Both were accepted and both change bytes. `@key` on a case member
    /// puts that member in the key holder, so an instance handle depends
    /// on which branch is selected; `@optional` on a case member plans a
    /// presence marker no implementation writes, so the discriminator's
    /// value and the member behind it come out of different offsets than
    /// any peer put them at.
    fn union_member_rules(&mut self, id: Id<TypeNode>) {
        for case in self.ast.cases_of(id).to_vec() {
            let Ok(case_node) = self.ast.case(case) else {
                continue;
            };
            let case_any = AnyId::Case(case, case_node.node_id);
            let member = case_node.member;
            let name = self
                .ast
                .member(member)
                .map_or_else(|_| String::from("a case member"), |node| node.name.clone());
            if name.eq_ignore_ascii_case("discriminator") {
                let span = self
                    .ast
                    .member(member)
                    .map(|node| self.span_of(AnyId::Member(member, node.node_id)))
                    .unwrap_or_default();
                let at = if span.end > span.start {
                    span
                } else {
                    self.span_of(case_any)
                };
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    "a union member cannot be named `discriminator`; the name is reserved for \
                     the union's discriminator",
                    "DDS-XTypes 1.3 §7.2.2.4.4.3",
                );
            }
            // A case's annotations are applied to the case node, which
            // is where `lower.rs` puts them, and the member behind it
            // carries none of its own. Both are read so the rule does
            // not depend on which side a future change writes to.
            //
            // The span comes from whichever of the two has one. A
            // diagnostic with an empty range points at byte zero of the
            // file, which is the one place the mistake is not.
            let member_span = self
                .ast
                .member(member)
                .map(|node| self.span_of(AnyId::Member(member, node.node_id)))
                .unwrap_or_default();
            let case_span = self.span_of(case_any);
            let at = if case_span.end > case_span.start {
                case_span
            } else {
                member_span
            };
            let member_xtypes = self
                .ast
                .member(member)
                .ok()
                .and_then(|node| {
                    self.ast
                        .facets(AnyId::Member(member, node.node_id))
                        .cloned()
                })
                .and_then(|facets| facets.xtypes);
            let Some(xtypes) = self
                .ast
                .facets(case_any)
                .and_then(|f| f.xtypes.clone())
                .or(member_xtypes)
            else {
                continue;
            };
            if xtypes.is_key {
                let message = alloc::format!(
                    "`{name}` is a union member, and only a union's discriminator may be a key"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.8, Table 21",
                );
            }
            if xtypes.is_optional {
                let message =
                    alloc::format!("`{name}` is a union member, and no union member is optional");
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.7, Table 21",
                );
            }
        }
    }

    // -- §6: bounds, dimensions and precision ----------------------------

    fn bound_is_positive(&mut self, bound: Bound, at: Span, what: &str) {
        let Bound::Const(expr) = bound else {
            return;
        };
        let Ok(value) = self.ast.eval_integral(expr) else {
            return;
        };
        if value <= 0 {
            let message = alloc::format!("{what} must be positive, not `{value}`");
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
    }

    /// `fixed<d,s>` requires `0 < d <= 31` and `0 <= s <= d`.
    /// IDL 4.2 §7.4.1.4.4.
    fn fixed_rules(&mut self, digits: u8, scale: u8, at: Span) {
        if digits == 0 || digits > MAX_FIXED_DIGITS {
            let message = alloc::format!(
                "`fixed` needs between 1 and {MAX_FIXED_DIGITS} digits, not `{digits}`"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
        if scale > digits {
            let message = alloc::format!(
                "a `fixed` scale must not exceed its digits: `{scale}` of `{digits}`"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
    }

    // -- §6: forward declarations ---------------------------------------

    /// A forward declaration must be defined in the same translation
    /// unit. IDL 4.2 §7.4.1.4.5.
    ///
    /// Over both arenas. An interface forward now makes an interface
    /// node rather than a `Structure` placeholder — rule (75) — and a
    /// rule that only walked the types would have stopped reporting the
    /// case it was written for.
    fn forward_declarations_are_defined(&mut self) {
        let unfinished: Vec<(AnyId, String)> = self
            .ast
            .types()
            .filter(|(_, node)| node.completeness == zerodds_ast::Completeness::Forward)
            .map(|(id, node)| (AnyId::Type(id, node.node_id), node.name.clone()))
            .chain(
                self.ast
                    .interfaces()
                    .filter(|(_, node)| node.completeness == zerodds_ast::Completeness::Forward)
                    .map(|(id, node)| (AnyId::Interface(id, node.node_id), node.name.clone())),
            )
            .collect();

        for (any, name) in unfinished {
            let at = self.span_of(any);
            let message = alloc::format!("`{name}` was declared but never defined in this file");
            self.diagnose_at(
                DiagnosticCode::UndefinedForward,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.5",
            );
        }
    }

    /// The byte range a node came from, empty when it has none.
    fn span_of(&self, any: AnyId) -> Span {
        self.ast
            .facets(any)
            .and_then(|facets| facets.source.span)
            .map_or(Span::new(0, 0), |span| Span::new(span.start, span.end))
    }
}

/// The value range a primitive discriminator admits.
///
/// `None` for kinds a label cannot be compared against numerically.
/// The annotation that states `extensibility`, as a diagnostic names it.
fn extensibility_word(extensibility: zerodds_ast::Extensibility) -> &'static str {
    match extensibility {
        zerodds_ast::Extensibility::Final => "`@final`",
        zerodds_ast::Extensibility::Appendable => "`@appendable`",
        zerodds_ast::Extensibility::Mutable => "`@mutable`",
    }
}

fn primitive_range(kind: PrimitiveKind) -> Option<(i128, i128)> {
    Some(match kind {
        PrimitiveKind::Boolean => (0, 1),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => (0, 255),
        PrimitiveKind::Char => (0, 255),
        PrimitiveKind::WChar => (0, 0xFFFF),
        PrimitiveKind::Int8 => (-128, 127),
        PrimitiveKind::Int16 => (-32_768, 32_767),
        PrimitiveKind::UInt16 => (0, 65_535),
        PrimitiveKind::Int32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveKind::UInt32 => (0, i128::from(u32::MAX)),
        PrimitiveKind::Int64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveKind::UInt64 => (0, i128::from(u64::MAX)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 | PrimitiveKind::Float128 => return None,
    })
}

/// The name of a diagnostic's rule, for a caller listing them.
#[must_use]
pub fn rule_name(code: DiagnosticCode) -> &'static str {
    match code {
        DiagnosticCode::DuplicateName => "duplicate-name",
        DiagnosticCode::AmbiguousName => "ambiguous-name",
        DiagnosticCode::UnresolvedName => "unresolved-name",
        DiagnosticCode::InvalidDiscriminator => "union-discriminator",
        DiagnosticCode::InvalidCaseLabel => "union-case-label",
        DiagnosticCode::ConflictingAnnotations => "conflicting-annotations",
        DiagnosticCode::InvalidBound => "bound-range",
        DiagnosticCode::UndefinedForward => "forward-defined",
        DiagnosticCode::DuplicateMemberId => "member-id-collision",
        DiagnosticCode::InvalidAnnotation => "annotation-argument",
        DiagnosticCode::KeywordCollision => "keyword-collision",
        DiagnosticCode::InvalidInheritance => "inheritance",
        DiagnosticCode::Preprocessing | DiagnosticCode::ErrorDirective => "preprocessor",
        DiagnosticCode::UnexpectedToken
        | DiagnosticCode::MissingToken
        | DiagnosticCode::UnterminatedDeclaration
        | DiagnosticCode::InvalidToken
        | DiagnosticCode::VendorExtension
        | DiagnosticCode::Unsupported => "syntax",
        _ => "unknown",
    }
}
