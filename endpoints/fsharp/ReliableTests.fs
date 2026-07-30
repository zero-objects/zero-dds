// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit suite for reliable.fs, mirroring crates/xrce/src/reliable.rs's own test
// module (and endpoints/csharp/ReliableTests.cs) plus a byte-golden assertion
// for the HEARTBEAT/ACKNACK wire codec.
//
// usage: ReliableTests [golden_heartbeat_le.bin golden_acknack_le.bin]
// Prints "ALL OK" and exits 0 on success (nonzero + "N FAILURE(S)" otherwise).

module ReliableTests

open System
open System.IO
open ZeroDDSReliable

let mutable private failures = 0

let private check cond msg =
    if not cond then
        printfn "FAIL: %s" msg
        failures <- failures + 1

let private eq (a: byte[]) (b: byte[]) = a = b

[<EntryPoint>]
let main argv =
    // ---- Sender ----
    (let s = ReliableSender()
     let st0, a = s.Submit([| 1uy; 2uy |])
     check (st0 = Ok && a = 0us) "monotonic seq 0"
     let st1, b = s.Submit([| 3uy; 4uy |])
     check (st1 = Ok && b = 1us) "monotonic seq 1"
     check (s.InFlightCount = 2) "in-flight count")

    (let s = ReliableSender()
     let huge = Array.zeroCreate<byte> (Wire.MaxPayload + 1)
     let st, _ = s.Submit(huge)
     check (st = PayloadTooLarge) "payload too large")

    (let s = ReliableSender()
     for _ in 1 .. Wire.Window do
         let st, _ = s.Submit([| 0uy |])
         check (st = Ok) "fill window"
     let st, _ = s.Submit([| 0uy |])
     check (st = WindowFull) "window full")

    (let s = ReliableSender()
     s.Submit([| 1uy |]) |> ignore
     let t0 = DateTime.UtcNow
     match s.PendingHeartbeat(t0) with
     | Some hb ->
         check true "heartbeat fires first"
         check (hb.First = 0s && hb.Last = 0s && hb.StreamId = 0x80uy) "heartbeat body"
     | None -> check false "heartbeat fires first"
     check (s.PendingHeartbeat(t0.AddMilliseconds(100.0)).IsNone) "heartbeat silenced <500ms"
     check (s.PendingHeartbeat(t0.AddMilliseconds(600.0)).IsSome) "heartbeat after 500ms")

    (let s = ReliableSender()
     check (s.PendingHeartbeat(DateTime.UtcNow).IsNone) "no heartbeat when empty")

    (let s = ReliableSender()
     s.Submit([| 0xA0uy |]) |> ignore // seq 0
     s.Submit([| 0xA1uy |]) |> ignore // seq 1
     s.Submit([| 0xA2uy |]) |> ignore // seq 2
     // base=2, bitmap=0b1 -> seq2 missing, 0+1 acked.
     s.RecvAckNack({ FirstUnacked = 2s; NackLo = 0x01uy; NackHi = 0x00uy; StreamId = 0x80uy })
     check (s.InFlightCount = 1) "acknack clears acked"
     check (s.GetInFlight(2us).IsSome) "seq2 retransmittable")

    (let s = ReliableSender()
     for _ in 0..4 do
         s.Submit([| 0uy |]) |> ignore
     s.RecvAckNack({ FirstUnacked = 5s; NackLo = 0uy; NackHi = 0uy; StreamId = 0x80uy }) // full clear
     check (s.InFlightCount = 0) "acknack full clear")

    // ---- Receiver ----
    (let r = ReliableReceiver()
     r.RecvData(0us, [| 10uy |]) |> ignore
     r.RecvData(1us, [| 11uy |]) |> ignore
     let d = r.DrainInOrder()
     check (d.Length = 2 && (snd d.[0]).[0] = 10uy && (snd d.[1]).[0] = 11uy) "in-order drain"
     check (r.Expected = 2us) "expected advanced")

    (let r = ReliableReceiver()
     r.RecvData(2us, [| 22uy |]) |> ignore
     r.RecvData(0us, [| 20uy |]) |> ignore
     let d1 = r.DrainInOrder()
     check (d1.Length = 1 && (snd d1.[0]).[0] = 20uy) "reorder: only seq0"
     r.RecvData(1us, [| 21uy |]) |> ignore
     let d2 = r.DrainInOrder()
     check (d2.Length = 2 && (snd d2.[0]).[0] = 21uy && (snd d2.[1]).[0] = 22uy) "reorder: seq1+2")

    (let r = ReliableReceiver()
     r.RecvData(0us, [| 1uy |]) |> ignore
     r.DrainInOrder() |> ignore
     r.RecvData(0us, [| 99uy |]) |> ignore // duplicate
     check (r.OutOfOrderCount = 0) "duplicate dropped")

    (let r = ReliableReceiver()
     for i in 1 .. Wire.ReceiverBuffer do
         check (r.RecvData(uint16 i, [| 1uy |]) = RecvOk) "fill recv buffer"
     check (r.RecvData(uint16 (Wire.ReceiverBuffer + 1), [| 1uy |]) = BufferFull) "recv buffer full")

    (let r = ReliableReceiver()
     r.RecvData(1us, [| 1uy |]) |> ignore
     r.RecvData(3us, [| 3uy |]) |> ignore
     let a = r.PendingAckNack(3us)
     let bm = a.Bitmap
     check ((bm &&& 1us) <> 0us) "slot 0 missing"
     check ((bm &&& (1us <<< 2)) <> 0us) "slot 2 missing"
     check ((bm &&& (1us <<< 1)) = 0us) "slot 1 present"
     check ((bm &&& (1us <<< 3)) = 0us) "slot 3 present")

    (let r = ReliableReceiver()
     r.RecvData(0us, [| 1uy |]) |> ignore
     r.Reset()
     check (r.Expected = 0us && r.OutOfOrderCount = 0) "reset clears receiver")

    // ---- end-to-end loss recovery (in-process) ----
    (let s = ReliableSender()
     let r = ReliableReceiver()
     let seqs = Array.zeroCreate<uint16> 3
     for i in 0..2 do
         let _, q = s.Submit([| byte i |])
         seqs.[i] <- q
     r.RecvData(seqs.[0], [| 0uy |]) |> ignore // seq1 lost
     r.RecvData(seqs.[2], [| 2uy |]) |> ignore
     let d = r.DrainInOrder()
     check (d.Length = 1) "only seq0 before recovery"
     let ack = r.PendingAckNack(seqs.[2])
     s.RecvAckNack(ack)
     check (s.GetInFlight(seqs.[1]).IsSome) "seq1 retransmittable"
     r.RecvData(seqs.[1], [| 1uy |]) |> ignore
     let d2 = r.DrainInOrder()
     check (d2.Length = 2) "seq1+2 after recovery")

    // ---- RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit
    //      wrap (mirrors crates/xrce's wrap regression tests). Seeds
    //      sender/receiver up to the wrap via the public API only, then straddles
    //      0x0000.
    (let s = ReliableSender()
     let mutable seq = 0us // walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
     let mutable go = true
     while go do
         let _, q = s.Submit([| 0uy |])
         seq <- q
         s.RecvAckNack({ FirstUnacked = int16 (seq + 1us); NackLo = 0uy; NackHi = 0uy; StreamId = 0x80uy })
         if seq = 0xFFFDus then go <- false
     check (s.InFlightCount = 0) "wrap seed: sender window drained"

     let _, q0 = s.Submit([| 10uy |]) // 0xFFFE
     let _, q1 = s.Submit([| 11uy |]) // 0xFFFF (lost)
     let _, q2 = s.Submit([| 12uy |]) // 0x0000
     let _, q3 = s.Submit([| 13uy |]) // 0x0001
     check (q0 = 0xFFFEus && q1 = 0xFFFFus && q2 = 0x0000us && q3 = 0x0001us) "wrap seqs"

     match s.PendingHeartbeat(DateTime.UtcNow) with
     | Some hbw ->
         check (uint16 hbw.First = 0xFFFEus && uint16 hbw.Last = 0x0001us)
             "heartbeat window across wrap = [0xFFFE,0x0001] (not numeric 0x0000,0xFFFF)"
     | None -> check false "heartbeat window across wrap"

     let r = ReliableReceiver() // seed expected to 0xFFFE.
     for k in 0..0xFFFD do
         r.RecvData(uint16 k, [| 0uy |]) |> ignore
         r.DrainInOrder() |> ignore
     check (r.Expected = 0xFFFEus) "wrap seed: receiver expects 0xFFFE"

     r.RecvData(q0, [| 10uy |]) |> ignore // 0xFFFF lost
     r.RecvData(q2, [| 12uy |]) |> ignore
     r.RecvData(q3, [| 13uy |]) |> ignore
     let dw = r.DrainInOrder()
     check (dw.Length = 1 && (snd dw.[0]).[0] = 10uy) "only 0xFFFE before recovery"
     check (r.Expected = 0xFFFFus) "receiver blocked at 0xFFFF"

     let ackw = r.PendingAckNack(q3)
     check (uint16 ackw.FirstUnacked = 0xFFFFus) "acknack base = 0xFFFF across wrap"
     check ((ackw.Bitmap &&& 0b1us) <> 0us) "0xFFFF NACKed"
     check ((ackw.Bitmap &&& 0b110us) = 0us) "0x0000/0x0001 present"

     s.RecvAckNack(ackw)
     check (s.GetInFlight(q1).IsSome) "0xFFFF retransmittable"
     check (s.GetInFlight(q0).IsNone && s.GetInFlight(q2).IsNone && s.GetInFlight(q3).IsNone)
         "0xFFFE/0x0000/0x0001 acked"
     check (s.InFlightCount = 1) "only 0xFFFF left in-flight"

     r.RecvData(q1, [| 11uy |]) |> ignore
     let dw2 = r.DrainInOrder()
     check (dw2.Length = 3 && (snd dw2.[0]).[0] = 11uy && (snd dw2.[1]).[0] = 12uy && (snd dw2.[2]).[0] = 13uy)
         "0xFFFF,0x0000,0x0001 deliver in RFC-1982 order")

    // ---- byte-golden ----
    let hbFrame = Wire.heartbeatFrame { First = 1s; Last = 3s; StreamId = 0x80uy } 1us
    let hbExpect =
        [| 0x80uy; 0x00uy; 0x01uy; 0x00uy; 0x0buy; 0x01uy; 0x05uy; 0x00uy; 0x01uy; 0x00uy; 0x03uy; 0x00uy; 0x80uy |]
    check (eq hbFrame hbExpect) "heartbeat byte-golden (hardcoded)"

    let akFrame = Wire.ackNackFrame { FirstUnacked = 1s; NackLo = 0uy; NackHi = 0uy; StreamId = 0x80uy } 1us
    let akExpect =
        [| 0x80uy; 0x00uy; 0x01uy; 0x00uy; 0x0auy; 0x01uy; 0x05uy; 0x00uy; 0x01uy; 0x00uy; 0x00uy; 0x00uy; 0x80uy |]
    check (eq akFrame akExpect) "acknack byte-golden (hardcoded)"

    if argv.Length >= 2 then
        let gHb = File.ReadAllBytes(argv.[0])
        let gAk = File.ReadAllBytes(argv.[1])
        check (eq hbFrame gHb) "heartbeat byte-identical to golden file"
        check (eq akFrame gAk) "acknack byte-identical to golden file"
        printfn "golden files matched"

    if failures > 0 then
        printfn "%d FAILURE(S)" failures
        1
    else
        printfn "ALL OK"
        0
