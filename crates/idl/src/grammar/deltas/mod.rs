// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar deltas (T6.4).
//!
//! A [`GrammarDelta`] is an additive patch for a base grammar:
//!
//! - **New productions** can be added (e.g. RTI-specific
//!   constructs like `<rti_keylist_pragma>`).
//! - **Existing productions** can be extended with additional alternatives
//!   (e.g. a new annotation variant).
//!
//! Composition via [`compose`] in [`super::compose`]: returns a
//! [`CompiledGrammar`](super::compile::CompiledGrammar) that combines
//! base + delta. Multiple deltas can be applied sequentially
//! (e.g. RTI delta + vendor-X delta).
//!
//! # Architecture rationale
//!
//! Vendor migration paths (RTI, OpenSplice, Cyclone) often need only
//! a few additional constructs (annotations, `#pragma` directives).
//! Instead of maintaining a separate grammar per vendor, deltas add
//! these extensions onto the base grammar — the single source of truth
//! for OMG IDL 4.2 is preserved. See RFC 0001 §5.5.
//!
//! Phase 0: concrete deltas follow with T6.5 (RTI). Here only the types.

pub mod rti_connext;

use super::{Alternative, Production, ProductionId};

pub use rti_connext::RTI_CONNEXT;

/// Additive grammar patch.
///
/// Combined with a base grammar in [`super::compose::compose`].
#[derive(Debug, Clone, Copy)]
pub struct GrammarDelta {
    /// Human-readable name for diagnostics (e.g. "RTI Connext 7.x").
    pub name: &'static str,
    /// New productions added to the base. Their IDs
    /// are reassigned during composition (from
    /// `base.production_count`).
    pub additional_productions: &'static [Production],
    /// Extend existing productions with new alternatives.
    pub alternative_extensions: &'static [AlternativeExtension],
}

/// Describes an extension to an existing production.
#[derive(Debug, Clone, Copy)]
pub struct AlternativeExtension {
    /// ID of the base production being extended.
    pub target: ProductionId,
    /// Additional alternatives appended to the end of the existing
    /// `alternatives` list.
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
