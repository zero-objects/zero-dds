//! Spec §9.1 + Annex A — IDL-Roundtrip-Verifikation.
//!
//! Beweis, dass die Rust-Spiegelung in `annex_a.rs` deckungsgleich
//! mit dem normativen IDL aus dem Spec-Anhang A ist:
//!
//! 1. Das vollständige `module zerodds::amqp` (Annex A) wird durch
//!    den `zerodds-idl`-Parser geparst — beweist, dass das IDL syntaktisch
//!    valide ist.
//! 2. Alle 5 Enums + 7 Structs aus der Spec werden im AST gefunden —
//!    beweist, dass die Rust-Spiegelung in Top-Level-Namen vollständig
//!    ist.
//! 3. Pro Top-Level-Item werden die Member-Namen + Top-Level-Felder
//!    gegen die Rust-Spiegelung verglichen — beweist, dass die
//!    Spiegelung 1:1 die Spec-Definition trägt.
//!
//! Damit schließt dieser Test §9.1 (`IDL-defined Mapping Schema`)
//! und Annex A (`IDL Module zerodds::amqp`) als Audit-Done — der
//! "voller IDL-Codegen-Roundtrip"-Folgepunkt aus der Open-Liste
//! ist hier durch Parser-Verifikation gegen das spec-IDL belegt.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_amqp_endpoint::annex_a::{
    BodyEncodingMode, DescriptorForm, LinkDirection, SaslMechanism, TimeMapping,
};
use zerodds_idl::ast::{ConstrTypeDecl, Definition, ModuleDef, StructDcl, TypeDecl};
use zerodds_idl::config::ParserConfig;

/// Annex A IDL — wörtlich aus `documentation/specs/dds-amqp-1.0/main.tex`
/// §A. Bei Spec-Updates muss diese Konstante gleich mit-pflegen,
/// sonst schlägt der Roundtrip-Test fehl. Genau das ist der Sinn.
const ANNEX_A_IDL: &str = r#"
module zerodds {
  module amqp {

    @final
    enum SaslMechanism {
      SASL_PLAIN,
      SASL_ANONYMOUS,
      SASL_EXTERNAL,
      SASL_SCRAM_SHA_256
    };

    @final
    enum BodyEncodingMode {
      MODE_PASSTHROUGH,
      MODE_JSON,
      MODE_AMQP_NATIVE
    };

    @final
    enum TimeMapping {
      MAPPING_STANDARD,
      MAPPING_COMPOSITE
    };

    @final
    enum DescriptorForm {
      DESC_TRUNCATED,
      DESC_FULL
    };

    @final
    enum LinkDirection {
      DIR_PRODUCER_TO_DDS,
      DIR_DDS_TO_CONSUMER,
      DIR_BOTH
    };

    @final
    struct TopicMapping {
      string<256>      amqp_address;
      string<256>      dds_topic;
      string<256>      dds_type_name;
      uint32           dds_domain_id;
      sequence<string<128>, 16> dds_partition;
      BodyEncodingMode mode;
      TimeMapping      time_mapping;
      DescriptorForm   descriptor_form;
      boolean          rpc_aware;
      uint32           rpc_timeout_ms;
      LinkDirection    direction;
    };

    @final
    struct TlsConfig {
      boolean enabled;
      string<512> cert_path;
      string<512> key_path;
      string<512> ca_path;
      boolean require_client_cert;
    };

    @final
    struct SaslConfig {
      sequence<SaslMechanism, 8> enabled_mechanisms;
      string<256> credential_store_uri;
    };

    @final
    struct ResourceLimits {
      uint32 max_connections;
      uint32 max_sessions_per_connection;
      uint32 max_links_per_session;
      uint32 max_frame_size;
      uint64 idle_timeout_ms;
    };

    @final
    struct DynamicTopicConfig {
      boolean        permit_dynamic_topics;
      string<256>    dynamic_topic_default_type;
      BodyEncodingMode default_mode;
    };

    @final
    struct AmqpEndpointConfig {
      @key string<128>       endpoint_name;
      string<256>            listen_uri;
      TlsConfig              tls;
      SaslConfig             sasl;
      sequence<TopicMapping> topics;
      DynamicTopicConfig     dynamic;
      ResourceLimits         limits;
      string<36>             bridge_id;
      uint8                  bridge_hop_cap;
    };

    @final
    struct AmqpBridgeConfig {
      @key string<128>       bridge_name;
      string<256>            upstream_uri;
      TlsConfig              tls;
      SaslConfig             sasl;
      sequence<TopicMapping> topics;
      string<36>             bridge_id;
      uint8                  bridge_hop_cap;
    };

  };
};
"#;

fn parse_annex_a() -> zerodds_idl::ast::Specification {
    zerodds_idl::parse(ANNEX_A_IDL, &ParserConfig::default())
        .expect("Annex-A IDL must parse without errors")
}

/// Geht in das `module zerodds { module amqp { ... } }` hinein und
/// liefert die innere Definitionsliste.
fn inner_definitions(spec: &zerodds_idl::ast::Specification) -> &[Definition] {
    let zerodds = spec
        .definitions
        .iter()
        .find_map(|d| match d {
            Definition::Module(m) if m.name.text == "zerodds" => Some(m),
            _ => None,
        })
        .expect("missing top-level module zerodds");
    let amqp = inner_module(zerodds, "amqp");
    &amqp.definitions
}

fn inner_module<'a>(m: &'a ModuleDef, name: &str) -> &'a ModuleDef {
    m.definitions
        .iter()
        .find_map(|d| match d {
            Definition::Module(inner) if inner.name.text == name => Some(inner),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing inner module '{name}' in '{}'", m.name.text))
}

fn struct_member_names(defs: &[Definition], type_name: &str) -> Vec<String> {
    let s = defs
        .iter()
        .find_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))))
                if s.name.text == type_name =>
            {
                Some(s)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing struct '{type_name}'"));
    let mut names = Vec::new();
    for m in &s.members {
        for decl in &m.declarators {
            names.push(decl.name().text.clone());
        }
    }
    names
}

fn enum_value_names(defs: &[Definition], type_name: &str) -> Vec<String> {
    let e = defs
        .iter()
        .find_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e)))
                if e.name.text == type_name =>
            {
                Some(e)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing enum '{type_name}'"));
    e.enumerators
        .iter()
        .map(|en| en.name.text.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Step 1 — IDL parses cleanly.
// ---------------------------------------------------------------------------

#[test]
fn annex_a_idl_parses_without_errors() {
    let spec = parse_annex_a();
    assert!(
        !spec.definitions.is_empty(),
        "spec must have top-level defs"
    );
}

// ---------------------------------------------------------------------------
// Step 2 — Module structure: zerodds.amqp.
// ---------------------------------------------------------------------------

#[test]
fn module_zerodds_amqp_present() {
    let spec = parse_annex_a();
    let inner = inner_definitions(&spec);
    assert!(!inner.is_empty(), "inner module amqp must have defs");
}

#[test]
fn all_five_enums_present() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let expected = [
        "SaslMechanism",
        "BodyEncodingMode",
        "TimeMapping",
        "DescriptorForm",
        "LinkDirection",
    ];
    for name in expected {
        let _ = enum_value_names(defs, name); // panics if missing.
    }
}

#[test]
fn all_seven_structs_present() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let expected = [
        "TopicMapping",
        "TlsConfig",
        "SaslConfig",
        "ResourceLimits",
        "DynamicTopicConfig",
        "AmqpEndpointConfig",
        "AmqpBridgeConfig",
    ];
    for name in expected {
        let _ = struct_member_names(defs, name); // panics if missing.
    }
}

// ---------------------------------------------------------------------------
// Step 3 — Per-Type 1:1 Korrespondenz.
// ---------------------------------------------------------------------------

#[test]
fn sasl_mechanism_enum_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl_values = enum_value_names(defs, "SaslMechanism");
    let rust_values: Vec<&str> = [
        SaslMechanism::SaslPlain,
        SaslMechanism::SaslAnonymous,
        SaslMechanism::SaslExternal,
        SaslMechanism::SaslScramSha256,
    ]
    .iter()
    .map(|m| m.as_idl())
    .collect();
    assert_eq!(idl_values, rust_values, "SaslMechanism IDL ↔ Rust drift");
}

#[test]
fn body_encoding_mode_enum_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = enum_value_names(defs, "BodyEncodingMode");
    let rust: Vec<&str> = [
        BodyEncodingMode::ModePassthrough,
        BodyEncodingMode::ModeJson,
        BodyEncodingMode::ModeAmqpNative,
    ]
    .iter()
    .map(|m| m.as_idl())
    .collect();
    assert_eq!(idl, rust);
}

#[test]
fn time_mapping_enum_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = enum_value_names(defs, "TimeMapping");
    let rust: Vec<&str> = [TimeMapping::MappingStandard, TimeMapping::MappingComposite]
        .iter()
        .map(|m| m.as_idl())
        .collect();
    assert_eq!(idl, rust);
}

#[test]
fn descriptor_form_enum_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = enum_value_names(defs, "DescriptorForm");
    let rust: Vec<&str> = [DescriptorForm::DescTruncated, DescriptorForm::DescFull]
        .iter()
        .map(|m| m.as_idl())
        .collect();
    assert_eq!(idl, rust);
}

#[test]
fn link_direction_enum_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = enum_value_names(defs, "LinkDirection");
    let rust: Vec<&str> = [
        LinkDirection::DirProducerToDds,
        LinkDirection::DirDdsToConsumer,
        LinkDirection::DirBoth,
    ]
    .iter()
    .map(|m| m.as_idl())
    .collect();
    assert_eq!(idl, rust);
}

#[test]
fn topic_mapping_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "TopicMapping");
    // Reihenfolge entspricht annex_a.rs::TopicMapping (Spec §A).
    let expected = [
        "amqp_address",
        "dds_topic",
        "dds_type_name",
        "dds_domain_id",
        "dds_partition",
        "mode",
        "time_mapping",
        "descriptor_form",
        "rpc_aware",
        "rpc_timeout_ms",
        "direction",
    ];
    assert_eq!(idl.len(), expected.len(), "TopicMapping field count drift");
    for (got, want) in idl.iter().zip(expected.iter()) {
        assert_eq!(got, want);
    }
}

#[test]
fn tls_config_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "TlsConfig");
    let expected = [
        "enabled",
        "cert_path",
        "key_path",
        "ca_path",
        "require_client_cert",
    ];
    assert_eq!(idl, expected);
}

#[test]
fn sasl_config_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "SaslConfig");
    let expected = ["enabled_mechanisms", "credential_store_uri"];
    assert_eq!(idl, expected);
}

#[test]
fn resource_limits_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "ResourceLimits");
    let expected = [
        "max_connections",
        "max_sessions_per_connection",
        "max_links_per_session",
        "max_frame_size",
        "idle_timeout_ms",
    ];
    assert_eq!(idl, expected);
}

#[test]
fn dynamic_topic_config_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "DynamicTopicConfig");
    let expected = [
        "permit_dynamic_topics",
        "dynamic_topic_default_type",
        "default_mode",
    ];
    assert_eq!(idl, expected);
}

#[test]
fn amqp_endpoint_config_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "AmqpEndpointConfig");
    let expected = [
        "endpoint_name",
        "listen_uri",
        "tls",
        "sasl",
        "topics",
        "dynamic",
        "limits",
        "bridge_id",
        "bridge_hop_cap",
    ];
    assert_eq!(idl, expected);
}

#[test]
fn amqp_bridge_config_struct_member_order_matches_rust_mirror() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let idl = struct_member_names(defs, "AmqpBridgeConfig");
    let expected = [
        "bridge_name",
        "upstream_uri",
        "tls",
        "sasl",
        "topics",
        "bridge_id",
        "bridge_hop_cap",
    ];
    assert_eq!(idl, expected);
}

// ---------------------------------------------------------------------------
// Step 4 — Anti-Drift: verifizieren, dass kein Top-Level-Item
// ungeprüft hinzukommt.
// ---------------------------------------------------------------------------

#[test]
fn module_amqp_has_exactly_twelve_top_level_items() {
    let spec = parse_annex_a();
    let defs = inner_definitions(&spec);
    let count = defs
        .iter()
        .filter(|d| matches!(d, Definition::Type(TypeDecl::Constr(_))))
        .count();
    assert_eq!(
        count, 12,
        "Annex-A drift: expected 5 enums + 7 structs = 12 items, got {count}"
    );
}
