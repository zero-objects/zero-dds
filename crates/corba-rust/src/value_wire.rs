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

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

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

fn enc_err(message: &'static str) -> EncodeError {
    EncodeError::ValueOutOfRange { message }
}
fn dec_err(kind: &'static str) -> DecodeError {
    DecodeError::InvalidEnum { kind, value: 0 }
}

/// A concrete valuetype that marshals over the §15.3.4 wire. Generated
/// code implements this in addition to [`ValueBase`].
pub trait ValueMarshal: ValueBase {
    /// Writes ONLY the state members (declaration order), without `value_tag`.
    ///
    /// # Errors
    /// CDR encode error.
    fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError>;
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
        w: &mut BufferWriter,
        value: Option<&Rc<dyn ValueMarshal>>,
    ) -> Result<(), EncodeError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL)?;
            return Ok(());
        };
        w.align(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            // Value sharing: indirection backwards to the first value_tag.
            w.write_u32(VALUE_INDIRECTION)?;
            // Offset relative to the position of the offset field (now = tag_pos + 4).
            let offset = i32::try_from(prior as i64 - w.position() as i64)
                .map_err(|_| enc_err("value indirection offset overflow"))?;
            w.write_u32(offset as u32)?;
            return Ok(());
        }
        self.seen.insert(id, tag_pos);
        w.write_u32(VALUE_TAG_BASE | VT_REPO_SINGLE)?; // 0x7fffff02
        w.write_string(v.repository_id())?;
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
        w: &mut BufferWriter,
        value: Option<&Rc<dyn ValueMarshal>>,
        codebase: &str,
    ) -> Result<(), EncodeError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL)?;
            return Ok(());
        };
        w.align(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION)?;
            let offset = i32::try_from(prior as i64 - w.position() as i64)
                .map_err(|_| enc_err("value indirection offset overflow"))?;
            w.write_u32(offset as u32)?;
            return Ok(());
        }
        self.seen.insert(id, tag_pos);
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CODEBASE | VT_REPO_SINGLE)?; // 0x7fffff03
        w.write_string(codebase)?;
        w.write_string(v.repository_id())?;
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
        w: &mut BufferWriter,
        value: Option<&Rc<dyn ValueMarshal>>,
        base_ids: &[&str],
    ) -> Result<(), EncodeError> {
        let Some(v) = value else {
            w.write_u32(VALUE_NULL)?;
            return Ok(());
        };
        w.align(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(v).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION)?;
            let offset = i32::try_from(prior as i64 - w.position() as i64)
                .map_err(|_| enc_err("value indirection offset overflow"))?;
            w.write_u32(offset as u32)?;
            return Ok(());
        }
        self.seen.insert(id, tag_pos);

        // value_tag: chunked + RepositoryId list (0x7fffff0e).
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)?;
        let count = u32::try_from(1 + base_ids.len())
            .map_err(|_| enc_err("value RepositoryId list too long"))?;
        w.write_u32(count)?;
        w.write_string(v.repository_id())?;
        for b in base_ids {
            w.write_string(b)?;
        }

        // Chunk: determine the size up front in an align-origin temp buffer, so that
        // the CDR alignment of the state (including 8-byte) exactly matches the stream offset.
        w.align(4);
        let state_offset = w.position() + 4; // the chunk_size long occupies 4 bytes
        let mut tmp = BufferWriter::new(w.endianness()).with_align_origin(state_offset);
        v.marshal_state(&mut tmp)?;
        let state = tmp.into_bytes();
        let size = i32::try_from(state.len()).map_err(|_| enc_err("chunk size overflow"))?;
        w.write_u32(size as u32)?;
        w.write_bytes(&state)?;

        // End-tag -1 (outermost value).
        w.align(4);
        w.write_u32((-1i32) as u32)
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
        w: &mut BufferWriter,
        node: &ChunkedNode<'_>,
    ) -> Result<(), EncodeError> {
        self.write_chunked_node(w, node, 1)
    }

    /// Recursive worker for [`write_chunked_tree`](Self::write_chunked_tree).
    /// `level` is the 1-based nesting depth used for this value's end-tag.
    ///
    /// zerodds-lint: recursion-depth 64 (Valuetype-Chunk-Nesting; bounded by GIOP value chunking)
    fn write_chunked_node(
        &mut self,
        w: &mut BufferWriter,
        node: &ChunkedNode<'_>,
        level: u32,
    ) -> Result<(), EncodeError> {
        w.align(4);
        let tag_pos = w.position();
        let id = Rc::as_ptr(node.value).cast::<()>() as usize;
        if let Some(&prior) = self.seen.get(&id) {
            w.write_u32(VALUE_INDIRECTION)?;
            let offset = i32::try_from(prior as i64 - w.position() as i64)
                .map_err(|_| enc_err("value indirection offset overflow"))?;
            w.write_u32(offset as u32)?;
            return Ok(());
        }
        self.seen.insert(id, tag_pos);

        // value_tag: chunked + RepositoryId list.
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)?;
        let count = u32::try_from(1 + node.base_ids.len())
            .map_err(|_| enc_err("value RepositoryId list too long"))?;
        w.write_u32(count)?;
        w.write_string(node.value.repository_id())?;
        for b in node.base_ids {
            w.write_string(b)?;
        }

        // Leading data chunk: the node's own state.
        w.align(4);
        let state_offset = w.position() + 4;
        let mut tmp = BufferWriter::new(w.endianness()).with_align_origin(state_offset);
        node.value.marshal_state(&mut tmp)?;
        let state = tmp.into_bytes();
        let size = i32::try_from(state.len()).map_err(|_| enc_err("chunk size overflow"))?;
        w.write_u32(size as u32)?;
        w.write_bytes(&state)?;

        // Nested chunked values follow at chunk boundaries (derived tail),
        // each self-delimited by its own end-tag at the deeper level.
        for child in node.nested {
            self.write_chunked_node(w, child, level + 1)?;
        }

        // This value's own end-tag (-level).
        w.align(4);
        w.write_u32((-(level as i32)) as u32)
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
pub type ValueCtor = Box<dyn Fn(&mut BufferReader<'_>) -> Result<Rc<dyn Any>, DecodeError>>;

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
    fn call(&self, r: &mut BufferReader<'_>) -> Result<Rc<dyn Any>, DecodeError> {
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
        if let (Some(cb), Some(res)) = (codebase, self.codebase_resolver.as_ref()) {
            if let Some(c) = res(cb, repo_id) {
                return Some(CtorRef::Owned(c));
            }
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
        r: &mut BufferReader<'_>,
        base: usize,
        reg: &ValueRegistry,
    ) -> Result<Option<Rc<dyn Any>>, DecodeError> {
        r.align(4)?;
        let tag_pos = base + r.position();
        let tag = r.read_u32()?;

        if tag == VALUE_NULL {
            return Ok(None);
        }
        if tag == VALUE_INDIRECTION {
            let off_field = base + r.position();
            let offset = r.read_u32()? as i32;
            if offset >= 0 {
                return Err(dec_err("value indirection: offset must be negative"));
            }
            let target = usize::try_from(off_field as i64 + i64::from(offset))
                .map_err(|_| dec_err("value indirection: target before stream start"))?;
            return self
                .cache
                .get(&target)
                .cloned()
                .map(Some)
                .ok_or_else(|| dec_err("value indirection: unresolved target"));
        }
        if tag < VALUE_TAG_BASE {
            return Err(dec_err("invalid value_tag"));
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
                    return Err(dec_err("empty value RepositoryId list"));
                }
                ids
            }
            VT_REPO_NONE => return Err(dec_err("value without type info unsupported")),
            _ => return Err(dec_err("invalid value_tag repo-id flags")),
        };

        let v = if chunked {
            read_chunked_state(r, &ids, reg, codebase.as_deref())?
        } else {
            // Non-chunked: the most-derived type (ids[0]) must be known
            // (without chunk sizes no truncation is possible) — possibly resolved
            // via the codebase URL.
            let ctor = reg
                .ctor_for(&ids[0], codebase.as_deref())
                .ok_or_else(|| dec_err("no ValueFactory for RepositoryId"))?;
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
    r: &mut BufferReader<'_>,
    ids: &[String],
    reg: &ValueRegistry,
    codebase: Option<&str>,
) -> Result<Rc<dyn Any>, DecodeError> {
    let ctor = ids
        .iter()
        .find_map(|id| reg.ctor_for(id, codebase))
        .ok_or_else(|| dec_err("no ValueFactory for any RepositoryId in chunked value"))?;

    let mut value: Option<Rc<dyn Any>> = None;
    loop {
        r.align(4)?;
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
                return Err(dec_err("chunked value: ctor over-read the chunk"));
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
    value.ok_or_else(|| dec_err("chunked value produced no state"))
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
fn skip_value_from_tag(r: &mut BufferReader<'_>, depth: u32, tag: u32) -> Result<u32, DecodeError> {
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
        _ => return Err(dec_err("nested value without type info")),
    }
    if tag & VT_FLAG_CHUNKED == 0 {
        // Non-chunked nested values carry no lengths → not skippable without a
        // factory (does not occur in the chunked context per spec).
        return Err(dec_err("non-chunked nested value cannot be skipped"));
    }
    skip_chunked_body(r, depth)
}

/// Skips the body of a chunked value at `depth` (data chunks,
/// nested values, indirections) up to the closing end-tag. Returns
/// the end-tag level that closed it (see [`skip_value_from_tag`]).
///
/// zerodds-lint: recursion-depth 64 (Valuetype-Chunk-Nesting; bounded by GIOP value chunking)
fn skip_chunked_body(r: &mut BufferReader<'_>, depth: u32) -> Result<u32, DecodeError> {
    loop {
        r.align(4)?;
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
    use super::*;
    use zerodds_cdr::{CdrDecode, CdrEncode, Endianness};

    // Test valuetype: `valuetype Point { public long x; public long y; };`
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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            self.x.encode(w)?;
            self.y.encode(w)
        }
    }
    fn point_registry() -> ValueRegistry {
        let mut reg = ValueRegistry::new();
        reg.register(
            "IDL:Geo/Point:1.0",
            Box::new(|r: &mut BufferReader<'_>| {
                let x = i32::decode(r)?;
                let y = i32::decode(r)?;
                Ok(Rc::new(Point { x, y }) as Rc<dyn Any>)
            }),
        );
        reg
    }

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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            self.x.encode(w)?;
            self.y.encode(w)
        }
    }

    /// Cross-ORB conformance: ZeroDDS must marshal byte-identically to JacORB 3.9
    /// (the reference ORB). Golden vector from a real
    /// `org.omg.CORBA_2_3.portable.OutputStream.write_value` capture on the Linux test host:
    /// `Point(42,-7)`, big-endian. value_tag 0x7fffff02 + CDR string "IDL:Point:1.0\0"
    /// + align(4) pad + long(42) + long(-7).
    #[test]
    fn jacorb_capture_byte_identical() {
        let mut w = BufferWriter::new(Endianness::Big);
        let mut vw = ValueWriter::new();
        let p: Rc<dyn ValueMarshal> = Rc::new(JPoint { x: 42, y: -7 });
        vw.write(&mut w, Some(&p)).unwrap();
        let bytes = w.into_bytes();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex, "7fffff020000000e49444c3a506f696e743a312e300000000000002afffffff9",
            "ZeroDDS-Valuetype-Wire weicht von JacORB-Capture ab"
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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            self.id.encode(w)
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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            self.id.encode(w)?; // base state first (§15.3.4.3)
            self.extra.encode(w)
        }
    }
    fn base_ctor(reg: &mut ValueRegistry) {
        reg.register(
            "IDL:Base:1.0",
            Box::new(|r: &mut BufferReader<'_>| {
                Ok(Rc::new(Base {
                    id: i32::decode(r)?,
                }) as Rc<dyn Any>)
            }),
        );
    }
    fn derived_ctor(reg: &mut ValueRegistry) {
        reg.register(
            "IDL:Derived:1.0",
            Box::new(|r: &mut BufferReader<'_>| {
                let id = i32::decode(r)?;
                let extra = alloc::string::String::decode(r)?;
                Ok(Rc::new(Derived { id, extra }) as Rc<dyn Any>)
            }),
        );
    }

    // Golden vector: JacORB 3.9 write_value(Derived(42,"hi"), "IDL:Derived:1.0"),
    // truncatable → chunked + repo-id list (capture from the Linux test host, 68 bytes, BE).
    const JACORB_CHUNKED: &str = "7fffff0e000000020000001049444c3a446572697665643a312e30000000000d49444c3a426173653a312e30000000000000000b0000002a0000000368690000ffffffff";

    #[test]
    fn chunked_encode_byte_identical_to_jacorb() {
        let mut w = BufferWriter::new(Endianness::Big);
        let mut vw = ValueWriter::new();
        let d: Rc<dyn ValueMarshal> = Rc::new(Derived {
            id: 42,
            extra: "hi".into(),
        });
        vw.write_chunked(&mut w, Some(&d), &["IDL:Base:1.0"])
            .unwrap();
        let hex: alloc::string::String = w
            .into_bytes()
            .iter()
            .map(|b| alloc::format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex, JACORB_CHUNKED,
            "chunked-Wire weicht von JacORB-Capture ab"
        );
    }

    fn jacorb_chunked_bytes() -> alloc::vec::Vec<u8> {
        (0..JACORB_CHUNKED.len() / 2)
            .map(|i| u8::from_str_radix(&JACORB_CHUNKED[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn chunked_decode_full_when_derived_known() {
        let bytes = jacorb_chunked_bytes();
        let mut reg = ValueRegistry::new();
        derived_ctor(&mut reg);
        base_ctor(&mut reg);
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(
            *v.downcast_ref::<Derived>().unwrap(),
            Derived {
                id: 42,
                extra: "hi".into()
            }
        );
    }

    #[test]
    fn chunked_decode_truncates_to_base() {
        // Only Base known → most-derived (Derived) is skipped, truncation.
        let bytes = jacorb_chunked_bytes();
        let mut reg = ValueRegistry::new();
        base_ctor(&mut reg);
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 });
    }

    #[test]
    fn chunked_roundtrip_zerodds_to_zerodds() {
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = BufferWriter::new(e);
            let d: Rc<dyn ValueMarshal> = Rc::new(Derived {
                id: 7,
                extra: "xyz".into(),
            });
            ValueWriter::new()
                .write_chunked(&mut w, Some(&d), &["IDL:Base:1.0"])
                .unwrap();
            let bytes = w.into_bytes();
            let mut reg = ValueRegistry::new();
            derived_ctor(&mut reg);
            base_ctor(&mut reg);
            let mut r = BufferReader::new(&bytes, e);
            let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
            assert_eq!(
                *v.downcast_ref::<Derived>().unwrap(),
                Derived {
                    id: 7,
                    extra: "xyz".into()
                }
            );
        }
    }

    #[test]
    fn single_value_and_null_roundtrip() {
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = BufferWriter::new(e);
            let mut vw = ValueWriter::new();
            let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 3, y: 7 });
            vw.write(&mut w, Some(&p)).unwrap();
            vw.write(&mut w, None).unwrap(); // null
            let bytes = w.into_bytes();
            // value_tag = 0x7fffff02 (single repo-id).
            let mut r = BufferReader::new(&bytes, e);
            let mut vr = ValueReader::new();
            let reg = point_registry();
            let v = vr.read(&mut r, 0, &reg).unwrap().expect("non-null");
            let pt = v.downcast_ref::<Point>().expect("Point");
            assert_eq!(*pt, Point { x: 3, y: 7 });
            assert!(vr.read(&mut r, 0, &reg).unwrap().is_none()); // null
        }
    }

    #[test]
    fn value_sharing_indirection() {
        // The same Rc instance written twice → the second is an indirection.
        // On reading, both must resolve to the SAME Rc instance.
        for e in [Endianness::Big, Endianness::Little] {
            let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 2 });
            let mut w = BufferWriter::new(e);
            let mut vw = ValueWriter::new();
            vw.write(&mut w, Some(&p)).unwrap();
            vw.write(&mut w, Some(&p)).unwrap(); // same instance → indirection
            let bytes = w.into_bytes();

            let mut r = BufferReader::new(&bytes, e);
            let mut vr = ValueReader::new();
            let reg = point_registry();
            let a = vr.read(&mut r, 0, &reg).unwrap().unwrap();
            let b = vr.read(&mut r, 0, &reg).unwrap().unwrap();
            assert!(
                Rc::ptr_eq(&a, &b),
                "value sharing: both refs = one instance"
            );
            assert_eq!(*a.downcast_ref::<Point>().unwrap(), Point { x: 1, y: 2 });
        }
    }

    #[test]
    fn distinct_values_are_not_shared() {
        let p1: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 1 });
        let p2: Rc<dyn ValueMarshal> = Rc::new(Point { x: 2, y: 2 });
        let mut w = BufferWriter::new(Endianness::Big);
        let mut vw = ValueWriter::new();
        vw.write(&mut w, Some(&p1)).unwrap();
        vw.write(&mut w, Some(&p2)).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let mut vr = ValueReader::new();
        let reg = point_registry();
        let a = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        let b = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(*a.downcast_ref::<Point>().unwrap(), Point { x: 1, y: 1 });
        assert_eq!(*b.downcast_ref::<Point>().unwrap(), Point { x: 2, y: 2 });
    }

    #[test]
    fn value_tag_is_single_repo_id_on_wire() {
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 0, y: 0 });
        let mut w = BufferWriter::new(Endianness::Big);
        ValueWriter::new().write(&mut w, Some(&p)).unwrap();
        let bytes = w.into_bytes();
        // Erste 4 Byte = value_tag 0x7fffff02 (BE).
        assert_eq!(&bytes[0..4], &[0x7f, 0xff, 0xff, 0x02]);
    }

    #[test]
    fn forward_indirection_rejected() {
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u32(VALUE_INDIRECTION).unwrap();
        w.write_u32(4).unwrap(); // positive offset
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert!(
            ValueReader::new()
                .read(&mut r, 0, &ValueRegistry::new())
                .is_err()
        );
    }

    // ---- §4.4 codebase-URL ----------------------------------------------------

    #[test]
    fn codebase_value_tag_and_roundtrip() {
        let mut w = BufferWriter::new(Endianness::Big);
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 5, y: 9 });
        ValueWriter::new()
            .write_with_codebase(&mut w, Some(&p), "file:///stubs.jar")
            .unwrap();
        let bytes = w.into_bytes();
        // value_tag 0x7fffff03 (single repo-id + codebase flag).
        assert_eq!(&bytes[0..4], &[0x7f, 0xff, 0xff, 0x03]);
        // Reads back correctly with a normal registry (RepositoryId known).
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let reg = point_registry();
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Point>().unwrap(), Point { x: 5, y: 9 });
    }

    #[test]
    fn codebase_resolver_supplies_missing_factory() {
        let mut w = BufferWriter::new(Endianness::Big);
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 1, y: 2 });
        ValueWriter::new()
            .write_with_codebase(&mut w, Some(&p), "ior://factory-host/Point")
            .unwrap();
        let bytes = w.into_bytes();

        // Registry WITHOUT a Point factory, but WITH a codebase resolver that
        // supplies it based on the codebase URL.
        let mut reg = ValueRegistry::new();
        reg.set_codebase_resolver(Box::new(|codebase: &str, repo_id: &str| {
            if codebase.contains("factory-host") && repo_id == "IDL:Geo/Point:1.0" {
                Some(Box::new(|r: &mut BufferReader<'_>| {
                    let x = i32::decode(r)?;
                    let y = i32::decode(r)?;
                    Ok(Rc::new(Point { x, y }) as Rc<dyn Any>)
                }) as ValueCtor)
            } else {
                None
            }
        }));

        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Point>().unwrap(), Point { x: 1, y: 2 });

        // Without a resolver: the same bytes → error (no factory).
        let mut r2 = BufferReader::new(&bytes, Endianness::Big);
        assert!(
            ValueReader::new()
                .read(&mut r2, 0, &ValueRegistry::new())
                .is_err()
        );
    }

    // ---- §4.4 nested chunked values + multi-level end-tags ------------

    /// Builds a chunked `Derived` value (repo-ids [Derived, Base]) whose
    /// derived tail contains a **nested chunked value** (`IDL:Inner:1.0`),
    /// terminated with its own end-tag (-2), then the outer end-tag (-1).
    fn chunked_with_nested_tail(e: Endianness) -> alloc::vec::Vec<u8> {
        let mut w = BufferWriter::new(e);
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)
            .unwrap(); // 0x7fffff0e
        w.write_u32(2).unwrap();
        w.write_string("IDL:Derived:1.0").unwrap();
        w.write_string("IDL:Base:1.0").unwrap();
        // Data chunk 1: Base state (long id = 42).
        w.align(4);
        w.write_u32(4).unwrap();
        w.write_u32(42).unwrap();
        // Nested chunked value in the derived tail.
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)
            .unwrap();
        w.write_u32(1).unwrap();
        w.write_string("IDL:Inner:1.0").unwrap();
        w.align(4);
        w.write_u32(4).unwrap(); // nested chunk size
        w.write_u32(0x0bad_cafe).unwrap(); // nested state
        w.align(4);
        w.write_u32((-2i32) as u32).unwrap(); // nested end-tag (depth 2)
        // Outer end-tag (depth 1).
        w.align(4);
        w.write_u32((-1i32) as u32).unwrap();
        w.into_bytes()
    }

    #[test]
    fn nested_chunked_value_in_tail_is_consumed_on_truncation() {
        for e in [Endianness::Big, Endianness::Little] {
            let bytes = chunked_with_nested_tail(e);
            // Only Base known → truncation to Base; the nested value in the
            // tail must be consumed correctly (no error).
            let mut reg = ValueRegistry::new();
            base_ctor(&mut reg);
            let mut r = BufferReader::new(&bytes, e);
            let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
            assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 });
            // Entire value stream consumed (position at the end).
            assert_eq!(r.position(), bytes.len());
        }
    }

    #[test]
    fn shared_end_tag_closes_nested_and_outer() {
        // Multi-level end-tag: ONE end-tag -1 closes the nested value
        // (depth 2) AND the outer one (depth 1) together (§15.3.4.3).
        let e = Endianness::Big;
        let mut w = BufferWriter::new(e);
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)
            .unwrap();
        w.write_u32(2).unwrap();
        w.write_string("IDL:Derived:1.0").unwrap();
        w.write_string("IDL:Base:1.0").unwrap();
        w.align(4);
        w.write_u32(4).unwrap();
        w.write_u32(42).unwrap(); // Base.id
        // Nested value …
        w.write_u32(VALUE_TAG_BASE | VT_FLAG_CHUNKED | VT_REPO_LIST)
            .unwrap();
        w.write_u32(1).unwrap();
        w.write_string("IDL:Inner:1.0").unwrap();
        w.align(4);
        w.write_u32(4).unwrap();
        w.write_u32(0x0bad_cafe).unwrap();
        // … shared end-tag -1 closes nested (2) AND outer (1).
        w.align(4);
        w.write_u32((-1i32) as u32).unwrap();
        let bytes = w.into_bytes();

        let mut reg = ValueRegistry::new();
        base_ctor(&mut reg);
        let mut r = BufferReader::new(&bytes, e);
        let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
        assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 42 });
        assert_eq!(r.position(), bytes.len());
    }

    // ---- multi-chunk ENCODE (write_chunked_tree) ----------------------------

    /// A value whose `value_tag` lists `IDL:Derived:1.0` + a base, but whose
    /// leading state is just the 4-byte base `id` (the derived tail lives in
    /// nested chunks). Mirrors the `chunked_with_nested_tail` hand-built wire.
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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            self.id.encode(w)
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
        fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
            w.write_u32(self.word)
        }
    }

    #[test]
    fn chunked_tree_leaf_is_byte_identical_to_write_chunked() {
        // A node with no nested children must produce exactly the same wire as
        // the single-chunk write_chunked (regression guard for the shared path).
        for e in [Endianness::Big, Endianness::Little] {
            let d: Rc<dyn ValueMarshal> = Rc::new(Derived {
                id: 42,
                extra: "hi".into(),
            });

            let mut w1 = BufferWriter::new(e);
            ValueWriter::new()
                .write_chunked(&mut w1, Some(&d), &["IDL:Base:1.0"])
                .unwrap();

            let mut w2 = BufferWriter::new(e);
            let node = ChunkedNode {
                value: &d,
                base_ids: &["IDL:Base:1.0"],
                nested: &[],
            };
            ValueWriter::new()
                .write_chunked_tree(&mut w2, &node)
                .unwrap();

            assert_eq!(
                w1.into_bytes(),
                w2.into_bytes(),
                "leaf tree != write_chunked"
            );
        }
    }

    #[test]
    fn chunked_tree_with_nested_matches_handbuilt_wire() {
        // The encoder reproduces the exact multi-chunk wire that the
        // nested-chunk DECODE tests validate (chunked_with_nested_tail):
        // outer [Derived,Base] data chunk(42) + nested Inner chunk(0x0badcafe)
        // + nested end-tag -2 + outer end-tag -1.
        for e in [Endianness::Big, Endianness::Little] {
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
            let mut w = BufferWriter::new(e);
            ValueWriter::new()
                .write_chunked_tree(&mut w, &outer_node)
                .unwrap();
            assert_eq!(
                w.into_bytes(),
                chunked_with_nested_tail(e),
                "multi-chunk encode != hand-built nested-tail wire"
            );
        }
    }

    #[test]
    fn chunked_tree_roundtrips_with_base_truncation() {
        // Encoder output decodes: a base-only reader truncates to the outer
        // base value and consumes the entire nested tail.
        for e in [Endianness::Big, Endianness::Little] {
            let inner: Rc<dyn ValueMarshal> = Rc::new(InnerVal { word: 0x0bad_cafe });
            let outer: Rc<dyn ValueMarshal> = Rc::new(OuterBaseState { id: 99 });
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
            let mut w = BufferWriter::new(e);
            ValueWriter::new()
                .write_chunked_tree(&mut w, &outer_node)
                .unwrap();
            let bytes = w.into_bytes();

            let mut reg = ValueRegistry::new();
            base_ctor(&mut reg);
            let mut r = BufferReader::new(&bytes, e);
            let v = ValueReader::new().read(&mut r, 0, &reg).unwrap().unwrap();
            assert_eq!(*v.downcast_ref::<Base>().unwrap(), Base { id: 99 });
            assert_eq!(r.position(), bytes.len(), "nested tail not fully consumed");
        }
    }
}
