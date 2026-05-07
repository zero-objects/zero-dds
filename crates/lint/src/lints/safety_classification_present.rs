// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_safety_classification_present` — jede Workspace-Crate mit einer
//! `src/lib.rs` muss in den ersten Zeilen den Marker
//! `Safety classification: **{SAFE|STANDARD|COMFORT|TOOLING}**` tragen.
//!
//! Damit ist die Klassifikation aus [`scanner`](crate::scanner) garantiert
//! vorhanden — Voraussetzung fuer die anderen Klassen-abhaengigen Lints.

use crate::diagnostic::Diagnostic;
use crate::scanner::CrateInfo;

use super::CrateLint;

/// Lint-Implementierung.
pub struct SafetyClassificationPresent;

const NAME: &str = "dds_safety_classification_present";

impl CrateLint for SafetyClassificationPresent {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, krate: &CrateInfo) -> Vec<Diagnostic> {
        let Some(lib_rs) = krate.lib_rs.as_ref() else {
            // Crates ohne lib.rs (z.B. reine Binary-Crates / tools) ueberspringen.
            return Vec::new();
        };
        if krate.classification.is_some() {
            return Vec::new();
        }
        vec![Diagnostic::error(
            lib_rs,
            1,
            1,
            NAME,
            format!(
                "Crate `{}` hat keine Klassifikation in lib.rs \
                 (erwartet: `Safety classification: **<KLASSE>**`)",
                krate.name
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classification::SafetyClass;
    use std::path::PathBuf;

    fn krate(class: Option<SafetyClass>, with_lib_rs: bool) -> CrateInfo {
        CrateInfo {
            name: "dds-test".to_string(),
            root: PathBuf::from("/tmp/x"),
            lib_rs: if with_lib_rs {
                Some(PathBuf::from("/tmp/x/src/lib.rs"))
            } else {
                None
            },
            classification: class,
            source_files: Vec::new(),
        }
    }

    #[test]
    fn classified_crate_passes() {
        let lint = SafetyClassificationPresent;
        assert!(lint.check(&krate(Some(SafetyClass::Safe), true)).is_empty());
    }

    #[test]
    fn unclassified_crate_with_lib_rs_fails() {
        let lint = SafetyClassificationPresent;
        let d = lint.check(&krate(None, true));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].lint, "dds_safety_classification_present");
    }

    #[test]
    fn crate_without_lib_rs_is_skipped() {
        let lint = SafetyClassificationPresent;
        assert!(lint.check(&krate(None, false)).is_empty());
    }
}
