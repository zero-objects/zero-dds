# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A run that records no sample leaves a capture that `info` and
  `zerodds-replay` read: the header (`RecordingSession::finish`). The file
  was empty.
- A sample of a RELIABLE writer that only the BEST_EFFORT reader took is
  recorded. It is one written after the writer matched the BEST_EFFORT
  reader and before it matched the RELIABLE one, which as a VOLATILE reader
  never gets it (DDS 1.4 §2.2.3.4); `WriterReliability::keep` dropped it as
  the RELIABLE reader's. It now keeps the first copy of each sample by its
  writer GUID and sequence number (`zerodds_cli_common::SourceCopies`,
  DDSI-RTPS 2.5 §8.3.5.4), from either reader; `zerodds-spy` and
  `zerodds-recorder-bridge` likewise. Measured with the example
  `decoded-recorder` of zero-dds-snippets on the Linux test host: before,
  5 of 5 runs lost the first of 20 samples in the spy or the recorder;
  after, 5 of 5 runs held 20.


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

