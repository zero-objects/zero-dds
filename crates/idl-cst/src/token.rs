// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Tokens and their trivia. Spec §1.

use crate::span::Span;
use alloc::string::String;
use alloc::vec::Vec;

/// What a token is.
///
/// Keywords are their own kinds rather than identifiers with a flag, so
/// a parser matching on `TokenKind::Struct` cannot accidentally accept
/// an identifier spelled the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TokenKind {
    // -- structure -------------------------------------------------------
    /// End of file. Carries the file's trailing trivia.
    Eof,
    /// A run of characters the lexer could not make sense of.
    ///
    /// The lexer never fails; it produces this instead, so the byte
    /// stream stays complete and the tree still round-trips. Spec §1.2.
    Error,
    /// Spaces, tabs and line breaks, as they appear in the tree.
    ///
    /// The lexer reports trivia beside the token it precedes; the parser
    /// puts it into the tree as a token of its own so the round trip is
    /// exact. It has its own kind rather than borrowing `Error`, because
    /// a consumer that skips what the lexer could not read must not also
    /// skip the spaces between two keywords — and one that reads the
    /// first token of a node must not read a space.
    Whitespace,
    /// `// ...` in the tree.
    LineComment,
    /// `/* ... */` in the tree.
    BlockComment,
    /// A preprocessor line in the tree.
    Directive,

    // -- names and literals ----------------------------------------------
    /// An identifier.
    Ident,
    /// An integer literal, in any radix.
    IntLiteral,
    /// A floating-point literal.
    FloatLiteral,
    /// A fixed-point literal, which ends in `d` or `D`.
    FixedLiteral,
    /// A character literal.
    CharLiteral,
    /// A wide-character literal, `L'x'`.
    WCharLiteral,
    /// A string literal.
    StringLiteral,
    /// A wide-string literal, `L"x"`.
    WStringLiteral,

    // -- punctuation -----------------------------------------------------
    /// `;`
    Semicolon,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `>>`
    ///
    /// One token. Whether it closes two type arguments or shifts is the
    /// parser's decision, not the lexer's. Spec §1.2, §2.3.
    GreaterGreater,
    /// `<<`
    LessLess,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `::`
    ColonColon,
    /// `=`
    Equals,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `~`
    Tilde,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `&`
    Ampersand,
    /// `@`
    At,
    /// `//@`, the marker of the suffix annotation form.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.5, Table 24 makes `//@name(args)` written
    /// after a declaration equivalent to `@name(args)` written before
    /// it, and §7.3.1.2.3 requires the at-sign to follow the double
    /// slash with nothing in between. One token rather than a `//`
    /// beside an `At`: the two spellings must stay distinguishable, and
    /// a rule reading an annotation's name has one thing to skip.
    ///
    /// The lexer never produces it — a `//@` comment is trivia there,
    /// like every other comment. The parser produces it where the
    /// suffix form is legal, out of the comment's own bytes, which is
    /// what keeps the round trip exact.
    SlashSlashAt,
    /// `#`, only meaningful at the start of a preprocessor line.
    Hash,

    // -- keywords, OMG IDL 4.2 §7.2.4 ------------------------------------
    /// `abstract`
    Abstract,
    /// `any`
    Any,
    /// `alias`
    Alias,
    /// `annotation`
    Annotation,
    /// `attribute`
    Attribute,
    /// `bitfield`
    Bitfield,
    /// `bitmask`
    Bitmask,
    /// `bitset`
    Bitset,
    /// `boolean`
    Boolean,
    /// `case`
    Case,
    /// `char`
    Char,
    /// `component`
    Component,
    /// `connector`
    Connector,
    /// `const`
    Const,
    /// `consumes`
    Consumes,
    /// `context`
    Context,
    /// `custom`
    Custom,
    /// `default`
    Default,
    /// `double`
    Double,
    /// `exception`
    Exception,
    /// `emits`
    Emits,
    /// `enum`
    Enum,
    /// `eventtype`
    Eventtype,
    /// `factory`
    Factory,
    /// `FALSE`
    False,
    /// `finder`
    Finder,
    /// `fixed`
    Fixed,
    /// `float`
    Float,
    /// `getraises`
    Getraises,
    /// `home`
    Home,
    /// `import`
    Import,
    /// `in`
    In,
    /// `inout`
    Inout,
    /// `int8`
    Int8,
    /// `int16`
    Int16,
    /// `int32`
    Int32,
    /// `int64`
    Int64,
    /// `interface`
    Interface,
    /// `keylist` — an RTI Connext directive keyword, not OMG IDL.
    Keylist,
    /// `local`
    Local,
    /// `long`
    Long,
    /// `manages`
    Manages,
    /// `map`
    Map,
    /// `mirrorport`
    Mirrorport,
    /// `module`
    Module,
    /// `multiple`
    Multiple,
    /// `native`
    Native,
    /// `Object`
    Object,
    /// `octet`
    Octet,
    /// `oneway`
    Oneway,
    /// `out`
    Out,
    /// `port`
    Port,
    /// `porttype`
    Porttype,
    /// `primarykey`
    Primarykey,
    /// `private`
    Private,
    /// `provides`
    Provides,
    /// `public`
    Public,
    /// `publishes`
    Publishes,
    /// `raises`
    Raises,
    /// `readonly`
    Readonly,
    /// `sequence`
    Sequence,
    /// `setraises`
    Setraises,
    /// `short`
    Short,
    /// `string`
    StringKw,
    /// `struct`
    Struct,
    /// `supports`
    Supports,
    /// `switch`
    Switch,
    /// `TRUE`
    True,
    /// `truncatable`
    Truncatable,
    /// `typedef`
    Typedef,
    /// `typeid`
    Typeid,
    /// `typename`
    Typename,
    /// `typeprefix`
    Typeprefix,
    /// `uint8`
    Uint8,
    /// `uint16`
    Uint16,
    /// `uint32`
    Uint32,
    /// `uint64`
    Uint64,
    /// `union`
    Union,
    /// `unsigned`
    Unsigned,
    /// `uses`
    Uses,
    /// `ValueBase`
    Valuebase,
    /// `valuetype`
    Valuetype,
    /// `void`
    Void,
    /// `wchar`
    Wchar,
    /// `wstring`
    Wstring,
}

impl TokenKind {
    /// Whether this kind is one of the IDL keywords.
    #[must_use]
    pub const fn is_keyword(self) -> bool {
        KEYWORDS_START <= self as u16 as usize
    }

    /// Whether this kind is trivia rather than syntax.
    ///
    /// Trivia is in the tree for the round trip; a rule reading a node's
    /// tokens wants the syntax.
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::Directive
        )
    }

    /// Whether this kind is a literal of any sort.
    #[must_use]
    pub const fn is_literal(self) -> bool {
        matches!(
            self,
            Self::IntLiteral
                | Self::FloatLiteral
                | Self::FixedLiteral
                | Self::CharLiteral
                | Self::WCharLiteral
                | Self::StringLiteral
                | Self::WStringLiteral
                | Self::True
                | Self::False
        )
    }
}

// `Abstract` is the first keyword variant; anything at or past it is a
// keyword. Kept as a constant so adding a keyword before it is a
// compile-visible change rather than a silent reclassification.
const KEYWORDS_START: usize = TokenKind::Abstract as u16 as usize;

/// The IDL 4.2 keywords and their kinds, in Table 7-6's order.
///
/// A slice rather than a fixed-size array: a keyword added to the
/// language should be one line, not one line and a length that has to
/// be counted by hand.
///
/// **The spelling is the table's, exactly.** OMG IDL 4.2 §7.2.4, p. 20:
/// "Keywords must be written exactly as shown in the above list.
/// Identifiers that collide with keywords (see 7.2.3, Identifiers) are
/// illegal." Four entries of Table 7-6 are not all lower case — `TRUE`,
/// `FALSE`, `Object` and `ValueBase` — and rules (117) and (131) write
/// the last two that way in the grammar itself.
///
/// The two sentences are two rules and they are enforced in two places.
/// Recognition is exact, in [`crate::lex()`]; the collision belongs to
/// §7.2.3 and is reported by name in the lowering, which is what
/// [`keyword_collision`] answers. Matching keywords case-insensitively
/// collapsed both into one and turned the identifier `CUSTOM` into the
/// keyword `custom`.
///
/// Table 7-6 is alphabetical without regard to case, and the order here
/// is the table's, so `Object` sits between `native` and `octet`.
pub const KEYWORDS: &[(&str, TokenKind)] = &[
    ("abstract", TokenKind::Abstract),
    ("alias", TokenKind::Alias),
    ("annotation", TokenKind::Annotation),
    ("any", TokenKind::Any),
    ("attribute", TokenKind::Attribute),
    ("bitfield", TokenKind::Bitfield),
    ("bitmask", TokenKind::Bitmask),
    ("bitset", TokenKind::Bitset),
    ("boolean", TokenKind::Boolean),
    ("case", TokenKind::Case),
    ("char", TokenKind::Char),
    ("component", TokenKind::Component),
    ("connector", TokenKind::Connector),
    ("const", TokenKind::Const),
    ("consumes", TokenKind::Consumes),
    ("context", TokenKind::Context),
    ("custom", TokenKind::Custom),
    ("default", TokenKind::Default),
    ("double", TokenKind::Double),
    ("emits", TokenKind::Emits),
    ("enum", TokenKind::Enum),
    ("eventtype", TokenKind::Eventtype),
    ("exception", TokenKind::Exception),
    ("factory", TokenKind::Factory),
    ("FALSE", TokenKind::False),
    ("finder", TokenKind::Finder),
    ("fixed", TokenKind::Fixed),
    ("float", TokenKind::Float),
    ("getraises", TokenKind::Getraises),
    ("home", TokenKind::Home),
    ("import", TokenKind::Import),
    ("in", TokenKind::In),
    ("inout", TokenKind::Inout),
    ("int16", TokenKind::Int16),
    ("int32", TokenKind::Int32),
    ("int64", TokenKind::Int64),
    ("int8", TokenKind::Int8),
    ("interface", TokenKind::Interface),
    ("keylist", TokenKind::Keylist),
    ("local", TokenKind::Local),
    ("long", TokenKind::Long),
    ("manages", TokenKind::Manages),
    ("map", TokenKind::Map),
    ("mirrorport", TokenKind::Mirrorport),
    ("module", TokenKind::Module),
    ("multiple", TokenKind::Multiple),
    ("native", TokenKind::Native),
    ("Object", TokenKind::Object),
    ("octet", TokenKind::Octet),
    ("oneway", TokenKind::Oneway),
    ("out", TokenKind::Out),
    ("port", TokenKind::Port),
    ("porttype", TokenKind::Porttype),
    ("primarykey", TokenKind::Primarykey),
    ("private", TokenKind::Private),
    ("provides", TokenKind::Provides),
    ("public", TokenKind::Public),
    ("publishes", TokenKind::Publishes),
    ("raises", TokenKind::Raises),
    ("readonly", TokenKind::Readonly),
    ("sequence", TokenKind::Sequence),
    ("setraises", TokenKind::Setraises),
    ("short", TokenKind::Short),
    ("string", TokenKind::StringKw),
    ("struct", TokenKind::Struct),
    ("supports", TokenKind::Supports),
    ("switch", TokenKind::Switch),
    ("TRUE", TokenKind::True),
    ("truncatable", TokenKind::Truncatable),
    ("typedef", TokenKind::Typedef),
    ("typeid", TokenKind::Typeid),
    ("typename", TokenKind::Typename),
    ("typeprefix", TokenKind::Typeprefix),
    ("uint16", TokenKind::Uint16),
    ("uint32", TokenKind::Uint32),
    ("uint64", TokenKind::Uint64),
    ("uint8", TokenKind::Uint8),
    ("union", TokenKind::Union),
    ("unsigned", TokenKind::Unsigned),
    ("uses", TokenKind::Uses),
    ("ValueBase", TokenKind::Valuebase),
    ("valuetype", TokenKind::Valuetype),
    ("void", TokenKind::Void),
    ("wchar", TokenKind::Wchar),
    ("wstring", TokenKind::Wstring),
];

/// The keyword an identifier collides with, differing from it only in
/// case.
///
/// `None` when the word *is* a keyword — then it is not an identifier at
/// all — and when it resembles none of them.
///
/// OMG IDL 4.2 §7.2.3: "when comparing two identifiers to see if they
/// collide … upper- and lower-case letters are treated as the same
/// letter", and §7.2.4: "Identifiers that collide with keywords … are
/// illegal". The clause's own examples are `typedef Long Foo;` and
/// `typedef boolean BOOLEAN;`.
///
/// The escaped form is the caller's business, because §7.2.3.2 makes the
/// underscore "a purely lexical convention that ONLY turns off keyword
/// checking": a caller holding `_custom` must not ask at all.
#[must_use]
pub fn keyword_collision(word: &str) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|(keyword, _)| *keyword != word && keyword.eq_ignore_ascii_case(word))
        .map(|(keyword, _)| *keyword)
}

/// The identifier an `Ident` token spells.
///
/// OMG IDL 4.2 §7.2.3.2: prepending an underscore is "a purely lexical
/// convention that ONLY turns off keyword checking", and the escaped
/// form "is treated as if it were" the identifier without it — `_long`
/// names `long`. The underscore is therefore not part of the name
/// anywhere the name is used: member-id hashing, scope lookup,
/// collision checks, TypeObject member names. Only one underscore is
/// lexical; §7.2.3 admits no identifier that starts with one, so a
/// second underscore never occurs in legal input and is left alone.
#[must_use]
pub fn identifier_name(word: &str) -> &str {
    word.strip_prefix('_').unwrap_or(word)
}

#[cfg(test)]
mod identifier_name_tests {
    use super::identifier_name;

    #[test]
    fn the_escape_underscore_is_not_part_of_the_name() {
        assert_eq!(identifier_name("_long"), "long");
        assert_eq!(identifier_name("plain"), "plain");
        assert_eq!(identifier_name("_switch"), "switch");
    }
}

/// The radix an integer literal was written in.
///
/// Retained because a formatter that rewrites `0x1F` as `31` has changed
/// the source, not formatted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Radix {
    /// `31`
    Decimal,
    /// `037`
    Octal,
    /// `0x1F`
    Hex,
}

/// What a token additionally knows about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenValue {
    /// Nothing beyond its kind and text.
    None,
    /// An identifier, with whether it was written in the escaped `_name`
    /// form. The escape is not part of the name.
    Ident {
        /// Whether the source wrote a leading underscore escape.
        escaped: bool,
    },
    /// An integer literal and the radix it was written in.
    Int {
        /// The value.
        value: i128,
        /// How it was written.
        radix: Radix,
    },
    /// A character literal's code point.
    Char {
        /// The resolved code point.
        value: u32,
    },
    /// A string literal with its escapes resolved. The raw text stays
    /// available through the token's span.
    Text {
        /// The resolved content.
        value: String,
    },
}

/// One lexed token, with the trivia that precedes it.
///
/// Trivia is attached leading, and the file's trailing trivia goes on
/// the end-of-file token. That rule is what makes "every byte is
/// reachable from the token stream" true by construction. Spec §1.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// The token's own text, excluding trivia.
    pub span: Span,
    /// Comments and spacing before it, in source order.
    pub leading: Vec<Trivia>,
    /// What it knows beyond its kind.
    pub value: TokenValue,
}

impl Token {
    /// The range covering the trivia and the token together.
    #[must_use]
    pub fn full_span(&self) -> Span {
        match self.leading.first() {
            Some(first) => Span::new(first.span.start, self.span.end),
            None => self.span,
        }
    }
}

/// What kind of trivia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriviaKind {
    /// Spaces and tabs.
    Whitespace,
    /// A line break.
    Newline,
    /// `// ...`
    LineComment,
    /// `/* ... */`
    BlockComment,
    /// A preprocessor line, kept as trivia by the lexer and interpreted
    /// by the preprocessor.
    Directive,
}

impl TriviaKind {
    /// Whether this trivia carries documentation a consumer may want.
    #[must_use]
    pub const fn is_comment(self) -> bool {
        matches!(self, Self::LineComment | Self::BlockComment)
    }
}

/// One run of trivia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trivia {
    /// What it is.
    pub kind: TriviaKind,
    /// Where it is.
    pub span: Span,
}
