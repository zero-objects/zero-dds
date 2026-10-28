// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `impl DdsBackend for Gateway` — Spec §7.4, backed by `zerodds-dcps`'s
//! public typed API.
//!
//! One [`Gateway`] per daemon process. It holds a tree that mirrors the
//! WebDDS object model: applications, each with `DomainParticipant`s,
//! each with topics/publishers/subscribers, each publisher/subscriber
//! with `DataWriter`/`DataReader`s. Names the trait's caller does not
//! supply (a participant, a publisher, a subscriber, a writer, a
//! reader) are generated here and returned in
//! `BackendResult::Created`'s path, the way `WebDdsRoot::
//! create_application` returns a generated `SessionId` — the caller
//! addresses the entity by that path in every later call.
//!
//! Sample bodies are always the DDS-WEB §8.1 XML representation
//! (`dynxml`), converted to/from wire bytes via `zerodds_wire_codec`
//! directly (the same encode/decode the type's `SerializedTypeSupport`
//! uses internally) plus `SerializedTypeSupport::read_body` for
//! validation and key-hash derivation before a write.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_dcps::serialized_type::SerializedSample;
use zerodds_dcps::topic::Topic;
use zerodds_dcps::{
    Condition, DataReader, DataReaderQos, DataWriter, DataWriterQos, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, Entity, Publisher, PublisherQos, ReadCondition,
    Subscriber, SubscriberQos, TopicQos, WaitSet,
};
use zerodds_web::bridge::{BackendError, BackendResult, DdsBackend};
use zerodds_wire_codec::ProbeRequest;

use crate::dynxml;
use crate::qos_update::EntityKind;
use crate::types_registry::{ResolvedType, TypeRegistry};

/// Always-used sample representation: XCDR2, little-endian. Consistent
/// throughout the gateway; a peer that needs a different one is a
/// per-writer/-reader `data_representation_offer` override this crate
/// does not (yet) expose through the REST/SOAP surface. The CDR dialect is
/// the one `resolved`'s support carries, its participant's
/// `wire.codec_dialect`.
fn probe_request(resolved: &ResolvedType) -> ProbeRequest {
    ProbeRequest::in_dialect(
        &resolved.dynamic_type.qualified_name(),
        Encoding::PlainCdr2,
        Endianness::Little,
        resolved.support.dialect(),
    )
}

/// Bound on automatic TypeLookup discovery in
/// [`Gateway::resolve_type_for_topic`] — an HTTP/SOAP request waits at
/// most this long for a `getTypes` round trip before the response names
/// the type as unresolved (never silently — see `route_to_outcome`'s
/// `BadRequest` mapping).
const TYPE_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

struct WriterEntry {
    writer: DataWriter<SerializedSample>,
    resolved: ResolvedType,
}

struct ReaderEntry {
    reader: Arc<DataReader<SerializedSample>>,
    resolved: ResolvedType,
}

struct PublisherEntry {
    publisher: Publisher,
    writers: BTreeMap<String, WriterEntry>,
}

struct SubscriberEntry {
    subscriber: Subscriber,
    readers: BTreeMap<String, ReaderEntry>,
}

struct ParticipantEntry {
    participant: DomainParticipant,
    topics: BTreeMap<String, Topic<SerializedSample>>,
    publishers: BTreeMap<String, PublisherEntry>,
    subscribers: BTreeMap<String, SubscriberEntry>,
}

struct WaitsetEntry {
    waitset: WaitSet,
    /// Reader path -> the `Arc<ReadCondition>` attached for it, so a
    /// triggered condition (returned by identity from `WaitSet::wait`)
    /// maps back to the reader path the caller asked to name.
    conditions: Vec<(String, Arc<ReadCondition>)>,
    /// One HTTP client waits at a time: a DCPS `WaitSet` refuses a second
    /// concurrent `wait` with PRECONDITION_NOT_MET (DDS 1.4
    /// §2.2.2.1.6), so a second client waits here for the first.
    turn: Mutex<()>,
}

/// The default long-poll of a waitset when the caller names no timeout.
pub const DEFAULT_WAITSET_TIMEOUT: Duration = Duration::from_secs(10);

/// A waitset taken out of the gateway, to wait on without holding the
/// gateway: a long-poll held it before, and every other request waited
/// for it (measured on the Linux test host: a list of applications took 3.7 s during
/// a 4 s long-poll, 0.2 ms without).
pub struct WaitsetHandle(Arc<WaitsetEntry>);

impl WaitsetHandle {
    /// Waits up to `timeout` for a condition of the waitset and lists the
    /// reader paths whose condition triggered; empty when the timeout
    /// elapsed.
    ///
    /// # Errors
    /// [`BackendError::Internal`] when the DCPS wait fails otherwise.
    pub fn wait(&self, timeout: Duration) -> Result<BackendResult, BackendError> {
        let entry = &self.0;
        let _turn = entry
            .turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let triggered = match entry.waitset.wait(timeout) {
            Ok(v) => v,
            Err(zerodds_dcps::DdsError::Timeout) => Vec::new(),
            Err(e) => return Err(BackendError::Internal(format!("{e:?}"))),
        };
        let mut paths = Vec::new();
        for cond in &triggered {
            for (path, rc) in &entry.conditions {
                if Arc::ptr_eq(&(Arc::clone(rc) as Arc<dyn Condition>), cond) {
                    paths.push(path.clone());
                }
            }
        }
        Ok(BackendResult::List(paths))
    }
}

/// A caller's long-poll timeout in seconds; none is
/// [`DEFAULT_WAITSET_TIMEOUT`].
///
/// # Errors
/// [`BackendError::BadRequest`] for text that is not a finite, non-negative
/// number of seconds a `Duration` holds. `Duration::from_secs_f64` panicked
/// on `-1`, `nan` and `1e300` (measured on the Linux test host: the connection's thread
/// ended without a response), and `abc` silently waited the default.
pub fn waitset_timeout(text: Option<&str>) -> Result<Duration, BackendError> {
    let Some(text) = text else {
        return Ok(DEFAULT_WAITSET_TIMEOUT);
    };
    text.trim()
        .parse::<f64>()
        .ok()
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
        .ok_or_else(|| {
            BackendError::BadRequest(format!("timeout `{text}` is not a number of seconds"))
        })
}

/// A participant's `domain_id` as a request carries it; none is domain 0.
///
/// # Errors
/// [`BackendError::BadRequest`] for text that is not a domain id. It was
/// domain 0 before, whatever the text said, and a value above `i32::MAX`
/// wrapped to a negative id.
pub fn domain_id_of(text: Option<&str>) -> Result<u32, BackendError> {
    let Some(text) = text else {
        return Ok(0);
    };
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|id| i32::try_from(*id).is_ok())
        .ok_or_else(|| BackendError::BadRequest(format!("domain_id `{text}` is not a domain id")))
}

#[derive(Default)]
struct AppEntry {
    participants: BTreeMap<String, ParticipantEntry>,
    waitsets: BTreeMap<String, Arc<WaitsetEntry>>,
    next_participant: u64,
    next_publisher: u64,
    next_subscriber: u64,
    next_writer: u64,
    next_reader: u64,
}

/// Where a config-named type's definition lives.
#[derive(Debug, Clone)]
pub enum TypeSource {
    /// `types[].idl_file` — `TypeRegistry::load_idl_file`.
    Idl(PathBuf),
    /// `types[].xml_file` — a DDS-XML 1.0 `<types>` document,
    /// `TypeRegistry::load_xml_file`.
    Xml(PathBuf),
}

/// The DCPS-backed gateway. `Arc<Gateway>` is shared between the REST
/// and SOAP dispatchers of one daemon process.
pub struct Gateway {
    apps: Mutex<BTreeMap<String, AppEntry>>,
    types: Arc<TypeRegistry>,
    /// `type_name -> source`, from the daemon config's `types:`
    /// section — resolved lazily on first use of that type name.
    type_sources: BTreeMap<String, TypeSource>,
    /// The daemon's §7.3.1 `Root` metadata store (QosLibrary/QosProfile/
    /// Type) — not a DCPS entity, so not part of `DdsBackend` itself
    /// (see `crate::metadata`'s module doc), but `update_*`'s
    /// `<qos_profile>library::profile</qos_profile>` reference
    /// (Spec SS7.4.4.4) resolves through it, so `Gateway` holds a
    /// handle rather than needing a `DdsBackend` signature change.
    metadata: Arc<crate::metadata::MetadataStore>,
}

impl Gateway {
    /// New, empty gateway. `type_sources` names every type this gateway
    /// can resolve from a config-named IDL or DDS-XML file; a type not
    /// in it must already be resolvable via
    /// [`crate::types_registry::TypeRegistry::resolve_discovered`] (or
    /// via automatic TypeLookup — see `Self::resolve_type_for_topic`)
    /// before `create_topic` is asked for it. `metadata` is the same
    /// store `rest_service`/`soap_service` dispatch QosLibrary/
    /// QosProfile/Type CRUD against.
    #[must_use]
    pub fn new(
        type_sources: BTreeMap<String, TypeSource>,
        metadata: Arc<crate::metadata::MetadataStore>,
    ) -> Self {
        Self {
            apps: Mutex::new(BTreeMap::new()),
            types: Arc::new(TypeRegistry::new()),
            type_sources,
            metadata,
        }
    }

    /// The shared type registry — the daemon's TypeLookup poll loop
    /// (`daemon::server`) resolves discovered types into it directly.
    #[must_use]
    pub fn types(&self) -> &Arc<TypeRegistry> {
        &self.types
    }

    /// Resolves `type_name` for a `create_topic` call naming `topic_name`:
    /// the cache, then a config-named IDL/XML file, then — only when
    /// neither answers — automatic TypeLookup discovery bounded by
    /// [`TYPE_LOOKUP_TIMEOUT`] (`TypeRegistry::resolve_named`), which
    /// needs `participant`'s live `DcpsRuntime` to issue `getTypes`
    /// against a peer SEDP already discovered for this topic.
    ///
    /// The support comes back in `participant`'s CDR dialect
    /// (`wire.codec_dialect`), so the samples this gateway writes and reads
    /// for the type are encoded and decoded in it.
    fn resolve_type_for_topic(
        &self,
        type_name: &str,
        topic_name: &str,
        participant: &DomainParticipant,
    ) -> Result<ResolvedType, BackendError> {
        let dialect = participant
            .runtime()
            .map(|rt| rt.codec_dialect())
            .unwrap_or_default();
        self.resolve_type_in_registry(type_name, topic_name, participant)
            .map(|resolved| ResolvedType {
                support: resolved.support.in_dialect(dialect),
                ..resolved
            })
    }

    fn resolve_type_in_registry(
        &self,
        type_name: &str,
        topic_name: &str,
        participant: &DomainParticipant,
    ) -> Result<ResolvedType, BackendError> {
        if let Some(r) = self.types.get(type_name) {
            return Ok(r);
        }
        if let Some(source) = self.type_sources.get(type_name) {
            return match source {
                TypeSource::Idl(path) => self.types.load_idl_file(path, type_name),
                TypeSource::Xml(path) => self.types.load_xml_file(path, type_name),
            }
            .map_err(|e| BackendError::Internal(e.to_string()));
        }
        let Some(runtime) = participant.runtime() else {
            return Err(BackendError::BadRequest(format!(
                "type `{type_name}` is not in this daemon's config, and this participant has \
                 no live runtime to discover it via TypeLookup"
            )));
        };
        self.types
            .resolve_named(runtime, topic_name, type_name, TYPE_LOOKUP_TIMEOUT)
            .map_err(|e| {
                BackendError::BadRequest(format!(
                    "type `{type_name}` for topic `{topic_name}`: {e}"
                ))
            })
    }

    /// The waitset `waitset` of application `app_name`, to wait on after
    /// the caller let go of the gateway.
    ///
    /// # Errors
    /// [`BackendError::NotFound`] for an unknown application or waitset.
    pub fn waitset_handle(
        &self,
        app_name: &str,
        waitset: &str,
    ) -> Result<WaitsetHandle, BackendError> {
        self.with_apps(|apps| {
            let app = apps
                .get(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            let entry = app
                .waitsets
                .get(waitset)
                .ok_or_else(|| BackendError::NotFound(waitset.to_string()))?;
            Ok(WaitsetHandle(Arc::clone(entry)))
        })
    }

    fn with_apps<R>(&self, f: impl FnOnce(&mut BTreeMap<String, AppEntry>) -> R) -> R {
        let mut apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut apps)
    }
}

/// Encodes a `DynamicData` sample of `resolved`'s type to wire bytes and
/// validates it into a `SerializedSample`, ready for `DataWriter::write`.
fn encode_sample(resolved: &ResolvedType, xml: &str) -> Result<SerializedSample, BackendError> {
    let value = dynxml::from_xml_document(xml, &resolved.dynamic_type)
        .map_err(|e| BackendError::BadRequest(e.to_string()))?;
    let request = probe_request(resolved);
    let bytes = zerodds_wire_codec::encode(resolved.dynamic_type.ast(), &request, &value)
        .map_err(|e| BackendError::BadRequest(format!("{e:?}")))?;
    resolved
        .support
        .read_body(&bytes, 2, false)
        .map_err(BackendError::BadRequest)
}

/// Decodes a `SerializedSample`'s wire body back to XML through the
/// type's own `DynamicType` descriptor. Uses the sample's own
/// representation/byte order (a reader may see either of a writer's
/// offer), not the fixed [`probe_request`] a write always uses.
fn sample_to_xml(
    resolved: &ResolvedType,
    sample: &SerializedSample,
) -> Result<String, BackendError> {
    let request = probe_request_for(sample, resolved);
    let value: zerodds_types::dynamic::DynamicData =
        zerodds_wire_codec::decode(resolved.dynamic_type.ast(), &request, sample.body())
            .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
    dynxml::to_xml_document("data", &value, &resolved.dynamic_type)
        .map_err(|e| BackendError::Internal(e.to_string()))
}

fn probe_request_for(sample: &SerializedSample, resolved: &ResolvedType) -> ProbeRequest {
    ProbeRequest::in_dialect(
        &resolved.dynamic_type.qualified_name(),
        if sample.representation() == 0 {
            Encoding::PlainCdr1
        } else {
            Encoding::PlainCdr2
        },
        if sample.big_endian() {
            Endianness::Big
        } else {
            Endianness::Little
        },
        resolved.support.dialect(),
    )
}

/// Splits `app/participant[/...]` style paths this trait's callers hand
/// back in. Not used at the trait boundary (the trait already takes
/// `app`/`participant` as separate strings) — kept for the WaitSet
/// reader-path bookkeeping, which stores composite paths as opaque
/// strings.
fn writer_path(app: &str, participant: &str, publisher: &str, writer: &str) -> String {
    format!(
        "applications/{app}/domain_participants/{participant}/publishers/{publisher}/data_writers/{writer}"
    )
}
fn reader_path(app: &str, participant: &str, subscriber: &str, reader: &str) -> String {
    format!(
        "applications/{app}/domain_participants/{participant}/subscribers/{subscriber}/data_readers/{reader}"
    )
}

impl DdsBackend for Gateway {
    fn create_application(&mut self, app_name: &str) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            if apps.contains_key(app_name) {
                return Err(BackendError::Conflict(app_name.to_string()));
            }
            apps.insert(app_name.to_string(), AppEntry::default());
            Ok(BackendResult::Created(format!("applications/{app_name}")))
        })
    }

    fn delete_application(&mut self, app_name: &str) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            if apps.remove(app_name).is_none() {
                return Err(BackendError::NotFound(app_name.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_applications(&self, pattern: &str) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let names = apps
            .keys()
            .filter(|n| pattern == "*" || pattern.is_empty() || n.as_str() == pattern)
            .cloned()
            .collect();
        Ok(BackendResult::List(names))
    }

    fn create_participant(
        &mut self,
        app_name: &str,
        domain_id: u32,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let doc = crate::contained_entities::parse_body(body)?;
        let qos_eq = match &doc {
            Some(doc) => crate::qos_update::resolve_entity_qos_from_element(
                &doc.root,
                &self.metadata,
                EntityKind::DomainParticipant,
            )?,
            None => None,
        };
        let publishers = match &doc {
            Some(doc) => crate::contained_entities::parse_publishers(&doc.root, &self.metadata)?,
            None => Vec::new(),
        };
        let subscribers = match &doc {
            Some(doc) => crate::contained_entities::parse_subscribers(&doc.root, &self.metadata)?,
            None => Vec::new(),
        };
        self.with_apps(|apps| {
            let app = apps
                .get_mut(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            let mut qos = DomainParticipantQos::default();
            if let Some(eq) = &qos_eq {
                qos = crate::qos_update::participant_qos_from(eq, qos);
            }
            let participant = DomainParticipantFactory::instance()
                .create_participant(
                    i32::try_from(domain_id).map_err(|_| {
                        BackendError::BadRequest(format!(
                            "domain_id {domain_id} is not a domain id"
                        ))
                    })?,
                    qos,
                )
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            app.next_participant += 1;
            let name = format!("p{}", app.next_participant);
            let mut entry = ParticipantEntry {
                participant,
                topics: BTreeMap::new(),
                publishers: BTreeMap::new(),
                subscribers: BTreeMap::new(),
            };
            sync_publishers(
                &mut entry.publishers,
                &entry.participant,
                &entry.topics,
                &self.types,
                &publishers,
            )?;
            sync_subscribers(
                &mut entry.subscribers,
                &entry.participant,
                &entry.topics,
                &self.types,
                &subscribers,
            )?;
            app.participants.insert(name.clone(), entry);
            Ok(BackendResult::Created(format!(
                "applications/{app_name}/domain_participants/{name}"
            )))
        })
    }

    fn update_participant(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let Some(doc) = crate::contained_entities::parse_body(body)? else {
            return self.with_apps(|apps| {
                participant_mut(apps, app_name, participant)?;
                Ok(BackendResult::Deleted)
            });
        };
        let qos_eq = crate::qos_update::resolve_entity_qos_from_element(
            &doc.root,
            &self.metadata,
            EntityKind::DomainParticipant,
        )?;
        // Absent from the body at all (not merely empty) means "leave
        // it as it is" — Spec: "If the participantObjectRepresentation
        // specifies a set of contained entities, then..." An empty
        // `<publishers/>` is instead a real "delete every publisher".
        let has_publishers = doc.root.child("publishers").is_some();
        let has_subscribers = doc.root.child("subscribers").is_some();
        let publishers = crate::contained_entities::parse_publishers(&doc.root, &self.metadata)?;
        let subscribers = crate::contained_entities::parse_subscribers(&doc.root, &self.metadata)?;
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            if let Some(eq) = &qos_eq {
                let qos = crate::qos_update::participant_qos_from(eq, part.participant.get_qos());
                part.participant
                    .set_qos(qos)
                    .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            }
            if has_publishers {
                sync_publishers(
                    &mut part.publishers,
                    &part.participant,
                    &part.topics,
                    &self.types,
                    &publishers,
                )?;
            }
            if has_subscribers {
                sync_subscribers(
                    &mut part.subscribers,
                    &part.participant,
                    &part.topics,
                    &self.types,
                    &subscribers,
                )?;
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_participant(
        &mut self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let app = apps
                .get_mut(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            if app.participants.remove(participant).is_none() {
                return Err(BackendError::NotFound(participant.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_participants(&self, app_name: &str) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let app = apps
            .get(app_name)
            .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
        Ok(BackendResult::List(
            app.participants.keys().cloned().collect(),
        ))
    }

    fn create_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic_name: &str,
        type_name: &str,
    ) -> Result<BackendResult, BackendError> {
        // Cloned without holding `self.apps`'s lock: `resolve_type_for_topic`
        // may block up to `TYPE_LOOKUP_TIMEOUT` on a `getTypes` round trip
        // when the type is not already known, and holding the apps lock
        // across that would stall every other request in the meantime.
        // `DomainParticipant` is `Clone` (a handle onto shared state).
        let participant_handle = self.with_apps(|apps| {
            participant_ref(apps, app_name, participant).map(|p| p.participant.clone())
        })?;
        let resolved = self.resolve_type_for_topic(type_name, topic_name, &participant_handle)?;
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            if part.topics.contains_key(topic_name) {
                return Err(BackendError::Conflict(topic_name.to_string()));
            }
            let topic = part
                .participant
                .create_serialized_topic(
                    topic_name,
                    TopicQos::default(),
                    Arc::clone(&resolved.support),
                )
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            part.topics.insert(topic_name.to_string(), topic);
            self.types.insert(type_name, resolved.clone());
            Ok(BackendResult::Created(format!(
                "applications/{app_name}/domain_participants/{participant}/topics/{topic_name}"
            )))
        })
    }

    fn update_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        // The topic name is at hand, so the profile's §7.3.2.4.3
        // `topic_filter`s are evaluated against it.
        let Some(eq) = crate::qos_update::resolve_entity_qos_for_topic(
            body,
            &self.metadata,
            EntityKind::Topic,
            topic,
        )?
        else {
            return self.with_apps(|apps| {
                let part = participant_mut(apps, app_name, participant)?;
                if part.topics.contains_key(topic) {
                    Ok(BackendResult::Deleted)
                } else {
                    Err(BackendError::NotFound(topic.to_string()))
                }
            });
        };
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let topic_handle = part
                .topics
                .get(topic)
                .ok_or_else(|| BackendError::NotFound(topic.to_string()))?;
            let qos = crate::qos_update::topic_qos_from(&eq, topic_handle.get_qos());
            topic_handle
                .set_qos(qos)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            if part.topics.remove(topic).is_none() {
                return Err(BackendError::NotFound(topic.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_topics(
        &self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let part = participant_ref(&apps, app_name, participant)?;
        Ok(BackendResult::List(part.topics.keys().cloned().collect()))
    }

    fn create_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let doc = crate::contained_entities::parse_body(body)?;
        let qos_eq = match &doc {
            Some(doc) => crate::qos_update::resolve_entity_qos_from_element(
                &doc.root,
                &self.metadata,
                EntityKind::Publisher,
            )?,
            None => None,
        };
        let data_writers = match &doc {
            Some(doc) => crate::contained_entities::parse_data_writers(&doc.root, &self.metadata)?,
            None => Vec::new(),
        };
        self.with_apps(|apps| {
            let app_entry_next = {
                let app = apps
                    .get_mut(app_name)
                    .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
                app.next_publisher += 1;
                app.next_publisher
            };
            let part = participant_mut(apps, app_name, participant)?;
            let mut qos = PublisherQos::default();
            if let Some(eq) = &qos_eq {
                qos = crate::qos_update::publisher_qos_from(eq, qos);
            }
            let publisher = part.participant.create_publisher(qos);
            let name = format!("pub{app_entry_next}");
            let mut writers = BTreeMap::new();
            sync_data_writers(
                &mut writers,
                &publisher,
                &part.topics,
                &self.types,
                &data_writers,
            )?;
            part.publishers
                .insert(name.clone(), PublisherEntry { publisher, writers });
            Ok(BackendResult::Created(format!(
                "applications/{app_name}/domain_participants/{participant}/publishers/{name}"
            )))
        })
    }

    fn update_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let Some(doc) = crate::contained_entities::parse_body(body)? else {
            return self.with_apps(|apps| {
                let part = participant_mut(apps, app_name, participant)?;
                if part.publishers.contains_key(publisher) {
                    Ok(BackendResult::Deleted)
                } else {
                    Err(BackendError::NotFound(publisher.to_string()))
                }
            });
        };
        let qos_eq = crate::qos_update::resolve_entity_qos_from_element(
            &doc.root,
            &self.metadata,
            EntityKind::Publisher,
        )?;
        let has_data_writers = doc.root.child("data_writers").is_some();
        let data_writers =
            crate::contained_entities::parse_data_writers(&doc.root, &self.metadata)?;
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let topics = part.topics.clone();
            let pub_entry = part
                .publishers
                .get_mut(publisher)
                .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
            if let Some(eq) = &qos_eq {
                let qos = crate::qos_update::publisher_qos_from(eq, pub_entry.publisher.get_qos());
                pub_entry
                    .publisher
                    .set_qos(qos)
                    .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            }
            if has_data_writers {
                sync_data_writers(
                    &mut pub_entry.writers,
                    &pub_entry.publisher,
                    &topics,
                    &self.types,
                    &data_writers,
                )?;
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            if part.publishers.remove(publisher).is_none() {
                return Err(BackendError::NotFound(publisher.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_publishers(
        &self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let part = participant_ref(&apps, app_name, participant)?;
        Ok(BackendResult::List(
            part.publishers.keys().cloned().collect(),
        ))
    }

    fn create_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let doc = crate::contained_entities::parse_body(body)?;
        let qos_eq = match &doc {
            Some(doc) => crate::qos_update::resolve_entity_qos_from_element(
                &doc.root,
                &self.metadata,
                EntityKind::Subscriber,
            )?,
            None => None,
        };
        let data_readers = match &doc {
            Some(doc) => crate::contained_entities::parse_data_readers(&doc.root, &self.metadata)?,
            None => Vec::new(),
        };
        self.with_apps(|apps| {
            let app_entry_next = {
                let app = apps
                    .get_mut(app_name)
                    .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
                app.next_subscriber += 1;
                app.next_subscriber
            };
            let part = participant_mut(apps, app_name, participant)?;
            let mut qos = SubscriberQos::default();
            if let Some(eq) = &qos_eq {
                qos = crate::qos_update::subscriber_qos_from(eq, qos);
            }
            let subscriber = part.participant.create_subscriber(qos);
            let name = format!("sub{app_entry_next}");
            let mut readers = BTreeMap::new();
            sync_data_readers(
                &mut readers,
                &subscriber,
                &part.topics,
                &self.types,
                &data_readers,
            )?;
            part.subscribers.insert(
                name.clone(),
                SubscriberEntry {
                    subscriber,
                    readers,
                },
            );
            Ok(BackendResult::Created(format!(
                "applications/{app_name}/domain_participants/{participant}/subscribers/{name}"
            )))
        })
    }

    fn update_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let Some(doc) = crate::contained_entities::parse_body(body)? else {
            return self.with_apps(|apps| {
                let part = participant_mut(apps, app_name, participant)?;
                if part.subscribers.contains_key(subscriber) {
                    Ok(BackendResult::Deleted)
                } else {
                    Err(BackendError::NotFound(subscriber.to_string()))
                }
            });
        };
        let qos_eq = crate::qos_update::resolve_entity_qos_from_element(
            &doc.root,
            &self.metadata,
            EntityKind::Subscriber,
        )?;
        let has_data_readers = doc.root.child("data_readers").is_some();
        let data_readers =
            crate::contained_entities::parse_data_readers(&doc.root, &self.metadata)?;
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let topics = part.topics.clone();
            let sub_entry = part
                .subscribers
                .get_mut(subscriber)
                .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
            if let Some(eq) = &qos_eq {
                let qos =
                    crate::qos_update::subscriber_qos_from(eq, sub_entry.subscriber.get_qos());
                sub_entry
                    .subscriber
                    .set_qos(qos)
                    .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            }
            if has_data_readers {
                sync_data_readers(
                    &mut sub_entry.readers,
                    &sub_entry.subscriber,
                    &topics,
                    &self.types,
                    &data_readers,
                )?;
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            if part.subscribers.remove(subscriber).is_none() {
                return Err(BackendError::NotFound(subscriber.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_subscribers(
        &self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let part = participant_ref(&apps, app_name, participant)?;
        Ok(BackendResult::List(
            part.subscribers.keys().cloned().collect(),
        ))
    }

    fn create_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let app_entry_next = {
                let app = apps
                    .get_mut(app_name)
                    .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
                app.next_writer += 1;
                app.next_writer
            };
            let part = participant_mut(apps, app_name, participant)?;
            let topic_handle = part
                .topics
                .get(topic)
                .ok_or_else(|| BackendError::NotFound(topic.to_string()))?
                .clone();
            let type_name = topic_handle.type_name().to_string();
            let resolved = self.types.get(&type_name).ok_or_else(|| {
                BackendError::Internal(format!("type `{type_name}` not resolved"))
            })?;
            let pub_entry = part
                .publishers
                .get_mut(publisher)
                .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
            let writer = pub_entry
                .publisher
                .create_datawriter(&topic_handle, DataWriterQos::default())
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            let name = format!("dw{app_entry_next}");
            pub_entry
                .writers
                .insert(name.clone(), WriterEntry { writer, resolved });
            Ok(BackendResult::Created(writer_path(
                app_name,
                participant,
                publisher,
                &name,
            )))
        })
    }

    fn update_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        // The body carries no topic name here (the writer's own topic
        // lives on the DDS entity, not in this API shape), so the
        // profile's §7.3.2.4.3 slots narrow via the filterless
        // container, else the first one.
        let Some(eq) =
            crate::qos_update::resolve_entity_qos(body, &self.metadata, EntityKind::DataWriter)?
        else {
            return self.with_apps(|apps| {
                let part = participant_mut(apps, app_name, participant)?;
                let pub_entry = part
                    .publishers
                    .get(publisher)
                    .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
                if pub_entry.writers.contains_key(writer) {
                    Ok(BackendResult::Deleted)
                } else {
                    Err(BackendError::NotFound(writer.to_string()))
                }
            });
        };
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let pub_entry = part
                .publishers
                .get(publisher)
                .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
            let writer_entry = pub_entry
                .writers
                .get(writer)
                .ok_or_else(|| BackendError::NotFound(writer.to_string()))?;
            let qos = crate::qos_update::writer_qos_from(&eq, writer_entry.writer.get_qos());
            writer_entry
                .writer
                .set_qos(qos)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let pub_entry = part
                .publishers
                .get_mut(publisher)
                .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
            if pub_entry.writers.remove(writer).is_none() {
                return Err(BackendError::NotFound(writer.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_data_writers(
        &self,
        app_name: &str,
        participant: &str,
        publisher: &str,
    ) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let part = participant_ref(&apps, app_name, participant)?;
        let pub_entry = part
            .publishers
            .get(publisher)
            .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
        Ok(BackendResult::List(
            pub_entry.writers.keys().cloned().collect(),
        ))
    }

    fn create_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let app_entry_next = {
                let app = apps
                    .get_mut(app_name)
                    .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
                app.next_reader += 1;
                app.next_reader
            };
            let part = participant_mut(apps, app_name, participant)?;
            let topic_handle = part
                .topics
                .get(topic)
                .ok_or_else(|| BackendError::NotFound(topic.to_string()))?
                .clone();
            let type_name = topic_handle.type_name().to_string();
            let resolved = self.types.get(&type_name).ok_or_else(|| {
                BackendError::Internal(format!("type `{type_name}` not resolved"))
            })?;
            let sub_entry = part
                .subscribers
                .get_mut(subscriber)
                .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
            let reader = sub_entry
                .subscriber
                .create_datareader(&topic_handle, DataReaderQos::default())
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            let name = format!("dr{app_entry_next}");
            sub_entry.readers.insert(
                name.clone(),
                ReaderEntry {
                    reader: Arc::new(reader),
                    resolved,
                },
            );
            Ok(BackendResult::Created(reader_path(
                app_name,
                participant,
                subscriber,
                &name,
            )))
        })
    }

    fn update_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        // Same slot narrowing as `update_data_writer`: no topic name
        // in the body, so the filterless container, else the first.
        let Some(eq) =
            crate::qos_update::resolve_entity_qos(body, &self.metadata, EntityKind::DataReader)?
        else {
            return self.with_apps(|apps| {
                let part = participant_mut(apps, app_name, participant)?;
                let sub_entry = part
                    .subscribers
                    .get(subscriber)
                    .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
                if sub_entry.readers.contains_key(reader) {
                    Ok(BackendResult::Deleted)
                } else {
                    Err(BackendError::NotFound(reader.to_string()))
                }
            });
        };
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let sub_entry = part
                .subscribers
                .get(subscriber)
                .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
            let reader_entry = sub_entry
                .readers
                .get(reader)
                .ok_or_else(|| BackendError::NotFound(reader.to_string()))?;
            let qos = crate::qos_update::reader_qos_from(&eq, reader_entry.reader.get_qos());
            reader_entry
                .reader
                .set_qos(qos)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            Ok(BackendResult::Deleted)
        })
    }

    fn delete_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let sub_entry = part
                .subscribers
                .get_mut(subscriber)
                .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
            if sub_entry.readers.remove(reader).is_none() {
                return Err(BackendError::NotFound(reader.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_data_readers(
        &self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
    ) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let part = participant_ref(&apps, app_name, participant)?;
        let sub_entry = part
            .subscribers
            .get(subscriber)
            .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
        Ok(BackendResult::List(
            sub_entry.readers.keys().cloned().collect(),
        ))
    }

    fn write_sample(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError> {
        let xml = std::str::from_utf8(body)
            .map_err(|e| BackendError::BadRequest(format!("body is not UTF-8: {e}")))?;
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let pub_entry = part
                .publishers
                .get_mut(publisher)
                .ok_or_else(|| BackendError::NotFound(publisher.to_string()))?;
            let writer_entry = pub_entry
                .writers
                .get(writer)
                .ok_or_else(|| BackendError::NotFound(writer.to_string()))?;
            let sample = encode_sample(&writer_entry.resolved, xml)?;
            writer_entry
                .writer
                .write(&sample)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            Ok(BackendResult::Deleted)
        })
    }

    fn read_samples(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
        _selector: Option<&str>,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let part = participant_mut(apps, app_name, participant)?;
            let sub_entry = part
                .subscribers
                .get_mut(subscriber)
                .ok_or_else(|| BackendError::NotFound(subscriber.to_string()))?;
            let reader_entry = sub_entry
                .readers
                .get(reader)
                .ok_or_else(|| BackendError::NotFound(reader.to_string()))?;
            let samples = reader_entry
                .reader
                .read()
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            let mut emitter = zerodds_xml_wire::emitter::XmlEmitter::new();
            emitter.declaration();
            emitter
                .start_element(
                    zerodds_web::representation::element_name(
                        zerodds_web::representation::ContentType::ReadSampleSeq,
                        zerodds_web::representation::RepresentationKind::List,
                    ),
                    &[],
                )
                .map_err(|e| BackendError::Internal(e.to_string()))?;
            let mut body = emitter.finish();
            for sample in &samples {
                let xml = sample_to_xml(&reader_entry.resolved, sample)?;
                // Strip the `<?xml ...?>` declaration each per-sample
                // document carries — only the outer document needs one.
                let stripped = xml.split_once("?>").map(|(_, rest)| rest).unwrap_or(&xml);
                body.push_str(stripped);
            }
            body.push_str(&format!(
                "</{}>",
                zerodds_web::representation::element_name(
                    zerodds_web::representation::ContentType::ReadSampleSeq,
                    zerodds_web::representation::RepresentationKind::List,
                )
            ));
            Ok(BackendResult::Body {
                content_type: zerodds_web::representation::CONTENT_TYPE_DDS_WEB_XML.to_string(),
                body: body.into_bytes(),
            })
        })
    }

    fn create_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        readers: &[String],
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let app = apps
                .get_mut(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            if app.waitsets.contains_key(waitset) {
                return Err(BackendError::Conflict(waitset.to_string()));
            }
            let entry = build_waitset_entry(app, readers)?;
            app.waitsets.insert(waitset.to_string(), Arc::new(entry));
            Ok(BackendResult::Created(format!(
                "applications/{app_name}/waitsets/{waitset}"
            )))
        })
    }

    fn update_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        readers: &[String],
    ) -> Result<BackendResult, BackendError> {
        // Spec §7.4.3.5 `update_waitset`: "It updates the conditions
        // associated with the WebDDS::WaitSet" — unlike every other
        // `update_*`, no QoS is involved; `readers` fully replaces the
        // condition set, built the same way `create_waitset`'s is.
        self.with_apps(|apps| {
            let app = apps
                .get_mut(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            if !app.waitsets.contains_key(waitset) {
                return Err(BackendError::NotFound(waitset.to_string()));
            }
            let entry = build_waitset_entry(app, readers)?;
            app.waitsets.insert(waitset.to_string(), Arc::new(entry));
            Ok(BackendResult::Deleted)
        })
    }

    fn wait_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        timeout: Duration,
    ) -> Result<BackendResult, BackendError> {
        self.waitset_handle(app_name, waitset)?.wait(timeout)
    }

    fn delete_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
    ) -> Result<BackendResult, BackendError> {
        self.with_apps(|apps| {
            let app = apps
                .get_mut(app_name)
                .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
            if app.waitsets.remove(waitset).is_none() {
                return Err(BackendError::NotFound(waitset.to_string()));
            }
            Ok(BackendResult::Deleted)
        })
    }

    fn list_waitsets(&self, app_name: &str) -> Result<BackendResult, BackendError> {
        let apps = self
            .apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let app = apps
            .get(app_name)
            .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?;
        Ok(BackendResult::List(app.waitsets.keys().cloned().collect()))
    }
}

/// Reconciles `writers` (a `Publisher`'s contained `DataWriter`s)
/// against `wanted` — Spec §7.4.4.8's rule for `update_publisher`,
/// reused as-is by `create_publisher`/`create_participant` (an empty
/// `writers` map makes every entry in `wanted` "does not yet exist"):
/// one already present (by name) has its QoS re-applied if `wanted`
/// named one; one absent from `wanted` is removed; one named in
/// `wanted` that does not yet exist is created against the topic it
/// names (which must already exist on the enclosing participant).
fn sync_data_writers(
    writers: &mut BTreeMap<String, WriterEntry>,
    publisher: &Publisher,
    topics: &BTreeMap<String, Topic<SerializedSample>>,
    types: &TypeRegistry,
    wanted: &[crate::contained_entities::ParsedDataWriter],
) -> Result<(), BackendError> {
    let wanted_names: std::collections::BTreeSet<&str> =
        wanted.iter().map(|w| w.name.as_str()).collect();
    writers.retain(|name, _| wanted_names.contains(name.as_str()));
    for w in wanted {
        if let Some(entry) = writers.get(&w.name) {
            if let Some(eq) = &w.qos {
                let qos = crate::qos_update::writer_qos_from(eq, entry.writer.get_qos());
                entry
                    .writer
                    .set_qos(qos)
                    .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            }
        } else {
            let topic_handle = topics.get(&w.topic_name).cloned().ok_or_else(|| {
                BackendError::BadRequest(format!(
                    "data_writer `{}`: unknown topic `{}`",
                    w.name, w.topic_name
                ))
            })?;
            let type_name = topic_handle.type_name().to_string();
            let resolved = types.get(&type_name).ok_or_else(|| {
                BackendError::Internal(format!("type `{type_name}` not resolved"))
            })?;
            let mut qos = DataWriterQos::default();
            if let Some(eq) = &w.qos {
                qos = crate::qos_update::writer_qos_from(eq, qos);
            }
            let writer = publisher
                .create_datawriter(&topic_handle, qos)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            writers.insert(w.name.clone(), WriterEntry { writer, resolved });
        }
    }
    Ok(())
}

/// Reconciles `readers` (a `Subscriber`'s contained `DataReader`s)
/// against `wanted`. See [`sync_data_writers`].
fn sync_data_readers(
    readers: &mut BTreeMap<String, ReaderEntry>,
    subscriber: &Subscriber,
    topics: &BTreeMap<String, Topic<SerializedSample>>,
    types: &TypeRegistry,
    wanted: &[crate::contained_entities::ParsedDataReader],
) -> Result<(), BackendError> {
    let wanted_names: std::collections::BTreeSet<&str> =
        wanted.iter().map(|r| r.name.as_str()).collect();
    readers.retain(|name, _| wanted_names.contains(name.as_str()));
    for r in wanted {
        if let Some(entry) = readers.get(&r.name) {
            if let Some(eq) = &r.qos {
                let qos = crate::qos_update::reader_qos_from(eq, entry.reader.get_qos());
                entry
                    .reader
                    .set_qos(qos)
                    .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            }
        } else {
            let topic_handle = topics.get(&r.topic_name).cloned().ok_or_else(|| {
                BackendError::BadRequest(format!(
                    "data_reader `{}`: unknown topic `{}`",
                    r.name, r.topic_name
                ))
            })?;
            let type_name = topic_handle.type_name().to_string();
            let resolved = types.get(&type_name).ok_or_else(|| {
                BackendError::Internal(format!("type `{type_name}` not resolved"))
            })?;
            let mut qos = DataReaderQos::default();
            if let Some(eq) = &r.qos {
                qos = crate::qos_update::reader_qos_from(eq, qos);
            }
            let reader = subscriber
                .create_datareader(&topic_handle, qos)
                .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
            readers.insert(
                r.name.clone(),
                ReaderEntry {
                    reader: Arc::new(reader),
                    resolved,
                },
            );
        }
    }
    Ok(())
}

/// Reconciles `publishers` (a `DomainParticipant`'s contained
/// `Publisher`s) against `wanted`, recursing into each one's own
/// contained `DataWriter`s ([`sync_data_writers`]). See
/// [`sync_data_writers`] for the add/re-QoS/remove rule.
fn sync_publishers(
    publishers: &mut BTreeMap<String, PublisherEntry>,
    participant: &DomainParticipant,
    topics: &BTreeMap<String, Topic<SerializedSample>>,
    types: &TypeRegistry,
    wanted: &[crate::contained_entities::ParsedPublisher],
) -> Result<(), BackendError> {
    let wanted_names: std::collections::BTreeSet<&str> =
        wanted.iter().map(|p| p.name.as_str()).collect();
    publishers.retain(|name, _| wanted_names.contains(name.as_str()));
    for p in wanted {
        let entry = match publishers.entry(p.name.clone()) {
            std::collections::btree_map::Entry::Occupied(occ) => {
                if let Some(eq) = &p.qos {
                    let qos =
                        crate::qos_update::publisher_qos_from(eq, occ.get().publisher.get_qos());
                    occ.get()
                        .publisher
                        .set_qos(qos)
                        .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
                }
                occ.into_mut()
            }
            std::collections::btree_map::Entry::Vacant(vac) => {
                let mut qos = PublisherQos::default();
                if let Some(eq) = &p.qos {
                    qos = crate::qos_update::publisher_qos_from(eq, qos);
                }
                let publisher = participant.create_publisher(qos);
                vac.insert(PublisherEntry {
                    publisher,
                    writers: BTreeMap::new(),
                })
            }
        };
        sync_data_writers(
            &mut entry.writers,
            &entry.publisher,
            topics,
            types,
            &p.data_writers,
        )?;
    }
    Ok(())
}

/// Reconciles `subscribers` (a `DomainParticipant`'s contained
/// `Subscriber`s) against `wanted`, recursing into each one's own
/// contained `DataReader`s ([`sync_data_readers`]). See
/// [`sync_publishers`].
fn sync_subscribers(
    subscribers: &mut BTreeMap<String, SubscriberEntry>,
    participant: &DomainParticipant,
    topics: &BTreeMap<String, Topic<SerializedSample>>,
    types: &TypeRegistry,
    wanted: &[crate::contained_entities::ParsedSubscriber],
) -> Result<(), BackendError> {
    let wanted_names: std::collections::BTreeSet<&str> =
        wanted.iter().map(|s| s.name.as_str()).collect();
    subscribers.retain(|name, _| wanted_names.contains(name.as_str()));
    for s in wanted {
        let entry = match subscribers.entry(s.name.clone()) {
            std::collections::btree_map::Entry::Occupied(occ) => {
                if let Some(eq) = &s.qos {
                    let qos =
                        crate::qos_update::subscriber_qos_from(eq, occ.get().subscriber.get_qos());
                    occ.get()
                        .subscriber
                        .set_qos(qos)
                        .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
                }
                occ.into_mut()
            }
            std::collections::btree_map::Entry::Vacant(vac) => {
                let mut qos = SubscriberQos::default();
                if let Some(eq) = &s.qos {
                    qos = crate::qos_update::subscriber_qos_from(eq, qos);
                }
                let subscriber = participant.create_subscriber(qos);
                vac.insert(SubscriberEntry {
                    subscriber,
                    readers: BTreeMap::new(),
                })
            }
        };
        sync_data_readers(
            &mut entry.readers,
            &entry.subscriber,
            topics,
            types,
            &s.data_readers,
        )?;
    }
    Ok(())
}

fn participant_mut<'a>(
    apps: &'a mut BTreeMap<String, AppEntry>,
    app_name: &str,
    participant: &str,
) -> Result<&'a mut ParticipantEntry, BackendError> {
    apps.get_mut(app_name)
        .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?
        .participants
        .get_mut(participant)
        .ok_or_else(|| BackendError::NotFound(participant.to_string()))
}

fn participant_ref<'a>(
    apps: &'a BTreeMap<String, AppEntry>,
    app_name: &str,
    participant: &str,
) -> Result<&'a ParticipantEntry, BackendError> {
    apps.get(app_name)
        .ok_or_else(|| BackendError::NotFound(app_name.to_string()))?
        .participants
        .get(participant)
        .ok_or_else(|| BackendError::NotFound(participant.to_string()))
}

/// Splits a `.../subscribers/<sub>/data_readers/<dr>` path (as returned
/// by `create_data_reader`) into `(app, participant, subscriber,
/// reader)`.
/// Builds a fresh [`WaitsetEntry`] — a new `DDS::WaitSet` with one
/// `ReadCondition` per reader path in `readers`, attached — shared by
/// `create_waitset` and `update_waitset` (Spec §7.4.3.4/§7.4.3.5): a
/// `WaitSet`'s conditions are always a full replacement, never an
/// incremental add.
fn build_waitset_entry(app: &AppEntry, readers: &[String]) -> Result<WaitsetEntry, BackendError> {
    let ws = WaitSet::new();
    let mut conditions = Vec::with_capacity(readers.len());
    for reader_path in readers {
        let (_, _, subscriber, reader_name) = split_reader_path(reader_path)
            .ok_or_else(|| BackendError::BadRequest(format!("not a reader path: {reader_path}")))?;
        let mut found = None;
        for part in app.participants.values() {
            if let Some(sub_entry) = part.subscribers.get(&subscriber) {
                if let Some(r) = sub_entry.readers.get(&reader_name) {
                    found = Some(Arc::clone(&r.reader));
                }
            }
        }
        let reader_handle = found.ok_or_else(|| BackendError::NotFound(reader_path.clone()))?;
        let condition = ReadCondition::new(0, 0, 0, move |_, _, _| {
            !reader_handle.read().unwrap_or_default().is_empty()
        });
        ws.attach_condition(Arc::clone(&condition) as Arc<dyn Condition>)
            .map_err(|e| BackendError::Internal(format!("{e:?}")))?;
        conditions.push((reader_path.clone(), condition));
    }
    Ok(WaitsetEntry {
        waitset: ws,
        conditions,
        turn: Mutex::new(()),
    })
}

fn split_reader_path(path: &str) -> Option<(String, String, String, String)> {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segments.len() != 8 || segments[0] != "applications" || segments[2] != "domain_participants"
    {
        return None;
    }
    Some((
        segments[1].to_string(),
        segments[3].to_string(),
        segments[5].to_string(),
        segments[7].to_string(),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn a_timeout_is_a_finite_non_negative_number_of_seconds() {
        assert_eq!(waitset_timeout(None), Ok(DEFAULT_WAITSET_TIMEOUT));
        assert_eq!(
            waitset_timeout(Some("2.5")),
            Ok(Duration::from_millis(2500))
        );
        assert_eq!(waitset_timeout(Some("0")), Ok(Duration::ZERO));
        for bad in ["-1", "nan", "inf", "1e300", "abc", ""] {
            assert!(
                matches!(waitset_timeout(Some(bad)), Err(BackendError::BadRequest(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_domain_id_is_read_or_refused() {
        assert_eq!(domain_id_of(None), Ok(0));
        assert_eq!(domain_id_of(Some(" 42 ")), Ok(42));
        for bad in ["abc", "-1", "2147483648", "4294967295", ""] {
            assert!(
                matches!(domain_id_of(Some(bad)), Err(BackendError::BadRequest(_))),
                "{bad}"
            );
        }
    }

    fn idl_file(idl: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("zd-webdds-gateway-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.idl");
        std::fs::write(&path, idl).unwrap();
        path
    }

    #[test]
    fn full_lifecycle_write_then_read_offline() {
        let path = idl_file("module M { @final struct Sensor { long value; }; };");
        let mut types = BTreeMap::new();
        types.insert(
            "M::Sensor".to_string(),
            crate::gateway::TypeSource::Idl(path),
        );
        let mut gw = Gateway::new(types, Arc::new(crate::metadata::MetadataStore::new()));

        gw.create_application("app1").expect("app");
        let BackendResult::Created(p) = gw.create_participant("app1", 0, b"").expect("participant")
        else {
            panic!()
        };
        let participant = p.rsplit('/').next().unwrap().to_string();
        gw.create_topic("app1", &participant, "Sensor", "M::Sensor")
            .expect("topic");
        let BackendResult::Created(p) =
            gw.create_publisher("app1", &participant, b"").expect("pub")
        else {
            panic!()
        };
        let publisher = p.rsplit('/').next().unwrap().to_string();
        let BackendResult::Created(p) = gw
            .create_data_writer("app1", &participant, &publisher, "Sensor")
            .expect("writer")
        else {
            panic!()
        };
        let writer = p.rsplit('/').next().unwrap().to_string();
        let r = gw.write_sample(
            "app1",
            &participant,
            &publisher,
            &writer,
            b"<data><value>42</value></data>",
        );
        assert!(r.is_ok(), "{r:?}");
    }

    /// `update_participant`'s recursive contained-entity sync end to
    /// end: a publisher + a data_writer nested in the PUT body, created
    /// (rather than the flat, per-resource POSTs
    /// `full_lifecycle_write_then_read_offline` exercises); a second
    /// `update_participant` with a DIFFERENT nested representation then
    /// deletes what the first one created and adds something new —
    /// Spec §7.4.3.2's own "creating/re-QoS'ing/deleting contained
    /// entities" rule, checked against actual DCPS state via
    /// `list_publishers`/`list_data_writers`, not just that the calls
    /// returned `Ok`.
    #[test]
    fn update_participant_creates_and_reconciles_contained_entities() {
        let path = idl_file("module M { @final struct Sensor { long value; }; };");
        let mut types = BTreeMap::new();
        types.insert(
            "M::Sensor".to_string(),
            crate::gateway::TypeSource::Idl(path),
        );
        let mut gw = Gateway::new(types, Arc::new(crate::metadata::MetadataStore::new()));

        gw.create_application("app1").expect("app");
        let BackendResult::Created(p) = gw.create_participant("app1", 0, b"").expect("participant")
        else {
            panic!()
        };
        let participant = p.rsplit('/').next().unwrap().to_string();
        gw.create_topic("app1", &participant, "Sensor", "M::Sensor")
            .expect("topic");

        // First update: creates a publisher AND its contained writer in
        // one call — neither existed before.
        let body = b"<domain_participant><publishers><element><name>pub1</name><data_writers><element><name>dw1</name><topic_name>Sensor</topic_name></element></data_writers></element></publishers></domain_participant>";
        gw.update_participant("app1", &participant, body)
            .expect("first update");
        let BackendResult::List(pubs) =
            gw.list_publishers("app1", &participant).expect("list pubs")
        else {
            panic!()
        };
        assert_eq!(pubs, vec!["pub1".to_string()]);
        let BackendResult::List(writers) = gw
            .list_data_writers("app1", &participant, "pub1")
            .expect("list writers")
        else {
            panic!()
        };
        assert_eq!(writers, vec!["dw1".to_string()]);

        // Second update: a body naming a DIFFERENT publisher ("pub2",
        // no "pub1") reconciles pub1 away and creates pub2 instead —
        // Spec §7.4.3.2's "deleting the entities ... which do not
        // appear in the representation" rule.
        let body = b"<domain_participant><publishers><element><name>pub2</name></element></publishers></domain_participant>";
        gw.update_participant("app1", &participant, body)
            .expect("second update");
        let BackendResult::List(pubs) = gw
            .list_publishers("app1", &participant)
            .expect("list pubs 2")
        else {
            panic!()
        };
        assert_eq!(pubs, vec!["pub2".to_string()]);

        // A body naming no `<publishers>` element at all leaves pub2
        // untouched — absent is "don't touch", not "delete everything".
        gw.update_participant("app1", &participant, b"<domain_participant/>")
            .expect("third update");
        let BackendResult::List(pubs) = gw
            .list_publishers("app1", &participant)
            .expect("list pubs 3")
        else {
            panic!()
        };
        assert_eq!(pubs, vec!["pub2".to_string()]);
    }

    #[test]
    fn unknown_application_is_not_found() {
        let mut gw = Gateway::new(
            BTreeMap::new(),
            Arc::new(crate::metadata::MetadataStore::new()),
        );
        let err = gw.create_participant("nope", 0, b"").unwrap_err();
        assert!(matches!(err, BackendError::NotFound(_)));
    }

    #[test]
    fn split_reader_path_roundtrip() {
        let p = "applications/a/domain_participants/p1/subscribers/s1/data_readers/dr1";
        assert_eq!(
            split_reader_path(p),
            Some(("a".into(), "p1".into(), "s1".into(), "dr1".into()))
        );
    }

    /// `create_topic` with a type this daemon's config does not name at
    /// all resolves it purely from a peer's SEDP announcement + a
    /// `getTypes` round trip (item 5 end to end, through the public
    /// `DdsBackend` surface rather than `types_registry` directly).
    #[test]
    fn create_topic_resolves_an_unconfigured_type_via_type_lookup() {
        let path = idl_file("module M { @final struct Discovered { @key long id; }; };");
        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id();

        // Peer: publishes the topic, with the type known locally.
        let reg = crate::types_registry::TypeRegistry::new();
        let resolved = reg
            .load_idl_file(&path, "M::Discovered")
            .expect("resolve peer side");
        let peer = zerodds_dcps::DomainParticipantFactory::instance()
            .create_participant(domain, zerodds_dcps::DomainParticipantQos::default())
            .expect("peer participant");
        let peer_topic = peer
            .create_serialized_topic("Discovered", TopicQos::default(), resolved.support)
            .expect("peer topic");
        let peer_publisher = peer.create_publisher(zerodds_dcps::PublisherQos::default());
        let _peer_writer = peer_publisher
            .create_datawriter(&peer_topic, zerodds_dcps::DataWriterQos::default())
            .expect("peer writer");

        // Gateway: empty `types:` config — the type is entirely
        // unconfigured here.
        let mut gw = Gateway::new(
            BTreeMap::new(),
            Arc::new(crate::metadata::MetadataStore::new()),
        );
        gw.create_application("app1").expect("app");
        let BackendResult::Created(p) = gw
            .create_participant("app1", domain as u32, b"")
            .expect("gateway participant")
        else {
            panic!()
        };
        let participant = p.rsplit('/').next().unwrap().to_string();

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut created = None;
        while created.is_none() && std::time::Instant::now() < deadline {
            match gw.create_topic("app1", &participant, "Discovered", "M::Discovered") {
                Ok(r) => created = Some(r),
                Err(BackendError::BadRequest(m)) if m.contains("no discovered") => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => panic!("create_topic failed: {e:?}"),
            }
        }
        assert!(
            matches!(created, Some(BackendResult::Created(_))),
            "topic was not created via TypeLookup discovery within 20s"
        );
    }
}
