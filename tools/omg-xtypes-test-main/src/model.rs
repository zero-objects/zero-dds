// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The type this run publishes or subscribes, read from the file the
//! suite names on the command line.
//!
//! This is the point of the v2 suite. `dds-xtypes` on its
//! `feature/xml-driver-enhanced` branch does not ship 25 compiled-in
//! types the way its `master` branch did: it ships ten type-definition
//! files with 128 types between them, names one file and one type per
//! run (`--type-folder types --type-file extensibility -y
//! Test::struct_f1`), and pairs two runs of two different types. Types
//! collide across the files on purpose — `Test::struct_x1` is one struct
//! in `type_consistency` and a different one in
//! `type_consistency_force_type_val`, which is what that pair of files
//! is for — so no single compiled-in namespace can hold them, and the
//! type has to be read when the run starts.
//!
//! The path the suite's own C and C++ clients build is
//! `<type-folder>/xml/<type-file>.xml`; the same directory carries
//! `<type-folder>/idl/<type-file>.idl`, which the suite's README calls
//! "the equivalent". This client reads the **XML**, and falls back to
//! the IDL only where no XML file exists.
//!
//! It used to be the other way round, on the argument that the IDL front
//! end is the one this project measures everywhere else. Three facts
//! overturned that.
//!
//! * The XML is what the suite's own clients read, so it is the
//!   declaration under test; the IDL beside it is a second copy that no
//!   client of the suite translates.
//! * Four of the ten IDL files are not valid IDL 4.2 and no compiler
//!   accepts them. `arrays.idl`, `sequences.idl` and `struct_names.idl`
//!   each declare two enumerations in module `Test` with the
//!   enumerators `VAL0`, `VAL1` and `VAL2` — §7.4.15 places enumerators
//!   in the *enclosing* scope, so the second declaration collides with
//!   the first — and `unions.idl` declares a member `DeFault`, which
//!   §7.2.3 forbids because it differs from the keyword `default` only
//!   in case. The XML has neither problem: an XML enumerator is scoped
//!   to its `<enum>`, and `<member name="DeFault">` is an attribute
//!   value and not an identifier in a grammar with keywords.
//! * `try_construct`, `unions_bitmask` and `unions_key_discriminator`
//!   exist **only** as XML.
//!
//! The front end is `zerodds-xml-compose`, which lowers DDS-XML into the
//! same `Ast` that `zerodds-idl-compose` lowers IDL into, so everything
//! after this file — `zerodds_types::from_ast`, the layout planner, the
//! codec — is unchanged and does not know which file was read. Which one
//! it was is printed, so a reviewer can see it.

use std::path::{Path, PathBuf};

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_types::{TypeIdentifier, type_object::TypeObject};
use zerodds_xml_compose::KeyOnUnionDiscriminator;

/// Everything one run needs to know about its type.
#[allow(clippy::missing_fields_in_debug)]
pub struct Model {
    /// The model the type was read from. Also what encodes and decodes
    /// the sample, through `zerodds_wire_codec`.
    pub ast: Ast,
    /// The type named by `-y`.
    pub type_: Id<TypeNode>,
    /// `-y` verbatim — what goes on the wire as the topic type name.
    pub type_name: String,
    /// `@final` / `@appendable` / `@mutable`, as the model resolves it.
    pub extensibility: zerodds_ast::Extensibility,
    /// Whether any member of the type carries `@key` (XTypes 1.3
    /// §7.6.8).
    pub keyed: bool,
    /// The `CompleteTypeObject` of the type.
    pub type_object: TypeObject,
    /// Its `TypeIdentifier` — the hash of the object above.
    pub type_identifier: TypeIdentifier,
    /// Every other `TypeObject` the model produced, to be registered so
    /// a peer can resolve the members this one names by identifier
    /// (XTypes 1.3 §7.3.4.6).
    pub dependencies: Vec<TypeObject>,
    /// The file the type came from, for the run's own log line.
    pub source_path: PathBuf,
}

/// Everything reading a type can be wrong about.
#[derive(Debug)]
pub enum ModelError {
    /// Neither the IDL nor the XML file exists.
    NoSuchFile {
        /// The paths that were tried.
        tried: Vec<PathBuf>,
    },
    /// The file exists and could not be read.
    Unreadable {
        /// The file.
        path: PathBuf,
        /// The OS error.
        cause: String,
    },
    /// The front end reported diagnostics.
    Diagnostics {
        /// The file.
        path: PathBuf,
        /// Rendered diagnostics.
        detail: String,
    },
    /// `-y` names a type the file does not declare.
    NoSuchType {
        /// The scoped name from `-y`.
        name: String,
        /// The file.
        path: PathBuf,
    },
    /// The type has no `TypeObject` — a construct the model carries and
    /// XTypes 1.3 §7.3.4 has no form for.
    NoTypeObject {
        /// The scoped name.
        name: String,
        /// Why.
        detail: String,
    },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchFile { tried } => {
                write!(f, "no type definition file; tried")?;
                for p in tried {
                    write!(f, " {}", p.display())?;
                }
                Ok(())
            }
            Self::Unreadable { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            Self::Diagnostics { path, detail } => {
                write!(f, "{} does not compile: {detail}", path.display())
            }
            Self::NoSuchType { name, path } => {
                write!(f, "{} declares no type '{name}'", path.display())
            }
            Self::NoTypeObject { name, detail } => {
                write!(f, "no TypeObject for '{name}': {detail}")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// The two paths the suite's layout puts a type file at, in the order
/// they are tried.
#[must_use]
pub fn candidate_paths(folder: Option<&str>, file: Option<&str>) -> Vec<PathBuf> {
    // `--type-file` alone means a path that already carries its
    // directory; `--type-folder` alone is not something the suite emits,
    // but its own `validate()` accepts either, so both are handled.
    //
    // The XML comes first because it is what the suite's own clients
    // read — see the module documentation for the two other reasons.
    let stem = file.unwrap_or("");
    match folder {
        Some(dir) => vec![
            Path::new(dir).join("xml").join(format!("{stem}.xml")),
            Path::new(dir).join("idl").join(format!("{stem}.idl")),
            Path::new(dir).join(format!("{stem}.xml")),
            Path::new(dir).join(format!("{stem}.idl")),
        ],
        None => vec![
            PathBuf::from(format!("{stem}.xml")),
            PathBuf::from(format!("{stem}.idl")),
            PathBuf::from(stem),
        ],
    }
}

/// The model one type file produces, whichever of the two languages it
/// is written in.
///
/// The extension decides, and nothing else: a file named `.xml` that
/// holds IDL fails as XML rather than being retried as IDL, because a
/// front end that guesses would report the second language's diagnostics
/// for a defect in the first.
fn compose(
    path: &Path,
    source: &str,
    key_on_union_discriminator: KeyOnUnionDiscriminator,
) -> Result<(Ast, String), String> {
    let name = path.to_string_lossy().into_owned();
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
    {
        let composed = zerodds_xml_compose::compose_source_with(
            &name,
            source,
            zerodds_xml_compose::ComposeOptions {
                key_on_union_discriminator,
                zero_bound_is_unbounded: true,
                ..zerodds_xml_compose::ComposeOptions::default()
            },
        );
        if composed.failed() {
            let detail = composed
                .diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(detail);
        }
        return Ok((composed.ast, name));
    }
    let composed = zerodds_idl_compose::compose_source_with(
        &name,
        source,
        // The suite's files `#include` nothing, so the bare loader is
        // the whole of the include path. A file that did include
        // something would fail here by name rather than silently drop
        // the declarations it could not find.
        &zerodds_idl_compose::PathLoader::bare(),
        zerodds_idl_compose::ComposeOptions {
            key_on_union_discriminator,
            ..zerodds_idl_compose::ComposeOptions::default()
        },
    );
    if !composed.diagnostics.is_empty() {
        return Err(format!("{:?}", composed.diagnostics));
    }
    Ok((composed.ast, name))
}

/// Reads the type named by `type_name` out of the file the two options
/// point at.
///
/// # Errors
/// [`ModelError`].
pub fn load(
    folder: Option<&str>,
    file: Option<&str>,
    type_name: &str,
    key_on_union_discriminator: KeyOnUnionDiscriminator,
) -> Result<Model, ModelError> {
    let tried = candidate_paths(folder, file);
    let path =
        tried
            .iter()
            .find(|p| p.is_file())
            .cloned()
            .ok_or_else(|| ModelError::NoSuchFile {
                tried: tried.clone(),
            })?;
    let source = std::fs::read_to_string(&path).map_err(|e| ModelError::Unreadable {
        path: path.clone(),
        cause: e.to_string(),
    })?;
    let ast = match compose(&path, &source, key_on_union_discriminator) {
        Ok((ast, _)) => ast,
        Err(detail) => return Err(ModelError::Diagnostics { path, detail }),
    };
    let type_ = find_type(&ast, type_name).ok_or_else(|| ModelError::NoSuchType {
        name: type_name.to_owned(),
        path: path.clone(),
    })?;

    let complete = zerodds_types::from_ast::complete_type_object(&ast, type_).map_err(|e| {
        ModelError::NoTypeObject {
            name: type_name.to_owned(),
            detail: format!("{e:?}"),
        }
    })?;
    let type_object = TypeObject::Complete(complete);
    let type_identifier = zerodds_types::from_ast::type_identifier(
        &ast,
        type_,
        zerodds_types::from_ast::Form::Complete,
    )
    .map_err(|e| ModelError::NoTypeObject {
        name: type_name.to_owned(),
        detail: format!("{e:?}"),
    })?;

    // Every other type the file declares. A peer resolving this
    // aggregate's members by identifier needs them registered too — see
    // `DomainParticipant::register_type_object`. The whole file rather
    // than the reachable set: the files are small (128 types across the
    // suite's ten), and a reachability walk that missed one would show
    // up as a match that silently fell back to the type name.
    let others: Vec<Id<TypeNode>> = ast
        .types()
        .map(|(id, _)| id)
        .filter(|id| *id != type_)
        .collect();
    let mut dependencies = Vec::new();
    for other in others {
        if let Ok(obj) = zerodds_types::from_ast::complete_type_object(&ast, other) {
            dependencies.push(TypeObject::Complete(obj));
        }
    }

    let extensibility = ast.effective_extensibility(type_);
    // Both positions Table 21 gives `@key`: a `@key` member, and a union
    // discriminator. A union keyed by its discriminator owns no keyed
    // member at all, so a walk over members alone reports it keyless and
    // the endpoint announces no KeyHash for a keyed topic.
    let keyed = zerodds_ast_layout::union_key_discriminator(&ast, type_).is_some()
        || ast.all_members_of(type_).into_iter().any(|m| {
            ast.member(m)
                .ok()
                .and_then(|node| node.facets.xtypes.as_ref())
                .is_some_and(|x| x.is_key)
        });

    Ok(Model {
        ast,
        type_,
        type_name: type_name.to_owned(),
        extensibility,
        keyed,
        type_object,
        type_identifier,
        dependencies,
        source_path: path,
    })
}

/// `Test::struct_f1` — an absolute scoped name whichever way the suite
/// spells it.
fn find_type(ast: &Ast, name: &str) -> Option<Id<TypeNode>> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// The path the suite's own C client builds is
    /// `<folder>/xml/<file>.xml` (`src/c/test_main.c`, the
    /// `sprintf(types_uri, "%s%s%s%s", type_folder, "/xml/", type_file,
    /// ".xml")`), and that is the one this client reads first. The IDL
    /// beside it stays a candidate, so a folder that ships only the IDL
    /// is still found.
    #[test]
    fn the_xml_the_suites_own_clients_read_is_tried_first() {
        let c = candidate_paths(Some("types"), Some("extensibility"));
        assert_eq!(c[0], PathBuf::from("types/xml/extensibility.xml"));
        assert_eq!(c[1], PathBuf::from("types/idl/extensibility.idl"));
    }

    /// A missing file names every path it tried, so a run started from
    /// the wrong directory says which directory it wanted.
    #[test]
    fn a_missing_type_file_names_what_it_tried() {
        match load(
            Some("no-such-folder"),
            Some("nothing"),
            "Test::x",
            KeyOnUnionDiscriminator::default(),
        ) {
            Err(ModelError::NoSuchFile { tried }) => assert_eq!(tried.len(), 4),
            Err(other) => panic!("expected NoSuchFile, got {other}"),
            Ok(_) => panic!("expected NoSuchFile, got a model"),
        }
    }
}
