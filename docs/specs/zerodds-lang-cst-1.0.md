# `zerodds-lang-cst` v1.0 — language CSTs and the rule layer

ZeroDDS vendor spec. Implemented in `crates/lang-cst`, `crates/lang-rules`,
`crates/lang-render` (to be created).

## Motivation

The previous stack had fifteen emitters, each walking a model and writing
text. `idl-rust` alone was 5943 lines. Every rule about how a construct
becomes code existed once per language, in imperative form, inside a
visitor — where it could not be compared with the same rule in another
language, and where a divergence showed up only as different bytes on the
wire.

This spec replaces that with two things the old design did not have:

- A **concrete syntax tree per target language**, so the shape of the
  generated code is a data structure before it is text.
- A **rule layer that is data**, mapping the common AST onto those trees.
  Two languages' treatment of `@optional` is a diff, not a reading of two
  files three thousand lines apart.

The renderer that turns a language CST into text decides nothing. Every
decision sits in a rule and is therefore visible, comparable and
checkable.

## Goals

- **Rules are data, not code.** A rule MUST be a value that can be
  compared, serialised and checked for completeness. A rule MUST NOT be a
  function.
- **Rules are minimal and positive.** Small rules that compose, monotone
  and add-only, with no negative application conditions.
- **Completeness is checkable.** For every construct the common AST can
  hold and every target language, either a rule covers it or the absence
  is a stated, tested gap.
- **The renderer is mechanical.** Given a language CST it produces text
  and makes no choice that could have gone otherwise.
- **Byte-identical to what shipped.** The gate is the output of the
  emitters this replaces.

## Non-goals

- Parsing the target languages. These CSTs are built, not parsed; a
  round trip from generated text back into the CST is not required.
- Formatting policy as an open setting. Each language CST renders in one
  documented style.

## Module layout

```text
crates/
  lang-cst/                     # the trees, no rules, no rendering
    src/
      lib.rs
      common.rs                 # what the four share: Ident, Path, Doc
      rust/mod.rs               # Item, Type, Expr, Attr, Derive
      cpp/mod.rs                # Decl, Type, Template, Namespace
      c/mod.rs                  # Decl, Type, Struct, Typedef
      java/mod.rs               # Decl, Type, Generic, Annotation
      builder.rs                # typed construction, no strings

  lang-rules/                   # common AST to language CST
    src/
      lib.rs
      adapter/
        ast.rs                  # common AST as a seesaw v2 graph
        cst.rs                  # language CST as a seesaw v2 graph
      rule/
        mod.rs                  # the rule catalogue per language
        rust.rs
        cpp.rs
        c.rs
        java.rs
      apply.rs                  # forward application, seesaw v2
      coverage.rs               # the completeness table

  lang-render/                  # language CST to text
    src/
      lib.rs
      rust.rs
      cpp.rs
      c.rs
      java.rs
      writer.rs                 # indentation, line breaking, nothing else
```

## §1 The language CSTs

### §1.1 One tree per language

There is no shared target model. Each language gets the tree its own
syntax needs: Rust has `Item`, `Attr` and `Derive`; C++ has `Namespace`
and `Template`; C has neither and has `Typedef` instead; Java has
`Annotation` and type parameters.

This is a decision against a common target AST. With one, the
language-specific part does not disappear — it moves into the renderer,
which is where it stops being reviewable. Twenty language backends cannot
be built on a model that pretends they are the same.

### §1.2 Concrete, not abstract

The tree MUST record what will be written, not merely what is meant. If
Rust output derives `Debug` and `Clone`, that is two `Derive` nodes in a
recorded order, not a flag the renderer expands. A rule that adds a
derive is then visible as a rule.

### §1.3 Construction is typed

A CST MUST be built through typed constructors. There MUST be no
constructor taking a fragment of target-language text: a string escape
hatch is how language-specific logic leaks back out of the rules.

## §2 The rule layer

### §2.1 Which engine

seesaw's **v2 data model and `v2::rule`**, the positive-only,
NAC-free layer that will be released as seesaw 1.1.0.

The crate carries both rule layers side by side until the cut, so
binding the crate is not enough. Use `seesaw_core::v2::rule`. An import
of `seesaw_core::rule` is the older layer and MUST fail review.

Until 1.1.0 is published the dependency is a path dependency on the
working tree; the crate's own version still reads `1.0.0-rc7` and no
`v1.1.0` tag exists. When it is published this becomes a version
dependency and nothing else changes.

### §2.2 The API this binds to

Verified against the working tree, so the spec names what exists rather
than what it assumes:

| Item | Role here |
|---|---|
| `RuleSetV2`, `RuleV2` | the catalogue and a rule — plain structs, `Serialize`/`Deserialize`, therefore diffable |
| `SideSpec`, `NodeSpec`, `ValueSpec`, `CorrSpec`, `BindingSpec` | L-side, R-side, correspondences, and the invertible value bindings |
| `lower(&RuleV2, &mut Graph) -> [DirectedRule; 2]` | one specification, two directed operationalisations |
| `apply_creation(..)` | the forward, monotone application this spec uses |
| `find_duplicate_rules(&RuleSetV2)` | two rules claiming the same thing, reported by the engine |

`find_duplicate_rules` is the engine's own answer to §4.2's second
requirement; the test asserts on it rather than re-deriving it.

### §2.3 What a rule is

A rule names an L-side pattern over the common AST, an R-side pattern
over a language CST, and the correspondences between them. Following v2:
nodes are positional and typed, links are anonymous and directed, and a
correspondence is an ordinary created node with two links rather than a
special form.

Values do not live in the overlay. A rule that carries a name from a
model node to a CST node binds it through an invertible transform, and
the value is resolved from the original. No value enters any hash, so
renaming a member cannot change the identity of anything.

### §2.4 Minimal and positive

A rule MUST be as small as it can be while still standing alone. One
rule maps a struct to a type declaration; a second maps a member to a
field; a third adds a derive. They compose. A single rule that maps a
struct with all its members and attributes at once is a rule that cannot
be reused for the members of a union.

Rules MUST be positive: they state what must be present, never what must
be absent. Application is monotone and add-only — a rule creates nodes
and links and removes nothing. Where a construct needs a different
result under a condition, that is a leaf constraint on the L-side, not a
negative condition and not a deletion.

### §2.5 Direction

A rule specification lowers to two directed operationalisations. This
spec requires only the **forward** direction: common AST to language
CST. The backward direction is what the engine gives for free and what a
later phase will use to read generated code back; it MUST NOT be
exercised here, so that no rule quietly comes to depend on it.

## §3 Adapters

The common AST and the language CSTs are not seesaw graphs. Two adapters
present them as one.

### §3.1 The model adapter

Every node of the common AST appears as a typed seesaw node; every
reference and every ownership relation appears as an anonymous directed
link. The adapter MUST be total over `NodeKind` — a node kind with no
representation is a construct no rule can ever match, which would be a
silent gap.

Facets appear as nodes too, not as attributes, so a rule can match on
`@key` being present the same way it matches on a member being present.

### §3.2 The CST adapter

The same in the other direction: a rule's R-side creates seesaw nodes,
and the adapter materialises them as language CST nodes. Creation MUST
be total: a seesaw node whose type has no CST counterpart is an error at
apply time, not a node silently dropped.

## §4 Coverage

### §4.1 The table

`coverage.rs` holds a table over `ConstructKind × Language` — the same
21 constructs `zerodds-ast-1.0` §8.2 rules on, across the four
languages. Each cell states either the rule that covers it or that it is
deliberately uncovered, with a reason.

The table MUST have no catch-all arm, so a construct added to the common
AST without a decision here is a compile error rather than a language
backend that silently emits nothing for it.

### §4.2 What a test can show that the table cannot

The table proves a rule exists. Tests MUST additionally show, per
language:

- Applying the catalogue to a model containing every construct produces
  a CST for each, with no construct silently absent.
- Two rules never claim the same construct: applying the catalogue twice
  to the same model yields the same CST, since application is monotone.
- Every rule in the catalogue is reachable — a rule no model can trigger
  is dead and MUST be reported.

## §5 Rendering

A renderer walks a language CST and writes text. It MUST NOT consult the
common AST, MUST NOT branch on anything but the CST node in hand, and
MUST NOT insert a construct that is not in the tree.

Each language renders in one documented style: Rust as `rustfmt` would,
C++ and C in the brace style the previous emitters used, Java in the
Oracle convention. Style is not configurable; a downstream formatter is
the place for that.

The renderer is the one place a `String` appears, and it appears only at
the leaves.

## §6 Conformance

### §6.1 The byte gate

The output MUST be byte-identical to the emitters this replaces, at
`e1f3af895`:

| Language | Reference |
|---|---|
| Rust | `crates/idl-rust` and its fixture outputs |
| C++ | `crates/idl-cpp`, including its C mode |
| C | `crates/idl-cpp` C mode |
| Java | `crates/idl-java` |

Byte-identity is the gate because it is the only claim that cannot be
argued with. Where the new output is deliberately different, the
difference MUST be recorded as a decision with its reason before the
fixture is updated — never by adjusting the expectation to the output.

### §6.2 The wire gate behind it

Byte-identical source is not the point; identical bytes on the wire are.
For every fixture type, the layout plan `zerodds-ast-layout` produces
MUST agree with what the generated code encodes. Source that differs
harmlessly is tolerable; wire that differs is not.

### §6.3 Coverage document

`docs/spec-coverage/zerodds-lang-cst-1.0.md`, bilingual, per §-section
with the code path and the test. Per language, the construct table's
state is reported as counts of covered and deliberately uncovered, with
each uncovered cell named. A count of passing fixtures is not coverage:
fixtures exercise the constructs someone thought to write down.
