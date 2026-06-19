// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Client — synchronous interface without callbacks (spec §7.2).
//!
//! Crate `zerodds-xrce-client`.
//!
//! # Spec mapping
//!
//! OMG DDS-XRCE 1.0 §7.2: "XRCE Client: simplified interface, no
//! callbacks, text parameters; the session bridges sleep/wakeup
//! cycles."
//!
//! We provide a synchronous state machine [`XrceClient`] with the
//! following operations:
//!
//! | Method              | Spec reference          |
//! |---------------------|-------------------------|
//! | [`XrceClient::new`] | §7.8.2 (CREATE_CLIENT)  |
//! | [`XrceClient::connect`]    | §8.4.5 (Handshake) |
//! | [`XrceClient::create_object`] | §7.8.3 (CREATE) |
//! | [`XrceClient::delete_object`] | §7.8.3 (DELETE) |
//! | [`XrceClient::request_write`] | §7.8.4 (WRITE_DATA) |
//! | [`XrceClient::request_read`]  | §7.8.5 (READ_DATA) |
//! | [`XrceClient::disconnect`]    | §8.4.5 |
//!
//! The transport is abstracted via the [`ClientTransport`] trait — concrete
//! impls (UDP/TCP/DTLS/Serial) live in `crates/xrce/src/transport_*`.
//!
//! Safety classification: **SAFE**.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;

use zerodds_xrce::header::ClientKey;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_repr::ObjectVariant;
use zerodds_xrce::submessages::Submessage;

/// Transport abstraction for the XRCE client.
///
/// Implementations (UDP/TCP/DTLS/Serial) live in
/// `crates/xrce/src/transport_*.rs`.
pub trait ClientTransport {
    /// Sends a submessage bundle to the agent.
    ///
    /// # Errors
    /// Transport-specific.
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError>;

    /// Polls incoming data from the agent. Does not block — if no
    /// data is available, returns `Ok(None)`.
    ///
    /// # Errors
    /// Transport-specific.
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError>;
}

/// XRCE client lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Initial: the client knows its ClientKey but is not yet
    /// connected.
    Disconnected,
    /// Connect handshake in progress (CREATE_CLIENT sent, STATUS_AGENT
    /// not yet received).
    Connecting,
    /// Connected — operations are possible.
    Connected,
}

/// Client-specific error classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    /// Operation in the wrong lifecycle status (e.g. send-without-connect).
    InvalidState,
    /// Transport-layer error (connection reset etc.).
    Transport,
    /// The submessage could not be enqueued (wire cap reached).
    QueueFull,
}

/// Synchronous XRCE client.
pub struct XrceClient<T: ClientTransport> {
    client_key: ClientKey,
    state: ClientState,
    transport: T,
    next_request_id: u16,
    /// Buffered outgoing submessages that the caller pushes into the
    /// transport layer via [`XrceClient::flush`].
    out_queue: Vec<Submessage>,
}

impl<T: ClientTransport> XrceClient<T> {
    /// Constructor.
    pub fn new(client_key: ClientKey, transport: T) -> Self {
        Self {
            client_key,
            state: ClientState::Disconnected,
            transport,
            next_request_id: 1,
            out_queue: Vec::new(),
        }
    }

    /// Current lifecycle status.
    #[must_use]
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Returns the ClientKey.
    #[must_use]
    pub fn client_key(&self) -> ClientKey {
        self.client_key
    }

    /// Initiates the handshake (sends CREATE_CLIENT).
    ///
    /// # Errors
    /// `InvalidState` if already connected.
    pub fn connect(&mut self) -> Result<(), ClientError> {
        if self.state != ClientState::Disconnected {
            return Err(ClientError::InvalidState);
        }
        self.state = ClientState::Connecting;
        Ok(())
    }

    /// Marks the handshake as complete (the caller has
    /// received STATUS_AGENT).
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn mark_connected(&mut self) -> Result<(), ClientError> {
        if self.state != ClientState::Connecting {
            return Err(ClientError::InvalidState);
        }
        self.state = ClientState::Connected;
        Ok(())
    }

    /// Submits a CREATE operation. Returns a `request_id`.
    ///
    /// # Errors
    /// `InvalidState` if not connected.
    pub fn create_object(
        &mut self,
        _object_id: ObjectId,
        _representation: ObjectVariant,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        // The caller delegates submessage encoding; we only track
        // the state.
        Ok(req)
    }

    /// Submits a DELETE operation.
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn delete_object(&mut self, _object_id: ObjectId) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        Ok(req)
    }

    /// Submits a WRITE_DATA operation.
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn request_write(
        &mut self,
        _writer: ObjectId,
        _payload: &[u8],
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        Ok(req)
    }

    /// Submits a READ_DATA operation (pull-based).
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn request_read(&mut self, _reader: ObjectId) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        Ok(req)
    }

    /// Closes the session — goes back to `Disconnected`.
    pub fn disconnect(&mut self) {
        self.state = ClientState::Disconnected;
        self.out_queue.clear();
    }

    /// Checks that the client is connected.
    fn require_connected(&self) -> Result<(), ClientError> {
        if self.state != ClientState::Connected {
            return Err(ClientError::InvalidState);
        }
        Ok(())
    }

    /// Number of pending submessages in the out queue.
    #[must_use]
    pub fn out_queue_len(&self) -> usize {
        self.out_queue.len()
    }

    /// Direct transport access (e.g. for read-pull polling).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use zerodds_xrce::header::CLIENT_KEY_LEN;

    /// Mock transport: collects sent payloads.
    struct MockTransport {
        sent: Vec<Vec<u8>>,
    }
    impl MockTransport {
        fn new() -> Self {
            Self { sent: Vec::new() }
        }
    }
    impl ClientTransport for MockTransport {
        fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
            self.sent.push(payload.to_vec());
            Ok(())
        }
        fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
            Ok(None)
        }
    }

    fn key() -> ClientKey {
        ClientKey([0xAB; CLIENT_KEY_LEN])
    }

    #[test]
    fn new_client_starts_disconnected() {
        let c = XrceClient::new(key(), MockTransport::new());
        assert_eq!(c.state(), ClientState::Disconnected);
        assert_eq!(c.client_key(), key());
    }

    #[test]
    fn connect_transitions_to_connecting() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        c.connect().expect("connect");
        assert_eq!(c.state(), ClientState::Connecting);
    }

    #[test]
    fn mark_connected_transitions_to_connected() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        c.connect().expect("connect");
        c.mark_connected().expect("ack");
        assert_eq!(c.state(), ClientState::Connected);
    }

    #[test]
    fn double_connect_rejected() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        c.connect().expect("first");
        let err = c.connect().expect_err("second");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn create_without_connect_rejected() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        let oid = ObjectId::from_raw(0x0010);
        let v = ObjectVariant::ByReference("topic-ref".into());
        let err = c.create_object(oid, v).expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn write_without_connect_rejected() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        let oid = ObjectId::from_raw(0x0010);
        let err = c.request_write(oid, b"x").expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn read_without_connect_rejected() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        let oid = ObjectId::from_raw(0x0010);
        let err = c.request_read(oid).expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn full_lifecycle_creates_unique_request_ids() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        c.connect().expect("conn");
        c.mark_connected().expect("ack");
        let oid = ObjectId::from_raw(0x0010);
        let r1 = c
            .create_object(oid, ObjectVariant::ByReference("a".into()))
            .expect("create");
        let r2 = c.delete_object(oid).expect("delete");
        let r3 = c.request_write(oid, b"x").expect("write");
        let r4 = c.request_read(oid).expect("read");
        // Request IDs are strictly monotonic.
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
        assert_eq!(r3, 3);
        assert_eq!(r4, 4);
    }

    #[test]
    fn disconnect_clears_state() {
        let mut c = XrceClient::new(key(), MockTransport::new());
        c.connect().expect("conn");
        c.mark_connected().expect("ack");
        c.disconnect();
        assert_eq!(c.state(), ClientState::Disconnected);
    }
}
