// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE 1.0 client for the native Rust endpoint.
//!
//! This endpoint's language *is* the reference implementation's language,
//! so unlike every other port in `endpoints/` it does not re-derive a
//! second state machine or a second wire encoder. `zerodds_xrce_client::
//! XrceClient` (`crates/xrce-client`) already builds and sends every real
//! wire submessage itself — `CREATE_CLIENT`/`CREATE`/`DELETE`/
//! `WRITE_DATA`/`READ_DATA`/`DISCONNECT`, decoding replies via
//! [`XrceClient::poll`] and driving §8.4.14 reliability via
//! [`XrceClient::tick`] — so [`XrceClient`] below is a type alias, not a
//! wrapper: adding a second copy of any of that encoding here would be
//! exactly the duplicate this module used to be (it once re-implemented
//! `CREATE_CLIENT`/`CREATE`/`DELETE`/`WRITE_DATA`/`READ_DATA` encoding by
//! hand, byte for byte, alongside the crate it was already depending on).
//!
//! The only thing this endpoint still owns is transport glue: adapting
//! [`crate::MemTransport`] (this endpoint's own in-memory transport,
//! already used by [`crate::Client`]/[`crate::AsyncReader`]) to
//! [`ClientTransport`], so the reference state machine can run over it
//! unmodified.
//!
//! # Spec mapping
//!
//! | Method                                 | Spec reference                  |
//! |-----------------------------------------|----------------------------------|
//! | [`XrceClient::new`]/[`XrceClient::connect`] | §7.8.2/§8.4.5 (`CREATE_CLIENT`) |
//! | [`XrceClient::create_object`]           | §7.8.3 (`CREATE`)               |
//! | [`XrceClient::delete_object`]           | §7.8.3 (`DELETE`)               |
//! | [`XrceClient::request_write`]/[`XrceClient::request_write_sample`] | §7.8.4 (`WRITE_DATA`) |
//! | [`XrceClient::request_read`]            | §7.8.5 (`READ_DATA`)            |
//! | [`XrceClient::disconnect`]              | §8.4.5 (`DELETE(OBJECTID_CLIENT)`) |
//! | [`XrceClient::poll`]                    | Decodes `STATUS_AGENT`/`STATUS`/`DATA`/`HEARTBEAT`/`ACKNACK`/`FRAGMENT` |
//! | [`XrceClient::tick`]                    | §8.4.14.1 (periodic `HEARTBEAT` for reliable in-flight messages) |
//!
//! See `zerodds_xrce_client`'s own module doc for the full behavior —
//! it applies unchanged here, since this *is* that type.

use crate::MemTransport;
pub use zerodds_xrce_client::{
    ClientError, ClientEvent, ClientState, ClientTransport, request_id_from_octets,
    request_id_octets,
};

/// Adapts [`MemTransport`] to [`ClientTransport`], so the reference state
/// machine can run over it unmodified.
impl ClientTransport for MemTransport {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        self.deliver(payload.to_vec());
        Ok(())
    }

    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        Ok(self.receive())
    }
}

/// DDS-XRCE 1.0 client, wired to this endpoint's in-memory transport.
///
/// This is additive to [`crate::Client`]/[`crate::AsyncReader`], which
/// speak an older, in-house, non-conformant frame on purpose (see their
/// doc comments) — this type is the conformant sibling that can reach a
/// real DDS-XRCE agent. A type alias rather than a wrapper struct: every
/// operation, state transition and error case is `zerodds_xrce_client::
/// XrceClient`'s own; there is nothing for this endpoint to add.
pub type XrceClient = zerodds_xrce_client::XrceClient<MemTransport>;

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;
    use zerodds_xrce::handshake::AgentRepresentation;
    use zerodds_xrce::header::{CLIENT_KEY_LEN, ClientKey};
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::object_id::ObjectId;
    use zerodds_xrce::object_info::ResultStatus;
    use zerodds_xrce::object_kind::ObjectKind;
    use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
    use zerodds_xrce::serial_number::SerialNumber16;
    use zerodds_xrce::submessages::write_data::DataFormat;
    use zerodds_xrce::submessages::{Message, StatusAgentPayload, SubmessageId};

    fn test_key() -> ClientKey {
        ClientKey([0xAB; CLIENT_KEY_LEN])
    }

    /// The new, endpoint-specific code in this module: [`MemTransport`]'s
    /// [`ClientTransport`] impl. Everything past the transport seam is
    /// `zerodds_xrce_client`'s own and already covered by
    /// `cargo test -p zerodds-xrce-client`.
    #[test]
    fn mem_transport_round_trips_through_client_transport() {
        let mut t = MemTransport::new();
        assert_eq!(ClientTransport::try_recv(&mut t).expect("recv"), None);
        ClientTransport::send(&mut t, b"hello").expect("send");
        assert_eq!(
            ClientTransport::try_recv(&mut t).expect("recv"),
            Some(b"hello".to_vec())
        );
        assert_eq!(ClientTransport::try_recv(&mut t).expect("recv"), None);
    }

    /// Delivers a real `STATUS_AGENT` (the same shape
    /// `zerodds-xrce-agent`'s `agent_status_reply` builds: a leading
    /// `ResultStatus` then `AGENT_Representation`) to `transport` so a test
    /// can drive `c` from `Connecting` to `Connected`.
    fn ack_connect(c: &mut XrceClient, transport: &MemTransport) {
        let agent_rep = AgentRepresentation {
            timestamp: None,
            version: [0x01, 0x00],
            vendor_id: [0, 0],
            properties: None,
        }
        .encode()
        .expect("encode agent representation");
        let mut body = ResultStatus::ok().encode().to_vec();
        body.extend_from_slice(&agent_rep);
        let sm = StatusAgentPayload {
            representation: body,
        }
        .into_submessage()
        .expect("status_agent submessage");
        let header = MessageHeader {
            session_id: SessionId(0x80),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: None,
        };
        let msg = Message {
            header,
            submessages: vec![sm],
        };
        transport.deliver(msg.encode().expect("encode message"));
        let events = c.poll(Duration::ZERO).expect("poll");
        assert_eq!(
            events,
            vec![ClientEvent::Connected {
                agent: AgentRepresentation {
                    timestamp: None,
                    version: [0x01, 0x00],
                    vendor_id: [0, 0],
                    properties: None,
                }
            }]
        );
        assert_eq!(c.state(), ClientState::Connected);
    }

    /// End to end over this endpoint's own transport: `connect` sends a
    /// real `CREATE_CLIENT`, a real `STATUS_AGENT` reply (queued via
    /// [`MemTransport::deliver`], exactly as an agent's datagram would
    /// arrive) drives the client to `Connected` through [`XrceClient::poll`].
    #[test]
    fn xrce_client_over_mem_transport_completes_a_real_handshake() {
        let transport = MemTransport::new();
        let mut c = XrceClient::new(test_key(), 0x81, transport.clone());
        assert_eq!(c.state(), ClientState::Disconnected);
        c.connect([0x01, 0x00]).expect("connect");
        assert_eq!(c.state(), ClientState::Connecting);
        let sent = transport.receive().expect("CREATE_CLIENT was sent");
        let msg = Message::decode(&sent).expect("decodes");
        assert_eq!(
            msg.submessages[0].header.submessage_id,
            SubmessageId::CreateClient
        );
        ack_connect(&mut c, &transport);
    }

    /// `create_object`/`request_write`/`request_read`/`delete_object`/
    /// `disconnect` each put a real, decodable submessage on
    /// [`MemTransport`] — proving the transport glue this endpoint owns
    /// carries the full real wire path, not just the handshake.
    #[test]
    fn xrce_client_over_mem_transport_sends_real_submessages_for_every_operation() {
        let transport = MemTransport::new();
        let mut c = XrceClient::new(test_key(), 0x81, transport.clone());
        c.connect([0x01, 0x00]).expect("connect");
        let _ = transport.receive(); // drain CREATE_CLIENT
        ack_connect(&mut c, &transport);

        let oid = ObjectId::new(0x0010, ObjectKind::Topic).expect("valid object id");
        c.create_object(
            oid,
            ObjectVariant::ByReference("a".into()),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(
                    ObjectId::new(0x0001, ObjectKind::Participant).expect("valid object id"),
                ),
            },
            false,
            false,
            true,
        )
        .expect("create");
        c.request_write(oid, b"x", true).expect("write");
        c.request_read(oid, 0x80, DataFormat::Data, None, None, true)
            .expect("read");
        c.delete_object(oid, true).expect("delete");
        c.disconnect();

        let mut ids = Vec::new();
        while let Some(bytes) = transport.receive() {
            let msg = Message::decode(&bytes).expect("decodes");
            ids.push(msg.submessages[0].header.submessage_id);
        }
        assert_eq!(
            ids,
            vec![
                SubmessageId::Create,
                SubmessageId::WriteData,
                SubmessageId::ReadData,
                SubmessageId::Delete,
                SubmessageId::Delete, // disconnect's DELETE(OBJECTID_CLIENT)
            ]
        );
        assert_eq!(c.state(), ClientState::Disconnected);
    }

    /// Operations before `connect` are refused, same as
    /// `zerodds_xrce_client::XrceClient`'s own `*_without_connect_rejected`
    /// tests — checked here too since it is the one behavior a caller of
    /// this endpoint would notice first.
    #[test]
    fn create_without_connect_rejected() {
        let mut c = XrceClient::new(test_key(), 0x81, MemTransport::new());
        let oid = ObjectId::from_raw(0x0010);
        let err = c
            .create_object(
                oid,
                ObjectVariant::ByReference("topic-ref".into()),
                ObjectVariantExtra::default(),
                false,
                false,
                true,
            )
            .expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }
}
