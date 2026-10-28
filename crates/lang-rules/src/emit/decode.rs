// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C decoder bodies, built from the same layout plan. Spec §5, §6.2.
//!
//! The counterpart of [`crate::encode_c`], which walks the plan and
//! writes. This walks it and reads. Not a second reading of the model,
//! for the reason the encoder gives: two walks that decided the framing
//! separately would agree with each other only until one of them
//! changed.
//!
//! ## Why the column needs this at all
//!
//! Encoding is measured by comparing bytes, and a byte comparison
//! passes a decoder that is wrong in the same way its own encoder is
//! wrong. A round trip through one implementation passes such a defect
//! every time; a round trip through two does not — and the C decoder
//! and the Rust decoder are two.
//! `docs/specs/zerodds-interop-suite-1.0.md` §4, P2.
//!
//! ## Where reading needs a decision writing did not
//!
//! The same three that `crates/wire-probe/src/decode.rs` names for the
//! reflective reader, and they are why this file is longer than the
//! encoder:
//!
//! - **A union's member.** The writer takes the member the value names;
//!   the reader has only the discriminator, so the generated code
//!   compares it against every case label, falls back to `default:`,
//!   and reads no member at all where neither matches.
//! - **A `@mutable` aggregate is driven by its headers and not by the
//!   plan.** A writer may order the members as it likes and leave any
//!   `@optional` one out, so the generated code is a dispatch on the
//!   member id rather than a straight line.
//!
//! ## What C forces that Rust does not
//!
//! - **Somewhere to put a string.** An IDL `string` is a
//!   `const char *` and a `sequence<T>` is a count beside a pointer, so
//!   the read direction needs storage the write direction did not. It
//!   comes from `zdw_reader_alloc`, out of a buffer the caller set with
//!   `zdw_reader_set_heap` — the same discipline as the writer's output
//!   buffer, and for the same reason: an embedded target decides where
//!   memory comes from.
//! - **Refusals the runtime cannot see.** An integer that names no
//!   enumerator and a member that never arrived are refusals of the
//!   *type*, which `runtime/c` does not know. `zdw_reader_refuse` is
//!   how the generated code says so; without it the decoder would hand
//!   its caller a value the type does not admit.

use crate::adapter::c::{ordered_types, ordered_types_from, ordered_types_from_pruning};
use crate::emit::encode::CodecOptions;
use crate::emit::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::encode::EncodeError;
use zerodds_ast::{
    Ast, CaseLabel, CharWidth, Extensibility, PrimitiveKind, TryConstruct, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{
    ElementPadding, FixedDigitCount, LayoutKey, WcharPosition, WcharWidth, WstringCodeUnits,
    WstringLengthCounts, WstringLengthPolicy, WstringUnitOrder, WstringUnitWidth,
};
use zerodds_ast_layout::plan::{Extent, MemberHeader, ScalarWrite, scalar_write};
use zerodds_ast_layout::xcdr::DepCollector;
use zerodds_lang_cst::common::{Doc, Literal};

/// The decoder function name for a type.
///
/// Named by the IDL kind for a primitive, for the reason
/// [`crate::encode_c::codec_name`] gives: `double` and `long double`
/// share a C type and do not share a wire form.
#[must_use]
pub fn decoder_name<T: CodecTarget>(ast: &Ast, id: Id<TypeNode>) -> String {
    let name = match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_dec_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_dec_{}", T::type_name(ast, id)),
    };
    // As `crate::emit::encode::codec_name`: the published spelling, in
    // one place. See [`CodecTarget::published_name`].
    T::published_name(name)
}

/// Build the decoder for every type in the model, under one plan.
///
/// Existing data entry points keep their published names and initialization.
///
/// # Errors
/// [`EncodeError`] when the layout cannot be planned or a construct has
/// no C reader — never a body that leaves a member out.
pub fn decoder<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    decoder_over::<T>(
        ast,
        &ordered_types(ast),
        &crate::emit::encode::sample_types(ast),
        key,
        options,
    )
}

/// [`decoder`] without the types in `excluded`. See
/// [`crate::emit::encode::codec_excluding`].
///
/// # Errors
/// As [`decoder`].
pub fn decoder_excluding<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    excluded: &[Id<TypeNode>],
    also_reaching: &[Id<TypeNode>],
) -> Result<Vec<T::Decl>, EncodeError> {
    if excluded.is_empty() {
        return decoder::<T>(ast, key, options);
    }
    let (types, entries) = crate::emit::encode::kept_after_excluding(ast, excluded, also_reaching);
    decoder_over::<T>(ast, &types, &entries, key, options)
}

/// The decoder for the types one root reaches, and no others.
///
/// What a probe's translation unit wants: a decoder for a type the root
/// cannot reach is a `static` function nothing calls, and a generated
/// file that warns is one whose warnings stop being read.
///
/// # Errors
/// As [`decoder`].
pub fn decoder_from<T: CodecTarget>(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    decoder_over::<T>(
        ast,
        &ordered_types_from(ast, &[root]),
        &[root],
        key,
        options,
    )
}

/// The decoder for the types several roots reach, each function once.
///
/// # Errors
/// As [`decoder`].
pub fn decoder_from_many<T: CodecTarget>(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    decoder_over::<T>(ast, &ordered_types_from(ast, roots), roots, key, options)
}

/// [`decoder_from_many`] without the types in `excluded`; see
/// [`crate::emit::encode::codec_from_many_excluding`].
///
/// # Errors
/// As [`decoder`].
pub fn decoder_from_many_excluding<T: CodecTarget>(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
    excluded: &[Id<TypeNode>],
) -> Result<Vec<T::Decl>, EncodeError> {
    let types = ordered_types_from_pruning(ast, roots, excluded);
    decoder_over::<T>(ast, &types, roots, key, options)
}

/// The distinct name of the bounded inverse key-holder entry.
#[must_use]
pub fn key_holder_decoder_name<T: CodecTarget>(ast: &Ast, root: Id<TypeNode>) -> String {
    T::published_name(format!("zd_keyread_{}", decoder_name::<T>(ast, root)))
}

/// Read a ForcedFinal PLAIN_CDR2-BE struct holder with scalar/string/enum keys.
/// The shared key plan decides root selection and member order. Nested
/// aggregates, aliases, collections and indirect keys explicitly refuse until
/// their full key context can be carried, never falling through to data reads.
/// Non-key fields come from the target's existing default materializer.
///
/// # Errors
/// A missing plan, unsupported root/member/dialect or target initialization.
pub fn key_holder_decoder_from<T: CodecTarget>(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Option<Vec<T::Decl>>, EncodeError> {
    let refuse = |detail: &str| EncodeError::NoLayout {
        detail: detail.to_string(),
    };
    if key != LayoutKey::key(zerodds_ast_layout::Encoding::PlainCdr2) {
        return Err(refuse(
            "key holder inverse supports the default ForcedFinal PLAIN_CDR2-BE key dialect only",
        ));
    }
    if !matches!(
        ast.type_(root).map(|node| &node.kind),
        Ok(TypeKind::Structure { .. })
    ) {
        return Err(refuse(
            "key holder inverse currently supports struct roots only",
        ));
    }
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        root,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(crate::encode::plan_refusal)?;
    if keys.is_empty() {
        return Ok(None);
    }
    let layout = zerodds_ast_layout::keyhash::key_holder_plan_with(
        ast,
        root,
        key,
        &mut DepCollector::default(),
    )
    .map_err(crate::encode::plan_refusal)?;
    let members = slots::<T>(ast, root, &layout)?;
    // key_members_with includes descendant member identities. Diagnose an
    // unsupported nested context before comparing the direct slot count.
    for slot in &members {
        if slot.optional
            || slot.external
            || slot.indirect
            || !matches!(
                ast.type_(slot.type_).map(|node| &node.kind),
                Ok(TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Enumeration { .. })
            )
        {
            return Err(refuse(&format!(
                "key holder inverse member `{}` requires unsupported nested, alias, collection or indirect key context",
                slot.field
            )));
        }
    }
    if members.len() != keys.len() || layout.needs_dheader {
        return Err(refuse(
            "key holder inverse could not describe every planned key member",
        ));
    }
    let mut declarations = Vec::new();
    let mut emitted = Vec::new();
    let mut statements = Vec::new();
    for slot in &members {
        let name = key_holder_decoder_name::<T>(ast, slot.type_);
        if !emitted.contains(&name) {
            let body = body_of::<T>(ast, slot.type_, key, options, &name)?
                .ok_or_else(|| refuse("key holder inverse has no member reader"))?;
            declarations.push(initialized_key_reader::<T>(
                ast,
                slot.type_,
                name.clone(),
                body,
            )?);
            emitted.push(name.clone());
        }
        statements.push(T::read_call(
            &name,
            arrow::<T>("v", &slot.field),
            Handoff::Value,
            T::constant(Constant::TryConstructDiscard),
        ));
    }
    declarations.push(initialized_key_reader::<T>(
        ast,
        root,
        key_holder_decoder_name::<T>(ast, root),
        T::block(statements),
    )?);
    Ok(Some(declarations))
}

fn initialized_key_reader<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    name: String,
    body: T::Block,
) -> Result<T::Decl, EncodeError> {
    let initializer = T::default_into(ast, id, deref::<T>("v"))?;
    T::initialized_reader(
        Routine {
            doc: Doc::none(),
            name,
            sample: T::value_type(ast, id),
            held: Handoff::Value,
            writing: false,
            tolerate_unused: false,
            extra: vec![(T::handle(Handle::TryConstruct), "zd_tc".to_string())],
            body: Some(with_tc_used::<T>(body)),
        },
        initializer,
    )
    .ok_or_else(|| EncodeError::NoLayout {
        detail: "target does not support explicit key-holder reader initialization".to_string(),
    })
}

fn decoder_over<T: CodecTarget>(
    ast: &Ast,
    types: &[Id<TypeNode>],
    entries: &[Id<TypeNode>],
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<T::Decl>, EncodeError> {
    crate::encode::axes_the_codecs_carry_for::<T>(key)?;
    let mut prototypes = Vec::new();
    let mut bodies = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    // As `crate::emit::encode::codec_over`: an octet decoder only a bulk
    // read would have called is not emitted.
    let uncalled = crate::emit::encode::called_only_in_bulk(
        ast,
        types,
        entries,
        T::bulk_octets_read("v", name::<T>("zd_n")).is_some(),
    );

    for id in types.iter().copied() {
        if uncalled.contains(&id) {
            continue;
        }
        let name = decoder_name::<T>(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(body) = body_of::<T>(ast, id, key, options, &name)? else {
            continue;
        };
        emitted.push(name.clone());
        // A target with no forward declarations answers `None` to the
        // prototype and the loop emits only the body.
        prototypes.extend(function::<T>(ast, id, key, &name, None));
        bodies.extend(function::<T>(
            ast,
            id,
            key,
            &name,
            Some(with_tc_used::<T>(body)),
        ));
    }

    prototypes.extend(bodies);
    // Not the Table 9 defaulters. Which helper a target needs for an
    // absent member — and whether it needs one at all — is the
    // materialiser's answer, and the C binding prepends C's after this
    // returns.
    Ok(prototypes)
}

/// The body, with `zd_tc` marked as read.
///
/// Most decoders have nothing to do with the behaviour — a `long` has no
/// bound to miss — and the C probe is built with `-Wall -Wextra
/// -Werror`, where an unused parameter fails the build. `(void)zd_tc;`
/// is valid whether or not the body goes on to use it, so it is emitted
/// unconditionally rather than from a guess about which bodies read it.
fn with_tc_used<T: CodecTarget>(body: T::Block) -> T::Block {
    let mut statements = T::mark_used("zd_tc");
    statements.extend(T::into_statements(body));
    T::block(statements)
}

/// `static void <name>(zdw_reader *r, <T> *v, zdw_try_construct zd_tc)`,
/// with or without a body.
///
/// The value is not `const` here and is in the encoder, which is the
/// only difference in the two signatures. An array parameter is the
/// array itself for the reason the encoder gives: C adjusts it to a
/// pointer to its first element, and `(*v)[i][j]` would otherwise need
/// parentheses at every use.
fn function<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
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
        writing: false,
        // The reading counterpart of `emit::encode::maybe_unused`: a
        // `Char16` read inline at every element site leaves this
        // function without a caller under TAO's framing.
        tolerate_unused: crate::emit::encode::maybe_unused(ast, id, key),
        // What this *position* does with a value the type cannot hold.
        // A parameter and not a property of the type, because
        // DDS-XTypes 1.3 §7.2.2.7 attaches the behaviour to the member,
        // the element, the map key or the discriminator — and one
        // `typedef string<5>` may be used at two positions that want
        // different answers.
        extra: vec![(T::handle(Handle::TryConstruct), "zd_tc".to_string())],
        body,
    })
}

/// Whether a type is a C array once typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by the
/// `depth > 64` check, past which the answer is `false` — the
/// destination form a non-array takes)
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

/// How a destination of `id` is handed to its decoder: the array
/// itself, or anything else in the form [`Handoff::Value`] names.
const fn destination_form(is_array: bool) -> Handoff {
    if is_array {
        Handoff::Array
    } else {
        Handoff::Value
    }
}

/// One member's read, with its explicit `@default` where the type's
/// decoder cannot supply it.
///
/// `USE_DEFAULT` plus a `@default` is the one combination a
/// type-keyed decoder cannot answer, so the site asks for DISCARD and
/// takes the answer back. `zdw_reader_clear_discard` clears that status
/// and no other: a truncated stream is not a narrowing. The position is
/// already right, because every narrowing reads the whole value off the
/// wire before it decides.
fn member_read<T: CodecTarget>(ast: &Ast, slot: &Slot<T>, place: T::Expr) -> Vec<T::Stmt> {
    let Some(assign_default) = slot.explicit_default.clone() else {
        return vec![decode_into::<T>(ast, slot.type_, place, slot.try_construct)];
    };
    vec![
        decode_into::<T>(ast, slot.type_, place, TryConstruct::Discard),
        T::when(
            call_expr::<T>("reader_clear_discard", Vec::new()),
            T::block(assign_default),
            None,
        ),
    ]
}

/// `zd_dec_<T>(r, <destination>, <behaviour>);`
/// As [`decode_into`], for a value reached as a collection element.
///
/// The decoder generated for a type is one function and cannot ask at
/// run time where it was called from, so the one primitive whose
/// framing depends on the position — `Char16` under
/// [`WcharFraming::GiopLengthPrefixedMemberOnly`] — is read inline here
/// with the element's own constant. Every other element and every other
/// framing falls through to the ordinary call, so the generated code is
/// unchanged outside TAO's dialect.
fn decode_element_into<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    place: T::Expr,
    behaviour: TryConstruct,
    key: LayoutKey,
) -> T::Stmt {
    if key.wchar_framing.has_length_octet(WcharPosition::Member)
        != key.wchar_framing.has_length_octet(WcharPosition::Element)
        && matches!(resolve_primitive(ast, id, 0), Some(PrimitiveKind::WChar))
    {
        let Some(write) = scalar_write(ast, id, key, WcharPosition::Element) else {
            return decode_into::<T>(ast, id, place, behaviour);
        };
        // `primitive_read_at` already casts a wchar read to the code
        // point holder; a second cast here survived `store`'s
        // one-cast folding and Java's `-Xlint:cast -Werror` names the
        // leftover `(int)` (36 tao/rti element-form cells).
        return T::store(place, T::value_type(ast, id), primitive_read_at::<T>(write));
    }
    decode_into::<T>(ast, id, place, behaviour)
}

/// The primitive kind `id` is once typedefs are seen through.
///
/// zerodds-lint: recursion-depth 32 (typedef chains; bounded by the
/// `depth > 32` check, past which the answer is `None` — not a
/// primitive)
fn resolve_primitive(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<PrimitiveKind> {
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

fn decode_into<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    place: T::Expr,
    behaviour: TryConstruct,
) -> T::Stmt {
    T::read_call(
        &decoder_name::<T>(ast, id),
        place,
        destination_form(is_array(ast, id, 0)),
        T::constant(try_construct_constant(behaviour)),
    )
}

/// The `zdw_try_construct` constant one behaviour is spelled by.
const fn try_construct_constant(behaviour: TryConstruct) -> Constant {
    match behaviour {
        TryConstruct::Discard => Constant::TryConstructDiscard,
        TryConstruct::UseDefault => Constant::TryConstructUseDefault,
        TryConstruct::Trim => Constant::TryConstructTrim,
    }
}

/// `switch (zd_tc)` over the three behaviours, as one statement.
///
/// `trim` is what TRIM does where Table 11 defines it and empty where
/// it does not — the frontend refuses such a declaration, so the arm
/// falls through to the discard it would otherwise silently become.
fn narrow<T: CodecTarget>(trim: Vec<T::Stmt>, use_default: Vec<T::Stmt>) -> T::Stmt {
    let mut discard = vec![refuse::<T>(Constant::StatusDiscarded), T::give_back(None)];
    let mut default_arm = use_default;
    default_arm.push(T::give_back(None));
    let mut trim_arm = trim;
    if trim_arm.is_empty() {
        trim_arm = core::mem::take(&mut discard);
    } else {
        trim_arm.push(T::give_back(None));
    }
    // Written as nested `if`s rather than a `switch`: the generated C is
    // compiled with `-Wswitch-enum` off but `-Wall -Wextra -Werror` on,
    // and a `switch` over an enum without a `default` is one warning
    // away from breaking every build the moment the enum grows.
    T::when(
        T::binary(
            "==",
            name::<T>("zd_tc"),
            T::constant(Constant::TryConstructTrim),
        ),
        T::block(trim_arm),
        Some(T::block(vec![T::when(
            T::binary(
                "==",
                name::<T>("zd_tc"),
                T::constant(Constant::TryConstructUseDefault),
            ),
            T::block(default_arm),
            Some(T::block(if discard.is_empty() {
                vec![refuse::<T>(Constant::StatusDiscarded), T::give_back(None)]
            } else {
                discard
            })),
        )])),
    )
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
            vec![T::store(
                deref::<T>("v"),
                T::value_type(ast, id),
                primitive_read_at::<T>(write),
            )]
        }

        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => {
            let mut statements = vec![assign::<T>(
                deref::<T>("v"),
                call_expr::<T>("read_string_alloc", Vec::new()),
            )];
            statements.extend(string_fit::<T>(ast, id, bound, CharWidth::Char)?);
            statements
        }
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => {
            let mut statements = vec![assign::<T>(
                deref::<T>("v"),
                match scalar_write(ast, id, key, WcharPosition::Member) {
                    Some(ScalarWrite::Wstr {
                        policy,
                        order,
                        code_units,
                    }) => match code_units {
                        WstringCodeUnits::Utf8 => call_expr::<T>(
                            "read_wstring_utf8_octets",
                            vec![T::constant(wstring_policy(policy))],
                        ),
                        WstringCodeUnits::Utf16 => call_expr::<T>(
                            "read_wstring_utf8",
                            vec![
                                T::constant(wstring_policy(policy)),
                                T::constant(wstring_unit_order(order)),
                            ],
                        ),
                    },
                    _ => {
                        return Err(EncodeError::NoWriter {
                            member: T::type_name(ast, id),
                            kind: "wstring the layout plan has no scalar write for",
                        });
                    }
                },
            )];
            statements.extend(string_fit::<T>(ast, id, bound, CharWidth::WChar)?);
            statements
        }

        TypeKind::Fixed { .. } => {
            let Some(ScalarWrite::Fixed {
                digits,
                scale,
                count,
            }) = scalar_write(ast, id, key, WcharPosition::Member)
            else {
                return Err(EncodeError::NoWriter {
                    member: T::type_name(ast, id),
                    kind: "fixed the layout plan has no scalar write for",
                });
            };
            vec![assign::<T>(
                deref::<T>("v"),
                call_expr::<T>(
                    "read_fixed",
                    vec![
                        int::<T>(i128::from(digits)),
                        int::<T>(i128::from(scale)),
                        T::constant(match count {
                            FixedDigitCount::Declared => Constant::FixedDeclared,
                            FixedDigitCount::ValuePrecision => Constant::FixedValuePrecision,
                            FixedDigitCount::ValueStringDigits => Constant::FixedValueStringDigits,
                        }),
                    ],
                ),
            )]
        }

        TypeKind::Enumeration { .. } => enumeration_body::<T>(ast, id, key),
        TypeKind::Bitmask { .. } => bitmask_body::<T>(ast, id),
        TypeKind::Bitset { .. } => {
            // The whole chain, base-first: see `encode_c`.
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
            let read = call_expr::<T>("read_bitset", vec![int::<T>(i128::from(anchor))]);
            // A bitset of padding alone still has its anchor on the wire,
            // and no field to read out of it: the octets are read and not
            // kept, where a local would be one no statement uses.
            let named = fields.iter().any(|field| {
                ast.bitfield(*field)
                    .is_ok_and(|field_node| field_node.name.is_some())
            });
            let mut statements = if named {
                vec![T::declare(T::scalar(Scalar::UInt64), "zd_bits", Some(read))]
            } else {
                vec![T::drop_value(read)]
            };
            let mut position: u32 = 0;
            for field in fields {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let width = u32::from(field_node.bits);
                if position.saturating_add(width) > 64 {
                    return Err(EncodeError::NoWriter {
                        member: own_name.to_string(),
                        kind: "bitset wider than the sixty-four bits an anchor holds",
                    });
                }
                // An anonymous field is padding: it moves the next
                // field along and carries no value of its own.
                if let Some(field_name) = field_node.name.as_ref() {
                    let mask: u64 = if width >= 64 {
                        u64::MAX
                    } else {
                        (1u64 << width) - 1
                    };
                    statements.push(assign::<T>(
                        arrow::<T>("v", &T::escape(field_name)),
                        // Into the member's own width and not into a
                        // `uint64_t`: the anchor is sixty-four bits and
                        // the field is as wide as `c_holder` made it,
                        // so the narrowing is a decision written out
                        // here rather than a conversion the compiler
                        // makes quietly. The mask above has already
                        // taken everything the field cannot hold.
                        cast::<T>(
                            T::unsigned_holder(field_node.bits),
                            T::paren(T::binary(
                                "&",
                                T::paren(T::binary(
                                    ">>",
                                    name::<T>("zd_bits"),
                                    // The shift distance in the anchor's
                                    // own width. C and Java shift a
                                    // 64-bit value by an `int` happily;
                                    // a TypeScript `bigint` shifted by a
                                    // `number` is a compile error, and
                                    // the distance is a property of the
                                    // anchor rather than of the field.
                                    T::wide_unsigned(u64::from(position)),
                                )),
                                T::wide_unsigned(mask),
                            )),
                        ),
                    ));
                }
                position = position.saturating_add(width);
            }
            statements
        }

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "typedef with an unresolved target",
                });
            };
            // A typedef is transparent on the wire and the C typedef
            // makes the two the same type, so the destination goes
            // straight through.
            // The behaviour goes through with the value: a typedef is
            // transparent on the wire and transparent to §7.2.2.7 too,
            // and the site that called this one is the site the bound
            // belongs to.
            let callee = decoder_name::<T>(ast, target);
            // Unless the alias and its target are one decoder. A target
            // that folds a `typedef` into the type it names gives the
            // two the same decoder name — Zig's `pub const NodeRef =
            // Node;` is the alias, so `Zig::type_name` answers `m_Node`
            // for both — and a forwarder to itself is not a forwarder:
            // `recursive-typedef-sequence` built
            // `fn zd_dec_m_Node(..) { … zd_dec_m_Node(..) }` and the
            // probe took SIGSEGV on the stack it ran out of. `None`
            // leaves the name unclaimed and the aliased type emits its
            // own body under it.
            if callee == own_name {
                return Ok(None);
            }
            vec![T::read_forward(&callee)]
        }

        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "sequence with an unresolved element",
                });
            };
            sequence_body::<T>(ast, id, element, bound, key)?
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
                lengths.push(ast.eval_integral(*dim).map_err(|_| EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "array whose length does not evaluate",
                })?);
            }
            // Table 17 gives an array no member-level behaviour — its
            // bounds must match the sender's, so only its elements can
            // fail — and there is no template syntax for its element
            // slot on a member declarator. `element_try_construct`
            // therefore lets the enclosing member's own annotation
            // reach the elements, and `zd_tc` is that annotation.
            array_body::<T>(
                ast,
                id,
                element,
                &lengths,
                key,
                ast.element_try_construct(id, None),
            )
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
            map_body::<T>(ast, id, k, value, bound, layout.needs_dheader, options)?
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            aggregate_body::<T>(ast, id, key)?
        }

        TypeKind::Union { discriminator, .. } => {
            union_body::<T>(ast, id, key, discriminator.resolved(), own_name)?
        }

        // No wire form at all, so there is nothing to read and nothing
        // is claimed.
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

/// An enumeration, matched on the bit pattern its holder carries.
///
/// The pattern and not the value, because the pattern is what a holder
/// narrower than the value's range leaves: Cyclone takes `@value(200)`
/// under `@bit_bound(8)` and writes `c8`, which read as a signed Int8 is
/// `-56`. Both readings name the same enumerator and only the pattern
/// says so. `vendor_bytes.md`, `enum_bb8_200`.
///
/// A pattern no enumerator has is refused rather than handed back as a
/// number: the value of an enumeration is a name, and there is no name
/// for this.
///
/// # The pattern is not the value the binding holds
///
/// The two part company above the holder's sign bit, and the corpus
/// caught it: `@value(200)` under `@bit_bound(8)` goes out as `c8`, and
/// `zdw_read_enum` sign-extends that to `-56` because §7.4.3.5.3 makes
/// the holder signed. The C binding stores the *declared* value, which
/// is what the fill side assigns and what the encoder writes, so a
/// decoder assigning the sign-extended pattern produces an integer no
/// enumerator constant equals — `enum-bitbound-8-value` came back as
/// `<enumerator -56>` where `BLUE` went in.
///
/// The Rust decoder never meets this: it matches on the pattern and
/// returns a name, so it has no in-memory integer to disagree about.
/// Each arm here therefore assigns the constant the materialiser
/// declared, by name, which is the same one the fill side uses.
fn enumeration_body<T: CodecTarget>(ast: &Ast, id: Id<TypeNode>, key: LayoutKey) -> Vec<T::Stmt> {
    // The reader takes the same axis the writer took, through the one
    // crossing function. `zerodds_ast_layout::enumeration_bit_bound`.
    let bits = zerodds_ast_layout::plan::enumeration_holder_bits(ast, id, key);
    let mut statements = vec![T::declare(
        T::scalar(Scalar::Int64),
        "zd_raw",
        Some(call_expr::<T>(
            "read_enum",
            vec![int::<T>(i128::from(bits))],
        )),
    )];

    // A pattern no enumerator has cannot construct an object of this
    // type — Table 18: "If the name or value of the T2 object does not
    // exist in T1, the object cannot construct any object of type T1" —
    // so it is the *position's* behaviour that decides. TRIM has no
    // meaning on an enumeration (Table 11) and the frontend refuses to
    // write it there, so the trim arm is empty and falls through to the
    // discard rather than inventing a nearest enumerator.
    let default_arm = crate::emit::default_enumerator_name(ast, id)
        .map(|literal| T::enumerator_constant(ast, id, &literal))
        .map(|constant| vec![assign::<T>(deref::<T>("v"), constant)])
        .unwrap_or_default();
    // Built from the last arm backwards, so the chain reads in
    // declaration order and the first enumerator with a given pattern
    // wins — which is the one the Rust reader also names.
    let mut chain: Option<T::Block> = Some(T::block(vec![narrow::<T>(Vec::new(), default_arm)]));
    let mut arms = Vec::new();
    // The same mapping the writer takes, from the same place: the
    // pattern on the wire is the enumerator's wire value and the
    // constant assigned is its declared one, and neither side resolves
    // it for itself.
    let resolved = zerodds_ast_layout::plan::enumerator_writes(ast, id, key).unwrap_or_default();
    for (name, value) in resolved {
        let Some(pattern) = enumerator_pattern(value, bits) else {
            continue;
        };
        arms.push((pattern, T::enumerator_constant(ast, id, &name)));
    }
    for (pattern, constant) in arms.into_iter().rev() {
        chain = Some(T::block(vec![T::when(
            T::binary("==", name::<T>("zd_raw"), T::wide_signed(pattern)),
            T::block(vec![assign::<T>(deref::<T>("v"), constant)]),
            chain,
        )]));
    }
    if let Some(chain) = chain {
        statements.extend(T::into_statements(chain));
    }
    statements
}

/// The signed value `zdw_read_enum` returns for an enumerator.
///
/// A second copy of the writer's rule and deliberately not a call into
/// it: the two directions are compared against each other, and a shared
/// helper would make them agree about a mistake. §7.4.3.5.3 makes the
/// holder signed, so the top bit of the bound reads back negative.
fn enumerator_pattern(value: i128, bits: u16) -> Option<i128> {
    if bits == 0 || bits > 32 {
        return None;
    }
    let width = i128::from(bits);
    if value < -(1i128 << (width - 1)) || value > (1i128 << width) - 1 {
        return None;
    }
    let masked = value & ((1i128 << width) - 1);
    Some(if masked >= (1i128 << (width - 1)) {
        masked - (1i128 << width)
    } else {
        masked
    })
}

/// A bitmask, reduced to the bits the type declares.
///
/// §7.4.1.1.3.2: "the states of the remaining serialized bits are not
/// specified, and those bits are not considered to be part of the
/// bitmask", and Table 18: "Preserve bit values by index for all bits
/// identified in both T1 and T2". A bit the stream sets and the type does
/// not name is not part of the value this type reads, so the read keeps
/// only the declared positions.
fn bitmask_body<T: CodecTarget>(ast: &Ast, id: Id<TypeNode>) -> Vec<T::Stmt> {
    let bound = ast.effective_bit_bound(id);
    let mut declared: u64 = 0;
    for bit in ast.bits_of(id) {
        let Some(position) = ast.effective_bit_position(*bit) else {
            continue;
        };
        if position < bound && position < 64 {
            declared |= 1u64 << position;
        }
    }
    vec![
        T::declare(
            T::scalar(Scalar::UInt64),
            "zd_bits",
            Some(T::paren(T::binary(
                "&",
                call_expr::<T>("read_bitmask", vec![int::<T>(i128::from(bound))]),
                // The holder's own width and not a bare number: both
                // sides of the `&` are sixty-four bits wide, and in a
                // target where that width is a separate type — a
                // TypeScript `bigint` — mixing it with a `number` is a
                // compile error rather than a wrong answer.
                T::wide_unsigned(declared),
            ))),
        ),
        // `T::bitmask_value` over `T::value_type` — the two are one cast
        // everywhere but TypeScript, whose `as` cannot narrow the
        // 64-bit read to a `number` holder; the trait doc carries it.
        // The value type and not `T::named(T::type_name(..))`: the two
        // are the same string in C, where a bitmask's type name *is* its
        // C type name, and are not the same anywhere else.
        // `Cpp::type_name` answers `a_b_Flags` — the tag a codec function
        // is named after — where the type is `::a::b::Flags`, so a cast
        // built from the tag named a type no C++ scope declares.
        assign::<T>(
            deref::<T>("v"),
            T::bitmask_value(ast, id, name::<T>("zd_bits")),
        ),
    ]
}

/// The limit a bound states, where it states one this column can read.
///
/// `None` for the unbounded form and for a bound that does not
/// evaluate; a negative one is not a length and is refused the same way.
/// One function because a `sequence`, a `string` and a `map` all read
/// their bound the same, and three copies would be three chances for
/// one collection to stop being checked while the others were.
fn bound_limit(ast: &Ast, bound: &zerodds_ast::Bound) -> Option<i128> {
    bound
        .expr()
        .and_then(|expr| ast.eval_integral(expr).ok())
        .filter(|limit| *limit >= 0)
}

/// A sequence: the count, then the elements into storage the reader
/// allocated.
fn sequence_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    element: Id<TypeNode>,
    bound: &zerodds_ast::Bound,
    key: LayoutKey,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let delimited = zerodds_ast_layout::collection_is_delimited(ast, element, key);
    let element_behaviour = ast.element_try_construct(id, None);
    let mut statements = Vec::new();
    if delimited {
        statements.push(discard_dheader::<T>());
    }
    statements.push(T::declare(
        T::scalar(Scalar::UInt32),
        "zd_n",
        Some(call_expr::<T>("read_u32", Vec::new())),
    ));
    // A count is one element or more, so a stream with fewer bytes left
    // than elements cannot be honest about it. Checked before the
    // allocation, which is how a reader turns a malformed message into
    // an out-of-memory when it is not.
    statements.push(T::when(
        T::binary(
            ">",
            cast::<T>(T::scalar(Scalar::Size), name::<T>("zd_n")),
            remaining::<T>(),
        ),
        T::block(vec![
            refuse::<T>(Constant::StatusImpossibleLength),
            T::give_back(None),
        ]),
        None,
    ));
    // The bound is applied after every element has been read and not
    // before: stopping at the bound would leave the rest of the
    // sequence in the stream and put every following member at the
    // wrong offset. `zd_over` is what the narrowing at the end reads.
    // §7.2.2.7, and OpenDDS does the same and then skips to the
    // collection's end.
    let limit = bound_limit(ast, bound);
    if let Some(limit) = limit {
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            "zd_over",
            Some(T::binary(">", name::<T>("zd_n"), T::unsigned_int(limit))),
        ));
    }
    let element_type = T::value_type(ast, element);
    // A run of octets is read in one piece where the target has the call
    // for it ([`CodecTarget::bulk_octets_read`]); the same octets as the
    // loop, with no per-element call.
    // The mirror of the writer: a `sequence` element of a non-`@final`
    // type is preceded by its own octet length under
    // `SequenceElementDelimiter::BeforeEachNonFinalElement`, and
    // `ElementPadding::Packed` counts every element's alignment from the
    // element's own first octet.
    let packed = key.element_padding == ElementPadding::Packed;
    let carries_length = crate::emit::encode::sequence_element_carries_length(ast, element, key)?;
    let fill = match crate::emit::encode::bulk_read::<T>(ast, element, name::<T>("zd_n")) {
        Some(bulk) => bulk,
        None => {
            // The element's place, and whatever the target needs around
            // it. See [`CodecTarget::sequence_element`]: every target but
            // one answers with the subscript and nothing else.
            let (before, place, after) =
                T::sequence_element("v", name::<T>("zd_i"), element_type.clone());
            let mut body: Vec<T::Stmt> = Vec::new();
            if packed {
                body.push(begin_packed::<T>());
            }
            if carries_length {
                body.extend(read_element_length::<T>());
            }
            body.extend(before);
            body.push(decode_element_into::<T>(
                ast,
                element,
                place,
                element_behaviour,
                key,
            ));
            body.extend(after);
            if carries_length {
                body.push(close_element_length::<T>());
            }
            if packed {
                body.push(finish_packed::<T>());
            }
            counted_loop::<T>("zd_i", name::<T>("zd_n"), T::block(body))
        }
    };
    statements.extend(T::make_room_of(
        ast,
        element,
        "v",
        element_type,
        name::<T>("zd_n"),
        true,
        vec![fill],
    ));
    if let Some(limit) = limit {
        statements.push(T::when(
            name::<T>("zd_over"),
            T::block(vec![narrow::<T>(
                // §7.3.1.2.1.12.3 keeps "the first four elements".
                // How a target says so is its own: C sets the
                // `_length` beside the buffer and leaves the elements
                // past the bound in it, unreferenced; a target whose
                // collection owns its storage has to shorten the
                // collection, and `v.size() = 5u` is not an
                // assignment at all.
                T::keep_first("v", T::unsigned_int(limit)),
                // Table 9, through this column's one derivation of
                // it, which for a sequence is the empty one.
                vec![T::default_into(ast, id, deref::<T>("v"))?],
            )]),
            None,
        ));
    }
    Ok(statements)
}

/// The narrowing a bounded string needs once it has been read.
///
/// Nothing at all for an unbounded one: `zdw_read_string_alloc` has
/// already produced the value and there is no bound to miss.
fn string_fit<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    bound: &zerodds_ast::Bound,
    width: CharWidth,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let Some(limit) = bound_limit(ast, bound) else {
        return Ok(Vec::new());
    };
    // Octets for a narrow string and UTF-16 code units for a wide one,
    // which is what the bound means to every peer that has to hold the
    // value — `string<5>` is `char[6]` in the C mapping of all four
    // measured implementations.
    //
    // Both are the runtime's: the measure has no receiver and the trim
    // takes one, which is exactly the difference between
    // [`CodecTarget::runtime_static`] and [`CodecTarget::runtime`]. They
    // were spelled `zdw_utf8_octets` and `zdw_trim_utf8` through
    // [`CodecTarget::call`] until a second reader column asked for them,
    // and `zdw_` is not a name any other runtime answers to.
    let (measure, trim) = match width {
        CharWidth::Char => ("utf8_octets", "trim_utf8"),
        CharWidth::WChar => ("utf16_units", "trim_utf16"),
    };
    // `(size_t)` written as a cast and not inside the literal, so that a
    // target with no C-style cast writes its own. C renders the two the
    // same.
    //
    // The number goes through `unsigned_int` and not through a
    // `format!("{limit}u")`: the `u` suffix is C's spelling of an
    // unsigned literal, and a walk that wrote it reached `32u` into a
    // Java probe, where `javac` calls it a syntax error. Every target
    // spells its own.
    let fits = || cast::<T>(T::scalar(Scalar::Size), T::unsigned_int(limit));
    let too_long = T::binary(
        ">",
        T::runtime_static(measure, vec![deref::<T>("v")]),
        fits(),
    );
    Ok(vec![T::when(
        // The value may not be there at all in a target whose reader
        // allocates and can fail. Where it cannot, the guard is not
        // written rather than written against a value that is always
        // present: `std::string != std::nullopt` does not compile.
        match T::materialised(deref::<T>("v")) {
            Some(present) => T::binary("&&", present, too_long),
            None => too_long,
        },
        T::block(vec![narrow::<T>(
            vec![assign::<T>(
                deref::<T>("v"),
                T::runtime("r", trim, vec![deref::<T>("v"), fits()]),
            )],
            // Table 9, through the one derivation of it this column
            // has. `default_c::try_construct_seeds` is what makes
            // sure the function exists.
            vec![T::default_into(ast, id, deref::<T>("v"))?],
        )]),
        None,
    )])
}

/// An array: the element count is in the type, so only the elements are
/// on the wire. §7.4.3.5 treats a multi-dimensional array as one flat
/// run with the first index varying most slowly, which is what nested
/// loops in written order already are.
fn array_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    element: Id<TypeNode>,
    lengths: &[i128],
    key: LayoutKey,
    element_behaviour: TryConstruct,
) -> Vec<T::Stmt> {
    let _ = id;
    let delimited = zerodds_ast_layout::array_is_delimited(ast, element, key);
    if let [length] = lengths {
        if let Some(bulk) =
            crate::emit::encode::bulk_read::<T>(ast, element, T::unsigned_int(*length))
        {
            let mut statements = Vec::new();
            if delimited {
                statements.push(discard_dheader::<T>());
            }
            statements.extend(T::open_array("v", lengths));
            statements.push(bulk);
            statements.extend(T::close_array("v", lengths));
            return statements;
        }
    }
    let mut access = name::<T>("v");
    for level in 0..lengths.len() {
        access = T::index(access, name::<T>(&format!("zd_i{level}")));
    }
    let mut inner = T::block(vec![decode_element_into::<T>(
        ast,
        element,
        access,
        element_behaviour,
        key,
    )]);
    // `ElementPadding::Packed`: each element of every dimension is its
    // own alignment block, as in the reference's `read_array_dimension`.
    let packed = key.element_padding == ElementPadding::Packed;
    for (level, length) in lengths.iter().enumerate().rev() {
        let mut body: Vec<T::Stmt> = Vec::new();
        if packed {
            body.push(begin_packed::<T>());
        }
        body.extend(T::into_statements(inner));
        if packed {
            body.push(finish_packed::<T>());
        }
        inner = T::block(vec![counted_loop::<T>(
            &format!("zd_i{level}"),
            T::unsigned_int(*length),
            T::block(body),
        )]);
    }
    let mut statements = Vec::new();
    if delimited {
        statements.push(discard_dheader::<T>());
    }
    statements.extend(T::open_array("v", lengths));
    statements.extend(T::into_statements(inner));
    statements.extend(T::close_array("v", lengths));
    statements
}

/// A map: §7.4.1.1.4's sequence of entries, in whichever of the two
/// readings the caller chose.
///
/// Under `rule-15-omits` a delimited map states no count, and the
/// delimiter's extent is the only thing that says where the entries
/// stop. The generated code then reads the entries twice: once into a
/// scratch entry to count them, then again into storage sized from that
/// count. `zdw_reader_restore` puts back both the cursor and the heap,
/// so the first pass leaves nothing behind.
///
/// This is the only reader mark left in the generated code. The other
/// one was the `@optional` guess, which rule (20) and §7.4.1.2 retired.
///
/// The bound is checked exactly as [`sequence_body`] checks a
/// sequence's, and for the same clauses: §7.2.2.7 makes an entry count
/// over the bound an invalid sample, and §7.3.1.2.1.12.3's TRIM keeps
/// the first `bound` entries. It is applied after every entry has been
/// read and not before: stopping at the bound would leave the rest of
/// the map in the stream and put every following member at the wrong
/// offset.
///
/// It was missing entirely: `map<long, long, 4>` read as many entries
/// as the stream stated in every one of the twenty columns, so a
/// sample the IDL does not describe reached the application with
/// nothing said about it. `sequence<long, 4>` beside it in the same
/// struct was checked, which is what made the gap visible.
///
/// # Errors
/// [`EncodeError`] when this column cannot spell Table 9's value for
/// the map — the same refusal a bounded sequence's TRIM raises.
fn map_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key_type: Id<TypeNode>,
    value_type_: Id<TypeNode>,
    bound: &zerodds_ast::Bound,
    delimited: bool,
    options: CodecOptions,
) -> Result<Vec<T::Stmt>, EncodeError> {
    // Table 17 gives a map's key no escape — a key that cannot be
    // constructed fails the whole map — and the frontend refuses
    // anything but DISCARD there. The value has its own slot.
    let key_behaviour = ast.key_try_construct(id);
    let value_behaviour = ast.element_try_construct(id, None);
    let entry_type = T::map_entry_type(ast, id);
    let counted = !delimited || options.map_length_with_delimiter;
    let mut statements = Vec::new();

    if delimited {
        statements.push(T::declare(
            T::scalar(Scalar::UInt32),
            "zd_dl",
            Some(call_expr::<T>("read_dheader", Vec::new())),
        ));
        statements.push(T::declare(
            T::scalar(Scalar::Size),
            "zd_end",
            Some(offset_plus::<T>(position::<T>(), name::<T>("zd_dl"))),
        ));
    }

    let read_entry = |index: T::Expr| {
        T::block(vec![
            decode_into::<T>(
                ast,
                key_type,
                T::entry_half(T::index(T::elements("v"), index.clone()), true),
                key_behaviour,
            ),
            decode_into::<T>(
                ast,
                value_type_,
                T::entry_half(T::index(T::elements("v"), index), false),
                value_behaviour,
            ),
        ])
    };

    if counted {
        statements.push(T::declare(
            T::scalar(Scalar::UInt32),
            "zd_n",
            Some(call_expr::<T>("read_u32", Vec::new())),
        ));
    } else {
        // The counting pass. A scratch entry rather than a discard,
        // because an entry may hold a string and the decoder has to
        // have somewhere to put one.
        statements.push(T::storage(entry_type.clone(), "zd_scratch"));
        statements.push(T::declare(
            T::handle(Handle::ReaderMark),
            "zd_mark",
            Some(call_expr::<T>("reader_save", Vec::new())),
        ));
        statements.push(T::declare(
            T::scalar(Scalar::UInt32),
            "zd_n",
            Some(T::unsigned_int(0)),
        ));
        statements.push(T::until(
            T::binary(
                "&&",
                T::binary("<", position::<T>(), name::<T>("zd_end")),
                status_is_ok::<T>(),
            ),
            T::block(vec![
                decode_into::<T>(
                    ast,
                    key_type,
                    T::entry_half(name::<T>("zd_scratch"), true),
                    key_behaviour,
                ),
                decode_into::<T>(
                    ast,
                    value_type_,
                    T::entry_half(name::<T>("zd_scratch"), false),
                    value_behaviour,
                ),
                assign::<T>(
                    name::<T>("zd_n"),
                    T::binary("+", name::<T>("zd_n"), T::unsigned_int(1)),
                ),
            ]),
        ));
        statements.push(T::perform(call_expr::<T>(
            "reader_restore",
            vec![name::<T>("zd_mark")],
        )));
    }

    statements.push(T::when(
        T::binary(
            ">",
            cast::<T>(T::scalar(Scalar::Size), name::<T>("zd_n")),
            remaining::<T>(),
        ),
        T::block(vec![
            refuse::<T>(Constant::StatusImpossibleLength),
            T::give_back(None),
        ]),
        None,
    ));
    // The entry count against the bound, read before the entries are
    // and acted on after them. `zd_over` is the same local a bounded
    // sequence declares, because it is the same rule over the same
    // count.
    let limit = bound_limit(ast, bound);
    if let Some(limit) = limit {
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            "zd_over",
            Some(T::binary(">", name::<T>("zd_n"), T::unsigned_int(limit))),
        ));
    }
    statements.extend(T::make_room(
        "v",
        entry_type,
        name::<T>("zd_n"),
        false,
        vec![counted_loop::<T>(
            "zd_i",
            name::<T>("zd_n"),
            read_entry(name::<T>("zd_i")),
        )],
    ));
    // Ahead of the narrowing below and not after it: every arm of
    // [`narrow`] leaves the routine, so a map that had to be trimmed
    // would return with the cursor short of the delimiter's end and put
    // every following member at the wrong offset. The cursor is put
    // where the delimiter says it belongs first, and what the sample
    // holds is decided after.
    if delimited {
        statements.push(T::perform(call_expr::<T>(
            "reader_skip_to",
            vec![name::<T>("zd_end")],
        )));
    }
    if let Some(limit) = limit {
        statements.push(T::when(
            name::<T>("zd_over"),
            T::block(vec![narrow::<T>(
                // §7.3.1.2.1.12.3 keeps "the first four elements", and
                // an entry is what a map's element is (§7.4.1.1.4).
                // How a column shortens the collection is its own; the
                // walk says how many to keep.
                T::keep_first("v", T::unsigned_int(limit)),
                // Table 9, through this column's one derivation of it,
                // which for a map is the empty one.
                vec![T::default_into(ast, id, deref::<T>("v"))?],
            )]),
            None,
        ));
    }
    Ok(statements)
}

/// One member of an aggregate, as the plan and the model together see
/// it.
struct Slot<T: CodecTarget> {
    field: String,
    type_: Id<TypeNode>,
    optional: bool,
    /// Whether the member is `@external` — stored behind the column's
    /// own indirection, where it keeps one. Table 21: no octet changes;
    /// which seam the access routes through does.
    external: bool,
    /// Whether the member's value has no room inside the aggregate that
    /// declares it — [`zerodds_ast_layout::member_held_inline_closes_a_cycle`].
    /// A column whose member is the value has to keep an indirection
    /// there whatever the annotations say.
    indirect: bool,
    /// Whether the column keeps the member behind a reference of its
    /// own making — [`CodecTarget::holds_by_reference`].
    by_reference: bool,
    member_id: u32,
    header: MemberHeader,
    /// The declaration, for the `@default` it may carry.
    member: Id<zerodds_ast::MemberNode>,
    /// The behaviour governing this member's value, already resolved
    /// through the alias chain of its type. §7.2.2.7.
    try_construct: TryConstruct,
    /// The statements that assign this member's explicit `@default`,
    /// when it has one and its behaviour is `USE_DEFAULT`.
    ///
    /// The type's decoder cannot answer that combination: it knows
    /// Table 9's value for the type and not the literal this member's
    /// declaration gave it, and §7.2.2.4.4.4.9 puts the literal first.
    /// So the call site asks for DISCARD and assigns the literal where
    /// it gets one.
    ///
    /// `None` where the member has no `@default` or does not take
    /// `USE_DEFAULT`; a literal this column cannot spell is a refusal
    /// and never a `None`.
    explicit_default: Option<Vec<T::Stmt>>,
}

fn slots<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    layout: &zerodds_ast_layout::TypeLayout,
) -> Result<Vec<Slot<T>>, EncodeError> {
    let all = ast.all_members_of(id);
    let mut out = Vec::with_capacity(layout.members.len());
    for member in &layout.members {
        let mut found = None;
        for candidate in &all {
            let Ok(node) = ast.member(*candidate) else {
                continue;
            };
            if node.node_id == member.member {
                found = Some((*candidate, node));
                break;
            }
        }
        let Some((handle, node)) = found else {
            continue;
        };
        let Some(member_type) = node.type_.resolved() else {
            continue;
        };
        let try_construct = ast.effective_try_construct(
            node.facets.xtypes.as_ref().and_then(|x| x.try_construct),
            Some(member_type),
        );
        // Where the member is reached from: the aggregate, or the base that
        // declares it where the column nests the base as `parent`.
        let reach = super::reach::<T>(ast, id, member.member, &node.name);
        let explicit_default = if try_construct == TryConstruct::UseDefault
            && node
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|x| x.default_value.is_some())
        {
            // Reported and not swallowed: a `@default` this column
            // cannot spell would otherwise leave the member taking
            // Table 9's zero, which is a different sample from the one
            // the designer declared.
            Some(T::member_default(
                ast,
                node,
                member_type,
                arrow::<T>("v", &reach.field),
            )?)
        } else {
            None
        };
        let optional = node.facets.is_optional();
        let external = super::reached_externally::<T>(ast, node);
        out.push(Slot::<T> {
            indirect: super::behind_pointer::<T>(
                ast,
                reach.holder,
                member_type,
                external,
                optional,
            ),
            by_reference: T::holds_by_reference(ast, reach.holder, member_type, optional, external),
            field: reach.field,
            type_: member_type,
            optional,
            external,
            member_id: member.member_id,
            header: member.header,
            member: handle,
            try_construct,
            explicit_default,
        });
    }
    Ok(out)
}

/// The reader's destination for one slot's member, through the seams
/// its annotations and its type pick: `@external` members go through
/// [`CodecTarget::external_destination`], `@optional` ones through
/// [`CodecTarget::optional_destination`], and a plain member is its
/// field — and any of the three goes on through
/// [`CodecTarget::make_indirection`] where the member's value has no
/// room inside the aggregate that declares it.
fn slot_destination<T: CodecTarget>(ast: &Ast, slot: &Slot<T>) -> (Vec<T::Stmt>, T::Expr) {
    if slot.by_reference {
        return T::reference_destination(ast, slot.type_, "v", &slot.field);
    }
    let (mut setup, place) = if slot.external {
        T::external_destination("v", &slot.field, slot.optional)
    } else if slot.optional {
        T::optional_destination("v", &slot.field)
    } else {
        (Vec::new(), arrow::<T>("v", &slot.field))
    };
    // A member with no room inside its own aggregate is reached through
    // the column's indirection, and the indirection points nowhere until
    // this makes it point somewhere.
    if slot.indirect {
        let (made, through) = T::make_indirection(ast, slot.type_, place);
        setup.extend(made);
        return (setup, through);
    }
    (setup, place)
}

/// Where a slot's Table 9 default goes, and what it takes to have
/// somewhere to put it.
///
/// The same seam [`slot_destination`] uses, for the write a *missing*
/// member gets: a column that keeps an indirection has nothing to
/// default into until the indirection points somewhere. `place` is the
/// answer for every member the column holds inline, which is every
/// member this does not report.
fn slot_default_place<T: CodecTarget>(
    ast: &Ast,
    slot: &Slot<T>,
    place: T::Expr,
) -> (Vec<T::Stmt>, T::Expr) {
    if slot.indirect {
        return T::make_indirection(ast, slot.type_, place);
    }
    (Vec::new(), place)
}

/// Say a slot's `@optional` member is present or absent, through the
/// external seam where the member is also `@external`.
fn slot_set_present<T: CodecTarget>(slot: &Slot<T>, present: bool) -> Vec<T::Stmt> {
    // Held by reference: the allocation `reference_destination` makes
    // is what makes the member present, and absence is the reference's
    // own null.
    if slot.by_reference {
        return if present {
            Vec::new()
        } else {
            T::reference_absent("v", &slot.field)
        };
    }
    if slot.external {
        return T::external_set_present("v", &slot.field, present);
    }
    T::set_present("v", &slot.field, present)
}

/// A structure or a valuetype, framed as its plan says.
///
/// Three shapes, and which one applies is the extensibility, read the
/// same way the planner reads it rather than inferred back out of the
/// plan it produced.
fn aggregate_body<T: CodecTarget>(
    ast: &Ast,
    id: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut deps = DepCollector::default();
    let layout = zerodds_ast_layout::xcdr::plan(ast, id, key, &mut deps)
        .map_err(crate::encode::plan_refusal)?;
    let members = slots::<T>(ast, id, &layout)?;
    let extensibility = ast.effective_extensibility(id);
    let mutable = extensibility == Extensibility::Mutable;

    let mut statements = Vec::new();
    // `ElementPadding::Packed`: an aggregate is its own alignment block
    // on the read side too. See the writer's `write_aggregate`.
    let packed = key.element_padding == ElementPadding::Packed;
    if packed {
        statements.push(begin_packed::<T>());
    }
    if layout.needs_dheader {
        statements.extend(read_delimiter::<T>());
    }
    // An `@optional` member's presence flag is set by this decoder and
    // never left at whatever the caller's storage held.
    for slot in &members {
        if slot.optional {
            statements.extend(slot_set_present::<T>(slot, false));
        }
    }

    if mutable && key.encoding.is_v1() {
        statements.extend(parameter_list::<T>(ast, &members)?);
    } else if mutable {
        if !layout.needs_dheader {
            return Err(EncodeError::NoLayout {
                detail: "a mutable type under XCDR2 with no DHEADER planned".to_string(),
            });
        }
        statements.extend(member_headers::<T>(ast, &members)?);
    } else {
        let tail = Tail {
            delimited: layout.needs_dheader,
            appendable: extensibility == Extensibility::Appendable,
        };
        statements.extend(plan_order::<T>(ast, &members, tail)?);
    }

    if layout.needs_dheader {
        statements.extend(close_delimiter::<T>());
    }
    if packed {
        statements.push(finish_packed::<T>());
    }
    Ok(statements)
}

/// `uint32_t zd_dl = …; size_t zd_end = position + zd_dl;`
fn read_delimiter<T: CodecTarget>() -> Vec<T::Stmt> {
    // Two statements and not one expression: C leaves the order of
    // evaluation between the call and `zdw_reader_position` unspecified,
    // so an end computed in one line would depend on the compiler.
    vec![
        T::declare(
            T::scalar(Scalar::UInt32),
            "zd_dl",
            Some(call_expr::<T>("read_dheader", Vec::new())),
        ),
        T::declare(
            T::scalar(Scalar::Size),
            "zd_end",
            Some(offset_plus::<T>(position::<T>(), name::<T>("zd_dl"))),
        ),
    ]
}

/// What the delimiter is checked against once the members are read.
///
/// The tail inside a delimiter is what a peer with fewer members than
/// the writer had leaves behind, and skipping it is what the DHEADER is
/// for.
///
/// It used to carry a second check as well: bytes left inside the
/// delimiter after every declared member had been read falsified a
/// guess that an `@optional` member was absent. There is no guess left
/// to falsify — rule (20) and §7.4.1.2 mark the gap and this column
/// reads the marker — so the check has nothing to catch and the
/// `zd_guessed` flag it needed is gone with it.
fn close_delimiter<T: CodecTarget>() -> Vec<T::Stmt> {
    let statements = vec![T::when(
        T::binary(">", position::<T>(), name::<T>("zd_end")),
        T::block(vec![refuse::<T>(Constant::StatusOutOfRange)]),
        None,
    )];
    let mut statements = statements;
    statements.push(T::perform(call_expr::<T>(
        "reader_skip_to",
        vec![name::<T>("zd_end")],
    )));
    statements
}

/// The local a body's packed-element origin is saved in.
const PACKED_SAVED: &str = "zd_saved";

/// `size_t zd_saved = reader_begin_packed_element(r);`
///
/// [`ElementPadding::Packed`] is HDDS's measured departure from §7.4.1:
/// a collection element and an aggregate value count their alignment
/// from their own first octet instead of from the encapsulation. The
/// generated codec holds the displaced origin and restores it with
/// [`finish_packed`]. The `reader_` prefix is the runtime's own, as for
/// `reader_save` and `reader_position`, and it is what keeps the read
/// direction's pair distinct from the write direction's.
fn begin_packed<T: CodecTarget>() -> T::Stmt {
    T::declare(
        T::scalar(Scalar::Size),
        PACKED_SAVED,
        Some(call_expr::<T>("reader_begin_packed_element", Vec::new())),
    )
}

/// `reader_finish_packed_element(r, zd_saved);`
fn finish_packed<T: CodecTarget>() -> T::Stmt {
    T::perform(call_expr::<T>(
        "reader_finish_packed_element",
        vec![name::<T>(PACKED_SAVED)],
    ))
}

/// The element's own octet length a `sequence` element of a non-`@final`
/// type carries.
///
/// [`zerodds_ast_layout::key::SequenceElementDelimiter::BeforeEachNonFinalElement`]
/// writes a four-octet `UInt32` in front of every element. The reader
/// checks the element against it rather than skipping to it: an element
/// longer or shorter than its own declaration is a stream that disagrees
/// with itself, and seeking past it would hide the disagreement.
fn read_element_length<T: CodecTarget>() -> Vec<T::Stmt> {
    vec![
        T::declare(
            T::scalar(Scalar::UInt32),
            "zd_ell",
            Some(call_expr::<T>("read_u32", Vec::new())),
        ),
        T::declare(
            T::scalar(Scalar::Size),
            "zd_elend",
            Some(offset_plus::<T>(position::<T>(), name::<T>("zd_ell"))),
        ),
    ]
}

fn close_element_length<T: CodecTarget>() -> T::Stmt {
    ends_where_the_header_said::<T>("zd_elend")
}

/// A `@final` or `@appendable` aggregate: the members in plan order.
///
/// Every member is where the plan says it is, including an `@optional`
/// one. §7.4.3.5.2 rule (20) puts a BOOLEAN `<is_present>` in front of
/// it under XCDR2 and §7.4.1.2 gives it a parameter header with a
/// declared length of zero under XCDR1, and all three implementations
/// write both — so nothing here is guessed and nothing rewinds.
///
/// This used to read the header against a `zdw_reader_mark` and rewind
/// when the member id was not the one expected. Against a stream that
/// marks the gap that guess reads the marker as a header, calls the
/// member absent, and takes the *next* member's bytes out of this
/// member's place.
fn plan_order<T: CodecTarget>(
    ast: &Ast,
    members: &[Slot<T>],
    tail: Tail,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut statements = Vec::new();
    // `!members.is_empty()`, because the latch is read only inside the
    // loop below and a declaration nothing reads is a
    // `-Wunused-variable` error in a probe compiled with `-Werror`. An
    // aggregate with no members reaches this only in a column whose
    // materialiser can declare one: C refuses `struct S {}` outright —
    // C23 §6.7.2.1p8 leaves a structure with no named member undefined —
    // and C++ declares it, so the C column never saw the shape.
    if tail.possible() && !members.is_empty() {
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            "zd_tail",
            Some(literal::<T>("false")),
        ));
    }
    for (index, slot) in members.iter().enumerate() {
        let place = arrow::<T>("v", &slot.field);
        if tail.possible() {
            // Once the writer's members have run out, every member from
            // here on takes its default rather than the bytes of
            // whatever follows. Latched, so a member that happens to
            // start at the delimiter's end does not un-set it.
            //
            // The latch half is left off the **first** member: the
            // declaration above sets `zd_tail` to `false` on the line
            // before, so `!zd_tail` there is true by construction. It
            // is not a style point. Elixir threads a rebinding out of a
            // block through a tuple, which is enough for its type
            // checker to flow-type the literal and report the guard as
            // a conditional that always holds — a warning the codec
            // gate compiles with warnings as errors. The member reads
            // stay uniform, which is what the shared walk is for; only
            // the one conjunct that cannot be false is gone.
            let reached = if index == 0 {
                T::paren(tail.reached::<T>())
            } else {
                T::binary(
                    "&&",
                    T::unary("!", name::<T>("zd_tail")),
                    T::paren(tail.reached::<T>()),
                )
            };
            statements.push(T::when(
                reached,
                T::block(vec![assign::<T>(
                    name::<T>("zd_tail"),
                    literal::<T>("true"),
                )]),
                None,
            ));
            let mut defaulted = Vec::new();
            if slot.optional {
                defaulted.extend(slot_set_present::<T>(slot, false));
            } else {
                let node = ast
                    .member(slot.member)
                    .map_err(|error| EncodeError::NoLayout {
                        detail: format!("{}: {error}", slot.field),
                    })?;
                let (room, into) = slot_default_place::<T>(ast, slot, place.clone());
                defaulted.extend(room);
                defaulted.extend(T::member_default(ast, node, slot.type_, into)?);
            }
            let read = plan_order_member::<T>(ast, slot, index, place)?;
            statements.push(T::when(
                name::<T>("zd_tail"),
                T::block(defaulted),
                Some(T::block(read)),
            ));
            continue;
        }
        statements.extend(plan_order_member::<T>(ast, slot, index, place)?);
    }
    Ok(statements)
}

/// Where a positional aggregate's members stop, which is not always
/// where this reader's plan stops.
///
/// §7.2.4.4.8, Table 19: "Each non-optional member in a T1 object that
/// is not present in the T2 object takes its default value." The two
/// ways a writer's members can run out are the two shapes §7.2.4.2
/// gives an appendable type — delimited under encoding version 2, and
/// not delimited under version 1.
///
/// `@final` gets neither. Table 19 requires the same set of member IDs
/// on both sides of an assignment between final types, so a short stream
/// is a type mismatch: Cyclone and RTI refuse it and Fast DDS defaults,
/// measured 2026-08-09 as `evo-final-tail`, and the clause is with the
/// two.
#[derive(Debug, Clone, Copy)]
struct Tail {
    /// Whether the type carries a DHEADER, which says where the writer
    /// stopped.
    delimited: bool,
    /// Whether the aggregate is `@appendable`.
    appendable: bool,
}

impl Tail {
    /// Whether this aggregate can meet a writer with fewer members.
    const fn possible(self) -> bool {
        self.appendable
    }

    /// The C condition that says the writer's members have run out.
    ///
    /// Under XCDR2 the DHEADER is exactly that statement. Under XCDR1
    /// there is none, and all three vendors default the tail when the
    /// bytes run out instead — `evo-appendable-tail` under `xcdr1`,
    /// where Cyclone, RTI and Fast DDS each hand back `c = 0`. The test
    /// is `remaining() == 0` and not "fewer bytes than the member
    /// needs": an appendable type nested inside another still has the
    /// enclosing type's bytes behind it, so nothing fires and the read
    /// fails loudly further on, which is what Cyclone and RTI do with
    /// the nested case.
    fn reached<T: CodecTarget>(self) -> T::Expr {
        if self.delimited {
            return T::binary(">=", position::<T>(), name::<T>("zd_end"));
        }
        // A plain zero and not [`CodecTarget::unsigned_int`]'s count:
        // `reader_remaining` answers in the target's own size type —
        // `int` in the C#, Go, Java and F# runtimes, `usize` in Zig's —
        // and a count literal beside one is a comparison between two
        // widths. C# and Go let it through by promotion; F# does not,
        // and `r.ReaderRemaining() = 0u` is `error FS0001` there.
        T::binary("==", remaining::<T>(), T::int(0))
    }
}

/// One member of a positional aggregate, read where the plan says.
fn plan_order_member<T: CodecTarget>(
    ast: &Ast,
    slot: &Slot<T>,
    index: usize,
    place: T::Expr,
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut statements = Vec::new();
    {
        match slot.header {
            MemberHeader::NoHeader => {
                if slot.optional {
                    let (room, into) = slot_destination::<T>(ast, slot);
                    statements.extend(room);
                    statements.extend(member_read::<T>(ast, slot, into));
                    statements.extend(slot_set_present::<T>(slot, true));
                } else if slot.external || slot.indirect {
                    // `slot_destination` answers `place` itself for a
                    // member the column holds inline, so this arm and
                    // the one below differ only where the column keeps
                    // something the read has to make first.
                    let (room, into) = slot_destination::<T>(ast, slot);
                    statements.extend(room);
                    statements.extend(member_read::<T>(ast, slot, into));
                } else {
                    statements.extend(member_read::<T>(ast, slot, place));
                }
            }
            MemberHeader::OptionalFlag => statements.extend(present_flag::<T>(ast, slot)),
            MemberHeader::OptionalCdr1 { .. } => {
                statements.extend(optional_parameter::<T>(ast, slot, index));
            }
            // The planner gives these two only to a member of a
            // `@mutable` type, which one of the two functions below
            // reads. A body that emitted them here would frame the
            // member one way and read it another.
            MemberHeader::EmHeader { .. } | MemberHeader::PidCdr1 { .. } => {
                return Err(EncodeError::NoLayout {
                    detail: format!(
                        "{}: an MMEMBER header in a type that is not `@mutable`",
                        slot.field
                    ),
                });
            }
        }
    }
    Ok(statements)
}

/// Rule (20): the BOOLEAN `<is_present>`, then the value behind it.
///
/// The flag lands in a local and the branches say what the structure
/// holds, rather than the flag being assigned into the structure
/// directly: a target where absence *is* the value has nothing to
/// assign to — `v.x != null` is not somewhere a `boolean` can be put.
/// `zdw_read_bool` refuses a byte that is neither 0 nor 1, which is
/// what keeps a mis-framed stream from reading as "present".
fn present_flag<T: CodecTarget>(ast: &Ast, slot: &Slot<T>) -> Vec<T::Stmt> {
    let flag = format!("zd_has_{}", slot.field);
    let mut arrived = slot_set_present::<T>(slot, true);
    let (room, into) = slot_destination::<T>(ast, slot);
    arrived.extend(room);
    arrived.extend(member_read::<T>(ast, slot, into));
    vec![
        T::declare(
            T::scalar(Scalar::Bool),
            &flag,
            Some(call_expr::<T>("read_bool", Vec::new())),
        ),
        T::when(
            name::<T>(&flag),
            T::block(arrived),
            Some(T::block(slot_set_present::<T>(slot, false))),
        ),
    ]
}

/// §7.4.1.2: the parameter header, whose declared length is zero when
/// the member took no value.
///
/// A header for another id is a refusal and not a rewind. The member is
/// not optional in the sense of "may be missing from the stream" — the
/// header is always there — so an id that is not this one means the
/// reader and the writer disagree about the type, and every byte after
/// it would come out of the wrong place.
fn optional_parameter<T: CodecTarget>(ast: &Ast, slot: &Slot<T>, index: usize) -> Vec<T::Stmt> {
    let header = format!("zd_p{index}");
    let flag = format!("zd_has_{}", slot.field);
    let mine = T::binary(
        "&&",
        T::paren(status_is_ok::<T>()),
        T::paren(T::binary(
            "&&",
            T::paren(T::binary(
                "==",
                T::header_field(name::<T>(&header), "pid"),
                T::unsigned_int(i128::from(slot.member_id)),
            )),
            T::paren(T::binary(
                "==",
                T::header_field(name::<T>(&header), "end"),
                literal::<T>("false"),
            )),
        )),
    );
    let mut arrived = slot_set_present::<T>(slot, true);
    let (room, into) = slot_destination::<T>(ast, slot);
    arrived.extend(room);
    arrived.extend(member_read::<T>(ast, slot, into));
    vec![
        T::declare(
            T::handle(Handle::PidRead),
            &header,
            Some(call_expr::<T>(
                "read_pid_as",
                vec![T::constant(Constant::ParameterOptional)],
            )),
        ),
        T::when(
            mine,
            T::block(vec![
                T::declare(
                    T::scalar(Scalar::Bool),
                    &flag,
                    Some(T::binary(
                        "!=",
                        T::header_field(name::<T>(&header), "length"),
                        T::unsigned_int(0),
                    )),
                ),
                T::when(
                    name::<T>(&flag),
                    T::block(arrived),
                    Some(T::block(slot_set_present::<T>(slot, false))),
                ),
                T::perform(call_expr::<T>("finish_pid_read", vec![name::<T>(&header)])),
            ]),
            Some(T::block(vec![
                refuse::<T>(Constant::StatusOutOfRange),
                T::give_back(None),
            ])),
        ),
    ]
}

/// A `@mutable` aggregate under XCDR2: EMHEADERs until the delimiter's
/// extent is used up.
///
/// Driven by the headers and not by the plan, because that is what the
/// headers are for.
fn member_headers<T: CodecTarget>(
    ast: &Ast,
    members: &[Slot<T>],
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut statements = Vec::new();
    for (index, slot) in members.iter().enumerate() {
        let _ = slot;
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            &format!("zd_seen{index}"),
            Some(literal::<T>("false")),
        ));
    }
    statements.push(T::declare(T::handle(Handle::EmHeaderRead), "zd_h", None));
    statements.push(T::declare(
        T::scalar(Scalar::Size),
        "zd_ends",
        Some(literal::<T>("0")),
    ));

    // The unknown-member arm, built first so the known ones can chain
    // in front of it. §7.4.3.5.3: a reader that cannot skip a member it
    // must understand may not pretend it did.
    let mut chain = Some(T::block(vec![T::when(
        T::header_field(name::<T>("zd_h"), "must_understand"),
        T::block(vec![refuse::<T>(Constant::StatusOutOfRange)]),
        Some(T::block(vec![T::perform(call_expr::<T>(
            "reader_skip_to",
            vec![name::<T>("zd_ends")],
        ))])),
    )]));

    for (index, slot) in members.iter().enumerate().rev() {
        let seen = format!("zd_seen{index}");
        // §7.4.3.4.2 gives each member one header, so a second one for
        // the same id is a stream that says two things.
        let mut arm = vec![T::when(
            name::<T>(&seen),
            T::block(vec![
                refuse::<T>(Constant::StatusOutOfRange),
                T::give_back(None),
            ]),
            None,
        )];
        arm.push(assign::<T>(name::<T>(&seen), literal::<T>("true")));
        if slot.optional {
            let (room, into) = slot_destination::<T>(ast, slot);
            arm.extend(room);
            arm.extend(member_read::<T>(ast, slot, into));
            arm.extend(slot_set_present::<T>(slot, true));
        } else {
            let (room, into) = slot_destination::<T>(ast, slot);
            arm.extend(room);
            arm.extend(member_read::<T>(ast, slot, into));
        }
        arm.push(ends_where_the_header_said::<T>("zd_ends"));
        chain = Some(T::block(vec![T::when(
            T::binary(
                "==",
                T::header_field(name::<T>("zd_h"), "member_id"),
                T::unsigned_int(i128::from(slot.member_id)),
            ),
            T::block(arm),
            chain,
        )]));
    }

    let mut loop_body = vec![
        assign::<T>(
            name::<T>("zd_h"),
            call_expr::<T>("read_emheader", Vec::new()),
        ),
        T::when(
            T::binary(
                "!=",
                call_expr::<T>("reader_status", Vec::new()),
                T::constant(Constant::StatusOk),
            ),
            T::block(vec![T::leave()]),
            None,
        ),
        assign::<T>(
            name::<T>("zd_ends"),
            offset_plus::<T>(
                position::<T>(),
                T::header_field(name::<T>("zd_h"), "extent"),
            ),
        ),
    ];
    if let Some(chain) = chain {
        loop_body.extend(T::into_statements(chain));
    }

    statements.push(T::until(
        T::binary(
            "&&",
            T::paren(T::binary("<", position::<T>(), name::<T>("zd_end"))),
            T::paren(status_is_ok::<T>()),
        ),
        T::block(loop_body),
    ));
    statements.extend(defaults_for_absent::<T>(ast, members)?);
    Ok(statements)
}

/// A `@mutable` aggregate under XCDR1: parameters until `PID_LIST_END`.
fn parameter_list<T: CodecTarget>(
    ast: &Ast,
    members: &[Slot<T>],
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut statements = Vec::new();
    for index in 0..members.len() {
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            &format!("zd_seen{index}"),
            Some(literal::<T>("false")),
        ));
    }
    statements.push(T::declare(T::handle(Handle::PidRead), "zd_p", None));
    statements.push(T::declare(
        T::scalar(Scalar::Size),
        "zd_at",
        Some(literal::<T>("0")),
    ));

    // Stepping over a parameter this type has no member for, in the one
    // place three different reasons reach it.
    let skip_parameter = T::block(vec![T::perform(call_expr::<T>(
        "reader_skip_to",
        vec![offset_plus::<T>(
            name::<T>("zd_at"),
            T::header_field(name::<T>("zd_p"), "length"),
        )],
    ))]);
    // A parameter this type does not declare. PL_CDR1 carries a length
    // on every parameter, so skipping is always possible — but
    // §7.2.2.4.4.4.6 says a reader that does not understand a
    // must-understand member shall discard the whole sample rather than
    // step over it. The XCDR2 arm below already refused; this one
    // skipped in silence.
    let unknown = T::block(vec![T::when(
        T::header_field(name::<T>("zd_p"), "must_understand"),
        T::block(vec![
            refuse::<T>(Constant::StatusOutOfRange),
            T::give_back(None),
        ]),
        Some(skip_parameter.clone()),
    )]);
    let mut chain = Some(unknown.clone());

    for (index, slot) in members.iter().enumerate().rev() {
        let seen = format!("zd_seen{index}");
        let mut arm = vec![T::when(
            name::<T>(&seen),
            T::block(vec![
                refuse::<T>(Constant::StatusOutOfRange),
                T::give_back(None),
            ]),
            None,
        )];
        arm.push(assign::<T>(name::<T>(&seen), literal::<T>("true")));
        if slot.optional {
            let (room, into) = slot_destination::<T>(ast, slot);
            arm.extend(room);
            arm.extend(member_read::<T>(ast, slot, into));
            arm.extend(slot_set_present::<T>(slot, true));
        } else {
            let (room, into) = slot_destination::<T>(ast, slot);
            arm.extend(room);
            arm.extend(member_read::<T>(ast, slot, into));
        }
        chain = Some(T::block(vec![T::when(
            T::binary(
                "==",
                T::header_field(name::<T>("zd_p"), "pid"),
                T::unsigned_int(i128::from(slot.member_id)),
            ),
            T::block(arm),
            chain,
        )]));
    }

    // §7.4.1.2.4: the list ends at the sentinel. A stream that runs out
    // first is one whose end nobody wrote.
    let mut loop_body = vec![
        T::when(
            T::binary("<", remaining::<T>(), T::int(4)),
            T::block(vec![refuse::<T>(Constant::StatusTruncated), T::leave()]),
            None,
        ),
        assign::<T>(name::<T>("zd_p"), call_expr::<T>("read_pid", Vec::new())),
        T::when(
            T::binary(
                "!=",
                call_expr::<T>("reader_status", Vec::new()),
                T::constant(Constant::StatusOk),
            ),
            T::block(vec![T::leave()]),
            None,
        ),
        T::when(
            T::header_field(name::<T>("zd_p"), "end"),
            T::block(vec![T::leave()]),
            None,
        ),
        assign::<T>(name::<T>("zd_at"), position::<T>()),
    ];
    // The same two gates the Rust and Python columns put in front of
    // the id chain, in the same order. `PID_IGNORE` first, because
    // §7.4.1.2.2 states no exception for the must-understand bit that
    // `unknown` would refuse on; then `zdw_pid_is_user_member`, which is
    // false for a vendor-private parameter and for a reserved id.
    if let Some(chain) = chain {
        loop_body.push(T::when(
            T::header_field(name::<T>("zd_p"), "ignore"),
            skip_parameter,
            Some(T::block(vec![T::when(
                T::runtime_static("pid_is_user_member", vec![name::<T>("zd_p")]),
                chain,
                Some(unknown),
            )])),
        ));
    }
    loop_body.push(T::perform(call_expr::<T>(
        "finish_pid_read",
        vec![name::<T>("zd_p")],
    )));

    statements.push(T::until(status_is_ok::<T>(), T::block(loop_body)));
    statements.extend(defaults_for_absent::<T>(ast, members)?);
    Ok(statements)
}

/// The members the stream did not carry, each taking its default.
///
/// §7.4.1.2.3: "any mutable structure member's value may be omitted.
/// For non-optional members this may be due to the sender having a
/// compatible type that does not have that member. In such a case, if
/// the receiver's member is not optional, it logically takes its
/// default value."
///
/// This column used to refuse with `ZDW_TRUNCATED`, which made a new
/// subscriber unable to receive from an old publisher — the exchange
/// `@mutable` exists for. Cyclone, RTI and Fast DDS all hand back the
/// default; measured 2026-08-09 as `evo-mutable-tail`,
/// `evo-mutable-middle` and `evo-mutable-defaults`, where all three
/// agree with Table 9 on every kind in it.
///
/// `@must_understand` makes no difference here, and that is measured
/// too: `evo-mutable-mustunderstand`, all three vendors, both encoding
/// versions. `crates/wire-probe/src/decode.rs::require_present` carries
/// the argument.
fn defaults_for_absent<T: CodecTarget>(
    ast: &Ast,
    members: &[Slot<T>],
) -> Result<Vec<T::Stmt>, EncodeError> {
    let mut statements = Vec::new();
    for (index, slot) in members.iter().enumerate() {
        // "If the member is optional, it takes no value at all." The
        // `_has_` flag was set false before the loop that reads, so
        // there is nothing to do.
        if slot.optional {
            continue;
        }
        let node = ast
            .member(slot.member)
            .map_err(|error| EncodeError::NoLayout {
                detail: format!("{}: {error}", slot.field),
            })?;
        let (room, into) = slot_default_place::<T>(ast, slot, arrow::<T>("v", &slot.field));
        let mut defaulted = room;
        defaulted.extend(T::member_default(ast, node, slot.type_, into)?);
        statements.push(T::when(
            T::binary(
                "==",
                name::<T>(&format!("zd_seen{index}")),
                literal::<T>("false"),
            ),
            T::block(defaulted),
            None,
        ));
    }
    Ok(statements)
}

/// The member has to end where its header said it would.
fn ends_where_the_header_said<T: CodecTarget>(ends: &str) -> T::Stmt {
    T::when(
        T::binary("!=", position::<T>(), name::<T>(ends)),
        T::block(vec![refuse::<T>(Constant::StatusOutOfRange)]),
        None,
    )
}

/// A union: the discriminator, then the member its labels select.
///
/// A discriminator matching no label and a union with no `default:` is
/// a legal outcome and reads no member at all —
/// `union-default-unselected` in the corpus is that sample.
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

    let mut statements = Vec::new();
    // `ElementPadding::Packed`: a union is its own alignment block on the
    // read side too. See the writer's `write_union`.
    let packed = key.element_padding == ElementPadding::Packed;
    if packed {
        statements.push(begin_packed::<T>());
    }
    if layout.needs_dheader {
        statements.extend(read_delimiter::<T>());
    }
    // Rules (27) and (28): a `@mutable` union frames its discriminator
    // as an `MMEMBER` at member id 0, and the discriminator comes first
    // because nothing else says which member follows it.
    statements.extend(read_mmember::<T>(
        layout.discriminator,
        0,
        "zd_hd",
        // §7.3.1.2.1.12.5: the discriminator has a slot of its own,
        // and Table 19 admits only DEFAULT there.
        decode_into::<T>(
            ast,
            discriminator,
            T::discriminant("v"),
            ast.discriminator_try_construct(id),
        ),
    ));

    // The default branch's member is still set from Table 9's default;
    // the branch the discriminator selects is the only one left live.
    // Every case, or the body is refused: a case skipped here keeps the
    // default value it was constructed with, and the decoded union then
    // shows a branch its discriminator did not select.
    let case_fields: Vec<String> = ast
        .cases_of(id)
        .iter()
        .map(|case| {
            ast.case(*case)
                .and_then(|case_node| ast.member(case_node.member))
                .map(|member| T::escape(&member.name))
                .map_err(|_| EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "union case the model does not hold",
                })
        })
        .collect::<Result<_, _>>()?;
    statements.extend(T::clear_union_cases("v", &case_fields));

    // Whether the sentinel that ends the parameter list has already
    // been consumed, which happens where the writer's union carried no
    // member and this one's discriminator selects one.
    //
    // Declared wherever a **branch takes a parameter header**, and not
    // wherever the list has a sentinel. The two used to coincide —
    // `needs_sentinel` was `encoding.is_v1() && mutable`, which is
    // exactly when a branch gets a `PidCdr1` header — and
    // `MutableUnionFraming::BranchesFromZero` separates them: it
    // suppresses the sentinel and keeps the headers, and
    // `read_union_member` assigns `zd_sent` from the header's `end`
    // whatever the list ends with. Gating the declaration on the
    // sentinel left the assignment referring to a variable that does not
    // exist, and the C column did not compile.
    let takes_parameter_headers = layout
        .members
        .iter()
        .any(|plan| matches!(plan.header, MemberHeader::PidCdr1 { .. }));
    if takes_parameter_headers {
        statements.push(T::declare(
            T::scalar(Scalar::Bool),
            "zd_sent",
            Some(literal::<T>("false")),
        ));
    }

    let mut arms: Vec<(T::Expr, T::Block)> = Vec::new();
    let mut fallback: Option<T::Block> = None;
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
        let planned = layout
            .members
            .iter()
            .find(|plan| plan.member == member.node_id);
        // Table 21: `@external` changes no octet, only where the binding
        // keeps the value — a column that keeps an indirection has to
        // make storage before there is a value to write into, exactly as
        // a structure member does.
        let external = super::reached_externally::<T>(ast, member);
        let (mut setup, place) = if external {
            T::external_case_destination("v", &field)
        } else {
            (Vec::new(), T::case_member("v", &field))
        };
        // A branch whose value has no room inside the union is reached
        // through the column's indirection, exactly as a structure
        // member is.
        let place = if super::behind_pointer::<T>(ast, id, member_type, external, false) {
            let (made, through) = T::make_indirection(ast, member_type, place);
            setup.extend(made);
            through
        } else {
            place
        };
        let mut arm_statements = setup;
        arm_statements.extend(read_union_member::<T>(
            planned.map(|plan| plan.header),
            planned.map_or(0, |plan| plan.member_id),
            &format!("zd_hu{}", planned.map_or(0, |plan| plan.member_id)),
            decode_into::<T>(
                ast,
                member_type,
                place.clone(),
                ast.effective_try_construct(
                    member.facets.xtypes.as_ref().and_then(|x| x.try_construct),
                    Some(member_type),
                ),
            ),
            // M11, §7.2.4.4.8, Table 19, the UNION_TYPE row: "If the
            // discriminator value does not select any member in O2,
            // then: If the discriminator value selects a member m1
            // in O1, then m1 is set to its default value according
            // to its type."
            T::default_into(ast, member_type, place)?,
        ));
        let read = T::block(arm_statements);

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
            fallback = Some(read.clone());
        }
        if let Some(condition) = condition {
            arms.push((condition, read));
        }
    }

    let mut chain: Option<T::Block> = fallback;
    for (condition, block) in arms.into_iter().rev() {
        chain = Some(T::block(vec![T::when(condition, block, chain)]));
    }
    if let Some(chain) = chain {
        statements.extend(T::into_statements(chain));
    }

    // The C probe is built with `-Wall -Wextra -Werror`, so a variable
    // that is set and never read fails the build. Under a framing that
    // writes no sentinel nothing reads `zd_sent`, and `(void)` is what
    // says so out loud rather than leaving the declaration out and the
    // assignment dangling.
    if takes_parameter_headers && !layout.needs_sentinel {
        statements.extend(T::mark_used("zd_sent"));
    }

    // Rule (28): the list ends at its sentinel, selected member or not
    // — unless the member's own read already consumed it, which is what
    // says the writer wrote no member at all.
    if layout.needs_sentinel {
        statements.push(T::when(
            T::unary("!", name::<T>("zd_sent")),
            T::block(vec![
                T::declare(
                    T::handle(Handle::PidRead),
                    "zd_pe",
                    Some(call_expr::<T>("read_pid", Vec::new())),
                ),
                T::when(
                    T::binary(
                        "==",
                        T::header_field(name::<T>("zd_pe"), "end"),
                        literal::<T>("false"),
                    ),
                    T::block(vec![
                        refuse::<T>(Constant::StatusOutOfRange),
                        T::give_back(None),
                    ]),
                    None,
                ),
            ]),
            None,
        ));
    }

    if layout.needs_dheader {
        statements.extend(close_delimiter::<T>());
    }
    if packed {
        statements.push(finish_packed::<T>());
    }
    Ok(statements)
}

/// The member a union's discriminator selects, or its default where the
/// stream does not carry one.
///
/// M11. §7.2.4.4.8, Table 19, the UNION_TYPE row: "If the discriminator
/// value does not select any member in O2, then: If the discriminator
/// value selects a member m1 in O1, then m1 is set to its default value
/// according to its type." Two versions of a union whose newer one gave
/// an existing discriminator value a case is exactly that, and RTI is
/// the witness — `evo-union-unselected`, `{"_d":2,"b":0}` in both
/// encoding versions.
///
/// Which of the two the stream is saying is read off the **framing** and
/// never guessed. Under XCDR1 a `PID_LIST_END` where the member's
/// parameter would be is the writer saying it wrote none, and the
/// sentinel is consumed by reading it, so `zd_sent` tells the caller not
/// to read it a second time. Under XCDR2 the delimiter's extent says the
/// union is over. A `@final` union has neither, which is right: Table 19
/// requires two final unions to have an identical set of labels, so a
/// stream missing the member is a type mismatch and not an evolution.
fn read_union_member<T: CodecTarget>(
    header: Option<MemberHeader>,
    member_id: u32,
    handle: &str,
    value: T::Stmt,
    default: T::Stmt,
) -> Vec<T::Stmt> {
    match header {
        // Rule (28), the parameter list.
        Some(MemberHeader::PidCdr1 { .. }) => vec![
            T::declare(
                T::handle(Handle::PidRead),
                handle,
                Some(call_expr::<T>("read_pid", Vec::new())),
            ),
            T::when(
                T::header_field(name::<T>(handle), "end"),
                T::block(vec![
                    default.clone(),
                    assign::<T>(name::<T>("zd_sent"), literal::<T>("true")),
                ]),
                Some(T::block(vec![
                    T::when(
                        T::binary(
                            "!=",
                            T::header_field(name::<T>(handle), "pid"),
                            T::unsigned_int(i128::from(member_id)),
                        ),
                        T::block(vec![
                            refuse::<T>(Constant::StatusOutOfRange),
                            T::give_back(None),
                        ]),
                        None,
                    ),
                    value.clone(),
                    T::perform(call_expr::<T>("finish_pid_read", vec![name::<T>(handle)])),
                ])),
            ),
        ],
        // Rule (27) under a DHEADER.
        Some(MemberHeader::EmHeader { .. }) => vec![T::when(
            T::binary(">=", position::<T>(), name::<T>("zd_end")),
            T::block(vec![default]),
            Some(T::block(read_mmember::<T>(
                header, member_id, handle, value,
            ))),
        )],
        // No framing to read an absence off, so there is none.
        _ => read_mmember::<T>(header, member_id, handle, value),
    }
}

/// One value behind the `MMEMBER` header the plan gave it, or bare.
///
/// `None` is a union of any extensibility but `@mutable`. The id is
/// checked and not trusted: a header for another member is not one this
/// decoder can step past, because the value behind it belongs to
/// something else and every byte after it would come out of the wrong
/// place.
fn read_mmember<T: CodecTarget>(
    header: Option<MemberHeader>,
    member_id: u32,
    handle: &str,
    value: T::Stmt,
) -> Vec<T::Stmt> {
    let wrong_id = |mine: T::Expr| {
        T::when(
            T::unary("!", T::paren(mine)),
            T::block(vec![
                refuse::<T>(Constant::StatusOutOfRange),
                T::give_back(None),
            ]),
            None,
        )
    };
    match header {
        None | Some(MemberHeader::NoHeader) => vec![value],
        Some(MemberHeader::PidCdr1 { .. }) => vec![
            T::declare(
                T::handle(Handle::PidRead),
                handle,
                Some(call_expr::<T>("read_pid", Vec::new())),
            ),
            wrong_id(T::binary(
                "&&",
                T::paren(T::binary(
                    "==",
                    T::header_field(name::<T>(handle), "pid"),
                    T::unsigned_int(i128::from(member_id)),
                )),
                T::paren(T::binary(
                    "==",
                    T::header_field(name::<T>(handle), "end"),
                    literal::<T>("false"),
                )),
            )),
            value,
            T::perform(call_expr::<T>("finish_pid_read", vec![name::<T>(handle)])),
        ],
        Some(MemberHeader::EmHeader { .. }) => vec![
            T::declare(
                T::handle(Handle::EmHeaderRead),
                handle,
                Some(call_expr::<T>("read_emheader", Vec::new())),
            ),
            wrong_id(T::binary(
                "==",
                T::header_field(name::<T>(handle), "member_id"),
                T::unsigned_int(i128::from(member_id)),
            )),
            value,
        ],
        // Neither `OPT_FMEMBER` form reaches a union: a case member is
        // selected by the discriminator and is never `@optional`.
        Some(MemberHeader::OptionalCdr1 { .. } | MemberHeader::OptionalFlag) => vec![value],
    }
}

/// The call one scalar position reads with.
///
/// Every policy is already answered: `zerodds_ast_layout::plan::scalar_write`
/// resolved them against the key and the position, so nothing here
/// reads an axis and nothing here can pass the wrong argument to one.
/// The same instruction the writer takes, which is what makes the two
/// directions answerable against each other.
fn primitive_read_at<T: CodecTarget>(write: ScalarWrite) -> T::Expr {
    match write {
        ScalarWrite::Bool => call_expr::<T>("read_bool", Vec::new()),
        ScalarWrite::U8 => call_expr::<T>("read_u8", Vec::new()),
        ScalarWrite::I8 => call_expr::<T>("read_i8", Vec::new()),
        // The runtime hands back a code point, which is wider than the
        // C storage. The narrowing is written out so that it is a
        // decision in the generated code and not a conversion the
        // compiler makes quietly — and an IDL `char` is one octet, so
        // there is nothing above the low eight bits to lose.
        ScalarWrite::Char => cast::<T>(
            T::scalar(Scalar::Char),
            call_expr::<T>("read_char", Vec::new()),
        ),
        ScalarWrite::Wchar {
            width,
            length_octet,
            order,
        } => cast::<T>(
            T::scalar(Scalar::UInt16),
            call_expr::<T>(
                "read_wchar",
                vec![
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
        ),
        ScalarWrite::I16 => call_expr::<T>("read_i16", Vec::new()),
        ScalarWrite::U16 => call_expr::<T>("read_u16", Vec::new()),
        ScalarWrite::I32 => call_expr::<T>("read_i32", Vec::new()),
        ScalarWrite::U32 => call_expr::<T>("read_u32", Vec::new()),
        ScalarWrite::I64 => call_expr::<T>("read_i64", Vec::new()),
        ScalarWrite::U64 => call_expr::<T>("read_u64", Vec::new()),
        ScalarWrite::F32 => call_expr::<T>("read_f32", Vec::new()),
        ScalarWrite::F64 => call_expr::<T>("read_f64", Vec::new()),
        ScalarWrite::F128 => call_expr::<T>("read_f128", Vec::new()),
        // The reader frames these itself: a string allocates, a fixed
        // takes its digits from the type, an enumeration reads a holder
        // and maps it back. They reach this function through their own
        // arms in `body_of`.
        ScalarWrite::Str
        | ScalarWrite::Wstr { .. }
        | ScalarWrite::Fixed { .. }
        | ScalarWrite::Enum { .. }
        | ScalarWrite::Bitmask { .. } => call_expr::<T>("read_u8", Vec::new()),
    }
}

/// The C name of a `wstring` length policy.
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

/// The C name of a `wstring` unit order.
const fn wstring_unit_order(order: WstringUnitOrder) -> Constant {
    match order {
        WstringUnitOrder::Stream => Constant::WstringUnitsStream,
        WstringUnitOrder::BigEndianAlways => Constant::WstringUnitsBigEndianAlways,
        WstringUnitOrder::ByteOrderMark => Constant::WstringUnitsByteOrderMark,
        WstringUnitOrder::HostUnswapped => Constant::WstringUnitsHostUnswapped,
    }
}

// -- small builders ------------------------------------------------------

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `T::name`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn name<T: CodecTarget>(text: &str) -> T::Expr {
    T::name(text)
}

/// An integer whose written form matters — a `u` suffix, a cast.
///
/// The suffixed forms are C's and are what this file has always
/// emitted; they stay here rather than in the backend because a target
/// that does not need them would not know what to drop.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `T::literal`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn literal<T: CodecTarget>(text: &str) -> T::Expr {
    T::literal(Literal::Int(text.to_string()))
}

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `T::int`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn int<T: CodecTarget>(value: i128) -> T::Expr {
    T::int(value)
}

fn deref<T: CodecTarget>(text: &str) -> T::Expr {
    T::whole(text)
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

fn assign<T: CodecTarget>(place: T::Expr, value: T::Expr) -> T::Stmt {
    T::perform(T::binary("=", place, value))
}

/// A runtime call on the reader.
///
/// `function_name` is the operation without the runtime's prefix —
/// `read_u32`, not `zdw_read_u32` — and the receiver is prepended by
/// the backend.
fn call_expr<T: CodecTarget>(function_name: &str, args: Vec<T::Expr>) -> T::Expr {
    T::runtime("r", function_name, args)
}

/// The reader's offset into the stream.
///
/// # The offset domain
///
/// [`position`], [`remaining`] and the four locals this walk declares
/// with [`Scalar::Size`] — `zd_end`, `zd_ends`, `zd_at` and a map's
/// `zd_end` — are one width, and it is the target's own size type:
/// `size_t` in C, `int` in the C#, Go, Java and F# runtimes, `usize` in
/// Zig's, `Long` in Kotlin's. Nothing else is.
///
/// A DHEADER is a `uint32`, an EMHEADER's extent is a `uint32`, a PID's
/// length is a `uint16` and a sentinel's size is a literal — none of the
/// four is in the domain, and each has to be brought into it before it
/// is put beside one that is. Fifteen of the twenty columns widen the
/// narrower side themselves and never notice; F# and Kotlin convert
/// nothing implicitly and name a type error.
///
/// So the domain has exactly three entrances and no fourth:
///
/// - [`offset_plus`] for an addition — it casts the count;
/// - [`cast`] to [`Scalar::Size`] for a comparison against a count;
/// - [`CodecTarget::int`] and **not** [`CodecTarget::unsigned_int`] for a
///   literal, because `4u` is not a token in every language and is a
///   second width in the ones where it is.
///
/// Measured: `zd_ends <- r.ReaderPosition() + zd_h.Extent` was
/// `error FS0001` ("uint32 does not match int") in every F# run and
/// `assignment type mismatch: actual type is 'Long', but 'Int' was
/// expected` in every Kotlin XCDR2 run, and
/// `if r.ReaderRemaining() < 4u then` was the same error a second time
/// in the PL_CDR1 arm.
fn position<T: CodecTarget>() -> T::Expr {
    call_expr::<T>("reader_position", Vec::new())
}

/// How many octets the reader has left, in the offset domain.
///
/// See [`position`] for what may be put beside it.
fn remaining<T: CodecTarget>() -> T::Expr {
    call_expr::<T>("reader_remaining", Vec::new())
}

/// `base + count`, where `base` is in the offset domain and `count` is a
/// length the stream carried.
///
/// The cast is the whole point: see [`position`].
fn offset_plus<T: CodecTarget>(base: T::Expr, count: T::Expr) -> T::Expr {
    T::binary("+", base, cast::<T>(T::scalar(Scalar::Size), count))
}

fn refuse<T: CodecTarget>(status: Constant) -> T::Stmt {
    T::perform(call_expr::<T>("reader_refuse", vec![T::constant(status)]))
}

fn status_is_ok<T: CodecTarget>() -> T::Expr {
    T::binary(
        "==",
        call_expr::<T>("reader_status", Vec::new()),
        T::constant(Constant::StatusOk),
    )
}

/// A DHEADER read and not kept.
///
/// A collection's delimiter says how long it is and the element count
/// says how many there are; the count is what the loop runs on, so the
/// length is read to move the cursor and nothing else. The same thing
/// the Rust reader does with it.
fn discard_dheader<T: CodecTarget>() -> T::Stmt {
    T::drop_value(call_expr::<T>("read_dheader", Vec::new()))
}

/// A loop from zero to `count`, exclusive.
///
/// The backend's, and the same one the encoder uses. The reader spelled
/// its bounds `0u` and `1u` where the writer spelled them `0` and `1`;
/// two spellings of one loop in one language is a difference nothing
/// depended on and nobody chose.
fn counted_loop<T: CodecTarget>(index: &str, count: T::Expr, body: T::Block) -> T::Stmt {
    T::count_up(index, count, body)
}
