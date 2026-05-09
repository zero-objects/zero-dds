# zerodds-cli-common

Shared CLI primitives used by the ZeroDDS user-facing tools
(`zerodds-record`, `-bench`, `-monitor`, `-spy`, `-snitch`, `-pcap`,
`-mq`).

## What's in here

- `parse_duration("5" | "30s" | "2m" | "1h")` — duration arg parser.
- `install_signal_handler(stop)` — SIGINT/SIGTERM hook (Linux/macOS;
  no-op on Windows).
- `stable_prefix(marker_byte)` — process-stable RTPS `GuidPrefix`
  (PID + nanos + tool-marker).
- `participant_guid(prefix)` — 16-byte GUID with
  `ENTITYID_PARTICIPANT` (`0xC1`).
- `unix_ns_now()` — current UNIX time in nanoseconds (`i64`).
- `raw_reader_config(topic_name)` — `UserReaderConfig` for untyped
  `zerodds::RawBytes` topics (RELIABLE, VOLATILE, Shared ownership).

## Stability

These helpers are stable for the duration of the 1.0.x cycle. The
crate is marked `publish = true` only because the dependent CLI
crates require their dependencies to resolve on crates.io — it is
not intended as a general-purpose library outside the ZeroDDS tool
ecosystem.

## License

Apache-2.0
