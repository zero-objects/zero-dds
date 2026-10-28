// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Runtime types that the generated code references.
//!
//! Two of them are no longer declared here. `ObjectReference` and
//! `ValueBase` are Annex A.1 rows, `zerodds-corba-codegen`'s Rust
//! column named `zerodds_corba_rt` for both, and that crate did not
//! carry them — so they moved there and this re-exports them. A
//! program that holds an object reference and never generates a line
//! of code should not depend on a code generator.
//!
//! `TypeCode` is the same move made earlier: this crate used to
//! declare a placeholder struct wrapping a repository id, nothing in
//! the workspace referred to it, and `zerodds_corba_rt::TypeCode` is
//! the real §15.3.5 type. The placeholder is gone.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
pub use zerodds_corba_rt::{ObjectReference, TypeCode, ValueBase};
use zerodds_wire::{Endianness, Reader, WireError, Writer, WstringUnitOrder};

/// CORBA system/user exception. Generated stubs/skeletons map
/// all error paths onto this variant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CorbaException {
    /// CORBA system exception (Spec §3.17.1). Minor code per OMG.
    SystemException {
        /// Minor code from the spec standard (e.g. `0x4F4D000B` =
        /// `BAD_OPERATION`).
        minor: u32,
        /// Static error description.
        message: &'static str,
    },
    /// User exception from IDL `exception E { ... };` (§15.4.3 / §15.3.5.1).
    ///
    /// `body` is the **complete** UserException reply body as a
    /// contiguous CDR stream: `string repository_id` **followed** by the
    /// exception members. Members are NOT extracted separately — their
    /// CDR alignment is relative to the body start (which is 8-aligned in
    /// the GIOP reply); a typed decoder first reads the repo-id (positioning
    /// the reader), then the members. `endianness` is the byte order of `body`
    /// (= reply byte order), required for correct member decoding.
    UserException {
        /// Repository ID of the exception type (for type selection + display);
        /// also appears at the start of `body`.
        repository_id: String,
        /// Complete exception body: `string repo_id` + members (CDR).
        body: Vec<u8>,
        /// On-wire byte order of `body`.
        endianness: Endianness,
    },
}

impl ::core::fmt::Display for CorbaException {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::SystemException { minor, message } => {
                write!(f, "CORBA::SystemException(minor={minor}): {message}")
            }
            Self::UserException { repository_id, .. } => {
                write!(f, "CORBA::UserException({repository_id})")
            }
        }
    }
}

impl ::std::error::Error for CorbaException {}

/// Result of a skeleton dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkeletonResult {
    /// Operation succeeded, reply bytes are available.
    Reply(Vec<u8>),
    /// Operation threw a user or system exception.
    Exception(CorbaException),
    /// Unknown operation name for the interface — the POA
    /// must return BAD_OPERATION.
    BadOperation,
    /// Operation is declared but wire marshalling is phase 2 —
    /// placeholder for now.
    NotYetWired,
}

/// POA servant marker. Concrete server implementations implement this
/// plus the respective IDL interface trait.
pub trait Servant {
    /// Repository ID of the interface that the servant implements.
    fn target_repository_id(&self) -> &str;
}

/// The writer a generated skeleton puts its reply in: the request's byte
/// order, and every `wstring` with its byte-order mark.
///
/// A server does not know which ORB asked. Measured on the Linux test host with
/// `crates/corba-interop/competitors/run_interop.sh`: the reply's
/// `wstring` without a mark, its units in the little-endian order of the
/// request, read back wrong in omniORB 4.3 and TAO 2.5 and right in
/// JacORB 3.9; omniORB's own request carries `ff fe` ahead of its units.
/// With the mark the order is the reply's own statement, whatever a
/// reader assumes without one.
#[must_use]
pub fn reply_writer(endianness: Endianness) -> Writer {
    Writer::new(endianness).with_wstring_unit_order(WstringUnitOrder::ByteOrderMark)
}

/// Answers one of the six `CORBA::Object` operations CORBA 3.3 Part 2
/// §15.4.2 lets a client send as a GIOP request without the target
/// interface having declared it (`_interface`, `_is_a`,
/// `_non_existent`/`_not_existent`, `_domain_managers`, `_component`,
/// `_repository_id`) — Part 1 §8.3 gives their semantics on
/// `CORBA::Object` itself. Every generated `dispatch_<iface>` calls this
/// first, ahead of its own `match operation`; `repository_id` and
/// `base_repository_ids` are the interface's own id and its transitive
/// base-interface chain, both computed by the codegen from the model
/// (`interface_emit::emit_interface_skeleton`).
///
/// `None` when `operation` is not one of the six — the generated
/// `match` continues to the interface's own operations as before.
#[must_use]
pub fn dispatch_object_operation(
    operation: &str,
    body: &[u8],
    endianness: Endianness,
    repository_id: &str,
    base_repository_ids: &[&str],
) -> Option<SkeletonResult> {
    let reply = object_ops::dispatch_with_bases(
        operation,
        body,
        endianness,
        repository_id,
        base_repository_ids,
    )?;
    Some(match reply {
        ObjectOpReply::Ok(b) => SkeletonResult::Reply(b),
        ObjectOpReply::Marshal => SkeletonResult::Exception(CorbaException::SystemException {
            minor: 0,
            message: "CORBA MARSHAL",
        }),
        ObjectOpReply::InterfaceRepositoryUnavailable => {
            SkeletonResult::Exception(CorbaException::SystemException {
                minor: 0,
                message: "CORBA INTF_REPOS",
            })
        }
    })
}

// ============================================================================
// §7.2 POA configuration builder (Spec §11.3 — 7 policies)
// ============================================================================

/// CORBA POA builder for server-side object activation.
///
/// Spec §11.3 defines 7 policies; all have sensible defaults
/// for typical embedded/server use cases. The end user
/// overrides selectively via `with_*` methods and calls `build()`
/// for the final `PoaConfiguration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoaBuilder {
    /// Threading policy (Spec §11.3.4).
    pub thread_policy: ThreadPolicy,
    /// Lifespan policy (§11.3.5).
    pub lifespan_policy: LifespanPolicy,
    /// IdAssignment policy (§11.3.6).
    pub id_assignment_policy: IdAssignmentPolicy,
    /// IdUniqueness policy (§11.3.7).
    pub id_uniqueness_policy: IdUniquenessPolicy,
    /// ImplicitActivation policy (§11.3.8).
    pub implicit_activation_policy: ImplicitActivationPolicy,
    /// ServantRetention policy (§11.3.9).
    pub servant_retention_policy: ServantRetentionPolicy,
    /// RequestProcessing policy (§11.3.10).
    pub request_processing_policy: RequestProcessingPolicy,
}

impl Default for PoaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PoaBuilder {
    /// Creates a new POA builder with spec default policies.
    #[must_use]
    pub fn new() -> Self {
        Self {
            thread_policy: ThreadPolicy::OrbCtrlModel,
            lifespan_policy: LifespanPolicy::Transient,
            id_assignment_policy: IdAssignmentPolicy::SystemId,
            id_uniqueness_policy: IdUniquenessPolicy::Unique,
            implicit_activation_policy: ImplicitActivationPolicy::NoImplicitActivation,
            servant_retention_policy: ServantRetentionPolicy::Retain,
            request_processing_policy: RequestProcessingPolicy::UseActiveObjectMapOnly,
        }
    }
}

/// POA threading policy (Spec §11.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadPolicy {
    /// Default: ORB chooses threading.
    OrbCtrlModel,
    /// Single-threaded.
    SingleThreadModel,
    /// Main-thread-only.
    MainThreadModel,
}

/// POA lifespan policy (Spec §11.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifespanPolicy {
    /// Default: object refs are valid only for the ORB lifetime.
    Transient,
    /// Object refs survive an ORB restart.
    Persistent,
}

/// POA IdAssignment policy (§11.3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdAssignmentPolicy {
    /// Default: ORB assigns object IDs.
    SystemId,
    /// Application assigns object IDs.
    UserId,
}

/// POA IdUniqueness policy (§11.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdUniquenessPolicy {
    /// Default: each servant has exactly one object ID.
    Unique,
    /// A servant may have multiple object IDs.
    Multiple,
}

/// POA ImplicitActivation policy (§11.3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitActivationPolicy {
    /// Default: explicit `activate_object` required.
    NoImplicitActivation,
    /// Auto-activation on the first method call.
    ImplicitActivation,
}

/// POA ServantRetention policy (§11.3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServantRetentionPolicy {
    /// Default: ActiveObjectMap holds servants.
    Retain,
    /// Stateless — a fresh servant per request.
    NonRetain,
}

/// POA RequestProcessing policy (§11.3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestProcessingPolicy {
    /// Default: ActiveObjectMap lookup, otherwise BAD_OPERATION.
    UseActiveObjectMapOnly,
    /// Lookup, then ServantManager fallback.
    UseDefaultServant,
    /// Lookup, then ServantActivator/Locator.
    UseServantManager,
}

// ============================================================================
// §4.4 Valuetype wire marshalling — minimal stream API
// ============================================================================

/// Stream API for ValueBase wire marshalling (CDR §15.3.4).
///
/// Phase 1: API skeleton that wraps `zerodds_wire::Reader`/`Writer` and
/// applies the value-tag logic (chunked encoding, repository-id list).
/// Phase 2 wires the full spec conformance (truncatable, custom
/// marshaling, codeset).
pub struct ValueStreamWriter<'a> {
    /// Inner writer.
    pub inner: &'a mut Writer,
}

impl<'a> ValueStreamWriter<'a> {
    /// Constructor.
    pub fn new(inner: &'a mut Writer) -> Self {
        Self { inner }
    }

    /// Writes a value-tag (CDR §15.3.4.2). Format: 0x7FFFFF02
    /// for single-repository-id, 0x7FFFFF06 for list-of-repository-
    /// ids, 0x00000000 for null-value.
    ///
    pub fn write_value_tag(&mut self, repository_id: &str) {
        // Single-repository-id tag (chunked = false, codeset = false).
        self.inner.write_u32(0x7FFF_FF02);
        self.inner.write_string(repository_id);
    }

    /// Writes a value-tag with a multi-repository-id list (CDR
    /// §15.3.4.2). Wire tag = 0x7FFF_FF06 (`list-of-repository-ids` +
    /// no chunked bit). Layout:
    ///
    /// ```text
    /// u32 tag = 0x7FFFFF06
    /// i32 count                 // > 0
    /// string id[0] .. id[N-1]   // CDR-strings (length + bytes + NUL)
    /// ```
    ///
    pub fn write_value_tag_multi(&mut self, repository_ids: &[&str]) {
        debug_assert!(
            !repository_ids.is_empty(),
            "Spec §15.3.4.2: list-of-repository-ids must contain >=1 entry"
        );
        self.inner.write_u32(0x7FFF_FF06);
        // count as signed long.
        let count: i32 = repository_ids.len() as i32;
        self.inner.write_u32(count as u32);
        for id in repository_ids {
            self.inner.write_string(id);
        }
    }

    /// Writes a chunked value-tag (CDR §15.3.4.2 + §15.3.4.3 —
    /// "chunked encoding"). Wire tag = 0x7FFF_FF0A
    /// (chunked flag + multi-repo-id flag, both set).
    ///
    /// After the tag come:
    /// 1. `i32 count` + `string id[N]` (multi-repo-id-list).
    /// 2. Per chunk: `i32 chunk_size_bytes` + `octet chunk_data[..]`.
    /// 3. Finally: `i32 end_tag = -<nesting_level>` (Spec
    ///    §15.3.4.3 negative-level marker).
    ///
    /// This function writes the tag header + repo IDs; the
    /// caller layer writes each chunk via [`Self::write_chunk`] and
    /// closes the ValueType with [`Self::write_chunked_end`].
    ///
    pub fn write_chunked_value_tag(&mut self, repository_ids: &[&str]) {
        debug_assert!(
            !repository_ids.is_empty(),
            "Spec §15.3.4.2: list-of-repository-ids must contain >=1 entry"
        );
        self.inner.write_u32(0x7FFF_FF0A);
        let count: i32 = repository_ids.len() as i32;
        self.inner.write_u32(count as u32);
        for id in repository_ids {
            self.inner.write_string(id);
        }
    }

    /// Writes a single chunk within a chunked-encoded
    /// value (CDR §15.3.4.3): `i32 chunk_size` + raw bytes.
    ///
    pub fn write_chunk(&mut self, chunk_data: &[u8]) {
        let size: i32 = chunk_data.len() as i32;
        self.inner.write_u32(size as u32);
        for b in chunk_data {
            self.inner.write_u8(*b);
        }
    }

    /// Writes the end-tag of a chunked-encoded value (Spec
    /// §15.3.4.3 — `i32 end_tag = -<nesting_level>` with
    /// `nesting_level >= 1` for the outermost value).
    ///
    pub fn write_chunked_end(&mut self, nesting_level: u32) {
        debug_assert!(
            nesting_level >= 1,
            "Spec §15.3.4.3: chunked-end nesting_level must be >= 1"
        );
        let end_tag: i32 = -(nesting_level as i32);
        self.inner.write_u32(end_tag as u32);
    }
}

/// Stream API for reading valuetype wire bytes (CDR §15.3.4).
pub struct ValueStreamReader<'a, 'b> {
    /// Inner reader.
    pub inner: &'a mut Reader<'b>,
}

impl<'a, 'b> ValueStreamReader<'a, 'b> {
    /// Constructor.
    pub fn new(inner: &'a mut Reader<'b>) -> Self {
        Self { inner }
    }

    /// Reads a value-tag and returns the Repository ID.
    /// `Ok(None)` for null-value (`0x00000000` tag).
    ///
    /// The return is `Some(first_repo_id)` — for multi-repo-id lists
    /// the first ID is delivered; for the full list
    /// roundtrip see [`Self::read_value_tag_full`].
    ///
    /// # Errors
    /// Decode error (truncated, unknown tag type).
    pub fn read_value_tag(&mut self) -> ::core::result::Result<Option<String>, WireError> {
        let header = self.read_value_tag_full()?;
        Ok(match header {
            ValueTagHeader::Null => None,
            ValueTagHeader::Single(id) => Some(id),
            ValueTagHeader::List(ids) => ids.into_iter().next(),
            ValueTagHeader::ChunkedList(ids) => ids.into_iter().next(),
        })
    }

    /// Reads a value-tag with full header information — single,
    /// multi-repo-id list, or chunked variant.
    ///
    /// # Errors
    /// Decode error or unknown wire tag.
    pub fn read_value_tag_full(&mut self) -> ::core::result::Result<ValueTagHeader, WireError> {
        let tag = self.inner.read_u32()?;
        match tag {
            0x0000_0000 => Ok(ValueTagHeader::Null),
            0x7FFF_FF02 => {
                let id = self.inner.read_string()?;
                Ok(ValueTagHeader::Single(id))
            }
            0x7FFF_FF06 => {
                let count = self.inner.read_u32()? as usize;
                // Every Repository ID needs >= 4 bytes (its own CDR
                // string length prefix). Reject before
                // `Vec::with_capacity` — mirrors
                // `crates/cdr/src/composite.rs`'s
                // `len > reader.remaining()` guard.
                if count > self.inner.remaining() / 4 {
                    return Err(WireError::ImpossibleLength {
                        at: self.inner.position(),
                        claimed: count,
                        available: self.inner.remaining(),
                    });
                }
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(self.inner.read_string()?);
                }
                Ok(ValueTagHeader::List(ids))
            }
            0x7FFF_FF0A => {
                let count = self.inner.read_u32()? as usize;
                if count > self.inner.remaining() / 4 {
                    return Err(WireError::ImpossibleLength {
                        at: self.inner.position(),
                        claimed: count,
                        available: self.inner.remaining(),
                    });
                }
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(self.inner.read_string()?);
                }
                Ok(ValueTagHeader::ChunkedList(ids))
            }
            _ => Err(WireError::refused(
                self.inner.position(),
                "a value tag of 0, 0x7FFFFF02, 0x7FFFFF06 or 0x7FFFFF0A",
            )),
        }
    }

    /// Reads a chunk within a chunked-encoded value.
    /// The return is the chunk size in bytes (positive) or a
    /// negative end-tag (Spec §15.3.4.3 — `-nesting_level` marks
    /// the end of the chunked value).
    ///
    /// # Errors
    /// Decode error.
    pub fn read_chunk_size(&mut self) -> ::core::result::Result<i32, WireError> {
        let raw = self.inner.read_u32()?;
        Ok(raw as i32)
    }
}

/// Spec §15.3.4.2 — header variant of a value-tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueTagHeader {
    /// Null-value tag (`0x00000000`).
    Null,
    /// Single Repository ID (`0x7FFFFF02`).
    Single(String),
    /// List of Repository IDs (`0x7FFFFF06`) — Spec §15.3.4.2,
    /// used to enumerate truncation candidates.
    List(Vec<String>),
    /// Chunked encoding with a Repository ID list (`0x7FFFFF0A`) —
    /// Spec §15.3.4.3.
    ChunkedList(Vec<String>),
}

// ============================================================================
// §7.1 Component / Home — minimal CCM servant bindings (phase 1).
// ============================================================================

/// CCM container servant marker. Concrete component implementations
/// implement this plus the `ComponentHome` trait.
///
/// Phase 2 extends it with receptacle/facet/event bindings (CCM 4.0 §6).
pub trait ComponentServant: Servant {
    /// Returns the Repository ID of the component definition.
    fn component_repository_id(&self) -> &str;
}

/// CCM home trait. End-user homes implement this plus the specific
/// `create()` operation.
pub trait ComponentHome {
    /// Returns the Repository ID of the home.
    fn home_repository_id(&self) -> &str;
}

// ============================================================================
// §7.3 GIOP wire wiring — connection stub.
// ============================================================================

/// Connection handle used by stubs to send GIOP requests.
/// Phase 1 is an adapter trait whose full implementation lives in
/// `corba-iiop`.
pub trait CorbaConnection {
    /// Sends a GIOP request and blocks until a reply or
    /// system exception arrives.
    ///
    /// `target_ior` = object reference, `operation` = method name,
    /// `request_endianness` = on-wire byte order in which `request_payload`
    /// is CDR-encoded (set into the GIOP flag), `request_payload` =
    /// already-encoded body (classic CDR of the in-params).
    ///
    /// Returns the reply body **plus its** on-wire byte order — the
    /// server may reply in its native order (CDR "receiver makes it
    /// right"), so the stub must read the reply in the returned order.
    ///
    /// # Errors
    /// Wire error or server-side exception.
    fn invoke(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        request_payload: &[u8],
    ) -> Result<(Vec<u8>, Endianness), CorbaException>;

    /// Sends a oneway request (no reply expected).
    ///
    /// # Errors
    /// Wire error during the send.
    fn invoke_oneway(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        request_payload: &[u8],
    ) -> Result<(), CorbaException>;
}

/// Reply outcome of an asynchronous invocation (CORBA Messaging §22): the raw
/// reply body + its on-wire byte order on success, otherwise the CORBA exception.
pub type AsyncReply = Result<(Vec<u8>, Endianness), CorbaException>;

/// Callback for the AMI callback model (§22.5) — invoked exactly once.
pub type AsyncReplyCallback = alloc::boxed::Box<dyn FnOnce(AsyncReply) + Send>;

/// Abstract **asynchronous** GIOP channel to ONE target object (CORBA Messaging
/// §22) used by the generated AMI stubs (`sendc_`/`sendp_`). Implemented
/// by the transport layer (`zerodds-corba-interop::AmiClient`) — same
/// layering pattern as [`CorbaConnection`] for the synchronous side.
///
/// The stubs marshal the in-args into `payload` (big-endian) and decode the
/// reply in a typed way; this channel carries only opaque bytes + the
/// `request_id` demux.
pub trait AsyncCorbaChannel {
    /// **Callback model** (§22.5): fires `operation(payload)` and registers
    /// `cb` for the reply; returns the `request_id`.
    ///
    /// # Errors
    /// Transport error during sending.
    fn send(
        &mut self,
        operation: &str,
        payload: &[u8],
        cb: AsyncReplyCallback,
    ) -> Result<u32, CorbaException>;

    /// **Polling model** (§22.6): fires `operation(payload)`, returns the
    /// `request_id`; the reply is fetched via [`get_reply`](Self::get_reply).
    ///
    /// # Errors
    /// Transport error during sending.
    fn send_poll(&mut self, operation: &str, payload: &[u8]) -> Result<u32, CorbaException>;

    /// Drives the channel until the reply for `request_id` (from
    /// [`send_poll`](Self::send_poll)) is available, and returns it.
    ///
    /// # Errors
    /// Transport error, or `request_id` is not open.
    fn get_reply(&mut self, request_id: u32) -> Result<AsyncReply, CorbaException>;

    /// Reads EXACTLY one arrived reply (blocking) and dispatches it
    /// (callback invocation or storage); returns the handled `request_id`.
    ///
    /// # Errors
    /// Transport error, or nothing is open.
    fn perform_work(&mut self) -> Result<u32, CorbaException>;

    /// Drives [`perform_work`](Self::perform_work) until all callback requests
    /// are processed.
    ///
    /// # Errors
    /// Transport error.
    fn perform_all(&mut self) -> Result<(), CorbaException>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod value_stream_tests {
    use super::*;
    use zerodds_wire::{Endianness, Reader, Writer};

    // -------------------------------------------------------------
    // Buffer-cap hardening — a wire-supplied Repository-ID-list count
    // must be rejected cleanly (no OOM, no panic) before
    // `Vec::with_capacity` runs. Mirrors the established
    // `len > reader.remaining()` guard in
    // crates/cdr/src/composite.rs.
    // -------------------------------------------------------------

    #[test]
    fn value_tag_list_rejects_oversized_count() {
        let mut w = Writer::new(Endianness::Big);
        w.write_u32(0x7FFF_FF06); // list-of-repository-ids tag
        // Announce a huge id count, but supply no id bytes.
        w.write_u32(1_000_000_000);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let mut vsr = ValueStreamReader::new(&mut r);
        let res = vsr.read_value_tag_full();
        assert!(
            matches!(res, Err(WireError::ImpossibleLength { .. })),
            "expected clean rejection, got {res:?}"
        );
    }

    #[test]
    fn value_tag_chunked_list_rejects_oversized_count() {
        let mut w = Writer::new(Endianness::Big);
        w.write_u32(0x7FFF_FF0A); // chunked list-of-repository-ids tag
        w.write_u32(1_000_000_000);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let mut vsr = ValueStreamReader::new(&mut r);
        let res = vsr.read_value_tag_full();
        assert!(
            matches!(res, Err(WireError::ImpossibleLength { .. })),
            "expected clean rejection, got {res:?}"
        );
    }

    #[test]
    fn value_tag_list_within_bound_still_round_trips() {
        let mut w = Writer::new(Endianness::Big);
        let mut vsw = ValueStreamWriter::new(&mut w);
        vsw.write_value_tag_multi(&["IDL:demo/A:1.0", "IDL:demo/B:1.0"]);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let mut vsr = ValueStreamReader::new(&mut r);
        let header = vsr.read_value_tag_full().unwrap();
        assert_eq!(
            header,
            ValueTagHeader::List(alloc::vec![
                "IDL:demo/A:1.0".to_string(),
                "IDL:demo/B:1.0".to_string()
            ])
        );
    }
}
