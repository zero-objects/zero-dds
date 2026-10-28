# identifier-target-keywords

**Construct** — identifiers legal in IDL and reserved in a generated target language

Built from GitHub issue #14: `CharacterClass class;` compiled, and the C++
backend refused it — *invalid identifier 'class': reserved C++ keyword*. IDL
4.2 §7.2.3 lists the words an identifier may not be, and `class`, `new`,
`delete`, `operator`, `template`, `virtual`, `friend`, `package`, `type`,
`impl`, `static`, `def`, `async` and `lambda` are none of them. A generator
that refuses one of them refuses valid IDL; a generator that escapes one of
them into the *type system* changes what goes on the wire.

Every position the report's fix had to reach is here at once, because they
fail independently: the module (`package`), the enumeration (`class`) and its
enumerators (`template`, `virtual`, `friend`), a structure name (`type`), that
structure's members (`new`, `delete`, `operator`), and the top-level members
(`impl`, `lambda`, `static`, `def`, `async`), one of which has a keyword-named
type.

The member type is written `type` and not `::package::type` on purpose. The
root-anchored form was here first and made the case unmeasurable: Cyclone
(*Scoped name '::package::type' cannot be resolved*), Fast DDS (*'::' was not
defined previously*) and `tao_idl` (*error in lookup of symbol*) all refuse it
before they reach an identifier, so every vendor column said `refused` for a
reason that had nothing to do with the question. That divergence is
`module-root-anchored`'s, and one variable per case is what keeps the two
apart.

What the case decides is the second half of the report's answer, and it is
the half a compilation check cannot make: the escaping is allowed to change
the generated identifier and is **not** allowed to change anything wire
visible. The bytes below are the assertion. `def` is a string so that a
keyword-named member of variable length is in the payload, and `impl` is
the nested aggregate so that a keyword-named type is reached through a
keyword-named module.

`identifier-keyword-member-ids` carries the part of the question this case
cannot ask: a member id and a KeyHash are derived from the member's *name*,
so a backend that escapes before it hashes moves them.

## What the four vendors did with it

All four refuse, each at a different word, and none of them at the same word:

| | where it stopped |
|---|---|
| Cyclone `idlc` | the generated C descriptor: `offsetof (zd_c4_package_Sample, static)` |
| `fastddsgen` | `gen.hpp:1042: invalid use of 'class zd_c4::package::Sample::m_lambda'` — the type named `class` |
| OpenDDS | `sample.lambda = ::zd_c4::package::virtual;` — the enumerator named `virtual` |
| RTI | the probe did not compile; `rtiddsgen` names no identifier |

The pattern is the one issue #14 reported and it is not this project's alone:
each front end escapes some positions and not others, and the position it
misses is the one it has not been shown. The two ROS 2 type supports refuse it
as well. The refusals are recorded here as the result — a construct four
implementations refuse is a statement about the construct's reach, not a hole
in the corpus — and the four ZeroDDS columns are what say the payload is
reachable at all.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `7929edff0000000000200940aaaa0000010000000100007f070000006772c3bcc39f0001` |
| `--encoding xcdr2 --endian be` | 36 bytes — `ffed29794009200000000000aaaa0000000000017f000001000000076772c3bcc39f0001` |
| `--encoding xcdr1 --endian le` | 40 bytes — `7929edff000000000000000000200940aaaa0000010000000100007f070000006772c3bcc39f0001` |
| `--encoding xcdr1 --endian be` | 40 bytes — `ffed2979000000004009200000000000aaaa0000000000017f000001000000076772c3bcc39f0001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
