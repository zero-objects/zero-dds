//! L2 latency probe — drives the **C-API FFI delivery path** directly
//! (writer_write + reader listener callback + a condvar wait), with NO rclpy,
//! NO rosidl serialize, NO rmw_zerodds.c. This isolates the c-api + listener +
//! inbox/condvar glue (the exact pattern the rmw shim's `SubInbox` uses) on top
//! of the dcps core, so the gap vs the pure-Rust `latency_ping` (native reader)
//! is the c-api/listener cost, and the gap vs the rmw bench is rclpy+rmw.c.
//!
//! Usage (two processes, same host):
//!   capi_latency pong
//!   capi_latency ping <size> <count>
#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::collections::VecDeque;
use std::ffi::{CString, c_void};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use zerodds::{
    zerodds_reader_create, zerodds_reader_set_data_callback, zerodds_runtime_create,
    zerodds_writer_create, zerodds_writer_write,
};

/// Mirrors the shim `SubInbox`: a queue of received seqs + a condvar.
struct Inbox {
    queue: Mutex<VecDeque<u32>>,
    cv: Condvar,
}

extern "C" fn on_data(ud: *mut c_void, payload: *const u8, len: usize, _repr: u8, _be: u8) {
    if ud.is_null() || len < 4 || payload.is_null() {
        return;
    }
    // SAFETY: ud is a live &Inbox for the program lifetime; payload[0..len] valid for this call.
    let inbox = unsafe { &*(ud as *const Inbox) };
    // SAFETY: payload is non-NULL with at least `len` (>= 4) readable bytes for
    // the duration of this callback (the runtime owns the listener buffer).
    let seq = unsafe {
        let s = core::slice::from_raw_parts(payload, len);
        u32::from_le_bytes([s[0], s[1], s[2], s[3]])
    };
    if let Ok(mut q) = inbox.queue.lock() {
        q.push_back(seq);
    }
    inbox.cv.notify_all();
}

fn pct(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn main() {
    let role = std::env::args().nth(1).unwrap_or_else(|| "ping".into());
    let size: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);
    let count: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    let ping_topic = CString::new("rt/ping").unwrap();
    let pong_topic = CString::new("rt/pong").unwrap();
    let type_name = CString::new("zerodds::LatencyMsg").unwrap();

    let inbox = Box::leak(Box::new(Inbox {
        queue: Mutex::new(VecDeque::new()),
        cv: Condvar::new(),
    }));
    let inbox_ptr = (inbox as *const Inbox) as *mut c_void;

    // SAFETY: all FFI args are valid for the program lifetime.
    unsafe {
        let rt = zerodds_runtime_create(0);
        assert!(!rt.is_null(), "runtime");

        if role == "pong" {
            let reader = zerodds_reader_create(rt, ping_topic.as_ptr(), type_name.as_ptr(), 1);
            let writer = zerodds_writer_create(rt, pong_topic.as_ptr(), type_name.as_ptr(), 1);
            assert!(!reader.is_null() && !writer.is_null());
            zerodds_reader_set_data_callback(reader, Some(on_data), inbox_ptr);
            // Echo loop: on each received seq, write back the same-sized payload.
            let mut payload = vec![0xABu8; size];
            println!("capi_latency pong: echoing rt/ping -> rt/pong");
            loop {
                let seq = {
                    let mut q = inbox.queue.lock().unwrap();
                    loop {
                        if let Some(s) = q.pop_front() {
                            break s;
                        }
                        let (g, _) = inbox.cv.wait_timeout(q, Duration::from_secs(5)).unwrap();
                        q = g;
                        if q.is_empty() {
                            // timeout heartbeat; keep waiting
                            continue;
                        }
                    }
                };
                payload[0..4].copy_from_slice(&seq.to_le_bytes());
                zerodds_writer_write(writer, payload.as_ptr(), size);
            }
        }

        // ping role
        let writer = zerodds_writer_create(rt, ping_topic.as_ptr(), type_name.as_ptr(), 1);
        let reader = zerodds_reader_create(rt, pong_topic.as_ptr(), type_name.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());
        zerodds_reader_set_data_callback(reader, Some(on_data), inbox_ptr);

        // Discovery settle (localhost completes well under a second).
        println!("capi_latency ping: settling discovery 3s");
        std::thread::sleep(Duration::from_secs(3));

        let warmup = 10u32;
        let total = warmup + count as u32;
        let mut payload = vec![0xABu8; size];
        let mut rtts: Vec<u64> = Vec::with_capacity(count);
        let mut lost = 0u32;
        for seq in 0..total {
            payload[0..4].copy_from_slice(&seq.to_le_bytes());
            let t0 = Instant::now();
            zerodds_writer_write(writer, payload.as_ptr(), size);
            // wait for echo seq
            let echo_deadline = Instant::now() + Duration::from_secs(2);
            let mut got = false;
            let mut q = inbox.queue.lock().unwrap();
            loop {
                while let Some(s) = q.pop_front() {
                    if s == seq {
                        got = true;
                    }
                }
                if got || Instant::now() >= echo_deadline {
                    break;
                }
                let (g, _) = inbox
                    .cv
                    .wait_timeout(q, Duration::from_millis(500))
                    .unwrap();
                q = g;
            }
            drop(q);
            if got {
                if seq >= warmup {
                    rtts.push(t0.elapsed().as_micros() as u64);
                }
            } else if seq >= warmup {
                lost += 1;
            }
        }
        rtts.sort_unstable();
        if rtts.is_empty() {
            println!("== no RTTs (no pong?) ==");
            std::process::exit(2);
        }
        let n = rtts.len();
        let mean = rtts.iter().sum::<u64>() as f64 / n as f64;
        println!(
            "== L2 c-api RTT ({size} B, {n} samples, {lost} lost): min={} p50={} p90={} p99={} max={} mean={:.0} (µs) ==",
            rtts[0],
            pct(&rtts, 50.0),
            pct(&rtts, 90.0),
            pct(&rtts, 99.0),
            rtts[n - 1],
            mean
        );
    }
}
