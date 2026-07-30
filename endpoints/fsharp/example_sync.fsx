// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native F# endpoint: a sensor-telemetry publisher
// writes typed Reading samples; a subscriber polls and decodes every field.
// Run: `dotnet fsi example_sync.fsx`

#load "zerodds.fs"

open ZeroDDS

type Reading = { Id: uint32; Value: float32; Label: string }

let marshal (r: Reading) (endian: Endian) =
    let w = Writer(endian)
    w.PutU32(r.Id)
    w.PutF32(r.Value)
    w.PutString(r.Label)
    w.Bytes()

let decodeReading (body: byte[]) =
    let r = Reader(body, LE)
    { Id = r.GetU32(); Value = r.GetF32(); Label = r.GetString() }

let total = 5
let t = memTransport ()
let c = Client(t)

// Publisher: frame + deliver 5 typed readings with varying values.
for i in 0 .. total - 1 do
    c.Write(marshal { Id = 0x1000u + uint32 i; Value = 20.0f + float32 i * 0.5f; Label = sprintf "bay-%02d" i } LE)

// Subscriber: poll; decode every field; stop at total.
let mutable got = 0
let mutable go = true
while got < total && go do
    match c.Poll() with
    | Some body ->
        let r = decodeReading body
        printfn "sync reading %d: id=0x%x value=%.1f label=\"%s\"" got r.Id (float r.Value) r.Label
        got <- got + 1
    | None -> go <- false

if got <> total then
    eprintfn "incomplete"
    exit 1

printfn "ALL OK"
