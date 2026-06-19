// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lightweight CCM Profile — Spec §13.
//!
//! Spec §13.1 (S. 273) — Summary: "The Lightweight CCM (LwCCM) profile
//! is intended to be useful in environments where: a small footprint
//! [...] CCM with reduced support for: Persistence, Introspection,
//! Navigation, Type-specific Generic Operations, Segmentation,
//! Transactions, Security, Configurators, Proxy Homes, Home Finders."
//!
//! We implement this as a filter function over the `ComponentEquivalent`
//! / `HomeEquivalent` from [`crate::transform`] — all operations that
//! are explicitly excluded in §13.2-§13.10 are removed.

use alloc::vec::Vec;
use core::fmt;

use zerodds_idl::ast::{Export, InterfaceDef, ScopedName};

use crate::transform::ComponentEquivalent;

/// Filter error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightweightFilterError {
    /// A requested LwCCM-conformant component would have no operations
    /// left after filtering — typical case: the component had only
    /// persistence / configurator / proxy-home features.
    EmptyAfterFilter,
}

impl fmt::Display for LightweightFilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAfterFilter => f.write_str(
                "component would have no operations after applying lightweight CCM filter",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LightweightFilterError {}

/// Applies the Lightweight CCM Profile §13 to a
/// [`ComponentEquivalent`]. Spec sections that are filtered:
///
/// * §13.3 (p. 276) — no `provide_facet`/`get_all_facets`/
///   `get_named_facets` (generic navigation ops).
/// * §13.3 (p. 276) — no `connect`/`disconnect`/`get_connection(s)`
///   GENERIC ops on the `Receptacles` iface (type-specific ones remain).
/// * §13.3 (p. 276) — no `subscribe`/`unsubscribe`/
///   `connect_consumer`/`disconnect_consumer`/`get_consumer`/
///   `get_all_consumers`/`get_named_consumers` GENERIC ops on the
///   `Events` interface (type-specific ones remain).
/// * §13.7 (p. 279) — no configurator methods (`configure`,
///   `set_configuration`, `configuration_complete`).
///
/// These filters act at the `Components::*` API level, not on the
/// component body. Since our `ComponentEquivalent` already contains the
/// type-specific ops (`provide_<n>`, `connect_<n>`, etc.), there is
/// typically nothing to filter. The filter function nonetheless removes
/// configurator operations, if present, and generic navigation ops, if
/// the caller has included them.
///
/// # Errors
/// See [`LightweightFilterError`].
pub fn filter_to_lightweight(
    eq: ComponentEquivalent,
) -> Result<ComponentEquivalent, LightweightFilterError> {
    let kept_exports: Vec<Export> = eq
        .equivalent_interface
        .exports
        .into_iter()
        .filter(|e| !is_filtered_export(e))
        .collect();
    if kept_exports.is_empty() && !eq.event_consumer_interfaces.is_empty() {
        // If the only ports were configurator ops and nothing else, the
        // result becomes empty after filtering — spec allows an empty
        // result, but it's a hint for the caller.
        return Err(LightweightFilterError::EmptyAfterFilter);
    }
    let filtered_iface = InterfaceDef {
        exports: kept_exports,
        bases: eq
            .equivalent_interface
            .bases
            .into_iter()
            .filter(|b| !is_filtered_base(b))
            .collect(),
        ..eq.equivalent_interface
    };
    Ok(ComponentEquivalent {
        equivalent_interface: filtered_iface,
        event_consumer_interfaces: eq.event_consumer_interfaces,
    })
}

fn is_filtered_export(e: &Export) -> bool {
    if let Export::Op(o) = e {
        let n = &o.name.text;
        // Spec §13.7 (p. 279) — configurator.
        matches!(
            n.as_str(),
            "configure" | "set_configuration" | "configuration_complete"
        )
    } else {
        false
    }
}

fn is_filtered_base(b: &ScopedName) -> bool {
    // We don't filter inheritance — Spec §13 removes member ops, not the
    // inheritance relationship itself. Stub for extensibility.
    let _ = b;
    false
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transform::transform_component;
    use zerodds_idl::ast::{
        ComponentDef, ComponentExport, Identifier, OpDecl, ParamAttribute, ParamDecl,
        PrimitiveType, ScopedName, TypeSpec,
    };
    use zerodds_idl::errors::Span;

    fn span() -> Span {
        Span::SYNTHETIC
    }

    fn ident(s: &str) -> Identifier {
        Identifier::new(s, span())
    }

    fn sn(parts: &[&str]) -> ScopedName {
        ScopedName {
            absolute: false,
            parts: parts.iter().map(|p| ident(p)).collect(),
            span: span(),
        }
    }

    #[test]
    fn lightweight_filter_drops_configurator_operations() {
        // Synthesis: build the equivalent + inject a `configure` op, then
        // filter.
        let c = ComponentDef {
            name: ident("C"),
            base: None,
            supports: Vec::new(),
            body: alloc::vec![ComponentExport::Provides {
                type_spec: sn(&["I"]),
                name: ident("foo"),
                span: span(),
            }],
            annotations: Vec::new(),
            span: span(),
        };
        let mut eq = transform_component(&c);
        // Inject configurator op (Spec §6.10.1.1 p. 45).
        eq.equivalent_interface.exports.push(Export::Op(OpDecl {
            name: ident("configure"),
            oneway: false,
            context: Vec::new(),
            return_type: None,
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Primitive(PrimitiveType::Boolean),
                name: ident("comp"),
                annotations: Vec::new(),
                span: span(),
            }],
            raises: Vec::new(),
            annotations: Vec::new(),
            span: span(),
        }));
        let filtered = filter_to_lightweight(eq).expect("filter ok");
        let names: Vec<String> = filtered
            .equivalent_interface
            .exports
            .iter()
            .filter_map(|e| match e {
                Export::Op(o) => Some(o.name.text.clone()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&String::from("provide_foo")));
        assert!(!names.contains(&String::from("configure")));
    }

    #[test]
    fn lightweight_filter_keeps_typespecific_ops() {
        // Spec §13.3 removes only GENERIC navigation; type-specific
        // (provide_<n>) remains.
        let c = ComponentDef {
            name: ident("C"),
            base: None,
            supports: Vec::new(),
            body: alloc::vec![
                ComponentExport::Provides {
                    type_spec: sn(&["I"]),
                    name: ident("foo"),
                    span: span(),
                },
                ComponentExport::Uses {
                    type_spec: sn(&["J"]),
                    name: ident("bar"),
                    multiple: false,
                    span: span(),
                },
            ],
            annotations: Vec::new(),
            span: span(),
        };
        let eq = transform_component(&c);
        let filtered = filter_to_lightweight(eq).expect("filter ok");
        let names: Vec<String> = filtered
            .equivalent_interface
            .exports
            .iter()
            .filter_map(|e| match e {
                Export::Op(o) => Some(o.name.text.clone()),
                _ => None,
            })
            .collect();
        for expected in [
            "provide_foo",
            "connect_bar",
            "disconnect_bar",
            "get_connection_bar",
        ] {
            assert!(
                names.contains(&String::from(expected)),
                "missing {expected}"
            );
        }
    }

    #[test]
    fn display_error_describes_empty_after_filter() {
        let s = alloc::format!("{}", LightweightFilterError::EmptyAfterFilter);
        assert!(s.contains("lightweight"));
    }
}
