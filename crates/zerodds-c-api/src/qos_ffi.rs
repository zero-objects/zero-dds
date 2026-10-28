// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! QoS C-FFI structs + conversions (Spec §2.2.3 + DDS-PSM-Cxx §7.2.4).
//!
//! All 22 normative QoS policies are exposed as `#[repr(C)]` structs with
//! an exact field layout. The caller fills them in directly in C
//! (no builder boilerplate).
//!
//! `Vec<u8>`/`Vec<String>` fields (UserData, TopicData, GroupData, Partition)
//! are carried as `(ptr, len)` pairs. The FFI structs do **not** own the
//! pointed-to data — the caller remains the owner; on `set_qos` the
//! converter copies the bytes/strings into the Rust heap.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;
use core::slice;
use std::ffi::CStr;

use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::{
    DataTagQosPolicy, DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy,
    DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy, Duration,
    EntityFactoryQosPolicy, GroupDataQosPolicy, HistoryKind, HistoryQosPolicy,
    LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
    OwnershipQosPolicy, OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope,
    PresentationQosPolicy, PropertyQosPolicy, ReaderDataLifecycleQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy, ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy,
};

use crate::ZeroDdsStatus;

// ---------------------------------------------------------------------------
// Time / Duration
// ---------------------------------------------------------------------------

/// Time point (Spec §2.2.3.5 + IDL §9.3.2). seconds + nanoseconds form
/// (the spec's `zerodds_Time`; cbindgen emits it as `zerodds_ZeroDdsTime`,
/// like every other FFI struct). Used by `zerodds_dp_get_current_time`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTime {
    /// Seconds.
    pub sec: i32,
    /// Nanoseconds.
    pub nanosec: u32,
}

/// Duration (Spec §2.2.3.5 + IDL §9.3.2). seconds + nanoseconds form.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDuration {
    /// Seconds.
    pub sec: i32,
    /// Nanoseconds.
    pub nanosec: u32,
}

impl ZeroDdsDuration {
    /// INFINITE marker.
    pub const INFINITE: Self = Self {
        sec: i32::MAX,
        nanosec: u32::MAX,
    };
}

impl From<Duration> for ZeroDdsDuration {
    fn from(d: Duration) -> Self {
        if d.is_infinite() {
            return Self::INFINITE;
        }
        // fraction (2^-32 s) → nanoseconds, rounded: a set_qos of whole
        // nanoseconds reads back as set.
        let (sec, nanosec) = d.to_sec_nanosec();
        Self { sec, nanosec }
    }
}

impl From<ZeroDdsDuration> for Duration {
    fn from(d: ZeroDdsDuration) -> Self {
        if d.sec == i32::MAX && d.nanosec == u32::MAX {
            return Duration::INFINITE;
        }
        Duration::from_sec_nanosec(d.sec, d.nanosec)
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Reliability-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsReliabilityKind {
    /// Best-effort.
    BestEffort = 1,
    /// Reliable.
    Reliable = 2,
}

/// Durability-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsDurabilityKind {
    /// Volatile.
    Volatile = 0,
    /// Transient-local.
    TransientLocal = 1,
    /// Transient.
    Transient = 2,
    /// Persistent.
    Persistent = 3,
}

/// Liveliness-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsLivelinessKind {
    /// Automatic.
    Automatic = 0,
    /// Manual by participant.
    ManualByParticipant = 1,
    /// Manual by topic.
    ManualByTopic = 2,
}

/// History-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsHistoryKind {
    /// Keep last N.
    KeepLast = 0,
    /// Keep all.
    KeepAll = 1,
}

/// Ownership-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsOwnershipKind {
    /// Shared.
    Shared = 0,
    /// Exclusive.
    Exclusive = 1,
}

/// DestinationOrder-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsDestinationOrderKind {
    /// By reception timestamp.
    ByReceptionTimestamp = 0,
    /// By source timestamp.
    BySourceTimestamp = 1,
}

/// Presentation-AccessScope.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsPresentationAccessScope {
    /// Instance.
    Instance = 0,
    /// Topic.
    Topic = 1,
    /// Group.
    Group = 2,
}

// ---------------------------------------------------------------------------
// Policies (22)
// ---------------------------------------------------------------------------

/// ReliabilityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsReliabilityQosPolicy {
    /// Kind (1=BestEffort, 2=Reliable).
    pub kind: u32,
    /// max_blocking_time.
    pub max_blocking_time: ZeroDdsDuration,
}

/// DurabilityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDurabilityQosPolicy {
    /// Kind (0=Volatile, 1=TransientLocal, 2=Transient, 3=Persistent).
    pub kind: u32,
}

/// HistoryQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsHistoryQosPolicy {
    /// Kind (0=KeepLast, 1=KeepAll).
    pub kind: u32,
    /// Depth (for KeepLast).
    pub depth: i32,
}

/// DeadlineQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDeadlineQosPolicy {
    /// period.
    pub period: ZeroDdsDuration,
}

/// LifespanQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLifespanQosPolicy {
    /// duration.
    pub duration: ZeroDdsDuration,
}

/// LatencyBudgetQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLatencyBudgetQosPolicy {
    /// duration.
    pub duration: ZeroDdsDuration,
}

/// TimeBasedFilterQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTimeBasedFilterQosPolicy {
    /// minimum_separation.
    pub minimum_separation: ZeroDdsDuration,
}

/// LivelinessQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLivelinessQosPolicy {
    /// Kind.
    pub kind: u32,
    /// lease_duration.
    pub lease_duration: ZeroDdsDuration,
}

/// OwnershipQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsOwnershipQosPolicy {
    /// Kind (0=Shared, 1=Exclusive).
    pub kind: u32,
}

/// OwnershipStrengthQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsOwnershipStrengthQosPolicy {
    /// strength.
    pub value: i32,
}

/// DestinationOrderQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDestinationOrderQosPolicy {
    /// Kind (0=ByReception, 1=BySource).
    pub kind: u32,
}

/// PresentationQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPresentationQosPolicy {
    /// access_scope.
    pub access_scope: u32,
    /// coherent_access.
    pub coherent_access: bool,
    /// ordered_access.
    pub ordered_access: bool,
}

/// ResourceLimitsQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsResourceLimitsQosPolicy {
    /// max_samples.
    pub max_samples: i32,
    /// max_instances.
    pub max_instances: i32,
    /// max_samples_per_instance.
    pub max_samples_per_instance: i32,
}

/// TransportPriorityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTransportPriorityQosPolicy {
    /// priority.
    pub value: i32,
}

/// EntityFactoryQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsEntityFactoryQosPolicy {
    /// autoenable_created_entities.
    pub autoenable_created_entities: bool,
}

/// WriterDataLifecycleQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsWriterDataLifecycleQosPolicy {
    /// autodispose_unregistered_instances.
    pub autodispose_unregistered_instances: bool,
}

/// ReaderDataLifecycleQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsReaderDataLifecycleQosPolicy {
    /// autopurge_nowriter_samples_delay.
    pub autopurge_nowriter_samples_delay: ZeroDdsDuration,
    /// autopurge_disposed_samples_delay.
    pub autopurge_disposed_samples_delay: ZeroDdsDuration,
}

/// DurabilityServiceQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDurabilityServiceQosPolicy {
    /// service_cleanup_delay.
    pub service_cleanup_delay: ZeroDdsDuration,
    /// history_kind (0=KeepLast, 1=KeepAll).
    pub history_kind: u32,
    /// history_depth.
    pub history_depth: i32,
    /// max_samples.
    pub max_samples: i32,
    /// max_instances.
    pub max_instances: i32,
    /// max_samples_per_instance.
    pub max_samples_per_instance: i32,
}

/// UserDataQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsUserDataQosPolicy {
    /// Pointer to bytes (Caller-owned).
    pub value: *const u8,
    /// Length.
    pub value_len: usize,
}

/// TopicDataQosPolicy.
pub type ZeroDdsTopicDataQosPolicy = ZeroDdsUserDataQosPolicy;

/// The longest list of representation identifiers a
/// [`ZeroDdsDataRepresentationQosPolicy`] holds; `ids` has this length (the
/// array length is written as a number, so that the generated header has it
/// too).
pub const ZERODDS_DATA_REPRESENTATION_MAX: usize = 8;

/// DataRepresentationQosPolicy (DDS-XTypes 1.3 §7.6.3.1.1, Annex D): the list
/// of representation identifiers (`XCDR` = 0, `XML` = 1, `XCDR2` = 2, negative
/// values for vendors).
///
/// `is_set` = 0 leaves the policy at its default, which the runtime resolves
/// to its own offer; an all-zero struct is that. `is_set` = 1 makes
/// `ids[0..count]` the list, and `count` = 0 is the empty list, "equivalent to
/// a list containing the single element XCDR" (§7.6.3.1.1). "When
/// representations are specified in the TopicQos, the first element of the
/// sequence applies to writers of the Topic, and the whole sequence applies
/// to readers of the Topic."
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsDataRepresentationQosPolicy {
    /// 0: left at the default; 1: `ids[0..count]` is the list.
    pub is_set: u8,
    /// Number of identifiers in `ids`, at most 8.
    pub count: u8,
    /// The identifiers.
    pub ids: [i16; 8],
}

const _: () = assert!(
    core::mem::size_of::<[i16; ZERODDS_DATA_REPRESENTATION_MAX]>() == 2 * 8,
    "ZERODDS_DATA_REPRESENTATION_MAX is the length of `ids`"
);

impl ZeroDdsDataRepresentationQosPolicy {
    /// The policy as the DCPS QoS holds it. A `count` past the capacity of
    /// `ids` is read as the capacity.
    fn to_qos(self) -> Option<Vec<i16>> {
        if self.is_set == 0 {
            return None;
        }
        let n = usize::from(self.count).min(ZERODDS_DATA_REPRESENTATION_MAX);
        Some(self.ids[..n].to_vec())
    }

    /// The C form of the DCPS QoS value.
    ///
    /// # Errors
    /// `OutOfResources` when the list is longer than `ids` holds: the list is
    /// not cut.
    fn from_qos(value: Option<&[i16]>) -> Result<Self, ZeroDdsStatus> {
        let mut out = Self::default();
        if let Some(list) = value {
            if list.len() > ZERODDS_DATA_REPRESENTATION_MAX {
                return Err(ZeroDdsStatus::OutOfResources);
            }
            out.is_set = 1;
            out.count = list.len() as u8;
            out.ids[..list.len()].copy_from_slice(list);
        }
        Ok(out)
    }
}

/// TypeConsistencyEnforcementQosPolicy (DDS-XTypes 1.3 §7.6.3.4.1, Annex D).
///
/// `is_set` = 0 leaves the policy at its default (`ALLOW_TYPE_COERCION` with
/// the defaults of the policy); an all-zero struct is that. `kind` is
/// `DISALLOW_TYPE_COERCION` = 0 or `ALLOW_TYPE_COERCION` = 1; the five flags
/// are 0 or 1.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsTypeConsistencyEnforcementQosPolicy {
    /// 0: left at the default; 1: the fields below are the policy.
    pub is_set: u8,
    /// `TypeConsistencyKind`.
    pub kind: u8,
    /// `ignore_sequence_bounds`.
    pub ignore_sequence_bounds: u8,
    /// `ignore_string_bounds`.
    pub ignore_string_bounds: u8,
    /// `ignore_member_names`.
    pub ignore_member_names: u8,
    /// `prevent_type_widening`.
    pub prevent_type_widening: u8,
    /// `force_type_validation`.
    pub force_type_validation: u8,
}

impl ZeroDdsTypeConsistencyEnforcementQosPolicy {
    /// The policy as the DCPS QoS holds it.
    ///
    /// # Errors
    /// `BadParameter` for a `kind` Annex D does not define.
    fn to_qos(self) -> Result<zerodds_types::qos::TypeConsistencyEnforcement, ZeroDdsStatus> {
        if self.is_set == 0 {
            return Ok(zerodds_types::qos::TypeConsistencyEnforcement::default());
        }
        let kind = zerodds_types::qos::TypeConsistencyKind::from_u32(u32::from(self.kind))
            .map_err(|_| ZeroDdsStatus::BadParameter)?;
        Ok(zerodds_types::qos::TypeConsistencyEnforcement {
            kind,
            ignore_sequence_bounds: self.ignore_sequence_bounds != 0,
            ignore_string_bounds: self.ignore_string_bounds != 0,
            ignore_member_names: self.ignore_member_names != 0,
            prevent_type_widening: self.prevent_type_widening != 0,
            force_type_validation: self.force_type_validation != 0,
        })
    }

    /// The C form of the DCPS QoS value, as a policy that is set.
    fn from_qos(value: &zerodds_types::qos::TypeConsistencyEnforcement) -> Self {
        Self {
            is_set: 1,
            kind: value.kind.to_u32() as u8,
            ignore_sequence_bounds: u8::from(value.ignore_sequence_bounds),
            ignore_string_bounds: u8::from(value.ignore_string_bounds),
            ignore_member_names: u8::from(value.ignore_member_names),
            prevent_type_widening: u8::from(value.prevent_type_widening),
            force_type_validation: u8::from(value.force_type_validation),
        }
    }
}
/// GroupDataQosPolicy.
pub type ZeroDdsGroupDataQosPolicy = ZeroDdsUserDataQosPolicy;

/// PartitionQosPolicy. List of C-string pointers + count.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPartitionQosPolicy {
    /// Array of C string pointers (caller-owned).
    pub names: *const *const c_char,
    /// Count.
    pub names_len: usize,
}

/// One string property of a [`ZeroDdsPropertyQosPolicy`] (DDS-Security 1.2
/// §7.3.3 `Property_t`: name, value, `propagate`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPropertyEntry {
    /// NUL-terminated name (caller-owned).
    pub name: *const c_char,
    /// NUL-terminated value (caller-owned).
    pub value: *const c_char,
    /// `propagate`: 1 announces the entry in the discovery data, 0 keeps it
    /// local (§7.3.21).
    pub propagate: u8,
}

/// One binary property of a [`ZeroDdsPropertyQosPolicy`] (§7.3.3
/// `BinaryProperty_t`: name, value, `propagate`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsBinaryPropertyEntry {
    /// NUL-terminated name (caller-owned).
    pub name: *const c_char,
    /// Pointer to the value bytes (caller-owned).
    pub value: *const u8,
    /// Length of the value.
    pub value_len: usize,
    /// `propagate`, see [`ZeroDdsPropertyEntry::propagate`].
    pub propagate: u8,
}

/// PropertyQosPolicy (DDS-Security 1.2 §7.3.21): `value` and `binary_value`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPropertyQosPolicy {
    /// Array of string properties (caller-owned).
    pub entries: *const ZeroDdsPropertyEntry,
    /// Count of `entries`.
    pub entries_len: usize,
    /// Array of binary properties (caller-owned).
    pub binary_entries: *const ZeroDdsBinaryPropertyEntry,
    /// Count of `binary_entries`.
    pub binary_entries_len: usize,
}

/// One tag of a [`ZeroDdsDataTagQosPolicy`] (§7.3.21 `Tag`: name, value).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTag {
    /// NUL-terminated name (caller-owned).
    pub name: *const c_char,
    /// NUL-terminated value (caller-owned).
    pub value: *const c_char,
}

/// DataTagQosPolicy (DDS-Security 1.2 §7.3.21): `TagSeq tags`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDataTagQosPolicy {
    /// Array of tags (caller-owned).
    pub tags: *const ZeroDdsTag,
    /// Count of `tags`.
    pub tags_len: usize,
}

/// Stable backing store for the PARTITION names emitted on a `get_qos` /
/// `get_default_*_qos` OUT path. The `ZeroDdsPartitionQosPolicy` carries the
/// names as a `*const *const c_char` borrow — the pointed-to C strings and the
/// pointer array must outlive the `*_qos_to_c` call, so they are owned by the
/// entity (a `Mutex<PartitionOutCache>`) and refreshed on each marshalling.
/// (DDS-PSM-Cxx §7.2.4 PartitionQosPolicy; the names are an
/// implementation-owned snapshot, not caller-owned like the IN path.)
#[derive(Default)]
pub struct PartitionOutCache {
    /// Owned NUL-terminated copies of each partition name.
    strings: Vec<std::ffi::CString>,
    /// `*const c_char` view over `strings`, the array `names` points at.
    ptrs: Vec<*const c_char>,
    /// Owned strings behind the `property` and `data_tags` OUT fields.
    property_strings: Vec<std::ffi::CString>,
    /// Owned values of the binary properties.
    property_values: Vec<Vec<u8>>,
    /// The string entries `property.entries` points at.
    property_entries: Vec<ZeroDdsPropertyEntry>,
    /// The binary entries `property.binary_entries` points at.
    property_binary_entries: Vec<ZeroDdsBinaryPropertyEntry>,
    /// Owned strings behind the tags.
    tag_strings: Vec<std::ffi::CString>,
    /// The tags `data_tags.tags` points at.
    tag_entries: Vec<ZeroDdsTag>,
}

// SAFETY: the raw pointers in `ptrs` only ever alias `strings` in the same
// struct; the cache is always guarded by a Mutex on the owning entity.
unsafe impl Send for PartitionOutCache {}

/// A C string of `text`, cut at the first interior NUL.
fn c_string_of(text: &str) -> std::ffi::CString {
    let bytes = text.as_bytes();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::ffi::CString::new(&bytes[..end]).unwrap_or_default()
}

impl PartitionOutCache {
    /// Refreshes the cache from `policy` and points `out` at its entries
    /// (valid until the next `get_qos` on the same entity).
    fn marshal_property(&mut self, policy: &PropertyQosPolicy, out: &mut ZeroDdsPropertyQosPolicy) {
        self.property_strings.clear();
        self.property_values.clear();
        self.property_entries.clear();
        self.property_binary_entries.clear();
        // Fill the owners first: the pointers are taken after the vectors stop
        // growing (a CString keeps its heap buffer, a Vec<u8> too).
        let strings: Vec<(std::ffi::CString, std::ffi::CString, u8)> = policy
            .entries()
            .map(|(n, v, p)| (c_string_of(n), c_string_of(v), u8::from(p)))
            .collect();
        let binary: Vec<(std::ffi::CString, Vec<u8>, u8)> = policy
            .binary_entries()
            .map(|(n, v, p)| (c_string_of(n), v.to_vec(), u8::from(p)))
            .collect();
        for (name, value, propagate) in strings {
            self.property_entries.push(ZeroDdsPropertyEntry {
                name: name.as_ptr(),
                value: value.as_ptr(),
                propagate,
            });
            self.property_strings.push(name);
            self.property_strings.push(value);
        }
        for (name, value, propagate) in binary {
            self.property_binary_entries
                .push(ZeroDdsBinaryPropertyEntry {
                    name: name.as_ptr(),
                    value: value.as_ptr(),
                    value_len: value.len(),
                    propagate,
                });
            self.property_strings.push(name);
            self.property_values.push(value);
        }
        *out = ZeroDdsPropertyQosPolicy {
            entries: if self.property_entries.is_empty() {
                ptr::null()
            } else {
                self.property_entries.as_ptr()
            },
            entries_len: self.property_entries.len(),
            binary_entries: if self.property_binary_entries.is_empty() {
                ptr::null()
            } else {
                self.property_binary_entries.as_ptr()
            },
            binary_entries_len: self.property_binary_entries.len(),
        };
    }

    /// Refreshes the cache from `policy` and points `out` at its tags.
    fn marshal_tags(&mut self, policy: &DataTagQosPolicy, out: &mut ZeroDdsDataTagQosPolicy) {
        self.tag_strings.clear();
        self.tag_entries.clear();
        let tags: Vec<(std::ffi::CString, std::ffi::CString)> = policy
            .tags
            .iter()
            .map(|t| (c_string_of(&t.name), c_string_of(&t.value)))
            .collect();
        for (name, value) in tags {
            self.tag_entries.push(ZeroDdsTag {
                name: name.as_ptr(),
                value: value.as_ptr(),
            });
            self.tag_strings.push(name);
            self.tag_strings.push(value);
        }
        *out = ZeroDdsDataTagQosPolicy {
            tags: if self.tag_entries.is_empty() {
                ptr::null()
            } else {
                self.tag_entries.as_ptr()
            },
            tags_len: self.tag_entries.len(),
        };
    }

    /// Refresh the cache from the current partition names and point
    /// `out_names`/`out_len` at the (now stable) internal pointer array.
    ///
    /// # Safety
    /// `out_names`/`out_len` must be valid for writes.
    unsafe fn marshal(
        &mut self,
        names: &[String],
        out_names: *mut *const *const c_char,
        out_len: *mut usize,
    ) {
        self.strings = names
            .iter()
            // A NUL byte inside a partition name is invalid (DDS names are C
            // strings); truncate at the first interior NUL rather than fail the
            // whole get_qos. `CString::new` over a NUL-free prefix never errors.
            .map(|n| {
                let bytes = n.as_bytes();
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                std::ffi::CString::new(&bytes[..end]).unwrap_or_default()
            })
            .collect();
        self.ptrs = self.strings.iter().map(|c| c.as_ptr()).collect();
        // SAFETY: caller pledge — out pointers are valid for writes.
        unsafe {
            if self.ptrs.is_empty() {
                *out_names = ptr::null();
            } else {
                *out_names = self.ptrs.as_ptr();
            }
            *out_len = self.ptrs.len();
        }
    }
}

/// Marshals a PARTITION policy onto an OUT struct field, using the entity-owned
/// `cache` as stable backing for the names (see [`PartitionOutCache`]).
///
/// # Safety
/// `out_names`/`out_len` valid for writes; `cache` guarded by the caller.
pub unsafe fn marshal_partition_out(
    cache: &std::sync::Mutex<PartitionOutCache>,
    names: &[String],
    out_names: *mut *const *const c_char,
    out_len: *mut usize,
) {
    if let Ok(mut c) = cache.lock() {
        // SAFETY: out pointers valid (caller pledge); cache held for the write.
        unsafe { c.marshal(names, out_names, out_len) };
    } else {
        // SAFETY: out pointers valid (caller pledge).
        unsafe {
            *out_names = ptr::null();
            *out_len = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// QoS-Sets (6)
// ---------------------------------------------------------------------------

/// DomainParticipantFactoryQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDomainParticipantFactoryQos {
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// DomainParticipantQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDomainParticipantQos {
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// TopicQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTopicQos {
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DurabilityService.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TransportPriority.
    pub transport_priority: ZeroDdsTransportPriorityQosPolicy,
    /// Lifespan.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// DataRepresentation (DDS-XTypes 1.3 Annex D `TopicQos::representation`).
    pub data_representation: ZeroDdsDataRepresentationQosPolicy,
}

/// PublisherQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPublisherQos {
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// SubscriberQos.
pub type ZeroDdsSubscriberQos = ZeroDdsPublisherQos;

/// DataWriterQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDataWriterQos {
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DurabilityService.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// Lifespan.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// OwnershipStrength.
    pub ownership_strength: ZeroDdsOwnershipStrengthQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TransportPriority.
    pub transport_priority: ZeroDdsTransportPriorityQosPolicy,
    /// WriterDataLifecycle.
    pub writer_data_lifecycle: ZeroDdsWriterDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
    /// DataRepresentation (DDS-XTypes 1.3 Annex D `DataWriterQos::representation`).
    pub data_representation: ZeroDdsDataRepresentationQosPolicy,
    /// Property (DDS-Security 1.2 §7.3.21); all-zero is no entries.
    pub property: ZeroDdsPropertyQosPolicy,
    /// DataTags (DDS-Security 1.2 §7.3.21); all-zero is no tags.
    pub data_tags: ZeroDdsDataTagQosPolicy,
}

/// DataReaderQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDataReaderQos {
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TimeBasedFilter.
    pub time_based_filter: ZeroDdsTimeBasedFilterQosPolicy,
    /// ReaderDataLifecycle.
    pub reader_data_lifecycle: ZeroDdsReaderDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
    /// DataRepresentation (DDS-XTypes 1.3 Annex D `DataReaderQos::representation`).
    pub data_representation: ZeroDdsDataRepresentationQosPolicy,
    /// TypeConsistencyEnforcement (DDS-XTypes 1.3 Annex D
    /// `DataReaderQos::type_consistency`).
    pub type_consistency: ZeroDdsTypeConsistencyEnforcementQosPolicy,
    /// Property (DDS-Security 1.2 §7.3.21); all-zero is no entries.
    pub property: ZeroDdsPropertyQosPolicy,
    /// DataTags (DDS-Security 1.2 §7.3.21); all-zero is no tags.
    pub data_tags: ZeroDdsDataTagQosPolicy,
}

// ---------------------------------------------------------------------------
// Konvertierung (C → Rust + Rust → C)
// ---------------------------------------------------------------------------

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn slice_or_empty<'a>(p: *const u8, n: usize) -> &'a [u8] {
    if p.is_null() || n == 0 {
        &[]
    } else {
        // SAFETY: caller contract ((p,n) valid range).
        unsafe { slice::from_raw_parts(p, n) }
    }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn cstr_vec(arr: *const *const c_char, n: usize) -> Vec<String> {
    if arr.is_null() || n == 0 {
        return Vec::new();
    }
    // SAFETY: caller contract: arr[0..n] valid.
    let slc = unsafe { slice::from_raw_parts(arr, n) };
    let mut out = Vec::with_capacity(n);
    for &p in slc {
        if p.is_null() {
            continue;
        }
        // SAFETY: caller contract: p is a NUL-terminated string.
        let cs = unsafe { CStr::from_ptr(p) };
        if let Ok(s) = cs.to_str() {
            out.push(s.to_string());
        }
    }
    out
}

impl From<ZeroDdsReliabilityQosPolicy> for ReliabilityQosPolicy {
    fn from(c: ZeroDdsReliabilityQosPolicy) -> Self {
        Self {
            kind: ReliabilityKind::from_u32(c.kind),
            max_blocking_time: c.max_blocking_time.into(),
        }
    }
}
impl From<ReliabilityQosPolicy> for ZeroDdsReliabilityQosPolicy {
    fn from(r: ReliabilityQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            max_blocking_time: r.max_blocking_time.into(),
        }
    }
}

impl From<ZeroDdsDurabilityQosPolicy> for DurabilityQosPolicy {
    fn from(c: ZeroDdsDurabilityQosPolicy) -> Self {
        Self {
            kind: DurabilityKind::from_u32(c.kind),
        }
    }
}
impl From<DurabilityQosPolicy> for ZeroDdsDurabilityQosPolicy {
    fn from(r: DurabilityQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsHistoryQosPolicy> for HistoryQosPolicy {
    fn from(c: ZeroDdsHistoryQosPolicy) -> Self {
        Self {
            kind: HistoryKind::from_u32(c.kind),
            depth: c.depth,
        }
    }
}
impl From<HistoryQosPolicy> for ZeroDdsHistoryQosPolicy {
    fn from(r: HistoryQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            depth: r.depth,
        }
    }
}

impl From<ZeroDdsDeadlineQosPolicy> for DeadlineQosPolicy {
    fn from(c: ZeroDdsDeadlineQosPolicy) -> Self {
        Self {
            period: c.period.into(),
        }
    }
}
impl From<DeadlineQosPolicy> for ZeroDdsDeadlineQosPolicy {
    fn from(r: DeadlineQosPolicy) -> Self {
        Self {
            period: r.period.into(),
        }
    }
}

impl From<ZeroDdsLifespanQosPolicy> for LifespanQosPolicy {
    fn from(c: ZeroDdsLifespanQosPolicy) -> Self {
        Self {
            duration: c.duration.into(),
        }
    }
}
impl From<LifespanQosPolicy> for ZeroDdsLifespanQosPolicy {
    fn from(r: LifespanQosPolicy) -> Self {
        Self {
            duration: r.duration.into(),
        }
    }
}

impl From<ZeroDdsLatencyBudgetQosPolicy> for LatencyBudgetQosPolicy {
    fn from(c: ZeroDdsLatencyBudgetQosPolicy) -> Self {
        Self {
            duration: c.duration.into(),
        }
    }
}
impl From<LatencyBudgetQosPolicy> for ZeroDdsLatencyBudgetQosPolicy {
    fn from(r: LatencyBudgetQosPolicy) -> Self {
        Self {
            duration: r.duration.into(),
        }
    }
}

impl From<ZeroDdsTimeBasedFilterQosPolicy> for TimeBasedFilterQosPolicy {
    fn from(c: ZeroDdsTimeBasedFilterQosPolicy) -> Self {
        Self {
            minimum_separation: c.minimum_separation.into(),
        }
    }
}
impl From<TimeBasedFilterQosPolicy> for ZeroDdsTimeBasedFilterQosPolicy {
    fn from(r: TimeBasedFilterQosPolicy) -> Self {
        Self {
            minimum_separation: r.minimum_separation.into(),
        }
    }
}

impl From<ZeroDdsLivelinessQosPolicy> for LivelinessQosPolicy {
    fn from(c: ZeroDdsLivelinessQosPolicy) -> Self {
        Self {
            kind: LivelinessKind::from_u32(c.kind),
            lease_duration: c.lease_duration.into(),
        }
    }
}
impl From<LivelinessQosPolicy> for ZeroDdsLivelinessQosPolicy {
    fn from(r: LivelinessQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            lease_duration: r.lease_duration.into(),
        }
    }
}

impl From<ZeroDdsOwnershipQosPolicy> for OwnershipQosPolicy {
    fn from(c: ZeroDdsOwnershipQosPolicy) -> Self {
        Self {
            kind: OwnershipKind::from_u32(c.kind),
        }
    }
}
impl From<OwnershipQosPolicy> for ZeroDdsOwnershipQosPolicy {
    fn from(r: OwnershipQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsOwnershipStrengthQosPolicy> for OwnershipStrengthQosPolicy {
    fn from(c: ZeroDdsOwnershipStrengthQosPolicy) -> Self {
        Self { value: c.value }
    }
}
impl From<OwnershipStrengthQosPolicy> for ZeroDdsOwnershipStrengthQosPolicy {
    fn from(r: OwnershipStrengthQosPolicy) -> Self {
        Self { value: r.value }
    }
}

impl From<ZeroDdsDestinationOrderQosPolicy> for DestinationOrderQosPolicy {
    fn from(c: ZeroDdsDestinationOrderQosPolicy) -> Self {
        Self {
            kind: DestinationOrderKind::from_u32(c.kind),
        }
    }
}
impl From<DestinationOrderQosPolicy> for ZeroDdsDestinationOrderQosPolicy {
    fn from(r: DestinationOrderQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsPresentationQosPolicy> for PresentationQosPolicy {
    fn from(c: ZeroDdsPresentationQosPolicy) -> Self {
        Self {
            access_scope: PresentationAccessScope::from_u32(c.access_scope),
            coherent_access: c.coherent_access,
            ordered_access: c.ordered_access,
        }
    }
}
impl From<PresentationQosPolicy> for ZeroDdsPresentationQosPolicy {
    fn from(r: PresentationQosPolicy) -> Self {
        Self {
            access_scope: r.access_scope as u32,
            coherent_access: r.coherent_access,
            ordered_access: r.ordered_access,
        }
    }
}

impl From<ZeroDdsResourceLimitsQosPolicy> for ResourceLimitsQosPolicy {
    fn from(c: ZeroDdsResourceLimitsQosPolicy) -> Self {
        Self {
            max_samples: c.max_samples,
            max_instances: c.max_instances,
            max_samples_per_instance: c.max_samples_per_instance,
        }
    }
}
impl From<ResourceLimitsQosPolicy> for ZeroDdsResourceLimitsQosPolicy {
    fn from(r: ResourceLimitsQosPolicy) -> Self {
        Self {
            max_samples: r.max_samples,
            max_instances: r.max_instances,
            max_samples_per_instance: r.max_samples_per_instance,
        }
    }
}

impl From<ZeroDdsTransportPriorityQosPolicy> for TransportPriorityQosPolicy {
    fn from(c: ZeroDdsTransportPriorityQosPolicy) -> Self {
        Self { value: c.value }
    }
}
impl From<TransportPriorityQosPolicy> for ZeroDdsTransportPriorityQosPolicy {
    fn from(r: TransportPriorityQosPolicy) -> Self {
        Self { value: r.value }
    }
}

impl From<ZeroDdsEntityFactoryQosPolicy> for EntityFactoryQosPolicy {
    fn from(c: ZeroDdsEntityFactoryQosPolicy) -> Self {
        Self {
            autoenable_created_entities: c.autoenable_created_entities,
        }
    }
}
impl From<EntityFactoryQosPolicy> for ZeroDdsEntityFactoryQosPolicy {
    fn from(r: EntityFactoryQosPolicy) -> Self {
        Self {
            autoenable_created_entities: r.autoenable_created_entities,
        }
    }
}

impl From<ZeroDdsWriterDataLifecycleQosPolicy> for WriterDataLifecycleQosPolicy {
    fn from(c: ZeroDdsWriterDataLifecycleQosPolicy) -> Self {
        let mut p = WriterDataLifecycleQosPolicy::default();
        p.autodispose_unregistered_instances = c.autodispose_unregistered_instances;
        p
    }
}
impl From<WriterDataLifecycleQosPolicy> for ZeroDdsWriterDataLifecycleQosPolicy {
    fn from(r: WriterDataLifecycleQosPolicy) -> Self {
        Self {
            autodispose_unregistered_instances: r.autodispose_unregistered_instances,
        }
    }
}

impl From<ZeroDdsReaderDataLifecycleQosPolicy> for ReaderDataLifecycleQosPolicy {
    fn from(c: ZeroDdsReaderDataLifecycleQosPolicy) -> Self {
        let mut p = ReaderDataLifecycleQosPolicy::default();
        p.autopurge_nowriter_samples_delay = c.autopurge_nowriter_samples_delay.into();
        p.autopurge_disposed_samples_delay = c.autopurge_disposed_samples_delay.into();
        p
    }
}
impl From<ReaderDataLifecycleQosPolicy> for ZeroDdsReaderDataLifecycleQosPolicy {
    fn from(r: ReaderDataLifecycleQosPolicy) -> Self {
        Self {
            autopurge_nowriter_samples_delay: r.autopurge_nowriter_samples_delay.into(),
            autopurge_disposed_samples_delay: r.autopurge_disposed_samples_delay.into(),
        }
    }
}

impl From<ZeroDdsDurabilityServiceQosPolicy> for DurabilityServiceQosPolicy {
    fn from(c: ZeroDdsDurabilityServiceQosPolicy) -> Self {
        Self {
            service_cleanup_delay: c.service_cleanup_delay.into(),
            history_kind: HistoryKind::from_u32(c.history_kind),
            history_depth: c.history_depth,
            max_samples: c.max_samples,
            max_instances: c.max_instances,
            max_samples_per_instance: c.max_samples_per_instance,
        }
    }
}
impl From<DurabilityServiceQosPolicy> for ZeroDdsDurabilityServiceQosPolicy {
    fn from(r: DurabilityServiceQosPolicy) -> Self {
        Self {
            service_cleanup_delay: r.service_cleanup_delay.into(),
            history_kind: r.history_kind as u32,
            history_depth: r.history_depth,
            max_samples: r.max_samples,
            max_instances: r.max_instances,
            max_samples_per_instance: r.max_samples_per_instance,
        }
    }
}

// ---------------------------------------------------------------------------
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn property_from_c(c: &ZeroDdsPropertyQosPolicy) -> PropertyQosPolicy {
    let mut out = PropertyQosPolicy::new();
    // SAFETY: caller contract: entries[0..entries_len] and
    // binary_entries[0..binary_entries_len] valid, every pointer inside valid.
    unsafe {
        if !c.entries.is_null() {
            for e in slice::from_raw_parts(c.entries, c.entries_len) {
                if e.name.is_null() {
                    continue;
                }
                let value = if e.value.is_null() {
                    Some("")
                } else {
                    CStr::from_ptr(e.value).to_str().ok()
                };
                let (Ok(name), Some(value)) = (CStr::from_ptr(e.name).to_str(), value) else {
                    continue;
                };
                out.set_propagated(name, value, e.propagate != 0);
            }
        }
        if !c.binary_entries.is_null() {
            for e in slice::from_raw_parts(c.binary_entries, c.binary_entries_len) {
                if e.name.is_null() {
                    continue;
                }
                let Ok(name) = CStr::from_ptr(e.name).to_str() else {
                    continue;
                };
                out.set_binary(
                    name,
                    slice_or_empty(e.value, e.value_len).to_vec(),
                    e.propagate != 0,
                );
            }
        }
    }
    out
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn data_tags_from_c(c: &ZeroDdsDataTagQosPolicy) -> DataTagQosPolicy {
    let mut out = DataTagQosPolicy::new();
    if c.tags.is_null() {
        return out;
    }
    // SAFETY: caller contract: tags[0..tags_len] valid, every pointer inside valid.
    unsafe {
        for t in slice::from_raw_parts(c.tags, c.tags_len) {
            if t.name.is_null() {
                continue;
            }
            let value = if t.value.is_null() {
                Some("")
            } else {
                CStr::from_ptr(t.value).to_str().ok()
            };
            let (Ok(name), Some(value)) = (CStr::from_ptr(t.name).to_str(), value) else {
                continue;
            };
            out = out.with(name, value);
        }
    }
    out
}

/// Writes the `property` and `data_tags` OUT fields of a writer or reader QoS
/// from `cache` (see [`PartitionOutCache`]); the entries stay valid until the
/// next `get_qos` on the same entity.
///
/// # Safety
/// `property`/`tags` valid for writes.
unsafe fn marshal_property_and_tags_out(
    cache: &std::sync::Mutex<PartitionOutCache>,
    r_property: &PropertyQosPolicy,
    r_tags: &DataTagQosPolicy,
    property: *mut ZeroDdsPropertyQosPolicy,
    tags: *mut ZeroDdsDataTagQosPolicy,
) {
    // SAFETY: caller pledge — out pointers valid for writes.
    unsafe {
        if let Ok(mut c) = cache.lock() {
            c.marshal_property(r_property, &mut *property);
            c.marshal_tags(r_tags, &mut *tags);
        } else {
            *property = ZeroDdsPropertyQosPolicy {
                entries: ptr::null(),
                entries_len: 0,
                binary_entries: ptr::null(),
                binary_entries_len: 0,
            };
            *tags = ZeroDdsDataTagQosPolicy {
                tags: ptr::null(),
                tags_len: 0,
            };
        }
    }
}

// QoS-Set Konvertierungen
// ---------------------------------------------------------------------------

/// Converts an FFI pointer (NULL = default) into a `DataWriterQos`.
///
/// # Safety
/// `c` may be NULL. If not NULL, it must point to a valid
/// `ZeroDdsDataWriterQos` struct.
pub unsafe fn dw_qos_from_c(c: *const ZeroDdsDataWriterQos) -> DataWriterQos {
    if c.is_null() {
        return DataWriterQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; all helper calls
    // (cstr_vec, slice_or_empty) inherit the caller pledge for (ptr,len) fields.
    unsafe {
        let q = &*c;
        DataWriterQos {
            reliability: q.reliability.into(),
            durability: q.durability.into(),
            durability_service: q.durability_service.into(),
            deadline: q.deadline.into(),
            latency_budget: q.latency_budget.into(),
            liveliness: q.liveliness.into(),
            destination_order: q.destination_order.into(),
            lifespan: q.lifespan.into(),
            ownership: q.ownership.into(),
            ownership_strength: q.ownership_strength.into(),
            partition: PartitionQosPolicy {
                names: cstr_vec(q.partition.names, q.partition.names_len),
            },
            presentation: q.presentation.into(),
            history: q.history.into(),
            resource_limits: q.resource_limits.into(),
            transport_priority: q.transport_priority.into(),
            writer_data_lifecycle: q.writer_data_lifecycle.into(),
            user_data: UserDataQosPolicy {
                value: slice_or_empty(q.user_data.value, q.user_data.value_len).to_vec(),
            },
            topic_data: TopicDataQosPolicy {
                value: slice_or_empty(q.topic_data.value, q.topic_data.value_len).to_vec(),
            },
            group_data: GroupDataQosPolicy {
                value: slice_or_empty(q.group_data.value, q.group_data.value_len).to_vec(),
            },
            data_representation: q.data_representation.to_qos(),
            property: property_from_c(&q.property),
            data_tags: data_tags_from_c(&q.data_tags),
        }
    }
}

/// Converts an FFI pointer into`DataReaderQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dr_qos_from_c(c: *const ZeroDdsDataReaderQos) -> DataReaderQos {
    if c.is_null() {
        return DataReaderQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; helper calls inherit the pledge.
    unsafe {
        let q = &*c;
        DataReaderQos {
            reliability: q.reliability.into(),
            durability: q.durability.into(),
            deadline: q.deadline.into(),
            latency_budget: q.latency_budget.into(),
            liveliness: q.liveliness.into(),
            destination_order: q.destination_order.into(),
            ownership: q.ownership.into(),
            partition: PartitionQosPolicy {
                names: cstr_vec(q.partition.names, q.partition.names_len),
            },
            presentation: q.presentation.into(),
            history: q.history.into(),
            resource_limits: q.resource_limits.into(),
            time_based_filter: q.time_based_filter.into(),
            reader_data_lifecycle: q.reader_data_lifecycle.into(),
            user_data: UserDataQosPolicy {
                value: slice_or_empty(q.user_data.value, q.user_data.value_len).to_vec(),
            },
            topic_data: TopicDataQosPolicy {
                value: slice_or_empty(q.topic_data.value, q.topic_data.value_len).to_vec(),
            },
            group_data: GroupDataQosPolicy {
                value: slice_or_empty(q.group_data.value, q.group_data.value_len).to_vec(),
            },
            data_representation: q.data_representation.to_qos(),
            property: property_from_c(&q.property),
            data_tags: data_tags_from_c(&q.data_tags),
            // A `kind` Annex D does not define is refused by
            // `dr_qos_from_c_checked`, which every entry point that takes a
            // QoS from the caller uses; here it reads as the default.
            type_consistency: q.type_consistency.to_qos().unwrap_or_default(),
        }
    }
}

/// As [`dr_qos_from_c`], refusing a QoS whose `type_consistency.kind` is not
/// one of the two values of Annex D.
///
/// # Errors
/// `BadParameter` for an undefined `TypeConsistencyKind`.
///
/// # Safety
/// As [`dr_qos_from_c`].
pub unsafe fn dr_qos_from_c_checked(
    c: *const ZeroDdsDataReaderQos,
) -> Result<DataReaderQos, ZeroDdsStatus> {
    if !c.is_null() {
        // SAFETY: `c` is non-NULL and valid per the caller's contract.
        unsafe { (*c).type_consistency.to_qos() }?;
    }
    // SAFETY: the caller's contract.
    Ok(unsafe { dr_qos_from_c(c) })
}

/// Converts an FFI pointer into`TopicQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn topic_qos_from_c(c: *const ZeroDdsTopicQos) -> TopicQos {
    if c.is_null() {
        return TopicQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; Helper vererbt Pledge.
    unsafe {
        let q = &*c;
        TopicQos {
            durability: q.durability.into(),
            durability_service: q.durability_service.into(),
            deadline: q.deadline.into(),
            latency_budget: q.latency_budget.into(),
            liveliness: q.liveliness.into(),
            reliability: q.reliability.into(),
            destination_order: q.destination_order.into(),
            history: q.history.into(),
            resource_limits: q.resource_limits.into(),
            transport_priority: q.transport_priority.into(),
            lifespan: q.lifespan.into(),
            ownership: q.ownership.into(),
            topic_data: TopicDataQosPolicy {
                value: slice_or_empty(q.topic_data.value, q.topic_data.value_len).to_vec(),
            },
            data_representation: q.data_representation.to_qos(),
        }
    }
}

/// Converts an FFI pointer into`PublisherQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn pub_qos_from_c(c: *const ZeroDdsPublisherQos) -> PublisherQos {
    if c.is_null() {
        return PublisherQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; Helper vererbt Pledge.
    unsafe {
        let q = &*c;
        PublisherQos {
            presentation: q.presentation.into(),
            partition: PartitionQosPolicy {
                names: cstr_vec(q.partition.names, q.partition.names_len),
            },
            group_data: GroupDataQosPolicy {
                value: slice_or_empty(q.group_data.value, q.group_data.value_len).to_vec(),
            },
            entity_factory: q.entity_factory.into(),
        }
    }
}

/// Converts an FFI pointer into`SubscriberQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn sub_qos_from_c(c: *const ZeroDdsSubscriberQos) -> SubscriberQos {
    if c.is_null() {
        return SubscriberQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; Helper vererbt Pledge.
    unsafe {
        let q = &*c;
        SubscriberQos {
            presentation: q.presentation.into(),
            partition: PartitionQosPolicy {
                names: cstr_vec(q.partition.names, q.partition.names_len),
            },
            group_data: GroupDataQosPolicy {
                value: slice_or_empty(q.group_data.value, q.group_data.value_len).to_vec(),
            },
            entity_factory: q.entity_factory.into(),
        }
    }
}

/// Converts an FFI pointer into`DomainParticipantQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dp_qos_from_c(c: *const ZeroDdsDomainParticipantQos) -> DomainParticipantQos {
    if c.is_null() {
        return DomainParticipantQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above; Helper vererbt Pledge.
    unsafe {
        let q = &*c;
        DomainParticipantQos {
            user_data: UserDataQosPolicy {
                value: slice_or_empty(q.user_data.value, q.user_data.value_len).to_vec(),
            },
            entity_factory: q.entity_factory.into(),
            ..DomainParticipantQos::default()
        }
    }
}

/// Converts an FFI pointer into`DomainParticipantFactoryQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dpf_qos_from_c(
    c: *const ZeroDdsDomainParticipantFactoryQos,
) -> DomainParticipantFactoryQos {
    if c.is_null() {
        return DomainParticipantFactoryQos::default();
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above.
    let q = unsafe { &*c };
    DomainParticipantFactoryQos {
        autoenable_created_entities: q.entity_factory.autoenable_created_entities,
    }
}

/// Writes a `DomainParticipantQos` snapshot into the caller buffer.
/// Variable-length fields (UserData) are copied into a caller-
/// allocated buffer if `out.user_data.value` is non-NULL
/// and `out.user_data.value_len` is large enough. On a too-small
/// buffer `value_len` is written with the required size
/// and `OUT_OF_RESOURCES` is returned.
///
/// # Safety
/// `out` valid.
pub unsafe fn dp_qos_to_c(
    r: &DomainParticipantQos,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the user_data.value buffer
    // has at least user_data.value_len bytes of capacity (caller pledge).
    unsafe {
        let needed = r.user_data.value.len();
        let cap = (*out).user_data.value_len;
        let dst_ptr = (*out).user_data.value as *mut u8;
        if needed > 0 {
            if dst_ptr.is_null() || cap < needed {
                (*out).user_data.value_len = needed;
                return ZeroDdsStatus::OutOfResources as c_int;
            }
            ptr::copy_nonoverlapping(r.user_data.value.as_ptr(), dst_ptr, needed);
        }
        (*out).user_data.value_len = needed;
        (*out).entity_factory = r.entity_factory.into();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a `TopicQos` snapshot into the caller buffer.
/// Variable-length fields (TopicData) as in `dp_qos_to_c`.
///
/// # Safety
/// `out` valid.
pub unsafe fn topic_qos_to_c(r: &TopicQos, out: *mut ZeroDdsTopicQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the topic_data.value buffer
    // has at least topic_data.value_len bytes of capacity (caller pledge).
    unsafe {
        let needed = r.topic_data.value.len();
        let cap = (*out).topic_data.value_len;
        let dst_ptr = (*out).topic_data.value as *mut u8;
        if needed > 0 {
            if dst_ptr.is_null() || cap < needed {
                (*out).topic_data.value_len = needed;
                return ZeroDdsStatus::OutOfResources as c_int;
            }
            ptr::copy_nonoverlapping(r.topic_data.value.as_ptr(), dst_ptr, needed);
        }
        (*out).topic_data.value_len = needed;
        (*out).durability = r.durability.into();
        (*out).durability_service = r.durability_service.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).reliability = r.reliability.into();
        (*out).destination_order = r.destination_order.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).transport_priority = r.transport_priority.into();
        (*out).lifespan = r.lifespan.into();
        (*out).ownership = r.ownership.into();
        match ZeroDdsDataRepresentationQosPolicy::from_qos(r.data_representation.as_deref()) {
            Ok(policy) => (*out).data_representation = policy,
            Err(status) => return status as c_int,
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a `DataWriterQos` snapshot. UserData/TopicData/GroupData
/// must have sufficient buffers. PARTITION names are round-tripped out via the
/// entity-owned `partition_cache` (see [`PartitionOutCache`]): the returned
/// `partition.names`/`names_len` reflect the QoS that was set (DDS 1.4
/// §2.2.3.10), pointing at implementation-owned C strings valid until the next
/// `get_qos` on the same entity.
///
/// # Safety
/// `out` valid.
pub unsafe fn dw_qos_to_c(
    r: &DataWriterQos,
    out: *mut ZeroDdsDataWriterQos,
    partition_cache: &std::sync::Mutex<PartitionOutCache>,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the UserData/TopicData/GroupData
    // value buffers have at least value_len bytes of capacity (caller pledge).
    unsafe {
        // Inline macro: non-NULL check + cap check + ptr::copy + final length write
        macro_rules! copy_bytes {
            ($field:ident) => {{
                let needed = r.$field.value.len();
                let cap = (*out).$field.value_len;
                let dst = (*out).$field.value as *mut u8;
                if needed > 0 {
                    if dst.is_null() || cap < needed {
                        (*out).$field.value_len = needed;
                        return ZeroDdsStatus::OutOfResources as c_int;
                    }
                    ptr::copy_nonoverlapping(r.$field.value.as_ptr(), dst, needed);
                }
                (*out).$field.value_len = needed;
            }};
        }
        copy_bytes!(user_data);
        copy_bytes!(topic_data);
        copy_bytes!(group_data);
        (*out).reliability = r.reliability.into();
        (*out).durability = r.durability.into();
        (*out).durability_service = r.durability_service.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).destination_order = r.destination_order.into();
        (*out).lifespan = r.lifespan.into();
        (*out).ownership = r.ownership.into();
        (*out).ownership_strength = r.ownership_strength.into();
        (*out).presentation = r.presentation.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).transport_priority = r.transport_priority.into();
        (*out).writer_data_lifecycle = r.writer_data_lifecycle.into();
        match ZeroDdsDataRepresentationQosPolicy::from_qos(r.data_representation.as_deref()) {
            Ok(policy) => (*out).data_representation = policy,
            Err(status) => return status as c_int,
        }
        // PARTITION round-trips out via the entity-owned cache (DDS 1.4 §2.2.3.10).
        marshal_partition_out(
            partition_cache,
            &r.partition.names,
            &mut (*out).partition.names,
            &mut (*out).partition.names_len,
        );
        marshal_property_and_tags_out(
            partition_cache,
            &r.property,
            &r.data_tags,
            &mut (*out).property,
            &mut (*out).data_tags,
        );
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a `DataReaderQos` snapshot.
///
/// # Safety
/// `out` valid.
pub unsafe fn dr_qos_to_c(
    r: &DataReaderQos,
    out: *mut ZeroDdsDataReaderQos,
    partition_cache: &std::sync::Mutex<PartitionOutCache>,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the UserData/TopicData/GroupData
    // value buffers have at least value_len bytes of capacity (caller pledge).
    unsafe {
        macro_rules! copy_bytes {
            ($field:ident) => {{
                let needed = r.$field.value.len();
                let cap = (*out).$field.value_len;
                let dst = (*out).$field.value as *mut u8;
                if needed > 0 {
                    if dst.is_null() || cap < needed {
                        (*out).$field.value_len = needed;
                        return ZeroDdsStatus::OutOfResources as c_int;
                    }
                    ptr::copy_nonoverlapping(r.$field.value.as_ptr(), dst, needed);
                }
                (*out).$field.value_len = needed;
            }};
        }
        copy_bytes!(user_data);
        copy_bytes!(topic_data);
        copy_bytes!(group_data);
        (*out).reliability = r.reliability.into();
        (*out).durability = r.durability.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).destination_order = r.destination_order.into();
        (*out).ownership = r.ownership.into();
        (*out).presentation = r.presentation.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).time_based_filter = r.time_based_filter.into();
        (*out).reader_data_lifecycle = r.reader_data_lifecycle.into();
        match ZeroDdsDataRepresentationQosPolicy::from_qos(r.data_representation.as_deref()) {
            Ok(policy) => (*out).data_representation = policy,
            Err(status) => return status as c_int,
        }
        (*out).type_consistency =
            ZeroDdsTypeConsistencyEnforcementQosPolicy::from_qos(&r.type_consistency);
        // PARTITION round-trips out via the entity-owned cache (DDS 1.4 §2.2.3.10).
        marshal_partition_out(
            partition_cache,
            &r.partition.names,
            &mut (*out).partition.names,
            &mut (*out).partition.names_len,
        );
        marshal_property_and_tags_out(
            partition_cache,
            &r.property,
            &r.data_tags,
            &mut (*out).property,
            &mut (*out).data_tags,
        );
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a `PublisherQos` snapshot.
///
/// # Safety
/// `out` valid.
pub unsafe fn pub_qos_to_c(
    r: &PublisherQos,
    out: *mut ZeroDdsPublisherQos,
    partition_cache: &std::sync::Mutex<PartitionOutCache>,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the group_data.value buffer
    // has at least group_data.value_len bytes of capacity (caller pledge).
    unsafe {
        let needed = r.group_data.value.len();
        let cap = (*out).group_data.value_len;
        let dst = (*out).group_data.value as *mut u8;
        if needed > 0 {
            if dst.is_null() || cap < needed {
                (*out).group_data.value_len = needed;
                return ZeroDdsStatus::OutOfResources as c_int;
            }
            ptr::copy_nonoverlapping(r.group_data.value.as_ptr(), dst, needed);
        }
        (*out).group_data.value_len = needed;
        (*out).presentation = r.presentation.into();
        (*out).entity_factory = r.entity_factory.into();
        // PARTITION round-trips out via the entity-owned cache (DDS 1.4 §2.2.3.10).
        marshal_partition_out(
            partition_cache,
            &r.partition.names,
            &mut (*out).partition.names,
            &mut (*out).partition.names_len,
        );
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a `SubscriberQos` snapshot. Identical layout to
/// PublisherQos.
///
/// # Safety
/// `out` valid.
pub unsafe fn sub_qos_to_c(
    r: &SubscriberQos,
    out: *mut ZeroDdsSubscriberQos,
    partition_cache: &std::sync::Mutex<PartitionOutCache>,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out NULL-checked above; the group_data.value buffer
    // has at least group_data.value_len bytes of capacity (caller pledge).
    unsafe {
        let needed = r.group_data.value.len();
        let cap = (*out).group_data.value_len;
        let dst = (*out).group_data.value as *mut u8;
        if needed > 0 {
            if dst.is_null() || cap < needed {
                (*out).group_data.value_len = needed;
                return ZeroDdsStatus::OutOfResources as c_int;
            }
            ptr::copy_nonoverlapping(r.group_data.value.as_ptr(), dst, needed);
        }
        (*out).group_data.value_len = needed;
        (*out).presentation = r.presentation.into();
        (*out).entity_factory = r.entity_factory.into();
        // PARTITION round-trips out via the entity-owned cache (DDS 1.4 §2.2.3.10).
        marshal_partition_out(
            partition_cache,
            &r.partition.names,
            &mut (*out).partition.names,
            &mut (*out).partition.names_len,
        );
    }
    ZeroDdsStatus::Ok as c_int
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn duration_roundtrip() {
        let r = Duration::from_millis(250);
        let c: ZeroDdsDuration = r.into();
        let back: Duration = c.into();
        // Conversion via nanosec may have minimal drift —
        // check only seconds + coarse nanosec.
        assert_eq!(back.seconds, r.seconds);
        assert!((back.fraction as i64 - r.fraction as i64).abs() < 1024);
    }

    #[test]
    fn duration_infinite_roundtrip() {
        let r = Duration::INFINITE;
        let c: ZeroDdsDuration = r.into();
        assert_eq!(c.sec, i32::MAX);
        assert_eq!(c.nanosec, u32::MAX);
        let back: Duration = c.into();
        assert!(back.is_infinite());
    }

    #[test]
    fn dw_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(ptr::null()) };
        assert_eq!(q, DataWriterQos::default());
    }

    #[test]
    fn dw_qos_reliability_roundtrip() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let mut c: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        c.reliability = ZeroDdsReliabilityQosPolicy {
            kind: 1, // BestEffort
            max_blocking_time: ZeroDdsDuration { sec: 1, nanosec: 0 },
        };
        c.history.kind = 0;
        c.history.depth = 1;
        c.resource_limits.max_samples = 1000;
        c.resource_limits.max_instances = 10;
        c.resource_limits.max_samples_per_instance = 100;
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(&c) };
        assert!(matches!(q.reliability.kind, ReliabilityKind::BestEffort));
    }

    /// DDS-Security 1.2 §7.3.21: `property` (with `propagate` per entry and
    /// binary properties) and `data_tags` of a writer and a reader QoS cross
    /// the C ABI in both directions.
    #[test]
    fn property_and_data_tags_cross_the_c_abi_both_ways() {
        let name_a = c"dds.sec.crypto.symmetric_cipher_algorithm";
        let value_a = c"AES256+GCM";
        let name_b = c"local.note";
        let value_b = c"x";
        let name_bin = c"blob";
        let blob = [1u8, 2, 3];
        let entries = [
            ZeroDdsPropertyEntry {
                name: name_a.as_ptr(),
                value: value_a.as_ptr(),
                propagate: 1,
            },
            ZeroDdsPropertyEntry {
                name: name_b.as_ptr(),
                value: value_b.as_ptr(),
                propagate: 0,
            },
        ];
        let binary = [ZeroDdsBinaryPropertyEntry {
            name: name_bin.as_ptr(),
            value: blob.as_ptr(),
            value_len: blob.len(),
            propagate: 1,
        }];
        let tag_name = c"classification";
        let tag_value = c"secret";
        let tags = [ZeroDdsTag {
            name: tag_name.as_ptr(),
            value: tag_value.as_ptr(),
        }];
        let property = ZeroDdsPropertyQosPolicy {
            entries: entries.as_ptr(),
            entries_len: entries.len(),
            binary_entries: binary.as_ptr(),
            binary_entries_len: binary.len(),
        };
        let data_tags = ZeroDdsDataTagQosPolicy {
            tags: tags.as_ptr(),
            tags_len: tags.len(),
        };
        let expected_property = PropertyQosPolicy::new()
            .with_propagated("dds.sec.crypto.symmetric_cipher_algorithm", "AES256+GCM")
            .with("local.note", "x");
        let mut expected_property = expected_property;
        expected_property.set_binary("blob", vec![1, 2, 3], true);
        let expected_tags = DataTagQosPolicy::new().with("classification", "secret");

        // Writer: C to Rust.
        // SAFETY: FFI-boundary; every pointer in the structs lives in this test.
        let mut cw: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        cw.property = property;
        cw.data_tags = data_tags;
        // SAFETY: as above.
        let w = unsafe { dw_qos_from_c(&cw) };
        assert_eq!(w.property, expected_property);
        assert_eq!(w.data_tags, expected_tags);
        // Writer: Rust to C, through the cache of the entity.
        let cache = std::sync::Mutex::new(PartitionOutCache::default());
        // SAFETY: as above.
        let mut out: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        // SAFETY: out is valid; no user data buffers are needed (all empty).
        let status = unsafe { dw_qos_to_c(&w, &mut out, &cache) };
        assert_eq!(status, ZeroDdsStatus::Ok as c_int);
        // SAFETY: the cache owns what out points at, and lives on.
        let back = unsafe { dw_qos_from_c(&out) };
        assert_eq!(back.property, expected_property);
        assert_eq!(back.data_tags, expected_tags);

        // Reader.
        // SAFETY: as above.
        let mut cr: ZeroDdsDataReaderQos = unsafe { core::mem::zeroed() };
        cr.property = property;
        cr.data_tags = data_tags;
        // SAFETY: as above.
        let r = unsafe { dr_qos_from_c(&cr) };
        assert_eq!(r.property, expected_property);
        assert_eq!(r.data_tags, expected_tags);
        let cache = std::sync::Mutex::new(PartitionOutCache::default());
        // SAFETY: as above.
        let mut out: ZeroDdsDataReaderQos = unsafe { core::mem::zeroed() };
        // SAFETY: as above.
        let status = unsafe { dr_qos_to_c(&r, &mut out, &cache) };
        assert_eq!(status, ZeroDdsStatus::Ok as c_int);
        // SAFETY: as above.
        let back = unsafe { dr_qos_from_c(&out) };
        assert_eq!(back.property, expected_property);
        assert_eq!(back.data_tags, expected_tags);
    }

    /// An all-zero struct, as every caller before this member passed it,
    /// has no property and no tags.
    #[test]
    fn an_all_zero_property_and_data_tags_are_empty() {
        // SAFETY: all-zero is a valid value of the struct.
        let c: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        // SAFETY: as above.
        let w = unsafe { dw_qos_from_c(&c) };
        assert!(w.property.is_empty());
        assert!(w.data_tags.is_empty());
    }

    #[test]
    fn dr_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dr_qos_from_c(ptr::null()) };
        assert_eq!(q, DataReaderQos::default());
    }

    #[test]
    fn topic_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { topic_qos_from_c(ptr::null()) };
        assert_eq!(q, TopicQos::default());
    }

    #[test]
    fn pub_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { pub_qos_from_c(ptr::null()) };
        assert_eq!(q, PublisherQos::default());
    }

    #[test]
    fn sub_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { sub_qos_from_c(ptr::null()) };
        assert_eq!(q, SubscriberQos::default());
    }

    #[test]
    fn dp_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dp_qos_from_c(ptr::null()) };
        assert_eq!(q, DomainParticipantQos::default());
    }

    #[test]
    fn dp_qos_userdata_passthrough() {
        let bytes = b"hello world";
        let c = ZeroDdsDomainParticipantQos {
            user_data: ZeroDdsUserDataQosPolicy {
                value: bytes.as_ptr(),
                value_len: bytes.len(),
            },
            entity_factory: ZeroDdsEntityFactoryQosPolicy {
                autoenable_created_entities: false,
            },
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dp_qos_from_c(&c) };
        assert_eq!(q.user_data.value, bytes.to_vec());
        assert!(!q.entity_factory.autoenable_created_entities);
    }

    #[test]
    fn partition_cstring_array_passthrough() {
        let p1 = c"alpha";
        let p2 = c"beta";
        let arr: [*const c_char; 2] = [p1.as_ptr(), p2.as_ptr()];
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let mut c: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        c.partition.names = arr.as_ptr();
        c.partition.names_len = 2;
        // Reliability/History/ResourceLimits are via zeroed() = 0
        // (BestEffort=1 is not 0, but ReliabilityKind::from_u32 maps
        // 0 to BestEffort as a fallback). Check only Partition.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(&c) };
        assert_eq!(
            q.partition.names,
            alloc::vec!["alpha".to_string(), "beta".to_string()]
        );
    }
}
