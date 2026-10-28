// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The command line of `srcCxx/ShapeType_main.cxx`, transcribed.
//!
//! ## Why this is hand-parsed and not `clap`
//!
//! The suite's options are single-dash words — `-pub`, `-sub`,
//! `-domain`, `-topic`, `-type`, `-samples`, `-timeout`, `-help`,
//! `-help-types`. `clap` reads `-pub` as the three short flags `-p -u
//! -b`, and there is no configuration that makes it read the suite's
//! spelling. Anything that translated the flags would be a second
//! command line beside the suite's, and the suite's is the contract:
//! `ShapeType_main.cxx` parses `argv` with a `strcmp` chain, and so does
//! this.
//!
//! What is transcribed with it: the value ranges
//! (`validate_long(argv[i], "domainId:", 0, 255)`, `samples` 1..=100,
//! `timeout` from 2), the defaults and the `Info:` lines the suite
//! prints when it falls back to one, and the exit statuses.

use std::fmt::Write as _;

/// `ExitStatus` of `ShapeType_main.cxx`, and what a POSIX shell sees.
///
/// The C++ returns these negative values from `main`, and a POSIX exit
/// status is the low eight bits of it: `-1` reaches the shell as `255`,
/// `-2` as `254`, `-3` as `253`. The numbers below are what the shell
/// sees, so a reviewer comparing `echo $?` between two vendors compares
/// the same thing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExitStatus {
    /// `OK = 0`.
    Ok,
    /// `ERROR = -1`.
    Error,
    /// `INTERRUPT = -2` — a signal ended the run.
    Interrupt,
    /// `TIMEOUT = -3` — `-timeout <seconds>` ran out on the publisher.
    Timeout,
}

impl ExitStatus {
    /// The process exit code.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Error => 255,
            Self::Interrupt => 254,
            Self::Timeout => 253,
        }
    }
}

/// `-pub` / `-sub`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `-pub` — the default when neither is given.
    Publish,
    /// `-sub`.
    Subscribe,
}

/// A parsed command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// `-pub` / `-sub`.
    pub mode: Mode,
    /// `-domain <domainId>`, 0..=255, default 0.
    pub domain_id: i32,
    /// `-topic <topicName>`, default `XTYPESTestTopic`.
    pub topic_name: String,
    /// `-type <typeName>`, default `Shape1Default`.
    pub type_name: String,
    /// `-samples <count>`, 1..=100. `None` = `NOT_SET`, unlimited.
    pub samples: Option<i64>,
    /// `-timeout <seconds>`, from 2. `None` = `NOT_SET`, unlimited.
    pub seconds: Option<i64>,
}

/// What the argument vector asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// Run with these options.
    Run(Box<Options>),
    /// `-help`: print the usage and exit `OK`.
    Help,
    /// `-help-types`: print the 25 type names and exit `OK`.
    HelpTypes,
    /// A rejected command line. The text is what goes to stderr; the
    /// second field says whether the usage follows it, as it does in
    /// `ShapeType_main.cxx` for an unrecognised option.
    Rejected {
        /// The message, without a trailing newline.
        message: String,
        /// Print the usage after the message.
        with_usage: bool,
    },
}

/// `validate_long` of `ShapeType_main.cxx` — a range check that names
/// the value it rejected.
fn validate_long(raw: &str, what: &str, min: i64, max: i64) -> Result<i64, String> {
    let value: i64 = raw
        .parse()
        .map_err(|_| format!("{what} {raw} contains no digits"))?;
    if value < min || value > max {
        return Err(format!("{what} Value {value} not in range {min} - {max}"));
    }
    Ok(value)
}

/// The usage text, `show_usage()`.
#[must_use]
pub fn usage(program: &str) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "Usage: {program} [-help] [help-type] [-pub | -sub] [-domain <domainId>] \
         [-topic <topicName>] [-type <typeName>] [-timeout <seconds>] [-samples M]\n\n"
    );
    out.push_str(concat!(
        "-help              -- display this help\n",
        "-help-types        -- list <typeNames> available for \"-type\" option\n",
        "-pub               -- run as a publisher [default]\n",
        "-sub               -- run as a subscriber\n",
        "-domain <domainId> -- publish or subscribe on the DDS domain with id <domainId> [default: 0]\n",
        "-topic <topicName> -- publish or subscribe topic <topicName> [default: XTYPESTestTopic]\n",
        "-type  <typeName>  -- publish or subscribe the data type <typeName> [default: Shape1Default]\n",
        "-samples <count>   -- exit (successfuly) after sending or receiving <count> samples [default: unlimited]\n",
        "-timeout <seconds> -- exit (with error) after <seconds> seconds [default: unlimited]\n",
    ));
    out
}

/// The `Info:` lines `ShapeType_main.cxx` prints for each default it
/// fell back to, in its order. Collected rather than printed so the
/// parser stays a function of its input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Defaults {
    /// The lines to print on stderr, in order.
    pub notes: Vec<String>,
}

/// Parses the suite's command line.
///
/// `args` is the whole argument vector including `argv[0]`, as
/// `std::env::args()` yields it.
///
/// The `is_variant` callback decides whether a `-type` name is one of
/// the 25 — the check `ShapeType_main.cxx` does through
/// `ShapeTypeVariants::check_type_variant`, kept out of here so this
/// module does not need the generated types.
pub fn parse<F: Fn(&str) -> bool>(args: &[String], is_variant: F) -> (Parsed, Defaults) {
    let mut defaults = Defaults::default();
    let mut mode: Option<Mode> = None;
    let mut domain_id: Option<i32> = None;
    let mut topic_name: Option<String> = None;
    let mut type_name: Option<String> = None;
    let mut samples: Option<i64> = None;
    let mut seconds: Option<i64> = None;

    let reject = |message: String, with_usage: bool| {
        (Parsed::Rejected {
            message,
            with_usage,
        },)
    };

    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        // A value-taking option: the suite reports the missing value by
        // the option's own name.
        let take_value = |i: &mut usize, what: &str| -> Result<String, String> {
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("Error: missing <{what}> after \"{}\"", args[*i - 1]))
        };
        match arg {
            "-pub" => mode = Some(Mode::Publish),
            "-sub" => mode = Some(Mode::Subscribe),
            "-domain" => match take_value(&mut i, "domainId") {
                Ok(v) => match validate_long(&v, "domainId:", 0, 255) {
                    #[allow(clippy::cast_possible_truncation)]
                    Ok(n) => domain_id = Some(n as i32),
                    Err(detail) => {
                        return (
                            reject(format!("{detail}\nError: invalid domainId"), false).0,
                            defaults,
                        );
                    }
                },
                Err(m) => return (reject(m, false).0, defaults),
            },
            "-topic" => match take_value(&mut i, "topicName") {
                Ok(v) => topic_name = Some(v),
                Err(m) => return (reject(m, false).0, defaults),
            },
            "-type" => match take_value(&mut i, "typeName") {
                Ok(v) => type_name = Some(v),
                Err(m) => return (reject(m, false).0, defaults),
            },
            "-samples" => match take_value(&mut i, "M") {
                Ok(v) => match validate_long(&v, "samples:", 1, 100) {
                    Ok(n) => samples = Some(n),
                    Err(detail) => {
                        return (
                            reject(format!("{detail}\nError: invalid samples"), false).0,
                            defaults,
                        );
                    }
                },
                Err(m) => return (reject(m, false).0, defaults),
            },
            "-timeout" => match take_value(&mut i, "N") {
                Ok(v) => match validate_long(&v, "timeout:", 2, i64::MAX) {
                    Ok(n) => seconds = Some(n),
                    Err(detail) => {
                        return (
                            reject(format!("{detail}\nError: invalid seconds"), false).0,
                            defaults,
                        );
                    }
                },
                Err(m) => return (reject(m, false).0, defaults),
            },
            "-help-types" => return (Parsed::HelpTypes, defaults),
            "-help" => return (Parsed::Help, defaults),
            other => {
                return (
                    reject(format!("Error: unrecognized option: \"{other}\""), true).0,
                    defaults,
                );
            }
        }
        i += 1;
    }

    // The order of the fallbacks below is `ShapeType_main.cxx`'s own:
    // type, then domain, then topic, then pub/sub — and the type is
    // validated between the first and the second.
    let type_name = type_name.unwrap_or_else(|| {
        defaults
            .notes
            .push("Info: \"-type\" was not specified. Defaulting to: \"Shape1Default\"".to_owned());
        "Shape1Default".to_owned()
    });
    if !is_variant(&type_name) {
        return (
            Parsed::Rejected {
                message: format!(
                    "Error: Type \"{type_name}\" is not a valid type. The possible types are:"
                ),
                with_usage: false,
            },
            defaults,
        );
    }
    let domain_id = domain_id.unwrap_or_else(|| {
        defaults
            .notes
            .push("Info: \"-domain\" was not specified. Defaulting to: 0".to_owned());
        0
    });
    let topic_name = topic_name.unwrap_or_else(|| {
        defaults.notes.push(
            "Info: \"-topic\" was not specified. Defaulting to: \"XTYPESTestTopic\"".to_owned(),
        );
        "XTYPESTestTopic".to_owned()
    });
    let mode = mode.unwrap_or_else(|| {
        defaults.notes.push(
            "Info: Neither \"-pub\"  nor \"-sub\" was specified. Defaulting to: publish".to_owned(),
        );
        Mode::Publish
    });

    (
        Parsed::Run(Box::new(Options {
            mode,
            domain_id,
            topic_name,
            type_name,
            samples,
            seconds,
        })),
        defaults,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("xtypes_main".to_owned())
            .chain(args.iter().map(|s| (*s).to_owned()))
            .collect()
    }

    fn known(name: &str) -> bool {
        name.starts_with("Shape") && !name.starts_with("Shape9")
    }

    fn run(args: &[&str]) -> Options {
        match parse(&argv(args), known).0 {
            Parsed::Run(o) => *o,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    /// The suite's own defaults, from `ShapeType_main.cxx`: publish,
    /// domain 0, topic `XTYPESTestTopic`, type `Shape1Default`, no
    /// sample and no time limit. A client with different defaults would
    /// need every invocation spelled out, and the README's example
    /// invocations do not spell them out.
    #[test]
    fn the_bare_command_line_is_the_suites_default_run() {
        let o = run(&[]);
        assert_eq!(o.mode, Mode::Publish);
        assert_eq!(o.domain_id, 0);
        assert_eq!(o.topic_name, "XTYPESTestTopic");
        assert_eq!(o.type_name, "Shape1Default");
        assert_eq!(o.samples, None);
        assert_eq!(o.seconds, None);
    }

    /// Each fallback says so on stderr, as the suite does — four lines
    /// for a bare command line, none for a fully specified one.
    #[test]
    fn every_default_taken_is_announced() {
        let (_, d) = parse(&argv(&[]), known);
        assert_eq!(d.notes.len(), 4, "{:?}", d.notes);
        assert!(d.notes[0].contains("\"-type\""));
        let (_, d) = parse(
            &argv(&[
                "-sub",
                "-domain",
                "3",
                "-topic",
                "T",
                "-type",
                "Shape2Mutable",
            ]),
            known,
        );
        assert!(d.notes.is_empty(), "{:?}", d.notes);
    }

    /// The suite's README example: `-sub -domain 0 -type Shape2Extensible`.
    #[test]
    fn the_readme_invocation_parses() {
        let o = run(&["-sub", "-domain", "0", "-type", "Shape2Extensible"]);
        assert_eq!(o.mode, Mode::Subscribe);
        assert_eq!(o.type_name, "Shape2Extensible");
        assert_eq!(o.topic_name, "XTYPESTestTopic");
    }

    /// `validate_long(argv[i], "domainId:", 0, 255)`: the range is the
    /// suite's, and a value outside it is refused rather than clamped.
    #[test]
    fn the_domain_id_range_is_the_suites() {
        assert_eq!(run(&["-domain", "255"]).domain_id, 255);
        assert_eq!(run(&["-domain", "0"]).domain_id, 0);
        for bad in ["256", "-1", "x"] {
            assert!(
                matches!(
                    parse(&argv(&["-domain", bad]), known).0,
                    Parsed::Rejected { .. }
                ),
                "-domain {bad} was accepted"
            );
        }
    }

    /// `-samples` is 1..=100 and `-timeout` starts at 2, both from
    /// `ShapeType_main.cxx`. `-samples 0` is not "no limit" there — it
    /// is an error, and a client that read it as unlimited would run
    /// forever where the suite expected an exit.
    #[test]
    fn the_sample_and_timeout_ranges_are_the_suites() {
        assert_eq!(run(&["-samples", "1"]).samples, Some(1));
        assert_eq!(run(&["-samples", "100"]).samples, Some(100));
        assert!(matches!(
            parse(&argv(&["-samples", "0"]), known).0,
            Parsed::Rejected { .. }
        ));
        assert!(matches!(
            parse(&argv(&["-samples", "101"]), known).0,
            Parsed::Rejected { .. }
        ));
        assert_eq!(run(&["-timeout", "2"]).seconds, Some(2));
        assert!(matches!(
            parse(&argv(&["-timeout", "1"]), known).0,
            Parsed::Rejected { .. }
        ));
    }

    /// An option whose value is missing is an error naming the option,
    /// not a silent default.
    #[test]
    fn a_value_option_without_its_value_is_refused() {
        for opt in ["-domain", "-topic", "-type", "-samples", "-timeout"] {
            match parse(&argv(&[opt]), known).0 {
                Parsed::Rejected { message, .. } => {
                    assert!(message.contains(opt), "{opt}: {message}");
                }
                other => panic!("{opt} was accepted: {other:?}"),
            }
        }
    }

    /// An unrecognised option is refused **with** the usage, which is
    /// the one place `ShapeType_main.cxx` prints it on an error path.
    #[test]
    fn an_unknown_option_is_refused_with_the_usage() {
        match parse(&argv(&["-P"]), known).0 {
            Parsed::Rejected {
                message,
                with_usage,
            } => {
                assert!(message.contains("unrecognized option"), "{message}");
                assert!(with_usage);
            }
            other => panic!("-P was accepted: {other:?}"),
        }
    }

    /// `-P`/`-S` belong to the *other* suite. `dds-rtps`'s `shape_main`
    /// uses them for publish/subscribe, and someone who has both
    /// binaries open will type them here. Being refused loudly is the
    /// point: silently reading `-P` as `-pub` would start a run whose
    /// output looks right and whose command line the reviewer's notes
    /// would not match.
    #[test]
    fn the_other_suites_flags_are_not_quietly_accepted() {
        for foreign in ["-P", "-S", "--pub", "-t", "-d"] {
            assert!(
                matches!(parse(&argv(&[foreign]), known).0, Parsed::Rejected { .. }),
                "{foreign} was accepted"
            );
        }
    }

    /// A `-type` outside the 25 is refused before anything is created,
    /// and the message is the suite's.
    #[test]
    fn an_unknown_type_is_refused_before_the_participant_exists() {
        match parse(&argv(&["-type", "Shape9Nonesuch"]), known).0 {
            Parsed::Rejected { message, .. } => {
                assert!(message.contains("is not a valid type"), "{message}");
            }
            other => panic!("accepted: {other:?}"),
        }
    }

    /// `-help` and `-help-types` win over everything after them, as in
    /// the suite's parse loop, and neither starts a participant.
    #[test]
    fn help_short_circuits_the_rest_of_the_line() {
        assert_eq!(
            parse(&argv(&["-help", "-domain", "999"]), known).0,
            Parsed::Help
        );
        assert_eq!(
            parse(&argv(&["-help-types", "-type", "Nope"]), known).0,
            Parsed::HelpTypes
        );
    }

    /// The negative `ExitStatus` values of `ShapeType_main.cxx` reach a
    /// POSIX shell as their low eight bits. A reviewer reading `$?`
    /// sees 253 for a timeout, not 3 and not 0.
    #[test]
    fn the_exit_codes_are_the_suites_seen_through_posix() {
        assert_eq!(ExitStatus::Ok.code(), 0);
        assert_eq!(ExitStatus::Error.code(), 255);
        assert_eq!(ExitStatus::Interrupt.code(), 254);
        assert_eq!(ExitStatus::Timeout.code(), 253);
    }
}
