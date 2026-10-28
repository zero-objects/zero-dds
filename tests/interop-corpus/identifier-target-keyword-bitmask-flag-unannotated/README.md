# identifier-target-keyword-bitmask-flag-unannotated

**Construct** — bitmask flags that are keywords of the generated target language, unannotated

The `identifier-target-keyword-bitmask-flag` type with its extensibility
annotation removed, and nothing else changed. What the twin isolates is the
default extensibility; what it repeats is the identifier question, because a
generator that spells a flag wrongly spells it wrongly in both files.

The third identifier position, and the one no case in the corpus had reached at
all: XTypes 1.3 §7.5.1.2.2 declares a bitmask's flags as enumerators with
`@value(1 << position)`, so a flag is an identifier in the generated source
exactly as an enumerator is, and it is spelled by a different emitter arm.
`identifier-target-keywords` has no bitmask.

`auto`, `extern` and `inline` are C keywords and are in no list of OMG IDL 4.2
§7.2.3. The bitmask is at the outermost scope for the reason
`identifier-target-keyword-enumerator` gives: a module path would be flattened
into the flag's name and hide the question.

## What the generators do with it

Measured 2026-08-11 on the Linux test host, over this file:

**Neither C column can answer it, and the two cannot for different reasons.**

`rtiddsgen` refuses the case at the front end — `wireprobe-rti: rtiddsgen
failed` — which is the refusal it already gives every one of the corpus's
bitmask cases and has nothing to do with the flag names. Cyclone wrote octets,
and they are ours, but `wireprobe-batch` had wrapped the bitmask in `module
zd_cN` first, so the flag it compiled was `zd_cN_auto`.

So the position is recorded as unmeasured rather than as agreed. The emitter
split it was landed to find is still real and is fixed on the enumerator's
measurement — `fill.rs::emit` went through `fill::identifier` and
`dump.rs::emit_bitmask` pasted `node.name`, so for RTI the two halves of one
translation unit named the flag two ways — but the vendor that would confirm
the flag arm specifically is the one whose front end will not take a bitmask
at all.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `08000000050000000100007f` |
| `--encoding xcdr2 --endian be` | 12 bytes — `00000008050000007f000001` |
| `--encoding xcdr1 --endian le` | 8 bytes — `050000000100007f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `050000007f000001` |

`05` is `auto | inline` — bits 0 and 2 of an eight-bit bound — and the three
octets after it are the padding to the `long`. The bytes are what the Rust
probe produces today, recorded so that this case is known to have been run and
not merely written. They are not a reference: the reference is the foreign
implementation's own output, which the suite compares against.
