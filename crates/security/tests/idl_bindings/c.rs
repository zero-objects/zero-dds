// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §11.3: the C language representation, obtained by
//! applying the IDL to C mapping (OMG IDL2C 1.0) to `dds_security.idl`.

use super::model::{Decl, Dir, Flat, Op, Ty};
use super::resolve::{Kind, Table};
use std::collections::BTreeSet;
use std::fmt::Write as _;

fn basic(b: &str) -> &'static str {
    match b {
        "long" | "int32" => "CORBA_long",
        "unsigned long" | "uint32" => "CORBA_unsigned_long",
        "long long" => "CORBA_long_long",
        "unsigned long long" => "CORBA_unsigned_long_long",
        "short" => "CORBA_short",
        "unsigned short" => "CORBA_unsigned_short",
        "octet" => "CORBA_octet",
        "boolean" => "CORBA_boolean",
        "char" => "CORBA_char",
        "float" => "CORBA_float",
        "double" => "CORBA_double",
        other => panic!("basic type {other}"),
    }
}

fn mangle(full: &str) -> String {
    full.replace("::", "_")
}

struct Ctx<'a> {
    t: &'a Table<'a>,
    inline_sequences: BTreeSet<String>,
}

impl<'a> Ctx<'a> {
    /// The C spelling of `ty` written in `path`.
    fn ty(&mut self, path: &[String], ty: &Ty) -> String {
        match ty {
            Ty::Basic(b) => basic(b).into(),
            Ty::Str => "CORBA_char *".into(),
            Ty::Sequence(inner) => {
                let elem = self.ty(path, inner);
                let name = format!(
                    "CORBA_sequence_{}",
                    elem.replace(' ', "")
                        .replace('*', "_ptr")
                        .replace("CORBA_", "")
                );
                self.inline_sequences.insert(format!("{name}|{elem}"));
                name
            }
            Ty::Named(n) => {
                let full = self.t.resolve(path, n);
                match self.t.decl(&full) {
                    Decl::Native { .. } => "void *".into(),
                    _ => mangle(&full),
                }
            }
        }
    }

    fn passed_by_value(&self, path: &[String], ty: &Ty) -> bool {
        matches!(
            self.t.kind(path, ty),
            Kind::Basic(_) | Kind::Enum | Kind::Interface | Kind::Native
        )
    }

    fn param(&mut self, path: &[String], dir: Dir, ty: &Ty, name: &str) -> String {
        let c = self.ty(path, ty);
        match dir {
            Dir::In => {
                if self.passed_by_value(path, ty) {
                    format!("{c} {name}")
                } else if matches!(self.t.kind(path, ty), Kind::Str) {
                    format!("const CORBA_char *{name}")
                } else {
                    format!("const {c} *{name}")
                }
            }
            Dir::InOut => format!("{c} *{name}"),
        }
    }

    fn members(&mut self, full: &str, out: &mut Vec<String>) {
        let Decl::Struct { base, members, .. } = self.t.decl(full) else {
            panic!("struct {full}");
        };
        let path = self.t.path_of(full).to_vec();
        if let Some(base) = base {
            let base_full = self.t.resolve(&path, base);
            self.members(&base_full, out);
        }
        for m in members {
            let c = self.ty(&path, &m.ty);
            let dims: String = m.dims.iter().map(|d| format!("[{d}]")).collect();
            out.push(format!("    {c} {}{dims};", m.name));
        }
    }
}

pub fn render(flat: &[Flat<'_>], table: &Table<'_>) -> String {
    let mut cx = Ctx {
        t: table,
        inline_sequences: BTreeSet::new(),
    };
    let mut body = String::new();
    let mut forwarded = BTreeSet::new();
    for f in flat {
        let path = &f.path;
        match f.decl {
            Decl::Typedef { name, ty, dims } => {
                let full = super::resolve::full_name(path, name);
                let cname = mangle(&full);
                if !dims.is_empty() {
                    let c = cx.ty(path, ty);
                    let d: String = dims.iter().map(|d| format!("[{d}]")).collect();
                    writeln!(body, "typedef {c} {cname}{d};").unwrap();
                } else if let Ty::Sequence(inner) = ty {
                    let elem = cx.ty(path, inner);
                    writeln!(
                        body,
                        "typedef struct {{\n    CORBA_unsigned_long _maximum;\n    CORBA_unsigned_long _length;\n    {elem} *_buffer;\n    CORBA_boolean _release;\n}} {cname};"
                    )
                    .unwrap();
                } else {
                    let c = cx.ty(path, ty);
                    writeln!(body, "typedef {c} {cname};").unwrap();
                }
            }
            Decl::Const { name, ty, value } => {
                let full = super::resolve::full_name(path, name);
                let c = cx.ty(path, ty);
                writeln!(body, "#define {} (({c})({value}))", mangle(&full)).unwrap();
            }
            Decl::Struct { name, .. } => {
                let full = super::resolve::full_name(path, name);
                let mut members = Vec::new();
                cx.members(&full, &mut members);
                let cname = mangle(&full);
                writeln!(body, "typedef struct {cname} {{").unwrap();
                for m in members {
                    writeln!(body, "{m}").unwrap();
                }
                writeln!(body, "}} {cname};").unwrap();
            }
            Decl::Enum { name, variants } => {
                let prefix = path.join("_");
                let cname = mangle(&super::resolve::full_name(path, name));
                writeln!(body, "typedef enum {{").unwrap();
                for v in variants {
                    writeln!(body, "    {prefix}_{v},").unwrap();
                }
                writeln!(body, "}} {cname};").unwrap();
            }
            Decl::Native { name } => {
                let cname = mangle(&super::resolve::full_name(path, name));
                writeln!(body, "typedef void *{cname};").unwrap();
            }
            Decl::Interface { name, ops, attrs } => {
                let cname = mangle(&super::resolve::full_name(path, name));
                if !forwarded.contains(&cname) {
                    writeln!(body, "typedef CORBA_Object {cname};").unwrap();
                }
                for op in ops {
                    writeln!(body, "{}", prototype(&mut cx, path, &cname, op)).unwrap();
                }
                for a in attrs {
                    let rt = cx.ty(path, &a.ty);
                    writeln!(
                        body,
                        "{rt} {cname}__get_{}({cname} _obj, CORBA_Environment *_ev);",
                        a.name
                    )
                    .unwrap();
                }
            }
            Decl::Forward { name } => {
                let cname = mangle(&super::resolve::full_name(path, name));
                writeln!(body, "typedef CORBA_Object {cname};").unwrap();
                forwarded.insert(cname);
            }
            Decl::Module { .. } => {}
        }
    }
    let mut out = String::new();
    out.push_str(PREAMBLE);
    for s in &cx.inline_sequences {
        let (name, elem) = s.split_once('|').unwrap();
        writeln!(
            out,
            "typedef struct {{\n    CORBA_unsigned_long _maximum;\n    CORBA_unsigned_long _length;\n    {elem} *_buffer;\n    CORBA_boolean _release;\n}} {name};"
        )
        .unwrap();
    }
    out.push('\n');
    out.push_str(&body);
    out.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif\n");
    out
}

fn prototype(cx: &mut Ctx<'_>, path: &[String], iface: &str, op: &Op) -> String {
    let ret = match &op.ret {
        None => "void".to_string(),
        Some(t) => cx.ty(path, t),
    };
    let mut params = vec![format!("{iface} _obj")];
    for p in &op.params {
        params.push(cx.param(path, p.dir, &p.ty, &p.name));
    }
    params.push("CORBA_Environment *_ev".into());
    format!("{ret} {iface}_{}({});", op.name, params.join(", "))
}

const PREAMBLE: &str = "/* Generated from dds_security.idl by the IDL to C mapping rules (DDS-Security 1.2 section 11.3). */\n\
#ifndef DDS_SECURITY_H\n#define DDS_SECURITY_H\n\n\
#include <stdint.h>\n\n\
#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n\
/* The base types of the IDL to C mapping. An ORB supplies them in its own\n   header; they are defined here so that this header stands by itself. */\n\
#ifndef CORBA_C_BASE_TYPES\n#define CORBA_C_BASE_TYPES\n\
typedef int16_t CORBA_short;\ntypedef uint16_t CORBA_unsigned_short;\n\
typedef int32_t CORBA_long;\ntypedef uint32_t CORBA_unsigned_long;\n\
typedef int64_t CORBA_long_long;\ntypedef uint64_t CORBA_unsigned_long_long;\n\
typedef float CORBA_float;\ntypedef double CORBA_double;\n\
typedef unsigned char CORBA_octet;\ntypedef unsigned char CORBA_boolean;\ntypedef char CORBA_char;\n\
typedef void *CORBA_Object;\n\
typedef struct CORBA_Environment {\n    int _major;\n    void *_exception;\n} CORBA_Environment;\n\
#endif\n\n";
