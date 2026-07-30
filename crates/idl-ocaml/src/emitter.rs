// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → OCaml emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! OCaml source file: a `Wire` module (byte-identical to `endpoints/ocaml`) plus,
//! per IDL `struct`, a module with a record type `t` and a `marshal(v, endian)`
//! function. `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlOcamlError::Unsupported`].

use std::fmt::Write as _;

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{
    Annotation, BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstType,
    ConstrTypeDecl, Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType,
    IntegerType, InterfaceDcl, Literal, LiteralKind, Member, PrimitiveType, ScopedName,
    SequenceType, Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp,
    UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlOcamlError, Result};
use crate::keywords::escape_ocaml_ident;

thread_local! {
    /// Fully-qualified IDL scope path of every named type declaration
    /// (e.g. `["a", "Reading"]`), populated by [`register_type_paths`] at the
    /// start of each run. A reference site resolves a (possibly partially
    /// qualified) `ScopedName` against the enclosing module scope by walking
    /// outward and matching one of these paths (§7.5.2), then flattens the
    /// match the SAME way [`qualify`] flattens the definition (#21).
    static TYPE_PATHS: std::cell::RefCell<Vec<Vec<String>>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Module scope of the aggregate currently being emitted. Set at the top of
    /// [`emit_struct`]/[`emit_union`]; empty at global scope.
    static CURRENT_SCOPE: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Flattened logical names of every `bitset`/`bitmask` declaration. A
    /// reference to one of these maps to an OCaml holder module whose wire form
    /// is a single backing integer (`marshal_into`/`read`) — no collection
    /// DHEADER, so it is treated as fully-descriptive (primitive) by the
    /// sequence/map framing rules (XTypes 1.3 §7.4.7).
    static BIT_NAMES: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());

    /// Set whenever a `fixed<P,S>` member is emitted, so the BCD prelude helper
    /// is appended exactly once (and only when needed).
    static USED_FIXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// Flattened qualified enum name → signed wire holder width in OCTETS
    /// (1/2/4), from `@bit_bound` (XTypes 1.3 §7.3.1.2.1.9 + §7.4.5.1) via the
    /// shared [`enum_wire_octets`]. Populated once per run; read at the single
    /// enum encode/decode site so a `@bit_bound(8)`/`@bit_bound(16)` enum
    /// narrows to 1/2 bytes instead of the former fixed 4.
    static ENUM_WIDTHS: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Signed wire holder width in octets (1/2/4) an enum named `name` serializes
/// at, per its `@bit_bound`. Defaults to 4 for an unregistered name / no
/// `@bit_bound` (XTypes 1.3 §7.4.5.1 default bound 32).
fn enum_wire_width(name: &str) -> u32 {
    ENUM_WIDTHS
        .with(|m| m.borrow().get(name).copied())
        .unwrap_or(4)
}

/// OCaml codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`zerodds_idl::semantics::annotations::Lowered::verbatims_for_language`]).
const OCAML_LANG_ALIASES: &[&str] = &["ocaml", "ml", "caml"];

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Emits every `@verbatim` block from `anns` whose language matches the OCaml
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-d`'s
/// `emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(OCAML_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            out.push_str(indent);
            out.push_str(line);
            out.push('\n');
        }
    }
}

/// Top-level annotations of a definition, for file-scope (`BEGIN_FILE` /
/// `END_FILE`) and per-declaration `@verbatim` placement. Mirrors `idl-d`'s
/// `def_annotations`.
fn def_annotations(d: &Definition) -> &[Annotation] {
    match d {
        Definition::Module(m) => &m.annotations,
        Definition::Type(TypeDecl::Constr(c)) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => &s.annotations,
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => &u.annotations,
            ConstrTypeDecl::Enum(e) => &e.annotations,
            ConstrTypeDecl::Bitset(b) => &b.annotations,
            ConstrTypeDecl::Bitmask(b) => &b.annotations,
            _ => &[],
        },
        Definition::Type(TypeDecl::Typedef(t)) => &t.annotations,
        Definition::Const(c) => &c.annotations,
        Definition::Except(e) => &e.annotations,
        _ => &[],
    }
}

/// Injective flattened logical name for a declaration `simple` in module
/// `scope`, via the shared [`zerodds_idl::naming::encode_scoped`] encoding. The
/// scope separator (`_s`) and a literal underscore in a name (`_u`) are
/// distinct, so `A::B_C` and `A_B::C` no longer collapse to `A_B_C` (unlike the
/// old `join("_")`, which was NOT collision-free). This logical name is then
/// lowered to an OCaml type name ([`type_ident`]) or upper-cased to an OCaml
/// module name ([`module_name`]) at the emission site. Two same-simple-name
/// types in different modules become distinct OCaml modules (#21).
fn qualify(scope: &[String], simple: &str) -> String {
    zerodds_idl::naming::encode_scoped(scope, simple)
}

/// Records the fully-qualified path of every named type declaration before
/// emission, so reference resolution can flatten a name the same way the
/// definition site does.
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn register_type_paths(defs: &[Definition], scope: &mut Vec<String>) {
    for def in defs {
        match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                register_type_paths(&m.definitions, scope);
                scope.pop();
            }
            Definition::Type(td) => register_type_decl_path(td, scope),
            // Interface-nested types are promoted to the top level under the
            // interface's own scope segment (#A39), so their reference paths
            // must be registered the same way the definition site flattens them.
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        register_type_decl_path(td, scope);
                    }
                }
                scope.pop();
            }
            _ => {}
        }
    }
}

/// Registers the fully-qualified path of a single `TypeDecl` (used for both
/// module-level and interface-nested declarations — #A39).
fn register_type_decl_path(td: &TypeDecl, scope: &[String]) {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            push_type_path(scope, &s.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => push_type_path(scope, &e.name.text),
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            push_type_path(scope, &u.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => push_type_path(scope, &b.name.text),
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => push_type_path(scope, &b.name.text),
        TypeDecl::Typedef(td) => {
            for d in &td.declarators {
                push_type_path(scope, &d.name().text);
            }
        }
        _ => {}
    }
}

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39). OCaml has no interface/nested
/// construct, so these are promoted to the top level under the interface's own
/// name segment (so two interfaces in one module do not collide), and their
/// DDS data types survive instead of being silently dropped with the body.
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_iface_types(defs: &[Definition]) -> Vec<(Vec<String>, &TypeDecl)> {
    let mut out = Vec::new();
    let mut scope = Vec::new();
    flatten_iface_types_into(defs, &mut scope, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_iface_types_into<'a>(
    defs: &'a [Definition],
    scope: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, &'a TypeDecl)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                flatten_iface_types_into(&m.definitions, scope, out);
                scope.pop();
            }
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        out.push((scope.clone(), td));
                    }
                }
                scope.pop();
            }
            _ => {}
        }
    }
}

fn push_type_path(scope: &[String], simple: &str) {
    let mut path = scope.to_vec();
    path.push(simple.to_string());
    TYPE_PATHS.with(|t| t.borrow_mut().push(path));
}

/// Resolves a referenced `ScopedName` against [`CURRENT_SCOPE`], returning the
/// flattened logical name (`join("_")`) of the matching declaration. Mirrors
/// IDL name lookup (§7.5.2): for each prefix of the enclosing scope (longest
/// first), then the global scope, check whether `prefix + parts` is a known
/// type path. Falls back to the literal flattening of the written parts.
fn resolve_scoped_name(sn: &ScopedName) -> String {
    let parts: Vec<String> = sn.parts.iter().map(|p| p.text.clone()).collect();
    let scope = CURRENT_SCOPE.with(|s| s.borrow().clone());
    let known: Vec<Vec<String>> = TYPE_PATHS.with(|t| t.borrow().clone());
    for cut in (0..=scope.len()).rev() {
        let mut cand = scope[..cut].to_vec();
        cand.extend(parts.iter().cloned());
        if known.contains(&cand) {
            return flatten_path(&cand);
        }
    }
    flatten_path(&parts)
}

/// Flattens a full type path (module components + simple name) exactly as the
/// definition site does via [`qualify`]/[`zerodds_idl::naming::encode_scoped`],
/// so a reference resolves to the same logical name the declaration emitted.
fn flatten_path(path: &[String]) -> String {
    match path.split_last() {
        Some((simple, scope)) => zerodds_idl::naming::encode_scoped(scope, simple),
        None => String::new(),
    }
}

/// Options for the OCaml backend.
#[derive(Debug, Clone, Default)]
pub struct OcamlGenOptions {}

/// The `Wire` module, byte-identical to `endpoints/ocaml`'s Wire.
const WIRE_MODULE: &str = r#"module Wire = struct
  type endian = LE | BE

  type writer = { buf : Buffer.t; endian : endian }

  let writer endian = { buf = Buffer.create 64; endian }

  let align w a =
    let cap = min a 4 in
    let pad = (cap - (Buffer.length w.buf mod cap)) mod cap in
    for _ = 1 to pad do Buffer.add_char w.buf '\000' done

  let put w a (le : bytes) =
    align w a;
    let n = Bytes.length le in
    if w.endian = BE then
      for i = n - 1 downto 0 do Buffer.add_char w.buf (Bytes.get le i) done
    else Buffer.add_bytes w.buf le

  let le_of_int v n =
    let b = Bytes.create n in
    for i = 0 to n - 1 do
      Bytes.set b i (Char.chr ((v lsr (8 * i)) land 0xff))
    done;
    b

  let put_u8 w v = Buffer.add_char w.buf (Char.chr (v land 0xff))
  let put_bool w v = put_u8 w (if v then 1 else 0)
  let put_u16 w v = put w 2 (le_of_int v 2)
  let put_u32 w v = put w 4 (le_of_int v 4)

  let put_u64 w (v : int64) =
    let b = Bytes.create 8 in
    for i = 0 to 7 do
      let byte = Int64.to_int (Int64.logand (Int64.shift_right_logical v (8 * i)) 0xffL) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_f32 w (v : float) =
    let bits = Int32.bits_of_float v in
    let b = Bytes.create 4 in
    for i = 0 to 3 do
      let byte = Int32.to_int (Int32.logand (Int32.shift_right_logical bits (8 * i)) 0xffl) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_f64 w (v : float) = put_u64 w (Int64.bits_of_float v)
  let put_bytes w (b : bytes) = Buffer.add_bytes w.buf b

  let put_string w s =
    put_u32 w (String.length s + 1);
    Buffer.add_string w.buf s;
    put_u8 w 0

  let put_seq_u8 w (b : bytes) =
    put_u32 w (Bytes.length b);
    Buffer.add_bytes w.buf b

  (* Self-contained UTF-8 scalar decode at byte offset [i]: returns
     (code point, byte length). Emitted in-tree instead of the stdlib's
     UTF-8 decode helpers, which are OCaml 4.14+, so the generated module
     also compiles on OCaml 4.13. Well-formed 1..4-byte sequences decode
     exactly; a malformed lead or truncated/continuation byte yields U+FFFD
     and advances one byte, matching the stdlib's replacement-character
     behavior for the encode/count paths. *)
  let zd_utf8_decode (s : string) (i : int) : int * int =
    let n = String.length s in
    let b k = Char.code (String.unsafe_get s k) in
    let cont k = k < n && b k land 0xC0 = 0x80 in
    let c0 = b i in
    if c0 < 0x80 then (c0, 1)
    else if c0 < 0xC0 then (0xFFFD, 1)
    else if c0 < 0xE0 then
      if cont (i + 1) then (((c0 land 0x1F) lsl 6) lor (b (i + 1) land 0x3F), 2)
      else (0xFFFD, 1)
    else if c0 < 0xF0 then
      if cont (i + 1) && cont (i + 2) then
        ( ((c0 land 0x0F) lsl 12)
          lor ((b (i + 1) land 0x3F) lsl 6)
          lor (b (i + 2) land 0x3F),
          3 )
      else (0xFFFD, 1)
    else if c0 < 0xF8 then
      if cont (i + 1) && cont (i + 2) && cont (i + 3) then
        ( ((c0 land 0x07) lsl 18)
          lor ((b (i + 1) land 0x3F) lsl 12)
          lor ((b (i + 2) land 0x3F) lsl 6)
          lor (b (i + 3) land 0x3F),
          4 )
      else (0xFFFD, 1)
    else (0xFFFD, 1)

  let put_wstring w s =
    let out = ref [] in
    let i = ref 0 in
    let n = String.length s in
    while !i < n do
      let cp, len = zd_utf8_decode s !i in
      i := !i + len;
      if cp <= 0xFFFF then out := cp :: !out
      else begin
        let rr = cp - 0x10000 in
        out := (0xDC00 lor (rr land 0x3FF)) :: (0xD800 lor (rr lsr 10)) :: !out
      end
    done;
    let units = List.rev !out in
    put_u32 w (List.length units * 2);
    List.iter (fun u -> put_u16 w u) units

  let put_long_double w (v : float) =
    let bits = Int64.bits_of_float v in
    let sign = Int64.shift_right_logical bits 63 in
    let exp = Int64.logand (Int64.shift_right_logical bits 52) 0x7FFL in
    let mant = Int64.logand bits 0xFFFFFFFFFFFFFL in
    let hi = ref (Int64.shift_left sign 63) in
    let lo = ref 0L in
    if not (exp = 0L && mant = 0L) then begin
      hi :=
        Int64.logor
          (Int64.logor (Int64.shift_left sign 63)
             (Int64.shift_left (Int64.add (Int64.sub exp 1023L) 16383L) 48))
          (Int64.shift_right_logical mant 4);
      lo := Int64.shift_left (Int64.logand mant 0xFL) 60
    end;
    let le = Bytes.create 16 in
    for i = 0 to 7 do
      Bytes.set le i
        (Char.chr (Int64.to_int (Int64.logand (Int64.shift_right_logical !lo (8 * i)) 0xFFL)));
      Bytes.set le (8 + i)
        (Char.chr (Int64.to_int (Int64.logand (Int64.shift_right_logical !hi (8 * i)) 0xFFL)))
    done;
    put w 4 le

  let bytes w = Buffer.to_bytes w.buf

  type reader = { rbuf : bytes; mutable pos : int; rendian : endian }

  let reader (b : bytes) endian = { rbuf = b; pos = 0; rendian = endian }

  let ralign r a =
    let cap = min a 4 in
    while r.pos mod cap <> 0 do r.pos <- r.pos + 1 done

  let get_u8 r =
    let v = Char.code (Bytes.get r.rbuf r.pos) in
    r.pos <- r.pos + 1;
    v

  let get_bool r = get_u8 r <> 0

  let get_le r a n =
    ralign r a;
    let v = ref 0L in
    if r.rendian = BE then
      for i = 0 to n - 1 do
        v := Int64.logor (Int64.shift_left !v 8) (Int64.of_int (Char.code (Bytes.get r.rbuf (r.pos + i))))
      done
    else
      for i = n - 1 downto 0 do
        v := Int64.logor (Int64.shift_left !v 8) (Int64.of_int (Char.code (Bytes.get r.rbuf (r.pos + i))))
      done;
    r.pos <- r.pos + n;
    !v

  let get_u16 r = Int64.to_int (get_le r 2 2)
  let get_u32 r = Int64.to_int (get_le r 4 4)
  let get_u64 r = get_le r 4 8
  let get_f32 r = Int32.float_of_bits (Int32.of_int (get_u32 r))
  let get_f64 r = Int64.float_of_bits (get_u64 r)

  let get_bytes_n r n =
    let b = Bytes.sub r.rbuf r.pos n in
    r.pos <- r.pos + n;
    b

  let get_string r =
    let n = get_u32 r in
    let s = Bytes.sub_string r.rbuf r.pos (n - 1) in
    r.pos <- r.pos + n;
    s

  let get_seq_u8 r =
    let n = get_u32 r in
    get_bytes_n r n

  let get_wstring r =
    let n = get_u32 r / 2 in
    let units = Array.make (max n 1) 0 in
    for i = 0 to n - 1 do units.(i) <- get_u16 r done;
    let buf = Buffer.create (n * 2) in
    let i = ref 0 in
    while !i < n do
      let u = units.(!i) in
      let cp =
        if u >= 0xD800 && u <= 0xDBFF && !i + 1 < n then begin
          let lo = units.(!i + 1) in
          i := !i + 2;
          0x10000 + ((u - 0xD800) lsl 10) + (lo - 0xDC00)
        end
        else begin
          incr i;
          u
        end
      in
      Buffer.add_utf_8_uchar buf (Uchar.of_int cp)
    done;
    Buffer.contents buf

  let get_long_double r =
    ralign r 4;
    let le = Bytes.copy (get_bytes_n r 16) in
    if r.rendian = BE then
      for i = 0 to 7 do
        let t = Bytes.get le i in
        Bytes.set le i (Bytes.get le (15 - i));
        Bytes.set le (15 - i) t
      done;
    let lo = ref 0L and hi = ref 0L in
    for i = 0 to 7 do
      lo := Int64.logor !lo (Int64.shift_left (Int64.of_int (Char.code (Bytes.get le i))) (8 * i));
      hi := Int64.logor !hi (Int64.shift_left (Int64.of_int (Char.code (Bytes.get le (8 + i)))) (8 * i))
    done;
    let sign = Int64.shift_right_logical !hi 63 in
    let exp = Int64.logand (Int64.shift_right_logical !hi 48) 0x7FFFL in
    let mant =
      Int64.logor
        (Int64.shift_left (Int64.logand !hi 0xFFFFFFFFFFFFL) 4)
        (Int64.shift_right_logical !lo 60)
    in
    let bits =
      if exp = 0L && mant = 0L then Int64.shift_left sign 63
      else
        Int64.logor
          (Int64.logor (Int64.shift_left sign 63)
             (Int64.shift_left (Int64.add (Int64.sub exp 16383L) 1023L) 52))
          mant
    in
    Int64.float_of_bits bits
end
"#;

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
let zd_fixed_enc (s : string) (p : int) (scale : int) : bytes =
  let sign = ref true in
  let i = ref 0 in
  let n = String.length s in
  if n > 0 && (s.[0] = '-' || s.[0] = '+') then begin
    sign := s.[0] <> '-';
    i := 1
  end;
  let rest = String.sub s !i (n - !i) in
  let dot = try String.index rest '.' with Not_found -> String.length rest in
  let ip = String.sub rest 0 dot in
  let fp =
    if dot < String.length rest then String.sub rest (dot + 1) (String.length rest - dot - 1)
    else ""
  in
  let db = Buffer.create 16 in
  let int_needed = p - scale in
  for _ = String.length ip to int_needed - 1 do Buffer.add_char db '0' done;
  Buffer.add_string db ip;
  Buffer.add_string db fp;
  for _ = String.length fp to scale - 1 do Buffer.add_char db '0' done;
  let digits = Buffer.contents db in
  let nib = ref [] in
  if p mod 2 = 0 then nib := 0 :: !nib;
  String.iter (fun c -> nib := (Char.code c - Char.code '0') :: !nib) digits;
  nib := (if !sign then 0x0C else 0x0D) :: !nib;
  let nibs = Array.of_list (List.rev !nib) in
  let out = Buffer.create 8 in
  let k = ref 0 in
  while !k < Array.length nibs do
    Buffer.add_char out (Char.chr ((nibs.(!k) lsl 4) lor nibs.(!k + 1)));
    k := !k + 2
  done;
  Buffer.to_bytes out
"#;

/// Generates a self-contained OCaml module from the IDL AST.
///
/// # Errors
/// Returns [`IdlOcamlError::Unsupported`] for constructs the OCaml backend does
/// not yet emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_ocaml_module(spec: &Specification, _opts: &OcamlGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "(* Code generated by zerodds-idlc (OCaml backend). DO NOT EDIT. *)"
    );
    let _ = writeln!(out, "(* SPDX-License-Identifier: Apache-2.0 *)\n");
    out.push_str(WIRE_MODULE);

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());
    USED_FIXED.with(|f| f.set(false));

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all top-level defs
    // (source order), emitted after the wire prelude, before any type.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::BeginFile);
    }

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);

    // Interface-nested type declarations (#A39): promoted to the top level
    // under the interface's own scope segment, so their DDS data types survive
    // instead of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Named enums/structs/bit-containers referenced by members, keyed by their
    // flattened module-qualified name (matching the definition site). Both
    // module-level and interface-nested type decls contribute (#A39).
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    let mut bit_names: HashSet<String> = HashSet::new();
    let mut enum_defs: HashMap<String, &EnumDef> = HashMap::new();
    for (scope, td) in flat
        .iter()
        .filter_map(|(s, d)| match d {
            Definition::Type(td) => Some((s, td)),
            _ => None,
        })
        .chain(iface_types.iter().map(|(s, td)| (s, *td)))
    {
        match td {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
                let n = qualify(scope, &e.name.text);
                enum_defs.insert(n.clone(), e);
                enum_names.insert(n);
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                struct_names.insert(qualify(scope, &s.name.text));
            }
            // `bitset`/`bitmask` logical names, published to `BIT_NAMES` so a
            // reference site resolves them to the integer-backed holder module
            // (no collection DHEADER — fully descriptive, XTypes 1.3 §7.4.7).
            TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => {
                bit_names.insert(qualify(scope, &b.name.text));
            }
            TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => {
                bit_names.insert(qualify(scope, &b.name.text));
            }
            _ => {}
        }
    }
    BIT_NAMES.with(|b| *b.borrow_mut() = bit_names);
    // Register each enum's @bit_bound-derived wire width (1/2/4 octets), P1.
    ENUM_WIDTHS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (name, e) in &enum_defs {
            m.insert(
                name.clone(),
                u32::from(enum_wire_octets(enum_bit_bound(&e.annotations))),
            );
        }
    });

    let mut typedefs = collect_typedefs(spec);
    let mut struct_defs = collect_struct_defs(spec);
    // Fold interface-nested typedefs/structs into the resolution maps (#A39).
    for (scope, td) in &iface_types {
        match td {
            TypeDecl::Typedef(tdd) => {
                for d in &tdd.declarators {
                    if let Declarator::Simple(name) = d {
                        typedefs.insert(qualify(scope, &name.text), tdd.type_spec.clone());
                    }
                }
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                struct_defs.insert(qualify(scope, &s.name.text), s);
            }
            _ => {}
        }
    }

    // Emit every type / const in DOCUMENT ORDER, descending into modules and
    // interface bodies in place (#A39). OCaml requires definition-before-use at
    // the top level; IDL's own declaration-before-use rule (§7.5.2) then makes
    // the emitted OCaml well-ordered — a promoted interface-nested type and any
    // module-level type that references it keep their source relative order.
    emit_defs_ordered(
        &mut out,
        &spec.definitions,
        &mut Vec::new(),
        &enum_names,
        &struct_names,
        &typedefs,
        &struct_defs,
        &enum_defs,
    )?;

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    if USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(FIXED_PRELUDE);
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (struct / union / enum / bitset / bitmask). Shared
/// by the module-level emit loop and the interface-nested type promotion (#A39)
/// so both paths stay byte-identical. `typedef`s are wire-transparent and carry
/// no emission of their own.
#[allow(clippy::too_many_arguments)]
fn emit_type_decl(
    out: &mut String,
    td: &TypeDecl,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(
                out,
                s,
                scope,
                enum_names,
                struct_names,
                typedefs,
                struct_defs,
            )?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            emit_union(out, u, scope, enum_names, struct_names, typedefs, enum_defs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, scope)?,
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => emit_bitmask(out, b, scope),
        _ => {}
    }
    Ok(())
}

/// Emits every type / `const` in document order, descending into `module` and
/// `interface` bodies in place. Preserving source order keeps the generated
/// OCaml definition-before-use, since IDL already forbids forward references
/// (§7.5.2) — a module-level type that names an interface-nested type (#A39)
/// always appears after it in the source, and thus in the output.
/// zerodds-lint: recursion-depth 16 (module/interface nesting; bounded by the
/// IDL grammar).
#[allow(clippy::too_many_arguments)]
fn emit_defs_ordered(
    out: &mut String,
    defs: &[Definition],
    scope: &mut Vec<String>,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    for def in defs {
        match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                emit_defs_ordered(
                    out,
                    &m.definitions,
                    scope,
                    enum_names,
                    struct_names,
                    typedefs,
                    struct_defs,
                    enum_defs,
                )?;
                scope.pop();
            }
            Definition::Type(td) => {
                let anns = def_annotations(def);
                // §7.2.2.4.8 — text directly before the annotated declaration.
                emit_verbatim_at(out, "", anns, PlacementKind::BeforeDeclaration);
                emit_type_decl(
                    out,
                    td,
                    scope,
                    enum_names,
                    struct_names,
                    typedefs,
                    struct_defs,
                    enum_defs,
                )?;
                // §7.2.2.4.8 — text directly after the annotated declaration.
                emit_verbatim_at(out, "", anns, PlacementKind::AfterDeclaration);
            }
            // #A5/P1 — a top-level `const` was silently dropped by the former
            // catch-all arm; emit it as an OCaml top-level `let` binding.
            Definition::Const(c) => {
                let anns = def_annotations(def);
                emit_verbatim_at(out, "", anns, PlacementKind::BeforeDeclaration);
                emit_const(out, c, scope);
                emit_verbatim_at(out, "", anns, PlacementKind::AfterDeclaration);
            }
            // #A39 — interface-nested type declarations promoted to the top
            // level, in place, under the interface's own scope segment.
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        emit_type_decl(
                            out,
                            td,
                            scope,
                            enum_names,
                            struct_names,
                            typedefs,
                            struct_defs,
                            enum_defs,
                        )?;
                    }
                }
                scope.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

/// Emits a top-level IDL `const` as an OCaml `let` binding (#A5/P1). The value
/// is wire-neutral — a codegen convenience — so an expression the renderer
/// cannot express (e.g. a `fixed` arithmetic tree) is skipped rather than
/// emitting a non-compiling token. The name is lower-cased (OCaml `let` names
/// must be lowercase-initial) and reserved-word-escaped, matching `type_ident`.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_ocaml(&c.value, &c.type_) else {
        return;
    };
    let name = type_ident(&qualify(scope, &c.name.text));
    let _ = writeln!(out, "\nlet {name} = {val}");
}

/// Renders a `ConstExpr` as an OCaml value expression for the given `const`
/// type, or `None` for a form the backend does not express.
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_ocaml(e: &ConstExpr, ct: &ConstType) -> Option<String> {
    let is64 = matches!(
        ct,
        ConstType::Integer(
            IntegerType::LongLong
                | IntegerType::ULongLong
                | IntegerType::Int64
                | IntegerType::UInt64
        )
    );
    match e {
        ConstExpr::Literal(l) => const_literal_to_ocaml(l, ct),
        // A scoped value: for an enum-typed const it names an enumerator (the
        // emitted variant constructor, verbatim); otherwise a const-alias whose
        // OCaml `let` name is lower-cased-initial (`type_ident`).
        ConstExpr::Scoped(sn) => {
            let last = sn.parts.last()?.text.clone();
            if matches!(ct, ConstType::Scoped(_)) {
                Some(last)
            } else {
                Some(type_ident(&last))
            }
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_ocaml(operand, ct)?;
            Some(match op {
                UnaryOp::Plus => v,
                UnaryOp::Minus if is64 => format!("(Int64.neg {v})"),
                UnaryOp::Minus => format!("(- {v})"),
                UnaryOp::BitNot if is64 => format!("(Int64.lognot {v})"),
                UnaryOp::BitNot => format!("(lnot {v})"),
            })
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_ocaml(lhs, ct)?;
            let r = const_expr_to_ocaml(rhs, ct)?;
            if is64 {
                let f = match op {
                    BinaryOp::Or => "Int64.logor",
                    BinaryOp::Xor => "Int64.logxor",
                    BinaryOp::And => "Int64.logand",
                    BinaryOp::Shl => {
                        return Some(format!("(Int64.shift_left {l} (Int64.to_int {r}))"));
                    }
                    BinaryOp::Shr => {
                        return Some(format!("(Int64.shift_right {l} (Int64.to_int {r}))"));
                    }
                    BinaryOp::Add => "Int64.add",
                    BinaryOp::Sub => "Int64.sub",
                    BinaryOp::Mul => "Int64.mul",
                    BinaryOp::Div => "Int64.div",
                    BinaryOp::Mod => "Int64.rem",
                };
                Some(format!("({f} {l} {r})"))
            } else {
                let o = match op {
                    BinaryOp::Or => "lor",
                    BinaryOp::Xor => "lxor",
                    BinaryOp::And => "land",
                    BinaryOp::Shl => "lsl",
                    BinaryOp::Shr => "lsr",
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "mod",
                };
                Some(format!("({l} {o} {r})"))
            }
        }
    }
}

/// Renders a single `const` literal as an OCaml value, normalizing the IDL
/// surface syntax to OCaml (e.g. `TRUE`→`true`, integer→`Int64` when the const
/// type is 64-bit, a bare integer→float when the const type is floating).
fn const_literal_to_ocaml(l: &Literal, ct: &ConstType) -> Option<String> {
    let raw = l.raw.trim();
    let is64 = matches!(
        ct,
        ConstType::Integer(
            IntegerType::LongLong
                | IntegerType::ULongLong
                | IntegerType::Int64
                | IntegerType::UInt64
        )
    );
    Some(match l.kind {
        LiteralKind::Integer => {
            let v = parse_int(raw)?;
            if matches!(ct, ConstType::Floating(_)) {
                format!("{v}.0")
            } else if is64 {
                format!("{v}L")
            } else {
                v.to_string()
            }
        }
        LiteralKind::Floating => {
            let s = raw.trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L']);
            if s.contains('.') || s.contains('e') || s.contains('E') {
                s.to_string()
            } else {
                format!("{s}.0")
            }
        }
        // A `fixed` decimal has no native OCaml type — render as a string
        // (matches the emitted `fixed` field carrying its BCD/decimal form).
        LiteralKind::Fixed => {
            format!(
                "\"{}\"",
                raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
            )
        }
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow char/string pass through (OCaml shares C-style escapes for the
        // common cases); wide literals drop the non-OCaml `L` prefix.
        LiteralKind::Char | LiteralKind::String => raw.to_string(),
        LiteralKind::WideChar | LiteralKind::WideString => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point,
/// so a union `case 'A':` resolves to the discriminant 65 (#A12/A13).
fn char_literal_value(raw: &str) -> Option<i64> {
    let s = raw.trim().strip_prefix('L').unwrap_or(raw.trim());
    let inner = s.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut it = inner.chars();
    let c = it.next()?;
    if c == '\\' {
        let e = it.next()?;
        let v = match e {
            'n' => 0x0A,
            't' => 0x09,
            'r' => 0x0D,
            '0' => 0x00,
            '\\' => 0x5C,
            '\'' => 0x27,
            '"' => 0x22,
            'a' => 0x07,
            'b' => 0x08,
            'f' => 0x0C,
            'v' => 0x0B,
            'x' => return i64::from_str_radix(it.as_str(), 16).ok(),
            _ => return None,
        };
        Some(v)
    } else {
        Some(i64::from(u32::from(c)))
    }
}

/// Evaluates a union case label (`case RED:`, `case 'A':`, `case TRUE:`,
/// `case 3:`) to its integer discriminant (#A11/A12/A13/P4). Beyond the plain
/// integer literals the former `array_size` accepted, this resolves enum
/// enumerators (via `enum_vals`, name → value of the switch enum), `char`
/// code points, and the `boolean` keywords `TRUE`/`FALSE`.
/// zerodds-lint: recursion-depth 64 (Const-Expr-Tree; bounded by IDL nesting)
fn eval_union_label(e: &ConstExpr, enum_vals: &HashMap<String, i64>) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal { kind, raw, .. }) => match kind {
            LiteralKind::Integer => parse_int(raw),
            LiteralKind::Char | LiteralKind::WideChar => char_literal_value(raw),
            LiteralKind::Boolean => Some(i64::from(raw.trim().eq_ignore_ascii_case("true"))),
            _ => None,
        },
        // `case ENUMERATOR:` — the label names an enumerator of the switch enum
        // (resolved by its simple, i.e. last, segment).
        ConstExpr::Scoped(sn) => {
            let last = sn.parts.last()?.text.clone();
            enum_vals.get(&last).copied()
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_union_label(operand, enum_vals)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(-v),
                UnaryOp::BitNot => Some(!v),
            }
        }
        ConstExpr::Binary { .. } => None,
    }
}

/// Resolves each enumerator's discriminant: default 0..N-1, honoring `@value`
/// (XTypes 1.3 §7.4.5.1 — the returned `i32` values match the wire encoding).
fn enumerator_values(e: &EnumDef) -> Vec<i32> {
    let mut values = Vec::with_capacity(e.enumerators.len());
    let mut next: i64 = 0;
    for en in &e.enumerators {
        let explicit = en.annotations.iter().find_map(|a| match lower_single(a) {
            Ok(Some(BuiltinAnnotation::Value(s))) => parse_int(&s),
            _ => None,
        });
        let v = explicit.unwrap_or(next);
        values.push(v as i32);
        next = i64::from(v as i32) + 1;
    }
    values
}

/// Parses a decimal or `0x` hex integer literal (possibly signed).
fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<i64>().ok()
    }
}

/// OCaml type identifier (lower-cased) from an IDL name, escaped if the
/// lower-cased form collides with an OCaml reserved word.
fn type_ident(name: &str) -> String {
    let mut c = name.chars();
    let lowered = match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    };
    escape_ocaml_ident(&lowered)
}

/// Emits an IDL `enum` as an OCaml variant type + a `<name>_to_int` function.
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = type_ident(&qualify(scope, &e.name.text));
    let ctors: Vec<String> = e
        .enumerators
        .iter()
        .map(|en| en.name.text.clone())
        .collect();
    let _ = writeln!(
        out,
        "
type {ty} = {}",
        ctors.join(" | ")
    );
    let arms: Vec<String> = e
        .enumerators
        .iter()
        .zip(&values)
        .map(|(en, v)| format!("{} -> {v}", en.name.text))
        .collect();
    let _ = writeln!(out, "let {ty}_to_int = function {}", arms.join(" | "));
    // Inverse (decode): total function; unknown values fall back to the first
    // constructor (never hit for values produced by `_to_int`).
    let of_arms: Vec<String> = e
        .enumerators
        .iter()
        .zip(&values)
        .map(|(en, v)| format!("{v} -> {}", en.name.text))
        .collect();
    let fallback = ctors.first().cloned().unwrap_or_default();
    let _ = writeln!(
        out,
        "let {ty}_of_int = function {} | _ -> {fallback}",
        of_arms.join(" | ")
    );
}

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→u8, `≤16`→u16, `≤32`→u32, else
/// u64). Returns `(OCaml type, put-fn, get-fn, is_int64)`. Storage ≤32 bits fits
/// OCaml's native 63-bit `int`; 64-bit uses `Int64` (`Wire.put_u64`/`get_u64`).
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str, bool) {
    match total_bits {
        0..=8 => ("int", "put_u8", "get_u8", false),
        9..=16 => ("int", "put_u16", "get_u16", false),
        17..=32 => ("int", "put_u32", "get_u32", false),
        _ => ("int64", "put_u64", "get_u64", true),
    }
}

/// Effective `@bit_bound` of a bitmask (default 32 — XTypes 1.3 §7.3.1.2.1.1:
/// an unannotated bitmask is a UInt32 on the wire, NOT the count of bits).
fn bitmask_bit_bound(anns: &[Annotation]) -> u32 {
    lower_annotations(anns)
        .ok()
        .and_then(|l| {
            l.builtins.iter().find_map(|a| match a {
                BuiltinAnnotation::BitBound(n) => Some(u32::from(*n)),
                _ => None,
            })
        })
        .unwrap_or(32)
}

/// `@position(n)` of a bitmask value, if present.
fn bit_position(anns: &[Annotation]) -> Option<u32> {
    lower_annotations(anns).ok().and_then(|l| {
        l.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Position(n) => Some(*n),
            _ => None,
        })
    })
}

/// Shared tail for a `bitset`/`bitmask` OCaml holder module:
/// `marshal_into`/`marshal`/`read`/`unmarshal` writing exactly the backing
/// integer via `Wire.{put}`/`Wire.{get}` (XTypes 1.3 §7.4.7 — wire = backing
/// int, no DHEADER), then the closing `end`.
fn write_bit_module(out: &mut String, put: &str, get: &str) {
    let _ = writeln!(
        out,
        "\n  let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="
    );
    let _ = writeln!(out, "    ignore endian;");
    let _ = writeln!(out, "    Wire.{put} w v.storage");
    let _ = writeln!(
        out,
        "\n  let marshal (v : t) (endian : Wire.endian) : bytes ="
    );
    let _ = writeln!(out, "    let w = Wire.writer endian in");
    let _ = writeln!(out, "    marshal_into v w endian;");
    let _ = writeln!(out, "    Wire.bytes w");
    let _ = writeln!(out, "\n  let read (r : Wire.reader) : t =");
    let _ = writeln!(out, "    {{ storage = Wire.{get} r }}");
    let _ = writeln!(
        out,
        "\n  let unmarshal (b : bytes) (endian : Wire.endian) : t ="
    );
    let _ = writeln!(out, "    read (Wire.reader b endian)");
    let _ = writeln!(out, "end");
}

/// Emits an IDL `bitset` as an OCaml holder module over its backing integer,
/// with a bit-accessor (getter + `set_` setter) per named bitfield and an XCDR2
/// `marshal`/`unmarshal` writing the backing integer (XTypes 1.3 §7.4.7).
///
/// # Errors
/// [`IdlOcamlError::Unsupported`] if a bitfield width is not a codegen-time
/// non-negative integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlOcamlError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (storage, put, get, is64) = bit_storage(total);
    let module = module_name(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nmodule {module} = struct");
    let _ = writeln!(out, "  type t = {{ mutable storage : {storage} }}");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_ocaml_ident(&name.text);
            write_bit_accessor(out, &field, offset, *width, is64);
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::EndDeclaration);
    write_bit_module(out, put, get);
    Ok(())
}

/// Emits one bit-accessor pair (getter + `set_` setter) for a bitfield of
/// `width` bits starting at `offset`. Width 1 → `bool`; wider → the backing
/// integer type. `is64` selects `Int64` arithmetic for a 64-bit backing store.
fn write_bit_accessor(out: &mut String, field: &str, offset: usize, width: usize, is64: bool) {
    if is64 {
        // 64-bit backing store: Int64 arithmetic.
        let mask: i64 = if width >= 64 {
            -1
        } else {
            ((1u64 << width) - 1) as i64
        };
        if width == 1 {
            let _ = writeln!(
                out,
                "  let {field} (v : t) : bool = Int64.logand (Int64.shift_right_logical v.storage {offset}) 1L <> 0L"
            );
            let _ = writeln!(
                out,
                "  let set_{field} (v : t) (b : bool) : unit = if b then v.storage <- Int64.logor v.storage (Int64.shift_left 1L {offset}) else v.storage <- Int64.logand v.storage (Int64.lognot (Int64.shift_left 1L {offset}))"
            );
        } else {
            let _ = writeln!(
                out,
                "  let {field} (v : t) : int64 = Int64.logand (Int64.shift_right_logical v.storage {offset}) {mask:#x}L"
            );
            let _ = writeln!(
                out,
                "  let set_{field} (v : t) (x : int64) : unit = v.storage <- Int64.logor (Int64.logand v.storage (Int64.lognot (Int64.shift_left {mask:#x}L {offset}))) (Int64.shift_left (Int64.logand x {mask:#x}L) {offset})"
            );
        }
    } else {
        let mask: i64 = (1i64 << width) - 1;
        if width == 1 {
            let _ = writeln!(
                out,
                "  let {field} (v : t) : bool = (v.storage lsr {offset}) land 1 <> 0"
            );
            let _ = writeln!(
                out,
                "  let set_{field} (v : t) (b : bool) : unit = if b then v.storage <- v.storage lor (1 lsl {offset}) else v.storage <- v.storage land (lnot (1 lsl {offset}))"
            );
        } else {
            let _ = writeln!(
                out,
                "  let {field} (v : t) : int = (v.storage lsr {offset}) land {mask}"
            );
            let _ = writeln!(
                out,
                "  let set_{field} (v : t) (x : int) : unit = v.storage <- (v.storage land (lnot ({mask} lsl {offset}))) lor ((x land {mask}) lsl {offset})"
            );
        }
    }
}

/// Emits an IDL `bitmask` as an OCaml holder module over its `@bit_bound`
/// backing integer (default 32), with an OR-able manifest constant per bit
/// value and an XCDR2 `marshal`/`unmarshal` writing the backing integer
/// (XTypes 1.3 §7.4.7). Manifest constants are lower-cased (OCaml `let` names
/// must be lowercase-initial), e.g. `PERM_READ` → `Perms.perm_read`.
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (storage, put, get, is64) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let module = module_name(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nmodule {module} = struct");
    let _ = writeln!(out, "  type t = {{ mutable storage : {storage} }}");
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::BeginDeclaration);
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = escape_ocaml_ident(&v.name.text.to_lowercase());
        if is64 {
            let _ = writeln!(out, "  let {cname} : int64 = Int64.shift_left 1L {pos}");
        } else {
            let _ = writeln!(out, "  let {cname} : int = 1 lsl {pos}");
        }
    }
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::EndDeclaration);
    write_bit_module(out, put, get);
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlOcamlError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlOcamlError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlOcamlError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
}

/// OCaml module identifier from an IDL name (upper-cases the first letter).
fn module_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Recursively descends into `Definition::Module`, returning every
/// non-module definition (struct/enum/union/typedef/…) in document order.
/// The IDL AST builder already merges a reopened `module M {} ... module
/// M {}` into one AST node (`crates/idl/src/ast/builder.rs`); this promotes
/// a module's members into the same flat namespace this backend already
/// uses for type-reference resolution — a module's members are promoted to the
/// top level, each paired with its module scope path so the definition and
/// reference sites can flatten each name to `scope_simple` ([`qualify`] /
/// [`resolve_scoped_name`]). Two same-simple-name types in different modules
/// therefore become distinct OCaml modules rather than colliding (#21).
///
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs(defs: &[Definition]) -> Vec<(Vec<String>, &Definition)> {
    let mut out = Vec::new();
    let mut scope = Vec::new();
    flatten_module_defs_into(defs, &mut scope, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs_into<'a>(
    defs: &'a [Definition],
    scope: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, &'a Definition)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                flatten_module_defs_into(&m.definitions, scope, out);
                scope.pop();
            }
            other => out.push((scope.clone(), other)),
        }
    }
}

/// Collects `typedef` aliases (simple declarators) as qualified-name -> aliased
/// type-spec. A typedef is wire-transparent, so members are resolved to the
/// underlying type before mapping (`typedef long Score; Score s;` → `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Typedef(td)) = def {
            for d in &td.declarators {
                if let Declarator::Simple(name) = d {
                    m.insert(qualify(&scope, &name.text), td.type_spec.clone());
                }
            }
        }
    }
    m
}

/// Collects `struct` definitions as qualified-name -> `StructDef`, so a
/// nested-struct `@key` member can be expanded to its own `@key` subset
/// (Bug A) and `keyhash::uses_md5` can resolve a struct-typed `@key` member's
/// size instead of unconditionally forcing the MD5 branch (Bug B).
fn collect_struct_defs(spec: &Specification) -> HashMap<String, &StructDef> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = def {
            m.insert(qualify(&scope, &s.name.text), s);
        }
    }
    m
}

/// Collects a struct's effective members base-first (#A10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated OCaml record and its wire form carry the inherited fields —
/// matching cpp/csharp/java (`resolve_wire_members`). Without this a
/// `struct D : Base` dropped every inherited field from both the type and the
/// wire. An unresolvable base (forward-only) contributes nothing.
/// zerodds-lint: recursion-depth 16 (struct inheritance chain; bounded by the
/// IDL aggregate nesting depth).
fn collect_base_members<'a>(
    s: &'a StructDef,
    structs: &HashMap<String, &'a StructDef>,
    out: &mut Vec<&'a Member>,
) {
    if let Some(base) = &s.base {
        if let Some(bs) = structs.get(&resolve_scoped_name(base)) {
            collect_base_members(bs, structs, out);
        }
    }
    for m in &s.members {
        out.push(m);
    }
}

/// Resolves a typedef chain to its underlying type-spec (recursing into
/// sequence elements). Non-typedef types pass through unchanged.
///
/// zerodds-lint: recursion-depth 32 (typedef alias chains + nested sequence
/// elements; bounded by the IDL's alias/collection nesting depth).
fn resolve_typedef(t: &TypeSpec, typedefs: &HashMap<String, TypeSpec>) -> TypeSpec {
    match t {
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            match typedefs.get(&name) {
                Some(u) => resolve_typedef(u, typedefs),
                None => t.clone(),
            }
        }
        TypeSpec::Sequence(seq) => TypeSpec::Sequence(SequenceType {
            elem: Box::new(resolve_typedef(&seq.elem, typedefs)),
            bound: seq.bound.clone(),
            span: seq.span,
        }),
        other => other.clone(),
    }
}

/// Inline UTF-8→UTF-16-code-unit-count lambda for a `wstring<N>` bound
/// check (B1 follow-up, XTypes 1.3 §7.4.3): mirrors `Wire.put_wstring`'s own
/// unit-counting loop (see `WIRE_MODULE` above) so the check counts the same
/// units the wire actually carries. It reuses the in-tree `Wire.zd_utf8_decode`
/// helper rather than `String.get_utf_8_uchar` / `Uchar.utf_decode_*` (OCaml
/// 4.14+), so the generated code compiles on OCaml 4.13 as well.
const UTF16_UNIT_COUNT_FN: &str = "(fun __zds -> let __zdi = ref 0 in let __zdc = ref 0 in let __zdn = String.length __zds in while !__zdi < __zdn do let (__zdcp, __zdlen) = Wire.zd_utf8_decode __zds !__zdi in __zdi := !__zdi + __zdlen; __zdc := !__zdc + (if __zdcp <= 0xFFFF then 1 else 2) done; !__zdc)";

/// Evaluates an IDL bound (`string<N>` / `sequence<T,N>` / `map<K,V,N>`) to
/// its integer literal for a generated bound-check message/comparison.
/// zerodds-lint: recursion-depth 32
fn bound_literal(e: &ConstExpr, construct: &str) -> Result<i64> {
    array_size(e)
        .ok_or_else(|| IdlOcamlError::Unsupported(format!("non-literal {construct} bound")))
}

/// Evaluates a fixed-array bound to its integer size (literal + unary sign).
/// zerodds-lint: recursion-depth 32
fn array_size(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int(raw),
        ConstExpr::Unary { op, operand, .. } => {
            let v = array_size(operand)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(-v),
                UnaryOp::BitNot => Some(!v),
            }
        }
        _ => None,
    }
}

/// Wraps a per-element put (`$elem`) in nested row-major `for … done` loops over
/// a fixed array `v.<field>.(zdi0).(zdi1)…`.
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!(".(zdi{k})")).collect();
    let mut body = elem_put.replace("$elem", &format!("v.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!("for zdi{k} = 0 to {} do\n{body}\ndone", sizes[k] - 1);
    }
    body
}

/// Maps an IDL union `switch` type to a `TypeSpec` so the discriminator reuses
/// the normal `map_type` path (integer family, char, boolean, or a named enum).
fn switch_typespec(s: &SwitchTypeSpec) -> TypeSpec {
    match s {
        SwitchTypeSpec::Integer(i) => TypeSpec::Primitive(PrimitiveType::Integer(*i)),
        SwitchTypeSpec::Char => TypeSpec::Primitive(PrimitiveType::Char),
        SwitchTypeSpec::Boolean => TypeSpec::Primitive(PrimitiveType::Boolean),
        SwitchTypeSpec::Octet => TypeSpec::Primitive(PrimitiveType::Octet),
        SwitchTypeSpec::Scoped(sn) => TypeSpec::Scoped(sn.clone()),
    }
}

/// How a union `match` observes and compares the discriminator (#A11/A12/A13).
/// The stored `disc` is idiomatically typed, but case labels are integers.
struct DiscMatch {
    /// `Some(<ty>)` for an enum discriminator (matched via `<ty>_to_int`).
    enum_ty: Option<String>,
    is_char: bool,
    is_bool: bool,
    is_i64: bool,
}

impl DiscMatch {
    /// The `match` subject for a discriminator held in `var`, normalized to a
    /// value the integer/`bool` labels compare against.
    fn subject(&self, var: &str) -> String {
        if let Some(ty) = &self.enum_ty {
            format!("({ty}_to_int {var})")
        } else if self.is_char {
            format!("(Char.code {var})")
        } else {
            var.to_string()
        }
    }

    /// Renders a case label `n` in the syntax the match subject expects.
    fn render_label(&self, n: i64) -> String {
        if self.is_bool {
            if n != 0 {
                "true".to_string()
            } else {
                "false".to_string()
            }
        } else if self.is_i64 {
            format!("{n}L")
        } else {
            n.to_string()
        }
    }
}

/// Classifies a union `switch` type for the `match` (#A11/A12/A13). An enum
/// discriminator matches through its `<ty>_to_int`; a 64-bit integer switch
/// needs `Int64` (`<n>L`) labels; `char`/`bool` map to their OCaml forms.
fn disc_match_of(s: &SwitchTypeSpec, enum_names: &HashSet<String>) -> DiscMatch {
    let mut d = DiscMatch {
        enum_ty: None,
        is_char: false,
        is_bool: false,
        is_i64: false,
    };
    match s {
        SwitchTypeSpec::Char => d.is_char = true,
        SwitchTypeSpec::Boolean => d.is_bool = true,
        SwitchTypeSpec::Integer(
            IntegerType::LongLong
            | IntegerType::ULongLong
            | IntegerType::Int64
            | IntegerType::UInt64,
        ) => d.is_i64 = true,
        SwitchTypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                d.enum_ty = Some(type_ident(&name));
            }
        }
        _ => {}
    }
    d
}

/// A generated union case: integer labels (empty + is_default = `default`), the
/// member field name, its language type, and the per-member put statement.
struct UnionCase {
    labels: Vec<i64>,
    is_default: bool,
    field: String,
    ty: String,
    put: String,
    get: String,
}

fn emit_struct(
    out: &mut String,
    s: &StructDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility(s);

    struct FieldGen {
        name: String,
        ocaml_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved_type: TypeSpec,
        array_sizes: Option<Vec<i64>>,
        // `@optional`: the member is a `'a option`. `opt_put` is the value put
        // with `zdOpt` (the `Some`-bound value) as the expression; `get` stays
        // the bare value read, wrapped in `Some`/`None` at the emit site.
        optional: bool,
        opt_put: String,
        // `@must_understand`: sets EMHEADER bit 31 in the `@mutable` framing
        // (#A17). Wire-neutral for `@final`/`@appendable`.
        must_understand: bool,
    }
    // #A10/P3: base-first effective member list — a `struct D : Base` carries
    // Base's members (recursively) ahead of its own, in the OCaml record type
    // and on the wire, matching cpp/csharp/java (`resolve_wire_members`).
    // Without this the inherited members were dropped from both.
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, struct_defs, &mut all_members);

    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &all_members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        let optional = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional))
        });
        let must_understand = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand))
        });
        if optional && key {
            // `@key` participates in the KeyHash unconditionally; an optional
            // key has no coherent presence semantics there — rejected loudly.
            return Err(IdlOcamlError::Unsupported(
                "@optional combined with @key".to_string(),
            ));
        }
        for d in &m.declarators {
            let name = escape_ocaml_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let mut array_sizes: Option<Vec<i64>> = None;
            let mut opt_put = String::new();
            let (ocaml_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, p) =
                        map_type(&resolved, &format!("v.{name}"), enum_names, struct_names)?;
                    let g = map_get(&resolved, enum_names, struct_names)?;
                    if optional {
                        // Value put with the `Some`-bound value as expression
                        // (XTypes 1.3 §7.4.5.1.4: uint8 present-flag then value).
                        let (_, op) = map_type(&resolved, "zdOpt", enum_names, struct_names)?;
                        opt_put = op;
                        (format!("{t} option"), p, g)
                    } else {
                        (t, p, g)
                    }
                }
                // Fixed array: elements inline, row-major, no length prefix.
                Declarator::Array(ad) => {
                    if optional {
                        // `@optional` on an array member: no reference backend
                        // covers it and its option-wrapping interacts with the
                        // row-major loop; rejected loudly rather than mis-emitted.
                        return Err(IdlOcamlError::Unsupported(format!(
                            "@optional on array member `{name}`"
                        )));
                    }
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlOcamlError::Unsupported(format!(
                                "non-literal array size on `{name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let ocaml_type = sizes
                        .iter()
                        .fold(elem_type.clone(), |inner, _| format!("{inner} array"));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, enum_names, struct_names)?;
                    let get = build_array_get(&sizes, &elem_type, &elem_get)?;
                    array_sizes = Some(sizes);
                    (ocaml_type, put, get)
                }
            };
            fields.push(FieldGen {
                name,
                ocaml_type,
                put,
                get,
                id,
                key,
                resolved_type: resolved.clone(),
                array_sizes,
                optional,
                opt_put,
                must_understand,
            });
        }
    }

    let module = module_name(&qualify(scope, &s.name.text));
    let _ = writeln!(out, "\nmodule {module} = struct");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &s.annotations, PlacementKind::BeginDeclaration);
    // #A15/A16: an OCaml record type must have at least one field — a struct
    // with no (effective) members is `unit`, not the illegal `type t = {}`.
    if fields.is_empty() {
        let _ = writeln!(out, "  type t = unit");
    } else {
        let _ = writeln!(out, "  type t = {{");
        for f in &fields {
            let _ = writeln!(out, "    {} : {};", f.name, f.ocaml_type);
        }
        let _ = writeln!(out, "  }}");
    }

    // marshal_into writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(
        out,
        "\n  let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="
    );
    let _ = writeln!(out, "    ignore endian;");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id, plus the must-understand bit 31 — #A17) + NEXTINT (body
        // length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "    let body = Wire.writer endian in");
        for f in &fields {
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): emit its EMHEADER+body only for `Some`.
            let (open, put_src, close) = if f.optional {
                (
                    format!("    (match v.{} with Some zdOpt ->\n", f.name),
                    f.opt_put.clone(),
                    "     | None -> ());".to_string(),
                )
            } else {
                (String::new(), f.put.clone(), String::new())
            };
            out.push_str(&open);
            // #A17: `@must_understand` sets EMHEADER bit 31 (0x8000_0000). The
            // LC field (bits 28-30) stays LC4 = 0x4000_0000 (byte-identity
            // golden — coordinated cross-backend, out of scope here).
            let mu = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu | 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "    Wire.put_u32 body 0x{emh:08x};");
            let _ = writeln!(out, "    let zdMem = Wire.writer endian in");
            let _ = writeln!(out, "    {};", put_src.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "    Wire.put_u32 body (Bytes.length (Wire.bytes zdMem));"
            );
            let _ = writeln!(out, "    Wire.put_bytes body (Wire.bytes zdMem);");
            if !close.is_empty() {
                let _ = writeln!(out, "{close}");
            }
        }
        let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length (Wire.bytes body));");
        let _ = writeln!(out, "    Wire.put_bytes w (Wire.bytes body)");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "    let body = Wire.writer endian in");
            "body"
        };
        for f in &fields {
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4).
                let op = f.opt_put.replace("$w", wv);
                let _ = writeln!(
                    out,
                    "    (match v.{name} with Some zdOpt -> Wire.put_u8 {wv} 1; {op} | None -> Wire.put_u8 {wv} 0);",
                    name = f.name
                );
            } else {
                let _ = writeln!(out, "    {};", f.put.replace("$w", wv));
            }
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "    let bb = Wire.bytes body in");
            let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length bb);");
            let _ = writeln!(out, "    Wire.put_bytes w bb");
        } else {
            let _ = writeln!(out, "    ()");
        }
    }

    let _ = writeln!(
        out,
        "\n  let marshal (v : t) (endian : Wire.endian) : bytes ="
    );
    let _ = writeln!(out, "    let w = Wire.writer endian in");
    let _ = writeln!(out, "    marshal_into v w endian;");
    let _ = writeln!(out, "    Wire.bytes w");
    let mut zdkeys: Vec<&FieldGen> = fields.iter().filter(|f| f.key).collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        let key_members: Vec<&Member> = s
            .members
            .iter()
            .filter(|m| {
                lower_annotations(&m.annotations)
                    .map(|l| l.has_key())
                    .unwrap_or(false)
            })
            .collect();
        let use_md5 = zerodds_idl::keyhash::uses_md5(&key_members, struct_defs, typedefs);
        let _ = writeln!(out, "\n  let key_hash (v : t) : bytes =");
        let _ = writeln!(out, "    let kw = Wire.writer Wire.BE in");
        for f in &zdkeys {
            // Bug A: a struct-typed `@key` member must expand to that
            // struct's own `@key` subset (or ALL its members if it declares
            // none — XTypes 1.3 §7.6.8), not the full member set that
            // `f.put` (shared with normal, non-key encoding) would emit via
            // `marshal_into`. Non-struct key fields are unaffected: `key_put`
            // falls back to the same `map_type` put used for `f.put`.
            let is_struct_key = matches!(&f.resolved_type, TypeSpec::Scoped(sn)
                if struct_defs.contains_key(&resolve_scoped_name(sn)));
            let put = if is_struct_key {
                match &f.array_sizes {
                    None => key_put(
                        &format!("v.{}", f.name),
                        &f.resolved_type,
                        enum_names,
                        struct_names,
                        struct_defs,
                        typedefs,
                    )?,
                    Some(sizes) => {
                        let elem_put = key_put(
                            "$elem",
                            &f.resolved_type,
                            enum_names,
                            struct_names,
                            struct_defs,
                            typedefs,
                        )?;
                        build_array_put(&f.name, sizes, &elem_put)
                    }
                }
            } else {
                f.put.clone()
            };
            let _ = writeln!(out, "    {};", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "    let b = Wire.bytes kw in");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "    Bytes.of_string (Digest.bytes b)");
        } else {
            let _ = writeln!(out, "    let outk = Bytes.make 16 '\\000' in");
            let _ = writeln!(out, "    Bytes.blit b 0 outk 0 (min 16 (Bytes.length b));");
            let _ = writeln!(out, "    outk");
        }
    }

    // Decode (inverse of marshal_into). Records are immutable, so each field is
    // read (in order) into a `let` binding and the record is built at the end.
    // @final reads inline, @appendable skips the DHEADER, @mutable skips DHEADER
    // then per member EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\n  let read (r : Wire.reader) : t =");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
        for f in &fields {
            let g = f.get.replace("$r", "r");
            // @mutable @optional decode rides the naive member-order decoder:
            // an absent member is omitted on encode, but this reader assumes a
            // present member per declared field (documented gap — worklist
            // "mutable-optional decode may ride the naive decoder"). Present
            // members round-trip; absent ones are not recovered.
            let g = if f.optional { format!("Some ({g})") } else { g };
            let _ = writeln!(
                out,
                "    let {} = (ignore (Wire.get_u32 r); ignore (Wire.get_u32 r); {g}) in",
                f.name
            );
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
        }
        for f in &fields {
            let g = f.get.replace("$r", "r");
            // @optional (final/appendable): read the uint8 presence flag, then
            // the value only if present (§7.4.5.1.4).
            let g = if f.optional {
                format!("(if Wire.get_bool r then Some ({g}) else None)")
            } else {
                g
            };
            let _ = writeln!(out, "    let {} = {} in", f.name, g);
        }
    }
    // #A15: an empty (member-less) struct is `unit`, so the decoded value is
    // `()` rather than the illegal empty record literal `{ }`.
    if fields.is_empty() {
        let _ = writeln!(out, "    ()");
    } else {
        let rec_fields = fields
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(out, "    {{ {rec_fields} }}");
    }
    let _ = writeln!(
        out,
        "\n  let unmarshal (b : bytes) (endian : Wire.endian) : t ="
    );
    let _ = writeln!(out, "    read (Wire.reader b endian)");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    Ok(())
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: DHEADER-framed body.
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    // Member references resolve against this union's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    if ext == ExtensibilityKind::Mutable {
        return Err(IdlOcamlError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "v.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(&switch_typespec(&u.switch_type), enum_names, struct_names)?;

    // How the OCaml `match` observes the discriminator (#A11/A12/A13). The
    // stored `disc` is idiomatically typed (enum variant / `char` / `bool` /
    // `int` / `int64`), but the case labels are integers, so the match subject
    // is normalized to a comparable form and each label rendered to match:
    //   enum  → `(<ty>_to_int disc)` vs decimal
    //   char  → `(Char.code disc)`   vs decimal
    //   bool  → `disc`               vs `true`/`false`
    //   i64   → `disc`               vs `<n>L`
    //   int   → `disc`               vs decimal
    let disc = disc_match_of(&u.switch_type, enum_names);

    // #P4: when the discriminator is an enum, resolve `case ENUMERATOR:` labels
    // to their integer value via the switch enum's enumerator table.
    let enum_vals: HashMap<String, i64> = match &u.switch_type {
        SwitchTypeSpec::Scoped(sn) => enum_defs
            .get(&resolve_scoped_name(sn))
            .map(|e| {
                e.enumerators
                    .iter()
                    .zip(enumerator_values(e))
                    .map(|(en, v)| (en.name.text.clone(), i64::from(v)))
                    .collect()
            })
            .unwrap_or_default(),
        _ => HashMap::new(),
    };

    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_ocaml_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlOcamlError::Unsupported(format!(
                            "non-integer union label in `{}`",
                            u.name.text
                        ))
                    })?)
                }
            }
        }
        cases.push(UnionCase {
            labels,
            is_default,
            field,
            ty,
            put,
            get,
        });
    }
    let has_default = cases.iter().any(|c| c.is_default);
    // A `boolean` discriminator covering both `true` and `false` makes the
    // match exhaustive; adding an `| _` fallback there is a dead branch (OCaml
    // warning 11), so it is emitted only when the match is not already total.
    let all_labels: HashSet<i64> = cases
        .iter()
        .flat_map(|c| c.labels.iter().copied())
        .collect();
    let bool_exhaustive = disc.is_bool && all_labels.contains(&0) && all_labels.contains(&1);
    let need_fallback = !has_default && !bool_exhaustive;

    let module = module_name(&qualify(scope, &u.name.text));
    let _ = writeln!(out, "\nmodule {module} = struct");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "  type t = {{");
    let _ = writeln!(out, "    disc : {disc_type};");
    for c in &cases {
        let _ = writeln!(out, "    {} : {};", c.field, c.ty);
    }
    let _ = writeln!(out, "  }}");
    let _ = writeln!(
        out,
        "\n  let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="
    );
    let _ = writeln!(out, "    ignore endian;");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "    let body = Wire.writer endian in");
        "body"
    };
    let _ = writeln!(out, "    {};", disc_put.replace("$w", wv));
    let _ = writeln!(out, "    (match {} with", disc.subject("v.disc"));
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "     | _ -> {}", c.put.replace("$w", wv));
        } else {
            let lbl = c
                .labels
                .iter()
                .map(|n| disc.render_label(*n))
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "     | {lbl} -> {}", c.put.replace("$w", wv));
        }
    }
    if need_fallback {
        let _ = writeln!(out, "     | _ -> ()");
    }
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "    );");
        let _ = writeln!(out, "    let bb = Wire.bytes body in");
        let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length bb);");
        let _ = writeln!(out, "    Wire.put_bytes w bb");
    } else {
        let _ = writeln!(out, "    )");
    }
    let _ = writeln!(
        out,
        "\n  let marshal (v : t) (endian : Wire.endian) : bytes ="
    );
    let _ = writeln!(out, "    let w = Wire.writer endian in");
    let _ = writeln!(out, "    marshal_into v w endian;");
    let _ = writeln!(out, "    Wire.bytes w");

    // Decode: read the discriminator, then build the record reading only the
    // selected member (others get a default). @appendable skips the DHEADER.
    let defaults: Vec<String> = cases
        .iter()
        .map(|c| ocaml_default(&c.ty))
        .collect::<Result<Vec<_>>>()?;
    // Renders the record literal for the branch that selects `sel` (None = the
    // catch-all: every member defaulted).
    let record_for = |sel: Option<&UnionCase>| -> String {
        let body = cases
            .iter()
            .zip(&defaults)
            .map(|(c, def)| {
                let is_sel = sel.is_some_and(|s| s.field == c.field);
                let val = if is_sel {
                    c.get.replace("$r", "r")
                } else {
                    def.clone()
                };
                format!("{} = {val}", c.field)
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!("{{ disc; {body} }}")
    };
    let _ = writeln!(out, "\n  let read (r : Wire.reader) : t =");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
    }
    let _ = writeln!(out, "    let disc = {} in", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "    (match {} with", disc.subject("disc"));
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "     | _ -> {}", record_for(Some(c)));
        } else {
            let lbl = c
                .labels
                .iter()
                .map(|n| disc.render_label(*n))
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "     | {lbl} -> {}", record_for(Some(c)));
        }
    }
    if need_fallback {
        let _ = writeln!(out, "     | _ -> {}", record_for(None));
    }
    let _ = writeln!(out, "    )");
    let _ = writeln!(
        out,
        "\n  let unmarshal (b : bytes) (endian : Wire.endian) : t ="
    );
    let _ = writeln!(out, "    read (Wire.reader b endian)");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &u.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    Ok(())
}

/// Maps an IDL type to `(OCaml type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive or an enum (i32). Others force a collection DHEADER.
fn is_primitive(t: &TypeSpec, enum_names: &HashSet<String>) -> bool {
    match t {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sn) => {
            let n = resolve_scoped_name(sn);
            enum_names.contains(&n) || is_bit_name(&n)
        }
        _ => false,
    }
}

/// Builds a map put (OCaml assoc list): sort by key, then `u32 count` + key/value
/// pairs (DHEADER-framed unless the key/value pair is primitive). `endian` is in
/// scope in `marshal_into`.
fn build_map_put(
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> Result<String> {
    let sorted = format!("List.sort (fun (a, _) (b, _) -> compare a b) {expr}");
    // Bounded `map<K,V,N>` (DDS-XTypes §7.4.3): reject an over-length map
    // before it hits the wire (B1 follow-up, mirrors the other backends'
    // encode-side map bound check). Checked against the ORIGINAL list
    // (sorting doesn't change its length).
    let bound_check = match bound {
        Some(b) => {
            let n = bound_literal(b, "map")?;
            format!(
                "if List.length {expr} > {n} then failwith \"bounded map length exceeds its IDL bound ({n})\";\n     "
            )
        }
        None => String::new(),
    };
    if prim {
        Ok(format!(
            "({bound_check}let zdSorted = {sorted} in\n     Wire.put_u32 $w (List.length zdSorted);\n     List.iter (fun (zdKk, zdKv) -> {key_put}; {val_put}) zdSorted)"
        ))
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        Ok(format!(
            "({bound_check}let zdSorted = {sorted} in\n     let zdSub = Wire.writer endian in\n     Wire.put_u32 zdSub (List.length zdSorted);\n     List.iter (fun (zdKk, zdKv) -> {kp}; {vp}) zdSorted;\n     let zdBB = Wire.bytes zdSub in\n     Wire.put_u32 $w (Bytes.length zdBB);\n     Wire.put_bytes $w zdBB)"
        ))
    }
}

/// zerodds-lint: recursion-depth 32
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        // Bounded `string<N>` / `wstring<N>` (DDS-XTypes §7.4.3): reject an
        // over-bound value before it hits the wire, like the other backends'
        // encode-side checks (idl-cpp `emit_value_write`, idl-csharp
        // `emit_encode_value`, idl-java `emit_typespec_encode`). Narrow uses
        // UTF-8 byte length (matches the CDR wire, `put_string`'s own length
        // prefix); `wstring<N>` uses the UTF-16 code-unit count (matches
        // `put_wstring`'s own unit count below) via an inline decode loop
        // over `Wire.zd_utf8_decode` (the same OCaml-4.13-safe helper the
        // wire module uses), so the check counts exactly the units the wire
        // carries.
        TypeSpec::String(st) if !st.wide => {
            let put = match &st.bound {
                Some(b) => {
                    let n = bound_literal(b, "string")?;
                    format!(
                        "(if String.length {expr} > {n} then failwith \"bounded string length exceeds its IDL bound ({n})\"); Wire.put_string $w {expr}"
                    )
                }
                None => format!("Wire.put_string $w {expr}"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::String(st) => {
            let put = match &st.bound {
                Some(b) => {
                    let n = bound_literal(b, "wstring")?;
                    format!(
                        "(if {UTF16_UNIT_COUNT_FN} {expr} > {n} then failwith \"bounded wstring length exceeds its IDL bound ({n})\"); Wire.put_wstring $w {expr}"
                    )
                }
                None => format!("Wire.put_wstring $w {expr}"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            expr,
            seq.bound.as_ref(),
            enum_names,
            struct_names,
        ),
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // OCaml field holds the BCD bytes directly; `zd_fixed_enc` (appended
        // prelude) builds them from a decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(("bytes".to_string(), format!("Wire.put_bytes $w {expr}")))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                let ty = type_ident(&name);
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // put_u8/put_u16 mask per octet, matching the int8/int16 path.
                let put = match enum_wire_width(&name) {
                    1 => format!("Wire.put_u8 $w ({ty}_to_int {expr})"),
                    2 => format!("Wire.put_u16 $w ({ty}_to_int {expr})"),
                    _ => format!("Wire.put_u32 $w ({ty}_to_int {expr})"),
                };
                Ok((ty.clone(), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                // A struct/bitset/bitmask reference marshals via its module's
                // `marshal_into` (bitset/bitmask wire = backing int, no DHEADER).
                let m = module_name(&name);
                Ok((
                    format!("{m}.t"),
                    format!("{m}.marshal_into {expr} $w endian"),
                ))
            } else {
                Err(IdlOcamlError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map (assoc list): sorted ascending by key, `u32 count` + key/value
        // pairs (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdKk", enum_names, struct_names)?;
            let (val_type, val_put) = map_type(&m.value, "zdKv", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok((
                format!("({key_type} * {val_type}) list"),
                build_map_put(expr, &key_put, &val_put, prim, m.bound.as_ref())?,
            ))
        }
        other => Err(IdlOcamlError::Unsupported(format!("type {other:?}"))),
    }
}

/// Builds a KeyHash-writer statement (using the `$w` placeholder like
/// `map_type`'s put strings) for one `@key` member value. Reuses the shared
/// per-field `map_type` put for primitive/string/enum/sequence/typedef
/// members — safe, since normal and key encoding agree there. For a
/// struct-typed key member (Bug A: nested-struct `@key` member must not
/// include non-key fields, XTypes 1.3 §7.6.8), does NOT call the struct's
/// full `marshal_into`; instead expands to the struct's own `@key` members
/// (or ALL members if it declares none), in member-id order, recursing for
/// further nesting. Rejects (loud error) an array-typed field found inside a
/// nested-struct key — mirrors the `idl-rust` reference fix
/// (`emit_key_field_write`), which rejects the same shape.
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded
/// by the IDL's aggregate nesting depth).
fn key_put(
    expr: &str,
    type_spec: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    struct_defs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<String> {
    let resolved = resolve_typedef(type_spec, typedefs);
    if let TypeSpec::Scoped(sn) = &resolved {
        let name = resolve_scoped_name(sn);
        if let Some(sd) = struct_defs.get(&name) {
            let nested_keys: Vec<&Member> = sd
                .members
                .iter()
                .filter(|m| {
                    lower_annotations(&m.annotations)
                        .map(|l| l.has_key())
                        .unwrap_or(false)
                })
                .collect();
            let effective: Vec<&Member> = if nested_keys.is_empty() {
                sd.members.iter().collect()
            } else {
                nested_keys
            };
            let mut ordered: Vec<(u32, &Member)> = effective
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let id = lower_annotations(&m.annotations)
                        .ok()
                        .and_then(|l| l.explicit_id())
                        .unwrap_or(i as u32);
                    (id, *m)
                })
                .collect();
            ordered.sort_by_key(|(id, _)| *id);
            let mut stmts: Vec<String> = Vec::new();
            for (_, m) in &ordered {
                for d in &m.declarators {
                    if matches!(d, Declarator::Array(_)) {
                        return Err(IdlOcamlError::Unsupported(
                            "array @key field inside a nested-struct key".to_string(),
                        ));
                    }
                    let field = d.name().text.clone();
                    stmts.push(key_put(
                        &format!("{expr}.{field}"),
                        &m.type_spec,
                        enum_names,
                        struct_names,
                        struct_defs,
                        typedefs,
                    )?);
                }
            }
            return Ok(stmts.join("; "));
        }
    }
    let (_, put) = map_type(&resolved, expr, enum_names, struct_names)?;
    Ok(put)
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("int", format!("Wire.put_u8 $w {expr}")),
        PrimitiveType::Boolean => ("bool", format!("Wire.put_bool $w {expr}")),
        PrimitiveType::Char => ("char", format!("Wire.put_u8 $w (Char.code {expr})")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => {
            ("float", format!("Wire.put_f32 $w {expr}"))
        }
        PrimitiveType::Floating(FloatingType::Double) => {
            ("float", format!("Wire.put_f64 $w {expr}"))
        }
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("float", format!("Wire.put_long_double $w {expr}"))
        }
        PrimitiveType::WideChar => ("int", format!("Wire.put_u32 $w {expr}")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // OCaml's 63-bit int holds every 8/16/32-bit value; 64-bit uses Int64.
    let (ty, put) = match i {
        IntegerType::Int8 | IntegerType::UInt8 => ("int", format!("Wire.put_u8 $w {expr}")),
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            ("int", format!("Wire.put_u16 $w {expr}"))
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            ("int", format!("Wire.put_u32 $w {expr}"))
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => ("int64", format!("Wire.put_u64 $w {expr}")),
    };
    Ok((ty.to_string(), put))
}

/// zerodds-lint: recursion-depth 32
fn map_sequence(
    elem: &TypeSpec,
    expr: &str,
    bound: Option<&ConstExpr>,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    // Bounded `sequence<T,N>` (DDS-XTypes §7.4.3): reject an over-length
    // list before it hits the wire (B1 follow-up, mirrors the other
    // backends' encode-side sequence bound check).
    let bound_check = match bound {
        Some(b) => {
            let n = bound_literal(b, "sequence")?;
            format!(
                "(if List.length {expr} > {n} then failwith \"bounded sequence length exceeds its IDL bound ({n})\"); "
            )
        }
        None => String::new(),
    };
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "bytes".to_string(),
            format!("{bound_check}Wire.put_seq_u8 $w {expr}"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let m = module_name(&name);
            let put = format!(
                "({bound_check}let sub = Wire.writer endian in                  Wire.put_u32 sub (List.length {expr});                  List.iter (fun e -> {m}.marshal_into e sub endian) {expr};                  let bb = Wire.bytes sub in                  Wire.put_u32 $w (Bytes.length bb); Wire.put_bytes $w bb)"
            );
            return Ok((format!("{m}.t list"), put));
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases reached here). Mirrors the
    // `idl-go` / `idl-d` fallback.
    let (elem_ty, elem_put) = map_type(elem, "zdElem", enum_names, struct_names)?;
    let put = format!(
        "({bound_check}Wire.put_u32 $w (List.length {expr}); List.iter (fun zdElem -> {elem_put}) {expr})"
    );
    Ok((format!("{elem_ty} list"), put))
}

// ---- decode (inverse of the put path): a `Wire.reader` wire-core in the module,
// plus `map_get` — the inverse of `map_type` — returning an EXPRESSION that reads
// one value from `$r`. OCaml records are immutable, so each field is read into a
// `let` binding and the record is built at the end. Roundtrip-verified.

/// A zero value of an OCaml type, used to pre-allocate fixed arrays before the
/// row-major fill (the values are all overwritten).
fn ocaml_default(t: &str) -> Result<String> {
    let d = if t == "int" {
        "0"
    } else if t == "int64" {
        "0L"
    } else if t == "float" {
        "0.0"
    } else if t == "bool" {
        "false"
    } else if t == "char" {
        "'\\000'"
    } else if t == "string" {
        "\"\""
    } else if t == "bytes" {
        "Bytes.empty"
    } else if t.ends_with(" array") {
        "[||]"
    } else if t.ends_with(" list") {
        // `sequence<T>` / `map<K,V>` elements — the empty collection.
        "[]"
    } else {
        return Err(IdlOcamlError::Unsupported(format!(
            "no default for array element type `{t}`"
        )));
    };
    Ok(d.to_string())
}

/// Reads a fixed array: allocate the nested `array` then row-major `for` loops
/// filling each element (inverse of [`build_array_put`]). `elem_get` is an
/// expression reading one element from `$r`.
///
/// A non-primitive element type (a nested struct's `M.t`) has no zero literal
/// to pre-fill `Array.make` with (#A27 — the former loud reject). Those are
/// read with nested `Array.init`, which evaluates left-to-right (OCaml ≥ 4.14),
/// so the elements are decoded in the SAME row-major order [`build_array_put`]
/// wrote them — no pre-allocation default is needed.
fn build_array_get(sizes: &[i64], elem_type: &str, elem_get: &str) -> Result<String> {
    let Ok(default) = ocaml_default(elem_type) else {
        // Non-primitive element (struct `M.t`): read directly via nested
        // `Array.init` (row-major, left-to-right — matches the encode order).
        fn init(sizes: &[i64], elem_get: &str) -> String {
            if sizes.len() == 1 {
                format!("Array.init {} (fun _ -> {elem_get})", sizes[0])
            } else {
                format!(
                    "Array.init {} (fun _ -> {})",
                    sizes[0],
                    init(&sizes[1..], elem_get)
                )
            }
        }
        return Ok(format!("({})", init(sizes, elem_get)));
    };
    // Allocation: innermost `Array.make s d`, outer `Array.init s (fun _ -> …)`.
    fn alloc(sizes: &[i64], default: &str) -> String {
        if sizes.len() == 1 {
            format!("Array.make {} {default}", sizes[0])
        } else {
            format!(
                "Array.init {} (fun _ -> {})",
                sizes[0],
                alloc(&sizes[1..], default)
            )
        }
    }
    let idx: String = (0..sizes.len()).map(|k| format!(".(zdi{k})")).collect();
    let mut fill = format!("zda{idx} <- {elem_get}");
    for k in (0..sizes.len()).rev() {
        fill = format!("for zdi{k} = 0 to {} do {fill} done", sizes[k] - 1);
    }
    Ok(format!(
        "(let zda = {} in {fill}; zda)",
        alloc(sizes, &default)
    ))
}

/// Returns an expression reading one value of IDL type `t` from `$r`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p),
        // Bounded `string<N>` / `wstring<N>` decode (B1 follow-up, XTypes
        // 1.3 §7.4.3): mirror the encode-side check above on decode too — a
        // well-formed-but-oversized wire value must be rejected, not just
        // whatever the wire's own remaining-byte check already does.
        //
        // Documented tradeoff (deep review of #22 decode-bounds-cross-backend,
        // moderate item): this checks the length AFTER `Wire.get_string`/
        // `Wire.get_wstring` has already materialized the value, not before.
        // That is the SAME order idl-rust's own reference decoder uses
        // (`struct_emit.rs::emit_decode_bound_checks`: "checked post-decode,
        // not pre-encode, since the value already exists in memory by the
        // time this runs") — intentional, not an oversight, for a single
        // primitive-collection read: its cost is bounded by the wire's own
        // remaining-buffer length, not by the attacker-declared bound, so
        // there's no separate amplification to guard against here. Checking
        // BEFORE materializing would require duplicating `Wire.get_string`'s
        // length-prefix-then-copy logic outside the `Wire` module, which is
        // kept byte-identical to the hand-maintained `endpoints/ocaml` copy
        // (see the module's own doc comment) — out of scope for this fix.
        // Contrast `map_get_sequence` below: a `sequence<struct, N>`'s
        // per-element DECODE LOOP is checked BEFORE it runs (right after
        // reading the wire count, before the loop), because there the
        // attacker-controlled count drives repeated allocation/decode work,
        // not a single bounded read.
        TypeSpec::String(st) if !st.wide => match &st.bound {
            Some(b) => {
                let n = bound_literal(b, "string")?;
                Ok(format!(
                    "(let __zdv = Wire.get_string $r in if String.length __zdv > {n} then failwith \"decoded string length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_string $r)".to_string()),
        },
        TypeSpec::String(st) => match &st.bound {
            Some(b) => {
                let n = bound_literal(b, "wstring")?;
                Ok(format!(
                    "(let __zdv = Wire.get_wstring $r in if {UTF16_UNIT_COUNT_FN} __zdv > {n} then failwith \"decoded wstring length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_wstring $r)".to_string()),
        },
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), enum_names, struct_names)
        }
        // `fixed<P,S>`: read the statically-known `(P+2)/2` packed-BCD octets
        // (no length prefix, no alignment).
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("(Wire.get_bytes_n $r {n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                let ty = type_ident(&name);
                // Read the @bit_bound-wide holder (XTypes 1.3 §7.4.5.1); mirrors
                // the backend's int8/int16 reads (get_u8/get_u16).
                let get = match enum_wire_width(&name) {
                    1 => format!("({ty}_of_int (Wire.get_u8 $r))"),
                    2 => format!("({ty}_of_int (Wire.get_u16 $r))"),
                    _ => format!("({ty}_of_int (Wire.get_u32 $r))"),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                let m = module_name(&name);
                Ok(format!("({m}.read $r)"))
            } else {
                Err(IdlOcamlError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let key_get = map_get(&m.key, enum_names, struct_names)?;
            let val_get = map_get(&m.value, enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim {
                ""
            } else {
                "ignore (Wire.get_u32 $r); "
            };
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // map bound check on decode — XTypes 1.3 §7.4.3.
            let bound_check = match &m.bound {
                Some(b) => {
                    let n = bound_literal(b, "map")?;
                    format!(
                        "if zdn > {n} then failwith \"decoded map length exceeds its IDL bound ({n})\"; "
                    )
                }
                None => String::new(),
            };
            Ok(format!(
                "({dh}let zdn = Wire.get_u32 $r in {bound_check}let rec zdloop k acc = if k = 0 then List.rev acc else (let zdk = {key_get} in let zdv = {val_get} in zdloop (k - 1) ((zdk, zdv) :: acc)) in zdloop zdn [])"
            ))
        }
        other => Err(IdlOcamlError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType) -> Result<String> {
    let e = match p {
        PrimitiveType::Octet => "(Wire.get_u8 $r)",
        PrimitiveType::Char => "(Char.chr (Wire.get_u8 $r))",
        PrimitiveType::Boolean => "(Wire.get_bool $r)",
        PrimitiveType::Integer(i) => return map_get_integer(i),
        PrimitiveType::Floating(FloatingType::Float) => "(Wire.get_f32 $r)",
        PrimitiveType::Floating(FloatingType::Double) => "(Wire.get_f64 $r)",
        PrimitiveType::Floating(FloatingType::LongDouble) => "(Wire.get_long_double $r)",
        PrimitiveType::WideChar => "(Wire.get_u32 $r)",
    };
    Ok(e.to_string())
}

fn map_get_integer(i: IntegerType) -> Result<String> {
    let e = match i {
        IntegerType::Int8 | IntegerType::UInt8 => "(Wire.get_u8 $r)",
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            "(Wire.get_u16 $r)"
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            "(Wire.get_u32 $r)"
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => "(Wire.get_u64 $r)",
    };
    Ok(e.to_string())
}

/// zerodds-lint: recursion-depth 32
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    // B1 follow-up (#22 decode-side parity): mirror the encode-side
    // sequence bound check on decode — XTypes 1.3 §7.4.3.
    //
    // Documented tradeoff (moderate item, deep review of #22): like the
    // narrow string/wstring case in `map_get` above, this octet sequence
    // checks AFTER `Wire.get_seq_u8` has already read the bytes — a single
    // bounded-by-remaining-buffer read, not a decode loop, so there's no
    // amplification vector (matches idl-rust's own post-decode-check
    // convention). The struct-element branch below is different: its count
    // drives a decode LOOP, so it's checked BEFORE the loop starts.
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return match bound {
            Some(b) => {
                let n = bound_literal(b, "sequence")?;
                Ok(format!(
                    "(let __zdv = Wire.get_seq_u8 $r in if Bytes.length __zdv > {n} then failwith \"decoded sequence length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_seq_u8 $r)".to_string()),
        };
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let m = module_name(&name);
            // Checked BEFORE `zdloop` runs (right after reading the wire
            // count `zdn`, before any element is decoded) — an
            // attacker-supplied huge `zdn` must not drive an unbounded
            // decode loop before the bound is ever checked.
            let bound_check = match bound {
                Some(b) => {
                    let n = bound_literal(b, "sequence")?;
                    format!(
                        "if zdn > {n} then failwith \"decoded sequence length exceeds its IDL bound ({n})\"; "
                    )
                }
                None => String::new(),
            };
            return Ok(format!(
                "(ignore (Wire.get_u32 $r); let zdn = Wire.get_u32 $r in {bound_check}let rec zdloop k acc = if k = 0 then List.rev acc else (let e = {m}.read $r in zdloop (k - 1) (e :: acc)) in zdloop zdn [])"
            ));
        }
    }
    // sequence<arbitrary> decode → u32 count + per-element read (no collection
    // DHEADER; inverse of `map_sequence`'s arbitrary encode fallback). Bound
    // checked BEFORE the loop, since the wire count drives it.
    let bound_check = match bound {
        Some(b) => {
            let n = bound_literal(b, "sequence")?;
            format!(
                "if zdn > {n} then failwith \"decoded sequence length exceeds its IDL bound ({n})\"; "
            )
        }
        None => String::new(),
    };
    let elem_get = map_get(elem, enum_names, struct_names)?;
    Ok(format!(
        "(let zdn = Wire.get_u32 $r in {bound_check}let rec zdloop k acc = if k = 0 then List.rev acc else (let zde = {elem_get} in zdloop (k - 1) (zde :: acc)) in zdloop zdn [])"
    ))
}
