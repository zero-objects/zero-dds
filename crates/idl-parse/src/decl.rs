// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Declaration forms. Spec §3.1.

use crate::diagnostic::DiagnosticCode;
use crate::parser::Parser;
use zerodds_idl_cst::{Checkpoint, CstKind, TokenKind};

impl Parser<'_> {
    /// One definition at the current scope.
    ///
    /// Annotations bind to the declaration that follows, so they are
    /// consumed first and the declaration node wraps them.
    pub(crate) fn parse_definition(&mut self) {
        match self.current() {
            TokenKind::At if self.peek(1) == TokenKind::Annotation => {
                self.parse_annotation_decl();
            }
            TokenKind::At => self.parse_annotated_definition(),
            TokenKind::Module => self.parse_module(),
            TokenKind::Const => self.parse_const(),
            TokenKind::Typedef => self.parse_typedef(),
            TokenKind::Struct => self.parse_struct(),
            TokenKind::Union => self.parse_union(),
            TokenKind::Enum => self.parse_enum(),
            TokenKind::Bitmask => self.parse_bitmask(),
            TokenKind::Bitset => self.parse_bitset(),
            TokenKind::Native => self.parse_native(),
            TokenKind::Exception => self.parse_exception(),
            // `abstract` belongs to two productions: (79)
            // `<interface_header> ::= ["local"|"abstract"] "interface"`
            // and (127) `<value_abs_def> ::= "abstract" ("valuetype" |
            // "eventtype") …`. One token of lookahead tells them apart,
            // and taking `abstract` for the first outright is what made
            // `abstract valuetype AV { … };` fail with "expected
            // `interface`".
            TokenKind::Abstract
                if matches!(self.peek(1), TokenKind::Valuetype | TokenKind::Eventtype) =>
            {
                self.parse_valuetype();
            }
            TokenKind::Interface | TokenKind::Local | TokenKind::Abstract => {
                self.parse_interface();
            }
            TokenKind::Valuetype | TokenKind::Custom | TokenKind::Eventtype => {
                self.parse_valuetype();
            }
            TokenKind::Component | TokenKind::Home | TokenKind::Porttype | TokenKind::Connector => {
                self.parse_component();
            }
            // (193) `<template_module_ref> ::= "alias" <scoped_name>
            // "<" <formal_parameter_names> ">" <identifier>`, which
            // (189) admits as a `<tpl_definition>` — inside a template
            // module body and nowhere else. Parsed wherever a definition
            // may stand and refused by the lowering when it stands
            // outside one, because "expected a declaration" on the
            // `alias` says nothing about the rule.
            TokenKind::Alias => self.parse_template_module_ref(),
            TokenKind::Typeid | TokenKind::Typeprefix => self.parse_repository_directive(),
            TokenKind::Import => self.parse_import(),
            // `keylist T (a, b);` is RTI's way of declaring keys. Not
            // OMG IDL, so it is parsed and kept rather than rejected —
            // dropping it would lose what a round trip back to that
            // dialect needs. Spec §4.6 of the AST.
            TokenKind::Keylist => self.parse_keylist(),
            TokenKind::Semicolon => {
                // A stray semicolon between declarations is harmless and
                // common; swallow it rather than reporting noise.
                self.bump();
            }
            _ => {
                self.error_token("expected a declaration");
                self.recover_to_declaration();
            }
        }
    }

    /// `@annotation Name { ... };`
    ///
    /// Distinguished from an annotated declaration by the name that
    /// follows: `@annotation` is the only annotation whose name is a
    /// keyword the grammar gives its own production. IDL 4.2 §8.1.
    fn parse_annotation_decl(&mut self) {
        self.start(CstKind::AnnotationDecl);
        self.bump(); // @
        self.bump(); // annotation
        self.expect(TokenKind::Ident, "an annotation name");
        if self.expect(TokenKind::LBrace, "`{` opening the annotation body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                // An annotation body may declare types of its own, and
                // rule (221) says which: `<annotation_body> ::= {
                // <annotation_member> | <enum_dcl> ";" | <const_dcl> ";"
                // | <typedef_dcl> ";" }*`. An enum for a member's kind,
                // a typedef, a constant, and its members — nothing else.
                // IDL 4.2 §7.4.15.3, §8.1.
                //
                // `struct`, `union`, `bitmask`, `bitset` and `native`
                // were taken here as well, silently. All seven foreign
                // front ends refuse them: the aggregate declared inside
                // an annotation reached the model as a type no peer
                // could name, and the file was unportable without being
                // told so.
                match self.current() {
                    TokenKind::Enum | TokenKind::Typedef | TokenKind::Const => {
                        self.parse_definition();
                    }
                    TokenKind::Struct
                    | TokenKind::Union
                    | TokenKind::Bitmask
                    | TokenKind::Bitset
                    | TokenKind::Native => {
                        self.diagnose(
                            DiagnosticCode::UnexpectedToken,
                            "an `@annotation` body admits an annotation member, an `enum`, a \
                             `const` or a `typedef`, and nothing else",
                            "OMG IDL 4.2 §7.4.15.3 rule (221)",
                        );
                        // Read it anyway. The declaration is still in
                        // the file, the round trip needs its tokens, and
                        // stopping here would report the rest of the
                        // body as a second mistake it is not.
                        self.parse_definition();
                    }
                    _ => {
                        self.start(CstKind::Member);
                        self.parse_type_spec();
                        self.expect(TokenKind::Ident, "an annotation member name");
                        // `default <expr>` gives the member its default.
                        if self.eat(TokenKind::Default) {
                            self.parse_const_expr();
                        }
                        self.expect(TokenKind::Semicolon, "`;` after the annotation member");
                        self.parse_suffix_annotations();
                        self.finish();
                    }
                }
                if self.remaining() == before {
                    self.error_token("expected an annotation member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the annotation body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the annotation");
        self.parse_suffix_annotations();
        self.finish();
    }

    fn parse_annotated_definition(&mut self) {
        // The annotations and the declaration they modify form one node,
        // so a formatter keeps them together and the lowering finds them
        // without looking backwards.
        self.start(CstKind::Member);
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        if self.at_eof() {
            self.finish();
            return;
        }
        self.parse_definition();
        self.finish();
    }

    /// `@ann`, `@ann(v)`, `@ann(k = v, ...)`, and every scoped spelling
    /// of the name.
    ///
    /// (218) `<annotation_appl> ::= "@" <scoped_name> [ "("
    /// <annotation_appl_params> ")" ]`, IDL 4.2 §8.2. A `<scoped_name>`
    /// is an identifier, a root-anchored `::` identifier, or a chain of
    /// them — rule (4) — and this took one token. `@::key` diagnosed on
    /// the `::`, and `@ann::mark(7)` took `ann` for the whole name and
    /// resumed at `::` inside what it then believed was a member
    /// declaration, which is where the eleven cascading diagnostics came
    /// from.
    pub(crate) fn parse_annotation(&mut self) {
        self.start(CstKind::Annotation);
        self.bump(); // @
        self.parse_annotation_tail();
        self.finish();
    }

    /// Everything of an annotation after its marker.
    ///
    /// Shared with the suffix form of DDS-XTypes 1.3 §7.3.1.2.5,
    /// Table 24, whose marker is `//@` rather than `@` and whose tokens
    /// come out of a comment. Table 24 calls the two forms equivalent,
    /// and one body for both is what makes them equivalent by
    /// construction rather than by two rules kept in step by hand.
    pub(crate) fn parse_annotation_tail(&mut self) {
        self.parse_annotation_name();
        if self.current() == TokenKind::LParen {
            self.start(CstKind::AnnotationArguments);
            self.bump();
            while !self.at_eof() && self.current() != TokenKind::RParen {
                self.start(CstKind::AnnotationArgument);
                if self.current() == TokenKind::Ident && self.peek(1) == TokenKind::Equals {
                    self.bump();
                    self.bump();
                }
                self.parse_const_expr();
                self.finish();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "`)` closing the annotation arguments");
            self.finish();
        }
    }

    /// The `<scoped_name>` after an `@`.
    ///
    /// Each segment is an identifier **or a keyword**: `@default` and
    /// `@public` collide with keywords by design, and §8.2 gives the
    /// name no exception from rule (4) for being written after an `@`.
    /// The loop is `parse_scoped_name`'s with that one widening, which
    /// is why it is written here rather than shared.
    fn parse_annotation_name(&mut self) {
        // A leading `::` anchors the name at the global scope.
        let _ = self.eat(TokenKind::ColonColon);
        if !self.eat_annotation_segment() {
            self.diagnose(
                DiagnosticCode::UnexpectedToken,
                "expected an annotation name after `@`",
                "OMG IDL 4.2 §8.2",
            );
            return;
        }
        while self.current() == TokenKind::ColonColon {
            self.bump();
            if !self.eat_annotation_segment() {
                self.diagnose(
                    DiagnosticCode::UnexpectedToken,
                    "expected an annotation name after `::`",
                    "OMG IDL 4.2 §8.2",
                );
                return;
            }
        }
    }

    /// One segment of an annotation's scoped name.
    fn eat_annotation_segment(&mut self) -> bool {
        if self.eat(TokenKind::Ident) {
            return true;
        }
        if self.current().is_keyword() {
            self.bump();
            return true;
        }
        false
    }

    /// `component`, `home`, `porttype` and `connector`.
    ///
    /// The four component-model declarations share a shape: a name, some
    /// clauses, and a body of ports and members. They reach the model as
    /// interfaces of the matching kind, with their ports in the CORBA
    /// facet — normalising a port into a plain attribute would lose what
    /// a round trip back to CCM IDL needs. IDL 4.2 §7.4.5 to §7.4.9.
    fn parse_component(&mut self) {
        // A forward declaration: `component C;`
        if self.peek(1) == TokenKind::Ident && self.peek(2) == TokenKind::Semicolon {
            self.start(CstKind::ForwardDecl);
            self.bump();
            self.bump();
            let _ = self.eat(TokenKind::Semicolon);
            self.finish();
            return;
        }

        self.start(CstKind::InterfaceDecl);
        self.bump(); // component / home / porttype / connector
        self.expect(TokenKind::Ident, "a name");

        if self.current() == TokenKind::Colon {
            self.start(CstKind::InheritanceClause);
            self.bump();
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.finish();
        }
        if self.current() == TokenKind::Supports {
            self.start(CstKind::InheritanceClause);
            self.bump();
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.finish();
        }
        // A home says what it manages and, optionally, the key it does
        // so by. IDL 4.2 §7.4.9.
        if self.current() == TokenKind::Manages {
            self.start(CstKind::ManagesClause);
            self.bump();
            self.parse_scoped_name();
            if self.eat(TokenKind::Primarykey) {
                self.parse_scoped_name();
            }
            self.finish();
        }

        if self.expect(TokenKind::LBrace, "`{` opening the body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                self.parse_component_element();
                if self.remaining() == before {
                    self.error_token("expected a port or member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the declaration");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// One port or member of a component-model declaration.
    fn parse_component_element(&mut self) {
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        match self.current() {
            TokenKind::Provides
            | TokenKind::Uses
            | TokenKind::Publishes
            | TokenKind::Emits
            | TokenKind::Consumes
            | TokenKind::Port
            | TokenKind::Mirrorport => self.parse_port(),
            TokenKind::Factory | TokenKind::Finder => self.parse_factory(),
            TokenKind::Readonly | TokenKind::Attribute => self.parse_attribute(None),
            TokenKind::Const
            | TokenKind::Typedef
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Enum
            | TokenKind::Exception
            | TokenKind::Native
            | TokenKind::Bitmask
            | TokenKind::Bitset => self.parse_definition(),
            TokenKind::RBrace | TokenKind::Eof => {}
            _ => self.parse_operation(),
        }
    }

    /// `provides I name;`, `uses multiple I name;`, `emits T name;` and
    /// the rest. IDL 4.2 §7.4.5.
    fn parse_port(&mut self) {
        self.start(CstKind::PortDecl);
        self.bump(); // the port keyword
        // `uses multiple I name;` accepts more than one connection.
        let _ = self.eat(TokenKind::Multiple);
        if self.current() == TokenKind::Object {
            // `provides Object name;` — the generic interface.
            self.bump();
        } else {
            self.parse_scoped_name();
        }
        self.expect(TokenKind::Ident, "a port name");
        self.expect(TokenKind::Semicolon, "`;` after the port");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `(in T a, out U b)`, including the parentheses.
    fn parse_parameter_list(&mut self) {
        if !self.expect(TokenKind::LParen, "`(` opening the parameter list") {
            return;
        }
        while !self.at_eof() && self.current() != TokenKind::RParen {
            self.start(CstKind::Parameter);
            while self.current() == TokenKind::At {
                self.parse_annotation();
            }
            if matches!(
                self.current(),
                TokenKind::In | TokenKind::Out | TokenKind::Inout
            ) {
                self.bump();
            } else {
                self.diagnose(
                    DiagnosticCode::MissingToken,
                    "expected `in`, `out` or `inout`",
                    "OMG IDL 4.2 §7.4.3.5",
                );
            }
            self.parse_type_spec();
            let _ = self.eat(TokenKind::Ident);
            self.finish();
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)` closing the parameter list");
    }

    /// One element of a valuetype body.
    ///
    /// A valuetype holds more than state: initialisers, operations,
    /// attributes and its own type declarations. IDL 4.2 §7.4.4.
    fn parse_value_element(&mut self) {
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        match self.current() {
            TokenKind::Factory => self.parse_factory(),
            TokenKind::Readonly | TokenKind::Attribute => self.parse_attribute(None),
            TokenKind::Const
            | TokenKind::Typedef
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Enum
            | TokenKind::Exception
            | TokenKind::Native
            | TokenKind::Bitmask
            | TokenKind::Bitset
            // A value element is an `<export>` too, so it takes (112)'s
            // three directives the interface body takes.
            | TokenKind::Typeid
            | TokenKind::Typeprefix
            | TokenKind::Import => self.parse_definition(),
            TokenKind::RBrace | TokenKind::Eof => {}
            _ => self.parse_state_or_operation(),
        }
    }

    /// `factory create(in long a) raises (E);` IDL 4.2 §7.4.4.
    fn parse_factory(&mut self) {
        self.start(CstKind::Operation);
        self.bump(); // factory
        self.expect(TokenKind::Ident, "an initialiser name");
        self.parse_parameter_list();
        if self.current() == TokenKind::Raises {
            self.parse_raises_clause();
        }
        self.expect(TokenKind::Semicolon, "`;` after the initialiser");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// A state member or an operation, told apart at the token after the
    /// name: a `(` begins a parameter list, anything else a declarator.
    fn parse_state_or_operation(&mut self) {
        let visibility = matches!(self.current(), TokenKind::Public | TokenKind::Private);
        if visibility {
            // `public` and `private` mark state, never an operation.
            self.start(CstKind::Member);
            self.bump();
            self.parse_type_spec();
            self.parse_declarator_list();
            self.expect(TokenKind::Semicolon, "`;` after the state member");
            self.parse_suffix_annotations();
            self.finish();
            return;
        }
        if self.current() == TokenKind::Void {
            self.parse_operation();
            return;
        }
        // A type specification followed by a name and `(` is an
        // operation; the lookahead is over the tokens the type spec and
        // the name would occupy.
        if self.operation_ahead() {
            self.parse_operation();
        } else {
            self.start(CstKind::Member);
            self.parse_type_spec();
            self.parse_declarator_list();
            self.expect(TokenKind::Semicolon, "`;` after the state member");
            self.parse_suffix_annotations();
            self.finish();
        }
    }

    /// Whether what follows is an operation rather than a state member.
    ///
    /// Scans ahead for the first `(` or `;` at depth zero: an operation
    /// reaches its parameter list, a member reaches its semicolon.
    fn operation_ahead(&self) -> bool {
        let mut depth = 0i32;
        let mut at = 0usize;
        loop {
            match self.peek(at) {
                TokenKind::Eof | TokenKind::RBrace => return false,
                TokenKind::Less | TokenKind::LBracket => depth += 1,
                TokenKind::Greater | TokenKind::RBracket => depth -= 1,
                TokenKind::LParen if depth <= 0 => return true,
                TokenKind::Semicolon | TokenKind::Comma if depth <= 0 => return false,
                _ => {}
            }
            at += 1;
        }
    }

    /// `alias M<T> R;` — rules (193) and (194).
    ///
    /// The names between the angle brackets are the **enclosing**
    /// template's formal parameters, rule (194)
    /// `<formal_parameter_names> ::= <identifier> { "," <identifier> }*`
    /// — not types. They are parsed as scoped names so the lowering
    /// reads them the same way it reads every other name.
    fn parse_template_module_ref(&mut self) {
        self.start(CstKind::TemplateModuleRef);
        self.bump(); // alias
        self.parse_scoped_name();
        self.start(CstKind::ActualParameters);
        if self.eat(TokenKind::Less) {
            while !self.at_eof()
                && !matches!(
                    self.current(),
                    TokenKind::Greater | TokenKind::GreaterGreater
                )
            {
                let before = self.remaining();
                self.parse_scoped_name();
                if self.remaining() == before {
                    self.error_token("expected a formal parameter name");
                    break;
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.close_type_arguments();
        } else {
            self.diagnose(
                DiagnosticCode::MissingToken,
                "expected `<` opening the formal parameter names",
                "OMG IDL 4.2 §7.4.12.3",
            );
        }
        self.finish();
        self.expect(TokenKind::Ident, "a name for the referenced module");
        self.expect(
            TokenKind::Semicolon,
            "`;` after the template module reference",
        );
        self.finish();
    }

    /// Whether what follows `module` is an instantiation.
    ///
    /// Scans the name, then the `<…>` with its nesting, and reads the
    /// token after the closing `>`. Nothing is consumed: a declaration
    /// and an instantiation diverge only there, and a parser that
    /// committed at the `<` would have to unwind.
    fn at_template_module_inst(&self) -> bool {
        let mut at = 1;
        // The name: `A`, `::A`, `A::B`.
        if self.peek(at) == TokenKind::ColonColon {
            at += 1;
        }
        if self.peek(at) != TokenKind::Ident {
            return false;
        }
        at += 1;
        while self.peek(at) == TokenKind::ColonColon {
            at += 1;
            if self.peek(at) != TokenKind::Ident {
                return false;
            }
            at += 1;
        }
        if self.peek(at) != TokenKind::Less {
            return false;
        }
        // `>>` closes two lists at once, which is why the depth moves by
        // two for it — the same token the type-argument parser splits.
        let mut depth = 0i32;
        loop {
            match self.peek(at) {
                TokenKind::Eof | TokenKind::LBrace | TokenKind::Semicolon => return false,
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    depth -= 1;
                    if depth == 0 {
                        return self.peek(at + 1) == TokenKind::Ident;
                    }
                }
                TokenKind::GreaterGreater => {
                    depth -= 2;
                    if depth <= 0 {
                        return self.peek(at + 1) == TokenKind::Ident;
                    }
                }
                _ => {}
            }
            at += 1;
        }
    }

    /// `module M<long, 8> N;` — rules (190), (191) and (192).
    fn parse_template_module_inst(&mut self) {
        self.start(CstKind::TemplateModuleInst);
        self.bump(); // module
        self.parse_scoped_name();
        self.parse_actual_parameters();
        self.expect(TokenKind::Ident, "a name for the instantiated module");
        self.expect(TokenKind::Semicolon, "`;` after the instantiation");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `<long, 8>` — rule (191), each element rule (192).
    ///
    /// (192) `<actual_parameter> ::= <type_spec> | <const_expr>`, and a
    /// scoped name is both. It is parsed as a type specification here
    /// and the lowering decides which it names, because only the lowering
    /// knows what the name is bound to.
    fn parse_actual_parameters(&mut self) {
        self.start(CstKind::ActualParameters);
        if !self.eat(TokenKind::Less) {
            self.diagnose(
                DiagnosticCode::MissingToken,
                "expected `<` opening the actual parameters",
                "OMG IDL 4.2 §7.4.12.3",
            );
            self.finish();
            return;
        }
        while !self.at_eof()
            && !matches!(
                self.current(),
                TokenKind::Greater | TokenKind::GreaterGreater
            )
        {
            let before = self.remaining();
            if self.at_actual_type() {
                self.parse_type_spec();
            } else {
                self.parse_const_expr();
            }
            if self.remaining() == before {
                self.error_token("expected an actual parameter");
                break;
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.close_type_arguments();
        self.finish();
    }

    /// Whether the actual parameter here is written as a type.
    fn at_actual_type(&self) -> bool {
        matches!(
            self.current(),
            TokenKind::Sequence
                | TokenKind::StringKw
                | TokenKind::Wstring
                | TokenKind::Map
                | TokenKind::Fixed
                | TokenKind::Ident
                | TokenKind::ColonColon
                | TokenKind::Boolean
                | TokenKind::Char
                | TokenKind::Wchar
                | TokenKind::Octet
                | TokenKind::Short
                | TokenKind::Long
                | TokenKind::Float
                | TokenKind::Double
                | TokenKind::Unsigned
                | TokenKind::Int8
                | TokenKind::Int16
                | TokenKind::Int32
                | TokenKind::Int64
                | TokenKind::Uint8
                | TokenKind::Uint16
                | TokenKind::Uint32
                | TokenKind::Uint64
                | TokenKind::Any
                | TokenKind::Object
                | TokenKind::Valuebase
        )
    }

    /// `<typename T, const long C, struct S, interface I>`
    ///
    /// Each parameter names what kind of entity it stands for; a `const`
    /// parameter also names the type its argument must have.
    /// IDL 4.2 §7.4.12.
    fn parse_template_parameters(&mut self) {
        self.start(CstKind::TemplateParameters);
        self.bump(); // <
        while !self.at_eof() && self.current() != TokenKind::Greater {
            let before = self.remaining();
            self.start(CstKind::TemplateParameter);
            match self.current() {
                // (188) `<formal_parameter_type> ::= "typename" |
                // "interface" | "valuetype" | "eventtype" | "struct" |
                // "union" | "exception" | "enum" | "sequence" | "const"
                // <const_type> | <sequence_type>`. Eleven alternatives,
                // and the last two are not bare keywords.
                //
                // `sequence` appears twice: on its own it is the
                // classifier "this parameter stands for some sequence",
                // and followed by `<` it is a whole `<sequence_type>`
                // constraining the element. Only the first parsed, so
                // `module M<sequence<long> S> { … };` failed on the `<`.
                TokenKind::Sequence if self.peek(1) == TokenKind::Less => {
                    self.parse_type_spec();
                }
                TokenKind::Typename
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
                | TokenKind::Exception
                | TokenKind::Interface
                | TokenKind::Valuetype
                | TokenKind::Eventtype
                | TokenKind::Sequence => {
                    self.bump();
                }
                TokenKind::Const => {
                    self.bump();
                    self.parse_type_spec();
                }
                _ => {
                    self.diagnose(
                        DiagnosticCode::UnexpectedToken,
                        "expected a template parameter kind",
                        "OMG IDL 4.2 §7.4.12",
                    );
                }
            }
            self.expect(TokenKind::Ident, "a template parameter name");
            self.finish();
            if self.remaining() == before {
                self.error_token("expected a template parameter");
                break;
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::Greater, "`>` closing the template parameters");
        self.finish();
    }

    /// `typeid <name> "id";` or `typeprefix <name> "prefix";`
    /// IDL 4.2 §7.4.14.
    fn parse_repository_directive(&mut self) {
        self.start(CstKind::RepositoryDirective);
        self.bump();
        self.parse_scoped_name();
        if self.expect(TokenKind::StringLiteral, "the repository identifier string") {
            self.eat_adjacent_string_literals(TokenKind::StringLiteral);
        }
        self.expect(TokenKind::Semicolon, "`;` after the directive");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `import <scoped_name>;` or `import "IDL:...";` IDL 4.2 §7.4.13.
    fn parse_import(&mut self) {
        self.start(CstKind::ImportDecl);
        self.bump();
        if self.eat(TokenKind::StringLiteral) {
            self.eat_adjacent_string_literals(TokenKind::StringLiteral);
        } else {
            self.parse_scoped_name();
        }
        self.expect(TokenKind::Semicolon, "`;` after the import");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `keylist <type> (<field>, ...);` — an RTI Connext directive.
    ///
    /// In no building block of IDL 4.2. It is kept in the tree rather
    /// than refused, because dropping it loses what a round trip back to
    /// that dialect needs — and it is reported either way, because a
    /// file that uses it is not one another vendor's compiler reads.
    /// `diagnose_vendor` decides only the severity.
    fn parse_keylist(&mut self) {
        self.diagnose_vendor(
            crate::dialect::VendorFlag::Rti,
            "`keylist` is an RTI Connext directive and is in no building block of OMG IDL 4.2; \
             a file that uses one is not portable to another vendor's front end",
            "zerodds-idl-frontend-1.0 §3.5",
        );
        self.start(CstKind::KeylistDecl);
        self.bump();
        self.parse_scoped_name();
        if self.eat(TokenKind::LParen) {
            while !self.at_eof() && self.current() != TokenKind::RParen {
                if !self.eat(TokenKind::Ident) {
                    break;
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "`)` closing the key list");
        }
        self.expect(TokenKind::Semicolon, "`;` after the keylist");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `module M { … };`, `module M<…> { … };` and `module M<…> N;`.
    ///
    /// The three share their first token, and the third is rule (190)
    /// `<template_module_inst> ::= "module" <scoped_name> "<"
    /// <actual_parameters> ">" <identifier>`. What tells an
    /// instantiation from a declaration is not the `<` — both have one —
    /// but what stands after the matching `>`: an identifier names the
    /// instance, a `{` opens a body.
    fn parse_module(&mut self) {
        if self.at_template_module_inst() {
            self.parse_template_module_inst();
            return;
        }
        self.start(CstKind::Module);
        self.bump(); // module
        self.expect(TokenKind::Ident, "a module name");
        // A template module names its formal parameters before the body.
        // IDL 4.2 §7.4.12.
        if self.current() == TokenKind::Less {
            self.parse_template_parameters();
        }
        if self.expect(TokenKind::LBrace, "`{` opening the module body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before_len = self.remaining();
                self.parse_definition();
                if self.remaining() == before_len {
                    self.error_token("expected a declaration inside the module");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the module body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the module");
        self.parse_suffix_annotations();
        self.finish();
    }

    fn parse_const(&mut self) {
        self.start(CstKind::ConstDecl);
        self.bump(); // const
        self.parse_type_spec();
        self.expect(TokenKind::Ident, "a constant name");
        self.expect(TokenKind::Equals, "`=` before the constant value");
        self.parse_const_expr();
        self.expect(TokenKind::Semicolon, "`;` after the constant");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `typedef <type> <declarators>;`
    ///
    /// Rule (64), `<type_declarator> ::= { <simple_type_spec> |
    /// <template_type_spec> | <constr_type_dcl> } <any_declarators>`:
    /// the third alternative defines the type in place, `typedef struct
    /// Inline { long m; } Ali;`. The definition is the typedef's child,
    /// without its own `;`, and the lowering declares it before it
    /// aliases it. `struct`, `union`, `enum`, `bitset` and `bitmask` are
    /// the five `<constr_type_dcl>` forms a typedef can name; none of
    /// them starts a type reference, so the token decides.
    fn parse_typedef(&mut self) {
        self.start(CstKind::TypedefDecl);
        self.bump(); // typedef
        match self.current() {
            TokenKind::Struct => self.parse_struct_def(false),
            TokenKind::Union => self.parse_union_def(false),
            TokenKind::Enum => self.parse_enum_def(false),
            TokenKind::Bitset => self.parse_bitset_def(false),
            TokenKind::Bitmask => self.parse_bitmask_def(false),
            _ => self.parse_type_spec(),
        }
        self.parse_declarator_list();
        self.expect(TokenKind::Semicolon, "`;` after the typedef");
        self.parse_suffix_annotations();
        self.finish();
    }

    fn parse_struct(&mut self) {
        self.parse_struct_def(true);
    }

    /// A struct definition; `terminated` is false inside a `typedef`,
    /// where the declarators and not a `;` follow the body (rule (64),
    /// see [`Self::parse_typedef`]).
    fn parse_struct_def(&mut self, terminated: bool) {
        // A forward declaration is `struct S;` — known only at the token
        // after the name.
        if terminated && self.peek(1) == TokenKind::Ident && self.peek(2) == TokenKind::Semicolon {
            self.start(CstKind::ForwardDecl);
            self.bump();
            self.bump();
            self.bump();
            self.finish();
            return;
        }

        self.start(CstKind::StructDecl);
        self.bump(); // struct
        self.expect(TokenKind::Ident, "a struct name");
        // Single inheritance: `struct Derived : Base`.
        if self.current() == TokenKind::Colon {
            self.start(CstKind::InheritanceClause);
            self.bump();
            self.parse_scoped_name();
            self.finish();
        }
        if self.expect(TokenKind::LBrace, "`{` opening the struct body") {
            self.parse_member_list();
            self.expect(TokenKind::RBrace, "`}` closing the struct body");
        }
        if terminated {
            self.expect(TokenKind::Semicolon, "`;` after the struct");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_union(&mut self) {
        self.parse_union_def(true);
    }

    /// A union definition; `terminated` as for [`Self::parse_struct_def`].
    fn parse_union_def(&mut self, terminated: bool) {
        if terminated && self.peek(1) == TokenKind::Ident && self.peek(2) == TokenKind::Semicolon {
            self.start(CstKind::ForwardDecl);
            self.bump();
            self.bump();
            self.bump();
            self.finish();
            return;
        }

        self.start(CstKind::UnionDecl);
        self.bump(); // union
        self.expect(TokenKind::Ident, "a union name");
        self.expect(TokenKind::Switch, "`switch` after the union name");
        self.expect(TokenKind::LParen, "`(` before the discriminator");
        // `switch (@try_construct T)` — DDS-XTypes 1.3 §7.3.1.2.1.12.5.
        self.parse_annotated_type_spec();
        self.expect(TokenKind::RParen, "`)` after the discriminator");
        if self.expect(TokenKind::LBrace, "`{` opening the union body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                self.parse_union_case();
                if self.remaining() == before {
                    self.error_token("expected a case");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the union body");
        }
        if terminated {
            self.expect(TokenKind::Semicolon, "`;` after the union");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_union_case(&mut self) {
        self.start(CstKind::UnionCase);
        // One case may carry several labels: `case 1: case 2: long x;`
        while matches!(self.current(), TokenKind::Case | TokenKind::Default) {
            self.start(CstKind::CaseLabel);
            let is_default = self.current() == TokenKind::Default;
            self.bump();
            if !is_default {
                self.parse_const_expr();
            }
            self.expect(TokenKind::Colon, "`:` after the case label");
            self.finish();
        }
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        if !self.at_eof() && self.current() != TokenKind::RBrace {
            self.parse_type_spec();
            self.parse_declarator_list();
            self.expect(TokenKind::Semicolon, "`;` after the case member");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_enum(&mut self) {
        self.parse_enum_def(true);
    }

    /// A enum definition; `terminated` as for [`Self::parse_struct_def`].
    fn parse_enum_def(&mut self, terminated: bool) {
        self.start(CstKind::EnumDecl);
        self.bump(); // enum
        self.expect(TokenKind::Ident, "an enum name");
        if self.expect(TokenKind::LBrace, "`{` opening the enum body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                self.start(CstKind::Enumerator);
                while self.current() == TokenKind::At {
                    self.parse_annotation();
                }
                if !self.eat(TokenKind::Ident) {
                    self.finish();
                    break;
                }
                // `A //@value(3), B` and `A, //@value(3) B` both suffix
                // `A`, so the separator is read inside the node this
                // opened rather than after it — DDS-XTypes 1.3
                // §7.3.1.2.5 puts the annotation after the declaration
                // and says nothing about which side of the comma.
                self.parse_suffix_annotations();
                let more = self.eat(TokenKind::Comma);
                if more {
                    self.parse_suffix_annotations();
                }
                self.finish();
                if !more {
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the enum body");
        }
        if terminated {
            self.expect(TokenKind::Semicolon, "`;` after the enum");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_bitmask(&mut self) {
        self.parse_bitmask_def(true);
    }

    /// A bitmask definition; `terminated` as for [`Self::parse_struct_def`].
    fn parse_bitmask_def(&mut self, terminated: bool) {
        self.start(CstKind::BitmaskDecl);
        self.bump(); // bitmask
        self.expect(TokenKind::Ident, "a bitmask name");
        if self.expect(TokenKind::LBrace, "`{` opening the bitmask body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                self.start(CstKind::BitValue);
                while self.current() == TokenKind::At {
                    self.parse_annotation();
                }
                if !self.eat(TokenKind::Ident) {
                    self.finish();
                    break;
                }
                // `A //@value(3), B` and `A, //@value(3) B` both suffix
                // `A`, so the separator is read inside the node this
                // opened rather than after it — DDS-XTypes 1.3
                // §7.3.1.2.5 puts the annotation after the declaration
                // and says nothing about which side of the comma.
                self.parse_suffix_annotations();
                let more = self.eat(TokenKind::Comma);
                if more {
                    self.parse_suffix_annotations();
                }
                self.finish();
                if !more {
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the bitmask body");
        }
        if terminated {
            self.expect(TokenKind::Semicolon, "`;` after the bitmask");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_bitset(&mut self) {
        self.parse_bitset_def(true);
    }

    /// A bitset definition; `terminated` as for [`Self::parse_struct_def`].
    fn parse_bitset_def(&mut self, terminated: bool) {
        self.start(CstKind::BitsetDecl);
        self.bump(); // bitset
        self.expect(TokenKind::Ident, "a bitset name");
        if self.current() == TokenKind::Colon {
            self.start(CstKind::InheritanceClause);
            self.bump();
            self.parse_scoped_name();
            self.finish();
        }
        if self.expect(TokenKind::LBrace, "`{` opening the bitset body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                self.start(CstKind::BitField);
                while self.current() == TokenKind::At {
                    self.parse_annotation();
                }
                if self.current() == TokenKind::Bitfield {
                    self.bump();
                    if self.eat(TokenKind::Less) {
                        self.parse_const_expr();
                        if self.eat(TokenKind::Comma) {
                            self.parse_type_spec();
                        }
                        self.expect(TokenKind::Greater, "`>` closing the bitfield width");
                    }
                    // The field name is optional: an anonymous bitfield
                    // is padding.
                    let _ = self.eat(TokenKind::Ident);
                    self.expect(TokenKind::Semicolon, "`;` after the bitfield");
                    self.parse_suffix_annotations();
                    self.finish();
                } else {
                    self.finish();
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the bitset body");
        }
        if terminated {
            self.expect(TokenKind::Semicolon, "`;` after the bitset");
            self.parse_suffix_annotations();
        }
        self.finish();
    }

    fn parse_native(&mut self) {
        self.start(CstKind::NativeDecl);
        self.bump(); // native
        self.expect(TokenKind::Ident, "a native type name");
        self.expect(TokenKind::Semicolon, "`;` after the native declaration");
        self.parse_suffix_annotations();
        self.finish();
    }

    fn parse_exception(&mut self) {
        self.start(CstKind::ExceptionDecl);
        self.bump(); // exception
        self.expect(TokenKind::Ident, "an exception name");
        if self.expect(TokenKind::LBrace, "`{` opening the exception body") {
            self.parse_member_list();
            self.expect(TokenKind::RBrace, "`}` closing the exception body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the exception");
        self.parse_suffix_annotations();
        self.finish();
    }

    fn parse_interface(&mut self) {
        // `local interface`, `abstract interface`, or plain.
        let modifier_ahead = matches!(self.current(), TokenKind::Local | TokenKind::Abstract);
        let forward_at = if modifier_ahead { 2 } else { 1 };
        if self.peek(forward_at) == TokenKind::Ident
            && self.peek(forward_at + 1) == TokenKind::Semicolon
        {
            self.start(CstKind::ForwardDecl);
            while !self.at_eof() && self.current() != TokenKind::Semicolon {
                self.bump();
            }
            let _ = self.eat(TokenKind::Semicolon);
            self.finish();
            return;
        }

        self.start(CstKind::InterfaceDecl);
        if modifier_ahead {
            self.bump();
        }
        self.expect(TokenKind::Interface, "`interface`");
        self.expect(TokenKind::Ident, "an interface name");
        if self.current() == TokenKind::Colon {
            self.start(CstKind::InheritanceClause);
            self.bump();
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.finish();
        }
        if self.expect(TokenKind::LBrace, "`{` opening the interface body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                self.parse_export();
                if self.remaining() == before {
                    self.error_token("expected an interface member");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the interface body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the interface");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// One member of an interface body.
    ///
    /// The annotations and the export they modify form **one** node, the
    /// way [`Self::parse_annotated_definition`] already did at file
    /// scope. They used to be siblings of it, and the lowering reads a
    /// declaration's annotations out of its own children — so every
    /// annotation on an operation or an attribute was parsed, kept in
    /// the tree, and never applied. `@ami long deposit(in long a);`
    /// left `CorbaFacet::is_ami` false, and IDL 4.2 §8.3.5's whole
    /// asynchronous mapping came out empty with no diagnostic.
    ///
    /// The wrapping is `start_at`, which puts the already-emitted
    /// annotation nodes inside the declaration's node. Nothing about
    /// the token stream changes; the round trip is unaffected.
    fn parse_export(&mut self) {
        let checkpoint = self.checkpoint();
        let annotated = self.current() == TokenKind::At;
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        let wrap = if annotated { Some(checkpoint) } else { None };
        match self.current() {
            TokenKind::Readonly | TokenKind::Attribute => self.parse_attribute(wrap),
            TokenKind::Oneway | TokenKind::Void => self.parse_operation_wrapped(wrap),
            TokenKind::Const
            | TokenKind::Typedef
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Enum
            | TokenKind::Exception
            | TokenKind::Native
            | TokenKind::Bitmask
            | TokenKind::Bitset => {
                // A nested declaration takes the same `Member` wrapper
                // file scope gives it, so `@nested struct S` inside an
                // interface reaches the lowering the same way.
                if let Some(checkpoint) = wrap {
                    self.start_at(checkpoint, CstKind::Member);
                    self.parse_definition();
                    self.finish();
                } else {
                    self.parse_definition();
                }
            }
            // (112) `<export> ::+ <type_id_dcl> ";" | <type_prefix_dcl>
            // ";" | <import_dcl> ";"` — the CORBA building block extends
            // the interface body with the three directives the file
            // scope already takes. Without the arm `typeid Ia "…";`
            // inside `Ia` read as an operation whose return type is
            // `typeid`, and failed with "expected a type".
            TokenKind::Typeid | TokenKind::Typeprefix | TokenKind::Import => {
                self.parse_definition();
            }
            TokenKind::RBrace | TokenKind::Eof => {}
            // Anything else at this point begins an operation with a
            // typed return.
            _ => self.parse_operation_wrapped(wrap),
        }
    }

    fn parse_attribute(&mut self, wrap: Option<Checkpoint>) {
        match wrap {
            Some(checkpoint) => self.start_at(checkpoint, CstKind::AttributeDecl),
            None => self.start(CstKind::AttributeDecl),
        }
        let _ = self.eat(TokenKind::Readonly);
        self.expect(TokenKind::Attribute, "`attribute`");
        self.parse_type_spec();
        self.parse_declarator_list();
        // A readonly attribute writes `raises`; a writable one writes
        // `getraises` and `setraises`. IDL 4.2 §7.4.3.6.
        while matches!(
            self.current(),
            TokenKind::Raises | TokenKind::Getraises | TokenKind::Setraises
        ) {
            self.parse_raises_clause();
        }
        self.expect(TokenKind::Semicolon, "`;` after the attribute");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// A `raises`, `getraises` or `setraises` clause.
    fn parse_raises_clause(&mut self) {
        self.start(CstKind::RaisesClause);
        self.bump();
        if self.eat(TokenKind::LParen) {
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "`)` closing the raises clause");
        }
        self.finish();
    }

    fn parse_operation(&mut self) {
        self.parse_operation_wrapped(None);
    }

    /// An operation, optionally wrapping the annotations already
    /// emitted since `wrap` into its own node. See [`Self::parse_export`].
    fn parse_operation_wrapped(&mut self, wrap: Option<Checkpoint>) {
        match wrap {
            Some(checkpoint) => self.start_at(checkpoint, CstKind::Operation),
            None => self.start(CstKind::Operation),
        }
        let _ = self.eat(TokenKind::Oneway);
        if !self.eat(TokenKind::Void) {
            self.parse_type_spec();
        }
        self.expect(TokenKind::Ident, "an operation name");
        self.parse_parameter_list();
        if self.current() == TokenKind::Raises {
            self.parse_raises_clause();
        }
        if self.current() == TokenKind::Context {
            self.start(CstKind::ContextClause);
            self.bump();
            if self.eat(TokenKind::LParen) {
                loop {
                    if self.eat(TokenKind::StringLiteral) {
                        self.eat_adjacent_string_literals(TokenKind::StringLiteral);
                    }
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "`)` closing the context clause");
            }
            self.finish();
        }
        self.expect(TokenKind::Semicolon, "`;` after the operation");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// `valuetype`, `custom valuetype`, `eventtype`, a forward
    /// declaration and a value box.
    ///
    /// The node's kind is decided after the name, because rule (126)
    /// `<value_box_def> ::= "valuetype" <identifier> <type_spec>` is
    /// told from rule (98)'s valuetype by what follows the name and by
    /// nothing before it. A checkpoint is what lets the decision come
    /// after the tokens it depends on.
    fn parse_valuetype(&mut self) {
        let start = self.checkpoint();
        let mut modified = false;
        while matches!(
            self.current(),
            TokenKind::Custom | TokenKind::Abstract | TokenKind::Eventtype
        ) {
            modified = true;
            self.bump();
        }
        let _ = self.eat(TokenKind::Valuetype);
        self.expect(TokenKind::Ident, "a valuetype name");

        // (126). A boxed value takes no modifier, no inheritance and no
        // body: the type it boxes follows the name directly. Anything
        // that can begin a type specification here is that type, and the
        // four tokens below are the other forms' own.
        if !modified
            && !matches!(
                self.current(),
                TokenKind::Colon
                    | TokenKind::Supports
                    | TokenKind::LBrace
                    | TokenKind::Semicolon
                    | TokenKind::Eof
            )
        {
            self.parse_type_spec();
            self.expect(TokenKind::Semicolon, "`;` after the boxed value");
            self.parse_suffix_annotations();
            self.start_at(start, CstKind::ValueBoxDecl);
            self.finish();
            return;
        }

        self.start_at(start, CstKind::ValuetypeDecl);
        if self.current() == TokenKind::Colon {
            self.start(CstKind::InheritanceClause);
            self.bump();
            let _ = self.eat(TokenKind::Truncatable);
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.finish();
        }
        if self.current() == TokenKind::Supports {
            self.start(CstKind::InheritanceClause);
            self.bump();
            loop {
                self.parse_scoped_name();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.finish();
        }
        // A forward-declared valuetype has no body: `valuetype V;`.
        if self.eat(TokenKind::Semicolon) {
            self.finish();
            return;
        }
        if self.expect(TokenKind::LBrace, "`{` opening the valuetype body") {
            while !self.at_eof() && self.current() != TokenKind::RBrace {
                let before = self.remaining();
                self.parse_value_element();
                if self.remaining() == before {
                    self.error_token("expected a valuetype element");
                }
            }
            self.expect(TokenKind::RBrace, "`}` closing the valuetype body");
        }
        self.expect(TokenKind::Semicolon, "`;` after the valuetype");
        self.parse_suffix_annotations();
        self.finish();
    }

    /// The members of a struct or exception body.
    fn parse_member_list(&mut self) {
        while !self.at_eof() && self.current() != TokenKind::RBrace {
            let before = self.remaining();
            self.start(CstKind::Member);
            while self.current() == TokenKind::At {
                self.parse_annotation();
            }
            if self.current() == TokenKind::RBrace || self.at_eof() {
                self.finish();
                break;
            }
            self.parse_type_spec();
            self.parse_declarator_list();
            self.expect(TokenKind::Semicolon, "`;` after the member");
            self.parse_suffix_annotations();
            self.finish();
            if self.remaining() == before {
                self.error_token("expected a member");
            }
        }
    }

    /// One or more declarators, comma separated.
    fn parse_declarator_list(&mut self) {
        loop {
            self.start(CstKind::Declarator);
            if !self.eat(TokenKind::Ident) {
                self.diagnose(
                    DiagnosticCode::MissingToken,
                    "expected a declarator name",
                    "OMG IDL 4.2 §7.4.1.4.4",
                );
                self.finish();
                break;
            }
            // DDS-XTypes 1.3 §7.3.1.2.1.4: "To declare the elements of a
            // collection type external, apply the annotation to the
            // collection declaration like this: `Foo array_of_foo
            // @external [42];`". The annotations stand between the
            // declarator's name and its dimensions and apply to the
            // array's elements, as `sequence<@external Foo, 42>` does
            // for a sequence's.
            while self.current() == TokenKind::At {
                self.parse_annotation();
            }
            // Whether this is an array is only known at the `[`.
            while self.current() == TokenKind::LBracket {
                self.start(CstKind::ArrayDimension);
                self.bump();
                self.parse_const_expr();
                self.expect(TokenKind::RBracket, "`]` closing the array dimension");
                self.finish();
            }
            self.finish();
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
    }

    /// How many tokens are left, so a rule can tell whether it consumed
    /// anything without reaching into the cursor.
    pub(crate) fn remaining(&self) -> usize {
        self.tokens_left()
    }
}
