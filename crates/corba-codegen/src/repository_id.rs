// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Repository identifiers. CORBA 3.3 part 1 §10.7.
//!
//! The default form is `IDL:<scoped name with `/` separators>:<major>.<minor>`,
//! and three preprocessor directives change it: `#pragma prefix` puts a
//! vendor prefix in front of the path, `#pragma version` replaces the
//! version, and `#pragma ID` sets the whole identifier outright.
//!
//! ## The model already answers this, and that is the point
//!
//! `zerodds-idl-lower` applies §10.7 while lowering — prefix, version
//! and `ID` included — and writes the result into
//! [`CorbaFacet::repository_id`]. So a generator must **read** the
//! identifier and not re-derive it: a second derivation here would see
//! none of the three directives and would quietly disagree with the
//! model for every file that used one.
//!
//! [`repository_id_of`] is that reader. [`build_repository_id`] is the
//! fallback for a caller holding a name and no model, and it says so.
//!
//! [`CorbaFacet::repository_id`]: zerodds_ast::CorbaFacet::repository_id

use alloc::format;
use alloc::string::{String, ToString};

use zerodds_ast::{AnyId, Ast, RepositoryIdOrigin};

/// Where an identifier came from, so a caller can tell a derived one
/// from a declared one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepositoryId {
    /// The identifier, as it goes on the wire.
    pub id: String,
    /// How it was arrived at, when the model recorded that.
    ///
    /// `None` means the model carried no identifier for this node and
    /// [`repository_id_of`] derived one from the scoped name — which is
    /// the default rule and nothing else, so a file using
    /// `#pragma prefix` on a node the model skipped would be wrong here
    /// too. See the crate documentation for which node kinds the model
    /// covers.
    pub origin: Option<RepositoryIdOrigin>,
}

/// The repository identifier of a declaration.
///
/// Reads [`CorbaFacet::repository_id`] first, which is the model's
/// answer and carries `#pragma prefix`, `#pragma version` and
/// `#pragma ID`. Falls back to the §10.7 default form built from the
/// scoped name, and records in [`ResolvedRepositoryId::origin`] which of
/// the two happened.
///
/// Returns `None` only when the handle is stale or the node has no
/// scoped name at all.
///
/// [`CorbaFacet::repository_id`]: zerodds_ast::CorbaFacet::repository_id
#[must_use]
pub fn repository_id_of(ast: &Ast, any: AnyId) -> Option<ResolvedRepositoryId> {
    if let Some(corba) = ast.facets(any).and_then(|facets| facets.corba.as_ref())
        && let Some(declared) = corba.repository_id.as_ref()
    {
        return Some(ResolvedRepositoryId {
            id: declared.0.clone(),
            origin: corba.repository_id_origin,
        });
    }
    let scoped = ast.scoped_name_of(any)?;
    let segments = scoped.segments();
    let (last, modules) = segments.split_last()?;
    let modules: alloc::vec::Vec<&str> = modules.iter().map(String::as_str).collect();
    Some(ResolvedRepositoryId {
        id: build_repository_id(&modules, last, 1, 0),
        origin: None,
    })
}

/// Build a repository identifier from a module path, a name and a
/// version.
///
/// The §10.7 **default** form and nothing more: no prefix, no `#pragma
/// version`, no `#pragma ID`. A caller that has a model should ask
/// [`repository_id_of`], which sees all three; this is for a caller that
/// has only a name — a hand-written IDL-free stub, a test, a diagnostic.
///
/// ```
/// use zerodds_corba_codegen::build_repository_id;
///
/// assert_eq!(
///     build_repository_id(&["MyApp", "Trading"], "Order", 1, 0),
///     "IDL:MyApp/Trading/Order:1.0"
/// );
/// assert_eq!(build_repository_id(&[], "Object", 1, 0), "IDL:Object:1.0");
/// ```
#[must_use]
pub fn build_repository_id(modules: &[&str], type_name: &str, major: u16, minor: u16) -> String {
    let mut path = String::new();
    for module in modules {
        // A caller that assembled its module path by hand can hand over
        // an empty segment; writing it would produce `IDL:A//B:1.0`,
        // which is a different identifier and no ORB's.
        if module.is_empty() {
            continue;
        }
        path.push_str(module);
        path.push('/');
    }
    path.push_str(type_name);
    format!("IDL:{path}:{major}.{minor}")
}

/// Build a repository identifier with a `#pragma prefix` in front of it.
///
/// §10.7.2: the prefix goes between `IDL:` and the scoped path, with a
/// `/` after it. Separate from [`build_repository_id`] rather than an
/// `Option` parameter, because an empty prefix and no prefix produce the
/// same identifier and a caller passing `Some("")` should not have to
/// know that.
#[must_use]
pub fn build_repository_id_with_prefix(
    prefix: &str,
    modules: &[&str],
    type_name: &str,
    major: u16,
    minor: u16,
) -> String {
    let plain = build_repository_id(modules, type_name, major, minor);
    if prefix.is_empty() {
        return plain;
    }
    format!("IDL:{prefix}/{}", plain.trim_start_matches("IDL:"))
}

/// Split a repository identifier back into prefix-and-path, and version.
///
/// A reader of a GIOP request holds the identifier and needs the version
/// out of it. Returns `None` for anything that is not `IDL:<path>:<v>`,
/// which includes the `RMI:` and `DCE:` formats §10.7 also defines —
/// they are not this one and are not silently read as it.
#[must_use]
pub fn split_repository_id(id: &str) -> Option<(&str, &str)> {
    let body = id.strip_prefix("IDL:")?;
    let (path, version) = body.rsplit_once(':')?;
    if path.is_empty() || version.is_empty() {
        return None;
    }
    // A version is `<major>.<minor>` and both are digits; `IDL:A:B:1.0`
    // has a colon in its path and must still split at the last one.
    let (major, minor) = version.split_once('.')?;
    let numeric = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    (numeric(major) && numeric(minor)).then_some((path, version))
}

/// The scoped name a repository identifier's path denotes.
///
/// `IDL:MyApp/Trading/Order:1.0` gives `MyApp::Trading::Order`. A prefix
/// cannot be told from a module here — §10.7 gives the identifier no
/// marker for where one ends — so the whole path is returned and a
/// caller that knows its prefix strips it.
#[must_use]
pub fn scoped_name_from_repository_id(id: &str) -> Option<String> {
    let (path, _) = split_repository_id(id)?;
    Some(
        path.split('/')
            .collect::<alloc::vec::Vec<_>>()
            .join("::")
            .to_string(),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_module_path() {
        assert_eq!(
            build_repository_id(&["demo"], "Echo", 1, 0),
            "IDL:demo/Echo:1.0"
        );
    }

    #[test]
    fn a_nested_module_path() {
        assert_eq!(
            build_repository_id(&["MyApp", "Trading"], "Order", 1, 0),
            "IDL:MyApp/Trading/Order:1.0"
        );
    }

    #[test]
    fn no_modules_at_all() {
        assert_eq!(build_repository_id(&[], "Object", 1, 0), "IDL:Object:1.0");
    }

    #[test]
    fn the_version_is_written_as_given() {
        assert_eq!(build_repository_id(&["x"], "Y", 2, 5), "IDL:x/Y:2.5");
    }

    #[test]
    fn an_empty_module_segment_is_dropped_rather_than_doubling_the_separator() {
        assert_eq!(
            build_repository_id(&["", "demo", ""], "Echo", 1, 0),
            "IDL:demo/Echo:1.0"
        );
    }

    #[test]
    fn a_prefix_goes_between_the_scheme_and_the_path() {
        assert_eq!(
            build_repository_id_with_prefix("omg.org", &["CosNaming"], "NamingContext", 1, 0),
            "IDL:omg.org/CosNaming/NamingContext:1.0"
        );
        assert_eq!(
            build_repository_id_with_prefix("", &["demo"], "Echo", 1, 0),
            "IDL:demo/Echo:1.0"
        );
    }

    #[test]
    fn splitting_takes_the_last_colon_and_checks_the_version_is_a_version() {
        assert_eq!(
            split_repository_id("IDL:MyApp/Trading/Order:1.0"),
            Some(("MyApp/Trading/Order", "1.0"))
        );
        // The `RMI:` and `DCE:` formats are not this one.
        assert_eq!(split_repository_id("RMI:java.lang.String:0000"), None);
        // A path containing a colon still splits at the last one.
        assert_eq!(split_repository_id("IDL:a:b:1.0"), Some(("a:b", "1.0")));
        // Not a version.
        assert_eq!(split_repository_id("IDL:a:one.zero"), None);
        assert_eq!(split_repository_id("IDL:a"), None);
    }

    #[test]
    fn a_path_reads_back_as_a_scoped_name() {
        assert_eq!(
            scoped_name_from_repository_id("IDL:MyApp/Trading/Order:1.0").as_deref(),
            Some("MyApp::Trading::Order")
        );
        assert_eq!(
            scoped_name_from_repository_id("IDL:Object:1.0").as_deref(),
            Some("Object")
        );
    }
}
