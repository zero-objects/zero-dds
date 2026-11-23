// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Where a call goes and what a message carries (spec §4.2, §4.4, §4.5, §5.1,
//! §7.3): the route of a service and method after the ACL, the `Sample` of a
//! `Publish` and of a delivered DDS sample, the encapsulation header.

use super::*;

/// The duration of a `grpc-timeout` (gRPC over HTTP/2, "Timeout").
pub(crate) fn timeout_duration(value: u32, unit: TimeoutUnit) -> Duration {
    let v = u64::from(value);
    match unit {
        TimeoutUnit::Hour => Duration::from_secs(v * 3600),
        TimeoutUnit::Minute => Duration::from_secs(v * 60),
        TimeoutUnit::Second => Duration::from_secs(v),
        TimeoutUnit::Millisecond => Duration::from_millis(v),
        TimeoutUnit::Microsecond => Duration::from_micros(v),
        TimeoutUnit::Nanosecond => Duration::from_nanos(v),
    }
}

/// Spec §4.5: a DDS write that found no room — RESOURCE_LIMITS reached, or
/// a reliable writer blocked beyond `max_blocking_time` — is
/// RESOURCE_EXHAUSTED; any other DDS error is INTERNAL.
pub(crate) fn status_of_write_error(e: &zerodds_dcps::DdsError) -> Status {
    use zerodds_dcps::DdsError;
    match e {
        DdsError::OutOfResources { .. } | DdsError::Timeout => Status::ResourceExhausted,
        _ => Status::Internal,
    }
}

/// Nanoseconds since the Unix epoch now.
pub(crate) fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Spec §6 Lifespan of the samples of a call: the metadata
/// `zerodds-lifespan-ms` (1 to 2^31-1 milliseconds), else the call's
/// `grpc-timeout` rounded up to whole seconds (a client sends the time left
/// to its deadline, which differs from call to call); `None` for neither.
///
/// # Errors
/// A `zerodds-lifespan-ms` that is not such a number.
pub(crate) fn lifespan_of(call: &GrpcCall) -> Result<Option<u64>, String> {
    if let Some((_, v)) = call
        .metadata
        .iter()
        .find(|(n, _)| n == "zerodds-lifespan-ms")
    {
        return match v.trim().parse::<u64>() {
            Ok(ms) if (1..=i32::MAX as u64).contains(&ms) => Ok(Some(ms)),
            _ => Err(format!(
                "zerodds-lifespan-ms {v:?} is not 1 to 2147483647 milliseconds (spec §6)"
            )),
        };
    }
    Ok(call
        .timeout
        .as_deref()
        .and_then(|t| decode_timeout(t).ok())
        .map(|(value, unit)| {
            let d = timeout_duration(value, unit);
            let secs = d.as_secs() + u64::from(d.subsec_nanos() > 0);
            (secs.max(1) * 1000).min(i32::MAX as u64)
        }))
}

/// The DDS sample a `Publish` message names (spec §4.2 `Sample`, §4.4):
/// the CDR bytes under their encapsulation header, or a dispose or
/// unregister of the instance `key_hash`.
///
/// # Errors
/// INVALID_ARGUMENT, with why: a message that is not a `Sample`, a
/// `type_name` or `topic_name` that is not the mapping's, an unknown flag,
/// a `key_hash` of another length than 16, a `source_ts_ns` above
/// [`dds_pump::MAX_SOURCE_TS_NS`] (the RTPS timestamp cannot hold it), a
/// dispose without a key or with `cdr_payload`, a `cdr_payload` without a
/// CDR encapsulation header.
pub(crate) fn sample_to_publish<'s>(
    topic: &TopicMapping,
    sample: &'s Sample,
) -> Result<Outgoing<'s>, String> {
    use crate::messages::KNOWN_FLAGS;
    if !sample.type_name.is_empty() && sample.type_name != topic.dds_type {
        return Err(format!(
            "Sample.type_name {:?} is not the type {:?} of {}",
            sample.type_name, topic.dds_type, topic.dds_name
        ));
    }
    if !sample.topic_name.is_empty() && sample.topic_name != topic.dds_name {
        return Err(format!(
            "Sample.topic_name {:?} is not the topic {:?} of the service",
            sample.topic_name, topic.dds_name
        ));
    }
    if sample.flags & !KNOWN_FLAGS != 0 {
        return Err(format!(
            "Sample.flags {:#x}: bits outside dispose (1) and unregister (2)",
            sample.flags
        ));
    }
    let key_hash = match sample.key_hash.len() {
        0 => None,
        16 => {
            let mut k = [0u8; 16];
            k.copy_from_slice(&sample.key_hash);
            Some(k)
        }
        n => return Err(format!("Sample.key_hash of {n} octets, not 16")),
    };
    // Spec §3 `keyed: false`: a type without a key has no instance a
    // key_hash could name.
    if key_hash.is_some() && !topic.keyed {
        return Err(format!(
            "Sample.key_hash on {}, whose type has no key (keyed: false)",
            topic.dds_name
        ));
    }
    let source_ts = match sample.source_ts_ns {
        0 => None,
        ns => Some(dds_pump::timestamp_of(ns).ok_or_else(|| {
            format!(
                "Sample.source_ts_ns {ns} is above {}, the last nanosecond an RTPS timestamp holds (spec §4.3)",
                dds_pump::MAX_SOURCE_TS_NS
            )
        })?),
    };
    if sample.flags != 0 {
        if key_hash.is_none() {
            return Err("a dispose or unregister needs Sample.key_hash".into());
        }
        if !sample.cdr_payload.is_empty() {
            return Err("a dispose or unregister carries no Sample.cdr_payload".into());
        }
        return Ok(Outgoing {
            identifier: [0, 0],
            body: &[],
            flags: sample.flags,
            key_hash,
            source_ts,
            lifespan_ms: None,
        });
    }
    let (identifier, body) = split_encapsulation(&sample.cdr_payload)
        .map_err(|e| format!("Sample.cdr_payload: {e} (spec §4.4)"))?;
    Ok(Outgoing {
        identifier,
        body,
        flags: 0,
        key_hash,
        source_ts,
        lifespan_ms: None,
    })
}

/// The `Sample` of a delivered DDS sample (spec §4.2): the encapsulation
/// header and the CDR bytes (§4.4), the flags, the key, the source
/// timestamp, the type and the topic.
pub(crate) fn sample_of(topic: &TopicMapping, d: &Delivered) -> Sample {
    let cdr_payload = if d.flags == 0 {
        let mut cdr = Vec::with_capacity(d.payload.len() + 4);
        cdr.extend_from_slice(&encapsulation(d.identifier));
        cdr.extend_from_slice(&d.payload);
        cdr
    } else {
        Vec::new()
    };
    Sample {
        cdr_payload,
        flags: d.flags,
        key_hash: d.key_hash.map(|k| k.to_vec()).unwrap_or_default(),
        source_ts_ns: d.source_ts_ns.unwrap_or(0),
        type_name: topic.dds_type.clone(),
        topic_name: topic.dds_name.clone(),
    }
}

/// The encapsulation header (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3
/// §7.6.3.1.2): the representation identifier the sample came under, and
/// options. The CDR bytes go on unchanged, so the identifier does too; it
/// names the XCDR version, the format (plain, delimited, parameter list) and
/// the byte order.
pub(crate) fn encapsulation(identifier: [u8; 2]) -> [u8; 4] {
    [identifier[0], identifier[1], 0, 0]
}

/// The representation identifier and the CDR bytes of a
/// `Sample.cdr_payload` (§4.4: the encapsulation header, then the CDR bytes).
///
/// # Errors
/// A payload shorter than the header, or an identifier that names no CDR
/// representation (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).
pub(crate) fn split_encapsulation(payload: &[u8]) -> Result<([u8; 2], &[u8]), String> {
    if payload.len() < 4 {
        return Err("shorter than the encapsulation header".into());
    }
    let identifier = [payload[0], payload[1]];
    match u16::from_be_bytes(identifier) {
        0x0000..=0x0003 | 0x0006..=0x000b => Ok((identifier, &payload[4..])),
        other => Err(format!(
            "encapsulation {other:#06x} is not a CDR representation"
        )),
    }
}

/// The data representation an identifier names (DDS-XTypes 1.3 Table 60):
/// `CDR`, `PL_CDR` are XCDR1, the others XCDR2.
pub(crate) fn offer_of(identifier: [u8; 2]) -> i16 {
    use zerodds_rtps::publication_data::data_representation as dr;
    if identifier[1] <= 0x03 {
        dr::XCDR
    } else {
        dr::XCDR2
    }
}

/// The extensibility the DDS writer frames a body under `identifier` with
/// (DDS-XTypes 1.3 Table 60): a parameter list (`PL_CDR`, `PL_CDR2`) is
/// `@mutable`, a delimited body (`D_CDR2`) `@appendable`, anything else
/// `@final`.
pub(crate) fn wire_extensibility_of(
    identifier: [u8; 2],
) -> zerodds_types::qos::ExtensibilityForRepr {
    use zerodds_types::qos::ExtensibilityForRepr as Ext;
    match identifier[1] & !0x01 {
        0x02 | 0x0a => Ext::Mutable,
        0x08 => Ext::Appendable,
        _ => Ext::Final,
    }
}

/// `direction` `in`/`bidir`: `Publish` carries gRPC messages into DDS.
pub(crate) fn bridged_into_dds(t: &TopicMapping) -> bool {
    matches!(t.direction.as_str(), "in" | "bidir")
}

/// `direction` `out`/`bidir`: `Subscribe` carries DDS samples to gRPC.
pub(crate) fn bridged_out_of_dds(t: &TopicMapping) -> bool {
    matches!(t.direction.as_str(), "out" | "bidir")
}

/// Where a call of `method` on `service` goes (spec §4.2, §4.6), after the
/// ACL of §7.3 (`Publish`/`PublishOne` write the topic, `Subscribe` reads
/// it; a tenant's topic is `<tenant>/<dds_name>`, §1 L6) and the
/// `direction` of the topic.
///
/// # Errors
/// The status the call ends with: UNIMPLEMENTED for the reflection service
/// when it is disabled and for a method the service does not have,
/// NOT_FOUND for a service that is not mapped, PERMISSION_DENIED,
/// FAILED_PRECONDITION for a direction the topic does not carry,
/// UNAVAILABLE without a DDS participant.
pub(crate) fn route(
    service: &str,
    method: &str,
    part: &Participant,
    reflection_enabled: bool,
    acl: &crate::bridge_security::Acl,
    subject: &AuthSubject,
) -> Result<Route, (Status, String)> {
    // Both names of the reflection service — a client asks for `v1`
    // first and only falls back to `v1alpha` on UNIMPLEMENTED.
    if crate::reflection::is_reflection_service(service) {
        return if !reflection_enabled {
            Err((Status::Unimplemented, "reflection disabled".into()))
        } else if method == "ServerReflectionInfo" {
            Ok(Route::Reflection)
        } else {
            Err((
                Status::Unimplemented,
                format!("method {method} not on {service}"),
            ))
        };
    }
    // Match on the full service name or the last component (spec §5.1).
    let Some(index) = topic_index(&part.topics, service) else {
        let packages = packages_of(&part.topics, service);
        if packages.len() > 1 {
            return Err((
                Status::NotFound,
                format!(
                    "service {service} is in the packages {}: the call names one (spec §5.1)",
                    packages.join(", ")
                ),
            ));
        }
        return Err((Status::NotFound, format!("unknown service {service}")));
    };
    let topic = &part.topics[index];
    let op = match method {
        "Publish" | "PublishOne" => Some(AclOp::Write),
        "Subscribe" => Some(AclOp::Read),
        "Catalog" => None,
        _ => {
            return Err((
                Status::Unimplemented,
                format!("method {method} not on {service}"),
            ));
        }
    };
    if let Some(op) = op
        && !authorize(acl, subject, op, &part.label(&topic.dds_name))
    {
        return Err((
            Status::PermissionDenied,
            format!(
                "{} may not {} {}",
                subject.name,
                if op == AclOp::Write {
                    "publish"
                } else {
                    "subscribe"
                },
                topic.dds_name
            ),
        ));
    }
    let carried = match op {
        Some(AclOp::Write) => bridged_into_dds(topic),
        Some(AclOp::Read) => bridged_out_of_dds(topic),
        None => true,
    };
    if !carried {
        return Err((
            Status::FailedPrecondition,
            format!(
                "{} is not bridged {} DDS (direction {})",
                topic.dds_name,
                if op == Some(AclOp::Write) {
                    "into"
                } else {
                    "out of"
                },
                topic.direction
            ),
        ));
    }
    if op.is_some() && part.dds().is_none() {
        return Err((Status::Unavailable, "no DDS participant".into()));
    }
    Ok(match method {
        "Publish" => Route::Publish(index),
        "PublishOne" => Route::PublishOne(index),
        "Subscribe" => Route::Subscribe(index),
        _ => Route::Catalog(index),
    })
}

/// The index of the mapping a service name addresses: the fully-qualified
/// name, or the service name without a package (spec §5.1:
/// `zerodds.chat.v1.ChatMessageStream` is the fully-qualified service,
/// `ChatMessageStream` the slug).
///
/// A service name without a package that two mappings in different
/// packages have addresses neither ([`packages_of`]).
pub(crate) fn topic_index(topics: &[TopicMapping], service: &str) -> Option<usize> {
    match service.rsplit_once('.') {
        // The fully-qualified name names the package of the mapping (§5.1:
        // `grpc_package` or its default).
        Some((qualifier, name)) => topics
            .iter()
            .position(|t| name == t.grpc_service && qualifier == t.package()),
        None => {
            let mut hits = topics
                .iter()
                .enumerate()
                .filter(|(_, t)| t.grpc_service == service)
                .map(|(i, _)| i);
            let first = hits.next()?;
            hits.next().is_none().then_some(first)
        }
    }
}

/// The packages of the mappings whose service is `service`, a name
/// without a package (spec §5.1); empty for a qualified name.
pub(crate) fn packages_of(topics: &[TopicMapping], service: &str) -> Vec<String> {
    if service.contains('.') {
        return Vec::new();
    }
    topics
        .iter()
        .filter(|t| t.grpc_service == service)
        .map(|t| t.package())
        .collect()
}

/// The mapping a service name addresses ([`topic_index`]).
pub(crate) fn topic_for<'a>(topics: &'a [TopicMapping], service: &str) -> Option<&'a TopicMapping> {
    topic_index(topics, service).map(|i| &topics[i])
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::bridge_daemon::test_support::{parse_yaml_subset, participant, topic};
    use crate::bridge_daemon::*;
    use crate::bridge_security::Acl;

    /// Spec §4.4 with DDS-XTypes 1.3 §7.6.3.1.2, Table 60: `cdr_payload`
    /// carries the encapsulation header the sample came under; the
    /// identifier names the representation, the format and the byte order.
    #[test]
    fn xtypes_7_6_3_1_2_cdr_payload_carries_the_header_the_sample_came_under() {
        let cdr = [&encapsulation([0x00, 0x0a])[..], &[1, 2, 3, 4]].concat();
        assert_eq!(&cdr[..4], &[0x00, 0x0a, 0x00, 0x00]);
        assert_eq!(
            split_encapsulation(&cdr).unwrap(),
            ([0x00, 0x0a], &[1u8, 2, 3, 4][..])
        );
        assert!(split_encapsulation(b"hi").is_err());
        assert!(
            split_encapsulation(&[0x00, 0x04, 0, 0, 1]).is_err(),
            "0x0004 is no CDR"
        );
        use zerodds_rtps::publication_data::data_representation as dr;
        use zerodds_types::qos::ExtensibilityForRepr as Ext;
        for (identifier, offer, extensibility, big_endian) in [
            ([0x00, 0x00], dr::XCDR, Ext::Final, true),
            ([0x00, 0x03], dr::XCDR, Ext::Mutable, false),
            ([0x00, 0x07], dr::XCDR2, Ext::Final, false),
            ([0x00, 0x08], dr::XCDR2, Ext::Appendable, true),
            ([0x00, 0x0b], dr::XCDR2, Ext::Mutable, false),
        ] {
            assert_eq!(offer_of(identifier), offer, "{identifier:?}");
            assert_eq!(
                wire_extensibility_of(identifier),
                extensibility,
                "{identifier:?}"
            );
            assert_eq!(identifier[1] & 0x01 == 0, big_endian, "{identifier:?}");
        }
    }

    fn route_of(cfg: &DaemonConfig, service: &str, method: &str) -> Result<Route, Status> {
        route(
            service,
            method,
            &participant(cfg),
            false,
            &Acl::allow_all(),
            &AuthSubject::anonymous(),
        )
        .map_err(|(status, _)| status)
    }

    /// §4.5: an unknown service is NOT_FOUND, a method the topic service
    /// does not have UNIMPLEMENTED, the reflection service UNIMPLEMENTED
    /// while disabled; `Catalog` needs no DDS side.
    #[test]
    fn dispatch_unknown_service_yields_not_found() {
        let cfg = DaemonConfig {
            topics: vec![TopicMapping {
                dds_name: "T::M".into(),
                grpc_service: "TMStream".into(),
                ..TopicMapping::default()
            }],
            ..DaemonConfig::default()
        };
        assert_eq!(route_of(&cfg, "no.such.Svc", "X"), Err(Status::NotFound));
        assert_eq!(
            route_of(&cfg, "TMStream", "Other"),
            Err(Status::Unimplemented)
        );
        assert_eq!(
            route_of(
                &cfg,
                "grpc.reflection.v1alpha.ServerReflection",
                "ServerReflectionInfo"
            ),
            Err(Status::Unimplemented)
        );
        assert_eq!(
            route_of(&cfg, "zerodds.t.v1.TMStream", "Catalog"),
            Ok(Route::Catalog(0))
        );
    }

    /// Without a DDS side `Publish` is UNAVAILABLE, not an OK a client would
    /// read as delivered.
    #[test]
    fn dispatch_publish_on_known_topic_is_ok() {
        let cfg = DaemonConfig {
            topics: vec![TopicMapping {
                dds_name: "T::M".into(),
                grpc_service: "TMStream".into(),
                direction: "bidir".into(),
                ..TopicMapping::default()
            }],
            ..DaemonConfig::default()
        };
        assert_eq!(
            route_of(&cfg, "TMStream", "Publish"),
            Err(Status::Unavailable)
        );
        assert_eq!(
            route_of(&cfg, "TMStream", "PublishOne"),
            Err(Status::Unavailable)
        );
    }

    /// `direction` decides which of `Publish` and `Subscribe` a topic
    /// answers; the other is FAILED_PRECONDITION, before the DDS side is
    /// consulted.
    #[test]
    fn dispatch_refuses_the_direction_a_topic_does_not_carry() {
        for (direction, method, status) in [
            ("out", "Publish", Status::FailedPrecondition),
            ("out", "PublishOne", Status::FailedPrecondition),
            ("in", "Subscribe", Status::FailedPrecondition),
            ("in", "Publish", Status::Unavailable),
            ("out", "Subscribe", Status::Unavailable),
        ] {
            let cfg = DaemonConfig {
                topics: vec![TopicMapping {
                    dds_name: "T::M".into(),
                    grpc_service: "TMStream".into(),
                    direction: direction.into(),
                    ..TopicMapping::default()
                }],
                ..DaemonConfig::default()
            };
            assert_eq!(
                route_of(&cfg, "TMStream", method),
                Err(status),
                "{direction} {method}"
            );
        }
    }

    /// Spec §3 `keyed: false`: a `key_hash` on a topic whose type has no
    /// key names no instance and is INVALID_ARGUMENT; the sample without
    /// one is written.
    #[test]
    fn grpcbridge_10_a_key_hash_on_a_keyless_topic_is_refused() {
        let t = TopicMapping {
            keyed: false,
            ..topic("T::K")
        };
        let s = Sample {
            cdr_payload: vec![0, 1, 0, 0, 9],
            key_hash: vec![3; 16],
            ..Sample::default()
        };
        assert!(
            sample_to_publish(&t, &s)
                .unwrap_err()
                .contains("keyed: false")
        );
        let plain = Sample {
            key_hash: Vec::new(),
            ..s
        };
        assert_eq!(sample_to_publish(&t, &plain).unwrap().key_hash, None);
    }

    /// §4.2, §4.4: what a `Publish` sample names is checked: the type and
    /// the topic of the mapping, known flags, a 16-octet key, a dispose
    /// with a key and without payload, a CDR encapsulation header.
    #[test]
    fn grpcbridge_4_2_a_publish_sample_is_checked() {
        let t = topic("T::M");
        let ok = Sample {
            cdr_payload: vec![0, 1, 0, 0, 9],
            key_hash: vec![3; 16],
            source_ts_ns: 1_500_000_000_250_000_000,
            type_name: "T::Type".into(),
            topic_name: "T::M".into(),
            ..Sample::default()
        };
        let o = sample_to_publish(&t, &ok).unwrap();
        assert_eq!(o.identifier, [0, 1]);
        assert_eq!(o.body, [9]);
        assert_eq!(o.key_hash, Some([3; 16]));
        assert_eq!(
            o.source_ts.map(dds_pump::nanos_of),
            Some(1_500_000_000_250_000_000)
        );
        let bad = |f: &dyn Fn(&mut Sample)| {
            let mut s = ok.clone();
            f(&mut s);
            sample_to_publish(&t, &s).unwrap_err()
        };
        assert!(bad(&|s| s.type_name = "Other".into()).contains("type_name"));
        assert!(bad(&|s| s.topic_name = "Other".into()).contains("topic_name"));
        assert!(bad(&|s| s.flags = 4).contains("flags"));
        assert!(bad(&|s| s.key_hash = vec![1; 15]).contains("15 octets"));
        // A source timestamp the RTPS timestamp cannot hold is refused,
        // not written without one; the last one it holds is taken.
        for ns in [u64::MAX, dds_pump::MAX_SOURCE_TS_NS + 1] {
            assert!(bad(&|s| s.source_ts_ns = ns).contains("source_ts_ns"));
        }
        let mut last = ok.clone();
        last.source_ts_ns = dds_pump::MAX_SOURCE_TS_NS;
        let ts = sample_to_publish(&t, &last).unwrap().source_ts.unwrap();
        assert_eq!(ts.seconds, u32::MAX);
        // No source timestamp: the write takes the time of the write.
        last.source_ts_ns = 0;
        assert_eq!(sample_to_publish(&t, &last).unwrap().source_ts, None);
        assert!(bad(&|s| s.cdr_payload = vec![1, 2]).contains("encapsulation"));
        assert!(bad(&|s| s.flags = 1).contains("no Sample.cdr_payload"));
        assert!(
            bad(&|s| {
                s.flags = 1;
                s.cdr_payload.clear();
                s.key_hash.clear();
            })
            .contains("key_hash")
        );
        let dispose = Sample {
            flags: 1,
            key_hash: vec![3; 16],
            ..Sample::default()
        };
        assert_eq!(sample_to_publish(&t, &dispose).unwrap().flags, 1);
        // Empty type and topic names are "not named".
        let bare = Sample {
            cdr_payload: vec![0, 7, 0, 0],
            ..Sample::default()
        };
        assert!(sample_to_publish(&t, &bare).is_ok());
    }

    /// §4.5: a DDS write without room is RESOURCE_EXHAUSTED, any other
    /// DDS error INTERNAL.
    #[test]
    fn grpcbridge_4_5_write_errors_map_to_resource_exhausted_and_internal() {
        use zerodds_dcps::DdsError;
        assert_eq!(
            status_of_write_error(&DdsError::OutOfResources { what: "x" }),
            Status::ResourceExhausted
        );
        assert_eq!(
            status_of_write_error(&DdsError::Timeout),
            Status::ResourceExhausted
        );
        assert_eq!(
            status_of_write_error(&DdsError::BadParameter { what: "x" }),
            Status::Internal
        );
        assert_eq!(
            status_of_write_error(&DdsError::NotEnabled),
            Status::Internal
        );
    }

    /// §4.3 `grpc-timeout`: the six units.
    #[test]
    fn grpcbridge_4_3_grpc_timeout_units() {
        let d = |h: &str| {
            let (v, u) = decode_timeout(h).unwrap();
            timeout_duration(v, u)
        };
        assert_eq!(d("2H"), Duration::from_secs(7200));
        assert_eq!(d("3M"), Duration::from_secs(180));
        assert_eq!(d("30S"), Duration::from_secs(30));
        assert_eq!(d("250m"), Duration::from_millis(250));
        assert_eq!(d("7u"), Duration::from_micros(7));
        assert_eq!(d("99999999n"), Duration::from_nanos(99_999_999));
        assert!(decode_timeout("123456789S").is_err());
    }

    /// Spec §5.1: a service name without a package that two mappings in
    /// different packages have addresses neither; each is addressed by its
    /// fully-qualified name.
    #[test]
    fn grpcbridge_5_1_a_service_name_in_two_packages_needs_the_package() {
        let cfg = parse_yaml_subset(
            "topics:\n  - dds_name: \"Chat::Message\"\n  - dds_name: \"Chat::Message\"\n    grpc_package: other.v1\n",
        )
        .expect("two packages");
        assert_eq!(topic_index(&cfg.topics, "ChatMessageStream"), None);
        assert_eq!(
            packages_of(&cfg.topics, "ChatMessageStream"),
            ["zerodds.chat.v1", "other.v1"]
        );
        assert_eq!(
            topic_index(&cfg.topics, "zerodds.chat.v1.ChatMessageStream"),
            Some(0)
        );
        assert_eq!(
            topic_index(&cfg.topics, "other.v1.ChatMessageStream"),
            Some(1)
        );
        assert_eq!(
            route_of(&cfg, "ChatMessageStream", "Catalog"),
            Err(Status::NotFound)
        );
        assert_eq!(
            route_of(&cfg, "other.v1.ChatMessageStream", "Catalog"),
            Ok(Route::Catalog(1))
        );
    }
}
