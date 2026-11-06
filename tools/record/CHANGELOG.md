# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A run that records no sample leaves a capture that `info` and
  `zerodds-replay` read: the header (`RecordingSession::finish`). The file
  was empty.


### Fixed

- A BEST_EFFORT writer is recorded. The recorder read RELIABLE only, and
  by the RxO rule of RELIABILITY (DDS 1.4 §2.2.3.14) such a reader matches
  no BEST_EFFORT writer (OPEN-ITEMS 90). It attaches a RELIABLE and a
  BEST_EFFORT reader per topic and type (`zerodds_cli_common::
  monitor_reader_configs`); a sample of a RELIABLE writer, which both take,
  is recorded from the RELIABLE one (`WriterReliability::keep`).
- The capture (`.zddsrec`, `--out-json`, `--out-sqlite`) keeps the
  representation identifier of each sample's encapsulation header
  (`UserSample::Alive::representation_identifier`, DDSI-RTPS 2.5 §10.5).

