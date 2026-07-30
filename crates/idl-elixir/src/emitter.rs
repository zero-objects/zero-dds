// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Elixir emitter. Walks the `zerodds-idl` AST and emits a
//! self-contained Elixir source file: a `<Pkg>.Wire` module (byte-identical to
//! `endpoints/elixir`) plus, per IDL `struct`, a `<Pkg>.<Name>` module with
//! `defstruct` and a `marshal_xcdr(v, endian)` function. `@final` and
//! `@appendable` are supported; other extensibilities and constructs raise
//! [`IdlElixirError::Unsupported`].

use std::fmt::Write as _;

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{
    Annotation, BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstrTypeDecl,
    Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType, IntegerType, InterfaceDcl,
    Literal, LiteralKind, Member, PrimitiveType, ScopedName, SequenceType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlElixirError, Result};
use crate::keywords::escape_elixir_ident;

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
    /// reference to one of these maps to an Elixir holder module whose wire form
    /// is a single backing integer (`marshal_into`/`read`) — no collection
    /// DHEADER, so it is treated as fully-descriptive (primitive) by the
    /// sequence/map framing rules (XTypes 1.3 §7.4.7).
    static BIT_NAMES: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());

    /// Set whenever a `fixed<P,S>` member is emitted, so the BCD prelude module
    /// is appended exactly once (and only when needed).
    static USED_FIXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// Flattened qualified enum name → signed wire holder width in OCTETS
    /// (1/2/4), from `@bit_bound` (XTypes 1.3 §7.3.1.2.1.9 + §7.4.5.1) via the
    /// shared [`enum_wire_octets`]. Populated once per run; read at the single
    /// enum encode/decode site so a `@bit_bound(8)`/`@bit_bound(16)` enum
    /// narrows to 1/2 bytes instead of the former fixed 4.
    static ENUM_WIDTHS: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());

    /// Every `const` value expression, keyed by simple name, so a named
    /// collection bound (`sequence<octet, MAX>`, `long a[LEN]`) or a named
    /// `case` label resolves to its folded integer (IDL 4.2 §7.4.1.4.4
    /// const_expr). Populated once per run by [`register_const_values`].
    static CONST_VALUES: std::cell::RefCell<HashMap<String, ConstExpr>> =
        std::cell::RefCell::new(HashMap::new());

    /// Every enumerator's integer value, keyed by simple name, so a named bound
    /// or `case` label that references an enumerator folds to its discriminant.
    static ENUM_LITERAL_VALUES: std::cell::RefCell<HashMap<String, i64>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Registers every `const` value expression and every enumerator value in the
/// spec into [`CONST_VALUES`] / [`ENUM_LITERAL_VALUES`] (keyed by simple name),
/// so [`eval_const_int`] can resolve a named collection bound or a named union
/// `case` label. Recurses into modules and interface bodies. Mirrors idl-zig's
/// `register_const_values`.
/// zerodds-lint: recursion-depth 16 (module/interface nesting; bounded by grammar).
fn register_const_values(defs: &[Definition]) {
    for def in defs {
        match def {
            Definition::Const(c) => {
                CONST_VALUES.with(|m| {
                    m.borrow_mut().insert(c.name.text.clone(), c.value.clone());
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                for (en, val) in e.enumerators.iter().zip(enumerator_values(e)) {
                    ENUM_LITERAL_VALUES.with(|m| {
                        m.borrow_mut().insert(en.name.text.clone(), i64::from(val));
                    });
                }
            }
            Definition::Module(m) => register_const_values(&m.definitions),
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                for ex in &iface.exports {
                    match ex {
                        Export::Const(c) => {
                            CONST_VALUES.with(|m| {
                                m.borrow_mut().insert(c.name.text.clone(), c.value.clone());
                            });
                        }
                        Export::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                            for (en, val) in e.enumerators.iter().zip(enumerator_values(e)) {
                                ENUM_LITERAL_VALUES.with(|m| {
                                    m.borrow_mut().insert(en.name.text.clone(), i64::from(val));
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Evaluates a constant expression to a signed integer, resolving named
/// constants (via [`CONST_VALUES`]), enumerator names (via
/// [`ENUM_LITERAL_VALUES`] or `locals`) and folding IDL arithmetic/bitwise
/// operators (IDL 4.2 §7.4.1.4.4). Mirrors idl-rust's `eval_const_i128` /
/// idl-zig's `eval_const_int` so a bound or `case` label evaluates to the SAME
/// integer in every backend. `locals` supplies the switch enum's enumerators
/// (union path); `depth` bounds const-reference chains.
/// zerodds-lint: recursion-depth 32 (const-reference chain; explicitly bounded).
fn eval_const_int(e: &ConstExpr, locals: &HashMap<String, i64>, depth: u32) -> Option<i64> {
    if depth > 32 {
        return None;
    }
    match e {
        ConstExpr::Literal(Literal { kind, raw, .. }) => match kind {
            LiteralKind::Integer => parse_int(raw),
            LiteralKind::Char | LiteralKind::WideChar => char_literal_value(raw),
            LiteralKind::Boolean => Some(i64::from(raw.trim().eq_ignore_ascii_case("true"))),
            _ => None,
        },
        // A named enumerator or constant, resolved by its simple (last) segment:
        // the switch enum's enumerators first (union `case` path), then the
        // spec-wide enumerator set, then a named `const` (recursively evaluated).
        ConstExpr::Scoped(sn) => {
            let last = sn.parts.last()?.text.clone();
            if let Some(v) = locals.get(&last) {
                return Some(*v);
            }
            if let Some(v) = ENUM_LITERAL_VALUES.with(|m| m.borrow().get(&last).copied()) {
                return Some(v);
            }
            let value = CONST_VALUES.with(|m| m.borrow().get(&last).cloned())?;
            eval_const_int(&value, locals, depth + 1)
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_const_int(operand, locals, depth + 1)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => v.checked_neg(),
                UnaryOp::BitNot => Some(!v),
            }
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let a = eval_const_int(lhs, locals, depth + 1)?;
            let b = eval_const_int(rhs, locals, depth + 1)?;
            match op {
                BinaryOp::Or => Some(a | b),
                BinaryOp::Xor => Some(a ^ b),
                BinaryOp::And => Some(a & b),
                BinaryOp::Shl => u32::try_from(b).ok().map(|s| a << s),
                BinaryOp::Shr => u32::try_from(b).ok().map(|s| a >> s),
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                BinaryOp::Mod => a.checked_rem(b),
            }
        }
    }
}

/// Signed wire holder width in octets (1/2/4) an enum named `name` serializes
/// at, per its `@bit_bound`. Defaults to 4 for an unregistered name / no
/// `@bit_bound` (XTypes 1.3 §7.4.5.1 default bound 32).
fn enum_wire_width(name: &str) -> u32 {
    ENUM_WIDTHS
        .with(|m| m.borrow().get(name).copied())
        .unwrap_or(4)
}

/// Elixir codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`verbatims_for_language`](zerodds_idl::semantics::annotations::Lowered::verbatims_for_language)).
const ELIXIR_LANG_ALIASES: &[&str] = &["elixir", "ex", "exs"];

/// BCD codec module for `fixed<P,S>`, appended once when any `fixed` member is
/// emitted. `enc/3` builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡
/// XCDR2 §7.4.4.5) from a decimal string: an optional leading pad nibble (so the
/// nibble count is even), `P` digit nibbles most-significant first, then the
/// sign nibble (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length
/// prefix. `__PKG__` is replaced with the package module name.
const FIXED_MODULE: &str = r#"
defmodule __PKG__.Fixed do
  @moduledoc false
  # Packed-BCD encoder for `fixed<P,S>`: returns the raw `(P+2)/2` octets that
  # the generated struct stores and writes verbatim (no length prefix).
  def enc(s, p, sc) do
    {sign, rest} =
      case s do
        <<"-", r::binary>> -> {false, r}
        <<"+", r::binary>> -> {true, r}
        _ -> {true, s}
      end

    {ip, fp} =
      case :binary.split(rest, ".") do
        [i, f] -> {i, f}
        [i] -> {i, ""}
      end

    ip = String.pad_leading(ip, p - sc, "0")
    fp = String.pad_trailing(fp, sc, "0")
    digits = ip <> fp

    pad = if rem(p + 1, 2) == 1, do: [0], else: []
    nibs = pad ++ (for <<c <- digits>>, do: c - ?0) ++ [if(sign, do: 0x0C, else: 0x0D)]

    nibs
    |> Enum.chunk_every(2)
    |> Enum.map(fn [hi, lo] -> <<hi::4, lo::4>> end)
    |> IO.iodata_to_binary()
  end
end
"#;

/// Emits every `@verbatim` block from `anns` whose language matches the Elixir
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-d`'s
/// `emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(ELIXIR_LANG_ALIASES) {
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

/// Upper-cases the first character (Elixir module alias segments must start with
/// an uppercase letter — an IDL `module a` scope part is lowercase).
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Collision-free flattened alias segment for a declaration `simple` in module
/// `scope`. Elixir module segments must be uppercase-initial, so the flattened
/// `scope_simple` name is capitalized (a plain lowercase `a_Reading` would be an
/// invalid alias). The bare `simple` is kept at global scope (already
/// CamelCase by IDL convention), preserving every existing top-level golden.
/// Two same-simple-name types in different modules become distinct modules
/// `<Pkg>.A_Reading`/`<Pkg>.B_Reading` (#21).
fn qualify(scope: &[String], simple: &str) -> String {
    if scope.is_empty() {
        // Capitalize even a global-scope name: an Elixir module alias segment
        // must start uppercase, so a lowercase or reserved-word-derived IDL type
        // name (`struct end`, `const max`) would otherwise flatten to an invalid
        // `Zdgen.end`/`Zdgen.max`. Idempotent on the CamelCase names IDL uses by
        // convention, so every existing top-level golden is unchanged.
        capitalize_first(simple)
    } else {
        let mut parts = scope.to_vec();
        parts.push(simple.to_string());
        capitalize_first(&flatten_path(&parts))
    }
}

/// Injectively flattens a module-qualified path (`["a", "b", "C"]`) into a
/// single flattened alias segment. Each segment's own underscores are doubled
/// and the segments joined by a single underscore, so `module A_B { struct C }`
/// (`["A_B","C"]` → `A__B_C`) never collides with `module A { module B {
/// struct C }}` (`["A","B","C"]` → `A_B_C`) — the previous `join("_")` mapped
/// both to `A_B_C` (#A35, non-injective flatten). A single (global-scope)
/// segment is returned verbatim so every existing top-level golden is
/// unchanged, and any segment without underscores (the common case) passes
/// through untouched. Mirrors `idl-go`'s `flatten_path`.
fn flatten_path(parts: &[String]) -> String {
    if parts.len() <= 1 {
        return parts.first().cloned().unwrap_or_default();
    }
    parts
        .iter()
        .map(|p| p.replace('_', "__"))
        .collect::<Vec<_>>()
        .join("_")
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
            // must be registered the same way.
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

/// Registers the fully-qualified path of a single `TypeDecl` (module-level or
/// interface-nested).
fn register_type_decl_path(td: &TypeDecl, scope: &[String]) {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            push_type_path(scope, &s.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
            push_type_path(scope, &e.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            push_type_path(scope, &u.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => {
            push_type_path(scope, &b.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => {
            push_type_path(scope, &b.name.text);
        }
        TypeDecl::Typedef(td) => {
            for d in &td.declarators {
                push_type_path(scope, &d.name().text);
            }
        }
        _ => {}
    }
}

fn push_type_path(scope: &[String], simple: &str) {
    let mut path = scope.to_vec();
    path.push(simple.to_string());
    TYPE_PATHS.with(|t| t.borrow_mut().push(path));
}

/// Resolves a referenced `ScopedName` against [`CURRENT_SCOPE`], returning the
/// flattened alias segment (same shape as [`qualify`]) of the matching
/// declaration. Mirrors IDL name lookup (§7.5.2): for each prefix of the
/// enclosing scope (longest first), then the global scope, check whether
/// `prefix + parts` is a known type path. Falls back to the literal flattening.
fn resolve_scoped_name(sn: &ScopedName) -> String {
    let parts: Vec<String> = sn.parts.iter().map(|p| p.text.clone()).collect();
    let scope = CURRENT_SCOPE.with(|s| s.borrow().clone());
    let known: Vec<Vec<String>> = TYPE_PATHS.with(|t| t.borrow().clone());
    for cut in (0..=scope.len()).rev() {
        let mut cand = scope[..cut].to_vec();
        cand.extend(parts.iter().cloned());
        if known.contains(&cand) {
            // Match the definition-site flattening (`qualify`), which
            // capitalizes at every scope depth (including global).
            return capitalize_first(&flatten_path(&cand));
        }
    }
    capitalize_first(&flatten_path(&parts))
}

/// Options for the Elixir backend.
#[derive(Debug, Clone)]
pub struct ElixirGenOptions {
    /// The top-level Elixir module namespace (CamelCase).
    pub module_name: String,
}

impl Default for ElixirGenOptions {
    fn default() -> Self {
        Self {
            module_name: "Zdgen".to_string(),
        }
    }
}

/// The `<Pkg>.Wire` module, byte-identical to `endpoints/elixir`'s ZeroDDS.Wire.
/// `__PKG__` is replaced with the package module name.
const WIRE_MODULE: &str = r#"defmodule __PKG__.Wire do
  @moduledoc false
  # Wire representation: xcdr1=false → XCDR2 (max alignment 4, DHEADER-framed
  # appendable/mutable); xcdr1=true → XCDR1 / classic CDR (max alignment 8, no
  # DHEADER, PL_CDR1 @mutable). `subwriter/1` inherits the mode so a body /
  # member sub-writer stays in the same representation as its parent.
  def writer(endian), do: %{buf: <<>>, endian: endian, xcdr1: false}
  def writer1(endian), do: %{buf: <<>>, endian: endian, xcdr1: true}
  def subwriter(w), do: %{buf: <<>>, endian: w.endian, xcdr1: w.xcdr1}

  defp align(w, a) do
    m = if w.xcdr1, do: 8, else: 4
    cap = min(a, m)
    pad = rem(cap - rem(byte_size(w.buf), cap), cap)
    %{w | buf: w.buf <> :binary.copy(<<0>>, pad)}
  end

  defp put(w, a, le) do
    w = align(w, a)
    bytes = if w.endian == :big, do: rev(le), else: le
    %{w | buf: w.buf <> bytes}
  end

  defp rev(bin), do: bin |> :binary.bin_to_list() |> Enum.reverse() |> :binary.list_to_bin()

  def put_u8(w, v), do: %{w | buf: w.buf <> <<v::8>>}
  def put_bool(w, v), do: put_u8(w, if(v, do: 1, else: 0))
  def put_u16(w, v), do: put(w, 2, <<v::little-16>>)
  def put_u32(w, v), do: put(w, 4, <<v::little-32>>)
  # 8-byte primitives request their NATURAL alignment 8; `align/2` caps it at 4
  # under XCDR2 (byte-identical to before) and keeps 8 under XCDR1.
  def put_u64(w, v), do: put(w, 8, <<v::little-64>>)
  def put_f32(w, v), do: put(w, 4, <<v::float-little-32>>)
  def put_f64(w, v), do: put(w, 8, <<v::float-little-64>>)
  def put_bytes(w, b), do: %{w | buf: w.buf <> b}

  # PL_CDR1 (@mutable, XCDR1) member: `[PID][len][body][pad-to-4]`. The PID
  # length carries the UNPADDED body length; member ids >= 0x3F00 or bodies over
  # 0xFFFF use the extended header (PID_EXTENDED, 32-bit id + length). Matches
  # `zerodds_cdr::xcdr1::encode_pl_cdr1_member`.
  def put_pl_cdr1_member(w, id, mbody) do
    bl = byte_size(mbody)
    w =
      if id >= 0x3F00 or bl > 0xFFFF do
        w |> put_u16(0x3F01) |> put_u16(8) |> put_u32(id) |> put_u32(bl)
      else
        w |> put_u16(id) |> put_u16(bl)
      end

    w = put_bytes(w, mbody)
    pad = rem(4 - rem(bl, 4), 4)
    put_bytes(w, :binary.copy(<<0>>, pad))
  end

  # PL_CDR1 sentinel terminator (PID_LIST_END = 0x3F02, length 0).
  def put_pl_cdr1_sentinel(w), do: w |> put_u16(0x3F02) |> put_u16(0)

  def put_string(w, s) do
    w = put_u32(w, byte_size(s) + 1)
    w = put_bytes(w, s)
    put_u8(w, 0)
  end

  def put_seq_u8(w, b) do
    w = put_u32(w, byte_size(b))
    put_bytes(w, b)
  end

  def put_wstring(w, s) do
    bin = :unicode.characters_to_binary(s, :utf8, {:utf16, :big})
    units = for <<u::16 <- bin>>, do: u
    w = put_u32(w, length(units) * 2)
    Enum.reduce(units, w, fn u, acc -> put_u16(acc, u) end)
  end

  def put_long_double(w, v) do
    <<sign::1, exp::11, mant::52>> = <<v::float-64>>
    {exp128, mant128} = if exp == 0 and mant == 0, do: {0, 0}, else: {exp - 1023 + 16383, mant}
    be = <<sign::1, exp128::15, mant128::52, 0::60>>
    put(w, 8, rev(be))
  end

  def bytes(w), do: w.buf

  # Reader: {bin, pos, endian}; each get returns {value, reader}. Alignment is
  # stream-relative (tracked via pos), inverse of the Writer's byte_size(buf).
  def reader(bin, endian), do: %{bin: bin, pos: 0, endian: endian, xcdr1: false}
  def reader1(bin, endian), do: %{bin: bin, pos: 0, endian: endian, xcdr1: true}

  defp ralign(r, a) do
    m = if r.xcdr1, do: 8, else: 4
    cap = min(a, m)
    pad = rem(cap - rem(r.pos, cap), cap)
    %{r | pos: r.pos + pad}
  end

  def get_u8(r), do: {:binary.at(r.bin, r.pos), %{r | pos: r.pos + 1}}

  def get_bool(r) do
    {v, r} = get_u8(r)
    {v != 0, r}
  end

  def get_u16(r) do
    r = ralign(r, 2)
    {:binary.decode_unsigned(:binary.part(r.bin, r.pos, 2), r.endian), %{r | pos: r.pos + 2}}
  end

  def get_u32(r) do
    r = ralign(r, 4)
    {:binary.decode_unsigned(:binary.part(r.bin, r.pos, 4), r.endian), %{r | pos: r.pos + 4}}
  end

  def get_u64(r) do
    r = ralign(r, 8)
    {:binary.decode_unsigned(:binary.part(r.bin, r.pos, 8), r.endian), %{r | pos: r.pos + 8}}
  end

  def get_f32(r) do
    {bits, r} = get_u32(r)
    <<v::float-32>> = <<bits::32>>
    {v, r}
  end

  def get_f64(r) do
    {bits, r} = get_u64(r)
    <<v::float-64>> = <<bits::64>>
    {v, r}
  end

  def get_bytes_n(r, n), do: {:binary.part(r.bin, r.pos, n), %{r | pos: r.pos + n}}

  def get_string(r) do
    {n, r} = get_u32(r)
    {:binary.part(r.bin, r.pos, n - 1), %{r | pos: r.pos + n}}
  end

  def get_seq_u8(r) do
    {n, r} = get_u32(r)
    get_bytes_n(r, n)
  end

  def get_wstring(r) do
    {n2, r} = get_u32(r)
    n = div(n2, 2)
    {units, r} =
      Enum.reduce(1..n//1, {[], r}, fn _, {acc, rr} ->
        {u, rr} = get_u16(rr)
        {[u | acc], rr}
      end)

    bin = for u <- Enum.reverse(units), into: <<>>, do: <<u::16-big>>
    {:unicode.characters_to_binary(bin, {:utf16, :big}, :utf8), r}
  end

  def get_long_double(r) do
    r = ralign(r, 8)
    bin = :binary.part(r.bin, r.pos, 16)
    be = if r.endian == :big, do: bin, else: rev(bin)
    <<sign::1, exp128::15, mant::52, _::60>> = be
    exp = if exp128 == 0 and mant == 0, do: 0, else: exp128 - 16383 + 1023
    <<v::float-64>> = <<sign::1, exp::11, mant::52>>
    {v, %{r | pos: r.pos + 16}}
  end

  # Reads one PL_CDR1 (@mutable, XCDR1) member. Returns `{:pl_end, r}` at the
  # sentinel (PID_LIST_END), else `{{member_id, body}, r}`. The RTPS
  # MUST_UNDERSTAND / impl-specific flag bits (top two of the 16-bit PID) are
  # stripped before comparing against the reserved PIDs. Mirrors
  # `zerodds_cdr::xcdr1::read_pl_cdr1_member`.
  def read_pl_cdr1_member(r) do
    {pid_raw, r} = get_u16(r)
    pid = Bitwise.band(pid_raw, 0x3FFF)
    {len, r} = get_u16(r)

    if pid == 0x3F02 do
      {:pl_end, r}
    else
      {mid, blen, r} =
        if pid == 0x3F01 do
          {m, r} = get_u32(r)
          {bl, r} = get_u32(r)
          {m, bl, r}
        else
          {pid, len, r}
        end

      {body, r} = get_bytes_n(r, blen)
      pad = rem(4 - rem(blen, 4), 4)
      skip = min(pad, byte_size(r.bin) - r.pos)
      {{mid, body}, %{r | pos: r.pos + skip}}
    end
  end

  # Reads a whole PL_CDR1 parameter list into a `%{member_id => body}` map, up to
  # (and consuming) the sentinel. Later duplicate ids overwrite earlier ones.
  def read_pl_cdr1_all(r), do: read_pl_cdr1_all(r, %{})

  defp read_pl_cdr1_all(r, acc) do
    case read_pl_cdr1_member(r) do
      {:pl_end, r} -> {acc, r}
      {{id, body}, r} -> read_pl_cdr1_all(r, Map.put(acc, id, body))
    end
  end
end
"#;

/// Generates a self-contained Elixir module from the IDL AST.
///
/// # Errors
/// Returns [`IdlElixirError::Unsupported`] for constructs the Elixir backend
/// does not yet emit (e.g. `@mutable` unions and non-literal array/sequence
/// bounds).
pub fn generate_elixir_module(spec: &Specification, opts: &ElixirGenOptions) -> Result<String> {
    let pkg = &opts.module_name;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Elixir backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "# SPDX-License-Identifier: Apache-2.0\n");
    out.push_str(&WIRE_MODULE.replace("__PKG__", pkg));

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());
    USED_FIXED.with(|f| f.set(false));

    // Named-constant / enumerator registries so a named or arithmetic collection
    // bound and a named union `case` label resolve to a folded integer
    // (`eval_const_int`, IDL 4.2 §7.4.1.4.4). Cleared first — thread-locals
    // persist across `generate_elixir_module` calls on the same thread.
    CONST_VALUES.with(|m| m.borrow_mut().clear());
    ENUM_LITERAL_VALUES.with(|m| m.borrow_mut().clear());
    register_const_values(&spec.definitions);

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all top-level defs
    // (source order), emitted after the wire module, before any type.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::BeginFile);
    }

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);
    // Every `TypeDecl` to emit — module-level AND interface-nested (#A39). The
    // name/def registries below are built from this combined list so a reference
    // to an interface-nested type resolves like any other.
    let type_decls = all_type_decls(spec);

    // `bitset`/`bitmask` logical (flattened) names, published to `BIT_NAMES` so a
    // reference site resolves them to the integer-backed holder (no collection
    // DHEADER).
    let bit_names: HashSet<String> = type_decls
        .iter()
        .filter_map(|(scope, td)| match td {
            TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => Some(qualify(scope, &b.name.text)),
            TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => Some(qualify(scope, &b.name.text)),
            _ => None,
        })
        .collect();
    BIT_NAMES.with(|b| *b.borrow_mut() = bit_names);

    // Named enums/structs keyed by their flattened module-qualified alias. An
    // enum member is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1).
    let enum_names: HashSet<String> = type_decls
        .iter()
        .filter_map(|(scope, td)| match td {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => Some(qualify(scope, &e.name.text)),
            _ => None,
        })
        .collect();

    // Qualified-name -> EnumDef, so a union switching on an enum can resolve a
    // `case ENUMERATOR:` label to its integer discriminant (#A11/P4).
    let enum_defs: HashMap<String, &EnumDef> = type_decls
        .iter()
        .filter_map(|(scope, td)| match td {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => Some((qualify(scope, &e.name.text), e)),
            _ => None,
        })
        .collect();
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

    let struct_names: HashSet<String> = type_decls
        .iter()
        .filter_map(|(scope, td)| match td {
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                Some(qualify(scope, &s.name.text))
            }
            _ => None,
        })
        .collect();

    // Qualified-name -> StructDef, so a nested-struct `@key` member's own
    // `@key` subset (and `keyhash::uses_md5`'s static max-size analysis) can be
    // resolved — mirrors `struct_names` above, keeping the full def. Also used
    // to splice a base struct's members into a derived one (#A10).
    let structs: HashMap<String, &StructDef> = type_decls
        .iter()
        .filter_map(|(scope, td)| match td {
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                Some((qualify(scope, &s.name.text), s))
            }
            _ => None,
        })
        .collect();

    let typedefs = collect_typedefs(spec);

    for (scope, def) in &flat {
        let anns = def_annotations(def);
        // §7.2.2.4.8 — text directly before the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::BeforeDeclaration);
        match def {
            Definition::Type(td) => {
                emit_type_decl(
                    &mut out,
                    td,
                    pkg,
                    scope,
                    &enum_names,
                    &struct_names,
                    &structs,
                    &typedefs,
                    &enum_defs,
                )?;
            }
            // #A5/P1: an IDL `const` used to vanish through the catch-all arm.
            Definition::Const(c) => emit_const(&mut out, c, pkg, scope),
            _ => {}
        }
        // §7.2.2.4.8 — text directly after the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::AfterDeclaration);
    }

    // Interface-nested types (#A39/P8), emitted after the module-level defs
    // under the interface's own scope segment.
    for (scope, td) in &flatten_iface_types(&spec.definitions) {
        emit_type_decl(
            &mut out,
            td,
            pkg,
            scope,
            &enum_names,
            &struct_names,
            &structs,
            &typedefs,
            &enum_defs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec module is appended once if any `fixed<P,S>` was emitted.
    if USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(&FIXED_MODULE.replace("__PKG__", pkg));
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (module-level or interface-nested). Shared by the
/// module-def loop and the interface-nested-types loop (#A39) so both paths
/// produce identical output for the same declaration.
#[allow(clippy::too_many_arguments)]
fn emit_type_decl(
    out: &mut String,
    td: &TypeDecl,
    pkg: &str,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, pkg, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(
                out,
                s,
                pkg,
                scope,
                enum_names,
                struct_names,
                structs,
                typedefs,
            )?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            emit_union(
                out,
                u,
                pkg,
                scope,
                enum_names,
                struct_names,
                typedefs,
                enum_defs,
            )?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, pkg, scope)?,
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => emit_bitmask(out, b, pkg, scope),
        _ => {}
    }
    Ok(())
}

/// Emits an IDL `const` as a zero-arg accessor in its own Elixir module (#A5/P1)
/// — `const long MAX = 10;` → `defmodule <Pkg>.MAX do def value, do: 10 end`.
/// A `const` used to vanish through the top-level catch-all arm. The module name
/// is the flattened, module-qualified alias (so two consts of the same simple
/// name in different modules do not collide, and a top-level const keeps its
/// bare name). Values Elixir cannot render as a literal (an enum-typed or
/// const-alias scoped reference) are skipped rather than emitting ill-formed
/// source.
fn emit_const(out: &mut String, c: &ConstDecl, pkg: &str, scope: &[String]) {
    let Some(val) = const_expr_to_elixir(&c.value) else {
        return;
    };
    let ty = escape_elixir_ident(&qualify(scope, &c.name.text));
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  @moduledoc false");
    let _ = writeln!(out, "  def value, do: {val}");
    let _ = writeln!(out, "end");
}

/// Renders a `ConstExpr` as an Elixir literal expression, or `None` for a form
/// Elixir cannot express as a constant (an enum-valued / const-alias scoped
/// reference — a bare last segment would be an undefined variable).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_elixir(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_elixir(l),
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_elixir(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                // Elixir bitwise-NOT is the `Bitwise.bnot/1` function.
                UnaryOp::BitNot => return Some(format!("Bitwise.bnot({v})")),
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_elixir(lhs)?;
            let r = const_expr_to_elixir(rhs)?;
            // Elixir spells the bitwise operators as `Bitwise` functions.
            let fun = match op {
                BinaryOp::Or => Some("bor"),
                BinaryOp::Xor => Some("bxor"),
                BinaryOp::And => Some("band"),
                BinaryOp::Shl => Some("bsl"),
                BinaryOp::Shr => Some("bsr"),
                _ => None,
            };
            if let Some(f) = fun {
                return Some(format!("Bitwise.{f}({l}, {r})"));
            }
            let o = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                // Integer division in IDL const context; Elixir `div/2` keeps it
                // integer, `/` would float. Use `div` for the common integer
                // case — a float const uses `+`/`-`/`*` only in practice.
                BinaryOp::Div => return Some(format!("div({l}, {r})")),
                BinaryOp::Mod => return Some(format!("rem({l}, {r})")),
                _ => "+",
            };
            Some(format!("({l} {o} {r})"))
        }
    }
}

/// Renders a single literal as valid Elixir source.
fn const_literal_to_elixir(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Elixir accepts decimal / `0x` integer literals; `0o`/`0b` too.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float suffix (`d`/`f`/`l`) Elixir rejects, and
        // normalize a bare `.5`/`5.` to a form Elixir's float parser accepts.
        LiteralKind::Floating => normalize_elixir_float(raw),
        // A `fixed` decimal has no native Elixir constant type — render as a
        // string (drops the trailing `d`/`D`).
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Never emit a bare `TRUE`/`FALSE` token (not an Elixir literal — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char pass through; wide literals drop the `L` prefix
        // (`L"x"`/`L'x'` is not valid Elixir — #A7-pattern). A `char` literal
        // like `'A'` is rendered as its Elixir codepoint integer (`?A`).
        LiteralKind::Char => {
            let inner = raw.strip_prefix('L').unwrap_or(raw);
            format!(
                "?{}",
                inner.trim_matches('\'').chars().next().unwrap_or(' ')
            )
        }
        LiteralKind::WideChar => {
            let inner = raw.strip_prefix('L').unwrap_or(raw);
            format!(
                "?{}",
                inner.trim_matches('\'').chars().next().unwrap_or(' ')
            )
        }
        LiteralKind::String => raw.to_string(),
        LiteralKind::WideString => raw.strip_prefix('L').unwrap_or(raw).to_string(),
    })
}

/// Normalizes an IDL floating literal to valid Elixir float syntax: strips the
/// trailing type suffix and ensures both an integer and a fractional digit
/// surround the decimal point (Elixir rejects `.5` and `5.`).
fn normalize_elixir_float(raw: &str) -> String {
    let s = raw.trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L']);
    // Leave exponent forms (`1e9`) as-is if they carry no bare dot.
    if let Some((int, frac)) = s.split_once('.') {
        let int = if int.is_empty() { "0" } else { int };
        // A fractional part may itself hold an exponent (`.5e3`).
        if frac.is_empty() {
            format!("{int}.0")
        } else {
            format!("{int}.{frac}")
        }
    } else {
        s.to_string()
    }
}

/// Evaluates a collection bound / array dimension / bitfield width / `fixed`
/// P/S to its integer value. Delegates to [`eval_const_int`] so a named `const`
/// (`long a[LEN]`, `sequence<octet, MAX>`), an enumerator, and folded
/// arithmetic all resolve — idl-rust/idl-zig `const_expr` parity (§7.4.1.4.4),
/// not just a bare integer literal + unary sign.
/// zerodds-lint: recursion-depth 1 (delegates to `eval_const_int`).
fn array_size(e: &ConstExpr) -> Option<i64> {
    eval_const_int(e, &HashMap::new(), 0)
}

/// Builds a pipe segment that marshals a fixed array via nested `Enum.reduce`
/// over the (list-of-)list(s), row-major. The element seg uses `$elem`.
fn build_array_seg(coll_root: &str, ndim: usize, elem_seg: &str) -> String {
    let leaf = format!(
        "acc{} |> {}",
        ndim - 1,
        elem_seg.replace("$elem", &format!("e{}", ndim - 1))
    );
    let mut body = leaf;
    for k in (0..ndim).rev() {
        let coll = if k == 0 {
            coll_root.to_string()
        } else {
            format!("e{}", k - 1)
        };
        let init = if k == 0 {
            "w".to_string()
        } else {
            format!("acc{}", k - 1)
        };
        body = format!("Enum.reduce({coll}, {init}, fn e{k}, acc{k} -> {body} end)");
    }
    format!("then(fn w -> {body} end)")
}

/// Maps an IDL union `switch` type to a `TypeSpec` so the discriminator reuses
/// the normal `map_type` path.
fn switch_typespec(s: &SwitchTypeSpec) -> TypeSpec {
    match s {
        SwitchTypeSpec::Integer(i) => TypeSpec::Primitive(PrimitiveType::Integer(*i)),
        SwitchTypeSpec::Char => TypeSpec::Primitive(PrimitiveType::Char),
        SwitchTypeSpec::Boolean => TypeSpec::Primitive(PrimitiveType::Boolean),
        SwitchTypeSpec::Octet => TypeSpec::Primitive(PrimitiveType::Octet),
        SwitchTypeSpec::Scoped(sn) => TypeSpec::Scoped(sn.clone()),
    }
}

/// One generated union case: its integer labels, whether it is the `default`,
/// the Elixir field name, and the encode (`seg`) / decode (`get`) fragments.
struct ElixirCase {
    labels: Vec<i64>,
    is_default: bool,
    field: String,
    seg: String,
    get: String,
}

/// Emits an IDL `union` as an Elixir struct (discriminator + one field per case
/// member) with a pipe-compatible `marshal_into` that puts the discriminator
/// then a `case` dispatches to the selected member (XCDR2 §7.4.3.5.4).
#[allow(clippy::too_many_arguments)]
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    pkg: &str,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    // Member references resolve against this union's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility_union(u);
    let disc_seg = map_type(
        &switch_typespec(&u.switch_type),
        "v.disc",
        pkg,
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        pkg,
        enum_names,
        struct_names,
    )?;

    // #A11/P4: when the discriminator is an enum, map enumerator name → value so
    // a `case ENUMERATOR:` label resolves to its integer discriminant.
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
    // A boolean discriminator switches on Elixir `true`/`false`, not integers.
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);

    let mut cases: Vec<ElixirCase> = Vec::new();
    for c in &u.cases {
        let field = escape_elixir_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let seg = map_type(
            &resolved,
            &format!("v.{field}"),
            pkg,
            enum_names,
            struct_names,
        )?;
        let get = map_get(&resolved, pkg, enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                // #A11/A12/A13/P4: resolve enum / char / boolean labels, not only
                // plain integer literals (the former `array_size` aborted on
                // those, dropping every non-integer-discriminated union).
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlElixirError::Unsupported(format!(
                            "non-integer union label in `{}`",
                            u.name.text
                        ))
                    })?)
                }
            }
        }
        cases.push(ElixirCase {
            labels,
            is_default,
            field,
            seg,
            get,
        });
    }
    let has_default = cases.iter().any(|c| c.is_default);

    let ty = escape_elixir_ident(&qualify(scope, &u.name.text));
    let wire = format!("{pkg}.Wire");
    let mut names = vec![":disc".to_string()];
    names.extend(cases.iter().map(|c| format!(":{}", c.field)));
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct [{}]", names.join(", "));
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &u.annotations, PlacementKind::BeginDeclaration);

    // Render one label value (a boolean discriminator uses `true`/`false`).
    let render_label = |v: i64| -> String {
        if disc_is_bool {
            (v != 0).to_string()
        } else {
            v.to_string()
        }
    };
    // A pattern for a case's labels: `1` for one, `d when d in [1, 3]` for many.
    let pat = |c: &ElixirCase| -> String {
        if c.is_default {
            "_".to_string()
        } else if c.labels.len() == 1 {
            render_label(c.labels[0])
        } else {
            let list = c
                .labels
                .iter()
                .map(|&v| render_label(v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("d when d in [{list}]")
        }
    };

    let _ = writeln!(out, "\n  def marshal_into(w, %__MODULE__{{}} = v) do");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1 classic CDR: PL_CDR1 — discriminator is member id 0, each case its
        // 1-based id (body member-relative in an XCDR1 sub-writer), sentinel; no
        // outer DHEADER (XTypes 1.3 §7.4.1.2 / §7.4.2).
        let _ = writeln!(out, "    if w.xcdr1 do");
        let _ = writeln!(out, "      w");
        let _ = writeln!(out, "      |> then(fn zw ->");
        let _ = writeln!(
            out,
            "        zdm = {wire}.subwriter(zw) |> {disc_seg} |> {wire}.bytes()"
        );
        let _ = writeln!(out, "        {wire}.put_pl_cdr1_member(zw, 0, zdm)");
        let _ = writeln!(out, "      end)");
        let _ = writeln!(out, "      |> then(fn zw ->");
        let _ = writeln!(out, "        case v.disc do");
        for (i, c) in cases.iter().enumerate() {
            let id = u32::try_from(i + 1).unwrap_or(0);
            let _ = writeln!(out, "          {} ->", pat(c));
            let _ = writeln!(
                out,
                "            zdm = {wire}.subwriter(zw) |> {} |> {wire}.bytes()",
                c.seg
            );
            let _ = writeln!(out, "            {wire}.put_pl_cdr1_member(zw, {id}, zdm)");
        }
        if !has_default {
            let _ = writeln!(out, "          _ -> zw");
        }
        let _ = writeln!(out, "        end");
        let _ = writeln!(out, "      end)");
        let _ = writeln!(out, "      |> {wire}.put_pl_cdr1_sentinel()");
        let _ = writeln!(out, "    else");
        // #A16: XCDR2 EMHEADER-framed member list — discriminator is member id 0,
        // each branch its 1-based id, the whole wrapped in the union's DHEADER
        // (LC4; #A19 compact codes are a separate coordinated change).
        let _ = writeln!(out, "    body = {wire}.writer(w.endian)");
        let emh0 = 0x4000_0000_u32;
        let _ = writeln!(out, "    body = body |> {wire}.put_u32(0x{emh0:08x})");
        let _ = writeln!(
            out,
            "    memd = {wire}.writer(w.endian) |> {disc_seg} |> {wire}.bytes()"
        );
        let _ = writeln!(
            out,
            "    body = body |> {wire}.put_u32(byte_size(memd)) |> {wire}.put_bytes(memd)"
        );
        let _ = writeln!(out, "    body =");
        let _ = writeln!(out, "      case v.disc do");
        for (i, c) in cases.iter().enumerate() {
            let emh = 0x4000_0000_u32 | (u32::try_from(i + 1).unwrap_or(0) & 0x0FFF_FFFF);
            let _ = writeln!(out, "        {} ->", pat(c));
            let _ = writeln!(out, "          bh = body |> {wire}.put_u32(0x{emh:08x})");
            let _ = writeln!(
                out,
                "          memb = {wire}.writer(w.endian) |> {} |> {wire}.bytes()",
                c.seg
            );
            let _ = writeln!(
                out,
                "          bh |> {wire}.put_u32(byte_size(memb)) |> {wire}.put_bytes(memb)"
            );
        }
        if !has_default {
            let _ = writeln!(out, "        _ -> body");
        }
        let _ = writeln!(out, "      end");
        let _ = writeln!(out, "    body_bytes = {wire}.bytes(body)");
        let _ = writeln!(
            out,
            "    w |> {wire}.put_u32(byte_size(body_bytes)) |> {wire}.put_bytes(body_bytes)"
        );
        let _ = writeln!(out, "    end");
    } else if ext == ExtensibilityKind::Final {
        // @final: disc + selected member inline for both representations.
        let _ = writeln!(out, "    w = w |> {disc_seg}");
        let _ = writeln!(out, "    case v.disc do");
        for c in &cases {
            let _ = writeln!(out, "      {} -> w |> {}", pat(c), c.seg);
        }
        if !has_default {
            let _ = writeln!(out, "      _ -> w");
        }
        let _ = writeln!(out, "    end");
    } else {
        // @appendable: XCDR1 inline (disc + member, no DHEADER) / XCDR2 body.
        let _ = writeln!(out, "    if w.xcdr1 do");
        let _ = writeln!(out, "      w = w |> {disc_seg}");
        let _ = writeln!(out, "      case v.disc do");
        for c in &cases {
            let _ = writeln!(out, "        {} -> w |> {}", pat(c), c.seg);
        }
        if !has_default {
            let _ = writeln!(out, "        _ -> w");
        }
        let _ = writeln!(out, "      end");
        let _ = writeln!(out, "    else");
        let _ = writeln!(out, "    body_w = {wire}.subwriter(w) |> {disc_seg}");
        let _ = writeln!(out, "    body_w =");
        let _ = writeln!(out, "      case v.disc do");
        for c in &cases {
            let _ = writeln!(out, "        {} -> body_w |> {}", pat(c), c.seg);
        }
        if !has_default {
            let _ = writeln!(out, "        _ -> body_w");
        }
        let _ = writeln!(out, "      end");
        let _ = writeln!(out, "    body = {wire}.bytes(body_w)");
        let _ = writeln!(
            out,
            "    w |> {wire}.put_u32(byte_size(body)) |> {wire}.put_bytes(body)"
        );
        let _ = writeln!(out, "    end");
    }
    let _ = writeln!(out, "  end");

    let _ = writeln!(out, "\n  def marshal_xcdr(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(
        out,
        "    {wire}.writer(endian) |> marshal_into(v) |> {wire}.bytes()"
    );
    let _ = writeln!(out, "  end");

    // XCDR1 / classic-CDR entry point (`writer1` sets the rep flag).
    let _ = writeln!(out, "\n  def marshal_xcdr1(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(
        out,
        "    {wire}.writer1(endian) |> marshal_into(v) |> {wire}.bytes()"
    );
    let _ = writeln!(out, "  end");

    // Decode: read the discriminator, then a `case` reads only the selected
    // member and builds the struct (@appendable skips the leading DHEADER;
    // @mutable skips the DHEADER then reads each member's EMHEADER + NEXTINT —
    // positional, so a fully-present union round-trips).
    let _ = writeln!(out, "\n  def read(r) do");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1: PL_CDR1 member map — discriminator is id 0, selected member its
        // 1-based id; each decoded from its own member-relative XCDR1 reader.
        let _ = writeln!(out, "    if r.xcdr1 do");
        let _ = writeln!(out, "      ze = r.endian");
        let _ = writeln!(out, "      {{zdpl, r}} = {wire}.read_pl_cdr1_all(r)");
        let _ = writeln!(
            out,
            "      {{disc, _zdr}} = {}",
            disc_get.replace("$r", &format!("{wire}.reader1(Map.get(zdpl, 0), ze)"))
        );
        let _ = writeln!(out, "      v =");
        let _ = writeln!(out, "        case disc do");
        for (i, c) in cases.iter().enumerate() {
            let id = u32::try_from(i + 1).unwrap_or(0);
            let g = c.get.replace("$r", &format!("{wire}.reader1(zdbody, ze)"));
            let _ = writeln!(out, "          {} ->", pat(c));
            let _ = writeln!(out, "            case Map.get(zdpl, {id}) do");
            let _ = writeln!(out, "              nil -> %__MODULE__{{disc: disc}}");
            let _ = writeln!(
                out,
                "              zdbody -> {{{}, _zdr}} = {g}; %__MODULE__{{disc: disc, {n}: {n}}}",
                c.field,
                n = c.field
            );
            let _ = writeln!(out, "            end");
        }
        if !has_default {
            let _ = writeln!(out, "          _ -> %__MODULE__{{disc: disc}}");
        }
        let _ = writeln!(out, "        end");
        let _ = writeln!(out, "      {{v, r}}");
        let _ = writeln!(out, "    else");
        // XCDR2: skip the union DHEADER, then the discriminator's EMHEADER +
        // NEXTINT, then (positional) the selected member's EMHEADER + NEXTINT.
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        emit_union_disc_case_read(out, &wire, &disc_get, &cases, has_default, &pat, true);
        let _ = writeln!(out, "    end");
    } else {
        if ext == ExtensibilityKind::Appendable {
            // XCDR2 frames the appendable body with a DHEADER; XCDR1 has none.
            let _ = writeln!(
                out,
                "    {{_, r}} = if r.xcdr1, do: {{0, r}}, else: {wire}.get_u32(r)"
            );
        }
        emit_union_disc_case_read(out, &wire, &disc_get, &cases, has_default, &pat, false);
    }
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal_xcdr1(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader1(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &u.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    Ok(())
}

/// Emits the shared "read discriminator, then a `case` reads the selected
/// member" tail of a union decode (the XCDR2/PLAIN_CDR path, used by @final,
/// @appendable, and the XCDR2 arm of @mutable). `mutable_headers` inserts the
/// per-member EMHEADER + NEXTINT skips (@mutable positional decode).
fn emit_union_disc_case_read(
    out: &mut String,
    wire: &str,
    disc_get: &str,
    cases: &[ElixirCase],
    has_default: bool,
    pat: &dyn Fn(&ElixirCase) -> String,
    mutable_headers: bool,
) {
    let _ = writeln!(out, "    {{disc, r}} = {}", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "    {{v, r}} =");
    let _ = writeln!(out, "      case disc do");
    for c in cases {
        let g = c.get.replace("$r", "r");
        let _ = writeln!(out, "        {} ->", pat(c));
        if mutable_headers {
            let _ = writeln!(out, "          {{_, r}} = {wire}.get_u32(r)");
            let _ = writeln!(out, "          {{_, r}} = {wire}.get_u32(r)");
        }
        let _ = writeln!(out, "          {{{}, r}} = {g}", c.field);
        let _ = writeln!(
            out,
            "          {{%__MODULE__{{disc: disc, {n}: {n}}}, r}}",
            n = c.field
        );
    }
    if !has_default {
        let _ = writeln!(out, "        _ -> {{%__MODULE__{{disc: disc}}, r}}");
    }
    let _ = writeln!(out, "      end");
    let _ = writeln!(out, "    {{v, r}}");
}

/// Evaluates a union case label (`case RED:`, `case 'A':`, `case TRUE:`,
/// `case 3:`) to its integer discriminant (#A11/A12/A13/P4). Beyond the plain
/// integer literals the former `array_size` accepted, this resolves enum
/// enumerators (via `enum_vals`, name → value of the switch enum), `char` code
/// points, and the `boolean` keywords `TRUE`/`FALSE`.
/// zerodds-lint: recursion-depth 32 (label expression; bounded by the grammar).
fn eval_union_label(e: &ConstExpr, enum_vals: &HashMap<String, i64>) -> Option<i64> {
    // The switch enum's enumerators take priority (passed as `locals`), then the
    // spec-wide enumerator/const registries and folded arithmetic — so a
    // `case ENUMERATOR:`, `case NAMED_CONST:`, or `case 1 + 1:` all resolve
    // (idl-rust/idl-zig parity), not only a plain integer / switch-enum name.
    eval_const_int(e, enum_vals, 0)
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point,
/// so a `case 'A':` union label resolves to the discriminant 65 (#A12).
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

/// Union extensibility (defaults to `@appendable`, matching structs).
fn extensibility_union(u: &UnionDef) -> ExtensibilityKind {
    lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
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

/// Emits an IDL `enum` as an Elixir module of enumerator-value functions. The
/// enum member itself is a plain integer field marshaled as an `i32`.
fn emit_enum(out: &mut String, e: &EnumDef, pkg: &str, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_elixir_ident(&qualify(scope, &e.name.text));
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &e.annotations, PlacementKind::BeginDeclaration);
    for (en, value) in e.enumerators.iter().zip(&values) {
        let name = escape_elixir_ident(&en.name.text.to_lowercase());
        let _ = writeln!(out, "  def {name}, do: {value}");
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &e.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
}

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→u8, `≤16`→u16, `≤32`→u32, else
/// u64). Returns `(put-method, get-method, bit-width)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, u32) {
    match total_bits {
        0..=8 => ("put_u8", "get_u8", 8),
        9..=16 => ("put_u16", "get_u16", 16),
        17..=32 => ("put_u32", "get_u32", 32),
        _ => ("put_u64", "get_u64", 64),
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

/// Emits the shared holder tail (`marshal_into`/`marshal_xcdr`/`read`/
/// `unmarshal`) of a `bitset`/`bitmask` module over its backing integer
/// (XTypes 1.3 §7.4.7 — wire = backing int). `put`/`get` are the `Wire`
/// method names for the backing width.
fn emit_bit_holder_tail(out: &mut String, pkg: &str, put: &str, get: &str) {
    let wire = format!("{pkg}.Wire");
    let _ = writeln!(out, "\n  def marshal_into(w, %__MODULE__{{}} = v) do");
    let _ = writeln!(out, "    {wire}.{put}(w, v.storage)");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def marshal_xcdr(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(
        out,
        "    {wire}.writer(endian) |> marshal_into(v) |> {wire}.bytes()"
    );
    let _ = writeln!(out, "  end");
    // XCDR1 / classic-CDR entry points. The backing integer marshals identically
    // in both representations; only the max alignment differs (a u64-backed
    // holder aligns to 8 under XCDR1, 4 under XCDR2), driven by the rep flag.
    let _ = writeln!(out, "\n  def marshal_xcdr1(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(
        out,
        "    {wire}.writer1(endian) |> marshal_into(v) |> {wire}.bytes()"
    );
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def read(r) do");
    let _ = writeln!(out, "    {{s, r}} = {wire}.{get}(r)");
    let _ = writeln!(out, "    {{%__MODULE__{{storage: s}}, r}}");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal_xcdr1(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader1(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "end");
}

/// Emits an IDL `bitset` as an Elixir holder module over its backing integer,
/// with a bit-accessor pair (`field`/`set_field`) per named bitfield and an
/// XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3 §7.4.7 —
/// wire = backing int).
///
/// # Errors
/// [`IdlElixirError::Unsupported`] if a bitfield width is not a codegen-time
/// integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, pkg: &str, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlElixirError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (put, get, _) = bit_storage(total);
    let ty = escape_elixir_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct storage: 0");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_elixir_ident(&name.text);
            if *width == 1 {
                // A 1-bit field reads/writes a boolean.
                let _ = writeln!(
                    out,
                    "  def {field}(%__MODULE__{{}} = v), do: Bitwise.band(Bitwise.bsr(v.storage, {offset}), 1) != 0"
                );
                let _ = writeln!(
                    out,
                    "  def set_{field}(%__MODULE__{{}} = v, b) do\n    bit = if b, do: Bitwise.bsl(1, {offset}), else: 0\n    %{{v | storage: Bitwise.bor(Bitwise.band(v.storage, Bitwise.bnot(Bitwise.bsl(1, {offset}))), bit)}}\n  end"
                );
            } else {
                let mask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "  def {field}(%__MODULE__{{}} = v), do: Bitwise.band(Bitwise.bsr(v.storage, {offset}), {mask})"
                );
                let _ = writeln!(
                    out,
                    "  def set_{field}(%__MODULE__{{}} = v, x) do\n    m = Bitwise.bsl({mask}, {offset})\n    %{{v | storage: Bitwise.bor(Bitwise.band(v.storage, Bitwise.bnot(m)), Bitwise.bsl(Bitwise.band(x, {mask}), {offset}))}}\n  end"
                );
            }
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::EndDeclaration);
    emit_bit_holder_tail(out, pkg, put, get);
    Ok(())
}

/// Emits an IDL `bitmask` as an Elixir holder module over its `@bit_bound`
/// backing integer (default 32), with an OR-able manifest constant per bit
/// value and an XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3
/// §7.4.7). Each constant's value is folded to a literal at codegen time
/// (`1 << position`).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, pkg: &str, scope: &[String]) {
    let (put, get, _) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_elixir_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct storage: 0");
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::BeginDeclaration);
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = escape_elixir_ident(&v.name.text.to_lowercase());
        let value: u64 = 1u64 << pos;
        let _ = writeln!(out, "  def {cname}, do: {value}");
    }
    emit_verbatim_at(out, "  ", &b.annotations, PlacementKind::EndDeclaration);
    emit_bit_holder_tail(out, pkg, put, get);
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlElixirError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlElixirError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlElixirError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
}

/// Recursively descends into `Definition::Module`, returning every
/// non-module definition (struct/enum/union/typedef/…) in document order.
/// The IDL AST builder already merges a reopened `module M {} ... module
/// M {}` into one AST node (`crates/idl/src/ast/builder.rs`); this promotes
/// a module's members into the same flat namespace this backend already
/// uses for type-reference resolution — a module's members are promoted to the
/// top level, each paired with its module scope path so the definition and
/// reference sites can flatten each name to a qualified alias ([`qualify`] /
/// [`resolve_scoped_name`]). Two same-simple-name types in different modules
/// therefore become distinct modules rather than colliding (#21).
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

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39/P8). Elixir has no nested-type
/// construct, so these are promoted to the top level under the interface's own
/// name segment (so two interfaces in one module do not collide). Without this
/// every type declared inside an `interface { ... }` body was silently dropped.
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

/// Every `TypeDecl` to emit, module-level and interface-nested (#A39), each
/// paired with its flattening scope. Name/def registries (enum/struct/bitset/
/// bitmask/typedef) are built from this combined list so a reference to an
/// interface-nested type resolves the same as a module-level one.
fn all_type_decls(spec: &Specification) -> Vec<(Vec<String>, &TypeDecl)> {
    let mut out: Vec<(Vec<String>, &TypeDecl)> = flatten_module_defs(&spec.definitions)
        .into_iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(td) => Some((scope, td)),
            _ => None,
        })
        .collect();
    out.extend(flatten_iface_types(&spec.definitions));
    out
}

/// Collects `typedef` aliases (simple declarators) as qualified-name -> aliased
/// type-spec. A typedef is wire-transparent, so members are resolved to the
/// underlying type before mapping (`typedef long Score; Score s;` → `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    // Module-level AND interface-nested typedefs (#A39) share the flat namespace.
    for (scope, td) in all_type_decls(spec) {
        if let TypeDecl::Typedef(td) = td {
            for d in &td.declarators {
                if let Declarator::Simple(name) = d {
                    m.insert(qualify(&scope, &name.text), td.type_spec.clone());
                }
            }
        }
    }
    m
}

/// Collects a struct's effective members base-first (#A10/P3): the base struct's
/// members (recursively) precede the derived struct's own, so the generated
/// module and its wire form carry the inherited fields — matching cpp/csharp/
/// java (`resolve_wire_members`). Without this a `struct D : Base` dropped every
/// inherited field from both the type and the wire. A cycle guard bounds
/// pathological inheritance loops.
/// zerodds-lint: recursion-depth 16 (struct inheritance chain; bounded by the
/// IDL aggregate nesting depth).
fn collect_base_members<'a>(
    s: &'a StructDef,
    structs: &HashMap<String, &'a StructDef>,
    seen: &mut HashSet<String>,
    out: &mut Vec<&'a Member>,
) {
    if let Some(base) = &s.base {
        let name = resolve_scoped_name(base);
        if seen.insert(name.clone()) {
            if let Some(bs) = structs.get(&name) {
                collect_base_members(bs, structs, seen, out);
            }
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

// `pkg` (the file-level module namespace) and `scope` (the IDL module scope for
// #21 name qualification) are both threaded through alongside the four
// name/def maps, one over clippy's 7-arg heuristic.
#[allow(clippy::too_many_arguments)]
fn emit_struct(
    out: &mut String,
    s: &StructDef,
    pkg: &str,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility(s);

    struct FieldGen {
        name: String,
        // full pipe segment after `|>`, e.g. `Pkg.Wire.put_u32(v.id)`.
        seg: String,
        // decode expression (placeholder `$r`) evaluating to `{value, reader}`.
        get: String,
        id: u32,
        key: bool,
        // `Some((type_spec, expr))` for a Simple (non-array) declarator, so a
        // `@key` field can be re-mapped through `map_key_type` instead of
        // reusing `seg` (which, for a struct-typed member, is the full
        // `marshal_into` call shared with normal, non-key encoding). `None`
        // for an array declarator — `map_key_type` expects a scalar
        // TypeSpec/expr pair and would otherwise encode the array's ELEMENT
        // type once against the whole list value (wrong KeyHash: scalar-
        // encoding a list). Array key fields reuse `seg` unchanged instead —
        // it already emits the correct row-major, no-length-prefix element
        // encoding (mirrors `idl-lua`'s `key_type: Option<..>` guard).
        key_type: Option<(TypeSpec, String)>,
        // `@optional`: a companion `<name>_present` presence flag (u8) precedes
        // the value, which is written only when present (XTypes 1.3 §7.4.5.1.4).
        optional: bool,
        // `@must_understand`: EMHEADER must-understand bit 31 under @mutable
        // (XTypes 1.3 §7.4.3.4.2 — #A17). No effect on @final/@appendable wire.
        must_understand: bool,
        // `@non_serialized`: kept in the Elixir defstruct, off every wire form.
        non_serialized: bool,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    // Container-level `@autoid(HASH)` (XTypes 1.3 §7.3.1.2.1.1): when set, every
    // member with no explicit `@id`/`@hashid` takes a name-hashed member id
    // instead of a sequential one. Resolved through the shared frontend
    // (`semantics::member_id`) so the EMHEADER/PL_CDR1/key-order ids match
    // idl-rust/idl-cpp and the TypeObject (findings A31/A32), byte-identical to
    // `NameHash::member_id_from_name` = MD5(name)[0..4] LE & 0x0FFFFFFF.
    let autoid_hash = zerodds_idl::semantics::member_id::container_autoid_hash(&s.annotations);
    // Sequential fallback counter (`@autoid(SEQUENTIAL)`): advances ONLY for a
    // member that takes the positional default — an explicit `@id`, `@hashid`,
    // or `@autoid(HASH)` id does NOT consume a slot, matching the canonical
    // `resolve_member_ids` in `zerodds-types` and idl-nim's fix.
    let mut next_id: u32 = 0;
    // #A10/P3: base-class members precede the struct's own, in both the Elixir
    // `defstruct` and the wire encode/decode/key order.
    let mut base_seen: HashSet<String> = HashSet::new();
    let mut resolved_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut base_seen, &mut resolved_members);
    for m in &resolved_members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
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
        // P0-5 (#2): a `@non_serialized` member keeps its defstruct field but is
        // off the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            // Raw IDL member name (never the escaped Elixir identifier): the wire
            // member-id hash is over the source spelling (XTypes §7.3.1.2.1.4).
            let raw = d.name().text.clone();
            let name = escape_elixir_ident(&raw);
            let id = if non_serialized {
                0
            } else {
                match zerodds_idl::semantics::member_id::fixed_member_id(
                    autoid_hash,
                    &m.annotations,
                    &raw,
                ) {
                    Some(fixed) => fixed,
                    None => {
                        let seq = next_id;
                        next_id += 1;
                        seq
                    }
                }
            };
            let (seg, get, key_type) = match d {
                Declarator::Simple(_) => {
                    let expr = format!("v.{name}");
                    let s = map_type(&resolved, &expr, pkg, enum_names, struct_names)?;
                    let g = map_get(&resolved, pkg, enum_names, struct_names)?;
                    (s, g, Some((resolved.clone(), expr)))
                }
                // Fixed array: elements inline, row-major, no length prefix —
                // nested `Enum.reduce` over the (list-of-)list(s).
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlElixirError::Unsupported(format!(
                                "non-literal array size on `{name}`"
                            ))
                        })?;
                    let elem_seg = map_type(&resolved, "$elem", pkg, enum_names, struct_names)?;
                    let seg = build_array_seg(&format!("v.{name}"), sizes.len(), &elem_seg);
                    let elem_get = map_get(&resolved, pkg, enum_names, struct_names)?;
                    let get = build_array_get(&sizes, &elem_get);
                    (seg, get, None)
                }
            };
            fields.push(FieldGen {
                name,
                seg,
                get,
                id,
                key,
                key_type,
                optional,
                must_understand,
                non_serialized,
            });
        }
    }

    // A field's marshal pipe step. `@optional`: a u8 presence flag, then the
    // value only when present (XTypes 1.3 §7.4.5.1.4). `zw`/the closure keep the
    // step pipe-compatible; `v` is captured from the enclosing function.
    let field_step = |f: &FieldGen| -> String {
        if f.optional {
            format!(
                "then(fn zw -> zw = {pkg}.Wire.put_u8(zw, if(v.{n}_present, do: 1, else: 0)); if v.{n}_present, do: zw |> {seg}, else: zw end)",
                n = f.name,
                seg = f.seg
            )
        } else {
            f.seg.clone()
        }
    };

    let ty = escape_elixir_ident(&qualify(scope, &s.name.text));
    let wire = format!("{pkg}.Wire");
    let names: Vec<String> = fields
        .iter()
        .flat_map(|f| {
            if f.optional {
                vec![format!(":{}_present", f.name), format!(":{}", f.name)]
            } else {
                vec![format!(":{}", f.name)]
            }
        })
        .collect();
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct [{}]", names.join(", "));
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "  ", &s.annotations, PlacementKind::BeginDeclaration);

    // marshal_into writes the struct into an existing writer (pipe-compatible:
    // takes the writer first, returns it) so nested composites keep stream-
    // relative alignment. The writer's `xcdr1` flag selects the framing at
    // runtime — one generated type serves both representations. @final: fields
    // inline (both). @appendable: DHEADER body (XCDR2) / inline (XCDR1).
    // @mutable: PL_CDR2 EMHEADER list (XCDR2) / PL_CDR1 PID list (XCDR1).
    let _ = writeln!(out, "\n  def marshal_into(w, %__MODULE__{{}} = v) do");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1 classic CDR: PL_CDR1 — [PID][len] members (body built
        // member-relative in an XCDR1 sub-writer), terminated by the 0x3F02
        // sentinel; NO outer DHEADER (XTypes 1.3 §7.4.1.2 / §7.4.2).
        let _ = writeln!(out, "    if w.xcdr1 do");
        let _ = writeln!(out, "      w");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "      |> then(fn zw ->");
            if f.optional {
                let _ = writeln!(out, "        if v.{}_present do", f.name);
                let _ = writeln!(
                    out,
                    "          zdm = {wire}.subwriter(zw) |> {} |> {wire}.bytes()",
                    f.seg
                );
                let _ = writeln!(
                    out,
                    "          {wire}.put_pl_cdr1_member(zw, {}, zdm)",
                    f.id
                );
                let _ = writeln!(out, "        else");
                let _ = writeln!(out, "          zw");
                let _ = writeln!(out, "        end");
            } else {
                let _ = writeln!(
                    out,
                    "        zdm = {wire}.subwriter(zw) |> {} |> {wire}.bytes()",
                    f.seg
                );
                let _ = writeln!(out, "        {wire}.put_pl_cdr1_member(zw, {}, zdm)", f.id);
            }
            let _ = writeln!(out, "      end)");
        }
        let _ = writeln!(out, "      |> {wire}.put_pl_cdr1_sentinel()");
        let _ = writeln!(out, "    else");
        // XCDR2: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id, plus must-understand bit 31 when @must_understand — #A17) +
        // NEXTINT (body length) + body (XTypes §7.4.3.4.2). LC4 is kept as the
        // universal length code (compact per-width codes — #A19 — are a separate
        // coordinated cross-backend change).
        let _ = writeln!(out, "    body = {wire}.writer(w.endian)");
        for (i, f) in fields.iter().enumerate() {
            if f.non_serialized {
                continue;
            }
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): guard its EMHEADER+body on the flag.
            let mu_bit = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu_bit | 0x4000_0000_u32 | (f.id & 0x0FFF_FFFF);
            if f.optional {
                let _ = writeln!(out, "    body =");
                let _ = writeln!(out, "      if v.{}_present do", f.name);
                let _ = writeln!(out, "        body = body |> {wire}.put_u32(0x{emh:08x})");
                let _ = writeln!(
                    out,
                    "        mem{i} = {wire}.writer(w.endian) |> {} |> {wire}.bytes()",
                    f.seg
                );
                let _ = writeln!(
                    out,
                    "        body |> {wire}.put_u32(byte_size(mem{i})) |> {wire}.put_bytes(mem{i})"
                );
                let _ = writeln!(out, "      else");
                let _ = writeln!(out, "        body");
                let _ = writeln!(out, "      end");
            } else {
                let _ = writeln!(out, "    body = body |> {wire}.put_u32(0x{emh:08x})");
                let _ = writeln!(
                    out,
                    "    mem{i} = {wire}.writer(w.endian) |> {} |> {wire}.bytes()",
                    f.seg
                );
                let _ = writeln!(
                    out,
                    "    body = body |> {wire}.put_u32(byte_size(mem{i})) |> {wire}.put_bytes(mem{i})"
                );
            }
        }
        let _ = writeln!(out, "    body_bytes = {wire}.bytes(body)");
        let _ = writeln!(
            out,
            "    w |> {wire}.put_u32(byte_size(body_bytes)) |> {wire}.put_bytes(body_bytes)"
        );
        let _ = writeln!(out, "    end");
    } else if ext == ExtensibilityKind::Final {
        // @final: inline for BOTH representations; the writer's rep flag decides
        // max alignment (4 XCDR2 / 8 XCDR1). No DHEADER either way.
        let _ = writeln!(out, "    w");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "    |> {}", field_step(f));
        }
    } else {
        // @appendable: XCDR1 inline (no DHEADER) / XCDR2 length-prefixed body.
        let _ = writeln!(out, "    if w.xcdr1 do");
        let _ = writeln!(out, "      w");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "      |> {}", field_step(f));
        }
        let _ = writeln!(out, "    else");
        let _ = writeln!(out, "    body =");
        let _ = writeln!(out, "      {wire}.subwriter(w)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "      |> {}", field_step(f));
        }
        let _ = writeln!(out, "      |> {wire}.bytes()");
        let _ = writeln!(out, "    w");
        let _ = writeln!(out, "    |> {wire}.put_u32(byte_size(body))");
        let _ = writeln!(out, "    |> {wire}.put_bytes(body)");
        let _ = writeln!(out, "    end");
    }
    let _ = writeln!(out, "  end");

    let _ = writeln!(out, "\n  def marshal_xcdr(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(out, "    {wire}.writer(endian)");
    let _ = writeln!(out, "    |> marshal_into(v)");
    let _ = writeln!(out, "    |> {wire}.bytes()");
    let _ = writeln!(out, "  end");

    // XCDR1 / classic-CDR entry point: same member logic, max-alignment-8
    // writer, no DHEADER, PL_CDR1 @mutable framing (`writer1` sets the flag).
    let _ = writeln!(out, "\n  def marshal_xcdr1(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(out, "    {wire}.writer1(endian)");
    let _ = writeln!(out, "    |> marshal_into(v)");
    let _ = writeln!(out, "    |> {wire}.bytes()");
    let _ = writeln!(out, "  end");

    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        // Include inherited `@key` members (#A10): key-hash covers base keys too.
        let key_members: Vec<&Member> = resolved_members
            .iter()
            .copied()
            .filter(|m| {
                lower_annotations(&m.annotations)
                    .map(|l| l.has_key())
                    .unwrap_or(false)
            })
            .collect();
        let use_md5 = zerodds_idl::keyhash::uses_md5(&key_members, structs, typedefs);
        let mut key_segs: Vec<String> = Vec::new();
        for f in &zdkeys {
            match &f.key_type {
                Some((ts, expr)) => {
                    key_segs.extend(map_key_type(
                        ts,
                        expr,
                        pkg,
                        enum_names,
                        struct_names,
                        structs,
                        typedefs,
                    )?);
                }
                None => key_segs.push(f.seg.clone()),
            }
        }
        let _ = writeln!(out, "\n  def key_hash(%__MODULE__{{}} = v) do");
        let _ = writeln!(out, "    b =");
        let _ = writeln!(out, "      {wire}.writer(:big)");
        for seg in &key_segs {
            let _ = writeln!(out, "      |> {seg}");
        }
        let _ = writeln!(out, "      |> {wire}.bytes()");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "    :erlang.md5(b)");
        } else {
            let _ = writeln!(out, "    binary_part(b <> <<0::128>>, 0, 16)");
        }
        let _ = writeln!(out, "  end");
    }

    // Decode (inverse of marshal_into). The reader is threaded functionally:
    // each field is `{name, r} = <get>`, then the struct is built. @final reads
    // inline, @appendable skips the DHEADER, @mutable skips DHEADER then per
    // member EMHEADER + NEXTINT (members in declaration order).
    //
    // `@optional` (final/appendable): read the u8 presence flag, then the value
    // only when present — the exact inverse of the encode step. `@optional`
    // under @mutable rides the existing naive positional decoder (it reads every
    // member's EMHEADER+NEXTINT in declaration order and does not skip an omitted
    // member), so it round-trips a *present* optional only; a member absent on
    // the wire would misalign this decoder — same limitation the sibling
    // backends carry for @mutable decode.
    let struct_fields = fields
        .iter()
        .filter(|f| !f.non_serialized)
        .flat_map(|f| {
            if f.optional {
                vec![
                    format!("{n}_present: {n}_present", n = f.name),
                    format!("{n}: {n}", n = f.name),
                ]
            } else {
                vec![format!("{n}: {n}", n = f.name)]
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "\n  def read(r) do");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1: read the whole PL_CDR1 member list into an id→body map, then
        // decode each field from its own member-relative XCDR1 reader. An absent
        // id leaves the field nil (and clears its @optional presence flag) — the
        // correct omitted-member behaviour (better than the XCDR2 positional
        // path below, which cannot skip an omitted member).
        let _ = writeln!(out, "    if r.xcdr1 do");
        let _ = writeln!(out, "      ze = r.endian");
        let _ = writeln!(out, "      {{zdpl, r}} = {wire}.read_pl_cdr1_all(r)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let g = f.get.replace("$r", &format!("{wire}.reader1(zdbody, ze)"));
            let _ = writeln!(out, "      {n} =", n = f.name);
            let _ = writeln!(out, "        case Map.get(zdpl, {}) do", f.id);
            let _ = writeln!(out, "          nil -> nil");
            let _ = writeln!(out, "          zdbody -> {{zdv, _zdr}} = ({g}); zdv");
            let _ = writeln!(out, "        end");
            if f.optional {
                let _ = writeln!(
                    out,
                    "      {n}_present = Map.has_key?(zdpl, {})",
                    f.id,
                    n = f.name
                );
            }
        }
        let _ = writeln!(out, "      {{%__MODULE__{{{struct_fields}}}, r}}");
        let _ = writeln!(out, "    else");
        // XCDR2: skip the DHEADER, then per member skip EMHEADER + NEXTINT and
        // read (members in declaration order).
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        for f in &fields {
            if f.non_serialized {
                continue; // off the wire; defstruct default (nil) in the struct.
            }
            let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
            let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
            let _ = writeln!(out, "    {{{}, r}} = {}", f.name, f.get.replace("$r", "r"));
            if f.optional {
                let _ = writeln!(out, "    {}_present = true", f.name);
            }
        }
        let _ = writeln!(out, "    {{%__MODULE__{{{struct_fields}}}, r}}");
        let _ = writeln!(out, "    end");
    } else {
        if ext == ExtensibilityKind::Appendable {
            // XCDR2 frames the appendable member block with a DHEADER; XCDR1
            // classic CDR has none.
            let _ = writeln!(
                out,
                "    {{_, r}} = if r.xcdr1, do: {{0, r}}, else: {wire}.get_u32(r)"
            );
        }
        for f in &fields {
            if f.non_serialized {
                continue; // off the wire; defstruct default (nil) in the struct.
            }
            if f.optional {
                let _ = writeln!(out, "    {{{}_present, r}} = {wire}.get_bool(r)", f.name);
                let _ = writeln!(
                    out,
                    "    {{{n}, r}} = if {n}_present, do: ({g}), else: {{nil, r}}",
                    n = f.name,
                    g = f.get.replace("$r", "r")
                );
            } else {
                let _ = writeln!(out, "    {{{}, r}} = {}", f.name, f.get.replace("$r", "r"));
            }
        }
        let _ = writeln!(out, "    {{%__MODULE__{{{struct_fields}}}, r}}");
    }
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal_xcdr1(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader1(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "  ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    Ok(())
}

/// Maps an IDL type to a `Wire` pipe segment (e.g. `put_u32(v.id)`).
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

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Builds a map pipe segment: sort entries by key, then `u32 count` + key/value
/// pairs (DHEADER-framed unless the key/value pair is primitive).
fn build_map_seg(
    expr: &str,
    pkg: &str,
    key_seg: &str,
    val_seg: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> String {
    let wire = format!("{pkg}.Wire");
    let reduce = format!(
        "Enum.reduce(Enum.sort(Map.to_list({expr})), %REPL%, fn {{zdKk, zdKv}}, acc -> acc |> {key_seg} |> {val_seg} end)"
    );
    // Bounded `map<K, V, N>` (DDS-XTypes §7.4.3): over-bound = encode error,
    // checked before any bytes are written (same `if cond, do: raise(...);`
    // fall-through idiom as the bounded-string/-sequence checks above).
    let bound_check = match bound.and_then(array_size) {
        Some(bv) => format!(
            "if map_size({expr}) > {bv}, do: raise(ArgumentError, \"bounded map length exceeds its IDL bound ({bv})\")\n      "
        ),
        None => String::new(),
    };
    if prim {
        let body = reduce.replace("%REPL%", "w");
        format!(
            "then(fn w ->\n      {bound_check}w = {wire}.put_u32(w, map_size({expr}))\n      {body}\n    end)"
        )
    } else {
        let body = reduce.replace("%REPL%", "zdBody");
        format!(
            "then(fn w ->\n      {bound_check}zdBody = {wire}.subwriter(w) |> {wire}.put_u32(map_size({expr}))\n      zdBody = {body}\n      zdBB = {wire}.bytes(zdBody)\n      w |> {wire}.put_u32(byte_size(zdBB)) |> {wire}.put_bytes(zdBB)\n    end)"
        )
    }
}

/// zerodds-lint: recursion-depth 32
fn map_type(
    t: &TypeSpec,
    expr: &str,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => Ok(format!("{pkg}.Wire.{}", map_primitive(*p, expr)?)),
        // Bounded `string<N>`/`wstring<N>` (DDS-XTypes §7.4.3): reject
        // over-bound on encode like the other backends do. `then/1` is
        // already the codebase's idiom for wrapping a pipe step in extra
        // logic (see the `sequence<struct>` case below), so the bound
        // check rides the same shape: validate, then apply the plain
        // `put_string`/`put_wstring` step to the piped writer.
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "then(fn zw -> if byte_size({expr}) > {bv}, do: raise(ArgumentError, \"bounded string length exceeds its IDL bound ({bv})\"); zw |> {pkg}.Wire.put_string({expr}) end)"
            )),
            None => Ok(format!("{pkg}.Wire.put_string({expr})")),
        },
        // B1 blocker fix (deep review of #22 decode-bounds-cross-backend):
        // `String.length/1` counts Elixir codepoints, NOT UTF-16 code units —
        // for a non-BMP codepoint (e.g. an emoji) that is 1 codepoint but 2
        // UTF-16 units on the wire, so the old check under-counted and could
        // let a wire-over-bound wstring pass. XTypes 1.3 §7.4.3's `wstring<N>`
        // bound is in UTF-16 code units; count them the same way
        // `Wire.put_wstring` itself does (round-trip through UTF-16 and take
        // `byte_size / 2`), matching the D/Go/Julia backends.
        TypeSpec::String(st) => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "then(fn zw -> if div(byte_size(:unicode.characters_to_binary({expr}, :utf8, {{:utf16, :big}})), 2) > {bv}, do: raise(ArgumentError, \"bounded wstring length exceeds its IDL bound ({bv})\"); zw |> {pkg}.Wire.put_wstring({expr}) end)"
            )),
            None => Ok(format!("{pkg}.Wire.put_wstring({expr})")),
        },
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            pkg,
            enum_names,
            struct_names,
        ),
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Elixir field holds the BCD bytes directly (built by `Pkg.Fixed.enc/3`),
        // so the put is a plain byte splice.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(format!("{pkg}.Wire.put_bytes({expr})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // Elixir bitstrings write signed/unsigned identically (2's compl).
                let (put, _get, _bits) = bit_storage((enum_wire_width(&name) * 8) as usize);
                Ok(format!("{pkg}.Wire.{put}({expr})"))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                // Nested struct / bit-holder member: marshal into the piped writer.
                let esc = escape_elixir_ident(&name);
                Ok(format!("{pkg}.{esc}.marshal_into({expr})"))
            } else {
                Err(IdlElixirError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: sorted ascending by key, `u32 count` + key/value pairs (no DHEADER
        // for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let key_seg = map_type(&m.key, "zdKk", pkg, enum_names, struct_names)?;
            let val_seg = map_type(&m.value, "zdKv", pkg, enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok(build_map_seg(
                expr,
                pkg,
                &key_seg,
                &val_seg,
                prim,
                m.bound.as_ref(),
            ))
        }
        other => Err(IdlElixirError::Unsupported(format!("type {other:?}"))),
    }
}

/// Maps a `@key` member's type to zero or more `KeyHash`-body pipe segments.
///
/// Unlike [`map_type`] — shared with normal (non-key) member encoding, where a
/// struct-typed member always emits the struct's FULL `marshal_into` — a
/// nested-struct `@key` member must expand into only *that* struct's own
/// `@key` members (or ALL of its members if it declares none), in member-id
/// order (XTypes 1.3 §7.6.8 step 3). So this function intercepts only the
/// nested-struct case and recurses; every other type (primitive/string/enum/
/// sequence/map, and typedefs already dealiased by the caller) reuses
/// `map_type` unchanged — reusing it there is safe because those arms only
/// ever encode the value at `expr` itself, not a struct's full member set.
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded
/// by the IDL's aggregate nesting depth).
fn map_key_type(
    t: &TypeSpec,
    expr: &str,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<Vec<String>> {
    if let TypeSpec::Scoped(sn) = t {
        let name = resolve_scoped_name(sn);
        if let Some(sd) = structs.get(&name) {
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
            // A nested-key struct's own members order by their resolved wire id
            // (XTypes §7.6.8 step 3) — honoring `@id`/`@hashid`/`@autoid(HASH)`
            // via the shared resolver, not just explicit `@id`, so the KeyHash
            // body ordering matches idl-rust/the TypeObject.
            let nested_autoid =
                zerodds_idl::semantics::member_id::container_autoid_hash(&sd.annotations);
            let mut ordered: Vec<(u32, &Member)> = effective
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let raw = m
                        .declarators
                        .first()
                        .map(|d| d.name().text.clone())
                        .unwrap_or_default();
                    let id = zerodds_idl::semantics::member_id::fixed_member_id(
                        nested_autoid,
                        &m.annotations,
                        &raw,
                    )
                    .unwrap_or(idx as u32);
                    (id, *m)
                })
                .collect();
            ordered.sort_by_key(|(id, _)| *id);
            let mut segs = Vec::new();
            for (_, m) in &ordered {
                for decl in &m.declarators {
                    // Arrays of nested-key structs are out of the proof scope
                    // (matches the `idl-rust` reference); reject explicitly
                    // rather than silently dropping dimensions.
                    if matches!(decl, Declarator::Array(_)) {
                        return Err(IdlElixirError::Unsupported(
                            "array @key field inside a nested-struct key".to_string(),
                        ));
                    }
                    let field = decl.name().text.clone();
                    let sub_expr = format!("{expr}.{field}");
                    let resolved_m = resolve_typedef(&m.type_spec, typedefs);
                    segs.extend(map_key_type(
                        &resolved_m,
                        &sub_expr,
                        pkg,
                        enum_names,
                        struct_names,
                        structs,
                        typedefs,
                    )?);
                }
            }
            return Ok(segs);
        }
    }
    Ok(vec![map_type(t, expr, pkg, enum_names, struct_names)?])
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<String> {
    let seg = match p {
        PrimitiveType::Octet | PrimitiveType::Char => format!("put_u8({expr})"),
        PrimitiveType::Boolean => format!("put_bool({expr})"),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => format!("put_f32({expr})"),
        PrimitiveType::Floating(FloatingType::Double) => format!("put_f64({expr})"),
        PrimitiveType::Floating(FloatingType::LongDouble) => format!("put_long_double({expr})"),
        PrimitiveType::WideChar => format!("put_u32({expr})"),
    };
    Ok(seg)
}

fn map_integer(i: IntegerType, expr: &str) -> Result<String> {
    // Elixir bitstrings encode signed and unsigned identically in 2's complement.
    let seg = match i {
        IntegerType::Int8 | IntegerType::UInt8 => format!("put_u8({expr})"),
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            format!("put_u16({expr})")
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            format!("put_u32({expr})")
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => format!("put_u64({expr})"),
    };
    Ok(seg)
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode
    // error, checked ahead of the element writes below (both supported
    // element kinds fall through to a `raise` prefix on the check-fail
    // branch of an `if`, which — with no `else` — evaluates to `nil` and
    // falls through to the next statement on success, same idiom as the
    // bounded-string check above).
    let bound_check = match bound.and_then(array_size) {
        Some(bv) => format!(
            "if length({expr}) > {bv}, do: raise(ArgumentError, \"bounded sequence length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        if bound_check.is_empty() {
            return Ok(format!("{pkg}.Wire.put_seq_u8({expr})"));
        }
        return Ok(format!(
            "then(fn zw -> {bound_check}zw |> {pkg}.Wire.put_seq_u8({expr}) end)"
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element (XTypes
    // §7.4.3.5.3). A one-line `then/2` keeps the pipe threading the writer.
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let esc = escape_elixir_ident(&name);
            let seg = format!(
                "then(fn w -> {bound_check}sub = {pkg}.Wire.subwriter(w) |> {pkg}.Wire.put_u32(length({expr}));                  sub = Enum.reduce({expr}, sub, fn e, acc -> {pkg}.{esc}.marshal_into(acc, e) end);                  body = {pkg}.Wire.bytes(sub);                  w |> {pkg}.Wire.put_u32(byte_size(body)) |> {pkg}.Wire.put_bytes(body) end)"
            );
            return Ok(seg);
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases reached here). Mirrors the
    // `idl-go`/`idl-d` fallback.
    let elem_seg = map_type(elem, "zdElem", pkg, enum_names, struct_names)?;
    Ok(format!(
        "then(fn zw ->\n      {bound_check}zw = {pkg}.Wire.put_u32(zw, length({expr}))\n      Enum.reduce({expr}, zw, fn zdElem, zdAcc -> zdAcc |> {elem_seg} end)\n    end)"
    ))
}

// ---- decode (inverse of the put path): a `Wire` reader in the module, plus
// `map_get` — the inverse of `map_type` — returning an EXPRESSION (placeholder
// `$r` for the reader) that evaluates to `{value, reader}`. Elixir is immutable,
// so the reader is threaded functionally. Roundtrip-verified.

/// Reads a fixed array: nested `Enum.reduce`s threading the reader, building
/// (list-of-)list(s) in row-major order (inverse of [`build_array_seg`]).
/// `elem_get` is an expression (placeholder `$r`) reading one element.
fn build_array_get(sizes: &[i64], elem_get: &str) -> String {
    /// zerodds-lint: recursion-depth 32
    fn rec(sizes: &[i64], depth: usize, elem_get: &str) -> String {
        let s = sizes[depth];
        let rvar = format!("zr{depth}");
        let acc = format!("zacc{depth}");
        if depth + 1 == sizes.len() {
            let eg = elem_get.replace("$r", &rvar);
            format!(
                "Enum.reduce(1..{s}//1, {{[], $R}}, fn _, {{{acc}, {rvar}}} -> {{zel, {rvar}}} = {eg}; {{[zel | {acc}], {rvar}}} end)"
            )
        } else {
            let inner = rec(sizes, depth + 1, elem_get).replace("$R", &rvar);
            format!(
                "Enum.reduce(1..{s}//1, {{[], $R}}, fn _, {{{acc}, {rvar}}} -> {{zrow, {rvar}}} = {inner}; {{[Enum.reverse(zrow) | {acc}], {rvar}}} end)"
            )
        }
    }
    // The outer reduce reads from `$r`; wrap to reverse the accumulated list.
    let body = rec(sizes, 0, elem_get).replace("$R", "$r");
    format!("(\n      {{zlst, zr}} = {body}\n      {{Enum.reverse(zlst), zr}}\n    )")
}

/// Returns an expression (placeholder `$r`) reading one value of IDL type `t`,
/// evaluating to `{value, reader}`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let wire = format!("{pkg}.Wire");
    match t {
        TypeSpec::Primitive(p) => Ok(format!("{wire}.{}", map_get_primitive(*p)?)),
        // B1 follow-up (#22 decode-side parity): mirror the encode-side
        // bound check (`map_type` above) on decode too — XTypes 1.3 §7.4.3
        // requires the IDL bound enforced on BOTH sides, not just whatever
        // wire-format validation `get_string`/`get_wstring` already does.
        // The parenthesized-statement-block idiom already used by
        // `build_array_get` above keeps this a single expression evaluating
        // to `{value, reader}`, so it drops into `map_get`'s single-expr
        // contract unchanged.
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "(\n      {{zdv, zdr}} = {wire}.get_string($r)\n      if byte_size(zdv) > {bv}, do: raise(ArgumentError, \"decoded string length exceeds its IDL bound ({bv})\")\n      {{zdv, zdr}}\n    )"
            )),
            None => Ok(format!("{wire}.get_string($r)")),
        },
        // B1 blocker fix (deep review of #22 decode-bounds-cross-backend):
        // count UTF-16 code units (not Elixir codepoints via `String.length`)
        // — see the encode-side comment above for why.
        TypeSpec::String(st) => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "(\n      {{zdv, zdr}} = {wire}.get_wstring($r)\n      if div(byte_size(:unicode.characters_to_binary(zdv, :utf8, {{:utf16, :big}})), 2) > {bv}, do: raise(ArgumentError, \"decoded wstring length exceeds its IDL bound ({bv})\")\n      {{zdv, zdr}}\n    )"
            )),
            None => Ok(format!("{wire}.get_wstring($r)")),
        },
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), pkg, enum_names, struct_names)
        }
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{wire}.get_bytes_n($r, {n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide holder (XTypes 1.3 §7.4.5.1).
                let (_put, get, _bits) = bit_storage((enum_wire_width(&name) * 8) as usize);
                Ok(format!("{wire}.{get}($r)"))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                let esc = escape_elixir_ident(&name);
                Ok(format!("{pkg}.{esc}.read($r)"))
            } else {
                Err(IdlElixirError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let key_get = map_get(&m.key, pkg, enum_names, struct_names)?.replace("$r", "zrr");
            let val_get = map_get(&m.value, pkg, enum_names, struct_names)?.replace("$r", "zrr");
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim {
                String::new()
            } else {
                format!("{{_, zr}} = {wire}.get_u32(zr)\n      ")
            };
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check (`build_map_seg` above) — XTypes 1.3 §7.4.3.
            let bound_check = match m.bound.as_ref().and_then(array_size) {
                Some(bv) => format!(
                    "if zn > {bv}, do: raise(ArgumentError, \"decoded map length exceeds its IDL bound ({bv})\")\n      "
                ),
                None => String::new(),
            };
            Ok(format!(
                "(\n      zr = $r\n      {dh}{{zn, zr}} = {wire}.get_u32(zr)\n      {bound_check}{{zpairs, zr}} = Enum.reduce(1..zn//1, {{[], zr}}, fn _, {{zacc, zrr}} -> {{zk, zrr}} = {key_get}; {{zv, zrr}} = {val_get}; {{[{{zk, zv}} | zacc], zrr}} end)\n      {{Map.new(zpairs), zr}}\n    )"
            ))
        }
        other => Err(IdlElixirError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType) -> Result<String> {
    let g = match p {
        PrimitiveType::Octet | PrimitiveType::Char => "get_u8($r)",
        PrimitiveType::Boolean => "get_bool($r)",
        PrimitiveType::Integer(i) => return map_get_integer(i),
        PrimitiveType::Floating(FloatingType::Float) => "get_f32($r)",
        PrimitiveType::Floating(FloatingType::Double) => "get_f64($r)",
        PrimitiveType::Floating(FloatingType::LongDouble) => "get_long_double($r)",
        PrimitiveType::WideChar => "get_u32($r)",
    };
    Ok(g.to_string())
}

fn map_get_integer(i: IntegerType) -> Result<String> {
    let g = match i {
        IntegerType::Int8 | IntegerType::UInt8 => "get_u8($r)",
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            "get_u16($r)"
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            "get_u32($r)"
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => "get_u64($r)",
    };
    Ok(g.to_string())
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let wire = format!("{pkg}.Wire");
    let bv = bound.and_then(array_size);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        // B1 follow-up (#22 decode-side parity): `get_seq_u8` returns the
        // decoded binary directly (its own count is not exposed here), so
        // the bound check runs post-decode on the result's byte size —
        // mirrors the encode-side `length(expr) > N` check in `map_sequence`
        // above (a `sequence<octet>` byte count IS its element count).
        return Ok(match bv {
            Some(bv) => format!(
                "(\n      {{zdv, zdr}} = {wire}.get_seq_u8($r)\n      if byte_size(zdv) > {bv}, do: raise(ArgumentError, \"decoded sequence length exceeds its IDL bound ({bv})\")\n      {{zdv, zdr}}\n    )"
            ),
            None => format!("{wire}.get_seq_u8($r)"),
        });
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let bound_check = match bv {
                Some(bv) => format!(
                    "if zn > {bv}, do: raise(ArgumentError, \"decoded sequence length exceeds its IDL bound ({bv})\")\n      "
                ),
                None => String::new(),
            };
            let esc = escape_elixir_ident(&name);
            return Ok(format!(
                "(\n      zr = $r\n      {{_, zr}} = {wire}.get_u32(zr)\n      {{zn, zr}} = {wire}.get_u32(zr)\n      {bound_check}{{zlst, zr}} = Enum.reduce(1..zn//1, {{[], zr}}, fn _, {{zacc, zrr}} -> {{ze, zrr}} = {pkg}.{esc}.read(zrr); {{[ze | zacc], zrr}} end)\n      {{Enum.reverse(zlst), zr}}\n    )"
            ));
        }
    }
    // sequence<arbitrary> → u32 count + per-element decode (no DHEADER; inverse
    // of the `map_sequence` arbitrary fallback).
    let bound_check = match bv {
        Some(bv) => format!(
            "if zn > {bv}, do: raise(ArgumentError, \"decoded sequence length exceeds its IDL bound ({bv})\")\n      "
        ),
        None => String::new(),
    };
    let elem_get = map_get(elem, pkg, enum_names, struct_names)?.replace("$r", "zrr");
    Ok(format!(
        "(\n      zr = $r\n      {{zn, zr}} = {wire}.get_u32(zr)\n      {bound_check}{{zlst, zr}} = Enum.reduce(1..zn//1, {{[], zr}}, fn _, {{zacc, zrr}} -> {{ze, zrr}} = {elem_get}; {{[ze | zacc], zrr}} end)\n      {{Enum.reverse(zlst), zr}}\n    )"
    ))
}
