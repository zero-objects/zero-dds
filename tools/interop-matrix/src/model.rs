// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Data model of the interop matrix.

/// Status of a test cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Full test passed.
    Pass,
    /// Partially passed — detail note required.
    Partial,
    /// Failed.
    Fail,
    /// Not applicable (vendor does not support the feature).
    NotApplicable,
    /// Unknown — test did not run or output is unclear.
    Unknown,
}

impl Status {
    /// Machine-readable label (used in JSON).
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

    /// Parses the JSON label. Returns `Unknown` on unknown.
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

    /// CSS class name for the HTML table.
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

/// A cell in the matrix: a test result with an optional note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Status.
    pub status: Status,
    /// Optional detail note (e.g. failure reason).
    pub note: Option<String>,
}

/// A vendor row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorRow {
    /// Vendor-Name.
    pub name: String,
    /// Version-String.
    pub version: String,
    /// Profile-name → Cell.
    pub results: Vec<(String, Cell)>,
}

/// Complete matrix.
#[derive(Clone, Debug)]
pub struct Matrix {
    /// ISO-8601 Generated-Time.
    pub generated_at: String,
    /// Optional: git SHA of the test run.
    pub git_sha: Option<String>,
    /// Profile column headers in sort order.
    pub profiles: Vec<String>,
    /// Vendor rows.
    pub vendors: Vec<VendorRow>,
}

impl Matrix {
    /// Number of red cells — input for the regression alarm.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        self.vendors
            .iter()
            .flat_map(|v| v.results.iter())
            .filter(|(_, c)| c.status == Status::Fail)
            .count()
    }

    /// `true` if the matrix has any red cells at all.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.fail_count() > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
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
