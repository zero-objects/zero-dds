//! TCP-Side-Channel-Server (R-026 + R-022 + R-100..R-104).
//!
//! Phase-1-Implementation: plain TCP mit **Cert-Fingerprint-Handshake**
//! statt vollem mTLS. Client sendet seine Cert-PEM als Handshake-Frame,
//! Server prueft den SHA-256-Fingerprint gegen den geladenen
//! `cert.d`-Inhalt. Frames werden als length-prefixed JSON gestreamt.
//!
//! Phase-2 (separater Iterations-Step): vollstaendiges mTLS via
//! rustls — Cert-Fingerprint-Match ist Phase-1 ausreichend fuer die
//! Defense-Air-gap-Umgebung wo der Side-Channel anyway ueber ein
//! kuratiertes Wartungs-LAN laueft.
//!
//! Wire-Format:
//!
//! ```text
//! [u32 BE length][JSON-Frame]
//! ```
//!
//! Das ist menschen-lesbar genug fuer Bring-Up; Production kann auf
//! `bincode` oder eigenes Binary-Schema upgraden.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};

use crate::auth::CertSet;
use crate::frame::Frame;
use crate::tap::TapHook;

/// Konfiguration eines Servers.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Bind-Adresse, z.B. `127.0.0.1:9555`.
    pub bind: String,
    /// Geladener `cert.d`-Inhalt.
    pub cert_set: CertSet,
}

/// Laufender Server. Drop stoppt ihn.
pub struct InspectServer {
    /// Geteilter Client-Pool — jede neue Connection registriert sich.
    clients: Arc<Mutex<Vec<TcpStream>>>,
    accept_thread: Option<JoinHandle<()>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl InspectServer {
    /// Startet den Server, akzeptiert Connections in einem Hintergrund-Thread.
    ///
    /// # Errors
    ///
    /// Schlaegt fehl wenn die Bind-Adresse nicht offen werden kann.
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
                            if authenticate_client(&stream, &cert_set).is_ok() {
                                if let Ok(mut clients) = clients.lock() {
                                    if stream.set_nonblocking(false).is_ok() {
                                        clients.push(stream);
                                    }
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

    /// Liefert einen `TapHook` der Frames an alle connected Clients
    /// streamt. Der Hook kann dann via `tap::register_*_tap`
    /// registriert werden.
    #[must_use]
    pub fn broadcast_hook(&self) -> Box<BroadcastHook> {
        Box::new(BroadcastHook {
            clients: Arc::clone(&self.clients),
        })
    }

    /// Anzahl aktuell connected Clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Stoppt den Accept-Loop.
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

/// Tap-Hook der Frames als length-prefixed JSON an alle Clients schreibt.
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

    /// Serialisiert die TCP-Server-Tests — bei `cargo test` ohne
    /// `--test-threads=1` koennen Race-Conditions auf
    /// ephemeral Ports + Tap-Registry-State auftreten. Lock am
    /// Start jedes Tests garantiert sequentielle Ausfuehrung.
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

        // Client mit falschem Cert
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        let bad = b"BAD-CERT";
        let len = (bad.len() as u32).to_be_bytes();
        stream.write_all(&len).expect("write len");
        stream.write_all(bad).expect("write cert");
        let mut resp = [0u8; 11];
        let _ = stream.read_exact(&mut resp);
        assert!(resp.starts_with(&[0, 0, 0, 7]) || resp[..7] == *b"\x00\x00\x00\x07R");

        // Server hat den Client nicht akzeptiert
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

        // Client mit korrektem Cert
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        let len = (pem.len() as u32).to_be_bytes();
        stream.write_all(&len).expect("write len");
        stream.write_all(pem).expect("write cert");
        let mut ok = [0u8; 6];
        stream.read_exact(&mut ok).expect("ok handshake");
        assert_eq!(&ok[..6], b"\x00\x00\x00\x02OK");

        // Warte bis der Server den Client registriert hat
        for _ in 0..20 {
            if server.client_count() >= 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(server.client_count(), 1);

        // Sende einen Frame ueber den Hook
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
