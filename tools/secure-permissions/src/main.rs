// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-secure-permissions` — CMS/PKCS#7 sign (and verify) DDS-Security
//! governance & permissions XML for the ZeroDDS builtin AccessControl plugin.
//!
//! ```text
//! zerodds-secure-permissions sign   --signer-cert <ca.pem> --signer-key <ca_key.pem> --in <doc.xml> --out <doc.p7s>
//! zerodds-secure-permissions verify --ca <permissions_ca.pem> --p7s <doc.p7s>
//! ```
//!
//! `sign` produces an opaque S/MIME `signed-data` document (the signed XML is
//! embedded), and self-checks it against the verifier before writing — a file
//! that lands on disk is guaranteed to load in a ZeroDDS participant.

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI-Tool: stdout/stderr sind die Ausgabe.

use std::process::ExitCode;

use zerodds_secure_permissions::{SignError, sign_xml_opaque};
use zerodds_security_permissions::{CmsPkcs7Verifier, XmlSignatureVerifier};

const USAGE: &str = "\
zerodds-secure-permissions — CMS/PKCS#7 sign & verify DDS-Security XML

USAGE:
    zerodds-secure-permissions sign   --signer-cert <ca.pem> --signer-key <ca_key.pem> \\
                                      --in <doc.xml> --out <doc.p7s>
    zerodds-secure-permissions verify --ca <permissions_ca.pem> --p7s <doc.p7s> [--out <doc.xml>]

COMMANDS:
    sign      CMS-sign governance/permissions XML with the Permissions CA.
              Output: opaque S/MIME signed-data (MIME headers + base64, NOT raw
              DER — `openssl cms -verify` needs the default `-inform SMIME`).
              The signer key must be PKCS#8 (`BEGIN PRIVATE KEY`); convert a
              SEC1 key with `openssl pkcs8 -topk8 -nocrypt`. Self-verified
              before writing.
    verify    Verify a signed document against a Permissions-CA cert; prints a
              status line, and with `--out` writes the recovered XML to a file.

EXIT CODES:
    0  ok      1  signing/verification error      2  bad invocation";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(m)) => {
            eprintln!("zerodds-secure-permissions: {m}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(CliError::Failed(m)) => {
            eprintln!("zerodds-secure-permissions: {m}");
            ExitCode::FAILURE
        }
    }
}

enum CliError {
    Usage(String),
    Failed(String),
}

impl From<SignError> for CliError {
    fn from(e: SignError) -> Self {
        CliError::Failed(e.to_string())
    }
}

fn run(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str).unwrap_or("") {
        "help" | "-h" | "--help" | "" => {
            println!("{USAGE}");
            Ok(())
        }
        "sign" => cmd_sign(&args[1..]),
        "verify" => cmd_verify(&args[1..]),
        other => Err(CliError::Usage(format!("unknown command `{other}`"))),
    }
}

/// Pull `--flag value` pairs into a small map; rejects unknown/odd args.
fn parse_flags(args: &[String], allowed: &[&str]) -> Result<Vec<(String, String)>, CliError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let flag = &args[i];
        if !flag.starts_with("--") || !allowed.contains(&flag.as_str()) {
            return Err(CliError::Usage(format!("unexpected argument `{flag}`")));
        }
        let val = args
            .get(i + 1)
            .ok_or_else(|| CliError::Usage(format!("`{flag}` needs a value")))?;
        out.push((flag.clone(), val.clone()));
        i += 2;
    }
    Ok(out)
}

fn get<'a>(flags: &'a [(String, String)], key: &str) -> Result<&'a str, CliError> {
    get_opt(flags, key).ok_or_else(|| CliError::Usage(format!("missing required `{key}`")))
}

fn get_opt<'a>(flags: &'a [(String, String)], key: &str) -> Option<&'a str> {
    flags
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn read_file(path: &str) -> Result<Vec<u8>, CliError> {
    std::fs::read(path).map_err(|e| CliError::Failed(format!("cannot read `{path}`: {e}")))
}

fn cmd_sign(args: &[String]) -> Result<(), CliError> {
    let flags = parse_flags(args, &["--signer-cert", "--signer-key", "--in", "--out"])?;
    let cert_path = get(&flags, "--signer-cert")?;
    let key_path = get(&flags, "--signer-key")?;
    let in_path = get(&flags, "--in")?;
    let out_path = get(&flags, "--out")?;

    let xml = read_file(in_path)?;
    let cert_pem = String::from_utf8(read_file(cert_path)?)
        .map_err(|_| CliError::Failed("signer cert is not UTF-8 PEM".to_string()))?;
    let key_pem = String::from_utf8(read_file(key_path)?)
        .map_err(|_| CliError::Failed("signer key is not UTF-8 PEM".to_string()))?;

    let p7s = sign_xml_opaque(&xml, &cert_pem, &key_pem)?;

    // Self-check: the document we are about to write MUST verify against the
    // Permissions CA (the signer cert is the CA in the default pattern).
    let verifier = CmsPkcs7Verifier::new(cert_pem.as_bytes())
        .map_err(|e| CliError::Failed(format!("self-check verifier: {e}")))?;
    let recovered = verifier
        .verify_and_extract(&p7s)
        .map_err(|e| CliError::Failed(format!("self-check failed: {e}")))?;
    if recovered != xml {
        return Err(CliError::Failed(
            "self-check: recovered XML differs from input".to_string(),
        ));
    }

    std::fs::write(out_path, &p7s)
        .map_err(|e| CliError::Failed(format!("cannot write `{out_path}`: {e}")))?;
    println!(
        "signed {in_path} -> {out_path} ({} bytes, self-check OK)",
        p7s.len()
    );
    Ok(())
}

fn cmd_verify(args: &[String]) -> Result<(), CliError> {
    let flags = parse_flags(args, &["--ca", "--p7s", "--out"])?;
    let ca_path = get(&flags, "--ca")?;
    let p7s_path = get(&flags, "--p7s")?;
    let out_path = get_opt(&flags, "--out");

    let ca_pem = read_file(ca_path)?;
    let p7s = read_file(p7s_path)?;
    let verifier =
        CmsPkcs7Verifier::new(&ca_pem).map_err(|e| CliError::Failed(format!("verifier: {e}")))?;
    let xml = verifier
        .verify_and_extract(&p7s)
        .map_err(|e| CliError::Failed(format!("verification failed: {e}")))?;

    if let Some(out) = out_path {
        std::fs::write(out, &xml)
            .map_err(|e| CliError::Failed(format!("cannot write `{out}`: {e}")))?;
        println!(
            "OK: {p7s_path} verifies against {ca_path}; recovered XML ({} bytes) -> {out}",
            xml.len()
        );
    } else {
        println!(
            "OK: {p7s_path} verifies against {ca_path} ({} bytes XML; use --out to extract it)",
            xml.len()
        );
    }
    Ok(())
}
