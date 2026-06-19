// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST Pretty-Print (T5.3).
//!
//! Implements [`fmt::Display`] for all AST root types. The output is
//! valid IDL — re-parsable to an equivalent AST (roundtrip secured via
//! T5.6).
//!
//! # Format convention
//! - 4-space indent
//! - block bodies with `{` on the same line, `}` standalone
//! - annotations before the annotated construct, one per line for
//!   top-level decls, inline for members/params
//! - no blank lines between top-level definitions (roundtrip stability)

use core::fmt::{self, Write};

use crate::ast::types::*;

const INDENT: &str = "    ";

// ============================================================================
// Hilfsfunktionen
// ============================================================================

fn write_indent(f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    for _ in 0..depth {
        f.write_str(INDENT)?;
    }
    Ok(())
}

fn write_inline_annotations(f: &mut fmt::Formatter<'_>, anns: &[Annotation]) -> fmt::Result {
    for ann in anns {
        write!(f, "{ann} ")?;
    }
    Ok(())
}

fn write_block_annotations(
    f: &mut fmt::Formatter<'_>,
    anns: &[Annotation],
    depth: usize,
) -> fmt::Result {
    for ann in anns {
        write_indent(f, depth)?;
        writeln!(f, "{ann}")?;
    }
    Ok(())
}

// ============================================================================
// Specification + Definition
// ============================================================================

impl fmt::Display for Specification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, def) in self.definitions.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            print_definition(f, def, 0)?;
        }
        Ok(())
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn print_definition(f: &mut fmt::Formatter<'_>, def: &Definition, depth: usize) -> fmt::Result {
    match def {
        Definition::Module(m) => print_module(f, m, depth),
        Definition::Type(t) => {
            print_type_decl(f, t, depth)?;
            f.write_str(";\n")
        }
        Definition::Const(c) => {
            write_block_annotations(f, &c.annotations, depth)?;
            write_indent(f, depth)?;
            print_const_decl(f, c)?;
            f.write_str(";\n")
        }
        Definition::Except(e) => {
            write_block_annotations(f, &e.annotations, depth)?;
            print_except_decl(f, e, depth)?;
            f.write_str(";\n")
        }
        Definition::Interface(i) => {
            print_interface_dcl(f, i, depth)?;
            f.write_str(";\n")
        }
        Definition::ValueBox(v) => {
            write_block_annotations(f, &v.annotations, depth)?;
            write_indent(f, depth)?;
            write!(f, "valuetype {} ", v.name.text)?;
            print_type_spec(f, &v.type_spec)?;
            f.write_str(";\n")
        }
        Definition::ValueForward(v) => {
            write_indent(f, depth)?;
            writeln!(f, "valuetype {};", v.name.text)
        }
        Definition::Annotation(a) => print_annotation_dcl(f, a, depth),
        Definition::ValueDef(_)
        | Definition::TypeId(_)
        | Definition::TypePrefix(_)
        | Definition::Import(_)
        | Definition::Component(_)
        | Definition::Home(_)
        | Definition::Event(_)
        | Definition::Porttype(_)
        | Definition::Connector(_)
        | Definition::TemplateModule(_)
        | Definition::TemplateModuleInst(_) => {
            write_indent(f, depth)?;
            // Pretty-printing for CORBA/CCM/template constructs is
            // code-gen material; here only a marker for roundtrip
            // diagnostics.
            writeln!(f, "/* corba/ccm/template construct (round-trip stub) */")
        }
        Definition::VendorExtension(v) => {
            write_indent(f, depth)?;
            // Vendor extensions cannot be printed back generically
            // — we mark them as a comment for
            // roundtrip diagnostics. Real vendor-specific pretty-
            // printers come with the respective vendor adapter (T6+).
            writeln!(
                f,
                "/* vendor-extension {} (raw not preserved) */",
                v.production_name
            )
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn print_module(f: &mut fmt::Formatter<'_>, m: &ModuleDef, depth: usize) -> fmt::Result {
    write_block_annotations(f, &m.annotations, depth)?;
    write_indent(f, depth)?;
    writeln!(f, "module {} {{", m.name.text)?;
    for d in &m.definitions {
        print_definition(f, d, depth + 1)?;
    }
    write_indent(f, depth)?;
    f.write_str("};\n")
}

// ============================================================================
// Type-Decl
// ============================================================================

fn print_type_decl(f: &mut fmt::Formatter<'_>, td: &TypeDecl, depth: usize) -> fmt::Result {
    match td {
        TypeDecl::Constr(c) => print_constr_type_decl(f, c, depth),
        TypeDecl::Typedef(t) => {
            write_block_annotations(f, &t.annotations, depth)?;
            write_indent(f, depth)?;
            f.write_str("typedef ")?;
            print_type_spec(f, &t.type_spec)?;
            f.write_str(" ")?;
            for (i, d) in t.declarators.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                print_declarator(f, d)?;
            }
            Ok(())
        }
        TypeDecl::Native(n) => {
            write_indent(f, depth)?;
            f.write_str("native ")?;
            f.write_str(&n.name.text)
        }
    }
}

fn print_constr_type_decl(
    f: &mut fmt::Formatter<'_>,
    c: &ConstrTypeDecl,
    depth: usize,
) -> fmt::Result {
    match c {
        ConstrTypeDecl::Struct(s) => print_struct_dcl(f, s, depth),
        ConstrTypeDecl::Union(u) => print_union_dcl(f, u, depth),
        ConstrTypeDecl::Enum(e) => print_enum_def(f, e, depth),
        ConstrTypeDecl::Bitset(b) => print_bitset_decl(f, b, depth),
        ConstrTypeDecl::Bitmask(b) => print_bitmask_decl(f, b, depth),
    }
}

// ============================================================================
// Struct
// ============================================================================

fn print_struct_dcl(f: &mut fmt::Formatter<'_>, s: &StructDcl, depth: usize) -> fmt::Result {
    match s {
        StructDcl::Def(d) => {
            write_block_annotations(f, &d.annotations, depth)?;
            write_indent(f, depth)?;
            write!(f, "struct {}", d.name.text)?;
            if let Some(base) = &d.base {
                write!(f, " : {base}")?;
            }
            f.write_str(" {")?;
            if d.members.is_empty() {
                f.write_str("}")?;
            } else {
                f.write_str("\n")?;
                for m in &d.members {
                    print_member(f, m, depth + 1)?;
                }
                write_indent(f, depth)?;
                f.write_str("}")?;
            }
            Ok(())
        }
        StructDcl::Forward(d) => {
            write_indent(f, depth)?;
            write!(f, "struct {}", d.name.text)
        }
    }
}

fn print_member(f: &mut fmt::Formatter<'_>, m: &Member, depth: usize) -> fmt::Result {
    write_indent(f, depth)?;
    write_inline_annotations(f, &m.annotations)?;
    print_type_spec(f, &m.type_spec)?;
    f.write_str(" ")?;
    for (i, d) in m.declarators.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        print_declarator(f, d)?;
    }
    f.write_str(";\n")
}

// ============================================================================
// Union
// ============================================================================

fn print_union_dcl(f: &mut fmt::Formatter<'_>, u: &UnionDcl, depth: usize) -> fmt::Result {
    match u {
        UnionDcl::Def(d) => {
            write_block_annotations(f, &d.annotations, depth)?;
            write_indent(f, depth)?;
            write!(f, "union {} switch (", d.name.text)?;
            print_switch_type_spec(f, &d.switch_type)?;
            f.write_str(") {")?;
            if d.cases.is_empty() {
                f.write_str("}")?;
            } else {
                f.write_str("\n")?;
                for c in &d.cases {
                    print_case(f, c, depth + 1)?;
                }
                write_indent(f, depth)?;
                f.write_str("}")?;
            }
            Ok(())
        }
        UnionDcl::Forward(d) => {
            write_indent(f, depth)?;
            write!(f, "union {}", d.name.text)
        }
    }
}

fn print_switch_type_spec(f: &mut fmt::Formatter<'_>, s: &SwitchTypeSpec) -> fmt::Result {
    match s {
        SwitchTypeSpec::Integer(i) => print_integer_type(f, *i),
        SwitchTypeSpec::Char => f.write_str("char"),
        SwitchTypeSpec::Boolean => f.write_str("boolean"),
        SwitchTypeSpec::Octet => f.write_str("octet"),
        SwitchTypeSpec::Scoped(s) => write!(f, "{s}"),
    }
}

fn print_case(f: &mut fmt::Formatter<'_>, c: &Case, depth: usize) -> fmt::Result {
    write_indent(f, depth)?;
    write_inline_annotations(f, &c.annotations)?;
    for label in &c.labels {
        match label {
            CaseLabel::Default => f.write_str("default: ")?,
            CaseLabel::Value(e) => {
                f.write_str("case ")?;
                print_const_expr(f, e)?;
                f.write_str(": ")?;
            }
        }
    }
    print_element_spec(f, &c.element)?;
    f.write_str(";\n")
}

fn print_element_spec(f: &mut fmt::Formatter<'_>, e: &ElementSpec) -> fmt::Result {
    write_inline_annotations(f, &e.annotations)?;
    print_type_spec(f, &e.type_spec)?;
    f.write_str(" ")?;
    print_declarator(f, &e.declarator)
}

// ============================================================================
// Enum / Bitset / Bitmask
// ============================================================================

fn print_enum_def(f: &mut fmt::Formatter<'_>, e: &EnumDef, depth: usize) -> fmt::Result {
    write_block_annotations(f, &e.annotations, depth)?;
    write_indent(f, depth)?;
    write!(f, "enum {} {{", e.name.text)?;
    if e.enumerators.is_empty() {
        f.write_str("}")?;
    } else {
        f.write_str("\n")?;
        for (i, en) in e.enumerators.iter().enumerate() {
            write_indent(f, depth + 1)?;
            write_inline_annotations(f, &en.annotations)?;
            f.write_str(&en.name.text)?;
            if i + 1 < e.enumerators.len() {
                f.write_str(",")?;
            }
            f.write_str("\n")?;
        }
        write_indent(f, depth)?;
        f.write_str("}")?;
    }
    Ok(())
}

fn print_bitset_decl(f: &mut fmt::Formatter<'_>, b: &BitsetDecl, depth: usize) -> fmt::Result {
    write_block_annotations(f, &b.annotations, depth)?;
    write_indent(f, depth)?;
    write!(f, "bitset {}", b.name.text)?;
    if let Some(base) = &b.base {
        write!(f, " : {base}")?;
    }
    f.write_str(" {")?;
    if b.bitfields.is_empty() {
        f.write_str("}")?;
    } else {
        f.write_str("\n")?;
        for bf in &b.bitfields {
            print_bitfield(f, bf, depth + 1)?;
        }
        write_indent(f, depth)?;
        f.write_str("}")?;
    }
    Ok(())
}

fn print_bitfield(f: &mut fmt::Formatter<'_>, b: &Bitfield, depth: usize) -> fmt::Result {
    write_indent(f, depth)?;
    write_inline_annotations(f, &b.annotations)?;
    f.write_str("bitfield<")?;
    print_const_expr(f, &b.spec.width)?;
    if let Some(dt) = &b.spec.dest_type {
        f.write_str(", ")?;
        print_primitive_type(f, *dt)?;
    }
    f.write_str(">")?;
    if let Some(name) = &b.name {
        write!(f, " {}", name.text)?;
    }
    f.write_str(";\n")
}

fn print_bitmask_decl(f: &mut fmt::Formatter<'_>, b: &BitmaskDecl, depth: usize) -> fmt::Result {
    write_block_annotations(f, &b.annotations, depth)?;
    write_indent(f, depth)?;
    write!(f, "bitmask {} {{", b.name.text)?;
    if b.values.is_empty() {
        f.write_str("}")?;
    } else {
        f.write_str("\n")?;
        for (i, v) in b.values.iter().enumerate() {
            write_indent(f, depth + 1)?;
            write_inline_annotations(f, &v.annotations)?;
            f.write_str(&v.name.text)?;
            if i + 1 < b.values.len() {
                f.write_str(",")?;
            }
            f.write_str("\n")?;
        }
        write_indent(f, depth)?;
        f.write_str("}")?;
    }
    Ok(())
}

// ============================================================================
// Declarator
// ============================================================================

fn print_declarator(f: &mut fmt::Formatter<'_>, d: &Declarator) -> fmt::Result {
    match d {
        Declarator::Simple(n) => f.write_str(&n.text),
        Declarator::Array(a) => {
            f.write_str(&a.name.text)?;
            for sz in &a.sizes {
                f.write_str("[")?;
                print_const_expr(f, sz)?;
                f.write_str("]")?;
            }
            Ok(())
        }
    }
}

// ============================================================================
// Const-Decl + ConstExpr
// ============================================================================

fn print_const_decl(f: &mut fmt::Formatter<'_>, c: &ConstDecl) -> fmt::Result {
    f.write_str("const ")?;
    print_const_type(f, &c.type_)?;
    write!(f, " {} = ", c.name.text)?;
    print_const_expr(f, &c.value)
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn print_annotation_dcl(
    f: &mut fmt::Formatter<'_>,
    a: &AnnotationDcl,
    depth: usize,
) -> fmt::Result {
    write_indent(f, depth)?;
    writeln!(f, "@annotation {} {{", a.name.text)?;
    for ec in &a.embedded_consts {
        write_indent(f, depth + 1)?;
        print_const_decl(f, ec)?;
        f.write_str(";\n")?;
    }
    for et in &a.embedded_types {
        print_type_decl(f, et, depth + 1)?;
        f.write_str(";\n")?;
    }
    for m in &a.members {
        write_indent(f, depth + 1)?;
        print_const_type(f, &m.type_spec)?;
        write!(f, " {}", m.name.text)?;
        if let Some(d) = &m.default {
            f.write_str(" default ")?;
            print_const_expr(f, d)?;
        }
        f.write_str(";\n")?;
    }
    write_indent(f, depth)?;
    f.write_str("};\n")
}

fn print_const_type(f: &mut fmt::Formatter<'_>, t: &ConstType) -> fmt::Result {
    match t {
        ConstType::Integer(i) => print_integer_type(f, *i),
        ConstType::Floating(fl) => print_floating_type(f, *fl),
        ConstType::Char => f.write_str("char"),
        ConstType::WideChar => f.write_str("wchar"),
        ConstType::Boolean => f.write_str("boolean"),
        ConstType::Octet => f.write_str("octet"),
        ConstType::String { wide: false } => f.write_str("string"),
        ConstType::String { wide: true } => f.write_str("wstring"),
        ConstType::Fixed => f.write_str("fixed"),
        ConstType::Scoped(s) => write!(f, "{s}"),
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn print_const_expr(f: &mut fmt::Formatter<'_>, e: &ConstExpr) -> fmt::Result {
    match e {
        ConstExpr::Literal(l) => f.write_str(&l.raw),
        ConstExpr::Scoped(s) => write!(f, "{s}"),
        ConstExpr::Unary { op, operand, .. } => {
            f.write_char(unary_op_char(*op))?;
            print_const_expr(f, operand)
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            // Parentheses for an unambiguous roundtrip reading.
            f.write_str("(")?;
            print_const_expr(f, lhs)?;
            write!(f, " {} ", binary_op_str(*op))?;
            print_const_expr(f, rhs)?;
            f.write_str(")")
        }
    }
}

const fn unary_op_char(op: UnaryOp) -> char {
    match op {
        UnaryOp::Plus => '+',
        UnaryOp::Minus => '-',
        UnaryOp::BitNot => '~',
    }
}

const fn binary_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "|",
        BinaryOp::Xor => "^",
        BinaryOp::And => "&",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
    }
}

// ============================================================================
// Exception
// ============================================================================

fn print_except_decl(f: &mut fmt::Formatter<'_>, e: &ExceptDecl, depth: usize) -> fmt::Result {
    write_indent(f, depth)?;
    write!(f, "exception {} {{", e.name.text)?;
    if e.members.is_empty() {
        f.write_str("}")?;
    } else {
        f.write_str("\n")?;
        for m in &e.members {
            print_member(f, m, depth + 1)?;
        }
        write_indent(f, depth)?;
        f.write_str("}")?;
    }
    Ok(())
}

// ============================================================================
// Interface
// ============================================================================

fn print_interface_dcl(f: &mut fmt::Formatter<'_>, i: &InterfaceDcl, depth: usize) -> fmt::Result {
    match i {
        InterfaceDcl::Def(d) => {
            write_block_annotations(f, &d.annotations, depth)?;
            write_indent(f, depth)?;
            print_interface_kind(f, d.kind)?;
            write!(f, "interface {}", d.name.text)?;
            if !d.bases.is_empty() {
                f.write_str(" : ")?;
                for (idx, b) in d.bases.iter().enumerate() {
                    if idx > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{b}")?;
                }
            }
            f.write_str(" {")?;
            if d.exports.is_empty() {
                f.write_str("}")?;
            } else {
                f.write_str("\n")?;
                for e in &d.exports {
                    print_export(f, e, depth + 1)?;
                }
                write_indent(f, depth)?;
                f.write_str("}")?;
            }
            Ok(())
        }
        InterfaceDcl::Forward(d) => {
            write_indent(f, depth)?;
            print_interface_kind(f, d.kind)?;
            write!(f, "interface {}", d.name.text)
        }
    }
}

fn print_interface_kind(f: &mut fmt::Formatter<'_>, kind: InterfaceKind) -> fmt::Result {
    match kind {
        InterfaceKind::Plain => Ok(()),
        InterfaceKind::Abstract => f.write_str("abstract "),
        InterfaceKind::Local => f.write_str("local "),
    }
}

fn print_export(f: &mut fmt::Formatter<'_>, e: &Export, depth: usize) -> fmt::Result {
    match e {
        Export::Op(o) => {
            write_block_annotations(f, &o.annotations, depth)?;
            write_indent(f, depth)?;
            print_op_decl(f, o)?;
            f.write_str(";\n")
        }
        Export::Attr(a) => {
            write_block_annotations(f, &a.annotations, depth)?;
            write_indent(f, depth)?;
            print_attr_decl(f, a)?;
            f.write_str(";\n")
        }
        Export::Type(t) => {
            print_type_decl(f, t, depth)?;
            f.write_str(";\n")
        }
        Export::Const(c) => {
            write_block_annotations(f, &c.annotations, depth)?;
            write_indent(f, depth)?;
            print_const_decl(f, c)?;
            f.write_str(";\n")
        }
        Export::Except(ex) => {
            print_except_decl(f, ex, depth)?;
            f.write_str(";\n")
        }
    }
}

fn print_op_decl(f: &mut fmt::Formatter<'_>, o: &OpDecl) -> fmt::Result {
    if o.oneway {
        f.write_str("oneway ")?;
    }
    match &o.return_type {
        None => f.write_str("void")?,
        Some(t) => print_type_spec(f, t)?,
    }
    write!(f, " {}(", o.name.text)?;
    for (i, p) in o.params.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        print_param_decl(f, p)?;
    }
    f.write_str(")")?;
    if !o.raises.is_empty() {
        f.write_str(" raises (")?;
        for (i, r) in o.raises.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{r}")?;
        }
        f.write_str(")")?;
    }
    // context (...)-Clause (§7.4.6.3 Rule 124).
    if !o.context.is_empty() {
        f.write_str(" context (")?;
        for (i, c) in o.context.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "\"{c}\"")?;
        }
        f.write_str(")")?;
    }
    Ok(())
}

fn print_param_decl(f: &mut fmt::Formatter<'_>, p: &ParamDecl) -> fmt::Result {
    write_inline_annotations(f, &p.annotations)?;
    f.write_str(match p.attribute {
        ParamAttribute::In => "in ",
        ParamAttribute::Out => "out ",
        ParamAttribute::InOut => "inout ",
    })?;
    print_type_spec(f, &p.type_spec)?;
    write!(f, " {}", p.name.text)
}

fn print_attr_decl(f: &mut fmt::Formatter<'_>, a: &AttrDecl) -> fmt::Result {
    if a.readonly {
        f.write_str("readonly ")?;
    }
    f.write_str("attribute ")?;
    print_type_spec(f, &a.type_spec)?;
    write!(f, " {}", a.name.text)
}

// ============================================================================
// Type-Spec
// ============================================================================

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn print_type_spec(f: &mut fmt::Formatter<'_>, t: &TypeSpec) -> fmt::Result {
    match t {
        TypeSpec::Primitive(p) => print_primitive_type(f, *p),
        TypeSpec::Scoped(s) => write!(f, "{s}"),
        TypeSpec::Sequence(s) => {
            f.write_str("sequence<")?;
            print_type_spec(f, &s.elem)?;
            if let Some(b) = &s.bound {
                f.write_str(", ")?;
                print_const_expr(f, b)?;
            }
            f.write_str(">")
        }
        TypeSpec::String(s) => {
            f.write_str(if s.wide { "wstring" } else { "string" })?;
            if let Some(b) = &s.bound {
                f.write_str("<")?;
                print_const_expr(f, b)?;
                f.write_str(">")?;
            }
            Ok(())
        }
        TypeSpec::Fixed(p) => {
            f.write_str("fixed<")?;
            print_const_expr(f, &p.digits)?;
            f.write_str(", ")?;
            print_const_expr(f, &p.scale)?;
            f.write_str(">")
        }
        TypeSpec::Map(m) => {
            f.write_str("map<")?;
            print_type_spec(f, &m.key)?;
            f.write_str(", ")?;
            print_type_spec(f, &m.value)?;
            if let Some(b) = &m.bound {
                f.write_str(", ")?;
                print_const_expr(f, b)?;
            }
            f.write_str(">")
        }
        TypeSpec::Any => f.write_str("any"),
    }
}

fn print_primitive_type(f: &mut fmt::Formatter<'_>, p: PrimitiveType) -> fmt::Result {
    match p {
        PrimitiveType::Integer(i) => print_integer_type(f, i),
        PrimitiveType::Floating(fl) => print_floating_type(f, fl),
        PrimitiveType::Char => f.write_str("char"),
        PrimitiveType::WideChar => f.write_str("wchar"),
        PrimitiveType::Boolean => f.write_str("boolean"),
        PrimitiveType::Octet => f.write_str("octet"),
    }
}

fn print_integer_type(f: &mut fmt::Formatter<'_>, i: IntegerType) -> fmt::Result {
    f.write_str(match i {
        IntegerType::Short => "short",
        IntegerType::Long => "long",
        IntegerType::LongLong => "long long",
        IntegerType::UShort => "unsigned short",
        IntegerType::ULong => "unsigned long",
        IntegerType::ULongLong => "unsigned long long",
        IntegerType::Int8 => "int8",
        IntegerType::Int16 => "int16",
        IntegerType::Int32 => "int32",
        IntegerType::Int64 => "int64",
        IntegerType::UInt8 => "uint8",
        IntegerType::UInt16 => "uint16",
        IntegerType::UInt32 => "uint32",
        IntegerType::UInt64 => "uint64",
    })
}

fn print_floating_type(f: &mut fmt::Formatter<'_>, fl: FloatingType) -> fmt::Result {
    f.write_str(match fl {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "long double",
    })
}

// ============================================================================
// ScopedName + Annotation (Display direkt)
// ============================================================================

impl fmt::Display for ScopedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.absolute {
            f.write_str("::")?;
        }
        for (i, p) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str("::")?;
            }
            f.write_str(&p.text)?;
        }
        Ok(())
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.name)?;
        match &self.params {
            AnnotationParams::None => Ok(()),
            AnnotationParams::Empty => f.write_str("()"),
            AnnotationParams::Single(e) => {
                f.write_str("(")?;
                print_const_expr(f, e)?;
                f.write_str(")")
            }
            AnnotationParams::Named(items) => {
                f.write_str("(")?;
                for (i, p) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}=", p.name.text)?;
                    print_const_expr(f, &p.value)?;
                }
                f.write_str(")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn print(src: &str) -> String {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        format!("{ast}")
    }

    /// Helper for tests that use CORBA constructs (oneway op,
    /// abstract/local interface, etc.) — turns on all features.
    fn print_full(src: &str) -> String {
        let cfg = ParserConfig::full_4_2();
        let ast = parse(src, &cfg).expect("parse");
        format!("{ast}")
    }

    #[test]
    fn prints_empty_module() {
        let out = print("module Empty {};");
        assert!(out.contains("module Empty"), "got: {out}");
        assert!(out.contains("};"));
    }

    #[test]
    fn prints_struct_with_members() {
        let out = print("struct P { long x; long y; };");
        assert!(out.contains("struct P"), "got: {out}");
        assert!(out.contains("long x;"));
        assert!(out.contains("long y;"));
    }

    #[test]
    fn prints_struct_with_inheritance() {
        let out = print("struct D : Base { long x; };");
        assert!(out.contains("struct D : Base"), "got: {out}");
    }

    #[test]
    fn prints_typedef_sequence() {
        let out = print("typedef sequence<long> Seq;");
        assert!(out.contains("typedef sequence<long> Seq"), "got: {out}");
    }

    #[test]
    fn prints_const_with_arithmetic() {
        let out = print("const long V = 1 + 2 * 3;");
        assert!(out.contains("const long V"), "got: {out}");
    }

    #[test]
    fn prints_enum() {
        let out = print("enum Color { RED, GREEN, BLUE };");
        assert!(out.contains("enum Color"), "got: {out}");
        assert!(out.contains("RED"));
        assert!(out.contains("GREEN"));
    }

    #[test]
    fn prints_annotated_struct() {
        let out = print("@topic struct Foo { @key long id; };");
        assert!(out.contains("@topic"), "got: {out}");
        assert!(out.contains("@key long id"));
    }

    #[test]
    fn prints_interface_with_op() {
        let out = print("interface S { long add(in long a, in long b); };");
        assert!(out.contains("interface S"), "got: {out}");
        assert!(out.contains("long add"));
        assert!(out.contains("in long a"));
    }

    #[test]
    fn prints_oneway_op_with_raises() {
        // The `oneway` op is a CORBA construct, gated via corba_oneway_op.
        let out = print_full("interface S { oneway void log(in string m) raises (E); };");
        assert!(out.contains("oneway void log"), "got: {out}");
        assert!(out.contains("raises (E)"));
    }

    #[test]
    fn prints_readonly_attribute() {
        let out = print("interface C { readonly attribute long value; };");
        assert!(out.contains("readonly attribute long value"), "got: {out}");
    }

    #[test]
    fn prints_union() {
        let out = print("union V switch (long) { case 1: long a; default: string b; };");
        assert!(out.contains("union V switch (long)"), "got: {out}");
        assert!(out.contains("case 1: long a"));
        assert!(out.contains("default: string b"));
    }

    #[test]
    fn prints_map() {
        let out = print("typedef map<string, long> Idx;");
        assert!(out.contains("map<string, long>"), "got: {out}");
    }

    #[test]
    fn prints_bitset() {
        let out = print("bitset F { bitfield<3> level; bitfield<1> on; };");
        assert!(out.contains("bitset F"), "got: {out}");
        assert!(out.contains("bitfield<3> level"));
    }

    #[test]
    fn prints_bitmask() {
        let out = print("bitmask P { READ, WRITE };");
        assert!(out.contains("bitmask P"), "got: {out}");
        assert!(out.contains("READ"));
        assert!(out.contains("WRITE"));
    }

    #[test]
    fn prints_value_box() {
        let out = print("valuetype Name string;");
        assert!(out.contains("valuetype Name string"), "got: {out}");
    }

    #[test]
    fn prints_module_with_nested_struct() {
        let out = print("module svc { struct Foo { long x; }; };");
        assert!(out.contains("module svc"), "got: {out}");
        assert!(out.contains("struct Foo"));
    }

    #[test]
    fn prints_scoped_name() {
        let out = print("typedef ::ns::T Alias;");
        assert!(out.contains("::ns::T"), "got: {out}");
    }

    #[test]
    fn prints_annotation_with_named_params() {
        let out = print("struct S { @range(min=0, max=10) long x; };");
        assert!(out.contains("@range(min=0, max=10)"), "got: {out}");
    }
}
