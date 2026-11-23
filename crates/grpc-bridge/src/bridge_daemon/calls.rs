// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The calls of one connection (spec §4.3, §4.5, §6): authentication per call,
//! `Publish` into DDS, the `Subscribe` streams out of it, deadlines, the end of
//! a call with its status.

use std::collections::VecDeque;

use super::*;

/// The poll interval of a connection with an open call: how long a DDS
/// sample may wait before it goes out as a stream message, and the
/// resolution of a call's deadline.
pub(crate) const STREAM_POLL: Duration = Duration::from_millis(20);

/// The octets a `Subscribe` stream may have waiting for the client's
/// flow-control window (RFC 9113 §6.9) before the daemon takes no more
/// samples from the stream's DataReader: what the client does not read
/// stays in DDS.
pub(crate) const STREAM_QUEUE_LIMIT: usize = 256 * 1024;

/// Spec §6 BEST_EFFORT: the `samples_dropped` lines of a stream come at
/// most this often.
pub(crate) const DROP_LOG_INTERVAL: Duration = Duration::from_secs(1);

/// The `samples_dropped` line (spec §6 Reliability): `count` samples of
/// the stream `id` of the topic `label` dropped.
pub(crate) fn log_dropped(id: u32, label: &str, count: u64) {
    event(Level::Warn, "samples_dropped")
        .num("stream_id", u64::from(id))
        .text("dds_topic", label)
        .num("count", count)
        .emit();
}

/// Where a call goes (spec §4.2: the four methods of a topic service, and
/// the reflection service of §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Reflection,
    /// The index of the topic in the participant's topics.
    Catalog(usize),
    Publish(usize),
    PublishOne(usize),
    Subscribe(usize),
}

/// A call being served on a connection.
pub(crate) struct Call {
    pub(crate) service: String,
    pub(crate) method: String,
    pub(crate) route: Route,
    /// §4.3 `grpc-timeout`: when the call ends with DEADLINE_EXCEEDED.
    pub(crate) deadline: Option<Instant>,
    /// §4.4 `grpc-encoding` as the client named it, and the encoding it
    /// names (`None` for one the daemon does not speak).
    pub(crate) encoding_name: Option<String>,
    pub(crate) encoding: Option<Encoding>,
    /// `:authority` of the request (the host of `idl_url`, §5.2).
    pub(crate) authority: Option<String>,
    /// Request messages received.
    pub(crate) messages: u64,
    /// Publish: the sequence number of the last sample written.
    pub(crate) last_sequence: i64,
    /// Subscribe: the stream's own DataReader and
    /// `SubscribeReq.include_dispose`.
    pub(crate) reader: Option<StreamReader>,
    pub(crate) include_dispose: bool,
    pub(crate) gauge: Option<Arc<zerodds_monitor::Gauge>>,
    /// Spec §6 Lifespan: the lifespan of the samples of a Publish.
    pub(crate) lifespan_ms: Option<u64>,
    /// Spec §6 `initial_burst`: the way of the stream from the retained
    /// samples to its live reader.
    pub(crate) join: Option<BurstJoin>,
    /// Spec §6 BEST_EFFORT: the samples dropped since the last
    /// `samples_dropped` line, and when that line was written.
    pub(crate) dropped_unlogged: u64,
    pub(crate) dropped_logged_at: Option<Instant>,
    /// Spec §6 Deadline: the misses of the stream's reader already logged.
    pub(crate) deadlines_logged: i32,
    /// Spec §8.1 `latency_us`, §8.3: when the call's HEADERS arrived.
    pub(crate) opened: Clock,
    /// Spec §8.1 `bytes`: the octets of the call's request and response
    /// messages, each with its 5-octet prefix.
    pub(crate) bytes: u64,
    /// Spec §8.3: the caller's span, from the `traceparent` of the call.
    pub(crate) parent: Option<zerodds_monitor::TraceParent>,
    /// Spec §8.3: the DDS side of the call, the span `dds.publish` or
    /// `dds.subscribe`.
    pub(crate) dds_leg: Option<DdsLeg>,
    /// Spec §4.6: the requests of a reflection stream that wait, as they
    /// came (Compressed-Flag, octets), while its answers hold
    /// [`STREAM_QUEUE_LIMIT`] for the client's window; and whether the
    /// client ended its side behind them.
    pub(crate) held: VecDeque<(bool, Vec<u8>)>,
    pub(crate) half_closed: bool,
}

/// Spec §8.1, §8.3: what the `rpc` line and the spans of a call that ended
/// say about it.
pub(crate) struct Ended<'a> {
    pub(crate) id: u32,
    pub(crate) service: &'a str,
    pub(crate) method: &'a str,
    pub(crate) route: Option<Route>,
    pub(crate) opened: Clock,
    pub(crate) bytes: u64,
    pub(crate) parent: Option<zerodds_monitor::TraceParent>,
    pub(crate) dds_leg: Option<DdsLeg>,
}

impl Call {
    /// Spec §8.3: a DDS write of the call from `start_ns` to `end_ns`.
    pub(crate) fn dds_written(&mut self, start_ns: u64, end_ns: u64) {
        let leg = self.dds_leg.get_or_insert(DdsLeg {
            name: "dds.publish",
            start_ns,
            end_ns,
            samples: 0,
        });
        leg.end_ns = end_ns;
        leg.samples += 1;
    }

    /// The fields of the call for its `rpc` line and its spans.
    pub(crate) fn ended(&self, id: u32) -> Ended<'_> {
        Ended {
            id,
            service: &self.service,
            method: &self.method,
            route: Some(self.route),
            opened: self.opened,
            bytes: self.bytes,
            parent: self.parent.clone(),
            dds_leg: self.dds_leg,
        }
    }

    /// The encoding of the answer: the client's, when it is one the daemon
    /// compresses with.
    pub(crate) fn response_encoding(&self) -> Encoding {
        self.encoding.unwrap_or(Encoding::Identity)
    }
}

/// One connection's calls.
pub(crate) struct Conn<'a> {
    pub(crate) grpc: GrpcServer,
    pub(crate) calls: HashMap<u32, Call>,
    pub(crate) env: &'a ConnEnv<'a>,
    pub(crate) part: &'a Participant,
    pub(crate) mtls_subject: Option<AuthSubject>,
    /// The connection runs over TLS (the scheme of `idl_url`, §5.2).
    pub(crate) tls: bool,
    /// The client's address (spec §8.1 `peer`).
    pub(crate) peer: String,
    /// When the HEADERS of the call being opened arrived.
    pub(crate) opening: Clock,
}

impl Conn<'_> {
    /// Spec §8.1, §8.2, §8.3: a call ended with `status` — counted in
    /// `zerodds_grpc_bridge_rpcs_total`, one `rpc` line, its spans.
    pub(crate) fn ended(&self, e: &Ended<'_>, status: Status, message: Option<&str>) {
        self.env
            .metrics
            .spec
            .count(self.part, e.service, e.method, status);
        let topic = match e.route {
            Some(
                Route::Catalog(t) | Route::Publish(t) | Route::PublishOne(t) | Route::Subscribe(t),
            ) => self.part.label(&self.part.topics[t].dds_name),
            _ => String::new(),
        };
        log_call(&CallLine {
            peer: &self.peer,
            service: e.service,
            method: e.method,
            dds_topic: &topic,
            bytes: e.bytes,
            status: Some(status),
            message,
            latency_us: e.opened.elapsed_us(),
            stream_id: e.id,
        });
        if self.env.tracer.enabled() {
            let end_ns = e.opened.unix_ns_now();
            let dds = e.dds_leg.map(|leg| match leg.name {
                // A stream's DDS side lasts as long as the stream.
                "dds.subscribe" => DdsLeg { end_ns, ..leg },
                _ => leg,
            });
            self.env.tracer.call(&CallSpan {
                service: e.service,
                method: e.method,
                dds_topic: &topic,
                peer: &self.peer,
                status,
                message,
                start_ns: e.opened.unix_ns,
                end_ns,
                parent: e.parent.as_ref(),
                dds,
            });
        }
    }

    /// The fields of a call that ends at its HEADERS.
    pub(crate) fn ended_at_open<'c>(&self, call: &'c GrpcCall) -> Ended<'c> {
        Ended {
            id: call.stream_id,
            service: &call.service,
            method: &call.method,
            route: None,
            opened: self.opening,
            bytes: 0,
            parent: traceparent_of(&call.metadata),
            dds_leg: None,
        }
    }

    pub(crate) fn on_event(&mut self, event: CallEvent) {
        match event {
            CallEvent::Open(call) => self.on_open(call),
            CallEvent::Message {
                stream_id,
                compressed,
                message,
            } => self.on_message(stream_id, compressed, message),
            CallEvent::HalfClose { stream_id } => self.on_half_close(stream_id),
            CallEvent::Reset { stream_id, code } => {
                if let Some(call) = self.calls.remove(&stream_id) {
                    let m = format!("the stream was reset (RST_STREAM {code})");
                    self.ended(&call.ended(stream_id), Status::Cancelled, Some(&m));
                    self.release(stream_id, call);
                }
            }
            CallEvent::Refused(r) => {
                if self.calls.contains_key(&r.stream_id) {
                    self.finish(r.stream_id, r.status, Some(r.message));
                } else {
                    let (service, method) = crate::parse_path(&r.path).unwrap_or_default();
                    let ended = Ended {
                        id: r.stream_id,
                        service: &service,
                        method: &method,
                        route: None,
                        opened: Clock::now(),
                        bytes: 0,
                        parent: None,
                        dds_leg: None,
                    };
                    self.ended(&ended, r.status, Some(&r.message));
                    self.grpc
                        .close_call(r.stream_id, r.status, Some(&r.message));
                }
            }
        }
    }

    /// Answers a call that is refused at its HEADERS.
    pub(crate) fn refuse_open(&mut self, call: &GrpcCall, status: Status, message: String) {
        self.ended(&self.ended_at_open(call), status, Some(&message));
        self.grpc.close_call(call.stream_id, status, Some(&message));
    }

    /// The request HEADERS of a call: what makes it a gRPC call the daemon
    /// serves (spec §4.3), its deadline, its authentication (§7.2) and its
    /// route (§4.2, §7.3).
    pub(crate) fn on_open(&mut self, call: GrpcCall) {
        let id = call.stream_id;
        self.opening = Clock::now();
        self.env.metrics.base.frames_in_total.inc();
        // Spec §5.2: the IDL of a topic, outside gRPC.
        if call.http_method == "GET" && call.path.starts_with("/idl/") {
            let ctx = self.env.live.current();
            let (status, ctype, body) = idl_answer(
                self.part,
                &call.path,
                authenticate_grpc(&ctx.auth, &call.metadata, self.mtls_subject.clone()),
                self.env.metrics,
            );
            self.grpc.respond_http(id, status, ctype, body.as_bytes());
            log_http(
                &self.peer,
                "h2",
                &call.http_method,
                &call.path,
                status,
                body.len(),
                self.opening,
            );
            return;
        }
        // gRPC over HTTP/2, "Requests": only POST; a content-type that is
        // not application/grpc gets HTTP 415, as grpc-go answers it.
        if call.http_method != "POST" {
            let m = format!(":method {} — a gRPC call is a POST", call.http_method);
            self.ended(&self.ended_at_open(&call), Status::Unimplemented, Some(&m));
            self.grpc.reject(id, 405, Status::Unimplemented, &m);
            return;
        }
        if !call.content_type_is_grpc() {
            let m = format!(
                "content-type {:?} is not application/grpc",
                call.content_type.as_deref().unwrap_or("")
            );
            self.ended(
                &self.ended_at_open(&call),
                Status::InvalidArgument,
                Some(&m),
            );
            self.grpc.reject(id, 415, Status::InvalidArgument, &m);
            return;
        }
        if !call.content_type_is_grpc_proto() {
            let m = format!(
                "content-type {:?}: the daemon speaks protobuf (application/grpc+proto) only",
                call.content_type.as_deref().unwrap_or("")
            );
            return self.refuse_open(&call, Status::Unimplemented, m);
        }
        let deadline = match call.timeout.as_deref().map(decode_timeout) {
            None => None,
            Some(Ok((value, unit))) => Instant::now().checked_add(timeout_duration(value, unit)),
            Some(Err(e)) => {
                // grpc-go answers a malformed grpc-timeout with INTERNAL.
                let m = format!("malformed grpc-timeout {:?}: {e}", call.timeout);
                return self.refuse_open(&call, Status::Internal, m);
            }
        };
        // §7.2 per call, against the settings of now.
        let ctx = self.env.live.current();
        let subject = match authenticate_grpc(&ctx.auth, &call.metadata, self.mtls_subject.clone())
        {
            Ok(subject) => subject,
            Err(e) => {
                self.env.metrics.spec.auth_failure(&e);
                event(Level::Warn, "auth_failed")
                    .text("peer", &self.peer)
                    .text("service", &call.service)
                    .text("err", &e.to_string())
                    .emit();
                return self.refuse_open(&call, Status::Unauthenticated, e.to_string());
            }
        };
        let route = match route(
            &call.service,
            &call.method,
            self.part,
            self.env.cfg.reflection_enabled,
            &ctx.acl,
            &subject,
        ) {
            Ok(r) => r,
            Err((status, m)) => return self.refuse_open(&call, status, m),
        };
        // Spec §6 Lifespan: `zerodds-lifespan-ms`, else `grpc-timeout`
        // (rounded up to whole seconds), for the samples of a Publish.
        let lifespan_ms = match lifespan_of(&call) {
            Ok(v) => v,
            Err(m) => return self.refuse_open(&call, Status::InvalidArgument, m),
        };
        let gauge = self.env.metrics.spec.stream_gauge(self.part, &call.service);
        if let Some(g) = &gauge {
            g.inc();
        }
        let parent = traceparent_of(&call.metadata);
        self.calls.insert(
            id,
            Call {
                service: call.service,
                method: call.method,
                route,
                deadline,
                encoding: Encoding::of(call.encoding.as_deref()),
                encoding_name: call.encoding,
                authority: call.authority,
                messages: 0,
                last_sequence: 0,
                reader: None,
                include_dispose: false,
                gauge,
                lifespan_ms,
                join: None,
                dropped_unlogged: 0,
                dropped_logged_at: None,
                deadlines_logged: 0,
                opened: self.opening,
                bytes: 0,
                parent,
                dds_leg: None,
                held: VecDeque::new(),
                half_closed: false,
            },
        );
    }

    /// One request message (spec §4.3). A reflection request waits while
    /// the stream's answers hold [`STREAM_QUEUE_LIMIT`] for the client's
    /// window, or while requests before it wait (§4.6): the server returns
    /// no receive window of the stream meanwhile, so the client sends at
    /// most what is left of it.
    pub(crate) fn on_message(&mut self, id: u32, compressed: bool, message: Vec<u8>) {
        let queued = self.grpc.queued(id);
        let Some(c) = self.calls.get_mut(&id) else {
            return;
        };
        if c.route == Route::Reflection && (!c.held.is_empty() || queued >= STREAM_QUEUE_LIMIT) {
            c.held.push_back((compressed, message));
            self.grpc.hold_window(id, true);
            return;
        }
        self.take_message(id, compressed, message);
    }

    /// Spec §4.6: the reflection requests that wait, as far as the answers
    /// leave room below [`STREAM_QUEUE_LIMIT`]; when none waits any more,
    /// the stream's window is returned again, and a half-close behind them
    /// ends the stream.
    pub(crate) fn serve_held(&mut self) {
        let ids: Vec<u32> = self
            .calls
            .iter()
            .filter(|(_, c)| !c.held.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            while self.grpc.queued(id) < STREAM_QUEUE_LIMIT {
                let Some((compressed, message)) =
                    self.calls.get_mut(&id).and_then(|c| c.held.pop_front())
                else {
                    break;
                };
                self.take_message(id, compressed, message);
            }
            let Some(c) = self.calls.get(&id) else {
                continue;
            };
            if c.held.is_empty() {
                let half_closed = c.half_closed;
                self.grpc.hold_window(id, false);
                if half_closed {
                    self.finish(id, Status::Ok, None);
                }
            }
        }
    }

    /// One request message taken: counted, checked against the deadline,
    /// decompressed per §4.4, then served.
    fn take_message(&mut self, id: u32, compressed: bool, message: Vec<u8>) {
        let limit = self.env.cfg.limits.max_recv_message_size as usize;
        let Some(c) = self.calls.get_mut(&id) else {
            return;
        };
        c.messages += 1;
        let wire_len = message.len() + 5;
        c.bytes += wire_len as u64;
        if c.deadline.is_some_and(|d| Instant::now() >= d) {
            return self.finish(
                id,
                Status::DeadlineExceeded,
                Some("the deadline of grpc-timeout passed".into()),
            );
        }
        let message = if compressed {
            match c.encoding {
                // gRPC Compression: a message compressed with an encoding
                // the server does not speak is UNIMPLEMENTED, and the
                // answer names the encodings it accepts.
                None => {
                    let m = format!(
                        "grpc-encoding {:?} is not supported (grpc-accept-encoding: {})",
                        c.encoding_name.as_deref().unwrap_or(""),
                        compression::ACCEPT_ENCODING
                    );
                    return self.finish(id, Status::Unimplemented, Some(m));
                }
                Some(Encoding::Identity) => {
                    return self.finish(
                        id,
                        Status::Internal,
                        Some("the Compressed-Flag is set and grpc-encoding is identity".into()),
                    );
                }
                Some(enc) => match compression::decompress(enc, &message, limit) {
                    Ok(m) => m,
                    Err(e @ compression::CompressionError::TooLarge { .. }) => {
                        return self.finish(
                            id,
                            Status::ResourceExhausted,
                            Some(format!("{e} (max_recv_message_size)")),
                        );
                    }
                    Err(e) => return self.finish(id, Status::Internal, Some(e.to_string())),
                },
            }
        } else {
            message
        };
        let (route, count) = (c.route, c.messages);
        self.on_request(id, route, count, &message, wire_len);
    }

    /// A request message of `route`; `count` is its number on the call.
    pub(crate) fn on_request(
        &mut self,
        id: u32,
        route: Route,
        count: u64,
        message: &[u8],
        wire_len: usize,
    ) {
        // grpc-java: a call of a unary request takes one message, a second
        // is INTERNAL ("too many requests").
        if count > 1 && !matches!(route, Route::Publish(_) | Route::Reflection) {
            return self.finish(
                id,
                Status::Internal,
                Some("too many requests: the method takes one request message".into()),
            );
        }
        match route {
            Route::Publish(i) => {
                if let Err((status, m)) = self.publish(id, i, message, wire_len) {
                    self.finish(id, status, Some(m));
                }
            }
            Route::PublishOne(i) => match self.publish(id, i, message, wire_len) {
                Ok(()) => self.acknowledge(id),
                Err((status, m)) => self.finish(id, status, Some(m)),
            },
            Route::Subscribe(i) => match SubscribeReq::decode(message) {
                Ok(req) => self.start_stream(id, i, &req),
                Err(e) => self.finish(
                    id,
                    Status::InvalidArgument,
                    Some(format!("SubscribeReq: {e}")),
                ),
            },
            Route::Catalog(_) => match decode_catalog_req(message) {
                Ok(()) => {
                    let base = idl_base(
                        self.tls,
                        self.calls
                            .get(&id)
                            .and_then(|c| c.authority.as_deref())
                            .unwrap_or(&self.env.cfg.bind),
                    );
                    let resp = catalog_resp(self.part, &base).encode();
                    if self.reply(id, &resp) {
                        self.finish(id, Status::Ok, None);
                    }
                }
                Err(e) => self.finish(
                    id,
                    Status::InvalidArgument,
                    Some(format!("CatalogReq: {e}")),
                ),
            },
            // §4.6: one response per request, for the life of the stream.
            Route::Reflection => {
                let answer = reflection::decode_request(message)
                    .map_err(|e| format!("ServerReflectionRequest: {e}"))
                    .and_then(|req| {
                        Reflection::of(&reflection_catalog(self.part))
                            .respond(&req)
                            .ok_or_else(|| "ServerReflectionRequest names no request".to_string())
                    });
                match answer {
                    Ok(resp) => {
                        self.reply(id, &resp);
                    }
                    Err(m) => self.finish(id, Status::InvalidArgument, Some(m)),
                }
            }
        }
    }

    /// The client ended its side: a `Publish` stream is acknowledged; a
    /// call of a unary request without a message takes the empty message
    /// (all fields default).
    pub(crate) fn on_half_close(&mut self, id: u32) {
        let Some(c) = self.calls.get(&id) else {
            return;
        };
        let (route, messages) = (c.route, c.messages);
        match route {
            Route::Publish(_) => self.acknowledge(id),
            // §4.6: the client ended the reflection stream; its waiting
            // requests are answered first.
            Route::Reflection => {
                if let Some(c) = self.calls.get_mut(&id)
                    && !c.held.is_empty()
                {
                    c.half_closed = true;
                } else {
                    self.finish(id, Status::Ok, None);
                }
            }
            _ if messages == 0 => {
                if let Some(c) = self.calls.get_mut(&id) {
                    c.messages = 1;
                }
                self.on_request(id, route, 1, &[], 5);
            }
            _ => {}
        }
    }

    /// Writes one `Sample` of a `Publish` or `PublishOne` to DDS.
    pub(crate) fn publish(
        &mut self,
        id: u32,
        topic: usize,
        message: &[u8],
        wire_len: usize,
    ) -> Result<(), (Status, String)> {
        let (part, metrics) = (self.part, self.env.metrics);
        let t = &part.topics[topic];
        let label = part.label(&t.dds_name);
        metrics.spec.received(&label, wire_len);
        let sample = Sample::decode(message)
            .map_err(|e| (Status::InvalidArgument, format!("Sample: {e}")))?;
        let mut outgoing =
            sample_to_publish(t, &sample).map_err(|m| (Status::InvalidArgument, m))?;
        outgoing.lifespan_ms = self.calls.get(&id).and_then(|c| c.lifespan_ms);
        let dds = part
            .dds()
            .ok_or((Status::Unavailable, "no DDS participant".to_string()))?;
        let write_start = unix_ns();
        match dds.publish(&t.dds_name, outgoing) {
            None => Err((
                Status::Internal,
                format!("{} has no DataWriter", t.dds_name),
            )),
            Some(Err(e)) => Err((
                status_of_write_error(&e),
                format!("DDS write to {}: {e}", t.dds_name),
            )),
            Some(Ok(sequence)) => {
                metrics.base.dds_samples_in_total.inc();
                metrics.spec.published(&label);
                if let Some(c) = self.calls.get_mut(&id) {
                    c.last_sequence = sequence;
                    c.dds_written(write_start, unix_ns());
                }
                Ok(())
            }
        }
    }

    /// `PublishAck` with the sequence number of the last sample written and
    /// the time, then OK (spec §4.2, §4.3).
    pub(crate) fn acknowledge(&mut self, id: u32) {
        let Some(c) = self.calls.get(&id) else {
            return;
        };
        let ack = PublishAck {
            sequence_number: u64::try_from(c.last_sequence).unwrap_or(0),
            acked_at_ns: now_ns(),
        };
        if self.reply(id, &ack.encode()) {
            self.finish(id, Status::Ok, None);
        }
    }

    /// Opens the server stream of a `Subscribe` call with a DataReader of
    /// its own (spec §4.3): in the partitions of
    /// `SubscribeReq.partition_filter` (comma-separated, wildcards as DDS
    /// partition names take them), with dispose and unregister samples when
    /// `include_dispose`.
    pub(crate) fn start_stream(&mut self, id: u32, topic: usize, req: &SubscribeReq) {
        let part = self.part;
        let t = &part.topics[topic];
        // Spec §6: the retained samples are those of the default
        // partition.
        if req.initial_burst > 0 && !req.partition_filter.trim().is_empty() {
            return self.finish(
                id,
                Status::InvalidArgument,
                Some(
                    "SubscribeReq.initial_burst with a partition_filter: the daemon retains the samples of the default partition (spec §6)"
                        .into(),
                ),
            );
        }
        let partitions: Vec<String> = req
            .partition_filter
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        let Some(dds) = part.dds() else {
            return self.finish(id, Status::Unavailable, Some("no DDS participant".into()));
        };
        let burst = req.initial_burst > 0 && dds.retains(&t.dds_name);
        match dds.open_reader(&t.dds_name, partitions, burst) {
            Ok(reader) => {
                event(Level::Debug, "stream_open")
                    .text("peer", &self.peer)
                    .num("stream_id", u64::from(id))
                    .text("dds_topic", &part.label(&t.dds_name))
                    .num("initial_burst", u64::from(req.initial_burst))
                    .emit();
                self.open(id);
                if let Some(c) = self.calls.get_mut(&id) {
                    c.reader = Some(reader);
                    c.include_dispose = req.include_dispose;
                    let now = unix_ns();
                    c.dds_leg = Some(DdsLeg {
                        name: "dds.subscribe",
                        start_ns: now,
                        end_ns: now,
                        samples: 0,
                    });
                }
                // Spec §6 TRANSIENT_LOCAL: the last `initial_burst` retained
                // samples first, read from where they start once the live
                // reader exists ([`BurstJoin`]); [`Self::pump`] sends them
                // within the stream's queue limit. A topic the daemon
                // retains nothing of has no burst.
                if burst
                    && let Some(cursor) = dds.burst_start(&t.dds_name, req.initial_burst as usize)
                    && let Some(c) = self.calls.get_mut(&id)
                {
                    c.join = Some(BurstJoin {
                        cursor,
                        ..BurstJoin::default()
                    });
                }
            }
            Err(e) => self.finish(id, Status::Internal, Some(e)),
        }
    }

    /// The response HEADERS of call `id`, with `grpc-encoding` when the
    /// answer is compressed and `grpc-accept-encoding` (§4.4).
    pub(crate) fn open(&mut self, id: u32) {
        let encoding = self
            .calls
            .get(&id)
            .map_or(Encoding::Identity, Call::response_encoding);
        if encoding == Encoding::Identity {
            self.grpc.open_response(
                id,
                &[("grpc-accept-encoding", compression::ACCEPT_ENCODING)],
            );
        } else {
            self.grpc.open_response(
                id,
                &[
                    ("grpc-encoding", encoding.name()),
                    ("grpc-accept-encoding", compression::ACCEPT_ENCODING),
                ],
            );
        }
    }

    /// One response message of call `id`, compressed with the call's
    /// encoding. `false` when the call ended instead (RESOURCE_EXHAUSTED
    /// above `max_send_message_size`, INTERNAL when the encoder failed).
    pub(crate) fn reply(&mut self, id: u32, message: &[u8]) -> bool {
        let encoding = self
            .calls
            .get(&id)
            .map_or(Encoding::Identity, Call::response_encoding);
        let wire = match compression::compress(encoding, message) {
            Ok(w) => w,
            Err(e) => {
                self.finish(id, Status::Internal, Some(format!("compress: {e}")));
                return false;
            }
        };
        self.open(id);
        match self
            .grpc
            .send_message(id, &wire, encoding != Encoding::Identity)
        {
            Ok(()) => {
                self.env.metrics.base.frames_out_total.inc();
                if let Some(c) = self.calls.get_mut(&id) {
                    c.bytes += wire.len() as u64 + 5;
                }
                true
            }
            Err(SendError::TooLarge) => {
                let m = format!(
                    "response message of {} octets exceeds max_send_message_size {}",
                    wire.len(),
                    self.env.cfg.limits.max_send_message_size
                );
                self.finish(id, Status::ResourceExhausted, Some(m));
                false
            }
            Err(SendError::Closed) => false,
        }
    }

    /// Ends call `id` with `status` in its trailers and releases it.
    pub(crate) fn finish(&mut self, id: u32, status: Status, message: Option<String>) {
        if !self.calls.contains_key(&id) {
            return;
        }
        self.open(id);
        let Some(call) = self.calls.remove(&id) else {
            return;
        };
        // Spec §6 Deadline: a stream of a topic with `qos.deadline_ms` ends
        // with the misses of its reader in `zerodds-deadline-missed`.
        let missed = match (call.route, &call.reader, self.part.dds()) {
            (Route::Subscribe(t), Some(reader), Some(dds))
                if self.part.topics[t].deadline_ms.is_some() =>
            {
                Some(dds.deadlines_missed(reader).to_string())
            }
            _ => None,
        };
        match &missed {
            Some(count) => self.grpc.close_call_with(
                id,
                status,
                message.as_deref(),
                &[("zerodds-deadline-missed", count)],
            ),
            None => self.grpc.close_call(id, status, message.as_deref()),
        }
        self.ended(&call.ended(id), status, message.as_deref());
        self.release(id, call);
    }

    /// A call `id` that ended: its gauge counts it out, its reader is
    /// deleted, the samples it dropped since the last `samples_dropped`
    /// line are logged (spec §6).
    pub(crate) fn release(&self, id: u32, call: Call) {
        if let Some(g) = &call.gauge {
            g.dec();
        }
        if call.dropped_unlogged > 0
            && let Route::Subscribe(t) = call.route
        {
            log_dropped(
                id,
                &self.part.label(&self.part.topics[t].dds_name),
                call.dropped_unlogged,
            );
        }
        if let (Some(reader), Some(dds)) = (call.reader, self.part.dds()) {
            dds.close_reader(reader);
        }
    }

    /// §4.3 `grpc-timeout`: a call past its deadline ends with
    /// DEADLINE_EXCEEDED (§4.5).
    pub(crate) fn check_deadlines(&mut self) {
        let now = Instant::now();
        let due: Vec<u32> = self
            .calls
            .iter()
            .filter(|(_, c)| c.deadline.is_some_and(|d| now >= d))
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            self.finish(
                id,
                Status::DeadlineExceeded,
                Some("the deadline of grpc-timeout passed".into()),
            );
        }
    }

    /// Sends what the DataReaders of the open Subscribe streams hold as
    /// stream messages (§4.2 `Sample`, §4.4), while the stream has less
    /// than [`STREAM_QUEUE_LIMIT`] waiting for the client's window; a
    /// stream with an `initial_burst` first the retained samples
    /// ([`BurstJoin`]). A message above `max_send_message_size` ends its
    /// stream with RESOURCE_EXHAUSTED (§3, §4.5); a stream whose DDS side
    /// lost its liveliness ends with UNAVAILABLE once its samples went out
    /// (§4.5).
    pub(crate) fn pump(&mut self) {
        self.serve_held();
        let part = self.part;
        let Some(dds) = part.dds() else {
            return;
        };
        let ids: Vec<u32> = self
            .calls
            .iter()
            .filter(|(_, c)| c.reader.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let mut drained = false;
            // The retained DataReaders are read at the start of a stream's
            // round, and again before a live sample of a writer the live
            // reader does not follow alone yet goes out.
            let mut first = true;
            while self.grpc.queued(id) < STREAM_QUEUE_LIMIT {
                let Some(c) = self.calls.get_mut(&id) else {
                    break;
                };
                let (Route::Subscribe(topic), Some(reader)) = (c.route, c.reader.as_ref()) else {
                    break;
                };
                let t = &part.topics[topic];
                let refresh = std::mem::take(&mut first);
                // Spec §6: the retained samples from the cursor first.
                if let Some(join) = c.join.as_mut()
                    && let Some((at, d)) = dds.retained_after(&t.dds_name, join.cursor, refresh)
                {
                    join.cursor = at;
                    if !join.take_retained(&d) {
                        continue;
                    }
                    let message = sample_of(t, &d).encode();
                    let label = part.label(&t.dds_name);
                    if !self.reply(id, &message) {
                        break;
                    }
                    self.streamed(id, &label, message.len());
                    continue;
                }
                let held = c.join.as_mut().and_then(|j| j.held.take());
                let Some(d) = held.or_else(|| BridgeDds::next_sample(reader, c.include_dispose))
                else {
                    drained = true;
                    break;
                };
                if let Some(join) = c.join.as_mut() {
                    match join.live_seen(&d) {
                        Some(true) => continue,
                        Some(false) => {}
                        None => {
                            // Retained samples of the writer may still be on
                            // their way into the store: they go before it.
                            if dds.retained_after(&t.dds_name, join.cursor, true).is_some() {
                                join.held = Some(d);
                                continue;
                            }
                            join.live_sent(&d);
                        }
                    }
                }
                let message = sample_of(t, &d).encode();
                let label = part.label(&t.dds_name);
                if !self.reply(id, &message) {
                    break;
                }
                self.streamed(id, &label, message.len());
            }
            // Spec §6 BEST_EFFORT: while the stream's queue for the
            // client's window is full, the samples the reader takes are
            // dropped and counted.
            if !drained {
                self.drop_best_effort(id);
            }
            self.log_deadlines(id);
            let lost = drained
                && self
                    .calls
                    .get(&id)
                    .and_then(|c| c.reader.as_ref())
                    .is_some_and(|r| dds.liveliness_lost(r));
            if lost {
                let topic = self
                    .calls
                    .get(&id)
                    .and_then(|c| match c.route {
                        Route::Subscribe(t) => Some(part.topics[t].dds_name.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                self.finish(
                    id,
                    Status::Unavailable,
                    Some(format!(
                        "DDS liveliness lost: no matched DataWriter of {topic} is alive"
                    )),
                );
            }
        }
    }

    /// A sample went out on the stream `id` (spec §8.2 `messages_out`,
    /// `bytes_out`; §8.3 the samples of `dds.subscribe`).
    pub(crate) fn streamed(&mut self, id: u32, label: &str, len: usize) {
        self.env.metrics.spec.streamed(label, len);
        if let Some(leg) = self.calls.get_mut(&id).and_then(|c| c.dds_leg.as_mut()) {
            leg.samples += 1;
        }
    }

    /// Spec §6 BEST_EFFORT: a stream of a best-effort topic whose queue
    /// holds [`STREAM_QUEUE_LIMIT`] for the client's window drops what its
    /// reader holds, counted in `zerodds_grpc_bridge_samples_dropped_total`
    /// and logged as `samples_dropped` at `warn` — at the first drop, then
    /// at most once per [`DROP_LOG_INTERVAL`] with the count since the last
    /// line, and at the end of the stream with the rest.
    pub(crate) fn drop_best_effort(&mut self, id: u32) {
        let part = self.part;
        let Some(c) = self.calls.get_mut(&id) else {
            return;
        };
        let (Route::Subscribe(topic), Some(reader)) = (c.route, c.reader.as_ref()) else {
            return;
        };
        let t = &part.topics[topic];
        if t.reliable || self.grpc.queued(id) < STREAM_QUEUE_LIMIT {
            return;
        }
        let mut dropped = 0u64;
        while BridgeDds::next_sample(reader, c.include_dispose).is_some() {
            dropped += 1;
        }
        if dropped == 0 {
            return;
        }
        let label = part.label(&t.dds_name);
        self.env.metrics.spec.dropped(&label, dropped);
        c.dropped_unlogged += dropped;
        if c.dropped_logged_at
            .is_none_or(|at| at.elapsed() >= DROP_LOG_INTERVAL)
        {
            c.dropped_logged_at = Some(Instant::now());
            log_dropped(id, &label, std::mem::take(&mut c.dropped_unlogged));
        }
    }

    /// Spec §6 Deadline: a log line when the stream's reader missed its
    /// requested deadline again.
    pub(crate) fn log_deadlines(&mut self, id: u32) {
        let part = self.part;
        let Some(dds) = part.dds() else {
            return;
        };
        let Some(c) = self.calls.get_mut(&id) else {
            return;
        };
        let (Route::Subscribe(topic), Some(reader)) = (c.route, c.reader.as_ref()) else {
            return;
        };
        if part.topics[topic].deadline_ms.is_none() {
            return;
        }
        let missed = dds.deadlines_missed(reader);
        if missed > c.deadlines_logged {
            c.deadlines_logged = missed;
            event(Level::Warn, "deadline_missed")
                .num("stream_id", u64::from(id))
                .text("dds_topic", &part.label(&part.topics[topic].dds_name))
                .int("total_count", i64::from(missed))
                .emit();
        }
    }

    /// §9.2 drain: every open call ends with UNAVAILABLE (§4.5).
    pub(crate) fn drain(&mut self) {
        let ids: Vec<u32> = self.calls.keys().copied().collect();
        for id in ids {
            self.finish(
                id,
                Status::Unavailable,
                Some("the daemon is shutting down".into()),
            );
        }
    }

    /// The connection ends: the calls left end CANCELLED (the client gets
    /// no status) and are released.
    pub(crate) fn close_all(&mut self) {
        let calls: Vec<(u32, Call)> = self.calls.drain().collect();
        for (id, call) in calls {
            self.ended(
                &call.ended(id),
                Status::Cancelled,
                Some("the connection closed"),
            );
            self.release(id, call);
        }
    }

    /// Writes what may go out now.
    pub(crate) fn flush<S: Write>(&mut self, out: &mut S) -> std::io::Result<()> {
        let bytes = self.grpc.take_output();
        if bytes.is_empty() {
            return Ok(());
        }
        self.env
            .metrics
            .base
            .bytes_out_total
            .add(bytes.len() as u64);
        out.write_all(&bytes)?;
        out.flush()
    }
}
