// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! JNDI ↔ CosNaming glue.
//!
//! JEE uses JNDI with paths like `java:global/<app>/<module>/<bean>`,
//! CORBA uses CosNaming with `<name>.<kind>/<name>.<kind>`.
//!
//! We map:
//!
//! * `java:global/X/Y/Bean` ↔ CosNaming `[("X", ""), ("Y", ""),
//!   ("Bean", "")]`
//!
//! This is a pragmatic convention — Spec CCM 4.0 §11 (Container
//! Programming Model) does not prescribe a standard for JNDI mapping.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A JNDI binding — name + associated object reference (as opaque IOR
/// bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JndiBinding {
    /// Fully qualified JNDI name (e.g. `java:global/my-app/Echo`).
    pub name: String,
    /// IOR bytes (no codec — caller-layer).
    pub ior: Vec<u8>,
}

/// JNDI context with a linear lookup table. A production caller will
/// map this onto a JNDI provider; the in-memory form is for tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct JndiContext {
    bindings: Vec<JndiBinding>,
}

impl JndiContext {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a name to an IOR.
    pub fn bind(&mut self, name: String, ior: Vec<u8>) {
        if let Some(b) = self.bindings.iter_mut().find(|b| b.name == name) {
            b.ior = ior;
        } else {
            self.bindings.push(JndiBinding { name, ior });
        }
    }

    /// Returns the IOR bytes for a name (copied).
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Vec<u8>> {
        self.bindings
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.ior.clone())
    }

    /// Removes a binding.
    pub fn unbind(&mut self, name: &str) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.name != name);
        before != self.bindings.len()
    }

    /// List of all bindings.
    #[must_use]
    pub fn list(&self) -> &[JndiBinding] {
        &self.bindings
    }
}

/// Converts a JNDI path into a CosNaming NameComponent list.
///
/// `java:global/foo/bar/Bean` → `[("foo",""), ("bar",""), ("Bean","")]`
/// The `java:global/` prefix is stripped.
#[must_use]
pub fn jndi_to_cos_naming(jndi: &str) -> Vec<(String, String)> {
    let path = jndi
        .strip_prefix("java:global/")
        .or_else(|| jndi.strip_prefix("java:comp/env/"))
        .unwrap_or(jndi);
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| (s.to_string(), String::new()))
        .collect()
}

/// Converts a CosNaming NameComponent list back into a
/// `java:global/...` path.
#[must_use]
pub fn cos_naming_to_jndi(name: &[(String, String)]) -> String {
    let mut out = String::from("java:global/");
    for (i, (id, kind)) in name.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(id);
        if !kind.is_empty() {
            out.push('.');
            out.push_str(kind);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn bind_and_lookup_round_trip() {
        let mut ctx = JndiContext::new();
        ctx.bind("java:global/Echo".into(), vec![1, 2, 3]);
        assert_eq!(ctx.lookup("java:global/Echo"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn rebind_replaces_ior() {
        let mut ctx = JndiContext::new();
        ctx.bind("java:global/Echo".into(), vec![1]);
        ctx.bind("java:global/Echo".into(), vec![2]);
        assert_eq!(ctx.list().len(), 1);
        assert_eq!(ctx.lookup("java:global/Echo"), Some(vec![2]));
    }

    #[test]
    fn unbind_removes_entry() {
        let mut ctx = JndiContext::new();
        ctx.bind("java:global/Echo".into(), vec![1]);
        assert!(ctx.unbind("java:global/Echo"));
        assert!(ctx.lookup("java:global/Echo").is_none());
    }

    #[test]
    fn unbind_unknown_returns_false() {
        let mut ctx = JndiContext::new();
        assert!(!ctx.unbind("nope"));
    }

    #[test]
    fn jndi_global_strips_prefix() {
        let n = jndi_to_cos_naming("java:global/app/module/Echo");
        assert_eq!(
            n,
            vec![
                ("app".into(), String::new()),
                ("module".into(), String::new()),
                ("Echo".into(), String::new()),
            ]
        );
    }

    #[test]
    fn jndi_comp_env_strips_prefix() {
        let n = jndi_to_cos_naming("java:comp/env/Echo");
        assert_eq!(n, vec![("Echo".into(), String::new())]);
    }

    #[test]
    fn cos_to_jndi_round_trip_no_kind() {
        let name = vec![
            ("app".into(), String::new()),
            ("Echo".into(), String::new()),
        ];
        assert_eq!(cos_naming_to_jndi(&name), "java:global/app/Echo");
    }

    #[test]
    fn cos_to_jndi_includes_kind() {
        let name = vec![("Echo".into(), "Component".into())];
        assert_eq!(cos_naming_to_jndi(&name), "java:global/Echo.Component");
    }
}
