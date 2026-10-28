//! Criterion benches for the IDL frontend's hot paths.
//!
//! Successor of `crates/idl/benches/parse_hotpaths.rs`, which was removed
//! together with `zerodds-idl` in d045e43ea86. The parser now builds a
//! lossless CST in `zerodds-idl-parse`; this bench measures that, at the
//! same three sizes the removed one used.
//!
//! Measured:
//! * a one-line struct — the fixed cost of a parse
//! * nested modules — scope handling
//! * a real DDS type set (enum, structs, union, exception, typedefs,
//!   annotations) — the size an application's IDL actually has
//!
//! `zerodds-idl-lower` is deliberately not benched here: it takes the
//! `SyntaxNode` this crate produces, so it belongs to a bench of its own
//! rather than inside the parser's numbers.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_idl_parse::{MemoryLoader, parse, parse_preprocessed, preprocess};

const SIMPLE_STRUCT: &str = "struct Point { long x; long y; };";

const NESTED_MODULES: &str = r"
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
";

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

  typedef sequence<SensorReading, 128> ReadingBatch;
  typedef double Transform[16];

  @mutable
  struct Telemetry {
    @key @id(1) string<64> source;
    @id(2) ReadingBatch readings;
    @id(3) Transform pose;
    @id(4) @external Position origin;
  };
};
"#;

fn bench_parse(c: &mut Criterion) {
    let cases: [(&str, &str); 3] = [
        ("simple_struct", SIMPLE_STRUCT),
        ("nested_modules", NESTED_MODULES),
        ("real_world_dds_types", REAL_WORLD_DDS_TYPES),
    ];

    let mut group = c.benchmark_group("idl_parse");
    for (name, source) in cases {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| {
                let parse = parse(black_box(source));
                black_box(parse.has_errors())
            });
        });
    }
    group.finish();
}

/// The path a real compile takes: preprocess, then parse the tokens the
/// preprocessor produced. `parse` alone skips the directive scan, so the
/// two together are what an `idlc` invocation actually pays.
fn bench_frontend(c: &mut Criterion) {
    let loader = MemoryLoader::new();

    let mut group = c.benchmark_group("idl_frontend");
    group.throughput(Throughput::Bytes(REAL_WORLD_DDS_TYPES.len() as u64));
    group.bench_function("preprocess", |b| {
        b.iter(|| {
            let pre = preprocess("bench.idl", black_box(REAL_WORLD_DDS_TYPES), &loader);
            black_box(pre.tokens.len())
        });
    });
    group.bench_function("preprocess_and_parse", |b| {
        b.iter(|| {
            let pre = preprocess("bench.idl", black_box(REAL_WORLD_DDS_TYPES), &loader);
            let parse = parse_preprocessed(&pre);
            black_box(parse.has_errors())
        });
    });
    group.finish();
}

criterion_group!(benches, bench_parse, bench_frontend);
criterion_main!(benches);
