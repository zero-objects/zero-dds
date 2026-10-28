// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §11.5 (Java classic, IDL2Java 1.3) and §11.7 (Java modern,
//! aligned with the DDS-JAVA5+ PSM): module `DDS` is the package
//! `org.omg.dds` and the native type DynamicData is
//! `org.omg.dds.type.dynamic.DynamicData`.

use super::model::{Decl, Dir, Flat, Op, Ty};
use super::resolve::Table;
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq)]
pub enum Dialect {
    Classic,
    Modern,
}

struct Ctx<'a> {
    t: &'a Table<'a>,
    d: Dialect,
}

pub struct File {
    pub path: String,
    pub text: String,
}

const CORBA: &str = "org.omg.CORBA";

impl<'a> Ctx<'a> {
    fn package_of(&self, path: &[String]) -> String {
        let parts: Vec<String> = path
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if self.d == Dialect::Modern && i == 0 && s == "DDS" {
                    "org.omg.dds".to_string()
                } else {
                    s.clone()
                }
            })
            .collect();
        parts.join(".")
    }

    fn class_name(&self, full: &str) -> String {
        let path = self.t.path_of(full);
        let name = full.rsplit("::").next().unwrap();
        format!("{}.{}", self.package_of(path), name)
    }

    /// Follows typedefs that name a plain (non-array, non-sequence) type, so
    /// that `IdentityToken` is spelled `DataHolder`, as IDL2Java does.
    fn deref(&self, path: &[String], ty: &Ty) -> (Vec<String>, Ty) {
        let mut path = path.to_vec();
        let mut ty = ty.clone();
        loop {
            let Ty::Named(n) = &ty else { break };
            let full = self.t.resolve(&path, n);
            match self.t.decl(&full) {
                Decl::Typedef {
                    ty: inner, dims, ..
                } if dims.is_empty() && !matches!(inner, Ty::Sequence(_)) => {
                    path = self.t.path_of(&full).to_vec();
                    ty = inner.clone();
                }
                _ => break,
            }
        }
        (path, ty)
    }

    /// The Java type of `ty`.
    fn ty(&self, path: &[String], ty: &Ty) -> String {
        let (path, ty) = self.deref(path, ty);
        match &ty {
            Ty::Basic(b) => match b.as_str() {
                "long" | "unsigned long" | "int32" | "uint32" => "int",
                "long long" | "unsigned long long" => "long",
                "short" | "unsigned short" => "short",
                "octet" => "byte",
                "boolean" => "boolean",
                "char" => "char",
                "float" => "float",
                "double" => "double",
                other => panic!("basic {other}"),
            }
            .into(),
            Ty::Str => "String".into(),
            Ty::Sequence(inner) => format!("{}[]", self.ty(&path, inner)),
            Ty::Named(n) => {
                let full = self.t.resolve(&path, n);
                match self.t.decl(&full) {
                    Decl::Native { .. } => match self.d {
                        Dialect::Classic => "java.lang.Object".into(),
                        Dialect::Modern => "org.omg.dds.type.dynamic.DynamicData".into(),
                    },
                    Decl::Typedef {
                        dims, ty: inner, ..
                    } => {
                        // An array or sequence typedef is its array type.
                        let base = self.ty(self.t.path_of(&full), inner);
                        format!("{base}{}", "[]".repeat(dims.len().max(0)))
                    }
                    _ => self.class_name(&full),
                }
            }
        }
    }

    /// The Holder class of `ty` for an inout parameter.
    fn holder(&self, path: &[String], ty: &Ty) -> String {
        let (p, ty) = self.deref(path, ty);
        match &ty {
            Ty::Basic(b) => format!(
                "{CORBA}.{}",
                match b.as_str() {
                    "long" | "unsigned long" | "int32" | "uint32" => "IntHolder",
                    "long long" | "unsigned long long" => "LongHolder",
                    "short" | "unsigned short" => "ShortHolder",
                    "octet" => "ByteHolder",
                    "boolean" => "BooleanHolder",
                    "char" => "CharHolder",
                    "float" => "FloatHolder",
                    "double" => "DoubleHolder",
                    other => panic!("basic {other}"),
                }
            ),
            Ty::Str => format!("{CORBA}.StringHolder"),
            Ty::Sequence(_) => panic!("inline sequence parameter"),
            Ty::Named(n) => {
                let full = self.t.resolve(&p, n);
                format!("{}Holder", self.class_name(&full))
            }
        }
    }

    fn param(&self, path: &[String], dir: Dir, ty: &Ty, name: &str) -> String {
        match dir {
            Dir::In => format!("{} {name}", self.ty(path, ty)),
            Dir::InOut => format!("{} {name}", self.holder(path, ty)),
        }
    }

    fn method(&self, path: &[String], op: &Op) -> String {
        let ret = op
            .ret
            .as_ref()
            .map_or("void".to_string(), |t| self.ty(path, t));
        let params: Vec<String> = op
            .params
            .iter()
            .map(|p| self.param(path, p.dir, &p.ty, &p.name))
            .collect();
        format!("    {ret} {}({});", op.name, params.join(", "))
    }

    fn header(&self, package: &str) -> String {
        format!(
            "// Generated from dds_security.idl by the IDL to Java mapping rules (DDS-Security 1.2 section {}).\n\
package {package};\n\n",
            if self.d == Dialect::Classic {
                "11.5"
            } else {
                "11.7"
            }
        )
    }

    fn write_expr(&self, path: &[String], ty: &Ty, value: &str, out: &str) -> String {
        let (p, ty) = self.deref(path, ty);
        match &ty {
            Ty::Basic(b) => {
                let m = match b.as_str() {
                    "long" | "int32" => "write_long",
                    "unsigned long" | "uint32" => "write_ulong",
                    "long long" => "write_longlong",
                    "unsigned long long" => "write_ulonglong",
                    "short" => "write_short",
                    "unsigned short" => "write_ushort",
                    "octet" => "write_octet",
                    "boolean" => "write_boolean",
                    "char" => "write_char",
                    "float" => "write_float",
                    "double" => "write_double",
                    other => panic!("basic {other}"),
                };
                format!("{out}.{m}({value});")
            }
            Ty::Str => format!("{out}.write_string({value});"),
            Ty::Sequence(inner) => {
                let e = self.write_expr(&p, inner, &format!("{value}[i]"), out);
                format!(
                    "{out}.write_ulong({value}.length);\n        for (int i = 0; i < {value}.length; ++i) {{ {e} }}"
                )
            }
            Ty::Named(n) => {
                let full = self.t.resolve(&p, n);
                match self.t.decl(&full) {
                    Decl::Native { .. } => String::new(),
                    Decl::Typedef { .. } => {
                        format!("{}Helper.write({out}, {value});", self.class_name(&full))
                    }
                    _ => format!("{}Helper.write({out}, {value});", self.class_name(&full)),
                }
            }
        }
    }

    fn read_expr(&self, path: &[String], ty: &Ty, target: &str, inp: &str) -> String {
        let (p, ty) = self.deref(path, ty);
        match &ty {
            Ty::Basic(b) => {
                let m = match b.as_str() {
                    "long" | "int32" => "read_long",
                    "unsigned long" | "uint32" => "read_ulong",
                    "long long" => "read_longlong",
                    "unsigned long long" => "read_ulonglong",
                    "short" => "read_short",
                    "unsigned short" => "read_ushort",
                    "octet" => "read_octet",
                    "boolean" => "read_boolean",
                    "char" => "read_char",
                    "float" => "read_float",
                    "double" => "read_double",
                    other => panic!("basic {other}"),
                };
                format!("{target} = {inp}.{m}();")
            }
            Ty::Str => format!("{target} = {inp}.read_string();"),
            Ty::Sequence(inner) => {
                let elem = self.ty(&p, inner);
                let r = self.read_expr(&p, inner, &format!("{target}[i]"), inp);
                format!(
                    "{target} = new {elem}[{inp}.read_ulong()];\n        for (int i = 0; i < {target}.length; ++i) {{ {r} }}"
                )
            }
            Ty::Named(n) => {
                let full = self.t.resolve(&p, n);
                match self.t.decl(&full) {
                    Decl::Native { .. } => String::new(),
                    _ => format!("{target} = {}Helper.read({inp});", self.class_name(&full)),
                }
            }
        }
    }
}

pub fn render(flat: &[Flat<'_>], table: &Table<'_>, d: Dialect) -> Vec<File> {
    let cx = Ctx { t: table, d };
    let mut files = Vec::new();
    let mut push = |package: &str, name: &str, body: String| {
        files.push(File {
            path: format!("{}/{name}.java", package.replace('.', "/")),
            text: body,
        });
    };
    for f in flat {
        let package = cx.package_of(&f.path);
        let path = &f.path;
        let header = cx.header(&package);
        match f.decl {
            Decl::Forward { .. } | Decl::Module { .. } | Decl::Native { .. } => {}
            Decl::Const { name, ty, value } => {
                // IDL2Java: a constant of a module is the field `value` of an
                // interface named after the constant.
                let jt = cx.ty(path, ty);
                let (_, base) = cx.deref(path, ty);
                let literal = match &base {
                    Ty::Basic(b) => match b.as_str() {
                        "unsigned long" | "uint32" => format!("(int) ({value}L)"),
                        "long long" | "unsigned long long" => format!("({value}L)"),
                        "unsigned short" | "short" => format!("(short) ({value})"),
                        _ => value.clone(),
                    },
                    _ => value.clone(),
                };
                let body = format!(
                    "{header}public interface {name} {{\n    public static final {jt} value = {literal};\n}}\n"
                );
                push(&package, name, body);
            }
            Decl::Typedef { name, ty, dims } => {
                // Only an array or a sequence is a type of its own.
                if dims.is_empty() && !matches!(ty, Ty::Sequence(_)) {
                    continue;
                }
                let full = super::resolve::full_name(path, name);
                let jt = cx.ty(path, &Ty::Named(name.clone()));
                let mut body = String::new();
                writeln!(
                    body,
                    "{header}public final class {name}Holder implements {CORBA}.portable.Streamable {{\n    public {jt} value;\n    public {name}Holder() {{}}\n    public {name}Holder({jt} initial) {{ value = initial; }}\n    public void _read({CORBA}.portable.InputStream in) {{ value = {name}Helper.read(in); }}\n    public void _write({CORBA}.portable.OutputStream out) {{ {name}Helper.write(out, value); }}\n}}"
                )
                .unwrap();
                push(&package, &format!("{name}Holder"), body);
                let mut h = String::new();
                let (write, read) = if dims.is_empty() {
                    let Ty::Sequence(inner) = ty else {
                        unreachable!()
                    };
                    (
                        cx.write_expr(path, &Ty::Sequence(inner.clone()), "value", "out"),
                        cx.read_expr(path, &Ty::Sequence(inner.clone()), "value", "in"),
                    )
                } else {
                    let n = dims[0];
                    let e = cx.write_expr(path, ty, "value[i]", "out");
                    let r = cx.read_expr(path, ty, "value[i]", "in");
                    (
                        format!("for (int i = 0; i < {n}; ++i) {{ {e} }}"),
                        format!(
                            "value = new {}[{n}];\n        for (int i = 0; i < {n}; ++i) {{ {r} }}",
                            cx.ty(path, ty)
                        ),
                    )
                };
                writeln!(
                    h,
                    "{header}public abstract class {name}Helper {{\n    public static String id() {{ return \"IDL:{}:1.0\"; }}\n    public static {jt} read({CORBA}.portable.InputStream in) {{\n        {jt} value;\n        {read}\n        return value;\n    }}\n    public static void write({CORBA}.portable.OutputStream out, {jt} value) {{\n        {write}\n    }}\n}}",
                    full.replace("::", "/")
                )
                .unwrap();
                push(&package, &format!("{name}Helper"), h);
            }
            Decl::Struct { name, .. } => {
                let full = super::resolve::full_name(path, name);
                let mut fields = Vec::new();
                collect_fields(&cx, &full, &mut fields);
                let mut body = format!(
                    "{header}public final class {name}{} implements {CORBA}.portable.IDLEntity {{\n",
                    ""
                );
                for fld in &fields {
                    writeln!(body, "    public {} {};", fld.java, fld.name).unwrap();
                }
                writeln!(body, "    public {name}() {{}}").unwrap();
                if !fields.is_empty() {
                    let params: Vec<String> = fields
                        .iter()
                        .map(|fld| format!("{} _{}", fld.java, fld.name))
                        .collect();
                    writeln!(body, "    public {name}({}) {{", params.join(", ")).unwrap();
                    for fld in &fields {
                        writeln!(body, "        this.{0} = _{0};", fld.name).unwrap();
                    }
                    writeln!(body, "    }}").unwrap();
                }
                writeln!(body, "}}").unwrap();
                push(&package, name, body);
                let holder = format!(
                    "{header}public final class {name}Holder implements {CORBA}.portable.Streamable {{\n    public {package}.{name} value;\n    public {name}Holder() {{}}\n    public {name}Holder({package}.{name} initial) {{ value = initial; }}\n    public void _read({CORBA}.portable.InputStream in) {{ value = {name}Helper.read(in); }}\n    public void _write({CORBA}.portable.OutputStream out) {{ {name}Helper.write(out, value); }}\n}}\n"
                );
                push(&package, &format!("{name}Holder"), holder);
                let mut helper = format!(
                    "{header}public abstract class {name}Helper {{\n    public static String id() {{ return \"IDL:{}:1.0\"; }}\n    public static {package}.{name} read({CORBA}.portable.InputStream in) {{\n        {package}.{name} value = new {package}.{name}();\n",
                    full.replace("::", "/")
                );
                for fld in &fields {
                    let n = &fld.name;
                    if let Some(len) = fld.dims.first() {
                        let elem = cx.ty(&fld.path, &fld.ty);
                        let r = cx.read_expr(&fld.path, &fld.ty, &format!("value.{n}[i]"), "in");
                        writeln!(
                            helper,
                            "        value.{n} = new {elem}[{len}];\n        for (int i = 0; i < {len}; ++i) {{ {r} }}"
                        )
                        .unwrap();
                        continue;
                    }
                    let r = cx.read_expr(&fld.path, &fld.ty, &format!("value.{n}"), "in");
                    if !r.is_empty() {
                        writeln!(helper, "        {r}").unwrap();
                    }
                }
                writeln!(helper, "        return value;\n    }}").unwrap();
                writeln!(
                    helper,
                    "    public static void write({CORBA}.portable.OutputStream out, {package}.{name} value) {{"
                )
                .unwrap();
                for fld in &fields {
                    let n = &fld.name;
                    if let Some(len) = fld.dims.first() {
                        let w = cx.write_expr(&fld.path, &fld.ty, &format!("value.{n}[i]"), "out");
                        writeln!(helper, "        for (int i = 0; i < {len}; ++i) {{ {w} }}")
                            .unwrap();
                        continue;
                    }
                    let w = cx.write_expr(&fld.path, &fld.ty, &format!("value.{n}"), "out");
                    if !w.is_empty() {
                        writeln!(helper, "        {w}").unwrap();
                    }
                }
                writeln!(helper, "    }}\n}}").unwrap();
                push(&package, &format!("{name}Helper"), helper);
            }
            Decl::Enum { name, variants } => {
                let full = super::resolve::full_name(path, name);
                let mut body = format!(
                    "{header}public final class {name} implements {CORBA}.portable.IDLEntity {{\n    private final int value_;\n    private static final {name}[] values_ = new {name}[{}];\n",
                    variants.len()
                );
                for (i, v) in variants.iter().enumerate() {
                    writeln!(body, "    public static final int _{v} = {i};").unwrap();
                    writeln!(
                        body,
                        "    public static final {name} {v} = new {name}(_{v});"
                    )
                    .unwrap();
                }
                writeln!(
                    body,
                    "    public int value() {{ return value_; }}\n    public static {name} from_int(int value) {{\n        if (value < 0 || value >= values_.length) throw new {CORBA}.BAD_PARAM();\n        return values_[value];\n    }}\n    protected {name}(int value) {{ value_ = value; values_[value] = this; }}\n}}"
                )
                .unwrap();
                push(&package, name, body);
                let holder = format!(
                    "{header}public final class {name}Holder implements {CORBA}.portable.Streamable {{\n    public {package}.{name} value;\n    public {name}Holder() {{}}\n    public {name}Holder({package}.{name} initial) {{ value = initial; }}\n    public void _read({CORBA}.portable.InputStream in) {{ value = {name}Helper.read(in); }}\n    public void _write({CORBA}.portable.OutputStream out) {{ {name}Helper.write(out, value); }}\n}}\n"
                );
                push(&package, &format!("{name}Holder"), holder);
                let helper = format!(
                    "{header}public abstract class {name}Helper {{\n    public static String id() {{ return \"IDL:{}:1.0\"; }}\n    public static {package}.{name} read({CORBA}.portable.InputStream in) {{ return {package}.{name}.from_int(in.read_long()); }}\n    public static void write({CORBA}.portable.OutputStream out, {package}.{name} value) {{ out.write_long(value.value()); }}\n}}\n",
                    full.replace("::", "/")
                );
                push(&package, &format!("{name}Helper"), helper);
            }
            Decl::Interface { name, ops, attrs } => {
                // A local interface: the Operations interface, the interface
                // and the Holder; IDL2Java makes no stub or skeleton for it.
                let mut ops_body = format!("{header}public interface {name}Operations {{\n");
                for op in ops {
                    writeln!(ops_body, "{}", cx.method(path, op)).unwrap();
                }
                for a in attrs {
                    writeln!(ops_body, "    {} {}();", cx.ty(path, &a.ty), a.name).unwrap();
                }
                writeln!(ops_body, "}}").unwrap();
                push(&package, &format!("{name}Operations"), ops_body);
                let iface = format!(
                    "{header}public interface {name} extends {name}Operations, {CORBA}.Object, {CORBA}.portable.IDLEntity {{\n}}\n"
                );
                push(&package, name, iface);
                let holder = format!(
                    "{header}public final class {name}Holder implements {CORBA}.portable.Streamable {{\n    public {package}.{name} value;\n    public {name}Holder() {{}}\n    public {name}Holder({package}.{name} initial) {{ value = initial; }}\n    public void _read({CORBA}.portable.InputStream in) {{ throw new {CORBA}.MARSHAL(); }}\n    public void _write({CORBA}.portable.OutputStream out) {{ throw new {CORBA}.MARSHAL(); }}\n}}\n"
                );
                push(&package, &format!("{name}Holder"), holder);
            }
        }
    }
    files
}

/// (java type, name, path of the declaration, IDL type) of a struct's fields,
/// the base's first.
fn collect_fields(cx: &Ctx<'_>, full: &str, out: &mut Vec<Field>) {
    let Decl::Struct { base, members, .. } = cx.t.decl(full) else {
        panic!("struct")
    };
    let path = cx.t.path_of(full).to_vec();
    if let Some(base) = base {
        let bf = cx.t.resolve(&path, base);
        collect_fields(cx, &bf, out);
    }
    for m in members {
        let mut jt = cx.ty(&path, &m.ty);
        for _ in &m.dims {
            jt.push_str("[]");
        }
        out.push(Field {
            java: jt,
            name: m.name.clone(),
            path: path.clone(),
            ty: m.ty.clone(),
            dims: m.dims.clone(),
        });
    }
}

/// A field of a structure, base fields first.
struct Field {
    java: String,
    name: String,
    path: Vec<String>,
    ty: Ty,
    dims: Vec<usize>,
}
