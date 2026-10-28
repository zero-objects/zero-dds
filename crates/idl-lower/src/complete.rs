// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Completion by specification rule. Spec §5.
//!
//! What the source leaves implicit is filled in here. Nothing is
//! guessed: every value below is determined by a paragraph, and every
//! completed value records how it was arrived at as well as what it is.
//!
//! The origin matters as much as the number. Two members with the same
//! id is only diagnosable as a mistake if it is known which one declared
//! its id and which one derived it — a declared id colliding with a
//! derived one is the declaration's fault, two declared ids colliding is
//! the source's.

use crate::lower::{HashedIdCounter, HashedNameSpelling, Lowering};
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast::{
    AnyId, AutoId, MemberIdAssignment, MemberIdOrigin, RepositoryIdOrigin, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_idl_parse::DiagnosticCode;

/// Members of one structure, as a path asks for them to be keys.
type MemberSet = alloc::collections::BTreeSet<Id<zerodds_ast::MemberNode>>;

/// One member name or path a key directive wrote, and where.
struct KeyDirective {
    /// The directive as a diagnostic quotes it, `keylist T` or
    /// `#pragma DCPS_DATA_KEY T`.
    text: String,
    /// The member name or path, as written.
    field: String,
    span: zerodds_idl_cst::Span,
}

/// A key path (`a.b`) waiting for [`Lowering::apply_key_paths`]: `member`
/// of `owner` is the first step, `rest` the member names below it.
struct KeyPath {
    owner: Id<TypeNode>,
    member: Id<zerodds_ast::MemberNode>,
    rest: Vec<String>,
    directive: KeyDirective,
}

impl Lowering<'_> {
    /// Fill in everything the source left implicit.
    ///
    /// Runs after the whole file is lowered, because a rule that reads
    /// the previous member's id needs every member to exist first.
    pub(crate) fn complete(&mut self) {
        self.apply_default_nested();
        let types: Vec<Id<TypeNode>> = self.ast.types().map(|(id, _)| id).collect();
        for id in self.base_first(&types) {
            self.complete_type_defaults(id);
        }
        self.apply_repository_directives();
        let mut paths = self.apply_keylists();
        paths.extend(self.apply_open_dds_pragmas());
        self.apply_key_paths(paths);
        self.keep_unrecognised_pragmas();
        self.derive_repository_ids(&types);
        self.check_bit_bounds(&types);
    }

    /// `Options::default_nested`, written onto the root module.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.7: the top-level modules that carry no
    /// `@default_nested` of their own take the compiler's default. The
    /// root module is the enclosing module of every top-level one, and
    /// `Ast::effective_nested` reads the nearest module that says
    /// something, so the option is the root's word and a module's own
    /// annotation still wins.
    fn apply_default_nested(&mut self) {
        let Some(value) = self.options.default_nested else {
            return;
        };
        let root = self.ast.root_module();
        let Ok(node) = self.ast.module(root) else {
            return;
        };
        let any = AnyId::Module(root, node.node_id);
        let already = self
            .ast
            .facets(any)
            .and_then(|facets| facets.xtypes.as_ref())
            .and_then(|x| x.default_nested);
        if already.is_none() {
            if let Some(facets) = self.ast.facets_mut(any) {
                facets.xtypes_mut().default_nested = Some(value);
            }
        }
    }

    /// The bounds OMG IDL 4.2 §7.4.13 puts on bit sets and bit masks.
    ///
    /// Seven sentences, and none of them held before this. The one that
    /// did hold is the lower one — `bitmask B { };` is refused at
    /// lowering — and the widest of these was caught only by the C
    /// writer, which is one whole backend past the clause that forbids
    /// it: a `bitfield<65>` emitted Rust and failed in C with "bitset
    /// wider than the sixty-four bits an anchor holds".
    ///
    /// Run from [`Self::complete`] rather than from the declaration,
    /// because `@bit_bound` and `@position` are annotations and are not
    /// applied until every declaration is in the model.
    fn check_bit_bounds(&mut self, types: &[Id<TypeNode>]) {
        for id in types {
            let Ok(node) = self.ast.type_(*id) else {
                continue;
            };
            match node.kind {
                TypeKind::Bitset { .. } => self.check_bitset_bounds(*id),
                TypeKind::Bitmask { .. } => self.check_bitmask_bounds(*id),
                TypeKind::Enumeration { .. } => self.check_enumeration_bound(*id),
                _ => {}
            }
        }
    }

    /// DDS-XTypes 1.3 §7.3.1.2.1.5: "the value member of the annotation
    /// may take any value from 1 to 32, inclusive, when this annotation is
    /// applied to an enumerated type."
    fn check_enumeration_bound(&mut self, id: Id<TypeNode>) {
        let Some(node) = self.ast.type_(id).ok() else {
            return;
        };
        let Some(written) = node.facets.xtypes.as_ref().and_then(|x| x.bit_bound) else {
            return;
        };
        let span = span_of(node.facets.source.span);
        if written == 0 || written > MAX_ENUM_BIT_BOUND {
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                span,
                &alloc::format!(
                    "`@bit_bound({written})` on an enumeration, and §7.3.1.2.1.5 says the \
                     value is from 1 to {MAX_ENUM_BIT_BOUND}"
                ),
                "DDS-XTypes 1.3 §7.3.1.2.1.5",
            );
        }
    }

    /// §7.4.13.4.3.2 — a bitfield's width, and the type that holds it.
    ///
    /// > The first one (`<positive_int_const>`) is the number of bits
    /// > that can be stored (its size). The maximum value is 64.
    ///
    /// > When the destination type is given, the number of stored bits
    /// > cannot exceed its size (i.e., 1 for boolean, 8 for octet, …).
    fn check_bitset_bounds(&mut self, id: Id<TypeNode>) {
        let fields: Vec<_> = self.ast.bitfields_of(id).to_vec();
        for field in fields {
            let Ok(node) = self.ast.bitfield(field) else {
                continue;
            };
            let bits = node.bits;
            let span = span_of(node.facets.source.span);
            let holder = node.holder_type.clone();
            if bits > MAX_BITFIELD_BITS {
                self.diagnose_at(
                    DiagnosticCode::InvalidBound,
                    span,
                    &alloc::format!(
                        "a bitfield stores {bits} bits, and §7.4.13.4.3.2 puts the maximum at \
                         {MAX_BITFIELD_BITS}"
                    ),
                    "OMG IDL 4.2 §7.4.13.4.3.2",
                );
                continue;
            }
            // `bitfield<n>` with no destination type has no second bound
            // to exceed.
            let Some(width) = holder.and_then(|holder| self.holder_width(&holder)) else {
                continue;
            };
            if bits > width {
                self.diagnose_at(
                    DiagnosticCode::InvalidBound,
                    span,
                    &alloc::format!(
                        "a bitfield stores {bits} bits in a destination type that holds \
                         {width}; §7.4.13.4.3.2 says the number of stored bits cannot exceed \
                         the destination's size"
                    ),
                    "OMG IDL 4.2 §7.4.13.4.3.2",
                );
            }
        }
    }

    /// How many bits a bitfield's declared destination type holds.
    ///
    /// `None` where the reference is not resolved or is not one of the
    /// integral types a bitfield may be stored in — saying nothing,
    /// rather than assuming a width and refusing over it.
    fn holder_width(&self, holder: &zerodds_ast_core::Ref<TypeNode>) -> Option<u16> {
        let zerodds_ast_core::Ref::Resolved(id) = holder else {
            return None;
        };
        let TypeKind::Primitive(primitive) = self.ast.type_(*id).ok()?.kind else {
            return None;
        };
        // The widths §7.4.13.4.3.2 names: "1 for boolean, 8 for octet,
        // …".
        Some(match primitive {
            zerodds_ast::PrimitiveKind::Boolean => 1,
            zerodds_ast::PrimitiveKind::Octet
            | zerodds_ast::PrimitiveKind::Int8
            | zerodds_ast::PrimitiveKind::UInt8 => 8,
            zerodds_ast::PrimitiveKind::Int16 | zerodds_ast::PrimitiveKind::UInt16 => 16,
            zerodds_ast::PrimitiveKind::Int32 | zerodds_ast::PrimitiveKind::UInt32 => 32,
            zerodds_ast::PrimitiveKind::Int64 | zerodds_ast::PrimitiveKind::UInt64 => 64,
            _ => return None,
        })
    }

    /// §7.4.13.4.3.3 — a bit mask's size, its count of values, and where
    /// each one may sit.
    ///
    /// > A bit mask must contain at least one value and no more than its
    /// > size expressed in bits.
    ///
    /// > By default, the size of a bit mask is 32.
    ///
    /// `@bit_bound` "must be lower than or equal to 64. Accordingly the
    /// number of values cannot then exceed the value given to
    /// @bit_bound"; `@position` — "Possible positions range from 0 …
    /// up to (size – 1)".
    fn check_bitmask_bounds(&mut self, id: Id<TypeNode>) {
        let bound = self.ast.effective_bit_bound(id);
        let declared = self
            .ast
            .type_(id)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .and_then(|x| x.bit_bound);
        let span = self
            .ast
            .type_(id)
            .ok()
            .map_or_else(|| span_of(None), |node| span_of(node.facets.source.span));

        if let Some(written) = declared {
            if written == 0 || written > MAX_BIT_BOUND {
                self.diagnose_at(
                    DiagnosticCode::InvalidBound,
                    span,
                    &alloc::format!(
                        "`@bit_bound({written})` on a bitmask, and §7.4.13.4.3.3 says it \
                         must be lower than or equal to {MAX_BIT_BOUND}"
                    ),
                    "OMG IDL 4.2 §7.4.13.4.3.3",
                );
                // Every count and position below would be measured
                // against a bound the clause does not admit.
                return;
            }
        }

        let bits: Vec<_> = self.ast.bits_of(id).to_vec();
        let count = bits.len();
        if count > usize::from(bound) {
            let how = if declared.is_some() {
                "the `@bit_bound` it was given"
            } else {
                "the default size of 32"
            };
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                span,
                &alloc::format!(
                    "the bitmask has {count} values and a size of {bound} bits; \
                     §7.4.13.4.3.3 says a bit mask holds no more values than {how}"
                ),
                "OMG IDL 4.2 §7.4.13.4.3.3",
            );
        }

        for bit in bits {
            let Ok(node) = self.ast.bit_value(bit) else {
                continue;
            };
            let Some(position) = node.position else {
                continue;
            };
            if position >= bound {
                let name = node.name.clone();
                let bit_span = span_of(node.facets.source.span);
                self.diagnose_at(
                    DiagnosticCode::InvalidBound,
                    bit_span,
                    &alloc::format!(
                        "`@position({position})` on `{name}` in a bitmask of {bound} bits; \
                         §7.4.13.4.3.3 puts the possible positions at 0 up to {}",
                        bound - 1
                    ),
                    "OMG IDL 4.2 §7.4.13.4.3.3",
                );
            }
        }
    }

    /// `#pragma DCPS_DATA_TYPE` and `#pragma DCPS_DATA_KEY`.
    fn apply_open_dds_pragmas(&mut self) -> Vec<KeyPath> {
        let mut paths = Vec::new();
        for name in core::mem::take(&mut self.open_dds_types) {
            let scoped = scoped_from_pragma(&name);
            let Ok(any) = self.ast.find_by_scoped_name(&scoped) else {
                self.unresolved_directive(Some(&scoped), "#pragma DCPS_DATA_TYPE");
                continue;
            };
            if let Some(facets) = self.ast.facets_mut(any) {
                facets
                    .vendor
                    .open_dds
                    .get_or_insert_with(Default::default)
                    .dcps_data_type = true;
            }
        }
        for (name, fields, span) in core::mem::take(&mut self.open_dds_keys) {
            let scoped = scoped_from_pragma(&name);
            let Ok(any) = self.ast.find_by_scoped_name(&scoped) else {
                self.unresolved_directive(Some(&scoped), "#pragma DCPS_DATA_KEY");
                continue;
            };
            if let Some(facets) = self.ast.facets_mut(any) {
                facets
                    .vendor
                    .open_dds
                    .get_or_insert_with(Default::default)
                    .dcps_data_keys
                    .extend(fields.iter().cloned());
            }
            // The facet keeps what was written; the member is what makes the
            // directive key anything (`Facets::is_key` reads `xtypes.is_key`).
            let Some(type_) = any.as_type() else {
                // `opendds_idl` refuses a DCPS_DATA_KEY on something that is
                // not a type; keying nothing without a word would build a
                // keyless topic from a file that asked for a key.
                let message = alloc::format!(
                    "`#pragma DCPS_DATA_KEY {name}` names something that is not a type; \
                     the directive keys nothing"
                );
                self.diagnose_at(
                    DiagnosticCode::Unsupported,
                    span,
                    &message,
                    "OMG DDS 1.4 §2.2.1.2.2",
                );
                continue;
            };
            if fields.is_empty() {
                let message = alloc::format!(
                    "`#pragma DCPS_DATA_KEY {name}` names no member; the directive keys nothing"
                );
                self.diagnose_at(
                    DiagnosticCode::Unsupported,
                    span,
                    &message,
                    "OMG DDS 1.4 §2.2.1.2.2",
                );
                continue;
            }
            for field in &fields {
                let directive = KeyDirective {
                    text: alloc::format!("#pragma DCPS_DATA_KEY {name}"),
                    field: field.clone(),
                    span,
                };
                self.key_by_directive(type_, directive, &mut paths);
            }
        }
        paths
    }

    /// A pragma with no typed meaning, kept on the root module.
    ///
    /// Dropping it would lose exactly what a round trip back to the
    /// dialect that wrote it needs. Spec §4.6 of the AST.
    fn keep_unrecognised_pragmas(&mut self) {
        let pragmas = core::mem::take(&mut self.unrecognised_pragmas);
        if pragmas.is_empty() {
            return;
        }
        let root = self.ast.root_module();
        let Ok(node_id) = self.ast.module(root).map(|m| m.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Module(root, node_id)) {
            facets
                .vendor
                .rti
                .get_or_insert_with(Default::default)
                .unrecognised
                .extend(pragmas);
        }
    }

    /// `typeid` sets a declaration's repository id outright; `typeprefix`
    /// sets the prefix its derived id is built from. CORBA 3.3 §10.7.
    fn apply_repository_directives(&mut self) {
        for (candidates, text, is_prefix) in core::mem::take(&mut self.repository_directives) {
            let Some((name, any)) = self.first_that_resolves(&candidates) else {
                // A directive naming nothing is reported by validation,
                // not silently applied to whatever is nearest.
                self.unresolved_directive(
                    candidates.first(),
                    if is_prefix { "typeprefix" } else { "typeid" },
                );
                continue;
            };
            let Some(facets) = self.ast.facets_mut(any) else {
                continue;
            };
            let corba = facets.corba_mut();
            if is_prefix {
                self.prefixes.push((name, text));
            } else {
                corba.repository_id = Some(zerodds_ast_core::RepositoryId(text));
                corba.repository_id_origin = Some(RepositoryIdOrigin::PragmaId);
            }
        }
    }

    /// The first of a directive's candidate scopes that names something.
    ///
    /// §7.5's lookup, applied after the walk rather than during it: the
    /// candidates are innermost first, so a sibling declaration wins
    /// over one further out, which is the rule for every other name.
    fn first_that_resolves(&self, candidates: &[ScopedName]) -> Option<(ScopedName, AnyId)> {
        candidates.iter().find_map(|name| {
            self.ast
                .find_by_scoped_name(name)
                .ok()
                .map(|any| (name.clone(), any))
        })
    }

    /// `keylist T (a, b);` and `#pragma keylist T a b` — RTI's and
    /// Cyclone's spelling of `@key`.
    ///
    /// Two things happen, and only one of them used to. The list is kept
    /// on the vendor facet, which is what a round trip back to that
    /// dialect needs. And **the named members become keys**, which is
    /// what the directive means.
    ///
    /// Without the second, the facet was written and nobody read it:
    /// `Facets::is_key` answers from `xtypes.is_key` alone, and the only
    /// other reader of `vendor.rti.keylist` in the tree is the snapshot
    /// serialiser. A Cyclone `.idl` therefore produced a type with no
    /// key at all — every sample in one instance, a constant KeyHash,
    /// and nothing anywhere reporting it.
    ///
    /// A field the type does not declare is reported rather than
    /// ignored: `#pragma keylist Sample id` naming a member that is not
    /// there is a typo in the directive, and silently keying nothing is
    /// the same failure as before.
    fn apply_keylists(&mut self) -> Vec<KeyPath> {
        let mut paths = Vec::new();
        for (candidates, fields, span) in core::mem::take(&mut self.keylists) {
            let Some((name, any)) = self.first_that_resolves(&candidates) else {
                self.unresolved_directive(candidates.first(), "keylist");
                continue;
            };
            if let Some(facets) = self.ast.facets_mut(any) {
                facets
                    .vendor
                    .rti
                    .get_or_insert_with(Default::default)
                    .keylist = fields.clone();
            }
            let Some(type_) = any.as_type() else {
                continue;
            };
            let written = name.segments().join("::");
            for field in &fields {
                let directive = KeyDirective {
                    text: alloc::format!("keylist {written}"),
                    field: field.clone(),
                    span,
                };
                self.key_by_directive(type_, directive, &mut paths);
            }
        }
        paths
    }

    /// Makes the member one key directive names a key, or says why it
    /// cannot. Shared by `keylist`, `#pragma keylist` and OpenDDS's
    /// `#pragma DCPS_DATA_KEY`.
    ///
    /// The name is looked up among the members the type declares **and
    /// inherits**. An inherited one is accepted only as a restatement of a
    /// key the base already has: DDS-XTypes 1.3 §7.2.2.4.5 gives a derived
    /// structure no key fields of its own, so its keys are its root
    /// base's. Cyclone DDS 11.0.1 accepts `#pragma keylist Derived id` on a
    /// base member that is no key (measured, key `parent.id`), which is
    /// the derived structure defining a key; before, this front end
    /// reported the member as not declared at all.
    ///
    /// A path `a.b` is not resolved here but handed back: whether `b` may
    /// key the structure `a` has depends on every other directive's keys,
    /// so [`Self::apply_key_paths`] resolves all paths at once, after every
    /// plain key is in place.
    fn key_by_directive(
        &mut self,
        type_: Id<TypeNode>,
        directive: KeyDirective,
        paths: &mut Vec<KeyPath>,
    ) {
        const SPEC: &str = "OMG DDS 1.4 §2.2.1.2.2";
        let field = directive.field.as_str();
        let text = directive.text.as_str();
        if field.contains('[') {
            // OpenDDS keys an array element this way. DDS-XTypes 1.3 keys
            // members, never elements (§7.6.8), so no key holder and no
            // TypeObject can say it; keying the whole array would make
            // every element part of the key — another type.
            let message = alloc::format!(
                "`{text} {field}` names an element of a collection; a key is a member, and \
                 no member here says which element — the directive keys nothing"
            );
            self.diagnose_at(DiagnosticCode::Unsupported, directive.span, &message, SPEC);
            return;
        }
        let segments: Vec<String> = field.split('.').map(String::from).collect();
        if segments.iter().any(String::is_empty) {
            let message =
                alloc::format!("`{text} {field}` is not a member name or a path of member names");
            self.diagnose_at(
                DiagnosticCode::UnresolvedName,
                directive.span,
                &message,
                SPEC,
            );
            return;
        }
        let Some(member) = self.member_named(type_, &segments[0]) else {
            let message = alloc::format!(
                "`{text}` names `{}`, which the type neither declares nor inherits; the \
                 directive keys nothing",
                segments[0]
            );
            self.diagnose_at(
                DiagnosticCode::UnresolvedName,
                directive.span,
                &message,
                SPEC,
            );
            return;
        };
        if !self.ast.members_of(type_).contains(&member) {
            if segments.len() == 1 && self.member_is_key(member) {
                return;
            }
            let owner = self
                .declaring_base(type_, member)
                .and_then(|b| self.ast.type_(b).ok().map(|n| n.name.clone()))
                .unwrap_or_default();
            let message = alloc::format!(
                "`{text} {field}` makes `{}`, which the type inherits from `{owner}`, a key; a \
                 derived structure defines no key fields, its keys are its root base's — the \
                 key belongs on `{owner}`",
                segments[0]
            );
            self.diagnose_at(
                DiagnosticCode::InvalidInheritance,
                directive.span,
                &message,
                "DDS-XTypes 1.3 §7.2.2.4.5",
            );
            return;
        }
        if segments.len() == 1 {
            self.mark_key(member);
            return;
        }
        paths.push(KeyPath {
            owner: type_,
            member,
            rest: segments[1..].to_vec(),
            directive,
        });
    }

    /// The member called `name` that `type_` declares or inherits.
    fn member_named(&self, type_: Id<TypeNode>, name: &str) -> Option<Id<zerodds_ast::MemberNode>> {
        self.ast
            .all_members_of(type_)
            .iter()
            .copied()
            .find(|id| self.ast.member(*id).is_ok_and(|node| node.name == name))
    }

    /// The structure in `type_`'s base chain that declares `member`.
    fn declaring_base(
        &self,
        type_: Id<TypeNode>,
        member: Id<zerodds_ast::MemberNode>,
    ) -> Option<Id<TypeNode>> {
        let mut current = self.ast.base_of(type_);
        while let Some(base) = current {
            if self.ast.members_of(base).contains(&member) {
                return Some(base);
            }
            current = self.ast.base_of(base);
        }
        None
    }

    fn member_is_key(&self, member: Id<zerodds_ast::MemberNode>) -> bool {
        self.ast.member(member).is_ok_and(|node| {
            self.ast
                .facets(AnyId::Member(member, node.node_id))
                .and_then(|f| f.xtypes.as_ref())
                .is_some_and(|x| x.is_key)
        })
    }

    fn mark_key(&mut self, member: Id<zerodds_ast::MemberNode>) {
        let Ok(node_id) = self.ast.member(member).map(|node| node.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Member(member, node_id)) {
            facets.xtypes.get_or_insert_with(Default::default).is_key = true;
        }
    }

    /// Resolves every key path (`a.b`) once all plain keys are in place.
    ///
    /// A path keys a member of a nested structure: `#pragma keylist Outer
    /// inner.b` makes `inner` a key of `Outer` and `b` a key of the
    /// structure `inner` has. That is how DDS-XTypes 1.3 §7.6.8 builds the
    /// key holder of a key member of structure type — from that
    /// structure's own key members — so the path becomes `@key` on each
    /// member along it, and the TypeObject and the KeyHash say what the
    /// path says. OpenSplice 6.9's `idlpp` keeps `"inner.b"` as its key
    /// list (measured); Cyclone 11.0.1 refuses paths ("Unknown key") and
    /// OpenDDS writes them in `DCPS_DATA_KEY`.
    ///
    /// One structure has one set of key members, and a path cannot give it
    /// a second. A path is refused when the nested structure already keys
    /// itself differently, when two paths ask it for different key sets,
    /// and when a member elsewhere keys by the whole structure — whose key
    /// the path would otherwise shrink to the path's members without a
    /// word.
    fn apply_key_paths(&mut self, paths: Vec<KeyPath>) {
        const SPEC: &str = "DDS-XTypes 1.3 §7.6.8";
        // Resolve each path to the members along it, level by level.
        struct Resolved {
            directive: KeyDirective,
            owner_member: Id<zerodds_ast::MemberNode>,
            // (structure, member of it) below the owner, outermost first.
            levels: Vec<(Id<TypeNode>, Id<zerodds_ast::MemberNode>)>,
        }
        let mut resolved: Vec<Resolved> = Vec::new();
        'paths: for path in paths {
            let text = path.directive.text.clone();
            let field = path.directive.field.clone();
            let mut levels = Vec::new();
            let mut through = path.member;
            for segment in &path.rest {
                let nested = self
                    .ast
                    .member(through)
                    .ok()
                    .and_then(|node| node.type_.resolved())
                    .and_then(|t| self.through_aliases(t))
                    .filter(|t| {
                        matches!(
                            self.ast.type_(*t).map(|n| &n.kind),
                            Ok(TypeKind::Structure { .. })
                        )
                    });
                let Some(nested) = nested else {
                    let name = self
                        .ast
                        .member(through)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    let message = alloc::format!(
                        "`{text} {field}` goes through `{name}`, which is not a structure; a \
                         path can only name a member of a nested structure"
                    );
                    self.diagnose_at(
                        DiagnosticCode::Unsupported,
                        path.directive.span,
                        &message,
                        SPEC,
                    );
                    continue 'paths;
                };
                let Some(member) = self.member_named(nested, segment) else {
                    let type_name = self
                        .ast
                        .type_(nested)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    let message = alloc::format!(
                        "`{text} {field}` names `{segment}`, which `{type_name}` neither \
                         declares nor inherits; the directive keys nothing"
                    );
                    self.diagnose_at(
                        DiagnosticCode::UnresolvedName,
                        path.directive.span,
                        &message,
                        SPEC,
                    );
                    continue 'paths;
                };
                levels.push((nested, member));
                through = member;
            }
            let _ = path.owner;
            resolved.push(Resolved {
                directive: path.directive,
                owner_member: path.member,
                levels,
            });
        }

        // What each nested structure is asked for: per directive and per
        // member leading into it, the set of its members to key. Paths of
        // one directive into one member group together (`inner.a inner.b`).
        // A set of members to key, and the path (index into `resolved`)
        // that first asked for it.
        type Request = (MemberSet, usize);
        let mut asked: alloc::collections::BTreeMap<Id<TypeNode>, Vec<Request>> =
            alloc::collections::BTreeMap::new();
        // Keyed by directive and the members leading into the structure.
        type Lead = (usize, Vec<Id<zerodds_ast::MemberNode>>);
        let mut groups: alloc::collections::BTreeMap<Lead, (Id<TypeNode>, MemberSet, usize)> =
            alloc::collections::BTreeMap::new();
        let directive_index = |r: &Resolved| r.directive.span.start as usize;
        for (index, r) in resolved.iter().enumerate() {
            let mut lead = alloc::vec![r.owner_member];
            for (nested, member) in &r.levels {
                let entry = groups
                    .entry((directive_index(r), lead.clone()))
                    .or_insert_with(|| (*nested, Default::default(), index));
                entry.1.insert(*member);
                lead.push(*member);
            }
        }
        for (nested, set, index) in groups.into_values() {
            asked.entry(nested).or_default().push((set, index));
        }

        // A nested structure's own keys, as they stood before any path.
        let mut refused: Vec<usize> = Vec::new();
        let mut clashes: Vec<(zerodds_idl_cst::Span, String)> = Vec::new();
        for (nested, requests) in &asked {
            let own: alloc::collections::BTreeSet<_> = self
                .ast
                .all_members_of(*nested)
                .iter()
                .copied()
                .filter(|m| self.member_is_key(*m))
                .collect();
            let type_name = self
                .ast
                .type_(*nested)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            let names = |set: &MemberSet| {
                set.iter()
                    .filter_map(|m| self.ast.member(*m).ok().map(|n| n.name.clone()))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let first = &requests[0].0;
            for (set, index) in requests {
                let clash = if !own.is_empty() && *set != own {
                    Some(alloc::format!(
                        "`{type_name}` keys itself by {}",
                        names(&own)
                    ))
                } else if set != first {
                    Some(alloc::format!(
                        "another path keys `{type_name}` by {}",
                        names(first)
                    ))
                } else {
                    None
                };
                if let Some(clash) = clash {
                    let directive = &resolved[*index].directive;
                    let message = alloc::format!(
                        "`{} {}` would key `{type_name}` by {}, but {clash}; one structure \
                         has one set of key members, which every key member of its type uses",
                        directive.text,
                        directive.field,
                        names(set)
                    );
                    clashes.push((directive.span, message));
                    refused.push(*index);
                }
            }
        }
        for (span, message) in clashes {
            self.diagnose_at(DiagnosticCode::ConflictingAnnotations, span, &message, SPEC);
        }

        // A member elsewhere that keys by a whole structure a path is about
        // to give keys to: its key would shrink to the path's members.
        let path_members: alloc::collections::BTreeSet<_> = resolved
            .iter()
            .flat_map(|r| core::iter::once(r.owner_member).chain(r.levels.iter().map(|(_, m)| *m)))
            .collect();
        let keyless_targets: Vec<Id<TypeNode>> = asked
            .keys()
            .copied()
            .filter(|t| {
                !self
                    .ast
                    .all_members_of(*t)
                    .iter()
                    .any(|m| self.member_is_key(*m))
            })
            .collect();
        let all_types: Vec<Id<TypeNode>> = self.ast.types().map(|(id, _)| id).collect();
        for holder in all_types {
            for member in self.ast.members_of(holder).to_vec() {
                if path_members.contains(&member) || !self.member_is_key(member) {
                    continue;
                }
                let Some(target) = self
                    .ast
                    .member(member)
                    .ok()
                    .and_then(|node| node.type_.resolved())
                    .and_then(|t| self.through_aliases(t))
                else {
                    continue;
                };
                if !keyless_targets.contains(&target) {
                    continue;
                }
                let holder_name = self
                    .ast
                    .type_(holder)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                let member_name = self
                    .ast
                    .member(member)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                let target_name = self
                    .ast
                    .type_(target)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                for (index, r) in resolved.iter().enumerate() {
                    if refused.contains(&index) || !r.levels.iter().any(|(t, _)| *t == target) {
                        continue;
                    }
                    let message = alloc::format!(
                        "`{} {}` would give `{target_name}` key members, and `{holder_name}::\
                         {member_name}` keys by the whole `{target_name}`; its key would shrink \
                         to the path's members",
                        r.directive.text,
                        r.directive.field
                    );
                    self.diagnose_at(
                        DiagnosticCode::ConflictingAnnotations,
                        r.directive.span,
                        &message,
                        SPEC,
                    );
                    refused.push(index);
                }
            }
        }

        for (index, r) in resolved.iter().enumerate() {
            if refused.contains(&index) {
                continue;
            }
            self.mark_key(r.owner_member);
            for (_, member) in &r.levels {
                self.mark_key(*member);
            }
        }
    }

    /// Every named declaration gets a repository id, derived from its
    /// scoped name unless a `typeid` gave it one. CORBA 3.3 §10.7.
    ///
    /// **Interfaces and exceptions too, and they did not used to.** This
    /// walked `self.ast.types()` and nothing else, so a `typeprefix`, a
    /// `#pragma prefix` or a `#pragma version` reached a `struct` and
    /// never an `interface` or an `exception` — the two declarations
    /// whose repository id is on the GIOP wire in every request header
    /// and every user-exception reply body. `typeprefix CosNaming
    /// "omg.org";` gave `IDL:CosNaming/NotFound:1.0` where omniORB, TAO
    /// and JacORB all write `IDL:omg.org/CosNaming/NotFound:1.0`, and a
    /// client raising that exception at any of the three would have had
    /// its `raises` arm not match.
    ///
    /// It was invisible because the one generator that needed the
    /// answer computed it a second time out of its own parse tree, with
    /// its own `typeprefix` table. That generator is over the model
    /// now.
    fn derive_repository_ids(&mut self, types: &[Id<TypeNode>]) {
        let mut targets: Vec<AnyId> = Vec::new();
        for id in types.iter().copied() {
            if let Ok(node) = self.ast.type_(id)
                && !node.name.is_empty()
            {
                targets.push(AnyId::Type(id, node.node_id));
            }
        }
        for (id, node) in self.ast.interfaces() {
            if !node.name.is_empty() {
                targets.push(AnyId::Interface(id, node.node_id));
            }
        }
        for (id, node) in self.ast.exceptions() {
            if !node.name.is_empty() {
                targets.push(AnyId::Exception(id, node.node_id));
            }
        }
        for any in targets {
            let declared = self
                .ast
                .facets(any)
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|c| c.repository_id.clone());
            if declared.is_some() {
                continue;
            }
            let Some(scoped) = self.ast.scoped_name_of(any) else {
                continue;
            };
            let (id_text, origin) = self.derived_repository_id(&scoped);
            if let Some(facets) = self.ast.facets_mut(any) {
                let corba = facets.corba_mut();
                corba.repository_id = Some(zerodds_ast_core::RepositoryId(id_text));
                corba.repository_id_origin = Some(origin);
            }
        }
    }

    /// `IDL:<prefix/><path>:1.0`, with the longest matching prefix.
    fn derived_repository_id(&self, scoped: &ScopedName) -> (String, RepositoryIdOrigin) {
        let path = scoped.segments().join("/");
        // `#pragma version X 1.2` replaces the version a derived id ends
        // in. CORBA 3.3 §10.7.
        let version = self
            .pragma_versions
            .iter()
            .find(|(name, _)| name.segments().last() == scoped.segments().last())
            .map_or("1.0", |(_, version)| version.as_str());

        let prefix = self
            .prefixes
            .iter()
            .chain(self.pragma_prefixes.iter())
            .filter(|(name, _)| starts_with(scoped, name))
            .max_by_key(|(name, _)| name.segments().len());
        match prefix {
            Some((_, text)) => (
                alloc::format!("IDL:{text}/{path}:{version}"),
                RepositoryIdOrigin::PragmaPrefix,
            ),
            None if version == "1.0" => (
                alloc::format!("IDL:{path}:1.0"),
                RepositoryIdOrigin::Derived,
            ),
            None => (
                alloc::format!("IDL:{path}:{version}"),
                RepositoryIdOrigin::PragmaVersion,
            ),
        }
    }

    fn unresolved_directive(&mut self, name: Option<&ScopedName>, what: &str) {
        let written = name.map_or_else(
            || alloc::string::String::from("nothing"),
            |name| name.segments().join("::"),
        );
        let message =
            alloc::format!("`{what}` names `{written}`, which is not declared in this file");
        self.diagnose_at(
            DiagnosticCode::UnresolvedName,
            zerodds_idl_cst::Span::new(0, 0),
            &message,
            "CORBA 3.3 §10.7",
        );
    }

    /// DDS-RPC 1.0 §7.5.1.2.1.1: "When present, the structure shall be
    /// interpreted as if the structure had the
    /// @Extensibility(MUTABLE_EXTENSIBILITY) annotation and all the
    /// members of the structure had the @Optional annotation."
    ///
    /// Written into the facets, where the consumers of `@mutable` and
    /// `@optional` read them. A structure that says another extensibility
    /// as well contradicts the clause and is reported.
    fn complete_choice(&mut self, id: Id<TypeNode>) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        if !matches!(node.kind, TypeKind::Structure { .. })
            || !node
                .facets
                .annotations
                .iter()
                .any(|applied| applied.name == "Choice")
        {
            return;
        }
        let any = AnyId::Type(id, node.node_id);
        let span = span_of(node.facets.source.span);
        let declared = node.facets.xtypes.as_ref().and_then(|x| x.extensibility);
        match declared {
            Some(zerodds_ast::Extensibility::Mutable) => {}
            Some(_) => self.diagnose_at(
                DiagnosticCode::InvalidAnnotation,
                span,
                "a `@Choice` structure is `@mutable`; it cannot be given another extensibility",
                "DDS-RPC 1.0 §7.5.1.2.1.1",
            ),
            None => {
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.xtypes_mut().extensibility = Some(zerodds_ast::Extensibility::Mutable);
                }
            }
        }
        for member in self.ast.members_of(id).to_vec() {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            if let Some(facets) = self.ast.facets_mut(member_any) {
                facets.xtypes_mut().is_optional = true;
            }
        }
    }

    fn complete_type_defaults(&mut self, id: Id<TypeNode>) {
        self.complete_choice(id);
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let node_id = node.node_id;
        let any = AnyId::Type(id, node_id);
        let is_aggregate = matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
        );

        // A `#pragma` that set the extensibility is something the source
        // said, so it is written into the facet. The rule's own default
        // is not: leaving it absent keeps "the source said appendable"
        // apart from "the source said nothing", and
        // `Ast::effective_extensibility` applies the rule for whoever
        // asks. The same holds for bit bounds, enumerator values and bit
        // positions. Spec §5.
        //
        // `Options::default_extensibility` is written in for the same
        // reason the pragma is, one step later: the pragma is the source
        // speaking, the option is the caller speaking, and the source
        // wins where both do. XTypes 1.3 §7.3.1.2.1.8 provides for the
        // option — "IDL compilers may provide configuration options to
        // allow users to specify whether types of unspecified
        // extensibility are to be considered final, appendable, or
        // mutable" — and a caller who asks for it wants every consumer
        // to see the answer, not each to apply its own default.
        //
        // Not into a structure that derives from another: without a word
        // of its own it has its base's extensibility (DDS-XTypes 1.3
        // §7.2.2.4.5), which `Ast::effective_extensibility` follows. The
        // caller's default belongs at the root of the chain; written into
        // each link it would give a derived structure an extensibility its
        // base does not have.
        let derives = matches!(node.kind, TypeKind::Structure { base: Some(_), .. });
        let filled = self.pragma_extensibility.or(if is_aggregate && !derives {
            self.options.default_extensibility
        } else {
            None
        });
        if let Some(extensibility) = filled {
            let declared = self
                .ast
                .facets(any)
                .and_then(|facets| facets.xtypes.as_ref())
                .and_then(|x| x.extensibility);
            if declared.is_none() {
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.xtypes_mut().extensibility = Some(extensibility);
                }
            }
        }

        if is_aggregate {
            self.member_ids(id);
        }
    }

    /// The types, every base ahead of what derives from it.
    ///
    /// Sequential member ids continue across an inheritance boundary, so
    /// a derived type's ids cannot be worked out before its base's are.
    fn base_first(&self, types: &[Id<TypeNode>]) -> Vec<Id<TypeNode>> {
        let mut out: Vec<Id<TypeNode>> = Vec::with_capacity(types.len());
        for id in types.iter().copied() {
            self.push_base_first(id, &mut out, 0);
        }
        out
    }

    fn push_base_first(&self, id: Id<TypeNode>, out: &mut Vec<Id<TypeNode>>, depth: usize) {
        // An inheritance cycle is invalid IDL and is reported by
        // validation; the depth cap is what keeps this pass finite until
        // it gets there.
        if depth > MAX_INHERITANCE_DEPTH || out.contains(&id) {
            return;
        }
        if let Some(base) = self.ast.base_of(id) {
            self.push_base_first(base, out, depth + 1);
        }
        out.push(id);
    }

    /// The id of the last member the base chain contributes.
    ///
    /// "In the case an aggregated type inherits from another aggregated
    /// type the first member of the derived type gets the member ID of
    /// the last member in the base type plus one. In other words, it
    /// shall behave as if the base type members appeared at the
    /// beginning of the derived type." XTypes 1.3 §7.3.1.2.1.1.
    ///
    /// Measured: Cyclone 0.11 and RTI Connext 7.7 both write EMHEADER
    /// ids 0, 1, 2, 3 for `@mutable struct Base { long base_a; short
    /// base_b; }; @mutable struct Sample : Base { double derived_a;
    /// octet derived_b; };` — the derived members start at 2, not at 0.
    fn last_base_member_id(&self, id: Id<TypeNode>) -> Option<u32> {
        let base = self.ast.base_of(id)?;
        let last = *self.ast.all_members_of(base).last()?;
        let node_id = self.ast.member(last).ok()?.node_id;
        self.ast
            .facets(AnyId::Member(last, node_id))
            .and_then(|facets| facets.xtypes.as_ref())
            .and_then(|x| x.member_id.as_ref())
            .map(|assignment| assignment.effective)
    }

    /// Member ids: `@id(n)`, else `@hashid`, else the aggregate's
    /// `@autoid`, else the previous member plus one, starting at 0 —
    /// or, for a derived type, at the base's last id plus one.
    /// Spec §5, XTypes 1.3 §7.3.1.2.1.1.
    fn member_ids(&mut self, id: Id<TypeNode>) {
        // The type's own `@autoid`, else the nearest enclosing module's
        // (DDS-XTypes 1.3 §7.3.1.2.1.1, Table 21).
        let autoid = self.ast.effective_autoid(id);

        let members: Vec<_> = self.ast.members_of(id).to_vec();
        let mut previous: Option<u32> = self.last_base_member_id(id);
        for member in members {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let name = node.name.clone();
            let declared = self
                .ast
                .facets(member_any)
                .and_then(|facets| facets.xtypes.as_ref())
                .and_then(|x| x.member_id.clone());
            // Under `AsWritten` a hash whose input is the member's own
            // name reads the spelling the file used — `_long` where the
            // stored name is `long`. An explicit `@hashid("name")`
            // parameter is already a spelling and passes unchanged.
            let escaped = self.options.hashed_name_spelling == HashedNameSpelling::AsWritten
                && self
                    .ast
                    .facets(member_any)
                    .and_then(|facets| facets.xtypes.as_ref())
                    .is_some_and(|x| x.escaped);
            let hash_of = |source: &str| {
                if escaped && source == name {
                    hash_id(&alloc::format!("_{source}"))
                } else {
                    hash_id(source)
                }
            };

            let assignment = match declared {
                // `@id(n)` decides outright.
                Some(assignment) if assignment.origin == MemberIdOrigin::Declared => assignment,
                // `@hashid` decides the name to hash; the number is the
                // hash of that name, computed here.
                Some(MemberIdAssignment {
                    origin: MemberIdOrigin::Hashed { source_name },
                    ..
                }) => MemberIdAssignment {
                    effective: hash_of(&source_name),
                    origin: MemberIdOrigin::Hashed { source_name },
                },
                // Anything else is derived, and the aggregate's @autoid
                // says how.
                _ => match autoid {
                    Some(AutoId::Hash) => MemberIdAssignment {
                        effective: hash_of(&name),
                        origin: MemberIdOrigin::AutoHash,
                    },
                    Some(AutoId::Sequential) => MemberIdAssignment {
                        effective: next_id(previous),
                        origin: MemberIdOrigin::AutoSequential,
                    },
                    None => MemberIdAssignment {
                        effective: next_id(previous),
                        origin: MemberIdOrigin::PreviousPlusOne,
                    },
                },
            };

            // Whether a hashed id counts towards the next member's.
            // §7.3.1.2.1.1 says it twice and not the same way — see
            // `Options::hashed_id_advances_the_counter`. Cyclone and
            // Fast DDS advance; RTI does not, and skips the hashed id
            // entirely rather than resetting to zero, so `previous`
            // keeps whatever the last declared or sequential id was.
            let hashed = matches!(
                assignment.origin,
                MemberIdOrigin::Hashed { .. } | MemberIdOrigin::AutoHash
            );
            if !hashed || self.options.hashed_id_advances_the_counter == HashedIdCounter::Advances {
                previous = Some(assignment.effective);
            }
            if let Some(facets) = self.ast.facets_mut(member_any) {
                facets.xtypes_mut().member_id = Some(assignment);
            }
        }
    }
}

/// How far an inheritance chain is walked before it is taken for a
/// cycle, matching `Ast::all_members_of`.
const MAX_INHERITANCE_DEPTH: usize = 64;

/// The next sequential id after `previous`.
fn next_id(previous: Option<u32>) -> u32 {
    previous.map_or(0, |last| last.saturating_add(1))
}

/// The member id `@hashid` gives a name.
///
/// DDS-XTypes 1.3 §7.3.1.2.1.1, verbatim: "1. Compute a 4-byte hash of
/// the string as specified in 7.2.2.4.4.4.5. 2. Interpret the resulting
/// 4-byte has as a Little Endian unsigned 32-bit integer. 3. Perform a
/// bitwise AND operation with the integer 0x0FFFFFFF to zero the most
/// significant 4-bits of the integer." §7.2.2.4.4.4.5 makes the 4-byte
/// hash "the first 4 bytes of the MD5 hash of the UTF-8 encoding of the
/// string, without any 'nul' terminator".
///
/// The little-endian step is the one that is easy to get backwards, and
/// the clause's two worked examples are the check: `color` gives
/// 0x0FA5DD70 and `shapesize` gives 0x047790DA. Reading the same four
/// bytes big-endian gives 0x00DDA5DF and 0x0A907714, which is what this
/// function returned until the derivation was measured against Cyclone
/// DDS, RTI Connext and Fast DDS — all three of which agree with the
/// clause. `crates/ast-layout/tests/fixtures/vendor_bytes.md`.
fn hash_id(name: &str) -> u32 {
    let digest = md5(name.as_bytes());
    let leading = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    leading & 0x0FFF_FFFF
}

/// MD5, as XTypes specifies it for member id hashing.
///
/// Not a security primitive here and not used as one: the specification
/// names MD5 for this, and a different digest would produce ids that no
/// other vendor agrees with.
#[allow(clippy::many_single_char_names)]
fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76a_a478,
        0xe8c7_b756,
        0x2420_70db,
        0xc1bd_ceee,
        0xf57c_0faf,
        0x4787_c62a,
        0xa830_4613,
        0xfd46_9501,
        0x6980_98d8,
        0x8b44_f7af,
        0xffff_5bb1,
        0x895c_d7be,
        0x6b90_1122,
        0xfd98_7193,
        0xa679_438e,
        0x49b4_0821,
        0xf61e_2562,
        0xc040_b340,
        0x265e_5a51,
        0xe9b6_c7aa,
        0xd62f_105d,
        0x0244_1453,
        0xd8a1_e681,
        0xe7d3_fbc8,
        0x21e1_cde6,
        0xc337_07d6,
        0xf4d5_0d87,
        0x455a_14ed,
        0xa9e3_e905,
        0xfcef_a3f8,
        0x676f_02d9,
        0x8d2a_4c8a,
        0xfffa_3942,
        0x8771_f681,
        0x6d9d_6122,
        0xfde5_380c,
        0xa4be_ea44,
        0x4bde_cfa9,
        0xf6bb_4b60,
        0xbebf_bc70,
        0x289b_7ec6,
        0xeaa1_27fa,
        0xd4ef_3085,
        0x0488_1d05,
        0xd9d4_d039,
        0xe6db_99e5,
        0x1fa2_7cf8,
        0xc4ac_5665,
        0xf429_2244,
        0x432a_ff97,
        0xab94_23a7,
        0xfc93_a039,
        0x655b_59c3,
        0x8f0c_cc92,
        0xffef_f47d,
        0x8584_5dd1,
        0x6fa8_7e4f,
        0xfe2c_e6e0,
        0xa301_4314,
        0x4e08_11a1,
        0xf753_7e82,
        0xbd3a_f235,
        0x2ad7_d2bb,
        0xeb86_d391,
    ];

    let mut message = input.to_vec();
    let bit_length = (input.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_le_bytes());

    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    for chunk in message.chunks_exact(64) {
        let mut words = [0u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let at = index * 4;
            *word = u32::from_le_bytes([chunk[at], chunk[at + 1], chunk[at + 2], chunk[at + 3]]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(words[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

/// The hash a name gets, for a consumer outside this module.
#[must_use]
pub fn hashed_member_id(name: &str) -> u32 {
    hash_id(name)
}

/// The MD5 digest of a byte string, as hexadecimal.
///
/// Exposed so the digest can be checked against the published vectors
/// rather than against itself.
#[must_use]
pub fn md5_hex(input: &[u8]) -> String {
    let digest = md5(input);
    let mut out = String::new();
    for byte in digest {
        out.push(hex_digit(byte >> 4));
        out.push(hex_digit(byte & 0x0F));
    }
    out
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        _ => char::from(b'a' + value - 10),
    }
}

/// The scoped name a pragma's argument stands for.
///
/// A pragma writes `A::B` where the rest of the grammar writes segments.
fn scoped_from_pragma(name: &str) -> ScopedName {
    ScopedName::absolute(
        name.split("::")
            .filter(|segment| !segment.is_empty())
            .map(alloc::string::ToString::to_string)
            .collect(),
    )
}

/// Whether `name` lies inside the scope `prefix` names.
fn starts_with(name: &ScopedName, prefix: &ScopedName) -> bool {
    let segments = name.segments();
    let head = prefix.segments();
    head.len() < segments.len() && segments.starts_with(head)
}

/// The widest bitfield §7.4.13.4.3.2 admits.
const MAX_BITFIELD_BITS: u16 = 64;

/// The largest `@bit_bound` §7.4.13.4.3.3 admits on a bitmask.
const MAX_BIT_BOUND: u16 = 64;

/// The largest `@bit_bound` DDS-XTypes 1.3 §7.3.1.2.1.5 admits on an
/// enumeration.
const MAX_ENUM_BIT_BOUND: u16 = 32;

/// A node's source range, or an empty one where the model has none.
fn span_of(span: Option<zerodds_ast::ByteSpan>) -> zerodds_idl_cst::Span {
    span.map_or_else(
        || zerodds_idl_cst::Span::new(0, 0),
        |span| zerodds_idl_cst::Span::new(span.start, span.end),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    /// RFC 1321 appendix A.5 publishes these digests; checking against
    /// them is what makes this MD5 rather than something that merely
    /// agrees with itself.
    #[test]
    fn the_digest_matches_the_published_vectors() {
        for (input, expected) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            assert_eq!(md5_hex(input.as_bytes()), expected, "{input:?}");
        }
    }

    #[test]
    fn a_hashed_id_stays_inside_the_28_bit_space() {
        for name in ["a", "member", "a_rather_longer_member_name"] {
            assert_eq!(hash_id(name) & 0xF000_0000, 0, "{name}");
        }
    }

    #[test]
    fn different_names_hash_differently() {
        assert_ne!(hash_id("a"), hash_id("b"));
    }

    /// DDS-XTypes 1.3 §7.3.1.2.1.1 works both of these out by hand in
    /// its Example 2, so they are the specification's own answer and not
    /// this project's. Cyclone DDS, RTI Connext 7.7 and Fast DDS all
    /// three write these ids into the EMHEADER as well —
    /// `crates/ast-layout/tests/fixtures/vendor_bytes.md`.
    #[test]
    fn the_clause_works_two_hashes_out_by_hand() {
        assert_eq!(hash_id("color"), 0x0FA5_DD70);
        assert_eq!(hash_id("shapesize"), 0x0477_90DA);
    }

    /// Masking to 28 bits can bring two distinct 4-byte hashes together:
    /// `m9852` and `m12254` differ only in the low nibble of the third
    /// digest byte, which the little-endian reading puts in the high
    /// nibble the mask clears. A reading that keeps them apart is
    /// reading the digest the wrong way round.
    #[test]
    fn two_names_can_share_an_id_after_the_mask() {
        assert_eq!(hash_id("m9852"), 0x0BE1_7DB5);
        assert_eq!(hash_id("m12254"), 0x0BE1_7DB5);
        assert_ne!(md5_hex(b"m9852")[..8], md5_hex(b"m12254")[..8]);
    }
}
