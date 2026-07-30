// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable endpoint demo (DDS-XRCE reliable stream), mirroring
// endpoints/csharp/ExampleReliable.cs and endpoints/rust/examples/example_reliable.rs. Modes:
//   ExampleReliable <port>  -- reliable SENDER to 127.0.0.1:<port> (used by the
//                              endpoint-e2e loss-recovery test). Enqueues N
//                              samples through the async-decoupled writer; the
//                              drain thread retransmits on ACKNACK until the
//                              window drains, then exits.
//   ExampleReliable bench   -- producer latency: non-blocking Enqueue
//                              (decoupled) vs inline Socket.Send, ns/op.
//   ExampleReliable         -- standalone in-process demo: a lossy receiver
//                              thread + the decoupled sender; the receiver prints
//                              the recovered contiguous sequence.

module ExampleReliable

open System
open System.Collections.Generic
open System.Diagnostics
open System.Net
open System.Net.Sockets
open System.Threading
open ZeroDDS
open ZeroDDSReliable

let private N = 12

// Reading { uint32 id; float value; string label } -- the same shape as the
// Rust/C# example's `Reading`, marshalled with the pure-F# XCDR2 writer
// (ZeroDDS.Writer), byte-identical to the Rust core.
let private sample (i: int) =
    let w = Writer(LE)
    w.PutU32(uint32 i)
    w.PutF32(float32 i)
    w.PutString(sprintf "s%d" i)
    w.Bytes()

/// E2E sender: enqueue N samples through the decoupled writer, drain, exit.
let private sendToPeer (port: int) =
    let sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    sock.Connect(IPEndPoint(IPAddress.Loopback, port))
    let writer = AsyncReliableWriter.Start(sock, 64)
    for i in 0 .. N - 1 do
        let item = sample i
        while not (writer.Handle.Enqueue(item)) do
            Thread.Yield() |> ignore // ring full -> spin
    writer.Shutdown() // retransmits until the window drains, then joins
    printfn "SENT %d reliable samples" N
    0

/// Producer latency: decoupled Enqueue (channel write, no I/O) vs. inline
/// Socket.Send (a syscall on the producer thread). `iters` stays below the ring
/// capacity so Enqueue never hits backpressure -- this measures the pure producer
/// cost, not the drain thread.
let private bench () =
    let iters = 50_000

    // Decoupled: the drain thread consumes; a sink thread empties the socket so
    // sends never block.
    let sink = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    sink.Bind(IPEndPoint(IPAddress.Loopback, 0))
    let sinkPort = (sink.LocalEndPoint :?> IPEndPoint).Port
    let sinkThread =
        Thread(fun () ->
            sink.ReceiveTimeout <- 200
            let buf = Array.zeroCreate<byte> 2048
            let mutable go = true
            while go do
                try
                    sink.Receive(buf) |> ignore
                with :? SocketException ->
                    go <- false)
    sinkThread.IsBackground <- true
    sinkThread.Start()

    let dsock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    dsock.Connect(IPEndPoint(IPAddress.Loopback, sinkPort))
    let writer = AsyncReliableWriter.Start(dsock, 1 <<< 20)
    let s = sample 1
    let sw = Stopwatch.StartNew()
    for _ in 0 .. iters - 1 do
        while not (writer.Handle.Enqueue(s)) do
            Thread.Yield() |> ignore
    let enqueueNs = sw.ElapsedTicks * 100L / int64 iters // 1 Stopwatch tick == 100ns
    writer.Shutdown()

    // Inline: marshal already done; a syscall per iteration on this thread.
    let isock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    isock.Connect(IPEndPoint(IPAddress.Loopback, sinkPort))
    let sw2 = Stopwatch.StartNew()
    for i in 0 .. iters - 1 do
        isock.Send(Wire.writeFrame (uint16 i) s) |> ignore
    let inlineNs = sw2.ElapsedTicks * 100L / int64 iters

    printfn "LATENCY enqueue(decoupled)=%dns inline(send)=%dns" enqueueNs inlineNs
    0

/// In-process demo: a lossy receiver (drops every 3rd distinct sample once) + the
/// decoupled sender; the receiver prints the recovered contiguous sequence.
let private standaloneDemo () =
    let rx = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    rx.Bind(IPEndPoint(IPAddress.Loopback, 0))
    let rxPort = (rx.LocalEndPoint :?> IPEndPoint).Port

    let delivered = List<uint32>()
    let receiver =
        Thread(fun () ->
            rx.ReceiveTimeout <- 200
            let state = ReliableReceiver()
            let droppedOnce = HashSet<uint16>()
            let mutable count = 0
            let mutable lastFrom: EndPoint option = None
            let deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10.0)
            let buf = Array.zeroCreate<byte> 2048
            while delivered.Count < N && DateTime.UtcNow < deadline do
                let mutable from: EndPoint = IPEndPoint(IPAddress.Any, 0)
                let n =
                    try
                        rx.ReceiveFrom(buf, &from)
                    with :? SocketException ->
                        -1
                if n >= 0 then
                    lastFrom <- Some from
                    let frame = buf.[0 .. n - 1]
                    match Wire.tryUnframeWrite frame with
                    | Some(seq, body) ->
                        count <- count + 1
                        // Drop every 3rd distinct sample once, forcing one retransmit.
                        if count % 3 = 0 && droppedOnce.Add(seq) then
                            ()
                        else
                            state.RecvData(seq, body) |> ignore
                            for (_, payload) in state.DrainInOrder() do
                                delivered.Add(Reader(payload, LE).GetU32())
                            rx.SendTo(Wire.ackNackFrame (state.PendingAckNack()) 1us, from) |> ignore
                    | None ->
                        match Wire.tryParseHeartbeat frame with
                        | Some _ -> rx.SendTo(Wire.ackNackFrame (state.PendingAckNack()) 1us, from) |> ignore
                        | None -> ()
            // Final all-acked ACKNACKs so the sender's window drains
            // deterministically instead of relying on its close-deadline.
            match lastFrom with
            | Some addr ->
                for _ in 0..2 do
                    rx.SendTo(Wire.ackNackFrame (state.PendingAckNack()) 1us, addr) |> ignore
            | None -> ())
    receiver.IsBackground <- true
    receiver.Start()

    let sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    sock.Connect(IPEndPoint(IPAddress.Loopback, rxPort))
    let writer = AsyncReliableWriter.Start(sock, 64)
    for i in 0 .. N - 1 do
        let item = sample i
        while not (writer.Handle.Enqueue(item)) do
            Thread.Yield() |> ignore
    writer.Shutdown()
    receiver.Join(TimeSpan.FromSeconds(15.0)) |> ignore

    printfn "RECOVERED [%s]" (String.Join(", ", delivered))
    let mutable ok = delivered.Count = N
    let mutable i = 0
    while ok && i < N do
        ok <- ok && delivered.[i] = uint32 i
        i <- i + 1
    if not ok then
        eprintfn "MISMATCH: expected 0..%d gap-free in order" (N - 1)
        1
    else
        printfn "OK: %d samples delivered gap-free in order despite injected loss" N
        0

[<EntryPoint>]
let main argv =
    if argv.Length >= 1 && argv.[0] = "bench" then
        bench ()
    elif argv.Length >= 1 then
        match Int32.TryParse(argv.[0]) with
        | true, port -> sendToPeer port
        | _ -> standaloneDemo ()
    else
        standaloneDemo ()
