# zerodds-recorder-decode

Decode DDS samples to typed JSON + SQLite for `zerodds-spy` / `zerodds-record`,
and read them back for `zerodds-replay`. Tooling support crate (safety class
**COMFORT** — no runtime hot path).

Sits on top of `zerodds-wire-codec` (`decode` / `encode`), which walks the
type model in both directions; a `DynamicType` is a view into the `Ast` that
`zerodds-idl-parse` and `zerodds-idl-lower` produce.

## Modules

- **`type_source`** — load an out-of-band IDL file (`--type-file`) and resolve a
  named type to a `DynamicType`; `parse_topic_type_map` for `topic=Type` entries.
- **`wire`** — which encoding a received sample is in, from the type and the
  XCDR version the reader was told.
- **`json_sink`** — `DynamicData` → JSON; an NDJSON sample sink. Each line is
  self-describing (`topic`, `type`, `recv_ts_ns`, `writer`, decoded `value`, and
  the raw CDR bytes as hex for byte-exact replay). Integers outside
  `i64`/`u64`, `fixed<P,S>` and `long double` are written as exact decimal
  strings; `NaN` and `±inf` become `null`.
- **`sqlite_sink`** — per-topic relational SQLite. One row per sample keyed by
  `sample_id` (the record marker) with typed scalar columns (1:1 nested structs
  and bitsets flattened to `field__sub`); child tables `t_<topic>__<member>`
  joined by `sample_id` for sequences/arrays of scalars or structs, for maps
  with scalar key and value, and for bitmasks (one row per set bit); `_types`
  holds the full IDL per topic. A `uint64` above `2^63`, `fixed<P,S>` and
  `long double` are stored as exact decimal `TEXT`.
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

The SQLite sink has no flat row shape for some members and skips them: a
sequence or array whose element is a sequence, array, map, bitmask or union; a
map whose key or value is not scalar; a union member. Each skipped member is
named in `SqliteSink::skipped_members()`; its bytes stay in `raw_cdr`, so
replay from SQLite is unaffected.

## License

Apache-2.0.
