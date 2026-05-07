// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar-Deltas (T6.4).
//!
//! Ein [`GrammarDelta`] ist ein additives Patch fuer eine Base-Grammar:
//!
//! - **Neue Productions** koennen hinzugefuegt werden (z.B. RTI-spezifische
//!   Konstrukte wie `<rti_keylist_pragma>`).
//! - **Bestehende Productions** koennen um zusaetzliche Alternativen
//!   erweitert werden (z.B. neue Annotation-Variante).
//!
//! Komposition via [`compose`] in [`super::compose`]: liefert eine
//! [`CompiledGrammar`](super::compile::CompiledGrammar), die Base + Delta
//! kombiniert. Mehrere Deltas koennen sequenziell appliziert werden
//! (z.B. RTI-Delta + Vendor-X-Delta).
//!
//! # Architektur-Begruendung
//!
//! Vendor-Migrations-Pfade (RTI, OpenSplice, Cyclone) brauchen oft nur
//! ein paar zusaetzliche Konstrukte (Annotations, `#pragma`-Direktiven).
//! Statt eine separate Grammar pro Vendor zu pflegen, addieren Deltas
//! diese Erweiterungen auf die Base-Grammar — Single-Source-of-Truth
//! fuer OMG-IDL-4.2 bleibt erhalten. Siehe RFC 0001 §5.5.
//!
//! Phase 0: Konkrete Deltas folgen mit T6.5 (RTI). Hier nur die Typen.

pub mod rti_connext;

use super::{Alternative, Production, ProductionId};

pub use rti_connext::RTI_CONNEXT;

/// Additives Grammar-Patch.
///
/// Wird in [`super::compose::compose`] mit einer Base-Grammar kombiniert.
#[derive(Debug, Clone, Copy)]
pub struct GrammarDelta {
    /// Menschenlesbarer Name fuer Diagnostik (z.B. "RTI Connext 7.x").
    pub name: &'static str,
    /// Neue Productions, die zur Base hinzugefuegt werden. Ihre IDs
    /// werden bei der Komposition neu vergeben (ab
    /// `base.production_count`).
    pub additional_productions: &'static [Production],
    /// Existierende Productions um neue Alternativen erweitern.
    pub alternative_extensions: &'static [AlternativeExtension],
}

/// Beschreibt eine Erweiterung an einer existierenden Production.
#[derive(Debug, Clone, Copy)]
pub struct AlternativeExtension {
    /// ID der Base-Production, die erweitert wird.
    pub target: ProductionId,
    /// Zusaetzliche Alternativen, die ans Ende der bestehenden
    /// `alternatives`-Liste angehaengt werden.
    pub extra_alternatives: &'static [Alternative],
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::grammar::{Symbol, TokenKind};

    /// Test-Helper.
    const fn alt(symbols: &'static [Symbol]) -> Alternative {
        Alternative {
            name: None,
            symbols,
            note: None,
        }
    }

    const fn prod(id: u32, name: &'static str, alts: &'static [Alternative]) -> Production {
        Production {
            id: ProductionId(id),
            name,
            spec_ref: super::super::SpecRef {
                doc: "TEST",
                section: "0",
            },
            alternatives: alts,
            ast_hint: None,
        }
    }

    static EXTRA_PRODS: &[Production] = &[prod(
        100,
        "rti_extension",
        &[alt(&[Symbol::Terminal(TokenKind::Keyword("rti_only"))])],
    )];

    static EXTRA_ALTS: &[Alternative] =
        &[alt(&[Symbol::Terminal(TokenKind::Keyword("vendor_kw"))])];

    static EXTENSIONS: &[AlternativeExtension] = &[AlternativeExtension {
        target: ProductionId(0),
        extra_alternatives: EXTRA_ALTS,
    }];

    static SAMPLE_DELTA: GrammarDelta = GrammarDelta {
        name: "sample",
        additional_productions: EXTRA_PRODS,
        alternative_extensions: EXTENSIONS,
    };

    #[test]
    fn delta_carries_name_and_additions() {
        assert_eq!(SAMPLE_DELTA.name, "sample");
        assert_eq!(SAMPLE_DELTA.additional_productions.len(), 1);
        assert_eq!(SAMPLE_DELTA.alternative_extensions.len(), 1);
    }

    #[test]
    fn additional_production_addressable() {
        let p = SAMPLE_DELTA.additional_productions[0];
        assert_eq!(p.name, "rti_extension");
        assert_eq!(p.alternatives.len(), 1);
    }

    #[test]
    fn alternative_extension_targets_specific_production() {
        let ext = SAMPLE_DELTA.alternative_extensions[0];
        assert_eq!(ext.target, ProductionId(0));
        assert_eq!(ext.extra_alternatives.len(), 1);
    }

    #[test]
    fn delta_is_copyable_and_clonable() {
        let d = SAMPLE_DELTA;
        let cloned = d;
        assert_eq!(cloned.name, "sample");
    }
}
