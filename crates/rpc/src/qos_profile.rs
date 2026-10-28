// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RpcQos` — QoS profile resolution for DDS-RPC (Spec §7.10).
//!
//! The DDS-RPC default QoS for the endpoints of a requester or replier
//! (OMG `formal/2017-04-01` §7.10.2) is:
//!
//! * **Reliability=Reliable** — both writer and reader. Rationale: a lost
//!   reply is semantically indistinguishable from a never-executed request.
//! * **History=KeepAll** (Spec §7.10.2) — every outstanding request or reply
//!   is kept, so a slow reader cannot lose an older one. Users can override
//!   the value via a profile.
//! * **Durability=Volatile** (Spec §7.10.2) — no history replay to
//!   late-joining peers.
//! * **ResourceLimits**: §7.10.2 names none, so the DDS default holds:
//!   `LENGTH_UNLIMITED` for all three. [`RpcQos::bounded`] is the named
//!   setting with `max_samples=512`, `max_instances=1`,
//!   `max_samples_per_instance=512` that earlier versions used by default.
//! * **Lifespan/Deadline/Latency-Budget**: the spec leaves them as
//!   `INFINITE`/`UNSET` — user-overridable.
//!
//! The module provides two builder defaults:
//!
//! * [`RpcQos::default_basic`] — the default QoS of §7.10.2.
//! * [`RpcQos::default_enhanced`] — like basic; the Enhanced mapping uses
//!   the same default QoS.
//!
//! Plus a [`RpcQos::from_xml_profile`] method that resolves a profile under
//! `library::profile` via the XML loader (`zerodds-xml::DdsXml`)
//! and materializes it onto `RpcQos`, following the profile's `base_name`
//! inheritance. Policies not specified in the XML stay at the spec default
//! of the matching foundation mode; a profile that sets something `RpcQos`
//! cannot carry is refused with `RpcError::UnmodelledQos`, unless
//! [`XmlProfileOptions::ignore_unmodelled`] says to read what it can.

extern crate alloc;

use alloc::string::ToString;

use zerodds_dcps::qos::{DataReaderQos, DataWriterQos};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, DurabilityQosPolicy, Duration, HistoryKind,
    HistoryQosPolicy, LifespanQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy,
};
use zerodds_xml::{DdsXml, EntityQos, QosProfile, ResolvedQos};

use crate::error::{RpcError, RpcResult};

/// Spec-default history for the endpoints of §7.10.2: `KEEP_ALL`, so no
/// outstanding request or reply is replaced by a newer one.
pub const DEFAULT_HISTORY_QOS: HistoryQosPolicy = HistoryQosPolicy {
    kind: HistoryKind::KeepAll,
    depth: 0,
};

/// Resource limits of the endpoints by default: §7.10.2 names none, so the DDS default
/// (`LENGTH_UNLIMITED`, DDS 1.4 §2.2.3.19) holds.
pub const DEFAULT_RESOURCE_LIMITS: ResourceLimitsQosPolicy = ResourceLimitsQosPolicy {
    max_samples: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
    max_instances: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
    max_samples_per_instance: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
};

/// The bounded limits for single-instance RPC endpoints: a setting, not the default.
pub const BOUNDED_RESOURCE_LIMITS: ResourceLimitsQosPolicy = ResourceLimitsQosPolicy {
    max_samples: 512,
    max_instances: 1,
    max_samples_per_instance: 512,
};

// ---------------------------------------------------------------------
// RpcQos
// ---------------------------------------------------------------------

/// Effective QoS configuration of a requester/replier (Spec §7.10).
///
/// Fields are public — users can patch them directly after the builder.
/// Validation happens when materializing in
/// [`Self::request_writer_qos`] etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcQos {
    /// Reliability for both endpoints. Spec §7.10.2: Reliable.
    pub reliability: ReliabilityQosPolicy,
    /// Durability for both endpoints. Spec §7.10.2: Volatile (no
    /// history replay to late-joining peers).
    pub durability: DurabilityQosPolicy,
    /// History for the request writer / request reader.
    pub request_history: HistoryQosPolicy,
    /// History for the reply writer / reply reader.
    pub reply_history: HistoryQosPolicy,
    /// Resource limits — the same for writer and reader.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// Lifespan policy. Default INFINITE.
    pub lifespan: LifespanQosPolicy,
    /// Deadline policy. Default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// Default timeout for `Requester::send_request_blocking`. Default
    /// 5 seconds.
    pub request_timeout: core::time::Duration,
    /// The QoS of the reply endpoints when `@Qos(ReplyProfile = ...)`
    /// (§7.10.1) names a profile of their own: the reply writer and the
    /// reply reader are built from it instead of from the fields above,
    /// which then apply to the request endpoints alone. `None`: both
    /// directions share the fields above.
    pub reply_profile: Option<alloc::boxed::Box<RpcQos>>,
}

impl RpcQos {
    /// Spec default (Spec §7.10.2). Reliable + KeepAll + Volatile +
    /// 5 s timeout.
    #[must_use]
    pub fn default_basic() -> Self {
        Self {
            reliability: ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(100),
            },
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            },
            request_history: DEFAULT_HISTORY_QOS,
            reply_history: DEFAULT_HISTORY_QOS,
            resource_limits: DEFAULT_RESOURCE_LIMITS,
            lifespan: LifespanQosPolicy::default(),
            deadline: DeadlineQosPolicy::default(),
            request_timeout: core::time::Duration::from_secs(5),
            reply_profile: None,
        }
    }

    /// This QoS with [`BOUNDED_RESOURCE_LIMITS`]: at most 512 outstanding samples per
    /// endpoint. A named setting; the default is unbounded (§7.10.2 names no limit).
    #[must_use]
    pub fn bounded(mut self) -> Self {
        self.resource_limits = BOUNDED_RESOURCE_LIMITS;
        self
    }

    /// Enhanced mode. The Enhanced mapping uses the same default QoS as
    /// the Basic mapping (§7.10.2).
    #[must_use]
    pub fn default_enhanced() -> Self {
        Self::default_basic()
    }

    /// Materializes the writer QoS for the **request topic** (sender:
    /// requester, receiver: replier).
    #[must_use]
    pub fn request_writer_qos(&self) -> DataWriterQos {
        DataWriterQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.request_history,
            resource_limits: self.resource_limits,
            lifespan: self.lifespan,
            deadline: self.deadline,
            ..DataWriterQos::default()
        }
    }

    /// Materializes the reader QoS for the **request topic** (receiver:
    /// replier).
    #[must_use]
    pub fn request_reader_qos(&self) -> DataReaderQos {
        DataReaderQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.request_history,
            resource_limits: self.resource_limits,
            deadline: self.deadline,
            ..DataReaderQos::default()
        }
    }

    /// Materializes the writer QoS for the **reply topic** (sender:
    /// replier).
    #[must_use]
    pub fn reply_writer_qos(&self) -> DataWriterQos {
        if let Some(reply) = self.reply_profile.as_deref() {
            return reply.reply_writer_qos();
        }
        DataWriterQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.reply_history,
            resource_limits: self.resource_limits,
            lifespan: self.lifespan,
            deadline: self.deadline,
            ..DataWriterQos::default()
        }
    }

    /// Materializes the reader QoS for the **reply topic** (receiver:
    /// requester).
    #[must_use]
    pub fn reply_reader_qos(&self) -> DataReaderQos {
        if let Some(reply) = self.reply_profile.as_deref() {
            return reply.reply_reader_qos();
        }
        DataReaderQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.reply_history,
            resource_limits: self.resource_limits,
            deadline: self.deadline,
            ..DataReaderQos::default()
        }
    }

    /// Resolves a QoS profile from a [`DdsXml`] and merges
    /// its policies over [`Self::default_basic`].
    ///
    /// `path` format: `library::profile` — e.g. `"RpcLib::Calculator"`.
    /// Profiles can set policies via either the `<datawriter_qos>` /
    /// `<datareader_qos>` containers — both are
    /// merged (the reader container overrides the writer for policies that
    /// are not pure writer-only policies).
    ///
    /// Policies not set in the profile stay at
    /// [`Self::default_basic`].
    ///
    /// # Errors
    /// * `RpcError::QosProfileNotFound` if `library` or `profile`
    ///   cannot be resolved or `path` does not have the
    ///   `library::profile` format.
    pub fn from_xml_profile(loader: &DdsXml, path: &str) -> RpcResult<Self> {
        Self::from_xml_profile_with(loader, path, XmlProfileOptions::default())
    }

    /// [`Self::from_xml_profile`] with the settings of `options`.
    ///
    /// The profile is resolved with its `base_name` inheritance chain
    /// (DDS-XML §7.3.2.4.2): a derived profile overrides the policies of
    /// its base where it sets them. The `<datawriter_qos>` and
    /// `<datareader_qos>` containers are read; the policies `RpcQos` models
    /// are reliability, durability, history, resource limits, lifespan and
    /// deadline.
    ///
    /// Anything else the profile sets is refused with
    /// `RpcError::UnmodelledQos` naming the container and the policy, since
    /// ignoring it would run the endpoints on a QoS the profile did not ask
    /// for. That covers the other policies, the containers of entities the
    /// RPC layer creates with their defaults (`topic_qos`, `publisher_qos`,
    /// `subscriber_qos`, `domainparticipant_qos`), `lifespan` in a reader
    /// container (it is a DataWriter policy, §2.2.3.16), and a policy the
    /// two containers set to different values, which one `RpcQos` cannot
    /// say. [`XmlProfileOptions::ignore_unmodelled`] reads what it can
    /// instead: the reader container wins a disagreement and the rest is
    /// left out.
    ///
    /// # Errors
    /// * `RpcError::QosProfileNotFound` as for [`Self::from_xml_profile`],
    ///   and for a missing base in the inheritance chain.
    /// * `RpcError::QosProfileInvalid` for a profile the shared resolver
    ///   refuses (a cycle in the inheritance chain, a depth past the limit).
    /// * `RpcError::UnmodelledQos` as above.
    pub fn from_xml_profile_with(
        loader: &DdsXml,
        path: &str,
        options: XmlProfileOptions,
    ) -> RpcResult<Self> {
        // Keep the path-shape check so a malformed path keeps its error.
        let _ = split_qos_path(path)?;
        // One resolver for the `base_name` chain (DDS-XML §7.3.2.4.2),
        // the shared one of `zerodds_xml`.
        let resolved = zerodds_xml::resolve_profile(&loader.qos_libraries, path)
            .map_err(|e| qos_resolve_error(path, e))?;
        Self::from_resolved(Self::default_basic(), &resolved, options)
    }

    fn from_resolved(
        mut base: Self,
        resolved: &ResolvedQos,
        options: XmlProfileOptions,
    ) -> RpcResult<Self> {
        let selected = ResolvedProfile::select(resolved);
        if !options.ignore_unmodelled {
            selected.check()?;
        }
        apply_entity_qos_to_writer_side(&mut base, &selected.writer);
        apply_entity_qos_to_reader_side(&mut base, &selected.reader);
        Ok(base)
    }

    /// Like [`Self::from_xml_profile`], but with an explicit default
    /// (e.g. `default_enhanced`). The profile is taken as written, without
    /// its `base_name` chain.
    #[must_use]
    pub fn from_profile_default(mut base: Self, prof: &QosProfile) -> Self {
        // RPC endpoints are not bound to one topic name, so the filter
        // selection of §7.3.2.4.3 falls back: the filterless container,
        // else the first one in document order.
        if let Some(eq) = zerodds_xml::default_qos(&prof.datawriter_qos) {
            apply_entity_qos_to_writer_side(&mut base, eq);
        }
        if let Some(eq) = zerodds_xml::default_qos(&prof.datareader_qos) {
            apply_entity_qos_to_reader_side(&mut base, eq);
        }
        base
    }

    /// Like [`Self::from_profile_default`], but over an already-resolved
    /// profile (with its `base_name` chain merged, §7.3.2.4.2).
    #[must_use]
    pub fn from_resolved_default(mut base: Self, prof: &ResolvedQos) -> Self {
        // Same fallback as `from_profile_default`: no topic name at hand.
        if let Some(eq) = zerodds_xml::default_qos(&prof.datawriter_qos) {
            apply_entity_qos_to_writer_side(&mut base, eq);
        }
        if let Some(eq) = zerodds_xml::default_qos(&prof.datareader_qos) {
            apply_entity_qos_to_reader_side(&mut base, eq);
        }
        base
    }

    /// The QoS `@Qos(RequestProfile = request_url, ReplyProfile =
    /// reply_url)` declares (Spec §7.10.1): the request endpoints from
    /// the first profile, the reply endpoints from the second, or from the
    /// first where `reply_url` is `None`.
    ///
    /// # Errors
    /// As [`Self::from_qos_url`], for either URL.
    #[cfg(feature = "std")]
    pub fn from_service_profiles(request_url: &str, reply_url: Option<&str>) -> RpcResult<Self> {
        Self::from_service_profiles_with(request_url, reply_url, XmlProfileOptions::default())
    }

    /// [`Self::from_service_profiles`] with the settings of `options`.
    ///
    /// # Errors
    /// As [`Self::from_qos_url_with`], for either URL.
    #[cfg(feature = "std")]
    pub fn from_service_profiles_with(
        request_url: &str,
        reply_url: Option<&str>,
        options: XmlProfileOptions,
    ) -> RpcResult<Self> {
        let mut qos = Self::from_qos_url_with(request_url, options)?;
        if let Some(url) = reply_url {
            qos.reply_profile = Some(alloc::boxed::Box::new(Self::from_qos_url_with(
                url, options,
            )?));
        }
        Ok(qos)
    }

    /// Resolves a QoS profile from a `@Qos` profile URL (Spec §7.10.1).
    ///
    /// The URL form is `[file://]path[#library]`; the protocol, the `:`
    /// and the `//` are optional and default to `file`, the one protocol
    /// the clause makes mandatory. The XML at `path` is parsed and its
    /// named library (or the first one) supplies the profile. The later
    /// `file` URL's contents are the DDS4CCM XML QoS profiles.
    ///
    /// This is the non-network loader; a URL on another protocol is
    /// refused rather than fetched.
    ///
    /// # Errors
    /// * `RpcError::QosProfileNotFound` if the URL is malformed, names a
    ///   non-`file` protocol, the file cannot be read or parsed, or the
    ///   library or its first profile is missing.
    #[cfg(feature = "std")]
    pub fn from_qos_url(url: &str) -> RpcResult<Self> {
        Self::from_qos_url_with(url, XmlProfileOptions::default())
    }

    /// [`Self::from_qos_url`] with the settings of `options`; see
    /// [`Self::from_xml_profile_with`] for what is refused.
    ///
    /// # Errors
    /// As [`Self::from_qos_url`] and [`Self::from_xml_profile_with`].
    #[cfg(feature = "std")]
    pub fn from_qos_url_with(url: &str, options: XmlProfileOptions) -> RpcResult<Self> {
        let (library, path) = split_qos_url(url)?;
        let text = std::fs::read_to_string(path)
            .map_err(|_| RpcError::QosProfileNotFound(url.to_string()))?;
        // §7.10.1: the files are "XML QoS Profiles as defined in the [DDS4CCM]
        // specification", which writes a profile directly under `<dds>`
        // (DDS4CCM 1.1 §7.4.2.3); DDS-XML 1.0 has it in a `<qos_library>`.
        // Both are read, the first as a vendor form.
        let loader = zerodds_xml::parse_dds_xml_with(
            &text,
            &zerodds_xml::ReadOptions::accept_vendor_forms(),
        )
        .map_err(|_| RpcError::QosProfileNotFound(url.to_string()))?
        .value;
        let lib = match library {
            Some(name) => loader
                .qos_libraries
                .iter()
                .find(|l| l.name == name)
                .ok_or_else(|| RpcError::QosProfileNotFound(url.to_string()))?,
            None => loader
                .qos_libraries
                .first()
                .ok_or_else(|| RpcError::QosProfileNotFound(url.to_string()))?,
        };
        let profile = lib
            .profiles
            .first()
            .ok_or_else(|| RpcError::QosProfileNotFound(url.to_string()))?;
        let path = alloc::format!("{}::{}", lib.name, profile.name);
        let resolved = zerodds_xml::resolve_profile(&loader.qos_libraries, &path)
            .map_err(|e| qos_resolve_error(url, e))?;
        Self::from_resolved(Self::default_basic(), &resolved, options)
    }
}

/// §7.10.1: split a `@Qos` profile URL into its optional library and its
/// filesystem path.
fn split_qos_url(url: &str) -> RpcResult<(Option<&str>, &str)> {
    // `<protocol>://path` where the protocol is optional and only `file`
    // is supported.
    let rest = match url.split_once("://") {
        Some((protocol, rest)) => {
            if protocol != "file" {
                return Err(RpcError::QosProfileNotFound(url.to_string()));
            }
            rest
        }
        // Accept the single-slash `file:/path` spelling too.
        None => url.strip_prefix("file:").unwrap_or(url),
    };
    let (path, library) = match rest.split_once('#') {
        Some((path, library)) if !library.is_empty() => (path, Some(library)),
        Some((path, _)) => (path, None),
        None => (rest, None),
    };
    if path.is_empty() {
        return Err(RpcError::QosProfileNotFound(url.to_string()));
    }
    Ok((library, path))
}

impl Default for RpcQos {
    fn default() -> Self {
        Self::default_basic()
    }
}

// ---------------------------------------------------------------------
// Profile lowering
// ---------------------------------------------------------------------

/// Settings of the XML profile readers ([`RpcQos::from_xml_profile_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct XmlProfileOptions {
    /// Read the policies `RpcQos` models and leave out the rest, instead of
    /// refusing a profile that sets something it cannot carry. Where the
    /// DataWriter and DataReader containers disagree, the reader's value
    /// is taken. Off by default: a profile is an instruction and
    /// not honouring it should be visible.
    pub ignore_unmodelled: bool,
}

/// The containers of a resolved profile that the RPC endpoints are
/// configured from.
struct ResolvedProfile {
    writer: EntityQos,
    reader: EntityQos,
    /// The containers of entities the RPC layer does not configure that the
    /// profile (or a base) sets, and the `topic_filter` containers beyond
    /// the one selected.
    other_containers: alloc::vec::Vec<&'static str>,
}

/// Maps a failure of the shared resolver onto the RPC error kinds: a path
/// that names nothing is `QosProfileNotFound`, any other refusal
/// (cycle, depth limit) is `QosProfileInvalid`.
fn qos_resolve_error(path: &str, e: zerodds_xml::XmlError) -> RpcError {
    match e {
        zerodds_xml::XmlError::UnresolvedReference(_) => {
            RpcError::QosProfileNotFound(path.to_string())
        }
        other => RpcError::QosProfileInvalid {
            path: path.to_string(),
            reason: other.to_string(),
        },
    }
}

impl ResolvedProfile {
    /// Picks the DataWriter and DataReader container of a resolved profile
    /// (`zerodds_xml::default_qos`, the same fallback as
    /// [`RpcQos::from_resolved_default`]) and notes what is left over.
    fn select(resolved: &ResolvedQos) -> Self {
        let mut other = alloc::vec::Vec::new();
        for (name, slots) in [
            ("topic_qos", &resolved.topic_qos),
            ("publisher_qos", &resolved.publisher_qos),
            ("subscriber_qos", &resolved.subscriber_qos),
            ("domainparticipant_qos", &resolved.domainparticipant_qos),
        ] {
            if slots.iter().any(|f| f.qos != EntityQos::default()) {
                other.push(name);
            }
        }
        // A second container with a `topic_filter` is not applied to RPC
        // endpoints, which are not bound to one topic name.
        for (name, slots) in [
            ("datawriter_qos", &resolved.datawriter_qos),
            ("datareader_qos", &resolved.datareader_qos),
        ] {
            if slots.len() > 1 {
                other.push(name);
            }
        }
        Self {
            writer: zerodds_xml::default_qos(&resolved.datawriter_qos)
                .cloned()
                .unwrap_or_default(),
            reader: zerodds_xml::default_qos(&resolved.datareader_qos)
                .cloned()
                .unwrap_or_default(),
            other_containers: other,
        }
    }
}

/// The policies of an entity container that `RpcQos` does not model, by
/// their XML element name.
fn unmodelled_policies(eq: &EntityQos) -> alloc::vec::Vec<&'static str> {
    let mut out = alloc::vec::Vec::new();
    macro_rules! unmodelled {
        ($($field:ident),* $(,)?) => {
            $(if eq.$field.is_some() { out.push(stringify!($field)); })*
        };
    }
    unmodelled!(
        durability_service,
        presentation,
        latency_budget,
        ownership,
        ownership_strength,
        liveliness,
        time_based_filter,
        partition,
        transport_priority,
        destination_order,
        entity_factory,
        writer_data_lifecycle,
        reader_data_lifecycle,
        user_data,
        topic_data,
        group_data,
    );
    out
}

impl ResolvedProfile {
    /// Refuses what `RpcQos` cannot carry.
    fn check(&self) -> RpcResult<()> {
        for (container, eq) in [
            ("datawriter_qos", &self.writer),
            ("datareader_qos", &self.reader),
        ] {
            if let Some(policy) = unmodelled_policies(eq).first() {
                return Err(RpcError::UnmodelledQos {
                    container,
                    policy,
                    reason: "the requester and replier endpoints are configured with reliability, durability, history, resource limits, lifespan and deadline only",
                });
            }
        }
        if let Some(container) = self.other_containers.first() {
            return Err(RpcError::UnmodelledQos {
                container,
                policy: container,
                reason: "the RPC layer creates this entity with its defaults",
            });
        }
        if self.reader.lifespan.is_some() {
            return Err(RpcError::UnmodelledQos {
                container: "datareader_qos",
                policy: "lifespan",
                reason: "lifespan is a DataWriter policy (DDS 1.4 §2.2.3.16)",
            });
        }
        // One RpcQos carries one value of a policy for the writer and the
        // reader of an endpoint pair.
        macro_rules! agree {
            ($($field:ident),* $(,)?) => {
                $(if let (Some(w), Some(r)) = (&self.writer.$field, &self.reader.$field) {
                    if w != r {
                        return Err(RpcError::UnmodelledQos {
                            container: "datareader_qos",
                            policy: stringify!($field),
                            reason: "the datawriter_qos and datareader_qos containers set different values, and RpcQos has one",
                        });
                    }
                })*
            };
        }
        agree!(reliability, durability, history, resource_limits, deadline);
        Ok(())
    }
}

fn apply_entity_qos_to_writer_side(q: &mut RpcQos, eq: &EntityQos) {
    if let Some(p) = eq.reliability {
        q.reliability = p;
    }
    if let Some(p) = eq.durability {
        q.durability = p;
    }
    if let Some(p) = eq.history {
        // Writes onto the request writer and reply writer.
        q.request_history = p;
        q.reply_history = p;
    }
    if let Some(p) = eq.resource_limits {
        q.resource_limits = p;
    }
    if let Some(p) = eq.lifespan {
        q.lifespan = p;
    }
    if let Some(p) = eq.deadline {
        q.deadline = p;
    }
}

fn apply_entity_qos_to_reader_side(q: &mut RpcQos, eq: &EntityQos) {
    // The reader container overrides only reader-relevant policies.
    if let Some(p) = eq.reliability {
        q.reliability = p;
    }
    if let Some(p) = eq.durability {
        q.durability = p;
    }
    if let Some(p) = eq.history {
        q.request_history = p;
        q.reply_history = p;
    }
    if let Some(p) = eq.resource_limits {
        q.resource_limits = p;
    }
    if let Some(p) = eq.deadline {
        q.deadline = p;
    }
    // `lifespan` is writer-only (Spec §2.2.3.16) → ignore.
}

fn split_qos_path(path: &str) -> RpcResult<(&str, &str)> {
    let (lib, prof) = path
        .split_once("::")
        .ok_or_else(|| RpcError::QosProfileNotFound(path.to_string()))?;
    if lib.is_empty() || prof.is_empty() {
        return Err(RpcError::QosProfileNotFound(path.to_string()));
    }
    Ok((lib, prof))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_xml::parse_dds_xml;

    #[test]
    fn default_basic_matches_spec_710_2() {
        let q = RpcQos::default_basic();
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.durability.kind, DurabilityKind::Volatile);
        assert_eq!(q.request_history.kind, HistoryKind::KeepAll);
        assert_eq!(q.reply_history.kind, HistoryKind::KeepAll);
        // §7.10.2 names no resource limit: the DDS default, unbounded.
        assert_eq!(q.resource_limits, DEFAULT_RESOURCE_LIMITS);
        assert_eq!(
            q.resource_limits.max_samples,
            zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED
        );
        let bounded = RpcQos::default_basic().bounded();
        assert_eq!(bounded.resource_limits.max_samples, 512);
        assert_eq!(bounded.resource_limits.max_instances, 1);
        assert_eq!(bounded.resource_limits.max_samples_per_instance, 512);
        assert_eq!(q.request_timeout, core::time::Duration::from_secs(5));
    }

    #[test]
    fn default_enhanced_uses_the_same_history() {
        let q = RpcQos::default_enhanced();
        assert_eq!(q.request_history.kind, HistoryKind::KeepAll);
        assert_eq!(q.reply_history.kind, HistoryKind::KeepAll);
        // Remaining policies as in basic.
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q, RpcQos::default_basic());
    }

    #[test]
    fn default_is_basic() {
        assert_eq!(RpcQos::default(), RpcQos::default_basic());
    }

    #[test]
    fn writer_qos_carries_reliability_and_history() {
        let q = RpcQos::default_basic();
        let wq = q.request_writer_qos();
        assert_eq!(wq.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(wq.history.kind, HistoryKind::KeepAll);
        assert_eq!(
            wq.resource_limits.max_samples,
            zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED
        );
        assert_eq!(
            RpcQos::default_basic()
                .bounded()
                .request_writer_qos()
                .resource_limits
                .max_samples,
            512
        );
    }

    #[test]
    fn reader_qos_carries_reliability() {
        let q = RpcQos::default_basic();
        let rq = q.reply_reader_qos();
        assert_eq!(rq.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(rq.history.kind, HistoryKind::KeepAll);
    }

    #[test]
    fn split_qos_path_accepts_double_colon() {
        let (l, p) = split_qos_path("Lib::Prof").unwrap();
        assert_eq!(l, "Lib");
        assert_eq!(p, "Prof");
    }

    #[test]
    fn split_qos_path_rejects_missing_separator() {
        assert!(matches!(
            split_qos_path("nope"),
            Err(RpcError::QosProfileNotFound(_))
        ));
    }

    #[test]
    fn split_qos_path_rejects_empty_parts() {
        assert!(matches!(
            split_qos_path("::Prof"),
            Err(RpcError::QosProfileNotFound(_))
        ));
        assert!(matches!(
            split_qos_path("Lib::"),
            Err(RpcError::QosProfileNotFound(_))
        ));
    }

    #[test]
    fn from_xml_profile_overrides_history_depth() {
        let xml = r#"<dds>
            <qos_library name="RpcLib">
                <qos_profile name="Calculator">
                    <datawriter_qos>
                        <history>
                            <kind>KEEP_LAST_HISTORY_QOS</kind>
                            <depth>32</depth>
                        </history>
                    </datawriter_qos>
                </qos_profile>
            </qos_library>
        </dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let q = RpcQos::from_xml_profile(&loader, "RpcLib::Calculator").unwrap();
        assert_eq!(q.request_history.depth, 32);
        assert_eq!(q.reply_history.depth, 32);
    }

    /// §7.3.2.4.2: the profile's `base_name` chain is resolved, so a base's
    /// policies reach the RPC QoS.
    #[test]
    fn from_xml_profile_follows_base_name_chain() {
        let xml = r#"<dds>
            <qos_library name="RpcLib">
                <qos_profile name="Base">
                    <datawriter_qos>
                        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>64</depth></history>
                    </datawriter_qos>
                </qos_profile>
                <qos_profile name="Child" base_name="Base">
                    <datawriter_qos>
                        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>32</depth></history>
                    </datawriter_qos>
                </qos_profile>
            </qos_library>
        </dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let q = RpcQos::from_xml_profile(&loader, "RpcLib::Child").unwrap();
        // Inherited from Base.
        assert_eq!(q.reliability.kind, ReliabilityKind::BestEffort);
        // Overridden by Child.
        assert_eq!(q.request_history.depth, 32);
    }

    /// A profile that exists but whose chain cannot be resolved is not
    /// "not found": the reason is kept.
    #[test]
    fn from_xml_profile_unresolvable_chain_is_invalid_not_missing() {
        let circular = r#"<dds><qos_library name="A">
            <qos_profile name="P" base_name="Q"/>
            <qos_profile name="Q" base_name="P"/>
        </qos_library></dds>"#;
        let loader = parse_dds_xml(circular).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "A::P").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileInvalid { .. }), "{err:?}");
        let forward = r#"<dds><qos_library name="A">
            <qos_profile name="P" base_name="Q"/>
            <qos_profile name="Q"/>
        </qos_library></dds>"#;
        let loader = parse_dds_xml(forward).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "A::P").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileInvalid { .. }), "{err:?}");
    }

    #[test]
    fn from_xml_profile_unknown_library_errors() {
        let xml = r#"<dds><qos_library name="A"><qos_profile name="B"/></qos_library></dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "Missing::B").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    #[test]
    fn from_xml_profile_unknown_profile_errors() {
        let xml = r#"<dds><qos_library name="A"><qos_profile name="X"/></qos_library></dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "A::Missing").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    #[test]
    fn from_xml_profile_malformed_path_errors() {
        let loader = DdsXml::default();
        let err = RpcQos::from_xml_profile(&loader, "no-colon").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    #[test]
    fn split_qos_url_defaults_the_protocol_to_file() {
        // §7.10.1: the protocol, `:` and `//` are optional and default to
        // `file`; only `file` is supported.
        assert_eq!(
            split_qos_url("qos.xml#Lib").unwrap(),
            (Some("Lib"), "qos.xml")
        );
        assert_eq!(
            split_qos_url("file:///tmp/qos.xml#Lib").unwrap(),
            (Some("Lib"), "/tmp/qos.xml")
        );
        assert_eq!(
            split_qos_url("file:/tmp/qos.xml").unwrap(),
            (None, "/tmp/qos.xml")
        );
        assert!(matches!(
            split_qos_url("http://host/q.xml"),
            Err(RpcError::QosProfileNotFound(_))
        ));
        assert!(matches!(
            split_qos_url("file://#Lib"),
            Err(RpcError::QosProfileNotFound(_))
        ));
    }

    #[test]
    fn from_qos_url_reads_the_file_and_applies_the_library() {
        let path = std::env::temp_dir().join(format!(
            "zerodds-rpc-qos-{}-{:?}.xml",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(
            &path,
            r#"<dds>
                <qos_library name="Other"><qos_profile name="Ignored"/></qos_library>
                <qos_library name="RpcLib">
                    <qos_profile name="Calculator">
                        <datawriter_qos>
                            <history>
                                <kind>KEEP_LAST_HISTORY_QOS</kind>
                                <depth>7</depth>
                            </history>
                        </datawriter_qos>
                    </qos_profile>
                </qos_library>
            </dds>"#,
        )
        .unwrap();
        let url = format!("file://{}#RpcLib", path.display());
        let q = RpcQos::from_qos_url(&url).unwrap();
        assert_eq!(q.request_history.depth, 7);
        assert_eq!(q.reply_history.depth, 7);
        std::fs::remove_file(&path).ok();
    }

    /// §7.10.1: `@Qos(RequestProfile, ReplyProfile)` names one profile for
    /// the request endpoints and one for the reply endpoints.
    #[test]
    fn the_request_and_the_reply_profile_apply_to_their_own_endpoints() {
        let dir = std::env::temp_dir();
        let write = |name: &str, depth: u32| {
            let path = dir.join(format!(
                "zerodds-rpc-qos-{name}-{}-{:?}.xml",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::write(
                &path,
                format!(
                    r#"<dds><qos_library name="L"><qos_profile name="P">
                        <datawriter_qos><history><kind>KEEP_LAST_HISTORY_QOS</kind>
                        <depth>{depth}</depth></history></datawriter_qos>
                        <datareader_qos><history><kind>KEEP_LAST_HISTORY_QOS</kind>
                        <depth>{depth}</depth></history></datareader_qos>
                    </qos_profile></qos_library></dds>"#
                ),
            )
            .unwrap();
            format!("file://{}#L", path.display())
        };
        let request_url = write("request", 3);
        let reply_url = write("reply", 9);

        let both = RpcQos::from_service_profiles(&request_url, Some(&reply_url)).unwrap();
        assert_eq!(both.request_writer_qos().history.depth, 3);
        assert_eq!(both.request_reader_qos().history.depth, 3);
        assert_eq!(both.reply_writer_qos().history.depth, 9);
        assert_eq!(both.reply_reader_qos().history.depth, 9);

        let request_only = RpcQos::from_service_profiles(&request_url, None).unwrap();
        assert_eq!(request_only.reply_writer_qos().history.depth, 3);
        assert_eq!(request_only.reply_reader_qos().history.depth, 3);

        assert!(
            RpcQos::from_service_profiles(&request_url, Some("file:///nonexistent.xml")).is_err()
        );
    }

    fn profile_xml(body: &str) -> DdsXml {
        parse_dds_xml(&alloc::format!("<dds>{body}</dds>")).unwrap()
    }

    /// DDS-XML §7.3.2.4.2: `base_name` names the profile whose policies the
    /// derived profile starts from, in the same library or as
    /// `library::profile`.
    #[test]
    fn a_profile_inherits_from_its_base_profiles() {
        let loader = profile_xml(
            r#"<qos_library name="Base">
                 <qos_profile name="Root">
                   <datawriter_qos>
                     <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                     <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>4</depth></history>
                   </datawriter_qos>
                 </qos_profile>
               </qos_library>
               <qos_library name="Rpc">
                 <qos_profile name="Mid" base_name="Base::Root">
                   <datawriter_qos>
                     <deadline><period><sec>7</sec><nanosec>0</nanosec></period></deadline>
                   </datawriter_qos>
                 </qos_profile>
                 <qos_profile name="Leaf" base_name="Mid">
                   <datawriter_qos>
                     <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>9</depth></history>
                   </datawriter_qos>
                 </qos_profile>
               </qos_library>"#,
        );
        let q = RpcQos::from_xml_profile(&loader, "Rpc::Leaf").unwrap();
        assert_eq!(q.reliability.kind, ReliabilityKind::BestEffort, "from Root");
        assert_eq!(q.deadline.period, Duration::from_secs(7), "from Mid");
        assert_eq!(q.request_history.depth, 9, "Leaf overrides Root");
        assert_eq!(q.reply_history.depth, 9);
    }

    #[test]
    fn an_inheritance_cycle_is_invalid_and_a_missing_base_is_not_found() {
        let loader = profile_xml(
            r#"<qos_library name="L">
                 <qos_profile name="A" base_name="B"/>
                 <qos_profile name="B" base_name="A"/>
                 <qos_profile name="C" base_name="Nowhere"/>
               </qos_library>"#,
        );
        for path in ["L::C", "L::Missing", "Missing::A"] {
            assert!(
                matches!(
                    RpcQos::from_xml_profile(&loader, path),
                    Err(RpcError::QosProfileNotFound(_))
                ),
                "{path}"
            );
        }
        assert!(matches!(
            RpcQos::from_xml_profile(&loader, "L::A"),
            Err(RpcError::QosProfileInvalid { .. })
        ));
    }

    /// Every policy `RpcQos` models, from a profile.
    #[test]
    fn the_modelled_policies_are_read() {
        let loader = profile_xml(
            r#"<qos_library name="L"><qos_profile name="P">
                 <datawriter_qos>
                   <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                   <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
                   <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>3</depth></history>
                   <resource_limits>
                     <max_samples>10</max_samples><max_instances>1</max_instances>
                     <max_samples_per_instance>10</max_samples_per_instance>
                   </resource_limits>
                   <lifespan><duration><sec>2</sec><nanosec>0</nanosec></duration></lifespan>
                   <deadline><period><sec>1</sec><nanosec>0</nanosec></period></deadline>
                 </datawriter_qos>
               </qos_profile></qos_library>"#,
        );
        let q = RpcQos::from_xml_profile(&loader, "L::P").unwrap();
        assert_eq!(q.reliability.kind, ReliabilityKind::BestEffort);
        assert_eq!(q.durability.kind, DurabilityKind::TransientLocal);
        assert_eq!(q.request_history.depth, 3);
        assert_eq!(q.resource_limits.max_samples, 10);
        assert_eq!(q.lifespan.duration, Duration::from_secs(2));
        assert_eq!(q.deadline.period, Duration::from_secs(1));
    }

    fn unmodelled(err: RpcError) -> (&'static str, &'static str) {
        match err {
            RpcError::UnmodelledQos {
                container, policy, ..
            } => (container, policy),
            other => panic!("not an UnmodelledQos: {other:?}"),
        }
    }

    /// A policy the endpoints do not carry is named, not dropped; the
    /// named setting reads the rest.
    #[test]
    fn a_policy_the_endpoints_cannot_carry_is_refused_by_name() {
        let loader = profile_xml(
            r#"<qos_library name="L"><qos_profile name="P">
                 <datawriter_qos>
                   <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
                   <liveliness><kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind>
                     <lease_duration><sec>2</sec><nanosec>0</nanosec></lease_duration></liveliness>
                 </datawriter_qos>
               </qos_profile></qos_library>"#,
        );
        let err = RpcQos::from_xml_profile(&loader, "L::P").unwrap_err();
        assert_eq!(unmodelled(err), ("datawriter_qos", "liveliness"));
        let q = RpcQos::from_xml_profile_with(
            &loader,
            "L::P",
            XmlProfileOptions {
                ignore_unmodelled: true,
            },
        )
        .unwrap();
        assert_eq!(q.request_history.depth, 5);
    }

    #[test]
    fn a_container_of_an_entity_the_rpc_layer_does_not_configure_is_refused() {
        let loader = profile_xml(
            r#"<qos_library name="L"><qos_profile name="P">
                 <topic_qos>
                   <deadline><period><sec>1</sec><nanosec>0</nanosec></period></deadline>
                 </topic_qos>
               </qos_profile></qos_library>"#,
        );
        let err = RpcQos::from_xml_profile(&loader, "L::P").unwrap_err();
        assert_eq!(unmodelled(err), ("topic_qos", "topic_qos"));
    }

    #[test]
    fn the_writer_and_reader_containers_must_agree() {
        let loader = profile_xml(
            r#"<qos_library name="L"><qos_profile name="P">
                 <datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datawriter_qos>
                 <datareader_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datareader_qos>
               </qos_profile></qos_library>"#,
        );
        let err = RpcQos::from_xml_profile(&loader, "L::P").unwrap_err();
        assert_eq!(unmodelled(err), ("datareader_qos", "reliability"));
        let q = RpcQos::from_xml_profile_with(
            &loader,
            "L::P",
            XmlProfileOptions {
                ignore_unmodelled: true,
            },
        )
        .unwrap();
        assert_eq!(
            q.reliability.kind,
            ReliabilityKind::BestEffort,
            "the reader wins"
        );
    }

    /// DDS-RPC 1.0 §7.10.1: the profile files are DDS4CCM QoS profiles, which
    /// write a `<qos_profile>` (and the single-QoS shortcut) directly under
    /// `<dds>` without a `<qos_library>`.
    #[test]
    fn from_qos_url_reads_a_ddscm_profile_file() {
        let path = std::env::temp_dir().join(format!(
            "zerodds-rpc-qos-ccm-{}-{:?}.xml",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(
            &path,
            r#"<dds>
                 <qos_profile name="StrictReliableCommunicationProfile">
                   <datawriter_qos>
                     <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
                     <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
                   </datawriter_qos>
                   <datareader_qos>
                     <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
                     <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
                   </datareader_qos>
                 </qos_profile>
               </dds>"#,
        )
        .unwrap();
        let q = RpcQos::from_qos_url(&format!("file://{}", path.display())).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.request_history.kind, HistoryKind::KeepAll);
    }

    /// A reader container has no lifespan (DDS 1.4 §2.2.3); `crates/xml`
    /// refuses it while reading, so a profile file that sets one is not a
    /// profile for `from_qos_url`.
    #[test]
    fn lifespan_in_a_reader_container_is_refused_by_the_xml_reader() {
        let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
                 <datareader_qos>
                   <lifespan><duration><sec>2</sec><nanosec>0</nanosec></duration></lifespan>
                 </datareader_qos>
               </qos_profile></qos_library></dds>"#;
        assert!(matches!(
            zerodds_xml::parse_dds_xml(xml),
            Err(zerodds_xml::XmlError::UnknownElement(_))
        ));
        let path = std::env::temp_dir().join(format!(
            "zerodds-rpc-qos-reader-lifespan-{}-{:?}.xml",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, xml).unwrap();
        let err = RpcQos::from_qos_url(&format!("file://{}", path.display())).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    /// `DdsXml` is a public structure and can be built without the reader;
    /// the check on a reader-side lifespan stays for such a document.
    #[test]
    fn lifespan_in_a_reader_container_of_a_built_document_is_refused() {
        let mut loader = profile_xml(
            r#"<qos_library name="L"><qos_profile name="P">
                 <datawriter_qos>
                   <lifespan><duration><sec>2</sec><nanosec>0</nanosec></duration></lifespan>
                 </datawriter_qos>
               </qos_profile></qos_library>"#,
        );
        let profile = &mut loader.qos_libraries[0].profiles[0];
        profile.datareader_qos = profile.datawriter_qos.clone();
        let err = RpcQos::from_xml_profile(&loader, "L::P").unwrap_err();
        assert_eq!(unmodelled(err), ("datareader_qos", "lifespan"));
    }
}
