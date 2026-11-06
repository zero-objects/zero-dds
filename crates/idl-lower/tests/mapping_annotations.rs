// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The annotations the IDL4 language mappings define for themselves:
//! `@java_mapping` (IDL4 to Java 1.0 §8.1), `@csharp_mapping` (IDL4 to
//! C# 1.0 §8.1) and `@cpp_mapping` (IDL4 to C++ 1.0 §8.1), in the IDL
//! their clauses declare them with.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{AnyId, Ast, MappingLanguage, MappingSetting, MappingValue};
use zerodds_idl_parse::Diagnostic;

fn lowered(source: &str) -> (Ast, Vec<Diagnostic>) {
    let parsed = zerodds_idl_parse::parse(source);
    let mut diagnostics = parsed.diagnostics().to_vec();
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    diagnostics.extend(lowered.diagnostics);
    (lowered.ast, diagnostics)
}

fn module_settings(ast: &Ast, name: &str) -> Vec<MappingSetting> {
    let (id, node) = ast
        .modules()
        .find(|(_, module)| module.name == name)
        .unwrap_or_else(|| panic!("module {name}"));
    ast.facets(AnyId::Module(id, node.node_id))
        .expect("the module's facets")
        .mappings
        .clone()
}

fn setting(language: MappingLanguage, parameter: &str, value: MappingValue) -> MappingSetting {
    MappingSetting {
        language,
        parameter: parameter.to_string(),
        value,
    }
}

/// §8.1's declaration: every parameter, by name, reaches the model in the
/// kind it is declared with — the clause's own examples among them.
#[test]
fn idl4java_8_1_java_mapping_reaches_the_model_parameter_by_parameter() {
    let (ast, found) = lowered(
        "@java_mapping(apply_naming_convention = JAVA_NAMING_CONVENTION,
                       constants_container = \"MathematicalConstants\",
                       promote_integer_width = TRUE,
                       string_type = \"java.lang.StringBuilder\")
         module MY_MATH { const double PI = 3.141592; };",
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(
        module_settings(&ast, "MY_MATH"),
        vec![
            setting(
                MappingLanguage::Java,
                "apply_naming_convention",
                MappingValue::Name("JAVA_NAMING_CONVENTION".into())
            ),
            setting(
                MappingLanguage::Java,
                "constants_container",
                MappingValue::Text("MathematicalConstants".into())
            ),
            setting(
                MappingLanguage::Java,
                "promote_integer_width",
                MappingValue::Flag(true)
            ),
            setting(
                MappingLanguage::Java,
                "string_type",
                MappingValue::Text("java.lang.StringBuilder".into())
            ),
        ]
    );
}

/// A parameter the declaration does not have, an enumerator its
/// enumeration does not have, a value of the wrong kind and the
/// positional form are errors (IDL 4.2 §7.4.15.4.2 holds an application to
/// the declaration).
#[test]
fn idl4java_8_1_an_application_that_does_not_fit_the_declaration_is_an_error() {
    for source in [
        "@java_mapping(naming = JAVA_NAMING_CONVENTION) module M { };",
        "@java_mapping(apply_naming_convention = DOTNET_NAMING_CONVENTION) module M { };",
        "@java_mapping(promote_integer_width = \"yes\") module M { };",
        "@java_mapping(constants_container = Constants) module M { };",
        "@java_mapping(JAVA_NAMING_CONVENTION) module M { };",
    ] {
        let (_, found) = lowered(source);
        assert!(
            found.iter().any(|d| d.code.as_str() == "IDL0007"),
            "{source}: {found:?}"
        );
    }
}

/// IDL4 to C# 1.0 §8.1 and IDL4 to C++ 1.0 §8.1, the same way.
#[test]
fn idl4csharp_8_1_and_idl4cpp_8_1_their_annotations_reach_the_model() {
    let (ast, found) = lowered(
        "@csharp_mapping(apply_naming_convention = DOTNET_NAMING_CONVENTION,
                         constants_container = \"Values\", struct_type = \"struct\")
         module A { };
         @cpp_mapping(struct_mapping = CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS)
         module B { };",
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(
        module_settings(&ast, "A"),
        vec![
            setting(
                MappingLanguage::CSharp,
                "apply_naming_convention",
                MappingValue::Name("DOTNET_NAMING_CONVENTION".into())
            ),
            setting(
                MappingLanguage::CSharp,
                "constants_container",
                MappingValue::Text("Values".into())
            ),
            setting(
                MappingLanguage::CSharp,
                "struct_type",
                MappingValue::Text("struct".into())
            ),
        ]
    );
    assert_eq!(
        module_settings(&ast, "B"),
        vec![setting(
            MappingLanguage::Cpp,
            "struct_mapping",
            MappingValue::Name("CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS".into())
        )]
    );
    let (_, found) = lowered("@cpp_mapping(struct_mapping = CLASS) struct S { long a; };");
    assert!(
        found.iter().any(|d| d.code.as_str() == "IDL0007"),
        "{found:?}"
    );
}

/// The settings survive a snapshot: a generator that read a model back
/// without them would map with the defaults.
#[test]
fn idl4java_8_1_the_settings_survive_a_snapshot() {
    let (ast, found) =
        lowered("@java_mapping(apply_naming_convention = JAVA_NAMING_CONVENTION) module M { };");
    assert!(found.is_empty(), "{found:?}");
    let bytes = ast.to_snapshot().expect("a snapshot");
    let back = Ast::from_snapshot(&bytes).expect("the snapshot reads back");
    assert_eq!(module_settings(&back, "M"), module_settings(&ast, "M"));
    assert_eq!(module_settings(&back, "M").len(), 1);
}
