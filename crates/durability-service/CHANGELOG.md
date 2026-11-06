# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A replayed sample goes out under the representation identifier it came
  in (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60), from the pump and,
  after a restart, from the store. The service kept the XCDR version and
  the byte order only and replayed a `D_CDR2` or `PL_CDR2` body under
  `CDR2`, the header of an `@final` type (OPEN-ITEMS 60). A sample under an
  identifier of no CDR form is reported (`ReplayWriteFailed`) and not
  stored.
- The replay writer is registered with the first sample it replays and
  announces that sample's data representation first (DDS-XTypes 1.3
  §7.6.3.1.1: a reader matches when it accepts the writer's first offered
  representation). It announced XCDR first for samples that came under
  XCDR2, and a Cyclone DDS reader that accepts XCDR2 alone never matched it
  (`tests/cyclone_late_joiner.rs`, measured on the Linux test host).
- `serve` takes its samples through the runtime-level reader of
  `serve_typed` (with a TRANSIENT_LOCAL reader, as before), so it keeps the
  identifier too; it stored every sample as XCDR2 little endian.

### Changed

- The tests: by B3 of internal/release/rc8-test-suite.md. The late joiner
  after the writer, the restart and the auto-discovery go to the integration
  pillar's scenario `services::durability_outlives_writer_and_daemon`; the
  external Cyclone harness becomes `tests/cyclone_late_joiner.rs` (a Cyclone
  writer and late joiner of an `@appendable` type, weekly job
  `vendor-live-durability-cyclone`); `tests/encapsulation_replay.rs` holds
  the replay to the vendors' octets of the interop corpus.


### Changed

- The tests wait for events, not for pauses. `restart::persistent_survives_service_restart`
  lost its two pauses of 300 ms, asserts the match of the writer and names
  in a failure when each phase ended; `auto_discovery`,
  `cross_vendor_ingest`, `explicit_topic_foreign_type`,
  `restart_from_stored_meta` and `writer_dies` lost their pauses of 300 ms
  after a participant was dropped (its runtime is shut down when the drop
  returns) or `serve_if_stored` returned (it primes the replay writer
  before); `keyed_topic_end_to_end` waits until `cleanup` purges the
  disposed instance instead of 300 ms for the dispose to arrive.
- The tests' sqlite files lie on `/dev/shm` where it exists. In the merge
  lane on the CI runner host the restart test took 8.8 s, 12.8 s and 24.1 s and was once
  stopped at 46.5 s (jobs 227834, 227784, 227979, 227936; the process ended
  21.5 s after nextest began to terminate it), while the Linux test host took
  1.2 to 1.8 s, also under a synchronous random-write load on its disk
  (`fio --fsync=1`).
