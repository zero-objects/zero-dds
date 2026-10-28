# `zerodds-ast` v1.0 — common AST for the ZeroDDS type system

ZeroDDS vendor spec. Implemented in `crates/ast-core`, `crates/ast`,
`crates/ast-layout` (to be created).

## Motivation

Every type-shaped concern in ZeroDDS previously carried its own model: the IDL
parser had `crates/idl/src/ast`, the XTypes runtime had `crates/types`, each of
the fifteen `idl-*` backends re-derived wire layout locally, `crates/xml` parsed
DDS-XML into a private shape, and ROS 2 message definitions were lowered ad hoc.
The same rule — for example the length-code framing of a variable-length
`@mutable` member — therefore existed in as many copies as there were consumers,
and diverged between them.

This spec defines one model that all frontends lower into and all backends read
from. Frontends (IDL parser, type discovery over the wire, `.msg`/`.srv`/
`.action`, DDS-XML, CORBA IDL, derive macros) produce it. Consumers (code
emitters, codecs, the XTypes runtime, linters, formatters, tooling and model
views) consume it. Wire-layout rules are computed in exactly one place.

## Goals

- **Superset, not intersection.** Every construct expressible in IDL 4.2,
  DDS-XTypes 1.3, CORBA 3.3 (including CCM), ROS 2 interface definitions,
  DDS-XML 1.0 and the FlatData/zero-copy model MUST be representable without
  loss.
- **Strictly typed.** No stringly-typed property bags, no `HashMap<String,
  Value>` escape hatch. Source-specific information lives in per-source facets,
  each its own `struct`.
- **Coherent under partial knowledge.** Type discovery delivers types whose
  references cannot yet be resolved. The model MUST remain valid in that state
  rather than requiring a separate "partial" representation.
- **Mutable through a transactional interface** with create/update/delete and
  rollback.
- **Observable.** Every committed mutation emits a changeset describing what was
  created, updated or deleted.
- **Traversable** as a whole, including graphs containing cycles.
- **Persistable.** Nodes carry a stable identity independent of in-memory
  placement, and the whole model serialises deterministically.

## Non-goals

- Parsing. No lexer, grammar or preprocessor is part of this spec.
- Code emission. No target-language rendering.
- Encoding and decoding of sample data. `ast-layout` computes the plan; it does
  not move bytes.
- The XTypes runtime services — assignability (`is_assignable_from`,
  DDS-XTypes 1.3 §7.2.4), `TryConstruct` coercion and the TypeLookup service are
  runtime concerns built on this model, not part of it.

## Module layout

```text
crates/
  ast-core/                     # no DDS knowledge
    src/
      lib.rs
      arena.rs                  # generational arena, typed handles
      id.rs                     # NodeId, IdSpace, allocation
      reference.rs              # Ref<T>, PendingRef, BreakCause
      txn.rs                    # Transaction, undo log, commit
      change.rs                 # Change, Changeset, FieldTag, FieldValue
      observer.rs               # AstObserver, channel adapter
      walk.rs                   # cycle-safe traversal primitives
      index.rs                  # incremental index maintenance
      snapshot.rs               # deterministic serialisation
      error.rs

  ast/                          # the model
    src/
      lib.rs
      model.rs                  # Ast: arenas, indices, façade
      node/
        module.rs
        type_.rs                # TypeNode, TypeKind
        member.rs
        interface.rs            # InterfaceNode, OperationNode, AttributeNode
        exception.rs
        constant.rs             # ConstNode, ConstExpr
        annotation.rs           # AnnotationDefNode
        deployment.rs           # domains, topics, QoS profiles, endpoints
      facet/
        mod.rs
        xtypes.rs
        corba.rs
        ros2.rs
        xml.rs
        flatdata.rs
        vendor.rs               # Rti, OpenDds, FastDds, CoreDx
        source.rs
      edit/                     # the add/remove/update surface
        mod.rs
        spec.rs                 # *Spec input structs
      invariant.rs              # structural invariants (every commit)
      profile.rs                # Profile validation (on demand)
      resolve.rs                # reference resolution transitions
      query.rs                  # name / TypeIdentifier / ROS name lookup

  ast-layout/
    src/
      lib.rs
      key.rs                    # LayoutKey: encoding, endianness, purpose
      plan.rs                   # TypeLayout, MemberPlan
      xcdr1.rs                  # PLAIN_CDR, PL_CDR
      xcdr2.rs                  # PLAIN_CDR2, DELIMITED_CDR, PL_CDR2
      keyhash.rs                # DDSI-RTPS 2.5 §9.6.3.8 key holder
      cache.rs                  # side table, changeset-driven invalidation
```

## §1 Node storage and identity

### §1.1 Arenas and handles

Nodes MUST be stored in per-kind generational arenas. A handle is
`Id<T> { slot: u32, generation: NonZeroU32 }`; handles MUST be distinct types per
node kind so that a member handle cannot be passed where a type handle is
expected.

Deleting a node MUST bump its slot generation. A read through a handle whose
generation no longer matches MUST return `AstError::StaleHandle`, never the node
that later occupied the slot. Silent aliasing after delete is a defect class this
spec exists to prevent.

`Ast` MUST be `Send + Sync`. Interior mutability through `Rc`/`RefCell` is
forbidden.

### §1.2 Stable identity

Every node MUST additionally carry a `NodeId`: a 128-bit value that is stable
across snapshot round-trips and across processes. `NodeId` allocation MUST be
deterministic for a given sequence of mutations, so that two runs producing the
same model produce the same identifiers.

`Id<T>` is the fast in-memory access path. `NodeId` is the persistence and
cross-process path. Conversion in both directions MUST be O(1).

The XTypes `TypeIdentifier` MUST NOT be used as node identity. It is a derived
value, it collapses structurally equal types, and it does not exist for modules,
interfaces, operations, constants or deployment nodes.

## §2 References and resolution state

A reference between nodes MUST be represented as:

```rust
pub enum Ref<T> {
    Resolved(Id<T>),
    Pending(PendingRef),
    Broken(BreakCause),
}

pub enum PendingRef {
    ScopedName(ScopedName),          // IDL frontends, forward declarations
    TypeIdentifier(TypeIdentifier),  // type discovery over the wire
    RosTypeName(RosTypeName),        // pkg/msg/Type
    RepositoryId(RepositoryId),      // CORBA
    XmlRef(XmlRefKey),               // DDS-XML idref
}

pub enum BreakCause {
    NotFound { what: PendingRef },
    Ambiguous { what: PendingRef, candidates: Vec<NodeId> },
    KindMismatch { expected: NodeKind, found: NodeKind },
    IllegalCycle { through: NodeId },
}
```

`Option<Id<T>>` MUST NOT be used to express "not resolved yet" — absence and
unresolvedness are different facts and are both needed.

Resolution is a state transition on an existing node, performed by `ast::resolve`.
It MUST emit an `Updated` change carrying the previous and new reference state.
A reference MUST NOT silently stay `Pending` when resolution failed: the
transition target is `Broken` with a cause.

## §3 Core node space

The core is source-neutral. Anything that exists only because a particular
frontend produced it belongs in a facet (§4), not here.

### §3.1 Modules

`ModuleNode { name, parent: Ref<ModuleNode>, members: Vec<AnyId> }`. Modules
nest. Reopening a module in a second translation unit MUST merge into one node,
not create a second — this is the structural resolution of the per-file
fragmentation that previously required an opt-in dedup pass.

### §3.2 Types

`TypeNode { name, module: Ref<ModuleNode>, kind: TypeKind, facets: Facets }`.

`TypeKind` MUST cover:

| Kind | Payload |
|---|---|
| `Primitive` | `boolean`, `octet`, `char`, `wchar`, `int8`, `uint8`, `int16`, `uint16`, `int32`, `uint32`, `int64`, `uint64`, `float32`, `float64`, `float128` |
| `Fixed` | `digits: u8, scale: u8` (IDL 4.2 `fixed<d,s>`) |
| `String` | `width: Char \| WChar`, `bound: Bound` |
| `Sequence` | `element: Ref<TypeNode>, bound: Bound` |
| `Array` | `element: Ref<TypeNode>, dims: Vec<ConstExprId>` |
| `Map` | `key: Ref<TypeNode>, value: Ref<TypeNode>, bound: Bound` |
| `Structure` | `base: Ref<TypeNode>, members: Vec<Id<MemberNode>>` |
| `Union` | `discriminator: Ref<TypeNode>, cases: Vec<Id<CaseNode>>, default_case: Option<Id<CaseNode>>` |
| `Enumeration` | `enumerators: Vec<Id<EnumeratorNode>>` |
| `Bitmask` | `bits: Vec<Id<BitValueNode>>` |
| `Bitset` | `base: Ref<TypeNode>, fields: Vec<Id<BitfieldNode>>` |
| `Alias` | `aliased: Ref<TypeNode>` (typedef) |
| `Valuetype` | `kind: Regular \| Abstract \| Custom \| Truncatable, bases, state: Vec<Id<MemberNode>>, initializers, supported_interfaces` |
| `ValueBox` | `boxed: Ref<TypeNode>` |
| `Native` | opaque, name only |
| `ObjectRef` | `interface: Ref<InterfaceNode>` |
| `AnnotationType` | `definition: Id<AnnotationDefNode>` |

`Bound` is `Unbounded | Const(Id<ConstExprNode>)`. A bound MUST retain the
expression that produced it, not only its evaluated value.

`float128` is a kind the model MUST be able to carry, independently of whether
any target language can currently represent it. The model describes the type; it
does not evaluate it. The known Rust `f128` gap therefore constrains backends,
not this spec.

Forward declarations do not get their own kind. A forward-declared struct, union,
interface or valuetype is a node whose kind payload is present but whose body is
marked incomplete; completing it is an update, so handles taken before the
definition arrives stay valid.

### §3.3 Members and aggregate parts

- `MemberNode { name, type_: Ref<TypeNode>, parent, facets }` — struct fields,
  valuetype state members, union case members, annotation members.
- `CaseNode { labels: Vec<CaseLabel>, member: Id<MemberNode> }` where
  `CaseLabel` is `Value(ConstExprId) | Default`.
- `EnumeratorNode { name, value: Option<ConstExprId>, facets }` — an explicit
  `@value` MUST be distinguishable from a positional value.
- `BitValueNode { name, position: Option<u16>, facets }` — likewise for
  `@position`.
- `BitfieldNode { name, bits: u16, holder_type: Option<Ref<TypeNode>>, facets }`.

### §3.4 Interfaces and operations

- `InterfaceNode { name, kind, bases: Vec<Ref<InterfaceNode>>, exports }`, where
  `kind` is `Interface | Local | Abstract | Component | Home | Connector |
  PortType`.
- `OperationNode { name, return_type: Ref<TypeNode>, params: Vec<ParamNode>,
  raises: Vec<Ref<ExceptionNode>>, oneway: bool, contexts: Vec<String> }`.
- `ParamNode { name, direction: In | Out | InOut, type_: Ref<TypeNode> }`.
- `AttributeNode { name, type_, readonly: bool, get_raises, set_raises }`.
- `ExceptionNode { name, members: Vec<Id<MemberNode>> }`.

Component-model specifics (facets, receptacles, event sources and sinks, home
factories and finders, port types, connectors, template module parameters and
instantiations) are carried as `CorbaFacet` payload on these nodes (§4.2), not as
separate core kinds.

### §3.5 Constants and expressions

`ConstNode { name, type_: Ref<TypeNode>, expr: Id<ConstExprNode> }`.

Expression nodes live in their own arena. `ConstExprNode` is a tree node —
literal, scoped-name reference, unary operator, or binary operator with its
operand handles — so an expression referenced from a bound, an array dimension,
a case label, a `@default` or an enumerator value is a first-class node with its
own `NodeId` and its own `SourceFacet`.

The tree MUST be retained, not collapsed to an evaluated value. Evaluation is available as a derived query. Without the tree, no
consumer can render the constant back to source in its original form, and the
`format` transpiler and the linter both require that.

### §3.6 Annotation definitions

`AnnotationDefNode { name, members: Vec<AnnotationMemberNode> }`, where a member
carries its type, optional default expression, and whether it is the implicit
`value` member. Built-in annotations MUST be represented as ordinary definitions
in a builtin module, so that user-defined and built-in annotations traverse
identically.

### §3.7 Deployment nodes

`DomainNode`, `TopicNode`, `QosProfileNode`, `ParticipantNode`, `EndpointNode`
(reader and writer), `LibraryNode`. A `QosProfileNode` holds typed policy values,
not XML text, and MAY inherit from another profile through a `Ref`.

## §4 Facets

Each node carries a `Facets` value. Every facet is a distinct `struct`; none is a
key-value bag.

```rust
pub struct Facets {
    pub source:   SourceFacet,             // always present
    pub xtypes:   Option<XTypesFacet>,
    pub corba:    Option<CorbaFacet>,
    pub ros2:     Option<Ros2Facet>,
    pub xml:      Option<XmlFacet>,
    pub flatdata: Option<FlatDataFacet>,
    pub vendor:   VendorFacets,
}
```

### §4.1 `XTypesFacet`

Extensibility (`Final | Appendable | Mutable`), `@key`, `@optional`,
`@external`, `@must_understand`, `@non_serialized`, `@try_construct`
(`Discard | UseDefault | Trim`), member id assignment (`@id`, `@hashid`,
`@autoid(SEQUENTIAL|HASH)` and the resulting effective id), `@bit_bound`,
`@default`, `@range`/`@min`/`@max`, `@unit`, `@verbatim` (with language and
placement), `@nested`, `@default_literal`.

The **effective** member id MUST be stored alongside its origin (declared,
inherited from `@autoid`, or previous-plus-one), because the origin is needed to
diagnose collisions and the effective value is needed for layout.

### §4.2 `CorbaFacet`

RepositoryId and its origin (`#pragma prefix`, `#pragma version`, `#pragma ID`,
or derived), valuetype modifiers, `context` clauses, component-model role
(facet, receptacle, event source/sink, home factory/finder, port type, connector),
template-module parameters and instantiation arguments, `native` mapping hints.

### §4.3 `Ros2Facet`

Default value expression, bounded-string and bounded-array origin as written in
the `.msg` file, role within a service or action triple
(`Request | Response | Goal | Result | Feedback`), the ROS type name and its
mapping to the IDL scoped name, and whether a declaration was a ROS constant
rather than a field.

### §4.4 `XmlFacet`

Originating DDS-XML element and attribute set, `idref` targets before resolution,
XSD type binding, and the profile/library nesting the element appeared in.

### §4.5 `FlatDataFacet`

Whether the type qualifies as plain (fixed size, no indirection), the alignment
requirement imposed by the zero-copy loan path, and whether the frontend
requested `@final` fixed-size layout explicitly.

### §4.6 `VendorFacets`

One `Option<T>` per vendor with its own struct: `Rti`, `OpenDds`, `FastDds`,
`CoreDx`. Vendor-specific annotations and pragmas MUST NOT be normalised into
core constructs, because doing so loses the information needed to round-trip
back to that vendor's dialect.

### §4.7 `SourceFacet`

File, byte span, the preprocessor macro expansion chain that produced the
construct, and attached documentation comments. Always present; a node created
programmatically records a synthetic origin rather than none.

## §5 Mutation, transactions and events

### §5.1 Transactions

```rust
let mut tx = ast.begin();
let sid = tx.add_structure(module, StructureSpec { .. })?;
tx.add_member(sid, MemberSpec { .. })?;
let changeset = tx.commit()?;
```

Every mutation MUST run inside a transaction. A mutating call made directly on
`&mut Ast` MUST open a single-operation transaction and commit it, so the two
paths cannot diverge.

`commit` MUST evaluate all structural invariants (§8.1). On violation it MUST
roll back through the undo log and return the violation; the model after a failed
commit MUST be byte-identical to the model before `begin`.

### §5.2 Changesets

```rust
pub enum Change {
    Created { node: NodeId, kind: NodeKind },
    Updated { node: NodeId, field: FieldTag, before: FieldValue, after: FieldValue },
    Deleted { node: NodeId, kind: NodeKind },
}
pub struct Changeset { pub changes: Vec<Change>, pub sequence: u64 }
```

`FieldTag` MUST be a typed enum per node kind, not a string. Changes MUST be
ordered so that replaying a changeset onto the prior model reproduces the
committed model exactly.

Deleting a node MUST cascade to its owned children and MUST convert incoming
references into `Broken { IllegalCycle | NotFound }` — never leave a dangling
`Resolved` handle.

### §5.3 Observers

```rust
pub trait AstObserver: Send + Sync {
    fn on_commit(&self, changeset: &Changeset);
}
```

Observers are invoked synchronously after a successful commit, never during one,
so an observer always sees a valid model. A channel adapter is provided for
asynchronous consumers. Polling for changes is not a supported pattern.

The layout cache (§7) and the indices (§6) are ordinary observers; they hold no
privileged path.

## §6 Traversal and query

`Visitor` and `VisitorMut` traits with default walking implementations. Walks
MUST be cycle-safe: recursive types (a struct containing `sequence<Self>`, mutual
recursion through aliases) are legal and MUST terminate.

Iterator views MUST be provided for the common shapes: all types, members of an
aggregate, cases of a union, exports of an interface, transitive descendants of
any node.

Indices maintained incrementally from the changeset:

| Index | Key |
|---|---|
| by scoped name | `ScopedName` |
| by type identifier | `TypeIdentifier` (MINIMAL and COMPLETE) |
| by ROS type name | `pkg/msg/Type` |
| by repository id | `RepositoryId` |
| unresolved references | `PendingRef` → the nodes waiting on it |

The last index is what lets type discovery resolve a newly arrived type in one
step instead of rescanning the model.

## §7 Layout view

```rust
pub struct LayoutKey {
    pub encoding:   Encoding,     // PlainCdr1 | PlCdr1 | PlainCdr2 | DelimitedCdr | PlCdr2
    pub endianness: Endianness,   // Little | Big
    pub purpose:    Purpose,      // Data | Key
}

pub struct TypeLayout {
    pub align:         u8,
    pub extent:        Extent,    // Sized(u32) | Unbounded
    pub needs_dheader: bool,
    pub members:       Vec<MemberPlan>,
}

pub struct MemberPlan {
    pub member:  NodeId,
    pub header:  MemberHeader,    // NoHeader | PidCdr1 { pid, extended } | EmHeader { lc, must_understand }
    pub offset:  Offset,          // Fixed(u32) | Dynamic
    pub align:   u8,
}
```

Layout MUST be computed from the model, never stored on core nodes, and MUST be
cached in a side table keyed by `(TypeNode, LayoutKey)`. The cache MUST be
invalidated from the changeset, per affected type and its dependents, not
wholesale.

Member offsets are invariant under endianness; the cache MUST share offset
computation between `Little` and `Big` and use the endianness only where it
genuinely matters — the big-endian key holder of DDSI-RTPS 2.5 §9.6.3.8 and the
per-primitive byte order.

### §7.1 Length-code policy

XTypes 1.3 §7.4.3.4.2 permits two framings for a `@mutable` member, and both
occur in the field. They produce different bytes, so the choice MUST be an
explicit part of the layout key rather than a hidden default.

- **Compact** uses LC5, LC6 and LC7 where the specification allows: the member's
  own leading length word doubles as the `NEXTINT`.
- **Universal NEXTINT** uses LC4 throughout, writing an explicit `NEXTINT` ahead
  of every member whatever its size.

**Compact is the default**, because it is what the other implementations emit.
Verified in their sources:

| Implementation | Evidence |
|---|---|
| Fast DDS | `CdrSizeCalculator.hpp` joins the NEXTINT with the following length word (`calculated_size -= 4`); `get_serialized_member_size` maps onto LC5 / LC6 / LC7 / LC4 |
| Cyclone DDS | `dds_cdrstream.c::get_length_code` returns `LENGTH_CODE_ALSO_NEXTINT` for strings, commented "nextint overlaps with length from serialized string data" |
| RTI Connext | selection lives in the binaries; the 7.7.0 headers do not export it, so it is evidence in neither direction |

Universal NEXTINT remains reachable as a configuration flag, because the
emitters this rebuild replaces produced it and the OMG wire vector V-10 at
`e1f3af895` is pinned in that shape. A deployment MUST be able to select it
without recompiling, and an unrecognised value for the setting MUST be rejected
rather than falling back to the default: a typo in a wire-affecting option must
be loud.

Sequence framing follows the element type, not the sequence's own size. A
one-byte element gives LC5 because the item count is the byte count; a two-byte
element gives LC4, because the count is neither the byte count nor backed by a
DHEADER; four- and eight-byte elements give LC6 and LC7; a non-primitive element
gives LC5 through the sequence's own DHEADER. A rule based on size alone gets
the two-byte case wrong.

Length-code selection MUST be computed in the layout view and only there.

`ast-layout` MUST NOT encode or decode sample data. It produces the plan; codecs
execute it.

## §8 Validation

### §8.1 Structural invariants — every commit

- Every `Resolved` reference points at a live node of the expected kind.
- Every node except roots has exactly one owning parent, and ownership is acyclic.
- Array dimensions evaluate to a positive integer.
- Union case labels are type-compatible with the discriminator, when the
  discriminator is resolved.
- Member names are unique within their aggregate; enumerator and bit-value names
  are unique within their type.
- Effective member ids are unique within their aggregate.

### §8.2 Profile validation — on demand

```rust
pub enum Profile { Idl42Core, XTypes13, Corba33, Ros2, DdsXml, FlatData, Vendor(VendorKind) }
ast.validate(Profile::Corba33) -> Result<(), Vec<ProfileViolation>>
```

The same model may be valid under one profile and invalid under another — `map<>`
exists in XTypes but not in CORBA 3.3; ROS 2 forbids constructs that IDL 4.2
permits. A profile violation MUST be reported with the node, the specification
paragraph and the reason. It MUST NOT be silently normalised away.

Completeness rules (no `Pending` references remain, every forward declaration has
a definition) belong to profile validation, not to §8.1, so that a partially
discovered model is not treated as broken.

## §9 Snapshot

The whole model MUST serialise to a binary snapshot and load back such that:

- every `NodeId` is preserved,
- the loaded model compares equal to the original,
- serialising the loaded model yields a byte-identical file.

Determinism is required so that a snapshot hash can key a build cache and so that
diffs between two runs reflect model differences rather than iteration order.

Snapshot loading MUST validate structurally (§8.1) and MUST reject malformed
input with a typed error rather than panicking. It is a fuzz target.

## §10 Error model

All fallible operations return `Result`. The public API MUST contain no
`panic!`, `unwrap` or `expect` on reachable paths. Errors are typed enums
carrying the offending `NodeId` and, where available, the `SourceFacet` span, so
that a diagnostic can point at the original file position.

## §11 Coverage obligation

Completeness of the superset MUST be demonstrated in
`docs/spec-coverage/zerodds-ast-1.0.md` (bilingual pair). Each row is one
specification feature with its paragraph reference; each row states whether it is
carried by a core node, a facet or the layout view, with the implementing code
path and the test that exercises it.

Source specifications for the inventory: IDL 4.2, DDS-XTypes 1.3, DDS-DCPS 1.4,
DDSI-RTPS 2.5, DDS-XML 1.0, DDS-RPC 1.0, CORBA 3.3, CCM 4.0, the ROS 2 interface
definition rules, and the OMG language PSMs (IDL4-C++, IDL4-Java, IDL4-C#,
DDS-PSM-CXX, DDS-Java-PSM).

An empty cell is an open break, reported as such. Coverage MUST NOT be claimed
from a count of grep hits or from tests that compare the implementation against
itself.
