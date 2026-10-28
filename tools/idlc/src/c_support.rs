// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C column's DDS TypeSupport: one `zerodds_typesupport_t` per topic
//! type, the table `zerodds_writer_write_typed` and
//! `zerodds_reader_take_typed` call through (`zerodds_xcdr2.h`).
//!
//! A file of its own, `<stem>_typesupport.h`, beside the codec header:
//! the codec header needs `runtime/c` and nothing else, and a program
//! that only encodes keeps that. This one needs the C API's headers too.
//! It includes the codec header, which has no include guard, so a
//! translation unit includes the one or the other.
//!
//! # The version it writes, and the ones it reads
//!
//! The C API's writer stamps XCDR2 on what it is handed — its offer puts
//! XCDR2 first (`ffi_data_representation_offer`) — so the table encodes
//! XCDR2 whatever `--encoding` the codec header was generated for. It
//! reads both versions in both byte orders, taking them from the
//! `representation` byte the take path hands over (`ZERODDS_TS_REPR_*`).
//! For each it carries its own codec: the same generator's output under
//! the same dialect with only the version changed, every function
//! renamed — `zd_x2_*`, `zd_x1_*`, and `zd_kh_*` for the key holder — so
//! nothing here answers a wire question a second time and no name
//! collides with the codec header's.
//!
//! # Memory
//!
//! `runtime/c` decodes into storage the caller hands it
//! (`zdw_reader_set_heap`) and never calls `malloc`. The decoder here
//! gives it one allocation, grows it and reads again while the reader
//! runs short of it, and then moves every string, sequence buffer and
//! indirection of the sample into storage of its own (`zd_own_*`) and
//! frees the heap. `sample_free` (`zd_free_*`) releases exactly what that
//! move allocated. The walks follow the encoder: a union's arm by its
//! discriminator, an `@optional` member by its pointer, a member on a type
//! cycle through its pointer — a sample a walk misread would free memory
//! it does not own.
//!
//! # What C has no form for
//!
//! A structure with no members (C23 §6.7.2.1p8) never reaches this: the
//! codec header refuses it first, and with it the file.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, CaseLabel, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::Purpose;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, PidFlagPolicy};
use zerodds_lang_cst::c;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::c::C;
use zerodds_lang_rules::emit::encode::CodecOptions;

/// The companion for every topic type of the model, or `None` when the
/// model declares none.
///
/// # Errors
/// A codec the generator cannot plan, a name the table would collide
/// with, or a type C has no walk for.
pub fn source(
    ast: &Ast,
    stem: &str,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Option<String>, String> {
    let topics = topic_types(ast);
    if topics.is_empty() {
        return Ok(None);
    }
    refuse_collisions(ast, &topics)?;

    let x2 = LayoutKey {
        encoding: Encoding::PlainCdr2,
        purpose: Purpose::Data,
        ..key
    };
    let x1 = LayoutKey {
        encoding: Encoding::PlainCdr1,
        purpose: Purpose::Data,
        ..key
    };

    // A version the codec header was generated for is read and written
    // through the codec header's own functions; only the others get a set
    // of their own. The header's functions would otherwise stand unused
    // beside their copies, which a compiler reports.
    let header_is = |encoding: Encoding| key.encoding == encoding && key.purpose == Purpose::Data;

    // XCDR2: the encoders and the decoders, with the defaulters those call.
    let (x2_prefix, x2_set) = if header_is(Encoding::PlainCdr2) {
        ("zd_", Vec::new())
    } else {
        let mut decls = zerodds_lang_rules::encode_c::c_codec(ast, x2, options)
            .map_err(|error| format!("the XCDR2 encoder: {error}"))?;
        decls.extend(
            zerodds_lang_rules::decode_c::c_decoder(ast, x2, options)
                .map_err(|error| format!("the XCDR2 decoder: {error}"))?,
        );
        let mut entries = Vec::new();
        for id in &topics {
            entries.push(zerodds_lang_rules::encode_c::codec_name(
                ast,
                *id,
                Purpose::Data,
            ));
            entries.push(zerodds_lang_rules::emit::decode::decoder_name::<C>(
                ast, *id,
            ));
        }
        ("zd_x2_", codec_set(decls, &entries, "zd_x2_")?)
    };

    // XCDR1: the decoders. A model with no XCDR1 plan reads XCDR1 as
    // unsupported, by name, rather than through the XCDR2 codec.
    let (x1_prefix, x1_set, x1_refusal) = if header_is(Encoding::PlainCdr1) {
        (Some("zd_"), Vec::new(), None)
    } else {
        match zerodds_lang_rules::decode_c::c_decoder(ast, x1, options) {
            Ok(decls) => {
                let entries: Vec<String> = topics
                    .iter()
                    .map(|id| zerodds_lang_rules::emit::decode::decoder_name::<C>(ast, *id))
                    .collect();
                (Some("zd_x1_"), codec_set(decls, &entries, "zd_x1_")?, None)
            }
            Err(error) => (None, Vec::new(), Some(error.to_string())),
        }
    };

    // The key holders, PLAIN_CDR2 big-endian (XTypes 1.3 §7.6.8), with the
    // data encoders a keyless nested holder writes whole.
    let holder_key = LayoutKey::key(Encoding::PlainCdr2);
    let mut holder_decls = Vec::new();
    let mut holders: Vec<(Id<TypeNode>, String, &'static str)> = Vec::new();
    for id in &topics {
        let Some(holder) = zerodds_lang_rules::encode_c::c_key_holder(ast, *id, holder_key)
            .map_err(|error| format!("{}: the key holder: {error}", type_label(ast, *id)))?
        else {
            continue;
        };
        let c::Decl::Function(function) = &holder else {
            return Err(format!(
                "{}: the key holder is not a function",
                type_label(ast, *id)
            ));
        };
        let form = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, *id, holder_key)
            .map_err(|error| format!("{}: the key holder's size: {error}", type_label(ast, *id)))?
        {
            zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zdw_zero_padded",
            zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "zdw_md5_of",
        };
        holders.push((*id, function.name.as_str().to_string(), form));
        holder_decls.extend(
            zerodds_lang_rules::encode_c::c_codec_from(ast, *id, holder_key, options)
                .map_err(|error| format!("{}: the key codec: {error}", type_label(ast, *id)))?,
        );
        holder_decls.extend(
            zerodds_lang_rules::encode_c::c_codec_from(
                ast,
                *id,
                LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
                options,
            )
            .map_err(|error| format!("{}: the key codec: {error}", type_label(ast, *id)))?,
        );
        holder_decls.push(holder);
    }
    let holder_entries: Vec<String> = holders.iter().map(|(_, name, _)| name.clone()).collect();
    let holder_set = codec_set(holder_decls, &holder_entries, "zd_kh_")?;

    // The walks over what a decoded sample owns.
    let mut walks = Walks::new(ast, x2);
    let mut owning = Vec::new();
    for id in &topics {
        owning.push(walks.entry(*id)?);
    }

    let mut out = String::new();
    let _ = write!(
        out,
        "// Generated by `idlc generate --lang c`: the DDS TypeSupport of {stem}.h. Do not edit.\n\
         //\n\
         // One `zerodds_typesupport_t` per topic type. The table writes XCDR2 —\n\
         // what the C API's writer stamps — and reads XCDR1 and XCDR2 in either\n\
         // byte order. A decoded sample owns its strings, sequence buffers and\n\
         // indirections; `sample_free` releases them. Include this file instead\n\
         // of {stem}.h, not beside it.\n\
         #ifndef ZD_TYPESUPPORT_{guard}_H\n\
         #define ZD_TYPESUPPORT_{guard}_H\n\
         \n\
         #include <stdint.h>\n\
         #include <stdlib.h>\n\
         #include <string.h>\n\
         \n\
         #include \"{stem}.h\"\n\
         #include \"zerodds_xcdr2.h\"\n\n",
        guard = guard_name(stem),
    );

    out.push_str(&framing(key));
    if x2_set.is_empty() {
        out.push_str("\n// XCDR2 is written and read through the codec header's functions.\n");
    } else {
        out.push_str("\n// The codec the table writes and reads XCDR2 with.\n");
        out.push_str(&render(&x2_set));
    }
    match (x1_prefix, &x1_refusal) {
        (None, refusal) => {
            let _ = writeln!(
                out,
                "\n// No XCDR1 codec: {}. A sample in XCDR1 is answered with\n\
                 // ZERODDS_TS_UNSUPPORTED.",
                refusal.as_deref().unwrap_or("the model has no XCDR1 plan")
            );
        }
        (Some(_), _) if x1_set.is_empty() => {
            out.push_str("\n// XCDR1 is read through the codec header's functions.\n");
        }
        (Some(_), _) => {
            out.push_str("\n// The codec the table reads XCDR1 with.\n");
            out.push_str(&render(&x1_set));
        }
    }
    if !holder_set.is_empty() {
        out.push_str("\n// The key holders, PLAIN_CDR2 big-endian (XTypes 1.3 §7.6.8).\n");
        out.push_str(&render(&holder_set));
    }
    out.push_str(&walks.render());

    for (id, owns) in topics.iter().zip(&owning) {
        let holder = holders
            .iter()
            .find(|(holder_id, _, _)| holder_id == id)
            .map(|(_, name, form)| (name.as_str(), *form));
        out.push('\n');
        out.push_str(&table(ast, *id, *owns, x2_prefix, x1_prefix, holder)?);
    }
    let _ = writeln!(out, "\n#endif /* ZD_TYPESUPPORT_{}_H */", guard_name(stem));
    Ok(Some(out))
}

/// The structures and unions a writer can be created for: named, and not
/// `@nested` (XTypes 1.3 §7.4.6.3.5 makes those member types only).
fn topic_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    ast.types()
        .filter(|(_, node)| {
            !node.name.is_empty()
                && matches!(
                    node.kind,
                    TypeKind::Structure { .. } | TypeKind::Union { .. }
                )
                && !crate::generate::is_nested(node)
        })
        .map(|(id, _)| id)
        .collect()
}

/// The IDL name of `id`, for a refusal.
fn type_label(ast: &Ast, id: Id<TypeNode>) -> String {
    ast.type_(id)
        .map_or_else(|_| String::from("?"), |node| node.name.clone())
}

/// The names this file declares for each topic type, against every type
/// name the codec header declares. A clash would not compile, or worse
/// would compile against the wrong declaration.
fn refuse_collisions(ast: &Ast, topics: &[Id<TypeNode>]) -> Result<(), String> {
    let declared: BTreeSet<String> = ast
        .types()
        .filter(|(_, node)| !node.name.is_empty())
        .map(|(id, _)| zerodds_lang_rules::adapter::c::c_type_name(ast, id))
        .collect();
    let mut ours = BTreeSet::new();
    for id in topics {
        let base = zerodds_lang_rules::adapter::c::c_type_name(ast, *id);
        for suffix in SUFFIXES {
            let name = format!("{base}{suffix}");
            if declared.contains(&name) {
                return Err(format!(
                    "the TypeSupport of `{}` would declare `{name}`, which the model declares \
                     as a type",
                    type_label(ast, *id)
                ));
            }
            if !ours.insert(name.clone()) {
                return Err(format!(
                    "two topic types would both declare `{name}` in the TypeSupport"
                ));
            }
        }
    }
    Ok(())
}

/// What each topic type's names end in.
const SUFFIXES: [&str; 6] = [
    "_typesupport",
    "_encode",
    "_decode",
    "_decode_repr",
    "_key_hash",
    "_sample_free",
];

/// `stem` as the include guard's middle part.
fn guard_name(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// The C name of a PID-flag policy. A `match` so a new policy has to be
/// given one.
const fn c_pid_flags(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => "ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND",
        PidFlagPolicy::Bare => "ZDW_PID_FLAGS_BARE",
        PidFlagPolicy::ImplExtensionOnMembers => "ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS",
    }
}

/// The writer and reader set-up every function below shares: the
/// dialect's PL_CDR1 framing and the half of the length-code policy a
/// plan cannot answer, the same six numbers the wire probe's harness
/// gives its writer.
fn framing(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    format!(
        "/* The flags a parameter id carries at each position, the highest short id,\n   \
         padded length, padded length for an @optional member outside a @mutable\n   \
         type, the member's own must-understand bit, padding after the value. */\n\
         static const zdw_pl_cdr1_framing zd_ts_framing = {{ {}, {}, {}, {}, {}, {} }};\n\
         \n\
         static void zd_ts_writer(zdw_writer *w, uint8_t *buffer, size_t capacity) {{\n    \
         zdw_writer_init(w, buffer, capacity, ZDW_LITTLE, {x2_align}u);\n    \
         zdw_writer_set_pl_cdr1(w, zd_ts_framing);\n    \
         zdw_writer_set_short_length_codes(w, {short_codes});\n\
         }}\n\
         \n\
         static void zd_ts_reader(zdw_reader *r, const uint8_t *bytes, size_t length,\n                         \
         zdw_endianness order, size_t max_align, uint8_t *heap, size_t capacity) {{\n    \
         zdw_reader_init(r, bytes, length, order, max_align);\n    \
         zdw_reader_set_pl_cdr1(r, zd_ts_framing);\n    \
         zdw_reader_set_heap(r, heap, capacity);\n\
         }}\n",
        c_pid_flags(key.pid_flags),
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value(),
        x2_align = Encoding::PlainCdr2.max_alignment(),
        short_codes = key.length_codes.short_code_where_it_fits(),
    )
}

/// The functions of a codec set: duplicates dropped (a definition met
/// twice has to be the same text — two different plans under one name
/// is a refusal), pruned to what `entries` reach, every name renamed
/// from `zd_` to `prefix`.
fn codec_set(
    decls: Vec<c::Decl>,
    entries: &[String],
    prefix: &str,
) -> Result<Vec<c::Decl>, String> {
    let mut kept: Vec<c::Function> = Vec::new();
    for decl in decls {
        let c::Decl::Function(function) = decl else {
            continue;
        };
        if let Some(seen) = kept.iter().find(|seen| {
            seen.name.as_str() == function.name.as_str()
                && seen.body.is_some() == function.body.is_some()
        }) {
            if render_function(seen) != render_function(&function) {
                return Err(format!(
                    "two different definitions of `{}` in one codec set",
                    function.name.as_str()
                ));
            }
            continue;
        }
        kept.push(function);
    }

    // Reachable from the entries, over direct calls.
    let mut wanted: Vec<String> = entries.to_vec();
    let mut index = 0;
    while index < wanted.len() {
        let name = wanted[index].clone();
        index += 1;
        for function in &kept {
            if function.name.as_str() != name {
                continue;
            }
            if let Some(body) = &function.body {
                let mut called = Vec::new();
                names_in_block(
                    &mut body.clone(),
                    &mut |ident: &mut zerodds_lang_cst::Ident| {
                        called.push(ident.as_str().to_string());
                    },
                );
                for callee in called {
                    if !wanted.contains(&callee) {
                        wanted.push(callee);
                    }
                }
            }
        }
    }
    let declared: BTreeSet<String> = kept
        .iter()
        .map(|function| function.name.as_str().to_string())
        .collect();
    for entry in entries {
        if !declared.contains(entry) {
            return Err(format!("the codec set declares no `{entry}`"));
        }
    }
    let renamed = |name: &str| format!("{prefix}{}", name.strip_prefix("zd_").unwrap_or(name));
    let mut out = Vec::new();
    for mut function in kept {
        if !wanted.iter().any(|name| name == function.name.as_str()) {
            continue;
        }
        function.name = zerodds_lang_cst::Ident::new(&renamed(function.name.as_str()));
        if let Some(body) = &mut function.body {
            names_in_block(body, &mut |ident: &mut zerodds_lang_cst::Ident| {
                if declared.contains(ident.as_str()) {
                    *ident = zerodds_lang_cst::Ident::new(&renamed(ident.as_str()));
                }
            });
        }
        out.push(c::Decl::Function(function));
    }
    Ok(out)
}

fn render_function(function: &c::Function) -> String {
    zerodds_lang_render::c::decl(&c::Decl::Function(function.clone()))
}

fn render(decls: &[c::Decl]) -> String {
    decls
        .iter()
        .map(zerodds_lang_render::c::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every name a block mentions, handed to `visit`.
///
/// zerodds-lint: recursion-depth = the nesting of the C the codec
/// emitters build. `c::Block`, `c::Statement` and `c::Expr` are an owned
/// tree with no shared or back reference, so the walk cannot cycle.
fn names_in_block(block: &mut c::Block, visit: &mut dyn FnMut(&mut zerodds_lang_cst::Ident)) {
    for statement in &mut block.statements {
        names_in_statement(statement, visit);
    }
}

/// zerodds-lint: recursion-depth = as [`names_in_block`].
fn names_in_statement(
    statement: &mut c::Statement,
    visit: &mut dyn FnMut(&mut zerodds_lang_cst::Ident),
) {
    match statement {
        c::Statement::Declare { value, .. } | c::Statement::Return(value) => {
            if let Some(value) = value {
                names_in_expr(value, visit);
            }
        }
        c::Statement::Expr(value) => names_in_expr(value, visit),
        c::Statement::If {
            condition,
            then,
            otherwise,
        } => {
            names_in_expr(condition, visit);
            names_in_block(then, visit);
            if let Some(block) = otherwise {
                names_in_block(block, visit);
            }
        }
        c::Statement::For {
            init,
            condition,
            step,
            body,
        } => {
            names_in_statement(init, visit);
            names_in_expr(condition, visit);
            names_in_expr(step, visit);
            names_in_block(body, visit);
        }
        c::Statement::While { condition, body } => {
            names_in_expr(condition, visit);
            names_in_block(body, visit);
        }
        c::Statement::Break | c::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth = as [`names_in_block`].
fn names_in_expr(expr: &mut c::Expr, visit: &mut dyn FnMut(&mut zerodds_lang_cst::Ident)) {
    match expr {
        c::Expr::Name(name) => visit(name),
        c::Expr::Call { callee, args } => {
            names_in_expr(callee, visit);
            for arg in args {
                names_in_expr(arg, visit);
            }
        }
        c::Expr::Member { value, .. }
        | c::Expr::Arrow { value, .. }
        | c::Expr::Unary { value, .. }
        | c::Expr::AddressOf(value)
        | c::Expr::Deref(value)
        | c::Expr::Paren(value)
        | c::Expr::Cast { value, .. } => names_in_expr(value, visit),
        c::Expr::Index { value, index } => {
            names_in_expr(value, visit);
            names_in_expr(index, visit);
        }
        c::Expr::Binary { lhs, rhs, .. } => {
            names_in_expr(lhs, visit);
            names_in_expr(rhs, visit);
        }
        c::Expr::Initializer(items) => {
            for item in items {
                names_in_expr(item, visit);
            }
        }
        c::Expr::Literal(_) | c::Expr::SizeOf(_) => {}
    }
}

/// `id` with every alias followed to the type it names.
fn resolved(ast: &Ast, mut id: Id<TypeNode>) -> Result<Id<TypeNode>, String> {
    for _ in 0..64 {
        let node = ast.type_(id).map_err(|error| format!("{error:?}"))?;
        match &node.kind {
            TypeKind::Alias { aliased } => {
                id = aliased
                    .resolved()
                    .ok_or_else(|| format!("`{}` names a type that is not resolved", node.name))?;
            }
            _ => return Ok(id),
        }
    }
    Err(String::from("a chain of more than 64 typedefs"))
}

/// The C spelling of `id`'s value type.
fn c_type(ast: &Ast, id: Id<TypeNode>) -> String {
    zerodds_lang_render::c::type_(&zerodds_lang_rules::adapter::c::c_value_type(ast, id))
}

/// Whether `place` is passed to a walk as it is (an array, which decays
/// to a pointer) or by its address.
fn argument(ast: &Ast, id: Id<TypeNode>, place: &str) -> Result<String, String> {
    let target = resolved(ast, id)?;
    Ok(match ast.type_(target).map(|node| &node.kind) {
        Ok(TypeKind::Array { .. }) => place.to_string(),
        _ => format!("&{place}"),
    })
}

/// The walks `zd_own_*` and `zd_free_*` over every type a decoded sample
/// can own storage through, keyed by the codec's tag for the type.
struct Walks<'a> {
    ast: &'a Ast,
    /// The plan a union's labels are read under: whether a negative label
    /// selects nothing is the dialect's, and the decoder chose by it.
    key: LayoutKey,
    emitted: BTreeSet<String>,
    prototypes: Vec<String>,
    bodies: Vec<String>,
    visiting: Vec<Id<TypeNode>>,
}

impl<'a> Walks<'a> {
    fn new(ast: &'a Ast, key: LayoutKey) -> Self {
        Self {
            ast,
            key,
            emitted: BTreeSet::new(),
            prototypes: Vec::new(),
            bodies: Vec::new(),
            visiting: Vec::new(),
        }
    }

    /// The walks for a topic type: whether its sample owns anything.
    fn entry(&mut self, id: Id<TypeNode>) -> Result<bool, String> {
        if !self.owns(id)? {
            return Ok(false);
        }
        self.walk(id)?;
        Ok(true)
    }

    /// Whether a value of `id` holds a pointer the decoder filled.
    ///
    /// zerodds-lint: recursion-depth 64 (the nesting of the model's types;
    /// a type the walk is already inside answers `false`, and every cycle
    /// carries a member held behind a pointer, which answers `true` on
    /// its own).
    fn owns(&mut self, id: Id<TypeNode>) -> Result<bool, String> {
        let id = resolved(self.ast, id)?;
        if self.visiting.contains(&id) {
            return Ok(false);
        }
        if self.visiting.len() > 64 {
            return Err(String::from("types nested more than 64 deep"));
        }
        let node = self.ast.type_(id).map_err(|error| format!("{error:?}"))?;
        let answer = match &node.kind {
            TypeKind::Primitive(_)
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. } => false,
            TypeKind::String { .. }
            | TypeKind::Fixed { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Map { .. } => true,
            TypeKind::Array { element, .. } => {
                let element = element.resolved().ok_or("an array of an unresolved type")?;
                self.visiting.push(id);
                let owns = self.owns(element);
                self.visiting.pop();
                owns?
            }
            TypeKind::Structure { .. } => {
                self.visiting.push(id);
                let owns = self.members_own(id);
                self.visiting.pop();
                owns?
            }
            TypeKind::Union { .. } => {
                self.visiting.push(id);
                let owns = self.arms_own(id);
                self.visiting.pop();
                owns?
            }
            TypeKind::Alias { .. } => {
                return Err(format!("`{}`: a typedef after its resolution", node.name));
            }
            _ => {
                return Err(format!(
                    "`{}`: no C TypeSupport for this kind of type",
                    node.name
                ));
            }
        };
        Ok(answer)
    }

    fn members_own(&mut self, id: Id<TypeNode>) -> Result<bool, String> {
        for zerodds_lang_rules::adapter::c::CMember { handle, owner, .. } in
            zerodds_lang_rules::adapter::c::c_members(self.ast, id)
        {
            let member = self
                .ast
                .member(handle)
                .map_err(|error| format!("{error:?}"))?;
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("`{}` has an unresolved type", member.name))?;
            if zerodds_lang_rules::adapter::c::member_held_behind_pointer(
                self.ast, owner, member, target,
            ) || self.owns(target)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn arms_own(&mut self, id: Id<TypeNode>) -> Result<bool, String> {
        for case in self.ast.cases_of(id) {
            let case = self.ast.case(*case).map_err(|error| format!("{error:?}"))?;
            let member = self
                .ast
                .member(case.member)
                .map_err(|error| format!("{error:?}"))?;
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("`{}` has an unresolved type", member.name))?;
            if zerodds_lang_rules::adapter::c::member_held_behind_pointer(
                self.ast, id, member, target,
            ) || self.owns(target)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The two walks over `id`, emitted once per tag.
    ///
    /// zerodds-lint: recursion-depth 64 (as [`Self::owns`]; a tag already
    /// emitted or being emitted returns at once, so a cycle stops at its
    /// second visit).
    fn walk(&mut self, id: Id<TypeNode>) -> Result<String, String> {
        let id = resolved(self.ast, id)?;
        let tag = zerodds_lang_rules::adapter::c::c_codec_tag(self.ast, id);
        if !self.emitted.insert(tag.clone()) {
            return Ok(tag);
        }
        let ty = c_type(self.ast, id);
        let node = self.ast.type_(id).map_err(|error| format!("{error:?}"))?;
        let array = matches!(node.kind, TypeKind::Array { .. });
        let param = if array {
            format!("{ty} v")
        } else {
            format!("{ty} *v")
        };
        let (own, free) = match &node.kind {
            TypeKind::String { .. } | TypeKind::Fixed { .. } => text_walks(),
            TypeKind::Sequence { element, .. } => {
                let element = element
                    .resolved()
                    .ok_or("a sequence of an unresolved type")?;
                self.buffer_walks(&c_type(self.ast, element), &[("", element)])?
            }
            TypeKind::Map { key, value, .. } => {
                let key = key.resolved().ok_or("a map with an unresolved key")?;
                let value = value.resolved().ok_or("a map with an unresolved value")?;
                let entry = zerodds_lang_rules::adapter::c::c_map_entry_name(self.ast, id);
                self.buffer_walks(&entry, &[(".key", key), (".value", value)])?
            }
            TypeKind::Array { element, dims } => {
                let element = element.resolved().ok_or("an array of an unresolved type")?;
                let mut sizes = Vec::new();
                for dim in dims {
                    let size = self
                        .ast
                        .eval_integral(*dim)
                        .map_err(|error| format!("an array bound: {error:?}"))?;
                    sizes.push(size);
                }
                self.array_walks(element, &sizes)?
            }
            TypeKind::Structure { .. } => self.structure_walks(id)?,
            TypeKind::Union { discriminator, .. } => {
                let discriminator = discriminator
                    .resolved()
                    .ok_or("a union with an unresolved discriminator")?;
                self.union_walks(id, discriminator)?
            }
            _ => {
                return Err(format!(
                    "`{}`: no C TypeSupport walk for this kind of type",
                    node.name
                ));
            }
        };
        self.prototypes.push(format!(
            "static void zd_own_{tag}({param}, int *zd_ok);\n\
             static void zd_free_{tag}({param});"
        ));
        self.bodies.push(format!(
            "static void zd_own_{tag}({param}, int *zd_ok) {{\n{own}}}\n\n\
             static void zd_free_{tag}({param}) {{\n{free}}}\n"
        ));
        Ok(tag)
    }

    /// A call of the walk over `id` at `place`, or nothing when a value of
    /// `id` owns nothing.
    fn call(&mut self, id: Id<TypeNode>, place: &str, own: bool) -> Result<Option<String>, String> {
        if !self.owns(id)? {
            return Ok(None);
        }
        let tag = self.walk(id)?;
        let argument = argument(self.ast, id, place)?;
        Ok(Some(if own {
            format!("zd_own_{tag}({argument}, zd_ok);")
        } else {
            format!("zd_free_{tag}({argument});")
        }))
    }

    /// A sequence or a map: the buffer moved into storage of its own and
    /// each element walked, or released.
    fn buffer_walks(
        &mut self,
        element: &str,
        parts: &[(&str, Id<TypeNode>)],
    ) -> Result<(String, String), String> {
        let mut own_calls = Vec::new();
        let mut free_calls = Vec::new();
        for (field, id) in parts {
            let place = format!("v->_buffer[zd_i]{field}");
            if let Some(call) = self.call(*id, &place, true)? {
                own_calls.push(call);
            }
            if let Some(call) = self.call(*id, &place, false)? {
                free_calls.push(call);
            }
        }
        let index = if own_calls.is_empty() {
            ""
        } else {
            "    uint32_t zd_i;\n"
        };
        // A sequence and a map keep their capacity and say who owns the buffer: the copy is
        // the sample's own heap block.
        let map_reset = "\x20       v->_maximum = 0u;\n\x20       v->_release = false;\n";
        let map_owned = "    v->_maximum = v->_length;\n    v->_release = true;\n";
        let mut own = format!(
            "    {element} *zd_copy = NULL;\n{index}\
             \x20   if (*zd_ok && v->_length > 0u) {{\n\
             \x20       zd_copy = ({element} *)malloc((size_t)v->_length * sizeof({element}));\n\
             \x20       if (zd_copy == NULL) {{\n\
             \x20           *zd_ok = 0;\n\
             \x20       }} else {{\n\
             \x20           memcpy(zd_copy, v->_buffer, (size_t)v->_length * sizeof({element}));\n\
             \x20       }}\n\
             \x20   }}\n\
             \x20   if (zd_copy == NULL) {{\n\
             \x20       v->_length = 0u;\n\
             \x20       v->_buffer = NULL;\n{map_reset}\
             \x20       return;\n\
             \x20   }}\n\
             \x20   v->_buffer = zd_copy;\n{map_owned}"
        );
        if !own_calls.is_empty() {
            let _ = writeln!(own, "    for (zd_i = 0u; zd_i < v->_length; zd_i++) {{");
            for call in &own_calls {
                let _ = writeln!(own, "        {call}");
            }
            own.push_str("    }\n");
        }
        let mut free = String::new();
        if !free_calls.is_empty() {
            free.push_str("    uint32_t zd_i;\n    if (v->_buffer != NULL) {\n");
            free.push_str("        for (zd_i = 0u; zd_i < v->_length; zd_i++) {\n");
            for call in &free_calls {
                let _ = writeln!(free, "            {call}");
            }
            free.push_str("        }\n    }\n");
        }
        free.push_str("    free(v->_buffer);\n    v->_buffer = NULL;\n    v->_length = 0u;\n");
        free.push_str("    v->_maximum = 0u;\n    v->_release = false;\n");
        Ok((own, free))
    }

    /// An array: every element walked, over one loop per dimension.
    fn array_walks(
        &mut self,
        element: Id<TypeNode>,
        sizes: &[i128],
    ) -> Result<(String, String), String> {
        let mut place = String::from("v");
        let mut header = String::new();
        let mut opening = String::new();
        let mut closing = String::new();
        for (depth, size) in sizes.iter().enumerate() {
            let _ = writeln!(header, "    size_t zd_i{depth};");
            let indent = "    ".repeat(depth + 1);
            let _ = writeln!(
                opening,
                "{indent}for (zd_i{depth} = 0u; zd_i{depth} < {size}u; zd_i{depth}++) {{"
            );
            closing = format!("{indent}}}\n{closing}");
            let _ = write!(place, "[zd_i{depth}]");
        }
        let inner = "    ".repeat(sizes.len() + 1);
        let own_call = self
            .call(element, &place, true)?
            .ok_or("an array whose element owns nothing has no walk")?;
        let free_call = self
            .call(element, &place, false)?
            .ok_or("an array whose element owns nothing has no walk")?;
        Ok((
            format!("{header}{opening}{inner}{own_call}\n{closing}"),
            format!("{header}{opening}{inner}{free_call}\n{closing}"),
        ))
    }

    /// A structure: every member that owns something, the `@optional` ones
    /// behind their flag, the ones on a type cycle through their pointer.
    fn structure_walks(&mut self, id: Id<TypeNode>) -> Result<(String, String), String> {
        let mut own = String::new();
        let mut free = String::new();
        // The base is the member `parent` (XTypes 1.3 §7.5.1.4.1.3): a member of it
        // is reached as `parent.m`, and is the base's to hold.
        for zerodds_lang_rules::adapter::c::CMember {
            handle,
            path,
            owner,
        } in zerodds_lang_rules::adapter::c::c_members(self.ast, id)
        {
            let member = self
                .ast
                .member(handle)
                .map_err(|error| format!("{error:?}"))?
                .clone();
            let field = C::escape(&member.name);
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("`{}` has an unresolved type", member.name))?;
            // XTypes 1.3 §7.5.1.2.4.1: an `@optional` member is a pointer
            // and NULL is the absent one. A `string` is a pointer already,
            // so its walk is guarded by the pointer itself.
            let flag = (member.facets.is_optional()
                && zerodds_lang_rules::adapter::c::value_is_pointer(self.ast, target))
            .then(|| format!("v->{path}{field} != NULL"));
            let place = format!("v->{path}{field}");
            let annotated = zerodds_lang_rules::adapter::c::pointer_annotated(&member);
            let (o, f) = self.member_walks(owner, target, annotated, &place, flag.as_deref())?;
            own.push_str(&o);
            free.push_str(&f);
        }
        if own.is_empty() {
            return Err(format!(
                "`{}` owns nothing and has no walk",
                type_label(self.ast, id)
            ));
        }
        Ok((own, free))
    }

    /// One member's two walks, or two empty strings when it owns nothing.
    fn member_walks(
        &mut self,
        aggregate: Id<TypeNode>,
        target: Id<TypeNode>,
        annotated: bool,
        place: &str,
        flag: Option<&str>,
    ) -> Result<(String, String), String> {
        if zerodds_lang_rules::adapter::c::pointer_member(self.ast, aggregate, target, annotated) {
            // The value lives behind a pointer the decoder allocated in its
            // heap: a copy of it, walked, takes its place. An absent
            // `@optional` member's pointer is not read at all: inside a value
            // the decoder put in its heap it was never written, and the flag
            // is what says whether it means anything. It is NULL afterwards.
            let ty = c_type(self.ast, target);
            let pointee = format!("(*{place})");
            let own_inner = self
                .call(target, "(*zd_copy)", true)?
                .map(|call| format!("            {call}\n"))
                .unwrap_or_default();
            let free_inner = self
                .call(target, &pointee, false)?
                .map(|call| format!("        {call}\n"))
                .unwrap_or_default();
            let present = flag.map_or_else(|| String::from("1"), str::to_string);
            let absent = flag
                .map(|flag| format!("        }} else {{\n            {flag} = false;\n"))
                .unwrap_or_default();
            let own = format!(
                "    {{\n\
                 \x20       {ty} *zd_copy = NULL;\n\
                 \x20       if ({present} && *zd_ok && {place} != NULL) {{\n\
                 \x20           zd_copy = ({ty} *)malloc(sizeof({ty}));\n\
                 \x20           if (zd_copy == NULL) {{\n\
                 \x20               *zd_ok = 0;\n\
                 \x20           }} else {{\n\
                 \x20               memcpy(zd_copy, {place}, sizeof({ty}));\n\
                 \x20           }}\n\
                 \x20       }}\n\
                 \x20       {place} = zd_copy;\n\
                 \x20       if (zd_copy != NULL) {{\n{own_inner}{absent}\
                 \x20       }}\n\
                 \x20   }}\n"
            );
            let free = format!(
                "    if ({place} != NULL) {{\n{free_inner}\
                 \x20       free({place});\n\
                 \x20       {place} = NULL;\n\
                 \x20   }}\n"
            );
            return Ok((own, free));
        }
        let (Some(own_call), Some(free_call)) = (
            self.call(target, place, true)?,
            self.call(target, place, false)?,
        ) else {
            return Ok((String::new(), String::new()));
        };
        Ok(match flag {
            Some(flag) => (
                format!("    if ({flag}) {{\n        {own_call}\n    }}\n"),
                format!("    if ({flag}) {{\n        {free_call}\n    }}\n"),
            ),
            None => (format!("    {own_call}\n"), format!("    {free_call}\n")),
        })
    }

    /// A union: the arm the discriminator selects, by the chain of
    /// comparisons the encoder builds — every arm in it, the ones that
    /// own nothing too, so `default:` stays the arm no label names.
    fn union_walks(
        &mut self,
        id: Id<TypeNode>,
        discriminator: Id<TypeNode>,
    ) -> Result<(String, String), String> {
        let suppress_negative =
            zerodds_ast_layout::plan::union_negative_selects_nothing(self.ast, id, self.key);
        let mut arms: Vec<(Option<String>, String, String)> = Vec::new();
        let mut fallback: Option<(String, String)> = None;
        for case in self.ast.cases_of(id).to_vec() {
            let case = self
                .ast
                .case(case)
                .map_err(|error| format!("{error:?}"))?
                .clone();
            let member = self
                .ast
                .member(case.member)
                .map_err(|error| format!("{error:?}"))?
                .clone();
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("`{}` has an unresolved type", member.name))?;
            let place = format!("v->_u.{}", C::escape(&member.name));
            let annotated = zerodds_lang_rules::adapter::c::pointer_annotated(&member);
            let (own, free) = self.member_walks(id, target, annotated, &place, None)?;
            let mut condition: Option<String> = None;
            let mut is_default = false;
            for label in &case.labels {
                match label {
                    CaseLabel::Default => is_default = true,
                    CaseLabel::Value(expr) => {
                        let value = self
                            .ast
                            .eval_integral(*expr)
                            .map_err(|error| format!("a union label: {error:?}"))?;
                        if suppress_negative && value < 0 {
                            continue;
                        }
                        let label = zerodds_lang_render::c::expr(&C::case_label(
                            self.ast,
                            discriminator,
                            value,
                        ));
                        let test = format!("v->_d == {label}");
                        condition = Some(match condition {
                            None => test,
                            Some(previous) => format!("{previous} || {test}"),
                        });
                    }
                }
            }
            if is_default {
                fallback = Some((own.clone(), free.clone()));
            }
            if let Some(condition) = condition {
                arms.push((Some(condition), own, free));
            }
        }
        let chain = |pick: fn(&(Option<String>, String, String)) -> &String,
                     fallback: Option<&String>| {
            let mut out = String::new();
            let mut first = true;
            for arm in &arms {
                let Some(condition) = &arm.0 else { continue };
                let keyword = if first { "    if" } else { " else if" };
                first = false;
                let _ = write!(
                    out,
                    "{keyword} ({condition}) {{\n{}    }}",
                    indent(pick(arm))
                );
            }
            if let Some(body) = fallback {
                if first {
                    out.push_str(body);
                    return out;
                }
                let _ = write!(out, " else {{\n{}    }}", indent(body));
            }
            if !first {
                out.push('\n');
            }
            out
        };
        let own = chain(|arm| &arm.1, fallback.as_ref().map(|(own, _)| own));
        let free = chain(|arm| &arm.2, fallback.as_ref().map(|(_, free)| free));
        Ok((own, free))
    }

    fn render(&self) -> String {
        if self.bodies.is_empty() {
            return String::new();
        }
        format!(
            "\n// What a decoded sample owns: `zd_own_*` moves it out of the decoder's heap\n\
             // into storage of its own, `zd_free_*` releases that storage. `*zd_ok` turns 0\n\
             // when an allocation fails, and the rest of the walk then leaves its pointers\n\
             // NULL rather than pointing into the heap.\n{}\n\n{}",
            self.prototypes.join("\n"),
            self.bodies.join("\n")
        )
    }
}

/// `body` one level deeper.
fn indent(body: &str) -> String {
    body.lines()
        .map(|line| {
            if line.is_empty() {
                String::from("\n")
            } else {
                format!("    {line}\n")
            }
        })
        .collect()
}

/// A string, a wstring (UTF-8) or a fixed (its decimal): one `char`
/// array, copied with its terminator.
fn text_walks() -> (String, String) {
    (
        String::from(
            "    char *zd_copy = NULL;\n\
             \x20   size_t zd_n;\n\
             \x20   if (*zd_ok && *v != NULL) {\n\
             \x20       zd_n = strlen(*v) + 1u;\n\
             \x20       zd_copy = (char *)malloc(zd_n);\n\
             \x20       if (zd_copy == NULL) {\n\
             \x20           *zd_ok = 0;\n\
             \x20       } else {\n\
             \x20           memcpy(zd_copy, *v, zd_n);\n\
             \x20       }\n\
             \x20   }\n\
             \x20   *v = zd_copy;\n",
        ),
        String::from("    free((void *)(uintptr_t)*v);\n    *v = NULL;\n"),
    )
}

/// The five functions and the table for one topic type.
fn table(
    ast: &Ast,
    id: Id<TypeNode>,
    owns: bool,
    x2: &str,
    x1: Option<&str>,
    holder: Option<(&str, &'static str)>,
) -> Result<String, String> {
    let node = ast.type_(id).map_err(|error| format!("{error:?}"))?;
    let name = zerodds_lang_rules::adapter::c::c_type_name(ast, id);
    let tag = zerodds_lang_rules::adapter::c::c_codec_tag(ast, id);
    let idl_name = crate::generate::qualified_idl_name(ast, id, node);
    let extensibility = match ast.effective_extensibility(id) {
        Extensibility::Final => 0,
        Extensibility::Appendable => 1,
        Extensibility::Mutable => 2,
    };
    let hash = type_hash(ast, id);
    let hash_text = hash
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let x2_align = Encoding::PlainCdr2.max_alignment();
    let x1_align = Encoding::PlainCdr1.max_alignment();
    // A codec header generated for another version than XCDR2 keeps an
    // encoder this table does not write with. Named here, where the reason
    // stands, so that a compiler does not report it unused.
    let header_encoder = if x2 == "zd_" {
        String::new()
    } else {
        format!(
            "    /* The codec header's own encoder writes the version it was generated\n\
             \x20      for; the C API's writer stamps XCDR2, which {x2}enc_{tag} writes. */\n\
             \x20   (void)zd_enc_{tag};\n"
        )
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "// ---- `{idl_name}` ----\n\
         \n\
         /* One sample as XCDR2 little-endian octets. `out_buf == NULL`, or a buffer too\n   \
         short, answers ZERODDS_TS_BUFFER_TOO_SMALL with the length needed in *out_len. */\n\
         static int {name}_encode(const void *sample, uint8_t *out_buf, size_t out_cap,\n\
         \x20                   size_t *out_len) {{\n\
         \x20   zdw_writer w;\n\
         \x20   zdw_status status;\n\
         \x20   uint8_t *scratch;\n\
         \x20   size_t capacity;\n\
         {header_encoder}\
         \x20   if (sample == NULL || out_len == NULL) {{\n\
         \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20   }}\n\
         \x20   if (out_buf != NULL) {{\n\
         \x20       zd_ts_writer(&w, out_buf, out_cap);\n\
         \x20       {x2}enc_{tag}(&w, (const {name} *)sample);\n\
         \x20       status = zdw_writer_status(&w);\n\
         \x20       if (status == ZDW_OK) {{\n\
         \x20           *out_len = zdw_writer_length(&w);\n\
         \x20           return ZERODDS_TS_OK;\n\
         \x20       }}\n\
         \x20       if (status != ZDW_SHORT_BUFFER) {{\n\
         \x20           return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20       }}\n\
         \x20   }}\n\
         \x20   /* The writer stops at the first short write, so the length is found by\n\
         \x20      writing into storage large enough for it. */\n\
         \x20   for (capacity = 256u;; capacity *= 2u) {{\n\
         \x20       scratch = (uint8_t *)malloc(capacity);\n\
         \x20       if (scratch == NULL) {{\n\
         \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
         \x20       }}\n\
         \x20       zd_ts_writer(&w, scratch, capacity);\n\
         \x20       {x2}enc_{tag}(&w, (const {name} *)sample);\n\
         \x20       status = zdw_writer_status(&w);\n\
         \x20       *out_len = zdw_writer_length(&w);\n\
         \x20       free(scratch);\n\
         \x20       if (status == ZDW_OK) {{\n\
         \x20           return ZERODDS_TS_BUFFER_TOO_SMALL;\n\
         \x20       }}\n\
         \x20       if (status != ZDW_SHORT_BUFFER) {{\n\
         \x20           return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20       }}\n\
         \x20       if (capacity > SIZE_MAX / 2u) {{\n\
         \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
         \x20       }}\n\
         \x20   }}\n\
         }}\n\n"
    );

    // The decoder: the heap grows while the reader runs short of it. A
    // decoder reads every string through `zdw_read_string_alloc`, so a
    // short heap is the only `ZDW_SHORT_BUFFER` a read here can answer.
    let x1_branch = if let Some(x1) = x1 {
        format!(
            "        }} else if (version == ZERODDS_TS_REPR_XCDR1) {{\n\
             \x20           zd_ts_reader(&r, buf, len, order, {x1_align}u, heap, capacity);\n\
             \x20           max_align = {x1_align}u;\n\
             \x20           {x1}dec_{tag}(&r, &v, ZDW_TC_DISCARD);\n"
        )
    } else {
        String::from(
            "        } else if (version == ZERODDS_TS_REPR_XCDR1) {\n\
             \x20           free(heap);\n\
             \x20           return ZERODDS_TS_UNSUPPORTED;\n",
        )
    };
    let own_call = if owns {
        format!(
            "    ok = 1;\n    zd_own_{tag}(&v, &ok);\n    free(heap);\n    if (!ok) {{\n        zd_free_{tag}(&v);\n        return ZERODDS_TS_OUT_OF_RESOURCES;\n    }}\n"
        )
    } else {
        String::from("    free(heap);\n")
    };
    let ok_decl = if owns { "    int ok;\n" } else { "" };
    let _ = write!(
        out,
        "/* One sample from its octets; `representation` is the version\n   \
         (ZERODDS_TS_REPR_XCDR1, ZERODDS_TS_REPR_XCDR2), with ZERODDS_TS_REPR_BIG_ENDIAN\n   \
         set for a big-endian sample. The sample owns what it holds afterwards:\n   \
         {name}_sample_free releases it. */\n\
         static int {name}_decode_repr(const uint8_t *buf, size_t len, uint8_t representation,\n\
         \x20                        void *out_sample) {{\n\
         \x20   {name} v;\n\
         \x20   zdw_reader r;\n\
         \x20   zdw_status status;\n\
         \x20   zdw_endianness order;\n\
         \x20   uint8_t version;\n\
         \x20   uint8_t *heap;\n\
         \x20   size_t capacity;\n\
         \x20   size_t max_align;\n\
         \x20   size_t pad;\n\
         {ok_decl}\
         \x20   if ((buf == NULL && len > 0u) || out_sample == NULL) {{\n\
         \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20   }}\n\
         \x20   order = (representation & ZERODDS_TS_REPR_BIG_ENDIAN) != 0u ? ZDW_BIG : ZDW_LITTLE;\n\
         \x20   version = (uint8_t)(representation & (uint8_t)~ZERODDS_TS_REPR_BIG_ENDIAN);\n\
         \x20   if (version != ZERODDS_TS_REPR_XCDR1 && version != ZERODDS_TS_REPR_XCDR2) {{\n\
         \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20   }}\n\
         \x20   for (capacity = len + 64u;; capacity *= 2u) {{\n\
         \x20       heap = (uint8_t *)malloc(capacity);\n\
         \x20       if (heap == NULL) {{\n\
         \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
         \x20       }}\n\
         \x20       memset(&v, 0, sizeof v);\n\
         \x20       if (version == ZERODDS_TS_REPR_XCDR2) {{\n\
         \x20           zd_ts_reader(&r, buf, len, order, {x2_align}u, heap, capacity);\n\
         \x20           max_align = {x2_align}u;\n\
         \x20           {x2}dec_{tag}(&r, &v, ZDW_TC_DISCARD);\n\
         {x1_branch}\
         \x20       }} else {{\n\
         \x20           free(heap);\n\
         \x20           return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20       }}\n\
         \x20       status = zdw_reader_status(&r);\n\
         \x20       if (status != ZDW_SHORT_BUFFER) {{\n\
         \x20           break;\n\
         \x20       }}\n\
         \x20       free(heap);\n\
         \x20       if (capacity > SIZE_MAX / 2u) {{\n\
         \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
         \x20       }}\n\
         \x20   }}\n\
         \x20   if (status != ZDW_OK) {{\n\
         \x20       free(heap);\n\
         \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20   }}\n\
         \x20   /* Nothing may be left that is not padding: a decoder that stopped short\n\
         \x20      would hand back a sample with its last members missing. */\n\
         \x20   pad = (max_align - (zdw_reader_position(&r) % max_align)) % max_align;\n\
         \x20   if (zdw_reader_remaining(&r) > pad) {{\n\
         \x20       free(heap);\n\
         \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
         \x20   }}\n\
         {own_call}\
         \x20   memcpy(out_sample, &v, sizeof v);\n\
         \x20   return ZERODDS_TS_OK;\n\
         }}\n\n\
         /* As {name}_decode_repr for XCDR2 little-endian, the version and order the\n   \
         C API's writer uses. */\n\
         static int {name}_decode(const uint8_t *buf, size_t len, void *out_sample) {{\n\
         \x20   return {name}_decode_repr(buf, len, ZERODDS_TS_REPR_XCDR2, out_sample);\n\
         }}\n\n"
    );

    let key_hash_entry = if let Some((holder, form)) = holder {
        let renamed = format!("zd_kh_{}", holder.strip_prefix("zd_").unwrap_or(holder));
        let _ = write!(
            out,
            "/* The KeyHash (DDSI-RTPS 2.5 §9.6.4.8): the key holder in PLAIN_CDR2\n   \
             big-endian, then {form}. */\n\
             static int {name}_key_hash(const void *sample, uint8_t *out_hash) {{\n\
             \x20   zdw_writer w;\n\
             \x20   zdw_status status;\n\
             \x20   uint8_t *scratch;\n\
             \x20   size_t capacity;\n\
             \x20   if (sample == NULL || out_hash == NULL) {{\n\
             \x20       return ZERODDS_TS_BAD_PARAMETER;\n\
             \x20   }}\n\
             \x20   for (capacity = 64u;; capacity *= 2u) {{\n\
             \x20       scratch = (uint8_t *)malloc(capacity);\n\
             \x20       if (scratch == NULL) {{\n\
             \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
             \x20       }}\n\
             \x20       zdw_writer_init(&w, scratch, capacity, ZDW_BIG, {x2_align}u);\n\
             \x20       {renamed}(&w, (const {name} *)sample);\n\
             \x20       status = zdw_writer_status(&w);\n\
             \x20       if (status == ZDW_OK) {{\n\
             \x20           {form}(&w, out_hash);\n\
             \x20           free(scratch);\n\
             \x20           return ZERODDS_TS_OK;\n\
             \x20       }}\n\
             \x20       free(scratch);\n\
             \x20       if (status != ZDW_SHORT_BUFFER) {{\n\
             \x20           return ZERODDS_TS_BAD_PARAMETER;\n\
             \x20       }}\n\
             \x20       if (capacity > SIZE_MAX / 2u) {{\n\
             \x20           return ZERODDS_TS_OUT_OF_RESOURCES;\n\
             \x20       }}\n\
             \x20   }}\n\
             }}\n\n"
        );
        format!("{name}_key_hash")
    } else {
        String::from("NULL")
    };

    let free_entry = if owns {
        let _ = write!(
            out,
            "/* Releases what a decoded sample owns; the sample itself is the caller's. */\n\
             static void {name}_sample_free(void *sample) {{\n\
             \x20   if (sample != NULL) {{\n\
             \x20       zd_free_{tag}(({name} *)sample);\n\
             \x20   }}\n\
             }}\n\n"
        );
        format!("{name}_sample_free")
    } else {
        String::from("NULL")
    };

    let _ = write!(
        out,
        "/* The table for `{idl_name}`. `type_hash` is the EquivalenceHash of its\n   \
         CompleteTypeObject (XTypes 1.3 §7.3.4.8), zero-padded to 16 octets, and all\n   \
         zero for a type without one. */\n\
         static const zerodds_typesupport_t {name}_typesupport = {{\n\
         \x20   .type_hash = {{ {hash_text} }},\n\
         \x20   .type_name = \"{idl_name}\",\n\
         \x20   .is_keyed = {keyed},\n\
         \x20   .extensibility = {extensibility},\n\
         \x20   ._reserved = {{ 0, 0, 0, 0, 0, 0 }},\n\
         \x20   .encode = {name}_encode,\n\
         \x20   .decode = {name}_decode,\n\
         \x20   .key_hash = {key_hash_entry},\n\
         \x20   .sample_free = {free_entry},\n\
         \x20   .decode_repr = {name}_decode_repr,\n\
         }};\n",
        keyed = u8::from(holder.is_some()),
    );
    Ok(out)
}

/// The EquivalenceHash of `id`'s CompleteTypeObject, fourteen octets and
/// two zeros; sixteen zeros for a type without a TypeObject.
fn type_hash(ast: &Ast, id: Id<TypeNode>) -> [u8; 16] {
    let mut out = [0u8; 16];
    let hash = zerodds_types::from_ast::complete_type_object(ast, id)
        .ok()
        .map(zerodds_types::type_object::TypeObject::Complete)
        .and_then(|object| zerodds_types::hash::compute_hash(&object).ok());
    if let Some(hash) = hash {
        out[..14].copy_from_slice(&hash.0);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn model(idl: &str) -> Ast {
        let composed = zerodds_idl_compose::compose_source(
            "support.idl",
            idl,
            &zerodds_idl_compose::PathLoader::bare(),
        );
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        composed.ast
    }

    /// The codec header and the companion, as `idlc generate --lang c`
    /// writes them.
    fn both(idl: &str, encoding: Encoding) -> (String, String) {
        let files = crate::generate::files(
            &model(idl),
            zerodds_lang_cst::Language::C,
            "support.idl",
            "support",
            LayoutKey::data(encoding, Endianness::Little),
            CodecOptions::default(),
            None,
        )
        .expect("the C files");
        let text = |name: &str| {
            files
                .iter()
                .find(|file| file.name == name)
                .map(|file| file.text.clone())
                .unwrap_or_else(|| panic!("no {name}"))
        };
        (text("support.h"), text("support_typesupport.h"))
    }

    fn companion(idl: &str) -> String {
        both(idl, Encoding::PlainCdr2).1
    }

    /// The six assertions the C API's conformance test made of
    /// `idl-cpp`'s table, against this one. `@final struct Empty {};`
    /// is the seventh row's snippet and C has no form for it — the codec
    /// header refuses it, and this file with it.
    #[test]
    fn the_tables_say_what_the_types_are() {
        let point = companion("@final struct Point { long x; long y; };");
        assert!(point.contains("static const zerodds_typesupport_t Point_typesupport"));
        assert!(point.contains(".type_name = \"Point\""));
        assert!(point.contains(".extensibility = 0,"));
        assert!(point.contains(".is_keyed = 0,"));
        assert!(point.contains(".key_hash = NULL,"));

        let scoped = companion("module Outer { module Inner { @final struct S { long x; }; }; };");
        assert!(
            scoped.contains(".type_name = \"Outer::Inner::S\""),
            "{scoped}"
        );
        assert!(scoped.contains("Outer_Inner_S_typesupport"));

        let sensor = companion("@final struct Sensor { @key long id; double value; };");
        assert!(sensor.contains(".is_keyed = 1,"));
        assert!(sensor.contains("static int Sensor_key_hash(const void *sample"));
        assert!(sensor.contains(".key_hash = Sensor_key_hash,"));
        assert!(sensor.contains("zdw_zero_padded(&w, out_hash);"));

        let appendable = companion("@appendable struct V { long a; long b; };");
        assert!(appendable.contains(".extensibility = 1,"));

        // The EMHEADERs are the codec's, and the table writes through it:
        // member 1 with LC=2 (four octets, `0x20000001`), member 2 with LC=5
        // (a string, NEXTINT is its own length, `0x50000002`), and no
        // PID_LIST_END — that is XCDR1's.
        let idl = "@mutable struct M { @id(1) long a; @id(2) string b; };";
        let (header, mutable) = both(idl, Encoding::PlainCdr2);
        assert!(mutable.contains(".extensibility = 2,"));
        assert!(mutable.contains("zd_enc_M(&w, (const M *)sample);"));
        let encoder = header
            .split("static void zd_enc_M(zdw_writer *w, const M *v) {")
            .nth(1)
            .expect("the encoder's body")
            .split("\n}\n")
            .next()
            .expect("its end");
        assert!(
            encoder.contains("zdw_write_emheader(w, 1, 2, false)"),
            "{encoder}"
        );
        assert!(encoder.contains("zdw_write_emheader(w, 2, 5, false)"));
        assert!(!encoder.contains("write_sentinel"), "{encoder}");

        // Generated for XCDR1, the table still writes XCDR2, through a codec
        // of its own, and reads XCDR1 through the header's.
        let (_, under_xcdr1) = both(idl, Encoding::PlainCdr1);
        assert!(under_xcdr1.contains("zd_x2_enc_M(&w, (const M *)sample);"));
        assert!(under_xcdr1.contains("zd_dec_M(&r, &v, ZDW_TC_DISCARD);"));
        assert!(under_xcdr1.contains("static void zd_x2_enc_M(zdw_writer *w, const M *v) {"));
    }

    /// A sample that owns nothing gets no `sample_free`; one that owns a
    /// string does, and the walk follows an `@optional` member's flag.
    #[test]
    fn a_sample_that_owns_storage_is_released_by_its_table() {
        let plain = companion("@final struct P { long x; };");
        assert!(plain.contains(".sample_free = NULL,"));
        let owning = companion("@final struct O { @optional string s; sequence<string> l; };");
        assert!(owning.contains(".sample_free = O_sample_free,"));
        assert!(owning.contains("if (v->s != NULL) {"), "{owning}");
    }

    /// A union walks the arm its discriminator selects, and `default:`
    /// stays the arm no label names.
    #[test]
    fn a_union_walks_the_selected_arm() {
        let text = companion(
            "union W switch (long) { case 1: long a; default: string d; };\n\
             @final struct H { W value; };",
        );
        let walk = text
            .split("static void zd_own_W(W *v, int *zd_ok) {")
            .nth(1)
            .expect("the walk")
            .split("\n}\n")
            .next()
            .expect("its end");
        assert!(walk.contains("if (v->_d == (int32_t)1) {"), "{walk}");
        assert!(walk.contains("} else {"), "{walk}");
    }
}
