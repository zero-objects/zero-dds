# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the `zerodds-idlc` IDL compiler.

### Subcommands

- `zerodds-idlc generate --lang <lang> <input.idl>` — generate code for
  Rust, C, C++, C#, Java, Python, or TypeScript from an OMG IDL 4.2 input.
- `zerodds-idlc check <input.idl>` — parse and validate an IDL file
  without emitting code.
- `zerodds-idlc dump-ast <input.idl>` — emit the parsed AST as JSON for
  external tooling.

### Spec References

- OMG IDL 4.2 (`formal/2018-01-05`) §7 — IDL syntax and semantics
- DDS-XTypes 1.3 (`formal/2024-04-01`) §7.4 — language-mapping rules
- Per-language PSM specs: DDS-PSM-Cxx 1.0, DDS-Java-PSM 1.0,
  ZeroDDS-IDL-Rust 1.0, ZeroDDS-XCDR2-{C,Csharp,Java,Rust,TS} 1.0

### Implementation

Multi-pass compiler: lexer → parser (LALR) → AST → resolver →
type-checker → backend dispatch. Each backend is a separate
`crates/idl-<lang>/` crate; `idlc` orchestrates them.

### Architecture

- Layer: Tools
- Dependencies: `zerodds-idl`, `zerodds-idl-{cpp,csharp,java,rust,ts}`,
  `clap` (CLI)

### Stability

CLI surface is stable for `1.0.x`. New language backends are additive
minor bumps. Generated-code surface follows each backend crate's
own SemVer commitment.
