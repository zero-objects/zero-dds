(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   In-process reliable demo: a sender submits N samples, a lossy channel
   drops every 3rd, and ACKNACK-driven retransmit recovers the full
   contiguous sequence. Prints "delivered: 0 1 2 ... " / "RELIABLE OK". *)

open Reliable

let () =
  let n = 12 in
  let s = Sender.create () in
  let r = Receiver.create () in

  (* Submit + first delivery attempt, dropping every 3rd sample. *)
  for i = 0 to n - 1 do
    match Sender.submit s (Bytes.make 1 (Char.chr i)) with
    | Ok seq -> if i mod 3 <> 2 then ignore (Receiver.recv_data r seq (Bytes.make 1 (Char.chr i)))
    | Error _ -> failwith "submit"
  done;

  (* Recover: the peer's ACKNACK reveals the gaps, the sender retransmits. *)
  let round = ref 0 in
  while Receiver.expected r <> n && !round < n do
    ignore (Receiver.drain_in_order r);
    if Receiver.expected r <> n then begin
      let ack = Receiver.pending_acknack r (Some (n - 1)) in
      Sender.recv_acknack s ack;
      let base = ack.ak_first_unacked in
      let n0, n1 = ack.ak_nack in
      let bitmap = n0 lor (n1 lsl 8) in
      for b = 0 to 15 do
        if (bitmap lsr b) land 1 = 1 then begin
          let seq = u16_wrap (base + b) in
          match Sender.get_in_flight s seq with
          | Some payload -> ignore (Receiver.recv_data r seq payload)
          | None -> ()
        end
      done
    end;
    incr round
  done;
  ignore (Receiver.drain_in_order r);

  print_string "delivered:";
  for i = 0 to Receiver.expected r - 1 do
    Printf.printf " %d" i
  done;
  print_newline ();
  if Receiver.expected r = n then print_string "RELIABLE OK\n"
  else begin
    Printf.printf "RELIABLE INCOMPLETE at %d\n" (Receiver.expected r);
    exit 1
  end
