//! TapHook trait + registry.
//!
//! When the Cargo feature `inspect` is on, external consumers
//! (the Reality Inspector over the side channel or local tools) can
//! register as a `TapHook`. The DDS stack calls `dispatch_*` at
//! defined insertion points — without the feature this is all gone.

use crate::frame::Frame;

/// Marker trait for a tap hook. Implementations must be `Send +
/// Sync` and do only **non-blocking** operations (hot path).
pub trait TapHook: Send + Sync + 'static {
    /// Called by the inspect endpoint when a frame is produced at this
    /// layer. Implementations should queue the frame or
    /// process it best-effort — **do not block**.
    fn on_frame(&self, frame: &Frame);
}

#[cfg(feature = "inspect")]
mod registry {
    use std::sync::{Mutex, OnceLock};

    use super::TapHook;
    use crate::frame::{Frame, FrameKind};

    /// A layer registry holds 0..N hooks per layer.
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

    /// Registers a hook at the DCPS layer.
    pub fn register_dcps_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Dcps).push(hook);
    }
    /// Registers a hook at the RTPS layer.
    pub fn register_rtps_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Rtps).push(hook);
    }
    /// Registers a hook at the transport layer.
    pub fn register_transport_tap(hook: Box<dyn TapHook>) {
        registry(FrameKind::Transport).push(hook);
    }

    /// Distributes a frame to all hooks at the matching layer.
    /// Called by the dcps/rtps/transport crates.
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
        // The RTPS counter is not incremented
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
