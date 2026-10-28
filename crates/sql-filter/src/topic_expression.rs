// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `TopicExpression` and `QueryExpression` of DDS 1.4 Annex B.2.
//!
//! ```text
//! TopicExpression ::= SelectFrom {Where} ';'
//! QueryExpression ::= {Condition} {'ORDER BY' (FIELDNAME // ',')}
//! SelectFrom      ::= 'SELECT' Aggregation 'FROM' Selection
//! Aggregation     ::= '*' | (SubjectFieldSpec // ',')
//! SubjectFieldSpec::= FIELDNAME | FIELDNAME 'AS' FIELDNAME | FIELDNAME FIELDNAME
//! Selection       ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
//! JoinItem        ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
//!                   | '(' TOPICNAME NaturalJoin JoinItem ')'
//! NaturalJoin     ::= 'INNER NATURAL JOIN' | 'NATURAL JOIN' | 'NATURAL INNER JOIN'
//! Where           ::= 'WHERE' Condition
//! ```
//!
//! Anything outside it is an error: the expression is refused, a clause is
//! never skipped. The one liberty is the closing `;`, which Annex B's own
//! example (B.2, last page) leaves out: it is optional.

use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::Expr;
use crate::parser::{ParseError, parse};

/// The `SELECT` list of a [`TopicExpression`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Select {
    /// `*`: every field of the joined topics.
    All,
    /// The fields to keep, in the order written.
    Fields(Vec<SubjectField>),
}

/// One `SubjectFieldSpec`: a field, and the name it has in the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectField {
    /// The field of the joined topics, dotted for a nested member.
    pub field: String,
    /// `AS name` or the juxtaposed name; `None` keeps the field's name.
    pub alias: Option<String>,
}

impl SubjectField {
    /// The name the field has in the result.
    #[must_use]
    pub fn name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.field)
    }
}

/// A parsed `TopicExpression` (the subscription expression of a MultiTopic,
/// DDS 1.4 §2.2.2.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct TopicExpression {
    /// The fields kept.
    pub select: Select,
    /// The topics of the `FROM` clause, in the order written. Every
    /// `NaturalJoin` joins the topics before it with the one after it.
    pub from: Vec<String>,
    /// The `WHERE` condition.
    pub condition: Option<Expr>,
}

/// A parsed `QueryExpression` (DDS 1.4 §2.2.2.5.9).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryExpression {
    /// The condition; `None` when the expression only orders.
    pub condition: Option<Expr>,
    /// The fields of `ORDER BY`, in the order written.
    pub order_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// A run of letters, digits, `_`, `.` and `-`.
    Word(String),
    /// A single-quoted name.
    Quoted(String),
    Star,
    Comma,
    LParen,
    RParen,
    Semi,
}

fn err(message: impl Into<String>) -> ParseError {
    ParseError(message.into())
}

/// Splits `input` into tokens until the keyword `stop` (outside quotes),
/// whose end is returned with them, or the end of the input.
fn scan(input: &str, stop: &[&str]) -> Result<(Vec<Tok>, Option<usize>), ParseError> {
    let mut out = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        match c {
            '*' => out.push(Tok::Star),
            ',' => out.push(Tok::Comma),
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            ';' => out.push(Tok::Semi),
            '\'' => {
                let mut text = String::new();
                let mut closed = false;
                for (_, d) in chars.by_ref() {
                    match d {
                        '\'' => {
                            closed = true;
                            break;
                        }
                        '\n' | '\r' => {
                            return Err(err("a quoted name may not contain a new-line"));
                        }
                        other => text.push(other),
                    }
                }
                if !closed {
                    return Err(err("unterminated quoted name"));
                }
                out.push(Tok::Quoted(text));
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' => {
                let mut end = i + c.len_utf8();
                while let Some(&(j, d)) = chars.peek() {
                    if d.is_ascii_alphanumeric() || d == '_' || d == '.' || d == '-' {
                        end = j + d.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let word = &input[i..end];
                if stop.iter().any(|k| word.eq_ignore_ascii_case(k)) {
                    return Ok((out, Some(end)));
                }
                out.push(Tok::Word(word.into()));
            }
            other => {
                return Err(err(alloc::format!(
                    "unexpected character '{other}' at position {i}"
                )));
            }
        }
    }
    Ok((out, None))
}

struct Cursor {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Cursor {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn peek_n(&self, n: usize) -> Option<&Tok> {
        self.tokens.get(self.pos + n)
    }

    fn is_keyword(&self, n: usize, keyword: &str) -> bool {
        matches!(self.peek_n(n), Some(Tok::Word(w)) if w.eq_ignore_ascii_case(keyword))
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        if self.is_keyword(0, keyword) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err(alloc::format!("expected {keyword}")))
        }
    }

    /// The length of a `NaturalJoin` at the cursor, 0 when there is none.
    fn natural_join_len(&self) -> usize {
        if self.is_keyword(0, "NATURAL") && self.is_keyword(1, "JOIN") {
            2
        } else if (self.is_keyword(0, "INNER")
            && self.is_keyword(1, "NATURAL")
            && self.is_keyword(2, "JOIN"))
            || (self.is_keyword(0, "NATURAL")
                && self.is_keyword(1, "INNER")
                && self.is_keyword(2, "JOIN"))
        {
            3
        } else {
            0
        }
    }
}

/// The words that structure a topic expression; a name may not be one.
const RESERVED: [&str; 7] = ["SELECT", "FROM", "WHERE", "AS", "NATURAL", "INNER", "JOIN"];

fn is_reserved(word: &str) -> bool {
    RESERVED.iter().any(|k| word.eq_ignore_ascii_case(k))
}

/// FIELDNAME: names joined by single dots; the first one starts with a
/// letter or `_`.
fn check_fieldname(word: &str) -> Result<(), ParseError> {
    let ok = !word.is_empty()
        && word.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        && word
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if ok && !is_reserved(word) {
        Ok(())
    } else {
        Err(err(alloc::format!("'{word}' is not a FIELDNAME")))
    }
}

/// TOPICNAME: "any series of characters 'a', …, 'z', 'A', …, 'Z', '0', …,
/// '9', '-' but may not start with a digit"; a quoted name is any text.
fn topic_name(cursor: &mut Cursor) -> Result<String, ParseError> {
    match cursor.peek().cloned() {
        Some(Tok::Quoted(name)) if !name.is_empty() => {
            cursor.pos += 1;
            Ok(name)
        }
        Some(Tok::Word(word)) => {
            let ok = word.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !word.starts_with(|c: char| c.is_ascii_digit())
                && !is_reserved(&word);
            if !ok {
                return Err(err(alloc::format!(
                    "'{word}' is not a TOPICNAME (letters, digits and '-', not starting with a \
                     digit; quote any other name)"
                )));
            }
            cursor.pos += 1;
            Ok(word)
        }
        _ => Err(err("expected a topic name")),
    }
}

/// The deepest nesting of parenthesized join items read.
const MAX_JOIN_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (`MAX_JOIN_DEPTH`, checked on entry)
fn join_item(cursor: &mut Cursor, from: &mut Vec<String>, depth: usize) -> Result<(), ParseError> {
    if depth > MAX_JOIN_DEPTH {
        return Err(err("join items are nested too deep"));
    }
    if matches!(cursor.peek(), Some(Tok::LParen)) {
        cursor.pos += 1;
        from.push(topic_name(cursor)?);
        let n = cursor.natural_join_len();
        if n == 0 {
            return Err(err(
                "a parenthesized join item joins: expected NATURAL JOIN",
            ));
        }
        cursor.pos += n;
        join_item(cursor, from, depth + 1)?;
        if !matches!(cursor.peek(), Some(Tok::RParen)) {
            return Err(err("expected ')'"));
        }
        cursor.pos += 1;
        return Ok(());
    }
    from.push(topic_name(cursor)?);
    let n = cursor.natural_join_len();
    if n > 0 {
        cursor.pos += n;
        join_item(cursor, from, depth + 1)?;
    }
    Ok(())
}

fn subject_field(cursor: &mut Cursor) -> Result<SubjectField, ParseError> {
    let Some(Tok::Word(field)) = cursor.peek().cloned() else {
        return Err(err("expected a FIELDNAME"));
    };
    check_fieldname(&field)?;
    cursor.pos += 1;
    let alias = if cursor.is_keyword(0, "AS") {
        cursor.pos += 1;
        let Some(Tok::Word(alias)) = cursor.peek().cloned() else {
            return Err(err("expected a FIELDNAME after AS"));
        };
        check_fieldname(&alias)?;
        cursor.pos += 1;
        Some(alias)
    } else if let Some(Tok::Word(alias)) = cursor.peek().cloned() {
        if cursor.is_keyword(0, "FROM") {
            None
        } else {
            check_fieldname(&alias)?;
            cursor.pos += 1;
            Some(alias)
        }
    } else {
        None
    };
    Ok(SubjectField { field, alias })
}

/// Parses a `TopicExpression`.
///
/// # Errors
/// [`ParseError`] for anything that is not Annex B.2's `TopicExpression`:
/// a missing clause, a name that is not a FIELDNAME or TOPICNAME, a topic
/// twice in the `FROM` clause, two result fields of one name, a `WHERE`
/// condition that is not a `Condition`, and text after the expression.
pub fn parse_topic_expression(input: &str) -> Result<TopicExpression, ParseError> {
    let (tokens, stopped) = scan(input, &["WHERE"])?;
    let mut cursor = Cursor { tokens, pos: 0 };
    cursor.expect_keyword("SELECT")?;
    let select = if matches!(cursor.peek(), Some(Tok::Star)) {
        cursor.pos += 1;
        Select::All
    } else {
        let mut fields = alloc::vec![subject_field(&mut cursor)?];
        while matches!(cursor.peek(), Some(Tok::Comma)) {
            cursor.pos += 1;
            fields.push(subject_field(&mut cursor)?);
        }
        for (i, field) in fields.iter().enumerate() {
            if fields[..i]
                .iter()
                .any(|earlier| earlier.name() == field.name())
            {
                return Err(err(alloc::format!(
                    "two fields are called '{}' in the result",
                    field.name()
                )));
            }
        }
        Select::Fields(fields)
    };
    cursor.expect_keyword("FROM")?;
    let mut from = Vec::new();
    from.push(topic_name(&mut cursor)?);
    let n = cursor.natural_join_len();
    if n > 0 {
        cursor.pos += n;
        join_item(&mut cursor, &mut from, 0)?;
    }
    for (i, topic) in from.iter().enumerate() {
        if from[..i].contains(topic) {
            return Err(err(alloc::format!(
                "the topic '{topic}' is in the FROM clause twice"
            )));
        }
    }
    let condition = match stopped {
        Some(end) => {
            // Only an optional ';' may stand between the join and WHERE's
            // condition; the tokens up to WHERE have all been read.
            if cursor.pos != cursor.tokens.len() {
                return Err(err("unexpected text before WHERE"));
            }
            let rest = input[end..].trim();
            let rest = rest.strip_suffix(';').unwrap_or(rest).trim();
            if rest.is_empty() {
                return Err(err("WHERE needs a condition"));
            }
            Some(parse(rest)?)
        }
        None => {
            if matches!(cursor.peek(), Some(Tok::Semi)) {
                cursor.pos += 1;
            }
            if cursor.pos != cursor.tokens.len() {
                return Err(err("unexpected text after the FROM clause"));
            }
            None
        }
    };
    Ok(TopicExpression {
        select,
        from,
        condition,
    })
}

/// Parses a `QueryExpression`: an optional condition, then an optional
/// `ORDER BY` with one or more FIELDNAMEs.
///
/// # Errors
/// [`ParseError`] for a condition that is not a `Condition`, an `ORDER BY`
/// without fields or with a name that is not a FIELDNAME, a field twice in
/// `ORDER BY`, and text after the last field.
pub fn parse_query_expression(input: &str) -> Result<QueryExpression, ParseError> {
    let stopped = scan_order_by(input)?;
    let (condition_text, order_text) = match stopped {
        Some((by_end, order_start)) => (&input[..order_start], Some(&input[by_end..])),
        None => (input, None),
    };
    let condition_text = condition_text.trim();
    let condition = if condition_text.is_empty() {
        None
    } else {
        Some(parse(condition_text)?)
    };
    let mut order_by = Vec::new();
    if let Some(text) = order_text {
        let (tokens, _) = scan(text, &[])?;
        let mut cursor = Cursor { tokens, pos: 0 };
        loop {
            let Some(Tok::Word(field)) = cursor.peek().cloned() else {
                return Err(err("ORDER BY needs a FIELDNAME"));
            };
            check_fieldname(&field)?;
            if order_by.contains(&field) {
                return Err(err(alloc::format!("'{field}' is in ORDER BY twice")));
            }
            order_by.push(field);
            cursor.pos += 1;
            if matches!(cursor.peek(), Some(Tok::Comma)) {
                cursor.pos += 1;
                continue;
            }
            break;
        }
        if cursor.pos != cursor.tokens.len() {
            return Err(err("unexpected text after ORDER BY"));
        }
    }
    Ok(QueryExpression {
        condition,
        order_by,
    })
}

/// Finds `ORDER BY` outside quotes: the end of `BY` and the start of
/// `ORDER`, when the expression has one.
fn scan_order_by(input: &str) -> Result<Option<(usize, usize)>, ParseError> {
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut in_quote = false;
    let mut last_word: Option<(usize, usize)> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if in_quote {
            if b == b'\'' {
                in_quote = false;
            }
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_quote = true;
            last_word = None;
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &input[start..i];
            // A word that is part of a dotted or subscripted name is no keyword.
            let attached = start > 0 && matches!(bytes[start - 1], b'.' | b'[' | b'%');
            if !attached {
                if word.eq_ignore_ascii_case("ORDER") {
                    last_word = Some((start, i));
                } else if word.eq_ignore_ascii_case("BY") {
                    if let Some((order_start, order_end)) = last_word {
                        if input[order_end..start].chars().all(char::is_whitespace) {
                            return Ok(Some((i, order_start)));
                        }
                    }
                    last_word = None;
                } else {
                    last_word = None;
                }
            } else {
                last_word = None;
            }
            continue;
        }
        if !b.is_ascii_whitespace() {
            last_word = None;
        }
        i += 1;
    }
    if in_quote {
        return Err(err("unterminated string literal"));
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn fields(e: &TopicExpression) -> Vec<(String, Option<String>)> {
        match &e.select {
            Select::Fields(f) => f
                .iter()
                .map(|f| (f.field.clone(), f.alias.clone()))
                .collect(),
            Select::All => panic!("expected a field list"),
        }
    }

    /// The example of Annex B.2: quoted topic names, `AS`, a condition over
    /// the alias.
    #[test]
    fn the_annex_b_example_parses() {
        let e = parse_topic_expression(
            "SELECT flight_name, x, y, z AS height FROM 'Location' NATURAL JOIN 'FlightPlan' \
             WHERE height < 1000 AND x <23",
        )
        .unwrap();
        assert_eq!(
            fields(&e),
            vec![
                ("flight_name".into(), None),
                ("x".into(), None),
                ("y".into(), None),
                ("z".into(), Some("height".into())),
            ]
        );
        assert_eq!(
            e.from,
            vec!["Location".to_string(), "FlightPlan".to_string()]
        );
        assert!(matches!(e.condition, Some(Expr::And(_, _))));
    }

    #[test]
    fn star_one_topic_no_where_and_the_closing_semicolon() {
        let e = parse_topic_expression("SELECT * FROM Location;").unwrap();
        assert_eq!(e.select, Select::All);
        assert_eq!(e.from, vec!["Location".to_string()]);
        assert!(e.condition.is_none());
        let e = parse_topic_expression("select * from Location where x > 1;").unwrap();
        assert!(e.condition.is_some());
    }

    #[test]
    fn the_three_spellings_of_the_natural_join() {
        for join in [
            "INNER NATURAL JOIN",
            "NATURAL JOIN",
            "NATURAL INNER JOIN",
            "natural join",
        ] {
            let e = parse_topic_expression(&format!("SELECT * FROM A {join} B")).unwrap();
            assert_eq!(e.from, vec!["A".to_string(), "B".to_string()], "{join}");
        }
    }

    #[test]
    fn join_items_may_nest_in_parentheses() {
        let e = parse_topic_expression("SELECT * FROM A NATURAL JOIN (B NATURAL JOIN C)").unwrap();
        assert_eq!(
            e.from,
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
        let e = parse_topic_expression("SELECT * FROM A NATURAL JOIN B NATURAL JOIN C").unwrap();
        assert_eq!(e.from.len(), 3);
        // A parenthesized item that does not join is not a JoinItem.
        assert!(parse_topic_expression("SELECT * FROM A NATURAL JOIN (B)").is_err());
    }

    #[test]
    fn a_field_takes_an_alias_with_as_or_juxtaposed() {
        let e = parse_topic_expression("SELECT a.b AS c, d e FROM T").unwrap();
        assert_eq!(
            fields(&e),
            vec![
                ("a.b".into(), Some("c".into())),
                ("d".into(), Some("e".into())),
            ]
        );
    }

    #[test]
    fn a_topic_name_is_letters_digits_and_hyphen_or_quoted() {
        assert!(parse_topic_expression("SELECT * FROM my-topic1").is_ok());
        assert!(parse_topic_expression("SELECT * FROM 1topic").is_err());
        assert!(parse_topic_expression("SELECT * FROM my_topic").is_err());
        let e = parse_topic_expression("SELECT * FROM 'rt/chatter'").unwrap();
        assert_eq!(e.from, vec!["rt/chatter".to_string()]);
    }

    /// Everything outside Annex B.2 is refused, not skipped.
    #[test]
    fn anything_else_is_refused() {
        for bad in [
            "",
            "x > 1",
            "SELECT FROM T",
            "SELECT * T",
            "SELECT * FROM",
            "SELECT * FROM T WHERE",
            "SELECT * FROM T WHERE x >",
            "SELECT * FROM T ORDER BY x",
            "SELECT * FROM T GROUP BY x",
            "SELECT * FROM A JOIN B",
            "SELECT * FROM A LEFT NATURAL JOIN B",
            "SELECT * FROM A, B",
            "SELECT * FROM A NATURAL JOIN A",
            "SELECT a, a FROM T",
            "SELECT a AS b, c AS b FROM T",
            "SELECT 1 FROM T",
            "SELECT a, FROM T",
            "SELECT * FROM T;;",
            "SELECT * FROM T; SELECT * FROM U",
            "SELECT * FROM T extra",
            "SELECT * FROM A NATURAL JOIN",
            "SELECT * FROM A NATURAL JOIN (B NATURAL JOIN (C NATURAL JOIN",
            "SELECT * FROM (A NATURAL JOIN B)",
            "SELECT * FROM T WHERE x > 1 ORDER BY x",
        ] {
            assert!(parse_topic_expression(bad).is_err(), "{bad}");
        }
    }

    /// Nesting is bounded: a deep expression is refused, not recursed into.
    #[test]
    fn join_items_nest_to_a_bound() {
        let nested = |depth: usize| {
            let mut text = String::from("SELECT * FROM T0");
            for i in 0..depth {
                text.push_str(&format!(" NATURAL JOIN (T{}", i + 1));
            }
            text.push_str(" NATURAL JOIN Z");
            text.push_str(&")".repeat(depth));
            text
        };
        assert!(parse_topic_expression(&nested(10)).is_ok());
        assert!(parse_topic_expression(&nested(200)).is_err());
    }

    #[test]
    fn where_in_a_string_is_not_the_keyword() {
        let e = parse_topic_expression("SELECT * FROM T WHERE name = 'WHERE'").unwrap();
        assert!(e.condition.is_some());
    }

    #[test]
    fn a_query_expression_is_a_condition_and_an_order() {
        let q = parse_query_expression("x > 1 ORDER BY a, b.c").unwrap();
        assert!(q.condition.is_some());
        assert_eq!(q.order_by, vec!["a".to_string(), "b.c".to_string()]);
        let q = parse_query_expression("ORDER BY a").unwrap();
        assert!(q.condition.is_none());
        assert_eq!(q.order_by, vec!["a".to_string()]);
        let q = parse_query_expression("x > 1").unwrap();
        assert!(q.order_by.is_empty());
        let q = parse_query_expression("name = 'ORDER BY'").unwrap();
        assert!(q.order_by.is_empty());
        let q = parse_query_expression("").unwrap();
        assert!(q.condition.is_none() && q.order_by.is_empty());
    }

    #[test]
    fn a_malformed_query_expression_is_refused() {
        for bad in [
            "ORDER BY",
            "ORDER BY a,",
            "ORDER BY a b",
            "ORDER BY a, a",
            "x > ORDER BY a",
            "ORDER BY 1",
        ] {
            assert!(parse_query_expression(bad).is_err(), "{bad}");
        }
    }
}
