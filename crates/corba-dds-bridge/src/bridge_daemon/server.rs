// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The IIOP and SSLIOP listeners, the connections and the requests (spec §4.2, §4.3, §4.7, §7).

#[allow(unused_imports)]
use super::prelude::*;

/// The writing half of a client connection, shared by the reader thread
/// of the connection and the reply dispatchers.
pub(crate) type ConnWriter = Arc<Mutex<Box<dyn Write + Send>>>;

/// An open client connection: its writer, the socket to close at
/// shutdown, the GIOP version its last message used, the client's address
/// and the listener it came in on.
pub(crate) struct Conn {
    pub(crate) writer: ConnWriter,
    pub(crate) sock: TcpStream,
    pub(crate) version: Mutex<Version>,
    /// The client's address.
    pub(crate) peer: String,
    /// Whether it came in on the SSLIOP listener.
    pub(crate) ssl: bool,
}

/// How long the shutdown waits for replies still pending (§9.2).
pub(crate) const DRAIN_LIMIT: Duration = Duration::from_secs(30);

/// What a connection needs to dispatch requests.
pub(crate) struct Dispatch {
    pub(crate) cfg: DaemonConfig,
    pub(crate) runtime: Arc<DcpsRuntime>,
    /// Object key → mapping index and its request writer.
    pub(crate) by_key: BTreeMap<Vec<u8>, (usize, EntityId)>,
    pub(crate) in_flight: Arc<InFlight>,
    pub(crate) next_id: AtomicU64,
    pub(crate) metrics: BridgeMetrics,
    /// Spec §8.2, when the registry was handed over.
    pub(crate) spec: Option<SpecMetrics>,
    /// ACL and SSLIOP material, replaced by a SIGHUP (§9.2).
    pub(crate) reloadable: std::sync::RwLock<Reloadable>,
    /// Open connections, by an id of the bridge.
    pub(crate) conns: Mutex<BTreeMap<u64, Arc<Conn>>>,
    pub(crate) next_conn: AtomicU64,
    /// Set at shutdown: new requests are refused while the pending ones
    /// are answered (§9.2).
    pub(crate) draining: AtomicBool,
    /// §7.2: the GSSUP credential source, when `corba.csiv2.enabled`.
    pub(crate) csiv2_store: Option<Arc<zerodds_corba_csiv2::GssupUserStore>>,
    /// `corba.naming.serve`: a `CosNaming::NamingContext` hosted on this
    /// daemon's own listener, sharing transport and servant code with
    /// `zerodds-naming-svc`. `None` when not configured.
    pub(crate) naming_servant: Option<Arc<zerodds_corba_cosnaming::servant::NamingServant>>,
    /// §8.3: the spans of the exchanges.
    pub(crate) tracer: Tracer,
    /// §1 L6: the tenant this participant serves; `None` for the one of
    /// `domain`.
    pub(crate) tenant: Option<String>,
    /// §6 `TRANSIENT_LOCAL`: the kept replies, per mapping.
    pub(crate) replay: Vec<Mutex<ReplayStore>>,
    /// §6: the last sample of each mapping's reply or notify topic.
    pub(crate) clocks: Vec<Mutex<SampleClock>>,
    /// When the participant started (the notify deadlines count from it).
    pub(crate) started: Instant,
    /// Whether a mapping has a deadline: the timeout thread then looks more
    /// often.
    pub(crate) has_deadlines: bool,
    /// Per mapping, the model of its generated types (§5.2): what a user
    /// exception's members are read with.
    pub(crate) models: Vec<Option<Arc<zerodds_ast::Ast>>>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn system_exception_with_sas(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    name: &str,
    completed: Completion,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    let body = system_exception_body(name, 0, completed, endianness);
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        ReplyStatusType::SystemException,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

/// Encodes and writes a `corba.naming.serve` [`ServantReply`] the same
/// way `zerodds-naming-svc`'s own `serve_connection` does -- the wire
/// shape of every reply status a `NamingServant` produces.
///
/// [`ServantReply`]: zerodds_corba_cosnaming::servant::ServantReply
#[allow(clippy::too_many_arguments)]
pub(crate) fn naming_dispatch_reply(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    respond: bool,
    dispatched: zerodds_corba_cosnaming::servant::ServantReply,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    if !respond {
        return Ok(());
    }
    use zerodds_corba_cosnaming::servant::ServantReply;
    let (status, body) = match dispatched {
        ServantReply::Ok(b) => (ReplyStatusType::NoException, b),
        ServantReply::UserException { body, .. } => (ReplyStatusType::UserException, body),
        ServantReply::BadOperation => (
            ReplyStatusType::SystemException,
            system_exception_body("BAD_OPERATION", 0, Completion::No, endianness),
        ),
        ServantReply::Marshal => (
            ReplyStatusType::SystemException,
            system_exception_body("MARSHAL", 0, Completion::No, endianness),
        ),
        ServantReply::ObjectNotExist => (
            ReplyStatusType::SystemException,
            system_exception_body("OBJECT_NOT_EXIST", 0, Completion::No, endianness),
        ),
        ServantReply::InterfaceRepositoryUnavailable => (
            ReplyStatusType::SystemException,
            system_exception_body("INTF_REPOS", 0, Completion::No, endianness),
        ),
    };
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        status,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

/// Encodes and writes an [`ObjectOpReply`] (CORBA 3.3 Part 2 §15.4.2's
/// six `CORBA::Object` operations) the same way every other outcome in
/// `on_request` is answered — `Ok` becomes `NO_EXCEPTION`,
/// `Marshal`/`InterfaceRepositoryUnavailable` the matching
/// `SystemException`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn object_op_reply(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    respond: bool,
    reply: ObjectOpReply,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    if !respond {
        return Ok(());
    }
    let (status, body) = match reply {
        ObjectOpReply::Ok(b) => (ReplyStatusType::NoException, b),
        ObjectOpReply::Marshal => (
            ReplyStatusType::SystemException,
            system_exception_body("MARSHAL", 0, Completion::No, endianness),
        ),
        ObjectOpReply::InterfaceRepositoryUnavailable => (
            ReplyStatusType::SystemException,
            system_exception_body("INTF_REPOS", 0, Completion::No, endianness),
        ),
    };
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        status,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

pub(crate) fn target_key(target: &TargetAddress) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(k) => Some(k.clone()),
        TargetAddress::Profile(p) => iiop_object_key(p),
        TargetAddress::Reference(info) => info.selected_profile().and_then(iiop_object_key),
    }
}

/// The object key of an IIOP profile a `ProfileAddr` or `ReferenceAddr`
/// target names (CORBA 3.3 Part 2 §9.4.2.1).
fn iiop_object_key(p: &zerodds_corba_giop::TargetProfile) -> Option<Vec<u8>> {
    if p.tag != ProfileId::InternetIop.as_u32() {
        return None;
    }
    let p = TaggedProfile {
        tag: ProfileId::InternetIop,
        profile_data: p.profile_data.clone(),
    };
    p.as_iiop().and_then(Result::ok).map(|b| b.object_key)
}

#[cfg(test)]
pub(crate) fn subject_allowed(cfg: &DaemonConfig, identities: &[String], repo_id: &str) -> bool {
    acl_allows(cfg.acl_default_deny, &cfg.acl_rules, identities, repo_id)
}

/// `identities`: what the connection proved — the certificate's subject DN
/// and its fingerprint; empty without a client certificate.
pub(crate) fn acl_allows(
    default_deny: bool,
    rules: &[AclRule],
    identities: &[String],
    repo_id: &str,
) -> bool {
    let rules: Vec<&AclRule> = rules
        .iter()
        .filter(|r| r.subject == "*" || identities.contains(&r.subject))
        .collect();
    let hit = rules.iter().any(|r| {
        r.allow_invoke.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => repo_id.starts_with(prefix),
            None => p == repo_id,
        })
    });
    hit || (!default_deny && rules.is_empty())
}

/// The part of the configuration a SIGHUP replaces (§9.2): the ACL, and the
/// SSLIOP server configuration for connections accepted afterwards.
pub(crate) struct Reloadable {
    pub(crate) acl_default_deny: bool,
    pub(crate) acl_rules: Vec<AclRule>,
    pub(crate) tls: Option<Arc<rustls::ServerConfig>>,
}

/// What `run_with_reload` needs to reload: the flag the signal watcher sets,
/// and how to read the configuration again (the same layered sources the
/// daemon started from).
pub struct Reload {
    /// Set on SIGHUP; cleared when the reload is done.
    pub flag: Arc<AtomicBool>,
    /// Reads and validates the configuration again.
    pub load: Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>,
}

/// The TLS server of the SSLIOP listener (§7.1): with `ca_file` it asks for
/// a client certificate and verifies one against the CA; with
/// `require_client_cert` a handshake without one fails, else a client may
/// go without (the IOR names `EstablishTrustInClient` as supported then).
pub(crate) fn ssliop_server_config(
    cfg: &DaemonConfig,
) -> Result<Arc<rustls::ServerConfig>, String> {
    use zerodds_bridge_security::tls;
    let cert = std::path::PathBuf::from(&cfg.ssliop_cert_file);
    let key = std::path::PathBuf::from(&cfg.ssliop_key_file);
    let ca = std::path::PathBuf::from(&cfg.ssliop_ca_file);
    let r = if cfg.ssliop_ca_file.is_empty() {
        tls::load_server_config(&cert, &key)
    } else if cfg.ssliop_require_client_cert {
        tls::load_server_config_with_client_auth(&cert, &key, &ca)
    } else {
        tls::load_server_config_with_optional_client_auth(&cert, &key, &ca)
    };
    r.map_err(|e| format!("SSLIOP: tls: {e}"))
}

/// One reload (§9.2): the new ACL and SSLIOP material take effect, the rest
/// needs a restart and is named. A configuration that does not load, or
/// SSLIOP material that does not, leaves everything as it was.
pub(crate) fn apply_reload(
    ds: &[Arc<Dispatch>],
    started: &DaemonConfig,
    load: &dyn Fn() -> Result<DaemonConfig, String>,
) {
    let new = match load() {
        Ok(c) => c,
        Err(e) => {
            log("reload_refused", &[("err", &e)]);
            return;
        }
    };
    let tls = if new.ssliop_enabled && started.ssliop_enabled {
        match ssliop_server_config(&new) {
            Ok(t) => Some(t),
            Err(e) => {
                log("reload_refused", &[("err", &e)]);
                return;
            }
        }
    } else {
        None
    };
    let mut restart: Vec<&str> = Vec::new();
    if new.mappings != started.mappings {
        restart.push("mappings");
    }
    if new.iiop_bind != started.iiop_bind || new.ssliop_bind != started.ssliop_bind {
        restart.push("binds");
    }
    if new.ssliop_enabled != started.ssliop_enabled {
        restart.push("corba.ssliop.enabled");
    }
    if new.domain != started.domain {
        restart.push("domain");
    }
    if new.tenants != started.tenants {
        restart.push("tenants");
    }
    for d in ds {
        if let Ok(mut r) = d.reloadable.write() {
            r.acl_default_deny = new.acl_default_deny;
            r.acl_rules = new.acl_rules.clone();
            if tls.is_some() {
                r.tls.clone_from(&tls);
            }
        }
    }
    log("reloaded", &[("applied", "acl, ssliop certificate")]);
    if !restart.is_empty() {
        log("reload_needs_restart", &[("changed", &restart.join(", "))]);
    }
}

/// Refuses a request with the system exception `name` (when it expects a
/// reply), and ends its exchange.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refuse(
    d: &Dispatch,
    conn: &ConnWriter,
    x: &ReqCtx,
    respond: bool,
    mapping: Option<usize>,
    name: &str,
    completed: Completion,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    if !respond {
        return Ok(());
    }
    let w0 = unix_ns();
    let r = system_exception_with_sas(
        d,
        conn,
        x.version,
        x.endianness,
        x.request_id,
        name,
        completed,
        sas_reply,
    );
    finish(
        d,
        x,
        mapping,
        Some(ReplyStatusType::SystemException),
        Some(name),
        None,
        None,
        Some((w0, unix_ns())),
    );
    r
}

/// The status of a GIOP reply a servant gave.
fn naming_status(r: &zerodds_corba_cosnaming::servant::ServantReply) -> ReplyStatusType {
    use zerodds_corba_cosnaming::servant::ServantReply;
    match r {
        ServantReply::Ok(_) => ReplyStatusType::NoException,
        ServantReply::UserException { .. } => ReplyStatusType::UserException,
        _ => ReplyStatusType::SystemException,
    }
}

/// One GIOP request (§4.3).
#[allow(clippy::too_many_lines)]
pub(crate) fn on_request(
    d: &Dispatch,
    conn: &ConnWriter,
    peer: &str,
    version: Version,
    endianness: Endianness,
    req: Request,
    identities: &[String],
) -> io::Result<()> {
    d.metrics.frames_in_total.inc();
    let respond = req.response_flags.response_expected();
    let x = ReqCtx {
        peer: peer.to_string(),
        version,
        endianness,
        request_id: req.request_id,
        operation: req.operation.clone(),
        bytes: req.body.len(),
        started: Instant::now(),
        start_ns: unix_ns(),
    };
    if d.draining.load(Ordering::SeqCst) {
        log("refused_while_draining", &[("operation", &req.operation)]);
        return refuse(
            d,
            conn,
            &x,
            respond,
            None,
            "TRANSIENT",
            Completion::No,
            None,
        );
    }

    // §7.2: CSIv2, evaluated before the target is looked up. Every
    // mechanism of the IOR requires trust in the client
    // (`ior::csiv2_mech_list`): a verified client certificate or GSSUP
    // credentials. A rejected EstablishContext (bad credentials, an
    // unsupported asserted identity) refuses the request, and so does a
    // request with neither. Once a SAS decision is made, every reply this
    // request gets carries it.
    let mut identities: Vec<String> = identities.to_vec();
    let mut sas_reply: Option<zerodds_corba_csiv2::sas::SasMessage> = None;
    if let Some(store) = &d.csiv2_store {
        let refused = |reason: &str| {
            if let Some(sm) = d.spec.as_ref() {
                sm.csiv2_failure(reason);
            }
            log(
                "csiv2_rejected",
                &[("operation", &req.operation), ("reason", reason)],
            );
        };
        match crate::csiv2_wire::evaluate(&req.service_context, store, d.cfg.csiv2_target_supports)
        {
            crate::csiv2_wire::Csiv2Outcome::Accepted { principal, reply } => {
                identities.push(format!("gssup:{principal}"));
                sas_reply = Some(reply);
            }
            crate::csiv2_wire::Csiv2Outcome::Rejected { reply } => {
                refused(csiv2_reason(&reply));
                return refuse(
                    d,
                    conn,
                    &x,
                    respond,
                    None,
                    "NO_PERMISSION",
                    Completion::No,
                    Some(&reply),
                );
            }
            crate::csiv2_wire::Csiv2Outcome::NotOffered if identities.is_empty() => {
                refused("missing");
                return refuse(
                    d,
                    conn,
                    &x,
                    respond,
                    None,
                    "NO_PERMISSION",
                    Completion::No,
                    None,
                );
            }
            crate::csiv2_wire::Csiv2Outcome::NotOffered => {}
        }
    }
    let identities = identities.as_slice();

    let key = target_key(&req.target);
    let Some((index, writer)) = key.as_ref().and_then(|k| d.by_key.get(k).copied()) else {
        // corba.naming.serve: this object key is not a DDS<->GIOP mapping,
        // but it may be the NameService root or one of its sub-contexts /
        // live BindingIterators -- try the hosted NamingServant before
        // refusing OBJECT_NOT_EXIST. `dispatch` itself reports
        // `ObjectNotExist` for a key it does not know either, so this
        // never masks a genuinely wrong object key.
        if let (Some(servant), Some(k)) = (&d.naming_servant, key.as_deref()) {
            // CORBA 3.3 Part 2 §9.4.2.1: the servant reads the body at its
            // offset in the message.
            let dispatched = match req.body_offset(version, endianness) {
                Ok(offset) => servant.dispatch(
                    k,
                    &req.operation,
                    &zerodds_corba_giop::AlignedBody::new(&req.body, offset),
                    endianness,
                ),
                Err(_) => zerodds_corba_cosnaming::servant::ServantReply::Marshal,
            };
            if !matches!(
                dispatched,
                zerodds_corba_cosnaming::servant::ServantReply::ObjectNotExist
            ) {
                log("naming_dispatch", &[("operation", &req.operation)]);
                let status = naming_status(&dispatched);
                let r = naming_dispatch_reply(
                    d,
                    conn,
                    version,
                    endianness,
                    req.request_id,
                    respond,
                    dispatched,
                    sas_reply.as_ref(),
                );
                finish(
                    d,
                    &x,
                    None,
                    respond.then_some(status),
                    None,
                    None,
                    None,
                    None,
                );
                return r;
            }
        }
        // CORBA 3.3 Part 2 §15.4.2: `_non_existent`/`_not_existent` answers
        // `true` for a key nothing here serves (neither a mapping nor,
        // above, the NameService), instead of the `OBJECT_NOT_EXIST` every
        // other operation gets on this path.
        if let Some(body) = object_ops::unknown_object_reply(&req.operation, endianness) {
            log("object_op", &[("operation", &req.operation)]);
            let r = object_op_reply(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                respond,
                ObjectOpReply::Ok(body),
                sas_reply.as_ref(),
            );
            finish(
                d,
                &x,
                None,
                respond.then_some(ReplyStatusType::NoException),
                None,
                None,
                None,
                None,
            );
            return r;
        }
        log("unknown_object", &[("operation", &req.operation)]);
        return refuse(
            d,
            conn,
            &x,
            respond,
            None,
            "OBJECT_NOT_EXIST",
            Completion::No,
            sas_reply.as_ref(),
        );
    };
    let m = &d.cfg.mappings[index];
    let allowed = || {
        d.reloadable
            .read()
            .map(|r| acl_allows(r.acl_default_deny, &r.acl_rules, identities, &m.repo_id))
            .unwrap_or(false)
    };
    let denied = || {
        log(
            "acl_denied",
            &[
                ("repo_id", &m.repo_id),
                (
                    "subject",
                    identities.first().map_or("(none)", String::as_str),
                ),
            ],
        );
        if respond {
            count_refused(d, index);
        }
    };
    if object_ops::is_object_operation(&req.operation) {
        // Same ACL gate as the mapped operation below (§7.3) — a client
        // without `allow_invoke` for this `repo_id` learns nothing about
        // it, not even via `_is_a`/`_repository_id`.
        if !allowed() {
            denied();
            return refuse(
                d,
                conn,
                &x,
                respond,
                Some(index),
                "NO_PERMISSION",
                Completion::No,
                sas_reply.as_ref(),
            );
        }
        // CORBA 3.3 Part 2 §9.4.2.1: the arguments are aligned from the
        // message start, the body read at its offset.
        let Ok(offset) = req.body_offset(version, endianness) else {
            return refuse(
                d,
                conn,
                &x,
                respond,
                Some(index),
                "MARSHAL",
                Completion::No,
                sas_reply.as_ref(),
            );
        };
        let aligned = zerodds_corba_giop::AlignedBody::new(&req.body, offset);
        if let Some(reply) =
            object_ops::dispatch_with_bases(&req.operation, &aligned, endianness, &m.repo_id, &[])
        {
            log(
                "object_op",
                &[("operation", &req.operation), ("repo_id", &m.repo_id)],
            );
            let status = match &reply {
                ObjectOpReply::Ok(_) => ReplyStatusType::NoException,
                _ => ReplyStatusType::SystemException,
            };
            let r = object_op_reply(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                respond,
                reply,
                sas_reply.as_ref(),
            );
            finish(
                d,
                &x,
                Some(index),
                respond.then_some(status),
                None,
                None,
                None,
                None,
            );
            return r;
        }
    }
    if req.operation != m.operation {
        log(
            "bad_operation",
            &[("operation", &req.operation), ("repo_id", &m.repo_id)],
        );
        if respond {
            count_refused(d, index);
        }
        return refuse(
            d,
            conn,
            &x,
            respond,
            Some(index),
            "BAD_OPERATION",
            Completion::No,
            sas_reply.as_ref(),
        );
    }
    if !allowed() {
        denied();
        return refuse(
            d,
            conn,
            &x,
            respond,
            Some(index),
            "NO_PERMISSION",
            Completion::No,
            sas_reply.as_ref(),
        );
    }
    if version != Version::V1_2 {
        // The DDS sample carries the body with GIOP 1.2's alignment; an
        // older version aligns it from another origin.
        log("giop_version_refused", &[("operation", &req.operation)]);
        if respond {
            count_refused(d, index);
        }
        return refuse(
            d,
            conn,
            &x,
            respond,
            Some(index),
            "NO_IMPLEMENT",
            Completion::No,
            sas_reply.as_ref(),
        );
    }
    // §6 TRANSIENT_LOCAL: a request the store holds a reply for is answered
    // from it, without a DDS sample.
    let replay_key = (respond && m.durability == zerodds_qos::DurabilityKind::TransientLocal)
        .then(|| replay_key(&req.operation, endianness, &req.body));
    let kept = replay_key.as_ref().and_then(|k| {
        d.replay.get(index).and_then(|s| {
            s.lock().ok().and_then(|mut s| {
                s.get(k, m.lifespan_ms.map(Duration::from_millis), Instant::now())
            })
        })
    });
    if let Some((status, body)) = kept {
        log(
            "replayed",
            &[("operation", &req.operation), ("repo_id", &m.repo_id)],
        );
        if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
            om.replies_sent.inc();
            om.requests[match status {
                ReplyStatusType::NoException => Outcome::Ok,
                ReplyStatusType::UserException => Outcome::User,
                _ => Outcome::System,
            } as usize]
                .inc();
        }
        let w0 = unix_ns();
        let msg = encode_reply_with_context(
            version,
            endianness,
            req.request_id,
            status,
            body,
            sas_service_context(sas_reply.as_ref(), endianness),
        )?;
        d.metrics.frames_out_total.inc();
        let r = write_giop(conn, &msg, d.cfg.fragment_size, req.request_id, endianness);
        finish(
            d,
            &x,
            Some(index),
            Some(status),
            None,
            None,
            None,
            Some((w0, unix_ns())),
        );
        return r;
    }
    let id = d.next_id.fetch_add(1, Ordering::Relaxed);
    let mut sample = Vec::with_capacity(8 + req.body.len());
    sample.extend_from_slice(&match endianness {
        Endianness::Little => id.to_le_bytes(),
        Endianness::Big => id.to_be_bytes(),
    });
    sample.extend_from_slice(&req.body);
    if respond {
        let mut p = d
            .in_flight
            .pending
            .lock()
            .map_err(|_| io::Error::other("pending map poisoned"))?;
        p.insert(
            id,
            Pending {
                conn: Arc::clone(conn),
                ctx: x.clone(),
                deadline: Instant::now() + Duration::from_millis(m.timeout_ms),
                mapping: index,
                sas_reply: sas_reply.clone(),
                publish: None,
                replay_key,
            },
        );
        if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
            om.pending.inc();
        }
        d.in_flight.changed.notify_all();
    }
    use zerodds_rtps::publication_data::data_representation as dr;
    let _ = d
        .runtime
        .set_user_writer_data_rep_override(writer, Some(vec![dr::XCDR]));
    let _ = d
        .runtime
        .set_user_writer_byte_order_override(writer, matches!(endianness, Endianness::Big));
    log(
        "request",
        &[("operation", &req.operation), ("id", &id.to_string())],
    );
    let p0 = unix_ns();
    if let Err(e) = d.runtime.write_user_sample_borrowed(writer, &sample) {
        d.metrics.errors_total.inc();
        log(
            "dds_write_failed",
            &[
                ("topic", &m.request_topic.dds_name),
                ("err", &format!("{e:?}")),
            ],
        );
        if respond {
            let removed = d
                .in_flight
                .pending
                .lock()
                .ok()
                .and_then(|mut p| p.remove(&id));
            if let Some(p) = removed {
                count_reply(d, &p, Outcome::System);
            }
        }
        return refuse(
            d,
            conn,
            &x,
            respond,
            Some(index),
            "TRANSIENT",
            Completion::No,
            sas_reply.as_ref(),
        );
    }
    let p1 = unix_ns();
    d.metrics.dds_samples_out_total.inc();
    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
        om.published.inc();
    }
    if respond {
        if let Ok(mut p) = d.in_flight.pending.lock()
            && let Some(p) = p.get_mut(&id)
        {
            p.publish = Some((p0, p1));
        }
    } else {
        // A oneway: the exchange ends with the sample.
        finish(d, &x, Some(index), None, None, Some((p0, p1)), None, None);
    }
    Ok(())
}

/// The `LocateReply` status of `target` (§4.7): `OBJECT_HERE` for a
/// mapping's object or one of the hosted NameService, `UNKNOWN_OBJECT`
/// otherwise.
pub(crate) fn locate_status(d: &Dispatch, target: &TargetAddress) -> LocateStatusType {
    if target_key(target).is_some_and(|k| knows(d, &k)) {
        LocateStatusType::ObjectHere
    } else {
        LocateStatusType::UnknownObject
    }
}

/// Whether `key` names an object here: a mapping's, or a context or
/// iterator of the hosted NameService.
pub(crate) fn knows(d: &Dispatch, key: &[u8]) -> bool {
    d.by_key.contains_key(key)
        || d.naming_servant.as_ref().is_some_and(|s| {
            s.graph().is_iterator_key(key) || s.graph().context_for_key(key).is_some()
        })
}

/// §4.7 with `corba.ssliop.required`: the IOR a client on plain IIOP is
/// forwarded to — the object's own (port 0, the SSLIOP port in
/// `TAG_SSL_SEC_TRANS`), or for an object of the NameService one with its
/// key and an empty type id.
pub(crate) fn forward_ior(d: &Dispatch, key: &[u8]) -> Option<Ior> {
    if let Some((index, _)) = d.by_key.get(key) {
        return build_ior(&d.cfg, &d.cfg.mappings[*index], &d.cfg.iiop_advertise_host).ok();
    }
    knows(d, key)
        .then(|| ior_for_key(&d.cfg, "", key.to_vec(), &d.cfg.iiop_advertise_host).ok())
        .flatten()
}

/// An IOR as the body of `OBJECT_FORWARD` or `LOCATION_FORWARD` (GIOP
/// §15.4.3.2, §15.4.6.2): the object reference in CDR, in the byte order
/// of the message.
pub(crate) fn forward_body(ior: &Ior, endianness: Endianness) -> io::Result<Vec<u8>> {
    let mut w = zerodds_wire::Writer::new(endianness);
    ior.encode(&mut w)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
    Ok(w.finish())
}

/// A request on plain IIOP while `corba.ssliop.required`: answered with
/// `LOCATION_FORWARD` to the object's SSLIOP-only IOR, or `OBJECT_NOT_EXIST`
/// for a key nothing here serves.
pub(crate) fn forward_request(
    d: &Dispatch,
    conn: &ConnWriter,
    peer: &str,
    version: Version,
    endianness: Endianness,
    req: &Request,
) -> io::Result<()> {
    d.metrics.frames_in_total.inc();
    if !req.response_flags.response_expected() {
        return Ok(());
    }
    let x = ReqCtx {
        peer: peer.to_string(),
        version,
        endianness,
        request_id: req.request_id,
        operation: req.operation.clone(),
        bytes: req.body.len(),
        started: Instant::now(),
        start_ns: unix_ns(),
    };
    let key = target_key(&req.target);
    let Some(ior) = key.as_deref().and_then(|k| forward_ior(d, k)) else {
        log("unknown_object", &[("operation", &req.operation)]);
        return refuse(
            d,
            conn,
            &x,
            true,
            None,
            "OBJECT_NOT_EXIST",
            Completion::No,
            None,
        );
    };
    log("forward", &[("operation", &req.operation), ("peer", peer)]);
    let w0 = unix_ns();
    let msg = encode_reply_with_context(
        version,
        endianness,
        req.request_id,
        ReplyStatusType::LocationForward,
        forward_body(&ior, endianness)?,
        ServiceContextList::default(),
    )?;
    d.metrics.frames_out_total.inc();
    let r = write_giop(conn, &msg, d.cfg.fragment_size, req.request_id, endianness);
    let index = key.as_ref().and_then(|k| d.by_key.get(k)).map(|(i, _)| *i);
    finish(
        d,
        &x,
        index,
        Some(ReplyStatusType::LocationForward),
        None,
        None,
        None,
        Some((w0, unix_ns())),
    );
    r
}

/// Sends `MessageError` (GIOP §15.4.8) for a message whose header is not
/// GIOP, whose version is unknown, or which does not decode, in the
/// version of the message where it is one of 1.0 to 1.2, else 1.2.
pub(crate) fn message_error(d: &Dispatch, conn: &ConnWriter, header: &[u8], why: &str) {
    if let Some(sm) = d.spec.as_ref() {
        sm.giop_message_errors.inc();
    }
    log("message_error", &[("err", why)]);
    let version = match header.get(4..6) {
        Some([1, minor]) if *minor <= 2 => Version {
            major: 1,
            minor: *minor,
        },
        _ => Version::V1_2,
    };
    let le = header.get(6).is_some_and(|f| f & 0x01 != 0);
    let error = encode_message(
        version,
        if le {
            Endianness::Little
        } else {
            Endianness::Big
        },
        false,
        &Message::MessageError(zerodds_corba_giop::MessageError),
    )
    .unwrap_or_default();
    let _ = conn
        .lock()
        .map(|mut w| w.write_all(&error).and_then(|()| w.flush()));
}

/// Writes a `LocateReply`.
fn locate_reply(
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    status: LocateStatusType,
    body: Vec<u8>,
) -> io::Result<()> {
    let reply = LocateReply {
        request_id,
        locate_status: status,
        body,
    };
    let bytes = encode_message(version, endianness, false, &Message::LocateReply(reply))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
    let mut w = conn
        .lock()
        .map_err(|_| io::Error::other("connection writer poisoned"))?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Serves one client connection until it closes.
#[allow(clippy::too_many_lines)]
pub(crate) fn serve_connection(
    d: &Dispatch,
    mut reader: Box<dyn Read + Send>,
    entry: &Conn,
    identities: Vec<String>,
) -> io::Result<()> {
    let conn = Arc::clone(&entry.writer);
    let forward = d.cfg.ssliop_required && !entry.ssl;
    let mut acc: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut frag = Reassembly::default();
    loop {
        // Whole messages in the buffer first.
        while acc.len() >= 12 {
            if &acc[..4] != b"GIOP" {
                message_error(d, &conn, &acc[..12], "not a GIOP message");
                return Ok(());
            }
            if acc[4] != 1 || acc[5] > 2 {
                message_error(
                    d,
                    &conn,
                    &acc[..12],
                    &format!("GIOP {}.{} is not one of 1.0, 1.1, 1.2", acc[4], acc[5]),
                );
                return Ok(());
            }
            let le = acc[6] & 0x01 != 0;
            let size = if le {
                u32::from_le_bytes([acc[8], acc[9], acc[10], acc[11]])
            } else {
                u32::from_be_bytes([acc[8], acc[9], acc[10], acc[11]])
            } as usize;
            if size > d.cfg.max_message_size {
                log("message_too_large", &[("size", &size.to_string())]);
                message_error(d, &conn, &acc[..12], "larger than max_message_size");
                return Ok(());
            }
            if acc.len() < 12 + size {
                break;
            }
            let raw: Vec<u8> = acc.drain(..12 + size).collect();
            let header: Vec<u8> = raw[..12].to_vec();
            let whole = match frag.feed(raw) {
                Ok(Some(w)) => w,
                Ok(None) => continue,
                Err(e) => {
                    message_error(d, &conn, &header, &e.to_string());
                    return Ok(());
                }
            };
            let version = Version {
                major: whole[4],
                minor: whole[5],
            };
            if let Ok(mut v) = entry.version.lock() {
                *v = version;
            }
            let (msg, endianness, _) = match decode_message_ctx(&whole) {
                Ok(m) => m,
                Err(e) => {
                    message_error(d, &conn, &header, &format!("giop: {e:?}"));
                    return Ok(());
                }
            };
            match msg {
                Message::Request(req) if forward => {
                    forward_request(d, &conn, &entry.peer, version, endianness, &req)?;
                }
                Message::Request(req) => {
                    on_request(d, &conn, &entry.peer, version, endianness, req, &identities)?
                }
                Message::LocateRequest(lr) => {
                    if let Some(sm) = d.spec.as_ref() {
                        sm.locate_requests.inc();
                    }
                    let ior = forward
                        .then(|| target_key(&lr.target).and_then(|k| forward_ior(d, &k)))
                        .flatten();
                    match ior {
                        Some(ior) => locate_reply(
                            &conn,
                            version,
                            endianness,
                            lr.request_id,
                            LocateStatusType::ObjectForward,
                            forward_body(&ior, endianness)?,
                        )?,
                        None => locate_reply(
                            &conn,
                            version,
                            endianness,
                            lr.request_id,
                            locate_status(d, &lr.target),
                            Vec::new(),
                        )?,
                    }
                }
                Message::CancelRequest(c) => {
                    cancel(d, &conn, c.request_id);
                }
                Message::CloseConnection(_) | Message::MessageError(_) => return Ok(()),
                other => {
                    log(
                        "unsupported_message",
                        &[("type", &format!("{:?}", other.message_type()))],
                    );
                }
            }
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        d.metrics.bytes_in_total.add(n as u64);
        acc.extend_from_slice(&buf[..n]);
    }
}

/// A TLS stream shared by the reading and the writing half of a
/// connection; the reader holds the lock only while TLS has plaintext.
pub(crate) struct TlsShared(Arc<Mutex<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>>);

impl Read for TlsShared {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            {
                let mut s = self
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("TLS stream poisoned"))?;
                match s.conn.reader().read(buf) {
                    Ok(n) => return Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e),
                }
            }
            // Wait for ciphertext without holding the lock.
            let sock = self
                .0
                .lock()
                .map_err(|_| io::Error::other("TLS stream poisoned"))?
                .sock
                .try_clone()?;
            let mut raw = [0u8; 16 * 1024];
            let n = (&sock).read(&mut raw)?;
            if n == 0 {
                return Ok(0);
            }
            let mut s = self
                .0
                .lock()
                .map_err(|_| io::Error::other("TLS stream poisoned"))?;
            let mut rest: &[u8] = &raw[..n];
            while !rest.is_empty() {
                s.conn.read_tls(&mut rest)?;
                s.conn
                    .process_new_packets()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            }
        }
    }
}

impl Write for TlsShared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("TLS stream poisoned"))?
            .write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("TLS stream poisoned"))?
            .flush()
    }
}

/// Accepts the connections of one listener; each is served by `pick`'s
/// participant (§1 L6: by the client certificate's subject).
pub(crate) fn accept_loop(
    listener: &TcpListener,
    tenants: &Arc<Tenants>,
    shutdown: &AtomicBool,
    ssl: bool,
) {
    let _ = listener.set_nonblocking(true);
    while !shutdown.load(Ordering::SeqCst) {
        let (stream, peer) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                log("accept_error", &[("err", &e.to_string())]);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let tenants = Arc::clone(tenants);
        // The SSLIOP configuration at accept time: a SIGHUP replaces it for
        // the connections that follow.
        let tls = if ssl {
            tenants
                .default
                .reloadable
                .read()
                .ok()
                .and_then(|r| r.tls.clone())
        } else {
            None
        };
        if ssl && tls.is_none() {
            log(
                "conn_error",
                &[
                    ("peer", &peer.to_string()),
                    ("err", "no SSLIOP configuration"),
                ],
            );
            continue;
        }
        std::thread::spawn(move || {
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_nodelay(true);
            let metrics = tenants.default.metrics.clone();
            metrics.connections_total.inc();
            metrics.connections_active.inc();
            let result = (|| -> io::Result<()> {
                let register = |d: &Dispatch, writer: ConnWriter, sock: TcpStream| {
                    let id = d.next_conn.fetch_add(1, Ordering::Relaxed);
                    let entry = Arc::new(Conn {
                        writer,
                        sock,
                        version: Mutex::new(Version::V1_2),
                        peer: peer.to_string(),
                        ssl,
                    });
                    if let Ok(mut c) = d.conns.lock() {
                        c.insert(id, Arc::clone(&entry));
                    }
                    (id, entry)
                };
                let (d, id, served) = match tls {
                    None => {
                        let d = Arc::clone(&tenants.default);
                        let writer: ConnWriter =
                            Arc::new(Mutex::new(Box::new(stream.try_clone()?)));
                        let (id, entry) = register(&d, writer, stream.try_clone()?);
                        let served = serve_connection(&d, Box::new(stream), &entry, Vec::new());
                        (d, id, served)
                    }
                    Some(cfg) => {
                        let conn = rustls::ServerConnection::new(cfg)
                            .map_err(|e| io::Error::other(format!("TLS: {e}")))?;
                        let mut tls = rustls::StreamOwned::new(conn, stream);
                        while tls.conn.is_handshaking() {
                            tls.conn.complete_io(&mut tls.sock)?;
                        }
                        // §7.3: the subject DN and the fingerprint
                        // `mtls:<SHA-256>`; an ACL rule may name either.
                        let subject = zerodds_bridge_security::ctx::extract_mtls_subject(&tls.conn)
                            .map(|s| {
                                let mut ids = vec![s.name];
                                ids.extend(s.fingerprint);
                                ids
                            })
                            .unwrap_or_default();
                        // §1 L6: the session is bound to the tenant its
                        // subject names, for its life.
                        let d = tenants.pick(&subject);
                        let sock = tls.sock.try_clone()?;
                        let shared = Arc::new(Mutex::new(tls));
                        let writer: ConnWriter =
                            Arc::new(Mutex::new(Box::new(TlsShared(Arc::clone(&shared)))));
                        let (id, entry) = register(&d, writer, sock);
                        let served =
                            serve_connection(&d, Box::new(TlsShared(shared)), &entry, subject);
                        (d, id, served)
                    }
                };
                if let Ok(mut c) = d.conns.lock() {
                    c.remove(&id);
                }
                served
            })();
            if let Err(e) = result {
                metrics.errors_total.inc();
                log(
                    "conn_error",
                    &[("peer", &peer.to_string()), ("err", &e.to_string())],
                );
            }
            metrics.connections_active.dec();
        });
    }
}

/// §1 L6: the participants of the daemon — the one of `domain`, and one per
/// tenant with the subjects bound to it.
pub(crate) struct Tenants {
    /// The participant of `domain`: plain IIOP and every SSLIOP session no
    /// tenant names.
    pub(crate) default: Arc<Dispatch>,
    /// Each tenant's subjects and participant.
    pub(crate) bound: Vec<(Vec<String>, Arc<Dispatch>)>,
}

impl Tenants {
    /// The participant of a session whose certificate proved `identities`.
    pub(crate) fn pick(&self, identities: &[String]) -> Arc<Dispatch> {
        self.bound
            .iter()
            .find(|(subjects, _)| subjects.iter().any(|s| identities.contains(s)))
            .map_or_else(|| Arc::clone(&self.default), |(_, d)| Arc::clone(d))
    }

    /// Every participant.
    pub(crate) fn all(&self) -> Vec<Arc<Dispatch>> {
        let mut v = vec![Arc::clone(&self.default)];
        v.extend(self.bound.iter().map(|(_, d)| Arc::clone(d)));
        v
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::bridge_daemon::config::tests::load;

    #[test]
    fn corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes() {
        let c = load("acl:\n  default_deny: true\n  rules:\n    - subject: \"CN=trader\"\n      allow_invoke: [\"IDL:zerodds/MarketData/*\"]\n").unwrap();
        assert!(subject_allowed(
            &c,
            &["CN=trader".to_string()],
            "IDL:zerodds/MarketData/Quote:1.0"
        ));
        assert!(!subject_allowed(
            &c,
            &["CN=trader".to_string()],
            "IDL:zerodds/Trading/X:1.0"
        ));
        assert!(!subject_allowed(
            &c,
            &[],
            "IDL:zerodds/MarketData/Quote:1.0"
        ));
        let open = load("acl:\n  default_deny: false\n").unwrap();
        assert!(subject_allowed(&open, &[], "IDL:any:1.0"));
    }

    /// A connection writer that keeps what is written.
    #[derive(Clone, Default)]
    pub(crate) struct Sink(pub(crate) Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Sink {
        pub(crate) fn writer(&self) -> ConnWriter {
            Arc::new(Mutex::new(Box::new(self.clone())))
        }
    }

    /// A participant of `yaml` on a leased domain, with the metrics of
    /// §8.2.
    pub(crate) fn fixture(yaml: &str) -> (Arc<Dispatch>, zerodds_test_domains::DomainLease) {
        let lease = zerodds_test_domains::lease_domain();
        let mut cfg = load(yaml).unwrap();
        cfg.domain = lease.id();
        let registry = Arc::new(zerodds_monitor::Registry::new());
        let metrics = BridgeMetrics::register_shared(&registry);
        let (d, _, _) = crate::bridge_daemon::build_dispatch(
            &cfg,
            None,
            RuntimeConfig::default(),
            &metrics,
            &Tracer::default(),
            None,
        )
        .unwrap();
        (d, lease)
    }

    /// A request of mapping 0 waiting under the bridge's `id`, with the
    /// GIOP request id `request_id`, on `conn`.
    pub(crate) fn pending(d: &Dispatch, id: u64, request_id: u32, conn: ConnWriter) {
        d.in_flight.pending.lock().unwrap().insert(
            id,
            Pending {
                conn,
                ctx: ReqCtx {
                    peer: "127.0.0.1:1".into(),
                    version: Version::V1_2,
                    endianness: Endianness::Little,
                    request_id,
                    operation: d.cfg.mappings[0].operation.clone(),
                    bytes: 0,
                    started: Instant::now(),
                    start_ns: unix_ns(),
                },
                deadline: Instant::now() + Duration::from_secs(60),
                mapping: 0,
                sas_reply: None,
                publish: None,
                replay_key: None,
            },
        );
    }

    /// The GIOP messages written to `sink`.
    pub(crate) fn messages(sink: &Sink) -> Vec<Message> {
        let bytes = sink.0.lock().unwrap().clone();
        let mut out = Vec::new();
        let mut at = 0;
        while at + 12 <= bytes.len() {
            let le = bytes[at + 6] & 1 != 0;
            let b: [u8; 4] = bytes[at + 8..at + 12].try_into().unwrap();
            let size = if le {
                u32::from_le_bytes(b)
            } else {
                u32::from_be_bytes(b)
            } as usize;
            out.push(decode_message_ctx(&bytes[at..at + 12 + size]).unwrap().0);
            at += 12 + size;
        }
        out
    }

    /// The replies written to `sink`.
    pub(crate) fn replies(sink: &Sink) -> Vec<zerodds_corba_giop::Reply> {
        messages(sink)
            .into_iter()
            .filter_map(|m| match m {
                Message::Reply(r) => Some(r),
                _ => None,
            })
            .collect()
    }

    const CALC: &str = "mappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n";

    fn request(d: &Dispatch, operation: &str) -> Request {
        Request {
            request_id: 4,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(object_key(&d.cfg.mappings[0])),
            operation: operation.into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: vec![1, 0, 0, 0],
        }
    }

    fn exception(r: &zerodds_corba_giop::Reply) -> String {
        exception_name(&r.body, Endianness::Little).unwrap()
    }

    /// §4.3: a mapped operation over GIOP 1.0 or 1.1 is `NO_IMPLEMENT`.
    #[test]
    fn corbabridge_4_3_a_mapped_operation_over_giop_1_1_is_no_implement() {
        let (d, _lease) = fixture(CALC);
        let sink = Sink::default();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_1,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        let r = replies(&sink);
        assert_eq!(r[0].reply_status, ReplyStatusType::SystemException);
        assert_eq!(exception(&r[0]), "NO_IMPLEMENT");
        assert!(d.in_flight.pending.lock().unwrap().is_empty());
    }

    /// §4.3: a DDS write that fails is `TRANSIENT`, and the request does not
    /// stay pending.
    #[test]
    fn corbabridge_4_3_a_failed_dds_write_is_transient() {
        let (d, _lease) = fixture(CALC);
        let (_, writer) = d.by_key[&object_key(&d.cfg.mappings[0])];
        d.runtime.remove_user_writer(writer);
        let sink = Sink::default();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        let r = replies(&sink);
        assert_eq!(exception(&r[0]), "TRANSIENT");
        assert!(d.in_flight.pending.lock().unwrap().is_empty());
    }

    /// §4.3: an unknown object key is `OBJECT_NOT_EXIST`, another operation
    /// `BAD_OPERATION`, the mapped one waits for its reply.
    #[test]
    fn corbabridge_4_3_the_target_and_the_operation_decide() {
        let (d, _lease) = fixture(CALC);
        let sink = Sink::default();
        let mut unknown = request(&d, "add");
        unknown.target = TargetAddress::Key(vec![9; 16]);
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            unknown,
            &[],
        )
        .unwrap();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "sub"),
            &[],
        )
        .unwrap();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        let r = replies(&sink);
        assert_eq!(exception(&r[0]), "OBJECT_NOT_EXIST");
        assert_eq!(exception(&r[1]), "BAD_OPERATION");
        assert_eq!(r.len(), 2);
        assert_eq!(d.in_flight.pending.lock().unwrap().len(), 1);
    }

    /// §4.7: `OBJECT_HERE` for a mapping's object, `UNKNOWN_OBJECT` for any
    /// other key.
    #[test]
    fn corbabridge_4_7_locate_names_known_and_unknown_objects() {
        let (d, _lease) = fixture(CALC);
        let key = object_key(&d.cfg.mappings[0]);
        assert_eq!(
            locate_status(&d, &TargetAddress::Key(key)),
            LocateStatusType::ObjectHere
        );
        assert_eq!(
            locate_status(&d, &TargetAddress::Key(vec![1; 16])),
            LocateStatusType::UnknownObject
        );
    }

    /// §4.7 with `corba.ssliop.required`: the forward IOR names port 0 and
    /// the SSLIOP port in `TAG_SSL_SEC_TRANS`, which requires integrity and
    /// confidentiality.
    #[test]
    fn corbabridge_4_7_the_forward_ior_of_ssliop_only_names_the_ssl_port() {
        use zerodds_corba_ior::component_tags::ComponentId;
        use zerodds_corba_ior::components::{StructuredComponent, TaggedComponent};
        let (d, _lease) = fixture(
            "corba:\n  iiop:\n    bind: \"127.0.0.1:7001\"\n  ssliop:\n    enabled: true\n    required: true\n    bind: \"127.0.0.1:7002\"\n    cert_file: c.pem\n    key_file: k.pem\nmappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n",
        );
        let ior = forward_ior(&d, &object_key(&d.cfg.mappings[0])).unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        assert_eq!(body.port, 0);
        let ssl = body
            .components
            .iter()
            .find(|c| c.tag == ComponentId::SslSecTrans.as_u32())
            .unwrap();
        let StructuredComponent::Ssl(ssl) = (TaggedComponent {
            tag: ComponentId::SslSecTrans,
            component_data: ssl.component_data.clone(),
        })
        .structured()
        .unwrap() else {
            panic!("not the SSL component");
        };
        assert_eq!(ssl.port, 7002);
        assert_eq!(
            ssl.target_requires & 0x06,
            0x06,
            "Integrity and Confidentiality"
        );
        assert_eq!(ssl.target_supports & 0x01, 0, "no NoProtection");
        assert!(forward_ior(&d, &[3; 16]).is_none());
        let reparsed = {
            let b = forward_body(&ior, Endianness::Little).unwrap();
            let mut r = zerodds_wire::Reader::new(&b, Endianness::Little);
            Ior::decode(&mut r).unwrap()
        };
        assert_eq!(reparsed, ior);
    }

    /// GIOP §15.4.8: a message whose magic is not `GIOP` or whose version is
    /// unknown gets `MessageError`, and the connection ends.
    #[test]
    fn corbabridge_4_2_a_message_that_is_not_giop_gets_message_error() {
        let (d, _lease) = fixture(CALC);
        for bad in [
            b"HTTP/1.1 200 OK".to_vec(),
            b"GIOP\x01\x03\x01\x00\x00\x00\x00\x00".to_vec(),
        ] {
            let sink = Sink::default();
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            let sock = TcpStream::connect(l.local_addr().unwrap()).unwrap();
            let entry = Conn {
                writer: sink.writer(),
                sock,
                version: Mutex::new(Version::V1_2),
                peer: "p".into(),
                ssl: false,
            };
            serve_connection(&d, Box::new(io::Cursor::new(bad)), &entry, Vec::new()).unwrap();
            let m = messages(&sink);
            assert!(matches!(m[..], [Message::MessageError(_)]), "{m:?}");
        }
    }

    /// §7.2: with CSIv2, a request without GSSUP and without a client
    /// certificate is refused, one with a verified certificate goes on.
    #[test]
    fn corbabridge_7_2_without_gssup_or_certificate_csiv2_refuses() {
        let dir = std::env::temp_dir().join(format!("corba-csiv2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pw = dir.join("gssup.passwd");
        std::fs::write(
            &pw,
            zerodds_corba_csiv2::gssup_store::format_entry("alice", "secret", b"s", 1000),
        )
        .unwrap();
        let (d, _lease) = fixture(&format!(
            "corba:\n  csiv2:\n    enabled: true\n    gssup:\n      password_file: \"{}\"\nmappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n",
            pw.display()
        ));
        let sink = Sink::default();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        on_request(
            &d,
            &sink.writer(),
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &["CN=client".to_string()],
        )
        .unwrap();
        let r = replies(&sink);
        assert_eq!(r.len(), 1);
        assert_eq!(exception(&r[0]), "NO_PERMISSION");
        assert_eq!(
            d.in_flight.pending.lock().unwrap().len(),
            1,
            "the one with a certificate waits"
        );
        // §8.2: csiv2_failures_total{reason="missing"}
        assert_eq!(d.spec.as_ref().unwrap().csiv2_failures[0].get(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §6 `TRANSIENT_LOCAL`: the reply of a request is kept, and the same
    /// request again is answered from the store, without a DDS sample.
    #[test]
    fn corbabridge_6_a_repeated_request_is_answered_from_the_replay_store() {
        let (d, _lease) = fixture(
            "mappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n    qos:\n      durability: transient_local\n",
        );
        let sink = Sink::default();
        let conn = sink.writer();
        on_request(
            &d,
            &conn,
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        let id = *d.in_flight.pending.lock().unwrap().keys().next().unwrap();
        let mut reply = id.to_le_bytes().to_vec();
        reply.extend_from_slice(&[0; 8]);
        reply.extend_from_slice(&[42, 0, 0, 0]);
        on_reply(&d, 0, &reply, false, [0, 1]);
        let published = d.spec.as_ref().unwrap().ops[0].published.get();
        on_request(
            &d,
            &conn,
            "p",
            Version::V1_2,
            Endianness::Little,
            request(&d, "add"),
            &[],
        )
        .unwrap();
        let r = replies(&sink);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].reply_status, ReplyStatusType::NoException);
        assert_eq!(r[1].body, [42, 0, 0, 0]);
        assert!(
            d.in_flight.pending.lock().unwrap().is_empty(),
            "no new request"
        );
        assert_eq!(
            d.spec.as_ref().unwrap().ops[0].published.get(),
            published,
            "no new sample"
        );
        let mut other = request(&d, "add");
        other.body = vec![2, 0, 0, 0];
        on_request(
            &d,
            &conn,
            "p",
            Version::V1_2,
            Endianness::Little,
            other,
            &[],
        )
        .unwrap();
        assert_eq!(
            d.in_flight.pending.lock().unwrap().len(),
            1,
            "another body goes to DDS"
        );
    }
}
