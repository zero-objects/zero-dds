// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ShapeType_main` — ZeroDDS reference client for the OMG `dds-security`
//! interoperability test suite (`github.com/omg-dds/dds-security`).
//!
//! BUILD + LOCAL-VALIDATE phase artifact. This binary is **not**
//! submitted anywhere — no CLA PR, no source PR, no release asset, no
//! push to `github.com/omg-dds`. That step is Sandra's.
//!
//! It is a CLI + protocol shim over ZeroDDS's existing DCPS API
//! (`crates/dcps`), QoS machinery (`crates/qos`) and DDS-Security stack
//! (`crates/security*`). No new wire, CDR, QoS or crypto code lives
//! here. See `README.md` for the flag-coverage table, the exact stdout
//! strings, and what the security half needs that the tree does not
//! have.
//!
//! Where the suite prints, this prints the same string. The `dds-rtps`
//! suite has a `pexpect` harness that matches stdout literally; this one
//! does not — its README validates by eye — so "the same string" here is
//! about being readable beside another vendor's transcript, not about a
//! regex. The strings are still transcribed rather than paraphrased,
//! because a transcript that has to be diffed is worth more than one
//! that reads better.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod cli;
mod security;
mod shape_type;

use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

use clap::Parser;

use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, PartitionQosPolicy, PublisherQos, SubscriberQos,
};
use zerodds_dcps::{
    DataReader, DataWriter, DomainParticipant, DomainParticipantFactory, DomainParticipantQos,
    Topic, TopicQos,
};
use zerodds_qos::Duration;
use zerodds_security_runtime::{LogLevel, LoggingPlugin, SecurityBundle};

use crate::cli::Options;
use crate::shape_type::ShapeType;

type BoxError = Box<dyn std::error::Error>;

/// The suite's `send_period` — `Duration_t send_period = {0, 100000000}`.
const SEND_PERIOD: StdDuration = StdDuration::from_millis(100);

/// The suite's `Logger` at `Verbosity::DEBUG`.
///
/// `log_message` writes to `std::cout`, so this writes to stdout too.
/// The `dds-rtps` client puts its diagnostics on stderr because a
/// `pexpect` harness matches its stdout and the two verbosity levels
/// have to produce the same bytes there; this suite has no such harness
/// and its own client prints diagnostics to stdout, so following the
/// suite is the choice that makes the two transcripts comparable.
macro_rules! debugln {
    ($opts:expr, $($arg:tt)*) => {
        if $opts.debug_verbosity() {
            println!("{}", format_args!($($arg)*));
        }
    };
}

/// The suite's `Logger` at `Verbosity::ERROR` — also `std::cout`.
macro_rules! errorln {
    ($($arg:tt)*) => {
        println!("{}", format_args!($($arg)*));
    };
}

fn main() -> ExitCode {
    let opts = match Options::try_parse_from(cli::normalize_args(std::env::args())) {
        Ok(o) => o,
        Err(e) => {
            // clap renders `--help`/`--version` as an "error" it wants
            // printed on stdout with status 0; a real parse failure goes
            // to stderr with a failure status. The suite's own
            // `print_usage` path returns -1 either way, which is a wart
            // we are not copying.
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
        }
    };

    // The suite's `main()` announces every default it applies, on
    // stderr, before it runs. Reproduced verbatim: an operator reading
    // two clients' transcripts side by side is reading these lines
    // first.
    if opts.use_security() {
        if opts.governance.is_none() {
            eprintln!(
                "Info: \"-governance\" unspecified. Default to \"{}\"",
                cli::DEFAULT_GOVERNANCE_FILE
            );
        }
        if opts.permissions.is_none() {
            eprintln!(
                "Info: \"-permissions\" unspecified. Default to \"{}\"",
                cli::DEFAULT_PERMISSIONS_FILE
            );
        }
        if opts.key_establishment_algorithm.is_none() {
            eprintln!(
                "Info: \"-kest\" unspecified. Default to \"{}\"",
                cli::DEFAULT_KEY_EST_ALGORITHM
            );
        }
    }
    if opts.publish_topic.is_none() && opts.subscribe_topic.is_none() {
        eprintln!(
            "Info: Neither \"-pub\" nor \"-sub\" was specified. Default to publish \"{}\" AND \
             subscribe \"{}\"",
            cli::DEFAULT_TOPIC,
            cli::DEFAULT_TOPIC
        );
    }
    if opts.color.is_none() {
        eprintln!(
            "Info: \"-color\" unspecified. Default to: \"{}\"",
            cli::DEFAULT_COLOR
        );
    }
    if opts.domain_id.is_none() {
        eprintln!("Info: \"-domain\" unspecified. Default to: 0");
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        if let Err(e) = ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        }) {
            eprintln!("Info: signal handler not installed: {e}");
        }
    }

    let (pub_topic_name, sub_topic_name) = opts.topics();
    if let Some(name) = &pub_topic_name {
        println!("Publishing:  '{name}'");
    }
    if let Some(name) = &sub_topic_name {
        println!("Subscribing: '{name}'");
    }

    match run(&opts, pub_topic_name, sub_topic_name, &stop) {
        Ok(()) => {
            eprintln!("Done...");
            ExitCode::SUCCESS
        }
        Err(e) => {
            errorln!("failed: {e}");
            ExitCode::FAILURE
        }
    }
}

// ============================================================================
// Security
// ============================================================================

/// `-logging`: the DDS-Security §8.6 logging plugin, printing to stdout
/// in the same place the suite's own `-logging` flag makes its vendor
/// print.
/// Enabled at the most verbose level, as before Table 46 came in;
/// `set_log_options`, `enable_logging` and `set_listener` act on the
/// shared `LogControl` (DDS-Security 1.2 §9.6.2.2).
struct StdoutSecurityLogger {
    control: zerodds_security_logging::LogControl,
}

impl StdoutSecurityLogger {
    fn new() -> Self {
        Self {
            control: zerodds_security_logging::LogControl::enabled_at(LogLevel::Trace, false),
        }
    }
}

impl LoggingPlugin for StdoutSecurityLogger {
    fn set_log_options(
        &self,
        options: zerodds_security::LogOptions,
    ) -> Result<(), zerodds_security::SecurityError> {
        // stdout only: no DataWriter to distribute to.
        self.control
            .set_log_options(options, false)
            .map_err(zerodds_security::SecurityError::from)
    }

    fn enable_logging(&self) -> Result<(), zerodds_security::SecurityError> {
        self.control
            .enable_logging()
            .map_err(zerodds_security::SecurityError::from)
    }

    fn set_listener(
        &self,
        listener: Option<std::sync::Arc<dyn zerodds_security::LoggerListener>>,
    ) -> Result<(), zerodds_security::SecurityError> {
        self.control
            .set_listener(listener)
            .map_err(zerodds_security::SecurityError::from)
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        if self
            .control
            .admit(level, participant, category, message)
            .is_none()
        {
            return;
        }
        println!("[security {level:?}] {category}: {message}");
        let _ = std::io::stdout().flush();
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:DDS_LogTopic"
    }
}

// ============================================================================
// Participant, topics, endpoints
// ============================================================================

/// Creates the participant the regular way: the `dds.sec.*` property
/// list goes on the `DomainParticipantQos` and the runtime does the
/// rest.
///
/// DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 — the plugins are selected and
/// configured through the participant's `PropertyQosPolicy`, and
/// `crates/dcps` reads it: `file:` URIs resolved, the one
/// `SecurityProfile` built, §8.4.2.9.3 `check_create_participant` run,
/// `dds.sec.auth.key_establishment_algorithm` applied, the handshake
/// driver armed on the profile the gate holds. An incomplete or
/// unreadable configuration fails this call and names the property; it
/// never produces a plaintext participant.
///
/// The one thing that is not a property is the §8.6 logging sink, which
/// is a callback and not a string: `-logging` hands the runtime a
/// logger-only `SecurityBundle`. That bundle carries no profile, so the
/// property path still runs and wires this sink into the PKI plugin it
/// builds.
fn create_participant(opts: &Options) -> Result<DomainParticipant, BoxError> {
    let domain_id = opts.resolved_domain_id();
    let qos = DomainParticipantQos {
        property: security::property_qos(opts),
        ..DomainParticipantQos::default()
    };

    debugln!(opts, "Running create_participant() function");
    for (name, value) in qos.property.iter() {
        debugln!(opts, "Security property: {name} = {value}");
    }
    if opts.use_security() && opts.accept_x509v1_identity_certs {
        println!(
            "Info: accepting X.509 v1 identity certificates — a deviation from \
             DDS-Security 1.2 §10.3.1.3, switched on by -acceptX509v1IdentityCerts. \
             Signature, validity, chain and subject are still checked."
        );
    }

    let mut builder = DomainParticipantFactory::create(domain_id).with_qos(qos);
    if opts.logging {
        builder = builder.with_security(
            SecurityBundle::builder()
                .logging_plugin(Box::new(StdoutSecurityLogger::new()))
                .build(),
        );
    }
    let participant = builder.build()?;
    debugln!(opts, "Participant created");
    Ok(participant)
}

fn create_topic(
    participant: &DomainParticipant,
    topic_name: &str,
) -> Result<Topic<ShapeType>, BoxError> {
    println!("Create topic: {topic_name}");
    Ok(participant.create_topic::<ShapeType>(topic_name, TopicQos::default())?)
}

fn liveliness_lease(period_secs: f32) -> Option<Duration> {
    if period_secs <= 0.0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let millis = (period_secs * 1000.0) as i32;
    Some(Duration::from_millis(millis))
}

fn partition_policy(partition: Option<&str>) -> PartitionQosPolicy {
    match partition {
        Some(name) => PartitionQosPolicy {
            names: vec![name.to_string()],
        },
        None => PartitionQosPolicy::default(),
    }
}

fn create_writer(
    opts: &Options,
    participant: &DomainParticipant,
    topic: &Topic<ShapeType>,
    topic_name: &str,
) -> Result<DataWriter<ShapeType>, BoxError> {
    debugln!(opts, "Running create_writer() function");
    let publisher = participant.create_publisher(PublisherQos {
        partition: partition_policy(opts.partition.as_deref()),
        ..PublisherQos::default()
    });
    let mut qos = DataWriterQos::default();
    if let Some(lease) = liveliness_lease(opts.liveliness_period) {
        qos.liveliness.lease_duration = lease;
    }
    println!(
        "Create writer for topic: {topic_name} color: {}",
        opts.resolved_color()
    );
    Ok(publisher.create_datawriter::<ShapeType>(topic, qos)?)
}

fn create_reader(
    opts: &Options,
    participant: &DomainParticipant,
    topic: &Topic<ShapeType>,
    topic_name: &str,
) -> Result<DataReader<ShapeType>, BoxError> {
    debugln!(opts, "Running create_reader() function");
    let subscriber = participant.create_subscriber(SubscriberQos {
        partition: partition_policy(opts.partition.as_deref()),
        ..SubscriberQos::default()
    });
    let mut qos = DataReaderQos::default();
    if let Some(lease) = liveliness_lease(opts.liveliness_period) {
        qos.liveliness.lease_duration = lease;
    }
    println!("Create reader for topic: {topic_name}");
    Ok(subscriber.create_datareader::<ShapeType>(topic, qos)?)
}

// ============================================================================
// The loop
// ============================================================================

/// The suite's sample: `x = 5*(sent_count%50)`, `y = (4 * (color[0] -
/// 'A')) % 250`, `shapesize = 30`, with `sent_count` already
/// incremented for this sample. Transcribed rather than invented, so
/// that two clients on the same colour draw the same shape in the same
/// place and a mismatch means something.
fn sample_for(color: &str, sent_count: i32) -> ShapeType {
    let first = color.as_bytes().first().copied().unwrap_or(b'A');
    let y = (4 * (i32::from(first) - i32::from(b'A'))).rem_euclid(250);
    ShapeType::new(color, 5 * (sent_count % 50), y, 30)
}

fn run(
    opts: &Options,
    pub_topic_name: Option<String>,
    sub_topic_name: Option<String>,
    stop: &Arc<AtomicBool>,
) -> Result<(), BoxError> {
    let participant = create_participant(opts)?;

    // The suite creates one topic when `-pub` and `-sub` name the same
    // one, which is the default case.
    let pub_topic = match &pub_topic_name {
        Some(name) => Some(create_topic(&participant, name)?),
        None => None,
    };
    let sub_topic = match (&sub_topic_name, &pub_topic_name, &pub_topic) {
        (Some(sub), Some(publ), Some(existing)) if sub == publ => Some(existing.clone()),
        (Some(sub), _, _) => Some(create_topic(&participant, sub)?),
        (None, _, _) => None,
    };

    let writer = match (&pub_topic, &pub_topic_name) {
        (Some(topic), Some(name)) => Some(create_writer(opts, &participant, topic, name)?),
        _ => None,
    };
    let reader = match (&sub_topic, &sub_topic_name) {
        (Some(topic), Some(name)) => Some(create_reader(opts, &participant, topic, name)?),
        _ => None,
    };

    let color = opts.resolved_color().to_string();
    let sub_label = sub_topic_name.clone().unwrap_or_default();

    let mut wait_count: i32 = 0;
    let mut sent_count: i32 = 0;
    let mut recv_count: i32 = 0;
    let mut next_send = SystemTime::now();

    while !stop.load(Ordering::SeqCst) {
        debugln!(
            opts,
            "\nLoop: wait count = {wait_count}, sent count = {sent_count}, received count = \
             {recv_count}\n"
        );
        wait_count += 1;

        if let Some(reader) = &reader {
            reader.drive_listeners();
            // The suite drains with `take_next_sample` until it returns
            // anything but OK; `take_with_info` is that drain in one
            // call. `valid_data == false` samples — dispose and
            // unregister notifications — are counted by neither client.
            if let Ok(samples) = reader.take_with_info() {
                for s in &samples {
                    if !s.info.valid_data {
                        continue;
                    }
                    recv_count += 1;
                    println!(
                        "{sub_label:<10} {:<10} {:03} {:03} [{}]",
                        s.data.color, s.data.x, s.data.y, s.data.shapesize
                    );
                    let _ = std::io::stdout().flush();
                }
            }
        }

        if let Some(writer) = &writer {
            writer.drive_listeners();
            let now = SystemTime::now();
            if now >= next_send {
                next_send = now + SEND_PERIOD;
                if opts.debug_verbosity() {
                    let stamp = now.duration_since(UNIX_EPOCH).unwrap_or_default();
                    println!(
                        "Time: {}:{} -- Sending sample: count = {sent_count}",
                        stamp.as_secs(),
                        stamp.subsec_nanos()
                    );
                }
                sent_count += 1;
                writer.write(&sample_for(&color, sent_count))?;
                let _ = std::io::stdout().flush();
            }
        }

        std::thread::sleep(SEND_PERIOD);
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// `ShapeType_main.cxx`'s own arithmetic, transcribed. `sent_count`
    /// is incremented before the sample is filled, so the first sample
    /// carries `x = 5`.
    #[test]
    fn the_sample_is_the_suites_own_arithmetic() {
        let first = sample_for("BLACK", 1);
        assert_eq!(first.x, 5);
        assert_eq!(first.y, 4, "'B' - 'A' = 1, times 4");
        assert_eq!(first.shapesize, 30);
        assert_eq!(first.color, "BLACK");

        // `5*(sent_count%50)` wraps at 50 iterations, back to 0.
        assert_eq!(sample_for("BLACK", 49).x, 245);
        assert_eq!(sample_for("BLACK", 50).x, 0);
        assert_eq!(sample_for("BLACK", 51).x, 5);
    }

    /// `y` is a function of the colour's first letter and stays inside
    /// the ShapesDemo canvas — `(4 * (c - 'A')) % 250`. A colour that
    /// sorts before 'A' would make the C client's `%` produce a negative
    /// y; `rem_euclid` keeps this one on the canvas instead of drawing
    /// off it, which is a deliberate divergence and the only one in this
    /// function.
    #[test]
    fn the_colour_decides_y_and_it_stays_on_the_canvas() {
        for color in ["BLACK", "RED", "GREEN", "BLUE", "YELLOW", "ORANGE"] {
            let y = sample_for(color, 1).y;
            assert!((0..250).contains(&y), "{color} put y at {y}");
        }
        assert_eq!(sample_for("A", 1).y, 0);
        assert_eq!(sample_for("!", 1).y, 122, "wrapped rather than negative");
    }

    /// `-livelinessPeriod` is seconds, and `0` (the suite's default)
    /// means "leave the QoS alone" rather than "a zero-second lease",
    /// which would expire the writer immediately.
    #[test]
    fn a_zero_liveliness_period_leaves_the_default_lease() {
        assert!(liveliness_lease(0.0).is_none());
        assert!(liveliness_lease(-1.0).is_none());
        assert_eq!(liveliness_lease(1.5), Some(Duration::from_millis(1500)));
    }

    /// `-partition` is one name, and no flag is no partition — not a
    /// partition named the empty string, which DDS 1.4 §2.2.3.13 treats
    /// as a real name that matches only the default partition.
    #[test]
    fn no_partition_flag_means_an_empty_partition_list() {
        assert!(partition_policy(None).names.is_empty());
        assert_eq!(partition_policy(Some("A")).names, vec!["A".to_string()]);
    }
}
