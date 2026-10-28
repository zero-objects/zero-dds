# What eleven implementations put on a socket

Every probe under `tools/wireprobe/` calls a vendor's **in-memory**
serialisation entry point. None of them opens a socket. That is fine for
every question a serialiser decides on its own, and it is not fine for
any question a transport layer can decide instead.

This directory is the second measurement level. Two runs live here.

| run | what it asked | scope |
|---|---|---|
| [the fill run](#the-fill-run) | what a peer puts in the alignment fill | one type, five peers, 2026-08-13 |
| [the corpus run](#the-corpus-run) | what the octets are, per corpus case | **107 cases, eleven implementations, 1605 combinations, 901 goldens; nine on 2026-08-20, OpenSplice and HDDS on 2026-10-07** |

The corpus run subsumes the fill run's method and widens it; the fill
run is kept because it is the measurement that eight rows of
`docs/wire-feature-inventory.md` and five probe templates cite, and its
type is shaped for that one question.

---

## The corpus run

`wire-golden.sh` takes a corpus case through a real `DataWriter` or a
real GIOP call, captures the loopback, and writes the octets that
carried the sample. `compare-levels.py` puts those beside the octets the
same implementation's in-memory entry point produced for the same case
— `tests/interop-results/sources/{vendor,ros,corba}-corpus.tsv` — and
the result is `tests/interop-results/wire-goldens.tsv`.

**One command runs the whole thing:**

```sh
. tools/interop-env.sh
tools/wire-capture/wire-golden.sh --all --out /root/wire-goldens
```

That is eleven implementations, each on a domain offset of its own, and
then the four files this tree keeps: `frames/`, `run-log.tsv`,
`tests/interop-results/wire-goldens.tsv` and the `coverage.tsv` ledger.
It ends with the summary line and the ledger's own count:

```
wire-golden: 107 cases x 11 implementations — 1605 rows, 901 goldens
coverage: 112 of 146 behaviours carry a wire-level measurement, 34 do not (1 no-golden, 33 no-wire)
```

One implementation on its own is still `--impl`, and `--assemble`
rebuilds the four files from an `--out` directory without running
anything.

```sh
tools/wire-capture/wire-golden.sh --impl cyclone --out /root/wg-run1
tools/wire-capture/wire-golden.sh --impl opendds --out /root/wg-run1 --retry
tools/wire-capture/wire-golden.sh --assemble --out /root/wg-run1
```

**How long it takes is a property of the host and not of the run.** The
2026-08-20 sweep took 1 h 55 min wall on the measurement host with
`--jobs 3`: the first hour at load 35, with four other sweeps on the
same eight cores, and the rest at load 5. OpenDDS is the long pole and
the reason is its build — `tao_idl`, `opendds_idl`, `tao_idl` again and
a `g++` over four generated translation units, once per case, which ran
at 90 s a row under load and 18 s a row without it. The load also
decides how much `--retry` there is to do: that sweep produced 50
`!unread` and 43 `!publisher` rows, and one retry pass at load 4 turned
every `!unread` and 16 of the `!publisher` into goldens.

### Why 107 cases and not 23

The first run of this harness, 2026-08-13, took the twenty-three cases
where the inventory already carried a divergence or a setting. It
produced 212 goldens and `docs/wire-feature-inventory.md` credited
**seven** of its 146 behaviours with a wire-level measurement in the
row's own text — A4, A5, B4, B5, C3, H8 and N12 — and three more (D3,
D4, D5) in the run's own section. The other 136 stood on an in-memory
assertion, which is a claim about a serialiser and not about a product.

**The limit was never the harness.** `--cases` defaults to every row of
`cases.tsv`, and `cases.tsv` had twenty-three rows. The git history is
the whole answer:

| commit | `cases.tsv` | `wire-goldens.tsv` |
|---|---|---|
| `950c48af9` | 23 cases | 299 rows |
| `419e496f4` | **107 cases** | **299 rows — unchanged** |

So the cut was widened to 107 and the table was not: the sweep that
would have filled it was started on 2026-08-19 and its output never
reached the tree. What that left is what `coverage.py` prints for
`419e496f4`'s tree — *"a case that no run reached: C7 D1 F1 G1 G4 G6 G14
G16 H6 H7 I7 I11 I15 I20 I22 J3 J10 J4 J9 M1 M3 P1 P2 P3 P4 P5"* — 26
behaviours with a case, no measurement and no reason, which is neither
of the two things a finished row can be. The three `tshark` processes
that sweep left running are in [their own
section](#what-a-killed-run-used-to-leave-behind).

The cut itself is built backwards from the inventory and the reason is
in `cases.tsv`'s header: **every behaviour a writer can put on a socket
gets a case**, whether or not a divergence was expected there.
`coverage.tsv` is the ledger — one line per behaviour, either the cases
that reach it or the one-line reason no capture can — and `coverage.py`
refuses to write it if any behaviour has neither, has both, or has a
case nothing ran.

`--all` runs it as its last step, so the ledger cannot describe the run
before last:

```sh
tools/wire-capture/coverage.py        # or --all, which ends with it
```

### The poison samples

Each case is written **poison, corpus, poison, corpus, …** through one
writer, and two of the four checks rest on the two samples differing.
The twenty-three original poison files are hand-written and *longer*
than their corpus sample wherever the type allows, so the corpus
sample's fill lands on octets the poison call wrote — that is the
control A4's fill-provenance question needs, and `cases.tsv` marks those
cases `longer`.

The eighty-four generated ones come from `make-poison.py`, which mutates
**numeric leaves only** and by a bijection (`v ^ 1` for an integer, `-v`
for a float, the flip for a boolean). A bijection is not tidiness: it is
what keeps a mutated `map` from coming out with two equal keys, which a
vendor's own map would refuse at run time and which would read as a
refusal of the case. A union's `_d`, bitmask flag lists and string
contents are left alone, each for a reason the script names; an
**enumerator** rotates to the next enumerator the case's own `type.idl`
declares, which is a bijection on that enum and is always a name the
file has. Those cases carry `equal`, and their fill column reads
`undetermined`.

```sh
tools/wire-capture/make-poison.py --cases-file tools/wire-capture/cases.tsv
```

**Bulk never overwrites.** `--force` applies to the cases named with
`--case` and to nothing else; the form over `--cases-file` reports
`0 written, 107 kept`. That is a guard rather than a convenience:
`--cases-file … --force` used to overwrite all twenty-three hand-written
files with generated ones and report "107 written", and thirteen of the
twenty-three are the *longer* controls the paragraph above is about — a
generated sample has the same layout and cannot reach the other's fill
at all, so the loss would have surfaced as those cases quietly answering
`undetermined`. `--force-every-case` is the word for meaning it.

### A fourth transport setting, and a fourth false green

OpenDDS's `rtps_udp` transport has `use_multicast` on by default, so its
reader advertises a multicast locator — 239.255.0.2 — and its writer
sends the user `DATA` there. With the transport bound to `127.0.0.1`
that datagram does not reach the reader on this host. What the run
reports is the quietest failure this harness has: the publisher writes
six samples and reports a matched reader, the capture holds all six, and
the subscriber reads **nothing**. Check 1 refuses the row, and the first
eleven OpenDDS runs of the 2026-08-19 sweep were exactly that.

`use_multicast=0`, measured over `struct-final` XCDR2 with the same
binary and the same domain: `read 0` without it, `read 6` with it. It
changes the destination locator and not one octet of the
`SerializedPayload`, and it joins Cyclone's `ParticipantIndex` and Fast
DDS's `data_sharing off` in the list of settings that each removed a
false green rather than tidied anything.

**And it uncovers the next one.** With multicast off, OpenDDS's unicast
`DATA` leaves from `local_address=127.0.0.1:0` — an ephemeral port — and
`extract-rtps.py` attributes a packet by mapping its destination port
back to a domain. An ephemeral port maps to no domain, so every run then
reported `no user DATA in the capture` while its subscriber read all
six: the run was right and the attribution was blind. The transport,
SEDP and SPDP unicast ports are pinned into the domain's own 250-port
block now, one set for the publisher and one for the subscriber, because
they are two processes on one host. Under multicast the data had gone to
the domain's multicast port and had been attributable all along, which
is why the first sweep of this harness met neither half.

`--retry` is for the other kind of failed row — a run that did not
happen because the host was busy. It re-runs the cases whose row says
so and no others: `!refused` is the product's answer and `!unstable` is
a finding, and neither is ever retried.

### What a killed run used to leave behind

`run_dds` and `run_giop` start `tshark` in the background and neither
returns before stopping it, so a run that *finishes* leaves nothing. A
run that is **killed** left both the capture and the `mktemp -d` the
build wrote into, and nothing ever collected them.

Measured on 2026-08-20, on this measurement host:

| what | since | size |
|---|---|---|
| `/tmp/tmp.oVuZPYbuPC/cap.pcap` | 2026-08-13 13:51 | **7 655 755 762** octets |
| `/tmp/tmp.BrvOgTJ6dC/cap.pcap` | 2026-08-13 13:54 | **7 942 895 330** octets |
| three `tshark`/`dumpcap` pairs, PIDs 3208548, 3426376, 3728753 | 2026-08-19 18:35–19:18 | still running, 11 h |

15 GB, and the reason it is that size rather than a few megabytes is in
the filter: `(udp portrange L-H) or (udp[8:4] = 0x52545053)`. The
second half is there so that a product ignoring §9.6.1.1's port scheme
still appears — and it matches *every* RTPS datagram anything else on
the host sends, so a leaked capture is not idle. Those two wrote for
three days after their run was killed. On a host at 94 % this is not
tidiness: it is the sweep that comes next failing for want of disk.

`wire-golden.sh` traps `EXIT`, `INT` and `TERM` now, kills the capture
and removes the work directory; `--all` forwards the signal to its
children so each of them runs its own trap. Checked by killing a run
mid-case: `tshark` 1994967 was running before the `TERM`, and
afterwards neither it nor `/tmp/tmp.wSgi2cVw4N` existed.

### The eleven

| implementation | version | transport | writer |
|---|---|---|---|
| Eclipse Cyclone DDS | 11.0.1 | RTPS/UDP | `dds_write` |
| RTI Connext | 7.7.0 | RTPS/UDP | `<T>DataWriter_write`, C API |
| eProsima Fast DDS | 3.6.1 | RTPS/UDP | `DataWriter::write` |
| OpenDDS | 3.34.0 | RTPS/UDP | `<T>DataWriter::write` |
| OpenSplice Community Edition | 6.9 (OSPL_V6_9_210323OSS_RELEASE) | RTPS/UDP (DDSI2) | `<T>DataWriter::write`, SACPP |
| HDDS | runtime 1.1.2, hddsgen 1.2.0 | RTPS/UDP | `DataWriter::write` |
| `rosidl_typesupport_fastrtps_c` | 2.2.2 over Fast DDS 2.6.10 | RTPS/UDP | `rmw_publish` through `rmw_fastrtps_cpp` |
| `rmw_cyclonedds_cpp` | 1.3.4 over Cyclone 0.10.5 | RTPS/UDP | `rmw_publish` through `rmw_cyclonedds_cpp` |
| omniORB | 4.3.3 | GIOP/TCP | a remote `put` |
| TAO | 2.5.24 | GIOP/TCP | a remote `put` |
| JacORB | 3.9 | GIOP/TCP | a remote `put` |

**Which of them can be driven live decides half the table.** Per
implementation, 2026-08-20, out of 214 combinations for the four DDS
products and 107 for the ROS type supports and the ORBs:

| implementation | golden | refused | writer failed |
|---|---|---|---|
| Cyclone DDS | 145 | 62 | 7 |
| RTI Connext | 135 | 76 | 3 unstable |
| Fast DDS | **191** | **18** | 5 |
| OpenDDS | 129 | 74 | 11 |
| `rosidl_typesupport_fastrtps_c` | 30 | 74 | 3 |
| `rmw_cyclonedds_cpp` | 33 | 74 | 0 |
| omniORB | 59 | 48 | 0 |
| TAO | 57 | 50 | 0 |
| JacORB | 57 | 49 | 1 |

OpenSplice and HDDS joined on 2026-10-07, after the other nine, out of
107 combinations each — OpenSplice writes XCDR1 only, and the published
HDDS runtime encodes XCDR2 only:

| implementation | golden | refused | writer failed |
|---|---|---|---|
| OpenSplice 6.9 | 28 | 69 | 10 unstable |
| HDDS | 37, and 14 with no in-memory row | 55 | 1 unstable |

**OpenSplice** runs the single-process deployment with the `ddsi2`
service alone, on the loopback, `ParticipantIndex auto` and unicast
discovery (`lo` carries no multicast). Its 69 refusals are the in-memory
probe's: the same IDL lowering (`opensplice-idl.py`) and `idlpp` reject
the same constructs, `@mutable`, `wchar`/`wstring` in a topic type,
`bitmask`, `bitset`, `map`, inheritance. The ten unstable rows differ only
in alignment fill — `struct-final` at octets 4–5, between the `long` and
the eight-aligned `double`; the union cases behind a one-octet
discriminator — which is the in-memory column's 30 padding-only rows seen
on the socket.

**HDDS** has no published runtime that compiles hddsgen 1.2.0's output
(`tools/wireprobe/hdds-shim/src/lib.rs`). `hdds-wire-template.rs` builds
the published runtime 1.1.2 and implements its `dds::DDS` for the
generated type by forwarding: `encode_cdr2` is the generated
`encode_xcdr2_le`, `decode_cdr2` the generated `decode_xcdr2_le`, the
`TypeObject` and the key the generated ones. No octet of a sample is
written by the adapter; the encapsulation, the trailing padding and the
inline QoS are the runtime's. Measured: hddsgen marks every struct
`IS_FINAL` — `@appendable` included, as `Dialect::Hdds` already records
in memory — and the runtime takes `0x0001`, PLAIN_CDR_LE, for a `@final`
type, so all 51 captured samples are XCDR2 octets under the XCDR1
encapsulation id. The 55 refusals are generated code that does not
compile, and match the in-memory probe's "the probe did not compile";
`map-key-string` is unstable because hddsgen maps `map<string, long>`
to a `std::collections::HashMap`, which iterates in a different order
per sample.

Fast DDS is the widest front end here by a distance, and that is not a
preference: **nine of the 112 measured behaviours are measured by Fast
DDS and by nothing else** — G9, G10, G11 (`map`), H6, H7 (inheritance)
and P1, P2, P3, P5 (`bitset`) — because every other front end in the
table refuses the construct. `coverage.tsv`'s `detail` column names the
implementations per row rather than counting them, for exactly this
reason.

### The cases, and why those

`cases.tsv` carries the reason per case, and the set is built backwards
from `docs/wire-feature-inventory.md`: a behaviour a writer can put on a
socket gets a case that carries it. The first twenty-three are the
families where the inventory already carried a divergence or a setting —
an alignment gap, an encapsulation field, a byte order, the key holder
whose hash only a real writer sends. The rest close the ledger:
`array-1d-primitives` for rule (8)'s `PARRAY_TYPE`,
`struct-inheritance-mutable` for whether member ids run on across the
inheritance boundary, `annotation-id-sparse` for `PID_EXTENDED`,
`bitset-*` and `fixed-*` for the constructs the standard does not
specify at all.

The corpus holds **413** cases and this run takes 107. The 306 it leaves
are not a backlog: **158 of them are an `-unannotated` twin** of a case
that is here (H9 is the one behaviour that pair answers, and
`primitive-mixed` and `primitive-mixed-unannotated` answer it), and the
rest are a second sample of a construct another case already puts on
the wire — `array-1d-enum` beside `sequence-enum`,
`enum-bitbound-twelve` beside `enum-bitbound-8-plain`,
`evolution-mutable-tail` beside `evolution-mutable-reordered`.

**Adding any of the 306 would add goldens and could not add a
behaviour.** Two halves to that, and only the first is
`coverage.py`'s: the ledger is complete — every one of the 146 has a
case or a reason — so a 108th case can only land on a row that already
has one. The second half is the two rows whose reason is `no-case`,
where a capture *could* reach the behaviour and no case carries the
shape, and neither is in the 306 either:

```sh
grep -rln 16128 tests/interop-corpus --include=type.idl            # I12: none
grep -rn must_understand tests/interop-corpus --include=type.idl   # I8: three files,
                                                                   # ids 3 and below
```

I12 needs member id `0x3F00` exactly and no case declares it; I8 needs
one member that is both `@must_understand` and past `0x3F00`, and the
three cases that use the annotation at all put it on ids 3 and below.
Closing either means a **new** corpus case, `tests/interop-corpus/` is
another strand's tree this cycle, and `reasons.tsv` names them rather
than closing them. That is the one place where the wire-level count is
held down by something other than the mechanism, and it is two rows.

The count that can still move without a new case is the number of
*implementations* behind a row, which is the `detail` column of
`coverage.tsv` and not its verdict.

The corpus figures were 378 and 271 until 2026-08-20 and were carried
from the revision before the corpus grew; they are counted off
`tests/interop-corpus/` here.

### The four checks

A run produces a golden only if all four hold. A failure writes `!` and
the reason into the row; a silent skip would read as agreement.

1. **The subscriber read every sample.** A capture of a writer nobody
   was listening to is a capture of nothing.
2. **Every sample reached the socket**, counted over the `DATA`
   submessages that carry one — not over sequence numbers, because
   OpenDDS spends sequence number 1 on a key-only `DATA` and a rule that
   read the parity would swap the poison and the corpus sample from
   there on.
3. **The corpus sample serialised to the same octets every time**, in
   steady state. A serialiser whose output is not a function of its
   input makes every column meaningless.
4. **The corpus sample and the poison sample differ.** If they do not,
   the extractor is reading something that is not the sample.

### A refusal the in-memory column does not carry

Most of what 107 cases through nine implementations produces is
`!refused`, and the table only means something if every one of those is
the *product's* refusal. `refusal-check.py` is that check, and it is not
decoration — it found twelve rows of the 2026-08-19 sweep that were this
harness refusing a type the same implementation's front end had already
accepted:

```sh
tools/wire-capture/refusal-check.py
```

| what it found | what it was |
|---|---|
| RTI, six rows, `'true' undeclared` | `wireprobe-fill --vendor rti` writes `sample.flag = true;` and `rti-wire-template.c` is C without `<stdbool.h>` |
| TAO, four rows, "anonymous types require the IDL version to be 4 or later" | `build_tao` called `tao_idl` at its default IDL 3, where `build_opendds` had always passed `--idl-version 4` |
| Fast DDS, two rows, `no matching function for call to 'Sample::note(const char [33])'` | `combined-everything`'s poison set an `@optional string`, which `wireprobe-fill`'s Fast DDS mapping cannot express — the same gap the in-memory column records for `annotation-must-understand` and `annotation-optional-mixed` |

Each of the twelve read, in the table, as a product refusing its own
type. The first two are fixed here; the third is a gap in
`wireprobe-fill` that both levels have, and the poison now leaves that
member absent, as the corpus sample does.

**Over the 2026-08-20 sweep it found eighteen more, in two kinds, and
both were mistakes of this directory's own:**

| what it found | what it was |
|---|---|
| six rows, `enum-negative-value` through Fast DDS, OpenDDS and RTI: `wireprobe-fill: Sample.first: not a member; the type declares ["MINUS", "ZERO", "MAX"]` | `make-poison.py`'s character fallback fires for a case with no numeric leaf, and that case's sample is two **enumerator names** — it shifted them a letter each into `"NJOVT"` and `"NBY"`. An enumerator now rotates to the next enumerator the case's own `type.idl` declares, which is a bijection and always a name the file has. The six became goldens, and one is a divergence: `{MINUS, MAX}` is `ffffffff ffffff7f` from RTI and Fast DDS and **`00000000 02000000` from OpenDDS**, its ordinals |
| twelve rows, the ORB columns on `fixed-basic`, `fixed-scales`, `primitive-wchar-above-bmp` and three `union-discriminator-*` cases | **`refusal-check.py` itself.** `corba-corpus.tsv` has no status column: a refusal lives in the *payload* column behind an `ERR1:` prefix, and this file read every `write` row as octets. All twelve carry `ERR1:` on the in-memory side too, so the check had accused the harness of refusing what the ORB accepts. `compare-levels.py` had the same reading and would have put a sentence where the `in-memory` column promises octets; both skip `ERR1:` now |

After both, the check exits 0 over the whole table: **no build refusal
has an in-memory counterpart.**

The check is one-directional on purpose. It looks for **in-memory wrote,
wire produced nothing**, and splits what it finds by whether the *build*
or the *run* failed. A build failure with an in-memory counterpart is
this harness and exits non-zero; a run failure is a finding, and the 21
it still prints are exactly that — eleven OpenDDS rows where
`create_datawriter` returns null for a representation `opendds_idl`'s
`Serializer` writes, five Cyclone rows where `dds_create_topic` refuses
XCDR1 with `Bad Parameter`, three Fast DDS and two
`rosidl_typesupport_fastrtps_c` rows that throw out of Fast CDR.

### The poison sample, and what it decides

Each case is written **poison, corpus, poison, corpus, …** through one
writer or one connection, and `poison/<case>.json` is longer than the
corpus sample wherever the type has a variable-length member. The corpus
sample's fill spans then land on octets the poison call wrote, so a `00`
there is an octet the peer wrote and not an octet that was already
there. A buffer that happens to be zero and a buffer that was zeroed
look the same on one sample.

`cases.tsv`'s `control` column says which cases carry that control:
`longer` where the poison sample is longer, `equal` where the type has
no variable-length member and neither sample can reach the other's fill.
An `equal` case says nothing about fill provenance and its note says so.

The **first** call of a run has no predecessor, so on a peer that does
not write its fill the first call carries whatever the buffer held
before the run and every later one carries the previous call's sample.
That is why check 3 is a steady-state check and why the row carries
`first-call-differs` and `poison-varies` as data rather than as faults.

### Attribution

One domain per case, so one UDP port range per case, and inside the
capture the writer is found by the capture's **own SEDP**: the builtin
publications writer announces every user writer with its
`PID_ENDPOINT_GUID` and its `PID_TOPIC_NAME`. Counting writers instead
does not survive an RMW, which creates a `ros_discovery_info` writer of
its own; picking the writer with the expected number of samples would be
the harness deciding which measurement it wanted.

Three transport settings are not tidying and each removed a false green:

- **Cyclone's `ParticipantIndex`.** Unset, Cyclone binds an ephemeral
  port and advertises it in SPDP, and a capture filtered on the domain's
  ports is empty.
- **Fast DDS's `data_sharing().off()`**, and the same through XML for
  `rmw_fastrtps_cpp`. With data sharing at its default two endpoints on
  one host exchange the payload through mapped memory: the publisher
  reports six writes, the subscriber reports six reads, and no user
  `DATA` reaches a socket.
- **The loopback**, everywhere. The measurement host has a real
  interface and the corpus does not belong on it.

### The artifacts

**The captures are not committed; the frames are.** A loopback `.pcap`
of a DDS run is mostly discovery and ACKNACKs, there are 200 of them,
and nothing in the table is derived from the part that would make them
large. What is committed under `frames/` is the octets that carried the
measurement: every UDP payload holding a user `DATA`, the SEDP
announcements that attribute it to a topic, and for the ORBs the
reassembled GIOP streams.

`recheck.sh` re-derives every row from those files alone — no host, no
vendor, no `tshark`:

```sh
tools/wire-capture/recheck.sh tools/wire-capture/frames
```

It cannot re-derive the checks that are properties of the run rather
than of the octets: how many samples the publisher wrote, how many
endpoints it matched, and how many the subscriber read back. None of
those is in the octets — a capture of a writer nobody was listening to
looks exactly like a capture of one that was — so they are recorded
where they can be read, in `run-log.tsv`, one line per row of the table
that produced octets. **All 915 wrote six samples and read six back.**

`recheck.sh` over the committed frames produces **929 rows** — the 915
plus the fourteen `unstable`, whose instability is a property of the
octets and is re-derivable from them — and `diff` against the run's own
rows is empty. It needs no vendor.

The per-run logs themselves stayed on the measurement host, and
`run-log.tsv` is what they said. A claim whose evidence lives only on a
machine is a claim with a deadline; `/root/tc-env.sh` is what that
looks like afterwards.

### What was already in the tree

This directory is partly a second build of machinery that exists. What
follows is what each of the older pieces does, what it does not do, and
what was taken from it — so the next person does not build it a third
time.

**`tests/interop/capture.sh`** — `sudo tshark -i any` over domain 0's
ports against `tests/interop/docker-compose.yml`, then
`tshark -Y "rtps.sm.id == 0x15" -T fields -e data` into a hex column.
Its stated purpose is in its own epilogue: paste a frame into
`crates/rtps/tests/fixtures/cyclone/<name>.hex` and write a test around
it. It produces **frames**, not payloads — it does not walk the
submessages, does not find where `serializedPayload` begins, does not
read inline QoS, has no notion of a topic or of a corpus case, and
needs `tshark` again to decode. It answers "give me some Cyclone bytes
by hand". It is not a driver for 107 cases through nine products and
would not have become one for less than `wire-golden.sh` cost.

**`crates/ros2-rmw/interop/run_capture.sh`** — a Cyclone talker and
listener built from `std_msgs_string.idl` by `idlc`, captured with
`tcpdump -i any`. It is **Cyclone with a ROS-shaped topic and not a ROS
type support**: the serialiser is `idlc`'s output, where the two ROS
columns here are `rosidl_typesupport_fastrtps_c` and
`rmw_cyclonedds_cpp`, neither of which `idlc` is. So it does not
substitute for the ROS runs.

**Its capture filter is better than this harness's first one, and that
is a finding.** It matches `udp[8:4] = 0x52545053` — the RTPS magic —
so it catches a product on any port. This harness started with a filter
on the domain's ports, and Cyclone with `ParticipantIndex` unset binds
an ephemeral port and advertises it in SPDP: the capture came back
empty and the run had produced nothing while reporting six writes and
six reads. `wire-golden.sh` now filters on **both**, and the older
script is where that came from.

**`zerodds-examples/idl-conformance/proofs/rtps-framing/`** — five
`.pcapng` of Cyclone 11.0.1, Fast DDS 3.6.1 and this project on domain
0, decoded field by field with Wireshark's RTPS dissector. It asks a
different question: the submessage envelope, the SPDP/SEDP
`ParameterList`s and the reliable-protocol submessages, over one type
and three systems. It carries no per-case payload and no in-memory
comparison, so it does not answer this one.

**It is used here as the one check this harness cannot make against
itself.** Every other check compares the harness's octets against the
harness's other octets. `oracle-check.sh` runs `extract-rtps.py` over
`hex/{cyclone,fastdds,zerodds}.user_data.hex` and holds its answer
against the decode Wireshark recorded beside it in `decodes/`: same
encapsulation kind, same options field, same `serializedData`, all
three. A decoder that is wrong in the same way twice is what that
catches, and nothing else here could.

```sh
tools/wire-capture/oracle-check.sh
```

`zerodds-examples` is its own repository and is ignored here, so the
check runs where it is checked out beside this tree and says so and
stops where it is not. Run 2026-08-13: all three agree.

It also has something this run does not: **`frag/` exercises
`DATA_FRAG`.** Every case here fits one datagram on the loopback, so
`extract-rtps.py` parses `DATA_FRAG` and has never been given one.

**`crates/corba-interop/competitors/`, deleted by the rebuild and still
in the history** (`git log --diff-filter=D --name-only --all`) — servers
and clients for omniORB, TAO and JacORB over a fixed `interop.idl`,
with the result written up in `docs/perf/corba-cross-orb-interop-2026-06-06.md`.
The reusable part is the ORB plumbing — `ORB.init`, the root POA, the
IOR to a file — and `corba-wire-template.cpp` and `CorbaWire.java` are
that plumbing again, about thirty lines each. The part that could not
be reused is the type: it has to come from `wireprobe-corba-idl` per
corpus case, which is the whole point of the run. One thing the old
harness lacks and this needs: a bounded run. `InteropServer.java` calls
`orb.run()` and is killed by its runner, so it can never say how many
calls arrived — the `done()` operation here exists for that.

Two of that document's findings are reproduced independently by this
run, from a different harness over different types: **JacORB's GIOP is
big-endian and omniORB's and TAO's are little-endian**, and the byte
order lives in the message header's flag byte.

### The files

| file | what it is |
|---|---|
| `wire-golden.sh` | the driver: build, capture, run, extract, check — and `--all`, which is the whole measurement in one command |
| `cases.tsv` | which corpus cases, on which domain, with which control, which inventory rows, and why |
| `reasons.tsv` | why a behaviour has no wire measurement, one line each, under one of three kinds |
| `coverage.py`, `coverage.tsv` | the ledger: every behaviour of the inventory, either its cases or its reason. `coverage.py` exits non-zero if any behaviour has neither, or has both |
| `make-poison.py` | the alternating sample, generated: numeric leaves only, by a bijection |
| `poison/<case>.json` | the alternating sample, per case |
| `cyclone-wire-template.c`, `rti-wire-template.c`, `fastdds-wire-template.cpp`, `opendds-wire-template.cpp` | one DataWriter each |
| `ros-wire-template.cpp` | `rmw_publish` through either RMW |
| `corba-wire-template.cpp`, `CorbaWire.java` | a remote call through omniORB/TAO and through JacORB |
| `extract-rtps.py` | user `DATA` out of a capture, with the encapsulation header, the inline-QoS key hash and the topic from SEDP |
| `extract-giop.py` | GIOP request bodies, with TCP reassembly |
| `check-run.py` | the four checks, and the row |
| `compare-levels.py` | the wire octets against the in-memory record |
| `recheck.sh` | every row again, from the committed frames |
| `refusal-check.py` | a wire refusal the in-memory column does not carry — this harness refusing what the product takes |
| `oracle-check.sh` | `extract-rtps.py` against Wireshark's dissector, over the frames in `zerodds-examples/idl-conformance/proofs/rtps-framing/` |
| `run-log.tsv` | what each run wrote, matched and read — the checks the octets cannot carry |

---

## The fill run

Recorded 2026-08-13, before the corpus run and with a type shaped for
one question: what a peer puts in the alignment fill.

`gapprobe.idl` (DDS) and `gap-corba.idl` (CORBA) declare the same shape:

```idl
struct GapProbe { string s; long b; octet c; double d; octet e; short f; };
```

Three fill spans, at three alignments: 3 octets before `b`, 3 (XCDR2) or
7 (XCDR1/GIOP) before `d`, 1 before `f`. A **long** and a **short**
sample alternate through **one** writer or **one** connection, with
`KeepLast(1)` on the DDS side, so the short sample's fill positions land
on octets the long sample wrote.

| peer | path | version | fill on the wire |
|---|---|---|---|
| RTI Connext | RTPS/UDP, real DataWriter | 7.7.0 | **`00`** — 30 samples XCDR2, 10 XCDR1, every span |
| RTI Connext | `rti::topic::to_cdr_buffer` (static) | 7.7.0 | **untouched** — `0xAA` survives in every span |
| Fast DDS | RTPS/UDP, real DataWriter | 3.6.1 / Fast CDR 2.3.5 | **`00`** — 30 samples each version |
| omniORB | GIOP/TCP | 4.3.3 | **the previous request's octets** |
| TAO | GIOP/TCP | 2.5.24 (ACE 6.5.24) | **the previous request's octets** |
| JacORB | GIOP/TCP, big-endian | 3.9 | **`00`** — 10 calls |

Cyclone DDS and OpenDDS are not in that table: both write the fill in
their own serialiser, with a source citation, so no transport can be the
reason their fill is zero. The corpus run measures them anyway.

The steady-state pairs that make omniORB and TAO decisive — each fill
span carrying the previous call's value from the same body offset:

| call | fill span | content | where it comes from |
|---|---|---|---|
| put *n* | body 25–27 | `03 02 01` | put *n-1*, `b = 0x01020304` |
| put *n* | body 33–39 | `00 00 00 00 00 04 40` | put *n-1*, `d = 2.5` |
| put *n+1* | body 21–23 | `41 41 41` | put *n*, the string body |
| put *n+1* | body 29–31 | `33 22 11` | put *n*, `b = 0x11223344` |

Two further facts fell out of the same runs and are recorded because
they are checked non-existences elsewhere:

- **JacORB's GIOP is big-endian**, TAO's and omniORB's are little-endian
  on x86-64. The GIOP message header's flag byte carries it (§9.4), so
  it is a transport decision and no CDR stream reports it. JacORB
  assigns that byte the constant `0` and has no code that emits the
  other order; `docs/peer-defects.md` carries the line.
- **Fast DDS delivers nothing over the network by default on one host.**
  With data sharing at its default the payload never reaches a socket;
  `data_sharing().off()` on both endpoints is what puts the samples on
  the wire.

### What it decided, in the probes

The measurement is the reason the buffers under `tools/wireprobe/` are
what they are, and the answer is not the same for all five:

| probe | buffer | why |
|---|---|---|
| `rti-template.c` | `memset(BUFFER, 0, …)` before each serialisation | RTI's RTPS layer writes `00`, so a cleared buffer records the wire. Without the clear the four `--matrix` combinations of a case wrote into each other |
| `omniorb-template.cpp` | `0xAA` through `put_octet_array`, then `rewindPtrs` | the wire fill is the previous request, which is not a function of the sample; the marker names the octets omniORB does not determine |
| `tao-template.cpp` | `0xAA` in a probe-owned buffer handed to `TAO_OutputCDR` | the same, and its recorded zeros were a fresh page rather than TAO's |
| `fastdds-template.cpp` | unchanged | `std::vector<char> raw(CAPACITY)` is value-initialised per call, so every combination already starts from `00`, which is also what the wire carries |
| `jacorb-template.java` | unchanged | JacORB zeroes the gap itself in `CDROutputStream.check(int, int)`, so no buffer argument is needed; the wire carries `00` too |

### Its artifacts

The fill run's captures stay on the measurement host under
`/root/gapprobe/`. sha256:

```
223931469f7ab4339a479bea6b1f92293d91867c21292f839eba1c7fa7996838  rti-alt-xcdr2.pcap
92a11e123f76bc95e21d1a23e49fba001cef0fd4bb1bb8471f97d815edfb835f  rti-alt-xcdr1.pcap
aa13ae93720830fcf6c35b295bf96cd769dfb9d2886e999ae792f841d75b37aa  fastdds/fd-xcdr2.pcap
7c414cf973a07162c9ca147da33f0b461d26147b7a050ad8e0fea1b035dec7f9  fastdds/fd-xcdr1.pcap
5dfc7374a33f2757a977b2e25d9a8e78677ca07e18919242efa0e6c267bcac7f  omni/omni-giop.pcap
bd1da75c90884f4483bf3b464e039f977c77f28038748f8d01119d7a9ad94248  tao/tao-giop.pcap
3fa7fc8e11b4f89ebdebca6ca8460c3f3518f10e2354d59108b78118e73128d4  jacorb/jacorb-giop.pcap
```

Peer identity: RTI `libnddscore.so` build id
`NDDSCORE_BUILD_7.7.0.0_20260407T000000Z_RTI_REL`; Fast DDS
`libfastdds.so.3.6.1.0` with `libfastcdr.so.2.3.5`; omniORB Debian
`4.3.3+ds1-1`; `libTAO.so.2.5.24`; `jacorb-3.9.jar`.

Its runners carry absolute `/root/gapprobe/...` paths from that run and
are a record of the measurement rather than a portable driver:
`run-rti.sh`, `run-fastdds.sh`, `run-omniorb.sh`, `run-tao.sh`,
`run-jacorb.sh` with their sources, `rti-static.cxx` for the static
contrast, and `decode-rtps.py` / `decode-giop.py`, which split that
type's payload at its fill spans. `JavaxRmiCorbaStub.java` exists
because JDK 21 dropped `javax.rmi.CORBA` and JacORB 3.9's
`org.jacorb.orb.Reference` extends `javax.rmi.CORBA.Stub`; nothing on
the IIOP path calls into it.

---

## What neither run answers

- **Only these eleven**, only on x86-64, only over loopback.
- **Only what a writer sends.** What a reader accepts is
  `tools/wireprobe`'s read direction and is not measured here.
- **Fragmentation is untested here.** Every case fits one datagram on
  the loopback, so `DATA_FRAG` is parsed by `extract-rtps.py` and never
  exercised. `zerodds-examples/idl-conformance/proofs/rtps-framing/frag/`
  has Cyclone and ZeroDDS `DATA_FRAG` captures and is where that
  question is already asked.
- **Whether a peer zeroes the fill or hands out a zeroed buffer** is not
  distinguished where the alternation applies, and does not need to be:
  what reaches a reader is zero either way.
