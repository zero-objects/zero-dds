// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Windows service control dispatcher of a daemon
//! (zerodds-deployment-1.0 §4.3): `StartServiceCtrlDispatcherW`, the
//! control handler of `RegisterServiceCtrlHandlerExW` and the states of
//! `SetServiceStatus`; a console control handler when the process runs
//! from a console. The calls into `advapi32` and `kernel32` are the only
//! `unsafe` code here, each with its SAFETY comment.

#![allow(unsafe_code)]

use std::process::ExitCode;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{
    BOOL, ERROR_CALL_NOT_IMPLEMENTED, ERROR_FAILED_SERVICE_CONTROLLER_CONNECT,
    ERROR_SERVICE_SPECIFIC_ERROR, FALSE, NO_ERROR, TRUE,
};
use windows_sys::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
    SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SERVICE_ACCEPT_PARAMCHANGE, SERVICE_ACCEPT_SHUTDOWN,
    SERVICE_ACCEPT_STOP, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS,
    SERVICE_STATUS_CURRENT_STATE, SERVICE_STOP_PENDING, SERVICE_STOPPED, SERVICE_TABLE_ENTRYW,
    SERVICE_WIN32_OWN_PROCESS, SetServiceStatus, StartServiceCtrlDispatcherW,
};
use windows_sys::core::PWSTR;

use super::{
    ServiceControl, ServiceState, exit_code_number, mark_running_as_service,
    request_service_reload, request_service_stop, service_control, service_state_of,
};

/// How long the service manager waits for `READY=1` before it takes the
/// start as hung (the DDS participant and the listeners come up in it).
const START_WAIT_HINT_MS: u32 = 30_000;
/// How long it waits for the end of a stop: a daemon's drain (the
/// WebSocket daemon's `shutdown_drain_ms` defaults to 30 s) and its
/// clean-up.
const STOP_WAIT_HINT_MS: u32 = 60_000;

/// The service the process runs: its name as a NUL-terminated UTF-16
/// string and the daemon.
struct Service {
    name: Vec<u16>,
    daemon: fn() -> ExitCode,
}

static SERVICE: OnceLock<Service> = OnceLock::new();
/// The handle of `RegisterServiceCtrlHandlerExW`; 0 outside service mode.
static STATUS_HANDLE: AtomicIsize = AtomicIsize::new(0);
/// The checkpoint of a pending state, counted up with each report.
static CHECKPOINT: AtomicU32 = AtomicU32::new(0);
/// The exit code of the daemon run by `service_main`.
static EXIT: AtomicU32 = AtomicU32::new(0);
/// Set when the daemon of a console run has returned.
static CONSOLE_DONE: AtomicBool = AtomicBool::new(false);

/// [`super::run_as_service`] on Windows.
pub(super) fn run(name: &str, daemon: fn() -> ExitCode) -> ExitCode {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    if SERVICE
        .set(Service {
            name: wide.clone(),
            daemon,
        })
        .is_err()
    {
        // The dispatcher runs once per process; a second call runs the
        // daemon from the console.
        return console(daemon);
    }
    let mut table_name = wide;
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: table_name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: core::ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    // SAFETY: `table` is an array of SERVICE_TABLE_ENTRYW ended by an
    // entry of null pointers, as the function requires; the name it points
    // to is a NUL-terminated UTF-16 string that outlives the call, which
    // returns only when the service has stopped.
    let dispatched = unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) };
    if dispatched != 0 {
        return ExitCode::from(u8::try_from(EXIT.load(Ordering::SeqCst)).unwrap_or(1));
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == i32::try_from(ERROR_FAILED_SERVICE_CONTROLLER_CONNECT).ok() {
        // Not started by the service control manager: a console program.
        return console(daemon);
    }
    eprintln!("the service control dispatcher of {name} failed: {error}");
    ExitCode::from(1)
}

/// The daemon as a console program: Ctrl+C, Ctrl+Break, the closing of the
/// console, logoff and shutdown request the stop.
fn console(daemon: fn() -> ExitCode) -> ExitCode {
    // SAFETY: `console_handler` is an `extern "system"` function of the
    // PHANDLER_ROUTINE signature that lives as long as the process.
    let installed = unsafe { SetConsoleCtrlHandler(Some(console_handler), TRUE) };
    if installed == 0 {
        eprintln!(
            "the console control handler was not installed: {}",
            std::io::Error::last_os_error()
        );
    }
    let code = daemon();
    CONSOLE_DONE.store(true, Ordering::SeqCst);
    code
}

// SAFETY: called by the system with a control code only; it reads no
// pointer and touches only atomics.
unsafe extern "system" fn console_handler(ctrl: u32) -> BOOL {
    match ctrl {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => {
            request_service_stop();
            TRUE
        }
        // After these the system ends the process when the handler
        // returns: the handler waits for the daemon's shutdown (the system
        // ends it after its own time limit).
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
            request_service_stop();
            while !CONSOLE_DONE.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            TRUE
        }
        _ => FALSE,
    }
}

// SAFETY: called by the service control dispatcher; the argument vector
// is not read.
unsafe extern "system" fn service_main(_argc: u32, _argv: *mut PWSTR) {
    let Some(service) = SERVICE.get() else {
        return;
    };
    // SAFETY: the name is a NUL-terminated UTF-16 string in a static;
    // `control_handler` has the LPHANDLER_FUNCTION_EX signature and lives
    // as long as the process; the context pointer is not used.
    let handle = unsafe {
        RegisterServiceCtrlHandlerExW(
            service.name.as_ptr(),
            Some(control_handler),
            core::ptr::null(),
        )
    };
    if handle == 0 {
        EXIT.store(1, Ordering::SeqCst);
        return;
    }
    STATUS_HANDLE.store(handle, Ordering::SeqCst);
    report(SERVICE_START_PENDING, NO_ERROR, 0, START_WAIT_HINT_MS);
    mark_running_as_service();
    let code = exit_code_number((service.daemon)());
    EXIT.store(code, Ordering::SeqCst);
    if code == 0 {
        report(SERVICE_STOPPED, NO_ERROR, 0, 0);
    } else {
        report(SERVICE_STOPPED, ERROR_SERVICE_SPECIFIC_ERROR, code, 0);
    }
}

// SAFETY: called by the service control manager; the event data and
// context pointers are not read.
unsafe extern "system" fn control_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
) -> u32 {
    match service_control(control) {
        ServiceControl::Stop => {
            report(SERVICE_STOP_PENDING, NO_ERROR, 0, STOP_WAIT_HINT_MS);
            request_service_stop();
            NO_ERROR
        }
        ServiceControl::Reload => {
            request_service_reload();
            NO_ERROR
        }
        ServiceControl::Interrogate => NO_ERROR,
        ServiceControl::NotImplemented => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

/// [`super::notify_service_manager`] on Windows: the state to the service
/// control manager, in service mode.
pub(super) fn notify(state: &str) {
    match service_state_of(state) {
        Some(ServiceState::Running) => report(SERVICE_RUNNING, NO_ERROR, 0, 0),
        Some(ServiceState::StopPending) => {
            report(SERVICE_STOP_PENDING, NO_ERROR, 0, STOP_WAIT_HINT_MS);
        }
        None => {}
    }
}

/// `SetServiceStatus` with `state`; the running service takes the stop,
/// shutdown and paramchange controls, a pending state counts its
/// checkpoint up. Nothing outside service mode.
fn report(state: SERVICE_STATUS_CURRENT_STATE, win32_exit: u32, specific: u32, wait_hint: u32) {
    let handle = STATUS_HANDLE.load(Ordering::SeqCst);
    if handle == 0 {
        return;
    }
    let pending = state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING;
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN | SERVICE_ACCEPT_PARAMCHANGE
        } else {
            0
        },
        dwWin32ExitCode: win32_exit,
        dwServiceSpecificExitCode: specific,
        dwCheckPoint: if pending {
            CHECKPOINT.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            0
        },
        dwWaitHint: wait_hint,
    };
    // SAFETY: `handle` came from RegisterServiceCtrlHandlerExW of this
    // process (it needs no closing); `status` is a valid SERVICE_STATUS
    // for the duration of the call.
    unsafe {
        SetServiceStatus(handle, &status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Services::{
        SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_PARAMCHANGE, SERVICE_CONTROL_SHUTDOWN,
        SERVICE_CONTROL_STOP,
    };

    /// The values `service_control` matches are the Win32 constants.
    #[test]
    fn deployment_4_3_the_control_codes_are_the_win32_constants() {
        assert_eq!(service_control(SERVICE_CONTROL_STOP), ServiceControl::Stop);
        assert_eq!(
            service_control(SERVICE_CONTROL_SHUTDOWN),
            ServiceControl::Stop
        );
        assert_eq!(
            service_control(SERVICE_CONTROL_PARAMCHANGE),
            ServiceControl::Reload
        );
        assert_eq!(
            service_control(SERVICE_CONTROL_INTERROGATE),
            ServiceControl::Interrogate
        );
    }
}
