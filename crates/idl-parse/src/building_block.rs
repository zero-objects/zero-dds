// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The building blocks of OMG IDL 4.2, their lattice, and the profiles of
//! clause 9 that combine them.
//!
//! OMG IDL 4.2 §7.1: "the rules are grouped into atomic building blocks"
//! which "are only used in a profile". Every rule of the consolidated
//! grammar (Annex A, rules 1 to 227) belongs to exactly one block; a
//! construct is in a profile when the block of every rule it is written
//! with is. [`blocks_used`] reads a parsed file and names, for every
//! construct in it, the block that admits it; [`outside_profile`] reports
//! the ones a profile does not include.
//!
//! §9.1 calls the profiles "not normative for the related middleware
//! solutions"; checking a file against one is a choice of the caller
//! (`zerodds-idlc --idl-profile`), not something every parse does.

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_idl_cst::{CstKind, Span, SyntaxNode, TokenKind};

/// One building block of OMG IDL 4.2 §7.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuildingBlock {
    /// §7.4.1, rules (1) to (68).
    CoreDataTypes,
    /// §7.4.2, rules (69) and (70).
    Any,
    /// §7.4.3, rules (71) to (96).
    InterfacesBasic,
    /// §7.4.4, rule (97).
    InterfacesFull,
    /// §7.4.5, rules (98) to (110).
    ValueTypes,
    /// §7.4.6, rules (111) to (124).
    CorbaSpecificInterfaces,
    /// §7.4.7, rules (125) to (132).
    CorbaSpecificValueTypes,
    /// §7.4.8, rules (133) to (143).
    ComponentsBasic,
    /// §7.4.9, rules (144) to (152).
    ComponentsHomes,
    /// §7.4.10, rules (153) to (170).
    CcmSpecific,
    /// §7.4.11, rules (171) to (183).
    ComponentsPortsAndConnectors,
    /// §7.4.12, rules (184) to (194).
    TemplateModules,
    /// §7.4.13, rules (195) to (215).
    ExtendedDataTypes,
    /// §7.4.14, rules (216) and (217).
    AnonymousTypes,
    /// §7.4.15, rules (218) to (227).
    Annotations,
}

impl BuildingBlock {
    /// Every building block, in the order of §7.4.
    pub const ALL: [Self; 15] = [
        Self::CoreDataTypes,
        Self::Any,
        Self::InterfacesBasic,
        Self::InterfacesFull,
        Self::ValueTypes,
        Self::CorbaSpecificInterfaces,
        Self::CorbaSpecificValueTypes,
        Self::ComponentsBasic,
        Self::ComponentsHomes,
        Self::CcmSpecific,
        Self::ComponentsPortsAndConnectors,
        Self::TemplateModules,
        Self::ExtendedDataTypes,
        Self::AnonymousTypes,
        Self::Annotations,
    ];

    /// The name the specification gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CoreDataTypes => "Core Data Types",
            Self::Any => "Any",
            Self::InterfacesBasic => "Interfaces – Basic",
            Self::InterfacesFull => "Interfaces – Full",
            Self::ValueTypes => "Value Types",
            Self::CorbaSpecificInterfaces => "CORBA-Specific – Interfaces",
            Self::CorbaSpecificValueTypes => "CORBA-Specific – Value Types",
            Self::ComponentsBasic => "Components – Basic",
            Self::ComponentsHomes => "Components – Homes",
            Self::CcmSpecific => "CCM-Specific",
            Self::ComponentsPortsAndConnectors => "Components – Ports and Connectors",
            Self::TemplateModules => "Template Modules",
            Self::ExtendedDataTypes => "Extended Data-Types",
            Self::AnonymousTypes => "Anonymous Types",
            Self::Annotations => "Annotations",
        }
    }

    /// The clause that defines it.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::CoreDataTypes => "§7.4.1",
            Self::Any => "§7.4.2",
            Self::InterfacesBasic => "§7.4.3",
            Self::InterfacesFull => "§7.4.4",
            Self::ValueTypes => "§7.4.5",
            Self::CorbaSpecificInterfaces => "§7.4.6",
            Self::CorbaSpecificValueTypes => "§7.4.7",
            Self::ComponentsBasic => "§7.4.8",
            Self::ComponentsHomes => "§7.4.9",
            Self::CcmSpecific => "§7.4.10",
            Self::ComponentsPortsAndConnectors => "§7.4.11",
            Self::TemplateModules => "§7.4.12",
            Self::ExtendedDataTypes => "§7.4.13",
            Self::AnonymousTypes => "§7.4.14",
            Self::Annotations => "§7.4.15",
        }
    }

    /// The first and last rule of Annex A the block contributes.
    #[must_use]
    pub const fn rules(self) -> (u16, u16) {
        match self {
            Self::CoreDataTypes => (1, 68),
            Self::Any => (69, 70),
            Self::InterfacesBasic => (71, 96),
            Self::InterfacesFull => (97, 97),
            Self::ValueTypes => (98, 110),
            Self::CorbaSpecificInterfaces => (111, 124),
            Self::CorbaSpecificValueTypes => (125, 132),
            Self::ComponentsBasic => (133, 143),
            Self::ComponentsHomes => (144, 152),
            Self::CcmSpecific => (153, 170),
            Self::ComponentsPortsAndConnectors => (171, 183),
            Self::TemplateModules => (184, 194),
            Self::ExtendedDataTypes => (195, 215),
            Self::AnonymousTypes => (216, 217),
            Self::Annotations => (218, 227),
        }
    }

    /// The block of a rule of Annex A, `None` outside 1 to 227.
    #[must_use]
    pub fn of_rule(rule: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|block| {
            let (first, last) = block.rules();
            (first..=last).contains(&rule)
        })
    }

    /// The blocks this one directly relies on: the edges of the lattice
    /// of Figure 7-2, as each block's "Dependencies with other Building
    /// Blocks" subclause states them.
    #[must_use]
    pub const fn relies_on(self) -> &'static [Self] {
        match self {
            // §7.4.1.2: "the root for all other building blocks and
            // requires no other ones."
            Self::CoreDataTypes => &[],
            // §7.4.2.2, §7.4.3.2, §7.4.12.2, §7.4.13.2, §7.4.14.2,
            // §7.4.15.2: each relies on Core Data Types only.
            Self::Any
            | Self::InterfacesBasic
            | Self::TemplateModules
            | Self::ExtendedDataTypes
            | Self::AnonymousTypes
            | Self::Annotations => &[Self::CoreDataTypes],
            // §7.4.4.2: "complements Building Block Interfaces – Basic".
            // §7.4.5.2: "requires Building Block Interfaces – Basic".
            // §7.4.8.2: "complements Building Block Interfaces – Basic".
            Self::InterfacesFull | Self::ValueTypes | Self::ComponentsBasic => {
                &[Self::InterfacesBasic]
            }
            // §7.4.6.2: "based on Building Block Interfaces – Full".
            Self::CorbaSpecificInterfaces => &[Self::InterfacesFull],
            // §7.4.7.2: "based on Building Block Value Types and
            // complements Building Block CORBA-Specific – Interfaces".
            Self::CorbaSpecificValueTypes => &[Self::ValueTypes, Self::CorbaSpecificInterfaces],
            // §7.4.9.2: "complements Building Block Components – Basic".
            // §7.4.11.2: "relies on the Building Block Components – Basic".
            Self::ComponentsHomes | Self::ComponentsPortsAndConnectors => &[Self::ComponentsBasic],
            // §7.4.10.2: "relies on Building Block Components – Basic and
            // Building Block CORBA-Specific – Value Types".
            Self::CcmSpecific => &[Self::ComponentsBasic, Self::CorbaSpecificValueTypes],
        }
    }

    /// This block and every block it relies on, directly or
    /// transitively — what the "Transitively, it relies on" sentences of
    /// §7.4.x.2 list.
    #[must_use]
    pub fn closure(self) -> Vec<Self> {
        let mut out = alloc::vec![self];
        let mut index = 0;
        while index < out.len() {
            for next in out[index].relies_on() {
                if !out.contains(next) {
                    out.push(*next);
                }
            }
            index += 1;
        }
        out.sort();
        out
    }
}

/// A group of standardized annotations, OMG IDL 4.2 §8.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnnotationGroup {
    /// §8.3.1.
    GeneralPurpose,
    /// §8.3.2.
    DataModeling,
    /// §8.3.3.
    UnitsAndRanges,
    /// §8.3.4.
    DataImplementation,
    /// §8.3.5.
    CodeGeneration,
    /// §8.3.6.
    Interfaces,
}

impl AnnotationGroup {
    /// Every group, in the order of §8.3.
    pub const ALL: [Self; 6] = [
        Self::GeneralPurpose,
        Self::DataModeling,
        Self::UnitsAndRanges,
        Self::DataImplementation,
        Self::CodeGeneration,
        Self::Interfaces,
    ];

    /// The name the specification gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GeneralPurpose => "General Purpose",
            Self::DataModeling => "Data Modeling",
            Self::UnitsAndRanges => "Units and Ranges",
            Self::DataImplementation => "Data Implementation",
            Self::CodeGeneration => "Code Generation",
            Self::Interfaces => "Interfaces",
        }
    }

    /// The annotations the group defines, as §8.3.x lists them.
    #[must_use]
    pub const fn annotations(self) -> &'static [&'static str] {
        match self {
            Self::GeneralPurpose => &[
                "id",
                "autoid",
                "optional",
                "position",
                "value",
                "extensibility",
                "final",
                "appendable",
                "mutable",
            ],
            Self::DataModeling => &["key", "must_understand", "default_literal"],
            Self::UnitsAndRanges => &["default", "range", "min", "max", "unit"],
            Self::DataImplementation => &["bit_bound", "external", "nested"],
            Self::CodeGeneration => &["verbatim"],
            Self::Interfaces => &["service", "oneway", "ami"],
        }
    }

    /// The group that standardizes an annotation, if one does.
    #[must_use]
    pub fn of_annotation(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|group| group.annotations().contains(&name))
    }
}

/// A profile of OMG IDL 4.2 clause 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdlProfile {
    /// §9.2.1.
    PlainCorba,
    /// §9.2.2.
    MinimumCorba,
    /// §9.2.3.
    Ccm,
    /// §9.2.4.
    CcmWithGenericInteractionSupport,
    /// §9.3.1.
    PlainDds,
    /// §9.3.2.
    ExtensibleDds,
    /// §9.3.3.
    RpcOverDds,
}

impl IdlProfile {
    /// Every profile, in the order of clause 9.
    pub const ALL: [Self; 7] = [
        Self::PlainCorba,
        Self::MinimumCorba,
        Self::Ccm,
        Self::CcmWithGenericInteractionSupport,
        Self::PlainDds,
        Self::ExtensibleDds,
        Self::RpcOverDds,
    ];

    /// The name the specification gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PlainCorba => "Plain CORBA",
            Self::MinimumCorba => "Minimum CORBA",
            Self::Ccm => "CCM",
            Self::CcmWithGenericInteractionSupport => "CCM with Generic Interaction Support",
            Self::PlainDds => "Plain DDS",
            Self::ExtensibleDds => "Extensible DDS",
            Self::RpcOverDds => "RPC over DDS",
        }
    }

    /// The command-line spelling of the name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlainCorba => "plain-corba",
            Self::MinimumCorba => "minimum-corba",
            Self::Ccm => "ccm",
            Self::CcmWithGenericInteractionSupport => "ccm-gis",
            Self::PlainDds => "plain-dds",
            Self::ExtensibleDds => "extensible-dds",
            Self::RpcOverDds => "rpc-over-dds",
        }
    }

    /// The profile a command-line spelling names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.as_str() == name)
    }

    /// The clause that defines it.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::PlainCorba => "§9.2.1",
            Self::MinimumCorba => "§9.2.2",
            Self::Ccm => "§9.2.3",
            Self::CcmWithGenericInteractionSupport => "§9.2.4",
            Self::PlainDds => "§9.3.1",
            Self::ExtensibleDds => "§9.3.2",
            Self::RpcOverDds => "§9.3.3",
        }
    }

    /// The building blocks the profile is "made of", as its clause lists
    /// them.
    #[must_use]
    pub const fn building_blocks(self) -> &'static [BuildingBlock] {
        use BuildingBlock as B;
        match self {
            Self::PlainCorba => &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
            ],
            Self::MinimumCorba => &[
                B::CoreDataTypes,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::CorbaSpecificInterfaces,
            ],
            Self::Ccm => &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
                B::ComponentsBasic,
                B::CcmSpecific,
            ],
            Self::CcmWithGenericInteractionSupport => &[
                B::CoreDataTypes,
                B::Any,
                B::InterfacesBasic,
                B::InterfacesFull,
                B::ValueTypes,
                B::CorbaSpecificInterfaces,
                B::CorbaSpecificValueTypes,
                B::ComponentsBasic,
                B::CcmSpecific,
                B::ComponentsPortsAndConnectors,
                B::TemplateModules,
            ],
            Self::PlainDds => &[B::CoreDataTypes, B::AnonymousTypes],
            Self::ExtensibleDds => &[
                B::CoreDataTypes,
                B::ExtendedDataTypes,
                B::AnonymousTypes,
                B::Annotations,
            ],
            Self::RpcOverDds => &[
                B::CoreDataTypes,
                B::ExtendedDataTypes,
                B::AnonymousTypes,
                B::InterfacesBasic,
                B::Annotations,
            ],
        }
    }

    /// The groups of annotations the profile includes.
    #[must_use]
    pub const fn annotation_groups(self) -> &'static [AnnotationGroup] {
        use AnnotationGroup as G;
        match self {
            Self::ExtensibleDds => &[
                G::GeneralPurpose,
                G::DataModeling,
                G::DataImplementation,
                G::CodeGeneration,
            ],
            Self::RpcOverDds => &[G::GeneralPurpose, G::Interfaces],
            _ => &[],
        }
    }

    /// Whether the profile includes `block`.
    #[must_use]
    pub fn includes(self, block: BuildingBlock) -> bool {
        self.building_blocks().contains(&block)
    }
}

/// One construct of a file and the building block that admits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockUse {
    /// The block.
    pub block: BuildingBlock,
    /// The rule of Annex A the construct is written with.
    pub rule: u16,
    /// What the construct is, as a diagnostic names it.
    pub what: &'static str,
    /// Where it is.
    pub span: Span,
}

/// One application of a standardized annotation and its group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupUse {
    /// The group.
    pub group: AnnotationGroup,
    /// The annotation's name.
    pub name: String,
    /// Where it is.
    pub span: Span,
}

/// Where a type is written, which decides whether a template type there
/// needs the Anonymous Types block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// The type a `typedef` names, rule (64): a template type is allowed.
    Typedef,
    /// The type of a constant, rule (6), or of an annotation member, rule
    /// (221): `string`, `wstring`, `fixed` are allowed.
    Constant,
    /// Any other `<type_spec>`: a template type is rule (216).
    TypeSpec,
    /// The type of a member of an annotation declaration, rule (223): a
    /// constant type, `any` (rule (224)) or a scoped name.
    AnnotationMember,
}

/// Every construct of a parsed file, with the block that admits it, in
/// source order; and every standardized annotation, with its group.
#[must_use]
pub fn blocks_used(root: &Arc<SyntaxNode>) -> (Vec<BlockUse>, Vec<GroupUse>) {
    let mut walk = Walk::default();
    walk.node(root, Context::default());
    (walk.blocks, walk.groups)
}

/// The constructs of a parsed file that `profile` does not include, as
/// diagnostics: a building block outside the profile (§9), an annotation
/// of a group the profile does not name (§8.1: "Groups of annotations
/// may be part of a given profile").
#[must_use]
pub fn outside_profile(root: &Arc<SyntaxNode>, profile: IdlProfile) -> Vec<Diagnostic> {
    let (blocks, groups) = blocks_used(root);
    let mut out = Vec::new();
    for found in blocks {
        if profile.includes(found.block) {
            continue;
        }
        out.push(Diagnostic::error(
            DiagnosticCode::OutsideProfile,
            found.span,
            format!(
                "{} is rule ({}) of the building block {} ({}), which the {} profile ({}) \
                 does not include",
                found.what,
                found.rule,
                found.block.name(),
                found.block.clause(),
                profile.name(),
                profile.clause()
            ),
            "OMG IDL 4.2 clause 9",
        ));
    }
    if profile.includes(BuildingBlock::Annotations) {
        for found in groups {
            if profile.annotation_groups().contains(&found.group) {
                continue;
            }
            out.push(Diagnostic::error(
                DiagnosticCode::OutsideProfile,
                found.span,
                format!(
                    "`@{}` is in the group of annotations {} (§8.3), which the {} profile ({}) \
                     does not include",
                    found.name,
                    found.group.name(),
                    profile.name(),
                    profile.clause()
                ),
                "OMG IDL 4.2 §8.1, clause 9",
            ));
        }
    }
    out
}

/// What the walk knows about the surroundings of a node.
#[derive(Debug, Clone, Copy, Default)]
struct Context {
    /// Inside the body of an interface (any kind), so a declaration there
    /// is an export of rule (97).
    interface_body: bool,
    /// Inside the body of a port type or a connector, where `provides`
    /// and `uses` are rules (176) and (183).
    port_body: bool,
    /// Inside the body of a home, where `factory` is rule (150).
    home_body: bool,
    /// Where a type written here stands.
    position: Option<Position>,
}

#[derive(Default)]
struct Walk {
    blocks: Vec<BlockUse>,
    groups: Vec<GroupUse>,
}

impl Walk {
    fn add(&mut self, block: BuildingBlock, rule: u16, what: &'static str, span: Span) {
        self.blocks.push(BlockUse {
            block,
            rule,
            what,
            span,
        });
    }

    fn children(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        for child in node.children() {
            self.node(&child, context);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn node(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        use BuildingBlock as B;
        let span = node.span();
        let first = first_keyword(node);
        let has = |kind: TokenKind| has_token(node, kind);
        match node.kind() {
            CstKind::Module => {
                if child_of_kind(node, CstKind::TemplateParameters) {
                    self.add(B::TemplateModules, 185, "a template module", span);
                } else {
                    self.add(B::CoreDataTypes, 3, "a module", span);
                }
                self.children(node, Context::default());
            }
            CstKind::TemplateModuleInst => {
                self.add(
                    B::TemplateModules,
                    190,
                    "a template module instantiation",
                    span,
                );
                self.children(
                    node,
                    Context {
                        position: Some(Position::TypeSpec),
                        ..Context::default()
                    },
                );
            }
            CstKind::TemplateModuleRef => {
                self.add(B::TemplateModules, 193, "a template module reference", span);
            }
            CstKind::TemplateParameters => {
                // Rule (188) admits `<sequence_type>` as a formal parameter
                // type; it is the template module's and not anonymous.
            }
            CstKind::ConstDecl => {
                self.export(context, span);
                self.add(B::CoreDataTypes, 5, "a constant", span);
                self.children(
                    node,
                    Context {
                        position: Some(Position::Constant),
                        ..context
                    },
                );
            }
            CstKind::TypedefDecl => {
                self.export(context, span);
                self.add(B::CoreDataTypes, 63, "a typedef", span);
                for child in node.children() {
                    match child.kind() {
                        CstKind::Declarator => {}
                        _ if is_type(child.kind()) => self.type_(
                            &child,
                            Context {
                                position: Some(Position::Typedef),
                                ..context
                            },
                        ),
                        _ => self.node(
                            &child,
                            Context {
                                position: None,
                                ..context
                            },
                        ),
                    }
                }
            }
            CstKind::StructDecl => {
                self.export(context, span);
                if child_of_kind(node, CstKind::InheritanceClause) {
                    self.add(B::ExtendedDataTypes, 195, "a structure with a base", span);
                } else if !child_of_kind(node, CstKind::Member) {
                    self.add(
                        B::ExtendedDataTypes,
                        195,
                        "a structure with no member",
                        span,
                    );
                } else {
                    self.add(B::CoreDataTypes, 46, "a structure", span);
                }
                self.body(node, context);
            }
            CstKind::UnionDecl => {
                self.export(context, span);
                self.add(B::CoreDataTypes, 50, "a union", span);
                for child in node.children() {
                    if child.kind() == CstKind::PrimitiveType {
                        // The discriminator: rule (51), and (196) for the
                        // two types Extended Data-Types adds.
                        if has_token(&child, TokenKind::Wchar) {
                            self.add(
                                B::ExtendedDataTypes,
                                196,
                                "a `wchar` discriminator",
                                child.span(),
                            );
                        } else if has_token(&child, TokenKind::Octet) {
                            self.add(
                                B::ExtendedDataTypes,
                                196,
                                "an `octet` discriminator",
                                child.span(),
                            );
                        }
                    }
                }
                self.body(node, context);
            }
            CstKind::EnumDecl => {
                self.export(context, span);
                self.add(B::CoreDataTypes, 57, "an enumeration", span);
                self.children(node, context);
            }
            CstKind::BitsetDecl => {
                self.export(context, span);
                self.add(B::ExtendedDataTypes, 200, "a bit set", span);
                self.children(
                    node,
                    Context {
                        position: Some(Position::Constant),
                        ..context
                    },
                );
            }
            CstKind::BitmaskDecl => {
                self.export(context, span);
                self.add(B::ExtendedDataTypes, 204, "a bit mask", span);
                self.children(node, context);
            }
            CstKind::NativeDecl => {
                self.export(context, span);
                self.add(B::CoreDataTypes, 61, "a native type", span);
            }
            CstKind::ExceptionDecl => {
                self.export(context, span);
                self.add(B::InterfacesBasic, 72, "an exception", span);
                self.body(node, context);
            }
            CstKind::ForwardDecl => self.forward(node, first, context),
            CstKind::InterfaceDecl => self.interface(node, first),
            CstKind::ValuetypeDecl => self.valuetype(node, first),
            CstKind::ValueBoxDecl => {
                self.add(B::CorbaSpecificValueTypes, 126, "a boxed value type", span);
                self.children(
                    node,
                    Context {
                        position: Some(Position::TypeSpec),
                        ..Context::default()
                    },
                );
            }
            CstKind::AnnotationDecl => {
                self.add(B::Annotations, 219, "an annotation declaration", span);
                self.children(
                    node,
                    Context {
                        position: Some(Position::AnnotationMember),
                        ..Context::default()
                    },
                );
            }
            CstKind::Annotation => {
                self.add(B::Annotations, 225, "an annotation", span);
                if let Some(name) = annotation_name(node)
                    && let Some(group) = AnnotationGroup::of_annotation(&name)
                {
                    self.groups.push(GroupUse { group, name, span });
                }
            }
            CstKind::RepositoryDirective => {
                let (rule, what) = if has(TokenKind::Typeprefix) {
                    (114, "a `typeprefix` declaration")
                } else {
                    (113, "a `typeid` declaration")
                };
                self.add(B::CorbaSpecificInterfaces, rule, what, span);
            }
            CstKind::ImportDecl => {
                self.add(
                    B::CorbaSpecificInterfaces,
                    115,
                    "an `import` declaration",
                    span,
                );
            }
            CstKind::Operation => self.operation(node, context),
            CstKind::AttributeDecl => {
                if has(TokenKind::Getraises) || has(TokenKind::Setraises) {
                    self.add(B::InterfacesBasic, 93, "an attribute with exceptions", span);
                } else {
                    self.add(B::InterfacesBasic, 88, "an attribute", span);
                }
                self.typed_children(node, context);
            }
            CstKind::PortDecl => self.port(node, first, context),
            CstKind::Member | CstKind::UnionCase | CstKind::Parameter => {
                if node.kind() == CstKind::Member
                    && (has(TokenKind::Public) || has(TokenKind::Private))
                {
                    self.add(B::ValueTypes, 106, "a state member", span);
                }
                for declarator in node
                    .children()
                    .into_iter()
                    .filter(|child| child.kind() == CstKind::Declarator)
                {
                    if child_of_kind(&declarator, CstKind::ArrayDimension) {
                        self.add(
                            B::AnonymousTypes,
                            217,
                            "an array declared at its member",
                            declarator.span(),
                        );
                    }
                }
                if context.position == Some(Position::AnnotationMember) {
                    self.children(node, context);
                } else {
                    self.typed_children(node, context);
                }
            }
            kind if is_type(kind) => self.type_(node, context),
            _ => self.children(node, context),
        }
    }

    /// A declaration written inside an interface body: rule (97).
    fn export(&mut self, context: Context, span: Span) {
        if context.interface_body {
            self.add(
                BuildingBlock::InterfacesFull,
                97,
                "a declaration inside an interface",
                span,
            );
        }
    }

    /// The members of a structure, union or exception.
    fn body(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        let inner = Context {
            interface_body: false,
            position: None,
            ..context
        };
        for child in node.children() {
            match child.kind() {
                // The discriminator of a union stands where rule (50)
                // writes a `<switch_type_spec>`, which takes no template.
                kind if is_type(kind) => self.type_(
                    &child,
                    Context {
                        position: Some(Position::Constant),
                        ..inner
                    },
                ),
                _ => self.node(&child, inner),
            }
        }
    }

    /// The children of a node whose type is a `<type_spec>`.
    fn typed_children(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        let inner = Context {
            position: Some(Position::TypeSpec),
            ..context
        };
        self.children(node, inner);
    }

    fn type_(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        use BuildingBlock as B;
        let span = node.span();
        let position = context.position.unwrap_or(Position::TypeSpec);
        match node.kind() {
            CstKind::PrimitiveType => {
                let extended = [
                    (TokenKind::Int8, 208, "`int8`"),
                    (TokenKind::Uint8, 209, "`uint8`"),
                    (TokenKind::Int16, 210, "`int16`"),
                    (TokenKind::Int32, 211, "`int32`"),
                    (TokenKind::Int64, 212, "`int64`"),
                    (TokenKind::Uint16, 213, "`uint16`"),
                    (TokenKind::Uint32, 214, "`uint32`"),
                    (TokenKind::Uint64, 215, "`uint64`"),
                ];
                if let Some((_, rule, what)) =
                    extended.iter().find(|(kind, _, _)| has_token(node, *kind))
                {
                    self.add(B::ExtendedDataTypes, *rule, what, span);
                } else if has_token(node, TokenKind::Any) {
                    // `any` as the type of an annotation member is rule
                    // (224) of the Annotations block, not the Any block.
                    if position != Position::AnnotationMember {
                        self.add(B::Any, 70, "`any`", span);
                    }
                } else if has_token(node, TokenKind::Object) {
                    self.add(B::CorbaSpecificInterfaces, 118, "`Object`", span);
                } else if has_token(node, TokenKind::Valuebase) {
                    self.add(B::CorbaSpecificValueTypes, 132, "`ValueBase`", span);
                }
            }
            CstKind::SequenceType
            | CstKind::StringType
            | CstKind::WStringType
            | CstKind::FixedType
            | CstKind::MapType => {
                if node.kind() == CstKind::MapType {
                    self.add(B::ExtendedDataTypes, 199, "a map", span);
                }
                let allowed = match position {
                    Position::Typedef => true,
                    Position::Constant | Position::AnnotationMember => matches!(
                        node.kind(),
                        CstKind::StringType | CstKind::WStringType | CstKind::FixedType
                    ),
                    Position::TypeSpec => false,
                };
                if !allowed {
                    self.add(
                        B::AnonymousTypes,
                        216,
                        "a template type written where a type is used",
                        span,
                    );
                }
                // The arguments of a template type are `<type_spec>`s.
                self.children(
                    node,
                    Context {
                        position: Some(Position::TypeSpec),
                        ..context
                    },
                );
            }
            // A constructed type defined inside a `typedef`, rule (64).
            CstKind::StructDecl
            | CstKind::UnionDecl
            | CstKind::EnumDecl
            | CstKind::BitsetDecl
            | CstKind::BitmaskDecl => self.node(
                node,
                Context {
                    position: None,
                    ..context
                },
            ),
            _ => self.children(node, context),
        }
    }

    fn forward(&mut self, node: &Arc<SyntaxNode>, first: Option<TokenKind>, context: Context) {
        use BuildingBlock as B;
        let span = node.span();
        let (block, rule, what) = match first {
            Some(TokenKind::Struct) => (B::CoreDataTypes, 48, "a structure forward declaration"),
            Some(TokenKind::Union) => (B::CoreDataTypes, 56, "a union forward declaration"),
            Some(TokenKind::Interface) => {
                (B::InterfacesBasic, 75, "an interface forward declaration")
            }
            Some(TokenKind::Local) => (
                B::CorbaSpecificInterfaces,
                119,
                "a local interface forward declaration",
            ),
            Some(TokenKind::Abstract) if has_token(node, TokenKind::Eventtype) => {
                (B::CcmSpecific, 167, "an event type forward declaration")
            }
            Some(TokenKind::Abstract) if has_token(node, TokenKind::Interface) => (
                B::CorbaSpecificValueTypes,
                129,
                "an abstract interface forward declaration",
            ),
            Some(TokenKind::Abstract) => (
                B::CorbaSpecificValueTypes,
                127,
                "an abstract value type forward declaration",
            ),
            Some(TokenKind::Valuetype) => (B::ValueTypes, 110, "a value type forward declaration"),
            Some(TokenKind::Custom) => (
                B::CorbaSpecificValueTypes,
                128,
                "a custom value type forward declaration",
            ),
            Some(TokenKind::Component) => {
                (B::ComponentsBasic, 135, "a component forward declaration")
            }
            Some(TokenKind::Eventtype) => {
                (B::CcmSpecific, 167, "an event type forward declaration")
            }
            Some(TokenKind::Porttype) => (
                B::ComponentsPortsAndConnectors,
                173,
                "a port type forward declaration",
            ),
            _ => {
                self.children(node, context);
                return;
            }
        };
        self.export(context, span);
        self.add(block, rule, what, span);
    }

    fn interface(&mut self, node: &Arc<SyntaxNode>, first: Option<TokenKind>) {
        use BuildingBlock as B;
        let span = node.span();
        let mut context = Context {
            interface_body: true,
            ..Context::default()
        };
        let supports = node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::InheritanceClause)
            .any(|clause| has_token(&clause, TokenKind::Supports));
        match first {
            Some(TokenKind::Local) => {
                self.add(B::CorbaSpecificInterfaces, 119, "a local interface", span);
            }
            Some(TokenKind::Abstract) => {
                self.add(
                    B::CorbaSpecificValueTypes,
                    129,
                    "an abstract interface",
                    span,
                );
            }
            Some(TokenKind::Component) => {
                if supports {
                    self.add(
                        B::CcmSpecific,
                        154,
                        "a component that supports interfaces",
                        span,
                    );
                } else {
                    self.add(B::ComponentsBasic, 136, "a component", span);
                }
            }
            Some(TokenKind::Home) => {
                context.home_body = true;
                if supports {
                    self.add(B::CcmSpecific, 162, "a home that supports interfaces", span);
                } else {
                    self.add(B::ComponentsHomes, 145, "a home", span);
                }
            }
            Some(TokenKind::Porttype) => {
                context.port_body = true;
                self.add(B::ComponentsPortsAndConnectors, 174, "a port type", span);
            }
            Some(TokenKind::Connector) => {
                context.port_body = true;
                self.add(B::ComponentsPortsAndConnectors, 180, "a connector", span);
            }
            _ => self.add(B::InterfacesBasic, 74, "an interface", span),
        }
        for child in node.children() {
            match child.kind() {
                CstKind::InheritanceClause => {}
                CstKind::ManagesClause => {
                    self.add(B::ComponentsHomes, 146, "a `manages` clause", child.span());
                    if has_token(&child, TokenKind::Primarykey) {
                        self.add(B::CcmSpecific, 163, "a primary key", child.span());
                    }
                }
                _ => self.node(&child, context),
            }
        }
    }

    fn valuetype(&mut self, node: &Arc<SyntaxNode>, first: Option<TokenKind>) {
        use BuildingBlock as B;
        let span = node.span();
        let event = has_token(node, TokenKind::Eventtype);
        match first {
            _ if event => self.add(B::CcmSpecific, 166, "an event type", span),
            Some(TokenKind::Abstract) => {
                self.add(
                    B::CorbaSpecificValueTypes,
                    127,
                    "an abstract value type",
                    span,
                );
            }
            Some(TokenKind::Custom) => {
                self.add(B::CorbaSpecificValueTypes, 128, "a custom value type", span);
            }
            _ => self.add(B::ValueTypes, 100, "a value type", span),
        }
        if node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::InheritanceClause)
            .any(|clause| has_token(&clause, TokenKind::Truncatable))
        {
            self.add(
                B::CorbaSpecificValueTypes,
                130,
                "a truncatable value type",
                span,
            );
        }
        let context = Context {
            interface_body: true,
            ..Context::default()
        };
        for child in node.children() {
            match child.kind() {
                CstKind::InheritanceClause => {}
                CstKind::Operation if has_token(&child, TokenKind::Factory) => {
                    self.add(B::ValueTypes, 107, "an initializer", child.span());
                    self.typed_children(&child, context);
                }
                _ => self.node(&child, context),
            }
        }
    }

    fn operation(&mut self, node: &Arc<SyntaxNode>, context: Context) {
        use BuildingBlock as B;
        let span = node.span();
        if has_token(node, TokenKind::Factory) {
            if context.home_body {
                self.add(B::ComponentsHomes, 150, "a factory operation", span);
            } else {
                self.add(B::ValueTypes, 107, "an initializer", span);
            }
        } else if has_token(node, TokenKind::Finder) {
            self.add(B::CcmSpecific, 165, "a finder operation", span);
        } else if has_token(node, TokenKind::Oneway) {
            self.add(B::CorbaSpecificInterfaces, 120, "a one-way operation", span);
        } else {
            self.add(B::InterfacesBasic, 82, "an operation", span);
        }
        if child_of_kind(node, CstKind::ContextClause) {
            self.add(
                B::CorbaSpecificInterfaces,
                124,
                "a context expression",
                span,
            );
        }
        self.typed_children(node, context);
    }

    fn port(&mut self, node: &Arc<SyntaxNode>, first: Option<TokenKind>, context: Context) {
        use BuildingBlock as B;
        let span = node.span();
        let (block, rule, what) = match first {
            Some(TokenKind::Provides) if context.port_body => (
                B::ComponentsPortsAndConnectors,
                176,
                "a facet of a port type or connector",
            ),
            Some(TokenKind::Uses) if context.port_body => (
                B::ComponentsPortsAndConnectors,
                176,
                "a receptacle of a port type or connector",
            ),
            Some(TokenKind::Provides) => (B::ComponentsBasic, 141, "a facet"),
            Some(TokenKind::Uses) if has_token(node, TokenKind::Multiple) => {
                (B::CcmSpecific, 158, "a multiple receptacle")
            }
            Some(TokenKind::Uses) => (B::ComponentsBasic, 143, "a receptacle"),
            Some(TokenKind::Emits) => (B::CcmSpecific, 159, "an event source (`emits`)"),
            Some(TokenKind::Publishes) => (B::CcmSpecific, 160, "an event source (`publishes`)"),
            Some(TokenKind::Consumes) => (B::CcmSpecific, 161, "an event sink"),
            Some(TokenKind::Port | TokenKind::Mirrorport) => {
                (B::ComponentsPortsAndConnectors, 178, "an extended port")
            }
            _ => return,
        };
        self.add(block, rule, what, span);
        if has_token(node, TokenKind::Object) {
            self.add(
                B::CcmSpecific,
                157,
                "`Object` as the interface of a port",
                span,
            );
        }
    }
}

/// Whether a node kind is a type written in place.
const fn is_type(kind: CstKind) -> bool {
    matches!(
        kind,
        CstKind::PrimitiveType
            | CstKind::NamedType
            | CstKind::SequenceType
            | CstKind::StringType
            | CstKind::WStringType
            | CstKind::FixedType
            | CstKind::MapType
    )
}

/// The first keyword of a node's own tokens.
fn first_keyword(node: &Arc<SyntaxNode>) -> Option<TokenKind> {
    node.syntax_tokens()
        .into_iter()
        .map(|(_, token)| token.kind())
        .find(|kind| *kind != TokenKind::Ident)
}

fn has_token(node: &Arc<SyntaxNode>, kind: TokenKind) -> bool {
    node.syntax_tokens()
        .iter()
        .any(|(_, token)| token.kind() == kind)
}

fn child_of_kind(node: &Arc<SyntaxNode>, kind: CstKind) -> bool {
    node.children().iter().any(|child| child.kind() == kind)
}

/// The name an annotation application writes after `@`: its last
/// segment, which may be a keyword (`@default`, §8.2).
fn annotation_name(node: &Arc<SyntaxNode>) -> Option<String> {
    let mut name = None;
    for (_, token) in node.syntax_tokens() {
        match token.kind() {
            TokenKind::At | TokenKind::SlashSlashAt | TokenKind::ColonColon => {}
            kind if kind == TokenKind::Ident || kind.is_keyword() => {
                name = Some(String::from(zerodds_idl_cst::identifier_name(token.text())));
            }
            _ => break,
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every rule of Annex A belongs to exactly one block, and the blocks
    /// cover 1 to 227 without a gap.
    #[test]
    fn idl_annex_a_every_rule_belongs_to_one_building_block() {
        for rule in 1..=227 {
            let owners = BuildingBlock::ALL
                .into_iter()
                .filter(|block| {
                    let (first, last) = block.rules();
                    (first..=last).contains(&rule)
                })
                .count();
            assert_eq!(owners, 1, "rule ({rule})");
        }
        assert_eq!(BuildingBlock::of_rule(0), None);
        assert_eq!(BuildingBlock::of_rule(228), None);
    }
}
