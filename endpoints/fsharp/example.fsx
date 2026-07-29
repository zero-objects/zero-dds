// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native F# endpoint: sync (poll) and async
// (MailboxProcessor agent). Run with `dotnet fsi example.fsx`.

#load "zerodds.fs"

open ZeroDDS

let sample (id: uint32) (label: string) =
    let w = Writer(LE)
    w.PutU32(id)
    w.PutString(label)
    w.Bytes()

// sync
let t = memTransport ()
let c = Client(t)
c.Write(sample 0x42u "sync-hello")
match c.Poll() with
| Some body ->
    let id = Reader(body, LE).GetU32()
    printfn "sync: received id=0x%x" id
| None -> printfn "sync: nothing"

// async
let t2 = memTransport ()
let w = Client(t2)
for i in 0 .. 2 do
    w.Write(sample (0x100u + uint32 i) "async")
let r = AsyncReader(t2)
for _ in 0 .. 2 do
    let body = r.Recv()
    let id = Reader(body, LE).GetU32()
    printfn "async: received id=0x%x" id

printfn "ALL OK"
