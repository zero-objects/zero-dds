(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Reliable-sender E2E app: argv = <peer-port> <N>. Enqueues N samples on the
   async-decoupled Writer (Mutex/Condition Mailbox + drain Thread); the drain
   thread submits each to the Sender's history, ships WRITE_DATA over UDP to
   the Rust peer, ticks HEARTBEAT, and retransmits whatever an ACKNACK still
   marks missing -- until the send window drains. The peer
   (zerodds-endpoint-e2e) injects loss and replies ACKNACK; loss recovery is
   proven when every sample lands gap-free in order on the peer.
*)

let sample_of (i : int) : bytes =
  let b = Bytes.create 4 in
  Bytes.set b 0 (Char.chr (i land 0xff));
  Bytes.set b 1 (Char.chr ((i lsr 8) land 0xff));
  Bytes.set b 2 (Char.chr ((i lsr 16) land 0xff));
  Bytes.set b 3 (Char.chr ((i lsr 24) land 0xff));
  b

let () =
  let port = int_of_string Sys.argv.(1) in
  let n = int_of_string Sys.argv.(2) in

  let sock = Unix.socket Unix.PF_INET Unix.SOCK_DGRAM 0 in
  Unix.bind sock (Unix.ADDR_INET (Unix.inet_addr_loopback, 0));
  let peer_addr = Unix.ADDR_INET (Unix.inet_addr_loopback, port) in

  let writer = Reliable.Writer.create sock peer_addr in
  for i = 0 to n - 1 do
    Reliable.Writer.enqueue writer (sample_of i)
  done;

  let drained = Reliable.Writer.wait_drained writer 20.0 in
  Reliable.Writer.stop writer;

  if drained then Printf.eprintf "RELIABLE OK: all %d acknowledged\n%!" n
  else begin
    Printf.eprintf "RELIABLE INCOMPLETE: %d still in-flight\n%!"
      (Reliable.Writer.in_flight_count writer);
    exit 1
  end
