# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Removed

- The feature `cross-vendor-tests` and `tests/cross_vendor.rs`: its three
  tests repeated the unit tests of `topic_mangling`, `service` and `action`,
  its fourth (`#[ignore]`) only checked that `ros2 topic list` exits.

### Changed

- The unit tests check against the ROS sources they name (rmw/types.h,
  rmw/ret_types.h, rmw/qos_profiles.h, REP-2003, REP-2004, the design
  articles on topic names and interface definitions, rcutils, RFC 8259 §7)
  and are named after them; round trips through the crate's own functions
  became checks against the literal values.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-ros2-rmw`** crate as a Layer-7 profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementation
ROS 2 RMW middleware-interface mapping (REP-2003/2004 + topic-name-mangling) for ZeroDDS bridge

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
All `pub` items are RC1-stable; breaking changes require a major bump.

### Added — Service / Action / Cross-Vendor

- Service layer (`service.rs`): REP-2008 request-reply over
  `zerodds-rpc` Topic-Naming.
- Action-Layer (`action.rs`): REP-2009 Goal/Feedback/Result-Pattern.
- IDL bridge (`msg_to_idl.rs`): ROS-2 `.msg`/`.srv` subset →
  IDL AST mapping for type hash (REP-2007).
- JSON log sink (`json_log.rs`) for structured rmw diagnostics.
- Cross-vendor interop module (`cross_vendor.rs`) for
  rclcpp/rclpy compatibility.
