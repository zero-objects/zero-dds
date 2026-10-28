// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Printing diagnostics, in the two forms a caller asks for.
//!
//! Text for a person, JSON for whatever reads the tool's output. Both
//! render the *same* diagnostics from the same place, so a rule that
//! fires in one and not the other is not a shape this file allows.

use zerodds_idl_parse::{Diagnostic, Severity};

use crate::source::{Location, Unit};

/// How a run ends.
///
/// The count is what a caller wants in the summary line, and the exit
/// code is derived from `errors` alone: a warning is worth printing and
/// not worth failing a build over, which is the distinction
/// `Severity` already draws.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    /// Diagnostics at `Severity::Error`.
    pub errors: usize,
    /// Diagnostics at `Severity::Warning`.
    pub warnings: usize,
}

impl Tally {
    /// Count one diagnostic.
    pub fn add(&mut self, diagnostic: &Diagnostic) {
        match diagnostic.severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
        }
    }

    /// Fold another tally in.
    pub fn merge(&mut self, other: Self) {
        self.errors += other.errors;
        self.warnings += other.warnings;
    }

    /// Whether anything failed.
    #[must_use]
    pub const fn failed(&self) -> bool {
        self.errors > 0
    }
}

/// Which form the output takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Form {
    /// One line per diagnostic, for a person.
    #[default]
    Text,
    /// One JSON object per run, for whatever reads it.
    Json,
}

/// Print `diagnostics` against `unit` and count them.
pub fn emit(unit: &Unit, diagnostics: &[Diagnostic], form: Form) -> Tally {
    let mut tally = Tally::default();
    for diagnostic in diagnostics {
        tally.add(diagnostic);
    }
    match form {
        Form::Text => print_text(unit, diagnostics),
        Form::Json => print_json(unit, diagnostics),
    }
    tally
}

fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// The text of the file a diagnostic points into: the unit's, or an
/// included file read once for this purpose.
struct Files<'a> {
    unit: &'a Unit,
    read: std::collections::BTreeMap<String, Option<String>>,
}

impl<'a> Files<'a> {
    fn new(unit: &'a Unit) -> Self {
        Self {
            unit,
            read: std::collections::BTreeMap::new(),
        }
    }

    /// The path to print and the text the spans index, for `diagnostic`.
    /// The text is `None` when an included file can no longer be read;
    /// the caller prints byte offsets then.
    fn of(&mut self, diagnostic: &Diagnostic) -> (String, Option<&str>) {
        match diagnostic.file.as_deref() {
            None => (self.unit.path.clone(), Some(self.unit.text.as_str())),
            Some(path) if path == self.unit.path => {
                (self.unit.path.clone(), Some(self.unit.text.as_str()))
            }
            Some(path) => {
                let text = self
                    .read
                    .entry(path.to_string())
                    .or_insert_with(|| std::fs::read_to_string(path).ok());
                (path.to_string(), text.as_deref())
            }
        }
    }
}

/// `line:column`, or the byte offset when the file's text is gone.
fn place(text: Option<&str>, span: zerodds_idl_cst::Span) -> String {
    match text {
        Some(text) => Location::start_of(text, span).to_string(),
        None => format!("byte {}", span.start),
    }
}

fn print_text(unit: &Unit, diagnostics: &[Diagnostic]) {
    let mut files = Files::new(unit);
    for diagnostic in diagnostics {
        let (path, text) = files.of(diagnostic);
        let at = place(text, diagnostic.primary);
        println!(
            "{path}:{at}: {}: [{}] {}",
            severity_word(diagnostic.severity),
            diagnostic.code.as_str(),
            diagnostic.message
        );
        // The paragraph the rule comes from, where the rule has one.
        // A diagnostic without one is ZeroDDS policy and says so in the
        // message, so printing an empty `note:` would be noise.
        if !diagnostic.spec_reference.is_empty() {
            println!("  note: {}", diagnostic.spec_reference);
        }
        for (span, note) in &diagnostic.labels {
            let label_at = place(text, *span);
            println!("  {label_at}: {note}");
        }
    }
}

/// Escape a string for a JSON document.
///
/// Written out rather than pulled in: the tool has no serde dependency
/// and this is the only place it would need one. The escapes are the
/// ones RFC 8259 §7 requires — the two structural characters, the
/// backslash, and every control character below `0x20`.
fn json_string(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn print_json(unit: &Unit, diagnostics: &[Diagnostic]) {
    let mut files = Files::new(unit);
    let mut out = String::from("{\"file\":");
    json_string(&unit.path, &mut out);
    out.push_str(",\"diagnostics\":[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let (path, text) = files.of(diagnostic);
        let at = text.map_or(Location { line: 0, column: 0 }, |text| {
            Location::start_of(text, diagnostic.primary)
        });
        out.push_str("{\"path\":");
        json_string(&path, &mut out);
        out.push_str(",\"severity\":");
        json_string(severity_word(diagnostic.severity), &mut out);
        out.push_str(",\"code\":");
        json_string(diagnostic.code.as_str(), &mut out);
        out.push_str(",\"message\":");
        json_string(&diagnostic.message, &mut out);
        out.push_str(",\"spec\":");
        json_string(diagnostic.spec_reference, &mut out);
        out.push_str(&format!(
            ",\"line\":{},\"column\":{},\"start\":{},\"end\":{}}}",
            at.line, at.column, diagnostic.primary.start, diagnostic.primary.end
        ));
    }
    out.push_str("]}");
    println!("{out}");
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn a_warning_does_not_fail_a_run() {
        let warned = Tally {
            warnings: 3,
            ..Tally::default()
        };
        assert!(!warned.failed());
        let failed = Tally {
            errors: 1,
            ..warned
        };
        assert!(failed.failed());
    }

    #[test]
    fn merging_adds_both_counts() {
        let mut a = Tally {
            errors: 1,
            warnings: 2,
        };
        a.merge(Tally {
            errors: 3,
            warnings: 4,
        });
        assert_eq!(
            a,
            Tally {
                errors: 4,
                warnings: 6
            }
        );
    }

    /// A message with a quote in it must not end the JSON string early —
    /// an IDL diagnostic quotes identifiers, so this is the common case
    /// and not an edge one.
    #[test]
    fn a_quoted_identifier_survives_json() {
        let mut out = String::new();
        json_string("`pose` and `Pose` differ only in case", &mut out);
        assert_eq!(out, "\"`pose` and `Pose` differ only in case\"");

        let mut out = String::new();
        json_string("he said \"no\"\\ever", &mut out);
        assert_eq!(out, "\"he said \\\"no\\\"\\\\ever\"");
    }

    #[test]
    fn a_control_character_becomes_an_escape() {
        let mut out = String::new();
        json_string("a\u{1}b\nc", &mut out);
        assert_eq!(out, "\"a\\u0001b\\nc\"");
    }
}
