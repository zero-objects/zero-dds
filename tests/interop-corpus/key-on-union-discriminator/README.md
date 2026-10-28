# key-on-union-discriminator

**Construct** — `@key` on the discriminator of a union that is the topic type

DDS-XTypes 1.3 Table 21 lists `@key` against "Structure Members, Union
discriminator", and §7.2.2.4.4.4.8's own example is
`union MyCommand switch (@key CommandKind)`. The key of the union is its
discriminator: the KeyHash is the discriminator serialized in big-endian
PLAIN_CDR2 and padded with zero octets to sixteen, `00000001` followed by twelve
zero octets for the discriminator 1. The payload of a `@final` union is the
discriminator and the selected member, with no header.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.
Measured earlier outside the corpus (`n11-keyunion`, 2026-08-10): Cyclone's
`idlc` refuses the file, `rtiddsgen` refuses it at the discriminator and
`fastddsgen` generates no `PubSubType` for a union.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `0100000005000000` |
| `--encoding xcdr2 --endian be` | 8 bytes — `0000000100000005` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0100000005000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000100000005` |
| `--emit keyhash` | 16 bytes — `00000001000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
