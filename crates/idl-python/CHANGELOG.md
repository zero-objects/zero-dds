# Changelog — `zerodds-idl-python`

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [Unreleased]

RC3-Vorbereitung. Phase-1 + Phase-2-Codegen abgeschlossen.

### Added — Phase 1

- `struct` mit primitives / string / sequence / scoped types.
- `enum` als `IntEnum`.
- `module` mit underscore-flattened Klassennamen (Python-PSM Annex B).
- `@idl_struct(...)` + `@dataclass`-Decorator-Stack ueber `zerodds.idl`-
  Brands (`Int32`, `Float64`, `String`, etc.).
- `PythonGenOptions::header_comment` fuer optionale Datei-Kommentare.
- Python-Keyword-Escape fuer Feldnamen (`class` → `class_`).

### Added — Phase 2

- `struct`-Inheritance via Python-dataclass-Subclassing
  (`class Derived(Base):`).
- `bitmask` als `class X(IntFlag)` mit `member = 1 << position`.
- `bitset` als `X: TypeAlias = Int64` + Helper-Klasse mit
  `_SHIFT`/`_WIDTH`/`_MASK`-Konstanten pro benanntem Bitfield
  (anonyme Padding-Bitfields werden im Shift-Counter beruecksichtigt,
  aber nicht emittiert).
- `union` via `idl_union(typename=..., discriminator=..., cases={...},
  default=...)`. Switch-Type unterstuetzt: alle Integer-Varianten,
  Boolean, Octet, Char, Scoped (z.B. IntEnum-Reference).
- `typedef T name` als `name: TypeAlias = T`, inkl. Array- und
  Sequence-Aliase.
- `exception` als `@idl_struct(typename=...)` + `@dataclass class
  X(Exception):` — Python-Standard-Exception-Behavior plus
  ZeroDDS-Wire-Encoding.
- Const-Expression-Eval fuer Union-Case-Labels: Integer-Literale
  (dezimal, hex, oktal), Unary `+` / `-` / `~`. Hex- und negative-
  Labels werden korrekt als signed-i64 emittiert.
- Smoke-Tests in `tests/smoke.rs` von 12 auf **26** erweitert.

### Phase-3-Material (heute `IdlPythonError::Unsupported`)

- `valuetype`, `interface`, `fixed`, `map`, `any`.
- Union-Cases mit Scoped-Const-Expressions (z.B. `case ENUM_MEMBER:`)
  — der Parser akzeptiert sie, der Codegen meldet sauberen Unsupported
  bis Phase-3 die ConstExpr-Resolution durchzieht.
- Union-Cases mit Binary-Const-Expressions (`case A | B:`).
