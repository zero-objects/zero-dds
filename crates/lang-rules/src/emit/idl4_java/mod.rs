// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to Java Language Mapping 1.0 (OMG formal/2021-08-01).
//!
//! `zerodds-idlc generate --lang java --java-mapping idl4` writes the
//! Java of clause 7 and 8 of that specification: a package per module
//! (§7.2.2), a file per public type, a class with the value in it per
//! constant (§7.2.3) or a container of them (§7.2.3.1), a class with
//! accessors, modifiers and the two constructors per structure
//! (§7.2.4.3.1), a final class with the discriminator per union
//! (§7.2.4.3.2), a Java `enum` per enumeration (§7.2.4.3.3), an
//! interface per interface with `Holder<E>` for `out` and `inout`
//! parameters (§7.4, §7.1.3), a class extending `RuntimeException` per
//! exception (§7.4.1), two classes per value type (§7.6), a Java
//! annotation type and its `Group` per user annotation (§7.16), and the
//! two naming schemes of §7.1.1 with `@java_mapping` (§8.1).
//!
//! The default column of `--lang java` is the DDS-Java-PSM 1.0 binding
//! (`crate::adapter::java`), which carries the wire codec of the Java
//! DDS runtime. This module is the language mapping on its own: it writes
//! types and interfaces, and no codec.
//!
//! The runtime classes the mapping names — `org.omg.type.Holder`, the
//! sequence classes of Table 7.4, `org.omg.type.Any`, and for the CORBA
//! platform `org.omg.CORBA.UserException`, `TypeCode`, `Any` and
//! `Object` — are in `runtime/java/src/main/java/org/omg/`.

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

pub use data::value_text;
pub use types::{JType, Range, Shape};

/// How deep a chain of typedefs, bases or modules is followed.
pub(crate) const MAX_DEPTH: usize = 64;

/// The platform of Annex A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    /// A.2: "DDS requires no additional platform-specific language
    /// mappings."
    #[default]
    Dds,
    /// A.1: exceptions extend `org.omg.CORBA.UserException`, `any` is
    /// `org.omg.CORBA.Any`, `Object` is `org.omg.CORBA.Object`.
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

/// What the generator starts from before any `@java_mapping`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The naming scheme (§7.1.1: "Implementations of this specification
    /// may also provide custom compiler settings or compiler parameters
    /// for such purpose").
    pub naming: Naming,
    /// The platform of Annex A.
    pub platform: Platform,
    /// `promote_integer_width` (§8.1.3); FALSE by default.
    pub promote_integer_width: bool,
    /// `string_type` (§8.1.4); `java.lang.String` by default.
    pub string_type: String,
    /// `constants_container` (§8.1.2): the alternative mapping of
    /// constants and the class it collects them in; `None` is the default
    /// mapping of §7.2.3.
    pub constants_container: Option<String>,
    /// Under [`Platform::Corba`], whether the Java of the `CORBA` module
    /// the IDL includes (`orb.idl`) is written too. Its classes are the
    /// ORB runtime's — A.1.2: "The resulting Java definitions shall be
    /// placed in the org.omg.CORBA package" — so by default an IDL file
    /// that includes `orb.idl` gets the Java of its own declarations only.
    pub corba_module: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            naming: Naming::Idl,
            platform: Platform::Dds,
            promote_integer_width: false,
            string_type: String::from("java.lang.String"),
            constants_container: None,
            corba_module: false,
        }
    }
}

/// One generated Java source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaFile {
    /// The path below the output directory: the package's directories
    /// and `<Class>.java`.
    pub path: String,
    /// The source.
    pub text: String,
}

/// Why the mapping could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Idl4JavaError {
    /// A construct the mapping gives no Java form.
    NoJavaForm {
        /// What.
        construct: String,
        /// Where.
        at: String,
    },
    /// Two IDL declarations map to one Java name (§7.1.1.2: "should
    /// report as an error collisions caused by the transformation").
    Collision {
        /// The Java name.
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

impl core::fmt::Display for Idl4JavaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoJavaForm { construct, at } => write!(
                f,
                "IDL4 to Java 1.0 gives {construct} no Java form (at `{at}`)"
            ),
            Self::Collision {
                name,
                first,
                second,
            } => write!(
                f,
                "`{first}` and `{second}` both map to the Java name `{name}` \
                 (IDL4 to Java 1.0 §7.1.1.2)"
            ),
            Self::Unevaluated { what, at } => write!(f, "{what} does not evaluate (at `{at}`)"),
            Self::Unresolved { what, at } => write!(f, "{what} does not resolve (at `{at}`)"),
        }
    }
}

impl std::error::Error for Idl4JavaError {}

/// The Java of `ast` under IDL4 to Java 1.0.
///
/// # Errors
/// [`Idl4JavaError`] for a construct with no Java form, two declarations
/// that map to one name, and a value or reference that does not
/// evaluate — never a file with a declaration left out.
pub fn generate(ast: &Ast, options: &Options) -> Result<Vec<JavaFile>, Idl4JavaError> {
    let intermediate = super::intermediate_idl::intermediate_idl(ast).map_err(|what| {
        Idl4JavaError::Unevaluated {
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
/// `@java_mapping` from the outermost module to the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    /// The naming scheme.
    pub naming: Naming,
    /// `promote_integer_width`.
    pub promote: bool,
    /// `string_type`.
    pub string_type: String,
    /// `constants_container`, when the alternative mapping is on.
    pub container: Option<String>,
}

impl Settings {
    fn of(options: &Options) -> Self {
        Self {
            naming: options.naming,
            promote: options.promote_integer_width,
            string_type: options.string_type.clone(),
            container: options.constants_container.clone(),
        }
    }

    /// The `@java_mapping` settings of one node on top.
    fn apply(&mut self, facets: &Facets) {
        for setting in &facets.mappings {
            if setting.language != MappingLanguage::Java {
                continue;
            }
            match (setting.parameter.as_str(), &setting.value) {
                ("apply_naming_convention", MappingValue::Name(name)) => {
                    if let Some(naming) = Naming::from_name(name) {
                        self.naming = naming;
                    }
                }
                ("constants_container", MappingValue::Text(name)) => {
                    self.container = Some(name.clone());
                }
                ("promote_integer_width", MappingValue::Flag(flag)) => self.promote = *flag,
                ("string_type", MappingValue::Text(name)) => self.string_type = name.clone(),
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
    files: Vec<JavaFile>,
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

impl Gen<'_> {
    fn module(&mut self, module: Id<ModuleNode>, depth: usize) -> Result<(), Idl4JavaError> {
        if depth > MAX_DEPTH {
            return Err(Idl4JavaError::Unevaluated {
                what: "a nesting of modules deeper than the mapping follows".to_string(),
                at: String::new(),
            });
        }
        let node = self
            .ast
            .module(module)
            .map_err(|_| Idl4JavaError::Unresolved {
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
                    let Ok(node) = self.ast.type_(id) else {
                        continue;
                    };
                    if node.scope.is_none() {
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
    /// scope and the built-in module left out.
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

    /// Whether `module` is the `CORBA` module of a CORBA platform: the
    /// module at the root named `CORBA` (OMG IDL 4.2 §7.4.6.4.7, CORBA 3.3
    /// part 1 §7.19), under [`Platform::Corba`].
    pub(crate) fn is_corba_module(&self, module: Id<ModuleNode>) -> bool {
        self.options.platform == Platform::Corba
            && self.ast.module(module).is_ok_and(|node| {
                node.name == "CORBA" && node.parent == Some(self.ast.root_module())
            })
    }

    /// Whether `module` is the `CORBA` module of a CORBA platform or
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
        // A.1.2: the types of the CORBA module are mapped "applying the
        // IDL Naming Scheme defined in Clause 7.1.1.1", whatever the rest
        // of the IDL is mapped with.
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
        settings
    }

    /// The settings of a type.
    pub(crate) fn settings_of_type(&self, id: Id<TypeNode>) -> Settings {
        match self.ast.type_(id) {
            Ok(node) => self.settings_in(node.module, node.scope.as_deref(), &node.facets),
            Err(_) => Settings::of(self.options),
        }
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

    /// The package segments of `module`. The CORBA module of a CORBA
    /// platform is `org.omg.CORBA` (A.1.2: "The resulting Java definitions
    /// shall be placed in the org.omg.CORBA package").
    pub(crate) fn package_of(&self, module: Id<ModuleNode>) -> Vec<String> {
        let mut package: Vec<String> = Vec::new();
        if self.in_corba_module(module) {
            package.push("org".to_string());
            package.push("omg".to_string());
        }
        package.extend(self.module_chain(module).into_iter().filter_map(|id| {
            let node = self.ast.module(id).ok()?;
            Some(self.settings_of_module(id).naming.package(&node.name))
        }));
        package
    }

    /// The Java name a scope name in `module` is written under: the
    /// interface's name, or the abstract class of a value type (§7.6).
    fn scope_class(&self, module: Id<ModuleNode>, scope: &str) -> Result<String, Idl4JavaError> {
        match self.scope_owner(module, scope) {
            Some(AnyId::Interface(id, _)) => self.interface_name(id),
            Some(AnyId::Type(id, _)) => {
                let settings = self.settings_of_type(id);
                Ok(format!(
                    "{}{}",
                    self.qualified_prefix(module)?,
                    abstract_class(&settings.naming.type_name(scope))
                ))
            }
            _ => Err(Idl4JavaError::Unresolved {
                what: format!("the interface or value type `{scope}`"),
                at: scope.to_string(),
            }),
        }
    }

    /// `pkg.` for a module, empty for the default package.
    fn qualified_prefix(&self, module: Id<ModuleNode>) -> Result<String, Idl4JavaError> {
        let package = self.package_of(module);
        Ok(if package.is_empty() {
            String::new()
        } else {
            format!("{}.", package.join("."))
        })
    }

    /// The qualified Java name of a declared type: the class a reference
    /// to it names (§7.6: "References to the value type from other
    /// classes map to references to the non-abstract class").
    pub(crate) fn qualified(&self, id: Id<TypeNode>) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a type".to_string(),
            at: String::new(),
        })?;
        let name = self.settings_of_type(id).naming.type_name(&node.name);
        match node.scope.as_deref() {
            Some(scope) => Ok(format!("{}.{name}", self.scope_class(node.module, scope)?)),
            None => Ok(format!("{}{name}", self.qualified_prefix(node.module)?)),
        }
    }

    /// The qualified Java name of an interface.
    pub(crate) fn interface_name(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an interface".to_string(),
                at: String::new(),
            })?;
        let settings = self.settings_in(node.module, None, &node.facets);
        Ok(format!(
            "{}{}",
            self.qualified_prefix(node.module)?,
            settings.naming.type_name(&node.name)
        ))
    }

    /// The qualified Java name of an exception.
    pub(crate) fn exception_name(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an exception".to_string(),
                at: String::new(),
            })?;
        let settings = self.settings_in(node.module, node.scope.as_deref(), &node.facets);
        let name = settings.naming.type_name(&node.name);
        match node.scope.as_deref() {
            Some(scope) => Ok(format!("{}.{name}", self.scope_class(node.module, scope)?)),
            None => Ok(format!("{}{name}", self.qualified_prefix(node.module)?)),
        }
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

    /// Write one top-level class of `module` into its file.
    pub(crate) fn file(
        &mut self,
        module: Id<ModuleNode>,
        class: &str,
        origin: &str,
        body: &str,
    ) -> Result<(), Idl4JavaError> {
        let package = self.package_of(module);
        let mut path = package.join("/");
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(class);
        path.push_str(".java");
        if let Some(first) = self.taken.get(&path) {
            return Err(Idl4JavaError::Collision {
                name: class.to_string(),
                first: first.clone(),
                second: origin.to_string(),
            });
        }
        self.taken.insert(path.clone(), origin.to_string());
        let mut w = Writer::new();
        w.line("// Generated from IDL by ZeroDDS: IDL4 to Java Language Mapping 1.0.");
        if !package.is_empty() {
            w.line("");
            w.line(&format!("package {};", package.join(".")));
        }
        w.line("");
        w.nested(body);
        self.files.push(JavaFile {
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

/// The abstract class of a value type, §7.6.
#[must_use]
pub fn abstract_class(name: &str) -> String {
    format!("{name}{}", names::ABSTRACT_SUFFIX)
}
