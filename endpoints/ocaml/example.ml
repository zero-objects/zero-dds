(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Runnable example for the native OCaml endpoint: sync (poll) and async
   (Thread + mailbox). Built alongside zerodds.ml. *)

let sample id label =
  let open Zerodds.Wire in
  let w = writer LE in
  put_u32 w id;
  put_string w label;
  bytes w

let () =
  (* sync *)
  let t = Zerodds.MemTransport.create () in
  let c = Zerodds.Client.create t in
  Zerodds.Client.write c (sample 0x42 "sync-hello");
  (match Zerodds.Client.poll c with
   | Some body ->
       let id = Zerodds.Wire.get_u32 (Zerodds.Wire.reader body Zerodds.Wire.LE) in
       Printf.printf "sync: received id=0x%x\n" id
   | None -> print_string "sync: nothing\n");

  (* async *)
  let t2 = Zerodds.MemTransport.create () in
  let w = Zerodds.Client.create t2 in
  for i = 0 to 2 do Zerodds.Client.write w (sample (0x100 + i) "async") done;
  let r = Zerodds.AsyncReader.start t2 in
  for _ = 0 to 2 do
    let body = Zerodds.AsyncReader.recv r in
    let id = Zerodds.Wire.get_u32 (Zerodds.Wire.reader body Zerodds.Wire.LE) in
    Printf.printf "async: received id=0x%x\n" id
  done;
  Zerodds.AsyncReader.stop r;
  print_string "ALL OK\n"
