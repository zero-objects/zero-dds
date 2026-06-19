// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C# codegen — DDS 1.4 §B.4 (analogous to the C++ PSM, dotnet form).

use alloc::format;
use alloc::string::String;

use crate::DlrlTypeInfo;

/// Generates a `partial class` with a `[DlrlObject]` attribute.
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_csharp_partial(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let ns = namespace(&info.name);
    let key_attrs: String = info
        .keys
        .iter()
        .map(|k| format!("    [DlrlKey] public long {k} {{ get; set; }}\n"))
        .collect();
    let rel_attrs: String = info
        .relations
        .iter()
        .map(|(rel, target)| {
            let t = simple_name(target);
            format!(
                "    [DlrlRelation(typeof({t}))] public RefList<{t}> {rel} {{ get; }} = new RefList<{t}>();\n"
            )
        })
        .collect();
    let body = format!(
        "[DlrlObject]\npublic partial class {cls} : ObjectRoot\n{{\n{key_attrs}{rel_attrs}}}\n"
    );
    if ns.is_empty() {
        body
    } else {
        format!("namespace {ns}\n{{\n{body}}}\n")
    }
}

/// Additionally generates a separate object class (complete, not
/// `partial`) — for callers that do not need the type as a user anchor.
#[allow(clippy::format_collect)]
#[must_use]
pub fn generate_csharp_object(info: &DlrlTypeInfo) -> String {
    let cls = simple_name(&info.name);
    let key_lines: String = info
        .keys
        .iter()
        .map(|k| format!("    public long {k};\n"))
        .collect();
    format!(
        "// Generated DLRL Object — Spec §B.6\n\
[DlrlObject]\npublic class {cls} : ObjectRoot\n{{\n{key_lines}}}\n"
    )
}

fn simple_name(scoped: &str) -> &str {
    scoped.rsplit("::").next().unwrap_or(scoped)
}

fn namespace(scoped: &str) -> String {
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
    fn csharp_partial_emits_namespace() {
        let s = generate_csharp_partial(&trade_info());
        assert!(s.contains("namespace demo"));
        assert!(s.contains("[DlrlObject]"));
        assert!(s.contains("partial class Trade : ObjectRoot"));
        assert!(s.contains("[DlrlKey] public long symbol"));
        assert!(s.contains("[DlrlRelation(typeof(Quote))]"));
        assert!(s.contains("RefList<Quote> quotes"));
    }

    #[test]
    fn csharp_partial_no_namespace_for_unscoped() {
        let info = DlrlTypeInfo {
            name: "Foo".into(),
            ..DlrlTypeInfo::default()
        };
        let s = generate_csharp_partial(&info);
        assert!(!s.contains("namespace"));
        assert!(s.contains("class Foo : ObjectRoot"));
    }

    #[test]
    fn csharp_object_emits_keys() {
        let s = generate_csharp_object(&trade_info());
        assert!(s.contains("public long symbol;"));
    }

    #[test]
    fn namespace_for_multi_level() {
        assert_eq!(namespace("a::b::C"), "a.b");
        assert_eq!(namespace("X"), "");
    }
}
