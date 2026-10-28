// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP wire codec for `CosNaming::NamingContext`, `NamingContextExt` and
//! `BindingIterator` — OMG Naming Service 1.3 (`formal/2004-10-03`) §2.4,
//! §2.5.4, §2.5.5, §3.5.1.
//!
//! Pure CDR encode/decode: every function here takes and returns the
//! domain values of [`crate`] (`Name`, [`crate::context::ObjectRef`],
//! [`crate::context::Binding`], …) or the raw [`Ior`] a caller already
//! built for a returned object reference. Building an [`Ior`] for a
//! particular [`crate::context::NamingContext`] — choosing its host,
//! port and object key — needs the naming graph and the daemon's bind
//! address, neither of which this module knows about; that is the
//! servant's job (`zerodds-corba-cosnaming`'s `servant` module, feature
//! `std`).
//!
//! ## Alignment
//!
//! A GIOP 1.2 request/reply body starts 8-aligned from the message's own
//! start (`RequestHeader_1_2`/`ReplyHeader_1_2`, CORBA 3.4 Part 2
//! §15.4.2.2/§15.4.3.1: header + alignment padding to 8, then the body).
//! Every CDR alignment this codec uses is 1, 2, 4 or 8 — each divides 8
//! evenly — so a [`Writer`]/[`Reader`] that treats the body's own first
//! byte as position 0 makes exactly the same padding decisions as one
//! that carried the true stream offset through. `crates/corba-dds-bridge`
//! relies on the same property for its mapped operations; this module
//! follows it for the same reason (CORBA 3.4 Part 2 §15.4.2.2, the
//! "aligned relative to the message header" clause quoted in
//! `bridge_daemon.rs`'s module doc).
//!
//! ## Object references
//!
//! CORBA 3.4 Part 2 §9.3.4 marshals an object reference as its IOR
//! (`string type_id; sequence<TaggedProfile> profiles;`) — there is no
//! separate "object reference" CDR form. [`Ior::encode`]/[`Ior::decode`]
//! is therefore the whole of it.
//!
//! ## Exceptions
//!
//! A raised exception's reply body is `string repository_id` followed by
//! the exception's members, contiguous, aligned from the body start —
//! the same `UserException` shape `zerodds_corba_rust::CorbaException`
//! documents. This module returns `(repository_id, body_bytes)` pairs for
//! that reason rather than depending on `zerodds-corba-rust` (a codegen
//! runtime crate this one has no other reason to pull in).

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ior::Ior;
use zerodds_wire::{Endianness, Reader, Writer};

use crate::context::{Binding, BindingType};
use crate::error::{NamingError, NotFoundReason};
use crate::name::{Name, NameComponent};

/// Repository id of `CosNaming::NamingContext::NotFound`.
pub const REPO_ID_NOT_FOUND: &str = "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0";
/// Repository id of `CosNaming::NamingContext::CannotProceed`.
pub const REPO_ID_CANNOT_PROCEED: &str = "IDL:omg.org/CosNaming/NamingContext/CannotProceed:1.0";
/// Repository id of `CosNaming::NamingContext::InvalidName`.
pub const REPO_ID_INVALID_NAME: &str = "IDL:omg.org/CosNaming/NamingContext/InvalidName:1.0";
/// Repository id of `CosNaming::NamingContext::AlreadyBound`.
pub const REPO_ID_ALREADY_BOUND: &str = "IDL:omg.org/CosNaming/NamingContext/AlreadyBound:1.0";
/// Repository id of `CosNaming::NamingContext::NotEmpty`.
pub const REPO_ID_NOT_EMPTY: &str = "IDL:omg.org/CosNaming/NamingContext/NotEmpty:1.0";
/// Repository id of `CosNaming::NamingContextExt::InvalidAddress`.
pub const REPO_ID_INVALID_ADDRESS: &str =
    "IDL:omg.org/CosNaming/NamingContextExt/InvalidAddress:1.0";

// ---------------------------------------------------------------------
// Name / NameComponent — Spec §2.4.2.
// ---------------------------------------------------------------------

fn write_name(w: &mut Writer, name: &Name) -> Result<(), zerodds_wire::WireError> {
    let n = u32::try_from(name.len()).map_err(|_| zerodds_wire::WireError::OutOfRange {
        at: w.position(),
        expected: alloc::format!("a Name of at most u32::MAX components, not {}", name.len()),
    })?;
    w.write_u32(n);
    for c in name {
        w.write_string(&c.id);
        w.write_string(&c.kind);
    }
    Ok(())
}

fn read_name(r: &mut Reader<'_>) -> Result<Name, zerodds_wire::WireError> {
    let n = r.read_u32()? as usize;
    // Each NameComponent needs >= 8 bytes (two empty-string length
    // prefixes). Reject an impossible count before allocating for it.
    if n > r.remaining() / 8 {
        return Err(zerodds_wire::WireError::ImpossibleLength {
            at: r.position(),
            claimed: n,
            available: r.remaining(),
        });
    }
    let mut out = Name::with_capacity(n);
    for _ in 0..n {
        let id = r.read_string()?;
        let kind = r.read_string()?;
        out.push(NameComponent { id, kind });
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// BindingType / Binding / BindingList — Spec §2.5.4.
// ---------------------------------------------------------------------

fn write_binding_type(w: &mut Writer, t: BindingType) {
    w.write_u32(match t {
        BindingType::Object => 0,  // nobject
        BindingType::Context => 1, // ncontext
    });
}

fn read_binding_type(r: &mut Reader<'_>) -> Result<BindingType, zerodds_wire::WireError> {
    Ok(match r.read_u32()? {
        1 => BindingType::Context,
        _ => BindingType::Object,
    })
}

fn write_binding(w: &mut Writer, b: &Binding) -> Result<(), zerodds_wire::WireError> {
    write_name(w, &alloc::vec![b.binding_name.clone()])?;
    write_binding_type(w, b.binding_type);
    Ok(())
}

fn read_binding(r: &mut Reader<'_>) -> Result<Binding, zerodds_wire::WireError> {
    let mut name = read_name(r)?;
    let binding_name = if name.len() == 1 {
        name.remove(0)
    } else {
        // A `Binding::binding_name` is itself a `Name` in the IDL (spec
        // §2.5.4), always one component in this implementation (a
        // binding lives directly under its parent context); an empty
        // or longer one is a malformed peer.
        return Err(zerodds_wire::WireError::OutOfRange {
            at: r.position(),
            expected: "a Binding.binding_name of exactly one NameComponent".into(),
        });
    };
    let binding_type = read_binding_type(r)?;
    Ok(Binding {
        binding_name,
        binding_type,
    })
}

fn write_binding_list(w: &mut Writer, list: &[Binding]) -> Result<(), zerodds_wire::WireError> {
    let n = u32::try_from(list.len()).map_err(|_| zerodds_wire::WireError::OutOfRange {
        at: w.position(),
        expected: alloc::format!(
            "a BindingList of at most u32::MAX entries, not {}",
            list.len()
        ),
    })?;
    w.write_u32(n);
    for b in list {
        write_binding(w, b)?;
    }
    Ok(())
}

fn read_binding_list(r: &mut Reader<'_>) -> Result<Vec<Binding>, zerodds_wire::WireError> {
    let n = r.read_u32()? as usize;
    if n > r.remaining() / 8 {
        return Err(zerodds_wire::WireError::ImpossibleLength {
            at: r.position(),
            claimed: n,
            available: r.remaining(),
        });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(read_binding(r)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Object references — an IOR, Spec CORBA 3.4 Part 2 §9.3.4.
// ---------------------------------------------------------------------

fn write_object(w: &mut Writer, ior: &Ior) -> Result<(), NamingWireError> {
    // `Ior::encode`'s error is `zerodds_corba_iiop::profile_body::CdrError`
    // — this module stays at `zerodds-corba-ior`'s own dependency depth
    // (it does not otherwise need `corba-iiop`), so the error is kept as
    // its `Debug` text rather than adding that crate just to name the type.
    ior.encode(w)
        .map_err(|e| NamingWireError::Ior(alloc::format!("{e:?}")))
}

fn read_object(r: &mut Reader<'_>) -> Result<Ior, NamingWireError> {
    Ior::decode(r).map_err(|e| NamingWireError::Ior(alloc::format!("{e:?}")))
}

/// Wire error — either the CDR runtime's or the IOR codec's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingWireError {
    /// `zerodds-wire` reader/writer error.
    Wire(zerodds_wire::WireError),
    /// `zerodds-corba-ior`/`zerodds-corba-iiop` encode/decode error
    /// (`Debug`-formatted — see `write_object`).
    Ior(String),
}

impl From<zerodds_wire::WireError> for NamingWireError {
    fn from(e: zerodds_wire::WireError) -> Self {
        Self::Wire(e)
    }
}

impl core::fmt::Display for NamingWireError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "{e:?}"),
            Self::Ior(e) => f.write_str(e),
        }
    }
}

// ---------------------------------------------------------------------
// Exceptions — Spec §2.5.4 (NamingContext) + §3.5.1 (NamingContextExt).
// ---------------------------------------------------------------------

fn write_not_found_reason(w: &mut Writer, why: NotFoundReason) {
    w.write_u32(match why {
        NotFoundReason::MissingNode => 0,
        NotFoundReason::NotContext => 1,
        NotFoundReason::NotObject => 2,
    });
}

fn read_not_found_reason(r: &mut Reader<'_>) -> Result<NotFoundReason, zerodds_wire::WireError> {
    Ok(match r.read_u32()? {
        1 => NotFoundReason::NotContext,
        2 => NotFoundReason::NotObject,
        _ => NotFoundReason::MissingNode,
    })
}

/// Encodes a `NotFound` exception body (`string repo_id; NotFoundReason
/// why; Name rest_of_name;`).
///
/// # Errors
/// [`NamingWireError`] on overflow.
pub fn encode_not_found(
    why: NotFoundReason,
    rest_of_name: &Name,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    w.write_string(REPO_ID_NOT_FOUND);
    write_not_found_reason(&mut w, why);
    write_name(&mut w, rest_of_name)?;
    Ok(w.finish())
}

/// Decodes a `NotFound` exception body (the repository id already
/// consumed by the caller, which is how it picked this decoder).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_not_found_members(
    r: &mut Reader<'_>,
) -> Result<(NotFoundReason, Name), NamingWireError> {
    let why = read_not_found_reason(r)?;
    let rest = read_name(r)?;
    Ok((why, rest))
}

/// Encodes a `CannotProceed` exception body (`string repo_id;
/// NamingContext cxt; Name rest_of_name;`). `cxt` is the IOR of the
/// context the operation could not proceed at — the servant supplies it
/// (this in-memory [`NamingError::CannotProceed`] carries no live
/// reference; see that variant's doc).
///
/// # Errors
/// [`NamingWireError`] on overflow or IOR encode error.
pub fn encode_cannot_proceed(
    cxt: &Ior,
    rest_of_name: &Name,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    w.write_string(REPO_ID_CANNOT_PROCEED);
    write_object(&mut w, cxt)?;
    write_name(&mut w, rest_of_name)?;
    Ok(w.finish())
}

/// Encodes an `InvalidName`/`AlreadyBound`/`NotEmpty`/`InvalidAddress`
/// exception body — all four have no members, `string repo_id` is the
/// whole body.
#[must_use]
pub fn encode_empty_exception(repo_id: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(repo_id);
    w.finish()
}

/// Turns a [`NamingError`] into `(repository_id, member_bytes_after_the_repo_id)`
/// for a reply the caller will still prefix with the repo-id string —
/// used by the servant, which already knows the correct `cxt` IOR for
/// `CannotProceed` (the servant's own object reference, since this
/// implementation never raises it except on an internal lock failure at
/// this same context).
///
/// # Errors
/// [`NamingWireError`] on overflow or IOR encode error.
pub fn encode_naming_error(
    err: &NamingError,
    cxt_for_cannot_proceed: &Ior,
    endianness: Endianness,
) -> Result<(&'static str, Vec<u8>), NamingWireError> {
    Ok(match err {
        NamingError::NotFound { why, rest_of_name } => (
            REPO_ID_NOT_FOUND,
            encode_not_found(*why, rest_of_name, endianness)?,
        ),
        NamingError::CannotProceed { rest_of_name } => (
            REPO_ID_CANNOT_PROCEED,
            encode_cannot_proceed(cxt_for_cannot_proceed, rest_of_name, endianness)?,
        ),
        NamingError::InvalidName => (
            REPO_ID_INVALID_NAME,
            encode_empty_exception(REPO_ID_INVALID_NAME, endianness),
        ),
        NamingError::AlreadyBound => (
            REPO_ID_ALREADY_BOUND,
            encode_empty_exception(REPO_ID_ALREADY_BOUND, endianness),
        ),
        NamingError::NotEmpty => (
            REPO_ID_NOT_EMPTY,
            encode_empty_exception(REPO_ID_NOT_EMPTY, endianness),
        ),
    })
}

// ---------------------------------------------------------------------
// Per-operation request/reply bodies.
// ---------------------------------------------------------------------

/// `bind`/`rebind` request: `(Name n, Object obj)`.
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_bind_request(
    body: &[u8],
    endianness: Endianness,
) -> Result<(Name, Ior), NamingWireError> {
    let mut r = Reader::new(body, endianness);
    let name = read_name(&mut r)?;
    let obj = read_object(&mut r)?;
    Ok((name, obj))
}

/// `bind`/`rebind` request, encoder (client side).
///
/// # Errors
/// [`NamingWireError`] on overflow or IOR encode error.
pub fn encode_bind_request(
    name: &Name,
    obj: &Ior,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    write_name(&mut w, name)?;
    write_object(&mut w, obj)?;
    Ok(w.finish())
}

/// `bind_context`/`rebind_context` request: `(Name n, NamingContext nc)`
/// — same shape as [`decode_bind_request`], named separately because the
/// IDL declares a distinct (narrower) parameter type.
///
/// # Errors
/// As [`decode_bind_request`].
pub fn decode_bind_context_request(
    body: &[u8],
    endianness: Endianness,
) -> Result<(Name, Ior), NamingWireError> {
    decode_bind_request(body, endianness)
}

/// `unbind`/`destroy` (`NamingContext::destroy`) request: `(Name n)` for
/// `unbind`; `destroy` and `new_context` take nothing.
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_name_request(body: &[u8], endianness: Endianness) -> Result<Name, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    Ok(read_name(&mut r)?)
}

/// `unbind`/`resolve` request encoder (client side).
///
/// # Errors
/// [`NamingWireError`] on overflow.
pub fn encode_name_request(
    name: &Name,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    write_name(&mut w, name)?;
    Ok(w.finish())
}

/// `resolve`/`bind_new_context`/`new_context` success reply: `(Object)`.
///
/// # Errors
/// [`NamingWireError`] on IOR encode error.
pub fn encode_object_reply(obj: &Ior, endianness: Endianness) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    write_object(&mut w, obj)?;
    Ok(w.finish())
}

/// `resolve`/`bind_new_context`/`new_context` success reply decoder
/// (client side).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_object_reply(body: &[u8], endianness: Endianness) -> Result<Ior, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    read_object(&mut r)
}

/// `list` request: `(unsigned long how_many)`.
///
/// # Errors
/// [`NamingWireError`] on a truncated body.
pub fn decode_list_request(body: &[u8], endianness: Endianness) -> Result<u32, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    Ok(r.read_u32()?)
}

/// `list` request encoder (client side).
#[must_use]
pub fn encode_list_request(how_many: u32, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_u32(how_many);
    w.finish()
}

/// `list` reply: `(BindingList bl, BindingIterator bi)` — `bi` is `Object
/// | Nil`; a `Nil` object reference is a zero-length `type_id` with no
/// profiles (Spec CORBA 3.4 Part 2 §9.3.4's null-object encoding).
///
/// # Errors
/// [`NamingWireError`] on overflow or IOR encode error.
pub fn encode_list_reply(
    bindings: &[Binding],
    iterator: Option<&Ior>,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    write_binding_list(&mut w, bindings)?;
    match iterator {
        Some(ior) => write_object(&mut w, ior)?,
        None => write_object(&mut w, &nil_object_reference())?,
    }
    Ok(w.finish())
}

/// `list` reply decoder (client side). `None` in the second slot is a nil
/// object reference (every binding fit in `bl`).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_list_reply(
    body: &[u8],
    endianness: Endianness,
) -> Result<(Vec<Binding>, Option<Ior>), NamingWireError> {
    let mut r = Reader::new(body, endianness);
    let bindings = read_binding_list(&mut r)?;
    let iterator = read_object(&mut r)?;
    let iterator = if iterator.is_nil() {
        None
    } else {
        Some(iterator)
    };
    Ok((bindings, iterator))
}

/// A nil object reference — Spec CORBA 3.4 Part 2 §9.3.4: an empty
/// `type_id` and an empty profile sequence ([`Ior::is_nil`]).
#[must_use]
pub fn nil_object_reference() -> Ior {
    Ior::default()
}

/// `BindingIterator::next_one` reply: `(boolean, Binding)`.
///
/// # Errors
/// [`NamingWireError`] on overflow.
pub fn encode_next_one_reply(
    binding: Option<&Binding>,
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    match binding {
        Some(b) => {
            w.write_u8(1);
            write_binding(&mut w, b)?;
        }
        None => {
            w.write_u8(0);
            // A `boolean` `out` is still followed by the `out Binding` slot
            // on the wire even when the return is `false` — the generated
            // stub for a `false` return does not read it, but a
            // spec-conformant peer (and this codec's own round trip) needs
            // a well-formed placeholder to keep later reads aligned. An
            // empty name + `nobject` is unambiguous filler.
            write_binding(
                &mut w,
                &Binding {
                    binding_name: NameComponent::new(String::new()),
                    binding_type: BindingType::Object,
                },
            )?;
        }
    }
    Ok(w.finish())
}

/// `BindingIterator::next_one` reply decoder (client side).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_next_one_reply(
    body: &[u8],
    endianness: Endianness,
) -> Result<Option<Binding>, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    let has_more = r.read_u8()? != 0;
    let binding = read_binding(&mut r)?;
    Ok(if has_more { Some(binding) } else { None })
}

/// `BindingIterator::next_n` request: `(unsigned long how_many)` — same
/// shape as `list`'s.
///
/// # Errors
/// As [`decode_list_request`].
pub fn decode_next_n_request(body: &[u8], endianness: Endianness) -> Result<u32, NamingWireError> {
    decode_list_request(body, endianness)
}

/// `BindingIterator::next_n` reply: `(boolean, BindingList)`.
///
/// # Errors
/// [`NamingWireError`] on overflow.
pub fn encode_next_n_reply(
    bindings: &[Binding],
    endianness: Endianness,
) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    w.write_u8(u8::from(!bindings.is_empty()));
    write_binding_list(&mut w, bindings)?;
    Ok(w.finish())
}

/// `BindingIterator::next_n` reply decoder (client side).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_next_n_reply(
    body: &[u8],
    endianness: Endianness,
) -> Result<Vec<Binding>, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    let _has_more = r.read_u8()?;
    read_binding_list(&mut r).map_err(NamingWireError::from)
}

/// `to_string`/`to_name`/`resolve_str`/`to_url` string-in-string-out
/// bodies (Spec §3.5.1). `to_string` and `resolve_str` take a `Name`
/// or a `StringName` respectively — see the dedicated encode/decode
/// pairs below; this one is the common `(string) -> string` shape of
/// `to_name`.
///
/// # Errors
/// [`NamingWireError`] on a truncated body.
pub fn decode_string_request(
    body: &[u8],
    endianness: Endianness,
) -> Result<String, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    Ok(r.read_string()?)
}

/// Encodes a bare `StringName`/`Address` request (client side: `to_name`,
/// `resolve_str`).
#[must_use]
pub fn encode_string_request(s: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(s);
    w.finish()
}

/// `to_string` reply / `to_name` request-string-reply shape:
/// `(string)`.
#[must_use]
pub fn encode_string_reply(s: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(s);
    w.finish()
}

/// `to_string`/`to_url` reply decoder (client side).
///
/// # Errors
/// [`NamingWireError`] on a truncated body.
pub fn decode_string_reply(body: &[u8], endianness: Endianness) -> Result<String, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    Ok(r.read_string()?)
}

/// `to_name` reply: `(Name)`.
///
/// # Errors
/// [`NamingWireError`] on overflow.
pub fn encode_name_reply(name: &Name, endianness: Endianness) -> Result<Vec<u8>, NamingWireError> {
    let mut w = Writer::new(endianness);
    write_name(&mut w, name)?;
    Ok(w.finish())
}

/// `to_name` reply decoder (client side).
///
/// # Errors
/// [`NamingWireError`] on a truncated/malformed body.
pub fn decode_name_reply(body: &[u8], endianness: Endianness) -> Result<Name, NamingWireError> {
    let mut r = Reader::new(body, endianness);
    Ok(read_name(&mut r)?)
}

/// `to_url` request: `(Address addr, StringName sn)`.
///
/// # Errors
/// [`NamingWireError`] on a truncated body.
pub fn decode_to_url_request(
    body: &[u8],
    endianness: Endianness,
) -> Result<(String, String), NamingWireError> {
    let mut r = Reader::new(body, endianness);
    let address = r.read_string()?;
    let name_part = r.read_string()?;
    Ok((address, name_part))
}

/// `to_url` request encoder (client side).
#[must_use]
pub fn encode_to_url_request(address: &str, name_part: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(address);
    w.write_string(name_part);
    w.finish()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// A minimal `Ior` for round-trip tests: an empty profile list.
    /// `crates/corba-ior`'s own tests cover the profile codec; this
    /// module only needs the `Ior::encode`/`decode` pair's outer shape
    /// (`type_id` + a `sequence<TaggedProfile>`, empty or not) to behave.
    fn ior(object_key: &[u8]) -> Ior {
        Ior {
            type_id: alloc::format!(
                "IDL:omg.org/CosNaming/NamingContext:1.0#{}",
                String::from_utf8_lossy(object_key)
            ),
            profiles: Vec::new(),
        }
    }

    fn name(parts: &[&str]) -> Name {
        parts.iter().map(|p| NameComponent::new(*p)).collect()
    }

    #[test]
    fn name_round_trips_both_orders() {
        let n = name(&["MyApp", "Trader"]);
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = Writer::new(e);
            write_name(&mut w, &n).unwrap();
            let bytes = w.finish();
            let mut r = Reader::new(&bytes, e);
            assert_eq!(read_name(&mut r).unwrap(), n);
        }
    }

    #[test]
    fn bind_request_round_trips() {
        let n = name(&["x"]);
        let target = ior(b"NameService");
        let bytes = encode_bind_request(&n, &target, Endianness::Big).unwrap();
        let (dn, dt) = decode_bind_request(&bytes, Endianness::Big).unwrap();
        assert_eq!(dn, n);
        assert_eq!(dt, target);
    }

    #[test]
    fn resolve_reply_round_trips_an_object() {
        let target = ior(b"some-object");
        let bytes = encode_object_reply(&target, Endianness::Little).unwrap();
        let decoded = decode_object_reply(&bytes, Endianness::Little).unwrap();
        assert_eq!(decoded, target);
    }

    #[test]
    fn list_reply_with_no_iterator_decodes_a_nil_object() {
        let bindings = alloc::vec![Binding {
            binding_name: NameComponent::new("x"),
            binding_type: BindingType::Object,
        }];
        let bytes = encode_list_reply(&bindings, None, Endianness::Big).unwrap();
        let (decoded_bindings, decoded_iter) = decode_list_reply(&bytes, Endianness::Big).unwrap();
        assert_eq!(decoded_bindings.len(), 1);
        assert!(decoded_iter.is_none());
    }

    #[test]
    fn list_reply_with_an_iterator_round_trips_its_ior() {
        let bindings = Vec::new();
        let iter_ior = ior(b"iter-42");
        let bytes = encode_list_reply(&bindings, Some(&iter_ior), Endianness::Little).unwrap();
        let (decoded_bindings, decoded_iter) =
            decode_list_reply(&bytes, Endianness::Little).unwrap();
        assert!(decoded_bindings.is_empty());
        assert_eq!(decoded_iter.unwrap(), iter_ior);
    }

    #[test]
    fn next_one_true_round_trips_a_binding() {
        let b = Binding {
            binding_name: NameComponent::with_kind("a", "k"),
            binding_type: BindingType::Context,
        };
        let bytes = encode_next_one_reply(Some(&b), Endianness::Big).unwrap();
        let decoded = decode_next_one_reply(&bytes, Endianness::Big).unwrap();
        assert_eq!(decoded, Some(b));
    }

    #[test]
    fn next_one_false_decodes_to_none() {
        let bytes = encode_next_one_reply(None, Endianness::Little).unwrap();
        let decoded = decode_next_one_reply(&bytes, Endianness::Little).unwrap();
        assert_eq!(decoded, None);
    }

    #[test]
    fn next_n_round_trips_several_bindings() {
        let bindings: Vec<Binding> = (0..4)
            .map(|i| Binding {
                binding_name: NameComponent::new(alloc::format!("o{i}")),
                binding_type: BindingType::Object,
            })
            .collect();
        let bytes = encode_next_n_reply(&bindings, Endianness::Big).unwrap();
        let decoded = decode_next_n_reply(&bytes, Endianness::Big).unwrap();
        assert_eq!(decoded, bindings);
    }

    #[test]
    fn next_n_empty_still_decodes() {
        let bytes = encode_next_n_reply(&[], Endianness::Little).unwrap();
        let decoded = decode_next_n_reply(&bytes, Endianness::Little).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn not_found_exception_round_trips_reason_and_rest() {
        let rest = name(&["b", "c"]);
        let bytes = encode_not_found(NotFoundReason::NotContext, &rest, Endianness::Big).unwrap();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let repo_id = r.read_string().unwrap();
        assert_eq!(repo_id, REPO_ID_NOT_FOUND);
        let (why, decoded_rest) = decode_not_found_members(&mut r).unwrap();
        assert_eq!(why, NotFoundReason::NotContext);
        assert_eq!(decoded_rest, rest);
    }

    #[test]
    fn cannot_proceed_carries_the_servant_ior_and_the_rest_of_name() {
        let cxt = ior(b"NameService");
        let rest = name(&["missing"]);
        let bytes = encode_cannot_proceed(&cxt, &rest, Endianness::Little).unwrap();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), REPO_ID_CANNOT_PROCEED);
        let decoded_cxt = read_object(&mut r).unwrap();
        assert_eq!(decoded_cxt, cxt);
        assert_eq!(read_name(&mut r).unwrap(), rest);
    }

    #[test]
    fn empty_exceptions_carry_only_the_repository_id() {
        for repo_id in [
            REPO_ID_INVALID_NAME,
            REPO_ID_ALREADY_BOUND,
            REPO_ID_NOT_EMPTY,
            REPO_ID_INVALID_ADDRESS,
        ] {
            let bytes = encode_empty_exception(repo_id, Endianness::Big);
            let mut r = Reader::new(&bytes, Endianness::Big);
            assert_eq!(r.read_string().unwrap(), repo_id);
        }
    }

    #[test]
    fn encode_naming_error_dispatches_to_the_right_repo_id() {
        let cxt = ior(b"root");
        for (err, expected) in [
            (
                NamingError::NotFound {
                    why: NotFoundReason::MissingNode,
                    rest_of_name: Name::new(),
                },
                REPO_ID_NOT_FOUND,
            ),
            (
                NamingError::CannotProceed {
                    rest_of_name: Name::new(),
                },
                REPO_ID_CANNOT_PROCEED,
            ),
            (NamingError::InvalidName, REPO_ID_INVALID_NAME),
            (NamingError::AlreadyBound, REPO_ID_ALREADY_BOUND),
            (NamingError::NotEmpty, REPO_ID_NOT_EMPTY),
        ] {
            let (repo_id, _) = encode_naming_error(&err, &cxt, Endianness::Big).unwrap();
            assert_eq!(repo_id, expected);
        }
    }

    #[test]
    fn string_and_to_url_bodies_round_trip() {
        let bytes = encode_string_request("App/Service", Endianness::Big);
        assert_eq!(
            decode_string_request(&bytes, Endianness::Big).unwrap(),
            "App/Service"
        );
        let bytes = encode_to_url_request(":host:2809/NS", "App/Service", Endianness::Little);
        let (addr, np) = decode_to_url_request(&bytes, Endianness::Little).unwrap();
        assert_eq!(addr, ":host:2809/NS");
        assert_eq!(np, "App/Service");
    }

    #[test]
    fn to_name_reply_round_trips_a_name() {
        let n = name(&["a", "b"]);
        let bytes = encode_name_reply(&n, Endianness::Big).unwrap();
        assert_eq!(decode_name_reply(&bytes, Endianness::Big).unwrap(), n);
    }

    #[test]
    fn oversized_name_count_is_rejected_before_allocating() {
        let mut w = Writer::new(Endianness::Little);
        w.write_u32(1_000_000_000);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert!(matches!(
            read_name(&mut r),
            Err(zerodds_wire::WireError::ImpossibleLength { .. })
        ));
    }
}
