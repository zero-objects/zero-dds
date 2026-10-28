// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The builtin `DDS:Logging:DDS_LogTopic` plugin (DDS-Security 1.2 §10.6).
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin publishes through a `dyn` publisher the participant hands
//! it: this crate cannot name the DCPS writer type without depending on
//! `zerodds-dcps`, which depends on it.)
//!
//! The plugin is a [`LoggingPlugin`] sink that fills a
//! [`BuiltinLoggingTypeV2`] from every event at or above the configured level
//! and publishes it on the DDS topic `DDS:Security:LogTopicV2`
//! ([`zerodds_security::logging::TOPIC_LOG_TOPIC_V2`]); [`Self::publish_legacy`]
//! additionally publishes the DDS-Security 1.1 `DDS:Security:LogTopic` topic
//! with a [`BuiltinLoggingType`], on a second DataWriter (§10.6).
//!
//! The DDS side of the publish is behind [`LogTopicPublisher`]: this crate has
//! no DCPS dependency (the DCPS runtime depends on it), so the `DataWriter`
//! lives in `zerodds-dcps`, which implements the trait and encodes the types.
//!
//! # Field mapping (§10.6.1, §9.6.2.2.2)
//!
//! | Field | Value |
//! |---|---|
//! | `facility` | `0x0A` ([`FACILITY_SEC_AUTH`]) |
//! | `severity` | the [`LogLevel`] as a [`LoggingLevel`] |
//! | `timestamp` | the time of the `log` call |
//! | `hostname` | `HOSTNAME` / `COMPUTERNAME`, else empty |
//! | `hostip` | `ZERODDS_HOSTIP`, else empty |
//! | `appname` | `ZERODDS_LOG_APPNAME`, else the executable name |
//! | `procid` | the process id |
//! | `msgid` | the `category` argument |
//! | `message` | the `message` argument |
//! | `structured_data["DDS"]` | `guid`, `plugin_class`, `plugin_method` |

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use std::sync::Mutex;

use zerodds_security::error::SecurityError;
use zerodds_security::logging::{LogLevel, LogOptions, LoggerListener, LoggingPlugin};

pub use crate::control::LogOptionsError;
use crate::control::{LogControl, hex16, report};

/// Facility `0x0A` (10) — "Indicates sec/auth msgs" (DDS-Security 1.2 §10.6).
pub const FACILITY_SEC_AUTH: u8 = 0x0A;

/// `LoggingLevel` of the BuiltinLoggingType IDL (DDS-Security 1.2 §10.6).
///
/// The IDL leaves the bit bound at the enum default (32), so the wire form is
/// a `uint32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LoggingLevel {
    /// System is unusable. Should not continue use.
    Emergency = 0,
    /// Should be corrected immediately.
    Alert = 1,
    /// A failure in primary application.
    Critical = 2,
    /// General error conditions.
    Error = 3,
    /// May indicate future error if action not taken.
    Warning = 4,
    /// Unusual, but not erroneous event or condition.
    Notice = 5,
    /// Normal operational. Requires no action.
    Informational = 6,
    /// Detailed information.
    Debug = 7,
}

impl From<LogLevel> for LoggingLevel {
    fn from(level: LogLevel) -> Self {
        // `LogLevel` is `repr(u8)` with the same order and values as the IDL
        // `LoggingLevel` (DDS-Security 1.2 §9.6.2.1.1 / §10.6).
        match level {
            LogLevel::Emergency => Self::Emergency,
            LogLevel::Alert => Self::Alert,
            LogLevel::Critical => Self::Critical,
            LogLevel::Error => Self::Error,
            LogLevel::Warning => Self::Warning,
            LogLevel::Notice => Self::Notice,
            LogLevel::Informational => Self::Informational,
            // `LoggingLevel` has no enumerator for TRACE_LEVEL (§10.6); the
            // most detailed one it has stands for it.
            LogLevel::Debug | LogLevel::Trace => Self::Debug,
            // `LogLevel` is non-exhaustive; a level added later is at most as
            // detailed as the finest one `LoggingLevel` names.
            _ => Self::Debug,
        }
    }
}

/// `NameValuePair` of the BuiltinLoggingType IDL (DDS-Security 1.2 §10.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameValuePair {
    /// `string name`.
    pub name: String,
    /// `string value`.
    pub value: String,
}

/// `typedef sequence<NameValuePair> NameValuePairSeq` (DDS-Security 1.2 §10.6).
pub type NameValuePairSeq = Vec<NameValuePair>;

/// `LegacyTime_t` of the legacy BuiltinLoggingType (DDS-Security 1.2 §10.6).
///
/// `long sec` and `unsigned long nanosec`, seconds since
/// 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LegacyTime {
    /// Seconds since the epoch (`long`).
    pub sec: i32,
    /// Nanoseconds within the second (`unsigned long`).
    pub nanosec: u32,
}

/// `Time_t` of BuiltinLoggingTypeV2 (DDS-Security 1.2 §10.6).
///
/// `long long sec` and `unsigned long nanosec`, seconds since
/// 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Time {
    /// Seconds since the epoch (`long long`).
    pub sec: i64,
    /// Nanoseconds within the second (`unsigned long`).
    pub nanosec: u32,
}

impl Time {
    /// The current UTC time, with the nanosecond part the clock reports.
    #[must_use]
    pub fn now() -> Self {
        match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => Self {
                // `u64` seconds cannot leave `i64` until year 292 billion.
                sec: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
                nanosec: d.subsec_nanos(),
            },
            // A clock before the epoch is expressed as a time before 1970
            // rather than wrapped to a future one.
            Err(e) => {
                let d = e.duration();
                Self {
                    sec: -i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
                    nanosec: d.subsec_nanos(),
                }
            }
        }
    }
}

impl LegacyTime {
    /// [`Time::now`] narrowed to the legacy `long` seconds field.
    #[must_use]
    pub fn now() -> Self {
        let now = Time::now();
        Self {
            sec: i32::try_from(now.sec).unwrap_or(if now.sec < 0 { i32::MIN } else { i32::MAX }),
            nanosec: now.nanosec,
        }
    }
}

/// `@extensibility(APPENDABLE) struct BuiltinLoggingType`
/// (DDS-Security 1.2 §10.6, legacy DDS-Security 1.1 type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinLoggingType {
    /// `octet facility` — set to `0x0A`.
    pub facility: u8,
    /// `LoggingLevel severity`.
    pub severity: LoggingLevel,
    /// `LegacyTime_t timestamp`.
    pub timestamp: LegacyTime,
    /// `string hostname` — IP host name of originator.
    pub hostname: String,
    /// `string hostip` — IP address of originator.
    pub hostip: String,
    /// `string appname` — identify the device or application.
    pub appname: String,
    /// `string procid` — process name/ID for syslog system.
    pub procid: String,
    /// `string msgid` — identify the type of message.
    pub msgid: String,
    /// `string message` — free-form message.
    pub message: String,
    /// `map<string, NameValuePairSeq> structured_data`.
    pub structured_data: BTreeMap<String, NameValuePairSeq>,
}

/// `@extensibility(APPENDABLE) struct BuiltinLoggingTypeV2`
/// (DDS-Security 1.2 §10.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinLoggingTypeV2 {
    /// `octet facility` — set to `0x0A`.
    pub facility: u8,
    /// `LoggingLevel severity`.
    pub severity: LoggingLevel,
    /// `Time_t timestamp`.
    pub timestamp: Time,
    /// `string hostname` — IP host name of originator.
    pub hostname: String,
    /// `string hostip` — IP address of originator.
    pub hostip: String,
    /// `string appname` — identify the device or application.
    pub appname: String,
    /// `string procid` — process name/ID for syslog system.
    pub procid: String,
    /// `string msgid` — identify the type of message.
    pub msgid: String,
    /// `string message` — free-form message.
    pub message: String,
    /// `map<string, NameValuePairSeq> structured_data`.
    pub structured_data: BTreeMap<String, NameValuePairSeq>,
}

/// The host name, from `HOSTNAME` / `COMPUTERNAME`, else empty.
///
/// The spec makes `hostname` optional ("may populate", §9.6.2.2.2); an empty
/// string is the honest value when no source names the host, and not a made-up
/// name that would point an audit at the wrong machine.
fn host_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_default()
}

/// The originator IP, from `ZERODDS_HOSTIP`, else empty.
fn host_ip() -> String {
    std::env::var("ZERODDS_HOSTIP").unwrap_or_default()
}

/// The application name: `ZERODDS_LOG_APPNAME`, else the executable file name,
/// else empty.
fn app_name() -> String {
    if let Ok(name) = std::env::var("ZERODDS_LOG_APPNAME") {
        return name;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// The `plugin_class` / `plugin_method` pair for a `category` string.
///
/// The category is the plugin-specific event name the runtime passes
/// (`"auth.handshake.failed"`, `"access.create_datawriter.denied"`, …). The
/// first dotted segment names the plugin, the rest the method.
fn plugin_class_and_method(category: &str) -> (String, String) {
    let (class, method) = category.split_once('.').unwrap_or(("", category));
    let class = match class {
        "auth" => "Authentication",
        "access" => "AccessControl",
        "crypto" => "Cryptographic",
        "logging" => "Logging",
        "data" => "DataTagging",
        other => other,
    };
    (class.to_string(), method.to_string())
}

/// The `structured_data["DDS"]` entry a `log` call fills
/// (DDS-Security 1.2 §9.6.2.2.2, Table 47).
fn dds_structured_data(participant: [u8; 16], category: &str) -> NameValuePairSeq {
    let (plugin_class, plugin_method) = plugin_class_and_method(category);
    vec![
        NameValuePair {
            name: "guid".to_string(),
            value: hex16(participant),
        },
        NameValuePair {
            name: "plugin_class".to_string(),
            value: plugin_class,
        },
        NameValuePair {
            name: "plugin_method".to_string(),
            value: plugin_method,
        },
    ]
}

fn structured_data(participant: [u8; 16], category: &str) -> BTreeMap<String, NameValuePairSeq> {
    let mut map = BTreeMap::new();
    map.insert(
        "DDS".to_string(),
        dds_structured_data(participant, category),
    );
    map
}

impl BuiltinLoggingTypeV2 {
    /// Fills a V2 sample from one `LoggingPlugin::log` call (§10.6.1).
    #[must_use]
    pub fn from_log_event(
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> Self {
        Self {
            facility: FACILITY_SEC_AUTH,
            severity: level.into(),
            timestamp: Time::now(),
            hostname: host_name(),
            hostip: host_ip(),
            appname: app_name(),
            procid: std::process::id().to_string(),
            msgid: category.to_string(),
            message: message.to_string(),
            structured_data: structured_data(participant, category),
        }
    }
}

impl BuiltinLoggingType {
    /// Fills a legacy sample from one `LoggingPlugin::log` call (§10.6.1),
    /// with the same fields as [`BuiltinLoggingTypeV2::from_log_event`] and the
    /// narrower `LegacyTime_t` timestamp.
    #[must_use]
    pub fn from_log_event(
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> Self {
        Self {
            facility: FACILITY_SEC_AUTH,
            severity: level.into(),
            timestamp: LegacyTime::now(),
            hostname: host_name(),
            hostip: host_ip(),
            appname: app_name(),
            procid: std::process::id().to_string(),
            msgid: category.to_string(),
            message: message.to_string(),
            structured_data: structured_data(participant, category),
        }
    }
}

/// What went wrong publishing a security-log event on a DDS Log Topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTopicPublishError {
    /// The topic the sample was headed for.
    pub topic: &'static str,
    /// The writer's error.
    pub reason: String,
}

impl fmt::Display for LogTopicPublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "publishing on '{}' failed: {}", self.topic, self.reason)
    }
}

impl std::error::Error for LogTopicPublishError {}

/// A DataWriter for the BuiltinLoggingTopic(s), as seen by the sink.
///
/// Implemented in `zerodds-dcps` over a typed `DataWriter`; this crate holds
/// only the trait so that it stays free of a DCPS dependency.
pub trait LogTopicPublisher: Send + Sync {
    /// Publishes on `DDS:Security:LogTopicV2` (type `BuiltinLoggingTypeV2`).
    ///
    /// # Errors
    /// The DataWriter's write error, so the sink can report it rather than
    /// drop it.
    fn publish_v2(&self, event: &BuiltinLoggingTypeV2) -> Result<(), LogTopicPublishError>;

    /// Publishes on the legacy `DDS:Security:LogTopic` (type
    /// `BuiltinLoggingType`).
    ///
    /// # Errors
    /// The DataWriter's write error.
    fn publish_legacy(&self, event: &BuiltinLoggingType) -> Result<(), LogTopicPublishError>;
}

/// The builtin `DDS:Logging:DDS_LogTopic` sink (DDS-Security 1.2 §10.6).
///
/// The sink follows the plugin's `log` behavior (§10.6.1): after
/// [`Self::enable_logging`], an event at or below the [`LogOptions::log_level`]
/// is appended to the [`LogOptions::log_file`] — one line per message,
/// `[SEC][<LEVEL>] participant=<hex16> category=<category> msg=<message>` —
/// when a file is set, and published on the BuiltinLoggingTopic through the
/// [`LogTopicPublisher`] only when [`LogOptions::distribute`] is set. The
/// legacy topic is published too when `publish_legacy` is set. Before
/// `enable_logging` the sink logs nothing.
///
/// A publish or file failure is written to `stderr` — the sink has nowhere
/// else to put it, and dropping it silently is what this plugin exists to
/// prevent.
pub struct LogTopicLoggingPlugin {
    publisher: Option<Arc<dyn LogTopicPublisher>>,
    publish_legacy: bool,
    control: LogControl,
}

impl LogTopicLoggingPlugin {
    /// A publishing sink at `min_level`, already enabled: distributes, writes
    /// no file, publishes the legacy topic when `publish_legacy` is set.
    ///
    /// For the `LogOptions` path — file and `distribute` — use
    /// [`Self::with_options`] and [`Self::enable_logging`].
    #[must_use]
    pub fn new(
        publisher: Arc<dyn LogTopicPublisher>,
        min_level: LogLevel,
        publish_legacy: bool,
    ) -> Self {
        Self {
            publisher: Some(publisher),
            publish_legacy,
            control: LogControl::enabled_at(min_level, true),
        }
    }

    /// A sink configured with `options` but **not yet enabled**; the caller
    /// must call [`Self::enable_logging`] before the first `log`, as the
    /// plugin's `set_log_options` operation does (DDS-Security 1.2 §10.6.1).
    ///
    /// `log_file` is opened here, so a file that cannot be opened is this
    /// call's error and not a logger that silently logs nowhere (§9.6.2.1.2).
    /// A `publisher` is required when `options.distribute` is set and is
    /// ignored otherwise.
    ///
    /// # Errors
    /// [`LogOptionsError::MissingPublisher`] when `distribute` is set and no
    /// publisher was supplied; [`LogOptionsError::Io`] when the file cannot be
    /// opened.
    pub fn with_options(
        publisher: Option<Arc<dyn LogTopicPublisher>>,
        options: LogOptions,
        publish_legacy: bool,
    ) -> Result<Self, LogOptionsError> {
        if options.distribute && publisher.is_none() {
            return Err(LogOptionsError::MissingPublisher);
        }
        Ok(Self {
            publisher,
            publish_legacy,
            control: LogControl::with_options(options)?,
        })
    }

    /// Sets the log options (the plugin's `set_log_options`, §10.6.1).
    ///
    /// # Errors
    /// [`LogOptionsError::AlreadyEnabled`] once [`Self::enable_logging`] has
    /// been called — the options may not change after logging commenced
    /// (§9.6.2.1, §9.6.2.2.1); [`LogOptionsError::MissingPublisher`] when the
    /// new options distribute but the sink has no publisher;
    /// [`LogOptionsError::Io`] when the `log_file` cannot be opened.
    pub fn set_log_options(&self, options: LogOptions) -> Result<(), LogOptionsError> {
        self.control
            .set_log_options(options, self.publisher.is_some())
    }

    /// Enables logging (the plugin's `enable_logging`, §10.6.1).
    ///
    /// # Errors
    /// [`LogOptionsError::OptionsNotSet`] when called before
    /// [`Self::set_log_options`] (§9.6.2.2.3).
    pub fn enable_logging(&self) -> Result<(), LogOptionsError> {
        self.control.enable_logging()
    }

    /// `true` once [`Self::enable_logging`] has been called.
    #[must_use]
    pub fn is_logging_enabled(&self) -> bool {
        self.control.is_enabled()
    }

    /// The options currently set, if any.
    #[must_use]
    pub fn log_options(&self) -> Option<LogOptions> {
        self.control.options()
    }

    /// `true` when the legacy `DDS:Security:LogTopic` is published too.
    #[must_use]
    pub fn publishes_legacy(&self) -> bool {
        self.publish_legacy
    }

    fn publish(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        let Some(publisher) = self.publisher.as_ref() else {
            report(LogOptionsError::MissingPublisher);
            return;
        };
        let v2 = BuiltinLoggingTypeV2::from_log_event(level, participant, category, message);
        if let Err(e) = publisher.publish_v2(&v2) {
            report(e);
        }
        if self.publish_legacy {
            let legacy = BuiltinLoggingType::from_log_event(level, participant, category, message);
            if let Err(e) = publisher.publish_legacy(&legacy) {
                report(e);
            }
        }
    }
}

impl LoggingPlugin for LogTopicLoggingPlugin {
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
        LogTopicLoggingPlugin::set_log_options(self, options).map_err(SecurityError::from)
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        LogTopicLoggingPlugin::enable_logging(self).map_err(SecurityError::from)
    }

    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError> {
        self.control
            .set_listener(listener)
            .map_err(SecurityError::from)
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        // §10.6.1: without a prior enable_logging the log operation does
        // nothing, and at a level above the saved one it does nothing either;
        // otherwise the line is appended and the listener notified.
        if self.control.admit(level, participant, category, message) == Some(true) {
            self.publish(level, participant, category, message);
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:DDS_LogTopic"
    }
}

/// A [`LogTopicPublisher`] whose DataWriter is attached after construction.
///
/// The participant materializes its logging sink from `dds.sec.log.*` before
/// its runtime exists, and the DataWriter needs the runtime and the
/// participant. The slot is filled right after the participant is built; until
/// then a `publish_*` call reports "writer not ready" instead of dropping the
/// event, and the sink writes that report to `stderr`.
#[derive(Default)]
pub struct LogTopicWriterSlot {
    inner: Mutex<Option<Arc<dyn LogTopicPublisher>>>,
}

impl LogTopicWriterSlot {
    /// An unfilled slot.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Attaches the publisher. Returns the argument back if one is already
    /// attached, so the caller can report the duplicate instead of losing it.
    ///
    /// # Errors
    /// `Err(publisher)` when the slot already holds one, or when the lock was
    /// poisoned.
    pub fn set(
        &self,
        publisher: Arc<dyn LogTopicPublisher>,
    ) -> Result<(), Arc<dyn LogTopicPublisher>> {
        match self.inner.lock() {
            Ok(mut slot) if slot.is_none() => {
                *slot = Some(publisher);
                Ok(())
            }
            _ => Err(publisher),
        }
    }

    /// `true` once a publisher is attached.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.inner.lock().map(|s| s.is_some()).unwrap_or(false)
    }
}

fn not_ready(topic: &'static str) -> LogTopicPublishError {
    LogTopicPublishError {
        topic,
        reason: "the DDS Log Topic DataWriter is not attached yet".to_string(),
    }
}

impl LogTopicPublisher for LogTopicWriterSlot {
    fn publish_v2(&self, event: &BuiltinLoggingTypeV2) -> Result<(), LogTopicPublishError> {
        let guard = self.inner.lock().map_err(|_| LogTopicPublishError {
            topic: zerodds_security::logging::TOPIC_LOG_TOPIC_V2,
            reason: "the log-topic writer slot lock is poisoned".to_string(),
        })?;
        match guard.as_ref() {
            Some(publisher) => publisher.publish_v2(event),
            None => Err(not_ready(zerodds_security::logging::TOPIC_LOG_TOPIC_V2)),
        }
    }

    fn publish_legacy(&self, event: &BuiltinLoggingType) -> Result<(), LogTopicPublishError> {
        let guard = self.inner.lock().map_err(|_| LogTopicPublishError {
            topic: zerodds_security::logging::TOPIC_LOG_TOPIC,
            reason: "the log-topic writer slot lock is poisoned".to_string(),
        })?;
        match guard.as_ref() {
            Some(publisher) => publisher.publish_legacy(event),
            None => Err(not_ready(zerodds_security::logging::TOPIC_LOG_TOPIC)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    type Heard = (LogLevel, [u8; 16], String, String);

    #[derive(Default)]
    struct ListenerRecorder(StdMutex<Vec<Heard>>);

    impl LoggerListener for ListenerRecorder {
        fn log_message(
            &self,
            level: LogLevel,
            participant: [u8; 16],
            category: &str,
            message: &str,
        ) -> i32 {
            self.0.lock().unwrap().push((
                level,
                participant,
                category.to_string(),
                message.to_string(),
            ));
            0
        }
    }

    #[derive(Default)]
    struct Recorder {
        v2: StdMutex<Vec<BuiltinLoggingTypeV2>>,
        legacy: StdMutex<Vec<BuiltinLoggingType>>,
        fail: bool,
    }

    impl LogTopicPublisher for Recorder {
        fn publish_v2(&self, event: &BuiltinLoggingTypeV2) -> Result<(), LogTopicPublishError> {
            if self.fail {
                return Err(LogTopicPublishError {
                    topic: zerodds_security::logging::TOPIC_LOG_TOPIC_V2,
                    reason: "write refused".to_string(),
                });
            }
            self.v2.lock().unwrap().push(event.clone());
            Ok(())
        }

        fn publish_legacy(&self, event: &BuiltinLoggingType) -> Result<(), LogTopicPublishError> {
            self.legacy.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    #[test]
    fn plugin_class_id_is_the_spec_name() {
        let sink = LogTopicLoggingPlugin::new(
            Arc::new(Recorder::default()),
            LogLevel::Informational,
            false,
        );
        assert_eq!(sink.plugin_class_id(), "DDS:Logging:DDS_LogTopic");
    }

    #[test]
    fn an_event_fills_every_field_from_the_clause() {
        let v2 = BuiltinLoggingTypeV2::from_log_event(
            LogLevel::Critical,
            [0xAA; 16],
            "access.create_datawriter.denied",
            "topic Chatter: denied",
        );
        assert_eq!(v2.facility, FACILITY_SEC_AUTH);
        assert_eq!(v2.severity, LoggingLevel::Critical);
        assert_eq!(v2.msgid, "access.create_datawriter.denied");
        assert_eq!(v2.message, "topic Chatter: denied");
        assert_eq!(v2.procid, std::process::id().to_string());
        // timestamp is the test's wall clock: before the epoch it would not
        // round-trip, and the second field is always < 1e9.
        assert!(v2.timestamp.nanosec < 1_000_000_000);
        let dds = v2.structured_data.get("DDS").expect("DDS entry");
        let get = |name: &str| {
            dds.iter()
                .find(|p| p.name == name)
                .map(|p| p.value.as_str())
                .unwrap_or("<missing>")
        };
        assert_eq!(get("guid"), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(get("plugin_class"), "AccessControl");
        assert_eq!(get("plugin_method"), "create_datawriter.denied");
    }

    #[test]
    fn events_above_the_level_are_dropped_and_at_or_below_pass() {
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::new(recorder.clone(), LogLevel::Warning, false);
        sink.log(LogLevel::Informational, [0; 16], "debug", "no");
        sink.log(LogLevel::Warning, [0; 16], "warn", "yes");
        assert_eq!(recorder.v2.lock().unwrap().len(), 1);
        assert_eq!(recorder.v2.lock().unwrap()[0].message, "yes");
    }

    #[test]
    fn legacy_topic_is_published_when_asked() {
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::new(recorder.clone(), LogLevel::Debug, true);
        sink.log(LogLevel::Error, [1; 16], "auth", "bad");
        assert_eq!(recorder.v2.lock().unwrap().len(), 1);
        assert_eq!(recorder.legacy.lock().unwrap().len(), 1);
        assert_eq!(
            recorder.legacy.lock().unwrap()[0].facility,
            FACILITY_SEC_AUTH
        );
    }

    #[test]
    fn an_unfilled_slot_reports_instead_of_dropping() {
        let slot = Arc::new(LogTopicWriterSlot::empty());
        assert!(!slot.is_set());
        let err = slot
            .publish_v2(&BuiltinLoggingTypeV2::from_log_event(
                LogLevel::Error,
                [0; 16],
                "auth",
                "x",
            ))
            .expect_err("not attached");
        assert!(err.reason.contains("not attached"), "{err}");
        let publisher: Arc<dyn LogTopicPublisher> = Arc::new(Recorder::default());
        assert!(slot.set(publisher).is_ok());
        assert!(slot.is_set());
        assert!(
            slot.publish_v2(&BuiltinLoggingTypeV2::from_log_event(
                LogLevel::Error,
                [0; 16],
                "auth",
                "x",
            ))
            .is_ok()
        );
    }

    #[test]
    fn a_second_set_is_refused_so_the_caller_can_report_it() {
        let slot = LogTopicWriterSlot::empty();
        let first: Arc<dyn LogTopicPublisher> = Arc::new(Recorder::default());
        let second: Arc<dyn LogTopicPublisher> = Arc::new(Recorder::default());
        assert!(slot.set(first).is_ok());
        assert!(slot.set(second).is_err(), "the duplicate is handed back");
    }

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(alloc::format!(
            "zerodds_log_topic_{}_{name}.log",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    /// §10.6.1 — one line per logged message, at and below the level and
    /// nothing above it.
    #[test]
    fn log_options_write_one_line_per_event_at_or_below_the_level() {
        let path = tmp_path("threshold");
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::with_options(
            Some(recorder.clone()),
            LogOptions {
                log_level: LogLevel::Warning,
                log_file: Some(path.to_string_lossy().into_owned()),
                distribute: false,
            },
            false,
        )
        .expect("options and file");
        assert!(!sink.is_logging_enabled());
        sink.enable_logging().expect("enable");
        assert!(sink.is_logging_enabled());
        sink.log(
            LogLevel::Error,
            [0xAB; 16],
            "auth.handshake.failed",
            "bad certificate",
        );
        sink.log(
            LogLevel::Informational,
            [0xCD; 16],
            "auth.debug",
            "below level",
        );

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "only Warning-and-above is written: {content}"
        );
        assert!(lines[0].contains("[SEC][ERROR]"), "got: {}", lines[0]);
        assert!(
            lines[0].contains("participant=abababababababababababababababab"),
            "got: {}",
            lines[0]
        );
        assert!(lines[0].contains("category=auth.handshake.failed"));
        assert!(lines[0].contains("msg=bad certificate"));
        let _ = std::fs::remove_file(&path);
    }

    /// A newline inside a field must not split one event into two lines.
    #[test]
    fn a_field_with_a_newline_stays_on_one_line() {
        let path = tmp_path("newline");
        let sink = LogTopicLoggingPlugin::with_options(
            None,
            LogOptions {
                log_level: LogLevel::Debug,
                log_file: Some(path.to_string_lossy().into_owned()),
                distribute: false,
            },
            false,
        )
        .expect("options and file");
        sink.enable_logging().expect("enable");
        sink.log(LogLevel::Warning, [0; 16], "auth", "first\nsecond");

        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content.lines().count(), 1, "got: {content}");
        assert!(content.contains("first\\nsecond"), "got: {content}");
        let _ = std::fs::remove_file(&path);
    }

    /// §9.6.2.1.3 — `distribute` off means nothing reaches the publisher,
    /// even with the legacy topic asked for.
    #[test]
    fn distribute_off_publishes_nothing() {
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::with_options(
            Some(recorder.clone()),
            LogOptions {
                log_level: LogLevel::Debug,
                log_file: None,
                distribute: false,
            },
            true,
        )
        .expect("publisher is supplied");
        sink.enable_logging().expect("enable");
        sink.log(LogLevel::Error, [0; 16], "auth", "local only");
        assert_eq!(recorder.v2.lock().unwrap().len(), 0);
        assert_eq!(recorder.legacy.lock().unwrap().len(), 0);
    }

    /// §9.6.2.1 / §9.6.2.2.1 — `set_log_options` after `enable_logging` is
    /// refused, so an attacker cannot retune or suspend the log at runtime.
    #[test]
    fn options_cannot_change_after_logging_is_enabled() {
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::with_options(
            Some(recorder),
            LogOptions::new(LogLevel::Warning),
            false,
        )
        .expect("publisher is supplied");
        sink.enable_logging().expect("enable");
        match sink.set_log_options(LogOptions::new(LogLevel::Debug)) {
            Err(LogOptionsError::AlreadyEnabled) => {}
            Err(other) => panic!("expected AlreadyEnabled, got {other:?}"),
            Ok(()) => panic!("expected AlreadyEnabled, got Ok"),
        }
        assert_eq!(
            sink.log_options().map(|o| o.log_level),
            Some(LogLevel::Warning),
            "the refused change must not alter the saved options"
        );
    }

    /// §9.6.2.1.2 — a file that cannot be opened is refused at option-setting
    /// time instead of silently disabling the file log.
    #[test]
    fn an_unopenable_log_file_is_refused() {
        let bad = String::from("/nonexistent-zerodds-dir/security.log");
        match LogTopicLoggingPlugin::with_options(
            None,
            LogOptions {
                log_level: LogLevel::Debug,
                log_file: Some(bad),
                distribute: false,
            },
            false,
        ) {
            Err(LogOptionsError::Io(_)) => {}
            Err(other) => panic!("expected Io, got {other:?}"),
            Ok(_) => panic!("expected Io, got Ok"),
        }
    }

    /// §10.6.1 Table 82 — `log` checks whether logging was enabled by a prior
    /// call to `enable_logging`; if not it returns without performing any
    /// action: nothing is appended to the file, nothing is published, no
    /// listener is told. After `enable_logging` the same call logs.
    #[test]
    fn log_before_enable_logging_does_nothing() {
        let path = tmp_path("before_enable");
        let recorder = Arc::new(Recorder::default());
        let sink = LogTopicLoggingPlugin::with_options(
            Some(recorder.clone()),
            LogOptions {
                log_level: LogLevel::Debug,
                log_file: Some(path.to_string_lossy().into_owned()),
                distribute: true,
            },
            true,
        )
        .expect("options and file");
        let heard = Arc::new(ListenerRecorder::default());
        LoggingPlugin::set_listener(&sink, Some(heard.clone())).expect("listener");

        sink.log(LogLevel::Error, [1; 16], "auth", "before enable");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        assert!(recorder.v2.lock().unwrap().is_empty());
        assert!(recorder.legacy.lock().unwrap().is_empty());
        assert!(heard.0.lock().unwrap().is_empty());

        LoggingPlugin::enable_logging(&sink).expect("enable");
        sink.log(LogLevel::Error, [1; 16], "auth", "after enable");
        assert_eq!(
            std::fs::read_to_string(&path)
                .expect("read")
                .lines()
                .count(),
            1
        );
        assert_eq!(recorder.v2.lock().unwrap().len(), 1);
        assert_eq!(recorder.legacy.lock().unwrap().len(), 1);
        assert_eq!(heard.0.lock().unwrap().len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    /// §10.6.1 Table 82 — `set_listener` saves a reference to the
    /// LoggerListener, which is notified each time a log message is produced;
    /// a message above the saved level produces none; the NIL listener removes
    /// it. Through the `LoggingPlugin` interface.
    #[test]
    fn the_listener_is_told_of_each_message_the_sink_logs() {
        let sink = LogTopicLoggingPlugin::with_options(
            None,
            LogOptions {
                log_level: LogLevel::Warning,
                log_file: None,
                distribute: false,
            },
            false,
        )
        .expect("options");
        let plugin: &dyn LoggingPlugin = &sink;
        let heard = Arc::new(ListenerRecorder::default());
        plugin.set_listener(Some(heard.clone())).expect("listener");
        plugin.enable_logging().expect("enable");
        plugin.log(LogLevel::Error, [2; 16], "auth", "one");
        plugin.log(LogLevel::Debug, [2; 16], "auth", "above the level");
        plugin.set_listener(None).expect("nil");
        plugin.log(LogLevel::Error, [2; 16], "auth", "two");
        assert_eq!(
            *heard.0.lock().unwrap(),
            vec![(
                LogLevel::Error,
                [2u8; 16],
                "auth".to_string(),
                "one".to_string()
            )]
        );
        // set_log_options through the interface after enable_logging is an
        // error, and says so as a SecurityException.
        let err = plugin
            .set_log_options(LogOptions::new(LogLevel::Debug))
            .expect_err("the options may not change");
        assert_eq!(
            err.kind,
            zerodds_security::error::SecurityErrorKind::BadArgument
        );
    }

    /// §9.6.2.2.3 — enabling without options is refused.
    #[test]
    fn enable_without_options_is_refused() {
        let sink = LogTopicLoggingPlugin {
            publisher: None,
            publish_legacy: false,
            control: LogControl::new(),
        };
        match sink.enable_logging() {
            Err(LogOptionsError::OptionsNotSet) => {}
            other => panic!("expected OptionsNotSet, got {other:?}"),
        }
    }
}
