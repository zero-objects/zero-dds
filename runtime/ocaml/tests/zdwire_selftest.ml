(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   A self-test of the OCaml wire runtime against fixed octets, one per
   HDDS axis of `Dialect::Hdds`. Expected octets from the recorded
   columns:

   tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
     (elementary packed elements),
   tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
     (a sequence element's own DHEADER length),
   tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
   tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
   tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
     (a wstring as its own UTF-8 octets).

   Exit 0 on agreement, exit 1 with every disagreement named. Run with
   `dune test` from runtime/ocaml, or
   `ocamlc zdwire.ml tests/zdwire_selftest.ml -o selftest && ./selftest`. *)

let hex (bytes : Bytes.t) : string =
  let out = Buffer.create (2 * Bytes.length bytes) in
  Bytes.iter (fun c -> Buffer.add_string out (Printf.sprintf "%02x" (Char.code c))) bytes;
  Buffer.contents out

(* The specification framing; none of the three axes reads it. *)
let framing : Zdwire.framing =
  { Zdwire.pid_flags = "sentinel-must-understand"; highest_short_id = 16128;
    pad_declared_length = true; pad_declared_optional_length = true;
    member_must_understand = true; pad_after_value = true }

let failed = ref false

let check (name : string) (ok : bool) : unit =
  if not ok then begin
    prerr_endline ("zdwire_selftest: " ^ name);
    failed := true
  end

let () =
  (* ElementPadding::Packed: each collection element counts its
     alignment from its own first octet. The second Cell's long sits at
     offset six, where the clause would put it at eight. *)
  let w = Zdwire.new_writer Zdwire.Little 8 framing false in
  let saved = Zdwire.begin_packed_element w in
  Zdwire.write_i32 w 1l;
  Zdwire.write_i16 w 2;
  Zdwire.finish_packed_element w saved;
  let saved = Zdwire.begin_packed_element w in
  Zdwire.write_i32 w 3l;
  Zdwire.write_i16 w 4;
  Zdwire.finish_packed_element w saved;
  Zdwire.write_u8 w 119;
  check "a packed element starts where the last ended"
    (hex (Zdwire.finish w) = "01000000020003000000040077");

  (* And back through the reader's packed pair. *)
  let r =
    Zdwire.new_reader "\x01\x00\x00\x00\x02\x00\x03\x00\x00\x00\x04\x00\x77"
      Zdwire.Little 8 framing
  in
  let saved = Zdwire.reader_begin_packed_element r in
  let a1 = Zdwire.read_i32 r in
  let b1 = Zdwire.read_i16 r in
  Zdwire.reader_finish_packed_element r saved;
  let saved = Zdwire.reader_begin_packed_element r in
  let a2 = Zdwire.read_i32 r in
  let b2 = Zdwire.read_i16 r in
  Zdwire.reader_finish_packed_element r saved;
  check "a packed element reads back"
    (a1 = 1l && b1 = 2 && a2 = 3l && b2 = 4 && Zdwire.read_u8 r = 119);

  (* SequenceElementDelimiter::BeforeEachNonFinalElement: an element's
     own octet length in DHEADER form, 06 00 00 00 for the six-octet
     Cell that follows. *)
  let w = Zdwire.new_writer Zdwire.Little 8 framing false in
  let d = Zdwire.write_dheader w in
  Zdwire.write_i32 w 1l;
  Zdwire.write_i16 w 2;
  Zdwire.finish_dheader w d;
  check "a sequence element's own length in DHEADER form"
    (hex (Zdwire.finish w) = "06000000010000000200");

  (* WstringCodeUnits::Utf8: three octets and a NUL, the length 4. *)
  let w = Zdwire.new_writer Zdwire.Little 8 framing false in
  Zdwire.write_wstring_utf8_octets w "abc" "octets/terminated/2";
  check "a UTF-8 wstring is its octets and a NUL"
    (hex (Zdwire.finish w) = "0400000061626300");

  let w = Zdwire.new_writer Zdwire.Little 8 framing false in
  Zdwire.write_wstring_utf8_octets w "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z"
    "octets/terminated/2";
  check "a multi-byte UTF-8 wstring counts its octets"
    (hex (Zdwire.finish w) = "0d000000c3a4c3b6c3bc20e188b4205a00");

  let w = Zdwire.new_writer Zdwire.Little 8 framing false in
  Zdwire.write_wstring_utf8_octets_bounded w "\xe1\x88\xb4\xc3\xa4Z" 3
    "octets/terminated/2";
  check "a bounded UTF-8 wstring holds three units"
    (hex (Zdwire.finish w) = "07000000e188b4c3a45a00");

  let r = Zdwire.new_reader "\x0d\x00\x00\x00\xc3\xa4\xc3\xb6\xc3\xbc\x20\xe1\x88\xb4\x20\x5a\x00"
      Zdwire.Little 8 framing in
  check "a UTF-8 wstring reads back"
    (Zdwire.read_wstring_utf8_octets r "octets/terminated/2" = "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z");

  if !failed then exit 1;
  print_endline "zdwire_selftest: ok"
