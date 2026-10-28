# union-discriminator-enum-bitbound

**Construct** — one 8-bit enumeration used as a union discriminator and as a member of the same type

XTypes 1.3 §7.4.3.5.4 serialises a discriminator as its own type, and §7.3.1.2.1.9 with §7.4.3.4.2 make an `@bit_bound(8)` enumeration one octet. Neither clause distinguishes the two positions, so the same enumeration should be the same width in both. The case puts it in both and makes the two distinguishable: `marker` sits directly behind `shade`, so a one-octet `shade` puts it at offset 1 and a four-octet one at offset 4, where trailing padding alone would have made the two spellings the same octets. Every union in the corpus switches on a 32-bit enumeration or on a primitive.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `02ab00000c00000001000000182d4454fb210940ffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `02ab00000000000c01000000400921fb54442d18ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `02ab010000000000182d4454fb210940ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `02ab010000000000400921fb54442d18ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
