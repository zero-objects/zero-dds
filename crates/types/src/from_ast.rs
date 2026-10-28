// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `TypeObject` of a type in the model. XTypes 1.3 §7.3.4.
//!
//! A `TypeObject` is what a peer is told about a type over discovery,
//! and this is the one place it is derived. Nothing here decides
//! anything about a declaration: which extensibility a type has, which
//! id a member carries, what an enumerator's value is and what a bit
//! bound defaults to are all the frontend's answers, read from
//! [`zerodds_ast::Ast`]. A second derivation of any of them would agree
//! with the first only until one of the two changed — which is the shape
//! of every defect this rebuild has found so far.
//!
//! ## What this module owns
//!
//! Two things, and no more. The **shape** §7.3.4 gives a `TypeObject` —
//! which flag bit an extensibility takes, where a member id goes, that a
//! minimal member carries a four-byte name hash where a complete one
//! carries the name. And the **classification** of §7.3.4.6: a type is
//! either plain, so its `TypeIdentifier` describes it outright, or it is
//! not, so the identifier is the hash of its `TypeObject`.
//!
//! ## What it refuses
//!
//! Loudly, with the declaration named:
//!
//! - **A recursive type.** §7.3.4.9 answers it with a strongly connected
//!   component identifier over the whole cycle, which is a different
//!   computation from a single type's hash and is not implemented.
//!   Returning a hash of a partially built object instead would be a
//!   `TypeIdentifier` no peer computes the same way, and two peers
//!   disagreeing about an identifier is worse than either refusing.
//! - **A construct §7.3 has no `TypeIdentifier` for**: `fixed`,
//!   `valuetype`, a value box, `native`, `any`, `ValueBase`, an object
//!   reference. IDL 4.2 declares them and XTypes does not carry them,
//!   so there is no right answer to give.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{
    AnnotationDefNode, Ast, Bound, CharWidth, ConstExprNode, ConstValue, Extensibility,
    MemberIdOrigin, MemberNode, TryConstruct, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref, VisitSet};
use zerodds_float128::Float128;

use crate::hash::{compute_complete_hash, compute_minimal_hash, compute_minimal_hash_with};
use crate::type_identifier::kinds::{EK_BOTH, EK_COMPLETE, EK_MINIMAL};
use crate::type_identifier::{EquivalenceHash, PrimitiveKind, TypeIdentifier};
use crate::type_object::common::{
    AnnotationParameterValue, AppliedAnnotation, AppliedAnnotationParameter,
    AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, AppliedVerbatimAnnotation,
    CommonStructMember, CommonUnionMember, CompleteMemberDetail, CompleteTypeDetail, NameHash,
    OptionalAppliedAnnotationSeq, VerbatimPlacement,
};
use crate::type_object::complete::{
    CompleteAliasBody, CompleteAliasHeader, CompleteAliasType, CompleteAnnotationHeader,
    CompleteAnnotationParameter, CompleteAnnotationType, CompleteEnumeratedHeader,
    CompleteEnumeratedLiteral, CompleteEnumeratedType, CompleteStructHeader, CompleteStructMember,
    CompleteStructType, CompleteUnionType, annotation_parameter_order,
};
use crate::type_object::flags::{
    AliasMemberFlag, AliasTypeFlag, BitfieldFlag, BitflagFlag, BitmaskTypeFlag, BitsetTypeFlag,
    EnumLiteralFlag, EnumTypeFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag,
    UnionMemberFlag, UnionTypeFlag, minimal_member_flags, minimal_type_flags,
    minimal_union_member_flags,
};
use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};
use crate::type_object::minimal::{
    CommonAliasBody, CommonBitfield, CommonBitflag, CommonDiscriminatorMember,
    CommonEnumeratedHeader, CommonEnumeratedLiteral, MinimalAliasBody, MinimalAliasType,
    MinimalBitfield, MinimalBitflag, MinimalBitmaskType, MinimalBitsetType,
    MinimalDiscriminatorMember, MinimalEnumeratedHeader, MinimalEnumeratedLiteral,
    MinimalEnumeratedType, MinimalStructHeader, MinimalStructMember, MinimalStructType,
    MinimalUnionMember, MinimalUnionType,
};
use crate::type_object::minimal::{
    CommonAnnotationParameter, MinimalAnnotationParameter, MinimalAnnotationType,
    ordered_by_name_hash,
};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};

/// The descent's state.
///
/// Carries the path — so a cycle is recognised rather than followed —
/// and, while a strongly connected component is being identified, the
/// component itself. The `push`/`pop`/`contains` surface is the
/// `Vec` this replaced, so the walkers below are unchanged.
struct Descent {
    seen: Vec<Id<TypeNode>>,
    scc: Option<Scc>,
    /// Whether nested minimal `TypeObject` hashes are taken over the
    /// RTI/CoreDX byte stream (one trailing octet per aggregated
    /// member). Carried down the descent so a nested identifier is
    /// hashed in the same camp as its parent. `docs/omg-xtypes-issues.md`
    /// §C2.
    minimal_member_trailing_octet: bool,
}

/// A strongly connected component, in the order XTypes 1.3 §7.3.4.9.2
/// step 4b fixes: lexicographic by fully qualified type name.
struct Scc {
    members: Vec<Id<TypeNode>>,
    /// `EK_MINIMAL` or `EK_COMPLETE` — the `TypeObjectHashId`
    /// discriminator the component identifier carries.
    ///
    /// Annex B puts a `TypeObjectHashId` and not a bare
    /// `EquivalenceHash` in a `StronglyConnectedComponentId`, and
    /// §7.3.4.6 tells the two forms apart by exactly that discriminator.
    /// With the field absent a minimal and a complete component
    /// identifier were the same fourteen octets under the same
    /// discriminator, so a registry could not hold both.
    equivalence_kind: u8,
    /// The component hash, once it is known.
    ///
    /// `None` while the hash **input** is being built. §7.3.4.9.2 step
    /// 4b.i gives a member that points back into the component, before
    /// the component's own hash exists, the temporary identifier
    /// `{TI_STRONGLY_CONNECTED_COMPONENT, sc_component_id = {EK, hash=0},
    /// scc_length, scc_index}`, and that is what the objects of step
    /// 4b.ii are built with.
    ///
    /// No implementation in the interop matrix computes an SCC
    /// identifier at all — Fast DDS declares
    /// `build_strongly_connected_component_id` and `fastddsgen` never
    /// calls it; for a mutually recursive pair it emits
    /// `register_A_type_identifier` calling `register_B` calling
    /// `register_A`. So this hash cannot be checked against another
    /// implementation.
    hash: Option<crate::type_identifier::EquivalenceHash>,
}

impl Descent {
    fn new() -> Self {
        Self {
            seen: Vec::new(),
            scc: None,
            minimal_member_trailing_octet: false,
        }
    }

    fn contains(&self, type_: &Id<TypeNode>) -> bool {
        self.seen.contains(type_)
    }

    fn push(&mut self, type_: Id<TypeNode>) {
        self.seen.push(type_);
    }

    fn pop(&mut self) {
        self.seen.pop();
    }

    /// Whether `type_` is a member of the component being identified.
    fn in_component(&self, type_: Id<TypeNode>) -> bool {
        self.scc
            .as_ref()
            .is_some_and(|scc| scc.members.contains(&type_))
    }

    /// The identifier a component member gets, or `None` when the type
    /// is not in the component being identified.
    fn component_identifier(&self, type_: Id<TypeNode>) -> Option<TypeIdentifier> {
        let scc = self.scc.as_ref()?;
        let index = scc.members.iter().position(|held| *held == type_)?;
        let length = i32::try_from(scc.members.len()).unwrap_or(i32::MAX);
        // Step 4b numbers from one, not from zero.
        let position = i32::try_from(index).unwrap_or(i32::MAX).saturating_add(1);
        // §7.3.4.9.2 step 4b.i: while the objects of the component are
        // built, the identifier of each type in it is temporarily
        // "discriminator = TI_STRONGLY_CONNECTED_COMPONENT, sc_component_id
        // = {discriminator=EK, hash= 0}, scc_length = Number of types in
        // SCC, scc_index = SCCIndex(U)". The hash is zero until step 4e
        // has computed it.
        let hash = scc
            .hash
            .unwrap_or(crate::type_identifier::EquivalenceHash::ZERO);
        Some(TypeIdentifier::StronglyConnectedComponent(
            crate::type_identifier::StronglyConnectedComponentId {
                equivalence_kind: scc.equivalence_kind,
                hash,
                scc_length: length,
                scc_index: position,
            },
        ))
    }
}

/// Every type reachable from `type_`, itself included when it is.
fn reachable(ast: &Ast, type_: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut found: Vec<Id<TypeNode>> = Vec::new();
    let mut queue: Vec<Id<TypeNode>> = Vec::new();
    if let Ok(node) = ast.type_(type_) {
        for reference in node.kind.type_refs() {
            if let Some(target) = reference.resolved() {
                queue.push(target);
            }
        }
        for member in ast.all_members_of(type_) {
            if let Ok(m) = ast.member(member) {
                if let Some(target) = m.type_.resolved() {
                    queue.push(target);
                }
            }
        }
    }
    while let Some(next) = queue.pop() {
        if found.contains(&next) {
            continue;
        }
        found.push(next);
        if let Ok(node) = ast.type_(next) {
            for reference in node.kind.type_refs() {
                if let Some(target) = reference.resolved() {
                    queue.push(target);
                }
            }
        }
        for member in ast.all_members_of(next) {
            if let Ok(m) = ast.member(member) {
                if let Some(target) = m.type_.resolved() {
                    queue.push(target);
                }
            }
        }
    }
    found
}

/// The strongly connected component `type_` belongs to, or `None` when
/// it belongs to none — the ordinary case, where one hash is the
/// identifier.
///
/// A type is in a component when it reaches itself. The component is
/// every type that reaches `type_` and is reached by it, sorted by
/// fully qualified name, which is XTypes 1.3 §7.3.4.9.2 step 4b.
fn strongly_connected_component(ast: &Ast, type_: Id<TypeNode>) -> Option<Vec<Id<TypeNode>>> {
    // A plain collection is in no component (see below), even when the
    // cycle runs through it. Asked about `sequence<Node>` this answered a
    // component of that sequence alone, and `identifier` then wanted a
    // `TypeObject` for a fully descriptive type (Linux test host, five
    // strongly_connected_components tests).
    if ast.type_(type_).is_ok_and(|node| is_plain(&node.kind)) {
        return None;
    }
    let from_here = reachable(ast, type_);
    if !from_here.contains(&type_) {
        return None;
    }
    // A component is made of types that *have* a `TypeObject`. A plain
    // collection — the `sequence<Node>` the recursion runs through — is
    // fully descriptive under §7.3.4.6: its identifier says what it is
    // and no object stands behind it, so it cannot be hashed as a
    // component member. The cycle still runs through it, which is why
    // `reachable` walks it and only membership excludes it.
    let mut members: Vec<Id<TypeNode>> = from_here
        .into_iter()
        .filter(|candidate| {
            !ast.type_(*candidate).is_ok_and(|node| is_plain(&node.kind))
                && (*candidate == type_ || reachable(ast, *candidate).contains(&type_))
        })
        .collect();
    if !members.contains(&type_) {
        members.push(type_);
    }
    members.sort_by_key(|id| qualified_name(ast, *id));
    Some(members)
}

/// Why a type in the model has no `TypeObject`.
///
/// Every variant names the declaration, because a caller lowering a
/// whole IDL file needs to know which one stopped it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FromAstError {
    /// A handle the model does not answer for — a reference left
    /// `Pending` or `Broken` by the resolver, or a stale id.
    Unresolved {
        /// Where the reference sat.
        at: String,
    },
    /// XTypes 1.3 §7.3 defines no `TypeIdentifier` for this construct.
    Unrepresentable {
        /// The declaration's name, or the construct's spelling when it
        /// is anonymous.
        what: String,
        /// Which construct it is.
        kind: &'static str,
    },
    /// The type refers to itself, directly or through other types.
    ///
    /// §7.3.4.9's strongly connected component identifiers are the
    /// answer and are not implemented; see the module documentation.
    Recursive {
        /// The declaration the cycle was entered at.
        what: String,
    },
    /// A bound, dimension, label or enumerator value the model could not
    /// evaluate to an integer.
    NotAConstant {
        /// Where the expression sat.
        at: String,
    },
    /// A value the wire form cannot hold — an array dimension past
    /// `u32::MAX`, an enumerator outside `i32`, a bit position past a
    /// `u16`.
    OutOfRange {
        /// Where the value sat.
        at: String,
        /// What the wire form admits there.
        expected: &'static str,
    },
    /// Serialising the `TypeObject` in order to hash it failed.
    Hash(crate::error::TypeCodecError),
}

impl core::fmt::Display for FromAstError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unresolved { at } => write!(f, "{at}: the reference is not resolved"),
            Self::Unrepresentable { what, kind } => {
                write!(
                    f,
                    "{what}: XTypes 1.3 §7.3 has no TypeIdentifier for {kind}"
                )
            }
            Self::Recursive { what } => write!(
                f,
                "{what}: recursive, and §7.3.4.9's strongly connected component identifiers are not implemented"
            ),
            Self::NotAConstant { at } => write!(f, "{at}: not a constant integer"),
            Self::OutOfRange { at, expected } => write!(f, "{at}: expected {expected}"),
            Self::Hash(e) => write!(f, "serialising the TypeObject to hash it: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FromAstError {}

impl From<crate::error::TypeCodecError> for FromAstError {
    fn from(e: crate::error::TypeCodecError) -> Self {
        Self::Hash(e)
    }
}

impl From<zerodds_wire::WireError> for FromAstError {
    fn from(e: zerodds_wire::WireError) -> Self {
        Self::Hash(crate::error::TypeCodecError::Wire(e))
    }
}

/// Which of the two `TypeObject` forms is being built.
///
/// §7.3.4.1: the minimal form carries a four-byte hash where the
/// complete form carries the name, and nothing else about the two
/// differs structurally. One walk with this as a parameter rather than
/// two walks that could drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// `EK_MINIMAL` — names replaced by their hashes.
    Minimal,
    /// `EK_COMPLETE` — names and annotations carried.
    Complete,
}

impl Form {
    /// The `EquivalenceKind` octet of this form — what a plain
    /// collection's header carries for an element of it (§7.3.4.6), and
    /// what a `StronglyConnectedComponentId`'s `TypeObjectHashId` names
    /// (§7.3.4.9.3).
    const fn equivalence_kind(self) -> u8 {
        match self {
            Self::Minimal => EK_MINIMAL,
            Self::Complete => EK_COMPLETE,
        }
    }
}

/// The `TypeIdentifier` of a type in the model.
///
/// Plain for a primitive, a string, a sequence, an array and a map —
/// §7.3.4.6 calls those "fully descriptive", and their identifier says
/// what they are without a `TypeObject` behind it. For everything else
/// it is the hash of the `TypeObject` this module builds.
///
/// # Errors
/// [`FromAstError`].
pub fn type_identifier(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
) -> Result<TypeIdentifier, FromAstError> {
    type_identifier_with(ast, type_, form, false)
}

/// Like [`type_identifier`], choosing the minimal-`TypeObject` vendor
/// camp for the hash and every nested hash it reaches. `false` is the
/// default Cyclone/Fast DDS camp; `true` the RTI/CoreDX one
/// (`docs/omg-xtypes-issues.md` §C2). Ignored for `Form::Complete`.
///
/// # Errors
/// [`FromAstError`].
pub fn type_identifier_with(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    minimal_member_trailing_octet: bool,
) -> Result<TypeIdentifier, FromAstError> {
    if let Some(members) = strongly_connected_component(ast, type_) {
        return component_identifier(ast, type_, form, &members, minimal_member_trailing_octet);
    }
    let mut seen = Descent::new();
    seen.minimal_member_trailing_octet = minimal_member_trailing_octet;
    identifier(ast, type_, form, &mut seen)
}

/// The `TI_STRONGLY_CONNECTED_COMPONENT` identifier of a type that
/// reaches itself. XTypes 1.3 §7.3.4.9.2.
///
/// Two passes, because the component's hash is needed to write the
/// identifiers that go into the objects the hash is computed over.
///
/// 1. Every member's `TypeObject` is built with the component's hash
///    still unknown, so a reference back into the component carries the
///    temporary identifier of step 4b.i (hash zero).
/// 2. Those objects, in step 4b's order, are serialised as one
///    `sequence<TypeObject>` under XCDR2 — a `DHEADER`, a `UInt32` count
///    and the objects, each on a four-octet boundary — and the first
///    fourteen octets of that stream's MD5 are the component hash.
///
/// # What is measured here and what is not
///
/// The order, the numbering from one, the temporary identifier, the
/// fourteen-octet MD5 and the XCDR2 stream are the specification's. No
/// implementation in the interop matrix computes an SCC identifier, so
/// this hash cannot be checked against one and is not claimed to match
/// any.
fn component_identifier(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    members: &[Id<TypeNode>],
    minimal_member_trailing_octet: bool,
) -> Result<TypeIdentifier, FromAstError> {
    let hash = component_hash(ast, type_, form, members, minimal_member_trailing_octet)?;
    let descent = Descent {
        seen: Vec::new(),
        scc: Some(Scc {
            members: members.to_vec(),
            equivalence_kind: form.equivalence_kind(),
            hash: Some(hash),
        }),
        minimal_member_trailing_octet,
    };
    descent
        .component_identifier(type_)
        .ok_or_else(|| FromAstError::Recursive {
            what: qualified_name(ast, type_),
        })
}

/// The component hash of `members`, per the two passes described on
/// [`component_identifier`].
fn component_hash(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    members: &[Id<TypeNode>],
    minimal_member_trailing_octet: bool,
) -> Result<crate::type_identifier::EquivalenceHash, FromAstError> {
    // Steps 4c to 4e: the objects "in the order of their scc_index", as
    // a `TypeObjectSeq` serialised "using the XCDR serialization for
    // sequences with encoding version 2 and little endian". A sequence of
    // a non-primitive element is a `DHEADER` over a `UInt32` count and
    // the elements (rule (12)); each element is a `TypeObject`, an
    // `@appendable` union with a `DHEADER` of its own, and starts on a
    // four-octet boundary (`MAXALIGN` is four under XCDR2).
    let count = u32::try_from(members.len()).map_err(|_| FromAstError::Recursive {
        what: qualified_name(ast, type_),
    })?;
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&count.to_le_bytes());

    for member in members {
        let mut descent = Descent {
            seen: Vec::new(),
            scc: Some(Scc {
                members: members.to_vec(),
                equivalence_kind: form.equivalence_kind(),
                hash: None,
            }),
            minimal_member_trailing_octet,
        };
        let object = type_object(ast, *member, form, &mut descent)?;
        let bytes = match &object {
            TypeObject::Minimal(minimal) => {
                crate::hash::minimal_hash_input_with(minimal, minimal_member_trailing_octet)
            }
            TypeObject::Complete(complete) => crate::hash::complete_hash_input(complete),
        }
        .map_err(|_| FromAstError::Unrepresentable {
            what: qualified_name(ast, *member),
            kind: "a component member whose TypeObject would not serialise",
        })?;
        while body.len() % 4 != 0 {
            body.push(0);
        }
        body.extend_from_slice(&bytes);
    }

    let length = u32::try_from(body.len()).map_err(|_| FromAstError::Recursive {
        what: qualified_name(ast, type_),
    })?;
    let mut stream: Vec<u8> = Vec::with_capacity(body.len() + 4);
    stream.extend_from_slice(&length.to_le_bytes());
    stream.extend_from_slice(&body);
    Ok(crate::hash::hash_bytes(&stream))
}

/// The `TypeObject` of an `@annotation` declaration — Annex B's
/// `case TK_ANNOTATION: MinimalAnnotationType annotation_type;` and its
/// `CompleteAnnotationType` counterpart.
///
/// An `@annotation` declaration is not a `TypeNode` in this model — it
/// is an `AnnotationDefNode`, so that a use of `@Name` resolves to the
/// declaration and not to a type — and until this function existed the
/// two variants were reachable only by hand: `@annotation A { long v; };`
/// went through the front end and left the model with no type named `A`,
/// so nothing could build one from IDL. That is `tk-annotation` of
/// `docs/spec-coverage/dds-xtypes-1.3.gaps.md` §A.1.
///
/// Each parameter's default value is Annex B's `AnnotationParameterValue`
/// (§7.3.4.5) in its declared kind: the discriminator octet of the
/// parameter's own `TypeKind`, then the value. A parameter the declaration
/// gives no `default` for takes the default value of its *type*, per
/// §7.3.1.2.4 — "If an omitted member does not have a specified default,
/// it will take the default value specified for its type in Clause
/// 7.2.2.4.4.4.7", which is Table 9. The member is not `@optional` in
/// either `MinimalAnnotationParameter` or `CompleteAnnotationParameter`,
/// so there is no octet on the wire for "no default"; the type's default is
/// what goes there.
///
/// The sequences come out in the order §7.3.4.5 requires — by `member_index`
/// (the declaration order) for the complete form
/// ([`crate::type_object::complete::AnnotationParameterOrder`] names the
/// other reading), by name hash for the minimal one — because the
/// `EquivalenceHash` is taken over exactly these octets (§7.3.1.2.1).
///
/// # Errors
/// [`FromAstError`] — an unresolved parameter type, one this crate has no
/// `TypeIdentifier` for, or one whose type's default value it cannot
/// represent.
pub fn annotation_type_object(
    ast: &Ast,
    annotation: Id<AnnotationDefNode>,
    form: Form,
) -> Result<TypeObject, FromAstError> {
    let definition = ast
        .annotation_def(annotation)
        .map_err(|_| FromAstError::Unresolved {
            at: String::from("an annotation declaration"),
        })?;
    let name = definition.name.clone();
    // An annotation applied to an annotation declaration has nowhere to
    // go: Annex B's `MinimalAnnotationType` carries a flag, a header and
    // a parameter sequence, and `CompleteAnnotationType` the same with a
    // `CompleteAnnotationHeader` that holds `annotation_name` alone.
    // Neither declares an `ann_custom`, and §7.3.4.5 does not give one
    // anywhere else, so the application is refused by name rather than
    // left off the object without saying so.
    if let Some(applied) = definition.facets.annotations.first() {
        return Err(FromAstError::Unrepresentable {
            what: alloc::format!("{name}::{}", applied.name),
            kind: "an annotation applied to an annotation declaration, which Annex B has no \
                   member for",
        });
    }
    let mut seen = Descent::new();
    let mut minimal = Vec::with_capacity(definition.members.len());
    let mut complete = Vec::with_capacity(definition.members.len());
    for id in &definition.members {
        let member = ast
            .annotation_member(*id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let at = alloc::format!("{name}::{}", member.name);
        let Ref::Resolved(target) = member.type_ else {
            return Err(FromAstError::Unresolved { at });
        };
        let member_type_id = identifier(ast, target, form, &mut seen)?;
        let default_value = annotation_default_value(ast, member.default, target, &at)?;
        match form {
            Form::Minimal => minimal.push(MinimalAnnotationParameter {
                common: CommonAnnotationParameter {
                    // Annex B's `CommonAnnotationParameter` has no
                    // member id; `AnnotationParameterFlag` is `typedef
                    // MemberFlag` and its comment is "Unused. No flags
                    // apply".
                    member_flags: AnnotationParameterFlag(0),
                    member_type_id,
                },
                name_hash: NameHash::from_name(&member.name),
                default_value: default_value.clone(),
            }),
            Form::Complete => complete.push(CompleteAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_flags: AnnotationParameterFlag(0),
                    member_type_id,
                },
                name: member.name.clone(),
                default_value,
            }),
        }
    }
    Ok(match form {
        Form::Minimal => {
            TypeObject::Minimal(MinimalTypeObject::Annotation(MinimalAnnotationType {
                annotation_flag: AnnotationTypeFlag(0),
                member_seq: ordered_by_name_hash(minimal),
            }))
        }
        Form::Complete => {
            TypeObject::Complete(CompleteTypeObject::Annotation(CompleteAnnotationType {
                annotation_flag: AnnotationTypeFlag(0),
                header: CompleteAnnotationHeader {
                    annotation_name: annotation_qualified_name(ast, annotation),
                },
                member_seq: annotation_parameter_order().ordered(complete),
            }))
        }
    })
}

/// The `TypeIdentifier` of an `@annotation` declaration.
///
/// §7.3.4.2's Table 29 gives `TK_ANNOTATION` one row — "Not plain type.
/// Has TypeObject. Uses Hash of the TypeObject representation of the
/// Annotation declaration" — and Annex B's `union TypeIdentifier switch
/// (octet)` has no case for it: its cases are the four string forms, the
/// six plain-collection forms, `TI_STRONGLY_CONNECTED_COMPONENT`,
/// `EK_COMPLETE` and `EK_MINIMAL`, and a `default:`. A writer that
/// follows Table 29 writes `0x50` and a hash behind it; a reader that
/// follows Annex B takes `0x50` as the `default:` arm, which is an
/// `@extensibility(MUTABLE)` structure and therefore a `DHEADER`, and
/// reads the first four hash octets as a length. `docs/omg-xtypes-issues.md`
/// §B9 writes the contradiction up and this tree follows Annex B.
///
/// What follows from that: the two `EquivalenceKind` cases are the only
/// place a `TypeIdentifier` can carry the fourteen hash octets Table 29
/// asks for, so that is where an annotation declaration's identifier
/// goes. `EK_COMPLETE` is what `applied_annotations` writes, because an
/// `AppliedAnnotation` is only reachable from a `CompleteTypeObject` —
/// `ann_custom` is a member of `CompleteTypeDetail` and of
/// `CompleteMemberDetail` and of no minimal struct, and §7.3.4.4 names
/// "user-defined annotations" among the things the minimal form leaves
/// out.
///
/// # Errors
/// [`FromAstError`] — whatever [`annotation_type_object`] returns for the
/// declaration, plus a hash that could not be computed over it.
pub fn annotation_identifier(
    ast: &Ast,
    annotation: Id<AnnotationDefNode>,
    form: Form,
) -> Result<TypeIdentifier, FromAstError> {
    let hash = annotation_equivalence_hash(ast, annotation, form)?;
    Ok(match form {
        Form::Minimal => TypeIdentifier::EquivalenceHashMinimal(hash),
        Form::Complete => TypeIdentifier::EquivalenceHashComplete(hash),
    })
}

/// The hash Table 29's `TK_ANNOTATION` row names: the first fourteen
/// octets of the MD5 over the `TypeObject` [`annotation_type_object`]
/// builds, which is what every other non-plain type's identifier is over
/// as well (§7.3.4.6).
///
/// # Errors
/// [`FromAstError`].
fn annotation_equivalence_hash(
    ast: &Ast,
    annotation: Id<AnnotationDefNode>,
    form: Form,
) -> Result<EquivalenceHash, FromAstError> {
    match annotation_type_object(ast, annotation, form)? {
        TypeObject::Minimal(minimal) => Ok(compute_minimal_hash(&minimal)?),
        TypeObject::Complete(complete) => Ok(compute_complete_hash(&complete)?),
    }
}

/// The `::`-qualified name of an annotation declaration.
fn annotation_qualified_name(ast: &Ast, annotation: Id<AnnotationDefNode>) -> String {
    let Ok(definition) = ast.annotation_def(annotation) else {
        return String::new();
    };
    ast.scoped_name_of(zerodds_ast::AnyId::AnnotationDef(
        annotation,
        definition.node_id,
    ))
    .map(|scoped| {
        let text = scoped.to_string();
        text.strip_prefix("::").unwrap_or(&text).to_string()
    })
    .unwrap_or_else(|| definition.name.clone())
}

/// An annotation parameter's default value, as Annex B's
/// `AnnotationParameterValue` (Annex B) holds it.
///
/// A declared `default` is evaluated by [`annotation_value`], the same
/// function `@min`/`@max` go through — one derivation of "a constant in its
/// member's own kind" rather than a second one that could disagree. A
/// parameter the declaration says nothing about takes [`table9_default`]:
/// neither `MinimalAnnotationParameter` nor `CompleteAnnotationParameter`
/// declares `default_value` `@optional`, so the wire has no octet for "no
/// default", and §7.3.1.2.4 says what a parameter without one takes.
///
/// # Errors
/// [`FromAstError`] for a default that is not a constant of the parameter's
/// kind, or one of a kind [`table9_default`] cannot represent.
fn annotation_default_value(
    ast: &Ast,
    default: Option<Id<ConstExprNode>>,
    member_type: Id<TypeNode>,
    at: &str,
) -> Result<AnnotationParameterValue, FromAstError> {
    match default {
        Some(expr) => annotation_value(ast, expr, member_type, at),
        None => table9_default(ast, member_type, at),
    }
}

/// The default value of a type, as Table 9 of XTypes 1.3 §7.2.2.4.4.7
/// gives it: zero for every integer and floating point type, `False` for
/// `boolean`, `'\0'` for `char`, `""` for a `string`, and "the first
/// literal in the enumeration" for an enumerated type.
///
/// §7.3.1.2.4 is what points here: "If an omitted member does not have a
/// specified default, it will take the default value specified for its type
/// in Clause 7.2.2.4.4.4.7." An annotation parameter's `default_value` is
/// not `@optional` in Annex B, so an annotation type built from IDL has to
/// put a value in it; a parameter of a kind Table 9 does not give a
/// default for is refused by name instead of being sent as a zero of some
/// kind nobody declared.
///
/// # Errors
/// [`FromAstError::Unrepresentable`] for a kind with no
/// `AnnotationParameterValue` case — everything the union of Annex B has
/// no case for — and
/// [`FromAstError::Unresolved`] for an enumeration whose first literal has
/// no value or one outside `i32`.
fn table9_default(
    ast: &Ast,
    member_type: Id<TypeNode>,
    at: &str,
) -> Result<AnnotationParameterValue, FromAstError> {
    use zerodds_ast::PrimitiveKind as P;
    let refuse = |kind: &'static str| FromAstError::Unrepresentable {
        what: String::from(at),
        kind,
    };
    let effective = strip_aliases(ast, member_type);
    let node = ast.type_(effective).map_err(|_| FromAstError::Unresolved {
        at: String::from(at),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(primitive) => match primitive {
            P::Boolean => AnnotationParameterValue::Boolean(false),
            P::Octet => AnnotationParameterValue::Byte(0),
            P::UInt8 => AnnotationParameterValue::UInt8(0),
            P::Int8 => AnnotationParameterValue::Int8(0),
            P::Int16 => AnnotationParameterValue::Int16(0),
            P::UInt16 => AnnotationParameterValue::UInt16(0),
            P::Int32 => AnnotationParameterValue::Int32(0),
            P::UInt32 => AnnotationParameterValue::UInt32(0),
            P::Int64 => AnnotationParameterValue::Int64(0),
            P::UInt64 => AnnotationParameterValue::UInt64(0),
            P::Float32 => AnnotationParameterValue::Float32Bits(0),
            P::Float64 => AnnotationParameterValue::Float64Bits(0),
            P::Char => AnnotationParameterValue::Char8('\0'),
            P::WChar => AnnotationParameterValue::Char16('\0'),
            P::Float128 => AnnotationParameterValue::Float128Bits(0),
        },
        TypeKind::Enumeration { .. } => {
            let Some(first) = ast.enumerators_of(effective).first().copied() else {
                return Err(FromAstError::Unresolved {
                    at: String::from(at),
                });
            };
            let value = ast.effective_enumerator_value(first).ok_or_else(|| {
                FromAstError::NotAConstant {
                    at: String::from(at),
                }
            })?;
            AnnotationParameterValue::Enum(i32::try_from(value).map_err(|_| {
                FromAstError::OutOfRange {
                    at: String::from(at),
                    expected: "a first enumerator that fits an i32, which is what Annex B writes",
                }
            })?)
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => AnnotationParameterValue::String8(String::new()),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => AnnotationParameterValue::String16(String::new()),
        _ => return Err(refuse("an annotation parameter of this kind")),
    })
}

/// The `MinimalTypeObject` of a type in the model.
///
/// # Errors
/// [`FromAstError`].
pub fn minimal_type_object(
    ast: &Ast,
    type_: Id<TypeNode>,
) -> Result<MinimalTypeObject, FromAstError> {
    minimal_type_object_with(ast, type_, false)
}

/// Like [`minimal_type_object`], choosing the minimal-`TypeObject`
/// vendor camp for this object and every nested hash it embeds
/// (`docs/omg-xtypes-issues.md` §C2). `false` is the default
/// Cyclone/Fast DDS camp.
///
/// # Errors
/// [`FromAstError`].
pub fn minimal_type_object_with(
    ast: &Ast,
    type_: Id<TypeNode>,
    minimal_member_trailing_octet: bool,
) -> Result<MinimalTypeObject, FromAstError> {
    // A type that reaches itself is built with its component in hand,
    // so members pointing back into it render as component identifiers
    // rather than sending the walker round again.
    let mut seen = match strongly_connected_component(ast, type_) {
        Some(members) => {
            let hash = component_hash(
                ast,
                type_,
                Form::Minimal,
                &members,
                minimal_member_trailing_octet,
            )?;
            Descent {
                seen: Vec::new(),
                scc: Some(Scc {
                    members,
                    equivalence_kind: Form::Minimal.equivalence_kind(),
                    hash: Some(hash),
                }),
                minimal_member_trailing_octet,
            }
        }
        None => {
            let mut d = Descent::new();
            d.minimal_member_trailing_octet = minimal_member_trailing_octet;
            d
        }
    };
    match type_object(ast, type_, Form::Minimal, &mut seen)? {
        TypeObject::Minimal(m) => Ok(m),
        TypeObject::Complete(_) => unreachable_form(),
    }
}

/// The `CompleteTypeObject` of a type in the model.
///
/// # Errors
/// [`FromAstError`].
pub fn complete_type_object(
    ast: &Ast,
    type_: Id<TypeNode>,
) -> Result<CompleteTypeObject, FromAstError> {
    // A type that reaches itself is built with its component in hand,
    // so members pointing back into it render as component identifiers
    // rather than sending the walker round again.
    let mut seen = match strongly_connected_component(ast, type_) {
        Some(members) => {
            let hash = component_hash(ast, type_, Form::Complete, &members, false)?;
            Descent {
                seen: Vec::new(),
                scc: Some(Scc {
                    members,
                    equivalence_kind: Form::Complete.equivalence_kind(),
                    hash: Some(hash),
                }),
                minimal_member_trailing_octet: false,
            }
        }
        None => Descent::new(),
    };
    match type_object(ast, type_, Form::Complete, &mut seen)? {
        TypeObject::Complete(c) => Ok(c),
        TypeObject::Minimal(_) => unreachable_form(),
    }
}

/// The strongly connected component `type_` belongs to, in both forms:
/// the objects of its members in the order §7.3.4.9.2 step 4b fixes, and
/// the hash each form is identified by. `None` for a type that is in no
/// component.
///
/// What a recursive topic type hands the runtime (see
/// [`crate::resolve::TypeComponent`]). The COMPLETE and the MINIMAL form
/// are two components of the same members, each with its own hash.
///
/// # Errors
/// [`FromAstError`] for a member whose object could not be built.
pub fn type_component(
    ast: &Ast,
    type_: Id<TypeNode>,
) -> Result<Option<crate::resolve::TypeComponent>, FromAstError> {
    let Some(members) = strongly_connected_component(ast, type_) else {
        return Ok(None);
    };
    let complete_hash = component_hash(ast, type_, Form::Complete, &members, false)?;
    let minimal_hash = component_hash(ast, type_, Form::Minimal, &members, false)?;
    let mut complete = Vec::with_capacity(members.len());
    let mut minimal = Vec::with_capacity(members.len());
    for member in &members {
        complete.push(complete_type_object(ast, *member)?);
        minimal.push(minimal_type_object(ast, *member)?);
    }
    Ok(Some(crate::resolve::TypeComponent {
        complete_hash,
        minimal_hash,
        complete,
        minimal,
    }))
}

/// The `CompleteTypeObject` of every type the object of `type_` names
/// by hash, transitively: the base type of a structure, the aggregate,
/// enumeration, bitmask, union and alias types its members reach,
/// through any depth of nesting, and the `@annotation` declarations the
/// closure applies. `type_` itself is not in the list.
///
/// XTypes 1.3 §7.3.4.6 identifies such a type by the hash of its object
/// and nothing else, so an endpoint that registers only its own object
/// announces an identity whose closure it cannot serve: a peer's
/// `getTypeDependencies` (§7.6.3.3.4) gets an empty answer, and the
/// endpoint's own match site cannot resolve its own type either. The
/// OMG `dds-xtypes` suite measured the second half — an inherited
/// `Shape5Default` whose base was never registered resolved to nothing,
/// and the runtime wired it against every type on the topic.
///
/// An annotation declaration is in that list for the same reason: §7.3.4.5's
/// `AppliedAnnotation::annotation_typeid` **is** the hash of its
/// `TypeObject`, and `crate::resolve::collect_referenced_hashes` counts
/// it as a reference. An endpoint that served the annotated type but not
/// the annotation would announce a dependency it cannot answer for.
///
/// Fully descriptive types (§7.3.4.6) have no object and are not in the
/// list; a type XTypes cannot represent is skipped the way
/// [`register_all`] skips it.
///
/// # Errors
/// [`FromAstError`] for a representable dependency that still could not
/// be built.
pub fn dependency_type_objects(
    ast: &Ast,
    type_: Id<TypeNode>,
) -> Result<Vec<CompleteTypeObject>, FromAstError> {
    let annotations = applied_annotation_declarations(ast, &ast.reachable_types(type_), type_);
    let reachable = closure_with_annotations(ast, type_, &annotations);
    let mut objects = Vec::new();
    for (id, node) in ast.types() {
        if id == type_ || !reachable.contains(node.node_id) || is_plain_in(ast, id, Form::Complete)
        {
            continue;
        }
        match complete_type_object(ast, id) {
            Ok(object) => objects.push(object),
            Err(FromAstError::Unrepresentable { .. } | FromAstError::Recursive { .. }) => {}
            Err(e) => return Err(e),
        }
    }
    for annotation in annotations {
        match annotation_type_object(ast, annotation, Form::Complete)? {
            TypeObject::Complete(object) => objects.push(object),
            // `annotation_type_object` returns the variant its `Form`
            // argument names and the two are decided in one match. The
            // arm is here because the return type is the enum and not
            // because the minimal object is reachable.
            TypeObject::Minimal(_) => {
                return Err(FromAstError::Unrepresentable {
                    what: annotation_qualified_name(ast, annotation),
                    kind: "an annotation declaration in the minimal form, asked for as complete",
                });
            }
        }
    }
    Ok(objects)
}

/// Every `@annotation` declaration applied to a type, to one of its
/// members, branches, literals, bits or bitfields, and every type those
/// declarations' parameters name by hash.
///
/// Both halves are dependencies of the object: §7.3.4.5's
/// `AppliedAnnotation::annotation_typeid` **is** the hash of the
/// declaration's `TypeObject`, and `CommonAnnotationParameter::member_type_id`
/// in that declaration is the hash of the parameter's — an `@annotation`
/// may declare a parameter of an enumerated type, which
/// `zerodds_idl_parse`'s own fixture does. Both are counted as
/// references by [`crate::resolve::collect_referenced_hashes`], and
/// [`register_all`] files both.
///
/// `reachable` is [`Ast::reachable_types`] for the type being closed
/// over and `root` is that type: a declaration applied to the type
/// itself is a dependency of it, which is why the walk starts there
/// rather than at its members.
///
/// **Each declaration is returned once.** It is one `TypeObject` however
/// often it is applied, and listing it twice would put the same bytes in
/// the dependency list twice.
fn applied_annotation_declarations(
    ast: &Ast,
    reachable: &VisitSet,
    root: Id<TypeNode>,
) -> Vec<Id<AnnotationDefNode>> {
    let mut found: Vec<Id<AnnotationDefNode>> = Vec::new();
    let note = |annotations: &[zerodds_ast::AppliedAnnotation], found: &mut Vec<_>| {
        for applied in annotations {
            if let Ref::Resolved(definition) = applied.definition {
                if !found.contains(&definition) {
                    found.push(definition);
                }
            }
        }
    };
    for (id, node) in ast.types() {
        if id != root && !reachable.contains(node.node_id) {
            continue;
        }
        note(&node.facets.annotations, &mut found);
        // The element, key and discriminator positions of §7.2.2.6 and
        // §7.3.1.2.5 are applications of their own: a declaration applied
        // only there is as much a dependency as one applied to a member.
        note(ast.element_annotations(id), &mut found);
        note(ast.key_annotations(id), &mut found);
        note(ast.discriminator_annotations(id), &mut found);
        for member in ast.members_of(id) {
            if let Ok(member) = ast.member(*member) {
                note(&member.facets.annotations, &mut found);
            }
        }
        for case in ast.cases_of(id) {
            if let Ok(case) = ast.case(*case) {
                note(&case.facets.annotations, &mut found);
            }
        }
        for literal in ast.enumerators_of(id) {
            if let Ok(literal) = ast.enumerator(*literal) {
                note(&literal.facets.annotations, &mut found);
            }
        }
        for bit in ast.bits_of(id) {
            if let Ok(bit) = ast.bit_value(*bit) {
                note(&bit.facets.annotations, &mut found);
            }
        }
        for field in ast.all_bitfields_of(id) {
            if let Ok(field) = ast.bitfield(field) {
                note(&field.facets.annotations, &mut found);
            }
        }
    }
    found
}

/// `reachable_types` of `type_`, extended with everything an `@annotation`
/// declaration it applies names: the declarations themselves and, through
/// their parameters, the types those parameters are.
fn closure_with_annotations(
    ast: &Ast,
    type_: Id<TypeNode>,
    annotations: &[Id<AnnotationDefNode>],
) -> VisitSet {
    let mut reachable = ast.reachable_types(type_);
    for annotation in annotations {
        let Ok(declaration) = ast.annotation_def(*annotation) else {
            continue;
        };
        for member in &declaration.members {
            let Ok(member) = ast.annotation_member(*member) else {
                continue;
            };
            let Ref::Resolved(parameter) = member.type_ else {
                continue;
            };
            for node in ast.reachable_types(parameter).iter() {
                reachable.mark(node);
            }
        }
    }
    reachable
}

/// Register every type in the model that has a `TypeObject`, so that
/// [`crate::assignability::is_assignable`] can follow a member's
/// identifier to the type behind it.
///
/// **Every `@annotation` declaration in the model is registered too**, in
/// both forms. §7.3.4.5's `AppliedAnnotation::annotation_typeid` is the
/// hash of the declaration's `TypeObject` — `annotation_typeid` is what a
/// reader follows to learn what `@Unit` is — so an endpoint that
/// registered only its own topic type would write a reference it could
/// not resolve, and `dynamic::to_ast` rebuilding its object would stop
/// at that member. This is the annotation's place in the dependency
/// closure, and it is the same place a base type or an enumeration goes.
///
/// A type the model holds but XTypes cannot represent is skipped rather
/// than fatal: a file that declares one `valuetype` beside twenty
/// structs still has twenty registrable types, and a caller asking about
/// one of the twenty must not be stopped by the twenty-first. What is
/// skipped is returned, so nothing is skipped silently.
///
/// A **fully descriptive** type is not in that list and is not a
/// refusal. §7.3.4.6 gives `long`, `string<8>` and `sequence<long>` a
/// `TypeIdentifier` that says what they are, and no `TypeObject` stands
/// behind one — there is nothing to register and nothing was lost. The
/// model holds a node for each of them because a member has to point
/// somewhere.
///
/// # Errors
/// [`FromAstError`] for a type that is representable and still could not
/// be built — an unresolved reference, a bound that is not a constant.
pub fn register_all(
    ast: &Ast,
    registry: &mut crate::resolve::TypeRegistry,
) -> Result<Vec<(String, FromAstError)>, FromAstError> {
    let mut skipped = Vec::new();
    let ids: Vec<Id<TypeNode>> = ast.types().map(|(id, _)| id).collect();
    let annotations: Vec<Id<AnnotationDefNode>> = ast
        .annotation_defs()
        // The **built-in** declarations live in this arena too, one per
        // annotation IDL 4.2 §8.3 defines, each with the implicit `value`
        // member that makes the `@ann(x)` short form work. That member's
        // type is a `Ref::Pending` no file can settle — it names the
        // annotation, and an annotation is not a type — so a built-in has
        // no `TypeObject` to build. It needs none either: `@key` and its
        // eleven relatives reach typed fields and never reach
        // `Facets::annotations`, which holds only what a user declared
        // with `@annotation`.
        .filter(|(_, node)| node.module != ast.builtin_module())
        .map(|(id, _)| id)
        .collect();
    for id in annotations {
        register_annotation(ast, id, registry, &mut skipped)?;
    }
    for id in ids {
        // A collection whose element carries a declared annotation has a
        // `TypeObject` in the COMPLETE form only (§7.3.4.1); the MINIMAL
        // form leaves the annotation out and the collection is the plain
        // type it is without it.
        if is_annotated_collection(ast, id) {
            let name = collection_type_name(ast, id);
            match complete_type_object(ast, id) {
                Ok(complete) => {
                    registry.insert_complete(compute_complete_hash(&complete)?, complete);
                }
                Err(
                    e @ (FromAstError::Unrepresentable { .. } | FromAstError::Recursive { .. }),
                ) => {
                    skipped.push((name, e));
                }
                Err(e) => return Err(e),
            }
            continue;
        }
        if ast.type_(id).is_ok_and(|node| is_plain(&node.kind)) {
            continue;
        }
        let name = type_name(ast, id);
        match (minimal_type_object(ast, id), complete_type_object(ast, id)) {
            (Ok(minimal), Ok(complete)) => {
                let minimal_hash = compute_minimal_hash(&minimal)?;
                let complete_hash = compute_complete_hash(&complete)?;
                registry.insert_minimal(minimal_hash, minimal);
                registry.insert_complete(complete_hash, complete);

                // A type in a strongly connected component is also
                // reachable by the component's own hash and a position
                // in it, which is the only way a member pointing back
                // into the component can be followed. Registering the
                // type alone would leave that member unresolvable, and a
                // reader would report the component as unknown rather
                // than as recursive.
                if let Some(component) = type_component(ast, id)? {
                    if registry
                        .component_member(&component.complete_hash, 1)
                        .is_none()
                    {
                        // The MINIMAL component is a second whole, with
                        // its own members and its own hash (§7.3.4.9.2
                        // runs once per equivalence kind), and a
                        // COMPLETE component cannot be projected onto it:
                        // `to_minimal` does not follow a component
                        // reference. TypeInformation names both.
                        registry.insert_type_component(&component);
                    }
                }
            }
            (Err(e), _) | (_, Err(e)) => {
                if matches!(
                    e,
                    FromAstError::Unrepresentable { .. } | FromAstError::Recursive { .. }
                ) {
                    skipped.push((name, e));
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(skipped)
}

/// File one `@annotation` declaration's two objects under their hashes.
///
/// Skipped on the same terms as a type — [`register_all`]'s — because a
/// declaration whose parameter has a kind Annex B has no
/// `AnnotationParameterValue` case for is a declaration a peer cannot be
/// told about, and one of those beside twenty registrable types must not
/// stop the twenty.
fn register_annotation(
    ast: &Ast,
    annotation: Id<AnnotationDefNode>,
    registry: &mut crate::resolve::TypeRegistry,
    skipped: &mut Vec<(String, FromAstError)>,
) -> Result<(), FromAstError> {
    let name = ast
        .annotation_def(annotation)
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|_| String::from("<unknown annotation>"));
    match (
        annotation_type_object(ast, annotation, Form::Minimal),
        annotation_type_object(ast, annotation, Form::Complete),
    ) {
        (Ok(TypeObject::Minimal(m)), Ok(TypeObject::Complete(c))) => {
            registry.insert_minimal(compute_minimal_hash(&m)?, m);
            registry.insert_complete(compute_complete_hash(&c)?, c);
        }
        // `annotation_type_object` returns the variant its `Form`
        // argument names and the two are decided in one match inside it,
        // so this pair cannot arrive. The arm is here because the two
        // results are typed as `TypeObject` and not as two different
        // types.
        (Ok(_), Ok(_)) => return unreachable_form(),
        (Err(e), _) | (_, Err(e)) => {
            if matches!(
                e,
                FromAstError::Unrepresentable { .. } | FromAstError::Recursive { .. }
            ) {
                skipped.push((name, e));
            } else {
                return Err(e);
            }
        }
    }
    Ok(())
}

// -- the walk ------------------------------------------------------------

fn unreachable_form<T>() -> Result<T, FromAstError> {
    // `type_object` returns the variant its `Form` argument names; the
    // two are decided in one match and cannot disagree. The arm exists
    // because the return type is the two-variant enum, not because the
    // other variant is reachable.
    Err(FromAstError::Unrepresentable {
        what: String::from("TypeObject"),
        kind: "a form other than the one asked for",
    })
}

fn type_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    ast.type_(type_)
        .map(|node| node.name.clone())
        .unwrap_or_else(|_| String::from("<unknown type>"))
}

/// The fully qualified name a `CompleteTypeObject` carries.
///
/// §7.3.4.5.4's `QualifiedTypeName`, without the leading `::` — Cyclone,
/// Fast DDS and RTI all omit it, which `builder.rs` records as
/// byte-verified.
fn qualified_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    any_type(ast, type_)
        .and_then(|any| ast.scoped_name_of(any))
        .map(|scoped| {
            let text = scoped.to_string();
            text.strip_prefix("::").unwrap_or(&text).to_string()
        })
        .unwrap_or_else(|| type_name(ast, type_))
}

fn any_type(ast: &Ast, type_: Id<TypeNode>) -> Option<zerodds_ast::AnyId> {
    ast.type_(type_)
        .ok()
        .map(|node| zerodds_ast::AnyId::Type(type_, node.node_id))
}

fn target(ast: &Ast, reference: &Ref<TypeNode>, at: &str) -> Result<Id<TypeNode>, FromAstError> {
    reference
        .resolved()
        .ok_or_else(|| FromAstError::Unresolved { at: at.to_string() })
        .and_then(|id| {
            if ast.type_(id).is_ok() {
                Ok(id)
            } else {
                Err(FromAstError::Unresolved { at: at.to_string() })
            }
        })
}

fn bound_of(ast: &Ast, bound: Bound, at: &str) -> Result<u32, FromAstError> {
    let Some(expr) = bound.expr() else {
        // §7.3.4.6: an unbounded collection writes a bound of zero.
        return Ok(0);
    };
    let value = ast
        .eval_integral(expr)
        .map_err(|_| FromAstError::NotAConstant { at: at.to_string() })?;
    u32::try_from(value).map_err(|_| FromAstError::OutOfRange {
        at: at.to_string(),
        expected: "a bound that fits a u32",
    })
}

/// Whether §7.3.4.6 calls this type "fully descriptive" — its
/// `TypeIdentifier` says what it is and no `TypeObject` stands behind
/// it.
fn is_plain(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. }
            | TypeKind::Map { .. }
    )
}

/// Whether a collection's element or key carries the application of a
/// **declared** annotation.
///
/// §7.3.4.1: a Plain Collection is an anonymous collection "that has no
/// annotations beyond @external and @try_construct", and "Non-plain types
/// have both a TypeIdentifier and a TypeObject". An application whose
/// declaration the file does not carry is not written (IDL 4.2
/// §7.4.15.4.2: an unknown annotation "should be ignored"), so it does
/// not make the collection non-plain either.
fn is_annotated_collection(ast: &Ast, type_: Id<TypeNode>) -> bool {
    let is_collection = ast.type_(type_).is_ok_and(|node| {
        matches!(
            node.kind,
            TypeKind::Sequence { .. } | TypeKind::Array { .. } | TypeKind::Map { .. }
        )
    });
    is_collection
        && ast
            .element_annotations(type_)
            .iter()
            .chain(ast.key_annotations(type_))
            .any(|application| matches!(application.definition, Ref::Resolved(_)))
}

/// [`is_plain`] for a node of the model, in one of the two forms.
///
/// The MINIMAL form ignores annotations (§7.3.4.4: it holds "only the
/// information relevant to assignability"), so an annotated collection is
/// the same plain type there that its unannotated twin is; in the COMPLETE
/// form it is a type of its own with a `TypeObject` (§7.3.4.1).
fn is_plain_in(ast: &Ast, type_: Id<TypeNode>, form: Form) -> bool {
    ast.type_(type_).is_ok_and(|node| is_plain(&node.kind))
        && !(form == Form::Complete && is_annotated_collection(ast, type_))
}

/// The `TypeIdentifier` of a type, keeping the descent path in `seen`.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path,
/// via [`identifier_inner`] (`Descent::seen` holds every type the walk
/// is inside; a member pointing back at one of them is answered by the
/// SCC identifier or refused with `FromAstError::Recursive`, so a
/// cyclic type graph cannot re-enter a type)
fn identifier(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeIdentifier, FromAstError> {
    let node = ast.type_(type_).map_err(|_| FromAstError::Unresolved {
        at: String::from("a type handle"),
    })?;
    let name = node.name.clone();

    // A member pointing into the component being identified is answered
    // by the component, not by descending into it again. This is the
    // whole of XTypes 1.3 §7.3.4.9.2 from the walker's side.
    if let Some(component) = seen.component_identifier(type_) {
        return Ok(component);
    }
    // A component reached from outside it — `Holder { B root; }` where
    // `B` and `A` reach each other — has no identifier but the
    // component's either. The walk started at `Holder`, which is in no
    // component, so it holds none; without this it went down into `B`,
    // round through `A` and back into `B`, and refused a type §7.3.4.9
    // has an identifier for.
    if !seen.in_component(type_) {
        if let Some(members) = strongly_connected_component(ast, type_) {
            return component_identifier(
                ast,
                type_,
                form,
                &members,
                seen.minimal_member_trailing_octet,
            );
        }
    }
    if seen.contains(&type_) {
        return Err(FromAstError::Recursive { what: name });
    }
    seen.push(type_);
    let outcome = identifier_inner(ast, type_, form, seen, &name);
    seen.pop();
    outcome
}

/// The body of [`identifier`], with `type_` already pushed onto the
/// descent path.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path
/// (every recursive call goes back through [`identifier`], which pushes
/// and refuses a repeat)
fn identifier_inner(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
    name: &str,
) -> Result<TypeIdentifier, FromAstError> {
    let node = ast.type_(type_).map_err(|_| FromAstError::Unresolved {
        at: name.to_string(),
    })?;
    // An annotated collection is a non-plain type in the COMPLETE form
    // (§7.3.4.1): it is identified by the hash of its `TypeObject` like an
    // aggregate, and the arm at the end of the match builds that.
    let annotated = form == Form::Complete && is_annotated_collection(ast, type_);
    match &node.kind {
        TypeKind::Primitive(primitive) => {
            Ok(TypeIdentifier::Primitive(primitive_kind(*primitive, name)?))
        }
        TypeKind::String { width, bound } => {
            let bound = bound_of(ast, *bound, name)?;
            Ok(match (width, u8::try_from(bound)) {
                (CharWidth::Char, Ok(small)) => TypeIdentifier::String8Small { bound: small },
                (CharWidth::Char, Err(_)) => TypeIdentifier::String8Large { bound },
                (CharWidth::WChar, Ok(small)) => TypeIdentifier::String16Small { bound: small },
                (CharWidth::WChar, Err(_)) => TypeIdentifier::String16Large { bound },
            })
        }
        TypeKind::Sequence { element, bound } if !annotated => {
            let element_id = target(ast, element, name)?;
            let element_ti = identifier(ast, element_id, form, seen)?;
            let header = collection_header(ast, element_id, form, type_);
            let bound = bound_of(ast, *bound, name)?;
            Ok(match u8::try_from(bound) {
                Ok(small) => TypeIdentifier::PlainSequenceSmall {
                    header,
                    bound: small,
                    element: Box::new(element_ti),
                },
                Err(_) => TypeIdentifier::PlainSequenceLarge {
                    header,
                    bound,
                    element: Box::new(element_ti),
                },
            })
        }
        TypeKind::Array { element, dims } if !annotated => {
            let element_id = target(ast, element, name)?;
            let element_ti = identifier(ast, element_id, form, seen)?;
            let header = collection_header(ast, element_id, form, type_);
            let bounds = array_bounds(ast, dims, name)?;
            Ok(if bounds.iter().all(|d| u8::try_from(*d).is_ok()) {
                TypeIdentifier::PlainArraySmall {
                    header,
                    #[allow(clippy::cast_possible_truncation)]
                    array_bounds: bounds.iter().map(|d| *d as u8).collect(),
                    element: Box::new(element_ti),
                }
            } else {
                TypeIdentifier::PlainArrayLarge {
                    header,
                    array_bounds: bounds,
                    element: Box::new(element_ti),
                }
            })
        }
        TypeKind::Map { key, value, bound } if !annotated => {
            let key_id = target(ast, key, name)?;
            let value_id = target(ast, value, name)?;
            let key_ti = identifier(ast, key_id, form, seen)?;
            let value_ti = identifier(ast, value_id, form, seen)?;
            let header = collection_header(ast, value_id, form, type_);
            let key_flags = element_flags(ast.key_try_construct(type_), ast.key_is_external(type_));
            let bound = bound_of(ast, *bound, name)?;
            Ok(match u8::try_from(bound) {
                Ok(small) => TypeIdentifier::PlainMapSmall {
                    header,
                    bound: small,
                    element: Box::new(value_ti),
                    key_flags,
                    key: Box::new(key_ti),
                },
                Err(_) => TypeIdentifier::PlainMapLarge {
                    header,
                    bound,
                    element: Box::new(value_ti),
                    key_flags,
                    key: Box::new(key_ti),
                },
            })
        }
        _ => {
            // Not fully descriptive: the identifier is the hash of the
            // TypeObject, so the TypeObject has to exist first.
            let object = type_object_inner(ast, type_, form, seen)?;
            Ok(match object {
                TypeObject::Minimal(m) => TypeIdentifier::EquivalenceHashMinimal(
                    compute_minimal_hash_with(&m, seen.minimal_member_trailing_octet)?,
                ),
                TypeObject::Complete(c) => {
                    TypeIdentifier::EquivalenceHashComplete(compute_complete_hash(&c)?)
                }
            })
        }
    }
}

/// The header a plain collection carries for its element. §7.3.4.6.
///
/// `equiv_kind` is `EK_BOTH` when the element is itself fully
/// descriptive — a `sequence<long>` has no hash on either side — and the
/// form's own kind otherwise.
fn collection_header(
    ast: &Ast,
    element: Id<TypeNode>,
    form: Form,
    collection: Id<TypeNode>,
) -> crate::type_identifier::PlainCollectionHeader {
    let plain = is_plain_in(ast, element, form);
    crate::type_identifier::PlainCollectionHeader {
        equiv_kind: if plain {
            EK_BOTH
        } else {
            form.equivalence_kind()
        },
        // The behaviour and the `@external` the collection's own element
        // carries (§7.3.1.2.1.12, §7.3.1.2.1.4): both are bits of the
        // `CollectionElementFlag` of the plain collection's identifier.
        element_flags: element_flags(
            ast.element_try_construct(collection, None),
            ast.element_is_external(collection),
        ),
    }
}

fn element_flags(
    behaviour: TryConstruct,
    external: bool,
) -> crate::type_identifier::CollectionElementFlag {
    let mut bits = try_construct_bits(behaviour);
    if external {
        bits |= crate::type_identifier::CollectionElementFlag::IS_EXTERNAL;
    }
    crate::type_identifier::CollectionElementFlag(bits)
}

/// The two `TRY_CONSTRUCT` bits of §7.3.1.2.1.1.
///
/// `DISCARD` is `01`, `USE_DEFAULT` is `10`, `TRIM` is `11`. `00` is
/// undefined and is never written.
const fn try_construct_bits(behaviour: TryConstruct) -> u16 {
    match behaviour {
        TryConstruct::Discard => StructMemberFlag::TRY_CONSTRUCT1,
        TryConstruct::UseDefault => StructMemberFlag::TRY_CONSTRUCT2,
        TryConstruct::Trim => StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2,
    }
}

fn primitive_kind(
    primitive: zerodds_ast::PrimitiveKind,
    at: &str,
) -> Result<PrimitiveKind, FromAstError> {
    use zerodds_ast::PrimitiveKind as Ast;
    Ok(match primitive {
        Ast::Boolean => PrimitiveKind::Boolean,
        // XTypes 1.3 §7.3.4.2 gives `octet` and `uint8` two distinct
        // type kinds — `TK_BYTE` (0x02) and `TK_UINT8` (0x0D) — and
        // §7.2.4.4.4.4 Table 15 makes a primitive assignable only to
        // its own kind. Merging them announced `TK_BYTE` for a
        // `uint8` member, which is wire-visible in the TypeIdentifier
        // octet and in every hash derived from it, and made the two
        // types assignable to each other before Table 15 was asked.
        Ast::Octet => PrimitiveKind::Byte,
        Ast::UInt8 => PrimitiveKind::UInt8,
        Ast::Char => PrimitiveKind::Char8,
        Ast::WChar => PrimitiveKind::Char16,
        Ast::Int8 => PrimitiveKind::Int8,
        Ast::Int16 => PrimitiveKind::Int16,
        Ast::UInt16 => PrimitiveKind::UInt16,
        Ast::Int32 => PrimitiveKind::Int32,
        Ast::UInt32 => PrimitiveKind::UInt32,
        Ast::Int64 => PrimitiveKind::Int64,
        Ast::UInt64 => PrimitiveKind::UInt64,
        Ast::Float32 => PrimitiveKind::Float32,
        Ast::Float64 => PrimitiveKind::Float64,
        Ast::Float128 => {
            // TK_FLOAT128 exists in §7.3.4.2 and this project cannot
            // produce a value of it — `docs/OPEN-ITEMS.md`, the `f128`
            // item. The identifier is representable and is what a peer
            // sending one would announce, so it is built rather than
            // refused; nothing here handles the value.
            let _ = at;
            PrimitiveKind::Float128
        }
    })
}

fn type_object(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    let node = ast.type_(type_).map_err(|_| FromAstError::Unresolved {
        at: String::from("a type handle"),
    })?;
    if seen.contains(&type_) {
        return Err(FromAstError::Recursive {
            what: node.name.clone(),
        });
    }
    seen.push(type_);
    let outcome = type_object_inner(ast, type_, form, seen);
    seen.pop();
    outcome
}

fn type_object_inner(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    let node = ast.type_(type_).map_err(|_| FromAstError::Unresolved {
        at: String::from("a type handle"),
    })?;
    let name = node.name.clone();
    match &node.kind {
        TypeKind::Structure { .. } => structure(ast, type_, form, seen),
        TypeKind::Union { .. } => union(ast, type_, form, seen),
        TypeKind::Enumeration { .. } => enumeration(ast, type_, form),
        TypeKind::Bitmask { .. } => bitmask(ast, type_, form),
        TypeKind::Bitset { .. } => bitset(ast, type_, form),
        TypeKind::Alias { aliased } => alias(ast, type_, aliased.clone(), form, seen),
        TypeKind::Sequence { .. } | TypeKind::Array { .. } | TypeKind::Map { .. }
            if form == Form::Complete && is_annotated_collection(ast, type_) =>
        {
            collection_object(ast, type_, seen)
        }
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. } => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "a fully descriptive type, which has a TypeIdentifier and no TypeObject",
        }),
        TypeKind::Fixed { .. } => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "`fixed` — DDSXTY14-28 asks for it and §7.3.4.2 has no kind",
        }),
        TypeKind::Valuetype { .. } => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "`valuetype`",
        }),
        TypeKind::ValueBox { .. } => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "a value box",
        }),
        TypeKind::Native => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "`native`",
        }),
        TypeKind::ObjectRef { .. } => Err(FromAstError::Unrepresentable {
            what: name,
            kind: "an object reference",
        }),
        other => Err(FromAstError::Unrepresentable {
            what: name,
            kind: kind_name(other),
        }),
    }
}

/// The spelling of a kind this module refuses, for the diagnostic.
///
/// A `match` and not a `Debug` rendering: the message a caller reads is
/// part of the interface, and `Debug` on a kind that carries handles
/// would print them.
const fn kind_name(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Any => "`any`",
        TypeKind::ValueBase => "`ValueBase`",
        TypeKind::AnnotationType { .. } => "an annotation type",
        _ => "this construct",
    }
}

// -- annotated collections ------------------------------------------------

/// The name an anonymous collection gets in its `CompleteTypeDetail`.
///
/// §7.3.4.9.2 step 4b names an anonymous type by "the fully-qualified
/// name of the containing type" and "the member name using '.' as the
/// separator, for example `MyModule::MyStruct.myMember`". The same rule
/// serves a collection that is the member of an aggregate or a branch of
/// a union; the target of an alias is named by the alias, and the element
/// or key of another collection by that collection's name and `.element`
/// or `.key`. A collection the model already names (one rebuilt from a
/// peer's `TypeObject`) keeps its name.
fn collection_type_name(ast: &Ast, collection: Id<TypeNode>) -> String {
    if ast
        .type_(collection)
        .is_ok_and(|node| !node.name.is_empty())
    {
        return qualified_name(ast, collection);
    }
    for (_, member) in ast.members() {
        if member.type_.resolved() == Some(collection) {
            if let Some(owner) = member.parent.as_type() {
                return alloc::format!("{}.{}", qualified_name(ast, owner), member.name);
            }
        }
    }
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        let holds = |reference: &Ref<TypeNode>| reference.resolved() == Some(collection);
        match &node.kind {
            TypeKind::Alias { aliased } if holds(aliased) => return qualified_name(ast, id),
            TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. }
                if holds(element) =>
            {
                return alloc::format!("{}.element", qualified_name(ast, id));
            }
            TypeKind::Map { key, value, .. } if holds(key) || holds(value) => {
                let part = if holds(key) { "key" } else { "element" };
                return alloc::format!("{}.{part}", qualified_name(ast, id));
            }
            _ => {}
        }
    }
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        let holds = |reference: &Ref<TypeNode>| reference.resolved() == Some(collection);
        if matches!(&node.kind, TypeKind::Union { discriminator, .. } if holds(discriminator)) {
            return alloc::format!("{}.discriminator", qualified_name(ast, id));
        }
    }
    String::new()
}

/// One element of an annotated collection: the identifier of its type,
/// the `@try_construct` and `@external` of its position, and the
/// declared annotations applied to it (Annex B's `ann_custom`).
fn collection_element(
    ast: &Ast,
    reference: &Ref<TypeNode>,
    flags: crate::type_identifier::CollectionElementFlag,
    applications: &[zerodds_ast::AppliedAnnotation],
    at: &str,
    seen: &mut Descent,
) -> Result<crate::type_object::complete::CompleteCollectionElement, FromAstError> {
    let element = target(ast, reference, at)?;
    Ok(crate::type_object::complete::CompleteCollectionElement {
        common: crate::type_object::minimal::CommonCollectionElement {
            element_flags: crate::type_object::flags::CollectionElementFlag(flags.0),
            type_id: identifier(ast, element, Form::Complete, seen)?,
        },
        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
        ann_custom: applied_annotations(ast, applications, at)?,
    })
}

/// The `CompleteTypeObject` of a collection whose element (or key) carries
/// the application of a declared annotation.
///
/// §7.3.4.1 makes such a collection a non-plain type, which has a
/// `TypeObject` beside its identifier, and Annex B gives
/// `CompleteCollectionElement` the `ann_custom` the annotation lands in.
/// Only the COMPLETE form has this object: the MINIMAL form holds "only
/// the information relevant to assignability" and annotations are not
/// that, so there the collection stays the plain type it is without them.
fn collection_object(
    ast: &Ast,
    type_: Id<TypeNode>,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    use crate::type_object::complete::{CompleteArrayType, CompleteMapType, CompleteSequenceType};
    use crate::type_object::flags::CollectionTypeFlag;
    let node = ast.type_(type_).map_err(|_| FromAstError::Unresolved {
        at: String::from("a type handle"),
    })?;
    let at = collection_type_name(ast, type_);
    let detail = CompleteTypeDetail {
        type_name: at.clone(),
        ann_builtin: Default::default(),
        ann_custom: OptionalAppliedAnnotationSeq(None),
    };
    // Annex B: the detail of a `CompleteCollectionHeader` is "not present
    // for anonymous". An IDL sequence or map has no name of its own; a node
    // that carries one (a type a `TypeObject` brought in under its detail)
    // keeps it. An array header has a detail in every case.
    let collection_detail = ast
        .type_(type_)
        .is_ok_and(|node| !node.name.is_empty())
        .then(|| detail.clone());
    let element_flags = element_flags(
        ast.element_try_construct(type_, None),
        ast.element_is_external(type_),
    );
    Ok(TypeObject::Complete(match &node.kind {
        TypeKind::Sequence { element, bound } => {
            CompleteTypeObject::Sequence(CompleteSequenceType {
                collection_flag: CollectionTypeFlag(0),
                bound: bound_of(ast, *bound, &at)?,
                detail: collection_detail,
                element: collection_element(
                    ast,
                    element,
                    element_flags,
                    ast.element_annotations(type_),
                    &at,
                    seen,
                )?,
            })
        }
        TypeKind::Array { element, dims } => CompleteTypeObject::Array(CompleteArrayType {
            collection_flag: CollectionTypeFlag(0),
            bound_seq: array_bounds(ast, dims, &at)?,
            detail,
            element: collection_element(
                ast,
                element,
                element_flags,
                ast.element_annotations(type_),
                &at,
                seen,
            )?,
        }),
        TypeKind::Map { key, value, bound } => CompleteTypeObject::Map(CompleteMapType {
            collection_flag: CollectionTypeFlag(0),
            bound: bound_of(ast, *bound, &at)?,
            detail: collection_detail,
            key: collection_element(
                ast,
                key,
                self::element_flags(ast.key_try_construct(type_), ast.key_is_external(type_)),
                ast.key_annotations(type_),
                &at,
                seen,
            )?,
            element: collection_element(
                ast,
                value,
                element_flags,
                ast.element_annotations(type_),
                &at,
                seen,
            )?,
        }),
        _ => {
            return Err(FromAstError::Unrepresentable {
                what: at,
                kind: "a type that is not a collection, asked for as an annotated collection",
            });
        }
    }))
}

/// The dimensions of an array, each an integer that fits a `u32`.
fn array_bounds(ast: &Ast, dims: &[Id<ConstExprNode>], at: &str) -> Result<Vec<u32>, FromAstError> {
    let mut bounds = Vec::with_capacity(dims.len());
    for dim in dims {
        let value = ast
            .eval_integral(*dim)
            .map_err(|_| FromAstError::NotAConstant { at: at.to_string() })?;
        bounds.push(u32::try_from(value).map_err(|_| FromAstError::OutOfRange {
            at: at.to_string(),
            expected: "an array dimension that fits a u32",
        })?);
    }
    Ok(bounds)
}

// -- struct ---------------------------------------------------------------

fn extensibility_bits(extensibility: Extensibility) -> u16 {
    match extensibility {
        Extensibility::Final => StructTypeFlag::IS_FINAL,
        Extensibility::Appendable => StructTypeFlag::IS_APPENDABLE,
        Extensibility::Mutable => StructTypeFlag::IS_MUTABLE,
    }
}

fn aggregate_flags(ast: &Ast, type_: Id<TypeNode>) -> u16 {
    let mut bits = extensibility_bits(ast.effective_extensibility(type_));
    // `@nested` on the type, else the `@default_nested` of the nearest
    // enclosing module that carries one (§7.3.1.2.1.7). Reading only the
    // type's own facet made `@default_nested(TRUE) module M { ... }`
    // emit topic types for everything in `M`.
    if ast.effective_nested(type_) {
        bits |= StructTypeFlag::IS_NESTED;
    }
    // `@autoid(HASH)` on the type, else on the nearest enclosing module
    // (§7.3.1.2.1.1, Table 21).
    if matches!(ast.effective_autoid(type_), Some(zerodds_ast::AutoId::Hash)) {
        bits |= StructTypeFlag::IS_AUTOID_HASH;
    }
    bits
}

fn member_bits(ast: &Ast, member: &MemberNode) -> u16 {
    let behaviour = member
        .facets
        .xtypes
        .as_ref()
        .and_then(|x| x.try_construct)
        .unwrap_or(TryConstruct::ABSENT);
    let mut bits = try_construct_bits(behaviour);
    let Some(facet) = member.facets.xtypes.as_ref() else {
        return bits;
    };
    if facet.is_external {
        bits |= StructMemberFlag::IS_EXTERNAL;
    }
    if facet.is_optional {
        bits |= StructMemberFlag::IS_OPTIONAL;
    }
    if facet.is_must_understand {
        bits |= StructMemberFlag::IS_MUST_UNDERSTAND;
    }
    if facet.is_key {
        bits |= StructMemberFlag::IS_KEY;
        // §7.2.2.4.4.4.8: a key member's must-understand property "shall
        // always be true". No implementation writes it — `docs/omg-
        // xtypes-issues.md` §B3 is that report — and the TypeObject is
        // where the clause is unambiguous, so it is set here.
        bits |= StructMemberFlag::IS_MUST_UNDERSTAND;
    }
    let _ = ast;
    bits
}

fn member_id(member: &MemberNode, fallback: u32) -> u32 {
    member
        .facets
        .xtypes
        .as_ref()
        .and_then(|x| x.effective_member_id())
        .unwrap_or(fallback)
}

/// The declared type behind any `typedef` chain, bounded like every
/// other walk over a reference chain in this workspace.
fn strip_aliases(ast: &Ast, mut type_: Id<TypeNode>) -> Id<TypeNode> {
    for _ in 0..64 {
        let Ok(node) = ast.type_(type_) else {
            return type_;
        };
        let TypeKind::Alias { aliased } = &node.kind else {
            return type_;
        };
        let Some(next) = aliased.resolved() else {
            return type_;
        };
        type_ = next;
    }
    type_
}

/// `@min`/`@max` as the `AnnotationParameterValue` Annex B stores:
/// the constant evaluated, then held in the member's own kind — the
/// only kind a reader without the IDL can interpret the value in.
fn annotation_value(
    ast: &Ast,
    expr: Id<ConstExprNode>,
    member_type: Id<TypeNode>,
    at: &str,
) -> Result<AnnotationParameterValue, FromAstError> {
    use zerodds_ast::PrimitiveKind as P;
    let value = ast
        .eval_const(expr)
        .map_err(|_| FromAstError::NotAConstant {
            at: String::from(at),
        })?;
    let effective = strip_aliases(ast, member_type);
    let node = ast.type_(effective).map_err(|_| FromAstError::Unresolved {
        at: String::from(at),
    })?;
    let refuse = |kind: &'static str| FromAstError::Unrepresentable {
        what: String::from(at),
        kind,
    };
    let integral = |value: &ConstValue| match value {
        ConstValue::Integer(v) => Ok(*v),
        _ => Err(FromAstError::NotAConstant {
            at: String::from(at),
        }),
    };
    let out_of_range = |expected: &'static str| FromAstError::OutOfRange {
        at: String::from(at),
        expected,
    };
    let float = |value: &ConstValue| -> Result<f64, FromAstError> {
        match value {
            ConstValue::Integer(v) => {
                // Deliberate: a float annotation written as an integer
                // literal. i128 → f64 rounds past 2^53, which is what the
                // wire form holds anyway.
                #[allow(clippy::cast_precision_loss)]
                Ok(*v as f64)
            }
            ConstValue::Float { mantissa, exponent } => {
                let text = match exponent {
                    Some(exponent) => alloc::format!("{mantissa}e{exponent}"),
                    None => mantissa.clone(),
                };
                text.parse::<f64>().map_err(|_| FromAstError::NotAConstant {
                    at: String::from(at),
                })
            }
            _ => Err(FromAstError::NotAConstant {
                at: String::from(at),
            }),
        }
    };
    Ok(match &node.kind {
        TypeKind::Primitive(primitive) => match primitive {
            P::Boolean => match value {
                ConstValue::Boolean(v) => AnnotationParameterValue::Boolean(v),
                _ => {
                    return Err(FromAstError::NotAConstant {
                        at: String::from(at),
                    });
                }
            },
            P::Octet => AnnotationParameterValue::Byte(
                u8::try_from(integral(&value)?).map_err(|_| out_of_range("an octet"))?,
            ),
            P::UInt8 => AnnotationParameterValue::UInt8(
                u8::try_from(integral(&value)?).map_err(|_| out_of_range("a uint8"))?,
            ),
            P::Int8 => AnnotationParameterValue::Int8(
                i8::try_from(integral(&value)?).map_err(|_| out_of_range("an int8"))?,
            ),
            P::Int16 => AnnotationParameterValue::Int16(
                i16::try_from(integral(&value)?).map_err(|_| out_of_range("an int16"))?,
            ),
            P::UInt16 => AnnotationParameterValue::UInt16(
                u16::try_from(integral(&value)?).map_err(|_| out_of_range("a uint16"))?,
            ),
            P::Int32 => AnnotationParameterValue::Int32(
                i32::try_from(integral(&value)?).map_err(|_| out_of_range("an int32"))?,
            ),
            P::UInt32 => AnnotationParameterValue::UInt32(
                u32::try_from(integral(&value)?).map_err(|_| out_of_range("a uint32"))?,
            ),
            P::Int64 => AnnotationParameterValue::Int64(
                i64::try_from(integral(&value)?).map_err(|_| out_of_range("an int64"))?,
            ),
            P::UInt64 => AnnotationParameterValue::UInt64(
                u64::try_from(integral(&value)?).map_err(|_| out_of_range("a uint64"))?,
            ),
            #[allow(clippy::cast_possible_truncation)]
            P::Float32 => AnnotationParameterValue::Float32Bits((float(&value)? as f32).to_bits()),
            P::Float64 => AnnotationParameterValue::Float64Bits(float(&value)?.to_bits()),
            P::Char => match value {
                ConstValue::Char(v) => AnnotationParameterValue::Char8(v),
                _ => {
                    return Err(FromAstError::NotAConstant {
                        at: String::from(at),
                    });
                }
            },
            P::WChar => match value {
                ConstValue::WChar(v) => AnnotationParameterValue::Char16(
                    char::from_u32(v)
                        .filter(|c| u16::try_from(u32::from(*c)).is_ok())
                        .ok_or_else(|| out_of_range("a wchar in the Basic Multilingual Plane"))?,
                ),
                _ => {
                    return Err(FromAstError::NotAConstant {
                        at: String::from(at),
                    });
                }
            },
            P::Float128 => AnnotationParameterValue::Float128Bits(match &value {
                ConstValue::Integer(v) => Float128::try_from_i128(*v)
                    .map_err(|_| out_of_range("an integer a long double holds exactly"))?
                    .to_bits(),
                ConstValue::Float { mantissa, exponent } => {
                    let text = match exponent {
                        Some(exponent) => alloc::format!("{mantissa}e{exponent}"),
                        None => mantissa.clone(),
                    };
                    text.parse::<Float128>()
                        .map_err(|_| FromAstError::NotAConstant {
                            at: String::from(at),
                        })?
                        .to_bits()
                }
                _ => {
                    return Err(FromAstError::NotAConstant {
                        at: String::from(at),
                    });
                }
            }),
        },
        TypeKind::Enumeration { .. } => AnnotationParameterValue::Enum(
            i32::try_from(integral(&value)?).map_err(|_| out_of_range("an enumerator value"))?,
        ),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => match value {
            ConstValue::String(v) => AnnotationParameterValue::String8(v),
            _ => {
                return Err(FromAstError::NotAConstant {
                    at: String::from(at),
                });
            }
        },
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => match value {
            ConstValue::WString(units) => {
                let mut text = String::new();
                for unit in units {
                    text.push(
                        char::from_u32(unit)
                            .ok_or_else(|| out_of_range("a wstring of Unicode scalar values"))?,
                    );
                }
                AnnotationParameterValue::String16(text)
            }
            _ => {
                return Err(FromAstError::NotAConstant {
                    at: String::from(at),
                });
            }
        },
        _ => return Err(refuse("`@min`/`@max` on a member of this kind")),
    })
}

/// `@default`, rendered as the text `AppliedBuiltinMemberAnnotations`
/// carries; the reader converts it back against the member's type.
fn default_text(ast: &Ast, expr: Id<ConstExprNode>, at: &str) -> Result<String, FromAstError> {
    let value = ast
        .eval_const(expr)
        .map_err(|_| FromAstError::NotAConstant {
            at: String::from(at),
        })?;
    Ok(match value {
        ConstValue::Boolean(true) => String::from("TRUE"),
        ConstValue::Boolean(false) => String::from("FALSE"),
        ConstValue::Integer(v) => alloc::format!("{v}"),
        ConstValue::Float { mantissa, exponent } => match exponent {
            Some(exponent) => alloc::format!("{mantissa}e{exponent}"),
            None => mantissa,
        },
        ConstValue::Fixed { digits, scale } => fixed_text(&digits, scale),
        ConstValue::Char(v) => String::from(v),
        ConstValue::String(v) => v,
        // Wide values have no faithful narrow-text form — §7.3.4.5.4's
        // record is a narrow string — and `ConstValue` is `#[non_exhaustive]`:
        // a value kind this match does not know is refused, never guessed.
        _ => {
            return Err(FromAstError::Unrepresentable {
                what: String::from(at),
                kind: "`@default` with a value no narrow text carries",
            });
        }
    })
}

/// A fixed literal's digits, with the point put back.
///
/// `pub(crate)`: also `crate::dynamic::default_literal::default_expr_text`'s,
/// which needs the exact same rendering for a `fixed<P,S>` `@default`
/// carried across a graft — one rule for "digits and scale as text",
/// not two.
pub(crate) fn fixed_text(digits: &str, scale: u8) -> String {
    let scale = usize::from(scale);
    if scale == 0 {
        return String::from(digits);
    }
    let padded = if digits.len() <= scale {
        let mut out = String::new();
        for _ in 0..=(scale - digits.len()) {
            out.push('0');
        }
        out.push_str(digits);
        out
    } else {
        String::from(digits)
    };
    let split = padded.len() - scale;
    alloc::format!("{}.{}", &padded[..split], &padded[split..])
}

/// The five builtin member annotations a `CompleteMemberDetail` carries
/// (§7.3.4.5.4), read off the facets the frontend writes them to. A
/// struct member's annotations sit on its `MemberNode`; a union
/// branch's sit on the `CaseNode` the branch was declared through, so
/// the caller hands both and the case's answer wins where both speak.
/// The minimal form (§7.3.4.1) has no place for any of them.
fn builtin_member_annotations(
    ast: &Ast,
    facets: &[Option<&zerodds_ast::XTypesFacet>],
    member_type: Id<TypeNode>,
    at: &str,
) -> Result<AppliedBuiltinMemberAnnotations, FromAstError> {
    let mut out = AppliedBuiltinMemberAnnotations::default();
    let first = |get: fn(&zerodds_ast::XTypesFacet) -> bool| {
        facets.iter().flatten().find(|facet| get(facet)).copied()
    };
    if let Some(facet) = first(|f| f.unit.is_some()) {
        out.unit.clone_from(&facet.unit);
    }
    if let Some(facet) = first(|f| {
        matches!(
            f.member_id,
            Some(zerodds_ast::MemberIdAssignment {
                origin: MemberIdOrigin::Hashed { .. },
                ..
            })
        )
    }) {
        if let Some(zerodds_ast::MemberIdAssignment {
            origin: MemberIdOrigin::Hashed { source_name },
            ..
        }) = &facet.member_id
        {
            out.hash_id = Some(source_name.clone());
        }
    }
    if let Some(facet) = first(|f| f.range.is_some()) {
        if let Some(range) = &facet.range {
            if let Some(expr) = range.min {
                out.min = Some(annotation_value(ast, expr, member_type, at)?);
            }
            if let Some(expr) = range.max {
                out.max = Some(annotation_value(ast, expr, member_type, at)?);
            }
        }
    }
    if let Some(facet) = first(|f| f.default_value.is_some()) {
        if let Some(expr) = facet.default_value {
            out.default_value = Some(default_text(ast, expr, at)?);
        }
    }
    Ok(out)
}

/// Annex B's `@optional AppliedBuiltinTypeAnnotations ann_builtin` of a
/// `CompleteTypeDetail`: the `@verbatim` of the type (§7.3.4.5,
/// `AppliedBuiltinTypeAnnotations`).
///
/// §7.3.4.3 asks that "any non-plain type represented in IDL can be
/// converted to the Complete TypeObject representation and back to IDL with
/// no information loss, other than formatting". `AppliedBuiltinTypeAnnotations`
/// has room for one `AppliedVerbatimAnnotation`, and §7.2.2.4.8 lets a
/// constructed type "refer to one or more" `VerbatimText`s. A type with
/// more than one `@verbatim` has nowhere to put the others, and the object
/// is refused by name rather than written without them.
///
/// # Errors
/// [`FromAstError::Unrepresentable`] for a type with more than one
/// `@verbatim`.
fn type_builtin_annotations(
    facets: &zerodds_ast::Facets,
    at: &str,
) -> Result<AppliedBuiltinTypeAnnotations, FromAstError> {
    let Some(xtypes) = facets.xtypes.as_ref() else {
        return Ok(AppliedBuiltinTypeAnnotations::default());
    };
    match xtypes.verbatim.as_slice() {
        [] => Ok(AppliedBuiltinTypeAnnotations::default()),
        [directive] => Ok(AppliedBuiltinTypeAnnotations {
            verbatim: Some(AppliedVerbatimAnnotation {
                placement: verbatim_placement_of(directive.placement),
                language: directive.language.clone(),
                text: directive.text.clone(),
            }),
        }),
        more => Err(FromAstError::Unrepresentable {
            what: alloc::format!("{at}: {} @verbatim annotations", more.len()),
            kind: "more than one @verbatim on a type, of which Annex B's \
                   AppliedBuiltinTypeAnnotations holds one",
        }),
    }
}

/// The wire placement of a model placement.
fn verbatim_placement_of(placement: zerodds_ast::VerbatimPlacement) -> VerbatimPlacement {
    use zerodds_ast::VerbatimPlacement as Model;
    match placement {
        Model::BeginFile => VerbatimPlacement::BeginFile,
        Model::BeforeDeclaration => VerbatimPlacement::Before,
        Model::BeginDeclaration => VerbatimPlacement::BeginDeclaration,
        Model::EndDeclaration => VerbatimPlacement::EndDeclaration,
        Model::AfterDeclaration => VerbatimPlacement::After,
        Model::EndFile => VerbatimPlacement::EndFile,
    }
}

/// Annex B's `@optional AppliedAnnotationSeq ann_custom`, written from
/// the applications of **declared** annotations the model's facets carry.
///
/// §7.3.4.5's `AppliedAnnotation` names its annotation by
/// `annotation_typeid` — the hash of the declaration's `TypeObject`, as
/// [`annotation_identifier`] derives it — and each of its
/// `AppliedAnnotationParameter`s by the `NameHash` of the parameter's
/// name. §7.3.4.5 orders both sequences, and this is the only place
/// either order is decided: `paramname_hash` ascending inside one
/// application, `annotation_typeid` ascending across them.
///
/// **An application whose declaration the file does not carry is not
/// written, and that is the clause's own instruction.** IDL 4.2
/// §7.4.15.4.2: "In case an unknown annotation is encountered, it should
/// be ignored by the IDL-processing tools." Such an application reaches
/// the model as an `AppliedAnnotation` whose `definition` the resolver
/// could not settle — `@RTI_topic_kind(NO_KEY)` is the case every vendor's
/// IDL file writes — and Annex B has no arm that could carry a typeid for
/// it, because there is no declaration to hash. Refusing the whole
/// `CompleteTypeObject` over one would make a vendor directive fatal; the
/// sequence leaves it out and `Facets::annotations` keeps it, which is
/// what the formatter renders from.
///
/// # Errors
/// [`FromAstError`] for a declaration whose `TypeObject` could not be
/// built or hashed, an argument naming no parameter of the declaration, a
/// positional argument on a declaration with not exactly one parameter, or
/// a value that is not a constant of the parameter's own kind.
fn applied_annotations(
    ast: &Ast,
    applications: &[zerodds_ast::AppliedAnnotation],
    at: &str,
) -> Result<OptionalAppliedAnnotationSeq, FromAstError> {
    let mut out: Vec<(EquivalenceHash, AppliedAnnotation)> = Vec::with_capacity(applications.len());
    for application in applications {
        let Ref::Resolved(definition) = application.definition else {
            continue;
        };
        // A built-in declaration has no `TypeObject` (its implicit `value`
        // member names the annotation itself), so an application of one is
        // not an `ann_custom`. DDS-RPC 1.0's `@Choice`, `@DDSRequestTopic`
        // and the others of its set are built-in here and kept whole in the
        // model for the code that reads them; the XTypes behaviour they
        // imply is in the type's own flags.
        if ast
            .annotation_def(definition)
            .is_ok_and(|node| node.module == ast.builtin_module())
        {
            continue;
        }
        let hash = annotation_equivalence_hash(ast, definition, Form::Complete)?;
        out.push((
            hash,
            AppliedAnnotation {
                annotation_typeid: TypeIdentifier::EquivalenceHashComplete(hash),
                param_seq: applied_parameters(ast, definition, application, at)?,
            },
        ));
    }
    // §7.3.4.5: "The elements in AppliedAnnotationSeq shall be ordered in
    // increasing values of their annotation_typeid." A `TypeIdentifier`
    // is not `Ord` — it is a union over eight shapes — and the hash is
    // the whole of what tells two of these apart, so the sequence is
    // built next to the hash it is ordered by and the identifier is made
    // from that same value. Two applications of one annotation keep the
    // order they were written in: a stable sort, and §7.3.4.5 says
    // nothing about a repeated key.
    out.sort_by(|(left, _), (right, _)| left.cmp(right));
    if out.is_empty() {
        // Absent is a single octet — the `@optional` of §7.4.3.5.3 and
        // what `OptionalAppliedAnnotationSeq` writes for `None`.
        return Ok(OptionalAppliedAnnotationSeq(None));
    }
    Ok(OptionalAppliedAnnotationSeq(Some(
        out.into_iter().map(|(_, applied)| applied).collect(),
    )))
}

/// The `AppliedAnnotationParameter`s one application gives: one per
/// parameter it gave a value for, in increasing `paramname_hash`.
///
/// A parameter the application said nothing about is not written, and its
/// declared default is where that value lives: `CompleteAnnotationParameter`
/// carries `default_value` on the **declaration**, and
/// `annotation_typeid` points at the declaration. Writing the default
/// again would put a value on the wire twice, under two objects, and the
/// two copies could disagree.
///
/// # Errors
/// [`FromAstError`] for an argument that names no parameter of the
/// declaration, a positional argument where the declaration has not
/// exactly one parameter, an unresolved parameter type, or a value that is
/// not a constant of the parameter's own kind.
fn applied_parameters(
    ast: &Ast,
    definition: Id<AnnotationDefNode>,
    application: &zerodds_ast::AppliedAnnotation,
    at: &str,
) -> Result<Vec<AppliedAnnotationParameter>, FromAstError> {
    let declared = ast
        .annotation_def(definition)
        .map_err(|_| FromAstError::Unresolved {
            at: String::from(at),
        })?;
    let members: Vec<(String, Ref<TypeNode>)> = declared
        .members
        .iter()
        .filter_map(|id| {
            let member = ast.annotation_member(*id).ok()?;
            Some((member.name.clone(), member.type_.clone()))
        })
        .collect();
    let mut parameters = Vec::with_capacity(application.arguments.len());
    for argument in &application.arguments {
        let name = match &argument.name {
            // The declaration's own spelling: "the name of an annotation
            // and its members are not case-sensitive" (§7.3.1.2.5), and the
            // parameter is named on the wire by the hash of its name.
            Some(written) => members
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(written))
                .map_or_else(|| written.clone(), |(member, _)| member.clone()),
            // IDL 4.2 §7.4.15.4.2: "In case there is only one member, the
            // annotation application may be shortened to the name of the
            // annotation prefixed with @ and followed with the constant
            // value of that unique member." So a positional argument
            // names the declaration's only parameter, and on the wire it
            // is named by that parameter's hash like any other — the
            // short form is not a second encoding, and nothing is lost
            // by not telling the two apart.
            None => {
                let [only] = members.as_slice() else {
                    return Err(FromAstError::Unresolved {
                        at: alloc::format!(
                            "{at}: `@{}` was applied without naming a parameter, and its \
                             declaration does not have exactly one",
                            application.name
                        ),
                    });
                };
                only.0.clone()
            }
        };
        let parameter_type = members
            .iter()
            .find(|(member, _)| *member == name)
            .map(|(_, declared)| declared.clone())
            .ok_or_else(|| FromAstError::Unresolved {
                at: alloc::format!(
                    "{at}: `@{}` declares no parameter named `{name}`",
                    application.name
                ),
            })?;
        let Ref::Resolved(parameter_type) = parameter_type else {
            return Err(FromAstError::Unresolved {
                at: alloc::format!("{at}: `@{}`'s `{name}`", application.name),
            });
        };
        let value_at = alloc::format!("{at}: `@{}({name})`", application.name);
        parameters.push(AppliedAnnotationParameter {
            paramname_hash: NameHash::from_name(&name),
            value: annotation_value(ast, argument.value, parameter_type, &value_at)?,
        });
    }
    // §7.3.4.5: "The elements in AnnotationParameterSeq shall be ordered
    // in increasing values of their paramname_hash."
    parameters.sort_by(|left, right| left.paramname_hash.cmp(&right.paramname_hash));
    Ok(parameters)
}

fn structure(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    let base = ast.base_of(type_);
    let base_ti = match base {
        Some(base) => identifier(ast, base, form, seen)?,
        None => TypeIdentifier::None,
    };
    let members = ast.members_of(type_).to_vec();

    let mut minimal = Vec::with_capacity(members.len());
    let mut complete = Vec::with_capacity(members.len());
    for (index, id) in members.iter().enumerate() {
        let member = ast
            .member(*id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        // §7.3.1.2.1.14: a `@non_serialized` member is "omitted from
        // serialization", and §7.2.4.4.2 says such members "shall be
        // ignored during type compatibility checking". Leaving it out of
        // the TypeObject is both at once: it is not on the wire and the
        // assignability rules, which read the TypeObject and nothing
        // else, cannot see it.
        //
        // Emitting it made a reader `@final struct R { long a; }` and a
        // writer `@final struct W { long a; @non_serialized long b; }`
        // non-assignable over a member neither of them sends. The member
        // *index* still advances, because a `@non_serialized` member
        // occupies a declaration position and the ids of the members
        // after it are counted from it (§7.3.1.2.1.1's sequential
        // assignment is over the declaration order).
        if member
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_non_serialized)
        {
            continue;
        }
        let at = alloc::format!("{name}::{}", member.name);
        let member_type = target(ast, &member.type_, &at)?;
        let common = CommonStructMember {
            member_id: member_id(member, u32::try_from(index).unwrap_or(u32::MAX)),
            member_flags: StructMemberFlag(member_bits(ast, member)),
            member_type_id: identifier(ast, member_type, form, seen)?,
        };
        match form {
            Form::Minimal => minimal.push(MinimalStructMember {
                common: CommonStructMember {
                    member_flags: StructMemberFlag(minimal_member_flags(common.member_flags.0)),
                    ..common
                },
                detail: NameHash::from_name(&member.name),
            }),
            Form::Complete => complete.push(CompleteStructMember {
                common,
                detail: CompleteMemberDetail {
                    name: member.name.clone(),
                    ann_builtin: builtin_member_annotations(
                        ast,
                        &[member.facets.xtypes.as_ref()],
                        member_type,
                        &at,
                    )?,
                    ann_custom: applied_annotations(ast, &member.facets.annotations, &at)?,
                },
            }),
        }
    }

    Ok(match form {
        Form::Minimal => TypeObject::Minimal(MinimalTypeObject::Struct(MinimalStructType {
            struct_flags: StructTypeFlag(minimal_type_flags(aggregate_flags(ast, type_))),
            header: MinimalStructHeader { base_type: base_ti },
            member_seq: minimal,
        })),
        Form::Complete => TypeObject::Complete(CompleteTypeObject::Struct(CompleteStructType {
            struct_flags: StructTypeFlag(aggregate_flags(ast, type_)),
            header: CompleteStructHeader {
                base_type: base_ti,
                detail: CompleteTypeDetail {
                    type_name: qualified_name(ast, type_),
                    ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                    ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                },
            },
            member_seq: complete,
        })),
    })
}

// -- union ----------------------------------------------------------------

fn union(
    ast: &Ast,
    type_: Id<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    let TypeKind::Union {
        discriminator,
        default_case,
        ..
    } = &node.kind
    else {
        return Err(FromAstError::Unrepresentable {
            what: name,
            kind: "a union that is not a union",
        });
    };
    let default_case = *default_case;
    let discriminator_id = target(ast, discriminator, &name)?;
    let discriminator_ti = identifier(ast, discriminator_id, form, seen)?;
    // §7.2.2.4.4.4.6: the discriminator's must-understand property
    // "shall always be true". `docs/omg-xtypes-issues.md` §B2 reports
    // that no implementation sets it; the clause is unambiguous and the
    // TypeObject is where it can be honoured without changing a byte of
    // any sample.
    // Table 21 permits `@key` on a union discriminator, and the bit it
    // sets is read in two places. §7.2.4.4.4.5 — Table 19's `UNION_TYPE`
    // row — makes it part of assignability: "either the discriminators
    // of both T1 and T2 are keys or neither are keys", and leaving it
    // clear made every keyed discriminator assignable from an unkeyed
    // one. It is also part of the `TypeIdentifier`, which is the point —
    // a keyed and an unkeyed union that hashed alike could not be told
    // apart at discovery.
    let key_bit = if ast.discriminator_is_key(type_) {
        UnionDiscriminatorFlag::IS_KEY
    } else {
        0
    };
    let discriminator_flags = UnionDiscriminatorFlag(
        UnionDiscriminatorFlag::IS_MUST_UNDERSTAND
            | key_bit
            | try_construct_bits(ast.discriminator_try_construct(type_)),
    );

    let cases = ast.cases_of(type_).to_vec();
    let mut minimal = Vec::with_capacity(cases.len());
    let mut complete = Vec::with_capacity(cases.len());
    for (index, case_id) in cases.iter().enumerate() {
        let case = ast
            .case(*case_id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let member = ast
            .member(case.member)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let at = alloc::format!("{name}::{}", member.name);
        let member_type = target(ast, &member.type_, &at)?;

        let mut labels = Vec::new();
        for label in &case.labels {
            let Some(expr) = label.expr() else {
                continue;
            };
            let value = ast
                .eval_integral(expr)
                .map_err(|_| FromAstError::NotAConstant { at: at.clone() })?;
            labels.push(i32::try_from(value).map_err(|_| FromAstError::OutOfRange {
                at: at.clone(),
                expected: "a case label that fits an i32, which is what §7.3.4.5.3.2 writes",
            })?);
        }

        let mut bits = member_bits(ast, member);
        if default_case == Some(*case_id) {
            bits |= UnionMemberFlag::IS_DEFAULT;
        }
        let common = CommonUnionMember {
            // The branches are numbered from **one**, not from the case's
            // index. §7.3.1.2.1.1 numbers the members of an aggregated
            // type from 0 and a union's discriminator is one of them, so
            // it takes 0 and the first branch takes 1 — which is the only
            // numbering under which the discriminator cannot collide with
            // an implicitly numbered branch.
            //
            // It is also what this project's own wire does:
            // `zerodds_ast_layout::xcdr` numbers a `@mutable` union's
            // branches from `MutableUnionFraming::first_branch_id`, which
            // is 1 by default, and RTI and Fast DDS both write `00000020`
            // for the discriminator and `01000020` for the first case
            // member — `crates/ast-layout/tests/fixtures/vendor_bytes.md`
            // §8.
            //
            // Written as the index, this announced `0` for a branch the
            // encoder wrote as `1`: a `TypeObject` saying something the
            // bytes beside it do not. Nothing caught it while no reader
            // built a type back out of a `TypeObject`;
            // `tests/type_object_bridge.rs` is that reader and named the
            // eight slots — `union-mutable` and `evolution-union-
            // unselected`, four modes each.
            member_id: member_id(
                member,
                u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1),
            ),
            member_flags: UnionMemberFlag(bits),
            type_id: identifier(ast, member_type, form, seen)?,
            label_seq: labels,
        };
        match form {
            Form::Minimal => minimal.push(MinimalUnionMember {
                common: CommonUnionMember {
                    member_flags: UnionMemberFlag(minimal_union_member_flags(
                        common.member_flags.0,
                    )),
                    ..common
                },
                detail: NameHash::from_name(&member.name),
            }),
            Form::Complete => {
                // The case's applications before the member's, which is
                // the order `builtin_member_annotations` reads their
                // annotations in: IDL writes a branch's annotations on
                // either the `case` or the member, and where both speak
                // the case is the outer one.
                let mut applied = Vec::new();
                applied.extend_from_slice(&case.facets.annotations);
                applied.extend_from_slice(&member.facets.annotations);
                complete.push(crate::type_object::complete::CompleteUnionMember {
                    common,
                    detail: CompleteMemberDetail {
                        name: member.name.clone(),
                        ann_builtin: builtin_member_annotations(
                            ast,
                            &[case.facets.xtypes.as_ref(), member.facets.xtypes.as_ref()],
                            member_type,
                            &at,
                        )?,
                        ann_custom: applied_annotations(ast, &applied, &at)?,
                    },
                });
            }
        }
    }

    let union_flags = UnionTypeFlag(aggregate_flags(ast, type_));
    Ok(match form {
        Form::Minimal => TypeObject::Minimal(MinimalTypeObject::Union(MinimalUnionType {
            union_flags: UnionTypeFlag(minimal_type_flags(union_flags.0)),
            discriminator: MinimalDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag(minimal_member_flags(
                        discriminator_flags.0,
                    )),
                    type_id: discriminator_ti,
                },
            },
            member_seq: minimal,
        })),
        Form::Complete => TypeObject::Complete(CompleteTypeObject::Union(CompleteUnionType {
            union_flags,
            header: crate::type_object::complete::CompleteUnionHeader {
                detail: CompleteTypeDetail {
                    type_name: qualified_name(ast, type_),
                    ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                    ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                },
            },
            discriminator: crate::type_object::complete::CompleteDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: discriminator_flags,
                    type_id: discriminator_ti,
                },
                ann_builtin: Default::default(),
                // §7.3.1.2.5: annotations "can be applied to the implicit
                // discriminator member of a union type by applying them
                // to the discriminator type declaration in the header of
                // the union type's definition" — `union U switch (@a K)`.
                // `zerodds_idl_lower` keeps them apart from the union's
                // own (`XTypesFacet::discriminator_annotations`), and
                // Annex B's `CompleteDiscriminatorMember` has the slot.
                ann_custom: applied_annotations(
                    ast,
                    ast.discriminator_annotations(type_),
                    &alloc::format!("{name} (discriminator)"),
                )?,
            },
            member_seq: complete,
        })),
    })
}

// -- enumeration ----------------------------------------------------------

fn enumeration(ast: &Ast, type_: Id<TypeNode>, form: Form) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    let bit_bound = ast.effective_bit_bound(type_);
    let enumerators = ast.enumerators_of(type_).to_vec();

    // §7.2.2.4.1.1: the default of an enumeration is its
    // `@default_literal`, else its first literal. The flag names the
    // literal either way: RTI, OpenDDS and Cyclone write `IS_DEFAULT`
    // on the first literal of an unannotated enumeration, and RTI
    // re-derives the type's hash from its own rendering, so an object
    // without the bit is filed under a hash no member references.
    let annotated_default = enumerators.iter().any(|id| {
        ast.enumerator(*id).is_ok_and(|e| {
            e.facets
                .xtypes
                .as_ref()
                .is_some_and(|x| x.is_default_literal)
        })
    });
    let mut minimal = Vec::with_capacity(enumerators.len());
    let mut complete = Vec::with_capacity(enumerators.len());
    for (index, id) in enumerators.iter().enumerate() {
        let enumerator = ast
            .enumerator(*id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let at = alloc::format!("{name}::{}", enumerator.name);
        let value = ast
            .effective_enumerator_value(*id)
            .ok_or_else(|| FromAstError::NotAConstant { at: at.clone() })?;
        let value = i32::try_from(value).map_err(|_| FromAstError::OutOfRange {
            at: at.clone(),
            expected: "an enumerator value that fits an i32, which is what §7.3.4.5.5 writes",
        })?;
        let is_default = if annotated_default {
            enumerator
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|x| x.is_default_literal)
        } else {
            index == 0
        };
        let flags = EnumLiteralFlag(if is_default {
            EnumLiteralFlag::IS_DEFAULT_LITERAL
        } else {
            0
        });
        let common = CommonEnumeratedLiteral { value, flags };
        match form {
            Form::Minimal => minimal.push(MinimalEnumeratedLiteral {
                common,
                detail: NameHash::from_name(&enumerator.name),
            }),
            Form::Complete => complete.push(CompleteEnumeratedLiteral {
                common,
                detail: CompleteMemberDetail {
                    name: enumerator.name.clone(),
                    ann_builtin: Default::default(),
                    ann_custom: applied_annotations(ast, &enumerator.facets.annotations, &at)?,
                },
            }),
        }
    }

    // §7.3.4.5: "The elements in CompleteEnumeratedLiteralSeq shall be
    // ordered in increasing values of their numeric value", and the same
    // for MinimalEnumeratedLiteralSeq. `enum E { @value(7) A, @value(0)
    // B }` is written B first. The sort is stable, so equal values (which
    // validation refuses) keep their declaration order.
    minimal.sort_by_key(|literal: &MinimalEnumeratedLiteral| literal.common.value);
    complete.sort_by_key(|literal: &CompleteEnumeratedLiteral| literal.common.value);

    // Annex B says `EnumTypeFlag` is unused; Table 18 compares the
    // extensibility of two enumerations, and RTI, OpenDDS and Cyclone
    // write the `TypeFlag` extensibility position into the word (Fast
    // DDS writes zero). The bits are written here, and
    // `check_enumerations` treats zero as "not announced" so a Fast DDS
    // enumeration still compares. `docs/omg-xtypes-issues.md` §B8 holds
    // the disagreement between the two clauses.
    //
    // §7.3.1.2.1.11: "@ignore_literal_names ... If the annotation has
    // the value TRUE in both enumerated types, then the literal names
    // are ignored for the purpose of checking the assignability of the
    // two Enumerated Types." "In both" means the annotation travels
    // with the type; `EnumTypeFlag::IGNORE_LITERAL_NAMES` is the bit.
    let mut enum_flags = extensibility_bits(ast.effective_extensibility(type_));
    if ast
        .types()
        .find(|(id, _)| *id == type_)
        .and_then(|(_, node)| node.facets.xtypes.as_ref())
        .is_some_and(|x| x.ignore_literal_names)
    {
        enum_flags |= EnumTypeFlag::IGNORE_LITERAL_NAMES;
    }
    let enum_flags = EnumTypeFlag(enum_flags);

    Ok(match form {
        Form::Minimal => {
            TypeObject::Minimal(MinimalTypeObject::Enumerated(MinimalEnumeratedType {
                enum_flags,
                header: MinimalEnumeratedHeader {
                    common: CommonEnumeratedHeader { bit_bound },
                },
                literal_seq: minimal,
            }))
        }
        Form::Complete => {
            TypeObject::Complete(CompleteTypeObject::Enumerated(CompleteEnumeratedType {
                enum_flags,
                header: CompleteEnumeratedHeader {
                    common: CommonEnumeratedHeader { bit_bound },
                    detail: CompleteTypeDetail {
                        type_name: qualified_name(ast, type_),
                        ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                        ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                    },
                },
                literal_seq: complete,
            }))
        }
    })
}

// -- bitmask --------------------------------------------------------------

fn bitmask(ast: &Ast, type_: Id<TypeNode>, form: Form) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    let bit_bound = ast.effective_bit_bound(type_);
    let bits = ast.bits_of(type_).to_vec();

    let mut minimal = Vec::with_capacity(bits.len());
    let mut complete = Vec::with_capacity(bits.len());
    for id in &bits {
        let bit = ast
            .bit_value(*id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let at = alloc::format!("{name}::{}", bit.name);
        let position = ast
            .effective_bit_position(*id)
            .ok_or_else(|| FromAstError::NotAConstant { at: at.clone() })?;
        let common = CommonBitflag {
            position,
            flags: BitflagFlag(0),
        };
        match form {
            Form::Minimal => minimal.push(MinimalBitflag {
                common,
                detail: NameHash::from_name(&bit.name),
            }),
            Form::Complete => {
                complete.push(crate::type_object::complete::CompleteBitflag {
                    common,
                    detail: CompleteMemberDetail {
                        name: bit.name.clone(),
                        ann_builtin: Default::default(),
                        ann_custom: applied_annotations(ast, &bit.facets.annotations, &at)?,
                    },
                });
            }
        }
    }

    // §7.3.4.5: "The elements in CompleteBitflagSeq shall be ordered in
    // increasing values of their position", and the same for
    // MinimalBitflagSeq: `bitmask M { @position(5) A, @position(1) B }` is
    // written B first.
    minimal.sort_by_key(|flag: &MinimalBitflag| flag.common.position);
    complete
        .sort_by_key(|flag: &crate::type_object::complete::CompleteBitflag| flag.common.position);

    Ok(match form {
        Form::Minimal => TypeObject::Minimal(MinimalTypeObject::Bitmask(MinimalBitmaskType {
            bitmask_flags: BitmaskTypeFlag(0),
            bit_bound,
            flag_seq: minimal,
        })),
        Form::Complete => TypeObject::Complete(CompleteTypeObject::Bitmask(
            crate::type_object::complete::CompleteBitmaskType {
                bitmask_flags: BitmaskTypeFlag(0),
                bit_bound,
                detail: CompleteTypeDetail {
                    type_name: qualified_name(ast, type_),
                    ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                    ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                },
                flag_seq: complete,
            },
        )),
    })
}

// -- bitset ---------------------------------------------------------------

fn bitset(ast: &Ast, type_: Id<TypeNode>, form: Form) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    // §7.3.4.5's `CommonBitfield` is ordered by position and IDL 4.2
    // §7.4.13.4.3.2 assigns positions by packing bitfields consecutively
    // in declaration order, anonymous ones included. `all_bitfields_of`
    // walks the inheritance chain, which is what a bitset with a base
    // needs.
    let fields = ast.all_bitfields_of(type_);

    let mut position: u16 = 0;
    let mut minimal = Vec::new();
    let mut complete = Vec::new();
    for id in &fields {
        let field = ast
            .bitfield(*id)
            .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
        let start = position;
        position = position
            .checked_add(field.bits)
            .ok_or_else(|| FromAstError::OutOfRange {
                at: name.clone(),
                expected: "a bitset narrower than 65536 bits",
            })?;
        if field.is_padding() {
            // An anonymous field occupies its bits and is not a member:
            // §7.4.13.4.3.2 counts it in the packing and nothing names
            // it, so it takes a position and no entry.
            continue;
        }
        let field_name = field.name.clone().unwrap_or_default();
        let at = alloc::format!("{name}::{field_name}");
        let bitcount = u8::try_from(field.bits).map_err(|_| FromAstError::OutOfRange {
            at: at.clone(),
            expected: "a bitfield of 1 to 64 bits, which is what `bitcount` holds",
        })?;
        let holder = match &field.holder_type {
            Some(reference) => {
                let holder_id = target(ast, reference, &name)?;
                let holder_node = ast
                    .type_(holder_id)
                    .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
                let TypeKind::Primitive(primitive) = &holder_node.kind else {
                    return Err(FromAstError::Unrepresentable {
                        what: at.clone(),
                        kind: "a bitfield holder that is not a primitive",
                    });
                };
                primitive_kind(*primitive, &name)?.to_u8()
            }
            None => default_holder(field.bits).to_u8(),
        };
        let common = CommonBitfield {
            position: start,
            flags: BitfieldFlag(0),
            bitcount,
            holder_type: holder,
        };
        match form {
            Form::Minimal => minimal.push(MinimalBitfield {
                common,
                name_hash: NameHash::from_name(&field_name),
            }),
            Form::Complete => {
                complete.push(crate::type_object::complete::CompleteBitfield {
                    common,
                    detail: CompleteMemberDetail {
                        name: field_name,
                        ann_builtin: Default::default(),
                        ann_custom: applied_annotations(ast, &field.facets.annotations, &at)?,
                    },
                });
            }
        }
    }

    Ok(match form {
        Form::Minimal => TypeObject::Minimal(MinimalTypeObject::Bitset(MinimalBitsetType {
            bitset_flags: BitsetTypeFlag(0),
            field_seq: minimal,
        })),
        Form::Complete => TypeObject::Complete(CompleteTypeObject::Bitset(
            crate::type_object::complete::CompleteBitsetType {
                bitset_flags: BitsetTypeFlag(0),
                detail: CompleteTypeDetail {
                    type_name: qualified_name(ast, type_),
                    ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                    ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                },
                field_seq: complete,
            },
        )),
    })
}

/// The holder a bitfield takes when the declaration names none.
///
/// IDL 4.2 §7.4.13.4.3.2: the smallest integer that holds the width. A
/// single bit is a `boolean`; the annex's own example runs 3, 1, 4, 10
/// and 12 bits.
const fn default_holder(bits: u16) -> PrimitiveKind {
    match bits {
        1 => PrimitiveKind::Boolean,
        2..=8 => PrimitiveKind::Byte,
        9..=16 => PrimitiveKind::UInt16,
        17..=32 => PrimitiveKind::UInt32,
        _ => PrimitiveKind::UInt64,
    }
}

// -- alias ----------------------------------------------------------------

fn alias(
    ast: &Ast,
    type_: Id<TypeNode>,
    aliased: Ref<TypeNode>,
    form: Form,
    seen: &mut Descent,
) -> Result<TypeObject, FromAstError> {
    let name = type_name(ast, type_);
    let node = ast
        .type_(type_)
        .map_err(|_| FromAstError::Unresolved { at: name.clone() })?;
    let target_id = target(ast, &aliased, &name)?;
    let related_type = identifier(ast, target_id, form, seen)?;
    let common = CommonAliasBody {
        related_flags: AliasMemberFlag(0),
        related_type,
    };
    Ok(match form {
        Form::Minimal => TypeObject::Minimal(MinimalTypeObject::Alias(MinimalAliasType {
            alias_flags: AliasTypeFlag(0),
            body: MinimalAliasBody { common },
        })),
        Form::Complete => TypeObject::Complete(CompleteTypeObject::Alias(CompleteAliasType {
            alias_flags: AliasTypeFlag(0),
            header: CompleteAliasHeader {
                detail: CompleteTypeDetail {
                    type_name: qualified_name(ast, type_),
                    ann_builtin: type_builtin_annotations(&node.facets, &name)?,
                    ann_custom: applied_annotations(ast, &node.facets.annotations, &name)?,
                },
            },
            body: CompleteAliasBody {
                related_flags: common.related_flags,
                related_type: common.related_type,
                ann_builtin: Default::default(),
                // The body is the other `ann_custom` a declaration has:
                // §7.3.1.2.1.2's `@external` on a typedef is what
                // `CompleteAliasBody` is for, and the model holds
                // `@external` as `XTypesFacet::is_external` on the member
                // beside it. A user-declared annotation on a typedef is
                // the declaration's own and went in the header above,
                // beside every other type's; writing it in both places
                // would put one application on the wire twice.
                ann_custom: OptionalAppliedAnnotationSeq(None),
            },
        })),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod primitive_kind_tests {
    use super::{PrimitiveKind, primitive_kind};
    use crate::type_identifier::kinds::{TK_BYTE, TK_UINT8};

    /// `octet` and `uint8` announce two type kinds, not one.
    ///
    /// XTypes 1.3 §7.3.4.2 gives them `TK_BYTE` (0x02) and `TK_UINT8`
    /// (0x0D); OpenDDS maps the same two IDL keywords to the same two
    /// constants (`dds/idl/typeobject_generator.cpp`, `PT_octet` →
    /// `TK_BYTE`, `PT_uint8` → `TK_UINT8`). Merging them announced
    /// `TK_BYTE` for a `uint8` member — wire-visible in the
    /// TypeIdentifier octet and in every hash derived from it — and
    /// made the two types assignable to each other before
    /// §7.2.4.4.4.4 Table 15 was asked. The two OMG `dds-xtypes` cases
    /// `struct_uint8_byte` and `struct_byte_uint8` expect
    /// `INCONSISTENT_TOPIC` and got a match.
    #[test]
    fn octet_and_uint8_are_two_kinds() {
        use zerodds_ast::PrimitiveKind as Ast;
        let octet = primitive_kind(Ast::Octet, "::T::m").expect("octet lowers");
        let uint8 = primitive_kind(Ast::UInt8, "::T::m").expect("uint8 lowers");
        assert_eq!(octet, PrimitiveKind::Byte);
        assert_eq!(uint8, PrimitiveKind::UInt8);
        assert_ne!(octet, uint8);
        assert_eq!(octet.to_u8(), TK_BYTE);
        assert_eq!(uint8.to_u8(), TK_UINT8);
    }
}
