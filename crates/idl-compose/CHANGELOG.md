# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A registration of type objects that stops at a type (`register_all` returns an
  error other than `Unrepresentable` or `Recursive`, e.g. a union case label
  outside the `long` of DDS-XTypes 1.3 §7.3.4.5.3.2) no longer leaves
  `Composed::without_type_objects` empty. Every type without a COMPLETE object
  in the registry is listed with the reason: its own error, or the error the
  registration stopped at. Before, the registry held the types up to the error
  and nothing said that the others were missing.
