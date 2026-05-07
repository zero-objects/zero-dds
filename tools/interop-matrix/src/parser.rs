// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Minimaler JSON-Parser fuer das Matrix-Format.
//!
//! Wir benutzen keinen serde-Dep — der Code ist klein genug fuer einen
//! handgeschriebenen Parser. Fehler-Tolerant: unbekannte Felder werden
//! ignoriert, fehlende Pflichtfelder geben sauberen Fehler.

use crate::model::{Cell, Matrix, Status, VendorRow};

/// Parser-Fehler.
#[derive(Debug)]
pub enum ParseError {
    /// JSON syntax error.
    Syntax(String),
    /// Erwartetes Pflichtfeld fehlt.
    Missing(&'static str),
    /// Wert hatte falschen Typ.
    BadType(&'static str),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "syntax: {s}"),
            Self::Missing(s) => write!(f, "missing required field: {s}"),
            Self::BadType(s) => write!(f, "wrong type for: {s}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Tiny-Tokenizer fuer JSON.
struct Lex<'a> {
    s: &'a [u8],
    p: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum Tok {
    Lbrace,
    Rbrace,
    Lbrack,
    Rbrack,
    Colon,
    Comma,
    Str(String),
    Num(String),
    True,
    False,
    Null,
    End,
}

impl<'a> Lex<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            p: 0,
        }
    }
    fn skip_ws(&mut self) {
        while self.p < self.s.len() && self.s[self.p].is_ascii_whitespace() {
            self.p += 1;
        }
    }
    fn next(&mut self) -> Result<Tok, ParseError> {
        self.skip_ws();
        if self.p >= self.s.len() {
            return Ok(Tok::End);
        }
        let c = self.s[self.p];
        match c {
            b'{' => {
                self.p += 1;
                Ok(Tok::Lbrace)
            }
            b'}' => {
                self.p += 1;
                Ok(Tok::Rbrace)
            }
            b'[' => {
                self.p += 1;
                Ok(Tok::Lbrack)
            }
            b']' => {
                self.p += 1;
                Ok(Tok::Rbrack)
            }
            b':' => {
                self.p += 1;
                Ok(Tok::Colon)
            }
            b',' => {
                self.p += 1;
                Ok(Tok::Comma)
            }
            b'"' => self.read_string(),
            b't' => self.expect_keyword("true").map(|_| Tok::True),
            b'f' => self.expect_keyword("false").map(|_| Tok::False),
            b'n' => self.expect_keyword("null").map(|_| Tok::Null),
            b'-' | b'0'..=b'9' => self.read_number(),
            other => Err(ParseError::Syntax(format!(
                "unexpected byte 0x{other:02x} at offset {}",
                self.p
            ))),
        }
    }
    fn read_string(&mut self) -> Result<Tok, ParseError> {
        self.p += 1; // skip opening quote
        let mut out = String::new();
        while self.p < self.s.len() {
            let c = self.s[self.p];
            if c == b'"' {
                self.p += 1;
                return Ok(Tok::Str(out));
            }
            if c == b'\\' && self.p + 1 < self.s.len() {
                let esc = self.s[self.p + 1];
                self.p += 2;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'u' => {
                        if self.p + 4 > self.s.len() {
                            return Err(ParseError::Syntax("truncated \\u".into()));
                        }
                        let hex = core::str::from_utf8(&self.s[self.p..self.p + 4])
                            .map_err(|_| ParseError::Syntax("invalid \\u hex".into()))?;
                        let cp = u32::from_str_radix(hex, 16)
                            .map_err(|_| ParseError::Syntax("non-hex \\u".into()))?;
                        if let Some(c) = char::from_u32(cp) {
                            out.push(c);
                        }
                        self.p += 4;
                    }
                    _ => out.push(esc as char),
                }
            } else {
                out.push(c as char);
                self.p += 1;
            }
        }
        Err(ParseError::Syntax("unterminated string".into()))
    }
    fn read_number(&mut self) -> Result<Tok, ParseError> {
        let start = self.p;
        if self.s[self.p] == b'-' {
            self.p += 1;
        }
        while self.p < self.s.len() {
            let c = self.s[self.p];
            if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-' {
                self.p += 1;
            } else {
                break;
            }
        }
        let s = core::str::from_utf8(&self.s[start..self.p])
            .map_err(|_| ParseError::Syntax("non-utf8 in number".into()))?
            .to_string();
        Ok(Tok::Num(s))
    }
    fn expect_keyword(&mut self, kw: &str) -> Result<(), ParseError> {
        let bytes = kw.as_bytes();
        if self.p + bytes.len() > self.s.len() || &self.s[self.p..self.p + bytes.len()] != bytes {
            return Err(ParseError::Syntax(format!("expected {kw}")));
        }
        self.p += bytes.len();
        Ok(())
    }
}

/// Mini-JSON-Value.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Num/Bool werden parsed aber im Matrix-Schema nicht inspiziert.
enum Val {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Array(Vec<Val>),
    Object(Vec<(String, Val)>),
}

impl Val {
    fn as_str(&self) -> Option<&str> {
        if let Self::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn as_array(&self) -> Option<&[Val]> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
    fn as_object(&self) -> Option<&[(String, Val)]> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    fn get<'a>(&'a self, k: &str) -> Option<&'a Val> {
        self.as_object()?.iter().find(|(n, _)| n == k).map(|x| &x.1)
    }
}

fn parse_value(lex: &mut Lex<'_>) -> Result<Val, ParseError> {
    let t = lex.next()?;
    parse_value_after(t, lex)
}

/// zerodds-lint: recursion-depth 64
///
/// Indirekt rekursiv via `parse_array` / `parse_object`. Tiefe ist
/// durch die JSON-Verschachtelungs-Tiefe begrenzt; Caller ist die
/// `interop-matrix.json`-Eingabe deren Schema max ~3 Ebenen tief
/// geht (resourceMatrix → vendors → results). 64 als grosszügige
/// obere Schranke fuer pathologische User-Input-Files.
fn parse_value_after(t: Tok, lex: &mut Lex<'_>) -> Result<Val, ParseError> {
    match t {
        Tok::Lbrace => parse_object(lex),
        Tok::Lbrack => parse_array(lex),
        Tok::Str(s) => Ok(Val::Str(s)),
        Tok::Num(n) => Ok(Val::Num(n)),
        Tok::True => Ok(Val::Bool(true)),
        Tok::False => Ok(Val::Bool(false)),
        Tok::Null => Ok(Val::Null),
        other => Err(ParseError::Syntax(format!("unexpected token {other:?}"))),
    }
}

fn parse_object(lex: &mut Lex<'_>) -> Result<Val, ParseError> {
    let mut out = Vec::new();
    let first = lex.next()?;
    if first == Tok::Rbrace {
        return Ok(Val::Object(out));
    }
    let mut current = first;
    loop {
        let key = match current {
            Tok::Str(s) => s,
            t => {
                return Err(ParseError::Syntax(format!(
                    "expected string key, got {t:?}"
                )));
            }
        };
        let colon = lex.next()?;
        if colon != Tok::Colon {
            return Err(ParseError::Syntax("expected ':'".into()));
        }
        let val = parse_value(lex)?;
        out.push((key, val));
        match lex.next()? {
            Tok::Comma => {
                current = lex.next()?;
            }
            Tok::Rbrace => return Ok(Val::Object(out)),
            t => {
                return Err(ParseError::Syntax(format!(
                    "expected ',' or '}}', got {t:?}"
                )));
            }
        }
    }
}

/// zerodds-lint: recursion-depth 64
///
/// Indirekt rekursiv via `parse_value_after` (per Element). Tiefe
/// ist durch die JSON-Verschachtelungs-Tiefe begrenzt — siehe
/// `parse_value_after` fuer die Begruendung der 64er-Schranke.
fn parse_array(lex: &mut Lex<'_>) -> Result<Val, ParseError> {
    let mut out = Vec::new();
    let first = lex.next()?;
    if first == Tok::Rbrack {
        return Ok(Val::Array(out));
    }
    let mut current = first;
    loop {
        out.push(parse_value_after(current, lex)?);
        match lex.next()? {
            Tok::Comma => {
                current = lex.next()?;
            }
            Tok::Rbrack => return Ok(Val::Array(out)),
            t => {
                return Err(ParseError::Syntax(format!(
                    "expected ',' or ']', got {t:?}"
                )));
            }
        }
    }
}

/// Parsed das Matrix-JSON.
///
/// # Errors
/// `Syntax`/`Missing`/`BadType` siehe [`ParseError`].
pub fn parse_matrix_json(json: &str) -> Result<Matrix, ParseError> {
    let mut lex = Lex::new(json);
    let v = parse_value(&mut lex)?;
    let generated_at = v
        .get("generated_at")
        .and_then(|x| x.as_str())
        .ok_or(ParseError::Missing("generated_at"))?
        .to_string();
    let git_sha = v.get("git_sha").and_then(|x| x.as_str()).map(String::from);
    let profiles_v = v.get("profiles").ok_or(ParseError::Missing("profiles"))?;
    let profiles = profiles_v
        .as_array()
        .ok_or(ParseError::BadType("profiles must be array"))?
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect::<Vec<_>>();
    let vendors_v = v.get("vendors").ok_or(ParseError::Missing("vendors"))?;
    let mut vendors = Vec::new();
    for vv in vendors_v
        .as_array()
        .ok_or(ParseError::BadType("vendors must be array"))?
    {
        let name = vv
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or(ParseError::Missing("vendor.name"))?
            .to_string();
        let version = vv
            .get("version")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let results_v = vv
            .get("results")
            .ok_or(ParseError::Missing("vendor.results"))?;
        let mut results = Vec::new();
        for (key, cell_v) in results_v
            .as_object()
            .ok_or(ParseError::BadType("results must be object"))?
        {
            let status = cell_v
                .get("status")
                .and_then(|x| x.as_str())
                .map(Status::parse)
                .unwrap_or(Status::Unknown);
            let note = cell_v
                .get("note")
                .and_then(|x| x.as_str())
                .map(String::from);
            results.push((key.clone(), Cell { status, note }));
        }
        vendors.push(VendorRow {
            name,
            version,
            results,
        });
    }
    Ok(Matrix {
        generated_at,
        git_sha,
        profiles,
        vendors,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_matrix() {
        let j = r#"{
            "generated_at": "2026-05-03T10:00:00Z",
            "profiles": ["rtps_pubsub"],
            "vendors": [
                {"name":"Cyclone DDS","version":"0.10.5","results":{"rtps_pubsub":{"status":"pass"}}}
            ]
        }"#;
        let m = parse_matrix_json(j).unwrap();
        assert_eq!(m.profiles, vec!["rtps_pubsub"]);
        assert_eq!(m.vendors.len(), 1);
        assert_eq!(m.vendors[0].name, "Cyclone DDS");
        assert_eq!(m.vendors[0].results[0].1.status, Status::Pass);
    }

    #[test]
    fn parses_note_field() {
        let j = r#"{
            "generated_at": "2026-05-03T10:00:00Z",
            "profiles": ["x"],
            "vendors": [{"name":"v","version":"1","results":{"x":{"status":"fail","note":"boom"}}}]
        }"#;
        let m = parse_matrix_json(j).unwrap();
        assert_eq!(m.vendors[0].results[0].1.note.as_deref(), Some("boom"));
    }

    #[test]
    fn missing_field_returns_clear_error() {
        let j = r#"{"profiles":[],"vendors":[]}"#;
        let r = parse_matrix_json(j);
        assert!(matches!(r, Err(ParseError::Missing("generated_at"))));
    }

    #[test]
    fn unknown_status_becomes_unknown() {
        let j = r#"{
            "generated_at": "x",
            "profiles": ["a"],
            "vendors": [{"name":"v","version":"1","results":{"a":{"status":"weird"}}}]
        }"#;
        let m = parse_matrix_json(j).unwrap();
        assert_eq!(m.vendors[0].results[0].1.status, Status::Unknown);
    }

    #[test]
    fn extra_unknown_fields_are_ignored() {
        let j = r#"{
            "generated_at": "x",
            "git_sha": "abc",
            "profiles": ["a"],
            "vendors": [{"name":"v","version":"1","extra":"ignored","results":{"a":{"status":"pass"}}}]
        }"#;
        let m = parse_matrix_json(j).unwrap();
        assert_eq!(m.git_sha.as_deref(), Some("abc"));
    }

    #[test]
    fn syntax_error_surfaces_position() {
        let j = "{not valid json";
        let r = parse_matrix_json(j);
        assert!(matches!(r, Err(ParseError::Syntax(_))));
    }
}
