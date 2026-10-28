# `zerodds-idlc`

Read, check and format OMG IDL 4.2, generate language source and codecs,
or DDS 1.2 §8 DLRL object/home classes from the DLRL pragmas.

```
zerodds-idlc check      FILE...   parse and lower; report every diagnostic
zerodds-idlc lint       FILE...   the rules a valid file can still break
zerodds-idlc format     FILE...   the canonical form, to stdout, --check or --write
zerodds-idlc dump-ast   FILE...   the model, as a tree
zerodds-idlc print-deps FILE...   the types each declaration names
zerodds-idlc generate --lang LANGUAGE [--out DIR] FILE...
zerodds-idlc dlrl     --lang LANGUAGE [--out DIR] FILE...
```

The first five commands inspect or format source. `generate` calls the
language backend to emit declarations and encode/decode code. Unsupported
constructs are reported, not silently omitted. An unknown `--lang` value
lists the currently accepted names.

```sh
zerodds-idlc generate --lang rust --out generated schema.idl
```

For Cargo build scripts, use [zerodds-build](../../crates/zerodds-build/README.md).
It adds dependency tracking and optional `DdsType` implementations. Generated
code still needs its language's wire runtime; this command is not a publisher
or subscriber generator.

For C#, generation additionally writes `<stem>.TypeSupport.cs` when the model
contains eligible topic aggregates. The main `<stem>.cs` remains a pure
`ZeroDds.Wire` consumer. Compile only that file for wire-only use. Projects
that glob every generated `.cs` also compile the companion and must reference
`ZeroDDS.Cdr` (and `ZeroDDS` when creating DDS entities).
The CLI uses the runtime's `IdlNameAttribute` and `Float128`, without emitting
duplicate definitions. Generated model/codec source explicitly uses C#'s
nullable-reference-oblivious mode, matching its mutable-field mapping. The
companion uses nullable annotations; this is not a nullable-reference guarantee
for the generated model.

The companion connects the current codec to `IDdsTopicType<T>` and declares
its generated XCDR version explicitly. It refuses a different representation,
decoder refusal status, or trailing bytes. It obtains metadata and key-holder
serialization from the common AST/planner. Types without a lowerable
CompleteTypeObject fail typed registration explicitly instead of silently
falling back to a raw-byte topic. `@nested` aggregates receive no topic support.
Native DDS use requires the matching `typed_wire` C-API entry points so the
advertised representation and extensibility agree with the generated body.
Generated keyed writers also require `zerodds_dw_write_keyed`, which carries
the full 16-byte KeyHash separately from the payload. An old native library
must not silently fall back to payload-derived instance identities. The
[fresh C# consumer](tests/csharp-support/README.md) documents this boundary.

Java similarly adds `ZdTypeSupports.java` beside `ZdTypes.java` and
`ZdCodec.java` for eligible topic aggregates. Compile the first two files for
wire-only use. The companion additionally needs the Java DDS binding and
provides explicit `TopicTypeSupport<T>` instances for its writer/reader APIs.
It declares only the generated representation and supports both byte orders.
It exposes shared-serialized standard TypeInformation when its complete
dependency closure is representable. The Java binding carries it in writer
and reader announcements. This is not a TypeLookup service or proof of remote
type resolution; see the [fresh Java consumer](tests/java-support/README.md).

Python appends `ZD_TOPIC_DESCRIPTORS` to the same generated module. The table
is keyed by the exact IDL type name and refers to the actual generated classes
and codecs. It requires the matching `zerodds_wire.topic_descriptor` module,
not the DDS binding. Its descriptors declare one DDS representation (0 or 2),
support explicit little/big byte order, and provide shared-plan KeyHashes,
actual key-holder bytes and TypeObjects where metadata lowering succeeds.
The bounded holder inverse and its unsupported forms are documented by the
fresh consumer below. Missing metadata explicitly
refuses access. See the [fresh Python consumer](tests/python-support/README.md).
The descriptor encodes a CDR body without its encapsulation header. Passed to
`zerodds.IdlTopic(participant, name, descriptor)`, it creates a typed native
topic: the participant registers its CompleteTypeObject with its dependencies
and announces the type (`IdlTopic.__init__`, `crates/py/python/zerodds/topic.py`;
`create_descriptor_topic`, `crates/py/src/ffi.rs`). This is the only Python
path `IdlTopic` accepts — there is no separate untyped decorator route any
more (`zerodds.idl`/`@idl_struct` were removed, C2(7)). The
[Python binding scope](../../crates/py/README.md#generated-python-and-native-topics)
shows the descriptor path end to end.

## `interface` — DDS-RPC 1.0

`--lang rust` produces a complete DDS-RPC 1.0 binding for every
`interface`: the request/reply topic types of §7.5.1.1 with a real
XCDR2 codec, a typed `<Iface>Client`, a `<Iface>` service trait with
`raises` in every method signature, and a `<Iface>Dispatcher` that
serves it over [`zerodds-rpc`](../../crates/rpc/README.md)'s
`Requester`/`Replier`. `out`/`inout` parameters are `&mut` arguments; a
pure `out` parameter besides the declared return travels back through
the method's `Ok` value. A base interface's operations are reached
through a Rust supertrait (`trait Derived: Base`) and, on the client,
through a composed base client field — DDS-RPC 1.0 §7.5.1.1.8 keeps a
base's topic separate from a derived interface's own, and so does this
binding: serve a hierarchy with one dispatcher per level. The generated
file depends on `zerodds-rpc`, `zerodds-dcps` and `zerodds-wire`; the
preamble says so.

C++, Java, TypeScript, C#, Go, Swift, Nim, Zig, Ada, D, Elixir, Julia,
Lua, OCaml and F# also have complete `interface` bindings —
`--lang cpp`/`java`/`typescript`/`csharp`/`go`/`swift`/`nim`/`zig`/
`ada`/`d`/`elixir`/`julia`/`lua`/`ocaml`/`fsharp` — none of them has a
codec to run, since none of their runtimes are implemented in this
tree: every operation and attribute reaches the output, an attribute
as that language's own idiom for one (a C#/Swift property, a Go/Swift/
D/Nim/Zig `name()`/`setName()`-shaped pair, an Ada/OCaml/Elixir/F#
`get_`/`set_` primitive operation or field, a Julia function stub, a
Lua descriptor entry), and a declared `raises`/`getraises`/
`setraises` reaches it too — as `throws` where the language has the
keyword (Swift), as a trailing `error` result (Go), or documented on
the member DDS-RPC 1.0 §7.11.1.1.2 rule 6.c already keeps it out of
the signature. A base interface's operations and attributes are
flattened into the derived declaration in fourteen of the fifteen —
none of them has a language mechanism this crate composes a base
through instead — Rust's is the one with a native supertrait
(`trait Derived: Base`), above.

No other language has a complete `interface` binding: C, Python and
Node have no mapping for one at all. `generate` refuses a model that
declares one for any of those three `--lang`s, naming the interface,
its position in the source, and which languages do have a binding —
exit `1`, never a partial declaration and never a silent, exit-`0`
answer with the interface missing from the output.

## `dlrl` — DDS 1.2 §8 DLRL

```sh
zerodds-idlc dlrl --lang cpp --out generated schema.idl
```

`dlrl` turns the three DLRL pragmas — `#pragma DCPS_DATA_TYPE`,
`#pragma DCPS_DATA_KEY` and `#pragma DCPS_DLRL_RELATION` — into the
object and home classes an application programs against: one object
class per data type, a getter per key field, an attribute per other
member of the data type, and an accessor per relation. A basic type, an
enumeration or a simple structure (every field a basic type, an
enumeration, a simple structure or a fixed array of those) is a
mono-valued attribute with a getter and a setter; a `sequence` or a
one-dimensional array is a List, a `map` keyed by `string` a StrMap and
one keyed by an integer type an IntMap, each with a getter (DDS 1.2
§8.1.2.2, §8.1.6.3.16–19). An enumeration or a structure is named the
way `idlc generate` declares it for the same language, so the classes
compile beside that output: the C++ header includes `<stem>.hpp`, the
TypeScript file imports from `./<stem>.js`. A member whose type has no
mapping (a union, a nested collection, a structure containing a
collection, a multi-dimensional array, a map with another key type, a
wide string) fails the run with its name and the reason.
`--lang` takes `cpp`, `csharp`, `java` or `typescript` (`cxx`/`cs`/`ts`
aliases accepted, like `generate`'s), or `all` for every one in a run.
C++, C# and TypeScript bundle every type into one `<stem>_dlrl.<ext>`
file; Java writes one `<Type>.java` and one `<Type>Home.java` per type,
since JLS §7.6 allows one `public` top-level type per compilation unit
and both the object class and its home are `public`.

### The pragma-name collision with OpenDDS

OpenDDS's own devguide reuses `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` — the
identical two pragma spellings — for an unrelated, DLRL-free
topic-type/key marker, which `check`/`generate` already read that way.
`dlrl` does not extend that reading with a second, DLRL meaning for the
same two spellings on the AST; it reads the raw IDL text on its own,
through `zerodds_dlrl::pragma::parse_pragma`, independently of
`idl-lower` entirely. Running `check`/`generate` and `dlrl` over the
same file therefore each apply exactly one reading, and neither
silently eats the other's — the diagnostic `check`/`generate` report on
`DCPS_DATA_TYPE`/`DCPS_DATA_KEY` names the other reading and where it
lives, and `DCPS_DLRL_RELATION`, which OpenDDS never defined, gets its
own named diagnostic there rather than falling into the generic
"not a directive this front end interprets" every truly unknown pragma
gets. A pragma that names one of the three DLRL tags and does not
parse under that grammar is a diagnostic and fails the run, not a
silent skip; a pragma outside the three tags is silent under `dlrl`,
whatever it means to `check`/`generate` or to another tool.

### No ZeroDDS DLRL runtime

The four backends emit classes that derive from PSM base types this
tree does not implement: C++'s from `DLRL::ObjectRoot`/`ObjectHome`/
`RefList` and `DDS::Long` (DDS 1.2 §B.6/§B.7, the specification's own
C++ PSM), Java's from `org.omg.dds.dlrl.ObjectRoot`/`ObjectHome`
(§B.8), C#'s from an `ObjectRoot`/`ObjectHome` pair and
`[DlrlKey]`/`[DlrlRelation]` attributes this codebase invented for a
target OMG never gave a PSM (there is no vendor mapping to follow
either). ZeroDDS ships a DLRL *runtime* for exactly one language,
Rust (`crates/dlrl`), and `dlrl-codegen` does not target Rust — the
four languages it does target have no runtime here for the generated
class to compile against, and every generated file's header comment
says so rather than writing an `#include`/`import` for a header or a
package that does not exist. TypeScript is the exception: its
generated `interface` and implementing class name no external type at
all.

## Options

| | |
|---|---|
| `--strict` | OMG IDL 4.2 only; reject every vendor extension |
| `--json` | machine-readable diagnostics |
| `--deny-warnings` | exit non-zero when anything was reported |
| `--default-extensibility final\|appendable\|mutable` | default for unannotated aggregates; default `appendable`, explicit annotations win. Cyclone's `idlc` assumes `final`, RTI, Fast DDS and OpenDDS `appendable` (`docs/wire-axis-flags.md`); IDL written for Cyclone needs `final` |
| `--check` | `format` only: name each non-canonical file, exit 1 |
| `--write` | `format` only: rewrite non-canonical files in place |
| `--lang NAME` | `generate`/`dlrl` only, required: target language — `generate` takes 21 wire languages, `dlrl` takes `cpp`/`csharp`/`java`/`typescript` |
| `--out DIR` | `generate`/`dlrl` only: output directory, default current directory |
| `--encoding ENC` | `generate` only: `xcdr1`, `xcdr2` (default), `pl-cdr1`, `pl-cdr2`, `delimited-cdr` |
| `--endian le\|be` | `generate` only: planned byte order, default `le`; runtimes carry byte order in their reader/writer |
| `--xcdr-endianness BE\|LE` | DDS-TS spelling of `--endian` |
| `--dialect NAME` | `generate` only: reproduce one peer's answers to every wire axis at once, before any axis flag below overrides one of them. Default `specification`, which is what `generate` always emitted; `docs/wire-axis-flags.md` has the other ten and what each answers |
| `--<axis> VALUE` | `generate` only: override one answer of the dialect. One flag per axis in `docs/wire-axis-flags.md`'s table — `--length-codes`, `--pid-flags`, `--wstring-length`, and the rest — each taking one of the values that axis's column lists. Applied after `--dialect`, in the order given |
| `--stamp`/`--depfile`/`--output-manifest FILE` | `generate`/`dlrl` only: written last and only on success (`--stamp`), a Make-format dependency file naming every IDL file read (`--depfile`), JSON of the files read/written (`--output-manifest`) |
| `-h`, `--help` | command reference |
| `-V`, `--version` | tool version |

Files parse under every vendor's extensions unless `--strict` says
otherwise: a tool pointed at a file somebody else wrote should read it
before it judges it. `--strict` answers the other question — would every
vendor accept the same source syntax. It is not a cross-vendor runtime
interoperability guarantee.

`generate` defaults to specification layout rules — `--dialect` and its
axis flags are what turn that into one peer's own reading, the same
table `wireprobe-zerodds` and `zerodds-build`'s `Config::dialect`/
`Config::axis` read. No flag means no change: the preamble names neither
a dialect nor an axis, and the bytes are the ones `generate` always
wrote.

## Exit codes

`0` nothing to report · `1` a diagnostic was reported · `2` the command
line was wrong · `3` a file could not be read.

## What `format` produces

One canonical layout over the lossless token stream: four-space
indentation, one declaration per line, one enumerator per line, at most
one blank line kept where the source had one. Spellings are never
normalised — `0x1F` stays `0x1F`. Comments and preprocessor directives
are part of the stream and are printed; a file whose reformatted output
would not carry every token, comment and directive of the input is
refused by name, never emitted without them. The same goes for a file
that does not parse cleanly: formatting a tree with error recovery in
it would lay out the recovery, not the source. The output is verified
on every run to lex back to the identical stream and to be idempotent.

## What `lint` reports

Six rules, and each is about a choice the specification permits:

- a `@mutable` aggregate above sixteen members, because each of them
  carries an EMHEADER on the wire (XTypes 1.3 §7.4.3.4.2) — at least
  four octets each, twelve where the length code needs the long form;
- a declared name that is a keyword in a language this tree generates
  for, because that collision surfaces as a compiler error in generated
  code rather than here;
- a `@topic` type in which nothing is `@key` — a single-instance topic,
  where a forgotten key looks exactly like an intended singleton;
- a member that is or contains an unbounded `sequence` or `map`, whose
  wire size is whatever the writer sends (unbounded strings are
  deliberately exempt: they are the idiomatic spelling, and a rule that
  fires on most files gets suppressed);
- a `@key` member whose serialized key has no maximum — there the
  string exemption ends, because the key is serialized for every
  keyhash;
- a `typedef` nothing in the unit references.

Deliberately absent: two members that resolve to one id, and a member
that is both `@key` and `@optional`. `check` already reports both as
errors, with a span, from the front end. A lint rule that repeats the
stage before it fires second, without a position, and teaches a reader
that the two disagree about severity when they do not.
