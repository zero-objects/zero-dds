// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable XRCE stream (stream_id >= 128, DDS-XRCE spec Sec 8.4.10/8.4.11) for
// the pure-F# endpoint SDK (ADR 0013). Self-contained: the state machine is
// mirrored byte-for-byte from crates/xrce/src/reliable.rs (also
// endpoints/rust/src/reliable.rs and endpoints/csharp/Reliable.cs), the
// WRITE_DATA/HEARTBEAT/ACKNACK wire codec is byte-identical to endpoints/c,
// endpoints/rust, endpoints/csharp, and crates/xrce, and AsyncReliableWriter is
// the async-decoupled writer: a bounded System.Threading.Channels ring (SPSC)
// plus a dedicated drain Thread that owns the ReliableSender and the UDP socket.
// The producer's Enqueue is a non-blocking channel write -- it never performs
// I/O; the drain thread does all sends, the HEARTBEAT timer, and ACKNACK-driven
// retransmit.
//
// Wire (little-endian; byte-identical to golden_heartbeat_le.bin /
// golden_acknack_le.bin from zerodds-endpoint-golden): an 8-byte header
// [session, stream, seq_lo, seq_hi, submsg, flags, len_lo, len_hi] then body.
// WRITE_DATA id=0x07 flags=0x03 on stream 0x80 (header seq = the sample's
// RFC-1982 seqnr). HEARTBEAT id=0x0B / ACKNACK id=0x0A, flags=0x01, on stream
// 0x00 (control); body is 5 bytes, the target reliable stream id travels in the
// body's last byte.

// Named `ZeroDDSReliable` (a single-identifier module) rather than
// `ZeroDDS.Reliable`: zerodds.fs already declares a top-level `module ZeroDDS`,
// and F# forbids the same name being both a module and a namespace in one
// assembly (FS0247). The C# sibling's `ZeroDDS.Endpoint` namespace has no direct
// F# equivalent here without restructuring zerodds.fs.
module ZeroDDSReliable

open System
open System.Collections.Generic
open System.Threading
open System.Threading.Channels

// --- HEARTBEAT / ACKNACK bodies ---

/// HEARTBEAT body.
type Heartbeat =
    { First: int16
      Last: int16
      StreamId: byte }

/// ACKNACK body. The 16-bit NACK bitmap travels as two little-endian bytes
/// (bit i set => seqnr FirstUnacked+i still missing).
type AckNack =
    { FirstUnacked: int16
      NackLo: byte
      NackHi: byte
      StreamId: byte }

    member this.Bitmap: uint16 =
        uint16 this.NackLo ||| (uint16 this.NackHi <<< 8)

// --- outcome unions (byte/semantics mirror of Reliable.cs enums) ---

type SubmitStatus =
    | Ok
    | PayloadTooLarge
    | WindowFull

type RecvStatus =
    | RecvOk
    | BufferFull

// --- Wire constants and the frame codec ---

module Wire =
    [<Literal>]
    let Session = 0x80uy

    [<Literal>]
    let StreamReliable = 0x80uy

    [<Literal>]
    let StreamNone = 0x00uy

    [<Literal>]
    let SmWriteData = 0x07uy

    [<Literal>]
    let SmAckNack = 0x0Auy

    [<Literal>]
    let SmHeartbeat = 0x0Buy

    [<Literal>]
    let WriteFlags = 0x03uy

    [<Literal>]
    let CtrlFlags = 0x01uy

    /// Sender window cap -- matches the 16-bit ACKNACK bitmap.
    [<Literal>]
    let Window = 16

    /// Receiver out-of-order buffer cap (DoS bound).
    [<Literal>]
    let ReceiverBuffer = 64

    /// Per-sample payload cap (u16 submessage length limit).
    [<Literal>]
    let MaxPayload = 65535

    /// Heartbeat period (spec recommends 100 ms; 500 ms conservative without a
    /// Tx pacing layer underneath).
    let HeartbeatPeriod = TimeSpan.FromMilliseconds(500.0)

    /// RFC-1982 16-bit "a is strictly before b" (wrapping serial comparison).
    let seqLt (a: uint16) (b: uint16) = int16 (a - b) < 0s

    /// RFC-1982 16-bit "a is strictly after b".
    let seqGt (a: uint16) (b: uint16) = int16 (a - b) > 0s

    /// Builds a reliable WRITE_DATA frame; the header seq carries the sample's
    /// RFC-1982 seqnr.
    let writeFrame (seq: uint16) (sample: byte[]) =
        let o = Array.zeroCreate<byte> (8 + sample.Length)
        o.[0] <- Session
        o.[1] <- StreamReliable
        o.[2] <- byte (seq &&& 0xFFus)
        o.[3] <- byte (seq >>> 8)
        o.[4] <- SmWriteData
        o.[5] <- WriteFlags
        let n = uint16 sample.Length
        o.[6] <- byte (n &&& 0xFFus)
        o.[7] <- byte (n >>> 8)
        Array.blit sample 0 o 8 sample.Length
        o

    /// Parses a reliable WRITE_DATA frame into Some(seq, sample); None on a short
    /// header or wrong submessage id.
    let tryUnframeWrite (frame: byte[]) =
        if frame.Length < 8 || frame.[4] <> SmWriteData then
            None
        else
            let seq = uint16 frame.[2] ||| (uint16 frame.[3] <<< 8)
            Some(seq, frame.[8..])

    let private ctrlHeader (submsg: byte) (msgSeq: uint16) =
        [| Session
           StreamNone
           byte (msgSeq &&& 0xFFus)
           byte (msgSeq >>> 8)
           submsg
           CtrlFlags
           5uy
           0uy |]

    /// Builds a HEARTBEAT control frame. Byte-identical to golden_heartbeat_le.bin
    /// when hb = { First = 1; Last = 3; StreamId = 0x80 } and msgSeq = 1.
    let heartbeatFrame (hb: Heartbeat) (msgSeq: uint16) =
        Array.append
            (ctrlHeader SmHeartbeat msgSeq)
            [| byte (hb.First &&& 0xFFs)
               byte ((hb.First >>> 8) &&& 0xFFs)
               byte (hb.Last &&& 0xFFs)
               byte ((hb.Last >>> 8) &&& 0xFFs)
               hb.StreamId |]

    /// Parses a HEARTBEAT frame (keys on the submessage id; header otherwise ignored).
    let tryParseHeartbeat (frame: byte[]) =
        if frame.Length < 13 || frame.[4] <> SmHeartbeat then
            None
        else
            Some
                { First = int16 (uint16 frame.[8] ||| (uint16 frame.[9] <<< 8))
                  Last = int16 (uint16 frame.[10] ||| (uint16 frame.[11] <<< 8))
                  StreamId = frame.[12] }

    /// Builds an ACKNACK control frame. Byte-identical to golden_acknack_le.bin
    /// when ack = { FirstUnacked = 1; NackLo = 0; NackHi = 0; StreamId = 0x80 }
    /// and msgSeq = 1.
    let ackNackFrame (ack: AckNack) (msgSeq: uint16) =
        Array.append
            (ctrlHeader SmAckNack msgSeq)
            [| byte (ack.FirstUnacked &&& 0xFFs)
               byte ((ack.FirstUnacked >>> 8) &&& 0xFFs)
               ack.NackLo
               ack.NackHi
               ack.StreamId |]

    /// Parses an ACKNACK frame (keys on the submessage id; header otherwise ignored).
    let tryParseAckNack (frame: byte[]) =
        if frame.Length < 13 || frame.[4] <> SmAckNack then
            None
        else
            Some
                { FirstUnacked = int16 (uint16 frame.[8] ||| (uint16 frame.[9] <<< 8))
                  NackLo = frame.[10]
                  NackHi = frame.[11]
                  StreamId = frame.[12] }

// --- Reliable sender ---

/// Reliable sender: assigns seqnrs, holds in-flight samples until acknowledged,
/// emits HEARTBEATs, and clears/keeps samples on ACKNACK. Mirrors the sender
/// half of crates/xrce/src/reliable.rs ReliableStreamState.
type ReliableSender() =
    let mutable nextSeq = 0us
    let inFlight = SortedDictionary<uint16, byte[]>()
    let mutable lastHeartbeat: DateTime option = None

    /// In-flight (unacknowledged) sample count.
    member _.InFlightCount = inFlight.Count

    /// Submits a sample, assigning it the next seqnr. Returns (status, seq); seq
    /// is 0 unless the status is Ok.
    member _.Submit(payload: byte[]) : SubmitStatus * uint16 =
        if payload.Length > Wire.MaxPayload then PayloadTooLarge, 0us
        elif inFlight.Count >= Wire.Window then WindowFull, 0us
        else
            let seq = nextSeq
            inFlight.[seq] <- payload
            nextSeq <- nextSeq + 1us
            Ok, seq

    /// The in-flight payload for `seq`, if still unacknowledged (for retransmit).
    member _.GetInFlight(seq: uint16) : byte[] option =
        match inFlight.TryGetValue(seq) with
        | true, p -> Some p
        | _ -> None

    /// All in-flight seqnrs, ascending (for retransmit against a NACK bitmap).
    member _.InFlightSeqs() : uint16 list = List.ofSeq inFlight.Keys

    /// Returns Some HEARTBEAT if the period elapsed and samples are in flight.
    member _.PendingHeartbeat(now: DateTime) : Heartbeat option =
        if inFlight.Count = 0 then
            None
        else
            let due =
                match lastHeartbeat with
                | None -> true
                | Some t -> (now - t) >= Wire.HeartbeatPeriod

            if not due then
                None
            else
                lastHeartbeat <- Some now
                // RFC-1982 window base (oldest unacked) + end (newest unacked);
                // NOT the numeric SortedDictionary first/last key, which is wrong
                // across a 16-bit wrap (window 0xFFFE,0xFFFF,0x0000,0x0001 -> base
                // 0xFFFE / end 0x0001, not 0x0000 / 0xFFFF). Mirrors window_base /
                // serial_max_in_flight in crates/xrce/src/reliable.rs.
                let mutable first = 0us
                let mutable last = 0us
                let mutable any = false

                for k in inFlight.Keys do
                    if not any then
                        first <- k
                        last <- k
                        any <- true
                    else
                        if Wire.seqLt k first then first <- k
                        if Wire.seqGt k last then last <- k

                Some
                    { First = int16 first
                      Last = int16 last
                      StreamId = Wire.StreamReliable }

    /// Processes an ACKNACK: drops everything strictly before `FirstUnacked` and
    /// every clear-bit slot in [FirstUnacked, FirstUnacked+16); keeps set-bit
    /// (still-missing) slots for retransmit.
    member _.RecvAckNack(ack: AckNack) =
        let baseSeq = uint16 ack.FirstUnacked
        let bitmap = ack.Bitmap
        let toDrop = [ for k in inFlight.Keys do if Wire.seqLt k baseSeq then yield k ]
        for k in toDrop do
            inFlight.Remove(k) |> ignore
        for i in 0..15 do
            let seq = baseSeq + uint16 i
            if ((bitmap >>> i) &&& 1us) = 0us then
                inFlight.Remove(seq) |> ignore

    /// Re-arms the heartbeat clock so the next tick fires immediately (used by the
    /// drain loop while it waits out a close-drain).
    member _.ResetHeartbeatClock() = lastHeartbeat <- None

// --- Reliable receiver ---

/// Reliable receiver: buffers out-of-order samples, delivers them contiguously,
/// and reports the missing slots via ACKNACK.
type ReliableReceiver() =
    let mutable expected = 0us
    let received = SortedDictionary<uint16, byte[]>()

    /// Next expected incoming seqnr.
    member _.Expected = expected

    /// Buffered out-of-order sample count.
    member _.OutOfOrderCount = received.Count

    /// Accepts a sample; drops duplicates before `Expected` or already buffered.
    member _.RecvData(seq: uint16, payload: byte[]) : RecvStatus =
        if Wire.seqLt seq expected then RecvOk // duplicate, drop
        elif received.ContainsKey(seq) then RecvOk // already buffered
        elif received.Count >= Wire.ReceiverBuffer then BufferFull
        else
            received.[seq] <- payload
            RecvOk

    /// Delivers all contiguous samples from `Expected`, advancing it.
    member _.DrainInOrder() : (uint16 * byte[]) list =
        let outp = ResizeArray<uint16 * byte[]>()
        let mutable go = true
        while go do
            match received.TryGetValue(expected) with
            | true, payload ->
                outp.Add((expected, payload))
                received.Remove(expected) |> ignore
                expected <- expected + 1us
            | _ -> go <- false
        List.ofSeq outp

    /// Computes the ACKNACK marking the missing slots in [Expected, Expected+16).
    /// A clear bit unambiguously means "received" when `hint` is None; with a
    /// `hint` (e.g. the HEARTBEAT's last-in-flight seqnr), slots strictly after it
    /// are left un-marked (not yet sent by the peer).
    member _.PendingAckNack(?hint: uint16) : AckNack =
        let mutable bitmap = 0us

        for i in 0..15 do
            let seq = expected + uint16 i
            let skip =
                match hint with
                | Some h -> Wire.seqGt seq h
                | None -> false
            if not skip && not (received.ContainsKey(seq)) then
                bitmap <- bitmap ||| (1us <<< i)

        { FirstUnacked = int16 expected
          NackLo = byte (bitmap &&& 0xFFus)
          NackHi = byte (bitmap >>> 8)
          StreamId = Wire.StreamReliable }

    /// Clears all receiver state.
    member _.Reset() =
        expected <- 0us
        received.Clear()

// --- async-decoupled reliable writer ---

/// Shared closed-flag between a `ReliableWriterHandle` and its
/// `AsyncReliableWriter`. Tracked independently of the channel's own completion
/// (which only completes once the channel has also been fully *drained* --
/// exactly the condition the drain thread needs to detect separately, since a
/// stalled window can leave items unread indefinitely).
type internal WriterCloseState() =
    let mutable closed = 0
    member _.Closed = Volatile.Read(&closed) <> 0
    member _.SetClosed() = Volatile.Write(&closed, 1)

/// Producer-side handle to the async-decoupled writer. `Enqueue` is a
/// non-blocking channel write -- it never performs I/O; the drain thread does all
/// sends. `Close` signals the drain thread that no more samples are coming; once
/// the send window has drained it exits.
type ReliableWriterHandle internal (writer: ChannelWriter<byte[]>, state: WriterCloseState) =
    /// Non-blocking enqueue. Returns false (backpressure) if the ring is full; the
    /// caller should retry (e.g. after a Thread.Yield()), the same contract as the
    /// Rust/D/C/C# SPSC-ring writers.
    member _.Enqueue(sample: byte[]) : bool = writer.TryWrite(sample)

    /// Signals the drain thread that no more samples will be enqueued.
    member _.Close() =
        state.SetClosed()
        writer.TryComplete() |> ignore

/// The async-decoupled reliable writer: a bounded, single-writer/single-reader
/// System.Threading.Channels ring plus a dedicated drain Thread that owns the
/// ReliableSender and the UDP socket. Mirrors
/// endpoints/rust/src/reliable.rs::AsyncReliableWriter and
/// endpoints/csharp/Reliable.cs::AsyncReliableWriter.
type AsyncReliableWriter private (sock: System.Net.Sockets.Socket, capacity: int) =
    let closeState = WriterCloseState()

    let channel =
        Channel.CreateBounded<byte[]>(
            BoundedChannelOptions(
                capacity,
                SingleWriter = true,
                SingleReader = true,
                FullMode = BoundedChannelFullMode.Wait
            )
        )

    let handle = ReliableWriterHandle(channel.Writer, closeState)
    let mutable shutdownCalled = false

    let trySend (frame: byte[]) =
        try
            sock.Send(frame) |> ignore
        with :? System.Net.Sockets.SocketException ->
            () // best-effort

    let tryReceive (buf: byte[]) : int =
        try
            sock.Receive(buf)
        with :? System.Net.Sockets.SocketException ->
            0

    let drainLoop () =
        sock.ReceiveTimeout <- 5
        let sender = ReliableSender()
        let mutable ctlSeq = 1us
        let buf = Array.zeroCreate<byte> 2048
        let mutable closeDeadline: DateTime option = None
        let mutable running = true

        while running do
            // 1) Drain the channel into the sender window; send fresh WRITE_DATA.
            let mutable more = true
            while more && sender.InFlightCount < Wire.Window do
                match channel.Reader.TryRead() with
                | true, sample ->
                    match sender.Submit(sample) with
                    | Ok, seq -> trySend (Wire.writeFrame seq sample)
                    | _ -> ()
                | _ -> more <- false

            // 2) HEARTBEAT when due.
            match sender.PendingHeartbeat(DateTime.UtcNow) with
            | Some hb ->
                trySend (Wire.heartbeatFrame hb ctlSeq)
                ctlSeq <- ctlSeq + 1us
            | None -> ()

            // 3) Drain incoming ACKNACKs -> retransmit the still-missing samples.
            let mutable rx = true
            while rx do
                let n = tryReceive buf
                if n <= 0 then
                    rx <- false
                else
                    match Wire.tryParseAckNack buf.[0 .. n - 1] with
                    | Some ack ->
                        sender.RecvAckNack(ack)
                        let baseSeq = uint16 ack.FirstUnacked
                        let bitmap = ack.Bitmap
                        for i in 0..15 do
                            if ((bitmap >>> i) &&& 1us) = 1us then
                                let seq = baseSeq + uint16 i
                                match sender.GetInFlight(seq) with
                                | Some p -> trySend (Wire.writeFrame seq p)
                                | None -> ()
                    | None -> ()

            // 4) Exit once closed AND the window has drained (ring empty, every
            //    in-flight sample acknowledged) -- or, as a safety valve, once the
            //    post-close drain deadline elapses (a vanished peer -- or a
            //    permanently-full window with nobody ACKing -- must never wedge
            //    Shutdown() forever).
            if closeState.Closed then
                if closeDeadline.IsNone then
                    closeDeadline <- Some(DateTime.UtcNow + TimeSpan.FromSeconds(5.0))
                let ringEmpty = not (fst (channel.Reader.TryPeek()))
                if sender.InFlightCount = 0 && ringEmpty then
                    running <- false
                elif DateTime.UtcNow > closeDeadline.Value then
                    running <- false // best-effort give-up
                else
                    // Keep heartbeating so the peer keeps ACKing until the window empties.
                    sender.ResetHeartbeatClock()

    let drain =
        let t = Thread(ThreadStart(drainLoop))
        t.IsBackground <- true
        t.Name <- "zerodds-reliable-drain"
        t.Start()
        t

    /// A handle the producer(s) hold; Enqueue never touches the kernel.
    member _.Handle = handle

    /// Spawns the drain thread. `sock` must already target the peer (connected UDP
    /// socket); the drain thread owns it exclusively from here on.
    static member Start(sock: System.Net.Sockets.Socket, ?capacity: int) =
        new AsyncReliableWriter(sock, defaultArg capacity 1024)

    /// Closes the writer and blocks until the drain thread finishes (the window
    /// has drained, or the best-effort deadline elapsed). Idempotent.
    member _.Shutdown() =
        if shutdownCalled then
            drain.Join()
        else
            shutdownCalled <- true
            handle.Close()
            drain.Join()

    interface IDisposable with
        member this.Dispose() = this.Shutdown()
