# recursive-array

**Construct** — a self-reference through an array

The cycle IDL 4.2 §7.4.1.4.4.4.4 does not admit: "an incomplete type can only appear as the element type of a sequence definition". An array has no length on the wire, so a value of this type never terminates — unlike `recursive-self-sequence`, where the sequence's length is the terminator, and unlike `recursive-optional-external`, where an absent member would be if the clause admitted it. Every implementation must refuse the file; the refusal is the measurement, and a depth limit tripping instead of a named refusal is the difference this case makes visible.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | **no** — the layout could not be planned: NodeId(7) closes a type cycle with nothing on it that terminates a value — no sequence, no map and no @optional member — which IDL 4.2 §7.4.1.4.4.4.4 does not admit; XTypes 1.3 §7.3.1.2.1.4 refuses only a cyclic value, which is a different question and is answered at value time |
| `--encoding xcdr2 --endian be` | **no** — the layout could not be planned: NodeId(7) closes a type cycle with nothing on it that terminates a value — no sequence, no map and no @optional member — which IDL 4.2 §7.4.1.4.4.4.4 does not admit; XTypes 1.3 §7.3.1.2.1.4 refuses only a cyclic value, which is a different question and is answered at value time |
| `--encoding xcdr1 --endian le` | **no** — the layout could not be planned: NodeId(7) closes a type cycle with nothing on it that terminates a value — no sequence, no map and no @optional member — which IDL 4.2 §7.4.1.4.4.4.4 does not admit; XTypes 1.3 §7.3.1.2.1.4 refuses only a cyclic value, which is a different question and is answered at value time |
| `--encoding xcdr1 --endian be` | **no** — the layout could not be planned: NodeId(7) closes a type cycle with nothing on it that terminates a value — no sequence, no map and no @optional member — which IDL 4.2 §7.4.1.4.4.4.4 does not admit; XTypes 1.3 §7.3.1.2.1.4 refuses only a cyclic value, which is a different question and is answered at value time |
| `--emit keyhash` | **no** — the layout could not be planned: NodeId(7) closes a type cycle with nothing on it that terminates a value — no sequence, no map and no @optional member — which IDL 4.2 §7.4.1.4.4.4.4 does not admit; XTypes 1.3 §7.3.1.2.1.4 refuses only a cyclic value, which is a different question and is answered at value time |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
