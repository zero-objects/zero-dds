// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ROS 2 Jazzy dynamic type support, backed by ZeroDDS' own reflective
//! `DynamicType`/`DynamicData` (`zerodds_types::dynamic`).
//!
//! This module is the Rust half of `rosidl_dynamic_typesupport_zerodds`:
//! a `rosidl_dynamic_typesupport_serialization_support_interface_t`
//! implementation, the way `rosidl_dynamic_typesupport_fastrtps` is one
//! for Fast DDS. The C side (`rmw_c/rmw_zerodds_dynamic.cpp`) populates
//! that C function-pointer table; every entry calls one of the `extern
//! "C"` functions below.
//!
//! ## Why a `graft`, not a shared model
//!
//! [`zerodds_types::dynamic::DynamicTypeBuilder`] is, by its own module
//! note, "one factory owning one model": every type it builds shares one
//! [`zerodds_ast::Ast`], and [`DynamicTypeBuilder::finish`] consumes the
//! builder to hand the model over as a shared, immutable
//! `Arc<Ast>` — after which nothing can be added to it.
//!
//! ROS 2's interface does not respect that shape: `dynamic_type_builder_init`
//! is called once per struct in a message's type graph (the root and every
//! nested/referenced type), each call getting its own
//! `rosidl_dynamic_typesupport_dynamic_type_builder_impl_t` with no
//! argument naming a shared parent. Composition happens afterwards, through
//! `add_complex_member`/`add_complex_member_builder`, which is hyour only
//! signal that two independently-created builders belong to the same
//! message.
//!
//! So each [`ZddsDynBuilder`] owns its own private, single-struct
//! [`DynamicTypeBuilder`]/`Ast`. Composing one into another
//! ([`graft`]) does not share the model; it walks the source type's
//! declaration (structure, primitive, string, sequence, array — the only
//! kinds this interface's builder ever produces) and re-declares it, node
//! by node, in the target's own `Ast`, memoised so a type referenced by
//! two members is copied once. The result is self-contained: finishing the
//! root builder produces an `Arc<Ast>` that needs nothing from any sibling
//! builder, which is exactly what a `DynamicType`/`DynamicData` requires.
//!
//! ## Extensibility
//!
//! `dynamic_type_builder_init` carries no extensibility argument, and this
//! interface never adds one — the ROS 2 type-description schema this
//! backend is built from has none either. Every struct this module builds
//! is declared `@final`: the plain, positional CDR framing (no DHEADER, no
//! EMHEADER) that `rmw_zerodds_cxx.cpp`'s introspection-driven codec
//! already writes for every ROS message on this shim's static path. A
//! peer whose *static* type for the same message is `@appendable` or
//! `@mutable` — legal for an IDL-authored ROS 2 message under REP-2011 —
//! decodes with the wrong framing here; see the crate README's "Dynamic
//! types" section.
//!
//! ## A member's `default_value`
//!
//! Every `dynamic_type_builder_add_*_member` in the interface carries a
//! `default_value`/`default_value_length` for the scalar/array/sequence
//! shapes of a primitive or a string kind (the `add_complex_*` family
//! carries the same two parameters, but `.msg`'s grammar has no syntax
//! for a nested-message-typed field's default, so `TypeDescription`
//! generation never fills them for one, and `rmw_zerodds_dynamic.cpp`
//! leaves those alone).
//!
//! A **scalar** member's default text becomes a real XTypes `@default`:
//! [`ros_default_literal`] turns it into the literal
//! [`NewMember::with_default`] and [`zerodds_types::dynamic::to_ast`]
//! both read the same way — every kind already spells its default the
//! way the model wants it (an integer or a float as `str()` gives it, a
//! string or a character verbatim) except `Boolean`, whose Python-side
//! spelling is `"True"`/`"False"` and not IDL's `TRUE`/`FALSE`.
//!
//! An **array or sequence** member's default has no XTypes counterpart
//! at all — §7.3.4.5.4 gives `@default` one `<const_expr>`, never a
//! list — so it never reaches the model. [`collection_default::parse`]
//! reads it directly into a `Vec<DynamicData>` and
//! [`ZddsDynBuilder`]/[`ZddsDynType`]/[`ZddsDynData`] carry that
//! alongside the type, keyed by the member's own name; applied at
//! `dynamic_data_init_from_dynamic_type*` and at
//! `dynamic_data_clear_value`/`clear_all_values`/`clear_nonkey_values`
//! (the last keeps the key members — see that function's doc). See [`collection_default`]'s module note for the
//! text's exact shape (`rosidl_adapter`'s `to_idl_literal` writes it as
//! `repr(tuple(value))` inside an IDL string literal, since IDL's
//! annotation grammar has no array-literal parameter).
//!
//! A text that does not translate — of either kind — is refused with
//! `RCUTILS_RET_ERROR`, never silently turned into a zero or a
//! shortened/clamped list.
//!
//! ## `long double` / `float128`
//!
//! `zerodds_float128::Float128` is true IEEE binary128, and the crate
//! already carries the two host bridges a `long double` needs
//! (`Float128::{to,from}_x87_padded_bytes` for the x86-64 80-bit
//! extended layout `sizeof(long double) == 16` gives it, and
//! `Float128::{to,from}_le_bytes` for the true-binary128 layout aarch64
//! glibc uses). Both are exposed below; `rmw_zerodds_dynamic.cpp` picks
//! one at compile time. See `internal/`'s note on `idl-4.2` long-double:
//! ZeroDDS's own IDL codegen already treats `float128` as the one
//! deferred bucket (Rust has no native `f128` yet), so a host whose
//! `long double` is neither of the two layouts above (32-bit ARM,
//! PowerPC's double-double) is out of scope here too, named rather than
//! silently mishandled — `get`/`set_float128_*` refuse it.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::sync::Arc;

mod collection_default;
use collection_default::LengthRule;

use zerodds_ast::{
    Ast, AutoId, Bound as AstBound, CharWidth, PrimitiveKind as AstPrimitiveKind,
    TypeKind as AstKind, TypeNode, XTypesFacet,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_float128::Float128;
use zerodds_types::dynamic::{
    DynamicData, DynamicType, DynamicTypeBuilder, ExtensibilityKind, MEMBER_ID_INVALID, MemberId,
    NewMember, TryConstructKind, TypeKind as DynKind, default_expr_text,
};
use zerodds_wire_codec::{ProbeRequest, Source as _};

/// `rcutils_ret.h`'s `RCUTILS_RET_OK`.
const OK: i32 = 0;
/// `rcutils_ret.h`'s `RCUTILS_RET_ERROR`.
const ERR: i32 = 2;
/// `rcutils_ret.h`'s `RCUTILS_RET_INVALID_ARGUMENT`.
const INVALID: i32 = 11;

/// A member's primitive/string kind, as the C side names it (matches the
/// `ZDDS_DYN_KIND_*` `#define`s `rmw_zerodds_dynamic.cpp` uses to call
/// in). Not [`DynKind`] directly: this also carries `Nested`, which is not
/// a [`DynKind`] value but "use the type a caller already built".
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZddsDynKindTag {
    /// `boolean`.
    Boolean = 0,
    /// `octet`/`byte` — `TK_BYTE`.
    Byte = 1,
    /// `char`.
    Char8 = 2,
    /// `wchar`.
    Char16 = 3,
    /// `float`/`float32`.
    Float32 = 4,
    /// `double`/`float64`.
    Float64 = 5,
    /// `long double`/`float128`.
    Float128 = 6,
    /// `int8`.
    Int8 = 7,
    /// `uint8` — `TK_UINT8`, distinct from `TK_BYTE`.
    UInt8 = 8,
    /// `int16`.
    Int16 = 9,
    /// `uint16`.
    UInt16 = 10,
    /// `int32`.
    Int32 = 11,
    /// `uint32`.
    UInt32 = 12,
    /// `int64`.
    Int64 = 13,
    /// `uint64`.
    UInt64 = 14,
    /// `string`/`wstring`, per the accompanying `wide` flag.
    String8 = 15,
    /// (kept numerically adjacent to `String8`; `wide` is what the caller
    /// passes, not a second tag — see [`ZddsDynKindTag::from_u32`]).
    String16 = 16,
    /// A previously built struct, named by handle rather than by kind.
    Nested = 17,
}

impl ZddsDynKindTag {
    fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Boolean,
            1 => Self::Byte,
            2 => Self::Char8,
            3 => Self::Char16,
            4 => Self::Float32,
            5 => Self::Float64,
            6 => Self::Float128,
            7 => Self::Int8,
            8 => Self::UInt8,
            9 => Self::Int16,
            10 => Self::UInt16,
            11 => Self::Int32,
            12 => Self::UInt32,
            13 => Self::Int64,
            14 => Self::UInt64,
            15 => Self::String8,
            16 => Self::String16,
            17 => Self::Nested,
            _ => return None,
        })
    }

    const fn as_dyn_kind(self) -> Option<DynKind> {
        Some(match self {
            Self::Boolean => DynKind::Boolean,
            Self::Byte => DynKind::Byte,
            Self::Char8 => DynKind::Char8,
            Self::Char16 => DynKind::Char16,
            Self::Float32 => DynKind::Float32,
            Self::Float64 => DynKind::Float64,
            Self::Float128 => DynKind::Float128,
            Self::Int8 => DynKind::Int8,
            Self::UInt8 => DynKind::UInt8,
            Self::Int16 => DynKind::Int16,
            Self::UInt16 => DynKind::UInt16,
            Self::Int32 => DynKind::Int32,
            Self::UInt32 => DynKind::UInt32,
            Self::Int64 => DynKind::Int64,
            Self::UInt64 => DynKind::UInt64,
            Self::String8 | Self::String16 | Self::Nested => return None,
        })
    }
}

/// A member's shape, as the C side names it: scalar, a fixed array, a
/// bounded sequence, or an unbounded sequence — the four shapes every
/// `_member`/`_array_member`/`_bounded_sequence_member`/
/// `_unbounded_sequence_member` family in the interface builds.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZddsDynShapeTag {
    /// One value.
    Scalar = 0,
    /// `T[N]`, one dimension — the only shape the ROS 2 type-description
    /// schema's `FieldType` capacity describes.
    Array = 1,
    /// `sequence<T, N>`.
    BoundedSequence = 2,
    /// `sequence<T>`.
    UnboundedSequence = 3,
}

impl ZddsDynShapeTag {
    fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Scalar,
            1 => Self::Array,
            2 => Self::BoundedSequence,
            3 => Self::UnboundedSequence,
            _ => return None,
        })
    }
}

// =====================================================================
// graft: copy a type declared in one `Ast` into another
// =====================================================================

/// Re-declare `src_id` of `src` inside `target`'s own model, and every
/// type it reaches, memoised so a type referenced from two places is
/// copied once and so a self-referential graph does not recurse forever.
///
/// Only the kinds this interface's builder ever produces are handled —
/// `Primitive`, `String`, `Sequence`, `Array`, `Structure` — matching
/// [`ZddsDynKindTag`]/[`ZddsDynShapeTag`]'s coverage exactly: nothing
/// grafted here was built any other way.
///
/// # Errors
/// [`zerodds_types::dynamic::DynamicError`] where the model refuses the
/// re-declaration, or where `src_id` names a kind this interface never
/// builds (a union, an enum, a bitset, a bitmask, an alias, a map) —
/// unreachable through this module's own builder calls, reported rather
/// than assumed impossible.
fn graft(
    target: &mut DynamicTypeBuilder,
    src: &Ast,
    src_id: Id<TypeNode>,
    memo: &mut HashMap<Id<TypeNode>, Id<TypeNode>>,
) -> Result<Id<TypeNode>, zerodds_types::dynamic::DynamicError> {
    graft_at(target, src, src_id, memo, 0)
}

/// Nesting bound of [`graft`]: a struct is memoised before its members,
/// so a cycle ends, but a chain of sequences and arrays is followed as
/// deep as the source model goes.
const MAX_GRAFT_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (one level per element or member type;
/// refused past `MAX_GRAFT_DEPTH`, and a struct reached again resolves
/// through `memo`)
fn graft_at(
    target: &mut DynamicTypeBuilder,
    src: &Ast,
    src_id: Id<TypeNode>,
    memo: &mut HashMap<Id<TypeNode>, Id<TypeNode>>,
    depth: usize,
) -> Result<Id<TypeNode>, zerodds_types::dynamic::DynamicError> {
    use zerodds_types::dynamic::DynamicError;
    if let Some(done) = memo.get(&src_id) {
        return Ok(*done);
    }
    if depth > MAX_GRAFT_DEPTH {
        return Err(DynamicError::Model {
            detail: format!("type nesting exceeds {MAX_GRAFT_DEPTH} levels"),
        });
    }
    let node = src.type_(src_id).map_err(|error| DynamicError::Model {
        detail: error.to_string(),
    })?;
    let name = node.name.clone();
    let out = match &node.kind {
        AstKind::Primitive(primitive) => target.primitive(DynKind::of_primitive(*primitive))?,
        AstKind::String { width, bound } => target.string(
            matches!(width, CharWidth::WChar),
            integral_bound(src, *bound)?,
        )?,
        AstKind::Sequence { element, bound } => {
            let element_id = resolved(element, &name)?;
            let bound = integral_bound(src, *bound)?;
            let grafted = graft_at(target, src, element_id, memo, depth + 1)?;
            target.sequence(grafted, bound)?
        }
        AstKind::Array { element, dims } => {
            let element_id = resolved(element, &name)?;
            let mut lengths = Vec::with_capacity(dims.len());
            for dim in dims {
                let value = src
                    .eval_integral(*dim)
                    .map_err(|error| DynamicError::Model {
                        detail: error.to_string(),
                    })?;
                lengths.push(u32::try_from(value).unwrap_or(u32::MAX));
            }
            let grafted = graft_at(target, src, element_id, memo, depth + 1)?;
            target.array(grafted, &lengths)?
        }
        AstKind::Structure { members, .. } => {
            let ext = src.effective_extensibility(src_id).into();
            let qualified = qualified_name_in(src, src_id, &name);
            let placeholder = declare_struct_at(target, &qualified, ext)?;
            // Registered before members are grafted: a self-referential
            // struct (a field whose type is the struct itself, reached
            // through a sequence or an array — direct self-nesting is not
            // representable without one) resolves to this same id rather
            // than recursing forever.
            memo.insert(src_id, placeholder);
            // The structure's own facets — `@nested`/`@autoid(HASH)`.
            // Unreachable through this module's own builder calls today
            // (`declare_named_struct` never asks for either), copied
            // anyway so a future caller that *can* set them is not a new
            // silent drop.
            let struct_xtypes = src
                .any_of_type(src_id)
                .ok()
                .and_then(|any| src.facets(any))
                .and_then(|facets| facets.xtypes.as_ref());
            let is_nested = struct_xtypes.is_some_and(|f| f.is_nested);
            let autoid_hash = struct_xtypes.is_some_and(|f| matches!(f.autoid, Some(AutoId::Hash)));
            if is_nested || autoid_hash {
                target.annotate(placeholder, is_nested, autoid_hash)?;
            }
            for member_id in members {
                let member = src
                    .member(*member_id)
                    .map_err(|error| DynamicError::Model {
                        detail: error.to_string(),
                    })?;
                let member_type = resolved(&member.type_, &member.name)?;
                let grafted_member_type = graft_at(target, src, member_type, memo, depth + 1)?;
                let at = format!("{qualified}::{}", member.name);
                // Every facet a member built through this interface can
                // carry (`NewMember`'s own field set) — `@key`,
                // `@optional`, `@must_understand`, `@external`,
                // `@try_construct`, the effective `@id`, and `@default`.
                // The one thing this cannot carry over is the id's
                // *origin* (`@hashid` vs `@id`) — `NewMember` has no
                // field for it, and this interface never produces a
                // `Hashed` origin itself (its own `add_*_member` calls
                // take a plain numeric id or none), so the effective
                // number survives and the origin distinction, which
                // this interface never made in the first place, does
                // not need to.
                let facet = member.facets.xtypes.as_ref();
                let mut new_member = NewMember {
                    name: member.name.clone(),
                    type_: grafted_member_type,
                    id: facet.and_then(XTypesFacet::effective_member_id),
                    is_key: facet.is_some_and(|f| f.is_key),
                    is_optional: facet.is_some_and(|f| f.is_optional),
                    is_must_understand: facet.is_some_and(|f| f.is_must_understand),
                    is_external: facet.is_some_and(|f| f.is_external),
                    try_construct: facet
                        .and_then(|f| f.try_construct)
                        .map(TryConstructKind::from),
                    default: None,
                };
                if let Some(expr) = facet.and_then(|f| f.default_value) {
                    new_member.default = Some(default_expr_text(src, expr, &at)?);
                }
                target.add_member(placeholder, &new_member)?;
            }
            return Ok(placeholder);
        }
        other => {
            return Err(DynamicError::Unrepresentable {
                what: name,
                kind: kind_name(other),
            });
        }
    };
    memo.insert(src_id, out);
    Ok(out)
}

const fn kind_name(kind: &AstKind) -> &'static str {
    match kind {
        AstKind::Union { .. } => "a union, which this interface never builds",
        AstKind::Enumeration { .. } => "an enumeration, which this interface never builds",
        AstKind::Bitmask { .. } => "a bitmask, which this interface never builds",
        AstKind::Bitset { .. } => "a bitset, which this interface never builds",
        AstKind::Alias { .. } => "a typedef, which this interface never builds",
        AstKind::Map { .. } => "a map, which this interface never builds",
        _ => "a kind this interface never builds",
    }
}

fn resolved(
    reference: &Ref<TypeNode>,
    at: &str,
) -> Result<Id<TypeNode>, zerodds_types::dynamic::DynamicError> {
    reference
        .resolved()
        .ok_or_else(|| zerodds_types::dynamic::DynamicError::Unresolved { at: at.to_string() })
}

fn integral_bound(
    src: &Ast,
    bound: AstBound,
) -> Result<Option<u32>, zerodds_types::dynamic::DynamicError> {
    let Some(expr) = bound.expr() else {
        return Ok(None);
    };
    let value =
        src.eval_integral(expr)
            .map_err(|error| zerodds_types::dynamic::DynamicError::Model {
                detail: error.to_string(),
            })?;
    Ok(Some(u32::try_from(value).unwrap_or(u32::MAX)))
}

// =====================================================================
// Handles
// =====================================================================

/// A collection member's ROS-side default, kept next to the type rather
/// than in the model — see the module note and [`collection_default`].
/// Keyed by the member's own name, since [`graft`] renumbers member ids
/// but never renames a member. `Arc`, so [`ZddsDynType::finish`] and
/// every [`ZddsDynData`] built from it share one map rather than cloning
/// its `Vec<DynamicData>`s.
type CollectionDefaults = Arc<HashMap<String, Vec<DynamicData>>>;

/// One `rosidl_dynamic_typesupport_dynamic_type_builder_impl_t`: a
/// private, single-struct model under construction. See the module note.
pub struct ZddsDynBuilder {
    model: DynamicTypeBuilder,
    root: Id<TypeNode>,
    /// The name ROS gave this builder — what `get_name` reports, kept
    /// separately from the model's own node name (which never carries
    /// the `::` module-path segments the model's name/module split
    /// wants; see [`declare_named_struct`]).
    original_name: CString,
    /// Not yet `Arc`-wrapped: still being written to by
    /// [`zerodds_dyn_builder_add_member`], one member at a time.
    collection_defaults: HashMap<String, Vec<DynamicData>>,
}

/// One `rosidl_dynamic_typesupport_dynamic_type_impl_t`: a finished,
/// self-contained type.
pub struct ZddsDynType {
    ty: DynamicType,
    original_name: CString,
    collection_defaults: CollectionDefaults,
}

/// One `rosidl_dynamic_typesupport_dynamic_data_impl_t`: a value of a
/// [`ZddsDynType`], plus the small scratch buffers the string/wstring
/// borrow accessors below need to hand C a stable pointer.
pub struct ZddsDynData {
    ty: DynamicType,
    value: DynamicData,
    scratch_bytes: Vec<u8>,
    scratch_units: Vec<u16>,
    own_name: CString,
    /// The member id/index this value was loaned at
    /// ([`zerodds_dyn_data_loan`]), or `None` for a value that was not
    /// produced by a loan. `dynamic_data_return_loaned_value`'s C
    /// signature carries no member id (only the parent and the loaned
    /// value), so the loaned value has to remember it — see
    /// [`zerodds_dyn_data_return_loan`].
    loan_origin: Option<u64>,
    /// The token of the loan this value is, when it is one: the parent's
    /// [`Self::outstanding_loan`] carries the same number while the loan
    /// is out. A number and not the parent's address, so that a loan from
    /// another parent, or from a parent that has since been freed and
    /// reallocated, never matches.
    loan_token: Option<u64>,
    /// The token of the loan of this value that is out, if one is
    /// (§7.5.2.11.7: "A given DynamicData object may support only a single
    /// outstanding loan at a time").
    outstanding_loan: Option<u64>,
    /// This value's own type's collection defaults, carried past the
    /// `ZddsDynType` that seeded it (which may already be freed — see
    /// [`zerodds_dyn_data_from_builder`]'s note) — consulted by
    /// [`zerodds_dyn_data_clear_value`]/[`zerodds_dyn_data_clear_all_values`].
    /// Empty for a value spawned as someone else's member/element/loan
    /// ([`zerodds_dyn_data_get_complex`]): a collection default names a
    /// member of *this* struct, and a spawned child's struct is a
    /// different one (or it has none, being a collection or a scalar).
    collection_defaults: CollectionDefaults,
}

/// Split a ROS 2 fully qualified type name (`std_msgs::msg::Header`) into
/// module path segments and a simple name, and declare an `@final`
/// struct for it — the one construction every `ZddsDynBuilder` starts
/// from, module by module so [`ProbeRequest`]'s scoped-name resolver
/// (which walks the module tree, not a flat string) finds it again.
fn declare_named_struct(
    model: &mut DynamicTypeBuilder,
    qualified_name: &str,
) -> Result<Id<TypeNode>, zerodds_types::dynamic::DynamicError> {
    declare_struct_at(model, qualified_name, ExtensibilityKind::Final)
}

/// As [`declare_named_struct`], with the extensibility [`graft`] read off
/// the source rather than always `@final` — used when re-declaring an
/// existing struct (clone, graft) rather than building a fresh one from
/// this interface's builder (which never carries an extensibility
/// argument at all, see the module note; that path always asks for
/// `@final` through [`declare_named_struct`]).
fn declare_struct_at(
    model: &mut DynamicTypeBuilder,
    qualified_name: &str,
    ext: ExtensibilityKind,
) -> Result<Id<TypeNode>, zerodds_types::dynamic::DynamicError> {
    let mut segments: Vec<&str> = qualified_name
        .split("::")
        .filter(|s| !s.is_empty())
        .collect();
    let simple = segments.pop().unwrap_or(qualified_name);
    if segments.is_empty() {
        model.structure(simple, ext)
    } else {
        let module = model.module(&segments)?;
        model.structure_in(module, simple, ext)
    }
}

/// The fully qualified name `src_id` is declared under in `src`, without
/// a leading `::` — the same spelling [`DynamicType::qualified_name`]
/// gives it, computed the same way, since [`graft`] only ever has a raw
/// `&Ast` (see the module note on why the model here is not shared
/// through an `Arc`).
fn qualified_name_in(src: &Ast, src_id: Id<TypeNode>, fallback: &str) -> String {
    let Ok(node) = src.type_(src_id) else {
        return fallback.to_string();
    };
    let any = zerodds_ast::AnyId::Type(src_id, node.node_id);
    src.scoped_name_of(any).map_or_else(
        || fallback.to_string(),
        |scoped| {
            let text = scoped.to_string();
            text.strip_prefix("::").unwrap_or(&text).to_string()
        },
    )
}

fn cstr_or_empty(ptr: *const c_char) -> Result<String, i32> {
    if ptr.is_null() {
        return Ok(String::new());
    }
    // SAFETY: caller passes a NUL-terminated C string per the interface
    // contract, or NULL (handled above).
    let cstr = unsafe { CStr::from_ptr(ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

fn bytes_or_empty<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: caller-supplied `(ptr, len)` per the interface contract.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

fn units_or_empty<'a>(ptr: *const u16, len: usize) -> &'a [u16] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: as `bytes_or_empty`.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

fn member_id_from_u64(id: u64) -> Option<MemberId> {
    let id = u32::try_from(id).unwrap_or(u32::MAX);
    (id != MEMBER_ID_INVALID).then_some(id)
}

// =====================================================================
// Builder lifecycle
// =====================================================================

/// Creates a `ZddsDynBuilder` for `name` (a ROS 2 fully qualified type
/// name, `pkg::msg::Type`), an `@final` struct with no members yet.
/// NULL on a name the model refuses.
///
/// # Safety
/// `name` NUL-terminated or NULL; `name_len` is not read (the ROS 2
/// interface always hands a NUL-terminated `const char *` here; the
/// length parameter is accepted for signature parity and ignored, as
/// `serialization_support_interface.h` itself allows — none of its
/// `_init`-family functions promise the string is not also
/// NUL-terminated).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_init(
    name: *const c_char,
    name_len: usize,
) -> *mut ZddsDynBuilder {
    let _ = name_len;
    let Ok(name) = cstr_or_empty(name) else {
        return std::ptr::null_mut();
    };
    let mut model = DynamicTypeBuilder::new();
    let Ok(root) = declare_named_struct(&mut model, &name) else {
        return std::ptr::null_mut();
    };
    let Ok(original_name) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ZddsDynBuilder {
        model,
        root,
        original_name,
        collection_defaults: HashMap::new(),
    }))
}

/// Deep-copies `src` into a new, independent `ZddsDynBuilder`. NULL on
/// failure.
///
/// # Safety
/// `src` a live `ZddsDynBuilder` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_clone(
    src: *const ZddsDynBuilder,
) -> *mut ZddsDynBuilder {
    if src.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL, caller-owned per the interface's fini/clone pairing.
    let src = unsafe { &*src };
    let mut model = DynamicTypeBuilder::new();
    let mut memo = HashMap::new();
    let Ok(root) = graft(&mut model, src.model.model(), src.root, &mut memo) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ZddsDynBuilder {
        model,
        root,
        original_name: src.original_name.clone(),
        collection_defaults: src.collection_defaults.clone(),
    }))
}

/// Frees a `ZddsDynBuilder`. A NULL/already-finished pointer is a no-op —
/// [`zerodds_dyn_builder_finish`]/[`zerodds_dyn_data_from_builder`]
/// consume the builder and the C wrapper nulls its local copy, matching
/// `dynamic_type_init_from_dynamic_type_builder` followed by
/// `dynamic_type_builder_fini` on the same handle.
///
/// # Safety
/// `b` a `ZddsDynBuilder` this module allocated, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_free(b: *mut ZddsDynBuilder) {
    if b.is_null() {
        return;
    }
    // SAFETY: non-NULL, caller-owned, freed at most once (the C side nulls
    // its copy after a consuming call, per this function's doc).
    drop(unsafe { Box::from_raw(b) });
}

/// Borrows the name [`zerodds_dyn_builder_init`] was given.
///
/// # Safety
/// `b` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_get_name(
    b: *const ZddsDynBuilder,
    out_ptr: *mut *const c_char,
    out_len: *mut usize,
) -> i32 {
    if b.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL per above.
    let b = unsafe { &*b };
    let bytes = b.original_name.as_bytes();
    // SAFETY: out_ptr/out_len checked non-NULL above.
    unsafe {
        *out_ptr = b.original_name.as_ptr();
        *out_len = bytes.len();
    }
    OK
}

/// Renames a builder's display name — what
/// [`zerodds_dyn_builder_get_name`] and, once finished,
/// [`zerodds_dyn_type_get_name`] report. Does not rename the module path
/// the model declared the struct under (§7.5.2 gives the interface no
/// call to re-home a type in a new module, and every reader of a
/// `ZddsDynType` reaches it by handle, not by re-resolving the display
/// name).
///
/// # Safety
/// `b` non-NULL and live; `name` NUL-terminated or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_set_name(
    b: *mut ZddsDynBuilder,
    name: *const c_char,
    name_len: usize,
) -> i32 {
    let _ = name_len;
    if b.is_null() {
        return INVALID;
    }
    let Ok(name) = cstr_or_empty(name) else {
        return INVALID;
    };
    let Ok(name) = CString::new(name) else {
        return INVALID;
    };
    // SAFETY: non-NULL per above.
    unsafe { &mut *b }.original_name = name;
    OK
}

/// Adds a scalar/array/bounded-sequence/unbounded-sequence member of a
/// primitive or string kind to `b`'s struct. Covers every
/// `dynamic_type_builder_add_*_member` in the interface except the four
/// `add_complex_*` families, which go through
/// [`zerodds_dyn_builder_add_nested_member`]/
/// [`zerodds_dyn_builder_add_nested_member_from_builder`].
///
/// `id`: the member id, or [`MEMBER_ID_INVALID`] (as `u64`) for none —
/// the model then numbers it "previous member plus one", exactly as an
/// IDL member with no `@id` is numbered.
///
/// `str_bound`: for `kind == String8`/`String16`, 0 = unbounded, `N` =
/// bounded/fixed at `N` (this interface's `fixed_*` and `bounded_*`
/// member families both reach here identically — XTypes has one
/// `string<N>`/`wstring<N>` construct, not two, see the crate's README).
/// Ignored for every other kind.
///
/// `container_bound`: for `shape == Array`, the one array dimension (must
/// be nonzero); for `shape == BoundedSequence`, the sequence bound (must
/// be nonzero); ignored for `Scalar`/`UnboundedSequence`.
///
/// `default_value`: the member's default, empty (zero-length, or NULL)
/// for none. Text as `rosidl_dynamic_typesupport_dynamic_type_builder_
/// init_from_description` passes it on — see the module note and
/// [`ros_default_literal`]; refused with `RCUTILS_RET_ERROR` where it
/// does not translate to this kind's literal form, or where it does not
/// then read in the member's own type (`@optional` never reaches here —
/// this interface has no such annotation — so only the text itself is
/// ever the reason).
///
/// # Safety
/// `b` non-NULL and live; `name`/`default_value` NUL-terminated or NULL.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn zerodds_dyn_builder_add_member(
    b: *mut ZddsDynBuilder,
    kind_tag: u32,
    shape_tag: u32,
    id: u64,
    name: *const c_char,
    name_len: usize,
    str_bound: u64,
    container_bound: u64,
    default_value: *const c_char,
    default_value_len: usize,
) -> i32 {
    let _ = (name_len, default_value_len);
    if b.is_null() {
        return INVALID;
    }
    let Some(kind_tag) = ZddsDynKindTag::from_u32(kind_tag) else {
        return INVALID;
    };
    let Some(shape_tag) = ZddsDynShapeTag::from_u32(shape_tag) else {
        return INVALID;
    };
    let Ok(name) = cstr_or_empty(name) else {
        return INVALID;
    };
    let Ok(default_text) = cstr_or_empty(default_value) else {
        return INVALID;
    };
    // A scalar's default becomes a real XTypes `@default` (below, via
    // `ros_default_literal`); an array/sequence's has no XTypes form at
    // all and is parsed into elements instead, kept next to the type —
    // see the module note and `collection_default`. Parsed *before*
    // `b`'s model is touched, so a text that does not parse leaves the
    // model exactly as it was, never a member with no default to match.
    let scalar_default = if default_text.is_empty() || shape_tag != ZddsDynShapeTag::Scalar {
        None
    } else {
        match ros_default_literal(kind_tag, &default_text) {
            Ok(literal) => Some(literal),
            Err(()) => return ERR,
        }
    };
    let collection_elements = if default_text.is_empty() {
        None
    } else {
        let length = match shape_tag {
            ZddsDynShapeTag::Scalar => None,
            ZddsDynShapeTag::Array => Some(LengthRule::Exact(container_bound)),
            ZddsDynShapeTag::BoundedSequence => Some(LengthRule::AtMost(container_bound)),
            ZddsDynShapeTag::UnboundedSequence => Some(LengthRule::Any),
        };
        match length {
            None => None,
            Some(rule) => match collection_default::parse(kind_tag, &default_text, str_bound, rule)
            {
                Ok(elements) => Some(elements),
                Err(_) => return ERR,
            },
        }
    };
    // SAFETY: non-NULL per above.
    let b = unsafe { &mut *b };
    let element = match kind_tag {
        ZddsDynKindTag::String8 | ZddsDynKindTag::String16 => {
            let bound = (str_bound != 0).then(|| u32::try_from(str_bound).unwrap_or(u32::MAX));
            b.model.string(kind_tag == ZddsDynKindTag::String16, bound)
        }
        ZddsDynKindTag::Nested => return INVALID,
        _ => {
            let Some(dyn_kind) = kind_tag.as_dyn_kind() else {
                return INVALID;
            };
            b.model.primitive(dyn_kind)
        }
    };
    let Ok(element) = element else {
        return ERR;
    };
    let rc = add_member_of_shape(
        b,
        element,
        shape_tag,
        container_bound,
        id,
        &name,
        scalar_default.as_deref(),
    );
    if rc != OK {
        return rc;
    }
    if let Some(elements) = collection_elements {
        b.collection_defaults.insert(name, elements);
    }
    OK
}

/// `default_value` text, as `rosidl_dynamic_typesupport_dynamic_type_
/// builder_init_from_description` passes it on from a `TypeDescription`'s
/// `Field::default_value` — into the literal text
/// [`NewMember::with_default`] and [`zerodds_types::dynamic::to_ast`]
/// both read the same way.
///
/// That text is `rosidl_generator_type_description`'s `serialize_field`:
/// `str(member.get_annotation_value('default')['value'])` over the
/// value `rosidl_parser`'s IDL grammar parsed the `.idl` `@default`
/// annotation's literal to — a native Python `bool`/`int`/`float`/`str`,
/// never a second-hand re-encoding of the `.msg` source text. Every kind
/// but `Boolean` already spells its default the way this crate's own
/// `@default` parser wants it: an integer or a float as Python's `str()`
/// gives it (`"42"`, `"1.5"`), a character or a string verbatim, with no
/// quoting either side adds. `Boolean` is the one kind whose Python
/// spelling — `str(True)` is `"True"`, not `"TRUE"` — is not this
/// crate's `TRUE`/`FALSE`, so it is rewritten here; the IDL and the
/// all-lowercase spellings are accepted too, since a caller of this
/// interface is not required to go through a `TypeDescription` to reach
/// this call.
///
/// # Errors
/// `Err(())` for a `Boolean` default that reads as neither true nor
/// false. Every other kind is returned unchanged — a text that does not
/// then read in the member's own type is [`DynamicTypeBuilder::add_member`]'s
/// refusal to make, not this translation's.
fn ros_default_literal(kind_tag: ZddsDynKindTag, text: &str) -> Result<String, ()> {
    if kind_tag != ZddsDynKindTag::Boolean {
        return Ok(text.to_string());
    }
    match text {
        "True" | "TRUE" | "true" => Ok("TRUE".to_string()),
        "False" | "FALSE" | "false" => Ok("FALSE".to_string()),
        _ => Err(()),
    }
}

/// Adds a nested/complex member whose type is `nested`, already finished.
///
/// # Safety
/// `b`/`nested` non-NULL and live; `name` NUL-terminated or NULL.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn zerodds_dyn_builder_add_nested_member(
    b: *mut ZddsDynBuilder,
    shape_tag: u32,
    id: u64,
    name: *const c_char,
    name_len: usize,
    container_bound: u64,
    nested: *const ZddsDynType,
) -> i32 {
    let _ = name_len;
    if b.is_null() || nested.is_null() {
        return INVALID;
    }
    let Some(shape_tag) = ZddsDynShapeTag::from_u32(shape_tag) else {
        return INVALID;
    };
    let Ok(name) = cstr_or_empty(name) else {
        return INVALID;
    };
    // SAFETY: non-NULL per above.
    let (b, nested) = unsafe { (&mut *b, &*nested) };
    let mut memo = HashMap::new();
    let Ok(element) = graft(&mut b.model, nested.ty.ast(), nested.ty.id(), &mut memo) else {
        return ERR;
    };
    // A nested/complex member's `default_value`: `.msg`'s grammar has no
    // syntax for a nested-message-typed field's default, so
    // `TypeDescription` generation never fills one, and this — like
    // `rmw_zerodds_dynamic.cpp`'s `add_complex_*` wrappers — takes none.
    // See the module note.
    add_member_of_shape(b, element, shape_tag, container_bound, id, &name, None)
}

/// As [`zerodds_dyn_builder_add_nested_member`], but the nested type's
/// own builder has not been finished yet — the
/// `add_complex_*_member_builder` family, for a type graph the caller is
/// still assembling bottom-up.
///
/// # Safety
/// `b`/`nested` non-NULL and live; `name` NUL-terminated or NULL.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn zerodds_dyn_builder_add_nested_member_from_builder(
    b: *mut ZddsDynBuilder,
    shape_tag: u32,
    id: u64,
    name: *const c_char,
    name_len: usize,
    container_bound: u64,
    nested: *const ZddsDynBuilder,
) -> i32 {
    let _ = name_len;
    if b.is_null() || nested.is_null() {
        return INVALID;
    }
    let Some(shape_tag) = ZddsDynShapeTag::from_u32(shape_tag) else {
        return INVALID;
    };
    let Ok(name) = cstr_or_empty(name) else {
        return INVALID;
    };
    // SAFETY: non-NULL per above. Two distinct builders: no aliasing.
    let (b, nested) = unsafe { (&mut *b, &*nested) };
    let mut memo = HashMap::new();
    let Ok(element) = graft(&mut b.model, nested.model.model(), nested.root, &mut memo) else {
        return ERR;
    };
    // See `zerodds_dyn_builder_add_nested_member`'s note: no default.
    add_member_of_shape(b, element, shape_tag, container_bound, id, &name, None)
}

fn add_member_of_shape(
    b: &mut ZddsDynBuilder,
    element: Id<TypeNode>,
    shape_tag: ZddsDynShapeTag,
    container_bound: u64,
    id: u64,
    name: &str,
    default: Option<&str>,
) -> i32 {
    let member_type = match shape_tag {
        ZddsDynShapeTag::Scalar => Ok(element),
        ZddsDynShapeTag::Array => {
            let length = u32::try_from(container_bound).unwrap_or(0);
            if length == 0 {
                return INVALID;
            }
            b.model.array(element, &[length])
        }
        ZddsDynShapeTag::BoundedSequence => {
            let bound = u32::try_from(container_bound).unwrap_or(0);
            if bound == 0 {
                return INVALID;
            }
            b.model.sequence(element, Some(bound))
        }
        ZddsDynShapeTag::UnboundedSequence => b.model.sequence(element, None),
    };
    let Ok(member_type) = member_type else {
        return ERR;
    };
    let mut member = NewMember::new(name, member_type);
    if let Some(explicit_id) = member_id_from_u64(id) {
        member = member.with_id(explicit_id);
    }
    if let Some(text) = default {
        member = member.with_default(text);
    }
    match b.model.add_member(b.root, &member) {
        Ok(()) => OK,
        Err(_) => ERR,
    }
}

/// Finishes `b` into a self-contained `ZddsDynType`, consuming `b`.
/// NULL on failure (`b` is still consumed either way, matching the
/// interface's "OUT" contract: `dynamic_type_init_from_dynamic_type_builder`
/// takes the builder and hands back a type, not a shared view of it).
///
/// # Safety
/// `b` non-NULL, live, and not reused after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_builder_finish(b: *mut ZddsDynBuilder) -> *mut ZddsDynType {
    if b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL, caller relinquishes ownership per this fn's doc.
    let owned = unsafe { Box::from_raw(b) };
    let ZddsDynBuilder {
        model,
        root,
        original_name,
        collection_defaults,
    } = *owned;
    let Ok(ty) = model.finish_as(root) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ZddsDynType {
        ty,
        original_name,
        collection_defaults: Arc::new(collection_defaults),
    }))
}

/// As [`zerodds_dyn_builder_finish`], but produces the type's
/// default-valued `ZddsDynData` directly —
/// `dynamic_data_init_from_dynamic_type_builder`.
///
/// # Safety
/// As [`zerodds_dyn_builder_finish`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_from_builder(b: *mut ZddsDynBuilder) -> *mut ZddsDynData {
    // SAFETY: forwarded.
    let ty = unsafe { zerodds_dyn_builder_finish(b) };
    if ty.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: freshly produced above, sole owner.
    let out = unsafe { zerodds_dyn_data_from_type(ty) };
    // SAFETY: `zerodds_dyn_data_from_type` clones what it needs; this
    // handle's onward lifetime is the data's, matching
    // `dynamic_message_type_support_impl_t`'s documented ownership (the
    // struct owns `dynamic_message_type`; the type used to seed a data's
    // default value is not additionally retained by that data).
    unsafe { zerodds_dyn_type_free(ty) };
    out
}

// =====================================================================
// Type
// =====================================================================

/// Frees a `ZddsDynType`.
///
/// # Safety
/// `t` a `ZddsDynType` this module allocated, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_type_free(t: *mut ZddsDynType) {
    if t.is_null() {
        return;
    }
    // SAFETY: as `zerodds_dyn_builder_free`.
    drop(unsafe { Box::from_raw(t) });
}

/// Deep-clones a `ZddsDynType`. NULL on failure.
///
/// # Safety
/// `t` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_type_clone(t: *const ZddsDynType) -> *mut ZddsDynType {
    if t.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL.
    let t = unsafe { &*t };
    let mut model = DynamicTypeBuilder::new();
    let mut memo = HashMap::new();
    let Ok(root) = graft(&mut model, t.ty.ast(), t.ty.id(), &mut memo) else {
        return std::ptr::null_mut();
    };
    let Ok(ty) = model.finish_as(root) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ZddsDynType {
        ty,
        original_name: t.original_name.clone(),
        collection_defaults: Arc::clone(&t.collection_defaults),
    }))
}

/// Structural equality — `dynamic_type_equals`.
///
/// # Safety
/// `a`/`b` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_type_equals(
    a: *const ZddsDynType,
    b: *const ZddsDynType,
) -> u8 {
    if a.is_null() || b.is_null() {
        return 0;
    }
    // SAFETY: non-NULL.
    let (a, b) = unsafe { (&*a, &*b) };
    u8::from(a.ty.equals(&b.ty))
}

/// `dynamic_type_get_member_count`.
///
/// # Safety
/// `t` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_type_member_count(t: *const ZddsDynType) -> u64 {
    if t.is_null() {
        return 0;
    }
    // SAFETY: non-NULL.
    u64::from(unsafe { &*t }.ty.member_count())
}

/// Borrows the type's display name.
///
/// # Safety
/// `t` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_type_get_name(
    t: *const ZddsDynType,
    out_ptr: *mut *const c_char,
    out_len: *mut usize,
) -> i32 {
    if t.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let t = unsafe { &*t };
    let bytes = t.original_name.as_bytes();
    // SAFETY: checked non-NULL above.
    unsafe {
        *out_ptr = t.original_name.as_ptr();
        *out_len = bytes.len();
    }
    OK
}

// =====================================================================
// Data
// =====================================================================

fn new_data(
    ty: DynamicType,
    value: DynamicData,
    member_name: &str,
    collection_defaults: CollectionDefaults,
) -> Result<ZddsDynData, ()> {
    let own_name = CString::new(member_name).map_err(|_| ())?;
    Ok(ZddsDynData {
        ty,
        value,
        scratch_bytes: Vec::new(),
        scratch_units: Vec::new(),
        own_name,
        loan_origin: None,
        loan_token: None,
        outstanding_loan: None,
        collection_defaults,
    })
}

/// Overwrites every top-level member `defaults` names with its own
/// recorded collection default. A no-op wherever `value` is not a
/// struct/union's `Aggregate` (a collection or a scalar has no *member*
/// to name) or `defaults` is empty (every collection member's default
/// was empty text, or there is none).
fn apply_collection_defaults(
    value: &mut DynamicData,
    defaults: &HashMap<String, Vec<DynamicData>>,
) {
    if defaults.is_empty() {
        return;
    }
    let DynamicData::Aggregate(members) = value else {
        return;
    };
    for (name, slot) in members {
        if let Some(elements) = defaults.get(name.as_str()) {
            *slot = DynamicData::Collection(elements.clone());
        }
    }
}

/// Produces `t`'s default-valued `ZddsDynData` —
/// `dynamic_data_init_from_dynamic_type`. NULL on failure.
///
/// # Safety
/// `t` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_from_type(t: *const ZddsDynType) -> *mut ZddsDynData {
    if t.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL.
    let t = unsafe { &*t };
    let Ok(mut value) =
        zerodds_wire_codec::default_of_type::<DynamicData>(t.ty.ast(), t.ty.id(), "")
    else {
        return std::ptr::null_mut();
    };
    apply_collection_defaults(&mut value, &t.collection_defaults);
    let name = t.ty.qualified_name();
    let Ok(data) = new_data(
        t.ty.clone(),
        value,
        &name,
        Arc::clone(&t.collection_defaults),
    ) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(data))
}

/// Frees a `ZddsDynData`.
///
/// # Safety
/// `d` a `ZddsDynData` this module allocated, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_free(d: *mut ZddsDynData) {
    if d.is_null() {
        return;
    }
    // SAFETY: as `zerodds_dyn_builder_free`.
    drop(unsafe { Box::from_raw(d) });
}

/// Deep-clones a `ZddsDynData`. NULL on failure.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_clone(d: *const ZddsDynData) -> *mut ZddsDynData {
    if d.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let name = d.own_name.to_string_lossy().into_owned();
    let Ok(clone) = new_data(
        d.ty.clone(),
        d.value.clone(),
        &name,
        Arc::clone(&d.collection_defaults),
    ) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(clone))
}

/// Value equality — `dynamic_data_equals`.
///
/// # Safety
/// `a`/`b` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_equals(
    a: *const ZddsDynData,
    b: *const ZddsDynData,
) -> u8 {
    if a.is_null() || b.is_null() {
        return 0;
    }
    // SAFETY: non-NULL.
    let (a, b) = unsafe { (&*a, &*b) };
    // §7.5.2.11.5: equal types, and values equal in the clause's sense —
    // members without regard to their order, a non-optional member left
    // out equal to its default.
    u8::from(a.ty.equals(&b.ty) && a.ty.values_equal(&a.value, &b.value).unwrap_or(false))
}

/// `dynamic_data_get_item_count` (§7.5.2.11.6): a collection's element
/// count, a bitmask's number of flags that are set, the number of members
/// a structure or a union holds, and one for a value that is atomic — a
/// primitive or an enumerated one.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_item_count(d: *const ZddsDynData) -> u64 {
    if d.is_null() {
        return 0;
    }
    // SAFETY: non-NULL.
    let value = &unsafe { &*d }.value;
    let count = match value {
        DynamicData::Bits(flags) => flags.len(),
        other => other
            .members()
            .map(<[_]>::len)
            .or_else(|| other.items().map(<[_]>::len))
            .unwrap_or(1),
    };
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// `dynamic_data_get_member_id_by_name`.
///
/// # Safety
/// `d` non-NULL and live; `out_id` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_member_id_by_name(
    d: *const ZddsDynData,
    name: *const c_char,
    name_len: usize,
    out_id: *mut u64,
) -> i32 {
    let _ = name_len;
    if d.is_null() || out_id.is_null() {
        return INVALID;
    }
    let Ok(name) = cstr_or_empty(name) else {
        return INVALID;
    };
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let Some(member) = d.ty.member_by_name(&name) else {
        return ERR;
    };
    // The id of a flag of a bitmask is its bit (a literal has
    // `MEMBER_ID_INVALID`, §7.5.2.7.5, and the bit is how a flag is
    // reached; see [`flag_name`]).
    let id = if matches!(d.value, DynamicData::Bits(_)) {
        let Some(bit) = member.literal_value() else {
            return ERR;
        };
        bit
    } else {
        member.id()
    };
    // SAFETY: out_id checked non-NULL above.
    unsafe { *out_id = u64::from(id) };
    OK
}

/// `dynamic_data_get_member_id_at_index`/`dynamic_data_get_array_index`:
/// both read as "the id at this position", which for a struct is the
/// member id at that declaration index and for a collection is the index
/// itself (a collection's "member id" *is* its element index — §7.5.2.1).
///
/// # Safety
/// `d` non-NULL and live; `out_id` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_id_at_index(
    d: *const ZddsDynData,
    index: u64,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() || out_id.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let index_usize = usize::try_from(index).unwrap_or(usize::MAX);
    let id = if d.value.members().is_some() {
        let Some(member) =
            d.ty.member_by_index(u32::try_from(index).unwrap_or(u32::MAX))
        else {
            return ERR;
        };
        u64::from(member.id())
    } else if d.value.items().is_some() {
        index
    } else {
        return ERR;
    };
    let _ = index_usize;
    // SAFETY: out_id checked non-NULL above.
    unsafe { *out_id = id };
    OK
}

/// `dynamic_data_get_name`: the type's qualified name for a root value,
/// or the member/loan name a nested value was produced under (see
/// [`zerodds_dyn_data_get_complex`]/[`zerodds_dyn_data_loan`]).
///
/// # Safety
/// `d` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_get_name(
    d: *const ZddsDynData,
    out_ptr: *mut *const c_char,
    out_len: *mut usize,
) -> i32 {
    if d.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let bytes = d.own_name.as_bytes();
    // SAFETY: checked non-NULL above.
    unsafe {
        *out_ptr = d.own_name.as_ptr();
        *out_len = bytes.len();
    }
    OK
}

/// `dynamic_data_clear_all_values` (§7.5.2.11.3): resets every
/// member/element to its default — a collection member's own recorded one
/// where it has one, the type's Table 9 default otherwise. Key members are
/// reset too; [`zerodds_dyn_data_clear_nonkey_values`] keeps them.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_clear_all_values(d: *mut ZddsDynData) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let Some(fresh) = cleared(d) else {
        return ERR;
    };
    d.value = fresh;
    OK
}

/// `dynamic_data_clear_nonkey_values` (§7.5.2.11.3): "exactly the same
/// effect as clear_all_values with one exception: the values of key
/// fields of aggregated types retain their values."
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_clear_nonkey_values(d: *mut ZddsDynData) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let Some(mut fresh) = cleared(d) else {
        return ERR;
    };
    if let (DynamicData::Aggregate(kept), DynamicData::Aggregate(entries)) = (&mut fresh, &d.value)
    {
        for member in d.ty.members() {
            if !member
                .descriptor()
                .is_ok_and(|descriptor| descriptor.is_key)
            {
                continue;
            }
            let Some((_, value)) = entries.iter().find(|(name, _)| name == member.name()) else {
                continue;
            };
            match kept.iter_mut().find(|(name, _)| name == member.name()) {
                Some((_, slot)) => *slot = value.clone(),
                None => kept.push((member.name().to_string(), value.clone())),
            }
        }
    }
    d.value = fresh;
    OK
}

/// The value `d` has once every member/element is cleared.
fn cleared(d: &ZddsDynData) -> Option<DynamicData> {
    let mut fresh =
        zerodds_wire_codec::default_of_type::<DynamicData>(d.ty.ast(), d.ty.id(), "").ok()?;
    apply_collection_defaults(&mut fresh, &d.collection_defaults);
    Some(fresh)
}

/// `dynamic_data_clear_value`: resets one member/element to its default
/// — a collection member's own recorded one where it has one (see the
/// module note); the member's own scalar `@default` where its
/// declaration carries one instead (§7.2.2.4.4.4.9); its type's Table 9
/// default otherwise. An element of a collection always takes its
/// type's default — a collection default names a *member*, and an
/// element is not one.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_clear_value(d: *mut ZddsDynData, id: u64) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    // §7.5.2.11.3: "If the sample is of a variable-length collection type,
    // remove the indicated element, shifting any subsequent elements to the
    // next-lowest index." An array's element is set to its default.
    if matches!(
        d.ty.resolve_alias().map(|resolved| resolved.kind()),
        Ok(zerodds_types::dynamic::TypeKind::Sequence)
    ) {
        if let DynamicData::Collection(items) = &mut d.value {
            let index = usize::try_from(id).unwrap_or(usize::MAX);
            if index >= items.len() {
                return ERR;
            }
            items.remove(index);
            return OK;
        }
    }
    let fresh: DynamicData = if d.value.members().is_some() {
        let Some(member) = d.ty.member_by_id(u32::try_from(id).unwrap_or(u32::MAX)) else {
            return ERR;
        };
        // §7.5.2.11.3: "If the indicated member is optional, remove it."
        if member
            .descriptor()
            .is_ok_and(|descriptor| descriptor.is_optional)
        {
            if let DynamicData::Aggregate(entries) = &mut d.value {
                entries.retain(|(name, _)| name != member.name());
            }
            return OK;
        }
        if let Some(elements) = d.collection_defaults.get(member.name()) {
            DynamicData::Collection(elements.clone())
        } else {
            let Ok(value) = member.default_value() else {
                return ERR;
            };
            value
        }
    } else {
        let Some(element_type) = element_type_for(d, id) else {
            return ERR;
        };
        let Ok(value) = zerodds_wire_codec::default_of_type::<DynamicData>(
            element_type.ast(),
            element_type.id(),
            "",
        ) else {
            return ERR;
        };
        value
    };
    write_slot(d, id, fresh)
}

/// The `DynamicType` of the member/element `id` names, for a struct
/// (member lookup) or a collection (the element type, `id` unused since
/// every element of a collection shares one type).
fn element_type_for(d: &ZddsDynData, id: u64) -> Option<DynamicType> {
    if d.value.members().is_some() {
        let member_id = u32::try_from(id).unwrap_or(u32::MAX);
        d.ty.member_by_id(member_id)
            .and_then(|m| m.dynamic_type().ok())
    } else if d.value.items().is_some() {
        d.ty.element_type()
    } else {
        None
    }
}

/// The Boolean a flag of a bitmask reads as: DDS-XTypes 1.3 §7.5.2.11.1,
/// "values correspond to the flags within the bitmask and are all of
/// Boolean type".
static FLAG_SET: DynamicData = DynamicData::Boolean(true);
static FLAG_CLEAR: DynamicData = DynamicData::Boolean(false);

/// The name of the flag at bit `id` of the bitmask `d` is, when it is one.
/// "Any bit within the bound of the bitmask may be accessed by its
/// index"; a bitmask value holds the names of the flags that are set, so
/// a bit that no flag names cannot be held and is refused.
fn flag_name(d: &ZddsDynData, id: u64) -> Option<String> {
    if !matches!(d.value, DynamicData::Bits(_)) {
        return None;
    }
    let bit = u32::try_from(id).ok()?;
    d.ty.resolve_alias()
        .ok()?
        .members()
        .into_iter()
        .find(|member| member.literal_value() == Some(bit))
        .map(|member| member.name().to_string())
}

/// Reads the value at `id` (a member id for a struct, an index for a
/// collection, the bit of a flag for a bitmask).
fn read_slot(d: &ZddsDynData, id: u64) -> Option<&DynamicData> {
    if let DynamicData::Bits(flags) = &d.value {
        let name = flag_name(d, id)?;
        let set = flags
            .iter()
            .any(|flag| matches!(flag, DynamicData::Text(text) if *text == name));
        return Some(if set { &FLAG_SET } else { &FLAG_CLEAR });
    }
    if let Some(entries) = d.value.members() {
        let member = d.ty.member_by_id(u32::try_from(id).unwrap_or(u32::MAX))?;
        entries
            .iter()
            .find(|(name, _)| name == member.name())
            .map(|(_, v)| v)
    } else if let Some(items) = d.value.items() {
        items.get(usize::try_from(id).unwrap_or(usize::MAX))
    } else {
        None
    }
}

/// Writes `new_value` at `id`, replacing what was there. A struct's slot
/// must already exist (created when the data was built from its type); a
/// collection's slot must already exist too — growing a collection is
/// [`zerodds_dyn_data_insert`]'s job, not a set's, matching the
/// interface's own split between `set_*_value` and `insert_*_value`.
fn write_slot(d: &mut ZddsDynData, id: u64, new_value: DynamicData) -> i32 {
    // A flag of a bitmask: the Boolean sets or clears it.
    if matches!(d.value, DynamicData::Bits(_)) {
        let (Some(name), DynamicData::Boolean(set)) = (flag_name(d, id), &new_value) else {
            return ERR;
        };
        let order: Vec<String> =
            d.ty.resolve_alias()
                .map(|ty| {
                    ty.members()
                        .into_iter()
                        .map(|member| member.name().to_string())
                        .collect()
                })
                .unwrap_or_default();
        let DynamicData::Bits(flags) = &mut d.value else {
            return ERR;
        };
        flags.retain(|flag| !matches!(flag, DynamicData::Text(text) if *text == name));
        if *set {
            flags.push(DynamicData::Text(name));
            flags.sort_by_key(|flag| match flag {
                DynamicData::Text(text) => order
                    .iter()
                    .position(|known| known == text)
                    .unwrap_or(usize::MAX),
                _ => usize::MAX,
            });
        }
        return OK;
    }
    match &mut d.value {
        DynamicData::Aggregate(entries) => {
            let Some(member) = d.ty.member_by_id(u32::try_from(id).unwrap_or(u32::MAX)) else {
                return ERR;
            };
            match entries.iter_mut().find(|(name, _)| name == member.name()) {
                Some((_, slot)) => {
                    *slot = new_value;
                    OK
                }
                None => {
                    entries.push((member.name().to_string(), new_value));
                    OK
                }
            }
        }
        DynamicData::Collection(items) => {
            let Some(index) = items.get_mut(usize::try_from(id).unwrap_or(usize::MAX)) else {
                return ERR;
            };
            *index = new_value;
            OK
        }
        _ => ERR,
    }
}

/// Appends `new_value` to a collection, answering its index (id).
fn append_slot(d: &mut ZddsDynData, new_value: DynamicData, out_id: *mut u64) -> i32 {
    let DynamicData::Collection(items) = &mut d.value else {
        return ERR;
    };
    let index = items.len();
    items.push(new_value);
    if !out_id.is_null() {
        // SAFETY: checked non-NULL above.
        unsafe { *out_id = u64::try_from(index).unwrap_or(u64::MAX) };
    }
    OK
}

macro_rules! scalar_get_set {
    ($get:ident, $set:ident, $insert:ident, $ty:ty, $variant:pat, $extract:expr, $build:expr) => {
        #[doc = concat!("`dynamic_data_get_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out` writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(d: *const ZddsDynData, id: u64, out: *mut $ty) -> i32 {
            if d.is_null() || out.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &*d };
            let Some(value) = read_slot(d, id) else {
                return ERR;
            };
            #[allow(clippy::redundant_closure_call)]
            let Some(extracted) = ($extract)(value) else {
                return ERR;
            };
            // SAFETY: out checked non-NULL above.
            unsafe { *out = extracted };
            OK
        }

        #[doc = concat!("`dynamic_data_set_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $set(d: *mut ZddsDynData, id: u64, value: $ty) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            #[allow(clippy::redundant_closure_call)]
            let built = ($build)(value);
            write_slot(d, id, built)
        }

        #[doc = concat!("`dynamic_data_insert_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out_id` writable or NULL.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(d: *mut ZddsDynData, value: $ty, out_id: *mut u64) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            #[allow(clippy::redundant_closure_call)]
            let built = ($build)(value);
            append_slot(d, built, out_id)
        }
    };
}

scalar_get_set!(
    zerodds_dyn_data_get_bool,
    zerodds_dyn_data_set_bool,
    zerodds_dyn_data_insert_bool,
    u8,
    _,
    |v: &DynamicData| v.as_bool().map(u8::from),
    |v: u8| DynamicData::Boolean(v != 0)
);

macro_rules! integer_get_set {
    ($get:ident, $set:ident, $insert:ident, $ty:ty, $kind:expr) => {
        #[doc = concat!("`dynamic_data_get_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out` writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(d: *const ZddsDynData, id: u64, out: *mut $ty) -> i32 {
            if d.is_null() || out.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &*d };
            let Some(DynamicData::Integer { value, .. }) = read_slot(d, id) else {
                return ERR;
            };
            let Ok(narrowed) = <$ty>::try_from(*value) else {
                return ERR;
            };
            // SAFETY: out checked non-NULL above.
            unsafe { *out = narrowed };
            OK
        }

        #[doc = concat!("`dynamic_data_set_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $set(d: *mut ZddsDynData, id: u64, value: $ty) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            write_slot(d, id, DynamicData::integer($kind, i128::from(value)))
        }

        #[doc = concat!("`dynamic_data_insert_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out_id` writable or NULL.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(d: *mut ZddsDynData, value: $ty, out_id: *mut u64) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            append_slot(d, DynamicData::integer($kind, i128::from(value)), out_id)
        }
    };
}

integer_get_set!(
    zerodds_dyn_data_get_int8,
    zerodds_dyn_data_set_int8,
    zerodds_dyn_data_insert_int8,
    i8,
    AstPrimitiveKind::Int8
);
integer_get_set!(
    zerodds_dyn_data_get_uint8,
    zerodds_dyn_data_set_uint8,
    zerodds_dyn_data_insert_uint8,
    u8,
    AstPrimitiveKind::UInt8
);
integer_get_set!(
    zerodds_dyn_data_get_byte,
    zerodds_dyn_data_set_byte,
    zerodds_dyn_data_insert_byte,
    u8,
    AstPrimitiveKind::Octet
);
integer_get_set!(
    zerodds_dyn_data_get_int16,
    zerodds_dyn_data_set_int16,
    zerodds_dyn_data_insert_int16,
    i16,
    AstPrimitiveKind::Int16
);
integer_get_set!(
    zerodds_dyn_data_get_uint16,
    zerodds_dyn_data_set_uint16,
    zerodds_dyn_data_insert_uint16,
    u16,
    AstPrimitiveKind::UInt16
);
integer_get_set!(
    zerodds_dyn_data_get_int32,
    zerodds_dyn_data_set_int32,
    zerodds_dyn_data_insert_int32,
    i32,
    AstPrimitiveKind::Int32
);
integer_get_set!(
    zerodds_dyn_data_get_uint32,
    zerodds_dyn_data_set_uint32,
    zerodds_dyn_data_insert_uint32,
    u32,
    AstPrimitiveKind::UInt32
);
integer_get_set!(
    zerodds_dyn_data_get_int64,
    zerodds_dyn_data_set_int64,
    zerodds_dyn_data_insert_int64,
    i64,
    AstPrimitiveKind::Int64
);
integer_get_set!(
    zerodds_dyn_data_get_uint64,
    zerodds_dyn_data_set_uint64,
    zerodds_dyn_data_insert_uint64,
    u64,
    AstPrimitiveKind::UInt64
);

macro_rules! char_get_set {
    ($get:ident, $set:ident, $insert:ident, $kind:expr) => {
        #[doc = concat!("`dynamic_data_get_", stringify!($get), "_value`: the character as a")]
        #[doc = "UTF-32 code point."]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out` writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(d: *const ZddsDynData, id: u64, out: *mut u32) -> i32 {
            if d.is_null() || out.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &*d };
            let Some(ch) = read_slot(d, id).and_then(DynamicData::as_char) else {
                return ERR;
            };
            // SAFETY: out checked non-NULL above.
            unsafe { *out = ch as u32 };
            OK
        }

        #[doc = concat!("`dynamic_data_set_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $set(d: *mut ZddsDynData, id: u64, value: u32) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            let Some(ch) = char::from_u32(value) else {
                return INVALID;
            };
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            write_slot(d, id, DynamicData::character($kind, ch))
        }

        #[doc = concat!("`dynamic_data_insert_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out_id` writable or NULL.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(d: *mut ZddsDynData, value: u32, out_id: *mut u64) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            let Some(ch) = char::from_u32(value) else {
                return INVALID;
            };
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            append_slot(d, DynamicData::character($kind, ch), out_id)
        }
    };
}

char_get_set!(
    zerodds_dyn_data_get_char,
    zerodds_dyn_data_set_char,
    zerodds_dyn_data_insert_char,
    AstPrimitiveKind::Char
);
char_get_set!(
    zerodds_dyn_data_get_wchar,
    zerodds_dyn_data_set_wchar,
    zerodds_dyn_data_insert_wchar,
    AstPrimitiveKind::WChar
);

macro_rules! float_get_set {
    ($get:ident, $set:ident, $insert:ident, $ty:ty, $kind:expr, $cast:ty) => {
        #[doc = concat!("`dynamic_data_get_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out` writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(d: *const ZddsDynData, id: u64, out: *mut $ty) -> i32 {
            if d.is_null() || out.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &*d };
            let Some(DynamicData::Float { value, .. }) = read_slot(d, id) else {
                return ERR;
            };
            // SAFETY: out checked non-NULL above.
            unsafe { *out = *value as $cast };
            OK
        }

        #[doc = concat!("`dynamic_data_set_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $set(d: *mut ZddsDynData, id: u64, value: $ty) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            write_slot(d, id, DynamicData::float($kind, f64::from(value)))
        }

        #[doc = concat!("`dynamic_data_insert_", stringify!($get), "_value`.")]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out_id` writable or NULL.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(d: *mut ZddsDynData, value: $ty, out_id: *mut u64) -> i32 {
            if d.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            append_slot(d, DynamicData::float($kind, f64::from(value)), out_id)
        }
    };
}

float_get_set!(
    zerodds_dyn_data_get_float32,
    zerodds_dyn_data_set_float32,
    zerodds_dyn_data_insert_float32,
    f32,
    AstPrimitiveKind::Float32,
    f32
);

/// `dynamic_data_get_float64_value`.
///
/// # Safety
/// `d` non-NULL and live; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_get_float64(
    d: *const ZddsDynData,
    id: u64,
    out: *mut f64,
) -> i32 {
    if d.is_null() || out.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let Some(DynamicData::Float { value, .. }) = read_slot(d, id) else {
        return ERR;
    };
    // SAFETY: out checked non-NULL above.
    unsafe { *out = *value };
    OK
}

/// `dynamic_data_set_float64_value`.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_set_float64(
    d: *mut ZddsDynData,
    id: u64,
    value: f64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    write_slot(d, id, DynamicData::float(AstPrimitiveKind::Float64, value))
}

/// `dynamic_data_insert_float64_value`.
///
/// # Safety
/// `d` non-NULL and live; `out_id` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_insert_float64(
    d: *mut ZddsDynData,
    value: f64,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    append_slot(
        d,
        DynamicData::float(AstPrimitiveKind::Float64, value),
        out_id,
    )
}

macro_rules! float128_get_set {
    ($get:ident, $set:ident, $insert:ident, $to_bytes:expr, $from_bytes:expr) => {
        #[doc = concat!("`dynamic_data_get_float128_value`, as sixteen raw bytes (see the")]
        #[doc = "module note on `long double`)."]
        ///
        /// # Safety
        /// `d` non-NULL and live; `out16` a writable 16-byte buffer.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(d: *const ZddsDynData, id: u64, out16: *mut u8) -> i32 {
            if d.is_null() || out16.is_null() {
                return INVALID;
            }
            // SAFETY: non-NULL.
            let d = unsafe { &*d };
            let Some(DynamicData::LongDouble(value)) = read_slot(d, id) else {
                return ERR;
            };
            #[allow(clippy::redundant_closure_call)]
            let bytes: [u8; 16] = ($to_bytes)(*value);
            // SAFETY: out16 checked non-NULL above, 16 bytes per contract.
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out16, 16) };
            OK
        }

        #[doc = "`dynamic_data_set_float128_value`, from sixteen raw bytes."]
        ///
        /// # Safety
        /// `d` non-NULL and live; `in16` readable for 16 bytes.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $set(d: *mut ZddsDynData, id: u64, in16: *const u8) -> i32 {
            if d.is_null() || in16.is_null() {
                return INVALID;
            }
            let mut bytes = [0u8; 16];
            // SAFETY: in16 checked non-NULL above, 16 bytes per contract.
            unsafe { std::ptr::copy_nonoverlapping(in16, bytes.as_mut_ptr(), 16) };
            #[allow(clippy::redundant_closure_call)]
            let value = ($from_bytes)(bytes);
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            write_slot(d, id, DynamicData::LongDouble(value))
        }

        #[doc = "`dynamic_data_insert_float128_value`, from sixteen raw bytes."]
        ///
        /// # Safety
        /// `d` non-NULL and live; `in16` readable for 16 bytes; `out_id`
        /// writable or NULL.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(
            d: *mut ZddsDynData,
            in16: *const u8,
            out_id: *mut u64,
        ) -> i32 {
            if d.is_null() || in16.is_null() {
                return INVALID;
            }
            let mut bytes = [0u8; 16];
            // SAFETY: as `$set`.
            unsafe { std::ptr::copy_nonoverlapping(in16, bytes.as_mut_ptr(), 16) };
            #[allow(clippy::redundant_closure_call)]
            let value = ($from_bytes)(bytes);
            // SAFETY: non-NULL.
            let d = unsafe { &mut *d };
            append_slot(d, DynamicData::LongDouble(value), out_id)
        }
    };
}

float128_get_set!(
    zerodds_dyn_data_get_float128_x87,
    zerodds_dyn_data_set_float128_x87,
    zerodds_dyn_data_insert_float128_x87,
    |v: Float128| v.to_x87_padded_bytes().0,
    Float128::from_x87_padded_bytes
);
float128_get_set!(
    zerodds_dyn_data_get_float128_ieee,
    zerodds_dyn_data_set_float128_ieee,
    zerodds_dyn_data_insert_float128_ieee,
    Float128::to_le_bytes,
    Float128::from_le_bytes
);

// -- strings ----------------------------------------------------------

/// Borrows `id`'s string value as UTF-8 into `d`'s scratch buffer —
/// `dynamic_data_get_string_value`/`_fixed_string_value`/
/// `_bounded_string_value` alike (the interface's three string-getter
/// families read back identically; `string_length`/`string_bound` name a
/// constraint on how the value was *declared*, not a second wire form).
///
/// # Safety
/// `d` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_get_string(
    d: *mut ZddsDynData,
    id: u64,
    out_ptr: *mut *const c_char,
    out_len: *mut usize,
) -> i32 {
    if d.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let text = match read_slot(d, id).and_then(zerodds_wire_codec::Source::as_text) {
        Some(text) => text.to_string(),
        None => return ERR,
    };
    d.scratch_bytes.clear();
    d.scratch_bytes.extend_from_slice(text.as_bytes());
    let ptr = d.scratch_bytes.as_ptr().cast::<c_char>();
    let len = d.scratch_bytes.len();
    // SAFETY: out_ptr/out_len checked non-NULL above.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    OK
}

/// Sets `id`'s string value from UTF-8, honouring `bound` (0 = unbounded)
/// — `dynamic_data_set_string_value`/`_fixed_string_value`/
/// `_bounded_string_value`. Refused (not truncated) past `bound`: a
/// silently truncated string on a bounded/fixed member would put a
/// shorter value on the wire than the caller asked to send.
///
/// # Safety
/// `d` non-NULL and live; `(ptr, len)` a valid UTF-8 byte span.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_set_string(
    d: *mut ZddsDynData,
    id: u64,
    ptr: *const u8,
    len: usize,
    bound: u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    let bytes = bytes_or_empty(ptr, len);
    let Ok(text) = std::str::from_utf8(bytes) else {
        return INVALID;
    };
    if bound != 0 && u64::try_from(text.chars().count()).unwrap_or(u64::MAX) > bound {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    write_slot(d, id, DynamicData::text(text.to_string()))
}

/// `dynamic_data_insert_string_value`/`_fixed_string_value`/
/// `_bounded_string_value`.
///
/// # Safety
/// As [`zerodds_dyn_data_set_string`]; `out_id` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_insert_string(
    d: *mut ZddsDynData,
    ptr: *const u8,
    len: usize,
    bound: u64,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    let bytes = bytes_or_empty(ptr, len);
    let Ok(text) = std::str::from_utf8(bytes) else {
        return INVALID;
    };
    if bound != 0 && u64::try_from(text.chars().count()).unwrap_or(u64::MAX) > bound {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    append_slot(d, DynamicData::text(text.to_string()), out_id)
}

/// Borrows `id`'s wide-string value as UTF-16 code units into `d`'s
/// scratch buffer.
///
/// # Safety
/// `d` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_get_wstring(
    d: *mut ZddsDynData,
    id: u64,
    out_ptr: *mut *const u16,
    out_len: *mut usize,
) -> i32 {
    if d.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let text = match read_slot(d, id).and_then(zerodds_wire_codec::Source::as_text) {
        Some(text) => text.to_string(),
        None => return ERR,
    };
    d.scratch_units.clear();
    d.scratch_units.extend(text.encode_utf16());
    let ptr = d.scratch_units.as_ptr();
    let len = d.scratch_units.len();
    // SAFETY: out_ptr/out_len checked non-NULL above.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    OK
}

/// Sets `id`'s wide-string value from UTF-16 code units, honouring
/// `bound` (0 = unbounded).
///
/// # Safety
/// `d` non-NULL and live; `(ptr, len)` a valid UTF-16 span.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_set_wstring(
    d: *mut ZddsDynData,
    id: u64,
    ptr: *const u16,
    len: usize,
    bound: u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    let units = units_or_empty(ptr, len);
    let Ok(text) = String::from_utf16(units) else {
        return INVALID;
    };
    if bound != 0 && u64::try_from(units.len()).unwrap_or(u64::MAX) > bound {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    write_slot(d, id, DynamicData::text(text))
}

/// `dynamic_data_insert_wstring_value` and its `_fixed`/`_bounded` kin.
///
/// # Safety
/// As [`zerodds_dyn_data_set_wstring`]; `out_id` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_insert_wstring(
    d: *mut ZddsDynData,
    ptr: *const u16,
    len: usize,
    bound: u64,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    let units = units_or_empty(ptr, len);
    let Ok(text) = String::from_utf16(units) else {
        return INVALID;
    };
    if bound != 0 && u64::try_from(units.len()).unwrap_or(u64::MAX) > bound {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    append_slot(d, DynamicData::text(text), out_id)
}

// -- sequences ----------------------------------------------------------

/// `dynamic_data_clear_sequence_data`: empties a collection.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_seq_clear(d: *mut ZddsDynData) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let DynamicData::Collection(items) = &mut unsafe { &mut *d }.value else {
        return ERR;
    };
    items.clear();
    OK
}

/// `dynamic_data_remove_sequence_data`: removes the element at index
/// `id`, shifting later elements down (their ids/indices change — the
/// same renumbering removing an element from any `sequence<T>` on the
/// wire implies).
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_seq_remove(d: *mut ZddsDynData, id: u64) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let DynamicData::Collection(items) = &mut unsafe { &mut *d }.value else {
        return ERR;
    };
    let index = usize::try_from(id).unwrap_or(usize::MAX);
    if index >= items.len() {
        return ERR;
    }
    items.remove(index);
    OK
}

/// `dynamic_data_insert_sequence_data`: appends the element type's
/// default value, answering its index.
///
/// # Safety
/// `d` non-NULL and live; `out_id` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_seq_insert_default(
    d: *mut ZddsDynData,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let Some(element_type) = d.ty.element_type() else {
        return ERR;
    };
    let Ok(fresh) = zerodds_wire_codec::default_of_type::<DynamicData>(
        element_type.ast(),
        element_type.id(),
        "",
    ) else {
        return ERR;
    };
    append_slot(d, fresh, out_id)
}

// -- nested -------------------------------------------------------------

/// `dynamic_data_get_complex_value`: a clone of the nested value at
/// `id`, as its own `ZddsDynData`. NULL on failure.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_get_complex(
    d: *const ZddsDynData,
    id: u64,
) -> *mut ZddsDynData {
    if d.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let Some(value) = read_slot(d, id).cloned() else {
        return std::ptr::null_mut();
    };
    let Some(member_type) = element_type_for(d, id) else {
        return std::ptr::null_mut();
    };
    let name = member_type.qualified_name();
    // A spawned child's own type is not `d.ty`, so `d`'s collection
    // defaults do not name any member of it — see the field's doc.
    let Ok(child) = new_data(member_type, value, &name, CollectionDefaults::default()) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(child))
}

/// `dynamic_data_set_complex_value`: overwrites the value at `id` with a
/// copy of `value`'s tree.
///
/// # Safety
/// `d`/`value` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_set_complex(
    d: *mut ZddsDynData,
    id: u64,
    value: *const ZddsDynData,
) -> i32 {
    if d.is_null() || value.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL, and distinct handles (the interface never passes
    // a data's own descendant back to itself as a whole value).
    let (d, value) = unsafe { (&mut *d, &*value) };
    write_slot(d, id, value.value.clone())
}

/// `dynamic_data_insert_complex_value_copy`: appends a copy of `value`'s
/// tree to a collection.
///
/// # Safety
/// `d`/`value` non-NULL and live; `out_id` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_insert_complex_copy(
    d: *mut ZddsDynData,
    value: *const ZddsDynData,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() || value.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let (d, value) = unsafe { (&mut *d, &*value) };
    append_slot(d, value.value.clone(), out_id)
}

/// `dynamic_data_insert_complex_value`: appends `value`'s tree, then
/// frees `value` (the non-copying insert — the interface's counterpart
/// to [`zerodds_dyn_data_insert_complex_copy`]).
///
/// # Safety
/// `d`/`value` non-NULL and live; `value` not used again after this
/// call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_insert_complex(
    d: *mut ZddsDynData,
    value: *mut ZddsDynData,
    out_id: *mut u64,
) -> i32 {
    if d.is_null() || value.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL, sole owner per this fn's doc.
    let owned = unsafe { Box::from_raw(value) };
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    append_slot(d, owned.value, out_id)
}

/// The numbers loans are tagged with; unique across the process, so that a
/// loan matches the parent it came from and no other.
static LOAN_TOKENS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// `dynamic_data_loan_value` (§7.5.2.11.7): a clone of the value at `id`,
/// tagged so [`zerodds_dyn_data_return_loan`] knows where to write it back.
///
/// Implemented as copy-out/write-back rather than a true zero-copy
/// alias: [`DynamicData`]'s tree has no interior-mutable child-borrow
/// primitive to alias into safely across the FFI boundary, and the
/// sequential loan/return pattern this API is documented for — loan,
/// mutate, return, and only then loan or mutate the parent again — is
/// behaviourally identical either way.
///
/// "A given DynamicData object may support only a single outstanding loan
/// at a time […] If an application violates this constraint, the loan
/// operation shall return a nil value". A loan of a value that does not
/// exist is nil as well.
///
/// # Safety
/// `d` non-NULL and live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_loan(d: *mut ZddsDynData, id: u64) -> *mut ZddsDynData {
    if d.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: non-NULL.
    if unsafe { &*d }.outstanding_loan.is_some() {
        return std::ptr::null_mut();
    }
    // SAFETY: forwarded.
    let child = unsafe { zerodds_dyn_data_get_complex(d, id) };
    if child.is_null() {
        return child;
    }
    let token = LOAN_TOKENS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: freshly produced above, sole owner, non-NULL.
    let child_ref = unsafe { &mut *child };
    child_ref.loan_origin = Some(id);
    child_ref.loan_token = Some(token);
    // SAFETY: non-NULL, checked above.
    unsafe { &mut *d }.outstanding_loan = Some(token);
    child
}

/// `dynamic_data_return_loaned_value`: writes `loaned` back into `d` at
/// the id it was loaned at ([`zerodds_dyn_data_loan`]'s tag — the
/// interface's own C signature for this call carries no member id, only
/// the parent and the loaned value, so the loaned value has to remember
/// it) and frees it.
///
/// §7.5.2.11.7: "The return_loaned_value operation shall return
/// RETCODE_PRECONDITION_NOT_MET if the provided sample object does not
/// represent an outstanding loan from the sample on which the operation is
/// invoked." A value that is not a loan, one from another parent and one
/// that was already returned are refused, RCUTILS_RET_INVALID_ARGUMENT,
/// and write nothing.
///
/// # Safety
/// `d`/`loaned` non-NULL and live; `loaned` not used again after this
/// call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_return_loan(
    d: *mut ZddsDynData,
    loaned: *mut ZddsDynData,
) -> i32 {
    if d.is_null() || loaned.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL, sole owner of `loaned` per this fn's doc.
    let owned = unsafe { Box::from_raw(loaned) };
    let (Some(id), Some(token)) = (owned.loan_origin, owned.loan_token) else {
        return INVALID;
    };
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    if d.outstanding_loan != Some(token) {
        return INVALID;
    }
    let status = write_slot(d, id, owned.value);
    if status == OK {
        d.outstanding_loan = None;
    }
    status
}

// -- serialize / deserialize --------------------------------------------

/// The 4-byte RTPS/XTypes encapsulation header identifying `@final`
/// little-endian PLAIN_CDR1 — `USER_PAYLOAD_ENCAP` in `zerodds-dcps`,
/// duplicated here rather than depending on `zerodds-dcps` for four
/// bytes: this module's only dependency on the DDS entity/writer stack
/// otherwise, and the one wire representation every struct this module
/// builds ever needs (see the module note on extensibility).
const FINAL_LE_ENCAP: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

fn representation_of(header: [u8; 4]) -> Option<(Encoding, Endianness)> {
    let id = header[1];
    let big_endian = id & 1 == 0;
    let encoding = match id & 0xFE {
        0x00 => Encoding::PlainCdr1,
        0x02 => Encoding::PlCdr1,
        0x06 => Encoding::PlainCdr2,
        0x08 => Encoding::DelimitedCdr,
        0x0a => Encoding::PlCdr2,
        _ => return None,
    };
    let endianness = if big_endian {
        Endianness::Big
    } else {
        Endianness::Little
    };
    Some((encoding, endianness))
}

/// The CDR dialect this process's dynamic type support reads and writes in:
/// `wire.codec_dialect` of the same configuration layers the node's
/// participant is started from (environment, files, `--set`), read once.
/// The interface hands these functions no participant to ask. A layer that
/// does not load leaves the specification's; `rmw_zerodds_init` refuses the
/// same configuration loudly.
fn codec_dialect() -> Dialect {
    static DIALECT: std::sync::OnceLock<Dialect> = std::sync::OnceLock::new();
    *DIALECT.get_or_init(|| {
        crate::ros2_config::process_source()
            .ok()
            .and_then(|source| zerodds_dcps::runtime::RuntimeConfig::from_source(&source).ok())
            .map(|config| config.codec_dialect)
            .unwrap_or_default()
    })
}

/// `dynamic_data_serialize`: the value as a CDR stream *with* its 4-byte
/// encapsulation header (`rmw_zerodds_take`'s own buffers carry the same
/// header; `rmw_serialize`/`rmw_take_serialized_message` across every rmw
/// implementation do too). Always `@final` little-endian PLAIN_CDR1 —
/// see the module note; a writer-negotiated `DataRepresentation` QoS is
/// not reachable from a bare `dynamic_data_impl_t*`, since the interface
/// gives this call no writer/QoS context at all.
///
/// # Safety
/// `d` non-NULL and live; `out_ptr`/`out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_serialize(
    d: *const ZddsDynData,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if d.is_null() || out_ptr.is_null() || out_len.is_null() {
        return INVALID;
    }
    // SAFETY: non-NULL.
    let d = unsafe { &*d };
    let request = ProbeRequest::in_dialect(
        &d.ty.qualified_name(),
        Encoding::PlainCdr1,
        Endianness::Little,
        codec_dialect(),
    );
    let Ok(body) = zerodds_wire_codec::encode(d.ty.ast(), &request, &d.value) else {
        return ERR;
    };
    let mut stream = Vec::with_capacity(body.len() + 4);
    stream.extend_from_slice(&FINAL_LE_ENCAP);
    stream.extend_from_slice(&body);
    let boxed = stream.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed).cast::<u8>();
    // SAFETY: out_ptr/out_len checked non-NULL above.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    OK
}

/// `dynamic_data_deserialize`: replaces the value from a CDR stream that
/// carries its own 4-byte encapsulation header — the representation "the
/// topic uses" is read from that header rather than assumed, so a real
/// wire sample decodes in whatever representation its writer actually
/// used.
///
/// # Safety
/// `d` non-NULL and live; `(bytes_ptr, bytes_len)` a valid byte span of
/// at least 4 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_data_deserialize(
    d: *mut ZddsDynData,
    bytes_ptr: *const u8,
    bytes_len: usize,
) -> i32 {
    if d.is_null() {
        return INVALID;
    }
    let bytes = bytes_or_empty(bytes_ptr, bytes_len);
    if bytes.len() < 4 {
        return INVALID;
    }
    let header = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let Some((encoding, endianness)) = representation_of(header) else {
        return INVALID;
    };
    // SAFETY: non-NULL.
    let d = unsafe { &mut *d };
    let request = ProbeRequest::in_dialect(
        &d.ty.qualified_name(),
        encoding,
        endianness,
        codec_dialect(),
    );
    let Ok(decoded) = zerodds_wire_codec::decode::<DynamicData>(d.ty.ast(), &request, &bytes[4..])
    else {
        return ERR;
    };
    d.value = decoded;
    OK
}

/// Frees a buffer [`zerodds_dyn_data_serialize`] returned.
///
/// # Safety
/// `(ptr, len)` exactly as returned by [`zerodds_dyn_data_serialize`],
/// not freed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyn_bytes_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: as documented above — the exact `(ptr, len)` a matching
    // `into_raw` produced.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn build_string_struct() -> *mut ZddsDynType {
        // SAFETY: test-only, single-threaded, checked at each step.
        unsafe {
            let name = std::ffi::CString::new("std_msgs::msg::String").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let field = std::ffi::CString::new("data").unwrap();
            let rc = zerodds_dyn_builder_add_member(
                b,
                ZddsDynKindTag::String8 as u32,
                ZddsDynShapeTag::Scalar as u32,
                u64::from(MEMBER_ID_INVALID),
                field.as_ptr(),
                0,
                0,
                0,
                std::ptr::null(),
                0,
            );
            assert_eq!(rc, OK);
            let ty = zerodds_dyn_builder_finish(b);
            assert!(!ty.is_null());
            ty
        }
    }

    #[test]
    fn a_string_message_round_trips_through_get_set_and_cdr() {
        // SAFETY: see above.
        unsafe {
            let ty = build_string_struct();
            let d = zerodds_dyn_data_from_type(ty);
            assert!(!d.is_null());
            let mut id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"data".as_ptr(), 0, &raw mut id),
                OK
            );

            let text = b"hello dynamic";
            assert_eq!(
                zerodds_dyn_data_set_string(d, id, text.as_ptr(), text.len(), 0),
                OK
            );

            let mut ptr: *const c_char = std::ptr::null();
            let mut len = 0usize;
            assert_eq!(
                zerodds_dyn_data_get_string(d, id, &raw mut ptr, &raw mut len),
                OK
            );
            let round_tripped = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
            assert_eq!(round_tripped, text);

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0usize;
            assert_eq!(
                zerodds_dyn_data_serialize(d, &raw mut out_ptr, &raw mut out_len),
                OK
            );
            let bytes = std::slice::from_raw_parts(out_ptr, out_len);
            assert_eq!(&bytes[0..4], &FINAL_LE_ENCAP);

            let d2 = zerodds_dyn_data_from_type(ty);
            assert_eq!(zerodds_dyn_data_deserialize(d2, out_ptr, out_len), OK);
            assert_eq!(zerodds_dyn_data_equals(d, d2), 1);

            zerodds_dyn_bytes_free(out_ptr, out_len);
            zerodds_dyn_data_free(d);
            zerodds_dyn_data_free(d2);
            zerodds_dyn_type_free(ty);
        }
    }

    #[test]
    fn every_primitive_kind_round_trips() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::AllPrimitives").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let kinds = [
                (ZddsDynKindTag::Boolean, "f_bool"),
                (ZddsDynKindTag::Byte, "f_byte"),
                (ZddsDynKindTag::Char8, "f_char"),
                (ZddsDynKindTag::Char16, "f_wchar"),
                (ZddsDynKindTag::Float32, "f_f32"),
                (ZddsDynKindTag::Float64, "f_f64"),
                (ZddsDynKindTag::Float128, "f_f128"),
                (ZddsDynKindTag::Int8, "f_i8"),
                (ZddsDynKindTag::UInt8, "f_u8"),
                (ZddsDynKindTag::Int16, "f_i16"),
                (ZddsDynKindTag::UInt16, "f_u16"),
                (ZddsDynKindTag::Int32, "f_i32"),
                (ZddsDynKindTag::UInt32, "f_u32"),
                (ZddsDynKindTag::Int64, "f_i64"),
                (ZddsDynKindTag::UInt64, "f_u64"),
                (ZddsDynKindTag::String8, "f_str"),
                (ZddsDynKindTag::String16, "f_wstr"),
            ];
            for (kind, field) in kinds {
                let cname = std::ffi::CString::new(field).unwrap();
                let rc = zerodds_dyn_builder_add_member(
                    b,
                    kind as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    cname.as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0,
                );
                assert_eq!(rc, OK, "adding {field}");
            }
            let ty = zerodds_dyn_builder_finish(b);
            assert!(!ty.is_null());
            assert_eq!(zerodds_dyn_type_member_count(ty), kinds.len() as u64);

            let d = zerodds_dyn_data_from_type(ty);
            assert!(!d.is_null());

            let mut id = 0u64;
            macro_rules! mid {
                ($name:literal) => {{
                    let c = std::ffi::CString::new($name).unwrap();
                    assert_eq!(
                        zerodds_dyn_data_member_id_by_name(d, c.as_ptr(), 0, &raw mut id),
                        OK
                    );
                    id
                }};
            }

            assert_eq!(zerodds_dyn_data_set_bool(d, mid!("f_bool"), 1), OK);
            let mut b_out = 0u8;
            assert_eq!(
                zerodds_dyn_data_get_bool(d, mid!("f_bool"), &raw mut b_out),
                OK
            );
            assert_eq!(b_out, 1);

            assert_eq!(zerodds_dyn_data_set_int32(d, mid!("f_i32"), -7), OK);
            let mut i_out = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(d, mid!("f_i32"), &raw mut i_out),
                OK
            );
            assert_eq!(i_out, -7);

            assert_eq!(zerodds_dyn_data_set_uint64(d, mid!("f_u64"), u64::MAX), OK);
            let mut u_out = 0u64;
            assert_eq!(
                zerodds_dyn_data_get_uint64(d, mid!("f_u64"), &raw mut u_out),
                OK
            );
            assert_eq!(u_out, u64::MAX);

            assert_eq!(zerodds_dyn_data_set_float64(d, mid!("f_f64"), 1.5), OK);
            let mut f_out = 0f64;
            assert_eq!(
                zerodds_dyn_data_get_float64(d, mid!("f_f64"), &raw mut f_out),
                OK
            );
            assert!((f_out - 1.5).abs() < f64::EPSILON);

            let f128 = Float128::from_f64(3.25);
            let ieee = f128.to_le_bytes();
            assert_eq!(
                zerodds_dyn_data_set_float128_ieee(d, mid!("f_f128"), ieee.as_ptr()),
                OK
            );
            let mut ieee_out = [0u8; 16];
            assert_eq!(
                zerodds_dyn_data_get_float128_ieee(d, mid!("f_f128"), ieee_out.as_mut_ptr()),
                OK
            );
            assert_eq!(ieee_out, ieee);

            assert_eq!(
                zerodds_dyn_data_set_char(d, mid!("f_char"), u32::from('z')),
                OK
            );
            let mut ch_out = 0u32;
            assert_eq!(
                zerodds_dyn_data_get_char(d, mid!("f_char"), &raw mut ch_out),
                OK
            );
            assert_eq!(ch_out, u32::from('z'));

            let wide: [u16; 3] = [0x0041, 0x00e9, 0x4e2d]; // "Aé中"
            assert_eq!(
                zerodds_dyn_data_set_wstring(d, mid!("f_wstr"), wide.as_ptr(), wide.len(), 0),
                OK
            );
            let mut wp: *const u16 = std::ptr::null();
            let mut wl = 0usize;
            assert_eq!(
                zerodds_dyn_data_get_wstring(d, mid!("f_wstr"), &raw mut wp, &raw mut wl),
                OK
            );
            assert_eq!(std::slice::from_raw_parts(wp, wl), &wide);

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0usize;
            assert_eq!(
                zerodds_dyn_data_serialize(d, &raw mut out_ptr, &raw mut out_len),
                OK
            );
            let d2 = zerodds_dyn_data_from_type(ty);
            assert_eq!(zerodds_dyn_data_deserialize(d2, out_ptr, out_len), OK);
            assert_eq!(zerodds_dyn_data_equals(d, d2), 1);

            zerodds_dyn_bytes_free(out_ptr, out_len);
            zerodds_dyn_data_free(d);
            zerodds_dyn_data_free(d2);
            zerodds_dyn_type_free(ty);
        }
    }

    /// 3.25's 80-bit extended layout, written out by hand rather than
    /// produced by [`zerodds_float128::Float128::to_x87_padded_bytes`] —
    /// 3.25 is `11.01₂` = `1.101₂ × 2¹`, so the explicit-integer-bit
    /// significand is `0xD000_0000_0000_0000` and the biased exponent is
    /// `0x4000` (the extended bias `0x3FFF` plus the exponent `1`), sign
    /// `0`, six bytes of zero padding. Set through the x87 entry and read
    /// back through both the x87 and the true-binary128 entry, so a
    /// mismatch in either direction of either bridge shows. Pure byte
    /// conversion — no host-specific `long double` layout is read or
    /// written, so this runs on any host, this arm64 one included.
    #[test]
    fn a_known_x87_value_sets_and_reads_back_both_ways() {
        const X87_3_25: [u8; 16] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD0, // significand
            0x00, 0x40, // sign_exponent: sign 0, biased exponent 0x4000
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // padding
        ];
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::LongDouble").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let field = std::ffi::CString::new("value").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Float128 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0,
                ),
                OK
            );
            let ty = zerodds_dyn_builder_finish(b);
            assert!(!ty.is_null());
            let d = zerodds_dyn_data_from_type(ty);
            assert!(!d.is_null());
            let mut id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, field.as_ptr(), 0, &raw mut id),
                OK
            );

            assert_eq!(
                zerodds_dyn_data_set_float128_x87(d, id, X87_3_25.as_ptr()),
                OK
            );

            let mut back_x87 = [0u8; 16];
            assert_eq!(
                zerodds_dyn_data_get_float128_x87(d, id, back_x87.as_mut_ptr()),
                OK
            );
            assert_eq!(back_x87, X87_3_25, "the x87 bytes round-trip exactly");

            let mut back_ieee = [0u8; 16];
            assert_eq!(
                zerodds_dyn_data_get_float128_ieee(d, id, back_ieee.as_mut_ptr()),
                OK
            );
            assert_eq!(
                Float128::from_le_bytes(back_ieee),
                Float128::from_f64(3.25),
                "the same value reads back as exactly 3.25 in true binary128"
            );

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// A nested struct with a scalar `@default`, a `@key` member and an
    /// `@optional` member, composed into an outer type through
    /// `add_complex_member`'s Rust half
    /// ([`zerodds_dyn_builder_add_nested_member`]) — [`graft`]'s
    /// facet-for-facet fix. The nested type is built directly through
    /// [`DynamicTypeBuilder`]/[`NewMember`] rather than through this
    /// module's own `zerodds_dyn_builder_add_member`, which has no
    /// parameter for `@key`/`@optional`/`@must_understand`/`@external`
    /// at all (only a scalar's `default_value` reaches it) — the point
    /// being to prove the fix for a type that carries these facets
    /// *however* it reached this module, not only for the narrower set
    /// this module's own C entry points can produce today.
    ///
    /// Two checks: the nested member's own default applies after
    /// `dynamic_data_init_from_dynamic_type`, and the composed type's
    /// wire bytes are octet-for-octet what the identical declaration —
    /// written in IDL, resolved, and encoded directly with
    /// `zerodds_wire_codec::encode` — produces. The second check is
    /// where a dropped `@key` or a dropped `@optional` would actually
    /// show: an unkeyed structure and an always-present member lay out
    /// no differently *on their own*, but §7.4.1.1 lays out an
    /// `@optional` member's presence flag ahead of its value and this
    /// interface's own key handling reads `is_key` off the same facet
    /// the old `graft` never copied, so either drop moves every byte
    /// after it.
    #[test]
    fn a_grafted_members_facets_survive_and_match_idl_encoding() {
        // SAFETY: see above.
        unsafe {
            // -- the nested type, with the facets this module's own
            // add_member has no parameter for at all.
            let mut nested_model = DynamicTypeBuilder::new();
            let int32 = nested_model.primitive(DynKind::Int32).expect("int32");
            let inner = nested_model
                .structure("test::Inner", ExtensibilityKind::Final)
                .expect("the struct declares");
            nested_model
                .add_member(inner, &NewMember::new("id", int32).keyed())
                .expect("the key member adds");
            nested_model
                .add_member(inner, &NewMember::new("count", int32).with_default("42"))
                .expect("the defaulted member adds");
            nested_model
                .add_member(inner, &NewMember::new("extra", int32).optional())
                .expect("the optional member adds");
            let nested_ty = nested_model
                .finish_as(inner)
                .expect("the nested type finishes");
            let nested = ZddsDynType {
                ty: nested_ty,
                original_name: std::ffi::CString::new("test::Inner").unwrap(),
                collection_defaults: CollectionDefaults::default(),
            };

            // -- composed into the outer type through this module's own
            // public entry point for a finished nested type.
            let outer_name = std::ffi::CString::new("test::Outer").unwrap();
            let outer_b = zerodds_dyn_builder_init(outer_name.as_ptr(), 0);
            assert!(!outer_b.is_null());
            let field = std::ffi::CString::new("nested").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_nested_member(
                    outer_b,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    &raw const nested,
                ),
                OK
            );
            let outer_ty = zerodds_dyn_builder_finish(outer_b);
            assert!(!outer_ty.is_null());

            let d = zerodds_dyn_data_from_type(outer_ty);
            assert!(!d.is_null());
            let mut inner_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, field.as_ptr(), 0, &raw mut inner_id),
                OK
            );

            // The nested member's own @default, read back after init —
            // never reached at all before this fix (graft dropped
            // `default_value`, so `count` initialised at Table 9's 0).
            let child = zerodds_dyn_data_get_complex(d, inner_id);
            assert!(!child.is_null());
            let mut count_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(child, c"count".as_ptr(), 0, &raw mut count_id),
                OK
            );
            let mut count_out = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(child, count_id, &raw mut count_out),
                OK
            );
            assert_eq!(
                count_out, 42,
                "the nested member's own @default survives the graft"
            );

            // A concrete value for the @key field — left at its own
            // Table 9 zero, "id" would still round-trip through the two
            // encodings equally, telling this test nothing about
            // whether `is_key` really carried over.
            let mut id_field_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(child, c"id".as_ptr(), 0, &raw mut id_field_id),
                OK
            );
            assert_eq!(zerodds_dyn_data_set_int32(child, id_field_id, 7), OK);
            // "extra" (@optional) is left unset — Table 9's "no value at
            // all", the same as the IDL reference below leaves it.
            assert_eq!(zerodds_dyn_data_set_complex(d, inner_id, child), OK);
            zerodds_dyn_data_free(child);

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0usize;
            assert_eq!(
                zerodds_dyn_data_serialize(d, &raw mut out_ptr, &raw mut out_len),
                OK
            );
            let dynamic_bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            zerodds_dyn_bytes_free(out_ptr, out_len);

            // -- the same declaration, written in IDL and encoded
            // directly with zerodds_wire_codec: the independent
            // reference this module's own graft-composed type is
            // measured against.
            let idl = r#"
                @final
                struct Inner {
                    @key long id;
                    @default(42) long count;
                    @optional long extra;
                };
                @final
                struct Outer {
                    Inner nested;
                };
            "#;
            let parsed = zerodds_idl_parse::parse(idl);
            assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
            let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "graft_facets_test.idl");
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            let mut idl_ast = lowered.ast;
            let report = idl_ast.resolve_all_by_name().expect("the resolve pass");
            assert!(report.is_clean(), "{:?}", report.broken);

            let reference = DynamicData::Aggregate(vec![(
                "nested".to_string(),
                DynamicData::Aggregate(vec![
                    (
                        "id".to_string(),
                        DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 7),
                    ),
                    (
                        "count".to_string(),
                        DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 42),
                    ),
                    // "extra": absent, matching @optional's Table 9 default.
                ]),
            )]);

            let request = ProbeRequest::new("Outer", Encoding::PlainCdr1, Endianness::Little);
            let body = zerodds_wire_codec::encode(&idl_ast, &request, &reference)
                .expect("the reference value encodes");
            let mut reference_bytes = Vec::with_capacity(body.len() + 4);
            reference_bytes.extend_from_slice(&FINAL_LE_ENCAP);
            reference_bytes.extend_from_slice(&body);

            assert_eq!(
                dynamic_bytes, reference_bytes,
                "the grafted type's wire bytes match the same declaration made through IDL"
            );

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(outer_ty);
        }
    }

    #[test]
    fn nested_struct_and_sequence_round_trip() {
        // SAFETY: see above.
        unsafe {
            let inner_name = std::ffi::CString::new("test::Inner").unwrap();
            let inner_b = zerodds_dyn_builder_init(inner_name.as_ptr(), 0);
            assert!(!inner_b.is_null());
            let x = std::ffi::CString::new("x").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    inner_b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    x.as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0
                ),
                OK
            );
            let inner_ty = zerodds_dyn_builder_finish(inner_b);
            assert!(!inner_ty.is_null());

            let outer_name = std::ffi::CString::new("test::Outer").unwrap();
            let outer_b = zerodds_dyn_builder_init(outer_name.as_ptr(), 0);
            let scalar = std::ffi::CString::new("one").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_nested_member(
                    outer_b,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    scalar.as_ptr(),
                    0,
                    0,
                    inner_ty
                ),
                OK
            );
            let seq = std::ffi::CString::new("many").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_nested_member(
                    outer_b,
                    ZddsDynShapeTag::UnboundedSequence as u32,
                    u64::from(MEMBER_ID_INVALID),
                    seq.as_ptr(),
                    0,
                    0,
                    inner_ty
                ),
                OK
            );
            let outer_ty = zerodds_dyn_builder_finish(outer_b);
            assert!(!outer_ty.is_null());

            let d = zerodds_dyn_data_from_type(outer_ty);
            assert!(!d.is_null());
            let mut one_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"one".as_ptr(), 0, &raw mut one_id),
                OK
            );
            let child = zerodds_dyn_data_get_complex(d, one_id);
            assert!(!child.is_null());
            let mut inner_x_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(child, c"x".as_ptr(), 0, &raw mut inner_x_id),
                OK
            );
            assert_eq!(zerodds_dyn_data_set_int32(child, inner_x_id, 42), OK);
            assert_eq!(zerodds_dyn_data_set_complex(d, one_id, child), OK);
            zerodds_dyn_data_free(child);

            let mut many_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"many".as_ptr(), 0, &raw mut many_id),
                OK
            );
            // A sequence-shaped member's own elements are reached through
            // its *own* `ZddsDynData` (loaned/copied out via
            // `get_complex`), not through the parent aggregate — the same
            // two-step `dynamic_data_get_complex_value` then
            // `dynamic_data_insert_sequence_data` the real interface
            // documents.
            let many = zerodds_dyn_data_get_complex(d, many_id);
            assert!(!many.is_null());
            let mut elem_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_seq_insert_default(many, &raw mut elem_id),
                OK
            );
            let elem = zerodds_dyn_data_get_complex(many, elem_id);
            assert!(!elem.is_null());
            assert_eq!(zerodds_dyn_data_set_complex(d, many_id, many), OK);
            zerodds_dyn_data_free(many);

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0usize;
            assert_eq!(
                zerodds_dyn_data_serialize(d, &raw mut out_ptr, &raw mut out_len),
                OK
            );
            let d2 = zerodds_dyn_data_from_type(outer_ty);
            assert_eq!(zerodds_dyn_data_deserialize(d2, out_ptr, out_len), OK);
            assert_eq!(zerodds_dyn_data_equals(d, d2), 1);

            zerodds_dyn_bytes_free(out_ptr, out_len);
            zerodds_dyn_data_free(elem);
            zerodds_dyn_data_free(d);
            zerodds_dyn_data_free(d2);
            zerodds_dyn_type_free(inner_ty);
            zerodds_dyn_type_free(outer_ty);
        }
    }

    #[test]
    fn clone_and_equals_agree() {
        // SAFETY: see above.
        unsafe {
            let ty = build_string_struct();
            let d = zerodds_dyn_data_from_type(ty);
            let mut id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"data".as_ptr(), 0, &raw mut id),
                OK
            );
            let text = b"clone-me";
            assert_eq!(
                zerodds_dyn_data_set_string(d, id, text.as_ptr(), text.len(), 0),
                OK
            );
            let clone = zerodds_dyn_data_clone(d);
            assert!(!clone.is_null());
            assert_eq!(zerodds_dyn_data_equals(d, clone), 1);

            let ty_clone = zerodds_dyn_type_clone(ty);
            assert!(!ty_clone.is_null());
            assert_eq!(zerodds_dyn_type_equals(ty, ty_clone), 1);

            zerodds_dyn_data_free(d);
            zerodds_dyn_data_free(clone);
            zerodds_dyn_type_free(ty);
            zerodds_dyn_type_free(ty_clone);
        }
    }

    /// A struct with one `int32` member `x`, built through the builder
    /// interface.
    ///
    /// # Safety
    /// As the other tests of this module.
    unsafe fn one_int_type(name: &str) -> *mut ZddsDynType {
        let name = std::ffi::CString::new(name).unwrap();
        // SAFETY: the C interface contract, as above.
        unsafe {
            let builder = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    builder,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    c"x".as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0
                ),
                OK
            );
            zerodds_dyn_builder_finish(builder)
        }
    }

    /// §7.5.2.11.7: one outstanding loan at a time, and a loan returns to
    /// the sample it came from and to no other.
    #[test]
    fn a_second_loan_and_a_foreign_return_are_refused() {
        // SAFETY: see above.
        unsafe {
            let ty = one_int_type("test::LoanOnce");
            let first = zerodds_dyn_data_from_type(ty);
            let second = zerodds_dyn_data_from_type(ty);
            let mut x = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(first, c"x".as_ptr(), 0, &raw mut x),
                OK
            );

            let loan = zerodds_dyn_data_loan(first, x);
            assert!(!loan.is_null());
            // The second loan of the same sample is nil.
            assert!(zerodds_dyn_data_loan(first, x).is_null());
            // The loan does not go back into another sample.
            assert_eq!(zerodds_dyn_data_return_loan(second, loan), INVALID);

            // The refusal did not return the loan: it is still out, and a
            // new loan of the sample is still nil.
            assert!(zerodds_dyn_data_loan(first, x).is_null());

            // A read of the sample is not a loan and is not refused.
            let again = zerodds_dyn_data_get_complex(first, x);
            assert!(!again.is_null());
            zerodds_dyn_data_free(again);

            zerodds_dyn_data_free(first);
            zerodds_dyn_data_free(second);
            zerodds_dyn_type_free(ty);
        }
    }

    /// A `ZddsDynType` over a type the model built with a feature the
    /// ROS interface cannot declare (a key, an optional member, a
    /// bitmask).
    fn boxed_type(ty: DynamicType, name: &str) -> *mut ZddsDynType {
        Box::into_raw(Box::new(ZddsDynType {
            ty,
            original_name: CString::new(name).unwrap(),
            collection_defaults: Arc::new(HashMap::new()),
        }))
    }

    /// §7.5.2.11.3: `clear_nonkey_values` "has exactly the same effect as
    /// clear_all_values with one exception: the values of key fields of
    /// aggregated types retain their values".
    #[test]
    fn clear_nonkey_values_keeps_the_key_members() {
        let mut b = DynamicTypeBuilder::new();
        let long = b.primitive(DynKind::Int32).unwrap();
        let s = b
            .structure("test::Keyed", ExtensibilityKind::Final)
            .unwrap();
        b.add_member(s, &NewMember::new("id", long).keyed())
            .unwrap();
        b.add_member(s, &NewMember::new("x", long)).unwrap();
        let ty = boxed_type(b.finish_as(s).unwrap(), "test::Keyed");
        // SAFETY: see above.
        unsafe {
            let d = zerodds_dyn_data_from_type(ty);
            let (mut id, mut x) = (0u64, 0u64);
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"id".as_ptr(), 0, &raw mut id),
                OK
            );
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"x".as_ptr(), 0, &raw mut x),
                OK
            );
            assert_eq!(zerodds_dyn_data_set_int32(d, id, 5), OK);
            assert_eq!(zerodds_dyn_data_set_int32(d, x, 6), OK);
            assert_eq!(zerodds_dyn_data_clear_nonkey_values(d), OK);
            let (mut got_id, mut got_x) = (-1i32, -1i32);
            assert_eq!(zerodds_dyn_data_get_int32(d, id, &raw mut got_id), OK);
            assert_eq!(zerodds_dyn_data_get_int32(d, x, &raw mut got_x), OK);
            assert_eq!((got_id, got_x), (5, 0), "the key keeps its value");
            assert_eq!(zerodds_dyn_data_clear_all_values(d), OK);
            assert_eq!(zerodds_dyn_data_get_int32(d, id, &raw mut got_id), OK);
            assert_eq!(got_id, 0, "clear_all_values resets the key too");
            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// §7.5.2.11.3: clearing an optional member removes it; clearing an
    /// element of a variable-length collection removes it and the rest
    /// move down.
    #[test]
    fn clear_value_removes_an_optional_member_and_a_sequence_element() {
        let mut b = DynamicTypeBuilder::new();
        let long = b.primitive(DynKind::Int32).unwrap();
        let seq = b.sequence(long, None).unwrap();
        let s = b
            .structure("test::Removing", ExtensibilityKind::Final)
            .unwrap();
        b.add_member(s, &NewMember::new("maybe", long).optional())
            .unwrap();
        b.add_member(s, &NewMember::new("values", seq)).unwrap();
        let ty = boxed_type(b.finish_as(s).unwrap(), "test::Removing");
        // SAFETY: see above.
        unsafe {
            let d = zerodds_dyn_data_from_type(ty);
            let (mut maybe, mut values) = (0u64, 0u64);
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"maybe".as_ptr(), 0, &raw mut maybe),
                OK
            );
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"values".as_ptr(), 0, &raw mut values),
                OK
            );
            // The optional member is absent until it is set, and clearing
            // removes it again.
            assert_eq!(zerodds_dyn_data_item_count(d), 1);
            assert_eq!(zerodds_dyn_data_set_int32(d, maybe, 5), OK);
            assert_eq!(zerodds_dyn_data_item_count(d), 2);
            assert_eq!(zerodds_dyn_data_clear_value(d, maybe), OK);
            assert_eq!(zerodds_dyn_data_item_count(d), 1);
            let mut out = 0i32;
            assert_ne!(zerodds_dyn_data_get_int32(d, maybe, &raw mut out), OK);

            // The sequence loses the element and the next ones move down.
            let child = zerodds_dyn_data_get_complex(d, values);
            assert!(!child.is_null());
            for value in [10, 20, 30] {
                let mut at = 0u64;
                assert_eq!(zerodds_dyn_data_insert_int32(child, value, &raw mut at), OK);
            }
            assert_eq!(zerodds_dyn_data_item_count(child), 3);
            assert_eq!(zerodds_dyn_data_clear_value(child, 1), OK);
            assert_eq!(zerodds_dyn_data_item_count(child), 2);
            assert_eq!(zerodds_dyn_data_get_int32(child, 1, &raw mut out), OK);
            assert_eq!(out, 30);
            assert_ne!(zerodds_dyn_data_clear_value(child, 5), OK);
            zerodds_dyn_data_free(child);
            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// §7.5.2.11.1, §7.5.2.11.3 and §7.5.2.11.6: the flags of a bitmask are
    /// Boolean values reached by bit or by name, clearing one clears the
    /// bit, and the item count is the number of flags that are set.
    #[test]
    fn a_bitmask_flag_is_a_boolean_by_bit() {
        let mut b = DynamicTypeBuilder::new();
        let mask = b.bitmask("test::Flags", Some(8)).unwrap();
        b.add_bit(mask, "READ", None).unwrap();
        b.add_bit(mask, "WRITE", Some(3)).unwrap();
        let s = b
            .structure("test::Holder", ExtensibilityKind::Final)
            .unwrap();
        b.add_member(s, &NewMember::new("flags", mask)).unwrap();
        let ty = boxed_type(b.finish_as(s).unwrap(), "test::Holder");
        // SAFETY: see above.
        unsafe {
            let d = zerodds_dyn_data_from_type(ty);
            let mut flags = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"flags".as_ptr(), 0, &raw mut flags),
                OK
            );
            let mask = zerodds_dyn_data_get_complex(d, flags);
            assert!(!mask.is_null());
            assert_eq!(zerodds_dyn_data_item_count(mask), 0);
            let mut write = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(mask, c"WRITE".as_ptr(), 0, &raw mut write),
                OK
            );
            assert_eq!(write, 3, "the id of a flag is its bit");
            let mut value = 9u8;
            assert_eq!(zerodds_dyn_data_get_bool(mask, write, &raw mut value), OK);
            assert_eq!(value, 0);
            assert_eq!(zerodds_dyn_data_set_bool(mask, write, 1), OK);
            assert_eq!(zerodds_dyn_data_get_bool(mask, write, &raw mut value), OK);
            assert_eq!(value, 1);
            assert_eq!(zerodds_dyn_data_item_count(mask), 1);
            // READ is bit 0 and still clear; bit 2 is no flag.
            assert_eq!(zerodds_dyn_data_get_bool(mask, 0, &raw mut value), OK);
            assert_eq!(value, 0);
            assert_ne!(zerodds_dyn_data_set_bool(mask, 2, 1), OK);
            assert_ne!(zerodds_dyn_data_get_bool(mask, 2, &raw mut value), OK);
            // Clearing the bit clears the flag.
            assert_eq!(zerodds_dyn_data_clear_value(mask, write), OK);
            assert_eq!(zerodds_dyn_data_get_bool(mask, write, &raw mut value), OK);
            assert_eq!(value, 0);
            assert_eq!(zerodds_dyn_data_item_count(mask), 0);
            zerodds_dyn_data_free(mask);
            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// §7.5.2.11.6: an atomic value has an item count of one.
    #[test]
    fn an_atomic_value_has_one_item() {
        // SAFETY: see above.
        unsafe {
            let ty = one_int_type("test::Atomic");
            let d = zerodds_dyn_data_from_type(ty);
            let mut x = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"x".as_ptr(), 0, &raw mut x),
                OK
            );
            assert_eq!(zerodds_dyn_data_item_count(d), 1, "one member");
            let leaf = zerodds_dyn_data_get_complex(d, x);
            assert!(!leaf.is_null());
            assert_eq!(zerodds_dyn_data_item_count(leaf), 1);
            zerodds_dyn_data_free(leaf);
            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// A loan mutates in place and writes back at the id it was loaned
    /// at — the tag [`zerodds_dyn_data_return_loan`] needs because
    /// `dynamic_data_return_loaned_value`'s own C signature carries no
    /// member id (see `rmw_zerodds_dynamic.cpp`'s wrapper). A value that
    /// was never loaned is refused rather than written to slot 0.
    #[test]
    fn a_loan_writes_back_at_its_own_origin() {
        // SAFETY: see above.
        unsafe {
            let inner_name = std::ffi::CString::new("test::LoanInner").unwrap();
            let inner_b = zerodds_dyn_builder_init(inner_name.as_ptr(), 0);
            let x = std::ffi::CString::new("x").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    inner_b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    x.as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0
                ),
                OK
            );
            let inner_ty = zerodds_dyn_builder_finish(inner_b);

            let outer_name = std::ffi::CString::new("test::LoanOuter").unwrap();
            let outer_b = zerodds_dyn_builder_init(outer_name.as_ptr(), 0);
            let field = std::ffi::CString::new("nested").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_nested_member(
                    outer_b,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    inner_ty
                ),
                OK
            );
            let outer_ty = zerodds_dyn_builder_finish(outer_b);
            let d = zerodds_dyn_data_from_type(outer_ty);

            let mut nested_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"nested".as_ptr(), 0, &raw mut nested_id),
                OK
            );

            let loaned = zerodds_dyn_data_loan(d, nested_id);
            assert!(!loaned.is_null());
            let mut inner_x_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(loaned, c"x".as_ptr(), 0, &raw mut inner_x_id),
                OK
            );
            assert_eq!(zerodds_dyn_data_set_int32(loaned, inner_x_id, 99), OK);
            assert_eq!(zerodds_dyn_data_return_loan(d, loaned), OK);

            let after = zerodds_dyn_data_get_complex(d, nested_id);
            assert!(!after.is_null());
            let mut readback = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(after, inner_x_id, &raw mut readback),
                OK
            );
            assert_eq!(readback, 99);

            // A value that was never loaned is refused, not silently
            // written to slot 0.
            let not_loaned = zerodds_dyn_data_get_complex(d, nested_id);
            assert_eq!(zerodds_dyn_data_return_loan(d, not_loaned), INVALID);

            zerodds_dyn_data_free(after);
            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(inner_ty);
            zerodds_dyn_type_free(outer_ty);
        }
    }

    #[test]
    fn a_bound_violation_is_refused_not_truncated() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::Bounded").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            let field = std::ffi::CString::new("s").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::String8 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    4,
                    0,
                    std::ptr::null(),
                    0
                ),
                OK
            );
            let ty = zerodds_dyn_builder_finish(b);
            let d = zerodds_dyn_data_from_type(ty);
            let mut id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"s".as_ptr(), 0, &raw mut id),
                OK
            );
            let too_long = b"too long";
            assert_eq!(
                zerodds_dyn_data_set_string(d, id, too_long.as_ptr(), too_long.len(), 4),
                INVALID
            );
            let ok = b"fits";
            assert_eq!(
                zerodds_dyn_data_set_string(d, id, ok.as_ptr(), ok.len(), 4),
                OK
            );

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// A member built with a `default_value` reads back that default,
    /// not its type's zero, both fresh off `dynamic_data_init_from_
    /// dynamic_type` and after `dynamic_data_clear_value` — the two
    /// paths this module's default-value fix touches. The boolean
    /// member exercises [`ros_default_literal`]'s translation: `"True"`,
    /// Python's `str(bool)` spelling and the one
    /// `rosidl_generator_type_description` actually writes, is not this
    /// crate's `"TRUE"`.
    #[test]
    fn a_defaulted_member_reads_back_its_default_after_init_and_clear() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::Defaulted").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());

            let count = std::ffi::CString::new("count").unwrap();
            let count_default = std::ffi::CString::new("42").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    count.as_ptr(),
                    0,
                    0,
                    0,
                    count_default.as_ptr(),
                    0,
                ),
                OK
            );

            let armed = std::ffi::CString::new("armed").unwrap();
            let armed_default = std::ffi::CString::new("True").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Boolean as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    armed.as_ptr(),
                    0,
                    0,
                    0,
                    armed_default.as_ptr(),
                    0,
                ),
                OK
            );

            let ty = zerodds_dyn_builder_finish(b);
            assert!(!ty.is_null());

            let d = zerodds_dyn_data_from_type(ty);
            assert!(!d.is_null());
            let mut count_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, count.as_ptr(), 0, &raw mut count_id),
                OK
            );
            let mut armed_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, armed.as_ptr(), 0, &raw mut armed_id),
                OK
            );

            let mut count_out = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(d, count_id, &raw mut count_out),
                OK
            );
            assert_eq!(count_out, 42, "the default applies on init, not a zero");

            let mut armed_out = 0u8;
            assert_eq!(
                zerodds_dyn_data_get_bool(d, armed_id, &raw mut armed_out),
                OK
            );
            assert_eq!(armed_out, 1, "\"True\" translates to boolean TRUE");

            // Set away from the default, then clear — `clear_value`
            // consults the member's own `@default`, not the type's.
            assert_eq!(zerodds_dyn_data_set_int32(d, count_id, 7), OK);
            assert_eq!(zerodds_dyn_data_clear_value(d, count_id), OK);
            let mut cleared = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(d, count_id, &raw mut cleared),
                OK
            );
            assert_eq!(cleared, 42, "clear_value restores the member's own default");

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// A `default_value` that does not translate — here, a `Boolean`
    /// text that is neither true nor false — is refused when the member
    /// is added, never silently turned into a zero default.
    #[test]
    fn an_untranslatable_default_is_refused() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::BadDefault").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let field = std::ffi::CString::new("flag").unwrap();
            let bad_default = std::ffi::CString::new("maybe").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Boolean as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    0,
                    bad_default.as_ptr(),
                    0,
                ),
                ERR
            );
            zerodds_dyn_builder_free(b);
        }
    }

    /// A member of each collection shape — a fixed array, a bounded
    /// sequence, an unbounded sequence, each with its own
    /// `default_value` — plus one plain scalar member with none, to
    /// prove `dynamic_data_init_from_dynamic_type`,
    /// `dynamic_data_clear_value` and `dynamic_data_clear_all_values`
    /// all apply a collection member's own recorded default (never
    /// Table 9's empty collection) and leave the plain member's Table 9
    /// zero alone.
    #[test]
    fn a_collection_members_default_is_applied_on_init_and_clear() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::Collections").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());

            let nums = std::ffi::CString::new("nums").unwrap();
            let nums_default = std::ffi::CString::new("(0, 2147483647, -2147483648)").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Array as u32,
                    u64::from(MEMBER_ID_INVALID),
                    nums.as_ptr(),
                    0,
                    0,
                    3,
                    nums_default.as_ptr(),
                    0,
                ),
                OK,
                "int32[3] with a default"
            );

            let flags = std::ffi::CString::new("flags").unwrap();
            let flags_default = std::ffi::CString::new("(True, False)").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Boolean as u32,
                    ZddsDynShapeTag::BoundedSequence as u32,
                    u64::from(MEMBER_ID_INVALID),
                    flags.as_ptr(),
                    0,
                    0,
                    3,
                    flags_default.as_ptr(),
                    0,
                ),
                OK,
                "bool[<=3] with a default"
            );

            let words = std::ffi::CString::new("words").unwrap();
            let words_default = std::ffi::CString::new("('', 'max value', 'min value')").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::String8 as u32,
                    ZddsDynShapeTag::UnboundedSequence as u32,
                    u64::from(MEMBER_ID_INVALID),
                    words.as_ptr(),
                    0,
                    0,
                    0,
                    words_default.as_ptr(),
                    0,
                ),
                OK,
                "string[] with a default"
            );

            let plain = std::ffi::CString::new("plain").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Scalar as u32,
                    u64::from(MEMBER_ID_INVALID),
                    plain.as_ptr(),
                    0,
                    0,
                    0,
                    std::ptr::null(),
                    0,
                ),
                OK,
                "a plain scalar with no default"
            );

            let ty = zerodds_dyn_builder_finish(b);
            assert!(!ty.is_null());

            let d = zerodds_dyn_data_from_type(ty);
            assert!(!d.is_null());

            let mut nums_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, nums.as_ptr(), 0, &raw mut nums_id),
                OK
            );
            let mut flags_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, flags.as_ptr(), 0, &raw mut flags_id),
                OK
            );
            let mut words_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, words.as_ptr(), 0, &raw mut words_id),
                OK
            );
            let mut plain_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, plain.as_ptr(), 0, &raw mut plain_id),
                OK
            );

            // Fresh init: every collection member starts at its own
            // default, never Table 9's empty collection.
            assert_int32_array(d, nums_id, &[0, 2_147_483_647, -2_147_483_648]);
            assert_bool_array(d, flags_id, &[true, false]);
            assert_string_array(d, words_id, &["", "max value", "min value"]);
            let mut plain_out = 0i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(d, plain_id, &raw mut plain_out),
                OK
            );
            assert_eq!(
                plain_out, 0,
                "a plain member still starts at Table 9's zero"
            );

            // Mutate one element of each collection away from its
            // default, then clear that one member — it must come back.
            set_int32_at(d, nums_id, 0, 99);
            assert_eq!(zerodds_dyn_data_clear_value(d, nums_id), OK);
            assert_int32_array(d, nums_id, &[0, 2_147_483_647, -2_147_483_648]);

            set_bool_at(d, flags_id, 0, false);
            assert_eq!(zerodds_dyn_data_clear_value(d, flags_id), OK);
            assert_bool_array(d, flags_id, &[true, false]);

            // Mutate every member, then clear all of them at once.
            set_int32_at(d, nums_id, 1, 1);
            set_bool_at(d, flags_id, 1, true);
            assert_eq!(zerodds_dyn_data_set_int32(d, plain_id, 7), OK);
            assert_eq!(zerodds_dyn_data_clear_all_values(d), OK);
            assert_int32_array(d, nums_id, &[0, 2_147_483_647, -2_147_483_648]);
            assert_bool_array(d, flags_id, &[true, false]);
            assert_string_array(d, words_id, &["", "max value", "min value"]);
            let mut plain_after = -1i32;
            assert_eq!(
                zerodds_dyn_data_get_int32(d, plain_id, &raw mut plain_after),
                OK
            );
            assert_eq!(plain_after, 0);

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// Setters used only by
    /// [`a_collection_members_default_is_applied_on_init_and_clear`]:
    /// mutate one element of a collection member in place (get the
    /// member as a complex child, set an index on it, write it back).
    ///
    /// # Safety
    /// As every other helper in this module: `d` non-NULL and live.
    unsafe fn set_int32_at(d: *mut ZddsDynData, member_id: u64, index: u64, value: i32) {
        // SAFETY: `d` is non-NULL and live (this function's contract); `child`
        // is checked non-NULL before use and freed once, after its last use.
        unsafe {
            let child = zerodds_dyn_data_get_complex(d, member_id);
            assert!(!child.is_null());
            assert_eq!(zerodds_dyn_data_set_int32(child, index, value), OK);
            assert_eq!(zerodds_dyn_data_set_complex(d, member_id, child), OK);
            zerodds_dyn_data_free(child);
        }
    }

    /// As [`set_int32_at`], for a `bool` element.
    ///
    /// # Safety
    /// As [`set_int32_at`].
    unsafe fn set_bool_at(d: *mut ZddsDynData, member_id: u64, index: u64, value: bool) {
        // SAFETY: `d` is non-NULL and live (this function's contract); `child`
        // is checked non-NULL before use and freed once, after its last use.
        unsafe {
            let child = zerodds_dyn_data_get_complex(d, member_id);
            assert!(!child.is_null());
            assert_eq!(zerodds_dyn_data_set_bool(child, index, u8::from(value)), OK);
            assert_eq!(zerodds_dyn_data_set_complex(d, member_id, child), OK);
            zerodds_dyn_data_free(child);
        }
    }

    /// Reads a member's whole `int32` collection back and compares it
    /// against `expected`, element for element and length for length.
    ///
    /// # Safety
    /// As [`set_int32_at`].
    unsafe fn assert_int32_array(d: *const ZddsDynData, member_id: u64, expected: &[i32]) {
        // SAFETY: `d` is non-NULL and live (this function's contract); `child`
        // is checked non-NULL before use and freed once, after its last use.
        unsafe {
            let child = zerodds_dyn_data_get_complex(d, member_id);
            assert!(!child.is_null());
            assert_eq!(zerodds_dyn_data_item_count(child), expected.len() as u64);
            for (index, want) in expected.iter().enumerate() {
                let mut got = 0i32;
                assert_eq!(
                    zerodds_dyn_data_get_int32(child, index as u64, &raw mut got),
                    OK
                );
                assert_eq!(got, *want, "element {index}");
            }
            zerodds_dyn_data_free(child);
        }
    }

    /// As [`assert_int32_array`], for a `bool` collection.
    ///
    /// # Safety
    /// As [`set_int32_at`].
    unsafe fn assert_bool_array(d: *const ZddsDynData, member_id: u64, expected: &[bool]) {
        // SAFETY: `d` is non-NULL and live (this function's contract); `child`
        // is checked non-NULL before use and freed once, after its last use.
        unsafe {
            let child = zerodds_dyn_data_get_complex(d, member_id);
            assert!(!child.is_null());
            assert_eq!(zerodds_dyn_data_item_count(child), expected.len() as u64);
            for (index, want) in expected.iter().enumerate() {
                let mut got = 0u8;
                assert_eq!(
                    zerodds_dyn_data_get_bool(child, index as u64, &raw mut got),
                    OK
                );
                assert_eq!(got != 0, *want, "element {index}");
            }
            zerodds_dyn_data_free(child);
        }
    }

    /// As [`assert_int32_array`], for a `string` collection.
    ///
    /// # Safety
    /// As [`set_int32_at`].
    unsafe fn assert_string_array(d: *const ZddsDynData, member_id: u64, expected: &[&str]) {
        // SAFETY: `d` is non-NULL and live (this function's contract); `child`
        // is checked non-NULL before use and freed once, after its last use.
        unsafe {
            let child = zerodds_dyn_data_get_complex(d, member_id);
            assert!(!child.is_null());
            assert_eq!(zerodds_dyn_data_item_count(child), expected.len() as u64);
            for (index, want) in expected.iter().enumerate() {
                let mut ptr: *const c_char = std::ptr::null();
                let mut len = 0usize;
                assert_eq!(
                    zerodds_dyn_data_get_string(child, index as u64, &raw mut ptr, &raw mut len),
                    OK
                );
                let got = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
                assert_eq!(got, want.as_bytes(), "element {index}");
            }
            zerodds_dyn_data_free(child);
        }
    }

    /// A collection default whose length does not match its array's own
    /// dimension is refused when the member is added, not silently
    /// truncated or padded.
    #[test]
    fn a_collection_default_of_the_wrong_length_is_refused() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::BadArrayDefault").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let field = std::ffi::CString::new("nums").unwrap();
            let too_few = std::ffi::CString::new("(1, 2)").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::Array as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    3,
                    too_few.as_ptr(),
                    0,
                ),
                ERR,
                "2 elements for an int32[3]"
            );
            zerodds_dyn_builder_free(b);
        }
    }

    /// A bounded sequence's default past its own bound is refused the
    /// same way.
    #[test]
    fn a_collection_default_past_its_bound_is_refused() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::BadSeqDefault").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            assert!(!b.is_null());
            let field = std::ffi::CString::new("nums").unwrap();
            let too_many = std::ffi::CString::new("(1, 2, 3, 4)").unwrap();
            assert_eq!(
                zerodds_dyn_builder_add_member(
                    b,
                    ZddsDynKindTag::Int32 as u32,
                    ZddsDynShapeTag::BoundedSequence as u32,
                    u64::from(MEMBER_ID_INVALID),
                    field.as_ptr(),
                    0,
                    0,
                    3,
                    too_many.as_ptr(),
                    0,
                ),
                ERR,
                "4 elements for an int32[<=3]"
            );
            zerodds_dyn_builder_free(b);
        }
    }

    /// The one framing every struct this module builds ever uses is
    /// plain, positional CDR — no DHEADER, no EMHEADER — which is also
    /// what `rmw_zerodds_cxx.cpp`'s introspection-driven codec writes for
    /// the same static message on the shim's non-dynamic path (that codec
    /// walks `rosidl_typesupport_introspection_c__MessageMembers` and
    /// writes each member positionally; there is no XTypes framing
    /// decision anywhere in it). This locks that byte layout in
    /// mechanically for a mixed-alignment struct — `bool` then `int32`
    /// then `float64` — against a by-hand CDR encoding, standing in for
    /// the C static path a Rust unit test cannot call into directly (see
    /// the crate's dynamic-typesupport test script for the one that
    /// compares against a *running* static subscriber).
    #[test]
    fn a_mixed_alignment_struct_matches_plain_cdr_byte_for_byte() {
        // SAFETY: see above.
        unsafe {
            let name = std::ffi::CString::new("test::Mixed").unwrap();
            let b = zerodds_dyn_builder_init(name.as_ptr(), 0);
            for (kind, field) in [
                (ZddsDynKindTag::Boolean, "flag"),
                (ZddsDynKindTag::Int32, "x"),
                (ZddsDynKindTag::Float64, "y"),
            ] {
                let cname = std::ffi::CString::new(field).unwrap();
                assert_eq!(
                    zerodds_dyn_builder_add_member(
                        b,
                        kind as u32,
                        ZddsDynShapeTag::Scalar as u32,
                        u64::from(MEMBER_ID_INVALID),
                        cname.as_ptr(),
                        0,
                        0,
                        0,
                        std::ptr::null(),
                        0
                    ),
                    OK
                );
            }
            let ty = zerodds_dyn_builder_finish(b);
            let d = zerodds_dyn_data_from_type(ty);

            let mut flag_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"flag".as_ptr(), 0, &raw mut flag_id),
                OK
            );
            let mut x_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"x".as_ptr(), 0, &raw mut x_id),
                OK
            );
            let mut y_id = 0u64;
            assert_eq!(
                zerodds_dyn_data_member_id_by_name(d, c"y".as_ptr(), 0, &raw mut y_id),
                OK
            );
            assert_eq!(zerodds_dyn_data_set_bool(d, flag_id, 1), OK);
            assert_eq!(zerodds_dyn_data_set_int32(d, x_id, -12_345), OK);
            assert_eq!(zerodds_dyn_data_set_float64(d, y_id, 6.5), OK);

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0usize;
            assert_eq!(
                zerodds_dyn_data_serialize(d, &raw mut out_ptr, &raw mut out_len),
                OK
            );
            let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            zerodds_dyn_bytes_free(out_ptr, out_len);

            // 4-byte CDR_LE encapsulation header, then the body, aligned
            // relative to the body's *own* start (offset 0 right after
            // the header) — the same origin `rmw_zerodds.c`'s
            // `cdr_align`/`rdr_align` use (`(len - 4) % align`, the `-4`
            // subtracting the header back out): `bool` (1 byte) + 3
            // bytes of padding to a 4-byte boundary, `int32` (4 bytes,
            // body offset 4..8), then `float64` (8 bytes) at body offset
            // 8 — already 8-aligned relative to the body, so no further
            // padding — plain CDR alignment (§7.4.1: align to the
            // primitive's own size, capped at 8 under XCDR1), no XTypes
            // framing.
            let mut expected = Vec::new();
            expected.extend_from_slice(&FINAL_LE_ENCAP); // 4
            expected.push(1u8); // body offset 0 -> 1
            expected.extend_from_slice(&[0u8; 3]); // pad to body offset 4
            expected.extend_from_slice(&(-12_345i32).to_le_bytes()); // -> body offset 8
            expected.extend_from_slice(&6.5f64.to_le_bytes()); // -> body offset 16

            assert_eq!(bytes, expected);

            zerodds_dyn_data_free(d);
            zerodds_dyn_type_free(ty);
        }
    }

    /// [`representation_of`] against the exact octets DDSI-RTPS 2.5
    /// §10.5 / XTypes 1.3 Table 59 assign — the same table
    /// `zerodds_dcps::runtime::user_payload_encap`'s doc comment quotes,
    /// so a change to one is visible against the other.
    #[test]
    fn representation_of_matches_the_rtps_table() {
        assert_eq!(
            representation_of([0x00, 0x00, 0x00, 0x00]),
            Some((Encoding::PlainCdr1, Endianness::Big))
        );
        assert_eq!(
            representation_of([0x00, 0x01, 0x00, 0x00]),
            Some((Encoding::PlainCdr1, Endianness::Little))
        );
        assert_eq!(
            representation_of([0x00, 0x02, 0x00, 0x00]),
            Some((Encoding::PlCdr1, Endianness::Big))
        );
        assert_eq!(
            representation_of([0x00, 0x03, 0x00, 0x00]),
            Some((Encoding::PlCdr1, Endianness::Little))
        );
        assert_eq!(
            representation_of([0x00, 0x06, 0x00, 0x00]),
            Some((Encoding::PlainCdr2, Endianness::Big))
        );
        assert_eq!(
            representation_of([0x00, 0x07, 0x00, 0x00]),
            Some((Encoding::PlainCdr2, Endianness::Little))
        );
        assert_eq!(
            representation_of([0x00, 0x08, 0x00, 0x00]),
            Some((Encoding::DelimitedCdr, Endianness::Big))
        );
        assert_eq!(
            representation_of([0x00, 0x09, 0x00, 0x00]),
            Some((Encoding::DelimitedCdr, Endianness::Little))
        );
        assert_eq!(
            representation_of([0x00, 0x0a, 0x00, 0x00]),
            Some((Encoding::PlCdr2, Endianness::Big))
        );
        assert_eq!(
            representation_of([0x00, 0x0b, 0x00, 0x00]),
            Some((Encoding::PlCdr2, Endianness::Little))
        );
        assert_eq!(representation_of([0x00, 0xff, 0x00, 0x00]), None);
    }
}
