// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Scopes and name binding. Spec §4.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_ast::{AnyId, ModuleNode};
use zerodds_ast_core::{Id, ScopedName};

/// How deep a chain of bases is followed before it is taken for a cycle.
const MAX_INHERITANCE_DEPTH: usize = 64;

/// What a scope has declared directly in it, in declaration order.
type Bindings = Vec<(String, AnyId)>;

/// What a name written in a scope denotes. OMG IDL 4.2 §7.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// One declaration.
    One(AnyId),
    /// Two or more different declarations reached through inheritance,
    /// none of them declared closer: §7.4.4.4 "A reference to a base
    /// interface element is ambiguous if the name is declared as a
    /// constant, type, or exception in more than one base interface."
    Ambiguous(Vec<AnyId>),
    /// No declaration is visible.
    Nothing,
}

/// The body of an interface, value type or other container, kept after it
/// closes so that a derived one can search it. OMG IDL 4.2 §7.5.2: "Scope
/// of N::Y's base interface M::B (inherited scope)."
#[derive(Debug, Clone)]
struct ClosedContainer {
    /// Its absolute path, outermost first.
    path: Vec<String>,
    /// What was declared directly in it.
    bindings: Bindings,
    /// The absolute paths of its direct bases.
    bases: Vec<Vec<String>>,
}

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
    /// The absolute paths of the direct bases of an interface or value
    /// type, searched after its own bindings. See [`ScopeStack::set_bases`].
    bases: Vec<Vec<String>>,
    /// The identifiers a use introduced into this scope, with what they
    /// denote. OMG IDL 4.2 §7.5.2. See [`ScopeStack::introduce`].
    introduced: Bindings,
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
    /// Every interface and value type body left so far, for the scope
    /// search through inheritance.
    containers: Vec<ClosedContainer>,
    /// Whether two names that differ only in case are two names
    /// (`Dialect::case_sensitive`); OMG IDL 4.2 §7.2.3 has them collide.
    case_sensitive: bool,
}

impl ScopeStack {
    /// Compare names for a collision with regard to case.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    /// Whether `a` and `b` are one name where a collision is decided.
    fn collide(&self, a: &str, b: &str) -> bool {
        if self.case_sensitive {
            a == b
        } else {
            a.eq_ignore_ascii_case(b)
        }
    }

    /// A stack that has not been given its root yet.
    ///
    /// Only for construction order: the lowering needs a context before
    /// it can ask the model for its root module.
    #[must_use]
    pub fn new_placeholder() -> Self {
        Self {
            scopes: Vec::new(),
            closed: Vec::new(),
            containers: Vec::new(),
            case_sensitive: false,
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
                bases: Vec::new(),
                introduced: Vec::new(),
            }],
            closed: Vec::new(),
            containers: Vec::new(),
            case_sensitive: false,
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
            bases: Vec::new(),
            introduced: Vec::new(),
        });
    }

    /// Enter the body of a structure, union or exception.
    ///
    /// Nothing is declared in it that a lookup could find — a member name
    /// is not a declaration other names resolve to — and it names nothing,
    /// so it adds no segment to [`Self::qualify`]. It exists for what a
    /// use introduces: OMG IDL 4.2 §7.5.3 gives a type used in a structure
    /// a scope that ends with the structure ("Scope of (introduced) M ends
    /// here") and that does not reach the module around it ("Redefinition
    /// of a type after use in a module is OK").
    pub fn push_body(&mut self) {
        self.scopes.push(Scope {
            module: None,
            name: String::new(),
            container: false,
            bindings: Vec::new(),
            bases: Vec::new(),
            introduced: Vec::new(),
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
            bases: Vec::new(),
            introduced: Vec::new(),
        });
    }

    /// Give the innermost scope, an interface or value type body, its
    /// direct bases: absolute paths, outermost segment first.
    ///
    /// OMG IDL 4.2 §7.5.2 searches "Scope of N::Y's base interface M::B
    /// (inherited scope)" after the scope of `N::Y` and before the module
    /// around it, and §7.4.4.4 "In a derived interface, all elements of a
    /// base class may be referred to as if they were elements of the
    /// derived class, unless they are redefined in the derived class."
    pub fn set_bases(&mut self, bases: Vec<Vec<String>>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bases = bases;
        }
    }

    /// The absolute path of the innermost named scope, outermost segment
    /// first; empty at the root.
    #[must_use]
    pub fn path_here(&self) -> Vec<String> {
        self.scopes
            .iter()
            .filter(|scope| !scope.name.is_empty())
            .map(|scope| scope.name.clone())
            .collect()
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
        let path = self.path_here();
        let Some(scope) = self.scopes.pop() else {
            return;
        };
        if scope.container {
            let closed = ClosedContainer {
                path,
                bindings: scope.bindings,
                bases: scope.bases,
            };
            match self
                .containers
                .iter_mut()
                .find(|known| known.path == closed.path)
            {
                Some(known) => *known = closed,
                None => self.containers.push(closed),
            }
            return;
        }
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
                .any(|(bound, _)| self.collide(bound, name))
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

    /// Look `name` up, innermost scope outwards, through the bases of
    /// every interface or value type on the way.
    ///
    /// `None` both when nothing is visible and when the name is
    /// ambiguous; [`Self::find`] tells the two apart.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<AnyId> {
        match self.find(name).0 {
            Found::One(node) => Some(node),
            Found::Ambiguous(_) | Found::Nothing => None,
        }
    }

    /// What `name`, written in the innermost scope, denotes, and where the
    /// search found it.
    ///
    /// OMG IDL 4.2 §7.5.2: "A name can be used in an unqualified form
    /// within a particular scope; it will be resolved by successively
    /// searching farther out in enclosing scopes, while taking into
    /// consideration inheritance relationships among interfaces." For
    /// each scope from the innermost outwards, its own declarations and
    /// then, for an interface or value type, the declarations of its
    /// bases (§7.5.2's steps 1 to 4). Two different declarations reached
    /// through two bases, with none closer, are [`Found::Ambiguous`];
    /// the same declaration reached along two paths is one (§7.5.1:
    /// "Multiple paths to the same original identifier … do not conflict
    /// with each other").
    ///
    /// The second element is the index of the scope the search stopped
    /// in and whether the name came from that scope's bases rather than
    /// from the scope itself.
    #[must_use]
    pub fn find(&self, name: &str) -> (Found, Option<(usize, bool)>) {
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some((_, node)) = scope.bindings.iter().find(|(bound, _)| bound == name) {
                return (Found::One(*node), Some((index, false)));
            }
            if scope.bases.is_empty() {
                continue;
            }
            let mut found: Vec<AnyId> = Vec::new();
            let mut visited: Vec<Vec<String>> = Vec::new();
            for base in &scope.bases {
                self.find_inherited(base, name, &mut found, &mut visited, 0);
            }
            match found.len() {
                0 => {}
                1 => return (Found::One(found[0]), Some((index, true))),
                _ => return (Found::Ambiguous(found), Some((index, true))),
            }
        }
        (Found::Nothing, None)
    }

    /// What `name` denotes inside the interface or value type at `path`:
    /// its own declaration, else one of its bases'. [`Found::Nothing`] when
    /// no container is known at `path`.
    #[must_use]
    pub fn find_in(&self, path: &[String], name: &str) -> Found {
        let mut found: Vec<AnyId> = Vec::new();
        let mut visited: Vec<Vec<String>> = Vec::new();
        self.find_inherited(path, name, &mut found, &mut visited, 0);
        match found.len() {
            0 => Found::Nothing,
            1 => Found::One(found[0]),
            _ => Found::Ambiguous(found),
        }
    }

    /// Search one base and, where it does not declare `name`, its own
    /// bases: a base that redefines a name hides what its bases declare
    /// under it (§7.4.4.4, "unless they are redefined").
    fn find_inherited(
        &self,
        base: &[String],
        name: &str,
        found: &mut Vec<AnyId>,
        visited: &mut Vec<Vec<String>>,
        depth: usize,
    ) {
        // Bounded: a cycle of bases is an error of its own, and a lookup
        // must not spin on it.
        if depth > MAX_INHERITANCE_DEPTH || visited.iter().any(|seen| seen == base) {
            return;
        }
        visited.push(base.to_vec());
        let Some((bindings, bases)) = self.container_at(base) else {
            return;
        };
        if let Some((_, node)) = bindings.iter().find(|(bound, _)| bound == name) {
            if !found.contains(node) {
                found.push(*node);
            }
            return;
        }
        for inner in bases {
            self.find_inherited(inner, name, found, visited, depth + 1);
        }
    }

    /// The declarations and the bases of the interface or value type at
    /// `path`: a closed one, or one whose body is still open — a name
    /// written inside `B` may go through `B` itself (`::B::E`).
    fn container_at(&self, path: &[String]) -> Option<(&Bindings, &Vec<Vec<String>>)> {
        if let Some(known) = self.containers.iter().find(|known| known.path == path) {
            return Some((&known.bindings, &known.bases));
        }
        let mut current: Vec<String> = Vec::new();
        for scope in &self.scopes {
            if !scope.name.is_empty() {
                current.push(scope.name.clone());
            }
            if scope.container && current == path {
                return Some((&scope.bindings, &scope.bases));
            }
        }
        None
    }

    /// The direct and indirect bases of the container at `path`,
    /// nearest first, each once.
    #[must_use]
    pub fn bases_of(&self, path: &[String]) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        let mut queue: Vec<Vec<String>> = self
            .container_at(path)
            .map_or_else(Vec::new, |(_, bases)| bases.clone());
        while let Some(next) = (!queue.is_empty()).then(|| queue.remove(0)) {
            if out.contains(&next) || out.len() > MAX_INHERITANCE_DEPTH {
                continue;
            }
            if let Some((_, bases)) = self.container_at(&next) {
                queue.extend(bases.iter().cloned());
            }
            out.push(next);
        }
        out
    }

    /// Record that `name`, written in the innermost scope, was found at
    /// `at` (the second element of [`Self::find`]) and denotes `node`.
    ///
    /// OMG IDL 4.2 §7.5.2: "An identifier from a surrounding scope is
    /// introduced into a scope if it is used in that scope." §7.5.3: "if
    /// a type name is introduced into a scope that is nested in a
    /// non-module scope definition its potential scope extends over all
    /// its enclosing scopes out to the enclosing non-module scope." So
    /// the innermost scope takes the name, and so does every enclosing
    /// scope as long as the chain of non-module scopes lasts, up to (not
    /// into) the scope that declares the name. A name found in the
    /// innermost scope's own declarations is not introduced: it was
    /// declared there, not brought in.
    pub fn introduce(&mut self, name: &str, node: AnyId, at: (usize, bool)) {
        let (index, inherited) = at;
        let Some(top) = self.scopes.len().checked_sub(1) else {
            return;
        };
        // The lowest scope that may take the name: the declaring scope
        // itself takes it only when the name came from its bases.
        let lowest = if inherited { index } else { index + 1 };
        if top < lowest {
            return;
        }
        let mut current = top;
        loop {
            let scope = &mut self.scopes[current];
            if !scope.introduced.iter().any(|(known, _)| known == name) {
                scope.introduced.push((name.to_string(), node));
            }
            let is_module = scope.module.is_some();
            if is_module || current == 0 || current - 1 < lowest {
                break;
            }
            if self.scopes[current - 1].module.is_some() {
                break;
            }
            current -= 1;
        }
    }

    /// What `name` was introduced into the innermost scope as, if it was,
    /// compared case-insensitively (§7.2.3). [`Self::introduce`].
    #[must_use]
    pub fn introduced_here(&self, name: &str) -> Option<(&str, AnyId)> {
        self.scopes.last().and_then(|scope| {
            scope
                .introduced
                .iter()
                .find(|(known, _)| self.collide(known, name))
                .map(|(known, node)| (known.as_str(), *node))
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
