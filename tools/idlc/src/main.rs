// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 compiler: backends for C, C++, C#, Java, Python, Rust (T7.1).
//!
//! Phase 1: alle sieben Backends verdrahtet — `--c`, `--cpp`, `--rust`,
//! `--ts`, `--csharp`, `--java`, `--python`.
//!
//! Preprocessor: `#include`/`#define`/`#ifdef`/`#pragma` werden vor
//! dem Parsen expandiert (`zerodds-idl::preprocessor`). `-I <dir>`
//! fuegt Include-Such-Pfade hinzu, `-D NAME[=VALUE]` definiert Macros.
//!
//! `--corba` ist ein Modifier-Flag (analog `--rti`) und emittiert
//! zusaetzlich CORBA-Service-Konstrukte (Interface-Traits, Client-Stubs,
//! Server-Skeletons). Unterstuetzt fuer `--cpp`, `--csharp`, `--java`
//! ueber OMG CORBA 3.3 Annex-A.1 (die Codegen-Crates exponieren
//! `emit_corba_traits`) und fuer `--rust` ueber die Vendor-Spec
//! `zerodds-corba-rust-1.0` (Two-File-Output `<base>.rs` + `<base>_corba.rs`).
//! Mit anderen Backends → Exit-Code 3.
//!
//! # Usage
//!
//! ```text
//! zerodds-idlc --parse-only <file.idl>             Parse mit OMG IDL 4.2 Base
//! zerodds-idlc --parse-only --rti <file.idl>       RTI Connext-Grammar-Delta
//! zerodds-idlc --c      -o <dir> <file.idl>        C-Header in <dir>/<base>.h
//! zerodds-idlc --cpp    -o <dir> <file.idl>        C++-Header in <dir>/<base>.hpp
//! zerodds-idlc --rust   -o <dir> <file.idl>        Rust-Code in <dir>/<base>.rs
//! zerodds-idlc --ts     -o <dir> <file.idl>        TS-Modul in <dir>/<base>.ts
//! zerodds-idlc --csharp -o <dir> <file.idl>        C#-Code in <dir>/<base>.cs
//! zerodds-idlc --java   -o <dir> <file.idl>        Java-Files in <dir>/<pkg>/
//! zerodds-idlc --python -o <dir> <file.idl>        Python-Modul in <dir>/<base>.py
//! zerodds-idlc --cpp    --corba -o <dir> <file.idl>  C++-Header inkl. CORBA-Traits
//! zerodds-idlc --csharp --corba -o <dir> <file.idl>  C#-Code inkl. CORBA-Traits
//! zerodds-idlc --java   --corba -o <dir> <file.idl>  Java-Files inkl. CorbaTraits-Klassen
//! zerodds-idlc --rust   --corba -o <dir> <file.idl>  Rust-Types + <base>_corba.rs Service-Code
//! zerodds-idlc --version                           Versions-Info
//! zerodds-idlc --help                              Hilfe
//! ```
//!
//! Exit-Codes:
//!   0   Erfolg
//!   1   parse-Fehler (Lex/Recognize/Build)
//!   2   CLI-Argumente ungueltig oder Datei nicht lesbar
//!   3   Backend (noch) nicht implementiert oder Codegen-Fehler

#![allow(clippy::print_stderr, clippy::print_stdout)] // CLI-Tool: I/O auf stdio zulaessig.

mod default_ext;
mod key_pragmas;
mod scaffold;
mod typeobject_emit;

use default_ext::DefaultExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use typeobject_emit::TypeObjectBlob;

use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::{parse, parse_with_deltas};
use zerodds_idl::preprocessor::{Include, Preprocessor, ResolveError, Resolver};
use zerodds_idl_cpp::{
    CGenOptions, CppGenOptions, generate_c_header, generate_cpp_header,
    generate_cpp_header_with_corba_traits,
};
use zerodds_idl_csharp::{CsGenOptions, generate_csharp, generate_csharp_with_corba_traits};
use zerodds_idl_java::{
    JavaGenOptions, generate_java_files, generate_java_files_with_corba_traits,
};
use zerodds_idl_python::{PythonGenOptions, generate_python_module};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};
use zerodds_idl_ts::generate_ts_source;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Color-Modus fuer die Fehlerausgabe vorab aus den Args ableiten —
    // run() selbst kennt erst nach dem Parsen den Modus.
    let color = color_mode_from_args(&args);
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if color.enabled() {
                // ANSI: bold red prefix.
                eprintln!("\x1b[1;31mzerodds-idlc:\x1b[0m {e}");
            } else {
                eprintln!("zerodds-idlc: {e}");
            }
            ExitCode::from(e.exit_code())
        }
    }
}

/// Liest `--color <when>` aus den Roh-Args (vor dem vollen Parse).
fn color_mode_from_args(args: &[String]) -> ColorMode {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--color" {
            return match it.next().map(String::as_str) {
                Some("always") => ColorMode::Always,
                Some("never") => ColorMode::Never,
                _ => ColorMode::Auto,
            };
        }
    }
    ColorMode::Auto
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String),
    Parse(String),
    NotImplemented(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::Io(_) => 2,
            Self::Parse(_) => 1,
            Self::NotImplemented(_) => 3,
        }
    }
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Usage(m) | Self::Io(m) | Self::Parse(m) | Self::NotImplemented(m) => {
                f.write_str(m)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    C,
    Cpp,
    Rust,
    Ts,
    CSharp,
    Java,
    Python,
}

/// Sub-Command (Handbook §3.3–3.7). Die flache Flag-Form bleibt
/// vollwertig erhalten — ein Sub-Command ist nur Zucker, der den
/// passenden Modus vorbelegt. `generate` ist der Default-Modus
/// (Codegen), die uebrigen mappen auf einen Dump-/Parse-Modus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubCommand {
    Generate,
    Check,
    DumpAst,
    DumpTypeObject,
    PrintDeps,
}

impl SubCommand {
    fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "generate" => Self::Generate,
            "check" => Self::Check,
            "dump-ast" => Self::DumpAst,
            "dump-typeobject" => Self::DumpTypeObject,
            "print-deps" => Self::PrintDeps,
            _ => return None,
        })
    }
}

/// Trennt einen fuehrenden Sub-Command von den restlichen Args.
///
/// Der Sub-Command — falls vorhanden — ist das erste Positional-
/// Argument; globale Flags duerfen davor stehen (`-v generate …`).
/// Argument-konsumierende Flags (`-o`, `-I`, `-D`, `--color`,
/// `--output`) werden beruecksichtigt, damit etwa `-o check` die
/// Ausgabe in den Ordner `check` schreibt statt als Sub-Command
/// `check` fehlinterpretiert zu werden.
fn split_subcommand(args: &[String]) -> (Option<SubCommand>, Vec<String>) {
    let consumes_arg = |f: &str| matches!(f, "-o" | "--output" | "-I" | "-D" | "--color");
    let mut skip_next = false;
    for (i, a) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if consumes_arg(a) {
            skip_next = true;
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        // Erstes Positional: Sub-Command-Keyword oder Input-Datei.
        return match SubCommand::parse(a) {
            Some(sc) => {
                let mut rest = args.to_vec();
                rest.remove(i);
                (Some(sc), rest)
            }
            None => (None, args.to_vec()),
        };
    }
    (None, args.to_vec())
}

/// Wann ANSI-Farben in Diagnostiken verwendet werden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum ColorMode {
    /// Farben nur wenn stderr ein TTY ist.
    #[default]
    Auto,
    /// Immer Farben.
    Always,
    /// Nie Farben.
    Never,
}

impl ColorMode {
    /// Effektiv aktiv? `Auto` prueft ob stderr ein Terminal ist.
    fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => std::io::IsTerminal::is_terminal(&std::io::stderr()),
        }
    }
}

#[derive(Default)]
struct CliOptions {
    /// Aktiver Sub-Command, falls als erstes Positional angegeben.
    subcommand: Option<SubCommand>,
    parse_only: bool,
    /// `--dump-deps` — Make-Dependency-Zeile statt Codegen.
    dump_deps: bool,
    /// `--dump-typeobject` — XTypes-TypeObject je Struct statt Codegen.
    dump_typeobject: bool,
    rti: bool,
    corba: bool,
    /// Gewaehlte Backends. Mehrere via wiederholten Flags oder `--all`.
    backends: Vec<Backend>,
    output: Option<PathBuf>,
    /// Per-Backend-Output-Override aus `--out-<lang> <dir>`. Schlaegt
    /// `-o` fuer das jeweilige Backend. Mehrfach (je Backend) moeglich.
    out_overrides: Vec<(Backend, PathBuf)>,
    file: Option<String>,
    /// Include-Search-Pfade aus `-I <path>` (in Reihenfolge).
    include_dirs: Vec<PathBuf>,
    /// Preprocessor-Defines aus `-D <name>[=<value>]`.
    defines: Vec<String>,
    /// Verbosity-Level (Anzahl `-v`). 0 = normal.
    verbose: u8,
    /// `-q` — nur Fehler, kein Status-Output.
    quiet: bool,
    /// `--color` — ANSI-Farb-Modus fuer Diagnostiken.
    color: ColorMode,
    /// `--default-extensibility` — Extensibility fuer un-annotierte
    /// struct/union/enum. `None` = OMG-Default belassen (Emitter-Fallback).
    default_ext: Option<DefaultExt>,
    /// `--default-nested true` — un-annotierte Typen als `@nested`
    /// markieren. `None`/`Some(false)` = OMG-Default belassen.
    default_nested: Option<bool>,
    /// `--no-typeobject` — XTypes-TypeObject-Emission unterdruecken.
    /// Default `false` (Emission an, wie RTI/Fast/Cyclone).
    no_type_objects: bool,
    /// `--scaffold` — pro Backend die idiomatische Build-Datei
    /// (Cargo.toml / CMakeLists.txt / …) mit-emittieren.
    scaffold: bool,
}

impl CliOptions {
    /// Status-Zeile auf stderr — unterdrueckt bei `-q`, nur ausgegeben
    /// wenn das aktuelle `verbose`-Level >= `min_level`.
    fn report(&self, min_level: u8, msg: &str) {
        if self.quiet || self.verbose < min_level {
            return;
        }
        eprintln!("zerodds-idlc: {msg}");
    }

    /// Effektives Output-Verzeichnis fuer ein Backend: der per-Backend-
    /// Override aus `--out-<lang>`, sonst das globale `-o`.
    fn backend_out_dir(&self, backend: Backend) -> Option<&Path> {
        self.out_overrides
            .iter()
            .find(|(b, _)| *b == backend)
            .map(|(_, p)| p.as_path())
            .or(self.output.as_deref())
    }
}

/// Flag-Suffix eines Backends fuer `--out-<lang>` und Diagnostiken
/// (`Backend::Rust` → `"rust"`).
fn backend_flag_suffix(b: Backend) -> &'static str {
    match b {
        Backend::C => "c",
        Backend::Cpp => "cpp",
        Backend::Rust => "rust",
        Backend::Ts => "ts",
        Backend::CSharp => "csharp",
        Backend::Java => "java",
        Backend::Python => "python",
    }
}

/// Backend zu einem `--out-<lang>`-Flag, `None` bei unbekanntem Suffix.
fn out_flag_backend(flag: &str) -> Option<Backend> {
    let suffix = flag.strip_prefix("--out-")?;
    ALL_BACKENDS
        .into_iter()
        .find(|b| backend_flag_suffix(*b) == suffix)
}

/// Alle sieben Backends — fuer `--all`.
const ALL_BACKENDS: [Backend; 7] = [
    Backend::C,
    Backend::Cpp,
    Backend::Rust,
    Backend::Ts,
    Backend::CSharp,
    Backend::Java,
    Backend::Python,
];

/// Filesystem-`Resolver` fuer den IDL-Preprocessor.
///
/// `#include "x"` → erst relativ zur einbindenden Datei, dann die
/// `-I`-Pfade in Reihenfolge. `#include <x>` → nur die `-I`-Pfade.
struct FsResolver {
    include_dirs: Vec<PathBuf>,
}

impl Resolver for FsResolver {
    fn resolve(&self, requesting_file: &str, include: &Include) -> Result<String, ResolveError> {
        let name = include.path();
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Quoted-Includes: zuerst relativ zur einbindenden Datei.
        if matches!(include, Include::Quoted(_)) {
            if let Some(parent) = Path::new(requesting_file).parent() {
                candidates.push(parent.join(name));
            }
        }
        for dir in &self.include_dirs {
            candidates.push(dir.join(name));
        }
        for cand in &candidates {
            if let Ok(text) = std::fs::read_to_string(cand) {
                return Ok(text);
            }
        }
        Err(ResolveError {
            requested: name.to_string(),
            message: format!(
                "include '{name}' not found (searched {} path(s); use -I)",
                candidates.len()
            ),
        })
    }
}

/// Fuegt ein Backend hinzu, falls noch nicht in der Liste (idempotent).
fn push_backend(backends: &mut Vec<Backend>, b: Backend) {
    if !backends.contains(&b) {
        backends.push(b);
    }
}

fn run(args: &[String]) -> Result<(), CliError> {
    let (subcommand, args) = split_subcommand(args);
    let mut opts = CliOptions {
        subcommand,
        ..CliOptions::default()
    };
    // Sub-Command belegt den Modus vor; die restlichen Args parsen
    // anschliessend ganz normal (ein Sub-Command schliesst keine Flags
    // aus, er waehlt nur den Default-Modus).
    match subcommand {
        Some(SubCommand::Generate) | None => {}
        Some(SubCommand::Check | SubCommand::DumpAst) => opts.parse_only = true,
        Some(SubCommand::DumpTypeObject) => opts.dump_typeobject = true,
        Some(SubCommand::PrintDeps) => opts.dump_deps = true,
    }
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("zerodds-idlc {VERSION}");
                return Ok(());
            }
            "--parse-only" => opts.parse_only = true,
            "--dump-deps" => opts.dump_deps = true,
            "--dump-typeobject" => opts.dump_typeobject = true,
            "--dump-ast" => opts.parse_only = true,
            "--rti" => opts.rti = true,
            "--corba" => opts.corba = true,
            // OpenDDS / Cyclone nutzen Standard-OMG-IDL-4.2; ihre
            // Vendor-Eigenheiten sind #pragma-basiert (keylist,
            // DCPS_DATA_KEY) und werden ohnehin immer verarbeitet.
            // Die Flags sind als explizite Intent-Marker akzeptiert.
            "--opendds" | "--cyclone" => {}
            "-v" | "--verbose" => opts.verbose = opts.verbose.saturating_add(1),
            // Aggregierte Kurzform `-vv` / `-vvv`.
            other
                if other.len() > 1
                    && other.starts_with('-')
                    && other[1..].chars().all(|c| c == 'v') =>
            {
                let n = u8::try_from(other.len() - 1).unwrap_or(u8::MAX);
                opts.verbose = opts.verbose.saturating_add(n);
            }
            "-q" | "--quiet" => opts.quiet = true,
            "--color" => {
                let value = iter.next().ok_or_else(|| {
                    CliError::Usage("--color requires auto|always|never".to_string())
                })?;
                opts.color = match value.as_str() {
                    "auto" => ColorMode::Auto,
                    "always" => ColorMode::Always,
                    "never" => ColorMode::Never,
                    other => {
                        return Err(CliError::Usage(format!(
                            "--color: expected auto|always|never, got '{other}'"
                        )));
                    }
                };
            }
            // Default-Extensibility (Fast DDS -de / Cyclone -x).
            "--default-extensibility" | "-de" => {
                let value = iter.next().ok_or_else(|| {
                    CliError::Usage(
                        "--default-extensibility requires final|appendable|mutable".to_string(),
                    )
                })?;
                opts.default_ext = Some(DefaultExt::parse(value).ok_or_else(|| {
                    CliError::Usage(format!(
                        "--default-extensibility: expected final|appendable|mutable, got '{value}'"
                    ))
                })?);
            }
            // TypeObject-Emission (default an — wie RTI/Fast/Cyclone).
            "--no-typeobject" => opts.no_type_objects = true,
            "--with-typeobject" => opts.no_type_objects = false,
            // Projekt-Scaffolding (opt-in, die „extra Meile").
            "--scaffold" => opts.scaffold = true,
            // Default-Nested (Cyclone -n).
            "--default-nested" => {
                let value = iter.next().ok_or_else(|| {
                    CliError::Usage("--default-nested requires true|false".to_string())
                })?;
                opts.default_nested = Some(match value.as_str() {
                    "true" => true,
                    "false" => false,
                    other => {
                        return Err(CliError::Usage(format!(
                            "--default-nested: expected true|false, got '{other}'"
                        )));
                    }
                });
            }
            // Backend-Flags — mehrfach kombinierbar. Duplikate werden
            // still ignoriert (idempotent).
            "--c" => push_backend(&mut opts.backends, Backend::C),
            "--rust" => push_backend(&mut opts.backends, Backend::Rust),
            "--cpp" => push_backend(&mut opts.backends, Backend::Cpp),
            "--ts" => push_backend(&mut opts.backends, Backend::Ts),
            "--csharp" => push_backend(&mut opts.backends, Backend::CSharp),
            "--java" => push_backend(&mut opts.backends, Backend::Java),
            "--python" => push_backend(&mut opts.backends, Backend::Python),
            "--all" => {
                for b in ALL_BACKENDS {
                    push_backend(&mut opts.backends, b);
                }
            }
            "-o" | "--output" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("{arg} requires a directory")))?;
                opts.output = Some(PathBuf::from(value));
            }
            // Per-Backend-Output: --out-rust / --out-c / --out-cpp /
            // --out-csharp / --out-java / --out-python / --out-ts.
            other if other.starts_with("--out-") => {
                let backend = out_flag_backend(other)
                    .ok_or_else(|| CliError::Usage(format!("unknown flag: {other}")))?;
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("{other} requires a directory")))?;
                // Letzter Override pro Backend gewinnt.
                opts.out_overrides.retain(|(b, _)| *b != backend);
                opts.out_overrides.push((backend, PathBuf::from(value)));
            }
            "-I" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage("-I requires a directory".to_string()))?;
                opts.include_dirs.push(PathBuf::from(value));
            }
            "-D" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage("-D requires NAME[=VALUE]".to_string()))?;
                opts.defines.push(value.clone());
            }
            // -I<path> / -D<name> in zusammengeschriebener Form (C-Konvention).
            other if other.starts_with("-I") && other.len() > 2 => {
                opts.include_dirs.push(PathBuf::from(&other[2..]));
            }
            other if other.starts_with("-D") && other.len() > 2 => {
                opts.defines.push(other[2..].to_string());
            }
            other if other.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown flag: {other}")));
            }
            other => {
                if opts.file.is_some() {
                    return Err(CliError::Usage("only one input file supported".to_string()));
                }
                opts.file = Some(other.to_string());
            }
        }
    }

    // --corba braucht eine konkrete Antwort BEVOR die generischen
    // backend/parse-only-Checks greifen, sonst bekommt der User die
    // weniger praezise "no action selected"-Meldung.
    if opts.corba {
        if opts.backends.is_empty() {
            return Err(CliError::Usage(
                "--corba requires a backend (--cpp, --csharp, --java, or --rust)".to_string(),
            ));
        }
        // CORBA-Code wird nur fuer cpp/csharp/java/rust emittiert.
        // Andere Backends im Set werden ohne CORBA-Code generiert —
        // ein hartes Reject waere bei --all unbrauchbar.
        let corba_capable = opts.backends.iter().any(|b| {
            matches!(
                b,
                Backend::Cpp | Backend::CSharp | Backend::Java | Backend::Rust
            )
        });
        if !corba_capable {
            return Err(CliError::NotImplemented(
                "--corba is only supported by --cpp, --csharp, --java, --rust".to_string(),
            ));
        }
    }
    let dump_action = opts.parse_only || opts.dump_deps || opts.dump_typeobject;
    if !dump_action && opts.backends.is_empty() {
        // Bei `generate` ohne Backend eine praezisere Meldung als beim
        // flachen Aufruf — der Sub-Command sagt klar, was gewollt war.
        let msg = if opts.subcommand == Some(SubCommand::Generate) {
            "generate requires a backend flag (--rust, --cpp, --c, --csharp, \
             --java, --python, --ts, or --all)"
        } else {
            "no action selected; use a sub-command (generate, check, dump-ast, \
             dump-typeobject, print-deps) or a backend flag (--rust, --all, …)"
        };
        return Err(CliError::NotImplemented(msg.to_string()));
    }
    if dump_action && !opts.backends.is_empty() {
        return Err(CliError::Usage(
            "the selected mode (check / dump-ast / dump-typeobject / print-deps / \
             --parse-only) does not emit code — drop the backend flag"
                .to_string(),
        ));
    }
    let path = opts
        .file
        .as_ref()
        .ok_or_else(|| CliError::Usage("missing input file (try --help)".to_string()))?;
    let raw = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("cannot read {path}: {e}")))?;

    // Preprocessor-Lauf: `-D`-Defines werden als synthetische
    // `#define`-Prelude vorangestellt (gcc-Konvention: `-D NAME` ohne
    // Wert => `1`). Anschliessend expandiert der Preprocessor alle
    // `#include`/`#define`/`#ifdef`/`#pragma`-Direktiven.
    let mut prelude = String::new();
    for def in &opts.defines {
        match def.split_once('=') {
            Some((name, value)) => prelude.push_str(&format!("#define {name} {value}\n")),
            None => prelude.push_str(&format!("#define {def} 1\n")),
        }
    }
    let pp_input = format!("{prelude}{raw}");
    let resolver = FsResolver {
        include_dirs: opts.include_dirs.clone(),
    };
    let processed = Preprocessor::new(resolver)
        .process(path, &pp_input)
        .map_err(|e| CliError::Parse(format!("preprocessing failed: {e:?}")))?;
    // --dump-deps: Make-Dependency-Zeile aus den vom Preprocessor
    // registrierten Quelldateien (Haupt-IDL + alle #include-Ziele).
    // Format: `<stem>.o: <file1> <file2> …` — direkt fuer Makefiles.
    if opts.dump_deps {
        let map = &processed.source_map;
        let deps: Vec<&str> = (0..map.file_count())
            .filter_map(|i| {
                map.file_name(zerodds_idl::preprocessor::FileId(
                    u32::try_from(i).unwrap_or(0),
                ))
            })
            .collect();
        let stem = basename_stem(path).unwrap_or_else(|| "output".to_string());
        println!("{stem}.o: {}", deps.join(" "));
        return Ok(());
    }

    let src = &processed.expanded;

    let cfg = ParserConfig::default();
    let mut ast = if opts.rti {
        parse_with_deltas(src, &cfg, &[&RTI_CONNEXT])
    } else {
        parse(src, &cfg)
    }
    .map_err(|e| CliError::Parse(format!("parse failed: {e}")))?;

    opts.report(1, &format!("parsed {path}"));

    // Vendor-Pragmas (#pragma keylist / DCPS_DATA_KEY / cats) auf den
    // AST anwenden — markiert die genannten Member als @key. Laeuft
    // immer (Cyclone/OpenDDS-IDL ist so verbreitet, dass die Pragmas
    // grundsaetzlich respektiert werden — die Migration-Guides
    // dokumentieren das als garantiertes Verhalten).
    let key_map =
        key_pragmas::collect_key_pragmas(&processed.pragma_keylists, &processed.opensplice_pragmas);
    let patched = key_pragmas::apply_key_pragmas(&mut ast, &key_map);
    if patched > 0 {
        opts.report(
            1,
            &format!("applied vendor key-pragmas: {patched} member(s) marked @key"),
        );
    }

    // Default-Extensibility / Default-Nested: un-annotierte Typen vor
    // dem Codegen explizit annotieren, damit alle Backends denselben
    // Default sehen.
    if let Some(ext) = opts.default_ext {
        let n = default_ext::apply_default_extensibility(&mut ast, ext);
        if n > 0 {
            opts.report(1, &format!("default extensibility applied to {n} type(s)"));
        }
    }
    if let Some(nested) = opts.default_nested {
        let n = default_ext::apply_default_nested(&mut ast, nested);
        if n > 0 {
            opts.report(1, &format!("default @nested applied to {n} type(s)"));
        }
    }

    if opts.parse_only {
        println!("{ast}");
        return Ok(());
    }

    if opts.dump_typeobject {
        dump_typeobjects(&ast);
        return Ok(());
    }

    let base = basename_stem(path)
        .ok_or_else(|| CliError::Usage(format!("cannot derive basename from: {path}")))?;

    // XTypes-TypeObjects einmal lowern + serialisieren (default-on).
    // Schlaegt das Mapping fehl (z.B. rekursiver Typ), wird gewarnt und
    // ohne TypeObject-Block weitergemacht — der Kern-Codegen (Typen +
    // CDR) ist davon unberuehrt.
    let type_objects: Vec<TypeObjectBlob> = if opts.no_type_objects {
        Vec::new()
    } else {
        match typeobject_emit::type_object_blobs(&ast) {
            Ok(blobs) => {
                opts.report(1, &format!("lowered {} TypeObject(s)", blobs.len()));
                blobs
            }
            Err(e) => {
                opts.report(0, &format!("warning: TypeObject emission skipped — {e}"));
                Vec::new()
            }
        }
    };

    // Jedes gewaehlte Backend in sein Ziel-Verzeichnis emittieren. Bei
    // --all sind das alle sieben; Reihenfolge = ALL_BACKENDS. Ziel ist
    // der `--out-<lang>`-Override, sonst das globale `-o`.
    for backend in &opts.backends {
        let suffix = backend_flag_suffix(*backend);
        let out_dir = opts.backend_out_dir(*backend).ok_or_else(|| {
            CliError::Usage(format!(
                "{} backend: no output directory — pass -o <dir> or --out-{suffix} <dir>",
                backend_name(*backend)
            ))
        })?;
        std::fs::create_dir_all(out_dir)
            .map_err(|e| CliError::Io(format!("cannot create {}: {e}", out_dir.display())))?;
        emit_backend(*backend, &ast, &opts, out_dir, &base, &type_objects)?;
        opts.report(
            1,
            &format!(
                "emitted {} backend → {}",
                backend_name(*backend),
                out_dir.display()
            ),
        );
    }
    Ok(())
}

/// `--dump-typeobject`: alle benannten Typen einer Spec ueber den
/// zweistufigen Registry-Mapper lowern und in topologischer Reihenfolge
/// (Abhaengigkeiten zuerst) ausgeben. Scoped Cross-Referenzen werden
/// aufgeloest. Schlaegt das Mapping fehl (z.B. zyklischer Typ), wird die
/// Mapper-Fehlermeldung ausgegeben.
fn dump_typeobjects(spec: &zerodds_idl::ast::Specification) {
    match zerodds_idl::semantics::build_type_registry(spec) {
        Ok(lowered) => {
            for (fqn, obj) in lowered.iter() {
                println!("// TypeObject for {fqn}\n{obj:#?}\n");
            }
        }
        Err(e) => println!("// TypeObject mapping failed: {e:?}"),
    }
}

/// Lesbarer Backend-Name fuer Status-Ausgaben.
fn backend_name(b: Backend) -> &'static str {
    match b {
        Backend::C => "C",
        Backend::Cpp => "C++",
        Backend::Rust => "Rust",
        Backend::Ts => "TypeScript",
        Backend::CSharp => "C#",
        Backend::Java => "Java",
        Backend::Python => "Python",
    }
}

/// Emittiert ein einzelnes Backend in `out_dir`. `type_objects` ist der
/// XTypes-TypeObject-Block, der an die generierte Ausgabe angehaengt
/// wird (leer = `--no-typeobject` oder Mapping fehlgeschlagen).
fn emit_backend(
    backend: Backend,
    ast: &zerodds_idl::ast::Specification,
    opts: &CliOptions,
    out_dir: &Path,
    base: &str,
    type_objects: &[TypeObjectBlob],
) -> Result<(), CliError> {
    match backend {
        Backend::C => {
            let mut code = generate_c_header(ast, &CGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("c codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.h"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Rust => {
            // DataTypes immer.
            let mut code = generate_rust_module(ast, &RustGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("rust codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.rs"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
            // Mit --corba zusaetzlich Service-Code in zweite Datei. Anders
            // als bei C++/C#/Java (Single-File-Header) kollidieren die
            // File-level Inner-Attributes von idl-rust und corba-rust beim
            // Concat — deshalb Two-File-Output. User bindet via
            // `mod <base>; mod <base>_corba;`.
            if opts.corba {
                let svc = generate_corba_rust_module(ast, &CorbaRustGenOptions::default())
                    .map_err(|e| {
                        CliError::NotImplemented(format!("corba-rust codegen failed: {e}"))
                    })?;
                let svc_path = out_dir.join(format!("{base}_corba.rs"));
                std::fs::write(&svc_path, svc).map_err(|e| {
                    CliError::Io(format!("cannot write {}: {e}", svc_path.display()))
                })?;
            }
        }
        Backend::Cpp => {
            let cpp_opts = CppGenOptions::default();
            let mut code = if opts.corba {
                generate_cpp_header_with_corba_traits(ast, &cpp_opts)
            } else {
                generate_cpp_header(ast, &cpp_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("cpp codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.hpp"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Ts => {
            let mut code = generate_ts_source(ast)
                .map_err(|e| CliError::NotImplemented(format!("ts codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.ts"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::CSharp => {
            let cs_opts = CsGenOptions::default();
            let mut code = if opts.corba {
                generate_csharp_with_corba_traits(ast, &cs_opts)
            } else {
                generate_csharp(ast, &cs_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("csharp codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.cs"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Java => {
            let java_opts = JavaGenOptions::default();
            let files = if opts.corba {
                generate_java_files_with_corba_traits(ast, &java_opts)
            } else {
                generate_java_files(ast, &java_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("java codegen failed: {e}")))?;
            for file in files {
                let pkg_subpath = file.package_path.replace('.', "/");
                let pkg_dir = if pkg_subpath.is_empty() {
                    out_dir.to_path_buf()
                } else {
                    out_dir.join(&pkg_subpath)
                };
                std::fs::create_dir_all(&pkg_dir).map_err(|e| {
                    CliError::Io(format!("cannot create {}: {e}", pkg_dir.display()))
                })?;
                let class_path = pkg_dir.join(format!("{}.java", file.class_name));
                std::fs::write(&class_path, &file.source).map_err(|e| {
                    CliError::Io(format!("cannot write {}: {e}", class_path.display()))
                })?;
            }
            // Java ist Multi-File — der TypeObject-Block kommt als
            // eigenstaendige Compilation-Unit `TypeObjects.java`.
            if !type_objects.is_empty() {
                let to_path = out_dir.join("TypeObjects.java");
                std::fs::write(&to_path, typeobject_emit::render(backend, type_objects)).map_err(
                    |e| CliError::Io(format!("cannot write {}: {e}", to_path.display())),
                )?;
            }
        }
        Backend::Python => {
            let mut code = generate_python_module(ast, &PythonGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("python codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, type_objects));
            let out_path = out_dir.join(format!("{base}.py"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
    }

    // --scaffold: idiomatische Build-Datei je Backend. Bereits
    // vorhandene Build-Dateien werden nicht ueberschrieben (der User
    // koennte sie angepasst haben).
    if opts.scaffold {
        for (name, content) in scaffold::scaffold_files(backend, base) {
            let path = out_dir.join(&name);
            if path.exists() {
                opts.report(1, &format!("scaffold: {name} exists — kept"));
            } else {
                std::fs::write(&path, content)
                    .map_err(|e| CliError::Io(format!("cannot write {}: {e}", path.display())))?;
                opts.report(1, &format!("scaffold: wrote {name}"));
            }
        }
    }
    Ok(())
}

/// Filename-Stem ohne Directory + ohne letzte Extension.
/// `foo/bar/chat.idl` → `Some("chat")`.
fn basename_stem(path: &str) -> Option<String> {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
}

fn print_help() {
    println!(
        "zerodds-idlc {VERSION}\n\
         IDL4 compiler (Phase 1: alle sieben Backends)\n\n\
         USAGE:\n\
         \x20   zerodds-idlc <COMMAND> <file.idl> [OPTIONS]\n\
         \x20   zerodds-idlc <file.idl> [BACKEND-FLAGS]            (flache Form)\n\n\
         COMMANDS:\n\
         \x20   generate           Codegen — ein/mehrere Backends emittieren\n\
         \x20   check              Parse + Validierung, kein Codegen\n\
         \x20   dump-ast           Geparstes AST ausgeben\n\
         \x20   dump-typeobject    XTypes-TypeObject je Struct ausgeben\n\
         \x20   print-deps         Make-Dependency-Zeile (Input + #includes)\n\
         \x20   (ohne Command: flache Form, Backend-Flags direkt am Aufruf)\n\n\
         OPTIONS:\n\
         \x20   --parse-only       Parse + print AST (kein Codegen)\n\
         \x20   --dump-ast         Alias fuer --parse-only\n\
         \x20   --dump-deps        Make-Dependency-Zeile (Input + #includes)\n\
         \x20   --dump-typeobject  XTypes-TypeObject je Struct (kein Codegen)\n\
         \x20   --rti              RTI Connext-Grammar-Delta beim Parse\n\
         \x20   --opendds          OpenDDS-Intent (Vendor-Pragmas werden\n\
         \x20                      ohnehin verarbeitet) — Kompat-Flag\n\
         \x20   --cyclone          Cyclone-DDS-Intent — Kompat-Flag\n\
         \x20   --corba            CORBA-Service-Code zusaetzlich emittieren\n\
         \x20                      (--cpp/--csharp/--java: Annex-A.1 inline;\n\
         \x20                       --rust: Two-File-Output via zerodds-corba-rust)\n\
         \x20   --c                C-Header (C-Mode) ueber zerodds-idl-cpp\n\
         \x20   --cpp              C++17-Header ueber zerodds-idl-cpp\n\
         \x20   --rust             Rust-Backend ueber zerodds-idl-rust\n\
         \x20   --ts               TypeScript-Modul ueber zerodds-idl-ts\n\
         \x20   --csharp           C#-Code ueber zerodds-idl-csharp\n\
         \x20   --java             Java-Files (Package-Layout) ueber zerodds-idl-java\n\
         \x20   --python           Python-Modul (@idl_struct + @dataclass) ueber zerodds-idl-python\n\
         \x20   --all              Alle sieben Backends in einem Lauf\n\
         \x20   --default-extensibility KIND  Extensibility fuer un-annotierte\n\
         \x20                      Typen: final|appendable|mutable (auch -de)\n\
         \x20   --default-nested true|false   Un-annotierte Typen @nested\n\
         \x20   --no-typeobject    XTypes-TypeObject-Emission abschalten\n\
         \x20                      (default an, wie RTI/Fast/Cyclone)\n\
         \x20   --scaffold         Build-Datei je Backend mit-emittieren\n\
         \x20                      (Cargo.toml/CMakeLists.txt/pom.xml/…)\n\
         \x20   -o, --output DIR   Ausgabe-Verzeichnis (fuer Backend-Modi)\n\
         \x20   --out-<lang> DIR   Per-Backend-Output-Override; <lang> =\n\
         \x20                      rust|c|cpp|csharp|java|python|ts\n\
         \x20   -I DIR             Include-Search-Pfad (wiederholbar)\n\
         \x20   -D NAME[=VALUE]    Preprocessor-Macro definieren (wiederholbar)\n\
         \x20   -v, --verbose      Mehr Status-Ausgabe (wiederholbar: -vv)\n\
         \x20   -q, --quiet        Nur Fehler ausgeben\n\
         \x20   --color WHEN       Farb-Modus: auto (default), always, never\n\
         \x20   -h, --help         Diese Hilfe\n\
         \x20   -V, --version      Versions-Info\n\n\
         PREPROCESSOR:\n\
         \x20   #include, #define, #ifdef/#else/#endif und #pragma werden\n\
         \x20   vor dem Parsen expandiert. -I fuegt Include-Pfade hinzu,\n\
         \x20   -D definiert Macros (ohne Wert => 1, gcc-Konvention).\n\n\
         EXIT-CODES:\n\
         \x20   0  Erfolg\n\
         \x20   1  Parse-Fehler\n\
         \x20   2  CLI/IO-Fehler\n\
         \x20   3  Backend nicht implementiert oder Codegen-Fehler\n"
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn run_with_no_args_errors_with_usage() {
        let result = run(&[]);
        assert!(matches!(result, Err(CliError::NotImplemented(_))));
    }

    #[test]
    fn run_with_unknown_flag_errors() {
        let result = run(&["--bogus".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn run_with_help_succeeds() {
        let result = run(&["--help".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_with_version_succeeds() {
        let result = run(&["--version".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_parse_only_without_file_errors() {
        let result = run(&["--parse-only".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn run_parse_only_with_missing_file_errors_io() {
        let result = run(&[
            "--parse-only".to_string(),
            "/nonexistent/file.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Io(_))));
    }

    #[test]
    fn cli_error_exit_codes_are_distinct() {
        assert_eq!(CliError::Parse("x".into()).exit_code(), 1);
        assert_eq!(CliError::Usage("x".into()).exit_code(), 2);
        assert_eq!(CliError::Io("x".into()).exit_code(), 2);
        assert_eq!(CliError::NotImplemented("x".into()).exit_code(), 3);
    }

    /// Eindeutiger Test-Workdir pro Aufruf — vermeidet Race zwischen
    /// `cargo test`-Threads, die parallel im selben temp_dir landen.
    fn unique_workdir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("zerodds-idlc-{label}-{pid}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("create workdir");
        dir
    }

    #[test]
    fn run_c_backend_writes_header_with_typedef() {
        let work = unique_workdir("c-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--c".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.h")).expect("read generated c header");
        assert!(
            generated.contains("Greeting"),
            "c header should declare Greeting symbol, got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_rust_backend_writes_file_with_expected_struct() {
        let work = unique_workdir("rust-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.rs")).expect("read generated file");
        assert!(
            generated.contains("pub struct Greeting"),
            "generated module should contain the struct, got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_cpp_backend_writes_header_with_expected_class() {
        let work = unique_workdir("cpp-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--cpp".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.hpp")).expect("read generated header");
        assert!(
            generated.contains("class Greeting"),
            "cpp header should declare class Greeting, got:\n{generated}"
        );
        assert!(
            generated.contains("#pragma once"),
            "cpp header should include #pragma once, got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_ts_backend_writes_module_with_expected_symbol() {
        let work = unique_workdir("ts-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--ts".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.ts")).expect("read generated ts");
        assert!(
            generated.contains("Greeting"),
            "ts module should mention Greeting, got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_csharp_backend_writes_file_with_expected_symbol() {
        let work = unique_workdir("csharp-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--csharp".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.cs")).expect("read generated cs");
        assert!(
            generated.contains("Greeting"),
            "C# file should mention Greeting, got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_python_backend_writes_module_with_dataclass() {
        let work = unique_workdir("python-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; string text; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--python".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.py")).expect("read generated py");
        assert!(
            generated.contains("@idl_struct(typename=\"Greeting\")"),
            "{generated}"
        );
        assert!(generated.contains("@dataclass"), "{generated}");
        assert!(generated.contains("class Greeting:"), "{generated}");

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_java_backend_writes_class_file_in_package_layout() {
        let work = unique_workdir("java-ok");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--java".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Recursive walk: irgendwo unter out_dir muss `Greeting.java` liegen,
        // dessen Pfad-Tiefe vom Package abhaengt (Default-Package = direkt
        // in out_dir, sonst in Package-Subdir).
        let mut found_path: Option<PathBuf> = None;
        let mut stack = vec![out_dir.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read out_dir") {
                let entry = entry.expect("dir entry");
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.file_name().and_then(|s| s.to_str()) == Some("Greeting.java") {
                    found_path = Some(p);
                    break;
                }
            }
        }
        let path = found_path
            .unwrap_or_else(|| panic!("Greeting.java not found under {}", out_dir.display()));
        let source = std::fs::read_to_string(&path).expect("read greeting.java");
        assert!(
            source.contains("class Greeting"),
            "Greeting.java should declare class Greeting, got:\n{source}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn color_mode_parses_when_values() {
        assert_eq!(
            color_mode_from_args(&["--color".into(), "always".into()]),
            ColorMode::Always
        );
        assert_eq!(
            color_mode_from_args(&["--color".into(), "never".into()]),
            ColorMode::Never
        );
        assert_eq!(color_mode_from_args(&[]), ColorMode::Auto);
    }

    #[test]
    fn run_color_rejects_bad_value() {
        let result = run(&["--color".into(), "rainbow".into()]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn run_verbose_flag_accepted() {
        let work = unique_workdir("verbose");
        let idl = work.join("v.idl");
        std::fs::write(&idl, "struct V { long a; };").expect("write idl");
        let out = work.join("out");
        // -v und -q sind reine Ausgabe-Modifier — duerfen den Codegen
        // nicht beeinflussen.
        let result = run(&[
            "-vv".to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        assert!(out.join("v.rs").exists());
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_duplicate_backend_flag_is_idempotent() {
        // `--rust --rust` darf kein Fehler sein — push_backend dedupliziert.
        let work = unique_workdir("dup-flag");
        let idl = work.join("d.idl");
        std::fs::write(&idl, "struct D { long a; };").expect("write idl");
        let out = work.join("out");
        let result = run(&[
            "--rust".to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(
            result.is_ok(),
            "duplicate --rust should be idempotent, got {result:?}"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_rust_backend_without_output_errors_usage() {
        let work = unique_workdir("rust-no-out");
        let idl_path = work.join("x.idl");
        std::fs::write(&idl_path, "struct X { long a; };").expect("write idl");

        let result = run(&["--rust".to_string(), idl_path.to_string_lossy().to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_parse_only_and_backend_are_mutually_exclusive() {
        let result = run(&[
            "--parse-only".to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            "/tmp/x".to_string(),
            "some.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn run_output_flag_without_value_errors_usage() {
        let result = run(&["--rust".to_string(), "-o".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    /// `--corba` ohne Backend: nicht klar was gemeint ist → Usage-Error.
    #[test]
    fn run_corba_alone_errors_usage() {
        let result = run(&[
            "--corba".to_string(),
            "-o".to_string(),
            "/tmp/x".to_string(),
            "some.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    /// `--corba` mit C/TS/Python: Library hat kein emit_corba_traits.
    /// Exit-Code 3 (NotImplemented) statt 2, damit CI das von
    /// „kaputtem User-Input" unterscheiden kann.
    #[test]
    fn run_corba_with_c_errors_not_implemented() {
        let work = unique_workdir("corba-c");
        let idl_path = work.join("x.idl");
        std::fs::write(&idl_path, "struct X { long a; };").expect("write idl");

        let result = run(&[
            "--c".to_string(),
            "--corba".to_string(),
            "-o".to_string(),
            work.join("out").to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(
            matches!(result, Err(CliError::NotImplemented(_))),
            "got {result:?}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    /// `--rust --corba` emittiert Two-File-Output: <base>.rs (Types)
    /// + <base>_corba.rs (Service-Code via zerodds-corba-rust).
    /// Vendor-Spec: zerodds-corba-rust-1.0.
    #[test]
    fn run_rust_corba_emits_types_and_service_files() {
        let work = unique_workdir("rust-corba");
        let idl_path = work.join("svc.idl");
        std::fs::write(
            &idl_path,
            "interface Calc { long add(in long a, in long b); };",
        )
        .expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--rust".to_string(),
            "--corba".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // svc.rs muss existieren (auch leer wenn IDL keine Types hat).
        let types_path = out_dir.join("svc.rs");
        assert!(
            types_path.exists(),
            "expected {} to exist",
            types_path.display(),
        );

        // svc_corba.rs muss den Calc-Trait + Stub enthalten.
        let svc_path = out_dir.join("svc_corba.rs");
        let svc = std::fs::read_to_string(&svc_path).expect("read svc_corba.rs");
        assert!(
            svc.contains("pub trait Calc"),
            "expected trait Calc, got:\n{svc}",
        );
        assert!(svc.contains("CalcStub"), "expected CalcStub, got:\n{svc}",);
        assert!(
            svc.contains("CALC_REPOSITORY_ID"),
            "expected Repository-ID const, got:\n{svc}",
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_cpp_corba_emits_namespace_corba_block() {
        let work = unique_workdir("cpp-corba");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--cpp".to_string(),
            "--corba".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.hpp")).expect("read generated header");
        assert!(
            generated.contains("namespace CORBA"),
            "cpp header with --corba should include 'namespace CORBA', got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_csharp_corba_emits_corba_namespace() {
        let work = unique_workdir("cs-corba");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--csharp".to_string(),
            "--corba".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("chat.cs")).expect("read generated cs");
        assert!(
            generated.contains("namespace Corba"),
            "C# with --corba should include 'namespace Corba', got:\n{generated}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_java_corba_emits_corba_traits_class() {
        let work = unique_workdir("java-corba");
        let idl_path = work.join("chat.idl");
        std::fs::write(&idl_path, "struct Greeting { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--java".to_string(),
            "--corba".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Recursive walk fuer eine *CorbaTraits.java-Datei
        let mut found = false;
        let mut stack = vec![out_dir.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read out_dir") {
                let entry = entry.expect("dir entry");
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.ends_with("CorbaTraits.java"))
                {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(
            found,
            "java --corba should emit a *CorbaTraits.java file under {}",
            out_dir.display()
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_resolves_include_directive() {
        let work = unique_workdir("pp-include");
        let inc = work.join("inc");
        std::fs::create_dir_all(&inc).expect("mkdir inc");
        std::fs::write(inc.join("common.idl"), "struct Vec3 { double x; };").expect("write common");
        let idl = work.join("robot.idl");
        std::fs::write(&idl, "#include \"common.idl\"\nstruct Pose { Vec3 p; };")
            .expect("write robot");
        let out = work.join("out");

        let result = run(&[
            "--rust".to_string(),
            "-I".to_string(),
            inc.to_string_lossy().to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let robot_src = std::fs::read_to_string(out.join("robot.rs")).expect("read gen");
        assert!(
            robot_src.contains("Vec3"),
            "included type Vec3 missing:\n{robot_src}"
        );
        assert!(robot_src.contains("Pose"), "Pose missing:\n{robot_src}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_define_flag_drives_ifdef() {
        let work = unique_workdir("pp-define");
        let idl = work.join("cfg.idl");
        std::fs::write(
            &idl,
            "#ifdef EXTENDED\nstruct Cfg { long a; long b; };\n\
             #else\nstruct Cfg { long a; };\n#endif",
        )
        .expect("write idl");

        // mit -D EXTENDED → beide Felder.
        let out_on = work.join("on");
        let r = run(&[
            "--rust".to_string(),
            "-D".to_string(),
            "EXTENDED".to_string(),
            "-o".to_string(),
            out_on.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(r.is_ok(), "expected Ok, got {r:?}");
        let gen_on = std::fs::read_to_string(out_on.join("cfg.rs")).expect("read on");
        assert!(gen_on.contains("pub b"), "EXTENDED branch missing field b");

        // ohne -D → nur Feld a (#else-Zweig).
        let out_off = work.join("off");
        let r = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out_off.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(r.is_ok(), "expected Ok, got {r:?}");
        let gen_off = std::fs::read_to_string(out_off.join("cfg.rs")).expect("read off");
        assert!(
            !gen_off.contains("pub b"),
            "#else branch should omit field b"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_define_with_value_expands_macro() {
        let work = unique_workdir("pp-macro");
        let idl = work.join("bound.idl");
        std::fs::write(&idl, "struct Buf { string<CAP> data; };").expect("write idl");
        let out = work.join("out");

        let r = run(&[
            "--rust".to_string(),
            "-D".to_string(),
            "CAP=64".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(r.is_ok(), "expected Ok, got {r:?}");
        // CAP wird zu 64 expandiert; ohne Expansion waere `string<CAP>`
        // ein Parse-Fehler (CAP ist kein numerisches Literal).
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_all_emits_every_backend() {
        let work = unique_workdir("all-backends");
        let idl = work.join("types.idl");
        std::fs::write(&idl, "struct Ping { @key long id; double value; };").expect("write idl");
        let out = work.join("out");

        let result = run(&[
            "--all".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Ein File pro Single-File-Backend.
        for ext in ["h", "hpp", "rs", "ts", "cs", "py"] {
            let p = out.join(format!("types.{ext}"));
            assert!(p.exists(), "--all should emit types.{ext}");
        }
        // Java emittiert ein Package-Layout — irgendeine .java muss da sein.
        let java_found = std::fs::read_dir(&out)
            .expect("read out")
            .filter_map(Result::ok)
            .any(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("java") || e.path().is_dir()
            });
        assert!(java_found, "--all should emit Java output");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_multiple_backend_flags_combine() {
        let work = unique_workdir("multi-flag");
        let idl = work.join("t.idl");
        std::fs::write(&idl, "struct T { long a; };").expect("write idl");
        let out = work.join("out");
        let result = run(&[
            "--rust".to_string(),
            "--python".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert!(out.join("t.rs").exists(), "rust output missing");
        assert!(out.join("t.py").exists(), "python output missing");
        assert!(!out.join("t.hpp").exists(), "cpp should NOT be emitted");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_dump_deps_lists_includes() {
        let work = unique_workdir("dump-deps");
        let inc = work.join("inc");
        std::fs::create_dir_all(&inc).expect("mkdir");
        std::fs::write(inc.join("base.idl"), "struct B { long x; };").expect("write base");
        let idl = work.join("top.idl");
        std::fs::write(&idl, "#include \"base.idl\"\nstruct T { B b; };").expect("write top");
        let result = run(&[
            "--dump-deps".to_string(),
            "-I".to_string(),
            inc.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        // --dump-deps ist eine Action ohne Backend -> kein Fehler.
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_dump_deps_rejects_backend_combo() {
        let result = run(&[
            "--dump-deps".to_string(),
            "--rust".to_string(),
            "some.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn run_dump_typeobject_maps_primitive_struct() {
        let work = unique_workdir("dump-to");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("v.idl");
        std::fs::write(&idl, "struct V { double x; double y; };").expect("write");
        let result = run(&[
            "--dump-typeobject".to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_dump_ast_is_parse_only_alias() {
        let work = unique_workdir("dump-ast");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("a.idl");
        std::fs::write(&idl, "struct A { long n; };").expect("write");
        let result = run(&["--dump-ast".to_string(), idl.to_string_lossy().to_string()]);
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_dump_typeobject_rejects_backend_combo() {
        let result = run(&[
            "--dump-typeobject".to_string(),
            "--java".to_string(),
            "some.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn subcommand_generate_emits_backend() {
        let work = unique_workdir("sc-generate");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("g.idl");
        std::fs::write(&idl, "struct G { long n; };").expect("write");
        let out = work.join("out");
        let result = run(&[
            "generate".to_string(),
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        assert!(out.join("g.rs").exists(), "g.rs not emitted");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn subcommand_generate_without_backend_errors() {
        let result = run(&["generate".to_string(), "some.idl".to_string()]);
        match result {
            Err(CliError::NotImplemented(m)) => {
                assert!(m.contains("generate requires"), "msg was: {m}");
            }
            other => panic!("expected NotImplemented, got {other:?}"),
        }
    }

    #[test]
    fn subcommand_check_parses_without_codegen() {
        let work = unique_workdir("sc-check");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("c.idl");
        std::fs::write(&idl, "struct C { long n; };").expect("write");
        let result = run(&["check".to_string(), idl.to_string_lossy().to_string()]);
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn subcommand_check_rejects_backend_flag() {
        let result = run(&[
            "check".to_string(),
            "some.idl".to_string(),
            "--rust".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn subcommand_print_deps_runs() {
        let work = unique_workdir("sc-deps");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("d.idl");
        std::fs::write(&idl, "struct D { long n; };").expect("write");
        let result = run(&["print-deps".to_string(), idl.to_string_lossy().to_string()]);
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn split_subcommand_detects_leading_keyword() {
        let args: Vec<String> = ["check", "foo.idl"].iter().map(|s| s.to_string()).collect();
        let (sc, rest) = split_subcommand(&args);
        assert_eq!(sc, Some(SubCommand::Check));
        assert_eq!(rest, vec!["foo.idl".to_string()]);
    }

    #[test]
    fn split_subcommand_allows_leading_global_flags() {
        let args: Vec<String> = ["-v", "generate", "foo.idl"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sc, rest) = split_subcommand(&args);
        assert_eq!(sc, Some(SubCommand::Generate));
        assert_eq!(rest, vec!["-v".to_string(), "foo.idl".to_string()]);
    }

    #[test]
    fn split_subcommand_ignores_keyword_as_output_dir() {
        // `-o check` => Ausgabe-Ordner "check", KEIN Sub-Command.
        let args: Vec<String> = ["-o", "check", "foo.idl"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sc, rest) = split_subcommand(&args);
        assert_eq!(sc, None);
        assert_eq!(rest, args);
    }

    #[test]
    fn run_out_override_directs_backend_output() {
        let work = unique_workdir("out-override");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("o.idl");
        std::fs::write(&idl, "struct O { long n; };").expect("write");
        let global = work.join("global");
        let rust_dir = work.join("rust-only");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "--python".to_string(),
            "-o".to_string(),
            global.to_string_lossy().to_string(),
            "--out-rust".to_string(),
            rust_dir.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        // Rust folgt dem Override, Python faellt auf -o zurueck.
        assert!(rust_dir.join("o.rs").exists(), "rust override ignored");
        assert!(global.join("o.py").exists(), "python -o fallback failed");
        assert!(!global.join("o.rs").exists(), "rust leaked into -o");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_backend_without_any_output_dir_errors() {
        // Echte Datei — sonst schlaegt der Datei-Lesefehler vor der
        // Output-Dir-Pruefung zu.
        let work = unique_workdir("no-outdir");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("n.idl");
        std::fs::write(&idl, "struct N { long n; };").expect("write");
        let result = run(&[idl.to_string_lossy().to_string(), "--rust".to_string()]);
        match result {
            Err(CliError::Usage(m)) => {
                assert!(m.contains("--out-rust"), "msg was: {m}");
            }
            other => panic!("expected Usage, got {other:?}"),
        }
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_emits_typeobject_block_by_default() {
        let work = unique_workdir("to-default");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("t.idl");
        std::fs::write(&idl, "struct T { long a; double b; };").expect("write");
        let out = work.join("gen");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let code = std::fs::read_to_string(out.join("t.rs")).expect("read");
        assert!(
            code.contains("pub mod type_objects"),
            "TypeObject block missing by default"
        );
        assert!(code.contains("pub const T:"), "type T const missing");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_no_typeobject_omits_block() {
        let work = unique_workdir("to-off");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("t.idl");
        std::fs::write(&idl, "struct T { long a; };").expect("write");
        let out = work.join("gen");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "--no-typeobject".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let code = std::fs::read_to_string(out.join("t.rs")).expect("read");
        assert!(
            !code.contains("type_objects"),
            "--no-typeobject must omit the block"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_recursive_type_skips_typeobject_but_still_generates() {
        // sequence<Node> erzeugt einen Mapper-Zyklus → TypeObject-
        // Emission entfaellt, der Kern-Codegen laeuft trotzdem.
        let work = unique_workdir("to-recursive");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("n.idl");
        std::fs::write(&idl, "struct Node { long id; sequence<Node> kids; };").expect("write");
        let out = work.join("gen");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(
            result.is_ok(),
            "codegen must succeed despite TO failure: {result:?}"
        );
        let code = std::fs::read_to_string(out.join("n.rs")).expect("read");
        assert!(code.contains("struct Node"), "type still generated");
        assert!(!code.contains("type_objects"), "TO block skipped on cycle");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_scaffold_writes_build_file() {
        let work = unique_workdir("scaffold");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("robot.idl");
        std::fs::write(&idl, "struct R { long a; };").expect("write");
        let out = work.join("gen");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "--scaffold".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let cargo = std::fs::read_to_string(out.join("Cargo.toml")).expect("Cargo.toml");
        assert!(cargo.contains("name = \"robot\""));
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_scaffold_keeps_existing_build_file() {
        let work = unique_workdir("scaffold-keep");
        let out = work.join("gen");
        std::fs::create_dir_all(&out).expect("mkdir");
        let idl = work.join("robot.idl");
        std::fs::write(&idl, "struct R { long a; };").expect("write");
        // Vorhandene Cargo.toml darf nicht ueberschrieben werden.
        std::fs::write(out.join("Cargo.toml"), "# hand-edited\n").expect("seed");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "--scaffold".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let cargo = std::fs::read_to_string(out.join("Cargo.toml")).expect("read");
        assert_eq!(cargo, "# hand-edited\n", "existing build file must be kept");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_out_override_alone_is_enough() {
        // Kein globales -o, aber --out-rust deckt das einzige Backend.
        let work = unique_workdir("out-only");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("s.idl");
        std::fs::write(&idl, "struct S { long n; };").expect("write");
        let rust_dir = work.join("rs");
        let result = run(&[
            idl.to_string_lossy().to_string(),
            "--rust".to_string(),
            "--out-rust".to_string(),
            rust_dir.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        assert!(rust_dir.join("s.rs").exists());
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn backend_out_dir_prefers_override_over_global() {
        let opts = CliOptions {
            output: Some(PathBuf::from("/global")),
            out_overrides: vec![(Backend::Java, PathBuf::from("/java-out"))],
            ..CliOptions::default()
        };
        assert_eq!(
            opts.backend_out_dir(Backend::Java),
            Some(Path::new("/java-out"))
        );
        assert_eq!(
            opts.backend_out_dir(Backend::Rust),
            Some(Path::new("/global"))
        );
        let empty = CliOptions::default();
        assert_eq!(empty.backend_out_dir(Backend::Rust), None);
    }

    #[test]
    fn run_default_extensibility_flag_accepted() {
        let work = unique_workdir("def-ext");
        std::fs::create_dir_all(&work).expect("mkdir");
        let idl = work.join("e.idl");
        std::fs::write(&idl, "struct E { long n; };").expect("write");
        let result = run(&[
            "check".to_string(),
            idl.to_string_lossy().to_string(),
            "--default-extensibility".to_string(),
            "mutable".to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_default_extensibility_rejects_bad_value() {
        let result = run(&[
            "check".to_string(),
            "x.idl".to_string(),
            "--default-extensibility".to_string(),
            "wobbly".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn run_default_nested_rejects_bad_value() {
        let result = run(&[
            "check".to_string(),
            "x.idl".to_string(),
            "--default-nested".to_string(),
            "maybe".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
    }

    #[test]
    fn split_subcommand_plain_file_has_no_subcommand() {
        let args: Vec<String> = ["foo.idl", "--rust"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sc, rest) = split_subcommand(&args);
        assert_eq!(sc, None);
        assert_eq!(rest, args);
    }

    #[test]
    fn run_pragma_keylist_marks_key() {
        let work = unique_workdir("pragma-keylist");
        let idl = work.join("p.idl");
        std::fs::write(
            &idl,
            "struct Pose { string rid; double x; };\n#pragma keylist Pose rid\n",
        )
        .expect("write idl");
        let out = work.join("out");
        let result = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let src = std::fs::read_to_string(out.join("p.rs")).expect("read");
        assert!(
            src.contains("HAS_KEY: bool = true"),
            "#pragma keylist should make Pose keyed:\n{src}"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_dcps_data_key_marks_key() {
        let work = unique_workdir("pragma-dcps");
        let idl = work.join("s.idl");
        std::fs::write(
            &idl,
            "struct Sensor { string sid; long v; };\n\
             #pragma DCPS_DATA_KEY \"Sensor sid\"\n",
        )
        .expect("write idl");
        let out = work.join("out");
        let result = run(&[
            "--rust".to_string(),
            "--opendds".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let src = std::fs::read_to_string(out.join("s.rs")).expect("read");
        assert!(
            src.contains("HAS_KEY: bool = true"),
            "#pragma DCPS_DATA_KEY should make Sensor keyed:\n{src}"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn run_missing_include_errors_parse() {
        let work = unique_workdir("pp-missing");
        let idl = work.join("broken.idl");
        std::fs::write(&idl, "#include \"nope.idl\"\nstruct X { long a; };").expect("write idl");
        let out = work.join("out");
        let result = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Parse(_))), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }
}
