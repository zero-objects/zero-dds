# identifier-keyword-member-ids

**Construct** — member ids and the KeyHash taken from the IDL spelling of a keyword name

The wire-visible half of GitHub issue #14. That report was answered by
escaping a reserved target-language word in the generated identifier —
`class` emitted as `class_` — with the statement that the original IDL
spelling stays authoritative for member ids, the KeyHash and the payload.
Nothing in the corpus held that statement to anything, because no case had a
member whose name was a reserved word in any target language.

`@hashid` with no argument makes the member id the first four octets of the
MD5 of the member's **name**, little-endian, masked to 28 bits — XTypes 1.3
§7.3.1.2.1.1 — and `@mutable` puts that id in an EMHEADER in front of every
member, so the id is in the octets rather than in an assertion about them.
Hash `class_` instead of `class` and the ids below change; every peer then
reads members it does not recognise and the sample decodes to defaults.
`@key` on the same member puts the name-derived id in the KeyHash's member
order as well, so an instance stops matching its own writer.

`plain` has no `@hashid` and is not a keyword, so a run that moved every id
and a run that moved only the hashed ones are distinguishable.

## What the four vendors did with it

Two write it and two cannot be asked, which is the split
`identifier-target-keywords` does not have:

| | |
|---|---|
| Cyclone `idlc` | writes octets; the ids below are its ids too |
| RTI | writes octets |
| `fastddsgen` | `genCdrAux.ipp:165: 'const class zd_c2::Sample' has no member named 'operator()'` — `operator` reached the generated C++ unescaped and became the call operator |
| OpenDDS | `expected unqualified-id before 'class'` |

So the member ids are held against two foreign implementations and not four,
and the two that refuse do so in the generated C++ rather than in the IDL —
the type is legal, the mapping of it is not.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `24000000a2f2edaf7929edff4b583336000000000020094066f6181baaaa000067f6182b0100007f` |
| `--encoding xcdr2 --endian be` | 40 bytes — `00000024afedf2a2ffed29793633584b40092000000000001b18f666aaaa00002b18f6677f000001` |
| `--encoding xcdr1 --endian le` | 72 bytes — `017f0800a2f2ed4f040000007929edff017f08004b583306080000000000000000200940017f080066f6180b02000000aaaa0000017f080067f6180b040000000100007f027f0000` |
| `--encoding xcdr1 --endian be` | 72 bytes — `7f0100084fedf2a200000004ffed29797f0100080633584b0000000840092000000000007f0100080b18f66600000002aaaa00007f0100080b18f667000000047f0000017f020000` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
