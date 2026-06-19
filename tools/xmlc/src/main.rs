// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-xmlc` — DDS-XML CLI: validator, deployment renderer, type lister.
//!
//! Thin command-line front-end over the `zerodds-xml` parsers (OMG DDS-XML
//! 1.0). It never touches the network — it parses a `.xml` file and reports.
//!
//! ```text
//! zerodds-xmlc <command> <file.xml>
//!   validate <file>   Parse every DDS-XML library (types/qos/domain/participant/
//!                     application); exit 0 if well-formed + parseable, else 1.
//!   render   <file>   Print a structured deployment summary (domains -> topics,
//!                     participants -> publishers/subscribers -> writers/readers).
//!   types    <file>   List the `<types>` (XTypes) definitions.
//!   help              Show this usage.
//! ```

#![allow(clippy::print_stderr)] // CLI-Tool: stderr fuer Fehler zulaessig (Policy: tools/* duerfen lockern).
#![allow(clippy::print_stdout)] // CLI-Tool: stdout ist die Ausgabe.

use std::process::ExitCode;

use zerodds_xml::parser::parse_xml_tree;
use zerodds_xml::xtypes_def::TypeDef;
use zerodds_xml::{
    XmlError, parse_application_libraries, parse_domain_libraries,
    parse_domain_participant_libraries, parse_qos_libraries, parse_type_libraries,
};

const USAGE: &str = "\
zerodds-xmlc — DDS-XML validator / deployment renderer / type lister

USAGE:
    zerodds-xmlc <command> <file.xml>

COMMANDS:
    validate <file>   Parse every DDS-XML library; exit 0 if valid, 1 on error
    render   <file>   Print a structured deployment summary
    types    <file>   List the <types> (XTypes) definitions
    help              Show this usage

EXIT CODES:
    0  success / valid      1  parse or validation error      2  bad invocation";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(m)) => {
            eprintln!("zerodds-xmlc: {m}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(CliError::Io(m) | CliError::Xml(m)) => {
            eprintln!("zerodds-xmlc: {m}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String),
    Xml(String),
}

fn run(args: &[String]) -> Result<(), CliError> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "help" | "-h" | "--help" | "" => {
            println!("{USAGE}");
            Ok(())
        }
        "validate" => {
            let (path, xml) = file_arg(cmd, args)?;
            cmd_validate(&path, &xml)
        }
        "render" => {
            let (_, xml) = file_arg(cmd, args)?;
            cmd_render(&xml)
        }
        "types" => {
            let (_, xml) = file_arg(cmd, args)?;
            cmd_types(&xml)
        }
        other => Err(CliError::Usage(format!("unknown command `{other}`"))),
    }
}

/// Resolve the `<file.xml>` positional argument and read it.
fn file_arg(cmd: &str, args: &[String]) -> Result<(String, String), CliError> {
    let path = args
        .get(1)
        .ok_or_else(|| CliError::Usage(format!("`{cmd}` needs a <file.xml> argument")))?;
    let xml = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("cannot read `{path}`: {e}")))?;
    Ok((path.clone(), xml))
}

fn xml_err(section: &str, e: &XmlError) -> CliError {
    CliError::Xml(format!("{section}: {e}"))
}

/// `validate` — well-formedness (`parse_xml_tree`) plus every library parser.
/// An absent library section is not an error (the parsers return an empty
/// `Vec`); only a malformed/invalid section fails. Prints a one-line tally.
fn cmd_validate(path: &str, xml: &str) -> Result<(), CliError> {
    parse_xml_tree(xml).map_err(|e| xml_err("xml", &e))?;
    let types = parse_type_libraries(xml).map_err(|e| xml_err("types", &e))?;
    let qos = parse_qos_libraries(xml).map_err(|e| xml_err("qos_library", &e))?;
    let domains = parse_domain_libraries(xml).map_err(|e| xml_err("domain_library", &e))?;
    let parts =
        parse_domain_participant_libraries(xml).map_err(|e| xml_err("participant_library", &e))?;
    let apps = parse_application_libraries(xml).map_err(|e| xml_err("application_library", &e))?;

    let n_types: usize = types.iter().map(|l| l.types.len()).sum();
    let n_profiles: usize = qos.iter().map(|l| l.profiles.len()).sum();
    let n_domains: usize = domains.iter().map(|l| l.domains.len()).sum();
    let n_parts: usize = parts.iter().map(|l| l.participants.len()).sum();
    println!(
        "valid: {path}\n  {} type-lib ({n_types} types), {} qos-lib ({n_profiles} profiles), \
         {} domain-lib ({n_domains} domains), {} participant-lib ({n_parts} participants), \
         {} application-lib",
        types.len(),
        qos.len(),
        domains.len(),
        parts.len(),
        apps.len(),
    );
    Ok(())
}

/// `render` — a human-readable deployment tree.
fn cmd_render(xml: &str) -> Result<(), CliError> {
    for lib in &parse_qos_libraries(xml).map_err(|e| xml_err("qos_library", &e))? {
        println!("qos_library {}", lib.name);
        for p in &lib.profiles {
            println!("  qos_profile {}", p.name);
        }
    }
    for lib in &parse_domain_libraries(xml).map_err(|e| xml_err("domain_library", &e))? {
        println!("domain_library {}", lib.name);
        for d in &lib.domains {
            println!("  domain {} (id={})", d.name, d.domain_id);
            for t in &d.topics {
                println!("    topic {} -> type {}", t.name, t.register_type_ref);
            }
        }
    }
    for lib in
        &parse_domain_participant_libraries(xml).map_err(|e| xml_err("participant_library", &e))?
    {
        println!("domain_participant_library {}", lib.name);
        for p in &lib.participants {
            println!("  participant {} -> domain {}", p.name, p.domain_ref);
            for pubr in &p.publishers {
                println!("    publisher {}", pubr.name);
                for w in &pubr.data_writers {
                    println!("      data_writer {} -> topic {}", w.name, w.topic_ref);
                }
            }
            for subr in &p.subscribers {
                println!("    subscriber {}", subr.name);
                for r in &subr.data_readers {
                    println!("      data_reader {} -> topic {}", r.name, r.topic_ref);
                }
            }
        }
    }
    Ok(())
}

/// `types` — list the `<types>` (XTypes) definitions with their kind + name.
fn cmd_types(xml: &str) -> Result<(), CliError> {
    let libs = parse_type_libraries(xml).map_err(|e| xml_err("types", &e))?;
    if libs.is_empty() {
        println!("(no <types> definitions)");
        return Ok(());
    }
    for lib in &libs {
        println!("type_library {} ({} types)", lib.name, lib.types.len());
        for t in &lib.types {
            print_typedef(t, 1);
        }
    }
    Ok(())
}

/// Pretty-print one `TypeDef`, recursing one level into modules.
///
/// zerodds-lint: recursion-depth 64
fn print_typedef(t: &TypeDef, indent: usize) {
    let pad = "  ".repeat(indent);
    match t {
        TypeDef::Struct(v) => println!("{pad}struct {}", v.name),
        TypeDef::Enum(v) => println!("{pad}enum {}", v.name),
        TypeDef::Union(v) => println!("{pad}union {}", v.name),
        TypeDef::Typedef(v) => println!("{pad}typedef {}", v.name),
        TypeDef::Bitmask(v) => println!("{pad}bitmask {}", v.name),
        TypeDef::Bitset(v) => println!("{pad}bitset {}", v.name),
        TypeDef::ForwardDcl(v) => println!("{pad}forward_dcl {}", v.name),
        TypeDef::Const(v) => println!("{pad}const {}", v.name),
        TypeDef::Include(v) => println!("{pad}include {}", v.file),
        TypeDef::Module(m) => {
            println!("{pad}module {}", m.name);
            for child in &m.types {
                print_typedef(child, indent + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<dds>
        <qos_library name="QL"><qos_profile name="Reliable"/></qos_library>
        <domain_library name="DL">
            <domain name="MyDomain" domain_id="42">
                <register_type name="State" type_ref="StateType"/>
                <topic name="StateTopic" register_type_ref="State"/>
            </domain>
        </domain_library>
        <domain_participant_library name="DPL">
            <domain_participant name="P" domain_ref="DL::MyDomain">
                <publisher name="Pub1">
                    <data_writer name="W1" topic_ref="StateTopic"/>
                </publisher>
                <subscriber name="Sub1">
                    <data_reader name="R1" topic_ref="StateTopic"/>
                </subscriber>
            </domain_participant>
        </domain_participant_library>
    </dds>"#;

    const TYPES_SAMPLE: &str = r#"<dds>
        <types name="Lib1">
            <struct name="Point"/>
            <enum name="Color"/>
            <module name="ns">
                <struct name="Inner"/>
            </module>
        </types>
    </dds>"#;

    #[test]
    fn validate_accepts_well_formed_doc() {
        assert!(cmd_validate("mem", SAMPLE).is_ok());
    }

    #[test]
    fn validate_rejects_malformed_xml() {
        let r = cmd_validate("mem", "<dds><qos_library></dds>");
        assert!(matches!(r, Err(CliError::Xml(_))), "got {r:?}");
    }

    #[test]
    fn render_walks_the_deployment_tree() {
        assert!(cmd_render(SAMPLE).is_ok());
    }

    #[test]
    fn types_parses_nested_modules() {
        assert!(cmd_types(TYPES_SAMPLE).is_ok());
    }

    #[test]
    fn run_requires_a_file_argument() {
        let r = run(&["validate".to_string()]);
        assert!(matches!(r, Err(CliError::Usage(_))), "got {r:?}");
    }

    #[test]
    fn run_rejects_unknown_command() {
        let r = run(&["frobnicate".to_string()]);
        assert!(matches!(r, Err(CliError::Usage(_))), "got {r:?}");
    }

    #[test]
    fn help_and_empty_args_are_ok() {
        assert!(run(&["help".to_string()]).is_ok());
        assert!(run(&[]).is_ok());
    }
}
