# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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
