(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Unit suite mirroring crates/xrce reliable.rs + byte-golden assertion.
   Usage: reliable_test [golden_heartbeat_le.bin golden_acknack_le.bin]
   Prints "ALL OK" and exits 0 on success.
*)

open Reliable

let failures = ref 0

let check (cond : bool) (msg : string) : unit =
  if not cond then begin
    Printf.printf "FAIL: %s\n" msg;
    incr failures
  end

let bytes_of_ints (xs : int list) : bytes =
  let bs = Bytes.create (List.length xs) in
  List.iteri (fun i x -> Bytes.set bs i (Char.chr (x land 0xff))) xs;
  bs

let read_file (path : string) : bytes =
  let ic = open_in_bin path in
  let n = in_channel_length ic in
  let bs = Bytes.create n in
  really_input ic bs 0 n;
  close_in ic;
  bs

let () =
  (* --- Sender --- *)
  (let s = Sender.create () in
   (match Sender.submit s (bytes_of_ints [ 1; 2 ]) with
   | Ok 0 -> ()
   | _ -> check false "monotonic seq 0");
   (match Sender.submit s (bytes_of_ints [ 3; 4 ]) with
   | Ok 1 -> ()
   | _ -> check false "monotonic seq 1");
   check (Sender.in_flight_count s = 2) "in-flight count");

  (let s = Sender.create () in
   let huge = Bytes.create (max_payload + 1) in
   match Sender.submit s huge with
   | Error `PayloadTooLarge -> ()
   | _ -> check false "payload too large");

  (let s = Sender.create () in
   for _ = 1 to window do
     match Sender.submit s (bytes_of_ints [ 0 ]) with
     | Ok _ -> ()
     | Error _ -> check false "fill window"
   done;
   match Sender.submit s (bytes_of_ints [ 0 ]) with
   | Error `WindowFull -> ()
   | _ -> check false "window full");

  (let s = Sender.create () in
   ignore (Sender.submit s (bytes_of_ints [ 1 ]));
   let base = 0.0 in
   (match Sender.pending_heartbeat s base with
   | Some hb -> check (hb.hb_first = 0 && hb.hb_last = 0 && hb.hb_stream = 0x80) "heartbeat body"
   | None -> check false "heartbeat fires first");
   (match Sender.pending_heartbeat s (base +. 0.1) with
   | None -> ()
   | Some _ -> check false "heartbeat silenced <500ms");
   match Sender.pending_heartbeat s (base +. 0.6) with
   | Some _ -> ()
   | None -> check false "heartbeat after 500ms");

  (let s = Sender.create () in
   match Sender.pending_heartbeat s 0.0 with
   | None -> ()
   | Some _ -> check false "no heartbeat when empty");

  (let s = Sender.create () in
   ignore (Sender.submit s (bytes_of_ints [ 0xA0 ])); (* seq 0 *)
   ignore (Sender.submit s (bytes_of_ints [ 0xA1 ])); (* seq 1 *)
   ignore (Sender.submit s (bytes_of_ints [ 0xA2 ])); (* seq 2 *)
   (* base=2, bitmap=0b1 => seq2 missing, 0+1 acked *)
   Sender.recv_acknack s { ak_first_unacked = 2; ak_nack = (0x01, 0x00); ak_stream = 0x80 };
   check (Sender.in_flight_count s = 1) "acknack clears acked";
   check (Sender.get_in_flight s 2 <> None) "seq2 retransmittable");

  (let s = Sender.create () in
   for _ = 1 to 5 do
     ignore (Sender.submit s (bytes_of_ints [ 0 ]))
   done;
   Sender.recv_acknack s { ak_first_unacked = 5; ak_nack = (0, 0); ak_stream = 0x80 };
   check (Sender.in_flight_count s = 0) "acknack full clear");

  (* --- Receiver --- *)
  (let r = Receiver.create () in
   ignore (Receiver.recv_data r 0 (bytes_of_ints [ 10 ]));
   ignore (Receiver.recv_data r 1 (bytes_of_ints [ 11 ]));
   let d = Receiver.drain_in_order r in
   (match d with
   | [ (0, p0); (1, p1) ] ->
       check (Bytes.get p0 0 = '\010') "in-order drain[0]";
       check (Bytes.get p1 0 = '\011') "in-order drain[1]"
   | _ -> check false "in-order drain shape");
   check (Receiver.expected r = 2) "expected advanced");

  (let r = Receiver.create () in
   ignore (Receiver.recv_data r 2 (bytes_of_ints [ 22 ]));
   ignore (Receiver.recv_data r 0 (bytes_of_ints [ 20 ]));
   let d1 = Receiver.drain_in_order r in
   check (List.length d1 = 1 && fst (List.nth d1 0) = 0) "reorder: only seq0";
   ignore (Receiver.recv_data r 1 (bytes_of_ints [ 21 ]));
   let d2 = Receiver.drain_in_order r in
   check
     (match d2 with [ (1, _); (2, _) ] -> true | _ -> false)
     "reorder: 1+2");

  (let r = Receiver.create () in
   ignore (Receiver.recv_data r 0 (bytes_of_ints [ 1 ]));
   ignore (Receiver.drain_in_order r);
   ignore (Receiver.recv_data r 0 (bytes_of_ints [ 99 ]));
   (* duplicate *)
   check (Receiver.out_of_order_count r = 0) "duplicate dropped");

  (let r = Receiver.create () in
   for i = 1 to recv_buf do
     match Receiver.recv_data r i (bytes_of_ints [ 1 ]) with
     | Ok () -> ()
     | Error _ -> check false "fill recv buffer"
   done;
   match Receiver.recv_data r (recv_buf + 1) (bytes_of_ints [ 1 ]) with
   | Error `BufferFull -> ()
   | _ -> check false "recv buffer full");

  (let r = Receiver.create () in
   ignore (Receiver.recv_data r 1 (bytes_of_ints [ 1 ]));
   ignore (Receiver.recv_data r 3 (bytes_of_ints [ 3 ]));
   let a = Receiver.pending_acknack r (Some 3) in
   let n0, n1 = a.ak_nack in
   let bm = n0 lor (n1 lsl 8) in
   check (bm land 1 <> 0) "slot 0 missing";
   check (bm land (1 lsl 2) <> 0) "slot 2 missing";
   check (bm land (1 lsl 1) = 0) "slot 1 present";
   check (bm land (1 lsl 3) = 0) "slot 3 present");

  (let r = Receiver.create () in
   ignore (Receiver.recv_data r 0 (bytes_of_ints [ 3 ]));
   Receiver.reset r;
   check (Receiver.expected r = 0 && Receiver.out_of_order_count r = 0) "reset clears receiver");

  (* --- end-to-end loss recovery (in-proc) --- *)
  (let s = Sender.create () in
   let r = Receiver.create () in
   let seqs = Array.make 3 0 in
   for i = 0 to 2 do
     match Sender.submit s (bytes_of_ints [ i ]) with
     | Ok seq -> seqs.(i) <- seq
     | Error _ -> check false "submit e2e"
   done;
   ignore (Receiver.recv_data r seqs.(0) (bytes_of_ints [ 0 ]));
   (* seqs.(1) lost *)
   ignore (Receiver.recv_data r seqs.(2) (bytes_of_ints [ 2 ]));
   let d = Receiver.drain_in_order r in
   check (List.length d = 1) "only seq0 before recovery";
   let ack = Receiver.pending_acknack r (Some seqs.(2)) in
   Sender.recv_acknack s ack;
   check (Sender.get_in_flight s seqs.(1) <> None) "seq1 retransmittable";
   ignore (Receiver.recv_data r seqs.(1) (bytes_of_ints [ 1 ]));
   let d2 = Receiver.drain_in_order r in
   check (List.length d2 = 2) "seq1+2 after recovery");

  (* --- byte-golden (hardcoded, matches golden-gen's HeartbeatPayload{first=1,
     last=3, stream=0x80} / AckNackPayload{first_unacked=1, bitmap=[0,0],
     stream=0x80} under StreamId::NONE, msg-seq=1) --- *)
  let hb = heartbeat_frame { hb_first = 1; hb_last = 3; hb_stream = 0x80 } in
  let hb_expect =
    bytes_of_ints
      [ 0x80; 0x00; 0x01; 0x00; 0x0b; 0x01; 0x05; 0x00; 0x01; 0x00; 0x03; 0x00; 0x80 ]
  in
  check (hb = hb_expect) "heartbeat byte-golden (hardcoded)";
  let ak = acknack_frame { ak_first_unacked = 1; ak_nack = (0, 0); ak_stream = 0x80 } in
  let ak_expect =
    bytes_of_ints
      [ 0x80; 0x00; 0x01; 0x00; 0x0a; 0x01; 0x05; 0x00; 0x01; 0x00; 0x00; 0x00; 0x80 ]
  in
  check (ak = ak_expect) "acknack byte-golden (hardcoded)";

  if Array.length Sys.argv >= 3 then begin
    let g_hb = read_file Sys.argv.(1) in
    let g_ak = read_file Sys.argv.(2) in
    check (hb = g_hb) "heartbeat byte-identical to golden file";
    check (ak = g_ak) "acknack byte-identical to golden file";
    print_string "golden files matched\n"
  end;

  if !failures = 0 then print_string "ALL OK\n"
  else begin
    Printf.printf "%d check(s) FAILED\n" !failures;
    exit 1
  end
