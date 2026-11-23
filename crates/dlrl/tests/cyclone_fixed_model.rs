// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.4.5 against Cyclone DDS: a Cyclone participant acts at
//! DCPS level on a topic of an existing model (`tests/cyclone/station.idl`,
//! keyed by `SITE` and `NUM`, no class and no oid field), a DLRL Cache of
//! ZeroDDS acts at DLRL level on the same topic. What Cyclone writes
//! becomes an object; an object registered and written in DLRL is a
//! sample Cyclone takes.
//!
//! The Cyclone side is `tests/cyclone/station.c`, built by the test with
//! Cyclone's `idlc` and C library under `CYCLONE_HOME` (default
//! `/opt/cyclone` where it exists, else `/usr`). Opt-in like the other
//! Cyclone live tests: `--ignored` with `INTEROP_REQUIRE=1`; the weekly
//! job `vendor-live-dcps` runs it.

#![cfg(all(feature = "dcps", target_os = "linux"))]
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stderr,
    missing_docs
)]

#[path = "support/dds.rs"]
mod dds;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use dds::{mono, station_mapping, wait_until};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos, Entity};
use zerodds_dlrl::cache::{CacheBase, CacheDescription, CacheFactory, CacheUsage, ObjectHome};
use zerodds_dlrl::model::Value;
use zerodds_types::dynamic::DynamicSample;

fn cyclone_home() -> PathBuf {
    if let Ok(home) = std::env::var("CYCLONE_HOME") {
        return PathBuf::from(home);
    }
    if Path::new("/opt/cyclone/bin/idlc").exists() {
        return PathBuf::from("/opt/cyclone");
    }
    PathBuf::from("/usr")
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what}: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The directory `build_station` builds into, removed when the test
/// ends, also when it panics.
struct BuildDir(PathBuf);

impl BuildDir {
    fn station(&self) -> PathBuf {
        self.0.join("station")
    }
}

impl Drop for BuildDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("{} is left: {e}", self.0.display());
        }
    }
}

/// Builds `station` from `tests/cyclone` into a directory of its own.
fn build_station() -> BuildDir {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cyclone");
    let dir =
        BuildDir(std::env::temp_dir().join(format!("dlrl-cyclone-station-{}", std::process::id())));
    let out = &dir.0;
    std::fs::create_dir_all(out).unwrap();
    let home = cyclone_home();
    run(
        Command::new(home.join("bin/idlc"))
            .current_dir(out)
            .arg(src.join("station.idl")),
        "idlc",
    );
    let lib = home.join("lib");
    run(
        Command::new("cc")
            .current_dir(out)
            .arg("-o")
            .arg("station")
            .arg(src.join("station.c"))
            .arg("station.c")
            .arg("-I.")
            .arg(format!("-I{}", home.join("include").display()))
            .arg(format!("-L{}", lib.display()))
            .arg(format!("-Wl,-rpath,{}", lib.display()))
            .arg("-lddsc"),
        "cc",
    );
    dir
}

/// A child whose stdout lines arrive on a channel.
struct Station {
    child: Child,
    lines: mpsc::Receiver<String>,
}

impl Station {
    fn spawn(bin: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("station");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, lines }
    }

    fn expect_line(&self, wanted: &str, within: Duration) {
        let deadline = std::time::Instant::now() + within;
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match self.lines.recv_timeout(left) {
                Ok(l) if l == wanted => return,
                Ok(l) => eprintln!("station: {l}"),
                Err(_) => break,
            }
        }
        panic!("station did not print `{wanted}`");
    }
}

impl Drop for Station {
    fn drop(&mut self) {
        // The child ends by itself after its time; it is killed only when
        // the test fails early.
        if matches!(self.child.try_wait(), Ok(None)) {
            self.child.kill().ok();
        }
        self.child.wait().ok();
    }
}

#[test]
#[ignore = "needs Cyclone DDS (idlc, libddsc); INTEROP_REQUIRE=1, job vendor-live-dcps"]
fn dlrl_8_1_4_5_a_fixed_cyclone_model_is_read_and_written_as_objects() {
    if std::env::var("INTEROP_REQUIRE").as_deref() != Ok("1") {
        eprintln!("INTEROP-SKIP[cyclone_fixed_model]: opt in with INTEROP_REQUIRE=1");
        return;
    }
    // Declared first, dropped last: after the two Cyclone processes.
    let build = build_station();
    let station = build.station();
    // Cyclone discovers on the specification's SPDP group: the default
    // configuration, with fast discovery.
    let domain = zerodds_test_domains::unique_domain();
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .unwrap();
    let m = station_mapping();
    let cache = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadWrite,
            CacheDescription {
                name: format!("cyclone-station-{domain}"),
                domain: participant,
            },
        )
        .unwrap();
    let home = ObjectHome::new(Arc::clone(&m), "Station").unwrap();
    cache.register_home(&home).unwrap();
    cache.register_all_for_pubsub().unwrap();
    // §8.1.5.3: QoS between register and enable. The DataWriter offers
    // XCDR1, the representation a reader of Cyclone 0.10 takes for a
    // @final type.
    let dcps_writer = cache
        .the_publisher()
        .unwrap()
        .lookup_datawriter::<DynamicSample>("STATION")
        .unwrap();
    let mut qos = dcps_writer.get_qos();
    qos.data_representation = Some(vec![0]);
    dcps_writer.set_qos(qos).unwrap();
    cache.enable_all_for_pubsub().unwrap();
    let base = CacheBase::Cache(cache.clone());
    let d = domain.to_string();

    // Cyclone writes, DLRL reads an object.
    let writer = Station::spawn(&station, &["write", &d, "north", "7", "1.5"]);
    writer.expect_line("WROTE", Duration::from_secs(25));
    wait_until("Cyclone's station as object", || {
        home.get_objects(&base).unwrap().iter().any(|o| {
            o.get("site").ok() == Some(mono(Value::String("north".into())))
                && o.get("num").ok() == Some(mono(Value::Long(7)))
                && o.get("level").ok() == Some(mono(Value::Double(1.5)))
        })
    });
    drop(writer);

    // DLRL writes, Cyclone takes the sample.
    let reader = Station::spawn(&station, &["read", &d, "20"]);
    reader.expect_line("READY", Duration::from_secs(10));
    // The Cache's own DataReader and Cyclone's.
    wait_until("Cyclone's reader matched", || {
        dcps_writer.get_matched_subscriptions().unwrap().len() >= 2
    });
    let access = cache.create_access(CacheUsage::ReadWrite).unwrap();
    let south = home.create_unregistered_object(&access).unwrap();
    south
        .set("site", mono(Value::String("south".into())))
        .unwrap();
    south.set("num", mono(Value::Long(3))).unwrap();
    south.set("level", mono(Value::Double(0.5))).unwrap();
    home.register_object(&south).unwrap();
    access.write().unwrap();
    reader.expect_line("SAMPLE south 3 0.5", Duration::from_secs(15));
    drop(reader);
    CacheFactory::get_instance().delete_cache(&cache).unwrap();
}
