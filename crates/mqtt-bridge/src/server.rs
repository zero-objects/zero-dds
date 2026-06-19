// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standalone MQTT broker server — Spec §4 (Operational Behavior).
//!
//! Exposes the in-memory [`crate::broker::Broker`] over TCP: a [`TcpListener`]
//! accept loop spawns, per client connection, a reader thread (drives the
//! broker) and a writer thread (the sole socket writer, draining an outbound
//! channel). Both MQTT 5.0 and 3.1.1 clients are served — the version is
//! negotiated from the CONNECT Protocol Level and every reply is encoded in the
//! client's dialect via the version-aware codec.
//!
//! Supports QoS 0/1/2 (exactly-once delivers on PUBREL), retained messages,
//! Will messages (delivered on abnormal disconnect), wildcard subscriptions,
//! and PING keep-alive. `std`-only.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::broker::{Broker, DeliveryEnvelope, QoS, Will};
use crate::control_packets::{
    AckBody, SubackBody, connect_flags, decode_connect_body_v, decode_subscribe_body_v,
    decode_unsubscribe_body_v, encode_ack_body_v, encode_connack_body_v, encode_suback_body_v,
    encode_unsuback_body_v,
};
use crate::net::{byte0, frame_packet, read_packet};
use crate::packet::ControlPacketType;
use crate::version::ProtocolVersion;

/// A message queued to a client's writer thread.
enum Outbound {
    /// A fanned-out PUBLISH to deliver to this subscriber.
    Deliver(DeliveryEnvelope),
    /// A pre-encoded control packet (CONNACK/SUBACK/PUBACK/…).
    Raw(Vec<u8>),
    /// Shut the writer down.
    Close,
}

/// Registry of connected clients → their writer channel.
type Registry = Arc<Mutex<BTreeMap<String, Sender<Outbound>>>>;

/// A running MQTT broker server.
pub struct MqttBrokerServer {
    listener: TcpListener,
    broker: Arc<Mutex<Broker>>,
    registry: Registry,
}

/// Handle to a server running on a background thread.
pub struct ServerHandle {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// The bound local address (useful when binding to port 0).
    #[must_use]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    /// Signals the accept loop to stop and waits for it to finish.
    pub fn shutdown(mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblock the blocking accept() with a throwaway connection.
        let _ = TcpStream::connect(self.addr);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl MqttBrokerServer {
    /// Binds the broker to `addr` (use `127.0.0.1:0` for an ephemeral port).
    ///
    /// # Errors
    /// I/O error binding the listener.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            broker: Arc::new(Mutex::new(Broker::new())),
            registry: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// The bound local address.
    ///
    /// # Errors
    /// I/O error querying the socket.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Spawns the accept loop on a background thread, returning a handle that
    /// can shut it down.
    ///
    /// # Errors
    /// I/O error cloning the listener for the worker thread.
    pub fn spawn(self) -> std::io::Result<ServerHandle> {
        let addr = self.listener.local_addr()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let join = thread::spawn(move || self.run(&flag));
        Ok(ServerHandle {
            addr,
            shutdown,
            join: Some(join),
        })
    }

    /// Runs the accept loop until `shutdown` is set. Each connection is handled
    /// on its own thread.
    fn run(self, shutdown: &AtomicBool) {
        for stream in self.listener.incoming() {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }
            let Ok(stream) = stream else { continue };
            let broker = Arc::clone(&self.broker);
            let registry = Arc::clone(&self.registry);
            thread::spawn(move || {
                let _ = handle_connection(stream, &broker, &registry);
            });
        }
    }
}

/// Per-connection handler: CONNECT handshake, then the reader loop. Spawns a
/// writer thread that owns the socket-write side.
fn handle_connection(
    stream: TcpStream,
    broker: &Arc<Mutex<Broker>>,
    registry: &Registry,
) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    let mut reader = stream.try_clone()?;
    let writer_stream = stream.try_clone()?;

    // ---- CONNECT (§3.1) -------------------------------------------------
    let (b0, body) = read_packet(&mut reader)?;
    if (b0 >> 4) != ControlPacketType::Connect.to_bits() {
        return Err(std::io::Error::other("first packet must be CONNECT"));
    }
    // Protocol Level lives at a fixed offset: utf8("MQTT")=6 bytes, then level.
    let level = *body
        .get(6)
        .ok_or_else(|| std::io::Error::other("short CONNECT"))?;
    let version = ProtocolVersion::from_level(level)
        .ok_or_else(|| std::io::Error::other("unsupported protocol level"))?;
    let connect = decode_connect_body_v(&body, version).map_err(codec)?;

    let clean_start = connect.connect_flags & connect_flags::CLEAN_START != 0;
    let will = extract_will(&connect);
    let client_id = if connect.client_id.is_empty() {
        // §3.1.3.1 — server-assigned id for an empty client id.
        std::format!("zerodds-{:p}", &reader as *const _)
    } else {
        connect.client_id.clone()
    };

    broker
        .lock()
        .expect("broker mutex")
        .connect(client_id.clone(), clean_start, will);

    // ---- per-connection writer thread (sole socket writer) --------------
    let (tx, rx) = channel::<Outbound>();
    registry
        .lock()
        .expect("registry mutex")
        .insert(client_id.clone(), tx.clone());
    let writer_join = spawn_writer(writer_stream, rx, version);

    // CONNACK (§3.2) — fresh session, success.
    let connack = encode_connack_body_v(
        &crate::control_packets::ConnackBody {
            session_present: false,
            reason_code: 0,
            properties: Vec::new(),
        },
        version,
    )
    .map_err(codec)?;
    tx.send(Outbound::Raw(frame_packet(
        byte0(ControlPacketType::ConnAck, 0),
        &connack,
    )?))
    .ok();

    // ---- reader loop ----------------------------------------------------
    let mut pending_qos2: BTreeMap<u16, (String, Vec<u8>, bool)> = BTreeMap::new();
    let mut clean_disconnect = false;
    // A read error / EOF ends the loop as an abnormal disconnect (Will fires).
    while let Ok((pb0, pbody)) = read_packet(&mut reader) {
        let ptype = (pb0 >> 4) & 0x0F;
        match ControlPacketType::from_bits(ptype) {
            Some(ControlPacketType::Publish) => {
                handle_publish(
                    pb0,
                    &pbody,
                    version,
                    broker,
                    registry,
                    &tx,
                    &mut pending_qos2,
                )?;
            }
            Some(ControlPacketType::PubRel) => {
                // §4.3.3 — exactly-once: deliver the stored message on PUBREL.
                if pbody.len() >= 2 {
                    let pid = u16::from_be_bytes([pbody[0], pbody[1]]);
                    if let Some((topic, payload, retain)) = pending_qos2.remove(&pid) {
                        fanout(broker, registry, &topic, &payload, QoS::ExactlyOnce, retain);
                    }
                    let comp = encode_ack_body_v(&ack(pid), version).map_err(codec)?;
                    tx.send(Outbound::Raw(frame_packet(
                        byte0(ControlPacketType::PubComp, 0),
                        &comp,
                    )?))
                    .ok();
                }
            }
            Some(ControlPacketType::PubRec) => {
                // A subscriber acked our QoS-2 delivery → release it.
                if pbody.len() >= 2 {
                    let pid = u16::from_be_bytes([pbody[0], pbody[1]]);
                    let rel = encode_ack_body_v(&ack(pid), version).map_err(codec)?;
                    // PUBREL has fixed flags 0b0010 (§3.6.1).
                    tx.send(Outbound::Raw(frame_packet(
                        byte0(ControlPacketType::PubRel, 0b0010),
                        &rel,
                    )?))
                    .ok();
                }
            }
            Some(ControlPacketType::PubAck | ControlPacketType::PubComp) => {
                // Downstream QoS-1/2 delivery acknowledged — nothing to do for
                // an in-memory broker (no retransmission queue).
            }
            Some(ControlPacketType::Subscribe) => {
                handle_subscribe(&pbody, version, broker, &client_id, &tx)?;
            }
            Some(ControlPacketType::Unsubscribe) => {
                let unsub = decode_unsubscribe_body_v(&pbody, version).map_err(codec)?;
                broker
                    .lock()
                    .expect("broker mutex")
                    .unsubscribe(&client_id, &unsub.topic_filters)
                    .ok();
                let ack = encode_unsuback_body_v(
                    &SubackBody {
                        packet_id: unsub.packet_id,
                        properties: Vec::new(),
                        reason_codes: std::vec![0x00; unsub.topic_filters.len()],
                    },
                    version,
                )
                .map_err(codec)?;
                tx.send(Outbound::Raw(frame_packet(
                    byte0(ControlPacketType::UnsubAck, 0),
                    &ack,
                )?))
                .ok();
            }
            Some(ControlPacketType::PingReq) => {
                tx.send(Outbound::Raw(frame_packet(
                    byte0(ControlPacketType::PingResp, 0),
                    &[],
                )?))
                .ok();
            }
            Some(ControlPacketType::Disconnect) => {
                clean_disconnect = true;
                break;
            }
            _ => {}
        }
    }

    // ---- teardown -------------------------------------------------------
    registry.lock().expect("registry mutex").remove(&client_id);
    tx.send(Outbound::Close).ok();
    let _ = writer_join.join();
    // Will is delivered only on an abnormal disconnect (§3.1.2.5).
    let envelopes = broker
        .lock()
        .expect("broker mutex")
        .disconnect(&client_id, !clean_disconnect);
    route(registry, envelopes);
    Ok(())
}

/// Spawns the writer thread that owns all socket writes for a connection.
fn spawn_writer(
    mut stream: TcpStream,
    rx: Receiver<Outbound>,
    version: ProtocolVersion,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut next_id: u16 = 1;
        for msg in rx {
            match msg {
                Outbound::Raw(bytes) => {
                    if stream.write_all(&bytes).is_err() {
                        break;
                    }
                }
                Outbound::Deliver(env) => {
                    let packet_id = if env.qos == QoS::AtMostOnce {
                        None
                    } else {
                        let id = next_id;
                        next_id = next_id.wrapping_add(1);
                        if next_id == 0 {
                            next_id = 1;
                        }
                        Some(id)
                    };
                    let publish = crate::codec::PublishPacket {
                        dup: false,
                        qos: env.qos.to_u8(),
                        retain: env.retain,
                        topic: env.topic,
                        packet_id,
                        properties: Vec::new(),
                        payload: env.payload,
                    };
                    match crate::codec::encode_publish_v(&publish, version) {
                        Ok(bytes) => {
                            if stream.write_all(&bytes).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                Outbound::Close => break,
            }
        }
    })
}

fn handle_publish(
    pb0: u8,
    pbody: &[u8],
    version: ProtocolVersion,
    broker: &Arc<Mutex<Broker>>,
    registry: &Registry,
    tx: &Sender<Outbound>,
    pending_qos2: &mut BTreeMap<u16, (String, Vec<u8>, bool)>,
) -> std::io::Result<()> {
    let full = frame_packet(pb0, pbody)?;
    let (_, p) = crate::codec::decode_publish_v(&full, version).map_err(codec)?;
    let qos = QoS::from_u8(p.qos).unwrap_or(QoS::AtMostOnce);
    match qos {
        QoS::AtMostOnce => {
            fanout(broker, registry, &p.topic, &p.payload, qos, p.retain);
        }
        QoS::AtLeastOnce => {
            fanout(broker, registry, &p.topic, &p.payload, qos, p.retain);
            if let Some(id) = p.packet_id {
                let puback = encode_ack_body_v(&ack(id), version).map_err(codec)?;
                tx.send(Outbound::Raw(frame_packet(
                    byte0(ControlPacketType::PubAck, 0),
                    &puback,
                )?))
                .ok();
            }
        }
        QoS::ExactlyOnce => {
            // §4.3.3 — store, ack with PUBREC, deliver on PUBREL.
            if let Some(id) = p.packet_id {
                pending_qos2.insert(id, (p.topic.clone(), p.payload.clone(), p.retain));
                let pubrec = encode_ack_body_v(&ack(id), version).map_err(codec)?;
                tx.send(Outbound::Raw(frame_packet(
                    byte0(ControlPacketType::PubRec, 0),
                    &pubrec,
                )?))
                .ok();
            }
        }
    }
    Ok(())
}

fn handle_subscribe(
    pbody: &[u8],
    version: ProtocolVersion,
    broker: &Arc<Mutex<Broker>>,
    client_id: &str,
    tx: &Sender<Outbound>,
) -> std::io::Result<()> {
    let sub = decode_subscribe_body_v(pbody, version).map_err(codec)?;
    let mut granted = Vec::with_capacity(sub.subscriptions.len());
    let mut broker_subs = Vec::with_capacity(sub.subscriptions.len());
    for s in &sub.subscriptions {
        let max_qos = QoS::from_u8(s.options & 0x03).unwrap_or(QoS::AtMostOnce);
        granted.push(max_qos.to_u8());
        broker_subs.push(crate::broker::Subscription {
            filter: s.topic_filter.clone(),
            max_qos,
            no_local: s.options & 0x04 != 0,
            retain_as_published: s.options & 0x08 != 0,
        });
    }
    // Collect retained matches before mutating, then apply the subscription.
    let mut retained_envs = Vec::new();
    {
        let mut b = broker.lock().expect("broker mutex");
        for (i, s) in sub.subscriptions.iter().enumerate() {
            for r in b.retained_for(&s.topic_filter) {
                let eff = min_qos(r.qos, broker_subs[i].max_qos);
                retained_envs.push(DeliveryEnvelope {
                    client_id: client_id.into(),
                    topic: r.topic.clone(),
                    payload: r.payload.clone(),
                    qos: eff,
                    retain: true,
                });
            }
        }
        match b.subscribe(client_id, broker_subs) {
            Ok(_) => {}
            Err(_) => granted.fill(0x80), // §3.9.3 — failure.
        }
    }
    let suback = encode_suback_body_v(
        &SubackBody {
            packet_id: sub.packet_id,
            properties: Vec::new(),
            reason_codes: granted,
        },
        version,
    )
    .map_err(codec)?;
    tx.send(Outbound::Raw(frame_packet(
        byte0(ControlPacketType::SubAck, 0),
        &suback,
    )?))
    .ok();
    // §3.3.1.3 — deliver retained messages matching the new subscription.
    for env in retained_envs {
        tx.send(Outbound::Deliver(env)).ok();
    }
    Ok(())
}

/// Publishes to the broker and routes the resulting envelopes to subscribers.
fn fanout(
    broker: &Arc<Mutex<Broker>>,
    registry: &Registry,
    topic: &str,
    payload: &[u8],
    qos: QoS,
    retain: bool,
) {
    let envelopes =
        broker
            .lock()
            .expect("broker mutex")
            .publish(topic, payload.to_vec(), qos, retain);
    route(registry, envelopes);
}

/// Sends each envelope to its target client's writer channel.
fn route(registry: &Registry, envelopes: Vec<DeliveryEnvelope>) {
    let reg = registry.lock().expect("registry mutex");
    for env in envelopes {
        if let Some(tx) = reg.get(&env.client_id) {
            tx.send(Outbound::Deliver(env)).ok();
        }
    }
}

fn extract_will(c: &crate::control_packets::ConnectBody) -> Option<Will> {
    if c.connect_flags & connect_flags::WILL == 0 {
        return None;
    }
    let qos = (c.connect_flags & connect_flags::WILL_QOS_MASK) >> 3;
    Some(Will {
        topic: c.will_topic.clone().unwrap_or_default(),
        payload: c.will_payload.clone(),
        qos: QoS::from_u8(qos).unwrap_or(QoS::AtMostOnce),
        retain: c.connect_flags & connect_flags::WILL_RETAIN != 0,
    })
}

fn min_qos(a: QoS, b: QoS) -> QoS {
    match (a, b) {
        (QoS::AtMostOnce, _) | (_, QoS::AtMostOnce) => QoS::AtMostOnce,
        (QoS::AtLeastOnce, _) | (_, QoS::AtLeastOnce) => QoS::AtLeastOnce,
        _ => QoS::ExactlyOnce,
    }
}

fn ack(packet_id: u16) -> AckBody {
    AckBody {
        packet_id,
        reason_code: 0,
        properties: Vec::new(),
    }
}

fn codec(e: crate::codec::CodecError) -> std::io::Error {
    std::io::Error::other(std::format!("mqtt codec: {e}"))
}
