// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §11.4 (C++ classic, IDL2C++ 1.3) and §11.6 (C++11,
//! IDL2C++11, aligned with the DDS-STDC++ PSM with five exceptions).

use super::model::{Decl, Dir, Flat, Op, Ty};
use super::resolve::{Kind, Table};
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq)]
pub enum Dialect {
    /// Without the C++ standard library; module DDS is namespace `DDS`.
    Classic,
    /// §11.6: module DDS is namespace `dds`, arrays are `dds::core::array`,
    /// enumerations are `dds::core::safe_enum`, DynamicData is
    /// `dds::core::xtypes::DynamicData`, and no C++11-only feature is used.
    Cxx11,
}

struct Ctx<'a> {
    t: &'a Table<'a>,
    d: Dialect,
}

impl<'a> Ctx<'a> {
    fn ns(&self, segment: &str) -> String {
        if self.d == Dialect::Cxx11 && segment == "DDS" {
            "dds".into()
        } else {
            segment.into()
        }
    }

    fn qualified(&self, full: &str) -> String {
        let parts: Vec<String> = full.split("::").map(|s| self.ns(s)).collect();
        format!("::{}", parts.join("::"))
    }

    fn basic(&self, b: &str) -> &'static str {
        match (self.d, b) {
            (Dialect::Classic, "long" | "int32") => "::CORBA::Long",
            (Dialect::Classic, "unsigned long" | "uint32") => "::CORBA::ULong",
            (Dialect::Classic, "long long") => "::CORBA::LongLong",
            (Dialect::Classic, "unsigned long long") => "::CORBA::ULongLong",
            (Dialect::Classic, "short") => "::CORBA::Short",
            (Dialect::Classic, "unsigned short") => "::CORBA::UShort",
            (Dialect::Classic, "octet") => "::CORBA::Octet",
            (Dialect::Classic, "boolean") => "::CORBA::Boolean",
            (Dialect::Classic, "char") => "::CORBA::Char",
            (Dialect::Classic, "float") => "::CORBA::Float",
            (Dialect::Classic, "double") => "::CORBA::Double",
            (Dialect::Cxx11, "long" | "int32") => "int32_t",
            (Dialect::Cxx11, "unsigned long" | "uint32") => "uint32_t",
            (Dialect::Cxx11, "long long") => "int64_t",
            (Dialect::Cxx11, "unsigned long long") => "uint64_t",
            (Dialect::Cxx11, "short") => "int16_t",
            (Dialect::Cxx11, "unsigned short") => "uint16_t",
            (Dialect::Cxx11, "octet") => "uint8_t",
            (Dialect::Cxx11, "boolean") => "bool",
            (Dialect::Cxx11, "char") => "char",
            (Dialect::Cxx11, "float") => "float",
            (Dialect::Cxx11, "double") => "double",
            (_, other) => panic!("basic type {other}"),
        }
    }

    /// Spelling of a type as a member or typedef target.
    fn ty(&self, path: &[String], ty: &Ty) -> String {
        match ty {
            Ty::Basic(b) => self.basic(b).into(),
            Ty::Str => match self.d {
                Dialect::Classic => "::CORBA::String_var".into(),
                Dialect::Cxx11 => "std::string".into(),
            },
            Ty::Sequence(inner) => {
                let e = self.ty(path, inner);
                match self.d {
                    Dialect::Classic => format!("::CORBA::Sequence< {e} >"),
                    Dialect::Cxx11 => format!("std::vector< {e} >"),
                }
            }
            Ty::Named(n) => {
                let full = self.t.resolve(path, n);
                match self.t.decl(&full) {
                    Decl::Native { .. } => match self.d {
                        Dialect::Classic => "void *".into(),
                        Dialect::Cxx11 => "::dds::core::xtypes::DynamicData".into(),
                    },
                    Decl::Interface { .. } => match self.d {
                        Dialect::Classic => format!("{} *", self.qualified(&full)),
                        Dialect::Cxx11 => format!("{} *", self.qualified(&full)),
                    },
                    Decl::Enum { .. } if self.d == Dialect::Cxx11 => self.qualified(&full),
                    _ => self.qualified(&full),
                }
            }
        }
    }

    fn param(&self, path: &[String], dir: Dir, ty: &Ty, name: &str) -> String {
        let kind = self.t.kind(path, ty);
        let c = self.ty(path, ty);
        match dir {
            Dir::In => match kind {
                Kind::Basic(_) | Kind::Enum | Kind::Interface => format!("{c} {name}"),
                Kind::Native if self.d == Dialect::Classic => format!("{c} {name}"),
                Kind::Str if self.d == Dialect::Classic => format!("const char *{name}"),
                _ => format!("const {c} &{name}"),
            },
            Dir::InOut => match kind {
                Kind::Str if self.d == Dialect::Classic => format!("char *&{name}"),
                _ => format!("{c} &{name}"),
            },
        }
    }

    fn struct_def(&self, out: &mut String, full: &str, indent: &str) {
        let Decl::Struct {
            name,
            base,
            members,
            ..
        } = self.t.decl(full)
        else {
            panic!("struct")
        };
        let path = self.t.path_of(full);
        let base_clause = match base {
            Some(b) => {
                let bf = self.t.resolve(path, b);
                format!(" : public {}", self.qualified(&bf))
            }
            None => String::new(),
        };
        writeln!(out, "{indent}struct {name}{base_clause} {{").unwrap();
        for m in members {
            let c = self.ty(path, &m.ty);
            if m.dims.is_empty() {
                writeln!(out, "{indent}    {c} {};", m.name).unwrap();
            } else if self.d == Dialect::Cxx11 {
                let mut t = c;
                for d in m.dims.iter().rev() {
                    t = format!("::dds::core::array< {t}, {d} >");
                }
                writeln!(out, "{indent}    {t} {};", m.name).unwrap();
            } else {
                let dims: String = m.dims.iter().map(|d| format!("[{d}]")).collect();
                writeln!(out, "{indent}    {c} {}{dims};", m.name).unwrap();
            }
        }
        writeln!(out, "{indent}}};").unwrap();
    }
}

pub fn render(flat: &[Flat<'_>], table: &Table<'_>, d: Dialect) -> String {
    let cx = Ctx { t: table, d };
    let mut out = String::new();
    out.push_str(match d {
        Dialect::Classic => CLASSIC_PREAMBLE,
        Dialect::Cxx11 => CXX11_PREAMBLE,
    });
    let mut open: Vec<String> = Vec::new();
    for f in flat {
        // Open and close namespaces to reach f.path.
        let target: Vec<String> = f.path.iter().map(|s| cx.ns(s)).collect();
        let common = open.iter().zip(&target).take_while(|(a, b)| a == b).count();
        while open.len() > common {
            open.pop();
            writeln!(out, "{}}}", "    ".repeat(open.len())).unwrap();
        }
        for seg in &target[common..] {
            writeln!(out, "{}namespace {seg} {{", "    ".repeat(open.len())).unwrap();
            open.push(seg.clone());
        }
        let indent = "    ".repeat(open.len());
        let path = &f.path;
        match f.decl {
            Decl::Typedef { name, ty, dims } => {
                let c = cx.ty(path, ty);
                if dims.is_empty() {
                    writeln!(out, "{indent}typedef {c} {name};").unwrap();
                } else if d == Dialect::Cxx11 {
                    let mut t = c;
                    for n in dims.iter().rev() {
                        t = format!("::dds::core::array< {t}, {n} >");
                    }
                    writeln!(out, "{indent}typedef {t} {name};").unwrap();
                } else {
                    let ds: String = dims.iter().map(|n| format!("[{n}]")).collect();
                    writeln!(out, "{indent}typedef {c} {name}{ds};").unwrap();
                }
            }
            Decl::Const { name, ty, value } => {
                let c = cx.ty(path, ty);
                writeln!(out, "{indent}const {c} {name} = {value};").unwrap();
            }
            Decl::Struct { name, .. } => {
                let full = super::resolve::full_name(path, name);
                cx.struct_def(&mut out, &full, &indent);
            }
            Decl::Enum { name, variants } => {
                let list = variants.join(", ");
                match d {
                    Dialect::Classic => {
                        writeln!(out, "{indent}enum {name} {{ {list} }};").unwrap();
                    }
                    Dialect::Cxx11 => {
                        writeln!(
                            out,
                            "{indent}struct {name}_def {{ enum type {{ {list} }}; }};\n{indent}typedef ::dds::core::safe_enum< {name}_def > {name};"
                        )
                        .unwrap();
                    }
                }
            }
            Decl::Native { name } => {
                let c = cx.ty(path, &Ty::Named(name.clone()));
                writeln!(out, "{indent}typedef {c} {name};").unwrap();
            }
            Decl::Forward { name } => {
                writeln!(out, "{indent}class {name};").unwrap();
            }
            Decl::Interface { name, ops, attrs } => {
                writeln!(out, "{indent}class {name} {{").unwrap();
                writeln!(out, "{indent}public:").unwrap();
                writeln!(out, "{indent}    virtual ~{name}() {{}}").unwrap();
                for op in ops {
                    writeln!(out, "{indent}    {}", method(&cx, path, op)).unwrap();
                }
                for a in attrs {
                    let c = cx.ty(path, &a.ty);
                    writeln!(out, "{indent}    virtual {c} {}() = 0;", a.name).unwrap();
                }
                writeln!(out, "{indent}}};").unwrap();
            }
            Decl::Module { .. } => {}
        }
    }
    while open.pop().is_some() {
        writeln!(out, "{}}}", "    ".repeat(open.len())).unwrap();
    }
    out.push_str("\n#endif\n");
    out
}

fn method(cx: &Ctx<'_>, path: &[String], op: &Op) -> String {
    let ret = match &op.ret {
        None => "void".to_string(),
        Some(t) => cx.ty(path, t),
    };
    let params: Vec<String> = op
        .params
        .iter()
        .map(|p| cx.param(path, p.dir, &p.ty, &p.name))
        .collect();
    format!("virtual {ret} {}({}) = 0;", op.name, params.join(", "))
}

const CLASSIC_PREAMBLE: &str = "// Generated from dds_security.idl by the IDL to C++ mapping rules (DDS-Security 1.2 section 11.4).\n\
// No part of the C++ standard library is used.\n\
#ifndef DDS_SECURITY_CLASSIC_HPP\n#define DDS_SECURITY_CLASSIC_HPP\n\n\
#include <stddef.h>\n#include <stdint.h>\n#include <string.h>\n\n\
// The base types of the IDL to C++ mapping. An ORB supplies them in its own\n// header; they are defined here so that this header stands by itself.\n\
#ifndef CORBA_CPP_BASE_TYPES\n#define CORBA_CPP_BASE_TYPES\n\
namespace CORBA {\n\
typedef int16_t Short;\ntypedef uint16_t UShort;\ntypedef int32_t Long;\ntypedef uint32_t ULong;\n\
typedef int64_t LongLong;\ntypedef uint64_t ULongLong;\ntypedef float Float;\ntypedef double Double;\n\
typedef unsigned char Octet;\ntypedef unsigned char Boolean;\ntypedef char Char;\n\
class String_var {\npublic:\n    String_var() : p_(0) {}\n    String_var(const char *s) : p_(copy(s)) {}\n    String_var(const String_var &o) : p_(copy(o.p_)) {}\n    ~String_var() { delete[] p_; }\n    String_var &operator=(const char *s) { char *n = copy(s); delete[] p_; p_ = n; return *this; }\n    String_var &operator=(const String_var &o) { return *this = o.p_; }\n    const char *in() const { return p_ ? p_ : \"\"; }\nprivate:\n    static char *copy(const char *s) { if (!s) return 0; char *n = new char[strlen(s) + 1]; strcpy(n, s); return n; }\n    char *p_;\n};\n\
template <class T> class Sequence {\npublic:\n    Sequence() : len_(0), buf_(0) {}\n    Sequence(const Sequence &o) : len_(0), buf_(0) { *this = o; }\n    ~Sequence() { delete[] buf_; }\n    Sequence &operator=(const Sequence &o) { if (this != &o) { T *n = o.len_ ? new T[o.len_] : 0; for (ULong i = 0; i < o.len_; ++i) n[i] = o.buf_[i]; delete[] buf_; buf_ = n; len_ = o.len_; } return *this; }\n    ULong length() const { return len_; }\n    void length(ULong n) { T *b = n ? new T[n] : 0; for (ULong i = 0; i < n && i < len_; ++i) b[i] = buf_[i]; delete[] buf_; buf_ = b; len_ = n; }\n    T &operator[](ULong i) { return buf_[i]; }\n    const T &operator[](ULong i) const { return buf_[i]; }\nprivate:\n    ULong len_;\n    T *buf_;\n};\n\
}\n#endif\n\n";

const CXX11_PREAMBLE: &str = "// Generated from dds_security.idl by the IDL to C++11 mapping rules with the five exceptions\n// of DDS-Security 1.2 section 11.6 (module DDS is namespace dds; no C++11-only feature;\n// arrays are dds::core::array; enumerations are dds::core::safe_enum; DynamicData is\n// dds::core::xtypes::DynamicData). The templates and DynamicData come from the DDS-STD-C++ PSM.\n\
#ifndef DDS_SECURITY_CXX11_HPP\n#define DDS_SECURITY_CXX11_HPP\n\n\
#include <stdint.h>\n#include <string>\n#include <vector>\n#include <dds/core/Array.hpp>\n#include <dds/core/SafeEnumeration.hpp>\n#include <dds/core/xtypes/DynamicData.hpp>\n\n";
