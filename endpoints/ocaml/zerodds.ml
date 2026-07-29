(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Native pure-OCaml endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
   byte-identical to the Rust core and the other SDKs. Sync (poll) and async
   (a Thread filling a Mutex/Condition mailbox — stdlib, no Lwt/Async). *)

module Wire = struct
  type endian = LE | BE

  (* --- Writer: a Buffer + endian; alignment relative to buffer start, cap 4 --- *)

  type writer = { buf : Buffer.t; endian : endian }

  let writer endian = { buf = Buffer.create 64; endian }

  let align w a =
    let cap = min a 4 in
    let pad = (cap - (Buffer.length w.buf mod cap)) mod cap in
    for _ = 1 to pad do Buffer.add_char w.buf '\000' done

  (* le: the value already encoded little-endian; reversed for BE. *)
  let put w a (le : bytes) =
    align w a;
    let n = Bytes.length le in
    if w.endian = BE then
      for i = n - 1 downto 0 do Buffer.add_char w.buf (Bytes.get le i) done
    else Buffer.add_bytes w.buf le

  let le_of_int v n =
    let b = Bytes.create n in
    for i = 0 to n - 1 do
      Bytes.set b i (Char.chr ((v lsr (8 * i)) land 0xff))
    done;
    b

  let put_u8 w v = Buffer.add_char w.buf (Char.chr (v land 0xff))
  let put_u16 w v = put w 2 (le_of_int v 2)
  let put_u32 w v = put w 4 (le_of_int v 4)

  let put_u64 w (v : int64) =
    let b = Bytes.create 8 in
    for i = 0 to 7 do
      let byte = Int64.to_int (Int64.logand (Int64.shift_right_logical v (8 * i)) 0xffL) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_f32 w (v : float) =
    let bits = Int32.bits_of_float v in
    let b = Bytes.create 4 in
    for i = 0 to 3 do
      let byte = Int32.to_int (Int32.logand (Int32.shift_right_logical bits (8 * i)) 0xffl) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_bytes w (b : bytes) = Buffer.add_bytes w.buf b

  let put_string w s =
    put_u32 w (String.length s + 1);
    Buffer.add_string w.buf s;
    put_u8 w 0

  let put_seq_u8 w (b : bytes) =
    put_u32 w (Bytes.length b);
    Buffer.add_bytes w.buf b

  let bytes w = Buffer.to_bytes w.buf

  (* --- Reader: a bytes cursor --- *)

  type reader = { data : bytes; mutable pos : int; rendian : endian }

  let reader data endian = { data; pos = 0; rendian = endian }

  let take r a n =
    let cap = min a 4 in
    let pad = (cap - (r.pos mod cap)) mod cap in
    r.pos <- r.pos + pad;
    let b = Bytes.sub r.data r.pos n in
    r.pos <- r.pos + n;
    if r.rendian = BE then begin
      let out = Bytes.create n in
      for i = 0 to n - 1 do Bytes.set out i (Bytes.get b (n - 1 - i)) done;
      out
    end
    else b

  let get_u32 r =
    let b = take r 4 4 in
    Char.code (Bytes.get b 0)
    lor (Char.code (Bytes.get b 1) lsl 8)
    lor (Char.code (Bytes.get b 2) lsl 16)
    lor (Char.code (Bytes.get b 3) lsl 24)

  (* --- primitives beyond get_u32 — the byte-exact inverse of the Writer --- *)

  let get_u8 r =
    let v = Char.code (Bytes.get r.data r.pos) in
    r.pos <- r.pos + 1;
    v

  let get_u16 r =
    let b = take r 2 2 in
    Char.code (Bytes.get b 0) lor (Char.code (Bytes.get b 1) lsl 8)

  let get_u64 r =
    let b = take r 4 8 in
    let v = ref 0L in
    for i = 0 to 7 do
      v :=
        Int64.logor !v
          (Int64.shift_left (Int64.of_int (Char.code (Bytes.get b i))) (8 * i))
    done;
    !v

  let get_f32 r = Int32.float_of_bits (Int32.of_int (get_u32 r))

  let get_string r =
    let n = get_u32 r in
    let s = Bytes.sub_string r.data r.pos (n - 1) in
    r.pos <- r.pos + n;
    s

  let get_seq_u8 r =
    let n = get_u32 r in
    let b = Bytes.sub r.data r.pos n in
    r.pos <- r.pos + n;
    b
end

module Endpoint = struct
  let session_nokey = 0x80
  let stream_best_effort = 0x01

  let write_frame session stream seq (sample : bytes) =
    let n = Bytes.length sample in
    let hdr = Bytes.create 8 in
    Bytes.set hdr 0 (Char.chr session);
    Bytes.set hdr 1 (Char.chr stream);
    Bytes.set hdr 2 (Char.chr (seq land 0xff));
    Bytes.set hdr 3 (Char.chr ((seq lsr 8) land 0xff));
    Bytes.set hdr 4 (Char.chr 0x07);
    Bytes.set hdr 5 (Char.chr 0x03);
    Bytes.set hdr 6 (Char.chr (n land 0xff));
    Bytes.set hdr 7 (Char.chr ((n lsr 8) land 0xff));
    Bytes.cat hdr sample

  let read_frame (frame : bytes) =
    if Bytes.length frame >= 8 && Char.code (Bytes.get frame 4) = 0x07 then
      Some (Bytes.sub frame 8 (Bytes.length frame - 8))
    else None
end

(* transport: deliver a frame, receive a frame (or None). *)
type transport = { deliver : bytes -> unit; receive : unit -> bytes option }

module Client = struct
  type t = { transport : transport; session : int; stream : int; mutable seq : int }

  let create transport =
    { transport; session = Endpoint.session_nokey; stream = Endpoint.stream_best_effort; seq = 1 }

  let write c (sample : bytes) =
    let frame = Endpoint.write_frame c.session c.stream c.seq sample in
    c.transport.deliver frame;
    c.seq <- (c.seq + 1) land 0xffff

  (* One non-blocking receive: returns the sample body, or None. *)
  let poll c =
    match c.transport.receive () with
    | None -> None
    | Some frame -> Endpoint.read_frame frame
end

(* A blocking FIFO mailbox (Mutex + Condition), the idiomatic stdlib primitive. *)
module Mailbox = struct
  type 'a t = { mutable q : 'a list; m : Mutex.t; c : Condition.t }

  let create () = { q = []; m = Mutex.create (); c = Condition.create () }

  let put mb x =
    Mutex.lock mb.m;
    mb.q <- mb.q @ [ x ];
    Condition.signal mb.c;
    Mutex.unlock mb.m

  let take mb =
    Mutex.lock mb.m;
    while mb.q = [] do Condition.wait mb.c mb.m done;
    let x = List.hd mb.q in
    mb.q <- List.tl mb.q;
    Mutex.unlock mb.m;
    x
end

module AsyncReader = struct
  type t = { mutable running : bool; mb : bytes Mailbox.t }

  let start transport =
    let r = { running = true; mb = Mailbox.create () } in
    let rec loop () =
      if r.running then begin
        (match transport.receive () with
         | None -> Thread.delay 0.001
         | Some frame -> (
             match Endpoint.read_frame frame with
             | Some body -> Mailbox.put r.mb body
             | None -> ()));
        loop ()
      end
    in
    ignore (Thread.create loop ());
    r

  let recv r = Mailbox.take r.mb
  let stop r = r.running <- false
end

module MemTransport = struct
  let create () =
    let q = ref [] in
    let m = Mutex.create () in
    {
      deliver = (fun frame ->
        Mutex.lock m;
        q := !q @ [ frame ];
        Mutex.unlock m);
      receive = (fun () ->
        Mutex.lock m;
        let r = match !q with [] -> None | x :: xs -> q := xs; Some x in
        Mutex.unlock m;
        r);
    }
end
