// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Persists a naming graph to a file so a restart keeps its bindings —
//! OMG Naming Service 1.3 leaves persistence to the implementation; this
//! is `zerodds-naming-svc`'s (`docs/specs/` — see this crate's README).
//!
//! ## Format
//!
//! One record per line, tab-separated, three kinds:
//!
//! ```text
//! ctx <id>
//! obj <parent_id>\t<name_id>\t<name_kind>\t<stringified_ior>
//! sub <parent_id>\t<name_id>\t<name_kind>\t<child_id>
//! ```
//!
//! `<id>`/`<parent_id>`/`<child_id>` are [`NamingContext::id`] values —
//! stable across a restart because loading reconstructs every context
//! with the id it was saved under ([`NamingContext::with_id`]), so an
//! IOR a client already holds (which embeds an object key derived from
//! this id — see the `servant` module, feature `serve`) keeps resolving.
//! The root is always id 0. `<name_id>`/`<name_kind>`/`<stringified_ior>`
//! are percent-escaped for `\t`, `\n` and `%` only — `Name`'s own
//! `/`/`.`/`\` escaping ([`crate::stringified`]) is for the *display*
//! grammar of a whole path and would have to un-escape and re-escape
//! every component for no benefit here, where each field is already on
//! its own line and never concatenated with another.
//!
//! `ctx` lines are written before any `obj`/`sub` line references that
//! id, and a whole graph — root plus every reachable sub-context — is
//! written on every save, so a load is one pass over `ctx` lines to
//! create every node and a second pass over `obj`/`sub` lines to bind
//! them; there is no forward-reference ordering to get wrong.
//!
//! ## Atomicity
//!
//! [`save`] writes the whole snapshot to a sibling temporary file, then
//! renames it over the destination (`tools/wireprobe/tests/
//! three_column_gate.rs`'s `write_atomically` pattern — POSIX rename is
//! atomic within one filesystem, so a reader — or a crash — never
//! observes a half-written file).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_corba_ior::{Ior, from_stringified};

use crate::context::{Entry, NamingContext, ObjectRef};
use crate::name::NameComponent;

/// Error while saving or loading a naming graph.
#[derive(Debug)]
pub enum PersistError {
    /// The file could not be read/written/renamed.
    Io(std::io::Error),
    /// A line did not match any of the three record kinds, or a field
    /// was malformed (a non-numeric id, an odd `%` escape, a stringified
    /// IOR that does not parse).
    Malformed {
        /// 1-based line number.
        line: usize,
        /// What was wrong.
        reason: String,
    },
    /// An `obj`/`sub` line named a `parent_id`/`child_id` no `ctx` line
    /// declared.
    UnknownContextId(u64),
}

impl core::fmt::Display for PersistError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Malformed { line, reason } => write!(f, "line {line}: {reason}"),
            Self::UnknownContextId(id) => write!(f, "no `ctx {id}` line for this reference"),
        }
    }
}

impl std::error::Error for PersistError {}

impl From<std::io::Error> for PersistError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'\t' | b'\n' | b'%' => {
                out.push('%');
                out.push_str(&alloc::format!("{b:02X}"));
            }
            _ => out.push(b as char),
        }
    }
    out
}

fn pct_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err("truncated %-escape".into());
            }
            let hex = core::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| "bad %-escape")?;
            let v = u8::from_str_radix(hex, 16).map_err(|_| "bad %-escape hex")?;
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "escaped field is not UTF-8".into())
}

/// Walks `root` and every context reachable from it and renders the
/// snapshot text (`save`'s payload, factored out so a caller can inspect
/// it without touching the filesystem — used by tests).
#[must_use]
pub fn render(root: &Arc<NamingContext>) -> String {
    let mut ctx_lines = Vec::new();
    let mut bind_lines = Vec::new();
    let mut seen: BTreeMap<u64, ()> = BTreeMap::new();
    let mut stack = alloc::vec![Arc::clone(root)];
    while let Some(ctx) = stack.pop() {
        if seen.insert(ctx.id(), ()).is_some() {
            continue;
        }
        ctx_lines.push(alloc::format!("ctx\t{}", ctx.id()));
        for (name, entry) in ctx.raw_entries() {
            let id_f = pct_encode(&name.id);
            let kind_f = pct_encode(&name.kind);
            match entry {
                Entry::Object(obj) => {
                    bind_lines.push(alloc::format!(
                        "obj\t{}\t{id_f}\t{kind_f}\t{}",
                        ctx.id(),
                        pct_encode(&obj.stringified)
                    ));
                }
                Entry::Context(child) => {
                    bind_lines.push(alloc::format!(
                        "sub\t{}\t{id_f}\t{kind_f}\t{}",
                        ctx.id(),
                        child.id()
                    ));
                    stack.push(child);
                }
            }
        }
    }
    let mut out = String::from("# zerodds-naming-svc persisted graph v1\n");
    for l in ctx_lines {
        out.push_str(&l);
        out.push('\n');
    }
    for l in bind_lines {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// Renders and atomically writes the snapshot: the full text to a
/// sibling `<path>.tmp.<pid>`, then `rename` over `path`.
///
/// # Errors
/// [`PersistError::Io`] on a write or rename failure. Neither leaves a
/// partially written `path` — the temporary file is either renamed whole
/// or (on a write failure) removed and `path` is untouched.
pub fn save(root: &Arc<NamingContext>, path: &std::path::Path) -> Result<(), PersistError> {
    let text = render(root);
    let tmp = path.with_extension(alloc::format!(
        "tmp.{}",
        std::process::id().to_string() + "." + &unique_suffix()
    ));
    let write_result = std::fs::write(&tmp, text.as_bytes());
    match write_result {
        Ok(()) => {
            std::fs::rename(&tmp, path)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

/// A per-call disambiguator so two saves from the same process in the
/// same millisecond (a rapid bind/unbind pair under test) do not fight
/// over one temporary file name.
fn unique_suffix() -> String {
    use core::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed).to_string()
}

/// Loads a snapshot [`save`] wrote, reconstructing the graph with every
/// context's original id intact.
///
/// # Errors
/// [`PersistError::Io`] if `path` cannot be read (the caller decides
/// whether "does not exist yet" is an error — a fresh naming graph has
/// nothing to load); [`PersistError::Malformed`]/`UnknownContextId` for
/// a corrupted or hand-edited file — refused loudly rather than starting
/// from a silently partial graph.
pub fn load(path: &std::path::Path) -> Result<Arc<NamingContext>, PersistError> {
    let text = std::fs::read_to_string(path)?;
    load_str(&text)
}

/// As [`load`], from an in-memory snapshot (what [`render`] produced) —
/// the filesystem-free half, for tests and for a caller that already has
/// the bytes.
///
/// # Errors
/// As [`load`].
pub fn load_str(text: &str) -> Result<Arc<NamingContext>, PersistError> {
    let mut contexts: BTreeMap<u64, Arc<NamingContext>> = BTreeMap::new();
    let mut binds: Vec<(usize, Vec<&str>)> = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.first().copied() {
            Some("ctx") => {
                let id = parse_id(fields.get(1), line_no)?;
                contexts.insert(id, Arc::new(NamingContext::with_id(id)));
            }
            Some("obj") | Some("sub") => binds.push((line_no, fields)),
            _ => {
                return Err(PersistError::Malformed {
                    line: line_no,
                    reason: alloc::format!("unknown record kind: {line}"),
                });
            }
        }
    }

    // Root is always id 0 — a fresh graph (no `ctx 0` line, e.g. an
    // empty file) still gets one.
    let root = contexts
        .entry(0)
        .or_insert_with(|| Arc::new(NamingContext::with_id(0)))
        .clone();

    for (line_no, fields) in binds {
        let kind = fields[0];
        if !matches!(kind, "obj" | "sub") {
            return Err(PersistError::Malformed {
                line: line_no,
                reason: alloc::format!("unknown record kind: {kind}"),
            });
        }
        if fields.len() != 5 {
            return Err(PersistError::Malformed {
                line: line_no,
                reason: alloc::format!("{kind} needs 4 fields, got {}", fields.len() - 1),
            });
        }
        let parent_id = parse_id(Some(&fields[1]), line_no)?;
        let parent = contexts
            .get(&parent_id)
            .ok_or(PersistError::UnknownContextId(parent_id))?;
        let name_id = pct_decode(fields[2]).map_err(|reason| PersistError::Malformed {
            line: line_no,
            reason,
        })?;
        let name_kind = pct_decode(fields[3]).map_err(|reason| PersistError::Malformed {
            line: line_no,
            reason,
        })?;
        let name = NameComponent {
            id: name_id,
            kind: name_kind,
        };
        match kind {
            "obj" => {
                let stringified =
                    pct_decode(fields[4]).map_err(|reason| PersistError::Malformed {
                        line: line_no,
                        reason,
                    })?;
                // Parsed (and dropped) purely to refuse a corrupted file
                // loudly at load time rather than handing a client an
                // IOR that fails to decode on first use.
                let ior: Result<Ior, _> = from_stringified(&stringified);
                if ior.is_err() {
                    return Err(PersistError::Malformed {
                        line: line_no,
                        reason: alloc::format!("stringified IOR does not parse: {stringified}"),
                    });
                }
                parent.insert_raw(
                    name,
                    Entry::Object(ObjectRef::from_stringified(stringified)),
                );
            }
            "sub" => {
                let child_id = parse_id(Some(&fields[4]), line_no)?;
                let child = contexts
                    .get(&child_id)
                    .ok_or(PersistError::UnknownContextId(child_id))?;
                parent.insert_raw(name, Entry::Context(Arc::clone(child)));
            }
            other => {
                return Err(PersistError::Malformed {
                    line: line_no,
                    reason: alloc::format!("unknown record kind: {other}"),
                });
            }
        }
    }

    Ok(root)
}

fn parse_id(field: Option<&&str>, line_no: usize) -> Result<u64, PersistError> {
    field
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| PersistError::Malformed {
            line: line_no,
            reason: "expected a numeric context id".into(),
        })
}

/// Re-encodes a stringified IOR the same way [`ObjectRef::from_ior`]
/// would, for a caller that wants to double-check a loaded binding's IOR
/// decodes and re-encodes to the same bytes — used by the round-trip
/// test below; not needed by `save`/`load` themselves, which pass the
/// stringified text through unchanged.
#[cfg(test)]
#[allow(clippy::expect_used)]
fn reencode_stringified(s: &str) -> String {
    use zerodds_corba_ior::to_stringified;
    use zerodds_wire::Endianness;
    let ior: Ior = from_stringified(s).expect("test IOR");
    to_stringified(&ior, Endianness::Big).expect("test IOR")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::context::{BindingType, ResolveResult};
    use zerodds_corba_ior::to_stringified;
    use zerodds_wire::Endianness;

    fn nc(id: &str) -> NameComponent {
        NameComponent::new(id)
    }

    fn test_ior_string(key: &str) -> String {
        let ior = Ior {
            type_id: "IDL:omg.org/CosNaming/NamingContext:1.0".into(),
            profiles: Vec::new(),
        };
        let _ = key; // the object key would live in a real IIOP profile
        to_stringified(&ior, Endianness::Big).expect("stringify")
    }

    #[test]
    fn pct_escaping_round_trips_tab_newline_and_percent() {
        let s = "a\tb\nc%d";
        let enc = pct_encode(s);
        assert!(!enc.contains('\t') && !enc.contains('\n'));
        assert_eq!(pct_decode(&enc).unwrap(), s);
    }

    #[test]
    fn an_empty_graph_round_trips_to_an_empty_root() {
        let root = Arc::new(NamingContext::with_id(0));
        let text = render(&root);
        let loaded = load_str(&text).unwrap();
        assert_eq!(loaded.id(), 0);
        assert!(loaded.all_bindings().is_empty());
    }

    #[test]
    fn objects_and_sub_contexts_round_trip_with_their_ids() {
        let root = Arc::new(NamingContext::with_id(0));
        root.bind(
            &alloc::vec![nc("printer")],
            ObjectRef::from_stringified(test_ior_string("printer")),
        )
        .unwrap();
        let app = root.bind_new_context(&alloc::vec![nc("App")]).unwrap();
        app.bind(
            &alloc::vec![nc("Service")],
            ObjectRef::from_stringified(test_ior_string("service")),
        )
        .unwrap();
        let app_id = app.id();

        let text = render(&root);
        let loaded = load_str(&text).unwrap();

        match loaded.resolve(&alloc::vec![nc("printer")]).unwrap() {
            ResolveResult::Object(o) => {
                assert_eq!(o.stringified, test_ior_string("printer"));
            }
            _ => panic!("expected an object"),
        }
        match loaded.resolve(&alloc::vec![nc("App")]).unwrap() {
            ResolveResult::Context(c) => assert_eq!(c.id(), app_id),
            _ => panic!("expected a context"),
        }
        match loaded
            .resolve(&alloc::vec![nc("App"), nc("Service")])
            .unwrap()
        {
            ResolveResult::Object(o) => {
                assert_eq!(o.stringified, test_ior_string("service"));
            }
            _ => panic!("expected an object"),
        }
        let bindings = loaded.all_bindings();
        assert_eq!(bindings.len(), 2);
        assert!(
            bindings
                .iter()
                .any(|b| b.binding_name.id == "App" && b.binding_type == BindingType::Context)
        );
    }

    #[test]
    fn a_name_component_with_a_kind_round_trips() {
        let root = Arc::new(NamingContext::with_id(0));
        root.bind(
            &alloc::vec![NameComponent::with_kind("trader", "Server")],
            ObjectRef::from_stringified(test_ior_string("t")),
        )
        .unwrap();
        let loaded = load_str(&render(&root)).unwrap();
        let bindings = loaded.all_bindings();
        assert_eq!(bindings[0].binding_name.kind, "Server");
    }

    #[test]
    fn a_name_with_tabs_and_percent_round_trips() {
        let root = Arc::new(NamingContext::with_id(0));
        root.bind(
            &alloc::vec![nc("weird\tname%with\nchars")],
            ObjectRef::from_stringified(test_ior_string("w")),
        )
        .unwrap();
        let loaded = load_str(&render(&root)).unwrap();
        assert_eq!(
            loaded.all_bindings()[0].binding_name.id,
            "weird\tname%with\nchars"
        );
    }

    #[test]
    fn loading_reserves_ids_so_new_contexts_never_collide() {
        let root = Arc::new(NamingContext::with_id(0));
        let _far = root.bind_new_context(&alloc::vec![nc("a")]).unwrap();
        let child_of_far = {
            let far = match root.resolve(&alloc::vec![nc("a")]).unwrap() {
                ResolveResult::Context(c) => c,
                ResolveResult::Object(_) => panic!("expected a context"),
            };
            far.bind_new_context(&alloc::vec![nc("b")]).unwrap()
        };
        let highest_saved_id = child_of_far.id();

        let loaded = load_str(&render(&root)).unwrap();
        let fresh = loaded.new_context();
        assert!(
            fresh.id() > highest_saved_id,
            "a context created after load must not collide with a loaded id"
        );
    }

    #[test]
    fn a_malformed_line_is_refused_loudly() {
        let err = load_str("ctx\t0\nobj\t0\tx\n").unwrap_err();
        assert!(matches!(err, PersistError::Malformed { .. }));
    }

    #[test]
    fn a_sub_line_naming_an_unknown_context_is_refused() {
        let err = load_str("ctx\t0\nsub\t0\tApp\t\t999\n").unwrap_err();
        assert!(matches!(err, PersistError::UnknownContextId(999)));
    }

    #[test]
    fn save_and_load_round_trip_through_an_atomic_file_write() {
        let dir = std::env::temp_dir().join(alloc::format!(
            "zd-naming-persist-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("graph.txt");

        let root = Arc::new(NamingContext::with_id(0));
        root.bind(
            &alloc::vec![nc("x")],
            ObjectRef::from_stringified(test_ior_string("x")),
        )
        .unwrap();
        save(&root, &path).unwrap();
        assert!(path.exists());
        // No leftover temporary file after a successful save.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp file(s) left behind: {leftovers:?}"
        );

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.all_bindings().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reencode_helper_is_stable() {
        let s = test_ior_string("x");
        assert_eq!(reencode_stringified(&s), s);
    }
}
