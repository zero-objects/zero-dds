// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-xmlc` — read an OMG DDS-XML 1.0 document and say what is in
//! it, or what is wrong with it.
//!
//! ```text
//! zerodds-xmlc validate FILE...   parse every library; report what failed
//! zerodds-xmlc types    FILE...   the type definitions, by library
//! zerodds-xmlc render   FILE...   the deployment: domains, participants, endpoints
//! ```
//!
//! It never opens a socket. A DDS-XML file describes a system; this
//! reads the description.
//!
//! ## Why `validate` parses all five libraries
//!
//! A DDS-XML document may hold types, QoS profiles, domains,
//! participants and applications, and any of them may be absent.
//! Stopping at the first library that parses would call a file valid on
//! the strength of the one part that happened to be first, so every
//! library is parsed and every failure is reported — the caller finds
//! out about all of them in one run instead of one per fix.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use zerodds_xml::application::parse_application_libraries_with;
use zerodds_xml::domain::parse_domain_libraries_with;
use zerodds_xml::participant::parse_domain_participant_libraries_with;
use zerodds_xml::qos_parser::parse_qos_libraries_with;
use zerodds_xml::xtypes_def::TypeDef;
use zerodds_xml::xtypes_parser::parse_type_libraries_with;
use zerodds_xml::{DiagnosticKind, Parsed, ReadOptions, UnknownElements, VendorForms, XmlError};

/// What the tool was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Parse every library and report what failed.
    Validate,
    /// The type definitions, by library.
    Types,
    /// The deployment the document describes.
    Render,
}

impl Command {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "validate" => Some(Self::Validate),
            "types" => Some(Self::Types),
            "render" => Some(Self::Render),
            _ => None,
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-xmlc {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let (command, files, options) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("error: {message}");
            print_help();
            return ExitCode::from(2);
        }
    };

    let mut failures = 0usize;
    let mut unreadable = false;
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("error: {path}: cannot be read");
            unreadable = true;
            continue;
        };
        failures += run_one(command, path, &text, &options);
    }

    if unreadable {
        return ExitCode::from(3);
    }
    if command == Command::Validate {
        println!("validate: {} file(s), {failures} failure(s)", files.len());
    }
    if failures > 0 {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn parse_args(args: &[String]) -> Result<(Command, Vec<String>, ReadOptions), String> {
    let mut iter = args.iter();
    let first = iter.next().ok_or("a subcommand is required")?;
    let command = Command::parse(first)
        .ok_or_else(|| format!("unknown subcommand `{first}` — see --help"))?;
    let mut files = Vec::new();
    let mut options = ReadOptions::default();
    for arg in iter {
        if arg == "--skip-unknown" {
            options.unknown = UnknownElements::ReportAndSkip;
        } else if arg == "--vendor-forms" {
            options.vendor_forms = VendorForms::Accept;
        } else if arg.starts_with('-') {
            return Err(format!("unknown option `{arg}`"));
        } else {
            files.push(arg.clone());
        }
    }
    if files.is_empty() {
        return Err(String::from("at least one file is required"));
    }
    Ok((command, files, options))
}

/// The value of a parse; what the reader left out is listed on stderr
/// with the element's path (only under `--skip-unknown`).
fn take<T>(path: &str, result: Result<Parsed<T>, XmlError>) -> Result<T, XmlError> {
    let parsed = result?;
    for diagnostic in &parsed.diagnostics {
        if diagnostic.kind == DiagnosticKind::VendorForm {
            eprintln!("{path}: vendor form {diagnostic}");
        } else {
            eprintln!("{path}: skipped {diagnostic}");
        }
    }
    Ok(parsed.value)
}

/// What one library's parse produced, reduced to what the caller prints.
struct Outcome {
    library: &'static str,
    error: Option<XmlError>,
}

/// Parse all five libraries. A library that is absent parses to an empty
/// list, which is not a failure — a document that only declares types is
/// a valid document.
fn parse_all(path: &str, text: &str, options: &ReadOptions) -> Vec<Outcome> {
    fn outcome<T>(library: &'static str, result: Result<T, XmlError>) -> Outcome {
        Outcome {
            library,
            error: result.err(),
        }
    }
    vec![
        outcome(
            "types",
            take(path, parse_type_libraries_with(text, options)),
        ),
        outcome("qos", take(path, parse_qos_libraries_with(text, options))),
        outcome(
            "domain",
            take(path, parse_domain_libraries_with(text, options)),
        ),
        outcome(
            "participant",
            take(path, parse_domain_participant_libraries_with(text, options)),
        ),
        outcome(
            "application",
            take(path, parse_application_libraries_with(text, options)),
        ),
    ]
}

fn run_one(command: Command, path: &str, text: &str, options: &ReadOptions) -> usize {
    match command {
        Command::Validate => {
            let mut failures = 0;
            for outcome in parse_all(path, text, options) {
                if let Some(error) = outcome.error {
                    println!("{path}: error: <{}>: {error:?}", outcome.library);
                    failures += 1;
                }
            }
            failures
        }
        Command::Types => match take(path, parse_type_libraries_with(text, options)) {
            Ok(libraries) => {
                for library in &libraries {
                    let name = if library.name.is_empty() {
                        "<unnamed>"
                    } else {
                        &library.name
                    };
                    println!("{path}: types {name}");
                    for definition in &library.types {
                        print_type(definition, 1);
                    }
                }
                0
            }
            Err(error) => {
                println!("{path}: error: <types>: {error:?}");
                1
            }
        },
        Command::Render => render(path, text, options),
    }
}

/// One type definition and, for a module, what is inside it.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by the document)
fn print_type(definition: &TypeDef, depth: usize) {
    let pad = "  ".repeat(depth);
    match definition {
        TypeDef::Struct(t) => println!("{pad}struct {}", t.name),
        TypeDef::Enum(t) => println!("{pad}enum {}", t.name),
        TypeDef::Union(t) => println!("{pad}union {}", t.name),
        TypeDef::Typedef(t) => println!("{pad}typedef {}", t.name),
        TypeDef::Bitmask(t) => println!("{pad}bitmask {}", t.name),
        TypeDef::Bitset(t) => println!("{pad}bitset {}", t.name),
        TypeDef::Module(m) => {
            println!("{pad}module {}", m.name);
            for inner in &m.types {
                print_type(inner, depth + 1);
            }
        }
        TypeDef::Include(i) => println!("{pad}#include {}", i.file),
        _ => println!("{pad}<other>"),
    }
}

/// The deployment: what domains exist, and which participants join them.
fn render(path: &str, text: &str, options: &ReadOptions) -> usize {
    let mut failures = 0;
    match take(path, parse_domain_libraries_with(text, options)) {
        Ok(libraries) => {
            for library in &libraries {
                for domain in &library.domains {
                    println!("{path}: domain {}", domain.name);
                }
            }
        }
        Err(error) => {
            println!("{path}: error: <domain_library>: {error:?}");
            failures += 1;
        }
    }
    match take(path, parse_domain_participant_libraries_with(text, options)) {
        Ok(libraries) => {
            for library in &libraries {
                for participant in &library.participants {
                    let domain = participant.domain_ref.as_deref().unwrap_or("<none>");
                    println!(
                        "{path}: participant {} -> domain {domain}",
                        participant.name
                    );
                }
            }
        }
        Err(error) => {
            println!("{path}: error: <domain_participant_library>: {error:?}");
            failures += 1;
        }
    }
    failures
}

fn print_help() {
    println!(
        "zerodds-xmlc — read an OMG DDS-XML 1.0 document\n\
         \n\
         USAGE:\n    \
             zerodds-xmlc <SUBCOMMAND> FILE...\n\
         \n\
         SUBCOMMANDS:\n    \
             validate    parse every library; report what failed\n    \
             types       the type definitions, by library\n    \
             render      the deployment: domains and participants\n\
         \n\
         OPTIONS:\n    \
             --vendor-forms  read the forms other DDS products write\n                             (topic_filter as element, qos_profile_ref,\n                             IDL type names, ...) and list each with its\n                             path on stderr; without it the first one is an\n                             error naming the DDS-XML 1.0 form\n    \
             --skip-unknown  leave out elements and attributes the DDS-XML\n                             schemas do not define and list each with its\n                             path on stderr; without it the first one is\n                             an error\n    \
             -h, --help     this text\n    \
             -V, --version  the version\n\
         \n\
         EXIT CODES:\n    \
             0  nothing to report\n    \
             1  a library failed to parse\n    \
             2  the command line was wrong\n    \
             3  a file could not be read\n"
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn args(tokens: &[&str]) -> Result<(Command, Vec<String>, ReadOptions), String> {
        parse_args(&tokens.iter().map(|t| (*t).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_subcommand_and_a_file_are_the_minimum() {
        let (command, files, _) = args(&["validate", "a.xml"]).expect("valid");
        assert_eq!(command, Command::Validate);
        assert_eq!(files, vec![String::from("a.xml")]);
        args(&["validate"]).expect_err("a file is required");
        args(&[]).expect_err("a subcommand is required");
    }

    #[test]
    fn what_was_not_understood_is_named() {
        let error = args(&["deploy", "a.xml"]).expect_err("not a subcommand");
        assert!(error.contains("deploy"), "{error}");
        let error = args(&["types", "--all", "a.xml"]).expect_err("unknown option");
        assert!(error.contains("--all"), "{error}");
    }

    /// A document that declares only types is valid. Nothing here treats
    /// an absent library as a failure — the five are all optional.
    #[test]
    fn a_document_with_one_library_validates() {
        let xml = r#"<dds><types><struct name="S"/></types></dds>"#;
        let failures: usize = parse_all("t.xml", xml, &ReadOptions::default())
            .iter()
            .filter(|o| o.error.is_some())
            .count();
        assert_eq!(failures, 0, "one library present, four absent");
    }

    /// Every library is parsed even after one fails, so a caller learns
    /// about all of them in one run.
    #[test]
    fn all_five_libraries_are_attempted() {
        let outcomes = parse_all("t.xml", "<dds></dds>", &ReadOptions::default());
        assert_eq!(outcomes.len(), 5);
        let names: Vec<&str> = outcomes.iter().map(|o| o.library).collect();
        assert_eq!(
            names,
            vec!["types", "qos", "domain", "participant", "application"]
        );
    }

    #[test]
    fn a_malformed_document_reports_a_failure() {
        let failures = run_one(
            Command::Validate,
            "bad.xml",
            "<dds><types>",
            &ReadOptions::default(),
        );
        assert!(failures > 0, "an unterminated element must not validate");
    }

    /// An element the schemas do not define fails by default and is left
    /// out, with its path, under `--skip-unknown`.
    #[test]
    fn unknown_elements_fail_unless_skipped() {
        let xml = "<dds><qos_library name=\"L\"><qos_profile name=\"P\">\
                   <datawriter_qos><publish_mode/></datawriter_qos>\
                   </qos_profile></qos_library></dds>";
        let strict = run_one(Command::Validate, "t.xml", xml, &ReadOptions::default());
        assert!(strict > 0, "default refuses");
        let (_, files, options) = args(&["validate", "--skip-unknown", "t.xml"]).expect("valid");
        assert_eq!(files, vec![String::from("t.xml")], "a flag is no file");
        let lenient = run_one(Command::Validate, "t.xml", xml, &options);
        assert_eq!(lenient, 0, "skipped under --skip-unknown");
    }

    /// A vendor form fails by default and is read under `--vendor-forms`.
    #[test]
    fn vendor_forms_fail_unless_accepted() {
        let xml = "<dds><qos_library name=\"L\"><qos_profile name=\"P\">\
                   <topic_filter>A*</topic_filter>\
                   </qos_profile></qos_library></dds>";
        let strict = run_one(Command::Validate, "t.xml", xml, &ReadOptions::default());
        assert!(strict > 0, "default refuses");
        let (_, _, options) = args(&["validate", "--vendor-forms", "t.xml"]).expect("valid");
        assert_eq!(run_one(Command::Validate, "t.xml", xml, &options), 0);
    }
}
