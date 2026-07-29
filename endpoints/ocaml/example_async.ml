(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Deep example (async): the same sensor-telemetry flow, but the subscriber does
   not own the run-loop. `AsyncReader.start` spawns a Thread that fills a
   Mutex/Condition mailbox; the consumer blocks on `recv` — the idiomatic OCaml
   stdlib concurrency model (no Lwt/Async). Every field is decoded. *)

module Reading = struct
  type t = { id : int; value : float; label : string }

  let marshal (v : t) (endian : Zerodds.Wire.endian) : bytes =
    let open Zerodds.Wire in
    let w = writer endian in
    put_u32 w v.id;
    put_f32 w v.value;
    put_string w v.label;
    bytes w

  let decode (body : bytes) : t =
    let open Zerodds.Wire in
    let r = reader body LE in
    let id = get_u32 r in
    let value = get_f32 r in
    let label = get_string r in
    { id; value; label }
end

let () =
  let total = 5 in
  let t = Zerodds.MemTransport.create () in
  let c = Zerodds.Client.create t in
  for i = 0 to total - 1 do
    let r =
      Reading.
        {
          id = 0x2000 + i;
          value = 100.0 -. float_of_int i;
          label = Printf.sprintf "sensor-%02d" i;
        }
    in
    Zerodds.Client.write c (Reading.marshal r Zerodds.Wire.LE)
  done;
  let reader = Zerodds.AsyncReader.start t in
  for got = 0 to total - 1 do
    let body = Zerodds.AsyncReader.recv reader in
    let { Reading.id; value; label } = Reading.decode body in
    Printf.printf "async reading %d: id=0x%x value=%.1f label=\"%s\"\n" got id
      value label
  done;
  Zerodds.AsyncReader.stop reader;
  print_string "ALL OK\n"
