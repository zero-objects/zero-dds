// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The configuration flags every ZeroDDS program reads, in one place.
//!
//! ```no_run
//! use std::process::ExitCode;
//! fn main() -> ExitCode {
//!     let configured = match zerodds_dcps::cli::Configured::from_env_args() {
//!         Ok(c) => c,
//!         Err(error) => {
//!             eprintln!("configuration: {error}");
//!             return ExitCode::from(2);
//!         }
//!     };
//!     if let Some(code) = configured.check_config() {
//!         return code;
//!     }
//!     let runtime_config = configured.runtime;
//!     let own_args = configured.args; // everything that was not a config flag
//!     # let _ = (runtime_config, own_args);
//!     ExitCode::SUCCESS
//! }
//! ```
//!
//! Precedence, lowest first: compiled-in defaults, environment
//! ([`crate::runtime::SETTINGS`]), files (`--config PATH` or the platform's
//! system layout, with drop-ins and includes), `--set path=value`.

// This module is the program's own voice: `--check-config` prints its report
// to stdout, and an unknown key at start is a warning on stderr, because a
// program that runs with a setting nobody reads must say so and has no other
// channel yet.
#![allow(clippy::print_stdout, clippy::print_stderr)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use std::process::ExitCode;

use zerodds_config::{CommandLine, ConfigError};

use crate::runtime::{ConfigSource, RuntimeConfig};

/// The help text for the configuration flags, for a program's `--help`.
pub const HELP: &str = "\
Configuration (lowest precedence first: defaults, environment, files, flags):
  --config PATH          base file; its sibling PATH.d/ holds drop-ins
                         (default: the system configuration directory)
  --set PATH=VALUE       one setting above every file, repeatable;
                         --set +PATH=VALUE appends to a list
  --check-config         validate and print the effective configuration
                         with the origin of every value, then exit:
                         0 valid, 1 invalid value or unknown key,
                         2 unreadable file, syntax error, include cycle";

/// Tells the service manager the daemon's state, `READY=1` once it serves
/// and `STOPPING=1` when the shutdown begins (systemd `sd_notify(3)`): one
/// datagram to the socket `$NOTIFY_SOCKET` names, a path or, with a leading
/// `@`, an abstract socket (Linux). Without the variable — not started by
/// systemd, or a unit not `Type=notify` — nothing happens. The packaged
/// units are `Type=notify` (deployment §2.3); before this, no daemon sent
/// anything and systemd waited for a readiness that never came.
///
/// On Windows the same states go to the service control manager when it
/// started the process ([`run_as_service`]): `READY=1` is
/// `SERVICE_RUNNING`, `STOPPING=1` `SERVICE_STOP_PENDING`.
pub fn notify_service_manager(state: &str) {
    #[cfg(windows)]
    windows_service::notify(state);
    if let Some(target) = std::env::var_os("NOTIFY_SOCKET") {
        notify_socket(&target.to_string_lossy(), state);
    }
}

/// One datagram `state` to the notify socket `target`.
fn notify_socket(target: &str, state: &str) {
    #[cfg(unix)]
    {
        let Ok(socket) = std::os::unix::net::UnixDatagram::unbound() else {
            return;
        };
        #[cfg(target_os = "linux")]
        if let Some(name) = target.strip_prefix('@') {
            use std::os::linux::net::SocketAddrExt as _;
            if let Ok(addr) = std::os::unix::net::SocketAddr::from_abstract_name(name) {
                let _ = socket.send_to_addr(state.as_bytes(), &addr);
            }
            return;
        }
        let _ = socket.send_to(state.as_bytes(), target);
    }
    #[cfg(not(unix))]
    let _ = (target, state);
}

#[cfg(windows)]
mod windows_service;

/// The stop request of the service manager or the console (Windows): the
/// counterpart of SIGTERM and SIGINT.
static SERVICE_STOP: AtomicBool = AtomicBool::new(false);
/// The reload request of the service manager (Windows
/// `SERVICE_CONTROL_PARAMCHANGE`): the counterpart of SIGHUP.
static SERVICE_RELOAD: AtomicBool = AtomicBool::new(false);
/// Set when the service control manager runs the daemon (Windows).
static AS_SERVICE: AtomicBool = AtomicBool::new(false);

/// Whether the daemon runs as a Windows service: the service control
/// manager started the process and [`run_as_service`] called the daemon
/// from the service's main function. A service has no console, so what
/// the daemon writes to stdout and stderr goes nowhere; a daemon that logs
/// there writes its log to a file instead (zerodds-deployment-1.0 §4.1,
/// `%PROGRAMDATA%\ZeroDDS\logs\`). False elsewhere and from a console.
#[must_use]
pub fn running_as_service() -> bool {
    AS_SERVICE.load(Ordering::SeqCst)
}

/// [`running_as_service`] from now on: the service's main function calls
/// it before the daemon.
#[cfg_attr(not(windows), allow(dead_code))]
fn mark_running_as_service() {
    AS_SERVICE.store(true, Ordering::SeqCst);
}

/// Asks the daemon to stop, as SIGTERM does on Unix. The Windows service
/// control handler calls it on `SERVICE_CONTROL_STOP` and
/// `SERVICE_CONTROL_SHUTDOWN`, the console handler on Ctrl+C, Ctrl+Break,
/// the closing of the console, logoff and shutdown.
pub fn request_service_stop() {
    SERVICE_STOP.store(true, Ordering::SeqCst);
}

/// Asks the daemon to read its configuration again, as SIGHUP does on
/// Unix. The Windows service control handler calls it on
/// `SERVICE_CONTROL_PARAMCHANGE` (`sc.exe control <name> paramchange`).
pub fn request_service_reload() {
    SERVICE_RELOAD.store(true, Ordering::SeqCst);
}

/// Whether [`request_service_stop`] was called.
#[must_use]
pub fn service_stop_requested() -> bool {
    SERVICE_STOP.load(Ordering::SeqCst)
}

/// Whether [`request_service_reload`] was called since the last call of
/// this function; the request is taken.
#[must_use]
pub fn take_service_reload_request() -> bool {
    SERVICE_RELOAD.swap(false, Ordering::SeqCst)
}

/// Runs the daemon `daemon` and returns its exit code.
///
/// On Windows, when the service control manager started the process, it
/// runs as the service `name` (`StartServiceCtrlDispatcherW`): the status
/// is `SERVICE_START_PENDING` until the daemon calls
/// [`notify_service_manager`] with `READY=1`, the stop and shutdown
/// controls call [`request_service_stop`], `paramchange` calls
/// [`request_service_reload`], and when `daemon` returns the status is
/// `SERVICE_STOPPED` with its exit code (`ERROR_SERVICE_SPECIFIC_ERROR`
/// and the code for any code but 0). Started from a console, it runs
/// `daemon` with a console control handler that calls
/// [`request_service_stop`]. Elsewhere it calls `daemon`.
pub fn run_as_service(name: &str, daemon: fn() -> ExitCode) -> ExitCode {
    #[cfg(windows)]
    {
        windows_service::run(name, daemon)
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        daemon()
    }
}

/// What a service control code (Win32 `SERVICE_CONTROL_*`) asks of the
/// daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum ServiceControl {
    /// `SERVICE_CONTROL_STOP` (1), `SERVICE_CONTROL_SHUTDOWN` (5).
    Stop,
    /// `SERVICE_CONTROL_PARAMCHANGE` (6).
    Reload,
    /// `SERVICE_CONTROL_INTERROGATE` (4): the status is reported again.
    Interrogate,
    /// Any other: `ERROR_CALL_NOT_IMPLEMENTED`.
    NotImplemented,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn service_control(code: u32) -> ServiceControl {
    match code {
        1 | 5 => ServiceControl::Stop,
        6 => ServiceControl::Reload,
        4 => ServiceControl::Interrogate,
        _ => ServiceControl::NotImplemented,
    }
}

/// The service state a [`notify_service_manager`] text names, one
/// `KEY=VALUE` per line as `sd_notify(3)` takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum ServiceState {
    /// `READY=1`: `SERVICE_RUNNING`.
    Running,
    /// `STOPPING=1`: `SERVICE_STOP_PENDING`.
    StopPending,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn service_state_of(state: &str) -> Option<ServiceState> {
    state.lines().find_map(|line| match line.trim() {
        "READY=1" => Some(ServiceState::Running),
        "STOPPING=1" => Some(ServiceState::StopPending),
        _ => None,
    })
}

/// The number of an exit code, 0 to 255 (the codes `ExitCode::from(u8)`
/// makes); 1 for a code of no such number.
#[cfg_attr(not(windows), allow(dead_code))]
fn exit_code_number(code: ExitCode) -> u32 {
    (0..=u8::MAX)
        .find(|n| ExitCode::from(*n) == code)
        .map_or(1, u32::from)
}

/// A program's configuration, loaded from its command line.
#[derive(Debug)]
pub struct Configured {
    /// The validated runtime settings. Compiled-in defaults under
    /// `--check-config`, which validates instead of failing early.
    pub runtime: RuntimeConfig,
    /// The layered source, for origins and application keys.
    pub source: ConfigSource,
    /// The arguments that were not configuration flags, in order.
    pub args: Vec<String>,
    command_line: CommandLine,
    app: fn(&str) -> bool,
}

fn no_app_keys(_: &str) -> bool {
    false
}

impl Configured {
    /// [`Self::from_args`] over `std::env::args()` without the program name.
    ///
    /// # Errors
    /// As [`Self::from_args`].
    pub fn from_env_args() -> Result<Self, ConfigError> {
        Self::from_args(std::env::args().skip(1))
    }

    /// Takes the configuration flags out of `args` and loads the runtime's
    /// layers.
    ///
    /// # Errors
    /// A malformed flag, an unreadable or malformed file, an include cycle,
    /// an unknown `--set` path, and — unless `--check-config` was given — an
    /// invalid value.
    pub fn from_args<I>(args: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = String>,
    {
        Self::for_program(args, "zerodds", no_app_keys)
    }

    /// As [`Self::from_args`], for a program with its own base file name and
    /// its own keys beside the runtime's.
    ///
    /// # Errors
    /// As [`Self::from_args`].
    pub fn for_program<I>(args: I, stem: &str, app: fn(&str) -> bool) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = String>,
    {
        let (command_line, args) = CommandLine::extract(args)?;
        Self::from_command_line(command_line, args, stem, app, &[])
    }

    /// As [`Self::for_program`], with the configuration flags already taken
    /// out: a program that turns its own convenience flags (`--listen X`)
    /// into `--set` entries adds them to `command_line` first, so they sit in
    /// the flag layer and name themselves as the origin.
    ///
    /// # Errors
    /// As [`Self::from_args`].
    ///
    /// `app_env` names the program's own environment variables, as
    /// `(variable, path)`; see [`ConfigSource::for_program`].
    pub fn from_command_line(
        command_line: CommandLine,
        args: Vec<String>,
        stem: &str,
        app: fn(&str) -> bool,
        app_env: &[(&str, &str)],
    ) -> Result<Self, ConfigError> {
        let source = ConfigSource::for_program(&command_line, stem, &app, app_env, |name| {
            std::env::var(name).ok()
        })?;
        let runtime = if command_line.check() {
            RuntimeConfig::default()
        } else {
            let runtime = RuntimeConfig::from_source(&source)?;
            for (path, origin) in source.unknown_keys(&app) {
                eprintln!("warning: {origin}: unknown setting {path} is not read");
            }
            runtime
        };
        Ok(Self {
            runtime,
            source,
            args,
            command_line,
            app,
        })
    }

    /// Under `--check-config`: prints the report and returns the exit code,
    /// 0 for a valid configuration and 1 otherwise. `None` without the flag.
    #[must_use]
    pub fn check_config(&self) -> Option<ExitCode> {
        self.check_config_valid().map(|valid| {
            if valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        })
    }

    /// As [`Self::check_config`], for a program with its own exit-code
    /// type: prints the report and returns whether the configuration is
    /// valid; `None` without the flag.
    #[must_use]
    pub fn check_config_valid(&self) -> Option<bool> {
        if !self.command_line.check() {
            return None;
        }
        Some(match self.source.check_report(&self.app) {
            Ok(report) => {
                print!("{report}");
                println!("configuration valid");
                true
            }
            Err(report) => {
                print!("{report}");
                println!("configuration invalid");
                false
            }
        })
    }

    /// Whether `--check-config` was given.
    #[must_use]
    pub fn checking(&self) -> bool {
        self.command_line.check()
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod notify_tests {
    /// The state arrives as one datagram on the socket `$NOTIFY_SOCKET`
    /// names.
    #[test]
    fn the_state_goes_to_the_notify_socket() {
        let dir = std::env::temp_dir().join(format!("zd-notify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notify.sock");
        let rx = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        super::notify_socket(path.to_str().unwrap(), "READY=1");
        let mut buf = [0u8; 64];
        rx.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let n = rx.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"READY=1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;

    /// zerodds-deployment-1.0 §4.3: the service controls the daemon takes,
    /// with the Win32 values of `SERVICE_CONTROL_*`.
    #[test]
    fn deployment_4_3_the_service_controls_are_stop_reload_and_interrogate() {
        assert_eq!(service_control(1), ServiceControl::Stop);
        assert_eq!(service_control(5), ServiceControl::Stop);
        assert_eq!(service_control(6), ServiceControl::Reload);
        assert_eq!(service_control(4), ServiceControl::Interrogate);
        for other in [0, 2, 3, 7, 0x10, 0x20, 0x40] {
            assert_eq!(service_control(other), ServiceControl::NotImplemented);
        }
    }

    /// zerodds-deployment-1.0 §4.3: `READY=1` is the running state,
    /// `STOPPING=1` the stop pending state, on any line of the text.
    #[test]
    fn deployment_4_3_the_notify_states_are_service_states() {
        assert_eq!(service_state_of("READY=1"), Some(ServiceState::Running));
        assert_eq!(
            service_state_of("STATUS=draining\nSTOPPING=1"),
            Some(ServiceState::StopPending)
        );
        assert_eq!(service_state_of("READY=0"), None);
        assert_eq!(service_state_of("STATUS=READY=1"), None);
    }

    /// zerodds-deployment-1.0 §4.3: the exit code a stopped service reports
    /// is the daemon's.
    #[test]
    fn deployment_4_3_the_exit_code_has_its_number() {
        assert_eq!(exit_code_number(ExitCode::SUCCESS), 0);
        assert_eq!(exit_code_number(ExitCode::FAILURE), 1);
        for n in [2u8, 3, 4, 255] {
            assert_eq!(exit_code_number(ExitCode::from(n)), u32::from(n));
        }
    }

    /// zerodds-deployment-1.0 §4.3: the stop and reload requests; a reload
    /// request is taken once. `run_as_service` of a process the service
    /// control manager did not start runs the daemon and returns its code.
    #[test]
    fn deployment_4_3_the_requests_and_the_run() {
        assert!(!take_service_reload_request());
        request_service_reload();
        assert!(take_service_reload_request());
        assert!(!take_service_reload_request());
        fn daemon() -> ExitCode {
            ExitCode::from(3)
        }
        // On Windows the test process was not started by the service
        // control manager: the dispatcher refuses with
        // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT and the daemon runs as a
        // console program.
        assert_eq!(run_as_service("ZeroDDSTest", daemon), ExitCode::from(3));
        assert!(!running_as_service(), "a console run is not a service");
        request_service_stop();
        assert!(service_stop_requested());
    }
}
