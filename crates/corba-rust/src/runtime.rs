// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Runtime types that the generated code references.
//!
//! Phase 1 skeleton. The full wire implementation (GIOP marshalling +
//! IIOP connection) follows in phase 2 via `corba-iiop` and
//! `corba-giop`. Here live only the public API structures that the
//! emitted stub/skeleton/valuetype references.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// CORBA object reference (IOR-encoded). Generated stubs hold an
/// instance of this struct as a connection handle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectReference {
    /// Type identifier (Repository ID, e.g.
    /// `IDL:omg.org/MyInterface:1.0`).
    pub type_id: String,
    /// IIOP profile encapsulation (CDR, one `TAG_INTERNET_IOP` profile body).
    pub iiop_profile: Vec<u8>,
}

/// `TAG_INTERNET_IOP` — the only profile tag that ZeroDDS ObjectReferences
/// carry (CORBA §13.6.3).
const TAG_INTERNET_IOP: u32 = 0;

/// Wire marshalling of an object reference as an **IOR** (CORBA §15.3.3):
/// `struct IOR { string type_id; sequence<TaggedProfile> profiles; }` with
/// `TaggedProfile { ProfileId tag; sequence<octet> profile_data; }`.
/// `iiop_profile` is already the finished profile encapsulation, so it is
/// inserted as `sequence<octet>` (length + bytes). This is the format
/// in which omniORB/TAO/JacORB expect an `Object` reference on the wire.
impl zerodds_cdr::CdrEncode for ObjectReference {
    fn encode(
        &self,
        writer: &mut zerodds_cdr::BufferWriter,
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        writer.write_string(&self.type_id)?;
        writer.write_u32(1)?; // exactly one profile
        writer.write_u32(TAG_INTERNET_IOP)?;
        writer.write_u32(self.iiop_profile.len() as u32)?;
        writer.write_bytes(&self.iiop_profile)?;
        ::core::result::Result::Ok(())
    }
}

impl zerodds_cdr::CdrDecode for ObjectReference {
    fn decode(
        reader: &mut zerodds_cdr::BufferReader<'_>,
    ) -> ::core::result::Result<Self, zerodds_cdr::DecodeError> {
        let type_id = reader.read_string()?;
        let profile_count = reader.read_u32()?;
        // Take the first TAG_INTERNET_IOP profile, skip the rest
        // (multi-profile IORs are rare; the first IIOP profile suffices).
        let mut iiop_profile = Vec::new();
        for _ in 0..profile_count {
            let tag = reader.read_u32()?;
            let len = reader.read_u32()? as usize;
            let data = reader.read_bytes(len)?;
            if tag == TAG_INTERNET_IOP && iiop_profile.is_empty() {
                iiop_profile = data.to_vec();
            }
        }
        ::core::result::Result::Ok(Self {
            type_id,
            iiop_profile,
        })
    }
}

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
        endianness: zerodds_cdr::Endianness,
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

/// ValueBase trait. All IDL valuetypes extend this.
pub trait ValueBase {
    /// Repository ID of the valuetype (e.g. `IDL:omg.org/MyValue:1.0`).
    fn repository_id(&self) -> &str;
}

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

// ============================================================================
// §2.4 TypeCode (CORBA 3.3 §10.7) — minimally functional wrapper.
// ============================================================================

/// CORBA `TypeCode` — type-reflection wrapper.
///
/// Full TypeCode operations (kind, member_count, member_name,
/// content_type, ...) are evaluated via the `corba-iiop` IIOP connection —
/// phase 1 here is a wrapper around the Repository ID,
/// phase 2 extends it with a lookup API.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeCode {
    /// Repository ID of the referenced type.
    pub repository_id: String,
}

impl TypeCode {
    /// Constructs a TypeCode from a Repository ID.
    #[must_use]
    pub fn new(repository_id: impl Into<String>) -> Self {
        Self {
            repository_id: repository_id.into(),
        }
    }
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
/// Phase 1: API skeleton that wraps `zerodds_cdr::BufferReader/Writer` and
/// applies the value-tag logic (chunked encoding, repository-id list).
/// Phase 2 wires the full spec conformance (truncatable, custom
/// marshaling, codeset).
pub struct ValueStreamWriter<'a> {
    /// Inner writer.
    pub inner: &'a mut zerodds_cdr::BufferWriter,
}

impl<'a> ValueStreamWriter<'a> {
    /// Constructor.
    pub fn new(inner: &'a mut zerodds_cdr::BufferWriter) -> Self {
        Self { inner }
    }

    /// Writes a value-tag (CDR §15.3.4.2). Format: 0x7FFFFF02
    /// for single-repository-id, 0x7FFFFF06 for list-of-repository-
    /// ids, 0x00000000 for null-value.
    ///
    /// # Errors
    /// Encode error.
    pub fn write_value_tag(
        &mut self,
        repository_id: &str,
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        // Single-repository-id tag (chunked = false, codeset = false).
        self.inner.write_u32(0x7FFF_FF02)?;
        self.inner.write_string(repository_id)?;
        Ok(())
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
    /// # Errors
    /// Encode error or empty `repository_ids` (spec requires N >= 1).
    pub fn write_value_tag_multi(
        &mut self,
        repository_ids: &[&str],
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        debug_assert!(
            !repository_ids.is_empty(),
            "Spec §15.3.4.2: list-of-repository-ids must contain >=1 entry"
        );
        self.inner.write_u32(0x7FFF_FF06)?;
        // count as signed long.
        let count: i32 = repository_ids.len() as i32;
        self.inner.write_u32(count as u32)?;
        for id in repository_ids {
            self.inner.write_string(id)?;
        }
        Ok(())
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
    /// # Errors
    /// Encode error or empty repo IDs.
    pub fn write_chunked_value_tag(
        &mut self,
        repository_ids: &[&str],
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        debug_assert!(
            !repository_ids.is_empty(),
            "Spec §15.3.4.2: list-of-repository-ids must contain >=1 entry"
        );
        self.inner.write_u32(0x7FFF_FF0A)?;
        let count: i32 = repository_ids.len() as i32;
        self.inner.write_u32(count as u32)?;
        for id in repository_ids {
            self.inner.write_string(id)?;
        }
        Ok(())
    }

    /// Writes a single chunk within a chunked-encoded
    /// value (CDR §15.3.4.3): `i32 chunk_size` + raw bytes.
    ///
    /// # Errors
    /// Encode error.
    pub fn write_chunk(
        &mut self,
        chunk_data: &[u8],
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        let size: i32 = chunk_data.len() as i32;
        self.inner.write_u32(size as u32)?;
        for b in chunk_data {
            self.inner.write_u8(*b)?;
        }
        Ok(())
    }

    /// Writes the end-tag of a chunked-encoded value (Spec
    /// §15.3.4.3 — `i32 end_tag = -<nesting_level>` with
    /// `nesting_level >= 1` for the outermost value).
    ///
    /// # Errors
    /// Encode error or `nesting_level == 0`.
    pub fn write_chunked_end(
        &mut self,
        nesting_level: u32,
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        debug_assert!(
            nesting_level >= 1,
            "Spec §15.3.4.3: chunked-end nesting_level must be >= 1"
        );
        let end_tag: i32 = -(nesting_level as i32);
        self.inner.write_u32(end_tag as u32)?;
        Ok(())
    }
}

/// Stream API for reading valuetype wire bytes (CDR §15.3.4).
pub struct ValueStreamReader<'a, 'b> {
    /// Inner reader.
    pub inner: &'a mut zerodds_cdr::BufferReader<'b>,
}

impl<'a, 'b> ValueStreamReader<'a, 'b> {
    /// Constructor.
    pub fn new(inner: &'a mut zerodds_cdr::BufferReader<'b>) -> Self {
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
    pub fn read_value_tag(
        &mut self,
    ) -> ::core::result::Result<Option<String>, zerodds_cdr::DecodeError> {
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
    pub fn read_value_tag_full(
        &mut self,
    ) -> ::core::result::Result<ValueTagHeader, zerodds_cdr::DecodeError> {
        let tag = self.inner.read_u32()?;
        match tag {
            0x0000_0000 => Ok(ValueTagHeader::Null),
            0x7FFF_FF02 => {
                let id = self.inner.read_string()?;
                Ok(ValueTagHeader::Single(id))
            }
            0x7FFF_FF06 => {
                let count = self.inner.read_u32()? as usize;
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(self.inner.read_string()?);
                }
                Ok(ValueTagHeader::List(ids))
            }
            0x7FFF_FF0A => {
                let count = self.inner.read_u32()? as usize;
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(self.inner.read_string()?);
                }
                Ok(ValueTagHeader::ChunkedList(ids))
            }
            _ => Err(zerodds_cdr::DecodeError::InvalidString {
                offset: 0,
                reason: "ValueStream: unsupported value-tag (only 0/0x7FFFFF02/0x7FFFFF06/0x7FFFFF0A)",
            }),
        }
    }

    /// Reads a chunk within a chunked-encoded value.
    /// The return is the chunk size in bytes (positive) or a
    /// negative end-tag (Spec §15.3.4.3 — `-nesting_level` marks
    /// the end of the chunked value).
    ///
    /// # Errors
    /// Decode error.
    pub fn read_chunk_size(&mut self) -> ::core::result::Result<i32, zerodds_cdr::DecodeError> {
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
        request_endianness: zerodds_cdr::Endianness,
        request_payload: &[u8],
    ) -> Result<(Vec<u8>, zerodds_cdr::Endianness), CorbaException>;

    /// Sends a oneway request (no reply expected).
    ///
    /// # Errors
    /// Wire error during the send.
    fn invoke_oneway(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: zerodds_cdr::Endianness,
        request_payload: &[u8],
    ) -> Result<(), CorbaException>;
}

/// Reply outcome of an asynchronous invocation (CORBA Messaging §22): the raw
/// reply body + its on-wire byte order on success, otherwise the CORBA exception.
pub type AsyncReply = Result<(Vec<u8>, zerodds_cdr::Endianness), CorbaException>;

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
mod object_reference_tests {
    use super::*;
    use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};

    #[test]
    fn object_reference_roundtrips_as_ior() {
        let obj = ObjectReference {
            type_id: "IDL:demo/Bench:1.0".into(),
            iiop_profile: alloc::vec![0x01, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        obj.encode(&mut w).unwrap();
        let bytes = w.into_bytes();

        // Wire structure (CORBA §15.3.3 IOR): string type_id + seq<TaggedProfile>.
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(r.read_string().unwrap(), "IDL:demo/Bench:1.0");
        assert_eq!(r.read_u32().unwrap(), 1, "exactly one profile");
        assert_eq!(r.read_u32().unwrap(), 0, "TAG_INTERNET_IOP");
        assert_eq!(r.read_u32().unwrap(), 8, "profile_data length");

        // Full roundtrip.
        let mut r2 = BufferReader::new(&bytes, Endianness::Big);
        let decoded = ObjectReference::decode(&mut r2).unwrap();
        assert_eq!(decoded, obj);
    }

    #[test]
    fn object_reference_little_endian_roundtrips() {
        let obj = ObjectReference {
            type_id: "IDL:X:1.0".into(),
            iiop_profile: alloc::vec![1, 2, 3],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        obj.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ObjectReference::decode(&mut r).unwrap(), obj);
    }
}
