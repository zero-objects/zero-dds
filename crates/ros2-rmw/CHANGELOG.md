# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung des Crates **`zerodds-ros2-rmw`** als Layer-7-Profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementierung
ROS 2 RMW middleware-interface mapping (REP-2003/2004 + topic-name-mangling) for ZeroDDS bridge

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.

### Added — Service / Action / Cross-Vendor

- Service-Layer (`service.rs`): REP-2008 Request-Reply ueber
  `zerodds-rpc` Topic-Naming.
- Action-Layer (`action.rs`): REP-2009 Goal/Feedback/Result-Pattern.
- IDL-Bridge (`msg_to_idl.rs`): ROS-2 `.msg`/`.srv`-Subset →
  IDL-AST-Mapping fuer Type-Hash (REP-2007).
- JSON-Log-Sink (`json_log.rs`) fuer strukturierte rmw-Diagnose.
- Cross-Vendor-Interop-Modul (`cross_vendor.rs`) fuer
  rclcpp/rclpy-Kompatibilitaet.
