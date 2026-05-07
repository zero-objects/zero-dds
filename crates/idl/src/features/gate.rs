// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Feature-Gate-Validator (A2 Spec-Check 2.0).
//!
//! Walks ein CST nach Recognition und prueft pro Production-ID ob das
//! korrespondierende Feature in [`IdlFeatures`] aktiv ist. Wenn nicht,
//! liefert er einen [`FeatureGateError`] mit klarer Fehlermeldung.
//!
//! # Architektur
//!
//! Der Validator betreibt eine statische Lookup-Tabelle `Production-ID
//! → Required-Feature`. Productions ohne Feature-Gate werden ignoriert
//! (die meisten Plain-DDS- und XTypes-Konstrukte sind feature-frei).
//! CORBA/CCM-Konstrukte und Vendor-Erweiterungen sind feature-gated.
//!
//! # Fehlerbehandlung
//!
//! Der Validator akkumuliert *alle* Verletzungen — der Caller bekommt
//! eine Liste, nicht nur den ersten Fehler. So sieht der User in einem
//! Pass alle disabled Features auf einmal.

use crate::cst::CstNode;
use crate::errors::Span;
use crate::features::IdlFeatures;
use crate::grammar::ProductionId;

/// Gate-Verletzung: ein verwendetes Konstrukt benoetigt ein Feature
/// das in [`IdlFeatures`] aus ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureGateError {
    /// Production-ID des verwendeten Konstrukts.
    pub production_id: ProductionId,
    /// Name der Production (Spec-BNF-Bezeichnung).
    pub production_name: &'static str,
    /// Name des inaktiven Features (Feld in [`IdlFeatures`]).
    pub required_feature: &'static str,
    /// Span im Source.
    pub span: Span,
}

impl core::fmt::Display for FeatureGateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "use of `{}` requires feature `{}` (currently disabled in ParserConfig::features)",
            self.production_name, self.required_feature
        )
    }
}

impl std::error::Error for FeatureGateError {}

/// Mappt eine `(Production-ID, Alternative-Index)`-Kombi auf das Feature
/// das sie aktiv haben muss. `None` heisst "kein Gate".
///
/// Wird vom Validator konsultiert *nach* `production_required_feature`,
/// um auch einzelne Alternativen einer ungated Production zu gaten
/// (z.B. `oneway`-Variante von `op_dcl`).
#[must_use]
pub fn alternative_required_feature(prod_id: ProductionId, alt_idx: usize) -> Option<&'static str> {
    use crate::grammar::idl42::{
        ID_INTERFACE_KIND, ID_OP_DCL, ID_VALUE_HEADER, ID_VALUE_INHERITANCE_SPEC,
    };
    match prod_id {
        // `op_dcl` Alts:
        // 0 = oneway_with_raises
        // 1 = oneway_no_raises
        // 2 = with_raises
        // 3 = no_raises
        id if id == ID_OP_DCL && (alt_idx == 0 || alt_idx == 1) => Some("corba_oneway_op"),
        // `interface_kind` Alts:
        // 0 = abstract
        // 1 = local
        id if id == ID_INTERFACE_KIND && alt_idx == 0 => Some("corba_abstract_interface"),
        id if id == ID_INTERFACE_KIND && alt_idx == 1 => Some("corba_local_interface"),
        // `value_header` Alts (§7.4.5 Extras: custom + abstract valuetype):
        // 0 = custom
        // 1 = plain
        // 2 = abstract
        id if id == ID_VALUE_HEADER && (alt_idx == 0 || alt_idx == 2) => {
            Some("corba_value_types_extras")
        }
        // `value_inheritance_spec` Alt 0 = extends_truncatable.
        id if id == ID_VALUE_INHERITANCE_SPEC && alt_idx == 0 => Some("corba_value_types_extras"),
        _ => None,
    }
}

/// Mappt eine Production-ID auf das Feature, das sie aktiv haben muss.
/// `None` heisst "kein Gate" (Plain-DDS-Konstrukt, immer erlaubt).
#[must_use]
pub fn production_required_feature(prod_id: ProductionId) -> Option<&'static str> {
    use crate::grammar::idl42::{
        ID_COMPONENT_BODY, ID_COMPONENT_DCL, ID_COMPONENT_DEF, ID_COMPONENT_EXPORT,
        ID_COMPONENT_FORWARD_DCL, ID_COMPONENT_HEADER, ID_COMPONENT_INHERITANCE_SPEC,
        ID_CONNECTOR_DCL, ID_CONNECTOR_EXPORT, ID_CONNECTOR_HEADER, ID_CONNECTOR_INHERIT_SPEC,
        ID_CONSUMES_DCL, ID_EMITS_DCL, ID_EVENT_DCL, ID_EVENT_DEF, ID_EVENT_FORWARD_DCL,
        ID_EVENT_HEADER, ID_FACTORY_DCL, ID_FACTORY_PARAM_DCLS, ID_FINDER_DCL, ID_HOME_BODY,
        ID_HOME_DCL, ID_HOME_EXPORT, ID_HOME_HEADER, ID_HOME_INHERITANCE_SPEC, ID_IMPORT_DCL,
        ID_IMPORTED_SCOPE, ID_INIT_DCL, ID_INIT_PARAM_DCL, ID_INIT_PARAM_DCLS, ID_INTERFACE_TYPE,
        ID_NATIVE_DCL, ID_PORT_BODY, ID_PORT_DCL, ID_PORT_EXPORT, ID_PORT_REF, ID_PORTTYPE_DCL,
        ID_PRIMARY_KEY_SPEC, ID_PROVIDES_DCL, ID_PUBLISHES_DCL, ID_STATE_MEMBER,
        ID_SUPPORTED_INTERFACE_SPEC, ID_TEMPLATE_MODULE_DCL, ID_TEMPLATE_MODULE_INST,
        ID_TYPE_ID_DCL, ID_TYPE_PREFIX_DCL, ID_USES_DCL, ID_VALUE_DEF, ID_VALUE_ELEMENT,
        ID_VALUE_HEADER, ID_VALUE_INHERITANCE_NAME_LIST, ID_VALUE_INHERITANCE_SPEC,
    };
    match prod_id {
        id if id == ID_NATIVE_DCL => Some("corba_native"),
        // §7.4.5 Value Types Full
        id if id == ID_VALUE_DEF
            || id == ID_VALUE_HEADER
            || id == ID_VALUE_INHERITANCE_SPEC
            || id == ID_VALUE_INHERITANCE_NAME_LIST
            || id == ID_VALUE_ELEMENT
            || id == ID_STATE_MEMBER
            || id == ID_INIT_DCL
            || id == ID_INIT_PARAM_DCLS
            || id == ID_INIT_PARAM_DCL =>
        {
            Some("corba_value_types_full")
        }
        // §7.4.6 Repository-IDs (typeid/typeprefix).
        id if id == ID_TYPE_ID_DCL || id == ID_TYPE_PREFIX_DCL => Some("corba_repository_ids"),
        // §7.4.6 Import.
        id if id == ID_IMPORT_DCL || id == ID_IMPORTED_SCOPE => Some("corba_import"),
        // §7.4.9 CCM Components-Basic
        id if id == ID_COMPONENT_DCL
            || id == ID_COMPONENT_FORWARD_DCL
            || id == ID_COMPONENT_DEF
            || id == ID_COMPONENT_HEADER
            || id == ID_COMPONENT_INHERITANCE_SPEC
            || id == ID_COMPONENT_BODY
            || id == ID_COMPONENT_EXPORT
            || id == ID_PROVIDES_DCL
            || id == ID_USES_DCL
            || id == ID_INTERFACE_TYPE
            || id == ID_SUPPORTED_INTERFACE_SPEC =>
        {
            Some("corba_components")
        }
        // §7.4.9 CCM Homes
        id if id == ID_HOME_DCL
            || id == ID_HOME_HEADER
            || id == ID_HOME_INHERITANCE_SPEC
            || id == ID_PRIMARY_KEY_SPEC
            || id == ID_HOME_BODY
            || id == ID_HOME_EXPORT
            || id == ID_FACTORY_DCL
            || id == ID_FACTORY_PARAM_DCLS
            || id == ID_FINDER_DCL =>
        {
            Some("corba_homes")
        }
        // §7.4.9 CCM Eventtypes (`emits`/`publishes`/`consumes` + event_def)
        id if id == ID_EVENT_DCL
            || id == ID_EVENT_FORWARD_DCL
            || id == ID_EVENT_DEF
            || id == ID_EVENT_HEADER
            || id == ID_EMITS_DCL
            || id == ID_PUBLISHES_DCL
            || id == ID_CONSUMES_DCL =>
        {
            Some("corba_eventtypes")
        }
        // §7.4.9 CCM Ports + Connectors
        id if id == ID_PORTTYPE_DCL
            || id == ID_PORT_BODY
            || id == ID_PORT_REF
            || id == ID_PORT_EXPORT
            || id == ID_PORT_DCL
            || id == ID_CONNECTOR_DCL
            || id == ID_CONNECTOR_HEADER
            || id == ID_CONNECTOR_INHERIT_SPEC
            || id == ID_CONNECTOR_EXPORT =>
        {
            Some("corba_ports")
        }
        // §7.4.12 Template Modules — nur Top-Level-Wrapper gaten,
        // weil Sub-Productions (formal/actual_parameter) Earley-State-
        // Knoten via type_spec/const_expr in nicht-Template-Kontexten
        // produzieren koennen (false-positive auf normales `long x`).
        id if id == ID_TEMPLATE_MODULE_DCL || id == ID_TEMPLATE_MODULE_INST => {
            Some("corba_template_modules")
        }
        _ => None,
    }
}

/// Prueft ein Feature-Flag in [`IdlFeatures`] anhand seines Namens.
///
/// Liefert `true` wenn das benannte Flag aktiv ist; `false` sonst
/// inkl. unbekannte Namen (defensive: unbekannte Namen werden als
/// "deaktiviert" interpretiert, sodass der Caller den Konflikt sieht).
#[must_use]
pub fn is_feature_enabled(features: &IdlFeatures, name: &str) -> bool {
    match name {
        "corba_value_types_full" => features.corba_value_types_full,
        "corba_value_types_extras" => features.corba_value_types_extras,
        "corba_repository_ids" => features.corba_repository_ids,
        "corba_import" => features.corba_import,
        "corba_local_interface" => features.corba_local_interface,
        "corba_abstract_interface" => features.corba_abstract_interface,
        "corba_object_base" => features.corba_object_base,
        "corba_oneway_op" => features.corba_oneway_op,
        "corba_context" => features.corba_context,
        "corba_components" => features.corba_components,
        "corba_homes" => features.corba_homes,
        "corba_eventtypes" => features.corba_eventtypes,
        "corba_ports" => features.corba_ports,
        "corba_template_modules" => features.corba_template_modules,
        "corba_native" => features.corba_native,
        "preprocessor_full" => features.preprocessor_full,
        "preprocessor_warning_line" => features.preprocessor_warning_line,
        "vendor_rti" => features.vendor_rti,
        "vendor_opensplice" => features.vendor_opensplice,
        "vendor_opensplice_legacy" => features.vendor_opensplice_legacy,
        "vendor_cyclonedds" => features.vendor_cyclonedds,
        "vendor_fastdds" => features.vendor_fastdds,
        _ => false,
    }
}

/// Walks das CST und sammelt alle Feature-Gate-Verletzungen.
///
/// Pro CstNode wird die Production-ID auf [`production_required_feature`]
/// gemapped; ist das Feature inaktiv → ein [`FeatureGateError`] wird
/// emittiert.
///
/// Iterative Implementation (Stack-explicit), weil Earley-CSTs sehr
/// tief verschachtelt sein koennen (Recursion-Limit-Risiko).
#[must_use]
pub fn validate<'a>(cst: &'a CstNode<'a>, features: &IdlFeatures) -> Vec<FeatureGateError> {
    let mut errors = Vec::new();
    let mut stack: Vec<&CstNode<'_>> = vec![cst];
    while let Some(node) = stack.pop() {
        if let Some(prod_id) = node.production() {
            // Production-Level-Gate.
            if let Some(required) = production_required_feature(prod_id) {
                if !is_feature_enabled(features, required) {
                    errors.push(FeatureGateError {
                        production_id: prod_id,
                        production_name: production_name_of(prod_id),
                        required_feature: required,
                        span: node.span,
                    });
                }
            }
            // Alternative-Level-Gate (z.B. `oneway`-Variante von op_dcl).
            if let Some(alt_idx) = node.alternative_index() {
                if let Some(required) = alternative_required_feature(prod_id, alt_idx) {
                    if !is_feature_enabled(features, required) {
                        errors.push(FeatureGateError {
                            production_id: prod_id,
                            production_name: production_name_of(prod_id),
                            required_feature: required,
                            span: node.span,
                        });
                    }
                }
            }
        }
        for child in &node.children {
            stack.push(child);
        }
    }
    errors
}

fn production_name_of(prod_id: ProductionId) -> &'static str {
    crate::grammar::idl42::IDL_42
        .production(prod_id)
        .map(|p| p.name)
        .unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::cst::build_cst;
    use crate::engine::Engine;
    use crate::grammar::idl42::IDL_42;
    use crate::lexer::Tokenizer;

    fn check(src: &str, features: IdlFeatures) -> Vec<FeatureGateError> {
        // CST-Reconstruction kann sehr tief rekursiv sein (Earley-Engine);
        // wir laufen daher in einem Thread mit grosszuegigem Stack.
        let src = src.to_string();
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let tokenizer = Tokenizer::for_grammar(&IDL_42);
                let stream = tokenizer.tokenize(&src).expect("lex");
                let engine = Engine::new(&IDL_42);
                let result = engine.recognize(stream.tokens()).expect("recognize");
                // compiled_grammar() liefert die desugarte Form mit
                // synthetisierten `_rep_X`-Productions, identisch zum
                // Pfad des Public-`parse()` API.
                let cst =
                    build_cst(engine.compiled_grammar(), stream.tokens(), &result).expect("cst");
                validate(&cst, &features)
            })
            .expect("spawn")
            .join()
            .expect("join")
    }

    #[test]
    fn no_corba_use_no_errors_in_dds_basic() {
        // Plain-DDS-IDL ohne CORBA-Konstrukte → keine Gate-Verletzung
        // selbst in dds_basic.
        let src = "module svc { struct Topic { long id; }; };";
        let errors = check(src, IdlFeatures::dds_basic());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn no_corba_use_no_errors_in_dds_extensible() {
        let src = "struct Foo { long x; long y; };";
        let errors = check(src, IdlFeatures::dds_extensible());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn dds_extensible_allows_native() {
        // corba_native ist in dds_extensible an.
        let errors = check("native Handle;", IdlFeatures::dds_extensible());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn dds_basic_rejects_native() {
        // corba_native ist in dds_basic aus → klare Fehlermeldung.
        let errors = check("native Handle;", IdlFeatures::dds_basic());
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_native" && e.production_name == "native_dcl"),
            "expected corba_native gate error, got {errors:?}"
        );
    }

    #[test]
    fn corba_full_allows_native() {
        let errors = check("native Handle;", IdlFeatures::corba_full());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn opensplice_legacy_allows_native() {
        let errors = check("native Handle;", IdlFeatures::opensplice_legacy());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn is_feature_enabled_matches_struct_fields() {
        let f = IdlFeatures::all();
        assert!(is_feature_enabled(&f, "corba_native"));
        assert!(is_feature_enabled(&f, "vendor_rti"));
        assert!(!is_feature_enabled(&f, "unknown_feature"));
    }

    #[test]
    fn feature_gate_error_has_helpful_message() {
        let errors = check("native Handle;", IdlFeatures::dds_basic());
        let msg = errors[0].to_string();
        assert!(msg.contains("native_dcl"), "got: {msg}");
        assert!(msg.contains("corba_native"), "got: {msg}");
        assert!(msg.contains("disabled"), "got: {msg}");
    }

    // -----------------------------------------------------------------
    // §7.4.5 Value Types Full — Feature-Gate-Tests
    // -----------------------------------------------------------------

    #[test]
    fn dds_extensible_rejects_value_def() {
        // corba_value_types_full ist in dds_extensible aus.
        let errors = check("valuetype V {};", IdlFeatures::dds_extensible());
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_value_types_full"),
            "expected corba_value_types_full gate error, got {errors:?}"
        );
    }

    #[test]
    fn corba_full_allows_value_def() {
        let errors = check("valuetype V { public long x; };", IdlFeatures::corba_full());
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn opensplice_legacy_allows_value_def_with_factory() {
        let errors = check(
            "valuetype V { exception E {}; factory create() raises (E); };",
            IdlFeatures::opensplice_legacy(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn dds_basic_allows_value_box() {
        // value_box ist nicht gated → bleibt erlaubt auch in dds_basic.
        let errors = check("valuetype Box long;", IdlFeatures::dds_basic());
        assert!(
            errors.is_empty(),
            "value_box bleibt ungated, got {errors:?}"
        );
    }

    // -----------------------------------------------------------------
    // §7.4.6 Repository-IDs / Import — Feature-Gate-Tests
    // -----------------------------------------------------------------

    #[test]
    fn dds_extensible_rejects_typeid() {
        let errors = check(
            r#"typeid Foo "IDL:foo:1.0";"#,
            IdlFeatures::dds_extensible(),
        );
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_repository_ids"),
            "expected corba_repository_ids gate error, got {errors:?}"
        );
    }

    #[test]
    fn dds_extensible_rejects_typeprefix() {
        let errors = check(
            r#"typeprefix Foo "company";"#,
            IdlFeatures::dds_extensible(),
        );
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_repository_ids"),
            "got {errors:?}"
        );
    }

    #[test]
    fn dds_extensible_rejects_import() {
        let errors = check("import Foo;", IdlFeatures::dds_extensible());
        assert!(
            errors.iter().any(|e| e.required_feature == "corba_import"),
            "got {errors:?}"
        );
    }

    #[test]
    fn corba_full_allows_repository_ids_and_import() {
        let errors = check(
            r#"typeid Foo "IDL:foo:1.0"; typeprefix M "company"; import Bar;"#,
            IdlFeatures::corba_full(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn opensplice_legacy_allows_repository_ids() {
        let errors = check(
            r#"typeid Foo "IDL:foo:1.0";"#,
            IdlFeatures::opensplice_legacy(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    // -----------------------------------------------------------------
    // §7.4.7 CORBA-Interface-Erweiterungen (oneway, abstract, local)
    // -----------------------------------------------------------------

    #[test]
    fn dds_extensible_rejects_oneway_op() {
        let errors = check(
            "interface S { oneway void log(in string m); };",
            IdlFeatures::dds_extensible(),
        );
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_oneway_op"),
            "expected corba_oneway_op gate error, got {errors:?}"
        );
    }

    #[test]
    fn dds_extensible_rejects_abstract_interface() {
        let errors = check(
            "abstract interface IBase {};",
            IdlFeatures::dds_extensible(),
        );
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_abstract_interface"),
            "got {errors:?}"
        );
    }

    #[test]
    fn dds_extensible_rejects_local_interface() {
        let errors = check("local interface ILocal {};", IdlFeatures::dds_extensible());
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_local_interface"),
            "got {errors:?}"
        );
    }

    #[test]
    fn corba_full_allows_oneway_and_abstract_local() {
        let errors = check(
            "abstract interface IA {}; local interface IL {}; \
             interface S { oneway void log(in string m); };",
            IdlFeatures::corba_full(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn dds_extensible_allows_plain_op() {
        // Normal-Op ohne oneway → kein Gate, geht in jedem Profil.
        let errors = check(
            "interface S { void log(in string m); };",
            IdlFeatures::dds_extensible(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    #[test]
    fn opensplice_legacy_allows_oneway_and_abstract_interface() {
        let errors = check(
            "abstract interface IA {}; interface S { oneway void log(in string m); };",
            IdlFeatures::opensplice_legacy(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }

    // -----------------------------------------------------------------
    // §7.4.5 Extras — custom/abstract valuetype + truncatable (B4)
    // -----------------------------------------------------------------

    #[test]
    fn corba_full_minus_extras_rejects_custom_valuetype() {
        // Mit corba_full (extras=on) sollte custom gehen.
        let mut f = IdlFeatures::corba_full();
        f.corba_value_types_extras = false;
        let errors = check("custom valuetype V {};", f);
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_value_types_extras"),
            "expected corba_value_types_extras gate, got {errors:?}"
        );
    }

    #[test]
    fn corba_full_minus_extras_rejects_abstract_valuetype() {
        let mut f = IdlFeatures::corba_full();
        f.corba_value_types_extras = false;
        let errors = check("abstract valuetype V {};", f);
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_value_types_extras"),
            "got {errors:?}"
        );
    }

    #[test]
    fn corba_full_minus_extras_rejects_truncatable() {
        let mut f = IdlFeatures::corba_full();
        f.corba_value_types_extras = false;
        let errors = check("valuetype Derived : truncatable Base {};", f);
        assert!(
            errors
                .iter()
                .any(|e| e.required_feature == "corba_value_types_extras"),
            "got {errors:?}"
        );
    }

    #[test]
    fn corba_full_allows_custom_abstract_truncatable() {
        let errors = check(
            "abstract valuetype IA {}; \
             custom valuetype V : truncatable Base {};",
            IdlFeatures::corba_full(),
        );
        assert!(errors.is_empty(), "got {errors:?}");
    }
}
