// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Multiplex-Receptacle — Spec §7.7, S. 13.
//!
//! Spec §7.7 (S. 13): "In case of a multiplex receptacle the context
//! will return a sequence of object references for AMI4CCM. [...]
//! `sendc_StockManagers` is an implied sequence."
//!
//! Wirkung im Component-Context:
//!
//! ```text
//!   sequence<AMI4CCM_StockManager> sendc_StockManagers ();
//! ```
//!
//! statt:
//!
//! ```text
//!   AMI4CCM_StockManager sendc_StockManager ();
//! ```
//!
//! Wir produzieren beide Varianten als IDL-Snippets fuer das Codegen.

use alloc::format;
use alloc::string::String;

/// Multiplex-aware Component-Context-Methoden-Generator (Spec §7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceptacleArity {
    /// `uses` (single connection).
    Simplex,
    /// `uses multiple` (sequence of connections).
    Multiplex,
}

/// Generiert die Component-Context-Methode fuer ein
/// AMI4CCM-enabled Receptacle.
///
/// Spec §7.7, S. 12-13:
/// * Simplex: `AMI4CCM_<Iface> sendc_<recep_name>();`
/// * Multiplex: `sequence<AMI4CCM_<Iface>> sendc_<recep_name>s();`
///
/// Gibt das IDL-Snippet als String zurueck. Caller entscheidet, wo
/// das eingefuegt wird (typisch ins `local interface CCM_<Comp>_Context`).
#[must_use]
pub fn context_method_for_receptacle(
    receptacle_name: &str,
    interface_type: &str,
    arity: ReceptacleArity,
) -> String {
    let ami_type = format!("AMI4CCM_{interface_type}");
    match arity {
        ReceptacleArity::Simplex => {
            format!("{ami_type} sendc_{receptacle_name}();")
        }
        ReceptacleArity::Multiplex => {
            // Spec §7.7, S. 13: pluralisiertes Suffix "s" am Receptacle-
            // Namen ist Beispiel-Konvention; Spec erlaubt jeden Namen.
            // Wir folgen der Beispiel-Form `sendc_<name>s` exakt.
            format!("sequence<{ami_type}> sendc_{receptacle_name}s();")
        }
    }
}

/// Generiert das `sequence<>`-typedef fuer ein Multiplex-Receptacle
/// (Spec §7.7, S. 13).
///
/// Vor der Component-Context-Definition wird typisch ein typedef
/// emittiert:
/// ```text
///   typedef sequence<AMI4CCM_StockManager> AMI4CCM_StockManagerSeq;
/// ```
#[must_use]
pub fn sequence_typedef_for_interface(interface_type: &str) -> String {
    let ami_type = format!("AMI4CCM_{interface_type}");
    format!("typedef sequence<{ami_type}> {ami_type}Seq;")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn simplex_produces_single_method() {
        let m = context_method_for_receptacle("manager", "StockManager", ReceptacleArity::Simplex);
        assert_eq!(m, "AMI4CCM_StockManager sendc_manager();");
    }

    #[test]
    fn multiplex_produces_sequence_method() {
        let m =
            context_method_for_receptacle("manager", "StockManager", ReceptacleArity::Multiplex);
        assert_eq!(m, "sequence<AMI4CCM_StockManager> sendc_managers();");
    }

    #[test]
    fn typedef_emits_seq_alias() {
        let t = sequence_typedef_for_interface("StockManager");
        assert_eq!(
            t,
            "typedef sequence<AMI4CCM_StockManager> AMI4CCM_StockManagerSeq;"
        );
    }

    #[test]
    fn arity_variants_are_distinct() {
        assert_ne!(ReceptacleArity::Simplex, ReceptacleArity::Multiplex);
    }
}
