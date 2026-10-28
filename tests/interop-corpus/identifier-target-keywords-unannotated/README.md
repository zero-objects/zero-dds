# identifier-target-keywords-unannotated

**Construct** — identifiers legal in IDL and reserved in a generated target language, unannotated

The twin of `identifier-target-keywords`: the same file with `@final`
stripped from both structures and nothing else changed, so the pair isolates
the extensibility default. Under the shipped
default an unannotated aggregate is `@final` and the two cases write the same
octets; under `--dialect rti`, `--dialect fastdds` and `--dialect
specification` this one gains a delimiter header per aggregate under encoding
version 2 and the other does not.

Both halves of the reserved-keyword report are here: a keyword-named
identifier reaches each foreign front end twice, once under an explicit
extensibility and once under the vendor's own default, so a front end that
refuses the identifier refuses it in both and a front end that refuses only
one is telling a different story.

The tracker number is on `identifier-target-keywords`, deliberately and only
there. This case is in the default-extensibility family — Cyclone's `idlc`
prints its no-default-extensibility warning over it — and the page carries
every number written on a case in that family to every other case in it, so
a number written here would be answered by a hundred and fifty rows that have
nothing to do with an identifier.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `280000000e0000007929edff0000000000200940aaaa0000010000000100007f070000006772c3bcc39f0001` |
| `--encoding xcdr2 --endian be` | 44 bytes — `000000280000000effed29794009200000000000aaaa0000000000017f000001000000076772c3bcc39f0001` |
| `--encoding xcdr1 --endian le` | 40 bytes — `7929edff000000000000000000200940aaaa0000010000000100007f070000006772c3bcc39f0001` |
| `--encoding xcdr1 --endian be` | 40 bytes — `ffed2979000000004009200000000000aaaa0000000000017f000001000000076772c3bcc39f0001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
