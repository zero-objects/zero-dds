// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The call sequence, as a program in each binding's own language.
//!
//! One file per binding, calling only what `endpoints/<lang>` exports
//! and writing the octets to `out.bin` beside the program. Nothing here
//! aligns, pads, counts a length or reverses a byte — every one of those
//! happens inside the binding when the program runs, which is what makes
//! the run a measurement rather than a rendering of this generator's
//! opinion.
//!
//! ## Why the byte order is compiled in and not an argument
//!
//! Because `endpoints/ada-83` is strict Ada 83, where there is no
//! `Ada.Command_Line` — child units are an Ada 95 addition and `-gnat83`
//! rejects them. One shape for all seventeen beats sixteen that take an
//! argument and one that cannot, and the probe caches on the generated
//! source, so two configurations whose program is the same text compile
//! once.
//!
//! ## Why the encoding reaches the constructor
//!
//! XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1 and four under
//! XCDR2. Every one of the seventeen writers now takes the version beside
//! the byte order and there is no default, so [`Program::version`] is
//! rendered into the constructor call and nothing here decides which
//! alignment cap the stream is written under.
//!
//! ## The framing locals
//!
//! A DHEADER, a member header and a PL_CDR1 parameter header are all
//! opened, filled and closed against a value the opening call handed
//! back — a body start or a parameter handle. They nest, so one local
//! per *open* header is what a renderer declares, numbered from the
//! outside in and released when the header closes. `framing` resolves
//! the numbering once for the whole program so that the closing call
//! names the local its opening call took, in every one of the seventeen.

use crate::ops::{Form, Int, Op, Program, Version};
use crate::surface::Language;
use std::fmt::Write as _;

/// Render the program for one binding.
/// # Errors
/// The name of an operation the language has no call for, or a string
/// in the sample whose octets are not UTF-8 where the language's string
/// literal is measured in characters. The walk refuses an unreachable
/// operation before a renderer ever sees one, so a refusal here is the
/// reach test and the renderers disagreeing — a defect of this
/// generator, reported as one (exit 7) rather than a panic.
pub fn render(
    lang: Language,
    program: &Program,
    big_endian: bool,
    form: Form,
) -> Result<String, String> {
    match lang {
        Language::Zig => zig(program, big_endian, form),
        Language::Nim => nim(program, big_endian, form),
        Language::D => d(program, big_endian, form),
        Language::Ada => ada(program, big_endian, form),
        Language::Ada83 => ada83(program, big_endian, form),
        Language::AdaNative => ada_native(program, big_endian, form),
        Language::Kotlin => kotlin(program, big_endian, form),
        Language::Node => node(program, big_endian, form),
        Language::FSharp => fsharp(program, big_endian, form),
        Language::Swift => swift(program, big_endian, form),
        Language::Go => go(program, big_endian, form),
        Language::Julia => julia(program, big_endian, form),
        Language::Lua => lua(program, big_endian, form),
        Language::OCaml => ocaml(program, big_endian, form),
        Language::Elixir => elixir(program, big_endian, form),
        Language::Python => python(program, big_endian, form),
        Language::Java => java(program, big_endian, form),
    }
}

/// The file name the rendered source takes.
#[must_use]
pub const fn file_name(lang: Language) -> &'static str {
    match lang {
        Language::Zig => "probe.zig",
        Language::Nim => "probe.nim",
        Language::D => "probe.d",
        Language::Ada | Language::Ada83 | Language::AdaNative => "probe.adb",
        Language::Kotlin => "Probe.kt",
        Language::Node => "probe.js",
        Language::FSharp => "probe.fsx",
        Language::Swift => "probe.swift",
        Language::Go => "probe.go",
        Language::Julia => "probe.jl",
        Language::Lua => "probe.lua",
        Language::OCaml => "probe.ml",
        Language::Elixir => "probe.exs",
        Language::Python => "probe.py",
        Language::Java => "Probe.java",
    }
}

/// How deep the framing nests, so a renderer that needs a local per
/// open header knows how many to declare.
///
/// All three kinds of open header count: a DHEADER, a member header and
/// a PL_CDR1 parameter header each hold a value from the moment they
/// open until the moment they close, and a `@mutable` struct whose
/// member is itself an `@appendable` struct has two of them live at
/// once.
fn depth(program: &Program) -> usize {
    let mut deepest = 0;
    let mut open = 0usize;
    for op in &program.ops {
        match op {
            Op::DHeaderBegin
            | Op::EmHeaderBegin { .. }
            | Op::PidBegin { .. }
            | Op::PidExtendedBegin { .. } => {
                open += 1;
                deepest = deepest.max(open);
            }
            Op::DHeaderEnd | Op::EmHeaderEnd { .. } | Op::PidEnd => open = open.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

/// The framing locals a program needs, and which one each framing
/// operation names.
///
/// Numbered from the outside in and released on close, so a program of
/// forty sequential members declares one local and not forty, and a
/// nested one declares exactly as many as it has live at once.
struct Frames {
    /// The local each operation opens or closes, by operation index.
    /// Meaningless for an operation that is not a framing one.
    slot: Vec<usize>,
    /// The numbers that need a body-start local: a DHEADER's or a
    /// member header's.
    marks: Vec<usize>,
    /// The numbers that need a parameter-handle local. A different
    /// type in every one of the seventeen, which is why the two sets are
    /// kept apart rather than one local being made to carry both.
    pids: Vec<usize>,
}

impl Frames {
    /// The local the operation at `index` names.
    fn at(&self, index: usize) -> usize {
        self.slot.get(index).copied().unwrap_or(0)
    }
}

/// Resolve the framing locals for one program.
fn framing(program: &Program) -> Frames {
    let deepest = depth(program);
    // (needs a body-start local, needs a parameter-handle local). A
    // number can be both across a program: the fifth member of a struct
    // may be a parameter where the fourth was a delimiter.
    let mut kind = vec![(false, false); deepest];
    let mut slot = vec![0usize; program.ops.len()];
    let mut open: Vec<usize> = Vec::new();
    let mut next = 0usize;
    for (index, op) in program.ops.iter().enumerate() {
        match op {
            Op::DHeaderBegin
            | Op::EmHeaderBegin { .. }
            | Op::PidBegin { .. }
            | Op::PidExtendedBegin { .. } => {
                let at = next;
                next += 1;
                open.push(at);
                if let Some(entry) = slot.get_mut(index) {
                    *entry = at;
                }
                if let Some(entry) = kind.get_mut(at) {
                    if matches!(op, Op::PidBegin { .. } | Op::PidExtendedBegin { .. }) {
                        entry.1 = true;
                    } else {
                        entry.0 = true;
                    }
                }
            }
            Op::DHeaderEnd | Op::EmHeaderEnd { .. } | Op::PidEnd => {
                let at = open.pop().unwrap_or(0);
                next = at;
                if let Some(entry) = slot.get_mut(index) {
                    *entry = at;
                }
            }
            _ => {}
        }
    }
    let marks = (0..deepest)
        .filter(|at| kind.get(*at).is_some_and(|entry| entry.0))
        .collect();
    let pids = (0..deepest)
        .filter(|at| kind.get(*at).is_some_and(|entry| entry.1))
        .collect();
    Frames { slot, marks, pids }
}

/// A real literal every one of the seventeen front ends parses the same
/// way: a shortest round-trip mantissa, a decimal point even when the
/// value is integral, and an uppercase exponent. Ada rejects `4e0` — a
/// real literal needs the point — and that is the only form Rust's
/// `{:e}` would otherwise produce for a whole number.
fn normalise(text: &str) -> String {
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let mantissa = if mantissa.contains('.') {
        mantissa.to_string()
    } else {
        format!("{mantissa}.0")
    };
    format!("{mantissa}E{exponent}")
}

fn real(value: f64) -> String {
    normalise(&format!("{value:e}"))
}

/// The same, for a binary32: formatted at its own width, so the literal
/// is the shortest decimal that reads back as this `float` and not the
/// longer one its widening to binary64 would need.
fn real32(value: f32) -> String {
    normalise(&format!("{value:e}"))
}

/// The length code, as the plain-integer argument the binding's call
/// takes.
///
/// The bindings whose member header takes the code as a number use this:
/// the integer is checked against 0..7 inside the binding. The three that
/// declare a `LengthCode` enumeration — Zig, Nim and Swift — take
/// [`lc_enum`] instead, because an integer is a type error in each of
/// them, and the four Ada/D bindings take [`lc_name`].
const fn code(lc: crate::ops::Lc) -> u8 {
    lc.index()
}

/// The length code as the `Length_Code` enumeration literal the three
/// Ada bindings declare — `LC0` through `LC7`. An integer here is a
/// type error under GNAT, which is the point of the enumeration.
const fn lc_name(lc: crate::ops::Lc) -> &'static str {
    match lc.index() {
        0 => "LC0",
        1 => "LC1",
        2 => "LC2",
        3 => "LC3",
        4 => "LC4",
        5 => "LC5",
        6 => "LC6",
        _ => "LC7",
    }
}

/// The length code as the enumeration literal the Zig, Nim and Swift
/// bindings declare: `lc0` through `lc7`, which the two that place the
/// enum in a type scope spell `.lc0`..`.lc7`.
const fn lc_enum(lc: crate::ops::Lc) -> &'static str {
    match lc.index() {
        0 => "lc0",
        1 => "lc1",
        2 => "lc2",
        3 => "lc3",
        4 => "lc4",
        5 => "lc5",
        6 => "lc6",
        _ => "lc7",
    }
}

// -- Zig ----------------------------------------------------------------

fn zig(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "// Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/zig/src/zerodds.zig when this runs.\n\
         // Nothing here decides a wire question.\n\n\
         const std = @import(\"std\");\n\
         const zd = @import(\"zerodds.zig\");\n\n\
         pub fn main(zd_init: std.process.Init) !void {\n\
         \x20   const io = zd_init.io;\n\
         \x20   var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);\n\
         \x20   defer arena.deinit();\n\
         \x20   var w = zd.Writer.init(arena.allocator(), ",
    );
    out.push_str(if big_endian { ".big" } else { ".little" });
    out.push_str(", ");
    out.push_str(match program.version {
        Version::Xcdr1 => ".xcdr1",
        Version::Xcdr2 => ".xcdr2",
    });
    out.push_str(");\n    defer w.deinit();\n");
    for at in &frames.marks {
        let _ = writeln!(out, "    var b{at}: usize = 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "    var p{at}: zd.Writer.Pid = undefined;");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("    try w.putU8({});\n", v.text(form)),
            Op::Bool(v) => format!("    try w.putBool({v});\n"),
            Op::Char(v) => format!("    try w.putChar({});\n", v.text(form)),
            Op::Wchar(v) => format!("    try w.putWchar({});\n", v.text(form)),
            Op::U16(v) => format!("    try w.putU16({});\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("    try w.putU32({});\n", v.text(form)),
            Op::U64(v) => format!("    try w.putU64({});\n", v.text(form)),
            Op::F32(v) => format!("    try w.putF32({});\n", float_text(&v.text)),
            Op::F64(v) => format!("    try w.putF64({});\n", real(*v)),
            Op::Str(bytes) => format!("    try w.putString(\"{}\");\n", zig_escape(bytes)),
            Op::SeqU8(bytes) => format!("    try w.putSeqU8(\"{}\");\n", zig_escape(bytes)),
            Op::Wstr(units) => format!("    try w.putWstring({});\n", zig_units(units)),
            Op::Bitset { bits, anchor } => {
                format!("    try w.putBitset({bits}, {anchor});\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "    try w.putFixed(\"{}\", {digits}, {scale});\n",
                zig_escape(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("    b{at} = try w.dheaderBegin();\n"),
            Op::DHeaderEnd => format!("    w.dheaderEnd(b{at});\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "    b{at} = try w.emHeaderBegin({id}, {must_understand}, .{});\n",
                lc_enum(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("    try w.emHeaderEnd(b{at}, .{});\n", lc_enum(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("    p{at} = try w.pidBegin({pid}, {must_understand});\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("    p{at} = try w.pidExtendedBegin({member_id}, {must_understand});\n"),
            Op::PidEnd => format!("    try w.pidEnd(p{at});\n"),
            Op::PidListEnd => "    try w.pidListEnd();\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "    // Zig 0.16: the filesystem is behind `std.Io`; `main` gets the\n\
         \x20   // `Io` through `std.process.Init`, as endpoints/zig's examples do.\n\
         \x20   const file = try std.Io.Dir.cwd().createFile(io, \"out.bin\", .{});\n\
         \x20   defer file.close(io);\n\
         \x20   try file.writeStreamingAll(io, w.bytes());\n\
         }\n",
    );
    Ok(out)
}

fn zig_escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        let _ = write!(out, "\\x{byte:02x}");
    }
    out
}

/// A `[]const u16` of the code units. An empty one still names the
/// element type: Zig infers nothing from no elements.
fn zig_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "&[_]u16{}".to_string();
    }
    let items = units
        .iter()
        .map(|unit| format!("0x{unit:04x}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[_]u16{{ {items} }}")
}

// -- Nim ----------------------------------------------------------------

fn nim(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "# Generated by wireprobe-endpoint-gen. Do not edit.\n\
         #\n\
         # The octets come out of endpoints/nim/zerodds.nim when this runs.\n\
         # Nothing here decides a wire question.\n\n\
         import zerodds\n\n\
         proc emit() =\n\
         \x20 var w = initWriter(",
    );
    out.push_str(if big_endian { "eBE" } else { "eLE" });
    out.push_str(", ");
    out.push_str(match program.version {
        Version::Xcdr1 => "eXcdr1",
        Version::Xcdr2 => "eXcdr2",
    });
    out.push_str(")\n");
    for at in &frames.marks {
        let _ = writeln!(out, "  var b{at} = 0");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "  var p{at}: Pid");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("  w.putU8({})\n", v.text(form)),
            Op::Bool(v) => format!("  w.putBool({v})\n"),
            Op::Char(v) => format!("  w.putChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("  w.putWchar({})\n", v.text(form)),
            Op::U16(v) => format!("  w.putU16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("  w.putU32({}'u32)\n", v.text(form)),
            Op::U64(v) => format!("  w.putU64({}'u64)\n", v.text(form)),
            Op::F32(v) => format!("  w.putF32(float32({}))\n", float_text(&v.text)),
            Op::F64(v) => format!("  w.putF64({})\n", real(*v)),
            Op::Str(bytes) => format!("  w.putString(\"{}\")\n", nim_escape(bytes)),
            Op::SeqU8(bytes) => format!("  w.putSeqU8({})\n", nim_bytes(bytes)),
            Op::Wstr(units) => format!("  w.putWstring({})\n", nim_units(units)),
            Op::Bitset { bits, anchor } => {
                format!("  w.putBitset({bits}'u64, {anchor})\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "  w.putFixed(\"{}\", {digits}, {scale})\n",
                nim_escape(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("  b{at} = w.dheaderBegin()\n"),
            Op::DHeaderEnd => format!("  w.dheaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "  b{at} = w.emHeaderBegin({id}'u32, {must_understand}, {})\n",
                lc_enum(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("  w.emHeaderEnd(b{at}, {})\n", lc_enum(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("  p{at} = w.pidBegin({pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("  p{at} = w.pidExtendedBegin({member_id}'u32, {must_understand})\n"),
            Op::PidEnd => format!("  w.pidEnd(p{at})\n"),
            Op::PidListEnd => "  w.pidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "  let octets = w.bytes()\n\
         \x20 let f = open(\"out.bin\", fmWrite)\n\
         \x20 if octets.len > 0:\n\
         \x20   discard f.writeBuffer(unsafeAddr octets[0], octets.len)\n\
         \x20 f.close()\n\n\
         emit()\n",
    );
    Ok(out)
}

fn nim_escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        let _ = write!(out, "\\x{byte:02X}");
    }
    out
}

fn nim_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "newSeq[byte]()".to_string();
    }
    let items: Vec<String> = bytes.iter().map(|byte| format!("{byte}'u8")).collect();
    format!("@[{}]", items.join(", "))
}

/// The code units as an `openArray[uint16]`. An empty array literal has
/// no element to take a type from, so the empty case is a typed `seq`.
fn nim_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "newSeq[uint16]()".to_string();
    }
    let items: Vec<String> = units.iter().map(|unit| format!("{unit}'u16")).collect();
    format!("[{}]", items.join(", "))
}

// -- D ------------------------------------------------------------------

fn d(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "// Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/d/zerodds.d when this runs.\n\
         // Nothing here decides a wire question.\n\n\
         import std.stdio;\n\
         import zerodds;\n\n\
         void main() {\n\
         \x20   auto w = Writer(",
    );
    out.push_str(if big_endian { "Endian.BE" } else { "Endian.LE" });
    out.push_str(", ");
    out.push_str(match program.version {
        Version::Xcdr1 => "Encoding.XCDR1",
        Version::Xcdr2 => "Encoding.XCDR2",
    });
    out.push_str(");\n");
    for at in &frames.marks {
        // `dheaderBegin` and `emHeaderBegin` answer `int` in this SDK.
        let _ = writeln!(out, "    int b{at} = 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "    Pid p{at};");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("    w.putU8({});\n", v.text(form)),
            Op::Bool(v) => format!("    w.putBool({v});\n"),
            Op::Char(v) => format!("    w.putChar({});\n", v.text(form)),
            Op::Wchar(v) => format!("    w.putWchar({});\n", v.text(form)),
            Op::U16(v) => format!("    w.putU16({});\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("    w.putU32({}u);\n", v.text(form)),
            Op::U64(v) => format!("    w.putU64({}UL);\n", v.text(form)),
            Op::F32(v) => format!("    w.putF32(cast(float) {});\n", float_text(&v.text)),
            Op::F64(v) => format!("    w.putF64({});\n", real(*v)),
            // `cast(string)` of an immutable octet array, not a string
            // literal: D validates the UTF-8 of a literal, and the point
            // of the probe is to hand the binding the octets the sample
            // holds whatever they are.
            Op::Str(bytes) => format!("    w.putString(cast(string) {});\n", d_bytes(bytes)),
            Op::SeqU8(bytes) => format!("    w.putSeqU8({});\n", d_bytes(bytes)),
            Op::Wstr(units) => format!("    w.putWstring({});\n", d_units(units)),
            Op::Bitset { bits, anchor } => {
                format!("    w.putBitset({bits}UL, {anchor});\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "    w.putFixed(cast(string) {}, {digits}, {scale});\n",
                d_bytes(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("    b{at} = w.dheaderBegin();\n"),
            Op::DHeaderEnd => format!("    w.dheaderEnd(b{at});\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "    b{at} = w.emHeaderBegin({id}, {must_understand}, LengthCode.{});\n",
                lc_name(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("    w.emHeaderEnd(b{at}, LengthCode.{});\n", lc_name(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("    p{at} = w.pidBegin({pid}, {must_understand});\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("    p{at} = w.pidExtendedBegin({member_id}, {must_understand});\n"),
            Op::PidEnd => format!("    w.pidEnd(p{at});\n"),
            Op::PidListEnd => "    w.pidListEnd();\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "    auto f = File(\"out.bin\", \"wb\");\n\
         \x20   f.rawWrite(w.bytes());\n\
         \x20   f.close();\n\
         }\n",
    );
    Ok(out)
}

fn d_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "cast(immutable(ubyte)[]) []".to_string();
    }
    let items: Vec<String> = bytes
        .iter()
        .map(|byte| format!("cast(immutable ubyte) {byte}"))
        .collect();
    format!("[{}]", items.join(", "))
}

/// A `const(ushort)[]` of the code units. `null` for the empty one:
/// an empty array literal has no element type in D either, and `null`
/// is the slice of length zero every `const(T)[]` parameter takes.
fn d_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "null".to_string();
    }
    let items: Vec<String> = units
        .iter()
        .map(|unit| format!("cast(ushort) {unit}"))
        .collect();
    format!("[{}]", items.join(", "))
}

// -- Ada, Stage 1 (Interfaces.C over endpoints/c) -----------------------

fn ada(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    // Every string, octet run and unit run is hoisted into the
    // declarative part, because `zdw_put_string` and `zdw_put_seq_u8`
    // take addresses and an aggregate has none; `Put_Wstring` takes a
    // `Code_Unit_Array` and the constant keeps the body readable.
    let mut decls = String::new();
    let mut body = String::new();
    let mut constant = 0usize;
    // `-gnatwae` makes an unreferenced `with` an error, so `System` is
    // named only when an empty `sequence<octet>` needs its null address,
    // and `Zdw_Xcdr` only when an operation of the framing half runs.
    let mut null_address = false;
    let mut xcdr = false;
    // A negative real literal resolves against the parameter's type, and
    // the unary minus of `Interfaces.C.C_float` / `Interfaces.C.double`
    // is not directly visible without a `use type`.
    let mut negative_f32 = false;
    let mut negative_f64 = false;
    // The slot of the open PL_CDR1 parameter. §7.4.1.2 aligns a value
    // against its block and the C writer has no origin field, so
    // `Pid_Begin` caps the writer's alignment at one and every boundary
    // inside the block is `Align_Block`'s — written here before each
    // aligned value, exactly as `test_ada_surface.adb` writes it.
    let mut open_pid: Option<usize> = None;
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        if let Some(pid) = open_pid {
            // The value's natural alignment; `Align_Block` caps it at
            // the encoding's, which the parameter saved.
            let align = match op {
                Op::U16(_) | Op::Wchar(_) => 2,
                Op::U32(_)
                | Op::Count(_)
                | Op::F32(_)
                | Op::Str(_)
                | Op::SeqU8(_)
                | Op::Wstr(_) => 4,
                Op::U64(_) | Op::F64(_) => 8,
                Op::Bitset { anchor, .. } => usize::from(*anchor),
                _ => 1,
            };
            if align > 1 {
                let _ = writeln!(body, "      Align_Block (W'Access, P{pid}, {align});");
            }
        }
        match op {
            Op::U8(v) => {
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_U8 (W'Access, {}));",
                    v.text(form)
                );
            }
            Op::Bool(v) => {
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_Bool (W'Access, {}));",
                    i32::from(*v)
                );
            }
            Op::Char(v) => {
                xcdr = true;
                let _ = writeln!(body, "      Put_Char (W'Access, {});", v.text(form));
            }
            Op::Wchar(v) => {
                xcdr = true;
                let _ = writeln!(body, "      Put_Wchar (W'Access, {});", v.text(form));
            }
            Op::U16(v) => {
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_U16 (W'Access, {}));",
                    v.text(form)
                );
            }
            Op::U32(v) | Op::Count(v) => {
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_U32 (W'Access, {}));",
                    v.text(form)
                );
            }
            Op::U64(v) => {
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_U64 (W'Access, (Hi => {}, Lo => {})));",
                    v.bits >> 32,
                    v.bits & 0xffff_ffff
                );
            }
            Op::F32(v) => {
                let literal = normalise(&v.text);
                negative_f32 |= literal.starts_with('-');
                let _ = writeln!(body, "      Check (Zdw.Put_F32 (W'Access, {literal}));");
            }
            Op::F64(v) => {
                negative_f64 |= v.is_sign_negative();
                let _ = writeln!(body, "      Check (Zdw.Put_F64 (W'Access, {}));", real(*v));
            }
            Op::Str(bytes) => {
                let name = format!("S{constant}");
                constant += 1;
                let _ = writeln!(decls, "   {name} : {}", ada_c_string(bytes));
                let _ = writeln!(
                    body,
                    "      Check (Zdw.Put_String (W'Access, {name}'Address));"
                );
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => {
                // The decimal is ASCII by construction, so it is an Ada
                // string literal; `Put_Fixed` takes a `String` and
                // raises rather than returning a code.
                xcdr = true;
                let _ = writeln!(
                    body,
                    "      Put_Fixed (W'Access, \"{value}\", {digits}, {scale});"
                );
            }
            Op::Bitset { bits, anchor } => {
                xcdr = true;
                let _ = writeln!(
                    body,
                    "      Put_Bitset (W'Access, (Hi => {}, Lo => {}), {anchor});",
                    bits >> 32,
                    bits & 0xffff_ffff
                );
            }
            Op::Wstr(units) => {
                xcdr = true;
                if units.is_empty() {
                    let _ = writeln!(body, "      Put_Wstring (W'Access, Empty_Units);");
                } else {
                    let name = format!("U{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : constant Code_Unit_Array (0 .. {}) := [{}];",
                        units.len() - 1,
                        units
                            .iter()
                            .map(|unit| format!("{unit}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(body, "      Put_Wstring (W'Access, {name});");
                }
            }
            Op::SeqU8(bytes) => {
                if bytes.is_empty() {
                    null_address = true;
                    let _ = writeln!(
                        body,
                        "      Check (Zdw.Put_Seq_U8 (W'Access, System.Null_Address, 0));"
                    );
                } else {
                    let name = format!("D{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : aliased constant Octets (0 .. {}) := [{}];",
                        bytes.len() - 1,
                        bytes
                            .iter()
                            .map(|byte| format!("{byte}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(
                        body,
                        "      Check (Zdw.Put_Seq_U8 (W'Access, {name}'Address, {}));",
                        bytes.len()
                    );
                }
            }
            Op::DHeaderBegin => {
                let _ = writeln!(body, "      B{at} := Zdw.DHeader_Begin (W'Access);");
            }
            Op::DHeaderEnd => {
                let _ = writeln!(body, "      Check (Zdw.DHeader_End (W'Access, B{at}));");
            }
            // The LC-aware member header is `Zdw_Xcdr`'s: the C core's
            // own `zdw_emheader_begin` writes LC4 and nothing else.
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => {
                xcdr = true;
                let _ = writeln!(
                    body,
                    "      EMHeader_Begin (W'Access, {id}, {}, {}, B{at});",
                    ada_bool(*must_understand),
                    lc_name(*lc)
                );
            }
            Op::EmHeaderEnd { lc } => {
                xcdr = true;
                let _ = writeln!(
                    body,
                    "      EMHeader_End (W'Access, B{at}, {});",
                    lc_name(*lc)
                );
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => {
                xcdr = true;
                open_pid = Some(at);
                let _ = writeln!(
                    body,
                    "      Pid_Begin (W'Access, {pid}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => {
                xcdr = true;
                open_pid = Some(at);
                let _ = writeln!(
                    body,
                    "      Pid_Extended_Begin (W'Access, {member_id}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidEnd => {
                xcdr = true;
                open_pid = None;
                let _ = writeln!(body, "      Pid_End (W'Access, P{at});");
            }
            Op::PidListEnd => {
                xcdr = true;
                let _ = writeln!(body, "      Pid_List_End (W'Access);");
            }
        }
    }
    let order = if big_endian { "ZDW_BE" } else { "ZDW_LE" };
    // The C writer is initialised for XCDR2; the cap is a field of the
    // writer record and XCDR1 is one assignment, which is how
    // `test_ada_surface.adb`'s `Start` writes it too.
    let cap = match program.version {
        Version::Xcdr1 => "ZDW_XCDR1_MAX_ALIGN",
        Version::Xcdr2 => "ZDW_XCDR2_MAX_ALIGN",
    };
    let system = if null_address { "with System;\n" } else { "" };
    let with_xcdr = if xcdr { "with Zdw_Xcdr;\n" } else { "" };
    let use_xcdr = if xcdr { "\x20  use Zdw_Xcdr;\n" } else { "" };
    let empty_units = if program
        .ops
        .iter()
        .any(|op| matches!(op, Op::Wstr(units) if units.is_empty()))
    {
        "\x20  Empty_Units : constant Code_Unit_Array (1 .. 0) := [others => 0];\n"
    } else {
        ""
    };
    let mut slots = String::new();
    for at in &frames.marks {
        let _ = writeln!(slots, "   B{at} : Interfaces.C.size_t := 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(slots, "   P{at} : Pid_Header;");
    }
    let mut float_ops = String::new();
    if negative_f32 {
        float_ops.push_str("   use type Interfaces.C.C_float;\n");
    }
    if negative_f64 {
        float_ops.push_str("   use type Interfaces.C.double;\n");
    }
    Ok(format!(
        "--  Generated by wireprobe-endpoint-gen. Do not edit.\n\
         --\n\
         --  The octets come out of endpoints/c through endpoints/ada/src/zdw.ads;\n\
         --  Zdw_Xcdr decides the framing the C core has no operation for. Nothing\n\
         --  here decides a wire question.\n\n\
         with Ada.Streams.Stream_IO;\n\
         with Interfaces.C;\n\
         {system}\
         with Zdw;\n\
         {with_xcdr}\n\
         procedure Probe is\n\
         {use_xcdr}\
         \x20  --  `Check` compares two `Interfaces.C.int`s, and the operator is\n\
         \x20  --  not directly visible without this.\n\
         \x20  use type Interfaces.C.int;\n\
         {float_ops}\n\
         \x20  type Octets is array (Natural range <>) of aliased Interfaces.C.unsigned_char;\n\
         \x20  pragma Convention (C, Octets);\n\n\
         \x20  Buf  : aliased Octets (0 .. 65535) := [others => 0];\n\
         \x20  W    : aliased Zdw.Zdw_Writer;\n\
         \x20  File : Ada.Streams.Stream_IO.File_Type;\n\
         {empty_units}\
         {slots}\
         {decls}\n\
         \x20  --  A writer that overran would report short octets as a divergence,\n\
         \x20  --  so a refused call stops the program instead. Called on the\n\
         \x20  --  sticky field at the end too, so a case with no members still\n\
         \x20  --  references it and -gnatwu has nothing to say.\n\
         \x20  procedure Check (Code : Interfaces.C.int) is\n\
         \x20  begin\n\
         \x20     if Code /= Zdw.ZDW_OK then\n\
         \x20        raise Program_Error;\n\
         \x20     end if;\n\
         \x20  end Check;\n\
         begin\n\
         \x20  Zdw.Writer_Init (W'Access, Buf'Address, Buf'Length, Zdw.{order});\n\
         \x20  W.Max_Align := Zdw.{cap};\n\
         {body}\
         \x20  Check (W.Error);\n\
         \x20  Ada.Streams.Stream_IO.Create\n\
         \x20    (File, Ada.Streams.Stream_IO.Out_File, \"out.bin\");\n\
         \x20  declare\n\
         \x20     Octet_Count : constant Natural := Natural (W.Len);\n\
         \x20     Payload : Ada.Streams.Stream_Element_Array\n\
         \x20       (1 .. Ada.Streams.Stream_Element_Offset (Octet_Count));\n\
         \x20  begin\n\
         \x20     for I in 1 .. Octet_Count loop\n\
         \x20        Payload (Ada.Streams.Stream_Element_Offset (I)) :=\n\
         \x20          Ada.Streams.Stream_Element (Buf (I - 1));\n\
         \x20     end loop;\n\
         \x20     Ada.Streams.Stream_IO.Write (File, Payload);\n\
         \x20  end;\n\
         \x20  Ada.Streams.Stream_IO.Close (File);\n\
         end Probe;\n"
    ))
}

/// A NUL-terminated `Interfaces.C.char_array`, one `char'Val` per
/// octet: the C entry points take a `const char *` and the octets are
/// the sample's, whatever encoding they are in.
fn ada_c_string(bytes: &[u8]) -> String {
    format!(
        "aliased constant Interfaces.C.char_array := [{}Interfaces.C.nul];",
        bytes
            .iter()
            .map(|byte| format!("Interfaces.C.char'Val ({byte}), "))
            .collect::<String>()
    )
}

/// An Ada `String` constant of `bytes`, as a positional-by-index
/// aggregate. `Character'Val` and not a literal, for the reason the C
/// one gives.
fn ada_string(name: &str, bytes: &[u8]) -> String {
    format!(
        "   {name} : constant String (1 .. {}) := ({});\n",
        bytes.len(),
        bytes
            .iter()
            .enumerate()
            .map(|(index, byte)| format!("{} => Character'Val ({byte})", index + 1))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

// -- Ada 83 -------------------------------------------------------------

fn ada83(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut decls = String::new();
    let mut body = String::new();
    let mut constant = 0usize;
    // `-gnatwae` makes an unreferenced constant an error, so the null
    // octet run and the null unit run are declared only where one is
    // written.
    let mut empty_seq = false;
    let mut empty_units = false;
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        match op {
            Op::U8(v) => {
                let _ = writeln!(body, "   Put_U8 (W, {});", v.text(form));
            }
            Op::Bool(v) => {
                let _ = writeln!(body, "   Put_Bool (W, {});", ada_bool(*v));
            }
            Op::Char(v) => {
                let _ = writeln!(body, "   Put_Char (W, {});", v.text(form));
            }
            Op::Wchar(v) => {
                let _ = writeln!(body, "   Put_Wchar (W, {});", v.text(form));
            }
            Op::U16(v) => {
                let _ = writeln!(body, "   Put_U16 (W, {});", v.text(form));
            }
            Op::U32(v) | Op::Count(v) => {
                let _ = writeln!(body, "   Put_U32 (W, {});", v.text(form));
            }
            Op::U64(v) => {
                let _ = writeln!(
                    body,
                    "   Put_U64 (W, {}, {});",
                    v.bits >> 32,
                    v.bits & 0xffff_ffff
                );
            }
            Op::F32(v) => {
                let _ = writeln!(body, "   Put_F32 (W, {});", normalise(&v.text));
            }
            Op::F64(v) => {
                let _ = writeln!(body, "   Put_F64 (W, {});", real(*v));
            }
            Op::Str(bytes) => {
                let name = format!("S{constant}");
                constant += 1;
                if bytes.is_empty() {
                    let _ = writeln!(body, "   Put_String (W, \"\");");
                } else {
                    decls.push_str(&ada_string(&name, bytes));
                    let _ = writeln!(body, "   Put_String (W, {name});");
                }
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => {
                if value.is_empty() {
                    let _ = writeln!(body, "   Put_Fixed (W, \"\", {digits}, {scale});");
                } else {
                    let name = format!("F{constant}");
                    constant += 1;
                    decls.push_str(&ada_string(&name, value.as_bytes()));
                    let _ = writeln!(body, "   Put_Fixed (W, {name}, {digits}, {scale});");
                }
            }
            Op::Bitset { bits, anchor } => {
                let _ = writeln!(
                    body,
                    "   Put_Bitset (W, {}, {}, {anchor});",
                    bits >> 32,
                    bits & 0xffff_ffff
                );
            }
            Op::Wstr(units) => {
                if units.is_empty() {
                    empty_units = true;
                    let _ = writeln!(body, "   Put_Wstring (W, Empty_Units);");
                } else {
                    let name = format!("U{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : constant Code_Unit_Array (1 .. {}) := ({});",
                        units.len(),
                        units
                            .iter()
                            .enumerate()
                            .map(|(index, unit)| format!("{} => {unit}", index + 1))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(body, "   Put_Wstring (W, {name});");
                }
            }
            Op::SeqU8(bytes) => {
                if bytes.is_empty() {
                    empty_seq = true;
                    let _ = writeln!(body, "   Put_Seq_U8 (W, Empty);");
                } else {
                    let name = format!("D{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : constant Byte_Array (1 .. {}) := ({});",
                        bytes.len(),
                        bytes
                            .iter()
                            .enumerate()
                            .map(|(index, byte)| format!("{} => {byte}", index + 1))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(body, "   Put_Seq_U8 (W, {name});");
                }
            }
            Op::DHeaderBegin => {
                let _ = writeln!(body, "   DHeader_Begin (W, B{at});");
            }
            Op::DHeaderEnd => {
                let _ = writeln!(body, "   DHeader_End (W, B{at});");
            }
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => {
                let _ = writeln!(
                    body,
                    "   EMHeader_Begin (W, {id}, {}, {}, B{at});",
                    ada_bool(*must_understand),
                    lc_name(*lc)
                );
            }
            Op::EmHeaderEnd { lc } => {
                let _ = writeln!(body, "   EMHeader_End (W, B{at}, {});", lc_name(*lc));
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => {
                let _ = writeln!(
                    body,
                    "   Pid_Begin (W, {pid}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => {
                let _ = writeln!(
                    body,
                    "   Pid_Extended_Begin (W, {member_id}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidEnd => {
                let _ = writeln!(body, "   Pid_End (W, P{at});");
            }
            Op::PidListEnd => {
                let _ = writeln!(body, "   Pid_List_End (W);");
            }
        }
    }
    let order = if big_endian { "Big" } else { "Little" };
    // The writer is initialised for XCDR2 and the cap is a record
    // field; XCDR1 is one assignment, the same one the binding's own
    // surface test writes.
    let cap = match program.version {
        Version::Xcdr1 => "Xcdr1_Max_Align",
        Version::Xcdr2 => "Xcdr2_Max_Align",
    };
    let empty = if empty_seq {
        "   Empty : constant Byte_Array (1 .. 0) := (others => 0);\n"
    } else {
        ""
    };
    let empty_unit = if empty_units {
        "   Empty_Units : constant Code_Unit_Array (1 .. 0) := (others => 0);\n"
    } else {
        ""
    };
    let mut slots = String::new();
    for at in &frames.marks {
        let _ = writeln!(slots, "   B{at} : Integer := 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(slots, "   P{at} : Pid_Header;");
    }
    Ok(format!(
        "--  Generated by wireprobe-endpoint-gen. Do not edit.\n\
         --\n\
         --  The octets come out of endpoints/ada-83 when this runs. Nothing here\n\
         --  decides a wire question. Sequential_IO and not Ada.Streams.Stream_IO:\n\
         --  child units are an Ada 95 addition and -gnat83 rejects them.\n\n\
         with Sequential_IO;\n\
         with Zerodds_Ada83_Wire;\n\n\
         procedure Probe is\n\
         \x20  use Zerodds_Ada83_Wire;\n\
         \x20  package Byte_IO is new Sequential_IO (Byte);\n\
         \x20  W     : Writer;\n\
         \x20  File  : Byte_IO.File_Type;\n\
         {empty}\
         {empty_unit}\
         {slots}\
         {decls}\
         begin\n\
         \x20  Init (W, {order});\n\
         \x20  W.Max_Align := {cap};\n\
         {body}\
         \x20  --  `Writer` is a bounded buffer that sets a sticky flag and\n\
         \x20  --  returns rather than raising, so an overrun would leave short\n\
         \x20  --  octets to be read as a divergence. Checked instead.\n\
         \x20  if W.Overflow then\n\
         \x20     raise Program_Error;\n\
         \x20  end if;\n\
         \x20  Byte_IO.Create (File, Byte_IO.Out_File, \"out.bin\");\n\
         \x20  for I in 0 .. Length (W) - 1 loop\n\
         \x20     Byte_IO.Write (File, W.Buf (I));\n\
         \x20  end loop;\n\
         \x20  Byte_IO.Close (File);\n\
         end Probe;\n"
    ))
}

/// Ada's boolean literal.
const fn ada_bool(flag: bool) -> &'static str {
    if flag { "True" } else { "False" }
}

// -- Ada, Stage 2 (pure Ada) --------------------------------------------

fn ada_native(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut decls = String::new();
    let mut body = String::new();
    let mut constant = 0usize;
    let mut empty_seq = false;
    let mut empty_units = false;
    // As in Stage 1: a negative real literal needs the unary minus of
    // `Interfaces.IEEE_Float_32` / `_64` to be directly visible, and
    // `Zerodds_Native_Wire`'s own `use Interfaces` does not reach a
    // client of it.
    let mut negative_f32 = false;
    let mut negative_f64 = false;
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        match op {
            Op::U8(v) => {
                let _ = writeln!(body, "   Put_U8 (W, {});", v.text(form));
            }
            Op::Bool(v) => {
                let _ = writeln!(body, "   Put_Bool (W, {});", ada_bool(*v));
            }
            Op::Char(v) => {
                let _ = writeln!(body, "   Put_Char (W, {});", v.text(form));
            }
            Op::Wchar(v) => {
                let _ = writeln!(body, "   Put_Wchar (W, {});", v.text(form));
            }
            Op::U16(v) => {
                let _ = writeln!(body, "   Put_U16 (W, {});", v.text(form));
            }
            Op::U32(v) | Op::Count(v) => {
                let _ = writeln!(body, "   Put_U32 (W, {});", v.text(form));
            }
            Op::U64(v) => {
                let _ = writeln!(body, "   Put_U64 (W, {});", v.text(form));
            }
            Op::F32(v) => {
                let literal = normalise(&v.text);
                negative_f32 |= literal.starts_with('-');
                let _ = writeln!(body, "   Put_F32 (W, {literal});");
            }
            Op::F64(v) => {
                negative_f64 |= v.is_sign_negative();
                let _ = writeln!(body, "   Put_F64 (W, {});", real(*v));
            }
            Op::Str(bytes) => {
                if bytes.is_empty() {
                    let _ = writeln!(body, "   Put_String (W, \"\");");
                } else {
                    let name = format!("S{constant}");
                    constant += 1;
                    decls.push_str(&ada_string(&name, bytes));
                    let _ = writeln!(body, "   Put_String (W, {name});");
                }
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => {
                if value.is_empty() {
                    let _ = writeln!(body, "   Put_Fixed (W, \"\", {digits}, {scale});");
                } else {
                    let name = format!("F{constant}");
                    constant += 1;
                    decls.push_str(&ada_string(&name, value.as_bytes()));
                    let _ = writeln!(body, "   Put_Fixed (W, {name}, {digits}, {scale});");
                }
            }
            Op::Bitset { bits, anchor } => {
                let _ = writeln!(body, "   Put_Bitset (W, {bits}, {anchor});");
            }
            Op::Wstr(units) => {
                if units.is_empty() {
                    empty_units = true;
                    let _ = writeln!(body, "   Put_Wstring (W, Empty_Units);");
                } else {
                    let name = format!("U{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : constant Code_Unit_Array (0 .. {}) := [{}];",
                        units.len() - 1,
                        units
                            .iter()
                            .map(|unit| format!("{unit}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(body, "   Put_Wstring (W, {name});");
                }
            }
            Op::SeqU8(bytes) => {
                if bytes.is_empty() {
                    empty_seq = true;
                    let _ = writeln!(body, "   Put_Seq_U8 (W, Empty);");
                } else {
                    let name = format!("D{constant}");
                    constant += 1;
                    let _ = writeln!(
                        decls,
                        "   {name} : constant Byte_Array (0 .. {}) := [{}];",
                        bytes.len() - 1,
                        bytes
                            .iter()
                            .map(|byte| format!("{byte}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let _ = writeln!(body, "   Put_Seq_U8 (W, {name});");
                }
            }
            Op::DHeaderBegin => {
                let _ = writeln!(body, "   DHeader_Begin (W, B{at});");
            }
            Op::DHeaderEnd => {
                let _ = writeln!(body, "   DHeader_End (W, B{at});");
            }
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => {
                let _ = writeln!(
                    body,
                    "   EMHeader_Begin (W, {id}, {}, {}, B{at});",
                    ada_bool(*must_understand),
                    lc_name(*lc)
                );
            }
            Op::EmHeaderEnd { lc } => {
                let _ = writeln!(body, "   EMHeader_End (W, B{at}, {});", lc_name(*lc));
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => {
                let _ = writeln!(
                    body,
                    "   Pid_Begin (W, {pid}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => {
                let _ = writeln!(
                    body,
                    "   Pid_Extended_Begin (W, {member_id}, {}, P{at});",
                    ada_bool(*must_understand)
                );
            }
            Op::PidEnd => {
                let _ = writeln!(body, "   Pid_End (W, P{at});");
            }
            Op::PidListEnd => {
                let _ = writeln!(body, "   Pid_List_End (W);");
            }
        }
    }
    let mut slots = String::new();
    for at in &frames.marks {
        let _ = writeln!(slots, "   B{at} : Natural := 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(slots, "   P{at} : Pid_Header;");
    }
    let order = if big_endian { "Big" } else { "Little" };
    // The writer is initialised for XCDR2 and the cap is a record
    // field; XCDR1 is one assignment, the same one the binding's own
    // surface test writes.
    let cap = match program.version {
        Version::Xcdr1 => "Xcdr1_Max_Align",
        Version::Xcdr2 => "Xcdr2_Max_Align",
    };
    let empty = if empty_seq {
        "   Empty : constant Byte_Array (1 .. 0) := [others => 0];\n"
    } else {
        ""
    };
    let empty_unit = if empty_units {
        "   Empty_Units : constant Code_Unit_Array (1 .. 0) := [others => 0];\n"
    } else {
        ""
    };
    let mut float_ops = String::new();
    if negative_f32 {
        float_ops.push_str("   use type Interfaces.IEEE_Float_32;\n");
    }
    if negative_f64 {
        float_ops.push_str("   use type Interfaces.IEEE_Float_64;\n");
    }
    let interfaces = if float_ops.is_empty() {
        ""
    } else {
        "with Interfaces;\n"
    };
    Ok(format!(
        "--  Generated by wireprobe-endpoint-gen. Do not edit.\n\
         --\n\
         --  The octets come out of endpoints/ada-native when this runs. Nothing\n\
         --  here decides a wire question.\n\n\
         with Ada.Streams.Stream_IO;\n\
         {interfaces}\
         with Zerodds_Native_Wire;\n\n\
         procedure Probe is\n\
         \x20  use Zerodds_Native_Wire;\n\
         {float_ops}\
         \x20  W     : Writer;\n\
         \x20  File  : Ada.Streams.Stream_IO.File_Type;\n\
         {empty}\
         {empty_unit}\
         {slots}\
         {decls}\
         begin\n\
         \x20  Init (W, {order});\n\
         \x20  W.Max_Align := {cap};\n\
         {body}\
         \x20  --  A bounded writer that overran sets a sticky flag and returns,\n\
         \x20  --  so short octets would be read as a divergence. Checked instead.\n\
         \x20  if W.Overflow then\n\
         \x20     raise Program_Error;\n\
         \x20  end if;\n\
         \x20  Ada.Streams.Stream_IO.Create\n\
         \x20    (File, Ada.Streams.Stream_IO.Out_File, \"out.bin\");\n\
         \x20  declare\n\
         \x20     Octets  : constant Byte_Array := Bytes (W);\n\
         \x20     Payload : Ada.Streams.Stream_Element_Array\n\
         \x20       (1 .. Ada.Streams.Stream_Element_Offset (Octets'Length));\n\
         \x20  begin\n\
         \x20     for I in Octets'Range loop\n\
         \x20        Payload\n\
         \x20          (Ada.Streams.Stream_Element_Offset (I - Octets'First + 1)) :=\n\
         \x20            Ada.Streams.Stream_Element (Octets (I));\n\
         \x20     end loop;\n\
         \x20     Ada.Streams.Stream_IO.Write (File, Payload);\n\
         \x20  end;\n\
         \x20  Ada.Streams.Stream_IO.Close (File);\n\
         end Probe;\n"
    ))
}

// -- Kotlin, Node, F# and Swift -----------------------------------------
//
// Four bindings with the same operation set and four different holder
// sets, which is the whole of what separates these renderers. Kotlin's
// widest integer is signed, so an `unsigned long long` reaches `putU64`
// as its two's complement; JavaScript's is a `BigInt`; F#'s putters are
// typed per width and take a suffixed literal; Swift's are unsigned and
// take the value as written.

/// A string literal for a language whose strings are UTF-16 code units.
///
/// Kotlin, JavaScript and F# all measure a string in UTF-16 units and
/// all take `\uXXXX`, so a character outside the basic plane is written
/// as its surrogate pair. Everything outside printable ASCII is escaped,
/// for the reason `wireprobe-zerodds-java` compiles with
/// `-encoding UTF-8`: a probe whose octets depended on how the compiler
/// read its own source file would be measuring the compiler.
///
/// `dollar` escapes `$` as well, which Kotlin needs because an
/// unescaped one starts a template expression.
fn utf16_literal(bytes: &[u8], dollar: bool) -> Result<String, String> {
    // The octets came out of a JSON string, so they are UTF-8 by
    // construction; a failure here is this generator's and is reported
    // as one rather than silently replaced with U+FFFD, which would put
    // octets on the wire that no sample asked for.
    let text = core::str::from_utf8(bytes)
        .map_err(|error| format!("a string that is not UTF-8: {error}"))?;
    let mut out = String::from("\"");
    for unit in text.encode_utf16() {
        match unit {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x24 if dollar => out.push_str("\\$"),
            0x20..=0x7E => out.push(char::from(unit as u8)),
            other => {
                let _ = write!(out, "\\u{other:04X}");
            }
        }
    }
    out.push('"');
    Ok(out)
}

/// The same, for Swift, whose escape names a Unicode scalar.
fn scalar_literal(bytes: &[u8]) -> Result<String, String> {
    let text = core::str::from_utf8(bytes)
        .map_err(|error| format!("a string that is not UTF-8: {error}"))?;
    let mut out = String::from("\"");
    for one in text.chars() {
        match one {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(one),
            other => {
                let _ = write!(out, "\\u{{{:X}}}", u32::from(other));
            }
        }
    }
    out.push('"');
    Ok(out)
}

// -- Kotlin -------------------------------------------------------------

fn kotlin(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "// Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/kotlin/src/Zerodds.kt when this\n\
         // runs. Nothing here decides a wire question.\n\n\
         import zerodds.Encoding\n\
         import zerodds.Endian\n",
    );
    // `-Werror` fails an unused import, so `Pid` is named only when a
    // parameter header is opened. It is `Writer.Pid`, a nested class:
    // `zerodds.Pid` is not a declaration the binding exports.
    if !frames.pids.is_empty() {
        out.push_str("import zerodds.Writer.Pid\n");
    }
    out.push_str("import zerodds.Writer\n\nfun main() {\n    val w = Writer(Endian.");
    out.push_str(if big_endian { "BIG" } else { "LITTLE" });
    out.push_str(", Encoding.");
    out.push_str(match program.version {
        Version::Xcdr1 => "XCDR1",
        Version::Xcdr2 => "XCDR2",
    });
    out.push_str(")\n");
    // Declared without an initialiser, and definite assignment left to
    // the compiler: `kotlinc -Werror` fails
    // `VARIABLE_WITH_REDUNDANT_INITIALIZER` on a value the opening call
    // overwrites before anything reads it.
    for at in &frames.marks {
        let _ = writeln!(out, "    var b{at}: Int");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "    var p{at}: Pid");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("    w.putU8({})\n", v.text(form)),
            Op::Bool(v) => format!("    w.putBool({v})\n"),
            Op::Char(v) => format!("    w.putChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("    w.putWchar({})\n", v.text(form)),
            Op::U16(v) => format!("    w.putU16({})\n", v.text(form)),
            // `putU32(v: Long)` and `putU64(v: Long)` both take a Long.
            // The 32-bit one fits its value; the 64-bit one takes the
            // **pattern**, because Kotlin's widest integer is signed and
            // 16045690984833335998 has no literal — the same conversion
            // `wireprobe-zerodds-java-gen` makes for `Writer.writeU64`.
            Op::U32(v) | Op::Count(v) => format!("    w.putU32({}L)\n", v.text(form)),
            Op::U64(v) => format!("    w.putU64({})\n", kotlin_long(*v, form)),
            Op::F32(v) => format!("    w.putF32({}f)\n", float_text(&v.text)),
            Op::F64(v) => format!("    w.putF64({})\n", real(*v)),
            Op::Str(bytes) => format!("    w.putString({})\n", utf16_literal(bytes, true)?),
            Op::SeqU8(bytes) => format!("    w.putSeqU8({})\n", kotlin_bytes(bytes)),
            Op::Wstr(units) => format!("    w.putWstring({})\n", kotlin_units(units)),
            Op::Bitset { bits, anchor } => format!(
                "    w.putBitset({}, {anchor})\n",
                kotlin_long(Int::new(*bits, 0), Form::Bits)
            ),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "    w.putFixed({}, {digits}, {scale})\n",
                utf16_literal(value.as_bytes(), true)?
            ),
            Op::DHeaderBegin => format!("    b{at} = w.dheaderBegin()\n"),
            Op::DHeaderEnd => format!("    w.dheaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "    b{at} = w.emheaderBegin({id}L, {must_understand}, {})\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("    w.emheaderEnd(b{at}, {})\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("    p{at} = w.pidBegin({pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("    p{at} = w.pidExtendedBegin({member_id}L, {must_understand})\n"),
            Op::PidEnd => format!("    w.pidEnd(p{at})\n"),
            Op::PidListEnd => "    w.pidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str("    java.io.File(\"out.bin\").writeBytes(w.bytes())\n}\n");
    Ok(out)
}

/// A 64-bit pattern as the `Long` `putU64` takes — and the `long`
/// Java's `u64` takes, which is the same signed holder with the same
/// one literal-less pattern, so the Java renderer calls this too.
///
/// One pattern has no literal at all. Kotlin has no negative literals:
/// `-9223372036854775808L` is unary minus applied to
/// `9223372036854775808L`, and that literal is one past `Long.MAX_VALUE`,
/// so `kotlinc` refuses the line. Every other value in the range writes
/// out as its signed decimal.
///
/// Measured on `bitmask-bitbound-64`, whose second member is bit 63 —
/// 0x8000000000000000 — in the six XCDR2 configurations:
/// `Probe.kt:12:14: error: type mismatch: inferred type is Int but Long
/// was expected`. The sweep reported it as a `generator-defect`, which
/// is what it was.
fn kotlin_long(argument: Int, form: Form) -> String {
    let signed = match form {
        Form::Bits => argument.bits as i64,
        // The natural spelling of a `long long` reaches the same wall
        // from the other side: −9223372036854775808 is the value, and
        // Kotlin spells that one `Long.MIN_VALUE` too. An `unsigned long
        // long` past `Long.MAX_VALUE` has no natural spelling at all,
        // which is the answer `endpoint-number-surface` is asking for
        // and is left to `kotlinc` to give.
        Form::Natural => match i64::try_from(argument.natural) {
            Ok(value) => value,
            Err(_) => return format!("{}L", argument.natural),
        },
    };
    if signed == i64::MIN {
        return "Long.MIN_VALUE".to_string();
    }
    format!("{signed}L")
}

/// A `ByteArray` literal. Kotlin's `Byte` is signed, so every octet
/// above 0x7F needs the conversion written out.
fn kotlin_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "byteArrayOf()".to_string();
    }
    let items = bytes
        .iter()
        .map(|byte| format!("0x{byte:02X}.toByte()"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("byteArrayOf({items})")
}

/// An `IntArray` of the code units — the argument `putWstring` takes,
/// because Kotlin's `Short` is signed and a unit above 0x7FFF would
/// need a conversion at every element.
fn kotlin_units(units: &[u16]) -> String {
    let items = units
        .iter()
        .map(|unit| format!("0x{unit:04X}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("intArrayOf({items})")
}

// -- Node ---------------------------------------------------------------

fn node(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "'use strict';\n\
         // Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/node/zerodds.js when this runs.\n\
         // Nothing here decides a wire question.\n\n\
         // `__dirname` and not a bare module name: the probe measures the copy\n\
         // the build put beside it and can never pick up one a node_modules\n\
         // shadowed.\n\
         const zd = require(__dirname + '/zerodds.js');\n\
         const w = new zd.Writer(zd.",
    );
    out.push_str(if big_endian { "BIG" } else { "LITTLE" });
    out.push_str(", zd.");
    out.push_str(match program.version {
        Version::Xcdr1 => "XCDR1",
        Version::Xcdr2 => "XCDR2",
    });
    out.push_str(");\n");
    for at in &frames.marks {
        let _ = writeln!(out, "let b{at} = 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "let p{at} = null;");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("w.putU8({});\n", v.text(form)),
            Op::Bool(v) => format!("w.putBool({v});\n"),
            Op::Char(v) => format!("w.putChar({});\n", v.text(form)),
            Op::Wchar(v) => format!("w.putWchar({});\n", v.text(form)),
            Op::U16(v) => format!("w.putU16({});\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("w.putU32({});\n", v.text(form)),
            // `putU64` takes a BigInt and the SDK says why: a JavaScript
            // number loses precision above 2^53. The pattern goes in
            // unsigned, because BigInt's bitwise operators work on an
            // infinite two's complement and the unsigned literal is the
            // one spelling that cannot be read two ways.
            Op::U64(v) => format!("w.putU64({}n);\n", v.text(form)),
            Op::F32(v) => format!("w.putF32({});\n", float_text(&v.text)),
            Op::F64(v) => format!("w.putF64({});\n", real(*v)),
            Op::Str(bytes) => format!("w.putString({});\n", utf16_literal(bytes, false)?),
            Op::SeqU8(bytes) => format!("w.putSeqU8(Buffer.from([{}]));\n", node_bytes(bytes)),
            Op::Wstr(units) => format!("w.putWstring([{}]);\n", node_units(units)),
            // A BigInt for the same reason `putU64` takes one: an
            // eight-octet anchor runs past 2^53.
            Op::Bitset { bits, anchor } => format!("w.putBitset({bits}n, {anchor});\n"),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "w.putFixed({}, {digits}, {scale});\n",
                utf16_literal(value.as_bytes(), false)?
            ),
            Op::DHeaderBegin => format!("b{at} = w.dheaderBegin();\n"),
            Op::DHeaderEnd => format!("w.dheaderEnd(b{at});\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "b{at} = w.emheaderBegin({id}, {must_understand}, {});\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("w.emheaderEnd(b{at}, {});\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("p{at} = w.pidBegin({pid}, {must_understand});\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("p{at} = w.pidExtendedBegin({member_id}, {must_understand});\n"),
            Op::PidEnd => format!("w.pidEnd(p{at});\n"),
            Op::PidListEnd => "w.pidListEnd();\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str("require('fs').writeFileSync('out.bin', w.bytes());\n");
    Ok(out)
}

fn node_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn node_units(units: &[u16]) -> String {
    units
        .iter()
        .map(|unit| format!("0x{unit:04x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

// -- F# -----------------------------------------------------------------

fn fsharp(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "// Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/fsharp/zerodds.fs when this runs.\n\
         // Nothing here decides a wire question.\n\n\
         // `#load` and not a project reference: `dotnet build` spends two to\n\
         // four seconds before a source file is read and a corpus sweep is\n\
         // thousands of runs. The build copies zerodds.fs beside this file.\n\
         #load \"zerodds.fs\"\n\
         open ZeroDDS\n\n\
         let w = Writer(",
    );
    out.push_str(if big_endian { "BE" } else { "LE" });
    out.push_str(", ");
    out.push_str(match program.version {
        Version::Xcdr1 => "XCDR1",
        Version::Xcdr2 => "XCDR2",
    });
    out.push_str(")\n");
    // `let mutable` and not a shadowing `let`: `dotnet fsi` compiles the
    // whole script as one module, where a second binding of the same
    // name is FS0037, a duplicate definition.
    for at in &frames.marks {
        let _ = writeln!(out, "let mutable b{at} = 0");
    }
    for at in &frames.pids {
        let _ = writeln!(
            out,
            "let mutable p{at} : Pid = {{ At = 0; Extended = false; SavedOrigin = 0 }}"
        );
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("w.PutU8({})\n", v.text(form)),
            Op::Bool(v) => format!("w.PutBool({v})\n"),
            Op::Char(v) => format!("w.PutChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("w.PutWchar({})\n", v.text(form)),
            Op::U16(v) => format!("w.PutU16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("w.PutU32({}u)\n", v.text(form)),
            Op::U64(v) => format!("w.PutU64({}UL)\n", v.text(form)),
            Op::F32(v) => format!("w.PutF32({}f)\n", float_text(&v.text)),
            Op::F64(v) => format!("w.PutF64({})\n", real(*v)),
            Op::Str(bytes) => format!("w.PutString({})\n", utf16_literal(bytes, false)?),
            Op::SeqU8(bytes) => format!("w.PutSeqU8({})\n", fsharp_bytes(bytes)),
            Op::Wstr(units) => format!("w.PutWstring({})\n", fsharp_units(units)),
            Op::Bitset { bits, anchor } => format!("w.PutBitset({bits}UL, {anchor})\n"),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "w.PutFixed({}, {digits}, {scale})\n",
                utf16_literal(value.as_bytes(), false)?
            ),
            Op::DHeaderBegin => format!("b{at} <- w.DHeaderBegin()\n"),
            Op::DHeaderEnd => format!("w.DHeaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "b{at} <- w.EmHeaderBegin({id}u, {must_understand}, {})\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("w.EmHeaderEnd(b{at}, {})\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("p{at} <- w.PidBegin({pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("p{at} <- w.PidExtendedBegin({member_id}u, {must_understand})\n"),
            Op::PidEnd => format!("w.PidEnd(p{at})\n"),
            Op::PidListEnd => "w.PidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str("System.IO.File.WriteAllBytes(\"out.bin\", w.Bytes())\n");
    Ok(out)
}

/// A `byte[]` literal. An empty array literal has no element to infer an
/// element type from, so the empty case is written with its type.
fn fsharp_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "Array.empty<byte>".to_string();
    }
    let items = bytes
        .iter()
        .map(|byte| format!("0x{byte:02X}uy"))
        .collect::<Vec<_>>()
        .join("; ");
    format!("[| {items} |]")
}

/// An `int[]` of the code units, which is what `PutWstring` takes.
fn fsharp_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "Array.empty<int>".to_string();
    }
    let items = units
        .iter()
        .map(|unit| format!("0x{unit:04X}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!("[| {items} |]")
}

// -- Swift --------------------------------------------------------------

fn swift(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = String::from(
        "// Generated by wireprobe-endpoint-gen. Do not edit.\n\
         //\n\
         // The octets come out of endpoints/swift/Sources/Zerodds/Zerodds.swift\n\
         // when this runs. Nothing here decides a wire question.\n\n\
         import Foundation\n\n",
    );
    // `var` only when something mutates it. `-warnings-as-errors` turns
    // "variable was never mutated" into a failed compile, and the corpus
    // holds a structure with no members at all.
    out.push_str(if program.ops.is_empty() {
        "let w"
    } else {
        "var w"
    });
    out.push_str(" = Writer(");
    out.push_str(if big_endian { ".big" } else { ".little" });
    out.push_str(", ");
    out.push_str(match program.version {
        Version::Xcdr1 => ".xcdr1",
        Version::Xcdr2 => ".xcdr2",
    });
    out.push_str(")\n");
    for at in &frames.marks {
        let _ = writeln!(out, "var b{at} = 0");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "var p{at}: Writer.Pid? = nil");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("w.putU8({})\n", v.text(form)),
            Op::Bool(v) => format!("w.putBool({v})\n"),
            // `putChar` and `putWchar` refuse a code point their width
            // cannot hold by throwing, where the other bindings take the
            // narrower integer at the call. `try` is the SDK's spelling
            // of the refusal, so it is written here.
            Op::Char(v) => format!("try w.putChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("try w.putWchar({})\n", v.text(form)),
            Op::U16(v) => format!("w.putU16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("w.putU32({})\n", v.text(form)),
            Op::U64(v) => format!("w.putU64({})\n", v.text(form)),
            Op::F32(v) => format!("w.putF32({})\n", float_text(&v.text)),
            Op::F64(v) => format!("w.putF64({})\n", real(*v)),
            Op::Str(bytes) => format!("w.putString({})\n", scalar_literal(bytes)?),
            Op::SeqU8(bytes) => format!("w.putSeqU8({})\n", swift_bytes(bytes)),
            Op::Wstr(units) => format!("w.putWstring({})\n", swift_units(units)),
            Op::Bitset { bits, anchor } => {
                format!("try w.putBitset({bits}, anchor: {anchor})\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "try w.putFixed({}, digits: {digits}, scale: {scale})\n",
                scalar_literal(value.as_bytes())?
            ),
            Op::DHeaderBegin => format!("b{at} = w.dheaderBegin()\n"),
            Op::DHeaderEnd => format!("w.dheaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "b{at} = try w.emheaderBegin(id: {id}, mustUnderstand: {must_understand}, \
                 lengthCode: .{})\n",
                lc_enum(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("try w.emheaderEnd(b{at}, lengthCode: .{})\n", lc_enum(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("p{at} = try w.pidBegin({pid}, mustUnderstand: {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!(
                "p{at} = try w.pidExtendedBegin({member_id}, \
                 mustUnderstand: {must_understand})\n"
            ),
            // Force-unwrapped for the reason Kotlin's `!!` is: the
            // matching begin assigned it, and a nil here would be this
            // generator's defect and must stop the program.
            Op::PidEnd => format!("try w.pidEnd(p{at}!)\n"),
            Op::PidListEnd => "w.pidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    // `fatalError` and not `exit`: it is Swift's own and needs no
    // platform module, and a probe that could not write its octets must
    // not leave a zero-length `out.bin` behind for the sweep to read as
    // a measurement.
    out.push_str(
        "do {\n\
         \x20   try Data(w.bytes()).write(to: URL(fileURLWithPath: \"out.bin\"))\n\
         } catch {\n\
         \x20   fatalError(\"out.bin: \\(error)\")\n\
         }\n",
    );
    Ok(out)
}

/// A `[UInt8]` literal. An empty one names its type, because Swift will
/// not infer an element type from no elements.
fn swift_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "[UInt8]()".to_string();
    }
    let items = bytes
        .iter()
        .map(|byte| format!("0x{byte:02X}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

/// A `[UInt16]` of the code units, empty one named for the same reason.
fn swift_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "[UInt16]()".to_string();
    }
    let items = units
        .iter()
        .map(|unit| format!("0x{unit:04X}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

// -- The corpus decimal, and the spellings it needs ---------------------
//
// All fifteen hand `put_f32` the decimal exactly as the sample wrote it,
// so the narrowing to `binary32` is the target language's and not this
// program's. What that reaches is the language's own decimal-to-float
// conversion — Lua, OCaml, Elixir, Node and Python have no 32-bit float
// at all, so their narrowing happens inside `put_f32` where it can be
// seen. `ops::Real` still carries the `binary32` this generator narrowed
// the sample to; the one renderer that keeps using it is `java`, which
// is not one of the fifteen.
//
// The literal is split by front end and not by binding. Eleven of the
// fifteen — Go, Zig, Nim, D, Kotlin, Node, F#, Swift, Lua, OCaml and
// Elixir — accept the corpus decimal through `float_text`, as does
// Python; the three Ada bindings take an Ada real literal through
// `normalise`; and Julia does not spell the exponent with an `e` at
// all. Every one preserves the corpus digits and only varies the point,
// the case and the suffix.
//
// `put_f64` is not split that way, because there is nothing to narrow:
// every one of the fifteen holds a `double` natively, so the shortest
// round-trip decimal of the `f64` is the same number in all of them.

/// The decimal split into a significand that always carries a point and
/// the power of ten, handed back apart.
///
/// `serde_json` renders a JSON number in the shortest form that reads
/// back as the same `f64`, which is `1` for one and `1e39` for a number
/// past the `binary32` range. Neither is a float literal everywhere:
/// `1` is an integer in OCaml, in Elixir and in Lua's
/// `string.pack("f", …)` argument check, and `1e39` is a syntax error in
/// Elixir, which wants a digit on both sides of the point. Both were
/// measured — the second by asking the five what `put_f32` does with a
/// value outside the range, where Elixir answered `invalid character "e"
/// after number 1`.
///
/// Apart rather than joined, because Julia does not spell the exponent
/// with an `e` at all.
fn decimal(text: &str) -> (String, Option<String>) {
    let (significand, exponent) = match text.find(['e', 'E']) {
        Some(at) => (
            text.get(..at).unwrap_or_default(),
            Some(text.get(at + 1..).unwrap_or_default().to_string()),
        ),
        None => (text, None),
    };
    let significand = if significand.contains('.') {
        significand.to_string()
    } else {
        format!("{significand}.0")
    };
    (significand, exponent)
}

/// A float literal for a language whose only number is a `double` — Go,
/// Lua, OCaml and Elixir.
fn float_text(text: &str) -> String {
    match decimal(text) {
        (significand, None) => significand,
        (significand, Some(exponent)) => format!("{significand}e{exponent}"),
    }
}

/// A float literal for Julia.
///
/// Under [`Form::Bits`] it is a `Float32` literal, which Julia spells
/// with `f` where every other language here spells `e`: `-3.75f0` and
/// `1.0f39`, never `-3.75f` or `1.0e39f0`. Under [`Form::Natural`] it is
/// the ordinary `Float64` literal, so `put_f32!` does the narrowing —
/// which is the same question the integer forms ask, one type over.
fn julia_float(text: &str, form: Form) -> String {
    let (significand, exponent) = decimal(text);
    match (form, exponent) {
        (Form::Bits, exponent) => {
            format!("{significand}f{}", exponent.unwrap_or_else(|| "0".into()))
        }
        (Form::Natural, None) => significand,
        (Form::Natural, Some(exponent)) => format!("{significand}e{exponent}"),
    }
}

/// `\xNN` for every octet — the escape Go, Lua, OCaml and Julia all
/// spell the same way. Escaping everything rather than only what needs it
/// keeps a string's octets out of the source file's own encoding.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for byte in bytes {
        let _ = write!(out, "\\x{byte:02x}");
    }
    out
}

/// `0xDE, 0xAD` — Elixir's and Julia's octet list, for the places where
/// no `\x` form survives a sequence that is not valid UTF-8.
fn octet_list(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("0x{byte:02X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The same, for a run of two-octet code units.
fn unit_list(units: &[u16]) -> String {
    units
        .iter()
        .map(|unit| format!("0x{unit:04X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The banner the five carry, in the comment syntax of each.
fn banner(comment: &str, sdk: &str) -> String {
    format!(
        "{comment} Generated by wireprobe-endpoint-gen. Do not edit.\n\
         {comment}\n\
         {comment} The octets come out of {sdk} when this runs.\n\
         {comment} Nothing here decides a wire question.\n\n"
    )
}

// -- Go -----------------------------------------------------------------

fn go(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("//", "endpoints/go/wire.go");
    out.push_str("package main\n\nimport (\n\t\"os\"\n\n\tzerodds \"zeroddsendpoint\"\n)\n\n");
    out.push_str("func main() {\n");
    let _ = writeln!(
        out,
        "\tw := zerodds.NewWriter(zerodds.{}, zerodds.{})",
        if big_endian { "Big" } else { "Little" },
        match program.version {
            Version::Xcdr1 => "Xcdr1",
            Version::Xcdr2 => "Xcdr2",
        }
    );
    for at in &frames.marks {
        let _ = writeln!(out, "\tvar b{at} int");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "\tvar p{at} zerodds.Pid");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("\tw.PutU8({})\n", v.text(form)),
            Op::Bool(v) => format!("\tw.PutBool({v})\n"),
            Op::Char(v) => format!("\tw.PutChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("\tw.PutWchar({})\n", v.text(form)),
            Op::U16(v) => format!("\tw.PutU16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("\tw.PutU32({})\n", v.text(form)),
            Op::U64(v) => format!("\tw.PutU64({})\n", v.text(form)),
            Op::F32(v) => format!("\tw.PutF32({})\n", float_text(&v.text)),
            Op::F64(v) => format!("\tw.PutF64({})\n", real(*v)),
            Op::Str(bytes) => format!("\tw.PutString(\"{}\")\n", escaped(bytes)),
            Op::SeqU8(bytes) => format!("\tw.PutSeqU8([]byte(\"{}\"))\n", escaped(bytes)),
            Op::Wstr(units) => format!("\tw.PutWstring({})\n", go_units(units)),
            Op::Bitset { bits, anchor } => format!("\tw.PutBitset({bits}, {anchor})\n"),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "\tw.PutFixed(\"{}\", {digits}, {scale})\n",
                escaped(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("\tb{at} = w.DHeaderBegin()\n"),
            Op::DHeaderEnd => format!("\tw.DHeaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "\tb{at} = w.EmHeaderBegin({id}, {must_understand}, {})\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("\tw.EmHeaderEnd(b{at}, {})\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("\tp{at} = w.PidBegin({pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("\tp{at} = w.PidExtendedBegin({member_id}, {must_understand})\n"),
            Op::PidEnd => format!("\tw.PidEnd(p{at})\n"),
            Op::PidListEnd => "\tw.PidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    // The error is read because `go vet` is this toolchain's warning
    // level and the probe treats a dirty vet as a failed build.
    out.push_str(
        "\tif err := os.WriteFile(\"out.bin\", w.Buf, 0o644); err != nil {\n\
         \t\tos.Exit(6)\n\t}\n}\n",
    );
    Ok(out)
}

/// A `[]uint16` of the code units. `nil` for the empty one, which is
/// the slice of length zero Go's own surface test hands over.
fn go_units(units: &[u16]) -> String {
    if units.is_empty() {
        return "nil".to_string();
    }
    format!("[]uint16{{{}}}", unit_list(units))
}

// -- Julia --------------------------------------------------------------

fn julia(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("#", "endpoints/julia/zerodds.jl");
    out.push_str("include(joinpath(@__DIR__, \"zerodds.jl\"))\n\n");
    let _ = writeln!(
        out,
        "const W = ZeroDDS.Writer(ZeroDDS.{}, ZeroDDS.{})\n",
        if big_endian { "BE" } else { "LE" },
        match program.version {
            Version::Xcdr1 => "XCDR1",
            Version::Xcdr2 => "XCDR2",
        }
    );
    // Inside a function, so Julia compiles the writes rather than
    // lowering them one at a time at top level: a case with a few
    // thousand writes is otherwise minutes. The framing locals are the
    // function's own; Julia declares a local by assigning it, so the
    // opening call is the declaration and no default is needed.
    out.push_str("function fill!()\n");
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("    ZeroDDS.put_u8!(W, {})\n", v.text(form)),
            Op::Bool(v) => format!("    ZeroDDS.put_bool!(W, {v})\n"),
            Op::Char(v) => format!("    ZeroDDS.put_char!(W, {})\n", v.text(form)),
            Op::Wchar(v) => format!("    ZeroDDS.put_wchar!(W, {})\n", v.text(form)),
            Op::U16(v) => format!("    ZeroDDS.put_u16!(W, {})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("    ZeroDDS.put_u32!(W, {})\n", v.text(form)),
            Op::U64(v) => format!("    ZeroDDS.put_u64!(W, {})\n", v.text(form)),
            Op::F32(v) => format!("    ZeroDDS.put_f32!(W, {})\n", julia_float(&v.text, form)),
            Op::F64(v) => format!("    ZeroDDS.put_f64!(W, {})\n", real(*v)),
            Op::Str(bytes) => format!("    ZeroDDS.put_string!(W, \"{}\")\n", escaped(bytes)),
            Op::SeqU8(bytes) => {
                format!("    ZeroDDS.put_seq_u8!(W, UInt8[{}])\n", octet_list(bytes))
            }
            Op::Wstr(units) => {
                format!(
                    "    ZeroDDS.put_wstring!(W, UInt16[{}])\n",
                    unit_list(units)
                )
            }
            // A sixteen-digit hexadecimal, which Julia types `UInt64`
            // from its digit count. A decimal past `typemax(Int64)` is
            // typed by its magnitude and a shorter one is an `Int`, so
            // the hexadecimal is the one spelling that is the same type
            // for every anchor.
            Op::Bitset { bits, anchor } => {
                format!("    ZeroDDS.put_bitset!(W, 0x{bits:016x}, {anchor})\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "    ZeroDDS.put_fixed!(W, \"{}\", {digits}, {scale})\n",
                escaped(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("    B{at} = ZeroDDS.dheader_begin!(W)\n"),
            Op::DHeaderEnd => format!("    ZeroDDS.dheader_end!(W, B{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "    B{at} = ZeroDDS.emheader_begin!(W, {id}, {must_understand}, {})\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("    ZeroDDS.emheader_end!(W, B{at}, {})\n", code(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("    P{at} = ZeroDDS.pid_begin!(W, {pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!(
                "    P{at} = ZeroDDS.pid_extended_begin!(W, {member_id}, {must_understand})\n"
            ),
            Op::PidEnd => format!("    ZeroDDS.pid_end!(W, P{at})\n"),
            Op::PidListEnd => "    ZeroDDS.pid_list_end!(W)\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str("end\n\nfill!()\nwrite(\"out.bin\", ZeroDDS.bytes(W))\n");
    Ok(out)
}

// -- Lua ----------------------------------------------------------------

fn lua(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("--", "endpoints/lua/zerodds.lua");
    out.push_str("local zd = require(\"zerodds\")\n");
    let _ = writeln!(
        out,
        "local w = zd.Writer.new(zd.{}, zd.{})",
        if big_endian { "BE" } else { "LE" },
        match program.version {
            Version::Xcdr1 => "XCDR1",
            Version::Xcdr2 => "XCDR2",
        }
    );
    // Declared once at the top and assigned at each open, not declared
    // at each open: a Lua function holds two hundred locals and a
    // `local` per member header in a large sample runs out of them.
    for at in &frames.marks {
        let _ = writeln!(out, "local b{at}");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "local p{at}");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("w:putU8({})\n", v.text(form)),
            Op::Bool(v) => format!("w:putBool({v})\n"),
            Op::Char(v) => format!("w:putChar({})\n", v.text(form)),
            Op::Wchar(v) => format!("w:putWchar({})\n", v.text(form)),
            Op::U16(v) => format!("w:putU16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("w:putU32({})\n", v.text(form)),
            // A 64-bit hexadecimal literal is an integer in Lua 5.3+ even
            // past `math.maxinteger`; the decimal for the same value is a
            // float, and `string.pack("I8", …)` needs the integer. That
            // is the divergence `endpoint-number-surface` measures, so
            // the two forms are handed over as they stand and neither is
            // corrected here.
            Op::U64(v) => format!("w:putU64({})\n", lua_u64(*v, form)),
            Op::F32(v) => format!("w:putF32({})\n", float_text(&v.text)),
            Op::F64(v) => format!("w:putF64({})\n", real(*v)),
            Op::Str(bytes) => format!("w:putString(\"{}\")\n", escaped(bytes)),
            Op::SeqU8(bytes) => format!("w:putSeqU8(\"{}\")\n", escaped(bytes)),
            Op::Wstr(units) => format!("w:putWstring({{{}}})\n", unit_list(units)),
            // Hexadecimal for the reason `putU64` is: `putBitset`
            // refuses an argument `math.type` does not call an integer,
            // and the decimal of an eight-octet anchor past
            // `math.maxinteger` is a float.
            Op::Bitset { bits, anchor } => {
                format!("w:putBitset(0x{bits:016X}, {anchor})\n")
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "w:putFixed(\"{}\", {digits}, {scale})\n",
                escaped(value.as_bytes())
            ),
            Op::DHeaderBegin => format!("b{at} = w:dheaderBegin()\n"),
            Op::DHeaderEnd => format!("w:dheaderEnd(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "b{at} = w:emheaderBegin({id}, {must_understand}, {})\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("w:emheaderEnd(b{at}, {})\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("p{at} = w:pidBegin({pid}, {must_understand})\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("p{at} = w:pidExtendedBegin({member_id}, {must_understand})\n"),
            Op::PidEnd => format!("w:pidEnd(p{at})\n"),
            Op::PidListEnd => "w:pidListEnd()\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "local f = assert(io.open(\"out.bin\", \"wb\"))\n\
         f:write(w:bytes())\n\
         f:close()\n",
    );
    Ok(out)
}

/// Lua's 64-bit argument.
///
/// Hexadecimal under [`Form::Bits`]: a decimal past `math.maxinteger` is
/// a **float** to Lua's lexer, and `string.pack("I8", …)` refuses a
/// float, so the decimal spelling of `0xFFFFFFFFFFFFFFFE` does not reach
/// the wire at all. The hexadecimal one is exactly the integer with those
/// octets. Under [`Form::Natural`] the decimal goes over as it stands,
/// which is the point of that form.
fn lua_u64(argument: Int, form: Form) -> String {
    match form {
        Form::Bits => format!("0x{:016X}", argument.bits),
        Form::Natural => argument.natural.to_string(),
    }
}

// -- OCaml --------------------------------------------------------------

fn ocaml(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    // `banner` writes a line comment per line and OCaml's only comment is
    // bracketed, so each line is closed.
    let mut out: String = banner("(*", "endpoints/ocaml/zerodds.ml")
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::from("\n")
            } else {
                format!("{line} *)\n")
            }
        })
        .collect();
    out.push_str("\nlet () =\n");
    let _ = writeln!(
        out,
        "  let w = Zerodds.Wire.writer Zerodds.Wire.{} Zerodds.Wire.{} in",
        if big_endian { "BE" } else { "LE" },
        match program.version {
            Version::Xcdr1 => "XCDR1",
            Version::Xcdr2 => "XCDR2",
        }
    );
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("  Zerodds.Wire.put_u8 w ({});\n", v.text(form)),
            Op::Bool(v) => format!("  Zerodds.Wire.put_bool w {v};\n"),
            Op::Char(v) => format!("  Zerodds.Wire.put_char w ({});\n", v.text(form)),
            Op::Wchar(v) => format!("  Zerodds.Wire.put_wchar w ({});\n", v.text(form)),
            Op::U16(v) => format!("  Zerodds.Wire.put_u16 w ({});\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("  Zerodds.Wire.put_u32 w ({});\n", v.text(form)),
            // `put_u64` takes an `Int64` and the `L` suffix is the literal
            // for one. OCaml's own `int` is 63 bits and could not hold the
            // top bit anyway. Under the natural form the suffix stays on,
            // which is how a value outside `Int64` becomes a compile error
            // rather than a quiet wrap — the measurement.
            Op::U64(v) => format!("  Zerodds.Wire.put_u64 w ({}L);\n", ocaml_i64(*v, form)),
            Op::F32(v) => format!("  Zerodds.Wire.put_f32 w ({});\n", float_text(&v.text)),
            Op::F64(v) => format!("  Zerodds.Wire.put_f64 w ({});\n", real(*v)),
            Op::Str(bytes) => format!("  Zerodds.Wire.put_string w \"{}\";\n", escaped(bytes)),
            Op::SeqU8(bytes) => format!(
                "  Zerodds.Wire.put_seq_u8 w (Bytes.of_string \"{}\");\n",
                escaped(bytes)
            ),
            Op::Wstr(units) => format!(
                "  Zerodds.Wire.put_wstring w [|{}|];\n",
                units
                    .iter()
                    .map(|unit| format!(" 0x{unit:04X};"))
                    .collect::<String>()
            ),
            // `put_bitset` takes an `Int64`, so the pattern goes over as
            // the signed decimal it is, for the reason `put_u64` does.
            Op::Bitset { bits, anchor } => format!(
                "  Zerodds.Wire.put_bitset w ({}L) {anchor};\n",
                *bits as i64
            ),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "  Zerodds.Wire.put_fixed w \"{}\" {digits} {scale};\n",
                escaped(value.as_bytes())
            ),
            // `let … in` and not a mutable cell: OCaml has no assignment
            // without a `ref`, and the body of the binding is everything
            // that follows it, so a header opened here is in scope until
            // the program ends. A number released on close is shadowed
            // by the next header that takes it, never read after that.
            Op::DHeaderBegin => format!("  let b{at} = Zerodds.Wire.dheader_begin w in\n"),
            Op::DHeaderEnd => format!("  Zerodds.Wire.dheader_end w b{at};\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "  let b{at} = Zerodds.Wire.emheader_begin w {id} {must_understand} {} in\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("  Zerodds.Wire.emheader_end w b{at} {};\n", code(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("  let p{at} = Zerodds.Wire.pid_begin w {pid} {must_understand} in\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!(
                "  let p{at} = Zerodds.Wire.pid_extended_begin w {member_id} \
                 {must_understand} in\n"
            ),
            Op::PidEnd => format!("  Zerodds.Wire.pid_end w p{at};\n"),
            Op::PidListEnd => "  Zerodds.Wire.pid_list_end w;\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "  let oc = open_out_bin \"out.bin\" in\n\
         \x20 output_bytes oc (Zerodds.Wire.bytes w);\n\
         \x20 close_out oc\n",
    );
    Ok(out)
}

/// OCaml's 64-bit argument, before the `L`.
///
/// The bit pattern goes over as the signed decimal it is, because
/// `Int64` is the only 64-bit integer OCaml has and `18446744073709551614L`
/// is outside it — `Integer literal exceeds the range of representable
/// integers of type int64`, which is a build failure and not octets.
fn ocaml_i64(argument: Int, form: Form) -> String {
    match form {
        Form::Bits => (argument.bits as i64).to_string(),
        Form::Natural => argument.natural.to_string(),
    }
}

// -- Elixir -------------------------------------------------------------

fn elixir(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("#", "endpoints/elixir/lib/zerodds.ex");
    let _ = writeln!(
        out,
        "w = ZeroDDS.Wire.writer(:{}, :{})",
        if big_endian { "big" } else { "little" },
        match program.version {
            Version::Xcdr1 => "xcdr1",
            Version::Xcdr2 => "xcdr2",
        }
    );
    // `w = w` with an empty pipeline is a rebinding Elixir warns about
    // and this probe compiles with `--warnings-as-errors`. The corpus
    // holds aggregates with no members, so the block is written only
    // when there is something to pipe through it — and a framing call
    // ends the block, because it answers with a tuple and a pipeline
    // cannot destructure one.
    let mut piped = false;
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let step = match op {
            Op::U8(v) => Some(format!("  |> ZeroDDS.Wire.put_u8({})\n", v.text(form))),
            Op::Bool(v) => Some(format!("  |> ZeroDDS.Wire.put_bool({v})\n")),
            Op::Char(v) => Some(format!("  |> ZeroDDS.Wire.put_char({})\n", v.text(form))),
            Op::Wchar(v) => Some(format!("  |> ZeroDDS.Wire.put_wchar({})\n", v.text(form))),
            Op::U16(v) => Some(format!("  |> ZeroDDS.Wire.put_u16({})\n", v.text(form))),
            Op::U32(v) | Op::Count(v) => {
                Some(format!("  |> ZeroDDS.Wire.put_u32({})\n", v.text(form)))
            }
            Op::U64(v) => Some(format!("  |> ZeroDDS.Wire.put_u64({})\n", v.text(form))),
            Op::F32(v) => Some(format!(
                "  |> ZeroDDS.Wire.put_f32({})\n",
                float_text(&v.text)
            )),
            Op::F64(v) => Some(format!("  |> ZeroDDS.Wire.put_f64({})\n", real(*v))),
            // A binary literal and not `"…"`: Elixir's string is UTF-8
            // and the corpus carries strings whose octets are not.
            Op::Str(bytes) => Some(format!(
                "  |> ZeroDDS.Wire.put_string(<<{}>>)\n",
                octet_list(bytes)
            )),
            Op::SeqU8(bytes) => Some(format!(
                "  |> ZeroDDS.Wire.put_seq_u8(<<{}>>)\n",
                octet_list(bytes)
            )),
            Op::Wstr(units) => Some(format!(
                "  |> ZeroDDS.Wire.put_wstring([{}])\n",
                unit_list(units)
            )),
            Op::Bitset { bits, anchor } => {
                Some(format!("  |> ZeroDDS.Wire.put_bitset({bits}, {anchor})\n"))
            }
            Op::Fixed {
                value,
                digits,
                scale,
            } => Some(format!(
                "  |> ZeroDDS.Wire.put_fixed(<<{}>>, {digits}, {scale})\n",
                octet_list(value.as_bytes())
            )),
            Op::PidListEnd => Some("  |> ZeroDDS.Wire.pid_list_end()\n".to_string()),
            Op::DHeaderBegin
            | Op::DHeaderEnd
            | Op::EmHeaderBegin { .. }
            | Op::EmHeaderEnd { .. }
            | Op::PidBegin { .. }
            | Op::PidExtendedBegin { .. }
            | Op::PidEnd => None,
        };
        match step {
            Some(step) => {
                if !piped {
                    out.push_str("\nw =\n  w\n");
                    piped = true;
                }
                out.push_str(&step);
            }
            None => {
                piped = false;
                out.push('\n');
                let line = match op {
                    Op::DHeaderBegin => {
                        format!("{{w, b{at}}} = ZeroDDS.Wire.dheader_begin(w)\n")
                    }
                    Op::DHeaderEnd => format!("w = ZeroDDS.Wire.dheader_end(w, b{at})\n"),
                    Op::EmHeaderBegin {
                        id,
                        must_understand,
                        lc,
                    } => format!(
                        "{{w, b{at}}} = ZeroDDS.Wire.emheader_begin(w, {id}, \
                         {must_understand}, {})\n",
                        code(*lc)
                    ),
                    Op::EmHeaderEnd { lc } => {
                        format!("w = ZeroDDS.Wire.emheader_end(w, b{at}, {})\n", code(*lc))
                    }
                    Op::PidBegin {
                        pid,
                        must_understand,
                    } => format!(
                        "{{w, p{at}}} = ZeroDDS.Wire.pid_begin(w, {pid}, {must_understand})\n"
                    ),
                    Op::PidExtendedBegin {
                        member_id,
                        must_understand,
                    } => format!(
                        "{{w, p{at}}} = ZeroDDS.Wire.pid_extended_begin(w, {member_id}, \
                         {must_understand})\n"
                    ),
                    _ => format!("w = ZeroDDS.Wire.pid_end(w, p{at})\n"),
                };
                out.push_str(&line);
            }
        }
    }
    out.push_str("\nFile.write!(\"out.bin\", ZeroDDS.Wire.bytes(w))\n");
    Ok(out)
}

// -- Python -------------------------------------------------------------

fn python(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("#", "endpoints/python/zerodds_wire.py");
    out.push_str("import zerodds_wire as z\n\n");
    let _ = writeln!(
        out,
        "w = z.Writer(z.{}, z.{})",
        if big_endian { "BE" } else { "LE" },
        match program.version {
            Version::Xcdr1 => "XCDR1",
            Version::Xcdr2 => "XCDR2",
        }
    );
    // The framing locals need no declaration: a Python name exists from
    // its first assignment, and the opening call always precedes the
    // closing one.
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("w.put_u8({})\n", v.text(form)),
            Op::Bool(v) => format!("w.put_bool({})\n", if *v { "True" } else { "False" }),
            Op::Char(v) => format!("w.put_char({})\n", v.text(form)),
            Op::Wchar(v) => format!("w.put_wchar({})\n", v.text(form)),
            Op::U16(v) => format!("w.put_u16({})\n", v.text(form)),
            Op::U32(v) | Op::Count(v) => format!("w.put_u32({})\n", v.text(form)),
            Op::U64(v) => format!("w.put_u64({})\n", v.text(form)),
            // The corpus decimal as written, not a binary32 narrowed
            // here: Python has no 32-bit float — like Lua, OCaml and
            // Elixir — so the narrowing is the SDK's `struct.pack("<f")`
            // and handing over anything else would measure this
            // generator's narrowing instead.
            Op::F32(v) => format!("w.put_f32({})\n", float_text(&v.text)),
            Op::F64(v) => format!("w.put_f64({})\n", real(*v)),
            // A bytes literal and not a str: the SDK takes either and
            // writes the octets, and `\xNN` keeps them out of the source
            // file's own encoding.
            Op::Str(bytes) => format!("w.put_string(b\"{}\")\n", escaped(bytes)),
            Op::SeqU8(bytes) => format!("w.put_seq_u8(b\"{}\")\n", escaped(bytes)),
            Op::Wstr(units) => format!("w.put_wstring([{}])\n", unit_list(units)),
            Op::Bitset { bits, anchor } => format!("w.put_bitset({bits}, {anchor})\n"),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!("w.put_fixed(\"{value}\", {digits}, {scale})\n"),
            Op::DHeaderBegin => format!("b{at} = w.dheader_begin()\n"),
            Op::DHeaderEnd => format!("w.dheader_end(b{at})\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "b{at} = w.emheader_begin({id}, {}, {})\n",
                if *must_understand { "True" } else { "False" },
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => format!("w.emheader_end(b{at}, {})\n", code(*lc)),
            Op::PidBegin {
                pid,
                must_understand,
            } => format!(
                "p{at} = w.pid_begin({pid}, {})\n",
                if *must_understand { "True" } else { "False" }
            ),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!(
                "p{at} = w.pid_extended_begin({member_id}, {})\n",
                if *must_understand { "True" } else { "False" }
            ),
            Op::PidEnd => format!("w.pid_end(p{at})\n"),
            Op::PidListEnd => "w.pid_list_end()\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str("\nwith open(\"out.bin\", \"wb\") as f:\n    f.write(w.bytes())\n");
    Ok(out)
}

// -- Java ---------------------------------------------------------------

fn java(program: &Program, big_endian: bool, form: Form) -> Result<String, String> {
    let frames = framing(program);
    let mut out = banner("//", "endpoints/java/Zdw.java");
    out.push_str(
        "public final class Probe {\n    public static void main(String[] args) throws java.io.IOException {\n",
    );
    let _ = writeln!(
        out,
        "        Zdw.Writer w = new Zdw.Writer(Zdw.{}, Zdw.{});",
        if big_endian { "BE" } else { "LE" },
        match program.version {
            Version::Xcdr1 => "XCDR1",
            Version::Xcdr2 => "XCDR2",
        }
    );
    // Initialised, unlike Kotlin's: javac has no warning for a redundant
    // initialiser, and `= 0` / `= null` keeps definite assignment out of
    // the flow analysis entirely.
    for at in &frames.marks {
        let _ = writeln!(out, "        int b{at} = 0;");
    }
    for at in &frames.pids {
        let _ = writeln!(out, "        Zdw.Pid p{at} = null;");
    }
    for (index, op) in program.ops.iter().enumerate() {
        let at = frames.at(index);
        let line = match op {
            Op::U8(v) => format!("        w.u8({});\n", v.text(form)),
            Op::Bool(v) => format!("        w.bool({v});\n"),
            Op::Char(v) => format!("        w.putChar({});\n", v.text(form)),
            Op::Wchar(v) => format!("        w.putWchar({});\n", v.text(form)),
            Op::U16(v) => format!("        w.u16({});\n", v.text(form)),
            // `u32(long)` takes the value with an `L`; `u64(long)` takes
            // the **pattern** as a signed two's complement, because
            // Java's widest integer is signed — the same conversion the
            // Kotlin renderer makes, and the same `Long.MIN_VALUE`
            // special case, so the one shared helper serves both.
            Op::U32(v) | Op::Count(v) => format!("        w.u32({}L);\n", v.text(form)),
            Op::U64(v) => format!("        w.u64({});\n", kotlin_long(*v, form)),
            Op::F32(v) => format!("        w.f32({}f);\n", real32(v.value)),
            Op::F64(v) => format!("        w.f64({});\n", real(*v)),
            Op::Str(bytes) => format!("        w.str({});\n", utf16_literal(bytes, false)?),
            Op::SeqU8(bytes) => format!("        w.seqU8({});\n", java_bytes(bytes)),
            Op::Wstr(units) => format!(
                "        w.putWstring(new int[] {{{}}});\n",
                unit_list(units)
            ),
            Op::Bitset { bits, anchor } => format!(
                "        w.putBitset({}, {anchor});\n",
                kotlin_long(Int::new(*bits, 0), Form::Bits)
            ),
            Op::Fixed {
                value,
                digits,
                scale,
            } => format!(
                "        w.putFixed({}, {digits}, {scale});\n",
                utf16_literal(value.as_bytes(), false)?
            ),
            Op::DHeaderBegin => format!("        b{at} = w.dheaderBegin();\n"),
            Op::DHeaderEnd => format!("        w.dheaderEnd(b{at});\n"),
            Op::EmHeaderBegin {
                id,
                must_understand,
                lc,
            } => format!(
                "        b{at} = w.emheaderBegin({id}L, {must_understand}, {});\n",
                code(*lc)
            ),
            Op::EmHeaderEnd { lc } => {
                format!("        w.emheaderEnd(b{at}, {});\n", code(*lc))
            }
            Op::PidBegin {
                pid,
                must_understand,
            } => format!("        p{at} = w.pidBegin({pid}, {must_understand});\n"),
            Op::PidExtendedBegin {
                member_id,
                must_understand,
            } => format!("        p{at} = w.pidExtendedBegin({member_id}L, {must_understand});\n"),
            Op::PidEnd => format!("        w.pidEnd(p{at});\n"),
            Op::PidListEnd => "        w.pidListEnd();\n".to_string(),
        };
        out.push_str(&line);
    }
    out.push_str(
        "        java.io.FileOutputStream f = new java.io.FileOutputStream(\"out.bin\");\n\
         \x20       f.write(w.bytes());\n\
         \x20       f.close();\n\
         \x20   }\n\
         }\n",
    );
    Ok(out)
}

/// A `byte[]` literal. Java's `byte` is signed, so every octet above
/// 0x7F needs the cast written out.
fn java_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "new byte[] {}".to_string();
    }
    let items = bytes
        .iter()
        .map(|byte| format!("(byte) 0x{byte:02X}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("new byte[] {{{items}}}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::ops::Lc;

    fn program(ops: Vec<Op>) -> Program {
        Program {
            ops,
            version: Version::Xcdr2,
        }
    }

    #[test]
    fn a_non_ascii_string_is_escaped_at_the_language_s_own_unit() {
        // "grüße" — the corpus's own multi-byte case, as the octets the
        // walk carries. Kotlin, JavaScript and F# count UTF-16 units;
        // Swift names Unicode scalars. Both spellings encode the same
        // UTF-8 octets, which is what `putString` writes.
        let bytes = "grüße".as_bytes();
        assert_eq!(
            utf16_literal(bytes, false).unwrap(),
            "\"gr\\u00FC\\u00DFe\""
        );
        assert_eq!(scalar_literal(bytes).unwrap(), "\"gr\\u{FC}\\u{DF}e\"");
    }

    #[test]
    fn a_character_outside_the_basic_plane_becomes_a_surrogate_pair() {
        let bytes = "\u{1F600}".as_bytes();
        assert_eq!(utf16_literal(bytes, false).unwrap(), "\"\\uD83D\\uDE00\"");
        assert_eq!(scalar_literal(bytes).unwrap(), "\"\\u{1F600}\"");
    }

    #[test]
    fn kotlin_escapes_the_dollar_that_starts_a_template() {
        assert_eq!(utf16_literal(b"a$b", true).unwrap(), "\"a\\$b\"");
        assert_eq!(utf16_literal(b"a$b", false).unwrap(), "\"a$b\"");
    }

    #[test]
    fn an_unsigned_pattern_above_long_max_reaches_kotlin_as_its_complement() {
        // 16045690984833335998 is `primitive-ulonglong`'s first member.
        // Kotlin's widest integer is signed and has no literal for it;
        // `putU64` shifts with `ushr` and therefore takes the pattern.
        let source = kotlin(
            &program(vec![Op::U64(Int::new(
                16_045_690_984_833_335_998,
                -2_401_053_088_876_215_618,
            ))]),
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(
            source.contains("w.putU64(-2401053088876215618L)"),
            "{source}"
        );
    }

    #[test]
    fn the_one_pattern_kotlin_has_no_literal_for_is_named() {
        // `bitmask-bitbound-64`'s bit 63. `-9223372036854775808L` is
        // unary minus on a literal one past Long.MAX_VALUE and does not
        // compile; the other three languages take the value as written.
        let kt = kotlin(
            &program(vec![Op::U64(Int::new(1 << 63, i128::from(i64::MIN)))]),
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(kt.contains("w.putU64(Long.MIN_VALUE)"), "{kt}");
        assert!(!kt.contains("-9223372036854775808L"), "{kt}");
        // The neighbour still writes out as a decimal.
        let near = kotlin(
            &program(vec![Op::U64(Int::new(
                (1 << 63) + 1,
                i128::from(i64::MIN) + 1,
            ))]),
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(near.contains("w.putU64(-9223372036854775807L)"), "{near}");
    }

    #[test]
    fn the_same_pattern_reaches_node_and_swift_unsigned() {
        let js = node(
            &program(vec![Op::U64(Int::new(
                16_045_690_984_833_335_998,
                -2_401_053_088_876_215_618,
            ))]),
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(js.contains("w.putU64(16045690984833335998n)"), "{js}");
        let sw = swift(
            &program(vec![Op::U64(Int::new(
                16_045_690_984_833_335_998,
                -2_401_053_088_876_215_618,
            ))]),
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(sw.contains("w.putU64(16045690984833335998)"), "{sw}");
    }

    /// Every one of the fifteen now exports a boolean writer, and the
    /// generated program calls it rather than putting the same octet
    /// through `put_u8`. The octet does not move — XTypes 1.3
    /// §7.4.1.1.1 Table 30 gives a `boolean` the one octet `put_u8`
    /// wrote — but the call under test is now the one a caller of that
    /// SDK would write.
    #[test]
    fn every_binding_calls_its_own_boolean_writer() {
        for lang in Language::ALL {
            let source = render(lang, &program(vec![Op::Bool(true)]), false, Form::Bits)
                .expect("a boolean renders");
            let lowered = source.to_lowercase();
            // `w.bool(..)` is the Java SDK's committed spelling of the
            // same writer; the property under test is which writer runs,
            // not how a binding spells it.
            assert!(
                lowered.contains("put_bool")
                    || lowered.contains("putbool")
                    || lowered.contains("w.bool("),
                "{}: no boolean writer in:\n{source}",
                lang.as_str()
            );
            assert!(
                !lowered.contains("put_u8 (w, 1)")
                    && !lowered.contains("putu8(1)")
                    && !lowered.contains("w.u8(1)"),
                "{}: a boolean still goes out through the octet writer:\n{source}",
                lang.as_str()
            );
        }
    }

    /// An IDL `char` calls `put_char` and not `put_u8`. Both write the
    /// same octet for every value either accepts, and only one of them
    /// refuses U+0161 rather than writing `a`.
    #[test]
    fn every_binding_calls_its_own_character_writer() {
        for lang in Language::ALL {
            let source = render(
                lang,
                &program(vec![Op::Char(Int::new(0x7a, 0x7a))]),
                false,
                Form::Bits,
            )
            .expect("a char renders");
            let lowered = source.to_lowercase();
            assert!(
                lowered.contains("put_char") || lowered.contains("putchar"),
                "{}: no character writer in:\n{source}",
                lang.as_str()
            );
        }
    }

    #[test]
    fn an_empty_program_binds_swift_immutably() {
        // `swiftc -warnings-as-errors` fails a `var` that is never
        // mutated, and `empty-struct` is a corpus case.
        let source = swift(&program(Vec::new()), false, Form::Bits).unwrap();
        assert!(
            source.contains("let w = Writer(.little, .xcdr2)"),
            "{source}"
        );
    }

    #[test]
    fn an_empty_octet_sequence_names_its_element_type_where_it_has_to() {
        assert_eq!(fsharp_bytes(&[]), "Array.empty<byte>");
        assert_eq!(swift_bytes(&[]), "[UInt8]()");
        assert_eq!(kotlin_bytes(&[]), "byteArrayOf()");
        assert_eq!(fsharp_units(&[]), "Array.empty<int>");
        assert_eq!(swift_units(&[]), "[UInt16]()");
        assert_eq!(nim_units(&[]), "newSeq[uint16]()");
        assert_eq!(zig_units(&[]), "&[_]u16{}");
        assert_eq!(go_units(&[]), "nil");
        assert_eq!(d_units(&[]), "null");
    }

    /// `double` reaches all fifteen now. It was outside eleven of the
    /// fifteen write surfaces, and a renderer that still refused it
    /// would turn a measurable cell into a `generator-defect`.
    #[test]
    fn every_binding_writes_a_sixty_four_bit_float() {
        for lang in Language::ALL {
            let source = render(
                lang,
                &program(vec![Op::F64(core::f64::consts::PI)]),
                false,
                Form::Bits,
            )
            .expect("a double renders");
            let lowered = source.to_lowercase();
            assert!(
                lowered.contains("put_f64")
                    || lowered.contains("putf64")
                    || lowered.contains("w.f64("),
                "{}: no 64-bit float writer in:\n{source}",
                lang.as_str()
            );
        }
    }

    /// The framing numbering is released on close, so a run of
    /// sequential members declares one local and not one per member.
    #[test]
    fn a_released_number_is_taken_again_by_the_next_header() {
        let ops = (0..4)
            .flat_map(|id| {
                vec![
                    Op::EmHeaderBegin {
                        id,
                        must_understand: false,
                        lc: Lc(2),
                    },
                    Op::U32(Int::new(1, 1)),
                    Op::EmHeaderEnd { lc: Lc(2) },
                ]
            })
            .collect();
        let frames = framing(&program(ops));
        assert_eq!(frames.marks, vec![0]);
        assert!(frames.pids.is_empty());
    }

    /// A parameter header and a delimiter need locals of different
    /// types, so a number used for both gets one declaration of each.
    #[test]
    fn a_parameter_handle_is_a_different_local_from_a_body_start() {
        let frames = framing(&program(vec![
            Op::DHeaderBegin,
            Op::DHeaderEnd,
            Op::PidBegin {
                pid: 3,
                must_understand: true,
            },
            Op::PidEnd,
        ]));
        assert_eq!(frames.marks, vec![0]);
        assert_eq!(frames.pids, vec![0]);
        let source = go(
            &Program {
                ops: vec![
                    Op::DHeaderBegin,
                    Op::DHeaderEnd,
                    Op::PidBegin {
                        pid: 3,
                        must_understand: true,
                    },
                    Op::PidEnd,
                ],
                version: Version::Xcdr1,
            },
            false,
            Form::Bits,
        )
        .unwrap();
        assert!(source.contains("\tvar b0 int\n"), "{source}");
        assert!(source.contains("\tvar p0 zerodds.Pid\n"), "{source}");
    }

    /// The three kinds of open header all count towards the depth: a
    /// `@mutable` member holding an `@appendable` struct has a
    /// parameter header and a delimiter live at the same time.
    #[test]
    fn the_depth_counts_a_parameter_header_too() {
        assert_eq!(
            depth(&program(vec![
                Op::PidBegin {
                    pid: 0,
                    must_understand: false
                },
                Op::DHeaderBegin,
                Op::DHeaderEnd,
                Op::PidEnd,
            ])),
            2
        );
    }
}
