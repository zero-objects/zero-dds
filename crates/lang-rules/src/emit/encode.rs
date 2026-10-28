// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder bodies, built from the layout plan. Spec §5, §6.2.
//!
//! One walk, emitted through a [`CodecTarget`]. Which member is written,
//! in what order, under which header and with which length code is
//! decided here and once; how that is spelled is the backend's.
//!
//! The counterpart of [`crate::encode`], which does the same for Rust
//! against the Rust CST directly and has not been brought over.
//! Every framing decision is read from `zerodds-ast-layout` and none is
//! re-derived: the plan is where the length-code policy and the XCDR2
//! alignment rule live, and a second derivation would be a second place
//! to get them wrong.
//!
//! What the emitted code does *not* delegate is the wire rule itself.
//! Alignment, the string length convention, the UTF-16 count, the BCD
//! packing, the enumerator's signed holder and the DHEADER, EMHEADER and
//! PID framing are the target runtime's — `runtime/c`, `runtime/java`
//! and the rest each implement them from the clause rather than calling
//! the Rust one. That is what makes a column a second reading of the
//! specification: where two disagree, one of them has misread it.
//!
//! ## What this walk assumes of every target
//!
//! - **A routine per type.** Rust's emitter names a writer method on the
//!   element type; C has no methods, so every type gets its own routine
//!   and collections call it by name. A target that would rather emit a
//!   method says so in [`CodecTarget::routine`].
//! - **Forward declarations are the target's business.** C needs them
//!   ahead of the bodies so a recursive type does not depend on visit
//!   order; a target with no such thing answers `None` and the loop
//!   emits only bodies.
//! - **A `switch` is an if-chain.** The union's branch is selected by
//!   comparing the discriminator against each case's labels, which every
//!   target can hold and which sidesteps the languages whose `switch`
//!   will not take a 64-bit label.

use crate::adapter::c::{ordered_types, ordered_types_from, ordered_types_from_pruning};
use crate::emit::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, Bound, CaseLabel, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_ast_layout::key::{
    ElementPadding, LayoutKey, PlCdr1HeaderPolicy, Purpose, WcharPosition, WcharWidth,
    WstringCodeUnits, WstringLengthCounts, WstringLengthPolicy, WstringUnitOrder, WstringUnitWidth,
};
use zerodds_ast_layout::key::{FixedDigitCount, MapKeyOrdering};
use zerodds_ast_layout::plan::{Extent, MemberHeader, ScalarWrite, scalar_write};
use zerodds_ast_layout::xcdr::DepCollector;
use zerodds_lang_cst::common::{Doc, Literal};

/// The one framing question the layout plan does not answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecOptions {
    /// Whether a delimited `map` also carries its entry count.
    ///
    /// XTypes 1.3 says it twice and differently: §7.4.1.1.4 defines a
    /// map by equivalence to a sequence, which has both a `DHEADER` and
    /// an `O.length`, while rule (15) of §7.4.3.5.3 writes the
    /// `DHEADER` and the pairs with no count. Both are decodable, so
    /// neither can be ruled out from the stream alone, and the caller
    /// chooses. `true` is the equivalence reading.
    pub map_length_with_delimiter: bool,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            map_length_with_delimiter: true,
        }
    }
}

/// The parameter that carries a sample's RTI PL_CDR1 step decision down
/// to a nested type's encoder.
///
/// RTI frames a sample under XCDR1 with the short parameter header until
/// its type's maximum serialized size passes a step, and from there
/// gives `PID_EXTENDED` to every member whose type is not a primitive or
/// an enumeration. The step is a property of the **sample's** type and
/// not of the type that declares a member, so a generated type cannot
/// bake the answer in: the same nested `@mutable` type is framed short
/// inside a small sample and extended inside a large one
/// (`crate::emit::encode`'s `body_name` doc and
/// `zerodds_ast_layout::key::PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`).
/// The flag is threaded at run time; the sample's root supplies it.
const EXTENDED_PARAM: &str = "zd_extended";

/// Whether the plan's PL_CDR1 header form is decided per sample, so every
/// generated encoder takes [`EXTENDED_PARAM`] and threads it to the
/// types it calls.
fn per_sample(key: LayoutKey) -> bool {
    key.pl_cdr1_headers.decided_per_sample() && key.encoding.is_v1()
}

/// The name of the encoder **body** of `name`: the function that takes
/// [`EXTENDED_PARAM`], or `name` itself when the form is fixed.
///
/// Under [`per_sample`] one type needs two functions. The body carries
/// the sample's decision as a parameter, so one type's codec can frame
/// its constructed members short in a small sample and extended in a
/// large one; the public entry under `name` bakes the decision in for
/// the sample whose root it is and calls the body. Without the split,
/// the nested `Inner` of `crate::emit::encode`'s `Inner` row could not
/// serve both its samples.
fn body_name(name: &str, key: LayoutKey) -> String {
    if per_sample(key) {
        format!("{name}_zd_extended")
    } else {
        name.to_string()
    }
}

/// A call to the encoder of a nested type: its body's name, and the
/// sample's flag when [`per_sample`] holds.
fn nested_call<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
    args: Vec<T::Expr>,
) -> T::Expr {
    nested_call_with::<T>(ast, id, key, args, T::name(EXTENDED_PARAM))
}

/// [`nested_call`] with the flag spelled by the caller.
///
/// The key holder passes the constant `false`. Under
/// [`PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`] a key
/// holder keeps the short form whatever the sample: the choice of
/// `PID_EXTENDED` for a constructed member is a data-sample decision —
/// [`LayoutKey::for_sample`] is asked for one — and the reflective
/// planner keeps a key holder short for that same reason. The data path
/// resolves the choice once at the root and hands it down; the holder
/// has no root decision to hand down and states the constant.
fn nested_call_with<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
    mut args: Vec<T::Expr>,
    flag: T::Expr,
) -> T::Expr {
    let name = body_name(&member_codec_name::<T>(ast, id, key), key);
    if per_sample(key) {
        args.push(flag);
    }
    T::call(&name, args)
}

/// Whether a sample of `root` passes RTI's short-header step, the
/// generation-time half of the per-sample decision.
///
/// The same count [`zerodds_ast_layout::key::LayoutKey::for_sample`]
/// makes: a `decided_per_sample` policy becomes
/// [`PlCdr1HeaderPolicy::ExtendedForConstructed`] when the sample's type
/// can exceed the short form's length. `for_sample` is asked rather than
/// `exceeds_short_length` directly so that the generated entry and the
/// reflective path read one answer.
fn sample_past_step(ast: &Ast, root: Id<TypeNode>, key: LayoutKey) -> Result<bool, EncodeError> {
    let resolved = key
        .for_sample(ast, root)
        .map_err(crate::encode::plan_refusal)?;
    Ok(resolved.pl_cdr1_headers == PlCdr1HeaderPolicy::ExtendedForConstructed)
}

/// The types a whole-model codec needs a public entry for: the named
/// structures, unions and value types that are not `@nested`, which are
/// what a caller can hold a sample of.
///
/// A type that is only ever a member is reached through its parent's
/// body and needs no entry; emitting one would be a `static` function
/// nothing calls, which every C consumer built with `-Werror` refuses.
/// [`codec_from`] knows its root and does not ask this.
pub(crate) fn sample_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    ast.types()
        .filter(|(_, node)| {
            !node.name.is_empty()
                && matches!(
                    node.kind,
                    TypeKind::Structure { .. }
                        | TypeKind::Union { .. }
                        | TypeKind::Valuetype { .. }
                )
                && !node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_nested)
        })
        .map(|(id, _)| id)
        .collect()
}

/// The body with `zd_extended` marked as read.
///
/// A primitive's or an enumeration's body has no parameter header and
/// never reads the flag; the C probe is built with `-Wunused-parameter
/// -Werror`, where that fails the build. `(void)zd_extended;` is valid
/// whether or not the body goes on to use it, so it is emitted
/// unconditionally rather than from a guess about which bodies read it —
/// the same shape `crate::emit::decode::with_tc_used` uses.
fn with_extended_used<T: CodecTarget>(body: T::Block) -> T::Block {
    let mut statements = T::mark_used(EXTENDED_PARAM);
    statements.extend(T::into_statements(body));
    T::block(statements)
}

/// The prefix an encoder function takes, by what the plan is for.
///
/// A key holder is a different plan over the same types — only the
/// `@key` members, big-endian — so it needs its own set of functions
/// rather than a flag threaded through the data ones.
const fn prefix(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::Data => "zd_enc_",
        Purpose::Key => "zd_kenc_",
    }
}

/// The encoder a member or element of `member_type` is written by,
/// under the key its enclosing body was generated with.
///
/// Not `codec_name(ast, member_type, key.purpose)`. §7.6.8 Step 1
/// removes the non-key members of a nested aggregate only "if there are
/// any key members", and one dialect reads the consequence differently:
/// under [`KeyHolderNesting::KeylessNestedWhole`] — RTI under XCDR1 —
/// the subtree below a keyless aggregate stops being filtered at all
/// and is taken as the data has it. `keyhash::member_key` is where that
/// rule lives; asking it here is what makes the holder call
/// `zd_enc_Middle` rather than `zd_kenc_Middle` for a keyless `Middle`,
/// and everything under it follows because the key threads down.
///
/// Inert outside a key holder: the rule only ever turns `Purpose::Key`
/// into `Purpose::Data`, so a payload run gets the name it always had.
fn member_codec_name<T: CodecTarget>(
    ast: &Ast,
    member_type: Id<TypeNode>,
    key: LayoutKey,
) -> String {
    codec_name::<T>(
        ast,
        member_type,
        zerodds_ast_layout::keyhash::member_key(ast, member_type, key).purpose,
    )
}

/// The encoder function name for a type.
#[must_use]
pub fn codec_name<T: CodecTarget>(ast: &Ast, id: Id<TypeNode>, purpose: Purpose) -> String {
    let head = prefix(purpose);
    let name = match ast.type_(id).map(|node| node.kind.clone()) {
        // Named by the IDL kind and not by the target's spelling:
        // `double` and `long double` share a C type and do not share a
        // wire form, and a name taken from the target type would give
        // them one function.
        Ok(TypeKind::Primitive(kind)) => {
            format!("{head}p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("{head}{}", T::type_name(ast, id)),
    };
    // The published spelling, once and here. See
    // [`CodecTarget::published_name`].
    T::published_name(name)
}

/// Build the encoder for every type in the model, under one plan.
///
/// Under `per_sample` the named non-`@nested` aggregates additionally
/// get a public entry that bakes their own sample decision in; every
/// other type is reached through the body of the type that declares it.
///
/// # Errors
/// [`EncodeError`] when the layout cannot be planned or a construct has
/// no C writer — never a body that leaves a member out.
pub fn codec<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    codec_over::<T>(ast, &ordered_types(ast), key, options, &sample_types(ast))
}

/// [`codec`] without the types in `excluded`, and with what the types in
/// `also_reaching` reach.
///
/// For a model whose declarations of some types are provided elsewhere —
/// DDS-RPC 1.0 §7.5.1.1.1's common types come with the runtime — and whose
/// request and reply types (§7.3.1.4) are encoded by a hand of the RPC
/// binding that writes the header the runtime's way.
///
/// # Errors
/// As [`codec`].
pub fn codec_excluding<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    excluded: &[Id<TypeNode>],
    also_reaching: &[Id<TypeNode>],
) -> Result<Vec<T::Decl>, EncodeError> {
    if excluded.is_empty() {
        return codec::<T>(ast, key, options);
    }
    let (types, entries) = kept_after_excluding(ast, excluded, also_reaching);
    codec_over::<T>(ast, &types, key, options, &entries)
}

/// The types, and the sample types among them, that remain once `excluded`
/// is left out: what the other sample types reach. A helper only an
/// excluded type needed is a `static` function nothing calls.
pub(crate) fn kept_after_excluding(
    ast: &Ast,
    excluded: &[Id<TypeNode>],
    also_reaching: &[Id<TypeNode>],
) -> (Vec<Id<TypeNode>>, Vec<Id<TypeNode>>) {
    let mut entries: Vec<Id<TypeNode>> = sample_types(ast)
        .into_iter()
        .filter(|id| !excluded.contains(id))
        .collect();
    for id in also_reaching {
        if !entries.contains(id) {
            entries.push(*id);
        }
    }
    let types = ordered_types_from_pruning(ast, &entries, excluded);
    (types, entries)
}

/// The encoder for the types one root reaches, and no others.
///
/// What a probe or a topic's translation unit wants: an encoder for a
/// type the root cannot reach is a `static` function nothing calls, and
/// a generated file that warns is one whose warnings stop being read.
/// Under `per_sample` the root gets the public entry and every type it
/// reaches gets a body.
///
/// # Errors
/// As [`codec`].
pub fn codec_from<T: CodecTarget>(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    codec_over::<T>(
        ast,
        &ordered_types_from(ast, &[root]),
        key,
        options,
        &[root],
    )
}

/// The encoder for the types several roots reach, each function once.
///
/// What a translation unit that encodes members of several types wants:
/// one call per root would define the functions the roots share twice.
///
/// # Errors
/// As [`codec`].
pub fn codec_from_many<T: CodecTarget>(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    codec_over::<T>(ast, &ordered_types_from(ast, roots), key, options, roots)
}

/// [`codec_from_many`] without the types in `excluded`, whose codecs are
/// written by hand elsewhere (DDS-RPC 1.0 §7.5.1.1.1's common types come
/// with the runtime in a shape of their own).
///
/// # Errors
/// As [`codec`].
pub fn codec_from_many_excluding<T: CodecTarget>(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
    excluded: &[Id<TypeNode>],
) -> Result<Vec<T::Decl>, EncodeError> {
    let types = ordered_types_from_pruning(ast, roots, excluded);
    codec_over::<T>(ast, &types, key, options, roots)
}

fn codec_over<T: CodecTarget>(
    ast: &Ast,
    types: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
    entries: &[Id<TypeNode>],
) -> Result<Vec<T::Decl>, EncodeError> {
    crate::encode::axes_the_codecs_carry_for::<T>(key)?;
    let mut prototypes = Vec::new();
    let mut bodies = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    let framed = per_sample(key);
    let uncalled = called_only_in_bulk(
        ast,
        types,
        entries,
        T::bulk_octets_write("v", T::name("zd_n")).is_some(),
    );

    for id in types.iter().copied() {
        if uncalled.contains(&id) {
            continue;
        }
        let name = codec_name::<T>(ast, id, key.purpose);
        if emitted.contains(&name) {
            continue;
        }
        let body = body_name(&name, key);
        let Some(statements) = body_of::<T>(ast, id, key, options, &body)? else {
            continue;
        };
        emitted.push(name.clone());
        // A target with no forward declarations answers `None` to the
        // prototype and the loop emits only the body.
        if !framed {
            prototypes.extend(function::<T>(ast, id, key, &name, None, false));
            bodies.extend(function::<T>(ast, id, key, &name, Some(statements), false));
            continue;
        }
        prototypes.extend(function::<T>(ast, id, key, &body, None, true));
        bodies.extend(function::<T>(ast, id, key, &body, Some(statements), true));
        // The public entry, for the samples whose root this type is:
        // it bakes the type's own step decision in and hands it to the
        // body, which threads it to every type the body calls.
        if entries.contains(&id) {
            let past = sample_past_step(ast, id, key)?;
            let call = T::perform(T::call(
                &body,
                vec![T::name("w"), T::name("v"), T::literal(Literal::Bool(past))],
            ));
            let entry = T::block(vec![call]);
            prototypes.extend(entry_function::<T>(ast, id, &name, None));
            bodies.extend(entry_function::<T>(ast, id, &name, Some(entry)));
        }
    }

    prototypes.extend(bodies);
    Ok(prototypes)
}

/// The key holder for a keyed type, and nothing for one with no `@key`.
///
/// The holder is the `@key` members alone, in member-id order, and it is
/// big-endian whatever the sample is: two peers that disagree about
/// either compute different hashes for the same instance.
/// DDSI-RTPS 2.5 §9.6.3.8, XTypes 1.3 §7.6.8.
///
/// `key` is the holder's own layout key, and it is taken rather than
/// assembled here because it carries the two things this function
/// cannot work out for itself: `key_holder_nesting`, which decides how
/// far below a keyless aggregate the key set reaches, and the purpose
/// each member's encoder is named under.
///
/// # Errors
/// [`EncodeError::NoLayout`] when the key set cannot be worked out.
pub fn key_holder<T: CodecTarget>(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Option<T::Decl>, EncodeError> {
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        root,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(crate::encode::plan_refusal)?;
    if keys.is_empty() {
        // §7.6.8 Step 1 over a union keyed by its discriminator: the
        // branches are not keys, so the holder is the discriminator alone,
        // as `zerodds_ast_layout::xcdr` plans it. `key_members_with` walks
        // members and answers the empty list for such a union.
        let Some(discriminator) = zerodds_ast_layout::keyhash::union_key_discriminator(ast, root)
        else {
            return Ok(None);
        };
        let statements = vec![T::perform(nested_call_with::<T>(
            ast,
            discriminator,
            key,
            vec![
                T::name("w"),
                argument::<T>(ast, discriminator, T::discriminant("v")),
            ],
            T::literal(Literal::Bool(false)),
        ))];
        return Ok(key_holder_routine::<T>(
            ast,
            root,
            &[
                "The key holder: the discriminator of a union keyed by it.",
                "Big-endian whatever the sample is — DDSI-RTPS 2.5 9.6.3.8.",
            ],
            statements,
        ));
    }

    let mut statements = Vec::new();
    for node in &keys {
        // A key member of a nested aggregate is written by that
        // aggregate's own key encoder, which the nested plan already
        // filters; only the root's own members are named here.
        let Some((name, member_type)) = member_by_node(ast, root, *node) else {
            continue;
        };
        let reach = super::reach::<T>(ast, root, *node, &name);
        statements.push(T::perform(nested_call_with::<T>(
            ast,
            member_type,
            key,
            vec![
                T::name("w"),
                argument::<T>(ast, member_type, arrow::<T>("v", &reach.field)),
            ],
            T::literal(Literal::Bool(false)),
        )));
    }

    Ok(key_holder_routine::<T>(
        ast,
        root,
        &[
            "The key holder: the @key members alone, in member-id order.",
            "Big-endian whatever the sample is — DDSI-RTPS 2.5 9.6.3.8.",
        ],
        statements,
    ))
}

/// The routine of a key holder of `root` with the body `statements`.
fn key_holder_routine<T: CodecTarget>(
    ast: &Ast,
    root: Id<TypeNode>,
    doc: &[&str],
    statements: Vec<T::Stmt>,
) -> Option<T::Decl> {
    T::routine(Routine {
        doc: Doc::lines(doc),
        name: format!("zd_keyholder_{}", T::type_name(ast, root)),
        // The sample's own type, as every codec routine takes it. The
        // flat `type_name` is a name for routines, not a type: in a
        // module it read `m_S` where C++ declares `::m::S`, and the
        // holder did not compile in C++, C# or TypeScript.
        sample: T::value_type(ast, root),
        held: Handoff::Value,
        writing: true,
        // Always emitted and always called; only the `Char16` codec's
        // caller depends on a setting. See `maybe_unused`.
        tolerate_unused: false,
        extra: Vec::new(),
        body: Some(T::block(statements)),
    })
}

/// The codec routine for a type, with or without a
/// body.
///
/// `framed` says this is a body under [`per_sample`]: it takes
/// [`EXTENDED_PARAM`] after the sample and marks it read, so a target
/// that warns on an unread parameter still compiles.
fn function<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
    name: &str,
    body: Option<T::Block>,
    framed: bool,
) -> Option<T::Decl> {
    let _ = key.purpose;
    T::routine(Routine {
        doc: Doc::none(),
        name: name.to_string(),
        sample: T::value_type(ast, id),
        held: if is_array(ast, id, 0) {
            Handoff::Array
        } else {
            Handoff::Value
        },
        writing: true,
        // A `Char16` whose element form is written inline at the call
        // site has no caller left when the type graph reaches one only
        // through a collection. Whether this function is used is then a
        // property of the dialect, not of the model, and the generated
        // code is compiled with an uncalled function as an error.
        tolerate_unused: maybe_unused(ast, id, key),
        extra: if framed {
            vec![(T::scalar(Scalar::Bool), EXTENDED_PARAM.to_string())]
        } else {
            Vec::new()
        },
        body: if framed {
            body.map(with_extended_used::<T>)
        } else {
            body
        },
    })
}

/// The public entry of a type under [`per_sample`]: the sample's own
/// step decision baked in, handed to the body.
///
/// The entry takes no [`EXTENDED_PARAM`] because the sample whose root
/// this type is decides once, at generation time; every nested type is
/// reached through a body that takes the decision at run time.
fn entry_function<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    name: &str,
    body: Option<T::Block>,
) -> Option<T::Decl> {
    T::routine(Routine {
        doc: Doc::none(),
        name: name.to_string(),
        sample: T::value_type(ast, id),
        held: if is_array(ast, id, 0) {
            Handoff::Array
        } else {
            Handoff::Value
        },
        writing: true,
        tolerate_unused: false,
        extra: Vec::new(),
        body,
    })
}

/// Whether the codec generated for `id` may legitimately have no caller.
///
/// True only for a `Char16` under a framing whose member and element
/// forms differ, which is TAO's alone. Everywhere else an uncalled
/// generated codec is a generator defect and the warning must stay a
/// warning.
pub(crate) fn maybe_unused(ast: &Ast, id: Id<TypeNode>, key: LayoutKey) -> bool {
    // The RPC binding of an interface or of a pair of types calls the codecs
    // of the types it carries and of nothing else (DDS-RPC 1.0 §7.5.1.1,
    // §7.3.1.4); a type declared beside it keeps its codec and has no caller
    // in the header.
    if ast.interfaces().next().is_some() || crate::emit::type_pair_cpp::has_pair_types(ast) {
        return true;
    }
    // A base structure is reached through the members of the structure derived from it
    // (`v->parent.m`), and its own codec has no caller where nothing else holds it.
    if is_a_base(ast, id) {
        return true;
    }
    key.wchar_framing.has_length_octet(WcharPosition::Member)
        != key.wchar_framing.has_length_octet(WcharPosition::Element)
        && matches!(resolve_primitive(ast, id, 0), Some(PrimitiveKind::WChar))
}

/// Whether another structure in the model derives from `id`.
fn is_a_base(ast: &Ast, id: Id<TypeNode>) -> bool {
    ast.all_nodes().any(|any| match any {
        zerodds_ast::AnyId::Type(other, _) => {
            crate::adapter::c::struct_base(ast, other) == Some(id)
        }
        _ => false,
    })
}

/// Whether a type is a C array once typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by the
/// `depth > 64` check, past which the answer is `false` — the
/// handoff form a non-array takes)
fn is_array(ast: &Ast, id: Id<TypeNode>, depth: usize) -> bool {
    if depth > 64 {
        return false;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Array { .. }) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_array(ast, target, depth + 1)),
        _ => false,
    }
}

/// How a value of `id` is handed to its encoder: the array itself, or
/// the address of anything else.
fn argument<T: CodecTarget>(ast: &Ast, id: Id<TypeNode>, value: T::Expr) -> T::Expr {
    if is_array(ast, id, 0) {
        value
    } else {
        T::handoff(value, Handoff::Value)
    }
}

#[allow(clippy::too_many_lines)]
fn body_of<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
    options: CodecOptions,
    own_name: &str,
) -> Result<Option<T::Block>, EncodeError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(None);
    };
    let statements = match &node.kind {
        TypeKind::Primitive(_) => {
            let Some(write) = scalar_write(ast, id, key, WcharPosition::Member) else {
                return Err(EncodeError::NoWriter {
                    member: T::type_name(ast, id),
                    kind: "primitive the layout plan has no scalar write for",
                });
            };
            vec![primitive_write_at::<T>(write, deref::<T>("v"))]
        }

        // A bounded `string` or `wstring` writes through
        // `bounded_string_write`, which calls the runtime's own
        // `write_string_bounded`/`write_wstring_bounded` and lets it
        // refuse an over-bound value — the same refusal
        // `crates/wire::Writer::write_string_bounded` raises for Rust,
        // and the same reason `write_string_bounded`'s own remarks give:
        // the bound is a promise about the source and not a difference
        // on the wire, so writing an over-bound value as if it fit, or
        // cutting it at a character boundary, would put a sample on the
        // wire nobody wrote.
        TypeKind::String { bound, .. } => {
            let Some(write) = scalar_write(ast, id, key, WcharPosition::Member) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "value the layout plan has no scalar write for",
                });
            };
            vec![match bound_limit(ast, bound) {
                Some(limit) => bounded_string_write::<T>(own_name, write, deref::<T>("v"), limit)?,
                None => primitive_write_at::<T>(write, deref::<T>("v")),
            }]
        }

        TypeKind::Fixed { .. } => {
            let Some(write) = scalar_write(ast, id, key, WcharPosition::Member) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "value the layout plan has no scalar write for",
                });
            };
            vec![primitive_write_at::<T>(write, deref::<T>("v"))]
        }

        // Two axes on one construct, and they are separate questions:
        // `enumeration_bit_bound` says how wide the holder is and
        // `enumerator_wire_value` says which integer goes into it. The
        // bitmask arm below is a different clause that neither reaches.
        TypeKind::Enumeration { .. } => enumeration_write::<T>(ast, id, key)?,

        TypeKind::Bitmask { .. } => {
            let Some(write) = scalar_write(ast, id, key, WcharPosition::Member) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "bitmask the layout plan has no scalar write for",
                });
            };
            vec![primitive_write_at::<T>(write, deref::<T>("v"))]
        }

        TypeKind::Bitset { .. } => {
            // The whole chain, base-first: the planner sizes the anchor
            // over it and the one implementation that parses an
            // inherited bitset packs it that way.
            // `docs/refused-constructs.md` has the measurement.
            let fields = &ast.all_bitfields_of(id);
            let mut deps = DepCollector::default();
            let layout = zerodds_ast_layout::xcdr::plan(ast, id, key, &mut deps)
                .map_err(crate::encode::plan_refusal)?;
            let Extent::Sized(anchor) = layout.extent else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "bitset whose anchor the planner does not size",
                });
            };
            let mut statements = vec![T::declare(
                T::scalar(Scalar::UInt64),
                "bits",
                Some(int::<T>(0)),
            )];
            let mut position: u32 = 0;
            for field in fields {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let width = u32::from(field_node.bits);
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(T::perform(T::binary(
                        "=",
                        T::name("bits"),
                        T::binary(
                            "|",
                            T::name("bits"),
                            call_expr::<T>(
                                "bitfield",
                                vec![
                                    cast::<T>(
                                        T::scalar(Scalar::UInt64),
                                        arrow::<T>("v", &T::escape(field_name)),
                                    ),
                                    int::<T>(i128::from(width)),
                                    int::<T>(i128::from(position)),
                                ],
                            ),
                        ),
                    )));
                }
                position = position.saturating_add(width);
            }
            statements.push(call::<T>(
                "write_bitset",
                vec![int::<T>(i128::from(anchor)), T::name("bits")],
            ));
            statements
        }

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "typedef with an unresolved target",
                });
            };
            // A typedef is transparent on the wire, and the C typedef
            // makes the two the same type, so the value goes straight
            // through.
            let callee = body_name(&member_codec_name::<T>(ast, target, key), key);
            // Unless the alias and its target are one encoder. A target
            // that folds a `typedef` into the type it names gives the
            // two the same encoder name — Zig's `pub const NodeRef =
            // Node;` is the alias, so `Zig::type_name` answers `m_Node`
            // for both — and a forwarder to itself is not a forwarder:
            // `recursive-typedef-sequence` built
            // `fn zd_enc_m_Node(..) { try zd_enc_m_Node(..); }` and the
            // probe took SIGSEGV on the stack it ran out of. `None`
            // leaves the name unclaimed and the aliased type emits its
            // own body under it.
            if callee == own_name {
                return Ok(None);
            }
            let mut args = vec![T::name("w"), T::name("v")];
            if per_sample(key) {
                args.push(T::name(EXTENDED_PARAM));
            }
            vec![T::perform(T::call(&callee, args))]
        }

        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "sequence with an unresolved element",
                });
            };
            let delimited = zerodds_ast_layout::collection_is_delimited(ast, element, key);
            // Checked before the DHEADER, and not after: `sequence<T,
            // N>` bounds the count `O.length` puts on the wire, and a
            // sender that wrote more elements than its own declaration
            // allows has written a value of a type nobody declared.
            // `crates/wire::Writer::check_sequence_bound` checks the
            // same thing at the same point, for the same reason.
            let mut statements = Vec::new();
            if let Some(limit) = bound_limit(ast, bound) {
                statements.push(bound_check::<T>(
                    "check_sequence_bound",
                    T::count_of("v", None),
                    limit,
                ));
            }
            statements.push(dheader_declaration::<T>(delimited));
            statements.push(call::<T>("write_u32", vec![T::count_of("v", None)]));
            if let Some(bulk) = bulk_write::<T>(ast, element, T::count_of("v", None)) {
                statements.push(bulk);
                statements.push(finish_dheader::<T>());
                return Ok(Some(T::block(statements)));
            }
            // A `sequence` element of a non-`@final` type carries its own
            // octet length under
            // `SequenceElementDelimiter::BeforeEachNonFinalElement`, and
            // every element counts its alignment from its own first octet
            // under `ElementPadding::Packed`. The order is the reference's:
            // the origin is moved first, then the element's length, then
            // the element, and both are undone in reverse.
            let packed = key.element_padding == ElementPadding::Packed;
            let carries_length = sequence_element_carries_length(ast, element, key)?;
            let at = T::index(T::elements("v"), T::name("i"));
            let mut body: Vec<T::Stmt> = Vec::new();
            if packed {
                body.push(begin_packed::<T>());
            }
            if carries_length {
                body.push(element_length_declaration::<T>());
            }
            body.push(
                element_write::<T>(ast, element, key, at.clone()).unwrap_or_else(|| {
                    T::perform(nested_call::<T>(
                        ast,
                        element,
                        key,
                        vec![T::name("w"), argument::<T>(ast, element, at)],
                    ))
                }),
            );
            if carries_length {
                body.push(finish_element_length::<T>());
            }
            if packed {
                body.push(finish_packed::<T>());
            }
            statements.push(T::count_up("i", T::count_of("v", None), T::block(body)));
            statements.push(finish_dheader::<T>());
            statements
        }

        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "array with an unresolved element",
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length = ast.eval_integral(*dim).map_err(|_| EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "array whose length does not evaluate",
                })?;
                lengths.push(length);
            }
            let delimited = zerodds_ast_layout::array_is_delimited(ast, element, key);
            // §7.4.3.5 treats a multi-dimensional array as one flat run
            // with the first index varying most slowly, which is what
            // nested loops in written order already are.
            let mut access = T::name("v");
            for level in 0..lengths.len() {
                access = T::index(access, T::name(&format!("i{level}")));
            }
            if let [length] = lengths.as_slice() {
                if let Some(bulk) = bulk_write::<T>(ast, element, int::<T>(*length)) {
                    return Ok(Some(T::block(vec![
                        dheader_declaration::<T>(delimited),
                        bulk,
                        finish_dheader::<T>(),
                    ])));
                }
            }
            let mut inner = T::block(vec![
                element_write::<T>(ast, element, key, access.clone()).unwrap_or_else(|| {
                    T::perform(nested_call::<T>(
                        ast,
                        element,
                        key,
                        vec![T::name("w"), argument::<T>(ast, element, access)],
                    ))
                }),
            ]);
            // Under `ElementPadding::Packed` each element of every
            // dimension is its own alignment block, as in the reference's
            // `write_array_dimension`.
            let packed = key.element_padding == ElementPadding::Packed;
            for (level, length) in lengths.iter().enumerate().rev() {
                let index = format!("i{level}");
                let mut body: Vec<T::Stmt> = Vec::new();
                if packed {
                    body.push(begin_packed::<T>());
                }
                body.extend(T::into_statements(inner));
                if packed {
                    body.push(finish_packed::<T>());
                }
                inner = T::block(vec![T::count_up(&index, int::<T>(*length), T::block(body))]);
            }
            let mut statements = vec![dheader_declaration::<T>(delimited)];
            statements.extend(T::into_statements(inner));
            statements.push(finish_dheader::<T>());
            statements
        }

        TypeKind::Map {
            key: k,
            value,
            bound,
        } => {
            let (Some(k), Some(value)) = (k.resolved(), value.resolved()) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "map with an unresolved key or value",
                });
            };
            let mut deps = DepCollector::default();
            let layout = zerodds_ast_layout::xcdr::plan(ast, id, key, &mut deps)
                .map_err(crate::encode::plan_refusal)?;
            let entry = T::map_entry_name(ast, id);
            let _ = &entry;
            // The order the entries go out in is a setting, and a key
            // type the crossing has no order for is a refusal rather
            // than a quiet fallback to declaration order.
            let Ok(ordering) = zerodds_ast_layout::plan::map_entry_ordering(ast, id, key) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "a map whose key type has no value order, under \
                           --map-entries ascending-key",
                });
            };
            // Checked before the DHEADER, as the sequence's own bound
            // is: `map<K, V, N>` bounds the **entry** count, and
            // `crates/wire::Writer::check_map_bound` checks it at the
            // same point and names it in entries and not in halves, for
            // the reason its own remarks give.
            let mut statements = Vec::new();
            if let Some(limit) = bound_limit(ast, bound) {
                statements.push(bound_check::<T>(
                    "check_map_bound",
                    T::count_of("v", None),
                    limit,
                ));
            }
            statements.push(dheader_declaration::<T>(layout.needs_dheader));
            if !layout.needs_dheader || options.map_length_with_delimiter {
                statements.push(call::<T>("write_u32", vec![T::count_of("v", None)]));
            }
            let element = |key: bool| T::entry_half(T::index(T::elements("v"), T::name("i")), key);
            let pair = T::block(vec![
                T::perform(nested_call::<T>(
                    ast,
                    k,
                    key,
                    vec![T::name("w"), argument::<T>(ast, k, element(true))],
                )),
                T::perform(nested_call::<T>(
                    ast,
                    value,
                    key,
                    vec![T::name("w"), argument::<T>(ast, value, element(false))],
                )),
            ]);
            statements.push(match ordering {
                None => over_the_entries::<T>("i", pair),
                Some(ordering) => ascending_by_key::<T>(ast, k, ordering, pair),
            });
            statements.push(finish_dheader::<T>());
            statements
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            aggregate_body::<T>(ast, id, key)?
        }

        TypeKind::Union { discriminator, .. } => {
            union_body::<T>(ast, id, key, discriminator.resolved(), own_name)?
        }

        // No wire form at all — `zerodds_ast_layout` refuses to plan
        // one — so there is nothing to emit and nothing is claimed.
        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(None);
        }
        TypeKind::ValueBox { .. } => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "boxed value, whose presence flag has no settled wire form",
            });
        }
        // Refused by name rather than passed over. `any` is in the model
        // because IDL 4.2 has it; DDS-XTypes has no `any` and no
        // encoding for one, and only CORBA CDR writes it — as a TypeCode
        // ahead of the value. Returning `Ok(None)` here would generate a
        // codec that silently omitted the member.
        TypeKind::Any => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "`any`, which DDS-XTypes has no encoding for; only CORBA CDR writes one, \
                       as a TypeCode ahead of the value",
            });
        }
        TypeKind::ValueBase => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "`ValueBase`, which DDS-XTypes has no encoding for; only CORBA CDR writes \
                       one, as a chunked value with its repository id",
            });
        }
    };
    Ok(Some(T::block(statements)))
}

fn aggregate_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut deps = DepCollector::default();
    let layout = zerodds_ast_layout::xcdr::plan(ast, id, key, &mut deps)
        .map_err(crate::encode::plan_refusal)?;

    // `ElementPadding::Packed`: an aggregate is its own alignment block,
    // so its members count from its own first octet and it is not padded
    // up to its own alignment before it starts. Measured on
    // `typedef-of-struct`, whose second `Cell` puts its `long` at offset
    // six where the clause puts it at eight. `saved` is the whole body's,
    // as in the reference's `write_aggregate`.
    let packed = key.element_padding == ElementPadding::Packed;
    let mut statements = Vec::new();
    if packed {
        statements.push(begin_packed::<T>());
    }
    statements.push(dheader_declaration::<T>(layout.needs_dheader));
    for (index, member) in layout.members.iter().enumerate() {
        let Some((name, member_type)) = member_by_node(ast, id, member.member) else {
            continue;
        };
        let reach = super::reach::<T>(ast, id, member.member, &name);
        let field = reach.field.clone();
        let node = ast
            .all_members_of(id)
            .into_iter()
            .filter_map(|m| ast.member(m).ok())
            .find(|m| m.node_id == member.member);
        let optional = node.as_ref().is_some_and(|m| m.facets.is_optional());
        // Table 21: `@external` changes no octet, only where the
        // binding keeps the value — which is exactly what the seam it
        // routes through decides.
        let external = node
            .as_ref()
            .is_some_and(|m| super::reached_externally::<T>(ast, m));

        // Whether the member's value has room inside `id` at all. A
        // column whose member is the value itself has none where the
        // member's type reaches `id` back over inline edges, and the
        // seam is where it says what it keeps instead.
        let indirect =
            super::behind_pointer::<T>(ast, reach.holder, member_type, external, optional);
        // And whether the column keeps the member behind a reference of
        // its own making. `CodecTarget::holds_by_reference`.
        let by_reference =
            T::holds_by_reference(ast, reach.holder, member_type, optional, external);

        // XTypes 1.3 §7.5.1.2.3.1.1 and §7.5.1.2.3.2.1: an `@external`
        // member that is not `@optional` may not be unset at the time of
        // a write. Only the columns that have an unset state say
        // anything here.
        if external && !optional && !by_reference {
            statements.extend(T::refuse_unset_external(T::external_present("v", &field)));
        }

        statements.extend(framed_member::<T>(
            ast,
            member,
            index,
            member_type,
            &field,
            key,
            optional,
            external,
            indirect,
            by_reference,
        )?);
    }
    if layout.needs_sentinel {
        statements.push(call::<T>("write_sentinel", Vec::new()));
    }
    statements.push(finish_dheader::<T>());
    if packed {
        statements.push(finish_packed::<T>());
    }
    Ok(statements)
}

/// One member with the header the plan gave it.
///
/// `optional` is the model's answer and the header is the plan's, and
/// the two together decide what an absent member looks like. A
/// `@mutable` member that is absent contributes nothing — the parameter
/// list is what says which members are there. An `OPT_FMEMBER` that is
/// absent contributes its marker: a `<is_present>` of zero under XCDR2
/// (rule 20) or a parameter header declaring a length of zero under
/// XCDR1 (§7.4.1.2). All three implementations write both.
#[allow(clippy::too_many_arguments)]
fn framed_member<T: CodecTarget>(
    ast: &Ast,
    member: &zerodds_ast_layout::plan::MemberPlan,
    index: usize,
    member_type: Id<TypeNode>,
    field: &str,
    key: LayoutKey,
    optional: bool,
    external: bool,
    indirect: bool,
    by_reference: bool,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let present = if by_reference {
        T::reference_present("v", field)
    } else if external {
        T::external_present("v", field)
    } else {
        T::present("v", field)
    };
    /// A member written only when the generated `_has_` flag is set.
    fn guarded<B: CodecTarget>(present: B::Expr, statements: Vec<B::Stmt>) -> B::Stmt {
        B::when(present, B::block(statements), None)
    }
    // An `@optional` member is reached through the target's own
    // accessor: C's is a plain `T` beside a flag and C++'s is a
    // `std::optional<T>` the codec cannot take whole. An `@external`
    // one is reached through its own seam — an indirection in the
    // column that keeps one, the member itself everywhere else — and
    // `@optional @external` is the external form: the indirection
    // already carries the absence.
    let held = if by_reference {
        T::reference_value(ast, member_type, "v", field)
    } else if external {
        T::external_value(ast, member_type, "v", field, optional)
    } else if optional {
        T::optional_value(ast, member_type, "v", field)
    } else {
        arrow::<T>("v", field)
    };
    // And through the indirection where the column had to put one:
    // `member_held_inline_closes_a_cycle` said the value has no room in
    // its own aggregate, so what the accessor above reaches is the
    // indirection and not the value.
    let held = if indirect && !by_reference {
        T::behind_indirection(ast, member_type, held)
    } else {
        held
    };
    let value = T::perform(nested_call::<T>(
        ast,
        member_type,
        key,
        vec![T::name("w"), argument::<T>(ast, member_type, held)],
    ));

    // A distinct name per member: two headers at the same block scope
    // cannot share one, and the plan may give every member a header.
    let handle = format!("h{index}");
    Ok(match member.header {
        MemberHeader::NoHeader => vec![value],
        MemberHeader::PidCdr1 {
            pid,
            extended,
            must_understand,
        } => {
            let mut body = open_pid_header::<T>(
                ast,
                key,
                &handle,
                pid,
                member.member_id,
                extended,
                Constant::ParameterMember,
                must_understand,
                member_type,
            )?;
            body.push(value);
            body.push(call::<T>("finish_pid", vec![T::name(&handle)]));
            if optional {
                vec![guarded::<T>(present, body)]
            } else {
                body
            }
        }
        MemberHeader::EmHeader {
            lc,
            must_understand,
        } => {
            let body = vec![
                T::declare(
                    T::handle(Handle::EmHeader),
                    &handle,
                    Some(call_expr::<T>(
                        "write_emheader",
                        vec![
                            int::<T>(i128::from(member.member_id)),
                            int::<T>(i128::from(lc.bits())),
                            T::literal(Literal::Bool(must_understand)),
                        ],
                    )),
                ),
                value,
                call::<T>("finish_emheader", vec![T::name(&handle)]),
            ];
            if optional {
                vec![guarded::<T>(present, body)]
            } else {
                body
            }
        }
        // Rule (19) and §7.4.1.2: the header goes out either way, and a
        // declared length of zero is what says the member did not.
        MemberHeader::OptionalCdr1 {
            pid,
            extended,
            must_understand,
        } => {
            let mut body = open_pid_header::<T>(
                ast,
                key,
                &handle,
                pid,
                member.member_id,
                extended,
                Constant::ParameterOptional,
                must_understand,
                member_type,
            )?;
            body.push(guarded::<T>(present, vec![value]));
            body.push(call::<T>("finish_pid", vec![T::name(&handle)]));
            body
        }
        // Rule (20), §7.4.3.5.2: the BOOLEAN `<is_present>`, then the
        // value behind it.
        MemberHeader::OptionalFlag => vec![
            call::<T>("write_bool", vec![present.clone()]),
            guarded::<T>(present, vec![value]),
        ],
    })
}

fn union_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
    discriminator: Option<Id<TypeNode>>,
    own_name: &str,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let Some(discriminator) = discriminator else {
        return Err(EncodeError::NoWriter {
            member: own_name.to_string(),
            kind: "union with an unresolved discriminator",
        });
    };
    let mut deps = DepCollector::default();
    let layout = zerodds_ast_layout::xcdr::plan(ast, id, key, &mut deps)
        .map_err(crate::encode::plan_refusal)?;
    // The one crossing [`UnionLabelMatch`] has. Under the default it is
    // false for every discriminator and the arm chain below is the
    // clause's, unchanged.
    let suppress_negative = zerodds_ast_layout::plan::union_negative_selects_nothing(ast, id, key);

    // `ElementPadding::Packed`: a union is its own alignment block too.
    // `annotation-external-union-branch` records one that starts where
    // the previous member ended.
    let packed = key.element_padding == ElementPadding::Packed;
    let mut statements = Vec::new();
    if packed {
        statements.push(begin_packed::<T>());
    }
    statements.push(dheader_declaration::<T>(layout.needs_dheader));
    let write_discriminator = T::perform(nested_call::<T>(
        ast,
        discriminator,
        key,
        vec![
            T::name("w"),
            argument::<T>(ast, discriminator, T::discriminant("v")),
        ],
    ));
    // Rules (27) and (28): a `@mutable` union frames its discriminator
    // as an `MMEMBER` at member id 0.
    statements.extend(mmember::<T>(
        ast,
        key,
        layout.discriminator,
        discriminator,
        0,
        "hd",
        write_discriminator,
    )?);

    // The branch, selected by comparing the discriminator against each
    // case's labels. `default:` is the else of the chain, so a value
    // that matches no label and has no default writes nothing after the
    // discriminator — which is what the type says.
    let mut arms: Vec<(Option<T::Expr>, T::Block)> = Vec::new();
    let mut fallback: Option<T::Block> = None;
    // The labels the setting says select nothing, with the member each
    // one would have selected: the overlay below still needs them.
    let mut suppressed: Vec<(i128, Id<TypeNode>, String, bool)> = Vec::new();
    let mut default_case: Option<(Id<TypeNode>, NodeId)> = None;
    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        let Ok(member) = ast.member(case_node.member) else {
            continue;
        };
        let Some(member_type) = member.type_.resolved() else {
            return Err(EncodeError::NoWriter {
                member: member.name.clone(),
                kind: "union case with an unresolved type",
            });
        };
        let field = T::escape(&member.name);
        // Table 21: `@external` changes no octet, only where the binding
        // keeps the value — the same seam a structure member routes
        // through, with the branch accessor instead of the member one.
        let external = super::reached_externally::<T>(ast, member);
        let held = if external {
            T::external_case_value(ast, member_type, "v", &field)
        } else {
            T::case_member("v", &field)
        };
        // A branch closes a cycle the same way a member does, and the
        // column keeps it the same way. §7.2.2.4.4.4.7 makes no union
        // member `@optional`, so a branch on a cycle is one the
        // enclosing type reaches back through some *other* type's
        // `@optional` member.
        let held = if super::behind_pointer::<T>(ast, id, member_type, external, false) {
            T::behind_indirection(ast, member_type, held)
        } else {
            held
        };
        let value = T::perform(nested_call::<T>(
            ast,
            member_type,
            key,
            vec![T::name("w"), argument::<T>(ast, member_type, held)],
        ));
        // The selected member takes its own `MMEMBER` header under
        // `@mutable`, and nothing under any other extensibility.
        let planned = layout
            .members
            .iter()
            .find(|plan| plan.member == member.node_id);
        let write = T::block(mmember::<T>(
            ast,
            key,
            planned.map(|plan| plan.header),
            member_type,
            planned.map_or(0, |plan| plan.member_id),
            &format!("hu{}", planned.map_or(0, |plan| plan.member_id)),
            value,
        )?);

        let mut condition: Option<T::Expr> = None;
        let mut is_default = false;
        for case_label in &case_node.labels {
            match case_label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => {
                    let value = ast
                        .eval_integral(*expr)
                        .map_err(|_| EncodeError::NoWriter {
                            member: member.name.clone(),
                            kind: "union label that does not evaluate",
                        })?;
                    // A label a setting says selects nothing gets no arm
                    // at all, so the chain falls through to `default:`
                    // where the union has one and past the whole union
                    // where it has none — which is what the C peer this
                    // reproduces does with its own real union.
                    // `zerodds_ast_layout::union_negative_matches_nothing`.
                    if suppress_negative && value < 0 {
                        suppressed.push((value, member_type, field.clone(), external));
                        continue;
                    }
                    let test = T::binary(
                        "==",
                        T::discriminant("v"),
                        T::case_label(ast, discriminator, value),
                    );
                    condition = Some(match condition {
                        None => test,
                        Some(previous) => T::binary("||", previous, test),
                    });
                }
            }
        }
        if is_default {
            fallback = Some(write.clone());
            default_case = Some((member_type, member.node_id));
        }
        if let Some(condition) = condition {
            arms.push((Some(condition), write));
        }
    }

    // The overlay the suppression owes. The peer this reproduces holds
    // its arms in one C storage: a discriminator carrying a suppressed
    // label falls through to `default:` there and writes the *storage*
    // as the default arm — the selected member's octets, reinterpreted.
    // A target with one field per arm would write the never-filled
    // default field instead, so each suppressed label gets a branch
    // ahead of the default write that puts the selected member's bit
    // pattern out at the default arm's width, through the runtime's
    // `write_union_overlay`. Mirrors `write_selected_branch` in
    // `zerodds-wire-codec`; a union without `default:` writes nothing
    // after the discriminator, exactly as the plain suppression does.
    if let (Some((default_type, default_node)), false) = (default_case, suppressed.is_empty()) {
        let width = overlay_width(ast, default_type, key, "default:")?;
        let planned = layout
            .members
            .iter()
            .find(|plan| plan.member == default_node);
        let mut chain = fallback;
        for (value, source_type, source_field, source_external) in suppressed.into_iter().rev() {
            let source = bit_pattern_source(ast, source_type, key, &source_field)?;
            // The overlay reproduces the peer's *storage*, which in the
            // peer's C union is the value itself — an `@external` arm's
            // pointer bytes are this process's addresses, not octets any
            // wire form has, so the dereference is taken first.
            let member = if source_external {
                T::external_case_value(ast, source_type, "v", &source_field)
            } else {
                T::case_member("v", &source_field)
            };
            // And behind the indirection where the branch has one, for
            // the reason the selected write gives.
            let member = if super::behind_pointer::<T>(ast, id, source_type, source_external, false)
            {
                T::behind_indirection(ast, source_type, member)
            } else {
                member
            };
            // An integer form crosses the target's `Int64` cast first —
            // the seam the enumeration codec reads its value through —
            // so an enum arm needs no spelling of its own and a target
            // with no unsigned types receives its widest integer.
            let member = match source {
                crate::emit::BitPatternSource::Int { .. } => cast::<T>(
                    T::scalar(Scalar::Int64),
                    T::enumerator_ordinal(ast, source_type, member),
                ),
                _ => member,
            };
            let pattern = T::bit_pattern(source, member);
            let write = T::perform(T::runtime(
                "w",
                "write_union_overlay",
                vec![pattern, int::<T>(i128::from(width))],
            ));
            let overlay = T::block(mmember::<T>(
                ast,
                key,
                planned.map(|plan| plan.header),
                source_type,
                planned.map_or(0, |plan| plan.member_id),
                &format!("ho{}", planned.map_or(0, |plan| plan.member_id)),
                write,
            )?);
            let test = T::binary(
                "==",
                T::discriminant("v"),
                T::case_label(ast, discriminator, value),
            );
            chain = Some(T::block(vec![T::when(test, overlay, chain)]));
        }
        fallback = chain;
    }

    // Built from the last arm backwards, so the chain nests the way it
    // was declared.
    let mut chain: Option<T::Block> = fallback;
    for (condition, block) in arms.into_iter().rev() {
        let Some(condition) = condition else {
            continue;
        };
        chain = Some(T::block(vec![T::when(condition, block, chain)]));
    }
    if let Some(chain) = chain {
        statements.extend(T::into_statements(chain));
    }

    // Rule (28): the parameter list a `@mutable` union writes under
    // XCDR1 ends at its sentinel, the same as a structure's.
    if layout.needs_sentinel {
        statements.push(call::<T>("write_sentinel", Vec::new()));
    }
    statements.push(finish_dheader::<T>());
    if packed {
        statements.push(finish_packed::<T>());
    }
    Ok(statements)
}

/// The width the union overlay writes at — the arm's wire extent,
/// where it is fixed at 1, 2, 4 or 8 octets.
///
/// `zerodds-wire-codec`'s `storage_width`, asked of the same plan. The
/// refusal is loud rather than silent because
/// `union_negative_selects_nothing` answers `true` only for unions
/// whose arms all have such a width; reaching the error means that
/// crossing and this reading of it have diverged.
fn overlay_width(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    member: &str,
) -> Result<u8, EncodeError> {
    let mut deps = DepCollector::default();
    let width = zerodds_ast_layout::xcdr::plan(ast, type_, key, &mut deps)
        .ok()
        .and_then(|layout| layout.extent.sized())
        .filter(|width| matches!(width, 1 | 2 | 4 | 8));
    match width {
        #[allow(clippy::cast_possible_truncation)]
        Some(width) => Ok(width as u8),
        None => Err(EncodeError::NoWriter {
            member: member.to_string(),
            kind: "a union branch with no fixed-width storage to overlay",
        }),
    }
}

/// What the overlay reads the suppressed arm's bits through.
///
/// The scalar instruction decides: an integer, character, boolean,
/// enumeration or bitmask holder has storage bits, a float has its
/// IEEE 754 bits, and anything else — a string, a `fixed`, a `wchar`
/// that carries GIOP's length octet — has no fixed-width pattern and
/// is refused with the same sentence `overlay_width` uses.
fn bit_pattern_source(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    member: &str,
) -> Result<crate::emit::BitPatternSource, EncodeError> {
    use crate::emit::BitPatternSource as Source;
    let no_pattern = || EncodeError::NoWriter {
        member: member.to_string(),
        kind: "a union branch with no fixed-width storage to overlay",
    };
    let octets_of = |bits: u16| match bits {
        1..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    };
    match scalar_write(ast, type_, key, WcharPosition::Member).ok_or_else(no_pattern)? {
        ScalarWrite::Bool => Ok(Source::Bool),
        ScalarWrite::U8 | ScalarWrite::I8 | ScalarWrite::Char => Ok(Source::Int { octets: 1 }),
        ScalarWrite::I16 | ScalarWrite::U16 => Ok(Source::Int { octets: 2 }),
        ScalarWrite::I32 | ScalarWrite::U32 => Ok(Source::Int { octets: 4 }),
        ScalarWrite::I64 | ScalarWrite::U64 => Ok(Source::Int { octets: 8 }),
        ScalarWrite::F32 => Ok(Source::Float32),
        ScalarWrite::F64 => Ok(Source::Float64),
        ScalarWrite::Wchar {
            width,
            length_octet: false,
            ..
        } => Ok(Source::Int {
            octets: match width {
                WcharWidth::Two => 2,
                WcharWidth::Four => 4,
            },
        }),
        ScalarWrite::Enum { bits } | ScalarWrite::Bitmask { bits } => Ok(Source::Int {
            octets: octets_of(bits),
        }),
        _ => Err(no_pattern()),
    }
}

/// One value behind the `MMEMBER` header the plan gave it, or bare.
///
/// `None` is a union of any extensibility but `@mutable`, where rules
/// (26) and (30) frame nothing per member.
///
/// `member_type` is the member's type, which the per-sample header form
/// needs: a constructed member takes `PID_EXTENDED` in a sample past
/// RTI's step and the short form in one below it.
#[allow(clippy::too_many_arguments)]
fn mmember<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    header: Option<MemberHeader>,
    member_type: Id<TypeNode>,
    member_id: u32,
    handle: &str,
    value: T::Stmt,
) -> Result<Vec<T::Stmt>, EncodeError> {
    Ok(match header {
        None | Some(MemberHeader::NoHeader) => vec![value],
        Some(MemberHeader::PidCdr1 {
            pid,
            extended,
            must_understand,
        }) => {
            let mut statements = open_pid_header::<T>(
                ast,
                key,
                handle,
                pid,
                member_id,
                extended,
                Constant::ParameterMember,
                must_understand,
                member_type,
            )?;
            statements.push(value);
            statements.push(call::<T>("finish_pid", vec![T::name(handle)]));
            statements
        }
        Some(MemberHeader::EmHeader {
            lc,
            must_understand,
        }) => vec![
            T::declare(
                T::handle(Handle::EmHeader),
                handle,
                Some(call_expr::<T>(
                    "write_emheader",
                    vec![
                        int::<T>(i128::from(member_id)),
                        int::<T>(i128::from(lc.bits())),
                        T::literal(Literal::Bool(must_understand)),
                    ],
                )),
            ),
            value,
            call::<T>("finish_emheader", vec![T::name(handle)]),
        ],
        // Neither `OPT_FMEMBER` form reaches a union: a case member is
        // selected by the discriminator and is never `@optional`.
        Some(MemberHeader::OptionalCdr1 { .. } | MemberHeader::OptionalFlag) => vec![value],
    })
}

/// The statements that open a PL_CDR1 parameter header for a member.
///
/// The form is fixed for every plan but
/// [`per_sample`], and under that plan it is fixed too for a member whose
/// id or length forces the extended form and for one whose type keeps the
/// short form ([`zerodds_ast_layout::keeps_short_header`]). The remaining
/// case — a constructed member of a sample whose type decides — writes a
/// run-time branch on [`EXTENDED_PARAM`], which is the decision the
/// sample's root handed down.
#[allow(clippy::too_many_arguments)]
fn open_pid_header<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    handle: &str,
    pid: u16,
    member_id: u32,
    extended: bool,
    role: Constant,
    must_understand: bool,
    member_type: Id<TypeNode>,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let flag = T::literal(Literal::Bool(must_understand));
    let extended_call = call_expr::<T>(
        "write_pid_extended",
        vec![
            int::<T>(i128::from(member_id)),
            T::constant(role),
            flag.clone(),
        ],
    );
    let short_call = call_expr::<T>(
        "write_pid",
        vec![int::<T>(i128::from(pid)), T::constant(role), flag],
    );
    if !per_sample(key) || extended {
        return Ok(vec![T::declare(
            T::handle(Handle::Pid),
            handle,
            Some(if extended { extended_call } else { short_call }),
        )]);
    }
    if zerodds_ast_layout::keeps_short_header(ast, member_type)
        .map_err(crate::encode::plan_refusal)?
    {
        return Ok(vec![T::declare(
            T::handle(Handle::Pid),
            handle,
            Some(short_call),
        )]);
    }
    let choose = |call: T::Expr| T::block(vec![T::perform(T::binary("=", T::name(handle), call))]);
    Ok(vec![
        // A local the body **assigns** before it reads, in both arms of
        // the `when` below — not [`CodecTarget::storage`], which a Java
        // local the body writes *into* needs and which allocates. A
        // Java handle class is constructed by the runtime and carries no
        // accessible no-argument constructor: `runtime/java`'s
        // `Writer.Pid(int, boolean, int, ParameterRole, int, boolean)` is
        // package-private, so `new Writer.Pid()` is not a program javac
        // accepts, and the `declare` spelling is what leaves it
        // definitely assigned here.
        T::declare(T::handle(Handle::Pid), handle, None),
        T::when(
            T::name(EXTENDED_PARAM),
            choose(extended_call),
            Some(choose(short_call)),
        ),
    ])
}

/// The name, and resolved type, of the member with this identity.
///
/// `all_members_of`, so a member the aggregate inherited is found here
/// and not skipped as though the plan had never named it.
fn member_by_node(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    node: NodeId,
) -> Option<(String, Id<TypeNode>)> {
    let id = ast
        .all_members_of(aggregate)
        .into_iter()
        .find(|id| ast.member(*id).is_ok_and(|m| m.node_id == node))?;
    let member = ast.member(id).ok()?;
    Some((member.name.clone(), member.type_.resolved()?))
}

/// The handle a `DHEADER` is opened into, or the no-op form.
///
/// Declared either way so that the body's shape does not depend on the
/// plan's answer; the no-op handle writes nothing and finishes nothing.
fn dheader_declaration<T: CodecTarget>(present: bool) -> T::Stmt {
    T::declare(
        T::handle(Handle::DHeader),
        "d",
        Some(if present {
            T::runtime("w", "write_dheader", Vec::new())
        } else {
            T::runtime_static("no_dheader", Vec::new())
        }),
    )
}

fn finish_dheader<T: CodecTarget>() -> T::Stmt {
    call::<T>("finish_dheader", vec![T::name("d")])
}

/// The local a body's packed-element origin is saved in.
///
/// A distinct name from the DHEADER handle `d` and the element-length
/// handle `zd_el`, because all three can sit in one body: a packed
/// sequence writes an element length inside a packed origin.
const PACKED_SAVED: &str = "zd_saved";

/// `size_t zd_saved = begin_packed_element(w);`
///
/// [`ElementPadding::Packed`] is HDDS's measured departure from §7.4.1:
/// a collection element and an aggregate value count their alignment
/// from their own first octet instead of from the encapsulation. The
/// generated codec holds the displaced origin and restores it with
/// [`finish_packed`].
fn begin_packed<T: CodecTarget>() -> T::Stmt {
    T::declare(
        T::scalar(Scalar::Size),
        PACKED_SAVED,
        Some(T::runtime("w", "begin_packed_element", Vec::new())),
    )
}

/// `finish_packed_element(w, zd_saved);`
fn finish_packed<T: CodecTarget>() -> T::Stmt {
    T::perform(T::runtime(
        "w",
        "finish_packed_element",
        vec![T::name(PACKED_SAVED)],
    ))
}

/// The handle a sequence element's own octet length is opened into.
///
/// [`zerodds_ast_layout::key::SequenceElementDelimiter::BeforeEachNonFinalElement`]
/// writes a four-octet `UInt32` in front of every element of a
/// `sequence` whose element type is not `@final`, under both encoding
/// versions. It is a DHEADER in form — the length is the element's own
/// octet count — so the runtime's `write_dheader` / `finish_dheader`
/// pair writes it.
fn element_length_declaration<T: CodecTarget>() -> T::Stmt {
    T::declare(
        T::handle(Handle::DHeader),
        "zd_el",
        Some(T::runtime("w", "write_dheader", Vec::new())),
    )
}

fn finish_element_length<T: CodecTarget>() -> T::Stmt {
    T::perform(T::runtime("w", "finish_dheader", vec![T::name("zd_el")]))
}

/// Whether an element of a `sequence` is preceded by its own octet
/// length under `key`.
///
/// `zerodds_ast_layout::xcdr::sequence_element_carries_length`, the rule
/// `zerodds_wire_codec` and the generated Rust ask too: a four-octet `UInt32` in front of every
/// element whose type is not `@final`. A type with no extensibility of
/// its own — a primitive, a string, an enumeration, a `typedef` — is
/// `@final` by construction and takes none; only the four aggregate
/// kinds can be `@appendable` or `@mutable`.
///
/// # Errors
/// [`EncodeError::NoLayout`] for an element id the model does not hold:
/// whether the element carries a length cannot be told, and guessing
/// either way frames every element of the sequence wrong.
pub(crate) fn sequence_element_carries_length(
    ast: &Ast,
    element: Id<TypeNode>,
    key: LayoutKey,
) -> Result<bool, EncodeError> {
    // The planner's rule, called rather than restated, so the generated
    // codecs, `zerodds-wire-codec` and the generated Rust cannot answer
    // differently about the same element.
    zerodds_ast_layout::xcdr::sequence_element_carries_length(ast, element, key).ok_or_else(|| {
        EncodeError::NoLayout {
            detail: format!(
                "the sequence element {element:?} is not a type the model holds, so whether it \
                 carries its own length cannot be told"
            ),
        }
    })
}

/// The runtime call one scalar position asks for.
///
/// Every policy is already answered: `zerodds_ast_layout::scalar_write`
/// resolved them against the key and the position, so nothing here
/// reads an axis and nothing here can pass the wrong argument to one.
fn primitive_write_at<T: CodecTarget>(write: ScalarWrite, value: T::Expr) -> T::Stmt {
    match write {
        ScalarWrite::Bool => call::<T>("write_bool", vec![value]),
        ScalarWrite::U8 => call::<T>("write_u8", vec![value]),
        ScalarWrite::I8 => call::<T>("write_i8", vec![value]),
        // The runtime takes a code point and keeps the low byte, which
        // is where the narrowing belongs: a caller that truncated first
        // would put a different character on the wire.
        ScalarWrite::Char => call::<T>("write_char", vec![T::code_point(value, true)]),
        ScalarWrite::Wchar {
            width,
            length_octet,
            order,
        } => call::<T>(
            "write_wchar",
            vec![
                T::code_point(value, false),
                T::constant(match width {
                    WcharWidth::Two => Constant::WcharWidth2,
                    WcharWidth::Four => Constant::WcharWidth4,
                }),
                T::constant(if length_octet {
                    Constant::WcharGiopLengthPrefixed
                } else {
                    Constant::WcharFixed
                }),
                T::constant(wstring_unit_order(order)),
            ],
        ),
        ScalarWrite::I16 => call::<T>("write_i16", vec![value]),
        ScalarWrite::U16 => call::<T>("write_u16", vec![value]),
        ScalarWrite::I32 => call::<T>("write_i32", vec![value]),
        ScalarWrite::U32 => call::<T>("write_u32", vec![value]),
        ScalarWrite::I64 => call::<T>("write_i64", vec![value]),
        ScalarWrite::U64 => call::<T>("write_u64", vec![value]),
        ScalarWrite::F32 => call::<T>("write_f32", vec![value]),
        ScalarWrite::F64 => call::<T>("write_f64", vec![value]),
        ScalarWrite::F128 => call::<T>("write_f128", vec![value]),
        ScalarWrite::Str => call::<T>("write_string", vec![value]),
        ScalarWrite::Wstr {
            policy,
            order,
            code_units,
        } => match code_units {
            // `WstringCodeUnits::Utf8`: the characters' own octets, with
            // the length the UTF-8 octet count and only C2 — the
            // terminator — still reaching the wire. The order is inert
            // and is not passed.
            WstringCodeUnits::Utf8 => call::<T>(
                "write_wstring_utf8_octets",
                vec![value, T::constant(wstring_policy(policy))],
            ),
            WstringCodeUnits::Utf16 => call::<T>(
                "write_wstring_utf8",
                vec![
                    value,
                    T::constant(wstring_policy(policy)),
                    T::constant(wstring_unit_order(order)),
                ],
            ),
        },
        ScalarWrite::Fixed {
            digits,
            scale,
            count,
        } => call::<T>(
            "write_fixed",
            vec![
                value,
                int::<T>(i128::from(digits)),
                int::<T>(i128::from(scale)),
                T::constant(fixed_digit_count(count)),
            ],
        ),
        ScalarWrite::Enum { bits } => {
            call::<T>("write_enum", vec![int::<T>(i128::from(bits)), value])
        }
        ScalarWrite::Bitmask { bits } => call::<T>(
            "write_bitmask",
            vec![
                int::<T>(i128::from(bits)),
                cast::<T>(T::scalar(Scalar::UInt64), value),
            ],
        ),
    }
}

const fn wstring_policy(policy: WstringLengthPolicy) -> Constant {
    match policy {
        WstringLengthPolicy::SPECIFICATION => Constant::WstringSpecification,
        WstringLengthPolicy::CHARACTER_COUNT => Constant::WstringCharacterCount,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED => {
            Constant::WstringCharacterCountTerminated
        }
        WstringLengthPolicy::SPECIFICATION_WIDE_UNITS => Constant::WstringSpecificationWideUnits,
        WstringLengthPolicy::ROS_TYPE_SUPPORT => Constant::WstringRosTypeSupport,
        // The three cells no measured implementation writes. Named on
        // both sides so the C emitter can reach every cell the Rust
        // model can; a mapping that stopped at the measured five would
        // put the emitters back where the enumeration had them.
        _ => match (policy.counts, policy.terminator, policy.unit) {
            (WstringLengthCounts::Octets, _, WstringUnitWidth::Two) => {
                Constant::WstringOctetsTerminated2
            }
            (WstringLengthCounts::Octets, _, WstringUnitWidth::Four) => {
                Constant::WstringOctetsTerminated4
            }
            _ => Constant::WstringCharactersTerminated2,
        },
    }
}

/// Whether a codec call for `element` has to be replaced by an inline
/// write because the element's `wchar` framing is not the member's.
///
/// The generated codec for a type is one function, named for the type
/// and not for where it is called from, so a `Char16` element under
/// [`WcharFraming::GiopLengthPrefixedMemberOnly`] cannot reach the right
/// framing through `zd_enc_p_wchar`. Rather than emit a second function
/// for the one primitive that needs one, the collection emitters write
/// the call out here with the element's own constant.
///
/// `None` for every other element and every other framing, so the
/// generated code is unchanged wherever the two positions agree — which
/// is every dialect but TAO's.
fn element_write<T: CodecTarget>(
    ast: &Ast,
    element: Id<TypeNode>,
    key: LayoutKey,
    value: T::Expr,
) -> Option<T::Stmt> {
    if key.wchar_framing.has_length_octet(WcharPosition::Member)
        == key.wchar_framing.has_length_octet(WcharPosition::Element)
    {
        return None;
    }
    if !matches!(
        resolve_primitive(ast, element, 0),
        Some(PrimitiveKind::WChar)
    ) {
        return None;
    }
    let write = scalar_write(ast, element, key, WcharPosition::Element)?;
    Some(primitive_write_at::<T>(write, value))
}

/// Whether the elements of a collection of `element` are octets a target
/// may copy as one run: `octet` or `uint8`, through typedefs. `int8` is
/// left to the loop, since its storage is a signed type in the targets
/// that have one.
pub(crate) fn octet_elements(ast: &Ast, element: Id<TypeNode>) -> bool {
    matches!(
        resolve_primitive(ast, element, 0),
        Some(PrimitiveKind::Octet | PrimitiveKind::UInt8)
    )
}

/// [`CodecTarget::bulk_octets_write`] for `count` elements of `v`, when
/// they are octets ([`octet_elements`]) and the target has the call.
/// `None` keeps the element loop.
fn bulk_write<T: CodecTarget>(ast: &Ast, element: Id<TypeNode>, count: T::Expr) -> Option<T::Stmt> {
    if octet_elements(ast, element) {
        T::bulk_octets_write("v", count)
    } else {
        None
    }
}

/// The read side of [`bulk_write`], for `crate::emit::decode`.
pub(crate) fn bulk_read<T: CodecTarget>(
    ast: &Ast,
    element: Id<TypeNode>,
    count: T::Expr,
) -> Option<T::Stmt> {
    if octet_elements(ast, element) {
        T::bulk_octets_read("v", count)
    } else {
        None
    }
}

/// The octet types among `types` whose codec nothing calls once `bulk`
/// holds: an `octet` or `uint8`, or a typedef of one, reached only as
/// the elements of a `sequence` or a one-dimensional array that the
/// target copies in one piece ([`CodecTarget::bulk_octets_write`]).
///
/// Such a codec is left out rather than emitted and excused: a `static`
/// function nothing calls is `-Wunused-function` under clang, in a
/// header too, and [`maybe_unused`] is the one codec that may have no
/// caller. The walk starts from every type that is not an octet type —
/// each of those is emitted as before — and follows the edges a codec
/// calls along ([`crate::adapter::c::dependencies`]), minus the ones a
/// bulk copy replaced. An octet type it reaches is called; the rest are
/// the answer. Empty when `bulk` does not hold.
pub(crate) fn called_only_in_bulk(
    ast: &Ast,
    types: &[Id<TypeNode>],
    entries: &[Id<TypeNode>],
    bulk: bool,
) -> Vec<Id<TypeNode>> {
    if !bulk {
        return Vec::new();
    }
    let octets: Vec<Id<TypeNode>> = types
        .iter()
        .copied()
        .filter(|id| octet_elements(ast, *id))
        .collect();
    if octets.is_empty() {
        return Vec::new();
    }
    // An octet type the caller of the codec names (`entries`) is called by
    // that caller: the RPC binding calls the codec of an `octet` parameter.
    let mut called: Vec<Id<TypeNode>> = octets
        .iter()
        .copied()
        .filter(|id| entries.contains(id))
        .collect();
    let mut pending: Vec<Id<TypeNode>> = types
        .iter()
        .copied()
        .filter(|id| !octets.contains(id))
        .collect();
    while let Some(caller) = pending.pop() {
        let copied_in_bulk = match ast.type_(caller).map(|node| &node.kind) {
            Ok(TypeKind::Sequence { element, .. }) => element
                .resolved()
                .is_some_and(|element| octet_elements(ast, element)),
            Ok(TypeKind::Array { element, dims }) => {
                dims.len() == 1
                    && element
                        .resolved()
                        .is_some_and(|element| octet_elements(ast, element))
            }
            _ => false,
        };
        if copied_in_bulk {
            continue;
        }
        for callee in crate::adapter::c::dependencies(ast, caller) {
            if octets.contains(&callee) && !called.contains(&callee) {
                called.push(callee);
                pending.push(callee);
            }
        }
    }
    octets
        .into_iter()
        .filter(|id| !called.contains(id))
        .collect()
}

/// The primitive kind `id` is once typedefs are seen through, if it is
/// one.
///
/// zerodds-lint: recursion-depth 32 (typedef chains; bounded by the
/// `depth > 32` check, past which the answer is `None` — not a
/// primitive)
pub(crate) fn resolve_primitive(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Option<PrimitiveKind> {
    if depth > 32 {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => Some(kind),
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| resolve_primitive(ast, target, depth + 1)),
        _ => None,
    }
}

/// The C name of a `wstring` unit order.
const fn wstring_unit_order(order: WstringUnitOrder) -> Constant {
    match order {
        WstringUnitOrder::Stream => Constant::WstringUnitsStream,
        WstringUnitOrder::BigEndianAlways => Constant::WstringUnitsBigEndianAlways,
        WstringUnitOrder::ByteOrderMark => Constant::WstringUnitsByteOrderMark,
        WstringUnitOrder::HostUnswapped => Constant::WstringUnitsHostUnswapped,
    }
}

/// The runtime's name for a [`FixedDigitCount`].
///
/// The C runtime implements the count from [CDR] §9.3.2.8 itself, the
/// same way it implements the packing; this only names which of the three
/// the caller asked for.
const fn fixed_digit_count(policy: FixedDigitCount) -> Constant {
    match policy {
        FixedDigitCount::Declared => Constant::FixedDeclared,
        FixedDigitCount::ValuePrecision => Constant::FixedValuePrecision,
        FixedDigitCount::ValueStringDigits => Constant::FixedValueStringDigits,
    }
}

// -- a map's entry order -------------------------------------------------

/// `for (uint32_t <index> = 0; <index> < v->_length; ...) { body }`.
///
/// One builder, because the entry order emits three of these nested and
/// three literal copies is how the three would come to disagree about
/// the bound.
fn over_the_entries<T: CodecTarget>(index: &str, body: T::Block) -> T::Stmt {
    T::count_up(index, T::count_of("v", None), body)
}

/// The entries in ascending key order, without a scratch array.
///
/// ```text
/// for (uint32_t zd_rank = 0; zd_rank < v->_length; …)
///     for (uint32_t i = 0; i < v->_length; …) {
///         uint32_t zd_before = 0;
///         for (uint32_t zd_k = 0; zd_k < v->_length; …)
///             if (<entry zd_k sorts before entry i>) zd_before = zd_before + 1;
///         if (zd_before == zd_rank) { <write the pair at i> }
///     }
/// ```
///
/// A rank and not a sort. The generated writer has no allocation and no
/// variable-length array — `runtime/c` allocates nothing in the write
/// path and an embedded target links it — so there is nowhere to put a
/// permutation. Computing each entry's rank in place needs no storage at
/// all, and a corpus map has at most three entries.
///
/// "Sorts before" carries the index as the tie-break, so two entries
/// with equal keys keep the order the sample declared: the same stable
/// order `encode.rs::map_entry_order` and Python's `sorted` give.
fn ascending_by_key<T: CodecTarget>(
    ast: &Ast,
    key_type: Id<TypeNode>,
    ordering: MapKeyOrdering,
    pair: T::Block,
) -> T::Stmt {
    let before = map_key_before::<T>(ast, key_type, ordering, "zd_k", "i");
    let count = over_the_entries::<T>(
        "zd_k",
        T::block(vec![T::when(
            before,
            T::block(vec![T::perform(increment::<T>("zd_before"))]),
            None,
        )]),
    );
    let select = over_the_entries::<T>(
        "i",
        T::block(vec![
            T::declare(T::scalar(Scalar::UInt32), "zd_before", Some(int::<T>(0))),
            count,
            T::when(
                T::binary("==", T::name("zd_before"), T::name("zd_rank")),
                pair,
                None,
            ),
        ]),
    );
    over_the_entries::<T>("zd_rank", T::block(vec![select]))
}

/// Whether the entry at `left` sorts before the entry at `right`.
///
/// `less(left, right) || (!less(right, left) && left < right)` — the key
/// order with the declared position as the tie-break.
fn map_key_before<T: CodecTarget>(
    ast: &Ast,
    key_type: Id<TypeNode>,
    ordering: MapKeyOrdering,
    left: &str,
    right: &str,
) -> T::Expr {
    let less = |a: &str, b: &str| map_key_less::<T>(ast, key_type, ordering, a, b);
    T::binary(
        "||",
        T::paren(less(left, right)),
        T::paren(T::binary(
            "&&",
            T::unary("!", T::paren(less(right, left))),
            T::paren(T::binary("<", T::name(left), T::name(right))),
        )),
    )
}

/// Whether the key at `left` is less than the key at `right`.
///
/// The comparison says the signedness out loud rather than taking the C
/// type's. An IDL `char` is a C `char`, whose signedness is
/// implementation-defined; the corpus was recorded on a host where it is
/// signed, and the cast pins that answer so the three columns still
/// agree on a host where it is not. Every other key type has a C
/// counterpart whose own `<` is the order the crossing named — `uint8_t`
/// and `uint16_t` and `bool` unsigned, the sized signed integers signed,
/// a generated `enum` by its declared values. A `string` and a `wstring`
/// are both a `const char *` holding UTF-8, and `strcmp` compares as
/// `unsigned char` (C11 §7.24.4), which is `std::string`'s order and,
/// UTF-8 being order preserving, `std::wstring`'s too.
fn map_key_less<T: CodecTarget>(
    ast: &Ast,
    key_type: Id<TypeNode>,
    ordering: MapKeyOrdering,
    left: &str,
    right: &str,
) -> T::Expr {
    let key_at = |index: &str| {
        let half = T::entry_half(T::index(T::elements("v"), T::name(index)), true);
        // An enumeration key is ordered by its enumerator *value*, and
        // a column whose enumeration is a string at the language
        // surface would otherwise compare the names. The identity
        // default leaves every other column's comparison where it was.
        if ordering == MapKeyOrdering::EnumeratorValue {
            return T::enumerator_ordinal(ast, key_type, half);
        }
        half
    };
    if ordering == MapKeyOrdering::CodeUnits {
        return T::binary(
            "<",
            T::call("strcmp", vec![key_at(left), key_at(right)]),
            int::<T>(0),
        );
    }
    let signed_char = ordering == MapKeyOrdering::SignedInteger
        && matches!(
            resolve_primitive(ast, key_type, 0),
            Some(PrimitiveKind::Char)
        );
    let read = |index: &str| {
        if signed_char {
            T::paren(cast::<T>(T::named("signed char"), key_at(index)))
        } else {
            key_at(index)
        }
    };
    // An eight-octet unsigned key crosses the target's own seam: a
    // column that holds `unsigned long long` as a bit pattern in its
    // widest signed type has no `<` that answers the unsigned order.
    // `CodecTarget::unsigned_less` says which columns those are.
    if ordering == MapKeyOrdering::UnsignedInteger
        && matches!(
            resolve_primitive(ast, key_type, 0),
            Some(PrimitiveKind::UInt64)
        )
    {
        return T::unsigned_less(8, read(left), read(right));
    }
    // A `boolean` key crosses the same kind of seam as the eight-octet
    // unsigned one: not every target's `<` takes two booleans.
    // `CodecTarget::bool_less` says how each column spells the order.
    if matches!(
        resolve_primitive(ast, key_type, 0),
        Some(PrimitiveKind::Boolean)
    ) {
        return T::bool_less(read(left), read(right));
    }
    T::binary("<", read(left), read(right))
}

/// An enumeration's write body: the holder's width, and the integer that
/// goes into it.
///
/// The generated C `enum` keeps its **declared** constants — that is the
/// language mapping and not a wire question — so under
/// [`zerodds_ast_layout::EnumeratorValue::Ordinal`] the value in the
/// variable and the value on the wire are different integers, and a
/// chain maps one to the other. Under `Declared` the chain is the
/// identity and is not emitted at all, so the default column's C is
/// unchanged.
///
/// The reverse chain is `decode_c.rs::enumeration_body`, which compares
/// the wire pattern and assigns the declared constant.
///
/// # Why an enumerator this cannot answer for is an error
///
/// `zd_wire` is initialised to the **declared** value and each arm
/// overwrites it, so an enumerator that reaches no arm is written with
/// the value the variable already holds. Under `Declared` that is the
/// right integer and the chain is not emitted at all; under `Ordinal`
/// it is a different integer, and the generated C would write the
/// declared value, return success, and disagree with every other column
/// on the same setting. The reader's chain in
/// `decode_c.rs::enumeration_body` can skip an arm because a pattern
/// with no arm is refused by name — an absent arm there is a refusal
/// and here it is a wrong value, which is why the two sides answer
/// differently.
///
/// # Errors
/// [`EncodeError::NoEnumeratorValue`] for an enumerator whose handle the
/// model no longer holds, or whose wire value the setting cannot
/// compute.
fn enumeration_write<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let Some(write) = scalar_write(ast, id, key, WcharPosition::Member) else {
        return Err(EncodeError::NoWriter {
            member: T::type_name(ast, id),
            kind: "enumeration the layout plan has no scalar write for",
        });
    };
    // Through the enumerator seam: a column whose enumeration is not a
    // number at the language surface has no integer in the variable,
    // and the identity default leaves every other column's output where
    // it was. `CodecTarget::enumerator_ordinal`.
    let holder = cast::<T>(
        T::scalar(Scalar::Int64),
        T::enumerator_ordinal(ast, id, deref::<T>("v")),
    );
    // The declared values reach the wire unchanged under the
    // specification's setting, so the generated code is one call and
    // the chain below is what a front end that drops `@value` needs.
    if zerodds_ast_layout::plan::enumerators_are_declared(key) {
        return Ok(vec![primitive_write_at::<T>(write, holder)]);
    }

    let enumeration = ast
        .type_(id)
        .map_err(|error| EncodeError::NoLayout {
            detail: format!("{error}"),
        })?
        .name
        .clone();
    // The mapping and not the policy: `enumerator_writes` resolved every
    // enumerator against the key, so nothing here can pass the wrong
    // setting or answer the axis by omission.
    let arms = zerodds_ast_layout::plan::enumerator_writes(ast, id, key).map_err(|enumerator| {
        EncodeError::NoEnumeratorValue {
            enumeration: enumeration.clone(),
            enumerator,
            detail: "its wire value does not evaluate under this --enumerator-value setting",
        }
    })?;

    let mut statements = vec![T::declare(
        T::scalar(Scalar::Int64),
        "zd_wire",
        Some(holder),
    )];
    // Built from the last enumerator backwards so the chain reads in
    // declaration order, which is the order the reader's chain takes
    // too: where two enumerators share a declared value the first one
    // wins in both directions.
    let mut chain: Option<T::Block> = None;
    for (name, on_wire) in arms.into_iter().rev() {
        // Through the enumerator seam and not through
        // `module_prefix` plus `escape`, which was the same leak the
        // C++ reader found in the other direction. The two are one
        // string in C, where an enumerator sits in the enclosing scope
        // and is not folded; they are two different strings in every
        // column that scopes its enumerations or renames its members.
        // The C# column is both — `global::a.Colour.RED` against a
        // member fold that would have written `Red`.
        let constant = T::enumerator_constant(ast, id, &name);
        chain = Some(T::block(vec![T::when(
            T::binary("==", deref::<T>("v"), constant),
            // `wide_int` and not `int`: `zd_wire` is the Int64 holder,
            // and the assigned literal has to be one of that width —
            // TypeScript's `bigint` takes `0n` and not `0` (TS2322 on
            // every ordinal-mapped enumeration), and a declared value
            // above `i32::MAX` needs the suffix in Java, C and C++
            // too. For every value inside `i32` each of those columns
            // spells `wide_int` exactly as it spelt `int`, so their
            // generated files do not move.
            T::block(vec![T::perform(T::binary(
                "=",
                T::name("zd_wire"),
                T::wide_int(on_wire),
            ))]),
            chain,
        )]));
    }
    if let Some(chain) = chain {
        statements.extend(T::into_statements(chain));
    }
    statements.push(primitive_write_at::<T>(write, T::name("zd_wire")));
    Ok(statements)
}

// -- small builders ------------------------------------------------------
//
// Each of these is now one call into the C backend of
// [`crate::emit::CodecTarget`]. They stay as free functions because the
// walk above reads better for it and because the argument order is this
// file's, not the trait's; what they no longer do is build a C node.
// When the walk itself becomes generic these become the trait's methods
// at their use sites and disappear.

/// A runtime call, evaluated for its effect.
///
/// `name` is the operation without the runtime's prefix — `write_u32`,
/// not `zdw_write_u32` — and the receiver is prepended by the backend.
fn call<T: CodecTarget>(name: &str, args: Vec<T::Expr>) -> T::Stmt {
    T::perform(T::runtime("w", name, args))
}

/// The same call where the result is kept.
///
/// A header-opening operation hands back the position it wrote at, which
/// the matching close needs; those bind rather than discard.
fn call_expr<T: CodecTarget>(name: &str, args: Vec<T::Expr>) -> T::Expr {
    T::runtime("w", name, args)
}

/// The limit a bound states, where it states one this column can write.
///
/// `None` for the unbounded form and for a bound that does not
/// evaluate; a negative one is not a length and is refused the same
/// way. The decoder's own copy is
/// [`crate::emit::decode::bound_limit`] — one function because a
/// `sequence`, a `string`, a `wstring` and a `map` all read their bound
/// the same, and three copies in this file would be three chances for
/// one collection to stop being checked while the others were.
fn bound_limit(ast: &Ast, bound: &Bound) -> Option<i128> {
    bound
        .expr()
        .and_then(|expr| ast.eval_integral(expr).ok())
        .filter(|limit| *limit >= 0)
}

/// The write for a bounded `string` or a bounded `wstring`.
///
/// The runtime's `write_string_bounded`/`write_wstring_bounded` takes
/// the bound as an argument and refuses a value that overruns it —
/// [`crate::encode::MemberWrite::String`] emits the same call for Rust,
/// against [`crate::encode`]'s own `write_string_bounded`, and
/// `crates/wire::Writer::write_string_bounded`'s remarks are why: the
/// bound is a promise about the source and not a difference on the
/// wire, so accepting an over-bound value and writing it as if it fit
/// would put a sample on the wire nobody wrote, and cutting it at a
/// character boundary would put a different one there with nothing to
/// say so.
///
/// # Errors
/// [`EncodeError::NoWriter`] for a bound on a scalar write that is
/// neither a string nor a wstring — never reached while
/// [`zerodds_ast_layout::plan::scalar_write`] answers a `string` type
/// with one of the two, which is the only way this is called.
fn bounded_string_write<T: CodecTarget>(
    own_name: &str,
    write: ScalarWrite,
    value: T::Expr,
    limit: i128,
) -> Result<T::Stmt, EncodeError> {
    match write {
        ScalarWrite::Str => Ok(call::<T>(
            "write_string_bounded",
            vec![value, T::unsigned_int(limit)],
        )),
        ScalarWrite::Wstr {
            policy,
            order,
            code_units,
        } => Ok(match code_units {
            // The bound still counts UTF-16 code units, whatever the wire
            // carries; the runtime's octet form measures them and writes
            // the UTF-8 octets. See `write_wstring_utf8_octets_bounded`.
            WstringCodeUnits::Utf8 => call::<T>(
                "write_wstring_utf8_octets_bounded",
                vec![
                    value,
                    T::unsigned_int(limit),
                    T::constant(wstring_policy(policy)),
                ],
            ),
            WstringCodeUnits::Utf16 => call::<T>(
                "write_wstring_bounded",
                vec![
                    value,
                    T::unsigned_int(limit),
                    T::constant(wstring_policy(policy)),
                    T::constant(wstring_unit_order(order)),
                ],
            ),
        }),
        _ => Err(EncodeError::NoWriter {
            member: own_name.to_string(),
            kind: "a bound on a scalar write that is not a string or a wstring",
        }),
    }
}

/// A collection's count, checked against its bound before anything of
/// the collection is written.
///
/// `method` is `check_sequence_bound` or `check_map_bound` — the
/// runtime's own names, matching
/// [`crate::encode`]'s Rust codegen and `crates/wire::Writer`'s two
/// checks of the same shape. Both take the count and the bound and
/// refuse loudly, in whatever their language's own mechanism is,
/// before the DHEADER or the length word goes out — a sender that
/// wrote more than its own declaration allows has written a value of a
/// type nobody declared, and stopping after the length word would
/// leave that word on the wire ahead of the refusal.
fn bound_check<T: CodecTarget>(method: &str, count: T::Expr, limit: i128) -> T::Stmt {
    call::<T>(method, vec![count, T::unsigned_int(limit)])
}

fn deref<T: CodecTarget>(name: &str) -> T::Expr {
    T::whole(name)
}

fn arrow<T: CodecTarget>(value: &str, field: &str) -> T::Expr {
    T::member(value, field)
}

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `T::cast`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn cast<T: CodecTarget>(type_: T::Type, value: T::Expr) -> T::Expr {
    T::cast(type_, value)
}

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `T::int`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn int<T: CodecTarget>(value: i128) -> T::Expr {
    T::int(value)
}

fn increment<T: CodecTarget>(name: &str) -> T::Expr {
    T::binary(
        "=",
        T::name(name),
        T::binary("+", T::name(name), int::<T>(1)),
    )
}
