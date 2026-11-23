// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP connection — TCP stream wrapper with GIOP message I/O.

use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use zerodds_corba_giop::{Message, Version};
use zerodds_wire::Endianness;

use crate::error::IiopError;
use crate::framing::{
    read_giop_message_ziop, write_giop_message, write_giop_message_fragmented,
    write_giop_message_ziop,
};
use zerodds_corba_giop::ziop::{ZiopCodec, ZiopSend};

/// Read+Write+Send stream for the TLS transport (rustls `StreamOwned` over a
/// client or server connection). Unlike TCP, a TLS stream cannot be split into
/// independent reader/writer halves via `try_clone` (the rustls session is
/// shared state), hence a single object for both.
#[cfg(feature = "tls")]
pub trait TlsStream: Read + Write + Send {}
#[cfg(feature = "tls")]
impl<T: Read + Write + Send> TlsStream for T {}

/// Transport substrate of a [`Connection`]: buffered TCP halves (standard
/// IIOP), a single TLS stream (SSLIOP, §15.x / OMG Security) or buffered
/// Unix-domain-socket halves (UIOP — a ZeroDDS vendor transport for same-host IPC).
enum Transport {
    /// Plain TCP with independently buffered reader/writer halves (try_clone).
    Tcp {
        reader: BufReader<TcpStream>,
        writer: BufWriter<TcpStream>,
    },
    /// TLS stream (rustls) + cloned raw socket for timeouts/shutdown.
    #[cfg(feature = "tls")]
    Tls {
        stream: Box<dyn TlsStream>,
        sock: TcpStream,
    },
    /// Unix-domain socket (UIOP), buffered halves via `try_clone` like TCP.
    #[cfg(unix)]
    Uds {
        reader: BufReader<std::os::unix::net::UnixStream>,
        writer: BufWriter<std::os::unix::net::UnixStream>,
    },
}

impl Transport {
    /// `&mut dyn Read` for the receive path.
    fn reader(&mut self) -> &mut dyn Read {
        match self {
            Self::Tcp { reader, .. } => reader,
            #[cfg(feature = "tls")]
            Self::Tls { stream, .. } => stream.as_mut(),
            #[cfg(unix)]
            Self::Uds { reader, .. } => reader,
        }
    }
    /// `&mut dyn Write` for the send path.
    fn writer(&mut self) -> &mut dyn Write {
        match self {
            Self::Tcp { writer, .. } => writer,
            #[cfg(feature = "tls")]
            Self::Tls { stream, .. } => stream.as_mut(),
            #[cfg(unix)]
            Self::Uds { writer, .. } => writer,
        }
    }
    fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp { writer, .. } => writer.get_ref().set_read_timeout(t),
            #[cfg(feature = "tls")]
            Self::Tls { sock, .. } => sock.set_read_timeout(t),
            #[cfg(unix)]
            Self::Uds { writer, .. } => writer.get_ref().set_read_timeout(t),
        }
    }
    fn set_write_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp { writer, .. } => writer.get_ref().set_write_timeout(t),
            #[cfg(feature = "tls")]
            Self::Tls { sock, .. } => sock.set_write_timeout(t),
            #[cfg(unix)]
            Self::Uds { writer, .. } => writer.get_ref().set_write_timeout(t),
        }
    }
    fn set_nodelay(&self, nodelay: bool) -> std::io::Result<()> {
        match self {
            Self::Tcp { writer, .. } => writer.get_ref().set_nodelay(nodelay),
            #[cfg(feature = "tls")]
            Self::Tls { sock, .. } => sock.set_nodelay(nodelay),
            // AF_UNIX has no Nagle/TCP_NODELAY → no-op.
            #[cfg(unix)]
            Self::Uds { .. } => Ok(()),
        }
    }
    fn shutdown(&self) -> std::io::Result<()> {
        match self {
            Self::Tcp { writer, .. } => writer.get_ref().shutdown(Shutdown::Both),
            #[cfg(feature = "tls")]
            Self::Tls { sock, .. } => sock.shutdown(Shutdown::Both),
            #[cfg(unix)]
            Self::Uds { writer, .. } => writer.get_ref().shutdown(Shutdown::Both),
        }
    }
}

/// IIOP connection — wraps a transport (buffered TCP or TLS) + GIOP I/O.
///
/// `Connection` is not `Clone`. For plain TCP, [`Self::from_stream`] splits
/// reader/writer halves via `try_clone`; TLS connections
/// (`Self::from_tls_stream`, feature `tls`) use a single stream.
pub struct Connection {
    transport: Transport,
    peer: SocketAddr,
    local: SocketAddr,
    /// The compressors ZIOP messages are read with (CORBA 3.3 Part 2
    /// §12); `None` refuses them.
    ziop: Option<Arc<dyn ZiopCodec>>,
}

impl Connection {
    /// Wraps an existing `TcpStream`.
    ///
    /// # Errors
    /// I/O error while reading the peer/local addresses or during
    /// the `try_clone` for the reader+writer half.
    pub fn from_stream(stream: TcpStream) -> Result<Self, IiopError> {
        let peer = stream.peer_addr()?;
        let local = stream.local_addr()?;
        // We split via try_clone so that reader and writer can have
        // independent BufReader/BufWriter. TCP stream halves share the
        // same socket; this is the standard pattern.
        let reader_stream = stream.try_clone()?;
        let writer_stream = stream;
        Ok(Self {
            transport: Transport::Tcp {
                reader: BufReader::new(reader_stream),
                writer: BufWriter::new(writer_stream),
            },
            peer,
            local,
            ziop: None,
        })
    }

    /// Wraps an already-handshaked TLS stream (rustls `StreamOwned`) +
    /// its associated raw `TcpStream` (for timeouts/shutdown) as an SSLIOP
    /// connection. `sock` should be a `try_clone` of the socket used by the
    /// stream.
    ///
    /// # Errors
    /// I/O error while reading the peer/local addresses.
    #[cfg(feature = "tls")]
    pub fn from_tls_stream(stream: Box<dyn TlsStream>, sock: TcpStream) -> Result<Self, IiopError> {
        let peer = sock.peer_addr()?;
        let local = sock.local_addr()?;
        Ok(Self {
            transport: Transport::Tls { stream, sock },
            peer,
            local,
            ziop: None,
        })
    }

    /// Wraps an existing `UnixStream` as a UIOP connection. Reader/writer
    /// are split into buffered halves via `try_clone`, as with TCP. UDS have
    /// no IP endpoints — [`Self::peer_addr`]/[`Self::local_addr`] return a
    /// `127.0.0.1:0` placeholder (the UDS identity is the socket path, which
    /// lives in the connector pool key).
    ///
    /// # Errors
    /// I/O error during the `try_clone` for the reader+writer half.
    #[cfg(unix)]
    pub fn from_unix_stream(stream: std::os::unix::net::UnixStream) -> Result<Self, IiopError> {
        let reader_stream = stream.try_clone()?;
        let writer_stream = stream;
        let placeholder = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0);
        Ok(Self {
            transport: Transport::Uds {
                reader: BufReader::new(reader_stream),
                writer: BufWriter::new(writer_stream),
            },
            peer: placeholder,
            local: placeholder,
            ziop: None,
        })
    }

    /// Sets the read timeout.
    ///
    /// # Errors
    /// I/O error.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), IiopError> {
        self.transport.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Sets the write timeout.
    ///
    /// # Errors
    /// I/O error.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<(), IiopError> {
        self.transport.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Enables/disables TCP nodelay (Nagle algorithm).
    ///
    /// The default in CORBA deployments is `true` (no Nagle), because
    /// GIOP round-trip latency is the dominant factor.
    ///
    /// # Errors
    /// I/O error.
    pub fn set_nodelay(&self, nodelay: bool) -> Result<(), IiopError> {
        self.transport.set_nodelay(nodelay)?;
        Ok(())
    }

    /// Returns the peer address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// Returns the local address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// Reads a GIOP message from the stream.
    ///
    /// The connection frames messages only. The portable interceptors
    /// of CORBA 3.3 Part 1 §16 run in the request path of the ORB
    /// (`zerodds-corba-interop`), where the request, its reply and the
    /// dispatch outcome are known.
    ///
    /// # Errors
    /// `IiopError::Closed` on clean EOF; otherwise `Io`/`Giop`.
    pub fn read_message(&mut self) -> Result<Message, IiopError> {
        Ok(self.read_message_with_endianness()?.0)
    }

    /// Like [`Self::read_message`], but additionally returns the on-wire byte
    /// order of the received message — needed to decode the opaque
    /// Request/Reply `body` (CDR-encoded operation args) in the correct order
    /// (CDR is "receiver makes it right", Spec §15.4.1 / §15.3.1).
    ///
    /// # Errors
    /// `IiopError::Closed` on clean EOF; otherwise `Io`/`Giop`.
    pub fn read_message_with_endianness(&mut self) -> Result<(Message, Endianness), IiopError> {
        let (msg, endianness, _version) = self.read_message_full()?;
        Ok((msg, endianness))
    }

    /// Like [`Self::read_message_with_endianness`], but additionally returns the
    /// **GIOP version** of the received message — so the server replies in the
    /// request's version (§15.4.1) rather than always 1.2.
    ///
    /// # Errors
    /// `IiopError::Closed` on clean EOF; otherwise `Io`/`Giop`.
    pub fn read_message_full(&mut self) -> Result<(Message, Endianness, Version), IiopError> {
        let ziop = self.ziop.clone();
        read_giop_message_ziop(self.transport.reader(), ziop.as_deref())
    }

    /// The compressors this connection reads ZIOP messages with (CORBA 3.3
    /// Part 2 §12.1); `None` (the default) refuses a ZIOP message with
    /// `InvalidMagic("ZIOP")`. Set it when this side declared ZIOP to the
    /// peer, so that the peer may send it ZIOP messages (§12.2).
    pub fn set_ziop_codec(&mut self, codec: Option<Arc<dyn ZiopCodec>>) {
        self.ziop = codec;
    }

    /// `true` when ZIOP messages are read on this connection.
    #[must_use]
    pub fn reads_ziop(&self) -> bool {
        self.ziop.is_some()
    }

    /// Writes `msg` as ZIOP messages where `send` allows it, fragmented
    /// when `max_fragment_payload` is given (CORBA 3.3 Part 2 §12.1,
    /// [`write_giop_message_ziop`]). Returns how many messages went out as
    /// ZIOP messages.
    ///
    /// # Errors
    /// I/O or GIOP error; `Giop(Malformed)` when the compressor fails.
    pub fn write_message_ziop(
        &mut self,
        version: Version,
        endianness: Endianness,
        msg: &Message,
        max_fragment_payload: Option<usize>,
        send: &ZiopSend,
        codec: &dyn ZiopCodec,
    ) -> Result<usize, IiopError> {
        write_giop_message_ziop(
            self.transport.writer(),
            version,
            endianness,
            msg,
            max_fragment_payload,
            send,
            codec,
        )
    }

    /// Writes a GIOP message to the stream.
    ///
    /// # Errors
    /// I/O or GIOP error.
    pub fn write_message(
        &mut self,
        version: Version,
        endianness: Endianness,
        more_fragments: bool,
        msg: &Message,
    ) -> Result<(), IiopError> {
        write_giop_message(
            self.transport.writer(),
            version,
            endianness,
            more_fragments,
            msg,
        )
    }

    /// Like [`write_message`](Self::write_message), but fragments the message
    /// (§15.4.9) if its body exceeds `max_fragment_payload` — for very large
    /// payloads or peers with a `giopMaxMsgSize` limit. Bodies ≤ the threshold,
    /// non-fragmentable types and GIOP < 1.2 go out unfragmented.
    ///
    /// # Errors
    /// Encode/I/O error.
    pub fn write_message_fragmented(
        &mut self,
        version: Version,
        endianness: Endianness,
        msg: &Message,
        max_fragment_payload: usize,
    ) -> Result<(), IiopError> {
        write_giop_message_fragmented(
            self.transport.writer(),
            version,
            endianness,
            msg,
            max_fragment_payload,
        )
    }

    /// Closes the connection cleanly (TCP FIN).
    ///
    /// # Errors
    /// I/O error.
    pub fn shutdown(&mut self) -> Result<(), IiopError> {
        // Flush the writer, then shut down both halves.
        let _ = std::io::Write::flush(self.transport.writer());
        self.transport.shutdown()?;
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

        // Client sends a request.
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
        // Server reads.
        let received = server.read_message().unwrap();
        assert_eq!(received, request);

        // Server replies.
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
        // Client closes.
        client.shutdown().unwrap();
        // Server reads -> Closed.
        let err = server.read_message().unwrap_err();
        assert!(matches!(err, IiopError::Closed));
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
