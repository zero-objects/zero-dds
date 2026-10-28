// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The command line of the OMG `dds-xtypes` v2 test application.
//!
//! Transcribed from `src/cxx/test_main.cxx::print_usage` and its `parse`
//! loop on the suite's `feature/xml-driver-enhanced` branch. That branch
//! is the one the suite's own workflow runs and the one its
//! `v2.0.2025-wip` release ships binaries for; the `master` branch's
//! `srcCxx/ShapeType_main.cxx` is a different program with a different
//! command line, and `tools/omg-xtypes-main` is the client for that one.
//!
//! Two conventions of this suite that a `clap`-shaped parser gets wrong,
//! which is why the loop is written out:
//!
//! * short options are single-dash single letters that take a separate
//!   value (`-t test`, `-y Test::struct_f1`), and
//! * long options are double-dash words (`--type-folder types`).
//!
//! The suite never passes `--flag=value`, and neither form is accepted
//! here: a value that arrived glued to its flag would be read as part of
//! the flag name and refused, rather than silently becoming a topic name
//! nobody asked for.

use std::fmt;
use zerodds_ast_layout::DefaultedDiscriminatorMember;
use zerodds_types::assignability::{KeyMemberBounds, StrongAssignability};
use zerodds_xml_compose::KeyOnUnionDiscriminator;

/// What one of the suite's six `TypeConsistencyEnforcementQosPolicy`
/// switches says. Four states, not three: `t` / `f` / `d`, and *absent*.
///
/// `d` is not "false" and it is not "absent" either. `test_main.cxx`
/// starts every run at its own baseline (see
/// `app::type_consistency`) and `d` asks for
/// `TypeConsistency_get_default()`, the QoS policy default of
/// §7.6.3.4.1. The two differ in `ignore_member_names`, and the suite
/// tests the difference: `struct_different_names_ok` passes no flag and
/// expects the pair to communicate, `tc_ignore_member_names_3` passes
/// `d` on the same shape and expects `INCONSISTENT_TOPIC`. Collapsing
/// absent into `d` answers one of the two wrong whichever way the
/// collapse goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriState {
    /// The flag was not given at all.
    Absent,
    /// `d` — the QoS policy default of XTypes 1.3 §7.6.3.4.1.
    Default,
    /// `t`.
    True,
    /// `f`.
    False,
}

impl TriState {
    /// Reads one `t` / `f` / `d` value.
    fn parse(flag: &str, value: &str) -> Result<Self, CliError> {
        match value {
            "t" | "true" | "1" => Ok(Self::True),
            "f" | "false" | "0" => Ok(Self::False),
            "d" | "default" => Ok(Self::Default),
            _ => Err(CliError::BadValue {
                flag: flag.to_owned(),
                value: value.to_owned(),
                expected: "t, f or d",
            }),
        }
    }

    /// The value to put in the QoS policy, or `None` where the switch
    /// names no value of its own — absent, or `d`.
    #[must_use]
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Self::Absent | Self::Default => None,
            Self::True => Some(true),
            Self::False => Some(false),
        }
    }
}

/// Everything the suite's command line can say.
#[derive(Debug, Clone)]
pub struct Options {
    /// `-d`. Default 0.
    pub domain_id: u32,
    /// `-t`. Required — `validate()` refuses without it.
    pub topic_name: Option<String>,
    /// `-y`, a scoped name such as `Test::struct_f1`.
    pub type_name: Option<String>,
    /// `--type-folder`.
    pub type_folder: Option<String>,
    /// `--type-file`, without extension.
    pub type_file: Option<String>,
    /// `--data-folder`.
    pub data_folder: Option<String>,
    /// `--data-file`, without extension.
    pub data_file: Option<String>,
    /// `-P`.
    pub publish: bool,
    /// `-S`.
    pub subscribe: bool,
    /// `-b` / `-r`. `true` = RELIABLE, the suite's default.
    pub reliable: bool,
    /// `-k`. `None` = the implementation default.
    pub history_depth: Option<i32>,
    /// `-p`.
    pub partition: Option<String>,
    /// `-D`.
    pub durability: Option<char>,
    /// `-x`. 1 = XCDR1, 2 = XCDR2. Default 2.
    pub data_representation: u8,
    /// `-w`.
    pub print_writer_samples: bool,
    /// `--force-type-validation`.
    pub force_type_validation: TriState,
    /// `--ignore-member-names`.
    pub ignore_member_names: TriState,
    /// `--ignore-seq-bounds`.
    pub ignore_seq_bounds: TriState,
    /// `--ignore-str-bounds`.
    pub ignore_str_bounds: TriState,
    /// `--prevent-type-widening`.
    pub prevent_type_widening: TriState,
    /// `--allow-type-coercion`.
    pub allow_type_coercion: TriState,
    /// `--disable-type-info`. When set, this endpoint announces no
    /// `TypeObject` and no `TypeIdentifier`, which drives the peer into
    /// Step 2 of XTypes 1.3 §7.6.3.4.2.
    pub disable_type_info: bool,
    /// `--type-object-version`. 1 or 2, default 2.
    pub type_object_version: u8,
    /// `--print-typeid`.
    pub print_typeid: bool,
    /// `--periodic-announcement`, in milliseconds. 0 = off.
    pub periodic_announcement_ms: u32,
    /// `-v e|d`. `true` for `d`.
    pub verbose: bool,
    /// `-f`, deadline period in seconds. 0 = off.
    pub deadline_secs: u32,
    /// `-i`, time-based-filter interval in seconds. 0 = off.
    pub time_based_filter_secs: u32,
    /// `-s`, ownership strength. `-1` = SHARED.
    pub ownership_strength: i32,
    /// `--key-on-union-discriminator`. The front-end axis that decides
    /// whether `key="true"` on a `<discriminator>` — `@key` on a union
    /// discriminator in IDL — is carried into the model.
    ///
    /// Not a flag the suite's own C client has. It is here because the
    /// suite's `union_uint32_one_key` declares the construct and no
    /// measured front end accepts it: the default reports the
    /// declaration and the case ends in `TOPIC_NOT_CREATED`, and
    /// `carried` loads it and lets §7.2.4.4.4.5 give the
    /// `INCONSISTENT_TOPIC` the case expects.
    pub key_on_union_discriminator: KeyOnUnionDiscriminator,
    /// `--defaulted-discriminator-member`. The codec axis that decides
    /// what a union's member becomes when `@try_construct(use_default)`
    /// defaulted its discriminator.
    ///
    /// Not a flag the suite's own C client has either, and here for the
    /// same kind of reason: the default is Table 19's member step, which
    /// is what RTI Connext and OpenDDS were measured handing back, and
    /// `stream-value` is what `tryc_union_enum_disc_2` compares against.
    pub defaulted_discriminator_member: DefaultedDiscriminatorMember,
    /// `--key-member-bounds`. The assignability axis over Table 19's
    /// three key-member bullets. The default enforces them, which is
    /// what the clause says and what Cyclone does; `ignored` is what
    /// the suite's four `struct_key_*` cases expect and what RTI,
    /// OpenDDS, CoreDX and HDDS were measured doing. A runtime setting
    /// (`xtypes.key_member_bounds`), not a QoS.
    pub key_member_bounds: KeyMemberBounds,
    /// `--strong-assignability`. The axis over XTypes 1.3 §7.2.4.3: by
    /// default an element or member type that is not delimited must be
    /// MINIMAL-equivalent, member names included; `names_ignored` is what
    /// the suite's `SFinal[10]_S[20]_SFinalAlt[10]_S[20]` and
    /// `seq(SFinal,10)_seq(SFinalAlt,10)` expect. A runtime setting
    /// (`xtypes.strong_assignability`), not a QoS; without the flag the
    /// configuration's value stands.
    pub strong_assignability: Option<StrongAssignability>,
    /// `--runtime-config <file>`. A runtime configuration file for this
    /// run instead of the standard paths (`/etc/zerodds/zerodds.yaml`
    /// and its drop-ins) — the way a diagnostic sink or a dialect
    /// setting is given to one probe without touching the host.
    pub runtime_config: Option<String>,
    /// `--minimal-hash-camp cyclone|rti`. The vendor camp for the
    /// minimal `TypeObject` EquivalenceHash (XTypes 1.3 §7.3.4.5;
    /// `docs/omg-xtypes-issues.md` §C2). `cyclone` (default) writes no
    /// trailing member octet; `rti` writes one, matching RTI Connext
    /// and CoreDX. `coredx` is an alias for `rti`.
    pub minimal_hash_camp: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            domain_id: 0,
            topic_name: None,
            type_name: None,
            type_folder: None,
            type_file: None,
            data_folder: None,
            data_file: None,
            publish: false,
            subscribe: false,
            reliable: true,
            history_depth: None,
            partition: None,
            durability: None,
            key_on_union_discriminator: KeyOnUnionDiscriminator::default(),
            defaulted_discriminator_member: DefaultedDiscriminatorMember::default(),
            key_member_bounds: KeyMemberBounds::default(),
            strong_assignability: None,
            minimal_hash_camp: false,
            runtime_config: None,
            data_representation: 2,
            print_writer_samples: false,
            force_type_validation: TriState::Absent,
            ignore_member_names: TriState::Absent,
            ignore_seq_bounds: TriState::Absent,
            ignore_str_bounds: TriState::Absent,
            prevent_type_widening: TriState::Absent,
            allow_type_coercion: TriState::Absent,
            disable_type_info: false,
            type_object_version: 2,
            print_typeid: false,
            periodic_announcement_ms: 0,
            verbose: false,
            deadline_secs: 0,
            time_based_filter_secs: 0,
            ownership_strength: -1,
        }
    }
}

/// Everything the command line can be wrong about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// A flag that takes a value came last.
    MissingValue(String),
    /// A flag's value is not one this flag accepts.
    BadValue {
        /// The flag.
        flag: String,
        /// What arrived.
        value: String,
        /// What the flag takes.
        expected: &'static str,
    },
    /// A token that is not a flag of this suite.
    Unknown(String),
    /// The combination cannot be run.
    Incomplete(&'static str),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::BadValue {
                flag,
                value,
                expected,
            } => write!(f, "{flag}: expected {expected}, got '{value}'"),
            Self::Unknown(token) => write!(f, "unknown argument '{token}'"),
            Self::Incomplete(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for CliError {}

/// Fetches the value that follows a flag.
fn value(args: &[String], i: &mut usize, flag: &str) -> Result<String, CliError> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| CliError::MissingValue(flag.to_owned()))
}

/// Parses one argument vector, without `argv[0]`.
///
/// # Errors
/// [`CliError`], naming the token.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut o = Options::default();
    let mut i = 0usize;
    while i < args.len() {
        let arg = args[i].clone();
        match arg.as_str() {
            "-d" => {
                let v = value(args, &mut i, "-d")?;
                o.domain_id = v.parse().map_err(|_| CliError::BadValue {
                    flag: "-d".to_owned(),
                    value: v.clone(),
                    expected: "a domain id",
                })?;
            }
            "-t" => o.topic_name = Some(value(args, &mut i, "-t")?),
            "-y" => o.type_name = Some(value(args, &mut i, "-y")?),
            "-p" => o.partition = Some(value(args, &mut i, "-p")?),
            "-P" => o.publish = true,
            "-S" => o.subscribe = true,
            "-b" => o.reliable = false,
            "-r" => o.reliable = true,
            "-w" => o.print_writer_samples = true,
            "-k" => {
                let v = value(args, &mut i, "-k")?;
                o.history_depth = Some(v.parse().map_err(|_| CliError::BadValue {
                    flag: "-k".to_owned(),
                    value: v.clone(),
                    expected: "a history depth",
                })?);
            }
            "-f" => {
                let v = value(args, &mut i, "-f")?;
                o.deadline_secs = v.parse().map_err(|_| CliError::BadValue {
                    flag: "-f".to_owned(),
                    value: v.clone(),
                    expected: "a deadline in seconds",
                })?;
            }
            "-i" => {
                let v = value(args, &mut i, "-i")?;
                o.time_based_filter_secs = v.parse().map_err(|_| CliError::BadValue {
                    flag: "-i".to_owned(),
                    value: v.clone(),
                    expected: "an interval in seconds",
                })?;
            }
            "-s" => {
                let v = value(args, &mut i, "-s")?;
                o.ownership_strength = v.parse().map_err(|_| CliError::BadValue {
                    flag: "-s".to_owned(),
                    value: v.clone(),
                    expected: "an ownership strength",
                })?;
            }
            "-D" => {
                let v = value(args, &mut i, "-D")?;
                let c = v.chars().next().unwrap_or(' ');
                if !matches!(c, 'v' | 'l' | 't' | 'p') {
                    return Err(CliError::BadValue {
                        flag: "-D".to_owned(),
                        value: v.clone(),
                        expected: "v, l, t or p",
                    });
                }
                o.durability = Some(c);
            }
            "-x" => {
                let v = value(args, &mut i, "-x")?;
                o.data_representation = match v.as_str() {
                    "1" => 1,
                    "2" => 2,
                    _ => {
                        return Err(CliError::BadValue {
                            flag: "-x".to_owned(),
                            value: v.clone(),
                            expected: "1 or 2",
                        });
                    }
                };
            }
            "-v" => {
                let v = value(args, &mut i, "-v")?;
                o.verbose = v.starts_with('d');
            }
            "--type-folder" => o.type_folder = Some(value(args, &mut i, "--type-folder")?),
            "--type-file" => o.type_file = Some(value(args, &mut i, "--type-file")?),
            "--data-folder" => o.data_folder = Some(value(args, &mut i, "--data-folder")?),
            "--data-file" => o.data_file = Some(value(args, &mut i, "--data-file")?),
            "--force-type-validation" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.force_type_validation = TriState::parse(&arg, &v)?;
            }
            "--ignore-member-names" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.ignore_member_names = TriState::parse(&arg, &v)?;
            }
            "--ignore-seq-bounds" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.ignore_seq_bounds = TriState::parse(&arg, &v)?;
            }
            "--ignore-str-bounds" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.ignore_str_bounds = TriState::parse(&arg, &v)?;
            }
            "--prevent-type-widening" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.prevent_type_widening = TriState::parse(&arg, &v)?;
            }
            "--allow-type-coercion" => {
                let v = value(args, &mut i, arg.as_str())?;
                o.allow_type_coercion = TriState::parse(&arg, &v)?;
            }
            "--disable-type-info" => o.disable_type_info = true,
            "--print-typeid" => o.print_typeid = true,
            "--type-object-version" => {
                let v = value(args, &mut i, "--type-object-version")?;
                o.type_object_version = match v.as_str() {
                    "1" => 1,
                    "2" => 2,
                    _ => {
                        return Err(CliError::BadValue {
                            flag: "--type-object-version".to_owned(),
                            value: v.clone(),
                            expected: "1 or 2",
                        });
                    }
                };
            }
            "--key-on-union-discriminator" => {
                let v = value(args, &mut i, "--key-on-union-discriminator")?;
                o.key_on_union_discriminator =
                    KeyOnUnionDiscriminator::parse(&v).ok_or_else(|| CliError::BadValue {
                        flag: "--key-on-union-discriminator".to_owned(),
                        value: v.clone(),
                        expected: "refused or carried",
                    })?;
            }
            "--defaulted-discriminator-member" => {
                let v = value(args, &mut i, "--defaulted-discriminator-member")?;
                o.defaulted_discriminator_member = DefaultedDiscriminatorMember::parse(&v)
                    .ok_or_else(|| CliError::BadValue {
                        flag: "--defaulted-discriminator-member".to_owned(),
                        value: v.clone(),
                        expected: "type-default or stream-value",
                    })?;
            }
            "--key-member-bounds" => {
                let v = value(args, &mut i, "--key-member-bounds")?;
                o.key_member_bounds =
                    KeyMemberBounds::parse(&v).ok_or_else(|| CliError::BadValue {
                        flag: "--key-member-bounds".to_owned(),
                        value: v.clone(),
                        expected: "enforced or ignored",
                    })?;
            }
            "--strong-assignability" => {
                let v = value(args, &mut i, "--strong-assignability")?;
                o.strong_assignability =
                    Some(
                        StrongAssignability::parse(&v).ok_or_else(|| CliError::BadValue {
                            flag: "--strong-assignability".to_owned(),
                            value: v.clone(),
                            expected: "equivalence or names_ignored",
                        })?,
                    );
            }
            "--runtime-config" => {
                o.runtime_config = Some(value(args, &mut i, "--runtime-config")?);
            }
            "--minimal-hash-camp" => {
                let v = value(args, &mut i, "--minimal-hash-camp")?;
                o.minimal_hash_camp = match v.as_str() {
                    "cyclone" | "fastdds" | "spec" | "specification" => false,
                    "rti" | "coredx" => true,
                    _ => {
                        return Err(CliError::BadValue {
                            flag: "--minimal-hash-camp".to_owned(),
                            value: v.clone(),
                            expected: "cyclone or rti",
                        });
                    }
                };
            }
            "--periodic-announcement" => {
                let v = value(args, &mut i, "--periodic-announcement")?;
                o.periodic_announcement_ms = v.parse().map_err(|_| CliError::BadValue {
                    flag: "--periodic-announcement".to_owned(),
                    value: v.clone(),
                    expected: "a period in milliseconds",
                })?;
            }
            other => return Err(CliError::Unknown(other.to_owned())),
        }
        i += 1;
    }
    Ok(o)
}

impl Options {
    /// The suite's own `validate()`.
    ///
    /// # Errors
    /// [`CliError::Incomplete`] with the suite's own wording.
    pub fn validate(&self) -> Result<(), CliError> {
        if self.print_typeid {
            // The typeid mode creates no entities and needs only a type.
            if self.type_name.is_none() {
                return Err(CliError::Incomplete("please specify type name [-y]"));
            }
            if self.type_folder.is_none() && self.type_file.is_none() {
                return Err(CliError::Incomplete(
                    "please provide the types via --type-folder or --type-file",
                ));
            }
            return Ok(());
        }
        if self.topic_name.is_none() {
            return Err(CliError::Incomplete("topic name unspecified [-t]"));
        }
        if self.type_name.is_none() {
            return Err(CliError::Incomplete("please specify type name [-y]"));
        }
        if !self.publish && !self.subscribe {
            return Err(CliError::Incomplete(
                "please specify publish [-P] or subscribe [-S]",
            ));
        }
        if self.publish && self.subscribe {
            return Err(CliError::Incomplete(
                "please specify only one of: publish [-P] or subscribe [-S]",
            ));
        }
        if self.type_folder.is_none() && self.type_file.is_none() {
            return Err(CliError::Incomplete(
                "please provide the types via --type-folder or --type-file",
            ));
        }
        Ok(())
    }
}

/// `print_usage` of `src/cxx/test_main.cxx`.
#[must_use]
pub fn usage(prog: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("{prog}: \n"));
    for line in [
        "   -d <int>        : domain id (default: 0)",
        "   -b              : BEST_EFFORT reliability",
        "   -r              : RELIABLE reliability",
        "   -k <depth>      : keep history depth [0: KEEP_ALL]",
        "   -f <interval>   : set a 'deadline' with interval (seconds) [0: OFF]",
        "   -i <interval>   : apply 'time based filter' with interval (seconds) [0: OFF]",
        "   -s <int>        : set ownership strength [-1: SHARED]",
        "   -t <topic_name> : set the topic name",
        "   -y <type_name>  : set the type name",
        "   -p <partition>  : set a 'partition' string",
        "   -D [v|l|t|p]    : set durability",
        "   -P              : publish samples",
        "   -S              : subscribe samples",
        "   -x [1|2]        : set data representation [1: XCDR, 2: XCDR2]",
        "   --type-folder <folder>: folder containing type definitions (eg: types)",
        "   --type-file <file>    : type definition file name without extension",
        "   --data-folder <folder>: folder containing data samples (eg: data)",
        "   --data-file <file>    : data sample file name without extension",
        "   -w              : print Publisher's samples",
        "   --force-type-validation [t|f|d]",
        "   --ignore-member-names [t|f|d]",
        "   --ignore-seq-bounds [t|f|d]",
        "   --ignore-str-bounds [t|f|d]",
        "   --prevent-type-widening [t|f|d]",
        "   --allow-type-coercion [t|f|d]",
        "   --disable-type-info",
        "   --type-object-version [1|2]",
        "   --print-typeid",
        "   --minimal-hash-camp [cyclone|rti]",
        "   --periodic-announcement <ms>",
        "   --key-on-union-discriminator [refused|carried]",
        "   --defaulted-discriminator-member [type-default|stream-value]",
        "   --key-member-bounds [enforced|ignored]",
        "   --strong-assignability [equivalence|names_ignored]",
        "   --runtime-config <file>",
        "   -v [e|d]        : set log message verbosity",
    ] {
        s.push_str(line);
        s.push('\n');
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn argv(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    /// The exact publisher line `interoperability_report.py` builds for
    /// `xtypes_v2_extensibility_test_suite_ext_final_struct_1`, taken
    /// from a run of the harness.
    #[test]
    fn the_harness_publisher_line_parses() {
        let o = parse(&argv(
            "-P -t test -y Test::struct_f1 --data-folder data --data-file struct_num_x1 \
             --type-folder types --type-file extensibility -x 2 --type-object-version 2",
        ))
        .expect("the harness line parses");
        o.validate().expect("and is complete");
        assert!(o.publish);
        assert!(!o.subscribe);
        assert_eq!(o.topic_name.as_deref(), Some("test"));
        assert_eq!(o.type_name.as_deref(), Some("Test::struct_f1"));
        assert_eq!(o.type_folder.as_deref(), Some("types"));
        assert_eq!(o.type_file.as_deref(), Some("extensibility"));
        assert_eq!(o.data_folder.as_deref(), Some("data"));
        assert_eq!(o.data_file.as_deref(), Some("struct_num_x1"));
        assert_eq!(o.data_representation, 2);
        assert_eq!(o.type_object_version, 2);
    }

    /// The subscriber line differs by `-S` and one tri-state.
    #[test]
    fn the_harness_subscriber_line_parses() {
        let o = parse(&argv(
            "-S -t test -y Test::struct_f2 --data-folder data --data-file struct_num_x1 \
             --ignore-member-names f --type-folder types --type-file extensibility \
             -x 2 --type-object-version 2",
        ))
        .expect("the harness line parses");
        o.validate().expect("and is complete");
        assert!(o.subscribe);
        assert_eq!(o.ignore_member_names, TriState::False);
    }

    /// `d` is not `f`: the suite passes both and expects different
    /// behaviour, XTypes 1.3 §7.6.3.4.1.
    #[test]
    fn the_default_tri_state_is_not_false() {
        assert_eq!(TriState::parse("--x", "d").unwrap().as_bool(), None);
        assert_eq!(TriState::parse("--x", "f").unwrap().as_bool(), Some(false));
        assert_eq!(TriState::parse("--x", "t").unwrap().as_bool(), Some(true));
    }

    /// `--strong-assignability` takes the two spellings of the setting and
    /// leaves the configuration's value when it is not given.
    #[test]
    fn the_strong_assignability_flag_reads_the_setting() {
        let o = parse(&argv(
            "-S -t test -y T --strong-assignability names_ignored",
        ))
        .unwrap();
        assert_eq!(
            o.strong_assignability,
            Some(StrongAssignability::NamesIgnored)
        );
        let o = parse(&argv("-S -t test -y T")).unwrap();
        assert_eq!(o.strong_assignability, None);
        assert!(parse(&argv("-S -t test -y T --strong-assignability loose")).is_err());
    }

    /// A value glued to its flag is refused rather than read as a name.
    #[test]
    fn a_glued_value_is_refused() {
        let e = parse(&argv("-P -t=test")).unwrap_err();
        assert_eq!(e, CliError::Unknown("-t=test".to_owned()));
    }

    /// `validate()` refuses both roles at once, as the suite does.
    #[test]
    fn publish_and_subscribe_together_are_refused() {
        let o = parse(&argv(
            "-P -S -t test -y Test::struct_f1 --type-folder types --type-file extensibility",
        ))
        .unwrap();
        assert!(o.validate().is_err());
    }

    /// The other suite's flags. `dds-rtps`' client spells the same two
    /// roles `-P`/`-S`, and this one does too — but its `-t` is
    /// `--type-file` here and its `-d` is a *different* domain flag. The
    /// one token that means something else in the two suites is `-D`
    /// (durability here, nothing there), and it is read as this suite's.
    #[test]
    fn durability_is_read_as_this_suites_flag() {
        let o = parse(&argv("-P -t test -y T::x -D l --type-file f")).unwrap();
        assert_eq!(o.durability, Some('l'));
    }
}
