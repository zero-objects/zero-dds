# identifier-target-keyword-enumerator

**Construct** — enumerators that are keywords of the generated target language

The second of the two identifier positions `identifier-target-keywords` cannot
reach. Its enumeration `class` is inside `module package`, so the generated
enumerators come out `package_template`, `package_virtual` and
`package_friend` — the keyword is a segment of a longer identifier and no
compiler reads it as a keyword. An enumeration at the outermost scope has no
path in front of it and each enumerator stands alone.

`register`, `volatile` and `signed` are C keywords and are in no list of OMG
IDL 4.2 §7.2.3. The enumeration is `Widths` and not `Register`: IDL compares
identifiers without regard to case within one scope (§7.2.3.1), so an
enumeration named for one of its own enumerators is a duplicate name and not a
keyword question.

## What the generators do with it

Measured 2026-08-11 on the Linux test host, over this file.

Both vendors wrote octets, and both wrote ours:

```text
cyclone  xcdr1/le  010000000100007f      rti  xcdr1/le  010000000100007f
cyclone  xcdr2/be  000000017f000001      rti  xcdr2/be  000000017f000001
```

RTI's is the measurement. `fill.rs` had already put every identifier through
`fill::identifier`, which prefixes `_c_` for RTI, so the write side compiled
against `_c_volatile` — which means `rtiddsgen -language C` does rename an
enumerator, as it renames a member. `dump.rs` pasted `node.name`, so the read
side of the same translation unit said `case (long long)volatile:` and the two
halves named one enumerator two ways. That is the defect this case landed to
find, and the vendor decided which of the two was right.

Cyclone's cell is not an answer to the same question — see below.

The unannotated twin carries a second reading, which is the twin's own point:
under XCDR2 RTI puts a `00000008` DHEADER in front of the aggregate where the
`@final` case has none, and Cyclone writes neither.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `010000000100007f` |
| `--encoding xcdr2 --endian be` | 8 bytes — `000000017f000001` |
| `--encoding xcdr1 --endian le` | 8 bytes — `010000000100007f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `000000017f000001` |

The bytes are what the Rust probe produces today, recorded so that this case is
known to have been run and not merely written. They are not a reference: the
reference is the foreign implementation's own output, which the suite compares
against.
