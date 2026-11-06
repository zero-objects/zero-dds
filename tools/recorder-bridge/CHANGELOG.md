# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- A BEST_EFFORT writer is recorded. The recorder read RELIABLE only, and
  by the RxO rule of RELIABILITY (DDS 1.4 §2.2.3.14) such a reader matches
  no BEST_EFFORT writer (OPEN-ITEMS 90). It attaches a RELIABLE and a
  BEST_EFFORT reader per topic and type (`zerodds_cli_common::
  monitor_reader_configs`); a sample of a RELIABLE writer, which both take,
  is recorded from the RELIABLE one (`WriterReliability::keep`).
- Records a dispose with unregister of one instance (DDSI-RTPS 2.5
  §9.6.4.9, the D and U bits of a status info) as two entries,
  `NotAliveDisposed` and then `NotAliveUnregistered`; it recorded the
  unregister alone and the dispose was lost (OPEN-ITEMS 46).

- A topic that keeps sending holds up neither the other readers nor the
  stop: each reader gets one wait per turn and gives at most 1024 queued
  samples. The loop received from one reader until a wait ran out.

- Records each sample with the representation identifier of its
  encapsulation header (DDSI-RTPS 2.5 §10.5): the readers live in a
  `DcpsRuntime` (they were C-API readers, whose take gave the body only),
  and the capture is `.zddsrec` version 3. A reader that cannot be created
  ends the run with the runtime shut down; the C-API path destroyed the
  runtime with the readers already made still registered. SIGTERM stops a
  run as SIGINT does.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

* OMG DDS-DCPS 1.4 §2.2 — DataReader API used to receive samples.
* ZeroDDS Recorder 1.0 — `.zddsrec` file format.

### CLI

* `--output <path>` — destination `.zddsrec` file.
* `--domain <id>` — DDS domain to subscribe on (default 0).
* `--topic <name>:<type>` — topic-name + type-name pair (repeatable).
* `--duration <Ns>` — wallclock cap; omit for unbounded recording.
* SIGINT — clean shutdown, file closed and flushed.

### Implementation

For each configured topic the bridge creates a `zerodds-c-api`
DataReader against the running participant, polls `take()` in a
tight loop, and forwards every sample to a single
`zerodds_recorder::RecordingSession`. The session writes a
length-prefixed framed format with per-frame
`(timestamp, topic_id, bytes)` tuples. Topic-ID lookup is
in-memory (one `ParticipantEntry` per topic).

A single participant per process keeps memory and discovery cost
bounded; multiple bridges can be run side by side for capture
across multiple domains.

### Architecture

* Layer: Tools.
* Dependencies (in): `zerodds-recorder`, `zerodds-c-api`.

### Stability

CLI is RC1-stable. Breaking changes require a major version bump.
