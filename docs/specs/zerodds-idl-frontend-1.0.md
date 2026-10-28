# `zerodds-idl-frontend` v1.0 — IDL source to the common AST

ZeroDDS vendor spec. Implemented in `crates/idl-cst`, `crates/idl-parse`,
`crates/idl-lower` (to be created).

## Motivation

The common AST (`docs/specs/zerodds-ast-1.0.md`) is the contract. This
spec covers the first frontend that fills it: OMG IDL 4.2 text in, a
resolved and validated model out.

The frontend is deliberately three crates rather than one. The previous
`crates/idl` mixed a data-driven grammar engine, a CST, a builder, a
preprocessor and eight semantic passes in one place, and the largest
single file was the grammar table at 6883 lines. Splitting the syntax
tree from the parser that builds it, and both from the lowering that
consumes them, is what lets each be tested against something external.

## Goals

- **Lossless syntax.** The CST MUST reproduce its input byte for byte,
  comments and spacing included. A formatter has to be able to correct a
  file rather than normalise it, and the rule layer that maps the common
  AST onto a language CST compares concrete trees.
- **Diagnostics that point at source.** Every error MUST carry a byte
  range and MUST NOT be the first error only: a file with three problems
  reports three.
- **Recovery.** A parse error MUST NOT abandon the file. The parser
  continues at the next declaration boundary so that one bad member does
  not hide every later one.
- **Completion is specification, not guesswork.** Everything the source
  leaves implicit is filled in by a stated rule (§5), never inferred.
- **Vendor dialects without a second grammar.** RTI, OpenDDS, Fast DDS
  and CoreDX extensions are parser options that populate the vendor
  facet; they do not fork the grammar.

## Non-goals

- Code emission. The language CSTs and the rule layer are
  `zerodds-lang-cst-1.0`.
- The XTypes runtime. Assignability, `TryConstruct` and TypeLookup are
  runtime services over the model.
- Grammar as data. This is a hand-written recursive-descent parser by
  decision; a dialect is a flag, not a table.

## Module layout

```text
crates/
  idl-cst/                      # the tree, no parser
    src/
      lib.rs
      token.rs                  # TokenKind, Token, trivia attachment
      lex.rs                    # the lexer
      tree.rs                   # CstNode, CstKind, green/red split
      trivia.rs                 # comments and spacing, kept
      span.rs                   # byte ranges, line/column on demand
      text.rs                   # lossless reconstruction

  idl-parse/                    # source text to CST
    src/
      lib.rs
      parser.rs                 # the recursive-descent driver
      decl.rs                   # module, type, interface, const
      type_spec.rs              # type expressions, the `>>` problem
      annotation.rs             # @ann and #pragma
      expr.rs                   # constant expressions with precedence
      recover.rs                # error recovery points
      dialect.rs                # vendor options
      diagnostic.rs
      preprocess/
        mod.rs                  # #include, #define, conditionals
        macro_.rs               # expansion with a source map

  idl-lower/                    # CST to common AST
    src/
      lib.rs
      lower.rs                  # the walk
      scope.rs                  # scope stack, name binding
      resolve.rs                # scoped-name resolution
      complete.rs               # §5, completion by rule
      validate.rs               # §6, IDL rules the AST cannot express
      diagnostic.rs
```

## §1 Lexing

### §1.1 Tokens carry their trivia

A token MUST carry the trivia that precedes it — comments, spacing,
line breaks — and the file's trailing trivia MUST be attached to the
end-of-file token. No byte of the input may be unreachable from the
token stream.

### §1.2 What the lexer must distinguish

Identifiers and the 60 IDL 4.2 keywords; the escaped identifier form
`_name`, which is the same identifier without the underscore but MUST
record that it was escaped. Integer literals in decimal, octal and
hexadecimal, retaining the written radix. Floating-point and fixed-point
literals retaining their written form (`1.` is not `1.0`). Character,
wide-character, string and wide-string literals with escape sequences
resolved, the raw text retained. Punctuation, including `<<` and `>>`.

The lexer MUST NOT decide whether `>>` is a shift operator or two
closing angle brackets. It emits one `>>` token and the parser splits it
(§2.3).

## §2 The concrete syntax tree

### §2.1 Shape

The CST is a green/red pair: an immutable, shareable green tree of kinds
and widths, and red nodes that carry absolute offsets and parent links,
built on demand. Identical subtrees MAY share their green node.

Every node MUST know its full width in bytes including trivia, so that
`text()` on any node reproduces the input for that range exactly.

### §2.2 Losslessness is a test, not a promise

`parse(source).text() == source` MUST hold for every fixture, including
files that fail to parse: a file with errors still round-trips, because
error recovery inserts error nodes rather than dropping input.

### §2.3 The `>>` problem

`sequence<sequence<long>>` ends in one `>>` token. The parser MUST split
it into two `>` tokens when it closes a type argument list, and MUST NOT
when it appears in a constant expression. The split MUST be visible in
the CST — two `>` nodes covering one source token — so that the round
trip still holds.

## §3 Parsing

### §3.1 Grammar coverage

The parser MUST accept the whole of OMG IDL 4.2 §7.4: the core building
block, plus anonymous types, annotations, extended data types, and the
building blocks for interfaces, values, CCM components, templates and
`import`.

### §3.2 Recovery

On an unexpected token the parser MUST emit a diagnostic, insert an
error node covering the offending range, and resynchronise at the next
`;`, `}` or top-level keyword. It MUST NOT stop.

A file MUST yield every diagnostic it has. Reporting only the first is a
defect: a frontend that stops at the first error makes a large file take
as many runs as it has mistakes.

### §3.3 Ambiguities that MUST be decided in the parser

- `>>` as shift versus two closing brackets (§2.3).
- An annotation application versus a `#pragma` carrying the same
  information; both MUST reach the same facet.
- `::A` at the start of a scoped name — the global scope, never a
  binary operator.
- A declarator that is an array versus one that is a simple name, which
  is only known at the `[`.

### §3.4 Preprocessing

`#include`, `#define`, `#undef`, `#if`/`#ifdef`/`#ifndef`/`#else`/
`#elif`/`#endif`, and the `#pragma` forms IDL uses (`prefix`, `version`,
`ID`, and the vendor ones).

Expansion MUST produce a source map: every token in the expanded stream
MUST resolve to the file and range it textually came from **and** to the
macro chain that produced it. A diagnostic inside a macro expansion MUST
be able to name both.

An `#include` cycle MUST be reported, not followed.

### §3.5 Dialects

`Dialect` is a set of flags, defaulting to strict IDL 4.2. A vendor flag
admits that vendor's extensions and routes them to its facet. A
construct admitted only by a flag MUST record which flag admitted it, so
that a later profile validation against `Profile::Idl42Core` can report
it.

### §3.6 The suffix form of an annotation

DDS-XTypes 1.3 §7.3.1.2.5, Table 24 gives a second production for
applying an annotation and calls the two equivalent:

```text
{ "@"   <annotation_type_name> [ "(" <arguments> ")" ] }*
{ "//@" <annotation_type_name> [ "(" <arguments> ")" ] }*
```

The parser MUST read `//@name` and `//@name(args)` written after a
declaration as the `@name(args)` written before it, and both MUST reach
the same model — the OMG `dds-security` interoperability suite writes
its `ShapeType` key that way and no other. §7.3.1.2.3 admits nothing
between the double slash and the at-sign, so `// @name`, `///@name` and
the block-comment form stay comments.

A `//@` standing where the specification allows no annotation MUST be a
diagnostic naming its range. Reading it as a comment would leave a file
that says `//@key` and a model that says the member is not a key, with
nothing in between.

## §4 Lowering to the common AST

The lowering walks the CST once and builds the model through the
transactional edit surface, so a file that fails validation leaves no
partial model behind.

Module reopening across files converges on one node — that is
`Ast::ensure_module` and the reason the frontend never needs a dedup
pass.

Every node the lowering creates MUST carry a `SourceFacet` naming the
file, the byte range and the macro chain.

## §5 Completion

What the source leaves implicit is filled in here by rule. Nothing is
guessed; each of these is determined by the specification.

| What | Rule | Source |
|---|---|---|
| Member id | `@id(n)`, else `@hashid`, else the enclosing `@autoid(SEQUENTIAL\|HASH)`, else previous member plus one, starting at 0 | XTypes 1.3 §7.3.1.2.1.1 |
| Extensibility | `@final`/`@appendable`/`@mutable`, else `@extensibility(...)`, else the enclosing `#pragma`, else `@appendable` | XTypes 1.3 §7.2.4.4 |
| Enum value | `@value(n)`, else previous plus one, starting at 0 | IDL 4.2 §7.4.13 |
| Bitmask position | `@position(k)`, else previous plus one, starting at 0 | XTypes 1.3 §7.3.1.2.1.10 |
| Bit bound | `@bit_bound(n)`, else 32 for enum and bitmask | XTypes 1.3 §7.3.1.2.1.9 |
| Constant type | the declared type; the expression is checked against it, not used to derive it | IDL 4.2 §7.4.1.4.2 |
| Array dimension | the constant expression, evaluated | IDL 4.2 §7.4.1.4.4 |
| Repository id | `#pragma ID` if present, else derived from the scoped name with `#pragma prefix` and `#pragma version` applied | CORBA 3.3 §10.7 |

Every completed value MUST record its origin, not only its result — the
model carries `MemberIdOrigin` for exactly this reason. A collision
between two members is only diagnosable if it is known whether the id
was declared or derived.

## §6 Validation

The frontend enforces the IDL rules the model cannot express
structurally. The model's own invariants (`zerodds-ast-1.0` §8.1) run at
every commit and are not repeated here.

Required at minimum: an identifier that differs from another in the same
scope only by case is an error, IDL being case-insensitive for collision
purposes but case-preserving otherwise; a scoped name that resolves to
more than one declaration is an error naming all of them; a union
discriminator must be integral, boolean, character or an enum; case
labels must be unique and within the discriminator's range; a
`@key` member must not be `@optional`; an array dimension must be a
positive integer constant; a `fixed<d,s>` must satisfy `0 < d <= 31` and
`0 <= s <= d`; a bounded string bound must be positive; forward
declarations must be defined in the same translation unit.

Every diagnostic MUST carry a byte range, a rule name and the
specification paragraph.

## §7 Diagnostics

```rust
pub struct Diagnostic {
    pub severity: Severity,     // Error | Warning
    pub code: DiagnosticCode,   // stable, documented, greppable
    pub message: String,
    pub primary: Span,
    pub labels: Vec<(Span, String)>,
    pub spec_reference: &'static str,
    pub macro_chain: Vec<MacroExpansion>,
}
```

Codes MUST be stable across releases: a build that greps for a code must
keep working. A diagnostic MUST NOT be emitted without a specification
reference; if no paragraph applies, the rule is ZeroDDS policy and MUST
say so.

## §8 Conformance

The frontend MUST parse every fixture at `e1f3af895`:

| Fixture set | Path |
|---|---|
| The XTypes specification's own IDL | `crates/idl/tests/fixtures/omg/dds_xtypes.idl` |
| The DCPS and Security specification IDL | `crates/idl/tests/fixtures/omg/zerodds_dcps.idl`, `zerodds_security.idl` |
| Cyclone DDS | `crates/idl/tests/fixtures/cyclonedds/` |
| Fast DDS | `crates/idl/tests/fixtures/fastdds/` |
| RTI Connext | `crates/idl/tests/fixtures/rti/` |

These are the gate because nothing in this rebuild produced them. Each
vendor set carries a `LICENSES.md` recording its provenance, which MUST
be preserved when the fixtures are restored.

Parsing is not enough: the lowered model MUST satisfy
`Ast::validate_complete()` for every fixture, and `parse(source).text()`
MUST equal the source byte for byte.

Coverage MUST be demonstrated in `docs/spec-coverage/zerodds-idl-frontend-1.0.md`
per §-section, with the code path and the test for each. An empty cell is
a break, reported as such. Coverage MUST NOT be claimed from a count of
accepted files: a parser that accepts everything and understands nothing
also parses every fixture.
