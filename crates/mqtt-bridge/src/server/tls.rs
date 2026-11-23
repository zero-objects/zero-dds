// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The TLS transport of the broker server: `mqtts`, and `wss` below the
//! WebSocket of MQTT 5.0 §6 (RFC 6455 §3, §4.1 /secure/).
//!
//! The TLS handshake is the one of the bridges
//! (`zerodds_bridge_security::serve_tls_handshake`) with the
//! `rustls::ServerConfig` the caller gives: certificate, key, ALPN and,
//! for client certificates, its verifier.
//!
//! A connection has two threads, the reader and the writer
//! ([`super::conn`]), and one TLS state. The reader waits for ciphertext
//! without a lock and takes the TLS state only to decrypt; the writer
//! encrypts under the TLS state and writes the records under a second
//! lock, the one of the socket's sending side. Every record leaves the TLS
//! state under that second lock, so records go out in the order of their
//! sequence numbers, and a writer blocked by a client that does not read
//! does not keep the reader from decrypting.

use std::boxed::Box;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use std::vec::Vec;

use rustls::ServerConnection;

use super::{ClientStream, ClientWriter};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The TLS handshake on `tcp` with `cfg`; each read of it waits at most
/// `timeout`.
///
/// # Errors
/// I/O errors of the connection and a failed handshake.
pub(super) fn accept(
    cfg: Arc<rustls::ServerConfig>,
    tcp: TcpStream,
    timeout: Duration,
) -> io::Result<TlsStream> {
    let (tcp, conn, _subject) = zerodds_bridge_security::serve_tls_handshake(cfg, tcp, timeout)?;
    tcp.set_read_timeout(None)?;
    tcp.set_write_timeout(None)?;
    let send = tcp.try_clone()?;
    Ok(TlsStream {
        tcp,
        shared: Arc::new(Shared {
            send: Mutex::new(send),
            tls: Mutex::new(conn),
            notified: AtomicBool::new(false),
        }),
    })
}

struct Shared {
    /// The sending side of the socket; records leave the TLS state only
    /// while it is held.
    send: Mutex<TcpStream>,
    tls: Mutex<ServerConnection>,
    /// The close_notify alert was queued.
    notified: AtomicBool,
}

impl Shared {
    /// Writes the records the TLS state holds.
    fn send_records(&self, tcp: &mut TcpStream) -> io::Result<()> {
        let records = take_records(&mut lock(&self.tls))?;
        tcp.write_all(&records)
    }

    /// The close_notify alert (once), then the shutdown `how`.
    fn end(&self, how: Shutdown) {
        let mut tcp = lock(&self.send);
        if !self.notified.swap(true, Ordering::SeqCst) {
            lock(&self.tls).send_close_notify();
        }
        // The peer may have closed already; there is nothing left to tell
        // it then.
        let _ = self.send_records(&mut tcp);
        let _ = tcp.shutdown(how);
    }
}

/// The records the TLS state holds for sending.
fn take_records(tls: &mut ServerConnection) -> io::Result<Vec<u8>> {
    let mut records = Vec::new();
    while tls.wants_write() {
        tls.write_tls(&mut records)?;
    }
    Ok(records)
}

/// The MQTT byte stream of a TLS connection the server accepted.
pub struct TlsStream {
    tcp: TcpStream,
    shared: Arc<Shared>,
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match lock(&self.shared.tls).reader().read(buf) {
                // 0: the client's close_notify.
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            // At most 16 KiB of ciphertext while no plaintext is buffered:
            // what it decrypts to stays within the plaintext limit of the
            // TLS state.
            let mut raw = [0u8; 16 * 1024];
            let n = self.tcp.read(&mut raw)?;
            if n == 0 {
                return Ok(0);
            }
            let (failed, wants_write) = {
                let mut tls = lock(&self.shared.tls);
                let mut rest = &raw[..n];
                let mut failed = None;
                while !rest.is_empty() && failed.is_none() {
                    tls.read_tls(&mut rest)?;
                    failed = tls.process_new_packets().err();
                }
                (failed, tls.wants_write())
            };
            // An alert for the error, or the answer to a key update.
            if wants_write {
                let mut tcp = lock(&self.shared.send);
                let sent = self.shared.send_records(&mut tcp);
                if failed.is_none() {
                    sent?;
                }
            }
            if let Some(e) = failed {
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        }
    }
}

impl ClientStream for TlsStream {
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>> {
        Ok(Box::new(TlsWriter {
            shared: Arc::clone(&self.shared),
        }))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.tcp.set_read_timeout(timeout)
    }
}

/// The writing side of a [`TlsStream`].
struct TlsWriter {
    shared: Arc<Shared>,
}

impl Write for TlsWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut tcp = lock(&self.shared.send);
        let records = {
            let mut tls = lock(&self.shared.tls);
            let mut records = Vec::new();
            let mut rest = buf;
            while !rest.is_empty() {
                let n = tls.writer().write(rest)?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "the TLS connection takes no more data",
                    ));
                }
                rest = &rest[n..];
                records.extend(take_records(&mut tls)?);
            }
            records
        };
        tcp.write_all(&records)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        lock(&self.shared.send).flush()
    }
}

impl ClientWriter for TlsWriter {
    fn close(&mut self) {
        self.shared.end(Shutdown::Both);
    }

    fn close_write(&mut self) {
        self.shared.end(Shutdown::Write);
    }
}
