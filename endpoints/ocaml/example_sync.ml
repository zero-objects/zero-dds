(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
   five typed `Reading { id; value; label }` samples and delivers them; the
   subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte. *)

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
          id = 0x1000 + i;
          value = 20.0 +. (float_of_int i *. 0.5);
          label = Printf.sprintf "bay-%02d" i;
        }
    in
    Zerodds.Client.write c (Reading.marshal r Zerodds.Wire.LE)
  done;
  let got = ref 0 in
  let continue = ref true in
  while !got < total && !continue do
    match Zerodds.Client.poll c with
    | Some body ->
        let { Reading.id; value; label } = Reading.decode body in
        Printf.printf "sync reading %d: id=0x%x value=%.1f label=\"%s\"\n" !got id
          value label;
        incr got
    | None -> continue := false
  done;
  if !got <> total then (
    Printf.eprintf "incomplete: got %d of %d\n" !got total;
    exit 1);
  print_string "ALL OK\n"
