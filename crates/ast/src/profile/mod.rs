// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Profile validation. Spec §8.2.
//!
//! The same model may be valid under one profile and invalid under
//! another — `map<>` exists in XTypes but not in CORBA 3.3, and ROS 2
//! forbids constructs IDL 4.2 permits. That asymmetry is the point.
//!
//! A violation is reported with its node, its rule and the paragraph the
//! rule comes from. It is never normalised away.

pub mod outside;
pub mod verdict;

pub use outside::Dropped;
pub use verdict::{
    ALL_PROFILES, ConstructKind, FacetProperty, Verdict, construct_verdict, facet_verdict,
};

use crate::facet::VendorKind;
use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use zerodds_ast_core::NodeId;

/// A specification a model can be measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    /// OMG IDL 4.2 core.
    Idl42Core,
    /// DDS-XTypes 1.3.
    XTypes13,
    /// CORBA 3.3.
    Corba33,
    /// ROS 2 interface definitions.
    Ros2,
    /// DDS-XML 1.0.
    DdsXml,
    /// The FlatData zero-copy model.
    FlatData,
    /// A vendor dialect.
    Vendor(VendorKind),
}

impl Profile {
    /// Every non-vendor profile.
    pub const ALL: [Self; 6] = [
        Self::Idl42Core,
        Self::XTypes13,
        Self::Corba33,
        Self::Ros2,
        Self::DdsXml,
        Self::FlatData,
    ];

    /// Profile name, for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idl42Core => "IDL 4.2",
            Self::XTypes13 => "DDS-XTypes 1.3",
            Self::Corba33 => "CORBA 3.3",
            Self::Ros2 => "ROS 2",
            Self::DdsXml => "DDS-XML 1.0",
            Self::FlatData => "FlatData",
            Self::Vendor(_) => "vendor dialect",
        }
    }
}

/// A rule of a profile that a node breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileViolation {
    /// The offending node.
    pub node: NodeId,
    /// Which profile the rule belongs to.
    pub profile: Profile,
    /// Short rule name.
    pub rule: &'static str,
    /// The specification paragraph the rule comes from.
    pub spec_reference: &'static str,
    /// What specifically was wrong.
    pub detail: String,
}

impl fmt::Display for ProfileViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}) at {:?}: {}",
            self.profile.as_str(),
            self.rule,
            self.spec_reference,
            self.node,
            self.detail
        )
    }
}

impl core::error::Error for ProfileViolation {}

impl Ast {
    /// Measure the model against a profile.
    ///
    /// Drives the construct table in [`verdict`], so every type kind and
    /// every facet property has an explicit ruling. Profile-specific
    /// checks that are not a simple yes/no on a construct — bit bounds,
    /// topic resolvability, plain-type reachability — run alongside it.
    ///
    /// # Errors
    /// Every violation found, never only the first.
    pub fn validate(&self, profile: Profile) -> Result<(), Vec<ProfileViolation>> {
        let mut out = Vec::new();

        for (id, node) in self.types() {
            let node_id = node.node_id;

            // FlatData rules on reachability rather than on the type
            // itself, so it is checked separately below.
            if profile != Profile::FlatData {
                let construct = ConstructKind::of(&node.kind);
                if let Verdict::Rejected {
                    rule,
                    spec_reference,
                    detail,
                } = construct_verdict(profile, construct)
                {
                    out.push(ProfileViolation {
                        node: node_id,
                        profile,
                        rule,
                        spec_reference,
                        detail: detail.to_string(),
                    });
                }
            }

            check_facets(self, node.facets_ref(), node_id, profile, &mut out);

            for member in node.kind.members() {
                let Ok(m) = self.member(*member) else {
                    continue;
                };
                check_facets(self, &m.facets, m.node_id, profile, &mut out);
            }

            match profile {
                Profile::XTypes13 => check_xtypes_bounds(self, id, node_id, &mut out),
                Profile::Corba33 => check_corba_shapes(self, id, node_id, &mut out),
                Profile::Ros2 => {
                    let mut seen = alloc::collections::BTreeSet::new();
                    check_ros_shapes(self, id, node_id, &mut seen, &mut out);
                }
                Profile::FlatData => check_flatdata(self, id, node_id, &mut out),
                _ => {}
            }
        }

        if matches!(profile, Profile::DdsXml) {
            check_dds_xml(self, &mut out);
        }

        if out.is_empty() { Ok(()) } else { Err(out) }
    }

    /// Check the completeness rules: nothing left pending, nothing
    /// broken, every forward declaration given a body.
    ///
    /// Deliberately separate from the commit gate: a partially
    /// discovered model passes every structural invariant and fails
    /// this. Spec §8.2.
    ///
    /// # Errors
    /// Every incompleteness found.
    pub fn validate_complete(&self) -> Result<(), Vec<ProfileViolation>> {
        let mut out = Vec::new();
        for (_, node) in self.types() {
            for reference in node.kind.type_refs() {
                if reference.is_pending() {
                    out.push(ProfileViolation {
                        node: node.node_id,
                        profile: Profile::Idl42Core,
                        rule: "no unresolved reference",
                        spec_reference: "zerodds-ast-1.0 §8.2",
                        detail: "a type reference is still pending".to_string(),
                    });
                } else if reference.is_broken() {
                    out.push(ProfileViolation {
                        node: node.node_id,
                        profile: Profile::Idl42Core,
                        rule: "no broken reference",
                        spec_reference: "zerodds-ast-1.0 §8.2",
                        detail: "a type reference could not be resolved".to_string(),
                    });
                }
            }
            if node.completeness == crate::Completeness::Forward {
                out.push(ProfileViolation {
                    node: node.node_id,
                    profile: Profile::Idl42Core,
                    rule: "forward declaration defined",
                    spec_reference: "zerodds-ast-1.0 §8.2",
                    detail: "declared but never defined".to_string(),
                });
            }
        }
        for (id, member) in self.members_all() {
            let _ = id;
            if member.type_.is_pending() || member.type_.is_broken() {
                out.push(ProfileViolation {
                    node: member.node_id,
                    profile: Profile::Idl42Core,
                    rule: "no unresolved reference",
                    spec_reference: "zerodds-ast-1.0 §8.2",
                    detail: "the member's type is not resolved".to_string(),
                });
            }
        }
        if out.is_empty() { Ok(()) } else { Err(out) }
    }

    pub(crate) fn members_all(
        &self,
    ) -> impl Iterator<Item = (zerodds_ast_core::Id<crate::MemberNode>, &crate::MemberNode)> {
        self.members.iter()
    }

    pub(crate) fn topics_all(
        &self,
    ) -> impl Iterator<Item = (zerodds_ast_core::Id<crate::TopicNode>, &crate::TopicNode)> {
        self.topics.iter()
    }

    pub(crate) fn qos_profiles_all(
        &self,
    ) -> impl Iterator<
        Item = (
            zerodds_ast_core::Id<crate::QosProfileNode>,
            &crate::QosProfileNode,
        ),
    > {
        self.qos_profiles.iter()
    }
}

fn violation(
    node: NodeId,
    profile: Profile,
    rule: &'static str,
    spec_reference: &'static str,
    detail: &str,
) -> ProfileViolation {
    ProfileViolation {
        node,
        profile,
        rule,
        spec_reference,
        detail: detail.to_string(),
    }
}

/// Rule on every facet property a node carries.
fn check_facets(
    ast: &Ast,
    facets: &crate::Facets,
    node_id: NodeId,
    profile: Profile,
    out: &mut Vec<ProfileViolation>,
) {
    let _ = ast;
    for property in FacetProperty::ALL {
        if !carries(facets, property) {
            continue;
        }
        if let Verdict::Rejected {
            rule,
            spec_reference,
            detail,
        } = facet_verdict(profile, property)
        {
            out.push(ProfileViolation {
                node: node_id,
                profile,
                rule,
                spec_reference,
                detail: detail.to_string(),
            });
        }
    }
}

/// Whether a node actually carries a facet property.
fn carries(facets: &crate::Facets, property: FacetProperty) -> bool {
    let xtypes = facets.xtypes.as_ref();
    match property {
        // Both positions Table 21 gives `@key`: a structure member, and
        // a union discriminator. A check that looked only at the member
        // would let `union U switch (@key K)` through a profile that
        // refuses keys altogether.
        FacetProperty::Key => facets.is_key() || xtypes.is_some_and(|x| x.discriminator_is_key),
        FacetProperty::Optional => facets.is_optional(),
        FacetProperty::External => xtypes.is_some_and(|x| x.is_external),
        FacetProperty::MustUnderstand => facets.is_must_understand(),
        FacetProperty::NonSerialized => xtypes.is_some_and(|x| x.is_non_serialized),
        // Any of the four positions counts. A profile that refuses
        // `@try_construct` refuses it wherever it was written, and a
        // check that looked only at the member position would let
        // `sequence<@try_construct(TRIM) T, N>` through a CORBA profile
        // that has no such concept.
        FacetProperty::TryConstruct => xtypes.is_some_and(|x| {
            x.try_construct.is_some()
                || x.element_try_construct.is_some()
                || x.key_try_construct.is_some()
                || x.discriminator_try_construct.is_some()
        }),
        FacetProperty::NonFinalExtensibility => matches!(
            facets.extensibility(),
            Some(crate::Extensibility::Appendable | crate::Extensibility::Mutable)
        ),
        // The *origin* and not the presence. Lowering fills a member id
        // in for every member, because XTypes requires one; the default
        // rule is the previous member's id plus one and no source ever
        // wrote it. Asking `member_id.is_some()` therefore rejected
        // every aggregate ever lowered — `struct Sample { long a; }`
        // came back with "member ids were introduced by XTypes", once
        // per member, and CORBA 3.3 admitted 2 of the 376 corpus cases
        // where the doc comment on [`FacetProperty::ExplicitMemberId`]
        // says the property is `@id`, `@hashid`, `@autoid`.
        FacetProperty::ExplicitMemberId => xtypes.is_some_and(|x| {
            x.member_id.as_ref().is_some_and(|id| {
                !matches!(
                    id.origin,
                    crate::facet::xtypes::MemberIdOrigin::PreviousPlusOne
                )
            })
        }),
        FacetProperty::DefaultValue => xtypes.is_some_and(|x| x.default_value.is_some()),
        FacetProperty::RangeConstraint => xtypes.is_some_and(|x| x.range.is_some()),
        FacetProperty::Unit => xtypes.is_some_and(|x| x.unit.is_some()),
        FacetProperty::Verbatim => xtypes.is_some_and(|x| !x.verbatim.is_empty()),
        FacetProperty::Ros2DefaultValue => facets
            .ros2
            .as_ref()
            .is_some_and(|r| r.default_value.is_some()),
        FacetProperty::VendorDirective => !facets.vendor.is_empty(),
    }
}

/// Bit-bound rules, which are a range check rather than a yes/no on a
/// construct. DDS-XTypes 1.3 §7.3.1.2.1.9 and §7.3.1.2.1.10.
/// Three CORBA rules the construct table cannot express, because they
/// are about a construct's *shape* rather than its kind.
///
/// `ConstructKind` has one `Structure` for a structure with a base and
/// one without, and no category at all for an enumeration's bit bound
/// or an enumerator's value. So the table admitted all three into
/// CORBA 3.3 while `crates/wire-probe/src/corba_idl.rs` — the tool
/// that actually rewrites a type into CORBA IDL — refused them by
/// name. Two opinions about one specification, and the laxer one was
/// the instrument used to decide whether a case belonged to a column
/// at all.
///
/// The rules are the rewriter's, with its clauses:
///
/// - CORBA 3.3 part 1 §7.4.1.4.4.2.2 has no syntax for a structure
///   with a base. Only a valuetype inherits.
/// - part 2 §9.3.2.5 makes an enumeration an unsigned long whose value
///   is the enumerator's ordinal position, so `@bit_bound` and
///   `@value` are both XTypes and both change the wire.
fn check_corba_shapes(
    ast: &Ast,
    id: zerodds_ast_core::Id<crate::TypeNode>,
    node_id: NodeId,
    out: &mut Vec<ProfileViolation>,
) {
    let Ok(node) = ast.type_(id) else {
        return;
    };
    match &node.kind {
        TypeKind::Structure { base: Some(_), .. } => out.push(violation(
            node_id,
            Profile::Corba33,
            "a structure with no base",
            "CORBA 3.3 part 1 §7.4.1.4.4.2.2",
            "CORBA IDL has no syntax for a structure with a base",
        )),
        TypeKind::Enumeration { .. } => {
            if ast.effective_bit_bound(id) != 32 {
                out.push(violation(
                    node_id,
                    Profile::Corba33,
                    "an enumeration in CORBA's holder",
                    "CORBA 3.3 part 2 §9.3.2.5",
                    "an enumeration with a @bit_bound, where CORBA's holder is \
                     always a 32-bit unsigned long",
                ));
            }
            for (position, enumerator) in ast.enumerators_of(id).iter().enumerate() {
                let ordinal = i128::try_from(position).unwrap_or(i128::MAX);
                if ast.effective_enumerator_value(*enumerator) != Some(ordinal) {
                    out.push(violation(
                        node_id,
                        Profile::Corba33,
                        "an enumerator at its ordinal",
                        "CORBA 3.3 part 2 §9.3.2.5",
                        "an enumerator with an explicit @value, where CORBA's is \
                         its ordinal position",
                    ));
                    break;
                }
            }
        }
        _ => {}
    }
}

/// Two ROS rules the construct table cannot express, for the same
/// reason as [`check_corba_shapes`]: they are about a construct's
/// shape and `ConstructKind` has one category for both shapes.
///
/// Both are `crates/wire-probe/src/ros_idl.rs`'s, in its words. A
/// `wchar` inside a collection is deliberately **not** here: that one
/// refuses because `rosidl_typesupport_fastrtps` generates C++ that
/// does not compile, which is a defect in a generator and not an
/// absence in the specification — the profile is right to admit it and
/// the finding belongs to rosidl.
/// How far a chain of typedefs is followed before it is taken for a
/// cycle, matching the depth the resolver uses.
const MAX_ALIAS_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth = distinct reachable type nodes (the
/// `seen` set admits every `Id<TypeNode>` once, so the descent cannot
/// re-enter a type and the AST's node count bounds it)
fn check_ros_shapes(
    ast: &Ast,
    id: zerodds_ast_core::Id<crate::TypeNode>,
    node_id: NodeId,
    seen: &mut alloc::collections::BTreeSet<zerodds_ast_core::Id<crate::TypeNode>>,
    out: &mut Vec<ProfileViolation>,
) {
    // A recursive type — `struct Node { @optional @external Node next;
    // … }` — reaches itself through its members; without this the walk
    // was the stack overflow `profile_matches_the_rewriter` died of,
    // not a verdict of any kind.
    if !seen.insert(id) {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    // A member's collection is often anonymous — `sequence<long>
    // rows[2];` declares an array on the member and no type of its own
    // — so the members are walked as well as the types. Without it the
    // rule fired for a named collection and not for the shape a corpus
    // case actually writes.
    if let TypeKind::Structure { members, .. } = &node.kind {
        for member in members {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            if let Some(inner) = member_node.type_.resolved() {
                check_ros_shapes(ast, inner, node_id, seen, out);
            }
        }
    }
    match &node.kind {
        TypeKind::Structure { base: Some(_), .. } => out.push(violation(
            node_id,
            Profile::Ros2,
            "a structure with no base",
            "ROS 2 interface definition rules",
            "rosidl's IDL grammar has no syntax for a struct with a base",
        )),
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            // Through the aliases: `typedef sequence<long> Ids; Ids
            // rows[3];` is an array whose element is an alias, and
            // stopping at the alias missed the shape the corpus
            // actually writes.
            let mut current = element.resolved();
            let mut nested = false;
            for _ in 0..MAX_ALIAS_DEPTH {
                let Some(inner) = current else { break };
                let Ok(node) = ast.type_(inner) else { break };
                match &node.kind {
                    TypeKind::Sequence { .. } | TypeKind::Array { .. } | TypeKind::Map { .. } => {
                        nested = true;
                        break;
                    }
                    TypeKind::Alias { aliased } => current = aliased.resolved(),
                    _ => break,
                }
            }
            if nested {
                out.push(violation(
                    node_id,
                    Profile::Ros2,
                    "a sequence of a non-collection",
                    "ROS 2 interface definition rules",
                    "a collection whose element is itself a collection, which rosidl \
                     has no shape for",
                ));
            }
        }
        _ => {}
    }
}

fn check_xtypes_bounds(
    ast: &Ast,
    id: zerodds_ast_core::Id<crate::TypeNode>,
    node_id: NodeId,
    out: &mut Vec<ProfileViolation>,
) {
    let Ok(node) = ast.type_(id) else {
        return;
    };
    let bit_bound = node.facets.xtypes.as_ref().and_then(|x| x.bit_bound);

    match &node.kind {
        TypeKind::Enumeration { .. } => {
            if let Some(bound) = bit_bound {
                if bound == 0 || bound > 32 {
                    out.push(violation(
                        node_id,
                        Profile::XTypes13,
                        "enum bit bound in 1..=32",
                        "DDS-XTypes 1.3 §7.3.1.2.1.9",
                        "an enumeration's @bit_bound must be between 1 and 32",
                    ));
                }
            }
        }
        TypeKind::Bitmask { bits } => {
            let bound = bit_bound.unwrap_or(32);
            if bound == 0 || bound > 64 {
                out.push(violation(
                    node_id,
                    Profile::XTypes13,
                    "bitmask bit bound in 1..=64",
                    "DDS-XTypes 1.3 §7.3.1.2.1.10",
                    "a bitmask's @bit_bound must be between 1 and 64",
                ));
            }
            for bit in bits {
                let Ok(b) = ast.bit_value(*bit) else {
                    continue;
                };
                if let Some(position) = b.position {
                    if position >= bound {
                        out.push(violation(
                            b.node_id,
                            Profile::XTypes13,
                            "bit position within bound",
                            "DDS-XTypes 1.3 §7.3.1.2.1.10",
                            "@position is at or beyond the bitmask's bit bound",
                        ));
                    }
                }
            }
        }
        _ => {}
    }
}

/// FlatData rules on what a type can *reach*, not on the type itself, so
/// it walks the graph instead of consulting the table directly.
fn check_flatdata(
    ast: &Ast,
    id: zerodds_ast_core::Id<crate::TypeNode>,
    node_id: NodeId,
    out: &mut Vec<ProfileViolation>,
) {
    let Ok(node) = ast.type_(id) else {
        return;
    };
    // Only a type the frontend marked plain is held to the rule; one
    // that never claimed to be loanable is not in scope.
    if !node
        .facets
        .flatdata
        .as_ref()
        .is_some_and(|f| f.is_plain || f.explicit_fixed_layout)
    {
        return;
    }

    for entry in ast.descendants(AnyId::Type(id, node_id)) {
        let Some(type_id) = entry.as_type() else {
            continue;
        };
        let Ok(reachable) = ast.type_(type_id) else {
            continue;
        };
        let construct = ConstructKind::of(&reachable.kind);
        if let Verdict::Rejected {
            rule,
            spec_reference,
            detail,
        } = construct_verdict(Profile::FlatData, construct)
        {
            out.push(ProfileViolation {
                node: node_id,
                profile: Profile::FlatData,
                rule,
                spec_reference,
                detail: detail.to_string(),
            });
        }
        check_facets(ast, &reachable.facets, node_id, Profile::FlatData, out);
        for member in reachable.kind.members() {
            if let Ok(m) = ast.member(*member) {
                check_facets(ast, &m.facets, node_id, Profile::FlatData, out);
            }
        }
    }
}

fn check_dds_xml(ast: &Ast, out: &mut Vec<ProfileViolation>) {
    for (id, topic) in ast.topics_all() {
        let _ = id;
        if !topic.type_.is_resolved() {
            out.push(violation(
                topic.node_id,
                Profile::DdsXml,
                "topic type resolvable",
                "DDS-XML 1.0 §7.3",
                "a topic must name a type the model holds",
            ));
        }
    }

    // A QoS profile's inheritance chain must terminate.
    for (id, profile) in ast.qos_profiles_all() {
        let mut seen = alloc::collections::BTreeSet::new();
        let _ = seen.insert(profile.node_id);
        let mut current = profile
            .base
            .as_ref()
            .and_then(zerodds_ast_core::Ref::resolved);
        let mut cyclic = false;
        while let Some(next) = current {
            let Ok(node) = ast.qos_profile(next) else {
                break;
            };
            if !seen.insert(node.node_id) {
                cyclic = true;
                break;
            }
            current = node.base.as_ref().and_then(zerodds_ast_core::Ref::resolved);
        }
        if cyclic {
            let _ = id;
            out.push(violation(
                profile.node_id,
                Profile::DdsXml,
                "acyclic profile inheritance",
                "DDS-XML 1.0 §7.5",
                "the profile's base chain closes a cycle",
            ));
        }
    }
}
