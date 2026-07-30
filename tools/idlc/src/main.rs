// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 compiler: 17 backends over the `zerodds-idl-*` codegen crates.
//!
//! Alle 17 Backends verdrahtet — `--c`, `--cpp`, `--rust`, `--ts`,
//! `--csharp`, `--java`, `--python`, `--go`, `--nim`, `--zig`, `--d`,
//! `--ada`, `--elixir`, `--ocaml`, `--julia`, `--lua`, `--swift`
//! (`--all` emittiert alle). Jedes Backend ruft `generate_<lang>_module`
//! seiner Codegen-Crate; identische AST-Quelle, byte-identisch zu den
//! `zerodds-cdr`-Goldens.
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
//! zerodds-idlc --go     -o <dir> <file.idl>        Go-Modul in <dir>/<base>.go
//! zerodds-idlc --<lang> -o <dir> <file.idl>        nim/zig/d/ada/elixir/ocaml/julia/lua/swift analog
//! zerodds-idlc --all    -o <dir> <file.idl>        alle 17 Backends
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
//!   3   Codegen-Fehler (oder `--corba` mit einem Backend ohne CORBA-Traits)

#![allow(clippy::print_stderr, clippy::print_stdout)] // CLI-Tool: I/O auf stdio zulaessig.

mod scaffold;
mod typeobject_emit;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use typeobject_emit::TypeObjectBlob;

use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::{parse, parse_with_deltas};
use zerodds_idl::preprocessor::Preprocessor;
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};
use zerodds_idl_compose::{DefaultExt, FsResolver, default_ext, key_pragmas};
use zerodds_idl_cpp::{
    CGenOptions, CppGenOptions, generate_c_header, generate_cpp_header,
    generate_cpp_header_with_corba_traits,
};
use zerodds_idl_csharp::{CsGenOptions, generate_csharp, generate_csharp_with_corba_traits};
use zerodds_idl_d::{DGenOptions, generate_d_module};
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};
use zerodds_idl_go::{GoGenOptions, generate_go_module};
use zerodds_idl_java::{
    JavaGenOptions, generate_java_files, generate_java_files_with_corba_traits,
};
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};
use zerodds_idl_python::{PythonGenOptions, generate_python_module};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};
use zerodds_idl_ts::generate_ts_source;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

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
    /// Semantic validation failed (resolver, unresolved type references, or
    /// a spec-constraint validator). Reported by the `check` and `generate`
    /// paths; carries one finding per line.
    Semantic(String),
    NotImplemented(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::Io(_) => 2,
            Self::Parse(_) | Self::Semantic(_) => 1,
            Self::NotImplemented(_) => 3,
        }
    }
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Usage(m)
            | Self::Io(m)
            | Self::Parse(m)
            | Self::Semantic(m)
            | Self::NotImplemented(m) => f.write_str(m),
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
    Go,
    Ada,
    Zig,
    Nim,
    D,
    Elixir,
    OCaml,
    Julia,
    Lua,
    Swift,
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
    /// `check` sub-command — run the semantic gate, emit no code and dump no
    /// AST. Distinct from `parse_only` (`dump-ast` / `--parse-only`), which
    /// stays lenient so a broken AST can still be inspected.
    check: bool,
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
    /// `--depfile <path>` — Make-/Ninja-kompatible Depfile schreiben.
    /// Target = Stamp (falls gesetzt) sonst alle erzeugten Outputs;
    /// Prerequisites = die IDL + alle transitiv `#include`-ten Dateien
    /// (kanonische Resolver-Pfade). Teil des Build-Vertrags.
    depfile: Option<PathBuf>,
    /// `--output-manifest <path>` (Alias `--manifest`) — JSON mit allen
    /// tatsaechlich erzeugten Dateien je Backend. Consumer (CMake/Gradle/
    /// MSBuild/SPM) brauchen keine Endungslogik mehr.
    manifest: Option<PathBuf>,
    /// `--stamp <path>` — eine stabile Stamp-Datei als einziger deklarierter
    /// OUTPUT (touch nach Erfolg), unabhaengig davon wie viele/welche Dateien
    /// ein Backend erzeugt.
    stamp: Option<PathBuf>,
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
        Backend::Go => "go",
        Backend::Ada => "ada",
        Backend::Zig => "zig",
        Backend::Nim => "nim",
        Backend::D => "d",
        Backend::Elixir => "elixir",
        Backend::OCaml => "ocaml",
        Backend::Julia => "julia",
        Backend::Lua => "lua",
        Backend::Swift => "swift",
    }
}

/// Backend zu einem `--out-<lang>`-Flag, `None` bei unbekanntem Suffix.
fn out_flag_backend(flag: &str) -> Option<Backend> {
    let suffix = flag.strip_prefix("--out-")?;
    ALL_BACKENDS
        .into_iter()
        .find(|b| backend_flag_suffix(*b) == suffix)
}

/// Alle 17 Backends — fuer `--all`.
const ALL_BACKENDS: [Backend; 17] = [
    Backend::C,
    Backend::Cpp,
    Backend::Rust,
    Backend::Ts,
    Backend::CSharp,
    Backend::Java,
    Backend::Python,
    Backend::Go,
    Backend::Ada,
    Backend::Zig,
    Backend::Nim,
    Backend::D,
    Backend::Elixir,
    Backend::OCaml,
    Backend::Julia,
    Backend::Lua,
    Backend::Swift,
];

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
        Some(SubCommand::Check) => opts.check = true,
        Some(SubCommand::DumpAst) => opts.parse_only = true,
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
            // Build-Vertrag (Schritt 1a): Depfile / Manifest / Stamp.
            "--depfile" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage("--depfile requires a path".to_string()))?;
                opts.depfile = Some(PathBuf::from(value));
            }
            "--output-manifest" | "--manifest" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("{arg} requires a path")))?;
                opts.manifest = Some(PathBuf::from(value));
            }
            "--stamp" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage("--stamp requires a path".to_string()))?;
                opts.stamp = Some(PathBuf::from(value));
            }
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
            "--go" => push_backend(&mut opts.backends, Backend::Go),
            "--ada" => push_backend(&mut opts.backends, Backend::Ada),
            "--zig" => push_backend(&mut opts.backends, Backend::Zig),
            "--nim" => push_backend(&mut opts.backends, Backend::Nim),
            "--d" => push_backend(&mut opts.backends, Backend::D),
            "--elixir" => push_backend(&mut opts.backends, Backend::Elixir),
            "--ocaml" => push_backend(&mut opts.backends, Backend::OCaml),
            "--julia" => push_backend(&mut opts.backends, Backend::Julia),
            "--lua" => push_backend(&mut opts.backends, Backend::Lua),
            "--swift" => push_backend(&mut opts.backends, Backend::Swift),
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
    // Modes that produce no backend code: the dump/parse actions plus
    // `check` (semantic gate only).
    let non_emitting = dump_action || opts.check;
    if !non_emitting && opts.backends.is_empty() {
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
    if non_emitting && !opts.backends.is_empty() {
        return Err(CliError::Usage(
            "the selected mode (check / dump-ast / dump-typeobject / print-deps / \
             --parse-only) does not emit code — drop the backend flag"
                .to_string(),
        ));
    }
    // Build-Vertrag (--depfile/--output-manifest/--stamp) beschreibt den
    // Codegen-Output — in einem nicht-emittierenden Modus gibt es nichts zu
    // beschreiben.
    if non_emitting && (opts.depfile.is_some() || opts.manifest.is_some() || opts.stamp.is_some()) {
        return Err(CliError::Usage(
            "--depfile / --output-manifest / --stamp require a backend (generate mode); \
             they describe the generated output"
                .to_string(),
        ));
    }
    let path = opts
        .file
        .as_ref()
        .ok_or_else(|| CliError::Usage("missing input file (try --help)".to_string()))?;
    let mut raw = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("cannot read {path}: {e}")))?;
    // Strip a leading UTF-8 BOM (U+FEFF) — not part of the IDL.
    if raw.starts_with('\u{feff}') {
        raw.remove(0);
    }

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
    let resolver = FsResolver::new(opts.include_dirs.clone());
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
    {
        // CLI `--default-extensibility` wins; otherwise the compile-time
        // `ext-default-*` Cargo feature (XTypes 1.3 §7.3.3.1, SX2 §7.3.3.1).
        let ext = opts.default_ext.unwrap_or_else(DefaultExt::cfg_default);
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

    // Semantic gate — the single barrier that `check` and `generate` share.
    // It runs after parse + default-patches and BEFORE TypeObject lowering,
    // so `--no-typeobject` cannot bypass it. The lenient inspection modes
    // (`dump-ast` / `--parse-only` / `dump-typeobject` / `print-deps`) skip
    // it so a broken AST stays inspectable.
    if opts.check || !non_emitting {
        let pragma_prefixes: Vec<(String, usize)> = processed
            .pragma_prefixes
            .iter()
            .map(|p| (p.prefix.clone(), p.line))
            .collect();
        zerodds_idl::semantics::resolve_and_validate(&ast, &pragma_prefixes)
            .map_err(|e| CliError::Semantic(format!("semantic validation failed:\n{e}")))?;
        opts.report(1, "semantic validation passed");
    }

    if opts.check {
        opts.report(0, &format!("{path}: OK"));
        return Ok(());
    }

    if opts.parse_only {
        println!("{ast}");
        return Ok(());
    }

    if opts.dump_typeobject {
        return dump_typeobjects(&ast);
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

    // Jedes gewaehlte Backend erzeugt seinen Output ZUERST vollstaendig im
    // Speicher (kein `std::fs::write`). Bei --all sind das alle 17; Reihenfolge
    // = ALL_BACKENDS. Ziel ist der `--out-<lang>`-Override, sonst das globale
    // `-o`. Jeder Lauf sammelt die erzeugten Dateien fuer den Build-Vertrag —
    // kein Raten von Endungen, ein Backend ohne Output wird ehrlich mit leerer
    // Dateiliste abgebildet. Erst wenn ALLE Backends erfolgreich gestaged
    // haben, werden die Dateien atomar auf die Platte geschrieben: faellt ein
    // Backend (oder das Semantik-Gate oben) aus, bleibt keine Teil-Ausgabe
    // eines anderen Backends liegen.
    let mut manifest_entries: Vec<BackendOutput> = Vec::with_capacity(opts.backends.len());
    for backend in &opts.backends {
        let suffix = backend_flag_suffix(*backend);
        let out_dir = opts.backend_out_dir(*backend).ok_or_else(|| {
            CliError::Usage(format!(
                "{} backend: no output directory — pass -o <dir> or --out-{suffix} <dir>",
                backend_name(*backend)
            ))
        })?;
        let files = emit_backend(*backend, &ast, &opts, out_dir, &base, &type_objects)?;
        opts.report(
            1,
            &format!(
                "staged {} backend ({} file(s)) → {}",
                backend_name(*backend),
                files.len(),
                out_dir.display()
            ),
        );
        manifest_entries.push(BackendOutput {
            backend: *backend,
            out_dir: out_dir.to_path_buf(),
            files,
        });
    }

    // Atomarer Veroeffentlichungs-Punkt: alle gestageten Dateien jetzt — nach
    // dem Erfolg saemtlicher Backends — auf die Platte schreiben.
    flush_staged_files(&manifest_entries)?;

    // Build-Vertrag: Depfile + Manifest + Stamp. Werden erst NACH dem
    // erfolgreichen Codegen aller Backends geschrieben — schlaegt ein
    // Backend fehl (oder das Semantik-Gate weiter oben), entsteht weder
    // Stamp noch Manifest noch Depfile.
    if opts.depfile.is_some() || opts.manifest.is_some() || opts.stamp.is_some() {
        write_build_contract(path, &processed, &opts, &manifest_entries)?;
    }
    Ok(())
}

/// Ein Backend-Eintrag fuer den Build-Vertrag: das Ziel-Verzeichnis und die
/// Liste der erzeugten Dateien (kann leer sein, wenn ein Backend fuer die
/// gegebene IDL keinen Output erzeugt). Die Dateien sind bis zum atomaren
/// Flush im Speicher gehalten ([`StagedFile`]).
struct BackendOutput {
    backend: Backend,
    out_dir: PathBuf,
    files: Vec<StagedFile>,
}

/// Schreibt die Build-Vertrag-Artefakte (Depfile / Manifest / Stamp) nach
/// erfolgreichem Codegen. Reihenfolge: Depfile + Manifest zuerst, Stamp
/// zuletzt — der Stamp ist der einzige deklarierte OUTPUT und wird als
/// letztes getoucht, damit seine mtime alle anderen Artefakte dominiert.
fn write_build_contract(
    idl_path: &str,
    processed: &zerodds_idl::preprocessor::ProcessedSource,
    opts: &CliOptions,
    entries: &[BackendOutput],
) -> Result<(), CliError> {
    // Prerequisites = die IDL + alle transitiv #include-ten Dateien. Der
    // Resolver liefert die kanonischen Pfade via Resolved{path}; die
    // source_map fuehrt Datei 0 = Top-IDL, danach die kanonischen Includes.
    let mut deps: Vec<String> = Vec::new();
    let map = &processed.source_map;
    for i in 0..map.file_count() {
        if let Some(name) = map.file_name(zerodds_idl::preprocessor::FileId(
            u32::try_from(i).unwrap_or(0),
        )) {
            // Die Top-IDL (Index 0) steht als CLI-Pfad drin — fuer eine
            // einheitliche (kanonische) Depfile wird sie ebenfalls
            // kanonisiert; Includes sind bereits kanonisch.
            let canon = std::fs::canonicalize(name)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| name.to_string());
            if !deps.contains(&canon) {
                deps.push(canon);
            }
        }
    }

    // Flache, deduplizierte, sortierte Liste aller erzeugten Dateien.
    let mut all_outputs: Vec<String> = Vec::new();
    for e in entries {
        for f in &e.files {
            let s = f.path.to_string_lossy().into_owned();
            if !all_outputs.contains(&s) {
                all_outputs.push(s);
            }
        }
    }
    all_outputs.sort();

    if let Some(dep_path) = &opts.depfile {
        // Depfile-Target: der Stamp (Ninja verlangt genau EIN Target),
        // sonst alle erzeugten Outputs (Make-Multi-Target-Regel).
        let targets: Vec<String> = match &opts.stamp {
            Some(stamp) => vec![stamp.to_string_lossy().into_owned()],
            None => all_outputs.clone(),
        };
        let content = render_depfile(&targets, &deps);
        write_contract_file(dep_path, content.as_bytes())?;
        opts.report(1, &format!("wrote depfile → {}", dep_path.display()));
    }

    if let Some(man_path) = &opts.manifest {
        let content = render_manifest(idl_path, opts, &deps, entries, &all_outputs);
        write_contract_file(man_path, content.as_bytes())?;
        opts.report(1, &format!("wrote manifest → {}", man_path.display()));
    }

    if let Some(stamp_path) = &opts.stamp {
        // Stamp zuletzt: leerer Inhalt, mtime = Erfolgs-Zeitpunkt.
        write_contract_file(stamp_path, b"")?;
        opts.report(1, &format!("touched stamp → {}", stamp_path.display()));
    }
    Ok(())
}

/// Schreibt eine Build-Vertrag-Datei; legt fehlende Elternverzeichnisse an.
fn write_contract_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Io(format!("cannot create {}: {e}", parent.display())))?;
        }
    }
    std::fs::write(path, bytes)
        .map_err(|e| CliError::Io(format!("cannot write {}: {e}", path.display())))
}

/// Rendert eine Make-/Ninja-kompatible Depfile-Regel:
/// `<target...>: <prereq...>`. Pfade werden GNU-Make-Style escaped
/// (Space/`#`/`$`). Eine einzelne Regel — der von Ninja verlangte
/// gemeinsame Nenner (kein `-MP`-Phony-Block, den Ninja ablehnt).
fn render_depfile(targets: &[String], deps: &[String]) -> String {
    let target = targets
        .iter()
        .map(|t| escape_make_path(t))
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = String::new();
    out.push_str(&target);
    out.push(':');
    for d in deps {
        out.push(' ');
        out.push_str(&escape_make_path(d));
    }
    out.push('\n');
    out
}

/// GNU-Make/Ninja-Depfile-Escaping fuer einen Pfad: Space → `\ `,
/// `#` → `\#`, `$` → `$$`.
fn escape_make_path(p: &str) -> String {
    let mut s = String::with_capacity(p.len());
    for c in p.chars() {
        match c {
            ' ' => s.push_str("\\ "),
            '#' => s.push_str("\\#"),
            '$' => s.push_str("$$"),
            _ => s.push(c),
        }
    }
    s
}

/// Rendert das Output-Manifest als JSON (ohne serde — die einzige
/// Datenquelle sind Strings + Backend-Namen). Schema:
/// `{ idl, compiler_version, stamp, depfile, inputs[], backends[{backend,
/// output_dir, files[]}], outputs[] }`.
fn render_manifest(
    idl_path: &str,
    opts: &CliOptions,
    deps: &[String],
    entries: &[BackendOutput],
    all_outputs: &[String],
) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"idl\": {},\n", json_string(idl_path)));
    s.push_str(&format!(
        "  \"compiler_version\": {},\n",
        json_string(VERSION)
    ));
    s.push_str(&format!(
        "  \"stamp\": {},\n",
        json_opt_path(opts.stamp.as_deref())
    ));
    s.push_str(&format!(
        "  \"depfile\": {},\n",
        json_opt_path(opts.depfile.as_deref())
    ));
    s.push_str("  \"inputs\": ");
    s.push_str(&json_string_array(deps));
    s.push_str(",\n");
    s.push_str("  \"backends\": [\n");
    for (idx, e) in entries.iter().enumerate() {
        let files: Vec<String> = e
            .files
            .iter()
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect();
        s.push_str("    {\n");
        s.push_str(&format!(
            "      \"backend\": {},\n",
            json_string(backend_flag_suffix(e.backend))
        ));
        s.push_str(&format!(
            "      \"output_dir\": {},\n",
            json_string(&e.out_dir.to_string_lossy())
        ));
        s.push_str("      \"files\": ");
        s.push_str(&json_string_array(&files));
        s.push('\n');
        s.push_str("    }");
        if idx + 1 < entries.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ],\n");
    s.push_str("  \"outputs\": ");
    s.push_str(&json_string_array(all_outputs));
    s.push('\n');
    s.push_str("}\n");
    s
}

/// JSON-Array aus Strings, eingerueckt auf einer Zeile.
fn json_string_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let inner = items
        .iter()
        .map(|s| json_string(s))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}

/// Optionaler Pfad als JSON-String oder `null`.
fn json_opt_path(p: Option<&Path>) -> String {
    match p {
        Some(path) => json_string(&path.to_string_lossy()),
        None => "null".to_string(),
    }
}

/// Minimaler JSON-String-Encoder (Quotes + Standard-Escapes).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `--dump-typeobject`: alle benannten Typen einer Spec ueber den
/// zweistufigen Registry-Mapper lowern und in topologischer Reihenfolge
/// (Abhaengigkeiten zuerst) ausgeben. Scoped Cross-Referenzen werden
/// aufgeloest. Schlaegt das Mapping fehl (z.B. zyklischer Typ), wird die
/// Mapper-Fehlermeldung ausgegeben.
/// Dumps every successfully-lowered minimal `TypeObject` to stdout and — this
/// is the error contract — fails (non-zero exit, message on stderr) when the
/// mapping was not total. A type that cannot be lowered (a recursive cycle, an
/// unsupported construct, or a dependent of either) is isolated per SCC by
/// `build_type_registry`: the lowerable types are still printed, but their
/// presence must not mask that others were dropped. Previously this printed
/// `// TypeObject mapping failed` to stdout and returned Ok, so a consumer saw
/// exit 0 ("success") despite the failure.
fn dump_typeobjects(spec: &zerodds_idl::ast::Specification) -> Result<(), CliError> {
    match zerodds_idl::semantics::build_type_registry(spec) {
        Ok(lowered) => {
            for (fqn, obj) in lowered.iter() {
                println!("// TypeObject for {fqn}\n{obj:#?}\n");
            }
            if lowered.skipped.is_empty() {
                return Ok(());
            }
            let detail = lowered
                .skipped
                .iter()
                .map(|(fqn, e)| format!("  {fqn}: {e:?}"))
                .collect::<Vec<_>>()
                .join("\n");
            Err(CliError::NotImplemented(format!(
                "TypeObject mapping incomplete — {} type(s) could not be lowered:\n{detail}",
                lowered.skipped.len()
            )))
        }
        Err(e) => Err(CliError::NotImplemented(format!(
            "TypeObject mapping failed: {e:?}"
        ))),
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
        Backend::Go => "Go",
        Backend::Ada => "Ada",
        Backend::Zig => "Zig",
        Backend::Nim => "Nim",
        Backend::D => "D",
        Backend::Elixir => "Elixir",
        Backend::OCaml => "OCaml",
        Backend::Julia => "Julia",
        Backend::Lua => "Lua",
        Backend::Swift => "Swift",
    }
}

/// Splices the C TypeObject `block` into `header` immediately before the
/// header's closing include-guard `#endif`, so the block sits inside the
/// guard. `c_mode.rs` closes the guard with a trailing `#endif` line; appending
/// the block after that line placed the `static const` arrays outside the
/// guard, so a double `#include` of the header re-emitted them → clang
/// redefinition. The insertion point is the last line that is an `#endif`
/// directive (the guard closer). If `block` is empty the header is returned
/// unchanged; if no `#endif` is found (never for a well-formed C header) the
/// block is appended as a safe fallback.
fn splice_c_typeobject_block(header: &str, block: &str) -> String {
    if block.is_empty() {
        return header.to_string();
    }
    // Byte offset of the last `#endif` directive line.
    let mut insert_at: Option<usize> = None;
    let mut offset = 0usize;
    for line in header.split_inclusive('\n') {
        if line.trim_start().starts_with("#endif") {
            insert_at = Some(offset);
        }
        offset += line.len();
    }
    match insert_at {
        Some(idx) => {
            let mut out = String::with_capacity(header.len() + block.len() + 1);
            out.push_str(&header[..idx]);
            out.push_str(block);
            if !block.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&header[idx..]);
            out
        }
        None => {
            let mut out = String::with_capacity(header.len() + block.len());
            out.push_str(header);
            out.push_str(block);
            out
        }
    }
}

/// Eine im Speicher gehaltene, erzeugte Backend-Datei (Pfad + Inhalt). Nichts
/// wird auf die Platte geschrieben, bevor ALLE Backends ihren Output erzeugt
/// haben — der Flush ist der eine atomare Veroeffentlichungs-Punkt. Faellt ein
/// spaeteres Backend aus (oder das Semantik-Gate), bleibt keine Teil-Ausgabe
/// eines frueheren Backends liegen.
struct StagedFile {
    path: PathBuf,
    contents: Vec<u8>,
    /// `false` fuer eine bereits existierende Scaffold-Datei, die unveraendert
    /// bleibt: im Manifest gefuehrt, aber nicht (neu) geschrieben.
    write: bool,
}

/// Staged eine erzeugte Backend-Datei im Speicher (Pfad + Inhalt) und
/// protokolliert sie im Manifest-Sammler `files`. Der eigentliche Platten-
/// Schreib passiert gebuendelt in [`flush_staged_files`] NACH dem Erfolg
/// aller Backends (atomares `--all`). Zentraler Sammel-Pfad, damit jede vom
/// Codegen erzeugte Datei ohne Endungs-Raten im Build-Vertrag auftaucht.
fn stage_file(path: &Path, contents: impl AsRef<[u8]>, files: &mut Vec<StagedFile>) {
    files.push(StagedFile {
        path: path.to_path_buf(),
        contents: contents.as_ref().to_vec(),
        write: true,
    });
}

/// Schreibt alle gestageten Dateien auf die Platte — der atomare
/// Veroeffentlichungs-Punkt. Legt fehlende Elternverzeichnisse an.
/// Scaffold-Dateien mit `write == false` (bereits vorhanden) werden
/// uebersprungen.
fn flush_staged_files(entries: &[BackendOutput]) -> Result<(), CliError> {
    for entry in entries {
        for f in &entry.files {
            if !f.write {
                continue;
            }
            if let Some(parent) = f.path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        CliError::Io(format!("cannot create {}: {e}", parent.display()))
                    })?;
                }
            }
            std::fs::write(&f.path, &f.contents)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", f.path.display())))?;
        }
    }
    Ok(())
}

/// Emittiert ein einzelnes Backend in `out_dir`. `type_objects` ist der
/// XTypes-TypeObject-Block, der an die generierte Ausgabe angehaengt
/// wird (leer = `--no-typeobject` oder Mapping fehlgeschlagen). Rueckgabe:
/// die Liste der wirklich geschriebenen Dateien (fuer den Build-Vertrag) —
/// kann leer sein, wenn ein Backend fuer die IDL keinen Output erzeugt.
fn emit_backend(
    backend: Backend,
    ast: &zerodds_idl::ast::Specification,
    opts: &CliOptions,
    out_dir: &Path,
    base: &str,
    type_objects: &[TypeObjectBlob],
) -> Result<Vec<StagedFile>, CliError> {
    let mut files: Vec<StagedFile> = Vec::new();
    match backend {
        Backend::C => {
            let code = generate_c_header(ast, &CGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("c codegen failed: {e}")))?;
            // The C header wraps its body in an include guard (`#ifndef …_H` /
            // closing `#endif`, emitted by c_mode.rs). The TypeObject block
            // must live INSIDE that guard: appended after the closing `#endif`
            // it lands outside the guard, so a second `#include` of the same
            // header re-emits the `static const` arrays → clang redefinition.
            // Splice it before the guard's closing `#endif` instead. C++ uses
            // `#pragma once` and the other single-file backends carry no guard,
            // so this fix-up is C-only.
            let block = typeobject_emit::render(backend, ast, base, type_objects);
            let code = splice_c_typeobject_block(&code, &block);
            let out_path = out_dir.join(format!("{base}.h"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Rust => {
            // DataTypes immer.
            let mut code = generate_rust_module(ast, &RustGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("rust codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, ast, base, type_objects));
            let out_path = out_dir.join(format!("{base}.rs"));
            stage_file(&out_path, code, &mut files);
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
                stage_file(&svc_path, svc, &mut files);
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
            code.push_str(&typeobject_emit::render(backend, ast, base, type_objects));
            let out_path = out_dir.join(format!("{base}.hpp"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Ts => {
            let mut code = generate_ts_source(ast)
                .map_err(|e| CliError::NotImplemented(format!("ts codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, ast, base, type_objects));
            let out_path = out_dir.join(format!("{base}.ts"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::CSharp => {
            let cs_opts = CsGenOptions::default();
            let mut code = if opts.corba {
                generate_csharp_with_corba_traits(ast, &cs_opts)
            } else {
                generate_csharp(ast, &cs_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("csharp codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, ast, base, type_objects));
            let out_path = out_dir.join(format!("{base}.cs"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Java => {
            let java_opts = JavaGenOptions::default();
            let java_files = if opts.corba {
                generate_java_files_with_corba_traits(ast, &java_opts)
            } else {
                generate_java_files(ast, &java_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("java codegen failed: {e}")))?;
            for file in java_files {
                let pkg_subpath = file.package_path.replace('.', "/");
                let pkg_dir = if pkg_subpath.is_empty() {
                    out_dir.to_path_buf()
                } else {
                    out_dir.join(&pkg_subpath)
                };
                // Das Package-Verzeichnis wird erst beim atomaren Flush
                // angelegt (flush_staged_files legt Elternverzeichnisse an) —
                // ein Fehler in einem spaeteren Backend hinterlaesst so keine
                // leeren Package-Ordner.
                let class_path = pkg_dir.join(format!("{}.java", file.class_name));
                stage_file(&class_path, &file.source, &mut files);
            }
            // Java ist Multi-File — der TypeObject-Block kommt als
            // eigenstaendige Compilation-Unit. Klasse + Dateiname sind pro
            // Basis-IDL eindeutig (`TypeObjects_<base>`), damit mehrere
            // separat generierte IDLs im selben Package nicht als
            // Doppel-Klasse kollidieren (Java bindet Dateiname an
            // public-class-Name).
            if !type_objects.is_empty() {
                let class = typeobject_emit::type_objects_class_name(ast, base);
                let to_path = out_dir.join(format!("{class}.java"));
                stage_file(
                    &to_path,
                    typeobject_emit::render(backend, ast, base, type_objects),
                    &mut files,
                );
            }
        }
        Backend::Python => {
            let mut code = generate_python_module(ast, &PythonGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("python codegen failed: {e}")))?;
            code.push_str(&typeobject_emit::render(backend, ast, base, type_objects));
            let out_path = out_dir.join(format!("{base}.py"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Go => {
            let code = generate_go_module(ast, &GoGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("go codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.go"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Ada => {
            // GNAT maps the unit name to lower-case file names; `base` is the
            // unit name (mixed-case is fine, GNAT is case-insensitive).
            let opts_ada = AdaGenOptions {
                package_name: base.to_string(),
            };
            let module = generate_ada_module(ast, &opts_ada)
                .map_err(|e| CliError::NotImplemented(format!("ada codegen failed: {e}")))?;
            let lower = base.to_lowercase();
            for (ext, code) in [("ads", &module.spec), ("adb", &module.body)] {
                let out_path = out_dir.join(format!("{lower}.{ext}"));
                stage_file(&out_path, code, &mut files);
            }
        }
        Backend::Zig => {
            let code = generate_zig_module(ast, &ZigGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("zig codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.zig"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Nim => {
            let code = generate_nim_module(ast, &NimGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("nim codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.nim"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::D => {
            let code = generate_d_module(ast, &DGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("d codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.d"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Elixir => {
            let module = base
                .split(['.', '_', '-'])
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let mut c = p.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<String>();
            let opts_ex = ElixirGenOptions {
                module_name: if module.is_empty() {
                    "Zdgen".to_string()
                } else {
                    module
                },
            };
            let code = generate_elixir_module(ast, &opts_ex)
                .map_err(|e| CliError::NotImplemented(format!("elixir codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.ex"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::OCaml => {
            let code = generate_ocaml_module(ast, &OcamlGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("ocaml codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.ml"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Julia => {
            let code = generate_julia_module(ast, &JuliaGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("julia codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.jl"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Lua => {
            let code = generate_lua_module(ast, &LuaGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("lua codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.lua"));
            stage_file(&out_path, code, &mut files);
        }
        Backend::Swift => {
            let code = generate_swift_module(ast, &SwiftGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("swift codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.swift"));
            stage_file(&out_path, code, &mut files);
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
                // Bereits vorhandene Build-Datei ist trotzdem Teil des
                // Projekt-Outputs → im Manifest fuehren, aber NICHT
                // ueberschreiben (write == false).
                files.push(StagedFile {
                    path,
                    contents: Vec::new(),
                    write: false,
                });
            } else {
                stage_file(&path, content, &mut files);
                opts.report(1, &format!("scaffold: staged {name}"));
            }
        }
    }
    Ok(files)
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
         IDL4 compiler (alle 17 Backends)\n\n\
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
         \x20   --all              Alle 17 Backends in einem Lauf\n\
         \x20   --default-extensibility KIND  Extensibility fuer un-annotierte\n\
         \x20                      Typen: final|appendable|mutable (auch -de)\n\
         \x20   --default-nested true|false   Un-annotierte Typen @nested\n\
         \x20   --no-typeobject    XTypes-TypeObject-Emission abschalten\n\
         \x20                      (default an, wie RTI/Fast/Cyclone)\n\
         \x20   --scaffold         Build-Datei je Backend mit-emittieren\n\
         \x20                      (Cargo.toml/CMakeLists.txt/pom.xml/…)\n\
         \x20   --depfile FILE     Make-/Ninja-Depfile schreiben: Target =\n\
         \x20                      Stamp (falls gesetzt) sonst alle Outputs,\n\
         \x20                      Prereqs = IDL + alle transitiven #includes\n\
         \x20   --output-manifest FILE  JSON aller erzeugten Dateien je\n\
         \x20                      Backend (Alias --manifest); kein Endungs-Raten\n\
         \x20   --stamp FILE       Stabile Stamp-Datei als einziger OUTPUT\n\
         \x20                      (touch nach erfolgreichem Codegen)\n\
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
    fn multi_backend_partial_failure_leaves_no_files() {
        // Atomares `--all` (P1): staged ein Backend erfolgreich und ein
        // spaeteres Backend faellt aus, darf KEINE Datei des ersten Backends
        // liegenbleiben. `long double` wird vom TS-Backend akzeptiert, vom
        // C-Backend abgelehnt (C5.1-a Foundation scope) — Reihenfolge
        // `--ts --c`: TS staged, C schlaegt fehl, Flush laeuft nie.
        let work = unique_workdir("atomic-partial");
        let idl_path = work.join("t.idl");
        std::fs::write(&idl_path, "struct S { long double d; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--ts".to_string(),
            "--c".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(
            matches!(result, Err(CliError::NotImplemented(_))),
            "expected the C backend to fail, got {result:?}"
        );

        // Der TS-Output (t.ts) des zuvor erfolgreich gestageten Backends darf
        // nicht existieren — und generell keine Datei im Zielverzeichnis.
        assert!(
            !out_dir.join("t.ts").exists(),
            "TS file must not be left behind after the C backend failed"
        );
        let leftover: Vec<_> = std::fs::read_dir(&out_dir)
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        assert!(
            leftover.is_empty(),
            "no files may be published on partial failure, found: {leftover:?}"
        );

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn multi_backend_all_success_publishes_every_file() {
        // Gegenprobe: gelingt jedes Backend, werden alle Dateien atomar
        // veroeffentlicht.
        let work = unique_workdir("atomic-success");
        let idl_path = work.join("t.idl");
        std::fs::write(&idl_path, "struct S { long id; };").expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--ts".to_string(),
            "--c".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert!(out_dir.join("t.ts").exists(), "TS file must be published");
        assert!(out_dir.join("t.h").exists(), "C header must be published");

        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn splice_puts_block_before_closing_endif() {
        let header =
            "#ifndef FOO_H\n#define FOO_H\nint x;\n#ifdef __cplusplus\n}\n#endif\n#endif\n";
        let block = "\n/* TO */\nstatic const unsigned char t[] = {0};\n";
        let spliced = splice_c_typeobject_block(header, block);
        // Block is present and lies before the final guard-closing `#endif`.
        let block_pos = spliced
            .find("static const unsigned char t[]")
            .expect("block present");
        let last_endif = spliced.rfind("#endif").expect("guard endif present");
        assert!(
            block_pos < last_endif,
            "typeobject block must precede the closing #endif:\n{spliced}"
        );
        // The guard closer survives (still ends the file inside the guard).
        assert!(spliced.trim_end().ends_with("#endif"));
    }

    #[test]
    fn splice_empty_block_is_identity() {
        let header = "#ifndef FOO_H\n#define FOO_H\nint x;\n#endif\n";
        assert_eq!(splice_c_typeobject_block(header, ""), header);
    }

    #[test]
    fn splice_without_endif_appends() {
        let header = "int x;\n";
        let block = "static const unsigned char t[] = {0};\n";
        let spliced = splice_c_typeobject_block(header, block);
        assert!(spliced.starts_with("int x;"));
        assert!(spliced.contains("static const unsigned char t[]"));
    }

    #[test]
    fn run_c_backend_keeps_typeobject_inside_include_guard() {
        let work = unique_workdir("c-typeobject-guard");
        let idl_path = work.join("keyed.idl");
        // A @key member triggers a TypeObject block in the post-pass.
        std::fs::write(&idl_path, "@final struct Keyed { @key long id; long v; };")
            .expect("write idl");
        let out_dir = work.join("out");

        let result = run(&[
            "--c".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let generated =
            std::fs::read_to_string(out_dir.join("keyed.h")).expect("read generated c header");
        let block_pos = generated
            .find("_type_object[]")
            .expect("typeobject block should be emitted for a @key type");
        let last_endif = generated
            .rfind("#endif")
            .expect("include-guard #endif present");
        assert!(
            block_pos < last_endif,
            "typeobject block must sit inside the include guard, got:\n{generated}"
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
            generated.contains("@idl_struct(typename=\"Greeting\""),
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

    /// Multi-File-Compose-Regression: zwei getrennt generierte IDLs, die im
    /// selben Namespace/Package zusammengefuehrt werden, duerfen keinen
    /// zweiten TypeObject-Holder gleichen Namens deklarieren
    /// (C# CS0101 / Java Doppel-Klasse). Der Holder ist pro Basis-IDL
    /// eindeutig (`TypeObjects_<base>`). Statische Invariante — der echte
    /// csc/javac-Compose-Compile ist gated auf codepit / lokaler Toolchain.
    #[test]
    fn compose_two_idls_yield_unique_typeobject_holders() {
        let work = unique_workdir("compose-to");
        let common = work.join("common.idl");
        let metrics = work.join("metrics.idl");
        std::fs::write(
            &common,
            "module common { struct KeyLabel { @key uint32 id; string label; }; };",
        )
        .expect("write common.idl");
        std::fs::write(
            &metrics,
            "module metrics { struct SystemCpuInfo { @key string id; string brand; \
             string name; float usage; }; };",
        )
        .expect("write metrics.idl");

        // C#: beide in DASSELBE Out-Dir (= ein Projekt/Namespace).
        let cs = work.join("cs");
        for idl in [&common, &metrics] {
            let r = run(&[
                "--csharp".to_string(),
                "-o".to_string(),
                cs.to_string_lossy().to_string(),
                idl.to_string_lossy().to_string(),
            ]);
            assert!(r.is_ok(), "csharp gen failed: {r:?}");
        }
        let cs_common = std::fs::read_to_string(cs.join("common.cs")).expect("read common.cs");
        let cs_metrics = std::fs::read_to_string(cs.join("metrics.cs")).expect("read metrics.cs");
        assert!(cs_common.contains("public static class TypeObjects_common"));
        assert!(cs_metrics.contains("public static class TypeObjects_metrics"));
        // Kein bare `class TypeObjects` (ohne per-Base-Suffix) => kein CS0101.
        assert!(!cs_common.contains("class TypeObjects\n"));
        assert!(!cs_metrics.contains("class TypeObjects\n"));
        assert!(!cs_metrics.contains("class TypeObjects_common"));

        // Java: beide ins selbe Out-Dir. Datei + Klasse pro Basis eindeutig.
        let java = work.join("java");
        for idl in [&common, &metrics] {
            let r = run(&[
                "--java".to_string(),
                "-o".to_string(),
                java.to_string_lossy().to_string(),
                idl.to_string_lossy().to_string(),
            ]);
            assert!(r.is_ok(), "java gen failed: {r:?}");
        }
        let jc = std::fs::read_to_string(java.join("TypeObjects_common.java"))
            .expect("TypeObjects_common.java must exist");
        let jm = std::fs::read_to_string(java.join("TypeObjects_metrics.java"))
            .expect("TypeObjects_metrics.java must exist");
        assert!(jc.contains("public final class TypeObjects_common"));
        assert!(jm.contains("public final class TypeObjects_metrics"));
        // Die alte feste `TypeObjects.java` darf nicht mehr entstehen.
        assert!(
            !java.join("TypeObjects.java").exists(),
            "fixed-name TypeObjects.java would collide on multi-file compose"
        );

        // Rust (GH #25): das TypeObject-Modul muss pro Basis eindeutig sein
        // (`type_objects_common`/`type_objects_metrics`), sonst E0428
        // "type_objects defined multiple times" beim `include!` beider Dateien
        // in ein gemeinsames Eltern-Modul. Der frühere Fix f2341dae behob nur
        // die doppelten `use`-Imports, nicht den `pub mod type_objects`-Wrapper.
        let rust = work.join("rust");
        for idl in [&common, &metrics] {
            let r = run(&[
                "--rust".to_string(),
                "-o".to_string(),
                rust.to_string_lossy().to_string(),
                idl.to_string_lossy().to_string(),
            ]);
            assert!(r.is_ok(), "rust gen failed: {r:?}");
        }
        let rc = std::fs::read_to_string(rust.join("common.rs")).expect("read common.rs");
        let rm = std::fs::read_to_string(rust.join("metrics.rs")).expect("read metrics.rs");
        assert!(rc.contains("pub mod type_objects_common"));
        assert!(rm.contains("pub mod type_objects_metrics"));
        // Kein bare `pub mod type_objects {` (ohne per-Base-Suffix) => kein E0428.
        assert!(!rc.contains("pub mod type_objects {"));
        assert!(!rm.contains("pub mod type_objects {"));

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

    // ---- Injective naming regressions (compose-naming-robustness spec §1) ----
    //
    // These drive the FULL CLI path (`run`), so the TypeObject post-pass that
    // appends the const block runs — unlike the emitter-direct W3 gate, which
    // never sees it. The core bug: `A::B_C` and `A_B::C` both flattened to
    // `A_B_C`, so the two TypeObject consts collided (Rust E0428, C# CS0102,
    // TS2451, javac redef; Python silently overwrote the first).

    /// Runs `run` for `backend` on `idl`, returns the single generated file's
    /// text. `out_name` is the emitted filename (`<base>.<ext>`).
    fn gen_via_cli(label: &str, idl: &str, backend: &str, base: &str, out_name: &str) -> String {
        let work = unique_workdir(label);
        let idl_path = work.join(format!("{base}.idl"));
        std::fs::write(&idl_path, idl).expect("write idl");
        let out_dir = work.join("out");
        let result = run(&[
            backend.to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "{backend} generate failed: {result:?}");
        let text = std::fs::read_to_string(out_dir.join(out_name))
            .unwrap_or_else(|e| panic!("read {out_name}: {e}"));
        std::fs::remove_dir_all(&work).ok();
        text
    }

    /// `A::B_C` and `A_B::C` in one IDL must yield DISTINCT TypeObject const
    /// names in every wired post-pass backend (was a hard compile error).
    #[test]
    fn scope_vs_underscore_yield_distinct_typeobject_consts() {
        // `module A { struct B_C {..}; }` and top-level `struct A_B { .. }`
        // referencing an inner `C` — two FQNs `A::B_C` and `A_B::C`.
        let idl = "\
            module A { @final struct B_C { long v; }; }; \
            module A_B { @final struct C { long w; }; };";
        // Injective encoding: A::B_C -> A_sB_uC, A_B::C -> A_uB_sC.
        for (backend, base, out, a, b) in [
            ("--rust", "n", "n.rs", "A_sB_uC", "A_uB_sC"),
            ("--cpp", "n", "n.hpp", "A_sB_uC", "A_uB_sC"),
            ("--c", "n", "n.h", "A_sB_uC", "A_uB_sC"),
            ("--csharp", "n", "n.cs", "A_sB_uC", "A_uB_sC"),
            ("--python", "n", "n.py", "A_sB_uC", "A_uB_sC"),
            ("--ts", "n", "n.ts", "A_sB_uC", "A_uB_sC"),
        ] {
            let text = gen_via_cli("scope-vs-us", idl, backend, base, out);
            assert!(text.contains(a), "{backend}: missing {a} in:\n{text}");
            assert!(text.contains(b), "{backend}: missing {b} in:\n{text}");
            assert_ne!(a, b);
        }
        // Java writes the holder as its own compilation unit.
        let java = gen_via_cli("scope-vs-us-java", idl, "--java", "n", "TypeObjects_n.java");
        assert!(java.contains("A_sB_uC"), "java missing A_sB_uC:\n{java}");
        assert!(java.contains("A_uB_sC"), "java missing A_uB_sC:\n{java}");
    }

    /// Two files `a-b.idl` and `a_b.idl` must yield DISTINCT Rust holder
    /// modules (`type_objects_a_hb` vs `type_objects_a_ub`) so composing both
    /// via `include!` into one parent module does not raise E0428 (#25).
    #[test]
    fn hyphen_vs_underscore_basename_yield_distinct_holders() {
        let idl = "@final struct S { long x; };";
        let hyphen = gen_via_cli("base-hyphen", idl, "--rust", "a-b", "a-b.rs");
        let under = gen_via_cli("base-under", idl, "--rust", "a_b", "a_b.rs");
        assert!(
            hyphen.contains("pub mod type_objects_a_hb"),
            "hyphen holder:\n{hyphen}"
        );
        assert!(
            under.contains("pub mod type_objects_a_ub"),
            "underscore holder:\n{under}"
        );
        // The old lossy sanitiser folded both to `type_objects_a_b`.
        assert!(!hyphen.contains("pub mod type_objects_a_b "));
        assert!(!under.contains("pub mod type_objects_a_hb"));
    }

    /// A user symbol equal to the synthetic holder must NOT be overwritten:
    /// the holder is disambiguated instead.
    #[test]
    fn user_symbol_colliding_with_holder_is_not_overwritten() {
        // Rust: user `module type_objects_input` vs holder for base `input`.
        let ridl = "\
            module type_objects_input { @final struct Marker { long x; }; }; \
            @final struct Payload { long y; };";
        let rust = gen_via_cli("holder-rust", ridl, "--rust", "input", "input.rs");
        // The user module survives verbatim …
        assert!(
            rust.contains("pub mod type_objects_input"),
            "user module lost:\n{rust}"
        );
        // … and the holder took a disambiguated name (never the user's).
        assert!(
            rust.contains("pub mod type_objects_input_x"),
            "holder not disambiguated:\n{rust}"
        );

        // Java: user `struct TypeObjects_input` vs holder for base `input`.
        let jidl = "\
            @final struct TypeObjects_input { long x; }; \
            @final struct Payload { long y; };";
        let work = unique_workdir("holder-java");
        let idl_path = work.join("input.idl");
        std::fs::write(&idl_path, jidl).expect("write idl");
        let out_dir = work.join("out");
        run(&[
            "--java".to_string(),
            "-o".to_string(),
            out_dir.to_string_lossy().to_string(),
            idl_path.to_string_lossy().to_string(),
        ])
        .expect("java generate");
        // The user class file must exist and be untouched by the holder.
        let user = std::fs::read_to_string(out_dir.join("TypeObjects_input.java"))
            .expect("user class file present");
        assert!(
            user.contains("class TypeObjects_input"),
            "user class overwritten:\n{user}"
        );
        assert!(
            !user.contains("Minimal TypeObject"),
            "holder overwrote user file:\n{user}"
        );
        // The holder went to a disambiguated file.
        let disambig = out_dir.join("TypeObjects_input_x.java");
        assert!(disambig.exists(), "holder file not disambiguated");
        std::fs::remove_dir_all(&work).ok();
    }

    // ---- Build-Vertrag: --depfile / --output-manifest / --stamp ----

    /// Sammelt rekursiv alle Dateien unter `dir` (absolute Pfade).
    fn collect_files_recursive(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in rd.filter_map(Result::ok) {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
        out
    }

    /// Escaping-Einheitstest fuer die Depfile-Pfade.
    #[test]
    fn escape_make_path_escapes_space_hash_dollar() {
        assert_eq!(escape_make_path("a b.idl"), "a\\ b.idl");
        assert_eq!(escape_make_path("a#b.idl"), "a\\#b.idl");
        assert_eq!(escape_make_path("a$b.idl"), "a$$b.idl");
        assert_eq!(escape_make_path("plain.idl"), "plain.idl");
    }

    /// JSON-Encoder: Quotes/Backslash/Steuerzeichen.
    #[test]
    fn json_string_escapes_specials() {
        assert_eq!(json_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_string("a\nb"), "\"a\\nb\"");
    }

    /// Depfile listet die kanonische `common.idl` als Prerequisite, das
    /// Manifest die real erzeugten Java-Dateien (mehrere .java +
    /// TypeObjects_<base>.java, nichts geraten), der Stamp existiert.
    #[test]
    fn build_contract_depfile_manifest_stamp_for_include() {
        let work = unique_workdir("bc-include");
        let inc = work.join("inc");
        std::fs::create_dir_all(&inc).expect("mkdir inc");
        std::fs::write(
            inc.join("common.idl"),
            "module common { @final struct Vec3 { @key long id; double x; }; };",
        )
        .expect("write common");
        let idl = work.join("robot.idl");
        std::fs::write(
            &idl,
            "#include \"common.idl\"\n\
             module robot { @final struct Pose { common::Vec3 p; }; };",
        )
        .expect("write robot");
        let out = work.join("out");
        let depfile = work.join("robot.d");
        let manifest = work.join("robot.manifest.json");
        let stamp = work.join("robot.stamp");

        let result = run(&[
            "--java".to_string(),
            "-I".to_string(),
            inc.to_string_lossy().to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            "--depfile".to_string(),
            depfile.to_string_lossy().to_string(),
            "--output-manifest".to_string(),
            manifest.to_string_lossy().to_string(),
            "--stamp".to_string(),
            stamp.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Stamp existiert nach Erfolg.
        assert!(stamp.exists(), "stamp not created");

        // Depfile: Target = Stamp, Prereq = common.idl (kanonisch).
        let dep = std::fs::read_to_string(&depfile).expect("read depfile");
        let canon_common = std::fs::canonicalize(inc.join("common.idl"))
            .expect("canonicalize common")
            .to_string_lossy()
            .into_owned();
        assert!(
            dep.contains(&escape_make_path(&canon_common)),
            "depfile must list canonical common.idl as prerequisite:\n{dep}"
        );
        let stamp_str = stamp.to_string_lossy().into_owned();
        assert!(
            dep.starts_with(&escape_make_path(&stamp_str)),
            "depfile target must be the stamp:\n{dep}"
        );

        // Manifest: die real erzeugten Java-Dateien (kein geratenes
        // Module.java), inkl. TypeObjects_robot.java.
        let man = std::fs::read_to_string(&manifest).expect("read manifest");
        assert!(man.contains("\"backend\": \"java\""), "manifest:\n{man}");
        assert!(
            man.contains("TypeObjects_robot.java"),
            "manifest must list the real TypeObjects holder:\n{man}"
        );
        // Jede real geschriebene Datei muss im Manifest stehen.
        let written = collect_files_recursive(&out);
        assert!(!written.is_empty(), "no java files written");
        for f in &written {
            let s = f.to_string_lossy().into_owned();
            assert!(
                man.contains(&s),
                "manifest missing generated file {s}:\n{man}"
            );
        }
        // Kein geratener Module.java-Name.
        assert!(!man.contains("robot/Module.java"), "guessed name leaked");

        std::fs::remove_dir_all(&work).ok();
    }

    /// Manifest-Inhalt = exakt die geschriebenen Dateien (kein Raten):
    /// die `outputs`-Liste deckt sich mit `ls` des out-dir.
    #[test]
    fn build_contract_manifest_matches_ls_of_outdir() {
        let work = unique_workdir("bc-exact");
        let idl = work.join("chat.idl");
        std::fs::write(&idl, "@final struct Greeting { @key long id; };").expect("write idl");
        let out = work.join("out");
        let manifest = work.join("chat.manifest.json");

        let result = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            "--output-manifest".to_string(),
            manifest.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let man = std::fs::read_to_string(&manifest).expect("read manifest");
        let on_disk = collect_files_recursive(&out);
        // Rust ist Single-File: genau eine Datei auf Platte.
        assert_eq!(
            on_disk.len(),
            1,
            "expected exactly chat.rs, got {on_disk:?}"
        );
        let path_str = on_disk[0].to_string_lossy().into_owned();
        // Die outputs-Liste enthaelt genau diese eine Datei.
        assert!(
            man.contains(&format!("\"outputs\": [{}]", json_string(&path_str))),
            "outputs array must equal exactly the ls of out-dir:\n{man}"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    /// Ein Backend ohne Output bildet sich ehrlich mit leerer Dateiliste ab
    /// (Manifest-Struktur), statt still Exit 0 ohne Spur. Hier ueber ein
    /// zweites Backend im selben Lauf geprueft: jeder Backend-Eintrag traegt
    /// seine eigene files-Liste.
    #[test]
    fn build_contract_manifest_has_per_backend_file_lists() {
        let work = unique_workdir("bc-perbackend");
        let idl = work.join("t.idl");
        std::fs::write(&idl, "@final struct T { @key long a; };").expect("write idl");
        let out = work.join("out");
        let manifest = work.join("t.manifest.json");
        let result = run(&[
            "--rust".to_string(),
            "--python".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            "--output-manifest".to_string(),
            manifest.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(result.is_ok(), "got {result:?}");
        let man = std::fs::read_to_string(&manifest).expect("read manifest");
        assert!(man.contains("\"backend\": \"rust\""), "{man}");
        assert!(man.contains("\"backend\": \"python\""), "{man}");
        assert!(man.contains("t.rs"), "{man}");
        assert!(man.contains("t.py"), "{man}");
        std::fs::remove_dir_all(&work).ok();
    }

    /// Bei einem semantischen Fehler entsteht weder Stamp noch Manifest noch
    /// Depfile, und der Exit-Code ist != 0 (Semantic → 1).
    #[test]
    fn build_contract_no_artifacts_on_semantic_error() {
        let work = unique_workdir("bc-semerr");
        let idl = work.join("bad.idl");
        // Unaufloesbarer Typ-Verweis → Semantik-Gate schlaegt fehl.
        std::fs::write(&idl, "@final struct Bad { DoesNotExist m; };").expect("write idl");
        let out = work.join("out");
        let depfile = work.join("bad.d");
        let manifest = work.join("bad.manifest.json");
        let stamp = work.join("bad.stamp");

        let result = run(&[
            "--rust".to_string(),
            "-o".to_string(),
            out.to_string_lossy().to_string(),
            "--depfile".to_string(),
            depfile.to_string_lossy().to_string(),
            "--output-manifest".to_string(),
            manifest.to_string_lossy().to_string(),
            "--stamp".to_string(),
            stamp.to_string_lossy().to_string(),
            idl.to_string_lossy().to_string(),
        ]);
        assert!(
            matches!(result, Err(CliError::Semantic(_))),
            "expected Semantic error, got {result:?}"
        );
        assert_eq!(CliError::Semantic("x".into()).exit_code(), 1);
        assert!(!stamp.exists(), "stamp must not exist on semantic error");
        assert!(
            !manifest.exists(),
            "manifest must not exist on semantic error"
        );
        assert!(
            !depfile.exists(),
            "depfile must not exist on semantic error"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    /// Build-Vertrag-Flags ohne Backend (nicht-emittierender Modus) → Usage.
    #[test]
    fn build_contract_flags_require_generate_mode() {
        let work = unique_workdir("bc-check");
        let idl = work.join("c.idl");
        std::fs::write(&idl, "@final struct C { long n; };").expect("write idl");
        let result = run(&[
            "check".to_string(),
            idl.to_string_lossy().to_string(),
            "--stamp".to_string(),
            work.join("c.stamp").to_string_lossy().to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))), "got {result:?}");
        std::fs::remove_dir_all(&work).ok();
    }
}
