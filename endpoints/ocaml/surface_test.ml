(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   The wire-core operations this SDK grew so that a caller can write every
   construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap,
   the delimiter, the member header in all eight length codes, the PL_CDR1
   parameter list, the wide character and string, the packed decimal and the
   bitset anchor.

   ## The expected octets are foreign

   Every multi-member case here is the byte string a *vendor's own*
   serialiser produced, recorded in
   `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` from
   `tools/byte-gate/cases/<case>` and named in the test. Cyclone DDS, RTI
   Connext, Fast DDS, omniORB and JacORB wrote these; comparing this SDK
   against a reference file this project also wrote would only show that two
   halves of one opinion agree.

   Where the vendors disagree the test says which reading this binding takes
   and which it does not, because a wire question with two answers in the
   field is a measurement and not a defect.

   Each operation is exercised at the ends of its range and not only in the
   middle. OCaml's `int` is 63 bits and not 64, so every 64-bit wire quantity
   here is an `int64` and the test says where the difference would show.

   Build and run with `make surface_test`. *)

open Zerodds.Wire

let failures = ref []

let hex (b : bytes) =
  let out = Buffer.create (Bytes.length b * 2) in
  Bytes.iter (fun c -> Buffer.add_string out (Printf.sprintf "%02x" (Char.code c))) b;
  Buffer.contents out

let unhex s =
  let n = String.length s / 2 in
  Bytes.init n (fun i -> Char.chr (int_of_string ("0x" ^ String.sub s (2 * i) 2)))

let check name got want =
  if got <> want then
    failures := Printf.sprintf "%s:\n    got  %s\n    want %s" name got want :: !failures

let check_int name got want = check name (string_of_int got) (string_of_int want)
let check_bool name got want = check name (string_of_bool got) (string_of_bool want)

let refuses name f =
  match f () with
  | () -> failures := (name ^ ": it was accepted") :: !failures
  | exception Invalid_argument _ -> ()
  | exception Failure _ -> ()

(* --- the alignment cap the encoding sets ---------------------------------

   tools/byte-gate/cases/ext_appendable: `@appendable struct Ext { long a;
   double b; short c; }` with a = 7, b = 1.5, c = -3. XTypes 1.3 §7.4.1 puts
   MAXALIGN at eight under XCDR1, so the `double` takes four octets of
   padding there and none under XCDR2 — which is exactly the difference
   between the two vendor rows. *)

let ext_appendable endian encoding framed =
  let w = writer endian encoding in
  let mark = if framed then dheader_begin w else 0 in
  put_u32 w 7;
  put_f64 w 1.5;
  put_u16 w (-3);
  if framed then dheader_end w mark;
  hex (bytes w)

(* Cyclone, RTI and Fast DDS all wrote these four, octet for octet. *)
let () =
  check "ext_appendable xcdr1 le" (ext_appendable LE XCDR1 false)
    "0700000000000000000000000000f83ffdff";
  check "ext_appendable xcdr1 be" (ext_appendable BE XCDR1 false)
    "00000007000000003ff8000000000000fffd";
  check "ext_appendable xcdr2 le" (ext_appendable LE XCDR2 true)
    "0e00000007000000000000000000f83ffdff";
  check "ext_appendable xcdr2 be" (ext_appendable BE XCDR2 true)
    "0000000e000000073ff8000000000000fffd";
  (* The cap is the encoding's and not a constant. *)
  check_int "max_align xcdr1" (writer BE XCDR1).max_align 8;
  check_int "max_align xcdr2" (writer BE XCDR2).max_align 4

(* --- the 64-bit float ---------------------------------------------------- *)

let () =
  List.iter
    (fun (value, want) ->
      let w = writer LE XCDR2 in
      put_f64 w value;
      check (Printf.sprintf "put_f64 %h" value) (hex (bytes w)) want)
    [ (1.5, "000000000000f83f");                   (* the vendors' own sample *)
      (0.0, "0000000000000000");
      (-0.0, "0000000000000080");                  (* a signed zero keeps its sign *)
      (1.7976931348623157e308, "ffffffffffffef7f");  (* MAX *)
      (5e-324, "0100000000000000");                (* the smallest subnormal *)
      (infinity, "000000000000f07f") ]

(* --- DHEADER ------------------------------------------------------------- *)

let () =
  (* §7.4.3.5: three octets of content, so the DHEADER reads 3 and the four
     octets it occupies are not in it. *)
  let w = writer LE XCDR2 in
  let mark = dheader_begin w in
  put_u8 w 1; put_u8 w 2; put_u8 w 3;
  dheader_end w mark;
  check "dheader counts the body" (hex (bytes w)) "03000000010203";
  let e = writer BE XCDR2 in
  dheader_end e (dheader_begin e);
  check "dheader of an empty body" (hex (bytes e)) "00000000"

(* --- the member header, in every length code -----------------------------

   tools/byte-gate/cases/ext_mutable: the same three members, `@mutable`.
   Cyclone and RTI wrote the XCDR2 rows identically. *)

let ext_mutable_xcdr2 endian =
  let w = writer endian XCDR2 in
  let d = dheader_begin w in
  let m = emheader_begin w 0 false lc2 in
  put_u32 w 7;
  emheader_end w m lc2;
  let m = emheader_begin w 1 false lc3 in
  put_f64 w 1.5;
  emheader_end w m lc3;
  let m = emheader_begin w 2 false lc1 in
  put_u16 w (-3);
  emheader_end w m lc1;
  dheader_end w d;
  hex (bytes w)

let () =
  check "ext_mutable xcdr2 le" (ext_mutable_xcdr2 LE)
    "1a000000000000200700000001000030000000000000f83f02000010fdff";
  check "ext_mutable xcdr2 be" (ext_mutable_xcdr2 BE)
    "0000001a2000000000000007300000013ff800000000000010000002fffd"

(* tools/byte-gate/cases/pid_3f00: `@mutable struct PidEdge { @id(16127) long
   a; @id(16128) long b; }`. Cyclone, RTI and Fast DDS agree on the XCDR2
   row, which is where the member id sits inside the EMHEADER's low 28
   bits. *)
let () =
  let w = writer LE XCDR2 in
  let d = dheader_begin w in
  let m = emheader_begin w 16127 false lc2 in
  put_u32 w 7;
  emheader_end w m lc2;
  let m = emheader_begin w 16128 false lc2 in
  put_u32 w 9;
  emheader_end w m lc2;
  dheader_end w d;
  check "pid_3f00 xcdr2 le" (hex (bytes w)) "10000000ff3e002007000000003f002009000000"

(* tools/byte-gate/cases/lc_5_6_7: five collection members whose length codes
   are LC5, LC5, LC6, LC7 and — where the two vendors part — LC4 for Cyclone
   and LC3 for Fast DDS. Both are written here, because a binding that could
   only produce one of them could not read the other's wire back. *)
let lc_5_6_7 tail_code =
  let w = writer LE XCDR2 in
  let d = dheader_begin w in
  let m = emheader_begin w 1 false lc5 in
  put_string w "abc";
  emheader_end w m lc5;
  let m = emheader_begin w 2 false lc5 in
  put_seq_u8 w (Bytes.of_string "\001\002\003");
  emheader_end w m lc5;
  let m = emheader_begin w 3 false lc6 in
  put_u32 w 1; put_u32 w 7;
  emheader_end w m lc6;
  let m = emheader_begin w 4 false lc7 in
  put_u32 w 1; put_f64 w 1.5;
  emheader_end w m lc7;
  let m = emheader_begin w 5 false tail_code in
  put_u32 w 2; put_u16 w 4; put_u16 w 5;
  emheader_end w m tail_code;
  dheader_end w d;
  hex (bytes w)

let lc_567_head =
  "01000050" ^ "04000000" ^ "61626300"
  ^ "02000050" ^ "03000000" ^ "01020300"
  ^ "03000060" ^ "01000000" ^ "07000000"
  ^ "04000070" ^ "01000000" ^ "000000000000f83f"

let () =
  check "lc_5_6_7 xcdr2 le (Cyclone, LC4 tail)" (lc_5_6_7 lc4)
    ("44000000" ^ lc_567_head ^ "05000040" ^ "08000000" ^ "02000000" ^ "04000500");
  check "lc_5_6_7 xcdr2 le (Fast DDS, LC3 tail)" (lc_5_6_7 lc3)
    ("40000000" ^ lc_567_head ^ "05000030" ^ "02000000" ^ "04000500")

(* The must-understand flag is bit 31 and not bit 30: 0x80000000 | (2 << 28). *)
let () =
  let w = writer LE XCDR2 in
  let m = emheader_begin w 0 true lc2 in
  put_u32 w 0;
  emheader_end w m lc2;
  check "must-understand is bit 31" (hex (bytes w)) "000000a000000000"

(* --- the PL_CDR1 parameter list ------------------------------------------

   tools/byte-gate/cases/ext_mutable, XCDR1. RTI's rows are the short-header
   form this binding writes. The three things they pin: the `double`'s
   parameter declares 8 and not 12, because its value aligns against its own
   block; the `short`'s declares 4 and not 2, because the padding that brings
   the next header back to a four-octet boundary is inside the length; and
   the sentinel carries the must-understand flag, so it is 0x7F02.

   Cyclone writes the same sample with an extended header on every parameter
   (56 octets against RTI's 32) and Fast DDS declares 2 for the `short` and a
   bare 0x3F02 sentinel. Three readings, all on the wire today; this binding
   takes RTI's. *)

let ext_mutable_xcdr1 endian =
  let w = writer endian XCDR1 in
  let p = pid_begin w 0 false in
  put_u32 w 7;
  pid_end w p;
  let p = pid_begin w 1 false in
  put_f64 w 1.5;
  pid_end w p;
  let p = pid_begin w 2 false in
  put_u16 w (-3);
  pid_end w p;
  pid_list_end w;
  hex (bytes w)

let () =
  check "ext_mutable xcdr1 le (RTI)" (ext_mutable_xcdr1 LE)
    "000004000700000001000800000000000000f83f02000400fdff0000027f0000";
  check "ext_mutable xcdr1 be (RTI)" (ext_mutable_xcdr1 BE)
    "0000000400000007000100083ff800000000000000020004fffd00007f020000"

(* tools/byte-gate/cases/pid_3f00 under XCDR1: RTI and Fast DDS both carry
   @id(16128) in the *short* header, which is what puts 0x3F00 at the top of
   its range inclusive rather than one past it. *)
let () =
  let w = writer LE XCDR1 in
  let p = pid_begin w 0x3EFF false in
  put_u32 w 7;
  pid_end w p;
  let p = pid_begin w 0x3F00 false in
  put_u32 w 9;
  pid_end w p;
  pid_list_end w;
  check "pid_3f00 xcdr1 le (RTI)" (hex (bytes w))
    "ff3e040007000000003f040009000000027f0000"

(* The extended header, for an id past 0x3F00. The marker carries the
   must-understand flag whatever the member says; the member's own flag is
   bit 30 of the eMemberHeader. *)
let () =
  let w = writer LE XCDR1 in
  let p = pid_extended_begin w 16383 false in
  put_u32 w 0xFFFFFFFE;
  pid_end w p;
  pid_list_end w;
  check "extended parameter header" (hex (bytes w))
    "017f0800ff3f000004000000feffffff027f0000";
  let m = writer LE XCDR1 in
  let p = pid_begin m 0 true in
  put_u32 m 7;
  pid_end m p;
  check "the parameter must-understand flag is bit 14" (hex (bytes m))
    "0040040007000000"

(* --- the wide character and string ---------------------------------------

   tools/byte-gate/cases/wchar_member: `@final struct WCh { wchar c; octet
   tail; }` with c = U+0061 and tail = 0xAB. Cyclone and Fast DDS write two
   octets in both encodings and RTI writes two under XCDR2; RTI's XCDR1
   `wchar` is four octets, which is the one reading this binding does not
   take. *)
let () =
  List.iter
    (fun (endian, want) ->
      List.iter
        (fun encoding ->
          let w = writer endian encoding in
          put_wchar w 0x0061;
          put_u8 w 0xAB;
          check "wchar_member" (hex (bytes w)) want)
        [ XCDR1; XCDR2 ])
    [ (LE, "6100ab"); (BE, "0061ab") ];
  (* A `wchar` aligns to two, so a `char` in front costs one octet of pad. *)
  let w = writer LE XCDR2 in
  put_char w (Char.code 'a');
  put_wchar w (Char.code 'z');
  check "wchar aligns to two" (hex (bytes w)) "61007a00"

(* tools/byte-gate/cases/wstring_member: w = "ab", tail = 0xAB. Cyclone and
   RTI-under-XCDR2 count **octets** (4) where Fast DDS counts code units (2).
   Under big endian Cyclone leaves the units little-endian — it copies the
   UTF-16 buffer — and RTI swaps them; this binding swaps, so its BE form is
   RTI's.

   tools/byte-gate/cases/wstring_nonbmp: U+1D11E as the surrogate pair
   D834 DD1E. Cyclone and RTI both write four octets; Fast DDS writes one
   unit, 0xD11E, which is neither half of the pair. *)
let () =
  List.iter
    (fun (endian, units, want, name) ->
      let w = writer endian XCDR2 in
      put_wstring w units;
      put_u8 w 0xAB;
      check name (hex (bytes w)) want)
    [ (LE, [| 0x0061; 0x0062 |], "0400000061006200ab",
       "wstring_member le (Cyclone + RTI)");
      (BE, [| 0x0061; 0x0062 |], "0000000400610062ab", "wstring_member be (RTI)");
      (LE, [| 0xD834; 0xDD1E |], "0400000034d81eddab",
       "wstring_nonbmp le (Cyclone + RTI)");
      (BE, [| 0xD834; 0xDD1E |], "00000004d834dd1eab", "wstring_nonbmp be (RTI)") ];
  let e = writer LE XCDR2 in
  put_wstring e [||];
  check "an empty wstring" (hex (bytes e)) "00000000"

(* --- the bitset anchor ---------------------------------------------------

   tools/wire-oracle/cases/bitset_chain: `bitset B3` chains 8 + 8 + 4 bits
   into one four-octet anchor, inside `@final struct BitChain { octet head;
   B3 value; octet tail; }` with head = 1, a = 0x11, b = 0x22, c = 0x3 and
   tail = 0xAB. Only Fast DDS produced octets — Cyclone's and RTI's IDL front
   ends do not parse `bitset` at all, which is itself the measurement
   recorded in vendor_bytes.rows.tsv. *)
let () =
  List.iter
    (fun (endian, want) ->
      let w = writer endian XCDR2 in
      put_u8 w 1;
      put_bitset w (Int64.of_int (0x11 lor (0x22 lsl 8) lor (0x3 lsl 16))) 4;
      put_u8 w 0xAB;
      check "bitset_chain (Fast DDS)" (hex (bytes w)) want)
    [ (LE, "0100000011220300ab"); (BE, "0100000000032211ab") ];
  (* tools/wire-oracle/cases/bitset_overwide: `bitfield<3> v` assigned 200.
     The holder keeps three bits, 200 mod 8 is 0, and the anchor is one octet
     — so it aligns to one and the `octet` behind it needs no padding. *)
  List.iter
    (fun endian ->
      let w = writer endian XCDR2 in
      put_bitset w (Int64.of_int (200 land 0x7)) 1;
      put_u8 w 0xAB;
      check "bitset_overwide (Fast DDS)" (hex (bytes w)) "00ab")
    [ LE; BE ];
  (* An eight-octet anchor is the one place a bitset differs between the two
     encodings: the anchor's own alignment, capped at four under XCDR2. *)
  let one = writer LE XCDR1 in
  put_u8 one 0xAB;
  put_bitset one 1L 8;
  check "an eight-octet anchor under XCDR1" (hex (bytes one))
    "ab000000000000000100000000000000";
  let two = writer LE XCDR2 in
  put_u8 two 0xAB;
  put_bitset two 1L 8;
  check "an eight-octet anchor under XCDR2" (hex (bytes two))
    "ab0000000100000000000000"

(* OCaml's `int` is 63 bits, so `0x8000000000000000` has no `int` at all: an
   eight-octet anchor with its top bit set is exactly where a path through
   `int` would drop the one bit that distinguishes it from zero. That is why
   [put_bitset] and [get_bitset] speak `int64`. *)
let () =
  check_int "OCaml's int is 63 bits here" Sys.int_size 63;
  let w = writer LE XCDR2 in
  put_bitset w Int64.min_int 8;
  check "a top-bit anchor" (hex (bytes w)) "0000000000000080";
  let r = reader (bytes w) LE XCDR2 in
  check "a top-bit anchor reads back"
    (Int64.to_string (get_bitset r 8)) (Int64.to_string Int64.min_int)

(* --- the packed decimal --------------------------------------------------

   crates/ast-layout/tests/fixtures/vendor_bytes.md §"`fixed<P,S>`, measured
   against two CORBA ORBs": no DDS generator parses the declaration at all —
   Cyclone, RTI, Fast DDS and OpenDDS's tao_idl each refuse it — so the
   oracle is omniORB 4.3.3 and JacORB 3.9, through
   `tools/wire-oracle/fixed-orbs.sh`. The struct below is theirs, member for
   member, and the octets are omniORB's.

   JacORB agrees on eight of the nine and writes one octet for `0.00` where
   omniORB writes three: its `write_fixed` takes a `BigDecimal`, and
   `new BigDecimal("0.00").precision()` is 1 in Java, so the declared width
   is gone before the ORB sees the value. This binding takes omniORB's
   reading — the octet count comes from the declared digits and not from the
   value. *)
let () =
  let w = writer LE XCDR2 in
  put_u8 w 0x77;
  put_fixed w "123.45" 5 2;
  put_fixed w "-123.45" 5 2;
  put_fixed w "0.00" 5 2;
  put_fixed w "12.34" 4 2;
  put_fixed w "0.123" 3 3;
  put_fixed w "7" 1 0;
  put_fixed w "42" 2 0;
  put_fixed w "1234567890123456789012345678901" 31 0;
  put_u8 w 0xAB;
  check "FixedAll (omniORB 4.3.3)" (hex (bytes w))
    ("77" ^ "12345c" ^ "12345d" ^ "00000c" ^ "01234c" ^ "123c" ^ "7c" ^ "042c"
    ^ "1234567890123456789012345678901c" ^ "ab");
  (* The octets do not swap: packed BCD is a byte sequence and not an
     integer, which is why omniORB records the same string under
     `--endian be`. *)
  let le = writer LE XCDR1 and be = writer BE XCDR2 in
  put_fixed le "123.45" 5 2;
  put_fixed be "123.45" 5 2;
  check "fixed is independent of the byte order" (hex (bytes le)) (hex (bytes be))

let () =
  List.iter
    (fun (value, digits, scale, want) ->
      let w = writer LE XCDR2 in
      put_fixed w value digits scale;
      check (Printf.sprintf "put_fixed %s %d %d" value digits scale) (hex (bytes w)) want;
      check_int
        (Printf.sprintf "put_fixed %s %d %d octet count" value digits scale)
        (Bytes.length (bytes w)) ((digits + 2) / 2))
    [ ("1", 1, 0, "1c"); ("9", 1, 0, "9c"); ("-9", 1, 0, "9d");
      ("-0", 1, 0, "0c"); ("-0.000", 3, 3, "000c");
      ("0.123", 3, 3, "123c"); ("-0.001", 3, 3, "001d");
      ("7.5", 9, 3, "000007500c");
      ("-1234567890123456789012345678901", 31, 0,
       "1234567890123456789012345678901d") ]

(* `omniidl` refuses the same three shapes, in its own words: `fixed<0,0>` is
   `Size must be at least 1`, `fixed<2,5>` is `Fixed point scale factor is
   greater than the number of digits`, `fixed<32,0>` is `Fixed point values
   may not have more than 31 digits`. *)
let () =
  List.iter
    (fun (name, value, digits, scale) ->
      refuses name (fun () -> ignore (fixed_octets value digits scale)))
    [ ("fixed<0,0>", "1", 0, 0); ("fixed<32,0>", "1", 32, 0);
      ("fixed<2,5>", "1.0", 2, 5);
      ("seven integer digits into six", "1234567", 6, 0);
      ("four fraction digits into three", "1.2345", 6, 3);
      ("not a decimal", "twelve", 4, 0); ("no digits at all", "", 4, 0) ]

(* --- the ends of every integer range -------------------------------------

   `put_u16 w 70000` used to answer `7011`: `le_of_int` masks each octet, so
   a value wider than the member went out as a frame of the right length
   holding a different number. Both spellings of an argument are taken — the
   unsigned pattern and the signed IDL value — and nothing outside their
   union is. *)

let () =
  List.iter
    (fun (name, put, ok, refused) ->
      List.iter
        (fun (value, want) ->
          let w = writer LE XCDR2 in
          put w value;
          check (Printf.sprintf "%s %d" name value) (hex (bytes w)) want)
        ok;
      List.iter
        (fun value ->
          refuses (Printf.sprintf "%s %d" name value) (fun () ->
              put (writer LE XCDR2) value))
        refused)
    [ ("put_u8", put_u8,
       [ (0, "00"); (-1, "ff"); (-128, "80"); (127, "7f"); (255, "ff") ],
       [ 256; -129; 300; 70000 ]);
      ("put_u16", put_u16,
       [ (0, "0000"); (-1, "ffff"); (-32768, "0080"); (32767, "ff7f");
         (65535, "ffff") ],
       [ 65536; -32769; 70000 ]);
      ("put_u32", put_u32,
       [ (0, "00000000"); (-1, "ffffffff"); (-2147483648, "00000080");
         (4294967295, "ffffffff") ],
       [ 4294967296; -2147483649 ]) ]

(* The 64-bit write takes an `int64` because OCaml's `int` is 63 bits: the
   top bit of an `unsigned long long` has no `int`, and nothing outside
   `int64` can be written down at all, so there is no range left to refuse.
   Both ends of it are here. *)
let () =
  List.iter
    (fun (value, want) ->
      let w = writer LE XCDR2 in
      put_u64 w value;
      check (Printf.sprintf "put_u64 %Ld" value) (hex (bytes w)) want)
    [ (0L, "0000000000000000");
      (-1L, "ffffffffffffffff");           (* = 0xFFFFFFFFFFFFFFFF *)
      (Int64.min_int, "0000000000000080"); (* = 0x8000000000000000 *)
      (Int64.max_int, "ffffffffffffff7f");
      (9007199254740993L, "0100000000002000") ]

(* 2^53 + 1 has no `float` — it rounds to 2^53 — and OCaml's `int` holds it
   exactly. The `int64` argument is what keeps the difference visible: a
   64-bit write that took a `float` would answer `2^53` here without a word. *)
let () =
  check_bool "2^53 + 1 is not a float"
    (Int64.of_float (Int64.to_float 9007199254740993L) = 9007199254740993L) false;
  let w = writer LE XCDR2 in
  put_u64 w 9007199254740993L;
  check "put_u64 keeps 2^53 + 1" (hex (bytes w)) "0100000000002000"

(* `char` and `wchar` at the ends of their code point ranges. U+0161's low
   octet is 0x61, which is `a`: a truncating writer produces a frame of the
   right length holding a plausible, wrong character. *)
let () =
  let w = writer LE XCDR2 in
  put_char w 0x00;
  put_char w 0xFF;
  check "char at both ends" (hex (bytes w)) "00ff";
  refuses "char U+0100" (fun () -> put_char (writer LE XCDR2) 0x100);
  refuses "char U+0161" (fun () -> put_char (writer LE XCDR2) 0x161);
  refuses "char below zero" (fun () -> put_char (writer LE XCDR2) (-1));
  let v = writer LE XCDR2 in
  put_wchar v 0x0000;
  put_wchar v 0xFFFF;
  check "wchar at both ends" (hex (bytes v)) "0000ffff";
  refuses "wchar U+10000" (fun () -> put_wchar (writer LE XCDR2) 0x10000)

(* The header fields at the ends of their own ranges. *)
let () =
  ignore (emheader_begin (writer LE XCDR2) 0x0FFFFFFF false lc2);
  refuses "a 29-bit member id" (fun () ->
      ignore (emheader_begin (writer LE XCDR2) 0x10000000 false lc2));
  refuses "length code 8" (fun () ->
      ignore (emheader_begin (writer LE XCDR2) 0 false 8));
  refuses "length code -1" (fun () ->
      ignore (emheader_begin (writer LE XCDR2) 0 false (-1)));
  refuses "LC2 over a body that is not four octets" (fun () ->
      let w = writer LE XCDR2 in
      let m = emheader_begin w 0 false lc2 in
      put_u16 w 1;
      emheader_end w m lc2);
  ignore (pid_begin (writer LE XCDR1) 0x3F00 false);
  refuses "a short header past 0x3F00" (fun () ->
      ignore (pid_begin (writer LE XCDR1) 0x3F01 false));
  refuses "a bitset anchor of three" (fun () -> put_bitset (writer LE XCDR2) 0L 3);
  refuses "a holder wider than its anchor" (fun () ->
      put_bitset (writer LE XCDR2) 256L 1)

(* --- the tail ------------------------------------------------------------

   tools/byte-gate/cases/tail_pad: `@final struct Odd { long a; short b; }`
   with a = 1, b = 2. All four vendors stop at six octets in both encodings —
   a `@final` aggregate is not padded out to its own alignment. *)
let () =
  List.iter
    (fun (endian, want) ->
      List.iter
        (fun encoding ->
          let w = writer endian encoding in
          put_u32 w 1;
          put_u16 w 2;
          check "tail_pad" (hex (bytes w)) want)
        [ XCDR1; XCDR2 ])
    [ (LE, "010000000200"); (BE, "000000010002") ]

(* --- the reading direction -----------------------------------------------

   Every write has a reader, and the vendor's own octets go in: a writer with
   no matching reader is not an endpoint binding. *)

let () =
  List.iter
    (fun encoding ->
      List.iter
        (fun endian ->
          let w = writer endian encoding in
          put_u8 w 0xFF;
          put_bool w true;
          put_char w 0xFF;
          put_u16 w 0xFFFF;
          put_u32 w 0xFFFFFFFF;
          put_u64 w (-1L);
          put_f32 w (-3.5);
          put_f64 w 1.5;
          put_wchar w 0xFFFF;
          put_string w "gr\xc3\xbc\xc3\x9fe";
          put_seq_u8 w (Bytes.of_string "\xDE\xAD");
          put_wstring w [| 0xD83D; 0xDE00 |];
          put_bitset w 0xBC614E00000FFFFFL 8;
          put_fixed w "-000000.001" 9 3;

          let buf = bytes w in
          let r = reader buf endian encoding in
          check_int "round trip u8" (get_u8 r) 0xFF;
          check_bool "round trip bool" (get_bool r) true;
          check_int "round trip char" (get_char r) 0xFF;
          check_int "round trip u16" (get_u16 r) 0xFFFF;
          check_int "round trip u32" (get_u32 r) 0xFFFFFFFF;
          check "round trip u64" (Int64.to_string (get_u64 r)) "-1";
          check "round trip f32" (Printf.sprintf "%h" (get_f32 r))
            (Printf.sprintf "%h" (-3.5));
          check "round trip f64" (Printf.sprintf "%h" (get_f64 r))
            (Printf.sprintf "%h" 1.5);
          check_int "round trip wchar" (get_wchar r) 0xFFFF;
          check "round trip string" (get_string r) "gr\xc3\xbc\xc3\x9fe";
          check "round trip seq_u8" (Bytes.to_string (get_seq_u8 r)) "\xDE\xAD";
          let units = get_wstring r in
          check_int "round trip wstring units" (Array.length units) 2;
          check_int "round trip wstring[0]" units.(0) 0xD83D;
          check_int "round trip wstring[1]" units.(1) 0xDE00;
          check "round trip bitset" (Int64.to_string (get_bitset r 8))
            (Int64.to_string 0xBC614E00000FFFFFL);
          check "round trip fixed" (get_fixed r 9 3) "-000000.001";
          check_int "round trip consumed everything" r.pos (Bytes.length buf))
        [ LE; BE ])
    [ XCDR1; XCDR2 ]

(* The vendors' own octets, straight in. `ext_mutable` under XCDR2 is
   Cyclone's and RTI's identical row; reading it back is what makes this a
   binding and not a writer. *)
let () =
  let buf = unhex "1a000000000000200700000001000030000000000000f83f02000010fdff" in
  let r = reader buf LE XCDR2 in
  check_int "vendor dheader" (dheader_read r) 26;
  List.iter
    (fun (want_id, want_lc, want_extent) ->
      let id, mu, lc, extent = emheader_read r in
      check_int "vendor emheader id" id want_id;
      check_int "vendor emheader lc" lc want_lc;
      check_int "vendor emheader extent" extent want_extent;
      check_bool "vendor emheader mu" mu false;
      if lc = lc2 then check_int "vendor member a" (get_u32 r) 7
      else if lc = lc3 then
        check "vendor member b" (Printf.sprintf "%h" (get_f64 r)) (Printf.sprintf "%h" 1.5)
      else check_int "vendor member c" (get_u16 r) 0xFFFD)
    [ (0, lc2, 4); (1, lc3, 8); (2, lc1, 2) ];
  check_int "vendor xcdr2 fully consumed" r.pos (Bytes.length buf)

(* RTI's XCDR1 row for the same sample, through the parameter-list reader. *)
let () =
  let buf =
    unhex "000004000700000001000800000000000000f83f02000400fdff0000027f0000"
  in
  let r = reader buf LE XCDR1 in
  List.iter
    (fun (want_id, want_len) ->
      let id, mu, length, list_end = pid_read r in
      check_bool "vendor pid end" list_end false;
      check_int "vendor pid id" id want_id;
      check_int "vendor pid length" length want_len;
      check_bool "vendor pid mu" mu false;
      let body = r.pos in
      (if id = 0 then check_int "vendor parameter a" (get_u32 r) 7
       else if id = 1 then
         check "vendor parameter b" (Printf.sprintf "%h" (get_f64 r))
           (Printf.sprintf "%h" 1.5)
       else check_int "vendor parameter c" (get_u16 r) 0xFFFD);
      pid_skip r body length)
    [ (0, 4); (1, 8); (2, 4) ];
  let _, _, _, list_end = pid_read r in
  check_bool "vendor sentinel ends the list" list_end true;
  check_int "vendor xcdr1 fully consumed" r.pos (Bytes.length buf)

(* Cyclone's lc_5_6_7 row, whose LC5, LC6 and LC7 members take their length
   from the body's own leading word. *)
let () =
  let r = reader (unhex (lc_5_6_7 lc4)) LE XCDR2 in
  ignore (dheader_read r);
  List.iter
    (fun (want_id, want_lc, want_extent) ->
      let id, _, lc, extent = emheader_read r in
      check_int "lc_5_6_7 read id" id want_id;
      check_int "lc_5_6_7 read lc" lc want_lc;
      check_int "lc_5_6_7 read extent" extent want_extent;
      r.pos <- r.pos + extent)
    [ (1, lc5, 8); (2, lc5, 7); (3, lc6, 8); (4, lc7, 12); (5, lc4, 8) ]

let () =
  match !failures with
  | [] ->
    print_endline
      "endpoints/ocaml: the surface holds, against Cyclone, RTI, Fast DDS and omniORB"
  | items ->
    List.iter (fun f -> prerr_endline f) (List.rev items);
    Printf.eprintf "%d failure(s)\n" (List.length items);
    exit 1
