// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! zerodds-mqtt-bridge-1.0 §7.2 `auth.mode: enhanced`: the enhanced
//! authentication of MQTT 5.0 §4.12 between the daemon's client and a test
//! broker that plays the server side — SCRAM-SHA-256 (RFC 5802, RFC 7677),
//! OAUTHBEARER (RFC 7628), the re-authentication of §4.12.1 after a reload,
//! and the protocol errors of [MQTT-4.12.0-5] and [MQTT-4.12.0-6]; the ACL
//! subject of a SCRAM user (§7.3) and the token of a tenant (§1 L6).
//!
//! The broker's SCRAM side computes with `ring` from the formulas of RFC
//! 5802 §3; the client side is checked octet for octet against RFC 7677 §3
//! in `zerodds-bridge-security` (`client_auth::tests`).

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ring::{digest, hmac, pbkdf2};

use zerodds_mqtt_bridge::control_packets::{
    AuthBody, ConnackBody, decode_auth_body, decode_connect_body, decode_disconnect_body,
    encode_auth_body, encode_connack_body,
};
use zerodds_mqtt_bridge::daemon::client::{ClientError, MqttClient};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::server;
use zerodds_mqtt_bridge::net::{frame_packet, read_packet};
use zerodds_mqtt_bridge::packet::ControlPacketType;
use zerodds_mqtt_bridge::property_set::{PropertyContext, PropertySet};

const SALT: &[u8] = b"zerodds-scram-salt";
const ITERATIONS: u32 = 4096;

/// What the test broker saw, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    /// A CONNECT: its client identifier, its Authentication Method and
    /// Data, whether it carried a User Name and a Password.
    Connect {
        client_id: String,
        method: Option<String>,
        data: Option<Vec<u8>>,
        user_name: bool,
        password: bool,
    },
    /// An AUTH: its Reason Code, method and data.
    Auth {
        reason: u8,
        method: Option<String>,
        data: Option<Vec<u8>>,
    },
    /// A DISCONNECT with its Reason Code.
    Disconnect(u8),
    /// A PUBLISH on its topic.
    Publish(String),
    /// Another packet, by its type.
    Other(u8),
}

/// How the test broker authenticates.
#[derive(Clone)]
enum Script {
    /// SCRAM-SHA-256 with the password in force; `sign_wrong` answers with
    /// a server signature of another password.
    Scram {
        password: Arc<Mutex<String>>,
        sign_wrong: bool,
    },
    /// OAUTHBEARER that takes the tokens in force.
    OAuth { tokens: Arc<Mutex<Vec<String>>> },
    /// An AUTH 0x18 to the CONNECT, whatever it carried.
    UnaskedAuth,
    /// A CONNACK with the Authentication Method `OTHER`.
    OtherMethod,
}

struct Broker {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
    connections: Arc<Mutex<usize>>,
}

fn send(s: &mut TcpStream, byte0: u8, body: &[u8]) {
    let _ = s.write_all(&frame_packet(byte0, body).unwrap());
}

fn send_auth(s: &mut TcpStream, reason: u8, method: &str, data: &[u8]) {
    let properties = PropertySet {
        authentication_method: Some(method.to_string()),
        authentication_data: Some(data.to_vec()),
        ..PropertySet::default()
    }
    .encode()
    .unwrap();
    let body = encode_auth_body(&AuthBody {
        reason_code: reason,
        properties,
    })
    .unwrap();
    send(s, 0xF0, &body);
}

fn send_connack(s: &mut TcpStream, reason: u8, method: Option<&str>, data: Option<Vec<u8>>) {
    let properties = PropertySet {
        authentication_method: method.map(str::to_string),
        authentication_data: data,
        ..PropertySet::default()
    }
    .encode()
    .unwrap();
    let body = encode_connack_body(&ConnackBody {
        session_present: false,
        reason_code: reason,
        properties,
    })
    .unwrap();
    send(s, 0x20, &body);
}

/// Reads one packet and notes it; `None` when the connection ended.
fn next(s: &mut TcpStream, seen: &Mutex<Vec<Seen>>) -> Option<(u8, Vec<u8>)> {
    let (byte0, body) = read_packet(s).ok()?;
    let note = match byte0 >> 4 {
        1 => {
            let c = decode_connect_body(&body).unwrap();
            let p = PropertySet::parse(
                &c.properties,
                PropertyContext::Packet(ControlPacketType::Connect),
            )
            .unwrap();
            Seen::Connect {
                client_id: c.client_id,
                method: p.authentication_method,
                data: p.authentication_data,
                user_name: c.user_name.is_some(),
                password: c.connect_flags & 0x40 != 0,
            }
        }
        15 => {
            let a = decode_auth_body(&body).unwrap();
            let p = PropertySet::parse(
                &a.properties,
                PropertyContext::Packet(ControlPacketType::Auth),
            )
            .unwrap();
            Seen::Auth {
                reason: a.reason_code,
                method: p.authentication_method,
                data: p.authentication_data,
            }
        }
        14 => Seen::Disconnect(decode_disconnect_body(&body).map_or(0, |d| d.reason_code)),
        3 => {
            let n = usize::from(u16::from_be_bytes([body[0], body[1]]));
            Seen::Publish(String::from_utf8(body[2..2 + n].to_vec()).unwrap())
        }
        t => Seen::Other(t),
    };
    seen.lock().unwrap().push(note.clone());
    Some((byte0, body))
}

fn data_of(seen: &Mutex<Vec<Seen>>) -> Vec<u8> {
    match seen.lock().unwrap().last() {
        Some(Seen::Connect { data, .. } | Seen::Auth { data, .. }) => {
            data.clone().unwrap_or_default()
        }
        other => panic!("no data in {other:?}"),
    }
}

/// The server side of one SCRAM-SHA-256 exchange (RFC 5802 §3): from the
/// client-first-message to the server-final-message, `None` when the
/// client's proof does not verify.
struct ScramServer {
    salted: [u8; 32],
    auth_message_prefix: String,
    nonce: String,
}

impl ScramServer {
    fn first(client_first: &[u8], password: &str) -> (Self, String) {
        let text = std::str::from_utf8(client_first).unwrap();
        let bare = text.strip_prefix("n,,").expect("GS2 header n,,");
        let cnonce = bare.split(",r=").nth(1).unwrap();
        let nonce = format!("{cnonce}srv-7f3a");
        let server_first = format!("r={nonce},s={},i={ITERATIONS}", B64.encode(SALT));
        let mut salted = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(ITERATIONS).unwrap(),
            SALT,
            password.as_bytes(),
            &mut salted,
        );
        (
            Self {
                salted,
                auth_message_prefix: format!("{bare},{server_first}"),
                nonce,
            },
            server_first,
        )
    }

    fn last(&self, client_final: &[u8], sign_wrong: bool) -> Option<String> {
        let text = std::str::from_utf8(client_final).unwrap();
        let (without_proof, proof) = text.rsplit_once(",p=")?;
        assert_eq!(without_proof, format!("c=biws,r={}", self.nonce));
        let auth_message = format!("{},{without_proof}", self.auth_message_prefix);
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.salted);
        let client_key = hmac::sign(&key, b"Client Key");
        let stored = digest::digest(&digest::SHA256, client_key.as_ref());
        let sig = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, stored.as_ref()),
            auth_message.as_bytes(),
        );
        let proof = B64.decode(proof).ok()?;
        let recovered: Vec<u8> = proof.iter().zip(sig.as_ref()).map(|(p, s)| p ^ s).collect();
        if digest::digest(&digest::SHA256, &recovered).as_ref() != stored.as_ref() {
            return None;
        }
        let server_key = hmac::sign(&key, b"Server Key");
        let mut v = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, server_key.as_ref()),
            auth_message.as_bytes(),
        )
        .as_ref()
        .to_vec();
        if sign_wrong {
            v[0] ^= 0xFF;
        }
        Some(format!("v={}", B64.encode(v)))
    }
}

/// One SCRAM exchange whose first data is the last seen; `ok` the reason
/// of the success packet (CONNACK 0x00 or AUTH 0x00).
fn scram_exchange(
    s: &mut TcpStream,
    seen: &Mutex<Vec<Seen>>,
    password: &str,
    sign_wrong: bool,
    reauth: bool,
) -> bool {
    let (server, server_first) = ScramServer::first(&data_of(seen), password);
    send_auth(s, 0x18, "SCRAM-SHA-256", server_first.as_bytes());
    let Some((byte0, _)) = next(s, seen) else {
        return false;
    };
    assert_eq!(byte0 >> 4, 15, "the answer to AUTH 0x18 is AUTH");
    match server.last(&data_of(seen), sign_wrong) {
        Some(v) if reauth => send_auth(s, 0x00, "SCRAM-SHA-256", v.as_bytes()),
        Some(v) => send_connack(s, 0x00, Some("SCRAM-SHA-256"), Some(v.into_bytes())),
        None if reauth => send(s, 0xE0, &[0x87, 0]),
        None => send_connack(s, 0x86, None, None),
    }
    true
}

/// The token of an OAUTHBEARER initial client response.
fn bearer_of(data: &[u8]) -> String {
    let text = String::from_utf8(data.to_vec()).unwrap();
    text.split('\x01')
        .find_map(|kv| kv.strip_prefix("auth=Bearer "))
        .unwrap()
        .to_string()
}

fn serve(mut s: TcpStream, script: &Script, seen: &Mutex<Vec<Seen>>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    if next(&mut s, seen).is_none() {
        return;
    }
    match script {
        Script::Scram {
            password,
            sign_wrong,
        } => {
            let pw = password.lock().unwrap().clone();
            if !scram_exchange(&mut s, seen, &pw, *sign_wrong, false) {
                return;
            }
        }
        Script::OAuth { tokens } => {
            let known = tokens.lock().unwrap().clone();
            if known.contains(&bearer_of(&data_of(seen))) {
                send_connack(&mut s, 0x00, Some("OAUTHBEARER"), None);
            } else {
                send_auth(
                    &mut s,
                    0x18,
                    "OAUTHBEARER",
                    br#"{"status":"invalid_token"}"#,
                );
                let _ = next(&mut s, seen);
                send_connack(&mut s, 0x87, None, None);
                return;
            }
        }
        Script::UnaskedAuth => send_auth(&mut s, 0x18, "SCRAM-SHA-256", b"r=x,s=AAAA,i=4096"),
        Script::OtherMethod => send_connack(&mut s, 0x00, Some("OTHER"), None),
    }
    while let Some((byte0, body)) = next(&mut s, seen) {
        match byte0 >> 4 {
            8 => {
                let mut b = body[..2].to_vec();
                b.extend([0, 0]);
                send(&mut s, 0x90, &b);
            }
            3 if (byte0 >> 1) & 0x03 == 1 => {
                let n = usize::from(u16::from_be_bytes([body[0], body[1]]));
                send(&mut s, 0x40, &[body[2 + n], body[3 + n], 0]);
            }
            12 => send(&mut s, 0xD0, &[]),
            14 => return,
            15 => match script {
                // §4.12.1: a re-authentication of the client.
                Script::Scram {
                    password,
                    sign_wrong,
                } => {
                    let pw = password.lock().unwrap().clone();
                    if !scram_exchange(&mut s, seen, &pw, *sign_wrong, true) {
                        return;
                    }
                }
                Script::OAuth { tokens } => {
                    if tokens.lock().unwrap().contains(&bearer_of(&data_of(seen))) {
                        send_auth(&mut s, 0x00, "OAUTHBEARER", b"");
                    } else {
                        send(&mut s, 0xE0, &[0x87, 0]);
                        return;
                    }
                }
                _ => return,
            },
            _ => {}
        }
    }
}

impl Broker {
    fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
        let connections: Arc<Mutex<usize>> = Arc::default();
        let (seen_t, conns_t) = (Arc::clone(&seen), Arc::clone(&connections));
        thread::spawn(move || {
            for s in listener.incoming() {
                let Ok(s) = s else { break };
                *conns_t.lock().unwrap() += 1;
                let (script, seen) = (script.clone(), Arc::clone(&seen_t));
                thread::spawn(move || serve(s, &script, &seen));
            }
        });
        Self {
            port,
            seen,
            connections,
        }
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

/// The error of a connect that must fail.
fn refused(r: Result<MqttClient, ClientError>) -> ClientError {
    match r {
        Ok(_) => panic!("the connect succeeded"),
        Err(e) => e,
    }
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(Instant::now() < deadline, "{what} within 10 s");
        thread::sleep(Duration::from_millis(20));
    }
}

/// `cfg` towards `broker` on a leased domain, its session state under a
/// directory of `leaf`.
fn daemon_cfg(
    mut cfg: DaemonConfig,
    broker: &Broker,
    leaf: &str,
) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.broker_url = format!("mqtt://127.0.0.1:{}", broker.port);
    cfg.session_dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-{leaf}-{}", std::process::id()))
        .display()
        .to_string();
    cfg.drain_timeout_secs = 2;
    (cfg, lease)
}

/// An out topic `dds` on `mqtt`, reliable (QoS 1).
fn out_topic(dds: &str, mqtt: &str) -> TopicConfig {
    TopicConfig {
        dds_name: dds.to_string(),
        dds_type: dds.to_string(),
        mqtt_topic: mqtt.to_string(),
        direction: "out".to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        ..TopicConfig::default()
    }
}

/// Whether the broker saw an AUTH with Reason Code `reason`.
fn saw_auth(broker: &Broker, want: u8) -> bool {
    broker
        .seen()
        .iter()
        .any(|s| matches!(s, Seen::Auth { reason, .. } if *reason == want))
}

fn scram_cfg(password: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "enhanced-cli".into();
    cfg.auth_enhanced_method = Some("SCRAM-SHA-256".into());
    cfg.outbound_username = Some("bridge".into());
    cfg.outbound_password = Some(password.into());
    cfg
}

/// OAUTHBEARER that takes `tokens`.
fn oauth(tokens: &[&str]) -> Script {
    Script::OAuth {
        tokens: Arc::new(Mutex::new(tokens.iter().map(ToString::to_string).collect())),
    }
}

fn oauth_cfg(token: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "oauth-cli".into();
    cfg.auth_enhanced_method = Some("OAUTHBEARER".into());
    cfg.auth_bearer_token = Some(token.into());
    cfg
}

/// Spec §7.2 OAUTHBEARER: the CONNECT carries the Authentication Method
/// and the initial client response of RFC 7628 §3.1 with the broker's host
/// and port, and neither User Name nor Password; the CONNACK with the
/// method ends the exchange.
#[test]
fn mqttbridge_7_2_enhanced_connect_carries_method_and_data() {
    let broker = Broker::start(oauth(&["tok-1"]));
    let c = MqttClient::connect("127.0.0.1", broker.port, &oauth_cfg("tok-1")).expect("CONNACK");
    assert_eq!(c.auth_method(), Some("OAUTHBEARER"));
    assert_eq!(
        broker.seen()[0],
        Seen::Connect {
            client_id: "oauth-cli".into(),
            method: Some("OAUTHBEARER".into()),
            data: Some(
                format!(
                    "n,,\x01host=127.0.0.1\x01port={}\x01auth=Bearer tok-1\x01\x01",
                    broker.port
                )
                .into_bytes()
            ),
            user_name: false,
            password: false,
        }
    );
}

/// Spec §7.2 SCRAM-SHA-256, MQTT 5.0 §4.12: the broker's AUTH 0x18 with the
/// server-first-message is answered with AUTH 0x18 and the
/// client-final-message before the CONNACK, and nothing else is sent before
/// it ([MQTT-3.1.2-30]); the server signature of the CONNACK verifies.
#[test]
fn mqttbridge_7_2_an_auth_continue_is_answered_before_connack() {
    let broker = Broker::start(Script::Scram {
        password: Arc::new(Mutex::new("pencil".into())),
        sign_wrong: false,
    });
    let c = MqttClient::connect("127.0.0.1", broker.port, &scram_cfg("pencil")).expect("CONNACK");
    assert_eq!(c.auth_method(), Some("SCRAM-SHA-256"));
    let seen = broker.seen();
    assert_eq!(seen.len(), 2, "{seen:?}");
    let Seen::Connect {
        method,
        data,
        user_name,
        password,
        ..
    } = &seen[0]
    else {
        panic!("{seen:?}");
    };
    assert_eq!(method.as_deref(), Some("SCRAM-SHA-256"));
    let first = String::from_utf8(data.clone().unwrap()).unwrap();
    assert!(first.starts_with("n,,n=bridge,r="), "{first}");
    assert!(!user_name && !password);
    let Seen::Auth {
        reason,
        method,
        data,
    } = &seen[1]
    else {
        panic!("{seen:?}");
    };
    assert_eq!((*reason, method.as_deref()), (0x18, Some("SCRAM-SHA-256")));
    let last = String::from_utf8(data.clone().unwrap()).unwrap();
    assert!(last.starts_with("c=biws,r="), "{last}");
}

/// Spec §7.2: a broker whose server signature does not verify does not know
/// the password; the daemon ends the connection with DISCONNECT 0x80, and
/// the error is of the auth class (exit 5). A wrong password is refused by
/// the broker (CONNACK 0x86, exit 5).
#[test]
fn mqttbridge_7_2_a_server_signature_that_does_not_verify_ends_the_connection() {
    let lying = Broker::start(Script::Scram {
        password: Arc::new(Mutex::new("pencil".into())),
        sign_wrong: true,
    });
    let e = refused(MqttClient::connect(
        "127.0.0.1",
        lying.port,
        &scram_cfg("pencil"),
    ));
    assert!(matches!(e, ClientError::Auth(_)), "{e}");
    assert!(e.is_auth_refusal());
    wait_for("the DISCONNECT", || {
        lying.seen().last() == Some(&Seen::Disconnect(0x80))
    });
    let strict = Broker::start(Script::Scram {
        password: Arc::new(Mutex::new("pencil".into())),
        sign_wrong: false,
    });
    let e = refused(MqttClient::connect(
        "127.0.0.1",
        strict.port,
        &scram_cfg("rubber"),
    ));
    assert!(
        matches!(e, ClientError::ConnAck { reason: 0x86, .. }),
        "{e}"
    );
    assert!(e.is_auth_refusal());
}

/// RFC 7628 §3.2.3: an error challenge is answered with `%x01`; the
/// refusal that follows names the status.
#[test]
fn mqttbridge_7_2_oauthbearer_answers_an_error_challenge_with_0x01() {
    let broker = Broker::start(oauth(&["good"]));
    let e = refused(MqttClient::connect(
        "127.0.0.1",
        broker.port,
        &oauth_cfg("stale"),
    ));
    match &e {
        ClientError::ConnAck { reason, detail, .. } => {
            assert_eq!(*reason, 0x87);
            assert!(detail.contains("invalid_token"), "{detail}");
        }
        other => panic!("{other}"),
    }
    assert!(e.is_auth_refusal());
    assert_eq!(
        broker.seen()[1],
        Seen::Auth {
            reason: 0x18,
            method: Some("OAUTHBEARER".into()),
            data: Some(b"\x01".to_vec()),
        }
    );
}

/// [MQTT-4.12.0-6]: an AUTH to a CONNECT without Authentication Method is
/// a protocol error, DISCONNECT 0x82; [MQTT-4.12.0-5]: so is a CONNACK
/// with another method than the CONNECT's.
#[test]
fn mqttbridge_7_2_an_auth_or_method_the_connect_did_not_ask_for_is_a_protocol_error() {
    let unasked = Broker::start(Script::UnaskedAuth);
    let mut plain = DaemonConfig::default_for_dev();
    plain.client_id = "plain-cli".into();
    let e = refused(MqttClient::connect("127.0.0.1", unasked.port, &plain));
    assert!(
        matches!(e, ClientError::Protocol { reason: 0x82, .. }),
        "{e}"
    );
    wait_for("the DISCONNECT 0x82", || {
        unasked.seen().last() == Some(&Seen::Disconnect(0x82))
    });
    let other = Broker::start(Script::OtherMethod);
    let e = refused(MqttClient::connect(
        "127.0.0.1",
        other.port,
        &oauth_cfg("t"),
    ));
    assert!(
        matches!(e, ClientError::Protocol { reason: 0x82, .. }),
        "{e}"
    );
    let plain_other = Broker::start(Script::OtherMethod);
    let e = refused(MqttClient::connect("127.0.0.1", plain_other.port, &plain));
    assert!(
        matches!(e, ClientError::Protocol { reason: 0x82, .. }),
        "{e}"
    );
}

/// MQTT 5.0 §4.12.1 on the client: AUTH 0x19 with the method and the new
/// first data, each AUTH 0x18 of the broker answered, its AUTH 0x00
/// checked; the connection stands.
#[test]
fn mqttbridge_7_2_a_scram_re_authentication_answers_the_continue() {
    let password = Arc::new(Mutex::new("pencil".to_string()));
    let broker = Broker::start(Script::Scram {
        password: Arc::clone(&password),
        sign_wrong: false,
    });
    let mut c =
        MqttClient::connect("127.0.0.1", broker.port, &scram_cfg("pencil")).expect("CONNACK");
    *password.lock().unwrap() = "pencil-2".into();
    c.reauthenticate(&scram_cfg("pencil-2"), "127.0.0.1", broker.port)
        .unwrap();
    assert!(c.reauthenticating());
    let deadline = Instant::now() + Duration::from_secs(10);
    while c.reauthenticating() {
        assert!(Instant::now() < deadline, "{:?}", broker.seen());
        assert!(c.next_event().unwrap().is_none());
    }
    let seen = broker.seen();
    let auths: Vec<u8> = seen
        .iter()
        .filter_map(|s| match s {
            Seen::Auth { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect();
    assert_eq!(auths, [0x18, 0x19, 0x18], "{seen:?}");
    assert!(!seen.iter().any(|s| matches!(s, Seen::Disconnect(_))));
    // Another method is refused before anything goes out.
    assert!(matches!(
        c.reauthenticate(&oauth_cfg("t"), "127.0.0.1", broker.port),
        Err(ClientError::Auth(_))
    ));
}

/// Spec §7.2, §9.2: a SIGHUP reload with a new token makes the daemon's
/// connection re-authenticate with it (AUTH 0x19), without a new
/// connection.
#[test]
fn mqttbridge_7_2_a_reload_re_authenticates_with_the_new_token() {
    let tokens = Arc::new(Mutex::new(vec!["tok-1".to_string()]));
    let broker = Broker::start(Script::OAuth {
        tokens: Arc::clone(&tokens),
    });
    let lease = zerodds_test_domains::lease_domain();
    let dir = std::env::temp_dir().join(format!("zerodds-mqtt-reauth-{}", std::process::id()));
    let mut cfg = oauth_cfg("tok-1");
    cfg.domain = lease.id();
    cfg.broker_url = format!("mqtt://127.0.0.1:{}", broker.port);
    cfg.session_dir = dir.display().to_string();
    cfg.drain_timeout_secs = 2;
    let next = Arc::new(Mutex::new(cfg.clone()));
    let n = Arc::clone(&next);
    let mut d = zerodds_mqtt_bridge::daemon::server::start_with_reload(
        cfg.clone(),
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("daemon start");
    *tokens.lock().unwrap() = vec!["tok-2".into()];
    next.lock().unwrap().auth_bearer_token = Some("tok-2".into());
    d.reload_flag.store(true, Ordering::SeqCst);
    wait_for("AUTH 0x19", || {
        broker.seen().iter().any(|s| {
            matches!(s, Seen::Auth { reason: 0x19, data: Some(d), .. } if bearer_of(d) == "tok-2")
        })
    });
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        *broker.connections.lock().unwrap(),
        1,
        "{:?}",
        broker.seen()
    );
    assert!(
        !broker
            .seen()
            .iter()
            .any(|s| matches!(s, Seen::Disconnect(_)))
    );
    d.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

/// Spec §7.2, §9.2: a reload that changes the ACL and keeps the
/// credentials sends no AUTH 0x19; a reload with a new token after it
/// does.
#[test]
fn mqttbridge_7_2_a_reload_that_keeps_the_credentials_does_not_re_authenticate() {
    let tokens = Arc::new(Mutex::new(vec!["tok-1".to_string()]));
    let broker = Broker::start(Script::OAuth {
        tokens: Arc::clone(&tokens),
    });
    let (cfg, _lease) = daemon_cfg(oauth_cfg("tok-1"), &broker, "reauth-acl");
    let next = Arc::new(Mutex::new(cfg.clone()));
    let n = Arc::clone(&next);
    let mut d = server::start_with_reload(
        cfg.clone(),
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("daemon start");
    next.lock().unwrap().acl_default_deny = Some(true);
    d.reload_flag.store(true, Ordering::SeqCst);
    wait_for("the reload", || !d.reload_flag.load(Ordering::SeqCst));
    thread::sleep(Duration::from_millis(800));
    assert!(!saw_auth(&broker, 0x19), "{:?}", broker.seen());
    tokens.lock().unwrap().push("tok-2".into());
    next.lock().unwrap().auth_bearer_token = Some("tok-2".into());
    d.reload_flag.store(true, Ordering::SeqCst);
    wait_for("AUTH 0x19", || saw_auth(&broker, 0x19));
    d.shutdown();
}

/// Spec §7.3 under `enhanced` with SCRAM-SHA-256: the subject is the SCRAM
/// user name, which the CONNECT does not carry as User Name; a rule for it
/// grants its topic, and `default_deny: true` refuses a topic of no rule.
#[test]
fn mqttbridge_7_3_a_rule_for_the_scram_user_grants_its_topic() {
    use zerodds_mqtt_bridge::daemon::acl::AclRule;
    use zerodds_mqtt_bridge::daemon::metrics::family;
    let broker = Broker::start(Script::Scram {
        password: Arc::new(Mutex::new("pencil".into())),
        sign_wrong: false,
    });
    let mut scram = scram_cfg("pencil");
    scram.client_id = "scram-acl-cli".into();
    let (mut cfg, _lease) = daemon_cfg(scram, &broker, "scram-acl");
    cfg.acl_default_deny = Some(true);
    cfg.acl_rules = vec![AclRule {
        subject: "bridge".into(),
        allow_publish: vec!["scram/+".into()],
        allow_subscribe: Vec::new(),
    }];
    cfg.topics.push(out_topic("Granted", "scram/a"));
    cfg.topics.push(out_topic("Closed", "other/b"));
    let mut d = server::start(cfg).expect("daemon start");
    let m = d.metrics.clone().unwrap();
    for name in ["Granted", "Closed"] {
        assert!(
            d.runtime
                .test_inject_user_alive(d.user_readers[name], vec![1, 2, 3])
        );
    }
    wait_for("the PUBLISH on scram/a", || {
        broker.seen().contains(&Seen::Publish("scram/a".into()))
    });
    wait_for("the publish denial", || {
        m.value(family::ACL_DENIALS, &[("reason", "publish")]) == 1
    });
    thread::sleep(Duration::from_millis(300));
    assert!(
        !broker.seen().contains(&Seen::Publish("other/b".into())),
        "{:?}",
        broker.seen()
    );
    d.shutdown();
}

/// Spec §1 L6 with §7.2: a tenant authenticates in the mode of the top
/// level; under `enhanced` with OAUTHBEARER its `mqtt.password` is the
/// token of its CONNECT, the top level's is `auth.bearer_token`. Such a
/// tenant used to be refused at load (`does not use mqtt.password`).
#[test]
fn mqttbridge_1_l6_a_tenant_presents_its_password_as_the_token() {
    let broker = Broker::start(oauth(&["tok-top", "tok-a"]));
    let lease = zerodds_test_domains::lease_domain();
    let tenant_lease = zerodds_test_domains::lease_domain();
    let mut cfg = DaemonConfig::load_from_str(&format!(
        "domain: {}\nmqtt:\n  broker_url: mqtt://127.0.0.1:{}\n  client_id: oauth-top\n\
         auth:\n  mode: enhanced\n  bearer_token: tok-top\n  enhanced:\n    method: OAUTHBEARER\n\
         tenants:\n  - name: a\n    domain: {}\n    mqtt:\n      client_id: oauth-tenant-a\n      password: tok-a\n",
        lease.id(),
        broker.port,
        tenant_lease.id()
    ))
    .expect("the tenant loads");
    cfg.session_dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-tenant-token-{}", std::process::id()))
        .display()
        .to_string();
    cfg.drain_timeout_secs = 2;
    let mut d = server::start(cfg).expect("daemon start");
    let bearer = |id: &str| {
        broker.seen().iter().find_map(|s| match s {
            Seen::Connect {
                client_id,
                data: Some(data),
                user_name,
                password,
                ..
            } if client_id == id => {
                assert!(!user_name && !password, "{id}");
                Some(bearer_of(data))
            }
            _ => None,
        })
    };
    assert_eq!(bearer("oauth-top").as_deref(), Some("tok-top"));
    assert_eq!(bearer("oauth-tenant-a").as_deref(), Some("tok-a"));
    d.shutdown();
}
