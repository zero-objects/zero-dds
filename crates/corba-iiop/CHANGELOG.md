# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-iiop`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 2**: §14 (IIOP-Overview), §15.7 (IIOP-Profile +
  ProfileBody), §15.9 (Bidirectional-GIOP).

### Public-API

- `bidir::{BiDirIiopListenPoint, BiDirIiopServiceContext,
  IIOP_BI_DIR_TAG}` — Bidirectional-GIOP-Aushandlung (§15.9).
- `profile_body::{IiopProfileBody, IiopVersion, TaggedComponent}` —
  ProfileBody fuer alle 4 IIOP-Versionen (1.0/1.1/1.2/1.3) inkl.
  Components-Sequenz.
- `acceptor::{Acceptor, AcceptorConfig}` (Feature `std`) — TCP-
  Listener-Loop mit Per-Connection-Worker-Thread.
- `connection::Connection` — TCP-Stream-Wrapper mit Frame-Reader
  (12-Byte-GIOP-Header → `message_size` → Body-Read).
- `connector::{Connector, ConnectorConfig}` — Client-Connect mit
  Connection-Reuse + thread-safe Pool + Connect-Timeout +
  Reconnect-Logik bei `CloseConnection`.
- `error::IiopError` — Error-Surface.
- `framing::{read_giop_message, write_giop_message}` — Frame-Codec
  ueber `corba-giop::Message`.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`IiopVersion`-Konstanten `V1_0`/`V1_1`/`V1_2`/`V1_3`.
`IiopProfileBody`-Encoder/Decoder respektiert die Versions-Quirks:
1.0 ohne Components-Sequenz, ab 1.1 mit `sequence<TaggedComponent>`.

`Connection` liest GIOP-Frames byte-genau ein und delegiert an
`corba-giop::decode_message`. `Acceptor` und `Connector` arbeiten
auf Standard-`std::net::TcpStream`.

`BiDirIiopServiceContext` (§15.9) erlaubt einem Server Requests an
einen Client-Endpoint zu schicken, nachdem dieser ein
`ListenPoint`-Set per ServiceContext bekanntgegeben hat.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-B).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-giop`.
- **Dependents (out):** `zerodds-corba-ior` (TaggedProfile-Inhalt),
  `zerodds-corba-dds-bridge` (Forwarder).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format durch OMG fixiert.
