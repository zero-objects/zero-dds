// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Agent discovery (DDS-XRCE 1.0 §11.2.4): `GET_INFO(OBJECTID_AGENT)` on
//! the multicast discovery group, answered `INFO` unicast with the
//! agent's listening locators.
//!
//! Discovery is deliberately **not** routed through
//! [`crate::daemon::session::SessionEngine`]: a discovery `GET_INFO` comes
//! from a peer with no established session (that is the point — it is how
//! a client finds the agent's real listener address before it has one),
//! and every `Outbound` the session engine produces is addressed by
//! [`crate::daemon::session::ClientKeyOrd`], resolved through the
//! transport's `PeerTable`, which a not-yet-registered peer has no entry
//! in. Threading an "anonymous, no `ClientKeyOrd`" case through that
//! addressing model for the one submessage kind that needs it was not
//! worth the complexity it would add to every other reply path, so this
//! module answers `GET_INFO(OBJECTID_AGENT)` directly and leaves
//! `GET_INFO` on any other object id (which only makes sense inside an
//! established session, querying one of that client's own objects) to
//! the session engine.

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::object_info::{BaseObjectReply, BaseObjectRequest, InfoMask, ResultStatus};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::submessages::{GetInfoPayload, InfoPayload};
use zerodds_xrce::transport_locator::TransportLocator;
use zerodds_xrce::{Message, Submessage, SubmessageId};

/// Runs the multicast discovery responder until `stop` is set. Blocking;
/// call from its own thread.
///
/// `vendor_id` is the agent's `xrce_vendor_id` for its `AGENT_Representation`.
/// `listening_locators` are what this agent reports in every `INFO`
/// reply — the daemon's own configured listener addresses (spec item 2:
/// "answer GET_INFO on the discovery port/multicast group [...] with
/// INFO carrying the listening locators").
pub fn run_discovery_responder(
    socket: UdpSocket,
    listening_locators: Vec<TransportLocator>,
    vendor_id: [u8; 2],
    stop: Arc<AtomicBool>,
) {
    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
    let mut buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        let (n, peer) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => continue,
        };
        let Ok(msg) = Message::decode(&buf[..n]) else {
            continue;
        };
        for sm in &msg.submessages {
            if sm.header.submessage_id != SubmessageId::GetInfo {
                continue;
            }
            let Ok(payload) = GetInfoPayload::try_from_submessage(sm) else {
                continue;
            };
            let Ok((base, _)) = BaseObjectRequest::decode(&payload.representation) else {
                continue;
            };
            // §8.3.5.3: an object_id of kind OBJK_AGENT is the Root.
            if base.object_id.kind() != Ok(ObjectKind::Agent) {
                continue; // not a discovery request — the session engine's job
            }
            let (status, mask) = match payload.parse(sm.header.body_endianness()) {
                // §8.4.4 Figure 10: an INFO with STATUS_ERR_INCOMPATIBLE for a
                // client of another protocol version.
                Ok((_, mask)) => (
                    crate::daemon::session::client_info_status(
                        &payload,
                        sm.header.body_endianness(),
                    ),
                    mask,
                ),
                Err(_) => (
                    zerodds_xrce::object_info::ResultStatusCode::InvalidData,
                    InfoMask(0),
                ),
            };
            if let Some(reply) =
                build_info_reply(base, status, mask, vendor_id, &listening_locators)
            {
                if let Ok(bytes) = reply.encode() {
                    let _ = socket.send_to(&bytes, peer);
                }
            }
        }
    }
}

fn build_info_reply(
    base: BaseObjectRequest,
    status: zerodds_xrce::object_info::ResultStatusCode,
    mask: InfoMask,
    vendor_id: [u8; 2],
    locators: &[TransportLocator],
) -> Option<Message> {
    let info = if status.is_success() {
        crate::daemon::session::agent_object_info(mask, vendor_id, locators)
    } else {
        zerodds_xrce::object_info::ObjectInfo::default()
    };
    let reply = BaseObjectReply {
        related_request: base,
        result: ResultStatus {
            status_code: status,
            implementation_status: 0,
        },
    };
    let sm: Submessage = InfoPayload::new(reply, &info, Endianness::Little)
        .and_then(InfoPayload::into_submessage)
        .ok()?;
    let header = zerodds_xrce::header::MessageHeader::without_client_key(
        zerodds_xrce::header::SessionId(zerodds_xrce::header::SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
        zerodds_xrce::header::StreamId::NONE,
        zerodds_xrce::serial_number::SerialNumber16::new(0),
    )
    .ok()?;
    Message::new(header, vec![sm]).ok()
}

/// Binds a socket on `0.0.0.0:port` and joins the multicast group `group`
/// (§11.2.4: "The XRCE Agents shall be pre-configured with the multicast
/// address and port number they shall listen to. By default, they shall be
/// the address 239.255.0.2 and the port 7400").
///
/// # Errors
/// [`std::io::Error`] if the bind or the multicast join fails, or `group` is
/// no multicast address.
pub fn bind_discovery_socket(group: std::net::Ipv4Addr, port: u16) -> std::io::Result<UdpSocket> {
    if !group.is_multicast() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{group} is no IPv4 multicast address"),
        ));
    }
    // Shared: 7400 is DDS domain 0's SPDP multicast port as well.
    let socket = zerodds_xrce::discovery::bind_shared_udp(port)?;
    socket.join_multicast_v4(&group, &std::net::Ipv4Addr::UNSPECIFIED)?;
    Ok(socket)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::thread;
    use zerodds_xrce::object_id::OBJECTID_AGENT;
    use zerodds_xrce::object_info::{ActivityInfoVariant, ObjectConfig, ResultStatusCode};
    use zerodds_xrce::transport_locator::TransportLocatorMedium;

    /// Starts a responder, sends it `request` and returns the answer.
    fn ask(request: Message) -> Option<Message> {
        let server = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let server_addr = server.local_addr().unwrap();
        let locators = vec![TransportLocator::Medium(TransportLocatorMedium {
            address: [127, 0, 0, 1],
            port: 2018,
        })];
        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);
        let handle =
            thread::spawn(move || run_discovery_responder(server, locators, [1, 15], stop_c));
        let client = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .send_to(&request.encode().unwrap(), server_addr)
            .unwrap();
        let mut buf = [0u8; 4096];
        let answer = client
            .recv_from(&mut buf)
            .ok()
            .map(|(n, _)| Message::decode(&buf[..n]).unwrap());
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        answer
    }

    fn get_info(object_id: zerodds_xrce::object_id::ObjectId, mask: Option<InfoMask>) -> Message {
        let base = BaseObjectRequest {
            request_id: [0x00, 0x01],
            object_id,
        };
        let sm = match mask {
            Some(mask) => GetInfoPayload::new(base, mask, Endianness::Little),
            None => GetInfoPayload {
                representation: base.encode().to_vec(),
            },
        }
        .into_submessage()
        .unwrap();
        let header = zerodds_xrce::header::MessageHeader::without_client_key(
            zerodds_xrce::header::SessionId(0x80),
            zerodds_xrce::header::StreamId::NONE,
            zerodds_xrce::serial_number::SerialNumber16::new(0),
        )
        .unwrap();
        Message {
            header,
            submessages: vec![sm],
        }
    }

    fn parse_info(reply: &Message) -> (BaseObjectReply, zerodds_xrce::object_info::ObjectInfo) {
        assert_eq!(reply.submessages.len(), 1);
        assert_eq!(
            reply.submessages[0].header.submessage_id,
            SubmessageId::Info
        );
        InfoPayload::try_from_submessage(&reply.submessages[0])
            .unwrap()
            .parse(reply.submessages[0].header.body_endianness())
            .unwrap()
    }

    /// §7.8.2.2, §8.3.5.7: the INFO answering a GET_INFO(OBJK_AGENT) echoes
    /// request_id and object_id, carries STATUS_OK, the activity
    /// (`AGENT_ActivityInfo` with the transport addresses) and the
    /// configuration (`AGENT_Representation` with version and vendor).
    #[test]
    fn discovery_responder_answers_get_info_on_agent() {
        let reply = ask(get_info(OBJECTID_AGENT, Some(InfoMask::ALL))).expect("INFO");
        let (base, info) = parse_info(&reply);
        assert_eq!(base.related_request.request_id, [0x00, 0x01]);
        assert_eq!(base.related_request.object_id, OBJECTID_AGENT);
        assert_eq!(base.result.status_code, ResultStatusCode::Ok);
        assert_eq!(
            info.activity,
            Some(ActivityInfoVariant::Agent {
                availability: 1,
                address_seq: vec![TransportLocator::Medium(TransportLocatorMedium {
                    address: [127, 0, 0, 1],
                    port: 2018,
                })],
            })
        );
        match info.config {
            Some(ObjectConfig::Agent(rep)) => {
                assert_eq!(rep.version, [1, 0]);
                assert_eq!(rep.vendor_id, [1, 15]);
            }
            other => panic!("config is not the AGENT_Representation: {other:?}"),
        }
    }

    /// §7.8.3.3: the info_mask selects what comes back.
    #[test]
    fn the_info_mask_selects_activity_and_configuration() {
        let only_activity = ask(get_info(
            OBJECTID_AGENT,
            Some(InfoMask(InfoMask::INFO_ACTIVITY)),
        ))
        .expect("INFO");
        let (_, info) = parse_info(&only_activity);
        assert!(info.activity.is_some() && info.config.is_none());
        let only_config = ask(get_info(
            OBJECTID_AGENT,
            Some(InfoMask(InfoMask::INFO_CONFIGURATION)),
        ))
        .expect("INFO");
        let (_, info) = parse_info(&only_config);
        assert!(info.activity.is_none() && info.config.is_some());
        let neither = ask(get_info(OBJECTID_AGENT, Some(InfoMask(0)))).expect("INFO");
        let (base, info) = parse_info(&neither);
        assert_eq!(base.result.status_code, ResultStatusCode::Ok);
        assert_eq!(info, zerodds_xrce::object_info::ObjectInfo::default());
    }

    /// §8.3.5.3: the ObjectKind in the last four bits selects the Root; any
    /// object_id of kind OBJK_AGENT does, not only 0xFFFD.
    #[test]
    fn any_object_id_of_kind_agent_is_the_root() {
        let other_agent = zerodds_xrce::object_id::ObjectId::new(
            0x012,
            zerodds_xrce::object_kind::ObjectKind::Agent,
        )
        .unwrap();
        let reply = ask(get_info(other_agent, Some(InfoMask::ALL))).expect("INFO");
        let (base, info) = parse_info(&reply);
        assert_eq!(base.related_request.object_id, other_agent);
        assert!(info.activity.is_some());
    }

    /// §8.3.5.7.2: a GET_INFO the agent cannot read (no InfoMask) is
    /// answered with an INFO whose ResultStatus is the error.
    #[test]
    fn a_get_info_without_a_mask_is_answered_with_an_error_info() {
        let reply = ask(get_info(OBJECTID_AGENT, None)).expect("INFO");
        let (base, info) = parse_info(&reply);
        assert_eq!(base.result.status_code, ResultStatusCode::InvalidData);
        assert_eq!(info, zerodds_xrce::object_info::ObjectInfo::default());
    }

    /// A GET_INFO of an object that is not the Root belongs to a session
    /// and is not answered by the discovery.
    #[test]
    fn get_info_on_a_non_agent_object_is_not_answered_by_discovery() {
        let topic =
            zerodds_xrce::object_id::ObjectId::new(1, zerodds_xrce::object_kind::ObjectKind::Topic)
                .unwrap();
        assert!(ask(get_info(topic, Some(InfoMask::ALL))).is_none());
    }

    /// A GET_INFO of the Root with the `client_info` of a client of another
    /// major version of the protocol.
    fn get_info_from_client_of_version(version: [u8; 2]) -> Message {
        use zerodds_xrce::handshake::ClientRepresentation;
        use zerodds_xrce::header::ClientKey;
        use zerodds_xrce::object_info::{ObjectConfig, ObjectInfo};
        let mut request = get_info(OBJECTID_AGENT, Some(InfoMask::ALL));
        let client_info = ObjectInfo {
            activity: None,
            config: Some(ObjectConfig::Client(ClientRepresentation {
                timestamp: None,
                version,
                vendor_id: [0, 0],
                // §7.8.2.2: CLIENTKEY_INVALID, "to not unnecessarily disclose
                // the ClientKey".
                client_key: ClientKey([0; 4]),
                session_id: 0x81,
                properties: None,
                requested_mtu: None,
            })),
        };
        let base = BaseObjectRequest {
            request_id: [0x00, 0x01],
            object_id: OBJECTID_AGENT,
        };
        request.submessages = vec![
            GetInfoPayload::with_client_info(base, InfoMask::ALL, &client_info, Endianness::Little)
                .unwrap()
                .into_submessage()
                .unwrap(),
        ];
        request
    }

    /// §8.4.4 Figure 10: "INFO(): STATUS_ERR_INCOMPATIBLE" for a client of
    /// another major version, "INFO(AGENT_Representation): STATUS_OK" for one
    /// of the same.
    #[test]
    fn a_client_of_another_major_version_is_answered_incompatible() {
        let incompatible = ask(get_info_from_client_of_version([2, 0])).expect("INFO");
        let (base, info) = parse_info(&incompatible);
        assert_eq!(base.result.status_code, ResultStatusCode::Incompatible);
        assert_eq!(info, zerodds_xrce::object_info::ObjectInfo::default());
        let compatible = ask(get_info_from_client_of_version([1, 7])).expect("INFO");
        let (base, info) = parse_info(&compatible);
        assert_eq!(base.result.status_code, ResultStatusCode::Ok);
        assert!(info.config.is_some());
    }

    /// §11.2.4: the agent joins the group it is configured with, 239.255.0.2
    /// by default; an address that is no multicast address is refused.
    #[test]
    fn the_discovery_group_is_the_one_given() {
        assert!(bind_discovery_socket(Ipv4Addr::new(10, 1, 2, 3), 0).is_err());
        let socket = bind_discovery_socket(Ipv4Addr::new(239, 255, 0, 2), 0);
        // The join needs a multicast route; where there is none the bind
        // still must not be mistaken for the refusal above.
        if let Err(e) = socket {
            assert_ne!(e.kind(), std::io::ErrorKind::InvalidInput);
        }
    }
}
