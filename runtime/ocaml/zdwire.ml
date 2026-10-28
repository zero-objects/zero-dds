(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)

(* The OCaml wire runtime: DDS-XTypes 1.3 section 7.4 from the clause.

   The counterpart of crates/wire, runtime/c, runtime/python, runtime/go
   and runtime/d — a further implementation of the same clauses, not a
   binding to any of them. Where two columns disagree on an octet, one of
   them has misread the specification, and that is the point of having
   both. Nothing here decides a type question: which members are written,
   in what order, under which header and with which length code arrives
   as arguments from the generated codec.

   The operations take the shared walk's argument order and its
   snake_case spelling unchanged — write_enum w bit_bound value,
   write_bitset w anchor value — because this runtime was written against
   the walk rather than adapted to it afterwards. The receiver is the
   first argument, which is what `w.write_u32 x` becomes in a language
   whose functions have no receiver.

   A malformed stream or a value outside its type raises Wire_error and
   unwinds to the harness, which turns it into the verdict's exit code; a
   refusal about the *type* sets the reader's status instead and the
   generated decoder walks on to the end of the sample.

   An IDL `unsigned long long` is held as decimal text — OCaml has no
   unsigned integer — and an IDL `long double` too: OCaml's float is a
   binary64 and a binary128 has 113 significand bits. The conversions
   live here: a correctly rounded parse (to nearest, ties to even, in one
   rounding) over a small base-2^24 bignum, and the exact decimal
   expansion in the spelling the sibling columns' renderers produce.

   Requires OCaml >= 4.08 and a 64-bit platform: an `int` holds every
   `unsigned long` value only there, and the adapter's memory-shape table
   is written against that. *)

(* -- errors and control ---------------------------------------------------- *)

(* Every wire failure, with its position and reason in the text. *)
exception Wire_error of string

(* The generated decoder's early return; the codec function catches it. *)
exception Zd_return

(* The generated decoder's loop exit; every generated loop catches it. *)
exception Zd_break

let fail_at (at : int) (what : string) : 'a =
  raise (Wire_error (Printf.sprintf "at %d: %s" at what))

(* -- policies -------------------------------------------------------------- *)

(* The 64-bit holder the generated codec hands a *pattern* to, as opposed
   to a number: a bitmask's bits and a bitfield's. OCaml has no unsigned
   integer and the alias is transparent, so this is int64 to the type
   checker; it exists so the renderer can tell a widening that keeps the
   bit pattern from one that keeps the sign. See the `Convert` arm of
   crates/lang-render/src/ocaml.rs. *)
type u64 = int64

type endianness = Little | Big

(* What a receiving position does with a value it cannot hold.
   DDS-XTypes 1.3 section 7.2.2.7, Table 11. *)
type try_construct = Discard | Use_default | Trim

(* What a reader answers about the type, without raising. *)
type status = St_ok | St_truncated | St_impossible_length | St_out_of_range | St_discarded

let status_text = function
  | St_ok -> "ok"
  | St_truncated -> "truncated"
  | St_impossible_length -> "impossible-length"
  | St_out_of_range -> "out-of-range"
  | St_discarded -> "discarded"

(* Which of section 7.4.1.2's two roles a parameter header stands in. *)
type role = Role_member | Role_optional

(* The parameter ids and flags of XTypes 1.3 section 7.4.1.2.2 and
   DDSI-RTPS 2.5 section 9.4.2.11.2. *)
let pid_extended_id = 0x3F01
let pid_list_end = 0x3F02
let pid_ignore_id = 0x3F03
let pid_short_max_len = 0xFFFF
let pid_must_understand_bit = 0x4000
let pid_impl_extension_bit = 0x8000
let e_flag_must_understand = 0x4000_0000
let e_flag_impl_extension = 0x8000_0000
let e_member_id_mask = 0x0FFF_FFFF
let pid_id_mask = 0x3FFF

(* What flags a PL_CDR1 parameter id carries at `position` — "member",
   "sentinel" or "extended-marker". An unknown policy raises: a typo in a
   wire-affecting setting must be loud. *)
let pid_flags_at (policy : string) (position : string) : int =
  match policy with
  | "sentinel-must-understand" ->
    if position = "sentinel" || position = "extended-marker" then pid_must_understand_bit else 0
  | "bare" -> 0
  | "impl-extension-members" ->
    if position = "member" || position = "extended-marker" then
      pid_impl_extension_bit lor pid_must_understand_bit
    else 0
  | other -> fail_at 0 ("a PID-flag policy, not `" ^ other ^ "`")

(* How a PL_CDR1 parameter list is framed — the ways the measured
   implementations differ inside an XCDR1 sample. *)
type framing = {
  pid_flags : string;
  highest_short_id : int;
  (* Whether a @mutable member's declared length covers its padding. *)
  pad_declared_length : bool;
  (* The same question for an @optional member outside @mutable. *)
  pad_declared_optional_length : bool;
  (* Whether a member's own must-understand bit goes out at all. *)
  member_must_understand : bool;
  (* Whether a parameter's value is followed by its own padding to four.
     Under Fast DDS's framing it is not: the next header aligns itself
     and a final parameter ends the stream where its value does. *)
  pad_after_value : bool;
}

let pads_declared_length (f : framing) (r : role) : bool =
  match r with
  | Role_optional -> f.pad_declared_optional_length
  | Role_member -> f.pad_declared_length

(* One cell of the wstring C1 x C2 x C3 product: whether the length
   counts characters, whether a terminator goes out, and the unit width.
   The spellings are WstringLengthPolicy::as_str's. *)
let wstring_cell (policy : string) : (bool * bool * int) option =
  match policy with
  | "specification" | "octets/unterminated/2" -> Some (false, false, 2)
  | "character-count" | "characters/unterminated/2" -> Some (true, false, 2)
  | "character-count-terminated" | "characters/terminated/4" -> Some (true, true, 4)
  | "specification-wide-units" | "octets/unterminated/4" -> Some (false, false, 4)
  | "ros-type-support" | "characters/unterminated/4" -> Some (true, false, 4)
  | "octets/terminated/2" -> Some (false, true, 2)
  | "octets/terminated/4" -> Some (false, true, 4)
  | "characters/terminated/2" -> Some (true, true, 2)
  | _ -> None

let wstring_order_known (order : string) : bool =
  order = "stream" || order = "big-endian-always" || order = "byte-order-mark"
  || order = "host-unswapped"

let byte_order_mark = 0xFEFF
let swapped_byte_order_mark = 0xFFFE

(* -- handles --------------------------------------------------------------- *)

(* A DHEADER waiting for its length; `dh_at` below zero is the delimiter
   a @final aggregate does not write. *)
type dheader = { dh_at : int }

(* The DHEADER a value that takes no delimiter does not write; the
   writer's finish_dheader ignores it. *)
let no_dheader () : dheader = { dh_at = -1 }

(* An EMHEADER waiting for its NEXTINT; `em_next` is -1 where the length
   code carries the length itself. `em_word` is where the header word
   itself sits, for the short-length-code rewrite. *)
type emheader = { em_next : int; em_word : int }

let emheader_none : emheader = { em_next = -1; em_word = 0 }

(* A parameter header waiting for its length. *)
type pid = {
  p_at : int;
  p_extended : bool;
  p_saved_origin : int;
  p_role : role;
  p_bare : int;
  p_mu : bool;
}

let pid_none : pid =
  { p_at = -1; p_extended = false; p_saved_origin = 0; p_role = Role_member; p_bare = 0;
    p_mu = false }

(* Everything a two-pass read has to put back. *)
type reader_mark = { mk_at : int; mk_origin : int; mk_status : status }

let mark_none : reader_mark = { mk_at = 0; mk_origin = 0; mk_status = St_ok }

(* An EMHEADER as read. `em_extent` is how many octets the member
   occupies after the header word, the NEXTINT included.
   XTypes 1.3 section 7.4.3.4.2. *)
type emheader_read = {
  em_member_id : int;
  em_length_code : int;
  em_must_understand : bool;
  em_next_int : int;
  em_extent : int;
}

let emheader_read_none : emheader_read =
  { em_member_id = 0; em_length_code = 0; em_must_understand = false; em_next_int = -1;
    em_extent = 0 }

(* A parameter header as read. *)
type pid_read = {
  pr_pid : int;
  pr_length : int;
  pr_end : bool;
  pr_must_understand : bool;
  pr_impl_extension : bool;
  pr_reserved : bool;
  pr_ignore : bool;
  pr_role : role;
  pr_saved_origin : int;
  pr_value_at : int;
}

let pid_read_none : pid_read =
  { pr_pid = 0; pr_length = 0; pr_end = false; pr_must_understand = false;
    pr_impl_extension = false; pr_reserved = false; pr_ignore = false; pr_role = Role_member;
    pr_saved_origin = 0; pr_value_at = 0 }

(* Whether this parameter can be a member of the reading type: false for
   a vendor-private parameter and for a reserved id. *)
let pid_is_user_member (h : pid_read) : bool =
  (not h.pr_impl_extension) && (not h.pr_reserved) && not h.pr_ignore

(* -- helpers the generated code names -------------------------------------- *)

(* C11 section 7.24.4's unsigned-octet order, as the sign the walk
   compares. OCaml's own string order over the UTF-8 bytes is exactly
   that order. *)
let strcmp (a : string) (b : string) : int = compare a b

(* How many octets `text` is, which is what a string<N> bound counts. *)
let utf8_octets (text : string) : int = String.length text

(* The UTF-8 decode this runtime does for itself: one code point at
   `i`, and the width it took. Invalid UTF-8 answers the position. *)
let utf8_decode (text : string) (i : int) : (int * int, int) result =
  let n = String.length text in
  let byte k = Char.code text.[k] in
  let b0 = byte i in
  if b0 < 0x80 then Ok (b0, 1)
  else if b0 < 0xC2 then Error i
  else if b0 < 0xE0 then begin
    if i + 1 >= n then Error i
    else
      let b1 = byte (i + 1) in
      if b1 land 0xC0 <> 0x80 then Error i
      else Ok (((b0 land 0x1F) lsl 6) lor (b1 land 0x3F), 2)
  end
  else if b0 < 0xF0 then begin
    if i + 2 >= n then Error i
    else
      let b1 = byte (i + 1) and b2 = byte (i + 2) in
      if b1 land 0xC0 <> 0x80 || b2 land 0xC0 <> 0x80 then Error i
      else
        let point = ((b0 land 0x0F) lsl 12) lor ((b1 land 0x3F) lsl 6) lor (b2 land 0x3F) in
        if point < 0x800 || (point >= 0xD800 && point <= 0xDFFF) then Error i
        else Ok (point, 3)
  end
  else if b0 < 0xF5 then begin
    if i + 3 >= n then Error i
    else
      let b1 = byte (i + 1) and b2 = byte (i + 2) and b3 = byte (i + 3) in
      if b1 land 0xC0 <> 0x80 || b2 land 0xC0 <> 0x80 || b3 land 0xC0 <> 0x80 then Error i
      else
        let point =
          ((b0 land 0x07) lsl 18) lor ((b1 land 0x3F) lsl 12) lor ((b2 land 0x3F) lsl 6)
          lor (b3 land 0x3F)
        in
        if point < 0x10000 || point > 0x10FFFF then Error i else Ok (point, 4)
  end
  else Error i

let utf8_encode (buf : Buffer.t) (point : int) : unit =
  if point < 0x80 then Buffer.add_char buf (Char.chr point)
  else if point < 0x800 then begin
    Buffer.add_char buf (Char.chr (0xC0 lor (point lsr 6)));
    Buffer.add_char buf (Char.chr (0x80 lor (point land 0x3F)))
  end
  else if point < 0x10000 then begin
    Buffer.add_char buf (Char.chr (0xE0 lor (point lsr 12)));
    Buffer.add_char buf (Char.chr (0x80 lor ((point lsr 6) land 0x3F)));
    Buffer.add_char buf (Char.chr (0x80 lor (point land 0x3F)))
  end
  else begin
    Buffer.add_char buf (Char.chr (0xF0 lor (point lsr 18)));
    Buffer.add_char buf (Char.chr (0x80 lor ((point lsr 12) land 0x3F)));
    Buffer.add_char buf (Char.chr (0x80 lor ((point lsr 6) land 0x3F)));
    Buffer.add_char buf (Char.chr (0x80 lor (point land 0x3F)))
  end

(* The code points of `text`, refusing bytes that are not UTF-8: writing
   a replacement character would put a string on the wire the caller
   never held. *)
let utf8_points (text : string) (at : int) : int list =
  let rec walk i acc =
    if i >= String.length text then List.rev acc
    else
      match utf8_decode text i with
      | Ok (point, width) -> walk (i + width) (point :: acc)
      | Error _ -> fail_at at "a UTF-8 string"
  in
  walk 0 []

(* How many UTF-16 code units `text` is, which is what a wstring<N>
   bound counts. A scalar above the basic plane is two. *)
let utf16_units (text : string) : int =
  List.fold_left (fun n point -> n + if point > 0xFFFF then 2 else 1) 0 (utf8_points text 0)

(* `point` as the one octet an IDL char is, refused above U+00FF rather
   than truncated: the refusal is the sample's. *)
let checked_char (point : int) : int =
  if point > 0xFF then fail_at 0 "a character below U+0100" else point

(* `point` as the one UTF-16 code unit an IDL wchar is, refused outside
   the BMP. Section 7.2.2.2.1.2.5. *)
let checked_wchar (point : int) : int =
  if point > 0xFFFF then fail_at 0 "a BMP character" else point

(* An `unsigned long long`'s decimal text as its 64-bit pattern. The
   `0u` prefix is what lets Int64.of_string carry the top half of the
   range. *)
let u64_of_dec (text : string) : int64 =
  match Int64.of_string_opt ("0u" ^ text) with
  | Some v -> v
  | None -> fail_at 0 ("a decimal for an unsigned long long, not `" ^ text ^ "`")

let u64_to_dec (v : int64) : string = Printf.sprintf "%Lu" v

(* The ascending unsigned order over two decimal-held 64-bit keys — the
   `unsigned_less` seam of the shared walk. *)
let u64_less (a : string) (b : string) : bool =
  Int64.unsigned_compare (u64_of_dec a) (u64_of_dec b) < 0

(* -- the trims a bound reaches for ----------------------------------------- *)

(* `text` cut to at most `octets` bytes, never inside a character.
   Table 11: the order is preserved, so this is a prefix. *)
let trim_utf8_text (text : string) (octets : int) : string =
  let rec walk i =
    if i >= String.length text then i
    else
      match utf8_decode text i with
      | Ok (_, width) -> if i + width > octets then i else walk (i + width)
      | Error _ -> i
  in
  String.sub text 0 (walk 0)

(* `text` cut to at most `units` UTF-16 code units. *)
let trim_utf16_text (text : string) (units : int) : string =
  let rec walk i used =
    if i >= String.length text then i
    else
      match utf8_decode text i with
      | Ok (point, width) ->
        let cost = if point > 0xFFFF then 2 else 1 in
        if used + cost > units then i else walk (i + width) (used + cost)
      | Error _ -> i
  in
  String.sub text 0 (walk 0 0)

(* -- a small bignum, for binary128 ----------------------------------------- *)

(* Non-negative integers, base 2^24, least significant limb first. Only
   what the two binary128 conversions need: nothing here is reached by
   any other clause. *)
module Big = struct
  let base_bits = 24
  let base = 1 lsl base_bits

  type t = int array (* normalised: no trailing zero limb; [||] is zero *)

  let zero : t = [||]
  let is_zero (v : t) = Array.length v = 0

  let normalise (v : int array) : t =
    let n = ref (Array.length v) in
    while !n > 0 && v.(!n - 1) = 0 do decr n done;
    Array.sub v 0 !n

  let of_int (v : int) : t =
    let rec build v acc = if v = 0 then List.rev acc else build (v lsr base_bits) ((v land (base - 1)) :: acc) in
    Array.of_list (build v [])

  (* v * m + a, with small m and a. *)
  let mul_add (v : t) (m : int) (a : int) : t =
    let n = Array.length v in
    let out = Array.make (n + 3) 0 in
    let carry = ref a in
    for i = 0 to n - 1 do
      let product = (v.(i) * m) + !carry in
      out.(i) <- product land (base - 1);
      carry := product lsr base_bits
    done;
    let i = ref n in
    while !carry <> 0 do
      out.(!i) <- !carry land (base - 1);
      carry := !carry lsr base_bits;
      incr i
    done;
    normalise out

  (* v / d and v mod d, with small d. *)
  let divmod_small (v : t) (d : int) : t * int =
    let n = Array.length v in
    let out = Array.make n 0 in
    let rem = ref 0 in
    for i = n - 1 downto 0 do
      let cur = (!rem lsl base_bits) lor v.(i) in
      out.(i) <- cur / d;
      rem := cur mod d
    done;
    (normalise out, !rem)

  let of_decimal (digits : string) : t =
    let v = ref zero in
    String.iter (fun c -> v := mul_add !v 10 (Char.code c - Char.code '0')) digits;
    !v

  let to_decimal (v : t) : string =
    if is_zero v then "0"
    else begin
      let out = Buffer.create 32 in
      let rec walk v groups =
        if is_zero v then groups
        else
          let q, r = divmod_small v 1000000000 in
          walk q (r :: groups)
      in
      (match walk v [] with
       | [] -> Buffer.add_char out '0'
       | first :: rest ->
         Buffer.add_string out (string_of_int first);
         List.iter (fun g -> Buffer.add_string out (Printf.sprintf "%09d" g)) rest);
      Buffer.contents out
    end

  let shift_left (v : t) (bits : int) : t =
    let v = ref v in
    let left = ref bits in
    while !left >= base_bits do
      v := mul_add !v base 0;
      left := !left - base_bits
    done;
    if !left > 0 then v := mul_add !v (1 lsl !left) 0;
    !v

  (* v lsr bits, and whether anything nonzero was dropped. *)
  let shift_right_sticky (v : t) (bits : int) : t * bool =
    let v = ref v in
    let dropped = ref false in
    let left = ref bits in
    while !left >= base_bits && not (is_zero !v) do
      let q, r = divmod_small !v base in
      if r <> 0 then dropped := true;
      v := q;
      left := !left - base_bits
    done;
    if !left > 0 && not (is_zero !v) then begin
      let q, r = divmod_small !v (1 lsl !left) in
      if r <> 0 then dropped := true;
      v := q
    end;
    (!v, !dropped)

  let bit_length (v : t) : int =
    let n = Array.length v in
    if n = 0 then 0
    else begin
      let top = v.(n - 1) in
      let bits = ref 0 in
      let t = ref top in
      while !t > 0 do incr bits; t := !t lsr 1 done;
      ((n - 1) * base_bits) + !bits
    end

  let compare_big (a : t) (b : t) : int =
    let la = Array.length a and lb = Array.length b in
    if la <> lb then compare la lb
    else begin
      let rec walk i = if i < 0 then 0 else if a.(i) <> b.(i) then compare a.(i) b.(i) else walk (i - 1) in
      walk (la - 1)
    end

  let add (a : t) (b : t) : t =
    let n = max (Array.length a) (Array.length b) + 1 in
    let out = Array.make n 0 in
    let carry = ref 0 in
    for i = 0 to n - 1 do
      let ai = if i < Array.length a then a.(i) else 0 in
      let bi = if i < Array.length b then b.(i) else 0 in
      let s = ai + bi + !carry in
      out.(i) <- s land (base - 1);
      carry := s lsr base_bits
    done;
    normalise out

  let logor_bit0 (v : t) : t =
    if is_zero v then of_int 1
    else begin
      let out = Array.copy v in
      out.(0) <- out.(0) lor 1;
      out
    end

  let land_low (v : t) (bits : int) : t =
    (* v mod 2^bits *)
    let limbs = (bits + base_bits - 1) / base_bits in
    let n = min limbs (Array.length v) in
    let out = Array.sub v 0 n in
    let top_bits = bits - ((limbs - 1) * base_bits) in
    if n = limbs && top_bits < base_bits then out.(n - 1) <- out.(n - 1) land ((1 lsl top_bits) - 1);
    normalise out

  let power (b : int) (k : int) : t =
    let v = ref (of_int 1) in
    for _ = 1 to k do v := mul_add !v b 0 done;
    !v

  let mul_big (a : t) (b : t) : t =
    (* Schoolbook over the smaller operand's limbs. *)
    let out = ref zero in
    Array.iteri
      (fun i limb ->
        if limb <> 0 then out := add !out (shift_left (mul_add a limb 0) (i * base_bits)))
      b;
    !out
end

(* -- binary128 ------------------------------------------------------------- *)

let f128_fraction_bits = 112
let f128_exponent_bias = 16383
let f128_exponent_max = 0x7FFF

(* The sixteen bytes of a bit pattern held as a Big, least significant
   byte first. *)
let f128_bytes (bits : Big.t) : int array =
  let out = Array.make 16 0 in
  let v = ref bits in
  for i = 0 to 15 do
    let q, r = Big.divmod_small !v 256 in
    out.(i) <- r;
    v := q
  done;
  out

let f128_exponent_bits () : Big.t = Big.shift_left (Big.of_int f128_exponent_max) f128_fraction_bits
let f128_sign_mask () : Big.t = Big.shift_left (Big.of_int 1) 127
let f128_quiet_bit () : Big.t = Big.shift_left (Big.of_int 1) (f128_fraction_bits - 1)

(* Shift right, rounding to nearest with ties to even. *)
let f128_shift_round (v : Big.t) (shift : int) : Big.t =
  if shift <= 0 then v
  else begin
    let kept, dropped = Big.shift_right_sticky v shift in
    if not dropped then kept
    else begin
      (* The dropped part against the half point. *)
      let half = Big.shift_left (Big.of_int 1) (shift - 1) in
      let low = Big.land_low v shift in
      let cmp = Big.compare_big low half in
      let odd = (not (Big.is_zero kept)) && (Big.land_low kept 1 |> Big.is_zero |> not) in
      if cmp > 0 || (cmp = 0 && odd) then Big.add kept (Big.of_int 1) else kept
    end
  end

(* Build a pattern from a sign, an integral significand and a binary
   exponent, rounding to nearest with ties to even. *)
let f128_compose (negative : bool) (significand : Big.t) (exponent : int) : Big.t =
  let sign = if negative then f128_sign_mask () else Big.zero in
  if Big.is_zero significand then sign
  else begin
    let value = ref significand in
    let exponent = ref exponent in
    let leading = Big.bit_length !value - 1 in
    if leading > f128_fraction_bits then begin
      let drop = leading - f128_fraction_bits in
      value := f128_shift_round !value drop;
      exponent := !exponent + drop;
      if Big.bit_length !value - 1 > f128_fraction_bits then begin
        let v, _ = Big.shift_right_sticky !value 1 in
        value := v;
        exponent := !exponent + 1
      end
    end
    else begin
      let shift = f128_fraction_bits - leading in
      value := Big.shift_left !value shift;
      exponent := !exponent - shift
    end;
    let biased = !exponent + f128_exponent_bias + f128_fraction_bits in
    if biased >= f128_exponent_max then Big.add sign (f128_exponent_bits ())
    else if biased <= 0 then Big.add sign (f128_shift_round !value (1 - biased))
    else
      Big.add sign
        (Big.add
           (Big.shift_left (Big.of_int biased) f128_fraction_bits)
           (Big.land_low !value f128_fraction_bits))
  end

(* Round numerator/denominator to binary128, to nearest with ties to
   even: scale so the quotient carries more bits than binary128 needs,
   fold everything discarded into one sticky bit, round once. The
   denominator is always a power of ten here, so the division is a chain
   of small ones. *)
let f128_from_fraction (negative : bool) (numerator : Big.t) (pow10 : int) : Big.t =
  let wanted = 130 in
  let denominator = Big.power 10 pow10 in
  let shift = max 0 (wanted - (Big.bit_length numerator - Big.bit_length denominator)) in
  let scaled = Big.shift_left numerator shift in
  (* scaled / denominator with a sticky remainder: divide by 2 and 5
     factors piecewise — 10^k = 2^k * 5^k. *)
  let q2, r2 = Big.shift_right_sticky scaled pow10 in
  let quotient = ref q2 in
  let sticky = ref r2 in
  for _ = 1 to pow10 do
    let q, r = Big.divmod_small !quotient 5 in
    if r <> 0 then sticky := true;
    quotient := q
  done;
  let quotient = if !sticky then Big.logor_bit0 !quotient else !quotient in
  f128_compose negative quotient (-shift)

(* Parse decimal text and answer the sixteen bytes, least-significant
   first. A malformed decimal raises at `at`. *)
let f128_from_decimal (text : string) (at : int) : int array =
  let refuse () = fail_at at ("a decimal for a long double, not `" ^ text ^ "`") in
  let stripped = String.trim text in
  let lowered = String.lowercase_ascii stripped in
  match lowered with
  | "inf" | "+inf" | "infinity" | "+infinity" -> f128_bytes (f128_exponent_bits ())
  | "-inf" | "-infinity" -> f128_bytes (Big.add (f128_exponent_bits ()) (f128_sign_mask ()))
  | "nan" | "+nan" -> f128_bytes (Big.add (f128_exponent_bits ()) (f128_quiet_bit ()))
  | "-nan" ->
    f128_bytes (Big.add (Big.add (f128_exponent_bits ()) (f128_quiet_bit ())) (f128_sign_mask ()))
  | _ ->
    let negative, magnitude =
      if String.length stripped > 0 && (stripped.[0] = '-' || stripped.[0] = '+') then
        (stripped.[0] = '-', String.sub stripped 1 (String.length stripped - 1))
      else (false, stripped)
    in
    let mantissa, exponent10 =
      match String.index_opt magnitude 'e', String.index_opt magnitude 'E' with
      | Some i, _ | None, Some i ->
        let tail = String.sub magnitude (i + 1) (String.length magnitude - i - 1) in
        (match int_of_string_opt tail with
         | Some e -> (String.sub magnitude 0 i, e)
         | None -> refuse ())
      | None, None -> (magnitude, 0)
    in
    let whole, fraction =
      match String.index_opt mantissa '.' with
      | Some i ->
        (String.sub mantissa 0 i, String.sub mantissa (i + 1) (String.length mantissa - i - 1))
      | None -> (mantissa, "")
    in
    let digits = whole ^ fraction in
    if String.length digits = 0 then refuse ();
    String.iter (fun c -> if c < '0' || c > '9' then refuse ()) digits;
    let significand = Big.of_decimal digits in
    let exponent10 = exponent10 - String.length fraction in
    if Big.is_zero significand then
      f128_bytes (if negative then f128_sign_mask () else Big.zero)
    else if exponent10 >= 0 then
      f128_bytes
        (f128_compose negative (Big.mul_big significand (Big.power 10 exponent10)) 0)
    else f128_bytes (f128_from_fraction negative significand (-exponent10))

(* 0.<digits> * 10^point, in the three forms all the columns use. *)
let f128_render (negative : bool) (digits : string) (point : int) : string =
  let zeros n = String.make (max 0 n) '0' in
  let sign = if negative then "-" else "" in
  if 0 < point && point <= 34 then begin
    if String.length digits <= point then
      sign ^ digits ^ zeros (point - String.length digits) ^ ".0"
    else sign ^ String.sub digits 0 point ^ "." ^ String.sub digits point (String.length digits - point)
  end
  else if -6 < point && point <= 0 then sign ^ "0." ^ zeros (-point) ^ digits
  else begin
    let mantissa =
      String.sub digits 0 1 ^ "."
      ^ (if String.length digits > 1 then String.sub digits 1 (String.length digits - 1) else "0")
    in
    let exponent = point - 1 in
    if exponent >= 0 then sign ^ mantissa ^ "e+" ^ string_of_int exponent
    else sign ^ mantissa ^ "e-" ^ string_of_int (-exponent)
  end

(* Every digit the value has, in the corpus's canonical form — the exact
   expansion, in exactly the spelling the sibling renderers produce. *)
let f128_to_exact_decimal (bytes : int array) : string =
  let bits = ref Big.zero in
  for i = 15 downto 0 do
    bits := Big.mul_add !bits 256 bytes.(i)
  done;
  let sign_and_rest, negative =
    let shifted, _ = Big.shift_right_sticky !bits 127 in
    (!bits, not (Big.is_zero shifted))
  in
  let bits = if negative then Big.land_low sign_and_rest 127 else sign_and_rest in
  let biased =
    let top, _ = Big.shift_right_sticky bits f128_fraction_bits in
    if Big.is_zero top then 0
    else int_of_string (Big.to_decimal top)
  in
  let fraction = Big.land_low bits f128_fraction_bits in
  if biased = f128_exponent_max then begin
    if not (Big.is_zero fraction) then "nan" else if negative then "-inf" else "inf"
  end
  else begin
    let significand, exponent =
      if biased = 0 then (fraction, 1 - f128_exponent_bias - f128_fraction_bits)
      else
        ( Big.add fraction (Big.shift_left (Big.of_int 1) f128_fraction_bits),
          biased - f128_exponent_bias - f128_fraction_bits )
    in
    if Big.is_zero significand then (if negative then "-0.0" else "0.0")
    else begin
      let digits, point =
        if exponent >= 0 then begin
          let text = Big.to_decimal (Big.shift_left significand exponent) in
          (text, String.length text)
        end
        else begin
          (* m * 2^-k is exactly m * 5^k * 10^-k. *)
          let scale = -exponent in
          let text = Big.to_decimal (Big.mul_big significand (Big.power 5 scale)) in
          (text, String.length text - scale)
        end
      in
      let trimmed =
        let n = ref (String.length digits) in
        while !n > 0 && digits.[!n - 1] = '0' do decr n done;
        if !n = 0 then "0" else String.sub digits 0 !n
      in
      f128_render negative trimmed point
    end
  end

(* -- fixed BCD ------------------------------------------------------------- *)

(* The nibbles of `text` as fixed<digits, scale>, [CDR] section 9.3.2.8.
   `count` is "declared", "value-precision" or "value-string-digits": the
   integer digits of `text` as written plus S, made odd by one leading zero, which
   ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string writes, where OpenDDS writes the declared count
   (0.12345 under fixed<5,5> is 0012345c; past 31 digits it is refused). *)
let fixed_nibbles (text : string) (digits : int) (scale : int) (count : string) (at : int)
    : int list =
  if digits = 0 then fail_at at "a fixed with at least one digit";
  if scale > digits then fail_at at "a scale no larger than the digit count";
  if count <> "declared" && count <> "value-precision" && count <> "value-string-digits" then
    fail_at at ("a fixed digit count, not `" ^ count ^ "`");
  let refuse () =
    fail_at at
      (Printf.sprintf "a decimal that fits fixed<%d, %d>, not `%s`" digits scale text)
  in
  let negative, magnitude =
    if String.length text > 0 && (text.[0] = '-' || text.[0] = '+') then
      (text.[0] = '-', String.sub text 1 (String.length text - 1))
    else (false, text)
  in
  let whole, fraction =
    match String.index_opt magnitude '.' with
    | Some i ->
      (String.sub magnitude 0 i, String.sub magnitude (i + 1) (String.length magnitude - i - 1))
    | None -> (magnitude, "")
  in
  if String.length whole = 0 && String.length fraction = 0 then refuse ();
  String.iter (fun c -> if c < '0' || c > '9' then refuse ()) whole;
  String.iter (fun c -> if c < '0' || c > '9' then refuse ()) fraction;
  let as_written = String.length whole + scale in
  let integral_width = digits - scale in
  let whole =
    let n = ref 0 in
    while !n < String.length whole - 0 && !n < String.length whole && whole.[!n] = '0' do incr n done;
    String.sub whole !n (String.length whole - !n)
  in
  if String.length whole > integral_width then
    fail_at at
      (Printf.sprintf "a decimal whose integer part fits %d digits, not `%s`" integral_width text);
  if String.length fraction > scale then
    fail_at at (Printf.sprintf "a decimal whose fraction fits %d digits, not `%s`" scale text);
  let every = ref [] in
  for _ = 1 to integral_width - String.length whole do every := 0 :: !every done;
  String.iter (fun c -> every := (Char.code c - Char.code '0') :: !every) whole;
  String.iter (fun c -> every := (Char.code c - Char.code '0') :: !every) fraction;
  for _ = 1 to scale - String.length fraction do every := 0 :: !every done;
  let every = Array.of_list (List.rev !every) in
  let significant =
    let rec first i = if i >= Array.length every then 0 else if every.(i) <> 0 then Array.length every - i else first (i + 1) in
    first 0
  in
  let wire =
    if count = "value-precision" then max 1 significant
    else if count = "value-string-digits" then begin
      (* ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
         zero. Past 31 no reader can be told the field's width. *)
      let wire = as_written + (if as_written mod 2 = 0 then 1 else 0) in
      if wire > 31 then
        fail_at at
          (Printf.sprintf "a fixed of at most 31 digits on the wire, not %d for `%s`" wire text);
      wire
    end
    else Array.length every
  in
  let kept = min wire (Array.length every) in
  let nibbles = ref [] in
  let sign = if negative && significant <> 0 then 0x0D else 0x0C in
  nibbles := [ sign ];
  for i = Array.length every - 1 downto Array.length every - kept do
    nibbles := every.(i) :: !nibbles
  done;
  (* A count wider than P is filled with leading zeros. *)
  for _ = 1 to wire - kept do nibbles := 0 :: !nibbles done;
  if (wire + 1) mod 2 = 1 then nibbles := 0 :: !nibbles;
  !nibbles

(* -- the writer ------------------------------------------------------------ *)

(* Accumulates the bytes of one sample. Alignment is relative to the
   start of the stream: XCDR aligns each primitive to its own size
   counted from the encapsulation. XTypes 1.3 section 7.4.1.1. *)
type writer = {
  mutable w_data : Bytes.t;
  mutable w_len : int;
  w_endianness : endianness;
  w_max_align : int;
  mutable w_origin : int;
  w_framing : framing;
  (* Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
     bytes is rewritten with a short length code (Fast DDS's form). *)
  w_short_lc : bool;
}

let new_writer (endianness : endianness) (max_align : int) (fr : framing) (short_lc : bool)
    : writer =
  { w_data = Bytes.make 256 '\000'; w_len = 0; w_endianness = endianness;
    w_max_align = (if max_align < 1 then 1 else max_align); w_origin = 0; w_framing = fr;
    w_short_lc = short_lc }

let writer_position (w : writer) : int = w.w_len

let finish (w : writer) : Bytes.t = Bytes.sub w.w_data 0 w.w_len

let ensure (w : writer) (extra : int) : unit =
  let needed = w.w_len + extra in
  if needed > Bytes.length w.w_data then begin
    let grown = Bytes.make (max needed (2 * Bytes.length w.w_data)) '\000' in
    Bytes.blit w.w_data 0 grown 0 w.w_len;
    w.w_data <- grown
  end

let put_byte (w : writer) (v : int) : unit =
  ensure w 1;
  Bytes.set w.w_data w.w_len (Char.chr (v land 0xFF));
  w.w_len <- w.w_len + 1

let align_to (w : writer) (alignment : int) : unit =
  let alignment = if alignment > w.w_max_align then w.w_max_align else alignment in
  if alignment > 1 then begin
    let offset = w.w_len - w.w_origin in
    let padding = (alignment - (offset mod alignment)) mod alignment in
    for _ = 1 to padding do put_byte w 0 done
  end

(* The low `width` bytes of `unsigned` (an int64 pattern), in stream
   order, unaligned. *)
let write_raw (w : writer) (unsigned : int64) (width : int) : unit =
  match w.w_endianness with
  | Little ->
    for i = 0 to width - 1 do
      put_byte w (Int64.to_int (Int64.logand (Int64.shift_right_logical unsigned (8 * i)) 0xFFL))
    done
  | Big ->
    for i = width - 1 downto 0 do
      put_byte w (Int64.to_int (Int64.logand (Int64.shift_right_logical unsigned (8 * i)) 0xFFL))
    done

let write_int_at_width (w : writer) (unsigned : int64) (width : int) : unit =
  align_to w width;
  write_raw w unsigned width

(* -- primitives *)

let write_bool (w : writer) (v : bool) : unit = put_byte w (if v then 1 else 0)

let check_range (w : writer) (v : int) (lo : int) (hi : int) (what : string) : unit =
  if v < lo || v > hi then fail_at w.w_len what

let write_u8 (w : writer) (v : int) : unit =
  check_range w v 0 255 "an octet in 0..255";
  put_byte w v

let write_i8 (w : writer) (v : int) : unit =
  check_range w v (-128) 127 "an int8 in -128..127";
  put_byte w (v land 0xFF)

(* `char`: one octet — which is the whole member. *)
let write_char (w : writer) (v : int) : unit =
  check_range w v 0 255 "a char in one octet";
  put_byte w v

let write_i16 (w : writer) (v : int) : unit =
  check_range w v (-32768) 32767 "a short in 16 bits";
  write_int_at_width w (Int64.of_int (v land 0xFFFF)) 2

let write_u16 (w : writer) (v : int) : unit =
  check_range w v 0 65535 "an unsigned short in 16 bits";
  write_int_at_width w (Int64.of_int v) 2

let write_i32 (w : writer) (v : int32) : unit =
  write_int_at_width w (Int64.logand (Int64.of_int32 v) 0xFFFF_FFFFL) 4

(* `unsigned long`, held as an OCaml int (the adapter widens it into
   int64 in the member; the codec narrows at the call). *)
let write_u32 (w : writer) (v : int) : unit =
  if v < 0 || v > 0xFFFF_FFFF then fail_at w.w_len "an unsigned long in 32 bits";
  write_int_at_width w (Int64.of_int v) 4

let write_i64 (w : writer) (v : int64) : unit = write_int_at_width w v 8

let write_u64 (w : writer) (v : string) : unit = write_int_at_width w (u64_of_dec v) 8

let write_f32 (w : writer) (v : float) : unit =
  write_int_at_width w (Int64.logand (Int64.of_int32 (Int32.bits_of_float v)) 0xFFFF_FFFFL) 4

let write_f64 (w : writer) (v : float) : unit = write_int_at_width w (Int64.bits_of_float v) 8

(* `long double`: sixteen bytes of IEEE 754 binary128, aligned to eight
   (capped by max-align). The value arrives as decimal text and is
   correctly rounded here, once. The two byte orders are the whole
   sixteen bytes reversed — CORBA 3.3 part 2 section 9.3.1.3. *)
let write_f128 (w : writer) (v : string) : unit =
  let bytes = f128_from_decimal v w.w_len in
  align_to w 8;
  (match w.w_endianness with
   | Little -> for i = 0 to 15 do put_byte w bytes.(i) done
   | Big -> for i = 15 downto 0 do put_byte w bytes.(i) done)

(* -- characters and strings *)

(* A 16-bit value in a fixed order whatever the stream is. *)
let write_u16_little (w : writer) (v : int) : unit =
  align_to w 2;
  put_byte w (v land 0xFF);
  put_byte w ((v lsr 8) land 0xFF)

let write_u16_big (w : writer) (v : int) : unit =
  align_to w 2;
  put_byte w ((v lsr 8) land 0xFF);
  put_byte w (v land 0xFF)

let write_u32_little (w : writer) (v : int) : unit =
  align_to w 4;
  for i = 0 to 3 do put_byte w ((v lsr (8 * i)) land 0xFF) done

let write_u32_big (w : writer) (v : int) : unit =
  align_to w 4;
  for i = 3 downto 0 do put_byte w ((v lsr (8 * i)) land 0xFF) done

(* `wchar`, at `width` octets and in `framing`'s shape.
   "giop-length-prefixed" is CORBA 3.3 part 2 section 9.3.1.6's GIOP 1.2
   form: the octet count first, then the code unit's octets, all aligned
   to one; every order but "stream" means big-endian there. *)
let write_wchar (w : writer) (v : int) (width : int) (framing : string) (order : string) : unit =
  if width <> 2 && width <> 4 then fail_at w.w_len "a wchar width of 2 or 4";
  if framing = "giop-length-prefixed" then begin
    put_byte w width;
    let big = order <> "stream" || w.w_endianness <> Little in
    if big then for i = width - 1 downto 0 do put_byte w ((v lsr (8 * i)) land 0xFF) done
    else for i = 0 to width - 1 do put_byte w ((v lsr (8 * i)) land 0xFF) done
  end
  else if framing <> "fixed" then fail_at w.w_len ("a wchar framing, not `" ^ framing ^ "`")
  else if width = 4 then write_u32 w v
  else write_u16 w v

(* A string: a four-byte length including the terminator, the UTF-8
   bytes, then a NUL. The length counts bytes. XTypes 1.3 section 7.4.1.1. *)
let write_string (w : writer) (v : string) : unit =
  write_u32 w (String.length v + 1);
  ensure w (String.length v);
  String.iter (fun c -> put_byte w (Char.code c)) v;
  put_byte w 0

(* A bounded string: the same bytes [write_string] writes. The bound is a
   promise about the source and not a difference on the wire, so this is
   the only place it can be enforced — refused rather than cut at a
   character boundary, which would put a sample on the wire nobody wrote. *)
let write_string_bounded (w : writer) (v : string) (bound : int) : unit =
  let octets = String.length v in
  if octets > bound then
    fail_at w.w_len
      (Printf.sprintf "at most %d octets in a bounded string, and this one has %d" bound octets);
  write_string w v

(* The count check every bounded sequence makes before a byte of it is
   written. Table 17 makes O2.length <= T1.length the condition for
   constructing a sequence<T, N>, and the wire carries no bound to
   compare against: a sender whose count already exceeds its own
   declaration has written a value of a type nobody declared. *)
let check_sequence_bound (w : writer) (actual : int) (bound : int) : unit =
  if actual > bound then
    fail_at w.w_len
      (Printf.sprintf "at most %d elements in a bounded sequence, and this one has %d" bound
         actual)

(* The count check every bounded map makes before a byte of it is
   written. Its own message and not [check_sequence_bound]'s: the unit is
   an entry and not an element, per section 7.4.1.1.4's
   map-as-a-sequence-of-pairs equivalence. *)
let check_map_bound (w : writer) (actual : int) (bound : int) : unit =
  if actual > bound then
    fail_at w.w_len
      (Printf.sprintf "at most %d entries in a bounded map, and this one has %d" bound actual)

let write_wstring_unit (w : writer) (unit_ : int) (width : int) (order : string) : unit =
  let little = order = "byte-order-mark" || order = "host-unswapped" in
  if width = 4 then begin
    if order = "big-endian-always" then write_u32_big w unit_
    else if little then write_u32_little w unit_
    else write_u32 w unit_
  end
  else if order = "big-endian-always" then write_u16_big w unit_
  else if little then write_u16_little w unit_
  else write_u16 w unit_

(* A wstring, in the form `policy` and `order` name. The value arrives as
   UTF-8 and the conversion to UTF-16 units happens here, which is what
   makes section 7.4.1.1.2's surrogate rule this runtime's own reading. *)
let write_wstring_utf8 (w : writer) (v : string) (policy : string) (order : string) : unit =
  let counts_characters, terminated, width =
    match wstring_cell policy with
    | Some cell -> cell
    | None -> fail_at w.w_len ("a wstring length policy, not `" ^ policy ^ "`")
  in
  if not (wstring_order_known order) then
    fail_at w.w_len ("a wstring unit order, not `" ^ order ^ "`");
  let units = ref [] in
  List.iter
    (fun point ->
      if point < 0x10000 then units := point :: !units
      else begin
        let offset = point - 0x10000 in
        units := (0xDC00 lor (offset land 0x3FF)) :: (0xD800 lor (offset lsr 10)) :: !units
      end)
    (utf8_points v w.w_len);
  let units = List.rev !units in
  if counts_characters && width = 2 then
    List.iteri
      (fun index unit_ ->
        if unit_ >= 0xD800 && unit_ <= 0xDFFF then
          fail_at index
            "a code point inside the BMP: a character count over two-octet units has no way to \
             carry one outside it")
      units;
  let units = if terminated then units @ [ 0 ] else units in
  (* An empty wstring carries no mark. *)
  let units =
    if order = "byte-order-mark" && List.length units > 0 then byte_order_mark :: units else units
  in
  if counts_characters then write_u32 w (List.length units)
  else write_u32 w (List.length units * width);
  List.iter (fun unit_ -> write_wstring_unit w unit_ width order) units

(* A bounded wstring, as [write_string_bounded] is to [write_string]. The
   bound counts UTF-16 code units, not characters and not octets,
   whatever `policy` puts on the wire. *)
let write_wstring_bounded (w : writer) (v : string) (bound : int) (policy : string)
    (order : string) : unit =
  let units = utf16_units v in
  if units > bound then
    fail_at w.w_len
      (Printf.sprintf "at most %d UTF-16 code units in a bounded wstring, and this one has %d"
         bound units);
  write_wstring_utf8 w v policy order

(* A wstring whose characters go out as their own UTF-8 octets.
   WstringCodeUnits::Utf8 is the measured departure of one product's
   generated serialiser: XTypes 1.3 Table 31 gives a wstring's code unit
   as Char16 and section 7.4.1.1.2 makes the length "twice the number of
   characters", so the clause has no UTF-8 form. The declared length is
   the UTF-8 octet count and only C2 -- the terminator -- still reaches
   the wire. wstring-member is 04 00 00 00 61 62 63 00 for "abc". *)
let write_wstring_utf8_octets (w : writer) (v : string) (policy : string) : unit =
  let _, terminated, _ =
    match wstring_cell policy with
    | Some cell -> cell
    | None -> fail_at w.w_len ("a wstring length policy, not `" ^ policy ^ "`")
  in
  (* The same malformed-UTF-8 refusal the UTF-16 writer makes. A string is
     not a `wstring` type, so this direction checks what the type checked
     for it elsewhere. *)
  ignore (utf8_points v w.w_len);
  let length = String.length v + (if terminated then 1 else 0) in
  if length > 0xFFFF_FFFF then fail_at w.w_len "a wstring length the UInt32 field can hold";
  write_u32 w length;
  String.iter (fun c -> put_byte w (Char.code c)) v;
  if terminated then put_byte w 0

(* A bounded UTF-8-octet wstring, as [write_wstring_bounded] is to
   [write_wstring_utf8]: the bound still counts UTF-16 code units, which is
   what every wstring<N> bound means, while the wire carries the UTF-8
   octets. *)
let write_wstring_utf8_octets_bounded (w : writer) (v : string) (bound : int) (policy : string)
    : unit =
  let units = utf16_units v in
  if units > bound then
    fail_at w.w_len
      (Printf.sprintf "at most %d UTF-16 code units in a bounded wstring, and this one has %d"
         bound units);
  write_wstring_utf8_octets w v policy

(* -- fixed *)

(* `fixed<digits, scale>` as packed BCD, [CDR] section 9.3.2.8 — the only
   wire definition an IDL fixed has. *)
let write_fixed (w : writer) (v : string) (digits : int) (scale : int) (count : string) : unit =
  let nibbles = Array.of_list (fixed_nibbles v digits scale count w.w_len) in
  let i = ref 0 in
  while !i < Array.length nibbles do
    put_byte w ((nibbles.(!i) lsl 4) lor nibbles.(!i + 1));
    i := !i + 2
  done

(* -- the bit-bounded kinds *)

(* An enumerator, as the integer its bit bound calls for. The holder is
   signed — section 7.4.3.5.3 rule (5) — and what fits is everything
   expressible in the bound's bits: -2^(n-1) ..= 2^n - 1. *)
let write_enum (w : writer) (bit_bound : int) (value : int64) : unit =
  if bit_bound = 0 || bit_bound > 32 then
    fail_at w.w_len (Printf.sprintf "an @bit_bound between 1 and 32, not %d" bit_bound);
  let low = Int64.neg (Int64.shift_left 1L (bit_bound - 1)) in
  let high = Int64.sub (Int64.shift_left 1L bit_bound) 1L in
  if value < low || value > high then
    fail_at w.w_len (Printf.sprintf "an enumerator a %d-bit holder can hold" bit_bound);
  let pattern = Int64.logand value (Int64.sub (Int64.shift_left 1L bit_bound) 1L) in
  if bit_bound <= 8 then write_u8 w (Int64.to_int pattern)
  else if bit_bound <= 16 then write_u16 w (Int64.to_int pattern)
  else write_u32 w (Int64.to_int pattern)

(* A bitmask, as the unsigned integer Table 33 gives its bound. The
   value arrives as its 64-bit pattern in an int64. *)
let write_bitmask (w : writer) (bit_bound : int) (value : int64) : unit =
  if bit_bound = 0 || bit_bound > 64 then
    fail_at w.w_len (Printf.sprintf "an @bit_bound between 1 and 64, not %d" bit_bound);
  if bit_bound < 64
     && Int64.unsigned_compare value (Int64.shift_left 1L bit_bound) >= 0 then
    fail_at w.w_len (Printf.sprintf "a bitmask below 2^%d" bit_bound);
  if bit_bound <= 8 then write_u8 w (Int64.to_int value)
  else if bit_bound <= 16 then write_u16 w (Int64.to_int value)
  else if bit_bound <= 32 then write_u32 w (Int64.to_int value)
  else write_int_at_width w value 8

(* One bitfield, shifted into the accumulator a bitset packs into. A
   value wider than the field is refused rather than masked. The writer
   is the first parameter because the shared walk spells this as an
   operation on the writer. *)
let bitfield (w : writer) (value : int64) (width : int) (position : int) : int64 =
  if width <= 0 || width > 64 || position < 0 || position + width > 64 then
    fail_at w.w_len (Printf.sprintf "a bitfield of %d bits at %d, inside 64" width position);
  if width < 64
     && Int64.unsigned_compare value (Int64.sub (Int64.shift_left 1L width) 1L) > 0 then
    fail_at w.w_len (Printf.sprintf "a value inside %d bits" width);
  Int64.shift_left value position

(* The Cyclone union overlay: the low `octets` of `pattern`, written as
   one unsigned integer of that width in the stream's byte order. *)
let write_union_overlay (w : writer) (pattern : int64) (octets : int) : unit =
  match octets with
  | 1 -> write_u8 w (Int64.to_int (Int64.logand pattern 0xFFL))
  | 2 -> write_u16 w (Int64.to_int (Int64.logand pattern 0xFFFFL))
  | 4 -> write_u32 w (Int64.to_int (Int64.logand pattern 0xFFFF_FFFFL))
  | 8 -> write_int_at_width w pattern 8
  | other -> fail_at w.w_len (Printf.sprintf "a union arm of 1, 2, 4 or 8 octets, not %d" other)

(* A bitset, as the anchor integer its bitfields pack into; the anchor's
   width comes from the layout planner. *)
let write_bitset (w : writer) (anchor : int) (value : int64) : unit =
  match anchor with
  | 1 -> write_u8 w (Int64.to_int (Int64.logand value 0xFFL))
  | 2 -> write_u16 w (Int64.to_int (Int64.logand value 0xFFFFL))
  | 4 -> write_u32 w (Int64.to_int (Int64.logand value 0xFFFF_FFFFL))
  | 8 -> write_int_at_width w value 8
  | other -> fail_at w.w_len (Printf.sprintf "a bitset anchor of 1, 2, 4 or 8 bytes, not %d" other)

(* -- the overlay patterns -------------------------------------------------- *)

(* The low `octets` of `value`'s two's-complement bits, zero-extended.
   The mask is the function: a plain widening of a negative value
   sign-extends, and -2 in four octets has to be 0x00000000FFFFFFFE. *)
let bit_pattern_int (value : int64) (octets : int) : int64 =
  if octets >= 8 then value
  else Int64.logand value (Int64.sub (Int64.shift_left 1L (octets * 8)) 1L)

let bit_pattern_bool (value : bool) : int64 = if value then 1L else 0L
let bit_pattern_f32 (value : float) : int64 =
  Int64.logand (Int64.of_int32 (Int32.bits_of_float value)) 0xFFFF_FFFFL
let bit_pattern_f64 (value : float) : int64 = Int64.bits_of_float value

(* -- framing (writing) ----------------------------------------------------- *)

let patch (w : writer) (at : int) (value : int) (width : int) : unit =
  if value < 0 || (width < 8 && value >= 1 lsl (width * 8)) then
    fail_at at (Printf.sprintf "a %d-byte length" width);
  (match w.w_endianness with
   | Little ->
     for i = 0 to width - 1 do
       Bytes.set w.w_data (at + i) (Char.chr ((value lsr (8 * i)) land 0xFF))
     done
   | Big ->
     for i = 0 to width - 1 do
       Bytes.set w.w_data (at + width - 1 - i) (Char.chr ((value lsr (8 * i)) land 0xFF))
     done)

let read_patched (w : writer) (at : int) : int =
  let byte i = Char.code (Bytes.get w.w_data (at + i)) in
  match w.w_endianness with
  | Little -> byte 0 lor (byte 1 lsl 8) lor (byte 2 lsl 16) lor (byte 3 lsl 24)
  | Big -> byte 3 lor (byte 2 lsl 8) lor (byte 1 lsl 16) lor (byte 0 lsl 24)

(* A DHEADER: a four-byte length placeholder, patched when the members
   are done. *)
let write_dheader (w : writer) : dheader =
  align_to w 4;
  let at = w.w_len in
  for _ = 1 to 4 do put_byte w 0 done;
  { dh_at = at }

let finish_dheader (w : writer) (header : dheader) : unit =
  if header.dh_at >= 0 then patch w header.dh_at (w.w_len - header.dh_at - 4) 4

(* An XCDR2 EMHEADER: id, length code and must-understand in one 32-bit
   word. Section 7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7 reuse the
   member's own leading word. *)
let write_emheader (w : writer) (member_id : int) (length_code : int) (must_understand : bool)
    : emheader =
  align_to w 4;
  let word =
    ((length_code land 0x7) lsl 28) lor (member_id land e_member_id_mask)
    lor (if must_understand then 0x8000_0000 else 0)
  in
  align_to w 4;
  let word_at = w.w_len in
  write_raw w (Int64.of_int word) 4;
  if length_code = 4 then begin
    let at = w.w_len in
    for _ = 1 to 4 do put_byte w 0 done;
    { em_next = at; em_word = word_at }
  end
  else { em_next = -1; em_word = word_at }

let shorten_emheader (w : writer) (header : emheader) (length : int) : unit =
  let code = match length with 1 -> 0 | 2 -> 1 | 4 -> 2 | _ -> 3 in
  (* Delete the NEXTINT's four bytes. *)
  Bytes.blit w.w_data (header.em_next + 4) w.w_data header.em_next
    (w.w_len - header.em_next - 4);
  w.w_len <- w.w_len - 4;
  let previous = read_patched w header.em_word in
  patch w header.em_word ((previous land lnot (0x7 lsl 28)) lor (code lsl 28)) 4

(* Patch an EMHEADER's NEXTINT; under short length codes an LC4 header
   whose member is 1, 2, 4 or 8 bytes loses its NEXTINT and states the
   length in the code. *)
let finish_emheader (w : writer) (header : emheader) : unit =
  if header.em_next >= 0 then begin
    let length = w.w_len - header.em_next - 4 in
    if w.w_short_lc && (length = 1 || length = 2 || length = 4 || length = 8) then
      shorten_emheader w header length
    else patch w header.em_next length 4
  end

let begin_parameter_value (w : writer) : int =
  let saved = w.w_origin in
  w.w_origin <- w.w_len;
  saved

(* -- packed collection elements *)

(* The same origin move [begin_parameter_value] makes, under the name the
   shared walk calls for ElementPadding::Packed. Kept apart because the
   two callers mean different things: a PL_CDR1 parameter value is its own
   block by section 7.4.1.2, a collection element under a packed key is
   its own block by HDDS's measured departure. array-1d-struct-final is
   01000000 0200 03000000 0400 77, the second Cell's long at offset six. *)
let begin_packed_element (w : writer) : int =
  let saved = w.w_origin in
  w.w_origin <- w.w_len;
  saved

let finish_packed_element (w : writer) (saved : int) : unit = w.w_origin <- saved

(* A PL_CDR1 parameter header, short form: [id:u16][length:u16]. *)
let write_pid (w : writer) (pid : int) (r : role) (must_understand : bool) : pid =
  align_to w 4;
  let flagged =
    (if must_understand && w.w_framing.member_must_understand then pid lor pid_must_understand_bit
     else pid)
    lor pid_flags_at w.w_framing.pid_flags "member"
  in
  let bare = flagged land lnot (pid_must_understand_bit lor pid_impl_extension_bit) in
  write_u16 w (flagged land 0xFFFF);
  let at = w.w_len in
  for _ = 1 to 2 do put_byte w 0 done;
  { p_at = at; p_extended = false; p_saved_origin = begin_parameter_value w; p_role = r;
    p_bare = bare; p_mu = must_understand }

(* A PL_CDR1 parameter header, extended form:
   [PID_EXTENDED][8][id:u32][length:u32]. Section 7.4.1.2. The member's
   must-understand is FLAG_2 of the eMemberHeader. *)
let write_pid_extended (w : writer) (pid : int) (r : role) (must_understand : bool) : pid =
  align_to w 4;
  let marker = pid_extended_id lor pid_flags_at w.w_framing.pid_flags "extended-marker" in
  write_u16 w (marker land 0xFFFF);
  write_u16 w 8;
  let member_header =
    (pid land e_member_id_mask)
    lor (if must_understand && w.w_framing.member_must_understand then e_flag_must_understand
         else 0)
  in
  write_raw w (Int64.of_int member_header) 4;
  let at = w.w_len in
  for _ = 1 to 4 do put_byte w 0 done;
  { p_at = at; p_extended = true; p_saved_origin = begin_parameter_value w; p_role = r;
    p_bare = 0; p_mu = false }

let promote_pid (w : writer) (header : pid) (length : int) : unit =
  let start = header.p_at - 2 in
  (* Insert eight bytes after the short header's four. *)
  ensure w 8;
  Bytes.blit w.w_data (header.p_at + 2) w.w_data (header.p_at + 10)
    (w.w_len - header.p_at - 2);
  Bytes.fill w.w_data (header.p_at + 2) 8 '\000';
  w.w_len <- w.w_len + 8;
  let marker = pid_extended_id lor pid_flags_at w.w_framing.pid_flags "extended-marker" in
  patch w start marker 2;
  patch w (start + 2) 8 2;
  let member_header =
    (header.p_bare land e_member_id_mask)
    lor (if header.p_mu && w.w_framing.member_must_understand then e_flag_must_understand else 0)
  in
  patch w (start + 4) member_header 4;
  patch w (start + 8) length 4

(* Patch a parameter header with its value's length; the padding to four
   goes outside the declared length or inside it, by framing. *)
let finish_pid (w : writer) (header : pid) : unit =
  let width = if header.p_extended then 4 else 2 in
  let length = ref (w.w_len - header.p_at - width) in
  if pads_declared_length w.w_framing header.p_role then begin
    let past = (w.w_len - w.w_origin) mod 4 in
    if past <> 0 then length := !length + (4 - past)
  end;
  if (not header.p_extended) && !length > pid_short_max_len then promote_pid w header !length
  else patch w header.p_at !length width;
  w.w_origin <- header.p_saved_origin;
  (* Under Fast DDS's framing the parameter writes no padding of its own:
     the next header aligns itself, and a final parameter ends the
     stream where its value does. *)
  if w.w_framing.pad_after_value then align_to w 4

(* The PID_LIST_END sentinel a PL_CDR1 parameter list ends with; its
   flags come from the framing asked at the "sentinel" position. *)
let write_sentinel (w : writer) : unit =
  align_to w 4;
  let sentinel = pid_list_end lor pid_flags_at w.w_framing.pid_flags "sentinel" in
  write_u16 w (sentinel land 0xFFFF);
  write_u16 w 0

(* -- the reader ------------------------------------------------------------ *)

(* Reads one sample's bytes: the mirror of the writer clause for clause.
   A malformed stream raises; a refusal about the type sets the status
   and the generated decoder walks on. *)
type reader = {
  r_data : string;
  mutable r_at : int;
  r_endianness : endianness;
  r_max_align : int;
  mutable r_origin : int;
  r_framing : framing;
  mutable r_status : status;
}

let new_reader (data : string) (endianness : endianness) (max_align : int) (fr : framing)
    : reader =
  { r_data = data; r_at = 0; r_endianness = endianness;
    r_max_align = (if max_align < 1 then 1 else max_align); r_origin = 0; r_framing = fr;
    r_status = St_ok }

let reader_status (r : reader) : status = r.r_status

(* Refuse the stream from outside the runtime; the first refusal sticks,
   and St_ok refuses nothing. *)
let reader_refuse (r : reader) (refusal : status) : unit =
  if refusal <> St_ok && r.r_status = St_ok then r.r_status <- refusal

(* Take back a St_discarded, and only that one: a truncated stream is
   not a narrowing. *)
let reader_clear_discard (r : reader) : bool =
  if r.r_status = St_discarded then begin
    r.r_status <- St_ok;
    true
  end
  else false

let reader_position (r : reader) : int = r.r_at

let reader_remaining (r : reader) : int =
  if r.r_at > String.length r.r_data then 0 else String.length r.r_data - r.r_at

(* Move the cursor forward to `target`, never backwards: a framing word
   pointing behind the cursor is a refusal about the stream's shape, and
   past the end raises. *)
let reader_skip_to (r : reader) (target : int) : unit =
  if r.r_status = St_ok then begin
    if target < r.r_at then reader_refuse r St_out_of_range
    else if target > String.length r.r_data then
      fail_at r.r_at
        (Printf.sprintf "%d bytes needed, %d available" (target - r.r_at) (reader_remaining r))
    else r.r_at <- target
  end

let reader_save (r : reader) : reader_mark =
  { mk_at = r.r_at; mk_origin = r.r_origin; mk_status = r.r_status }

let reader_restore (r : reader) (mark : reader_mark) : unit =
  r.r_at <- mark.mk_at;
  r.r_origin <- mark.mk_origin;
  r.r_status <- mark.mk_status

(* The read-side pair of [begin_packed_element]. *)
let reader_begin_packed_element (r : reader) : int =
  let saved = r.r_origin in
  r.r_origin <- r.r_at;
  saved

let reader_finish_packed_element (r : reader) (saved : int) : unit = r.r_origin <- saved

let reader_align_to (r : reader) (alignment : int) : unit =
  let alignment = if alignment > r.r_max_align then r.r_max_align else alignment in
  if alignment > 1 then begin
    let offset = r.r_at - r.r_origin in
    r.r_at <- r.r_at + ((alignment - (offset mod alignment)) mod alignment)
  end

let take (r : reader) (count : int) : int =
  let start = r.r_at in
  if start + count > String.length r.r_data then
    fail_at r.r_at (Printf.sprintf "%d bytes needed, %d available" count (reader_remaining r));
  r.r_at <- start + count;
  start

let byte_at (r : reader) (i : int) : int = Char.code r.r_data.[i]

(* The next `width` bytes as an unsigned pattern in an int64. *)
let read_unsigned (r : reader) (width : int) : int64 =
  reader_align_to r width;
  let start = take r width in
  let value = ref 0L in
  (match r.r_endianness with
   | Little ->
     for i = width - 1 downto 0 do
       value := Int64.logor (Int64.shift_left !value 8) (Int64.of_int (byte_at r (start + i)))
     done
   | Big ->
     for i = 0 to width - 1 do
       value := Int64.logor (Int64.shift_left !value 8) (Int64.of_int (byte_at r (start + i)))
     done);
  !value

(* -- primitives *)

(* `boolean`; a byte that is neither 0 nor 1 is refused rather than
   coerced. *)
let read_bool (r : reader) : bool =
  let position = r.r_at in
  match byte_at r (take r 1) with
  | 0 -> false
  | 1 -> true
  | _ -> fail_at position "a boolean of 0 or 1"

let read_u8 (r : reader) : int = byte_at r (take r 1)

let read_i8 (r : reader) : int =
  let v = byte_at r (take r 1) in
  if v >= 128 then v - 256 else v

let read_char (r : reader) : int = byte_at r (take r 1)

let read_i16 (r : reader) : int =
  let v = Int64.to_int (read_unsigned r 2) in
  if v >= 32768 then v - 65536 else v

let read_u16 (r : reader) : int = Int64.to_int (read_unsigned r 2)

let read_i32 (r : reader) : int32 = Int64.to_int32 (read_unsigned r 4)

let read_u32 (r : reader) : int = Int64.to_int (read_unsigned r 4)

let read_i64 (r : reader) : int64 = read_unsigned r 8

let read_u64 (r : reader) : string = u64_to_dec (read_unsigned r 8)

let read_f32 (r : reader) : float = Int32.float_of_bits (Int64.to_int32 (read_unsigned r 4))

let read_f64 (r : reader) : float = Int64.float_of_bits (read_unsigned r 8)

(* `long double`: all sixteen bytes, as the exact decimal the generated
   member holds. *)
let read_f128 (r : reader) : string =
  reader_align_to r 8;
  let start = take r 16 in
  let bytes = Array.make 16 0 in
  (match r.r_endianness with
   | Little -> for i = 0 to 15 do bytes.(i) <- byte_at r (start + i) done
   | Big -> for i = 0 to 15 do bytes.(i) <- byte_at r (start + 15 - i) done);
  f128_to_exact_decimal bytes

(* `wchar`, at `width` octets and in `framing`'s shape. The four-byte
   form zero-extends one UTF-16 code unit; a value that uses the high
   bits, and a lone surrogate, are refused. *)
let read_wchar (r : reader) (width : int) (framing : string) (order : string) : int =
  let position = r.r_at in
  if width <> 2 && width <> 4 then fail_at position "a wchar width of 2 or 4";
  let value =
    if framing = "giop-length-prefixed" then begin
      let declared = read_u8 r in
      if declared <> width then
        fail_at position
          (Printf.sprintf
             "a GIOP 1.2 wchar of %d octets, the configured transmission code set, and not %d"
             width declared);
      let big = order <> "stream" || r.r_endianness <> Little in
      let start = take r width in
      let v = ref 0 in
      if big then for i = 0 to width - 1 do v := (!v lsl 8) lor byte_at r (start + i) done
      else for i = width - 1 downto 0 do v := (!v lsl 8) lor byte_at r (start + i) done;
      !v
    end
    else if framing <> "fixed" then fail_at position ("a wchar framing, not `" ^ framing ^ "`")
    else if width = 4 then begin
      let v = read_u32 r in
      if v > 0xFFFF then fail_at position "a four-byte wchar holding one UTF-16 code unit";
      v
    end
    else read_u16 r
  in
  if value >= 0xD800 && value <= 0xDFFF then fail_at position "a Unicode code point";
  value

(* -- strings *)

(* A string: a length including the terminator, the bytes, the NUL. *)
let read_string_alloc (r : reader) : string =
  let position = r.r_at in
  let length = read_u32 r in
  if length = 0 || length > reader_remaining r + 1 then
    fail_at position (Printf.sprintf "a length of %d with %d bytes left" length (reader_remaining r));
  let start = take r (length - 1) in
  let candidate = String.sub r.r_data start (length - 1) in
  (* Validate UTF-8. *)
  let rec walk i =
    if i < String.length candidate then
      match utf8_decode candidate i with
      | Ok (_, width) -> walk (i + width)
      | Error _ -> fail_at position "the string is not UTF-8"
  in
  walk 0;
  ignore (read_u8 r);
  candidate

let read_u16_little (r : reader) : int =
  reader_align_to r 2;
  let start = take r 2 in
  byte_at r start lor (byte_at r (start + 1) lsl 8)

let read_u16_big (r : reader) : int =
  reader_align_to r 2;
  let start = take r 2 in
  (byte_at r start lsl 8) lor byte_at r (start + 1)

let read_u32_little (r : reader) : int =
  reader_align_to r 4;
  let start = take r 4 in
  byte_at r start lor (byte_at r (start + 1) lsl 8) lor (byte_at r (start + 2) lsl 16)
  lor (byte_at r (start + 3) lsl 24)

let read_u32_big (r : reader) : int =
  reader_align_to r 4;
  let start = take r 4 in
  (byte_at r start lsl 24) lor (byte_at r (start + 1) lsl 16) lor (byte_at r (start + 2) lsl 8)
  lor byte_at r (start + 3)

let strip_byte_order_mark (units : int list) : int list =
  match units with
  | [] -> []
  | first :: rest when first = byte_order_mark -> rest
  | _ ->
    let rest = match units with first :: rest when first = swapped_byte_order_mark -> rest | _ -> units in
    List.map (fun unit_ -> ((unit_ land 0xFF) lsl 8) lor (unit_ lsr 8)) rest

(* A wstring, in the form `policy` and `order` name, answered as UTF-8 —
   which is what the generated member holds. The form is never inferred
   from the stream. *)
let read_wstring_utf8 (r : reader) (policy : string) (order : string) : string =
  let position = r.r_at in
  let declared = read_u32 r in
  if not (wstring_order_known order) then
    fail_at position ("a wstring unit order, not `" ^ order ^ "`");
  let counts_characters, terminated, width =
    match wstring_cell policy with
    | Some cell -> cell
    | None -> fail_at position ("a wstring length policy, not `" ^ policy ^ "`")
  in
  let count =
    if counts_characters then declared
    else begin
      if declared mod width <> 0 then
        fail_at position (Printf.sprintf "a length of %d with %d bytes left" declared (reader_remaining r));
      declared / width
    end
  in
  if count * width > reader_remaining r then
    fail_at position
      (Printf.sprintf "a length of %d with %d bytes left" (count * width) (reader_remaining r));
  let little = order = "byte-order-mark" || order = "host-unswapped" in
  let units = ref [] in
  for _ = 1 to count do
    let unit_ =
      if width = 4 then begin
        let wide =
          if order = "big-endian-always" then read_u32_big r
          else if little then read_u32_little r
          else read_u32 r
        in
        if wide > 0xFFFF then
          fail_at position "a four-byte wstring character holding one UTF-16 unit";
        wide
      end
      else if order = "big-endian-always" then read_u16_big r
      else if little then read_u16_little r
      else read_u16 r
    in
    units := unit_ :: !units
  done;
  let units = List.rev !units in
  let units =
    if terminated then begin
      if declared < 1 then
        fail_at position (Printf.sprintf "a length of %d with %d bytes left" declared (reader_remaining r));
      match List.rev units with
      | last :: kept_reversed ->
        if last <> 0 then
          fail_at position "a NUL character ending the wstring, as its count includes one";
        let kept = List.rev kept_reversed in
        List.iter
          (fun unit_ ->
            if unit_ = 0 then fail_at position "no NUL inside a wstring a C peer would stop at")
          kept;
        kept
      | [] -> []
    end
    else units
  in
  let units = if order = "byte-order-mark" then strip_byte_order_mark units else units in
  let out = Buffer.create 16 in
  let rec walk = function
    | [] -> ()
    | high :: low :: rest when high >= 0xD800 && high <= 0xDBFF && low >= 0xDC00 && low <= 0xDFFF ->
      utf8_encode out (0x10000 + ((high - 0xD800) lsl 10) + (low - 0xDC00));
      walk rest
    | unit_ :: _ when unit_ >= 0xD800 && unit_ <= 0xDFFF ->
      fail_at position "a UTF-16 sequence with no lone surrogate"
    | unit_ :: rest ->
      utf8_encode out unit_;
      walk rest
  in
  walk units;
  Buffer.contents out

(* The inverse of [write_wstring_utf8_octets]: the declared length is the
   UTF-8 octet count, the terminator `policy` asks for is the last of those
   octets, and a count the stream cannot hold, a missing or an interior
   NUL, or an ill-formed UTF-8 sequence is refused. *)
let read_wstring_utf8_octets (r : reader) (policy : string) : string =
  let position = r.r_at in
  let declared = read_u32 r in
  if declared > reader_remaining r then
    fail_at position
      (Printf.sprintf "a length of %d with %d bytes left" declared (reader_remaining r));
  let _, terminated, _ =
    match wstring_cell policy with
    | Some cell -> cell
    | None -> fail_at position ("a wstring length policy, not `" ^ policy ^ "`")
  in
  let start = take r declared in
  let data = String.sub r.r_data start declared in
  let kept =
    if terminated then begin
      if declared = 0 || data.[String.length data - 1] <> '\000' then
        fail_at position "a NUL octet ending the wstring, as its count includes one";
      let kept = String.sub data 0 (String.length data - 1) in
      String.iter
        (fun c ->
          if c = '\000' then
            fail_at position "no NUL inside a wstring a C peer would stop at")
        kept;
      kept
    end
    else data
  in
  (* The declared length is the UTF-8 octet count: a value that is not
     UTF-8 is refused rather than carried as a string no reader could tell
     was never a wstring. *)
  ignore (utf8_points kept position);
  kept

(* `fixed<digits, scale>`, as the decimal string the writer took: the
   leading zeros go, and a negative zero loses its sign. *)
let read_fixed (r : reader) (digits : int) (scale : int) (count : string) : string =
  if count <> "declared" && count <> "value-precision" && count <> "value-string-digits" then
    fail_at r.r_at ("a fixed digit count, not `" ^ count ^ "`");
  (* The nibbles, collected most recent first and reversed into stream
     order below; the trailing sign is the stream's last nibble. *)
  let reversed = ref [] in
  if count = "value-precision" || count = "value-string-digits" then begin
    (* The field ends at the first octet whose low nibble is a sign; one
       that never presents a sign within the sixteen octets of the
       widest fixed IDL admits is refused. *)
    let stop = ref false in
    while not !stop do
      if List.length !reversed >= 32 then fail_at r.r_at "a sign nibble within sixteen octets";
      let b = byte_at r (take r 1) in
      reversed := (b land 0x0F) :: (b lsr 4) :: !reversed;
      if b land 0x0F >= 0x0A then stop := true
    done
  end
  else begin
    let width = (digits + 2) / 2 in
    let start = take r width in
    for i = 0 to width - 1 do
      let b = byte_at r (start + i) in
      reversed := (b land 0x0F) :: (b lsr 4) :: !reversed
    done
  end;
  let sign, nibbles =
    match !reversed with
    | s :: rest -> (s, List.rev rest)
    | [] -> (0x0C, [])
  in
  (* An even digit count starts with a pad nibble under "declared". *)
  let nibbles =
    if count = "declared" && (digits + 1) mod 2 = 1 then
      match nibbles with _ :: rest -> rest | [] -> []
    else nibbles
  in
  List.iter (fun nibble -> if nibble > 9 then fail_at r.r_at "BCD digits") nibbles;
  let text = Buffer.create 16 in
  List.iter (fun nibble -> Buffer.add_char text (Char.chr (Char.code '0' + nibble))) nibbles;
  let text = ref (Buffer.contents text) in
  while String.length !text < scale + 1 do text := "0" ^ !text done;
  let split = String.length !text - scale in
  let whole = ref (String.sub !text 0 split) in
  while String.length !whole > 1 && !whole.[0] = '0' do
    whole := String.sub !whole 1 (String.length !whole - 1)
  done;
  let fraction = String.sub !text split scale in
  let result = if scale > 0 then !whole ^ "." ^ fraction else !whole in
  let zero =
    !whole = "0"
    &&
    let all = ref true in
    String.iter (fun c -> if c <> '0' then all := false) fraction;
    !all
  in
  if sign = 0x0D && not zero then "-" ^ result else result

(* -- the trims, reached as reader operations by the shared walk *)

let trim_utf8 (_r : reader) (text : string) (octets : int) : string = trim_utf8_text text octets
let trim_utf16 (_r : reader) (text : string) (units : int) : string = trim_utf16_text text units

(* -- the bit-bounded kinds *)

(* An enumerator's holder, as the signed integer section 7.4.3.5.3 makes
   it — the holder's width and not the bound's. *)
let read_enum (r : reader) (bit_bound : int) : int64 =
  if bit_bound = 0 || bit_bound > 32 then
    fail_at r.r_at (Printf.sprintf "an @bit_bound between 1 and 32, not %d" bit_bound);
  if bit_bound <= 8 then Int64.of_int (read_i8 r)
  else if bit_bound <= 16 then Int64.of_int (read_i16 r)
  else Int64.of_int32 (read_i32 r)

(* A bitmask's holder. Table 33 of section 7.4.1.1.3.2. *)
let read_bitmask (r : reader) (bit_bound : int) : int64 =
  if bit_bound = 0 || bit_bound > 64 then
    fail_at r.r_at (Printf.sprintf "an @bit_bound between 1 and 64, not %d" bit_bound);
  if bit_bound <= 8 then Int64.of_int (read_u8 r)
  else if bit_bound <= 16 then Int64.of_int (read_u16 r)
  else if bit_bound <= 32 then Int64.of_int (read_u32 r)
  else read_unsigned r 8

(* A bitset's anchor integer, of the width the planner sized. *)
let read_bitset (r : reader) (anchor : int) : int64 =
  match anchor with
  | 1 -> Int64.of_int (read_u8 r)
  | 2 -> Int64.of_int (read_u16 r)
  | 4 -> Int64.of_int (read_u32 r)
  | 8 -> read_unsigned r 8
  | other -> fail_at r.r_at (Printf.sprintf "a bitset anchor of 1, 2, 4 or 8 bytes, not %d" other)

(* -- framing (reading) ----------------------------------------------------- *)

(* A DHEADER: the length the members that follow occupy, refusing one
   the stream cannot hold. *)
let read_dheader (r : reader) : int =
  let position = r.r_at in
  let length = read_u32 r in
  if length > reader_remaining r then
    fail_at position (Printf.sprintf "a length of %d with %d bytes left" length (reader_remaining r));
  length

let em_extent (length_code : int) (next_int : int) : int =
  if length_code <= 3 then 1 lsl length_code
  else if next_int < 0 then 0
  else if length_code = 4 then next_int
  else begin
    let scale = if length_code = 6 then 4 else if length_code = 7 then 8 else 1 in
    4 + (next_int * scale)
  end

(* An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they read, but
   the reader is put back before it: that word is the member's own
   leading length. Section 7.4.3.4.2. *)
let read_emheader (r : reader) : emheader_read =
  let word = Int64.to_int (read_unsigned r 4) in
  let member_id = word land e_member_id_mask in
  let length_code = (word lsr 28) land 0x7 in
  let must_understand = word lsr 31 <> 0 in
  let next_int =
    if length_code = 4 then read_u32 r
    else if length_code >= 5 then begin
      let position = r.r_at in
      let peeked = read_u32 r in
      r.r_at <- position;
      peeked
    end
    else -1
  in
  { em_member_id = member_id; em_length_code = length_code; em_must_understand = must_understand;
    em_next_int = next_int; em_extent = em_extent length_code next_int }

(* A parameter header, and the value block it opens: section 7.4.1.2
   makes a parameter's value its own stream block, so a member's internal
   alignment is counted from the first octet after the header. *)
let read_pid_as (r : reader) (role_ : role) : pid_read =
  reader_align_to r 4;
  let pid = read_u16 r in
  let declared = read_u16 r in
  if pid land pid_id_mask = pid_extended_id land pid_id_mask then begin
    (* The four flag bits are not part of the id: section 7.4.1.2.1
       splits the eMemberHeader into FLAG_1 to FLAG_4 and a 28-bit member
       id, and FLAG_2 is the member's must-understand. *)
    let member_header = Int64.to_int (read_unsigned r 4) in
    let wide_length = read_u32 r in
    let saved = r.r_origin in
    r.r_origin <- r.r_at;
    { pr_pid = member_header land e_member_id_mask; pr_length = wide_length; pr_end = false;
      pr_must_understand = member_header land e_flag_must_understand <> 0;
      pr_impl_extension = member_header land e_flag_impl_extension <> 0; pr_reserved = false;
      pr_ignore = false; pr_role = role_; pr_saved_origin = saved; pr_value_at = r.r_at }
  end
  else begin
    let identifier = pid land pid_id_mask in
    let end_ = identifier = pid_list_end land pid_id_mask in
    (* Under a framing that puts FLAG_IMPL_EXTENSION on every member id
       the bit no longer tells a vendor-private parameter from a user
       member; it is reported absent there because it carries no
       information. *)
    let flagged =
      pid land pid_impl_extension_bit <> 0
      && pid_flags_at r.r_framing.pid_flags "member" land pid_impl_extension_bit = 0
    in
    let saved = r.r_origin in
    r.r_origin <- r.r_at;
    { pr_pid = identifier; pr_length = declared; pr_end = end_;
      pr_must_understand = pid land pid_must_understand_bit <> 0; pr_impl_extension = flagged;
      pr_reserved = (not end_) && identifier > r.r_framing.highest_short_id;
      pr_ignore = identifier = pid_ignore_id; pr_role = role_; pr_saved_origin = saved;
      pr_value_at = r.r_at }
  end

let read_pid (r : reader) : pid_read = read_pid_as r Role_member

(* End a parameter's value and step to the next header. The declared
   length is measured against the value's own start and checked against
   both conventions where they coincide; the trailing padding to four is
   stepped only under a framing that writes one — Fast DDS's does not,
   and a final parameter there ends the stream where its value does. *)
let finish_pid_read (r : reader) (header : pid_read) : unit =
  r.r_origin <- header.pr_saved_origin;
  if r.r_status = St_ok && not header.pr_end then begin
    let used = r.r_at - header.pr_value_at in
    let declared = header.pr_length in
    let padded = (used + 3) land lnot 3 in
    let pads = pads_declared_length r.r_framing header.pr_role in
    if declared <> (if pads then padded else used) && declared <> used && declared <> padded then
      reader_refuse r St_out_of_range;
    if r.r_framing.pad_after_value then reader_align_to r 4
  end

(* -- JSON helpers for the generated dumpers -------------------------------- *)

let json_string (text : string) : string =
  let out = Buffer.create (String.length text + 2) in
  Buffer.add_char out '"';
  String.iter
    (fun c ->
      match c with
      | '"' -> Buffer.add_string out "\\\""
      | '\\' -> Buffer.add_string out "\\\\"
      | '\n' -> Buffer.add_string out "\\n"
      | '\r' -> Buffer.add_string out "\\r"
      | '\t' -> Buffer.add_string out "\\t"
      | c when Char.code c < 0x20 -> Buffer.add_string out (Printf.sprintf "\\u%04x" (Char.code c))
      | c -> Buffer.add_char out c)
    text;
  Buffer.add_char out '"';
  Buffer.contents out

(* One character, as the one-character JSON string the dialect writes. *)
let json_char (point : int) : string =
  let utf8 = Buffer.create 4 in
  utf8_encode utf8 point;
  json_string (Buffer.contents utf8)

(* A float as a JSON number, refused where JSON has none. *)
let json_float (v : float) (path : string) : string =
  match Float.classify_float v with
  | FP_nan | FP_infinite -> raise (Wire_error (path ^ ": the value has no JSON form"))
  | _ ->
    let shortest = Printf.sprintf "%.17g" v in
    let shorter = Printf.sprintf "%.16g" v in
    let even_shorter = Printf.sprintf "%.15g" v in
    if float_of_string even_shorter = v then even_shorter
    else if float_of_string shorter = v then shorter
    else shortest
