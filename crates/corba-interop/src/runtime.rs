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

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use zerodds_corba_ccm::orb_core::Orb;
use zerodds_corba_ccm::portable_interceptor::{
    InterceptorRegistry, IorInfo, Outcome, RequestException, RequestInfo,
};
use zerodds_corba_csiv2::gssup::GssupCredentialToken;
use zerodds_corba_csiv2::sas::{AuthorizationElement, EstablishContext, IdentityToken, SasMessage};
use zerodds_corba_csiv2::{AssertionTrust, SasOutcome, authenticate_with_verifier};
use zerodds_corba_giop::messaging::{
    PolicyValue, TAG_POLICIES, decode_policy_values, encode_policy_values,
    invocation_policies_context, invocation_policies_of,
};
use zerodds_corba_giop::ziop::{
    COMPRESSION_ENABLING_POLICY_ID, COMPRESSOR_ID_LEVEL_LIST_POLICY_ID, PeerZiop, ZiopCodec,
    ZiopPolicies, ZiopSend,
};
use zerodds_corba_giop::{
    AddressingDisposition, CodeSetContext, GiopError, IorAddressingInfo, LocateReply,
    LocateRequest, LocateStatusType, Message, MessageError, Reply, ReplyStatusType, Request,
    ResponseFlags, ServiceContext, ServiceContextList, ServiceContextTag, TargetAddress,
    TargetProfile, Version, body_writer, finish_body, is_valid_for,
};
use zerodds_corba_iiop::{
    Acceptor, AcceptorConfig, BiDirIiopListenPoint, BiDirIiopServiceContext, Connection, Connector,
    ConnectorConfig, IIOP_BI_DIR_TAG, IiopError, IiopProfileBody, IiopVersion, TaggedComponent,
};
use zerodds_corba_ior::{
    ComponentId, Ior, ProfileId, StructuredComponent, TaggedProfile, from_stringified,
    to_stringified,
};
use zerodds_corba_rt::RawProfile;
use zerodds_corba_rust::{
    AlignedBody, CompletionStatus, CorbaConnection, CorbaException, MarshalArgs, ObjectReference,
    SkeletonResult, SystemException, SystemExceptionKind, marshaled_body,
};
use zerodds_wire::{Endianness, Reader, Writer};

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
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0); // big-endian
    // CodeSetComponentInfo: for_char_data (native UTF-8, conv ISO-8859-1) …
    w.write_u32(UTF_8);
    w.write_u32(1);
    w.write_u32(ISO_8859_1);
    // … for_wchar_data (native UTF-16, conv UCS-2).
    w.write_u32(UTF_16);
    w.write_u32(1);
    w.write_u32(UCS_2);
    TaggedComponent {
        tag: TAG_CODE_SETS,
        component_data: w.finish(),
    }
}

/// The IIOP profile version the references of this ORB carry unless a
/// [`CorbaServer`] is told another ([`CorbaServer::set_profile_version`]).
///
/// The server processes GIOP 1.0 to 1.4 ([`IiopVersion::MAX`]); a profile
/// may name any version up to that (Part 2 §9.7.2: the agent "must be able
/// to accept messages complying with the specified version or any previous
/// minor version"). The default is 1.2 because the ORBs a ZeroDDS server
/// meets do not take a higher one, measured on the Linux test host: a
/// JacORB 3.9 client raises `MARSHAL` ("Unknown GIOP minor: 4") when it
/// calls through an IIOP 1.4 profile, an omniORB 4.3.3 client calls through
/// one but raises `INV_OBJREF` when a reply carries a reference with an
/// IIOP 1.4 profile; §9.7.2 asks them to accept any minor version.
pub const DEFAULT_PROFILE_VERSION: IiopVersion = IiopVersion::V1_2;

/// Builds an [`ObjectReference`] (IIOP profile encapsulation,
/// [`DEFAULT_PROFILE_VERSION`]) for a locally bound server —
/// host/port/object-key.
#[must_use]
pub fn object_reference(
    type_id: &str,
    host: &str,
    port: u16,
    object_key: &[u8],
) -> ObjectReference {
    reference_with_components(
        type_id,
        host,
        port,
        object_key,
        DEFAULT_PROFILE_VERSION,
        None,
        &[],
    )
}

/// [`object_reference`] for an object of `orb`: the IOR interceptors its
/// `ORBInitializer`s registered run `establish_components`, and the
/// components they add for every profile or for `TAG_INTERNET_IOP` go
/// into the IIOP profile (CORBA 3.3 Part 1 §16.6.4, §16.6.5).
#[must_use]
pub fn object_reference_for(
    orb: &Orb,
    type_id: &str,
    host: &str,
    port: u16,
    object_key: &[u8],
) -> ObjectReference {
    reference_with_components(
        type_id,
        host,
        port,
        object_key,
        DEFAULT_PROFILE_VERSION,
        Some(orb),
        &[],
    )
}

/// The reference, and the `TAG_POLICIES` component (CORBA 3.3 Part 1
/// §17.4.2) when `policies` is not empty.
fn reference_with_components(
    type_id: &str,
    host: &str,
    port: u16,
    object_key: &[u8],
    version: IiopVersion,
    orb: Option<&Orb>,
    policies: &[PolicyValue],
) -> ObjectReference {
    let mut profile = IiopProfileBody::for_endpoint(version, host, port, object_key.to_vec());
    profile.components.push(code_sets_component());
    if !policies.is_empty() {
        profile.components.push(TaggedComponent {
            tag: TAG_POLICIES,
            component_data: encode_policy_values(policies, Endianness::Big)
                .expect("encode PolicyValueSeq"),
        });
    }
    if let Some(registry) = orb.and_then(Orb::interceptor_registry) {
        // The references of this adapter carry one profile,
        // TAG_INTERNET_IOP; the ORB knows the policy types of its
        // registered factories; the adapter sets no policies.
        let mut info = IorInfo::new(
            vec![ProfileId::InternetIop.as_u32()],
            registry.policy_types(),
            BTreeMap::new(),
        );
        registry.establish_components(&mut info);
        for (tag, component_data) in info.components_for_profile(ProfileId::InternetIop.as_u32()) {
            profile.components.push(TaggedComponent {
                tag,
                component_data,
            });
        }
    }
    let iiop_profile = profile
        .encode_encapsulation(Endianness::Big)
        .expect("encode iiop profile");
    ObjectReference {
        type_id: type_id.to_string(),
        iiop_profile,
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    }
}

/// Serializes a local server endpoint as a stringified IOR (`IOR:<hex>`, CORBA
/// §13.6.10) — the portable reference that a foreign ORB (omniORB/TAO/JacORB)
/// parses to call ZeroDDS objects.
#[must_use]
pub fn stringify_object_ref(type_id: &str, host: &str, port: u16, object_key: &[u8]) -> String {
    let mut body =
        IiopProfileBody::for_endpoint(DEFAULT_PROFILE_VERSION, host, port, object_key.to_vec());
    body.components.push(code_sets_component());
    let profile = TaggedProfile::iiop(&body, Endianness::Big).expect("encode iiop profile");
    let ior = Ior::new(type_id.to_string(), vec![profile]);
    to_stringified(&ior, Endianness::Big).expect("stringify ior")
}

/// Builds the `TAG_SSL_SEC_TRANS` component (OMG SSLIOP): announces the SSL port
/// and the AssociationOptions. Encapsulation layout as expected by omniORB/TAO —
/// byte-order octet, then three 2-aligned `unsigned short`
/// (target_supports / target_requires / port). Built with a SINGLE
/// `Writer` (octet + auto-alignment), not via
/// `StructuredComponent::encode_encapsulation`, whose separate writer wrongly
/// places the u16 at offset 1 instead of 2 (self-consistent, but not
/// wire-correct).
fn ssl_component(ssl_port: u16) -> TaggedComponent {
    // Security::AssociationOptions: Integrity=2, Confidentiality=4,
    // EstablishTrustInTarget=0x20. The server REQUIRES Integrity+Confidentiality
    // → the client must speak TLS (no cleartext fallback).
    const SUPPORTS: u16 = 0x0002 | 0x0004 | 0x0020;
    const REQUIRES: u16 = 0x0002 | 0x0004;
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0); // big-endian
    w.write_u16(SUPPORTS); // auto-align → Offset 2
    w.write_u16(REQUIRES); // Offset 4
    w.write_u16(ssl_port); // Offset 6
    TaggedComponent {
        tag: TAG_SSL_SEC_TRANS,
        component_data: w.finish(),
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
    let mut r = Reader::new(component_data, endianness);
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
        IiopProfileBody::for_endpoint(DEFAULT_PROFILE_VERSION, host, 0, object_key.to_vec());
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
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0); // big-endian
    w.write_string(path);
    TaggedComponent {
        tag: TAG_ZERODDS_UDS_TRANS,
        component_data: w.finish(),
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
    let mut r = Reader::new(component_data, endianness);
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
        DEFAULT_PROFILE_VERSION,
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

/// What a client puts into the CSIv2 `SAS::EstablishContext` of every
/// request (CORBA 3.3 Part 2 §10.2.2): GSSUP credentials, an asserted
/// identity, an authorization token.
#[derive(Default)]
struct Csiv2Client {
    credentials: Option<(String, String)>,
    identity: Option<IdentityToken>,
    authorization: Vec<AuthorizationElement>,
}

impl Csiv2Client {
    fn is_empty(&self) -> bool {
        self.credentials.is_none() && self.identity.is_none() && self.authorization.is_empty()
    }
}

/// Builds the CSIv2 `SAS::EstablishContext` ServiceContext (id 15,
/// CORBA 3.3 Part 2 §10.2.2), stateless (`client_context_id` 0): the GSSUP
/// username/password token when credentials are set (else an empty
/// client authentication token: the transport authenticates the client),
/// the asserted identity (else `ITTAbsent`) and the authorization token
/// (the push model of §10.6.2.1).
fn csiv2_establish_context(
    client: &Csiv2Client,
    endianness: Endianness,
) -> Result<ServiceContext, CorbaException> {
    // GSS-InitialContextToken wrap (0x60 + mech OID + GSSUP encapsulation) — the
    // cross-ORB-correct form of the client_authentication_token (CSIv2 §24.7.1).
    let gssup = match &client.credentials {
        Some((username, password)) => {
            GssupCredentialToken::new(username.clone(), password.clone(), Vec::new())
                .to_gss_token(endianness)
                .map_err(|_| comm_failure())?
        }
        None => Vec::new(),
    };
    let sas = SasMessage::EstablishContext(EstablishContext {
        client_context_id: 0,
        authorization_token: client.authorization.clone(),
        identity_token: client.identity.clone().unwrap_or(IdentityToken::Absent),
        client_authentication_token: gssup,
    })
    .encode_encapsulation(endianness)
    .map_err(|_| comm_failure())?;
    Ok(ServiceContext::new(
        ServiceContextTag::SecurityAttributeService.as_u32(),
        sas,
    ))
}

/// Server-side CSIv2 evaluation (CORBA 3.3 Part 2 §10.3.1.1): the SAS
/// `EstablishContext` of the ServiceContext list (id 15) through
/// [`authenticate_with_verifier`] — the GSSUP token checked by
/// `validate`, an identity token checked against the supported identity
/// types and the trust rules of `assertion` (backward trust evaluation).
/// [`SasOutcome::NoContext`] when the request carries no SAS context.
fn csiv2_evaluate(
    ctxs: &ServiceContextList,
    validate: &dyn Fn(&str, &str) -> bool,
    assertion: &IdentityAssertion,
) -> SasOutcome {
    let sas_id = ServiceContextTag::SecurityAttributeService.as_u32();
    let Some(sc) = ctxs.0.iter().find(|c| c.context_id == sas_id) else {
        return SasOutcome::NoContext;
    };
    authenticate_with_verifier(
        &sc.context_data,
        validate,
        assertion.target_supports,
        &assertion.trust,
    )
}

/// What a [`CorbaServer`] accepts in an identity token (CORBA 3.3 Part 2
/// §10.3.1.1, §10.6.1.2.3): the `IDENTITY_TYPE_*` bits of
/// `zerodds_corba_csiv2` it supports (0 by default: every identity token
/// is refused with `ContextError` invalid mechanism) and the trust rules
/// of the backward trust evaluation.
#[derive(Debug, Clone, Default)]
struct IdentityAssertion {
    target_supports: u32,
    trust: AssertionTrust,
}

/// The invocation principal of the request the current thread
/// dispatches in a [`CorbaServer`] that requires CSIv2 credentials
/// (CORBA 3.3 Part 2 Table 10.4): the GSSUP user, or the asserted
/// identity the user was trusted to assert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedPrincipal {
    /// The GSSUP user name, or the text of the asserted identity
    /// (`zerodds_corba_csiv2::AssertedIdentity::principal_text`).
    pub principal: String,
    /// `true` when `principal` is an asserted identity.
    pub asserted: bool,
}

std::thread_local! {
    static RECEIVED_PRINCIPAL: std::cell::RefCell<Option<ReceivedPrincipal>> =
        const { std::cell::RefCell::new(None) };
}

/// The invocation principal of the request this thread dispatches: set
/// while a dispatcher registered with [`CorbaServer::register`] runs
/// for a request a [`CorbaServer::require_credentials`] server
/// accepted; `None` outside such a dispatch.
#[must_use]
pub fn received_principal() -> Option<ReceivedPrincipal> {
    RECEIVED_PRINCIPAL.with(|p| p.borrow().clone())
}

/// Sets the thread's [`received_principal`] for the lifetime of the
/// guard and restores the previous one afterwards.
struct PrincipalScope(Option<ReceivedPrincipal>);

impl PrincipalScope {
    fn enter(principal: Option<ReceivedPrincipal>) -> Self {
        Self(RECEIVED_PRINCIPAL.with(|p| p.replace(principal)))
    }
}

impl Drop for PrincipalScope {
    fn drop(&mut self) {
        let previous = self.0.take();
        RECEIVED_PRINCIPAL.with(|p| *p.borrow_mut() = previous);
    }
}

/// Parses a stringified IOR (from any ORB) into an [`ObjectReference`] that the
/// generated stub can call. The prefix and the hex digits are read in either
/// case (CORBA 3.3 Part 2 §7.6.9). Every profile of the IOR is kept in its
/// order (§7.6.3, Full IOR conformance); an invocation uses the first IIOP
/// profile and address it can reach.
///
/// # Errors
/// `BAD_PARAM` with the minor code of Part 2 §7.6.10 for text that is not
/// a stringified IOR: 7 without the `IOR:` scheme, 9 for an encoding that
/// does not decode.
pub fn object_reference_from_ior(ior_str: &str) -> Result<ObjectReference, CorbaException> {
    let ior = from_stringified(ior_str).map_err(|e| {
        CorbaException::System(SystemException::omg(
            SystemExceptionKind::BadParam,
            e.bad_param_minor(),
            CompletionStatus::No,
        ))
    })?;
    Ok(ObjectReference::from_profiles(
        ior.type_id,
        ior.profiles
            .into_iter()
            .map(|p| RawProfile {
                tag: p.tag.as_u32(),
                data: p.profile_data,
            })
            .collect(),
    ))
}

/// Serializes an [`ObjectReference`] with every profile it has to a
/// portable stringified IOR — the inverse of [`object_reference_from_ior`].
/// Needed to persist object references, e.g. in a NameService.
///
/// # Errors
/// CDR encoding error.
pub fn stringify_object_reference(obj: &ObjectReference) -> Result<String, CorbaException> {
    let profiles = obj
        .tagged_profiles()
        .into_iter()
        .map(|p| TaggedProfile {
            tag: ProfileId::from_u32(p.tag),
            profile_data: p.data,
        })
        .collect();
    let ior = Ior::new(obj.type_id.clone(), profiles);
    to_stringified(&ior, Endianness::Big).map_err(|_| comm_failure())
}

/// CORBA 3.3 Part 1 §8.5.1 `ORB_init` with this crate's IIOP client
/// installed as the ORB's transport, so that `string_to_object` can
/// resolve a `corbaname:` URL on its naming context. Use
/// [`IiopCorbaConnection::with_orb`] and [`CorbaServer::set_orb`] to run
/// the application's requests through the ORB's interceptors. Arguments and ORB
/// table as [`zerodds_corba_ccm::orb_core::orb_init`].
///
/// # Errors
/// `BAD_PARAM` for an `ORB_init` argument the ORB refuses.
pub fn orb_init(
    args: &mut Vec<String>,
    orb_identifier: &str,
) -> Result<Arc<zerodds_corba_ccm::orb_core::Orb>, SystemException> {
    let orb = zerodds_corba_ccm::orb_core::orb_init(args, orb_identifier)?;
    if !orb.has_connection() {
        // The ORB's own invocations are ORB mediated: they run through its
        // client request interceptors (Part 1 §16.4.1 item 1).
        orb.set_connection(Arc::new(IiopCorbaConnection::new().with_orb(&orb)));
    }
    Ok(orb)
}

/// `COMM_FAILURE` (CORBA 3.3 Part 1 §8.12.3.5): the connection failed
/// while the operation was in progress, so whether it ran is not known.
fn comm_failure() -> CorbaException {
    CorbaException::system(SystemExceptionKind::CommFailure, 0, CompletionStatus::Maybe)
}

/// The [`SkeletonResult`] for an `operation` against an object key
/// nothing here has a registered dispatcher for. CORBA 3.3 Part 2
/// §15.4.2's `_non_existent`/`_not_existent` answers `true` instead of
/// the `OBJECT_NOT_EXIST` every other operation gets — see
/// `zerodds_corba_giop::object_ops::unknown_object_reply`'s own doc.
fn unregistered_key_result(operation: &str, e: Endianness) -> SkeletonResult {
    match zerodds_corba_giop::object_ops::unknown_object_reply(operation, e) {
        Some(body) => SkeletonResult::Reply(body),
        None => SkeletonResult::Exception(CorbaException::system(
            SystemExceptionKind::ObjectNotExist,
            0,
            CompletionStatus::No,
        )),
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
    // Optional CSIv2 context: GSSUP credentials, an asserted identity, an
    // authorization token; when any is set, a SAS-EstablishContext
    // ServiceContext (id 15) is attached to every request.
    csiv2: Csiv2Client,
    // The ORB whose client request interceptors and PICurrent every request
    // runs through (CORBA 3.3 Part 1 §16.4.5 to §16.4.7). Weak: the ORB holds
    // its own connection.
    orb: Option<std::sync::Weak<Orb>>,
    // The `LOCATION_FORWARD_PERM` / `OBJECT_FORWARD_PERM` targets this client
    // recorded, by the reference they replace (Part 2 §9.6).
    forwards: Mutex<HashMap<ForwardKey, ObjectReference>>,
    // The ZIOP policies of this client and its compressors (Part 2 §12).
    ziop: Option<Ziop>,
    // The compressors of a client that has neither its own nor an ORB's
    // and gets ZIOP policies from the overrides of a reference.
    default_codec: std::sync::OnceLock<Arc<dyn ZiopCodec>>,
    // Set once this client sent ZIOP policies with compression enabled: a
    // server keeps them for the connection (Part 2 §12.2), so a later
    // invocation without ZIOP policies declares compression disabled.
    declared_ziop: std::sync::atomic::AtomicBool,
}

/// The ZIOP policies of one side and the compressors it uses (CORBA 3.3
/// Part 2 §12).
#[derive(Clone)]
struct Ziop {
    policies: ZiopPolicies,
    codec: Arc<dyn ZiopCodec>,
}

impl Ziop {
    /// The codec ZIOP messages from the peer are read with: only when
    /// this side enabled compression and so may have declared it.
    fn read_codec(&self) -> Option<Arc<dyn ZiopCodec>> {
        self.policies
            .compression_enabled
            .then(|| Arc::clone(&self.codec))
    }

    /// The `INVOCATION_POLICIES` service context with the client-exposed
    /// ZIOP policies of this side (Part 2 §12.2).
    fn invocation_policies(&self, endianness: Endianness) -> Result<ServiceContext, GiopError> {
        invocation_policies_context(&self.policies.client_exposed(endianness)?, endianness)
    }
}

/// The raw reply of one request; `None` for a oneway.
/// A reply, its byte order and the offset of its body in the reply
/// message (CORBA 3.3 Part 2 §9.4.3), or none for a oneway request.
type RawReply = Option<(Reply, Endianness, usize)>;

/// A reference as the key of the forward table: type id and profiles.
type ForwardKey = (String, Vec<RawProfile>);

fn forward_key(r: &ObjectReference) -> ForwardKey {
    (r.type_id.clone(), r.tagged_profiles())
}

/// The IIOP profile of a reference an invocation goes over (Part 2
/// §7.6.3, §9.4.2.1): its index in the IOR, its body and its
/// encapsulation.
struct SelectedProfile {
    index: u32,
    body: IiopProfileBody,
    data: Vec<u8>,
}

/// The `TAG_INTERNET_IOP` profiles of `reference` this client can read,
/// in the order of the IOR.
///
/// # Errors
/// `IMP_LIMIT` minor 1 (Part 2 §7.6.3: "unable to use any profile", a
/// non-transient reason) when the reference has none.
fn iiop_profiles_of(reference: &ObjectReference) -> Result<Vec<SelectedProfile>, CorbaException> {
    let found: Vec<SelectedProfile> = reference
        .tagged_profiles()
        .into_iter()
        .enumerate()
        .filter(|(_, p)| p.tag == ProfileId::InternetIop.as_u32())
        .filter_map(|(i, p)| {
            let body = IiopProfileBody::decode_encapsulation(&p.data).ok()?;
            Some(SelectedProfile {
                index: u32::try_from(i).ok()?,
                body,
                data: p.data,
            })
        })
        .collect();
    if found.is_empty() {
        return Err(CorbaException::System(SystemException::omg(
            SystemExceptionKind::ImpLimit,
            1,
            CompletionStatus::No,
        )));
    }
    Ok(found)
}

/// The addresses of an IIOP profile in the order a client tries them:
/// the host and port of the profile body, then each
/// `TAG_ALTERNATE_IIOP_ADDRESS` component (Part 2 §7.6.6.2). A component
/// whose data does not decode names no address; the profile's own
/// address and the other components stay usable.
fn profile_addresses(body: &IiopProfileBody) -> Vec<(String, u16)> {
    let mut out = vec![(body.host.clone(), body.port)];
    for c in body
        .components
        .iter()
        .filter(|c| c.tag == ComponentId::AlternateIiopAddress.as_u32())
    {
        if let Ok(StructuredComponent::AlternateIiopAddress(a)) =
            StructuredComponent::decode(ComponentId::AlternateIiopAddress, &c.component_data)
        {
            out.push((a.host, a.port));
        }
    }
    out
}

/// `TRANSIENT` minor 2 (Part 2 §7.6.3, Annex A: "No usable profile in
/// IOR"): no address of any profile could be reached.
fn no_usable_profile() -> CorbaException {
    CorbaException::System(SystemException::omg(
        SystemExceptionKind::Transient,
        2,
        CompletionStatus::No,
    ))
}

/// How many `LOCATION_FORWARD`s one invocation follows before it gives up.
const MAX_FORWARDS: usize = 16;

/// How many `NEEDS_ADDRESSING_MODE` replies one target may give one
/// invocation: there are three dispositions, and the first was refused.
const MAX_READDRESSES: usize = 2;

/// The answer of a `LocateRequest` (Part 2 §9.4.5, §9.4.6) after the
/// forwards it led to (§9.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateOutcome {
    /// `OBJECT_HERE`: the agent at this reference (the original one, a
    /// recorded permanent forward, or where `OBJECT_FORWARD` replies led)
    /// takes requests for the object directly.
    Here(ObjectReference),
    /// `UNKNOWN_OBJECT` at the address of the original reference: the
    /// object is deemed non-existent (§9.6).
    Unknown,
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
            csiv2: Csiv2Client::default(),
            orb: None,
            forwards: Mutex::new(HashMap::new()),
            ziop: None,
            default_codec: std::sync::OnceLock::new(),
            declared_ziop: std::sync::atomic::AtomicBool::new(false),
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
            csiv2: Csiv2Client::default(),
            orb: None,
            forwards: Mutex::new(HashMap::new()),
            ziop: None,
            default_codec: std::sync::OnceLock::new(),
            declared_ziop: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Builder: attaches CSIv2 GSSUP credentials to every request as a
    /// SAS-`EstablishContext` ServiceContext (id 15) (CSIv2 §10/§24). Commonly
    /// combined with [`Self::with_client_tls`] (all ORBs couple GSSUP to TLS).
    #[must_use]
    pub fn with_csiv2_credentials(mut self, username: &str, password: &str) -> Self {
        self.csiv2.credentials = Some((username.to_string(), password.to_string()));
        self
    }

    /// Builder: asserts `identity` in the CSIv2 `EstablishContext` of every
    /// request (CORBA 3.3 Part 2 §10.2.5, §10.6.1.2.3), e.g. an
    /// `ITTPrincipalName` with the GSSUP exported name of
    /// `zerodds_corba_csiv2::identity::exported_gssup_name`. The target
    /// decides whether the client authentication identity may assert it.
    #[must_use]
    pub fn with_csiv2_identity_assertion(mut self, identity: IdentityToken) -> Self {
        self.csiv2.identity = Some(identity);
        self
    }

    /// Builder: pushes `token`, an authorization token of
    /// `AuthorizationElement`s (e.g. an X.509 attribute certificate chain),
    /// in the CSIv2 `EstablishContext` of every request (CORBA 3.3 Part 2
    /// §10.2.4, §10.6.2.1).
    #[must_use]
    pub fn with_csiv2_authorization_token(mut self, token: Vec<AuthorizationElement>) -> Self {
        self.csiv2.authorization = token;
        self
    }

    /// Builder: every request runs through the client request
    /// interceptors of `orb` (`send_request`, then `receive_reply`,
    /// `receive_exception` or `receive_other`, CORBA 3.3 Part 1 §16.4.7)
    /// with the calling thread's `PortableInterceptor::Current` slots as
    /// its request scope (§16.5.4).
    #[must_use]
    pub fn with_orb(mut self, orb: &Arc<Orb>) -> Self {
        self.orb = Some(Arc::downgrade(orb));
        self
    }

    /// Builder: the ZIOP policies of this client and the compressors it
    /// uses (CORBA 3.3 Part 2 §12), e.g. a
    /// `zerodds_corba_ccm::compression::CompressionManager`. Every request
    /// carries the client-exposed policies (`CompressionEnablingPolicy`,
    /// `CompressorIdLevelListPolicy`) in an `INVOCATION_POLICIES` service
    /// context (§12.2). A request goes out as a ZIOP message only to a
    /// target whose reference declares ZIOP in its `TAG_POLICIES`
    /// component, with the first compressor of `policies` that the target
    /// lists, when the body reaches the low value and the minimum ratio is
    /// met (§12.3). With compression enabled, ZIOP replies are read.
    ///
    /// These are the client's own policies. A ZIOP policy overridden for
    /// an invocation (Part 2 §12.2: "at ORB, thread or reference level";
    /// Part 1 §8.9.1) takes their place: the override of the reference
    /// ([`ObjectReference::policy_overrides`], `Object::set_policy_overrides`),
    /// else of the calling thread (the `PolicyCurrent` of the ORB of
    /// [`Self::with_orb`]), else of that ORB (its `ORBPolicyManager`). A
    /// client without its own policies takes the ORB defaults for the
    /// policies not overridden and the ORB's `CompressionManager`.
    #[must_use]
    pub fn with_ziop(mut self, policies: ZiopPolicies, codec: Arc<dyn ZiopCodec>) -> Self {
        self.ziop = Some(Ziop { policies, codec });
        self
    }

    /// The ZIOP policies and compressors of an invocation on `target`
    /// (Part 2 §12.2): the client's own, with the overrides of the
    /// reference, the calling thread and the ORB over them (see
    /// [`Self::with_ziop`]); `None` without either. A client that declared
    /// compression enabled before declares it disabled instead of nothing:
    /// a server keeps the last declaration for the connection.
    fn effective_ziop(
        &self,
        target: &ObjectReference,
        orb: Option<&Orb>,
    ) -> Result<Option<Ziop>, CorbaException> {
        let base = self.ziop.as_ref();
        let policies = match orb {
            Some(o) => o.client_ziop_policies(target, base.map(|z| &z.policies)),
            None => zerodds_corba_ccm::orb_core::client_ziop_policies(
                target,
                None,
                None,
                base.map(|z| &z.policies),
            ),
        }
        .map_err(CorbaException::System)?;
        let policies = policies.or_else(|| {
            self.declared_ziop
                .load(Ordering::Relaxed)
                .then(|| ZiopPolicies {
                    compression_enabled: false,
                    ..ZiopPolicies::default()
                })
        });
        if policies.as_ref().is_some_and(|p| p.compression_enabled) {
            self.declared_ziop.store(true, Ordering::Relaxed);
        }
        Ok(policies.map(|policies| {
            let codec = match (base, orb) {
                (Some(z), _) => Arc::clone(&z.codec),
                (None, Some(o)) => {
                    let manager: Arc<dyn ZiopCodec> = o.compression_manager();
                    manager
                }
                (None, None) => Arc::clone(self.default_codec.get_or_init(|| {
                    Arc::new(
                        zerodds_corba_ccm::compression::CompressionManager::with_default_factories(
                        ),
                    )
                })),
            };
            Ziop { policies, codec }
        }))
    }

    /// Builds the ServiceContext list this ORB originates for a request
    /// in GIOP `version`, each only where Table 7.2 (CORBA 3.3 Part 2
    /// §7.9) has it in that version: the codeset context (from GIOP 1.1,
    /// first, so that it precedes every context that depends on it,
    /// §7.10.2.6), the SAS `EstablishContext` (GSSUP) when CSIv2
    /// credentials are set (from GIOP 1.2), the `INVOCATION_POLICIES`
    /// of the ZIOP policies (from GIOP 1.2). Without the codeset context
    /// the connection uses the default code sets (§7.10.2.6); without the
    /// policies the request is not compressed.
    ///
    /// # Errors
    /// `NO_PERMISSION` (`COMPLETED_NO`) when CSIv2 credentials are set
    /// and `version` has no `SecurityAttributeService` context: the
    /// request is not sent without the credentials the client was
    /// given.
    fn service_contexts(
        &self,
        endianness: Endianness,
        ziop: Option<&Ziop>,
        version: Version,
    ) -> Result<Vec<ServiceContext>, CorbaException> {
        let mut ctxs = Vec::new();
        if is_valid_for(ServiceContextTag::CodeSets.as_u32(), version) {
            ctxs.push(
                CodeSetContext::default_pair()
                    .to_service_context(endianness)
                    .map_err(|_| comm_failure())?,
            );
        }
        if !self.csiv2.is_empty() {
            if !is_valid_for(
                ServiceContextTag::SecurityAttributeService.as_u32(),
                version,
            ) {
                return Err(CorbaException::system(
                    SystemExceptionKind::NoPermission,
                    0,
                    CompletionStatus::No,
                ));
            }
            ctxs.push(csiv2_establish_context(&self.csiv2, endianness)?);
        }
        if let Some(z) = ziop
            && is_valid_for(ServiceContextTag::InvocationPolicies.as_u32(), version)
        {
            ctxs.push(
                z.invocation_policies(endianness)
                    .map_err(|_| marshal_no())?,
            );
        }
        Ok(ctxs)
    }

    /// How a request to `profile` in `version` is compressed (CORBA 3.3
    /// Part 2 §12.2): `None` without ZIOP policies on this client, below
    /// GIOP 1.2, or when the reference does not declare ZIOP with a
    /// compressor this client has. A `TAG_POLICIES` component that does
    /// not decode makes the reference unusable: `INV_OBJREF`.
    fn ziop_send(
        ziop: Option<&Ziop>,
        profile: &IiopProfileBody,
        version: Version,
    ) -> Result<Option<ZiopSend>, CorbaException> {
        let Some(z) = ziop else {
            return Ok(None);
        };
        if !version.supports_ziop() {
            return Ok(None);
        }
        let mut values = Vec::new();
        for c in profile.components.iter().filter(|c| c.tag == TAG_POLICIES) {
            values.extend(decode_policy_values(&c.component_data).map_err(|_| inv_objref())?);
        }
        let peer = PeerZiop::from_policy_values(&values).map_err(|_| inv_objref())?;
        Ok(z.policies.send_policy(&peer, z.codec.as_ref()))
    }

    /// The `LOCATION_FORWARD_PERM` (or `OBJECT_FORWARD_PERM`) target this
    /// client recorded for `original` (Part 2 §9.6): the new reference is
    /// preferred for future use, the old one stays valid. Invocations on
    /// `original` start at the recorded target and fall back to `original`
    /// when it cannot be reached or no longer knows the object.
    #[must_use]
    pub fn forwarded_reference(&self, original: &ObjectReference) -> Option<ObjectReference> {
        self.forwards
            .lock()
            .ok()
            .and_then(|g| g.get(&forward_key(original)).cloned())
    }

    fn record_forward(&self, original: &ObjectReference, to: &ObjectReference) {
        if let Ok(mut g) = self.forwards.lock() {
            g.insert(forward_key(original), to.clone());
        }
    }

    fn forget_forward(&self, original: &ObjectReference) {
        if let Ok(mut g) = self.forwards.lock() {
            g.remove(&forward_key(original));
        }
    }

    /// A connection to the object of `reference` (Part 2 §7.6.3,
    /// §7.6.6.2): each IIOP profile in the order of the IOR, and in each
    /// profile its own address, then its `TAG_ALTERNATE_IIOP_ADDRESS`
    /// addresses, until one connection opens. Returns the connection and
    /// the profile it belongs to.
    ///
    /// # Errors
    /// `IMP_LIMIT` minor 1 when the reference has no IIOP profile this
    /// client can read; `TRANSIENT` minor 2 when no address could be
    /// reached; the error of `prepare`, which runs on each profile before
    /// a connection to it is opened (a reference whose profile is broken
    /// is refused without a connection attempt).
    fn connect_reference<T>(
        &self,
        reference: &ObjectReference,
        prepare: impl Fn(&IiopProfileBody) -> Result<T, CorbaException>,
    ) -> Result<
        (
            zerodds_corba_iiop::connector::PooledConnection,
            SelectedProfile,
            T,
        ),
        CorbaException,
    > {
        for selected in iiop_profiles_of(reference)? {
            let prepared = prepare(&selected.body)?;
            if let Some(pooled) = self.connection_for(&selected.body) {
                return Ok((pooled, selected, prepared));
            }
        }
        Err(no_usable_profile())
    }

    /// A connection to the agent of `profile`: over UIOP when it carries
    /// `TAG_ZERODDS_UDS_TRANS`, over SSLIOP when it carries
    /// `TAG_SSL_SEC_TRANS` and a client TLS config is set (a pooled TLS
    /// connection to the profile's host and SSL port, no handshake per
    /// call), else over TCP to the profile's address and then its
    /// alternate addresses. `None` when no connection opens.
    fn connection_for(
        &self,
        profile: &IiopProfileBody,
    ) -> Option<zerodds_corba_iiop::connector::PooledConnection> {
        #[cfg(unix)]
        if let Some(uds_path) = profile
            .components
            .iter()
            .find(|c| c.tag == TAG_ZERODDS_UDS_TRANS)
            .and_then(|c| uds_path_of(&c.component_data))
        {
            return self
                .connector
                .connect_uds(std::path::Path::new(&uds_path))
                .ok();
        }
        if let Some(cfg) = &self.client_tls
            && let Some(ssl_port) = profile
                .components
                .iter()
                .find(|c| c.tag == TAG_SSL_SEC_TRANS)
                .and_then(|c| ssl_port_of(&c.component_data))
        {
            return self
                .connector
                .connect_tls(&profile.host, ssl_port, &self.sni, Arc::clone(cfg))
                .ok();
        }
        profile_addresses(profile)
            .into_iter()
            .find_map(|(host, port)| self.connector.connect(&host, port).ok())
    }

    /// One request on the wire and its raw reply. The target is addressed
    /// in `disposition` (Part 2 §9.4.2.1); `arguments` write the body at
    /// its offset in this request ([`request_body`]). A pooled connection that fails
    /// is invalidated. `passed` are the service contexts of a bridged
    /// request, `extra` those interceptors added; both take the place of
    /// a base context with the same id ([`merge_contexts`]). `ziop` are
    /// the ZIOP policies of the invocation.
    #[allow(clippy::too_many_arguments)]
    fn exchange(
        &self,
        target_ior: &ObjectReference,
        disposition: AddressingDisposition,
        request_id: u32,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        arguments: &MarshalArgs<'_>,
        passed: &[ServiceContext],
        extra: &std::collections::BTreeMap<u32, Vec<u8>>,
        ziop: Option<&Ziop>,
    ) -> Result<RawReply, CorbaException> {
        let (pooled, selected, send) = self.connect_reference(target_ior, |profile| {
            Self::ziop_send(ziop, profile, giop_request_version(profile))
        })?;
        let profile = &selected.body;
        let version = giop_request_version(profile);
        // Codeset negotiation (§13.10.2.5): the client records the chosen
        // transmission codeset (default UTF-8/UTF-16) as an IOP::CodeSets
        // ServiceContext. omniORB/TAO expect it at connection start in order
        // to be allowed to interpret wchar/wstring data.
        let contexts = merge_contexts(
            self.service_contexts(endianness, ziop, version)?,
            passed,
            extra,
        );
        let mut request = Request {
            request_id,
            response_flags: flags,
            target: target_address(disposition, target_ior, &selected),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context: ServiceContextList(contexts),
            body: Vec::new(),
        };
        request.body = request_body(&request, version, endianness, arguments)?;
        let request = Message::Request(request);
        let ziop = ziop.map(|z| (z, send));
        roundtrip(pooled, version, endianness, &request, flags, ziop)
    }

    /// An invocation: the client request interceptors of the ORB, the
    /// request, the reply, and what the reply asks for (Part 2 §9.4.3.2,
    /// §9.6): `LOCATION_FORWARD` — from the server or from an
    /// interceptor's `ForwardRequest` (Part 1 §16.4.6) — resends the
    /// request to the new target, at most [`MAX_FORWARDS`] times;
    /// `LOCATION_FORWARD_PERM` also records the new target for later
    /// invocations; `NEEDS_ADDRESSING_MODE` resends it to the same target
    /// in the requested addressing disposition (`receive_other` with
    /// `TRANSPORT_RETRY`, §16.4.12.10). A recorded target that cannot be
    /// reached, or answers `TRANSIENT` or `OBJECT_NOT_EXIST` with
    /// `COMPLETED_NO`, is dropped and the request restarts at the
    /// original reference.
    fn send(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        arguments: &MarshalArgs<'_>,
    ) -> Result<Option<(Vec<u8>, Endianness, usize)>, CorbaException> {
        self.send_passing(
            target_ior,
            operation,
            flags,
            endianness,
            arguments,
            &ServiceContextList::default(),
            &mut ServiceContextList::default(),
        )
    }

    /// A two-way request on behalf of the client of a request-level
    /// bridge (CORBA 3.3 Part 2 §7.7.2: a received service context "must
    /// be passed on through a bridge"): `contexts`, the service contexts
    /// of the received request, go out with it, each in place of a
    /// context with the same id this ORB would send, and with those an
    /// interceptor adds taking precedence; the reply's service contexts
    /// are returned with the outcome, also when it is an exception.
    /// `arguments` write the body; a bridge passes the received body
    /// with [`marshaled_body`] and its offset in the received message.
    /// The reply body comes with its offset in the reply message (Part 2
    /// §9.4.3), so that a bridge writes it on the same offset modulo
    /// eight.
    pub fn invoke_passing_contexts(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        endianness: Endianness,
        arguments: &MarshalArgs<'_>,
        contexts: &ServiceContextList,
    ) -> (
        Result<(AlignedBody, Endianness), CorbaException>,
        ServiceContextList,
    ) {
        let mut reply_contexts = ServiceContextList::default();
        let outcome = self
            .send_passing(
                target_ior,
                operation,
                ResponseFlags::SYNC_WITH_TARGET,
                endianness,
                arguments,
                contexts,
                &mut reply_contexts,
            )
            .and_then(|r| r.ok_or_else(comm_failure))
            .map(|(body, e, offset)| (AlignedBody::new(&body, offset), e));
        (outcome, reply_contexts)
    }

    /// [`Self::send`] with the service contexts `passed` of a bridged
    /// request; `reply_contexts` receives those of the last reply.
    #[allow(clippy::too_many_arguments)]
    fn send_passing(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        flags: ResponseFlags,
        endianness: Endianness,
        arguments: &MarshalArgs<'_>,
        passed: &ServiceContextList,
        reply_contexts: &mut ServiceContextList,
    ) -> Result<Option<(Vec<u8>, Endianness, usize)>, CorbaException> {
        let orb = self.orb.as_ref().and_then(std::sync::Weak::upgrade);
        let registry = orb
            .as_ref()
            .and_then(|o| o.interceptor_registry())
            .filter(|r| r.client_count() > 0);
        // Part 2 §12.2: the ZIOP policies of this invocation, from the
        // overrides of the reference the application called (a forward
        // keeps them), the calling thread and the ORB.
        let ziop = self.effective_ziop(target_ior, orb.as_deref())?;
        let mut route = Route::start(self, target_ior);
        let mut thread_slots = orb.as_ref().map(|o| o.pi_current().thread_slots());
        loop {
            let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
            let mut info = RequestInfo::client(
                request_id,
                operation,
                flags.response_expected(),
                target_ior.clone(),
            );
            info.set_effective_target(route.target.clone());
            info.set_sync_scope(i16::from(flags.0));
            if let Some(slots) = thread_slots.take() {
                info.set_slots(slots.clone());
                thread_slots = Some(slots);
            }
            let depth = match &registry {
                Some(r) => match r.client_start(&mut info, false) {
                    Ok(d) => d,
                    Err(Outcome::Forward(f)) => {
                        route.forward(f, false)?;
                        continue;
                    }
                    Err(Outcome::Exception(e)) => return Err(corba_exception(e)),
                    Err(Outcome::Proceed | Outcome::Other | Outcome::Retry) => {
                        return Err(CorbaException::system(
                            SystemExceptionKind::Internal,
                            0,
                            CompletionStatus::No,
                        ));
                    }
                },
                None => 0,
            };
            let mut body = None;
            let mut notes = ReplyNotes::default();
            let received = match self.exchange(
                &route.target,
                route.disposition,
                request_id,
                operation,
                flags,
                endianness,
                arguments,
                &passed.0,
                info.request_service_contexts(),
                ziop.as_ref(),
            ) {
                Err(CorbaException::System(e)) => Outcome::Exception(RequestException::System(e)),
                Err(other) => return Err(other),
                Ok(None) => Outcome::Other,
                Ok(Some((reply, reply_e, offset))) => {
                    for sc in &reply.service_context.0 {
                        info.set_received_reply_context(sc.context_id, sc.context_data.clone());
                    }
                    reply_contexts.clone_from(&reply.service_context);
                    reply_outcome(reply, reply_e, offset, &mut body, &mut notes)
                }
            };
            let outcome = match &registry {
                Some(r) => r.client_end(&mut info, depth, received),
                None => received,
            };
            match outcome {
                Outcome::Proceed => return body.map(Some).ok_or_else(comm_failure),
                Outcome::Other => return Ok(None),
                Outcome::Retry => route.readdress(notes.readdress)?,
                Outcome::Forward(f) => {
                    let permanent = notes.permanent.as_ref() == Some(&f);
                    route.forward(f, permanent)?;
                }
                Outcome::Exception(RequestException::System(e)) if route.can_restart(&e) => {
                    route.restart();
                }
                Outcome::Exception(e) => return Err(corba_exception(e)),
            }
        }
    }

    /// `LocateRequest` for `target_ior` (Part 2 §9.4.5, §9.6): asks the
    /// agent whether it takes requests for the object, follows
    /// `OBJECT_FORWARD` and `OBJECT_FORWARD_PERM` (the latter recorded
    /// as in [`Self::forwarded_reference`]) at most `MAX_FORWARDS`
    /// times, resends after `LOC_NEEDS_ADDRESSING_MODE` in the requested
    /// disposition, and restarts at the original reference when a
    /// recorded target cannot be reached or answers `UNKNOWN_OBJECT`.
    ///
    /// # Errors
    /// The system exception of a `LOC_SYSTEM_EXCEPTION` reply,
    /// `TRANSIENT` when no connection can be opened or the forwards do not
    /// end, `COMM_FAILURE` on a broken connection, `MARSHAL` for a body
    /// that does not decode.
    pub fn locate(&self, target_ior: &ObjectReference) -> Result<LocateOutcome, CorbaException> {
        let mut route = Route::start(self, target_ior);
        loop {
            let (reply, e) = match self.locate_once(&route.target, route.disposition) {
                Ok(r) => r,
                Err(CorbaException::System(s)) if route.can_restart(&s) => {
                    route.restart();
                    continue;
                }
                Err(other) => return Err(other),
            };
            match reply.locate_status {
                LocateStatusType::ObjectHere => return Ok(LocateOutcome::Here(route.target)),
                LocateStatusType::UnknownObject if route.on_recorded => route.restart(),
                LocateStatusType::UnknownObject => return Ok(LocateOutcome::Unknown),
                LocateStatusType::ObjectForward | LocateStatusType::ObjectForwardPerm => {
                    let to = decode_forward(&reply.body, e)
                        .ok_or_else(|| CorbaException::System(marshal_maybe()))?;
                    let permanent = reply.locate_status == LocateStatusType::ObjectForwardPerm;
                    route.forward(to, permanent)?;
                }
                LocateStatusType::LocSystemException => {
                    match decode_system_exception(&reply.body, e) {
                        CorbaException::System(s) if route.can_restart(&s) => route.restart(),
                        other => return Err(other),
                    }
                }
                LocateStatusType::LocNeedsAddressingMode => {
                    let asked = AddressingDisposition::decode(&mut Reader::new(&reply.body, e));
                    match asked {
                        Ok(d) => route.readdress(Some(d))?,
                        Err(_) => return Err(CorbaException::System(marshal_maybe())),
                    }
                }
            }
        }
    }

    /// One `LocateRequest` and its `LocateReply`.
    fn locate_once(
        &self,
        target: &ObjectReference,
        disposition: AddressingDisposition,
    ) -> Result<(LocateReply, Endianness), CorbaException> {
        let (mut pooled, selected, ()) = self.connect_reference(target, |_| Ok(()))?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request = Message::LocateRequest(LocateRequest {
            request_id,
            target: target_address(disposition, target, &selected),
        });
        let version = giop_request_version(&selected.body);
        let conn = pooled.connection().ok_or_else(comm_failure)?;
        if conn
            .write_message(version, Endianness::Big, false, &request)
            .is_err()
        {
            pooled.invalidate();
            return Err(comm_failure());
        }
        match conn.read_message_with_endianness() {
            Ok((Message::LocateReply(r), e)) if r.request_id == request_id => Ok((r, e)),
            _ => {
                pooled.invalidate();
                Err(comm_failure())
            }
        }
    }
}

/// Where one invocation or locate of a client goes (Part 2 §9.6): the
/// original reference or its recorded permanent forward to begin with,
/// then the forwards the agents answer; the addressing disposition of the
/// next attempt (§9.4.2.1). Every attempt either ends the invocation or
/// counts against a bound.
struct Route<'a> {
    client: &'a IiopCorbaConnection,
    original: &'a ObjectReference,
    target: ObjectReference,
    disposition: AddressingDisposition,
    /// The target is the recorded permanent forward of `original`.
    on_recorded: bool,
    forwards: usize,
    readdresses: usize,
}

impl<'a> Route<'a> {
    fn start(client: &'a IiopCorbaConnection, original: &'a ObjectReference) -> Self {
        let recorded = client.forwarded_reference(original);
        Self {
            client,
            original,
            on_recorded: recorded.is_some(),
            target: recorded.unwrap_or_else(|| original.clone()),
            disposition: AddressingDisposition::KeyAddr,
            forwards: 0,
            readdresses: 0,
        }
    }

    /// The next attempt goes to `to`; a permanent forward is recorded.
    fn forward(&mut self, to: ObjectReference, permanent: bool) -> Result<(), CorbaException> {
        self.forwards += 1;
        if self.forwards > MAX_FORWARDS {
            return Err(unreachable_target());
        }
        if permanent {
            self.client.record_forward(self.original, &to);
        }
        self.target = to;
        self.disposition = AddressingDisposition::KeyAddr;
        self.on_recorded = false;
        self.readdresses = 0;
        Ok(())
    }

    /// The next attempt goes to the same target in the disposition the
    /// agent asked for. An agent that asks for the disposition it just
    /// got, or asks more often than there are dispositions, cannot be
    /// served: `TRANSIENT`, the request was not processed.
    fn readdress(&mut self, asked: Option<AddressingDisposition>) -> Result<(), CorbaException> {
        match asked {
            Some(d) if d != self.disposition && self.readdresses < MAX_READDRESSES => {
                self.readdresses += 1;
                self.disposition = d;
                Ok(())
            }
            Some(_) => Err(unreachable_target()),
            None => Err(CorbaException::system(
                SystemExceptionKind::Internal,
                0,
                CompletionStatus::No,
            )),
        }
    }

    /// Whether `e` from a recorded forward restarts at the original
    /// reference: the recorded agent is gone (`TRANSIENT`, `COMM_FAILURE`)
    /// or no longer has the object (`OBJECT_NOT_EXIST`), and the request
    /// did not run.
    fn can_restart(&self, e: &SystemException) -> bool {
        self.on_recorded
            && e.completed == CompletionStatus::No
            && matches!(
                e.kind,
                SystemExceptionKind::Transient
                    | SystemExceptionKind::CommFailure
                    | SystemExceptionKind::ObjectNotExist
            )
    }

    /// §9.6: "if that fails, it shall restart the location process using
    /// the original address".
    fn restart(&mut self) {
        self.client.forget_forward(self.original);
        self.target = self.original.clone();
        self.disposition = AddressingDisposition::KeyAddr;
        self.on_recorded = false;
        self.readdresses = 0;
    }
}

/// The `TargetAddress` of a request to `reference` over its profile
/// `selected` in `disposition` (Part 2 §9.4.2.1): the object key, the IIOP
/// profile, or the whole IOR with that profile selected.
fn target_address(
    disposition: AddressingDisposition,
    reference: &ObjectReference,
    selected: &SelectedProfile,
) -> TargetAddress {
    match disposition {
        AddressingDisposition::KeyAddr => TargetAddress::Key(selected.body.object_key.clone()),
        AddressingDisposition::ProfileAddr => TargetAddress::Profile(TargetProfile {
            tag: ProfileId::InternetIop.as_u32(),
            profile_data: selected.data.clone(),
        }),
        AddressingDisposition::ReferenceAddr => TargetAddress::Reference(IorAddressingInfo {
            selected_profile_index: selected.index,
            type_id: reference.type_id.clone(),
            profiles: reference
                .tagged_profiles()
                .into_iter()
                .map(|p| TargetProfile {
                    tag: p.tag,
                    profile_data: p.data,
                })
                .collect(),
        }),
    }
}

/// `TRANSIENT`, `COMPLETED_NO` (Part 1 §8.12): the object could not be
/// reached, the request was not sent or not processed.
fn unreachable_target() -> CorbaException {
    CorbaException::system(SystemExceptionKind::Transient, 0, CompletionStatus::No)
}

/// `INV_OBJREF`, `COMPLETED_NO` (Part 1 §8.12): the reference is not
/// usable, the request was not sent.
fn inv_objref() -> CorbaException {
    CorbaException::system(SystemExceptionKind::InvObjref, 0, CompletionStatus::No)
}

/// The body of a received `request` of GIOP `version` with its offset
/// in the message, from which its arguments are aligned (CORBA 3.3 Part
/// 2 §9.4.2.1).
///
/// # Errors
/// `MARSHAL` (`COMPLETED_NO`) when the header does not encode again.
fn received_body(
    request: &Request,
    version: Version,
    endianness: Endianness,
) -> Result<AlignedBody, CorbaException> {
    let offset = request
        .body_offset(version, endianness)
        .map_err(|_| marshal_no())?;
    Ok(AlignedBody::new(&request.body, offset))
}

/// The body of `request` (whose own body is ignored) in a GIOP
/// `version` message: `arguments` write it at its offset in the message
/// (CORBA 3.3 Part 2 §9.4.2.1: a GIOP 1.0/1.1 body follows the header
/// without padding, and its alignment counts from the message start).
///
/// # Errors
/// The error of `arguments`; `MARSHAL` (`COMPLETED_NO`) when the header
/// does not encode.
fn request_body(
    request: &Request,
    version: Version,
    endianness: Endianness,
    arguments: &MarshalArgs<'_>,
) -> Result<Vec<u8>, CorbaException> {
    let offset = request
        .body_offset(version, endianness)
        .map_err(|_| marshal_no())?;
    let mut w = body_writer(offset, endianness);
    arguments(&mut w)?;
    Ok(finish_body(w, offset))
}

/// The service contexts of a request: the ORB's own `base` contexts,
/// in their order, each replaced in place by the contexts with the same
/// id `passed` on from a bridged request, or by the context with that
/// id an interceptor added (`extra`, which takes precedence); then the
/// other `passed` contexts in their order, with every context of an id
/// an interceptor set replaced by that one; then the other contexts of
/// the interceptors. A context keeps its position, so the codeset
/// context stays ahead of the contexts that depend on it (Part 2
/// §7.10.2.6). `passed` is a list: two passed contexts with one id
/// both go out (§7.7.2, a bridge passes received contexts on).
fn merge_contexts(
    base: Vec<ServiceContext>,
    passed: &[ServiceContext],
    extra: &std::collections::BTreeMap<u32, Vec<u8>>,
) -> Vec<ServiceContext> {
    let base_ids: std::collections::BTreeSet<u32> = base.iter().map(|c| c.context_id).collect();
    let mut placed = std::collections::BTreeSet::new();
    let mut contexts = Vec::with_capacity(base.len() + passed.len() + extra.len());
    for c in base {
        let id = c.context_id;
        placed.insert(id);
        if let Some(data) = extra.get(&id) {
            contexts.push(ServiceContext::new(id, data.clone()));
        } else if passed.iter().any(|p| p.context_id == id) {
            contexts.extend(passed.iter().filter(|p| p.context_id == id).cloned());
        } else {
            contexts.push(c);
        }
    }
    for p in passed {
        let id = p.context_id;
        if base_ids.contains(&id) {
            continue;
        }
        match extra.get(&id) {
            Some(data) => {
                if placed.insert(id) {
                    contexts.push(ServiceContext::new(id, data.clone()));
                }
            }
            None => contexts.push(p.clone()),
        }
    }
    contexts.extend(
        extra
            .iter()
            .filter(|(id, _)| !placed.contains(id))
            .map(|(id, data)| ServiceContext::new(*id, data.clone())),
    );
    contexts
}

/// `MARSHAL`, `COMPLETED_NO`: a request that could not be encoded.
fn marshal_no() -> CorbaException {
    CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
}

/// Writes `request` on a pooled connection and reads the reply of a
/// two-way request; a failed write or read invalidates the connection.
fn roundtrip(
    mut pooled: zerodds_corba_iiop::connector::PooledConnection,
    version: Version,
    endianness: Endianness,
    request: &Message,
    flags: ResponseFlags,
    ziop: Option<(&Ziop, Option<ZiopSend>)>,
) -> Result<RawReply, CorbaException> {
    let conn = pooled.connection().ok_or_else(comm_failure)?;
    // Part 2 §12.2: a client that declared ZIOP may get ZIOP replies.
    conn.set_ziop_codec(ziop.and_then(|(z, _)| z.read_codec()));
    let written = match ziop {
        Some((z, Some(send))) => conn
            .write_message_ziop(version, endianness, request, None, &send, z.codec.as_ref())
            .map(|_| ()),
        _ => conn.write_message(version, endianness, false, request),
    };
    if written.is_err() {
        pooled.invalidate();
        return Err(comm_failure());
    }
    if !flags.response_expected() {
        return Ok(None);
    }
    match conn.read_message_full() {
        Ok((Message::Reply(r), reply_e, reply_version)) => {
            // The body's place in the message is the reply's own: its
            // version and its service contexts.
            let offset = r
                .body_offset(reply_version, reply_e)
                .map_err(|_| CorbaException::System(marshal_maybe()))?;
            Ok(Some((r, reply_e, offset)))
        }
        Ok(_) => Err(comm_failure()),
        Err(_) => {
            pooled.invalidate();
            Err(comm_failure())
        }
    }
}

/// What a reply asks of the client besides its outcome.
#[derive(Default)]
struct ReplyNotes {
    /// `LOCATION_FORWARD_PERM` to this reference.
    permanent: Option<ObjectReference>,
    /// `NEEDS_ADDRESSING_MODE` with this disposition.
    readdress: Option<AddressingDisposition>,
}

/// A reply as a client acts on it (Part 2 §9.4.3.2).
enum ClientReply {
    /// `NO_EXCEPTION` with its body, or the exception of the reply.
    Done(AmiReply),
    /// `LOCATION_FORWARD` or `LOCATION_FORWARD_PERM`: resend there.
    Forward {
        target: ObjectReference,
        permanent: bool,
    },
    /// `NEEDS_ADDRESSING_MODE`: resend in this disposition.
    Readdress(AddressingDisposition),
}

/// Classifies a reply whose body stood at `offset` in its message. The
/// body of a `LOCATION_FORWARD(_PERM)` reply is the IOR to send the
/// request to; the body of `NEEDS_ADDRESSING_MODE` a
/// `GIOP::AddressingDisposition`; the body of a `NO_EXCEPTION` or a
/// `USER_EXCEPTION` reply keeps its offset, where the alignment of the
/// result, the out arguments and the exception's members counts from
/// (Part 2 §9.4.3). A body that does not decode is `MARSHAL`,
/// `COMPLETED_MAYBE`.
fn classify_reply(
    status: ReplyStatusType,
    body: Vec<u8>,
    e: Endianness,
    offset: usize,
) -> ClientReply {
    match status {
        ReplyStatusType::NoException => ClientReply::Done(Ok((AlignedBody::new(&body, offset), e))),
        ReplyStatusType::UserException => ClientReply::Done(Err(decode_user_exception(
            AlignedBody::new(&body, offset),
            e,
        ))),
        ReplyStatusType::SystemException => {
            ClientReply::Done(Err(decode_system_exception(&body, e)))
        }
        ReplyStatusType::LocationForward | ReplyStatusType::LocationForwardPerm => {
            match decode_forward(&body, e) {
                Some(target) => ClientReply::Forward {
                    target,
                    permanent: status == ReplyStatusType::LocationForwardPerm,
                },
                None => ClientReply::Done(Err(CorbaException::System(marshal_maybe()))),
            }
        }
        ReplyStatusType::NeedsAddressingMode => {
            match AddressingDisposition::decode(&mut Reader::new(&body, e)) {
                Ok(d) => ClientReply::Readdress(d),
                Err(_) => ClientReply::Done(Err(CorbaException::System(marshal_maybe()))),
            }
        }
    }
}

/// The IOR of a forward reply body, when it has an IIOP profile.
fn decode_forward(body: &[u8], e: Endianness) -> Option<ObjectReference> {
    let mut r = Reader::new(body, e);
    ObjectReference::decode(&mut r)
        .ok()
        .filter(|f| !f.iiop_profile.is_empty())
}

/// The outcome a reply carries for the interceptors; the body of a
/// normal reply goes to `body`, a permanent forward and a requested
/// disposition to `notes`.
fn reply_outcome(
    reply: Reply,
    e: Endianness,
    offset: usize,
    body: &mut Option<(Vec<u8>, Endianness, usize)>,
    notes: &mut ReplyNotes,
) -> Outcome {
    match classify_reply(reply.reply_status, reply.body, e, offset) {
        ClientReply::Done(Ok((b, e))) => {
            *body = Some((b.bytes().to_vec(), e, b.offset()));
            Outcome::Proceed
        }
        ClientReply::Done(Err(CorbaException::UserException {
            repository_id,
            body,
            endianness,
        })) => Outcome::Exception(RequestException::User {
            repository_id,
            body,
            endianness,
        }),
        ClientReply::Done(Err(CorbaException::System(s))) => {
            Outcome::Exception(RequestException::System(s))
        }
        ClientReply::Done(Err(_)) => Outcome::Exception(RequestException::System(marshal_maybe())),
        ClientReply::Forward { target, permanent } => {
            if permanent {
                notes.permanent = Some(target.clone());
            }
            Outcome::Forward(target)
        }
        ClientReply::Readdress(d) => {
            notes.readdress = Some(d);
            Outcome::Retry
        }
    }
}

fn marshal_maybe() -> SystemException {
    SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::Maybe)
}

/// The offset of a received reply's body in its message (Part 2 §9.4.3);
/// a reply whose header does not encode again is `MARSHAL`.
fn reply_body_offset(r: &Reply, version: Version, e: Endianness) -> Result<usize, CorbaException> {
    r.body_offset(version, e)
        .map_err(|_| CorbaException::System(marshal_maybe()))
}

/// The exception of an intercepted request as the stub's exception.
fn corba_exception(e: RequestException) -> CorbaException {
    match e {
        RequestException::System(s) => CorbaException::System(s),
        RequestException::User {
            repository_id,
            body,
            endianness,
        } => CorbaException::UserException {
            repository_id,
            body,
            endianness,
        },
    }
}

impl CorbaConnection for IiopCorbaConnection {
    fn invoke(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        arguments: &MarshalArgs<'_>,
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        self.send(
            target_ior,
            operation,
            ResponseFlags::SYNC_WITH_TARGET,
            request_endianness,
            arguments,
        )?
        .map(|(body, e, _)| (body, e))
        .ok_or_else(comm_failure)
    }

    /// The reply body at its offset in the reply message (CORBA 3.3 Part
    /// 2 §9.4.3): a GIOP 1.0 or 1.1 reply of another ORB may put it on
    /// four modulo eight.
    fn invoke_aligned(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        arguments: &MarshalArgs<'_>,
    ) -> Result<(zerodds_corba_rust::AlignedBody, Endianness), CorbaException> {
        self.send(
            target_ior,
            operation,
            ResponseFlags::SYNC_WITH_TARGET,
            request_endianness,
            arguments,
        )?
        .map(|(body, e, offset)| (zerodds_corba_rust::AlignedBody::new(&body, offset), e))
        .ok_or_else(comm_failure)
    }

    fn invoke_oneway(
        &self,
        target_ior: &ObjectReference,
        operation: &str,
        request_endianness: Endianness,
        arguments: &MarshalArgs<'_>,
    ) -> Result<(), CorbaException> {
        self.send(
            target_ior,
            operation,
            ResponseFlags::SYNC_NONE,
            request_endianness,
            arguments,
        )?;
        Ok(())
    }
}

// ---- AMI (CORBA Messaging §22) ---------------------------------------------

/// Reply outcome of an asynchronous invocation: the reply body at its offset
/// in the reply message and its byte order on success (CORBA 3.3 Part 2
/// §9.4.3: read it through [`AlignedBody::reader`]), otherwise the decoded
/// CORBA exception (system or user).
pub type AmiReply = Result<(AlignedBody, Endianness), CorbaException>;

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
    target: ObjectReference,
    profile: SelectedProfile,
    version: Version,
    next_id: u32,
    /// `request_id` → callback (callback model, in-flight).
    pending: HashMap<u32, AmiCallback>,
    /// Sent, not-yet-answered polling `request_id`s (in-flight, without callback).
    outstanding: std::collections::HashSet<u32>,
    /// `request_id` → finished reply without callback, waiting for `get_reply`.
    ready: HashMap<u32, AmiReply>,
    /// The requests on the wire, by the `request_id` they went out with.
    in_flight: HashMap<u32, AmiRequest>,
    /// The client that resends a request a `LOCATION_FORWARD` moved to
    /// another agent (Part 2 §9.6).
    forwarder: IiopCorbaConnection,
}

/// A request of an [`AmiClient`] that has no reply yet.
struct AmiRequest {
    /// The `request_id` [`AmiClient::send`] / [`AmiClient::send_poll`]
    /// returned; a resend after `NEEDS_ADDRESSING_MODE` keeps it.
    id: u32,
    operation: String,
    /// The body, marshaled once for an offset of 0 and once for an
    /// offset of 4 modulo 8 in the message, or the exception its
    /// arguments raised there ([`AmiRequest::arguments`]).
    bodies: [Result<Vec<u8>, CorbaException>; 2],
    disposition: AddressingDisposition,
    readdresses: usize,
}

impl AmiRequest {
    /// The body `arguments` write, for the two offsets modulo 8 a
    /// request body of this ORB can have: the request goes out again
    /// after the call returned (a `NEEDS_ADDRESSING_MODE` resend, a
    /// `LOCATION_FORWARD` to another target) with another header, and a
    /// GIOP 1.0/1.1 body is aligned from the message start (Part 2
    /// §9.4.2.1). The header of a request ends on a multiple of 4: every
    /// field is aligned to at most 4 and the last one, the empty
    /// `requesting_principal` of GIOP 1.0/1.1, is its length; a GIOP 1.2
    /// body starts on a multiple of 8. Arguments that cannot be written
    /// at one of the two offsets ([`marshaled_body`] of a body written
    /// for the other) keep their exception for that offset; a request
    /// that needs it raises it.
    fn marshal(arguments: &MarshalArgs<'_>) -> [Result<Vec<u8>, CorbaException>; 2] {
        let at = |offset: usize| -> Result<Vec<u8>, CorbaException> {
            let mut w = body_writer(offset, Endianness::Big);
            arguments(&mut w)?;
            Ok(finish_body(w, offset))
        };
        [at(0), at(4)]
    }

    /// The [`MarshalArgs`] that write this request's body for the offset
    /// of the writer they are given: the body marshaled for it, or the
    /// exception its arguments raised there; `MARSHAL` (`COMPLETED_NO`)
    /// for an offset that is not a multiple of 4.
    fn arguments(&self) -> impl Fn(&mut Writer) -> Result<(), CorbaException> + '_ {
        move |w: &mut Writer| {
            let body = match w.position() % 8 {
                0 => &self.bodies[0],
                4 => &self.bodies[1],
                _ => return Err(marshal_no()),
            };
            w.write_bytes(body.as_ref().map_err(Clone::clone)?);
            Ok(())
        }
    }
}

impl AmiClient {
    /// Opens a dedicated (unpooled) GIOP connection to the target object over
    /// plain-TCP IIOP.
    ///
    /// # Errors
    /// Profile decode or connect error (`COMM_FAILURE`).
    pub fn connect(target_ior: &ObjectReference) -> Result<Self, CorbaException> {
        // Part 2 §7.6.3, §7.6.6.2: each IIOP profile, each of its
        // addresses, until one connection opens.
        let (stream, profile) = iiop_profiles_of(target_ior)?
            .into_iter()
            .find_map(|p| {
                profile_addresses(&p.body)
                    .into_iter()
                    .find_map(|(host, port)| {
                        std::net::TcpStream::connect((host.as_str(), port)).ok()
                    })
                    .map(|s| (s, p))
            })
            .ok_or_else(no_usable_profile)?;
        let conn = Connection::from_stream(stream).map_err(|_| comm_failure())?;
        Ok(Self {
            conn,
            target: target_ior.clone(),
            version: giop_request_version(&profile.body),
            profile,
            next_id: 1,
            pending: HashMap::new(),
            outstanding: std::collections::HashSet::new(),
            ready: HashMap::new(),
            in_flight: HashMap::new(),
            forwarder: IiopCorbaConnection::new(),
        })
    }

    /// Writes out a request (response_expected) without blocking, and returns
    /// the assigned `request_id`.
    fn fire(
        &mut self,
        operation: &str,
        arguments: &MarshalArgs<'_>,
    ) -> Result<u32, CorbaException> {
        let request_id = self.next_id;
        let request = AmiRequest {
            id: request_id,
            operation: operation.to_string(),
            bodies: AmiRequest::marshal(arguments),
            disposition: AddressingDisposition::KeyAddr,
            readdresses: 0,
        };
        self.write(request)
    }

    /// Writes `request` with the next `request_id` on the wire and keeps
    /// it until its reply; returns the wire `request_id`.
    fn write(&mut self, request: AmiRequest) -> Result<u32, CorbaException> {
        let wire_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut message = Request {
            request_id: wire_id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: target_address(request.disposition, &self.target, &self.profile),
            operation: request.operation.clone(),
            requesting_principal: None,
            service_context: ServiceContextList(Vec::new()),
            body: Vec::new(),
        };
        message.body = request_body(
            &message,
            self.version,
            Endianness::Big,
            &request.arguments(),
        )?;
        let message = Message::Request(message);
        self.conn
            .write_message(self.version, Endianness::Big, false, &message)
            .map_err(|_| comm_failure())?;
        self.in_flight.insert(wire_id, request);
        Ok(wire_id)
    }

    /// Hands the outcome of request `id` to its callback or keeps it for
    /// [`get_reply`](Self::get_reply).
    fn deliver(&mut self, id: u32, outcome: AmiReply) {
        if let Some(cb) = self.pending.remove(&id) {
            cb(outcome);
        } else {
            self.outstanding.remove(&id);
            self.ready.insert(id, outcome);
        }
    }

    /// **Callback model** (§22.5): fires `operation` with the body
    /// `arguments` write ([`MarshalArgs`]) and registers `cb` for the
    /// reply. Returns the `request_id`.
    ///
    /// # Errors
    /// Transport error while sending; the error of `arguments`.
    pub fn send(
        &mut self,
        operation: &str,
        arguments: &MarshalArgs<'_>,
        cb: AmiCallback,
    ) -> Result<u32, CorbaException> {
        let id = self.fire(operation, arguments)?;
        self.pending.insert(id, cb);
        Ok(id)
    }

    /// **Polling model** (§22.6): fires `operation` with the body
    /// `arguments` write and returns the `request_id`; the reply is
    /// fetched via [`get_reply`](Self::get_reply).
    ///
    /// # Errors
    /// Transport error while sending; the error of `arguments`.
    pub fn send_poll(
        &mut self,
        operation: &str,
        arguments: &MarshalArgs<'_>,
    ) -> Result<u32, CorbaException> {
        let id = self.fire(operation, arguments)?;
        self.outstanding.insert(id);
        Ok(id)
    }

    /// Reads replies (blocking) until EXACTLY one request has its outcome
    /// and delivers it: registered callback → invocation, otherwise stored
    /// for [`get_reply`](Self::get_reply). Returns the handled `request_id`.
    ///
    /// The client acts on a reply as Part 2 §9.4.3.2 and §9.6 require,
    /// transparently to the caller: a `LOCATION_FORWARD(_PERM)` request is
    /// resent to the new target (over a connection of its own, following
    /// further forwards there, see [`IiopCorbaConnection`]) and its reply
    /// is the outcome; a `NEEDS_ADDRESSING_MODE` request is resent on this
    /// connection in the requested disposition and keeps its `request_id`.
    ///
    /// # Errors
    /// Transport/protocol error (`COMM_FAILURE`), a reply to no request
    /// of this client (`COMM_FAILURE`), or nothing is open.
    pub fn perform_work(&mut self) -> Result<u32, CorbaException> {
        loop {
            if self.pending.is_empty() && self.outstanding.is_empty() {
                return Err(comm_failure());
            }
            let (msg, e, version) = self.conn.read_message_full().map_err(|_| comm_failure())?;
            let Message::Reply(r) = msg else {
                return Err(comm_failure());
            };
            let Some(mut request) = self.in_flight.remove(&r.request_id) else {
                return Err(comm_failure());
            };
            let id = request.id;
            let offset = reply_body_offset(&r, version, e)?;
            let outcome = match classify_reply(r.reply_status, r.body, e, offset) {
                ClientReply::Done(outcome) => outcome,
                ClientReply::Forward { target, permanent } => {
                    if permanent {
                        self.forwarder.record_forward(&self.target, &target);
                    }
                    // The reply of the new target at its offset too.
                    self.forwarder.invoke_aligned(
                        &target,
                        &request.operation,
                        Endianness::Big,
                        &request.arguments(),
                    )
                }
                ClientReply::Readdress(d)
                    if d != request.disposition && request.readdresses < MAX_READDRESSES =>
                {
                    request.disposition = d;
                    request.readdresses += 1;
                    match self.write(request) {
                        Ok(_) => continue,
                        Err(err) => Err(err),
                    }
                }
                ClientReply::Readdress(_) => Err(unreachable_target()),
            };
            self.deliver(id, outcome);
            return Ok(id);
        }
    }

    /// The `LOCATION_FORWARD_PERM` target a reply recorded for the
    /// reference this client was opened on (Part 2 §9.6); a new
    /// [`AmiClient::connect`] to it reaches the object directly.
    #[must_use]
    pub fn forwarded_reference(&self) -> Option<ObjectReference> {
        self.forwarder.forwarded_reference(&self.target)
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
        arguments: &MarshalArgs<'_>,
        cb: zerodds_corba_rust::AsyncReplyCallback,
    ) -> Result<u32, CorbaException> {
        AmiClient::send(self, operation, arguments, cb)
    }
    fn send_poll(
        &mut self,
        operation: &str,
        arguments: &MarshalArgs<'_>,
    ) -> Result<u32, CorbaException> {
        AmiClient::send_poll(self, operation, arguments)
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

/// The object key a GIOP `TargetAddress` names (Part 2 §9.4.2.1): the
/// key itself, or the key of the IIOP profile of a `ProfileAddr`, or of
/// the selected profile of a `ReferenceAddr`. `None` for a profile that
/// is not a decodable IIOP profile.
pub(crate) fn target_object_key(target: &TargetAddress) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(k) => Some(k.clone()),
        TargetAddress::Profile(p) => iiop_object_key(p),
        TargetAddress::Reference(info) => info.selected_profile().and_then(iiop_object_key),
    }
}

fn iiop_object_key(p: &TargetProfile) -> Option<Vec<u8>> {
    if p.tag != ProfileId::InternetIop.as_u32() {
        return None;
    }
    IiopProfileBody::decode_encapsulation(&p.profile_data)
        .ok()
        .map(|b| b.object_key)
}

/// The body of a `NEEDS_ADDRESSING_MODE` reply or a
/// `LOC_NEEDS_ADDRESSING_MODE` locate reply asking for `KeyAddr`: a
/// server sends it for a target whose object key it cannot read (Part 2
/// §9.4.3.2, §9.4.6.2).
pub(crate) fn key_addr_body(e: Endianness) -> Vec<u8> {
    let mut w = Writer::new(e);
    AddressingDisposition::KeyAddr.encode(&mut w);
    w.finish()
}

/// The `LocateReply` of a server to `lr` (Part 2 §9.4.6, §9.6):
/// `OBJECT_FORWARD(_PERM)` with the IOR for a forwarded key (`_PERM`
/// only from GIOP 1.2 on), `OBJECT_HERE` for a key `here` says is served,
/// `UNKNOWN_OBJECT` otherwise; `LOC_NEEDS_ADDRESSING_MODE` asking for
/// `KeyAddr` when the target names no readable key.
fn locate_reply(
    lr: &LocateRequest,
    version: Version,
    e: Endianness,
    forwarded: impl Fn(&[u8]) -> Option<(ObjectReference, bool)>,
    here: impl Fn(&[u8]) -> bool,
) -> Message {
    let (locate_status, body) = match target_object_key(&lr.target) {
        None => (LocateStatusType::LocNeedsAddressingMode, key_addr_body(e)),
        Some(key) => match forwarded(&key) {
            Some((to, permanent)) => {
                let mut w = Writer::new(e);
                to.encode(&mut w);
                let status = if permanent && version.uses_v1_2_request_layout() {
                    LocateStatusType::ObjectForwardPerm
                } else {
                    LocateStatusType::ObjectForward
                };
                (status, w.finish())
            }
            None if here(&key) => (LocateStatusType::ObjectHere, Vec::new()),
            None => (LocateStatusType::UnknownObject, Vec::new()),
        },
    };
    Message::LocateReply(LocateReply {
        request_id: lr.request_id,
        locate_status,
        body,
    })
}

/// Builds a `BiDirIIOPServiceContext` as an IOP ServiceContext (tag 5) — a CDR
/// encapsulation (byte-order octet + body), as sent along with the first client
/// request (§15.8).
fn bidir_service_context(sc: &BiDirIiopServiceContext) -> ServiceContext {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0); // big-endian encapsulation
    sc.encode(&mut w).expect("encode BiDir SC");
    ServiceContext {
        context_id: IIOP_BI_DIR_TAG,
        context_data: w.finish(),
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
    let mut r = Reader::new(&sc.context_data, e);
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
    /// Replies read while waiting for another: body, byte order, status
    /// and the body's offset in its message.
    stash: HashMap<u32, (Vec<u8>, Endianness, ReplyStatusType, usize)>,
    /// The requests on the wire, by `request_id`, until their reply.
    sent: HashMap<u32, BiDirRequest>,
    /// The client that resends a request a `LOCATION_FORWARD` moved to
    /// another agent (Part 2 §9.6).
    forwarder: IiopCorbaConnection,
}

/// A request a [`BiDirEndpoint`] sent and has no reply to yet.
struct BiDirRequest {
    object_key: Vec<u8>,
    operation: String,
    body: Vec<u8>,
    disposition: AddressingDisposition,
    readdresses: usize,
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
            sent: HashMap::new(),
            forwarder: IiopCorbaConnection::new(),
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
            sent: HashMap::new(),
            forwarder: IiopCorbaConnection::new(),
        }
    }

    /// Registers a local object (`object_key` → dispatcher) that the other side
    /// may call over this connection.
    pub fn register<F>(&mut self, object_key: &[u8], dispatcher: F)
    where
        F: Fn(&str, &AlignedBody, Endianness) -> SkeletonResult + Send + Sync + 'static,
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
    /// and returns the `request_id` (parity-correct). `body` is the CDR of
    /// the in-args written from offset 0: Bi-Directional GIOP is GIOP 1.2
    /// (Part 2 §9.8, Table 7.2), whose body starts on a multiple of 8.
    ///
    /// # Errors
    /// Transport error while sending.
    pub fn invoke_async(
        &mut self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
    ) -> Result<u32, CorbaException> {
        self.write_request(BiDirRequest {
            object_key: object_key.to_vec(),
            operation: operation.to_string(),
            body: body.to_vec(),
            disposition: AddressingDisposition::KeyAddr,
            readdresses: 0,
        })
    }

    /// Writes `request` with the next parity-correct `request_id` and
    /// keeps it until its reply.
    fn write_request(&mut self, request: BiDirRequest) -> Result<u32, CorbaException> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(2);
        let mut scs = Vec::new();
        if let Some(adv) = self.advertise.take() {
            scs.push(bidir_service_context(&adv));
        }
        let target = match request.disposition {
            AddressingDisposition::KeyAddr => TargetAddress::Key(request.object_key.clone()),
            d => {
                let (host, port) = match self.peer_listen_points.first() {
                    Some(lp) => (lp.host.clone(), lp.port),
                    None => {
                        let peer = self.conn.peer_addr();
                        (peer.ip().to_string(), peer.port())
                    }
                };
                let reference = object_reference("", &host, port, &request.object_key);
                let selected = iiop_profiles_of(&reference)?
                    .into_iter()
                    .next()
                    .ok_or_else(comm_failure)?;
                target_address(d, &reference, &selected)
            }
        };
        let message = Message::Request(Request {
            request_id: id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target,
            operation: request.operation.clone(),
            requesting_principal: None,
            service_context: ServiceContextList(scs),
            body: request.body.clone(),
        });
        self.conn
            .write_message(self.version, Endianness::Big, false, &message)
            .map_err(|_| comm_failure())?;
        self.sent.insert(id, request);
        Ok(id)
    }

    /// Serves EXACTLY one incoming message: a request is dispatched + answered
    /// locally, a reply is stashed for [`collect_reply`](Self::collect_reply).
    ///
    /// # Errors
    /// Transport error.
    pub fn serve_one(&mut self) -> Result<(), CorbaException> {
        let (msg, e, version) = self.conn.read_message_full().map_err(|_| comm_failure())?;
        match msg {
            Message::Request(req) => self.handle_request(&req, e),
            Message::Reply(r) => {
                let offset = reply_body_offset(&r, version, e)?;
                self.stash
                    .insert(r.request_id, (r.body, e, r.reply_status, offset));
                Ok(())
            }
            Message::LocateRequest(lr) => self.handle_locate_request(&lr, e),
            _ => Ok(()),
        }
    }

    /// Drives the connection until the reply for `request_id` arrives; incoming
    /// requests from the other side are served reentrantly along the way (§15.8).
    ///
    /// # Errors
    /// CORBA exception (system/user) or transport error.
    ///
    /// The reply is acted on as Part 2 §9.4.3.2 and §9.6 require: after a
    /// `LOCATION_FORWARD(_PERM)` the request is resent to the new target
    /// over a connection of its own (see [`IiopCorbaConnection`]); after
    /// `NEEDS_ADDRESSING_MODE` it is resent over this connection in the
    /// requested disposition, the profile naming the peer's first listen
    /// point or, without one, the connected peer address.
    ///
    /// The body is the reply's at its offset in the reply message (Part
    /// 2 §9.4.3): read it through [`AlignedBody::reader`].
    pub fn collect_reply(
        &mut self,
        request_id: u32,
    ) -> Result<(AlignedBody, Endianness), CorbaException> {
        let mut request_id = request_id;
        loop {
            let (body, e, status, offset) = match self.stash.remove(&request_id) {
                Some(stashed) => stashed,
                None => {
                    let (msg, e, version) =
                        self.conn.read_message_full().map_err(|_| comm_failure())?;
                    match msg {
                        Message::Reply(r) if r.request_id == request_id => {
                            let offset = reply_body_offset(&r, version, e)?;
                            (r.body, e, r.reply_status, offset)
                        }
                        Message::Reply(r) => {
                            let offset = reply_body_offset(&r, version, e)?;
                            self.stash
                                .insert(r.request_id, (r.body, e, r.reply_status, offset));
                            continue;
                        }
                        Message::Request(req) => {
                            self.handle_request(&req, e)?;
                            continue;
                        }
                        Message::LocateRequest(lr) => {
                            self.handle_locate_request(&lr, e)?;
                            continue;
                        }
                        _ => continue,
                    }
                }
            };
            let request = self.sent.remove(&request_id);
            match (classify_reply(status, body, e, offset), request) {
                (ClientReply::Done(outcome), _) => return outcome,
                (ClientReply::Forward { target, .. }, Some(request)) => {
                    // The body was written for a GIOP 1.2 message, at an
                    // offset that is a multiple of 8.
                    return self.forwarder.invoke_aligned(
                        &target,
                        &request.operation,
                        Endianness::Big,
                        &marshaled_body(&request.body, 0),
                    );
                }
                (ClientReply::Readdress(d), Some(mut request))
                    if d != request.disposition && request.readdresses < MAX_READDRESSES =>
                {
                    request.disposition = d;
                    request.readdresses += 1;
                    request_id = self.write_request(request)?;
                }
                (ClientReply::Readdress(_), Some(_)) => return Err(unreachable_target()),
                // A forward or readdress for a request this endpoint did not send.
                (_, None) => return Err(comm_failure()),
            }
        }
    }

    /// Answers a `LocateRequest` (GIOP §15.4.5) — a `corbaloc:` client, or
    /// any ORB that probes an object's location before its first real
    /// invocation, blocks (→ `COMM_FAILURE`) without a reply. `OBJECT_HERE`
    /// for a registered key, `UNKNOWN_OBJECT` otherwise.
    fn handle_locate_request(
        &mut self,
        lr: &LocateRequest,
        e: Endianness,
    ) -> Result<(), CorbaException> {
        let reply = locate_reply(
            lr,
            self.version,
            e,
            |_| None,
            |k| self.objects.contains_key(k),
        );
        self.conn
            .write_message(self.version, e, false, &reply)
            .map_err(|_| comm_failure())
    }

    /// Synchronous call of a peer object: send + fetch reply (reentrant),
    /// the reply body at its offset as [`Self::collect_reply`] returns it.
    ///
    /// # Errors
    /// CORBA exception or transport error.
    pub fn invoke(
        &mut self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
    ) -> Result<(AlignedBody, Endianness), CorbaException> {
        let id = self.invoke_async(object_key, operation, body)?;
        self.collect_reply(id)
    }

    /// Dispatches an incoming request locally + sends the reply back.
    fn handle_request(&mut self, req: &Request, e: Endianness) -> Result<(), CorbaException> {
        // §15.8: remember the other side's listen points from the BiDir ServiceContext.
        for sc in &req.service_context.0 {
            if sc.context_id == IIOP_BI_DIR_TAG
                && let Some(bd) = parse_bidir(sc)
            {
                self.peer_listen_points = bd.listen_points;
            }
        }
        // §9.4.2.1: GIOP 1.2 callbacks address via KeyAddr/ProfileAddr/
        // ReferenceAddr — extract the object key from each variant; a
        // target without a readable key is asked for KeyAddr.
        let reply = match target_object_key(&req.target) {
            Some(key) => {
                let dispatcher = self.objects.get(&key).cloned();
                let result = match (dispatcher, received_body(req, self.version, e)) {
                    (Some(d), Ok(body)) => d(&req.operation, &body, e),
                    (Some(_), Err(exc)) => SkeletonResult::Exception(exc),
                    (None, _) => unregistered_key_result(&req.operation, e),
                };
                build_reply(req.request_id, result, e)
            }
            None => (needs_key_addr_reply(req.request_id, e), e, 0),
        };
        // A relayed user exception goes out in its own byte order.
        let (mut reply, reply_e, origin) = reply;
        if req.response_flags.response_expected() {
            place_reply_body(&mut reply, self.version, reply_e, origin)
                .map_err(|()| comm_failure())?;
            self.conn
                .write_message(self.version, reply_e, false, &reply)
                .map_err(|_| comm_failure())?;
        }
        Ok(())
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
        let (mut reply, reply_e, origin) = build_reply(self.request_id, result, self.endianness);
        // Part 2 §9.4.3: a GIOP 1.0/1.1 body on the offset it was
        // written for; a user exception in its own byte order.
        place_reply_body(&mut reply, self.version, reply_e, origin).map_err(|()| comm_failure())?;
        let mut conn = self.conn.lock().map_err(|_| comm_failure())?;
        conn.write_message(self.version, reply_e, false, &reply)
            .map_err(|_| comm_failure())
    }
}

/// A request accepted by the [`AmhEndpoint`] together with a deferred [`AmhResponseHandler`].
pub struct AmhRequest {
    /// Operation name.
    pub operation: String,
    /// Request body (CDR of the in-args) with its offset in the message;
    /// [`AlignedBody::reader`] reads the arguments.
    pub body: AlignedBody,
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
    /// Transport error; `Ok(None)` on a non-request message; `MARSHAL`
    /// when the request header does not encode again for its body offset.
    pub fn accept_request(&self) -> Result<Option<AmhRequest>, CorbaException> {
        let (msg, endianness, version) = {
            let mut conn = self.conn.lock().map_err(|_| comm_failure())?;
            conn.read_message_full().map_err(|_| comm_failure())?
        };
        let Message::Request(req) = msg else {
            return Ok(None);
        };
        let body = received_body(&req, version, endianness)?;
        let object_key = match &req.target {
            TargetAddress::Key(k) => k.clone(),
            _ => Vec::new(),
        };
        Ok(Some(AmhRequest {
            operation: req.operation,
            body,
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
/// `dispatch_<iface>` call. The body comes with its offset in the
/// request message: under GIOP 1.0/1.1 it follows the header without
/// padding, and its arguments are aligned from the message start (CORBA
/// 3.3 Part 2 §9.4.2.1), which [`AlignedBody::reader`] reads.
pub type Dispatcher = Arc<dyn Fn(&str, &AlignedBody, Endianness) -> SkeletonResult + Send + Sync>;

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
    /// The identity types and trust rules identity tokens are evaluated
    /// with when `validator` is set.
    identity_assertion: Arc<Mutex<IdentityAssertion>>,
    /// Optional observer of each request's incoming `ServiceContextList`
    /// (inspection only; does not affect dispatch).
    context_observer: Arc<Mutex<Option<ContextObserver>>>,
    /// The ORB whose server request interceptors and
    /// `PortableInterceptor::Current` the requests run through
    /// (CORBA 3.3 Part 1 §16.4.8 to §16.4.10, §16.5).
    orb: Arc<Mutex<Option<Arc<Orb>>>>,
    /// The object keys this server forwards, to the reference and whether
    /// the forward is permanent (Part 2 §9.6).
    forwards: ForwardMap,
    /// The ZIOP policies of this server and its compressors (Part 2 §12).
    ziop: Arc<Mutex<Option<Ziop>>>,
    /// The component reference of an object key: what `_component`
    /// answers for a facet or a component (Part 2 §9.4.2.1 Table 7.2,
    /// Part 3 §6.4.3.1).
    components: ComponentMap,
    /// The version of the profiles of [`Self::object_reference`].
    profile_version: Arc<Mutex<Option<IiopVersion>>>,
}

/// The component reference of each object key that is a facet or a
/// component.
type ComponentMap = Arc<Mutex<HashMap<Vec<u8>, ObjectReference>>>;

/// The object keys a [`CorbaServer`] forwards, to the reference and
/// whether the forward is permanent.
type ForwardMap = Arc<Mutex<HashMap<Vec<u8>, (ObjectReference, bool)>>>;

/// The state a connection loop of a [`CorbaServer`] reads.
#[derive(Clone)]
struct ServerShared {
    objects: Arc<Mutex<HashMap<Vec<u8>, Dispatcher>>>,
    validator: Arc<Mutex<Option<CredentialValidator>>>,
    identity_assertion: Arc<Mutex<IdentityAssertion>>,
    observer: Arc<Mutex<Option<ContextObserver>>>,
    orb: Arc<Mutex<Option<Arc<Orb>>>>,
    forwards: ForwardMap,
    ziop: Arc<Mutex<Option<Ziop>>>,
    components: ComponentMap,
}

impl CorbaServer {
    /// Creates an empty server registry (no registered objects yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a dispatcher under `object_key`. Typically:
    /// `server.register(b"Echo", move |op, body, e| dispatch_echo(&servant, op, body, e))`.
    /// The dispatcher reads the body at its offset ([`Dispatcher`]).
    pub fn register<F>(&self, object_key: &[u8], dispatch: F)
    where
        F: Fn(&str, &AlignedBody, Endianness) -> SkeletonResult + Send + Sync + 'static,
    {
        self.objects
            .lock()
            .unwrap()
            .insert(object_key.to_vec(), Arc::new(dispatch));
    }

    /// Enables **CSIv2 GSSUP authentication** (CORBA 3.3 Part 2 §10): if
    /// set, every request must carry a valid SAS `EstablishContext` with a
    /// username/password that `validate(username, password)` accepts and,
    /// if it asserts an identity, one [`Self::set_identity_assertion`]
    /// allows — otherwise the server responds with `NO_PERMISSION` and a
    /// `ContextError` service context. An accepted request is answered
    /// with a `CompleteEstablishContext` service context, and its
    /// dispatcher sees the invocation principal in
    /// [`received_principal`].
    pub fn require_credentials<F>(&self, validate: F)
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        *self.validator.lock().unwrap() = Some(Arc::new(validate));
    }

    /// Identity assertion for a [`Self::require_credentials`] server
    /// (CORBA 3.3 Part 2 §10.3.1.1, §10.6.1.2.3): `target_supports` are
    /// the `IDENTITY_TYPE_*` bits of `zerodds_corba_csiv2` the server
    /// accepts in an identity token, `trust` the rules of the backward
    /// trust evaluation (a user may always assert itself and the
    /// anonymous identity). Without this call no identity type is
    /// supported and every request with an identity token is refused
    /// (`ContextError` invalid mechanism).
    pub fn set_identity_assertion(&self, target_supports: u32, trust: AssertionTrust) {
        *self.identity_assertion.lock().unwrap() = IdentityAssertion {
            target_supports,
            trust,
        };
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

    /// Runs every request through the server request interceptors of
    /// `orb` (registered by its `ORBInitializer`s) and its
    /// `PortableInterceptor::Current`: `receive_request_service_contexts`
    /// and `receive_request` before the servant, `send_reply`,
    /// `send_exception` or `send_other` after it, with the flow-stack
    /// rules of CORBA 3.3 Part 1 §16.4.10; reply service contexts the
    /// interceptors add go into the reply, a `ForwardRequest` becomes a
    /// `LOCATION_FORWARD` reply.
    pub fn set_orb(&self, orb: Arc<Orb>) {
        *self.orb.lock().unwrap() = Some(orb);
    }

    /// Makes this server a location agent for `object_key` (Part 2 §9.6):
    /// a request for it is answered with `LOCATION_FORWARD` to `to`
    /// (`LOCATION_FORWARD_PERM` when `permanent` and the request is GIOP
    /// 1.2), a `LocateRequest` with `OBJECT_FORWARD` (`OBJECT_FORWARD_PERM`)
    /// and the IOR (§9.4.3.2, §9.4.6.2). The forward takes precedence over
    /// an object registered under the same key and runs before the server
    /// interceptors: the request is not served here.
    pub fn forward(&self, object_key: &[u8], to: ObjectReference, permanent: bool) {
        self.forwards
            .lock()
            .unwrap()
            .insert(object_key.to_vec(), (to, permanent));
    }

    /// Declares the object of `object_key` a facet or a component of the
    /// component `component` (CORBA 3.3 Part 3 §6.4.3.1): the implicit
    /// operation `_component` (`CORBA::Object::get_component`, Part 2
    /// §9.4.2.1, Table 7.2 from GIOP 1.3) on that key answers with
    /// `component`, ahead of the key's dispatcher; for a component itself
    /// `component` is its own reference. A key without a component
    /// reference answers `_component` through its dispatcher, which
    /// returns the nil reference (`object_ops::dispatch`).
    pub fn set_component_reference(&self, object_key: &[u8], component: ObjectReference) {
        self.components
            .lock()
            .unwrap()
            .insert(object_key.to_vec(), component);
    }

    /// The ZIOP policies of the objects of this server and the
    /// compressors it uses (CORBA 3.3 Part 2 §12, the policies a POA is
    /// created with), e.g. a
    /// `zerodds_corba_ccm::compression::CompressionManager`. The
    /// references of [`Self::object_reference`] declare the client-exposed
    /// policies in their `TAG_POLICIES` component (§12.2, §12.4); with
    /// compression enabled the server reads ZIOP requests, and a reply goes
    /// out as a ZIOP message to a client whose `INVOCATION_POLICIES`
    /// service context enables compression with a compressor in common,
    /// when the body reaches the low value and the minimum ratio is met
    /// (§12.3). The policies a client sent hold for its connection until it
    /// sends others: a client may send them in its first request only.
    /// Every reply carries the server's client-exposed policies in an
    /// `INVOCATION_POLICIES` service context.
    ///
    /// A server without its own ZIOP policies that is bound to an ORB
    /// ([`Self::set_orb`]) takes the ZIOP overrides at the ORB scope (the
    /// ORB's `ORBPolicyManager`, Part 1 §8.9.1) over the ORB defaults, with
    /// the ORB's `CompressionManager`, as long as the ORB scope overrides a
    /// ZIOP policy.
    pub fn set_ziop(&self, policies: ZiopPolicies, codec: Arc<dyn ZiopCodec>) {
        *self.ziop.lock().unwrap() = Some(Ziop { policies, codec });
    }

    /// The IIOP version of the profiles [`Self::object_reference`] writes,
    /// 1.1 to 1.4 ([`DEFAULT_PROFILE_VERSION`] when not set): the highest
    /// GIOP minor version clients may send to this server (Part 2 §9.7.2).
    /// The server processes GIOP 1.0 to 1.4 whatever its profiles say.
    ///
    /// # Errors
    /// `BAD_PARAM` for a version below 1.1, whose profile has no place for
    /// the `TAG_CODE_SETS` component every reference carries, or above
    /// [`IiopVersion::MAX`].
    pub fn set_profile_version(&self, version: IiopVersion) -> Result<(), CorbaException> {
        if !version.has_components() || version > IiopVersion::MAX {
            return Err(CorbaException::system(
                SystemExceptionKind::BadParam,
                0,
                CompletionStatus::No,
            ));
        }
        *self.profile_version.lock().unwrap() = Some(version);
        Ok(())
    }

    /// The reference of `object_key` on this server at `host:port` (an
    /// IIOP profile of [`Self::set_profile_version`], 1.4 for an IPv6
    /// `host`): the components the IOR interceptors of the
    /// server's ORB add (Part 1 §16.6.4) and, with [`Self::set_ziop`], the
    /// `TAG_POLICIES` component with the client-exposed ZIOP policies
    /// (Part 1 §17.4.2, Part 2 §12.4).
    #[must_use]
    pub fn object_reference(
        &self,
        type_id: &str,
        host: &str,
        port: u16,
        object_key: &[u8],
    ) -> ObjectReference {
        let orb = self.orb.lock().unwrap().clone();
        let policies = server_ziop(&self.ziop, &self.orb)
            .map(|z| {
                z.policies
                    .client_exposed(Endianness::Big)
                    .expect("encode ZIOP policies")
            })
            .unwrap_or_default();
        let version = self
            .profile_version
            .lock()
            .unwrap()
            .unwrap_or(DEFAULT_PROFILE_VERSION);
        reference_with_components(
            type_id,
            host,
            port,
            object_key,
            version,
            orb.as_deref(),
            &policies,
        )
    }

    /// Ends the forward of `object_key`; `true` when there was one.
    pub fn stop_forwarding(&self, object_key: &[u8]) -> bool {
        self.forwards.lock().unwrap().remove(object_key).is_some()
    }

    fn shared(&self) -> ServerShared {
        ServerShared {
            objects: Arc::clone(&self.objects),
            validator: Arc::clone(&self.validator),
            identity_assertion: Arc::clone(&self.identity_assertion),
            observer: Arc::clone(&self.context_observer),
            orb: Arc::clone(&self.orb),
            forwards: Arc::clone(&self.forwards),
            ziop: Arc::clone(&self.ziop),
            components: Arc::clone(&self.components),
        }
    }

    /// Starts the IIOP accept loop: reads GIOP requests, dispatches via the
    /// registry, writes the GIOP reply (in the request byte order).
    ///
    /// # Errors
    /// Bind/listener error.
    pub fn serve(&self, bind: SocketAddr) -> Result<Acceptor, IiopError> {
        let shared = self.shared();
        Acceptor::start(AcceptorConfig::new(bind), move |conn| {
            dispatch_connection(&shared, conn);
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
        let shared = self.shared();
        Acceptor::start_tls(AcceptorConfig::new(bind), tls_config, move |conn| {
            dispatch_connection(&shared, conn);
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
        let shared = self.shared();
        // AcceptorConfig.bind is ignored for UDS (only timeouts/poll are used).
        let cfg = AcceptorConfig::new("127.0.0.1:0".parse().expect("placeholder addr"));
        Acceptor::start_uds(path, cfg, move |conn| {
            dispatch_connection(&shared, conn);
        })
    }
}

/// Serves one connection: read GIOP requests, dispatch via the registry, write
/// the reply (in request byte order). Identical for plain-IIOP and SSLIOP.
fn dispatch_connection(shared: &ServerShared, mut conn: zerodds_corba_iiop::Connection) {
    let objects = &shared.objects;
    // The client-exposed ZIOP policies the client of this connection sent
    // last (Part 2 §12.2).
    let mut peer_ziop = PeerZiop::default();
    loop {
        // Part 2 §12.2: ZIOP requests are read when this server declared
        // ZIOP in its references.
        let ziop = server_ziop(&shared.ziop, &shared.orb);
        conn.set_ziop_codec(ziop.as_ref().and_then(Ziop::read_codec));
        let (msg, endianness, req_version) = match conn.read_message_full() {
            Ok(m) => m,
            Err(e) => {
                let _ = refuse_message(&mut conn, &e);
                break;
            }
        };
        // Part 2 §9.4.1, §9.7: the server answers in the request version
        // (1.0 to 1.4), never higher — a GIOP 1.0/1.1 client gets a 1.0/1.1
        // reply.
        let reply_version = cap_giop_version(req_version);
        // LocateRequest: omniORB/TAO probe the object location before the first
        // call (GIOP §15.4.5). Without a LocateReply the foreign client blocks →
        // COMM_FAILURE.
        if let Message::LocateRequest(lr) = &msg {
            let reply = locate_reply(
                lr,
                reply_version,
                endianness,
                |k| shared.forwards.lock().unwrap().get(k).cloned(),
                |k| objects.lock().unwrap().contains_key(k),
            );
            let _ = conn.write_message(reply_version, endianness, false, &reply);
            continue;
        }
        let req = match msg {
            Message::Request(req) => req,
            other => {
                if server_other_message(&mut conn, &other) {
                    continue;
                }
                break;
            }
        };
        // Part 2 §9.4.2.1: the key of a KeyAddr, ProfileAddr or
        // ReferenceAddr target; a target without a readable key is asked
        // for KeyAddr (§9.4.3.2), the request is not processed.
        let Some(key) = target_object_key(&req.target) else {
            if req.response_flags.response_expected() {
                let reply = needs_key_addr_reply(req.request_id, endianness);
                let _ = conn.write_message(reply_version, endianness, false, &reply);
            }
            continue;
        };
        // §9.6: a key this server forwards is not served here.
        let forwarded = shared.forwards.lock().unwrap().get(&key).cloned();
        if let Some((to, permanent)) = forwarded {
            if req.response_flags.response_expected() {
                let reply =
                    forward_reply(req.request_id, &to, permanent, reply_version, endianness);
                let _ = conn.write_message(reply_version, endianness, false, &reply);
            }
            continue;
        }
        // Part 2 §12.2: the client-exposed ZIOP policies of the client. A
        // client may send them once per connection (omniORB does), so they
        // hold for the connection until a later request sends others. An
        // INVOCATION_POLICIES context that does not decode is MARSHAL, the
        // request is not processed.
        match ziop.as_ref().map(|_| peer_ziop_of(&req.service_context)) {
            Some(Err(())) => {
                if req.response_flags.response_expected() {
                    let (reply, _, _) = build_reply(
                        req.request_id,
                        SkeletonResult::Exception(marshal_no()),
                        endianness,
                    );
                    let _ = conn.write_message(reply_version, endianness, false, &reply);
                }
                continue;
            }
            Some(Ok(Some(peer))) => peer_ziop = peer,
            Some(Ok(None)) | None => {}
        }
        // Observe the incoming service contexts (e.g. OTS PropagationContext in
        // id=0) before dispatch — inspection only.
        if let Some(obs) = shared.observer.lock().unwrap().clone() {
            obs(&req.service_context);
        }
        // The SAS reply (CompleteEstablishContext or ContextError) of a
        // server that requires CSIv2 credentials.
        let mut sas_reply: Option<SasMessage> = None;
        let mut dispatch = || {
            // CSIv2: if a validator is set, the request MUST carry a valid
            // SAS-EstablishContext (GSSUP) whose identity assertion, if any,
            // passes — otherwise NO_PERMISSION (before dispatch).
            let validator_opt = shared.validator.lock().unwrap().clone();
            let principal = match &validator_opt {
                Some(v) => {
                    let assertion = shared.identity_assertion.lock().unwrap().clone();
                    match csiv2_evaluate(&req.service_context, &**v, &assertion) {
                        SasOutcome::Accepted {
                            reply,
                            principal,
                            asserted,
                        } => {
                            sas_reply = Some(reply);
                            Some(Some(ReceivedPrincipal {
                                principal,
                                asserted,
                            }))
                        }
                        SasOutcome::Rejected { reply } => {
                            sas_reply = Some(reply);
                            None
                        }
                        SasOutcome::NoContext => None,
                    }
                }
                None => Some(None),
            };
            match principal {
                Some(principal) => {
                    let body = match received_body(&req, req_version, endianness) {
                        Ok(body) => body,
                        Err(exc) => return SkeletonResult::Exception(exc),
                    };
                    // Part 3 §6.4.3.1: `_component` on a facet or a
                    // component answers with the component's reference.
                    if req.operation == zerodds_corba_giop::object_ops::OP_COMPONENT
                        && objects.lock().unwrap().contains_key(&key)
                        && let Some(component) =
                            shared.components.lock().unwrap().get(&key).cloned()
                    {
                        let mut w = Writer::new(endianness);
                        component.encode(&mut w);
                        return SkeletonResult::Reply(w.finish());
                    }
                    let dispatcher = objects.lock().unwrap().get(&key).cloned();
                    let _scope = PrincipalScope::enter(principal);
                    let servant_orb = shared.orb.lock().unwrap().clone();
                    match (dispatcher, servant_orb) {
                        // Part 1 §8.9.1: the servant runs without the
                        // thread-scope overrides of the ORB, and what it
                        // sets there does not outlive the request.
                        (Some(d), Some(o)) => o
                            .policy_current()
                            .dispatch(|| d(&req.operation, &body, endianness)),
                        (Some(d), None) => d(&req.operation, &body, endianness),
                        (None, _) => unregistered_key_result(&req.operation, endianness),
                    }
                }
                None => SkeletonResult::Exception(CorbaException::system(
                    SystemExceptionKind::NoPermission,
                    0,
                    CompletionStatus::No,
                )),
            }
        };
        let orb = shared.orb.lock().unwrap().clone();
        let registry = orb
            .as_ref()
            .and_then(|o| o.interceptor_registry())
            .filter(|r| r.server_count() > 0);
        // A user exception the servant relays goes out in its own byte
        // order (`reply_status_and_body`): the reply is written in
        // `reply_endianness`.
        let (mut reply, reply_endianness, origin) = match (orb, registry) {
            (Some(orb), Some(registry)) => {
                intercepted_reply(&orb, &registry, &req, &key, endianness, dispatch)
            }
            _ => build_reply(req.request_id, dispatch(), endianness),
        };
        // §10.3.1.1: the SAS reply goes back in the reply's service
        // contexts (a ContextError with the NO_PERMISSION exception). The
        // two message shapes are small and fixed; a CDR encode failure
        // would leave the reply without the context, not drop the reply.
        if let (Some(sas), Message::Reply(r)) = (sas_reply, &mut reply)
            && let Ok(bytes) = sas.encode_encapsulation(endianness)
        {
            r.service_context.0.push(ServiceContext::new(
                ServiceContextTag::SecurityAttributeService.as_u32(),
                bytes,
            ));
        }
        // oneway: no reply.
        if !req.response_flags.response_expected() {
            continue;
        }
        // Part 2 §9.4.3: under GIOP 1.0/1.1 the body follows the reply
        // header without padding and goes on the offset modulo eight it
        // was written for (`place_reply_body`). A reply that does not
        // encode is not sent: the connection ends.
        if place_reply_body(&mut reply, reply_version, reply_endianness, origin).is_err() {
            break;
        }
        let Some(z) = ziop else {
            let _ = conn.write_message(reply_version, reply_endianness, false, &reply);
            continue;
        };
        // Part 2 §12.2: the server's client-exposed ZIOP policies go back
        // in the reply, in a GIOP version that has the context (Table
        // 7.2: from 1.2); the reply is a ZIOP message when both sides
        // enable compression with a compressor in common.
        if is_valid_for(
            ServiceContextTag::InvocationPolicies.as_u32(),
            reply_version,
        ) && let (Message::Reply(r), Ok(ctx)) = (&mut reply, z.invocation_policies(endianness))
        {
            r.service_context.0.push(ctx);
        }
        let send = reply_version
            .supports_ziop()
            .then(|| z.policies.send_policy(&peer_ziop, z.codec.as_ref()))
            .flatten();
        let _ = match send {
            Some(send) => conn
                .write_message_ziop(
                    reply_version,
                    reply_endianness,
                    &reply,
                    None,
                    &send,
                    z.codec.as_ref(),
                )
                .map(|_| ()),
            None => conn.write_message(reply_version, reply_endianness, false, &reply),
        };
    }
}

/// The ZIOP policies of a server (Part 2 §12): its own
/// ([`CorbaServer::set_ziop`]), else the ZIOP overrides at the scope of
/// its ORB with the ORB's `CompressionManager`; `None` without either.
fn server_ziop(ziop: &Mutex<Option<Ziop>>, orb: &Mutex<Option<Arc<Orb>>>) -> Option<Ziop> {
    if let Some(z) = ziop.lock().unwrap().clone() {
        return Some(z);
    }
    let orb = orb.lock().unwrap().clone()?;
    // The ORB's PolicyManager holds only ZIOP values that
    // `check_overrides` accepted, one per type: they always make ZIOP
    // policies, the error arm is not reached.
    let policies = match orb.server_ziop_policies() {
        Ok(p) => p?,
        Err(_) => return None,
    };
    let codec: Arc<dyn ZiopCodec> = orb.compression_manager();
    Some(Ziop { policies, codec })
}

/// The client-exposed ZIOP policies in the `INVOCATION_POLICIES` service
/// context of a request (Part 2 §12.2); `None` without that context or
/// without a ZIOP policy in it.
fn peer_ziop_of(contexts: &ServiceContextList) -> Result<Option<PeerZiop>, ()> {
    match invocation_policies_of(contexts) {
        Ok(Some(values))
            if values.iter().any(|v| {
                v.ptype == COMPRESSION_ENABLING_POLICY_ID
                    || v.ptype == COMPRESSOR_ID_LEVEL_LIST_POLICY_ID
            }) =>
        {
            PeerZiop::from_policy_values(&values)
                .map(Some)
                .map_err(|_| ())
        }
        Ok(_) => Ok(None),
        Err(_) => Err(()),
    }
}

/// What a server does with a message other than a `Request` or a
/// `LocateRequest` it read on `conn` (Part 2 §9.4); `true` when the
/// connection stays open.
///
/// * `CancelRequest` (§9.4.4) is advisory: the server answers each
///   request before it reads the next message, so the request it names
///   has been answered already and there is nothing left to cancel.
/// * `CloseConnection` (§9.4.7) and `MessageError` (§9.4.8): the peer
///   ends the connection or could not read what this side sent; the
///   connection ends.
/// * `Reply`, `LocateReply` and a `Fragment` without a fragmented message
///   before it (§9.4.9) are messages a server does not take: it answers
///   with `MessageError` and the connection ends.
pub(crate) fn server_other_message(
    conn: &mut zerodds_corba_iiop::Connection,
    msg: &Message,
) -> bool {
    match msg {
        Message::Request(_) | Message::LocateRequest(_) | Message::CancelRequest(_) => true,
        Message::CloseConnection(_) | Message::MessageError(_) => false,
        Message::Reply(_) | Message::LocateReply(_) | Message::Fragment(_) => {
            let _ = conn.write_message(
                Version::V1_0,
                Endianness::Big,
                false,
                &Message::MessageError(MessageError),
            );
            false
        }
    }
}

/// Answers a message whose header this server does not accept — an unknown
/// magic, a ZIOP message the server did not declare ZIOP for, an unknown
/// version or message type — with a GIOP `MessageError` before the
/// connection is closed (Part 2 §9.4.8); `true` when it did. Other read
/// errors (end of stream, I/O) close the connection only.
pub(crate) fn refuse_message(conn: &mut zerodds_corba_iiop::Connection, e: &IiopError) -> bool {
    let refused = matches!(
        e,
        IiopError::Giop(
            GiopError::InvalidMagic(_)
                | GiopError::UnsupportedVersion { .. }
                | GiopError::UnknownMessageType(_)
        )
    );
    if refused {
        // Part 2 §9.4.1: a request of a higher minor version than the
        // server supports is answered by an error message of the highest
        // minor version the server supports, then the connection closes.
        let version = if matches!(e, IiopError::Giop(GiopError::UnsupportedVersion { .. })) {
            Version::MAX
        } else {
            Version::V1_0
        };
        let _ = conn.write_message(
            version,
            Endianness::Big,
            false,
            &Message::MessageError(MessageError),
        );
    }
    refused
}

/// One request through the server request interceptors (CORBA 3.3 Part
/// 1 §16.4.10) and `PortableInterceptor::Current` (§16.5.4.5): the
/// request scope slots start empty; `receive_request_service_contexts`
/// runs in a thread scope of its own (§16.5.4.6); the request scope is
/// copied into the thread scope of the servant, in which
/// `receive_request` and the servant run; the thread scope is copied
/// back into the request scope for the ending points; the reply carries
/// the outcome and the reply service contexts. The thread scope of the
/// connection thread is restored afterwards.
fn intercepted_reply(
    orb: &Orb,
    registry: &InterceptorRegistry,
    req: &Request,
    key: &[u8],
    endianness: Endianness,
    dispatch: impl FnOnce() -> SkeletonResult,
) -> (Message, Endianness, usize) {
    let current = orb.pi_current();
    let mut info = RequestInfo::server(
        req.request_id,
        req.operation.clone(),
        req.response_flags.response_expected(),
        key.to_vec(),
    );
    info.set_sync_scope(i16::from(req.response_flags.0));
    info.set_server_identity(orb.id(), orb.server_id().unwrap_or_default());
    for sc in &req.service_context.0 {
        info.set_received_request_context(sc.context_id, sc.context_data.clone());
    }
    info.set_slots(current.empty_slots());
    let saved = current.thread_slots();
    current.set_thread_slots(current.empty_slots());
    let (outcome, body) = match registry.server_start(&mut info) {
        Ok(depth) => {
            // Step 5: after receive_request_service_contexts, before
            // receive_request.
            current.set_thread_slots(info.slots().to_vec());
            match registry.server_receive_request(&mut info, depth) {
                Ok(()) => {
                    let result = dispatch();
                    // Step 8: the servant's thread scope back into the
                    // request scope.
                    info.set_slots(current.thread_slots());
                    let (produced, body) = skeleton_outcome(result);
                    (registry.server_end(&mut info, depth, produced), body)
                }
                Err(outcome) => (outcome, Vec::new()),
            }
        }
        Err(outcome) => (outcome, Vec::new()),
    };
    current.set_thread_slots(saved);
    let (reply_status, body, reply_endianness, origin) = outcome_reply(outcome, body, endianness);
    let service_context = ServiceContextList(
        info.reply_service_contexts()
            .iter()
            .map(|(id, data)| ServiceContext {
                context_id: *id,
                context_data: data.clone(),
            })
            .collect(),
    );
    (
        Message::Reply(Reply {
            request_id: req.request_id,
            reply_status,
            service_context,
            body,
        }),
        reply_endianness,
        origin,
    )
}

/// A dispatch result as the outcome the server interceptors see, and the
/// reply body of a normal reply.
fn skeleton_outcome(result: SkeletonResult) -> (Outcome, Vec<u8>) {
    match result {
        SkeletonResult::Reply(body) => (Outcome::Proceed, body),
        SkeletonResult::Exception(CorbaException::UserException {
            repository_id,
            body,
            endianness,
        }) => (
            Outcome::Exception(RequestException::User {
                repository_id,
                body,
                endianness,
            }),
            Vec::new(),
        ),
        SkeletonResult::Exception(CorbaException::System(e)) => {
            (Outcome::Exception(RequestException::System(e)), Vec::new())
        }
        SkeletonResult::BadOperation => (
            Outcome::Exception(RequestException::System(SystemException::omg(
                SystemExceptionKind::BadOperation,
                2,
                CompletionStatus::No,
            ))),
            Vec::new(),
        ),
        SkeletonResult::NotYetWired => (
            Outcome::Exception(RequestException::System(SystemException::new(
                SystemExceptionKind::NoImplement,
                0,
                CompletionStatus::No,
            ))),
            Vec::new(),
        ),
        // SkeletonResult + CorbaException are `#[non_exhaustive]`.
        _ => (
            Outcome::Exception(RequestException::System(SystemException::new(
                SystemExceptionKind::Unknown,
                0,
                CompletionStatus::Maybe,
            ))),
            Vec::new(),
        ),
    }
}

/// The reply status and body for the outcome after the interceptors, the
/// byte order of the reply and the offset the body was written for
/// ([`reply_status_and_body`]).
fn outcome_reply(
    outcome: Outcome,
    body: Vec<u8>,
    endianness: Endianness,
) -> (ReplyStatusType, Vec<u8>, Endianness, usize) {
    let (status, body) = match outcome {
        Outcome::Proceed | Outcome::Other => (ReplyStatusType::NoException, body),
        Outcome::Exception(RequestException::System(e)) => (
            ReplyStatusType::SystemException,
            encode_system_exception(&e, endianness),
        ),
        Outcome::Exception(RequestException::User {
            body, endianness, ..
        }) => {
            return (
                ReplyStatusType::UserException,
                body.bytes().to_vec(),
                endianness,
                body.offset(),
            );
        }
        // Part 2 §9.4.3: the LOCATION_FORWARD reply body is the IOR.
        Outcome::Forward(target) => {
            let mut w = Writer::new(endianness);
            target.encode(&mut w);
            (ReplyStatusType::LocationForward, w.finish())
        }
        // `server_end` never produces TRANSPORT_RETRY, a client status.
        Outcome::Retry => (
            ReplyStatusType::SystemException,
            encode_system_exception(
                &SystemException::new(SystemExceptionKind::Internal, 0, CompletionStatus::No),
                endianness,
            ),
        ),
    };
    (status, body, endianness, 0)
}

/// Caps a GIOP version to the highest supported one, 1.4 (Part 2 §7.9).
/// Inbound versions above it are refused by the codec; this safeguards the
/// reply/request choice.
fn cap_giop_version(v: Version) -> Version {
    if v > Version::MAX { Version::MAX } else { v }
}

/// GIOP version for an outgoing request: from the IIOP profile version of the
/// target IOR (Part 2 §9.7.2: the profile's minor version is the highest GIOP
/// minor version the server supports), capped to the highest this client
/// supports, 1.4. The client speaks GIOP 1.0 to 1.3 when the target IOR says
/// so.
fn giop_request_version(profile: &IiopProfileBody) -> Version {
    cap_giop_version(Version::new(
        profile.iiop_version.major,
        profile.iiop_version.minor,
    ))
}

/// The `NEEDS_ADDRESSING_MODE` reply asking for `KeyAddr` (Part 2
/// §9.4.3.2): the request was not processed.
pub(crate) fn needs_key_addr_reply(request_id: u32, e: Endianness) -> Message {
    Message::Reply(Reply {
        request_id,
        reply_status: ReplyStatusType::NeedsAddressingMode,
        service_context: ServiceContextList::default(),
        body: key_addr_body(e),
    })
}

/// The `LOCATION_FORWARD(_PERM)` reply of a server that forwards the
/// object key (Part 2 §9.4.3.2, §9.6); `_PERM` only from GIOP 1.2 on.
fn forward_reply(
    request_id: u32,
    to: &ObjectReference,
    permanent: bool,
    version: Version,
    e: Endianness,
) -> Message {
    let mut w = Writer::new(e);
    to.encode(&mut w);
    Message::Reply(Reply {
        request_id,
        reply_status: if permanent && version.uses_v1_2_request_layout() {
            ReplyStatusType::LocationForwardPerm
        } else {
            ReplyStatusType::LocationForward
        },
        service_context: ServiceContextList::default(),
        body: w.finish(),
    })
}

/// The reply for a dispatch result, the byte order it goes out in and the
/// offset its body was written for ([`reply_status_and_body`]);
/// [`place_reply_body`] places it once the reply's service contexts are
/// complete.
fn build_reply(
    request_id: u32,
    result: SkeletonResult,
    endianness: Endianness,
) -> (Message, Endianness, usize) {
    let (status, body, reply_endianness, origin) = reply_status_and_body(result, endianness);
    (
        Message::Reply(Reply {
            request_id,
            reply_status: status,
            service_context: ServiceContextList::default(),
            body,
        }),
        reply_endianness,
        origin,
    )
}

/// Places the body of `reply` on the offset modulo eight it was written
/// for, `origin` (CORBA 3.3 Part 2 §9.4.3, `Reply::place_body_at`): 0 for
/// a body a servant wrote from its first octet, the offset in the reply it
/// came in for a user exception relayed from another ORB. A GIOP 1.2 reply
/// cannot carry a body written for four modulo eight; the operation ran,
/// its exception cannot go out as written: the reply becomes `MARSHAL`,
/// `COMPLETED_YES`, with the reply's service contexts.
///
/// # Errors
/// `()` when the reply header does not encode.
pub(crate) fn place_reply_body(
    reply: &mut Message,
    version: Version,
    endianness: Endianness,
    origin: usize,
) -> Result<(), ()> {
    let Message::Reply(r) = reply else {
        return Ok(());
    };
    if r.place_body_at(version, endianness, origin).is_ok() {
        return Ok(());
    }
    if origin.is_multiple_of(8) {
        return Err(());
    }
    r.reply_status = ReplyStatusType::SystemException;
    r.body = encode_system_exception(
        &SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::Yes),
        endianness,
    );
    r.place_body_at_eight(version, endianness).map_err(|_| ())
}

/// The reply status and body for a dispatch result, the byte order the
/// reply goes out in and the offset the body was written for: the
/// request's byte order `endianness` and 0, but for a user exception the
/// byte order and the offset its body keeps (a relayed one stood where
/// the reply it came in put it, in the byte order of that reply: its
/// octets cannot be read again in another, Part 2 §9.4.1). A system
/// exception goes out with its own repository id, minor code and
/// completion status (CORBA 3.3 Part 1 §8.12, Part 2 §9.4.3.2).
pub(crate) fn reply_status_and_body(
    result: SkeletonResult,
    endianness: Endianness,
) -> (ReplyStatusType, Vec<u8>, Endianness, usize) {
    let (status, body) = match result {
        SkeletonResult::Reply(b) => (ReplyStatusType::NoException, b),
        SkeletonResult::Exception(CorbaException::System(e)) => (
            ReplyStatusType::SystemException,
            encode_system_exception(&e, endianness),
        ),
        // `body` is already the complete exception reply body (repo_id +
        // members, contiguous) in the exception's byte order — use it
        // directly as the reply body (no re-encode, otherwise alignment
        // breaks), on the offset it was written for, in a reply of its
        // byte order.
        SkeletonResult::Exception(CorbaException::UserException {
            body, endianness, ..
        }) => {
            return (
                ReplyStatusType::UserException,
                body.bytes().to_vec(),
                endianness,
                body.offset(),
            );
        }
        // Annex A.6: BAD_OPERATION minor 2, "Operation or attribute not
        // known to target object".
        SkeletonResult::BadOperation => (
            ReplyStatusType::SystemException,
            encode_system_exception(
                &SystemException::omg(SystemExceptionKind::BadOperation, 2, CompletionStatus::No),
                endianness,
            ),
        ),
        SkeletonResult::NotYetWired => (
            ReplyStatusType::SystemException,
            encode_system_exception(
                &SystemException::new(SystemExceptionKind::NoImplement, 0, CompletionStatus::No),
                endianness,
            ),
        ),
        // SkeletonResult + CorbaException are `#[non_exhaustive]`.
        _ => (
            ReplyStatusType::SystemException,
            encode_system_exception(
                &SystemException::new(SystemExceptionKind::Unknown, 0, CompletionStatus::Maybe),
                endianness,
            ),
        ),
    };
    (status, body, endianness, 0)
}

// ---- Exception-Wire (CORBA §15.4.3) ----------------------------------------

fn encode_system_exception(e: &SystemException, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    e.encode(&mut w);
    w.finish()
}

/// The exception a `SYSTEM_EXCEPTION` reply carries. A body that does
/// not decode is `MARSHAL`, `COMPLETED_MAYBE`: the reply came, what it
/// says about the operation cannot be read.
fn decode_system_exception(body: &[u8], endianness: Endianness) -> CorbaException {
    let mut r = Reader::new(body, endianness);
    match SystemException::decode(&mut r) {
        Ok(e) => CorbaException::System(e),
        Err(_) => CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::Maybe),
    }
}

/// The exception of a `USER_EXCEPTION` reply. `body` is the complete
/// exception body (repository id and members) at its offset in the reply
/// message: the typed stub reads the repository id and then the members
/// through [`AlignedBody::reader`], whose alignment counts from the
/// message start (Part 2 §9.4.3); nothing is extracted here.
fn decode_user_exception(body: AlignedBody, endianness: Endianness) -> CorbaException {
    let Ok(repository_id) = body.reader(endianness).read_string() else {
        return CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::Maybe);
    };
    CorbaException::UserException {
        repository_id,
        body,
        endianness,
    }
}
