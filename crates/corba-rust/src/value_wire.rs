// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA `valuetype` wire (§15.3.4) — the self-describing, **shared**
//! object encoding. A value instance is written as `value_tag` + RepositoryId +
//! state members; the same instance (Rc identity) a second time as an
//! **indirection** (value sharing, §15.3.4) — so object graphs with
//! multiple references are preserved.
//!
//! ## value_tag (§15.3.4.1) — bit layout
//!
//! Verified against JacORB `CDRInputStream` (the reference implementation
//! against which cross-ORB testing is done):
//! * `0x00000000` — null value.
//! * `0xffffffff` — indirection, followed by a `long` offset relative to
//!   the position of the offset field (backwards to the target `value_tag`).
//! * `0x7fffff00 .. 0x7fffffff` — a value follows; flags in the low byte:
//!   * Bit 0 (`0x01`) — codebase URL present.
//!   * Bit 3 (`0x08`) — chunked encoding.
//!   * Bits 1-2 (`0x06`) — type info: `0x00` none / `0x02` single RepositoryId /
//!     `0x06` list of RepositoryIds.
//!
//! **Scope of this engine**: non-chunked + chunked encoding, single/list
//! RepositoryId, value sharing (DAG), truncation, **nested chunked
//! values** + multi-level end-tags, and **codebase URL resolution** via a
//! [`CodebaseResolver`].

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use core::any::Any;

use zerodds_wire::{Reader, WireError, Writer};

use crate::runtime::ValueBase;

/// `value_tag` base (§15.3.4.1).
const VALUE_TAG_BASE: u32 = 0x7fff_ff00;
/// Bit 0: codebase URL present.
const VT_FLAG_CODEBASE: u32 = 0x0000_0001;
/// Bit 3: chunked encoding.
const VT_FLAG_CHUNKED: u32 = 0x0000_0008;
/// Bits 1-2: RepositoryId type info.
const VT_REPO_MASK: u32 = 0x0000_0006;
const VT_REPO_NONE: u32 = 0x0000_0000;
const VT_REPO_SINGLE: u32 = 0x0000_0002;
const VT_REPO_LIST: u32 = 0x0000_0006;
/// null value.
const VALUE_NULL: u32 = 0x0000_0000;
/// Indirection marker.
const VALUE_INDIRECTION: u32 = 0xffff_ffff;

/// A refusal from the value layer, at the offset it was decided.
///
/// `zerodds-cdr` had `EncodeError::ValueOutOfRange` and
/// `DecodeError::InvalidEnum` with a value of zero standing in for
/// everything this file can refuse; `zerodds-wire` has one error type
/// and every variant of it names an offset. So both take one.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `WireError::refused`, an inherent constructor on another type;
/// `dds_bounded_recursion` compares only the last segment of a call
/// path, so the delegation reads as a self-call.
fn refused(at: usize, what: &'static str) -> WireError {
    WireError::refused(at, what)
}

/// A concrete valuetype that marshals over the §15.3.4 wire. Generated
/// code implements this in addition to [`ValueBase`].
pub trait ValueMarshal: ValueBase {
    /// Writes ONLY the state members (declaration order), without `value_tag`.
    ///
    /// # Errors
    /// CDR encode error.
    fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError>;
}

/// Writes valuetype instances with **value sharing**: the same `Rc` instance is
/// encoded as an indirection on the second write (one object, not two copies).
#[derive(Default)]
pub struct ValueWriter {
    /// `Rc` pointer identity → stream position of the corresponding `value_tag`.
    seen: BTreeMap<usize, usize>,
}

impl ValueWriter {
    /// Fresh writer (empty sharing table).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes an optional valuetype: `None` → null; known instance →
    /// indirection; otherwise `value_tag`(single RepositoryId) + RepositoryId + state.
    ///
    /// # Errors
    /// CDR encode error / offset overflow.
    pub fn write(
        &mut self,
        w: &mut Writer,
        value: Option<&Rc<dyn ValueMarshal>>,
    ) -> Result<(), WireError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL);
            return Ok(());
        };
        w.align_to(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            // Value sharing: indirection backwards to the first value_tag.
            w.write_u32(VALUE_INDIRECTION);
            // Offset relative to the position of the offset field (now = tag_pos + 4).
            let offset = i32::try_from(prior as i64 - w.position() as i64).map_err(|_| {
                refused(w.position(), "a value indirection offset that fits a long")
            })?;
            w.write_u32(offset as u32);
            return Ok(());
        }
        self.seen.insert(id, tag_pos);
        w.write_u32(VALUE_TAG_BASE | VT_REPO_SINGLE); // 0x7fffff02
        w.write_string(v.repository_id());
        v.marshal_state(w)
    }

    /// Like [`write`](Self::write), but with a **codebase URL** (§15.3.4.1, bit 0):
    /// `value_tag` 0x7fffff03, then the codebase URL, then RepositoryId + state.
    /// The reader can use the URL via a [`CodebaseResolver`] for factory
    /// resolution.
    ///
    /// # Errors
    /// CDR encode error / offset overflow.
    pub fn write_with_codebase(
        &mut self,
        w: &mut Writer,
        value: Option<&Rc<dyn ValueMarshal>>,
        codebase: &str,
    ) -> Result<(), WireError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL);
            return Ok(());
        };
        w.align_to(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION);
            let offset = i32::try_from(prior as i64 - w.position() as i64).map_err(|_| {
                refused(w.position(), "a value indirection offset that fits a long")
            })?;
            w.write_u32(offset as u32);
            return Ok(());
        }
        self.seen.insert(id, tag_pos);
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CODEBASE | VT_REPO_SINGLE); // 0x7fffff03
        w.write_string(codebase);
        w.write_string(v.repository_id());
        v.marshal_state(w)
    }

    /// Writes a valuetype **chunked** with a RepositoryId list (§15.3.4.3) —
    /// the format that makes truncatable valuetypes interoperable over the wire
    /// (a foreign ORB can truncate to a base). `base_ids` are the base
    /// RepositoryIds (most-derived → base order); the state lies in one
    /// chunk, terminated with end-tag `-1`. Byte-identical to JacORB 3.9.
    ///
    /// # Errors
    /// CDR encode error / offset or length overflow.
    pub fn write_chunked(
        &mut self,
        w: &mut Writer,
        value: Option<&Rc<dyn ValueMarshal>>,
        base_ids: &[&str],
    ) -> Result<(), WireError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL);
            return Ok(());
        };
        w.align_to(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION);
            let offset = i32::try_from(prior as i64 - w.position() as i64).map_err(|_| {
                refused(w.position(), "a value indirection offset that fits a long")
            })?;
            w.write_u32(offset as u32);
            return Ok(());
        }
        self.seen.insert(id, tag_pos);

        // value_tag: chunked + RepositoryId list (0x7fffff0e).
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST);
        let count = u32::try_from(1 + base_ids.len()).map_err(|_| {
            refused(
                w.position(),
                "a repository id list shorter than 2^32 entries",
            )
        })?;
        w.write_u32(count);
        w.write_string(v.repository_id());
        for b in base_ids {
            w.write_string(b);
        }

        // The chunk's size is a placeholder patched once the state is
        // written, and the state goes into `w` — not into a second
        // buffer told that its byte 0 is byte `state_offset`. That is
        // what this did, and an alignment counted from a coordinate a
        // buffer was given rather than one it has is a coordinate that
        // can be given wrongly. `write_dheader` reserves the same four
        // octets after the same `align_to(4)`, `finish_dheader` writes
        // the same length, and every alignment inside `marshal_state`
        // is counted from the offset the octets actually have.
        let chunk = w.write_dheader();
        v.marshal_state(w)?;
        w.finish_dheader(chunk);

        // End-tag -1 (outermost value).
        w.align_to(4);
        w.write_u32((-1i32) as u32);
        Ok(())
    }

    /// Writes a **multi-chunk** valuetype tree (§15.3.4.3): a chunked value
    /// whose body carries further chunked values at chunk boundaries, each
    /// self-delimited by its own end-tag `-level`. This is the encoder
    /// counterpart to the decoder's nested-chunk handling: where
    /// [`write_chunked`](Self::write_chunked) emits a single flat state chunk
    /// plus an end-tag, this emits the leading state chunk followed by the
    /// node's nested values — the wire a base-only reader truncates over
    /// (consuming the derived/nested tail). A leaf node (no nested children)
    /// is byte-identical to [`write_chunked`](Self::write_chunked).
    ///
    /// # Errors
    /// CDR encode error / offset or length overflow.
    pub fn write_chunked_tree(
        &mut self,
        w: &mut Writer,
        node: &ChunkedNode<'_>,
    ) -> Result<(), WireError> {
        self.write_chunked_node(w, node, 1)
    }

    /// Recursive worker for [`write_chunked_tree`](Self::write_chunked_tree).
    /// `level` is the 1-based nesting depth used for this value's end-tag.
    ///
    /// zerodds-lint: recursion-depth 64 (Valuetype-Chunk-Nesting; bounded by GIOP value chunking)
    fn write_chunked_node(
        &mut self,
        w: &mut Writer,
        node: &ChunkedNode<'_>,
        level: u32,
    ) -> Result<(), WireError> {
        w.align_to(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(node.value).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION);
            let offset = i32::try_from(prior as i64 - w.position() as i64).map_err(|_| {
                refused(w.position(), "a value indirection offset that fits a long")
            })?;
            w.write_u32(offset as u32);
            return Ok(());
        }
        self.seen.insert(id, tag_pos);

        // value_tag: chunked + RepositoryId list.
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST);
        let count = u32::try_from(1 + node.base_ids.len()).map_err(|_| {
            refused(
                w.position(),
                "a repository id list shorter than 2^32 entries",
            )
        })?;
        w.write_u32(count);
        w.write_string(node.value.repository_id());
        for b in node.base_ids {
            w.write_string(b);
        }

        // Leading data chunk: the node's own state, written into the
        // stream and sized afterwards. See `write_chunked`.
        let chunk = w.write_dheader();
        node.value.marshal_state(w)?;
        w.finish_dheader(chunk);

        // Nested chunked values follow at chunk boundaries (derived tail),
        // each self-delimited by its own end-tag at the deeper level.
        for child in node.nested {
            self.write_chunked_node(w, child, level + 1)?;
        }

        // This value's own end-tag (-level).
        w.align_to(4);
        w.write_u32((-(level as i32)) as u32);
        Ok(())
    }
}

/// A node in a chunked valuetype tree (§15.3.4.3) for **multi-chunk** encoding
/// via [`ValueWriter::write_chunked_tree`]: a value, its base RepositoryIds
/// (most-derived → base, written in the `value_tag` list), and nested chunked
/// values appended after its state at chunk boundaries. Lets a chunked value
/// carry further chunked values (e.g. a truncatable derived tail) instead of a
/// single flat state chunk.
pub struct ChunkedNode<'a> {
    /// The value whose leading state forms the first data chunk.
    pub value: &'a Rc<dyn ValueMarshal>,
    /// Base RepositoryIds (most-derived → base) for the `value_tag` list.
    pub base_ids: &'a [&'a str],
    /// Nested chunked values, appended after the state at chunk boundaries.
    pub nested: &'a [ChunkedNode<'a>],
}

/// Factory closure: reads the state members of a valuetype and returns a
/// typed instance as `Rc<dyn Any>` (the caller downcasts to the concrete type).
pub type ValueCtor = Box<dyn Fn(&mut Reader<'_>) -> Result<Rc<dyn Any>, WireError>>;

/// Codebase resolver: returns — based on the codebase URL carried over the
/// wire (§15.3.4.1, value_tag bit 0) and the RepositoryId — a
/// factory for a valuetype whose factory is not statically registered.
/// This way the codebase URL is **taken into account in factory resolution**
/// (instead of just being skipped) — the Rust counterpart to the CORBA codebase download.
pub type CodebaseResolver = Box<dyn Fn(&str, &str) -> Option<ValueCtor>>;

/// A resolved factory: either borrowed from the static registry or
/// freshly supplied (owned) by the [`CodebaseResolver`].
enum CtorRef<'a> {
    Borrowed(&'a ValueCtor),
    Owned(ValueCtor),
}

impl CtorRef<'_> {
    fn call(&self, r: &mut Reader<'_>) -> Result<Rc<dyn Any>, WireError> {
        match self {
            CtorRef::Borrowed(c) => c(r),
            CtorRef::Owned(c) => c(r),
        }
    }
}

/// RepositoryId → state-reader factory. Generated code registers a factory per
/// valuetype; an optional [`CodebaseResolver`] resolves unknown RepositoryIds
/// via the codebase URL.
#[derive(Default)]
pub struct ValueRegistry {
    ctors: BTreeMap<String, ValueCtor>,
    codebase_resolver: Option<CodebaseResolver>,
}

impl ValueRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a state-reader factory for a RepositoryId.
    pub fn register(&mut self, repo_id: impl Into<String>, ctor: ValueCtor) {
        self.ctors.insert(repo_id.into(), ctor);
    }

    /// Sets the codebase resolver: for RepositoryIds without a static factory it
    /// is queried with `(codebase_url, repo_id)`.
    pub fn set_codebase_resolver(&mut self, resolver: CodebaseResolver) {
        self.codebase_resolver = Some(resolver);
    }

    /// Resolves a factory for `repo_id`: first statically registered, otherwise —
    /// if a codebase URL is present — via the [`CodebaseResolver`].
    fn ctor_for(&self, repo_id: &str, codebase: Option<&str>) -> Option<CtorRef<'_>> {
        if let Some(c) = self.ctors.get(repo_id) {
            return Some(CtorRef::Borrowed(c));
        }
        if let (Some(cb), Some(res)) = (codebase, self.codebase_resolver.as_ref())
            && let Some(c) = res(cb, repo_id)
        {
            return Some(CtorRef::Owned(c));
        }
        None
    }
}

/// Reads valuetype instances with **indirection resolution** (value sharing): an
/// indirection yields the same `Rc` instance as the referenced value_tag.
#[derive(Default)]
pub struct ValueReader {
    /// `value_tag` stream position → already-read instance.
    cache: BTreeMap<usize, Rc<dyn Any>>,
}

impl ValueReader {
    /// Fresh reader.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads an optional valuetype. `base` = absolute stream offset of
    /// reader byte 0 (for the indirection position arithmetic; top-level `0`).
    ///
    /// # Errors
    /// Unknown RepositoryId, unresolvable/forward indirection,
    /// (yet) unsupported chunked encoding, CDR decode error.
    pub fn read(
        &mut self,
        r: &mut Reader<'_>,
        base: usize,
        reg: &ValueRegistry,
    ) -> Result<Option<Rc<dyn Any>>, WireError> {
        r.align_to(4)?;
        let tag_pos = base + r.position();
        let tag = r.read_u32()?;

        if tag == VALUE_NULL {
            return Ok(None);
        }
        if tag == VALUE_INDIRECTION {
            let off_field = base + r.position();
            let offset = r.read_u32()? as i32;
            if offset >= 0 {
                return Err(refused(off_field, "a negative value indirection offset"));
            }
            let target = usize::try_from(off_field as i64 + i64::from(offset))
                .map_err(|_| refused(off_field, "an indirection target inside the stream"))?;
            return self
                .cache
                .get(&target)
                .cloned()
                .map(Some)
                .ok_or_else(|| refused(off_field, "an indirection target already read"));
        }
        if tag < VALUE_TAG_BASE {
            return Err(refused(
                tag_pos,
                "a value tag of 0, 0xffffffff or 0x7fffff00 and above",
            ));
        }
        let chunked = tag & VT_FLAG_CHUNKED != 0;
        // codebase URL (bit 0): read it in + remember it for factory resolution.
        let codebase: Option<String> = if tag & VT_FLAG_CODEBASE != 0 {
            Some(r.read_string()?)
        } else {
            None
        };
        // RepositoryId list: 1 entry for single, n for list (most-derived first,
        // then bases — for truncation).
        let ids: alloc::vec::Vec<String> = match tag & VT_REPO_MASK {
            VT_REPO_SINGLE => alloc::vec![r.read_string()?],
            VT_REPO_LIST => {
                let n = r.read_u32()? as usize;
                let mut ids = alloc::vec::Vec::with_capacity(n.min(16));
                for _ in 0..n {
                    ids.push(r.read_string()?);
                }
                if ids.is_empty() {
                    return Err(refused(base + r.position(), "at least one repository id"));
                }
                ids
            }
            VT_REPO_NONE => return Err(refused(tag_pos, "a value tag carrying type information")),
            _ => return Err(refused(tag_pos, "repository id flags of 0x02 or 0x06")),
        };

        let v = if chunked {
            read_chunked_state(r, &ids, reg, codebase.as_deref())?
        } else {
            // Non-chunked: the most-derived type (ids[0]) must be known
            // (without chunk sizes no truncation is possible) — possibly resolved
            // via the codebase URL.
            let ctor = reg.ctor_for(&ids[0], codebase.as_deref()).ok_or_else(|| {
                refused(tag_pos, "a registered value factory for the repository id")
            })?;
            ctor.call(r)?
        };
        self.cache.insert(tag_pos, Rc::clone(&v));
        Ok(Some(v))
    }
}

/// Reads the **chunked** state of a valuetype (§15.3.4.3) with **truncation**:
/// the first RepositoryId of the list that is known in `reg` (possibly resolved
/// via `codebase`) is instantiated; the derived state remainder is skipped via
/// the chunk sizes up to the end-tag.
///
/// Handles **nested chunked values** (own `value_tag` at a chunk boundary)
/// recursively, **multi-level end-tags** (one end-tag `-k` with
/// `k ≤ 1` also closes this value), and **indirections** in the chunk stream.
fn read_chunked_state(
    r: &mut Reader<'_>,
    ids: &[String],
    reg: &ValueRegistry,
    codebase: Option<&str>,
) -> Result<Rc<dyn Any>, WireError> {
    let ctor = ids
        .iter()
        .find_map(|id| reg.ctor_for(id, codebase))
        .ok_or_else(|| {
            refused(
                r.position(),
                "a registered value factory for one of the repository ids",
            )
        })?;

    let mut value: Option<Rc<dyn Any>> = None;
    loop {
        r.align_to(4)?;
        let marker = r.read_u32()? as i32;
        if marker < 0 {
            break; // end-tag (-nesting_level) → end of this (level-1) value
        }
        let marker_u = marker as u32;
        if marker_u == VALUE_INDIRECTION {
            // Shared value in the chunk stream: 8-byte indirection, skipped here.
            let _offset = r.read_u32()?;
            continue;
        }
        if marker_u >= VALUE_TAG_BASE {
            // Nested value (in the derived tail or as a member that this
            // truncation path does not decode) → consume recursively.
            let closed_level = skip_value_from_tag(r, 2, marker_u)?;
            if closed_level <= 1 {
                break; // shared end-tag also closed this value
            }
            continue;
        }
        let chunk_size = marker as usize;
        let pos_before = r.position();
        if value.is_none() {
            // First data chunk carries the (base) state of the matched type.
            let v = ctor.call(r)?;
            let consumed = r.position() - pos_before;
            if consumed > chunk_size {
                return Err(refused(
                    r.position(),
                    "a value factory that stays inside its chunk",
                ));
            }
            if consumed < chunk_size {
                // Truncation: skip the derived state remainder of this chunk.
                let _ = r.read_bytes(chunk_size - consumed)?;
            }
            value = Some(v);
        } else {
            // Already truncated → skip further data chunks.
            let _ = r.read_bytes(chunk_size)?;
        }
    }
    value.ok_or_else(|| refused(r.position(), "at least one data chunk in a chunked value"))
}

/// Consumes (skips) a nested value whose `value_tag`
/// (`tag`) has already been read, at nesting `depth`. Reads the
/// codebase URL + RepositoryId(s) and — when chunked — the body.
///
/// Returns the end-tag level `k` that closed this (and possibly outer) values:
/// `k == depth` closes exactly this value, `k < depth` additionally closes
/// the `k..depth-1` enclosing values (shared end-tag, §15.3.4.3).
///
/// zerodds-lint: recursion-depth 64 (Valuetype-Chunk-Nesting; bounded by GIOP value chunking)
fn skip_value_from_tag(r: &mut Reader<'_>, depth: u32, tag: u32) -> Result<u32, WireError> {
    if tag & VT_FLAG_CODEBASE != 0 {
        let _ = r.read_string()?;
    }
    match tag & VT_REPO_MASK {
        VT_REPO_SINGLE => {
            let _ = r.read_string()?;
        }
        VT_REPO_LIST => {
            let n = r.read_u32()? as usize;
            for _ in 0..n {
                let _ = r.read_string()?;
            }
        }
        _ => {
            return Err(refused(
                r.position(),
                "a nested value tag carrying type information",
            ));
        }
    }
    if tag & VT_FLAG_CHUNKED == 0 {
        // Non-chunked nested values carry no lengths → not skippable without a
        // factory (does not occur in the chunked context per spec).
        return Err(refused(
            r.position(),
            "a chunked nested value, which is the only skippable form",
        ));
    }
    skip_chunked_body(r, depth)
}

/// Skips the body of a chunked value at `depth` (data chunks,
/// nested values, indirections) up to the closing end-tag. Returns
/// the end-tag level that closed it (see [`skip_value_from_tag`]).
///
/// zerodds-lint: recursion-depth 64 (Valuetype-Chunk-Nesting; bounded by GIOP value chunking)
fn skip_chunked_body(r: &mut Reader<'_>, depth: u32) -> Result<u32, WireError> {
    loop {
        r.align_to(4)?;
        let marker = r.read_u32()? as i32;
        if marker < 0 {
            return Ok((-marker) as u32);
        }
        let marker_u = marker as u32;
        if marker_u == VALUE_INDIRECTION {
            let _offset = r.read_u32()?;
            continue;
        }
        if marker_u >= VALUE_TAG_BASE {
            let closed_level = skip_value_from_tag(r, depth + 1, marker_u)?;
            if closed_level <= depth {
                // Shared end-tag also closed this (and possibly outer) values.
                return Ok(closed_level);
            }
            continue;
        }
        let _ = r.read_bytes(marker as usize)?;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! The expected octets are written out by hand from the value encoding
    //! of CORBA §15.3.4 (`value_tag`, codebase URL, type information,
    //! state, chunks, end tags, indirections) or are JacORB 3.9 captures;
    //! none of them comes from this encoder.
    use super::*;
    use zerodds_wire::Endianness;

    /// Octets from a hex string; blanks separate the fields.
    fn octets(hex: &str) -> alloc::vec::Vec<u8> {
        let digits: alloc::string::String = hex.chars().filter(|c| !c.is_whitespace()).collect();
        (0..digits.len() / 2)
            .map(|i| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn encoded(e: Endianness, write: impl FnOnce(&mut Writer)) -> alloc::vec::Vec<u8> {
        let mut w = Writer::new(e);
        write(&mut w);
        w.finish()
    }

    // `valuetype Point { public long x; public long y; };`, repository id
    // `IDL:Geo/Point:1.0` (17 characters, a CDR string of 18 octets).
    #[derive(Debug, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }
    impl ValueBase for Point {
        fn repository_id(&self) -> &str {
            "IDL:Geo/Point:1.0"
        }
    }
    impl ValueMarshal for Point {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_i32(self.x);
            w.write_i32(self.y);
            Ok(())
        }
    }
    fn point_ctor() -> ValueCtor {
        Box::new(|r: &mut Reader<'_>| {
            let x = r.read_i32()?;
            let y = r.read_i32()?;
            Ok(Rc::new(Point { x, y }) as Rc<dyn Any>)
        })
    }
    fn point_registry() -> ValueRegistry {
        let mut reg = ValueRegistry::new();
        reg.register("IDL:Geo/Point:1.0", point_ctor());
        reg
    }

    /// `IDL:Geo/Point:1.0` with its terminating NUL.
    const GEO_POINT: &str = "49444c3a47656f2f506f696e743a312e3000";

    // valuetype with JacORB RepositoryId for the cross-ORB capture comparison.
    #[derive(Debug, PartialEq, Eq)]
    struct JPoint {
        x: i32,
        y: i32,
    }
    impl ValueBase for JPoint {
        fn repository_id(&self) -> &str {
            "IDL:Point:1.0"
        }
    }
    impl ValueMarshal for JPoint {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_i32(self.x);
            w.write_i32(self.y);
            Ok(())
        }
    }

    /// §15.3.4.1: a value is its `value_tag` (`0x7fffff02`: one repository
    /// id follows), the repository id and the state; a null value is the
    /// tag `0`. The repository id ends at octet 26, the state is aligned
    /// to 28.
    #[test]
    fn corbarust_4_2_a_value_and_a_null_are_the_octets_of_15_3_4_1() {
        for (e, hex) in [
            (
                Endianness::Big,
                alloc::format!("7fffff02 00000012 {GEO_POINT} 0000 00000003 00000007 00000000"),
            ),
            (
                Endianness::Little,
                alloc::format!("02ffff7f 12000000 {GEO_POINT} 0000 03000000 07000000 00000000"),
            ),
        ] {
            let expected = octets(&hex);
            let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 3, y: 7 });
            let written = encoded(e, |w| {
                let mut vw = ValueWriter::new();
                vw.write(w, Some(&p)).unwrap();
                vw.write(w, None).unwrap();
            });
            assert_eq!(written, expected, "{e:?}");

            let reg = point_registry();
            let mut r = Reader::new(&expected, e);
            let mut vr = ValueReader::new();
            let v = vr.read(&mut r, 0, &reg).unwrap().expect("a value");
            assert_eq!(*v.downcast_ref::<Point>().unwrap(), Point { x: 3, y: 7 });
            assert!(vr.read(&mut r, 0, &reg).unwrap().is_none(), "the null");
        }
    }

    /// §15.3.4.1: a value written a second time is the indirection tag
    /// `0xffffffff` and a long offset from that long back to the first
    /// `value_tag` (octet 0 − octet 40 = −40, `0xffffffd8`). Read back,
    /// both references are one instance.
    #[test]
    fn corbarust_4_2_a_shared_value_is_an_indirection_back_to_its_tag() {
        for (e, hex) in [
            (
                Endianness::Big,
                alloc::format!(
                    "7fffff02 00000012 {GEO_POINT} 0000 00000001 00000002 ffffffff ffffffd8"
                ),
            ),
            (
                Endianness::Little,
                alloc::format!(
                    "02ffff7f 12000000 {GEO_POINT} 0000 01000000 02000000 ffffffff d8ffffff"
                ),
            ),
        ] {
            let expected = octets(&hex);
            let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 2 });
            let written = encoded(e, |w| {
                let mut vw = ValueWriter::new();
                vw.write(w, Some(&p)).unwrap();
                vw.write(w, Some(&p)).unwrap();
            });
            assert_eq!(written, expected, "{e:?}");

            let reg = point_registry();
            let mut r = Reader::new(&expected, e);
            let mut vr = ValueReader::new();
            let a = vr.read(&mut r, 0, &reg).unwrap().unwrap();
            let b = vr.read(&mut r, 0, &reg).unwrap().unwrap();
            assert!(Rc::ptr_eq(&a, &b), "both references are one instance");
            assert_eq!(*a.downcast_ref::<Point>().unwrap(), Point { x: 1, y: 2 });
        }
    }

    /// Two instances with equal state are two values, each with its own
    /// `value_tag`, and read back as two instances.
    #[test]
    fn corbarust_4_2_two_instances_are_two_values() {
        let expected = octets(&alloc::format!(
            "7fffff02 00000012 {GEO_POINT} 0000 00000001 00000001 \
             7fffff02 00000012 {GEO_POINT} 0000 00000001 00000001"
        ));
        let p1: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 1 });
        let p2: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 1 });
        let written = encoded(Endianness::Big, |w| {
            let mut vw = ValueWriter::new();
            vw.write(w, Some(&p1)).unwrap();
            vw.write(w, Some(&p2)).unwrap();
        });
        assert_eq!(written, expected);

        let reg = point_registry();
        let mut r = Reader::new(&expected, Endianness::Big);
        let mut vr = ValueReader::new();
        let a = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        let b = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        assert!(!Rc::ptr_eq(&a, &b));
    }

    /// §15.3.4.1: an indirection points back; a positive offset is refused.
    #[test]
    fn corbarust_4_2_a_forward_indirection_is_refused() {
        let bytes = octets("ffffffff 00000004");
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(
            ValueReader::new()
                .read(&mut r, 0, &ValueRegistry::new())
                .is_err()
        );
    }

    /// A value is what JacORB 3.9 writes for it.
    ///
    /// Golden vector from a real
    /// `org.omg.CORBA_2_3.portable.OutputStream.write_value` capture on the
    /// Linux test host: `Point(42,-7)`, big-endian. value_tag 0x7fffff02 +
    /// CDR string "IDL:Point:1.0\0" + align(4) pad + long(42) + long(-7).
    #[test]
    fn corbarust_4_2_a_value_is_the_octets_jacorb_writes() {
        let p: Rc<dyn ValueMarshal> = Rc::new(JPoint { x: 42, y: -7 });
        let written = encoded(Endianness::Big, |w| {
            ValueWriter::new().write(w, Some(&p)).unwrap();
        });
        assert_eq!(
            written,
            octets("7fffff020000000e49444c3a506f696e743a312e300000000000002afffffff9"),
            "the octets of the JacORB capture"
        );
    }

    // Truncatable hierarchy: valuetype Derived : truncatable Base.
    // Base { long id; }; Derived { + string extra; } — state base-first.
    #[derive(Debug, PartialEq, Eq)]
    struct Base {
        id: i32,
    }
    impl ValueBase for Base {
        fn repository_id(&self) -> &str {
            "IDL:Base:1.0"
        }
    }
    impl ValueMarshal for Base {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_i32(self.id);
            Ok(())
        }
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Derived {
        id: i32,
        extra: alloc::string::String,
    }
    impl ValueBase for Derived {
        fn repository_id(&self) -> &str {
            "IDL:Derived:1.0"
        }
    }
    impl ValueMarshal for Derived {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_i32(self.id); // base state first (§15.3.4.3)
            w.write_string(&self.extra);
            Ok(())
        }
    }
    fn base_ctor(reg: &mut ValueRegistry) {
        reg.register(
            "IDL:Base:1.0",
            Box::new(|r: &mut Reader<'_>| Ok(Rc::new(Base { id: r.read_i32()? }) as Rc<dyn Any>)),
        );
    }
    fn derived_ctor(reg: &mut ValueRegistry) {
        reg.register(
            "IDL:Derived:1.0",
            Box::new(|r: &mut Reader<'_>| {
                let id = r.read_i32()?;
                let extra = r.read_string()?;
                Ok(Rc::new(Derived { id, extra }) as Rc<dyn Any>)
            }),
        );
    }

    // Golden vector: JacORB 3.9 write_value(Derived(42,"hi"), "IDL:Derived:1.0"),
    // truncatable → chunked + repo-id list (capture from the Linux test host, 68 bytes, BE).
    const JACORB_CHUNKED: &str = "7fffff0e000000020000001049444c3a446572697665643a312e30000000000d49444c3a426173653a312e30000000000000000b0000002a0000000368690000ffffffff";

    /// The JacORB capture field by field in little-endian (§15.3.4.1 and
    /// §15.3.4.3: every long in the stream's byte order, the chunk size
    /// `11` = long 42 + string length + `hi\0`).
    const CHUNKED_LE: &str = "0effff7f 02000000 10000000 49444c3a446572697665643a312e3000 \
                              0d000000 49444c3a426173653a312e3000 000000 \
                              0b000000 2a000000 03000000 686900 00 ffffffff";

    fn derived_42_hi() -> Rc<dyn ValueMarshal> {
        Rc::new(Derived {
            id: 42,
            extra: "hi".into(),
        })
    }

    #[test]
    fn corbarust_4_3_a_chunked_value_is_the_octets_jacorb_writes() {
        let d = derived_42_hi();
        for (e, expected) in [
            (Endianness::Big, octets(JACORB_CHUNKED)),
            (Endianness::Little, octets(CHUNKED_LE)),
        ] {
            let written = encoded(e, |w| {
                ValueWriter::new()
                    .write_chunked(w, Some(&d), &["IDL:Base:1.0"])
                    .unwrap();
            });
            assert_eq!(written, expected, "{e:?}");
        }
    }

    #[test]
    fn corbarust_4_3_a_known_most_derived_type_reads_the_whole_value() {
        for (e, bytes) in [
            (Endianness::Big, octets(JACORB_CHUNKED)),
            (Endianness::Little, octets(CHUNKED_LE)),
        ] {
            let mut reg = ValueRegistry::new();
            derived_ctor(&mut reg);
            base_ctor(&mut reg);
            let mut r = Reader::new(&bytes, e);
            let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
            assert_eq!(
                *v.downcast_ref::<Derived>().unwrap(),
                Derived {
                    id: 42,
                    extra: "hi".into()
                },
                "{e:?}"
            );
        }
    }

    #[test]
    fn corbarust_4_3_an_unknown_most_derived_type_truncates_to_its_base() {
        let bytes = octets(JACORB_CHUNKED);
        let mut reg = ValueRegistry::new();
        base_ctor(&mut reg);
        let mut r = Reader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 });
        assert_eq!(
            r.position(),
            bytes.len(),
            "the rest of the value is skipped"
        );
    }

    // ---- §4.4 codebase URL --------------------------------------------------

    /// §15.3.4.1: tag `0x7fffff03` (codebase URL and one repository id),
    /// then the URL, then the repository id, then the state.
    const CODEBASE_BE: &str = "7fffff03 00000012 66696c653a2f2f2f73747562732e6a617200 0000 \
                               00000012 49444c3a47656f2f506f696e743a312e3000 0000 \
                               00000005 00000009";

    #[test]
    fn corbarust_4_4_the_codebase_url_comes_ahead_of_the_repository_id() {
        let expected = octets(CODEBASE_BE);
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 5, y: 9 });
        let written = encoded(Endianness::Big, |w| {
            ValueWriter::new()
                .write_with_codebase(w, Some(&p), "file:///stubs.jar")
                .unwrap();
        });
        assert_eq!(written, expected);

        let mut r = Reader::new(&expected, Endianness::Big);
        let v = ValueReader::new()
            .read(&mut r, 0, &point_registry())
            .unwrap()
            .unwrap();
        assert_eq!(*v.downcast_ref::<Point>().unwrap(), Point { x: 5, y: 9 });
    }

    #[test]
    fn corbarust_4_4_the_codebase_resolver_supplies_a_missing_factory() {
        let bytes = octets(CODEBASE_BE);
        let mut reg = ValueRegistry::new();
        reg.set_codebase_resolver(Box::new(|codebase: &str, repo_id: &str| {
            (codebase == "file:///stubs.jar" && repo_id == "IDL:Geo/Point:1.0").then(point_ctor)
        }));
        let mut r = Reader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Point>().unwrap(), Point { x: 5, y: 9 });

        // Without a resolver: no factory for the repository id.
        let mut r2 = Reader::new(&bytes, Endianness::Big);
        assert!(
            ValueReader::new()
                .read(&mut r2, 0, &ValueRegistry::new())
                .is_err()
        );
    }

    // ---- §4.4 nested chunked values + multi-level end-tags ------------

    /// A chunked `Derived` (repository ids Derived, Base) whose first chunk
    /// holds the base state (long 42), followed by a nested chunked value
    /// `IDL:Inner:1.0` with one chunk (`0x0badcafe`), its end tag −2 and the
    /// outer end tag −1 (§15.3.4.3).
    const NESTED_BE: &str = "7fffff0e 00000002 00000010 49444c3a446572697665643a312e3000 \
                             0000000d 49444c3a426173653a312e3000 000000 \
                             00000004 0000002a \
                             7fffff0e 00000001 0000000e 49444c3a496e6e65723a312e3000 0000 \
                             00000004 0badcafe fffffffe ffffffff";
    const NESTED_LE: &str = "0effff7f 02000000 10000000 49444c3a446572697665643a312e3000 \
                             0d000000 49444c3a426173653a312e3000 000000 \
                             04000000 2a000000 \
                             0effff7f 01000000 0e000000 49444c3a496e6e65723a312e3000 0000 \
                             04000000 fecaad0b feffffff ffffffff";

    #[test]
    fn corbarust_4_4_a_nested_value_in_the_skipped_tail_is_consumed() {
        for (e, bytes) in [
            (Endianness::Big, octets(NESTED_BE)),
            (Endianness::Little, octets(NESTED_LE)),
        ] {
            let mut reg = ValueRegistry::new();
            base_ctor(&mut reg);
            let mut r = Reader::new(&bytes, e);
            let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
            assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 }, "{e:?}");
            assert_eq!(r.position(), bytes.len(), "{e:?}");
        }
    }

    /// §15.3.4.3: one end tag −1 closes the nested value (level 2) and the
    /// outer one (level 1) together.
    #[test]
    fn corbarust_4_4_one_end_tag_closes_the_nested_and_the_outer_value() {
        let bytes = octets(
            "7fffff0e 00000002 00000010 49444c3a446572697665643a312e3000 \
             0000000d 49444c3a426173653a312e3000 000000 \
             00000004 0000002a \
             7fffff0e 00000001 0000000e 49444c3a496e6e65723a312e3000 0000 \
             00000004 0badcafe ffffffff",
        );
        let mut reg = ValueRegistry::new();
        base_ctor(&mut reg);
        let mut r = Reader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 });
        assert_eq!(r.position(), bytes.len());
    }

    // ---- multi-chunk ENCODE (write_chunked_tree) ----------------------------

    /// A value whose `value_tag` lists `IDL:Derived:1.0` + a base, but whose
    /// leading state is just the 4-byte base `id` (the derived tail lives in
    /// nested chunks).
    #[derive(Debug)]
    struct OuterBaseState {
        id: i32,
    }
    impl ValueBase for OuterBaseState {
        fn repository_id(&self) -> &str {
            "IDL:Derived:1.0"
        }
    }
    impl ValueMarshal for OuterBaseState {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_i32(self.id);
            Ok(())
        }
    }

    /// A nested chunked value (repo `IDL:Inner:1.0`) holding one 4-byte word.
    #[derive(Debug)]
    struct InnerVal {
        word: u32,
    }
    impl ValueBase for InnerVal {
        fn repository_id(&self) -> &str {
            "IDL:Inner:1.0"
        }
    }
    impl ValueMarshal for InnerVal {
        fn marshal_state(&self, w: &mut Writer) -> Result<(), WireError> {
            w.write_u32(self.word);
            Ok(())
        }
    }

    #[test]
    fn corbarust_4_4_a_chunk_tree_without_nested_values_is_the_jacorb_layout() {
        let d = derived_42_hi();
        for (e, expected) in [
            (Endianness::Big, octets(JACORB_CHUNKED)),
            (Endianness::Little, octets(CHUNKED_LE)),
        ] {
            let node = ChunkedNode {
                value: &d,
                base_ids: &["IDL:Base:1.0"],
                nested: &[],
            };
            let written = encoded(e, |w| {
                ValueWriter::new().write_chunked_tree(w, &node).unwrap();
            });
            assert_eq!(written, expected, "{e:?}");
        }
    }

    #[test]
    fn corbarust_4_4_a_chunk_tree_writes_the_nested_value_and_both_end_tags() {
        for (e, expected) in [
            (Endianness::Big, octets(NESTED_BE)),
            (Endianness::Little, octets(NESTED_LE)),
        ] {
            let inner: Rc<dyn ValueMarshal> = Rc::new(InnerVal { word: 0x0bad_cafe });
            let outer: Rc<dyn ValueMarshal> = Rc::new(OuterBaseState { id: 42 });
            let inner_node = ChunkedNode {
                value: &inner,
                base_ids: &[],
                nested: &[],
            };
            let outer_node = ChunkedNode {
                value: &outer,
                base_ids: &["IDL:Base:1.0"],
                nested: core::slice::from_ref(&inner_node),
            };
            let written = encoded(e, |w| {
                ValueWriter::new()
                    .write_chunked_tree(w, &outer_node)
                    .unwrap();
            });
            assert_eq!(written, expected, "{e:?}");
        }
    }
}
