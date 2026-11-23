// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The IOR of a mapping and its components (spec §4.1).

#[allow(unused_imports)]
use super::prelude::*;

pub(crate) fn host_port(bind: &str, default_port: u16) -> (String, u16) {
    match bind.rsplit_once(':') {
        Some((h, p)) => (
            h.trim_matches(['[', ']']).to_string(),
            p.parse().unwrap_or(default_port),
        ),
        None => (bind.to_string(), default_port),
    }
}

/// §4.1: the IOR of a mapping: one IIOP profile for the IIOP listener, its
/// object key; `advertise_host` replaces a wildcard bind address.
///
/// # Errors
/// An encoding error of the IOR codec.
pub fn build_ior(cfg: &DaemonConfig, m: &Mapping, advertise_host: &str) -> Result<Ior, String> {
    ior_for_key(cfg, &m.repo_id, object_key(m), advertise_host)
}

/// The host an IOR names: the bind address, or `advertise_host` for a
/// wildcard bind.
pub(crate) fn advertised(bind_host: String, advertise_host: &str) -> String {
    if bind_host == "0.0.0.0" || bind_host == "::" || bind_host.is_empty() {
        advertise_host.to_string()
    } else {
        bind_host
    }
}

/// The IOR of the object `key` on this daemon. With `corba.ssliop.required`
/// the IIOP profile names port 0: the object is reachable over SSLIOP only,
/// on the port `TAG_SSL_SEC_TRANS` names (SSLIOP, the port of a target that
/// accepts no unprotected IIOP).
///
/// # Errors
/// An encoding error of the IOR codec.
pub(crate) fn ior_for_key(
    cfg: &DaemonConfig,
    type_id: &str,
    key: Vec<u8>,
    advertise_host: &str,
) -> Result<Ior, String> {
    let (host, port) = host_port(&cfg.iiop_bind, 6833);
    let host = advertised(host, advertise_host);
    let port = if cfg.ssliop_required { 0 } else { port };
    let mut body = IiopProfileBody::new(
        IiopVersion::new(cfg.giop_version.major, cfg.giop_version.minor),
        host.clone(),
        port,
        key,
    );
    // IIOP 1.0 profiles carry no components (CORBA 3.3 Part 2 §15.7.2).
    if cfg.giop_version.minor >= 1 {
        body.components = profile_components(cfg, &host).map_err(|e| format!("{e:?}"))?;
    }
    let profile = TaggedProfile::iiop(&body, Endianness::Little).map_err(|e| format!("{e:?}"))?;
    Ok(Ior {
        type_id: type_id.to_string(),
        profiles: vec![profile],
    })
}

/// What the SSLIOP endpoint supports and requires (SSLIOP
/// `AssociationOptions`): integrity, confidentiality and trust in the target
/// always, trust in the client with a CA; without `corba.ssliop.required`
/// also no protection, since the plain IIOP listener serves the same objects.
pub(crate) fn ssl_options(cfg: &DaemonConfig) -> (u16, u16) {
    use zerodds_corba_csiv2::AssociationOptions as A;
    let with_ca = !cfg.ssliop_ca_file.is_empty();
    let mut supports = A::INTEGRITY | A::CONFIDENTIALITY | A::ESTABLISH_TRUST_IN_TARGET;
    let mut requires = 0;
    if !cfg.ssliop_required {
        supports |= A::NO_PROTECTION;
    } else {
        requires |= A::INTEGRITY | A::CONFIDENTIALITY;
    }
    if with_ca {
        supports |= A::ESTABLISH_TRUST_IN_CLIENT;
    }
    if with_ca && cfg.ssliop_require_client_cert {
        requires |= A::ESTABLISH_TRUST_IN_CLIENT;
    }
    (supports, requires)
}

/// UTF-8 (OSF code set registry `0x05010001`).
pub(crate) const CODE_SET_UTF8: u32 = 0x0501_0001;
/// ISO 8859-1 (`0x00010001`), the GIOP default for `char`.
pub(crate) const CODE_SET_ISO_8859_1: u32 = 0x0001_0001;
/// UTF-16 (`0x00010109`).
pub(crate) const CODE_SET_UTF16: u32 = 0x0001_0109;

/// The components of the bridge's IIOP profile:
///
/// * `TAG_CODE_SETS` (CORBA 3.3 Part 2 §13.10.2.4): `char` natively UTF-8 —
///   the bridge passes string bytes through to DDS, whose strings are UTF-8 —
///   with ISO 8859-1 as conversion; `wchar` UTF-16. Without it a client
///   assumes ISO 8859-1 and refuses `wchar`.
/// * `TAG_SSL_SEC_TRANS` (SSLIOP) when SSLIOP is enabled: its port, what it
///   supports (integrity, confidentiality, trust in the target; trust in
///   the client with a CA) and requires (trust in the client with
///   `require_client_cert`). Plain IIOP stays open next to it, so
///   `NoProtection` is supported and nothing else is required of a client
///   that does not ask for SSL. Without this component an SSLIOP client finds
///   no SSL port in the IOR.
///
/// No `TAG_ORB_TYPE`: its value is an OMG-assigned ORB type, and ZeroDDS has
/// none.
pub(crate) fn profile_components(
    cfg: &DaemonConfig,
    host: &str,
) -> Result<
    Vec<zerodds_corba_iiop::profile_body::TaggedComponent>,
    zerodds_corba_iiop::profile_body::CdrError,
> {
    use zerodds_corba_ior::component_tags::ComponentId;
    use zerodds_corba_ior::components::{
        CodeSetComponent, CodeSetComponentInfo, Ssl, StructuredComponent,
    };
    let component = |tag: ComponentId, c: StructuredComponent| {
        Ok::<_, zerodds_corba_iiop::profile_body::CdrError>(
            zerodds_corba_iiop::profile_body::TaggedComponent {
                tag: tag.as_u32(),
                component_data: c.encode_encapsulation(Endianness::Little)?,
            },
        )
    };
    let mut out = vec![component(
        ComponentId::CodeSets,
        StructuredComponent::CodeSets(CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: CODE_SET_UTF8,
                conversion_code_sets: vec![CODE_SET_ISO_8859_1],
            },
            for_wchar_data: CodeSetComponent {
                native_code_set: CODE_SET_UTF16,
                conversion_code_sets: Vec::new(),
            },
        }),
    )?];
    if cfg.ssliop_enabled {
        let (_, ssl_port) = host_port(&cfg.ssliop_bind, 6834);
        let (supports, requires) = ssl_options(cfg);
        out.push(component(
            ComponentId::SslSecTrans,
            StructuredComponent::Ssl(Ssl {
                target_supports: supports,
                target_requires: requires,
                port: ssl_port,
            }),
        )?);
    }
    if cfg.csiv2_enabled {
        out.push(component(
            ComponentId::CsiSecMechList,
            StructuredComponent::CsiSecMechList(csiv2_mech_list(cfg, host)?),
        )?);
    }
    Ok(out)
}

/// The `TLS_SEC_TRANS` body of a CSIv2 transport mechanism (CSIv2,
/// `TAG_TLS_SEC_TRANS`): `target_supports`, `target_requires` and the one
/// `TransportAddress` (host, port) of the SSLIOP listener, as an
/// encapsulation.
pub(crate) fn tls_sec_trans(cfg: &DaemonConfig, host: &str) -> Vec<u8> {
    let (_, ssl_port) = host_port(&cfg.ssliop_bind, 6834);
    let (supports, requires) = ssl_options(cfg);
    let mut w = zerodds_wire::Writer::new(Endianness::Little);
    w.write_u8(1); // little-endian encapsulation octet
    w.write_u16(supports);
    w.write_u16(requires);
    w.write_u32(1);
    w.write_string(host);
    w.write_u16(ssl_port);
    w.finish()
}

/// The `TAG_CSI_SEC_MECH_LIST` component body (CSIv2, `CompoundSecMechList`):
/// GSSUP as the authentication layer and identity assertion as configured,
/// over each transport the daemon offers — TLS on the SSLIOP listener
/// (`TAG_TLS_SEC_TRANS`), and plain IIOP (`TAG_NULL_TAG`) unless
/// `corba.ssliop.required`.
///
/// Every mechanism requires trust in the client: a request is accepted
/// with a client certificate the TLS layer verified or with GSSUP
/// credentials (`server::on_request`). Over TLS with a CA the certificate
/// can carry it, so GSSUP is supported there and required only where no
/// certificate can.
pub(crate) fn csiv2_mech_list(
    cfg: &DaemonConfig,
    host: &str,
) -> Result<zerodds_corba_csiv2::CompoundSecMechList, zerodds_corba_iiop::profile_body::CdrError> {
    use zerodds_corba_csiv2::{
        AsContextSec, AssociationOptions as A, CompoundSecMech, CompoundSecMechList, SasContextSec,
    };
    use zerodds_corba_ior::component_tags::ComponentId;
    let mech = |transport_mech_tag: u32, transport_mech_data: Vec<u8>, certificate: bool| {
        let (_, ssl_requires) = ssl_options(cfg);
        let transport_requires = if transport_mech_tag == ComponentId::TlsSecTrans.as_u32() {
            ssl_requires
        } else {
            0
        };
        CompoundSecMech {
            target_requires: A::default()
                .with(A::ESTABLISH_TRUST_IN_CLIENT)
                .with(transport_requires),
            transport_mech_tag,
            transport_mech_data,
            as_context: AsContextSec {
                target_supports: A::default().with(A::ESTABLISH_TRUST_IN_CLIENT),
                target_requires: if certificate {
                    A::default()
                } else {
                    A::default().with(A::ESTABLISH_TRUST_IN_CLIENT)
                },
                client_authentication_mech: zerodds_corba_csiv2::gssup::GSSUP_OID_DER.to_vec(),
                target_name: cfg.csiv2_target_name.as_bytes().to_vec(),
            },
            sas_context: SasContextSec {
                target_supports: if cfg.csiv2_target_supports == 0 {
                    A::default()
                } else {
                    A::default().with(A::IDENTITY_ASSERTION)
                },
                target_requires: A::default(),
                privilege_authorities: Vec::new(),
                supported_naming_mechanisms: Vec::new(),
                supported_identity_types: cfg.csiv2_target_supports,
            },
        }
    };
    let mut mechanism_list = Vec::new();
    if cfg.ssliop_enabled {
        mechanism_list.push(mech(
            ComponentId::TlsSecTrans.as_u32(),
            tls_sec_trans(cfg, host),
            !cfg.ssliop_ca_file.is_empty(),
        ));
    }
    if !cfg.ssliop_required {
        mechanism_list.push(mech(ComponentId::NullTag.as_u32(), Vec::new(), false));
    }
    Ok(CompoundSecMechList {
        // A stateless target: the daemon evaluates an EstablishContext on
        // every request (`csiv2_wire::evaluate`), never `MessageInContext`.
        stateful: false,
        mechanism_list,
    })
}

/// The stringified IORs, one line per mapping that receives GIOP:
/// `<repo_id> <operation> IOR:…`.
///
/// # Errors
/// An encoding error of the IOR codec.
pub fn ior_lines(cfg: &DaemonConfig, advertise_host: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for m in cfg.mappings.iter().filter(|m| m.receives_giop()) {
        let ior = build_ior(cfg, m, advertise_host)?;
        let s = to_stringified(&ior, Endianness::Little).map_err(|e| format!("{e:?}"))?;
        out.push(format!("{} {} {s}", m.repo_id, m.operation));
    }
    // §1 L6: a tenant's objects, its name first.
    for t in &cfg.tenants {
        let tc = cfg.of_tenant(t);
        for m in tc.mappings.iter().filter(|m| m.receives_giop()) {
            let ior = build_ior(&tc, m, advertise_host)?;
            let s = to_stringified(&ior, Endianness::Little).map_err(|e| format!("{e:?}"))?;
            out.push(format!("@{} {} {} {s}", t.name, m.repo_id, m.operation));
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::bridge_daemon::config::tests::load;

    /// §4.1: the profile carries TAG_CODE_SETS and, with SSLIOP,
    /// TAG_SSL_SEC_TRANS naming the SSL port and what it requires.
    #[test]
    fn corbabridge_4_1_the_ior_carries_code_sets_and_the_ssl_component() {
        use zerodds_corba_ior::component_tags::ComponentId;
        use zerodds_corba_ior::components::{StructuredComponent, TaggedComponent};
        let yaml = "corba:\n  iiop:\n    bind: \"127.0.0.1:7001\"\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:7002\"\n    cert_file: c.pem\n    key_file: k.pem\n    ca_file: ca.pem\n    require_client_cert: true\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let structured: Vec<StructuredComponent> = body
            .components
            .iter()
            .map(|c| {
                TaggedComponent {
                    tag: ComponentId::from_u32(c.tag),
                    component_data: c.component_data.clone(),
                }
                .structured()
                .unwrap()
            })
            .collect();
        match &structured[..] {
            [
                StructuredComponent::CodeSets(cs),
                StructuredComponent::Ssl(ssl),
            ] => {
                assert_eq!(cs.for_char_data.native_code_set, 0x0501_0001);
                assert_eq!(cs.for_wchar_data.native_code_set, 0x0001_0109);
                assert_eq!(ssl.port, 7002);
                assert_eq!(ssl.target_requires, 64, "EstablishTrustInClient");
                assert_eq!(ssl.target_supports & 0x0067, 0x0067);
            }
            other => panic!("components {other:?}"),
        }
        let plain = load("mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        let ior = build_ior(&plain, &plain.mappings[0], "127.0.0.1").unwrap();
        let tags: Vec<u32> = ior.profiles[0]
            .as_iiop()
            .unwrap()
            .unwrap()
            .components
            .iter()
            .map(|c| c.tag)
            .collect();
        assert_eq!(tags, [1], "without SSLIOP: code sets only");
    }

    /// §7.2 / CORBA 3.4 Part 2 §10.5: with `corba.csiv2.enabled` the IOR
    /// also carries `TAG_CSI_SEC_MECH_LIST`, naming the GSSUP AS-mechanism,
    /// the configured `target_name` and `target_supports`. Without SSLIOP
    /// its transport_mech is `TAG_NULL_TAG`; with SSLIOP it is the SSL port.
    #[test]
    fn the_ior_carries_the_csiv2_mech_list_when_enabled() {
        use zerodds_corba_ior::component_tags::ComponentId;
        use zerodds_corba_ior::components::{StructuredComponent, TaggedComponent};
        let yaml = "corba:\n  csiv2:\n    enabled: true\n    target_supports: [principal_name]\n    gssup:\n      password_file: /etc/zerodds/gssup.passwd\n      target_name: REALM.LAB\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let structured: Vec<StructuredComponent> = body
            .components
            .iter()
            .map(|c| {
                TaggedComponent {
                    tag: ComponentId::from_u32(c.tag),
                    component_data: c.component_data.clone(),
                }
                .structured()
                .unwrap()
            })
            .collect();
        let found = structured
            .iter()
            .find(|c| matches!(c, StructuredComponent::CsiSecMechList(_)))
            .expect("TAG_CSI_SEC_MECH_LIST present");
        let StructuredComponent::CsiSecMechList(list) = found else {
            panic!("expected CsiSecMechList, got {found:?}");
        };
        assert!(!list.stateful, "SAS EstablishContext per request (§10.3)");
        assert_eq!(list.mechanism_list.len(), 1);
        let mech = &list.mechanism_list[0];
        assert_eq!(
            mech.transport_mech_tag,
            ComponentId::NullTag.as_u32(),
            "no SSLIOP configured"
        );
        assert_eq!(
            mech.as_context.client_authentication_mech,
            zerodds_corba_csiv2::gssup::GSSUP_OID_DER
        );
        assert_eq!(mech.as_context.target_name, b"REALM.LAB");
        assert_eq!(
            mech.sas_context.supported_identity_types,
            zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME
        );

        let with_ssl = "corba:\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:7002\"\n    cert_file: c.pem\n    key_file: k.pem\n  csiv2:\n    enabled: true\n    gssup:\n      password_file: /etc/zerodds/gssup.passwd\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(with_ssl).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let mech_list_component = body
            .components
            .iter()
            .find(|c| ComponentId::from_u32(c.tag) == ComponentId::CsiSecMechList)
            .expect("component present");
        let structured = TaggedComponent {
            tag: ComponentId::CsiSecMechList,
            component_data: mech_list_component.component_data.clone(),
        }
        .structured()
        .unwrap();
        let StructuredComponent::CsiSecMechList(list) = &structured else {
            panic!("expected CsiSecMechList, got {structured:?}");
        };
        // CSIv2: TLS on the SSLIOP listener as TAG_TLS_SEC_TRANS with its
        // address, then plain IIOP as TAG_NULL_TAG.
        assert_eq!(list.mechanism_list.len(), 2);
        let tls = &list.mechanism_list[0];
        assert_eq!(tls.transport_mech_tag, ComponentId::TlsSecTrans.as_u32());
        assert_eq!(
            list.mechanism_list[1].transport_mech_tag,
            ComponentId::NullTag.as_u32()
        );
        let body = &tls.transport_mech_data;
        // An encapsulation aligns from its byte-order octet.
        let mut r = zerodds_wire::Reader::new(body, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 1, "a little-endian encapsulation");
        let supports = r.read_u16().unwrap();
        let _requires = r.read_u16().unwrap();
        assert_ne!(supports & 0x0006, 0, "Integrity and Confidentiality");
        assert_eq!(r.read_u32().unwrap(), 1, "one TransportAddress");
        assert_eq!(r.read_string().unwrap(), "127.0.0.1");
        assert_eq!(r.read_u16().unwrap(), 7002);
    }

    /// `corba.iiop.advertise_host` names the host of a wildcard bind; its
    /// environment variable maps onto that key.
    #[test]
    fn the_advertise_host_is_a_setting() {
        let yaml = "corba:\n  iiop:\n    bind: \"0.0.0.0:7001\"\n    advertise_host: bridge.example\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], &cfg.iiop_advertise_host).unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        assert_eq!((body.host.as_str(), body.port), ("bridge.example", 7001));
        let default = load("mappings: []\n").unwrap();
        assert_eq!(default.iiop_advertise_host, "127.0.0.1");
        for (_, key) in ENV {
            assert!(is_key(key), "{key}");
        }
    }

    #[test]
    fn corbabridge_4_4_corbaloc_and_ior_targets_resolve() {
        let t = resolve_target("corbaloc:iiop:1.2@127.0.0.1:2809/Listener").unwrap();
        assert_eq!(
            (t.host.as_str(), t.port, t.object_key.as_slice()),
            ("127.0.0.1", 2809, &b"Listener"[..])
        );
        let cfg = load("corba:\n  iiop:\n    bind: \"0.0.0.0:7001\"\nmappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        let line = &ior_lines(&cfg, "10.0.0.5").unwrap()[0];
        let ior = line.split(' ').nth(2).unwrap();
        let t = resolve_target(ior).unwrap();
        assert_eq!((t.host.as_str(), t.port), ("10.0.0.5", 7001));
        assert_eq!(t.object_key, object_key(&cfg.mappings[0]));
        // CORBA 3.3 Part 2 §7.6.9, §7.6.10: the case of the scheme is not
        // significant.
        let lower = format!("ior:{}", &ior[4..]);
        assert_eq!(resolve_target(&lower).unwrap(), t);
        let t = resolve_target("CorbaLoc:iiop:1.2@127.0.0.1:2809/Listener").unwrap();
        assert_eq!((t.host.as_str(), t.port), ("127.0.0.1", 2809));
        assert!(resolve_target("corbaname::127.0.0.1#x").is_err());
    }

    /// IIOP 1.0 profiles carry no components (CORBA 3.3 Part 2 §15.7.2).
    #[test]
    fn corbabridge_4_1_an_iiop_1_0_profile_has_no_components() {
        let cfg = load("corba:\n  iiop:\n    giop_version: \"1.0\"\nmappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        assert_eq!((body.iiop_version.major, body.iiop_version.minor), (1, 0));
        assert!(body.components.is_empty());
    }

    /// §1 L6: a tenant's objects have their lines, its name first.
    #[test]
    fn corbabridge_4_1_a_tenants_ior_line_names_the_tenant() {
        let cfg = load("tenants:\n  - name: plant-a\n    domain: 4\n    subjects: [\"CN=a\"]\n    mappings:\n      - repo_id: \"IDL:x/Y:1.0\"\n        operation: op\n").unwrap();
        let lines = ior_lines(&cfg, "127.0.0.1").unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].starts_with("@plant-a IDL:x/Y:1.0 op IOR:"),
            "{}",
            lines[0]
        );
    }
}
