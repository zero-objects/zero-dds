// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The state behind the Table 46 operations of a Logging plugin
//! (DDS-Security 1.2 §9.6.2.2): the options, whether logging is enabled, the
//! opened `log_file` and the listener.
//!
//! Every sink of this crate holds a [`LogControl`], so `set_log_options`,
//! `enable_logging`, `set_listener` and the admission check of `log` behave
//! the same for all of them.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The listener of `set_listener` (DDS-Security 1.2 §9.6.2.2.4) is a `dyn LoggerListener`.)

use alloc::string::String;
use alloc::sync::Arc;
use core::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::Mutex;

use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::{LogLevel, LogOptions, LoggerListener};

/// Why a sink refused a [`LogOptions`] change, an `enable_logging` call or a
/// listener.
#[derive(Debug)]
pub enum LogOptionsError {
    /// `set_log_options` was called after `enable_logging`; the options may
    /// not change once logging has commenced (DDS-Security 1.2 §9.6.2.1,
    /// §9.6.2.2.1).
    AlreadyEnabled,
    /// `enable_logging` was called before any options were set
    /// (§9.6.2.2.3).
    OptionsNotSet,
    /// `distribute` is set but the sink cannot publish the event: it has no
    /// [`crate::LogTopicPublisher`], or it is not a sink for the DDS Log
    /// Topic.
    MissingPublisher,
    /// The `log_file` could not be opened for appending (permissions, missing
    /// parent directory).
    Io(std::io::Error),
    /// The sink's state lock was poisoned by a panic in another thread.
    Poisoned,
}

impl fmt::Display for LogOptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyEnabled => write!(
                f,
                "log options cannot be set after logging was enabled \
                 (DDS-Security 1.2 §9.6.2.1)"
            ),
            Self::OptionsNotSet => {
                write!(f, "logging cannot be enabled before log options are set")
            }
            Self::MissingPublisher => write!(
                f,
                "distribute is set, but no DDS Log Topic DataWriter was supplied \
                 (DDS-Security 1.2 §10.6.1)"
            ),
            Self::Io(e) => write!(f, "log file i/o error: {e}"),
            Self::Poisoned => write!(f, "the log sink state lock is poisoned"),
        }
    }
}

impl std::error::Error for LogOptionsError {}

impl From<LogOptionsError> for SecurityError {
    fn from(e: LogOptionsError) -> Self {
        let kind = match e {
            LogOptionsError::AlreadyEnabled | LogOptionsError::OptionsNotSet => {
                SecurityErrorKind::BadArgument
            }
            LogOptionsError::MissingPublisher | LogOptionsError::Io(_) => {
                SecurityErrorKind::InvalidConfiguration
            }
            LogOptionsError::Poisoned => SecurityErrorKind::Internal,
        };
        SecurityError::new(kind, alloc::format!("{e}"))
    }
}

struct State {
    options: Option<LogOptions>,
    enabled: bool,
    file: Option<BufWriter<File>>,
    listener: Option<Arc<dyn LoggerListener>>,
}

/// Opens `log_file` for appending, or `Ok(None)` when it is unset
/// (DDS-Security 1.2 §9.6.2.1.2).
fn open_log_file(path: Option<&str>) -> Result<Option<BufWriter<File>>, LogOptionsError> {
    match path {
        Some(p) => {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .map_err(LogOptionsError::Io)?;
            Ok(Some(BufWriter::new(file)))
        }
        None => Ok(None),
    }
}

/// The severity label a file line carries.
fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Emergency => "EMERGENCY",
        LogLevel::Alert => "ALERT",
        LogLevel::Critical => "CRITICAL",
        LogLevel::Error => "ERROR",
        LogLevel::Warning => "WARNING",
        LogLevel::Notice => "NOTICE",
        LogLevel::Informational => "INFORMATIONAL",
        LogLevel::Debug => "DEBUG",
        _ => "TRACE",
    }
}

/// Lower-case hex of a GUID.
pub(crate) fn hex16(bytes: [u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&alloc::format!("{b:02x}"));
    }
    s
}

/// Escapes a field so that one logged message occupies exactly one file line:
/// a real newline in `message` would otherwise split one event into two
/// lines and let it forge a record.
fn escape_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&alloc::format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The string representation of one `log` call that is appended to
/// `LogOptions::log_file` (DDS-Security 1.2 §10.6.1): every parameter of the
/// `log` operation plus the participant GUID the logger adds (§9.6.2.2.2):
/// `[SEC][<LEVEL>] participant=<hex16> category=<category> msg=<message>`.
pub(crate) fn file_line(
    level: LogLevel,
    participant: [u8; 16],
    category: &str,
    message: &str,
) -> String {
    alloc::format!(
        "[SEC][{lvl}] participant={pid} category={cat} msg={msg}",
        lvl = level_label(level),
        pid = hex16(participant),
        cat = escape_line(category),
        msg = escape_line(message),
    )
}

/// Reports a failure on `stderr`: the sinks have nowhere else to put an
/// error of `log`, and dropping it silently is what a security log must not
/// do.
pub(crate) fn report(message: impl fmt::Display) {
    let mut out = std::io::stderr().lock();
    let _ = writeln!(out, "[SEC][ERROR] security logging: {message}");
}

/// Options, enabled flag, log file and listener of one sink.
pub struct LogControl {
    state: Mutex<State>,
}

impl Default for LogControl {
    fn default() -> Self {
        Self::new()
    }
}

impl LogControl {
    /// A control with no options set and logging not enabled: `log` does
    /// nothing until `set_log_options` and `enable_logging` were called
    /// (DDS-Security 1.2 §10.6.1).
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                options: None,
                enabled: false,
                file: None,
                listener: None,
            }),
        }
    }

    /// A control that is enabled at `level` with no file, and with
    /// `distribute` as given: the state of a sink that was configured
    /// through its constructor.
    #[must_use]
    pub fn enabled_at(level: LogLevel, distribute: bool) -> Self {
        Self {
            state: Mutex::new(State {
                options: Some(LogOptions {
                    log_level: level,
                    log_file: None,
                    distribute,
                }),
                enabled: true,
                file: None,
                listener: None,
            }),
        }
    }

    /// A control holding `options` and an opened `log_file`, not yet
    /// enabled.
    ///
    /// # Errors
    /// [`LogOptionsError::Io`] when the file cannot be opened.
    pub fn with_options(options: LogOptions) -> Result<Self, LogOptionsError> {
        let file = open_log_file(options.log_file.as_deref())?;
        Ok(Self {
            state: Mutex::new(State {
                options: Some(options),
                enabled: false,
                file,
                listener: None,
            }),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, LogOptionsError> {
        self.state.lock().map_err(|_| LogOptionsError::Poisoned)
    }

    /// `set_log_options` (§9.6.2.2.1). `can_distribute` is `false` for a sink
    /// that has no DataWriter for the BuiltinLoggingTopic; `distribute` in
    /// the options is then refused.
    ///
    /// # Errors
    /// [`LogOptionsError::AlreadyEnabled`] after `enable_logging`;
    /// [`LogOptionsError::MissingPublisher`] for `distribute` without
    /// `can_distribute`; [`LogOptionsError::Io`] when the file cannot be
    /// opened. The saved options stay as they were.
    pub fn set_log_options(
        &self,
        options: LogOptions,
        can_distribute: bool,
    ) -> Result<(), LogOptionsError> {
        let mut state = self.lock()?;
        if state.enabled {
            return Err(LogOptionsError::AlreadyEnabled);
        }
        if options.distribute && !can_distribute {
            return Err(LogOptionsError::MissingPublisher);
        }
        // Open before mutating: a bad file leaves the previous options in
        // place rather than half-applying the new ones.
        let file = open_log_file(options.log_file.as_deref())?;
        state.options = Some(options);
        state.file = file;
        Ok(())
    }

    /// `enable_logging` (§9.6.2.2.3).
    ///
    /// # Errors
    /// [`LogOptionsError::OptionsNotSet`] when no options were set.
    pub fn enable_logging(&self) -> Result<(), LogOptionsError> {
        let mut state = self.lock()?;
        if state.options.is_none() {
            return Err(LogOptionsError::OptionsNotSet);
        }
        state.enabled = true;
        Ok(())
    }

    /// `set_listener` (§9.6.2.2.4); `None` removes the listener.
    ///
    /// # Errors
    /// [`LogOptionsError::Poisoned`].
    pub fn set_listener(
        &self,
        listener: Option<Arc<dyn LoggerListener>>,
    ) -> Result<(), LogOptionsError> {
        self.lock()?.listener = listener;
        Ok(())
    }

    /// `true` once `enable_logging` has been called.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.state.lock().map(|s| s.enabled).unwrap_or(false)
    }

    /// The options currently set, if any.
    #[must_use]
    pub fn options(&self) -> Option<LogOptions> {
        self.state.lock().ok().and_then(|s| s.options.clone())
    }

    /// The admission check of `log` (§10.6.1): `None` when logging is not
    /// enabled or `level` is above the saved level (the call does nothing);
    /// otherwise the event is appended to the log file, the listener is
    /// notified, and `Some(distribute)` says whether the sink has to publish
    /// it.
    pub fn admit(
        &self,
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> Option<bool> {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                report(LogOptionsError::Poisoned);
                return None;
            }
        };
        if !state.enabled {
            return None;
        }
        let options = state.options.as_ref()?;
        if level > options.log_level {
            return None;
        }
        let distribute = options.distribute;
        if let Some(file) = state.file.as_mut() {
            let line = file_line(level, participant, category, message);
            if let Err(e) = writeln!(file, "{line}").and_then(|()| file.flush()) {
                report(LogOptionsError::Io(e));
            }
        }
        let listener = state.listener.clone();
        drop(state);
        if let Some(listener) = listener {
            listener.log_message(level, participant, category, message);
        }
        Some(distribute)
    }
}

/// `set_log_options`, `enable_logging` and `set_listener` of a
/// `LoggingPlugin` (DDS-Security 1.2 §9.6.2.2) that only writes to a sink of
/// its own: the three operations go to its `control`, and the options name
/// no DataWriter to distribute the log on (`false`).
macro_rules! sink_control_operations {
    () => {
        fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
            self.control
                .set_log_options(options, false)
                .map_err(SecurityError::from)
        }

        fn enable_logging(&self) -> Result<(), SecurityError> {
            self.control.enable_logging().map_err(SecurityError::from)
        }

        fn set_listener(
            &self,
            listener: Option<Arc<dyn LoggerListener>>,
        ) -> Result<(), SecurityError> {
            self.control
                .set_listener(listener)
                .map_err(SecurityError::from)
        }
    };
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::{FanOutLoggingPlugin, JsonLinesLoggingPlugin, StderrLoggingPlugin};
    use alloc::boxed::Box;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;
    use std::sync::Mutex as StdMutex;
    use zerodds_security::logging::{FATAL_LEVEL, LoggingPlugin, TRACE_LEVEL};

    #[derive(Default)]
    struct Heard(StdMutex<Vec<(LogLevel, String)>>);

    impl LoggerListener for Heard {
        fn log_message(&self, level: LogLevel, _p: [u8; 16], _c: &str, message: &str) -> i32 {
            self.0.lock().unwrap().push((level, message.to_string()));
            0
        }
    }

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(alloc::format!(
            "zerodds_log_control_{}_{name}.log",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    /// §10.6.1 Table 82 — `log` without a prior `enable_logging` returns
    /// without performing any action: no file line, no listener call.
    #[test]
    fn log_before_enable_logging_does_nothing() {
        let path = tmp_path("before_enable");
        let control = LogControl::with_options(LogOptions {
            log_level: LogLevel::Debug,
            log_file: Some(path.to_string_lossy().into_owned()),
            distribute: false,
        })
        .expect("options");
        let heard = Arc::new(Heard::default());
        control.set_listener(Some(heard.clone())).expect("listener");
        assert_eq!(control.admit(LogLevel::Error, [0; 16], "c", "m"), None);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        assert!(heard.0.lock().unwrap().is_empty());
        control.enable_logging().expect("enable");
        assert_eq!(
            control.admit(LogLevel::Error, [0; 16], "c", "m"),
            Some(false)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        let _ = std::fs::remove_file(&path);
    }

    /// §10.6.1 Table 82 — `set_listener`: the listener is notified each time a
    /// log message is produced; a message above the level produces none; the
    /// NIL listener ends the notifications.
    #[test]
    fn the_listener_hears_each_logged_message() {
        let control = LogControl::enabled_at(LogLevel::Warning, false);
        let heard = Arc::new(Heard::default());
        control.set_listener(Some(heard.clone())).expect("listener");
        control.admit(LogLevel::Error, [1; 16], "c", "one");
        control.admit(LogLevel::Debug, [1; 16], "c", "above the level");
        control.admit(LogLevel::Warning, [1; 16], "c", "two");
        control.set_listener(None).expect("nil listener");
        control.admit(LogLevel::Error, [1; 16], "c", "three");
        assert_eq!(
            *heard.0.lock().unwrap(),
            vec![
                (LogLevel::Error, "one".to_string()),
                (LogLevel::Warning, "two".to_string())
            ]
        );
    }

    /// §9.6.2.1.1 — `log_level` takes the levels `FATAL_LEVEL` to
    /// `TRACE_LEVEL`: a message is logged at or below the saved level, and
    /// `TRACE_LEVEL` is the finest.
    #[test]
    fn the_spec_levels_filter_from_fatal_to_trace() {
        let heard = Arc::new(Heard::default());
        for spec in FATAL_LEVEL..=TRACE_LEVEL {
            let saved = LogLevel::from_spec_level(spec).expect("level");
            let control = LogControl::enabled_at(saved, false);
            control.set_listener(Some(heard.clone())).expect("listener");
            heard.0.lock().unwrap().clear();
            for logged in FATAL_LEVEL..=TRACE_LEVEL {
                let l = LogLevel::from_spec_level(logged).expect("level");
                control.admit(l, [0; 16], "c", l.spec_name());
            }
            let n = heard.0.lock().unwrap().len();
            assert_eq!(n as i32, spec + 1, "saved level {}", saved.spec_name());
        }
    }

    /// §9.6.2.2.1 — a sink with no DataWriter cannot distribute and says so;
    /// the options stay unset.
    #[test]
    fn a_sink_without_a_writer_refuses_distribute() {
        let control = LogControl::new();
        let err = control
            .set_log_options(LogOptions::new(LogLevel::Debug), false)
            .expect_err("distribute without a writer");
        assert!(matches!(err, LogOptionsError::MissingPublisher));
        assert_eq!(
            SecurityError::from(err).kind,
            SecurityErrorKind::InvalidConfiguration
        );
        assert_eq!(control.options(), None);
        let mut file_only = LogOptions::new(LogLevel::Debug);
        file_only.distribute = false;
        control
            .set_log_options(file_only, false)
            .expect("file only");
    }

    /// The fan-out and the stderr sink are enabled when built: new options
    /// are refused whatever they ask for.
    #[test]
    fn stderr_and_fanout_are_enabled_when_built() {
        let sinks: Vec<Box<dyn LoggingPlugin>> = vec![
            Box::new(StderrLoggingPlugin::new()),
            Box::new(FanOutLoggingPlugin::new()),
        ];
        for sink in sinks {
            let err = sink
                .set_log_options(LogOptions::new(LogLevel::Debug))
                .expect_err("enabled when built");
            assert_eq!(err.kind, SecurityErrorKind::BadArgument);
        }
    }

    /// §9.6.2.1 — the sinks built with a level are enabled at construction, so
    /// they refuse a change of options; `enable_logging` succeeds.
    #[test]
    fn a_sink_enabled_at_construction_refuses_new_options() {
        let path = tmp_path("jsonl");
        let sink = JsonLinesLoggingPlugin::open(&path, LogLevel::Warning).expect("open");
        let mut options = LogOptions::new(LogLevel::Debug);
        options.distribute = false;
        let err = sink.set_log_options(options).expect_err("already enabled");
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
        sink.enable_logging().expect("enable");
        let _ = std::fs::remove_file(&path);
    }

    /// §9.6.2.2.3 — `enable_logging` before any options is an error.
    #[test]
    fn enable_logging_without_options_is_an_error() {
        let control = LogControl::new();
        assert!(matches!(
            control.enable_logging(),
            Err(LogOptionsError::OptionsNotSet)
        ));
        let e: SecurityError = LogOptionsError::OptionsNotSet.into();
        assert_eq!(e.kind, SecurityErrorKind::BadArgument);
    }
}
