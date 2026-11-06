# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- The NDJSON and SQLite sinks keep the representation identifier of each
  sample's encapsulation header next to `raw_cdr` (`representation_identifier`,
  hex in NDJSON, a BLOB column in SQLite, added to a table of an earlier
  recording); `RecordedSample::representation_identifier` reads it back
  (`None` for a capture made before). `JsonSink::write_sample`,
  `sample_line` and `SqliteSink::insert_sample` take the body and the
  identifier together as a `RawSample` in place of `raw_cdr`.

