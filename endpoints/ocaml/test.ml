(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Tests for the native OCaml endpoint: byte-identity vs the Rust goldens, plus
   sync + async loopback. Built alongside zerodds.ml (threads.posix). *)

let golden_dir = try Sys.getenv "GOLDEN_DIR" with Not_found -> "build"

let fixture endian =
  let open Zerodds.Wire in
  let w = writer endian in
  put_u32 w 0xA1B2C3D4;
  put_u16 w 0x1234;
  put_u8 w 0x5A;
  put_f32 w 3.5;
  put_u64 w 0x0102030405060708L;
  put_string w "bay-12";
  put_seq_u8 w (Bytes.of_string "\xDE\xAD\xBE\xEF");
  bytes w

let read_file path =
  let ic = open_in_bin path in
  let n = in_channel_length ic in
  let b = Bytes.create n in
  really_input ic b 0 n;
  close_in ic;
  b

let sample_body id =
  let open Zerodds.Wire in
  let w = writer LE in
  put_u32 w id;
  put_u16 w 0;
  put_u8 w 0;
  bytes w

let () =
  (* byte identity *)
  List.iter
    (fun (endian, file) ->
      let got = fixture endian in
      let golden = read_file (Filename.concat golden_dir file) in
      if got <> golden then
        failwith
          (Printf.sprintf "%s: not byte-identical (got %d want %d)" file
             (Bytes.length got) (Bytes.length golden));
      Printf.printf "%s: %d bytes byte-identical\n" file (Bytes.length golden))
    [ (Zerodds.Wire.LE, "golden_le.bin"); (Zerodds.Wire.BE, "golden_be.bin") ];

  (* sync loopback *)
  let t = Zerodds.MemTransport.create () in
  let c = Zerodds.Client.create t in
  for i = 0 to 4 do
    Zerodds.Client.write c (sample_body (0x3000 + i))
  done;
  for i = 0 to 4 do
    match Zerodds.Client.poll c with
    | Some body ->
        let id = Zerodds.Wire.get_u32 (Zerodds.Wire.reader body Zerodds.Wire.LE) in
        if id <> 0x3000 + i then failwith "sync: id mismatch"
    | None -> failwith "sync: no sample"
  done;
  print_string "sync loopback: 5 samples OK\n";

  (* async loopback (Thread + mailbox) *)
  let t2 = Zerodds.MemTransport.create () in
  let w = Zerodds.Client.create t2 in
  for i = 0 to 4 do
    Zerodds.Client.write w (sample_body (0x1000 + i))
  done;
  let r = Zerodds.AsyncReader.start t2 in
  for i = 0 to 4 do
    let body = Zerodds.AsyncReader.recv r in
    let id = Zerodds.Wire.get_u32 (Zerodds.Wire.reader body Zerodds.Wire.LE) in
    if id <> 0x1000 + i then failwith "async: id mismatch"
  done;
  Zerodds.AsyncReader.stop r;
  print_string "async loopback: 5 samples OK\n";
  print_string "ALL OK\n"
