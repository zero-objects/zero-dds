// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
//! CORBA codegen runtime: connects the generated stubs/skeletons
//! (`zerodds-corba-rust`) to the IIOP/GIOP transport.
//!
//! * [`IiopCorbaConnection`] — client side, implements the [`CorbaConnection`]
//!   trait referenced by the stub over a TCP connection pool.
//! * [`CorbaServer`] — server side, an `object_key → dispatch` registry with an
//!   accept loop that calls the generated `dispatch_<iface>(servant, op, body,
//!   endianness)` and maps the [`SkeletonResult`] to a GIOP reply.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_csiv2::gssup::GssupCredentialToken;
use zerodds_corba_csiv2::sas::{EstablishContext, IdentityToken, SasMessage};
use zerodds_corba_giop::{
    CodeSetContext, LocateReply, LocateStatusType, Message, Reply, ReplyStatusType, Request,
    ResponseFlags, ServiceContext, ServiceContextList, ServiceContextTag, TargetAddress, Version,
};
use zerodds_corba_iiop::{
    Acceptor, AcceptorConfig, BiDirIiopListenPoint, BiDirIiopServiceContext, Connection, Connector,
    ConnectorConfig, IIOP_BI_DIR_TAG, IiopError, IiopProfileBody, IiopVersion, TaggedComponent,
};
use zerodds_corba_ior::{Ior, ProfileId, TaggedProfile, from_stringified, to_stringified};
use zerodds_corba_rust::{CorbaConnection, CorbaException, ObjectReference, SkeletonResult};

use rustls::{ClientConfig, ServerConfig};

/// `TAG_SSL_SEC_TRANS` (OMG Security/SSLIOP). Carries the SSL port +
/// AssociationOptions in the IOR; a caller selects the TLS path when it finds
/// this component in the target IOR.
const TAG_SSL_SEC_TRANS: u32 = 20;

/// Builds the `TAG_CODE_SETS` component (§13.10.2.4) for the IOR: advertises the
/// transmission codesets supported by the server. Strict ORBs (omniORB) refuse
/// to send `wstring`/`wchar` to an IOR without a wchar codeset (`INV_OBJREF`) —
/// this component enables codeset negotiation.
///
/// `component_data` is a CDR encapsulation with standard alignment from the
/// byte-order octet (the first `native_code_set` `u32` lands at offset 4), as
/// omniORB/TAO expect it.
fn code_sets_component() -> TaggedComponent {
    const TAG_CODE_SETS: u32 = 1;
    const ISO_8859_1: u32 = 0x0001_0001;
    const UTF_8: u32 = 0x0501_0001;
    const UTF_16: u32 = 0x0001_0109;
    const UCS_2: u32 = 0x0001_0100;
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u8(0).expect("bo octet"); // big-endian
    // CodeSetComponentInfo: for_char_data (native UTF-8, conv ISO-8859-1) …
    w.write_u32(UTF_8).expect("char native");
    w.write_u32(1).expect("char conv count");
    w.write_u32(ISO_8859_1).expect("char conv");
    // … for_wchar_data (native UTF-16, conv UCS-2).
    w.write_u32(UTF_16).expect("wchar native");
    w.write_u32(1).expect("wchar conv count");
    w.write_u32(UCS_2).expect("wchar conv");
    TaggedComponent {
        tag: TAG_CODE_SETS,
        component_data: w.into_bytes(),
    }
}

/// Builds an [`ObjectReference`] (IIOP profile encapsulation, GIOP 1.2) for a
/// locally bound server — host/port/object-key.
#[must_use]
pub fn object_reference(
    type_id: &str,
    host: &str,
    port: u16,
    object_key: &[u8],
) -> ObjectReference {
    let mut profile = IiopProfileBody::new(
        IiopVersion::V1_2,
        host.to_string(),
        port,
        object_key.to_vec(),
    );
    profile.components.push(code_sets_component());
    let iiop_profile = profile
        .encode_encapsulation(Endianness::Big)
        .expect("encode iiop profile");
    ObjectReference {
        type_id: type_id.to_string(),
        iiop_profile,
    }
}

/// Serializes a local server endpoint as a stringified IOR (`IOR:<hex>`, CORBA
/// §13.6.10) — the portable reference that a foreign ORB (omniORB/TAO/JacORB)
/// parses to call ZeroDDS objects.
#[must_use]
pub fn stringify_object_ref(type_id: &str, host: &str, port: u16, object_key: &[u8]) -> String {
    let mut body = IiopProfileBody::new(
        IiopVersion::V1_2,
        host.to_string(),
        port,
        object_key.to_vec(),
    );
    body.components.push(code_sets_component());
    let profile = TaggedProfile::iiop(&body, Endianness::Big).expect("encode iiop profile");
    let ior = Ior::new(type_id.to_string(), vec![profile]);
    to_stringified(&ior, Endianness::Big).expect("stringify ior")
}

/// Builds the `TAG_SSL_SEC_TRANS` component (OMG SSLIOP): announces the SSL port
/// and the AssociationOptions. Encapsulation layout as expected by omniORB/TAO —
/// byte-order octet, then three 2-aligned `unsigned short`
/// (target_supports / target_requires / port). Built with a SINGLE
/// `BufferWriter` (octet + auto-alignment), not via
/// `StructuredComponent::encode_encapsulation`, whose separate writer wrongly
/// places the u16 at offset 1 instead of 2 (self-consistent, but not
/// wire-correct).
fn ssl_component(ssl_port: u16) -> TaggedComponent {
    // Security::AssociationOptions: Integrity=2, Confidentiality=4,
    // EstablishTrustInTarget=0x20. The server REQUIRES Integrity+Confidentiality
    // → the client must speak TLS (no cleartext fallback).
    const SUPPORTS: u16 = 0x0002 | 0x0004 | 0x0020;
    const REQUIRES: u16 = 0x0002 | 0x0004;
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u8(0).expect("bo octet"); // big-endian
    w.write_u16(SUPPORTS).expect("target_supports"); // auto-align → Offset 2
    w.write_u16(REQUIRES).expect("target_requires"); // Offset 4
    w.write_u16(ssl_port).expect("ssl port"); // Offset 6
    TaggedComponent {
        tag: TAG_SSL_SEC_TRANS,
        component_data: w.into_bytes(),
    }
}

/// Reads the SSL port from a `TAG_SSL_SEC_TRANS` component encapsulation
/// (byte-order octet + 2-aligned supports/requires/port). `None` on a malformed
/// layout.
fn ssl_port_of(component_data: &[u8]) -> Option<u16> {
    let endianness = match component_data.first()? {
        0 => Endianness::Big,
        1 => Endianness::Little,
        _ => return None,
    };
    let mut r = BufferReader::new(component_data, endianness);
    r.read_u8().ok()?; // byte-order octet
    r.read_u16().ok()?; // target_supports (offset 2)
    r.read_u16().ok()?; // target_requires (offset 4)
    r.read_u16().ok() // port (offset 6)
}

/// Serializes a local **SSLIOP** endpoint as a stringified IOR: the IIOP
/// ProfileBody carries host + port 0 (no cleartext endpoint), the
/// `TAG_SSL_SEC_TRANS` component the real TLS port. A foreign ORB (omniORB) as
/// well as the ZeroDDS client use it to select the TLS transport.
#[must_use]
pub fn stringify_object_ref_ssl(
    type_id: &str,
    host: &str,
    ssl_port: u16,
    object_key: &[u8],
) -> String {
    let mut body =
        IiopProfileBody::new(IiopVersion::V1_2, host.to_string(), 0, object_key.to_vec());
    body.components.push(code_sets_component());
    body.components.push(ssl_component(ssl_port));
    let profile = TaggedProfile::iiop(&body, Endianness::Big).expect("encode iiop profile");
    let ior = Ior::new(type_id.to_string(), vec![profile]);
    to_stringified(&ior, Endianness::Big).expect("stringify ior")
}

/// `TAG_ZERODDS_UDS_TRANS` — ZeroDDS vendor component (UIOP): carries the
/// Unix-domain-socket path for same-host IPC. Lives in the vendor-specific tag
/// space (ASCII `"ZDUD"`); other ORBs ignore it. See vendor spec
/// `docs/specs/zerodds-uiop-transport-1.0.md`.
const TAG_ZERODDS_UDS_TRANS: u32 = 0x5A44_5544;

/// Builds the `TAG_ZERODDS_UDS_TRANS` component: byte-order octet + CDR string
/// (socket path).
fn uds_component(path: &str) -> TaggedComponent {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u8(0).expect("bo octet"); // big-endian
    w.write_string(path).expect("uds path");
    TaggedComponent {
        tag: TAG_ZERODDS_UDS_TRANS,
        component_data: w.into_bytes(),
    }
}

/// Reads the socket path from a `TAG_ZERODDS_UDS_TRANS` component.
/// `None` on a malformed layout.
fn uds_path_of(component_data: &[u8]) -> Option<String> {
    let endianness = match component_data.first()? {
        0 => Endianness::Big,
        1 => Endianness::Little,
        _ => return None,
    };
    let mut r = BufferReader::new(component_data, endianness);
    r.read_u8().ok()?; // byte-order octet
    r.read_string().ok()
}

/// Serializes a local **UIOP** endpoint (Unix domain socket) as a stringified
/// IOR: IIOP ProfileBody with host `localhost`/port 0 (no TCP endpoint) +
/// `TAG_ZERODDS_UDS_TRANS` with the socket path. The ZeroDDS client uses it to
/// select the UDS transport.
#[must_use]
pub fn stringify_object_ref_uds(type_id: &str, socket_path: &str, object_key: &[u8]) -> String {
    let mut body = IiopProfileBody::new(
        IiopVersion::V1_2,
        "localhost".to_string(),
        0,
        object_key.to_vec(),
    );
    body.components.push(code_sets_component());
    body.components.push(uds_component(socket_path));
    let profile = TaggedProfile::iiop(&body, Endianness::Big).expect("encode iiop profile");
    let ior = Ior::new(type_id.to_string(), vec![profile]);
    to_stringified(&ior, Endianness::Big).expect("stringify ior")
}

/// Builds the CSIv2 `SAS::EstablishContext` ServiceContext (id 15, §24.2.6.2)
/// with a GSSUP username/password token. Stateless (`client_context_id` 0),
/// `IdentityToken::Absent` — the pure authentication path.
fn csiv2_establish_context(
    username: &str,
    password: &str,
    endianness: Endianness,
) -> Result<ServiceContext, CorbaException> {
    // GSS-InitialContextToken wrap (0x60 + mech OID + GSSUP encapsulation) — the
    // cross-ORB-correct form of the client_authentication_token (CSIv2 §24.7.1).
    let gssup = GssupCredentialToken::new(username.to_string(), password.to_string(), Vec::new())
        .to_gss_token(endianness)
        .map_err(|_| comm_failure())?;
    let sas = SasMessage::EstablishContext(EstablishContext {
        client_context_id: 0,
        authorization_token: Vec::new(),
        identity_token: IdentityToken::Absent,
        client_authentication_token: gssup,
    })
    .encode_encapsulation(endianness)
    .map_err(|_| comm_failure())?;
    Ok(ServiceContext::new(
        ServiceContextTag::SecurityAttributeService.as_u32(),
        sas,
    ))
}

/// Server-side CSIv2 validation: extracts the SAS `EstablishContext` from the
/// ServiceContext list (id 15), decodes the GSSUP token and calls the validator.
/// `Ok(true)` = authenticated, `Ok(false)` = rejected (`NO_PERMISSION`), `Err` =
/// no/invalid security context.
fn csiv2_authenticate(ctxs: &ServiceContextList, validate: &dyn Fn(&str, &str) -> bool) -> bool {
    let sas_id = ServiceContextTag::SecurityAttributeService.as_u32();
    let Some(sc) = ctxs.0.iter().find(|c| c.context_id == sas_id) else {
        return false;
    };
    let Ok(SasMessage::EstablishContext(ec)) = SasMessage::decode_encapsulation(&sc.context_data)
    else {
        return false;
    };
    // Unwrap the GSS-InitialContextToken (0x60 + OID + GSSUP encapsulation).
    let Ok(token) = GssupCredentialToken::from_gss_token(&ec.client_authentication_token) else {
        return false;
    };
    validate(&token.username, &token.password)
}

/// Parses a stringified IOR (from any ORB) into an [`ObjectReference`] that the
/// generated stub can call. Takes the first `TAG_INTERNET_IOP` profile
/// (host/port/object-key, GIOP version).
///
/// # Errors
/// No IIOP profile, invalid IOR format, or CDR decode error.
pub fn object_reference_from_ior(ior_str: &str) -> Result<ObjectReference, CorbaException> {
    let ior = from_stringified(ior_str).map_err(|_| comm_failure())?;
    let body = ior
        .profiles
        .iter()
        .find_map(TaggedProfile::as_iiop)
        .ok_or_else(comm_failure)?
        .map_err(|_| comm_failure())?;
    // Re-encode as an encapsulation for the ObjectReference (GIOP version from
    // the foreign IOR is preserved).
    let iiop_profile = body
        .encode_encapsulation(Endianness::Big)
        .map_err(|_| comm_failure())?;
    Ok(ObjectReference {
        type_id: ior.type_id,
        iiop_profile,
    })
}

/// Serializes an [`ObjectReference`] (type_id + IIOP profile encapsulation) to a
/// portable stringified IOR — the inverse of [`object_reference_from_ior`].
/// Needed to persist object references, e.g. in a NameService.
///
/// # Errors
/// CDR encoding error.
pub fn stringify_object_reference(obj: &ObjectReference) -> Result<String, CorbaException> {
    let profile = TaggedProfile {
        tag: ProfileId::InternetIop,
        profile_data: obj.iiop_profile.clone(),
    };
    let ior = Ior::new(obj.type_id.clone(), vec![profile]);
    to_stringified(&ior, Endianness::Big).map_err(|_| comm_failure())
}

fn comm_failure() -> CorbaException {
    CorbaException::SystemException {
        minor: 0,
        message: "CORBA COMM_FAILURE",
    }
}

// ---- Client ----------------------------------------------------------------

/// IIOP backend for the [`CorbaConnection`] trait: sends GIOP requests over a
/// TCP [`Connector`] pool and returns the reply body + its byte order.
pub struct IiopCorbaConnection {
    connector: Connector,
    next_request_id: AtomicU32,
    // Optional SSLIOP client: if a target IOR carries TAG_SSL_SEC_TRANS and this
    // config is set, the request runs over TLS (connect_tls) instead of the
    // plain TCP pool. `sni` must match the SAN/CN of the server cert.
    client_tls: Option<Arc<ClientConfig>>,
    sni: String,
    // Optional CSIv2 GSSUP credentials (username, password): if set, a
    // SAS-EstablishContext ServiceContext (id 15) is attached to every request.
    csiv2: Option<(String, String)>,
}

impl Default for IiopCorbaConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl IiopCorbaConnection {
    /// Creates a new IIOP client connection with a fresh connection pool and a
    /// request-ID counter starting at 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connector: Connector::new(ConnectorConfig::default()),
            next_request_id: AtomicU32::new(1),
            client_tls: None,
            sni: String::new(),
            csiv2: None,
        }
    }

    /// Like [`Self::new`], but enables the SSLIOP client: target IORs with
    /// `TAG_SSL_SEC_TRANS` are called over TLS. `ca_pem` contains the server
    /// cert(s) trusted as root (self-signed test cert = own root), `sni` the
    /// expected server name (e.g. `"localhost"`).
    ///
    /// # Errors
    /// PEM parse / rustls config error.
    pub fn with_client_tls(ca_pem: &[u8], sni: &str) -> Result<Self, CorbaException> {
        let cfg = zerodds_corba_iiop::tls::load_client_config_trusting(ca_pem)
            .map_err(|_| comm_failure())?;
        Ok(Self {
            connector: Connector::new(ConnectorConfig::default()),
            next_request_id: AtomicU32::new(1),
            client_tls: Some(cfg),
            sni: sni.to_string(),
            csiv2: None,
        })
    }

    /// Builder: attaches CSIv2 GSSUP credentials to every request as a
    /// SAS-`EstablishContext` ServiceContext (id 15) (CSIv2 §10/§24). Commonly
    /// combined with [`Self::with_client_tls`] (all ORBs couple GSSUP to TLS).
    #[must_use]
    pub fn with_csiv2_credentials(mut self, username: &str, password: &str) -> Self {
        self.csiv2 = Some((username.to_string(), password.to_string()));
        self
    }

    /// Builds the ServiceContext list for a request: codeset (always) +
    /// optionally the SAS-EstablishContext (GSSUP) when CSIv2 credentials are set.
    fn service_contexts(
        &self,
        endianness: Endianness,
    ) -> Result<Vec<ServiceContext>, CorbaException> {
        let mut ctxs = vec![
            CodeSetContext::default_pair()
                .to_service_context(endianness)
                .map_err(|_| comm_failure())?,
        ];
        if let Some((user, pass)) = &self.csiv2 {
            ctxs.push(csiv2_establish_context(user, pass, endianness)?);
        }
        Ok(ctxs)
    }

    /// SSLIOP path: request→reply over a **pooled** TLS connection (connector
    /// pool, keyed by address+SNI+config) — no handshake per call, the
    /// established connection is reused across calls. On a wire error the
    /// connection is invalidated instead of returned to the pool.
    /// Precondition: `self.client_tls` is `Some` (checked by the caller).
    fn send_tls(
        &self,
        profile: &IiopProfileBody,
        ssl_port: u16,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        payload: &[u8],
    ) -> Result<Option<(Vec<u8>, Endianness)>, CorbaException> {
        let cfg = Arc::clone(self.client_tls.as_ref().ok_or_else(comm_failure)?);
        let mut pooled = self
            .connector
            .connect_tls(&profile.host, ssl_port, &self.sni, cfg)
            .map_err(|_| comm_failure())?;
        let service_context = ServiceContextList(self.service_contexts(endianness)?);
        let request = Message::Request(Request {
            request_id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            response_flags: flags,
            target: TargetAddress::Key(profile.object_key.clone()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context,
            body: payload.to_vec(),
        });
        let conn = pooled.connection().ok_or_else(comm_failure)?;
        if conn
            .write_message(giop_request_version(profile), endianness, false, &request)
            .is_err()
        {
            pooled.invalidate();
            return Err(comm_failure());
        }
        if !flags.response_expected() {
            return Ok(None);
        }
        match conn.read_message_with_endianness() {
            Ok((reply_msg, reply_e)) => match reply_msg {
                Message::Reply(r) => match r.reply_status {
                    ReplyStatusType::NoException => Ok(Some((r.body, reply_e))),
                    ReplyStatusType::UserException => Err(decode_user_exception(&r.body, reply_e)),
                    _ => Err(decode_system_exception(&r.body, reply_e)),
                },
                _ => Err(comm_failure()),
            },
            Err(_) => {
                pooled.invalidate();
                Err(comm_failure())
            }
        }
    }

    /// UIOP path: request→reply over a **pooled** Unix-domain-socket connection
    /// (same-host IPC). Mirrors `send_tls`, just with `connect_uds` instead of
    /// TLS.
    #[cfg(unix)]
    fn send_uds(
        &self,
        socket_path: &str,
        profile: &IiopProfileBody,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        payload: &[u8],
    ) -> Result<Option<(Vec<u8>, Endianness)>, CorbaException> {
        let mut pooled = self
            .connector
            .connect_uds(std::path::Path::new(socket_path))
            .map_err(|_| comm_failure())?;
        let service_context = ServiceContextList(self.service_contexts(endianness)?);
        let request = Message::Request(Request {
            request_id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            response_flags: flags,
            target: TargetAddress::Key(profile.object_key.clone()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context,
            body: payload.to_vec(),
        });
        let conn = pooled.connection().ok_or_else(comm_failure)?;
        if conn
            .write_message(giop_request_version(profile), endianness, false, &request)
            .is_err()
        {
            pooled.invalidate();
            return Err(comm_failure());
        }
        if !flags.response_expected() {
            return Ok(None);
        }
        match conn.read_message_with_endianness() {
            Ok((reply_msg, reply_e)) => match reply_msg {
                Message::Reply(r) => match r.reply_status {
                    ReplyStatusType::NoException => Ok(Some((r.body, reply_e))),
                    ReplyStatusType::UserException => Err(decode_user_exception(&r.body, reply_e)),
                    _ => Err(decode_system_exception(&r.body, reply_e)),
                },
                _ => Err(comm_failure()),
            },
            Err(_) => {
                pooled.invalidate();
                Err(comm_failure())
            }
        }
    }

    fn send(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        payload: &[u8],
    ) -> Result<Option<(Vec<u8>, Endianness)>, CorbaException> {
        let profile = IiopProfileBody::decode_encapsulation(&target_ior.iiop_profile)
            .map_err(|_| comm_failure())?;
        // UIOP: if the target IOR carries TAG_ZERODDS_UDS_TRANS, the request runs
        // over the Unix domain socket (same-host IPC, no TCP/IP).
        #[cfg(unix)]
        if let Some(uds_path) = profile
            .components
            .iter()
            .find(|c| c.tag == TAG_ZERODDS_UDS_TRANS)
            .and_then(|c| uds_path_of(&c.component_data))
        {
            return self.send_uds(&uds_path, &profile, operation, flags, endianness, payload);
        }
        // SSLIOP: if the target IOR carries TAG_SSL_SEC_TRANS and we have a
        // client TLS config, the request runs over TLS to the advertised SSL port.
        if self.client_tls.is_some() {
            if let Some(ssl_port) = profile
                .components
                .iter()
                .find(|c| c.tag == TAG_SSL_SEC_TRANS)
                .and_then(|c| ssl_port_of(&c.component_data))
            {
                return self.send_tls(&profile, ssl_port, operation, flags, endianness, payload);
            }
        }
        let mut pooled = self
            .connector
            .connect(&profile.host, profile.port)
            .map_err(|_| comm_failure())?;
        let conn = pooled.connection().ok_or_else(comm_failure)?;
        // Codeset negotiation (§13.10.2.5): the client records the chosen
        // transmission codeset (default UTF-8/UTF-16) as an IOP::CodeSets
        // ServiceContext. omniORB/TAO expect it at connection start in order to
        // be allowed to interpret wchar/wstring data.
        let service_context = ServiceContextList(self.service_contexts(endianness)?);
        let request = Message::Request(Request {
            request_id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            response_flags: flags,
            target: TargetAddress::Key(profile.object_key.clone()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context,
            body: payload.to_vec(),
        });
        conn.write_message(giop_request_version(&profile), endianness, false, &request)
            .map_err(|_| comm_failure())?;
        if !flags.response_expected() {
            return Ok(None);
        }
        let (reply_msg, reply_e) = conn
            .read_message_with_endianness()
            .map_err(|_| comm_failure())?;
        match reply_msg {
            Message::Reply(r) => match r.reply_status {
                ReplyStatusType::NoException => Ok(Some((r.body, reply_e))),
                ReplyStatusType::UserException => Err(decode_user_exception(&r.body, reply_e)),
                _ => Err(decode_system_exception(&r.body, reply_e)),
            },
            _ => Err(comm_failure()),
        }
    }
}

impl CorbaConnection for IiopCorbaConnection {
    fn invoke(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        request_payload: &[u8],
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        self.send(
            target_ior,
            operation,
            ResponseFlags::SYNC_WITH_TARGET,
            request_endianness,
            request_payload,
        )?
        .ok_or_else(comm_failure)
    }

    fn invoke_oneway(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        request_payload: &[u8],
    ) -> Result<(), CorbaException> {
        self.send(
            target_ior,
            operation,
            ResponseFlags::SYNC_NONE,
            request_endianness,
            request_payload,
        )?;
        Ok(())
    }
}

// ---- AMI (CORBA Messaging §22) ---------------------------------------------

/// Reply outcome of an asynchronous invocation: raw reply body + endianness on
/// success, otherwise the decoded CORBA exception (system or user).
pub type AmiReply = Result<(Vec<u8>, Endianness), CorbaException>;

/// Callback for the AMI callback model (§22.5): invoked exactly once when the
/// reply arrives.
pub type AmiCallback = Box<dyn FnOnce(AmiReply) + Send>;

/// Asynchronous CORBA client (CORBA Messaging §22): a held, **multiplexing**
/// GIOP connection to ONE target object. `send`/`send_poll` fire requests
/// without blocking on the reply (response_expected, but no immediate read);
/// [`perform_work`](Self::perform_work) delivers one arrived reply each.
///
/// * **Callback model** (§22.5): [`send`](Self::send) registers a callback;
///   `perform_work` invokes it on the reply.
/// * **Polling model** (§22.6): [`send_poll`](Self::send_poll) returns a
///   `request_id`; [`get_reply`](Self::get_reply) drives the connection until
///   exactly that reply is in, and returns it.
///
/// No background thread and **no busy-poll**: `perform_work` blocks on the
/// socket read (like `ORB::perform_work`). Several requests may be open at
/// once — the mapping runs over the GIOP `request_id`.
pub struct AmiClient {
    conn: Connection,
    object_key: Vec<u8>,
    version: Version,
    next_id: u32,
    /// `request_id` → callback (callback model, in-flight).
    pending: HashMap<u32, AmiCallback>,
    /// Sent, not-yet-answered polling `request_id`s (in-flight, without callback).
    outstanding: std::collections::HashSet<u32>,
    /// `request_id` → finished reply without callback, waiting for `get_reply`.
    ready: HashMap<u32, AmiReply>,
}

impl AmiClient {
    /// Opens a dedicated (unpooled) GIOP connection to the target object over
    /// plain-TCP IIOP.
    ///
    /// # Errors
    /// Profile decode or connect error (`COMM_FAILURE`).
    pub fn connect(target_ior: &ObjectReference) -> Result<Self, CorbaException> {
        let profile = IiopProfileBody::decode_encapsulation(&target_ior.iiop_profile)
            .map_err(|_| comm_failure())?;
        let stream = std::net::TcpStream::connect((profile.host.as_str(), profile.port))
            .map_err(|_| comm_failure())?;
        let conn = Connection::from_stream(stream).map_err(|_| comm_failure())?;
        Ok(Self {
            conn,
            object_key: profile.object_key.clone(),
            version: giop_request_version(&profile),
            next_id: 1,
            pending: HashMap::new(),
            outstanding: std::collections::HashSet::new(),
            ready: HashMap::new(),
        })
    }

    /// Writes out a request (response_expected) without blocking, and returns
    /// the assigned `request_id`.
    fn fire(&mut self, operation: &str, payload: &[u8]) -> Result<u32, CorbaException> {
        let request_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let request = Message::Request(Request {
            request_id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(self.object_key.clone()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context: ServiceContextList(Vec::new()),
            body: payload.to_vec(),
        });
        self.conn
            .write_message(self.version, Endianness::Big, false, &request)
            .map_err(|_| comm_failure())?;
        Ok(request_id)
    }

    /// **Callback model** (§22.5): fires `operation(payload)` and registers `cb`
    /// for the reply. Returns the `request_id`.
    ///
    /// # Errors
    /// Transport error while sending.
    pub fn send(
        &mut self,
        operation: &str,
        payload: &[u8],
        cb: AmiCallback,
    ) -> Result<u32, CorbaException> {
        let id = self.fire(operation, payload)?;
        self.pending.insert(id, cb);
        Ok(id)
    }

    /// **Polling model** (§22.6): fires `operation(payload)` and returns the
    /// `request_id`; the reply is fetched via [`get_reply`](Self::get_reply).
    ///
    /// # Errors
    /// Transport error while sending.
    pub fn send_poll(&mut self, operation: &str, payload: &[u8]) -> Result<u32, CorbaException> {
        let id = self.fire(operation, payload)?;
        self.outstanding.insert(id);
        Ok(id)
    }

    /// Reads EXACTLY one arrived reply (blocking) and delivers it: registered
    /// callback → invocation, otherwise stored for [`get_reply`](Self::get_reply).
    /// Returns the handled `request_id`.
    ///
    /// # Errors
    /// Transport/protocol error (`COMM_FAILURE`), or nothing is open.
    pub fn perform_work(&mut self) -> Result<u32, CorbaException> {
        if self.pending.is_empty() && self.outstanding.is_empty() {
            return Err(comm_failure());
        }
        let (msg, e) = self
            .conn
            .read_message_with_endianness()
            .map_err(|_| comm_failure())?;
        let Message::Reply(r) = msg else {
            return Err(comm_failure());
        };
        let outcome: AmiReply = match r.reply_status {
            ReplyStatusType::NoException => Ok((r.body, e)),
            ReplyStatusType::UserException => Err(decode_user_exception(&r.body, e)),
            _ => Err(decode_system_exception(&r.body, e)),
        };
        if let Some(cb) = self.pending.remove(&r.request_id) {
            cb(outcome);
        } else {
            self.outstanding.remove(&r.request_id);
            self.ready.insert(r.request_id, outcome);
        }
        Ok(r.request_id)
    }

    /// **Polling model**: drives the connection until the reply for `request_id`
    /// (from [`send_poll`](Self::send_poll)) has arrived, and returns it.
    ///
    /// # Errors
    /// Transport error, or `request_id` is no longer open.
    pub fn get_reply(&mut self, request_id: u32) -> Result<AmiReply, CorbaException> {
        loop {
            if let Some(outcome) = self.ready.remove(&request_id) {
                return Ok(outcome);
            }
            if !self.outstanding.contains(&request_id) {
                return Err(comm_failure());
            }
            self.perform_work()?;
        }
    }

    /// Drives `perform_work` until ALL callback requests are processed (§22.5
    /// "flush"). Polling replies (without callback) stay in `ready`.
    ///
    /// # Errors
    /// Transport error.
    pub fn perform_all(&mut self) -> Result<(), CorbaException> {
        while !self.pending.is_empty() {
            self.perform_work()?;
        }
        Ok(())
    }

    /// Number of still-open callback requests.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// Binds [`AmiClient`] to the abstract AMI trait used by codegen, so the
/// generated `sendc_`/`sendp_` stubs (in `zerodds-corba-rust`) can drive it
/// type-independently — same layering as `CorbaConnection` for the synchronous
/// side.
impl zerodds_corba_rust::AsyncCorbaChannel for AmiClient {
    fn send(
        &mut self,
        operation: &str,
        payload: &[u8],
        cb: zerodds_corba_rust::AsyncReplyCallback,
    ) -> Result<u32, CorbaException> {
        AmiClient::send(self, operation, payload, cb)
    }
    fn send_poll(&mut self, operation: &str, payload: &[u8]) -> Result<u32, CorbaException> {
        AmiClient::send_poll(self, operation, payload)
    }
    fn get_reply(&mut self, request_id: u32) -> Result<AmiReply, CorbaException> {
        AmiClient::get_reply(self, request_id)
    }
    fn perform_work(&mut self) -> Result<u32, CorbaException> {
        AmiClient::perform_work(self)
    }
    fn perform_all(&mut self) -> Result<(), CorbaException> {
        AmiClient::perform_all(self)
    }
}

// ---- Bidirectional GIOP (§15.8) --------------------------------------------

/// Extracts the object key from a GIOP `TargetAddress` (§15.4.2): directly for
/// `KeyAddr`, otherwise from the IIOP ProfileBody of the
/// `ProfileAddr`/`ReferenceAddr`.
fn target_object_key(target: &TargetAddress, e: Endianness) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(k) => Some(k.clone()),
        TargetAddress::Profile(bytes) => {
            let mut r = zerodds_cdr::BufferReader::new(bytes, e);
            let tp = zerodds_corba_ior::TaggedProfile::decode(&mut r).ok()?;
            tp.as_iiop()?.ok().map(|b| b.object_key)
        }
        TargetAddress::Reference { ior, .. } => {
            let mut r = zerodds_cdr::BufferReader::new(ior, e);
            let parsed = Ior::decode(&mut r).ok()?;
            parsed
                .profiles
                .iter()
                .find_map(|p| p.as_iiop().and_then(Result::ok).map(|b| b.object_key))
        }
    }
}

/// Builds a `BiDirIIOPServiceContext` as an IOP ServiceContext (tag 5) — a CDR
/// encapsulation (byte-order octet + body), as sent along with the first client
/// request (§15.8).
fn bidir_service_context(sc: &BiDirIiopServiceContext) -> ServiceContext {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u8(0).expect("bo octet"); // big-endian encapsulation
    sc.encode(&mut w).expect("encode BiDir SC");
    ServiceContext {
        context_id: IIOP_BI_DIR_TAG,
        context_data: w.into_bytes(),
    }
}

/// Parses a `BiDirIIOPServiceContext` from a ServiceContext (encapsulation:
/// byte-order octet + body, alignment relative to the octet).
fn parse_bidir(sc: &ServiceContext) -> Option<BiDirIiopServiceContext> {
    let bo = *sc.context_data.first()?;
    let e = if bo == 0 {
        Endianness::Big
    } else {
        Endianness::Little
    };
    let mut r = BufferReader::new(&sc.context_data, e);
    r.read_u8().ok()?; // consume the byte-order octet (position → 1)
    BiDirIiopServiceContext::decode(&mut r).ok()
}

/// **Bidirectional-GIOP endpoint** (§15.8): a peer over ONE connection that
/// simultaneously sends requests (client role) AND serves incoming requests
/// from the other side (server role) — so a server can *call back* over the
/// connection the client opened, without building a new one (NAT/firewall
/// friendly).
///
/// To avoid collisions, §15.8 partitions the `request_id`s by parity: the
/// **originator** (which opened the connection) uses even IDs, the **acceptor**
/// odd ones. The originator advertises its listen points in the
/// `BiDirIIOPServiceContext` (tag 5) of its first request.
pub struct BiDirEndpoint {
    conn: Connection,
    objects: HashMap<Vec<u8>, Dispatcher>,
    next_id: u32,
    version: Version,
    /// Originator: advertised listen points (sent exactly once).
    advertise: Option<BiDirIiopServiceContext>,
    /// Listen points received from the other side.
    peer_listen_points: Vec<BiDirIiopListenPoint>,
    /// Out-of-order arrived replies (request_id → (body, e, status)).
    stash: HashMap<u32, (Vec<u8>, Endianness, ReplyStatusType)>,
}

impl BiDirEndpoint {
    /// Originator side (opened the connection): even `request_id`s, advertises
    /// `listen_points` in the BiDir ServiceContext of the first request.
    #[must_use]
    pub fn originator(conn: Connection, listen_points: Vec<BiDirIiopListenPoint>) -> Self {
        Self {
            conn,
            objects: HashMap::new(),
            next_id: 2,
            version: Version::V1_2,
            advertise: Some(BiDirIiopServiceContext { listen_points }),
            peer_listen_points: Vec::new(),
            stash: HashMap::new(),
        }
    }

    /// Acceptor side (accepted the connection): odd `request_id`s.
    #[must_use]
    pub fn acceptor(conn: Connection) -> Self {
        Self {
            conn,
            objects: HashMap::new(),
            next_id: 1,
            version: Version::V1_2,
            advertise: None,
            peer_listen_points: Vec::new(),
            stash: HashMap::new(),
        }
    }

    /// Registers a local object (`object_key` → dispatcher) that the other side
    /// may call over this connection.
    pub fn register<F>(&mut self, object_key: &[u8], dispatcher: F)
    where
        F: Fn(&str, &[u8], Endianness) -> SkeletonResult + Send + Sync + 'static,
    {
        self.objects
            .insert(object_key.to_vec(), Arc::new(dispatcher));
    }

    /// Listen points received from the other side (after the first dispatched request).
    #[must_use]
    pub fn peer_listen_points(&self) -> &[BiDirIiopListenPoint] {
        &self.peer_listen_points
    }

    /// Sends a request to a peer object over the shared connection (non-blocking)
    /// and returns the `request_id` (parity-correct).
    ///
    /// # Errors
    /// Transport error while sending.
    pub fn invoke_async(
        &mut self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
    ) -> Result<u32, CorbaException> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(2);
        let mut scs = Vec::new();
        if let Some(adv) = self.advertise.take() {
            scs.push(bidir_service_context(&adv));
        }
        let request = Message::Request(Request {
            request_id: id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(object_key.to_vec()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context: ServiceContextList(scs),
            body: body.to_vec(),
        });
        self.conn
            .write_message(self.version, Endianness::Big, false, &request)
            .map_err(|_| comm_failure())?;
        Ok(id)
    }

    /// Serves EXACTLY one incoming message: a request is dispatched + answered
    /// locally, a reply is stashed for [`collect_reply`](Self::collect_reply).
    ///
    /// # Errors
    /// Transport error.
    pub fn serve_one(&mut self) -> Result<(), CorbaException> {
        let (msg, e) = self
            .conn
            .read_message_with_endianness()
            .map_err(|_| comm_failure())?;
        match msg {
            Message::Request(req) => self.handle_request(&req, e),
            Message::Reply(r) => {
                self.stash.insert(r.request_id, (r.body, e, r.reply_status));
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Drives the connection until the reply for `request_id` arrives; incoming
    /// requests from the other side are served reentrantly along the way (§15.8).
    ///
    /// # Errors
    /// CORBA exception (system/user) or transport error.
    pub fn collect_reply(
        &mut self,
        request_id: u32,
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        loop {
            if let Some((body, e, status)) = self.stash.remove(&request_id) {
                return Self::reply_outcome(body, e, status);
            }
            let (msg, e) = self
                .conn
                .read_message_with_endianness()
                .map_err(|_| comm_failure())?;
            match msg {
                Message::Reply(r) if r.request_id == request_id => {
                    return Self::reply_outcome(r.body, e, r.reply_status);
                }
                Message::Reply(r) => {
                    self.stash.insert(r.request_id, (r.body, e, r.reply_status));
                }
                Message::Request(req) => self.handle_request(&req, e)?,
                _ => {}
            }
        }
    }

    /// Synchronous call of a peer object: send + fetch reply (reentrant).
    ///
    /// # Errors
    /// CORBA exception or transport error.
    pub fn invoke(
        &mut self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        let id = self.invoke_async(object_key, operation, body)?;
        self.collect_reply(id)
    }

    /// Dispatches an incoming request locally + sends the reply back.
    fn handle_request(&mut self, req: &Request, e: Endianness) -> Result<(), CorbaException> {
        // §15.8: remember the other side's listen points from the BiDir ServiceContext.
        for sc in &req.service_context.0 {
            if sc.context_id == IIOP_BI_DIR_TAG {
                if let Some(bd) = parse_bidir(sc) {
                    self.peer_listen_points = bd.listen_points;
                }
            }
        }
        // §15.4.2: GIOP 1.2 callbacks address via KeyAddr/ProfileAddr/
        // ReferenceAddr — extract the object key from each variant.
        let key = match target_object_key(&req.target, e) {
            Some(k) => k,
            None => return Ok(()),
        };
        let dispatcher = self.objects.get(&key).cloned();
        let result = match dispatcher {
            Some(d) => d(&req.operation, &req.body, e),
            None => SkeletonResult::Exception(CorbaException::SystemException {
                minor: 0,
                message: "CORBA OBJECT_NOT_EXIST",
            }),
        };
        if req.response_flags.response_expected() {
            let reply = build_reply(req.request_id, result, e);
            self.conn
                .write_message(self.version, e, false, &reply)
                .map_err(|_| comm_failure())?;
        }
        Ok(())
    }

    fn reply_outcome(
        body: Vec<u8>,
        e: Endianness,
        status: ReplyStatusType,
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        match status {
            ReplyStatusType::NoException => Ok((body, e)),
            ReplyStatusType::UserException => Err(decode_user_exception(&body, e)),
            _ => Err(decode_system_exception(&body, e)),
        }
    }
}

// ---- AMH: server-side Asynchronous Method Handling -------------------------

/// Handle for **deferred** sending of a request's reply (AMH, CORBA Messaging
/// §22.9). The servant receives it instead of answering synchronously and sends
/// the reply once the (asynchronous) work is done — the server thread stays free
/// meanwhile. Usable exactly once (`send_reply` OR `send_exception`).
pub struct AmhResponseHandler {
    conn: Arc<Mutex<Connection>>,
    request_id: u32,
    endianness: Endianness,
    version: Version,
}

impl AmhResponseHandler {
    /// Sends the normal reply (NoException) with `body`.
    ///
    /// # Errors
    /// Transport error.
    pub fn send_reply(self, body: Vec<u8>) -> Result<(), CorbaException> {
        self.write(SkeletonResult::Reply(body))
    }

    /// Sends an exception reply (system or user exception).
    ///
    /// # Errors
    /// Transport error.
    pub fn send_exception(self, exc: CorbaException) -> Result<(), CorbaException> {
        self.write(SkeletonResult::Exception(exc))
    }

    fn write(self, result: SkeletonResult) -> Result<(), CorbaException> {
        let reply = build_reply(self.request_id, result, self.endianness);
        let mut conn = self.conn.lock().map_err(|_| comm_failure())?;
        conn.write_message(self.version, self.endianness, false, &reply)
            .map_err(|_| comm_failure())
    }
}

/// A request accepted by the [`AmhEndpoint`] together with a deferred [`AmhResponseHandler`].
pub struct AmhRequest {
    /// Operation name.
    pub operation: String,
    /// Request body (CDR of the in-args).
    pub body: Vec<u8>,
    /// On-wire byte order of the request.
    pub endianness: Endianness,
    /// Object key of the target object.
    pub object_key: Vec<u8>,
    /// Handle for the deferred reply.
    pub handler: AmhResponseHandler,
}

/// Server endpoint for **Asynchronous Method Handling** (§22.9): accepts
/// requests and hands them out together with an [`AmhResponseHandler`], WITHOUT
/// replying inline. Several requests may be "parked" at once and answered in any
/// order — the core of AMH.
pub struct AmhEndpoint {
    conn: Arc<Mutex<Connection>>,
    version: Version,
}

impl AmhEndpoint {
    /// New AMH endpoint over an accepted connection.
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            version: Version::V1_2,
        }
    }

    /// Reads EXACTLY one request and returns it together with a deferred handler.
    /// It does NOT reply — the caller (servant) replies later via the handler.
    ///
    /// # Errors
    /// Transport error; `Ok(None)` on a non-request message.
    pub fn accept_request(&self) -> Result<Option<AmhRequest>, CorbaException> {
        let (msg, endianness) = {
            let mut conn = self.conn.lock().map_err(|_| comm_failure())?;
            conn.read_message_with_endianness()
                .map_err(|_| comm_failure())?
        };
        let Message::Request(req) = msg else {
            return Ok(None);
        };
        let object_key = match &req.target {
            TargetAddress::Key(k) => k.clone(),
            _ => Vec::new(),
        };
        Ok(Some(AmhRequest {
            operation: req.operation,
            body: req.body,
            endianness,
            object_key,
            handler: AmhResponseHandler {
                conn: Arc::clone(&self.conn),
                request_id: req.request_id,
                endianness,
                version: self.version,
            },
        }))
    }
}

// ---- Server ----------------------------------------------------------------

/// Dispatch closure: `(operation, request_body, endianness) -> SkeletonResult`.
/// One per registered object — wraps the servant + the generated
/// `dispatch_<iface>` call.
pub type Dispatcher = Arc<dyn Fn(&str, &[u8], Endianness) -> SkeletonResult + Send + Sync>;

/// CSIv2 credential validator: `(username, password) -> accepted?`.
pub type CredentialValidator = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

/// Observer for the **incoming** `ServiceContextList` of each request — invoked
/// before dispatch. Used to inspect propagated contexts (e.g. the OTS
/// `PropagationContext` in service context id=0) without affecting dispatch.
pub type ContextObserver = Arc<dyn Fn(&ServiceContextList) + Send + Sync>;

/// CORBA server runtime: `object_key → Dispatcher` registry + IIOP accept loop.
#[derive(Clone, Default)]
pub struct CorbaServer {
    objects: Arc<Mutex<HashMap<Vec<u8>, Dispatcher>>>,
    /// Optional CSIv2 GSSUP validator: if set, every request MUST carry a valid
    /// SAS-EstablishContext (id 15), otherwise `NO_PERMISSION`.
    validator: Arc<Mutex<Option<CredentialValidator>>>,
    /// Optional observer of each request's incoming `ServiceContextList`
    /// (inspection only; does not affect dispatch).
    context_observer: Arc<Mutex<Option<ContextObserver>>>,
}

impl CorbaServer {
    /// Creates an empty server registry (no registered objects yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a dispatcher under `object_key`. Typically:
    /// `server.register(b"Echo", move |op, body, e| dispatch_echo(&servant, op, body, e))`.
    pub fn register<F>(&self, object_key: &[u8], dispatch: F)
    where
        F: Fn(&str, &[u8], Endianness) -> SkeletonResult + Send + Sync + 'static,
    {
        self.objects
            .lock()
            .unwrap()
            .insert(object_key.to_vec(), Arc::new(dispatch));
    }

    /// Enables **CSIv2 GSSUP authentication** (§10/§24): if set, every request
    /// must carry a valid SAS-EstablishContext with a username/password that
    /// `validate(username, password)` accepts — otherwise the server responds
    /// with `NO_PERMISSION`.
    pub fn require_credentials<F>(&self, validate: F)
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        *self.validator.lock().unwrap() = Some(Arc::new(validate));
    }

    /// Registers an observer invoked with each request's incoming
    /// `ServiceContextList` before dispatch. Inspection only — it cannot reject
    /// a request. Used e.g. to capture a propagated OTS `PropagationContext`
    /// (service context id=0) from a transactional foreign-ORB invocation.
    pub fn on_request_contexts<F>(&self, observe: F)
    where
        F: Fn(&ServiceContextList) + Send + Sync + 'static,
    {
        *self.context_observer.lock().unwrap() = Some(Arc::new(observe));
    }

    /// Starts the IIOP accept loop: reads GIOP requests, dispatches via the
    /// registry, writes the GIOP reply (in the request byte order).
    ///
    /// # Errors
    /// Bind/listener error.
    pub fn serve(&self, bind: SocketAddr) -> Result<Acceptor, IiopError> {
        let objects = Arc::clone(&self.objects);
        let validator = Arc::clone(&self.validator);
        let observer = Arc::clone(&self.context_observer);
        Acceptor::start(AcceptorConfig::new(bind), move |conn| {
            dispatch_connection(&objects, &validator, &observer, conn);
        })
    }

    /// Like [`Self::serve`], but accepts **SSLIOP** connections (every incoming
    /// stream is TLS-wrapped via the rustls `ServerConfig`). The GIOP dispatch is
    /// identical to the plain-IIOP path — only the transport is TLS. The
    /// corresponding IOR is published with [`stringify_object_ref_ssl`].
    ///
    /// # Errors
    /// Bind/listener error.
    pub fn serve_tls(
        &self,
        bind: SocketAddr,
        tls_config: Arc<ServerConfig>,
    ) -> Result<Acceptor, IiopError> {
        let objects = Arc::clone(&self.objects);
        let validator = Arc::clone(&self.validator);
        let observer = Arc::clone(&self.context_observer);
        Acceptor::start_tls(AcceptorConfig::new(bind), tls_config, move |conn| {
            dispatch_connection(&objects, &validator, &observer, conn);
        })
    }

    /// Like [`Self::serve`], but accepts **UIOP** connections on a Unix-domain-
    /// socket `path` (same-host IPC). The corresponding IOR is published with
    /// [`stringify_object_ref_uds`].
    ///
    /// # Errors
    /// Bind/listener error.
    #[cfg(unix)]
    pub fn serve_uds(&self, path: &std::path::Path) -> Result<Acceptor, IiopError> {
        let objects = Arc::clone(&self.objects);
        let validator = Arc::clone(&self.validator);
        let observer = Arc::clone(&self.context_observer);
        // AcceptorConfig.bind is ignored for UDS (only timeouts/poll are used).
        let cfg = AcceptorConfig::new("127.0.0.1:0".parse().expect("placeholder addr"));
        Acceptor::start_uds(path, cfg, move |conn| {
            dispatch_connection(&objects, &validator, &observer, conn);
        })
    }
}

/// Serves one connection: read GIOP requests, dispatch via the registry, write
/// the reply (in request byte order). Identical for plain-IIOP and SSLIOP.
fn dispatch_connection(
    objects: &Arc<Mutex<HashMap<Vec<u8>, Dispatcher>>>,
    validator: &Arc<Mutex<Option<CredentialValidator>>>,
    context_observer: &Arc<Mutex<Option<ContextObserver>>>,
    mut conn: zerodds_corba_iiop::Connection,
) {
    while let Ok((msg, endianness, req_version)) = conn.read_message_full() {
        // §15.4.1: the server answers in the request version (capped to the
        // maximum supported 1.2), never higher — a GIOP 1.0/1.1 client gets a
        // 1.0/1.1 reply, not a hard 1.2.
        let reply_version = cap_giop_version(req_version);
        // LocateRequest: omniORB/TAO probe the object location before the first
        // call (GIOP §15.4.5). Without a LocateReply the foreign client blocks →
        // COMM_FAILURE.
        if let Message::LocateRequest(lr) = &msg {
            let here = match &lr.target {
                TargetAddress::Key(k) => objects.lock().unwrap().contains_key(k),
                _ => false,
            };
            let status = if here {
                LocateStatusType::ObjectHere
            } else {
                LocateStatusType::UnknownObject
            };
            let reply = Message::LocateReply(LocateReply {
                request_id: lr.request_id,
                locate_status: status,
                body: Vec::new(),
            });
            let _ = conn.write_message(reply_version, endianness, false, &reply);
            continue;
        }
        let Message::Request(req) = msg else {
            continue;
        };
        let key = match &req.target {
            TargetAddress::Key(k) => k.clone(),
            _ => continue,
        };
        // Observe the incoming service contexts (e.g. OTS PropagationContext in
        // id=0) before dispatch — inspection only.
        if let Some(obs) = context_observer.lock().unwrap().clone() {
            obs(&req.service_context);
        }
        // CSIv2: if a validator is set, the request MUST carry a valid
        // SAS-EstablishContext (GSSUP) — otherwise NO_PERMISSION (before dispatch).
        let validator_opt = validator.lock().unwrap().clone();
        let authenticated = match &validator_opt {
            Some(v) => csiv2_authenticate(&req.service_context, &**v),
            None => true,
        };
        let result = if authenticated {
            let dispatcher = objects.lock().unwrap().get(&key).cloned();
            match dispatcher {
                Some(d) => d(&req.operation, &req.body, endianness),
                None => SkeletonResult::Exception(CorbaException::SystemException {
                    minor: 0,
                    message: "CORBA OBJECT_NOT_EXIST",
                }),
            }
        } else {
            SkeletonResult::Exception(CorbaException::SystemException {
                minor: 0,
                message: "CORBA NO_PERMISSION",
            })
        };
        // oneway: no reply.
        if !req.response_flags.response_expected() {
            continue;
        }
        let reply = build_reply(req.request_id, result, endianness);
        let _ = conn.write_message(reply_version, endianness, false, &reply);
    }
}

/// Caps a GIOP version to the maximum supported (1.2). Inbound versions >1.2 are
/// rejected by the codec anyway; this safeguards the reply/request choice.
fn cap_giop_version(v: Version) -> Version {
    if v.is_at_least(1, 3) {
        Version::V1_2
    } else {
        v
    }
}

/// GIOP version for an outgoing request: from the IIOP profile version of the
/// target IOR (§15.7.2), capped to the maximum supported 1.2. This way the
/// client speaks GIOP 1.0/1.1 if the target IOR dictates it, instead of always
/// 1.2.
fn giop_request_version(profile: &IiopProfileBody) -> Version {
    cap_giop_version(Version::new(
        profile.iiop_version.major,
        profile.iiop_version.minor,
    ))
}

fn build_reply(request_id: u32, result: SkeletonResult, endianness: Endianness) -> Message {
    let (status, body) = match result {
        SkeletonResult::Reply(b) => (ReplyStatusType::NoException, b),
        SkeletonResult::Exception(CorbaException::SystemException { minor, message: _ }) => (
            ReplyStatusType::SystemException,
            encode_system_exception("IDL:omg.org/CORBA/UNKNOWN:1.0", minor, endianness),
        ),
        // `body` is already the complete exception reply body (repo_id +
        // members, contiguous) in the reply byte order — use it directly as the
        // reply body (no re-encode, otherwise alignment breaks).
        SkeletonResult::Exception(CorbaException::UserException { body, .. }) => {
            (ReplyStatusType::UserException, body)
        }
        SkeletonResult::BadOperation => (
            ReplyStatusType::SystemException,
            encode_system_exception("IDL:omg.org/CORBA/BAD_OPERATION:1.0", 0, endianness),
        ),
        SkeletonResult::NotYetWired => (
            ReplyStatusType::SystemException,
            encode_system_exception("IDL:omg.org/CORBA/NO_IMPLEMENT:1.0", 0, endianness),
        ),
        // SkeletonResult + CorbaException are `#[non_exhaustive]`.
        _ => (
            ReplyStatusType::SystemException,
            encode_system_exception("IDL:omg.org/CORBA/UNKNOWN:1.0", 0, endianness),
        ),
    };
    Message::Reply(Reply {
        request_id,
        reply_status: status,
        service_context: ServiceContextList::default(),
        body,
    })
}

// ---- Exception-Wire (CORBA §15.4.3) ----------------------------------------

fn encode_system_exception(repo_id: &str, minor: u32, endianness: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endianness);
    let _ = w.write_string(repo_id);
    let _ = w.write_u32(minor);
    let _ = w.write_u32(1); // completion_status = COMPLETED_NO
    w.into_bytes()
}

fn decode_system_exception(body: &[u8], endianness: Endianness) -> CorbaException {
    let mut r = BufferReader::new(body, endianness);
    let _repo = r.read_string().unwrap_or_default();
    let minor = r.read_u32().unwrap_or(0);
    let _completion = r.read_u32().unwrap_or(0);
    CorbaException::SystemException {
        minor,
        message: "CORBA remote system exception",
    }
}

fn decode_user_exception(body: &[u8], endianness: Endianness) -> CorbaException {
    // `body` is the complete exception reply body (repo_id + members). The typed
    // stub decoder first reads the repo_id (positions the reader), then the
    // members — the members align relative to the body start, so nothing is
    // extracted here; the whole body is passed through.
    let repository_id = BufferReader::new(body, endianness)
        .read_string()
        .unwrap_or_default();
    CorbaException::UserException {
        repository_id,
        body: body.to_vec(),
        endianness,
    }
}
