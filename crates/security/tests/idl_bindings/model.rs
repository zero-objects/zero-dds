// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! A reader for the subset of OMG IDL that `dds_security.idl` uses: modules,
//! typedefs (also of sequences and arrays), constants, structures with a base,
//! enumerations, native types, `local interface` declarations with operations,
//! and annotations (read and dropped, except `@extensibility`).

#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// A basic IDL type by its IDL spelling (`long`, `unsigned long long`,
    /// `octet`, `boolean`, `uint32`, ...).
    Basic(String),
    Str,
    Sequence(Box<Ty>),
    /// A scoped or plain name as written.
    Named(String),
}

#[derive(Debug, Clone)]
pub struct Member {
    pub ty: Ty,
    pub name: String,
    pub dims: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dir {
    In,
    InOut,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub dir: Dir,
    pub ty: Ty,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Op {
    pub ret: Option<Ty>,
    pub name: String,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct Attr {
    pub ty: Ty,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Module {
        name: String,
        decls: Vec<Decl>,
    },
    Typedef {
        name: String,
        ty: Ty,
        dims: Vec<usize>,
    },
    Const {
        name: String,
        ty: Ty,
        value: String,
    },
    Struct {
        name: String,
        base: Option<String>,
        members: Vec<Member>,
        extensibility: String,
    },
    Enum {
        name: String,
        variants: Vec<String>,
    },
    Native {
        name: String,
    },
    Interface {
        name: String,
        ops: Vec<Op>,
        attrs: Vec<Attr>,
    },
    Forward {
        name: String,
    },
}

struct Lexer<'a> {
    text: &'a str,
    at: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Id(String),
    Num(String),
    Punct(char),
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Option<Tok> {
        let bytes = self.text.as_bytes();
        loop {
            while self.at < bytes.len() && bytes[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.text[self.at..].starts_with("//") {
                while self.at < bytes.len() && bytes[self.at] != b'\n' {
                    self.at += 1;
                }
                continue;
            }
            break;
        }
        if self.at >= bytes.len() {
            return None;
        }
        let c = bytes[self.at] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = self.at;
            while self.at < bytes.len()
                && ((bytes[self.at] as char).is_ascii_alphanumeric() || bytes[self.at] == b'_')
            {
                self.at += 1;
            }
            return Some(Tok::Id(self.text[start..self.at].to_string()));
        }
        if c.is_ascii_digit() {
            let start = self.at;
            while self.at < bytes.len() && (bytes[self.at] as char).is_ascii_alphanumeric() {
                self.at += 1;
            }
            return Some(Tok::Num(self.text[start..self.at].to_string()));
        }
        if c == ':' && self.text[self.at..].starts_with("::") {
            self.at += 2;
            return Some(Tok::Punct('#')); // scope operator
        }
        self.at += 1;
        Some(Tok::Punct(c))
    }
}

pub struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    pub fn new(text: &str) -> Self {
        let mut lexer = Lexer { text, at: 0 };
        let mut toks = Vec::new();
        while let Some(t) = lexer.next() {
            toks.push(t);
        }
        Self { toks, at: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.at].clone();
        self.at += 1;
        t
    }

    fn is_id(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(x)) if x == s)
    }

    fn is_p(&self, c: char) -> bool {
        matches!(self.peek(), Some(Tok::Punct(x)) if *x == c)
    }

    fn eat_p(&mut self, c: char) {
        assert!(
            self.is_p(c),
            "expected `{c}` at token {} ({:?})",
            self.at,
            self.peek()
        );
        self.at += 1;
    }

    fn id(&mut self) -> String {
        match self.bump() {
            Tok::Id(s) => s,
            other => panic!("identifier expected, got {other:?} at {}", self.at),
        }
    }

    /// Annotations `@name` or `@name(...)`: returns `extensibility` if given.
    fn annotations(&mut self) -> Option<String> {
        let mut extensibility = None;
        while self.is_p('@') {
            self.at += 1;
            let name = self.id();
            if self.is_p('(') {
                self.at += 1;
                let mut depth = 1;
                let mut arg = String::new();
                while depth > 0 {
                    match self.bump() {
                        Tok::Punct('(') => depth += 1,
                        Tok::Punct(')') => depth -= 1,
                        Tok::Id(s) | Tok::Num(s) if depth == 1 => arg.push_str(&s),
                        _ => {}
                    }
                }
                if name == "extensibility" {
                    extensibility = Some(arg);
                }
            }
        }
        extensibility
    }

    fn scoped(&mut self) -> String {
        let mut name = self.id();
        while self.is_p('#') {
            self.at += 1;
            name.push_str("::");
            name.push_str(&self.id());
        }
        name
    }

    fn ty(&mut self) -> Ty {
        let first = self.id();
        match first.as_str() {
            "string" => Ty::Str,
            "sequence" => {
                self.eat_p('<');
                let inner = self.ty();
                self.eat_p('>');
                Ty::Sequence(Box::new(inner))
            }
            "unsigned" => {
                let next = self.id();
                if next == "long" && self.is_id("long") {
                    self.at += 1;
                    Ty::Basic("unsigned long long".into())
                } else {
                    Ty::Basic(format!("unsigned {next}"))
                }
            }
            "long" => {
                if self.is_id("long") {
                    self.at += 1;
                    Ty::Basic("long long".into())
                } else {
                    Ty::Basic("long".into())
                }
            }
            "short" | "octet" | "boolean" | "int32" | "uint32" | "char" | "float" | "double" => {
                Ty::Basic(first)
            }
            _ => {
                let mut name = first;
                while self.is_p('#') {
                    self.at += 1;
                    name.push_str("::");
                    name.push_str(&self.id());
                }
                Ty::Named(name)
            }
        }
    }

    fn dims(&mut self) -> Vec<usize> {
        let mut dims = Vec::new();
        while self.is_p('[') {
            self.at += 1;
            let Tok::Num(n) = self.bump() else {
                panic!("array size expected")
            };
            dims.push(n.parse().expect("array size"));
            self.eat_p(']');
        }
        dims
    }

    pub fn file(&mut self) -> Vec<Decl> {
        let mut decls = Vec::new();
        while self.peek().is_some() {
            decls.push(self.decl());
        }
        decls
    }

    fn decl(&mut self) -> Decl {
        let extensibility = self.annotations().unwrap_or_else(|| "APPENDABLE".into());
        let kw = self.id();
        match kw.as_str() {
            "module" => {
                let name = self.id();
                self.eat_p('{');
                let mut decls = Vec::new();
                while !self.is_p('}') {
                    decls.push(self.decl());
                }
                self.eat_p('}');
                self.eat_p(';');
                Decl::Module { name, decls }
            }
            "typedef" => {
                let ty = self.ty();
                let name = self.id();
                let dims = self.dims();
                self.eat_p(';');
                Decl::Typedef { name, ty, dims }
            }
            "const" => {
                let ty = self.ty();
                let name = self.id();
                self.eat_p('=');
                let mut value = String::new();
                while !self.is_p(';') {
                    match self.bump() {
                        Tok::Id(s) | Tok::Num(s) => value.push_str(&s),
                        Tok::Punct(c) => value.push(c),
                    }
                }
                self.eat_p(';');
                Decl::Const { name, ty, value }
            }
            "struct" => {
                let name = self.id();
                let base = if self.is_p(':') {
                    self.at += 1;
                    Some(self.scoped())
                } else {
                    None
                };
                self.eat_p('{');
                let mut members = Vec::new();
                while !self.is_p('}') {
                    self.annotations();
                    let ty = self.ty();
                    let name = self.id();
                    let dims = self.dims();
                    self.eat_p(';');
                    members.push(Member { ty, name, dims });
                }
                self.eat_p('}');
                self.eat_p(';');
                Decl::Struct {
                    name,
                    base,
                    members,
                    extensibility,
                }
            }
            "enum" => {
                let name = self.id();
                self.eat_p('{');
                let mut variants = Vec::new();
                loop {
                    variants.push(self.id());
                    if self.is_p(',') {
                        self.at += 1;
                    } else {
                        break;
                    }
                }
                self.eat_p('}');
                self.eat_p(';');
                Decl::Enum { name, variants }
            }
            "native" => {
                let name = self.id();
                self.eat_p(';');
                Decl::Native { name }
            }
            "local" => {
                let interface = self.id();
                assert_eq!(interface, "interface");
                self.interface()
            }
            "interface" => self.interface(),
            other => panic!("unsupported declaration `{other}` at token {}", self.at),
        }
    }

    fn interface(&mut self) -> Decl {
        let name = self.id();
        if self.is_p(';') {
            self.at += 1;
            return Decl::Forward { name };
        }
        self.eat_p('{');
        let mut ops = Vec::new();
        let mut attrs = Vec::new();
        while !self.is_p('}') {
            if self.is_id("readonly") {
                self.at += 1;
                let a = self.id();
                assert_eq!(a, "attribute");
                let ty = self.ty();
                let name = self.id();
                self.eat_p(';');
                attrs.push(Attr { ty, name });
                continue;
            }
            let ret = if self.is_id("void") {
                self.at += 1;
                None
            } else {
                Some(self.ty())
            };
            let op_name = self.id();
            self.eat_p('(');
            let mut params = Vec::new();
            while !self.is_p(')') {
                let dir = match self.id().as_str() {
                    "in" => Dir::In,
                    "inout" => Dir::InOut,
                    other => panic!("parameter direction `{other}`"),
                };
                let ty = self.ty();
                let pname = self.id();
                params.push(Param {
                    dir,
                    ty,
                    name: pname,
                });
                if self.is_p(',') {
                    self.at += 1;
                }
            }
            self.eat_p(')');
            self.eat_p(';');
            ops.push(Op {
                ret,
                name: op_name,
                params,
            });
        }
        self.eat_p('}');
        self.eat_p(';');
        Decl::Interface { name, ops, attrs }
    }
}

pub fn parse(text: &str) -> Vec<Decl> {
    Parser::new(text).file()
}

/// A flat view: every declaration with its module path.
pub struct Flat<'a> {
    pub path: Vec<String>,
    pub decl: &'a Decl,
}

pub fn flatten<'a>(decls: &'a [Decl], path: &mut Vec<String>, out: &mut Vec<Flat<'a>>) {
    for d in decls {
        match d {
            Decl::Module { name, decls } => {
                path.push(name.clone());
                flatten(decls, path, out);
                path.pop();
            }
            other => out.push(Flat {
                path: path.clone(),
                decl: other,
            }),
        }
    }
}
