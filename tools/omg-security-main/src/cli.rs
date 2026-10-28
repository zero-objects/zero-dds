// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CLI flag surface for the OMG `dds-security` interoperability-test-suite
//! `ShapeType_main` reference client.
//!
//! Every flag and every default below is transcribed from the suite's own
//! `srcCxx/ShapeType_main.cxx` — its `main()` argument loop and its
//! `print_usage()` — cross-read against `README.md`'s two worked
//! invocations and against `srcCxx/opendds/ShapeType_configurator_opendds.h`,
//! which is the one committed configurator that actually configures
//! security rather than ignoring the arguments.
//!
//! ## Single-dash long options
//!
//! This suite's client is a hand-rolled `strcmp` loop, so its options are
//! single-dash long words: `-pub`, `-governance`, `-livelinessPeriod`.
//! `clap` spells long options with two dashes and has no single-dash-long
//! mode. [`normalize_args`] rewrites exactly the suite's spellings to
//! clap's before parsing, so an invocation copied verbatim out of the
//! suite README works and `--governance` also works. `-v` and `-h` are
//! genuine short options and are left alone.
//!
//! ## Five flags the suite does not have
//!
//! `-identityCa`, `-identityCert`, `-identityKey` and `-permissionsCa`.
//! The suite's clients hardcode their vendor's certificate paths in the
//! configurator header (`oci_opendds_dds_certs/…`) and offer environment
//! overrides for two of them (`OPENDDS_ID_CERTIFICATE`,
//! `OPENDDS_ID_PRIVATE_KEY`). This repository does not carry any
//! vendor's certificates — see `README.md` — so the four paths are
//! parameters here, each with a `ZERODDS_ID_*` / `ZERODDS_PERMISSIONS_CA`
//! environment fallback in the same spirit.
//!
//! The fifth is `-acceptX509v1IdentityCerts`, and it exists because of
//! what the suite's own certificates measure as — see [`Options`].

use clap::{Parser, ValueEnum};

/// `-v [e|d]` — the suite's two log levels.
///
/// `ERROR=1`, `DEBUG=2`, and `log_message` prints when
/// `level_verbosity <= verbosity_`: `e` prints error messages only, `d`
/// prints those plus the per-step diagnostics. Both go to **stdout** in
/// the suite (`std::cout`), and so do they here — this suite has no
/// `pexpect` harness whose stdout must stay level-independent, and
/// splitting them would make our transcript unlike every other client's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum VerbosityArg {
    /// `e` — ERROR only (the default).
    E,
    /// `d` — DEBUG (verbose).
    D,
}

/// The suite's `DEFAULT_PUBLISH_TOPIC` / `DEFAULT_SUBSCRIBE_TOPIC`.
///
/// `OD_OA_OM_OD` reads out of `TESTONLY_governance.xml`'s own comment:
/// Open Discovery, Open Access, Open Meta-data, Open Data — the one
/// topic rule with every protection switched off.
pub const DEFAULT_TOPIC: &str = "OD_OA_OM_OD";

/// The suite's `DEFAULT_COLOR`.
pub const DEFAULT_COLOR: &str = "BLACK";

/// The suite's `DEFAULT_GOVERNANCE_FILE`.
pub const DEFAULT_GOVERNANCE_FILE: &str = "TESTONLY_governance_signed.p7s";

/// The suite's `DEFAULT_PERMISSIONS_FILE`. Note that no file of this
/// name exists in the suite: every permissions document is per vendor
/// and per access mode
/// (`<vendor>_certs/TESTONLY_<vendor>_permissions_{join,read,write,readwrite,allowall}_signed.p7s`),
/// so a run that does not pass `-permissions` fails on a missing file.
/// That is the suite's own default and it is reproduced, not improved.
pub const DEFAULT_PERMISSIONS_FILE: &str = "TESTONLY_permissions_signed.p7s";

/// The suite's `DEFAULT_KEY_EST_ALGORITHM`.
pub const DEFAULT_KEY_EST_ALGORITHM: &str = "ECDHE-CEUM+P256";

/// The suite's default identity-CA path, from
/// `srcCxx/opendds/ShapeType_configurator_opendds.h` — relative to
/// `srcCxx/`, which is the working directory the suite README runs from.
pub const DEFAULT_IDENTITY_CA: &str = "file:../TESTONLY_identity_ca_cert.pem";

/// The suite's default permissions-CA path, same source and same
/// working-directory assumption.
pub const DEFAULT_PERMISSIONS_CA: &str = "file:../TESTONLY_permissions_ca_cert.pem";

/// The suite's long options, in the spelling its `strcmp` loop accepts.
/// [`normalize_args`] rewrites exactly these and nothing else.
const SUITE_LONG_OPTIONS: &[&str] = &[
    "domain",
    "pub",
    "sub",
    "color",
    "governance",
    "permissions",
    "kest",
    "partition",
    "livelinessPeriod",
    "disableSecurity",
    "logging",
    "help",
    // The four this client adds because the certificates are not in this
    // repository. Same spelling style, so an operator does not have to
    // remember which half of the command line takes one dash.
    "identityCa",
    "identityCert",
    "identityKey",
    "permissionsCa",
    // The fifth: a deliberate deviation from DDS-Security 1.2 §10.3.1.3,
    // without which this client cannot join the suite at all.
    "acceptX509v1IdentityCerts",
];

/// Rewrites the suite's single-dash long options (`-pub`) into clap's
/// spelling (`--pub`), leaving everything else untouched.
///
/// Only a token that is exactly `-<known long option>` is rewritten, so
/// an option *value* that happens to look like a flag passes through
/// unchanged, and `-v`/`-h` stay the short options they are.
#[must_use]
pub fn normalize_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    args.into_iter()
        .map(Into::into)
        .map(|arg| match arg.strip_prefix('-') {
            Some(name) if !name.starts_with('-') && SUITE_LONG_OPTIONS.contains(&name) => {
                format!("--{name}")
            }
            _ => arg,
        })
        .collect()
}

/// OMG `dds-security` `ShapeType_main` — ZeroDDS reference client.
#[derive(Parser, Debug, Clone)]
#[command(name = "shapetype_main", author, version, about, long_about = None)]
pub struct Options {
    /// `-v [e|d]`: set log message verbosity.
    #[arg(short = 'v', value_enum)]
    pub verbosity: Option<VerbosityArg>,

    /// `-domain <domainId>`: the DDS domain id (default: 0).
    #[arg(long = "domain")]
    pub domain_id: Option<i32>,

    /// `-pub <pubTopic>`: the topic to publish on.
    #[arg(long = "pub")]
    pub publish_topic: Option<String>,

    /// `-sub <subTopic>`: the topic to subscribe to.
    #[arg(long = "sub")]
    pub subscribe_topic: Option<String>,

    /// `-color <colorName>`: the colour written into every sample.
    #[arg(long = "color")]
    pub color: Option<String>,

    /// `-governance <governanceFile>`: the CMS-signed governance
    /// document (`.p7s`). A `file:` URI or a plain path.
    #[arg(long = "governance")]
    pub governance: Option<String>,

    /// `-permissions <permissionsFile>`: the CMS-signed permissions
    /// document (`.p7s`). A `file:` URI or a plain path.
    #[arg(long = "permissions")]
    pub permissions: Option<String>,

    /// `-kest <dds.sec.auth.key_establishment_algorithm>`: the key
    /// agreement to announce as initiator.
    #[arg(long = "kest")]
    pub key_establishment_algorithm: Option<String>,

    /// `-partition <partitionStr>`: one PARTITION name on the publisher
    /// and the subscriber.
    #[arg(long = "partition")]
    pub partition: Option<String>,

    /// `-livelinessPeriod <float>`: LIVELINESS lease duration in
    /// seconds (`0` = leave the default).
    #[arg(long = "livelinessPeriod", default_value_t = 0.0)]
    pub liveliness_period: f32,

    /// `-disableSecurity`: run plaintext RTPS. Every `dds.sec.*`
    /// property is then absent, which is the point of the flag.
    #[arg(long = "disableSecurity")]
    pub disable_security: bool,

    /// `-logging`: turn the DDS-Security logging plugin on.
    #[arg(long = "logging")]
    pub logging: bool,

    /// `-identityCa <file>`: the identity CA bundle
    /// (`dds.sec.auth.identity_ca`). Not a suite flag — see the module
    /// documentation.
    #[arg(long = "identityCa")]
    pub identity_ca: Option<String>,

    /// `-identityCert <file>`: this participant's identity certificate
    /// (`dds.sec.auth.identity_certificate`). Not a suite flag.
    #[arg(long = "identityCert")]
    pub identity_cert: Option<String>,

    /// `-identityKey <file>`: this participant's PKCS#8 private key
    /// (`dds.sec.auth.private_key`). Not a suite flag.
    #[arg(long = "identityKey")]
    pub identity_key: Option<String>,

    /// `-permissionsCa <file>`: the permissions CA bundle
    /// (`dds.sec.access.permissions_ca`). Not a suite flag.
    #[arg(long = "permissionsCa")]
    pub permissions_ca: Option<String>,

    /// `-acceptX509v1IdentityCerts`: accept X.509 **v1** identity
    /// certificates — a deliberate deviation from DDS-Security 1.2
    /// §10.3.1.3, which requires v3. Off by default.
    ///
    /// It exists because of what the suite ships. Measured with
    /// `openssl x509 -noout -text` on `github.com/omg-dds/dds-security`,
    /// the identity certificates of RTI Connext, eProsima Fast DDS and
    /// ADLINK Vortex — and `TESTONLY_permissions_ca_cert.pem` — are
    /// `Version: 1 (0x0)`; only the identity CA is v3. `rustls-webpki`
    /// refuses a v1 end-entity certificate outright
    /// (`UnsupportedCertVersion`), so without this flag a run against the
    /// suite's own material fails before a participant exists. All six
    /// vendors accept each other's v1 identities, which makes the suite's
    /// material — not this client — the thing that departs from
    /// §10.3.1.3.
    ///
    /// The flag sets one property,
    /// `org.zerodds.sec.auth.accept_x509v1_identity_certificate`,
    /// on the participant's `PropertyQosPolicy`, and that property
    /// relaxes the version check and nothing else: signature, validity
    /// period, chain to the identity CA and subject are still checked,
    /// and every accepted v1 certificate produces a `Warning`
    /// security-log event naming its subject and the clause.
    #[arg(long = "acceptX509v1IdentityCerts")]
    pub accept_x509v1_identity_certs: bool,
}

impl Options {
    /// `-v d` — debug verbosity is on.
    #[must_use]
    pub fn debug_verbosity(&self) -> bool {
        matches!(self.verbosity, Some(VerbosityArg::D))
    }

    /// Security is on unless `-disableSecurity` says otherwise, exactly
    /// as the suite's `use_security` defaults to `true`.
    #[must_use]
    pub fn use_security(&self) -> bool {
        !self.disable_security
    }

    /// The suite's rule: when neither `-pub` nor `-sub` is given, the
    /// client does both on `OD_OA_OM_OD`. Otherwise each side is present
    /// only if its own flag was given.
    #[must_use]
    pub fn topics(&self) -> (Option<String>, Option<String>) {
        match (&self.publish_topic, &self.subscribe_topic) {
            (None, None) => (
                Some(DEFAULT_TOPIC.to_string()),
                Some(DEFAULT_TOPIC.to_string()),
            ),
            (p, s) => (p.clone(), s.clone()),
        }
    }

    /// `-color`, defaulted the suite's way.
    #[must_use]
    pub fn resolved_color(&self) -> &str {
        self.color.as_deref().unwrap_or(DEFAULT_COLOR)
    }

    /// `-domain`, defaulted the suite's way.
    #[must_use]
    pub fn resolved_domain_id(&self) -> i32 {
        self.domain_id.unwrap_or(0)
    }

    /// The identity CA path: the flag, else `ZERODDS_ID_CA`, else
    /// nothing (the caller applies the suite's default).
    #[must_use]
    pub fn resolved_identity_ca(&self) -> Option<String> {
        or_env(self.identity_ca.as_deref(), "ZERODDS_ID_CA")
    }

    /// The identity certificate path: the flag, else
    /// `ZERODDS_ID_CERTIFICATE`. There is no default — see
    /// `security::profile_config_from_properties`.
    #[must_use]
    pub fn resolved_identity_cert(&self) -> Option<String> {
        or_env(self.identity_cert.as_deref(), "ZERODDS_ID_CERTIFICATE")
    }

    /// The private-key path: the flag, else `ZERODDS_ID_PRIVATE_KEY`.
    #[must_use]
    pub fn resolved_identity_key(&self) -> Option<String> {
        or_env(self.identity_key.as_deref(), "ZERODDS_ID_PRIVATE_KEY")
    }

    /// The permissions CA path: the flag, else
    /// `ZERODDS_PERMISSIONS_CA`.
    #[must_use]
    pub fn resolved_permissions_ca(&self) -> Option<String> {
        or_env(self.permissions_ca.as_deref(), "ZERODDS_PERMISSIONS_CA")
    }
}

/// The flag wins over the environment variable, and an environment
/// variable set to the empty string counts as unset.
///
/// The environment fallback exists because the suite's own clients have
/// one — `srcCxx/opendds/ShapeType_configurator_opendds.h` reads
/// `OPENDDS_ID_CERTIFICATE` and `OPENDDS_ID_PRIVATE_KEY` over its
/// hardcoded paths. Ours covers all four paths, because none of them is
/// hardcoded here: this repository carries no certificates.
///
/// Resolved here rather than through clap's `env` feature, so that this
/// crate's clap feature set stays the one `tools/omg-shape-main` already
/// builds.
fn or_env(flag: Option<&str>, var: &str) -> Option<String> {
    if let Some(value) = flag {
        return Some(value.to_string());
    }
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        Options::try_parse_from(normalize_args(args.iter().copied())).expect("parse")
    }

    /// The suite README's own publisher invocation, copied verbatim,
    /// has to reach the same three fields it reaches in the C++ client.
    #[test]
    fn the_suite_readme_invocation_parses_as_written() {
        let opts = parse(&[
            "shapetype_main",
            "-pub",
            "PD_RWA_OM_OD",
            "-permissions",
            "../rti_connext_dds_certs/TESTONLY_rti_connext_dds_permissions_write_signed.p7s",
            "-governance",
            "../TESTONLY_governance_signed.p7s",
        ]);
        assert_eq!(opts.publish_topic.as_deref(), Some("PD_RWA_OM_OD"));
        assert_eq!(
            opts.permissions.as_deref(),
            Some("../rti_connext_dds_certs/TESTONLY_rti_connext_dds_permissions_write_signed.p7s")
        );
        assert_eq!(
            opts.governance.as_deref(),
            Some("../TESTONLY_governance_signed.p7s")
        );
        assert_eq!(opts.subscribe_topic, None, "-pub alone publishes only");
    }

    /// `ShapeType_main.cxx`: neither `-pub` nor `-sub` means both, on
    /// `OD_OA_OM_OD`.
    #[test]
    fn neither_pub_nor_sub_defaults_to_both_on_the_open_topic() {
        let (p, s) = parse(&["shapetype_main"]).topics();
        assert_eq!(p.as_deref(), Some(DEFAULT_TOPIC));
        assert_eq!(s.as_deref(), Some(DEFAULT_TOPIC));

        let (p, s) = parse(&["shapetype_main", "-sub", "PD_RA_OM_OD"]).topics();
        assert_eq!(p, None, "-sub alone subscribes only");
        assert_eq!(s.as_deref(), Some("PD_RA_OM_OD"));
    }

    /// `normalize_args` rewrites option names and leaves values alone.
    /// A topic or a path that happens to spell an option name must not
    /// grow a second dash on its way through.
    #[test]
    fn normalize_rewrites_option_names_and_not_their_values() {
        assert_eq!(
            normalize_args(["shapetype_main", "-pub", "-domain", "-v", "d"]),
            vec!["shapetype_main", "--pub", "--domain", "-v", "d"],
        );
        // A value is only left alone because it does not start with a
        // dash; the suite's own loop has the same blind spot and this
        // test records where the equivalence ends.
        assert_eq!(
            normalize_args(["shapetype_main", "-color", "domain"]),
            vec!["shapetype_main", "--color", "domain"],
        );
        assert_eq!(normalize_args(["-unknown"]), vec!["-unknown"]);
        assert_eq!(normalize_args(["--pub"]), vec!["--pub"]);
    }

    /// `-v` takes the suite's two letters and nothing else: an
    /// unrecognised level is a hard error there
    /// (`return -1` after "unrecognized value for verbosity"), so it is
    /// one here.
    #[test]
    fn verbosity_takes_only_the_two_suite_letters() {
        assert!(parse(&["shapetype_main", "-v", "d"]).debug_verbosity());
        assert!(!parse(&["shapetype_main", "-v", "e"]).debug_verbosity());
        assert!(!parse(&["shapetype_main"]).debug_verbosity());
        assert!(
            Options::try_parse_from(normalize_args(["shapetype_main", "-v", "x"])).is_err(),
            "an unknown verbosity letter must be refused, not ignored"
        );
    }

    /// `use_security` defaults to true; `-disableSecurity` is the only
    /// thing that turns it off.
    #[test]
    fn security_is_on_unless_disable_security_is_passed() {
        assert!(parse(&["shapetype_main"]).use_security());
        assert!(!parse(&["shapetype_main", "-disableSecurity"]).use_security());
    }
}
