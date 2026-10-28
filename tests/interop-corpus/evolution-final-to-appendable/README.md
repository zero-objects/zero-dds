# evolution-final-to-appendable

**Construct** — a `@final` writer and an `@appendable` reader over the same member set

The failure GitHub issues #41 and #42 report, written down as a pair.
Every member is the same, every type is the same, every name is the same; the
two files differ in one annotation. Under encoding version 1 the two readings
are octet for octet identical and the exchange works. Under encoding version 2
the writer emits no delimiter header and the reader expects one, so the reader
takes the first four octets of the payload — here the `long a`, −1234567 — for
a length, gets 4293732729, and stops. That is the whole of the reported
symptom: *deserialization … failed (for reasons unknown)* on a type nobody
changed.

Two things about it are worth having as a row rather than as prose. The
divergence is invisible in one encoding and fatal in the other, which is why
the report says the pair worked *until* XCDR2 was turned on. And the failure
is a misread length rather than a short read, so the diagnostic names an
absurd number and not a missing member — a reader arriving from that
diagnostic has nothing to search for unless it is written down.

XTypes 1.3 §7.2.4.4.8, Table 19 makes this pair non-assignable: two types are
assignable only if their extensibility kinds agree. So the outcome recorded
below is what the clause requires and not a defect in either side. Which
answer to reach for is `tests/interop-results/matrix.html`, "Which flag do I
set?": annotate the type, which settles it for every implementation at once,
or set the dialect of the peer, which settles it for that peer.

Two reports share this cause and are deliberately not claimed by this case.
The CycloneDDS interoperability report, tracker 27, reaches the framing
question through a reported failure that turned out to be discovery on a
multi-adapter host; the octets here answer its XCDR2 half and nothing about
whether two participants find each other. Tracker 30 asks for the mismatch to
be caught at *match* time — a peer's standard `PID_TYPE_INFORMATION` consumed
so an unassignable pair is rejected instead of matching and then failing to
decode — which is a question about discovery parameters and not about octets.
Neither is a row this page can produce.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 18 bytes — `0e0000007929edff0000000000200940fdff` |
| `--encoding xcdr2 --endian be` | 18 bytes — `0000000effed29794009200000000000fffd` |
| `--encoding xcdr1 --endian le` | 18 bytes — `7929edff000000000000000000200940fdff` |
| `--encoding xcdr1 --endian be` | 18 bytes — `ffed2979000000004009200000000000fffd` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

## Evolution

`old.idl` writes its own sample and `type.idl` reads those bytes.
XTypes 1.3 §7.4.1.2.3 and §7.2.4.4.8, Table 19.
The Rust, C and Python columns are all held to this table:
`crates/wire-probe/tests/evolution.rs`. The C half runs under
`ZERODDS_GATE_TIER=full`, because it compiles two translation units
per row under `-Werror`.

| run | what `type.idl` reads from `old.idl`'s bytes |
|---|---|
| `--encoding xcdr2 --endian le` | **no** — Evo: expected a readable value, found at 0: a length of 4293732729 with 10 bytes left |
| `--encoding xcdr2 --endian be` | **no** — Evo: expected a readable value, found at 0: a length of 4293732729 with 10 bytes left |
| `--encoding xcdr1 --endian le` | `{"a":-1234567,"b":3.140625,"c":-3}` |
| `--encoding xcdr1 --endian be` | `{"a":-1234567,"b":3.140625,"c":-3}` |
