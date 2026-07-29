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
    CaseLabel, ConstExpr, ConstrTypeDecl, Declarator, Definition, EnumDef, FloatingType,
    IntegerType, Literal, LiteralKind, Member, PrimitiveType, SequenceType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_single,
};

use crate::error::{IdlElixirError, Result};
use crate::keywords::escape_elixir_ident;

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
  def writer(endian), do: %{buf: <<>>, endian: endian}

  defp align(w, a) do
    cap = min(a, 4)
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
  def put_u64(w, v), do: put(w, 4, <<v::little-64>>)
  def put_f32(w, v), do: put(w, 4, <<v::float-little-32>>)
  def put_f64(w, v), do: put(w, 4, <<v::float-little-64>>)
  def put_bytes(w, b), do: %{w | buf: w.buf <> b}

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
    put(w, 4, rev(be))
  end

  def bytes(w), do: w.buf

  # Reader: {bin, pos, endian}; each get returns {value, reader}. Alignment is
  # stream-relative (tracked via pos), inverse of the Writer's byte_size(buf).
  def reader(bin, endian), do: %{bin: bin, pos: 0, endian: endian}

  defp ralign(r, a) do
    cap = min(a, 4)
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
    r = ralign(r, 4)
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
    r = ralign(r, 4)
    bin = :binary.part(r.bin, r.pos, 16)
    be = if r.endian == :big, do: bin, else: rev(bin)
    <<sign::1, exp128::15, mant::52, _::60>> = be
    exp = if exp128 == 0 and mant == 0, do: 0, else: exp128 - 16383 + 1023
    <<v::float-64>> = <<sign::1, exp::11, mant::52>>
    {v, %{r | pos: r.pos + 16}}
  end
end
"#;

/// Generates a self-contained Elixir module from the IDL AST.
///
/// # Errors
/// Returns [`IdlElixirError::Unsupported`] for constructs the Elixir backend
/// does not yet emit (unions, nested-struct members, maps, `long double`,
/// `@mutable`, …).
pub fn generate_elixir_module(spec: &Specification, opts: &ElixirGenOptions) -> Result<String> {
    let pkg = &opts.module_name;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Elixir backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "# SPDX-License-Identifier: Apache-2.0\n");
    out.push_str(&WIRE_MODULE.replace("__PKG__", pkg));

    // `module X { ... }` content is promoted into the same flat, top-level
    // definition list (see `flatten_module_defs`) so it is no longer
    // silently dropped (swarm59 #21b).
    let flat = flatten_module_defs(&spec.definitions);

    // Named enums: an enum member is a 32-bit signed integer on the wire
    // (XTypes 1.3 §7.4.5.1), byte-identical to the int32/uint32 path.
    let enum_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(e.name.text.clone())
            }
            _ => None,
        })
        .collect();

    let struct_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some(s.name.text.clone())
            }
            _ => None,
        })
        .collect();

    // Name -> StructDef, so a nested-struct `@key` member's own `@key` subset
    // (and `keyhash::uses_md5`'s static max-size analysis) can be resolved —
    // mirrors `struct_names` above, just keeping the full def instead of only
    // the name.
    let structs: HashMap<String, &StructDef> = spec
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some((s.name.text.clone(), s))
            }
            _ => None,
        })
        .collect();

    let typedefs = collect_typedefs(spec);

    for def in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                emit_enum(&mut out, e, pkg);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(
                    &mut out,
                    s,
                    pkg,
                    &enum_names,
                    &struct_names,
                    &structs,
                    &typedefs,
                )?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(&mut out, u, pkg, &enum_names, &struct_names, &typedefs)?;
            }
            _ => {}
        }
    }
    Ok(out)
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

/// Emits an IDL `union` as an Elixir struct (discriminator + one field per case
/// member) with a pipe-compatible `marshal_into` that puts the discriminator
/// then a `case` dispatches to the selected member (XCDR2 §7.4.3.5.4).
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    let ext = extensibility_union(u);
    if ext == ExtensibilityKind::Mutable {
        return Err(IdlElixirError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
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
    struct ElixirCase {
        labels: Vec<i64>,
        is_default: bool,
        field: String,
        seg: String,
        get: String,
    }
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
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlElixirError::Unsupported(format!(
                        "non-integer union label in `{}`",
                        u.name.text
                    ))
                })?),
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

    let ty = escape_elixir_ident(&u.name.text);
    let wire = format!("{pkg}.Wire");
    let mut names = vec![":disc".to_string()];
    names.extend(cases.iter().map(|c| format!(":{}", c.field)));
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct [{}]", names.join(", "));

    let _ = writeln!(out, "\n  def marshal_into(w, %__MODULE__{{}} = v) do");
    // A pattern for a case's labels: `1` for one, `d when d in [1, 3]` for many.
    let pat = |c: &ElixirCase| -> String {
        if c.is_default {
            "_".to_string()
        } else if c.labels.len() == 1 {
            c.labels[0].to_string()
        } else {
            let list = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("d when d in [{list}]")
        }
    };
    if ext == ExtensibilityKind::Final {
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
        let _ = writeln!(out, "    body_w = {wire}.writer(w.endian) |> {disc_seg}");
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
    }
    let _ = writeln!(out, "  end");

    let _ = writeln!(out, "\n  def marshal_xcdr(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(
        out,
        "    {wire}.writer(endian) |> marshal_into(v) |> {wire}.bytes()"
    );
    let _ = writeln!(out, "  end");

    // Decode: read the discriminator, then a `case` reads only the selected
    // member and builds the struct (@appendable skips the leading DHEADER).
    let _ = writeln!(out, "\n  def read(r) do");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
    }
    let _ = writeln!(out, "    {{disc, r}} = {}", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "    {{v, r}} =");
    let _ = writeln!(out, "      case disc do");
    for c in &cases {
        let g = c.get.replace("$r", "r");
        let _ = writeln!(out, "        {} ->", pat(c));
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
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "end");
    Ok(())
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
fn emit_enum(out: &mut String, e: &EnumDef, pkg: &str) {
    let values = enumerator_values(e);
    let ty = escape_elixir_ident(&e.name.text);
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let name = escape_elixir_ident(&en.name.text.to_lowercase());
        let _ = writeln!(out, "  def {name}, do: {value}");
    }
    let _ = writeln!(out, "end");
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
/// uses for type-reference resolution (`sn.parts.last()` below) — module
/// content is no longer silently dropped (swarm59 #21b), it is simply not
/// namespaced: two same-named types in different modules collide, exactly
/// as two same-named top-level types would.
///
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs(defs: &[Definition]) -> Vec<&Definition> {
    let mut out = Vec::new();
    flatten_module_defs_into(defs, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs_into<'a>(defs: &'a [Definition], out: &mut Vec<&'a Definition>) {
    for d in defs {
        match d {
            Definition::Module(m) => flatten_module_defs_into(&m.definitions, out),
            other => out.push(other),
        }
    }
}

/// Collects `typedef` aliases (simple declarators) as name -> aliased type-spec.
/// A typedef is wire-transparent, so members are resolved to the underlying
/// type before mapping (`typedef long Score; Score s;` marshals as `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    for def in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Typedef(td)) = def {
            for d in &td.declarators {
                if let Declarator::Simple(name) = d {
                    m.insert(name.text.clone(), td.type_spec.clone());
                }
            }
        }
    }
    m
}

/// Resolves a typedef chain to its underlying type-spec (recursing into
/// sequence elements). Non-typedef types pass through unchanged.
///
/// zerodds-lint: recursion-depth 32 (typedef alias chains + nested sequence
/// elements; bounded by the IDL's alias/collection nesting depth).
fn resolve_typedef(t: &TypeSpec, typedefs: &HashMap<String, TypeSpec>) -> TypeSpec {
    match t {
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
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

fn emit_struct(
    out: &mut String,
    s: &StructDef,
    pkg: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
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
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let name = escape_elixir_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
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
            });
        }
    }

    let ty = escape_elixir_ident(&s.name.text);
    let wire = format!("{pkg}.Wire");
    let names: Vec<String> = fields.iter().map(|f| format!(":{}", f.name)).collect();
    let _ = writeln!(out, "\ndefmodule {pkg}.{ty} do");
    let _ = writeln!(out, "  defstruct [{}]", names.join(", "));

    // marshal_into writes the struct into an existing writer (pipe-compatible:
    // takes the writer first, returns it) so nested composites keep stream-
    // relative alignment. @final: fields inline; @appendable: DHEADER body.
    let _ = writeln!(out, "\n  def marshal_into(w, %__MODULE__{{}} = v) do");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "    body = {wire}.writer(w.endian)");
        for (i, f) in fields.iter().enumerate() {
            let emh = 0x4000_0000_u32 | f.id;
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
        let _ = writeln!(out, "    body_bytes = {wire}.bytes(body)");
        let _ = writeln!(
            out,
            "    w |> {wire}.put_u32(byte_size(body_bytes)) |> {wire}.put_bytes(body_bytes)"
        );
    } else if ext == ExtensibilityKind::Final {
        let _ = writeln!(out, "    w");
        for f in &fields {
            let _ = writeln!(out, "    |> {}", f.seg);
        }
    } else {
        let _ = writeln!(out, "    body =");
        let _ = writeln!(out, "      {wire}.writer(w.endian)");
        for f in &fields {
            let _ = writeln!(out, "      |> {}", f.seg);
        }
        let _ = writeln!(out, "      |> {wire}.bytes()");
        let _ = writeln!(out, "    w");
        let _ = writeln!(out, "    |> {wire}.put_u32(byte_size(body))");
        let _ = writeln!(out, "    |> {wire}.put_bytes(body)");
    }
    let _ = writeln!(out, "  end");

    let _ = writeln!(out, "\n  def marshal_xcdr(%__MODULE__{{}} = v, endian) do");
    let _ = writeln!(out, "    {wire}.writer(endian)");
    let _ = writeln!(out, "    |> marshal_into(v)");
    let _ = writeln!(out, "    |> {wire}.bytes()");
    let _ = writeln!(out, "  end");

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
    let _ = writeln!(out, "\n  def read(r) do");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        for f in &fields {
            let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
            let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
            let _ = writeln!(out, "    {{{}, r}} = {}", f.name, f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "    {{_, r}} = {wire}.get_u32(r)");
        }
        for f in &fields {
            let _ = writeln!(out, "    {{{}, r}} = {}", f.name, f.get.replace("$r", "r"));
        }
    }
    let struct_fields = fields
        .iter()
        .map(|f| format!("{n}: {n}", n = f.name))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "    {{%__MODULE__{{{struct_fields}}}, r}}");
    let _ = writeln!(out, "  end");
    let _ = writeln!(out, "\n  def unmarshal(bin, endian) do");
    let _ = writeln!(out, "    {{v, _r}} = read({wire}.reader(bin, endian))");
    let _ = writeln!(out, "    v");
    let _ = writeln!(out, "  end");
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
            enum_names.contains(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default())
        }
        _ => false,
    }
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
            "then(fn w ->\n      {bound_check}zdBody = {wire}.writer(w.endian) |> {wire}.put_u32(map_size({expr}))\n      zdBody = {body}\n      zdBB = {wire}.bytes(zdBody)\n      w |> {wire}.put_u32(byte_size(zdBB)) |> {wire}.put_bytes(zdBB)\n    end)"
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
        TypeSpec::Sequence(seq) => {
            map_sequence(&seq.elem, seq.bound.as_ref(), expr, pkg, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                // Enum member: 32-bit signed integer on the wire.
                Ok(format!("{pkg}.Wire.put_u32({expr})"))
            } else if struct_names.contains(&name) {
                // Nested struct member: marshal into the piped writer.
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
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
            let mut ordered: Vec<(u32, &Member)> = effective
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let id = lower_annotations(&m.annotations)
                        .ok()
                        .and_then(|l| l.explicit_id())
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

fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    pkg: &str,
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let esc = escape_elixir_ident(&name);
            let seg = format!(
                "then(fn w -> {bound_check}sub = {pkg}.Wire.writer(w.endian) |> {pkg}.Wire.put_u32(length({expr}));                  sub = Enum.reduce({expr}, sub, fn e, acc -> {pkg}.{esc}.marshal_into(acc, e) end);                  body = {pkg}.Wire.bytes(sub);                  w |> {pkg}.Wire.put_u32(byte_size(body)) |> {pkg}.Wire.put_bytes(body) end)"
            );
            return Ok(seg);
        }
    }
    Err(IdlElixirError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
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
            map_get_sequence(&seq.elem, seq.bound.as_ref(), pkg, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!("{wire}.get_u32($r)"))
            } else if struct_names.contains(&name) {
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

fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    pkg: &str,
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
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
    Err(IdlElixirError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
