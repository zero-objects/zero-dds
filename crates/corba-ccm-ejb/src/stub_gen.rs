// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Java-CCM-Stub-Codegen — CCM 4.0 Annex A Java-PSM.
//!
//! Erzeugt Java-Quellcode fuer:
//!
//! * `<Comp>Bean` — Stateful-Session-Bean mit `@Stateful` + Lifecycle-
//!   Hooks (`@PostConstruct` etc.) und Delegationen an die CCM-
//!   ComponentExecutor-Methoden.
//! * `<Home>Home` — Home-Local-Interface (CCM-Home-Operations).
//!
//! Der Codegen ist textuell (kein Bytecode); Output ist der Inhalt
//! eines `.java`-Files. Caller-Layer (idl-java-Pipeline) routet die
//! Datei an den richtigen Pfad.

use alloc::format;
use alloc::string::String;

/// Stub-Variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StubKind {
    /// `@Stateful`-Bean mit Lifecycle-Hooks.
    StatefulBean,
    /// `@Stateless`-Bean (z.B. fuer Service-Components).
    StatelessBean,
    /// Home-Local-Interface (CCM 4.0 §6.7).
    HomeLocal,
}

/// Erzeugt Java-Source fuer einen Component-Bean-Stub.
///
/// Spec CCM 4.0 Annex A, Java-PSM:
/// `class <Comp>Bean implements <Comp>Operations` mit
/// JEE-Lifecycle-Annotations.
#[must_use]
pub fn generate_bean_stub(component_name: &str, package: &str, kind: StubKind) -> String {
    match kind {
        StubKind::StatefulBean => generate_stateful_bean(component_name, package),
        StubKind::StatelessBean => generate_stateless_bean(component_name, package),
        StubKind::HomeLocal => generate_home_local(component_name, package),
    }
}

fn generate_stateful_bean(component_name: &str, package: &str) -> String {
    format!(
        r#"package {package};

import javax.ejb.Stateful;
import javax.annotation.PostConstruct;
import javax.annotation.PreDestroy;

/**
 * CCM-EJB-Bridge-Stub für Component {component_name}.
 * Generiert von zerodds-corba-ccm-ejb (Spec CCM 4.0 Annex A Java-PSM).
 */
@Stateful
public class {component_name}Bean implements {component_name}Operations {{
    private {component_name}Executor executor;

    @PostConstruct
    public void postConstruct() {{
        executor = new {component_name}Executor();
        executor.set_context(null);
        executor.ccm_activate();
    }}

    @PreDestroy
    public void preDestroy() {{
        executor.ccm_passivate();
        executor.ccm_remove();
    }}
}}
"#
    )
}

fn generate_stateless_bean(component_name: &str, package: &str) -> String {
    format!(
        r#"package {package};

import javax.ejb.Stateless;

/**
 * CCM-EJB-Bridge-Stub für Service-Component {component_name}.
 * Generiert von zerodds-corba-ccm-ejb (Spec CCM 4.0 Annex A Java-PSM).
 */
@Stateless
public class {component_name}Bean implements {component_name}Operations {{
    private {component_name}Executor executor = new {component_name}Executor();
}}
"#
    )
}

fn generate_home_local(component_name: &str, package: &str) -> String {
    format!(
        r#"package {package};

import javax.ejb.Local;

/**
 * CCM-Home-Local-Interface für Component {component_name}.
 * Generiert von zerodds-corba-ccm-ejb (Spec CCM 4.0 §6.7).
 */
@Local
public interface {component_name}Home {{
    {component_name} create();
    void remove({component_name} comp);
}}
"#
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn stateful_bean_has_postconstruct_and_predestroy() {
        let s = generate_bean_stub("Echo", "demo", StubKind::StatefulBean);
        assert!(s.contains("@Stateful"));
        assert!(s.contains("@PostConstruct"));
        assert!(s.contains("@PreDestroy"));
        assert!(s.contains("class EchoBean implements EchoOperations"));
        assert!(s.contains("package demo;"));
    }

    #[test]
    fn stateless_bean_has_no_lifecycle_hooks() {
        let s = generate_bean_stub("Calculator", "demo", StubKind::StatelessBean);
        assert!(s.contains("@Stateless"));
        assert!(!s.contains("@PostConstruct"));
        assert!(!s.contains("@PreDestroy"));
    }

    #[test]
    fn home_local_uses_local_annotation() {
        let s = generate_bean_stub("Echo", "demo", StubKind::HomeLocal);
        assert!(s.contains("@Local"));
        assert!(s.contains("interface EchoHome"));
        assert!(s.contains("Echo create();"));
        assert!(s.contains("void remove(Echo comp);"));
    }

    #[test]
    fn package_declaration_uses_argument() {
        let s = generate_bean_stub("Foo", "com.example.bar", StubKind::StatefulBean);
        assert!(s.starts_with("package com.example.bar;"));
    }

    #[test]
    fn stub_kind_variants_distinct() {
        assert_ne!(StubKind::StatefulBean, StubKind::StatelessBean);
        assert_ne!(StubKind::StatefulBean, StubKind::HomeLocal);
        assert_ne!(StubKind::StatelessBean, StubKind::HomeLocal);
    }
}
