//! Small generation contract for the language consumers selected by the CI DAG.
//! This does not claim foreign compilation or runtime interoperability.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stdout
)]

use zerodds_lang_cst::Language;

#[test]
fn affected_languages_generate_a_codec() {
    let requested = std::env::var("MERGE_CODEGEN_LANGUAGES").ok();
    let available: Vec<_> = Language::ALL
        .into_iter()
        .map(|language| {
            let name = format!("{language:?}").to_lowercase();
            let name = if name == "ada83" {
                "ada-83".to_owned()
            } else {
                name
            };
            (name, language)
        })
        .collect();
    let names: Vec<_> = requested.as_ref().map_or_else(
        || available.iter().map(|(name, _)| name.as_str()).collect(),
        |value| value.split(',').collect(),
    );
    assert!(!names.is_empty());
    for name in &names {
        assert!(
            available.iter().any(|(known, _)| known == name),
            "unknown language: {name}"
        );
    }
    let parsed = zerodds_idl_parse::parse(
        "module m { @appendable struct Reading { long id; double value; string label; }; };",
    );
    assert!(parsed.diagnostics().is_empty());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dependency-smoke.idl");
    assert!(lowered.diagnostics.is_empty());
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    for name in names {
        let (_, language) = available
            .iter()
            .find(|(known, _)| known == name)
            .expect("validated language");
        let text = zerodds_lang_rules::generate::source(&ast, *language, key)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(text.contains("Reading"), "{name}: missing type");
        let encoder = match language {
            Language::Rust => "fn encode",
            Language::Go => "Zd_enc_",
            _ => "zd_enc_",
        };
        assert!(text.contains(encoder), "{name}: missing encoder");
        println!("generation contract passed: {name}");
    }
}
