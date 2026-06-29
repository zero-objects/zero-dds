# zerodds-recorder-decode

Decode DDS samples to typed JSON + SQLite for `zerodds-spy` / `zerodds-record`,
and read them back for `zerodds-replay`. Tooling support crate (safety class
**COMFORT** — no runtime hot path).

Sits on top of the reflective XTypes codec
(`zerodds_types::dynamic::codec::{decode_dynamic, encode_dynamic}`).

## Modules

- **`type_source`** — load an out-of-band IDL file (`--type-file`) and resolve a
  named type to a `DynamicType`; `parse_topic_type_map` for `topic=Type` entries.
- **`json_sink`** — `DynamicData` → JSON; an NDJSON sample sink. Each line is
  self-describing (`topic`, `type`, `recv_ts_ns`, `writer`, decoded `value`, and
  the raw CDR bytes as hex for byte-exact replay).
- **`sqlite_sink`** — per-topic relational SQLite. One row per sample keyed by
  `sample_id` (the record marker) with typed scalar columns (1:1 nested structs
  flattened to `field__sub`); child tables `t_<topic>__<member>` for scalar
  sequences/arrays joined by `sample_id`; `_types` holds the full IDL per topic.
  `helper_queries` emits ready-to-run SQL ("record by property", "nth record",
  "full record by join").
- **`replay_source`** — read the decoded formats back into a format-neutral
  `RecordedSample` (`topic`, `type_name`, `recv_ts_ns`, `raw_cdr`):
  `from_ndjson` (NDJSON) and `from_sqlite` (read-only, globally ts-ordered).

## Replay fidelity

Every sink retains the raw CDR bytes per sample, so `zerodds-replay` re-publishes
byte-exact from `.zddsrec`, NDJSON, **and** SQLite. The `decode_roundtrip` e2e
test asserts both byte-exact `raw_cdr` and value-exact re-decode across
XCDR1/XCDR2 × little/big-endian.

## Residual

Composite-element collections (`sequence<Struct>` etc.), alias-to-composite,
mutable-union, wstring encode and bitmask/bitset are reported as `NotSupported`
(honest error, never silent corruption) — gated on the collection
`element_type` model refactor. Scalar-element collections are fully supported.

## License

Apache-2.0.
