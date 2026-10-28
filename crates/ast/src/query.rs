// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Query and traversal. Spec §6.

use crate::facet::Extensibility;
use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{
    Id, IndexLookup, NodeId, RepositoryId, RosTypeName, ScopedName, TypeIdentifierKey, VisitSet,
    WalkOrder, walk_graph,
};

/// How far an inheritance chain is walked before it is taken for a
/// cycle. Deep enough for any real hierarchy, finite for a broken one.
const MAX_INHERITANCE_DEPTH: usize = 64;

impl Ast {
    /// Find any node by its full scoped name.
    ///
    /// # Errors
    /// [`IndexLookup::Missing`] when nothing carries the name,
    /// [`IndexLookup::Ambiguous`] with every candidate when several do.
    pub fn find_by_scoped_name(&self, name: &ScopedName) -> Result<AnyId, IndexLookup> {
        let node = self.by_scoped_name.unique(name)?;
        self.resolve_node_id(node).ok_or(IndexLookup::Missing)
    }

    /// Find a type by its XTypes type identifier.
    ///
    /// # Errors
    /// As [`Ast::find_by_scoped_name`].
    pub fn find_by_type_identifier(
        &self,
        key: &TypeIdentifierKey,
    ) -> Result<Id<crate::TypeNode>, IndexLookup> {
        let node = self.by_type_identifier.unique(key)?;
        self.resolve_node_id(node)
            .and_then(AnyId::as_type)
            .ok_or(IndexLookup::Missing)
    }

    /// Find a type by its ROS 2 interface name.
    ///
    /// # Errors
    /// As [`Ast::find_by_scoped_name`].
    pub fn find_by_ros_name(&self, name: &RosTypeName) -> Result<Id<crate::TypeNode>, IndexLookup> {
        let node = self.by_ros_name.unique(name)?;
        self.resolve_node_id(node)
            .and_then(AnyId::as_type)
            .ok_or(IndexLookup::Missing)
    }

    /// Find any node by its CORBA repository identifier.
    ///
    /// # Errors
    /// As [`Ast::find_by_scoped_name`].
    pub fn find_by_repository_id(&self, id: &RepositoryId) -> Result<AnyId, IndexLookup> {
        let node = self.by_repository_id.unique(id)?;
        self.resolve_node_id(node).ok_or(IndexLookup::Missing)
    }

    /// The full scoped name of a node, if it has one.
    ///
    /// Nodes that live inside a type rather than a module — members,
    /// cases, parameters — have no scoped name of their own.
    #[must_use]
    pub fn scoped_name_of(&self, any: AnyId) -> Option<ScopedName> {
        // A declaration inside an interface or a value type is named
        // through it: `module CosNaming { interface NamingContext {
        // exception NotFound …` is `CosNaming::NamingContext::NotFound`.
        let mut scope: Option<String> = None;
        let (module, name) = match any {
            AnyId::Type(id, _) => {
                let node = self.types.get(id).ok()?;
                scope.clone_from(&node.scope);
                (Some(node.module), node.name.clone())
            }
            AnyId::Interface(id, _) => {
                let node = self.interfaces.get(id).ok()?;
                (Some(node.module), node.name.clone())
            }
            AnyId::Exception(id, _) => {
                let node = self.exceptions.get(id).ok()?;
                scope.clone_from(&node.scope);
                (Some(node.module), node.name.clone())
            }
            AnyId::Const(id, _) => {
                let node = self.constants.get(id).ok()?;
                scope.clone_from(&node.scope);
                (Some(node.module), node.name.clone())
            }
            AnyId::AnnotationDef(id, _) => {
                let node = self.annotation_defs.get(id).ok()?;
                (Some(node.module), node.name.clone())
            }
            AnyId::Module(id, _) => {
                let node = self.modules.get(id).ok()?;
                (node.parent, node.name.clone())
            }
            // An enumerator is named from the scope enclosing its
            // enumeration, not through it. OMG IDL 4.2 §7.4.1.4.2 spells
            // this out with `module M { enum Size { small, medium, large
            // }; }; const M::Size MYSIZE = M::medium;` — `M::medium`,
            // never `M::Size::medium`.
            AnyId::Enumerator(id, _) => {
                let node = self.enumerators.get(id).ok()?;
                let enumeration = self.types.get(node.parent).ok()?;
                (Some(enumeration.module), node.name.clone())
            }
            _ => return None,
        };

        let mut segments: Vec<String> = Vec::new();
        let mut current = module;
        while let Some(id) = current {
            let node = self.modules.get(id).ok()?;
            if !node.name.is_empty() {
                segments.push(node.name.clone());
            }
            current = node.parent;
        }
        segments.reverse();
        segments.extend(scope);
        segments.push(name);
        Some(ScopedName::absolute(segments))
    }

    /// The members of an aggregate, empty for every other kind.
    #[must_use]
    pub fn members_of(&self, type_: Id<crate::TypeNode>) -> &[Id<crate::MemberNode>] {
        self.types.get(type_).map_or(&[], |t| t.kind.members())
    }

    /// The members of an aggregate, its bases' members first.
    ///
    /// [`Ast::members_of`] answers what a type declares; this answers
    /// what it *has*, which is what every wire consumer needs.
    /// DDS-XTypes 1.3 §7.4.1.1.5.1: "The members defined by the base
    /// type, if any, shall be serialized before the members of their
    /// derived types. The representation shall be exactly as if all of
    /// the members had been defined, in the same order, in the
    /// most-derived type."
    ///
    /// The chain is walked to a fixed depth and a base already seen is
    /// not walked again: an inheritance cycle is invalid IDL, and
    /// looping here would hang instead of reporting it.
    #[must_use]
    pub fn all_members_of(&self, type_: Id<crate::TypeNode>) -> Vec<Id<crate::MemberNode>> {
        let mut chain: Vec<Id<crate::TypeNode>> = Vec::new();
        let mut current = Some(type_);
        while let Some(id) = current {
            if chain.contains(&id) || chain.len() >= MAX_INHERITANCE_DEPTH {
                break;
            }
            chain.push(id);
            current = self.base_of(id);
        }
        // Walked derived-to-base, written base-to-derived.
        let mut out = Vec::new();
        for id in chain.into_iter().rev() {
            out.extend_from_slice(self.members_of(id));
        }
        out
    }

    /// The base an aggregate extends, if it has one.
    ///
    /// A valuetype may name several; only a single base is a structure
    /// on the wire, so the first is what the layout follows.
    ///
    /// A bitset counts: IDL 4.2 §7.4.13.4.3.2 gives `bitset` "an
    /// optional single inheritance specification", and the inherited
    /// bitfields are part of what the derived bitset packs.
    /// [`Ast::members_of`] is empty for a bitset either way, so
    /// [`Ast::all_members_of`] is unaffected by the extra arm.
    #[must_use]
    pub fn base_of(&self, type_: Id<crate::TypeNode>) -> Option<Id<crate::TypeNode>> {
        match self.types.get(type_).map(|t| &t.kind) {
            Ok(TypeKind::Structure { base, .. } | TypeKind::Bitset { base, .. }) => {
                base.as_ref().and_then(|b| b.resolved())
            }
            Ok(TypeKind::Valuetype { bases, .. }) => bases.first().and_then(|b| b.resolved()),
            _ => None,
        }
    }

    /// The bitfields a bitset declares, empty for every other kind.
    #[must_use]
    pub fn bitfields_of(&self, type_: Id<crate::TypeNode>) -> &[Id<crate::BitfieldNode>] {
        match self.types.get(type_).map(|t| &t.kind) {
            Ok(TypeKind::Bitset { fields, .. }) => fields,
            _ => &[],
        }
    }

    /// The bitfields a bitset *has*, its bases' bitfields first.
    ///
    /// The bitset counterpart of [`Ast::all_members_of`], and needed for
    /// the same reason: a plan that leaves the inherited bitfields out
    /// sizes the anchor as if they did not exist, and every emitter
    /// behind it writes short.
    ///
    /// # Why base-first, when no specification says so
    ///
    /// DDS-XTypes 1.3 has no serialisation rule for a bitset — see the
    /// note above `wire_probe::encode::write_bitset`. The order is
    /// therefore taken from the one implementation that serialises the
    /// construct at all. Measured on 2026-08-09, eProsima Fast DDS
    /// 3.6.1 flattens `bitset B3 : B2 : B1` with `bitfield<8> a`,
    /// `bitfield<8> b`, `bitfield<4> c` into a `std::bitset<20>` and
    /// packs `a` at bits 0..8, `b` at 8..16 and `c` at 16..20 — the
    /// base's bitfields at the least significant end, in declaration
    /// order. Cyclone's `idlc` and RTI's `rtiddsgen` do not parse
    /// `bitset` at all, so Fast DDS is the only oracle there is.
    ///
    /// The chain is walked to the same fixed depth as
    /// [`Ast::all_members_of`], and a base already seen is not walked
    /// again: an inheritance cycle is invalid IDL, and looping here
    /// would hang instead of reporting it.
    #[must_use]
    pub fn all_bitfields_of(&self, type_: Id<crate::TypeNode>) -> Vec<Id<crate::BitfieldNode>> {
        let mut chain: Vec<Id<crate::TypeNode>> = Vec::new();
        let mut current = Some(type_);
        while let Some(id) = current {
            if chain.contains(&id) || chain.len() >= MAX_INHERITANCE_DEPTH {
                break;
            }
            chain.push(id);
            current = self.base_of(id);
        }
        // Walked derived-to-base, packed base-to-derived.
        let mut out = Vec::new();
        for id in chain.into_iter().rev() {
            out.extend_from_slice(self.bitfields_of(id));
        }
        out
    }

    /// The enumerators of an enumeration, empty for every other kind.
    #[must_use]
    pub fn enumerators_of(&self, type_: Id<crate::TypeNode>) -> &[Id<crate::EnumeratorNode>] {
        match self.types.get(type_).map(|t| &t.kind) {
            Ok(TypeKind::Enumeration { enumerators }) => enumerators,
            _ => &[],
        }
    }

    /// The bits of a bitmask, empty for every other kind.
    #[must_use]
    pub fn bits_of(&self, type_: Id<crate::TypeNode>) -> &[Id<crate::BitValueNode>] {
        match self.types.get(type_).map(|t| &t.kind) {
            Ok(TypeKind::Bitmask { bits }) => bits,
            _ => &[],
        }
    }

    /// The attributes an interface declares directly.
    #[must_use]
    pub fn attributes_of(
        &self,
        interface: Id<crate::InterfaceNode>,
    ) -> &[Id<crate::AttributeNode>] {
        self.interfaces
            .get(interface)
            .map_or(&[], |node| node.attributes.as_slice())
    }

    /// The extensibility a type has, written or defaulted.
    ///
    /// The facet keeps `None` for a type whose source said nothing, so
    /// what was written stays distinguishable from what the rule gives.
    /// This is the rule: `@final`/`@appendable`/`@mutable`, else
    /// `@extensibility(...)`, else — for a structure that derives from
    /// another — its base's, else appendable. DDS-XTypes 1.3 §7.2.4.4,
    /// §7.2.2.4.5.
    ///
    /// Appendable, not final: a consumer that assumed final would emit
    /// types that cannot evolve, and no DHEADER where one belongs.
    ///
    /// The base's, not appendable: §7.2.2.4.5 lets a derived structure
    /// leave its extensibility unspecified or give the base's, so an
    /// unspecified one has the base's. Cyclone DDS 11.0.1 frames
    /// `struct Derived : Base` of a `@mutable` base as mutable
    /// (`DDS_OP_PLC`, measured); falling back to appendable here framed it
    /// with a DHEADER and no EMHEADERs.
    #[must_use]
    pub fn effective_extensibility(&self, type_: Id<crate::TypeNode>) -> Extensibility {
        let mut current = type_;
        // Bounded by the number of types: a base chain visits each at most
        // once, and a cycle, which a valid model does not have, ends here
        // rather than looping.
        for _ in 0..=self.types.len() {
            let Ok(node) = self.types.get(current) else {
                break;
            };
            if let Some(extensibility) = node.facets.extensibility() {
                return extensibility;
            }
            let TypeKind::Structure {
                base: Some(base), ..
            } = &node.kind
            else {
                break;
            };
            let Some(base) = base.resolved() else {
                break;
            };
            current = base;
        }
        crate::DEFAULT_EXTENSIBILITY
    }

    /// The `@autoid` policy of a type: its own, else the nearest enclosing
    /// module's.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.1 and Table 21 apply `@autoid` to
    /// "Module declarations, Structure declarations, Union declarations";
    /// on a module it governs every aggregated type declared in it that
    /// does not carry its own. `None` is "nothing said", which numbers
    /// the members sequentially.
    #[must_use]
    pub fn effective_autoid(&self, type_: Id<crate::TypeNode>) -> Option<crate::AutoId> {
        let node = self.types.get(type_).ok()?;
        if let Some(own) = node.facets.xtypes.as_ref().and_then(|x| x.autoid) {
            return Some(own);
        }
        self.module_chain_value(node.module, |x| x.autoid)
    }

    /// The `@data_representation` mask of a type: its own, else the
    /// nearest enclosing module's.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.15: the annotation "may be applied to
    /// modules and to types"; a module's value is the default for the
    /// types in it. `None` is "nothing said", which §7.6.3.1.1 reads as
    /// every representation the implementation supports.
    #[must_use]
    pub fn effective_data_representation(
        &self,
        type_: Id<crate::TypeNode>,
    ) -> Option<crate::DataRepresentationMask> {
        let node = self.types.get(type_).ok()?;
        if let Some(own) = node
            .facets
            .xtypes
            .as_ref()
            .and_then(|x| x.data_representation)
        {
            return Some(own);
        }
        self.module_chain_value(node.module, |x| x.data_representation)
    }

    /// The `DataRepresentationMask` a type has, with the annotation-less
    /// case resolved: a type and its modules that carry no
    /// `@data_representation` are "interpreted as if the
    /// DataRepresentationMask value was set to 0xffffffff" (DDS-XTypes
    /// 1.3 §7.3.1.2.1.15), which is [`crate::DataRepresentationMask::ALL`].
    #[must_use]
    pub fn data_representation_mask(
        &self,
        type_: Id<crate::TypeNode>,
    ) -> crate::DataRepresentationMask {
        self.effective_data_representation(type_)
            .unwrap_or(crate::DataRepresentationMask::ALL)
    }

    /// The first value `read` finds on a module or the modules around it.
    fn module_chain_value<T>(
        &self,
        start: Id<crate::ModuleNode>,
        read: impl Fn(&crate::XTypesFacet) -> Option<T>,
    ) -> Option<T> {
        let mut module = Some(start);
        for _ in 0..crate::node::constant::MAX_EXPR_DEPTH {
            let id = module?;
            let m = self.modules.get(id).ok()?;
            if let Some(found) = m.facets.xtypes.as_ref().and_then(&read) {
                return Some(found);
            }
            module = m.parent;
        }
        None
    }

    /// The bit bound an enum or bitmask has, written or defaulted.
    /// DDS-XTypes 1.3 §7.3.1.2.1.9.
    #[must_use]
    pub fn effective_bit_bound(&self, type_: Id<crate::TypeNode>) -> u16 {
        self.types
            .get(type_)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .and_then(|x| x.bit_bound)
            .unwrap_or(crate::DEFAULT_BIT_BOUND)
    }

    /// The `try_construct` behaviour that governs one *use* of a type.
    ///
    /// `written` is the annotation at the use site — a member's own
    /// `@try_construct`, or the collection's `element_try_construct` /
    /// `key_try_construct`, or the union's
    /// `discriminator_try_construct`. `used` is the type being used
    /// there.
    ///
    /// The order is: what the use site wrote, else what the alias chain
    /// of the used type declares nearest-first, else
    /// `TryConstruct::ABSENT` — `DISCARD`, which §7.2.2.7 names as
    /// the default when the annotation is not used.
    ///
    /// # Why an alias may carry one
    ///
    /// The clause does not list aliases among the positions
    /// (§7.3.1.2.1.12 names members, the discriminator, array and
    /// sequence elements, and map keys and values), and both Cyclone and
    /// RTI refuse `@try_construct` on a `typedef` by name. Neither of
    /// those is a statement about what an alias-borne annotation would
    /// *mean*; they are front ends declining to accept it, and the two
    /// implementations that do accept it — Fast DDS and OpenDDS — say
    /// nothing about it either. With the specification silent, the
    /// reading here is the broader one: an alias states the behaviour
    /// that uses of it take by default, and any use site that writes its
    /// own overrides it. Nothing an author wrote can be lost that way,
    /// and `typedef string<5> String5` — the very declaration
    /// §7.3.1.2.1.12.4 builds its example on — becomes able to carry the
    /// behaviour it exists to give its uses.
    ///
    /// A peer built by Cyclone or RTI will not compile such a
    /// declaration at all;
    /// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`
    /// records that.
    #[must_use]
    pub fn effective_try_construct(
        &self,
        written: Option<crate::TryConstruct>,
        used: Option<Id<crate::TypeNode>>,
    ) -> crate::TryConstruct {
        if let Some(behaviour) = written {
            return behaviour;
        }
        let mut next = used;
        // Bounded like every other walk over a reference chain: a
        // `typedef` cycle would otherwise spin here rather than in the
        // validation rule that reports it.
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let Some(id) = next else { break };
            let Ok(node) = self.types.get(id) else { break };
            if let Some(behaviour) = node.facets.xtypes.as_ref().and_then(|x| x.try_construct) {
                return behaviour;
            }
            let TypeKind::Alias { aliased } = &node.kind else {
                break;
            };
            next = aliased.resolved();
        }
        crate::TryConstruct::ABSENT
    }

    /// The behaviour that governs the elements of one collection.
    ///
    /// `collection` is a sequence, an array or a map — for a map this is
    /// the **value** slot. `member_wrote` is the `@try_construct` on the
    /// member whose type this collection is, if any.
    ///
    /// # Why a member's annotation can reach an array's elements
    ///
    /// An array cannot be trimmed and cannot overflow: Table 17 requires
    /// `T1.bounds[] == T2.bounds[]`, so the only way an array fails to
    /// construct is that one of its elements does. Its row accordingly
    /// speaks of nothing but "the TryConstruct behavior associated with
    /// T1 element type" — an array has no member-level behaviour to
    /// carry. There is also no template syntax to annotate an array's
    /// element with: `@try_construct(TRIM) String5 m[3]` writes the
    /// annotation in front of the member, which is where OpenDDS reads
    /// its `array_element_try_construct` from as well.
    ///
    /// A sequence or a map has both, and they are different questions —
    /// the member's governs a length the receiver's bound cannot hold
    /// (§7.3.1.2.1.12.3), the element's governs one element that will
    /// not construct (§7.3.1.2.1.12.4) — so a member's annotation never
    /// reaches their elements.
    #[must_use]
    pub fn element_try_construct(
        &self,
        collection: Id<crate::TypeNode>,
        member_wrote: Option<crate::TryConstruct>,
    ) -> crate::TryConstruct {
        let Ok(node) = self.types.get(collection) else {
            return crate::TryConstruct::ABSENT;
        };
        let (element, written) = match (&node.kind, node.facets.xtypes.as_ref()) {
            (TypeKind::Array { element, .. }, xtypes) => (
                element.resolved(),
                xtypes
                    .and_then(|x| x.element_try_construct)
                    .or(member_wrote),
            ),
            (TypeKind::Sequence { element, .. }, xtypes) => (
                element.resolved(),
                xtypes.and_then(|x| x.element_try_construct),
            ),
            (TypeKind::Map { value, .. }, xtypes) => (
                value.resolved(),
                xtypes.and_then(|x| x.element_try_construct),
            ),
            _ => return crate::TryConstruct::ABSENT,
        };
        self.effective_try_construct(written, element)
    }

    /// The behaviour that governs a `map`'s keys.
    ///
    /// Table 17 gives a key no escape — a key that cannot construct
    /// fails the whole map — so this is `DISCARD` unless a declaration
    /// wrote `DISCARD` explicitly, which the front end is the only thing
    /// that admits. It reads the slot rather than returning a constant
    /// so that a future revision of the clause has one place to change.
    #[must_use]
    pub fn key_try_construct(&self, map: Id<crate::TypeNode>) -> crate::TryConstruct {
        let Ok(node) = self.types.get(map) else {
            return crate::TryConstruct::ABSENT;
        };
        let TypeKind::Map { key, .. } = &node.kind else {
            return crate::TryConstruct::ABSENT;
        };
        let written = node
            .facets
            .xtypes
            .as_ref()
            .and_then(|x| x.key_try_construct);
        self.effective_try_construct(written, key.resolved())
    }

    /// Whether a type is `@nested` — its own annotation, else the
    /// `@default_nested` of the nearest enclosing module that carries
    /// one.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.7: "@default_nested ... applied to a
    /// module ... indicates whether the types defined within it are
    /// nested by default ... value defaults to that of the enclosing
    /// module. If a top-level module is not annotated, the default is
    /// FALSE."
    ///
    /// A type's own `@nested` wins over the module's. `@default_nested`
    /// used to reach nothing at all: the annotation parsed, the model
    /// dropped it, and a whole module declared nested emitted
    /// `IS_NESTED`-free `TypeObject`s — which is a topic type per
    /// endpoint that the author said should be none.
    ///
    /// The walk is bounded by the module chain, which the model builds
    /// acyclically; the counter is there so a handle that somehow points
    /// at its own ancestor ends the loop rather than the process.
    #[must_use]
    pub fn effective_nested(&self, type_: Id<crate::TypeNode>) -> bool {
        let Ok(node) = self.types.get(type_) else {
            return false;
        };
        if let Some(x) = node.facets.xtypes.as_ref() {
            if x.is_nested {
                return true;
            }
            // §7.3.1.2.1.16: "The @topic annotation overrides the @nested
            // and @default_nested annotations." A type that is a topic is
            // not nested whatever its module says.
            if x.topic.is_some() {
                return false;
            }
            // `@nested(FALSE)` is the type speaking, and it wins over the
            // module's `@default_nested(TRUE)`.
            if x.nested_explicit {
                return false;
            }
        }
        let mut module = Some(node.module);
        for _ in 0..crate::node::constant::MAX_EXPR_DEPTH {
            let Some(id) = module else { break };
            let Ok(m) = self.modules.get(id) else { break };
            if let Some(written) = m.facets.xtypes.as_ref().and_then(|x| x.default_nested) {
                return written;
            }
            module = m.parent;
        }
        false
    }

    /// Whether the elements of a collection are `@external` — the
    /// sequence or array element, or the value of a map
    /// (DDS-XTypes 1.3 §7.3.1.2.1.4).
    #[must_use]
    pub fn element_is_external(&self, collection: Id<crate::TypeNode>) -> bool {
        self.types
            .get(collection)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .is_some_and(|x| x.element_is_external)
    }

    /// The user-declared annotations written on the element type of a
    /// collection (DDS-XTypes 1.3 §7.2.2.6), in the order they were
    /// written. For a map, the value's.
    #[must_use]
    pub fn element_annotations(
        &self,
        collection: Id<crate::TypeNode>,
    ) -> &[crate::AppliedAnnotation] {
        self.types
            .get(collection)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .map_or(&[], |x| x.element_annotations.as_slice())
    }

    /// The user-declared annotations written on the key type of a map.
    #[must_use]
    pub fn key_annotations(&self, map: Id<crate::TypeNode>) -> &[crate::AppliedAnnotation] {
        self.types
            .get(map)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .map_or(&[], |x| x.key_annotations.as_slice())
    }

    /// The user-declared annotations written on the discriminator type of
    /// a union (DDS-XTypes 1.3 §7.3.1.2.5).
    #[must_use]
    pub fn discriminator_annotations(
        &self,
        union_: Id<crate::TypeNode>,
    ) -> &[crate::AppliedAnnotation] {
        self.types
            .get(union_)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .map_or(&[], |x| x.discriminator_annotations.as_slice())
    }

    /// Whether the key of a map is `@external`.
    #[must_use]
    pub fn key_is_external(&self, map: Id<crate::TypeNode>) -> bool {
        self.types
            .get(map)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .is_some_and(|x| x.key_is_external)
    }

    /// The behaviour that governs a union's discriminator.
    /// DDS-XTypes 1.3 §7.3.1.2.1.12.5, Table 19.
    #[must_use]
    pub fn discriminator_try_construct(&self, union: Id<crate::TypeNode>) -> crate::TryConstruct {
        let Ok(node) = self.types.get(union) else {
            return crate::TryConstruct::ABSENT;
        };
        let TypeKind::Union { discriminator, .. } = &node.kind else {
            return crate::TryConstruct::ABSENT;
        };
        let written = node
            .facets
            .xtypes
            .as_ref()
            .and_then(|x| x.discriminator_try_construct);
        self.effective_try_construct(written, discriminator.resolved())
    }

    /// Whether a union's discriminator carries `@key`.
    ///
    /// DDS-XTypes 1.3 Table 21 lists `@key` against "Structure Members,
    /// Union discriminator" and §7.2.2.4.4.4.8's example is `union
    /// MyCommand switch (@key CommandKind)`. A union that answers true
    /// here is a keyed topic type whose key holder is the discriminator
    /// alone, and §7.2.4.4.4.5 — Table 19's `UNION_TYPE` row, "Either
    /// the discriminators of both T1 and T2 are keys or neither are
    /// keys" — makes it unassignable to a union that answers false.
    ///
    /// False for anything that is not a union, and false for a union
    /// whose front end was not asked to carry the annotation — see
    /// `zerodds_idl_lower::KeyOnUnionDiscriminator`.
    #[must_use]
    pub fn discriminator_is_key(&self, union: Id<crate::TypeNode>) -> bool {
        let Ok(node) = self.types.get(union) else {
            return false;
        };
        if !matches!(node.kind, TypeKind::Union { .. }) {
            return false;
        }
        node.facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.discriminator_is_key)
    }

    /// The value an enumerator has, written or positional.
    ///
    /// `@value(n)`, else the previous enumerator's value plus one,
    /// starting at 0. OMG IDL 4.2 §7.4.13.
    ///
    /// Returns `None` only when the handle is stale or an explicit
    /// expression does not evaluate — never a substituted 0, which would
    /// give two enumerators the same value without saying so.
    #[must_use]
    pub fn effective_enumerator_value(&self, id: Id<crate::EnumeratorNode>) -> Option<i128> {
        let node = self.enumerators.get(id).ok()?;
        let siblings = self.enumerators_of(node.parent);
        let mut previous: Option<i128> = None;
        for sibling in siblings {
            let current = self.enumerators.get(*sibling).ok()?;
            let value = match current.value {
                Some(expr) => self.eval_integral(expr).ok()?,
                None => previous.map_or(0, |last| last.saturating_add(1)),
            };
            if *sibling == id {
                return Some(value);
            }
            previous = Some(value);
        }
        None
    }

    /// The position a bitmask bit has, written or positional.
    /// DDS-XTypes 1.3 §7.3.1.2.1.10.
    #[must_use]
    pub fn effective_bit_position(&self, id: Id<crate::BitValueNode>) -> Option<u16> {
        let node = self.bit_values.get(id).ok()?;
        let siblings = self.bits_of(node.parent);
        let mut previous: Option<u16> = None;
        for sibling in siblings {
            let current = self.bit_values.get(*sibling).ok()?;
            let position = match current.position {
                Some(written) => written,
                None => previous.map_or(0, |last| last.saturating_add(1)),
            };
            if *sibling == id {
                return Some(position);
            }
            previous = Some(position);
        }
        None
    }

    /// The cases of a union, empty for every other kind.
    #[must_use]
    pub fn cases_of(&self, type_: Id<crate::TypeNode>) -> &[Id<crate::CaseNode>] {
        match self.types.get(type_).map(|t| &t.kind) {
            Ok(TypeKind::Union { cases, .. }) => cases,
            _ => &[],
        }
    }

    /// The operations an interface declares directly.
    ///
    /// **Not the whole answer.** `InterfaceNode.bases` exists and OMG
    /// IDL 4.2 §7.4.3 gives a derived interface every operation of its
    /// bases, so this list is short by exactly those. Only a caller that
    /// writes the base into the target language — a Java `extends`, a
    /// C++ base clause, a TypeScript `extends` — wants it; every caller
    /// that emits a flat declaration wants [`Ast::all_operations_of`],
    /// and one that took this instead emitted a type the source did not
    /// describe with nothing to report it.
    #[must_use]
    pub fn operations_of(
        &self,
        interface: Id<crate::InterfaceNode>,
    ) -> &[Id<crate::OperationNode>] {
        self.interfaces
            .get(interface)
            .map_or(&[], |i| &i.operations)
    }

    /// Every operation an interface has, its bases' before its own.
    ///
    /// [`Ast::operations_of`] answers what an interface *declares*;
    /// this answers what it *has*. OMG IDL 4.2 §7.4.3: a derived
    /// interface "inherits all the operations and attributes of its
    /// base interfaces", and a client holding a `Derived` may call
    /// every one of them. A generated binding that carries only the
    /// declared ones is a type the source did not describe, and nothing
    /// reports it — the operation is simply not there.
    ///
    /// Multiple inheritance is a graph and not a chain, so bases are
    /// walked in declaration order — the order §7.4.3 resolves an
    /// operation lookup along — and an interface reached twice through
    /// a diamond contributes its operations once. The depth bound and
    /// the seen list are [`Ast::all_members_of`]'s, for the same reason:
    /// an inheritance cycle is invalid IDL and looping here would hang
    /// instead of reporting it.
    #[must_use]
    pub fn all_operations_of(
        &self,
        interface: Id<crate::InterfaceNode>,
    ) -> Vec<Id<crate::OperationNode>> {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        self.collect_operations(interface, &mut seen, &mut out, 0);
        out
    }

    fn collect_operations(
        &self,
        interface: Id<crate::InterfaceNode>,
        seen: &mut Vec<Id<crate::InterfaceNode>>,
        out: &mut Vec<Id<crate::OperationNode>>,
        depth: usize,
    ) {
        if depth >= MAX_INHERITANCE_DEPTH || seen.contains(&interface) {
            return;
        }
        seen.push(interface);
        let Ok(node) = self.interfaces.get(interface) else {
            return;
        };
        for base in &node.bases {
            if let Some(base) = base.resolved() {
                self.collect_operations(base, seen, out, depth + 1);
            }
        }
        for operation in &node.operations {
            if !out.contains(operation) {
                out.push(*operation);
            }
        }
    }

    /// Every node reachable from `root`, itself included, in pre-order.
    ///
    /// Terminates on recursive types: a struct containing
    /// `sequence<Self>` appears once. Spec §6.
    #[must_use]
    pub fn descendants(&self, root: AnyId) -> Vec<AnyId> {
        let ids = walk_graph(
            &[root.node_id()],
            |node| self.successors_of(node),
            WalkOrder::PreOrder,
        );
        ids.into_iter()
            .filter_map(|id| self.resolve_node_id(id))
            .collect()
    }

    /// Every node the model holds, in identity order.
    pub fn all_nodes(&self) -> impl Iterator<Item = AnyId> + '_ {
        self.by_node_id.values().copied()
    }

    /// The nodes one node points at, for traversal.
    pub(crate) fn successors_of(&self, node: NodeId) -> Vec<NodeId> {
        let Some(any) = self.resolve_node_id(node) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match any {
            AnyId::Module(id, _) => {
                if let Ok(m) = self.modules.get(id) {
                    out.extend(m.children.iter().map(|c| c.node_id()));
                }
            }
            AnyId::Type(id, _) => {
                let Ok(t) = self.types.get(id) else {
                    return out;
                };
                for reference in t.kind.type_refs() {
                    if let Some(target) = reference.resolved() {
                        if let Ok(node) = self.types.get(target) {
                            out.push(node.node_id);
                        }
                    }
                }
                match &t.kind {
                    TypeKind::Structure { members, .. }
                    | TypeKind::Valuetype { state: members, .. } => {
                        for member in members {
                            if let Ok(m) = self.members.get(*member) {
                                out.push(m.node_id);
                            }
                        }
                    }
                    TypeKind::Union { cases, .. } => {
                        for case in cases {
                            if let Ok(c) = self.cases.get(*case) {
                                out.push(c.node_id);
                            }
                        }
                    }
                    TypeKind::Enumeration { enumerators } => {
                        for enumerator in enumerators {
                            if let Ok(e) = self.enumerators.get(*enumerator) {
                                out.push(e.node_id);
                            }
                        }
                    }
                    TypeKind::Bitmask { bits } => {
                        for bit in bits {
                            if let Ok(b) = self.bit_values.get(*bit) {
                                out.push(b.node_id);
                            }
                        }
                    }
                    TypeKind::Bitset { fields, .. } => {
                        for field in fields {
                            if let Ok(b) = self.bitfields.get(*field) {
                                out.push(b.node_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
            AnyId::Member(id, _) => {
                if let Ok(m) = self.members.get(id) {
                    if let Some(target) = m.type_.resolved() {
                        if let Ok(t) = self.types.get(target) {
                            out.push(t.node_id);
                        }
                    }
                }
            }
            AnyId::Case(id, _) => {
                if let Ok(c) = self.cases.get(id) {
                    if let Ok(m) = self.members.get(c.member) {
                        out.push(m.node_id);
                    }
                }
            }
            AnyId::Interface(id, _) => {
                if let Ok(i) = self.interfaces.get(id) {
                    for operation in &i.operations {
                        if let Ok(o) = self.operations.get(*operation) {
                            out.push(o.node_id);
                        }
                    }
                    for attribute in &i.attributes {
                        if let Ok(a) = self.attributes.get(*attribute) {
                            out.push(a.node_id);
                        }
                    }
                }
            }
            AnyId::Operation(id, _) => {
                if let Ok(o) = self.operations.get(id) {
                    for param in &o.params {
                        if let Ok(p) = self.params.get(*param) {
                            out.push(p.node_id);
                        }
                    }
                }
            }
            AnyId::Exception(id, _) => {
                if let Ok(e) = self.exceptions.get(id) {
                    for member in &e.members {
                        if let Ok(m) = self.members.get(*member) {
                            out.push(m.node_id);
                        }
                    }
                }
            }
            AnyId::Library(id, _) => {
                if let Ok(l) = self.libraries.get(id) {
                    out.extend(l.children.iter().map(|c| c.node_id()));
                }
            }
            AnyId::Domain(id, _) => {
                if let Ok(d) = self.domains.get(id) {
                    for topic in &d.topics {
                        if let Ok(t) = self.topics.get(*topic) {
                            out.push(t.node_id);
                        }
                    }
                }
            }
            AnyId::Participant(id, _) => {
                if let Ok(p) = self.participants.get(id) {
                    for endpoint in &p.endpoints {
                        if let Ok(e) = self.endpoints.get(*endpoint) {
                            out.push(e.node_id);
                        }
                    }
                }
            }
            AnyId::AnnotationDef(id, _) => {
                if let Ok(a) = self.annotation_defs.get(id) {
                    for member in &a.members {
                        if let Ok(m) = self.annotation_members.get(*member) {
                            out.push(m.node_id);
                        }
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// Every type reachable from `type_`, itself included.
    ///
    /// The layout cache uses this to know which cached plans a change
    /// invalidates. Spec §7.
    #[must_use]
    pub fn reachable_types(&self, type_: Id<crate::TypeNode>) -> VisitSet {
        let Ok(node) = self.types.get(type_) else {
            return VisitSet::new();
        };
        zerodds_ast_core::reachable(&[node.node_id], |id| self.successors_of(id))
    }

    /// Rebuild every index from the model's current content.
    ///
    /// Called after loading a snapshot; ordinary editing maintains the
    /// indices from the changeset instead. Spec §6, §9.
    pub fn reindex(&mut self) {
        self.by_scoped_name.clear();
        self.by_type_identifier.clear();
        self.by_ros_name.clear();
        self.by_repository_id.clear();

        let nodes: Vec<AnyId> = self.by_node_id.values().copied().collect();
        for any in nodes {
            self.index_node(any);
        }
    }

    /// Add one node to every index it belongs in.
    pub(crate) fn index_node(&mut self, any: AnyId) {
        let node = any.node_id();
        // The object-reference type derived from an interface carries the
        // interface's name so a backend can render it, but the name in
        // the scope belongs to the interface. Indexing both would make
        // every use of that name ambiguous.
        let derived_object_ref = match any {
            AnyId::Type(id, _) => self.types.get(id).is_ok_and(|node| {
                matches!(node.kind, crate::TypeKind::ObjectRef { interface: Some(_) })
            }),
            _ => false,
        };
        if let Some(name) = self.scoped_name_of(any).filter(|_| !derived_object_ref) {
            self.by_scoped_name.insert(name, node);
        }
        let Some(facets) = self.facets(any) else {
            return;
        };
        let ros_name = facets.ros2.as_ref().and_then(|r| r.ros_name.clone());
        let repository_id = facets.corba.as_ref().and_then(|c| c.repository_id.clone());
        if let Some(name) = ros_name {
            self.by_ros_name.insert(name, node);
        }
        if let Some(id) = repository_id {
            self.by_repository_id.insert(id, node);
        }
    }

    /// Remove one node from every index.
    pub(crate) fn unindex_node(&mut self, node: NodeId) {
        self.by_scoped_name.remove_node(node);
        self.by_type_identifier.remove_node(node);
        self.by_ros_name.remove_node(node);
        self.by_repository_id.remove_node(node);
    }

    /// Register a type identifier for a type.
    ///
    /// The identifier is computed by the XTypes runtime, not here; the
    /// model only indexes it. Spec §1.2.
    ///
    /// # Errors
    /// [`crate::AstError::Core`] with a stale handle.
    pub fn set_type_identifier(
        &mut self,
        type_: Id<crate::TypeNode>,
        key: TypeIdentifierKey,
    ) -> Result<(), crate::AstError> {
        let node = self.types.get(type_)?.node_id;
        self.by_type_identifier.insert(key, node);
        Ok(())
    }
}
