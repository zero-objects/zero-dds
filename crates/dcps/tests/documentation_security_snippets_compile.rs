// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Compile + smoke test for the `documentation/**` fences that need the
//! `security` feature — split out from `documentation_snippets_compile.rs`
//! (which is deliberately feature-ungated so it runs in the default
//! `cargo test --workspace` job) rather than gating that whole file, the
//! way `website_snippets_compile.rs` does.
//!
//! **CI status: wired.** The `test:` job in `.gitlab-ci.yml` runs
//! `cargo test -p zerodds-dcps --features security --test
//! documentation_security_snippets_compile -- --test-threads=1` (D.3,
//! mirroring the existing `--features same-host-shm` / `--features
//! delivery-iceoryx` steps). Before that step existed, no
//! `.gitlab-ci.yml` job passed `--features security` (or `--all-features`)
//! to `cargo test`, so this file compiled + ran nowhere — the same gap
//! `website_snippets_compile.rs` had (see that file's doc-comment / the
//! commit that introduced `documentation_snippets_compile.rs`, d3cf0200).
//! Local run: `cargo test -p zerodds-dcps --features security --test
//! documentation_security_snippets_compile`.
//!
//! Convention otherwise identical to `documentation_snippets_compile.rs`:
//! one function per fence, doc-commented with the exact `documentation/**`
//! path + line, no live sockets touched (`DcpsRuntime::start` is not
//! called here — the live proof is the `security-governance-gate`
//! companion's `cargo run`).

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

/// `documentation/03-configuration/security.md` line 151 — "Code: wiring
/// up the gate". `RuntimeConfig.security` only exists behind
/// `#[cfg(feature = "security")]` (`crates/dcps/src/runtime.rs`), so this
/// fence cannot compile in `documentation_snippets_compile.rs`. Companion:
/// `zerodds-examples/security-governance-gate` (parses the page's own
/// Governance XML example and starts a real `DcpsRuntime` with the gate
/// wired, live).
#[test]
fn security_governance_gate_snippet() {
    use std::sync::Arc;

    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;
    use zerodds_security_runtime::SharedSecurityGate;

    // Same governance XML as the page's own "Governance XML" section
    // (lines 69-97 — encrypt everything on domain 0).
    let governance = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="omg_shared_ca_governance.xsd">
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id>0</id>
      </domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>TRUE</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</discovery_protection_kind>
      <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>TRUE</enable_discovery_protection>
          <enable_liveliness_protection>TRUE</enable_liveliness_protection>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#;

    // Doc fence, verbatim (minus reading `governance.xml` from disk):
    let gov = parse_governance_xml(governance).expect("parse governance xml");
    let crypto = Box::new(AesGcmCryptoPlugin::new());

    let gate = Arc::new(SharedSecurityGate::new(0, gov, crypto));

    let cfg = RuntimeConfig {
        security: Some(gate),
        ..Default::default()
    };
    assert!(cfg.security.is_some());
}
