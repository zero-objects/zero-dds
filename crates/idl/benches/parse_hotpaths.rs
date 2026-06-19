//! Criterion benches for IDL-parser hot paths.
//!
//! Measures:
//! * Top-level `zerodds_idl::parse` with small + complex specs
//! * Parsing real DDS builtin topic data types (ParticipantData,
//!   PublicationData, SubscriptionData) as a typical real-world size.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_idl::config::ParserConfig;

const SIMPLE_STRUCT: &str = "struct Point { long x; long y; };";

const NESTED_MODULES: &str = r#"
module com {
  module example {
    module dds {
      struct Sensor {
        long id;
        double value;
        string label;
      };
    };
  };
};
"#;

const REAL_WORLD_DDS_TYPES: &str = r#"
module zerodds {
  enum SensorKind { TEMP, PRESSURE, HUMIDITY };

  @final
  struct Position {
    @key long id;
    double x;
    double y;
    double z;
  };

  @appendable
  struct SensorReading {
    @key long sensor_id;
    SensorKind kind;
    Position location;
    double value;
    @optional string label;
    sequence<long, 32> tags;
  };

  union Result switch (long) {
    case 0: SensorReading ok;
    case 1: string error_message;
    default: long status_code;
  };

  exception SensorOutOfRange {
    long sensor_id;
    double observed;
    double max_allowed;
  };

  interface SensorService {
    Result read_sensor(in long id) raises (SensorOutOfRange);
    void calibrate(in long id, in double offset);
  };
};
"#;

fn bench_parse_simple_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("idl_parse_simple_struct");
    group.throughput(Throughput::Bytes(SIMPLE_STRUCT.len() as u64));
    let cfg = ParserConfig::default();
    group.bench_function("Point", |b| {
        b.iter(|| {
            let _ = zerodds_idl::parse(black_box(SIMPLE_STRUCT), &cfg);
        });
    });
    group.finish();
}

fn bench_parse_nested_modules(c: &mut Criterion) {
    let mut group = c.benchmark_group("idl_parse_nested_modules");
    group.throughput(Throughput::Bytes(NESTED_MODULES.len() as u64));
    let cfg = ParserConfig::default();
    group.bench_function("3_levels", |b| {
        b.iter(|| {
            let _ = zerodds_idl::parse(black_box(NESTED_MODULES), &cfg);
        });
    });
    group.finish();
}

fn bench_parse_real_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("idl_parse_real_world_dds");
    group.throughput(Throughput::Bytes(REAL_WORLD_DDS_TYPES.len() as u64));
    let cfg = ParserConfig::default();
    group.bench_function("zerodds_module", |b| {
        b.iter(|| {
            let _ = zerodds_idl::parse(black_box(REAL_WORLD_DDS_TYPES), &cfg);
        });
    });
    group.finish();
}

fn bench_parse_with_50_annotations(c: &mut Criterion) {
    // Realistic maximum (below the cap MAX_CONSECUTIVE_ANNOTATIONS=64).
    let mut src = String::new();
    for _ in 0..50 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let cfg = ParserConfig::default();
    c.bench_function("idl_parse_with_50_annotations", |b| {
        b.iter(|| {
            let _ = zerodds_idl::parse(black_box(&src), &cfg);
        });
    });
}

criterion_group!(
    benches,
    bench_parse_simple_struct,
    bench_parse_nested_modules,
    bench_parse_real_world,
    bench_parse_with_50_annotations,
);
criterion_main!(benches);
