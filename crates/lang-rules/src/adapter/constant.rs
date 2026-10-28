// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Turning a `ConstValue` into text a target language reads back
//! unchanged.
//!
//! [`Ast::eval_const`](zerodds_ast::Ast::eval_const) answers what a
//! constant *is*; the four materialisers decide what their language
//! *writes*. What is shared is only the part where all four have to
//! agree, because they are all reproducing one source token:
//!
//! - the decimal a `fixed` was written as, which is digits and a scale
//!   and never a binary float;
//! - the spelling of a floating-point literal that C, C++, Java and Rust
//!   all parse as the same number;
//! - the UTF-8 of a `wstring`, for the two bindings that hold one as
//!   UTF-8.
//!
//! Nothing here chooses a type. `const string MODULE_ID` is a
//! `const char *` in C, a `constexpr const char *` in C++, a
//! `static final String` in Java and a `&'static str` in Rust, and each
//! of those is a decision about the language rather than about the
//! value. Putting them in one function would make the four columns agree
//! by construction, which is the one thing this project's generators
//! must not do.
//!
//! ## Why the float stays text
//!
//! `ConstValue::Float` carries the mantissa and the exponent as the
//! source wrote them. The decimal `0.1` and the nearest `f64` are two
//! different numbers, and a generator that parsed the first into the
//! second and printed it back would put a token in the output that the
//! source did not contain. Every target language parses decimal
//! floating-point text itself, with its own rounding, so handing the
//! text through is both simpler and exact.

/// The decimal a `fixed<P,S>` was written as.
///
/// `ConstValue::Fixed` carries the digits without the point and the
/// count of them that sit after it, so the point is put back here.
/// `digits = "5"`, `scale = 2` is `0.05` and not `5.00`: the scale says
/// where the point goes, and left-padding is what puts it there.
///
/// A leading `-` is kept and does not count as a digit.
#[must_use]
pub fn fixed_decimal(digits: &str, scale: u8) -> String {
    let (sign, magnitude) = match digits.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", digits),
    };
    let scale = usize::from(scale);
    if scale == 0 {
        let whole = magnitude.trim_start_matches('0');
        return format!("{sign}{}", if whole.is_empty() { "0" } else { whole });
    }
    // One more digit than the scale, so there is always something in
    // front of the point: `.05` is not a decimal every language reads.
    let padded = format!("{magnitude:0>width$}", width = scale + 1);
    let split = padded.len() - scale;
    format!("{sign}{}.{}", &padded[..split], &padded[split..])
}

/// A floating-point literal that C, C++, Java and Rust all read as the
/// number the source wrote.
///
/// The mantissa and the exponent are the source's own, so the value is
/// never rounded here. Two adjustments, and each is because one of the
/// four languages is stricter than the others:
///
/// - a mantissa that starts with the point gets a leading zero, because
///   Rust does not accept `.5`;
/// - a mantissa with neither a point nor an exponent gets `.0`, because
///   `const X: f64 = 1;` is a type error in Rust while `double x = 1;`
///   is fine in the other three.
///
/// Neither changes the value. The suffix a 32-bit float needs is not
/// added here: `f` is right for C, C++ and Java and wrong for Rust,
/// where the constant's declared type already says it.
#[must_use]
pub fn float_text(mantissa: &str, exponent: Option<&str>) -> String {
    let mut out = String::with_capacity(mantissa.len() + 4);
    if mantissa.starts_with('.') {
        out.push('0');
    } else if let Some(rest) = mantissa.strip_prefix('-').filter(|r| r.starts_with('.')) {
        out.push_str("-0");
        out.push_str(rest);
        return with_exponent(out, exponent);
    }
    out.push_str(mantissa);
    if out.ends_with('.') {
        out.push('0');
    }
    if exponent.is_none() && !out.contains('.') {
        out.push_str(".0");
    }
    with_exponent(out, exponent)
}

fn with_exponent(mut text: String, exponent: Option<&str>) -> String {
    if let Some(exponent) = exponent {
        text.push('e');
        text.push_str(exponent);
    }
    text
}

/// A `wstring` constant's code points as UTF-8.
///
/// For the two bindings that hold a `wstring` as UTF-8 — C's
/// `zd_wstring` and C++'s `std::string` — which is the same choice the
/// member types make, and for the same reason: it keeps the §7.4.1.1.2
/// conversion inside the runtime.
///
/// `None` for a code point that is not a Unicode scalar value. A lone
/// surrogate is not a character, and writing one into a source file
/// would produce a literal no compiler reads as the string the IDL
/// declared.
#[must_use]
pub fn wstring_utf8(points: &[u32]) -> Option<String> {
    points.iter().map(|point| char::from_u32(*point)).collect()
}

/// A `wchar` constant's code point as a character.
///
/// `None` for a value that is not a Unicode scalar value, for the reason
/// [`wstring_utf8`] gives.
#[must_use]
pub fn wchar_char(point: u32) -> Option<char> {
    char::from_u32(point)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_fixed_puts_its_point_back_where_the_scale_says() {
        assert_eq!(fixed_decimal("12345", 2), "123.45");
        assert_eq!(fixed_decimal("12345", 0), "12345");
        // The scale reaches past the digits, so the padding is what puts
        // something in front of the point.
        assert_eq!(fixed_decimal("5", 2), "0.05");
        assert_eq!(fixed_decimal("5", 1), "0.5");
        assert_eq!(fixed_decimal("-125", 2), "-1.25");
        assert_eq!(fixed_decimal("0", 0), "0");
    }

    #[test]
    fn a_float_keeps_the_source_s_digits_and_gains_only_what_rust_needs() {
        assert_eq!(float_text("1.5", None), "1.5");
        assert_eq!(float_text("1.5", Some("3")), "1.5e3");
        assert_eq!(float_text("1.5", Some("-3")), "1.5e-3");
        // Rust reads neither of these two as written.
        assert_eq!(float_text(".5", None), "0.5");
        assert_eq!(float_text("-.5", None), "-0.5");
        assert_eq!(float_text("1", None), "1.0");
        assert_eq!(float_text("1.", None), "1.0");
        // With an exponent there is nothing to add: `1e3` is a float
        // literal in all four.
        assert_eq!(float_text("1", Some("3")), "1e3");
    }

    #[test]
    fn a_wstring_that_is_not_scalar_values_is_refused_rather_than_mangled() {
        assert_eq!(wstring_utf8(&[0x48, 0xE4]).as_deref(), Some("Hä"));
        // Outside the Basic Multilingual Plane, which is the case the
        // wstring length rule turns on.
        assert_eq!(wstring_utf8(&[0x1_D11E]).as_deref(), Some("\u{1d11e}"));
        // A lone surrogate is not a character.
        assert!(wstring_utf8(&[0xD800]).is_none());
        assert!(wchar_char(0xDC00).is_none());
        assert_eq!(wchar_char(0xE4), Some('ä'));
    }
}

#[cfg(test)]
mod four_columns {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use crate::ModelGraph;
    use crate::adapter::{c, cpp, cst, java};
    use crate::apply::apply;
    use crate::rule;
    use zerodds_ast::Ast;

    /// The IDL that made this necessary, and the one it is measured on.
    ///
    /// `crates/spatial-dds/idl/spatial_core.idl` declares six
    /// `MODULE_ID` string constants in six different modules. Until
    /// `eval_const` existed the four materialisers asked
    /// `eval_integral` about them, got `Unevaluated`, and emitted none
    /// of the six — a header, a class and a module that silently lacked
    /// the constant the source declares.
    ///
    /// A short IDL here rather than that file, so the test says what it
    /// is testing; the whole file is the acceptance test the build
    /// script runs.
    const SOURCE: &str = r#"
        module spatial {
          module sensing {
            const string MODULE_ID = "spatial.sensing.lidar/1.5";
            // `L"…"` and not `"…"`: IDL 4.2 §7.2.6.3 makes assigning a
            // non-wide string literal to a `wstring` constant a
            // compile-time diagnostic, and the front end reports it.
            const wstring WIDE = L"wide";
            const double RATE = 12.5;
            const float NARROW = .5;
            const fixed PRICE = 1.25d;
            const char TAG = 'x';
            const boolean ON = TRUE;
            const long COUNT = 7;
          };
        };
    "#;

    fn model() -> Ast {
        let parsed = zerodds_idl_parse::parse(SOURCE);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "constants.idl");
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let mut ast = lowered.ast;
        ast.resolve_all_by_name().expect("the model resolves");
        ast
    }

    /// What each column renders for the whole file.
    fn rendered() -> [(&'static str, String); 4] {
        let ast = model();
        let graph = ModelGraph::of(&ast);

        let applied = apply(&graph, &rule::c::catalogue()).expect("the C rules apply");
        let c_text = zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            guard: None,
            includes: Vec::new(),
            decls: c::c_items(&ast, &applied).expect("the C declarations"),
        });

        let applied = apply(&graph, &rule::cpp::catalogue()).expect("the C++ rules apply");
        let cpp_text = zerodds_lang_render::cpp::file(&zerodds_lang_cst::cpp::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            pragma_once: false,
            includes: Vec::new(),
            decls: cpp::cpp_items(&ast, &applied).expect("the C++ declarations"),
        });

        let applied = apply(&graph, &rule::java::catalogue()).expect("the Java rules apply");
        let java_text = zerodds_lang_render::java::file(&zerodds_lang_cst::java::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            package: None,
            imports: Vec::new(),
            types: java::java_items(&ast, &applied).expect("the Java declarations"),
        });

        let applied = apply(&graph, &rule::rust::catalogue()).expect("the Rust rules apply");
        let rust_text = cst::rust_items(&ast, &applied)
            .expect("the Rust declarations")
            .iter()
            .map(zerodds_lang_render::rust::item)
            .collect::<Vec<_>>()
            .join("\n");

        [
            ("C", c_text),
            ("C++", cpp_text),
            ("Java", java_text),
            ("Rust", rust_text),
        ]
    }

    #[test]
    fn a_string_constant_reaches_every_column() {
        for (language, text) in rendered() {
            assert!(
                text.contains(r#""spatial.sensing.lidar/1.5""#),
                "the {language} column dropped MODULE_ID:\n{text}"
            );
        }
    }

    #[test]
    fn every_non_integral_form_reaches_every_column() {
        for (language, text) in rendered() {
            // The `wstring`, as UTF-8 in the two columns that hold one
            // that way and as a `String` in the other two — the text is
            // the same either way.
            assert!(
                text.contains(r#""wide""#),
                "{language}: no wstring constant"
            );
            // The decimal the source wrote, not the nearest binary
            // float printed back.
            assert!(text.contains("12.5"), "{language}: no double constant");
            // Rust does not read `.5`, so every column gets the leading
            // zero; it is the same number.
            assert!(text.contains("0.5"), "{language}: no float constant");
            // A `fixed` is its decimal and never a binary float.
            assert!(text.contains(r#""1.25""#), "{language}: no fixed constant");
            assert!(text.contains("'x'"), "{language}: no char constant");
            assert!(text.contains('7'), "{language}: no integer constant");
        }
    }

    /// The suffix is the language's, and only where the language needs
    /// one.
    ///
    /// A `float` initialised from a decimal literal is a `double`
    /// narrowing in C, C++ and Java — an error in Java and a
    /// `-Wconversion` report in the other two — and in Rust the
    /// constant's declared type already says `f32`, where a suffix would
    /// be the thing that disagreed with it.
    #[test]
    fn a_thirty_two_bit_float_takes_the_suffix_its_language_needs() {
        for (language, text) in rendered() {
            let suffixed = text.contains("0.5f");
            assert_eq!(
                suffixed,
                language != "Rust",
                "{language}: the float suffix is wrong:\n{text}"
            );
        }
    }
}
