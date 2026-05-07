//! Criterion-Benches für DDS-XML-Parser-Hot-Paths.
//!
//! Misst typische QoS-Profile-Parses + Tree-Building.
//! Regression-Detection.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_xml::parser::parse_xml_tree;
use zerodds_xml::qos_parser::parse_qos_libraries;
use zerodds_xml::zerodds_xml::parse_dds_xml;

const MINIMAL_QOS_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-XML">
  <qos_library name="Lib1">
    <qos_profile name="DefaultProfile">
      <datawriter_qos>
        <reliability>
          <kind>RELIABLE_RELIABILITY_QOS</kind>
        </reliability>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>"#;

const COMPLEX_QOS_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-XML">
  <qos_library name="ComplexLib">
    <qos_profile name="P1">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>10</depth></history>
        <resource_limits>
          <max_samples>1000</max_samples>
          <max_instances>100</max_instances>
          <max_samples_per_instance>10</max_samples_per_instance>
        </resource_limits>
        <deadline><period><sec>1</sec><nanosec>0</nanosec></period></deadline>
        <ownership><kind>SHARED_OWNERSHIP_QOS</kind></ownership>
      </datawriter_qos>
      <datareader_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
      </datareader_qos>
    </qos_profile>
    <qos_profile name="P2">
      <topic_qos>
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
      </topic_qos>
    </qos_profile>
  </qos_library>
</dds>"#;

fn bench_parse_xml_tree_minimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("xml_parse_tree_minimal");
    group.throughput(Throughput::Bytes(MINIMAL_QOS_PROFILE.len() as u64));
    group.bench_function("default_profile", |b| {
        b.iter(|| {
            let _ = parse_xml_tree(black_box(MINIMAL_QOS_PROFILE));
        });
    });
    group.finish();
}

fn bench_parse_xml_tree_complex(c: &mut Criterion) {
    let mut group = c.benchmark_group("xml_parse_tree_complex");
    group.throughput(Throughput::Bytes(COMPLEX_QOS_PROFILE.len() as u64));
    group.bench_function("complex_profile", |b| {
        b.iter(|| {
            let _ = parse_xml_tree(black_box(COMPLEX_QOS_PROFILE));
        });
    });
    group.finish();
}

fn bench_parse_dds_xml_complex(c: &mut Criterion) {
    c.bench_function("xml_parse_dds_xml_complex", |b| {
        b.iter(|| {
            let _ = parse_dds_xml(black_box(COMPLEX_QOS_PROFILE));
        });
    });
}

fn bench_parse_qos_libraries_complex(c: &mut Criterion) {
    c.bench_function("xml_parse_qos_libraries_complex", |b| {
        b.iter(|| {
            let _ = parse_qos_libraries(black_box(COMPLEX_QOS_PROFILE));
        });
    });
}

criterion_group!(
    benches,
    bench_parse_xml_tree_minimal,
    bench_parse_xml_tree_complex,
    bench_parse_dds_xml_complex,
    bench_parse_qos_libraries_complex,
);
criterion_main!(benches);
