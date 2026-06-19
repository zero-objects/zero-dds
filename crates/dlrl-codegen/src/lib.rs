// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DLRL code-generation helpers — DDS 1.4 §B.4.
//!
//! Crate `zerodds-dlrl-codegen`. Safety classification: **STANDARD**.
//!
//! Generates language-specific boilerplate for the DLRL pragmas
//! (`DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION`).
//!
//! # Consumes
//!
//! A list of [`zerodds_dlrl::pragma::DlrlPragma`] values that the
//! frontend parser has already validated. From these pragmas, a home
//! class plus object class is generated per `DCPS_DATA_TYPE`, a key
//! field hint per `DCPS_DATA_KEY`, and a relationship accessor method
//! per `DCPS_DLRL_RELATION`.
//!
//! # Backends
//!
//! * `cpp`   — C++ headers + inline implementations.
//! * `csharp`— C# partial classes with `[DlrlObject]` attributes.
//! * `java`  — Java interfaces + skeleton implementations.
//! * `ts`    — TypeScript interfaces + class skeletons.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod cpp;
pub mod csharp;
pub mod java;
pub mod ts;

pub use cpp::{generate_cpp_home, generate_cpp_object};
pub use csharp::{generate_csharp_object, generate_csharp_partial};
pub use java::{generate_java_object, generate_java_object_listener};
pub use ts::{generate_ts_class, generate_ts_interface};

use alloc::string::String;
use alloc::vec::Vec;
use zerodds_dlrl::pragma::DlrlPragma;

/// Aggregated type info — all pragmas that pertain to a single type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DlrlTypeInfo {
    /// Fully qualified type name (`demo::Trade`).
    pub name: String,
    /// List of key fields (order as in the IDL).
    pub keys: Vec<String>,
    /// List of relationships (relation_name, target_type).
    pub relations: Vec<(String, String)>,
}

/// Collects a `DlrlTypeInfo` map from a flat pragma list
/// (key = type name).
#[must_use]
pub fn collect_type_infos(
    pragmas: &[DlrlPragma],
) -> alloc::collections::BTreeMap<String, DlrlTypeInfo> {
    use alloc::collections::BTreeMap;
    let mut out: BTreeMap<String, DlrlTypeInfo> = BTreeMap::new();
    for p in pragmas {
        match p {
            DlrlPragma::DataType { name } => {
                out.entry(name.clone()).or_insert_with(|| DlrlTypeInfo {
                    name: name.clone(),
                    ..DlrlTypeInfo::default()
                });
            }
            DlrlPragma::DataKey { type_name, field } => {
                out.entry(type_name.clone())
                    .or_insert_with(|| DlrlTypeInfo {
                        name: type_name.clone(),
                        ..DlrlTypeInfo::default()
                    })
                    .keys
                    .push(field.clone());
            }
            DlrlPragma::DlrlRelation {
                type_name,
                relation,
                target,
            } => {
                out.entry(type_name.clone())
                    .or_insert_with(|| DlrlTypeInfo {
                        name: type_name.clone(),
                        ..DlrlTypeInfo::default()
                    })
                    .relations
                    .push((relation.clone(), target.clone()));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn collect_groups_by_type_name() {
        let pragmas = alloc::vec![
            DlrlPragma::DataType {
                name: "demo::Trade".into()
            },
            DlrlPragma::DataKey {
                type_name: "demo::Trade".into(),
                field: "symbol".into(),
            },
            DlrlPragma::DataKey {
                type_name: "demo::Trade".into(),
                field: "venue".into(),
            },
            DlrlPragma::DlrlRelation {
                type_name: "demo::Trade".into(),
                relation: "quotes".into(),
                target: "demo::Quote".into(),
            },
        ];
        let infos = collect_type_infos(&pragmas);
        assert_eq!(infos.len(), 1);
        let trade = infos.get("demo::Trade").unwrap();
        assert_eq!(
            trade.keys,
            alloc::vec!["symbol".to_string(), "venue".into()]
        );
        assert_eq!(trade.relations.len(), 1);
        assert_eq!(trade.relations[0].0, "quotes");
    }

    #[test]
    fn data_type_without_keys_yields_empty_lists() {
        let pragmas = alloc::vec![DlrlPragma::DataType { name: "Foo".into() }];
        let infos = collect_type_infos(&pragmas);
        assert!(infos.get("Foo").unwrap().keys.is_empty());
        assert!(infos.get("Foo").unwrap().relations.is_empty());
    }

    #[test]
    fn keys_without_data_type_still_create_info() {
        let pragmas = alloc::vec![DlrlPragma::DataKey {
            type_name: "Bar".into(),
            field: "k".into(),
        }];
        let infos = collect_type_infos(&pragmas);
        assert!(infos.contains_key("Bar"));
        assert_eq!(infos["Bar"].keys, alloc::vec!["k".to_string()]);
    }
}
