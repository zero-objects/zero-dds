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

printfn "ALL OK"
