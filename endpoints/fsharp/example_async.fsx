// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native F# endpoint: the same telemetry publisher,
// but the subscriber consumes via the MailboxProcessor agent (RecvAsync) and
// decodes every field. Run: `dotnet fsi example_async.fsx`

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
let w = Client(t)

// Publisher.
for i in 0 .. total - 1 do
    w.Write(marshal { Id = 0x2000u + uint32 i; Value = 100.0f - float32 i; Label = sprintf "sensor-%02d" i } LE)

// Subscriber: await the agent per sample; decode; stop at total.
let reader = AsyncReader(t)

let run =
    async {
        for got in 0 .. total - 1 do
            let! body = reader.RecvAsync()
            let r = decodeReading body
            printfn "async reading %d: id=0x%x value=%.1f label=\"%s\"" got r.Id (float r.Value) r.Label
    }

run |> Async.RunSynchronously
printfn "ALL OK"
