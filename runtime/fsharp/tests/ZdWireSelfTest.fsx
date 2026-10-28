// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A self-test of the F# wire runtime against fixed octets, one per HDDS
// axis of `Dialect::Hdds`. Expected octets from the recorded columns:
//
//   tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
//     (packed elements),
//   tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
//     (a sequence element's own DHEADER length),
//   tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
//   tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
//   tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
//     (a wstring as its own UTF-8 octets).
//
// Run with `dotnet fsi --exec tests/ZdWireSelfTest.fsx` from
// runtime/fsharp. Exit 0 on agreement, exit 1 with every disagreement
// named.

#load "../ZdWire.fs"

open ZdWire

let hex (b: byte[]) =
    b |> Array.map (fun x -> sprintf "%02x" x) |> String.concat ""

let mutable failed = false

let check (name: string) (ok: bool) =
    if not ok then
        eprintfn "ZDWireSelfTest: %s" name
        failed <- true

let writer () = Writer(LE, XCDR2, specificationFraming, false)

// Each check is its own function: a script's top-level `let` may not
// name a value twice (FS0037), and the checks reuse `w`, `r` and `saved`.

// ElementPadding::Packed: each collection element counts its alignment
// from its own first octet. The second Cell's `long` sits at offset six,
// where the clause would put it at eight.
let packedWrite () =
    let w = writer ()
    let saved = w.BeginPackedElement()
    w.WriteI32(1)
    w.WriteI16(2s)
    w.FinishPackedElement(saved)
    let saved = w.BeginPackedElement()
    w.WriteI32(3)
    w.WriteI16(4s)
    w.FinishPackedElement(saved)
    w.WriteU8(119uy)
    check "a packed element starts where the last ended" (hex (w.Finish()) = "01000000020003000000040077")

// And back through the reader's packed pair.
let packedRead () =
    let r =
        Reader([| 0x01uy; 0x00uy; 0x00uy; 0x00uy; 0x02uy; 0x00uy; 0x03uy; 0x00uy; 0x00uy; 0x00uy; 0x04uy; 0x00uy; 0x77uy |], LE, XCDR2)
    let saved = r.ReaderBeginPackedElement()
    let a1 = r.ReadI32()
    let b1 = r.ReadI16()
    r.ReaderFinishPackedElement(saved)
    let saved = r.ReaderBeginPackedElement()
    let a2 = r.ReadI32()
    let b2 = r.ReadI16()
    r.ReaderFinishPackedElement(saved)
    check "a packed element reads back" (a1 = 1 && b1 = 2s && a2 = 3 && b2 = 4s && r.ReadU8() = 119uy)

// SequenceElementDelimiter::BeforeEachNonFinalElement: an element's own
// octet length in DHEADER form, 06 00 00 00 for the six-octet Cell that
// follows.
let elementLength () =
    let w = writer ()
    let d = w.WriteDheader()
    w.WriteI32(1)
    w.WriteI16(2s)
    w.FinishDheader(d)
    check "a sequence element's own length in DHEADER form" (hex (w.Finish()) = "06000000010000000200")

// WstringCodeUnits::Utf8: three octets and a NUL, the length 4.
let utf8Wstring () =
    let w = writer ()
    w.WriteWstringUtf8Octets("abc", "octets/terminated/2")
    check "a UTF-8 wstring is its octets and a NUL" (hex (w.Finish()) = "0400000061626300")
    let w = writer ()
    w.WriteWstringUtf8Octets("\u00e4\u00f6\u00fc \u1234 Z", "octets/terminated/2")
    check "a multi-byte UTF-8 wstring counts its octets" (hex (w.Finish()) = "0d000000c3a4c3b6c3bc20e188b4205a00")
    let w = writer ()
    w.WriteWstringUtf8OctetsBounded("\u1234\u00e4Z", 3u, "octets/terminated/2")
    check "a bounded UTF-8 wstring holds three units" (hex (w.Finish()) = "07000000e188b4c3a45a00")
    let r =
        Reader(
            [| 0x0duy; 0x00uy; 0x00uy; 0x00uy; 0xc3uy; 0xa4uy; 0xc3uy; 0xb6uy; 0xc3uy; 0xbcuy;
               0x20uy; 0xe1uy; 0x88uy; 0xb4uy; 0x20uy; 0x5auy; 0x00uy |],
            LE,
            XCDR2
        )
    check "a UTF-8 wstring reads back" (r.ReadWstringUtf8Octets("octets/terminated/2") = "\u00e4\u00f6\u00fc \u1234 Z")

packedWrite ()
packedRead ()
elementLength ()
utf8Wstring ()

if failed then
    exit 1

printfn "ZDWireSelfTest: ok"
