// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Windows: the wait of a read of an accepted connection
//! (`tcp_transport::UntilClosed`). `WSAPoll` is the only `unsafe` code
//! here, with its SAFETY comments.

#![allow(unsafe_code)]

use std::net::TcpStream;
use std::os::windows::io::AsRawSocket;
use std::time::Duration;

use windows_sys::Win32::Networking::WinSock::{
    POLLRDNORM, SOCKET, SOCKET_ERROR, WSAGetLastError, WSAPOLLFD, WSAPoll,
};

/// Waits until `stream` has octets to read, has ended or has failed, at
/// most `timeout`; `false` when the time ran out.
pub(crate) fn wait_readable(stream: &TcpStream, timeout: Duration) -> std::io::Result<bool> {
    let handle = SOCKET::try_from(stream.as_raw_socket()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "socket handle out of range",
        )
    })?;
    let mut fd = WSAPOLLFD {
        fd: handle,
        events: POLLRDNORM,
        revents: 0,
    };
    let ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `fd` is one valid `WSAPOLLFD` that lives for the call, and
    // the count passed is 1.
    let n = unsafe { WSAPoll(&mut fd, 1, ms) };
    if n == SOCKET_ERROR {
        // SAFETY: no precondition; it reads the calling thread's last
        // Winsock error.
        return Err(std::io::Error::from_raw_os_error(unsafe {
            WSAGetLastError()
        }));
    }
    Ok(n > 0)
}
