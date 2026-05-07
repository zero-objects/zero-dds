// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! JNDI ↔ CosNaming Glue.
//!
//! JEE benutzt JNDI mit Pfaden wie `java:global/<app>/<module>/<bean>`,
//! CORBA verwendet CosNaming mit `<name>.<kind>/<name>.<kind>`.
//!
//! Wir mappen:
//!
//! * `java:global/X/Y/Bean` ↔ CosNaming `[("X", ""), ("Y", ""),
//!   ("Bean", "")]`
//!
//! Dies ist eine pragmatische Convention — Spec CCM 4.0 §11
//! (Container Programming Model) gibt keinen Standard fuer JNDI-
//! Mapping vor.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Eine JNDI-Bindung — Name + zugewiesene Object-Reference (als
/// opaque IOR-Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JndiBinding {
    /// Vollqualifizierter JNDI-Name (z.B. `java:global/my-app/Echo`).
    pub name: String,
    /// IOR-Bytes (kein Codec — Caller-Layer).
    pub ior: Vec<u8>,
}

/// JNDI-Context mit linearer Lookup-Tabelle. Production-Caller wird
/// das auf einen JNDI-Provider mappen; In-Memory-Form ist fuer Tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct JndiContext {
    bindings: Vec<JndiBinding>,
}

impl JndiContext {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bindet einen Namen an eine IOR.
    pub fn bind(&mut self, name: String, ior: Vec<u8>) {
        if let Some(b) = self.bindings.iter_mut().find(|b| b.name == name) {
            b.ior = ior;
        } else {
            self.bindings.push(JndiBinding { name, ior });
        }
    }

    /// Liefert die IOR-Bytes fuer einen Namen (kopiert).
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Vec<u8>> {
        self.bindings
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.ior.clone())
    }

    /// Loescht eine Bindung.
    pub fn unbind(&mut self, name: &str) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.name != name);
        before != self.bindings.len()
    }

    /// Liste aller Bindings.
    #[must_use]
    pub fn list(&self) -> &[JndiBinding] {
        &self.bindings
    }
}

/// Konvertiert einen JNDI-Pfad in eine CosNaming-NameComponent-Liste.
///
/// `java:global/foo/bar/Bean` → `[("foo",""), ("bar",""), ("Bean","")]`
/// Der `java:global/`-Prefix wird gestripped.
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

/// Konvertiert eine CosNaming-NameComponent-Liste zurueck in einen
/// `java:global/...`-Pfad.
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
