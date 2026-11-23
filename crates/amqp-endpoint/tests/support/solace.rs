// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A Solace PubSub+ Standard broker for the broker matrix
//! (`zerodds-amqp-bridge-daemon-1.0` §10). Solace ships the broker as a
//! container image only, so the test does not start it: a CI service
//! container or a container started by hand, named by environment
//! variables:
//!
//! * `ZERODDS_SOLACE`: the host name of the broker (a CI service alias, or
//!   `127.0.0.1`);
//! * `ZERODDS_SOLACE_AMQP_PORT` (default 5672), `ZERODDS_SOLACE_AMQPS_PORT`
//!   (default 5671), `ZERODDS_SOLACE_SEMP_PORT` (default 8080).
//!
//! ```bash
//! docker run -d --rm --name solace --shm-size=1g --ulimit nofile=2448:1048576 \
//!   -p 127.0.0.1:5672:5672 -p 127.0.0.1:5671:5671 -p 127.0.0.1:8080:8080 \
//!   -e username_admin_globalaccesslevel=admin -e username_admin_password=admin \
//!   solace/solace-pubsub-standard:10.26.0.9219
//! ZERODDS_SOLACE=127.0.0.1 cargo nextest run --profile integration \
//!   -p zerodds-amqp-endpoint --test bridge_solace_e2e --run-ignored all
//! ```
//!
//! The broker's platform check refuses Docker's default of 64 MB
//! `/dev/shm` and a hard `nofile` limit below 1048576 (measured with
//! 524288: `POST Violation [022]:Required system resource missing, Hard
//! resource limit nofile 1048576 is required, 524288 detected`); 1 GB of
//! `/dev/shm` is enough.
//!
//! The tests run one at a time, as the profile `integration` runs them:
//! each sets the broker's server certificate to one of a CA of its own, and
//! the reconnect case disconnects every AMQP client of the VPN.
//!
//! Each test configures the broker through its management API SEMP v2
//! (user `admin`, password `admin`, as the container's environment sets
//! them): basic authentication of the message VPN `default` against the
//! broker's own user table, the client user `default` with the password
//! `default`, and a server certificate of a CA the test creates for the
//! AMQP TLS port. The bridge connects over TLS with SASL PLAIN, because the
//! daemon sends PLAIN in the clear only to a loopback broker (§7.2) and a
//! CI service container is not one. qpid-proton connects to the plain AMQP
//! port.
//!
//! A missing variable or a broker that does not answer fails the test.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long the broker may take to answer SEMP.
const READY: Duration = Duration::from_secs(20);

/// The client user of the message VPN `default`, and its password.
pub const USER: &str = "default";
pub const PASSWORD: &str = "default";

/// `admin:admin` in base64 (RFC 7617 Basic), the SEMP user of the
/// container's environment.
const SEMP_AUTH: &str = "YWRtaW46YWRtaW4=";

/// A configured Solace broker.
pub struct SolaceBroker {
    /// The host name the bridge and qpid-proton connect to.
    pub host: String,
    pub amqp_port: u16,
    pub amqps_port: u16,
    semp_port: u16,
    /// Holds `ca.pem`, the CA of the broker's server certificate.
    dir: PathBuf,
}

fn port(var: &str, default: u16) -> u16 {
    std::env::var(var)
        .ok()
        .map(|v| v.parse().unwrap_or_else(|_| panic!("{var}={v} is no port")))
        .unwrap_or(default)
}

/// `s` as the content of a JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Every octet of `s` outside RFC 3986's unreserved set percent-encoded.
fn path_segment(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl SolaceBroker {
    /// The broker `ZERODDS_SOLACE` names, configured for the test.
    ///
    /// # Panics
    /// When the variable is missing, SEMP does not answer within 20 s, or
    /// the broker refuses a configuration step.
    pub fn connect() -> Self {
        let host = std::env::var("ZERODDS_SOLACE").unwrap_or_else(|_| {
            panic!(
                "ZERODDS_SOLACE is not set: the host of a Solace PubSub+ container (tests/support/solace.rs)"
            )
        });
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("zerodds-solace-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let broker = Self {
            host,
            amqp_port: port("ZERODDS_SOLACE_AMQP_PORT", 5672),
            amqps_port: port("ZERODDS_SOLACE_AMQPS_PORT", 5671),
            semp_port: port("ZERODDS_SOLACE_SEMP_PORT", 8080),
            dir,
        };
        broker.wait_ready();
        broker.configure();
        broker
    }

    fn wait_ready(&self) {
        let deadline = Instant::now() + READY;
        loop {
            match self.try_semp("GET", "/SEMP/v2/monitor/about/api", None) {
                Ok((200, _)) => break,
                other => assert!(
                    Instant::now() < deadline,
                    "Solace SEMP at {}:{} did not answer within {READY:?}: {other:?}",
                    self.host,
                    self.semp_port
                ),
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        // SEMP answers before the AMQP service does: the broker listens
        // early and aborts a connection until it answers the protocol
        // header (Part 2 §2.2).
        let deadline = Instant::now() + READY;
        while !self.amqp_answers() {
            assert!(
                Instant::now() < deadline,
                "Solace at {}:{} does not answer the AMQP protocol header",
                self.host,
                self.amqp_port
            );
            std::thread::sleep(Duration::from_millis(500));
        }
        while TcpStream::connect((self.host.as_str(), self.amqps_port)).is_err() {
            assert!(
                Instant::now() < deadline,
                "Solace does not listen on {}:{}",
                self.host,
                self.amqps_port
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Whether the broker answers the SASL protocol header with a header.
    fn amqp_answers(&self) -> bool {
        let Ok(mut s) = TcpStream::connect((self.host.as_str(), self.amqp_port)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        let mut header = [0u8; 8];
        s.write_all(b"AMQP\x03\x01\x00\x00").is_ok()
            && s.read_exact(&mut header).is_ok()
            && &header[..4] == b"AMQP"
    }

    /// Basic authentication against the broker's user table, the client
    /// user's password, a server certificate for the AMQP TLS port.
    fn configure(&self) {
        self.semp_ok(
            "PATCH",
            "/SEMP/v2/config/msgVpns/default",
            r#"{"authenticationBasicType":"internal"}"#,
        );
        self.semp_ok(
            "PATCH",
            "/SEMP/v2/config/msgVpns/default/clientUsernames/default",
            &format!(r#"{{"password":{}}}"#, json_string(PASSWORD)),
        );
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let mut names = vec![self.host.clone()];
        if self.host != "localhost" {
            names.push("localhost".into());
        }
        let cert = rcgen::CertificateParams::new(names)
            .unwrap()
            .signed_by(&key, &ca, &ca_key)
            .unwrap();
        std::fs::write(self.dir.join("ca.pem"), ca.pem()).unwrap();
        let content = format!("{}{}", cert.pem(), key.serialize_pem());
        self.semp_ok(
            "PATCH",
            "/SEMP/v2/config",
            &format!(r#"{{"tlsServerCertContent":{}}}"#, json_string(&content)),
        );
    }

    /// The CA file of the broker's server certificate.
    #[must_use]
    pub fn ca_file(&self) -> PathBuf {
        self.dir.join("ca.pem")
    }

    /// A queue of the VPN `default` that every client may consume from and
    /// send to; an existing one is kept.
    pub fn queue(&self, name: &str) {
        self.create_queue(name, r#""permission":"consume""#);
    }

    /// A queue owned by another user with the permission `no-access`: the
    /// broker refuses the attach of a receiver from it
    /// (`amqp:unauthorized-access`, "Permission Not Allowed"); a sender
    /// attaches.
    pub fn queue_without_access(&self, name: &str) {
        self.create_queue(name, r#""owner":"zd-nobody","permission":"no-access""#);
    }

    fn create_queue(&self, name: &str, access: &str) {
        let body = format!(
            r#"{{"queueName":{},"ingressEnabled":true,"egressEnabled":true,{access}}}"#,
            json_string(name)
        );
        // The message spool comes up after the AMQP service
        // (`MESSAGE_SPOOL_DATA_NOT_AVAILABLE` until then).
        let deadline = Instant::now() + READY;
        loop {
            let (status, text) = self.semp(
                "POST",
                "/SEMP/v2/config/msgVpns/default/queues",
                Some(&body),
            );
            if status == 200 || text.contains("ALREADY_EXISTS") {
                return;
            }
            assert!(
                text.contains("MESSAGE_SPOOL_DATA_NOT_AVAILABLE") && Instant::now() < deadline,
                "queue {name}: {status} {text}"
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// The names of the AMQP clients connected to the VPN `default`.
    pub fn amqp_clients(&self) -> Vec<String> {
        let (status, text) = self.semp(
            "GET",
            "/SEMP/v2/monitor/msgVpns/default/clients?select=clientName&count=100",
            None,
        );
        assert_eq!(status, 200, "{text}");
        text.split("\"clientName\":\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(|name| name.replace("\\/", "/"))
            .filter(|name| name.starts_with("#amqp/client/"))
            .collect()
    }

    /// Ends every AMQP connection through SEMP's action `disconnect`; the
    /// broker closes each with `amqp:connection:forced`.
    pub fn disconnect_amqp_clients(&self) {
        let clients = self.amqp_clients();
        assert!(!clients.is_empty(), "no AMQP client to disconnect");
        for name in clients {
            self.semp_ok(
                "PUT",
                &format!(
                    "/SEMP/v2/action/msgVpns/default/clients/{}/disconnect",
                    path_segment(&name)
                ),
                "{}",
            );
        }
    }

    fn semp_ok(&self, method: &str, path: &str, body: &str) {
        let (status, text) = self.semp(method, path, Some(body));
        assert_eq!(status, 200, "SEMP {method} {path}: {text}");
    }

    fn semp(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
        self.try_semp(method, path, body)
            .unwrap_or_else(|e| panic!("SEMP {method} {path}: {e}"))
    }

    /// One HTTP/1.0 request to SEMP (no chunked answer), its status and
    /// body.
    fn try_semp(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let mut s =
            TcpStream::connect((self.host.as_str(), self.semp_port)).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let body = body.unwrap_or("");
        let request = format!(
            "{method} {path} HTTP/1.0\r\nHost: {}:{}\r\nAuthorization: Basic {SEMP_AUTH}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.host,
            self.semp_port,
            body.len()
        );
        s.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
        let mut answer = Vec::new();
        s.read_to_end(&mut answer).map_err(|e| e.to_string())?;
        let answer = String::from_utf8_lossy(&answer).to_string();
        let status = answer
            .split(' ')
            .nth(1)
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| format!("no HTTP status in {answer:?}"))?;
        let body = answer
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        Ok((status, body))
    }
}

impl Drop for SolaceBroker {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
