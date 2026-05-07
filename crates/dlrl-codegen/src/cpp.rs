// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C++-Codegen — DDS 1.4 §B.4 Annex C++ PSM.

use alloc::format;
use alloc::string::String;

use crate::DlrlTypeInfo;

/// Erzeugt einen C++ ObjectRoot-abgeleiteten Class-Header
/// (`class T : public DLRL::ObjectRoot`). Spec §B.6.
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_cpp_object(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let mut out = format!(
        "// Generated DLRL Object — Spec §B.6\n\
class {cls} : public DLRL::ObjectRoot {{\npublic:\n"
    );
    for k in &info.keys {
        out.push_str(&format!("    DDS::Long {k}_;\n"));
        out.push_str(&format!(
            "    DDS::Long get_{k}() const {{ return {k}_; }}\n"
        ));
    }
    for (rel, target) in &info.relations {
        let target_cls = simple_name(target);
        out.push_str(&format!(
            "    DLRL::RefList< {target_cls}* > {rel}_;\n\
    DLRL::RefList< {target_cls}* >& {rel}() {{ return {rel}_; }}\n"
        ));
    }
    out.push_str("};\n");
    out
}

/// Erzeugt eine C++-Home-Class fuer einen DLRL-Type. Spec §B.3.
#[must_use]
pub fn generate_cpp_home(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    format!(
        "// Generated DLRL Home — Spec §B.3\n\
class {cls}Home : public DLRL::HomeBase {{\npublic:\n\
    {cls}* create();\n\
    void destroy({cls}* obj);\n\
    {cls}* find_by_key(/* key fields */);\n\
}};\n"
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
    fn cpp_object_inherits_object_root() {
        let s = generate_cpp_object(&trade_info());
        assert!(s.contains("class Trade : public DLRL::ObjectRoot"));
        assert!(s.contains("symbol_"));
        assert!(s.contains("get_symbol"));
        assert!(s.contains("RefList< Quote* > quotes_"));
    }

    #[test]
    fn cpp_home_inherits_home_base() {
        let s = generate_cpp_home(&trade_info());
        assert!(s.contains("class TradeHome : public DLRL::HomeBase"));
        assert!(s.contains("Trade* create();"));
        assert!(s.contains("void destroy(Trade* obj);"));
    }

    #[test]
    fn simple_name_strips_scope() {
        assert_eq!(simple_name("a::b::C"), "C");
        assert_eq!(simple_name("Foo"), "Foo");
    }
}
