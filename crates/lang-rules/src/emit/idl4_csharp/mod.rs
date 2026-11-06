// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C# Language Mapping 1.0 (OMG formal/2021-07-01).
//!
//! `zerodds-idlc generate --lang csharp --csharp-mapping idl4` writes the
//! C# of clauses 7 and 8 of that specification: a namespace per module
//! (§7.2.2), a static class per constant or a `Constants` container
//! (§7.2.3), a class with properties, three constructors and
//! `IEquatable<T>` per structure (§7.2.4.3.1), a class with
//! `Discriminator` per union (§7.2.4.3.2), a C# `enum` per enumeration
//! (§7.2.4.3.3), `I<Name>` per interface (§7.4) with `I<Name>Operations`
//! and a class for its embedded declarations (§7.5), an exception class
//! extending `System.Exception` (§7.4.1), an abstract class per value type
//! (§7.6), a `struct` per bit set (§7.14.3.2), a `[Flags]` enum per bit
//! mask (§7.14.3.3), an attribute class per user annotation (§7.16), and
//! the two naming schemes of §7.1.1 with `@csharp_mapping` (§8.1).
//!
//! The default column of `--lang csharp` is the XCDR2 C# binding
//! (`crate::adapter::csharp`), which carries the wire codec. This module is
//! the language mapping on its own: it writes types and interfaces, and no
//! codec.
//!
//! The runtime classes the mapping names — `Omg.Types.ISequence<T>`, its
//! implementations, `Omg.Types.Any` and the attributes of §7.6 and §7.17,
//! and for the CORBA platform `Corba.UserException`, `Corba.TypeCode`,
//! `Corba.Any`, `Corba.Object` and `Corba.IObject` — are in
//! `crates/idl-csharp/runtime/`.

pub mod names;
pub mod text;
mod types;

mod behaviour;
mod data;

pub use names::Naming;
use std::collections::BTreeMap;
use text::Writer;
use zerodds_ast::{
    AnyId, Ast, Facets, MappingLanguage, MappingValue, ModuleNode, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;

pub use types::{CType, Range, Shape};

/// How deep a chain of typedefs, bases or modules is followed.
pub(crate) const MAX_DEPTH: usize = 64;

/// The platform of Annex A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    /// A.2: "DDS requires no additional platform-specific language
    /// mappings."
    #[default]
    Dds,
    /// A.1: exceptions extend `Corba.UserException`, `any` is
    /// `Corba.Any`, `Object` is `Corba.IObject`, interfaces extend
    /// `Corba.IObject` and their classes `Corba.Object`.
    Corba,
}

impl Platform {
    /// The name the command line gives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dds => "dds",
            Self::Corba => "corba",
        }
    }

    /// The platform a command-line name selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dds" => Some(Self::Dds),
            "corba" => Some(Self::Corba),
            _ => None,
        }
    }
}

/// What the generator starts from before any `@csharp_mapping`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Options {
    /// The naming scheme (§7.1.1: "Implementations of this specification
    /// may also provide custom compiler settings or compiler parameters
    /// for such purpose").
    pub naming: Naming,
    /// The platform of Annex A.
    pub platform: Platform,
    /// `constants_container` (§8.1.2): `None` is the Standalone Constants
    /// Mapping of §7.2.3.1, a name the Constants Container Mapping of
    /// §7.2.3.2 into a class of that name.
    pub constants_container: Option<String>,
    /// Under [`Platform::Corba`], whether the C# of the `CORBA` module the
    /// IDL includes (`orb.idl`) is written too. Its types belong in the
    /// `Corba` namespace of the ORB's runtime (A.1.2), so by default an
    /// IDL file that includes `orb.idl` gets the C# of its own
    /// declarations only.
    pub corba_module: bool,
}

/// One generated C# source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsFile {
    /// The path below the output directory: the namespace's directories
    /// and `<Type>.cs`.
    pub path: String,
    /// The source.
    pub text: String,
}

/// Why the mapping could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Idl4CsharpError {
    /// A construct the mapping gives no C# form.
    NoCsharpForm {
        /// What.
        construct: String,
        /// Where.
        at: String,
    },
    /// Two IDL declarations map to one C# name.
    Collision {
        /// The C# name.
        name: String,
        /// The declaration that took it first.
        first: String,
        /// The declaration that wanted it too.
        second: String,
    },
    /// A value the output has to state does not evaluate.
    Unevaluated {
        /// What.
        what: String,
        /// Where.
        at: String,
    },
    /// A reference that does not resolve.
    Unresolved {
        /// What.
        what: String,
        /// Where.
        at: String,
    },
}

impl core::fmt::Display for Idl4CsharpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoCsharpForm { construct, at } => {
                write!(f, "IDL4 to C# 1.0 gives {construct} no C# form (at `{at}`)")
            }
            Self::Collision {
                name,
                first,
                second,
            } => write!(
                f,
                "`{first}` and `{second}` both map to the C# name `{name}` (IDL4 to C# 1.0 §7.1.1)"
            ),
            Self::Unevaluated { what, at } => write!(f, "{what} does not evaluate (at `{at}`)"),
            Self::Unresolved { what, at } => write!(f, "{what} does not resolve (at `{at}`)"),
        }
    }
}

impl std::error::Error for Idl4CsharpError {}

/// The C# of `ast` under IDL4 to C# 1.0.
///
/// # Errors
/// [`Idl4CsharpError`] for a construct with no C# form, two declarations
/// that map to one name, and a value or reference that does not
/// evaluate — never a file with a declaration left out.
pub fn generate(ast: &Ast, options: &Options) -> Result<Vec<CsFile>, Idl4CsharpError> {
    let intermediate = crate::emit::intermediate_idl::intermediate_idl(ast).map_err(|what| {
        Idl4CsharpError::Unevaluated {
            what,
            at: String::new(),
        }
    })?;
    let ast = &intermediate;
    let mut generator = Gen {
        ast,
        options,
        files: Vec::new(),
        taken: BTreeMap::new(),
    };
    generator.module(ast.root_module(), 0)?;
    Ok(generator.files)
}

/// What one declaration is mapped with: the options, then every
/// `@csharp_mapping` from the outermost module to the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    /// The naming scheme.
    pub naming: Naming,
    /// `constants_container`, when the container mapping is on.
    pub container: Option<String>,
    /// `struct_type = "struct"` (§8.1.3).
    pub struct_type: bool,
}

impl Settings {
    fn of(options: &Options) -> Self {
        Self {
            naming: options.naming,
            container: options.constants_container.clone(),
            struct_type: false,
        }
    }

    /// The `@csharp_mapping` settings of one node on top.
    fn apply(&mut self, facets: &Facets) {
        for setting in &facets.mappings {
            if setting.language != MappingLanguage::CSharp {
                continue;
            }
            match (setting.parameter.as_str(), &setting.value) {
                ("apply_naming_convention", MappingValue::Name(name)) => {
                    if let Some(naming) = Naming::from_name(name) {
                        self.naming = naming;
                    }
                }
                // §8.1.2: "To enable the Standalone Constants Mapping ...
                // constants_container shall be set to an empty string."
                ("constants_container", MappingValue::Text(name)) => {
                    self.container = (!name.is_empty()).then(|| name.clone());
                }
                ("struct_type", MappingValue::Text(kind)) => {
                    self.struct_type = kind == "struct";
                }
                // The front end admits nothing else (idl-lower,
                // `mapping_annotation`).
                _ => {}
            }
        }
    }
}

/// The generator's state.
pub(crate) struct Gen<'a> {
    pub(crate) ast: &'a Ast,
    pub(crate) options: &'a Options,
    files: Vec<CsFile>,
    /// Every file path written, with the IDL declaration it holds.
    taken: BTreeMap<String, String>,
}

/// The declarations of one scope: a module's, or an interface's or value
/// type's embedded ones (§7.5).
pub(crate) struct Scoped {
    pub types: Vec<Id<TypeNode>>,
    pub constants: Vec<Id<zerodds_ast::ConstNode>>,
    pub exceptions: Vec<Id<zerodds_ast::ExceptionNode>>,
}

impl Scoped {
    pub(crate) fn is_empty(&self) -> bool {
        self.types.is_empty() && self.constants.is_empty() && self.exceptions.is_empty()
    }
}

impl Gen<'_> {
    fn module(&mut self, module: Id<ModuleNode>, depth: usize) -> Result<(), Idl4CsharpError> {
        if depth > MAX_DEPTH {
            return Err(Idl4CsharpError::Unevaluated {
                what: "a nesting of modules deeper than the mapping follows".to_string(),
                at: String::new(),
            });
        }
        let node = self
            .ast
            .module(module)
            .map_err(|_| Idl4CsharpError::Unresolved {
                what: "a module".to_string(),
                at: String::new(),
            })?;
        let children = node.children.clone();
        let mut constants = Vec::new();
        for child in children {
            match child {
                AnyId::Module(id, _) if self.is_corba_module(id) && !self.options.corba_module => {}
                AnyId::Module(id, _) => self.module(id, depth + 1)?,
                AnyId::Type(id, _) => {
                    if self.ast.type_(id).is_ok_and(|t| t.scope.is_none()) {
                        self.top_level_type(id)?;
                    }
                }
                AnyId::Const(id, _) => {
                    if self.ast.constant(id).is_ok_and(|c| c.scope.is_none())
                        && !self.is_template_binding(id)
                    {
                        constants.push(id);
                    }
                }
                AnyId::Exception(id, _) => {
                    if self.ast.exception(id).is_ok_and(|e| e.scope.is_none()) {
                        self.top_level_exception(id)?;
                    }
                }
                AnyId::Interface(id, _) => self.top_level_interface(id)?,
                AnyId::AnnotationDef(id, _) => self.annotation_type(id)?,
                _ => {}
            }
        }
        self.top_level_constants(module, &constants)?;
        Ok(())
    }

    /// Whether a constant is the binding of a template module's `const`
    /// formal parameter in an instance: the instance "is exactly as a
    /// classical module" (OMG IDL 4.2 §7.4.12.4.2), and the formal is no
    /// declaration of it.
    fn is_template_binding(&self, id: Id<zerodds_ast::ConstNode>) -> bool {
        let Ok(constant) = self.ast.constant(id) else {
            return false;
        };
        self.ast
            .module(constant.module)
            .ok()
            .and_then(|module| module.facets.corba.as_ref())
            .is_some_and(|corba| {
                corba.template_args.iter().any(|arg| {
                    matches!(arg, zerodds_ast::TemplateArg::Const(expr) if *expr == constant.expr)
                })
            })
    }

    /// The modules from the outermost to `module`, the specification
    /// scope left out.
    fn module_chain(&self, module: Id<ModuleNode>) -> Vec<Id<ModuleNode>> {
        let mut chain = Vec::new();
        let mut current = Some(module);
        while let Some(id) = current {
            if id == self.ast.root_module() || chain.len() > MAX_DEPTH {
                break;
            }
            chain.push(id);
            current = self.ast.module(id).ok().and_then(|node| node.parent);
        }
        chain.reverse();
        chain
    }

    /// Whether `module` is the `CORBA` module of the CORBA platform.
    pub(crate) fn is_corba_module(&self, module: Id<ModuleNode>) -> bool {
        self.options.platform == Platform::Corba
            && self.ast.module(module).is_ok_and(|node| {
                node.name == "CORBA" && node.parent == Some(self.ast.root_module())
            })
    }

    /// Whether `module` is the `CORBA` module of the CORBA platform or
    /// nested in it.
    pub(crate) fn in_corba_module(&self, module: Id<ModuleNode>) -> bool {
        self.module_chain(module)
            .first()
            .is_some_and(|outermost| self.is_corba_module(*outermost))
    }

    /// The settings in force inside `module`.
    pub(crate) fn settings_of_module(&self, module: Id<ModuleNode>) -> Settings {
        let mut settings = Settings::of(self.options);
        if let Ok(root) = self.ast.module(self.ast.root_module()) {
            settings.apply(&root.facets);
        }
        for id in self.module_chain(module) {
            if let Ok(node) = self.ast.module(id) {
                settings.apply(&node.facets);
            }
        }
        // A.1.2: the CORBA module's types are mapped "applying the IDL
        // Naming Scheme defined in Clause 7.1.1.1".
        if self.in_corba_module(module) {
            settings.naming = Naming::Idl;
        }
        settings
    }

    /// The settings of a declaration in `module`, inside the interface or
    /// value type `scope` names, carrying `facets`.
    pub(crate) fn settings_in(
        &self,
        module: Id<ModuleNode>,
        scope: Option<&str>,
        facets: &Facets,
    ) -> Settings {
        let mut settings = self.settings_of_module(module);
        if let Some(scope) = scope
            && let Some(owner) = self.scope_owner(module, scope)
            && let Some(owner_facets) = self.ast.facets(owner)
        {
            settings.apply(owner_facets);
        }
        settings.apply(facets);
        if self.in_corba_module(module) {
            settings.naming = Naming::Idl;
        }
        settings
    }

    /// The settings of a type.
    pub(crate) fn settings_of_type(&self, id: Id<TypeNode>) -> Settings {
        match self.ast.type_(id) {
            Ok(node) => self.settings_in(node.module, node.scope.as_deref(), &node.facets),
            Err(_) => Settings::of(self.options),
        }
    }

    /// Whether the structure `id` is a C# `struct` (§8.1.3).
    pub(crate) fn struct_type_of(&self, id: Id<TypeNode>) -> bool {
        self.settings_of_type(id).struct_type
    }

    /// The interface or value type a scope name in `module` is.
    pub(crate) fn scope_owner(&self, module: Id<ModuleNode>, scope: &str) -> Option<AnyId> {
        if let Some((id, node)) = self
            .ast
            .interfaces()
            .find(|(_, node)| node.module == module && node.name == scope)
        {
            return Some(AnyId::Interface(id, node.node_id));
        }
        self.ast
            .types()
            .find(|(_, node)| {
                node.module == module
                    && node.name == scope
                    && node.scope.is_none()
                    && matches!(node.kind, TypeKind::Valuetype { .. })
            })
            .map(|(id, node)| AnyId::Type(id, node.node_id))
    }

    /// The namespace segments of `module`. The CORBA module of the CORBA
    /// platform is `Corba` (A.1.2: "placed under the Corba namespace").
    pub(crate) fn namespace_of(&self, module: Id<ModuleNode>) -> Vec<String> {
        self.module_chain(module)
            .into_iter()
            .filter_map(|id| {
                let node = self.ast.module(id).ok()?;
                if self.is_corba_module(id) {
                    return Some("Corba".to_string());
                }
                Some(self.settings_of_module(id).naming.namespace(&node.name))
            })
            .collect()
    }

    /// `global::Ns.` for a module, `global::` for the global namespace.
    pub(crate) fn qualified_prefix(&self, module: Id<ModuleNode>) -> String {
        let namespace = self.namespace_of(module);
        if namespace.is_empty() {
            "global::".to_string()
        } else {
            format!("global::{}.", namespace.join("."))
        }
    }

    /// The C# name a scope name in `module` is written under: the class
    /// of an interface's embedded declarations (§7.5), or a value type's
    /// abstract class (§7.6).
    fn scope_class(&self, module: Id<ModuleNode>, scope: &str) -> Result<String, Idl4CsharpError> {
        match self.scope_owner(module, scope) {
            Some(AnyId::Interface(id, _)) => {
                let node = self.ast.interface(id).map_err(|_| unresolved(scope))?;
                let settings = self.settings_in(module, None, &node.facets);
                Ok(format!(
                    "{}{}",
                    self.qualified_prefix(module),
                    settings.naming.type_name(scope)
                ))
            }
            Some(AnyId::Type(id, _)) => {
                let settings = self.settings_of_type(id);
                Ok(format!(
                    "{}{}",
                    self.qualified_prefix(module),
                    settings.naming.type_name(scope)
                ))
            }
            _ => Err(unresolved(scope)),
        }
    }

    /// The qualified C# name of a declared type.
    pub(crate) fn qualified(&self, id: Id<TypeNode>) -> Result<String, Idl4CsharpError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        let settings = self.settings_of_type(id);
        let name = match node.kind {
            TypeKind::Bitmask { .. } => settings.naming.bitmask_name(&node.name),
            _ => settings.naming.type_name(&node.name),
        };
        match node.scope.as_deref() {
            Some(scope) => Ok(format!("{}.{name}", self.scope_class(node.module, scope)?)),
            None => Ok(format!("{}{name}", self.qualified_prefix(node.module))),
        }
    }

    /// The qualified C# name of an interface: `I<Name>` (§7.4).
    pub(crate) fn interface_name(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        // A.1.2: `CORBA::TypeCode` is the class `Corba.TypeCode`.
        if node.name == "TypeCode" && self.is_corba_module(node.module) {
            return Ok("global::Corba.TypeCode".to_string());
        }
        let settings = self.settings_in(node.module, None, &node.facets);
        Ok(format!(
            "{}{}",
            self.qualified_prefix(node.module),
            settings.naming.interface_name(&node.name)
        ))
    }

    /// The declarations embedded in the interface or value type `scope`
    /// of `module`.
    pub(crate) fn embedded(&self, module: Id<ModuleNode>, scope: &str) -> Scoped {
        let mut out = Scoped {
            types: Vec::new(),
            constants: Vec::new(),
            exceptions: Vec::new(),
        };
        let Ok(node) = self.ast.module(module) else {
            return out;
        };
        for child in &node.children {
            match child {
                AnyId::Type(id, _)
                    if self
                        .ast
                        .type_(*id)
                        .is_ok_and(|t| t.scope.as_deref() == Some(scope)) =>
                {
                    out.types.push(*id);
                }
                AnyId::Const(id, _)
                    if self
                        .ast
                        .constant(*id)
                        .is_ok_and(|c| c.scope.as_deref() == Some(scope)) =>
                {
                    out.constants.push(*id);
                }
                AnyId::Exception(id, _)
                    if self
                        .ast
                        .exception(*id)
                        .is_ok_and(|e| e.scope.as_deref() == Some(scope)) =>
                {
                    out.exceptions.push(*id);
                }
                _ => {}
            }
        }
        out
    }

    /// Write one top-level declaration of `module` into its file, in the
    /// module's namespace.
    pub(crate) fn file(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        origin: &str,
        body: &str,
    ) -> Result<(), Idl4CsharpError> {
        let namespace = self.namespace_of(module);
        let mut path = namespace.join("/");
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(name.trim_start_matches('@'));
        path.push_str(".cs");
        if let Some(first) = self.taken.get(&path) {
            return Err(Idl4CsharpError::Collision {
                name: name.to_string(),
                first: first.clone(),
                second: origin.to_string(),
            });
        }
        self.taken.insert(path.clone(), origin.to_string());
        let mut w = Writer::new();
        w.line("// Generated from IDL by ZeroDDS: IDL4 to C# Language Mapping 1.0.");
        w.line("");
        if namespace.is_empty() {
            w.nested(body);
        } else {
            w.open(&format!("namespace {}", namespace.join(".")));
            w.nested(body);
            w.close();
        }
        self.files.push(CsFile {
            path,
            text: w.finish(),
        });
        Ok(())
    }

    /// The scoped name of a node, for messages.
    pub(crate) fn origin(&self, any: AnyId) -> String {
        self.ast
            .scoped_name_of(any)
            .map_or_else(String::new, |name| name.segments().join("::"))
    }
}

fn unresolved(what: &str) -> Idl4CsharpError {
    Idl4CsharpError::Unresolved {
        what: what.to_string(),
        at: String::new(),
    }
}
