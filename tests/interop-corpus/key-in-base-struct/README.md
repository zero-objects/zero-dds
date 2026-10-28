# key-in-base-struct

**Construct** — a key member in the base, inherited by the derived type

XTypes 1.3 §7.2.2.4.5 gives a derived structure no key fields of its own: its keys are its root base's. §7.4.1.1.5.1 puts a base's members first and §7.6.8 builds the KeyHash from the key members, so the derived type's KeyHash is the base's key, and an implementation that nests the base as a member rather than flattening it still has to find the key inside it.

The case used to key `slot` in `Sample` as well. Every foreign system refused that form — Cyclone DDS 11.0.1 "Derived struct is not allowed to expand on keys", Fast DDS "@key cannot be used in a derived structure", RTI, OpenDDS, OpenSplice and HDDS without bytes (`../../interop-results/REFUSALS.md` in the history of this row) — and ZeroDDS refuses it now too (`crates/idl-lower/tests/keys_and_inheritance.rs`).

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `7929edff07000000feff0000182d4454fb210940` |
| `--encoding xcdr2 --endian be` | 20 bytes — `ffed297900000007fffe0000400921fb54442d18` |
| `--encoding xcdr1 --endian le` | 24 bytes — `7929edff07000000feff000000000000182d4454fb210940` |
| `--encoding xcdr1 --endian be` | 24 bytes — `ffed297900000007fffe000000000000400921fb54442d18` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
