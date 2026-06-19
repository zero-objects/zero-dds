// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Multiplex receptacle — Spec §7.7, p. 13.
//!
//! Spec §7.7 (p. 13): "In case of a multiplex receptacle the context
//! will return a sequence of object references for AMI4CCM. [...]
//! `sendc_StockManagers` is an implied sequence."
//!
//! Effect in the component context:
//!
//! ```text
//!   sequence<AMI4CCM_StockManager> sendc_StockManagers ();
//! ```
//!
//! instead of:
//!
//! ```text
//!   AMI4CCM_StockManager sendc_StockManager ();
//! ```
//!
//! We produce both variants as IDL snippets for the codegen.

use alloc::format;
use alloc::string::String;

/// Multiplex-aware component-context method generator (Spec §7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceptacleArity {
    /// `uses` (single connection).
    Simplex,
    /// `uses multiple` (sequence of connections).
    Multiplex,
}

/// Generates the component-context method for an
/// AMI4CCM-enabled receptacle.
///
/// Spec §7.7, p. 12-13:
/// * Simplex: `AMI4CCM_<Iface> sendc_<recep_name>();`
/// * Multiplex: `sequence<AMI4CCM_<Iface>> sendc_<recep_name>s();`
///
/// Returns the IDL snippet as a string. The caller decides where it
/// is inserted (typically into `local interface CCM_<Comp>_Context`).
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
            // Spec §7.7, p. 13: the pluralized "s" suffix on the
            // receptacle name is an example convention; the spec allows
            // any name. We follow the example form `sendc_<name>s` exactly.
            format!("sequence<{ami_type}> sendc_{receptacle_name}s();")
        }
    }
}

/// Generates the `sequence<>` typedef for a multiplex receptacle
/// (Spec §7.7, p. 13).
///
/// A typedef is typically emitted before the component-context
/// definition:
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
