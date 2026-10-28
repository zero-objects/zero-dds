// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Logging plugin SPI (OMG DDS-Security 1.2 §9.6).
//!
//! Separate plugin slot for security events — important for audits,
//! pen tests, forensics. Separated from general application logging
//! so that security-critical events do not accidentally get lost under
//! the debug flag.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn LoggingPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

use crate::error::SecurityError;

/// `FATAL_LEVEL` of DDS-Security 1.2 §9.6.2.1.1: security error causing a
/// shutdown or failure of the DomainParticipant. The `log_level` values are
/// numbered from low to high in the order of the clause.
pub const FATAL_LEVEL: i32 = 0;
/// `SEVERE_LEVEL` (§9.6.2.1.1): major security error or fault.
pub const SEVERE_LEVEL: i32 = 1;
/// `ERROR_LEVEL` (§9.6.2.1.1): minor security error or fault.
pub const ERROR_LEVEL: i32 = 2;
/// `WARNING_LEVEL` (§9.6.2.1.1): undesirable or unexpected behavior.
pub const WARNING_LEVEL: i32 = 3;
/// `NOTICE_LEVEL` (§9.6.2.1.1): important security event.
pub const NOTICE_LEVEL: i32 = 4;
/// `INFO_LEVEL` (§9.6.2.1.1): interesting security event.
pub const INFO_LEVEL: i32 = 5;
/// `DEBUG_LEVEL` (§9.6.2.1.1): detailed information on the flow of the
/// security events.
pub const DEBUG_LEVEL: i32 = 6;
/// `TRACE_LEVEL` (§9.6.2.1.1): even more detailed information.
pub const TRACE_LEVEL: i32 = 7;

/// Severity of a security event (spec §9.6.2.1.1).
///
/// The variants carry the syslog names of the `LoggingLevel` enumeration of
/// §10.6. §9.6.2.1.1 names eight levels of its own, [`FATAL_LEVEL`] to
/// [`TRACE_LEVEL`]; [`LogLevel::to_spec_level`] and
/// [`LogLevel::from_spec_level`] convert between the two. `Emergency` is
/// `FATAL_LEVEL`, `Alert` and `Critical` are `SEVERE_LEVEL`, and `Trace` is
/// `TRACE_LEVEL`, which `LoggingLevel` has no enumerator for. The order of
/// the variants is the order of the levels, low to high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[non_exhaustive]
pub enum LogLevel {
    /// Security emergency (auth handshake failed, key-exchange break).
    Emergency = 0,
    /// Alert.
    Alert = 1,
    /// Critical.
    Critical = 2,
    /// Error.
    Error = 3,
    /// Warning.
    Warning = 4,
    /// Notice.
    Notice = 5,
    /// Informational.
    Informational = 6,
    /// Debug.
    Debug = 7,
    /// Trace: even more detailed information than `Debug` (`TRACE_LEVEL`).
    Trace = 8,
}

impl LogLevel {
    /// The `log_level` value of §9.6.2.1.1 (`FATAL_LEVEL` to `TRACE_LEVEL`).
    ///
    /// `Alert` and `Critical` both give [`SEVERE_LEVEL`].
    #[must_use]
    pub const fn to_spec_level(self) -> i32 {
        match self {
            Self::Emergency => FATAL_LEVEL,
            Self::Alert | Self::Critical => SEVERE_LEVEL,
            Self::Error => ERROR_LEVEL,
            Self::Warning => WARNING_LEVEL,
            Self::Notice => NOTICE_LEVEL,
            Self::Informational => INFO_LEVEL,
            Self::Debug => DEBUG_LEVEL,
            Self::Trace => TRACE_LEVEL,
        }
    }

    /// The level for a `log_level` value of §9.6.2.1.1, or `None` for a
    /// number that is none of `FATAL_LEVEL` to `TRACE_LEVEL`
    /// (`log` requires "one of the levels defined in 9.6.2.1.1").
    ///
    /// [`SEVERE_LEVEL`] gives `Critical`.
    #[must_use]
    pub const fn from_spec_level(level: i32) -> Option<Self> {
        match level {
            FATAL_LEVEL => Some(Self::Emergency),
            SEVERE_LEVEL => Some(Self::Critical),
            ERROR_LEVEL => Some(Self::Error),
            WARNING_LEVEL => Some(Self::Warning),
            NOTICE_LEVEL => Some(Self::Notice),
            INFO_LEVEL => Some(Self::Informational),
            DEBUG_LEVEL => Some(Self::Debug),
            TRACE_LEVEL => Some(Self::Trace),
            _ => None,
        }
    }

    /// The name of the level in §9.6.2.1.1, e.g. `"WARNING_LEVEL"`.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self.to_spec_level() {
            FATAL_LEVEL => "FATAL_LEVEL",
            SEVERE_LEVEL => "SEVERE_LEVEL",
            ERROR_LEVEL => "ERROR_LEVEL",
            WARNING_LEVEL => "WARNING_LEVEL",
            NOTICE_LEVEL => "NOTICE_LEVEL",
            INFO_LEVEL => "INFO_LEVEL",
            DEBUG_LEVEL => "DEBUG_LEVEL",
            _ => "TRACE_LEVEL",
        }
    }
}

/// `LogOptions` (spec §9.6.2.1, Table 45).
///
/// Set through [`LoggingPlugin::set_log_options`] before
/// [`LoggingPlugin::enable_logging`]; they may not change once logging has
/// commenced (§9.6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogOptions {
    /// `log_level` — messages at or below this level are logged
    /// (§9.6.2.1.1).
    pub log_level: LogLevel,
    /// `log_file` — the full path of a local file the logger appends to, or
    /// `None` to log no file (§9.6.2.1.2).
    pub log_file: Option<String>,
    /// `distribute` — whether the log events are published as a DDS Topic
    /// (§9.6.2.1.3).
    pub distribute: bool,
}

impl LogOptions {
    /// Options at `log_level` with no file and `distribute` on.
    #[must_use]
    pub fn new(log_level: LogLevel) -> Self {
        Self {
            log_level,
            log_file: None,
            distribute: true,
        }
    }
}

/// `LoggerListener` (spec §9.6.2, Figure 21): notified of each log message
/// the logger produces (§10.6.1 `set_listener`).
pub trait LoggerListener: Send + Sync {
    /// `log_message` of Figure 21, with the parameters of the `log`
    /// operation. The `Integer` the figure returns has no meaning in the
    /// specification; a listener returns 0.
    fn log_message(
        &self,
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> i32;
}

/// Topic name of the DDS-Security 1.2 `BuiltinLoggingTopic`, whose type is
/// `BuiltinLoggingTypeV2` (DDS-Security 1.2 §10.6).
pub const TOPIC_LOG_TOPIC_V2: &str = "DDS:Security:LogTopicV2";

/// Topic name of the legacy (DDS-Security 1.1 and earlier)
/// `BuiltinLoggingTopic`, whose type is `BuiltinLoggingType`
/// (DDS-Security 1.2 §10.6).
pub const TOPIC_LOG_TOPIC: &str = "DDS:Security:LogTopic";

/// Logging plugin (spec §9.6.2.2, Table 46: `set_log_options`, `log`,
/// `enable_logging`, `set_listener`).
pub trait LoggingPlugin: Send + Sync {
    /// `set_log_options` (§9.6.2.2.1): sets the options for the logger. It
    /// must be called before [`Self::enable_logging`]; setting the options
    /// after logging has been enabled is an error.
    ///
    /// # Errors
    /// A `SecurityException` when the options are not set: logging is
    /// enabled already, the log file cannot be opened, or `distribute` is set
    /// and the logger cannot distribute.
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError>;

    /// `enable_logging` (§9.6.2.2.3): from here on `log` logs according to
    /// the options, and the options may not be modified.
    ///
    /// # Errors
    /// A `SecurityException` when no options were set.
    fn enable_logging(&self) -> Result<(), SecurityError>;

    /// `set_listener` (§9.6.2.2.4): the listener the logger notifies of each
    /// log message it produces. `None` is the NIL listener: no listener.
    ///
    /// # Errors
    /// A `SecurityException` when the logger cannot save the listener.
    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError>;

    /// Log a security event.
    ///
    /// Spec §9.6.2.2.2 `log`. `participant` identifies the affected
    /// participant (GUID bytes, 16 octets). `category` is a
    /// plugin-specific string ("auth.handshake.failed" etc.).
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str);

    /// Plugin class id (e.g. "DDS:Logging:DDS_LogTopic" for the
    /// spec-intended LogTopic dispatch).
    fn plugin_class_id(&self) -> &str;
}

/// Factory alias.
pub type LoggingPluginBox = Box<dyn LoggingPlugin>;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_spec_level_names_are_numbered_low_to_high() {
        let names = [
            (FATAL_LEVEL, "FATAL_LEVEL"),
            (SEVERE_LEVEL, "SEVERE_LEVEL"),
            (ERROR_LEVEL, "ERROR_LEVEL"),
            (WARNING_LEVEL, "WARNING_LEVEL"),
            (NOTICE_LEVEL, "NOTICE_LEVEL"),
            (INFO_LEVEL, "INFO_LEVEL"),
            (DEBUG_LEVEL, "DEBUG_LEVEL"),
            (TRACE_LEVEL, "TRACE_LEVEL"),
        ];
        for (i, (value, name)) in names.iter().enumerate() {
            assert_eq!(*value, i as i32);
            let level = LogLevel::from_spec_level(*value).expect("level");
            assert_eq!(level.to_spec_level(), *value);
            assert_eq!(level.spec_name(), *name);
        }
        assert_eq!(LogLevel::from_spec_level(-1), None);
        assert_eq!(LogLevel::from_spec_level(8), None);
        // Alert is the same severity as Critical: SEVERE_LEVEL.
        assert_eq!(LogLevel::Alert.to_spec_level(), SEVERE_LEVEL);
        assert_eq!(
            LogLevel::Trace,
            LogLevel::from_spec_level(TRACE_LEVEL).unwrap()
        );
        assert!(LogLevel::Trace > LogLevel::Debug);
    }

    #[test]
    fn log_levels_order_correctly() {
        assert!(LogLevel::Emergency < LogLevel::Warning);
        assert!(LogLevel::Debug > LogLevel::Error);
    }
}
