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
pub fn notify_service_manager(state: &str) {
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
