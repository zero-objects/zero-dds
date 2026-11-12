// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C++ Language Mapping 1.0 (OMG formal/25-03-03).
//!
//! `zerodds-idlc generate --lang c++ --cpp-mapping idl4` writes the C++ of
//! clauses 7 and 8 of that specification into one header: a namespace per
//! module (§7.2.2), `constexpr` constants (§7.2.3), a `struct` with
//! comparison operators and `swap` per structure (§7.2.4.3.1), a class
//! with `_d()` and accessors and modifiers per union (§7.2.4.3.2), an
//! `enum class` per enumeration (§7.2.4.3.3), `using` per typedef
//! (§7.2.4.6), an abstract class per interface (§7.4, §7.5) and value
//! type with its `_factory` (§7.6), an exception class on
//! `std::exception` (§7.4.1), a `struct` of bit fields per bit set
//! (§7.14.3.2) and a `struct` with `_flags` per bit mask (§7.14.3.3), the
//! annotations of §7.17 and `@cpp_mapping` (§8.1), and the type trait
//! specializations of §7.1.4 for what it declares.
//!
//! The default column of `--lang c++` is the DDS-PSM-Cxx binding
//! (`crate::adapter::cpp`), which carries the wire codec. This module is
//! the language mapping on its own: it writes types and interfaces, and no
//! codec. The runtime it names — `omg::types` — is
//! `runtime/cpp/include/omg/types.hpp`.
//!
//! [`Standard::Cxx03`] applies the compatibility rules of Annex C for
//! C++98 and C++03.

mod behaviour;
mod corba;
mod data;
mod order;
pub mod text;
mod types;

use std::collections::BTreeSet;
use text::Writer;
use zerodds_ast::{
    AnyId, Ast, Facets, MappingLanguage, MappingValue, ModuleNode, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;

pub use types::{CppType, Shape};

/// How deep a chain of typedefs, bases or modules is followed.
pub(crate) const MAX_DEPTH: usize = 64;

/// The platform of Annex A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    /// A.2: "DDS requires no additional platform-specific language
    /// mappings."
    #[default]
    Dds,
    /// A.1: the CORBA-specific mappings.
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

/// The C++ standard the header is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Standard {
    /// C++11 and later: clauses 7 and 8 (§7.1.3: "The language mappings
    /// defined in this specification require C++11 ... as the minimum C++
    /// standard version").
    #[default]
    Cxx11,
    /// C++98 and C++03: the rules of Annex C where they differ.
    Cxx03,
}

impl Standard {
    /// The name the command line gives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cxx11 => "c++11",
            Self::Cxx03 => "c++03",
        }
    }

    /// The standard a command-line name selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "c++11" | "c++14" | "c++17" | "c++20" | "c++23" | "11" | "14" | "17" | "20" | "23" => {
                Some(Self::Cxx11)
            }
            "c++98" | "c++03" | "98" | "03" => Some(Self::Cxx03),
            _ => None,
        }
    }
}

/// What the generator starts from before any `@cpp_mapping`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The platform of Annex A.
    pub platform: Platform,
    /// The C++ standard (§7.1.3, Annex C).
    pub standard: Standard,
    /// The header's file name; its include guard is derived from it.
    pub header: String,
    /// Under [`Platform::Corba`], whether the C++ of the `CORBA` module
    /// the IDL includes (`orb.idl`) is written too. Its types are the
    /// ORB's runtime (A.1), so by default an IDL file that includes
    /// `orb.idl` gets the C++ of its own declarations only.
    pub corba_module: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            platform: Platform::Dds,
            standard: Standard::Cxx11,
            header: "idl4.hpp".to_string(),
            corba_module: false,
        }
    }
}

/// One generated C++ source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppFile {
    /// The path below the output directory.
    pub path: String,
    /// The source.
    pub text: String,
}

/// Why the mapping could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Idl4CppError {
    /// A construct the mapping gives no C++ form.
    NoCppForm {
        /// What.
        construct: String,
        /// Where.
        at: String,
    },
    /// Two IDL declarations map to one C++ name.
    Collision {
        /// The C++ name.
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

impl core::fmt::Display for Idl4CppError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoCppForm { construct, at } => {
                write!(
                    f,
                    "IDL4 to C++ 1.0 gives {construct} no C++ form (at `{at}`)"
                )
            }
            Self::Collision {
                name,
                first,
                second,
            } => write!(
                f,
                "`{first}` and `{second}` both map to the C++ name `{name}` (IDL4 to C++ 1.0 §7.1.1)"
            ),
            Self::Unevaluated { what, at } => write!(f, "{what} does not evaluate (at `{at}`)"),
            Self::Unresolved { what, at } => write!(f, "{what} does not resolve (at `{at}`)"),
        }
    }
}

impl std::error::Error for Idl4CppError {}

/// The C++ of `ast` under IDL4 to C++ 1.0: one header.
///
/// # Errors
/// [`Idl4CppError`] for a construct with no C++ form, two declarations
/// that map to one name, and a value or reference that does not
/// evaluate — never a header with a declaration left out.
pub fn generate(ast: &Ast, options: &Options) -> Result<Vec<CppFile>, Idl4CppError> {
    let intermediate = crate::emit::intermediate_idl::intermediate_idl(ast).map_err(|what| {
        Idl4CppError::Unevaluated {
            what,
            at: String::new(),
        }
    })?;
    let ast = &intermediate;
    let generator = Gen {
        ast,
        options,
        typecodes: std::cell::RefCell::new(BTreeSet::new()),
    };
    let text = generator.header()?;
    Ok(vec![CppFile {
        path: options.header.clone(),
        text,
    }])
}

/// What one declaration is mapped with: the options, then every
/// `@cpp_mapping` from the outermost module to the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Settings {
    /// `struct_mapping = CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS`
    /// (§8.1.1).
    pub accessors: bool,
    /// `enum_prefix` (Annex C.3.1.1).
    pub enum_prefix: String,
    /// `enum_suffix` (Annex C.3.1.2).
    pub enum_suffix: String,
}

impl Settings {
    /// The `@cpp_mapping` settings of one node on top.
    fn apply(&mut self, facets: &Facets) {
        for setting in &facets.mappings {
            if setting.language != MappingLanguage::Cpp {
                continue;
            }
            match (setting.parameter.as_str(), &setting.value) {
                ("struct_mapping", MappingValue::Name(name)) => {
                    self.accessors = name == "CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS";
                }
                ("enum_prefix", MappingValue::Text(text)) => self.enum_prefix.clone_from(text),
                ("enum_suffix", MappingValue::Text(text)) => self.enum_suffix.clone_from(text),
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
    /// Under the CORBA platform, the types whose `_tc_` constant the header
    /// has defined so far: a type code that names a type not among them
    /// (a structure declared forward and completed later) names it by a
    /// recursive type code instead (A.1.3, CORBA 3.3 part 1 §8.11.3).
    pub(crate) typecodes: std::cell::RefCell<BTreeSet<Id<TypeNode>>>,
}

/// The declarations of one scope: a module's, or an interface's or value
/// type's embedded ones (§7.5).
#[derive(Default)]
pub(crate) struct Scoped {
    pub types: Vec<Id<TypeNode>>,
    pub constants: Vec<Id<zerodds_ast::ConstNode>>,
    pub exceptions: Vec<Id<zerodds_ast::ExceptionNode>>,
}

/// One top-level declaration, as the header orders them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Item {
    Type(Id<TypeNode>),
    Const(Id<zerodds_ast::ConstNode>),
    Exception(Id<zerodds_ast::ExceptionNode>),
    Interface(Id<zerodds_ast::InterfaceNode>),
}

impl Gen<'_> {
    fn cxx11(&self) -> bool {
        self.options.standard == Standard::Cxx11
    }

    /// The whole header.
    fn header(&self) -> Result<String, Idl4CppError> {
        let guard = include_guard(&self.options.header);
        let (begin_file, end_file) =
            crate::verbatim::file_slots(self.ast, crate::verbatim::aliases::CPP);
        let mut w = Writer::new();
        w.line("// Generated from IDL by ZeroDDS: IDL4 to C++ Language Mapping 1.0.");
        w.line(&format!("#ifndef {guard}"));
        w.line(&format!("#define {guard}"));
        w.line("");
        // The CORBA platform's header includes omg/types.hpp itself, after
        // it has routed the bound check to CORBA::BAD_PARAM (A.1.2.4).
        match self.options.platform {
            Platform::Dds => w.line("#include <omg/types.hpp>"),
            // An IDL file that includes `orb.idl` names the CORBA module.
            // Its declarations beyond the classes of Annex A.1 are the
            // C++ of that IDL, which omg/corba_module.hpp holds; the
            // header that is asked to write them itself is that file.
            Platform::Corba if self.has_corba_module() && !self.options.corba_module => {
                w.line("#include <omg/corba_module.hpp>");
            }
            Platform::Corba => w.line("#include <omg/corba.hpp>"),
        }
        for text in &begin_file {
            w.verbatim(text);
        }
        let items = self.items()?;
        let mut namespaces = Namespaces::default();
        // Every class is declared ahead of every definition: an interface
        // or value type is used through a reference, a structure through
        // a sequence of it, before its own definition (§7.2.4.3.4).
        for item in &items {
            if let Some((namespace, declaration)) = self.forward_declaration(*item)? {
                namespaces.enter(&mut w, &namespace);
                w.line(&declaration);
            }
        }
        for item in self.ordered(&items)? {
            let namespace = self.namespace_of_item(item)?;
            let source = self.item_source(item)?;
            if source.is_empty() {
                continue;
            }
            namespaces.enter(&mut w, &namespace);
            w.line("");
            w.nested(&source);
        }
        namespaces.enter(&mut w, &[]);
        let traits = self.trait_specializations(&items)?;
        if !traits.is_empty() && self.cxx11() {
            w.line("");
            w.open("namespace omg { namespace types");
            w.nested(&traits);
            w.close_with(" } // namespace omg::types");
        }
        if self.options.platform == Platform::Corba {
            let corba = self.corba_traits(&items)?;
            if !corba.is_empty() {
                w.line("");
                w.open("namespace CORBA");
                w.nested(&corba);
                w.close_with(" // namespace CORBA");
            }
        }
        for text in &end_file {
            w.verbatim(text);
        }
        w.line("");
        w.line(&format!("#endif // {guard}"));
        let text = w.finish();
        Ok(match self.options.standard {
            Standard::Cxx11 => text,
            // C++03 reads `<:` as the digraph of `[` (ISO/IEC 14882:2003
            // §2.5), so a template argument list that starts with a
            // qualified name `<::a::B>` needs the space C++11 made
            // unnecessary.
            Standard::Cxx03 => text.replace("<::", "< ::"),
        })
    }

    /// Every top-level declaration of the model, outside the CORBA
    /// module unless it is asked for.
    fn items(&self) -> Result<Vec<Item>, Idl4CppError> {
        let mut out = Vec::new();
        self.collect(self.ast.root_module(), 0, &mut out)?;
        Ok(out)
    }

    fn collect(
        &self,
        module: Id<ModuleNode>,
        depth: usize,
        out: &mut Vec<Item>,
    ) -> Result<(), Idl4CppError> {
        if depth > MAX_DEPTH {
            return Err(Idl4CppError::Unevaluated {
                what: "a nesting of modules deeper than the mapping follows".to_string(),
                at: String::new(),
            });
        }
        let node = self
            .ast
            .module(module)
            .map_err(|_| unresolved("a module"))?;
        for child in &node.children {
            match child {
                AnyId::Module(id, _) if self.is_corba_module(*id) && !self.options.corba_module => {
                }
                AnyId::Module(id, _) => self.collect(*id, depth + 1, out)?,
                AnyId::Type(id, _) => {
                    if self
                        .ast
                        .type_(*id)
                        .is_ok_and(|t| t.scope.is_none() && !self.runtime_holds(t.module, &t.name))
                    {
                        out.push(Item::Type(*id));
                    }
                }
                AnyId::Const(id, _) => {
                    if self
                        .ast
                        .constant(*id)
                        .is_ok_and(|c| c.scope.is_none() && !self.runtime_holds(c.module, &c.name))
                        && !self.is_template_binding(*id)
                    {
                        out.push(Item::Const(*id));
                    }
                }
                AnyId::Exception(id, _) => {
                    if self
                        .ast
                        .exception(*id)
                        .is_ok_and(|e| e.scope.is_none() && !self.runtime_holds(e.module, &e.name))
                    {
                        out.push(Item::Exception(*id));
                    }
                }
                // A port type is the grouping of basic ports the ports
                // that name it were replaced by (§7.12); it declares
                // nothing of its own in the intermediate IDL.
                AnyId::Interface(id, _) => {
                    let left_out = self.ast.interface(*id).is_ok_and(|node| {
                        node.kind == zerodds_ast::InterfaceKind::PortType
                            || self.runtime_holds(node.module, &node.name)
                    });
                    if !left_out {
                        out.push(Item::Interface(*id));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Whether a constant is the binding of a template module's `const`
    /// formal parameter in an instance (OMG IDL 4.2 §7.4.12.4.2): no
    /// declaration of the instance.
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

    /// Whether `name`, declared in `module`, is a declaration of the
    /// `CORBA` module that `omg/corba.hpp` holds: the classes Annex A.1
    /// defines itself (A.1.2 the exceptions, A.1.3 `TypeCode` and what
    /// its declaration uses) instead of deriving them from IDL.
    pub(crate) fn runtime_holds(&self, module: Id<ModuleNode>, name: &str) -> bool {
        self.is_corba_module(module) && corba::RUNTIME_HOLDS.contains(&name)
    }

    /// Whether the model declares the `CORBA` module: the IDL included
    /// `orb.idl` (OMG IDL 4.2 §7.4.6.4.7).
    pub(crate) fn has_corba_module(&self) -> bool {
        self.ast.module(self.ast.root_module()).is_ok_and(|root| {
            root.children
                .iter()
                .any(|child| matches!(child, AnyId::Module(id, _) if self.is_corba_module(*id)))
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
        let mut settings = Settings::default();
        if let Ok(root) = self.ast.module(self.ast.root_module()) {
            settings.apply(&root.facets);
        }
        for id in self.module_chain(module) {
            if let Ok(node) = self.ast.module(id) {
                settings.apply(&node.facets);
            }
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
            Err(_) => Settings::default(),
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

    /// The namespace segments of `module`.
    pub(crate) fn namespace_of(&self, module: Id<ModuleNode>) -> Vec<String> {
        self.module_chain(module)
            .into_iter()
            .filter_map(|id| self.ast.module(id).ok().map(|node| names::name(&node.name)))
            .collect()
    }

    /// `::a::b::` for a module, `::` for the global namespace.
    pub(crate) fn qualified_prefix(&self, module: Id<ModuleNode>) -> String {
        let mut out = String::from("::");
        for segment in self.namespace_of(module) {
            out.push_str(&segment);
            out.push_str("::");
        }
        out
    }

    /// The qualified C++ name of a declared type.
    pub(crate) fn qualified(&self, id: Id<TypeNode>) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        let name = names::name(&node.name);
        match node.scope.as_deref() {
            Some(scope) => Ok(format!(
                "{}{}::{name}",
                self.qualified_prefix(node.module),
                names::name(scope)
            )),
            None => Ok(format!("{}{name}", self.qualified_prefix(node.module))),
        }
    }

    /// The qualified C++ name of an interface.
    pub(crate) fn interface_name(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        Ok(format!(
            "{}{}",
            self.qualified_prefix(node.module),
            names::name(&node.name)
        ))
    }

    /// The qualified C++ name of an exception.
    pub(crate) fn exception_name(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception"))?;
        let name = names::name(&node.name);
        match node.scope.as_deref() {
            Some(scope) => Ok(format!(
                "{}{}::{name}",
                self.qualified_prefix(node.module),
                names::name(scope)
            )),
            None => Ok(format!("{}{name}", self.qualified_prefix(node.module))),
        }
    }

    /// The declarations embedded in the interface or value type `scope`
    /// of `module`.
    pub(crate) fn embedded(&self, module: Id<ModuleNode>, scope: &str) -> Scoped {
        let mut out = Scoped::default();
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

    /// The module an item is declared in.
    fn module_of_item(&self, item: Item) -> Result<Id<ModuleNode>, Idl4CppError> {
        match item {
            Item::Type(id) => Ok(self.ast.type_(id).map_err(|_| unresolved("a type"))?.module),
            Item::Const(id) => Ok(self
                .ast
                .constant(id)
                .map_err(|_| unresolved("a constant"))?
                .module),
            Item::Exception(id) => Ok(self
                .ast
                .exception(id)
                .map_err(|_| unresolved("an exception"))?
                .module),
            Item::Interface(id) => Ok(self
                .ast
                .interface(id)
                .map_err(|_| unresolved("an interface"))?
                .module),
        }
    }

    fn namespace_of_item(&self, item: Item) -> Result<Vec<String>, Idl4CppError> {
        Ok(self.namespace_of(self.module_of_item(item)?))
    }

    /// `class X;` or `struct X;` for an item that is a class, in its
    /// namespace.
    fn forward_declaration(
        &self,
        item: Item,
    ) -> Result<Option<(Vec<String>, String)>, Idl4CppError> {
        let key_and_name = match item {
            Item::Type(id) => {
                let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
                let key = match node.kind {
                    TypeKind::Structure { .. } => {
                        if self.settings_of_type(id).accessors {
                            "class"
                        } else {
                            "struct"
                        }
                    }
                    TypeKind::Bitset { .. } | TypeKind::Bitmask { .. } => "struct",
                    TypeKind::Union { .. }
                    | TypeKind::Valuetype { .. }
                    | TypeKind::ValueBox { .. } => "class",
                    _ => return Ok(None),
                };
                Some((key, names::name(&node.name)))
            }
            Item::Interface(id) => {
                let node = self
                    .ast
                    .interface(id)
                    .map_err(|_| unresolved("an interface"))?;
                Some(("class", names::name(&node.name)))
            }
            Item::Const(_) | Item::Exception(_) => None,
        };
        let Some((key, name)) = key_and_name else {
            return Ok(None);
        };
        Ok(Some((
            self.namespace_of_item(item)?,
            format!("{key} {name};"),
        )))
    }

    /// The source of one item.
    fn item_source(&self, item: Item) -> Result<String, Idl4CppError> {
        let corba = self.options.platform == Platform::Corba;
        match item {
            Item::Type(id) => {
                let mut source = self.type_declaration(id)?;
                // A.1.3 and A.1.7: the type code and the `Any` operators,
                // at the scope of the type.
                if corba
                    && !source.is_empty()
                    && let Some(extras) = self.corba_type_extras(id)?
                {
                    source.push('\n');
                    source.push_str(&Self::extras_text(&extras));
                }
                Ok(source)
            }
            Item::Const(id) => self.constant(id, false),
            Item::Exception(id) => {
                let mut source = self.exception(id)?;
                if corba {
                    source.push('\n');
                    source.push_str(&Self::extras_text(&self.corba_exception_extras(id)?));
                }
                Ok(source)
            }
            Item::Interface(id) => {
                let mut source = self.interface(id)?;
                if corba && !source.is_empty() {
                    source.push('\n');
                    source.push_str(&Self::extras_text(&self.corba_interface_extras(id)?));
                }
                Ok(source)
            }
        }
    }

    /// The scoped name of a node, for messages.
    pub(crate) fn origin(&self, any: AnyId) -> String {
        self.ast
            .scoped_name_of(any)
            .map_or_else(String::new, |name| name.segments().join("::"))
    }

    /// The types the header declares, for the trait specializations.
    fn declared_types(&self, items: &[Item]) -> BTreeSet<Id<TypeNode>> {
        let mut out = BTreeSet::new();
        for item in items {
            if let Item::Type(id) = item {
                out.insert(*id);
            }
            let module = self.module_of_item(*item).ok();
            let scope = match item {
                Item::Interface(id) => self.ast.interface(*id).ok().map(|n| n.name.clone()),
                Item::Type(id) => self.ast.type_(*id).ok().and_then(|n| {
                    matches!(n.kind, TypeKind::Valuetype { .. }).then(|| n.name.clone())
                }),
                _ => None,
            };
            if let (Some(module), Some(scope)) = (module, scope) {
                for id in self.embedded(module, &scope).types {
                    out.insert(id);
                }
            }
        }
        out
    }
}

/// The namespaces the writer is in, entered and left as items need.
#[derive(Default)]
struct Namespaces {
    open: Vec<String>,
}

impl Namespaces {
    fn enter(&mut self, w: &mut Writer, target: &[String]) {
        let common = self
            .open
            .iter()
            .zip(target)
            .take_while(|(a, b)| a == b)
            .count();
        while self.open.len() > common {
            let name = self.open.pop().unwrap_or_default();
            w.close_with(&format!(" // namespace {name}"));
        }
        for segment in &target[common..] {
            w.line("");
            w.open(&format!("namespace {segment}"));
            self.open.push(segment.clone());
        }
    }
}

/// `ZERODDS_IDL4_<NAME>_HPP` from the header's file name.
fn include_guard(header: &str) -> String {
    let mut out = String::from("ZERODDS_IDL4_");
    for c in header.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

pub(crate) fn unresolved(what: &str) -> Idl4CppError {
    Idl4CppError::Unresolved {
        what: what.to_string(),
        at: String::new(),
    }
}

/// The targets of `references`. One the model did not resolve is an error
/// that names `what`: a base left out is a class without its inheritance,
/// and the header would compile.
pub(crate) fn resolved_all<T>(
    references: &[zerodds_ast_core::Ref<T>],
    what: &str,
) -> Result<Vec<Id<T>>, Idl4CppError> {
    references
        .iter()
        .map(|reference| reference.resolved().ok_or_else(|| unresolved(what)))
        .collect()
}

/// Names: §7.1.1 and §7.1.2.
pub mod names {
    /// The C++ keywords of §7.1.2, and those of later C++ versions the list
    /// "includes but is not limited to".
    pub const RESERVED: &[&str] = &[
        "alignas",
        "alignof",
        "and",
        "and_eq",
        "asm",
        "atomic_cancel",
        "atomic_commit",
        "atomic_noexcept",
        "auto",
        "bitand",
        "bitor",
        "bool",
        "break",
        "case",
        "catch",
        "char",
        "char8_t",
        "char16_t",
        "char32_t",
        "class",
        "compl",
        "concept",
        "const",
        "consteval",
        "constexpr",
        "constinit",
        "const_cast",
        "continue",
        "co_await",
        "co_return",
        "co_yield",
        "decltype",
        "default",
        "delete",
        "do",
        "double",
        "dynamic_cast",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "false",
        "float",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "mutable",
        "namespace",
        "new",
        "noexcept",
        "not",
        "not_eq",
        "nullptr",
        "operator",
        "or",
        "or_eq",
        "private",
        "protected",
        "public",
        "reflexpr",
        "register",
        "reinterpret_cast",
        "requires",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "static_assert",
        "static_cast",
        "struct",
        "switch",
        "synchronized",
        "template",
        "this",
        "thread_local",
        "throw",
        "true",
        "try",
        "typedef",
        "typeid",
        "typename",
        "union",
        "unsigned",
        "using",
        "virtual",
        "void",
        "volatile",
        "wchar_t",
        "while",
        "xor",
        "xorg",
        "xor_eq",
    ];

    /// The C++ name of an IDL identifier: "with no change" (§7.1.1), with
    /// "an underscore ("_") prepended" when it is one of the reserved
    /// names of §7.1.2. The clause states the prefix for type and
    /// interface names; it is applied to every name here, because a member
    /// or parameter named `class` is no C++ either.
    #[must_use]
    pub fn name(idl: &str) -> String {
        if RESERVED.contains(&idl) {
            format!("_{idl}")
        } else {
            idl.to_string()
        }
    }
}
