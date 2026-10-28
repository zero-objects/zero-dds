# sequence-bounded-overrun

**Construct** — a `sequence<long, 2>` with a member behind it, the shape a bound overrun is visible in

The sequence twin of `string-bounded-overrun`, and the four implementations answer it in three different layers. RTI's answer comes from the container: `DDS_LongSeq_set_length(&v, 3)` returns false — *"available space 2 < 3"* — and raising the maximum is refused too — *"failed to assert new max cannot be larger than absolute maximum"* — so the over-long value cannot be built. OpenDDS's comes from the type: `bounded_value_sequence<Long, 2>::length(3)` raises `CORBA::BAD_PARAM`. Cyclone's comes from the serialiser: the C mapping's `_length` will hold 3 and `dds_stream_write_sample` then refuses the sample. Fast DDS enforces nothing anywhere — `sequence<long, 2>` is a plain `std::vector<int32_t>` and three elements go out.

`tools/inventory-probes/bound-overrun/` is the run; `tail` is what a third element moves.

`rosidl_typesupport_fastrtps_c` is a fifth answer and the one already recorded in G16: it generates an explicit check, `if (size > N) { … return false; }`, for a bounded sequence — while generating none for a bounded string.

There is no `-unannotated` twin: the question is the bound and not the extensibility.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0200000007000000090000002a000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000200000007000000090000002a` |
| `--encoding xcdr1 --endian le` | 16 bytes — `0200000007000000090000002a000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `0000000200000007000000090000002a` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
