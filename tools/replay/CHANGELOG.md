# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- `replay --inject` sends each sample under the representation identifier it
  was recorded with (DDSI-RTPS 2.5 §10.5): the writer offers that XCDR
  version and declares that extensibility and byte order
  (`DcpsRuntime::set_user_writer_representation_identifier`). It wrote every
  body under its own header (XCDR2, little endian, final), so an XCDR1,
  big-endian, `@appendable` or `@mutable` sample went out under a header that
  did not describe it. A capture without the identifier (`.zddsrec` version 2,
  NDJSON or SQLite from before) is not injected unless `--representation
  <hex>` names it. Dispose and unregister frames are not written as samples
  (counted on stderr); a topic a `TopicAdd` frame added is found. The writers
  are RELIABLE (DDS 1.4 §2.2.3.14 DataWriter default; they were best effort)
  and live in a `DcpsRuntime` instead of the C API.
- `replay --inject` waits until the readers of every writer acknowledged what
  it wrote, at most 5 s (DDS 1.4 §2.2.2.4.2.15), and fails naming the topics
  that were not; it shut its runtime down right after the last write, and a
  sample written before a reader matched the writer went unrepaired with it
  (`inject_under_the_recorded_identifier` received two of three samples on
  the Linux test host).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

* ZeroDDS Recorder 1.0 — `.zddsrec` file format definition.
* OMG DDSI-RTPS 2.5 §10 — sample serialisation form.

### CLI sub-commands

* `inspect <file>` — print recording header + per-topic frame count.
* `dump <file>` — list every frame (timestamp, topic, length).
* `replay <file> [--time-scale F] [--topic NAME]... [--inject [--inject-domain N]]` —
  replay frames at scaled wallclock, optionally re-inject into a
  live DCPS domain.

### Implementation

A pure-Rust binary built on `zerodds-recorder::RecordReader`. The
inspect and dump paths are read-only; the replay path optionally
opens a `zerodds-c-api` participant per
`--inject-domain` and caches a typed DataWriter per recorded topic.
Sample bytes are re-published verbatim — no re-serialisation, no
schema check — preserving byte-identical replay.

Topic filtering is set-membership against the `--topic` whitelist
(empty = all topics). Time-scale `F` scales the inter-frame
sleep: `F=1.0` matches the original wallclock cadence; `F=60.0`
plays back one hour of recording in one minute.

### Architecture

* Layer: Tools.
* Dependencies (in): `zerodds-recorder`, `zerodds-c-api`.
* Dependents (out): none.

### Stability

All public CLI flags are RC1-stable. Breaking changes require a
major version bump.
