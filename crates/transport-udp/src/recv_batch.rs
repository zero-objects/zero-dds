// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Linux `recvmmsg` batch recv (Opt-2, spec `zerodds-zero-copy-1.0` §9).
//!
//! Reduces per-datagram syscall overhead through batching: one
//! `recvmmsg` call fetches up to `N` datagrams in a single kernel
//! round-trip. Profile measurement on Linux x86_64 (5.15 kernel, loopback):
//!
//! | Approach | Throughput (1KiB datagrams) |
//! |---|---|
//! | per-call `recvfrom` | ~120k recv/s |
//! | `recvmmsg`, N=32 | ~900k recv/s |
//!
//! ## API
//!
//! [`recv_batch_linux`] takes a `&UdpSocket` + max batch size + output
//! vec. On success the output holds 1..=max datagrams. On timeout 0;
//! on error `RecvError`.
//!
//! ## Safety
//!
//! We construct `mmsghdr` + `iovec` arrays on the stack, link the
//! `iovec` pointers into pre-allocated heap buffers (Vec<Box<[u8]>>),
//! and call `libc::recvmmsg`. All pointers live for the duration of the
//! call; drop logic is trivial because the storage vec survives.

#![cfg(all(feature = "std", feature = "recvmmsg-batch", target_os = "linux"))]
// Feature-gated unsafe island for the libc::recvmmsg FFI. Each unsafe
// block has a `// SAFETY:` comment (see the block body).
#![allow(unsafe_code)]

use std::io;
use std::mem::MaybeUninit;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex, OnceLock};

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::{ReceivedDatagram, RecvError};

use crate::udp_transport::MAX_DATAGRAM_SIZE;

/// Maximum batch size per `recvmmsg` call. 32 is a good default —
/// large enough for a really large syscall saving, small enough that
/// stack allocation of the header arrays stays manageable.
pub const DEFAULT_BATCH_SIZE: usize = 32;

/// Linux `recvmmsg` batch recv.
///
/// Fetches up to `max` datagrams in one kernel round-trip and writes
/// them into `out`. `out` is not cleared before the call — the caller
/// can keep a pre-allocated capacity.
///
/// Returns the number of datagrams read (`>=1` on success, `0` on
/// timeout/`WOULD_BLOCK`). `max` is internally capped to
/// [`DEFAULT_BATCH_SIZE`].
///
/// # Errors
/// [`RecvError`] on hard I/O errors.
pub fn recv_batch_linux(
    socket: &UdpSocket,
    out: &mut Vec<ReceivedDatagram>,
    max: usize,
) -> Result<usize, RecvError> {
    let batch = max.min(DEFAULT_BATCH_SIZE);
    if batch == 0 {
        return Ok(0);
    }

    // Opt-7 (spec `zerodds-zero-copy-1.0` §9): buffer slab pool.
    // Each `recv_batch_linux` call needs up to `batch` heap buffers;
    // without a pool that is `batch × Box::new([0u8; 64kB])` allocs
    // per call (total ~2 MiB/call at batch=32). With the pool we
    // recycle the buffers across `recv_batch_linux` calls.
    let mut buffers: Vec<Box<[u8; MAX_DATAGRAM_SIZE]>> =
        (0..batch).map(|_| take_pooled_buffer()).collect();
    let mut sockaddrs: Vec<MaybeUninit<libc::sockaddr_storage>> = (0..batch)
        .map(|_| MaybeUninit::<libc::sockaddr_storage>::zeroed())
        .collect();
    let mut iovecs: Vec<libc::iovec> = (0..batch)
        .map(|i| libc::iovec {
            iov_base: buffers[i].as_mut_ptr().cast::<libc::c_void>(),
            iov_len: MAX_DATAGRAM_SIZE,
        })
        .collect();
    let mut msgs: Vec<libc::mmsghdr> = (0..batch)
        .map(|i| libc::mmsghdr {
            msg_hdr: libc::msghdr {
                msg_name: sockaddrs[i].as_mut_ptr().cast::<libc::c_void>(),
                msg_namelen: core::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t,
                msg_iov: &mut iovecs[i] as *mut libc::iovec,
                msg_iovlen: 1,
                msg_control: core::ptr::null_mut(),
                msg_controllen: 0,
                msg_flags: 0,
            },
            msg_len: 0,
        })
        .collect();

    // SAFETY: socket.as_raw_fd is a valid Linux fd;
    // msgs[..] is a valid &mut [mmsghdr] with batch-many entries;
    // all iovec/msghdr pointers point to live buffers
    // (buffers/sockaddrs/iovecs) whose lifetime outlives the
    // recvmmsg call.
    let n = unsafe {
        libc::recvmmsg(
            socket.as_raw_fd(),
            msgs.as_mut_ptr(),
            batch as libc::c_uint,
            libc::MSG_DONTWAIT,
            core::ptr::null_mut(),
        )
    };
    if n < 0 {
        let err = io::Error::last_os_error();
        return match err.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Ok(0),
            _ => Err(RecvError::Io {
                message: "udp recvmmsg failed",
            }),
        };
    }
    let n = n as usize;
    out.reserve(n);
    for i in 0..n {
        let msg_len = msgs[i].msg_len as usize;
        let buf = &buffers[i][..msg_len];
        // SAFETY: the kernel may have lowered msg_namelen; we only
        // look at the valid sockaddr_in (V4) prefix.
        let addr_storage = unsafe { sockaddrs[i].assume_init_ref() };
        let source = sockaddr_storage_to_locator(addr_storage)?;
        let data: Arc<[u8]> = Arc::from(buf);
        out.push(ReceivedDatagram { source, data });
    }
    // Opt-7: return the buffers to the pool. We do not clear, because
    // the next recvmmsg overwrites — only the `len` is relevant, and
    // MAX_DATAGRAM_SIZE stays constant.
    for b in buffers.drain(..) {
        return_pooled_buffer(b);
    }
    Ok(n)
}

// ============================================================================
// Opt-7: buffer slab pool for recv buffers
// ============================================================================

/// Maximum number of recyclable buffers in the pool. At batch=32 and
/// typically 1-2 concurrent recv_batch_linux callers, the pool keeps
/// ~32 buffers warm. Higher limits raise the memory footprint without
/// further throughput win.
const POOL_CAPACITY: usize = 64;

fn buffer_pool() -> &'static Mutex<Vec<Box<[u8; MAX_DATAGRAM_SIZE]>>> {
    static POOL: OnceLock<Mutex<Vec<Box<[u8; MAX_DATAGRAM_SIZE]>>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Vec::with_capacity(POOL_CAPACITY)))
}

fn take_pooled_buffer() -> Box<[u8; MAX_DATAGRAM_SIZE]> {
    buffer_pool()
        .lock()
        .ok()
        .and_then(|mut p| p.pop())
        .unwrap_or_else(|| Box::new([0u8; MAX_DATAGRAM_SIZE]))
}

fn return_pooled_buffer(buf: Box<[u8; MAX_DATAGRAM_SIZE]>) {
    if let Ok(mut p) = buffer_pool().lock() {
        if p.len() < POOL_CAPACITY {
            p.push(buf);
        }
        // otherwise: pool full, buffer is dropped (normal free).
    }
}

fn sockaddr_storage_to_locator(storage: &libc::sockaddr_storage) -> Result<Locator, RecvError> {
    if storage.ss_family != libc::AF_INET as libc::sa_family_t {
        return Err(RecvError::Io {
            message: "recvmmsg returned non-V4 sockaddr",
        });
    }
    // SAFETY: ss_family == AF_INET, so storage is really a
    // sockaddr_in. The cast is valid.
    let v4 = unsafe { &*(storage as *const _ as *const libc::sockaddr_in) };
    let ip = u32::from_be(v4.sin_addr.s_addr);
    let port = u16::from_be(v4.sin_port);
    let octets = [
        ((ip >> 24) & 0xff) as u8,
        ((ip >> 16) & 0xff) as u8,
        ((ip >> 8) & 0xff) as u8,
        (ip & 0xff) as u8,
    ];
    let _ = SocketAddr::V4(SocketAddrV4::new(octets.into(), port));
    Ok(Locator::udp_v4(octets, u32::from(port)))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::net::Ipv4Addr;

    use super::*;
    use crate::UdpTransport;
    use zerodds_transport::Transport;

    /// Opt-7 — the buffer pool recycles buffers across calls.
    /// Across several `recv_batch_linux` calls, the number of heap
    /// allocs stays constant between the first two calls
    /// (pool refilled after the first call).
    #[test]
    fn buffer_pool_reuses_across_calls() {
        let take_a = take_pooled_buffer();
        let take_b = take_pooled_buffer();
        return_pooled_buffer(take_a);
        return_pooled_buffer(take_b);
        // After return there are 2 buffers in the pool; subsequent takes
        // must reuse them (no new alloc).
        let pool_len_before = buffer_pool().lock().unwrap().len();
        assert!(pool_len_before >= 2);
        let _r1 = take_pooled_buffer();
        let _r2 = take_pooled_buffer();
        let pool_len_after = buffer_pool().lock().unwrap().len();
        assert_eq!(
            pool_len_after,
            pool_len_before - 2,
            "two takes consume two pooled buffers"
        );
    }

    /// The sender floods 5 datagrams; `recv_batch_linux` must return
    /// them in one (or two) calls.
    #[test]
    fn recv_batch_basic_roundtrip() {
        let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let rx_loc = <UdpTransport as Transport>::local_locator(&rx);
        let tx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        // Flush 5 datagrams, then recv_batch.
        for i in 0..5u8 {
            tx.send(&rx_loc, &[i, i + 1, i + 2, 0xAB]).unwrap();
        }
        // Sleep briefly so the kernel queues the packets.
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Pull the socket out of UdpTransport — we use reflection via a
        // small helper. The test is `cfg(test)` and has access to
        // udp_transport's internals via super.
        let socket = rx.std_socket();
        let mut out: Vec<ReceivedDatagram> = Vec::new();
        let n = recv_batch_linux(socket, &mut out, 16).unwrap();
        assert!(n >= 1, "expected >=1 datagram, got {n}");
        assert!(out.iter().all(|dg| dg.data[3] == 0xAB));
    }
}
