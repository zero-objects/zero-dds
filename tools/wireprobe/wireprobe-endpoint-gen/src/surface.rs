// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What each hand-written endpoint SDK can put on the wire.
//!
//! One record per binding under `endpoints/`, read off the exported
//! declarations and nothing else. A case the surface cannot express is
//! not a failure of the binding — it is the extent of it — so the reach
//! is decided here, before a byte is written, and reported separately
//! from a divergence.
//!
//! ## The rule this file encodes
//!
//! A leaf of the IDL type is **reachable** when the binding exports one
//! operation that writes that leaf's entire wire form in one call.
//! Composition is allowed only for structure the IDL itself gives — the
//! members of an aggregate, the elements of an array or sequence, the
//! selected arm of a union — and never for a scalar's own framing.
//!
//! The line matters because these surfaces are wide enough to fake
//! almost anything if a generator is allowed to lay bytes itself. A
//! `wstring` is a length word and some code units, and `put_u32` plus
//! `put_u16` would produce them; but which length (octets, code units,
//! with a terminator or without), which unit order and which unit width
//! are policy axes `crates/ast-layout/src/key.rs` carries, and a
//! generator that picked one would be measuring its own answer.
//!
//! That is why this table shrank rather than why the constructs became
//! reachable. `wchar`, `wstring`, `fixed<P,S>` and `bitset` are not
//! composed here now; each one is a call the binding exports and
//! answers — `put_wchar` decides the unit width and the order,
//! `put_wstring` decides what the length counts, `put_fixed` decides
//! the nibble order and the sign nibble, `put_bitset` decides which
//! write the anchor goes out through. The walk checks that the key
//! asked for the answer the binding gives and refuses when it did not,
//! which keeps a second dialect from being reported under this one's
//! name.
//!
//! What *is* composed: the integer widths. Every one of these bindings
//! exports its writers by wire width — `put_u8`, `put_u16`, `put_u32`,
//! `put_u64` — and not by IDL type, so `octet`, `int8` and `uint8` all
//! reach the wire through the same exported call, each writing exactly
//! the octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it. An `enum` and a
//! `bitmask` go out in the holder `zerodds-ast-layout` resolves for
//! them, through the writer of that width.

/// One of the hand-written bindings this probe measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// `endpoints/ada` — Stage 1, `Interfaces.C` imports over
    /// `endpoints/c`.
    Ada,
    /// `endpoints/ada-83` — strict Ada 83, `Long_Integer` arithmetic.
    Ada83,
    /// `endpoints/ada-native` — Stage 2, pure Ada, SPARK subset.
    AdaNative,
    /// `endpoints/d`.
    D,
    /// `endpoints/nim`.
    Nim,
    /// `endpoints/zig`.
    Zig,
    /// `endpoints/kotlin/src/Zerodds.kt`.
    Kotlin,
    /// `endpoints/node/zerodds.js`.
    Node,
    /// `endpoints/fsharp/zerodds.fs`.
    FSharp,
    /// `endpoints/swift/Sources/Zerodds/Zerodds.swift`.
    Swift,
    /// `endpoints/go/wire.go`.
    Go,
    /// `endpoints/julia/zerodds.jl`.
    Julia,
    /// `endpoints/lua/zerodds.lua`.
    Lua,
    /// `endpoints/ocaml/zerodds.ml`.
    OCaml,
    /// `endpoints/elixir/lib/zerodds.ex`.
    Elixir,
    /// `endpoints/python/zerodds_wire.py`.
    Python,
    /// `endpoints/java/Zdw.java`.
    Java,
}

impl Language {
    /// Every one, in the order the report lists them.
    pub const ALL: [Self; 17] = [
        Self::Ada,
        Self::Ada83,
        Self::AdaNative,
        Self::D,
        Self::Nim,
        Self::Zig,
        Self::Kotlin,
        Self::Node,
        Self::FSharp,
        Self::Swift,
        Self::Go,
        Self::Julia,
        Self::Lua,
        Self::OCaml,
        Self::Elixir,
        Self::Python,
        Self::Java,
    ];

    /// The name on the command line and in the ledger directory.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ada => "ada",
            Self::Ada83 => "ada-83",
            Self::AdaNative => "ada-native",
            Self::D => "d",
            Self::Nim => "nim",
            Self::Zig => "zig",
            Self::Kotlin => "kotlin",
            Self::Node => "node",
            Self::FSharp => "fsharp",
            Self::Swift => "swift",
            Self::Go => "go",
            Self::Julia => "julia",
            Self::Lua => "lua",
            Self::OCaml => "ocaml",
            Self::Elixir => "elixir",
            Self::Python => "python",
            Self::Java => "java",
        }
    }

    /// Parse the `--lang` value.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.as_str() == name)
    }

    /// What this binding exports.
    #[must_use]
    pub const fn surface(self) -> Surface {
        match self {
            // `endpoints/ada-83` is strict Ada 83 and has no `Float`
            // wider than 32 bits it can name portably; whether GNAT's
            // `Long_Float` under `-gnat83` is IEEE binary64 is recorded
            // beside the binding rather than assumed here.
            Self::Ada83 => Surface { f64: ADA83_F64 },
            // Every other binding exports a 64-bit float writer.
            _ => Surface { f64: true },
        }
    }
}

/// Whether `endpoints/ada-83` carries a 64-bit float.
///
/// A named constant and not a literal, because it is the one axis of
/// this table that is a measurement of a toolchain rather than a
/// reading of a source file: `-gnat83` predates `Interfaces`, so the
/// binding cannot name `IEEE_Float_64`, and what `Long_Float` is left
/// with is GNAT's.
const ADA83_F64: bool = true;

/// The operations one binding exports, as far as the reach test needs
/// them.
///
/// One field, where there were four. The three that are gone —
/// `dheader`, `emheader` and `max_align_settable` — were the record of
/// a surface that wrote XCDR2 and only XCDR2, and every one of the
/// seventeen now exports the delimiter, the member header in all eight
/// length codes, the PL_CDR1 parameter list and an encoding argument
/// that decides MAXALIGN. What is left in the table is what is still
/// genuinely uneven.
///
/// `long double` is uneven in the other direction and is not a field:
/// no binding exports a 128-bit float and none is planned, so the walk
/// refuses it by name for all seventeen at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    /// A 64-bit float writer. `double` is unreachable without it: the
    /// binding's `put_u64` takes an integer, and handing it the bit
    /// pattern would mean this generator, not the binding, decided that
    /// a `double` goes out as `f64::to_bits` — which is the rule
    /// `endpoints/c/include/zerodds_wire.h` states for `zdw_put_f64`
    /// and which a binding without it never states.
    pub f64: bool,
}
