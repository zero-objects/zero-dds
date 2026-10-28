// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the four trees share. Spec §1.
//!
//! Only what is genuinely common: an identifier, a path, a doc comment,
//! a literal. Everything else differs between the languages and lives in
//! their own modules — a shared `Type` would be a shared target model
//! under another name.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Which language a tree belongs to.
///
/// Carried so a coverage table can be indexed by it and a diagnostic can
/// name it; the trees themselves never branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    /// Rust.
    Rust,
    /// C++.
    Cpp,
    /// C.
    C,
    /// Java.
    Java,
    /// C#.
    CSharp,
    /// Python.
    Python,
    /// TypeScript.
    TypeScript,
    /// Go.
    Go,
    /// OCaml.
    OCaml,
    /// Ada.
    Ada,
    /// D.
    D,
    /// Elixir.
    Elixir,
    /// Julia.
    Julia,
    /// Lua.
    Lua,
    /// Nim.
    Nim,
    /// Swift.
    Swift,
    /// Zig.
    Zig,
    /// Node (plain JavaScript).
    Node,
    /// Ada 83 (ANSI/MIL-STD-1815A), a second column for a language
    /// already on the list — it answers enough clauses differently to
    /// need its own materialisation.
    Ada83,
    /// F#.
    FSharp,
    /// Kotlin.
    Kotlin,
}

impl Language {
    /// Every language these trees cover.
    ///
    /// The languages whose backends are **complete**, so a coverage
    /// table over this array is total by construction.
    ///
    /// A variant exists in `Language` before it is in here: the enum is
    /// what a module registers itself under while it is being built, and
    /// this array is what says it is done. A language added to the enum
    /// and not to this list appears in no table and claims no coverage —
    /// which is the honest state for one whose catalogue is still empty.
    pub const ALL: [Self; 21] = [
        Self::Rust,
        Self::Cpp,
        Self::C,
        Self::Java,
        Self::CSharp,
        Self::Python,
        Self::TypeScript,
        Self::Go,
        Self::OCaml,
        Self::Ada,
        Self::D,
        Self::Elixir,
        Self::Julia,
        Self::Lua,
        Self::Nim,
        Self::Swift,
        Self::Zig,
        Self::Node,
        Self::Ada83,
        Self::FSharp,
        Self::Kotlin,
    ];

    /// The name a diagnostic or a table column uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Cpp => "c++",
            Self::C => "c",
            Self::Java => "java",
            Self::CSharp => "c#",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::OCaml => "ocaml",
            Self::Ada => "ada",
            Self::D => "d",
            Self::Elixir => "elixir",
            Self::Julia => "julia",
            Self::Lua => "lua",
            Self::Nim => "nim",
            Self::Swift => "swift",
            Self::Zig => "zig",
            Self::Node => "node",
            Self::Ada83 => "ada83",
            Self::FSharp => "f#",
            Self::Kotlin => "kotlin",
        }
    }

    /// The file extension generated output takes.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Cpp => "hpp",
            Self::C => "h",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::OCaml => "ml",
            Self::Ada => "ads",
            Self::D => "d",
            Self::Elixir => "ex",
            Self::Julia => "jl",
            Self::Lua => "lua",
            Self::Nim => "nim",
            Self::Swift => "swift",
            Self::Zig => "zig",
            Self::Node => "js",
            Self::Ada83 => "ads",
            Self::FSharp => "fs",
            Self::Kotlin => "kt",
        }
    }
}

/// An identifier in a target language.
///
/// Holds the text as it will be written. Escaping a name that collides
/// with a target keyword is a rule's job, not the identifier's: a type
/// that silently renamed what it was given would hide the collision that
/// made it necessary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ident(String);

impl Ident {
    /// An identifier spelled exactly `text`.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_string())
    }

    /// How it is spelled.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether it is empty, which no rule should produce.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A qualified name, outermost segment first.
///
/// One type for `std::vector`, `java.util.List` and `::Robot::Pose`: the
/// separator is the renderer's business, the segments are the tree's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Path {
    segments: Vec<Ident>,
    rooted: bool,
}

impl Path {
    /// A path over `segments`, relative to whatever scope it appears in.
    #[must_use]
    pub fn new(segments: &[&str]) -> Self {
        Self {
            segments: segments.iter().map(|s| Ident::new(s)).collect(),
            rooted: false,
        }
    }

    /// A path from the root: `::A::B` in C++, `crate::a::B` in Rust.
    #[must_use]
    pub fn rooted(segments: &[&str]) -> Self {
        Self {
            segments: segments.iter().map(|s| Ident::new(s)).collect(),
            rooted: true,
        }
    }

    /// A path of one segment.
    #[must_use]
    pub fn single(name: &str) -> Self {
        Self::new(&[name])
    }

    /// Its segments, outermost first.
    #[must_use]
    pub fn segments(&self) -> &[Ident] {
        &self.segments
    }

    /// Whether it starts at the root.
    #[must_use]
    pub const fn is_rooted(&self) -> bool {
        self.rooted
    }

    /// The last segment, which is the name itself.
    #[must_use]
    pub fn last(&self) -> Option<&Ident> {
        self.segments.last()
    }

    /// The same path with one more segment on the end.
    #[must_use]
    pub fn join(&self, name: &str) -> Self {
        let mut out = self.clone();
        out.segments.push(Ident::new(name));
        out
    }
}

/// A documentation comment, one line per entry.
///
/// Lines rather than one string: the renderer prefixes each with the
/// marker its language uses, and a string with embedded newlines would
/// make that the renderer's parsing problem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    lines: Vec<String>,
}

impl Doc {
    /// An empty documentation comment, which renders as nothing.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// A documentation comment of one line.
    #[must_use]
    pub fn line(text: &str) -> Self {
        Self {
            lines: alloc::vec![text.to_string()],
        }
    }

    /// A documentation comment over several lines.
    #[must_use]
    pub fn lines(lines: &[&str]) -> Self {
        Self {
            lines: lines.iter().map(|line| (*line).to_string()).collect(),
        }
    }

    /// Add a line.
    pub fn push(&mut self, text: &str) {
        self.lines.push(text.to_string());
    }

    /// Its lines.
    #[must_use]
    pub fn as_lines(&self) -> &[String] {
        &self.lines
    }

    /// Whether there is nothing to render.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// `@verbatim` text inside a declaration's body.
///
/// OMG IDL 4.2 §8.3.5.1's `BEGIN_DECLARATION` and `END_DECLARATION`
/// placements, for a declaration whose body is a list of fields or
/// properties and has no member variant to hold a raw line. Copied
/// unchanged, one output line per text line, at the body's indentation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyText {
    /// Text written first inside the body.
    pub begin: Vec<String>,
    /// Text written last inside the body.
    pub end: Vec<String>,
}

impl BodyText {
    /// No text, which is the common case.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// A literal value, in the form it will be written.
///
/// The written form rather than a number: `0x1F` and `31` are the same
/// value and different source, and a generator reproducing a constant
/// has to write the one that was there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// An integer, as written.
    Int(String),
    /// A floating-point number, as written.
    Float(String),
    /// A boolean.
    Bool(bool),
    /// A character.
    Char(char),
    /// A string, unescaped — the renderer escapes for its language.
    Str(String),
    /// A byte string, for the languages that have one.
    Bytes(Vec<u8>),
    /// The language's null or none.
    Null,
}

impl Literal {
    /// An integer literal from a value, in decimal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Int(alloc::format!("{value}"))
    }

    /// A string literal.
    #[must_use]
    pub fn string(text: &str) -> Self {
        Self::Str(text.to_string())
    }
}

/// How visible a declaration is.
///
/// The three states the four languages between them need. A language
/// that has no word for one renders it as nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Visibility {
    /// Visible outside its unit: `pub`, `public`.
    Public,
    /// Visible within its unit: `pub(crate)`, package-private, `protected`.
    Internal,
    /// Not visible outside its declaration: `private`, no modifier in Rust.
    #[default]
    Private,
}

impl Visibility {
    /// Every visibility the trees distinguish.
    pub const ALL: [Self; 3] = [Self::Public, Self::Internal, Self::Private];
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_path_keeps_its_segments_in_order_and_knows_whether_it_is_rooted() {
        let path = Path::rooted(&["Robot", "Pose"]);
        assert!(path.is_rooted());
        assert_eq!(path.segments().len(), 2);
        assert_eq!(path.last().map(Ident::as_str), Some("Pose"));

        let joined = path.join("Inner");
        assert_eq!(joined.segments().len(), 3);
        assert!(joined.is_rooted(), "joining lost the root");
    }

    #[test]
    fn an_identifier_is_written_exactly_as_it_was_given() {
        // Escaping a keyword collision is a rule's decision; a type that
        // renamed silently would hide what made it necessary.
        for name in ["Pose", "type", "class", "_reserved"] {
            assert_eq!(Ident::new(name).as_str(), name);
        }
    }

    #[test]
    fn every_language_has_its_own_name_and_extension() {
        let mut seen = alloc::vec::Vec::new();
        for language in Language::ALL {
            assert!(!seen.contains(&language.as_str()));
            seen.push(language.as_str());
            assert!(!language.extension().is_empty());
        }
        assert_eq!(seen.len(), Language::ALL.len());
    }

    #[test]
    fn a_doc_comment_keeps_its_lines_apart() {
        let doc = Doc::lines(&["first", "second"]);
        assert_eq!(doc.as_lines().len(), 2);
        assert!(!doc.is_empty());
        assert!(Doc::none().is_empty());
    }
}
