# primitive-longdouble

**Construct** — primitive `long double`

`long double` is IEEE 754 binary128 on the wire: one sign bit, a
fifteen-bit exponent biased by 16383 and a 112-bit stored fraction, in
sixteen bytes aligned to eight. CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2
gives the layout; DDS-XTypes 1.3 Table 31 gives the size and the
alignment, and `MAXALIGN` caps that alignment at four under XCDR2.
Figure 9.2 also settles the byte order: the two orders are the whole
sixteen bytes reversed, not two eight-byte halves swapped in place.

This case used to be marked `placeholder`, because all three ZeroDDS
columns wrote a `double` followed by eight zero bytes — the right length,
the wrong value, and no error. It is now measured in all three:
`crates/float128`, `runtime/c/src/float128.c` and
`runtime/python/zerodds_wire/float128.py` are three independent
implementations of the same clause, and the suite compares them.

## The six values, and what each one catches

| member | value | what it separates |
|---|---|---|
| `fits_a_double` | `-2.75` | nothing on its own — the control. Every candidate format holds it. |
| `needs_sixty_four_bits` | `1 + 2^-60` | a column that narrows through a `double`. 61 significand bits: a binary64 holds 53, x87 extended holds 64. |
| `needs_all_of_them` | `1 + 2^-112` | a column that narrows through **x87 extended**. 113 significand bits, which only binary128 has. |
| `smallest_normal` | `2^-16382` | the exponent boundary: biased exponent 1, an empty fraction. |
| `largest_subnormal` | `(2^112 - 1) * 2^-16494` | the other side of it: biased exponent 0, a full fraction. The two are adjacent and a wrong implied-one rule swaps them. |
| `largest_finite` | `(2 - 2^-112) * 2^16383` | the top of the range, and the value an overflow rounds to an infinity from. |

The values are written in the shortest decimal that reads back as the
same pattern, which is what a human can check; the canonical form the
columns are compared in is the *exact* expansion, and
`crates/wire-probe/src/decode.rs` says why.

## What the vendors do, measured

None of the four settles the representation, which is why the clause
does. `docs/float128-column.md` §2 has the runs; in short:

| vendor | what happens |
|---|---|
| Cyclone DDS 11.0.1 | `idlc` refuses: "Long doubles are currently unsupported". No generated type exists. |
| RTI Connext 7.7 | `DDS_LongDouble` is `struct { char bytes[16]; }` on Linux. RTI transports the sixteen octets, byte-reversed as a whole for a big-endian stream, and names no representation for them. |
| Fast DDS / FastCDR | maps to the compiler's `long double` and copies its sixteen bytes — x87 80-bit extended on x86-64, **including six bytes of uninitialised padding**, and binary128 on aarch64. |
| OpenDDS 3.34 / ACE 6.5.24 | the same copy, with a correct whole-sixteen-byte swap for big endian. |

So the three vendor emitters refuse this case rather than fill it. For
Cyclone there is nothing to fill; for RTI a literal would make this
harness choose a wire format on a vendor's behalf; for Fast DDS a literal
would record one architecture's bytes as Fast DDS's. The refusals are in
`C_REFUSALS` and `CPP_REFUSALS` in
`crates/wire-probe/tests/interop_corpus.rs`.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 96 bytes — `000000000000000000000000006000c00000000000001000000000000000ff3f0100000000000000000000000000ff3f00000000000000000000000000000100ffffffffffffffffffffffffffff0000fffffffffffffffffffffffffffffe7f` |
| `--encoding xcdr2 --endian be` | 96 bytes — `c00060000000000000000000000000003fff00000000000000100000000000003fff0000000000000000000000000001000100000000000000000000000000000000ffffffffffffffffffffffffffff7ffeffffffffffffffffffffffffffff` |
| `--encoding xcdr1 --endian le` | 96 bytes — `000000000000000000000000006000c00000000000001000000000000000ff3f0100000000000000000000000000ff3f00000000000000000000000000000100ffffffffffffffffffffffffffff0000fffffffffffffffffffffffffffffe7f` |
| `--encoding xcdr1 --endian be` | 96 bytes — `c00060000000000000000000000000003fff00000000000000100000000000003fff0000000000000000000000000001000100000000000000000000000000000000ffffffffffffffffffffffffffff7ffeffffffffffffffffffffffffffff` |

The two encodings agree here because every member is sixteen bytes and
starts at a multiple of sixteen, so eight-capped and four-capped
alignment insert the same nothing. A seventh member of another width
would separate them; `sequence-double-and-longdouble` already does.

The bytes are what the Rust probe produces, recorded so that this case is
known to have been run and not merely written. They are not a reference:
the reference is the foreign implementation's own output, which the suite
compares against — and for this construct there is none, which is itself
the finding.
