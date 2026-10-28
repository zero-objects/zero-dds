# The wire oracle

The second byte-gate case set, and the two ORB one-offs beside it.

```sh
tools/wire-oracle/gate.sh --cases map_val_struct
tools/wire-oracle/gate.sh --vendors opendds --out oracle.tsv
tools/wire-oracle/fixed-orbs.sh
```

`gate.sh` is a thin wrapper: it runs `tools/byte-gate/gate.sh` over
`cases/` here instead of over the first set. Two case directories and
one driver, so a case added for OpenDDS or an ORB cannot disturb a run
of the set the layout rules were read off.

## The seventeen cases

The constructs the first three vendors cannot all be asked about.

- **`map`** — seven cases. Both C emitters refuse the keyword, so the
  whole family has two witnesses: Fast DDS in C++ and OpenDDS. They
  disagree, and the disagreement is §7.4.3.5.3 contradicting itself:
  Fast DDS implements rule (12) and OpenDDS implements rule (15).
- **`bitset`** — three cases. Of five IDL front ends only `fastddsgen`
  parses the keyword at all, so anchor width, bit order and where an
  inherited bitfield sits have exactly one witness each.
- **`fixed<P,S>`** — one case. No DDS specification has a fixed-point
  type; see `fixed-orbs.sh`.
- **the length-code cases** — six, `mut_lc_*` and `mut_lengthcodes`,
  which needed a fourth implementation before the short codes could be
  read as settled.

Eight of the seventeen carry a `fill.odds.inc` and are the whole of
OpenDDS's column. A case without one is recorded as `unfilled`: the
Fast DDS fill calls setters OpenDDS's C++ mapping does not have, and
borrowing it would blame OpenDDS for this harness's gap.

## `omni/` and `jacorb/`

`fixed<P,S>` through two CORBA ORBs. One struct with nine `fixed`
typedefs — varying precision, scale, sign and size, including
`fixed<31,0>`, the widest CORBA allows — with an `octet` in front and
another behind so the padding on both sides is visible. No case
directory: every member after a mis-marshalled `fixed` would be
unreadable, so a per-member case could not say which member went wrong.

`omni/r.idl` and `omni/b.idl` are the two refusals — a `fixed<31,31>`
omniidl rejects and the `bitset` no CORBA front end parses. A refusal is
a measurement of a front end, and re-deriving it needs the file that
provoked it.

JacORB's `*Helper.java` and `*Holder.java` are not in the tree;
`org.jacorb.idl.parser` writes them from `gen.idl` on every run.

Was `/root/wire-oracle/` on the Linux test host.
