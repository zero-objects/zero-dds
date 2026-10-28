// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Scopes and name binding. Spec §4.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_ast::{AnyId, ModuleNode};
use zerodds_ast_core::{Id, ScopedName};

/// What a scope has declared directly in it, in declaration order.
type Bindings = Vec<(String, AnyId)>;

/// One lexical scope.
#[derive(Debug, Clone)]
struct Scope {
    /// The module this scope corresponds to, if it is one.
    module: Option<Id<ModuleNode>>,
    /// The name of this scope, empty for the root.
    name: String,
    /// Whether this is an interface or a value type, whose name is a
    /// segment of what is declared in it. See [`ScopeStack::push_container`].
    container: bool,
    /// What was declared directly in it.
    bindings: Bindings,
}

/// The scope stack.
///
/// IDL name lookup walks outwards from the innermost scope, which is
/// what makes a type usable by its simple name inside its own module and
/// by a scoped name outside. OMG IDL 4.2 §7.5.
#[derive(Debug, Clone)]
pub struct ScopeStack {
    scopes: Vec<Scope>,
    /// What each module scope held when it was last left.
    ///
    /// A module may be reopened — OMG IDL 4.2 §7.4.2 — and the second
    /// opening is the same scope as the first, not a fresh one. Without
    /// this, a scope left behind takes its bindings with it, and
    /// `module common { struct First {…}; }; module common { struct S {
    /// First f; }; };` fails on a name the first opening declared.
    closed: Vec<(Id<ModuleNode>, Bindings)>,
}

impl ScopeStack {
    /// A stack that has not been given its root yet.
    ///
    /// Only for construction order: the lowering needs a context before
    /// it can ask the model for its root module.
    #[must_use]
    pub fn new_placeholder() -> Self {
        Self {
            scopes: Vec::new(),
            closed: Vec::new(),
        }
    }

    /// A stack holding only the global scope.
    #[must_use]
    pub fn new(root: Id<ModuleNode>) -> Self {
        Self {
            scopes: alloc::vec![Scope {
                module: Some(root),
                name: String::new(),
                container: false,
                bindings: Vec::new(),
            }],
            closed: Vec::new(),
        }
    }

    /// Enter a named scope.
    ///
    /// Reopening a module resumes it: the scope starts with everything
    /// the previous opening left in it, so an unqualified reference
    /// reaches back across the `};` and a name declared twice is still
    /// a collision.
    pub fn push(&mut self, name: &str, module: Option<Id<ModuleNode>>) {
        let bindings = module
            .and_then(|id| self.closed.iter().find(|(closed, _)| *closed == id))
            .map_or_else(Vec::new, |(_, bindings)| bindings.clone());
        self.scopes.push(Scope {
            module,
            name: name.to_string(),
            container: false,
            bindings,
        });
    }

    /// Enter the body of an interface or a value type.
    ///
    /// A scope of its own for lookup like any other, and in addition one
    /// that names what is declared in it: the scoped name and the
    /// repository id of `exception NotFound` in `interface NamingContext`
    /// in `module CosNaming` go through all three (OMG IDL 4.2 §7.5,
    /// CORBA 3.3 part 1 §14.7.1.1). An annotation declaration's body is a
    /// scope that names nothing and is entered with [`Self::push`].
    pub fn push_container(&mut self, name: &str) {
        self.scopes.push(Scope {
            module: None,
            name: name.to_string(),
            container: true,
            bindings: Vec::new(),
        });
    }

    /// The interface or value type a declaration made now sits in, when
    /// the innermost scope is one.
    #[must_use]
    pub fn enclosing_container(&self) -> Option<&str> {
        self.scopes
            .last()
            .filter(|scope| scope.container)
            .map(|scope| scope.name.as_str())
    }

    /// Leave the innermost scope.
    pub fn pop(&mut self) {
        // The global scope is never popped: a stray `}` would otherwise
        // leave the stack empty and every later lookup would fail for
        // the wrong reason.
        if self.scopes.len() <= 1 {
            return;
        }
        let Some(scope) = self.scopes.pop() else {
            return;
        };
        // A module scope is kept so a later opening can resume it. The
        // bindings it carries already include what earlier openings put
        // there, `push` having seeded them, so this replaces rather than
        // appends.
        let Some(module) = scope.module else {
            return;
        };
        match self.closed.iter_mut().find(|(closed, _)| *closed == module) {
            Some((_, stored)) => *stored = scope.bindings,
            None => self.closed.push((module, scope.bindings)),
        }
    }

    /// The module the innermost enclosing scope belongs to.
    #[must_use]
    pub fn current_module(&self) -> Option<Id<ModuleNode>> {
        self.scopes.iter().rev().find_map(|scope| scope.module)
    }

    /// Bind `name` in the innermost scope.
    pub fn bind(&mut self, name: &str, node: AnyId) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.push((name.to_string(), node));
        }
    }

    /// Whether `name` is already bound in the innermost scope.
    ///
    /// Case-insensitively, because IDL treats two identifiers differing
    /// only by case as the same name for collision purposes.
    /// OMG IDL 4.2 §7.2.3.
    #[must_use]
    pub fn is_bound_here(&self, name: &str) -> bool {
        self.scopes.last().is_some_and(|scope| {
            scope
                .bindings
                .iter()
                .any(|(bound, _)| bound.eq_ignore_ascii_case(name))
        })
    }

    /// Look `name` up in the innermost scope only, matching exactly.
    ///
    /// Exact rather than case-insensitive: completing a forward
    /// declaration requires the same name, while two names differing
    /// only by case are a collision and have to be reported as one.
    #[must_use]
    pub fn lookup_here(&self, name: &str) -> Option<AnyId> {
        self.scopes.last().and_then(|scope| {
            scope
                .bindings
                .iter()
                .find(|(bound, _)| bound == name)
                .map(|(_, node)| *node)
        })
    }

    /// Look `name` up, innermost scope outwards.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<AnyId> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .bindings
                .iter()
                .find(|(bound, _)| bound == name)
                .map(|(_, node)| *node)
        })
    }

    /// The scoped name a declaration in the innermost scope would have.
    #[must_use]
    pub fn qualify(&self, name: &str) -> ScopedName {
        let mut segments: Vec<String> = self
            .scopes
            .iter()
            .filter(|scope| !scope.name.is_empty())
            .map(|scope| scope.name.clone())
            .collect();
        segments.push(name.to_string());
        ScopedName::absolute(segments)
    }

    /// The scoped name a written name has from the current scope.
    ///
    /// A directive inside a module names its target relatively, so the
    /// enclosing scope's path comes first. IDL 4.2 §7.5.
    #[must_use]
    pub fn qualify_segments(&self, segments: &[String]) -> ScopedName {
        let mut path: Vec<String> = self
            .scopes
            .iter()
            .filter(|scope| !scope.name.is_empty())
            .map(|scope| scope.name.clone())
            .collect();
        path.extend_from_slice(segments);
        ScopedName::absolute(path)
    }

    /// Every scoped name a written name could denote, innermost first.
    ///
    /// §7.5: a name is looked up in the scope that writes it and then in
    /// each enclosing scope outwards, and the first declaration found is
    /// the one it means. [`Self::qualify_segments`] gives only the first
    /// of those, which is right for a declaration being made and wrong
    /// for a name being *used*: `module m { typeprefix m "…"; };` named
    /// `m::m`, a scope that does not exist, and the directive was
    /// reported as naming nothing while `::m` sat one step out.
    ///
    /// The list always ends at the root, so a name declared at top level
    /// is reachable from any depth.
    #[must_use]
    pub fn lookup_candidates(&self, segments: &[String]) -> Vec<ScopedName> {
        let enclosing: Vec<String> = self
            .scopes
            .iter()
            .filter(|scope| !scope.name.is_empty())
            .map(|scope| scope.name.clone())
            .collect();
        let mut out = Vec::with_capacity(enclosing.len() + 1);
        for depth in (0..=enclosing.len()).rev() {
            let mut path = enclosing[..depth].to_vec();
            path.extend_from_slice(segments);
            out.push(ScopedName::absolute(path));
        }
        out
    }

    /// The name of the innermost named scope, if there is one.
    ///
    /// Empty for the root, whose scope carries no name. IDL 4.2 §7.5.2
    /// forbids redefining that name inside it, and the rule needs to
    /// know what it is.
    #[must_use]
    pub fn innermost_name(&self) -> Option<&str> {
        self.scopes
            .last()
            .map(|scope| scope.name.as_str())
            .filter(|name| !name.is_empty())
    }

    /// How deep the stack is, the global scope counting as one.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}
