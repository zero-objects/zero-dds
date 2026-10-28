// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `@Qos` profile-URL side of the DDS-RPC 1.0 C++ binding (§7.10.1):
//! [`zerodds_endpoint_qos_from_profile_url`] resolves a profile URL to the
//! endpoint QoS an XML QoS profile names. Needs `zerodds-xml`, so it is
//! built with the `rpc` feature; it is re-exported from
//! [`crate::rpc_qos_ffi`] under its old path.

use core::ffi::{c_char, c_int};
use std::ffi::CStr;

use zerodds_qos::{DurabilityKind, HistoryKind, ReliabilityKind};

use crate::{ZeroDdsEndpointQos, ZeroDdsStatus, zerodds_endpoint_qos_default};

/// The nanoseconds of a DDS duration, `0` for infinite.
fn duration_ns(d: zerodds_qos::Duration) -> u64 {
    if d.is_infinite() {
        0
    } else {
        u64::try_from(d.to_nanos()).unwrap_or(0)
    }
}

/// `[<protocol>://]path[#library]`, §7.10.1: the protocol is optional and
/// defaults to `file`, the one protocol the clause makes mandatory.
fn split_profile_url(url: &str) -> Option<(Option<&str>, &str)> {
    let rest = match url.split_once("://") {
        Some((protocol, rest)) => {
            if protocol != "file" {
                return None;
            }
            rest
        }
        None => url.strip_prefix("file:").unwrap_or(url),
    };
    let (path, library) = match rest.split_once('#') {
        Some((path, library)) if !library.is_empty() => (path, Some(library)),
        Some((path, _)) => (path, None),
        None => (rest, None),
    };
    if path.is_empty() {
        return None;
    }
    Some((library, path))
}

/// The endpoint QoS DDS-RPC 1.0 §7.10.2 names as the default: RELIABLE,
/// KEEP_ALL, VOLATILE.
fn rpc_default() -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        transient_local: 0,
        history_keep_all: 1,
        history_depth: 1,
        ..zerodds_endpoint_qos_default()
    }
}

/// Applies the policies an XML `<datawriter_qos>` / `<datareader_qos>`
/// container names. `writer` says whether the container is the writer's:
/// LIFESPAN belongs to the writer only (DDS 1.4 §2.2.3.16).
fn apply_container(
    out: &mut ZeroDdsEndpointQos,
    container: &zerodds_xml::EntityQos,
    writer: bool,
) -> Result<(), ZeroDdsStatus> {
    if let Some(p) = container.reliability {
        out.reliable = c_int::from(p.kind == ReliabilityKind::Reliable);
    }
    if let Some(p) = container.durability {
        match p.kind {
            DurabilityKind::Volatile => out.transient_local = 0,
            DurabilityKind::TransientLocal => out.transient_local = 1,
            // The runtime endpoints know the two; a durability service is
            // not one of them, and falling back to another kind would be a
            // different contract than the profile states.
            _ => return Err(ZeroDdsStatus::Unsupported),
        }
    }
    if let Some(p) = container.history {
        out.history_keep_all = c_int::from(p.kind == HistoryKind::KeepAll);
        out.history_depth = u32::try_from(p.depth).unwrap_or(1).max(1);
    }
    if let Some(p) = container.resource_limits {
        out.max_samples = p.max_samples;
        out.max_instances = p.max_instances;
        out.max_samples_per_instance = p.max_samples_per_instance;
    }
    if let Some(p) = container.deadline {
        out.deadline_ns = duration_ns(p.period);
    }
    if writer {
        if let Some(p) = container.lifespan {
            out.lifespan_ns = duration_ns(p.duration);
        }
    }
    Ok(())
}

/// Resolves a `@Qos` profile URL (DDS-RPC 1.0 §7.10.1) to endpoint QoS.
///
/// The URL is `[file://]path[#library]`; the XML at `path` holds DDS4CCM QoS
/// profiles, and the named library — or the first one — supplies its first
/// profile. The profile's `<datawriter_qos>` and then its `<datareader_qos>`
/// are applied over the DDS-RPC defaults of §7.10.2 (RELIABLE, KEEP_ALL,
/// VOLATILE), the reader container leaving the writer-only LIFESPAN alone.
/// Another protocol than `file` is refused, not fetched.
///
/// Returns `ZERODDS_OK`; `BadHandle` for NULL, `BadParameter` for a URL that
/// is not one or names another protocol, `Error` when the file cannot be read
/// or parsed or the library or profile is missing, `Unsupported` for a
/// durability kind the runtime endpoints do not have.
///
/// # Safety
/// `url` is a NUL-terminated string and `out` points at a writable
/// [`ZeroDdsEndpointQos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_endpoint_qos_from_profile_url(
    url: *const c_char,
    out: *mut ZeroDdsEndpointQos,
) -> c_int {
    if url.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: `url` is NUL-terminated per the # Safety doc.
    let Ok(url) = unsafe { CStr::from_ptr(url) }.to_str() else {
        return ZeroDdsStatus::InvalidUtf8 as c_int;
    };
    let Some((library, path)) = split_profile_url(url) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return ZeroDdsStatus::Error as c_int;
    };
    // DDS-RPC 1.0 §7.10.1: the files are DDS4CCM QoS profiles, which write a
    // profile directly under `<dds>`; DDS-XML 1.0 has it in a `<qos_library>`.
    let Ok(parsed) =
        zerodds_xml::parse_dds_xml_with(&text, &zerodds_xml::ReadOptions::accept_vendor_forms())
    else {
        return ZeroDdsStatus::Error as c_int;
    };
    let document = parsed.value;
    let lib = match library {
        Some(name) => document.qos_libraries.iter().find(|l| l.name == name),
        None => document.qos_libraries.first(),
    };
    let Some(profile) = lib.and_then(|l| l.profiles.first()) else {
        return ZeroDdsStatus::Error as c_int;
    };
    // The `base_name` chain is folded in (DDS-XML §7.3.2.4.2), and of
    // several containers with a `topic_filter` the filterless one is taken.
    let Ok(resolved) = zerodds_xml::resolve_profile(
        &document.qos_libraries,
        &format!("{}::{}", lib.map_or("", |l| l.name.as_str()), profile.name),
    ) else {
        return ZeroDdsStatus::Error as c_int;
    };
    let mut qos = rpc_default();
    if let Some(container) = zerodds_xml::default_qos(&resolved.datawriter_qos) {
        if let Err(status) = apply_container(&mut qos, container, true) {
            return status as c_int;
        }
    }
    if let Some(container) = zerodds_xml::default_qos(&resolved.datareader_qos) {
        if let Err(status) = apply_container(&mut qos, container, false) {
            return status as c_int;
        }
    }
    // SAFETY: `out` is writable per the # Safety doc.
    unsafe { *out = qos };
    ZeroDdsStatus::Ok as c_int
}
