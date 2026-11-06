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

  (* -- the operations no corpus case reaches -----------------------------

     The five the generated code reaches only in a dialect or with a member
     no corpus case carries: write_pid_extended, write_union_overlay,
     reader_clear_discard, reader_save and reader_restore
     (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
     vectors, each against an oracle outside this code. *)
  let specification : Zdwire.framing =
    { Zdwire.pid_flags = "sentinel-must-understand"; highest_short_id = 16128;
      pad_declared_length = false; pad_declared_optional_length = false;
      member_must_understand = true; pad_after_value = true } in

  (* The first parameter of `annotation-autoid-sequential` as Cyclone DDS
     recorded it, XCDR1 little endian (tests/interop-corpus/expected-bytes.tsv). *)
  let pid = Zdwire.new_writer Zdwire.Little 8 specification false in
  let header = Zdwire.write_pid_extended pid 0 Zdwire.Role_member false in
  Zdwire.write_i32 pid (-1234567l);
  Zdwire.finish_pid pid header;
  check "write_pid_extended writes Cyclone's recorded parameter"
    (hex (Zdwire.finish pid) = "017f0800" ^ "00000000" ^ "04000000" ^ "7929edff");

  (* A `short` read through a four-octet and a one-octet `default:` branch:
     Cyclone DDS 11.0.1 writes the union's storage, the member's octets with
     zeros above (`combined-everything`, crates/wire-codec/src/encode.rs
     `write_selected_branch`). *)
  let overlay = Zdwire.new_writer Zdwire.Little 4 specification false in
  let pattern = Zdwire.bit_pattern_int (-2L) 2 in
  Zdwire.write_union_overlay overlay pattern 4;
  Zdwire.write_union_overlay overlay pattern 1;
  check "write_union_overlay writes the storage at the branch's width"
    (hex (Zdwire.finish overlay) = "feff0000" ^ "fe");

  (* XTypes 1.3 section 7.2.2.4.4.4.9: a member's explicit @default comes
     before Table 9's value, so the call site asks for DISCARD and takes it
     back. Any other refusal is not a narrowing and stays. *)
  let discarded = Zdwire.new_reader "\x01\x02\x03\x04" Zdwire.Little 4 specification in
  Zdwire.reader_refuse discarded Zdwire.St_discarded;
  check "reader_clear_discard takes back a discarded"
    (Zdwire.reader_clear_discard discarded && Zdwire.reader_status discarded = Zdwire.St_ok);
  let refused = Zdwire.new_reader "\x01\x02\x03\x04" Zdwire.Little 4 specification in
  Zdwire.reader_refuse refused Zdwire.St_out_of_range;
  check "reader_clear_discard leaves an out-of-range"
    ((not (Zdwire.reader_clear_discard refused))
     && Zdwire.reader_status refused = Zdwire.St_out_of_range);

  (* XTypes 1.3 section 7.4.3.5.3 rule (15): a delimited map<long, long> as
     its DHEADER and the pairs, without a count. The reader counts the pairs,
     goes back and reads them again; a refusal made while counting goes back
     with the cursor. *)
  let map = Zdwire.new_reader
      "\x10\x00\x00\x00\x01\x00\x00\x00\x0a\x00\x00\x00\x02\x00\x00\x00\x14\x00\x00\x00"
      Zdwire.Little 4 specification in
  let extent = Zdwire.read_dheader map in
  let end_at = Zdwire.reader_position map + extent in
  let mark = Zdwire.reader_save map in
  let pairs = ref 0 in
  while Zdwire.reader_position map < end_at && Zdwire.reader_status map = Zdwire.St_ok do
    ignore (Zdwire.read_i32 map);
    ignore (Zdwire.read_i32 map);
    incr pairs
  done;
  Zdwire.reader_refuse map Zdwire.St_out_of_range;
  Zdwire.reader_restore map mark;
  let k1 = Zdwire.read_i32 map in
  let v1 = Zdwire.read_i32 map in
  let k2 = Zdwire.read_i32 map in
  let v2 = Zdwire.read_i32 map in
  check "reader_save and reader_restore count a delimited map and read it again"
    (!pairs = 2 && k1 = 1l && v1 = 10l && k2 = 2l && v2 = 20l
     && Zdwire.reader_position map = 20 && Zdwire.reader_status map = Zdwire.St_ok);

  if !failed then exit 1;
  print_endline "zdwire_selftest: ok"
