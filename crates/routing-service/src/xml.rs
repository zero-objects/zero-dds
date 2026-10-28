// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XML config parser. Supports a native `<zerodds_routing>` form and a focused
//! subset of the RTI Routing Service `<dds><routing_service>` schema.

use roxmltree::{Document, Node};

use crate::config::ContentFilter;
use crate::config::{Durability, Endpoint, Ownership, QosSpec, Route, RouterConfig};
use crate::error::{Result, RoutingError};

fn cfg_err(msg: impl Into<String>) -> RoutingError {
    RoutingError::Config(msg.into())
}

/// Parses either supported XML form into a [`RouterConfig`].
pub(crate) fn parse_router_xml(s: &str) -> Result<RouterConfig> {
    zerodds_foundation::check_nesting(s, zerodds_foundation::XML_NESTING_LIMIT)
        .map_err(|e| cfg_err(format!("xml: {e}")))?;
    let doc = Document::parse(s).map_err(|e| cfg_err(format!("xml: {e}")))?;
    let root = doc.root_element();
    match root.tag_name().name() {
        "zerodds_routing" => parse_native(root),
        "dds" => parse_rti(root),
        "routing_service" => parse_rti_service(root, "zerodds-router"),
        other => Err(cfg_err(format!(
            "unsupported root element <{other}> (expected <zerodds_routing> or <dds>)"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Native form
// ---------------------------------------------------------------------------

fn parse_native(root: Node) -> Result<RouterConfig> {
    only(root, &["name"], &["route"])?;
    let name = root
        .attribute("name")
        .unwrap_or("zerodds-router")
        .to_string();
    let mut routes = Vec::new();
    for rn in root.children().filter(|n| n.has_tag_name("route")) {
        routes.push(parse_native_route(rn)?);
    }
    if routes.is_empty() {
        return Err(cfg_err("<zerodds_routing> has no <route> children"));
    }
    // XML router configs do not carry an access-control list — the ACL is a
    // JSON-config feature (O1). XML routes run unrestricted.
    Ok(RouterConfig {
        name,
        routes,
        access_control: None,
    })
}

fn parse_native_route(rn: Node) -> Result<Route> {
    only(
        rn,
        &[
            "name",
            "loop_guard",
            "preserve_source_timestamp",
            "max_samples_per_second",
            "tenant",
            "relay",
        ],
        &["input", "output"],
    )?;
    let name = rn
        .attribute("name")
        .ok_or_else(|| cfg_err("<route> missing name"))?
        .to_string();
    let input = parse_native_endpoint(
        child(rn, "input").ok_or_else(|| cfg_err(format!("route '{name}': missing <input>")))?,
        true,
    )?;
    let output = parse_native_endpoint(
        child(rn, "output").ok_or_else(|| cfg_err(format!("route '{name}': missing <output>")))?,
        false,
    )?;
    let filter = child(rn, "input")
        .and_then(|i| child(i, "filter"))
        .and_then(|f| f.text())
        .map(|t| ContentFilter {
            expression: t.trim().to_string(),
            parameters: Vec::new(),
        });
    Ok(Route {
        name,
        input,
        output,
        filter,
        transform: None,
        loop_guard: bool_attr(rn, "loop_guard", true)?,
        preserve_source_timestamp: bool_attr(rn, "preserve_source_timestamp", false)?,
        max_samples_per_second: rn
            .attribute("max_samples_per_second")
            .map(|v| {
                v.parse::<u32>().map_err(|_| {
                    cfg_err(format!(
                        "{}: max_samples_per_second=\"{v}\" is not a count",
                        at(rn)
                    ))
                })
            })
            .transpose()?,
        tenant: rn.attribute("tenant").map(str::to_string),
        relay: bool_attr(rn, "relay", false)?,
    })
}

fn parse_native_endpoint(en: Node, input: bool) -> Result<Endpoint> {
    // `<filter>` belongs to the input: a filter on the output was dropped.
    let children: &[&str] = if input {
        &["partition", "qos", "filter"]
    } else {
        &["partition", "qos"]
    };
    only(
        en,
        &["domain", "topic", "type_name", "keyed", "type_file"],
        children,
    )?;
    for leaf in en
        .children()
        .filter(|n| n.has_tag_name("partition") || n.has_tag_name("filter"))
    {
        only(leaf, &[], &[])?;
    }
    let domain: i32 = en
        .attribute("domain")
        .ok_or_else(|| cfg_err("endpoint missing domain"))?
        .parse()
        .map_err(|_| cfg_err("endpoint domain not an integer"))?;
    let topic = en
        .attribute("topic")
        .ok_or_else(|| cfg_err("endpoint missing topic"))?
        .to_string();
    let type_name = en.attribute("type_name").unwrap_or("*").to_string();
    let keyed = bool_attr(en, "keyed", false)?;
    let partition: Vec<String> = en
        .children()
        .filter(|n| n.has_tag_name("partition"))
        .filter_map(|p| p.text())
        .map(|t| t.trim().to_string())
        .collect();
    let qos = child(en, "qos")
        .map(parse_qos)
        .transpose()?
        .unwrap_or_default();
    Ok(Endpoint {
        domain,
        topic,
        type_name,
        keyed,
        partition,
        qos,
        type_file: en.attribute("type_file").unwrap_or_default().to_string(),
    })
}

fn parse_qos(qn: Node) -> Result<QosSpec> {
    only(
        qn,
        &["reliable", "durability", "ownership", "ownership_strength"],
        &[],
    )?;
    let reliable = bool_attr(qn, "reliable", true)?;
    let durability = match qn.attribute("durability").unwrap_or("volatile") {
        "volatile" => Durability::Volatile,
        "transient_local" => Durability::TransientLocal,
        "transient" => Durability::Transient,
        "persistent" => Durability::Persistent,
        other => return Err(cfg_err(format!("unknown durability '{other}'"))),
    };
    let ownership = match qn.attribute("ownership").unwrap_or("shared") {
        "shared" => Ownership::Shared,
        "exclusive" => Ownership::Exclusive,
        other => return Err(cfg_err(format!("unknown ownership '{other}'"))),
    };
    let ownership_strength = qn
        .attribute("ownership_strength")
        .map(|s| {
            s.parse()
                .map_err(|_| cfg_err("ownership_strength not an integer"))
        })
        .transpose()?
        .unwrap_or(0);
    Ok(QosSpec {
        reliable,
        durability,
        ownership,
        ownership_strength,
        data_representation: None,
    })
}

// ---------------------------------------------------------------------------
// RTI subset
// ---------------------------------------------------------------------------
//
// Supported shape (the common case):
//
//   <dds>
//     <routing_service name="r">
//       <domain_route name="dr">
//         <participant name="p_in">  <domain_id>0</domain_id> </participant>
//         <participant name="p_out"> <domain_id>1</domain_id> </participant>
//         <session name="s">
//           <topic_route name="tr">
//             <input participant="p_in">
//               <registered_type_name>T</registered_type_name>
//               <topic_name>Src</topic_name>
//             </input>
//             <output participant="p_out">
//               <registered_type_name>T</registered_type_name>
//               <topic_name>Dst</topic_name>
//             </output>
//           </topic_route>
//         </session>
//       </domain_route>
//     </routing_service>
//   </dds>

fn parse_rti(dds: Node) -> Result<RouterConfig> {
    only(dds, &[], &["routing_service"])?;
    let svc =
        child(dds, "routing_service").ok_or_else(|| cfg_err("<dds> has no <routing_service>"))?;
    let name = svc.attribute("name").unwrap_or("zerodds-router");
    parse_rti_service(svc, name)
}

fn parse_rti_service(svc: Node, default_name: &str) -> Result<RouterConfig> {
    only(svc, &["name"], &["domain_route"])?;
    let name = svc.attribute("name").unwrap_or(default_name).to_string();
    let mut routes = Vec::new();
    for dr in svc.children().filter(|n| n.has_tag_name("domain_route")) {
        only(dr, &["name"], &["participant", "session"])?;
        // participant name → domain id
        let mut dom_of = std::collections::BTreeMap::new();
        for p in dr.children().filter(|n| n.has_tag_name("participant")) {
            only(p, &["name"], &["domain_id"])?;
            let pname = p
                .attribute("name")
                .ok_or_else(|| cfg_err("<participant> missing name"))?;
            let dom: i32 = child(p, "domain_id")
                .and_then(|d| d.text())
                .ok_or_else(|| cfg_err(format!("participant '{pname}' missing <domain_id>")))?
                .trim()
                .parse()
                .map_err(|_| cfg_err("domain_id not an integer"))?;
            dom_of.insert(pname.to_string(), dom);
        }
        for sess in dr.children().filter(|n| n.has_tag_name("session")) {
            only(sess, &["name"], &["topic_route"])?;
            for tr in sess.children().filter(|n| n.has_tag_name("topic_route")) {
                routes.push(parse_rti_topic_route(tr, &dom_of)?);
            }
        }
    }
    if routes.is_empty() {
        return Err(cfg_err("RTI config produced no routes"));
    }
    // XML router configs do not carry an access-control list — the ACL is a
    // JSON-config feature (O1). XML routes run unrestricted.
    Ok(RouterConfig {
        name,
        routes,
        access_control: None,
    })
}

fn parse_rti_topic_route(
    tr: Node,
    dom_of: &std::collections::BTreeMap<String, i32>,
) -> Result<Route> {
    only(tr, &["name"], &["input", "output"])?;
    let name = tr.attribute("name").unwrap_or("topic_route").to_string();
    let inp =
        child(tr, "input").ok_or_else(|| cfg_err(format!("topic_route '{name}': no <input>")))?;
    let out =
        child(tr, "output").ok_or_else(|| cfg_err(format!("topic_route '{name}': no <output>")))?;
    let input = rti_endpoint(inp, dom_of, &name)?;
    let output = rti_endpoint(out, dom_of, &name)?;
    Ok(Route {
        name,
        input,
        output,
        filter: None,
        transform: None,
        loop_guard: true,
        preserve_source_timestamp: false,
        max_samples_per_second: None,
        tenant: None,
        relay: false,
    })
}

fn rti_endpoint(
    n: Node,
    dom_of: &std::collections::BTreeMap<String, i32>,
    route: &str,
) -> Result<Endpoint> {
    only(n, &["participant"], &["registered_type_name", "topic_name"])?;
    let pname = n
        .attribute("participant")
        .ok_or_else(|| cfg_err(format!("route '{route}': input/output missing participant")))?;
    let domain = *dom_of
        .get(pname)
        .ok_or_else(|| cfg_err(format!("route '{route}': unknown participant '{pname}'")))?;
    let topic = child(n, "topic_name")
        .and_then(|t| t.text())
        .ok_or_else(|| cfg_err(format!("route '{route}': missing <topic_name>")))?
        .trim()
        .to_string();
    let type_name = child(n, "registered_type_name")
        .and_then(|t| t.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| "*".to_string());
    Ok(Endpoint {
        domain,
        topic,
        type_name,
        keyed: false,
        partition: Vec::new(),
        qos: QosSpec::default(),
        type_file: String::new(),
    })
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn child<'a, 'd>(n: Node<'a, 'd>, tag: &str) -> Option<Node<'a, 'd>> {
    n.children().find(|c| c.has_tag_name(tag))
}

/// A boolean attribute; an unreadable value is an error, it used to become
/// the default without a word.
fn bool_attr(n: Node, attr: &str, default: bool) -> Result<bool> {
    match n.attribute(attr) {
        None => Ok(default),
        Some("true" | "1" | "yes") => Ok(true),
        Some("false" | "0" | "no") => Ok(false),
        Some(other) => Err(cfg_err(format!(
            "{}: {attr}=\"{other}\" is not a boolean (true/false)",
            at(n)
        ))),
    }
}

/// `<tag>` at line L of the document.
fn at(n: Node) -> String {
    let pos = n.document().text_pos_at(n.range().start);
    format!("line {}: <{}>", pos.row, n.tag_name().name())
}

/// Refuses an attribute or child element the parser does not read. Both
/// forms used to skip them: an RTI element outside the supported subset,
/// or a misspelt attribute, changed the routing without a message.
fn only(n: Node, attrs: &[&str], children: &[&str]) -> Result<()> {
    if let Some(a) = n.attributes().find(|a| !attrs.contains(&a.name())) {
        return Err(cfg_err(format!(
            "{}: attribute {} is not read (expected: {})",
            at(n),
            a.name(),
            if attrs.is_empty() {
                "none".to_string()
            } else {
                attrs.join(", ")
            }
        )));
    }
    if let Some(c) = n
        .children()
        .filter(Node::is_element)
        .find(|c| !children.contains(&c.tag_name().name()))
    {
        return Err(cfg_err(format!(
            "{}: element <{}> is not read (expected: {})",
            at(n),
            c.tag_name().name(),
            if children.is_empty() {
                "none".to_string()
            } else {
                children.join(", ")
            }
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn native_basic() {
        let xml = r#"
        <zerodds_routing name="r1">
          <route name="a" loop_guard="true">
            <input domain="0" topic="Src" type_name="T" keyed="true">
              <partition>p</partition>
              <qos reliable="true" durability="volatile"/>
            </input>
            <output domain="1" topic="Dst" type_name="T" keyed="true">
              <qos reliable="true" durability="transient_local" ownership="exclusive" ownership_strength="5"/>
            </output>
          </route>
        </zerodds_routing>"#;
        let c = parse_router_xml(xml).unwrap();
        assert_eq!(c.name, "r1");
        assert_eq!(c.routes.len(), 1);
        let r = &c.routes[0];
        assert_eq!(r.input.domain, 0);
        assert_eq!(r.output.domain, 1);
        assert_eq!(r.output.topic, "Dst");
        assert!(r.input.keyed && r.output.keyed);
        assert_eq!(r.input.partition, vec!["p".to_string()]);
        assert_eq!(r.output.qos.ownership_strength, 5);
        assert_eq!(r.output.qos.durability, Durability::TransientLocal);
    }

    #[test]
    fn native_relay() {
        let xml = r#"
        <zerodds_routing>
          <route name="a" relay="true">
            <input domain="3" topic="T" type_name="X"/>
            <output domain="3" topic="T" type_name="X"/>
          </route>
        </zerodds_routing>"#;
        let c = parse_router_xml(xml).unwrap();
        assert!(c.routes[0].relay);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn native_filter() {
        let xml = r#"
        <zerodds_routing>
          <route name="a">
            <input domain="0" topic="S"><filter>temp &gt; 50</filter></input>
            <output domain="1" topic="S"/>
          </route>
        </zerodds_routing>"#;
        let c = parse_router_xml(xml).unwrap();
        assert_eq!(c.routes[0].filter.as_ref().unwrap().expression, "temp > 50");
    }

    #[test]
    fn rti_subset() {
        let xml = r#"
        <dds>
          <routing_service name="svc">
            <domain_route name="dr">
              <participant name="p_in"><domain_id>0</domain_id></participant>
              <participant name="p_out"><domain_id>2</domain_id></participant>
              <session name="s">
                <topic_route name="tr">
                  <input participant="p_in">
                    <registered_type_name>ShapeType</registered_type_name>
                    <topic_name>Square</topic_name>
                  </input>
                  <output participant="p_out">
                    <registered_type_name>ShapeType</registered_type_name>
                    <topic_name>Circle</topic_name>
                  </output>
                </topic_route>
              </session>
            </domain_route>
          </routing_service>
        </dds>"#;
        let c = parse_router_xml(xml).unwrap();
        assert_eq!(c.name, "svc");
        assert_eq!(c.routes.len(), 1);
        let r = &c.routes[0];
        assert_eq!(r.input.domain, 0);
        assert_eq!(r.output.domain, 2);
        assert_eq!(r.input.topic, "Square");
        assert_eq!(r.output.topic, "Circle");
        assert_eq!(r.input.type_name, "ShapeType");
    }

    /// What the parser does not read is refused with its line, and an
    /// unreadable boolean or count no longer becomes the default.
    #[test]
    fn unknown_elements_attributes_and_bad_values_are_refused() {
        let native = |route_attrs: &str, input_inner: &str| {
            format!(
                r#"<zerodds_routing><route name="a" {route_attrs}><input domain="0" topic="S">{input_inner}</input><output domain="1" topic="S"/></route></zerodds_routing>"#
            )
        };
        for (xml, needle) in [
            (native("lop_guard=\"true\"", ""), "attribute lop_guard"),
            (native("", "<qos reliable=\"maybe\"/>"), "not a boolean"),
            (native("max_samples_per_second=\"lots\"", ""), "not a count"),
            (native("", "<transform/>"), "element <transform>"),
            (
                r#"<dds><routing_service><administration/></routing_service></dds>"#.to_string(),
                "element <administration>",
            ),
        ] {
            let err = parse_router_xml(&xml).unwrap_err().to_string();
            assert!(err.contains(needle), "{needle}: {err}");
            assert!(err.contains("line 1"), "{err}");
        }
        let out_filter = r#"<zerodds_routing><route name="a"><input domain="0" topic="S"/><output domain="1" topic="S"><filter>x</filter></output></route></zerodds_routing>"#;
        assert!(
            parse_router_xml(out_filter).is_err(),
            "a filter on the output"
        );
    }

    #[test]
    fn rejects_unknown_root() {
        assert!(parse_router_xml("<foo/>").is_err());
    }
}

#[cfg(test)]
mod nesting_tests {
    use super::*;

    /// `roxmltree` recurses once per open element; a router configuration from outside
    /// with a long run of unclosed start tags is refused before it reaches
    /// the parser, instead of overflowing the stack.
    #[test]
    fn a_deeply_nested_text_is_refused_before_the_parser() {
        let text = "<a>".repeat(100_000);
        assert!(matches!(
            parse_router_xml(&text),
            Err(RoutingError::Config(_))
        ));
    }
}
