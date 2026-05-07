// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Runtime-Types die der generierte Code referenziert.
//!
//! Phase-1-Skelett. Volle Wire-Implementation (GIOP-Marshalling +
//! IIOP-Connection) folgt in Phase-2 ueber `corba-iiop` und
//! `corba-giop`. Hier liegen nur die Public-API-Strukturen die der
//! emittierte Stub/Skeleton/Valuetype referenziert.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// CORBA-Object-Reference (IOR-encoded). Generierte Stubs halten eine
/// Instanz dieser Struct als Connection-Handle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectReference {
    /// Type-Identifier (Repository-ID, z.B.
    /// `IDL:omg.org/MyInterface:1.0`).
    pub type_id: String,
    /// IIOP-Profile-Bytes (CDR-encoded). Phase-1 ist das opaque;
    /// Phase-2 nutzt corba-iiop fuer die echte Connection.
    pub iiop_profile: Vec<u8>,
}

/// CORBA-System-/User-Exception. Generierte Stubs/Skeletons mappen
/// alle Fehlerpfade auf diese Variant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CorbaException {
    /// CORBA-System-Exception (Spec §3.17.1). Minor-Code laut OMG.
    SystemException {
        /// Minor-Code aus dem Spec-Standard (z.B. `0x4F4D000B` =
        /// `BAD_OPERATION`).
        minor: u32,
        /// Statische Fehler-Beschreibung.
        message: &'static str,
    },
    /// User-Exception aus IDL `exception E { ... };`. Wir tragen den
    /// Repository-ID + Payload-Bytes (Phase-1-MVP).
    UserException {
        /// Repository-ID des Exception-Types (z.B.
        /// `IDL:omg.org/MyError:1.0`).
        repository_id: String,
        /// Payload-Bytes (XCDR-encoded).
        payload: Vec<u8>,
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

/// ValueBase-Trait. Alle IDL-Valuetypes erweitern diesen.
pub trait ValueBase {
    /// Repository-ID des Valuetypes (z.B. `IDL:omg.org/MyValue:1.0`).
    fn repository_id(&self) -> &str;
}

/// Result eines Skeleton-Dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkeletonResult {
    /// Operation erfolgreich, Reply-Bytes liegen vor.
    Reply(Vec<u8>),
    /// Operation hat eine User- oder System-Exception geworfen.
    Exception(CorbaException),
    /// Unbekannter Operation-Name fuer das Interface — der POA
    /// muss BAD_OPERATION zurueckgeben.
    BadOperation,
    /// Operation ist deklariert aber Wire-Marshalling ist Phase-2 —
    /// vorerst Platzhalter.
    NotYetWired,
}

/// POA-Servant-Marker. Concrete Server-Implementierungen impl-en das
/// plus den jeweiligen IDL-Interface-Trait.
pub trait Servant {
    /// Repository-ID des Interface, das der Servant implementiert.
    fn target_repository_id(&self) -> &str;
}

// ============================================================================
// §2.4 TypeCode (CORBA 3.3 §10.7) — minimal-funktionaler Wrapper.
// ============================================================================

/// CORBA `TypeCode` — Type-Reflection-Wrapper.
///
/// Vollstaendige TypeCode-Operationen (kind, member_count, member_name,
/// content_type, ...) werden ueber `corba-iiop`-IIOP-Connection
/// ausgewertet — Phase-1 ist hier ein Wrapper um den Repository-ID,
/// Phase-2 erweitert mit Lookup-API.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeCode {
    /// Repository-ID des referenzierten Types.
    pub repository_id: String,
}

impl TypeCode {
    /// Konstruiert einen TypeCode aus einem Repository-ID.
    #[must_use]
    pub fn new(repository_id: impl Into<String>) -> Self {
        Self {
            repository_id: repository_id.into(),
        }
    }
}

// ============================================================================
// §7.2 POA-Configuration-Builder (Spec §11.3 — 7 Policies)
// ============================================================================

/// CORBA-POA-Builder fuer Server-Side-Object-Aktivierung.
///
/// Spec §11.3 definiert 7 Policies; alle haben sinnvolle Defaults
/// fuer typische Embedded-/Server-Use-Cases. End-User
/// ueberschreibt selektiv via `with_*`-Methoden und ruft `build()`
/// fuer den finalen `PoaConfiguration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoaBuilder {
    /// Threading-Policy (Spec §11.3.4).
    pub thread_policy: ThreadPolicy,
    /// Lifespan-Policy (§11.3.5).
    pub lifespan_policy: LifespanPolicy,
    /// IdAssignment-Policy (§11.3.6).
    pub id_assignment_policy: IdAssignmentPolicy,
    /// IdUniqueness-Policy (§11.3.7).
    pub id_uniqueness_policy: IdUniquenessPolicy,
    /// ImplicitActivation-Policy (§11.3.8).
    pub implicit_activation_policy: ImplicitActivationPolicy,
    /// ServantRetention-Policy (§11.3.9).
    pub servant_retention_policy: ServantRetentionPolicy,
    /// RequestProcessing-Policy (§11.3.10).
    pub request_processing_policy: RequestProcessingPolicy,
}

impl Default for PoaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PoaBuilder {
    /// Erzeugt einen neuen POA-Builder mit Spec-Default-Policies.
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

/// POA-Threading-Policy (Spec §11.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadPolicy {
    /// Default: ORB waehlt Threading.
    OrbCtrlModel,
    /// Single-Threaded.
    SingleThreadModel,
    /// Main-Thread-Only.
    MainThreadModel,
}

/// POA-Lifespan-Policy (Spec §11.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifespanPolicy {
    /// Default: Object-Refs gelten nur fuer ORB-Lifetime.
    Transient,
    /// Object-Refs ueberleben ORB-Restart.
    Persistent,
}

/// POA-IdAssignment-Policy (§11.3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdAssignmentPolicy {
    /// Default: ORB vergibt Object-IDs.
    SystemId,
    /// Anwendung vergibt Object-IDs.
    UserId,
}

/// POA-IdUniqueness-Policy (§11.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdUniquenessPolicy {
    /// Default: jeder Servant hat genau eine Object-ID.
    Unique,
    /// Servant kann mehrere Object-IDs haben.
    Multiple,
}

/// POA-ImplicitActivation-Policy (§11.3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitActivationPolicy {
    /// Default: explicit `activate_object` Pflicht.
    NoImplicitActivation,
    /// Auto-activation bei ersten Method-Call.
    ImplicitActivation,
}

/// POA-ServantRetention-Policy (§11.3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServantRetentionPolicy {
    /// Default: ActiveObjectMap haelt Servants.
    Retain,
    /// Stateless — pro Request frischer Servant.
    NonRetain,
}

/// POA-RequestProcessing-Policy (§11.3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestProcessingPolicy {
    /// Default: ActiveObjectMap-Lookup, sonst BAD_OPERATION.
    UseActiveObjectMapOnly,
    /// Lookup, dann ServantManager-Fallback.
    UseDefaultServant,
    /// Lookup, dann ServantActivator/-Locator.
    UseServantManager,
}

// ============================================================================
// §4.4 Valuetype Wire-Marshalling — minimaler Stream-API
// ============================================================================

/// Stream-API fuer ValueBase-Wire-Marshalling (CDR §15.3.4).
///
/// Phase-1: API-Skelett, das `zerodds_cdr::BufferReader/Writer` wraped und
/// die value-tag-Logic anwendet (chunked encoding, repository-id-list).
/// Phase-2 wired die volle Spec-Konformitaet (truncatable, custom
/// marshaling, codeset).
pub struct ValueStreamWriter<'a> {
    /// Inner-Writer.
    pub inner: &'a mut zerodds_cdr::BufferWriter,
}

impl<'a> ValueStreamWriter<'a> {
    /// Konstruktor.
    pub fn new(inner: &'a mut zerodds_cdr::BufferWriter) -> Self {
        Self { inner }
    }

    /// Schreibt einen value-tag (CDR §15.3.4.2). Format: 0x7FFFFF02
    /// fuer single-repository-id, 0x7FFFFF06 fuer list-of-repository-
    /// ids, 0x00000000 fuer null-value.
    ///
    /// # Errors
    /// Encode-Fehler.
    pub fn write_value_tag(
        &mut self,
        repository_id: &str,
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        // Single-Repository-ID-Tag (chunked = false, codeset = false).
        self.inner.write_u32(0x7FFF_FF02)?;
        self.inner.write_string(repository_id)?;
        Ok(())
    }

    /// Schreibt einen value-tag mit multi-repository-id-Liste (CDR
    /// §15.3.4.2). Wire-Tag = 0x7FFF_FF06 (`list-of-repository-ids` +
    /// kein Chunked-Bit). Layout:
    ///
    /// ```text
    /// u32 tag = 0x7FFFFF06
    /// i32 count                 // > 0
    /// string id[0] .. id[N-1]   // CDR-strings (length + bytes + NUL)
    /// ```
    ///
    /// # Errors
    /// Encode-Fehler oder leere `repository_ids` (Spec verlangt N >= 1).
    pub fn write_value_tag_multi(
        &mut self,
        repository_ids: &[&str],
    ) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {
        debug_assert!(
            !repository_ids.is_empty(),
            "Spec §15.3.4.2: list-of-repository-ids must contain >=1 entry"
        );
        self.inner.write_u32(0x7FFF_FF06)?;
        // count als signed long.
        let count: i32 = repository_ids.len() as i32;
        self.inner.write_u32(count as u32)?;
        for id in repository_ids {
            self.inner.write_string(id)?;
        }
        Ok(())
    }

    /// Schreibt einen Chunked-Value-Tag (CDR §15.3.4.2 + §15.3.4.3 —
    /// "chunked encoding"). Wire-Tag = 0x7FFF_FF0A
    /// (chunked-flag + multi-repo-id-flag, beide gesetzt).
    ///
    /// Nach dem Tag folgen:
    /// 1. `i32 count` + `string id[N]` (multi-repo-id-list).
    /// 2. Pro Chunk: `i32 chunk_size_bytes` + `octet chunk_data[..]`.
    /// 3. Zum Schluss: `i32 end_tag = -<nesting_level>` (Spec
    ///    §15.3.4.3 negative-Level-Marker).
    ///
    /// Diese Funktion schreibt den Tag-Header + Repo-IDs; die
    /// Caller-Layer schreibt jeden Chunk via [`Self::write_chunk`] und
    /// schliesst den ValueType mit [`Self::write_chunked_end`].
    ///
    /// # Errors
    /// Encode-Fehler oder leere Repo-IDs.
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

    /// Schreibt einen einzelnen Chunk innerhalb eines chunked-encoded
    /// Value (CDR §15.3.4.3): `i32 chunk_size` + raw bytes.
    ///
    /// # Errors
    /// Encode-Fehler.
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

    /// Schreibt den End-Tag eines chunked-encoded Value (Spec
    /// §15.3.4.3 — `i32 end_tag = -<nesting_level>` mit
    /// `nesting_level >= 1` fuer den outermost Value).
    ///
    /// # Errors
    /// Encode-Fehler oder `nesting_level == 0`.
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

/// Stream-API zum Lesen von Valuetype-Wire-Bytes (CDR §15.3.4).
pub struct ValueStreamReader<'a, 'b> {
    /// Inner-Reader.
    pub inner: &'a mut zerodds_cdr::BufferReader<'b>,
}

impl<'a, 'b> ValueStreamReader<'a, 'b> {
    /// Konstruktor.
    pub fn new(inner: &'a mut zerodds_cdr::BufferReader<'b>) -> Self {
        Self { inner }
    }

    /// Liest einen value-tag und gibt die Repository-ID zurueck.
    /// `Ok(None)` bei null-value (`0x00000000`-Tag).
    ///
    /// Rueckgabe ist `Some(first_repo_id)` — bei multi-repo-id-Listen
    /// wird die erste ID geliefert; fuer den vollstaendigen Listen-
    /// Roundtrip siehe [`Self::read_value_tag_full`].
    ///
    /// # Errors
    /// Decode-Fehler (truncated, unbekannter Tag-Typ).
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

    /// Liest einen value-tag mit voller Header-Information — single,
    /// multi-repo-id-Liste oder chunked-Variante.
    ///
    /// # Errors
    /// Decode-Fehler oder unbekannter Wire-Tag.
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

    /// Liest einen Chunk innerhalb eines chunked-encoded Values.
    /// Rueckgabe ist die Chunk-Groesse in Bytes (positiv) bzw. ein
    /// negativer End-Tag (Spec §15.3.4.3 — `-nesting_level` markiert
    /// das Ende des chunked-Values).
    ///
    /// # Errors
    /// Decode-Fehler.
    pub fn read_chunk_size(&mut self) -> ::core::result::Result<i32, zerodds_cdr::DecodeError> {
        let raw = self.inner.read_u32()?;
        Ok(raw as i32)
    }
}

/// Spec §15.3.4.2 — Header-Variante eines value-tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueTagHeader {
    /// Null-Value-Tag (`0x00000000`).
    Null,
    /// Single-Repository-ID (`0x7FFFFF02`).
    Single(String),
    /// Liste von Repository-IDs (`0x7FFFFF06`) — Spec §15.3.4.2,
    /// dient der Truncation-Kandidaten-Auflistung.
    List(Vec<String>),
    /// Chunked-Encoding mit Repository-ID-Liste (`0x7FFFFF0A`) —
    /// Spec §15.3.4.3.
    ChunkedList(Vec<String>),
}

// ============================================================================
// §7.1 Component / Home — minimale CCM-Servant-Bindings (Phase-1).
// ============================================================================

/// CCM-Container-Servant-Marker. Concrete Component-Implementations
/// impl-en das plus den `ComponentHome`-Trait.
///
/// Phase-2 erweitert mit Receptacle-/Facet-/Event-Bindings (CCM 4.0 §6).
pub trait ComponentServant: Servant {
    /// Liefert die Repository-ID der Component-Definition.
    fn component_repository_id(&self) -> &str;
}

/// CCM-Home-Trait. End-User-Homes impl-en das plus die spezifische
/// `create()`-Operation.
pub trait ComponentHome {
    /// Liefert die Repository-ID des Home.
    fn home_repository_id(&self) -> &str;
}

// ============================================================================
// §7.3 GIOP-Wire-Wiring — Connection-Stub.
// ============================================================================

/// Connection-Handle, das von Stubs zur GIOP-Request-Versendung
/// verwendet wird. Phase-1 ist das ein Adapter-Trait, dessen volle
/// Implementation in `corba-iiop` lebt.
pub trait CorbaConnection {
    /// Sendet einen GIOP-Request und blockiert bis Reply oder
    /// System-Exception kommt.
    ///
    /// `target_ior` = Object-Reference, `operation` = Method-Name,
    /// `request_payload` = bereits encoder Body (DataType-CDR).
    ///
    /// # Errors
    /// Wire-Fehler oder Server-Side-Exception.
    fn invoke(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_payload: &[u8],
    ) -> Result<Vec<u8>, CorbaException>;

    /// Sendet einen oneway-Request (kein Reply erwartet).
    ///
    /// # Errors
    /// Wire-Fehler waehrend des Send.
    fn invoke_oneway(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_payload: &[u8],
    ) -> Result<(), CorbaException>;
}
