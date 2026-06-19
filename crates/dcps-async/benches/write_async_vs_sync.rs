// The bench setup may use expect() (no no-panic contract) and
// does not require missing_docs on helper items.
#![allow(clippy::expect_used, missing_docs)]

//! Bench: async vs sync write latency.
//!
//! Spec §9.1 performance target: write().await ≤ 5 % overhead versus
//! sync write. This bench measures both variants in an
//! offline participant (no UDP, so pure API overhead).
//!
//! Run: `cargo bench -p zerodds-dcps-async --bench write_async_vs_sync`

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_dcps_async::AsyncDomainParticipantFactory;

fn bench_sync_write(c: &mut Criterion) {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(99, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("BenchSync", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");

    c.bench_function("sync_write_64_bytes", |b| {
        let payload = RawBytes::new(vec![0xAA; 64]);
        b.iter(|| {
            writer.write(black_box(&payload)).expect("write");
        });
    });
}

fn bench_async_write(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");

    let factory = AsyncDomainParticipantFactory::instance();
    let p = factory.create_participant_offline(99);
    let topic = p
        .create_topic::<RawBytes>("BenchAsync", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");

    c.bench_function("async_write_64_bytes", |b| {
        let payload = RawBytes::new(vec![0xAA; 64]);
        b.iter(|| {
            rt.block_on(async { writer.write(black_box(&payload)).await.expect("write") });
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(3));
    targets = bench_sync_write, bench_async_write
}
criterion_main!(benches);
