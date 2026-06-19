// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TypeScript-Codegen — DDS-XRCE 1.0 + DDS-TS 1.0 (Annex Web-PSM).

use alloc::format;
use alloc::string::String;

use crate::DlrlTypeInfo;

/// Generates a TypeScript interface with key markers (`__dlrlKey: true`).
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_ts_interface(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let key_lines: String = info
        .keys
        .iter()
        .map(|k| format!("  /** DLRL key field. */ {k}: number;\n"))
        .collect();
    let rel_lines: String = info
        .relations
        .iter()
        .map(|(rel, target)| {
            let t = simple_name(target);
            format!("  /** DLRL relation → {t} */ {rel}: ReadonlyArray<{t}>;\n")
        })
        .collect();
    format!(
        "// Generated DLRL Interface — Spec §B.6 (TS PSM)\n\
export interface {cls} {{\n{key_lines}{rel_lines}}}\n"
    )
}

/// Generates a TypeScript class with a home-factory stub.
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_ts_class(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let key_lines: String = info
        .keys
        .iter()
        .map(|k| format!("  {k}: number = 0;\n"))
        .collect();
    let rel_lines: String = info
        .relations
        .iter()
        .map(|(rel, target)| {
            let t = simple_name(target);
            format!("  {rel}: {t}[] = [];\n")
        })
        .collect();
    format!(
        "// Generated DLRL Class — Spec §B.6 (TS PSM)\n\
export class {cls}Impl implements {cls} {{\n{key_lines}{rel_lines}}}\n\n\
export class {cls}Home {{\n  create(): {cls}Impl {{ return new {cls}Impl(); }}\n  destroy(_obj: {cls}Impl): void {{}}\n}}\n"
    )
}

fn simple_name(scoped: &str) -> &str {
    scoped.rsplit("::").next().unwrap_or(scoped)
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
    fn ts_interface_emits_keys_and_relations() {
        let s = generate_ts_interface(&trade_info());
        assert!(s.contains("export interface Trade"));
        assert!(s.contains("symbol: number;"));
        assert!(s.contains("quotes: ReadonlyArray<Quote>;"));
    }

    #[test]
    fn ts_class_emits_impl_and_home() {
        let s = generate_ts_class(&trade_info());
        assert!(s.contains("class TradeImpl implements Trade"));
        assert!(s.contains("symbol: number = 0;"));
        assert!(s.contains("quotes: Quote[] = [];"));
        assert!(s.contains("class TradeHome"));
        assert!(s.contains("create(): TradeImpl"));
        assert!(s.contains("destroy(_obj: TradeImpl): void"));
    }
}
