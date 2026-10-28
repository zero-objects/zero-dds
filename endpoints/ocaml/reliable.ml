(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Reliable XRCE stream (stream_id >= 128, spec Sec 8.4.10/8.4.11) for the
   native OCaml endpoint SDK. Self-contained: state machine + HEARTBEAT /
   ACKNACK / WRITE_DATA wire codec + a Mutex/Condition mailbox with a drain
   Thread (the async-decoupled writer). Byte-identical to endpoints/d and
   crates/xrce.

   This file is its own module (`Reliable`, file-as-module) and never defines
   a nested `Wire` submodule, so it stays clear of the `module Wire` clash
   the ping-pong test (crates/endpoint-e2e/tests/ocaml.rs) hit between the
   idlc-generated module and `zerodds.ml`'s SDK module -- `reliable.ml` does
   not depend on `zerodds.ml` at all.
*)

(* ------------------------------------------------------------------- *)
(* Constants (spec + this SDK's fixed caps)                             *)
(* ------------------------------------------------------------------- *)

(** Sender window cap: 16 in-flight samples (matches the 16-bit ACKNACK
    bitmap). *)
let window = 16

(** Receiver out-of-order buffer cap (DoS bound). *)
let recv_buf = 64

(** Per-sample payload cap: 64 KiB (u16 submessage length limit). *)
let max_payload = 65_535

(** HEARTBEAT period in seconds (spec recommends 100ms; 500ms here, matching
    crates/xrce's [DEFAULT_HEARTBEAT_PERIOD]). *)
let heartbeat_period = 0.5

let session = 0x80
let stream_reliable = 0x80
let stream_none = 0x00
let sm_write = 0x07
let sm_acknack = 0x0A
let sm_heartbeat = 0x0B
(* E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
   DDS-XRCE 1.0 8.3.5.8. *)
let flags_write = 0x01

(* Wire size of the BaseObjectRequest that precedes the sample (7.7.8). *)
let base_size = 4
let flags_ctrl = 0x01

(* ------------------------------------------------------------------- *)
(* RFC-1982 16-bit sequence-number ordering                             *)
(* ------------------------------------------------------------------- *)

(** [seq_lt a b] -- RFC-1982 "a is strictly before b" on a 16-bit wrapping
    sequence space. *)
let seq_lt (a : int) (b : int) : bool =
  let d = (a - b) land 0xffff in
  d land 0x8000 <> 0

(** RFC-1982 "a is strictly after b". *)
let seq_gt (a : int) (b : int) : bool = seq_lt b a

let u16_wrap (v : int) : int = v land 0xffff

(* ------------------------------------------------------------------- *)
(* Control-message payloads                                             *)
(* ------------------------------------------------------------------- *)

(** Encoding of the DDS-XRCE 1.0 Annex A types without [@extensibility]
    (APPENDABLE by DDS-XTypes 1.3 7.3.1.2.1.8). [Appendable] (default): a
    DHEADER in front of the members. [Final]: the form of the reference
    implementation (eProsima), no DHEADER. This SDK writes one such type:
    ACKNACK_Payload. HEARTBEAT_Payload is FINAL in both modes. *)
type unannotated_types = Appendable | Final

type heartbeat = { hb_first : int; hb_last : int; hb_stream : int }

(** [ak_nack] is the little-endian 16-bit bitmap as its two bytes
    [(lo, hi)]. *)
type acknack = { ak_first_unacked : int; ak_nack : int * int; ak_stream : int }

(* ------------------------------------------------------------------- *)
(* Wire codec (little-endian; byte-identical to golden_heartbeat_le.bin /
   golden_acknack_le.bin, see endpoints/golden-gen)                     *)
(* ------------------------------------------------------------------- *)

let u16_le (v : int) : int * int = (v land 0xff, (v lsr 8) land 0xff)

(** Reliable WRITE_DATA frame: header seq carries the RFC-1982 sample seq, and
    the submessage body is the BaseObjectRequest (7.7.8: request_id then
    object_id, each two octets most significant first) followed by the sample.
    [obj_id] names the target DataWriter; the request_id is the sample's own
    sequence number. *)
let reliable_write_frame (seq : int) (obj_id : int) (sample : bytes) : bytes =
  let seq = u16_wrap seq in
  let n = base_size + Bytes.length sample in
  let s_lo, s_hi = u16_le seq in
  let n_lo, n_hi = u16_le n in
  let hdr = Bytes.create 12 in
  Bytes.set hdr 0 (Char.chr session);
  Bytes.set hdr 1 (Char.chr stream_reliable);
  Bytes.set hdr 2 (Char.chr s_lo);
  Bytes.set hdr 3 (Char.chr s_hi);
  Bytes.set hdr 4 (Char.chr sm_write);
  Bytes.set hdr 5 (Char.chr flags_write);
  Bytes.set hdr 6 (Char.chr n_lo);
  Bytes.set hdr 7 (Char.chr n_hi);
  Bytes.set hdr 8 (Char.chr ((seq lsr 8) land 0xff));
  Bytes.set hdr 9 (Char.chr (seq land 0xff));
  Bytes.set hdr 10 (Char.chr ((obj_id lsr 8) land 0xff));
  Bytes.set hdr 11 (Char.chr (obj_id land 0xff));
  Bytes.cat hdr sample

(** [(seq, sample)] from a reliable WRITE_DATA frame. The sample starts after
    the BaseObjectRequest; a body too short to hold one is refused rather than
    handed back truncated. *)
let parse_write_frame (frame : bytes) : (int * bytes) option =
  if Bytes.length frame < 8 + base_size || Char.code (Bytes.get frame 4) <> sm_write then None
  else
    let sm_len = Char.code (Bytes.get frame 6) lor (Char.code (Bytes.get frame 7) lsl 8) in
    if sm_len < base_size || 8 + sm_len > Bytes.length frame then None
    else
      let seq = Char.code (Bytes.get frame 2) lor (Char.code (Bytes.get frame 3) lsl 8) in
      let sample = Bytes.sub frame (8 + base_size) (sm_len - base_size) in
      Some (seq, sample)

(** Golden control-frame header: session, stream=NONE, msg-seq=1, submsg,
    flags, body-len (LE): 5 for HEARTBEAT, 9 for ACKNACK (Appendable) or 5
    (Final). *)
let ctrl_header (submsg : int) (body_len : int) : bytes =
  let hdr = Bytes.create 8 in
  Bytes.set hdr 0 (Char.chr session);
  Bytes.set hdr 1 (Char.chr stream_none);
  Bytes.set hdr 2 '\001';
  Bytes.set hdr 3 '\000';
  Bytes.set hdr 4 (Char.chr submsg);
  Bytes.set hdr 5 (Char.chr flags_ctrl);
  Bytes.set hdr 6 (Char.chr body_len);
  Bytes.set hdr 7 '\000';
  hdr

let heartbeat_frame (hb : heartbeat) : bytes =
  let f_lo, f_hi = u16_le (u16_wrap hb.hb_first) in
  let l_lo, l_hi = u16_le (u16_wrap hb.hb_last) in
  let body = Bytes.create 5 in
  Bytes.set body 0 (Char.chr f_lo);
  Bytes.set body 1 (Char.chr f_hi);
  Bytes.set body 2 (Char.chr l_lo);
  Bytes.set body 3 (Char.chr l_hi);
  Bytes.set body 4 (Char.chr hb.hb_stream);
  Bytes.cat (ctrl_header sm_heartbeat 5) body

(** ACKNACK frame. ACKNACK_Payload has no @extensibility, so it is APPENDABLE
    (DDS-XTypes 1.3 7.3.1.2.1.8) and carries a DHEADER in XCDR2.
    [~unannotated_types:Appendable] (default): 17 octets, body = DHEADER u32 LE
    (= 5) + the 5-octet payload, body-len = 9.
    [~unannotated_types:Final]: 13 octets, body = the 5-octet payload,
    body-len = 5 (form of the reference implementation). *)
let acknack_frame ?(unannotated_types = Appendable) (ak : acknack) : bytes =
  let f_lo, f_hi = u16_le (u16_wrap ak.ak_first_unacked) in
  let n0, n1 = ak.ak_nack in
  let payload = Bytes.create 5 in
  Bytes.set payload 0 (Char.chr f_lo);
  Bytes.set payload 1 (Char.chr f_hi);
  Bytes.set payload 2 (Char.chr (n0 land 0xff));
  Bytes.set payload 3 (Char.chr (n1 land 0xff));
  Bytes.set payload 4 (Char.chr ak.ak_stream);
  match unannotated_types with
  | Appendable ->
      let dheader = Bytes.make 4 '\000' in
      Bytes.set dheader 0 '\005';
      Bytes.cat (ctrl_header sm_acknack 9) (Bytes.cat dheader payload)
  | Final -> Bytes.cat (ctrl_header sm_acknack 5) payload

(** Accepts body length 9 (4-octet DHEADER, then the 5-octet payload) and the
    FINAL form with body length 5 (no DHEADER), whatever the writer's
    [unannotated_types] setting. *)
let parse_acknack (frame : bytes) : acknack option =
  if Bytes.length frame < 13 || Char.code (Bytes.get frame 4) <> sm_acknack then None
  else
    let blen = Char.code (Bytes.get frame 6) lor (Char.code (Bytes.get frame 7) lsl 8) in
    let o =
      if blen = 9 && Bytes.length frame >= 17 then Some 12
      else if blen = 5 then Some 8
      else None
    in
    match o with
    | None -> None
    | Some o ->
        let first =
          Char.code (Bytes.get frame o) lor (Char.code (Bytes.get frame (o + 1)) lsl 8)
        in
        let n0 = Char.code (Bytes.get frame (o + 2)) in
        let n1 = Char.code (Bytes.get frame (o + 3)) in
        let stream = Char.code (Bytes.get frame (o + 4)) in
        Some { ak_first_unacked = first; ak_nack = (n0, n1); ak_stream = stream }

let parse_heartbeat (frame : bytes) : heartbeat option =
  if Bytes.length frame < 13 || Char.code (Bytes.get frame 4) <> sm_heartbeat then None
  else
    let first = Char.code (Bytes.get frame 8) lor (Char.code (Bytes.get frame 9) lsl 8) in
    let last = Char.code (Bytes.get frame 10) lor (Char.code (Bytes.get frame 11) lsl 8) in
    let stream = Char.code (Bytes.get frame 12) in
    Some { hb_first = first; hb_last = last; hb_stream = stream }

(* ------------------------------------------------------------------- *)
(* Sender state machine (mirror of crates/xrce reliable.rs)             *)
(* ------------------------------------------------------------------- *)

module Sender = struct
  type t = {
    mutable next_seq : int;
    in_flight : (int, bytes) Hashtbl.t;
    mutable have_heartbeat : bool;
    mutable last_heartbeat : float;
    stream : int;
  }

  let create () : t =
    {
      next_seq = 0;
      in_flight = Hashtbl.create 32;
      have_heartbeat = false;
      last_heartbeat = 0.0;
      stream = stream_reliable;
    }

  let in_flight_count (s : t) : int = Hashtbl.length s.in_flight

  let get_in_flight (s : t) (seq : int) : bytes option = Hashtbl.find_opt s.in_flight seq

  (** Sorted in-flight seqs (for deterministic first/last + retransmit). *)
  let in_flight_seqs (s : t) : int list =
    Hashtbl.fold (fun k _ acc -> k :: acc) s.in_flight [] |> List.sort compare

  (** Submits a new sample, returning its assigned seq. *)
  let submit (s : t) (payload : bytes) : (int, [ `PayloadTooLarge | `WindowFull ]) result =
    if Bytes.length payload > max_payload then Error `PayloadTooLarge
    else if Hashtbl.length s.in_flight >= window then Error `WindowFull
    else begin
      let seq = s.next_seq in
      Hashtbl.replace s.in_flight seq payload;
      s.next_seq <- u16_wrap (s.next_seq + 1);
      Ok seq
    end

  (** [Some hb] if the heartbeat period elapsed and samples are in flight. *)
  let pending_heartbeat (s : t) (now : float) : heartbeat option =
    if Hashtbl.length s.in_flight = 0 then None
    else
      let due = (not s.have_heartbeat) || now -. s.last_heartbeat >= heartbeat_period in
      if not due then None
      else begin
        s.have_heartbeat <- true;
        s.last_heartbeat <- now;
        (* RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
           numeric-sorted head/last of [in_flight_seqs], which is wrong across a
           16-bit wrap: window 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end
           0x0001, not 0x0000 / 0xFFFF. Mirrors window_base /
           serial_max_in_flight in crates/xrce/src/reliable.rs. *)
        let first, last =
          Hashtbl.fold
            (fun k _ acc ->
              match acc with
              | None -> Some (k, k)
              | Some (mn, mx) ->
                let mn = if seq_lt k mn then k else mn in
                let mx = if seq_gt k mx then k else mx in
                Some (mn, mx))
            s.in_flight None
          |> Option.get
        in
        Some { hb_first = first; hb_last = last; hb_stream = s.stream }
      end

  (** Clears acknowledged seqs: everything strictly before [base] (RFC-1982),
      plus every clear bit within [base, base+16). *)
  let recv_acknack (s : t) (ak : acknack) : unit =
    let base = u16_wrap ak.ak_first_unacked in
    let n0, n1 = ak.ak_nack in
    let bitmap = n0 lor (n1 lsl 8) in
    let before =
      Hashtbl.fold (fun k _ acc -> if seq_lt k base then k :: acc else acc) s.in_flight []
    in
    List.iter (Hashtbl.remove s.in_flight) before;
    for i = 0 to 15 do
      let seq = u16_wrap (base + i) in
      if (bitmap lsr i) land 1 = 0 then Hashtbl.remove s.in_flight seq
    done

  let reset (s : t) : unit =
    s.next_seq <- 0;
    Hashtbl.clear s.in_flight;
    s.have_heartbeat <- false;
    s.last_heartbeat <- 0.0
end

(* ------------------------------------------------------------------- *)
(* Receiver state machine                                               *)
(* ------------------------------------------------------------------- *)

module Receiver = struct
  type t = {
    mutable expected_seq : int;
    received : (int, bytes) Hashtbl.t;
    stream : int;
    unannotated_types : unannotated_types;
  }

  (** [?unannotated_types] (default [Appendable]) selects the encoding of the
      ACKNACK_Payload this receiver writes, see {!acknack_frame}. *)
  let create ?(unannotated_types = Appendable) () : t =
    { expected_seq = 0; received = Hashtbl.create 32; stream = stream_reliable; unannotated_types }

  let expected (r : t) : int = r.expected_seq
  let out_of_order_count (r : t) : int = Hashtbl.length r.received

  let recv_data (r : t) (seq : int) (payload : bytes) : (unit, [ `BufferFull ]) result =
    if seq_lt seq r.expected_seq then Ok () (* already delivered: duplicate, drop *)
    else if Hashtbl.mem r.received seq then Ok () (* already buffered *)
    else if Hashtbl.length r.received >= recv_buf then Error `BufferFull
    else begin
      Hashtbl.replace r.received seq payload;
      Ok ()
    end

  (** Contiguous samples starting at [expected]; advances [expected]. *)
  let drain_in_order (r : t) : (int * bytes) list =
    let out = ref [] in
    let continue_ = ref true in
    while !continue_ do
      match Hashtbl.find_opt r.received r.expected_seq with
      | Some payload ->
          out := (r.expected_seq, payload) :: !out;
          Hashtbl.remove r.received r.expected_seq;
          r.expected_seq <- u16_wrap (r.expected_seq + 1)
      | None -> continue_ := false
    done;
    List.rev !out

  (** Bitmap of missing slots in [expected, expected+16); with [hint] given,
      slots strictly after it are not marked (mirrors the sender-side
      HEARTBEAT hint so a clear bit means "confirmed present", not just
      "not yet looked at"). *)
  let pending_acknack (r : t) (hint : int option) : acknack =
    let bitmap = ref 0 in
    for i = 0 to 15 do
      let seq = u16_wrap (r.expected_seq + i) in
      let skip = match hint with Some h -> seq_gt seq h | None -> false in
      if (not skip) && not (Hashtbl.mem r.received seq) then bitmap := !bitmap lor (1 lsl i)
    done;
    {
      ak_first_unacked = r.expected_seq;
      ak_nack = (!bitmap land 0xff, (!bitmap lsr 8) land 0xff);
      ak_stream = r.stream;
    }

  (** ACKNACK frame for the current window, encoded per the receiver's
      [unannotated_types]. *)
  let acknack_frame (r : t) (hint : int option) : bytes =
    acknack_frame ~unannotated_types:r.unannotated_types (pending_acknack r hint)

  let reset (r : t) : unit =
    r.expected_seq <- 0;
    Hashtbl.clear r.received
end

(* ------------------------------------------------------------------- *)
(* Mutex/Condition mailbox -- the producer side of the async-decoupled  *)
(* writer. [put] never blocks on I/O: mutex + list-cons + signal only.  *)
(* ------------------------------------------------------------------- *)

module Mailbox = struct
  type 'a t = { mutable q : 'a list; m : Mutex.t; c : Condition.t }

  let create () : 'a t = { q = []; m = Mutex.create (); c = Condition.create () }

  (** Producer: enqueue and return immediately. [Condition.signal] wakes a
      thread blocked in [take] (unused by the drain loop, which polls
      non-blockingly via [drain_all] so it can also service the HEARTBEAT
      timer and inbound ACKNACKs -- but exposed for a consumer that wants to
      block instead). *)
  let put (mb : 'a t) (x : 'a) : unit =
    Mutex.lock mb.m;
    mb.q <- x :: mb.q;
    Condition.signal mb.c;
    Mutex.unlock mb.m

  (** Drain: pops everything queued so far, in FIFO (submission) order,
      without blocking. *)
  let drain_all (mb : 'a t) : 'a list =
    Mutex.lock mb.m;
    let items = List.rev mb.q in
    mb.q <- [];
    Mutex.unlock mb.m;
    items

  let is_empty (mb : 'a t) : bool =
    Mutex.lock mb.m;
    let e = mb.q = [] in
    Mutex.unlock mb.m;
    e
end

(* ------------------------------------------------------------------- *)
(* The async-decoupled writer: producer enqueues into the Mailbox; a    *)
(* drain Thread owns the Sender state + the UDP socket and does         *)
(* submit -> history -> WRITE_DATA, ticks HEARTBEAT, and retransmits on *)
(* ACKNACK. `recv_acknack` (called from the drain thread) is also the   *)
(* prune step: acknowledged entries leave `in_flight`.                  *)
(* ------------------------------------------------------------------- *)

module Writer = struct
  type t = {
    sock : Unix.file_descr;
    peer_addr : Unix.sockaddr;
    (* ObjectId of the target DataWriter (7.7.8 BaseObjectRequest). *)
    object_id : int;
    sender : Sender.t;
    mailbox : bytes Mailbox.t;
    mutable running : bool;
    mutable thread : Thread.t option;
  }

  let send_frame (w : t) (frame : bytes) : unit =
    ignore (Unix.sendto w.sock frame 0 (Bytes.length frame) [] w.peer_addr)

  let retransmit_missing (w : t) (ak : acknack) : unit =
    let base = u16_wrap ak.ak_first_unacked in
    let n0, n1 = ak.ak_nack in
    let bitmap = n0 lor (n1 lsl 8) in
    for i = 0 to 15 do
      if (bitmap lsr i) land 1 = 1 then begin
        let seq = u16_wrap (base + i) in
        match Sender.get_in_flight w.sender seq with
        | Some payload -> send_frame w (reliable_write_frame seq w.object_id payload)
        | None -> ()
      end
    done

  (* One tick: drain queued submissions, tick the heartbeat, poll for an
     ACKNACK (short timeout, so the loop keeps ticking the heartbeat and can
     notice `running` go false promptly). *)
  let tick (w : t) (buf : bytes) : unit =
    let items = Mailbox.drain_all w.mailbox in
    List.iter
      (fun payload ->
        match Sender.submit w.sender payload with
        | Ok seq -> send_frame w (reliable_write_frame seq w.object_id payload)
        | Error `WindowFull ->
            (* Window full: requeue for a later tick, once ACKNACKs free room. *)
            Mailbox.put w.mailbox payload
        | Error `PayloadTooLarge ->
            Printf.eprintf "Writer: dropping oversized payload (%d bytes)\n%!"
              (Bytes.length payload))
      items;

    (match Sender.pending_heartbeat w.sender (Unix.gettimeofday ()) with
    | Some hb -> send_frame w (heartbeat_frame hb)
    | None -> ());

    match Unix.select [ w.sock ] [] [] 0.02 with
    | [], _, _ -> ()
    | _ -> (
        try
          let n, _ = Unix.recvfrom w.sock buf 0 (Bytes.length buf) [] in
          let frame = Bytes.sub buf 0 n in
          match parse_acknack frame with
          | Some ak ->
              Sender.recv_acknack w.sender ak;
              retransmit_missing w ak
          | None -> ()
        with Unix.Unix_error _ -> ())

  let drain_loop (w : t) : unit =
    let buf = Bytes.create 4096 in
    while w.running do
      tick w buf
    done

  (** Spawns the drain thread and returns the writer handle. *)
  let create (sock : Unix.file_descr) (peer_addr : Unix.sockaddr) (object_id : int) : t =
    let w =
      {
        sock;
        peer_addr;
        object_id;
        sender = Sender.create ();
        mailbox = Mailbox.create ();
        running = true;
        thread = None;
      }
    in
    w.thread <- Some (Thread.create drain_loop w);
    w

  (** Producer: enqueue a sample; returns immediately -- the mutex + list-cons
      is the entire cost, no syscall on this path. *)
  let enqueue (w : t) (payload : bytes) : unit = Mailbox.put w.mailbox payload

  let in_flight_count (w : t) : int = Sender.in_flight_count w.sender

  (** Polls (does not touch [running]) until every enqueued sample has been
      submitted and acknowledged, or [timeout_s] elapses. Returns whether it
      drained in time. *)
  let wait_drained (w : t) (timeout_s : float) : bool =
    let deadline = Unix.gettimeofday () +. timeout_s in
    let rec loop () =
      if Sender.in_flight_count w.sender = 0 && Mailbox.is_empty w.mailbox then true
      else if Unix.gettimeofday () > deadline then false
      else begin
        Thread.delay 0.005;
        loop ()
      end
    in
    loop ()

  (** Stops the drain thread and joins it. Always terminates promptly (the
      loop only checks [running], bounded by the 20ms select timeout) --
      call after [wait_drained], successful or not. *)
  let stop (w : t) : unit =
    w.running <- false;
    match w.thread with
    | Some t ->
        Thread.join t;
        w.thread <- None
    | None -> ()
end
