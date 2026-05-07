// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C-Style Preprocessor fuer OMG IDL 4.2.
//!
//! IDL erbt vom C-Preprocessor — und Vendor-IDL-Files (RTI, OpenSplice)
//! nutzen `#include`, `#define`, `#ifdef`, `#pragma` regelmaessig.
//! Damit der Parser solche Files konsumieren kann, sitzt der
//! Preprocessor VOR dem Lexer und expandiert die Directives zu reinem
//! IDL-Source.
//!
//! # Scope
//!
//! - **`#include "rel/path.idl"`** und **`#include <abs/path.idl>`**:
//!   text-basierte Inklusion via [`Resolver`]-Trait
//! - **`#define MACRO value`** (object-like) und
//!   **`#define NAME(p1, p2) body`** (function-like) inkl.
//!   `#`-Stringize und `##`-Token-Paste (Spec §7.2.5 + ISO 14882
//!   §16.3.2/§16.3.3)
//! - **`#ifdef`** / **`#ifndef`** / **`#if`** / **`#elif`** /
//!   **`#else`** / **`#endif`**: konditionelle Kompilation mit
//!   Expression-Eval (`defined`, `&&`, `||`, `!`, numerische Literale)
//! - **`#pragma <args>`**: stripped (nicht im Output) — Vendor-Pragmas
//!   wie RTI's `#pragma keylist` werden als spezielle AST-Nodes erfasst
//! - **`#undef`**: Macro entfernen
//!
//! Nicht supported:
//! - Recursive Macro-Expansion (eine Pass)
//! - Variadic-Macros (`__VA_ARGS__`)
//! - `#error`, `#warning`, `#line` (geparst, aber nicht funktional)
//! - Volle C-PP-Arithmetik in `#if` (Vergleiche, Bitops, Ternary)
//!
//! # Source-Map
//!
//! [`SourceMap`] mappt jede Position im expandierten Output auf
//! `(file_id, byte_offset_im_original)`. Damit Diagnostiken nach
//! Parsing auf die richtige Original-Datei und -Zeile zeigen.
//!
//! # Beispiel
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

#![allow(missing_docs)] // Field-Level-Doc-Kommentare nicht überall vollständig

mod source_map;

pub use source_map::{FileId, SourceLocation, SourceMap};

use std::collections::HashMap;

/// Trait fuer Include-File-Resolution.
///
/// Erlaubt File-IO (`FsResolver`), In-Memory-Tests (`MemoryResolver`)
/// oder benutzerdefinierte Strategien.
pub trait Resolver {
    /// Loest einen `#include "path"` (relativ) oder `#include <path>`
    /// (system) zu Source-Text auf.
    ///
    /// # Errors
    /// Implementierungs-spezifisch. Sollte einen sprechenden Fehler
    /// liefern, der den gesuchten Pfad enthaelt.
    fn resolve(&self, requesting_file: &str, include: &Include) -> Result<String, ResolveError>;
}

/// Beschreibt einen Include-Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Include {
    /// `#include "name"` — relative/lokale Suche.
    Quoted(String),
    /// `#include <name>` — System-Pfad-Suche.
    System(String),
}

impl Include {
    /// Pfad-Komponente unabhaengig vom Style.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Quoted(p) | Self::System(p) => p,
        }
    }
}

/// Resolver-Fehler. Fehlende Datei, IO-Fehler, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    /// Gesuchter Pfad (so wie er im `#include` stand).
    pub requested: String,
    /// Sprechende Beschreibung.
    pub message: String,
}

/// In-Memory-Resolver fuer Tests und CLI-Tools, die Source ohne
/// Filesystem-Zugriff verwalten.
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

    /// Registriert eine Datei.
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
/// Gesammelt vom Preprocessor; vom Konsumenten (Spec-Validator) gegen
/// `typeprefix`-Decls auf Repository-ID-Konflikt geprueft (IDL 4.2
/// §7.4.6.4.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaPrefix {
    /// Prefix-String (ohne umgebende Anfuehrungszeichen).
    pub prefix: String,
    /// Quelldatei.
    pub file: String,
    /// Zeile (1-basiert).
    pub line: usize,
}

/// `#pragma keylist Foo a b c` — Cyclone/OpenSplice-Konvention.
///
/// Gesammelt vom Preprocessor; vom Konsumenten (idlc, AST-Builder)
/// kann ueber [`ProcessedSource::pragma_keylists`] gelesen werden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaKeylist {
    /// Topic-Type-Name.
    pub type_name: String,
    /// Key-Member-Namen.
    pub keys: Vec<String>,
    /// Quelldatei.
    pub file: String,
    /// Zeile (1-basiert).
    pub line: usize,
}

/// OpenSplice-Legacy-spezifische Pragmas (`#pragma DCPS_DATA_TYPE`,
/// `#pragma DCPS_DATA_KEY`, `#pragma cats`, `#pragma genequality`).
///
/// In OpenSplice-Versionen 5.x/6.x waren diese Pragmas die primaere
/// Methode, IDL-Types als DDS-Topics zu markieren — vor der OMG-IDL-4.2-
/// Annotation `@key`. Migration-Use-Cases muessen sie auf moderne
/// `@key`/`@topic`-Annotations mappen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenSplicePragma {
    /// `#pragma DCPS_DATA_TYPE "<TypeName>"` — markiert Type als
    /// DDS-Topic-Type.
    DataType {
        type_name: String,
        file: String,
        line: usize,
    },
    /// `#pragma DCPS_DATA_KEY "<TypeName>.<field>"` — markiert
    /// Member als Key.
    DataKey {
        type_name: String,
        field: String,
        file: String,
        line: usize,
    },
    /// `#pragma cats <TypeName> <field>` — catenated keys
    /// (alternative key-Markierung).
    Cats {
        type_name: String,
        keys: Vec<String>,
        file: String,
        line: usize,
    },
    /// `#pragma genequality` — codegen-Flag fuer Gleichheits-Operator
    /// in C++/Java-Bindings.
    GenEquality { file: String, line: usize },
}

/// `#pragma dds_xtopics version="1.3"` (XTypes 1.3 §7.3.1.1.1) —
/// erlaubt einer IDL-Datei zu markieren, gegen welche XTypes-Spec-
/// Version sie geschrieben wurde. Der Compiler-Frontend kann dann
/// vendor-Erweiterungen mit/ohne Version-Match validieren.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaDdsXtopics {
    /// Version-String (z.B. `"1.3"`). Leer wenn nicht angegeben.
    pub version: String,
    /// Quelldatei.
    pub file: String,
    /// Zeile.
    pub line: usize,
}

/// Resultat eines Preprocessor-Laufs.
#[derive(Debug, Clone, Default)]
pub struct ProcessedSource {
    /// Expandierter IDL-Source, fertig fuer den Lexer.
    pub expanded: String,
    /// Mapping von Output-Position zu Original-(Datei,Position).
    pub source_map: SourceMap,
    /// Gesammelte `#pragma keylist`-Direktiven.
    pub pragma_keylists: Vec<PragmaKeylist>,
    /// Gesammelte OpenSplice-Legacy-Pragmas (`DCPS_DATA_TYPE`,
    /// `DCPS_DATA_KEY`, `cats`, `genequality`).
    pub opensplice_pragmas: Vec<OpenSplicePragma>,
    /// Gesammelte `#pragma prefix "<prefix>"`-Direktiven (CORBA Part 1
    /// §14.7.5). Vom Spec-Validator fuer §7.4.6.4.1.3 Repository-ID-
    /// Konflikt-Detection genutzt.
    pub pragma_prefixes: Vec<PragmaPrefix>,
    /// Gesammelte `#pragma dds_xtopics version="..."`-Direktiven
    /// (XTypes 1.3 §7.3.1.1.1). Mehrfach-Pragmas pro File sind erlaubt;
    /// der Validator prueft Versions-Konsistenz.
    pub pragma_dds_xtopics: Vec<PragmaDdsXtopics>,
}

/// Top-Level-Preprocessor-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    /// `#include` konnte nicht aufgeloest werden.
    IncludeNotFound(ResolveError),
    /// `#include` schon im Expansion-Stack — Zyklus erkannt.
    IncludeCycle {
        /// Datei, die zyklisch eingebunden werden sollte.
        file: String,
    },
    /// `#endif` ohne passendes `#ifdef`/`#ifndef`.
    UnmatchedEndif {
        /// Quell-Datei der unmatched-Direktive.
        file: String,
        /// 1-indexierte Zeile.
        line: usize,
    },
    /// `#else` ohne passendes `#ifdef`/`#ifndef`.
    UnmatchedElse { file: String, line: usize },
    /// `#ifdef`/`#ifndef` ohne abschliessendes `#endif`.
    UnclosedConditional { file: String, line: usize },
    /// Direktive mit fehlerhafter Syntax (z.B. `#define` ohne Name).
    SyntaxError {
        file: String,
        line: usize,
        message: String,
    },
    /// `#error <message>` — explizit angefragter Build-Stopp.
    ErrorDirective {
        /// Quelldatei.
        file: String,
        /// Zeile.
        line: usize,
        /// Inhalt der Direktive.
        message: String,
    },
    /// Backslash als letztes Zeichen im Source-File — Spec §7.3:
    /// "A backslash character may not be the last character in a source
    /// file."
    TrailingBackslash {
        /// Quelldatei.
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

/// Top-Level-Preprocessor.
///
/// Konstruktion via [`Preprocessor::new`] mit einem [`Resolver`].
/// Anschliessend `process(file_name, source)` aufrufen.
pub struct Preprocessor<R: Resolver> {
    resolver: R,
}

impl<R: Resolver> Preprocessor<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }

    /// Verarbeitet einen Source-String und expandiert alle Direktiven.
    ///
    /// `file_name` wird in Diagnostiken angezeigt und ist die "current
    /// file" fuer relative `#include`-Aufloesung.
    ///
    /// # Errors
    /// Siehe [`PreprocessError`].
    pub fn process(
        &self,
        file_name: &str,
        source: &str,
    ) -> Result<ProcessedSource, PreprocessError> {
        let mut state = State::new();
        let root_id = state.source_map.add_file(file_name);
        let mut output = String::new();
        // Backslash-Newline-Continuation (Spec §7.3, ISO 14882 5.2):
        // Ein `\` direkt vor `\n` wird durch Token-Splice ersetzt — d.h.
        // beide Zeichen werden entfernt, die nachfolgende Zeile setzt
        // syntaktisch fort. Wir tun das vor der Zeilen-Iteration, damit
        // multi-line `#define X foo \\\n bar` korrekt erkannt werden.
        let spliced = splice_backslash_newlines(source);
        // Spec §7.3: "A backslash character may not be the last character
        // in a source file." Wenn nach dem Splicing noch ein Backslash am
        // File-Ende uebrig ist, war er nicht von einem `\n` gefolgt.
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

            // Conditional-Skipping: wenn aktuell in einem inactive
            // Frame, alles ausser #else/#endif/#ifdef/#ifndef ueberspringen.
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
                        // Else aktiviert sich nur wenn parent aktiv UND
                        // bisher KEINE Branch genommen wurde.
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
                        // Erst aktiv wenn parent aktiv UND noch keine
                        // taken Branch UND expr=true.
                        let cond = frame.parent_active
                            && !frame.taken
                            && eval_if_expr(expr, &state.macros);
                        frame.active = cond;
                        if cond {
                            frame.taken = true;
                        }
                    }
                    _ if !active => {
                        // In inaktivem Block — andere Direktiven nicht
                        // ausfuehren.
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
                        // Cycle-Detection vor Resolve, damit Zyklen
                        // unabhaengig vom Resolver erkannt werden.
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
                        // Andere Pragmas: gestrippt.
                    }
                    Directive::Error(msg) => {
                        return Err(PreprocessError::ErrorDirective {
                            file: file_name.to_string(),
                            line: line_no,
                            message: msg.trim().to_string(),
                        });
                    }
                    Directive::Warning(_msg) => {
                        // `#warning` ist Diagnose ohne Abort.
                        // gestripped; UI-Layer kann Warnungen via Hook
                        // rendern (kommt mit Diagnostic-Reporter).
                    }
                    Directive::Line(_args) => {
                        // `#line N "file"` — Position-Override.
                        // SourceMap-Integration steht aus.
                    }
                }
            } else if active {
                // Normale Source-Zeile: Macros expandieren und ans
                // Output anhaengen.
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

/// Definition eines `#define`-Macros — entweder object-like oder
/// function-like (mit Parameter-Liste).
#[derive(Clone, Debug, PartialEq, Eq)]
struct MacroDef {
    /// `Some(params)` fuer function-like Macros (`#define NAME(p1, p2) body`),
    /// `None` fuer object-like (`#define NAME body`).
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

/// Splice backslash-newline pairs (Token-Splicing) gemaess §7.3 / ISO 14882.
fn splice_backslash_newlines(src: &str) -> String {
    // Wir arbeiten Byte-orientiert; UTF-8 kompatibel weil `\` und `\n` ASCII sind.
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
    // Sicher: nur ASCII-Bytes wurden entfernt → bleibt valides UTF-8.
    String::from_utf8(out).unwrap_or_default()
}

struct ConditionalFrame {
    /// `true`, wenn die aktuelle Branch aktiv ist (Tokens werden emittiert).
    active: bool,
    /// `true`, wenn `#else` schon gesehen wurde (zweiter `#else` Fehler).
    else_seen: bool,
    /// War der Parent-Frame aktiv? Wenn nein, ist diese Branch ohnehin
    /// nicht aktiv — wichtig fuer korrektes #else-Toggling in nested.
    parent_active: bool,
    /// `true` sobald irgendeine Branch (`#if`/`#elif`) als aktiv gewaehlt
    /// wurde. Folgende `#elif`/`#else` werden ignoriert. (#if/#elif-Eval)
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
    /// `#if <const-expr>` — vereinfachte Expression-Eval:
    /// `defined(MACRO)`, `0`/`1`, sowie `&&`/`||`/`!`.
    /// (Spec-Stufe-2; gated via `preprocessor_full` Feature.)
    If(&'a str),
    /// `#elif <const-expr>` — Variante von `#if`.
    Elif(&'a str),
    Else,
    Endif,
    Pragma(&'a str),
    /// `#error <message>` — bricht den Build ab.
    Error(&'a str),
    /// `#warning <message>` — Diagnose ohne Abort
    /// (gated via `preprocessor_warning_line` Feature).
    Warning(&'a str),
    /// `#line <linenum> ["filename"]` — Source-Position-Override
    /// (gated via `preprocessor_warning_line` Feature; /// gestripped, SourceMap-Update folgt bei Bedarf).
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

/// Vereinfachte `#if`-Expression-Evaluation (Spec §7.3.2 + ISO 14882
/// constant-expression Subset).
///
/// Unterstuetzt:
/// - Numerische Literale: `0` (false), alles andere (true).
/// - `defined(MACRO)` und `defined MACRO` — true wenn Macro definiert.
/// - Boolean-Operatoren `&&`/`||`/`!` (links-nach-rechts, kein Precedence).
/// - Macro-Identifiers werden als nicht-definiert (false) interpretiert,
///   ausser sie sind explizit als `defined(...)` gewrapped.
///
/// Volle C-Preprocessor-Expression-Eval (Arithmetik, Vergleiche,
/// Bitops, Ternary) ist nicht implementiert.
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
            _ => {} // unbekannte Zeichen ignorieren (defensive)
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
        // `defined(MACRO)` oder `defined MACRO`.
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
    // Numerisches Literal: `0` = false, sonst true.
    if let Ok(n) = tok.parse::<i64>() {
        return (n != 0, idx + 1);
    }
    // Identifier ohne `defined()` — als macro-value-Lookup behandeln;
    // wenn macro-body parsbar als int → entsprechend; sonst true (Macro
    // existiert). Function-like Macros werden hier als true behandelt
    // (Aufruf-Parameter im `#if`-Kontext sind unueblich).
    if let Some(def) = macros.get(tok) {
        if let Ok(n) = def.body.trim().parse::<i64>() {
            return (n != 0, idx + 1);
        }
        return (true, idx + 1);
    }
    // Unbekannter Identifier → false (Spec C-PP-Konvention).
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
    // Identifier-Teil bis zum ersten Whitespace ODER `(`. Ein direkt
    // anschliessendes `(` ohne Whitespace markiert ein function-like
    // Macro (Spec §7.2.5 + ISO 14882 §16.3).
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
/// Liefert `None`, wenn die Pragma kein prefix-Pragma ist oder das
/// String-Argument fehlt/leer ist.
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
/// Liefert `None`, wenn die Pragma keine dds_xtopics-Pragma ist.
fn parse_pragma_dds_xtopics(args: &str, file: &str, line: usize) -> Option<PragmaDdsXtopics> {
    let trimmed = args.trim();
    let rest = trimmed.strip_prefix("dds_xtopics")?.trim_start();
    // Akzeptiert sowohl `version="1.3"` als auch `version=1.3`.
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

/// `#pragma keylist <Type> <field>*` — Cyclone-DDS-Konvention.
///
/// Liefert `None`, wenn die Pragma kein keylist-Pragma ist.
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

/// Parst OpenSplice-Legacy-Pragmas (`DCPS_DATA_TYPE`, `DCPS_DATA_KEY`,
/// `cats`, `genequality`).
fn parse_opensplice_pragma(args: &str, file: &str, line: usize) -> Option<OpenSplicePragma> {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("DCPS_DATA_TYPE") {
        let payload = rest.trim();
        // Akzeptiert sowohl quoted ("Type") als auch unquoted (Type).
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
        let dot = payload.find('.')?;
        let type_name = payload[..dot].trim().to_string();
        let field = payload[dot + 1..].trim().to_string();
        if type_name.is_empty() || field.is_empty() {
            return None;
        }
        return Some(OpenSplicePragma::DataKey {
            type_name,
            field,
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

/// Object-like Macro-Substitution. Iteriert ueber Identifier-Tokens und
/// ersetzt Macro-Namen durch ihre Werte. Vereinfachte Variante — keine
/// Re-Expansion (keine Macro-in-Macro), keine function-like Macros.
fn expand_macros(line: &str, macros: &HashMap<String, MacroDef>) -> String {
    expand_macros_rec(line, macros, 0)
}

/// Maximal-Tiefe fuer rekursive `#define`-Expansion. Schuetzt vor
/// pathologischen `#define A B` / `#define B A`-Cycles und vor
/// indirekten Zyklen ueber 2+-Hops. Wert deckt typische
/// IDL-Const-Pfade (≤ 8 Hops) mit reichlich Reserve ab.
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
                    // function-like: `(` direkt (oder nach Whitespace)
                    // erwartet, sonst Identifier durchreichen.
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
    // Wenn etwas expandiert wurde, kann die Expansion selbst weitere
    // Macro-Refs enthalten — rekursiv nach-expandieren bis Fixed-Point
    // erreicht ist. `MAX_MACRO_EXPANSION_DEPTH` schuetzt vor
    // selbst-rekursiven `#define A A`-Pathologien.
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

/// Parst `( arg1 , arg2 , ... )` ab Position `start` (zeigt auf `(`).
/// Liefert die Argumente und den Index direkt nach `)`. Kommas innerhalb
/// von Klammer-Paaren werden ignoriert (verschachtelte Calls).
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
/// `#param` (Stringize, ISO 14882 §16.3.2) und `a##b` (Token-Paste,
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
    // Token-Paste-Pass: bindet `<a> ## <b>` zu `<a><b>`. Whitespace
    // direkt um `##` wird verworfen. Bei Treffer wird der vorhergehende
    // Eintrag aus `after_paste` gepoppt und mit dem rechten Operanden
    // (param-substituiert) konkateniert.
    let mut after_paste: Vec<BodyTok> = Vec::with_capacity(tokens.len());
    let mut k = 0;
    while k < tokens.len() {
        if matches!(tokens[k], BodyTok::Paste) {
            // Whitespace-Tokens links/rechts vom `##` ueberspringen:
            // Whitespace links liegt schon in `after_paste` — darum wird
            // der letzte Non-Whitespace-Eintrag gesucht.
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
                    // Stand-alone `##` ohne Operanden — als Literal behalten.
                    after_paste.push(BodyTok::Paste);
                    k += 1;
                }
            }
        } else {
            after_paste.push(tokens[k].clone());
            k += 1;
        }
    }
    // Render-Pass: Stringize und Param-Substitution.
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
                // Stand-alone `##` ohne LHS — als Literal ausgeben
                // (sollte nach dem Paste-Pass nicht mehr vorkommen).
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
        // Mind. zwei Segments fuer zwei Zeilen.
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
        // `XY` enthaelt `X` als Substring, darf aber nicht ersetzt werden.
        let out = expand_macros("X XY", &m);
        assert_eq!(out, "100 XY");
    }

    // -----------------------------------------------------------------
    // §7.3 Stufe 2 — #if/#elif/#warning/#line (B7)
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
        // §7.2.5: `&&` mit einem undefined Operand → false.
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
        // §7.2.5: `&&` mit beiden undefined → false.
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
        // MODE nicht definiert → Default-Branch.
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
    // C1 — OpenSplice-Legacy Pragmas (DCPS_DATA_TYPE/DATA_KEY/cats/
    // genequality). Migration-Use-Case fuer Referenz-Kunden.
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
                type_name, field, ..
            } => {
                assert_eq!(type_name, "Sensor");
                assert_eq!(field, "id");
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
        // Realistisches OpenSplice-Legacy-Pattern: Topic + Key via
        // DCPS-Pragmas plus genequality fuer Codegen-Hint.
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
    // §7.3 — Whitespace vor `#` (Phase 1.7)
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
        // Spec §7.3: backslash-newline wird durch Splicing entfernt.
        let out = run("#define LONG_MACRO foo \\\nbar\nLONG_MACRO\n");
        // Macro-Body ist `foo bar`, nach Substitution erscheint das im
        // Output.
        assert!(out.contains("foo bar"), "got: {out}");
    }

    #[test]
    fn line_continuation_in_idl_line() {
        // Backslash-Newline auch außerhalb Direktiven entfernt.
        let out = run("const long\\\nX = 1;\n");
        // Nach Splicing: `const longX = 1;`. Nicht semantisch sinnvoll
        // aber Hauptsache: keine zwei Zeilen mehr — kein `\n` zwischen
        // `long` und `X`.
        assert!(!out.contains("long\nX"), "got: {out}");
    }

    #[test]
    fn line_continuation_with_crlf() {
        // Windows-Style: `\\\r\n` muss ebenfalls als Continuation erkannt
        // werden.
        let out = run("#define M foo \\\r\nbar\nM\n");
        assert!(out.contains("foo bar"), "got: {out}");
    }

    #[test]
    fn multi_line_continuation() {
        // Drei Zeilen mit Continuation verkettet.
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
        // Voraussetzung fuer Stringize/Token-Paste: function-like
        // Macros werden ueberhaupt expandiert.
        let src = "#define ADD(a, b) a + b\nconst long L = ADD(1, 2);\n";
        let out = run(src);
        assert!(out.contains("1 + 2"), "got: {out}");
    }

    #[test]
    fn stringize_param_in_function_macro() {
        // Spec §7.2.5 + ISO 14882 §16.3.2: `#param` im function-like-
        // Macro-Body wandelt das Argument in einen String-Literal um.
        let src = "#define STR(x) #x\nconst string S = STR(hello);\n";
        let out = run(src);
        assert!(out.contains("\"hello\""), "got: {out}");
    }

    #[test]
    fn stringize_escapes_quotes_and_backslashes() {
        // ISO 14882 §16.3.2: `\` und `"` im Argument werden im
        // resultierenden String-Literal escaped.
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
        // Token-Paste muss Whitespace zwischen den Operanden tilgen.
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
        // Zwei dds_xtopics-Pragmas mit verschiedenen Versionen — beide werden
        // gesammelt; der Spec-Validator (separater Pass) erkennt
        // Mismatches.
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
        // dds_xtopics + keylist + prefix in derselben Datei — alle drei
        // Pragmas werden separat gesammelt, ohne Konflikt.
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
        // `#pragma dds_xtopics` ohne version= — zulaessig (Marker-only),
        // version-Feld ist leer.
        let out = process("#pragma dds_xtopics\nstruct S { long x; };\n");
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "");
    }

    // ---- §7.3.1.3 Const-Eval: nested #define-Refs ----

    #[test]
    fn nested_define_two_hops() {
        // #define A 100; #define B A; const long x = B;
        // Erwartet: B → A → 100.
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
        // BUF wird zu (UNIT * 4) → (8 * 4); Caller-Eval macht das Ausrechnen.
        assert!(out.contains("(8 * 4)"), "{out}");
    }

    #[test]
    fn nested_define_self_recursive_terminates() {
        // #define A A — pathologisch; expand_macros darf NICHT in
        // Endlosschleife laufen. Output muss "A" enthalten (Selbst-
        // Reference loest sich nicht auf).
        let out = run("#define A A\nconst long x = A;\n");
        assert!(out.contains("const long x = A;"), "{out}");
    }

    #[test]
    fn nested_define_mutually_recursive_terminates() {
        // #define A B; #define B A; expand-Cap MUSS terminieren.
        let out = run("#define A B\n#define B A\nconst long x = A;\n");
        // Konkretes Resultat ist implementation-defined; wichtig: kein Hang.
        assert!(out.contains("const long x ="));
    }

    #[test]
    fn pragma_dds_xtopics_unquoted_version_accepted() {
        let out = process("#pragma dds_xtopics version=1.3\nstruct S { long x; };\n");
        assert_eq!(out.pragma_dds_xtopics.len(), 1);
        assert_eq!(out.pragma_dds_xtopics[0].version, "1.3");
    }
}
