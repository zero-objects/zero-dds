// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `shape_main` — ZeroDDS reference client for the OMG `dds-rtps`
//! interoperability test suite (`github.com/omg-dds/dds-rtps`).
//!
//! BUILD + LOCAL-VALIDATE phase artifact. This binary is **not**
//! submitted anywhere — no CLA PR, no source PR, no release asset, no
//! push to `github.com/omg-dds`. That step is held back for Sandra.
//!
//! It is a thin CLI + protocol shim over ZeroDDS's existing DCPS API
//! (`crates/dcps`) and QoS machinery (`crates/qos`) — no new wire/CDR/QoS
//! code lives here. See `tools/omg-shape-main/README.md` for the full
//! flag-coverage table and the exact `pexpect` strings this binary
//! reproduces.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod cli;
mod listeners;
mod qos_map;
mod shape_filter;

use std::cell::Cell;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration as StdDuration;

use clap::Parser;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::psm_constants::status as status_bits;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReader, DataWriter, DdsType, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, HANDLE_NIL, InstanceHandle, InstanceStateKind, Sample, TopicQos,
};

use cli::{AccessScopeArg, FinalInstanceStateArg, Options};
use listeners::ShapeListener;

type BoxError = Box<dyn std::error::Error>;

fn main() -> ExitCode {
    let opts = Options::parse();
    if opts.publish == opts.subscribe {
        eprintln!("shape_main: exactly one of -P or -S is required");
        return ExitCode::FAILURE;
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        if let Err(e) = ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        }) {
            eprintln!("shape_main: warning: SIGINT handler not installed: {e}");
        }
    }

    let result = if opts.publish {
        run_publisher(&opts, &stop)
    } else {
        run_subscriber(&opts, &stop)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("shape_main: error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `<base>`, `<base>1`, `<base>2`, ... — the `--num-instances` /
/// `--num-topics` naming convention documented in the README.
fn numbered(base: &str, n: u32) -> Vec<String> {
    (0..n.max(1))
        .map(|i| {
            if i == 0 {
                base.to_string()
            } else {
                format!("{base}{i}")
            }
        })
        .collect()
}

/// Canonical ShapesDemo sine-wave motion on the 240x270 canvas.
fn motion(t: f32) -> (i32, i32) {
    #[allow(clippy::cast_possible_truncation)]
    let x = (120.0 + 80.0 * t.sin()) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let y = (135.0 + 90.0 * (t * 1.3).cos()) as i32;
    (x, y)
}

fn create_participant(
    factory: &DomainParticipantFactory,
    opts: &Options,
) -> zerodds_dcps::Result<DomainParticipant> {
    if opts.periodic_announcement_ms > 0 {
        let config = RuntimeConfig {
            spdp_period: StdDuration::from_millis(u64::from(opts.periodic_announcement_ms)),
            ..RuntimeConfig::default()
        };
        factory.create_participant_with_config(
            opts.domain_id,
            DomainParticipantQos::default(),
            config,
        )
    } else {
        factory.create_participant(opts.domain_id, DomainParticipantQos::default())
    }
}

// ============================================================================
// Publisher
// ============================================================================

struct WriterCtx {
    topic_name: String,
    writer: DataWriter<ShapeType>,
    instances: Vec<(String, InstanceHandle)>,
}

fn run_publisher(opts: &Options, stop: &Arc<AtomicBool>) -> Result<(), BoxError> {
    let factory = DomainParticipantFactory::instance();
    let participant = create_participant(factory, opts)?;
    let publisher = participant.create_publisher(qos_map::publisher_qos(opts));

    let topic_names = numbered(&opts.topic_name, opts.num_topics);
    let colors = numbered(opts.resolved_color(), opts.num_instances);
    let base_size = if opts.shapesize <= 0 {
        20
    } else {
        opts.shapesize
    };

    let mut writers = Vec::with_capacity(topic_names.len());
    for tname in &topic_names {
        println!("Create topic: {tname}");
        let topic = participant.create_topic::<ShapeType>(tname, TopicQos::default())?;
        let writer = publisher.create_datawriter::<ShapeType>(&topic, qos_map::writer_qos(opts))?;
        println!(
            "Create writer for topic: {tname} color: {}",
            opts.resolved_color()
        );
        let listener = Arc::new(ShapeListener::new(tname.clone(), ShapeType::TYPE_NAME));
        writer.set_listener(Some(listener), status_bits::ANY);

        let mut instances = Vec::with_capacity(colors.len());
        for color in &colors {
            let seed = ShapeType::new(color.clone(), 0, 0, base_size);
            let handle = writer.register_instance(&seed)?;
            instances.push((color.clone(), handle));
        }
        writers.push(WriterCtx {
            topic_name: tname.clone(),
            writer,
            instances,
        });
    }

    let write_period = StdDuration::from_millis(opts.write_period_ms.max(1));
    let mut iteration: u64 = 0;
    let mut t: f32 = 0.0;

    while !stop.load(Ordering::SeqCst) {
        if let Some(max) = opts.num_iterations {
            if iteration >= max {
                break;
            }
        }
        let (x, y) = motion(t);
        // `-z 0`: "increase the size for every sample". The harness's own
        // check functions (`test_suite_functions.py`
        // `test_reliability_no_losses_w_instances` /
        // `test_order_w_instances`) treat `[shapesize]` as an implicit
        // per-instance sequence counter — RELIABLE tests require it to
        // increment by *exactly* 1 per sample, BEST_EFFORT tests only
        // require it to strictly increase (never repeat or go backwards).
        // Every writer iteration here writes one sample per instance with
        // the *same* `size`, and `size` itself increments by exactly 1
        // per iteration, so each instance's own successive samples are
        // always +1 — deliberately unbounded (no modulo wrap), since a
        // wrap would violate both check functions.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let size = if opts.shapesize == 0 {
            20 + iteration as i32
        } else {
            opts.shapesize
        };
        for wctx in &writers {
            for (color, _handle) in &wctx.instances {
                let sample = ShapeType::new(color.clone(), x, y, size);
                wctx.writer.write(&sample)?;
                if opts.print_writer_samples {
                    println!("{:<10} {color:<10} {x:03} {y:03} [{size}]", wctx.topic_name);
                }
            }
            wctx.writer.drive_listeners();
        }
        t += 0.15;
        iteration += 1;
        std::thread::sleep(write_period);
    }

    if let Some(fis) = opts.final_instance_state {
        for wctx in &writers {
            for (color, handle) in &wctx.instances {
                let sample = ShapeType::new(color.clone(), 0, 0, base_size);
                match fis {
                    FinalInstanceStateArg::U => {
                        wctx.writer.unregister_instance(&sample, *handle)?;
                    }
                    FinalInstanceStateArg::D => {
                        wctx.writer.dispose(&sample, *handle)?;
                    }
                }
            }
        }
    }
    Ok(())
}

// ============================================================================
// Subscriber
// ============================================================================

struct ReaderCtx {
    topic_name: String,
    reader: DataReader<ShapeType>,
    prev_instance: Cell<InstanceHandle>,
}

fn run_subscriber(opts: &Options, stop: &Arc<AtomicBool>) -> Result<(), BoxError> {
    let factory = DomainParticipantFactory::instance();
    let participant = create_participant(factory, opts)?;
    let subscriber = participant.create_subscriber(qos_map::subscriber_qos(opts));

    let topic_names = numbered(&opts.topic_name, opts.num_topics);
    let group_coherent = opts.coherent && matches!(opts.access_scope, Some(AccessScopeArg::G));

    let mut readers = Vec::with_capacity(topic_names.len());
    for tname in &topic_names {
        println!("Create topic: {tname}");
        let topic = participant.create_topic::<ShapeType>(tname, TopicQos::default())?;
        let mut reader =
            subscriber.create_datareader::<ShapeType>(&topic, qos_map::reader_qos(opts))?;

        if let Some(expr) = &opts.cft {
            match shape_filter::compile(expr) {
                Ok(f) => reader = reader.with_filter(f),
                Err(_) => {
                    println!("failed to create content filtered topic");
                    continue;
                }
            }
        }

        println!("Create reader for topic: {tname} ");
        let listener = Arc::new(ShapeListener::new(tname.clone(), ShapeType::TYPE_NAME));
        reader.set_listener(Some(listener), status_bits::ANY);
        readers.push(ReaderCtx {
            topic_name: tname.clone(),
            reader,
            prev_instance: Cell::new(HANDLE_NIL),
        });
    }

    // README `--take-read`: uses take()/read() instead of
    // take_next_instance()/read_next_instance(). A `--cft` filter is only
    // documented to apply on the plain take()/read() path
    // (`DataReader::with_filter` doc comment), so a CFT forces this too.
    let use_plain = opts.take_read || opts.cft.is_some();

    let read_period = StdDuration::from_millis(opts.read_period_ms.max(1));
    let mut iteration: u64 = 0;

    while !stop.load(Ordering::SeqCst) {
        if let Some(max) = opts.num_iterations {
            if iteration >= max {
                break;
            }
        }
        for rctx in &readers {
            rctx.reader.drive_listeners();

            if group_coherent {
                subscriber.begin_access();
                println!("Reading coherent sets");
            }
            if opts.ordered {
                println!("Reading with ordered access");
            }

            let samples = if use_plain {
                if opts.use_read {
                    rctx.reader.read_with_info()
                } else {
                    rctx.reader.take_with_info()
                }
            } else {
                next_instance_batch(&rctx.reader, &rctx.prev_instance, opts.use_read)
            };

            if group_coherent {
                let _ = subscriber.end_access();
            }

            if let Ok(samples) = samples {
                for s in &samples {
                    print_sample(&rctx.topic_name, s);
                }
            }
        }
        iteration += 1;
        std::thread::sleep(read_period);
    }
    Ok(())
}

/// `take_next_instance`/`read_next_instance`, cycling through every known
/// instance once per call (README default subscriber behaviour, absent
/// `--take-read`).
///
/// **Known upstream quirk** (see crate README "Known runtime quirks"):
/// `DataReader::{take,read}_next_instance` resolve the next instance via
/// `InstanceTracker::next_handle_after` *before* draining the incoming
/// sample channel into the cache (that drain — `ingest_into_cache` — only
/// runs inside `take_instance`/`read_instance`, which are never reached if
/// `next_handle_after` already returns `None`). On a reader that has never
/// called a plain `take`/`read`/`*_with_info` yet, the instance tracker is
/// still empty, so the very first `take_next_instance(HANDLE_NIL)` always
/// returns an empty `Vec` — even with data already in flight. A
/// non-consuming `read_with_info()` call forces that same ingestion path
/// as a side effect without disturbing the cache for the following
/// `take_next_instance`/`read_next_instance` call, so we do exactly that
/// once per poll tick before resolving the next instance. This is an
/// app-level workaround; the shared `crates/dcps` fix (call
/// `ingest_into_cache()` unconditionally at the top of
/// `take_next_instance`/`read_next_instance`, ahead of the
/// `next_handle_after` lookup) is out of scope for this binary's isolated
/// worktree and is called out for a separate fix.
fn next_instance_batch(
    reader: &DataReader<ShapeType>,
    prev: &Cell<InstanceHandle>,
    use_read: bool,
) -> zerodds_dcps::Result<Vec<Sample<ShapeType>>> {
    let _ = reader.read_with_info();
    let previous = prev.get();
    let out = if use_read {
        reader.read_next_instance(previous)
    } else {
        reader.take_next_instance(previous)
    };
    match &out {
        Ok(samples) => {
            if let Some(last) = samples.last() {
                prev.set(last.info.instance_handle);
            } else {
                prev.set(HANDLE_NIL);
            }
        }
        Err(_) => prev.set(HANDLE_NIL),
    }
    out
}

/// Prints the exact `basic_check`-compatible sample line
/// (`<topic> <color> <x> <y> [<shapesize>]`), or a lifecycle-marker line
/// for a dispose/unregister sample (`valid_data == false`).
fn print_sample(topic: &str, s: &Sample<ShapeType>) {
    if s.info.valid_data {
        println!(
            "{topic:<10} {:<10} {:03} {:03} [{}]",
            s.data.color, s.data.x, s.data.y, s.data.shapesize
        );
    } else {
        let state = match s.info.instance_state {
            InstanceStateKind::NotAliveDisposed => "NOT_ALIVE_DISPOSED_INSTANCE_STATE",
            InstanceStateKind::NotAliveNoWriters => "NOT_ALIVE_NO_WRITERS_INSTANCE_STATE",
            InstanceStateKind::Alive => "ALIVE_INSTANCE_STATE",
        };
        println!("{topic:<10} {:<10} {state}", s.data.color);
    }
}
