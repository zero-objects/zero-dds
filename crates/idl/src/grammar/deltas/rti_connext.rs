// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTI Connext DDS grammar delta (proof-of-concept, T6.5).
//!
//! Extends the OMG-IDL-4.2 base grammar with RTI-specific constructs.
//! In phase 0 this module demonstrates the delta architecture on a
//! characteristic vendor feature:
//!
//! - **`keylist <Type> ( <field>+ );`** — top-level directive to
//!   mark the key fields of a topic. RTI historically uses
//!   `#pragma keylist <Type> <field>...` (the preprocessor
//!   variant, comes with T6.1). The parser variant with parentheses and
//!   comma separation is modeled here as the canonical example,
//!   because it needs no preprocessor and thus tests the
//!   delta architecture in isolation.
//!
//! The production IDs from `1000` are reserved for vendor deltas
//! (convention, not enforced). Multiple deltas can coexist,
//! as long as their IDs do not collide.

use super::{AlternativeExtension, GrammarDelta};
use crate::grammar::idl42::{ID_DEFINITION, ID_IDENTIFIER};
use crate::grammar::{Alternative, Production, ProductionId, SpecRef, Symbol, TokenKind};

const RTI_SPEC: SpecRef = SpecRef {
    doc: "RTI-Connext-7.x",
    section: "vendor-extensions",
};

/// Production ID for `<rti_keylist>`.
/// Vendor deltas reserve IDs from `1000` (convention).
pub const ID_RTI_KEYLIST: ProductionId = ProductionId(1000);
/// Production ID for `<rti_key_field_list>`.
pub const ID_RTI_KEY_FIELD_LIST: ProductionId = ProductionId(1001);

const KEYLIST_ALTS: &[Alternative] = &[Alternative {
    name: Some("rti_keylist"),
    symbols: &[
        Symbol::Terminal(TokenKind::Keyword("keylist")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("(")),
        Symbol::Nonterminal(ID_RTI_KEY_FIELD_LIST),
        Symbol::Terminal(TokenKind::Punct(")")),
    ],
    note: Some("RTI Connext: the key fields of a topic"),
}];

const KEY_FIELD_LIST_ALTS: &[Alternative] = &[
    Alternative {
        name: Some("single"),
        symbols: &[Symbol::Nonterminal(ID_IDENTIFIER)],
        note: None,
    },
    Alternative {
        name: Some("cons"),
        symbols: &[
            Symbol::Nonterminal(ID_RTI_KEY_FIELD_LIST),
            Symbol::Terminal(TokenKind::Punct(",")),
            Symbol::Nonterminal(ID_IDENTIFIER),
        ],
        note: None,
    },
];

const ADDITIONAL_PRODS: &[Production] = &[
    Production {
        id: ID_RTI_KEYLIST,
        name: "rti_keylist",
        spec_ref: RTI_SPEC,
        alternatives: KEYLIST_ALTS,
        ast_hint: None,
    },
    Production {
        id: ID_RTI_KEY_FIELD_LIST,
        name: "rti_key_field_list",
        spec_ref: RTI_SPEC,
        alternatives: KEY_FIELD_LIST_ALTS,
        ast_hint: None,
    },
];

/// New `<definition>` alternative: allows `keylist Type (fields);`
/// as a top-level statement. The annotation prefix is **not** forked,
/// to keep the delta minimal — RTI keylist is conventionally
/// not annotatable.
const DEFINITION_RTI_KEYLIST_ALT: &[Alternative] = &[Alternative {
    name: Some("rti_keylist"),
    symbols: &[
        Symbol::Nonterminal(ID_RTI_KEYLIST),
        Symbol::Terminal(TokenKind::Punct(";")),
    ],
    note: Some("RTI Connext keylist-Direktive"),
}];

const EXTENSIONS: &[AlternativeExtension] = &[AlternativeExtension {
    target: ID_DEFINITION,
    extra_alternatives: DEFINITION_RTI_KEYLIST_ALT,
}];

/// Der RTI-Connext-Grammar-Delta. Konsumiert via
/// [`crate::grammar::compose::compose`].
pub const RTI_CONNEXT: GrammarDelta = GrammarDelta {
    name: "RTI Connext (Phase 0 PoC)",
    additional_productions: ADDITIONAL_PRODS,
    alternative_extensions: EXTENSIONS,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::compose::compose;
    use crate::grammar::idl42::IDL_42;

    #[test]
    fn rti_delta_has_two_additional_productions() {
        assert_eq!(RTI_CONNEXT.additional_productions.len(), 2);
    }

    #[test]
    fn rti_delta_extends_definition() {
        assert_eq!(RTI_CONNEXT.alternative_extensions.len(), 1);
        assert_eq!(RTI_CONNEXT.alternative_extensions[0].target, ID_DEFINITION);
    }

    #[test]
    fn composing_rti_delta_with_idl42_adds_two_productions() {
        let base = compose(&IDL_42, &[]);
        let with_rti = compose(&IDL_42, &[&RTI_CONNEXT]);
        assert_eq!(with_rti.production_count(), base.production_count() + 2);
    }

    #[test]
    fn composed_definition_gains_rti_keylist_alternative() {
        let with_rti = compose(&IDL_42, &[&RTI_CONNEXT]);
        let def = with_rti
            .productions
            .iter()
            .find(|p| p.id == ID_DEFINITION)
            .expect("definition present");
        // The base has 19 alternatives (module/type/const/except/interface/
        // value_box/value_forward/value_def/typeid/typeprefix/import/
        // component/home/event/porttype/connector/template_module/
        // template_inst/annotation_dcl); the delta adds 1 = 20.
        assert_eq!(def.alternatives.len(), 20);
        assert!(
            def.alternatives
                .iter()
                .any(|a| a.name == Some("rti_keylist")),
            "rti_keylist alternative missing"
        );
    }

    #[test]
    fn keylist_production_uses_keyword_keylist() {
        let kw_present = KEYLIST_ALTS[0]
            .symbols
            .iter()
            .any(|s| matches!(s, Symbol::Terminal(TokenKind::Keyword("keylist"))));
        assert!(kw_present);
    }

    #[test]
    fn rti_delta_composition_validates_clean() {
        // T6.9: no errors after base+delta composition
        // (invalid-start or dangling-reference).
        use crate::grammar::validate::validate_compiled;
        let composed = compose(&IDL_42, &[&RTI_CONNEXT]);
        let report = validate_compiled(&composed);
        assert!(
            !report.has_errors(),
            "RTI delta must not introduce validation errors: {:?}",
            report.errors().collect::<Vec<_>>()
        );
    }
}
