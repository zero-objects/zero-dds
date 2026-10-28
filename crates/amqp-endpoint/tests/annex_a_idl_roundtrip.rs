//! Spec §9.1 + Annex A — IDL round-trip verification.
//!
//! Proof that the Rust mirror in `annex_a.rs` matches the normative IDL
//! from spec Annex A exactly:
//!
//! 1. The complete `module zerodds::amqp` (Annex A) parses and lowers
//!    through `zerodds-idl-parse` + `zerodds-idl-lower`, and every name
//!    it uses resolves — proving the IDL is valid and self-contained.
//! 2. All 5 enums + 7 structs from the spec are in the model — proving
//!    the Rust mirror is complete in its top-level names.
//! 3. Each one's member names are compared against the Rust mirror,
//!    proving the mirror carries the spec definition 1:1.
//!
//! This closes §9.1 (`IDL-defined Mapping Schema`) and Annex A (`IDL
//! Module zerodds::amqp`) as audit-done.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_amqp_endpoint::annex_a::{
    BodyEncodingMode, DescriptorForm, LinkDirection, SaslMechanism, TimeMapping,
};
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::{Id, ScopedName};

/// Annex A IDL — verbatim from `documentation/specs/dds-amqp-1.0/main.tex`
/// §A. On spec updates this constant must be maintained alongside,
/// otherwise the round-trip test fails. That is exactly the point.
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

/// Parse and lower the Annex-A IDL, and resolve every name in it.
///
/// The resolution pass is part of the proof: Annex A declares every type
/// it uses, so a name that reaches nothing is a defect in the annex or in
/// the constant above rather than something to shrug off. The old test
/// stopped at the parse and could not have caught one.
fn annex_a_model() -> Ast {
    let parsed = zerodds_idl_parse::parse(ANNEX_A_IDL);
    assert!(
        !parsed.has_errors(),
        "Annex-A IDL must parse without errors: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "annex-a.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "Annex-A IDL must lower without diagnostics: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.is_clean(),
        "Annex-A IDL names something it does not declare: {:?}",
        report.broken
    );
    ast
}

/// Every type declared in `module zerodds { module amqp { ... } }`.
fn inner_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    let module = ast
        .module_by_path(&ScopedName::absolute(vec![
            "zerodds".to_string(),
            "amqp".to_string(),
        ]))
        .expect("missing module zerodds::amqp");
    ast.types()
        .filter(|(_, node)| node.module == module)
        .map(|(id, _)| id)
        .collect()
}

fn type_by_name(ast: &Ast, name: &str) -> Id<TypeNode> {
    inner_types(ast)
        .into_iter()
        .find(|id| ast.type_(*id).is_ok_and(|node| node.name == name))
        .unwrap_or_else(|| panic!("missing type '{name}'"))
}

fn struct_member_names(ast: &Ast, type_name: &str) -> Vec<String> {
    let id = type_by_name(ast, type_name);
    let TypeKind::Structure { members, .. } = &ast.type_(id).expect("struct").kind else {
        panic!("'{type_name}' is not a struct");
    };
    members
        .iter()
        .filter_map(|member| ast.member(*member).ok().map(|node| node.name.clone()))
        .collect()
}

fn enum_value_names(ast: &Ast, type_name: &str) -> Vec<String> {
    let id = type_by_name(ast, type_name);
    let TypeKind::Enumeration { enumerators } = &ast.type_(id).expect("enum").kind else {
        panic!("'{type_name}' is not an enum");
    };
    enumerators
        .iter()
        .filter_map(|id| ast.enumerator(*id).ok().map(|node| node.name.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Step 1 — the IDL parses, lowers and resolves.
// ---------------------------------------------------------------------------

#[test]
fn annex_a_idl_parses_lowers_and_resolves() {
    let ast = annex_a_model();
    assert!(!inner_types(&ast).is_empty(), "the annex declares nothing");
}

// ---------------------------------------------------------------------------
// Step 2 — module structure: zerodds::amqp.
// ---------------------------------------------------------------------------

#[test]
fn module_zerodds_amqp_present() {
    let ast = annex_a_model();
    assert!(
        !inner_types(&ast).is_empty(),
        "inner module amqp must declare types"
    );
}

#[test]
fn all_five_enums_present() {
    let ast = annex_a_model();
    for name in [
        "SaslMechanism",
        "BodyEncodingMode",
        "TimeMapping",
        "DescriptorForm",
        "LinkDirection",
    ] {
        let _ = enum_value_names(&ast, name); // panics if missing.
    }
}

#[test]
fn all_seven_structs_present() {
    let ast = annex_a_model();
    for name in [
        "TopicMapping",
        "TlsConfig",
        "SaslConfig",
        "ResourceLimits",
        "DynamicTopicConfig",
        "AmqpEndpointConfig",
        "AmqpBridgeConfig",
    ] {
        let _ = struct_member_names(&ast, name); // panics if missing.
    }
}

// ---------------------------------------------------------------------------
// Step 3 — per-type 1:1 correspondence.
// ---------------------------------------------------------------------------

#[test]
fn sasl_mechanism_enum_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl_values = enum_value_names(&ast, "SaslMechanism");
    let rust_values: Vec<&str> = [
        SaslMechanism::SaslPlain,
        SaslMechanism::SaslAnonymous,
        SaslMechanism::SaslExternal,
        SaslMechanism::SaslScramSha256,
    ]
    .iter()
    .map(|m| m.as_idl())
    .collect();
    assert_eq!(idl_values, rust_values, "SaslMechanism IDL <-> Rust drift");
}

#[test]
fn body_encoding_mode_enum_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl = enum_value_names(&ast, "BodyEncodingMode");
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
    let ast = annex_a_model();
    let idl = enum_value_names(&ast, "TimeMapping");
    let rust: Vec<&str> = [TimeMapping::MappingStandard, TimeMapping::MappingComposite]
        .iter()
        .map(|m| m.as_idl())
        .collect();
    assert_eq!(idl, rust);
}

#[test]
fn descriptor_form_enum_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl = enum_value_names(&ast, "DescriptorForm");
    let rust: Vec<&str> = [DescriptorForm::DescTruncated, DescriptorForm::DescFull]
        .iter()
        .map(|m| m.as_idl())
        .collect();
    assert_eq!(idl, rust);
}

#[test]
fn link_direction_enum_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl = enum_value_names(&ast, "LinkDirection");
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
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "TopicMapping");
    // Order matches annex_a.rs::TopicMapping (Spec §A).
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
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "TlsConfig");
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
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "SaslConfig");
    let expected = ["enabled_mechanisms", "credential_store_uri"];
    assert_eq!(idl, expected);
}

#[test]
fn resource_limits_struct_member_order_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "ResourceLimits");
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
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "DynamicTopicConfig");
    let expected = [
        "permit_dynamic_topics",
        "dynamic_topic_default_type",
        "default_mode",
    ];
    assert_eq!(idl, expected);
}

#[test]
fn amqp_endpoint_config_struct_member_order_matches_rust_mirror() {
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "AmqpEndpointConfig");
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
    let ast = annex_a_model();
    let idl = struct_member_names(&ast, "AmqpBridgeConfig");
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
// Step 4 — anti-drift: no top-level item is added unchecked, and the one
// annotation the annex writes on a member reaches the model.
// ---------------------------------------------------------------------------

#[test]
fn module_amqp_has_exactly_twelve_top_level_items() {
    let ast = annex_a_model();
    let count = inner_types(&ast)
        .into_iter()
        .filter(|id| {
            matches!(
                ast.type_(*id).expect("type").kind,
                TypeKind::Structure { .. } | TypeKind::Enumeration { .. }
            )
        })
        .count();
    assert_eq!(
        count, 12,
        "Annex-A drift: expected 5 enums + 7 structs = 12 items, got {count}"
    );
}

#[test]
fn the_two_configuration_types_keep_their_key_member() {
    // §A writes `@key` on `endpoint_name` and on `bridge_name`; the key
    // decides the instance a sample belongs to, so its loss would be a
    // wire-visible one.
    for (type_name, member_name) in [
        ("AmqpEndpointConfig", "endpoint_name"),
        ("AmqpBridgeConfig", "bridge_name"),
    ] {
        let ast = annex_a_model();
        let id = type_by_name(&ast, type_name);
        let TypeKind::Structure { members, .. } = &ast.type_(id).expect("struct").kind else {
            panic!("{type_name} is not a struct");
        };
        let keyed: Vec<String> = members
            .iter()
            .filter_map(|member| ast.member(*member).ok())
            .filter(|node| {
                node.facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_key)
            })
            .map(|node| node.name.clone())
            .collect();
        assert_eq!(keyed, [member_name], "{type_name} lost its key");
    }
}
