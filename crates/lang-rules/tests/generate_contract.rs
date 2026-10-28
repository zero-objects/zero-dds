//! Does the contract actually produce source?
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
use zerodds_lang_cst::Language;

/// One IDL, lowered and resolved once, reused by every language.
fn reading_ast() -> zerodds_ast::Ast {
    let source = "
        module m {
          @appendable struct Reading {
            long id;
            double value;
            string label;
          };
        };
    ";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "probe.idl",
        zerodds_idl_lower::Options::default(),
    );
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

#[test]
fn one_idl_becomes_lua() {
    let ast = reading_ast();
    let text =
        zerodds_lang_rules::generate::declarations(&ast, Language::Lua).expect("lua declarations");
    assert!(!text.is_empty(), "no source came out");
    assert!(
        text.contains("Reading"),
        "the type is not in the output:\n{text}"
    );
}

/// An enumeration, lowered and resolved once, reused by every language.
///
/// Not [`reading_ast`]'s struct: `adapter::node` declares nothing for a
/// struct, a union, a bitset, a valuetype, an exception, an interface or
/// a typedef — a plain object literal needs no named declaration — so a
/// struct-only probe would legitimately come back empty for
/// `Language::Node` and the loop below could not tell that apart from a
/// silently dropped declaration. An enumeration is the one construct
/// almost every wired language binds a name to — `Language::Ada83` is
/// the one exception the loop below still has to name, and why is on
/// that arm.
fn mode_ast() -> zerodds_ast::Ast {
    let source = "
        module m {
          enum Mode { IDLE, ACTIVE };
        };
    ";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "probe.idl",
        zerodds_idl_lower::Options::default(),
    );
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// The same enumeration, through every language `generate::declarations`
/// routes — `Language::ALL`, which is every variant the enum has. A
/// language that produced no text, or text without the type it was
/// asked for, would be a silently dropped declaration and not a passing
/// backend.
#[test]
fn one_idl_becomes_every_wired_language() {
    let ast = mode_ast();
    for language in Language::ALL {
        let text = zerodds_lang_rules::generate::declarations(&ast, language)
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
        assert!(!text.is_empty(), "{language:?}: no source came out");
        // `Language::Ada83` binds no name to the enumeration type
        // itself — `rule::ada83::ENUM_TO_CONSTANTS`'s module
        // documentation gives the reason (no `'Enum_Val`/`'Enum_Rep`
        // confirmed under `-gnat83`), and
        // `tests/ada83_end_to_end.rs`'s
        // `an_enumerator_is_a_named_constant_and_never_an_enumeration_literal`
        // asserts the absence of `type Mode is (` outright. What this
        // column still has to bind a name to is the enumerator itself.
        let bound_name = if language == Language::Ada83 {
            "IDLE"
        } else {
            "Mode"
        };
        assert!(
            text.contains(bound_name),
            "{language:?}: the type is not in the output:\n{text}"
        );
    }
}

/// The declarations *and* the codec, through every language
/// `generate::source` routes — the same `Language::ALL` as above, over
/// [`reading_ast`] rather than [`mode_ast`]: a codec is generated per
/// struct, union and valuetype, and an enumeration is not one of those.
///
/// Every backend but Rust's and Go's shares one naming function for its
/// encoder — `zerodds_lang_rules::emit::encode::codec_name` — so
/// `zd_enc_` is literally the encoder's name in the rendered text of
/// nineteen of them; Rust's own `encode.rs` names its encoder `encode`
/// instead, as a method on the type. Checking for either is a loop over
/// the twenty-one, not twenty-one assertions that happen to differ by
/// one word.
///
/// Go's is `Zd_enc_`. Go has no visibility keyword — an identifier is
/// exported if and only if its first character is upper case (Go
/// specification, "Exported identifiers") — so a codec a program
/// outside the generated package can call has to be spelled that way,
/// and `emit::go::exported_name` raises the first letter and only that.
/// Written out here rather than asked of that function: a test that
/// asked the same function the code asks would pass whatever it
/// answered.
#[test]
fn one_idl_becomes_a_full_codec_in_every_wired_language() {
    let ast = reading_ast();
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    for language in Language::ALL {
        let text = zerodds_lang_rules::generate::source(&ast, language, key)
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
        assert!(
            text.contains("Reading"),
            "{language:?}: the type is not in the output:\n{text}"
        );
        let has_encoder = match language {
            Language::Rust => text.contains("fn encode"),
            Language::Go => text.contains("Zd_enc_"),
            _ => text.contains("zd_enc_"),
        };
        assert!(
            has_encoder,
            "{language:?}: no encoder in the output:\n{text}"
        );
    }
}
