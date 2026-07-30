// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XCDR1 / classic-CDR (PLAIN_CDR + PL_CDR1) parity for the Elixir backend.
//!
//! idl-rust (and idl-cpp/idl-csharp/idl-java/idl-ts, and the sibling
//! runtime-flag backends idl-zig/idl-swift) emit a classic-CDR wire path
//! alongside XCDR2: 8-byte primitive alignment, NO DHEADER for `@appendable`,
//! and a PL_CDR1 parameter list (`[PID][len]` members + `0x3F02` sentinel) for
//! `@mutable`. idl-elixir was XCDR2-only. The generated `Wire` module now
//! carries a runtime `xcdr1` rep flag (a body/member sub-writer inherits it via
//! `subwriter/1`), so one generated type serves both representations; each type
//! gains `marshal_xcdr1/2` + `unmarshal_xcdr1/2` entry points.
//!
//! Byte proof: the PL_CDR1 framing constants and layout are pinned against
//! `zerodds_cdr::xcdr1` — `PID_LIST_END = 0x3F02`, `PID_EXTENDED = 0x3F01`, the
//! `(P+2)/2` pad-to-4, and the 8-vs-4 max alignment. `elixir` is not required to
//! run these (string assertions); a runtime byte round-trip vs the idl-rust
//! goldens is the Linux/CI boundary, as for every other `elixir`-gated test in
//! this crate (`elixir` is not installed in every dev/CI host).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// The shared `Wire` module carries the rep flag and the PL_CDR1 machinery.
#[test]
fn wire_prelude_has_xcdr1_machinery() {
    let e = emit("@final struct S { uint64 a; };");
    // Representation flag + classic-CDR entry constructors.
    assert!(e.contains("xcdr1: false"), "{e}");
    assert!(e.contains("def writer1(endian)"), "{e}");
    assert!(e.contains("def reader1(bin, endian)"), "{e}");
    assert!(e.contains("def subwriter(w)"), "{e}");
    // 8-byte alignment under XCDR1, capped at 4 under XCDR2.
    assert!(e.contains("m = if w.xcdr1, do: 8, else: 4"), "{e}");
    assert!(e.contains("m = if r.xcdr1, do: 8, else: 4"), "{e}");
    // 8-byte primitives request natural alignment 8 (capped at 4 by XCDR2).
    assert!(e.contains("def put_u64(w, v), do: put(w, 8,"), "{e}");
    assert!(e.contains("def put_f64(w, v), do: put(w, 8,"), "{e}");
    // PL_CDR1 framing helpers + the reserved PID constants.
    assert!(e.contains("def put_pl_cdr1_member(w, id, mbody)"), "{e}");
    assert!(e.contains("def put_pl_cdr1_sentinel(w)"), "{e}");
    assert!(e.contains("def read_pl_cdr1_member(r)"), "{e}");
    assert!(e.contains("0x3F01"), "{e}"); // PID_EXTENDED
    assert!(e.contains("0x3F02"), "{e}"); // PID_LIST_END
    // Extended-header threshold + pad-to-4 (matches zerodds_cdr::xcdr1).
    assert!(e.contains("if id >= 0x3F00 or bl > 0xFFFF do"), "{e}");
    assert!(e.contains("pad = rem(4 - rem(bl, 4), 4)"), "{e}");
}

/// Every struct gains classic-CDR entry points beside the XCDR2 ones.
#[test]
fn struct_emits_xcdr1_entry_points() {
    let e = emit("@final struct S { uint32 a; };");
    assert!(
        e.contains("def marshal_xcdr1(%__MODULE__{} = v, endian)"),
        "{e}"
    );
    assert!(e.contains("def unmarshal_xcdr1(bin, endian)"), "{e}");
    assert!(e.contains("Zdgen.Wire.writer1(endian)"), "{e}");
    assert!(e.contains("Zdgen.Wire.reader1(bin, endian)"), "{e}");
}

/// `@appendable` has NO DHEADER under XCDR1 (inline members) but keeps the
/// DHEADER under XCDR2 — a runtime branch on `w.xcdr1` / `r.xcdr1`.
#[test]
fn appendable_suppresses_dheader_under_xcdr1() {
    let e = emit("@appendable struct S { uint32 a; };");
    assert!(e.contains("if w.xcdr1 do"), "{e}");
    // The DHEADER length prefix only appears in the XCDR2 (else) arm.
    assert!(e.contains("|> Zdgen.Wire.put_u32(byte_size(body))"), "{e}");
    // Decode: the DHEADER is skipped only under XCDR2.
    assert!(
        e.contains("{_, r} = if r.xcdr1, do: {0, r}, else: Zdgen.Wire.get_u32(r)"),
        "{e}"
    );
}

/// `@mutable` uses a PL_CDR1 parameter list under XCDR1.
#[test]
fn mutable_uses_pl_cdr1_under_xcdr1() {
    let e = emit("@mutable struct S { @id(3) uint64 a; @id(7) uint32 b; };");
    // Encode: each member framed by put_pl_cdr1_member with its id, then sentinel.
    assert!(
        e.contains("Zdgen.Wire.put_pl_cdr1_member(zw, 3, zdm)"),
        "{e}"
    );
    assert!(
        e.contains("Zdgen.Wire.put_pl_cdr1_member(zw, 7, zdm)"),
        "{e}"
    );
    assert!(e.contains("|> Zdgen.Wire.put_pl_cdr1_sentinel()"), "{e}");
    // Member body built member-relative in an XCDR1 sub-writer (inherits mode).
    assert!(e.contains("Zdgen.Wire.subwriter(zw)"), "{e}");
    // Decode: id-dispatched from the PL_CDR1 map, member-relative reader1.
    assert!(
        e.contains("{zdpl, r} = Zdgen.Wire.read_pl_cdr1_all(r)"),
        "{e}"
    );
    assert!(e.contains("case Map.get(zdpl, 3) do"), "{e}");
    assert!(e.contains("Zdgen.Wire.reader1(zdbody, ze)"), "{e}");
    // The XCDR2 EMHEADER path is still present (LC4 | id).
    assert!(e.contains("Zdgen.Wire.put_u32(0x40000003)"), "{e}");
}

/// `@mutable` + `@optional`: an absent member is simply omitted from the
/// PL_CDR1 list (guarded on the presence flag), and decode derives presence
/// from the id's presence in the member map — the correct omitted-member form.
#[test]
fn mutable_optional_omits_absent_member_under_xcdr1() {
    let e = emit("@mutable struct S { @id(1) uint64 a; @id(2) @optional uint32 b; };");
    assert!(e.contains("if v.b_present do"), "{e}");
    assert!(e.contains("b_present = Map.has_key?(zdpl, 2)"), "{e}");
}

/// Unions carry the XCDR1 machinery too (final/appendable/mutable): the
/// discriminator is PL_CDR1 member id 0, each case its 1-based id.
#[test]
fn union_emits_xcdr1_paths() {
    let e = emit(
        "@mutable union U switch (long) { case 1: uint64 a; case 2: uint32 b; default: octet c; };",
    );
    assert!(
        e.contains("def marshal_xcdr1(%__MODULE__{} = v, endian)"),
        "{e}"
    );
    assert!(e.contains("def unmarshal_xcdr1(bin, endian)"), "{e}");
    // Discriminator is PL_CDR1 member id 0.
    assert!(
        e.contains("Zdgen.Wire.put_pl_cdr1_member(zw, 0, zdm)"),
        "{e}"
    );
    assert!(e.contains("|> Zdgen.Wire.put_pl_cdr1_sentinel()"), "{e}");
}

/// `@appendable` and `@final` unions also gain the classic-CDR entry points and
/// (appendable) the mode-gated DHEADER skip.
#[test]
fn appendable_union_suppresses_dheader_under_xcdr1() {
    let e = emit("@appendable union U switch (long) { case 1: uint64 a; default: uint32 b; };");
    assert!(
        e.contains("def marshal_xcdr1(%__MODULE__{} = v, endian)"),
        "{e}"
    );
    assert!(
        e.contains("{_, r} = if r.xcdr1, do: {0, r}, else: Zdgen.Wire.get_u32(r)"),
        "{e}"
    );
}

/// Bitset/bitmask holders expose the classic-CDR entry points as well.
#[test]
fn holder_emits_xcdr1_entry_points() {
    let e = emit("@bit_bound(16) bitmask Flags { FA, FB };");
    assert!(
        e.contains("def marshal_xcdr1(%__MODULE__{} = v, endian)"),
        "{e}"
    );
    assert!(e.contains("def unmarshal_xcdr1(bin, endian)"), "{e}");
}

/// XCDR2 stays byte-stable: an unannotated (`@appendable`) primitive struct's
/// default `marshal_xcdr` still produces the XCDR2 form — the `writer/1` still
/// defaults the rep flag to `false`, and `put_u64` alignment is unchanged for
/// XCDR2 (`put(w, 8, ..)` caps to 4 under XCDR2).
#[test]
fn xcdr2_default_writer_unchanged() {
    let e = emit("@final struct S { uint64 a; };");
    assert!(
        e.contains("def writer(endian), do: %{buf: <<>>, endian: endian, xcdr1: false}"),
        "{e}"
    );
    // The default marshal_xcdr entry still uses the XCDR2 writer.
    assert!(e.contains("Zdgen.Wire.writer(endian)"), "{e}");
}

/// Real-interpreter proof (Linux/CI boundary): round-trip a rich spec through
/// the generated XCDR1 path — final/appendable/mutable structs, `@optional`,
/// 8-byte fields, nested struct, and a mutable union — and check both LE and BE
/// self-consistency. Gated on `elixir` on PATH (not installed in every host).
#[test]
fn generated_xcdr1_round_trips_at_runtime() {
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP generated_xcdr1_round_trips_at_runtime: `elixir` not on PATH");
        return;
    }
    let mut src = emit(
        "@final struct Inner { uint64 stamp; uint16 kind; };\n\
         @appendable struct Mid { uint32 a; Inner inner; string label; };\n\
         @mutable struct Top {\n\
            @id(1) uint64 big;\n\
            @id(2) @optional uint32 maybe;\n\
            @id(3) Mid mid;\n\
         };\n\
         @mutable union Sel switch (long) { case 1: uint64 a; case 2: Inner b; default: octet c; };\n",
    );
    src.push_str(
        r#"
inner = struct(Zdgen.Inner, stamp: 99, kind: 3)
mid = struct(Zdgen.Mid, a: 7, inner: inner, label: "hi")
top = struct(Zdgen.Top, big: 0x0102030405060708, maybe_present: true, maybe: 42, mid: mid)

for e <- [:little, :big] do
  b1 = Zdgen.Top.marshal_xcdr1(top, e)
  back = Zdgen.Top.unmarshal_xcdr1(b1, e)
  true = back.big == top.big
  true = back.maybe == 42
  true = back.mid.inner.stamp == 99
  true = back.mid.label == "hi"

  sel = struct(Zdgen.Sel, disc: 2, b: inner)
  bs = Zdgen.Sel.marshal_xcdr1(sel, e)
  sb = Zdgen.Sel.unmarshal_xcdr1(bs, e)
  true = sb.disc == 2
  true = sb.b.stamp == 99
end

IO.puts("ok")
"#,
    );
    run_exs("idlelixir_xcdr1", &src);
}

/// Runs an `.exs` script and asserts it prints exactly `ok`.
fn run_exs(tag: &str, src: &str) {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success() && stdout.trim() == "ok",
        "elixir script failed:\nstdout={stdout}\nstderr={stderr}\n--- src ---\n{src}"
    );
}

/// Direct byte proof of the PL_CDR1 `@mutable` framing against
/// `zerodds_cdr::xcdr1`: `@mutable struct S { @id(3) uint32 a; }` with
/// `a = 0xAABBCCDD`, little-endian, must serialize to exactly
/// `[03 00 04 00]` (standard PID header id=3 len=4) + `[DD CC BB AA]` (body) +
/// `[02 3F 00 00]` (PID_LIST_END sentinel) — the same layout
/// `encode_pl_cdr1_member` + `write_pl_cdr1_sentinel` produce. Gated on
/// `elixir` on PATH (Linux/CI boundary).
#[test]
fn mutable_pl_cdr1_bytes_match_xcdr1_spec() {
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP mutable_pl_cdr1_bytes_match_xcdr1_spec: `elixir` not on PATH");
        return;
    }
    let mut src = emit("@mutable struct S { @id(3) uint32 a; };");
    src.push_str(
        r#"
s = struct(Zdgen.S, a: 0xAABBCCDD)
hex = Base.encode16(Zdgen.S.marshal_xcdr1(s, :little), case: :lower)
# [03 00 04 00] PID id=3 len=4 | [dd cc bb aa] body | [02 3f 00 00] sentinel
^hex = "030004 00ddccbbaa023f0000" |> String.replace(" ", "")
IO.puts("ok")
"#,
    );
    run_exs("idlelixir_plcdr1_bytes", &src);
}

/// Direct byte proof of XCDR1 8-byte primitive alignment (vs XCDR2's 4):
/// `@final struct S { uint32 a; uint64 b; }` with `a = 1`, `b = 2`, LE. Under
/// XCDR1 `b` aligns to 8, so there is a 4-byte pad after `a`:
/// `[01 00 00 00][00 00 00 00][02 00 00 00 00 00 00 00]`. Under XCDR2 `b`
/// aligns to 4 (no pad): `[01 00 00 00][02 00 00 00 00 00 00 00]` — proving the
/// two representations differ exactly at the alignment and that XCDR2 is
/// unchanged. Gated on `elixir` (Linux/CI boundary).
#[test]
fn xcdr1_eight_byte_alignment_vs_xcdr2() {
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP xcdr1_eight_byte_alignment_vs_xcdr2: `elixir` not on PATH");
        return;
    }
    let mut src = emit("@final struct S { uint32 a; uint64 b; };");
    src.push_str(
        r#"
s = struct(Zdgen.S, a: 1, b: 2)
x1 = Base.encode16(Zdgen.S.marshal_xcdr1(s, :little), case: :lower)
x2 = Base.encode16(Zdgen.S.marshal_xcdr(s, :little), case: :lower)
# XCDR1: a(4) + 4-byte pad + b(8) = 16 bytes.
^x1 = "01000000" <> "00000000" <> "0200000000000000"
# XCDR2: a(4) + b(8, aligned to 4, no pad) = 12 bytes (byte-stable).
^x2 = "01000000" <> "0200000000000000"
IO.puts("ok")
"#,
    );
    run_exs("idlelixir_xcdr1_align", &src);
}
