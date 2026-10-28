# recursive-self-sequence

**Construct** — a struct holding a sequence of itself

A struct that reaches itself, directly or through another type. Legal
IDL: a linked list or a tree is ordinary in a data model, and
`sequence<Node>` is how IDL spells one. Legal XTypes: §7.3.1.2.1.5 has
`TypeIdentifier` rules for exactly this.

**ZeroDDS cannot plan it.** `zerodds_ast_layout::plan::MAX_DEPTH` stops
the descent at 64 and reports `type graph below … nests too deeply`. The
cycle goes through a **sequence**, whose length is a run-time value, so
the element's plan is the outer type's plan — what the planner needs is
to notice it has been at this type before and emit a back-reference, not
to keep descending. See docs/OPEN-ITEMS.md.

The case is here **because** this implementation fails it. A corpus that
only holds what its own column passes measures nothing about anyone. The
other four columns are what say whether this is a gap or a shared limit.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000000700000014000000010000000c000000080000000400000000000000` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c0000000700000014000000010000000c000000080000000400000000` |
| `--encoding xcdr1 --endian le` | 16 bytes — `07000000010000000800000000000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000007000000010000000800000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
