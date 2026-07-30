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
    let w = Writer(endian)
    w.PutU32(0xA1B2C3D4u)
    w.PutU16(0x1234)
    w.PutU8(0x5A)
    w.PutF32(3.5f)
    w.PutU64(0x0102030405060708UL)
    w.PutString("bay-12")
    w.PutSeqU8([| 0xDEuy; 0xADuy; 0xBEuy; 0xEFuy |])
    w.Bytes()

let sampleBody (id: uint32) =
    let w = Writer(LE)
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
let c = Client(t)
for i in 0 .. 4 do
    c.Write(sampleBody (0x3000u + uint32 i))
for i in 0 .. 4 do
    match c.Poll() with
    | Some body ->
        let id = Reader(body, LE).GetU32()
        if id <> 0x3000u + uint32 i then failwith "sync: id mismatch"
    | None -> failwith "sync: no sample"
printfn "sync loopback: 5 samples OK"

// async loopback (MailboxProcessor agent)
let t2 = memTransport ()
let w = Client(t2)
for i in 0 .. 4 do
    w.Write(sampleBody (0x1000u + uint32 i))
let r = AsyncReader(t2)
for i in 0 .. 4 do
    let body = r.Recv()
    let id = Reader(body, LE).GetU32()
    if id <> 0x1000u + uint32 i then failwith "async: id mismatch"
printfn "async loopback: 5 samples OK"

// string is UTF-8 with a byte-count length prefix (not the char count).
// "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
let s = "grüße"
let utf8 = [| 0x67uy; 0x72uy; 0xC3uy; 0xBCuy; 0xC3uy; 0x9Fuy; 0x65uy |]
let sw = Writer(LE)
sw.PutString(s)
let wire = sw.Bytes()
let prefix =
    (int wire.[0]) ||| ((int wire.[1]) <<< 8) ||| ((int wire.[2]) <<< 16) ||| ((int wire.[3]) <<< 24)
if prefix <> utf8.Length + 1 then failwithf "length prefix must be UTF-8 byte count + 1, got %d" prefix
if wire.[4 .. 4 + utf8.Length - 1] <> utf8 then failwith "UTF-8 body mismatch"
if int wire.[4 + utf8.Length] <> 0 then failwith "missing NUL terminator"
if wire.Length <> 4 + utf8.Length + 1 then failwithf "total wire length %d" wire.Length
if Reader(wire, LE).GetString() <> s then failwith "roundtrip mismatch"
printfn "string UTF-8: byte-count length prefix + roundtrip OK"

// negative frame vectors: length bounding + malformed reject
let first = writeFrame SessionNoKey StreamBestEffort 1 [| 0xAAuy; 0xBBuy; 0xCCuy |]
let second = writeFrame SessionNoKey StreamBestEffort 2 [| 0xDDuy; 0xEEuy |]
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
if writeFrame SessionNoKey StreamBestEffort 1 (Array.zeroCreate 0x10000) <> Array.empty then
    failwith "sample > 0xFFFF must be refused"
printfn "negative frame vectors: OK"

printfn "ALL OK"
