// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-F# endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
// .NET/Mono, byte-identical to the Rust core and the other SDKs. Sync (poll)
// and async (an `async { }` MailboxProcessor agent — the idiomatic F# actor).

module ZeroDDS

open System

type Endian =
    | LE
    | BE

// --- Writer: a byte accumulator; alignment relative to buffer start, cap 4 ---

type Writer(endian: Endian) =
    let buf = ResizeArray<byte>()

    member _.Align(a: int) =
        let cap = min a 4
        let pad = (cap - (buf.Count % cap)) % cap
        for _ in 1 .. pad do
            buf.Add(0uy)

    // le: the value already encoded little-endian; reversed for BE.
    member this.Put(a: int, le: byte[]) =
        this.Align(a)
        if endian = BE then buf.AddRange(Array.rev le) else buf.AddRange(le)

    member _.PutU8(v: int) = buf.Add(byte (v &&& 0xff))
    member this.PutU16(v: int) = this.Put(2, [| byte v; byte (v >>> 8) |])

    member this.PutU32(v: uint32) =
        this.Put(4, [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |])

    member this.PutU64(v: uint64) =
        this.Put(4, [| for i in 0 .. 7 -> byte (v >>> (8 * i)) |])

    member this.PutF32(v: float32) =
        // GetBytes yields platform-endian bytes; on LE hosts that is our LE form.
        let b = BitConverter.GetBytes(v)
        let le = if BitConverter.IsLittleEndian then b else Array.rev b
        this.Put(4, le)

    member _.PutBytes(b: byte[]) = buf.AddRange(b)

    // Wire rule (docs/specs/zerodds-xcdr2-fsharp-1.0.md §"string"): uint32 (UTF-8
    // byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the BYTE
    // count of the UTF-8 encoding, not the .NET string length (UTF-16 code units)
    // — multibyte characters occupy more than one byte. Byte-identical to the
    // C/Rust core.
    member this.PutString(s: string) =
        let raw = Text.Encoding.UTF8.GetBytes(s)
        this.PutU32(uint32 (raw.Length + 1))
        buf.AddRange(raw)
        this.PutU8(0)

    member this.PutSeqU8(b: byte[]) =
        this.PutU32(uint32 b.Length)
        buf.AddRange(b)

    member _.Bytes() = buf.ToArray()

// --- Reader: a byte cursor ---

type Reader(data: byte[], endian: Endian) =
    let mutable pos = 0

    member _.Take(a: int, n: int) =
        let cap = min a 4
        let pad = (cap - (pos % cap)) % cap
        pos <- pos + pad
        let chunk = Array.sub data pos n
        pos <- pos + n
        if endian = BE then Array.rev chunk else chunk

    member this.GetU32() =
        let b = this.Take(4, 4)
        uint32 b.[0]
        ||| (uint32 b.[1] <<< 8)
        ||| (uint32 b.[2] <<< 16)
        ||| (uint32 b.[3] <<< 24)

    // --- primitives beyond GetU32 — the byte-exact inverse of the Writer ---
    member _.GetU8() =
        let b = data.[pos]
        pos <- pos + 1
        b

    member this.GetU16() =
        let b = this.Take(2, 2)
        uint16 b.[0] ||| (uint16 b.[1] <<< 8)

    member this.GetU64() =
        let b = this.Take(4, 8)
        let mutable v = 0UL
        for i in 0 .. 7 do
            v <- v ||| (uint64 b.[i] <<< (8 * i))
        v

    member this.GetF32() =
        let le = this.Take(4, 4)
        let hb = if BitConverter.IsLittleEndian then le else Array.rev le
        BitConverter.ToSingle(hb, 0)

    // Inverse of PutString: read n bytes (n incl. the NUL), decode the first
    // n-1 as UTF-8.
    member this.GetString() =
        let n = int (this.GetU32())
        let s = Text.Encoding.UTF8.GetString(data, pos, n - 1)
        pos <- pos + n
        s

    member this.GetSeqU8() =
        let n = int (this.GetU32())
        let b = Array.sub data pos n
        pos <- pos + n
        b

// --- XRCE WRITE_DATA framing ---

let SessionNoKey = 0x80uy
let StreamBestEffort = 0x01uy

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF; refuse (empty frame) rather than truncate the length while appending
// the full payload (mirrors the C SDK's rejection).
let writeFrame (session: byte) (stream: byte) (seq: int) (sample: byte[]) =
    if sample.Length > 0xFFFF then
        Array.empty
    else

    let n = sample.Length

    let hdr =
        [| session
           stream
           byte (seq &&& 0xff)
           byte ((seq >>> 8) &&& 0xff)
           0x07uy
           0x03uy
           byte (n &&& 0xff)
           byte ((n >>> 8) &&& 0xff) |]

    Array.append hdr sample

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: Array.sub frame 8 smLen, never (frame.Length - 8) to
// the end, so trailing padding or an appended submessage is not folded into the
// sample. Returns None for a short header, a wrong submessage id, or a declared
// length that runs past the datagram (truncation / wrong length).
let readFrame (frame: byte[]) =
    if frame.Length >= 8 && frame.[4] = 0x07uy then
        let smLen = int frame.[6] ||| (int frame.[7] <<< 8)
        if 8 + smLen > frame.Length then None
        else Some(Array.sub frame 8 smLen)
    else
        None

// --- transport ---

type Transport =
    { Deliver: byte[] -> unit
      Receive: unit -> byte[] option }

// --- sync client ---

type Client(transport: Transport) =
    let mutable seq = 1

    member _.Write(sample: byte[]) =
        let frame = writeFrame SessionNoKey StreamBestEffort seq sample
        transport.Deliver(frame)
        seq <- (seq + 1) &&& 0xffff

    // One non-blocking receive: returns the sample body, or None.
    member _.Poll() =
        match transport.Receive() with
        | None -> None
        | Some frame -> readFrame frame

// --- async reader: an async{} MailboxProcessor agent (idiomatic F#) ---

type private ReaderMsg = Next of AsyncReplyChannel<byte[]>

type AsyncReader(transport: Transport) =
    let agent =
        MailboxProcessor<ReaderMsg>.Start(fun inbox ->
            let rec loop () =
                async {
                    let! (Next reply) = inbox.Receive()
                    // cooperatively poll until a decoded sample is available
                    let rec pull () =
                        async {
                            match transport.Receive() with
                            | Some frame ->
                                match readFrame frame with
                                | Some body -> reply.Reply(body)
                                | None -> return! pull ()
                            | None ->
                                do! Async.Sleep(1)
                                return! pull ()
                        }

                    do! pull ()
                    return! loop ()
                }

            loop ())

    member _.RecvAsync() : Async<byte[]> = agent.PostAndAsyncReply(fun rc -> Next rc)
    member this.Recv() = this.RecvAsync() |> Async.RunSynchronously

// In-memory FIFO transport for tests + example.
let memTransport () =
    let q = System.Collections.Generic.Queue<byte[]>()
    let gate = obj ()

    { Deliver = fun frame -> lock gate (fun () -> q.Enqueue(Array.copy frame))
      Receive = fun () -> lock gate (fun () -> if q.Count > 0 then Some(q.Dequeue()) else None) }
