// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! JSON PSM — Spec §7.3.2.
//!
//! Wir liefern einen **Renderer** in JSON nach dem Spec-Schema (das
//! XSD-Schema spiegelt sich 1:1 in der JSON-Object-Struktur). Ein
//! Reader-Pfad braeuchte einen JSON-Parser im Workspace; das ist ein
//! separater Aufwand und wird hier explizit ausgelassen — der
//! XML-Pfad ist die kanonische Loader-Form, JSON ist primaer ein
//! Render-Output (z.B. fuer REST-APIs oder Tooling).

use alloc::format;
use alloc::string::String;

use super::DdsTsnConfig;
use super::application::{
    Application, ApplicationLibrary, Domain, DomainLibrary, DomainParticipant,
    DomainParticipantLibrary, QosLibrary, QosProfile, RegisteredType, TopicLibraryEntry,
};
use super::deployment::{Deployment, DeploymentLibrary, IpV4, IpV6, MacAddr, Node, NodeLibrary};

/// Renderer-Fehler — derzeit kein Fall noetig (Strings + Bytes
/// koennen nicht scheitern), aber wir definieren den Type fuer
/// Forward-Compat zu einem JSON-Numeric-Encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderJsonError {}

/// Rendert eine `DdsTsnConfig` als JSON-Objekt mit den Spec-Feld-
/// Namen `qos_library`, `domain_library`, `domain_participant_library`,
/// `application_library`, `node_library`, `deployment_library`.
///
/// # Errors
/// Aktuell kein Fail-Pfad; siehe [`RenderJsonError`].
pub fn render_dds_tsn_json(cfg: &DdsTsnConfig) -> Result<String, RenderJsonError> {
    let mut out = String::new();
    out.push('{');
    push_array(
        &mut out,
        "qos_library",
        &cfg.qos_libraries,
        render_qos_library,
    );
    out.push(',');
    push_array(
        &mut out,
        "domain_library",
        &cfg.domain_libraries,
        render_domain_library,
    );
    out.push(',');
    push_array(
        &mut out,
        "domain_participant_library",
        &cfg.domain_participant_libraries,
        render_dp_library,
    );
    out.push(',');
    push_array(
        &mut out,
        "application_library",
        &cfg.application_libraries,
        render_application_library,
    );
    out.push(',');
    push_array(
        &mut out,
        "node_library",
        &cfg.node_libraries,
        render_node_library,
    );
    out.push(',');
    push_array(
        &mut out,
        "deployment_library",
        &cfg.deployment_libraries,
        render_deployment_library,
    );
    out.push('}');
    Ok(out)
}

fn push_array<T>(out: &mut String, key: &str, items: &[T], mut render: impl FnMut(&T) -> String) {
    out.push('"');
    out.push_str(key);
    out.push_str("\":[");
    for (i, x) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&render(x));
    }
    out.push(']');
}

fn render_qos_library(lib: &QosLibrary) -> String {
    let profiles: alloc::vec::Vec<String> =
        lib.qos_profiles.iter().map(render_qos_profile).collect();
    format!(
        r#"{{"name":{},"qos_profile":[{}]}}"#,
        json_str(&lib.name),
        profiles.join(",")
    )
}

fn render_qos_profile(p: &QosProfile) -> String {
    let mut s = String::new();
    s.push_str(&format!(r#"{{"name":{}"#, json_str(&p.name)));
    if !p.domain_participant_qos.is_empty() {
        let names: alloc::vec::Vec<String> = p
            .domain_participant_qos
            .iter()
            .map(|q| format!(r#"{{"name":{}}}"#, json_str(&q.name)))
            .collect();
        s.push_str(&format!(
            r#","domain_participant_qos":[{}]"#,
            names.join(",")
        ));
    }
    if !p.datawriter_qos.is_empty() {
        let names: alloc::vec::Vec<String> = p
            .datawriter_qos
            .iter()
            .map(|q| format!(r#"{{"name":{}}}"#, json_str(&q.name)))
            .collect();
        s.push_str(&format!(r#","datawriter_qos":[{}]"#, names.join(",")));
    }
    if !p.datareader_qos.is_empty() {
        let names: alloc::vec::Vec<String> = p
            .datareader_qos
            .iter()
            .map(|q| format!(r#"{{"name":{}}}"#, json_str(&q.name)))
            .collect();
        s.push_str(&format!(r#","datareader_qos":[{}]"#, names.join(",")));
    }
    s.push('}');
    s
}

fn render_domain_library(lib: &DomainLibrary) -> String {
    let domains: alloc::vec::Vec<String> = lib.domains.iter().map(render_domain).collect();
    format!(
        r#"{{"name":{},"domain":[{}]}}"#,
        json_str(&lib.name),
        domains.join(",")
    )
}

fn render_domain(d: &Domain) -> String {
    let regs: alloc::vec::Vec<String> = d
        .registered_types
        .iter()
        .map(render_registered_type)
        .collect();
    let topics: alloc::vec::Vec<String> = d.topics.iter().map(render_topic).collect();
    format!(
        r#"{{"name":{},"registered_type":[{}],"topic":[{}]}}"#,
        json_str(&d.name),
        regs.join(","),
        topics.join(",")
    )
}

fn render_registered_type(r: &RegisteredType) -> String {
    format!(
        r#"{{"name":{},"type_ref":{}}}"#,
        json_str(&r.name),
        json_str(&r.type_ref)
    )
}

fn render_topic(t: &TopicLibraryEntry) -> String {
    format!(
        r#"{{"topic_name":{},"type_name":{}}}"#,
        json_str(&t.topic_name),
        json_str(&t.type_name)
    )
}

fn render_dp_library(lib: &DomainParticipantLibrary) -> String {
    let dps: alloc::vec::Vec<String> = lib.domain_participants.iter().map(render_dp).collect();
    format!(
        r#"{{"name":{},"domain_participant":[{}]}}"#,
        json_str(&lib.name),
        dps.join(",")
    )
}

fn render_dp(dp: &DomainParticipant) -> String {
    let mut s = format!(r#"{{"name":{}"#, json_str(&dp.name));
    if let Some(b) = &dp.base_name {
        s.push_str(&format!(r#","base_name":{}"#, json_str(b)));
    }
    if let Some(d) = &dp.domain_ref {
        s.push_str(&format!(r#","domain_ref":{}"#, json_str(d)));
    }
    s.push('}');
    s
}

fn render_application_library(lib: &ApplicationLibrary) -> String {
    let apps: alloc::vec::Vec<String> = lib.applications.iter().map(render_application).collect();
    format!(
        r#"{{"name":{},"application":[{}]}}"#,
        json_str(&lib.name),
        apps.join(",")
    )
}

fn render_application(a: &Application) -> String {
    let refs: alloc::vec::Vec<String> = a
        .domain_participant_refs
        .iter()
        .map(|r| json_str(r))
        .collect();
    format!(
        r#"{{"name":{},"domain_participant":[{}]}}"#,
        json_str(&a.name),
        refs.join(",")
    )
}

fn render_node_library(lib: &NodeLibrary) -> String {
    let nodes: alloc::vec::Vec<String> = lib.nodes.iter().map(render_node).collect();
    format!(
        r#"{{"name":{},"node":[{}]}}"#,
        json_str(&lib.name),
        nodes.join(",")
    )
}

fn render_node(n: &Node) -> String {
    let mut s = format!(
        r#"{{"name":{},"hostname":{},"mac_address":{}"#,
        json_str(&n.name),
        json_str(&n.hostname),
        json_str(&format_mac(n.mac_address)),
    );
    if let Some(v4) = n.ipv4_address {
        s.push_str(&format!(
            r#","ipv4_address":{}"#,
            json_str(&format_ipv4(v4))
        ));
    }
    if let Some(v6) = n.ipv6_address {
        s.push_str(&format!(
            r#","ipv6_address":{}"#,
            json_str(&format_ipv6(v6))
        ));
    }
    s.push('}');
    s
}

fn render_deployment_library(lib: &DeploymentLibrary) -> String {
    let deps: alloc::vec::Vec<String> = lib.deployments.iter().map(render_deployment).collect();
    format!(
        r#"{{"name":{},"deployment":[{}]}}"#,
        json_str(&lib.name),
        deps.join(",")
    )
}

fn render_deployment(d: &Deployment) -> String {
    let refs: alloc::vec::Vec<String> = d.application_refs.iter().map(|r| json_str(r)).collect();
    let mut s = format!(
        r#"{{"name":{},"node_ref":{},"application_list":[{}]"#,
        json_str(&d.name),
        json_str(&d.node_ref),
        refs.join(",")
    );
    if let Some(c) = &d.configuration {
        s.push_str(r#","configuration":{"#);
        if let Some(t) = &c.tsn_configuration_ref {
            s.push_str(&format!(r#""tsn":{{"name":{}}}"#, json_str(t)));
        }
        s.push('}');
    }
    s.push('}');
    s
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn format_mac(m: MacAddr) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        m.0[0], m.0[1], m.0[2], m.0[3], m.0[4], m.0[5]
    )
}

fn format_ipv4(v: IpV4) -> String {
    format!("{}.{}.{}.{}", v.0[0], v.0[1], v.0[2], v.0[3])
}

fn format_ipv6(v: IpV6) -> String {
    let groups: [u16; 8] =
        core::array::from_fn(|i| (u16::from(v.0[i * 2]) << 8) | u16::from(v.0[i * 2 + 1]));
    let parts: alloc::vec::Vec<String> = groups.iter().map(|g| format!("{g:x}")).collect();
    parts.join(":")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::pim::application::{DataReaderQosRef, DataWriterQosRef, DomainParticipantQosRef};

    fn full_config() -> DdsTsnConfig {
        DdsTsnConfig {
            qos_libraries: alloc::vec![QosLibrary {
                name: "lib1".into(),
                qos_profiles: alloc::vec![QosProfile {
                    name: "default".into(),
                    domain_participant_qos: alloc::vec![DomainParticipantQosRef {
                        name: "dp1".into(),
                        raw_qos: String::new(),
                    }],
                    datawriter_qos: alloc::vec![DataWriterQosRef {
                        name: "dw1".into(),
                        raw_qos: String::new(),
                    }],
                    datareader_qos: alloc::vec![DataReaderQosRef {
                        name: "dr1".into(),
                        raw_qos: String::new(),
                    }],
                    ..QosProfile::default()
                }],
            }],
            node_libraries: alloc::vec![NodeLibrary {
                name: "nlib".into(),
                nodes: alloc::vec![Node {
                    name: "Edge1".into(),
                    hostname: "edge1.lab".into(),
                    mac_address: MacAddr([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
                    ipv4_address: Some(IpV4([10, 0, 0, 1])),
                    ipv6_address: None,
                }],
            }],
            ..DdsTsnConfig::default()
        }
    }

    #[test]
    fn render_includes_six_top_level_arrays() {
        let cfg = full_config();
        let s = render_dds_tsn_json(&cfg).unwrap();
        for k in [
            "qos_library",
            "domain_library",
            "domain_participant_library",
            "application_library",
            "node_library",
            "deployment_library",
        ] {
            assert!(s.contains(k), "missing key {k} in {s}");
        }
    }

    #[test]
    fn render_qos_profile_emits_named_categories() {
        let cfg = full_config();
        let s = render_dds_tsn_json(&cfg).unwrap();
        assert!(s.contains(r#""name":"dp1""#));
        assert!(s.contains(r#""name":"dw1""#));
        assert!(s.contains(r#""name":"dr1""#));
    }

    #[test]
    fn render_node_emits_mac_in_canonical_form() {
        let cfg = full_config();
        let s = render_dds_tsn_json(&cfg).unwrap();
        assert!(s.contains("aa:bb:cc:dd:ee:ff"));
        assert!(s.contains("10.0.0.1"));
    }

    #[test]
    fn render_escapes_special_chars() {
        let cfg = DdsTsnConfig {
            qos_libraries: alloc::vec![QosLibrary {
                name: "lib\"with\\quotes".into(),
                qos_profiles: alloc::vec::Vec::new(),
            }],
            ..DdsTsnConfig::default()
        };
        let s = render_dds_tsn_json(&cfg).unwrap();
        assert!(s.contains(r#""lib\"with\\quotes""#));
    }
}
