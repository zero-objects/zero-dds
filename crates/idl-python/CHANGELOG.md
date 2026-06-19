# Changelog — `zerodds-idl-python`

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [Unreleased]

RC3 preparation. Phase-1 + Phase-2 codegen complete.

### Added — Phase 1

- `struct` with primitives / string / sequence / scoped types.
- `enum` as `IntEnum`.
- `module` with underscore-flattened class names (Python-PSM Annex B).
- `@idl_struct(...)` + `@dataclass` decorator stack via `zerodds.idl`
  brands (`Int32`, `Float64`, `String`, etc.).
- `PythonGenOptions::header_comment` for optional file-level comments.
- Python keyword escaping for field names (`class` → `class_`).

### Added — Phase 2

- `struct` inheritance via Python dataclass subclassing
  (`class Derived(Base):`).
- `bitmask` as `class X(IntFlag)` with `member = 1 << position`.
- `bitset` as `X: TypeAlias = Int64` + helper class with
  `_SHIFT`/`_WIDTH`/`_MASK` constants per named bitfield
  (anonymous padding bitfields are accounted for in the shift counter
  but not emitted).
- `union` via `idl_union(typename=..., discriminator=..., cases={...},
  default=...)`. Switch-type supports: all integer variants,
  Boolean, Octet, Char, Scoped (e.g. IntEnum reference).
- `typedef T name` as `name: TypeAlias = T`, including array and
  sequence aliases.
- `exception` as `@idl_struct(typename=...)` + `@dataclass class
  X(Exception):` — standard Python exception behavior plus
  ZeroDDS wire encoding.
- Const-expression evaluation for union case labels: integer literals
  (decimal, hex, octal), unary `+` / `-` / `~`. Hex and negative
  labels are correctly emitted as signed i64.
- Smoke tests in `tests/smoke.rs` extended from 12 to **26**.

### Phase-3 material (currently `IdlPythonError::Unsupported`)

- `valuetype`, `interface`, `fixed`, `map`, `any`.
- Union cases with scoped const-expressions (e.g. `case ENUM_MEMBER:`)
  — the parser accepts them; the codegen reports a clean Unsupported
  until Phase-3 implements full ConstExpr resolution.
- Union cases with binary const-expressions (`case A | B:`).
