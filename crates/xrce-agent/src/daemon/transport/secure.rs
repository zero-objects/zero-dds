// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The secured listeners of the agent: DTLS over UDP (DDS-XRCE 1.0 §11.2)
//! and TLS over TCP (§11.3).
//!
//! "Any exchange of the clientKey is protected by the security mechanisms
//! provided by the XRCE transport" (§8.3.2.4), and a transport provides "the
//! means for the Client to authenticate the Agent and the means for secure
//! (encrypted and authenticated) message exchange" (§11.1 (5)). A listener
//! here presents the agent's certificate and hands the decrypted messages to
//! the session engine; the replies, and what the ticker has for a client,
//! go back the same way, through the connection's queue
//! ([`Route::Stream`]).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_xrce::Message;

use crate::daemon::dds_port::DdsPort;
use crate::daemon::session::SessionEngine;

use super::{PeerTable, Route, apply_events, next_connection, tick_now};

/// How long a connection may stay without a first complete message before
/// the agent closes it: a peer that opens a connection and never completes
/// the handshake or sends nothing must not hold a thread for ever.
const FIRST_MESSAGE_DEADLINE: Duration = Duration::from_secs(10);

/// Hands one decrypted message to the session engine and queues its replies
/// on `tx`. Returns `false` when the queue is gone.
fn serve_message<P: DdsPort + Send>(
    msg: &Message,
    peer: SocketAddr,
    engine: &Arc<Mutex<SessionEngine<P>>>,
    peers: &Arc<Mutex<PeerTable>>,
    route: &Route,
    tx: &Sender<Message>,
) -> bool {
    let resolved = {
        let Ok(t) = peers.lock() else { return false };
        t.resolve(&msg.header, peer, &msg.submessages)
    };
    let Ok(mut e) = engine.lock() else {
        return false;
    };
    let out = e.handle_message(resolved, &msg.header, &msg.submessages, tick_now());
    apply_events(&mut e, peers, Some(peer), route);
    drop(e);
    for ob in &out {
        let reply = Message {
            header: ob.header,
            submessages: ob.submessages.clone(),
        };
        if tx.send(reply).is_err() {
            return false;
        }
    }
    true
}

#[cfg(feature = "tls")]
pub(crate) use tls::run_tls_listener;

#[cfg(feature = "tls")]
mod tls {
    use super::{
        Arc, AtomicBool, Duration, FIRST_MESSAGE_DEADLINE, Instant, Mutex, Ordering, PeerTable,
        Route, SessionEngine, SocketAddr, next_connection, serve_message, thread,
    };
    use crate::daemon::dds_port::DdsPort;
    use zerodds_xrce::transport_tls::{RustlsServerStream, RustlsTlsServer, XrceTlsStream};

    /// Accepts TLS connections and runs [`run_tls_connection`] for each.
    /// Blocking; call from its own thread.
    pub(crate) fn run_tls_listener<P: DdsPort + Send + 'static>(
        server: RustlsTlsServer,
        engine: Arc<Mutex<SessionEngine<P>>>,
        peers: Arc<Mutex<PeerTable>>,
        stop: Arc<AtomicBool>,
    ) {
        if let Err(e) = server.set_nonblocking(true) {
            eprintln!("[zerodds-xrce-agentd] tls: listener not usable: {e}");
            return;
        }
        while !stop.load(Ordering::Relaxed) {
            match server.try_accept() {
                Ok(Some((stream, peer))) => {
                    let engine = Arc::clone(&engine);
                    let peers = Arc::clone(&peers);
                    let stop = Arc::clone(&stop);
                    thread::spawn(move || run_tls_connection(stream, peer, engine, peers, stop));
                }
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    eprintln!("[zerodds-xrce-agentd] tls: accept failed: {e}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    /// One TLS connection: the handshake happens on the first read; the
    /// messages are framed with the 2-byte little-endian length of §11.3.3
    /// inside the encrypted stream. One thread reads with a short timeout and
    /// writes what its queue holds in between, because the stream cannot be
    /// shared.
    fn run_tls_connection<P: DdsPort + Send>(
        mut stream: RustlsServerStream,
        peer: SocketAddr,
        engine: Arc<Mutex<SessionEngine<P>>>,
        peers: Arc<Mutex<PeerTable>>,
        stop: Arc<AtomicBool>,
    ) {
        if stream
            .set_read_timeout(Some(Duration::from_millis(20)))
            .is_err()
        {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<zerodds_xrce::Message>();
        let connection = next_connection();
        let route = Route::Stream {
            connection,
            tx: tx.clone(),
        };
        let started = Instant::now();
        let mut heard = false;
        'connection: while !stop.load(Ordering::Relaxed) {
            while let Ok(msg) = rx.try_recv() {
                if stream.send_message(&msg).is_err() {
                    eprintln!("[zerodds-xrce-agentd] tls {peer}: write failed, closing");
                    break 'connection;
                }
            }
            match stream.poll_message() {
                Ok(Some(msg)) => {
                    heard = true;
                    if !serve_message(&msg, peer, &engine, &peers, &route, &tx) {
                        break;
                    }
                }
                Ok(None) => {
                    if !heard && started.elapsed() > FIRST_MESSAGE_DEADLINE {
                        eprintln!(
                            "[zerodds-xrce-agentd] tls {peer}: no message after the handshake \
                             deadline, closing"
                        );
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = stream.close();
        if let Ok(mut t) = peers.lock() {
            t.close_connection(connection);
        }
    }
}

#[cfg(feature = "dtls")]
pub(crate) use dtls::run_dtls_listener;

#[cfg(feature = "dtls")]
mod dtls {
    use super::{
        Arc, AtomicBool, Duration, FIRST_MESSAGE_DEADLINE, Instant, Mutex, Ordering, PeerTable,
        Route, SessionEngine, next_connection, serve_message, thread,
    };
    use crate::daemon::dds_port::DdsPort;
    use zerodds_xrce::transport_dtls::{DtlsError, WebrtcDtlsConn, WebrtcDtlsListener};

    /// Accepts DTLS connections and runs [`run_dtls_connection`] for each.
    /// Blocking; call from its own thread.
    pub(crate) fn run_dtls_listener<P: DdsPort + Send + 'static>(
        listener: WebrtcDtlsListener,
        engine: Arc<Mutex<SessionEngine<P>>>,
        peers: Arc<Mutex<PeerTable>>,
        stop: Arc<AtomicBool>,
    ) {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept_timeout(Duration::from_millis(200)) {
                Ok(Some(conn)) => {
                    let engine = Arc::clone(&engine);
                    let peers = Arc::clone(&peers);
                    let stop = Arc::clone(&stop);
                    thread::spawn(move || run_dtls_connection(conn, engine, peers, stop));
                }
                Ok(None) => {}
                Err(DtlsError::Closed) => break,
                Err(e) => eprintln!("[zerodds-xrce-agentd] dtls: {e}"),
            }
        }
    }

    /// One DTLS connection: each decrypted record is one XRCE message
    /// (§11.2.3). A writer thread sends what the connection's queue holds;
    /// this thread reads.
    fn run_dtls_connection<P: DdsPort + Send>(
        conn: WebrtcDtlsConn,
        engine: Arc<Mutex<SessionEngine<P>>>,
        peers: Arc<Mutex<PeerTable>>,
        stop: Arc<AtomicBool>,
    ) {
        let peer = conn.peer_addr();
        let conn = Arc::new(conn);
        let (tx, rx) = std::sync::mpsc::channel::<zerodds_xrce::Message>();
        let writer = {
            let conn = Arc::clone(&conn);
            thread::spawn(move || {
                for msg in rx {
                    match msg.encode() {
                        Ok(bytes) => {
                            if conn.send(&bytes).is_err() {
                                break;
                            }
                        }
                        Err(e) => eprintln!(
                            "[zerodds-xrce-agentd] dtls {peer}: a message of the agent does not \
                             encode: {e}"
                        ),
                    }
                }
            })
        };
        let connection = next_connection();
        let route = Route::Stream {
            connection,
            tx: tx.clone(),
        };
        let started = Instant::now();
        let mut heard = false;
        while !stop.load(Ordering::Relaxed) {
            match conn.recv_timeout(Duration::from_millis(200)) {
                Ok(Some(bytes)) => {
                    let Ok(msg) = zerodds_xrce::Message::decode(&bytes) else {
                        eprintln!(
                            "[zerodds-xrce-agentd] dtls {peer}: dropped {} bytes -- not a valid \
                             XRCE message",
                            bytes.len()
                        );
                        continue;
                    };
                    heard = true;
                    if !serve_message(&msg, peer, &engine, &peers, &route, &tx) {
                        break;
                    }
                }
                Ok(None) => {
                    if !heard && started.elapsed() > FIRST_MESSAGE_DEADLINE {
                        eprintln!(
                            "[zerodds-xrce-agentd] dtls {peer}: no message after the handshake \
                             deadline, closing"
                        );
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        conn.close();
        if let Ok(mut t) = peers.lock() {
            t.close_connection(connection);
        }
        drop(route);
        drop(tx);
        let _ = writer.join();
    }
}
