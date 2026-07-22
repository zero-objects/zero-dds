// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C-style preprocessor for OMG IDL 4.2.
//!
//! IDL inherits from the C preprocessor — and vendor IDL files (RTI, OpenSplice)
//! use `#include`, `#define`, `#ifdef`, `#pragma` regularly.
//! So that the parser can consume such files, the
//! preprocessor sits BEFORE the lexer and expands the directives into pure
//! IDL source.
//!
//! # Scope
//!
//! - **`#include "rel/path.idl"`** and **`#include <abs/path.idl>`**:
//!   text-based inclusion via the [`Resolver`] trait
//! - **`#define MACRO value`** (object-like) and
//!   **`#define NAME(p1, p2) body`** (function-like) incl.
//!   `#` stringize and `##` token-paste (Spec §7.2.5 + ISO 14882
//!   §16.3.2/§16.3.3)
//! - **`#ifdef`** / **`#ifndef`** / **`#if`** / **`#elif`** /
//!   **`#else`** / **`#endif`**: conditional compilation with
//!   expression eval (`defined`, `&&`, `||`, `!`, numeric literals)
//! - **`#pragma <args>`**: stripped (not in the output) — vendor pragmas
//!   like RTI's `#pragma keylist` are captured as special AST nodes
//! - **`#undef`**: remove a macro
//!
//! Not supported:
//! - Recursive macro expansion (one pass)
//! - Variadic macros (`__VA_ARGS__`)
//! - `#error`, `#warning`, `#line` (parsed, but not functional)
//! - Full C-PP arithmetic in `#if` (comparisons, bitops, ternary)
//!
//! # Source map
//!
//! [`SourceMap`] maps every position in the expanded output to
//! `(file_id, byte_offset_in_original)`. So that diagnostics after
//! parsing point to the correct original file and line.
//!
//! # Example
//!
//! ```
//! use zerodds_idl::preprocessor::{Preprocessor, MemoryResolver};
//!
//! let mut resolver = MemoryResolver::new();
//! resolver.add("Foo.idl", "struct Foo { long x; };");
//!
//! let pp = Preprocessor::new(resolver);
//! let result = pp.process("main.idl", r#"
//!     #include "Foo.idl"
//!     #define MAX 100
//!     struct Bar { long limit; };
//! "#).expect("preprocess");
//!
//! assert!(result.expanded.contains("struct Foo"));
//! assert!(result.expanded.contains("struct Bar"));
//! assert!(!result.expanded.contains("#define"));
//! ```

#![allow(missing_docs)] // field-level doc-comments not complete everywhere

mod source_map;

pub use source_map::{FileId, SourceLocation, SourceMap};

use std::collections::HashMap;

/// Trait for include-file resolution.
///
/// Allows file IO (`FsResolver`), in-memory tests (`MemoryResolver`)
/// or custom strategies.
pub trait Resolver {
    /// Resolves a `#include "path"` (relative) or `#include <path>`
    /// (system) to source text.
    ///
    /// # Errors
    /// Implementation-specific. Should return a meaningful error
    /// that contains the requested path.
    fn resolve(&self, requesting_file: &str, include: &Include) -> Result<String, ResolveError>;
}

/// Describes an include request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Include {
    /// `#include "name"` — relative/local search.
    Quoted(String),
    /// `#include <name>` — system-path search.
    System(String),
}

impl Include {
    /// Path component independent of the style.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Quoted(p) | Self::System(p) => p,
        }
    }
}

/// Resolver error. Missing file, IO error, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    /// Requested path (as it appeared in the `#include`).
    pub requested: String,
    /// Meaningful description.
    pub message: String,
}

/// In-memory resolver for tests and CLI tools that manage source without
/// filesystem access.
#[derive(Debug, Clone, Default)]
pub struct MemoryResolver {
    files: HashMap<String, String>,
}

impl MemoryResolver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Registers a file.
    pub fn add(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.files.insert(name.into(), content.into());
    }
}

impl Resolver for MemoryResolver {
    fn resolve(&self, _requesting: &str, include: &Include) -> Result<String, ResolveError> {
        let path = include.path();
        self.files.get(path).cloned().ok_or_else(|| ResolveError {
            requested: path.to_string(),
            message: format!("file not in MemoryResolver: {path}"),
        })
    }
}

/// `#pragma prefix "<prefix>"` — CORBA Part 1 §14.7.5.
///
/// Collected by the preprocessor; checked by the consumer (spec validator) against
/// `typeprefix` decls for a repository-ID conflict (IDL 4.2
/// §7.4.6.4.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaPrefix {
    /// Prefix string (without surrounding quotes).
    pub prefix: String,
    /// Source file.
    pub file: String,
    /// Line (1-based).
    pub line: usize,
}

/// `#pragma keylist Foo a b c` — Cyclone/OpenSplice convention.
///
/// Collected by the preprocessor; can be read by the consumer (idlc, AST builder)
/// via [`ProcessedSource::pragma_keylists`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaKeylist {
    /// Topic-type name.
    pub type_name: String,
    /// Key member names.
    pub keys: Vec<String>,
    /// Source file.
    pub file: String,
    /// Line (1-based).
    pub line: usize,
}

/// OpenSplice-legacy-specific pragmas (`#pragma DCPS_DATA_TYPE`,
/// `#pragma DCPS_DATA_KEY`, `#pragma cats`, `#pragma genequality`).
///
/// In OpenSplice versions 5.x/6.x these pragmas were the primary
/// method of marking IDL types as DDS topics — before the OMG-IDL-4.2
/// annotation `@key`. Migration use cases must map them to modern
/// `@key`/`@topic` annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenSplicePragma {
    /// `#pragma DCPS_DATA_TYPE "<TypeName>"` — marks the type as a
    /// DDS-topic type.
    DataType {
        type_name: String,
        file: String,
        line: usize,
    },
    /// `#pragma DCPS_DATA_KEY "<TypeName>.<field>"` (dot form) or
    /// `#pragma DCPS_DATA_KEY <TypeName> <field>...` (OpenDDS space
    /// form, one or more fields) — marks members as keys.
    DataKey {
        type_name: String,
        /// One or more key fields.
        fields: Vec<String>,
        file: String,
        line: usize,
    },
    /// `#pragma cats <TypeName> <field>` — catenated keys
    /// (alternative key marking).
    Cats {
        type_name: String,
        keys: Vec<String>,
        file: String,
        line: usize,
    },
    /// `#pragma genequality` — codegen flag for the equality operator
    /// in C++/Java bindings.
    GenEquality { file: String, line: usize },
}

/// `#pragma dds_xtopics version="1.3"` (XTypes 1.3 §7.3.1.1.1) —
/// allows an IDL file to mark which XTypes spec
/// version it was written against. The compiler frontend can then
/// validate vendor extensions with/without a version match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaDdsXtopics {
    /// Version string (e.g. `"1.3"`). Empty if not specified.
    pub version: String,
    /// Source file.
    pub file: String,
    /// Line.
    pub line: usize,
}

/// Result of a preprocessor run.
#[derive(Debug, Clone, Default)]
pub struct ProcessedSource {
    /// Expanded IDL source, ready for the lexer.
    pub expanded: String,
    /// Mapping from output position to original (file, position).
    pub source_map: SourceMap,
    /// Collected `#pragma keylist` directives.
    pub pragma_keylists: Vec<PragmaKeylist>,
    /// Collected OpenSplice legacy pragmas (`DCPS_DATA_TYPE`,
    /// `DCPS_DATA_KEY`, `cats`, `genequality`).
    pub opensplice_pragmas: Vec<OpenSplicePragma>,
    /// Collected `#pragma prefix "<prefix>"` directives (CORBA Part 1
    /// §14.7.5). Used by the spec validator for §7.4.6.4.1.3 repository-ID
    /// conflict detection.
    pub pragma_prefixes: Vec<PragmaPrefix>,
    /// Collected `#pragma dds_xtopics version="..."` directives
    /// (XTypes 1.3 §7.3.1.1.1). Multiple pragmas per file are allowed;
    /// the validator checks version consistency.
    pub pragma_dds_xtopics: Vec<PragmaDdsXtopics>,
}

/// Top-level preprocessor error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    /// `#include` could not be resolved.
    IncludeNotFound(ResolveError),
    /// `#include` already in the expansion stack — cycle detected.
    IncludeCycle {
        /// File that was to be included cyclically.
        file: String,
    },
    /// `#endif` without a matching `#ifdef`/`#ifndef`.
    UnmatchedEndif {
        /// Source file of the unmatched directive.
        file: String,
        /// 1-indexed line.
        line: usize,
    },
    /// `#else` without a matching `#ifdef`/`#ifndef`.
    UnmatchedElse { file: String, line: usize },
    /// `#ifdef`/`#ifndef` without a closing `#endif`.
    UnclosedConditional { file: String, line: usize },
    /// Directive with faulty syntax (e.g. `#define` without a name).
    SyntaxError {
        file: String,
        line: usize,
        message: String,
    },
    /// `#error <message>` — explicitly requested build stop.
    ErrorDirective {
        /// Source file.
        file: String,
        /// Line.
        line: usize,
        /// Content of the directive.
        message: String,
    },
    /// Backslash as the last character in the source file — Spec §7.3:
    /// "A backslash character may not be the last character in a source
    /// file."
    TrailingBackslash {
        /// Source file.
        file: String,
    },
}

impl core::fmt::Display for PreprocessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IncludeNotFound(e) => write!(f, "include not found: {}", e.requested),
            Self::IncludeCycle { file } => write!(f, "include cycle: {file}"),
            Self::UnmatchedEndif { file, line } => {
                write!(f, "unmatched #endif at {file}:{line}")
            }
            Self::UnmatchedElse { file, line } => {
                write!(f, "unmatched #else at {file}:{line}")
            }
            Self::UnclosedConditional { file, line } => {
                write!(f, "unclosed conditional starting at {file}:{line}")
            }
            Self::SyntaxError {
                file,
                line,
                message,
            } => {
                write!(f, "preprocessor syntax error at {file}:{line}: {message}")
            }
            Self::ErrorDirective {
                file,
                line,
                message,
            } => {
                write!(f, "#error at {file}:{line}: {message}")
            }
            Self::TrailingBackslash { file } => {
                write!(f, "trailing backslash at end of source file: {file}")
            }
        }
    }
}

impl std::error::Error for PreprocessError {}

/// Top-level preprocessor.
///
/// Construction via [`Preprocessor::new`] with a [`Resolver`].
/// Then call `process(file_name, source)`.
pub struct Preprocessor<R: Resolver> {
    resolver: R,
}

impl<R: Resolver> Preprocessor<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }

    /// Processes a source string and expands all directives.
    ///
    /// `file_name` is shown in diagnostics and is the "current
    /// file" for relative `#include` resolution.
    ///
    /// # Errors
    /// See [`PreprocessError`].
    pub fn process(
        &self,
        file_name: &str,
        source: &str,
    ) -> Result<ProcessedSource, PreprocessError> {
        let mut state = State::new();
        let root_id = state.source_map.add_file(file_name);
        let mut output = String::new();
        // Backslash-Newline-Continuation (Spec §7.3, ISO 14882 5.2):
        // A `\` directly before `\n` is replaced by a token splice — i.e.
        // both characters are removed, the following line continues
        // syntactically. We do this before the line iteration, so that
        // multi-line `#define X foo \\\n bar` is recognized correctly.
        let spliced = splice_backslash_newlines(source);
        // Spec §7.3: "A backslash character may not be the last character
        // in a source file." If a backslash remains at the file end
        // after splicing, it was not followed by a `\n`.
        if spliced.ends_with('\\') {
            return Err(PreprocessError::TrailingBackslash {
                file: file_name.to_string(),
            });
        }
        self.expand_into(file_name, &spliced, root_id, &mut state, &mut output, 0)?;
        Ok(ProcessedSource {
            expanded: output,
            source_map: state.source_map,
            pragma_keylists: state.pragma_keylists,
            opensplice_pragmas: state.opensplice_pragmas,
            pragma_prefixes: state.pragma_prefixes,
            pragma_dds_xtopics: state.pragma_dds_xtopics,
        })
    }

    fn expand_into(
        &self,
        file_name: &str,
        source: &str,
        file_id: FileId,
        state: &mut State,
        output: &mut String,
        depth: usize,
    ) -> Result<(), PreprocessError> {
        if state.include_stack.iter().any(|f| f == file_name) {
            return Err(PreprocessError::IncludeCycle {
                file: file_name.to_string(),
            });
        }
        state.include_stack.push(file_name.to_string());

        let mut conditional_stack: Vec<ConditionalFrame> = Vec::new();
        let mut byte_offset = 0usize;

        for (line_idx, line) in source.split_inclusive('\n').enumerate() {
            let line_no = line_idx + 1;
            let trimmed = line.trim_start();

            // Conditional skipping: if currently in an inactive
            // frame, skip everything except #else/#endif/#ifdef/#ifndef.
            let active = conditional_stack.iter().all(|f| f.active);

            if let Some(directive) = parse_directive(trimmed) {
                match directive {
                    Directive::Ifdef(name) => {
                        let parent_active = active;
                        let cond = parent_active && state.macros.contains_key(name);
                        conditional_stack.push(ConditionalFrame {
                            active: cond,
                            else_seen: false,
                            parent_active,
                            taken: cond,
                        });
                    }
                    Directive::Ifndef(name) => {
                        let parent_active = active;
                        let cond = parent_active && !state.macros.contains_key(name);
                        conditional_stack.push(ConditionalFrame {
                            active: cond,
                            else_seen: false,
                            parent_active,
                            taken: cond,
                        });
                    }
                    Directive::Else => {
                        let frame = conditional_stack.last_mut().ok_or_else(|| {
                            PreprocessError::UnmatchedElse {
                                file: file_name.to_string(),
                                line: line_no,
                            }
                        })?;
                        if frame.else_seen {
                            return Err(PreprocessError::SyntaxError {
                                file: file_name.to_string(),
                                line: line_no,
                                message: "duplicate #else".to_string(),
                            });
                        }
                        frame.else_seen = true;
                        // Else activates only if the parent is active AND
                        // NO branch has been taken so far.
                        frame.active = frame.parent_active && !frame.taken;
                        if frame.active {
                            frame.taken = true;
                        }
                    }
                    Directive::Endif => {
                        if conditional_stack.pop().is_none() {
                            return Err(PreprocessError::UnmatchedEndif {
                                file: file_name.to_string(),
                                line: line_no,
                            });
                        }
                    }
                    Directive::If(expr) => {
                        let parent_active = active;
                        let cond = parent_active && eval_if_expr(expr, &state.macros);
                        conditional_stack.push(ConditionalFrame {
                            parent_active,
                            active: cond,
                            else_seen: false,
                            taken: cond,
                        });
                    }
                    Directive::Elif(expr) => {
                        let Some(frame) = conditional_stack.last_mut() else {
                            return Err(PreprocessError::UnmatchedEndif {
                                file: file_name.to_string(),
                                line: line_no,
                            });
                        };
                        if frame.else_seen {
                            return Err(PreprocessError::SyntaxError {
                                file: file_name.to_string(),
                                line: line_no,
                                message: "#elif after #else".to_string(),
                            });
                        }
                        // Active only if the parent is active AND no
                        // branch has been taken yet AND expr=true.
                        let cond = frame.parent_active
                            && !frame.taken
                            && eval_if_expr(expr, &state.macros);
                        frame.active = cond;
                        if cond {
                            frame.taken = true;
                        }
                    }
                    _ if !active => {
                        // In an inactive block — do not execute other
                        // directives.
                    }
                    Directive::Define(name, def) => {
                        state.macros.insert(name.to_string(), def);
                    }
                    Directive::Undef(name) => {
                        state.macros.remove(name);
                    }
                    Directive::Include(inc) => {
                        if depth > MAX_INCLUDE_DEPTH {
                            return Err(PreprocessError::SyntaxError {
                                file: file_name.to_string(),
                                line: line_no,
                                message: format!("include depth exceeded {MAX_INCLUDE_DEPTH}"),
                            });
                        }
                        let inc_path = inc.path().to_string();
                        // Cycle detection before resolve, so that cycles
                        // are detected independently of the resolver.
                        if state.include_stack.iter().any(|f| f == &inc_path) {
                            return Err(PreprocessError::IncludeCycle { file: inc_path });
                        }
                        let included = self
                            .resolver
                            .resolve(file_name, &inc)
                            .map_err(PreprocessError::IncludeNotFound)?;
                        let inc_id = state.source_map.add_file(&inc_path);
                        self.expand_into(&inc_path, &included, inc_id, state, output, depth + 1)?;
                    }
                    Directive::Pragma(args) => {
                        if let Some(keylist) = parse_pragma_keylist(args, file_name, line_no) {
                            state.pragma_keylists.push(keylist);
                        } else if let Some(osp) = parse_opensplice_pragma(args, file_name, line_no)
                        {
                            state.opensplice_pragmas.push(osp);
                        } else if let Some(pp) = parse_pragma_prefix(args, file_name, line_no) {
                            state.pragma_prefixes.push(pp);
                        } else if let Some(xt) = parse_pragma_dds_xtopics(args, file_name, line_no)
                        {
                            state.pragma_dds_xtopics.push(xt);
                        }
                        // Other pragmas: stripped.
                    }
                    Directive::Error(msg) => {
                        return Err(PreprocessError::ErrorDirective {
                            file: file_name.to_string(),
                            line: line_no,
                            message: msg.trim().to_string(),
                        });
                    }
                    Directive::Warning(_msg) => {
                        // `#warning` is a diagnostic without abort.
                        // stripped; the UI layer can render warnings via a hook
                        // (comes with the diagnostic reporter).
                    }
                    Directive::Line(_args) => {
                        // `#line N "file"` — position override.
                        // SourceMap integration is still pending.
                    }
                }
            } else if active {
                // Normal source line: expand macros and append to the
                // output.
                let expanded = expand_macros(line, &state.macros);
                state
                    .source_map
                    .record_segment(output.len(), expanded.len(), file_id, byte_offset);
                output.push_str(&expanded);
            }

            byte_offset += line.len();
        }

        if let Some(frame) = conditional_stack.first() {
            let _ = frame;
            return Err(PreprocessError::UnclosedConditional {
                file: file_name.to_string(),
                line: 0,
            });
        }

        state.include_stack.pop();
        Ok(())
    }
}

const MAX_INCLUDE_DEPTH: usize = 64;

struct State {
    macros: HashMap<String, MacroDef>,
    include_stack: Vec<String>,
    source_map: SourceMap,
    pragma_keylists: Vec<PragmaKeylist>,
    opensplice_pragmas: Vec<OpenSplicePragma>,
    pragma_prefixes: Vec<PragmaPrefix>,
    pragma_dds_xtopics: Vec<PragmaDdsXtopics>,
}

impl State {
    fn new() -> Self {
        Self {
            macros: HashMap::new(),
            include_stack: Vec::new(),
            source_map: SourceMap::new(),
            pragma_keylists: Vec::new(),
            opensplice_pragmas: Vec::new(),
            pragma_prefixes: Vec::new(),
            pragma_dds_xtopics: Vec::new(),
        }
    }
}

/// Definition of a `#define` macro — either object-like or
/// function-like (with a parameter list).
#[derive(Clone, Debug, PartialEq, Eq)]
struct MacroDef {
    /// `Some(params)` for function-like macros (`#define NAME(p1, p2) body`),
    /// `None` for object-like (`#define NAME body`).
    params: Option<Vec<String>>,
    /// Macro-Body (unexpandiert).
    body: String,
}

impl MacroDef {
    fn object_like(body: &str) -> Self {
        Self {
            params: None,
            body: body.to_string(),
        }
    }

    fn function_like(params: Vec<String>, body: &str) -> Self {
        Self {
            params: Some(params),
            body: body.to_string(),
        }
    }
}

/// Splice backslash-newline pairs (token splicing) per §7.3 / ISO 14882.
fn splice_backslash_newlines(src: &str) -> String {
    // We work byte-oriented; UTF-8 compatible because `\` and `\n` are ASCII.
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            // Skip both bytes.
            i += 2;
            continue;
        }
        if bytes[i] == b'\\'
            && i + 2 < bytes.len()
            && bytes[i + 1] == b'\r'
            && bytes[i + 2] == b'\n'
        {
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Safe: only ASCII bytes were removed → stays valid UTF-8.
    String::from_utf8(out).unwrap_or_default()
}

struct ConditionalFrame {
    /// `true` if the current branch is active (tokens are emitted).
    active: bool,
    /// `true` if `#else` has already been seen (a second `#else` is an error).
    else_seen: bool,
    /// Was the parent frame active? If not, this branch is inactive
    /// anyway — important for correct #else toggling in nested cases.
    parent_active: bool,
    /// `true` as soon as any branch (`#if`/`#elif`) was chosen as active.
    /// Subsequent `#elif`/`#else` are ignored. (#if/#elif eval)
    taken: bool,
}

/// Erkennbare Preprocessor-Direktiven.
#[derive(Debug, PartialEq, Eq)]
enum Directive<'a> {
    Include(Include),
    Define(&'a str, MacroDef),
    Undef(&'a str),
    Ifdef(&'a str),
    Ifndef(&'a str),
    /// `#if <const-expr>` — simplified expression eval:
    /// `defined(MACRO)`, `0`/`1`, as well as `&&`/`||`/`!`.
    /// (spec stage 2; gated via the `preprocessor_full` feature.)
    If(&'a str),
    /// `#elif <const-expr>` — variant of `#if`.
    Elif(&'a str),
    Else,
    Endif,
    Pragma(&'a str),
    /// `#error <message>` — aborts the build.
    Error(&'a str),
    /// `#warning <message>` — diagnostic without abort
    /// (gated via the `preprocessor_warning_line` feature).
    Warning(&'a str),
    /// `#line <linenum> ["filename"]` — source position override
    /// (gated via the `preprocessor_warning_line` feature; /// stripped, SourceMap update follows when needed).
    Line(&'a str),
}

fn parse_directive(line: &str) -> Option<Directive<'_>> {
    let stripped = line.strip_prefix('#')?.trim_start();
    let (head, rest) = match stripped.find(|c: char| c.is_whitespace()) {
        Some(idx) => (&stripped[..idx], stripped[idx..].trim()),
        None => (stripped.trim_end(), ""),
    };
    match head {
        "include" => parse_include(rest).map(Directive::Include),
        "define" => parse_define(rest),
        "undef" => Some(Directive::Undef(rest)),
        "ifdef" => Some(Directive::Ifdef(rest)),
        "ifndef" => Some(Directive::Ifndef(rest)),
        "if" => Some(Directive::If(rest)),
        "elif" => Some(Directive::Elif(rest)),
        "else" => Some(Directive::Else),
        "endif" => Some(Directive::Endif),
        "pragma" => Some(Directive::Pragma(rest)),
        "error" => Some(Directive::Error(rest)),
        "warning" => Some(Directive::Warning(rest)),
        "line" => Some(Directive::Line(rest)),
        _ => None,
    }
}

/// Simplified `#if`-expression evaluation (Spec §7.3.2 + ISO 14882
/// constant-expression subset).
///
/// Supports:
/// - Numeric literals: `0` (false), everything else (true).
/// - `defined(MACRO)` and `defined MACRO` — true if the macro is defined.
/// - Boolean operators `&&`/`||`/`!` (left-to-right, no precedence).
/// - Macro identifiers are interpreted as undefined (false),
///   unless they are explicitly wrapped as `defined(...)`.
///
/// Full C-preprocessor expression eval (arithmetic, comparisons,
/// bitops, ternary) is not implemented.
fn eval_if_expr(expr: &str, macros: &HashMap<String, MacroDef>) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Tokenize einfach.
    let normalized = normalize_if_tokens(trimmed);
    eval_if_tokens(&normalized, macros)
}

fn normalize_if_tokens(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => {}
            '(' | ')' | '!' => out.push(c.to_string()),
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                out.push("&&".into());
            }
            '|' if chars.peek() == Some(&'|') => {
                chars.next();
                out.push("||".into());
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut buf = String::from(c);
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_alphanumeric() || n == '_' {
                        buf.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(buf);
            }
            c if c.is_ascii_digit() => {
                let mut buf = String::from(c);
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_digit() {
                        buf.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(buf);
            }
            _ => {} // ignore unknown characters (defensive)
        }
    }
    out
}

/// zerodds-lint: recursion-depth 64 (If-Expr; bounded by IDL nesting)
fn eval_if_tokens(tokens: &[String], macros: &HashMap<String, MacroDef>) -> bool {
    let (val, _) = eval_or(tokens, 0, macros);
    val
}

fn eval_or(tokens: &[String], idx: usize, macros: &HashMap<String, MacroDef>) -> (bool, usize) {
    let (mut left, mut i) = eval_and(tokens, idx, macros);
    while tokens.get(i).map(String::as_str) == Some("||") {
        let (right, ni) = eval_and(tokens, i + 1, macros);
        left = left || right;
        i = ni;
    }
    (left, i)
}

fn eval_and(tokens: &[String], idx: usize, macros: &HashMap<String, MacroDef>) -> (bool, usize) {
    let (mut left, mut i) = eval_not(tokens, idx, macros);
    while tokens.get(i).map(String::as_str) == Some("&&") {
        let (right, ni) = eval_not(tokens, i + 1, macros);
        left = left && right;
        i = ni;
    }
    (left, i)
}

/// zerodds-lint: recursion-depth 16 (logical-not chain; bounded by IDL macro nesting)
fn eval_not(tokens: &[String], idx: usize, macros: &HashMap<String, MacroDef>) -> (bool, usize) {
    if tokens.get(idx).map(String::as_str) == Some("!") {
        let (v, ni) = eval_not(tokens, idx + 1, macros);
        return (!v, ni);
    }
    eval_atom(tokens, idx, macros)
}

fn eval_atom(tokens: &[String], idx: usize, macros: &HashMap<String, MacroDef>) -> (bool, usize) {
    let Some(tok) = tokens.get(idx) else {
        return (false, idx);
    };
    if tok == "(" {
        let (v, ni) = eval_or(tokens, idx + 1, macros);
        let after = if tokens.get(ni).map(String::as_str) == Some(")") {
            ni + 1
        } else {
            ni
        };
        return (v, after);
    }
    if tok == "defined" {
        // `defined(MACRO)` or `defined MACRO`.
        let (next_idx, ident) = if tokens.get(idx + 1).map(String::as_str) == Some("(") {
            (
                idx + 3,
                tokens.get(idx + 2).map(String::as_str).unwrap_or(""),
            )
        } else {
            (
                idx + 2,
                tokens.get(idx + 1).map(String::as_str).unwrap_or(""),
            )
        };
        let v = macros.contains_key(ident);
        let after = if tokens.get(idx + 1).map(String::as_str) == Some("(") {
            // Skip closing ')'.
            if tokens.get(next_idx).map(String::as_str) == Some(")") {
                next_idx + 1
            } else {
                next_idx
            }
        } else {
            next_idx
        };
        return (v, after);
    }
    // Numeric literal: `0` = false, otherwise true.
    if let Ok(n) = tok.parse::<i64>() {
        return (n != 0, idx + 1);
    }
    // Identifier without `defined()` — treat as a macro-value lookup;
    // if the macro body parses as an int → accordingly; otherwise true (macro
    // exists). Function-like macros are treated as true here
    // (call parameters in the `#if` context are unusual).
    if let Some(def) = macros.get(tok) {
        if let Ok(n) = def.body.trim().parse::<i64>() {
            return (n != 0, idx + 1);
        }
        return (true, idx + 1);
    }
    // Unknown identifier → false (spec C-PP convention).
    (false, idx + 1)
}

fn parse_include(rest: &str) -> Option<Include> {
    let rest = rest.trim();
    if let Some(stripped) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Some(Include::Quoted(stripped.to_string()));
    }
    rest.strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .map(|stripped| Include::System(stripped.to_string()))
}

fn parse_define(rest: &str) -> Option<Directive<'_>> {
    let rest = rest.trim_end_matches('\n').trim();
    if rest.is_empty() {
        return None;
    }
    // Identifier part up to the first whitespace OR `(`. A directly
    // following `(` without whitespace marks a function-like
    // macro (spec §7.2.5 + ISO 14882 §16.3).
    let name_end = rest
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    if name.is_empty() {
        return None;
    }
    let after_name = &rest[name_end..];
    if let Some(after_paren) = after_name.strip_prefix('(') {
        // function-like: `NAME(p1, p2, ...) body`
        let close = after_paren.find(')')?;
        let params_src = &after_paren[..close];
        let body = after_paren[close + 1..].trim();
        let params: Vec<String> = if params_src.trim().is_empty() {
            Vec::new()
        } else {
            params_src
                .split(',')
                .map(|p| p.trim().to_string())
                .collect()
        };
        return Some(Directive::Define(
            name,
            MacroDef::function_like(params, body),
        ));
    }
    let body = after_name.trim();
    Some(Directive::Define(name, MacroDef::object_like(body)))
}

/// `#pragma prefix "<prefix>"` — CORBA Part 1 §14.7.5.
///
/// Returns `None` if the pragma is not a prefix pragma or the
/// string argument is missing/empty.
fn parse_pragma_prefix(args: &str, file: &str, line: usize) -> Option<PragmaPrefix> {
    let trimmed = args.trim();
    let rest = trimmed.strip_prefix("prefix")?.trim_start();
    let prefix = strip_optional_quotes(rest).trim().to_string();
    if prefix.is_empty() {
        return None;
    }
    Some(PragmaPrefix {
        prefix,
        file: file.to_string(),
        line,
    })
}

/// `#pragma dds_xtopics version="1.3"` — XTypes 1.3 §7.3.1.1.1.
///
/// Returns `None` if the pragma is not a dds_xtopics pragma.
fn parse_pragma_dds_xtopics(args: &str, file: &str, line: usize) -> Option<PragmaDdsXtopics> {
    let trimmed = args.trim();
    let rest = trimmed.strip_prefix("dds_xtopics")?.trim_start();
    // Accepts both `version="1.3"` and `version=1.3`.
    let version = if rest.is_empty() {
        String::new()
    } else if let Some(v) = rest.strip_prefix("version") {
        v.trim_start()
            .strip_prefix('=')
            .unwrap_or(v)
            .trim()
            .trim_matches('"')
            .to_string()
    } else {
        rest.trim_matches('"').to_string()
    };
    Some(PragmaDdsXtopics {
        version,
        file: file.to_string(),
        line,
    })
}

/// `#pragma keylist <Type> <field>*` — Cyclone DDS convention.
///
/// Returns `None` if the pragma is not a keylist pragma.
fn parse_pragma_keylist(args: &str, file: &str, line: usize) -> Option<PragmaKeylist> {
    let trimmed = args.trim();
    let rest = trimmed.strip_prefix("keylist")?.trim_start();
    let mut parts = rest.split_whitespace();
    let type_name = parts.next()?.to_string();
    let keys: Vec<String> = parts.map(str::to_string).collect();
    Some(PragmaKeylist {
        type_name,
        keys,
        file: file.to_string(),
        line,
    })
}

/// Parses OpenSplice legacy pragmas (`DCPS_DATA_TYPE`, `DCPS_DATA_KEY`,
/// `cats`, `genequality`).
fn parse_opensplice_pragma(args: &str, file: &str, line: usize) -> Option<OpenSplicePragma> {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("DCPS_DATA_TYPE") {
        let payload = rest.trim();
        // Accepts both quoted ("Type") and unquoted (Type).
        let type_name = strip_optional_quotes(payload).to_string();
        if type_name.is_empty() {
            return None;
        }
        return Some(OpenSplicePragma::DataType {
            type_name,
            file: file.to_string(),
            line,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("DCPS_DATA_KEY") {
        let payload = strip_optional_quotes(rest.trim());
        // Zwei akzeptierte Formen:
        //   "Type.field"            — dot separator (legacy)
        //   "Type field1 field2..."  — OpenDDS space form (1..n fields)
        let (type_name, fields): (String, Vec<String>) = if let Some(dot) = payload.find('.') {
            let ty = payload[..dot].trim().to_string();
            let f = payload[dot + 1..].trim().to_string();
            (ty, vec![f])
        } else {
            let mut parts = payload.split_whitespace();
            let ty = parts.next()?.to_string();
            (ty, parts.map(str::to_string).collect())
        };
        if type_name.is_empty() || fields.iter().any(String::is_empty) || fields.is_empty() {
            return None;
        }
        return Some(OpenSplicePragma::DataKey {
            type_name,
            fields,
            file: file.to_string(),
            line,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("cats") {
        let mut parts = rest.split_whitespace();
        let type_name = parts.next()?.to_string();
        let keys: Vec<String> = parts.map(str::to_string).collect();
        if keys.is_empty() {
            return None;
        }
        return Some(OpenSplicePragma::Cats {
            type_name,
            keys,
            file: file.to_string(),
            line,
        });
    }
    if trimmed == "genequality" {
        return Some(OpenSplicePragma::GenEquality {
            file: file.to_string(),
            line,
        });
    }
    None
}

fn strip_optional_quotes(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(s)
}

/// Object-like macro substitution. Iterates over identifier tokens and
/// replaces macro names with their values. Simplified variant — no
/// re-expansion (no macro-in-macro), no function-like macros.
fn expand_macros(line: &str, macros: &HashMap<String, MacroDef>) -> String {
    expand_macros_rec(line, macros, 0)
}

/// Maximum depth for recursive `#define` expansion. Protects against
/// pathological `#define A B` / `#define B A` cycles and against
/// indirect cycles over 2+ hops. The value covers typical
/// IDL const paths (≤ 8 hops) with ample reserve.
const MAX_MACRO_EXPANSION_DEPTH: usize = 32;

/// zerodds-lint: recursion-depth 32
fn expand_macros_rec(line: &str, macros: &HashMap<String, MacroDef>, depth: usize) -> String {
    if macros.is_empty() || depth >= MAX_MACRO_EXPANSION_DEPTH {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut expanded_any = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() || c == b'_' {
            // Identifier scannen.
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &line[start..i];
            let Some(def) = macros.get(ident) else {
                out.push_str(ident);
                continue;
            };
            expanded_any = true;
            match &def.params {
                None => out.push_str(&def.body),
                Some(params) => {
                    // function-like: `(` expected directly (or after whitespace),
                    // otherwise pass the identifier through.
                    let after = skip_ascii_ws(bytes, i);
                    if after >= bytes.len() || bytes[after] != b'(' {
                        out.push_str(ident);
                        continue;
                    }
                    let Some((args, end)) = parse_call_args(line, after) else {
                        out.push_str(ident);
                        continue;
                    };
                    let expanded = expand_function_like(params, &args, &def.body);
                    out.push_str(&expanded);
                    i = end;
                }
            }
        } else {
            out.push(c as char);
            i += 1;
        }
    }
    // If something was expanded, the expansion itself may contain further
    // macro refs — re-expand recursively until the fixed point
    // is reached. `MAX_MACRO_EXPANSION_DEPTH` protects against
    // self-recursive `#define A A` pathologies.
    if expanded_any && out != line {
        return expand_macros_rec(&out, macros, depth + 1);
    }
    out
}

fn skip_ascii_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    i
}

/// Parses `( arg1 , arg2 , ... )` from position `start` (points at `(`).
/// Returns the arguments and the index directly after `)`. Commas inside
/// parenthesis pairs are ignored (nested calls).
fn parse_call_args(line: &str, start: usize) -> Option<(Vec<String>, usize)> {
    let bytes = line.as_bytes();
    debug_assert_eq!(bytes.get(start), Some(&b'('));
    let mut i = start + 1;
    let mut depth: usize = 1;
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
                i += 1;
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(cur.trim().to_string());
                    return Some((args, i + 1));
                }
                cur.push(c);
                i += 1;
            }
            ',' if depth == 1 => {
                args.push(cur.trim().to_string());
                cur.clear();
                i += 1;
            }
            _ => {
                cur.push(c);
                i += 1;
            }
        }
    }
    None
}

/// Substituiert Parameter im function-like-Macro-Body. Erkennt
/// `#param` (stringize, ISO 14882 §16.3.2) and `a##b` (token paste,
/// §16.3.3).
fn expand_function_like(params: &[String], args: &[String], body: &str) -> String {
    let arg_for = |name: &str| -> Option<&str> {
        params
            .iter()
            .position(|p| p == name)
            .and_then(|idx| args.get(idx).map(String::as_str))
    };
    // Tokenisiere Body grob: Identifier vs. Rest.
    let mut tokens: Vec<BodyTok> = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            tokens.push(BodyTok::Ident(body[start..i].to_string()));
        } else if c == b'#' && i + 1 < bytes.len() && bytes[i + 1] == b'#' {
            tokens.push(BodyTok::Paste);
            i += 2;
        } else if c == b'#' {
            tokens.push(BodyTok::Stringize);
            i += 1;
        } else {
            tokens.push(BodyTok::Other((c as char).to_string()));
            i += 1;
        }
    }
    // Token-paste pass: binds `<a> ## <b>` into `<a><b>`. Whitespace
    // directly around `##` is discarded. On a match the preceding
    // entry is popped from `after_paste` and concatenated with the right operand
    // (param-substituted).
    let mut after_paste: Vec<BodyTok> = Vec::with_capacity(tokens.len());
    let mut k = 0;
    while k < tokens.len() {
        if matches!(tokens[k], BodyTok::Paste) {
            // Skip whitespace tokens left/right of the `##`:
            // whitespace on the left is already in `after_paste` — so we
            // search for the last non-whitespace entry.
            let mut lhs: Option<BodyTok> = None;
            while let Some(last) = after_paste.last() {
                if let BodyTok::Other(s) = last {
                    if s.chars().all(char::is_whitespace) {
                        after_paste.pop();
                        continue;
                    }
                }
                lhs = after_paste.pop();
                break;
            }
            let rhs_idx = skip_body_ws(&tokens, k + 1);
            match (lhs, tokens.get(rhs_idx)) {
                (Some(lhs_tok), Some(rhs_tok)) => {
                    let lhs_text = render_tok(&lhs_tok, params, args);
                    let rhs_text = render_tok(rhs_tok, params, args);
                    after_paste.push(BodyTok::Ident(format!("{lhs_text}{rhs_text}")));
                    k = rhs_idx + 1;
                }
                _ => {
                    // Stand-alone `##` without operands — keep it as a literal.
                    after_paste.push(BodyTok::Paste);
                    k += 1;
                }
            }
        } else {
            after_paste.push(tokens[k].clone());
            k += 1;
        }
    }
    // Render pass: stringize and param substitution.
    let mut out = String::new();
    let mut j = 0;
    while j < after_paste.len() {
        match &after_paste[j] {
            BodyTok::Stringize => {
                let target_idx = skip_body_ws(&after_paste, j + 1);
                let arg_text = match after_paste.get(target_idx) {
                    Some(BodyTok::Ident(name)) => arg_for(name).unwrap_or(name).to_string(),
                    _ => String::new(),
                };
                out.push('"');
                for ch in arg_text.chars() {
                    if ch == '"' || ch == '\\' {
                        out.push('\\');
                    }
                    out.push(ch);
                }
                out.push('"');
                j = target_idx + 1;
            }
            BodyTok::Ident(name) => {
                if let Some(text) = arg_for(name) {
                    out.push_str(text);
                } else {
                    out.push_str(name);
                }
                j += 1;
            }
            BodyTok::Other(s) => {
                out.push_str(s);
                j += 1;
            }
            BodyTok::Paste => {
                // Stand-alone `##` without an LHS — emit as a literal
                // (should no longer occur after the paste pass).
                out.push_str("##");
                j += 1;
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
enum BodyTok {
    Ident(String),
    Other(String),
    Stringize,
    Paste,
}

fn skip_body_ws(tokens: &[BodyTok], mut i: usize) -> usize {
    while let Some(BodyTok::Other(s)) = tokens.get(i) {
        if !s.chars().all(char::is_whitespace) {
            break;
        }
        i += 1;
    }
    i
}

fn render_tok(tok: &BodyTok, params: &[String], args: &[String]) -> String {
    match tok {
        BodyTok::Ident(name) => params
            .iter()
            .position(|p| p == name)
            .and_then(|idx| args.get(idx).cloned())
            .unwrap_or_else(|| name.clone()),
        BodyTok::Other(s) => s.clone(),
        BodyTok::Stringize => "#".to_string(),
        BodyTok::Paste => "##".to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    fn run(src: &str) -> String {
        Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok")
            .expanded
    }

    fn run_with(resolver: MemoryResolver, src: &str) -> String {
        Preprocessor::new(resolver)
            .process("main.idl", src)
            .expect("ok")
            .expanded
    }

    #[test]
    fn passthrough_for_source_without_directives() {
        let out = run("struct Foo { long x; };\n");
        assert!(out.contains("struct Foo"));
    }

    #[test]
    fn pragma_is_stripped() {
        let out = run("#pragma keylist Foo x\nstruct Foo { long x; };\n");
        assert!(!out.contains("#pragma"));
        assert!(out.contains("struct Foo"));
    }

    #[test]
    fn define_object_like_substitutes_in_subsequent_lines() {
        let out = run("#define MAX 100\nconst long L = MAX;\n");
        assert!(out.contains("const long L = 100;"), "{out}");
        assert!(!out.contains("#define"));
    }

    #[test]
    fn ifdef_keeps_block_when_macro_defined() {
        let out = run("#define WITH\n#ifdef WITH\nstruct A {};\n#endif\n");
        assert!(out.contains("struct A"), "{out}");
    }

    #[test]
    fn ifdef_drops_block_when_macro_not_defined() {
        let out = run("#ifdef WITH\nstruct A {};\n#endif\n");
        assert!(!out.contains("struct A"), "{out}");
    }

    #[test]
    fn ifndef_inverse_of_ifdef() {
        let out = run("#ifndef WITH\nstruct B {};\n#endif\n");
        assert!(out.contains("struct B"), "{out}");
    }

    #[test]
    fn else_branch_taken_when_initial_false() {
        let out = run("#ifdef NOPE\nstruct A {};\n#else\nstruct B {};\n#endif\n");
        assert!(!out.contains("struct A"), "{out}");
        assert!(out.contains("struct B"), "{out}");
    }

    #[test]
    fn nested_ifdef_works() {
        let out = run("#define X\n\
             #ifdef X\n\
                #ifdef Y\nstruct YY {};\n#else\nstruct XnotY {};\n#endif\n\
             #endif\n");
        assert!(out.contains("struct XnotY"), "{out}");
        assert!(!out.contains("struct YY"));
    }

    #[test]
    fn undef_removes_macro() {
        let out = run("#define M\n#undef M\n#ifdef M\nA\n#endif\n");
        assert!(!out.contains('A'), "{out}");
    }

    #[test]
    fn quoted_include_resolves() {
        let mut r = MemoryResolver::new();
        r.add("inc.idl", "struct Inc {};\n");
        let out = run_with(r, "#include \"inc.idl\"\nstruct Main {};\n");
        assert!(out.contains("struct Inc"), "{out}");
        assert!(out.contains("struct Main"), "{out}");
    }

    #[test]
    fn system_include_resolves() {
        let mut r = MemoryResolver::new();
        r.add("sys.idl", "struct Sys {};\n");
        let out = run_with(r, "#include <sys.idl>\nstruct Main {};\n");
        assert!(out.contains("struct Sys"), "{out}");
    }

    #[test]
    fn missing_include_is_error() {
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", "#include \"missing.idl\"\n");
        assert!(matches!(res, Err(PreprocessError::IncludeNotFound(_))));
    }

    #[test]
    fn include_cycle_is_detected() {
        let mut r = MemoryResolver::new();
        r.add("a.idl", "#include \"main.idl\"\n");
        let res = Preprocessor::new(r).process("main.idl", "#include \"a.idl\"\n");
        assert!(matches!(res, Err(PreprocessError::IncludeCycle { .. })));
    }

    #[test]
    fn unmatched_endif_is_error() {
        let res = Preprocessor::new(MemoryResolver::new()).process("main.idl", "#endif\n");
        assert!(matches!(res, Err(PreprocessError::UnmatchedEndif { .. })));
    }

    #[test]
    fn unclosed_conditional_is_error() {
        let res = Preprocessor::new(MemoryResolver::new()).process("main.idl", "#ifdef X\n");
        assert!(matches!(
            res,
            Err(PreprocessError::UnclosedConditional { .. })
        ));
    }

    #[test]
    fn unmatched_else_is_error() {
        let res = Preprocessor::new(MemoryResolver::new()).process("main.idl", "#else\n");
        assert!(matches!(res, Err(PreprocessError::UnmatchedElse { .. })));
    }

    #[test]
    fn macro_in_inactive_branch_does_not_take_effect() {
        let out = run("#ifdef NOPE\n#define M 99\n#endif\n#ifdef M\nseen\n#endif\n");
        assert!(!out.contains("seen"));
    }

    #[test]
    fn source_map_records_segments() {
        let result = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", "struct A {};\nstruct B {};\n")
            .expect("ok");
        // At least two segments for two lines.
        assert!(
            result.source_map.segment_count() >= 2,
            "got {} segments",
            result.source_map.segment_count()
        );
    }

    #[test]
    fn expand_macros_skips_unknown_identifiers() {
        let macros = HashMap::new();
        let out = expand_macros("foo bar baz", &macros);
        assert_eq!(out, "foo bar baz");
    }

    #[test]
    fn expand_macros_substitutes_only_full_idents() {
        let mut m = HashMap::new();
        m.insert("X".to_string(), MacroDef::object_like("100"));
        // `XY` contains `X` as a substring, but must not be replaced.
        let out = expand_macros("X XY", &m);
        assert_eq!(out, "100 XY");
    }

    // -----------------------------------------------------------------
    // §7.3 stage 2 — #if/#elif/#warning/#line (B7)
    // -----------------------------------------------------------------

    #[test]
    fn if_eval_defined_macro_keeps_block() {
        let src = "\
#define FOO 1
#if defined(FOO)
struct InFoo { long x; };
#endif
struct After { long y; };
";
        let out = run(src);
        assert!(out.contains("struct InFoo"), "got: {out}");
        assert!(out.contains("struct After"), "got: {out}");
    }

    #[test]
    fn if_eval_undefined_macro_drops_block() {
        let src = "\
#if defined(FOO)
struct ShouldBeGone { long x; };
#endif
struct Visible { long y; };
";
        let out = run(src);
        assert!(!out.contains("ShouldBeGone"), "got: {out}");
        assert!(out.contains("struct Visible"));
    }

    #[test]
    fn if_eval_numeric_zero_drops_block() {
        let src = "#if 0\nstruct X { long x; };\n#endif\nstruct Y {};\n";
        let out = run(src);
        assert!(!out.contains("struct X"), "got: {out}");
    }

    #[test]
    fn if_eval_numeric_nonzero_keeps_block() {
        let src = "#if 1\nstruct X { long x; };\n#endif\n";
        let out = run(src);
        assert!(out.contains("struct X"), "got: {out}");
    }

    #[test]
    fn if_eval_logical_or() {
        let src = "\
#define A 1
#if defined(A) || defined(B)
struct Match { long m; };
#endif
";
        let out = run(src);
        assert!(out.contains("struct Match"), "got: {out}");
    }

    #[test]
    fn if_eval_logical_not() {
        let src = "#if !defined(NOT_DEFINED)\nstruct K {};\n#endif\n";
        let out = run(src);
        assert!(out.contains("struct K"), "got: {out}");
    }

    #[test]
    fn if_eval_logical_and_both_defined_keeps_block() {
        // §7.2.5: `&&` in `#if`-Eval — beide Operanden true.
        let src = "\
#define A 1
#define B 1
#if defined(A) && defined(B)
struct Both {};
#endif
";
        let out = run(src);
        assert!(out.contains("struct Both"), "got: {out}");
    }

    #[test]
    fn if_eval_logical_and_one_undefined_drops_block() {
        // §7.2.5: `&&` with one undefined operand → false.
        let src = "\
#define A 1
#if defined(A) && defined(NOT_DEFINED)
struct OnlyA {};
#endif
";
        let out = run(src);
        assert!(!out.contains("struct OnlyA"), "got: {out}");
    }

    #[test]
    fn if_eval_logical_and_both_undefined_drops_block() {
        // §7.2.5: `&&` with both undefined → false.
        let src = "\
#if defined(NOT_A) && defined(NOT_B)
struct Neither {};
#endif
";
        let out = run(src);
        assert!(!out.contains("struct Neither"), "got: {out}");
    }

    #[test]
    fn if_elif_else_branches() {
        let src = "\
#if defined(NOT_DEFINED)
struct One {};
#elif defined(MODE)
struct WithMode {};
#else
struct Default {};
#endif
";
        // MODE not defined → default branch.
        let out = run(src);
        assert!(out.contains("struct Default"), "got: {out}");
        assert!(!out.contains("struct One"));
        assert!(!out.contains("struct WithMode"));
    }

    #[test]
    fn if_elif_picks_first_true_branch() {
        let src = "\
#define MODE 1
#if defined(NOT_DEFINED)
struct A {};
#elif defined(MODE)
struct B {};
#elif defined(ANOTHER)
struct C {};
#else
struct D {};
#endif
";
        let out = run(src);
        assert!(out.contains("struct B"), "got: {out}");
        assert!(!out.contains("struct A"));
        assert!(!out.contains("struct C"));
        assert!(!out.contains("struct D"));
    }

    #[test]
    fn warning_directive_does_not_abort() {
        let src = "#warning this is a warning\nstruct OK {};\n";
        let out = run(src);
        assert!(out.contains("struct OK"), "got: {out}");
    }

    #[test]
    fn line_directive_does_not_abort() {
        let src = "#line 42 \"original.idl\"\nstruct X {};\n";
        let out = run(src);
        assert!(out.contains("struct X"), "got: {out}");
    }

    // -----------------------------------------------------------------
    // C1 — OpenSplice legacy pragmas (DCPS_DATA_TYPE/DATA_KEY/cats/
    // genequality). Migration use case for reference customers.
    // -----------------------------------------------------------------

    #[test]
    fn opensplice_pragma_data_type_quoted() {
        let src = r#"#pragma DCPS_DATA_TYPE "Sensor"
struct Sensor { long id; };
"#;
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        assert_eq!(res.opensplice_pragmas.len(), 1);
        match &res.opensplice_pragmas[0] {
            OpenSplicePragma::DataType { type_name, .. } => {
                assert_eq!(type_name, "Sensor");
            }
            other => panic!("expected DataType, got {other:?}"),
        }
    }

    #[test]
    fn opensplice_pragma_data_type_unquoted() {
        let src = "#pragma DCPS_DATA_TYPE Sensor\nstruct Sensor {};\n";
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        match &res.opensplice_pragmas[0] {
            OpenSplicePragma::DataType { type_name, .. } => {
                assert_eq!(type_name, "Sensor");
            }
            other => panic!("expected DataType, got {other:?}"),
        }
    }

    #[test]
    fn opensplice_pragma_data_key() {
        let src = r#"#pragma DCPS_DATA_KEY "Sensor.id"
struct Sensor { long id; };
"#;
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        match &res.opensplice_pragmas[0] {
            OpenSplicePragma::DataKey {
                type_name, fields, ..
            } => {
                assert_eq!(type_name, "Sensor");
                assert_eq!(fields, &["id"]);
            }
            other => panic!("expected DataKey, got {other:?}"),
        }
    }

    #[test]
    fn opensplice_pragma_data_key_space_form() {
        // OpenDDS space form with multiple fields.
        let src = "#pragma DCPS_DATA_KEY \"Sensor id region\"\n\
                   struct Sensor { long id; long region; };\n";
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        match &res.opensplice_pragmas[0] {
            OpenSplicePragma::DataKey {
                type_name, fields, ..
            } => {
                assert_eq!(type_name, "Sensor");
                assert_eq!(fields, &["id", "region"]);
            }
            other => panic!("expected DataKey, got {other:?}"),
        }
    }

    #[test]
    fn opensplice_pragma_cats() {
        let src = "#pragma cats Sensor id sub_id\nstruct Sensor {};\n";
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        match &res.opensplice_pragmas[0] {
            OpenSplicePragma::Cats {
                type_name, keys, ..
            } => {
                assert_eq!(type_name, "Sensor");
                assert_eq!(keys, &vec!["id".to_string(), "sub_id".to_string()]);
            }
            other => panic!("expected Cats, got {other:?}"),
        }
    }

    #[test]
    fn opensplice_pragma_genequality() {
        let src = "#pragma genequality\nstruct S {};\n";
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        assert!(matches!(
            res.opensplice_pragmas.first(),
            Some(OpenSplicePragma::GenEquality { .. })
        ));
    }

    #[test]
    fn opensplice_legacy_full_topic_decl() {
        // Realistic OpenSplice legacy pattern: topic + key via
        // DCPS pragmas plus genequality for a codegen hint.
        let src = r#"#pragma DCPS_DATA_TYPE "Sensor"
#pragma DCPS_DATA_KEY "Sensor.id"
#pragma genequality
struct Sensor {
    long id;
    double value;
};
"#;
        let res = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok");
        assert_eq!(res.opensplice_pragmas.len(), 3);
        assert!(res.expanded.contains("struct Sensor"));
    }

    #[test]
    fn nested_if_in_active_branch() {
        let src = "\
#define OUTER 1
#if defined(OUTER)
#if defined(INNER)
struct ShouldBeGone {};
#else
struct InnerElse {};
#endif
#endif
";
        let out = run(src);
        assert!(out.contains("struct InnerElse"), "got: {out}");
        assert!(!out.contains("ShouldBeGone"), "got: {out}");
    }

    // -----------------------------------------------------------------
    // §7.3 — whitespace before `#` (Phase 1.7)
    // -----------------------------------------------------------------

    #[test]
    fn leading_whitespace_before_hash_accepted() {
        // Spec §7.3: "White space may appear before the #."
        let out = run("    #define X 1\nconst long Y = X;\n");
        assert!(out.contains("const long Y = 1;"), "got: {out}");
    }

    // -----------------------------------------------------------------
    // §7.3 — Backslash-Newline-Continuation (Phase 1.8)
    // -----------------------------------------------------------------

    #[test]
    fn line_continuation_in_define() {
        // Spec §7.3: backslash-newline is removed by splicing.
        let out = run("#define LONG_MACRO foo \\\nbar\nLONG_MACRO\n");
        // The macro body is `foo bar`; after substitution that appears in
        // the output.
        assert!(out.contains("foo bar"), "got: {out}");
    }

    #[test]
    fn line_continuation_in_idl_line() {
        // Backslash-newline removed outside directives too.
        let out = run("const long\\\nX = 1;\n");
        // After splicing: `const longX = 1;`. Not semantically meaningful,
        // but the main point: no two lines anymore — no `\n` between
        // `long` and `X`.
        assert!(!out.contains("long\nX"), "got: {out}");
    }

    #[test]
    fn line_continuation_with_crlf() {
        // Windows style: `\\\r\n` must also be recognized as a continuation.
        let out = run("#define M foo \\\r\nbar\nM\n");
        assert!(out.contains("foo bar"), "got: {out}");
    }

    #[test]
    fn multi_line_continuation() {
        // Three lines joined with continuations.
        let out = run("#define M a \\\nb \\\nc\nM\n");
        assert!(out.contains("a b c"), "got: {out}");
    }

    // -----------------------------------------------------------------
    // §7.3 — Backslash am File-Ende (Phase 1.9)
    // -----------------------------------------------------------------

    #[test]
    fn trailing_backslash_at_file_end_is_error() {
        // Spec §7.3: "A backslash character may not be the last
        // character in a source file."
        let result = Preprocessor::new(MemoryResolver::new()).process("main.idl", "foo\\");
        assert!(
            matches!(result, Err(PreprocessError::TrailingBackslash { .. })),
            "got: {result:?}"
        );
    }

    // -----------------------------------------------------------------
    // §7.2.5 — `#` Stringize + `##` Token-Paste in function-like Macros
    // -----------------------------------------------------------------

    #[test]
    fn function_like_macro_substitutes_args() {
        // Precondition for stringize/token-paste: function-like
        // macros are expanded at all.
        let src = "#define ADD(a, b) a + b\nconst long L = ADD(1, 2);\n";
        let out = run(src);
        assert!(out.contains("1 + 2"), "got: {out}");
    }

    #[test]
    fn stringize_param_in_function_macro() {
        // Spec §7.2.5 + ISO 14882 §16.3.2: `#param` im function-like-
        // The macro body turns the argument into a string literal.
        let src = "#define STR(x) #x\nconst string S = STR(hello);\n";
        let out = run(src);
        assert!(out.contains("\"hello\""), "got: {out}");
    }

    #[test]
    fn stringize_escapes_quotes_and_backslashes() {
        // ISO 14882 §16.3.2: `\` and `"` in the argument are escaped in the
        // resulting string literal.
        let src = "#define STR(x) #x\nconst string S = STR(a\"b\\c);\n";
        let out = run(src);
        assert!(out.contains("\"a\\\"b\\\\c\""), "got: {out}");
    }

    #[test]
    fn token_paste_concatenates_idents() {
        // Spec §7.2.5 + ISO 14882 §16.3.3: `a##b` konkateniert zu `ab`.
        let src = "#define CAT(a, b) a##b\nconst long CAT(foo, bar) = 0;\n";
        let out = run(src);
        assert!(out.contains("foobar"), "got: {out}");
    }

    #[test]
    fn token_paste_with_macro_args_produces_single_ident() {
        // Token-paste must erase whitespace between the operands.
        let src = "#define CAT(a, b) a ## b\nconst long CAT(x, y) = 0;\n";
        let out = run(src);
        assert!(out.contains("xy"), "got: {out}");
    }

    // ---- §7.3.1.1.1 dds_xtopics-Pragma ----

    fn process(src: &str) -> ProcessedSource {
        Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("ok")
    }

    #[test]
    fn pragma_dds_xtopics_version_match() {
        let out = process("#pragma dds_xtopics version=\"1.3\"\nstruct S { long x; };\n");
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "1.3");
    }

    #[test]
    fn pragma_dds_xtopics_version_mismatch_warns() {
        // Two dds_xtopics pragmas with different versions — both are
        // collected; the spec validator (separate pass) detects
        // mismatches.
        let out = process(
            "#pragma dds_xtopics version=\"1.0\"\n\
             #pragma dds_xtopics version=\"1.3\"\n\
             struct S { long x; };\n",
        );
        assert_eq!(out.pragma_dds_xtopics.len(), 2);
        let versions: Vec<&str> = out
            .pragma_dds_xtopics
            .iter()
            .map(|p| p.version.as_str())
            .collect();
        assert!(versions.contains(&"1.0"));
        assert!(versions.contains(&"1.3"));
    }

    #[test]
    fn pragma_dds_xtopics_nested_pragmas_handled() {
        // dds_xtopics + keylist + prefix in the same file — all three
        // pragmas are collected separately, without conflict.
        let out = process(
            "#pragma prefix \"acme.com\"\n\
             #pragma dds_xtopics version=\"1.3\"\n\
             #pragma keylist Topic key_field\n\
             struct Topic { long key_field; };\n",
        );
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "1.3");
        assert_eq!(out.pragma_prefixes.len(), 1);
        assert_eq!(out.pragma_keylists.len(), 1);
    }

    #[test]
    fn pragma_dds_xtopics_without_version_value_is_empty() {
        // `#pragma dds_xtopics` without version= — permitted (marker-only),
        // the version field is empty.
        let out = process("#pragma dds_xtopics\nstruct S { long x; };\n");
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "");
    }

    // ---- §7.3.1.3 Const-Eval: nested #define-Refs ----

    #[test]
    fn nested_define_two_hops() {
        // #define A 100; #define B A; const long x = B;
        // Expected: B → A → 100.
        let out = run("#define A 100\n#define B A\nconst long x = B;\n");
        assert!(out.contains("const long x = 100;"), "{out}");
    }

    #[test]
    fn nested_define_three_hops() {
        let out = run("#define A 7\n\
             #define B A\n\
             #define C B\n\
             const long x = C;\n");
        assert!(out.contains("const long x = 7;"), "{out}");
    }

    #[test]
    fn nested_define_with_arithmetic_expression() {
        let out = run("#define UNIT 8\n\
             #define BUF (UNIT * 4)\n\
             const long x = BUF;\n");
        // BUF becomes (UNIT * 4) → (8 * 4); caller eval does the computation.
        assert!(out.contains("(8 * 4)"), "{out}");
    }

    #[test]
    fn nested_define_self_recursive_terminates() {
        // #define A A — pathological; expand_macros must NOT run in an
        // infinite loop. The output must contain "A" (the self-
        // reference does not resolve).
        let out = run("#define A A\nconst long x = A;\n");
        assert!(out.contains("const long x = A;"), "{out}");
    }

    #[test]
    fn nested_define_mutually_recursive_terminates() {
        // #define A B; #define B A; the expand cap MUST terminate.
        let out = run("#define A B\n#define B A\nconst long x = A;\n");
        // The concrete result is implementation-defined; important: no hang.
        assert!(out.contains("const long x ="));
    }

    #[test]
    fn pragma_dds_xtopics_unquoted_version_accepted() {
        let out = process("#pragma dds_xtopics version=1.3\nstruct S { long x; };\n");
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "1.3");
    }
}
