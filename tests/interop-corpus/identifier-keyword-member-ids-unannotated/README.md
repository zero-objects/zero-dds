# identifier-keyword-member-ids-unannotated

**Construct** — member ids and the KeyHash taken from the IDL spelling of a keyword name, unannotated

The twin of `identifier-keyword-member-ids`: the same file with `@mutable`
stripped and nothing else changed. `@hashid` and `@key`
stay, so the member ids are still derived from the keyword-named members'
spellings — they are simply no longer written into the payload, which is what
the pair is for. The KeyHash is computed from the same ids in both, so it is
identical here and in the twin while the payload is not.

Under the shipped default this type is `@final` and carries no delimiter
header; under `--dialect rti`, `--dialect fastdds` and `--dialect
specification` it is `@appendable` and carries one under encoding version 2.
An escaping that moved the ids would move this case's KeyHash and leave its
payload alone, which no other case in the corpus can tell apart.

The tracker number is on `identifier-keyword-member-ids`, deliberately and
only there. This case is in the default-extensibility family, and the page
carries every number written on a case in that family to every other case in
it, so a number written here would be answered by a hundred and fifty rows
about something else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `140000007929edff0000000000200940aaaa00000100007f` |
| `--encoding xcdr2 --endian be` | 24 bytes — `00000014ffed29794009200000000000aaaa00007f000001` |
| `--encoding xcdr1 --endian le` | 24 bytes — `7929edff000000000000000000200940aaaa00000100007f` |
| `--encoding xcdr1 --endian be` | 24 bytes — `ffed2979000000004009200000000000aaaa00007f000001` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
