# The live half of the padding question

`tools/optional-bits/gate.sh` asks what a vendor's serialiser writes
into alignment padding. These four programs ask whether what it writes
gets *sent*, by putting a real writer and a real reader on a domain and
letting the sample cross an RTPS transport.

The two are not the same question. A serialiser that leaves
uninitialised process memory in padding is a curiosity as long as the
bytes stay inside the process; the moment they are on a wire they are an
information leak, and the reader on the other side has to be shown to
ignore them rather than assumed to.

```
cyclone/    pub.c  sub.c  gen.idl     `idlc` + `dds_create_writer`
cyclone1/   the same under encoding version 1
fastdds/    pub.cpp sub.cpp gen.idl   `fastddsgen` + `DataWriter`
rti/        app.c  gen.idl            `rtiddsgen -example`, both roles in one
```

There is no driver script and there was none on the test host either:
each pair was built by hand against its vendor and run as `./pub` and
`./sub` on a quiet domain, with the capture taken beside it. The
generated type support is not in the tree — `idlc`, `fastddsgen` and
`rtiddsgen` write it from the `gen.idl` next to each program.

Build them the way `tools/byte-gate/gate.sh` builds its probes; the
include and link lines are the same, and `tools/interop-vendors.sh` has
the prefixes.

Was `/root/opt-bits/wire`.
