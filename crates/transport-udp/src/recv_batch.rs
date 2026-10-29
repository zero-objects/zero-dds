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
//! The `mmsghdr` and `iovec` arrays and the receive buffers they point
//! into are kept per thread across calls; each call links the pointers
//! afresh and calls `libc::recvmmsg`. All pointers live for the duration
//! of the call, because nothing touches the storage while it runs.

#![cfg(all(feature = "std", feature = "recvmmsg-batch", target_os = "linux"))]
// Feature-gated unsafe island for the libc::recvmmsg FFI. Each unsafe
// block has a `// SAFETY:` comment (see the block body).
#![allow(unsafe_code)]

use std::io;
use std::mem::MaybeUninit;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::AsRawFd;
use std::sync::Arc;

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
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        recv_batch_with(socket, out, batch, &mut scratch)
    })
}

/// What one `recvmmsg` call needs besides the socket: the receive buffers
/// and the header arrays that point into them, kept by the receiving
/// thread across calls.
///
/// They used to be built for each call: four lists, two of them past the
/// size the allocator's per-thread cache serves (the 32 `sockaddr_storage`
/// are 4 kB, zeroed), and buffers from a pool behind a lock. Every call
/// went to the receiving thread's malloc arena, and the arena's lock is
/// also the lock another thread takes to free what this one allocated —
/// the datagrams it hands on. Measured on the Linux test host with a
/// subscriber taking 800 000 samples a second, the receive thread waited
/// on that lock about 1 100 times in 4 s in this function.
#[derive(Default)]
struct Scratch {
    buffers: Vec<Box<[u8; MAX_DATAGRAM_SIZE]>>,
    sockaddrs: Vec<MaybeUninit<libc::sockaddr_storage>>,
    iovecs: Vec<libc::iovec>,
    msgs: Vec<libc::mmsghdr>,
}

std::thread_local! {
    static SCRATCH: core::cell::RefCell<Scratch> = core::cell::RefCell::new(Scratch::default());
}

fn recv_batch_with(
    socket: &UdpSocket,
    out: &mut Vec<ReceivedDatagram>,
    batch: usize,
    scratch: &mut Scratch,
) -> Result<usize, RecvError> {
    let Scratch {
        buffers,
        sockaddrs,
        iovecs,
        msgs,
    } = scratch;
    // The buffers are not cleared between calls: `recvmmsg` overwrites
    // them, and only the `msg_len` octets it reports are read.
    while buffers.len() < batch {
        buffers.push(Box::new([0u8; MAX_DATAGRAM_SIZE]));
    }
    // Zeroed once, so that reading one is reading initialized memory; the
    // kernel writes the source address of each datagram it returns.
    if sockaddrs.len() < batch {
        sockaddrs.resize(batch, MaybeUninit::<libc::sockaddr_storage>::zeroed());
    }
    iovecs.clear();
    iovecs.extend(buffers[..batch].iter_mut().map(|b| libc::iovec {
        iov_base: b.as_mut_ptr().cast::<libc::c_void>(),
        iov_len: MAX_DATAGRAM_SIZE,
    }));
    msgs.clear();
    msgs.extend(
        sockaddrs[..batch]
            .iter_mut()
            .zip(iovecs.iter_mut())
            .map(|(addr, iov)| {
                // SAFETY: `mmsghdr` is POD; all-zero is a header without
                // control data, flags and length. It is filled field by
                // field: musl's `msghdr` has private padding fields.
                let mut msg: libc::mmsghdr = unsafe { core::mem::zeroed() };
                msg.msg_hdr.msg_name = addr.as_mut_ptr().cast::<libc::c_void>();
                msg.msg_hdr.msg_namelen =
                    core::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
                msg.msg_hdr.msg_iov = iov as *mut libc::iovec;
                msg.msg_hdr.msg_iovlen = 1;
                msg
            }),
    );

    // SAFETY: socket.as_raw_fd is a valid Linux fd;
    // msgs[..] is a valid &mut [mmsghdr] with batch-many entries;
    // all iovec/msghdr pointers point to live buffers
    // (buffers/sockaddrs/iovecs of `scratch`) that are neither moved nor
    // dropped while the call runs. `flags` is `c_int` in glibc's
    // `recvmmsg` and `c_uint` in musl's.
    let n = unsafe {
        libc::recvmmsg(
            socket.as_raw_fd(),
            msgs.as_mut_ptr(),
            batch as libc::c_uint,
            libc::MSG_DONTWAIT as _,
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
    Ok(n)
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

    /// The receive buffers stay with the thread: neither an empty poll —
    /// the common answer, as the call runs with `MSG_DONTWAIT` — nor a
    /// receive gives them up, so no call allocates them again. An empty
    /// poll that dropped them once cost the next call 2 MiB to allocate and
    /// zero; measured with `perf`, that memset was 27 % of the receive
    /// thread's cycles.
    #[test]
    fn the_receive_buffers_outlive_the_call() {
        let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let rx_loc = <UdpTransport as Transport>::local_locator(&rx);
        let socket = rx.std_socket();
        socket.set_nonblocking(true).expect("nonblocking");
        let first_buffer =
            || SCRATCH.with(|cell| cell.borrow().buffers.first().map(|b| b.as_ptr()));
        let mut out = Vec::new();
        assert_eq!(
            recv_batch_linux(socket, &mut out, 8).expect("empty poll"),
            0,
            "nothing was sent, so nothing is received"
        );
        let after_empty_poll = first_buffer().expect("the buffers are kept");
        let tx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        tx.send(&rx_loc, &[1, 2, 3, 0xAB]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(recv_batch_linux(socket, &mut out, 8).expect("receive"), 1);
        assert_eq!(&*out[0].data, &[1, 2, 3, 0xAB]);
        assert_eq!(
            first_buffer(),
            Some(after_empty_poll),
            "a receive reuses the buffers of the call before it"
        );
        assert_eq!(
            recv_batch_linux(socket, &mut out, 8).expect("empty poll"),
            0
        );
        assert_eq!(first_buffer(), Some(after_empty_poll));
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
