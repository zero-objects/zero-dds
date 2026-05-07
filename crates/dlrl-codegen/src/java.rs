// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Java-Codegen — DDS 1.4 §B.4 Annex Java-PSM.

use alloc::format;
use alloc::string::String;

use crate::DlrlTypeInfo;

/// Erzeugt einen Java-Class-Source fuer einen DLRL-Object-Type.
/// Spec §B.6 Java-PSM: extends `ObjectRoot`.
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_java_object(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let pkg = package(&info.name);
    let key_lines: String = info
        .keys
        .iter()
        .map(|k| format!("    @DlrlKey\n    public long {k};\n"))
        .collect();
    let rel_lines: String = info
        .relations
        .iter()
        .map(|(rel, target)| {
            let t = simple_name(target);
            format!(
                "    @DlrlRelation(target = {t}.class)\n    public RefList<{t}> {rel} = new RefList<>();\n"
            )
        })
        .collect();
    let header = if pkg.is_empty() {
        String::new()
    } else {
        format!("package {pkg};\n\n")
    };
    format!(
        "{header}// Generated DLRL Object — Spec §B.6 Java-PSM\n\
@DlrlObject\npublic class {cls} extends ObjectRoot {{\n{key_lines}{rel_lines}}}\n"
    )
}

/// Erzeugt einen `ObjectListener<T>`-Interface-Stub. Spec §B.6.7.
#[must_use]
pub fn generate_java_object_listener(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let pkg = package(&info.name);
    let header = if pkg.is_empty() {
        String::new()
    } else {
        format!("package {pkg};\n\n")
    };
    format!(
        "{header}// Generated DLRL ObjectListener — Spec §B.6.7\n\
public interface {cls}Listener extends ObjectListener<{cls}> {{\n\
    void on{cls}Created({cls} obj);\n\
    void on{cls}Modified({cls} obj);\n\
    void on{cls}Deleted({cls} obj);\n\
}}\n"
    )
}

fn simple_name(scoped: &str) -> &str {
    scoped.rsplit("::").next().unwrap_or(scoped)
}

fn package(scoped: &str) -> String {
    let parts: alloc::vec::Vec<&str> = scoped.split("::").collect();
    if parts.len() <= 1 {
        return String::new();
    }
    parts[..parts.len() - 1].join(".")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn trade_info() -> DlrlTypeInfo {
        DlrlTypeInfo {
            name: "demo::Trade".into(),
            keys: alloc::vec!["symbol".into()],
            relations: alloc::vec![("quotes".into(), "demo::Quote".into())],
        }
    }

    #[test]
    fn java_object_emits_package_and_annotations() {
        let s = generate_java_object(&trade_info());
        assert!(s.contains("package demo;"));
        assert!(s.contains("@DlrlObject"));
        assert!(s.contains("class Trade extends ObjectRoot"));
        assert!(s.contains("@DlrlKey"));
        assert!(s.contains("public long symbol;"));
        assert!(s.contains("@DlrlRelation(target = Quote.class)"));
        assert!(s.contains("RefList<Quote> quotes"));
    }

    #[test]
    fn java_object_no_package_for_unscoped() {
        let info = DlrlTypeInfo {
            name: "Foo".into(),
            ..DlrlTypeInfo::default()
        };
        let s = generate_java_object(&info);
        assert!(!s.contains("package "));
        assert!(s.contains("class Foo extends ObjectRoot"));
    }

    #[test]
    fn listener_interface_emits_three_callbacks() {
        let s = generate_java_object_listener(&trade_info());
        assert!(s.contains("interface TradeListener extends ObjectListener<Trade>"));
        assert!(s.contains("void onTradeCreated(Trade obj);"));
        assert!(s.contains("void onTradeModified(Trade obj);"));
        assert!(s.contains("void onTradeDeleted(Trade obj);"));
    }
}
