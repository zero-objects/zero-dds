# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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
