(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Native pure-OCaml endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
   byte-identical to the Rust core and the other SDKs. Sync (poll) and async
   (a Thread filling a Mutex/Condition mailbox — stdlib, no Lwt/Async). *)

module Wire = struct
  type endian = LE | BE

  (* The XCDR version the stream is written in. A constructor argument with
     no default: XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1 and
     four under XCDR2, so a writer that did not know which one it was in
     would be writing XCDR2 and calling it XCDR — which is what this SDK did
     until the cap became a field. `align` read a constant four, and every
     `long long` in an XCDR1 sample landed four octets early with nothing in
     the interface to say so. *)
  type encoding = XCDR1 | XCDR2

  let max_alignment = function XCDR1 -> 8 | XCDR2 -> 4

  (* The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The
     code occupies bits 28..30 of the EMHEADER and decides how the member's
     byte length is recovered: LC0..LC3 give it from the code alone, LC4..LC7
     from a word that follows or from the body's own leading word. *)
  let lc0 = 0   (* the member body is one octet *)
  let lc1 = 1   (* two octets *)
  let lc2 = 2   (* four octets *)
  let lc3 = 3   (* eight octets *)
  let lc4 = 4   (* a NEXTINT holding the body's byte length follows *)
  let lc5 = 5   (* the body's first four octets are the length, and are body *)
  let lc6 = 6   (* as LC5, and the length counts four-octet elements *)
  let lc7 = 7   (* as LC5, and the length counts eight-octet elements *)

  (* PID_LIST_END, and the PID that introduces the twelve-octet parameter
     header. XTypes 1.3 §7.4.1.2 Table 31. *)
  let pid_list_end_id = 0x3F02
  let pid_extended_id = 0x3F01

  (* The highest member id a short parameter header carries, inclusive —
     which is what RTI and Fast DDS write for `@id(16128)`. *)
  let pid_short_max_id = 0x3F00

  (* Bit 14 of a short parameter id. Bit 15 is the implementation-extension
     flag and is never set here. *)
  let pid_flag_must_understand = 0x4000

  (* Bit 30 of the eMemberHeader inside an extended parameter header — not
     bit 31, which is the impl-extension flag. *)
  let em_flag_must_understand = 0x4000_0000

  (* The two's complement pattern of [v] in a holder of [bits], or a refusal.

     Both spellings of an integer argument reach the wire through the same
     call: the unsigned pattern a `uint16` carries and the signed value an
     `int16` carries. Both are taken, and the bound is the union of the two
     ranges — the same bound the corpus generator uses to decide that a
     sample fits its member.

     Refused and not masked. `put_u8 w 300` was `Char.chr (300 land 0xff)`
     and wrote `0x2C`, and `put_u16 w 70000` went through `le_of_int`, which
     masks each octet, and answered `7011`. Either is a frame of the right
     length carrying a different number.

     OCaml's `int` is 63 bits on a 64-bit target and 31 on a 32-bit one.
     `Sys.int_size` says which. Where the holder is at least as wide as the
     `int` the caller has, there is nothing outside it left to refuse — the
     value could not have been written down in the first place — so the check
     is skipped there rather than computing `1 lsl 32` in a 31-bit word and
     getting zero. That is a property of the platform's `int` and not of the
     wire, which is why it is named here instead of being silently absent. *)
  let pattern v bits =
    if bits < Sys.int_size then begin
      let top = (1 lsl bits) - 1 in
      let bottom = -(1 lsl (bits - 1)) in
      if v > top || v < bottom then
        invalid_arg (Printf.sprintf "%d does not fit %d bits" v bits)
    end;
    if bits >= Sys.int_size then v else v land ((1 lsl bits) - 1)

  (* --- Writer: a growable Bytes + endian; alignment relative to origin --- *)

  (* A growable [Bytes.t] and not a [Buffer.t]: a DHEADER, a NEXTINT and a
     PL_CDR1 parameter length are all a slot reserved before the body and
     filled in once the body's length is known, and [Buffer] has no way to
     write into a position it has already passed. *)
  type writer = {
    mutable data : bytes;
    mutable len : int;
    endian : endian;
    (* The alignment cap, from the encoding the writer was made with. *)
    max_align : int;
    (* The position alignment is measured from. Zero for the stream, and
       moved to the start of a PL_CDR1 parameter's value for the length of
       that parameter — §7.4.1.2 aligns a parameter's value against its own
       block, not against the stream. Without it a `double` in a parameter
       takes four octets of padding and the header declares twelve where
       Cyclone, RTI and Fast DDS all declare eight. *)
    mutable origin : int;
  }

  let writer endian encoding =
    { data = Bytes.create 64; len = 0; endian;
      max_align = max_alignment encoding; origin = 0 }

  let ensure w n =
    let need = w.len + n in
    if need > Bytes.length w.data then begin
      let cap = ref (max 64 (Bytes.length w.data)) in
      while !cap < need do cap := !cap * 2 done;
      let bigger = Bytes.create !cap in
      Bytes.blit w.data 0 bigger 0 w.len;
      w.data <- bigger
    end

  let push w c =
    ensure w 1;
    Bytes.set w.data w.len c;
    w.len <- w.len + 1

  let align w a =
    let cap = min a w.max_align in
    let pad = (cap - ((w.len - w.origin) mod cap)) mod cap in
    for _ = 1 to pad do push w '\000' done

  (* le: the value already encoded little-endian; reversed for BE. *)
  let put w a (le : bytes) =
    align w a;
    let n = Bytes.length le in
    if w.endian = BE then
      for i = n - 1 downto 0 do push w (Bytes.get le i) done
    else
      for i = 0 to n - 1 do push w (Bytes.get le i) done

  let le_of_int v n =
    let b = Bytes.create n in
    for i = 0 to n - 1 do
      Bytes.set b i (Char.chr ((v lsr (8 * i)) land 0xff))
    done;
    b

  let le_of_int64 (v : int64) =
    let b = Bytes.create 8 in
    for i = 0 to 7 do
      let byte =
        Int64.to_int (Int64.logand (Int64.shift_right_logical v (8 * i)) 0xffL)
      in
      Bytes.set b i (Char.chr byte)
    done;
    b

  let put_u8 w v = push w (Char.chr (pattern v 8))
  let put_u16 w v = put w 2 (le_of_int (pattern v 16) 2)
  let put_u32 w v = put w 4 (le_of_int (pattern v 32) 4)

  (* A 64-bit integer at its natural eight-octet alignment, which [max_align]
     then caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1. It
     passed 4 here regardless of the encoding, so every `long long` in an
     XCDR1 sample landed four octets early.

     The argument is an [int64] and not an [int] because OCaml's `int` is 63
     bits: `0x8000000000000000` has no `int` on any target, and a path that
     went through one would drop the bit that distinguishes it from zero. *)
  let put_u64 w (v : int64) = put w 8 (le_of_int64 v)

  let put_f32 w (v : float) =
    let bits = Int32.bits_of_float v in
    let b = Bytes.create 4 in
    for i = 0 to 3 do
      let byte =
        Int32.to_int (Int32.logand (Int32.shift_right_logical bits (8 * i)) 0xffl)
      in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  (* Binary64 as its IEEE-754 bit pattern, at the same alignment as put_u64.
     `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had
     no writer here at all. OCaml's `float` is already a binary64, so nothing
     narrows on the way. *)
  let put_f64 w (v : float) = put w 8 (le_of_int64 (Int64.bits_of_float v))

  (* A `boolean` as the one octet §7.4.1.1.1 Table 30 gives it: 0 or 1. *)
  let put_bool w (v : bool) = put_u8 w (if v then 1 else 0)

  let put_bytes w (b : bytes) =
    for i = 0 to Bytes.length b - 1 do push w (Bytes.get b i) done

  let put_string w s =
    put_u32 w (String.length s + 1);
    for i = 0 to String.length s - 1 do push w s.[i] done;
    put_u8 w 0

  let put_seq_u8 w (b : bytes) =
    put_u32 w (Bytes.length b);
    put_bytes w b

  (* An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.

     A code point above U+00FF is refused and not truncated. `š` is U+0161
     and its low octet is 0x61, which is `a`: a truncating writer produces a
     frame of the right length holding a different, plausible character and
     nothing downstream can tell. *)
  let put_char w code_point =
    if code_point < 0 || code_point > 0xFF then
      invalid_arg
        (Printf.sprintf "expected a character below U+0100, found U+%04X" code_point);
    put_u8 w code_point

  (* An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
     order.

     The four policy axes a wide character carries are answered here and not
     by the caller: the unit is two octets wide, it goes out in the stream's
     byte order, there is no GIOP length octet before it, and no byte-order
     mark. That is what Cyclone and Fast DDS write in both encodings and what
     RTI writes under XCDR2; RTI's XCDR1 `wchar` is four octets, which is the
     one reading this binding does not take.

     A code point above U+FFFF is refused. The refusal is width-independent —
     a four-octet form would zero-extend a code *unit*, not a code point. *)
  let put_wchar w code_point =
    if code_point < 0 || code_point > 0xFFFF then
      invalid_arg
        (Printf.sprintf "expected a BMP character, found U+%04X" code_point);
    put_u16 w code_point

  (* An IDL `wstring` from its UTF-16 code units.

     The length word counts **octets** and not code units, there is no
     terminator, there is no byte-order mark, and each unit goes out two
     octets wide in the stream's own byte order. Those are the four axes and
     they are the binding's answer. Cyclone writes exactly this in both
     encodings and RTI under XCDR2; Fast DDS counts units, which is the
     reading this binding does not take. *)
  let put_wstring w (units : int array) =
    put_u32 w (Array.length units * 2);
    Array.iter (fun unit -> put_u16 w unit) units

  (* A `bitset` as its anchor word: the holder the declared bits pack into,
     written at the anchor's own width and therefore at the anchor's own
     alignment. XTypes 1.3 §7.3.1.4 and §7.4.1.

     [bits] is an [int64] for the same reason [put_u64]'s argument is: an
     eight-octet anchor with bit 63 set has no OCaml `int`, and a path
     through one would lose exactly the bit the type declared. *)
  let put_bitset w (bits : int64) anchor =
    match anchor with
    | 1 -> put_u8 w (Int64.to_int bits)
    | 2 -> put_u16 w (Int64.to_int bits)
    | 4 -> put_u32 w (Int64.to_int bits)
    | 8 -> put_u64 w bits
    | n ->
      invalid_arg
        (Printf.sprintf "a bitset anchor is one, two, four or eight octets, not %d" n)

  (* The packing [put_fixed] writes, apart so it can be tested without a
     writer.

     An IDL `fixed<digits,scale>` as packed BCD.
     CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.

     [value] is the decimal as the sample wrote it — an optional sign, an
     integer part and an optional fraction. Everything after that is this
     call's:

     - the integer part is left-padded and the fraction right-padded to
       exactly [digits] decimal digits, with the point implied [scale] from
       the right; the scale itself is not on the wire,
     - a leading zero nibble is prepended when [digits + 1] is odd, so digits
       and sign pair up,
     - two nibbles to the octet, most significant digit first, high nibble
       first,
     - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative
       zero goes out positive, because there is one zero to write,
     - alignment is one, and no multi-octet integer is written, so the byte
       order never enters.

     The octet count is therefore ceil((digits + 1) / 2) whatever the value,
     which is the point omniORB 4.3.3 and JacORB 3.9 differ on and the one
     this binding takes from omniORB. A decimal string and not a float:
     `fixed<31,0>` has thirty-one significant digits and OCaml's `float`
     holds about sixteen. *)
  let fixed_octets value digits scale =
    if digits < 1 || digits > 31 then
      invalid_arg (Printf.sprintf "fixed<%d,%d>: a digit count is 1..31" digits scale);
    if scale < 0 || scale > digits then
      invalid_arg (Printf.sprintf "fixed<%d,%d>: a scale is 0..digits" digits scale);
    let negative =
      String.length value > 0 && (value.[0] = '-')
    in
    let text =
      if String.length value > 0 && (value.[0] = '+' || value.[0] = '-') then
        String.sub value 1 (String.length value - 1)
      else value
    in
    let integral, fraction =
      match String.index_opt text '.' with
      | Some at ->
        (String.sub text 0 at, String.sub text (at + 1) (String.length text - at - 1))
      | None -> (text, "")
    in
    if integral = "" && fraction = "" then
      invalid_arg (Printf.sprintf "fixed<%d,%d>: `%s` has no digits" digits scale value);
    String.iter
      (fun c ->
        if c < '0' || c > '9' then
          invalid_arg
            (Printf.sprintf "fixed<%d,%d>: `%s` is not a decimal" digits scale value))
      (integral ^ fraction);
    (* Leading zeros carry no value, so they go before the width is checked:
       `000000.001` is one significant integer digit, not six. *)
    let trimmed = ref 0 in
    while !trimmed < String.length integral && integral.[!trimmed] = '0' do
      incr trimmed
    done;
    let integral = String.sub integral !trimmed (String.length integral - !trimmed) in
    if String.length integral > digits - scale then
      invalid_arg
        (Printf.sprintf "fixed<%d,%d>: `%s` needs %d integer digits and the type has %d"
           digits scale value (String.length integral) (digits - scale));
    (* A trailing zero in the fraction is padding either way, so it goes
       before the width is checked and is re-added below. *)
    let fraction = ref fraction in
    while
      String.length !fraction > scale && !fraction.[String.length !fraction - 1] = '0'
    do
      fraction := String.sub !fraction 0 (String.length !fraction - 1)
    done;
    let fraction = !fraction in
    if String.length fraction > scale then
      invalid_arg
        (Printf.sprintf "fixed<%d,%d>: `%s` needs %d fraction digits and the type has %d"
           digits scale value (String.length fraction) scale);

    let nibbles = ref [] in
    let push_nibble n = nibbles := n :: !nibbles in
    if (digits + 1) mod 2 = 1 then push_nibble 0;
    for _ = 1 to digits - scale - String.length integral do push_nibble 0 done;
    String.iter (fun c -> push_nibble (Char.code c - Char.code '0')) integral;
    String.iter (fun c -> push_nibble (Char.code c - Char.code '0')) fraction;
    for _ = 1 to scale - String.length fraction do push_nibble 0 done;
    let non_zero = List.exists (fun n -> n <> 0) !nibbles in
    push_nibble (if negative && non_zero then 0x0D else 0x0C);
    let flat = Array.of_list (List.rev !nibbles) in
    let out = Bytes.create (Array.length flat / 2) in
    for i = 0 to (Array.length flat / 2) - 1 do
      Bytes.set out i (Char.chr ((flat.(2 * i) lsl 4) lor flat.((2 * i) + 1)))
    done;
    out

  let put_fixed w value digits scale = put_bytes w (fixed_octets value digits scale)

  let bytes w = Bytes.sub w.data 0 w.len

  (* --- back-patched framing ---------------------------------------------

     A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
     shape: a slot reserved before the body and filled in once the body's
     length is known. These two are the only places that write into an
     already-written position, and they honour the wire byte order like every
     other write. [at] is a 0-based offset, so the arithmetic reads the same
     as the other fourteen bindings. *)

  let patch w at width v =
    let le = le_of_int v width in
    for i = 0 to width - 1 do
      let octet = if w.endian = BE then Bytes.get le (width - 1 - i) else Bytes.get le i in
      Bytes.set w.data (at + i) octet
    done

  (* Reserve a 4-aligned DHEADER slot and return the position the body starts
     at, to hand back to [dheader_end]. XTypes 1.3 §7.4.3.5.

     `@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so
     a struct written with no annotation at all had no form here before. *)
  let dheader_begin w =
    put_u32 w 0;
    w.len

  (* Back-patch the DHEADER with the byte length written since [body_start].
     The DHEADER itself is not counted — §7.4.3.5 measures the content that
     follows it. *)
  let dheader_end w body_start = patch w (body_start - 4) 4 (w.len - body_start)

  (* The EMHEADER for one member of a `@mutable` aggregate. §7.4.3.4.2:

       EMHEADER = (must_understand << 31) | (length_code << 28) | member_id

     with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
     [emheader_end] fills it in; for LC0..LC3 the code alone gives the length
     and nothing follows; for LC5..LC7 the body's own leading four octets are
     the length, so no separate NEXTINT is written and writing one would put
     every following member at the wrong offset. *)
  let emheader_begin w id must_understand length_code =
    if id < 0 || id > 0x0FFF_FFFF then
      invalid_arg
        (Printf.sprintf "member id %d does not fit the 28 bits of an EMHEADER" id);
    if length_code < lc0 || length_code > lc7 then
      invalid_arg (Printf.sprintf "length code %d is not one of LC0..LC7" length_code);
    let header = (length_code lsl 28) lor id in
    let header = if must_understand then header lor (1 lsl 31) else header in
    put_u32 w header;
    if length_code = lc4 then put_u32 w 0;
    w.len

  (* Close the member [emheader_begin] opened.

     The length code is taken back rather than remembered, so a caller that
     paired an LC4 begin with a differently-coded end gets a refusal instead
     of a NEXTINT patched over the first four octets of a member body. *)
  let emheader_end w body_start length_code =
    let written = w.len - body_start in
    if length_code = lc4 then patch w (body_start - 4) 4 written
    else if length_code <= lc3 then begin
      (* 1, 2, 4, 8 — the code *is* the length, so a body of any other size
         means the plan and the member disagree. Silence here would put a
         length on the wire that no octet count backs. *)
      let want = 1 lsl length_code in
      if written <> want then
        invalid_arg
          (Printf.sprintf "LC%d frames %d octets and the member wrote %d"
             length_code want written)
    end
    else if written < 4 then
      invalid_arg
        (Printf.sprintf
           "LC%d takes its length from the body's first four octets and the member wrote %d"
           length_code written)

  (* An open PL_CDR1 parameter header, to be handed back to [pid_end]. *)
  type pid = {
    pat : int;             (* the position of the length field *)
    pextended : bool;      (* whether the length field is four octets or two *)
    psaved_origin : int;
  }

  (* A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
     boundary an `unsigned short` parameter id, then an `unsigned short` byte
     length of the value that follows. Both go out in the stream's byte order.

     The must-understand flag is bit 14 of the id field. Bit 15 is the
     implementation-extension flag and is not set here.

     The parameter's value aligns against the block and not against the
     stream, so the alignment origin moves for the length of the parameter
     and [pid_end] restores it. *)
  let pid_begin w id must_understand =
    if id < 0 || id > pid_short_max_id then
      invalid_arg (Printf.sprintf "member id %d needs the extended parameter header" id);
    align w 4;
    let field = if must_understand then id lor pid_flag_must_understand else id in
    put w 2 (le_of_int field 2);
    let at = w.len in
    push w '\000';
    push w '\000';
    let saved = w.origin in
    w.origin <- w.len;
    { pat = at; pextended = false; psaved_origin = saved }

  (* The twelve-octet parameter header §7.4.1.2 requires for a member id
     above 0x3F00 or a value longer than 65535 octets:

       [PID_EXTENDED | mu : u16][8 : u16][eMemberHeader : u32][length : u32]

     The must-understand flag on the marker is not the member's — it is on
     the marker under this framing whatever the member says, and the member's
     own flag is bit 30 of the eMemberHeader. Reading it off the marker
     instead makes every parameter must-understand. *)
  let pid_extended_begin w member_id must_understand =
    if member_id < 0 || member_id > 0x0FFF_FFFF then
      invalid_arg
        (Printf.sprintf "member id %d does not fit the 28 bits of an eMemberHeader"
           member_id);
    align w 4;
    put w 2 (le_of_int (pid_extended_id lor pid_flag_must_understand) 2);
    put w 2 (le_of_int 8 2);
    let header =
      if must_understand then member_id lor em_flag_must_understand else member_id
    in
    put w 4 (le_of_int header 4);
    let at = w.len in
    for _ = 1 to 4 do push w '\000' done;
    let saved = w.origin in
    w.origin <- w.len;
    { pat = at; pextended = true; psaved_origin = saved }

  (* Close the parameter [pid_begin] or [pid_extended_begin] opened.

     The declared length counts the value's octets **and** the padding that
     brings the next header back to a four-octet boundary: a reader skips
     [length] from the end of the header and has to land on the next
     parameter. The padding is written after the length is patched, so the
     two cannot disagree. *)
  let pid_end w header =
    let width = if header.pextended then 4 else 2 in
    let length = w.len - header.pat - width in
    (* The parameter's own origin is still in force here, so the round-up is
       measured from the block that the length describes. *)
    let past = (w.len - header.psaved_origin) mod 4 in
    let length = if past <> 0 then length + (4 - past) else length in
    if (not header.pextended) && length > 0xFFFF then
      invalid_arg
        (Printf.sprintf "a short parameter header describes at most 65535 octets, not %d"
           length);
    patch w header.pat width length;
    w.origin <- header.psaved_origin;
    align w 4

  (* The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
     with a zero length, four-aligned. §7.4.1.2.

     The must-understand flag is set on the sentinel — 0x7F02 on the wire,
     which is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02;
     the flag is not the sentinel's meaning either way, and this is the
     majority form. *)
  let pid_list_end w =
    align w 4;
    put w 2 (le_of_int (pid_list_end_id lor pid_flag_must_understand) 2);
    put w 2 (le_of_int 0 2)

  (* --- Reader: a bytes cursor --- *)

  type reader = {
    data : bytes;
    mutable pos : int;
    rendian : endian;
    (* The same cap the writer used, or the fields land elsewhere. *)
    rmax_align : int;
    (* Mirrors the writer's: a PL_CDR1 parameter's value aligns against its
       own block. *)
    mutable rorigin : int;
  }

  let reader data endian encoding =
    { data; pos = 0; rendian = endian; rmax_align = max_alignment encoding; rorigin = 0 }

  let take r a n =
    let cap = min a r.rmax_align in
    let pad = (cap - ((r.pos - r.rorigin) mod cap)) mod cap in
    r.pos <- r.pos + pad;
    let b = Bytes.sub r.data r.pos n in
    r.pos <- r.pos + n;
    if r.rendian = BE then begin
      let out = Bytes.create n in
      for i = 0 to n - 1 do Bytes.set out i (Bytes.get b (n - 1 - i)) done;
      out
    end
    else b

  let get_u32 r =
    let b = take r 4 4 in
    Char.code (Bytes.get b 0)
    lor (Char.code (Bytes.get b 1) lsl 8)
    lor (Char.code (Bytes.get b 2) lsl 16)
    lor (Char.code (Bytes.get b 3) lsl 24)

  (* --- primitives beyond get_u32 — the byte-exact inverse of the Writer --- *)

  let get_u8 r =
    let v = Char.code (Bytes.get r.data r.pos) in
    r.pos <- r.pos + 1;
    v

  let get_u16 r =
    let b = take r 2 2 in
    Char.code (Bytes.get b 0) lor (Char.code (Bytes.get b 1) lsl 8)

  (* A 64-bit integer at its natural eight-octet alignment, which
     [rmax_align] then caps — the same eight the writer passes. It passed 4
     here while the writer passed 4 as well, so both were wrong together
     under XCDR1 and the round trip never showed it. *)
  let get_u64 r =
    let b = take r 8 8 in
    let v = ref 0L in
    for i = 0 to 7 do
      v :=
        Int64.logor !v
          (Int64.shift_left (Int64.of_int (Char.code (Bytes.get b i))) (8 * i))
    done;
    !v

  let get_f32 r = Int32.float_of_bits (Int32.of_int (get_u32 r))
  let get_f64 r = Int64.float_of_bits (get_u64 r)
  let get_bool r = get_u8 r <> 0
  (* The one octet an IDL `char` occupies. *)
  let get_char r = get_u8 r
  (* One two-octet `wchar` code unit. *)
  let get_wchar r = get_u16 r

  (* A `wstring` back into its UTF-16 code units. The length word counts
     octets, so the unit count is half of it. *)
  let get_wstring r =
    let octets = get_u32 r in
    Array.init (octets / 2) (fun _ -> get_u16 r)

  (* A `bitset` anchor of the given width back into its holder word. *)
  let get_bitset r anchor =
    match anchor with
    | 1 -> Int64.of_int (get_u8 r)
    | 2 -> Int64.of_int (get_u16 r)
    | 4 -> Int64.of_int (get_u32 r)
    | 8 -> get_u64 r
    | n ->
      invalid_arg
        (Printf.sprintf "a bitset anchor is one, two, four or eight octets, not %d" n)

  (* A `fixed<digits,scale>` back into the decimal it was written from: a
     leading `-` when the sign nibble is 0xD, [digits - scale] integer
     digits, and a `.` with [scale] fraction digits where the scale is not
     zero. The scale is not on the wire, so it is an argument here for the
     same reason it is one in [put_fixed]. *)
  let get_fixed r digits scale =
    let octets = (digits + 2) / 2 in
    let nibbles = Array.make (octets * 2) 0 in
    for i = 0 to octets - 1 do
      let b = get_u8 r in
      nibbles.((2 * i)) <- b lsr 4;
      nibbles.((2 * i) + 1) <- b land 0x0F
    done;
    let sign = nibbles.(Array.length nibbles - 1) in
    let first = Array.length nibbles - 1 - digits in
    let out = Buffer.create (digits + 2) in
    if sign = 0x0D then Buffer.add_char out '-';
    for i = 0 to digits - 1 do
      if scale <> 0 && i = digits - scale then Buffer.add_char out '.';
      Buffer.add_char out (Char.chr (Char.code '0' + nibbles.(first + i)))
    done;
    Buffer.contents out

  let get_string r =
    let n = get_u32 r in
    let s = Bytes.sub_string r.data r.pos (n - 1) in
    r.pos <- r.pos + n;
    s

  let get_seq_u8 r =
    let n = get_u32 r in
    let b = Bytes.sub r.data r.pos n in
    r.pos <- r.pos + n;
    b

  (* A DHEADER, as the announced body byte length. *)
  let dheader_read r = get_u32 r

  (* A member header: the id, the must-understand flag, the length code, and
     the member body's extent in octets measured from the position this call
     leaves the cursor at.

     LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
     follows the header. LC5..LC7 consume nothing: the next four octets are
     the member's own leading word and belong to the body, so they are peeked
     and the extent is 4 + word for LC5, 4 + 4*word for LC6 and 4 + 8*word
     for LC7 — the last two count elements, not octets. *)
  let emheader_read r =
    let header = get_u32 r in
    let id = header land 0x0FFF_FFFF in
    let must_understand = header land (1 lsl 31) <> 0 in
    let length_code = (header lsr 28) land 0x7 in
    let extent =
      if length_code <= lc3 then 1 lsl length_code
      else if length_code = lc4 then get_u32 r
      else begin
        let at = r.pos in
        let word = get_u32 r in
        r.pos <- at;
        if length_code = lc5 then 4 + word
        else if length_code = lc6 then 4 + (4 * word)
        else 4 + (8 * word)
      end
    in
    (id, must_understand, length_code, extent)

  (* A PL_CDR1 parameter header in either form: the id with its flags
     stripped, the member's must-understand flag, the declared byte length,
     and whether the list ends here.

     The extended form is recognised by the marker PID_EXTENDED, and the
     must-understand answer then comes from bit 30 of the eMemberHeader and
     never from the marker — the marker carries the flag under this framing
     whatever the member says.

     The value's alignment origin is moved to the position this call leaves,
     and [pid_skip] restores it. *)
  let pid_read r =
    let raw = get_u16 r in
    let short = raw land 0x3FFF in
    if short = pid_list_end_id then begin
      ignore (get_u16 r);
      (0, false, 0, true)
    end
    else if short = pid_extended_id then begin
      ignore (get_u16 r); (* the slength, always 8 *)
      let header = get_u32 r in
      let length = get_u32 r in
      r.rorigin <- r.pos;
      (header land 0x0FFF_FFFF, header land em_flag_must_understand <> 0, length, false)
    end
    else begin
      let length = get_u16 r in
      r.rorigin <- r.pos;
      (short, raw land pid_flag_must_understand <> 0, length, false)
    end

  (* Move the cursor past a parameter value of [length] octets from
     [body_start] and restore the stream's alignment origin. *)
  let pid_skip r body_start length =
    r.pos <- body_start + length;
    r.rorigin <- 0;
    while r.pos mod 4 <> 0 do r.pos <- r.pos + 1 done
end

module Endpoint = struct
  let session_nokey = 0x80
  let stream_best_effort = 0x01

  (* The 16-bit submessage_length field cannot describe a sample larger than
     0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
     truncate the length while appending the full payload (the C SDK's
     zdw_xrce_write_frame rejects the same way).

     `invalid_arg` and no longer `Bytes.create 0`. `Client.write` handed the
     empty bytes to `deliver` without looking, so
     `Bytes.length (write_frame 0x80 0x01 1 (Bytes.create 70000)) = 0` went out
     as an empty datagram with no error anywhere. *)
  (* E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3
     -- DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would
     additionally require a SampleInfo in the body, which this profile does not
     carry. *)
  let write_flags = 0x01

  (* Wire size of the BaseObjectRequest that precedes the sample (7.7.8). *)
  let base_size = 4

  (* OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2). *)
  let objk_datawriter = 0x05

  (* An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind. *)
  let object_id raw_id kind = ((raw_id land 0x0FFF) lsl 4) lor (kind land 0x0F)

  (* The submessage body is the BaseObjectRequest (7.7.8: request_id then
     object_id, each two octets most significant first) followed by the sample.
     [object_id] names the target DataWriter -- without it no XRCE agent can
     route the sample. The request_id is the message's own [seq], which 7.7.8
     only requires to be unique per stream. *)
  let write_frame session stream seq obj_id (sample : bytes) =
    let n = base_size + Bytes.length sample in
    if n > 0xFFFF then
      invalid_arg
        (Printf.sprintf
           "%d octets: the 16-bit XRCE submessage_length describes at most 65535"
           (Bytes.length sample))
    else
    let hdr = Bytes.create 12 in
    Bytes.set hdr 0 (Char.chr session);
    Bytes.set hdr 1 (Char.chr stream);
    Bytes.set hdr 2 (Char.chr (seq land 0xff));
    Bytes.set hdr 3 (Char.chr ((seq lsr 8) land 0xff));
    Bytes.set hdr 4 (Char.chr 0x07);
    Bytes.set hdr 5 (Char.chr write_flags);
    Bytes.set hdr 6 (Char.chr (n land 0xff));
    Bytes.set hdr 7 (Char.chr ((n lsr 8) land 0xff));
    Bytes.set hdr 8 (Char.chr ((seq lsr 8) land 0xff));
    Bytes.set hdr 9 (Char.chr (seq land 0xff));
    Bytes.set hdr 10 (Char.chr ((obj_id lsr 8) land 0xff));
    Bytes.set hdr 11 (Char.chr (obj_id land 0xff));
    Bytes.cat hdr sample

  (* Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
     len_hi] then the sample body. The 16-bit little-endian submessage_length
     (bytes 6..7) bounds the body exactly: Bytes.sub frame 8 sm_len, never
     (Bytes.length frame - 8), so trailing padding or an appended submessage is
     not folded into the sample.

     Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
     hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
     Returns None for a short header, a wrong submessage id, or a declared
     length that runs past the datagram (truncation / wrong length). *)
  let read_frame (frame : bytes) =
    if Bytes.length frame < 8 + base_size then None
    else
      let id = Char.code (Bytes.get frame 4) in
      if id <> 0x07 && id <> 0x09 then None
      else
        let sm_len =
          Char.code (Bytes.get frame 6) lor (Char.code (Bytes.get frame 7) lsl 8)
        in
        if sm_len < base_size || 8 + sm_len > Bytes.length frame then None
        else Some (Bytes.sub frame (8 + base_size) (sm_len - base_size))

  (* [(request_id, object_id)] of a data frame (7.7.8), or None under the same
     rules as [read_frame]. *)
  let frame_base (frame : bytes) =
    match read_frame frame with
    | None -> None
    | Some _ ->
      Some
        ( (Char.code (Bytes.get frame 8) lsl 8) lor Char.code (Bytes.get frame 9),
          (Char.code (Bytes.get frame 10) lsl 8) lor Char.code (Bytes.get frame 11) )
end

(* transport: deliver a frame, receive a frame (or None). *)
type transport = { deliver : bytes -> unit; receive : unit -> bytes option }

module Client = struct
  type t = {
    transport : transport;
    session : int;
    stream : int;
    (* ObjectId of the target DataWriter (7.7.8 BaseObjectRequest). *)
    object_id : int;
    mutable seq : int;
  }

  let create transport object_id =
    {
      transport;
      session = Endpoint.session_nokey;
      stream = Endpoint.stream_best_effort;
      object_id;
      seq = 1;
    }

  (* A sample the envelope cannot carry raises out of `write_frame`, so the
     sequence is not advanced and nothing reaches the transport. *)
  let write c (sample : bytes) =
    let frame = Endpoint.write_frame c.session c.stream c.seq c.object_id sample in
    c.transport.deliver frame;
    c.seq <- (c.seq + 1) land 0xffff

  (* One non-blocking receive: returns the sample body, or None. *)
  let poll c =
    match c.transport.receive () with
    | None -> None
    | Some frame -> Endpoint.read_frame frame
end

(* A blocking FIFO mailbox (Mutex + Condition), the idiomatic stdlib primitive. *)
module Mailbox = struct
  type 'a t = { mutable q : 'a list; m : Mutex.t; c : Condition.t }

  let create () = { q = []; m = Mutex.create (); c = Condition.create () }

  let put mb x =
    Mutex.lock mb.m;
    mb.q <- mb.q @ [ x ];
    Condition.signal mb.c;
    Mutex.unlock mb.m

  let take mb =
    Mutex.lock mb.m;
    while mb.q = [] do Condition.wait mb.c mb.m done;
    let x = List.hd mb.q in
    mb.q <- List.tl mb.q;
    Mutex.unlock mb.m;
    x
end

module AsyncReader = struct
  type t = { mutable running : bool; mb : bytes Mailbox.t }

  let start transport =
    let r = { running = true; mb = Mailbox.create () } in
    let rec loop () =
      if r.running then begin
        (match transport.receive () with
         | None -> Thread.delay 0.001
         | Some frame -> (
             match Endpoint.read_frame frame with
             | Some body -> Mailbox.put r.mb body
             | None -> ()));
        loop ()
      end
    in
    ignore (Thread.create loop ());
    r

  let recv r = Mailbox.take r.mb
  let stop r = r.running <- false
end

module MemTransport = struct
  let create () =
    let q = ref [] in
    let m = Mutex.create () in
    {
      deliver = (fun frame ->
        Mutex.lock m;
        q := !q @ [ frame ];
        Mutex.unlock m);
      receive = (fun () ->
        Mutex.lock m;
        let r = match !q with [] -> None | x :: xs -> q := xs; Some x in
        Mutex.unlock m;
        r);
    }
end
