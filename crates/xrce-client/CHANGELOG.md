# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Added
- `XrceClient::set_representation_timestamps` (DDS-XRCE 1.0 §7.8.2.1): the form of the `CLIENT_Representation` it sends and the `AGENT_Representation` it reads, with or without the timestamp member; default without (Annex A).
- `STATUS_AGENT`, `STATUS` and `INFO` are read in the form of either setting of `zerodds_xrce::encoding::UnannotatedTypes` (DHEADER or none).
- `XrceClient::send_timestamp` and `send_timestamp_reply` (DDS-XRCE 1.0 §8.3.5.15, §8.3.5.16) on `STREAMID_NONE`; `ClientEvent::TimestampReceived` for a `TIMESTAMP` of the agent and `ClientEvent::TimestampReply` for the answer to one the client sent. Neither side sent a `TIMESTAMP` before.
- `XrceClient::update_object`: the update of an object as a CREATE with `reuse` and `replace` (DDS-XRCE 1.0 §8.4.13, Table 14).
- `discovery` (DDS-XRCE 1.0 §8.4.4, §11.2.4, §11.3.4): `AgentDiscovery` sends `GET_INFO(OBJECTID_AGENT)` to a multicast address and port or to a list of candidate addresses every period and stops at the first suitable `INFO` (`STATUS_OK`, the major version of the protocol; `STATUS_ERR_INCOMPATIBLE` and other versions are passed over), optionally with the `client_info` of Figure 10; `discover_over_udp` runs it on a socket and returns the transport addresses of the agent; `connect_over_tcp` attempts connections to a list of TCP addresses every period, sends the `CREATE_CLIENT` and returns the first client the agent accepts; `TcpTransport` frames messages with the 2-octet length of §11.3.3. The client found no agent by itself before.
- `ClientEvent::Sample` (a `DATA` in FORMAT_SAMPLE, with its `SampleInfo`) and `ClientEvent::SampleSequence` (FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ, FORMAT_PACKED_SAMPLES: count, `info_base` and the elements as they are on the wire; the type of the topic tells where each sample ends). They were dropped without a word, FORMAT_SAMPLE reached the application as `Data` without its `SampleInfo` (DDS-XRCE 1.0 §8.3.5.10.2).
- `XrceClient::set_delivery_control_form`: `READ_DATA` writes the `DataDeliveryControl` with the DHEADER the spec requires (default) or without it, as the eProsima Agent reads it.
- `XrceClient::request_get_info` and `request_agent_info` (GET_INFO, also before the session, DDS-XRCE 1.0 §7.8.2.2, §8.3.5.3) and `ClientEvent::Info`/`InfoUnreadable` for the INFO answer (§8.3.5.7). A `STATUS_AGENT` is read with the position of the `AGENT_Representation` in the payload.

### Fixed
- A reassembled FRAGMENT run that holds no submessage is reported as `ClientEvent::FragmentDiscarded` (DDS-XRCE 1.0 §8.3.5.14).
- A `STATUS_AGENT` with the E flag clear is read big-endian (DDS-XRCE 1.0 §8.3.4.2).
- `XrceClient::connect` sends the CREATE_CLIENT in a header with `SESSION_ID_NONE_WITH_CLIENT_KEY` (session_id 0 to 127) or `SESSION_ID_NONE_WITHOUT_CLIENT_KEY` (128 to 255), not with the session it asks for (DDS-XRCE 1.0 §8.3.2.1).
- A message on a best-effort stream that is a duplicate of the last one or older is discarded and reported as `ClientEvent::BestEffortMessageDiscarded` (§8.3.2.3).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-xrce-client`** crate as a Layer-7 profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementation
XRCE client for Micro profile, transport-agnostic

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
All `pub` items are RC1-stable; breaking changes require a major bump.
