// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder and decoder bodies, built from the layout plan. Spec §5, §6.2.
//!
//! The body of an `encode` is a language CST — statements, calls, casts —
//! not a template with holes for names. A template would put the
//! per-construct decisions back where no diff of two languages can see
//! them, one hole at a time, which is what this rebuild took them out
//! of.
//!
//! Every framing decision is read from `zerodds-ast-layout`, never
//! re-derived here. The plan is where the length-code policy and the
//! XCDR2 alignment rule live; a second derivation would be a second
//! place to get them wrong.

use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TryConstruct, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_ast_layout::key::{
    ElementPadding, FixedDigitCount, LayoutKey, PlCdr1HeaderPolicy, Purpose, WcharPosition,
    WirePosition, WstringCodeUnits, WstringLengthCounts, WstringLengthPolicy, WstringTerminator,
    WstringUnitOrder, WstringUnitWidth,
};
use zerodds_ast_layout::plan::{
    Extent, LayoutError, MemberHeader, MemberPlan, ScalarWrite, TypeLayout,
};
use zerodds_ast_layout::xcdr::DepCollector;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path, Visibility};
use zerodds_lang_cst::rust;

/// Why an encoder could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// The layout could not be planned, with the plan's own reason.
    NoLayout {
        /// What the layout crate said.
        detail: String,
    },
    /// The specification itself does not admit the type, in the
    /// planner's own words and by its clause.
    ///
    /// Kept apart from [`Self::NoLayout`] because the two answer
    /// different questions and `docs/specs/zerodds-verdicts-1.0.md`
    /// gives them different exit codes: a gap is "a tool of **ours**
    /// has not written it" (exit 6, `harness-gap`), and this is "it was
    /// asked and said no" (exit 1, `refused`). Reporting a clause of
    /// IDL 4.2 as a gap of this tree states something about this tree
    /// that is not true.
    ///
    /// The distinction was a string until 2026-08-26, and twelve
    /// generators each classified the same refusal on their own:
    /// measured over `recursive-array`, ada and d answered exit 3, zig
    /// answered 1, and twelve answered 6.
    SpecRefuses {
        /// The clause and what it refuses, from the planner.
        detail: String,
    },
    /// A member's type has no writer.
    ///
    /// Reported rather than skipped: a member left out of an encoder is
    /// a wire format that silently disagrees with its own plan.
    NoWriter {
        /// The member.
        member: String,
        /// What kind of type it is.
        kind: &'static str,
    },
    /// An enumerator's wire value could not be computed.
    ///
    /// Reported rather than skipped, for the same reason as
    /// [`Self::NoWriter`] and with a sharper edge. The C encoder's
    /// enumeration chain initialises its holder to the **declared**
    /// value and lets each arm overwrite it, so an enumerator that
    /// reaches no arm is not absent from the stream — it is written
    /// with the declared integer. Under
    /// `zerodds_ast_layout::EnumeratorValue::Ordinal` that is a
    /// different integer from the one every other column writes, and
    /// nothing at run time would say so.
    NoEnumeratorValue {
        /// The enumeration the enumerator belongs to.
        enumeration: String,
        /// The enumerator.
        enumerator: String,
        /// Why its wire value could not be computed.
        detail: &'static str,
    },
    /// A member's PL_CDR1 header form is not one the sample's flag can
    /// express.
    ///
    /// Under a [per-sample](`PlCdr1HeaderPolicy::decided_per_sample`)
    /// policy the header is chosen at run time, and the choice is read
    /// by planning the type under the two policies the flag stands
    /// between — [`PlCdr1HeaderPolicy::Specification`] and
    /// [`PlCdr1HeaderPolicy::ExtendedForConstructed`]. A member whose
    /// two plans differ in `extended` alone is one the flag decides; a
    /// member whose two plans differ in anything else is a case where
    /// `zd_extended` is not the whole of the answer, and picking one of
    /// the two plans would write bytes nothing measured.
    AmbiguousHeader {
        /// The member the two plans disagree about.
        member: String,
        /// What each plan said, so the difference can be read.
        detail: String,
    },
}

impl EncodeError {
    /// Whether the specification refused the type, rather than this tree
    /// not having written a writer for it.
    ///
    /// The one place a generator asks the question, so that sixteen of
    /// them do not each answer it from the error's text.
    #[must_use]
    pub const fn is_spec_refusal(&self) -> bool {
        matches!(self, Self::SpecRefuses { .. })
    }
}

/// Refuses a layout key whose axes the **non-Rust** generated codecs do
/// not carry.
///
/// [`zerodds_ast_layout::Dialect::Hdds`] selects three writer axes the
/// planner and `zerodds-wire-codec` carry: collection elements packed
/// without alignment, a length in front of each element of a sequence of a
/// non-`@final` type, and a `wstring` in UTF-8. The Rust codec carries all
/// three — see [`axes_the_rust_codec_carries`], which [`rust_codec`] and
/// [`rust_codec_pair`] call instead. A generated codec of another column
/// asks [`axes_the_codecs_carry_for`], which passes for a target whose
/// runtime carries them and refuses the rest: generating their default
/// form under them would write the specification's octets where the key
/// asked for another implementation's. The refusal is a gap of this tree
/// (`NoLayout`, exit 6), named axis by axis; this function is it for a
/// caller that names no target.
///
/// # Errors
/// [`EncodeError::NoLayout`] naming every such axis the key selects.
pub fn axes_the_codecs_carry(key: LayoutKey) -> Result<(), EncodeError> {
    axes_the_codecs_carry_named(key, false)
}

/// [`axes_the_codecs_carry`] for a generated codec, which carries the
/// axes its runtime does.
///
/// A target whose runtime has the packed-element origin, the element
/// length in DHEADER form and a UTF-8 `wstring` — the three axes
/// [`zerodds_ast_layout::Dialect::Hdds`] selects — answers
/// [`crate::emit::CodecTarget::carries_hdds_axes`] `true` and passes;
/// one that has not been given them is refused with the axes named.
///
/// # Errors
/// [`EncodeError::NoLayout`] naming every axis the key selects that the
/// target does not carry.
pub fn axes_the_codecs_carry_for<T: crate::emit::CodecTarget>(
    key: LayoutKey,
) -> Result<(), EncodeError> {
    axes_the_codecs_carry_named(key, T::carries_hdds_axes())
}

fn axes_the_codecs_carry_named(key: LayoutKey, carries: bool) -> Result<(), EncodeError> {
    if carries {
        return Ok(());
    }
    let mut missing: Vec<&str> = Vec::new();
    if !key.element_padding.is_default() {
        missing.push(key.element_padding.as_str());
    }
    if !key.sequence_element_delimiter.is_default() {
        missing.push(key.sequence_element_delimiter.as_str());
    }
    if !key.wstring_code_units.is_default() {
        missing.push(key.wstring_code_units.as_str());
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(EncodeError::NoLayout {
            detail: format!(
                "the generated codecs do not carry {} yet",
                missing.join(", ")
            ),
        })
    }
}

/// The same check for the Rust target, which carries every HDDS axis.
///
/// [`ElementPadding::Packed`],
/// [`zerodds_ast_layout::SequenceElementDelimiter::BeforeEachNonFinalElement`]
/// and [`WstringCodeUnits::Utf8`] are emitted by [`rust_codec`] and
/// [`rust_codec_pair`] exactly as `zerodds-wire-codec` writes them, so
/// this target has no axis left to refuse. The function exists so that the
/// two targets ask one question through one name rather than the Rust
/// path skipping the check by having no call at all.
///
/// # Errors
/// Nothing today; the signature matches [`axes_the_codecs_carry_for`],
/// the check of the other columns.
pub fn axes_the_rust_codec_carries(_key: LayoutKey) -> Result<(), EncodeError> {
    Ok(())
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoLayout { detail } => write!(f, "the layout could not be planned: {detail}"),
            Self::SpecRefuses { detail } => write!(f, "{detail}"),
            Self::NoWriter { member, kind } => {
                write!(f, "`{member}` is a {kind}, which has no writer yet")
            }
            Self::NoEnumeratorValue {
                enumeration,
                enumerator,
                detail,
            } => write!(
                f,
                "`{enumeration}::{enumerator}` has no wire value: {detail}"
            ),
            Self::AmbiguousHeader { member, detail } => write!(
                f,
                "`{member}` has no single PL_CDR1 header form to choose between: {detail}"
            ),
        }
    }
}

impl core::error::Error for EncodeError {}

/// The name the **second** codec pair on a type carries.
///
/// A Rust type may hold only one inherent function per name, so a type
/// that carries both of its XCDR streams needs two names. The XCDR
/// version is what earns one: XTypes 1.3 §7.4 frames the two versions
/// differently — no DHEADER under version 1, PID headers instead of
/// EMHEADERs for `@mutable`, `MAXALIGN` eight instead of four — so the
/// two bodies are different code.
///
/// The byte order does not earn one. Nothing in this emitter reads
/// [`LayoutKey::endianness`]: the order is the `Writer`'s and the
/// `Reader`'s, so one body serves both and a suffix per order would
/// duplicate identical code.
///
/// [`rust_codec`] keeps the bare names whatever encoding it is given, so
/// every consumer that generates one codec per type is unaffected by
/// this existing; only [`rust_codec_pair`], which exists to put a second
/// one beside it, spells them out.
pub const SECONDARY_SUFFIX: &str = "_xcdr1";

/// The parameter that carries a sample's RTI PL_CDR1 step decision down
/// to a nested type's encoder.
///
/// RTI frames a sample under XCDR1 with the short parameter header until
/// its type's maximum serialized size passes a step, and from there
/// gives `PID_EXTENDED` to every member whose type is not a primitive or
/// an enumeration (XTypes 1.3 §7.4.1.2, in RTI's measured reading of it).
/// The step is a property of the **sample's** type and not of the type
/// that declares a member, so a generated type cannot bake the answer in:
/// the same nested `@mutable` type is framed short inside a small sample
/// and extended inside a large one. The flag is threaded at run time; the
/// sample's root supplies it through [`sample_past_step`], which asks the
/// same crossing the reflective encoder asks.
const EXTENDED_PARAM: &str = "zd_extended";

/// The suffix the encoder **body** takes under a [per-sample](`per_sample`)
/// plan.
///
/// A Rust type may hold only one inherent function per name, so a type
/// that needs both a body taking the flag and a public entry does so the
/// same way [`SECONDARY_SUFFIX`] splits the two encodings.
const EXTENDED_SUFFIX: &str = "_zd_extended";

/// Whether the plan's PL_CDR1 header form is decided per sample, so every
/// generated encoder takes [`EXTENDED_PARAM`] and threads it to the types
/// it calls.
fn per_sample(key: LayoutKey) -> bool {
    key.pl_cdr1_headers.decided_per_sample() && key.encoding.is_v1()
}

/// Whether a sample of `root` passes RTI's short-header step, the
/// generation-time half of the per-sample decision.
///
/// The same count [`LayoutKey::for_sample`] makes, asked through it
/// rather than through `exceeds_short_length` so that the generated
/// entry and the reflective path read one answer.
fn sample_past_step(ast: &Ast, root: Id<TypeNode>, key: LayoutKey) -> Result<bool, EncodeError> {
    let resolved = key.for_sample(ast, root).map_err(plan_refusal)?;
    Ok(resolved.pl_cdr1_headers == PlCdr1HeaderPolicy::ExtendedForConstructed)
}

/// The name of the encoder **body** for a codec suffix: the method that
/// takes [`EXTENDED_PARAM`], or the public entry itself when the plan's
/// header form is fixed.
///
/// Under [`per_sample`] one type needs two functions. The body carries
/// the sample's decision as a parameter, so one type's codec can frame
/// its constructed members short in a small sample and extended in a
/// large one; the public entry bakes the decision in for the sample whose
/// root this type is. Without the split, the nested `Inner` of
/// `tools/idlc/tests/dialect_byte_equality.rs`'s `rti_step_cases` could
/// not serve both its samples.
fn body_name(codec: &str, key: LayoutKey) -> String {
    if per_sample(key) {
        framed_body(codec)
    } else {
        format!("encode{codec}")
    }
}

/// The name of an encoder body that takes the sample's decision, for a
/// nested call that carries `extended`.
fn nested_method(codec: &str, extended: Extended) -> String {
    match extended {
        Extended::Fixed => format!("encode{codec}"),
        Extended::Inherited | Extended::Constant(_) => framed_body(codec),
    }
}

/// [`nested_method`]'s name on the other side of the split.
fn framed_body(codec: &str) -> String {
    format!("encode{codec}{EXTENDED_SUFFIX}")
}

/// The sample's RTI PL_CDR1 step decision, as a nested type's call
/// carries it.
///
/// A data sample's body passes its own flag down, because the sample's
/// root resolved it; a key holder passes [`Self::Constant`] `false`. A
/// key holder keeps the short form whatever the sample: the choice of
/// `PID_EXTENDED` for a constructed member is a data-sample decision —
/// [`LayoutKey::for_sample`] is asked for one, and
/// `zerodds_ast_layout::xcdr::extended_as_constructed` refuses the key
/// purpose for the same reason — and the reflective planner keeps a key
/// holder short. The data path resolves the choice once at the root and
/// hands it down; the holder has no root decision to hand down and states
/// the constant.
///
/// `pub(crate)`: it is a field of [`MemberWrite::Nested`], which is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Extended {
    /// The plan fixes the header form: the call names the public entry,
    /// which takes no flag. Every plan outside [`per_sample`].
    Fixed,
    /// Hand the enclosing body's flag to the nested call.
    Inherited,
    /// Pass this constant instead.
    Constant(bool),
}

impl Extended {
    /// The answer for a codec written under `key`.
    ///
    /// The key holder's constant is what `crate::emit::encode`'s
    /// `nested_call_with` passes for the same reason.
    #[must_use]
    fn of(key: LayoutKey) -> Self {
        if !per_sample(key) {
            Self::Fixed
        } else if key.purpose == Purpose::Data {
            Self::Inherited
        } else {
            Self::Constant(false)
        }
    }

    /// The flag argument the nested call passes, if it passes one.
    #[must_use]
    fn argument(self) -> Option<rust::Expr> {
        match self {
            Self::Fixed => None,
            Self::Inherited => Some(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            Self::Constant(value) => Some(rust::Expr::Literal(Literal::Bool(value))),
        }
    }
}

/// Which of one type's members take the sample's step decision at run
/// time.
///
/// A member is one exactly when the two policies a per-sample plan is
/// between disagree about its `extended` bit:
/// [`PlCdr1HeaderPolicy::Specification`] frames a sample that stays
/// within the short form's length,
/// [`PlCdr1HeaderPolicy::ExtendedForConstructed`] one that can exceed it.
/// Keyed by member identity and not by position, because a union finds
/// its case's plan in the layout by identity.
struct StepHeaders {
    /// Whether the plan decides the form per sample at all, and the
    /// entries below mean anything.
    per_sample: bool,
    /// One entry per member of the layout: does the flag choose its
    /// header form?
    members: Vec<(NodeId, bool)>,
    /// The same for a `@mutable` union's discriminator, which rules (27)
    /// and (28) frame as an `MMEMBER` at member id 0 and which is
    /// therefore a header the flag can choose too.
    discriminator: bool,
}

impl StepHeaders {
    /// No member takes the flag, which is what every plan outside
    /// [`per_sample`] plans.
    fn none() -> Self {
        Self {
            per_sample: false,
            members: Vec::new(),
            discriminator: false,
        }
    }

    /// Whether the flag chooses `plan`'s member's header form.
    ///
    /// # Errors
    /// [`EncodeError::NoLayout`] for a member this set does not name —
    /// a plan that lost one would leave a member whose header nothing
    /// wrote.
    fn member(&self, plan: &MemberPlan, name: &str) -> Result<bool, EncodeError> {
        if !self.per_sample {
            return Ok(false);
        }
        self.members
            .iter()
            .find(|(id, _)| *id == plan.member)
            .map(|(_, chosen)| *chosen)
            .ok_or_else(|| EncodeError::NoLayout {
                detail: format!(
                    "the PL_CDR1 plans for a sample past RTI's step name no member `{name}`, \
                     and its header form would be left unwritten"
                ),
            })
    }
}

/// The plan `type_` takes under one PL_CDR1 header policy.
///
/// # Errors
/// [`EncodeError`] when the type cannot be planned under that policy.
fn plan_under(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    policy: PlCdr1HeaderPolicy,
) -> Result<TypeLayout, EncodeError> {
    zerodds_ast_layout::xcdr::plan(
        ast,
        type_,
        key.with_pl_cdr1_headers(policy),
        &mut DepCollector::default(),
    )
    .map_err(plan_refusal)
}

/// Whether the flag chooses this member's header form, from the two plans
/// a per-sample decision stands between.
///
/// `framed` is the plan the codec is written from — the unresolved key's,
/// which frames short because the choice belongs to the sample and not to
/// the type — `below` the one a sample under the step gets and `past` the
/// one a sample over it gets. Where `below` and `past` agree, the form is
/// fixed and nothing is threaded; where they differ in `extended` alone,
/// the flag picks; anything else is a case `zd_extended` cannot express
/// and is refused rather than answered with one of the two plans.
///
/// `framed` is checked against `below` too. The unresolved policy is
/// neither of the two, so a member it framed differently from a small
/// sample would mean the two are not the ends of one decision.
///
/// # Errors
/// [`EncodeError::AmbiguousHeader`] naming the member when two of the
/// three plans disagree about more than `extended`.
fn header_is_chosen(
    member: &str,
    framed: MemberHeader,
    below: MemberHeader,
    past: MemberHeader,
) -> Result<bool, EncodeError> {
    let refuse = || ambiguous(member, framed, below, past);
    if below == past {
        if framed == below {
            return Ok(false);
        }
        return Err(refuse());
    }
    // `MemberHeader`'s own equality has already settled that the two
    // differ in nothing but `extended`, so what is left is which way and
    // which header: both forms of a PL_CDR1 parameter header take the
    // same branch, `write_pid` below the step and `write_pid_extended`
    // past it, and nothing else does.
    match (below, past) {
        (
            MemberHeader::PidCdr1 {
                extended: false, ..
            },
            MemberHeader::PidCdr1 { extended: true, .. },
        )
        | (
            MemberHeader::OptionalCdr1 {
                extended: false, ..
            },
            MemberHeader::OptionalCdr1 { extended: true, .. },
        ) if framed == below => Ok(true),
        _ => Err(refuse()),
    }
}

/// The refusal [`header_is_chosen`] makes, naming all three plans.
fn ambiguous(
    member: &str,
    framed: MemberHeader,
    below: MemberHeader,
    past: MemberHeader,
) -> EncodeError {
    EncodeError::AmbiguousHeader {
        member: member.to_string(),
        detail: format!(
            "the codec's own plan frames it {framed:?}, a sample within RTI's short-header step \
             {below:?} and a sample past it {past:?}, and one flag cannot choose between them"
        ),
    }
}

/// Every member of `type_` whose PL_CDR1 header form the sample decides,
/// under `framed`.
///
/// [`StepHeaders::none`] for every plan outside [`per_sample`], so a
/// dialect whose header form is fixed never pays for two more plans and
/// generates exactly the code it generated before.
///
/// # Errors
/// [`EncodeError`] when one of the two plans cannot be made, when they do
/// not name the same members as `framed`, or when they differ in anything
/// but `extended`.
fn step_headers(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    framed: &TypeLayout,
) -> Result<StepHeaders, EncodeError> {
    if !per_sample(key) {
        return Ok(StepHeaders::none());
    }
    let name = ast
        .type_(type_)
        .map_or_else(|_| "<unresolved>".to_string(), |node| node.name.clone());
    let below = plan_under(ast, type_, key, PlCdr1HeaderPolicy::Specification)?;
    let past = plan_under(ast, type_, key, PlCdr1HeaderPolicy::ExtendedForConstructed)?;
    if below.members.len() != framed.members.len() || past.members.len() != framed.members.len() {
        return Err(EncodeError::NoLayout {
            detail: format!(
                "`{name}`: the PL_CDR1 plans for the two sides of RTI's step name {} and {} \
                 members where the codec's own plan names {}",
                below.members.len(),
                past.members.len(),
                framed.members.len()
            ),
        });
    }
    let mut members = Vec::with_capacity(framed.members.len());
    for ((own, below), past) in framed.members.iter().zip(&below.members).zip(&past.members) {
        let member = member_name(ast, type_, own.member);
        members.push((
            own.member,
            header_is_chosen(&member, own.header, below.header, past.header)?,
        ));
    }
    let discriminator = match (
        framed.discriminator,
        below.discriminator,
        past.discriminator,
    ) {
        (None, None, None) => false,
        (Some(framed), Some(below), Some(past)) => {
            header_is_chosen(&format!("{name}::discriminator"), framed, below, past)?
        }
        _ => {
            return Err(EncodeError::NoLayout {
                detail: format!(
                    "`{name}`: the PL_CDR1 plans for the two sides of RTI's step disagree about \
                     whether its discriminator carries a header at all"
                ),
            });
        }
    };
    Ok(StepHeaders {
        per_sample: true,
        members,
        discriminator,
    })
}

/// The name a member node has inside `type_`, for an error that names it.
fn member_name(ast: &Ast, type_: Id<TypeNode>, member: NodeId) -> String {
    ast.all_members_of(type_)
        .into_iter()
        .find(|id| ast.member(*id).is_ok_and(|node| node.node_id == member))
        .and_then(|id| ast.member(id).ok())
        .map_or_else(|| alloc_format(&member), |node| node.name.clone())
}

/// Build the `encode` and `decode` functions for one type, with the
/// `Default` impl and — for a keyed type — the key-holder pair.
///
/// This column writes a member's collection where the member is, in the
/// parent's own body and through a closure, and never hoists it into a
/// routine named after its element type. So the bound-in-the-name
/// defect the other columns carry — `sequence<long>` and
/// `sequence<long, 4>` sharing one hoisted routine, the first declared
/// deciding whether the check exists — cannot reach here: every bound
/// travels as an argument at the member's own call site
/// (`write_string_bounded`, `write_sequence_bounded`, and the `fit_…`
/// pair on the read side). Measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };` in
/// both declaration orders: both members keep exactly the bound their
/// declaration gives them. `crates/lang-rules/src/emit/zig.rs`'s
/// `bound_tag` is the fix in the columns that do hoist.
///
/// # Errors
/// [`EncodeError`] when the layout cannot be planned or a member has no
/// writer — never a body that leaves a member out.
pub fn rust_codec(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Vec<rust::Item>, EncodeError> {
    codec_items(ast, type_, key, Companions::Emit)
}

/// Build only the `encode` and `decode` pair for one type.
///
/// For a **second** plan over a type that already has one: the `Default`
/// impl and the key holder do not belong to an encoding and would be
/// emitted twice under the same name, which Rust refuses. The key holder
/// in particular is not a per-encoding thing at all — XTypes 1.3 §7.6.8
/// fixes it as PLAIN_CDR2 big-endian whatever the sample's encoding is —
/// so a second copy of it would be the same bytes under a name that
/// suggests otherwise.
///
/// # Errors
/// As [`rust_codec`].
pub fn rust_codec_pair(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Vec<rust::Item>, EncodeError> {
    codec_items(ast, type_, key, Companions::Omit)
}

/// Whether [`rust_codec`] gives a type of this kind a codec of its own:
/// an aggregate (structure, union, valuetype), an enumeration, a bitmask
/// or a bitset. A bitmask's answer is empty, since its Rust form is an
/// alias of a primitive and its member site writes the holder. Every
/// other kind — a primitive, a string, a collection, an alias — is
/// written where it is used and has no `impl` to hang a pair on.
#[must_use]
pub const fn has_codec(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
    )
}

/// Whether a codec run also emits what does not belong to an encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Companions {
    /// The `Default` impl and the key holder come with the codec.
    Emit,
    /// Only the codec pair.
    Omit,
}

fn codec_items(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    companions: Companions,
) -> Result<Vec<rust::Item>, EncodeError> {
    axes_the_rust_codec_carries(key)?;
    // Which set of names this run writes, and — through
    // `MemberWrite::Nested` — which set it calls on a member. The two
    // have to be the same set: a nested call that named the XCDR2
    // encoder from inside the XCDR1 body would frame the member under
    // version 2 in a version 1 stream, and the sample would decode into
    // wrong values with nothing reporting it.
    let codec = match companions {
        Companions::Emit => "",
        Companions::Omit => SECONDARY_SUFFIX,
    };
    // Asked for a kind with no codec of its own, the generic path below
    // emitted `impl long { … }` or `impl Count { … }` for a
    // `typedef long Count;` — not Rust (E0390), and an `encode` that wrote
    // nothing. Every caller filtered the kinds it asked for; the refusal
    // is here so none has to.
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    if !has_codec(&node.kind) {
        return Err(EncodeError::NoWriter {
            member: node.name.clone(),
            kind: "a type with no codec of its own; it is written where it is used",
        });
    }
    let mut deps = DepCollector::default();
    let layout =
        zerodds_ast_layout::xcdr::plan(ast, type_, key, &mut deps).map_err(plan_refusal)?;
    // Which of this type's members the sample's PL_CDR1 step decides. One
    // set per codec, shared by every encoder below, because they are all
    // the same type under the same plan.
    let steps = step_headers(ast, type_, key, &layout)?;

    // An enumeration has no layout **members**, and the generic path
    // below walks exactly those. It therefore produced
    //
    //     pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> { Ok(()) }
    //     pub fn decode(r: &mut Reader) -> Result<Self, WireError> { Ok(Self { }) }
    //
    // — an encoder that puts zero octets on the wire for a value that
    // has a wire form, and a decoder that is not Rust at all: `Self { }`
    // is a struct literal and the type is an enum. It was invisible
    // because an enumeration is normally reached as a *member*, where
    // `MemberWrite::Enumeration` writes its holder, and nothing in the
    // tree asked an enumeration for its own codec until a CORBA
    // operation took one as a parameter — an argument has no enclosing
    // aggregate to be a member of.
    if let TypeKind::Enumeration { enumerators } = &node.kind {
        let mut items = enumeration_encode_fns(ast, type_, enumerators, key, codec)?;
        items.push(enumeration_decode_fn(ast, type_, enumerators, key, codec)?);
        return Ok(vec![rust::Item::Impl(rust::Impl {
            doc: Doc::none(),
            attrs: Vec::new(),
            generics: Vec::new(),
            trait_: None,
            self_type: rust::Type::named(Path::single(&node.name)),
            items,
        })]);
    }

    // A bitmask has **no** codec of its own, and cannot have one.
    // `crate::adapter::cst` materialises it as `pub type Flags = u16;`,
    // and `impl Flags` on a type alias to a primitive is E0390 —
    // "cannot define inherent `impl` for primitive types". The generic
    // path below emitted exactly that, with an `encode` that put zero
    // octets on the wire and a `decode` returning `Ok(Self { })`: a
    // file that did not compile, so a model with a `bitmask` in it had
    // no Rust output at all. `MemberWrite::Bitmask` writes the holder
    // at the member site instead, which is why nothing has to be
    // emitted here and why an empty answer is not a member left out.
    //
    // The name stays in `generate::rust_codec_pairs`' list because the
    // other twenty columns do give a bitmask a codec function; this is
    // the one column whose language mapping has nowhere to put it.
    if matches!(node.kind, TypeKind::Bitmask { .. }) {
        return Ok(Vec::new());
    }

    // A bitset has a codec and had an empty one. It materialises as a
    // struct with one field per named bitfield, so there is an `impl`
    // to hang the pair on — but a bitset's layout has no **members**,
    // and the generic path walks exactly those. What came out was an
    // `encode` that wrote nothing at all for a type with a wire form
    // and a `decode` returning `Ok(Self { })` for a struct with two
    // fields. The same hole as the enumeration's above, and the same
    // fix: the construct's own pair.
    if matches!(node.kind, TypeKind::Bitset { .. }) {
        let mut items = bitset_encode_fns(ast, type_, &layout, key, codec)?;
        items.push(bitset_decode_fn(ast, type_, &layout, codec)?);
        let mut out = vec![rust::Item::Impl(rust::Impl {
            doc: Doc::none(),
            attrs: Vec::new(),
            generics: Vec::new(),
            trait_: None,
            self_type: rust::Type::named(Path::single(&node.name)),
            items,
        })];
        if companions == Companions::Emit
            && let Some(default) = default_impl(ast, type_)?
        {
            out.push(default);
        }
        return Ok(out);
    }

    // A union has the same shape of hole as an enumeration, one level
    // up: `crate::adapter::cst` materialises it as a Rust enum with one
    // payload variant per case, and the generic path below builds a
    // struct literal over the layout's members. It produced an `encode`
    // that wrote nothing and a `decode` that returned `Ok(Self { })`.
    if let TypeKind::Union {
        discriminator,
        cases,
        ..
    } = &node.kind
    {
        let items = union_codec(
            ast,
            type_,
            &layout,
            &steps,
            discriminator,
            cases,
            key,
            codec,
        )?;
        let mut out = vec![rust::Item::Impl(rust::Impl {
            doc: Doc::none(),
            attrs: Vec::new(),
            generics: Vec::new(),
            trait_: None,
            self_type: rust::Type::named(Path::single(&node.name)),
            items,
        })];
        // A union returns from here and not through the tail, so its
        // `impl Default` has to be added on this path too. It was not,
        // and the omission was silent: the type simply had no `Default`,
        // which shows up wherever a caller writes `Default::default()`
        // for a member of one.
        if companions == Companions::Emit
            && let Some(default) = default_impl(ast, type_)?
        {
            out.push(default);
        }
        return Ok(out);
    }

    let mut items = encode_fns(ast, type_, &layout, &steps, key, codec)?;
    items.push(decode_fn(ast, type_, &layout, key, codec)?);
    if companions == Companions::Emit {
        items.append(&mut key_holder_fns(ast, type_, key)?);
    }

    let mut out = vec![rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(&node.name)),
        items,
    })];
    // The specification's default values, as an `impl Default`. A member
    // that has none — a type this column cannot give a default for —
    // leaves the impl out rather than substituting one.
    if companions == Companions::Emit
        && let Some(default) = default_impl(ast, type_)?
    {
        out.push(default);
    }
    Ok(out)
}

/// `encode` and `decode` for a union. §7.4.3.5 rule (10): the
/// discriminator, then the member the discriminator selects — and around
/// them the framing the plan gives the union's extensibility: rule (11)'s
/// DHEADER for an `@appendable` one, rules (27) and (28)'s member headers
/// for a `@mutable` one, the discriminator under member id 0. This column
/// wrote rule (10) for every union, so an `@appendable` union went out
/// without the DHEADER the C, Java and C# columns and every vendor write
/// (measured: `idl-conformance/_interop` `mapenum`, a `sequence<Sel>` of
/// an `@appendable` union — 44 octets here, 48 in C).
///
/// The Rust form is `crate::adapter::cst`'s — an enum with one payload
/// variant per case — so `encode` matches the variant and writes the
/// label that selects it, and `decode` reads the discriminator and
/// matches its value back.
///
/// A discriminator value that selects no case and no `default:` leaves
/// the member absent (§7.4.3.5); it is read into the variant
/// `crate::rust_union::NO_MEMBER`, which holds the discriminator alone.
/// A variant that carries its discriminator is checked before it is
/// written: a pairing the type does not allow is refused.
#[allow(clippy::too_many_arguments)]
fn union_codec(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    steps: &StepHeaders,
    discriminator: &zerodds_ast_core::Ref<TypeNode>,
    cases: &[Id<zerodds_ast::CaseNode>],
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Item>, EncodeError> {
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let name = node.name.clone();
    let discriminator = discriminator.resolved().ok_or(EncodeError::NoWriter {
        member: name.clone(),
        kind: "a union with an unresolved discriminator",
    })?;
    // The discriminator is written as its *value* and not through the
    // enumeration's own codec: what a union writes here is the label the
    // selected branch carries, which the emitter already has as an
    // integer. A member of an enumeration type is the other case and
    // goes through `<path>::encode` — see the arm in `write_of`.
    let discriminator_write = match ast.type_(discriminator).map(|d| &d.kind) {
        Ok(TypeKind::Enumeration { .. }) => MemberWrite::Enumeration {
            bits: zerodds_ast_layout::enumeration_bit_bound(ast, discriminator, key.enum_holder),
        },
        _ => write_of(
            ast,
            node.module,
            discriminator,
            key,
            codec,
            Position {
                behaviour: zerodds_ast::TryConstruct::Discard,
                written: None,
            },
        ),
    };

    // The union as this column holds it: which variant carries its
    // discriminator, whether one holds a discriminator alone, and how a
    // label is spelled. `crate::rust_union`, shared with the declaration
    // and the default.
    let shape = crate::rust_union::shape(ast, type_).map_err(|kind| EncodeError::NoWriter {
        member: name.clone(),
        kind,
    })?;
    let refuse_label = |_| EncodeError::NoWriter {
        member: name.clone(),
        kind: "a union label with no source form in this column",
    };
    // A label as the discriminator's *write* takes it: the integer an
    // enumeration's holder carries, the literal otherwise.
    let written_label = |value: i128| -> Result<rust::Expr, EncodeError> {
        Ok(match shape.kind {
            crate::rust_union::DiscriminatorKind::Enumeration(_) => {
                rust::Expr::Literal(Literal::int(value))
            }
            kind => crate::rust_union::label_literal(ast, node.module, kind, value)
                .map_err(refuse_label)?
                .expr(),
        })
    };
    // A label as a pattern over the discriminator a variant carries, and
    // over the one `decode` reads: an enumeration's is converted to the
    // enumeration first.
    // An enumeration discriminator's own name, for a refusal that names it.
    let enumeration_name = |enumeration: Id<TypeNode>| {
        ast.type_(enumeration)
            .map_or_else(|_| name.clone(), |node| node.name.clone())
    };
    let label_pattern = |value: i128| -> Result<rust::Pattern, EncodeError> {
        Ok(
            crate::rust_union::label_literal(ast, node.module, shape.kind, value)
                .map_err(refuse_label)?
                .pattern(),
        )
    };
    let refused =
        |what: String, sink: &str| rust::Expr::Return(Some(Box::new(refused_err(sink, &what))));
    // A carried discriminator as a match scrutinee: the binding, which a
    // `match` on `&self` makes a reference, dereferenced for the copyable
    // primitives; an enumeration is matched through the reference.
    let carried_scrutinee = || match shape.kind {
        crate::rust_union::DiscriminatorKind::Enumeration(_) => {
            rust::Expr::Path(Path::single("discriminator"))
        }
        _ => rust::Expr::Deref(Box::new(rust::Expr::Path(Path::single("discriminator")))),
    };
    // A carried discriminator written: an enumeration through its holder
    // integer, every other kind as itself.
    let write_carried = || -> Result<rust::Expr, EncodeError> {
        match shape.kind {
            crate::rust_union::DiscriminatorKind::Enumeration(enumeration) => {
                let mut arms = Vec::new();
                for enumerator in ast.enumerators_of(enumeration) {
                    // The enumerator by name, and the integer the wire
                    // carries for it — which the setting decides.
                    let (_, value) = enumerator_variant_and_value(
                        ast,
                        &enumeration_name(enumeration),
                        *enumerator,
                        key,
                    )?;
                    arms.push(rust::MatchArm {
                        pattern: rust::Pattern::Path(
                            crate::rust_union::enumerator_path_of(
                                ast,
                                node.module,
                                enumeration,
                                *enumerator,
                            )
                            .map_err(refuse_label)?,
                        ),
                        guard: None,
                        body: rust::Expr::Literal(Literal::int(value)),
                    });
                }
                write_expr_for(
                    &discriminator_write,
                    rust::Expr::Match {
                        scrutinee: Box::new(rust::Expr::Path(Path::single("discriminator"))),
                        arms,
                    },
                    &name,
                    false,
                )
            }
            _ => write_expr_for(
                &discriminator_write,
                rust::Expr::Path(Path::single("discriminator")),
                &name,
                true,
            ),
        }
    };
    // The check ahead of a carried discriminator's write: one of `own`
    // where the variant is a case with several labels; none of every
    // case's labels where it is the `default:` case or the variant that
    // holds no member. A pairing the type does not allow is refused, not
    // written: a peer would read another branch than the one the value
    // names.
    let check_carried =
        |own: Option<&[i128]>, what: String| -> Result<rust::Statement, EncodeError> {
            let mut arms = Vec::new();
            match own {
                Some(labels) => {
                    for value in labels {
                        arms.push(rust::MatchArm {
                            pattern: label_pattern(*value)?,
                            guard: None,
                            body: rust::Expr::Block(rust::Block {
                                statements: Vec::new(),
                            }),
                        });
                    }
                    arms.push(rust::MatchArm {
                        pattern: rust::Pattern::Wildcard,
                        guard: None,
                        body: refused(what, "w"),
                    });
                }
                None => {
                    for value in shape.all_labels() {
                        arms.push(rust::MatchArm {
                            pattern: label_pattern(value)?,
                            guard: None,
                            body: refused(what.clone(), "w"),
                        });
                    }
                    arms.push(rust::MatchArm {
                        pattern: rust::Pattern::Wildcard,
                        guard: None,
                        body: rust::Expr::Block(rust::Block {
                            statements: Vec::new(),
                        }),
                    });
                }
            }
            Ok(rust::Statement::Semi(rust::Expr::Match {
                scrutinee: Box::new(carried_scrutinee()),
                arms,
            }))
        };

    // The discriminator's header, as a plan the header helpers take: only
    // a `@mutable` union has one, under member id 0.
    let discriminator_plan = layout.discriminator.map(|header| MemberPlan {
        member: layout.type_,
        member_id: 0,
        header,
        offset: zerodds_ast_layout::plan::Offset::Dynamic,
        align: 1,
    });

    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();
    let mut default_arm = None;

    for case in cases {
        let case_node = ast.case(*case).map_err(|error| EncodeError::NoLayout {
            detail: alloc_format(&error),
        })?;
        let member = ast
            .member(case_node.member)
            .map_err(|error| EncodeError::NoLayout {
                detail: alloc_format(&error),
            })?;
        let rust_case = shape
            .cases
            .iter()
            .find(|c| c.member.node_id == member.node_id)
            .ok_or(EncodeError::NoWriter {
                member: member.name.clone(),
                kind: "a union case the shape does not hold",
            })?;
        let variant = rust_case.variant.clone();
        let carries = rust_case.carries_discriminator();
        let member_type = member.type_.resolved().ok_or(EncodeError::NoWriter {
            member: member.name.clone(),
            kind: "a union case with an unresolved type",
        })?;
        let member_write = write_of(
            ast,
            node.module,
            member_type,
            key,
            codec,
            Position {
                behaviour: ast.effective_try_construct(None, Some(member_type)),
                written: None,
            },
        );
        let member_write = external_write(ast, member, member_write);
        let label_values = rust_case.labels.clone();
        let is_default = rust_case.is_default;

        // The case's own header, which only a `@mutable` union's plan has.
        let case_plan = layout
            .members
            .iter()
            .find(|plan| plan.member == member.node_id)
            .copied();

        // `Self::Variant(value)`, or `Self::Variant(discriminator, value)`
        // where the variant carries what `decode` read.
        let construct = |value: rust::Expr| {
            let mut args = Vec::new();
            if carries {
                args.push(rust::Expr::Path(Path::single("discriminator")));
            }
            args.push(value);
            rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                args: vec![rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&["Self", &variant]))),
                    args,
                }],
            }
        };
        let read_member = read_for(&member.name, &member_write)?;
        let build = match case_plan {
            None => construct(rust::Expr::Try(Box::new(read_member))),
            Some(plan) => {
                let mut statements = read_header(&plan);
                statements.push(let_statement(
                    "value",
                    rust::Expr::Try(Box::new(read_member)),
                ));
                statements.extend(finish_read_header(&plan));
                statements.push(rust::Statement::Tail(construct(rust::Expr::Path(
                    Path::single("value"),
                ))));
                rust::Expr::Block(rust::Block { statements })
            }
        };

        if is_default {
            default_arm = Some(build.clone());
        } else {
            for value in &label_values {
                decode_arms.push(rust::MatchArm {
                    pattern: label_pattern(*value)?,
                    guard: None,
                    body: build.clone(),
                });
            }
        }

        // `Self::Variant(value) => { <discriminator write>; <member write>; }`
        // with the discriminator the case fixes, or
        // `Self::Variant(discriminator, value) => { <check>; <discriminator
        // write>; <member write>; }` with the one the variant carries.
        let mut statements = Vec::new();
        if carries {
            let own = (!is_default).then_some(label_values.as_slice());
            let what = if is_default {
                format!("a discriminator no label of `{name}` takes, for its `default:` member")
            } else {
                format!("a label of the member `{}` of `{name}`", member.name)
            };
            statements.push(check_carried(own, what)?);
        }
        if let Some(plan) = &discriminator_plan {
            statements.extend(open_header(plan, steps.discriminator));
        }
        let discriminator_write_expr = if carries {
            write_carried()?
        } else {
            // A case that does not carry its discriminator has exactly one
            // label: `carries_discriminator` is true for every other.
            let label = label_values.first().copied().ok_or(EncodeError::NoWriter {
                member: member.name.clone(),
                kind: "a union case carrying no case label",
            })?;
            write_expr_for(&discriminator_write, written_label(label)?, &name, false)?
        };
        statements.push(rust::Statement::Semi(discriminator_write_expr));
        if let Some(plan) = &discriminator_plan {
            statements.extend(close_header(plan));
        }
        if let Some(plan) = &case_plan {
            statements.extend(open_header(plan, steps.member(plan, &member.name)?));
        }
        // `value` is the variant's binding, which a `match` on `&self`
        // makes a reference.
        statements.push(rust::Statement::Semi(write_expr_for(
            &member_write,
            rust::Expr::Path(Path::single("value")),
            &member.name,
            true,
        )?));
        if let Some(plan) = &case_plan {
            statements.extend(close_header(plan));
        }
        let mut elements = Vec::new();
        if carries {
            elements.push(rust::Pattern::Binding(Ident::new("discriminator")));
        }
        elements.push(rust::Pattern::Binding(Ident::new("value")));
        encode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&["Self", &variant]),
                elements,
            },
            guard: None,
            body: rust::Expr::Block(rust::Block { statements }),
        });
    }

    // The variant that holds a discriminator selecting no member
    // (§7.4.3.5: the member is absent): the discriminator and nothing
    // after it, under the same check as the `default:` case's.
    if shape.no_member {
        let mut statements = vec![check_carried(
            None,
            format!("a discriminator no label of `{name}` takes, for no member"),
        )?];
        if let Some(plan) = &discriminator_plan {
            statements.extend(open_header(plan, steps.discriminator));
        }
        statements.push(rust::Statement::Semi(write_carried()?));
        if let Some(plan) = &discriminator_plan {
            statements.extend(close_header(plan));
        }
        encode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&["Self", crate::rust_union::NO_MEMBER]),
                elements: vec![rust::Pattern::Binding(Ident::new("discriminator"))],
            },
            guard: None,
            body: rust::Expr::Block(rust::Block { statements }),
        });
    }

    // Past the labels: the `default:` case, or the variant that holds no
    // member. A Rust `char` has more values than an IDL `char` or `wchar`
    // labels can cover, so a character discriminator always has the arm;
    // an integer, `bool` or enumeration discriminator whose labels cover
    // its type has no value left for it.
    let rest = default_arm.or_else(|| {
        shape.no_member.then(|| rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&[
                    "Self",
                    crate::rust_union::NO_MEMBER,
                ]))),
                args: vec![rust::Expr::Path(Path::single("discriminator"))],
            }],
        })
    });
    match rest {
        Some(body) => decode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::Wildcard,
            guard: None,
            body,
        }),
        None if shape.kind == crate::rust_union::DiscriminatorKind::Character => {
            decode_arms.push(refusal_arm(&format!(
                "a discriminator selecting a case of `{name}`"
            )));
        }
        None => {}
    }

    // An enumeration discriminator is read as its holder integer and
    // turned into the enumerator here, so the arms above match — and the
    // variants carry — the enumeration and not its holder. A holder no
    // enumerator has is refused, as the enumeration's own `decode` does.
    let to_enumeration = match shape.kind {
        crate::rust_union::DiscriminatorKind::Enumeration(enumeration) => {
            let mut arms = Vec::new();
            for enumerator in ast.enumerators_of(enumeration) {
                let (_, value) = enumerator_variant_and_value(
                    ast,
                    &enumeration_name(enumeration),
                    *enumerator,
                    key,
                )?;
                arms.push(rust::MatchArm {
                    pattern: rust::Pattern::Literal(Literal::int(value)),
                    guard: None,
                    body: rust::Expr::Path(
                        crate::rust_union::enumerator_path_of(
                            ast,
                            node.module,
                            enumeration,
                            *enumerator,
                        )
                        .map_err(refuse_label)?,
                    ),
                });
            }
            arms.push(rust::MatchArm {
                pattern: rust::Pattern::Wildcard,
                guard: None,
                body: refused(
                    format!("an enumerator of the discriminator of `{name}`"),
                    "r",
                ),
            });
            Some(rust::Statement::Let {
                name: Ident::new("discriminator"),
                type_: None,
                value: Some(rust::Expr::Match {
                    scrutinee: Box::new(rust::Expr::Path(Path::single("discriminator"))),
                    arms,
                }),
                mutable: false,
            })
        }
        _ => None,
    };

    let mut encode_statements = Vec::new();
    if per_sample(key) {
        // As in `encode_fns`: the flag is read only by a case member or
        // by the discriminator whose header form the sample decides.
        encode_statements.push(rust::Statement::Let {
            name: Ident::new("_"),
            type_: None,
            value: Some(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            mutable: false,
        });
    }
    if layout.needs_dheader {
        encode_statements.push(let_statement("dheader", {
            rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Path(Path::single("w"))),
                method: Ident::new("write_dheader"),
                args: Vec::new(),
            }
        }));
    }
    encode_statements.push(rust::Statement::Semi(rust::Expr::Match {
        scrutinee: Box::new(rust::Expr::Path(Path::single("self"))),
        arms: encode_arms,
    }));
    if layout.needs_sentinel {
        encode_statements.push(call_statement("write_sentinel", Vec::new()));
    }
    if layout.needs_dheader {
        encode_statements.push(call_statement(
            "finish_dheader",
            vec![rust::Expr::Path(Path::single("dheader"))],
        ));
    }
    encode_statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Tuple(Vec::new())],
    }));
    // As in `encode_fns`: under `ElementPadding::Packed` a union value is
    // its own alignment block. See `zerodds-wire-codec`'s `write_union`.
    let encode_statements = wrap_packed_body(
        encode_statements,
        "w",
        key.element_padding == ElementPadding::Packed,
    );
    let encode = encode_function(&body_name(codec, key), &encode_statements, per_sample(key));

    let decode = rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&format!("decode{codec}")),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Reader"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block {
            statements: wrap_packed_body(
                union_decode_body(
                    layout,
                    discriminator_plan,
                    &discriminator_write,
                    to_enumeration,
                    decode_arms,
                )?,
                "r",
                key.element_padding == ElementPadding::Packed,
            ),
        }),
    });

    let mut out = with_entry(encode, encode_entry(ast, type_, key, codec)?);
    out.push(decode);
    Ok(out)
}

/// The body of a union's `decode`: the framing the plan gives it around
/// the discriminator's read and the match that reads the branch.
///
/// A `@final` union keeps the shape it always had — the discriminator,
/// then the match as the tail — so its generated code does not change.
fn union_decode_body(
    layout: &TypeLayout,
    discriminator_plan: Option<MemberPlan>,
    discriminator_write: &MemberWrite,
    to_enumeration: Option<rust::Statement>,
    decode_arms: Vec<rust::MatchArm>,
) -> Result<Vec<rust::Statement>, EncodeError> {
    let read_discriminator = rust::Statement::Let {
        name: Ident::new("discriminator"),
        type_: None,
        value: Some(rust::Expr::Try(Box::new(read_for(
            "discriminator",
            discriminator_write,
        )?))),
        mutable: false,
    };
    let branch = rust::Expr::Match {
        scrutinee: Box::new(rust::Expr::Path(Path::single("discriminator"))),
        arms: decode_arms,
    };
    if !layout.needs_dheader && !layout.needs_sentinel && discriminator_plan.is_none() {
        let mut statements = vec![read_discriminator];
        statements.extend(to_enumeration);
        statements.push(rust::Statement::Tail(branch));
        return Ok(statements);
    }

    let mut statements = Vec::new();
    if layout.needs_dheader {
        // Where the writer's union ends: a newer writer's branch may
        // carry more than this reader's type knows, and the reader steps
        // over it rather than reading the next value from inside it.
        statements.push(let_statement(
            "end",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(rust::Expr::Try(Box::new(call("read_dheader", Vec::new())))),
                rhs: Box::new(call("position", Vec::new())),
            },
        ));
    }
    if let Some(plan) = &discriminator_plan {
        statements.extend(read_header(plan));
    }
    statements.push(read_discriminator);
    if let Some(plan) = &discriminator_plan {
        statements.extend(finish_read_header(plan));
    }
    statements.extend(to_enumeration);
    statements.push(let_statement("value", rust::Expr::Try(Box::new(branch))));
    if layout.needs_sentinel {
        statements.extend(read_sentinel());
    }
    if layout.needs_dheader {
        statements.push(rust::Statement::Semi(rust::Expr::Try(Box::new(call(
            "skip_to",
            vec![rust::Expr::Path(Path::single("end"))],
        )))));
    }
    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Path(Path::single("value"))],
    }));
    Ok(statements)
}

/// Rule (23)'s `PID_SENTINEL` at the end of a PL_CDR1 parameter list,
/// read and checked: a parameter list that ends anywhere else is refused.
fn read_sentinel() -> Vec<rust::Statement> {
    vec![
        let_statement(
            "sentinel",
            rust::Expr::Try(Box::new(call("read_pid", Vec::new()))),
        ),
        rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Not(Box::new(rust::Expr::Field {
                receiver: Box::new(rust::Expr::Path(Path::single("sentinel"))),
                name: Ident::new("end"),
            }))),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Return(Some(Box::new(
                    rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::single("Err"))),
                        args: vec![rust::Expr::Call {
                            callee: Box::new(rust::Expr::Path(Path::new(&[
                                "WireError",
                                "refused",
                            ]))),
                            args: vec![
                                call("position", Vec::new()),
                                rust::Expr::Literal(Literal::string(
                                    "PID_SENTINEL closing the parameter list, per rule (23)",
                                )),
                            ],
                        }],
                    },
                ))))],
            },
            otherwise: None,
        }),
    ]
}

/// A member header's read, before the value, for a member read where
/// the stream puts it — the decoder's half of [`open_header`].
fn read_header(plan: &MemberPlan) -> Vec<rust::Statement> {
    match plan.header {
        MemberHeader::NoHeader | MemberHeader::OptionalFlag => Vec::new(),
        MemberHeader::PidCdr1 { .. } | MemberHeader::OptionalCdr1 { .. } => vec![let_statement(
            "pid",
            rust::Expr::Try(Box::new(call("read_pid", Vec::new()))),
        )],
        MemberHeader::EmHeader { .. } => vec![let_statement(
            "_emheader",
            rust::Expr::Try(Box::new(call("read_emheader", Vec::new()))),
        )],
    }
}

/// What closes a header after its value on the read side — the
/// decoder's half of [`close_header`].
fn finish_read_header(plan: &MemberPlan) -> Vec<rust::Statement> {
    match plan.header {
        MemberHeader::PidCdr1 { .. } | MemberHeader::OptionalCdr1 { .. } => {
            vec![rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                "finish_pid",
                vec![rust::Expr::Path(Path::single("pid"))],
            ))))]
        }
        MemberHeader::NoHeader | MemberHeader::OptionalFlag | MemberHeader::EmHeader { .. } => {
            Vec::new()
        }
    }
}

/// The write of one value, as an expression, with its `?` where the
/// write can refuse.
fn write_expr_for(
    kind: &MemberWrite,
    value: rust::Expr,
    at: &str,
    borrowed: bool,
) -> Result<rust::Expr, EncodeError> {
    let (expr, fallible) = write_call("w", SinkShape::Reference, at, kind, value, borrowed)?;
    Ok(if fallible {
        rust::Expr::Try(Box::new(expr))
    } else {
        expr
    })
}

/// The unsigned Rust type an enumeration's holder takes, and the
/// `Writer`/`Reader` method for it.
///
/// `zerodds_ast_layout::enumeration_bit_bound` decides the width, the
/// same crossing function the member position asks — a second table
/// here would be a second place for a holder to be the wrong size.
const fn enum_holder(bits: u16) -> (rust::Primitive, &'static str, &'static str) {
    match bits {
        0..=8 => (rust::Primitive::U8, "write_u8", "read_u8"),
        9..=16 => (rust::Primitive::U16, "write_u16", "read_u16"),
        17..=32 => (rust::Primitive::U32, "write_u32", "read_u32"),
        _ => (rust::Primitive::U64, "write_u64", "read_u64"),
    }
}

/// The encoder body for an enumeration: its holder, at the width its bit
/// bound gives, and — under [`per_sample`] — the public entry beside it.
///
/// An enumeration's body reads no [`EXTENDED_PARAM`] and never writes a
/// parameter header: `keeps_short_header` counts an enumeration among the
/// types that keep the short form in a sample past RTI's step. The body
/// takes the flag anyway because a `MemberWrite::Nested` call on an
/// enumeration names it, and the same name under every plan is what keeps
/// a call from framing an enumeration member with the wrong encoder.
///
/// A `match` and not `*self as uN`. The cast is a move, and
/// `crate::adapter::cst` derives nothing on what it declares — not
/// `Copy` either — so the cast form did not compile. The match is also
/// the honest one: it names each enumerator's wire value where a
/// reader can compare it with `decode`'s.
fn enumeration_encode_fns(
    ast: &Ast,
    type_: Id<TypeNode>,
    enumerators: &[Id<zerodds_ast::EnumeratorNode>],
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Item>, EncodeError> {
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let bits = zerodds_ast_layout::enumeration_bit_bound(ast, type_, key.enum_holder);
    let (primitive, writer, _) = enum_holder(bits);

    let mut arms = Vec::new();
    for id in enumerators {
        let (name, value) = enumerator_variant_and_value(ast, &node.name, *id, key)?;
        arms.push(rust::MatchArm {
            pattern: rust::Pattern::Path(Path::new(&["Self", &name])),
            guard: None,
            body: rust::Expr::Literal(Literal::int(value)),
        });
    }

    let mut statements = Vec::new();
    if per_sample(key) {
        statements.push(rust::Statement::Let {
            name: Ident::new("_"),
            type_: None,
            value: Some(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            mutable: false,
        });
    }
    statements.extend([
        rust::Statement::Let {
            name: Ident::new("value"),
            type_: Some(rust::Type::Primitive(primitive)),
            value: Some(rust::Expr::Match {
                scrutinee: Box::new(rust::Expr::Path(Path::single("self"))),
                arms,
            }),
            mutable: false,
        },
        rust::Statement::Semi(rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("w"))),
            method: Ident::new(writer),
            args: vec![rust::Expr::Path(Path::single("value"))],
        }),
        rust::Statement::Tail(rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![rust::Expr::Tuple(Vec::new())],
        }),
    ]);
    Ok(with_entry(
        encode_function(&body_name(codec, key), &statements, per_sample(key)),
        encode_entry(ast, type_, key, codec)?,
    ))
}

/// One enumerator's Rust variant name and the integer it serialises as.
///
/// The value goes through
/// [`zerodds_ast_layout::enumerator_wire_value`] and not through
/// `Ast::effective_enumerator_value`. The two answer differently under
/// [`zerodds_ast_layout::EnumeratorValue::Ordinal`], and this integer is
/// the one that leaves the process: `enumeration_encode_fn` puts it in
/// the holder and `enumeration_decode_fn` matches an incoming holder
/// against it. Reading the declared value here wrote `@value(200)`
/// where a front end that drops the annotation writes `1`, in both
/// directions, with nothing at run time saying so — the Rust column
/// carries no corpus row that reaches this code, so only
/// `crates/wire-probe/tests/enumerator_value.rs::the_sites_go_through_the_crossing`
/// stands between it and a silent bypass.
///
/// The Rust *discriminant* is a different question and stays declared:
/// `crate::adapter::cst` writes it, and it is the language mapping.
fn enumerator_variant_and_value(
    ast: &Ast,
    enumeration: &str,
    id: Id<zerodds_ast::EnumeratorNode>,
    key: LayoutKey,
) -> Result<(String, i128), EncodeError> {
    let enumerator = ast.enumerator(id).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let value = zerodds_ast_layout::enumerator_wire_value(ast, id, key.enumerator_value)
        .ok_or_else(|| EncodeError::NoEnumeratorValue {
            enumeration: enumeration.to_string(),
            enumerator: enumerator.name.clone(),
            detail: "the enumerator has no computable wire value",
        })?;
    // `escape_keyword` and not `variant_name`: the declaration is
    // `crate::adapter::cst`'s and it writes the enumerator's own name.
    // `variant_name` capitalises, which is what a *union* case takes —
    // it is built from a member name — and using it here produced
    // `Self::Missing_node` for an enumerator declared `missing_node`.
    Ok((rust::escape_keyword(&enumerator.name), value))
}

/// `fn decode(r: &mut Reader) -> Result<Self, WireError>` for an
/// enumeration.
///
/// A value that names no enumerator is **refused** and not mapped onto
/// the first variant. §7.2.2.4.1.2 gives an enumeration a fixed set of
/// literals; a decoder that answered `RED` for a value nobody wrote
/// would hand the caller a sample that never existed, which is the
/// silent half of the drop class.
fn enumeration_decode_fn(
    ast: &Ast,
    type_: Id<TypeNode>,
    enumerators: &[Id<zerodds_ast::EnumeratorNode>],
    key: LayoutKey,
    codec: &'static str,
) -> Result<rust::Item, EncodeError> {
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let bits = zerodds_ast_layout::enumeration_bit_bound(ast, type_, key.enum_holder);
    let (_, _, reader) = enum_holder(bits);

    let mut arms = Vec::new();
    for id in enumerators {
        let (name, value) = enumerator_variant_and_value(ast, &node.name, *id, key)?;
        arms.push(rust::MatchArm {
            pattern: rust::Pattern::Literal(Literal::int(value)),
            guard: None,
            body: rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                args: vec![rust::Expr::Path(Path::new(&["Self", &name]))],
            },
        });
    }
    arms.push(refusal_arm(&format!("an enumerator of `{}`", node.name)));

    let statements = vec![
        rust::Statement::Let {
            name: Ident::new("value"),
            type_: None,
            value: Some(rust::Expr::Try(Box::new(rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Path(Path::single("r"))),
                method: Ident::new(reader),
                args: Vec::new(),
            }))),
            mutable: false,
        },
        rust::Statement::Tail(rust::Expr::Match {
            scrutinee: Box::new(rust::Expr::Path(Path::single("value"))),
            arms,
        }),
    ];

    Ok(rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&format!("decode{codec}")),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Reader"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block { statements }),
    }))
}

/// The anchor a bitset packs into, in octets.
///
/// The **plan's** answer and not a sum of the widths: an anchor is one,
/// two, four or eight octets whatever the bitfields add up to, and
/// `zerodds_ast_layout::xcdr::plan` is the crossing the other twenty
/// columns ask through `Extent::Sized`.
fn bitset_anchor(layout: &TypeLayout, name: &str) -> Result<u32, EncodeError> {
    match layout.extent {
        Extent::Sized(anchor) => Ok(anchor),
        Extent::Unbounded => Err(EncodeError::NoWriter {
            member: name.to_string(),
            kind: "bitset whose anchor the planner does not size",
        }),
    }
}

/// Each named bitfield with the width and the position it takes in the
/// anchor.
///
/// Positions run cumulatively from zero in declaration order, **padding
/// included**: an anonymous `bitfield<4>` carries no value and still
/// moves the next field along. The whole inheritance chain, base first,
/// because that is what the planner sizes the anchor over —
/// `crates/lang-rules/src/emit/encode.rs` reads the same list the same
/// way.
///
/// # Errors
/// [`EncodeError::NoWriter`] for a chain that runs past the sixty-four
/// bits an anchor holds — never a field silently dropped off the end.
fn bitset_fields(
    ast: &Ast,
    type_: Id<TypeNode>,
    name: &str,
) -> Result<Vec<(String, u16, u32)>, EncodeError> {
    let mut out = Vec::new();
    let mut position: u32 = 0;
    for field in &ast.all_bitfields_of(type_) {
        // Refused and not skipped: a skipped field takes its width out
        // of the running position, so every field after it would be
        // packed at an offset the peer does not read it from — a sample
        // that encodes, reports success and means something else.
        let Ok(field_node) = ast.bitfield(*field) else {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind: "bitset with a stale bitfield handle",
            });
        };
        let width = u32::from(field_node.bits);
        if position.saturating_add(width) > 64 {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind: "bitset wider than the sixty-four bits an anchor holds",
            });
        }
        if let Some(field_name) = field_node.name.as_ref() {
            out.push((
                rust::escape_keyword(field_name).to_string(),
                field_node.bits,
                position,
            ));
        }
        position = position.saturating_add(width);
    }
    Ok(out)
}

/// A bitset's encoder body — each field shifted into the anchor, then the
/// anchor — and, under [`per_sample`], the public entry beside it.
///
/// As `enumeration_encode_fns`'s: the body reads no [`EXTENDED_PARAM`]
/// and takes it anyway, because a `MemberWrite::Nested` call on a bitset
/// names it.
///
/// `Writer::bitfield` does the shift and refuses a value the field's
/// width cannot hold. The generated field is as wide as its **holder**
/// — `bitfield<3> a` is a `u8` — so a value that does not fit is a
/// program error the writer reports rather than an octet it masks off.
fn bitset_encode_fns(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Item>, EncodeError> {
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let anchor = bitset_anchor(layout, &node.name)?;
    let mut statements = Vec::new();
    if per_sample(key) {
        statements.push(rust::Statement::Let {
            name: Ident::new("_"),
            type_: None,
            value: Some(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            mutable: false,
        });
    }
    statements.push(rust::Statement::Let {
        name: Ident::new("bits"),
        type_: Some(rust::Type::Primitive(rust::Primitive::U64)),
        value: Some(rust::Expr::Literal(Literal::int(0))),
        mutable: true,
    });
    for (field, bits, position) in bitset_fields(ast, type_, &node.name)? {
        let value = rust::Expr::Field {
            receiver: Box::new(rust::Expr::Path(Path::single("self"))),
            name: Ident::new(&field),
        };
        statements.push(rust::Statement::Semi(rust::Expr::Assign {
            target: Box::new(rust::Expr::Path(Path::single("bits"))),
            value: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::BitOr,
                lhs: Box::new(rust::Expr::Path(Path::single("bits"))),
                rhs: Box::new(rust::Expr::Try(Box::new(rust::Expr::MethodCall {
                    receiver: Box::new(rust::Expr::Path(Path::single("w"))),
                    method: Ident::new("bitfield"),
                    args: vec![
                        widen_to_u64(value, bits),
                        rust::Expr::Literal(Literal::int(i128::from(bits))),
                        rust::Expr::Literal(Literal::int(i128::from(position))),
                    ],
                }))),
            }),
        }));
    }
    statements.push(rust::Statement::Semi(rust::Expr::Try(Box::new(
        rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("w"))),
            method: Ident::new("write_bitset"),
            args: vec![
                rust::Expr::Literal(Literal::int(i128::from(anchor))),
                rust::Expr::Path(Path::single("bits")),
            ],
        },
    ))));
    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Tuple(Vec::new())],
    }));
    Ok(with_entry(
        encode_function(&body_name(codec, key), &statements, per_sample(key)),
        encode_entry(ast, type_, key, codec)?,
    ))
}

/// A bitset's `decode`: the anchor, then each field shifted and masked
/// back out of it.
///
/// The mask is written out rather than left to the narrowing cast: the
/// anchor is sixty-four bits and the field is as wide as its holder, so
/// `bitfield<3> a` in a `u8` would otherwise take five bits that belong
/// to the field beside it.
fn bitset_decode_fn(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    codec: &'static str,
) -> Result<rust::Item, EncodeError> {
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    let anchor = bitset_anchor(layout, &node.name)?;
    let mut statements = vec![rust::Statement::Let {
        name: Ident::new("bits"),
        type_: None,
        value: Some(rust::Expr::Try(Box::new(rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("r"))),
            method: Ident::new("read_bitset"),
            args: vec![rust::Expr::Literal(Literal::int(i128::from(anchor)))],
        }))),
        mutable: false,
    }];
    let mut fields = Vec::new();
    for (field, bits, position) in bitset_fields(ast, type_, &node.name)? {
        let mask: u64 = if bits >= 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let masked = rust::Expr::Binary {
            op: rust::BinaryOp::BitAnd,
            lhs: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Shr,
                lhs: Box::new(rust::Expr::Path(Path::single("bits"))),
                rhs: Box::new(rust::Expr::Literal(Literal::int(i128::from(position)))),
            }),
            rhs: Box::new(rust::Expr::Literal(Literal::int(i128::from(mask)))),
        };
        let holder = crate::adapter::cst::bitmask_primitive(bits);
        let value = if holder == rust::Primitive::U64 {
            masked
        } else {
            rust::Expr::Cast {
                value: Box::new(masked),
                type_: Box::new(rust::Type::Primitive(holder)),
            }
        };
        fields.push((Ident::new(&field), value));
    }
    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::StructLit {
            path: Path::single("Self"),
            fields,
            base: None,
        }],
    }));
    Ok(rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&format!("decode{codec}")),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Reader"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block { statements }),
    }))
}

/// The encoder **body** for a structure or a valuetype, and — under
/// [`per_sample`] — the public entry beside it.
///
/// The statements follow the plan's member order, which is wire order
/// and not necessarily declaration order.
///
/// Fallible even for a type with nothing fallible in it, so the
/// signature is the type's and not its members'. `char` and `wchar`
/// refuse a code point they cannot carry, and an `encode` whose
/// signature changed with the member list would make that a source
/// break for every caller the day someone added one.
///
/// `steps` names the members whose PL_CDR1 header form the sample
/// decides, and the body reads [`EXTENDED_PARAM`] for each of them.
fn encode_fns(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    steps: &StepHeaders,
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Item>, EncodeError> {
    let mut statements = Vec::new();
    // The flag is read by the members whose header form the sample
    // decides, and by nothing else: a `@final` aggregate of primitives
    // takes it and never looks at it, which is `unused_variables` in
    // every crate that includes the generated text. `let _ =` is valid
    // whether or not the body goes on to use it, so it is emitted
    // unconditionally rather than from a guess about which bodies read it
    // — the same shape `crate::emit::encode`'s `with_extended_used` uses
    // for the twenty other languages.
    if per_sample(key) {
        statements.push(rust::Statement::Let {
            name: Ident::new("_"),
            type_: None,
            value: Some(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            mutable: false,
        });
    }

    // The DHEADER the plan asks for, and nothing more: whether an
    // appendable type carries one is the plan's decision, made once.
    if layout.needs_dheader {
        // The length is not known until the members are written, so the
        // header is a placeholder the runtime patches. A generator that
        // wrote a length it computed itself would be computing the
        // layout a second time.
        statements.push(rust::Statement::Let {
            name: Ident::new("dheader"),
            type_: None,
            value: Some(rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Path(Path::single("w"))),
                method: Ident::new("write_dheader"),
                args: Vec::new(),
            }),
            mutable: false,
        });
    }

    for plan in &layout.members {
        let Some(member) = member_of(ast, type_, plan.member, key, codec) else {
            continue;
        };
        let name = member.0;
        let kind = member.1;
        let optional = member_node_of(ast, type_, plan.member)
            .is_some_and(|member| member.facets.is_optional());

        let receiver = holder_of(ast, type_, plan.member);
        if optional {
            statements.extend(write_optional_member(
                receiver,
                plan,
                &name,
                &kind,
                steps.member(plan, &name)?,
            )?);
            continue;
        }

        // The header the plan chose, including the length code — this
        // is where `LengthCodePolicy` becomes visible in the output. A
        // member the sample's step decides gets the run-time branch
        // instead, because the plan named the short side of a decision
        // the sample makes.
        statements.extend(open_header(plan, steps.member(plan, &name)?));
        statements.push(write_for(receiver, &name, &kind)?);
        // The header's length is patched after the value, because that
        // is when it is known.
        statements.extend(close_header(plan));
    }

    // The sentinel a PL_CDR1 parameter list ends with. A reader that
    // stops at the last member it recognises instead of at the sentinel
    // misreads whatever follows.
    if layout.needs_sentinel {
        statements.push(call_statement("write_sentinel", Vec::new()));
    }

    if layout.needs_dheader {
        statements.push(call_statement(
            "finish_dheader",
            vec![rust::Expr::Path(Path::single("dheader"))],
        ));
    }

    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Tuple(Vec::new())],
    }));

    // An aggregate value is its own alignment block under
    // `ElementPadding::Packed`; see `zerodds-wire-codec`'s
    // `write_aggregate`.
    let statements = wrap_packed_body(
        statements,
        "w",
        key.element_padding == ElementPadding::Packed,
    );

    Ok(with_entry(
        encode_function(&body_name(codec, key), &statements, per_sample(key)),
        encode_entry(ast, type_, key, codec)?,
    ))
}

/// A generated `encode`, under either name the per-sample split gives it.
///
/// `flagged` is the body, which takes [`EXTENDED_PARAM`] after the writer;
/// the public entry takes no flag, because the sample whose root this type
/// is resolves the decision itself and passes the constant it resolved.
fn encode_function(name: &str, statements: &[rust::Statement], flagged: bool) -> rust::Item {
    let mut params = vec![
        rust::Param {
            name: None,
            type_: rust::Type::unit(),
            receiver: Some(rust::Receiver::Ref),
        },
        rust::Param {
            name: Some(Ident::new("w")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Writer"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        },
    ];
    // After the writer, so the two the entry passes read in that order:
    // the sink, then the sample's decision.
    if flagged {
        params.push(rust::Param {
            name: Some(Ident::new(EXTENDED_PARAM)),
            type_: rust::Type::Primitive(rust::Primitive::Bool),
            receiver: None,
        });
    }
    rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        params,
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::unit(),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block {
            statements: statements.to_vec(),
        }),
    })
}

/// The body, and the public entry that bakes this type's own step
/// decision in beside it — or the body alone, for every plan whose header
/// form is fixed.
fn with_entry(body: rust::Item, entry: Option<rust::Item>) -> Vec<rust::Item> {
    match entry {
        Some(entry) => vec![body, entry],
        None => vec![body],
    }
}

/// The public `encode(&self, w)` of a type under [`per_sample`], and
/// `None` without it.
///
/// RTI decides the PL_CDR1 header form for the **sample's** type, so the
/// sample whose root this type is resolves the decision once, here, and
/// hands it to the body — which threads it to every type the body calls.
/// The same nested `@mutable` type is then framed short inside a small
/// sample and extended inside a large one, which a single `encode` per
/// type cannot be.
///
/// The decision itself is [`sample_past_step`]'s, which asks
/// [`LayoutKey::for_sample`] — the crossing the reflective encoder makes
/// for the same sample, and the one that keeps
/// `crates/wire-codec`'s encoder and this codec writing the same octets.
///
/// # Errors
/// [`EncodeError`] when the sample's type cannot be sized, which is what
/// [`LayoutKey::for_sample`] reports.
fn encode_entry(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    codec: &'static str,
) -> Result<Option<rust::Item>, EncodeError> {
    if !per_sample(key) {
        return Ok(None);
    }
    let past = sample_past_step(ast, type_, key)?;
    let statements = vec![rust::Statement::Tail(rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("self"))),
        method: Ident::new(&body_name(codec, key)),
        args: vec![
            rust::Expr::Path(Path::single("w")),
            rust::Expr::Literal(Literal::Bool(past)),
        ],
    })];
    Ok(Some(encode_function(
        &format!("encode{codec}"),
        &statements,
        false,
    )))
}

/// The statements that open one member's framing, before its value.
///
/// Split out of the loop because an `@optional` member writes some of
/// them inside a `match` arm and some outside it, and a second copy of
/// the header shapes would be a second place for a length code to be
/// wrong.
///
/// `chosen` says that the sample, not the plan, picks the PL_CDR1 header
/// form, and the statements are then a run-time branch on
/// [`EXTENDED_PARAM`] between `write_pid` and `write_pid_extended`. It is
/// what [`StepHeaders`] records for the member and what the enclosing
/// body threads down from the sample's root.
fn open_header(plan: &zerodds_ast_layout::plan::MemberPlan, chosen: bool) -> Vec<rust::Statement> {
    match plan.header {
        MemberHeader::NoHeader => Vec::new(),
        MemberHeader::PidCdr1 {
            pid,
            extended,
            must_understand,
        } => vec![bind_pid_header(
            chosen,
            (pid, plan.member_id),
            extended,
            "Member",
            must_understand,
        )],
        // Rule (19) and §7.4.1.2. The `Optional` role is what makes
        // the declared length follow the optional convention rather
        // than the `@mutable` one; the two differ in Cyclone.
        MemberHeader::OptionalCdr1 {
            pid,
            extended,
            must_understand,
        } => vec![bind_pid_header(
            chosen,
            (pid, plan.member_id),
            extended,
            "Optional",
            must_understand,
        )],
        // Rule (20), §7.4.3.5.2: a BOOLEAN `<is_present>`. Written by
        // `write_optional_member`, which is the only caller that reaches
        // this arm and knows whether the member is there.
        MemberHeader::OptionalFlag => Vec::new(),
        MemberHeader::EmHeader {
            lc,
            must_understand,
        } => {
            // The member id and the length code both come from the
            // plan: the id is the completion pass's, the code is the
            // policy's, and neither is decided again here.
            vec![bind_header(
                "emheader",
                "write_emheader",
                vec![
                    rust::Expr::Literal(Literal::int(i128::from(plan.member_id))),
                    rust::Expr::Literal(Literal::int(i128::from(lc as u8))),
                    rust::Expr::Literal(Literal::Bool(must_understand)),
                ],
            )]
        }
    }
}

/// The `let pid = …` that opens a PL_CDR1 parameter header, in the form
/// `plan` names or in the one the sample's flag chooses.
///
/// Both forms call the same two runtime methods, so the branch is over
/// the *call* and not over anything else: `finish_pid` patches whichever
/// header was opened and reads its form back off the stream
/// (§7.4.1.2.3), and the id, the role and the flag are the plan's in
/// either arm. `chosen` with a plan that already names the extended form
/// cannot happen: [`StepHeaders`] records a member as chosen only where
/// the two sides of the step disagree, and the plan the codec is written
/// from is the short side.
fn bind_pid_header(
    chosen: bool,
    (pid, member_id): (u16, u32),
    extended: bool,
    role: &str,
    must_understand: bool,
) -> rust::Statement {
    // The short header carries the 16-bit parameter id; the extended one
    // the member id in 28 bits (XTypes 1.3 §7.4.1.2.2). The plan's `pid`
    // saturates at `u16::MAX` for a member id past 16 bits — the extended
    // form is then mandatory and takes `member_id`, or a `@hashid` member
    // goes out as member 65535.
    let writer = |method: &str| {
        let id = if method == "write_pid_extended" {
            i128::from(member_id)
        } else {
            i128::from(pid)
        };
        rust::Expr::Try(Box::new(rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("w"))),
            method: Ident::new(method),
            args: vec![
                rust::Expr::Literal(Literal::int(id)),
                parameter_role(role),
                rust::Expr::Literal(Literal::Bool(must_understand)),
            ],
        }))
    };
    let value = if !chosen {
        writer(if extended {
            "write_pid_extended"
        } else {
            "write_pid"
        })
    } else {
        // `zd_extended` is the sample's answer, resolved once by
        // [`sample_past_step`] in the entry that called this body and
        // threaded down to here.
        rust::Expr::If {
            condition: Box::new(rust::Expr::Path(Path::single(EXTENDED_PARAM))),
            then: rust::Block {
                statements: vec![rust::Statement::Tail(writer("write_pid_extended"))],
            },
            otherwise: Some(rust::Block {
                statements: vec![rust::Statement::Tail(writer("write_pid"))],
            }),
        }
    };
    rust::Statement::Let {
        name: Ident::new("pid"),
        type_: None,
        value: Some(value),
        mutable: false,
    }
}

/// The statements that close one member's framing, after its value.
fn close_header(plan: &zerodds_ast_layout::plan::MemberPlan) -> Vec<rust::Statement> {
    match plan.header {
        MemberHeader::NoHeader | MemberHeader::OptionalFlag => Vec::new(),
        MemberHeader::PidCdr1 { .. } | MemberHeader::OptionalCdr1 { .. } => vec![call_statement(
            "finish_pid",
            vec![rust::Expr::Path(Path::single("pid"))],
        )],
        // `?`: under a length code of 0 to 3 the code is the member's
        // length, and `finish_emheader` refuses a value of another size
        // rather than let the header misdeclare it (§7.4.3.4.2).
        MemberHeader::EmHeader { .. } => vec![try_call_statement(
            "finish_emheader",
            vec![rust::Expr::Path(Path::single("emheader"))],
        )],
    }
}

/// One `@optional` member's write, over the `Option<T>` field.
///
/// Three shapes, one per way the specification marks the absence, and
/// the *field* is what says which one this sample wants:
///
/// - rule (20), XCDR2 outside a `@mutable` type: a BOOLEAN
///   `<is_present>` either way, and the value only when it is `true`;
/// - rule (19), XCDR1 outside a `@mutable` type: the parameter header
///   either way, with a declared length of zero for the absence
///   (§7.4.1.2), which is what `finish_pid` computes from the bytes
///   that follow;
/// - inside a `@mutable` type: no parameter at all. §7.4.1.2.3 —
///   a member the stream does not carry is one the reader defaults, and
///   for an `@optional` member the default is the absence itself.
///
/// The match is over `&self.<name>`, so the binding is a borrow and the
/// value goes to the runtime the same way a borrowed field does.
///
/// `chosen` is [`open_header`]'s, passed on: an `@optional` member of a
/// `@mutable` type is framed as an `MMEMBER` and takes the same run-time
/// branch as every other member of the type.
fn write_optional_member(
    receiver: rust::Expr,
    plan: &zerodds_ast_layout::plan::MemberPlan,
    name: &str,
    kind: &MemberWrite,
    chosen: bool,
) -> Result<Vec<rust::Statement>, EncodeError> {
    let bound = rust::escape_keyword(name);
    let value = rust::Expr::Path(Path::single(&bound));

    let (mut present, absent) = match plan.header {
        MemberHeader::OptionalFlag => (
            vec![call_statement(
                "write_bool",
                vec![rust::Expr::Literal(Literal::Bool(true))],
            )],
            vec![call_statement(
                "write_bool",
                vec![rust::Expr::Literal(Literal::Bool(false))],
            )],
        ),
        // The header is outside the match: it is written whether or not
        // the value is, and its declared length is the marker.
        MemberHeader::OptionalCdr1 { .. } => (Vec::new(), Vec::new()),
        // A parameter list simply omits the parameter, so the header
        // goes inside the arm with the value.
        _ => (open_header(plan, chosen), Vec::new()),
    };
    present.push(write_value_to("w", name, kind, value, true)?);
    if !matches!(plan.header, MemberHeader::OptionalCdr1 { .. }) {
        present.extend(close_header(plan));
    }

    let matched = rust::Statement::Semi(rust::Expr::Match {
        scrutinee: Box::new(rust::Expr::Ref {
            target: Box::new(rust::Expr::Field {
                receiver: Box::new(receiver),
                name: Ident::new(&bound),
            }),
            mutable: false,
        }),
        arms: vec![
            rust::MatchArm {
                pattern: rust::Pattern::TupleStruct {
                    path: Path::single("Some"),
                    elements: vec![rust::Pattern::Binding(Ident::new(&bound))],
                },
                guard: None,
                body: rust::Expr::Block(rust::Block {
                    statements: present,
                }),
            },
            rust::MatchArm {
                pattern: rust::Pattern::Path(Path::single("None")),
                guard: None,
                body: rust::Expr::Block(rust::Block { statements: absent }),
            },
        ],
    });

    if matches!(plan.header, MemberHeader::OptionalCdr1 { .. }) {
        let mut all = open_header(plan, chosen);
        all.push(matched);
        all.extend(close_header(plan));
        return Ok(all);
    }
    Ok(vec![matched])
}

/// `fn key_holder_be(&self) -> Result<Vec<u8>, WireError>`,
/// `fn key_from_holder_be(r: &mut Reader) -> Result<Self, WireError>` and
/// `fn key_hash(&self) -> Result<[u8; 16], WireError>`, for a keyed type.
///
/// The two are one function apart — §7.6.8 steps 1 to 4 build the
/// holder, step 5 turns it into the hash — and they are emitted together
/// so that no caller can reach the hash through a holder written by
/// anything but this plan.
///
/// Empty for a type with no `@key` member: a KeyHash over nothing is
/// not a KeyHash, and emitting one would give every instance of that
/// type the same handle. Settled in favour of this position after
/// `wireprobe-zerodds --emit keyhash` was found answering a keyless type
/// with sixteen zero bytes, or with the MD5 of nothing when the type was
/// `@mutable`. Cyclone, RTI and Fast DDS all decide on "no keys" before
/// any hash is computed and produce none;
/// `crates/wire-probe/src/encode.rs::key_hash` carries the measurement
/// and now refuses as well.
///
/// The holder is always big-endian — DDSI-RTPS 2.5 §9.6.3.8 — and that
/// fact comes from `key_holder_plan_with`, not from a second reading of
/// the model here.
///
/// What the holder *contains* and in what order does take the dialect.
/// This column used to plan the holder under `LayoutKey::key`, whose
/// comment said "§7.6.8 leaves no room for a dialect"; three of the
/// holder's rules are measured divergences and the other three columns
/// have selected them for a while — `KeyHolderExtensibility` (RTI keeps
/// the type's own kind under XCDR1), `KeyHolderNesting` (RTI stops
/// filtering below a keyless aggregate under XCDR1) and
/// `KeyHolderOrder` (RTI under XCDR1 keeps declaration order).
/// A column that ignored all three answered `--dialect rti` with
/// Cyclone's instance handle.
fn key_holder_fns(
    ast: &Ast,
    type_: Id<TypeNode>,
    data_key: LayoutKey,
) -> Result<Vec<rust::Item>, EncodeError> {
    // The holder's own key, not the data key: big-endian and the key
    // purpose, both of which §7.6.8 fixes, over the dialect the data
    // plan was built with.
    let key = LayoutKey {
        purpose: zerodds_ast_layout::Purpose::Key,
        endianness: zerodds_ast_layout::Endianness::Big,
        ..data_key
    };
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        type_,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(|error| EncodeError::NoLayout {
        detail: alloc_format(&error),
    })?;
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let form =
        zerodds_ast_layout::keyhash::key_hash_form_for(ast, type_, key).map_err(|error| {
            EncodeError::NoLayout {
                detail: alloc_format(&error),
            }
        })?;

    let mut statements = vec![rust::Statement::Let {
        name: Ident::new("holder"),
        type_: None,
        // An associated function, not a method: the constructor takes
        // both the order and the alignment cap, and `Writer.with_…()` is
        // not Rust.
        //
        // The cap is four and not the `Writer`'s own default of eight.
        // XTypes 1.3 §7.6.8 step 4 serialises the holder as PLAIN_CDR2,
        // whose `MAXALIGN` §7.4.1 gives as four, whatever version the
        // sample itself goes out in. `Writer::big_endian()` stood here
        // and left the cap at eight, so a holder whose plan put a
        // `@key double` at offset four had it written at offset eight —
        // a KeyHash that disagrees with the plan that sized it, and with
        // every peer, for every keyed type with an eight-octet key
        // member behind a four-octet one. Nothing reported it: a
        // KeyHash is sixteen opaque octets and a wrong one only shows up
        // as an instance that never matches.
        //
        // The order is written as an absolute path rather than through
        // an import, because the generated file's import list is built
        // by each consumer of this emitter and a new name in it would
        // have to be added in each one.
        value: Some(rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["Writer", "with_max_align"]))),
            args: vec![
                rust::Expr::Path(Path::new(&["zerodds_wire", "Endianness", "Big"])),
                // PLAIN_CDR2's four, unless the dialect aligns the holder
                // to its data encoding or packs it (`KeyHolderAlignment`).
                rust::Expr::Literal(Literal::int(i128::from(
                    key.key_holder_alignment.max_alignment(key.encoding),
                ))),
            ],
        }),
        mutable: true,
    }];

    // A nested key member is written through the **bare** codec, not
    // through a versioned one: XTypes 1.3 §7.6.8 step 4 serialises the
    // holder as PLAIN_CDR2 whatever version the sample goes out in, so
    // there is no XCDR1 key holder for an XCDR1 codec to call.
    let codec = "";

    // Only the key members, in the order `key_members_with` gives —
    // which is the dialect's holder order, and the same order
    // `plan_aggregate` laid the holder out in.
    for node in &keys {
        let Some((name, kind)) = member_of(ast, type_, *node, key, codec) else {
            continue;
        };
        statements.push(write_value_to_sink(
            "holder",
            SinkShape::Owned,
            &name,
            &kind,
            rust::Expr::Field {
                receiver: Box::new(holder_of(ast, type_, *node)),
                name: Ident::new(&rust::escape_keyword(&name)),
            },
            // A key member is never `@optional` — §7.2.2.4.4.4.8, and
            // `idl-lower::validate::member_rules` refuses the pair — so
            // the field here is the member's own type and not an
            // `Option` of it.
            false,
        )?);
    }

    // The octets of the holder itself, before step 5 turns them into a
    // hash. A `DdsType` impl needs exactly these: the runtime's
    // instance path writes the holder into a `PlainCdr2BeKeyHolder` and
    // decides the branch of step 5 from the type's declared maximum, so
    // handing it a finished hash would be handing it the wrong thing.
    let mut holder_statements = statements.clone();
    holder_statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("holder"))),
            method: Ident::new("finish"),
            args: Vec::new(),
        }],
    }));
    let key_holder_be = rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("key_holder_be"),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: None,
            type_: rust::Type::unit(),
            receiver: Some(rust::Receiver::Ref),
        }],
        // Fallible for the same reason `key_hash` is.
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::generic(
                    Path::single("Vec"),
                    vec![rust::Type::Primitive(rust::Primitive::U8)],
                ),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block {
            statements: holder_statements,
        }),
    });

    // The inverse of `key_holder_be`: the same key members, read in the
    // same order the same plan wrote them.
    //
    // XTypes 1.3 §7.6.8 builds a holder that is not the type's own
    // stream — big-endian, `MAXALIGN` four, only the key members, no
    // DHEADER and no member headers — so `decode` cannot read one, and a
    // type that only had `encode_key_holder_be` could serialise an
    // instance's key and never read it back. DDS 1.4 §2.2.2.5.1.13 needs
    // exactly that read: on a dispose/unregister sample
    // `SampleInfo.valid_data` is `false` and "only the fields that form
    // the key" are meaningful, and those fields have to come from
    // somewhere.
    //
    // The members outside the holder are not in the stream at all and
    // take their §7.3.1.2.1.10 defaults through `Default::default()` —
    // the same values `default_impl` emits, so the two cannot drift.
    let mut from_holder = Vec::new();
    let mut key_fields = Vec::new();
    // A key member this emitter cannot describe is not skipped here. The
    // writer above skips it — a judged silent drop, allowlisted in
    // `tools/silent-drop-guard.sh` — but a *reader* that skipped one would
    // read the following member off the octets of the one it left out and
    // hand back a key that is wrong rather than absent. The whole reader
    // refuses instead, and says which member it could not read.
    let mut undescribed = None;
    for node in &keys {
        match member_of(ast, type_, *node, key, codec) {
            Some((name, kind)) => {
                let field = rust::escape_keyword(&name);
                from_holder.push(let_statement(
                    &field,
                    rust::Expr::Try(Box::new(read_for(&name, &kind)?)),
                ));
                key_fields.push((
                    *node,
                    (Ident::new(&field), rust::Expr::Path(Path::single(&field))),
                ));
            }
            None => undescribed = Some(*node),
        }
    }
    if let Some(node) = undescribed {
        from_holder = vec![rust::Statement::Tail(refused_err(
            "r",
            &format!("a key holder with a member this emitter cannot read ({node:?})"),
        ))];
    } else {
        from_holder.push(rust::Statement::Tail(rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![nested_struct_literal(ast, type_, key_fields, &|| {
                Some(Box::new(rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&["Default", "default"]))),
                    args: Vec::new(),
                }))
            })],
        }));
    }
    let key_from_holder_be = rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("key_from_holder_be"),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Reader"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block {
            statements: from_holder,
        }),
    });

    // A declared maximum of sixteen bytes or fewer: zero-pad the holder.
    // A larger or unbounded maximum: MD5. Variable length alone does not
    // select MD5; the shared maximum-size calculation decides.
    let finish = match form {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zero_padded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
    };
    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("holder"))),
            method: Ident::new(finish),
            args: Vec::new(),
        }],
    }));

    Ok(vec![
        key_holder_be,
        key_from_holder_be,
        rust::Item::Function(rust::Function {
            doc: Doc::none(),
            attrs: Vec::new(),
            visibility: Visibility::Public,
            name: Ident::new("key_hash"),
            generics: Vec::new(),
            params: vec![rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            }],
            // Fallible for the reason `encode` is: a `@key char` refuses
            // a code point above U+00FF, and a KeyHash that swallowed
            // the refusal would hash a character nobody wrote.
            returns: Some(rust::Type::generic(
                Path::single("Result"),
                vec![
                    rust::Type::Array {
                        element: Box::new(rust::Type::Primitive(rust::Primitive::U8)),
                        length: Box::new(rust::Expr::Literal(Literal::int(16))),
                    },
                    rust::Type::named(Path::single("WireError")),
                ],
            )),
            body: Some(rust::Block { statements }),
        }),
    ])
}

/// `fn decode(r: &mut Reader) -> Self`.
///
/// The mirror of the encoder, member for member and header for header,
/// built from the same plan — which is what makes the two agree by
/// construction rather than by review.
fn decode_fn(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    key: LayoutKey,
    codec: &'static str,
) -> Result<rust::Item, EncodeError> {
    let statements = if ast.effective_extensibility(type_) == zerodds_ast::Extensibility::Mutable {
        decode_by_member_id(ast, type_, layout, key, codec)?
    } else {
        decode_in_plan_order(ast, type_, layout, key, codec)?
    };
    // The read side of `encode_fns`'s block: an aggregate value's members
    // align from its own first octet under `ElementPadding::Packed`.
    let statements = wrap_packed_body(
        statements,
        "r",
        key.element_padding == ElementPadding::Packed,
    );

    Ok(rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&format!("decode{codec}")),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::named(Path::single("Reader"))),
                mutable: true,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::single("WireError")),
            ],
        )),
        body: Some(rust::Block { statements }),
    }))
}

/// A `@final` or `@appendable` aggregate: the members where the plan
/// says, because that is where the writer put them.
///
/// §7.4.1.2.3 makes only a `@mutable` list unordered. A positional
/// aggregate that read by id would be inventing a framing its own
/// encoder does not write.
fn decode_in_plan_order(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Statement>, EncodeError> {
    // A writer with fewer members than this reader stops early, and
    // §7.2.4.4.8, Table 19, says "each non-optional member in a T1
    // object that is not present in the T2 object takes its default
    // value". Only an `@appendable` type may meet one: Table 19 requires
    // two final types to have the same set of member IDs.
    let appendable = ast.effective_extensibility(type_) == zerodds_ast::Extensibility::Appendable;
    let mut statements = Vec::new();
    if layout.needs_dheader {
        // The DHEADER is what says where the writer's members stopped —
        // "skipping the members an older reader does not know is what
        // the DHEADER is for". `end` is that offset; a type that cannot
        // meet a shorter writer still reads it, to move the cursor.
        if appendable {
            statements.push(let_statement(
                "end",
                rust::Expr::Binary {
                    op: rust::BinaryOp::Add,
                    lhs: Box::new(rust::Expr::Try(Box::new(call("read_dheader", Vec::new())))),
                    rhs: Box::new(call("position", Vec::new())),
                },
            ));
        } else {
            statements.push(let_statement(
                "_dheader",
                rust::Expr::Try(Box::new(call("read_dheader", Vec::new()))),
            ));
        }
    }

    let mut fields = Vec::new();
    for plan in &layout.members {
        let Some((name, kind)) = member_of(ast, type_, plan.member, key, codec) else {
            continue;
        };
        // The appendable tail. Under encoding version 2 the delimiter
        // says where the writer stopped; under version 1 §7.2.4.2 leaves
        // an appendable type undelimited and the payload's end is what
        // all three vendors use instead — Cyclone, RTI and Fast DDS each
        // default the tail of a short XCDR1 stream, measured 2026-08-09
        // as `evo-appendable-tail`.
        if appendable && matches!(plan.header, MemberHeader::NoHeader) {
            let Some(member) = member_node_of(ast, type_, plan.member) else {
                continue;
            };
            let Some(member_type) = member.type_.resolved() else {
                continue;
            };
            let reached = if layout.needs_dheader {
                rust::Expr::Binary {
                    op: rust::BinaryOp::Ge,
                    lhs: Box::new(call("position", Vec::new())),
                    rhs: Box::new(rust::Expr::Path(Path::single("end"))),
                }
            } else {
                rust::Expr::Binary {
                    op: rust::BinaryOp::Eq,
                    lhs: Box::new(call("remaining", Vec::new())),
                    rhs: Box::new(rust::Expr::Literal(Literal::int(0))),
                }
            };
            statements.push(let_statement(
                &name,
                rust::Expr::If {
                    condition: Box::new(reached),
                    then: rust::Block {
                        statements: vec![rust::Statement::Tail(
                            crate::default_rust::member_default(
                                ast,
                                module_of(ast, type_),
                                member,
                                member_type,
                            )?,
                        )],
                    },
                    otherwise: Some(rust::Block {
                        statements: vec![rust::Statement::Tail(rust::Expr::Try(Box::new(
                            read_for(&name, &kind)?,
                        )))],
                    }),
                },
            ));
            fields.push((plan.member, field_binding(&name)));
            continue;
        }
        // An `@optional` member reads into an `Option<T>`, so the marker
        // the specification puts on the wire decides between `Some` and
        // `None` instead of between a value and a refusal.
        if member_node_of(ast, type_, plan.member).is_some_and(|member| member.facets.is_optional())
        {
            statements.extend(read_optional_member(plan, &name, &kind)?);
            fields.push((plan.member, field_binding(&name)));
            continue;
        }
        match plan.header {
            MemberHeader::NoHeader | MemberHeader::OptionalFlag => {}
            MemberHeader::PidCdr1 { .. } | MemberHeader::OptionalCdr1 { .. } => {
                statements.push(let_statement(
                    "pid",
                    rust::Expr::Try(Box::new(call("read_pid", Vec::new()))),
                ));
            }
            MemberHeader::EmHeader { .. } => {
                statements.push(let_statement(
                    "emheader",
                    rust::Expr::Try(Box::new(call("read_emheader", Vec::new()))),
                ));
            }
        }
        // Every read is fallible, so every read is followed by `?`. A
        // decoder that could not fail would have to guess at bytes that
        // are not there, and a guess is a value nobody sent.
        statements.push(let_statement(
            &name,
            rust::Expr::Try(Box::new(read_for(&name, &kind)?)),
        ));
        // The mirror of the encoder's patch step. `Reader::read_pid`
        // makes the parameter's value its own alignment origin — §7.4.1.2
        // calls each parameter "a CDR-encapsulated block of data" — and
        // `finish_pid` is the only thing that puts the origin back and
        // steps over the padding to the next four-byte boundary.
        //
        // Without it the origin stayed at the last parameter's value
        // start for the rest of the aggregate. A parameter whose value
        // begins at an offset that is 4 modulo 8 then aligns an
        // eight-byte member after it four bytes late:
        // `@final struct S { @optional octet b; double c; };` writes `c`
        // at payload offset 8 and read it at 12. It also skipped
        // `check_declared_length`, so a peer's length that disagreed
        // with the value that followed it was accepted in silence.
        match plan.header {
            MemberHeader::NoHeader | MemberHeader::OptionalFlag | MemberHeader::EmHeader { .. } => {
            }
            MemberHeader::PidCdr1 { .. } | MemberHeader::OptionalCdr1 { .. } => {
                statements.push(rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                    "finish_pid",
                    vec![rust::Expr::Path(Path::single("pid"))],
                )))));
            }
        }
        fields.push((plan.member, field_binding(&name)));
    }

    // Rule (23): a PL_CDR1 parameter list ends at `PID_SENTINEL`, not at
    // the last member the reader recognises. The encoder writes one and
    // the decoder left it in the stream, which puts every byte after the
    // aggregate four positions early for any caller that reads on.
    if layout.needs_sentinel {
        statements.extend(read_sentinel());
    }

    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![nested_struct_literal(ast, type_, fields, &|| None)],
    }));

    Ok(statements)
}

/// A `@mutable` aggregate: the members its headers name, in the order
/// the stream names them.
///
/// XTypes 1.3 §7.4.1.2.3 — "any mutable structure member's value may be
/// omitted" — and §7.4.3.4.2, which gives every member an EMHEADER
/// carrying its id. A `@mutable` list is unordered by construction and
/// the ids are what orders it; this column read it in *plan* order
/// instead, so a member the stream omitted took the next member's bytes
/// and a member the peer reordered was read as the wrong member. Both
/// produce a sample and neither reports anything. The reflective, C and
/// Python columns have read by id since the evolution run.
///
/// Two loops, one per encoding version, because the two have different
/// ends: rule (21) bounds the list with the DHEADER's extent and rule
/// (23) with `PID_SENTINEL`.
///
/// A member the stream did not carry takes its default afterwards —
/// §7.4.1.2.3, "if the receiver's member is not optional, it logically
/// takes its default value" — and an `@optional` one takes the absence,
/// which is what its slot already holds.
fn decode_by_member_id(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &TypeLayout,
    key: LayoutKey,
    codec: &'static str,
) -> Result<Vec<rust::Statement>, EncodeError> {
    let mut slots = Vec::new();
    for plan in &layout.members {
        let Some((name, kind)) = member_of(ast, type_, plan.member, key, codec) else {
            continue;
        };
        slots.push(MutableSlot {
            name,
            kind,
            member_id: plan.member_id,
        });
    }

    let mut statements = Vec::new();
    // Rule (21): under encoding version 2 the DHEADER is what says where
    // the list stops. Under version 1 rule (23) uses `PID_SENTINEL`
    // instead and there is no delimiter to read.
    if layout.needs_dheader {
        statements.push(let_statement(
            "end",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(rust::Expr::Try(Box::new(call("read_dheader", Vec::new())))),
                rhs: Box::new(call("position", Vec::new())),
            },
        ));
    }
    // One slot per member, empty until a header names it. `Option` is
    // also the "seen" flag: a second header for the same id is a stream
    // that says two things about one member, and §7.4.3.4.2 gives each
    // one header.
    for slot in &slots {
        statements.push(rust::Statement::Let {
            name: Ident::new(&rust::escape_keyword(&slot.name)),
            type_: None,
            value: Some(rust::Expr::Path(Path::single("None"))),
            mutable: true,
        });
    }

    if key.encoding.is_v1() {
        statements.extend(parameter_list_loop(&slots)?);
    } else {
        statements.extend(member_header_loop(&slots)?);
    }

    // The members no header named. Written after the loop and not
    // inside it, because "did not arrive" is only knowable once the
    // list has ended.
    let mut fields = Vec::new();
    for plan in &layout.members {
        let Some((name, _)) = member_of(ast, type_, plan.member, key, codec) else {
            continue;
        };
        let bound = rust::escape_keyword(&name);
        let optional = member_node_of(ast, type_, plan.member)
            .is_some_and(|member| member.facets.is_optional());
        if !optional {
            let Some(member) = member_node_of(ast, type_, plan.member) else {
                continue;
            };
            let Some(member_type) = member.type_.resolved() else {
                continue;
            };
            statements.push(let_statement(
                &name,
                or_default(
                    &bound,
                    crate::default_rust::member_default(
                        ast,
                        module_of(ast, type_),
                        member,
                        member_type,
                    )?,
                ),
            ));
        }
        fields.push((
            plan.member,
            (Ident::new(&bound), rust::Expr::Path(Path::single(&bound))),
        ));
    }

    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![nested_struct_literal(ast, type_, fields, &|| None)],
    }));
    Ok(statements)
}

/// The value of a member no header named: `bound` if one did, its
/// default if not.
///
/// Spelled the way clippy asks for, so a crate including the generated
/// text needs no allowance: `unwrap_or_default()` where Table 9's value
/// is also Rust's `Default` (a zero, `false`, `'\0'`, an empty `String`,
/// `Vec` or `BTreeMap`), `unwrap_or(value)` where the value is a constant
/// expression, and `unwrap_or_else(|| value)` where building it calls
/// something. A `match` with a `None` arm was all three, and
/// `manual_unwrap_or{,_default}` fired once per member.
fn or_default(bound: &str, value: rust::Expr) -> rust::Expr {
    let receiver = Box::new(rust::Expr::Path(Path::single(bound)));
    if is_rust_default(&value) {
        return rust::Expr::MethodCall {
            receiver,
            method: Ident::new("unwrap_or_default"),
            args: Vec::new(),
        };
    }
    if is_constant(&value) {
        return rust::Expr::MethodCall {
            receiver,
            method: Ident::new("unwrap_or"),
            args: vec![value],
        };
    }
    rust::Expr::MethodCall {
        receiver,
        method: Ident::new("unwrap_or_else"),
        args: vec![rust::Expr::Closure {
            params: Vec::new(),
            body: Box::new(value),
        }],
    }
}

/// Whether `value` is what `Default::default()` gives for its type.
fn is_rust_default(value: &rust::Expr) -> bool {
    match value {
        rust::Expr::Literal(Literal::Int(text)) => text == "0",
        rust::Expr::Literal(Literal::Float(text)) => {
            matches!(text.as_str(), "0.0" | "0.0f32" | "0.0f64")
        }
        rust::Expr::Literal(Literal::Bool(flag)) => !flag,
        rust::Expr::Literal(Literal::Char(ch)) => *ch == '\0',
        rust::Expr::Call { callee, args } if args.is_empty() => {
            let rust::Expr::Path(path) = callee.as_ref() else {
                return false;
            };
            let names: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
            matches!(
                names.as_slice(),
                ["String", "new"] | ["Vec", "new"] | ["BTreeMap", "new"]
            )
        }
        _ => false,
    }
}

/// Whether `value` is built without calling anything: literals, paths,
/// and aggregates of them.
///
/// zerodds-lint: recursion-depth = the default value's nesting, one level
/// per composite literal; `default_rust` bounds it at 64.
fn is_constant(value: &rust::Expr) -> bool {
    match value {
        rust::Expr::Literal(_) | rust::Expr::Path(_) => true,
        rust::Expr::Array(elements) | rust::Expr::Tuple(elements) => {
            elements.iter().all(is_constant)
        }
        rust::Expr::Repeat { value, .. } => is_constant(value),
        rust::Expr::StructLit { fields, base, .. } => {
            base.is_none() && fields.iter().all(|(_, field)| is_constant(field))
        }
        rust::Expr::Cast { value, .. } => is_constant(value),
        _ => false,
    }
}

/// `return Err(WireError::refused(r.position(), "…"));`
fn refuse_statement(why: &str) -> rust::Statement {
    rust::Statement::Semi(rust::Expr::Return(Some(Box::new(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Err"))),
        args: vec![rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["WireError", "refused"]))),
            args: vec![
                call("position", Vec::new()),
                rust::Expr::Literal(Literal::string(why)),
            ],
        }],
    }))))
}

/// `if <slot>.is_some() { return Err(…); }`
fn refuse_a_second_header(slot: &str, member_id: u32) -> rust::Statement {
    rust::Statement::Semi(rust::Expr::If {
        condition: Box::new(rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single(slot))),
            method: Ident::new("is_some"),
            args: Vec::new(),
        }),
        then: rust::Block {
            statements: vec![refuse_statement(&format!(
                "one header per member, and member id {member_id} has two"
            ))],
        },
        otherwise: None,
    })
}

/// Rule (21): EMHEADERs until the delimiter's extent is used up.
fn member_header_loop(slots: &[MutableSlot]) -> Result<Vec<rust::Statement>, EncodeError> {
    let mut body = vec![
        let_statement(
            "header",
            rust::Expr::Try(Box::new(call("read_emheader", Vec::new()))),
        ),
        // Where this member ends, whatever it turns out to be. Read
        // before the value, because after it the length code's own
        // rounding is no longer recoverable.
        let_statement(
            "ends",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(call("position", Vec::new())),
                rhs: Box::new(rust::Expr::Field {
                    receiver: Box::new(rust::Expr::Path(Path::single("header"))),
                    name: Ident::new("extent"),
                }),
            },
        ),
    ];

    // §7.2.2.4.4.4.6: a member this type does not declare is skipped
    // when the writer said it may be, and discards the whole sample
    // when it said it may not.
    let mut chain = Some(rust::Block {
        statements: vec![rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Field {
                receiver: Box::new(rust::Expr::Path(Path::single("header"))),
                name: Ident::new("must_understand"),
            }),
            then: rust::Block {
                statements: vec![refuse_statement(
                    "a must-understand member this type does not declare, per §7.2.2.4.4.4.6",
                )],
            },
            otherwise: Some(rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                    "skip_to",
                    vec![rust::Expr::Path(Path::single("ends"))],
                ))))],
            }),
        })],
    });

    for slot in slots.iter().rev() {
        let bound = rust::escape_keyword(&slot.name);
        let mut arm = vec![refuse_a_second_header(&bound, slot.member_id)];
        arm.push(rust::Statement::Semi(rust::Expr::Assign {
            target: Box::new(rust::Expr::Path(Path::single(&bound))),
            value: Box::new(rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Some"))),
                args: vec![rust::Expr::Try(Box::new(read_for(&slot.name, &slot.kind)?))],
            }),
        }));
        // The header said how long the member is; a value that did not
        // fill it means the two disagree, and reading on would take the
        // next header out of this member's bytes.
        arm.push(rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Ne,
                lhs: Box::new(call("position", Vec::new())),
                rhs: Box::new(rust::Expr::Path(Path::single("ends"))),
            }),
            then: rust::Block {
                statements: vec![refuse_statement(
                    "a member filling the extent its EMHEADER declared",
                )],
            },
            otherwise: None,
        }));
        chain = Some(rust::Block {
            statements: vec![rust::Statement::Semi(rust::Expr::If {
                condition: Box::new(rust::Expr::Binary {
                    op: rust::BinaryOp::Eq,
                    lhs: Box::new(rust::Expr::Field {
                        receiver: Box::new(rust::Expr::Path(Path::single("header"))),
                        name: Ident::new("member_id"),
                    }),
                    rhs: Box::new(rust::Expr::Literal(Literal::int(i128::from(
                        slot.member_id,
                    )))),
                }),
                then: rust::Block { statements: arm },
                otherwise: chain,
            })],
        });
    }
    if let Some(chain) = chain {
        body.extend(chain.statements);
    }

    Ok(vec![rust::Statement::Semi(rust::Expr::While {
        condition: Box::new(rust::Expr::Binary {
            op: rust::BinaryOp::Lt,
            lhs: Box::new(call("position", Vec::new())),
            rhs: Box::new(rust::Expr::Path(Path::single("end"))),
        }),
        body: rust::Block { statements: body },
    })])
}

/// Rule (23): parameters until `PID_SENTINEL`.
fn parameter_list_loop(slots: &[MutableSlot]) -> Result<Vec<rust::Statement>, EncodeError> {
    // A sentinel is the only thing that ends the list, so the loop
    // condition is a flag the sentinel sets rather than a count.
    let mut statements = vec![rust::Statement::Let {
        name: Ident::new("ended"),
        type_: None,
        value: Some(rust::Expr::Literal(Literal::Bool(false))),
        mutable: true,
    }];

    // Stepping over a parameter this type has no member for, in the
    // one place three different reasons reach it: an id the model does
    // not declare, a vendor-private parameter and one inside
    // §7.4.1.2.2's reserved range.
    let skip_parameter_block = rust::Block {
        statements: vec![rust::Statement::Semi(rust::Expr::Try(Box::new(call(
            "skip_parameter",
            vec![rust::Expr::Path(Path::single("pid"))],
        ))))],
    };
    let unknown = rust::Block {
        statements: vec![rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Field {
                receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                name: Ident::new("must_understand"),
            }),
            then: rust::Block {
                statements: vec![refuse_statement(
                    "a must-understand parameter this type does not declare, per §7.2.2.4.4.4.6",
                )],
            },
            otherwise: Some(skip_parameter_block.clone()),
        })],
    };
    let mut chain = Some(unknown.clone());

    for slot in slots.iter().rev() {
        let bound = rust::escape_keyword(&slot.name);
        let mut arm = vec![refuse_a_second_header(&bound, slot.member_id)];
        arm.push(rust::Statement::Semi(rust::Expr::Assign {
            target: Box::new(rust::Expr::Path(Path::single(&bound))),
            value: Box::new(rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Some"))),
                args: vec![rust::Expr::Try(Box::new(read_for(&slot.name, &slot.kind)?))],
            }),
        }));
        arm.push(rust::Statement::Semi(rust::Expr::Try(Box::new(call(
            "finish_pid",
            vec![rust::Expr::Path(Path::single("pid"))],
        )))));
        chain = Some(rust::Block {
            statements: vec![rust::Statement::Semi(rust::Expr::If {
                condition: Box::new(rust::Expr::Binary {
                    op: rust::BinaryOp::Eq,
                    lhs: Box::new(rust::Expr::Field {
                        receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                        name: Ident::new("pid"),
                    }),
                    rhs: Box::new(rust::Expr::Literal(Literal::int(i128::from(
                        slot.member_id,
                    )))),
                }),
                then: rust::Block { statements: arm },
                otherwise: chain,
            })],
        });
    }

    // Three gates in front of the id chain, in the order the clauses
    // rank them.
    //
    // `PID_IGNORE` first: §7.4.1.2.2 says "All consumers of this Data
    // Representation shall ignore parameters with this ID" and states no
    // exception, so it outranks the must-understand bit that `unknown`
    // would refuse on.
    //
    // Then `is_user_member`, which is false for a parameter carrying
    // `FLAG_IMPL_EXTENSION` and for one inside the reserved range. Both
    // are legal for a peer to send and neither is a member of this type,
    // so both take the unknown path instead of being matched by their
    // low bits against a member that happens to share them.
    let matched = rust::Block {
        statements: chain.map_or_else(Vec::new, |chain| chain.statements),
    };
    let tail = rust::Block {
        statements: vec![rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Field {
                receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                name: Ident::new("ignore"),
            }),
            then: skip_parameter_block,
            otherwise: Some(rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::If {
                    condition: Box::new(rust::Expr::MethodCall {
                        receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                        method: Ident::new("is_user_member"),
                        args: Vec::new(),
                    }),
                    then: matched,
                    otherwise: Some(unknown),
                })],
            }),
        })],
    };
    let body = vec![
        let_statement(
            "pid",
            rust::Expr::Try(Box::new(call("read_pid", Vec::new()))),
        ),
        rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Field {
                receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                name: Ident::new("end"),
            }),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Assign {
                    target: Box::new(rust::Expr::Path(Path::single("ended"))),
                    value: Box::new(rust::Expr::Literal(Literal::Bool(true))),
                })],
            },
            otherwise: Some(tail),
        }),
    ];

    statements.push(rust::Statement::Semi(rust::Expr::While {
        condition: Box::new(rust::Expr::Not(Box::new(rust::Expr::Path(Path::single(
            "ended",
        ))))),
        body: rust::Block { statements: body },
    }));
    Ok(statements)
}

/// What the two `@mutable` loops need of a member: the id its header
/// carries, the name its slot takes, and how to read it.
struct MutableSlot {
    /// The member's name, as the source wrote it.
    name: String,
    /// What reading it takes.
    kind: MemberWrite,
    /// The id its header carries.
    member_id: u32,
}

/// The write call one member takes.
///
/// One arm per primitive rather than a formatted name: the writer for a
/// `u32` and the writer for an `i32` are different functions, and a
/// name assembled from a string would hide a wrong one until the bytes
/// came out different.
fn write_for(
    receiver: rust::Expr,
    name: &str,
    kind: &MemberWrite,
) -> Result<rust::Statement, EncodeError> {
    let value = rust::Expr::Field {
        receiver: Box::new(receiver),
        name: Ident::new(&rust::escape_keyword(name)),
    };
    write_value_to("w", name, kind, value, false)
}

/// The same, naming the writer it goes to.
///
/// The KeyHash builds its own holder rather than writing to the
/// sample's writer: the holder is big-endian regardless of what the
/// sample is, and sharing one writer would make the hash depend on the
/// sample's byte order.
///
/// `borrowed` says the expression is already a `&T` — a binding out of
/// `match &self.<name> { Some(v) => … }`, or a closure's element —
/// rather than the field itself. It decides one thing per shape and no
/// more: a primitive is dereferenced, a string is not re-borrowed.
/// Passing the wrong one does not compile, which is the point of
/// carrying it rather than guessing from the shape of the expression.
///
/// `pub(crate)`: `emit::interface_rust` hand-assembles the DDS-RPC 1.0
/// §7.5.1.1 topic types the same way this file assembles an aggregate's
/// codec, member by member, and calls this rather than re-deriving how
/// one value goes onto a `Writer`.
pub(crate) fn write_value_to(
    sink: &str,
    name: &str,
    kind: &MemberWrite,
    value: rust::Expr,
    borrowed: bool,
) -> Result<rust::Statement, EncodeError> {
    write_value_to_sink(sink, SinkShape::Reference, name, kind, value, borrowed)
}

/// Whether the name the writes go to is already a `&mut Writer`.
///
/// Every arm but one calls a method on the sink, and a method call
/// auto-references. `MemberWrite::Nested` hands it over as an *argument* —
/// `self.k.encode(sink)` — and there Rust does not: a local `Writer` where a
/// `&mut Writer` is wanted is `error[E0308]`, in generated code, which is the
/// worst place for it. The key-holder path is the one with a local, so it says
/// so instead of the emitter guessing from the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SinkShape {
    /// A `&mut Writer` parameter — pass it along as it is.
    Reference,
    /// A local `Writer` binding — take a reference where one is needed.
    Owned,
}

fn write_value_to_sink(
    sink: &str,
    shape: SinkShape,
    name: &str,
    kind: &MemberWrite,
    value: rust::Expr,
    borrowed: bool,
) -> Result<rust::Statement, EncodeError> {
    let (call, fallible) = write_call(sink, shape, name, kind, value, borrowed)?;
    // A write that can refuse is followed by `?`. Dropping the `Result`
    // would compile — it is only a warning — and would put the narrowed
    // character on the wire, which is the defect the runtime stopped
    // committing.
    Ok(rust::Statement::Semi(if fallible {
        rust::Expr::Try(Box::new(call))
    } else {
        call
    }))
}

/// The call one value's write is, and whether it returns a `Result`.
///
/// One arm per shape, and the shape decides three things at once: which
/// runtime function, how the value is handed over, and whether the
/// result has to be looked at.
///
/// zerodds-lint: recursion-depth 64 (collection nesting of the owned
/// `MemberWrite` tree, via `element_writer`). That tree is built by
/// `write_of_at`, whose `MAX_WRITE_DEPTH` bounds its height, and it
/// is owned, so it holds no cycle of its own.
fn write_call(
    sink: &str,
    shape: SinkShape,
    name: &str,
    kind: &MemberWrite,
    value: rust::Expr,
    borrowed: bool,
) -> Result<(rust::Expr, bool), EncodeError> {
    let sink_expr = || rust::Expr::Path(Path::single(sink));
    let deref = |value: rust::Expr| {
        if borrowed {
            rust::Expr::Deref(Box::new(value))
        } else {
            value
        }
    };
    // A `String` field goes in by reference and a `&String` binding is
    // one already; both coerce to the `&str` the runtime takes. A
    // collection is the same question one type up — `&Vec<T>` and
    // `&[T; N]` both coerce to the `&[T]` the runtime takes.
    let by_ref = |value: rust::Expr| {
        if borrowed {
            value
        } else {
            rust::Expr::Ref {
                target: Box::new(value),
                mutable: false,
            }
        }
    };
    Ok(match kind {
        MemberWrite::External(inner) => {
            return write_call(
                sink,
                shape,
                name,
                inner,
                rust::Expr::MethodCall {
                    receiver: Box::new(value),
                    method: Ident::new("as_ref"),
                    args: Vec::new(),
                },
                true,
            );
        }
        MemberWrite::Primitive(primitive) => (
            rust::Expr::MethodCall {
                receiver: Box::new(sink_expr()),
                method: Ident::new(writer_name(*primitive)),
                args: vec![deref(value)],
            },
            // `char` is one byte and `wchar` two, and a code point that
            // fits neither is refused rather than narrowed —
            // §7.2.2.2.1.2.2 and §7.2.2.2.1.2.5.
            matches!(primitive, PrimitiveKind::Char | PrimitiveKind::WChar),
        ),
        MemberWrite::String { bound, .. } => {
            // A bounded string writes the same bytes as an unbounded one
            // of the same length; the bound is a promise about the
            // source, not a difference on the wire. The runtime takes it
            // as a second argument and not as part of the name — and
            // refuses a value that overruns it, which is why the bounded
            // form is fallible and the plain one is not.
            let (method, mut args, fallible) = match bound {
                Some(bound) => (
                    "write_string_bounded",
                    vec![rust::Expr::Literal(Literal::int(*bound))],
                    true,
                ),
                None => ("write_string", Vec::new(), false),
            };
            let mut all = vec![by_ref(value)];
            all.append(&mut args);
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(sink_expr()),
                    method: Ident::new(method),
                    args: all,
                },
                fallible,
            )
        }
        MemberWrite::Fixed {
            digits,
            scale,
            count,
        } => (
            rust::Expr::MethodCall {
                receiver: Box::new(sink_expr()),
                method: Ident::new("write_fixed"),
                args: vec![
                    by_ref(value),
                    rust::Expr::Literal(Literal::int(i128::from(*digits))),
                    rust::Expr::Literal(Literal::int(i128::from(*scale))),
                    fixed_digits_expr(*count),
                ],
            },
            // Always fallible: a value the declaration cannot carry is
            // refused rather than truncated, and a truncated decimal is the
            // wrong number with nothing to notice it by.
            true,
        ),
        MemberWrite::WString {
            bound,
            policy,
            units,
            order,
            position,
            ..
        } => {
            // Every write here is fallible, bounded or not: the
            // `CHARACTER_COUNT` cell refuses a code point outside the
            // BMP rather than truncating it to sixteen bits the way
            // Fast CDR does, so even an unbounded `wstring` can say no.
            //
            // `WstringCodeUnits::Utf8` names the runtime's UTF-8 pair,
            // which takes no order: UTF-8 octets have no byte order of
            // their own, and C1 and C3 are vacuous because there is no
            // fixed-width code unit for either to speak about.
            let (method, mut args) = match (units, bound) {
                (WstringCodeUnits::Utf8, Some(bound)) => (
                    "write_wstring_utf8_bounded",
                    vec![rust::Expr::Literal(Literal::int(*bound))],
                ),
                (WstringCodeUnits::Utf8, None) => ("write_wstring_utf8", Vec::new()),
                (WstringCodeUnits::Utf16, Some(bound)) => (
                    "write_wstring_bounded",
                    vec![rust::Expr::Literal(Literal::int(*bound))],
                ),
                (WstringCodeUnits::Utf16, None) => ("write_wstring", Vec::new()),
            };
            let mut all = vec![by_ref(value)];
            all.append(&mut args);
            all.push(wstring_policy_expr(*policy));
            if *units == WstringCodeUnits::Utf16 {
                all.push(wstring_order_expr(*order));
            }
            all.push(wire_position_expr(name, *position)?);
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(sink_expr()),
                    method: Ident::new(method),
                    args: all,
                },
                true,
            )
        }
        MemberWrite::Enumeration { bits } => (
            rust::Expr::MethodCall {
                receiver: Box::new(sink_expr()),
                method: Ident::new(enum_writer(*bits)),
                args: vec![deref(value)],
            },
            false,
        ),
        // The bound goes into the call and the holder is widened to the
        // `u64` the runtime takes. Fallible: a value with a bit set
        // above the bound is refused rather than masked, which is what
        // `runtime/c/src/writer.c` does with `ZDW_RANGE`.
        MemberWrite::Bitmask { bits, .. } => (
            rust::Expr::MethodCall {
                receiver: Box::new(sink_expr()),
                method: Ident::new("write_bitmask"),
                args: vec![
                    rust::Expr::Literal(Literal::int(i128::from(*bits))),
                    widen_to_u64(deref(value), *bits),
                ],
            },
            true,
        ),
        // An aggregate encodes itself. There is no `Writer::write_nested`
        // and there was never going to be one: the nested type's own
        // `encode` is what carries its plan, and a runtime function
        // could not have known it.
        //
        // Under a per-sample plan the call names the nested type's
        // **body** and hands it the sample's decision: the same nested
        // type is framed short in a sample below RTI's step and extended
        // in one past it, so a call through the public entry would frame
        // it by the nested type's own size instead of the sample's.
        MemberWrite::Nested {
            codec, extended, ..
        } => {
            let mut args = vec![match shape {
                SinkShape::Reference => sink_expr(),
                SinkShape::Owned => rust::Expr::Ref {
                    target: Box::new(sink_expr()),
                    mutable: true,
                },
            }];
            args.extend(extended.argument());
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(value),
                    method: Ident::new(&nested_method(codec, *extended)),
                    // The one place the sink is an argument and not a
                    // receiver, so the one place auto-referencing does
                    // not happen for us.
                    args,
                },
                true,
            )
        }
        MemberWrite::Sequence {
            element,
            bound,
            delimited,
            packed,
            carries_length,
            ..
        } => {
            // `sequence<T, N>`: the count is checked before a byte is
            // written. Table 17 makes `O2.length <= T1.length` the
            // condition for constructing one, the wire carries no bound
            // to compare against, and a sender that wrote more elements
            // than its own declaration allows has written a value of a
            // type nobody declared. G16, on the write side.
            let method = match (bound, delimited) {
                (Some(_), true) => "write_delimited_sequence_bounded",
                (Some(_), false) => "write_sequence_bounded",
                (None, true) => "write_delimited_sequence",
                (None, false) => "write_sequence",
            };
            let mut args = vec![by_ref(value)];
            if let Some(bound) = bound {
                args.push(rust::Expr::Literal(Literal::int(*bound)));
            }
            args.push(sequence_element_writer(
                name,
                element,
                *packed,
                *carries_length,
            )?);
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(sink_expr()),
                    method: Ident::new(method),
                    args,
                },
                true,
            )
        }
        MemberWrite::Map {
            key,
            value: entry_value,
            bound,
            delimited,
            ascending,
            ..
        } => {
            // The count on the wire is the **entry** count and not the
            // number of halves — §7.4.1.1.4 defines a `map<K, V>` by
            // equivalence to a `sequence` whose element is the pair —
            // and the runtime writes it from `BTreeMap::len`, so there
            // is nothing to count here. `map<K, V, N>` bounds that same
            // number, which is why the bounded form passes it through
            // unchanged from the sequence's shape.
            let method = match (bound, delimited) {
                (Some(_), true) => "write_delimited_map_bounded",
                (Some(_), false) => "write_map_bounded",
                (None, true) => "write_delimited_map",
                (None, false) => "write_map",
            };
            // Held as its entries, the map goes out in the order it is held; ascending by key
            // is a view over them, and a `BTreeMap` is ascending already.
            let held = match crate::adapter::cst::rust_map_form() {
                crate::adapter::cst::RustMapForm::Entries if *ascending => rust::Expr::Ref {
                    target: Box::new(rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "zerodds_wire",
                            "AscendingKey",
                        ]))),
                        args: vec![by_ref(value)],
                    }),
                    mutable: false,
                },
                _ => by_ref(value),
            };
            let mut args = vec![held];
            if let Some(bound) = bound {
                args.push(rust::Expr::Literal(Literal::int(*bound)));
            }
            args.push(element_writer(name, key)?);
            args.push(element_writer(name, entry_value)?);
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(sink_expr()),
                    method: Ident::new(method),
                    args,
                },
                true,
            )
        }
        MemberWrite::Array {
            element,
            dims,
            delimited,
            packed,
        } => {
            // No length: an array's element count is in the type, not on
            // the wire. XTypes 1.3 §7.4.3.5. A multi-dimensional array is
            // one flat run (G4), so the delimiter — where rule (9) asks
            // for one — goes round the whole thing and the inner
            // dimensions are plain.
            (
                rust::Expr::MethodCall {
                    receiver: Box::new(sink_expr()),
                    method: Ident::new(if *delimited {
                        "write_delimited_array"
                    } else {
                        "write_array"
                    }),
                    args: vec![
                        by_ref(value),
                        array_element_writer(name, element, dims.len().saturating_sub(1), *packed)?,
                    ],
                },
                true,
            )
        }
        MemberWrite::Unsupported(kind) => {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind,
            });
        }
    })
}

/// The digit-count axis as the generated code names it.
///
/// A path and not a boolean: the emitted call says `FixedDigits::Declared`
/// or `FixedDigits::ValuePrecision`, so a reader of the generated file sees
/// which reading of CORBA 3.3 part 2 §9.3.2.8 it writes without going back
/// to the plan that chose it.
fn fixed_digits_expr(count: FixedDigitCount) -> rust::Expr {
    rust::Expr::Path(Path::new(&[
        "zerodds_wire",
        "FixedDigits",
        match count {
            FixedDigitCount::Declared => "Declared",
            FixedDigitCount::ValuePrecision => "ValuePrecision",
            FixedDigitCount::ValueStringDigits => "ValueStringDigits",
        },
    ]))
}

/// `|w, v| <the element's write>`, always returning a `Result`.
///
/// A closure and not a named path. `Writer::write_i32` takes an `i32` by
/// value where an element function takes a `&i32`; a nested aggregate's
/// write is a method on the *value*; and a collection's write takes its
/// own element function as a second argument, which no path can supply.
/// The runtime's element functions were made `Fn`-generic and the CST
/// gained `Expr::Closure` for exactly this, and then nothing emitted
/// one — `element_writer_name` kept naming paths and refused the two
/// shapes that have none. That refusal is `AL-9`, and this is it closed.
///
/// zerodds-lint: recursion-depth 64 (collection nesting of the owned
/// `MemberWrite` tree, via `write_call`). That tree is built by
/// `write_of_at`, whose `MAX_WRITE_DEPTH` bounds its height, and it
/// is owned, so it holds no cycle of its own.
fn element_writer(at: &str, element: &MemberWrite) -> Result<rust::Expr, EncodeError> {
    // A `wstring` element is written at the element position, which is
    // substituted here for the same reason the `wchar` element's
    // framing is: `write_call` was handed a member's shape, and a
    // caller that could leave the position alone would take the
    // member's answer for an element by not asking.
    let substituted = as_element(element);
    let element = substituted.as_ref().unwrap_or(element);
    // A `wchar` element takes the element framing, which under TAO's
    // dialect is not the member's. `write_call` names the member's
    // method, so the element's is substituted here rather than passed
    // down through five shapes that do not care.
    if matches!(element, MemberWrite::Primitive(PrimitiveKind::WChar)) {
        return Ok(rust::Expr::Path(Path::new(&[
            "Writer",
            "write_wchar_element",
        ])));
    }
    let (call, fallible) = write_call(
        "w",
        SinkShape::Reference,
        at,
        element,
        rust::Expr::Path(Path::single("v")),
        true,
    )?;
    Ok(rust::Expr::Closure {
        params: vec![Ident::new("w"), Ident::new("v")],
        // An infallible write followed by `Ok(())`: one element-function
        // shape rather than two, because three of the four element kinds
        // can refuse and a generator choosing between two shapes would
        // be choosing per construct. Not `Ok(call)`: that hands the
        // write's `()` to a function, which is `clippy::unit_arg` in
        // every crate that includes the generated text.
        body: Box::new(if fallible {
            call
        } else {
            rust::Expr::Block(rust::Block {
                statements: vec![
                    rust::Statement::Semi(call),
                    rust::Statement::Tail(rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                        args: vec![rust::Expr::Tuple(Vec::new())],
                    }),
                ],
            })
        }),
    })
}

/// The element function for an array with `remaining` dimensions still
/// to unwrap.
///
/// `long grid[2][3]` is `[[i32; 3]; 2]`, so the outer `write_array`
/// hands each row to a function that writes a row.
///
/// `packed` wraps each level's element in a
/// `Writer::begin_packed_element` / `finish_packed_element` pair, at the
/// same site `zerodds-wire-codec`'s `write_array_dimension` writes them:
/// the row and every cell inside it each begin their own alignment block.
///
/// zerodds-lint: recursion-depth 8 (one level per dimension of the array
/// declarator: `remaining` falls by one per call and the cell ends it; the
/// depth is the number of dimensions the IDL writes, not a property of a
/// sample)
fn array_element_writer(
    at: &str,
    element: &MemberWrite,
    remaining: usize,
    packed: bool,
) -> Result<rust::Expr, EncodeError> {
    if remaining == 0 {
        // A cell: the element position and the packed block, and never an
        // element length — only a `sequence` has that axis.
        return sequence_element_writer(at, element, packed, false);
    }
    let inner = array_element_writer(at, element, remaining - 1, packed)?;
    let call = rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("w"))),
        method: Ident::new("write_array"),
        args: vec![rust::Expr::Path(Path::single("v")), inner],
    };
    if !packed {
        return Ok(rust::Expr::Closure {
            params: vec![Ident::new("w"), Ident::new("v")],
            body: Box::new(call),
        });
    }
    Ok(rust::Expr::Closure {
        params: vec![Ident::new("w"), Ident::new("v")],
        body: Box::new(rust::Expr::Block(rust::Block {
            statements: vec![
                let_statement("zd_origin", begin_packed("w")),
                rust::Statement::Semi(rust::Expr::Try(Box::new(call))),
                rust::Statement::Semi(finish_packed("w", "zd_origin")),
                rust::Statement::Tail(ok_unit()),
            ],
        })),
    })
}

/// The element function a `sequence`'s write takes, with the two HDDS
/// axes the reference codec puts around each element.
///
/// With neither axis selected this is [`element_writer`] unchanged, so
/// every other dialect's emitted code is the same text it was.
fn sequence_element_writer(
    at: &str,
    element: &MemberWrite,
    packed: bool,
    carries_length: bool,
) -> Result<rust::Expr, EncodeError> {
    if !packed && !carries_length {
        return element_writer(at, element);
    }
    let (call, fallible) = element_write_call(at, element)?;
    let mut statements = Vec::new();
    if packed {
        statements.push(let_statement("zd_origin", begin_packed("w")));
    }
    if carries_length {
        statements.push(let_statement(
            "zd_length",
            rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Path(Path::single("w"))),
                method: Ident::new("write_dheader"),
                args: Vec::new(),
            },
        ));
    }
    statements.push(rust::Statement::Semi(if fallible {
        rust::Expr::Try(Box::new(call))
    } else {
        call
    }));
    if carries_length {
        statements.push(rust::Statement::Semi(finish_dheader("w", "zd_length")));
    }
    if packed {
        statements.push(rust::Statement::Semi(finish_packed("w", "zd_origin")));
    }
    statements.push(rust::Statement::Tail(ok_unit()));
    Ok(rust::Expr::Closure {
        params: vec![Ident::new("w"), Ident::new("v")],
        body: Box::new(rust::Expr::Block(rust::Block { statements })),
    })
}

/// The write one collection element is, at the element position, and
/// whether it returns a `Result`.
///
/// [`write_call`] names a `wchar`'s *member* framing; a collection
/// element takes the other one, so it is substituted here. A `wstring`
/// element has its position rewritten by [`as_element`] first, for the
/// same reason.
fn element_write_call(at: &str, element: &MemberWrite) -> Result<(rust::Expr, bool), EncodeError> {
    let substituted = as_element(element);
    let element = substituted.as_ref().unwrap_or(element);
    if matches!(element, MemberWrite::Primitive(PrimitiveKind::WChar)) {
        return Ok((
            rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&[
                    "Writer",
                    "write_wchar_element",
                ]))),
                args: vec![
                    rust::Expr::Path(Path::single("w")),
                    rust::Expr::Path(Path::single("v")),
                ],
            },
            true,
        ));
    }
    write_call(
        "w",
        SinkShape::Reference,
        at,
        element,
        rust::Expr::Path(Path::single("v")),
        true,
    )
}

/// `w.begin_packed_element()`.
fn begin_packed(sink: &str) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single(sink))),
        method: Ident::new("begin_packed_element"),
        args: Vec::new(),
    }
}

/// `w.finish_packed_element(saved)`.
fn finish_packed(sink: &str, saved: &str) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single(sink))),
        method: Ident::new("finish_packed_element"),
        args: vec![rust::Expr::Path(Path::single(saved))],
    }
}

/// `w.finish_dheader(header)`.
fn finish_dheader(sink: &str, header: &str) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single(sink))),
        method: Ident::new("finish_dheader"),
        args: vec![rust::Expr::Path(Path::single(header))],
    }
}

/// `Ok(())`.
fn ok_unit() -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Tuple(Vec::new())],
    }
}

/// Wrap a generated `encode`/`decode` body in a packed-element block.
///
/// `ElementPadding::Packed` makes every aggregate and union value its own
/// alignment block: its members align from its own first octet, not from
/// the encapsulation, and the value is not padded up to its own alignment
/// before it starts. `sink` is `w` on the write side and `r` on the read
/// side.
///
/// The `finish` goes before the body's final statement — the `Ok(…)` the
/// body builds — so the restored origin is in effect for whatever the
/// caller writes after the value. A body that returns early on an error
/// leaves the origin displaced, which the caller discards with the
/// buffer: the same shape `zerodds-wire-codec`'s `?` between begin and
/// finish has.
fn wrap_packed_body(
    mut statements: Vec<rust::Statement>,
    sink: &str,
    packed: bool,
) -> Vec<rust::Statement> {
    if !packed {
        return statements;
    }
    let tail = statements.pop();
    let mut out = Vec::with_capacity(statements.len() + 3);
    out.push(let_statement("zd_origin", begin_packed(sink)));
    out.extend(statements);
    out.push(rust::Statement::Semi(finish_packed(sink, "zd_origin")));
    if let Some(tail) = tail {
        out.push(tail);
    }
    out
}

/// The same shape, reached as a collection element rather than as a
/// member.
///
/// `None` where the shape carries no position, which is every shape but
/// one. A `Some` is a shape whose emitted call names a position and
/// therefore has to be rewritten before the call is built — not a
/// difference in the octets today, because all nine measured
/// implementations answer the two positions alike on all four `wstring`
/// axes, and the rewrite is what keeps that a measurement rather than a
/// thing nobody asked.
fn as_element(element: &MemberWrite) -> Option<MemberWrite> {
    match element {
        MemberWrite::WString {
            bound,
            narrow,
            policy,
            units,
            order,
            position: _,
        } => Some(MemberWrite::WString {
            bound: *bound,
            narrow: narrow.clone(),
            policy: *policy,
            units: *units,
            order: *order,
            position: WirePosition::Element,
        }),
        _ => None,
    }
}

/// `zerodds_wire::WstringLengthPolicy::new(C1, C2, C3)`.
///
/// The constructor and not one of the five named constants, for every
/// cell alike. A table from cell to name is a table that can be wrong
/// about a cell, and the three cells no implementation writes have no
/// name to look up — which is exactly the shape that left the ROS cell
/// unreachable while the names were the values.
///
/// Absolute paths rather than imports, because the generated file's
/// import list is built by each consumer of this emitter and a new name
/// in it would have to be added in each one.
fn wstring_policy_expr(policy: WstringLengthPolicy) -> rust::Expr {
    let counts = match policy.counts {
        WstringLengthCounts::Octets => "Octets",
        WstringLengthCounts::Characters => "Characters",
    };
    let terminator = match policy.terminator {
        WstringTerminator::Absent => "Absent",
        WstringTerminator::Present => "Present",
    };
    let unit = match policy.unit {
        WstringUnitWidth::Two => "Two",
        WstringUnitWidth::Four => "Four",
    };
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::new(&[
            "zerodds_wire",
            "WstringLengthPolicy",
            "new",
        ]))),
        args: vec![
            rust::Expr::Path(Path::new(&["zerodds_wire", "WstringLengthCounts", counts])),
            rust::Expr::Path(Path::new(&[
                "zerodds_wire",
                "WstringTerminator",
                terminator,
            ])),
            rust::Expr::Path(Path::new(&["zerodds_wire", "WstringUnitWidth", unit])),
        ],
    }
}

/// `zerodds_wire::WstringUnitOrder::<C4>` — the fourth axis.
fn wstring_order_expr(order: WstringUnitOrder) -> rust::Expr {
    rust::Expr::Path(Path::new(&[
        "zerodds_wire",
        "WstringUnitOrder",
        match order {
            WstringUnitOrder::Stream => "Stream",
            WstringUnitOrder::BigEndianAlways => "BigEndianAlways",
            WstringUnitOrder::ByteOrderMark => "ByteOrderMark",
            WstringUnitOrder::HostUnswapped => "HostUnswapped",
        },
    ]))
}

/// `zerodds_wire::WirePosition::<position>`.
///
/// The two positions a `wstring` can be reached at, and an error for
/// the four it cannot. Not a fallback to `Member`: a position that
/// silently became the member's is precisely how a `wchar` element got
/// a member's framing through five emitters, and the runtime's own
/// enumeration has no name for a `wstring` in a parameter sentinel to
/// map onto.
fn wire_position_expr(member: &str, position: WirePosition) -> Result<rust::Expr, EncodeError> {
    let name = match position {
        WirePosition::Member => "Member",
        WirePosition::Element => "Element",
        WirePosition::Discriminator
        | WirePosition::KeyHolderMember
        | WirePosition::Sentinel
        | WirePosition::ExtendedMarker => {
            return Err(EncodeError::NoWriter {
                member: member.to_string(),
                kind: "a wstring at a position the wire runtime has no name for",
            });
        }
    };
    Ok(rust::Expr::Path(Path::new(&[
        "zerodds_wire",
        "WirePosition",
        name,
    ])))
}

/// The integer writer an enumeration's bit bound calls for.
const fn enum_writer(bits: u16) -> &'static str {
    match bits {
        0..=8 => "write_u8",
        9..=16 => "write_u16",
        17..=32 => "write_u32",
        _ => "write_u64",
    }
}

/// The expression that reads one value, of type `Result<T, WireError>`.
///
/// The narrowing §7.2.2.7 asks for is part of this expression and not a
/// step after it, so a member and a collection element get the same
/// treatment from one place — the element position is the one
/// §7.3.1.2.1.12.4 names and the one OpenDDS implements separately from
/// the collection's own bound.
///
/// zerodds-lint: recursion-depth 64 — one frame per external-member nesting
/// level; the shared lowering bounds that nesting before it reaches here.
///
/// `pub(crate)`: see [`write_value_to`].
pub(crate) fn read_for(name: &str, kind: &MemberWrite) -> Result<rust::Expr, EncodeError> {
    let read = read_whole(name, kind)?;
    match narrowing_of(kind) {
        Some(narrow) => narrowed(read, kind, narrow, name),
        None => Ok(read),
    }
}

/// The narrowing of a shape, where the shape has one.
const fn narrowing_of(kind: &MemberWrite) -> Option<&Narrowing> {
    match kind {
        MemberWrite::String { narrow, .. }
        | MemberWrite::WString { narrow, .. }
        | MemberWrite::Sequence { narrow, .. }
        | MemberWrite::Map { narrow, .. } => narrow.as_ref(),
        _ => None,
    }
}

/// `{ let v = <read>?; match r.fit_…(v, bound, TryConstruct::X)? { … } }`
///
/// The whole value comes off the wire first and the bound is applied
/// afterwards. A reader that stopped at its own bound would leave the
/// rest in the stream and put every following member at the wrong
/// offset — OpenDDS reads all twelve characters of `"Hello World!"`
/// before it defaults or truncates, and the members after it come back
/// correct, which is the property `try_construct_behaviour.md` singles
/// out.
fn narrowed(
    read: rust::Expr,
    kind: &MemberWrite,
    narrow: &Narrowing,
    name: &str,
) -> Result<rust::Expr, EncodeError> {
    let fit = match kind {
        MemberWrite::String { .. } => "fit_string",
        // The bound counts UTF-16 code units and not octets, so the
        // runtime has its own narrowing rather than `fit_string` with
        // another argument: one function taking "octets or units" would
        // be a call site deciding which by getting an argument right.
        MemberWrite::WString { .. } => "fit_wstring",
        MemberWrite::Sequence { .. } => "fit_sequence",
        // Table 11 names `map` in the same breath as `sequence`, and
        // the bound counts entries, so the runtime has its own `fit_map`
        // rather than a `fit_sequence` over the halves.
        MemberWrite::Map { .. } => match crate::adapter::cst::rust_map_form() {
            crate::adapter::cst::RustMapForm::Entries => "fit_map_entries",
            crate::adapter::cst::RustMapForm::BTreeMap => "fit_map",
        },
        // Table 11: TRIM "only applies to elements or members of type
        // string, wide string, sequence, or map", and the front end
        // refuses the annotation anywhere else
        // (`validate.rs::refuse_trim`). No other shape carries a
        // `Narrowing` at all, so this arm is a model that did not come
        // through the front end.
        _ => {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind: "a bound on a type that cannot have one",
            });
        }
    };
    let bound = rust::escape_keyword(name);
    // USE_DEFAULT is the only behaviour that needs a value here; under
    // DISCARD `fit_…` has already returned `Err(WireError::Discarded)`
    // and under TRIM it has returned the trimmed value.
    let default = match (&narrow.default, narrow.behaviour) {
        (Some(default), _) => default.clone(),
        (None, TryConstruct::UseDefault) => {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind: "a `@try_construct(USE_DEFAULT)` position whose default this column \
                       cannot spell",
            });
        }
        // Under DISCARD and TRIM the runtime never answers `Default`,
        // so the arm exists only because the match has to be
        // exhaustive. A refusal and not a panic: a panic in a decoder is
        // a denial of service a peer can send, and this one would mean
        // the generated code and the runtime disagree about what the
        // behaviour was.
        (None, _) => rust::Expr::Path(Path::single("__unreachable")),
    };
    Ok(rust::Expr::Block(rust::Block {
        statements: vec![
            let_statement(name, rust::Expr::Try(Box::new(read))),
            rust::Statement::Tail(rust::Expr::Match {
                scrutinee: Box::new(rust::Expr::Try(Box::new(call(
                    fit,
                    vec![
                        rust::Expr::Path(Path::single(&bound)),
                        rust::Expr::Literal(Literal::int(narrow.bound)),
                        rust::Expr::Path(Path::new(&[
                            "TryConstruct",
                            try_construct_variant(narrow.behaviour),
                        ])),
                    ],
                )))),
                arms: vec![
                    rust::MatchArm {
                        pattern: rust::Pattern::TupleStruct {
                            path: Path::new(&["Narrowed", "Value"]),
                            elements: vec![rust::Pattern::Binding(Ident::new(&bound))],
                        },
                        guard: None,
                        body: rust::Expr::Call {
                            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                            args: vec![rust::Expr::Path(Path::single(&bound))],
                        },
                    },
                    rust::MatchArm {
                        pattern: rust::Pattern::Path(Path::new(&["Narrowed", "Default"])),
                        guard: None,
                        body: if narrow.behaviour == TryConstruct::UseDefault {
                            rust::Expr::Call {
                                callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                                args: vec![default],
                            }
                        } else {
                            rust::Expr::Call {
                                callee: Box::new(rust::Expr::Path(Path::single("Err"))),
                                args: vec![rust::Expr::Call {
                                    callee: Box::new(rust::Expr::Path(Path::new(&[
                                        "WireError",
                                        "refused",
                                    ]))),
                                    args: vec![
                                        call("position", Vec::new()),
                                        rust::Expr::Literal(Literal::string(
                                            "a narrowing that asked for no default and got one",
                                        )),
                                    ],
                                }],
                            }
                        },
                    },
                ],
            }),
        ],
    }))
}

/// The runtime's spelling of one behaviour.
const fn try_construct_variant(behaviour: TryConstruct) -> &'static str {
    match behaviour {
        TryConstruct::Discard => "Discard",
        TryConstruct::UseDefault => "UseDefault",
        TryConstruct::Trim => "Trim",
    }
}

/// The read itself, before any bound is applied.
///
/// zerodds-lint: recursion-depth 64 — one frame per external-member nesting
/// level; the shared lowering bounds that nesting before it reaches here.
fn read_whole(name: &str, kind: &MemberWrite) -> Result<rust::Expr, EncodeError> {
    Ok(match kind {
        MemberWrite::External(inner) => rust::Expr::MethodCall {
            receiver: Box::new(read_for(name, inner)?),
            method: Ident::new("map"),
            args: vec![rust::Expr::Path(Path::new(&["Box", "new"]))],
        },
        MemberWrite::Primitive(primitive) => call(reader_name(*primitive), Vec::new()),
        MemberWrite::String { .. } => call("read_string", Vec::new()),
        MemberWrite::Fixed {
            digits,
            scale,
            count,
        } => call(
            "read_fixed",
            vec![
                rust::Expr::Literal(Literal::int(i128::from(*digits))),
                rust::Expr::Literal(Literal::int(i128::from(*scale))),
                fixed_digits_expr(*count),
            ],
        ),
        MemberWrite::WString {
            policy,
            units,
            order,
            position,
            ..
        } => {
            // The mirror of `write_call`'s branch: `Utf8` names the
            // runtime's UTF-8 reader, which takes no order.
            if *units == WstringCodeUnits::Utf8 {
                call(
                    "read_wstring_utf8",
                    vec![
                        wstring_policy_expr(*policy),
                        wire_position_expr(name, *position)?,
                    ],
                )
            } else {
                call(
                    "read_wstring",
                    vec![
                        wstring_policy_expr(*policy),
                        wstring_order_expr(*order),
                        wire_position_expr(name, *position)?,
                    ],
                )
            }
        }
        MemberWrite::Enumeration { bits } => call(enum_reader(*bits), Vec::new()),
        // `read_bitmask` answers in the `u64` it reads into, and the
        // field is the holder the bound calls for, so the narrowing
        // rides on the `Result` rather than after it: `read_for` may
        // wrap this expression in a `try_construct` arm, and a cast
        // written outside would have been applied to the wrong thing.
        MemberWrite::Bitmask { bits, declared } => narrow_from_u64(
            call(
                "read_bitmask",
                vec![
                    rust::Expr::Literal(Literal::int(i128::from(*bits))),
                    rust::Expr::Literal(Literal::int(i128::from(*declared))),
                ],
            ),
            *bits,
        ),
        // The mirror of the encoder's `self.<field>.encode(w)`. The read
        // side needs no `extended`: `Reader::read_pid_as` takes the
        // header form off the wire, so one decoder serves both forms a
        // per-sample plan writes.
        MemberWrite::Nested {
            type_name, codec, ..
        } => rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&[
                type_name.as_str(),
                &format!("decode{codec}"),
            ]))),
            args: vec![rust::Expr::Path(Path::single("r"))],
        },
        MemberWrite::Sequence {
            element,
            delimited,
            packed,
            carries_length,
            ..
        } => call(
            if *delimited {
                "read_delimited_sequence"
            } else {
                "read_sequence"
            },
            vec![sequence_element_reader(
                name,
                element,
                *packed,
                *carries_length,
            )?],
        ),
        MemberWrite::Map {
            key,
            value,
            delimited,
            ..
        } => call(
            match (crate::adapter::cst::rust_map_form(), *delimited) {
                (crate::adapter::cst::RustMapForm::Entries, true) => "read_delimited_map_entries",
                (crate::adapter::cst::RustMapForm::Entries, false) => "read_map_entries",
                (crate::adapter::cst::RustMapForm::BTreeMap, true) => "read_delimited_map",
                (crate::adapter::cst::RustMapForm::BTreeMap, false) => "read_map",
            },
            vec![element_reader(name, key)?, element_reader(name, value)?],
        ),
        MemberWrite::Array {
            element,
            dims,
            delimited,
            packed,
        } => call(
            // `_n`, not the `Vec` form: the field an array materialises
            // to is `[T; N]`, and `N` is inferred from it.
            if *delimited {
                "read_delimited_array_n"
            } else {
                "read_array_n"
            },
            vec![array_element_reader(
                name,
                element,
                dims.len().saturating_sub(1),
                *packed,
            )?],
        ),
        MemberWrite::Unsupported(kind) => {
            return Err(EncodeError::NoWriter {
                member: name.to_string(),
                kind,
            });
        }
    })
}

/// The element function a collection's read takes.
///
/// A path where one has the right shape — `Reader::read_i32` and
/// `Cell::decode` are both `fn(&mut Reader) -> Result<T, WireError>` —
/// and a closure where the read needs an argument of its own.
fn element_reader(at: &str, element: &MemberWrite) -> Result<rust::Expr, EncodeError> {
    // The element position, substituted for the reason
    // `element_writer` gives.
    let substituted = as_element(element);
    let element = substituted.as_ref().unwrap_or(element);
    // An element with a bound of its own takes a closure whatever its
    // shape: §7.3.1.2.1.12.4 gives the element position its own
    // behaviour, and a named path has nowhere to put it. This is the
    // split Table 17 draws and OpenDDS implements — the sequence's
    // length overflow and an element's bound overflow fail separately.
    if narrowing_of(element).is_some() {
        return Ok(rust::Expr::Closure {
            params: vec![Ident::new("r")],
            body: Box::new(read_for(at, element)?),
        });
    }
    Ok(match element {
        // As `element_writer`: the element's `wchar` reader, not the
        // member's.
        MemberWrite::Primitive(PrimitiveKind::WChar) => {
            rust::Expr::Path(Path::new(&["Reader", "read_wchar_element"]))
        }
        MemberWrite::Primitive(primitive) => {
            rust::Expr::Path(Path::new(&["Reader", reader_name(*primitive)]))
        }
        MemberWrite::String { .. } => rust::Expr::Path(Path::new(&["Reader", "read_string"])),
        // A closure and not a path, for the reason `wstring` needs one:
        // `read_fixed` takes the declaration and the digit count as
        // arguments, so there is no bare
        // `fn(&mut Reader) -> Result<T, WireError>` to hand over.
        MemberWrite::Fixed { .. } => rust::Expr::Closure {
            params: vec![Ident::new("r")],
            body: Box::new(read_for(at, element)?),
        },
        // A closure and not a path: `read_wstring` takes the cell and
        // the order as arguments, so there is no bare
        // `fn(&mut Reader) -> Result<T, WireError>` to hand over.
        MemberWrite::WString { .. } => rust::Expr::Closure {
            params: vec![Ident::new("r")],
            body: Box::new(read_for(at, element)?),
        },
        MemberWrite::Enumeration { bits } => {
            rust::Expr::Path(Path::new(&["Reader", enum_reader(*bits)]))
        }
        MemberWrite::Nested {
            type_name, codec, ..
        } => rust::Expr::Path(Path::new(&[type_name.as_str(), &format!("decode{codec}")])),
        // A closure for the reason `read_wstring` needs one: the bound
        // and the declared mask are arguments, so there is no bare
        // `fn(&mut Reader) -> Result<T, WireError>` to name.
        MemberWrite::Bitmask { .. }
        | MemberWrite::External(_)
        | MemberWrite::Sequence { .. }
        | MemberWrite::Array { .. }
        | MemberWrite::Map { .. } => rust::Expr::Closure {
            params: vec![Ident::new("r")],
            body: Box::new(read_for(at, element)?),
        },
        MemberWrite::Unsupported(kind) => {
            return Err(EncodeError::NoWriter {
                member: at.to_string(),
                kind,
            });
        }
    })
}

/// A bitmask's holder widened to the `u64` `Writer::write_bitmask`
/// takes.
///
/// `as u64` and not `u64::from`: the 64-bit holder is already a `u64`
/// and `u64::from(x)` on one is a type error, while the cast is
/// accepted for all four widths. Widening is exact in every one of
/// them, so nothing can be lost here.
fn widen_to_u64(value: rust::Expr, bits: u16) -> rust::Expr {
    if crate::adapter::cst::bitmask_primitive(bits) == rust::Primitive::U64 {
        return value;
    }
    rust::Expr::Cast {
        value: Box::new(value),
        type_: Box::new(rust::Type::Primitive(rust::Primitive::U64)),
    }
}

/// `Reader::read_bitmask`'s `u64` narrowed back to the holder the field
/// is declared as.
///
/// Exact for the same reason the writer's widening is: the runtime has
/// already refused every bit above the bound, so nothing this cast drops
/// could have been set.
fn narrow_from_u64(read: rust::Expr, bits: u16) -> rust::Expr {
    let holder = crate::adapter::cst::bitmask_primitive(bits);
    if holder == rust::Primitive::U64 {
        return read;
    }
    rust::Expr::MethodCall {
        receiver: Box::new(read),
        method: Ident::new("map"),
        args: vec![rust::Expr::Closure {
            params: vec![Ident::new("v")],
            body: Box::new(rust::Expr::Cast {
                value: Box::new(rust::Expr::Path(Path::single("v"))),
                type_: Box::new(rust::Type::Primitive(holder)),
            }),
        }],
    }
}

/// The element function for an array with `remaining` dimensions left.
///
/// `packed` wraps each level's element in a
/// `Reader::begin_packed_element` / `finish_packed_element` pair, the
/// read side of `array_element_writer`'s: the row and every cell inside
/// it each begin their own alignment block.
///
/// zerodds-lint: recursion-depth 8 (as `array_element_writer`: one level
/// per dimension of the array declarator)
fn array_element_reader(
    at: &str,
    element: &MemberWrite,
    remaining: usize,
    packed: bool,
) -> Result<rust::Expr, EncodeError> {
    if remaining == 0 {
        // A cell: the element position and the packed block, and never an
        // element length.
        return sequence_element_reader(at, element, packed, false);
    }
    let inner = array_element_reader(at, element, remaining - 1, packed)?;
    let call = rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("r"))),
        method: Ident::new("read_array_n"),
        args: vec![inner],
    };
    if !packed {
        return Ok(rust::Expr::Closure {
            params: vec![Ident::new("r")],
            body: Box::new(call),
        });
    }
    Ok(rust::Expr::Closure {
        params: vec![Ident::new("r")],
        body: Box::new(rust::Expr::Block(rust::Block {
            statements: vec![
                let_statement("zd_origin", begin_packed("r")),
                let_statement("value", rust::Expr::Try(Box::new(call))),
                rust::Statement::Semi(finish_packed("r", "zd_origin")),
                rust::Statement::Tail(rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                    args: vec![rust::Expr::Path(Path::single("value"))],
                }),
            ],
        })),
    })
}

/// The element function a `sequence`'s read takes, with the two HDDS
/// axes the reference codec puts around each element.
///
/// With neither axis selected this is [`element_reader`] unchanged, so
/// every other dialect's emitted code is the same text it was.
///
/// The element's own length is checked against what reading it consumed,
/// exactly as `zerodds-wire-codec`'s sequence walk does: the declared
/// count is a peer's claim, and a value that did not fill it leaves the
/// next element to be read from inside this one.
fn sequence_element_reader(
    at: &str,
    element: &MemberWrite,
    packed: bool,
    carries_length: bool,
) -> Result<rust::Expr, EncodeError> {
    if !packed && !carries_length {
        return element_reader(at, element);
    }
    let read = element_read_expr(at, element)?;
    let mut statements = Vec::new();
    if packed {
        statements.push(let_statement("zd_origin", begin_packed("r")));
    }
    if carries_length {
        statements.push(let_statement(
            "zd_declared",
            rust::Expr::Cast {
                value: Box::new(rust::Expr::Try(Box::new(call("read_u32", Vec::new())))),
                type_: Box::new(rust::Type::named(Path::single("usize"))),
            },
        ));
        statements.push(let_statement("zd_before", call("position", Vec::new())));
    }
    statements.push(let_statement("value", rust::Expr::Try(Box::new(read))));
    if carries_length {
        statements.push(rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Ne,
                lhs: Box::new(rust::Expr::Binary {
                    op: rust::BinaryOp::Sub,
                    lhs: Box::new(call("position", Vec::new())),
                    rhs: Box::new(rust::Expr::Path(Path::single("zd_before"))),
                }),
                rhs: Box::new(rust::Expr::Path(Path::single("zd_declared"))),
            }),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Return(Some(Box::new(
                    refused_err(
                        "r",
                        "a sequence element of the octet count its own length declared",
                    ),
                ))))],
            },
            otherwise: None,
        }));
    }
    if packed {
        statements.push(rust::Statement::Semi(finish_packed("r", "zd_origin")));
    }
    statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Path(Path::single("value"))],
    }));
    Ok(rust::Expr::Closure {
        params: vec![Ident::new("r")],
        body: Box::new(rust::Expr::Block(rust::Block { statements })),
    })
}

/// The read one collection element is, at the element position.
///
/// [`read_for`] names a `wchar`'s *member* framing; a collection element
/// takes the other one, so it is substituted here. A `wstring` element
/// has its position rewritten by [`as_element`] first.
fn element_read_expr(at: &str, element: &MemberWrite) -> Result<rust::Expr, EncodeError> {
    let substituted = as_element(element);
    let element = substituted.as_ref().unwrap_or(element);
    if matches!(element, MemberWrite::Primitive(PrimitiveKind::WChar)) {
        return Ok(call("read_wchar_element", Vec::new()));
    }
    read_for(at, element)
}

/// The integer reader an enumeration's bit bound calls for.
const fn enum_reader(bits: u16) -> &'static str {
    match bits {
        0..=8 => "read_u8",
        9..=16 => "read_u16",
        17..=32 => "read_u32",
        _ => "read_u64",
    }
}

/// What kind of write a member needs.
///
/// `pub(crate)`: see [`write_value_to`]. `emit::interface_rust` never
/// constructs a variant beside [`Self::Primitive`] — it gets every other
/// shape from [`write_of`] — so it names the type and this one variant
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemberWrite {
    /// Owning indirection in the Rust binding, with unchanged wire layout.
    External(Box<MemberWrite>),
    /// A primitive, with its own writer.
    Primitive(PrimitiveKind),
    /// A string, with its bound when it has one.
    ///
    /// The bound is written into the call rather than checked at
    /// runtime by the writer: a bounded string that overruns is a
    /// program error, and the bound is what the IDL said, not a
    /// parameter of the wire format.
    String {
        /// The bound, absent when unbounded.
        bound: Option<i128>,
        /// What this *position* does with a value the bound cannot hold.
        /// Absent when the string is unbounded, because there is then
        /// nothing a value can fail to fit.
        narrow: Option<Narrowing>,
    },
    /// A `fixed<P, S>`, with its declaration and the digit-count axis.
    ///
    /// The value is a decimal **string** in this column — `cst.rs` gives
    /// `fixed` the Rust spelling `String` and `default_rust.rs` gives it a
    /// zero at the declared scale — for the reason the other twenty columns
    /// carry it as text too: Rust has no decimal type in `core`, and a
    /// `fixed<31, 2>` does not fit any integer Rust has. A binary float
    /// would be the wrong answer rather than an approximate one, because
    /// `fixed` exists to be exact.
    ///
    /// Three fields and not one: `P` and `S` are the declaration, and
    /// `count` is the dialect axis over CORBA 3.3 part 2 §9.3.2.8's open
    /// question of how many digit nibbles go out. All three go into the
    /// emitted call, so the generated codec states the cell it writes.
    Fixed {
        /// `P` — the total digits the declaration names.
        digits: u8,
        /// `S` — the digits after the point.
        scale: u8,
        /// Whether `P` digits go out whatever the value is, or as many as
        /// the value has. `zerodds_ast_layout::key::FixedDigitCount`.
        count: FixedDigitCount,
    },
    /// A `wstring`, with its bound and the four axes that decide its
    /// octets.
    ///
    /// Five fields and not one, because the cell is a product: C1, C2
    /// and C3 are [`WstringLengthPolicy`]'s three coordinates, C4 is
    /// [`WstringUnitOrder`], and the position is what keeps the claim
    /// that the two positions agree a measurement rather than an
    /// assumption. All four come out of the [`LayoutKey`] and go into
    /// the emitted call, so the generated codec states the cell it
    /// writes instead of inheriting one from the runtime.
    WString {
        /// The bound in **UTF-16 code units**, absent when unbounded.
        ///
        /// Units and not octets: `wstring<N>` is an `N`-unit buffer in
        /// every measured mapping, and an octet bound would mean two
        /// different capacities under C3's two widths.
        bound: Option<i128>,
        /// What this *position* does with a value the bound cannot
        /// hold. Absent when the `wstring` is unbounded.
        narrow: Option<Narrowing>,
        /// C1 × C2 × C3 — what the length counts, whether a terminator
        /// joins the run, how wide one code unit is.
        policy: WstringLengthPolicy,
        /// What the content's characters are made of. Under
        /// [`WstringCodeUnits::Utf8`] the characters go out as their own
        /// UTF-8 octets and C1 and C3 are vacuous, so the emitted call
        /// names the runtime's UTF-8 entry point instead.
        units: WstringCodeUnits,
        /// C4 — in what byte order the units go out, and whether a
        /// byte-order mark leads them.
        order: WstringUnitOrder,
        /// Where the `wstring` is reached from.
        position: WirePosition,
    },
    /// A sequence, with its element and its bound.
    Sequence {
        /// What each element is.
        element: Box<MemberWrite>,
        /// The bound, absent when unbounded.
        bound: Option<i128>,
        /// As [`MemberWrite::String`]'s.
        narrow: Option<Narrowing>,
        /// Whether a `DHEADER` precedes the length. Rule (12), decided
        /// by `zerodds_ast_layout::collection_is_delimited` so that the
        /// emitted bytes and the planned layout cannot disagree.
        delimited: bool,
        /// Whether each element begins its own alignment block.
        /// `ElementPadding::Packed`'s writer half: this column wraps the
        /// element function in a `Writer::begin_packed_element` /
        /// `finish_packed_element` pair, at the same site
        /// `zerodds-wire-codec` writes them.
        packed: bool,
        /// Whether each element is preceded by its own four-octet
        /// length. `SequenceElementDelimiter::BeforeEachNonFinalElement`,
        /// decided by `zerodds_ast_layout::xcdr::sequence_element_carries_length`
        /// so the two walkers ask one crossing.
        carries_length: bool,
    },
    /// A map, with both halves and its bound.
    ///
    /// Two element functions and not one: a `map<K, V>` is a sequence of
    /// pairs (§7.4.1.1.4) and the halves are two different types, so the
    /// runtime takes a function per half.
    ///
    /// # The entry order is the container's and is not decided here
    ///
    /// `adapter::cst` materialises an IDL `map` as a `BTreeMap<K, V>`,
    /// which iterates ascending by key, and
    /// `zerodds_ast_layout::MapEntryOrder::AscendingKey` is defined as
    /// what a binding that holds a `map` in an ordered container writes.
    /// So this column answers the axis by the type it already emits.
    /// There is no walk here to bypass it and no second reading to keep
    /// in step — which is why this shape carries no ordering field, and
    /// why `crates/lang-rules/src/encode.rs` is not one of the sites
    /// `wire-probe`'s `map_entry_order.rs` counts crossings in.
    Map {
        /// What each entry's key is.
        key: Box<MemberWrite>,
        /// What each entry's value is.
        value: Box<MemberWrite>,
        /// The bound, in **entries**, absent when unbounded.
        bound: Option<i128>,
        /// As [`MemberWrite::String`]'s. Table 11 names `map` among the
        /// forms a TRIM may be written on.
        narrow: Option<Narrowing>,
        /// Whether a `DHEADER` precedes the entry count. Rule (14)
        /// exempts a map only when both halves are primitive and rule
        /// (15) delimits the rest, decided by
        /// `zerodds_ast_layout::xcdr::plan` through `MapDelimiter` so
        /// that the emitted bytes and the planned layout cannot
        /// disagree.
        delimited: bool,
        /// Whether the entries are written ascending by key: the map is held as its entries
        /// (`adapter::cst::RustMapForm::Entries`), which keep the order they were put in, and
        /// `MapEntryOrder::AscendingKey` asked for the other one.
        ascending: bool,
    },
    /// An array, with its element and its dimensions outermost first.
    Array {
        /// What each element is.
        element: Box<MemberWrite>,
        /// The dimensions, outermost first.
        dims: Vec<i128>,
        /// Whether a `DHEADER` precedes the elements. Rule (9).
        delimited: bool,
        /// Whether each element begins its own alignment block, as
        /// [`MemberWrite::Sequence`]'s. Arrays never carry an element
        /// length: only `sequence` has that axis.
        packed: bool,
    },
    /// An enumeration, written as the integer its bit bound calls for.
    Enumeration {
        /// The bit bound.
        bits: u16,
    },
    /// A bitmask, written in the holder Table 33 of XTypes 1.3
    /// §7.4.1.1.3.2 gives for its `@bit_bound`.
    ///
    /// # Not `Enumeration`, and not `Nested`
    ///
    /// Not [`MemberWrite::Enumeration`], although both are an integer in
    /// a holder: an enumeration's holder width goes through
    /// `EnumHolderWidth`, the axis M1 measured, and a bitmask's does
    /// not — `zerodds_ast_layout::xcdr::enumeration_bit_bound` says so
    /// in as many words. Sharing the shape would have let a setting
    /// meant for one construct move the other's octets.
    ///
    /// Not [`MemberWrite::Nested`] either, although the other twenty
    /// columns give a bitmask its own codec function. `adapter::cst`
    /// materialises a bitmask as `pub type Flags = u16;` — a type alias
    /// to a primitive, because a Rust `enum` cannot hold two bits at
    /// once — and there is no `impl` block to hang an `encode` on.
    Bitmask {
        /// The bound Table 33 sizes the holder by, from
        /// `zerodds_ast_layout::plan::scalar_write`.
        bits: u16,
        /// The positions the type names, one bit per position. A bit set
        /// inside the bound that this mask does not carry is refused on
        /// the way in.
        declared: u64,
    },
    /// Another aggregate, which encodes itself.
    ///
    /// The name is carried because the decode side has to write it:
    /// `Cell::decode(r)` names the type and `r.read_nested()` cannot.
    Nested {
        /// The generated Rust type's name.
        type_name: String,
        /// The suffix the nested type's codec functions carry: empty
        /// for the type's first codec, [`SECONDARY_SUFFIX`] for the
        /// second one beside it.
        ///
        /// Carried rather than recomputed at the call site: `write_call`
        /// and `read_whole` are handed a `MemberWrite` and no key, and a
        /// nested call that named `encode` while the enclosing body was
        /// the XCDR1 one would frame the member under version 2 inside a
        /// version 1 stream — a sample that decodes into wrong values
        /// with nothing reporting it.
        codec: &'static str,
        /// How the sample's PL_CDR1 step decision reaches this call, and
        /// which of the nested type's two methods it names under
        /// [`Extended::Fixed`].
        ///
        /// Carried for the same reason as `codec`, and read from the same
        /// key: `write_call` is handed the shape and nothing else, so a
        /// call that could not say whether the flag was threaded would
        /// frame every nested type of a per-sample plan with the short
        /// header, also inside a sample past RTI's step.
        extended: Extended,
    },
    /// A kind with no writer yet, named so the error can say which.
    Unsupported(&'static str),
}

/// What one position does with a value its bound cannot hold.
///
/// DDS-XTypes 1.3 §7.2.2.7 attaches the behaviour to the position — a
/// member, a collection element, a map key, a union case — and not to
/// the type, so it is carried alongside the shape rather than derived
/// from it. One `typedef string<5>` may be used at two positions that
/// want different answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Narrowing {
    /// What the receiving type holds.
    bound: i128,
    /// Which of the three §7.3.1.2.1.12 behaviours this position asks
    /// for.
    behaviour: TryConstruct,
    /// The value `USE_DEFAULT` substitutes, when that is the behaviour.
    ///
    /// Computed only for `USE_DEFAULT`: a structure's default is a walk
    /// of its own members, and computing one for every bounded string
    /// in the model would be a walk per string for a value nothing
    /// reads.
    default: Option<rust::Expr>,
}

/// Where a type is being used, for the purpose of `try_construct`.
///
/// `pub(crate)`: see [`write_value_to`]. `emit::interface_rust` builds
/// one per field from `Ast::effective_try_construct` itself — an
/// operation parameter has no member declarator, so it always passes
/// `written: None`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Position {
    /// The behaviour this position asks for, already resolved through
    /// the alias chain by `Ast::effective_try_construct`.
    pub(crate) behaviour: TryConstruct,
    /// What the *member declarator* wrote, which `element_try_construct`
    /// needs for the one case §7.3.1.2.1.12 lets a member's annotation
    /// reach the elements: an array declared on the member.
    pub(crate) written: Option<TryConstruct>,
}

fn member_of(
    ast: &Ast,
    type_: Id<TypeNode>,
    node: zerodds_ast_core::NodeId,
    key: LayoutKey,
    codec: &'static str,
) -> Option<(String, MemberWrite)> {
    // `all_members_of`, so a member the aggregate inherited is found
    // here instead of being skipped as though the plan had not named it.
    let member = ast
        .all_members_of(type_)
        .into_iter()
        .find(|id| ast.member(*id).is_ok_and(|m| m.node_id == node))?;
    let member = ast.member(member).ok()?;
    // The member's own `@try_construct`, resolved through the alias
    // chain — §7.2.2.7 puts the behaviour on the position, and this is
    // the member position.
    let written = member.facets.xtypes.as_ref().and_then(|x| x.try_construct);
    let position = Position {
        behaviour: ast.effective_try_construct(written, member.type_.resolved()),
        written,
    };
    // Types this member names are written relative to the module the
    // aggregate itself lands in.
    let from = module_of(ast, type_);
    let mut write = member.type_.resolved().map_or(
        MemberWrite::Unsupported("member with an unresolved type"),
        |target| write_of(ast, from, target, key, codec, position),
    );
    // §7.2.2.4.4.4.9 puts an explicit `@default` ahead of Table 9's, and
    // only the member position can carry one — a collection element has
    // no declarator to write it on.
    if let Some(target) = member.type_.resolved() {
        if let Some(narrow) = narrowing_of_mut(&mut write) {
            if narrow.behaviour == TryConstruct::UseDefault {
                narrow.default = crate::default_rust::member_value_default(
                    ast,
                    module_of(ast, type_),
                    member,
                    target,
                )
                .ok();
            }
        }
    }
    Some((member.name.clone(), external_write(ast, member, write)))
}

fn external_write(ast: &Ast, member: &zerodds_ast::MemberNode, write: MemberWrite) -> MemberWrite {
    if crate::adapter::cst::is_external(ast, member) {
        MemberWrite::External(Box::new(write))
    } else {
        write
    }
}

/// The narrowing of a shape, where the shape has one.
fn narrowing_of_mut(write: &mut MemberWrite) -> Option<&mut Narrowing> {
    match write {
        MemberWrite::String { narrow, .. }
        | MemberWrite::WString { narrow, .. }
        | MemberWrite::Sequence { narrow, .. }
        | MemberWrite::Map { narrow, .. } => narrow.as_mut(),
        _ => None,
    }
}

/// The declaration one plan entry names.
///
/// `member_of` answers with the name and the writer shape; the default
/// value needs the declaration itself, for the `@default` it may carry.
fn member_node_of(
    ast: &Ast,
    type_: Id<TypeNode>,
    node: zerodds_ast_core::NodeId,
) -> Option<&zerodds_ast::MemberNode> {
    let member = ast
        .all_members_of(type_)
        .into_iter()
        .find(|id| ast.member(*id).is_ok_and(|m| m.node_id == node))?;
    ast.member(member).ok()
}

/// What writing one type takes.
///
/// Recursive, because a sequence of sequences is written by writing the
/// outer length and then each inner sequence — the shape of the writer
/// follows the shape of the type, which is what keeps the two from
/// drifting apart.
///
/// `pub(crate)`: see [`write_value_to`]. Unlike [`member_of`], this
/// takes a bare `Id<TypeNode>` and not a member of some aggregate — an
/// operation parameter or an exception member is neither, and DDS-RPC
/// 1.0 §7.5.1.1.4-7 synthesizes structures for them that never reach the
/// model at all.
pub(crate) fn write_of(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    target: Id<TypeNode>,
    key: LayoutKey,
    codec: &'static str,
    at: Position,
) -> MemberWrite {
    write_of_at(ast, from, target, key, codec, at, 0)
}

/// How deep a member's writer may nest before the walk refuses the type.
///
/// The same 64 every reference walk in the tree carries;
/// `zerodds-wire-codec`'s `default.rs` and `zerodds-types`'
/// `dynamic/type_.rs` both stop there.
const MAX_WRITE_DEPTH: usize = 64;

/// [`write_of`], carrying how many collection and typedef steps the walk
/// has already taken.
///
/// An aggregate is a leaf here — it writes through its own generated
/// codec under [`MemberWrite::Nested`] — so only the connectors can come
/// back round: a `sequence`'s element, an array's element, a `map`'s two
/// halves, a `typedef`'s target. A cycle among those is a finding the
/// model's invariant pass reports rather than a shape it rejects, and
/// without the counter this function read one forever.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map and typedef
/// nesting; bounded by `MAX_WRITE_DEPTH`, which returns
/// [`MemberWrite::Unsupported`] naming the construct — the same channel
/// every other refusal in this walk uses, so the emitter reports it by
/// name instead of writing a member it cannot spell)
fn write_of_at(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    target: Id<TypeNode>,
    key: LayoutKey,
    codec: &'static str,
    at: Position,
    depth: usize,
) -> MemberWrite {
    if depth > MAX_WRITE_DEPTH {
        return MemberWrite::Unsupported("a type whose writer nests past the recursion bound");
    }
    let Ok(node) = ast.type_(target) else {
        return MemberWrite::Unsupported("member with a stale type handle");
    };
    // The narrowing a bound calls for, computed once for both bounded
    // shapes. `None` for an unbounded one: there is nothing a value can
    // fail to fit.
    let narrowing = |bound: Option<i128>| {
        bound.map(|bound| Narrowing {
            bound,
            behaviour: at.behaviour,
            default: if at.behaviour == TryConstruct::UseDefault {
                crate::default_rust::default_of(ast, from, target, &node.name).ok()
            } else {
                None
            },
        })
    };
    match &node.kind {
        TypeKind::Primitive(kind) => MemberWrite::Primitive(*kind),
        // A `wstring`. The default cell is a UInt32 **octet** count and
        // UTF-16 code units with no NUL — XTypes 1.3 §7.4.1.1.2 — and
        // it is one cell of eight, not one form of three. The three
        // binary axes are what a length prefix counts (C1), whether a
        // terminating NUL joins the run and is counted (C2), and how
        // wide one code unit is (C3); `WstringLengthPolicy` is their
        // product, `WstringLengthPolicy::ALL_CELLS` enumerates it, and
        // five of the eight cells carry the name of an implementation
        // measured writing them. The byte **order** of the units is a
        // fourth axis, `WstringUnitOrder`, with four points of its own,
        // because it crosses the length convention rather than
        // extending it.
        //
        // All four come out of the key, the way `MapDelimiter` and
        // `MapEntryOrder` do, and go into the emitted call as
        // arguments. A runtime default would be a second place the wire
        // format lives.
        //
        // The bound counts UTF-16 code units and not octets, which is
        // `Reader::fit_wstring` and not `Reader::fit_string`.
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => {
            let bound = bound.expr().and_then(|expr| ast.eval_integral(expr).ok());
            MemberWrite::WString {
                bound,
                narrow: narrowing(bound),
                policy: key.wstring_length,
                units: key.wstring_code_units,
                order: key.wstring_unit_order,
                // The member position. A `wstring` reached as a
                // collection element is rewritten to
                // `WirePosition::Element` by `element_writer` and
                // `element_reader`, which is where the `wchar` framing's
                // element answer is substituted as well — the nine
                // measured implementations answer the two positions
                // alike on all four `wstring` axes, and passing the
                // position is what keeps that a measurement.
                position: WirePosition::Member,
            }
        }
        TypeKind::String { bound, .. } => {
            let bound = bound.expr().and_then(|expr| ast.eval_integral(expr).ok());
            MemberWrite::String {
                bound,
                narrow: narrowing(bound),
            }
        }
        // The digit count comes out of the key the way `MapEntryOrder` and
        // the four `wstring` axes do, and goes into the emitted call as an
        // argument. A runtime default would be a second place the wire
        // format lives.
        TypeKind::Fixed { digits, scale } => MemberWrite::Fixed {
            digits: *digits,
            scale: *scale,
            count: key.fixed_digits,
        },
        TypeKind::Sequence { element, bound } => element.resolved().map_or_else(
            || MemberWrite::Unsupported("sequence with an unresolved element"),
            |element| {
                let bound = bound.expr().and_then(|expr| ast.eval_integral(expr).ok());
                // Whether each element carries its own length is the same
                // crossing `zerodds-wire-codec`'s sequence walk asks, so
                // the emitted bytes and the reference codec cannot part.
                let Some(carries_length) =
                    zerodds_ast_layout::xcdr::sequence_element_carries_length(ast, element, key)
                else {
                    return MemberWrite::Unsupported(
                        "a sequence element whose own length cannot be told",
                    );
                };
                MemberWrite::Sequence {
                    element: Box::new(write_of_at(
                        ast,
                        from,
                        element,
                        key,
                        codec,
                        inside(ast, target, at),
                        depth + 1,
                    )),
                    bound,
                    narrow: narrowing(bound),
                    delimited: zerodds_ast_layout::collection_is_delimited(ast, element, key),
                    packed: key.element_padding == ElementPadding::Packed,
                    carries_length,
                }
            },
        ),
        TypeKind::Array { element, dims } => element.resolved().map_or_else(
            || MemberWrite::Unsupported("array with an unresolved element"),
            |element| {
                // A dimension that does not evaluate is not a zero-length
                // array. Substituting one wrote no elements at all and
                // said the sample had been encoded.
                let Ok(dims) = dims
                    .iter()
                    .map(|dim| ast.eval_integral(*dim))
                    .collect::<Result<Vec<_>, _>>()
                else {
                    return MemberWrite::Unsupported("array whose length does not evaluate");
                };
                MemberWrite::Array {
                    element: Box::new(write_of_at(
                        ast,
                        from,
                        element,
                        key,
                        codec,
                        inside(ast, target, at),
                        depth + 1,
                    )),
                    delimited: zerodds_ast_layout::array_is_delimited(ast, element, key),
                    dims,
                    packed: key.element_padding == ElementPadding::Packed,
                }
            },
        ),
        // A `map<K, V>` is a sequence of pairs, §7.4.1.1.4, and the two
        // halves are two different types — so both are walked, at the
        // element position §7.3.1.2.1.12.4 gives them.
        TypeKind::Map {
            key: k,
            value,
            bound,
        } => {
            let (Some(k), Some(value)) = (k.resolved(), value.resolved()) else {
                return MemberWrite::Unsupported("map with an unresolved key or value");
            };
            // The same refusal the C emitter makes, and the plan's own:
            // a setting that asked for ascending entries over a key type
            // whose values have no order cannot be honoured, and falling
            // back to declaration order would be a flag that reported
            // success and changed no octet. This column reaches it only
            // through the plan — the order it writes is its `BTreeMap`'s
            // — which is why the crossing itself is not named here.
            if zerodds_ast_layout::plan::map_entry_ordering(ast, target, key).is_err() {
                return MemberWrite::Unsupported(
                    "a map whose key type has no value order, under --map-entries ascending-key",
                );
            }
            let bound = bound.expr().and_then(|expr| ast.eval_integral(expr).ok());
            let mut deps = zerodds_ast_layout::xcdr::DepCollector::default();
            let Ok(layout) = zerodds_ast_layout::xcdr::plan(ast, target, key, &mut deps) else {
                return MemberWrite::Unsupported("map whose layout does not plan");
            };
            let inside = inside(ast, target, at);
            let ascending = matches!(
                zerodds_ast_layout::plan::map_entry_ordering(ast, target, key),
                Ok(Some(_))
            );
            MemberWrite::Map {
                key: Box::new(write_of_at(ast, from, k, key, codec, inside, depth + 1)),
                value: Box::new(write_of_at(ast, from, value, key, codec, inside, depth + 1)),
                bound,
                ascending,
                narrow: narrowing(bound),
                // Rule (14) exempts a map only when both halves are
                // primitive; the plan reads that through `MapDelimiter`,
                // and asking it here is what keeps the emitted bytes and
                // the planned extent from disagreeing.
                delimited: layout.needs_dheader,
            }
        }
        // A bitmask is the unsigned integer Table 33 of §7.4.1.1.3.2
        // gives for its `@bit_bound`, and the width is the **plan's**
        // answer: `plan::scalar_write` is the crossing every other
        // column asks — `crates/lang-rules/src/emit/encode.rs` reaches
        // `ScalarWrite::Bitmask` through the same call — so the twenty
        // other columns' octets and this one's cannot part company by
        // this column computing a width of its own.
        //
        // Unlike an enumeration this does **not** write through a
        // generated codec: `adapter::cst` materialises a bitmask as
        // `pub type Flags = u16;`, and a type alias to a primitive has
        // no `impl` block for an `encode` to live in.
        TypeKind::Bitmask { .. } => {
            let Some(ScalarWrite::Bitmask { bits }) =
                zerodds_ast_layout::plan::scalar_write(ast, target, key, WcharPosition::Member)
            else {
                return MemberWrite::Unsupported("a bitmask the layout plan gives no holder for");
            };
            let Some(declared) = declared_bits(ast, target, bits) else {
                return MemberWrite::Unsupported(
                    "a bitmask with a bit position that does not evaluate",
                );
            };
            MemberWrite::Bitmask { bits, declared }
        }
        // An enumeration writes through its own generated codec, like any
        // other named type. Writing the holder straight from here emitted
        // `w.write_u32(self.coord_convention)` — the field is the
        // enumeration and not its holder, so the generated file did not
        // compile, and no enumeration had ever been a struct member in a
        // tree this emitter had been pointed at. `enumeration_encode_fn`
        // already produces the `match self { Self::X => 0, .. }` that
        // respects `@value`, and it takes the holder's width from the
        // same `key`, so nothing about the wire changes.
        TypeKind::Enumeration { .. } => match crate::adapter::cst::relative_path(ast, from, node) {
            Ok(path) => MemberWrite::Nested {
                type_name: path
                    .segments()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
                codec,
                extended: Extended::of(key),
            },
            Err(_) => MemberWrite::Unsupported("an enumeration the model has no module for"),
        },
        // The behaviour was already resolved through the alias chain by
        // `Ast::effective_try_construct`, so the position carries over
        // unchanged.
        TypeKind::Alias { aliased } => aliased.resolved().map_or_else(
            || MemberWrite::Unsupported("typedef with an unresolved target"),
            |aliased| write_of_at(ast, from, aliased, key, codec, at, depth + 1),
        ),
        // A union is written the same way, and for the same reason: it
        // has a generated codec of its own, and `union_codec` gives it
        // exactly the `encode(&self, w)` and `decode(r)` that
        // `MemberWrite::Nested` calls. Leaving it out of this arm did not
        // make a union member unwritable in some deeper sense — it made
        // the emitter refuse a type it had already generated the codec
        // for, which is what `NoWriter { member: "cov", kind: "union" }`
        // was: `FramedPose.cov` is a `CovMatrix`, an `@appendable` union
        // over an enumerated discriminator, and every one of its own
        // cases emitted fine.
        // A bitset joins them: `adapter::cst` materialises it as a
        // struct with one field per named bitfield, and `bitset_encode_fn`
        // gives that struct the `encode(&self, w)` and `decode(r)` this
        // shape calls. It was in the `other` arm below, which is what
        // `NoWriter { kind: "bitset" }` was.
        TypeKind::Structure { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. } => {
            // The path, not the name: `Nested` becomes `<path>::decode`
            // in the reader, and a sibling module's type does not resolve
            // by its simple name from here.
            match crate::adapter::cst::relative_path(ast, from, node) {
                Ok(path) => MemberWrite::Nested {
                    type_name: path
                        .segments()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                    codec,
                    extended: Extended::of(key),
                },
                Err(_) => MemberWrite::Unsupported("a type the model has no module for"),
            }
        }
        other => MemberWrite::Unsupported(other.as_str()),
    }
}

/// The positions a bitmask names, one bit per position.
///
/// The mask `Reader::read_bitmask` reduces a stream to. §7.4.1.1.3.2
/// puts a bit above the bound outside the bitmask, so a position at or
/// beyond the bound is not part of the mask; a bit set inside the bound
/// that the type does not name is not part of the value either, and the
/// reader keeps only the declared positions.
///
/// The same rule `bitmask_body` in `crates/lang-rules/src/emit/decode.rs`
/// emits inline for the other twenty columns — a second copy rather than
/// a shared call, because the two are compared against each other and a
/// shared helper would make them agree about a mistake.
fn declared_bits(ast: &Ast, type_: Id<TypeNode>, bound: u16) -> Option<u64> {
    let mut declared = 0u64;
    for bit in ast.bits_of(type_) {
        // `None` and not `continue`: a bit whose position does not
        // evaluate is a bit this mask cannot name, and leaving it out
        // would make the reader drop a bit the type does declare, for a
        // reason nothing states. The caller
        // turns this into `MemberWrite::Unsupported` naming the
        // construct.
        let position = ast.effective_bit_position(*bit)?;
        if position < bound && position < 64 {
            declared |= 1u64 << position;
        }
    }
    Some(declared)
}

/// The position a collection's elements sit in.
///
/// §7.3.1.2.1.12.4 gives the elements their own behaviour —
/// `sequence<@try_construct(TRIM) String5, 10>` — and Table 17 keeps it
/// separate from the collection's own: the sequence's length overflow
/// and an element's bound overflow are two different failures, which is
/// the split OpenDDS's generated element loop implements.
fn inside(ast: &Ast, collection: Id<TypeNode>, at: Position) -> Position {
    Position {
        behaviour: ast.element_try_construct(collection, at.written),
        written: None,
    }
}

/// The writer a primitive takes.
///
/// Exhaustive, so a primitive added to the model without a writer is a
/// compile error rather than a member that silently takes another one's.
const fn writer_name(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "write_bool",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "write_u8",
        PrimitiveKind::Char => "write_char",
        // Member and element take different octets under TAO's framing,
        // so there is no positionless `write_wchar` to name here. This
        // arm is the member's; `element_writer_name` has the element's.
        PrimitiveKind::WChar => "write_wchar_member",
        PrimitiveKind::Int8 => "write_i8",
        PrimitiveKind::Int16 => "write_i16",
        PrimitiveKind::UInt16 => "write_u16",
        PrimitiveKind::Int32 => "write_i32",
        PrimitiveKind::UInt32 => "write_u32",
        PrimitiveKind::Int64 => "write_i64",
        PrimitiveKind::UInt64 => "write_u64",
        PrimitiveKind::Float32 => "write_f32",
        PrimitiveKind::Float64 => "write_f64",
        PrimitiveKind::Float128 => "write_f128",
    }
}

const fn reader_name(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "read_bool",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "read_u8",
        PrimitiveKind::Char => "read_char",
        // As `writer_name`: the member's reader.
        PrimitiveKind::WChar => "read_wchar_member",
        PrimitiveKind::Int8 => "read_i8",
        PrimitiveKind::Int16 => "read_i16",
        PrimitiveKind::UInt16 => "read_u16",
        PrimitiveKind::Int32 => "read_i32",
        PrimitiveKind::UInt32 => "read_u32",
        PrimitiveKind::Int64 => "read_i64",
        PrimitiveKind::UInt64 => "read_u64",
        PrimitiveKind::Float32 => "read_f32",
        PrimitiveKind::Float64 => "read_f64",
        PrimitiveKind::Float128 => "read_f128",
    }
}

fn call(method: &str, args: Vec<rust::Expr>) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("r"))),
        method: Ident::new(method),
        args,
    }
}

/// Bind a framing header so it can be patched once its value is written.
/// Bind an opened header, propagating the writer's refusal.
///
/// The three operations this opens — `write_pid`, `write_pid_extended`
/// and `write_emheader` — refuse a member id their header's field cannot
/// carry (XTypes 1.3 §7.2.2.4.4.4.4 for the 28 bits, §7.4.1.2.2 for the
/// reserved short ids), so each returns a `Result` and the `?` carries
/// the refusal out of the generated `encode`. Nothing here can trip it
/// today, because the plan chose the id and the form together; it is
/// there because a generated codec must not be the one place a masked
/// id reaches the wire.
fn bind_header(name: &str, method: &str, args: Vec<rust::Expr>) -> rust::Statement {
    rust::Statement::Let {
        name: Ident::new(name),
        type_: None,
        value: Some(rust::Expr::Try(Box::new(rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Path(Path::single("w"))),
            method: Ident::new(method),
            args,
        }))),
        mutable: false,
    }
}

fn call_statement(method: &str, args: Vec<rust::Expr>) -> rust::Statement {
    rust::Statement::Semi(rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("w"))),
        method: Ident::new(method),
        args,
    })
}

/// The same call, with the `?` a fallible one needs.
fn try_call_statement(method: &str, args: Vec<rust::Expr>) -> rust::Statement {
    rust::Statement::Semi(rust::Expr::Try(Box::new(rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Path(Path::single("w"))),
        method: Ident::new(method),
        args,
    })))
}

/// `ParameterRole::Member` or `ParameterRole::Optional`, as a path.
fn parameter_role(role: &str) -> rust::Expr {
    rust::Expr::Path(Path::new(&["ParameterRole", role]))
}

/// One `@optional` member's read, into the `Option<T>` field.
///
/// The mirror of [`write_optional_member`], marker for marker:
///
/// - rule (20): the BOOLEAN `<is_present>` decides;
/// - rule (19): a declared parameter length of zero is the absence
///   (§7.4.1.2), and `finish_pid` still runs either way — the parameter
///   occupies its place in the list whether or not it carries a value,
///   and skipping the close would leave the alignment origin inside it;
/// - inside a `@mutable` type: the parameter is present, because a
///   parameter list that omitted it would not have brought the reader
///   here at all.
fn read_optional_member(
    plan: &zerodds_ast_layout::plan::MemberPlan,
    name: &str,
    kind: &MemberWrite,
) -> Result<Vec<rust::Statement>, EncodeError> {
    let some = |value: rust::Expr| rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Some"))),
        args: vec![value],
    };
    let read = rust::Expr::Try(Box::new(read_for(name, kind)?));
    match plan.header {
        MemberHeader::OptionalFlag => Ok(vec![let_statement(
            name,
            rust::Expr::If {
                condition: Box::new(rust::Expr::Try(Box::new(call("read_bool", Vec::new())))),
                then: rust::Block {
                    statements: vec![rust::Statement::Tail(some(read))],
                },
                otherwise: Some(rust::Block {
                    statements: vec![rust::Statement::Tail(rust::Expr::Path(Path::single(
                        "None",
                    )))],
                }),
            },
        )]),
        MemberHeader::OptionalCdr1 { .. } => Ok(vec![
            let_statement(
                "pid",
                rust::Expr::Try(Box::new(call(
                    "read_pid_as",
                    vec![parameter_role("Optional")],
                ))),
            ),
            let_statement(
                name,
                rust::Expr::If {
                    condition: Box::new(rust::Expr::Binary {
                        op: rust::BinaryOp::Eq,
                        lhs: Box::new(rust::Expr::Field {
                            receiver: Box::new(rust::Expr::Path(Path::single("pid"))),
                            name: Ident::new("length"),
                        }),
                        rhs: Box::new(rust::Expr::Literal(Literal::int(0))),
                    }),
                    then: rust::Block {
                        statements: vec![rust::Statement::Tail(rust::Expr::Path(Path::single(
                            "None",
                        )))],
                    },
                    otherwise: Some(rust::Block {
                        statements: vec![rust::Statement::Tail(some(read))],
                    }),
                },
            ),
            rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                "finish_pid",
                vec![rust::Expr::Path(Path::single("pid"))],
            )))),
        ]),
        _ => {
            let mut out = match plan.header {
                MemberHeader::PidCdr1 { .. } => vec![let_statement(
                    "pid",
                    rust::Expr::Try(Box::new(call("read_pid", Vec::new()))),
                )],
                MemberHeader::EmHeader { .. } => vec![let_statement(
                    "emheader",
                    rust::Expr::Try(Box::new(call("read_emheader", Vec::new()))),
                )],
                _ => Vec::new(),
            };
            out.push(let_statement(name, some(read)));
            if matches!(plan.header, MemberHeader::PidCdr1 { .. }) {
                out.push(rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                    "finish_pid",
                    vec![rust::Expr::Path(Path::single("pid"))],
                )))));
            }
            Ok(out)
        }
    }
}

/// One `field: local` pair of the struct literal a decoder ends with.
///
/// Both halves go through `escape_keyword`, and that is the whole point:
/// the field is a member named in IDL and the local is the binding
/// `let_statement` made for it, so a member called `type` needs `r#type`
/// on both sides. Escaping only the field produced `r#type: type`, which
/// is a Rust syntax error — the generated file did not compile, and the
/// three call sites that built this pair by hand had each got it the
/// same way wrong.
fn field_binding(name: &str) -> (Ident, rust::Expr) {
    let escaped = rust::escape_keyword(name);
    (
        Ident::new(&escaped),
        rust::Expr::Path(Path::single(&escaped)),
    )
}

fn let_statement(name: &str, value: rust::Expr) -> rust::Statement {
    rust::Statement::Let {
        name: Ident::new(&rust::escape_keyword(name)),
        type_: None,
        value: Some(value),
        mutable: false,
    }
}

/// `#[allow(<lint>)]`.
fn allow_lint(lint: &str) -> rust::Attr {
    rust::Attr {
        path: Path::single("allow"),
        args: vec![rust::AttrArg::Word(Ident::new(lint))],
    }
}

/// `impl Default` for an aggregate, from the values XTypes 1.3 Table 9
/// and `@default` give its members.
///
/// Not `#[derive(Default)]`: Rust's default and the specification's are
/// two different things wherever a member is an enumeration — Rust has
/// none without a `#[default]` attribute, and §7.3.1.2.1.10 names the
/// first enumerator or the one `@default_literal` picks. The emitter
/// already computes the right value for every member, and this is where
/// a caller can reach it.
///
/// # Errors
/// [`EncodeError`] naming the member whose default the model cannot
/// give.
fn default_impl(ast: &Ast, type_: Id<TypeNode>) -> Result<Option<rust::Item>, EncodeError> {
    let Ok(node) = ast.type_(type_) else {
        return Ok(None);
    };
    // A bitset joins them, and for the same reason: it materialises as
    // a struct, so `Default::default()` on one is a thing a caller can
    // write and used to be `the trait bound `BS: Default` is not
    // satisfied`. A valuetype is deliberately not here — it was not
    // before — and a bitmask cannot be: `impl Default for u16` is not
    // this crate's to write.
    if !matches!(
        node.kind,
        TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Bitset { .. }
    ) {
        return Ok(None);
    }
    let from = node.module;
    // Named from inside its own module, which is the plain name.
    let self_type = rust::Type::named(Path::single(&rust::escape_keyword(&node.name)));
    let value = crate::default_rust::default_of(ast, from, type_, &node.name)?;
    Ok(Some(rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        // Where every member's Table 9 value happens to be Rust's own
        // default, clippy sees a derivable impl. It is written out on
        // purpose (see above), so the allowance is the generator's and
        // sits on the impl, not in each crate that includes the text.
        attrs: vec![allow_lint("clippy::derivable_impls")],
        generics: Vec::new(),
        trait_: Some(rust::Type::named(Path::single("Default"))),
        self_type: self_type.clone(),
        items: vec![rust::Item::Function(rust::Function {
            doc: Doc::none(),
            attrs: Vec::new(),
            visibility: Visibility::Private,
            name: Ident::new("default"),
            generics: Vec::new(),
            params: Vec::new(),
            returns: Some(rust::Type::named(Path::single("Self"))),
            body: Some(rust::Block {
                statements: vec![rust::Statement::Tail(value)],
            }),
        })],
    })))
}

/// The module a type is declared in, for naming other types relative to
/// it. A stale handle falls back to the root, where a relative path is
/// the plain name — the same thing the emitter wrote before it named
/// anything relatively.
/// `self`, then `.parent` once per structure between `type_` and the one that declares
/// `member`: the base is the first member `parent` of the derived structure (XTypes 1.3
/// §7.5.1.4.1.3).
fn holder_of(ast: &Ast, type_: Id<TypeNode>, member: NodeId) -> rust::Expr {
    let mut place = rust::Expr::Path(Path::single("self"));
    for _ in 0..crate::emit::inheritance_depth(ast, type_, member) {
        place = rust::Expr::Field {
            receiver: Box::new(place),
            name: Ident::new(crate::adapter::cst::PARENT),
        };
    }
    place
}

/// A literal of `type_` from the fields of its own members and those of its bases, each
/// level nested in the `parent` of the one above it. `rest` is what a level not every
/// member of which is named takes the others from.
fn nested_struct_literal(
    ast: &Ast,
    type_: Id<TypeNode>,
    entries: Vec<(NodeId, (Ident, rust::Expr))>,
    rest: &impl Fn() -> Option<Box<rust::Expr>>,
) -> rust::Expr {
    let deepest = entries
        .iter()
        .map(|(node, _)| crate::emit::inheritance_depth(ast, type_, *node))
        .max()
        .unwrap_or(0);
    let mut inner: Option<rust::Expr> = None;
    for depth in (0..=deepest).rev() {
        let mut fields: Vec<(Ident, rust::Expr)> = Vec::new();
        if let Some(parent) = inner.take() {
            fields.push((Ident::new(crate::adapter::cst::PARENT), parent));
        }
        for (node, field) in &entries {
            if crate::emit::inheritance_depth(ast, type_, *node) == depth {
                fields.push(field.clone());
            }
        }
        let path = if depth == 0 {
            Path::single("Self")
        } else {
            let owner = crate::emit::base_at(ast, type_, depth);
            ast.type_(owner)
                .ok()
                .and_then(|node| {
                    crate::adapter::cst::relative_path(ast, module_of(ast, type_), node).ok()
                })
                .unwrap_or_else(|| Path::single("Self"))
        };
        inner = Some(rust::Expr::StructLit {
            path,
            fields,
            base: rest(),
        });
    }
    inner.unwrap_or(rust::Expr::StructLit {
        path: Path::single("Self"),
        fields: Vec::new(),
        base: rest(),
    })
}

fn module_of(ast: &Ast, type_: Id<TypeNode>) -> Id<zerodds_ast::ModuleNode> {
    ast.type_(type_)
        .map_or_else(|_| ast.root_module(), |node| node.module)
}

fn alloc_format(error: &impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

/// Classify what the planner said: a clause of the specification, or a
/// gap of this tree.
///
/// The planner already carries the distinction in its own type —
/// `LayoutError::RecursionOutsideCollection` names IDL 4.2
/// §7.4.1.4.4.4.4 — and it was lost here, where every reason became one
/// string. Read the variant, never the message: a classification that
/// greps its own error text breaks the first time the text is reworded.
pub(crate) fn plan_refusal(error: zerodds_ast_layout::plan::LayoutError) -> EncodeError {
    match error {
        LayoutError::RecursionOutsideCollection { .. } => EncodeError::SpecRefuses {
            detail: format!("{error}"),
        },
        other => EncodeError::NoLayout {
            detail: alloc_format(&other),
        },
    }
}

/// `_ => Err(WireError::refused(r.position(), "<what>"))`: the arm a
/// generated decoder ends a match with when the octets name nothing the
/// type declares.
fn refusal_arm(what: &str) -> rust::MatchArm {
    rust::MatchArm {
        pattern: rust::Pattern::Wildcard,
        guard: None,
        body: refused_err("r", what),
    }
}

/// `Err(WireError::refused(<sink>.position(), "<what>"))`, the value a
/// generated codec returns for octets or a sample it cannot carry.
fn refused_err(sink: &str, what: &str) -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Err"))),
        args: vec![rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["WireError", "refused"]))),
            args: vec![
                rust::Expr::MethodCall {
                    receiver: Box::new(rust::Expr::Path(Path::single(sink))),
                    method: Ident::new("position"),
                    args: Vec::new(),
                },
                rust::Expr::Literal(Literal::string(what)),
            ],
        }],
    }
}
