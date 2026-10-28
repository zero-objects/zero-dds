// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the native F# endpoint: byte-identity vs the Rust goldens, plus
// sync + async loopback. Run with `dotnet fsi test.fsx` (GOLDEN_DIR=build).

#load "zerodds.fs"

open System
open ZeroDDS

let goldenDir =
    match Environment.GetEnvironmentVariable("GOLDEN_DIR") with
    | null | "" -> "build"
    | d -> d

let fixture endian =
    let w = Writer(endian, XCDR2)
    w.PutU32(0xA1B2C3D4u)
    w.PutU16(0x1234)
    w.PutU8(0x5A)
    w.PutF32(3.5f)
    w.PutU64(0x0102030405060708UL)
    w.PutString("bay-12")
    w.PutSeqU8([| 0xDEuy; 0xADuy; 0xBEuy; 0xEFuy |])
    w.Bytes()

let sampleBody (id: uint32) =
    let w = Writer(LE, XCDR2)
    w.PutU32(id)
    w.PutU16(0)
    w.PutU8(0)
    w.Bytes()

// byte identity
for (endian, file) in [ (LE, "golden_le.bin"); (BE, "golden_be.bin") ] do
    let got = fixture endian
    let golden = IO.File.ReadAllBytes(IO.Path.Combine(goldenDir, file))
    if got <> golden then
        failwithf "%s: not byte-identical (got %d want %d)" file got.Length golden.Length
    printfn "%s: %d bytes byte-identical" file golden.Length

// sync loopback
let t = memTransport ()
let c = Client(t, xrceObjectId 0x001 ObjkDataWriter)
for i in 0 .. 4 do
    c.Write(sampleBody (0x3000u + uint32 i))
for i in 0 .. 4 do
    match c.Poll() with
    | Some body ->
        let id = Reader(body, LE, XCDR2).GetU32()
        if id <> 0x3000u + uint32 i then failwith "sync: id mismatch"
    | None -> failwith "sync: no sample"
printfn "sync loopback: 5 samples OK"

// async loopback (MailboxProcessor agent)
let t2 = memTransport ()
let w = Client(t2, xrceObjectId 0x001 ObjkDataWriter)
for i in 0 .. 4 do
    w.Write(sampleBody (0x1000u + uint32 i))
let r = AsyncReader(t2)
for i in 0 .. 4 do
    let body = r.Recv()
    let id = Reader(body, LE, XCDR2).GetU32()
    if id <> 0x1000u + uint32 i then failwith "async: id mismatch"
printfn "async loopback: 5 samples OK"

// string is UTF-8 with a byte-count length prefix (not the char count).
// "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
let s = "grüße"
let utf8 = [| 0x67uy; 0x72uy; 0xC3uy; 0xBCuy; 0xC3uy; 0x9Fuy; 0x65uy |]
let sw = Writer(LE, XCDR2)
sw.PutString(s)
let wire = sw.Bytes()
let prefix =
    (int wire.[0]) ||| ((int wire.[1]) <<< 8) ||| ((int wire.[2]) <<< 16) ||| ((int wire.[3]) <<< 24)
if prefix <> utf8.Length + 1 then failwithf "length prefix must be UTF-8 byte count + 1, got %d" prefix
if wire.[4 .. 4 + utf8.Length - 1] <> utf8 then failwith "UTF-8 body mismatch"
if int wire.[4 + utf8.Length] <> 0 then failwith "missing NUL terminator"
if wire.Length <> 4 + utf8.Length + 1 then failwithf "total wire length %d" wire.Length
if Reader(wire, LE, XCDR2).GetString() <> s then failwith "roundtrip mismatch"
printfn "string UTF-8: byte-count length prefix + roundtrip OK"

// negative frame vectors: length bounding + malformed reject
let WriterId = xrceObjectId 0x001 ObjkDataWriter

let frameOf seq sample =
    match writeFrame SessionNoKey StreamBestEffort seq WriterId sample with
    | Some frame -> frame
    | None -> failwith "a sample inside 0xFFFF must frame"

let first = frameOf 1 [| 0xAAuy; 0xBBuy; 0xCCuy |]
let second = frameOf 2 [| 0xDDuy; 0xEEuy |]
match readFrame (Array.append first second) with
| Some body when body = [| 0xAAuy; 0xBBuy; 0xCCuy |] -> ()
| _ -> failwith "appended submessage must not leak into sample"
let overlong = Array.copy first
overlong.[6] <- 0xFFuy
overlong.[7] <- 0xFFuy
match readFrame overlong with
| None -> ()
| Some _ -> failwith "over-long length must reject"
match readFrame [| 0x80uy; 0x01uy; 0x00uy; 0x00uy; 0x07uy |] with
| None -> ()
| Some _ -> failwith "truncated header must reject"
// DDS-XRCE 8.3.5: DATA (0x09) is agent->client, the only submessage a client
// receives from a real hub. `readFrame` tested for 0x07 alone, so every one of
// them came back as None and `Client.Poll` reported "nothing arrived" — a
// received sample dropped without a word. The C, Kotlin, Node and Swift SDKs
// all take both ids; this is the vector that holds F# to them.
match
    readFrame
        [| 0x80uy; 0x01uy; 0x01uy; 0x00uy; 0x09uy; 0x01uy; 0x05uy; 0x00uy
           0x00uy; 0x01uy; 0x00uy; 0x15uy; 0xABuy |]
with
| Some body when body = [| 0xABuy |] -> ()
| _ -> failwith "a DATA (0x09) frame from the hub must be read, not dropped"
// An id that is neither still rejects: a reader that took anything would fold a
// HEARTBEAT or an ACKNACK body into a sample.
match
    readFrame
        [| 0x80uy; 0x01uy; 0x01uy; 0x00uy; 0x0Buy; 0x01uy; 0x05uy; 0x00uy
           0x00uy; 0x01uy; 0x00uy; 0x15uy; 0xABuy |]
with
| None -> ()
| Some _ -> failwith "an unknown submessage id must reject"
// A sample the 16-bit submessage_length cannot describe is refused, and the
// refusal is a value the caller has to unwrap. It used to be `Array.empty`,
// which `Client.Write` handed straight to `Deliver`: an empty datagram went out
// and nothing said so. Both halves are held here — the builder answers None,
// and `Client.Write` raises rather than delivering.
match writeFrame SessionNoKey StreamBestEffort 1 WriterId (Array.zeroCreate 0x10000) with
| None -> ()
| Some _ -> failwith "sample > 0xFFFF must be refused"
// A maximal body still frames: the bound is the field's, not one off it.
match writeFrame SessionNoKey StreamBestEffort 1 WriterId (Array.zeroCreate (0xFFFF - XrceBaseSize)) with
| Some frame when frame.Length = 8 + 0xFFFF -> ()
| _ -> failwith "a maximal body must still frame"
let mutable refused = false
let sink = { Deliver = (fun _ -> failwith "an oversize sample must not be delivered")
             Receive = fun () -> None }
try
    Client(sink, WriterId).Write(Array.zeroCreate 0x10000)
with :? System.ArgumentException -> refused <- true
if not refused then failwith "Client.Write must refuse a sample above 0xFFFF"
printfn "negative frame vectors: OK"

// --- the wire surface, against octets this project did not write ---------
//
// Every expected string below is a row of
// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`, which holds what
// Cyclone DDS, RTI Connext, Fast DDS and OpenDDS each produced from their own
// serialiser for the cases under `tools/byte-gate/cases/`. The vendor and the
// row are named at every check, so a change here that agreed with itself and
// not with a foreign implementation fails.
//
// Where the vendors disagree the divergence is written down rather than
// averaged away: a case with three answers is three findings, and this SDK
// follows a named one.
//
// Each operation is exercised at the ends of its range and not only in the
// middle, because that is where the last four defects in these bindings were.

let hex (b: byte[]) =
    String.Concat(Array.map (fun (x: byte) -> x.ToString("x2")) b)

let unhex (s: string) =
    [| for i in 0 .. 2 .. s.Length - 2 -> Convert.ToByte(s.Substring(i, 2), 16) |]

let vector (name: string) (w: Writer) (want: string) =
    let got = hex (w.Bytes())
    if got <> want then failwithf "%s: got %s want %s" name got want

let refuses (name: string) (body: unit -> unit) =
    let mutable refused = false
    try
        body ()
    with :? ArgumentException -> refused <- true
    if not refused then failwithf "%s: accepted what it must refuse" name

// -- the encoding decides MAXALIGN ---------------------------------------
//
// byte-gate `ext_final`: `@final struct Ext { long a; double b; short c; }`
// with a = 7, b = 1.5, c = -3. Cyclone, RTI and Fast DDS agree on all four
// rows. The `double` aligns to eight under XCDR1 and to four under XCDR2, so
// the same sample is 18 octets in one encoding and 14 in the other — which is
// the whole of what the encoding argument buys.
for (endian, encoding, want) in
    [ (LE, XCDR1, "0700000000000000000000000000f83ffdff")
      (LE, XCDR2, "07000000000000000000f83ffdff")
      (BE, XCDR1, "00000007000000003ff8000000000000fffd")
      (BE, XCDR2, "000000073ff8000000000000fffd") ] do
    let w = Writer(endian, encoding)
    w.PutU32(7u)
    w.PutF64(1.5)
    w.PutU16(-3)
    vector "ext_final (cyclone+rti+fastdds)" w want

if Writer(LE, XCDR1).MaxAlign <> 8 || Writer(LE, XCDR2).MaxAlign <> 4 then
    failwith "the encoding does not decide MaxAlign"

// -- DHEADER --------------------------------------------------------------
//
// byte-gate `ext_appendable`: the same members under `@appendable`, which
// XTypes 1.3 §7.3.1.2.1.8 makes the default extensibility. Under XCDR2 a
// DHEADER precedes the body and counts it — 0x0e = 14 over an 18-octet sample.
// Cyclone, RTI and Fast DDS agree on both rows.
for (endian, want) in
    [ (LE, "0e00000007000000000000000000f83ffdff")
      (BE, "0000000e000000073ff8000000000000fffd") ] do
    let w = Writer(endian, XCDR2)
    let d = w.DHeaderBegin()
    w.PutU32(7u)
    w.PutF64(1.5)
    w.PutU16(-3)
    w.DHeaderEnd(d)
    vector "ext_appendable (cyclone+rti+fastdds)" w want

// -- the member header ----------------------------------------------------
//
// byte-gate `ext_mutable` under XCDR2: all three vendors write the same 30
// octets. The member headers are LC2, LC3 and LC1 — the length is in the code
// and no NEXTINT is written for any of them — inside a DHEADER of 26.
for (endian, want) in
    [ (LE, "1a000000000000200700000001000030000000000000f83f02000010fdff")
      (BE, "0000001a2000000000000007300000013ff800000000000010000002fffd") ] do
    let w = Writer(endian, XCDR2)
    let d = w.DHeaderBegin()
    let m0 = w.EmHeaderBegin(0u, false, LC2)
    w.PutU32(7u)
    w.EmHeaderEnd(m0, LC2)
    let m1 = w.EmHeaderBegin(1u, false, LC3)
    w.PutF64(1.5)
    w.EmHeaderEnd(m1, LC3)
    let m2 = w.EmHeaderBegin(2u, false, LC1)
    w.PutU16(-3)
    w.EmHeaderEnd(m2, LC1)
    w.DHeaderEnd(d)
    vector "ext_mutable/XCDR2 (cyclone+rti+fastdds)" w want

// The must-understand flag is bit 31 of the EMHEADER: `mut_mustunderstand`
// under XCDR2 reads `010000a0` for id 1 at LC2 with the flag — Cyclone and RTI
// both write it. Fast DDS writes `01000020` for the same member, i.e. it drops
// the flag; two of three carry it and this SDK carries it.
let mu2 = Writer(LE, XCDR2)
let mu2d = mu2.DHeaderBegin()
let mu2a = mu2.EmHeaderBegin(1u, true, LC2)
mu2.PutU32(7u)
mu2.EmHeaderEnd(mu2a, LC2)
let mu2b = mu2.EmHeaderBegin(2u, false, LC2)
mu2.PutU32(0xFFFFFFFEu)
mu2.EmHeaderEnd(mu2b, LC2)
mu2.DHeaderEnd(mu2d)
vector "mut_mustunderstand/XCDR2 (cyclone+rti; fastdds drops bit 31)" mu2
    "10000000010000a00700000002000020feffffff"

// The top of the range: 0x0FFFFFFF fits and 0x10000000 does not. A truncating
// header would put a different member's id on the wire.
Writer(LE, XCDR2).EmHeaderBegin(0x0FFFFFFFu, false, LC2) |> ignore
refuses "EmHeaderBegin(0x10000000)" (fun () ->
    Writer(LE, XCDR2).EmHeaderBegin(0x10000000u, false, LC2) |> ignore)
refuses "LC2 over a two-octet body" (fun () ->
    let w = Writer(LE, XCDR2)
    let m = w.EmHeaderBegin(0u, false, LC2)
    w.PutU16(1)
    w.EmHeaderEnd(m, LC2))

// -- the PL_CDR1 parameter list -------------------------------------------
//
// byte-gate `ext_mutable` under XCDR1, RTI's row. Three things it pins: the
// `double`'s parameter declares 8 and not 12, because its value aligns against
// its own block; the `short`'s declares 4 and not 2, because the padding that
// brings the next header back to a four-octet boundary is inside the length;
// and the sentinel carries the must-understand flag, so it is 0x7F02.
//
// The vendors do not agree here and the disagreement is the measurement:
// Cyclone writes every parameter in the twelve-octet extended form (56 octets
// for this sample), Fast DDS declares 2 for the `short` and closes with the
// bare 0x3F02 (32 octets, differing from RTI in two places).
for (endian, want) in
    [ (LE, "000004000700000001000800000000000000f83f02000400fdff0000027f0000")
      (BE, "0000000400000007000100083ff800000000000000020004fffd00007f020000") ] do
    let w = Writer(endian, XCDR1)
    let p0 = w.PidBegin(0, false)
    w.PutU32(7u)
    w.PidEnd(p0)
    let p1 = w.PidBegin(1, false)
    w.PutF64(1.5)
    w.PidEnd(p1)
    let p2 = w.PidBegin(2, false)
    w.PutU16(-3)
    w.PidEnd(p2)
    w.PidListEnd()
    vector "ext_mutable/XCDR1 (rti; cyclone extended, fastdds len 2 + 3f02)" w want

// The parameter must-understand flag is bit 14 of the id field: RTI's
// `mut_mustunderstand` row reads `0140` for id 1 with the flag, and Cyclone
// sets the same bit inside its extended header. Fast DDS writes `0100`.
let mu1 = Writer(LE, XCDR1)
let mu1a = mu1.PidBegin(1, true)
mu1.PutU32(7u)
mu1.PidEnd(mu1a)
let mu1b = mu1.PidBegin(2, false)
mu1.PutU32(0xFFFFFFFEu)
mu1.PidEnd(mu1b)
mu1.PidListEnd()
vector "mut_mustunderstand/XCDR1 (rti; fastdds drops bit 14)" mu1
    "014004000700000002000400feffffff027f0000"

// byte-gate `pid_3f00`: `@id(16127)` and `@id(16128)`, i.e. 0x3EFF and 0x3F00.
// RTI and Fast DDS both still use the short header for 0x3F00, so the boundary
// is inclusive — and one past it is refused rather than truncated.
let pe = Writer(LE, XCDR1)
let pea = pe.PidBegin(0x3EFF, false)
pe.PutU32(7u)
pe.PidEnd(pea)
let peb = pe.PidBegin(0x3F00, false)
pe.PutU32(9u)
pe.PidEnd(peb)
pe.PidListEnd()
vector "pid_3f00/XCDR1 (rti+fastdds; cyclone extended)" pe
    "ff3e040007000000003f040009000000027f0000"
refuses "PidBegin(0x3F01)" (fun () -> Writer(LE, XCDR1).PidBegin(0x3F01, false) |> ignore)

// -- the tail is not padded ------------------------------------------------
//
// byte-gate `tail_pad`: `@final struct Odd { long a; short b; }` with 1 and 2
// is six octets in all four rows and all three vendors. A writer that rounded
// the sample up to a four-octet boundary would add two.
for (endian, want) in [ (LE, "010000000200"); (BE, "000000010002") ] do
    for encoding in [ XCDR1; XCDR2 ] do
        let w = Writer(endian, encoding)
        w.PutU32(1u)
        w.PutU16(2)
        vector "tail_pad (cyclone+rti+fastdds)" w want

// -- the wide character and string ----------------------------------------
//
// byte-gate `wchar_member`: `@final struct WCh { wchar c; octet tail; }` with
// U+0061 and 0xAB. Cyclone and Fast DDS write a two-octet code unit in both
// encodings; RTI writes four octets under XCDR1 and two under XCDR2. This SDK
// writes two octets always — the majority form, and RTI's own XCDR2 form.
for (endian, want) in [ (LE, "6100ab"); (BE, "0061ab") ] do
    for encoding in [ XCDR1; XCDR2 ] do
        let w = Writer(endian, encoding)
        w.PutWchar(0x0061)
        w.PutU8(0xAB)
        vector "wchar_member (cyclone+fastdds; rti 4 octets under XCDR1)" w want

Writer(LE, XCDR2).PutWchar(0xFFFF) // the last one that fits
refuses "PutWchar(U+10000)" (fun () -> Writer(LE, XCDR2).PutWchar(0x10000))

// byte-gate `wstring_member`: `wstring w; octet tail;` with the two units
// U+0061 U+0062. Three vendors, three answers, and the length word is the axis
// they split on:
//
//   cyclone  04000000 6100 6200 ab  — the length counts OCTETS
//   fastdds  02000000 6100 6200 ab  — the length counts CODE UNITS
//   rti      03000000 ...           — units incl. a terminator, four octets
//            wide, under XCDR1; under XCDR2 RTI writes the octet count.
//
// This SDK writes the octet count with two-octet units and no terminator.
//
// A fourth axis separates them in a big-endian stream, and only there:
// Cyclone's row is `00000004 6100 6200 ab` — the length word is swapped and the
// code units are not, so its wide units go out little-endian whatever the
// stream says. RTI's row is `00000004 0061 0062 ab`, every word in the stream's
// order. Cyclone swaps a standalone `wchar` (its `wchar_member` row is
// `0061ab`) and does not swap the elements of a `wstring`, which is a
// disagreement inside one vendor. This SDK writes every unit in the stream's
// order — RTI's form, and Cyclone's own form for a `wchar`.
for (endian, want) in
    [ (LE, "0400000061006200ab")   // cyclone + rti agree
      (BE, "0000000400610062ab") ] do // rti; cyclone leaves the units LE
    let w = Writer(endian, XCDR2)
    w.PutWstring([| 0x0061; 0x0062 |])
    w.PutU8(0xAB)
    vector "wstring_member/XCDR2 (rti; fastdds counts units)" w want

// The empty one is a length of zero and nothing after it.
let we = Writer(LE, XCDR2)
we.PutWstring([||])
vector "wstring empty" we "00000000"

// byte-gate `wstring_nonbmp`: U+1D11E, which is one code point and two UTF-16
// code units. Cyclone writes the surrogate pair and a length of four octets.
// Fast DDS writes `010000001ed1ab` — one unit 0xD11E, the low sixteen bits of
// the code point: a silent truncation that produces a frame of the right shape
// holding a different character.
let wn = Writer(LE, XCDR2)
wn.PutWstring([| 0xD834; 0xDD1E |])
wn.PutU8(0xAB)
vector "wstring_nonbmp/LE (cyclone+rti; fastdds truncates to 0xD11E)" wn "0400000034d81eddab"
let wnbe = Writer(BE, XCDR2)
wnbe.PutWstring([| 0xD834; 0xDD1E |])
wnbe.PutU8(0xAB)
vector "wstring_nonbmp/BE (rti; cyclone leaves the units LE)" wnbe "00000004d834dd1eab"

// U+0161's low octet is 0x61, which is `a`: a truncating writer produces a
// frame of the right length holding a plausible, wrong character.
Writer(LE, XCDR2).PutChar(0xFF)
refuses "PutChar(U+0161)" (fun () -> Writer(LE, XCDR2).PutChar(0x0161))

// -- the 64-bit float ------------------------------------------------------
//
// 1.5 is `ext_final`'s `b` and is checked above against three vendors; the rest
// are the ends of the exponent range, where a narrowing to `float32` would show.
for (value, want) in
    [ (1.5, "000000000000f83f")
      (0.0, "0000000000000000")
      (3.141592653589793, "182d4454fb210940")
      (Double.MaxValue, "ffffffffffffef7f")
      (Double.Epsilon, "0100000000000000") ] do // the smallest subnormal
    let w = Writer(LE, XCDR2)
    w.PutF64(value)
    vector (sprintf "PutF64 %f" value) w want

// -- the ends of every integer range ---------------------------------------
//
// A width that truncates is the defect this SDK carried: `PutU16 70000` wrote
// `7011`, the low sixteen bits of 0x11170, with no error anywhere. Each width
// is checked at 0, at -1, at the signed minimum, at the unsigned maximum and
// one past it.
let e8 = Writer(LE, XCDR2)
e8.PutU8(0)
e8.PutU8(255)
e8.PutU8(-128)
e8.PutU8(-1)
vector "PutU8 at its ends" e8 "00ff80ff"
refuses "PutU8 256" (fun () -> Writer(LE, XCDR2).PutU8(256))
refuses "PutU8 -129" (fun () -> Writer(LE, XCDR2).PutU8(-129))

let e16 = Writer(LE, XCDR2)
e16.PutU16(0)
e16.PutU16(65535)
e16.PutU16(-32768)
e16.PutU16(-1)
vector "PutU16 at its ends" e16 "0000ffff0080ffff"
refuses "PutU16 65536" (fun () -> Writer(LE, XCDR2).PutU16(65536))
refuses "PutU16 70000" (fun () -> Writer(LE, XCDR2).PutU16(70000))
refuses "PutU16 -32769" (fun () -> Writer(LE, XCDR2).PutU16(-32769))

// PutU32 and PutU64 take unsigned arguments, so their ends are the type's and
// the compiler refuses one past — the check is that both extremes still write.
let e32 = Writer(LE, XCDR2)
e32.PutU32(0u)
e32.PutU32(UInt32.MaxValue)
vector "PutU32 at its ends" e32 "00000000ffffffff"
let e64 = Writer(LE, XCDR2)
e64.PutU64(0UL)
e64.PutU64(UInt64.MaxValue)
vector "PutU64 at its ends" e64 "0000000000000000ffffffffffffffff"

// -- the packed decimal ----------------------------------------------------
//
// `tests/interop-corpus/fixed-basic` is `@final struct Sample { Money amount;
// Money debt; octet tail; }` over `typedef fixed<9,3> Money`, with "123456.789",
// "-000000.001" and 0xAB. P + 1 = 10 is even, so there is no leading zero nibble
// and each value is five octets.
//
// `fixed<P,S>` is CORBA's and not DDS's, so the foreign octets come from an ORB
// and not from the four DDS vendors: Cyclone, RTI, Fast DDS and OpenDDS each
// refuse the type in their IDL front end, which is why `fixed_basic` carries no
// `wrote` row in vendor_bytes.rows.tsv at all.
//
// omniORB wrote `123456789c000000001dab` in both byte orders
// (tests/interop-results/fixed-basic/omniorb/plain-cdr1.{le,be}.bin), octet for
// octet what this SDK writes — and the two orders being equal is the measurement
// that no multi-octet integer is written here.
//
// JacORB wrote `123456789c1dab` for the same sample, big-endian only: the first
// value agrees and the second is one octet rather than five, so it packs the
// significant digits instead of the declared width. TAO produced no bytes at
// all — `tao_idl` refused the IDL.
let fx = Writer(LE, XCDR1)
fx.PutFixed("123456.789", 9, 3)
fx.PutFixed("-000000.001", 9, 3)
fx.PutU8(0xAB)
vector "fixed-basic/LE (omniorb; jacorb packs the second value to one octet)" fx
    "123456789c000000001dab"
let fxbe = Writer(BE, XCDR1)
fxbe.PutFixed("123456.789", 9, 3)
fxbe.PutFixed("-000000.001", 9, 3)
fxbe.PutU8(0xAB)
vector "fixed-basic/BE (omniorb)" fxbe "123456789c000000001dab"
if hex (fxbe.Bytes()) <> hex (fx.Bytes()) then
    failwith "fixed must not depend on the byte order"

for (value, digits, scale, want) in
    [ ("1", 1, 0, "1c")
      ("-9", 1, 0, "9d")
      ("12", 2, 0, "012c")          // P + 1 odd -> a leading zero nibble
      ("0.123", 3, 3, "123c")       // scale = digits: all fraction
      ("-0", 1, 0, "0c")            // a negative zero is written positive
      ("-0.000", 3, 3, "000c")
      ("7.5", 9, 3, "000007500c")   // padding on both sides at once
      ("1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901c")
      ("-1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901d") ] do
    let w = Writer(LE, XCDR2)
    w.PutFixed(value, digits, scale)
    vector (sprintf "PutFixed %s %d %d" value digits scale) w want
    if (w.Bytes()).Length <> (digits + 2) / 2 then
        failwithf "PutFixed %s: wrong octet count" value

for (value, digits, scale) in
    [ ("1234567", 6, 0)   // seven integer digits into six
      ("1.2345", 6, 3)    // four fraction digits into three
      ("1.0", 2, 3)       // a scale wider than the digit count
      ("twelve", 4, 0)    // not a decimal
      ("", 4, 0) ] do     // no digits at all
    refuses (sprintf "PutFixed %s %d %d" value digits scale) (fun () ->
        Writer(LE, XCDR2).PutFixed(value, digits, scale))

// -- the bitset anchor -----------------------------------------------------
//
// The anchor is written at its own width and therefore at its own alignment,
// which the encoding caps: eight under XCDR1 and four under XCDR2.
let b1 = Writer(LE, XCDR1)
b1.PutU8(0xAB)
b1.PutBitset(1UL, 8)
vector "bitset anchor 8 under XCDR1" b1 "ab000000000000000100000000000000"
let b2 = Writer(LE, XCDR2)
b2.PutU8(0xAB)
b2.PutBitset(1UL, 8)
vector "bitset anchor 8 under XCDR2" b2 "ab0000000100000000000000"
refuses "a three-octet bitset anchor" (fun () -> Writer(LE, XCDR2).PutBitset(0UL, 3))

// -- the reading direction --------------------------------------------------
//
// The vendors' own octets, read back into the sample they were written from:
// `ext_appendable` under XCDR2 in both byte orders.
for (endian, wire) in
    [ (LE, "0e00000007000000000000000000f83ffdff")
      (BE, "0000000e000000073ff8000000000000fffd") ] do
    let bytes = unhex wire
    let r = Reader(bytes, endian, XCDR2)
    if r.DHeaderRead() <> 14u then failwith "ext_appendable: DHEADER"
    if r.GetU32() <> 7u then failwith "ext_appendable: a"
    if r.GetF64() <> 1.5 then failwith "ext_appendable: b"
    if r.GetU16() <> 0xFFFCus + 1us then failwith "ext_appendable: c"
    if r.Pos <> bytes.Length then failwithf "ext_appendable: stopped at %d" r.Pos

// The vendors' `ext_mutable` under XCDR2, through the member header.
let mrBytes = unhex "1a000000000000200700000001000030000000000000f83f02000010fdff"
let mr = Reader(mrBytes, LE, XCDR2)
if mr.DHeaderRead() <> 26u then failwith "ext_mutable: DHEADER"
for (id, lc, extent) in [ (0u, LC2, 4u); (1u, LC3, 8u); (2u, LC1, 2u) ] do
    let m = mr.EmHeaderRead()
    if m.Id <> id || m.LengthCode <> lc || m.Extent <> extent then
        failwithf "ext_mutable: id=%d lc=%d extent=%d" m.Id m.LengthCode m.Extent
    mr.Pos <- mr.Pos + int m.Extent
if mr.Pos <> mrBytes.Length then failwithf "ext_mutable: stopped at %d" mr.Pos

// RTI's `ext_mutable` under XCDR1, through the parameter list.
let prBytes = unhex "000004000700000001000800000000000000f83f02000400fdff0000027f0000"
let pr = Reader(prBytes, LE, XCDR1)
for (id, length) in [ (0u, 4u); (1u, 8u); (2u, 4u) ] do
    let p = pr.PidRead()
    if p.ListEnd || p.Id <> id || p.Length <> length then
        failwithf "ext_mutable/XCDR1: id=%d len=%d" p.Id p.Length
    pr.PidSkip(pr.Pos, p.Length)
if not (pr.PidRead()).ListEnd then failwith "ext_mutable/XCDR1: no sentinel"
if pr.Pos <> prBytes.Length then failwithf "ext_mutable/XCDR1: stopped at %d" pr.Pos

// Every write reads back, in both encodings and both byte orders.
for encoding in [ XCDR1; XCDR2 ] do
    for endian in [ LE; BE ] do
        let w = Writer(endian, encoding)
        w.PutU8(0xFF)
        w.PutBool(true)
        w.PutChar(0xFF)
        w.PutU16(0xFFFF)
        w.PutU32(UInt32.MaxValue)
        w.PutU64(UInt64.MaxValue)
        w.PutF32(-3.5f)
        w.PutF64(3.141592653589793)
        w.PutWchar(0xFFFF)
        w.PutString("grüße")
        w.PutSeqU8([| 0xDEuy; 0xADuy |])
        w.PutWstring([| 0xD83D; 0xDE00 |])
        w.PutBitset(0xBC614E00000FFFFFUL, 8)
        w.PutFixed("-000000.001", 9, 3)

        let r = Reader(w.Bytes(), endian, encoding)
        if r.GetU8() <> 0xFFuy then failwith "roundtrip GetU8"
        if not (r.GetBool()) then failwith "roundtrip GetBool"
        if r.GetChar() <> 0xFF then failwith "roundtrip GetChar"
        if r.GetU16() <> UInt16.MaxValue then failwith "roundtrip GetU16"
        if r.GetU32() <> UInt32.MaxValue then failwith "roundtrip GetU32"
        if r.GetU64() <> UInt64.MaxValue then failwith "roundtrip GetU64"
        if r.GetF32() <> -3.5f then failwith "roundtrip GetF32"
        if r.GetF64() <> 3.141592653589793 then failwith "roundtrip GetF64"
        if r.GetWchar() <> 0xFFFF then failwith "roundtrip GetWchar"
        if r.GetString() <> "grüße" then failwith "roundtrip GetString"
        if hex (r.GetSeqU8()) <> "dead" then failwith "roundtrip GetSeqU8"
        if r.GetWstring() <> [| 0xD83D; 0xDE00 |] then failwith "roundtrip GetWstring"
        if r.GetBitset(8) <> 0xBC614E00000FFFFFUL then failwith "roundtrip GetBitset"
        if r.GetFixed(9, 3) <> "-000000.001" then failwith "roundtrip GetFixed"
        if r.Pos <> (w.Bytes()).Length then failwithf "roundtrip stopped at %d" r.Pos

printfn "surface: foreign octets — cyclone, rti and fastdds over 9 byte-gate cases, omniORB over fixed-basic"

printfn "ALL OK"
