# identifier-target-keyword-typename-unannotated

**Construct** — a type name that is a keyword of the generated target language, unannotated

The `identifier-target-keyword-typename` type with its extensibility annotation
removed, and nothing else changed. What the twin isolates is the default
extensibility and not the identifier; what it repeats is the identifier
question, because a generator that spells the type name wrongly spells it
wrongly in both files.

`identifier-target-keywords` reaches every identifier position of issue #14
except one. Its structures are named `type` and `Sample` and its enumeration
`class`, and all three sit inside `module package`, so both C generators
flatten the module path into the name — `package_type`, `package_Sample` — and
the keyword becomes a segment of a longer identifier that no compiler reads as
a keyword. The position that stays unmeasured is a type declared at the
outermost scope, where there is no path to flatten and the name stands alone in
the generated source.

That is this case. `struct static` is legal IDL — `static` is in no list
of OMG IDL 4.2 §7.2.3 — and `static x;` is not a C declaration.

## What the generators do with it

Measured 2026-08-11 on the Linux test host, over this file:

RTI answers, and it answers by naming the spelling itself. `wireprobe-rti`
over this case, before the emitter was corrected:

```text
probe.c:63:30: error: type defaults to 'int' in declaration of 'sample'
probe.c:80:9:  error: implicit declaration of function
               'staticPlugin_get_serialized_sample_max_size'; did you mean
               '_c_staticPlugin_get_serialized_sample_max_size'?
```

`rtiddsgen -language C` renders the type `_c_static` and every symbol derived
from it, so its escaping does cover the type-name position. What did not was
`fill.rs::c_name`, which wrote `static sample;` — and `static sample;` is not a
C declaration, which is the first two lines above.

With `c_name` corrected, RTI compiles and writes our octets:

```text
rti  xcdr1/le  7929edff070000006772c3bcc39f00
rti  xcdr2/be  ffed2979000000076772c3bcc39f00
```

The unannotated twin adds the reading the twin exists for: under XCDR2 RTI puts
a `0000000f` DHEADER in front of the aggregate where the `@final` case has none.

## Only one of the two C columns can be asked

`tools/wireprobe/wireprobe-batch`, which is how Cyclone is measured, puts every
case in a module of its own — `module zd_c4 { … }` — so that one front-end run
covers the whole corpus. That wrapping is exactly the flattening this case
exists to get out from under: inside it the generated name is `zd_c4_static`,
which no compiler reads as a keyword. `tools/wireprobe/wireprobe-rti` copies
the case's IDL verbatim and does not wrap, so RTI sees the declaration at the
outermost scope as written.

The Cyclone cell of this case is therefore a measurement of `zd_c4_static` and
not of `static`, and a green cell there is not an answer to the question. The
line is here because a harness that cannot present a construct must say so
rather than record the pass it gets for not asking.

`crates/wire-probe/src/fill.rs::c_name` joined the scoped name's segments and
applied no keyword list at all, so the probe emitted `static sample;` for both
vendors. The module path had hidden it: every case that reached `c_name` before
this one had at least two segments.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 19 bytes — `0f0000007929edff070000006772c3bcc39f00` |
| `--encoding xcdr2 --endian be` | 19 bytes — `0000000fffed2979000000076772c3bcc39f00` |
| `--encoding xcdr1 --endian le` | 15 bytes — `7929edff070000006772c3bcc39f00` |
| `--encoding xcdr1 --endian be` | 15 bytes — `ffed2979000000076772c3bcc39f00` |

The bytes are what the Rust probe produces today, recorded so that this case is
known to have been run and not merely written. They are not a reference: the
reference is the foreign implementation's own output, which the suite compares
against.
