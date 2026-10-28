// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Name resolution over the declarations of `dds_security.idl`.

#![allow(dead_code)]

use super::model::{Decl, Flat, Ty, flatten};
use std::collections::BTreeMap;

pub struct Table<'a> {
    /// Full scoped name (`DDS::Security::Foo`) to the declaration.
    pub by_name: BTreeMap<String, &'a Flat<'a>>,
}

/// What a type is, once typedefs are followed.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Basic(String),
    Str,
    Sequence(Box<Ty>, Vec<String>),
    Array,
    Struct,
    Enum,
    Interface,
    Native,
}

pub fn full_name(path: &[String], name: &str) -> String {
    let mut full = path.join("::");
    if !full.is_empty() {
        full.push_str("::");
    }
    full.push_str(name);
    full
}

impl<'a> Table<'a> {
    pub fn new(flat: &'a [Flat<'a>]) -> Self {
        let mut by_name = BTreeMap::new();
        for f in flat {
            let name = match f.decl {
                Decl::Typedef { name, .. }
                | Decl::Struct { name, .. }
                | Decl::Enum { name, .. }
                | Decl::Native { name }
                | Decl::Interface { name, .. }
                | Decl::Const { name, .. } => name,
                Decl::Forward { .. } | Decl::Module { .. } => continue,
            };
            by_name.insert(full_name(&f.path, name), f);
        }
        Self { by_name }
    }

    /// The full name `name` denotes when written inside module `path`.
    pub fn resolve(&self, path: &[String], name: &str) -> String {
        let mut scope = path.to_vec();
        loop {
            let candidate = full_name(&scope, name);
            if self.by_name.contains_key(&candidate) {
                return candidate;
            }
            if scope.pop().is_none() {
                panic!("unresolved name `{name}` in {path:?}");
            }
        }
    }

    pub fn decl(&self, full: &str) -> &'a Decl {
        self.by_name[full].decl
    }

    pub fn path_of(&self, full: &str) -> &'a [String] {
        &self.by_name[full].path
    }

    /// The kind of `ty` written in `path`, following typedefs. For a typedef
    /// of an array the kind is [`Kind::Array`].
    pub fn kind(&self, path: &[String], ty: &Ty) -> Kind {
        match ty {
            Ty::Basic(b) => Kind::Basic(b.clone()),
            Ty::Str => Kind::Str,
            Ty::Sequence(inner) => Kind::Sequence(inner.clone(), path.to_vec()),
            Ty::Named(n) => {
                let full = self.resolve(path, n);
                match self.decl(&full) {
                    Decl::Typedef { ty, dims, .. } => {
                        if dims.is_empty() {
                            self.kind(self.path_of(&full), ty)
                        } else {
                            Kind::Array
                        }
                    }
                    Decl::Struct { .. } => Kind::Struct,
                    Decl::Enum { .. } => Kind::Enum,
                    Decl::Interface { .. } => Kind::Interface,
                    Decl::Native { .. } => Kind::Native,
                    other => panic!("`{n}` is not a type: {other:?}"),
                }
            }
        }
    }
}

pub fn flat_of(decls: &[Decl]) -> Vec<Flat<'_>> {
    let mut out = Vec::new();
    flatten(decls, &mut Vec::new(), &mut out);
    out
}
