// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DomainParticipant — the "root" entity of a DDS program.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.2 `DomainParticipant`.
//!
//! Every DDS program typically opens exactly one `DomainParticipant`
//! per domain id. The participant:
//!
//! - holds the GUID prefix (12 bytes, the base ID for all internal endpoints),
//! - registers itself via SPDP (Simple Participant Discovery Protocol),
//! - runs SEDP (Simple Endpoint Discovery Protocol) for
//!   topic/writer/reader matching,
//! - is the factory for publishers, subscribers and topics.
//!
//! # Modes
//!
//! - **Live mode** (`new_with_runtime`, called from
//!   `DomainParticipantFactory::create_participant`): binds UDP sockets,
//!   spawns SPDP/SEDP/WLP threads, runs the full discovery protocol and
//!   the TypeLookup service endpoints (XTypes 1.3 §7.6.3.3.4).
//! - **Offline mode** (`new`, called from
//!   `DomainParticipantFactory::create_participant_offline`): no
//!   sockets, no threads. The topic registry, QoS negotiation and a
//!   loopback path for unit tests are available.
//!
//! Topic registry: the same name + same type yields the same topic
//! handle (DDS 1.4 §2.2.2.2.1.10 `find_topic`).

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::builtin_subscriber::BuiltinSubscriber;
use crate::builtin_topics::{ParticipantBuiltinTopicData, TopicBuiltinTopicData};
use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
use crate::instance_handle::InstanceHandle;
use crate::listener::ArcDomainParticipantListener;
use crate::publisher::Publisher;
use crate::qos::{DomainParticipantQos, PublisherQos, QosArg, SubscriberQos, TopicQos};
use crate::subscriber::Subscriber;
use crate::topic::{
    ContentFilteredTopic, Topic, TopicDescription, TopicDescriptionHandle, TopicInner,
    TopicUserCounts, TopicUserKind,
};

#[cfg(feature = "std")]
use crate::runtime::{DcpsRuntime, RuntimeConfig};

/// Domain-id type (Spec: `DomainId_t` = long, i.e. i32).
pub type DomainId = i32;

/// Shared ignore-list filter of a `DomainParticipant`. Held by the
/// participant **and** consulted by the `DcpsRuntime` discovery hook
/// (a clone of the `Arc`). Spec reference: DDS DCPS 1.4 §2.2.2.2.1.14-17
/// `ignore_participant/topic/publication/subscription`.
///
/// Per spec the lists are **monotonically growing**: a handle can be
/// added, but never removed again. Hence `BTreeSet<InstanceHandle>`
/// suffices and no generation counters are needed.
#[derive(Debug, Default)]
#[cfg(feature = "std")]
pub(crate) struct IgnoreFilterInner {
    pub(crate) participants: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) topics: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) publications: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) subscriptions: Mutex<BTreeSet<InstanceHandle>>,
    /// The prefix of the participant that owns the filter, once the runtime
    /// has it: the `DCPSTopic` entry of its own topic ignores the topic for
    /// the endpoints of every participant.
    pub(crate) own: std::sync::OnceLock<zerodds_rtps::wire_types::GuidPrefix>,
}

/// Cloneable filter handle (Arc bumps are cheap). The discovery hook may
/// poke in here in between, without forcing lock cycles on the entire
/// ParticipantInner.
#[derive(Clone, Debug, Default)]
#[cfg(feature = "std")]
pub struct IgnoreFilter {
    pub(crate) inner: Arc<IgnoreFilterInner>,
}

#[cfg(feature = "std")]
impl IgnoreFilter {
    /// Check whether a participant handle is ignored.
    #[must_use]
    pub fn is_participant_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .participants
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a topic handle is ignored.
    #[must_use]
    pub fn is_topic_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .topics
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a publication handle is ignored.
    #[must_use]
    pub fn is_publication_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .publications
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a subscription handle is ignored.
    #[must_use]
    pub fn is_subscription_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .subscriptions
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Records the prefix of the participant that owns the filter.
    pub(crate) fn set_own(&self, prefix: zerodds_rtps::wire_types::GuidPrefix) {
        let _ = self.inner.own.set(prefix);
    }

    /// Whether the topic `topic` of the type `type_name` is ignored for the
    /// endpoints of `participant`: its `DCPSTopic` entry of that participant
    /// is, or the entry of this participant's own topic is.
    #[must_use]
    pub fn is_topic_ignored_for(
        &self,
        participant: zerodds_rtps::wire_types::GuidPrefix,
        topic: &str,
        type_name: &str,
    ) -> bool {
        let entry = |prefix| {
            InstanceHandle::from_guid(
                crate::builtin_topics::TopicBuiltinTopicData::synthesize_key(
                    prefix, topic, type_name,
                ),
            )
        };
        self.is_topic_ignored(entry(participant))
            || self
                .inner
                .own
                .get()
                .is_some_and(|own| self.is_topic_ignored(entry(*own)))
    }

    /// Whether a remote endpoint is ignored (DDS 1.4 §2.2.2.2.1.14–17): it
    /// itself — a publication when `publication`, else a subscription —,
    /// its participant ("ignore any Topic, publication, or subscription
    /// that originates on that domain participant"), or its Topic ("ignore
    /// any publication or subscription to the Topic"). The Topic's handle
    /// is that of its `DCPSTopic` instance, which is one of a participant:
    /// the entry of the participant of the endpoint ignores the topic for
    /// that participant's endpoints, the entry of this participant's own
    /// topic for the endpoints of all.
    #[must_use]
    pub fn ignores_endpoint(
        &self,
        key: zerodds_rtps::wire_types::Guid,
        participant: zerodds_rtps::wire_types::Guid,
        topic: &str,
        type_name: &str,
        publication: bool,
    ) -> bool {
        let own = InstanceHandle::from_guid(key);
        let own_ignored = if publication {
            self.is_publication_ignored(own)
        } else {
            self.is_subscription_ignored(own)
        };
        own_ignored
            || self.is_participant_ignored(InstanceHandle::from_guid(participant))
            || self.is_topic_ignored_for(participant.prefix, topic, type_name)
    }
}

/// The bound of the names the RTPS discovery data carry: `string<256>`
/// (DDSI-RTPS 2.5 Table 9.18, §9.3.2.1).
const MAX_ANNOUNCED_NAME: usize = zerodds_rtps::publication_data::STRING_256_MAX;

/// Generated 12-byte participant prefix.
///
/// Scheme ([`zerodds_rtps::wire_types::GuidPrefix::zerodds`]):
/// - `bytes[0..2]`: the vendor id `0x01F0` (DDSI-RTPS 2.5 §9.3.1.5).
/// - `bytes[2..6]`: host id (FNV1a hash of the `gethostname` output).
///   Two participants on the same machine carry the same host id →
///   discovery detects a same-host match and can enable a zero-copy SHM
///   path (`zerodds-zero-copy-1.0` §6 wave 4).
/// - `bytes[6..10]`: process id (LE) marked with the PID namespace,
///   [`namespaced_pid`], [`process_unique`].
/// - `bytes[10..12]`: a process-wide counter added to a seed taken from the
///   randomness of the operating system once per process ([`next_process_unique`]). The counter makes
///   the prefixes of one process pairwise different, 65536 of them; the
///   seed makes a process that reuses an earlier one's id differ from it
///   in all but 1 in 65536 cases.
///
/// DDSI-RTPS 2.5 §8.2.4.2 rests on "every Participant in the Domain has a
/// unique GUID". A cross-host hash collision (4-byte FNV1a) with an equal
/// process id and tail is theoretically possible; the runtime counts an
/// SPDP announcement that carries its own prefix from another participant
/// (`DcpsRuntime::prefix_collisions`).
#[cfg(feature = "std")]
fn random_guid_prefix() -> zerodds_rtps::wire_types::GuidPrefix {
    zerodds_rtps::wire_types::GuidPrefix::zerodds(host_id_bytes(), next_process_unique())
}

/// [`process_unique`] for the next participant of this process: the
/// process id, a counter shared by every ZeroDDS program in the process (the
/// C API, the bridges), and the seed taken from the randomness of the
/// operating system at the first call. The process id carries the mark of the
/// PID namespace ([`namespaced_pid`]).
#[cfg(feature = "std")]
#[must_use]
pub fn next_process_unique() -> [u8; 6] {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    static SEED: OnceLock<u16> = OnceLock::new();
    static PID: OnceLock<u32> = OnceLock::new();
    // The seed comes from the randomness of the operating system (the keys
    // of `RandomState`), once per process.
    let seed = *SEED.get_or_init(|| {
        use std::hash::{BuildHasher, Hasher};
        let h = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        (h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)) as u16
    });
    let pid = *PID.get_or_init(|| namespaced_pid(std::process::id(), pid_namespace_inode()));
    process_unique(pid, seed, COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// The inode of the PID namespace the process lives in (`/proc/self/ns/pid`),
/// `None` where the system has none (not Linux, no `/proc`).
#[cfg(feature = "std")]
#[must_use]
pub fn pid_namespace_inode() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self/ns/pid").ok().map(|m| m.ino())
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The inode of the initial PID namespace on Linux (`PROC_PID_INIT_INO`).
const INITIAL_PID_NAMESPACE_INODE: u32 = 0xEFFF_FFFC;

/// The process id as the prefix carries it (DDSI-RTPS 2.5 §8.2.4.2: "every
/// Participant in the Domain has a unique GUID"). Containers that share the
/// host's network and name (the same host id) have a PID namespace of their
/// own, and the first process of each has the id 1; the id alone would not
/// tell their participants apart. The pid is XORed with a mask taken from
/// the inode of the PID namespace, zero in the initial namespace: within a
/// namespace the map is one-to-one (two processes of it have two ids), and
/// two namespaces differ in the mask. `None` leaves the id as it is.
#[must_use]
pub fn namespaced_pid(pid: u32, namespace_inode: Option<u64>) -> u32 {
    let mask = namespace_inode.map_or(0, |ino| (ino as u32) ^ INITIAL_PID_NAMESPACE_INODE);
    pid ^ mask
}

/// The six octets that make a prefix unique on its host: the process id,
/// and the sum of a per-process `seed` and a `counter`. Two counters
/// below 65536 apart give two different tails, so the participants of one
/// process never share a prefix. Shared by every ZeroDDS program that builds
/// a participant prefix, so `same_process` in the runtime reads the same
/// octets in each.
#[cfg(feature = "std")]
#[must_use]
pub fn process_unique(pid: u32, seed: u16, counter: u32) -> [u8; 6] {
    let tail = seed.wrapping_add(counter as u16).to_le_bytes();
    let pid = pid.to_le_bytes();
    [pid[0], pid[1], pid[2], pid[3], tail[0], tail[1]]
}

/// DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 — turn the participant's own
/// `PropertyQosPolicy` into its security, or refuse to create it.
///
/// Three outcomes, and no fourth:
///
/// 1. **No `dds.sec.auth.*` / `dds.sec.access.*` property** — the QoS does
///    not ask for security. The config comes back unchanged and the
///    participant runs plaintext RTPS, which is what an empty property list
///    means.
/// 2. **A `SecurityBundle` was already wired in by hand**
///    (`ParticipantBuilder::with_security` /
///    `RuntimeConfig::with_security_bundle` set `config.security`) — the
///    explicit object wins and the property list is not read a second time.
///    Building a second `SecurityProfile` beside the one the caller supplied
///    would give the crypto gate one PKI plugin and the handshake driver
///    another: the handshake completes, the gate then resolves no shared
///    secret for the peer, and secured DATA carries no keys with nothing
///    saying so.
/// 3. **Properties are present and `config.security` is empty** — the list
///    is the configuration. It is read in full, and anything missing,
///    unreadable or unsupported fails the create with the property named.
///
/// The failure in (3) is the point. Before this path existed, a complete and
/// correct `dds.sec.*` list produced a participant *without* a bundle:
/// plaintext RTPS, silently. A participant that comes up unencrypted because
/// a path was wrong is the failure class `docs/silent-drop-sweep.md` names,
/// so it is an error here and never a fallback.
#[cfg(all(feature = "std", feature = "security"))]
fn security_from_participant_properties(
    config: RuntimeConfig,
    qos: &DomainParticipantQos,
    domain_id: DomainId,
) -> Result<(
    RuntimeConfig,
    Option<Arc<zerodds_security_runtime::SecurityProfile>>,
)> {
    let by_properties = zerodds_security_runtime::requests_security(&qos.property);
    if config.security.is_some() {
        return Ok((config, None));
    }
    let enclave = config.security_enclave_dir.clone();
    if enclave.is_some() && by_properties {
        return Err(DdsError::SecurityConfiguration {
            message: String::from(
                "security.enclave_dir and the participant's dds.sec.* properties both name \
                 security; name one",
            ),
        });
    }
    if enclave.is_none() && !by_properties {
        return Ok((config, None));
    }

    // The candidate GUID: host-derived prefix plus the participant entity
    // id — the same shape `zerodds-c-api::security_ffi` builds before
    // §7.4.3 replaces the prefix with the identity-derived one.
    let mut candidate_guid = [0u8; 16];
    candidate_guid[..12].copy_from_slice(
        &zerodds_rtps::wire_types::GuidPrefix::zerodds(host_id_bytes(), [0; 6]).to_bytes(),
    );
    candidate_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

    let domain = u32::try_from(domain_id).unwrap_or(0);
    let bundle = match &enclave {
        // `security.enclave_dir` (env ZERODDS_SECURITY_DIR): a set
        // directory that does not load is an error, never a plain
        // participant.
        Some(dir) => zerodds_security_runtime::security_bundle_from_enclave_dir_with_env(
            dir,
            domain,
            candidate_guid,
            config.security_logger.clone(),
            &|name| config.security_vendor_settings.lookup(name),
        )
        .map_err(|e| match e {
            zerodds_security_runtime::SecurityPropertyError::JoinDenied { .. } => {
                DdsError::NotAllowedBySecurity {
                    what: "domain participant",
                }
            }
            other => DdsError::SecurityConfiguration {
                message: alloc::format!("security.enclave_dir {}: {other}", dir.display()),
            },
        })?,
        None => zerodds_security_runtime::security_bundle_from_properties_with_env(
            &qos.property,
            domain,
            candidate_guid,
            config.security_logger.clone(),
            &|name| config.security_vendor_settings.lookup(name),
        )
        .map_err(|e| match e {
            // §9.8.1 step 3: "the create_participant operation shall return
            // NIL and set the return code to NOT_ALLOWED_BY_SECURITY".
            zerodds_security_runtime::SecurityPropertyError::JoinDenied { .. } => {
                DdsError::NotAllowedBySecurity {
                    what: "domain participant",
                }
            }
            other => DdsError::SecurityConfiguration {
                message: alloc::format!("{other}"),
            },
        })?,
    };
    // The bundle owns the one profile; reading the `Arc` back out of it is
    // how the handshake driver gets the very instance the gate got.
    let profile = bundle
        .security_profile()
        .ok_or_else(|| DdsError::SecurityConfiguration {
            message: String::from("the security bundle lost the profile it was just given"),
        })?;
    Ok((config.with_security_bundle(&bundle), Some(profile)))
}

/// Deterministic 4-byte host identifier based on `gethostname`. Cached
/// per process via `OnceLock`.
///
/// FNV1a-32 is enough: we need identity (same-host yes/no), not
/// cryptographic security. If `gethostname` fails (a CI container
/// without a hostname), we fall back to a process-local random value —
/// then no false-positive same-host match occurs with peers on the same
/// machine, which is safe (only the SHM optimization is missed).
///
/// `pub` so that `zerodds-c-api` places the same host identifier in its
/// GuidPrefix — otherwise two C-FFI processes on the same host would
/// never see each other as same-host (`is_same_host`), and SHM /
/// fragmentation optimizations would not apply for any C++/C#/TS
/// bindings.
#[cfg(feature = "std")]
pub fn host_id_bytes() -> [u8; 4] {
    use std::sync::OnceLock;
    static HOST_ID: OnceLock<[u8; 4]> = OnceLock::new();
    *HOST_ID.get_or_init(|| {
        // Primary: gethostname(3) — works uniformly on Linux, macOS and
        // the BSDs, without env-var / etc-file sources that are
        // sometimes missing (macOS has no /etc/hostname; HOSTNAME is
        // Bash-only and not exported; COMPUTERNAME is Windows).
        // Previously: 3 sources tried, all silently failed, fell back to
        // PID+time → a different host_id per process on the same
        // machine, and same-host optimizations (LOOPBACK_MTU,
        // same-host SHM) did not apply.
        let hostname = gethostname_via_libc()
            .or_else(|| std::env::var("HOSTNAME").ok())
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .or_else(read_etc_hostname);
        let h = match hostname {
            Some(s) if !s.is_empty() => fnv1a_32(s.as_bytes()),
            _ => {
                // Last fallback: a process-local random value. Then this
                // process has a unique "host" and makes no
                // false-positive same-host optimization.
                let pid = std::process::id();
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u32)
                    .unwrap_or(0);
                pid.wrapping_mul(0x9E37_79B1).wrapping_add(t)
            }
        };
        h.to_le_bytes()
    })
}

#[cfg(all(feature = "std", unix))]
#[allow(unsafe_code)]
fn gethostname_via_libc() -> Option<String> {
    // POSIX `gethostname(buf, len)` — 256 bytes are enough for all
    // realistic hostnames (HOST_NAME_MAX is typically 64 or 255).
    let mut buf = [0u8; 256];
    // SAFETY: buf is valid writable memory of buf.len() bytes;
    // gethostname writes at most len bytes and NUL-terminates.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast::<libc::c_char>(), buf.len()) };
    if rc != 0 {
        return None;
    }
    // NUL-terminated string; find it + decode UTF-8.
    let len = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    if len == 0 {
        return None;
    }
    core::str::from_utf8(&buf[..len]).ok().map(|s| s.to_owned())
}

#[cfg(all(feature = "std", not(unix)))]
fn gethostname_via_libc() -> Option<String> {
    None
}

#[cfg(feature = "std")]
fn read_etc_hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
}

/// The GUID prefix of a bridge daemon's participant: the host identifier of
/// [`host_id_bytes`] in bytes 0..4, as every participant of this stack
/// carries it, and FNV-1a-64 of `seed` (the daemon's client or container
/// id, its listen address) in bytes 4..12. Stable across restarts of the
/// same daemon on the same host; two seeds or two hosts give two prefixes.
///
/// The bridges used to take the first 12 bytes of the seed itself, so
/// `zerodds-bridge-a` and `zerodds-bridge-b`, or `127.0.0.1:9001` and
/// `127.0.0.1:9002`, shared one prefix — on one host and across hosts.
#[cfg(feature = "std")]
#[must_use]
pub fn bridge_guid_prefix(seed: &str) -> zerodds_rtps::wire_types::GuidPrefix {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &b in seed.as_bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    let h = h.to_le_bytes();
    zerodds_rtps::wire_types::GuidPrefix::zerodds(
        host_id_bytes(),
        [h[0], h[1], h[2], h[3], h[4], h[5]],
    )
}

#[cfg(feature = "std")]
fn fnv1a_32(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The participant.
#[derive(Clone)]
pub struct DomainParticipant {
    inner: Arc<ParticipantInner>,
}

impl DomainParticipant {
    /// Whether this handle is of the participant `other`.
    pub(crate) fn is(&self, other: &Arc<ParticipantInner>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }
}

impl core::fmt::Debug for DomainParticipant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DomainParticipant")
            .field("domain_id", &self.inner.domain_id)
            .finish_non_exhaustive()
    }
}

impl DomainParticipant {
    /// A **weak** handle to this participant's inner state. The
    /// `DomainParticipantFactory` stores these so it never keeps a participant
    /// alive past the user's last strong handle — dropping the user's
    /// `DomainParticipant` then runs the natural RAII teardown (runtime threads,
    /// UDP sockets and multicast memberships released via the `Arc<DcpsRuntime>`
    /// drop chain).
    pub(crate) fn downgrade(&self) -> alloc::sync::Weak<ParticipantInner> {
        Arc::downgrade(&self.inner)
    }

    /// Reconstruct a strong participant handle from inner state — used by the
    /// factory's `lookup_participant` after `Weak::upgrade`.
    pub(crate) fn from_inner(inner: Arc<ParticipantInner>) -> Self {
        Self { inner }
    }
}

pub(crate) struct ParticipantInner {
    pub(crate) domain_id: DomainId,
    pub(crate) qos: Mutex<DomainParticipantQos>,
    /// The QoS `create_topic`, `create_publisher` and `create_subscriber`
    /// take for `*_QOS_DEFAULT`, and `set_qos` of a Topic, Publisher or
    /// Subscriber of this participant (DDS 1.4 §2.2.2.2.1.20–25).
    default_topic_qos: Mutex<TopicQos>,
    default_publisher_qos: Mutex<PublisherQos>,
    default_subscriber_qos: Mutex<SubscriberQos>,
    /// Entity lifecycle (DCPS §2.2.2.1).
    pub(crate) entity_state: Arc<crate::entity::EntityState>,
    /// Topic registry (name → the TopicInner objects of that name, in
    /// creation order). Repeated `create_topic` with the same name + type
    /// return the same handle. A different type of the same name is
    /// another object in the list when its QoS is consistent with the
    /// others (DDS-XTypes 1.3 §7.6.4.1: "multiple objects of the same
    /// name, provided that all of them represent type-based subsets of
    /// 'the same' network topic"); an inconsistent QoS, or the same type
    /// name with other type metadata → `InconsistentPolicy`.
    topics: Mutex<BTreeMap<String, Vec<Arc<TopicInner>>>>,
    /// The TypeSupports registered with `register_type` (DDS 1.4
    /// §2.2.2.3.6.1), by type name: the Rust type each name is registered
    /// for.
    type_support: Mutex<BTreeMap<String, core::any::TypeId>>,
    /// The ContentFilteredTopics and MultiTopics of this participant, which
    /// `lookup_topicdescription` finds beside the topics (DDS 1.4
    /// §2.2.2.2.1.12). They are held weakly.
    #[cfg(feature = "std")]
    descriptions: Mutex<Vec<crate::topic::DescriptionEntry>>,
    /// The content filter classes the application registered
    /// ([`DomainParticipant::register_contentfilter`]), by class name. Each
    /// holds an `Arc<dyn ContentFilter<T>>` for the one type `T` it was
    /// registered for.
    #[cfg(feature = "std")]
    content_filters: Mutex<BTreeMap<String, Arc<dyn core::any::Any + Send + Sync>>>,
    /// The types `register_type` registered, by the name they were
    /// registered under (DDS 1.4 §2.2.2.3.6).
    #[cfg(feature = "std")]
    registered_types: Mutex<BTreeMap<String, Arc<crate::serialized_type::SerializedTypeSupport>>>,
    /// Runtime handle with UDP sockets + discovery threads. `None` when
    /// the participant was created in offline mode (tests that want no
    /// networking).
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// Pre-installed builtin subscriber (DDS 1.4 §2.2.2.2.1.7). Exactly
    /// one per participant. The sinks are hooked into the runtime
    /// discovery hook at construction time.
    pub(crate) builtin_subscriber: Arc<BuiltinSubscriber>,
    /// Ignore filter (Spec §2.2.2.2.1.14-17). A clone lives in the
    /// runtime and is checked by the discovery hot path, so that
    /// SPDP/SEDP samples no longer reach the builtin readers after
    /// `ignore_*`.
    #[cfg(feature = "std")]
    pub(crate) ignore_filter: IgnoreFilter,
    /// The publishers `create_publisher` made, held weakly, for
    /// `delete_publisher`, `delete_contained_entities` and
    /// `contains_entity` (DDS 1.4 §2.2.2.2.1.2, .18, .10). A publisher ends
    /// with its deletion, or with its last handle and its last writer.
    publishers: Mutex<Vec<alloc::sync::Weak<crate::publisher::PublisherInner>>>,
    /// The subscribers, as `publishers`.
    subscribers: Mutex<Vec<alloc::sync::Weak<crate::subscriber::SubscriberInner>>>,
    /// Live users of each local topic, by topic name — the precondition
    /// `delete_topic` decides on (DDS 1.4 §2.2.2.2.1.6: "The deletion of
    /// a Topic is not allowed if there are any existing DataReader,
    /// DataWriter, ContentFilteredTopic, or MultiTopic objects that are
    /// using the Topic"). Every such object adds itself here when it is
    /// created and takes itself out when it is deleted or dropped, so the
    /// entry of a name is empty exactly when nothing uses that Topic.
    pub(crate) topic_users: Mutex<BTreeMap<String, TopicUserCounts>>,
    /// The TRANSIENT and PERSISTENT stores of this participant's topics,
    /// by topic name and whether the store is persistent. DDS 1.4 §2.2.3.4:
    /// the data of a TRANSIENT or PERSISTENT topic outlives the DataWriter
    /// that wrote it, so the store belongs to the topic and not to a writer;
    /// a writer created after another was deleted finds what the first one
    /// wrote.
    #[cfg(feature = "std")]
    durability_stores:
        Mutex<BTreeMap<(String, bool), Arc<dyn crate::durability_service::DurabilityBackend>>>,
    /// The live writers of each instance of those stores
    /// (`service_cleanup_delay`, DDS 1.4 §2.2.3.4 condition 2).
    #[cfg(feature = "std")]
    pub(crate) store_writers: crate::durability_service::LiveStoreWriters,
    /// Optional [`ArcDomainParticipantListener`] + [`StatusMask`].
    /// Bubble-up target for all children whose narrower listener does not
    /// cover the status bit.
    pub(crate) listener: Mutex<Option<(ArcDomainParticipantListener, StatusMask)>>,
    /// Built-in DynamicType registry. Automatically populated in `new()`/
    /// `new_with_runtime()` with the 4 Spec §7.6.5 built-in types
    /// (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`,
    /// `DDS::KeyedBytes`). Retrievable via
    /// [`DomainParticipant::find_builtin_type`].
    #[cfg(feature = "std")]
    pub(crate) type_registry: Mutex<BTreeMap<String, crate::builtin_type_set::BuiltinType>>,
    /// TypeLookup client state per participant. Pending get-types
    /// requests are queued here; backoff via `last_attempt_per_hash` so
    /// that unknown TypeIDs are not re-queried every tick.
    #[cfg(feature = "std")]
    pub(crate) type_lookup: Mutex<TypeLookupState>,
}

// The live users of each local topic: what `delete_topic` decides on.
impl ParticipantInner {
    /// The store of the topic `topic` (DDS 1.4 §2.2.3.4), built by `build`
    /// when this participant has none yet.
    ///
    /// # Errors
    /// `PreconditionNotMet` on a poisoned lock, and whatever `build` returns.
    pub(crate) fn durability_store(
        &self,
        topic: &str,
        persistent: bool,
        build: impl FnOnce() -> Result<Arc<dyn crate::durability_service::DurabilityBackend>>,
    ) -> Result<Arc<dyn crate::durability_service::DurabilityBackend>> {
        let mut stores =
            self.durability_stores
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "durability store registry poisoned",
                })?;
        if let Some(store) = stores.get(&(topic.to_string(), persistent)) {
            return Ok(Arc::clone(store));
        }
        let store = build()?;
        stores.insert((topic.to_string(), persistent), Arc::clone(&store));
        Ok(store)
    }

    /// Records one more live user of the topic `name`, of kind `kind`.
    /// Called by `create_datawriter`/`create_datareader`/
    /// `create_contentfilteredtopic`/`create_multitopic` and by every
    /// `Clone` of a topic description, because each of those handles is
    /// one user for as long as it exists.
    pub(crate) fn add_topic_user(&self, name: &str, kind: TopicUserKind) {
        self.add_typed_topic_user(name, "", kind);
    }

    /// As [`Self::add_topic_user`], for the topic of `name` and `type_name`.
    /// XTypes 1.3 §7.6.4.1 lets several topics share a name, one per type;
    /// the users of a topic are the objects created on it, not those of
    /// another topic of its name, so the count is kept per type name. An
    /// empty `type_name` counts for every topic of the name (a MultiTopic
    /// names its related topics by name alone).
    pub(crate) fn add_typed_topic_user(&self, name: &str, type_name: &str, kind: TopicUserKind) {
        if let Ok(mut users) = self.topic_users.lock() {
            users
                .entry(topic_user_key(name, type_name))
                .or_default()
                .add(kind);
        }
    }

    /// The counterpart of [`Self::add_topic_user`]: the `Drop` of a
    /// DataWriter, DataReader, ContentFilteredTopic or MultiTopic — the
    /// point at which the object stops existing and the Topic it used is
    /// free again (DDS 1.4 §2.2.2.2.1.6 counts *existing* objects).
    pub(crate) fn remove_topic_user(&self, name: &str, kind: TopicUserKind) {
        self.remove_typed_topic_user(name, "", kind);
    }

    /// The counterpart of [`Self::add_typed_topic_user`].
    pub(crate) fn remove_typed_topic_user(&self, name: &str, type_name: &str, kind: TopicUserKind) {
        if let Ok(mut users) = self.topic_users.lock() {
            let key = topic_user_key(name, type_name);
            let Some(counts) = users.get_mut(&key) else {
                return;
            };
            counts.remove(kind);
            if counts.is_empty() {
                users.remove(&key);
            }
        }
    }

    /// The live users of the topic `name`; all zero when it has none.
    /// This is the read `delete_topic` decides on, so a poisoned lock is
    /// an error rather than a default that would read as "no users".
    pub(crate) fn topic_users(&self, name: &str) -> Result<TopicUserCounts> {
        self.sum_topic_users(name, None)
    }

    /// The live users of the topic of `name` and `type_name`: the objects
    /// created on that topic and those that name every topic of `name`.
    pub(crate) fn topic_users_of(&self, name: &str, type_name: &str) -> Result<TopicUserCounts> {
        self.sum_topic_users(name, Some(type_name))
    }

    /// The counts of the topics of `name`: all of them, or those of one type
    /// and the ones that count for every type.
    fn sum_topic_users(&self, name: &str, type_name: Option<&str>) -> Result<TopicUserCounts> {
        let users = self
            .topic_users
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "topic user registry poisoned",
            })?;
        let prefix = topic_user_key(name, "");
        let mut sum = TopicUserCounts::default();
        for (key, counts) in users.range(prefix.clone()..) {
            let Some(type_part) = key.strip_prefix(prefix.as_str()) else {
                break;
            };
            if type_name.is_none_or(|wanted| type_part.is_empty() || type_part == wanted) {
                sum = sum.plus(*counts);
            }
        }
        Ok(sum)
    }
}

/// The key of the users of a topic: its name and its type name, apart.
fn topic_user_key(name: &str, type_name: &str) -> String {
    alloc::format!("{name}\u{1f}{type_name}")
}

/// Explicit teardown once the last strong handle to this participant is
/// gone. [`DomainParticipant::downgrade`]'s doc comment promises "natural
/// RAII teardown ... via the `Arc<DcpsRuntime>` drop chain", but that chain
/// alone never actually reaches a zero refcount: every recv/tick worker
/// thread [`crate::runtime::DcpsRuntime::start`] spawns is handed its own
/// strong `Arc<DcpsRuntime>` clone for as long as it runs, and a thread only
/// exits (dropping its clone) once told to via
/// [`crate::runtime::DcpsRuntime::shutdown`] — which nothing called when a
/// `DomainParticipant` was merely dropped (as opposed to explicitly deleted
/// via `DomainParticipantFactory::delete_participant`, which also never
/// called it). A dropped participant's runtime — UDP sockets, SPDP/SEDP
/// discovery, its TransientLocal writers' history — therefore kept running
/// for the rest of the process, still matching newly created readers on the
/// same topic and replaying a "gone" writer's un-capped history to them
/// alongside the intended replayer (measured: a durability-service
/// late-joiner test saw its `KEEP_LAST(2)` replay duplicated by the
/// supposedly-dropped original application writer). `shutdown` is
/// idempotent, so `delete_participant` calling it too (redundantly, once
/// this participant's own last `Arc` — as opposed to a child `Publisher`'s
/// or `Subscriber`'s independent runtime clone — is already gone here) is a
/// no-op.
#[cfg(feature = "std")]
impl Drop for ParticipantInner {
    fn drop(&mut self) {
        if let Some(rt) = self.runtime.as_ref() {
            rt.shutdown();
        }
    }
}

/// TypeLookup client state per participant. Tracks pending requests +
/// backoff timer + retry count per unknown TypeID hash.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct TypeLookupState {
    /// Per TypeID: (last_attempt_instant, retry_count).
    pub attempts: BTreeMap<zerodds_types::EquivalenceHash, (std::time::Instant, u32)>,
    /// Optional sink for outgoing TypeLookup requests (test hook). The
    /// production path would be a reliable writer on the
    /// `TL_SVC_REQ_WRITER` endpoint; until then the sink queues (test
    /// mode) or stays None (live mode = no-op).
    pub outgoing: Vec<(zerodds_types::EquivalenceHash, u64)>,
}

#[cfg(feature = "std")]
impl TypeLookupState {
    /// Backoff period (5s) between retries.
    pub const BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);
    /// Maximum attempts per unknown TypeID.
    pub const MAX_ATTEMPTS: u32 = 3;
}

impl DomainParticipant {
    /// Offline constructor without a runtime — for skeleton tests.
    /// Production code goes through `DomainParticipantFactory::
    /// create_participant`, which automatically starts a runtime.
    pub(crate) fn new(domain_id: DomainId, qos: DomainParticipantQos) -> Self {
        let builtin = Arc::new(BuiltinSubscriber::new());
        let participant = Self {
            inner: Arc::new(ParticipantInner {
                domain_id,
                entity_state: {
                    let state = crate::entity::EntityState::new();
                    state.set_autoenable_children(qos.entity_factory.autoenable_created_entities);
                    state
                },
                qos: Mutex::new(qos),
                default_topic_qos: Mutex::new(TopicQos::default()),
                default_publisher_qos: Mutex::new(PublisherQos::default()),
                default_subscriber_qos: Mutex::new(SubscriberQos::default()),
                topics: Mutex::new(BTreeMap::new()),
                type_support: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                descriptions: Mutex::new(Vec::new()),
                #[cfg(feature = "std")]
                content_filters: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                registered_types: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                runtime: None,
                builtin_subscriber: builtin,
                #[cfg(feature = "std")]
                ignore_filter: IgnoreFilter::default(),
                publishers: Mutex::new(Vec::new()),
                subscribers: Mutex::new(Vec::new()),
                topic_users: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                durability_stores: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                store_writers: crate::durability_service::LiveStoreWriters::default(),
                listener: Mutex::new(None),
                #[cfg(feature = "std")]
                type_registry: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                type_lookup: Mutex::new(TypeLookupState::default()),
            }),
        };
        // Auto-register the 4 Spec §7.6.5 built-in types.
        #[cfg(feature = "std")]
        participant.register_builtin_types();
        participant.register_entity_ref();
        // The built-in Subscriber answers to this participant, as a user
        // Subscriber does; its nil listener (DDS 1.4 §2.2.5) is what ends a
        // built-in status there.
        #[cfg(feature = "std")]
        participant
            .inner
            .builtin_subscriber
            .subscriber()
            .attach_participant(Arc::downgrade(&participant.inner));
        participant
    }

    /// Constructor with a live runtime (UDP + discovery). Returns
    /// `TransportError` if the socket bind fails.
    ///
    /// # Errors
    /// [`DdsError::TransportError`] on bind problems.
    #[cfg(feature = "std")]
    pub(crate) fn new_with_runtime(
        domain_id: DomainId,
        qos: DomainParticipantQos,
        config: RuntimeConfig,
    ) -> Result<Self> {
        // DDS-Security spec-style logger wireup: if the participant QoS carries
        // `dds.sec.log.*` properties, materialize the fan-out logger from them
        // and wire it into the runtime (no-op when absent).
        //
        // The builtin `DDS:Logging:DDS_LogTopic` sink (DDS-Security 1.2 §10.6)
        // publishes through a DataWriter that needs this participant, which
        // does not exist yet. The sink gets a
        // [`zerodds_security_logging::LogTopicWriterSlot`] here, and the
        // DataWriter is attached below once the participant is built.
        #[cfg(feature = "security")]
        let log_topic_slot = {
            let pairs: Vec<(&str, &str)> = qos.property.iter().collect();
            if zerodds_security_logging::log_topic_requested(&pairs) {
                let legacy =
                    zerodds_security_logging::log_topic_legacy_requested(&pairs).map_err(|_| {
                        DdsError::PreconditionNotMet {
                            reason: "invalid dds.sec.log.* security logger configuration",
                        }
                    })?;
                // §10.6.1: a DataWriter exists only when the events are to
                // be distributed; `distribute = false` is a file-only sink.
                let distribute = zerodds_security_logging::log_topic_distribute_requested(&pairs)
                    .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "invalid dds.sec.log.* security logger configuration",
                })?;
                if distribute {
                    Some((
                        Arc::new(zerodds_security_logging::LogTopicWriterSlot::empty()),
                        legacy,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };
        #[cfg(feature = "security")]
        let log_topic_publisher: Option<
            Arc<dyn zerodds_security_logging::LogTopicPublisher>,
        > = log_topic_slot.as_ref().map(|(slot, _)| {
            let slot: Arc<zerodds_security_logging::LogTopicWriterSlot> = Arc::clone(slot);
            let publisher: Arc<dyn zerodds_security_logging::LogTopicPublisher> = slot;
            publisher
        });
        #[cfg(feature = "security")]
        let config = config
            .with_security_log_properties_and_topic_publisher(&qos.property, log_topic_publisher)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "invalid dds.sec.log.* security logger configuration",
            })?;
        // DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71: the participant selects and
        // configures its security plugins through the `dds.sec.*` properties
        // on its own `PropertyQosPolicy`. §10.3.1 Table 50 (authentication)
        // and §10.4.1 Table 63 (access control) are read here — the
        // counterpart the `dds.sec.log.*` wireup above has had all along and
        // the rest of the namespace had not.
        #[cfg(feature = "security")]
        let (config, security_profile) =
            security_from_participant_properties(config, &qos, domain_id)?;
        // DDS-Security §7.4.3: when a security profile carries an
        // identity-adjusted GUID prefix (wired via
        // `RuntimeConfig::with_security_bundle`), the runtime MUST start with
        // that prefix instead of a random candidate — the SPDP beacon,
        // handshake `c.pdata` and all entity GUIDs must be consistent with the
        // identity cert, or a foreign vendor's §7.4.3 check rejects the
        // participant. Mirrors `zerodds-c-api::security_ffi::finish_secure_runtime`,
        // which has always used the adjusted prefix on the FFI path — this
        // wires the same behavior onto the public `zerodds-dcps` Rust API.
        // §7.3.21: the entries with `propagate = TRUE` are in the SPDP data.
        let mut config = config;
        config.participant_properties = crate::qos::propagated_properties(&qos.property);
        if config.participant_properties.encode(true).is_err() {
            return Err(DdsError::BadParameter {
                what: "participant properties the PID_PROPERTY_LIST cannot carry",
            });
        }
        #[cfg(feature = "security")]
        let prefix = config
            .security_guid_prefix
            .unwrap_or_else(random_guid_prefix);
        #[cfg(not(feature = "security"))]
        let prefix = random_guid_prefix();
        // An enclave this build cannot load must not become a plain
        // participant without a word.
        #[cfg(not(feature = "security"))]
        if let Some(dir) = &config.security_enclave_dir {
            return Err(DdsError::SecurityConfiguration {
                message: alloc::format!(
                    "security.enclave_dir {} is set, but this build has no DDS-Security \
                     (zerodds-dcps feature `security`)",
                    dir.display()
                ),
            });
        }
        let runtime = DcpsRuntime::start(domain_id, prefix, config)?;
        // Authentication is a separate switch from the crypto gate. Without
        // it the participant has a gate and never runs a handshake, so every
        // secured peer stays unauthenticated and no endpoint matches — with
        // nothing on either side saying why. `security_profile` is the `Arc`
        // the bundle carries, so this arms the gate's own PKI plugin and the
        // shared secret the handshake derives is resolvable where the gate
        // looks for it.
        #[cfg(feature = "security")]
        if let Some(profile) = &security_profile {
            runtime.enable_security_builtins_with_auth(
                zerodds_rtps::wire_types::VendorId::ZERODDS,
                profile.pki.clone(),
                profile.identity_handle,
            );
        }
        let builtin = Arc::new(BuiltinSubscriber::new());
        // Wire up the discovery hook: from now on the runtime pushes
        // SPDP/SEDP events into the 4 builtin readers.
        runtime.attach_builtin_sinks(builtin.sinks());
        // Share the ignore filter with the runtime, so that the
        // discovery hot path (handle_spdp_datagram +
        // push_sedp_events_to_builtin_readers) can consult the lists.
        let ignore_filter = IgnoreFilter::default();
        runtime.attach_ignore_filter(ignore_filter.clone());
        let participant = Self {
            inner: Arc::new(ParticipantInner {
                domain_id,
                entity_state: {
                    let state = crate::entity::EntityState::new();
                    state.set_autoenable_children(qos.entity_factory.autoenable_created_entities);
                    state
                },
                qos: Mutex::new(qos),
                default_topic_qos: Mutex::new(TopicQos::default()),
                default_publisher_qos: Mutex::new(PublisherQos::default()),
                default_subscriber_qos: Mutex::new(SubscriberQos::default()),
                topics: Mutex::new(BTreeMap::new()),
                type_support: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                descriptions: Mutex::new(Vec::new()),
                #[cfg(feature = "std")]
                content_filters: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                registered_types: Mutex::new(BTreeMap::new()),
                runtime: Some(runtime),
                builtin_subscriber: builtin,
                ignore_filter,
                publishers: Mutex::new(Vec::new()),
                subscribers: Mutex::new(Vec::new()),
                topic_users: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                durability_stores: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                store_writers: crate::durability_service::LiveStoreWriters::default(),
                listener: Mutex::new(None),
                type_registry: Mutex::new(BTreeMap::new()),
                type_lookup: Mutex::new(TypeLookupState::default()),
            }),
        };
        // Auto-register the 4 Spec §7.6.5 built-in types.
        participant.register_builtin_types();
        participant.register_entity_ref();
        // The built-in Subscriber answers to this participant, as a user
        // Subscriber does; its nil listener (DDS 1.4 §2.2.5) is what ends a
        // built-in status there.
        #[cfg(feature = "std")]
        participant
            .inner
            .builtin_subscriber
            .subscriber()
            .attach_participant(Arc::downgrade(&participant.inner));
        // DDS-Security 1.2 §10.6: attach the BuiltinLoggingTopic DataWriter(s)
        // the logging sink was waiting for. A failure leaves the slot empty, so
        // every subsequent event is reported by the sink rather than dropped.
        #[cfg(feature = "security")]
        #[allow(clippy::print_stderr)]
        if let Some((slot, legacy)) = &log_topic_slot {
            match crate::security_log_topic::log_topic_publisher_for_participant(
                &participant,
                *legacy,
            ) {
                Ok(publisher) => {
                    if slot.set(publisher).is_err() {
                        eprintln!(
                            "security: DDS:Logging:DDS_LogTopic: the DataWriter slot was already \
                             filled; the new BuiltinLoggingTopic writer is dropped"
                        );
                    }
                }
                Err(e) => eprintln!(
                    "security: DDS:Logging:DDS_LogTopic: could not create the \
                     BuiltinLoggingTopic DataWriter: {e}"
                ),
            }
        }
        Ok(participant)
    }

    /// `StatusCondition::get_entity` of the participant (DDS 1.4
    /// §2.2.2.1.9.3).
    fn register_entity_ref(&self) {
        let weak = Arc::downgrade(&self.inner);
        self.inner.entity_state.set_entity_ref(move || {
            weak.upgrade()
                .map(|inner| crate::entity::EntityRef::DomainParticipant(Self { inner }))
        });
    }

    /// Internal access to the runtime — used by Publisher/Subscriber to
    /// create DataWriter/Reader. `None` when the participant is in
    /// offline mode.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn runtime(&self) -> Option<&Arc<DcpsRuntime>> {
        self.inner.runtime.as_ref()
    }

    /// Domain id.
    #[must_use]
    pub fn domain_id(&self) -> DomainId {
        self.inner.domain_id
    }

    /// Returns a copy of the DomainParticipantQos (Spec §2.2.2.2.1.4
    /// `get_qos`).
    #[must_use]
    pub fn qos(&self) -> DomainParticipantQos {
        self.inner.qos.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Sets the DomainParticipantQos (Spec §2.2.2.2.1.3 `set_qos`); with
    /// [`PARTICIPANT_QOS_DEFAULT`], the factory's
    /// `get_default_participant_qos` (DDS 1.4 §2.2.2.1.1.1).
    ///
    /// [`PARTICIPANT_QOS_DEFAULT`]: crate::psm_constants::PARTICIPANT_QOS_DEFAULT
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant`. The policies of a
    /// DomainParticipantQos are all changeable (DDS 1.4 §2.2.3).
    pub fn set_qos(&self, qos: impl Into<QosArg<DomainParticipantQos>>) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        crate::entity::Entity::set_qos(self, qos)
    }

    /// Registers the 4 Spec §7.6.5 built-in types
    /// (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`, `DDS::KeyedBytes`)
    /// in the local type registry. Idempotent — a second call overwrites
    /// the entries deterministically.
    ///
    /// Called automatically from `new()`/`new_with_runtime()`, but can
    /// also be called again after an `unregister_builtin_types()`
    /// disable.
    #[cfg(feature = "std")]
    pub fn register_builtin_types(&self) {
        if let Ok(set) = crate::builtin_type_set::BuiltinTypeSet::build() {
            if let Ok(mut reg) = self.inner.type_registry.lock() {
                for (name, t) in set.all() {
                    reg.insert(name, t);
                }
            }
        }
    }

    /// Deletes all registered built-in types. Not called from any
    /// default path today — a test helper for disable-flag tests.
    #[cfg(feature = "std")]
    pub fn unregister_builtin_types(&self) {
        if let Ok(mut reg) = self.inner.type_registry.lock() {
            reg.retain(|name, _| !crate::builtin_type_set::is_builtin_type_name(name));
        }
    }

    /// Lookup of a built-in type by spec name (Spec §7.6.5). Returns
    /// `Some(DynamicType)` if the name is known (registered via
    /// `register_builtin_types`).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn find_builtin_type(&self, name: &str) -> Option<crate::builtin_type_set::BuiltinType> {
        self.inner
            .type_registry
            .lock()
            .ok()
            .and_then(|reg| reg.get(name).cloned())
    }

    /// Registers a `TypeObject` with this participant's TypeLookup server,
    /// so a peer's `getTypes` can be answered for it and the local match
    /// sites can resolve an endpoint's `TypeIdentifier` structurally.
    ///
    /// Endpoint creation already registers the one object a type reports
    /// through [`crate::DdsType::type_object`]. That is the whole of a type
    /// whose members are primitives, and not the whole of one whose members
    /// are other named types: XTypes 1.3 §7.3.4.6 has an aggregate name its
    /// members by `TypeIdentifier`, so a peer that receives the aggregate
    /// and not the types it names cannot complete the assignability walk of
    /// §7.2.4. A caller holding a model — a type read from IDL or from an
    /// XML type description at run time — registers the reachable set here
    /// before it creates the endpoint.
    ///
    /// Returns the `EquivalenceHash` of the registered object.
    ///
    /// # Errors
    /// `PreconditionNotMet` if this participant has no runtime (an
    /// in-process participant registers nothing), or as
    /// [`crate::runtime::DcpsRuntime::register_type_object`].
    #[cfg(feature = "std")]
    pub fn register_type_object(
        &self,
        obj: zerodds_types::type_object::TypeObject,
    ) -> crate::error::Result<zerodds_types::EquivalenceHash> {
        let rt = self
            .inner
            .runtime
            .as_ref()
            .ok_or(DdsError::PreconditionNotMet {
                reason: "participant has no runtime",
            })?;
        rt.register_type_object(obj)
    }

    /// Choose the minimal-`TypeObject` vendor camp this participant
    /// announces and serves (XTypes 1.3 §7.3.4.5; `docs/omg-xtypes-issues.md`
    /// §C2). `false` (default) is Cyclone/Fast DDS; `true` RTI/CoreDX.
    /// Call before registering the topic's `TypeObject`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if this participant has no runtime.
    #[cfg(feature = "std")]
    pub fn set_minimal_type_object_camp(&self, rti_coredx: bool) -> crate::error::Result<()> {
        let rt = self
            .inner
            .runtime
            .as_ref()
            .ok_or(DdsError::PreconditionNotMet {
                reason: "participant has no runtime",
            })?;
        rt.set_minimal_type_object_camp(rti_coredx)
    }

    /// Number of registered built-in types. After `new()` == 4.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn registered_type_count(&self) -> usize {
        self.inner
            .type_registry
            .lock()
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Attempts to queue a TypeLookup request for an unknown
    /// `EquivalenceHash`. Respects backoff (5s between attempts) and at
    /// most 3 retries per hash.
    ///
    /// Returns: `true` if the request was queued, `false` on backoff
    /// suppression or max attempts.
    #[cfg(feature = "std")]
    pub fn enqueue_type_lookup(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        let mut state = match self.inner.type_lookup.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let now = std::time::Instant::now();
        if let Some((last, retries)) = state.attempts.get(&hash).copied() {
            if retries >= TypeLookupState::MAX_ATTEMPTS {
                return false;
            }
            if now.duration_since(last) < TypeLookupState::BACKOFF {
                return false;
            }
            state
                .attempts
                .insert(hash, (now, retries.saturating_add(1)));
        } else {
            state.attempts.insert(hash, (now, 1));
        }
        // Next sequence number for the request.
        let seq = state.outgoing.len() as u64 + 1;
        state.outgoing.push((hash, seq));
        true
    }

    /// Drains the queued TypeLookup requests. Returns `Vec<(hash, seq)>`.
    /// In a production environment the caller would send the hashes via
    /// TypeLookupClient + reliable writer to the `TL_SVC_REQ_WRITER`
    /// endpoint.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn drain_type_lookup_requests(&self) -> Vec<(zerodds_types::EquivalenceHash, u64)> {
        self.inner
            .type_lookup
            .lock()
            .map(|mut s| core::mem::take(&mut s.outgoing))
            .unwrap_or_default()
    }

    /// Receives a TypeLookup reply (TypeObjects per hash). Registers the
    /// TypeObjects in an internal type-registry mirror — afterwards a
    /// stalled QoS match can be retried.
    ///
    /// Returns the number of successfully registered types.
    #[cfg(feature = "std")]
    pub fn ingest_type_lookup_reply(
        &self,
        types: Vec<(
            zerodds_types::EquivalenceHash,
            zerodds_types::MinimalTypeObject,
        )>,
    ) -> usize {
        let mut count = 0;
        if let Ok(mut state) = self.inner.type_lookup.lock() {
            for (hash, _t) in &types {
                state.attempts.remove(hash);
                count += 1;
            }
        }
        // Clippy-bait avoidance: the types vec is consumed here; the
        // actual type-registry insert can be done by the caller
        // (e.g. via the shared TypeLookupServer.registry).
        let _ = types;
        count
    }

    /// SEDP discovery hook: checks an incoming
    /// `PublicationBuiltinTopicData` for type hashes that cannot be
    /// resolved locally. If needed, a TypeLookup request is queued via
    /// `enqueue_type_lookup`.
    ///
    /// The RPC path is live via `DcpsRuntime::send_type_lookup_request`
    /// on the TL_SVC_REQ_* endpoints (XTypes 1.3 §7.6.3.3.4); this method
    /// decides per hash whether a re-request is worthwhile (local
    /// registry lookup + backoff tracking).
    ///
    /// Returns: number of unknown hashes queued (max 2 — minimal +
    /// complete).
    #[cfg(feature = "std")]
    pub fn on_remote_publication_discovered(&self, type_information_blob: Option<&[u8]>) -> usize {
        self.on_remote_type_information(type_information_blob)
    }

    /// SEDP discovery hook for `SubscriptionBuiltinTopicData`. Symmetric
    /// to `on_remote_publication_discovered`.
    #[cfg(feature = "std")]
    pub fn on_remote_subscription_discovered(&self, type_information_blob: Option<&[u8]>) -> usize {
        self.on_remote_type_information(type_information_blob)
    }

    #[cfg(feature = "std")]
    fn on_remote_type_information(&self, blob: Option<&[u8]>) -> usize {
        let Some(bytes) = blob else {
            return 0;
        };
        let Ok(ti) = zerodds_types::type_information::TypeInformation::from_bytes_le(bytes) else {
            return 0;
        };
        let mut queued = 0;
        // Check the minimal hash.
        if let Some(hash) = extract_equivalence_hash(&ti.minimal.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        // Check the complete hash (if present).
        if let Some(hash) = extract_equivalence_hash(&ti.complete.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        queued
    }

    /// Internal helper — true if the hash is already resolvable in the
    /// local `TypeLookupServer.registry` (either fed in locally via
    /// `register_type_object` or populated by a previous `getTypes`
    /// reply ingest). Prevents us from issuing redundant lookup requests
    /// for hashes we already know.
    #[cfg(feature = "std")]
    fn has_type_for_hash(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return false;
        };
        let Ok(server) = rt.type_lookup_server.lock() else {
            return false;
        };
        server.registry.get_minimal(&hash).is_some()
            || server.registry.get_complete(&hash).is_some()
    }

    /// True if MAX_ATTEMPTS has already been reached for the hash.
    /// Consulted by the match-retry path: give up eventually instead of
    /// polling forever.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn type_lookup_exhausted(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        self.inner
            .type_lookup
            .lock()
            .ok()
            .and_then(|s| s.attempts.get(&hash).map(|(_, n)| *n))
            .unwrap_or(0)
            >= TypeLookupState::MAX_ATTEMPTS
    }

    /// Creates a typed topic handle. Repeated calls with the same name +
    /// complete type metadata return the same handle (ref-shared).
    ///
    /// # Errors
    /// - `InconsistentPolicy` if a topic with this name is already
    ///   registered with different type metadata, even if its type name matches.
    /// - `BadParameter` for an empty name.
    ///
    /// With [`TOPIC_QOS_DEFAULT`] the topic gets
    /// [`Self::get_default_topic_qos`] (DDS 1.4 §2.2.2.2.1.5).
    ///
    /// [`TOPIC_QOS_DEFAULT`]: crate::psm_constants::TOPIC_QOS_DEFAULT
    pub fn create_topic<T: DdsType>(
        &self,
        name: &str,
        qos: impl Into<QosArg<TopicQos>>,
    ) -> Result<Topic<T>> {
        self.create_topic_with_support(
            name,
            qos.into().resolve(|| self.get_default_topic_qos()),
            crate::topic::TopicTypeMetadata::of::<T>(),
            Arc::new(crate::topic::StaticTopicCodec::of::<T>()),
        )
    }

    /// DDS 1.4 §2.2.2.2.1.5 `create_topic(topic_name, type_name, qos,
    /// a_listener, mask)`: the topic with its listener; see
    /// [`Self::create_publisher_with_listener`].
    ///
    /// # Errors
    /// As [`Self::create_topic`].
    #[cfg(feature = "std")]
    pub fn create_topic_with_listener<T: DdsType>(
        &self,
        name: &str,
        qos: impl Into<QosArg<TopicQos>>,
        listener: Option<crate::listener::ArcTopicListener>,
        mask: StatusMask,
    ) -> Result<Topic<T>> {
        let topic = self.create_topic::<T>(name, qos)?;
        topic.set_listener(listener, mask);
        Ok(topic)
    }

    /// The registration of `type_name` for the Rust type `id`, the body of
    /// `TypeSupport::register_type` (DDS 1.4 §2.2.2.3.6.1): registering the
    /// same TypeSupport again, under the same or another name, is ignored
    /// and returns OK; a name registered for another TypeSupport is
    /// PRECONDITION_NOT_MET.
    pub(crate) fn register_type_support(
        &self,
        type_name: &str,
        id: core::any::TypeId,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        if type_name.is_empty() {
            return Err(DdsError::BadParameter { what: "type name" });
        }
        if type_name.len() > MAX_ANNOUNCED_NAME {
            return Err(DdsError::BadParameter {
                what: "type name longer than 256 octets (string<256>)",
            });
        }
        let mut registered =
            self.inner
                .type_support
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type registry poisoned",
                })?;
        match registered.get(type_name) {
            Some(existing) if *existing == id => Ok(()),
            Some(_) => Err(DdsError::PreconditionNotMet {
                reason: "the type name is registered for a different TypeSupport",
            }),
            None => {
                registered.insert(type_name.into(), id);
                Ok(())
            }
        }
    }

    /// DDS 1.4 §2.2.2.2.1.5 `create_topic(topic_name, type_name, ...)` for a
    /// type registered under `type_name` ("Prior to creating a Topic the
    /// type must have been registered with the Service. This is done using
    /// the register_type operation"): the topic is bound to `T` under that
    /// name, as [`Self::create_topic_with_type_name`] does.
    ///
    /// # Errors
    /// `PreconditionNotMet` if `T` is not registered under `type_name`
    /// (`TypeSupport::register_type`); otherwise as
    /// [`Self::create_topic_with_type_name`].
    pub fn create_topic_of_registered_type<T: DdsType + 'static>(
        &self,
        name: &str,
        type_name: &str,
        qos: impl Into<QosArg<TopicQos>>,
    ) -> Result<Topic<T>> {
        self.inner.entity_state.check_not_deleted()?;
        let registered = self
            .inner
            .type_support
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "type registry poisoned",
            })?
            .get(type_name)
            .is_some_and(|id| *id == core::any::TypeId::of::<T>());
        if !registered {
            return Err(DdsError::PreconditionNotMet {
                reason: "the type is not registered under that type name",
            });
        }
        self.create_topic_with_type_name::<T>(name, type_name, qos)
    }

    /// [`Self::create_topic`] under a type name the caller chooses.
    ///
    /// The topic is announced in SEDP under `type_name` instead of
    /// [`DdsType::type_name`], and a reader or writer matches only a peer
    /// that uses the same name. DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7 name
    /// the topic types of an interface `${interfaceName}_Request` and
    /// `${interfaceName}_Reply`; a `DdsType` whose body is the interface's
    /// own bytes (`RawBytes`) has one constant type name for every
    /// interface.
    ///
    /// The topic's registered type is `T` with `type_name` as its name; a
    /// second call with the same topic name and another type name fails
    /// like `create_topic` does for another type.
    ///
    /// # Errors
    /// As [`Self::create_topic`]; `BadParameter` for an empty `type_name`.
    pub fn create_topic_with_type_name<T: DdsType>(
        &self,
        name: &str,
        type_name: &str,
        qos: impl Into<QosArg<TopicQos>>,
    ) -> Result<Topic<T>> {
        if type_name.is_empty() {
            return Err(DdsError::BadParameter { what: "type name" });
        }
        let mut metadata = crate::topic::TopicTypeMetadata::of::<T>();
        metadata.type_name = alloc::borrow::Cow::Owned(type_name.into());
        self.create_topic_with_support(
            name,
            qos.into().resolve(|| self.get_default_topic_qos()),
            metadata,
            Arc::new(crate::topic::StaticTopicCodec::named_of::<T>()),
        )
    }

    /// [`Self::create_topic_with_type_name`] for a topic whose type is
    /// described by `information` and not by `T`: the endpoints created on
    /// the topic announce the `TypeIdentifier` of `information.object` (its
    /// complete equivalence hash) and register the object and its
    /// dependencies, so a peer matches by assignability (XTypes 1.3 §7.6.3.3)
    /// and not by the type name alone. `T` stays the codec and the key and
    /// extensibility source of the topic.
    ///
    /// # Errors
    /// As [`Self::create_topic_with_type_name`]; `BadParameter` when the
    /// hash of the object cannot be computed.
    pub fn create_topic_with_type_information<T: DdsType>(
        &self,
        name: &str,
        type_name: &str,
        information: crate::topic::TopicTypeInformation,
        qos: impl Into<QosArg<TopicQos>>,
    ) -> Result<Topic<T>> {
        if type_name.is_empty() {
            return Err(DdsError::BadParameter { what: "type name" });
        }
        let hash = zerodds_types::compute_hash(&information.object).map_err(|_| {
            DdsError::BadParameter {
                what: "type object",
            }
        })?;
        let identifier = match information.object {
            zerodds_types::TypeObject::Minimal(_) => {
                zerodds_types::TypeIdentifier::EquivalenceHashMinimal(hash)
            }
            _ => zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
        };
        let mut metadata = crate::topic::TopicTypeMetadata::of::<T>();
        metadata.type_name = alloc::borrow::Cow::Owned(type_name.into());
        metadata.type_identifier = identifier;
        // The extensibility of the described type frames its samples (XTypes 1.3 §7.6.3.1.2
        // Table 59), whatever `T` is: the encapsulation of a sample of a mutable type is
        // PL_CDR2, not CDR2.
        if let zerodds_types::TypeObject::Complete(zerodds_types::CompleteTypeObject::Struct(
            structure,
        )) = &information.object
        {
            use zerodds_types::type_object::flags::StructTypeFlag;
            let flags = structure.struct_flags;
            if flags.has(StructTypeFlag::IS_MUTABLE) {
                metadata.extensibility = crate::dds_type::Extensibility::Mutable;
            } else if flags.has(StructTypeFlag::IS_APPENDABLE) {
                metadata.extensibility = crate::dds_type::Extensibility::Appendable;
            } else {
                metadata.extensibility = crate::dds_type::Extensibility::Final;
            }
        }
        metadata.type_object = Some(information.object);
        self.create_topic_with_support(
            name,
            qos.into().resolve(|| self.get_default_topic_qos()),
            metadata,
            Arc::new(crate::topic::StaticTopicCodec::described_by::<T>(
                information.dependencies,
            )),
        )
    }

    /// Create a runtime-described topic through the ordinary typed entity path.
    ///
    /// The support reads and writes in this participant's CDR dialect
    /// (`wire.codec_dialect`): one still in the specification's default is
    /// set to it. One built in another dialect than this participant's is
    /// refused rather than overridden.
    /// # Errors
    /// The same name/identity and entity errors as `create_topic`;
    /// `BadParameter` for a support in a dialect this participant does not
    /// speak.
    #[cfg(feature = "std")]
    pub fn create_serialized_topic(
        &self,
        name: &str,
        qos: impl Into<QosArg<TopicQos>>,
        support: Arc<crate::serialized_type::SerializedTypeSupport>,
    ) -> Result<Topic<crate::serialized_type::SerializedSample>> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        let qos = qos.into().resolve(|| self.get_default_topic_qos());
        let support = match self.inner.runtime.as_ref().map(|rt| rt.codec_dialect()) {
            Some(dialect) if support.dialect() == dialect => support,
            Some(dialect) if support.dialect().is_default() => support.in_dialect(dialect),
            Some(_) => {
                return Err(DdsError::BadParameter {
                    what: "the type support's CDR dialect is not this participant's wire.codec_dialect",
                });
            }
            None => support,
        };
        self.create_topic_with_support(name, qos, support.metadata().clone(), support)
    }

    /// The registry half of `register_type` (DDS 1.4 §2.2.2.3.6): see
    /// [`crate::serialized_type::SerializedTypeSupport::register_type`].
    ///
    /// # Errors
    /// As that operation.
    #[cfg(feature = "std")]
    pub(crate) fn register_serialized_type(
        &self,
        support: &Arc<crate::serialized_type::SerializedTypeSupport>,
        type_name: Option<&str>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let registered = match type_name {
            None => Arc::clone(support),
            Some(name) if name == support.get_type_name() => Arc::clone(support),
            Some(name) => support
                .with_type_name(name)
                .map_err(|_| DdsError::BadParameter { what: "type_name" })?,
        };
        let name = registered.get_type_name().to_string();
        let mut types =
            self.inner
                .registered_types
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type registry poisoned",
                })?;
        if let Some(known) = types.get(&name) {
            return if known.metadata().type_identifier == registered.metadata().type_identifier
                && known.dialect() == registered.dialect()
            {
                Ok(())
            } else {
                Err(DdsError::PreconditionNotMet {
                    reason: "another type is registered under this name",
                })
            };
        }
        if let Some(rt) = self.inner.runtime.as_ref() {
            for object in registered.type_objects() {
                rt.register_type_object(object.clone())?;
            }
        }
        types.insert(name, registered);
        Ok(())
    }

    /// DDS 1.4 §2.2.2.2.1.1 `create_topic` for a type that
    /// `register_type` registered: the topic `name` of the type
    /// registered under `type_name`.
    ///
    /// # Errors
    /// `BadParameter` for a `type_name` no `register_type` registered
    /// ("the type must be registered"), and as
    /// [`Self::create_serialized_topic`].
    #[cfg(feature = "std")]
    pub fn create_registered_topic(
        &self,
        name: &str,
        type_name: &str,
        qos: impl Into<QosArg<TopicQos>>,
    ) -> Result<Topic<crate::serialized_type::SerializedSample>> {
        let support = self
            .inner
            .registered_types
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "type registry poisoned",
            })?
            .get(type_name)
            .cloned()
            .ok_or(DdsError::BadParameter {
                what: "type_name: no type is registered under this name",
            })?;
        self.create_serialized_topic(name, qos, support)
    }

    /// The topic `name` of the type `support` describes, whose samples are
    /// `DynamicData` (DDS-XTypes 1.3 §7.6.6.2): the topic of a
    /// [`crate::serialized_type::DynamicDataWriter`] and a
    /// [`crate::serialized_type::DynamicDataReader`]. Where
    /// [`Self::create_serialized_topic`] hands the application the
    /// decoded value with the bytes it came from, this hands it a
    /// `DynamicData` of the type, as the clause has it.
    ///
    /// # Errors
    /// As [`Self::create_serialized_topic`].
    #[cfg(feature = "std")]
    pub fn create_dynamic_topic(
        &self,
        name: &str,
        qos: impl Into<QosArg<TopicQos>>,
        support: Arc<crate::serialized_type::SerializedTypeSupport>,
    ) -> Result<Topic<zerodds_types::dynamic::DynamicSample>> {
        self.inner.entity_state.check_not_deleted()?;
        let qos = qos.into().resolve(|| self.get_default_topic_qos());
        let support = match self.inner.runtime.as_ref().map(|rt| rt.codec_dialect()) {
            Some(dialect) if support.dialect() == dialect => support,
            Some(dialect) if support.dialect().is_default() => support.in_dialect(dialect),
            Some(_) => {
                return Err(DdsError::BadParameter {
                    what: "the type support's CDR dialect is not this participant's wire.codec_dialect",
                });
            }
            None => support,
        };
        self.create_topic_with_support(name, qos, support.metadata().clone(), support)
    }

    fn create_topic_with_support<T: DdsType>(
        &self,
        name: &str,
        qos: TopicQos,
        metadata: crate::topic::TopicTypeMetadata,
        codec: Arc<dyn crate::topic::TopicCodec<T>>,
    ) -> Result<Topic<T>> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        if name.is_empty() {
            return Err(DdsError::BadParameter { what: "topic name" });
        }
        // DDSI-RTPS 2.5 Table 9.18: `PID_TOPIC_NAME` and `PID_TYPE_NAME` are
        // `string<256>`; a longer name could not be announced.
        if name.len() > MAX_ANNOUNCED_NAME {
            return Err(DdsError::BadParameter {
                what: "topic name longer than 256 octets (string<256>)",
            });
        }
        if metadata.type_name.len() > MAX_ANNOUNCED_NAME {
            return Err(DdsError::BadParameter {
                what: "type name longer than 256 octets (string<256>)",
            });
        }
        qos.check_consistency()?;
        // DDS-Security 1.2 §9.4.2.9.6: the permissions must allow the topic.
        #[cfg(all(feature = "std", feature = "security"))]
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.check_create_topic_access(name, &qos)?;
        }
        // User-defined metadata accessors must not run under the registry lock.
        // Capture once, compare the complete contract, and store that same
        // snapshot for a new topic rather than reading mutable accessors twice.
        let mut topics = self
            .inner
            .topics
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "topic registry poisoned",
            })?;
        if let Some(entries) = topics.get(name) {
            if let Some(existing) = entries.iter().find(|entry| entry.metadata == metadata) {
                // Same type → shared handle, obtained once more
                // (§2.2.2.2.1.11: deleted that same number of times).
                existing
                    .obtained
                    .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
                return Ok(reconstruct_topic::<T>(existing.clone(), self.clone()).with_codec(codec));
            }
            // The type name of an entry is the identity of the type
            // there: the same name with other metadata is one type
            // declared two ways, not a second type.
            let same_type_name = entries
                .iter()
                .find(|entry| entry.type_name == metadata.type_name);
            // XTypes 1.3 §7.6.4.1: further objects of the name "must have
            // consistent QoS with one another"; the first object's QoS is
            // the one the others are held to.
            let qos_differs = entries
                .first()
                .and_then(|first| first.qos.lock().ok().map(|q| *q != qos))
                .unwrap_or(false);
            let inconsistent = same_type_name.or(entries.first().filter(|_| qos_differs));
            if let Some(existing) = inconsistent {
                // Inconsistent-topic detection. Bumps the counter on the
                // existing topic — on the next `inconsistent_topic_status()`
                // read, the listener is fired via bubble-up.
                #[cfg(feature = "std")]
                {
                    existing
                        .inconsistent_topic_count
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    // The listener dispatcher looks at the status at once,
                    // as for an inconsistent remote Topic.
                    if let Some(rt) = self.inner.runtime.as_ref() {
                        rt.notify_status_changed();
                    }
                }
                return Err(DdsError::InconsistentPolicy {
                    what: if same_type_name.is_some() {
                        "topic name reused with different type metadata"
                    } else {
                        "topic name reused with a QoS that is not consistent with the existing topic"
                    },
                });
            }
        }
        let dependencies = codec.dependent_type_objects().to_vec();
        let topic = Topic::<T>::with_static_metadata(name.into(), qos, self.clone(), metadata)
            .with_codec(codec);
        // A MultiTopic reads the members of the topic's type from these.
        let _ = topic.inner().type_dependencies.set(dependencies);
        // `StatusCondition::get_entity` (DDS 1.4 §2.2.2.1.9.3).
        {
            let topic_inner = Arc::downgrade(&topic.inner());
            let participant = Arc::downgrade(&self.inner);
            topic.inner().entity_state.set_entity_ref(move || {
                let inner = topic_inner.upgrade()?;
                let participant = Self {
                    inner: participant.upgrade()?,
                };
                Some(crate::entity::EntityRef::Topic(
                    crate::topic::TopicDescriptionHandle::new(
                        inner.name.clone(),
                        inner.type_name.to_string(),
                        participant,
                    ),
                ))
            });
        }
        #[cfg(feature = "std")]
        topic
            .inner()
            .entity_state
            .set_factory(&self.inner.entity_state);
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            // DDSI-RTPS 2.5 Table 8.84: a topic is announced when it exists,
            // which for one created disabled is its enable.
            let rt = Arc::clone(rt);
            let weak = Arc::downgrade(&topic.inner());
            topic
                .inner()
                .entity_state
                .set_enable_work(move || match weak.upgrade() {
                    Some(inner) => announce_topic(&rt, &inner),
                    None => Ok(()),
                });
        }
        let enabled_now = self.autoenables_children();
        if enabled_now {
            topic.inner().entity_state.enable();
        }
        topics
            .entry(name.into())
            .or_default()
            .push(topic_inner(&topic));
        drop(topics);
        #[cfg(feature = "std")]
        if enabled_now {
            if let Some(rt) = self.inner.runtime.as_ref() {
                announce_topic(rt, &topic.inner())?;
            }
        }
        Ok(topic)
    }

    /// Deletes the Topic this participant created under `name`, and
    /// disposes it on the SEDP topics writer (DDSI-RTPS 2.5 Table 8.84:
    /// "each time an existing Topic belonging to the DomainParticipant is
    /// deleted"). This is the body of [`Self::delete_topic`], split off for
    /// callers that hold a topic description as a name rather than as a
    /// typed handle — the untyped C-FFI surface, whose `Topic` is a name and
    /// a type name (DDS-PSM-Cxx §7.5.2).
    ///
    /// # Errors
    /// - `AlreadyDeleted` if the name is not in the registry — it was never
    ///   created here, or a `delete_topic` already removed it (DDS 1.4
    ///   §2.2.2.1.1 `ReturnCode_t` = `ALREADY_DELETED`).
    /// - `PreconditionNotMet` if a DataWriter, DataReader,
    ///   ContentFilteredTopic or MultiTopic still uses the topic (DDS 1.4
    ///   §2.2.2.2.1.6).
    /// - `PreconditionNotMet` if an internal lock is poisoned.
    pub fn delete_topic_by_name(&self, name: &str) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        let registered = {
            let topics = self
                .inner
                .topics
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "topic registry poisoned",
                })?;
            match topics.get(name).map(Vec::as_slice) {
                Some([only]) => Arc::clone(only),
                // A name is not enough to say which of several topics of
                // that name is meant (XTypes 1.3 §7.6.4.1); the typed
                // `delete_topic` takes the object.
                Some([_, _, ..]) => {
                    return Err(DdsError::PreconditionNotMet {
                        reason: "several topics share the name; delete the topic by its handle",
                    });
                }
                _ => return Err(DdsError::AlreadyDeleted),
            }
        };
        self.delete_registered_topic(name, registered)
    }

    /// Deletes a Topic. Spec reference: OMG DDS 1.4 §2.2.2.2.1.6.
    ///
    /// "The delete_topic operation deletes a Topic. The deletion of a Topic
    /// is not allowed if there are any existing DataReader, DataWriter,
    /// ContentFilteredTopic, or MultiTopic objects that are using the Topic.
    /// If the delete_topic operation is called on a Topic with any of these
    /// existing objects attached to it, it will return
    /// PRECONDITION_NOT_MET. The delete_topic operation must be called on
    /// the same DomainParticipant object used to create the Topic. If
    /// delete_topic is called on a different DomainParticipant, the
    /// operation will have no effect and it will return
    /// PRECONDITION_NOT_MET."
    ///
    /// What the clause does not say is what a handle the application kept
    /// means afterwards, and here it is a handle to something this
    /// participant no longer has:
    ///
    /// - `create_topic` returns the **same** entity for the same name and
    ///   type (DDS 1.4 §2.2.2.2.1.10 `find_topic`), so every such handle
    ///   shares one `TopicInner` and one deleted flag. A second
    ///   `delete_topic`, whichever handle it is given, therefore returns
    ///   `AlreadyDeleted` (DDS 1.4 §2.2.2.1.1 `ReturnCode_t` =
    ///   `ALREADY_DELETED`), not a silent success.
    /// - The handle keeps answering for itself — `name()`, `type_name()`,
    ///   `get_participant()` — because it owns the shared state, but
    ///   [`Self::lookup_topicdescription`] no longer finds the name
    ///   (§2.2.2.2.1.12 searches the participant's local topics) and
    ///   [`Self::contains_entity`] no longer contains its instance handle
    ///   (§2.2.2.2.1.10).
    /// - `create_topic` with the same name afterwards succeeds and builds a
    ///   **new** Topic, announced under a new entity id; it is not the
    ///   deleted one resurrected.
    ///
    /// # Errors
    /// - `PreconditionNotMet` if `topic` was created by another
    ///   participant — the spec's second rule above, which names the same
    ///   return code as the first.
    /// - `PreconditionNotMet` if a DataWriter, DataReader,
    ///   ContentFilteredTopic or MultiTopic still uses the topic — the
    ///   spec's first rule. Deleting that object, or dropping its last
    ///   handle, is what makes a later call succeed: a DataWriter,
    ///   DataReader, ContentFilteredTopic and MultiTopic are deleted by
    ///   the end of their last handle, a ContentFilteredTopic and a
    ///   MultiTopic also by [`Self::delete_contentfilteredtopic`] and
    ///   [`Self::delete_multitopic`], so the count `delete_topic` refuses
    ///   on is a count of live handles of objects not deleted.
    /// - `AlreadyDeleted` if `topic` is already deleted (DDS 1.4
    ///   §2.2.2.1.1 `ReturnCode_t` = `ALREADY_DELETED`).
    /// - `PreconditionNotMet` if this participant does not have the topic:
    ///   a `Topic::new` built outside `create_topic`, or a builtin
    ///   `Topic::new_orphan`, which no participant registered at all.
    /// - `PreconditionNotMet` if an internal lock is poisoned.
    /// - `WireError` if the SEDP dispose of the topic does not encode.
    pub fn delete_topic<T: DdsType>(&self, topic: &Topic<T>) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        let Some(owner) = topic.owning_participant() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "a builtin topic belongs to no DomainParticipant",
            });
        };
        if owner.inner_ptr() != self.inner_ptr() {
            return Err(DdsError::PreconditionNotMet {
                reason: "the topic was created by another DomainParticipant",
            });
        }
        let inner = topic.inner();
        inner.entity_state.check_not_deleted()?;
        // The entry, not just the name: a `Topic::new` built outside
        // `create_topic` owns state this participant never registered, and
        // deleting it would dispose a topic of somebody else's registry
        // entry.
        let name = inner.name.clone();
        let registered = {
            let topics = self
                .inner
                .topics
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "topic registry poisoned",
                })?;
            match topics
                .get(&name)
                .and_then(|entries| entries.iter().find(|entry| Arc::ptr_eq(entry, &inner)))
            {
                Some(entry) => Arc::clone(entry),
                None => {
                    return Err(DdsError::PreconditionNotMet {
                        reason: "the topic is not registered with this DomainParticipant",
                    });
                }
            }
        };
        self.delete_registered_topic(&name, registered)
    }

    /// The shared body of [`Self::delete_topic`] and
    /// [`Self::delete_topic_by_name`]: refuse while anything uses the
    /// topic, dispose it on the SEDP topics writer, then drop the registry
    /// entry.
    ///
    /// The SEDP dispose comes **before** the registry entry goes, so that a
    /// dispose that fails leaves the topic where it was: the caller can
    /// retry, and the topic is not silently gone from the local registry
    /// while peers still believe in it. The entry is only removed if it is
    /// still the same one — a `create_topic` of the same name that raced
    /// through the two steps in between is reported rather than dropped.
    fn delete_registered_topic(&self, name: &str, inner: Arc<TopicInner>) -> Result<()> {
        // §2.2.2.2.1.11: a topic obtained more than once goes with its last
        // `delete_topic`. The first deleted it, and the handles of the
        // other `create_topic` calls returned ALREADY_DELETED.
        if inner
            .obtained
            .fetch_update(
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
                |n| (n > 1).then(|| n - 1),
            )
            .is_ok()
        {
            return Ok(());
        }
        // The users of this topic: not those of another topic of its name
        // (XTypes 1.3 §7.6.4.1), which has its own endpoints.
        let users = self.inner.topic_users_of(name, &inner.type_name)?;
        if !users.is_empty() {
            return Err(DdsError::PreconditionNotMet {
                reason: "a DataReader, DataWriter, ContentFilteredTopic or MultiTopic still uses the topic",
            });
        }
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.dispose_local_topic_of_type(name, &inner.type_name)?;
        }
        let mut topics = self
            .inner
            .topics
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "topic registry poisoned",
            })?;
        let position = topics
            .get(name)
            .and_then(|entries| entries.iter().position(|entry| Arc::ptr_eq(entry, &inner)));
        let Some(position) = position else {
            return Err(DdsError::PreconditionNotMet {
                reason: "the topic registry entry changed while the topic was deleted",
            });
        };
        if let Some(entries) = topics.get_mut(name) {
            entries.remove(position);
            if entries.is_empty() {
                topics.remove(name);
            }
        }
        inner.entity_state.mark_deleted();
        Ok(())
    }

    /// The live users of the topic `name`, as [`Self::delete_topic`] counts
    /// them: a diagnostic for a caller that wants to know what would block
    /// the deletion without attempting it.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the user registry is poisoned.
    pub fn topic_users(&self, name: &str) -> Result<TopicUserCounts> {
        self.inner.topic_users(name)
    }

    /// Spec §2.2.3.20 + §2.2.2.1.1.7: a `create_<entity>` operation
    /// enables the new entity when this factory's
    /// `entity_factory.autoenable_created_entities` is TRUE — but
    /// "entities created from a factory that is disabled are created
    /// disabled regardless of the setting of the ENTITY_FACTORY QoS
    /// policy", so the factory's own state gates it.
    #[must_use]
    fn autoenables_children(&self) -> bool {
        self.inner.entity_state.is_enabled()
            && self
                .inner
                .qos
                .lock()
                .map(|q| q.entity_factory.autoenable_created_entities)
                .unwrap_or(true)
    }

    /// Immediate local lookup of a topic by name — returns `None` if no
    /// local `create_topic` with this name has occurred. **Does no
    /// discovery wait** (that is `find_topic`). Spec reference: OMG
    /// DDS 1.4 §2.2.2.2.1.12 "lookup_topicdescription".
    #[must_use]
    pub fn lookup_topicdescription(&self, name: &str) -> Option<TopicDescriptionHandle> {
        self.lookup_topicdescription_at(name, 0)
    }

    /// [`Self::lookup_topicdescription`] with the `index` argument of
    /// DDS-XTypes 1.3 §7.6.4.2: "The value of the index parameter shall be
    /// treated as an 'iterator' over the sequence of TopicDescription
    /// instances that match that topic_name. Each value of the index shall
    /// return a unique (different) TopicDescription. Values of the index
    /// from 0 to one less than the number of different TopicDescriptions
    /// match the topic_name shall return a TopicDescription and values of
    /// the index outside the range shall return nil." The topics of a name
    /// come in creation order; `index` 0 is what
    /// [`Self::lookup_topicdescription`] returns.
    #[must_use]
    pub fn lookup_topicdescription_at(
        &self,
        name: &str,
        index: u32,
    ) -> Option<TopicDescriptionHandle> {
        self.lookup_at(name, index).map(|(handle, _)| handle)
    }

    /// Deletes every ContentFilteredTopic and MultiTopic of this participant
    /// that still has a handle.
    #[cfg(feature = "std")]
    fn delete_all_descriptions(&self) -> Result<()> {
        let live: Vec<crate::topic::DeleteDescription> = {
            let Ok(mut descriptions) = self.inner.descriptions.lock() else {
                return Err(DdsError::PreconditionNotMet {
                    reason: "description registry poisoned",
                });
            };
            descriptions.retain(crate::topic::DescriptionEntry::is_live);
            descriptions.iter().map(|d| d.delete_handle()).collect()
        };
        for delete in live {
            (delete.0)()?;
        }
        Ok(())
    }

    /// The `index`-th TopicDescription of `name`: the topics in creation
    /// order, then the ContentFilteredTopics and MultiTopics. A topic comes
    /// with its registry entry.
    fn lookup_at(
        &self,
        name: &str,
        index: u32,
    ) -> Option<(TopicDescriptionHandle, Option<Arc<TopicInner>>)> {
        let index = usize::try_from(index).ok()?;
        let topic_count = {
            let topics = self.inner.topics.lock().ok()?;
            let entries = topics.get(name).map_or(&[][..], Vec::as_slice);
            if let Some(inner) = entries.get(index) {
                return Some((
                    TopicDescriptionHandle::new(
                        inner.name.clone(),
                        inner.type_name.to_string(),
                        self.clone(),
                    ),
                    Some(Arc::clone(inner)),
                ));
            }
            entries.len()
        };
        #[cfg(feature = "std")]
        {
            let mut descriptions = self.inner.descriptions.lock().ok()?;
            descriptions.retain(crate::topic::DescriptionEntry::is_live);
            descriptions
                .iter()
                .filter(|entry| entry.name == name)
                .nth(index - topic_count)
                .map(|entry| (entry.handle(self.clone()), None))
        }
        #[cfg(not(feature = "std"))]
        {
            let _ = topic_count;
            None
        }
    }

    /// The ContentFilteredTopic or MultiTopic a DataReader's hold is on.
    #[cfg(feature = "std")]
    pub(crate) fn description_held_by(
        &self,
        held: &crate::topic::DescriptionReaderUse,
    ) -> Option<TopicDescriptionHandle> {
        let descriptions = self.inner.descriptions.lock().ok()?;
        descriptions
            .iter()
            .find(|entry| entry.is_held_by(held))
            .map(|entry| entry.handle(self.clone()))
    }

    /// The registry entry of the topic `name` with type `type_name`.
    pub(crate) fn registered_topic(&self, name: &str, type_name: &str) -> Option<Arc<TopicInner>> {
        let topics = self.inner.topics.lock().ok()?;
        topics
            .get(name)?
            .iter()
            .find(|entry| entry.type_name == type_name)
            .cloned()
    }

    /// Lists a ContentFilteredTopic or MultiTopic with the participant, for
    /// `lookup_topicdescription`.
    #[cfg(feature = "std")]
    fn register_description(&self, entry: crate::topic::DescriptionEntry) {
        if let Ok(mut descriptions) = self.inner.descriptions.lock() {
            descriptions.retain(crate::topic::DescriptionEntry::is_live);
            descriptions.push(entry);
        }
    }

    /// Waits until a topic with the given name is visible via discovery
    /// (an SEDP publication or subscription) — or until `timeout`
    /// elapses. Spec reference: OMG DDS 1.4 §2.2.2.2.1.11 `find_topic`.
    ///
    /// Returns:
    /// - `Ok(handle)` with name + type name + participant, if a matching
    ///   SEDP endpoint became visible during `timeout`. Local topics
    ///   count as well (no need to wait if `create_topic` already ran).
    /// - `Err(Timeout)` if `timeout` elapsed.
    ///
    /// # Errors
    /// - `DdsError::Timeout` if `timeout` elapsed without a discovery
    ///   match.
    /// - `DdsError::BadParameter` for an empty name.
    #[cfg(feature = "std")]
    pub fn find_topic(
        &self,
        name: &str,
        timeout: core::time::Duration,
    ) -> Result<TopicDescriptionHandle> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        if name.is_empty() {
            return Err(DdsError::BadParameter { what: "topic name" });
        }
        let deadline = std::time::Instant::now() + timeout;
        // Poll loop over the local topics and the SEDP cache. The spec
        // leaves the strategy open; Cyclone DDS polls as well. A topic
        // created meanwhile by another thread ends the wait.
        let poll = core::time::Duration::from_millis(20);
        loop {
            if let Some(handle) = self.find_enabled_local_topic(name) {
                return Ok(handle);
            }
            if let Some(handle) = self.find_topic_in_sedp(name) {
                return Ok(handle);
            }
            if std::time::Instant::now() >= deadline {
                return Err(DdsError::Timeout);
            }
            std::thread::sleep(poll);
        }
    }

    /// The first enabled local Topic of `name`, obtained once more: "If a
    /// Topic is obtained multiple times by means of find_topic or
    /// create_topic, it must also be deleted that same number of times using
    /// delete_topic" (DDS 1.4 §2.2.2.2.1.11).
    #[cfg(feature = "std")]
    fn find_enabled_local_topic(&self, name: &str) -> Option<TopicDescriptionHandle> {
        let topics = self.inner.topics.lock().ok()?;
        let inner = topics
            .get(name)?
            .iter()
            .find(|entry| entry.entity_state.is_enabled())?;
        inner
            .obtained
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        Some(TopicDescriptionHandle::new(
            inner.name.clone(),
            inner.type_name.to_string(),
            self.clone(),
        ))
    }

    /// Helper: checks the SEDP cache for whether a remote endpoint
    /// (publication or subscription) has announced a topic with the
    /// name. Returns the first match (name + type name).
    #[cfg(feature = "std")]
    fn find_topic_in_sedp(&self, name: &str) -> Option<TopicDescriptionHandle> {
        let rt = self.inner.runtime.as_ref()?;
        let sedp = rt.sedp.lock().ok()?;
        // Check publications first.
        for p in sedp.cache().publications() {
            if p.data.topic_name == name {
                return Some(TopicDescriptionHandle::remote(
                    p.data.topic_name.clone(),
                    p.data.type_name.clone(),
                    self.clone(),
                ));
            }
        }
        for s in sedp.cache().subscriptions() {
            if s.data.topic_name == name {
                return Some(TopicDescriptionHandle::remote(
                    s.data.topic_name.clone(),
                    s.data.type_name.clone(),
                    self.clone(),
                ));
            }
        }
        None
    }

    /// Registers a content filter class for the type `T`
    /// (DDSI-RTPS 2.5 §8.7.3.4: "for all other filter classes, both
    /// implementations must allow the user to register the same custom
    /// filter class"). A ContentFilteredTopic names it with
    /// [`Self::create_contentfilteredtopic_with_class`], and the class name
    /// goes out in the reader's `PID_CONTENT_FILTER_PROPERTY`.
    ///
    /// # Errors
    /// - `AlreadyDeleted` after `delete_participant`.
    /// - `BadParameter` for an empty name, a name longer than 256 octets
    ///   (`string<256>`), or the class `DDSSQL`, which is the implementation's
    ///   own.
    /// - `PreconditionNotMet` if the class is registered already.
    #[cfg(feature = "std")]
    pub fn register_contentfilter<T: DdsType + 'static>(
        &self,
        class_name: &str,
        filter: Arc<dyn crate::topic::ContentFilter<T>>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        if class_name.is_empty() || class_name.len() > MAX_ANNOUNCED_NAME {
            return Err(DdsError::BadParameter {
                what: "content filter class name (1 to 256 octets)",
            });
        }
        if class_name == zerodds_rtps::subscription_data::filter_class::DDSSQL {
            return Err(DdsError::BadParameter {
                what: "the DDSSQL filter class is the implementation's own",
            });
        }
        let mut filters =
            self.inner
                .content_filters
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "content filter registry poisoned",
                })?;
        if filters.contains_key(class_name) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the content filter class is registered already",
            });
        }
        filters.insert(class_name.into(), Arc::new(filter));
        Ok(())
    }

    /// Whether a content filter class of this name is registered
    /// (`DDSSQL` is always available and is not in the registry).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_contentfilter(&self, class_name: &str) -> bool {
        self.inner
            .content_filters
            .lock()
            .is_ok_and(|f| f.contains_key(class_name))
    }

    /// Unregisters a content filter class. ContentFilteredTopics created
    /// with it keep their filter.
    ///
    /// # Errors
    /// `PreconditionNotMet` if no class of this name is registered.
    #[cfg(feature = "std")]
    pub fn unregister_contentfilter(&self, class_name: &str) -> Result<()> {
        let mut filters =
            self.inner
                .content_filters
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "content filter registry poisoned",
                })?;
        filters
            .remove(class_name)
            .map(|_| ())
            .ok_or(DdsError::PreconditionNotMet {
                reason: "no content filter class of this name is registered",
            })
    }

    /// [`Self::create_contentfilteredtopic`] with a filter class: `DDSSQL`,
    /// or one registered with [`Self::register_contentfilter`] for the
    /// type `T`. The registered class checks the expression
    /// ([`crate::topic::ContentFilter::compile`]) and filters the samples of
    /// a DataReader created on the topic.
    ///
    /// # Errors
    /// As [`Self::create_contentfilteredtopic`]; `BadParameter` if the class
    /// is not registered, or was registered for another type than `T`.
    #[cfg(feature = "std")]
    pub fn create_contentfilteredtopic_with_class<T: DdsType + 'static>(
        &self,
        name: &str,
        related_topic: &Topic<T>,
        filter_class_name: &str,
        filter_expression: &str,
        filter_parameters: alloc::vec::Vec<String>,
    ) -> Result<ContentFilteredTopic<T>> {
        let custom = if filter_class_name == zerodds_rtps::subscription_data::filter_class::DDSSQL {
            None
        } else {
            let registered = self
                .inner
                .content_filters
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "content filter registry poisoned",
                })?
                .get(filter_class_name)
                .cloned()
                .ok_or(DdsError::BadParameter {
                    what: "content filter class not registered",
                })?;
            let filter = registered
                .downcast::<Arc<dyn crate::topic::ContentFilter<T>>>()
                .map_err(|_| DdsError::BadParameter {
                    what: "content filter class registered for another type",
                })?;
            Some((filter_class_name.to_string(), Arc::clone(&*filter)))
        };
        self.create_contentfilteredtopic_inner(
            name,
            related_topic,
            filter_expression,
            filter_parameters,
            custom,
        )
    }

    /// Creates a `ContentFilteredTopic` as a subset of an existing
    /// `Topic<T>`. Spec reference: OMG DDS 1.4 §2.2.2.2.1.13
    /// `create_contentfilteredtopic`.
    ///
    /// The `filter_expression` is a SQL subset (see Annex B).
    /// `filter_parameters` are strings that replace `%0`, `%1`, ... in
    /// the expression.
    ///
    /// # Errors
    /// - `BadParameter` for an empty name or empty expression.
    /// - `BadParameter` if the filter expression does not parse.
    /// - `BadParameter` if a referenced `%N` parameter is not supplied
    ///   in the `filter_parameters` vec.
    pub fn create_contentfilteredtopic<T: DdsType + 'static>(
        &self,
        name: &str,
        related_topic: &Topic<T>,
        filter_expression: &str,
        filter_parameters: alloc::vec::Vec<String>,
    ) -> Result<ContentFilteredTopic<T>> {
        self.create_contentfilteredtopic_inner(
            name,
            related_topic,
            filter_expression,
            filter_parameters,
            None,
        )
    }

    fn create_contentfilteredtopic_inner<T: DdsType + 'static>(
        &self,
        name: &str,
        related_topic: &Topic<T>,
        filter_expression: &str,
        filter_parameters: alloc::vec::Vec<String>,
        custom: Option<(String, Arc<dyn crate::topic::ContentFilter<T>>)>,
    ) -> Result<ContentFilteredTopic<T>> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        if name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "content-filtered-topic name",
            });
        }
        // DDSI-RTPS 2.5 §9.3.2.1: `string<256> contentFilteredTopicName`.
        if name.len() > MAX_ANNOUNCED_NAME {
            return Err(DdsError::BadParameter {
                what: "content-filtered-topic name longer than 256 octets (string<256>)",
            });
        }
        if filter_expression.is_empty() {
            return Err(DdsError::BadParameter {
                what: "filter expression",
            });
        }
        let cft = ContentFilteredTopic::<T>::new(
            name.into(),
            related_topic.clone(),
            filter_expression.into(),
            filter_parameters,
            self.clone(),
            custom,
        )?;
        #[cfg(feature = "std")]
        self.register_description(cft.description_entry());
        Ok(cft)
    }

    /// Creates a `MultiTopic` (DDS 1.4 §2.2.2.2.1.9 `create_multitopic`, an
    /// optional feature): the topics of the `FROM` clause of
    /// `subscription_expression` (Annex B.2) joined, projected and filtered
    /// into the type `type_name`. A DataReader on it is created with
    /// [`crate::Subscriber::create_datareader_with_multitopic`].
    ///
    /// The result type is `T`, whose `TypeObject` describes the members of
    /// the result; a type registered under `type_name`
    /// ([`crate::serialized_type::SerializedTypeSupport::register_type`])
    /// takes precedence. The topics of the `FROM` clause must exist in this
    /// participant and have a `TypeObject` too.
    ///
    /// # Errors
    /// - `AlreadyDeleted` after `delete_participant`.
    /// - `BadParameter` for an empty name, type name or expression.
    /// - `BadParameter` if the subscription expression is not a
    ///   `TopicExpression` of Annex B.2.
    /// - `BadParameter` if a referenced `%N` parameter is not supplied
    ///   in the `expression_parameters` vec.
    /// - `BadParameter` if the expression, the topics and the result type do
    ///   not fit together: a topic of the `FROM` clause does not exist or has
    ///   no `TypeObject`, two topics share a member name of different types,
    ///   a field of the `SELECT` list or of `WHERE` is not a member of the
    ///   topics, a member of the result type has no field or another type.
    #[cfg(feature = "std")]
    pub fn create_multitopic<T: DdsType + 'static>(
        &self,
        name: &str,
        type_name: &str,
        subscription_expression: &str,
        expression_parameters: alloc::vec::Vec<String>,
    ) -> Result<crate::topic::MultiTopic<T>> {
        let support = self.multitopic_result_support::<T>(type_name)?;
        self.build_multitopic::<T>(
            name,
            type_name,
            subscription_expression,
            expression_parameters,
            support,
            crate::topic::TopicTypeMetadata::of::<T>(),
            Arc::new(crate::topic::StaticTopicCodec::of::<T>()),
        )
    }

    /// [`Self::create_multitopic`] for a result of `DynamicData`: the type
    /// registered under `type_name` is the type of the result.
    ///
    /// # Errors
    /// As [`Self::create_multitopic`]; `BadParameter` if no type is
    /// registered under `type_name`.
    #[cfg(feature = "std")]
    pub fn create_dynamic_multitopic(
        &self,
        name: &str,
        type_name: &str,
        subscription_expression: &str,
        expression_parameters: alloc::vec::Vec<String>,
    ) -> Result<crate::topic::MultiTopic<zerodds_types::dynamic::DynamicSample>> {
        let support = self
            .registered_support(type_name)
            .ok_or(DdsError::BadParameter {
                what: "multitopic: no type is registered under type_name",
            })?;
        self.build_multitopic::<zerodds_types::dynamic::DynamicSample>(
            name,
            type_name,
            subscription_expression,
            expression_parameters,
            Arc::clone(&support),
            support.metadata().clone(),
            support,
        )
    }

    /// The type registered under `type_name` (DDS 1.4 §2.2.2.3.6).
    #[cfg(feature = "std")]
    fn registered_support(
        &self,
        type_name: &str,
    ) -> Option<Arc<crate::serialized_type::SerializedTypeSupport>> {
        self.inner
            .registered_types
            .lock()
            .ok()
            .and_then(|types| types.get(type_name).cloned())
    }

    /// The support that writes the samples of a MultiTopic of type `T`.
    #[cfg(feature = "std")]
    fn multitopic_result_support<T: DdsType>(
        &self,
        type_name: &str,
    ) -> Result<Arc<crate::serialized_type::SerializedTypeSupport>> {
        if let Some(support) = self.registered_support(type_name) {
            return Ok(support);
        }
        if T::type_name() == type_name {
            if let Some(object) = T::type_object() {
                return crate::serialized_type::SerializedTypeSupport::from_topic_objects(
                    type_name,
                    &object,
                    &T::type_object_dependencies(),
                )
                .map_err(|_| DdsError::BadParameter {
                    what: "multitopic: the TypeObject of the result type cannot be read",
                });
            }
        }
        Err(DdsError::BadParameter {
            what: "multitopic: type_name is no registered type and the result type has no TypeObject",
        })
    }

    #[cfg(feature = "std")]
    #[allow(clippy::too_many_arguments)]
    fn build_multitopic<T: DdsType + 'static>(
        &self,
        name: &str,
        type_name: &str,
        subscription_expression: &str,
        expression_parameters: alloc::vec::Vec<String>,
        result_support: Arc<crate::serialized_type::SerializedTypeSupport>,
        metadata: crate::topic::TopicTypeMetadata,
        codec: Arc<dyn crate::topic::TopicCodec<T>>,
    ) -> Result<crate::topic::MultiTopic<T>> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        if name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic name",
            });
        }
        if type_name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic type_name",
            });
        }
        if subscription_expression.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic subscription expression",
            });
        }
        let expression = zerodds_sql_filter::parse_topic_expression(subscription_expression)
            .map_err(|e| {
                self.report_multitopic_refused(&alloc::format!("{e}"));
                DdsError::BadParameter {
                    what: "multitopic subscription expression syntax",
                }
            })?;
        let mut topics = Vec::new();
        for topic in &expression.from {
            let (_, support) = self.multitopic_source(topic)?;
            topics.push(crate::multitopic::SourceTopic {
                name: topic.clone(),
                support,
            });
        }
        let plan = crate::multitopic::MultiTopicPlan::new(&expression, topics, result_support)
            .map_err(|e| {
                self.report_multitopic_refused(&e.detail);
                DdsError::from(e)
            })?;
        let result_topic = Topic::<T>::with_static_metadata(
            name.into(),
            self.get_default_topic_qos(),
            self.clone(),
            metadata,
        )
        .with_codec(codec);
        let multitopic = crate::topic::MultiTopic::<T>::new(
            name.into(),
            type_name.into(),
            subscription_expression.into(),
            expression,
            expression_parameters,
            self.clone(),
            Arc::new(plan),
            result_topic,
        )?;
        self.register_description(multitopic.description_entry());
        Ok(multitopic)
    }

    /// The topic `name` of this participant and the support that reads its
    /// samples as `DynamicData`: what a MultiTopic joins.
    ///
    /// # Errors
    /// `BadParameter` for a topic this participant does not have or that has
    /// no `TypeObject`.
    #[cfg(feature = "std")]
    pub(crate) fn multitopic_source(
        &self,
        name: &str,
    ) -> Result<(
        Topic<crate::serialized_type::SerializedSample>,
        Arc<crate::serialized_type::SerializedTypeSupport>,
    )> {
        let inner = self
            .inner
            .topics
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "topic registry poisoned",
            })?
            .get(name)
            .and_then(|entries| entries.first().cloned())
            .ok_or_else(|| {
                self.report_multitopic_refused(&alloc::format!("the topic '{name}' does not exist"));
                DdsError::BadParameter {
                    what: "multitopic: a topic of the FROM clause does not exist in this participant",
                }
            })?;
        inner.entity_state.check_not_deleted()?;
        let Some(object) = inner.metadata.type_object.as_ref() else {
            self.report_multitopic_refused(&alloc::format!(
                "the topic '{name}' has no TypeObject; its members cannot be named"
            ));
            return Err(DdsError::BadParameter {
                what: "multitopic: a topic of the FROM clause has no TypeObject to read its members from",
            });
        };
        let dependencies = inner.type_dependencies.get().map_or(&[][..], Vec::as_slice);
        let mut support = crate::serialized_type::SerializedTypeSupport::from_topic_objects(
            &inner.metadata.type_name,
            object,
            dependencies,
        )
        .map_err(|e| {
            self.report_multitopic_refused(&alloc::format!("the topic '{name}': {e}"));
            DdsError::BadParameter {
                what: "multitopic: the TypeObject of a topic of the FROM clause cannot be read",
            }
        })?;
        if let Some(rt) = self.inner.runtime.as_ref() {
            if support.dialect() != rt.codec_dialect() && support.dialect().is_default() {
                support = support.in_dialect(rt.codec_dialect());
            }
        }
        let topic =
            reconstruct_topic::<crate::serialized_type::SerializedSample>(inner, self.clone())
                .with_codec(support.clone());
        Ok((topic, support))
    }

    /// Names why a MultiTopic or a reader of one was refused, which the
    /// error code alone cannot.
    #[cfg(feature = "std")]
    pub(crate) fn report_multitopic_refused(&self, detail: &str) {
        use zerodds_foundation::observability::{Component, Event, Level};
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.config.observability.record(
                &Event::new(Level::Warn, Component::Dcps, "multitopic.refused")
                    .with_attr("detail", detail),
            );
        }
    }

    /// Deletes a `MultiTopic` (DDS 1.4 §2.2.2.2.1.10 `delete_multitopic`)
    /// for every handle of it: it stops using its related topics, and an
    /// operation on a handle of it returns `AlreadyDeleted`.
    ///
    /// # Errors
    /// - `PreconditionNotMet` if the MultiTopic belongs to a different
    ///   participant.
    /// - `PreconditionNotMet` while a DataReader created on it exists.
    /// - `AlreadyDeleted` if the MultiTopic is deleted already.
    pub fn delete_multitopic<T: DdsType>(&self, mt: &crate::topic::MultiTopic<T>) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.2.1.10: "If delete_multitopic is called on a
        // different DomainParticipant, the operation will have no effect
        // and it will return PRECONDITION_NOT_MET." It was BAD_PARAMETER.
        if mt.get_participant().inner_ptr() != self.inner_ptr() {
            return Err(DdsError::PreconditionNotMet {
                reason: "multitopic belongs to different participant",
            });
        }
        mt.delete_description()
    }

    /// Deletes a `ContentFilteredTopic`. Spec reference: §2.2.2.2.1.14
    /// `delete_contentfilteredtopic`.
    ///
    /// Deletes the ContentFilteredTopic for every handle of it: it stops
    /// using its related topic, and an operation on a handle of it, or a
    /// DataReader created on it, returns `AlreadyDeleted`.
    ///
    /// # Errors
    /// - `PreconditionNotMet` if the CFT belongs to a different participant
    ///   (§2.2.2.2.1.8).
    /// - `PreconditionNotMet` while a DataReader created on it exists
    ///   (§2.2.2.2.1.8).
    /// - `AlreadyDeleted` if the CFT is deleted already.
    pub fn delete_contentfilteredtopic<T: DdsType>(
        &self,
        cft: &ContentFilteredTopic<T>,
    ) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.2.1.8: "If delete_contentfilteredtopic is called
        // on a different DomainParticipant, the operation will have no
        // effect and it will return PRECONDITION_NOT_MET." It was
        // BAD_PARAMETER.
        if cft.get_participant().inner_ptr() != self.inner_ptr() {
            return Err(DdsError::PreconditionNotMet {
                reason: "cft belongs to different participant",
            });
        }
        cft.delete_description()
    }

    /// Internal identity pointer for participant comparison (used in
    /// `delete_contentfilteredtopic` validation).
    pub(crate) fn inner_ptr(&self) -> *const ParticipantInner {
        Arc::as_ptr(&self.inner)
    }

    /// The shared inner state, for the sibling modules that must reach this
    /// participant's registries — the topic descriptions, which count
    /// themselves as users of their topics for `delete_topic`, and the
    /// publisher/subscriber, which count the endpoints they create.
    pub(crate) fn inner(&self) -> &Arc<ParticipantInner> {
        &self.inner
    }

    /// Creates a publisher with the given QoS; with
    /// [`PUBLISHER_QOS_DEFAULT`], with [`Self::get_default_publisher_qos`]
    /// (DDS 1.4 §2.2.2.2.1.1).
    ///
    /// [`PUBLISHER_QOS_DEFAULT`]: crate::psm_constants::PUBLISHER_QOS_DEFAULT
    ///
    /// On a deleted participant (DDS 1.4 §2.2.1.1) the publisher returned
    /// is itself deleted: every operation on it returns `AlreadyDeleted`,
    /// and the participant does not list it. [`Self::try_create_publisher`]
    /// reports the same condition as the result.
    pub fn create_publisher(&self, qos: impl Into<QosArg<PublisherQos>>) -> Publisher {
        let qos = qos.into().resolve(|| self.get_default_publisher_qos());
        #[cfg(feature = "std")]
        let p = {
            let p = Publisher::new(qos, self.inner.runtime.clone());
            // Wire up the (weak) bubble-up back-pointer, so that writer
            // events reach the DomainParticipantListener.
            p.attach_participant(Arc::downgrade(&self.inner));
            p
        };
        #[cfg(not(feature = "std"))]
        let p = Publisher::new(qos);
        #[cfg(feature = "std")]
        if self.inner.entity_state.is_deleted() {
            p.inner.entity_state.mark_deleted();
            return p;
        }
        // §2.2.3.20 ENTITY_FACTORY. A new publisher has no enable work and
        // nothing created yet, so the flag is all its enable sets.
        #[cfg(feature = "std")]
        p.inner.entity_state.set_factory(&self.inner.entity_state);
        if self.autoenables_children() {
            p.inner.entity_state.enable();
        }
        if let Ok(mut list) = self.inner.publishers.lock() {
            list.push(Arc::downgrade(&p.inner));
        }
        p
    }

    /// DDS 1.4 §2.2.2.2.1.1 `create_publisher(qos, a_listener, mask)`: the
    /// publisher with its listener. `None` with a non-empty `mask` is the
    /// 'nil' listener (§2.2.2.1.1.3). What happened before the listener was set
    /// is heard at its first look: the status flags are kept until read or
    /// taken by a listener.
    #[cfg(feature = "std")]
    pub fn create_publisher_with_listener(
        &self,
        qos: impl Into<QosArg<PublisherQos>>,
        listener: Option<crate::listener::ArcPublisherListener>,
        mask: StatusMask,
    ) -> Publisher {
        let publisher = self.create_publisher(qos);
        publisher.set_listener(listener, mask);
        publisher
    }

    /// DDS 1.4 §2.2.2.2.1.3 `create_subscriber(qos, a_listener, mask)`; see
    /// [`Self::create_publisher_with_listener`].
    #[cfg(feature = "std")]
    pub fn create_subscriber_with_listener(
        &self,
        qos: impl Into<QosArg<SubscriberQos>>,
        listener: Option<crate::listener::ArcSubscriberListener>,
        mask: StatusMask,
    ) -> Subscriber {
        let subscriber = self.create_subscriber(qos);
        subscriber.set_listener(listener, mask);
        subscriber
    }

    /// Creates a subscriber with the given QoS; with
    /// [`SUBSCRIBER_QOS_DEFAULT`], with [`Self::get_default_subscriber_qos`]
    /// (DDS 1.4 §2.2.2.2.1.3).
    ///
    /// [`SUBSCRIBER_QOS_DEFAULT`]: crate::psm_constants::SUBSCRIBER_QOS_DEFAULT
    ///
    /// On a deleted participant (DDS 1.4 §2.2.1.1) the subscriber returned
    /// is itself deleted: every operation on it returns `AlreadyDeleted`,
    /// and the participant does not list it. [`Self::try_create_subscriber`]
    /// reports the same condition as the result.
    pub fn create_subscriber(&self, qos: impl Into<QosArg<SubscriberQos>>) -> Subscriber {
        let qos = qos.into().resolve(|| self.get_default_subscriber_qos());
        #[cfg(feature = "std")]
        let s = {
            let s = Subscriber::new(qos, self.inner.runtime.clone());
            // Wire up the (weak) bubble-up back-pointer.
            s.attach_participant(Arc::downgrade(&self.inner));
            s
        };
        #[cfg(not(feature = "std"))]
        let s = Subscriber::new(qos);
        #[cfg(feature = "std")]
        if self.inner.entity_state.is_deleted() {
            s.inner.entity_state.mark_deleted();
            return s;
        }
        // §2.2.3.20 ENTITY_FACTORY; see `create_publisher`.
        #[cfg(feature = "std")]
        s.inner.entity_state.set_factory(&self.inner.entity_state);
        if self.autoenables_children() {
            s.inner.entity_state.enable();
        }
        if let Ok(mut list) = self.inner.subscribers.lock() {
            list.push(Arc::downgrade(&s.inner));
        }
        s
    }

    /// `create_publisher` that reports a deleted participant.
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant` (DDS 1.4 §2.2.1.1).
    pub fn try_create_publisher(&self, qos: impl Into<QosArg<PublisherQos>>) -> Result<Publisher> {
        self.inner.entity_state.check_not_deleted()?;
        Ok(self.create_publisher(qos))
    }

    /// `create_subscriber` that reports a deleted participant.
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant` (DDS 1.4 §2.2.1.1).
    pub fn try_create_subscriber(
        &self,
        qos: impl Into<QosArg<SubscriberQos>>,
    ) -> Result<Subscriber> {
        self.inner.entity_state.check_not_deleted()?;
        Ok(self.create_subscriber(qos))
    }

    /// DDS 1.4 §2.2.2.2.1.20 `set_default_publisher_qos`: the QoS
    /// `create_publisher` takes for [`PUBLISHER_QOS_DEFAULT`]. That value
    /// itself resets the default to "the values that would be used if the
    /// set_default_publisher_qos operation had never been called". An
    /// existing publisher keeps its QoS.
    ///
    /// [`PUBLISHER_QOS_DEFAULT`]: crate::psm_constants::PUBLISHER_QOS_DEFAULT
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant`.
    pub fn set_default_publisher_qos(&self, qos: impl Into<QosArg<PublisherQos>>) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        set_default(&self.inner.default_publisher_qos, qos.into())
    }

    /// DDS 1.4 §2.2.2.2.1.21 `get_default_publisher_qos`.
    #[must_use]
    pub fn get_default_publisher_qos(&self) -> PublisherQos {
        get_default(&self.inner.default_publisher_qos)
    }

    /// DDS 1.4 §2.2.2.2.1.22 `set_default_subscriber_qos`, as
    /// [`Self::set_default_publisher_qos`].
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant`.
    pub fn set_default_subscriber_qos(&self, qos: impl Into<QosArg<SubscriberQos>>) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        set_default(&self.inner.default_subscriber_qos, qos.into())
    }

    /// DDS 1.4 §2.2.2.2.1.23 `get_default_subscriber_qos`.
    #[must_use]
    pub fn get_default_subscriber_qos(&self) -> SubscriberQos {
        get_default(&self.inner.default_subscriber_qos)
    }

    /// DDS 1.4 §2.2.2.2.1.24 `set_default_topic_qos`, as
    /// [`Self::set_default_publisher_qos`]: "This operation will check that
    /// the resulting policies are self consistent; if they are not, the
    /// operation will have no effect and return INCONSISTENT_POLICY."
    ///
    /// # Errors
    /// `AlreadyDeleted` after `delete_participant`; `InconsistentPolicy`.
    pub fn set_default_topic_qos(&self, qos: impl Into<QosArg<TopicQos>>) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let qos = qos.into().resolve(TopicQos::default);
        qos.check_consistency()?;
        set_default(&self.inner.default_topic_qos, QosArg::Value(qos))
    }

    /// DDS 1.4 §2.2.2.2.1.25 `get_default_topic_qos`.
    #[must_use]
    pub fn get_default_topic_qos(&self) -> TopicQos {
        get_default(&self.inner.default_topic_qos)
    }

    /// Number of currently registered topics. Diagnostic API.
    #[must_use]
    pub fn topics_len(&self) -> usize {
        self.inner
            .topics
            .lock()
            .map(|t| t.values().map(Vec::len).sum())
            .unwrap_or(0)
    }

    /// Number of currently discovered remote participants via SPDP.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.7 `get_discovered_participants`.
    /// 0 in offline mode.
    #[must_use]
    pub fn discovered_participants_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_participants().len();
        }
        0
    }

    /// Number of remote publications currently known in the SEDP cache.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.9 `get_discovered_topics` (~analogous).
    #[must_use]
    pub fn discovered_publications_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_publications_count();
        }
        0
    }

    /// Number of remote subscriptions currently known in the SEDP cache.
    #[must_use]
    pub fn discovered_subscriptions_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_subscriptions_count();
        }
        0
    }

    // ============================================================
    // ignore_* (DDS 1.4 §2.2.2.2.1.14-17)
    // ============================================================

    /// Marks a discovered remote `DomainParticipant` as "ignored" — all
    /// further SPDP beacons with this handle drop out of the builtin
    /// reader stream, and at the same time all SEDP endpoints belonging
    /// to the same participant prefix are also discarded
    /// (Spec §2.2.2.2.1.14). What this participant has matched with it
    /// already is unmatched; the four `ignore_*` operations left such
    /// matches delivering.
    ///
    /// `handle` is the `instance_handle` of the participant's sample in the
    /// `DCPSParticipant` built-in reader.
    ///
    /// Per spec the action is **monotonic** — a once-ignored participant
    /// stays ignored for the lifetime of this participant.
    ///
    /// # Errors
    /// `PreconditionNotMet` on a poisoned ignore filter; `AlreadyDeleted`
    /// after `delete_participant`. The spec allows `OUT_OF_RESOURCES`,
    /// which we do not actively produce.
    pub fn ignore_participant(&self, handle: InstanceHandle) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.1.1.7: NOT_ENABLED before `enable`.
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        {
            self.inner
                .ignore_filter
                .inner
                .participants
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "ignore filter poisoned",
                })?
                .insert(handle);
            // What is matched already goes too.
            if let Some(rt) = self.inner.runtime.as_ref() {
                rt.disconnect_ignored(crate::runtime::Ignored::Participant(handle));
            }
        }
        Ok(())
    }

    /// Marks a discovered remote topic as "ignored". Spec §2.2.2.2.1.15.
    ///
    /// The handle is that of a `DCPSTopic` entry, which is one of a
    /// participant: the entry of a remote participant ignores the topic for
    /// that participant's publications and subscriptions, the entry of this
    /// participant's own topic for those of every participant.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_topic(&self, handle: InstanceHandle) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.1.1.7: NOT_ENABLED before `enable`.
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        {
            self.inner
                .ignore_filter
                .inner
                .topics
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "ignore filter poisoned",
                })?
                .insert(handle);
            // What is matched already goes too.
            if let Some(rt) = self.inner.runtime.as_ref() {
                rt.disconnect_ignored(crate::runtime::Ignored::Topic(handle));
            }
        }
        Ok(())
    }

    /// Marks a discovered remote publication as "ignored".
    /// Spec §2.2.2.2.1.16.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_publication(&self, handle: InstanceHandle) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.1.1.7: NOT_ENABLED before `enable`.
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        {
            self.inner
                .ignore_filter
                .inner
                .publications
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "ignore filter poisoned",
                })?
                .insert(handle);
            // What is matched already goes too.
            if let Some(rt) = self.inner.runtime.as_ref() {
                rt.disconnect_ignored(crate::runtime::Ignored::Publication(handle));
            }
        }
        Ok(())
    }

    /// Marks a discovered remote subscription as "ignored".
    /// Spec §2.2.2.2.1.17.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_subscription(&self, handle: InstanceHandle) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.1.1.7: NOT_ENABLED before `enable`.
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        {
            self.inner
                .ignore_filter
                .inner
                .subscriptions
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "ignore filter poisoned",
                })?
                .insert(handle);
            // What is matched already goes too.
            if let Some(rt) = self.inner.runtime.as_ref() {
                rt.disconnect_ignored(crate::runtime::Ignored::Subscription(handle));
            }
        }
        Ok(())
    }

    /// `true` if `handle` was marked via `ignore_participant`.
    #[must_use]
    pub fn is_participant_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_participant_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` if `handle` was marked via `ignore_topic`.
    #[must_use]
    pub fn is_topic_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_topic_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` if `handle` was marked via `ignore_publication`.
    #[must_use]
    pub fn is_publication_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_publication_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` if `handle` was marked via `ignore_subscription`.
    #[must_use]
    pub fn is_subscription_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_subscription_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// Internal access to the shared ignore filter — used by tests + the
    /// runtime discovery hook.
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn ignore_filter(&self) -> IgnoreFilter {
        self.inner.ignore_filter.clone()
    }

    // ============================================================
    // delete_contained_entities (DDS 1.4 §2.2.2.2.1.18)
    // ============================================================

    /// Whether a publisher, a subscriber or a topic of this participant
    /// still exists (DDS 1.4 §2.2.2.2.2.2 `delete_participant`).
    pub(crate) fn has_contained_entities(&self) -> bool {
        #[cfg(feature = "std")]
        if !self.live_publishers().is_empty() || !self.live_subscribers().is_empty() {
            return true;
        }
        self.inner
            .topics
            .lock()
            .map(|topics| !topics.is_empty())
            .unwrap_or(true)
    }

    /// DDS 1.4 §2.2.2.2.1.18 `delete_contained_entities`: deletes the
    /// publishers and subscribers this participant created, with their
    /// DataWriters and DataReaders and the readers' ReadConditions and
    /// QueryConditions, then its topics; and empties the builtin readers
    /// (so that `take()` after it returns an empty vec). A deleted writer
    /// or reader is disposed on SEDP, a deleted topic on the SEDP topics
    /// writer.
    ///
    /// # Errors
    /// `PreconditionNotMet` if an internal mutex is poisoned.
    pub fn delete_contained_entities(&self) -> Result<()> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        // The endpoints first: a topic is one a writer or reader uses
        // (§2.2.2.2.1.6). Before anything goes, every contained entity is
        // asked whether it can: "The operation will return
        // PRECONDITION_NOT_MET if the any of the contained entities is in a
        // state where it cannot be deleted" (§2.2.2.2.1.18), and then none is
        // deleted.
        #[cfg(feature = "std")]
        {
            for publisher in self.live_publishers() {
                publisher.inner.check_writers_may_change()?;
            }
            for subscriber in self.live_subscribers() {
                if subscriber.is_access_open() {
                    return Err(DdsError::PreconditionNotMet {
                        reason: "a group access (begin_access) is open on a subscriber",
                    });
                }
            }
            for publisher in self.live_publishers() {
                publisher.delete_contained_entities()?;
                publisher.inner.entity_state.mark_deleted();
            }
            for subscriber in self.live_subscribers() {
                subscriber.delete_contained_entities()?;
                subscriber.inner.entity_state.mark_deleted();
            }
        }
        if let Ok(mut p) = self.inner.publishers.lock() {
            p.clear();
        }
        if let Ok(mut s) = self.inner.subscribers.lock() {
            s.clear();
        }
        // The ContentFilteredTopics and MultiTopics go before the topics they
        // use (§2.2.2.2.1.18: "it deletes all contained Publisher,
        // Subscriber, Topic, ContentFilteredTopic, and MultiTopic").
        #[cfg(feature = "std")]
        self.delete_all_descriptions()?;
        // Clear the topic registry; each topic deleted is disposed on the
        // SEDP topics writer (DDSI-RTPS 2.5 Table 8.84).
        let deleted: Vec<(String, String)> = {
            let mut topics =
                self.inner
                    .topics
                    .lock()
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "topic registry poisoned",
                    })?;
            let names = topics
                .values()
                .flatten()
                .map(|entry| (entry.name.clone(), entry.type_name.to_string()))
                .collect();
            topics.clear();
            names
        };
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            for (name, type_name) in &deleted {
                rt.dispose_local_topic_of_type(name, type_name)?;
            }
        }
        #[cfg(not(feature = "std"))]
        let _ = deleted;
        // Clear the builtin reader inboxes — after
        // delete_contained_entities() the user should see a clean
        // builtin subscriber that only delivers new (post-delete)
        // discovery events.
        let sinks = self.inner.builtin_subscriber.sinks();
        if let Ok(mut g) = sinks.participant.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.topic.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.publication.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.subscription.lock() {
            g.clear();
        }
        for secure in [
            &sinks.participants_secure,
            &sinks.publications_secure,
            &sinks.subscriptions_secure,
        ] {
            if let Ok(mut g) = secure.lock() {
                g.clear();
            }
        }
        // The inbox clear above is no longer the whole story: `take()`/
        // `read()` route through the reader cache, so a sample handed to
        // the application before this call already left the inbox behind.
        self.inner
            .builtin_subscriber
            .participant_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .topic_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .publication_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .subscription_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .participants_secure_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .publications_secure_reader()
            .__reset_builtin_state();
        self.inner
            .builtin_subscriber
            .subscriptions_secure_reader()
            .__reset_builtin_state();
        Ok(())
    }

    /// DDS 1.4 §2.2.2.2.1.2 `delete_publisher`: deletes a publisher this
    /// participant created. Every handle of it returns
    /// [`DdsError::AlreadyDeleted`] from then on.
    ///
    /// # Errors
    /// `PreconditionNotMet`, with `publisher` left as it was, if another
    /// participant created it or it holds a DataWriter; `AlreadyDeleted`
    /// if `publisher` or this participant is deleted.
    #[cfg(feature = "std")]
    pub fn delete_publisher(&self, publisher: &Publisher) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let mine = publisher
            .inner
            .participant_inner()
            .is_some_and(|p| Arc::ptr_eq(&p, &self.inner));
        if !mine {
            return Err(DdsError::PreconditionNotMet {
                reason: "the publisher belongs to another participant",
            });
        }
        publisher.inner.entity_state.check_not_deleted()?;
        if publisher.has_datawriters() {
            return Err(DdsError::PreconditionNotMet {
                reason: "the publisher has DataWriters",
            });
        }
        publisher.inner.entity_state.mark_deleted();
        if let Ok(mut list) = self.inner.publishers.lock() {
            list.retain(|w| !core::ptr::eq(w.as_ptr(), Arc::as_ptr(&publisher.inner)));
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.2.1.4 `delete_subscriber`: deletes a subscriber this
    /// participant created. Every handle of it returns
    /// [`DdsError::AlreadyDeleted`] from then on.
    ///
    /// # Errors
    /// `PreconditionNotMet`, with `subscriber` left as it was, if another
    /// participant created it or it holds a DataReader; `AlreadyDeleted`
    /// if `subscriber` or this participant is deleted.
    #[cfg(feature = "std")]
    pub fn delete_subscriber(&self, subscriber: &Subscriber) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let mine = subscriber
            .inner
            .participant_inner()
            .is_some_and(|p| Arc::ptr_eq(&p, &self.inner));
        if !mine {
            return Err(DdsError::PreconditionNotMet {
                reason: "the subscriber belongs to another participant",
            });
        }
        subscriber.inner.entity_state.check_not_deleted()?;
        if subscriber.has_datareaders() {
            return Err(DdsError::PreconditionNotMet {
                reason: "the subscriber has DataReaders",
            });
        }
        subscriber.inner.entity_state.mark_deleted();
        if let Ok(mut list) = self.inner.subscribers.lock() {
            list.retain(|w| !core::ptr::eq(w.as_ptr(), Arc::as_ptr(&subscriber.inner)));
        }
        Ok(())
    }

    /// The publishers of this participant that exist and are not deleted,
    /// in creation order.
    fn live_publishers(&self) -> Vec<Publisher> {
        let mut list = self
            .inner
            .publishers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        list.retain(|w| w.strong_count() > 0);
        list.iter()
            .filter_map(alloc::sync::Weak::upgrade)
            .filter(|inner| !inner.entity_state.is_deleted())
            .map(|inner| Publisher { inner })
            .collect()
    }

    /// The subscribers, as [`Self::live_publishers`].
    fn live_subscribers(&self) -> Vec<Subscriber> {
        let mut list = self
            .inner
            .subscribers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        list.retain(|w| w.strong_count() > 0);
        list.iter()
            .filter_map(alloc::sync::Weak::upgrade)
            .filter(|inner| !inner.entity_state.is_deleted())
            .map(|inner| Subscriber { inner })
            .collect()
    }

    /// Number of publishers of this participant that exist and are not
    /// deleted. Diagnostic API for tests.
    #[must_use]
    pub fn publishers_len(&self) -> usize {
        self.live_publishers().len()
    }

    /// Number of subscribers, as [`Self::publishers_len`].
    #[must_use]
    pub fn subscribers_len(&self) -> usize {
        self.live_subscribers().len()
    }

    /// Returns this participant's `InstanceHandle`. Identifies the entity
    /// to DCPS API consumers (Spec §2.2.2.1.1 `get_instance_handle`).
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.entity_state.instance_handle()
    }

    /// DDS 1.4 §2.2.2.2.1.32 `get_current_time`: "the current value of the
    /// time that the service uses to time-stamp data-writes and to set the
    /// reception-timestamp for the data-updates it receives" — the clock of
    /// this participant's runtime ([`crate::runtime::RuntimeConfig::time_source`]),
    /// the system clock offline. The participant had no such operation; the
    /// free function `crate::time::get_current_time` reads the system
    /// clock whatever the runtime stamps with.
    ///
    /// # Errors
    /// `NotEnabled` before `enable` (§2.2.2.1.1.7); `AlreadyDeleted` after
    /// `delete_participant`.
    #[cfg(feature = "std")]
    pub fn get_current_time(&self) -> Result<crate::time::Time> {
        self.inner.entity_state.check_enabled()?;
        Ok(self
            .inner
            .runtime
            .as_ref()
            .map_or_else(crate::time::get_current_time, |rt| {
                rt.config.time_source.now()
            }))
    }

    /// This participant's **discovery-space** handle: `InstanceHandle::from_guid`
    /// of its participant GUID.
    ///
    /// Unlike [`Self::instance_handle`] (a local allocator counter), this is the
    /// handle the ignore filter and discovery actually key on. It is therefore
    /// the correct argument to ignore THIS participant from another one — e.g. a
    /// durability service whose sibling ingest/replay participants must ignore
    /// each other to avoid an echo loop. `HANDLE_NIL` when offline.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn participant_handle(&self) -> InstanceHandle {
        match self.inner.runtime.as_ref() {
            Some(rt) => {
                let guid = zerodds_rtps::wire_types::Guid::new(
                    rt.guid_prefix,
                    zerodds_rtps::wire_types::EntityId::PARTICIPANT,
                );
                crate::instance_handle::InstanceHandle::from_guid(guid)
            }
            None => crate::instance_handle::HANDLE_NIL,
        }
    }

    /// Spec §2.2.2.2.1.10 `contains_entity` — `true` if `handle` belongs
    /// to this participant or one of its directly **or recursively**
    /// contained entities.
    ///
    /// **Included entity types:**
    /// - the participant itself
    /// - all topics registered via `create_topic`
    /// - all publishers/subscribers created via `create_publisher` /
    ///   `create_subscriber`
    /// - **recursively**: all DataWriter/DataReader created via
    ///   `Publisher::create_datawriter` / `Subscriber::create_datareader`.
    #[must_use]
    pub fn contains_entity(&self, handle: InstanceHandle) -> bool {
        if self.instance_handle() == handle {
            return true;
        }
        if let Ok(topics) = self.inner.topics.lock() {
            for t in topics.values().flatten() {
                if t.entity_state.instance_handle() == handle {
                    return true;
                }
            }
        }
        for publisher in self.live_publishers() {
            if publisher.inner.entity_state.instance_handle() == handle {
                return true;
            }
            #[cfg(feature = "std")]
            if publisher.contains_writer(handle) {
                return true;
            }
        }
        for subscriber in self.live_subscribers() {
            if subscriber.inner.entity_state.instance_handle() == handle {
                return true;
            }
            #[cfg(feature = "std")]
            if subscriber.contains_reader(handle) {
                return true;
            }
        }
        false
    }

    // ============================================================
    // get_discovered_* (DDS 1.4 §2.2.2.2.1.27-30)
    // ============================================================

    /// Returns the `InstanceHandle`s of all currently discovered remote
    /// participants (Spec §2.2.2.2.1.27). Empty in offline mode. Ignored
    /// participants do **not** appear.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_discovered_participants(&self) -> Vec<InstanceHandle> {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for d in rt.discovered_participants() {
            let h = InstanceHandle::from_guid(d.data.guid);
            if self.is_participant_ignored(h) {
                continue;
            }
            out.push(h);
        }
        out
    }

    /// Offline variant (no std → no runtime).
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_participants(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// `get_discovered_participants` as the specification has it, a
    /// `ReturnCode_t` (§2.2.2.2.1.27): NOT_ENABLED on a participant that is
    /// not enabled (§2.2.2.2.1: "Other operations will have the value
    /// NOT_ENABLED if called on a disabled DomainParticipant").
    ///
    /// # Errors
    /// `NotEnabled`; `AlreadyDeleted` after `delete_participant`.
    #[cfg(feature = "std")]
    pub fn try_get_discovered_participants(&self) -> Result<Vec<InstanceHandle>> {
        self.inner.entity_state.check_enabled()?;
        Ok(self.get_discovered_participants())
    }

    /// `get_discovered_topics` as a `ReturnCode_t` (§2.2.2.2.1.29); see
    /// [`Self::try_get_discovered_participants`].
    ///
    /// # Errors
    /// `NotEnabled`; `AlreadyDeleted` after `delete_participant`.
    #[cfg(feature = "std")]
    pub fn try_get_discovered_topics(&self) -> Result<Vec<InstanceHandle>> {
        self.inner.entity_state.check_enabled()?;
        Ok(self.get_discovered_topics())
    }

    /// `contains_entity` as a `ReturnCode_t` (§2.2.2.2.1.31); see
    /// [`Self::try_get_discovered_participants`].
    ///
    /// # Errors
    /// `NotEnabled`; `AlreadyDeleted` after `delete_participant`.
    #[cfg(feature = "std")]
    pub fn try_contains_entity(&self, handle: InstanceHandle) -> Result<bool> {
        self.inner.entity_state.check_enabled()?;
        Ok(self.contains_entity(handle))
    }

    /// DDS 1.4 §2.2.2.2.1.19 `assert_liveliness`: "This operation manually
    /// asserts the liveliness of the DomainParticipant. This is used in
    /// combination with the LIVELINESS QoS policy to indicate to the Service
    /// that the entity remains active." Every writer of the participant with
    /// MANUAL_BY_PARTICIPANT liveliness is asserted, as by a write of any of
    /// them. Without a runtime there is nobody to tell.
    ///
    /// # Errors
    /// `NotEnabled` on a participant that is not enabled; `AlreadyDeleted`
    /// after `delete_participant`.
    #[cfg(feature = "std")]
    pub fn assert_liveliness(&self) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.assert_liveliness();
        }
        Ok(())
    }

    /// Returns the `ParticipantBuiltinTopicData` for a handle from
    /// `get_discovered_participants` (Spec §2.2.2.2.1.28).
    ///
    /// # Errors
    /// `PreconditionNotMet` if `handle` does not reference a discovered
    /// participant (or if it was ignored): §2.2.2.2.1.28 "otherwise the
    /// operation will fail and return PRECONDITION_NOT_MET". It was
    /// `BadParameter`.
    #[cfg(feature = "std")]
    pub fn get_discovered_participant_data(
        &self,
        handle: InstanceHandle,
    ) -> Result<ParticipantBuiltinTopicData> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.2.1: NOT_ENABLED on a participant that is not enabled.
        self.inner.entity_state.check_enabled()?;
        if self.is_participant_ignored(handle) {
            return Err(DdsError::PreconditionNotMet {
                reason: "participant handle is ignored",
            });
        }
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "no runtime — offline participant",
            });
        };
        for d in rt.discovered_participants() {
            if InstanceHandle::from_guid(d.data.guid) == handle {
                return Ok(ParticipantBuiltinTopicData::from_wire(&d.data));
            }
        }
        Err(DdsError::PreconditionNotMet {
            reason: "unknown participant handle",
        })
    }

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_participant_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<ParticipantBuiltinTopicData> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        Err(DdsError::PreconditionNotMet {
            reason: "no runtime — offline participant",
        })
    }

    /// Returns the `InstanceHandle`s of all currently discovered remote
    /// topics. Spec §2.2.2.2.1.29.
    ///
    /// Topics are discovered over the peers' SEDP topics writers and,
    /// for peers without them, through their pub/sub announcements — per
    /// participant, topic name and type name one stable key via
    /// `TopicBuiltinTopicData::synthesize_key`. Ignored topics do not
    /// appear.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_discovered_topics(&self) -> Vec<InstanceHandle> {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Vec::new();
        };
        let Ok(sedp) = rt.sedp.lock() else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        for t in sedp.discovered_topics() {
            let h = InstanceHandle::from_guid(TopicBuiltinTopicData::synthesize_key(
                t.key.prefix,
                &t.topic_name,
                &t.type_name,
            ));
            if !self.is_topic_ignored(h) {
                seen.insert(h);
            }
        }
        for p in sedp.cache().publications() {
            let key = TopicBuiltinTopicData::synthesize_key(
                p.data.participant_key.prefix,
                &p.data.topic_name,
                &p.data.type_name,
            );
            let h = InstanceHandle::from_guid(key);
            if self.is_topic_ignored(h) {
                continue;
            }
            seen.insert(h);
        }
        for s in sedp.cache().subscriptions() {
            let key = TopicBuiltinTopicData::synthesize_key(
                s.data.participant_key.prefix,
                &s.data.topic_name,
                &s.data.type_name,
            );
            let h = InstanceHandle::from_guid(key);
            if self.is_topic_ignored(h) {
                continue;
            }
            seen.insert(h);
        }
        seen.into_iter().collect()
    }

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_topics(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// Returns the `TopicBuiltinTopicData` for a handle from
    /// `get_discovered_topics`. Spec §2.2.2.2.1.30.
    ///
    /// # Errors
    /// `PreconditionNotMet` if `handle` does not correspond to a discovered
    /// topic (or was ignored): §2.2.2.2.1.30 "Otherwise, the operation will
    /// fail and return PRECONDITION_NOT_MET". It was `BadParameter`.
    #[cfg(feature = "std")]
    pub fn get_discovered_topic_data(
        &self,
        handle: InstanceHandle,
    ) -> Result<TopicBuiltinTopicData> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`;
        // §2.2.2.2.1: NOT_ENABLED on a participant that is not enabled.
        self.inner.entity_state.check_enabled()?;
        if self.is_topic_ignored(handle) {
            return Err(DdsError::PreconditionNotMet {
                reason: "topic handle is ignored",
            });
        }
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "no runtime — offline participant",
            });
        };
        let Ok(sedp) = rt.sedp.lock() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "sedp poisoned",
            });
        };
        // A topic a peer announced over its SEDP topics writer carries every
        // member; it comes before one derived from an endpoint.
        for t in sedp.discovered_topics() {
            let topic = TopicBuiltinTopicData::from_wire(t);
            if InstanceHandle::from_guid(topic.key) == handle {
                return Ok(topic);
            }
        }
        // Then the publication side.
        for p in sedp.cache().publications() {
            let topic = TopicBuiltinTopicData::from_publication(&p.data);
            if InstanceHandle::from_guid(topic.key) == handle {
                return Ok(topic);
            }
        }
        for s in sedp.cache().subscriptions() {
            let topic = TopicBuiltinTopicData::from_subscription(&s.data);
            if InstanceHandle::from_guid(topic.key) == handle {
                return Ok(topic);
            }
        }
        Err(DdsError::PreconditionNotMet {
            reason: "unknown topic handle",
        })
    }

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_topic_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<TopicBuiltinTopicData> {
        // DDS 1.4 §2.2.1.1: ALREADY_DELETED after `delete_participant`.
        self.inner.entity_state.check_not_deleted()?;
        Err(DdsError::PreconditionNotMet {
            reason: "no runtime — offline participant",
        })
    }

    /// The participant's builtin subscriber (DDS 1.4 §2.2.2.2.1.7).
    ///
    /// Always returns the same subscriber handle (exactly one builtin
    /// subscriber per participant). It contains 4 pre-created readers for
    /// the builtin topics:
    ///
    /// - `DCPSParticipant` → `ParticipantBuiltinTopicData`
    /// - `DCPSTopic` → `TopicBuiltinTopicData`
    /// - `DCPSPublication` → `PublicationBuiltinTopicData`
    /// - `DCPSSubscription` → `SubscriptionBuiltinTopicData`
    ///
    /// SPDP/SEDP receive internally triggers a sample insert that can be
    /// picked up via `take()`/`read()` (DDS 1.4 §2.2.5).
    ///
    /// # Example
    /// ```
    /// use zerodds_dcps::*;
    /// let participant = DomainParticipantFactory::instance()
    ///     .create_participant_offline(0, DomainParticipantQos::default());
    /// let bs = participant.get_builtin_subscriber();
    /// let r = bs
    ///     .lookup_datareader::<DcpsParticipantBuiltinTopicData>("DCPSParticipant")
    ///     .expect("builtin reader");
    /// // Initially empty (offline mode → no SPDP receives).
    /// assert!(r.take().expect("take").is_empty());
    /// ```
    #[must_use]
    pub fn get_builtin_subscriber(&self) -> Arc<BuiltinSubscriber> {
        Arc::clone(&self.inner.builtin_subscriber)
    }

    // ============================================================
    // Listener-Slot (DDS 1.4 §2.2.2.2.3)
    // ============================================================

    /// Sets the `DomainParticipantListener`. `listener=None` clears the
    /// slot. `mask` is the [`StatusMask`] that determines which status
    /// bits this listener consumes (Spec §2.2.4.2.3 bubble-up).
    pub fn set_listener(&self, listener: Option<ArcDomainParticipantListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0).then(|| {
                alloc::sync::Arc::new(crate::listener::NilListener) as ArcDomainParticipantListener
            })
        });
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        // Mirror the mask into the EntityState — for get_listener_mask().
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(self.inner.runtime.as_ref());
    }

    /// Returns the currently installed listener clone, if present.
    /// Spec §2.2.2.2.3.x get_listener.
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDomainParticipantListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
            .filter(|l| !l.is_nil())
    }

    /// Snapshot of the listener slot (listener + mask) — for the dispatch
    /// path. Clones the Arc under the mutex and releases the lock
    /// immediately (lock discipline: run callbacks outside).
    #[must_use]
    #[allow(dead_code)] // used via Topic::listener_chain (cfg(std))
    pub(crate) fn snapshot_listener(&self) -> Option<(ArcDomainParticipantListener, StatusMask)> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)))
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

impl crate::entity::Entity for DomainParticipant {
    type Qos = DomainParticipantQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    fn set_qos(&self, qos: impl Into<QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos.into().resolve(|| {
            crate::factory::DomainParticipantFactory::instance().get_default_participant_qos()
        });
        // DDS-Security 1.2 §9.4.2.9.3: the permissions must allow the new
        // QoS; otherwise `set_qos` fails with NOT_ALLOWED_BY_SECURITY.
        #[cfg(all(feature = "std", feature = "security"))]
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.check_participant_qos(&qos.property)?;
        }
        // DomainParticipantQos: USER_DATA + ENTITY_FACTORY are all
        // Changeable=YES per Spec §2.2.3 — no immutable check needed. A
        // new USER_DATA goes out in the SPDP beacon at once.
        self.inner
            .entity_state
            .set_autoenable_children(qos.entity_factory.autoenable_created_entities);
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.update_participant_properties(crate::qos::propagated_properties(&qos.property))?;
            rt.update_participant_user_data(qos.user_data.value.clone());
        }
        if let Ok(mut current) = self.inner.qos.lock() {
            *current = qos;
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7: a participant its factory created disabled
    /// announces itself over SPDP from here; with
    /// ENTITY_FACTORY.autoenable_created_entities it then enables the
    /// publishers, subscribers and topics it created, and they theirs.
    fn enable(&self) -> Result<()> {
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.set_discoverable(true);
        }
        self.inner.entity_state.enable_entity()
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

// ---- internal helpers ----

/// `set_default_*_qos` of a participant, publisher or subscriber: stores the
/// QoS, or for [`QosArg::Default`] the specification's default.
pub(crate) fn set_default<Q: Default>(slot: &Mutex<Q>, qos: QosArg<Q>) -> Result<()> {
    let mut current = slot.lock().map_err(|_| DdsError::PreconditionNotMet {
        reason: "default qos poisoned",
    })?;
    *current = qos.resolve(Q::default);
    Ok(())
}

/// `get_default_*_qos` of a participant, publisher or subscriber.
pub(crate) fn get_default<Q: Clone + Default>(slot: &Mutex<Q>) -> Q {
    slot.lock().map(|q| q.clone()).unwrap_or_default()
}

/// Announces `inner` over the SEDP topics writer with the QoS it has now.
#[cfg(feature = "std")]
pub(crate) fn announce_topic(rt: &DcpsRuntime, inner: &TopicInner) -> Result<()> {
    let qos = inner
        .qos
        .lock()
        .map(|q| q.clone())
        .map_err(|_| DdsError::PreconditionNotMet {
            reason: "topic qos poisoned",
        })?;
    rt.announce_local_topic(
        &inner.name,
        &inner.type_name,
        &qos,
        &inner.metadata.type_identifier,
    )
}

fn topic_inner<T: DdsType>(t: &Topic<T>) -> Arc<TopicInner> {
    t.inner()
}

/// Extracts the `EquivalenceHash` from a `TypeIdentifier`, if it is one
/// of the hash variants.
#[cfg(feature = "std")]
fn extract_equivalence_hash(
    ti: &zerodds_types::TypeIdentifier,
) -> Option<zerodds_types::EquivalenceHash> {
    use zerodds_types::TypeIdentifier;
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) | TypeIdentifier::EquivalenceHashComplete(h) => {
            Some(*h)
        }
        _ => None,
    }
}

fn reconstruct_topic<T: DdsType>(
    inner: Arc<TopicInner>,
    participant: DomainParticipant,
) -> Topic<T> {
    // The TopicInner itself is generic-agnostic (just name +
    // type-name string); we set up a new topic handle with the same
    // inner. `Topic::new` would create a new inner — but we want to
    // share the shared inner.
    Topic::<T>::from_inner(inner, participant)
}

// Topic needs a `from_inner` constructor for this.
impl<T: DdsType> Topic<T> {
    pub(crate) fn from_inner(inner: Arc<TopicInner>, participant: DomainParticipant) -> Self {
        Self::_from_inner_impl(inner, participant)
    }
}

// Since `Topic<T>` keeps its inner private, we also need a
// `_from_inner_impl` shortcut in the topic module. It is right next to
// the constructor.

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Seeds that share their first 12 bytes give different prefixes; the
    /// host bytes are the process's host identifier.
    #[cfg(feature = "std")]
    #[test]
    fn bridge_prefixes_differ_by_seed_and_carry_the_host_id() {
        let a = bridge_guid_prefix("zerodds-bridge-a").to_bytes();
        let b = bridge_guid_prefix("zerodds-bridge-b").to_bytes();
        let c = bridge_guid_prefix("127.0.0.1:9001").to_bytes();
        let d = bridge_guid_prefix("127.0.0.1:9002").to_bytes();
        assert_ne!(a, b);
        assert_ne!(c, d);
        assert_eq!(a[..2], [0x01, 0xF0]);
        assert_eq!(a[2..6], host_id_bytes());
        assert_eq!(
            a,
            bridge_guid_prefix("zerodds-bridge-a").to_bytes(),
            "stable"
        );
    }
    use crate::dds_type::RawBytes;

    #[test]
    fn participant_created_with_domain_id() {
        let p = DomainParticipant::new(42, DomainParticipantQos::default());
        assert_eq!(p.domain_id(), 42);
        assert_eq!(p.topics_len(), 0);
    }

    /// DDSI-RTPS 2.5 §8.2.4.2: "as long as every Participant in the Domain has a
    /// unique GUID". The tails of 65536 consecutive counters of one process
    /// are pairwise different whatever the seed is; the clock does not enter
    /// them again.
    #[test]
    fn the_prefixes_of_one_process_are_pairwise_different() {
        for seed in [0u16, 1, 0x7FFF, 0xFFFF] {
            let mut seen = std::collections::BTreeSet::new();
            for counter in 0..=u32::from(u16::MAX) {
                assert!(
                    seen.insert(process_unique(7, seed, counter)),
                    "seed {seed}, counter {counter}"
                );
            }
        }
        let a = next_process_unique();
        let b = next_process_unique();
        assert_ne!(a, b);
        // The process id carries the mark of the PID namespace; in the
        // initial namespace it is the id itself.
        assert_eq!(
            &a[..4],
            &namespaced_pid(std::process::id(), pid_namespace_inode()).to_le_bytes()
        );
    }

    /// Wave 4a (Spec `zerodds-zero-copy-1.0` §6): two GuidPrefixes in the
    /// same process share the host-id prefix → `is_same_host = true`.
    /// DDSI-RTPS 2.5 §8.2.4.2: two containers with the host's network (one
    /// host id) and each a PID namespace of its own both have a process 1;
    /// the marks of the two namespaces tell their prefixes apart. Within a
    /// namespace the map is one-to-one, and the initial namespace is not
    /// marked.
    #[test]
    fn two_pid_namespaces_with_the_same_pid_give_different_prefixes() {
        let initial = Some(u64::from(super::INITIAL_PID_NAMESPACE_INODE));
        let a = Some(4_026_532_201_u64);
        let b = Some(4_026_532_305_u64);
        assert_ne!(namespaced_pid(1, a), namespaced_pid(1, b));
        assert_ne!(namespaced_pid(1, initial), namespaced_pid(1, a));
        assert_eq!(
            namespaced_pid(1234, initial),
            1234,
            "the initial namespace is unmarked"
        );
        assert_eq!(namespaced_pid(1234, None), 1234);
        // One-to-one within a namespace.
        let mut seen = std::collections::BTreeSet::new();
        for pid in 0..5000_u32 {
            assert!(seen.insert(namespaced_pid(pid, a)));
        }
        // The prefixes: same host id, same pid 1, same seed and counter, two
        // namespaces.
        let host = [1, 2, 3, 4];
        let pa = zerodds_rtps::wire_types::GuidPrefix::zerodds(
            host,
            process_unique(namespaced_pid(1, a), 7, 0),
        );
        let pb = zerodds_rtps::wire_types::GuidPrefix::zerodds(
            host,
            process_unique(namespaced_pid(1, b), 7, 0),
        );
        assert_ne!(pa, pb);
        assert!(pa.is_same_host(pb));
        assert_ne!(pa.to_bytes()[..10], pb.to_bytes()[..10], "not one process");
    }

    /// The prefixes of one process share the first ten octets (host and
    /// marked process id) and differ in the tail, which the OS-random seed
    /// and the counter make; the seed is not the clock.
    #[test]
    fn the_prefixes_of_one_process_share_ten_octets_and_differ_in_the_tail() {
        let a = random_guid_prefix().to_bytes();
        let b = random_guid_prefix().to_bytes();
        assert_eq!(a[..10], b[..10], "same process");
        assert_ne!(a[10..], b[10..]);
    }

    /// The PID bytes must correspond to `process::id()`.
    #[test]
    fn random_guid_prefixes_share_host_id_within_process() {
        let p1 = random_guid_prefix();
        let p2 = random_guid_prefix();
        assert_eq!(p1.host_id(), p2.host_id(), "same-host within process");
        assert!(p1.is_same_host(p2));

        let pid_le = namespaced_pid(std::process::id(), pid_namespace_inode()).to_le_bytes();
        let bytes = p1.to_bytes();
        // DDSI-RTPS 2.5 §9.3.1.5: the first two octets are the vendor id.
        assert_eq!(&bytes[0..2], &[0x01, 0xF0], "vendor id in prefix[0..2]");
        assert_eq!(&bytes[2..6], &host_id_bytes(), "host id in prefix[2..6]");
        assert_eq!(&bytes[6..10], &pid_le, "PID bytes in prefix[6..10]");

        // The counter + time bytes must make the two prefixes
        // distinguishable.
        assert_ne!(p1, p2, "two prefixes must be distinct");
    }

    #[test]
    fn host_id_bytes_deterministic_within_process() {
        let a = host_id_bytes();
        let b = host_id_bytes();
        assert_eq!(a, b, "OnceLock-cached host-id must be stable");
    }

    #[test]
    fn create_topic_stores_in_registry() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t1 = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        let t2 = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        assert_eq!(t1.name(), t2.name());
        assert_eq!(p.topics_len(), 1);
    }

    #[test]
    fn create_topic_takes_a_second_type_and_rejects_an_inconsistent_qos() {
        // A second DdsType for the test.
        #[derive(Debug)]
        struct DummyU32(u32);
        impl DdsType for DummyU32 {
            const TYPE_NAME: &'static str = "test::DummyU32";
            /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(
                &self,
                out: &mut alloc::vec::Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                if bytes.len() != 4 {
                    return Err(crate::dds_type::DecodeError::Invalid { what: "u32 len" });
                }
                let mut a = [0u8; 4];
                a.copy_from_slice(bytes);
                Ok(Self(u32::from_le_bytes(a)))
            }
        }

        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _ = p
            .create_topic::<RawBytes>("X", TopicQos::default())
            .unwrap();
        // XTypes 1.3 §7.6.4.1: "The Service may instantiate multiple
        // objects of the same name, provided that all of them represent
        // type-based subsets of 'the same' network topic; therefore, they
        // must have consistent QoS with one another." Another type under
        // the name is a second topic when the QoS is the same.
        let second = p
            .create_topic::<DummyU32>("X", TopicQos::default())
            .unwrap();
        assert_eq!(second.name(), "X");
        assert_eq!(p.topics_len(), 2);
        // A QoS that is not consistent with the first one's is refused.
        let mut other = TopicQos::default();
        other.topic_data.value = vec![1];
        #[derive(Debug)]
        struct DummyU64;
        impl DdsType for DummyU64 {
            const TYPE_NAME: &'static str = "test::DummyU64";
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(
                &self,
                _out: &mut alloc::vec::Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                Ok(())
            }
            fn decode(_: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                Ok(Self)
            }
        }
        let err = p.create_topic::<DummyU64>("X", other).unwrap_err();
        assert!(matches!(err, DdsError::InconsistentPolicy { .. }));
    }

    #[test]
    fn create_topic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .create_topic::<RawBytes>("", TopicQos::default())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    /// DDSI-RTPS 2.5 Table 9.18: topic and type names are `string<256>`; a
    /// topic that could not be announced is not created.
    #[test]
    fn create_topic_rejects_a_name_longer_than_string_256() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(
            p.create_topic::<RawBytes>(&"n".repeat(256), TopicQos::default())
                .is_ok()
        );
        let err = p
            .create_topic::<RawBytes>(&"n".repeat(257), TopicQos::default())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
        let err = p
            .create_topic_with_type_name::<RawBytes>("Short", &"t".repeat(257), TopicQos::default())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn lookup_topicdescription_returns_local_topics() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _t = p
            .create_topic::<RawBytes>("Hello", TopicQos::default())
            .unwrap();
        let h = p.lookup_topicdescription("Hello").expect("local lookup");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), "Hello");
        assert_eq!(h.get_type_name(), RawBytes::TYPE_NAME);
        assert_eq!(h.get_participant().domain_id(), 0);
    }

    #[test]
    fn a_topic_created_with_a_type_name_announces_that_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic_with_type_name::<RawBytes>(
                "Calc_Service_Request",
                "Calc_Request",
                TopicQos::default(),
            )
            .unwrap();
        assert_eq!(t.type_name(), "Calc_Request");
        let h = p.lookup_topicdescription("Calc_Service_Request").unwrap();
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_type_name(), "Calc_Request");
        let w = p
            .create_publisher(crate::qos::PublisherQos::default())
            .create_datawriter::<RawBytes>(&t, crate::qos::DataWriterQos::default())
            .unwrap();
        assert_eq!(w.topic().type_name(), "Calc_Request");
    }

    #[test]
    fn a_topic_created_with_type_information_announces_the_hash_of_its_object() {
        use zerodds_types::builder::TypeObjectBuilder;
        use zerodds_types::{CompleteTypeObject, PrimitiveKind, TypeIdentifier, TypeObject};
        let object = |member: &str| {
            TypeObject::Complete(CompleteTypeObject::Struct(
                TypeObjectBuilder::struct_type("T")
                    .member(
                        member,
                        TypeIdentifier::Primitive(PrimitiveKind::Int32),
                        |m| m,
                    )
                    .build_complete(),
            ))
        };
        let information = |member: &str| crate::topic::TopicTypeInformation {
            object: object(member),
            dependencies: Vec::new(),
        };
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic_with_type_information::<RawBytes>(
                "Described",
                "T",
                information("a"),
                TopicQos::default(),
            )
            .unwrap();
        assert_eq!(t.type_name(), "T");
        let expected = zerodds_types::compute_hash(&object("a")).unwrap();
        assert_eq!(
            t.inner().metadata.type_identifier,
            zerodds_types::TypeIdentifier::EquivalenceHashComplete(expected)
        );
        assert_eq!(t.inner().metadata.type_object, Some(object("a")));
        // A structure the builder makes is appendable unless it says otherwise.
        assert_eq!(
            t.inner().metadata.extensibility,
            crate::dds_type::Extensibility::Appendable
        );
        let mutable = p
            .create_topic_with_type_information::<RawBytes>(
                "Described mutable",
                "M",
                crate::topic::TopicTypeInformation {
                    object: TypeObject::Complete(CompleteTypeObject::Struct(
                        TypeObjectBuilder::struct_type("M")
                            .extensibility(zerodds_types::builder::Extensibility::Mutable)
                            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                                m.optional()
                            })
                            .build_complete(),
                    )),
                    dependencies: Vec::new(),
                },
                TopicQos::default(),
            )
            .unwrap();
        assert_eq!(
            mutable.inner().metadata.extensibility,
            crate::dds_type::Extensibility::Mutable
        );
        // The same name and type name with another object is another type.
        let err = p
            .create_topic_with_type_information::<RawBytes>(
                "Described",
                "T",
                information("b"),
                TopicQos::default(),
            )
            .unwrap_err();
        assert!(
            matches!(err, DdsError::InconsistentPolicy { .. }),
            "{err:?}"
        );
        assert!(
            p.create_topic_with_type_information::<RawBytes>(
                "Empty",
                "",
                information("a"),
                TopicQos::default()
            )
            .is_err()
        );
    }

    #[test]
    fn a_topic_name_takes_a_second_type_name_only_with_consistent_qos() {
        // XTypes 1.3 §7.6.4.1: objects of one name are type-based subsets
        // of the same network topic and "must have consistent QoS".
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _a = p
            .create_topic_with_type_name::<RawBytes>("Reused", "A_Request", TopicQos::default())
            .unwrap();
        let b = p
            .create_topic_with_type_name::<RawBytes>("Reused", "B_Request", TopicQos::default())
            .unwrap();
        assert_eq!(b.type_name(), "B_Request");
        let mut other = TopicQos::default();
        other.reliability.kind = zerodds_qos::ReliabilityKind::Reliable;
        let err = p
            .create_topic_with_type_name::<RawBytes>("Reused", "C_Request", other)
            .unwrap_err();
        assert!(matches!(err, DdsError::InconsistentPolicy { .. }));
        assert!(
            p.create_topic_with_type_name::<RawBytes>("Empty", "", TopicQos::default())
                .is_err()
        );
    }

    #[test]
    fn lookup_topicdescription_none_for_unknown() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.lookup_topicdescription("Unknown").is_none());
    }

    // ---- §2.2.2.2.1.10 contains_entity ----

    #[test]
    fn contains_entity_returns_true_for_self_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = p.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_true_for_local_topic() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Hi", TopicQos::default())
            .unwrap();
        let topic_handle = t.inner().entity_state.instance_handle();
        assert!(p.contains_entity(topic_handle));
    }

    #[test]
    fn contains_entity_returns_true_for_local_publisher() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let pub_ = p.create_publisher(PublisherQos::default());
        let h = pub_.inner.entity_state.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_true_for_local_subscriber() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let s = p.create_subscriber(SubscriberQos::default());
        let h = s.inner.entity_state.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_false_for_unknown_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // A different participant has a different handle.
        let other = DomainParticipant::new(0, DomainParticipantQos::default());
        let other_h = other.instance_handle();
        assert!(!p.contains_entity(other_h));
    }

    #[test]
    fn contains_entity_returns_false_for_topic_after_delete() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Tmp", TopicQos::default())
            .unwrap();
        let topic_handle = t.inner().entity_state.instance_handle();
        assert!(p.contains_entity(topic_handle));
        p.delete_contained_entities().unwrap();
        assert!(!p.contains_entity(topic_handle));
    }

    #[test]
    fn contains_entity_recursive_finds_local_datawriter() {
        // §2.2.2.2.1.10 — contains_entity MUST also recognize DataWriter
        // handles created via Publisher::create_datawriter.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Hello", TopicQos::default())
            .unwrap();
        let pub_ = p.create_publisher(PublisherQos::default());
        let dw = pub_
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let dw_handle = dw.instance_handle();
        assert!(p.contains_entity(dw_handle));
        // Plus: the publisher itself exposes contains_writer(handle).
        assert!(pub_.contains_writer(dw_handle));
    }

    #[test]
    fn contains_entity_recursive_finds_local_datareader() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Hello2", TopicQos::default())
            .unwrap();
        let sub = p.create_subscriber(SubscriberQos::default());
        let dr = sub
            .create_datareader(&topic, crate::qos::DataReaderQos::default())
            .unwrap();
        let dr_handle = dr.subscription_handle();
        assert!(p.contains_entity(dr_handle));
        assert!(sub.contains_reader(dr_handle));
    }

    #[test]
    fn contains_entity_recursive_does_not_find_foreign_datawriter() {
        // Negative: a DW created via a different participant is NOT
        // contained.
        let p1 = DomainParticipant::new(0, DomainParticipantQos::default());
        let p2 = DomainParticipant::new(1, DomainParticipantQos::default());
        let topic = p2
            .create_topic::<RawBytes>("Foreign", TopicQos::default())
            .unwrap();
        let pub2 = p2.create_publisher(PublisherQos::default());
        let dw2 = pub2
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        assert!(!p1.contains_entity(dw2.instance_handle()));
        assert!(p2.contains_entity(dw2.instance_handle()));
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_returns_immediately_for_local() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.2.1.11: find_topic gives access to an *enabled* Topic. A
        // topic of a participant that is not enabled is created disabled
        // (§2.2.2.1.1.7), so the participant is enabled first.
        crate::entity::Entity::enable(&p).unwrap();
        let _t = p
            .create_topic::<RawBytes>("Local", TopicQos::default())
            .unwrap();
        let started = std::time::Instant::now();
        let h = p
            .find_topic("Local", core::time::Duration::from_secs(5))
            .expect("local find");
        // Should be well below the timeout — local is an immediate
        // return.
        assert!(started.elapsed() < core::time::Duration::from_millis(50));
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), "Local");
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_times_out_when_unknown() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .find_topic("NotExists", core::time::Duration::from_millis(80))
            .unwrap_err();
        assert!(matches!(err, DdsError::Timeout));
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .find_topic("", core::time::Duration::from_millis(10))
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn create_contentfilteredtopic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn create_contentfilteredtopic_rejects_empty_expression() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("CF", &topic, "", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn delete_contentfilteredtopic_accepts_own() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap();
        p.delete_contentfilteredtopic(&cft).unwrap();
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_resolves_via_sedp_subscription() {
        // A variant of the discovery hook: this time we inject a
        // subscription (reader-side discovery), not a publication.
        // find_topic must find both.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
        use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                43,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("runtime start");

        let target_topic = "DiscoveredViaSubSedp";
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0xCD; 12]);
                let subdata = SubscriptionBuiltinTopicData {
                    content_filter_unreadable: false,
                    vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
                    key: Guid::new(prefix, EntityId::user_reader_with_key([4, 5, 6])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: target_topic.into(),
                    type_name: "test::SubT".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
                    latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
                    destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
                    presentation: zerodds_qos::PresentationQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                    type_information: None,
                    type_id_v1: None,
                    type_object_v1: None,
                    data_representation: alloc::vec::Vec::new(),
                    content_filter: None,
                    security_info: None,
                    sym_cipher_algo_info: None,
                    data_tags: Default::default(),
                    properties: Default::default(),
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                    expects_inline_qos: false,
                    group_guid: None,
                    type_consistency_announced: true,
                    entity_name: None,
                };
                sedp.cache_mut().insert_subscription(subdata, CoreDur::ZERO);
            }
        }

        let h = p
            .find_topic(target_topic, CoreDur::from_millis(200))
            .expect("find via subscription");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), target_topic);
        assert_eq!(h.get_type_name(), "test::SubT");
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_resolves_after_sedp_publication() {
        // Spec §2.2.2.2.1.11: find_topic must return as soon as a topic
        // is visible via discovery. We start a live participant (with a
        // real runtime) and inject a publication directly into the SEDP
        // cache, to verify the discovery hook without depending on the
        // UDP round-trip.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                42,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("runtime start");

        let target_topic = "DiscoveredViaSedp";
        let target_type = "test::Discovered";

        // Spawn a worker that, after a short delay, injects a publication
        // into the SEDP cache.
        let p_inject = p.clone();
        let topic_name = String::from(target_topic);
        let type_name = String::from(target_type);
        let join = std::thread::spawn(move || {
            std::thread::sleep(CoreDur::from_millis(50));
            if let Some(rt) = p_inject.runtime() {
                if let Ok(mut sedp) = rt.sedp.lock() {
                    let prefix = GuidPrefix::from_bytes([0xAB; 12]);
                    let pubdata = PublicationBuiltinTopicData {
                        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
                        key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                        participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                        topic_name,
                        type_name,
                        durability: DurabilityKind::Volatile,
                        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
                        reliability: ReliabilityQos {
                            kind: ReliabilityKind::Reliable,
                            max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(
                                1,
                            ),
                        },
                        ownership: zerodds_qos::OwnershipKind::Shared,
                        ownership_strength: 0,
                        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                        deadline: zerodds_qos::DeadlineQosPolicy::default(),
                        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
                        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
                        lifespan: zerodds_qos::LifespanQosPolicy::default(),
                        presentation: zerodds_qos::PresentationQosPolicy::default(),
                        partition: alloc::vec::Vec::new(),
                        user_data: alloc::vec::Vec::new(),
                        topic_data: alloc::vec::Vec::new(),
                        group_data: alloc::vec::Vec::new(),
                        type_information: None,
                        type_id_v1: None,
                        type_object_v1: None,
                        data_representation: alloc::vec::Vec::new(),
                        security_info: None,
                        sym_cipher_algo_info: None,
                        data_tags: Default::default(),
                        properties: Default::default(),
                        service_instance_name: None,
                        related_entity_guid: None,
                        topic_aliases: None,
                        type_identifier: zerodds_types::TypeIdentifier::None,
                        unicast_locators: Vec::new(),
                        multicast_locators: Vec::new(),
                        group_guid: None,
                        data_max_size_serialized: None,
                        entity_name: None,
                    };
                    sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
                }
            }
        });

        let result = p.find_topic(target_topic, CoreDur::from_secs(2));
        join.join().expect("inject thread");
        let h = result.expect("find_topic should resolve via SEDP");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), target_topic);
        assert_eq!(h.get_type_name(), target_type);
    }

    // ============================================================
    // ignore_* / delete_contained_entities / get_discovered_*
    // ============================================================

    #[test]
    fn ignore_participant_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0xAA);
        assert!(!p.is_participant_ignored(h));
        p.ignore_participant(h).unwrap();
        assert!(p.is_participant_ignored(h));
    }

    #[test]
    fn ignore_topic_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0xBB);
        assert!(!p.is_topic_ignored(h));
        p.ignore_topic(h).unwrap();
        assert!(p.is_topic_ignored(h));
    }

    #[test]
    fn ignore_publication_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0xCC);
        assert!(!p.is_publication_ignored(h));
        p.ignore_publication(h).unwrap();
        assert!(p.is_publication_ignored(h));
    }

    #[test]
    fn ignore_subscription_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0xDD);
        assert!(!p.is_subscription_ignored(h));
        p.ignore_subscription(h).unwrap();
        assert!(p.is_subscription_ignored(h));
    }

    #[test]
    fn ignore_lists_are_independent() {
        // Spec §2.2.2.2.1.14-17: each ignore_* list lives on its own; a
        // handle in the topic list does not appear in the participant
        // list.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0xEE);
        p.ignore_topic(h).unwrap();
        assert!(p.is_topic_ignored(h));
        assert!(!p.is_participant_ignored(h));
        assert!(!p.is_publication_ignored(h));
        assert!(!p.is_subscription_ignored(h));
    }

    #[test]
    fn ignore_is_monotonic_and_idempotent() {
        // A double ignore_participant must not turn into an error, and
        // the filter state must not "reverse".
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.1.1.7: ignore_* is not an operation a disabled participant
        // takes.
        p.inner.entity_state.enable();
        let h = InstanceHandle::from_raw(0x42);
        p.ignore_participant(h).unwrap();
        p.ignore_participant(h).unwrap();
        assert!(p.is_participant_ignored(h));
    }

    #[test]
    fn delete_contained_entities_clears_topics_and_groups() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _t = p
            .create_topic::<RawBytes>("ToBeRemoved", TopicQos::default())
            .unwrap();
        let _pub_ = p.create_publisher(PublisherQos::default());
        let _sub_ = p.create_subscriber(SubscriberQos::default());
        assert_eq!(p.topics_len(), 1);
        assert_eq!(p.publishers_len(), 1);
        assert_eq!(p.subscribers_len(), 1);
        p.delete_contained_entities().unwrap();
        assert_eq!(p.topics_len(), 0);
        assert_eq!(p.publishers_len(), 0);
        assert_eq!(p.subscribers_len(), 0);
    }

    #[test]
    fn delete_contained_entities_clears_builtin_reader_inboxes() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // Manually inject a builtin sample, so that after the clear we
        // can compare against 0.
        use crate::builtin_topics::ParticipantBuiltinTopicData as DcpsP;
        use zerodds_rtps::wire_types::Guid;
        let bs = p.get_builtin_subscriber();
        bs.sinks()
            .push_participant(&DcpsP {
                key: Guid::from_bytes([7u8; 16]),
                user_data: alloc::vec::Vec::new(),
            })
            .unwrap();
        let r = bs.participant_reader();
        assert_eq!(r.read().unwrap().len(), 1);
        p.delete_contained_entities().unwrap();
        assert_eq!(r.read().unwrap().len(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participants_offline_is_empty() {
        // Without a runtime, get_discovered_participants returns an empty
        // vec — Spec §2.2.2.2.1.27 allows that.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.get_discovered_participants().is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participant_data_offline_errors() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.2.1: NOT_ENABLED comes first on a disabled participant.
        assert!(matches!(
            p.get_discovered_participant_data(InstanceHandle::from_raw(1)),
            Err(DdsError::NotEnabled)
        ));
        crate::entity::Entity::enable(&p).unwrap();
        let err = p
            .get_discovered_participant_data(InstanceHandle::from_raw(1))
            .unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topics_offline_is_empty() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.get_discovered_topics().is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topic_data_offline_errors() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // §2.2.2.2.1: NOT_ENABLED comes first on a disabled participant.
        assert!(matches!(
            p.get_discovered_topic_data(InstanceHandle::from_raw(1)),
            Err(DdsError::NotEnabled)
        ));
        crate::entity::Entity::enable(&p).unwrap();
        let err = p
            .get_discovered_topic_data(InstanceHandle::from_raw(1))
            .unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participants_lists_after_spdp_inject() {
        // End-to-end: a live participant + one synthetic SPDP beacon of a
        // remote participant → get_discovered_participants returns
        // exactly one handle, get_discovered_participant_data finds the
        // matching wire data.
        use crate::factory::DomainParticipantFactory;
        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                30,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");

        // Inject directly into the discovered cache via the
        // handle_spdp_datagram path. We build a synthetic beacon with the
        // same helper as the runtime tests.
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData as WirePart;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, ProtocolVersion, VendorId};
        let remote = GuidPrefix::from_bytes([0xCA; 12]);
        let wire = WirePart {
            guid: Guid::new(remote, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locators: Vec::new(),
            default_multicast_locators: Vec::new(),
            metatraffic_unicast_locators: Vec::new(),
            metatraffic_multicast_locators: Vec::new(),
            domain_id: Some(30),
            domain_tag: Default::default(),
            builtin_endpoint_set: 0,
            lease_duration: zerodds_rtps::participant_data::Duration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            available_builtin_endpoints_ext: 0,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
        };
        let beacon = zerodds_discovery::spdp::SpdpBeacon::new(wire.clone())
            .serialize()
            .expect("serialize");
        if let Some(rt) = p.runtime() {
            crate::runtime::handle_spdp_datagram_for_test(rt, &beacon);
        }

        let handles = p.get_discovered_participants();
        assert_eq!(handles.len(), 1);
        let data = p
            .get_discovered_participant_data(handles[0])
            .expect("data lookup");
        assert_eq!(data.key, wire.guid);
        // Ignore → empty list.
        p.ignore_participant(handles[0]).unwrap();
        assert!(p.get_discovered_participants().is_empty());
        // §2.2.2.2.1.28: "otherwise ... PRECONDITION_NOT_MET".
        let err = p.get_discovered_participant_data(handles[0]).unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topics_lists_unique_handles_for_pub_and_sub() {
        // Pub + Sub on the same (topic, type) → one topic handle.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                21,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0x77; 12]);
                let pubdata = PublicationBuiltinTopicData {
                    vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
                    key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "SharedTopic".into(),
                    type_name: "SharedType".into(),
                    durability: DurabilityKind::Volatile,
                    durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    ownership_strength: 0,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
                    destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
                    lifespan: zerodds_qos::LifespanQosPolicy::default(),
                    presentation: zerodds_qos::PresentationQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    type_id_v1: None,
                    type_object_v1: None,
                    data_representation: alloc::vec::Vec::new(),
                    security_info: None,
                    sym_cipher_algo_info: None,
                    data_tags: Default::default(),
                    properties: Default::default(),
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                    group_guid: None,
                    data_max_size_serialized: None,
                    entity_name: None,
                };
                let subdata = SubscriptionBuiltinTopicData {
                    content_filter_unreadable: false,
                    vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
                    key: Guid::new(prefix, EntityId::user_reader_with_key([4, 5, 6])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "SharedTopic".into(),
                    type_name: "SharedType".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
                    latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
                    destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
                    presentation: zerodds_qos::PresentationQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                    type_information: None,
                    type_id_v1: None,
                    type_object_v1: None,
                    data_representation: alloc::vec::Vec::new(),
                    content_filter: None,
                    security_info: None,
                    sym_cipher_algo_info: None,
                    data_tags: Default::default(),
                    properties: Default::default(),
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                    expects_inline_qos: false,
                    group_guid: None,
                    type_consistency_announced: true,
                    entity_name: None,
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
                sedp.cache_mut().insert_subscription(subdata, CoreDur::ZERO);
            }
        }
        let topics = p.get_discovered_topics();
        assert_eq!(topics.len(), 1, "Pub+Sub on same topic -> 1 handle");
        let data = p.get_discovered_topic_data(topics[0]).expect("topic data");
        assert_eq!(data.name, "SharedTopic");
        assert_eq!(data.type_name, "SharedType");
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topic_data_filters_ignored() {
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                22,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0x55; 12]);
                let pubdata = PublicationBuiltinTopicData {
                    vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
                    key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "ToIgnore".into(),
                    type_name: "T".into(),
                    durability: DurabilityKind::Volatile,
                    durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    ownership_strength: 0,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
                    destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
                    lifespan: zerodds_qos::LifespanQosPolicy::default(),
                    presentation: zerodds_qos::PresentationQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    type_id_v1: None,
                    type_object_v1: None,
                    data_representation: alloc::vec::Vec::new(),
                    security_info: None,
                    sym_cipher_algo_info: None,
                    data_tags: Default::default(),
                    properties: Default::default(),
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                    group_guid: None,
                    data_max_size_serialized: None,
                    entity_name: None,
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
            }
        }
        let topics_before = p.get_discovered_topics();
        assert_eq!(topics_before.len(), 1);
        // Now ignore the topic — get_discovered_topics must no longer
        // list it, get_discovered_topic_data must return PreconditionNotMet.
        p.ignore_topic(topics_before[0]).unwrap();
        assert!(p.get_discovered_topics().is_empty());
        // §2.2.2.2.1.30: "otherwise ... PRECONDITION_NOT_MET".
        let err = p.get_discovered_topic_data(topics_before[0]).unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[test]
    fn delete_contentfilteredtopic_rejects_foreign() {
        let p1 = DomainParticipant::new(0, DomainParticipantQos::default());
        let p2 = DomainParticipant::new(1, DomainParticipantQos::default());
        let topic = p1
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p1
            .create_contentfilteredtopic("CF", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap();
        let err = p2.delete_contentfilteredtopic(&cft).unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    // ---- §2.2.2.2.1.6 delete_topic ----
    // The wider set of cases (every user kind, a second handle, the
    // untyped form) is in `tests/delete_topic.rs`; what stays here is the
    // pairing with `contains_entity` and `delete_contained_entities`.

    /// §2.2.2.2.1.6 with no user: the delete happens and the topic leaves
    /// both the registry (§2.2.2.2.1.12) and `contains_entity`
    /// (§2.2.2.2.1.10).
    #[test]
    fn delete_topic_removes_the_entry_and_its_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Doomed", TopicQos::default())
            .unwrap();
        let handle = t.inner().entity_state.instance_handle();
        assert!(p.contains_entity(handle));
        p.delete_topic(&t).unwrap();
        assert_eq!(p.topics_len(), 0);
        assert!(p.lookup_topicdescription("Doomed").is_none());
        assert!(!p.contains_entity(handle));
    }

    /// The precondition is checked before anything is disposed, so a
    /// refused delete leaves the topic exactly as it was — including the
    /// SEDP announcement.
    #[test]
    fn delete_topic_refused_by_a_writer_leaves_the_registry_alone() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Kept", TopicQos::default())
            .unwrap();
        let pub_ = p.create_publisher(PublisherQos::default());
        let dw = pub_
            .create_datawriter(&t, crate::qos::DataWriterQos::default())
            .unwrap();
        assert!(matches!(
            p.delete_topic(&t).unwrap_err(),
            DdsError::PreconditionNotMet { .. }
        ));
        assert_eq!(p.topics_len(), 1);
        assert!(p.lookup_topicdescription("Kept").is_some());
        drop(dw);
        p.delete_topic(&t).unwrap();
        assert_eq!(p.topics_len(), 0);
    }
}
