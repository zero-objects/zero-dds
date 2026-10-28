# The targeted inventory probes

Questions from [`docs/wire-feature-inventory.md`](../../docs/wire-feature-inventory.md)
that no corpus case can express, each needing an entry point or a
payload the byte gate cannot produce.

```sh
tools/inventory-probes/run.sh                 the first three
tools/inventory-probes/run.sh keyholder
tools/inventory-probes/parameter-edges/run.sh
tools/inventory-probes/bound-overrun/run.sh
tools/inventory-probes/encapsulation-be/run.sh
tools/inventory-probes/longdouble-xcdr2/run.sh
tools/inventory-probes/key-map/run.sh
```

| group | rows | why not a case |
|---|---|---|
| `keyholder` | N3, N7 | the key **holder** is not a payload. It reads Cyclone's `dds_stream_extract_keyBE_from_key`, RTI's `PRESTypePlugin_interpretedSerializeKeyForKeyhash` under all five encapsulations, and Fast DDS's generated `compute_key` — three entry points no `--emit payload` reaches |
| `bool-read` | C2, D1 | the payload is one no writer will produce. A boolean octet of `0x02` has to be handed to each vendor's own deserialiser, and there is no writer that would put it there |
| `key-union` | N11 | three of the four front ends refuse the file, so there is nothing to serialise and the refusal is the result |
| `parameter-edges` | I9, I10, I14, I17, L4 | the same reason as `bool-read`, five more times over: a vendor-private parameter, a header with `FLAG_3` or `FLAG_4` set, a reserved parameter id, `PID_IGNORE`, and a length above 4 GiB are all octets no conforming writer produces. Each is built once **per vendor framing** and handed to that vendor's own deserialiser |
| `bound-overrun` | D6, G16 | a bound that is respected costs no octet, so the only measurement is what happens when it is not — and `wireprobe-<vendor>` refuses an over-long value before any vendor sees it. The probe sets the member after the type support is generated |
| `encapsulation-be` | B1 | `D_CDR2_BE` needs a serialiser whose byte order is not its host's. OpenDDS's `Encoding` carries the endianness as a member, so the big-endian half of every identifier pair is reachable on x86-64 |
| `longdouble-xcdr2` | C7 | `wireprobe-fill` refuses a `long double` by name, because its *value* is the C++ compiler's and not the clause's. The layout is a different question and this asks only that |
| `key-map` | N10 | as `key-union` — except that the one front end which accepts the file is the measurement there, not a message |

What they found, in one line each: the key holder orders by ascending
member id everywhere except RTI under XCDR1; the MD5 branch starts at
the type's *maximum* serialized key size and all four agree on
`4321f7288e521aa62aee2745f3f8d92b` for the twenty-octet holder; Cyclone
and RTI hand back a boolean octet of `0x02` unchanged and Fast DDS
refuses it; and `union MyCommand switch (@key CommandKind)` —
§7.2.2.4.4.4.8's own example — is refused by name by Cyclone and RTI.

What the five newer groups found, in one line each: RTI reads a
vendor-private parameter as the user member with the same low id, and
Fast DDS and OpenDDS do too in the short header; Fast DDS and OpenDDS
**drop a member in silence** when its extended header sets a bit
§7.4.1.2.1 only asks them to zero; OpenDDS **writes** a reserved
parameter id, `04ff0400`, which nothing was supposed to do; OpenDDS is
the one implementation of four that honours `PID_IGNORE` over the
must-understand bit, which is what the clause says; every implementation
refuses a sequence length above 4 GiB; Fast DDS truncates a bounded
string in silence and enforces no sequence bound, where the other three
refuse in three different layers; OpenDDS derives `D_CDR2_BE = 0x0008`
on an x86-64 host; Fast DDS and OpenDDS put a `long double` at payload
offset 4 under XCDR2 and 8 under XCDR1, sixteen octets each; and
`fastddsgen` is the only front end that accepts `@key map<K,V>` — and it
keys on the map.

The generated type support is not in the tree; `idlc`, `rtiddsgen`,
`fastddsgen`, `opendds_idl` and `tao_idl` write it from the IDL beside
each probe on every run.

Was `/root/n3-keyholder`, `/root/c2-boolread` and `/root/n11-keyunion`.
