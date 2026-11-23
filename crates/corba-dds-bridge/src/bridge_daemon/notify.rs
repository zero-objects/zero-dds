// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The oneway requests to a notify target (spec §4.4).

#[allow(unused_imports)]
use super::prelude::*;

/// A forward chain longer than this is a misconfiguration (a cycle, or a
/// peer that never stops forwarding), not something to follow forever.
pub(crate) const MAX_FORWARD_HOPS: u32 = 8;

/// How long [`Notifier`] waits for a `LocateReply`, or for the
/// unsolicited `Reply` some peers send even to a oneway request, before
/// giving up and proceeding as if none is coming. Not every ORB answers
/// a `LocateRequest` at all (optional per spec — measured against
/// JacORB and omniORB, `tests/omniorb_e2e.rs`), so this is the
/// observable signal for "this peer does not forward", not an error.
pub(crate) const NOTIFY_READ_TIMEOUT: Duration = Duration::from_millis(500);

/// A oneway GIOP request with `body` to `target` (§4.4); connections are
/// kept per target and opened again after a failure.
///
/// `target` starts as whatever `dds_to_corba.targets` names — often a
/// `corbaloc:`, i.e. an *object key* at a *host:port*, not necessarily
/// the servant's own key. Measured against TAO 2.5
/// (`tests/tao_e2e.rs`, `tests/tao/listener_server.cpp`): TAO's
/// `IORTable` binds a logical name like this to the real,
/// system-assigned key of an activated servant and answers a request
/// against the logical name with `LOCATION_FORWARD` to it — to the
/// oneway `on_value` request itself, not only to a `LocateRequest`,
/// and regardless of `response_flags`; JacORB and omniORB answer the
/// logical name directly, no forward. [`Notifier::resolve`] follows a
/// forward ahead of the first send (GIOP §15.4.5/§15.4.6); the send
/// itself still reads for one anyway.
pub(crate) struct Notifier {
    pub(crate) target: Target,
    pub(crate) operation: String,
    pub(crate) conn: Option<Connection>,
    pub(crate) next_request_id: u32,
    /// `false` until a `LocateRequest` has confirmed `target` is the
    /// real servant — forces resolution before the next send. Cleared
    /// after a connection failure too: the peer may have moved the
    /// object by the time a new connection opens.
    pub(crate) resolved: bool,
}

/// Decodes a `LOCATION_FORWARD`/`LOCATION_FORWARD_PERM` reply body
/// (GIOP §15.4.3.3/§15.4.6.2 — the plain `Object` CDR form: `string
/// type_id; sequence<TaggedProfile> profiles`, in the reply's own byte
/// order, not a further-nested encapsulation) into a [`Target`]. `None`
/// on a decode error or an IOR with no IIOP profile.
pub(crate) fn decode_forward_target(body: &[u8], endianness: Endianness) -> Option<Target> {
    let mut r = zerodds_wire::Reader::new(body, endianness);
    let ior = Ior::decode(&mut r).ok()?;
    ior.profiles.iter().find_map(|p| {
        let iiop = p.as_iiop()?.ok()?;
        Some(Target {
            host: iiop.host,
            port: iiop.port,
            object_key: iiop.object_key,
        })
    })
}

impl Notifier {
    pub(crate) fn ensure_connected(&mut self) -> io::Result<()> {
        if self.conn.is_none() {
            let s = TcpStream::connect((self.target.host.as_str(), self.target.port))?;
            s.set_nodelay(true)?;
            let conn =
                Connection::from_stream(s).map_err(|e| io::Error::other(format!("{e:?}")))?;
            conn.set_read_timeout(Some(NOTIFY_READ_TIMEOUT))
                .map_err(|e| io::Error::other(format!("{e:?}")))?;
            self.conn = Some(conn);
        }
        Ok(())
    }

    pub(crate) fn next_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }

    /// Resolves `self.target` via a `LocateRequest` (GIOP §15.4.5),
    /// following `OBJECT_FORWARD`/`OBJECT_FORWARD_PERM` up to
    /// [`MAX_FORWARD_HOPS`]. A no-op once `self.resolved`.
    ///
    /// # Errors
    /// I/O error opening the connection, or a forward chain past the
    /// hop bound — every other outcome (an `OBJECT_HERE`, a peer that
    /// answers something else, or none at all within
    /// [`NOTIFY_READ_TIMEOUT`]) proceeds with whatever `target` is.
    pub(crate) fn resolve(&mut self, endianness: Endianness) -> io::Result<()> {
        if self.resolved {
            return Ok(());
        }
        for _ in 0..MAX_FORWARD_HOPS {
            self.ensure_connected()?;
            let request_id = self.next_id();
            let lr = Message::LocateRequest(LocateRequest {
                request_id,
                target: TargetAddress::Key(self.target.object_key.clone()),
            });
            let conn = self.conn.as_mut().ok_or_else(|| {
                io::Error::other("connection missing right after ensure_connected")
            })?;
            if conn
                .write_message(Version::V1_2, endianness, false, &lr)
                .is_err()
            {
                // The real send retries the connection itself; resolving
                // it here too would just duplicate that retry.
                self.conn = None;
                return Ok(());
            }
            match self.conn.as_mut().and_then(|c| c.read_message().ok()) {
                Some(Message::LocateReply(reply)) => match reply.locate_status {
                    LocateStatusType::ObjectHere => {
                        self.resolved = true;
                        return Ok(());
                    }
                    LocateStatusType::ObjectForward | LocateStatusType::ObjectForwardPerm => {
                        match decode_forward_target(&reply.body, endianness) {
                            Some(t) => {
                                self.target = t;
                                self.conn = None;
                            }
                            None => {
                                self.resolved = true;
                                return Ok(());
                            }
                        }
                    }
                    LocateStatusType::UnknownObject
                    | LocateStatusType::LocSystemException
                    | LocateStatusType::LocNeedsAddressingMode => {
                        self.resolved = true;
                        return Ok(());
                    }
                },
                _ => {
                    self.resolved = true;
                    return Ok(());
                }
            }
        }
        Err(io::Error::other(format!(
            "LocateRequest forward chain exceeded {MAX_FORWARD_HOPS} hops for object key {:02x?}",
            self.target.object_key
        )))
    }

    /// Sends one oneway GIOP request, resolving `target` first
    /// ([`Notifier::resolve`]) and following an unsolicited
    /// `LOCATION_FORWARD`/`LOCATION_FORWARD_PERM` `Reply` to the send
    /// itself (measured against TAO — see the struct doc) up to
    /// [`MAX_FORWARD_HOPS`].
    ///
    /// # Errors
    /// I/O error; a forward chain past the hop bound; or a `Reply` with
    /// any other status — a sample this cannot deliver must be
    /// reported, never silently treated as delivered.
    pub(crate) fn send(&mut self, body: &[u8], endianness: Endianness) -> io::Result<()> {
        self.resolve(endianness)?;
        let mut reconnected = false;
        for _ in 0..MAX_FORWARD_HOPS {
            self.ensure_connected()?;
            let request_id = self.next_id();
            let req = Message::Request(Request {
                request_id,
                response_flags: ResponseFlags::from_response_expected(false),
                target: TargetAddress::Key(self.target.object_key.clone()),
                operation: self.operation.clone(),
                requesting_principal: None,
                service_context: ServiceContextList::default(),
                body: body.to_vec(),
            });
            let conn = self.conn.as_mut().ok_or_else(|| {
                io::Error::other("connection missing right after ensure_connected")
            })?;
            if let Err(e) = conn.write_message(Version::V1_2, endianness, false, &req) {
                if reconnected {
                    return Err(io::Error::other(format!("{e:?}")));
                }
                reconnected = true;
                log(
                    "notify_failed",
                    &[("err", &format!("{e:?}")), ("retry", "reconnect")],
                );
                self.conn = None;
                self.resolved = false;
                self.resolve(endianness)?;
                continue;
            }
            let read = self.conn.as_mut().map(Connection::read_message);
            match read {
                Some(Ok(Message::Reply(reply))) => match reply.reply_status {
                    ReplyStatusType::LocationForward | ReplyStatusType::LocationForwardPerm => {
                        let Some(t) = decode_forward_target(&reply.body, endianness) else {
                            return Err(io::Error::other(
                                "LOCATION_FORWARD reply body did not decode as an object reference",
                            ));
                        };
                        self.target = t;
                        self.conn = None;
                        // `resolved` stays true: this forward already
                        // names where the object lives now.
                        continue;
                    }
                    other => {
                        return Err(io::Error::other(format!(
                            "peer replied {other:?} to a oneway {}",
                            self.operation
                        )));
                    }
                },
                // The peer closed the connection: the oneway went out
                // before the close; the next sample opens a new one
                // instead of writing into this one.
                Some(Err(e)) if closed(&e) => {
                    self.conn = None;
                    self.resolved = false;
                    return Ok(());
                }
                // No reply within the timeout, or a message this
                // connection has no other use for: the ordinary oneway
                // case.
                _ => return Ok(()),
            }
        }
        Err(io::Error::other(format!(
            "LOCATION_FORWARD chain exceeded {MAX_FORWARD_HOPS} hops for object key {:02x?}",
            self.target.object_key
        )))
    }
}

/// Whether a read error means the peer closed the connection.
pub(crate) fn closed(e: &zerodds_corba_iiop::IiopError) -> bool {
    match e {
        zerodds_corba_iiop::IiopError::Closed => true,
        zerodds_corba_iiop::IiopError::Io(io) => matches!(
            io.kind(),
            io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::UnexpectedEof
        ),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::bridge_daemon::config::tests::load;

    fn a_notifier(port: u16, object_key: &[u8]) -> Notifier {
        Notifier {
            target: Target {
                host: "127.0.0.1".into(),
                port,
                object_key: object_key.to_vec(),
            },
            operation: "on_value".into(),
            conn: None,
            next_request_id: 1,
            resolved: false,
        }
    }

    /// The plain `Object` CDR form a `LOCATE_REPLY`/`Reply` forward body
    /// carries (see `decode_forward_target`'s doc).
    fn forward_body(host: &str, port: u16, object_key: &[u8], e: Endianness) -> Vec<u8> {
        let profile = TaggedProfile::iiop(
            &IiopProfileBody::new(
                IiopVersion::new(1, 2),
                host.to_string(),
                port,
                object_key.to_vec(),
            ),
            e,
        )
        .unwrap();
        let ior = Ior {
            type_id: "IDL:zerodds_test/Listener:1.0".into(),
            profiles: vec![profile],
        };
        let mut w = zerodds_wire::Writer::new(e);
        ior.encode(&mut w).unwrap();
        w.finish()
    }

    fn accept_one(listener: &TcpListener) -> Connection {
        let (stream, _) = listener.accept().unwrap();
        Connection::from_stream(stream).unwrap()
    }

    /// `Notifier::resolve` follows a `LocateReply`'s `OBJECT_FORWARD` to
    /// the real object key, then confirms it with a plain `OBJECT_HERE`
    /// on the reconnect — the steady state TAO's `IORTable` measured.
    #[test]
    fn resolve_follows_object_forward_then_confirms_object_here() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let real_key = b"RealListener".to_vec();
        let server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                assert_eq!(lr.target, TargetAddress::Key(b"Listener".to_vec()));
                let body = forward_body("127.0.0.1", addr.port(), &real_key, e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectForward,
                        body,
                    }),
                )
                .unwrap();

                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                assert_eq!(lr.target, TargetAddress::Key(real_key));
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectHere,
                        body: Vec::new(),
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(addr.port(), b"Listener");
        notifier.resolve(Endianness::Big).unwrap();
        assert_eq!(notifier.target.object_key, real_key);
        assert!(notifier.resolved);
        server.join().unwrap();
    }

    /// A peer that answers `OBJECT_FORWARD` past [`MAX_FORWARD_HOPS`] is a
    /// misconfiguration (a forwarding loop) — a loud error, not an
    /// infinite retry.
    #[test]
    fn resolve_gives_up_loudly_past_the_forward_hop_bound() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..MAX_FORWARD_HOPS {
                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                let body = forward_body("127.0.0.1", addr.port(), b"Loop", e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectForward,
                        body,
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(addr.port(), b"Loop");
        let err = notifier.resolve(Endianness::Big).unwrap_err();
        assert!(err.to_string().contains("hops"), "{err}");
        server.join().unwrap();
    }

    /// TAO answers `LOCATION_FORWARD` to the oneway request itself, not
    /// only to a `LocateRequest`, regardless of `response_flags` —
    /// `Notifier::send` must read for it and resend to the real servant.
    #[test]
    fn send_follows_location_forward_on_the_reply_to_the_oneway_itself() {
        let real_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let real_addr = real_listener.local_addr().unwrap();
        let real_key = b"RealListener".to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        let real_server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&real_listener);
                let (msg, _e, _v) = conn.read_message_full().unwrap();
                let Message::Request(req) = msg else {
                    panic!("expected a Request, got {:?}", msg.message_type())
                };
                assert_eq!(req.target, TargetAddress::Key(real_key));
                tx.send((req.operation, req.body)).unwrap();
            }
        });

        let logical_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let logical_addr = logical_listener.local_addr().unwrap();
        let logical_server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&logical_listener);
                // The LocateRequest sees no forward — it is the oneway
                // send itself that gets one, as TAO's IORTable does.
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectHere,
                        body: Vec::new(),
                    }),
                )
                .unwrap();

                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::Request(req) = msg else {
                    panic!("expected a Request, got {:?}", msg.message_type())
                };
                let body = forward_body("127.0.0.1", real_addr.port(), &real_key, e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::Reply(zerodds_corba_giop::Reply {
                        request_id: req.request_id,
                        reply_status: ReplyStatusType::LocationForward,
                        service_context: ServiceContextList::default(),
                        body,
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(logical_addr.port(), b"Listener");
        notifier.send(&[7, 8, 9], Endianness::Big).unwrap();
        assert_eq!(notifier.target.object_key, real_key);

        let (operation, body) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(operation, "on_value");
        assert_eq!(body, vec![7, 8, 9]);

        logical_server.join().unwrap();
        real_server.join().unwrap();
    }

    /// Any reply status other than `LOCATION_FORWARD[_PERM]` to the
    /// oneway send is reported, never treated as a delivered sample.
    #[test]
    fn send_reports_an_unexpected_reply_status_instead_of_treating_it_as_delivered() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut conn = accept_one(&listener);
            let (msg, e, v) = conn.read_message_full().unwrap();
            let Message::LocateRequest(lr) = msg else {
                panic!("expected a LocateRequest, got {:?}", msg.message_type())
            };
            conn.write_message(
                v,
                e,
                false,
                &Message::LocateReply(LocateReply {
                    request_id: lr.request_id,
                    locate_status: LocateStatusType::ObjectHere,
                    body: Vec::new(),
                }),
            )
            .unwrap();

            let (msg, e, v) = conn.read_message_full().unwrap();
            let Message::Request(req) = msg else {
                panic!("expected a Request, got {:?}", msg.message_type())
            };
            conn.write_message(
                v,
                e,
                false,
                &Message::Reply(zerodds_corba_giop::Reply {
                    request_id: req.request_id,
                    reply_status: ReplyStatusType::SystemException,
                    service_context: ServiceContextList::default(),
                    body: Vec::new(),
                }),
            )
            .unwrap();
        });

        let mut notifier = a_notifier(addr.port(), b"X");
        let err = notifier.send(&[1], Endianness::Big).unwrap_err();
        assert!(err.to_string().contains("SystemException"), "{err}");
        server.join().unwrap();
    }

    /// §4.4: a target that closed the connection gets the next sample over
    /// a new one.
    #[test]
    fn corbabridge_4_4_a_closed_connection_is_opened_again() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for _ in 0..2 {
                let mut conn = accept_one(&listener);
                conn.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                while let Ok(m) = conn.read_message() {
                    if let Message::Request(r) = m {
                        bodies.push(r.body);
                        break;
                    }
                }
                // closes the connection
            }
            bodies
        });
        let mut n = a_notifier(port, b"Listener");
        n.send(&[1, 0, 0, 0], Endianness::Little).unwrap();
        // the peer closes; the first send saw the close
        std::thread::sleep(Duration::from_millis(100));
        n.send(&[2, 0, 0, 0], Endianness::Little).unwrap();
        assert_eq!(
            server.join().unwrap(),
            vec![vec![1, 0, 0, 0], vec![2, 0, 0, 0]]
        );
    }
}
