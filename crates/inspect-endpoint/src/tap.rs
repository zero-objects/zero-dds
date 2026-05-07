//! Tap-Hook-Trait + Registry.
//!
//! Wenn das Cargo-Feature `inspect` an ist, koennen externe Konsumenten
//! (Reality Inspector ueber Side-Channel oder lokale Tools) sich als
//! `TapHook` registrieren. Der DDS-Stack ruft `dispatch_*` an
//! definierten Insertion-Points — ohne Feature ist das alles weg.

use crate::frame::Frame;

/// Marker-Trait fuer einen Tap-Hook. Implementierungen muessen `Send +
/// Sync` sein und nur **non-blocking** Operationen tun (Hot-Path).
pub trait TapHook: Send + Sync + 'static {
    /// Wird vom Inspect-Endpoint aufgerufen, wenn ein Frame an diesem
    /// Layer entsteht. Implementierungen sollen den Frame queueen oder
    /// best-effort verarbeiten — **nicht blockieren**.
    fn on_frame(&self, frame: &Frame);
}

#[cfg(feature = "inspect")]
mod registry {
    use std::sync::{Mutex, OnceLock};

    use super::TapHook;
    use crate::frame::{Frame, FrameKind};

    /// Eine Layer-Registry haelt 0..N Hooks pro Layer.
    struct LayerRegistry {
        hooks: Mutex<Vec<Box<dyn TapHook>>>,
    }

    impl LayerRegistry {
        const fn new() -> Self {
            Self {
                hooks: Mutex::new(Vec::new()),
            }
        }
        fn push(&self, hook: Box<dyn TapHook>) {
            if let Ok(mut hooks) = self.hooks.lock() {
                hooks.push(hook);
            }
        }
        fn dispatch(&self, frame: &Frame) {
            if let Ok(hooks) = self.hooks.lock() {
                for h in hooks.iter() {
                    h.on_frame(frame);
                }
            }
        }
    }

    static DCPS: OnceLock<LayerRegistry> = OnceLock::new();
    static RTPS: OnceLock<LayerRegistry> = OnceLock::new();
    static TRANSPORT: OnceLock<LayerRegistry> = OnceLock::new();

    fn registry(kind: FrameKind) -> &'static LayerRegistry {
        match kind {
            FrameKind::Dcps => DCPS.get_or_init(LayerRegistry::new),
            FrameKind::Rtps => RTPS.get_or_init(LayerRegistry::new),
            FrameKind::Transport => TRANSPORT.get_or_init(LayerRegistry::new),
        }
    }

    /// Registriert einen Hook am DCPS-Layer.
    pub fn register_dcps_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Dcps).push(hook);
    }
    /// Registriert einen Hook am RTPS-Layer.
    pub fn register_rtps_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Rtps).push(hook);
    }
    /// Registriert einen Hook am Transport-Layer.
    pub fn register_transport_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Transport).push(hook);
    }

    /// Verteilt einen Frame an alle Hooks am passenden Layer.
    /// Wird von dcps/rtps/transport-Crates aufgerufen.
    pub fn dispatch(frame: &Frame) {
        registry(frame.kind).dispatch(frame);
    }
}

#[cfg(feature = "inspect")]
pub use registry::{dispatch, register_dcps_tap, register_rtps_tap, register_transport_tap};

#[cfg(all(test, feature = "inspect"))]
mod tests {
    use super::*;
    use crate::frame::Frame;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHook(Arc<AtomicUsize>);
    impl TapHook for CountingHook {
        fn on_frame(&self, _frame: &Frame) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dcps_hook_receives_dcps_frames() {
        let counter = Arc::new(AtomicUsize::new(0));
        register_dcps_tap(Box::new(CountingHook(Arc::clone(&counter))));
        dispatch(&Frame::dcps("t".into(), 0, 0, vec![]));
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn rtps_hook_does_not_receive_dcps_frames() {
        let counter = Arc::new(AtomicUsize::new(0));
        register_rtps_tap(Box::new(CountingHook(Arc::clone(&counter))));
        let initial = counter.load(Ordering::SeqCst);
        dispatch(&Frame::dcps("t".into(), 0, 0, vec![]));
        // RTPS-Counter ist nicht inkrementiert
        assert_eq!(counter.load(Ordering::SeqCst), initial);
    }

    #[test]
    fn transport_hook_receives_transport_frames() {
        let counter = Arc::new(AtomicUsize::new(0));
        register_transport_tap(Box::new(CountingHook(Arc::clone(&counter))));
        dispatch(&Frame::transport(0, 0, vec![0xFF]));
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }
}
