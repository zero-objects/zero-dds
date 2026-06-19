# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

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
