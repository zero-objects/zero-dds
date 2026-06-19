//! Multi-endpoint soak (CI-4c wave).
//!
//! Creates N topics + N writers (mode `pub_n`) or N readers (mode `sub_n`)
//! in **one** DomainParticipant. Stress test for endpoint scaling
//! in the DDS runtime: WriterCache, ReaderCache, SEDP endpoint list,
//! discovery match loops with N x N match points.
//!
//! # Modes
//!
//! * `pub_n <count> <runtime_secs>` — N writers on topics
//!   `MultiPerf0`..`MultiPerf{N-1}`, each writing 1 sample/second.
//! * `sub_n <count> <runtime_secs>` — N readers on the same topics,
//!   counting received samples. Output every 60 s with total + per-topic
//!   delta.
//!
//! # Choosing N
//!
//! Default CI-4c wave: N=100. Empirically sustainable with 16+ GB RAM
//! (each reader/writer ~50-200 KB history cache + discovery state).

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::time::{Duration, Instant};

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    SubscriberQos, TopicQos,
};

fn topic_name(i: usize) -> String {
    format!("MultiPerf{i}")
}

fn run_pub_n(count: usize, runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());

    let mut writers = Vec::with_capacity(count);
    println!("[multi_pub] creating {count} writers");
    let create_start = Instant::now();
    for i in 0..count {
        let topic = participant.create_topic::<ShapeType>(&topic_name(i), TopicQos::default())?;
        let writer = publisher.create_datawriter::<ShapeType>(&topic, DataWriterQos::default())?;
        writers.push(writer);
    }
    println!(
        "[multi_pub] created {count} writers in {:.2}s",
        create_start.elapsed().as_secs_f64()
    );

    let start = Instant::now();
    let mut tick = 0u64;
    let mut total_writes = 0u64;
    while start.elapsed() < runtime {
        let tick_start = Instant::now();
        for (i, w) in writers.iter().enumerate() {
            let s = ShapeType::new(format!("MEP{i:04}"), tick as i32, 0, 30);
            if w.write(&s).is_ok() {
                total_writes += 1;
            }
        }
        tick += 1;
        // 1 Hz per writer — sleeps until the next second
        if tick % 60 == 0 {
            println!(
                "{:.3}  multi_pub tick={} writers={} total_writes={} loop_us={}",
                start.elapsed().as_secs_f64(),
                tick,
                count,
                total_writes,
                tick_start.elapsed().as_micros()
            );
        }
        let elapsed = tick_start.elapsed();
        if elapsed < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_secs(1) - elapsed);
        }
    }
    println!(
        "# multi_pub-done: writers={count} total_writes={} runtime={:.3}s",
        total_writes,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn run_sub_n(count: usize, runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());

    let mut readers = Vec::with_capacity(count);
    println!("[multi_sub] creating {count} readers");
    let create_start = Instant::now();
    for i in 0..count {
        let topic = participant.create_topic::<ShapeType>(&topic_name(i), TopicQos::default())?;
        let reader = subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;
        readers.push(reader);
    }
    println!(
        "[multi_sub] created {count} readers in {:.2}s",
        create_start.elapsed().as_secs_f64()
    );

    let start = Instant::now();
    let mut total_received = 0u64;
    let mut last_report = start;
    while start.elapsed() < runtime {
        for r in &readers {
            if let Ok(samples) = r.take() {
                total_received += samples.len() as u64;
            }
        }
        if last_report.elapsed() >= Duration::from_secs(60) {
            println!(
                "{:.3}  multi_sub readers={} total_received={}",
                start.elapsed().as_secs_f64(),
                count,
                total_received
            );
            last_report = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "# multi_sub-done: readers={count} total_received={} runtime={:.3}s",
        total_received,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  multi_endpoint_perf pub_n <count> <runtime_secs>");
    eprintln!("  multi_endpoint_perf sub_n <count> <runtime_secs>");
    std::process::exit(1);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        usage();
    }
    let mode = args[1].as_str();
    let count: usize = args[2].parse().unwrap_or_else(|_| usage());
    let secs: u64 = args[3].parse().unwrap_or_else(|_| usage());
    match mode {
        "pub_n" => run_pub_n(count, Duration::from_secs(secs)),
        "sub_n" => run_sub_n(count, Duration::from_secs(secs)),
        _ => usage(),
    }
}
