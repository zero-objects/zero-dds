(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Producer-latency micro-bench: the async-decoupled writer's Mailbox.put
   (mutex + list-cons + condition-signal, no syscall) vs. an inline UDP
   sendto. Prints median ns/op for both -- the syscall the enqueue avoids on
   the producer's hot path.

   `Unix.gettimeofday` has only microsecond-ish resolution, well above a
   single mutex+cons (tens of ns), so each sample times a whole `batch` of
   operations and divides -- the standard technique for sub-clock-resolution
   micro-benchmarks. *)

open Reliable

let batch = 200
let samples = 500

(** Median ns/op over [samples] batches of [batch] calls to [f]. *)
let median_ns_per_op (f : unit -> unit) : float =
  let ns = Array.make samples 0.0 in
  for s = 0 to samples - 1 do
    let t0 = Unix.gettimeofday () in
    for _ = 1 to batch do
      f ()
    done;
    let elapsed = Unix.gettimeofday () -. t0 in
    ns.(s) <- elapsed *. 1_000_000_000.0 /. float_of_int batch
  done;
  Array.sort compare ns;
  ns.(samples / 2)

let () =
  let frame = reliable_write_frame 0 (Bytes.of_string "\001\002\003\004") in

  (* Inline path: a real sendto per sample (kernel transition). *)
  let sock = Unix.socket Unix.PF_INET Unix.SOCK_DGRAM 0 in
  Unix.bind sock (Unix.ADDR_INET (Unix.inet_addr_loopback, 0));
  let sink = Unix.socket Unix.PF_INET Unix.SOCK_DGRAM 0 in
  Unix.bind sink (Unix.ADDR_INET (Unix.inet_addr_loopback, 0));
  let sink_addr = Unix.getsockname sink in

  let inline_med =
    median_ns_per_op (fun () -> ignore (Unix.sendto sock frame 0 (Bytes.length frame) [] sink_addr))
  in

  (* Decoupled path: Mailbox.put only -- no syscall on the producer side.
     Drain after each batch to bound memory (the real drain thread does this
     concurrently, off the producer's hot path). *)
  let mailbox = Mailbox.create () in
  let ring_med = median_ns_per_op (fun () -> Mailbox.put mailbox frame) in
  ignore (Mailbox.drain_all mailbox);

  Printf.printf
    "producer latency: mailbox-enqueue median = %.1f ns/op, inline-sendto median = %.1f ns/op \
     (%.1fx)\n"
    ring_med inline_med
    (if ring_med <= 0.0 then 0.0 else inline_med /. ring_med);
  Unix.close sock;
  Unix.close sink
