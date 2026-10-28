# The scripts behind the measurements the documents cite

These are transcripts of runs, not maintained tools. They are here
because the claims they produced are quoted elsewhere in this tree, and a
quoted measurement whose script lives only on the measurement host is a
claim nobody can check a second time. One such script was already lost
to a cleanup run — `/root/tc-env.sh`, which every instruction in this
repository named and no file in it contained.

They are kept as they ran, with their absolute paths and their rough
edges. Rewriting them into something tidier would produce a script that
has never been executed, which is a worse artefact than an untidy one
that has.

## `m1.sh`, `m1b.sh`, `m1c.sh` — what a narrow `@bit_bound` reaches

Run 2026-08-12 on the measurement host. `@bit_bound(8)` and
`@bit_bound(16)` on an enumeration, through each of the three IDL front
ends, reading the generated declaration and serializer:

| front end | result |
|---|---|
| `rtiddsgen` 4.7.0 | refuses at parse — *Only 32 bit bounds are accepted for enumerations* |
| `fastddsgen` 4.3.0 | accepts, emits `enum class Colour : int32_t` and `BitBound … = 32` for **both** bounds |
| `opendds_idl` | `error in lookup of symbol: bit_bound`; serializes through `ACE_CDR::Long` |

This is the measurement behind the decision that the four-octet holder is
**not** a serialisation choice: neither implementation carries the
declared bound as far as a serializer, so neither has a code path that
could write anything else. It settled that the ~150 rows involved are a
missing front-end feature rather than an axis — and, separately, that a
compatibility setting is still required, because the octets on the wire
differ whatever the cause. See `docs/wire-axes-followup.md`.

`m1c.sh` reads OpenDDS from `/opt/opendds`; `tools/interop-env.sh` uses
`/root/vendors/opendds`, which is the installation the probes build
against. Both existed on the host and the front-end answer was the same
from each.

## `final-record.sh` — the order the recording steps run in

The six sweeps in sequence with their exit codes, ending in a marker the
caller can wait on. `record.sh` takes `--vendors`, `--orbs`, `--ros`,
`--warnings` and `--read`; `regenerate.sh` rebuilds the pages from what
they wrote. The order matters: `--read` reads the payloads the write
sweeps recorded, so a read sweep run before them measures the previous
payloads and says nothing about the current ones. That has happened once
and left 24 cells claiming the write direction produced nothing where
octets were sitting in the record.
