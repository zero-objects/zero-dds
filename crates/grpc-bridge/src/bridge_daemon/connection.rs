// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The listener and one HTTP/2 connection (spec §4.1, §9.2): accept, TLS with
//! ALPN, the frame loop, keepalive, the drain at shutdown.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn serve(
    cfg: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    once: bool,
    idle_timeout_ms: u64,
    metrics: &DaemonMetrics,
    participants: &[Participant],
    live: &zerodds_bridge_security::LiveSecurity,
    tracer: &Tracer,
) -> Result<Instant, DaemonError> {
    let listener = TcpListener::bind(&cfg.bind)
        .map_err(|e| DaemonError::new(2, format!("bind {}: {}", cfg.bind, e)))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    if idle_timeout_ms > 0 {
        // For test mode: stop accepting after timeout.
        let sd = Arc::clone(&shutdown);
        let dur = Duration::from_millis(idle_timeout_ms);
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            sd.store(true, Ordering::SeqCst);
        });
    }

    event(Level::Info, "listening")
        .text("addr", &cfg.bind)
        .emit();
    zerodds_dcps::cli::notify_service_manager("READY=1");

    listener
        .set_nonblocking(true)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    // One thread per connection: a client that keeps its connection open
    // (every gRPC client does) held the accept loop until its 5 s read
    // timeout, and each further client waited behind it.
    let mut accepted = 0usize;
    // When the accept loop saw the stop: the start of the drain (§9.2).
    let mut stopped_at = Instant::now();
    std::thread::scope(|scope| {
        // Moved in: the socket closes when the accept loop ends.
        let listener = listener;
        while !shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    event(Level::Debug, "accept")
                        .text("peer", &peer.to_string())
                        .emit();
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    metrics.base.connections_total.inc();
                    metrics.base.connections_active.inc();
                    let env = ConnEnv {
                        cfg,
                        metrics,
                        participants,
                        live,
                        shutdown: &shutdown,
                        tracer,
                    };
                    let conn = scope.spawn(move || {
                        // The TLS configuration at accept time: a SIGHUP
                        // applies to the next connection, an open one keeps
                        // its session.
                        let served = match stream.try_clone() {
                            Err(e) => Err(e),
                            Ok(ctl) => match live.current().tls.clone() {
                                Some(tls) => serve_tls(stream, &ctl, tls, &env),
                                None => handle_connection(
                                    stream,
                                    &ctl,
                                    &env,
                                    &participants[0],
                                    None,
                                    false,
                                ),
                            },
                        };
                        if let Err(e) = served {
                            metrics.base.errors_total.inc();
                            event(Level::Error, "conn_error")
                                .text("peer", &peer.to_string())
                                .text("msg", &e.to_string())
                                .emit();
                        }
                        metrics.base.connections_active.dec();
                    });
                    accepted += 1;
                    if once {
                        let _ = conn.join();
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    event(Level::Error, "accept_error")
                        .text("msg", &e.to_string())
                        .emit();
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        // The accept loop has seen the stop; the connections now send
        // GOAWAY and drain (§9.2). The listening socket closes at once: a
        // client that connects during the drain is refused and can turn
        // to another server, instead of waiting in the backlog.
        stopped_at = Instant::now();
        drop(listener);
        zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    });
    event(Level::Info, "shutdown")
        .num("connections", accepted as u64)
        .emit();
    Ok(stopped_at)
}

/// §7.1: the TLS handshake, then the HTTP/2 connection over it (ALPN
/// `h2`) with the client certificate's subject for `auth.mode: mtls`; a
/// connection that negotiated `http/1.1`, or none and sends an HTTP/1.1
/// request line while `grpc.tls.alpn` offers `http/1.1`, is the HTTP/1.1
/// IDL endpoint of §5.2.
pub(crate) fn serve_tls(
    stream: TcpStream,
    ctl: &TcpStream,
    tls: Arc<rustls::ServerConfig>,
    env: &ConnEnv<'_>,
) -> std::io::Result<()> {
    let conn = rustls::ServerConnection::new(tls).map_err(std::io::Error::other)?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    while tls.conn.is_handshaking() {
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| std::io::Error::new(e.kind(), format!("tls handshake: {e}")))?;
    }
    let subject = extract_mtls_subject(&tls.conn);
    let peer = peer_of(ctl);
    // §1 L6: the tenant of the certificate, for the connection's life.
    let part = participant_of(env.participants, subject.as_ref());
    let http1_offered = env.cfg.tls_alpn.iter().any(|p| p == "http/1.1");
    // §4.1, RFC 9113 §3.2: HTTP/2 over TLS is negotiated with ALPN `h2`.
    // A client that offered only protocols the listener does not offer
    // failed the handshake (no_application_protocol).
    let alpn = tls.conn.alpn_protocol().map(<[u8]>::to_vec);
    match alpn.as_deref() {
        Some(b"h2") => {}
        Some(b"http/1.1") => return serve_http1(tls, &[], env, part, subject, &peer),
        _ => {
            // No ALPN: an HTTP/1.1 request line is served as one (§5.2);
            // HTTP/2 needs `h2`.
            let mut start = [0u8; 4];
            tls.read_exact(&mut start)?;
            if http1_offered && is_http1_start(&start) {
                return serve_http1(tls, &start, env, part, subject, &peer);
            }
            event(Level::Warn, "tls_without_alpn_h2")
                .text("peer", &peer)
                .emit();
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "tls: the client did not negotiate ALPN h2 (spec §4.1, RFC 9113 §3.2)",
            ));
        }
    }
    if let Some(tenant) = &part.tenant {
        event(Level::Info, "tenant_bound")
            .text("peer", &peer)
            .text("tenant", tenant)
            .text(
                "subject",
                subject
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or_default(),
            )
            .emit();
    }
    handle_connection(tls, ctl, env, part, subject, true)
}

/// Whether the first octets of a connection are an HTTP/1.1 request line
/// (an upper-case method), not the HTTP/2 preface `PRI `.
pub(crate) fn is_http1_start(start: &[u8]) -> bool {
    start.len() >= 3 && start != b"PRI " && start[..3].iter().all(u8::is_ascii_uppercase)
}

/// What a connection thread shares with the daemon.
pub(crate) struct ConnEnv<'a> {
    pub(crate) cfg: &'a DaemonConfig,
    pub(crate) metrics: &'a DaemonMetrics,
    pub(crate) participants: &'a [Participant],
    pub(crate) live: &'a zerodds_bridge_security::LiveSecurity,
    pub(crate) shutdown: &'a AtomicBool,
    /// Spec §8.3: where the spans of the calls go.
    pub(crate) tracer: &'a Tracer,
}

/// The client's address of a connection, `ip:port` (spec §8.1 `peer`).
pub(crate) fn peer_of(ctl: &TcpStream) -> String {
    ctl.peer_addr().map(|a| a.to_string()).unwrap_or_default()
}

/// How often a connection looks at the shutdown flag while it waits for
/// the client.
pub(crate) const CONN_POLL: Duration = Duration::from_millis(200);
/// A connection without a frame for this long is closed (with GOAWAY).
pub(crate) const CONN_IDLE: Duration = Duration::from_secs(5);

pub(crate) fn handle_connection<S: Read + Write>(
    mut stream: S,
    ctl: &TcpStream,
    env: &ConnEnv<'_>,
    part: &Participant,
    mtls_subject: Option<AuthSubject>,
    tls: bool,
) -> std::io::Result<()> {
    let (cfg, metrics) = (env.cfg, env.metrics);
    let peer = peer_of(ctl);
    // Read client preface (24 bytes). Spec §5.2: on the cleartext listener
    // a request that begins as HTTP/1.1 is answered as one (the IDL
    // endpoint).
    let mut preface_buf = [0u8; 24];
    stream.read_exact(&mut preface_buf[..4])?;
    if !tls && is_http1_start(&preface_buf[..4]) {
        return serve_http1(stream, &preface_buf[..4], env, part, None, &peer);
    }
    stream.read_exact(&mut preface_buf[4..])?;
    if check_preface(&preface_buf).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad preface",
        ));
    }

    // §3/§4.1: our SETTINGS with the limits of `grpc.*`, and the
    // WINDOW_UPDATE that opens the connection window to
    // `initial_window_size`.
    let mut conn = Conn {
        grpc: GrpcServer::with_limits(cfg.limits),
        calls: HashMap::new(),
        env,
        part,
        mtls_subject,
        tls,
        peer,
        opening: Clock::now(),
    };
    stream.write_all(&conn.grpc.handshake())?;

    let mut read_buf = vec![0u8; 65536];
    let mut acc: Vec<u8> = Vec::new();

    // §9.2: after the preface the connection polls, so that SIGTERM
    // reaches it. The idle close keeps the former 5 s read timeout and
    // says so with GOAWAY.
    ctl.set_read_timeout(Some(CONN_POLL))?;
    let mut idle = Duration::ZERO;
    // §3 `grpc.keepalive`: when a frame last arrived, and the PING in
    // flight with the time it went out.
    let keepalive_time = Duration::from_secs(cfg.keepalive_time_secs);
    let keepalive_timeout = Duration::from_secs(cfg.keepalive_timeout_secs);
    let mut last_read = Instant::now();
    let mut ping_out: Option<([u8; 8], Instant)> = None;
    let mut pings = 0u64;
    // §9.2: when the drain of this connection ends.
    let mut drain_until: Option<Instant> = None;
    let result = loop {
        if env.shutdown.load(Ordering::SeqCst) {
            // §9.2: GOAWAY with the last stream the client opened at once;
            // the calls up to it run on and end as they do, a stream the
            // client opens after it is refused (RFC 9113 §6.8). The
            // connection ends when its last call ended and every answer
            // went out within the client's windows — a call that ended has
            // left `calls`, its stream stays while its queue waits for a
            // WINDOW_UPDATE, as does the stream of a GET `/idl/` — or, after
            // `grpc.drain_secs`, with the calls still open ending
            // UNAVAILABLE (§4.5) and what is still queued left unsent.
            let until = *drain_until.get_or_insert_with(|| {
                conn.grpc.begin_drain();
                event(Level::Info, "goaway")
                    .text("peer", &conn.peer)
                    .num("last_stream_id", u64::from(conn.grpc.last_stream_id()))
                    .num("open_calls", conn.calls.len() as u64)
                    .num("drain_secs", cfg.drain_secs)
                    .emit();
                Instant::now() + Duration::from_secs(cfg.drain_secs)
            });
            let over = Instant::now() >= until;
            let open = !conn.calls.is_empty() || conn.grpc.active_streams() > 0;
            if over && open {
                event(Level::Warn, "drain_limit")
                    .text("peer", &conn.peer)
                    .num("open_calls", conn.calls.len() as u64)
                    .num("open_streams", conn.grpc.active_streams() as u64)
                    .num("drain_secs", cfg.drain_secs)
                    .emit();
                conn.drain();
            }
            if over || !open {
                break conn.flush(&mut stream);
            }
        }
        // §3 `grpc.keepalive`, §6 Liveliness: a PING after `time_secs`
        // without a frame — while a call is open, or always with
        // `permit_without_calls` — and GOAWAY when its ACK does not come
        // within `timeout_secs`.
        match ping_out {
            Some((opaque, _)) if conn.grpc.ping_acked(opaque) => ping_out = None,
            Some((_, sent)) if sent.elapsed() >= keepalive_timeout => {
                event(Level::Warn, "keepalive_timeout")
                    .text("peer", &conn.peer)
                    .num("timeout_secs", cfg.keepalive_timeout_secs)
                    .emit();
                conn.grpc
                    .goaway(ErrorCode::NoError, b"keepalive ping not acknowledged");
                break conn.flush(&mut stream);
            }
            Some(_) => {}
            None => {
                let calls_open = !conn.calls.is_empty() || conn.grpc.active_streams() > 0;
                if (calls_open || cfg.limits.permit_pings_without_calls)
                    && last_read.elapsed() >= keepalive_time
                {
                    pings += 1;
                    let opaque = pings.to_be_bytes();
                    conn.grpc.keepalive_ping(opaque);
                    ping_out = Some((opaque, Instant::now()));
                }
            }
        }
        // §4.5 `grpc-timeout`, then the samples of the open Subscribe
        // streams, as far as the client's windows take them.
        conn.check_deadlines();
        conn.pump();
        if let Err(e) = conn.flush(&mut stream) {
            break Err(e);
        }
        // While a call is open its samples and its deadline are looked at
        // every STREAM_POLL, and the connection is not idle.
        ctl.set_read_timeout(Some(if conn.calls.is_empty() {
            CONN_POLL
        } else {
            STREAM_POLL
        }))?;
        let n = match stream.read(&mut read_buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                idle = Duration::ZERO;
                last_read = Instant::now();
                n
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                // An open call keeps the connection; so does keepalive
                // with `permit_without_calls`, which ends it itself when
                // the client stops answering.
                if !conn.calls.is_empty()
                    || conn.grpc.active_streams() > 0
                    || cfg.limits.permit_pings_without_calls
                {
                    continue;
                }
                idle += CONN_POLL;
                if idle >= CONN_IDLE {
                    conn.grpc.goaway(ErrorCode::NoError, b"idle");
                    break conn.flush(&mut stream);
                }
                continue;
            }
            Err(e) => break Err(e),
        };
        acc.extend_from_slice(&read_buf[..n]);
        metrics.base.bytes_in_total.add(n as u64);

        // Every whole frame read so far.
        let mut ended = None;
        loop {
            // §8.2 `h2_resets_total`: a RST_STREAM from the client
            // (RFC 9113 §6.4, a 4-octet error code).
            if acc.len() >= 13 && acc[3] == FrameType::RstStream as u8 && acc[..3] == [0, 0, 4] {
                metrics
                    .spec
                    .h2_reset(u32::from_be_bytes([acc[9], acc[10], acc[11], acc[12]]));
            }
            match conn.grpc.process_frame(&acc) {
                Ok(0) => break,
                Ok(consumed) => {
                    acc.drain(..consumed);
                    for event in conn.grpc.take_events() {
                        conn.on_event(event);
                    }
                    // RFC 9113 §6.5.3 / §6.7 — the SETTINGS and PING
                    // acknowledgements the frame made due (without them a
                    // Go gRPC client closes the connection before it sends
                    // a request), WINDOW_UPDATEs, resets and the answers.
                    if let Err(e) = conn.flush(&mut stream) {
                        ended = Some(Err(e));
                        break;
                    }
                    if conn.grpc.is_closing() {
                        // §3 `keepalive.permit_without_calls: false`: too
                        // many PINGs without a call.
                        event(Level::Info, "goaway")
                            .text("peer", &conn.peer)
                            .text("reason", "too_many_pings")
                            .num("last_stream_id", u64::from(conn.grpc.last_stream_id()))
                            .emit();
                        ended = Some(Ok(()));
                        break;
                    }
                }
                Err(e) => {
                    // RFC 9113 §5.4.1: GOAWAY with the error code is
                    // queued; it goes out, then the connection closes.
                    metrics.base.errors_total.inc();
                    event(Level::Error, "connection_error")
                        .text("peer", &conn.peer)
                        .text("code", &format!("{:?}", e.code))
                        .text("msg", e.reason)
                        .emit();
                    let _ = conn.flush(&mut stream);
                    ended = Some(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )));
                    break;
                }
            }
        }
        if let Some(r) = ended {
            break r;
        }
    };
    conn.close_all();
    result
}
