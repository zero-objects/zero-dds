//! WP-D Integration-Test: DCPS-Tap-Hook empfaengt einen Frame nach
//! `write_user_sample`.
//!
//! Nur aktiv wenn das Feature `inspect` an ist.

#![cfg(feature = "inspect")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_inspect_endpoint::{Frame, FrameKind, TapHook, tap};

/// Hook der einen geteilten Atomic + Vec befuellt. Wird via
/// `Box<CapturingHook>` registriert; die innere Mutex erlaubt
/// dem Test auf die Frames zuzugreifen.
struct CapturingHook {
    state: Arc<HookState>,
}

struct HookState {
    count: AtomicUsize,
    frames: Mutex<Vec<Frame>>,
}

impl CapturingHook {
    fn new() -> (Self, Arc<HookState>) {
        let state = Arc::new(HookState {
            count: AtomicUsize::new(0),
            frames: Mutex::new(Vec::new()),
        });
        (
            Self {
                state: Arc::clone(&state),
            },
            state,
        )
    }
}

impl TapHook for CapturingHook {
    fn on_frame(&self, frame: &Frame) {
        self.state.count.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut frames) = self.state.frames.lock() {
            frames.push(frame.clone());
        }
    }
}

#[test]
fn dcps_tap_sees_frames_when_dispatched_directly() {
    // Smoke-Test der Tap-Registry — verifiziert dass die
    // dispatch-API erreichbar ist und die Hook-Sammlung
    // funktioniert. Ein End-to-End-Test der durch den
    // DcpsRuntime laeuft braucht ein vollstaendiges
    // Participant-Setup mit Discovery, das ist Aufgabe
    // separater e2e-Tests.
    let (hook, state) = CapturingHook::new();
    tap::register_dcps_tap(Box::new(hook));

    let frame = Frame::dcps("Speed".into(), 1_000_000, 42, vec![1, 2, 3]);
    tap::dispatch(&frame);

    assert!(state.count.load(Ordering::SeqCst) >= 1);
    let frames = state.frames.lock().expect("frames lock");
    let speed_frame = frames.iter().find(|f| f.topic == "Speed");
    assert!(speed_frame.is_some(), "Speed-Frame nicht empfangen");
    let speed_frame = speed_frame.expect("found above");
    assert_eq!(speed_frame.kind, FrameKind::Dcps);
    assert_eq!(speed_frame.payload, vec![1, 2, 3]);
}

#[test]
fn dcps_tap_handles_alive_payload() {
    // Verifiziert dass Frame::dcps fuer typische Receive-Payloads
    // (CDR-encoded Sample) korrekt routed wird. Volle End-to-End-
    // Tests durch handle_user_datagram leben separat in den e2e-
    // Suites.
    let (hook, state) = CapturingHook::new();
    tap::register_dcps_tap(Box::new(hook));

    let payload = b"\x00\x01\x02\x03sample-data".to_vec();
    let frame = Frame::dcps("Receive".into(), 2_000_000, 7, payload.clone());
    tap::dispatch(&frame);

    let frames = state.frames.lock().expect("frames lock");
    let receive_frame = frames.iter().find(|f| f.topic == "Receive");
    assert!(receive_frame.is_some());
    let receive_frame = receive_frame.expect("found above");
    assert_eq!(receive_frame.payload, payload);
}

#[test]
fn rtps_layer_does_not_receive_dcps_dispatch() {
    let (dcps_hook, dcps_state) = CapturingHook::new();
    let (rtps_hook, rtps_state) = CapturingHook::new();
    tap::register_dcps_tap(Box::new(dcps_hook));
    tap::register_rtps_tap(Box::new(rtps_hook));

    let initial_dcps = dcps_state.count.load(Ordering::SeqCst);
    let initial_rtps = rtps_state.count.load(Ordering::SeqCst);
    tap::dispatch(&Frame::dcps("X".into(), 0, 1, vec![]));

    assert!(dcps_state.count.load(Ordering::SeqCst) > initial_dcps);
    assert_eq!(rtps_state.count.load(Ordering::SeqCst), initial_rtps);
}
