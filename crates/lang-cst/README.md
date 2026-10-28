# `zerodds-lang-cst`

Concrete syntax trees for the 21 target languages of `Language::ALL`
(`src/common.rs`), one module per language: Rust, C++, C, Java, C#, Python,
TypeScript, Go, OCaml, Ada, D, Elixir, Julia, Lua, Nim, Swift, Zig, Node,
Ada 83, F#, Kotlin.

These trees are **built, never parsed**. A generator produces one and a
renderer writes it out; nothing reads target-language text back in.

Two properties the trees exist for:

- **Concrete, not abstract.** The tree records what will be written. If
  the output derives `Debug` and `Clone`, that is two derive nodes in a
  recorded order — not a flag a renderer expands, because a renderer that
  expands flags is a renderer that makes decisions.
- **Typed construction.** There is no constructor that takes a fragment
  of target-language text. A string escape hatch is how
  language-specific logic leaks back out of the rules and into the
  emitter, which is the shape this rebuild replaced.

There is deliberately no shared target model. Rust has attributes and
derives, C++ has namespaces and templates, C has neither and has
typedefs, Java has annotations and type parameters. A model that
pretended they were the same would push the difference into the renderer,
where it stops being reviewable.

See `docs/specs/zerodds-lang-cst-1.0.md` §1.
