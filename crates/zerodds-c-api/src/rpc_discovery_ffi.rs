// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The discovery side of the DDS-RPC 1.0 Enhanced Service Mapping (§7.6.2)
//! for the C++ binding.
//!
//! * [`zerodds_writer_set_rpc_discovery`] and
//!   [`zerodds_reader_set_rpc_discovery`] announce the three members of
//!   §7.6.2.1 — `PID_SERVICE_INSTANCE_NAME` (0x0080),
//!   `PID_RELATED_ENTITY_GUID` (0x0081), `PID_TOPIC_ALIASES` (0x0082).
//! * [`zerodds_writer_matched_rpc_readers`] and
//!   [`zerodds_reader_matched_rpc_writers`] read what the matched peers
//!   announced.
//! * [`zerodds_rpc_service_matching_complete`] is the client-side service
//!   matching of §7.6.2.2.2 (Figure 7.4) and
//!   [`zerodds_rpc_client_matching_complete`] the service-side client
//!   matching of §7.6.2.2.1 (Figure 7.3): the per-client mask.

use core::ffi::{c_char, c_int};
use std::ffi::CStr;

use zerodds_dcps::runtime::{RpcDiscoveryExt, RpcPeer};
use zerodds_rpc::discovery_ext::{client_matching_complete, service_matching_complete};

use crate::{ZeroDdsReader, ZeroDdsStatus, ZeroDdsWriter};

/// `string<256>` of §7.6.2.1 plus its NUL. (Literal in the struct below:
/// the C header carries no constants.)
const NAME_CAP: usize = 257;
/// Room for the topic aliases of a peer, `'\n'` between two, and a NUL.
const ALIASES_CAP: usize = 1025;

/// What one matched peer announced (§7.6.2.1).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsRpcPeer {
    /// The peer endpoint's GUID.
    pub key: [u8; 16],
    /// `1` if the peer announced `related_entity_guid`; `0` reads as
    /// `GUID_UNKNOWN`.
    pub has_related: u8,
    /// The related endpoint's GUID.
    pub related: [u8; 16],
    /// `1` if the peer announced `service_instance_name`.
    pub has_instance_name: u8,
    /// The instance name, NUL-terminated.
    pub instance_name: [c_char; 257],
    /// How many topic aliases the peer announced.
    pub alias_count: u32,
    /// The aliases, joined by `'\n'`, NUL-terminated.
    pub aliases: [c_char; 1025],
    /// `1` if `aliases` holds fewer than `alias_count` of them.
    pub aliases_truncated: u8,
    /// `1` if `instance_name` was cut at its capacity: the name the peer
    /// announced is longer. A truncated name must not be compared; the
    /// matching calls work on the full one.
    pub name_truncated: u8,
}

fn copy_text<const N: usize>(dst: &mut [c_char; N], text: &str) -> bool {
    let bytes = text.as_bytes();
    let n = bytes.len().min(N - 1);
    for (slot, byte) in dst.iter_mut().zip(bytes.iter().take(n)) {
        *slot = *byte as c_char;
    }
    dst[n] = 0;
    n == bytes.len()
}

fn to_c_peer(peer: &RpcPeer) -> ZeroDdsRpcPeer {
    let mut out = ZeroDdsRpcPeer {
        key: peer.key,
        has_related: 0,
        related: [0; 16],
        has_instance_name: 0,
        instance_name: [0; NAME_CAP],
        alias_count: 0,
        aliases: [0; ALIASES_CAP],
        aliases_truncated: 0,
        name_truncated: 0,
    };
    if let Some(related) = peer.ext.related_entity_guid {
        out.has_related = 1;
        out.related = related;
    }
    if let Some(name) = peer.ext.service_instance_name.as_deref() {
        out.has_instance_name = 1;
        out.name_truncated = u8::from(!copy_text(&mut out.instance_name, name));
    }
    if let Some(aliases) = peer.ext.topic_aliases.as_deref() {
        out.alias_count = u32::try_from(aliases.len()).unwrap_or(u32::MAX);
        let joined = aliases.join("\n");
        out.aliases_truncated = u8::from(!copy_text(&mut out.aliases, &joined));
    }
    out
}

/// The extension the C arguments stand for. `Err` for a name that is not
/// UTF-8.
///
/// # Safety
/// `instance_name` is NULL or NUL-terminated; `related_guid` is NULL or 16
/// readable octets; `aliases` is NULL or `alias_count` NUL-terminated
/// strings.
unsafe fn read_ext(
    instance_name: *const c_char,
    related_guid: *const u8,
    aliases: *const *const c_char,
    alias_count: usize,
) -> Result<RpcDiscoveryExt, ZeroDdsStatus> {
    let service_instance_name = if instance_name.is_null() {
        None
    } else {
        // SAFETY: NUL-terminated per the contract.
        let text = unsafe { CStr::from_ptr(instance_name) }
            .to_str()
            .map_err(|_| ZeroDdsStatus::InvalidUtf8)?;
        Some(text.to_string())
    };
    let related_entity_guid = if related_guid.is_null() {
        None
    } else {
        let mut guid = [0u8; 16];
        // SAFETY: 16 readable octets per the contract.
        guid.copy_from_slice(unsafe { core::slice::from_raw_parts(related_guid, 16) });
        Some(guid)
    };
    let topic_aliases = if aliases.is_null() {
        None
    } else {
        let mut out = Vec::with_capacity(alias_count);
        for index in 0..alias_count {
            // SAFETY: `alias_count` valid strings per the contract.
            let item = unsafe { *aliases.add(index) };
            if item.is_null() {
                return Err(ZeroDdsStatus::BadParameter);
            }
            // SAFETY: NUL-terminated per the contract.
            let text = unsafe { CStr::from_ptr(item) }
                .to_str()
                .map_err(|_| ZeroDdsStatus::InvalidUtf8)?;
            out.push(text.to_string());
        }
        Some(out)
    };
    Ok(RpcDiscoveryExt {
        service_instance_name,
        related_entity_guid,
        topic_aliases,
    })
}

fn status_of(error: &zerodds_dcps::error::DdsError) -> c_int {
    use zerodds_dcps::error::DdsError;
    match error {
        DdsError::BadParameter { .. } => ZeroDdsStatus::BadParameter as c_int,
        DdsError::PreconditionNotMet { .. } => ZeroDdsStatus::PreconditionNotMet as c_int,
        _ => ZeroDdsStatus::Error as c_int,
    }
}

/// Announces the DDS-RPC discovery extension of a writer (§7.6.2.1.1):
/// `instance_name` (NULL: none; at most 256 octets), `related_guid` (NULL:
/// none; the 16 octets of the related reader's GUID) and `aliases`
/// (NULL: none; `alias_count` strings of at most 256 octets).
///
/// Returns `ZERODDS_OK`; `BadHandle` for a NULL writer; `BadParameter` for
/// a string longer than `string<256>`; `InvalidUtf8`.
///
/// # Safety
/// `writer` from `zerodds_writer_create*`; the other pointers as above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_set_rpc_discovery(
    writer: *mut ZeroDdsWriter,
    instance_name: *const c_char,
    related_guid: *const u8,
    aliases: *const *const c_char,
    alias_count: usize,
) -> c_int {
    if writer.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: per the contract.
    let ext = match unsafe { read_ext(instance_name, related_guid, aliases, alias_count) } {
        Ok(ext) => ext,
        Err(status) => return status as c_int,
    };
    // SAFETY: NULL-checked above.
    let (rt, eid) = unsafe { ((*writer).rt.clone(), (*writer).eid) };
    match rt.set_user_writer_rpc_discovery(eid, &ext) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(error) => status_of(&error),
    }
}

/// The reader half of [`zerodds_writer_set_rpc_discovery`] (§7.6.2.1.2).
///
/// # Safety
/// `reader` from `zerodds_reader_create*`; the other pointers as there.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_rpc_discovery(
    reader: *mut ZeroDdsReader,
    instance_name: *const c_char,
    related_guid: *const u8,
    aliases: *const *const c_char,
    alias_count: usize,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: per the contract.
    let ext = match unsafe { read_ext(instance_name, related_guid, aliases, alias_count) } {
        Ok(ext) => ext,
        Err(status) => return status as c_int,
    };
    // SAFETY: NULL-checked above.
    let (rt, eid) = unsafe { ((*reader).rt.clone(), (*reader).eid) };
    match rt.set_user_reader_rpc_discovery(eid, &ext) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(error) => status_of(&error),
    }
}

/// Copies up to `cap` peers into `out` and stores the total in `*out_count`.
/// The status is `OutOfResources` when the total exceeds `cap` (the first
/// `cap` peers are written), as for the other list calls of
/// `docs/specs/zerodds-c-api-1.0.md` §2.8.
///
/// # Safety
/// `out` points at `cap` writable [`ZeroDdsRpcPeer`] (or is NULL when
/// `cap` is 0); `out_count` is writable.
unsafe fn write_peers(
    peers: &[RpcPeer],
    out: *mut ZeroDdsRpcPeer,
    cap: usize,
    out_count: *mut usize,
) -> c_int {
    for (index, peer) in peers.iter().take(cap).enumerate() {
        // SAFETY: `index < cap` writable slots.
        unsafe { out.add(index).write(to_c_peer(peer)) };
    }
    // SAFETY: writable per the contract.
    unsafe { *out_count = peers.len() };
    if peers.len() > cap {
        ZeroDdsStatus::OutOfResources as c_int
    } else {
        ZeroDdsStatus::Ok as c_int
    }
}

/// The readers a writer is matched with and what each announced
/// (§7.6.2.1.2). `*out_count` is the total; `OutOfResources` when it
/// exceeds `cap`.
///
/// # Safety
/// `writer` valid; `out`/`out_count` as for `write_peers`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_matched_rpc_readers(
    writer: *mut ZeroDdsWriter,
    out: *mut ZeroDdsRpcPeer,
    cap: usize,
    out_count: *mut usize,
) -> c_int {
    if writer.is_null() || out_count.is_null() || (out.is_null() && cap > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-checked above.
    let (rt, eid) = unsafe { ((*writer).rt.clone(), (*writer).eid) };
    let peers = rt.user_writer_matched_rpc_readers(eid);
    // SAFETY: per the contract.
    unsafe { write_peers(&peers, out, cap, out_count) }
}

/// The writers a reader is matched with and what each announced
/// (§7.6.2.1.1). `*out_count` is the total; `OutOfResources` when it
/// exceeds `cap`.
///
/// # Safety
/// `reader` valid; `out`/`out_count` as for `write_peers`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_matched_rpc_writers(
    reader: *mut ZeroDdsReader,
    out: *mut ZeroDdsRpcPeer,
    cap: usize,
    out_count: *mut usize,
) -> c_int {
    if reader.is_null() || out_count.is_null() || (out.is_null() && cap > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-checked above.
    let (rt, eid) = unsafe { ((*reader).rt.clone(), (*reader).eid) };
    let peers = rt.user_reader_matched_rpc_writers(eid);
    // SAFETY: per the contract.
    unsafe { write_peers(&peers, out, cap, out_count) }
}

/// §7.6.2.2.2, Figure 7.4: has the client, whose request writer and reply
/// reader these are, discovered both endpoints of a service instance it
/// may address? `instance_name` (NULL: the empty name) selects the
/// instance by the name its service announced. `accept_unannounced` reads a
/// peer that announced no related endpoint as complete by the DDS match
/// alone. `*out_complete` is `1` or `0`.
///
/// # Safety
/// The handles valid; `instance_name` NULL or NUL-terminated;
/// `out_complete` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_rpc_service_matching_complete(
    request_writer: *mut ZeroDdsWriter,
    reply_reader: *mut ZeroDdsReader,
    instance_name: *const c_char,
    accept_unannounced: u8,
    out_complete: *mut u8,
) -> c_int {
    if request_writer.is_null() || reply_reader.is_null() || out_complete.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let name = if instance_name.is_null() {
        ""
    } else {
        // SAFETY: NUL-terminated per the contract.
        match unsafe { CStr::from_ptr(instance_name) }.to_str() {
            Ok(text) => text,
            Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
        }
    };
    // SAFETY: NULL-checked above.
    let ((wrt, weid), (rrt, reid)) = unsafe {
        (
            ((*request_writer).rt.clone(), (*request_writer).eid),
            ((*reply_reader).rt.clone(), (*reply_reader).eid),
        )
    };
    let readers = wrt.user_writer_matched_rpc_readers(weid);
    let writers = rrt.user_reader_matched_rpc_writers(reid);
    let complete = service_matching_complete(&readers, &writers, name, accept_unannounced != 0);
    // SAFETY: writable per the contract.
    unsafe { *out_complete = u8::from(complete) };
    ZeroDdsStatus::Ok as c_int
}

/// §7.6.2.2.1, Figure 7.3: the mask of one client. Has the service, whose
/// request reader and reply writer these are, discovered both endpoints of
/// the client whose request writer is `client_writer_guid` (16 octets)? A
/// service processes the requests of that client only once this is `1`.
/// A client writer the request reader is not matched with is `0` unless
/// `accept_unannounced` is set.
///
/// # Safety
/// The handles valid; `client_writer_guid` 16 readable octets;
/// `out_complete` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_rpc_client_matching_complete(
    request_reader: *mut ZeroDdsReader,
    reply_writer: *mut ZeroDdsWriter,
    client_writer_guid: *const u8,
    accept_unannounced: u8,
    out_complete: *mut u8,
) -> c_int {
    if request_reader.is_null()
        || reply_writer.is_null()
        || client_writer_guid.is_null()
        || out_complete.is_null()
    {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let mut guid = [0u8; 16];
    // SAFETY: 16 readable octets per the contract.
    guid.copy_from_slice(unsafe { core::slice::from_raw_parts(client_writer_guid, 16) });
    // SAFETY: NULL-checked above.
    let ((rrt, reid), (wrt, weid)) = unsafe {
        (
            ((*request_reader).rt.clone(), (*request_reader).eid),
            ((*reply_writer).rt.clone(), (*reply_writer).eid),
        )
    };
    let writers = rrt.user_reader_matched_rpc_writers(reid);
    let readers = wrt.user_writer_matched_rpc_readers(weid);
    let accept = accept_unannounced != 0;
    let complete = match writers.iter().find(|peer| peer.key == guid) {
        Some(client) => client_matching_complete(client, &readers, accept),
        None => accept,
    };
    // SAFETY: writable per the contract.
    unsafe { *out_complete = u8::from(complete) };
    ZeroDdsStatus::Ok as c_int
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn peer(name: &str, aliases: Option<Vec<String>>) -> RpcPeer {
        RpcPeer {
            key: [7; 16],
            ext: RpcDiscoveryExt {
                service_instance_name: Some(name.to_string()),
                related_entity_guid: None,
                topic_aliases: aliases,
            },
        }
    }

    fn text(buf: &[c_char]) -> String {
        String::from_utf8(
            buf.iter()
                .take_while(|c| **c != 0)
                .map(|c| *c as u8)
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn a_name_that_fits_is_copied_whole() {
        let out = to_c_peer(&peer(&"a".repeat(256), None));
        assert_eq!(out.name_truncated, 0);
        assert_eq!(text(&out.instance_name).len(), 256);
    }

    #[test]
    fn a_name_of_300_octets_is_cut_and_says_so() {
        let out = to_c_peer(&peer(&"b".repeat(300), None));
        assert_eq!(out.has_instance_name, 1);
        assert_eq!(out.name_truncated, 1);
        assert_eq!(text(&out.instance_name).len(), 256);
    }

    #[test]
    fn aliases_past_their_capacity_are_flagged() {
        let aliases: Vec<String> = (0..20).map(|i| format!("{i}{}", "x".repeat(100))).collect();
        let out = to_c_peer(&peer("n", Some(aliases)));
        assert_eq!(out.name_truncated, 0);
        assert_eq!(out.alias_count, 20);
        assert_eq!(out.aliases_truncated, 1);
    }
}
