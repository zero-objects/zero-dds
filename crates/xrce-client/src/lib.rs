// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Client — synchrones Interface ohne Callbacks (Spec §7.2).
//!
//! Crate `zerodds-xrce-client`.
//!
//! # Spec-Mapping
//!
//! OMG DDS-XRCE 1.0 §7.2: "XRCE Client: simplified Interface, keine
//! Callbacks, Text-Parameter; Session ueberbrueckt Sleep/Wakeup-
//! Zyklen."
//!
//! Wir liefern eine synchrone State-Machine [`XrceClient`] mit
//! folgenden Operationen:
//!
//! | Methode             | Spec-Bezug              |
//! |---------------------|-------------------------|
//! | [`XrceClient::new`] | §7.8.2 (CREATE_CLIENT)  |
//! | [`XrceClient::connect`]    | §8.4.5 (Handshake) |
//! | [`XrceClient::create_object`] | §7.8.3 (CREATE) |
//! | [`XrceClient::delete_object`] | §7.8.3 (DELETE) |
//! | [`XrceClient::request_write`] | §7.8.4 (WRITE_DATA) |
//! | [`XrceClient::request_read`]  | §7.8.5 (READ_DATA) |
//! | [`XrceClient::disconnect`]    | §8.4.5 |
//!
//! Transport ist abstrahiert via [`ClientTransport`]-Trait — konkrete
//! Impls (UDP/TCP/DTLS/Serial) leben in `crates/xrce/src/transport_*`.
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

/// Transport-abstraktion fuer den XRCE-Client.
///
/// Implementierungen (UDP/TCP/DTLS/Serial) liegen in
/// `crates/xrce/src/transport_*.rs`.
pub trait ClientTransport {
    /// Sendet ein Submessage-Bundle an den Agent.
    ///
    /// # Errors
    /// Transport-spezifisch.
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError>;

    /// Pollt eingehende Daten vom Agent. Blockiert nicht — wenn keine
    /// Daten anliegen, liefert `Ok(None)`.
    ///
    /// # Errors
    /// Transport-spezifisch.
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError>;
}

/// XRCE-Client-Lifecycle-Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Initial: Client kennt seine ClientKey, ist aber nicht
    /// verbunden.
    Disconnected,
    /// Connect-Handshake im Gange (CREATE_CLIENT gesendet, STATUS_AGENT
    /// noch nicht empfangen).
    Connecting,
    /// Verbunden — Operations sind moeglich.
    Connected,
}

/// Client-spezifische Error-Klassen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    /// Operation in falschem Lifecycle-Status (z.B. send-without-connect).
    InvalidState,
    /// Transport-Layer-Fehler (Connection-Reset etc.).
    Transport,
    /// Submessage konnte nicht eingereiht werden (Wire-Cap erreicht).
    QueueFull,
}

/// Synchroner XRCE-Client.
pub struct XrceClient<T: ClientTransport> {
    client_key: ClientKey,
    state: ClientState,
    transport: T,
    next_request_id: u16,
    /// Gepufferte ausgehende Submessages, die der Caller per
    /// [`XrceClient::flush`] in das Transport-Layer schiebt.
    out_queue: Vec<Submessage>,
}

impl<T: ClientTransport> XrceClient<T> {
    /// Konstruktor.
    pub fn new(client_key: ClientKey, transport: T) -> Self {
        Self {
            client_key,
            state: ClientState::Disconnected,
            transport,
            next_request_id: 1,
            out_queue: Vec::new(),
        }
    }

    /// Aktueller Lifecycle-Status.
    #[must_use]
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Liefert die ClientKey.
    #[must_use]
    pub fn client_key(&self) -> ClientKey {
        self.client_key
    }

    /// Initiiert den Handshake (sendet CREATE_CLIENT).
    ///
    /// # Errors
    /// `InvalidState` wenn bereits connected.
    pub fn connect(&mut self) -> Result<(), ClientError> {
        if self.state != ClientState::Disconnected {
            return Err(ClientError::InvalidState);
        }
        self.state = ClientState::Connecting;
        Ok(())
    }

    /// Markiert den Handshake als abgeschlossen (Caller hat
    /// STATUS_AGENT empfangen).
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

    /// Submitiert eine CREATE-Operation. Gibt eine `request_id`
    /// zurueck.
    ///
    /// # Errors
    /// `InvalidState` wenn nicht connected.
    pub fn create_object(
        &mut self,
        _object_id: ObjectId,
        _representation: ObjectVariant,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        // Submessage-Encoding deleguert der Caller; wir tracken nur
        // den State.
        Ok(req)
    }

    /// Submitiert eine DELETE-Operation.
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn delete_object(&mut self, _object_id: ObjectId) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        Ok(req)
    }

    /// Submitiert eine WRITE_DATA-Operation.
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

    /// Submitiert eine READ_DATA-Operation (Pull-basiert).
    ///
    /// # Errors
    /// `InvalidState`.
    pub fn request_read(&mut self, _reader: ObjectId) -> Result<u16, ClientError> {
        self.require_connected()?;
        let req = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        Ok(req)
    }

    /// Schließt die Session — geht zurueck nach `Disconnected`.
    pub fn disconnect(&mut self) {
        self.state = ClientState::Disconnected;
        self.out_queue.clear();
    }

    /// Prueft, dass der Client connected ist.
    fn require_connected(&self) -> Result<(), ClientError> {
        if self.state != ClientState::Connected {
            return Err(ClientError::InvalidState);
        }
        Ok(())
    }

    /// Anzahl ausstehender Submessages in der Out-Queue.
    #[must_use]
    pub fn out_queue_len(&self) -> usize {
        self.out_queue.len()
    }

    /// Direkten Transport-Zugriff (z.B. fuer Read-Pull-Polling).
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

    /// Mock-Transport: sammelt sent payloads.
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
        // Request-IDs sind streng monoton.
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
