// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Dynamic types for `dds::core::xtypes` (DDS-PSM-Cxx 1.0 §7.5, DDS-XTypes 1.3 §7.5).
//!
//! A type arrives as IDL text, which the C++ side writes from its `DynamicType`. It becomes a
//! model with the front end every other column uses, so the TypeObject, the key, the
//! extensibility and the wire framing are the ones a compiled type of the same IDL has. A value
//! crosses the boundary as JSON in the dialect of `zerodds-wire-probe` (a 64-bit integer, a
//! `long double` and a `fixed` are strings, an `enum` its enumerator's name, a `map` an array of
//! pairs, a `union` an object with `_d`), and `zerodds-wire-codec` turns it into the octets of the
//! chosen encoding and back.
//!
//! The octets are the body of a sample, without the encapsulation header.

use core::ffi::{c_char, c_int};
use std::collections::BTreeSet;
use std::ffi::{CStr, CString};

use zerodds_ast::{Ast, Extensibility, TypeKind, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_wire_codec::ProbeRequest;

use crate::ZeroDdsStatus;

/// A dynamic type: the model the IDL lowered to, and what a typed endpoint needs of it.
pub struct ZeroDdsDynType {
    ast: Ast,
    id: Id<TypeNode>,
    name: String,
    keyed: bool,
    extensibility: c_int,
    type_object: Vec<u8>,
    dependencies: Vec<Vec<u8>>,
}

fn standard_dependencies(ast: &Ast, id: Id<TypeNode>) -> Result<Vec<Vec<u8>>, String> {
    use zerodds_types::resolve::{
        DEFAULT_MAX_RESOLVE_DEPTH, DEFAULT_MAX_RESOLVE_NODES, TypeRegistry,
        collect_referenced_hashes,
    };
    use zerodds_types::type_object::TypeObject;
    let mut registry = TypeRegistry::new();
    let _unrepresented = zerodds_types::from_ast::register_all(ast, &mut registry)
        .map_err(|e| format!("type registry: {e:?}"))?;
    let complete = zerodds_types::from_ast::complete_type_object(ast, id)
        .map_err(|e| format!("complete object: {e:?}"))?;
    let root = TypeObject::Complete(complete);
    let root_hash =
        zerodds_types::hash::compute_hash(&root).map_err(|e| format!("root hash: {e:?}"))?;
    let mut seen = BTreeSet::from([root_hash]);
    let mut pending = collect_referenced_hashes(&root, DEFAULT_MAX_RESOLVE_DEPTH)
        .map_err(|e| format!("root dependencies: {e:?}"))?;
    let mut out = Vec::new();
    while let Some(hash) = pending.pop() {
        if !seen.insert(hash) {
            continue;
        }
        if seen.len() > DEFAULT_MAX_RESOLVE_NODES {
            return Err("type dependency closure exceeds node limit".into());
        }
        let object = registry
            .get_complete(&hash)
            .ok_or_else(|| format!("missing complete dependency {hash:?}"))?;
        let wrapped = TypeObject::Complete(object.clone());
        pending.extend(
            collect_referenced_hashes(&wrapped, DEFAULT_MAX_RESOLVE_DEPTH)
                .map_err(|e| format!("transitive dependencies: {e:?}"))?,
        );
        out.push(
            wrapped
                .to_bytes_le()
                .map_err(|e| format!("dependency encode: {e:?}"))?,
        );
    }
    Ok(out)
}

fn build(idl: &str, type_name: &str) -> Result<ZeroDdsDynType, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if parsed.has_errors() {
        return Err("the IDL does not parse".to_string());
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dynamic.idl");
    if !lowered.diagnostics.is_empty() {
        return Err(format!("the IDL does not lower: {:?}", lowered.diagnostics));
    }
    let mut ast = lowered.ast;
    // Names that stand in another module statement of the same text (`::m::Label`) are
    // resolved here; without it the type object cannot name them.
    ast.resolve_all_by_name()
        .map_err(|e| format!("the IDL names a type it does not declare: {e:?}"))?;
    let segments: Vec<String> = type_name
        .split("::")
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("no type `{type_name}` in the IDL"))?;
    let node = ast.type_(id).map_err(|e| format!("{e:?}"))?;
    if !matches!(
        node.kind,
        TypeKind::Structure { .. } | TypeKind::Union { .. }
    ) {
        return Err(format!("`{type_name}` is not a structure or a union"));
    }
    let extensibility = match ast.effective_extensibility(id) {
        Extensibility::Final => 0,
        Extensibility::Appendable => 1,
        Extensibility::Mutable => 2,
    };
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Big);
    let keyed = zerodds_ast_layout::keyhash::key_members_with(
        &ast,
        id,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .is_ok_and(|members| !members.is_empty());
    let complete = zerodds_types::from_ast::complete_type_object(&ast, id)
        .map_err(|e| format!("complete object: {e:?}"))?;
    let type_object = zerodds_types::type_object::TypeObject::Complete(complete)
        .to_bytes_le()
        .map_err(|e| format!("TypeObject encode: {e:?}"))?;
    let dependencies = standard_dependencies(&ast, id)?;
    Ok(ZeroDdsDynType {
        ast,
        id,
        name: type_name.trim_start_matches("::").to_string(),
        keyed,
        extensibility,
        type_object,
        dependencies,
    })
}

fn text<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    // SAFETY: NUL-terminated per the callers' contract.
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

fn owned_text(s: &str, out: *mut *mut c_char) -> c_int {
    match CString::new(s) {
        Ok(c) => {
            // SAFETY: `out` is writable per the callers' contract.
            unsafe { *out = c.into_raw() };
            ZeroDdsStatus::Ok as c_int
        }
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Builds a dynamic type from IDL text: `type_name` (`"a::b::T"`) names the structure or union in
/// `idl`. Returns NULL and sets `*out_status` (if not NULL) otherwise: `ZERODDS_BAD_PARAMETER` for
/// IDL that does not parse or lower, a name it does not declare, or a type that is no structure
/// or union.
///
/// # Safety
/// `idl` and `type_name` are NUL-terminated strings; `out_status` is writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_from_idl(
    idl: *const c_char,
    type_name: *const c_char,
    out_status: *mut c_int,
) -> *mut ZeroDdsDynType {
    let set = |status: ZeroDdsStatus| {
        if !out_status.is_null() {
            // SAFETY: writable per the caller's contract.
            unsafe { *out_status = status as c_int };
        }
    };
    let (Some(idl), Some(type_name)) = (text(idl), text(type_name)) else {
        set(ZeroDdsStatus::BadParameter);
        return core::ptr::null_mut();
    };
    match build(idl, type_name) {
        Ok(t) => {
            set(ZeroDdsStatus::Ok);
            Box::into_raw(Box::new(t))
        }
        Err(_) => {
            set(ZeroDdsStatus::BadParameter);
            core::ptr::null_mut()
        }
    }
}

/// Destroys a dynamic type.
///
/// # Safety
/// `t` is NULL or from [`zerodds_dyntype_from_idl`], not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_destroy(t: *mut ZeroDdsDynType) {
    if !t.is_null() {
        // SAFETY: from `Box::into_raw` per the contract.
        drop(unsafe { Box::from_raw(t) });
    }
}

/// The type's scoped name without the leading `::`, which is the topic type name peers match by.
/// Free it with `zerodds_string_free`.
///
/// # Safety
/// `t` is a live dynamic type; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_name(
    t: *const ZeroDdsDynType,
    out: *mut *mut c_char,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: live per the contract.
    owned_text(unsafe { &(*t).name }, out)
}

/// Whether the type is keyed (XTypes 1.3 §7.6.8): 1 or 0, a negative status for a bad handle.
///
/// # Safety
/// `t` is a live dynamic type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_is_keyed(t: *const ZeroDdsDynType) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: live per the contract.
    c_int::from(unsafe { (*t).keyed })
}

/// The extensibility: 0 FINAL, 1 APPENDABLE, 2 MUTABLE; a negative status for a bad handle.
///
/// # Safety
/// `t` is a live dynamic type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_extensibility(t: *const ZeroDdsDynType) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: live per the contract.
    unsafe { (*t).extensibility }
}

/// The `CompleteTypeObject` of the type, XCDR2-LE (XTypes 1.3 §7.3.4). The pointer is valid
/// until the type is destroyed.
///
/// # Safety
/// `t` is a live dynamic type; `len` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_type_object(
    t: *const ZeroDdsDynType,
    len: *mut usize,
) -> *const u8 {
    if t.is_null() || len.is_null() {
        return core::ptr::null();
    }
    // SAFETY: live per the contract.
    unsafe {
        *len = (*t).type_object.len();
        (*t).type_object.as_ptr()
    }
}

/// How many complete TypeObjects the type's own refers to, transitively.
///
/// # Safety
/// `t` is a live dynamic type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_dependency_count(t: *const ZeroDdsDynType) -> usize {
    if t.is_null() {
        return 0;
    }
    // SAFETY: live per the contract.
    unsafe { (*t).dependencies.len() }
}

/// Dependency `i` (see [`zerodds_dyntype_dependency_count`]); NULL beyond the count. Valid until
/// the type is destroyed.
///
/// # Safety
/// `t` is a live dynamic type; `len` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_dependency(
    t: *const ZeroDdsDynType,
    i: usize,
    len: *mut usize,
) -> *const u8 {
    if t.is_null() || len.is_null() {
        return core::ptr::null();
    }
    // SAFETY: live per the contract.
    let t = unsafe { &*t };
    match t.dependencies.get(i) {
        Some(bytes) => {
            // SAFETY: `len` is writable per the contract.
            unsafe { *len = bytes.len() };
            bytes.as_ptr()
        }
        None => core::ptr::null(),
    }
}

/// The default value of the type (XTypes 1.3 Table 9) as JSON. Free with `zerodds_string_free`.
///
/// # Safety
/// `t` is a live dynamic type; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_default_json(
    t: *const ZeroDdsDynType,
    out: *mut *mut c_char,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: live per the contract.
    let t = unsafe { &*t };
    match zerodds_wire_codec::default_of_type::<serde_json::Value>(&t.ast, t.id, &t.name) {
        Ok(value) => owned_text(&value.to_string(), out),
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

fn request(t: &ZeroDdsDynType, xcdr2: bool, big_endian: bool) -> ProbeRequest {
    ProbeRequest::new(
        &t.name,
        if xcdr2 {
            Encoding::PlainCdr2
        } else {
            Encoding::PlainCdr1
        },
        if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        },
    )
}

/// Writes a value, given as JSON, as the body of a sample: XCDR1 or XCDR2 (`xcdr2` non-zero),
/// little- or big-endian. The octets go to `*out`/`*len`; release them with
/// [`zerodds_dyntype_bytes_free`]. `ZERODDS_BAD_PARAMETER` when the JSON is not a value of the
/// type — a missing member, an unknown one, a number out of range.
///
/// # Safety
/// `t` is a live dynamic type; `json` is NUL-terminated; `out` and `len` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_encode_json(
    t: *const ZeroDdsDynType,
    json: *const c_char,
    xcdr2: c_int,
    big_endian: c_int,
    out: *mut *mut u8,
    len: *mut usize,
) -> c_int {
    if t.is_null() || out.is_null() || len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let Some(json) = text(json) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: live per the contract.
    let t = unsafe { &*t };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    match zerodds_wire_codec::encode(&t.ast, &request(t, xcdr2 != 0, big_endian != 0), &value) {
        Ok(bytes) => {
            let boxed = bytes.into_boxed_slice();
            // SAFETY: writable per the contract.
            unsafe {
                *len = boxed.len();
                *out = Box::into_raw(boxed).cast::<u8>();
            }
            ZeroDdsStatus::Ok as c_int
        }
        Err(_) => ZeroDdsStatus::BadParameter as c_int,
    }
}

/// Releases octets from [`zerodds_dyntype_encode_json`].
///
/// # Safety
/// `(ptr, len)` come from that function and are not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_bytes_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: a boxed slice of this length, per the contract.
        drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

/// Reads the body of a sample as a value of the type and returns it as JSON (free with
/// `zerodds_string_free`). `ZERODDS_BAD_PARAMETER` when the octets are not a value of the type.
///
/// # Safety
/// `t` is a live dynamic type; `bytes[0..len]` is readable; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dyntype_decode_json(
    t: *const ZeroDdsDynType,
    bytes: *const u8,
    len: usize,
    xcdr2: c_int,
    big_endian: c_int,
    out: *mut *mut c_char,
) -> c_int {
    if t.is_null() || out.is_null() || (bytes.is_null() && len != 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: live and readable per the contract.
    let (t, slice) = unsafe {
        (
            &*t,
            if len == 0 {
                &[][..]
            } else {
                core::slice::from_raw_parts(bytes, len)
            },
        )
    };
    match zerodds_wire_codec::decode::<serde_json::Value>(
        &t.ast,
        &request(t, xcdr2 != 0, big_endian != 0),
        slice,
    ) {
        Ok(value) => owned_text(&value.to_string(), out),
        Err(_) => ZeroDdsStatus::BadParameter as c_int,
    }
}
