// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OMG CIDL and the PSDL declarations it contains.
//!
//! CORBA 3.3 Part 3 §8.3: "The CIDL grammar is a combination of the PSDL
//! and IDL grammars plus the following productions." The PSDL part is
//! Persistent State Service 2.0 §3.2.3, productions (1) to (48); the
//! CIDL part is §8.3, productions (1) to (34). Both are reached only
//! under `Dialect::cidl`, whose keywords the lexer never produces
//! (`Parser::mark_cidl_keywords`).
//!
//! Where the clause text and the grammar of §8 disagree, both forms are
//! read and the tree keeps what was written:
//!
//! - §8.14 names the keywords of a feature delegation `delegatesTo`,
//!   `abstract` and `storagetype`; production (20) writes
//!   `delegatesTo storage`. Either is a feature delegation.
//! - §9.2.9.1 gives "the normative form for the declaration of a
//!   segmented executor" as `segment S { storedOn H; provides (a, b);
//!   };`; productions (16) and (19) write `provides facet a, b;` and no
//!   `;` after the segment. Both are read.
//! - Production (17) puts a `;` after `<segment_persistence_dcl>`,
//!   which (18) already ends with one; one `;` ends it.
//! - Production (11) writes `bindsTo <identifier>`, §8.8 `bindsTo
//!   <abstract_storagehome_name>`, a scoped name; the scoped name is read.
//!
//! The catalogs of the §9.2.7 examples (`uses catalog { … }`,
//! `bindsTo store.TSHome`) belong to PSS 1.0 and are in neither grammar;
//! they are syntax errors here.

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use crate::parser::Parser;
use alloc::string::ToString;
use zerodds_idl_cst::{CstKind, Span, TokenKind};

/// The clause a CIDL syntax diagnostic cites.
const CIDL: &str = "CORBA 3.3 Part 3 §8.3";
/// The clause a PSDL syntax diagnostic cites.
const PSDL: &str = "Persistent State Service 2.0 §3.2.3";

impl Parser<'_> {
    // -- PSDL ------------------------------------------------------------

    /// `abstract storagetype …` or `abstract storagehome …`.
    pub(crate) fn parse_abstract_storage(&mut self) {
        if self.peek(1) == TokenKind::Storagetype {
            self.parse_abstract_storagetype();
        } else {
            self.parse_abstract_storagehome();
        }
    }

    /// Productions (5) to (12): an abstract storage type, or its forward
    /// declaration.
    fn parse_abstract_storagetype(&mut self) {
        self.start(CstKind::AbstractStorageTypeDecl);
        self.bump(); // abstract
        self.bump(); // storagetype
        self.expect_name("an abstract storage type name", PSDL);
        if self.current() == TokenKind::Colon {
            self.parse_name_list_clause(CstKind::InheritanceClause);
        }
        if self.at_body() {
            self.bump(); // {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                match self.current() {
                    TokenKind::Readonly | TokenKind::State => self.parse_state_decl(),
                    _ => self.parse_psdl_local_op(true),
                }
                if self.remaining() == before {
                    self.error_token("expected a state member or an operation");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the abstract storage type");
        }
        self.expect(TokenKind::Semicolon, "`;` after the abstract storage type");
        self.finish();
    }

    /// Productions (16) to (22): an abstract storage home, or its forward
    /// declaration.
    fn parse_abstract_storagehome(&mut self) {
        self.start(CstKind::AbstractStorageHomeDecl);
        self.bump(); // abstract
        self.bump(); // storagehome
        self.expect_name("an abstract storage home name", PSDL);
        if self.current() == TokenKind::Semicolon {
            self.bump();
            self.finish();
            return;
        }
        self.parse_of_clause();
        if self.current() == TokenKind::Colon {
            self.parse_name_list_clause(CstKind::InheritanceClause);
        }
        self.parse_storagehome_body("`}` closing the abstract storage home");
        self.expect(TokenKind::Semicolon, "`;` after the abstract storage home");
        self.finish();
    }

    /// Productions (26) to (39): a storage type, or its forward
    /// declaration.
    pub(crate) fn parse_storagetype(&mut self) {
        self.start(CstKind::StorageTypeDecl);
        self.bump(); // storagetype
        self.expect_name("a storage type name", PSDL);
        if self.current() == TokenKind::Colon {
            // (32): one base storage type.
            self.parse_name_list_clause(CstKind::InheritanceClause);
        }
        if self.current() == TokenKind::Implements {
            self.parse_name_list_clause(CstKind::ImplementsClause);
        }
        if self.current() == TokenKind::Ref {
            self.parse_ref_rep_directive();
        }
        if self.at_body() {
            self.bump(); // {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                match self.current() {
                    TokenKind::Readonly | TokenKind::State => self.parse_state_decl(),
                    TokenKind::Stores => self.parse_store_directive(),
                    _ => self.parse_psdl_local_op(true),
                }
                if self.remaining() == before {
                    self.error_token("expected a state member, a store directive or an operation");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the storage type");
        }
        self.expect(TokenKind::Semicolon, "`;` after the storage type");
        self.finish();
    }

    /// Productions (40) to (46): a storage home.
    pub(crate) fn parse_storagehome(&mut self) {
        self.start(CstKind::StorageHomeDecl);
        self.bump(); // storagehome
        self.expect_name("a storage home name", PSDL);
        self.parse_of_clause();
        if self.current() == TokenKind::Colon {
            self.parse_name_list_clause(CstKind::InheritanceClause);
        }
        if self.current() == TokenKind::Implements {
            self.parse_name_list_clause(CstKind::ImplementsClause);
        }
        if self.current() == TokenKind::Primary {
            self.start(CstKind::PrimaryKeyDecl);
            self.bump(); // primary
            self.expect(TokenKind::Key, "`key` after `primary`");
            if !self.eat(TokenKind::Ref) {
                self.expect_name("the name of a key, or `ref`", PSDL);
            }
            self.finish();
        }
        self.parse_storagehome_body("`}` closing the storage home");
        self.expect(TokenKind::Semicolon, "`;` after the storage home");
        self.finish();
    }

    /// Whether a declaration's body begins here; a forward declaration
    /// has none.
    fn at_body(&self) -> bool {
        self.current() == TokenKind::LBrace
    }

    /// `of <name>`, production (19) and (41).
    fn parse_of_clause(&mut self) {
        self.start(CstKind::OfClause);
        if self.expect(TokenKind::Of, "`of` and the storage type the home manages") {
            self.parse_scoped_name();
        }
        self.finish();
    }

    /// `: A, B` or `implements A, B`: the keyword or colon and a list of
    /// scoped names, in a node of `kind`.
    fn parse_name_list_clause(&mut self, kind: CstKind) {
        self.start(kind);
        self.bump(); // `:` or `implements`
        loop {
            self.parse_scoped_name();
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.finish();
    }

    /// Productions (20) and (42): the members of an abstract or concrete
    /// storage home, `{ … }`.
    fn parse_storagehome_body(&mut self, closing: &str) {
        if !self.expect(TokenKind::LBrace, "`{` opening the storage home body") {
            return;
        }
        while !self.at_eof() && self.current() != TokenKind::RBrace {
            let before = self.remaining();
            match self.current() {
                TokenKind::Key => self.parse_key_decl(),
                TokenKind::Factory => self.parse_psdl_factory(),
                _ => self.parse_psdl_local_op(false),
            }
            if self.remaining() == before {
                self.error_token("expected a key, a factory or an operation");
            }
        }
        self.expect(TokenKind::RBrace, closing);
    }

    /// Production (13): `[readonly] state <type> a, b;`.
    fn parse_state_decl(&mut self) {
        self.start(CstKind::StateDecl);
        let _ = self.eat(TokenKind::Readonly);
        self.expect(TokenKind::State, "`state`");
        self.parse_state_type();
        self.parse_simple_declarators("a state member name");
        self.expect(TokenKind::Semicolon, "`;` after the state member");
        self.finish();
    }

    /// Production (14), `<psdl_state_type_spec>`: a base type, a string,
    /// a reference to an abstract storage type, or a scoped name.
    ///
    /// Not a sequence, a map or a fixed type: PSS 2.0 §3.2.5.10,
    /// "Anonymous sequences and arrays are not supported"; a `typedef`
    /// names one.
    fn parse_state_type(&mut self) {
        match self.current() {
            TokenKind::Strong | TokenKind::Ref => self.parse_storage_ref_type(),
            TokenKind::Sequence | TokenKind::Map | TokenKind::Fixed => {
                self.diagnose(
                    DiagnosticCode::UnexpectedToken,
                    "the type of a state member is a base type, a string, `ref<…>` or a \
                     scoped name; an anonymous sequence, map or fixed type is named by a \
                     typedef first",
                    "Persistent State Service 2.0 §3.2.3 (14), §3.2.5.10",
                );
                self.parse_type_spec();
            }
            _ => self.parse_type_spec(),
        }
    }

    /// Productions (15) and (35): `[strong] ref<T>`.
    fn parse_storage_ref_type(&mut self) {
        self.start(CstKind::StorageRefType);
        let _ = self.eat(TokenKind::Strong);
        self.expect(TokenKind::Ref, "`ref`");
        if self.expect(TokenKind::Less, "`<` after `ref`") {
            self.parse_scoped_name();
            if self.current() == TokenKind::GreaterGreater {
                self.split_shift_right();
            }
            self.expect(TokenKind::Greater, "`>` closing `ref<…>`");
        }
        self.finish();
    }

    /// Productions (23) and (24): a local operation of a storage type
    /// (`const` allowed after it) or of a storage home.
    fn parse_psdl_local_op(&mut self, allow_const: bool) {
        self.start(CstKind::Operation);
        if !self.eat(TokenKind::Void) {
            self.parse_type_spec();
        }
        self.expect_name("an operation name", PSDL);
        self.parse_parameter_list();
        if self.current() == TokenKind::Raises {
            self.parse_raises_clause();
        }
        if self.current() == TokenKind::Const {
            if !allow_const {
                self.diagnose(
                    DiagnosticCode::UnexpectedToken,
                    "`const` marks an operation of a storage type; a storage home's \
                     operation does not take it",
                    "Persistent State Service 2.0 §3.2.3 (23), (24)",
                );
            }
            self.bump();
        }
        self.expect(TokenKind::Semicolon, "`;` after the operation");
        self.finish();
    }

    /// Production (25): `key k [(a, b)];`.
    fn parse_key_decl(&mut self) {
        self.start(CstKind::KeyDecl);
        self.bump(); // key
        self.expect_name("a key name", PSDL);
        if self.eat(TokenKind::LParen) {
            self.parse_simple_declarators("a state member name");
            self.expect(TokenKind::RParen, "`)` closing the key's state members");
        }
        self.expect(TokenKind::Semicolon, "`;` after the key");
        self.finish();
    }

    /// Productions (47) and (48): `factory f(a, b);` or `factory f();`.
    fn parse_psdl_factory(&mut self) {
        self.start(CstKind::PsdlFactoryDecl);
        self.bump(); // factory
        self.expect_name("a factory name", PSDL);
        if self.expect(TokenKind::LParen, "`(` opening the factory's state members") {
            if self.current() != TokenKind::RParen {
                self.parse_simple_declarators("a state member name");
            }
            self.expect(TokenKind::RParen, "`)` closing the factory's state members");
        }
        self.expect(TokenKind::Semicolon, "`;` after the factory");
        self.finish();
    }

    /// Productions (36) to (38): `stores m as T [scope H];`.
    fn parse_store_directive(&mut self) {
        self.start(CstKind::StoreDirective);
        self.bump(); // stores
        self.parse_simple_declarators("a state member name");
        if self.expect(TokenKind::As, "`as` and the concrete state type") {
            if self.current() == TokenKind::Ref {
                self.parse_storage_ref_type();
            } else {
                self.parse_scoped_name();
            }
        }
        if self.eat(TokenKind::Scope) {
            self.parse_scoped_name();
        }
        self.expect(TokenKind::Semicolon, "`;` after the store directive");
        self.finish();
    }

    /// Production (39): `ref(a, b)`.
    fn parse_ref_rep_directive(&mut self) {
        self.start(CstKind::RefRepDirective);
        self.bump(); // ref
        if self.expect(TokenKind::LParen, "`(` after `ref`") {
            self.parse_simple_declarators("a state member name");
            self.expect(
                TokenKind::RParen,
                "`)` closing the reference representation",
            );
        }
        self.finish();
    }

    /// `<simple_declarator> { "," <simple_declarator> }*`, each a
    /// `Declarator` node.
    fn parse_simple_declarators(&mut self, what: &str) {
        loop {
            self.start(CstKind::Declarator);
            let named = self.expect_name(what, PSDL);
            self.finish();
            if !named || !self.eat(TokenKind::Comma) {
                break;
            }
        }
    }

    // -- CIDL ------------------------------------------------------------

    /// Productions (4) to (6): `composition <category> X { … };`.
    pub(crate) fn parse_composition(&mut self) {
        self.start(CstKind::CompositionDecl);
        self.bump(); // composition
        if matches!(
            self.current(),
            TokenKind::Entity | TokenKind::Process | TokenKind::Service | TokenKind::Session
        ) {
            self.bump();
        } else {
            self.diagnose(
                DiagnosticCode::MissingToken,
                "expected the life cycle category: `entity`, `process`, `service` or `session`",
                "CORBA 3.3 Part 3 §8.5 (5)",
            );
        }
        self.expect_name("a composition name", CIDL);
        if self.expect(TokenKind::LBrace, "`{` opening the composition body") {
            if self.current() == TokenKind::Home {
                self.parse_home_executor();
            } else {
                self.diagnose(
                    DiagnosticCode::MissingToken,
                    "expected `home executor`: a composition body begins with its home \
                     executor definition",
                    "CORBA 3.3 Part 3 §8.5 (6)",
                );
            }
            if self.current() == TokenKind::Proxy {
                self.parse_proxy_home();
            }
            self.skip_to_closing_brace("the composition body");
            self.expect(TokenKind::RBrace, "`}` closing the composition body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the composition");
        self.finish();
    }

    /// Productions (7) and (8): `home executor X { … };`.
    ///
    /// Production (8) fixes the members of the body, their order and how
    /// often each stands: `implements`, then `bindsTo`, `storedOn`,
    /// `manages`, `delegatesTo abstract storagehome`, `delegatesTo
    /// executor` and `abstract`, each at most once, `implements` and
    /// `manages` required. A member out of that order, or a second one
    /// of a kind, is reported and kept in an error node, so that the
    /// lowering reads each member once and in its place.
    fn parse_home_executor(&mut self) {
        self.start(CstKind::HomeExecutorDef);
        self.bump(); // home
        self.expect(TokenKind::Executor, "`executor` after `home`");
        self.expect_name("a home executor name", CIDL);
        if self.expect(TokenKind::LBrace, "`{` opening the home executor body") {
            // The last member accepted in its place, and every kind
            // written, in its place or not.
            let mut last: Option<HomeMember> = None;
            let mut written: alloc::vec::Vec<HomeMember> = alloc::vec::Vec::new();
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                let checkpoint = self.checkpoint();
                let at = self.current_span();
                let member = match self.current() {
                    TokenKind::Implements => {
                        self.parse_named_clause(CstKind::HomeImplDecl, "the home it implements");
                        Some(HomeMember::Implements)
                    }
                    TokenKind::BindsTo => {
                        self.parse_named_clause(
                            CstKind::StorageHomeBinding,
                            "the abstract storage home it binds to",
                        );
                        Some(HomeMember::BindsTo)
                    }
                    TokenKind::StoredOn => {
                        self.parse_named_clause(
                            CstKind::PersistenceDecl,
                            "the abstract storage home it is stored on",
                        );
                        Some(HomeMember::StoredOn)
                    }
                    TokenKind::Manages => {
                        self.parse_executor_def();
                        Some(HomeMember::Manages)
                    }
                    TokenKind::DelegatesTo => self
                        .parse_delegation_member(
                            &[DelegationKind::StorageHome, DelegationKind::Executor],
                            "the home executor body",
                            "CORBA 3.3 Part 3 §8.6 (8)",
                            true,
                        )
                        .map(|kind| {
                            if kind == DelegationKind::Executor {
                                HomeMember::ExecutorDelegation
                            } else {
                                HomeMember::StorageHomeDelegation
                            }
                        }),
                    TokenKind::Abstract => {
                        self.parse_abstract_spec();
                        Some(HomeMember::Abstract)
                    }
                    _ => {
                        self.error_token(
                            "expected `implements`, `bindsTo`, `storedOn`, `manages`, \
                             `delegatesTo` or `abstract` in the home executor body",
                        );
                        self.skip_past_semicolon();
                        None
                    }
                };
                if let Some(member) = member {
                    written.push(member);
                    let problem = last.and_then(|previous| {
                        if previous == member {
                            Some(alloc::format!("a second {}", member.what()))
                        } else if previous > member {
                            Some(alloc::format!(
                                "{} after {}",
                                member.what(),
                                previous.what()
                            ))
                        } else {
                            None
                        }
                    });
                    match problem {
                        Some(problem) => {
                            self.diagnose_span(
                                at,
                                &alloc::format!(
                                    "{problem} in the home executor body: it holds `implements`, \
                                     `bindsTo`, `storedOn`, `manages`, `delegatesTo abstract \
                                     storagehome`, `delegatesTo executor` and `abstract` in this \
                                     order, each at most once"
                                ),
                                "CORBA 3.3 Part 3 §8.6 (8)",
                            );
                            self.start_at(checkpoint, CstKind::Error);
                            self.finish();
                        }
                        None => last = Some(member),
                    }
                }
                if self.remaining() == before {
                    self.error_token("expected a home executor member");
                }
            }
            // A member written out of its place is reported above, not
            // again as missing.
            let closing = self.current_span();
            if !written.contains(&HomeMember::Implements) {
                self.diagnose_span(
                    closing,
                    "the home executor body has no `implements`: it names the home it \
                     implements first",
                    "CORBA 3.3 Part 3 §8.6 (8), §8.7",
                );
            }
            if !written.contains(&HomeMember::Manages) {
                self.diagnose_span(
                    closing,
                    "the home executor body has no `manages`: it defines the executor of \
                     the components the home manages",
                    "CORBA 3.3 Part 3 §8.6 (8), §8.10",
                );
            }
            self.expect(TokenKind::RBrace, "`}` closing the home executor body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the home executor");
        self.finish();
    }

    /// `<keyword> <scoped_name> ;` in a node of `kind`: productions (9),
    /// (11), (12) and (18).
    fn parse_named_clause(&mut self, kind: CstKind, what: &str) {
        self.start(kind);
        self.bump(); // the keyword
        if matches!(self.current(), TokenKind::Ident | TokenKind::ColonColon) {
            self.parse_scoped_name();
        } else {
            self.diagnose(
                DiagnosticCode::MissingToken,
                &alloc::format!("expected {what}"),
                CIDL,
            );
        }
        self.expect(TokenKind::Semicolon, "`;` after the declaration");
        self.finish();
    }

    /// Productions (13) to (15): `manages X [{ … }];`. A body holds at
    /// least one member (14): a segment or a feature delegation (15).
    fn parse_executor_def(&mut self) {
        self.start(CstKind::ExecutorDef);
        self.bump(); // manages
        self.expect_name("a component executor name", CIDL);
        if self.current() == TokenKind::LBrace {
            let opening = self.current_span();
            self.bump(); // {
            if self.current() == TokenKind::RBrace {
                self.diagnose_span(
                    opening,
                    "an executor body holds at least one segment or feature delegation; \
                     an executor without either is written `manages X;`",
                    "CORBA 3.3 Part 3 §8.10 (14)",
                );
            }
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                match self.current() {
                    TokenKind::Segment => self.parse_segment(),
                    TokenKind::DelegatesTo => {
                        // Production (15) ends a feature delegation
                        // without a `;`; one after it is read too.
                        let _ = self.parse_delegation_member(
                            &[DelegationKind::Feature],
                            "the executor body",
                            "CORBA 3.3 Part 3 §8.10 (15)",
                            false,
                        );
                    }
                    _ => {
                        self.error_token(
                            "expected `segment` or `delegatesTo storage` in the executor body",
                        );
                        self.skip_past_semicolon();
                    }
                }
                if self.remaining() == before {
                    self.error_token("expected an executor member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the executor body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the executor definition");
        self.finish();
    }

    /// Productions (16) and (17): `segment X { … }`, with at least one
    /// member.
    fn parse_segment(&mut self) {
        self.start(CstKind::SegmentDef);
        self.bump(); // segment
        self.expect_name("a segment name", CIDL);
        let opening = self.current_span();
        if self.expect(TokenKind::LBrace, "`{` opening the segment") {
            if self.current() == TokenKind::RBrace {
                self.diagnose_span(
                    opening,
                    "a segment holds at least one `storedOn` or `provides facet`",
                    "CORBA 3.3 Part 3 §8.11 (16)",
                );
            }
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                match self.current() {
                    TokenKind::StoredOn => self.parse_named_clause(
                        CstKind::PersistenceDecl,
                        "the abstract storage home the segment is stored on",
                    ),
                    TokenKind::Provides => self.parse_facet_decl(),
                    _ => {
                        self.error_token("expected `storedOn` or `provides` in the segment");
                        self.skip_past_semicolon();
                    }
                }
                if self.remaining() == before {
                    self.error_token("expected a segment member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the segment");
        }
        // §9.2.9.1 writes `};` after a segment, production (16) `}`.
        let _ = self.eat(TokenKind::Semicolon);
        self.finish();
    }

    /// Production (19), `provides facet a, b;`, and the form of the
    /// §9.2.9.1 schematic, `provides (a, b);`.
    fn parse_facet_decl(&mut self) {
        self.start(CstKind::FacetDecl);
        self.bump(); // provides
        if self.eat(TokenKind::LParen) {
            self.parse_simple_declarators("a facet name");
            self.expect(TokenKind::RParen, "`)` closing the facet list");
        } else {
            self.expect(TokenKind::Facet, "`facet` after `provides`");
            self.parse_simple_declarators("a facet name");
        }
        self.expect(TokenKind::Semicolon, "`;` after the facet declaration");
        self.finish();
    }

    /// A `delegatesTo …` member of a body whose production allows the
    /// kinds `allowed`, with the `;` after it (required, or read when
    /// written). A delegation of another kind is reported and kept in an
    /// error node, so that the lowering never reads it as a member of
    /// this body; `None` then.
    fn parse_delegation_member(
        &mut self,
        allowed: &[DelegationKind],
        place: &str,
        clause: &'static str,
        semicolon_required: bool,
    ) -> Option<DelegationKind> {
        let checkpoint = self.checkpoint();
        let at = self.current_span();
        let kind = self.parse_delegation_spec();
        if semicolon_required {
            self.expect(TokenKind::Semicolon, "`;` after the delegation");
        } else {
            let _ = self.eat(TokenKind::Semicolon);
        }
        if allowed.contains(&kind) {
            return Some(kind);
        }
        // An unknown kind was reported by `parse_delegation_spec`.
        if kind != DelegationKind::Unknown {
            let names: alloc::vec::Vec<alloc::string::String> = allowed
                .iter()
                .map(|k| alloc::format!("`{}`", k.written()))
                .collect();
            self.diagnose_span(
                at,
                &alloc::format!(
                    "`{}` does not belong in {place}, which takes {}",
                    kind.written(),
                    names.join(" or ")
                ),
                clause,
            );
        }
        self.start_at(checkpoint, CstKind::Error);
        self.finish();
        None
    }

    /// After `delegatesTo`: a feature delegation (`storage` or `abstract
    /// storagetype`, productions (20) to (24)), a storage home delegation
    /// (`abstract storagehome`, (25)), an executor delegation
    /// (`executor`, (26)) or a home delegation (`home`, (34)). The `;`
    /// that ends some of them is the caller's. Which one it read.
    fn parse_delegation_spec(&mut self) -> DelegationKind {
        let checkpoint = self.checkpoint();
        self.bump(); // delegatesTo
        let kind = match self.current() {
            TokenKind::Ident if self.current_text() == "storage" => {
                self.bump();
                DelegationKind::Feature
            }
            TokenKind::Abstract if self.peek(1) == TokenKind::Storagetype => {
                self.bump();
                self.bump();
                DelegationKind::Feature
            }
            TokenKind::Abstract => {
                self.bump();
                if self.expect(TokenKind::Storagehome, "`storagehome` after `abstract`") {
                    DelegationKind::StorageHome
                } else {
                    DelegationKind::Unknown
                }
            }
            TokenKind::Executor => {
                self.bump();
                DelegationKind::Executor
            }
            TokenKind::Home => {
                self.bump();
                DelegationKind::Home
            }
            _ => {
                self.diagnose(
                    DiagnosticCode::MissingToken,
                    "expected `storage`, `abstract storagehome`, `executor` or `home` after \
                     `delegatesTo`",
                    CIDL,
                );
                DelegationKind::Unknown
            }
        };
        if kind == DelegationKind::Feature {
            self.start_at(checkpoint, CstKind::FeatureDelegationSpec);
            if self.expect(TokenKind::LParen, "`(` opening the feature delegations") {
                loop {
                    self.start(CstKind::FeatureDelegation);
                    self.expect_name("a feature name", CIDL);
                    self.expect(
                        TokenKind::Colon,
                        "`:` between the feature and its storage member",
                    );
                    self.expect_name("a storage member name", CIDL);
                    self.finish();
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "`)` closing the feature delegations");
            }
        } else {
            self.start_at(checkpoint, CstKind::DelegationSpec);
            if self.expect(TokenKind::LParen, "`(` opening the delegation list") {
                loop {
                    self.start(CstKind::Delegation);
                    self.expect_name("an operation name", CIDL);
                    if self.eat(TokenKind::Colon) {
                        self.expect_name("the operation it delegates to", CIDL);
                    }
                    self.finish();
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "`)` closing the delegation list");
            }
        }
        self.finish();
        kind
    }

    /// Productions (30) and (31): `abstract (a, b);`.
    fn parse_abstract_spec(&mut self) {
        self.start(CstKind::AbstractSpec);
        self.bump(); // abstract
        if self.expect(TokenKind::LParen, "`(` opening the abstract operations") {
            self.parse_simple_declarators("an operation name");
            self.expect(TokenKind::RParen, "`)` closing the abstract operations");
        }
        self.expect(TokenKind::Semicolon, "`;` after the abstract specification");
        self.finish();
    }

    /// Productions (32) to (34): `proxy home X { … };`, with at least
    /// one member: a home delegation or an abstract specification.
    fn parse_proxy_home(&mut self) {
        self.start(CstKind::ProxyHomeDef);
        self.bump(); // proxy
        self.expect(TokenKind::Home, "`home` after `proxy`");
        self.expect_name("a proxy home name", CIDL);
        let opening = self.current_span();
        if self.expect(TokenKind::LBrace, "`{` opening the proxy home") {
            if self.current() == TokenKind::RBrace {
                self.diagnose_span(
                    opening,
                    "a proxy home holds at least one `delegatesTo home` or `abstract`",
                    "CORBA 3.3 Part 3 §8.18 (32)",
                );
            }
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                match self.current() {
                    TokenKind::DelegatesTo => {
                        let _ = self.parse_delegation_member(
                            &[DelegationKind::Home],
                            "a proxy home",
                            "CORBA 3.3 Part 3 §8.18 (33)",
                            true,
                        );
                    }
                    TokenKind::Abstract => self.parse_abstract_spec(),
                    _ => {
                        self.error_token(
                            "expected `delegatesTo home` or `abstract` in the proxy home",
                        );
                        self.skip_past_semicolon();
                    }
                }
                if self.remaining() == before {
                    self.error_token("expected a proxy home member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the proxy home");
        }
        self.expect(TokenKind::Semicolon, "`;` after the proxy home");
        self.finish();
    }

    // -- helpers ---------------------------------------------------------

    /// An identifier, or a diagnostic naming what was expected. A CIDL
    /// keyword where a name belongs is said to be one, with its escape.
    fn expect_name(&mut self, what: &str, clause: &'static str) -> bool {
        if self.eat(TokenKind::Ident) {
            return true;
        }
        let message = if self.current().is_keyword() && self.dialect.cidl {
            alloc::format!(
                "expected {what}; `{}` is a keyword in CIDL, and `_{}` is the identifier",
                self.current_text(),
                self.current_text()
            )
        } else {
            alloc::format!("expected {what}")
        };
        self.diagnose(DiagnosticCode::MissingToken, &message, clause);
        false
    }

    /// A syntax diagnostic at `span`, an earlier token than the current
    /// one.
    fn diagnose_span(&mut self, span: Span, message: &str, clause: &'static str) {
        self.diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnexpectedToken,
            span,
            message.to_string(),
            clause,
        ));
    }

    /// Skip what is left of a body up to its `}`, as an error node.
    fn skip_to_closing_brace(&mut self, what: &str) {
        if self.at_eof() || self.current() == TokenKind::RBrace {
            return;
        }
        self.diagnose(
            DiagnosticCode::UnexpectedToken,
            &alloc::format!("unexpected text in {what}"),
            CIDL,
        );
        self.start(CstKind::Error);
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.current() {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RBrace => depth -= 1,
                _ => {}
            }
            self.bump();
        }
        self.finish();
    }

    /// After an error token: skip to and past the next `;` at this
    /// depth, stopping at a `}` that closes the body.
    fn skip_past_semicolon(&mut self) {
        self.start(CstKind::Error);
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.current() {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Semicolon if depth == 0 => {
                    self.bump();
                    break;
                }
                _ => {}
            }
            self.bump();
        }
        self.finish();
    }
}

/// A member of a home executor body, in the order of production (8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HomeMember {
    Implements,
    BindsTo,
    StoredOn,
    Manages,
    StorageHomeDelegation,
    ExecutorDelegation,
    Abstract,
}

impl HomeMember {
    /// The member as the diagnostics name it.
    const fn what(self) -> &'static str {
        match self {
            Self::Implements => "`implements`",
            Self::BindsTo => "`bindsTo`",
            Self::StoredOn => "`storedOn`",
            Self::Manages => "`manages`",
            Self::StorageHomeDelegation => "`delegatesTo abstract storagehome`",
            Self::ExecutorDelegation => "`delegatesTo executor`",
            Self::Abstract => "`abstract`",
        }
    }
}

/// What a `delegatesTo` delegates to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegationKind {
    /// `storage` or `abstract storagetype`: feature delegation, (20).
    Feature,
    /// `abstract storagehome`, (25).
    StorageHome,
    /// `executor`, (26).
    Executor,
    /// `home`, (34).
    Home,
    /// None of these; reported where it was read.
    Unknown,
}

impl DelegationKind {
    /// The kind as written.
    const fn written(self) -> &'static str {
        match self {
            Self::Feature => "delegatesTo storage",
            Self::StorageHome => "delegatesTo abstract storagehome",
            Self::Executor => "delegatesTo executor",
            Self::Home => "delegatesTo home",
            Self::Unknown => "delegatesTo",
        }
    }
}
