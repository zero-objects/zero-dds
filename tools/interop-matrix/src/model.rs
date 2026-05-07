// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Datenmodell der Interop-Matrix.

/// Status einer Test-Zelle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Volltest bestanden.
    Pass,
    /// Teilbestanden — Detail-Note erforderlich.
    Partial,
    /// Fehlgeschlagen.
    Fail,
    /// Nicht anwendbar (Vendor unterstuetzt das Feature nicht).
    NotApplicable,
    /// Unbekannt — Test ist nicht gelaufen oder Output unklar.
    Unknown,
}

impl Status {
    /// Maschinenlesbares Label (in JSON benutzt).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Partial => "partial",
            Self::Fail => "fail",
            Self::NotApplicable => "na",
            Self::Unknown => "unknown",
        }
    }

    /// Parsed das JSON-Label. Returnt `Unknown` bei unbekanntem.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "pass" => Self::Pass,
            "partial" => Self::Partial,
            "fail" => Self::Fail,
            "na" | "n/a" | "not_applicable" => Self::NotApplicable,
            _ => Self::Unknown,
        }
    }

    /// CSS-Klassenname fuer die HTML-Tabelle.
    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Pass => "cell-pass",
            Self::Partial => "cell-partial",
            Self::Fail => "cell-fail",
            Self::NotApplicable => "cell-na",
            Self::Unknown => "cell-unknown",
        }
    }
}

/// Eine Zelle in der Matrix: ein Test-Ergebnis mit optionaler Note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Status.
    pub status: Status,
    /// Optionale Detail-Note (z.B. Failure-Reason).
    pub note: Option<String>,
}

/// Eine Vendor-Zeile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorRow {
    /// Vendor-Name.
    pub name: String,
    /// Version-String.
    pub version: String,
    /// Profile-name → Cell.
    pub results: Vec<(String, Cell)>,
}

/// Komplette Matrix.
#[derive(Clone, Debug)]
pub struct Matrix {
    /// ISO-8601 Generated-Time.
    pub generated_at: String,
    /// Optional: git-SHA des Test-Runs.
    pub git_sha: Option<String>,
    /// Profile-Spaltenkoepfe in Sortierung.
    pub profiles: Vec<String>,
    /// Vendor-Zeilen.
    pub vendors: Vec<VendorRow>,
}

impl Matrix {
    /// Anzahl roter Zellen — Eingabe fuer Regression-Alarm.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        self.vendors
            .iter()
            .flat_map(|v| v.results.iter())
            .filter(|(_, c)| c.status == Status::Fail)
            .count()
    }

    /// `true` wenn die Matrix ueberhaupt rote Zellen hat.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.fail_count() > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        for s in [
            Status::Pass,
            Status::Partial,
            Status::Fail,
            Status::NotApplicable,
            Status::Unknown,
        ] {
            assert_eq!(Status::parse(s.as_str()), s);
        }
    }

    #[test]
    fn status_parses_aliases_for_na() {
        assert_eq!(Status::parse("n/a"), Status::NotApplicable);
        assert_eq!(Status::parse("not_applicable"), Status::NotApplicable);
    }

    #[test]
    fn fail_count_counts_only_red() {
        let m = Matrix {
            generated_at: "x".into(),
            git_sha: None,
            profiles: vec!["a".into()],
            vendors: vec![VendorRow {
                name: "v".into(),
                version: "1".into(),
                results: vec![
                    (
                        "a".into(),
                        Cell {
                            status: Status::Fail,
                            note: None,
                        },
                    ),
                    (
                        "b".into(),
                        Cell {
                            status: Status::Pass,
                            note: None,
                        },
                    ),
                ],
            }],
        };
        assert_eq!(m.fail_count(), 1);
        assert!(m.has_failures());
    }
}
