//! TCP side-channel server (R-026 + R-022 + R-100..R-104).
//!
//! Phase-1 implementation: plain TCP with a **cert-fingerprint handshake**
//! instead of full mTLS. The client sends its cert PEM as a handshake frame,
//! the server checks the SHA-256 fingerprint against the loaded
//! `cert.d` content. Frames are streamed as length-prefixed JSON.
//!
//! Phase-2 (a separate iteration step): full mTLS via
//! rustls — cert-fingerprint matching is sufficient for Phase-1 in the
//! defense air-gap environment where the side channel runs over a
//! curated maintenance LAN anyway.
//!
//! Wire format:
//!
//! ```text
//! [u32 BE length][JSON frame]
//! ```
//!
//! This is human-readable enough for bring-up; production can upgrade to
//! `bincode` or its own binary schema.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};

use crate::auth::CertSet;
use crate::frame::Frame;
use crate::tap::TapHook;

/// Configuration of a server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Bind address, e.g. `127.0.0.1:9555`.
    pub bind: String,
    /// Loaded `cert.d` content.
    pub cert_set: CertSet,
}

/// Running server. Drop stops it.
pub struct InspectServer {
    /// Shared client pool — every new connection registers itself.
    clients: Arc<Mutex<Vec<TcpStream>>>,
    accept_thread: Option<JoinHandle<()>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl InspectServer {
    /// Starts the server, accepts connections in a background thread.
    ///
    /// # Errors
    ///
    /// Fails if the bind address cannot be opened.
    pub fn start(cfg: ServerConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(&cfg.bind)?;
        listener.set_nonblocking(true)?;
        let clients: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cert_set = Arc::new(cfg.cert_set);

        let accept_thread = {
            let clients = Arc::clone(&clients);
            let shutdown = Arc::clone(&shutdown);
            let cert_set = Arc::clone(&cert_set);
            std::thread::spawn(move || {
                loop {
                    if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            // Accepted sockets inherit the listener's
                            // non-blocking flag on BSD/macOS. The handshake read
                            // (`authenticate_client` → `read_exact` with
                            // `set_read_timeout`) needs a BLOCKING
                            // socket, though — `set_read_timeout` only works there; on a
                            // non-blocking socket `read_exact` immediately returns
                            // `WouldBlock` if the client bytes have not yet
                            // arrived → sporadically rejected clients.
                            // So switch to blocking BEFORE authentication.
                            if stream.set_nonblocking(false).is_ok()
                                && authenticate_client(&stream, &cert_set).is_ok()
                            {
                                if let Ok(mut clients) = clients.lock() {
                                    clients.push(stream);
                                }
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        Err(_) => break,
                    }
                }
            })
        };

        Ok(Self {
            clients,
            accept_thread: Some(accept_thread),
            shutdown,
        })
    }

    /// Returns a `TapHook` that streams frames to all connected clients.
    /// The hook can then be registered via `tap::register_*_tap`.
    #[must_use]
    pub fn broadcast_hook(&self) -> Box<BroadcastHook> {
        Box::new(BroadcastHook {
            clients: Arc::clone(&self.clients),
        })
    }

    /// Number of currently connected clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Stops the accept loop.
    pub fn shutdown(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for InspectServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Tap hook that writes frames as length-prefixed JSON to all clients.
pub struct BroadcastHook {
    clients: Arc<Mutex<Vec<TcpStream>>>,
}

impl TapHook for BroadcastHook {
    fn on_frame(&self, frame: &Frame) {
        let Ok(json) = serde_json::to_vec(frame) else {
            return;
        };
        let Ok(len) = u32::try_from(json.len()) else {
            return;
        };
        let prefix = len.to_be_bytes();

        let Ok(mut clients) = self.clients.lock() else {
            return;
        };
        // Best-effort write; drop disconnected clients silently.
        let mut alive: Vec<TcpStream> = Vec::with_capacity(clients.len());
        for mut stream in clients.drain(..) {
            if stream.write_all(&prefix).is_ok() && stream.write_all(&json).is_ok() {
                alive.push(stream);
            }
        }
        *clients = alive;
    }
}

fn authenticate_client(stream: &TcpStream, cert_set: &CertSet) -> std::io::Result<()> {
    let mut stream = stream.try_clone().map_err(std::io::Error::other)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let cert_len = u32::from_be_bytes(len_buf) as usize;
    if cert_len > 64 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "cert too large",
        ));
    }
    let mut cert_buf = vec![0u8; cert_len];
    stream.read_exact(&mut cert_buf)?;
    let mut hasher = Sha256::new();
    hasher.update(&cert_buf);
    let fingerprint: [u8; 32] = hasher.finalize().into();
    if cert_set.contains_fingerprint(&fingerprint) {
        // Acknowledge handshake
        stream.write_all(b"\x00\x00\x00\x02OK")?;
        Ok(())
    } else {
        let _ = stream.write_all(b"\x00\x00\x00\x07REJECT!");
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "cert fingerprint not in cert.d",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::auth::{CertSet, LoadedCert};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Serializes the TCP server tests — under `cargo test` without
    /// `--test-threads=1`, race conditions can occur on
    /// ephemeral ports + tap-registry state. A lock at the
    /// start of each test guarantees sequential execution.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn pick_unused_port() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let port = l.local_addr().expect("local_addr").port();
        drop(l);
        port
    }

    fn cert_set_with(pem: &[u8]) -> CertSet {
        let mut hasher = Sha256::new();
        hasher.update(pem);
        let fp: [u8; 32] = hasher.finalize().into();
        CertSet {
            certs: vec![LoadedCert {
                source: PathBuf::from("test.pem"),
                fingerprint: fp,
                pem: pem.to_vec(),
            }],
        }
    }

    #[test]
    fn server_starts_and_shuts_down() {
        let _guard = TEST_LOCK.lock();
        let port = pick_unused_port();
        let cfg = ServerConfig {
            bind: format!("127.0.0.1:{port}"),
            cert_set: cert_set_with(b"--cert--"),
        };
        let mut server = InspectServer::start(cfg).expect("start");
        assert_eq!(server.client_count(), 0);
        server.shutdown();
    }

    #[test]
    fn unauthenticated_client_is_rejected() {
        let _guard = TEST_LOCK.lock();
        let port = pick_unused_port();
        let cfg = ServerConfig {
            bind: format!("127.0.0.1:{port}"),
            cert_set: cert_set_with(b"--cert--"),
        };
        let server = InspectServer::start(cfg).expect("start");

        // Client with a wrong cert
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        let bad = b"BAD-CERT";
        let len = (bad.len() as u32).to_be_bytes();
        stream.write_all(&len).expect("write len");
        stream.write_all(bad).expect("write cert");
        let mut resp = [0u8; 11];
        let _ = stream.read_exact(&mut resp);
        assert!(resp.starts_with(&[0, 0, 0, 7]) || resp[..7] == *b"\x00\x00\x00\x07R");

        // The server did not accept the client
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(server.client_count(), 0);
    }

    #[test]
    fn authenticated_client_receives_frames() {
        let _guard = TEST_LOCK.lock();
        let pem = b"--good-cert-pem--";
        let port = pick_unused_port();
        let cfg = ServerConfig {
            bind: format!("127.0.0.1:{port}"),
            cert_set: cert_set_with(pem),
        };
        let server = InspectServer::start(cfg).expect("start");

        // Client with the correct cert
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        let len = (pem.len() as u32).to_be_bytes();
        stream.write_all(&len).expect("write len");
        stream.write_all(pem).expect("write cert");
        let mut ok = [0u8; 6];
        stream.read_exact(&mut ok).expect("ok handshake");
        assert_eq!(&ok[..6], b"\x00\x00\x00\x02OK");

        // Wait until the server has registered the client
        for _ in 0..20 {
            if server.client_count() >= 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(server.client_count(), 1);

        // Send a frame via the hook
        let hook = server.broadcast_hook();
        let frame = Frame::dcps("topic-1".into(), 100, 42, vec![1, 2, 3]);
        hook.on_frame(&frame);

        // Client liest den Frame
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).expect("read len");
        let n = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; n];
        stream.read_exact(&mut payload).expect("read frame");
        let received: Frame = serde_json::from_slice(&payload).expect("parse json");
        assert_eq!(received.topic, "topic-1");
        assert_eq!(received.payload, vec![1, 2, 3]);
    }
}
