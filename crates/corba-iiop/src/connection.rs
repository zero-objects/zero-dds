// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP-Connection — TCP-Stream-Wrapper mit GIOP-Message-IO.

use std::io::{BufReader, BufWriter};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use zerodds_cdr::Endianness;
use zerodds_corba_ccm::orb_extensions::{
    ClientInterceptionPoint, InterceptorRegistry, ServerInterceptionPoint,
};
use zerodds_corba_giop::{Message, Version};

use crate::error::IiopError;
use crate::framing::{read_giop_message, write_giop_message};

/// IIOP-Connection — wraps eine `TcpStream` + Buffered-Reader/Writer.
///
/// `Connection` ist nicht `Clone` — fuer Sharing-Szenarien (z.B.
/// pro-Connection-Worker-Thread mit getrennten Reader/Writer-Halves)
/// nutzt der Caller [`std::net::TcpStream::try_clone`] auf dem inneren
/// Stream via `peer_addr()`/`local_addr()` und konstruiert eine
/// neue `Connection`.
pub struct Connection {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    peer: SocketAddr,
    local: SocketAddr,
    /// Spec §16 Portable Interceptors — wenn gesetzt, walked
    /// `write_message`/`read_message` die Pipeline an den
    /// passenden Points (`SendRequest`/`ReceiveReply` Client-Side
    /// bzw. `ReceiveRequest`/`SendReply` Server-Side).
    interceptors: Option<Arc<InterceptorRegistry>>,
}

impl Connection {
    /// Wraps eine bestehende `TcpStream`.
    ///
    /// # Errors
    /// IO-Fehler beim Auslesen der Peer/Local-Addresses oder beim
    /// `try_clone` fuer Reader+Writer-Half.
    pub fn from_stream(stream: TcpStream) -> Result<Self, IiopError> {
        let peer = stream.peer_addr()?;
        let local = stream.local_addr()?;
        // Wir splitten via try_clone, sodass Reader und Writer
        // unabhaengige BufReader/BufWriter haben koennen. TCP-Stream-
        // Halves teilen denselben Socket; das ist Standard-Pattern.
        let reader_stream = stream.try_clone()?;
        let writer_stream = stream;
        Ok(Self {
            reader: BufReader::new(reader_stream),
            writer: BufWriter::new(writer_stream),
            peer,
            local,
            interceptors: None,
        })
    }

    /// Spec §16.4.x — installiert eine [`InterceptorRegistry`] fuer
    /// diese Connection. Subsequent `write_message`/`read_message`
    /// walken die Pipeline an den passenden Points.
    #[must_use]
    pub fn with_interceptors(mut self, registry: Arc<InterceptorRegistry>) -> Self {
        self.interceptors = Some(registry);
        self
    }

    /// Liefert die aktive Interceptor-Registry, falls eine installiert
    /// ist.
    #[must_use]
    pub fn interceptors(&self) -> Option<&Arc<InterceptorRegistry>> {
        self.interceptors.as_ref()
    }

    /// Spec §16.4.2 — walked Client-Pipeline am Point `point` mit
    /// Operation-Namen `op`. Wenn keine Registry installiert ist,
    /// no-op.
    pub fn run_client_pipeline(&self, point: ClientInterceptionPoint, op: &str) {
        if let Some(r) = &self.interceptors {
            r.walk_client(point, op);
        }
    }

    /// Spec §16.4.3 — walked Server-Pipeline.
    pub fn run_server_pipeline(&self, point: ServerInterceptionPoint, op: &str) {
        if let Some(r) = &self.interceptors {
            r.walk_server(point, op);
        }
    }

    /// Setzt den Read-Timeout.
    ///
    /// # Errors
    /// IO-Fehler.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), IiopError> {
        self.reader.get_ref().set_read_timeout(timeout)?;
        Ok(())
    }

    /// Setzt den Write-Timeout.
    ///
    /// # Errors
    /// IO-Fehler.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<(), IiopError> {
        self.writer.get_ref().set_write_timeout(timeout)?;
        Ok(())
    }

    /// Aktiviert/deaktiviert TCP-Nodelay (Nagle-Algorithmus).
    ///
    /// Default in CORBA-Deployments ist `true` (kein Nagle), weil
    /// GIOP-Roundtrip-Latency dominanter Faktor ist.
    ///
    /// # Errors
    /// IO-Fehler.
    pub fn set_nodelay(&self, nodelay: bool) -> Result<(), IiopError> {
        self.writer.get_ref().set_nodelay(nodelay)?;
        Ok(())
    }

    /// Liefert die Peer-Address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// Liefert die Local-Address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// Liest eine GIOP-Message vom Stream.
    ///
    /// Walkt die Server-Pipeline an `ReceiveRequest` (fuer eingehende
    /// `Request`-Messages) bzw. die Client-Pipeline an `ReceiveReply`
    /// (fuer eingehende `Reply`-Messages), wenn eine
    /// `InterceptorRegistry` installiert ist (Spec §16.4.2/§16.4.3).
    ///
    /// # Errors
    /// `IiopError::Closed` bei sauberem EOF; sonst `Io`/`Giop`.
    pub fn read_message(&mut self) -> Result<Message, IiopError> {
        let msg = read_giop_message(&mut self.reader)?;
        if let Some(r) = &self.interceptors {
            match &msg {
                Message::Request(req) => {
                    r.walk_server(ServerInterceptionPoint::ReceiveRequest, &req.operation);
                }
                Message::Reply(_) => {
                    // Op-Name ist im Reply nicht enthalten; wir nutzen
                    // den Spec-konformen leeren Marker. Caller mit
                    // Op-Tracking kann `run_client_pipeline` direkt
                    // mit dem Op-Namen aufrufen.
                    r.walk_client(ClientInterceptionPoint::ReceiveReply, "");
                }
                _ => {}
            }
        }
        Ok(msg)
    }

    /// Schreibt eine GIOP-Message auf den Stream.
    ///
    /// Walkt die Client-Pipeline an `SendRequest` (fuer ausgehende
    /// `Request`-Messages) bzw. die Server-Pipeline an `SendReply`
    /// (fuer ausgehende `Reply`-Messages), wenn eine
    /// `InterceptorRegistry` installiert ist (Spec §16.4.2/§16.4.3).
    ///
    /// # Errors
    /// IO- oder GIOP-Fehler.
    pub fn write_message(
        &mut self,
        version: Version,
        endianness: Endianness,
        more_fragments: bool,
        msg: &Message,
    ) -> Result<(), IiopError> {
        if let Some(r) = &self.interceptors {
            match msg {
                Message::Request(req) => {
                    r.walk_client(ClientInterceptionPoint::SendRequest, &req.operation);
                }
                Message::Reply(_) => {
                    r.walk_server(ServerInterceptionPoint::SendReply, "");
                }
                _ => {}
            }
        }
        write_giop_message(&mut self.writer, version, endianness, more_fragments, msg)
    }

    /// Schliesst die Connection ordentlich (TCP-FIN).
    ///
    /// # Errors
    /// IO-Fehler.
    pub fn shutdown(&mut self) -> Result<(), IiopError> {
        // Writer flushen, dann beide Halves shut.
        let _ = std::io::Write::flush(&mut self.writer);
        self.writer.get_ref().shutdown(Shutdown::Both)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use zerodds_corba_giop::{
        CloseConnection, Message, ReplyStatusType, ServiceContextList, TargetAddress,
    };

    fn make_test_pair() -> (Connection, Connection) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();
        let acceptor = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            Connection::from_stream(stream).unwrap()
        });
        let client_stream = TcpStream::connect(server_addr).unwrap();
        let client = Connection::from_stream(client_stream).unwrap();
        let server = acceptor.join().unwrap();
        (client, server)
    }

    #[test]
    fn round_trip_request_reply() {
        let (mut client, mut server) = make_test_pair();
        client.set_nodelay(true).unwrap();
        server.set_nodelay(true).unwrap();

        // Client sendet Request.
        let request = Message::Request(zerodds_corba_giop::Request {
            request_id: 1,
            response_flags: zerodds_corba_giop::ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab]),
            operation: "echo".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4],
        });
        client
            .write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        // Server liest.
        let received = server.read_message().unwrap();
        assert_eq!(received, request);

        // Server antwortet.
        let reply = Message::Reply(zerodds_corba_giop::Reply {
            request_id: 1,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xff],
        });
        server
            .write_message(Version::V1_2, Endianness::Big, false, &reply)
            .unwrap();
        let client_received = client.read_message().unwrap();
        assert_eq!(client_received, reply);
    }

    #[test]
    fn shutdown_propagates_eof_to_peer() {
        let (mut client, mut server) = make_test_pair();
        // Client schliesst.
        client.shutdown().unwrap();
        // Server liest -> Closed.
        let err = server.read_message().unwrap_err();
        assert!(matches!(err, IiopError::Closed));
    }

    // §16 Portable Interceptors Wire-up

    use std::sync::Mutex;
    use zerodds_corba_ccm::orb_extensions::{
        ClientRequestInterceptor, IorInterceptor, ServerRequestInterceptor,
    };

    struct RecordingClient {
        events: Arc<Mutex<Vec<(ClientInterceptionPoint, String)>>>,
    }
    impl ClientRequestInterceptor for RecordingClient {
        fn name(&self) -> &str {
            "recording-client"
        }
        fn intercept(&self, point: ClientInterceptionPoint, op: &str) {
            if let Ok(mut g) = self.events.lock() {
                g.push((point, op.to_string()));
            }
        }
    }

    struct RecordingServer {
        events: Arc<Mutex<Vec<(ServerInterceptionPoint, String)>>>,
    }
    impl ServerRequestInterceptor for RecordingServer {
        fn name(&self) -> &str {
            "recording-server"
        }
        fn intercept(&self, point: ServerInterceptionPoint, op: &str) {
            if let Ok(mut g) = self.events.lock() {
                g.push((point, op.to_string()));
            }
        }
    }

    struct ComponentTagIor;
    impl IorInterceptor for ComponentTagIor {
        fn name(&self) -> &str {
            "tagger"
        }
        fn establish_components(&self) -> Vec<u32> {
            // Spec §13.6 — TAG_ORB_TYPE als Marker-Tag.
            alloc::vec![0x4F4D_4730]
        }
    }

    #[test]
    fn pipeline_walks_client_send_request() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut registry = InterceptorRegistry::new();
        registry.add_client(Arc::new(RecordingClient {
            events: events.clone(),
        }) as Arc<dyn ClientRequestInterceptor>);
        let registry = Arc::new(registry);

        let (client, server) = make_test_pair();
        let mut client = client.with_interceptors(registry.clone());
        let mut server = server;
        client.set_nodelay(true).unwrap();
        server.set_nodelay(true).unwrap();

        let request = Message::Request(zerodds_corba_giop::Request {
            request_id: 7,
            response_flags: zerodds_corba_giop::ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab]),
            operation: "do_work".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![],
        });
        client
            .write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        let _ = server.read_message().unwrap();

        let g = events.lock().unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, ClientInterceptionPoint::SendRequest);
        assert_eq!(g[0].1, "do_work");
    }

    #[test]
    fn pipeline_walks_server_receive_request() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut registry = InterceptorRegistry::new();
        registry.add_server(Arc::new(RecordingServer {
            events: events.clone(),
        }) as Arc<dyn ServerRequestInterceptor>);
        let registry = Arc::new(registry);

        let (client, server) = make_test_pair();
        let mut server = server.with_interceptors(registry.clone());
        let mut client = client;
        client.set_nodelay(true).unwrap();
        server.set_nodelay(true).unwrap();

        let request = Message::Request(zerodds_corba_giop::Request {
            request_id: 9,
            response_flags: zerodds_corba_giop::ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xcd]),
            operation: "echo".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3],
        });
        client
            .write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        let _ = server.read_message().unwrap();

        let g = events.lock().unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, ServerInterceptionPoint::ReceiveRequest);
        assert_eq!(g[0].1, "echo");
    }

    #[test]
    fn ior_interceptor_fires_on_walk_ior() {
        let mut registry = InterceptorRegistry::new();
        registry.add_ior(Arc::new(ComponentTagIor) as Arc<dyn IorInterceptor>);
        let tags = registry.walk_ior();
        assert_eq!(tags, alloc::vec![0x4F4D_4730]);
    }

    #[test]
    fn close_connection_message_round_trip() {
        let (mut client, mut server) = make_test_pair();
        client
            .write_message(
                Version::V1_2,
                Endianness::Big,
                false,
                &Message::CloseConnection(CloseConnection),
            )
            .unwrap();
        let received = server.read_message().unwrap();
        assert!(matches!(received, Message::CloseConnection(_)));
    }
}
