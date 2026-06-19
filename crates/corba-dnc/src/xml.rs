// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XML loader for D&C plan files — spec D&C §10 (XML encoding).
//!
//! Spec §10 specifies the XML schema (`Deployment.xsd`). We implement
//! a minimal, robust parser for the plan structures that callers
//! typically write:
//!
//! ```xml
//! <deploymentPlan>
//!   <label>Plan1</label>
//!   <UUID>uuid-1</UUID>
//!   <implementation>
//!     <label>EchoImpl</label>
//!     <UUID>echo</UUID>
//!     <artifact>lib/echo.so</artifact>
//!     <realizes>IDL:demo/Echo:1.0</realizes>
//!   </implementation>
//!   <instance>
//!     <name>echo1</name>
//!     <implementation>EchoImpl</implementation>
//!     <node>Node1</node>
//!     <coLocateWith>echo2</coLocateWith>
//!   </instance>
//! </deploymentPlan>
//! ```

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::plan::{
    DeploymentPlan, ImplementationDescription, InstanceDeploymentDescription, PlanConnection,
};

/// XML parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Expected tag not found.
    ExpectedTag(String),
    /// Tag is not closed.
    UnterminatedTag,
    /// Plan validation failed.
    ValidationFailed(String),
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ExpectedTag(t) => write!(f, "expected tag <{t}>"),
            Self::UnterminatedTag => f.write_str("unterminated tag"),
            Self::ValidationFailed(msg) => write!(f, "validation: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Parses a D&C `<deploymentPlan>` XML into a [`DeploymentPlan`].
///
/// The parser is strictly sequential and expects the spec order
/// `<label>` → `<UUID>` → `<realizes>?` → `<implementation>*` →
/// `<instance>*` → `<connection>*`. Whitespace is ignored.
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_plan_xml(input: &str) -> Result<DeploymentPlan, ParseError> {
    let body = inner_text(input, "deploymentPlan")?;
    let label = inner_text(&body, "label").unwrap_or_default();
    let uuid = inner_text(&body, "UUID").unwrap_or_default();
    let realizes = inner_text(&body, "realizes").unwrap_or_default();

    let mut plan = DeploymentPlan {
        label,
        uuid,
        realizes,
        ..DeploymentPlan::default()
    };

    // Spec §10 — the plan body has `<implementation>`, `<instance>`,
    // and `<connection>` as top-level children. Since `<instance>`
    // itself carries an `<implementation>` leaf element (a reference to
    // an ImplementationDescription by label), we parse instances first
    // and strip their blocks from the body before looking for further
    // top-level implementations.
    let instance_blocks = iter_blocks(&body, "instance");
    let connection_blocks = iter_blocks(&body, "connection");
    let body_without_instances = strip_blocks(&body, "instance");
    let body_without_inst_conn = strip_blocks(&body_without_instances, "connection");

    for impl_block in iter_blocks(&body_without_inst_conn, "implementation") {
        plan.implementations.push(parse_implementation(&impl_block));
    }
    for inst_block in instance_blocks {
        plan.instances.push(parse_instance(&inst_block));
    }
    for conn_block in connection_blocks {
        plan.connections.push(parse_connection(&conn_block));
    }

    plan.validate()
        .map_err(|e| ParseError::ValidationFailed(format_err(&e)))?;
    Ok(plan)
}

fn strip_blocks(input: &str, tag: &str) -> String {
    let open = alloc::format!("<{tag}>");
    let close = alloc::format!("</{tag}>");
    let mut out = String::new();
    let mut cursor = 0;
    while let Some(s) = input[cursor..].find(&open) {
        let start = cursor + s;
        out.push_str(&input[cursor..start]);
        if let Some(e) = input[start..].find(&close) {
            cursor = start + e + close.len();
        } else {
            cursor = input.len();
            break;
        }
    }
    out.push_str(&input[cursor..]);
    out
}

fn parse_implementation(block: &str) -> ImplementationDescription {
    let label = inner_text(block, "label").unwrap_or_default();
    let uuid = inner_text(block, "UUID").unwrap_or_default();
    let realizes = inner_text(block, "realizes").unwrap_or_default();
    let artifacts: Vec<String> = collect_inner_texts(block, "artifact");
    ImplementationDescription {
        label,
        uuid,
        artifacts,
        realizes,
        exec_params: BTreeMap::new(),
        depends_on: Vec::new(),
    }
}

fn parse_instance(block: &str) -> InstanceDeploymentDescription {
    let name = inner_text(block, "name").unwrap_or_default();
    let implementation = inner_text(block, "implementation").unwrap_or_default();
    let node = inner_text(block, "node").unwrap_or_default();
    let co_locate_with = collect_inner_texts(block, "coLocateWith");
    InstanceDeploymentDescription {
        name,
        implementation,
        node,
        config_props: BTreeMap::new(),
        co_locate_with,
    }
}

fn parse_connection(block: &str) -> PlanConnection {
    PlanConnection {
        name: inner_text(block, "name").unwrap_or_default(),
        source_instance: inner_text(block, "sourceInstance").unwrap_or_default(),
        source_port: inner_text(block, "sourcePort").unwrap_or_default(),
        target_instance: inner_text(block, "targetInstance").unwrap_or_default(),
        target_port: inner_text(block, "targetPort").unwrap_or_default(),
    }
}

fn inner_text(input: &str, tag: &str) -> Result<String, ParseError> {
    let open = alloc::format!("<{tag}>");
    let close = alloc::format!("</{tag}>");
    let start = input
        .find(&open)
        .ok_or_else(|| ParseError::ExpectedTag(tag.to_string()))?
        + open.len();
    let end = input[start..]
        .find(&close)
        .ok_or(ParseError::UnterminatedTag)?
        + start;
    Ok(input[start..end].trim().to_string())
}

fn collect_inner_texts(input: &str, tag: &str) -> Vec<String> {
    let open = alloc::format!("<{tag}>");
    let close = alloc::format!("</{tag}>");
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(s) = input[cursor..].find(&open) {
        let start = cursor + s + open.len();
        if let Some(e) = input[start..].find(&close) {
            let end = start + e;
            out.push(input[start..end].trim().to_string());
            cursor = end + close.len();
        } else {
            break;
        }
    }
    out
}

fn iter_blocks(input: &str, tag: &str) -> Vec<String> {
    let open = alloc::format!("<{tag}>");
    let close = alloc::format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(s) = input[cursor..].find(&open) {
        let start = cursor + s + open.len();
        if let Some(e) = input[start..].find(&close) {
            let end = start + e;
            blocks.push(input[start..end].to_string());
            cursor = end + close.len();
        } else {
            break;
        }
    }
    blocks
}

fn format_err(e: &crate::plan::PlanError) -> String {
    alloc::format!("{e}")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
<deploymentPlan>
  <label>Plan1</label>
  <UUID>uuid-1</UUID>
  <realizes>RootApp</realizes>
  <implementation>
    <label>EchoImpl</label>
    <UUID>echo-uuid</UUID>
    <artifact>lib/echo.so</artifact>
    <realizes>IDL:demo/Echo:1.0</realizes>
  </implementation>
  <instance>
    <name>echo1</name>
    <implementation>EchoImpl</implementation>
    <node>Node1</node>
  </instance>
</deploymentPlan>
"#;

    #[test]
    fn parses_minimal_plan() {
        let plan = parse_plan_xml(SAMPLE).unwrap();
        assert_eq!(plan.label, "Plan1");
        assert_eq!(plan.uuid, "uuid-1");
        assert_eq!(plan.implementations.len(), 1);
        assert_eq!(
            plan.implementations[0].artifacts,
            alloc::vec!["lib/echo.so".to_string()]
        );
        assert_eq!(plan.instances.len(), 1);
        assert_eq!(plan.instances[0].node, "Node1");
    }

    #[test]
    fn parses_multiple_artifacts() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <implementation>
    <label>I1</label>
    <UUID>i1</UUID>
    <artifact>a.so</artifact>
    <artifact>b.so</artifact>
    <realizes>IDL:X:1.0</realizes>
  </implementation>
</deploymentPlan>
"#;
        let plan = parse_plan_xml(xml).unwrap();
        assert_eq!(plan.implementations[0].artifacts.len(), 2);
    }

    #[test]
    fn validation_fails_on_unknown_impl_reference() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <instance>
    <name>x</name>
    <implementation>NoSuchImpl</implementation>
    <node>N</node>
  </instance>
</deploymentPlan>
"#;
        let err = parse_plan_xml(xml).unwrap_err();
        match err {
            ParseError::ValidationFailed(_) => {}
            e => panic!("unexpected error variant: {e}"),
        }
    }

    #[test]
    fn missing_root_tag_returns_expected_tag() {
        let err = parse_plan_xml("<empty/>").unwrap_err();
        assert_eq!(err, ParseError::ExpectedTag("deploymentPlan".into()));
    }

    #[test]
    fn co_locate_with_parsed() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <implementation>
    <label>I</label>
    <UUID>i</UUID>
    <artifact>a.so</artifact>
    <realizes>IDL:X:1.0</realizes>
  </implementation>
  <instance>
    <name>a</name>
    <implementation>I</implementation>
    <node>N</node>
  </instance>
  <instance>
    <name>b</name>
    <implementation>I</implementation>
    <node>N</node>
    <coLocateWith>a</coLocateWith>
  </instance>
</deploymentPlan>
"#;
        let plan = parse_plan_xml(xml).unwrap();
        assert_eq!(
            plan.instances[1].co_locate_with,
            alloc::vec!["a".to_string()]
        );
    }
}
