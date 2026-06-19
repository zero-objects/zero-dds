// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The forwarding engine: a pump that moves samples from one user reader to one
//! user writer (possibly on a different domain), type-agnostically.
//!
//! Built on the runtime user-entity byte path (`register_user_reader_kind` →
//! `mpsc::Receiver<UserSample>` → `write_user_sample_borrowed`), the same
//! primitive the durability service uses. The engine adds: per-sample loop
//! guard, representation-faithful encapsulation override, keyed-instance
//! lifecycle propagation, and a pluggable [`SampleProcessor`] hook for content
//! filtering / field transformation (wired in v3).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, UserSample};
use zerodds_rtps::EntityId;

use crate::metrics::RouteMetrics;

/// Per-sample processor: content filter and/or field transformation.
///
/// `process` receives the input CDR body (no encapsulation header) and its
/// representation (`0` = XCDR1, `1`/`2` = XCDR2). It returns:
/// * `Some((payload, representation))` — forward this (possibly transformed)
///   body with the given representation.
/// * `None` — drop the sample (filtered out).
///
/// Byte pass-through routes use no processor (the body is forwarded verbatim).
pub trait SampleProcessor: Send {
    /// Process one sample body.
    fn process(&mut self, payload: &[u8], representation: u8) -> Option<(Vec<u8>, u8)>;
}

/// Inputs needed to build a [`ForwardingSession`]. The engine registers the
/// reader/writer and hands the pieces over.
pub struct SessionParts {
    /// Route name (for metrics + thread name).
    pub route_name: String,
    /// Channel of samples from the input reader.
    pub rx: Receiver<UserSample>,
    /// Output runtime (the participant on the output domain).
    pub output_rt: Arc<DcpsRuntime>,
    /// Output writer entity id.
    pub writer_eid: EntityId,
    /// Whether the topic is keyed (governs lifecycle forwarding).
    pub keyed: bool,
    /// Optional content-filter / transform processor (v3).
    pub processor: Option<Box<dyn SampleProcessor>>,
    /// Metrics sink for this route.
    pub metrics: Arc<RouteMetrics>,
}

/// A running forward pump for one route.
pub struct ForwardingSession {
    route_name: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ForwardingSession {
    /// Starts the pump thread.
    ///
    /// # Errors
    /// [`RoutingError::Dds`] if the OS refuses to spawn the pump thread.
    pub fn start(parts: SessionParts) -> crate::error::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = Arc::clone(&stop);
        let route_name = parts.route_name.clone();
        let handle = std::thread::Builder::new()
            .name(format!("router-pump-{}", parts.route_name))
            .spawn(move || pump(parts, &pump_stop))
            .map_err(|e| {
                crate::error::RoutingError::Dds(format!("spawn router pump '{route_name}': {e}"))
            })?;
        Ok(Self {
            route_name,
            stop,
            handle: Some(handle),
        })
    }

    /// Route name this session forwards.
    #[must_use]
    pub fn route_name(&self) -> &str {
        &self.route_name
    }

    /// Signals the pump to stop and joins it.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for ForwardingSession {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Maps a lifecycle [`ChangeKind`](zerodds_rtps::history_cache::ChangeKind) to
/// the RTPS `PID_STATUS_INFO` bits (Spec §9.6.3.9).
fn status_bits(kind: zerodds_rtps::history_cache::ChangeKind) -> u32 {
    use zerodds_rtps::history_cache::ChangeKind;
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    match kind {
        ChangeKind::NotAliveDisposed => DISPOSED,
        ChangeKind::NotAliveUnregistered => UNREGISTERED,
        ChangeKind::NotAliveDisposedUnregistered => DISPOSED | UNREGISTERED,
        // Alive / AliveFiltered never reach here (handled as data).
        ChangeKind::Alive | ChangeKind::AliveFiltered => 0,
    }
}

fn pump(mut parts: SessionParts, stop: &AtomicBool) {
    // Track the last representation written so the encapsulation override is
    // set only on change (a topic is representation-consistent in practice).
    let mut last_rep: u8 = 255;
    while !stop.load(Ordering::Relaxed) {
        let sample = match parts.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(s) => s,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match sample {
            UserSample::Alive {
                payload,
                representation,
                ..
            } => {
                // Filter / transform, or byte pass-through.
                if let Some(p) = parts.processor.as_mut() {
                    match p.process(payload.as_slice(), representation) {
                        Some((out, rep)) => {
                            set_rep(&parts, &mut last_rep, rep);
                            write_alive(&parts, &out);
                        }
                        None => parts.metrics.inc_dropped_filter(),
                    }
                } else {
                    set_rep(&parts, &mut last_rep, representation);
                    write_alive(&parts, payload.as_slice());
                }
            }
            UserSample::Lifecycle { key_hash, kind } => {
                // Lifecycle (dispose/unregister) only carries meaning for keyed
                // topics; forward it so a downstream reader sees the instance
                // state transition.
                if parts.keyed {
                    if parts
                        .output_rt
                        .write_user_lifecycle(parts.writer_eid, key_hash, status_bits(kind))
                        .is_ok()
                    {
                        parts.metrics.inc_lifecycle();
                    } else {
                        parts.metrics.inc_errors();
                    }
                }
            }
        }
    }
}

fn set_rep(parts: &SessionParts, last_rep: &mut u8, rep: u8) {
    if rep != *last_rep {
        // representation: 0 = XCDR1, 1/2 = XCDR2 → data_representation id 0 / 2.
        let off: i16 = if rep == 0 { 0 } else { 2 };
        let _ = parts
            .output_rt
            .set_user_writer_data_rep_override(parts.writer_eid, Some(vec![off]));
        *last_rep = rep;
    }
}

fn write_alive(parts: &SessionParts, body: &[u8]) {
    if parts
        .output_rt
        .write_user_sample_borrowed(parts.writer_eid, body)
        .is_ok()
    {
        parts.metrics.inc_forwarded();
    } else {
        parts.metrics.inc_errors();
    }
}
