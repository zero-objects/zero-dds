// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The lexer. Spec §1.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_cst::{KEYWORDS, Radix, Span, TokenKind, TokenValue, TriviaKind, lex};

fn kinds(source: &str) -> Vec<TokenKind> {
    lex(source).into_iter().map(|t| t.kind).collect()
}

/// Every byte of the input is reachable from the stream: concatenating
/// each token's trivia and its own text reproduces the source. This is
/// the property the whole lossless tree rests on.
fn reconstruct(source: &str) -> String {
    let mut out = String::new();
    for token in lex(source) {
        for trivia in &token.leading {
            out.push_str(trivia.span.slice(source).unwrap_or(""));
        }
        out.push_str(token.span.slice(source).unwrap_or(""));
    }
    out
}

// -- keywords ------------------------------------------------------------

#[test]
fn every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier() {
    for (text, expected) in KEYWORDS {
        let tokens = lex(text);
        assert_eq!(
            tokens.first().map(|t| t.kind),
            Some(*expected),
            "`{text}` did not lex as its keyword"
        );
        assert_ne!(
            tokens.first().map(|t| t.kind),
            Some(TokenKind::Ident),
            "`{text}` lexed as an identifier"
        );
    }
}

#[test]
fn a_keyword_is_recognised_only_in_the_spelling_table_7_6_gives() {
    // IDL 4.2 §7.2.4, p. 20: "Keywords must be written exactly as shown
    // in the above list. Identifiers that collide with keywords (see
    // 7.2.3, Identifiers) are illegal."
    //
    // Two rules, and folding them into one match turned the identifier
    // `CUSTOM` into the keyword `custom` — a silently wrong parse of
    // valid-looking IDL. Recognition is exact here; the collision is
    // §7.2.3's rule and the lowering reports it by name.
    assert_eq!(kinds("struct")[0], TokenKind::Struct);
    for spelling in ["STRUCT", "Struct", "sTrUcT"] {
        assert_eq!(kinds(spelling)[0], TokenKind::Ident, "`{spelling}`");
    }
    assert_eq!(kinds("custom")[0], TokenKind::Custom);
    assert_eq!(kinds("CUSTOM")[0], TokenKind::Ident);
}

#[test]
fn the_four_keywords_table_7_6_capitalises_are_spelled_as_it_spells_them() {
    // `TRUE`, `FALSE`, `Object` and `ValueBase` are the four entries of
    // Table 7-6 that are not all lower case, and rules (117), (131) and
    // the boolean literal production write them that way. Measured on
    // the Linux test host: OpenDDS and TAO answer the lower-case
    // spelling with "identifier spellings differ only in case:
    // \"object\" and \"Object\"", JacORB with "Identifier true collides
    // with keyword TRUE", omniORB with "not found" — three front ends
    // and four cases saying the capital form is the keyword.
    assert_eq!(kinds("TRUE")[0], TokenKind::True);
    assert_eq!(kinds("FALSE")[0], TokenKind::False);
    assert_eq!(kinds("Object")[0], TokenKind::Object);
    assert_eq!(kinds("ValueBase")[0], TokenKind::Valuebase);
    for spelling in ["true", "false", "object", "valuebase"] {
        assert_eq!(kinds(spelling)[0], TokenKind::Ident, "`{spelling}`");
    }
}

#[test]
fn an_identifier_that_differs_from_a_keyword_only_in_case_is_named_with_it() {
    // The lexer answers the question; §7.2.3's rule is enforced one
    // layer up, and needs to know which keyword was collided with in
    // order to say so.
    assert_eq!(zerodds_idl_cst::keyword_collision("CUSTOM"), Some("custom"));
    assert_eq!(zerodds_idl_cst::keyword_collision("Long"), Some("long"));
    assert_eq!(zerodds_idl_cst::keyword_collision("true"), Some("TRUE"));
    assert_eq!(zerodds_idl_cst::keyword_collision("object"), Some("Object"));
    // An exact keyword is not an identifier at all, and a name that
    // resembles none of them is nobody's collision.
    assert_eq!(zerodds_idl_cst::keyword_collision("custom"), None);
    assert_eq!(zerodds_idl_cst::keyword_collision("Colour"), None);
}

#[test]
fn an_escaped_identifier_is_the_name_without_the_underscore() {
    let tokens = lex("_long");
    assert_eq!(tokens[0].kind, TokenKind::Ident);
    assert_eq!(tokens[0].value, TokenValue::Ident { escaped: true });
    // The escape is recorded, not lost: a formatter has to write it back.
    assert_eq!(tokens[0].span.slice("_long"), Some("_long"));
}

#[test]
fn an_ordinary_identifier_is_not_marked_escaped() {
    let tokens = lex("robot");
    assert_eq!(tokens[0].kind, TokenKind::Ident);
    assert_eq!(tokens[0].value, TokenValue::Ident { escaped: false });
}

#[test]
fn an_underscore_prefixed_non_keyword_is_still_an_escaped_identifier() {
    // `_foo` is `foo`, whether or not `foo` is a keyword.
    let tokens = lex("_foo");
    assert_eq!(tokens[0].value, TokenValue::Ident { escaped: true });
}

// -- numbers -------------------------------------------------------------

#[test]
fn every_radix_reaches_the_same_value_and_keeps_how_it_was_written() {
    // A formatter that rewrites 0x1F as 31 has changed the source.
    for (text, radix) in [
        ("31", Radix::Decimal),
        ("037", Radix::Octal),
        ("0x1F", Radix::Hex),
        ("0X1f", Radix::Hex),
    ] {
        let tokens = lex(text);
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral, "{text}");
        assert_eq!(
            tokens[0].value,
            TokenValue::Int { value: 31, radix },
            "{text}"
        );
    }
}

#[test]
fn float_forms_stay_distinguishable_from_one_another() {
    for text in ["1.", "1.0", "1e5", "1.5e-3", ".5"] {
        assert_eq!(kinds(text)[0], TokenKind::FloatLiteral, "{text}");
    }
    // Different source, different span text — the round trip depends on
    // this and a normalising lexer would break it.
    assert_ne!(
        lex("1.")[0].span.slice("1."),
        lex("1.0")[0].span.slice("1.0")
    );
}

#[test]
fn a_d_suffix_makes_a_fixed_point_literal() {
    for text in ["3.14d", "42D", "1.0d"] {
        assert_eq!(kinds(text)[0], TokenKind::FixedLiteral, "{text}");
    }
}

#[test]
fn a_literal_too_large_for_i128_is_an_error_not_a_wrapped_value() {
    // A silently wrapped bound is worse than a rejected file.
    let huge = "9".repeat(60);
    assert_eq!(kinds(&huge)[0], TokenKind::Error);
}

// -- character and string literals ---------------------------------------

#[test]
fn character_and_string_literals_resolve_their_escapes() {
    assert_eq!(
        lex(r"'\n'")[0].value,
        TokenValue::Char {
            value: u32::from(b'\n')
        }
    );
    assert_eq!(
        lex(r"'\101'")[0].value,
        TokenValue::Char {
            value: u32::from(b'A')
        }
    );
    assert_eq!(
        lex(r"'\x41'")[0].value,
        TokenValue::Char {
            value: u32::from(b'A')
        }
    );

    let tokens = lex(r#""a\tb""#);
    assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
    assert_eq!(
        tokens[0].value,
        TokenValue::Text {
            value: "a\tb".to_string()
        }
    );
}

#[test]
fn wide_literals_are_their_own_kinds() {
    assert_eq!(kinds("L'x'")[0], TokenKind::WCharLiteral);
    assert_eq!(kinds(r#"L"x""#)[0], TokenKind::WStringLiteral);
    // A bare `L` is still an identifier.
    assert_eq!(kinds("L")[0], TokenKind::Ident);
}

#[test]
fn an_unterminated_string_is_an_error_token_and_the_stream_stays_complete() {
    // The lexer never fails. Spec §1.2.
    let source = "\"unterminated\nstruct S {};";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, TokenKind::Error);
    // Lexing continued past it.
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Struct));
    assert_eq!(reconstruct(source), source);
}

#[test]
fn multi_byte_utf8_in_a_string_survives_intact() {
    let source = "\"Grüße für Sandra\"";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
    assert_eq!(
        tokens[0].value,
        TokenValue::Text {
            value: "Grüße für Sandra".to_string()
        }
    );
    assert_eq!(reconstruct(source), source);
}

// -- trivia --------------------------------------------------------------

#[test]
fn a_comment_before_a_token_is_that_tokens_leading_trivia() {
    let source = "// a comment\nstruct";
    let tokens = lex(source);
    let struct_token = &tokens[0];
    assert_eq!(struct_token.kind, TokenKind::Struct);
    assert!(
        struct_token
            .leading
            .iter()
            .any(|t| t.kind == TriviaKind::LineComment)
    );
}

#[test]
fn the_eof_token_carries_the_files_trailing_trivia() {
    // Otherwise the last comment in a file would be unreachable.
    let source = "struct S;\n// trailing\n";
    let tokens = lex(source);
    let eof = tokens.last().expect("an eof token");
    assert_eq!(eof.kind, TokenKind::Eof);
    assert!(
        eof.leading
            .iter()
            .any(|t| t.kind == TriviaKind::LineComment)
    );
    assert_eq!(reconstruct(source), source);
}

#[test]
fn a_block_comment_is_one_piece_of_trivia_even_across_lines() {
    let source = "/* one\n   two */ struct";
    let tokens = lex(source);
    let comments: Vec<_> = tokens[0]
        .leading
        .iter()
        .filter(|t| t.kind == TriviaKind::BlockComment)
        .collect();
    assert_eq!(comments.len(), 1);
}

#[test]
fn an_unterminated_block_comment_runs_to_end_of_file_rather_than_failing() {
    let source = "/* never closed\nstruct S;";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
    assert_eq!(reconstruct(source), source);
}

#[test]
fn a_preprocessor_line_is_trivia_including_its_continuations() {
    let source = "#define X \\\n  42\nstruct";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, TokenKind::Struct);
    assert!(
        tokens[0]
            .leading
            .iter()
            .any(|t| t.kind == TriviaKind::Directive)
    );
    assert_eq!(reconstruct(source), source);
}

#[test]
fn a_hash_that_is_not_at_the_start_of_a_line_is_punctuation() {
    let tokens = lex("a # b");
    assert_eq!(tokens[1].kind, TokenKind::Hash);
}

// -- punctuation ---------------------------------------------------------

#[test]
fn shift_right_is_one_token_because_the_lexer_does_not_decide() {
    // Spec §1.2, §2.3: whether it closes two type arguments or shifts is
    // the parser's call.
    assert_eq!(kinds(">>")[0], TokenKind::GreaterGreater);
    assert_eq!(kinds("> >")[0], TokenKind::Greater);
    assert_eq!(kinds("<<")[0], TokenKind::LessLess);
    assert_eq!(kinds("::")[0], TokenKind::ColonColon);
    assert_eq!(kinds(":")[0], TokenKind::Colon);
}

#[test]
fn an_illegal_byte_run_produces_one_error_token_not_one_per_byte() {
    let tokens = lex("struct $$$ S");
    let errors: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Error)
        .collect();
    assert_eq!(errors.len(), 1);
}

// -- the property everything rests on ------------------------------------

#[test]
fn every_byte_is_reachable_for_a_realistic_file() {
    let source = r#"
// A module.
module Robot {
  /* the pose */
  @final
  struct Pose {
    @key long id;      // the key
    double x, y;
    string<64> name;
    sequence<sequence<long>> nested;
  };
  const long MAX = 0x1F + 037;
};
"#;
    assert_eq!(reconstruct(source), source);
}

#[test]
fn every_byte_is_reachable_for_a_broken_file() {
    // Recovery depends on this: a file that fails to parse still round
    // trips, because nothing was dropped on the way in.
    let source = "struct { ??? \"unterminated\n};;;";
    assert_eq!(reconstruct(source), source);
}

#[test]
fn an_empty_file_yields_one_eof_token() {
    let tokens = lex("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
}

#[test]
fn a_file_of_only_trivia_puts_all_of_it_on_the_eof_token() {
    let source = "// nothing but a comment\n\n";
    let tokens = lex(source);
    assert_eq!(tokens.len(), 1);
    assert!(!tokens[0].leading.is_empty());
    assert_eq!(reconstruct(source), source);
}

#[test]
fn spans_never_run_past_the_input() {
    let source = "struct S { long x; };";
    let end = u32::try_from(source.len()).expect("small file");
    for token in lex(source) {
        assert!(token.span.end <= end, "{:?} ran past the input", token.kind);
        for trivia in &token.leading {
            assert!(trivia.span.end <= end);
        }
    }
}

#[test]
fn a_span_covers_what_it_claims() {
    let source = "module Robot";
    let tokens = lex(source);
    assert_eq!(tokens[1].span.slice(source), Some("Robot"));
    assert_eq!(Span::new(0, 6).slice(source), Some("module"));
}

proptest::proptest! {
    /// Whatever the bytes are, the stream reconstructs them and the
    /// lexer terminates.
    #[test]
    fn any_input_round_trips(source in "\\PC{0,200}") {
        proptest::prop_assert_eq!(reconstruct(&source), source);
    }
}

// -- literal values from token text, spec §1.2 ---------------------------

#[test]
fn a_literal_value_is_recovered_from_the_token_text_alone() {
    // The green tree carries text, not values; a consumer re-lexes the
    // token rather than reimplementing radix and escape handling.
    for (kind, text, expected) in [
        (TokenKind::IntLiteral, "42", 42_i128),
        (TokenKind::IntLiteral, "0x1F", 31),
        (TokenKind::IntLiteral, "0X1f", 31),
        (TokenKind::IntLiteral, "017", 15),
        (TokenKind::IntLiteral, "0", 0),
    ] {
        let value = zerodds_idl_cst::literal_value(kind, text);
        assert_eq!(
            value,
            TokenValue::Int {
                value: expected,
                radix: match text {
                    t if t.starts_with("0x") || t.starts_with("0X") => Radix::Hex,
                    t if t.len() > 1 && t.starts_with('0') => Radix::Octal,
                    _ => Radix::Decimal,
                },
            },
            "{text}"
        );
    }
}

#[test]
fn a_string_literal_value_has_its_escapes_resolved() {
    assert_eq!(
        zerodds_idl_cst::literal_value(TokenKind::StringLiteral, r#""a\nb""#),
        TokenValue::Text {
            value: "a\nb".to_string()
        }
    );
    assert_eq!(
        zerodds_idl_cst::literal_value(TokenKind::CharLiteral, r"'\t'"),
        TokenValue::Char { value: 9 }
    );
}

#[test]
fn text_that_is_not_one_whole_token_of_that_kind_has_no_value() {
    // A partial read must not pass for a value.
    assert_eq!(
        zerodds_idl_cst::literal_value(TokenKind::IntLiteral, "42 43"),
        TokenValue::None
    );
    assert_eq!(
        zerodds_idl_cst::literal_value(TokenKind::IntLiteral, "abc"),
        TokenValue::None
    );
    assert_eq!(
        zerodds_idl_cst::literal_value(TokenKind::StringLiteral, "\"unterminated"),
        TokenValue::None
    );
}

/// A raw non-ASCII character in a `char` literal is one character.
///
/// This kept the *last octet* of the UTF-8 encoding, so `'ä'` — C3 A4 —
/// became U+00A4 and not U+00E4, with nothing said. Found by
/// `docs/corba-dialect-followup.md` item 4, printing the model back as
/// CORBA IDL. `lex_char` carries the reading and why it is the broadest
/// of the three §7.4.1.1.2 leaves open.
#[test]
fn a_raw_non_ascii_char_literal_is_its_code_point_and_not_its_last_octet() {
    let tokens = lex("'ä'");
    assert_eq!(tokens[0].kind, TokenKind::CharLiteral);
    assert_eq!(tokens[0].value, TokenValue::Char { value: 0x00E4 });
    // The escaped form was right all along, and still is.
    let tokens = lex(r"'\xe4'");
    assert_eq!(tokens[0].kind, TokenKind::CharLiteral);
    assert_eq!(tokens[0].value, TokenValue::Char { value: 0x00E4 });
}

/// A character the type cannot hold is refused rather than narrowed.
///
/// §7.2.2.2.1.2.2 makes a `CHAR_8_TYPE` "representable using 8 bits" and
/// §7.2.2.2.1.2.5 restricts a `CHAR_16_TYPE` to the Basic Multilingual
/// Plane. Narrowing here is how the defect above produced a different
/// character in the first place.
#[test]
fn a_char_literal_the_type_cannot_hold_is_an_error_token() {
    // U+0161 needs nine bits.
    assert_eq!(lex("'š'")[0].kind, TokenKind::Error);
    // A `wchar` takes it.
    assert_eq!(lex("L'š'")[0].kind, TokenKind::WCharLiteral);
    // And a code point outside the BMP has no `wchar`.
    assert_eq!(lex("L'𝄞'")[0].kind, TokenKind::Error);
    // Two characters is not one character, whatever the widths.
    assert_eq!(lex("'ab'")[0].kind, TokenKind::Error);
    assert_eq!(lex("''")[0].kind, TokenKind::Error);
}

#[test]
fn a_leading_zero_before_a_point_is_a_decimal_number_and_not_an_octal_one() {
    // §7.4.1.4.3's own worked example of a fixed-point literal is
    // `0123.450d`, "considered to be fixed<7,3>". Read as an octal
    // literal it became `0123` followed by `.450d`: two tokens, and a
    // parse error on the second. §7.2.6.1 gives an octal literal digits
    // and nothing after them.
    assert_eq!(kinds("0123.450d")[0], TokenKind::FixedLiteral);
    assert_eq!(kinds("0123.450")[0], TokenKind::FloatLiteral);
    assert_eq!(kinds("0123e5")[0], TokenKind::FloatLiteral);
    assert_eq!(kinds("0123d")[0], TokenKind::FixedLiteral);
    // And an octal literal is still octal.
    assert_eq!(kinds("0123")[0], TokenKind::IntLiteral);
    assert_eq!(
        lex("0123")[0].value,
        TokenValue::Int {
            value: 0o123,
            radix: Radix::Octal
        }
    );
}
