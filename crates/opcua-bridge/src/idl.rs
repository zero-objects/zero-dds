// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Compiles an operator-supplied `.idl` file into a model this daemon can
//! resolve DDS types from, and builds the
//! [`zerodds-dcps::serialized_type::SerializedTypeSupport`] a topic named
//! in the config needs to decode/encode its samples without a compiled
//! Rust type.

use std::path::Path;
use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_idl_compose::PathLoader;
use zerodds_types::TypeObject;
use zerodds_types::dynamic::DynamicType;
use zerodds_types::from_ast::complete_type_object;

/// An IDL file failed to compile, or a config-named type is not in it.
#[derive(Debug, Clone)]
pub enum IdlError {
    /// `zerodds-idl-compose` reported one or more diagnostics of
    /// [`zerodds_idl_parse::Severity::Error`].
    Compose {
        /// The file that failed.
        path: String,
        /// The diagnostics, rendered.
        diagnostics: Vec<String>,
    },
    /// The named type does not exist in the composed model, or exists but
    /// is not something a topic can be typed with (a module, a constant).
    NoSuchType {
        /// The type name that was looked up.
        name: String,
    },
    /// A type object could not be built for the model, or the resulting
    /// bytes did not validate against `keyed` (see
    /// [`SerializedTypeSupport::from_type_object`]).
    TypeObject {
        /// The type name.
        name: String,
        /// The underlying error, rendered.
        message: String,
    },
}

impl core::fmt::Display for IdlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Compose { path, diagnostics } => {
                write!(f, "{path}: {}", diagnostics.join("; "))
            }
            Self::NoSuchType { name } => write!(f, "no such type: `{name}`"),
            Self::TypeObject { name, message } => write!(f, "`{name}`: {message}"),
        }
    }
}

impl std::error::Error for IdlError {}

/// Compiles `path` (and whatever it `#include`s, resolved against its own
/// directory and `search_roots`) into a model.
///
/// # Errors
/// [`IdlError::Compose`] naming every error-severity diagnostic.
pub fn compile(path: &Path, search_roots: &[std::path::PathBuf]) -> Result<Arc<Ast>, IdlError> {
    let loader = PathLoader::new(search_roots.to_vec());
    let path_str = path.to_string_lossy().into_owned();
    let composed =
        zerodds_idl_compose::compose(&path_str, &loader).map_err(|e| IdlError::Compose {
            path: path_str.clone(),
            diagnostics: vec![e],
        })?;
    if composed.failed() {
        return Err(IdlError::Compose {
            path: path_str,
            diagnostics: composed
                .diagnostics
                .iter()
                .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
                .map(|d| format!("{d}"))
                .collect(),
        });
    }
    Ok(Arc::new(composed.ast))
}

/// Compiles every file in `paths` into one model — later files see earlier
/// ones' declarations, the same include-search order `search_roots` gives
/// each. Used when a gateway config's referenced types are spread across
/// more than one `.idl` file.
///
/// # Errors
/// [`IdlError::Compose`] for the first file that fails.
pub fn compile_all(
    paths: &[std::path::PathBuf],
    search_roots: &[std::path::PathBuf],
) -> Result<Vec<Arc<Ast>>, IdlError> {
    paths.iter().map(|p| compile(p, search_roots)).collect()
}

/// Finds `type_name` across `models`, in order — the first model that
/// declares it wins.
///
/// # Errors
/// [`IdlError::NoSuchType`] if no model declares it.
pub fn find_type(models: &[Arc<Ast>], type_name: &str) -> Result<DynamicType, IdlError> {
    for ast in models {
        if let Ok(dt) = DynamicType::find(ast, type_name) {
            return Ok(dt);
        }
    }
    Err(IdlError::NoSuchType {
        name: type_name.to_string(),
    })
}

/// Builds the [`SerializedTypeSupport`] for `type_name`, resolved from
/// `models` — the reflective decoder/encoder this daemon reads/writes every
/// DDS sample of that topic through (spec item: "types via TypeObject/
/// TypeLookup → `SerializedTypeSupport`").
///
/// `keyed`/`extensibility` are read off the model itself (like
/// [`SerializedTypeSupport::from_complete_type_objects`]'s registry path) —
/// there is nothing external here to disagree with them, unlike the
/// config-supplied-bytes path `zerodds-xrce-agent`'s daemon uses.
///
/// # Errors
/// [`IdlError::NoSuchType`] / [`IdlError::TypeObject`].
pub fn type_support_for(
    models: &[Arc<Ast>],
    type_name: &str,
) -> Result<Arc<SerializedTypeSupport>, IdlError> {
    let dt = find_type(models, type_name)?;
    let complete = complete_type_object(dt.ast(), dt.id()).map_err(|e| IdlError::TypeObject {
        name: type_name.to_string(),
        message: format!("{e:?}"),
    })?;
    let bytes = TypeObject::Complete(complete)
        .to_bytes_le()
        .map_err(|e| IdlError::TypeObject {
            name: type_name.to_string(),
            message: format!("{e:?}"),
        })?;
    let keyed = zerodds_ast_layout::keyhash::holder_keeps_only_keys(dt.ast(), dt.id())
        || zerodds_ast_layout::union_key_discriminator(dt.ast(), dt.id()).is_some();
    // `from_type_object` asserts a caller-supplied `extensibility` against
    // the bytes; we already read it off the model, so the two tries below
    // are not a guess — `dt.extensibility()` names one of Final/Appendable/
    // Mutable and `from_type_object`'s own encoding is 0/1/2 in that order.
    let extensibility = match dt.extensibility() {
        Some(zerodds_types::dynamic::ExtensibilityKind::Final) => 0,
        Some(zerodds_types::dynamic::ExtensibilityKind::Appendable) => 1,
        Some(zerodds_types::dynamic::ExtensibilityKind::Mutable) => 2,
        None => {
            return Err(IdlError::TypeObject {
                name: type_name.to_string(),
                message: "topic root must be a struct or a union (no extensibility)".to_string(),
            });
        }
    };
    SerializedTypeSupport::from_type_object(type_name, &bytes, keyed, extensibility).map_err(|e| {
        IdlError::TypeObject {
            name: type_name.to_string(),
            message: e,
        }
    })
}

/// Whether `ty` has one or more `@key` fields — the same test
/// [`type_support_for`] already runs to build `SerializedTypeSupport`,
/// exposed so a caller (`crate::daemon::server`) that already resolved a
/// `DynamicType` (for [`crate::dynamic_type::to_dds_type`]) does not need
/// to re-derive it from the type support alone.
#[must_use]
pub fn is_keyed(ty: &DynamicType) -> bool {
    zerodds_ast_layout::keyhash::holder_keeps_only_keys(ty.ast(), ty.id())
        || zerodds_ast_layout::union_key_discriminator(ty.ast(), ty.id()).is_some()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn write_idl(dir: &Path, name: &str, source: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, source).expect("write idl");
        path
    }

    #[test]
    fn compiles_a_struct_and_resolves_its_type_support() {
        let dir = std::env::temp_dir().join(format!("zd-opcua-idl-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(
            &dir,
            "point.idl",
            "@final struct Point { @key long id; long value; };",
        );
        let ast = compile(&path, &[]).expect("compile");
        let support = type_support_for(&[ast], "Point").expect("type support");
        assert!(support.is_keyed());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_type_is_reported_by_name() {
        let dir = std::env::temp_dir().join(format!("zd-opcua-idl-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(&dir, "empty.idl", "struct A { long x; };");
        let ast = compile(&path, &[]).expect("compile");
        let err = type_support_for(&[ast], "NoSuchType").expect_err("missing type");
        assert!(matches!(err, IdlError::NoSuchType { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_idl_is_reported_with_diagnostics() {
        let dir = std::env::temp_dir().join(format!("zd-opcua-idl-test3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(&dir, "broken.idl", "struct A { long x");
        let result = compile(&path, &[]);
        let Err(IdlError::Compose { diagnostics, .. }) = result else {
            panic!("expected a Compose error");
        };
        assert!(!diagnostics.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
