// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CLI flag surface for the OMG `dds-rtps` interoperability-test-suite
//! `shape_main` reference client.
//!
//! The flag set + short/long spellings below are transcribed **verbatim**
//! from `github.com/omg-dds/dds-rtps` `README.md` ("Shape Application
//! parameters" section), cross-validated against the two committed Rust
//! reference implementations in that repo (`srcRs/DustDDS/src/main.rs`,
//! `srcRs/HDDS/src/main.rs`) — both independently reproduce the same flag
//! names, so this is the canonical set the `interoperability_report.py` /
//! `test_suite.py` harness feeds via `'apps': ['-P -t Square -r ...', ...]`
//! parameter strings.
//!
//! A handful of short-letter aliases (`-i`, `-l`, `-n`, `-I`, `-E`, `-M`,
//! `-C`, `-T`, `-O`, `-H`, `-B`, `-K`) are **not** in the README table
//! itself but appear in both Rust reference implementations' `clap`/manual
//! parsers; we mirror them for drop-in compatibility with hand-typed
//! invocations, but the test suite always uses the long form.
//!
//! `--cft <expr>` is a vendor extension (mirrored from DustDDS/HDDS, not
//! in the README table) needed so the harness's
//! `'failed to create content filtered topic'` pexpect alternative has
//! something to trigger against.

use clap::{Parser, ValueEnum};

/// `-D [v|l|t|p]` — durability kind letters, exactly as in the README.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum DurabilityArg {
    /// `v` — VOLATILE.
    V,
    /// `l` — TRANSIENT_LOCAL.
    L,
    /// `t` — TRANSIENT.
    T,
    /// `p` — PERSISTENT.
    P,
}

/// `--access-scope [i|t|g]` — PresentationQosPolicy.access_scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum AccessScopeArg {
    /// `i` — INSTANCE (default).
    I,
    /// `t` — TOPIC.
    T,
    /// `g` — GROUP.
    G,
}

/// `--final-instance-state [u|d]` — action after the DataWriter's last
/// write, before the writer is torn down.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum FinalInstanceStateArg {
    /// `u` — `unregister_instance`.
    U,
    /// `d` — `dispose`.
    D,
}

/// `-v [e|d]` — log verbosity. Accepted for CLI compatibility; ZeroDDS
/// does not have a separate shape_main-local log-verbosity knob, so this
/// only gates a handful of `eprintln!` diagnostics in this binary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum VerbosityArg {
    /// `e` — ERROR only.
    E,
    /// `d` — DEBUG (verbose).
    D,
}

/// OMG `dds-rtps` `shape_main` — ZeroDDS reference client.
///
/// Flag documentation strings below are the README's own wording (kept
/// verbatim where practical) so `--help` output stays diffable against
/// the upstream table.
#[derive(Parser, Debug, Clone)]
#[command(name = "shape_main", author, version, about, long_about = None)]
pub struct Options {
    /// `-v [e|d]`: set log message verbosity.
    #[arg(short = 'v', value_enum)]
    pub verbosity: Option<VerbosityArg>,

    /// `-P`: publish samples.
    #[arg(short = 'P')]
    pub publish: bool,

    /// `-S`: subscribe samples.
    #[arg(short = 'S')]
    pub subscribe: bool,

    /// `-d <int>`: domain id (default: 0).
    #[arg(short = 'd', default_value_t = 0)]
    pub domain_id: i32,

    /// `-b`: BEST_EFFORT reliability.
    #[arg(short = 'b')]
    pub best_effort: bool,

    /// `-r`: RELIABLE reliability.
    #[arg(short = 'r')]
    pub reliable: bool,

    /// `-k <depth>`: keep history depth (`0` = KEEP_ALL).
    #[arg(short = 'k', allow_negative_numbers = false)]
    pub history_depth: Option<i32>,

    /// `-f <interval>`: DEADLINE interval in ms (`0` = OFF).
    #[arg(short = 'f', default_value_t = 0)]
    pub deadline_ms: i64,

    /// `-s <strength>`: OwnershipStrength (`-1` = SHARED ownership).
    #[arg(short = 's', allow_negative_numbers = true, default_value_t = -1)]
    pub ownership_strength: i32,

    /// `-t <topic_name>`: the topic name.
    #[arg(short = 't', default_value = "Square")]
    pub topic_name: String,

    /// `-c <color>`: color to publish (filter, if subscriber).
    #[arg(short = 'c')]
    pub color: Option<String>,

    /// `-p <partition>`: a 'partition' string.
    #[arg(short = 'p')]
    pub partition: Option<String>,

    /// `-D [v|l|t|p]`: durability.
    #[arg(short = 'D', value_enum)]
    pub durability: Option<DurabilityArg>,

    /// `-x [1|2]`: data representation (1: XCDR, 2: XCDR2).
    #[arg(short = 'x')]
    pub data_representation: Option<u8>,

    /// `-w`: print Publisher's samples.
    #[arg(short = 'w')]
    pub print_writer_samples: bool,

    /// `-z <int>`: shapesize (`0` = increase the size for every sample).
    #[arg(short = 'z', default_value_t = 30)]
    pub shapesize: i32,

    /// `-R`: use `read()` instead of `take()`.
    #[arg(short = 'R')]
    pub use_read: bool,

    /// `--write-period <ms>`: waiting period between `write()` calls.
    #[arg(long = "write-period", default_value_t = 33)]
    pub write_period_ms: u64,

    /// `--read-period <ms>`: waiting period between `read()`/`take()` calls.
    #[arg(long = "read-period", default_value_t = 100)]
    pub read_period_ms: u64,

    /// `--time-filter <interval>`: TIME_BASED_FILTER interval in ms
    /// (`0` = OFF). Short alias `-i` (DustDDS convention).
    #[arg(long = "time-filter", short = 'i', default_value_t = 0)]
    pub time_filter_ms: i64,

    /// `--lifespan <int>`: LIFESPAN of a sample in ms. Short alias `-l`.
    #[arg(long = "lifespan", short = 'l', default_value_t = 0)]
    pub lifespan_ms: i64,

    /// `--num-iterations <int>`: iterations of the main loop before exit
    /// (default: infinite). Short alias `-n`.
    #[arg(long = "num-iterations", short = 'n')]
    pub num_iterations: Option<u64>,

    /// `--num-instances <int>`: number of instances a DataWriter writes
    /// (color, color1, color2, ...). Short alias `-I`.
    #[arg(long = "num-instances", short = 'I', default_value_t = 1)]
    pub num_instances: u32,

    /// `--num-topics <int>`: number of topics created (Square, Square1,
    /// Square2, ...), one DataWriter/DataReader each. Short alias `-E`.
    #[arg(long = "num-topics", short = 'E', default_value_t = 1)]
    pub num_topics: u32,

    /// `--final-instance-state [u|d]`: action after the DataWriter
    /// finishes, before it is torn down. Short alias `-M`.
    #[arg(long = "final-instance-state", short = 'M', value_enum)]
    pub final_instance_state: Option<FinalInstanceStateArg>,

    /// `--access-scope [i|t|g]`: Presentation.access_scope. Short alias
    /// `-C`.
    #[arg(long = "access-scope", short = 'C', value_enum)]
    pub access_scope: Option<AccessScopeArg>,

    /// `--coherent`: Presentation.coherent_access = true. Short alias
    /// `-T`.
    #[arg(long = "coherent", short = 'T')]
    pub coherent: bool,

    /// `--ordered`: Presentation.ordered_access = true. Short alias `-O`.
    #[arg(long = "ordered", short = 'O')]
    pub ordered: bool,

    /// `--coherent-sample-count <int>`: samples per coherent set, per
    /// DataWriter+instance. Short alias `-H`.
    #[arg(long = "coherent-sample-count", short = 'H', default_value_t = 0)]
    pub coherent_sample_count: u32,

    /// `--additional-payload-size <bytes>`: extra bytes appended to each
    /// sample (large-data tests). Short alias `-B`.
    ///
    /// **Known gap** (see crate README): ZeroDDS's shared `ShapeType`
    /// wire type (`crates/dcps/src/interop.rs`) does not carry an
    /// `additional_payload_size` field — this binary intentionally does
    /// not touch that shared file (out of scope / other agents'
    /// territory), so this flag is accepted and stored but does not
    /// currently affect the wire payload.
    #[arg(long = "additional-payload-size", short = 'B', default_value_t = 0)]
    pub additional_payload_size: u32,

    /// `--take-read`: use `take()`/`read()` instead of
    /// `take_next_instance()`/`read_next_instance()`. Short alias `-K`.
    #[arg(long = "take-read", short = 'K')]
    pub take_read: bool,

    /// `--periodic-announcement <ms>`: participant announcement period
    /// (`0` = off / use the ZeroDDS runtime default SPDP cadence).
    #[arg(long = "periodic-announcement", default_value_t = 0)]
    pub periodic_announcement_ms: u32,

    /// `--cft <expr>`: content-filter expression for the subscriber's
    /// reader (vendor extension, mirrors DustDDS/HDDS `--cft`; not in
    /// the README flag table). SQL-92 subset per `zerodds-sql-filter`
    /// (DDS-DCPS Annex B).
    #[arg(long = "cft")]
    pub cft: Option<String>,
}

impl Options {
    /// Resolved color: `-c` if given, else the ShapesDemo canonical
    /// default `"BLUE"`.
    #[must_use]
    pub fn resolved_color(&self) -> &str {
        self.color.as_deref().unwrap_or("BLUE")
    }
}
