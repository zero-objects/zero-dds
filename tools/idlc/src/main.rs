// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 compiler: backends for C, C++, C#, Java, Python, Rust (T7.1).
//!
//! Phase 1: alle sieben Backends verdrahtet — `--c`, `--cpp`, `--rust`,
//! `--ts`, `--csharp`, `--java`, `--python`.
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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::{parse, parse_with_deltas};
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
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zerodds-idlc: {e}");
            ExitCode::from(e.exit_code())
        }
    }
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

#[derive(Default)]
struct CliOptions {
    parse_only: bool,
    rti: bool,
    corba: bool,
    backend: Option<Backend>,
    output: Option<PathBuf>,
    file: Option<String>,
}

fn run(args: &[String]) -> Result<(), CliError> {
    let mut opts = CliOptions::default();
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
            "--rti" => opts.rti = true,
            "--corba" => opts.corba = true,
            "--c" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::C);
            }
            "--rust" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::Rust);
            }
            "--cpp" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::Cpp);
            }
            "--ts" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::Ts);
            }
            "--csharp" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::CSharp);
            }
            "--java" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::Java);
            }
            "--python" => {
                if opts.backend.is_some() {
                    return Err(CliError::Usage(
                        "multiple backends selected; pick one of --c, --cpp, --rust, --ts, --csharp, --java, --python"
                            .to_string(),
                    ));
                }
                opts.backend = Some(Backend::Python);
            }
            "-o" | "--output" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("{arg} requires a directory")))?;
                opts.output = Some(PathBuf::from(value));
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
        match opts.backend {
            Some(Backend::Cpp | Backend::CSharp | Backend::Java | Backend::Rust) => {}
            Some(other) => {
                return Err(CliError::NotImplemented(format!(
                    "--corba is only supported by --cpp, --csharp, --java, --rust (not {})",
                    backend_flag_name(other),
                )));
            }
            None => {
                return Err(CliError::Usage(
                    "--corba requires a backend (--cpp, --csharp, --java, or --rust)".to_string(),
                ));
            }
        }
    }
    if !opts.parse_only && opts.backend.is_none() {
        return Err(CliError::NotImplemented(
            "no action selected; use --parse-only or a backend flag (--rust)".to_string(),
        ));
    }
    if opts.parse_only && opts.backend.is_some() {
        return Err(CliError::Usage(
            "--parse-only and a backend flag are mutually exclusive".to_string(),
        ));
    }
    let path = opts
        .file
        .as_ref()
        .ok_or_else(|| CliError::Usage("missing input file (try --help)".to_string()))?;
    let src = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("cannot read {path}: {e}")))?;

    let cfg = ParserConfig::default();
    let ast = if opts.rti {
        parse_with_deltas(&src, &cfg, &[&RTI_CONNEXT])
    } else {
        parse(&src, &cfg)
    }
    .map_err(|e| CliError::Parse(format!("parse failed: {e}")))?;

    if opts.parse_only {
        println!("{ast}");
        return Ok(());
    }

    // Praesenz oben validiert; let-else statt .expect() wegen workspace
    // clippy::expect_used = "deny".
    let Some(backend) = opts.backend else {
        return Err(CliError::NotImplemented(
            "no backend selected (internal: should be caught above)".to_string(),
        ));
    };
    let out_dir = opts
        .output
        .as_ref()
        .ok_or_else(|| CliError::Usage("backend requires -o <dir>".to_string()))?;
    let base = basename_stem(path)
        .ok_or_else(|| CliError::Usage(format!("cannot derive basename from: {path}")))?;

    std::fs::create_dir_all(out_dir)
        .map_err(|e| CliError::Io(format!("cannot create {}: {e}", out_dir.display())))?;

    match backend {
        Backend::C => {
            let code = generate_c_header(&ast, &CGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("c codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.h"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Rust => {
            // DataTypes immer.
            let code = generate_rust_module(&ast, &RustGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("rust codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.rs"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
            // Mit --corba zusaetzlich Service-Code in zweite Datei. Anders
            // als bei C++/C#/Java (Single-File-Header) kollidieren die
            // File-level Inner-Attributes von idl-rust und corba-rust beim
            // Concat — deshalb Two-File-Output. User bindet via
            // `mod <base>; mod <base>_corba;`.
            if opts.corba {
                let svc = generate_corba_rust_module(&ast, &CorbaRustGenOptions::default())
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
            let code = if opts.corba {
                generate_cpp_header_with_corba_traits(&ast, &cpp_opts)
            } else {
                generate_cpp_header(&ast, &cpp_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("cpp codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.hpp"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Ts => {
            let code = generate_ts_source(&ast)
                .map_err(|e| CliError::NotImplemented(format!("ts codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.ts"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::CSharp => {
            let cs_opts = CsGenOptions::default();
            let code = if opts.corba {
                generate_csharp_with_corba_traits(&ast, &cs_opts)
            } else {
                generate_csharp(&ast, &cs_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("csharp codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.cs"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
        }
        Backend::Java => {
            let java_opts = JavaGenOptions::default();
            let files = if opts.corba {
                generate_java_files_with_corba_traits(&ast, &java_opts)
            } else {
                generate_java_files(&ast, &java_opts)
            }
            .map_err(|e| CliError::NotImplemented(format!("java codegen failed: {e}")))?;
            for file in files {
                let pkg_subpath = file.package_path.replace('.', "/");
                let pkg_dir = if pkg_subpath.is_empty() {
                    out_dir.clone()
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
        }
        Backend::Python => {
            let code = generate_python_module(&ast, &PythonGenOptions::default())
                .map_err(|e| CliError::NotImplemented(format!("python codegen failed: {e}")))?;
            let out_path = out_dir.join(format!("{base}.py"));
            std::fs::write(&out_path, code)
                .map_err(|e| CliError::Io(format!("cannot write {}: {e}", out_path.display())))?;
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

fn backend_flag_name(b: Backend) -> &'static str {
    match b {
        Backend::C => "--c",
        Backend::Cpp => "--cpp",
        Backend::Rust => "--rust",
        Backend::Ts => "--ts",
        Backend::CSharp => "--csharp",
        Backend::Java => "--java",
        Backend::Python => "--python",
    }
}

fn print_help() {
    println!(
        "zerodds-idlc {VERSION}\n\
         IDL4 compiler (Phase 1: alle sieben Backends)\n\n\
         USAGE:\n\
         \x20   zerodds-idlc --parse-only [--rti] <file.idl>\n\
         \x20   zerodds-idlc --c      -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --cpp    -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --rust   -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --ts     -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --csharp -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --java   -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --python -o <dir> <file.idl>\n\
         \x20   zerodds-idlc --cpp --corba    -o <dir> <file.idl>  (C++/C#/Java/Rust)\n\n\
         OPTIONS:\n\
         \x20   --parse-only       Parse + print AST (kein Codegen)\n\
         \x20   --rti              RTI Connext-Grammar-Delta beim Parse\n\
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
         \x20   -o, --output DIR   Ausgabe-Verzeichnis (fuer Backend-Modi)\n\
         \x20   -h, --help         Diese Hilfe\n\
         \x20   -V, --version      Versions-Info\n\n\
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
    fn run_multiple_backends_error_usage() {
        let result = run(&[
            "--rust".to_string(),
            "--cpp".to_string(),
            "-o".to_string(),
            "/tmp/x".to_string(),
            "some.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Usage(_))));
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
}
