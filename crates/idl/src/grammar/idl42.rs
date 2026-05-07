// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL 4.2 Grammar — `IDL_42`-Konstante und alle Productions.
//!
//! Die Grammar wird ueber die Tasks T3.1–T3.10 inkrementell aufgebaut.
//! Jede Section spiegelt die jeweilige Spec-Section in OMG IDL 4.2:
//!
//! - **§7.2** Lexical Conventions (T3.1)
//! - **§7.4.1.1–3** `<specification>`, `<definition>`, `<module>` (T3.2)
//! - **§7.4.1.4.1–2** `<type_decl>`, `<type_spec>`, `<simple_type_spec>` (T3.3)
//! - **§7.4.1.4.4.1** Primitive Types (T3.4)
//! - **§7.4.1.4.3** `<template_type_spec>` (T3.5)
//! - **§7.4.1.4.4.2** `<struct_def>`, `<member>`, `<declarators>` (T3.6)
//! - **§7.4.1.4.4.4** `<enum_dcl>`, `<enumerator>` (T3.7)
//! - **§7.4.1.4.4.5** `<const_decl>` (T3.8)
//! - **§7.4.1.4.4.6** `<typedef_decl>`, `<type_declarator>` (T3.9)
//! - **§7.4.1.4.4.7** `<except_decl>` (T3.10)
//!
//! ## Designentscheidung: manuelles Repeat/Choice-Desugaring
//!
//! Die Earley-Engine in Phase 0 behandelt `Symbol::Repeat` und
//! `Symbol::Choice` nicht direkt. Wiederholungen in der IDL-BNF werden
//! daher als rekursive Productions geschrieben — z.B.
//!
//! ```text
//! <member_list> ::= <member>
//!                | <member_list> <member>
//! ```
//!
//! statt der EBNF-Form `<member>+`. Das ist mehr Schreibarbeit, haelt
//! aber die Engine sauber. Engine-Erweiterung um Repeat/Choice ist als
//! spaetere Optimierung vorgesehen.
//!
//! ## Production-IDs
//!
//! IDs sind als `pub const` aufgefuehrt — stabil ueber alle
//! `IDL_42`-Aenderungen, damit externe Werkzeuge (`tools/idlc`,
//! `tools/traceability`) auf sie verweisen koennen. Die `IDL_42.productions`-
//! Reihenfolge entspricht den IDs.

// Production-Konstanten und ihre IDs sind durch Section-Kommentare
// gruppiert dokumentiert; einzelne Doc-Comments waeren redundant.
#![allow(missing_docs)]

use super::{
    AltRef, Alternative, Grammar, IdlVersion, Production, ProductionId, RepeatKind, SpecRef,
    Symbol, TokenKind,
};

// ============================================================================
// Spec-Anker
// ============================================================================

const fn spec(section: &'static str) -> SpecRef {
    SpecRef {
        doc: "OMG IDL 4.2",
        section,
    }
}

const fn alt(symbols: &'static [Symbol]) -> Alternative {
    Alternative {
        name: None,
        symbols,
        note: None,
    }
}

const fn named_alt(name: &'static str, symbols: &'static [Symbol]) -> Alternative {
    Alternative {
        name: Some(name),
        symbols,
        note: None,
    }
}

// ============================================================================
// Production-ID-Konstanten
// ============================================================================
// Nach Section gruppiert. Reihenfolge stabil — niemals umnummerieren ohne
// Code-Generator-Update.

// §7.2 Lexical Conventions (T3.1) — Wrapper-Productions ueber TokenKind.
pub const ID_IDENTIFIER: ProductionId = ProductionId(0);
pub const ID_INTEGER_LITERAL: ProductionId = ProductionId(1);
pub const ID_FLOATING_PT_LITERAL: ProductionId = ProductionId(2);
pub const ID_FIXED_PT_LITERAL: ProductionId = ProductionId(3);
pub const ID_CHARACTER_LITERAL: ProductionId = ProductionId(4);
pub const ID_WIDE_CHARACTER_LITERAL: ProductionId = ProductionId(5);
pub const ID_STRING_LITERAL: ProductionId = ProductionId(6);
pub const ID_WIDE_STRING_LITERAL: ProductionId = ProductionId(7);
pub const ID_BOOLEAN_LITERAL: ProductionId = ProductionId(8);

// §7.4.1.1–3 Specification / Definition / Module (T3.2).
pub const ID_SPECIFICATION: ProductionId = ProductionId(9);
pub const ID_DEFINITION_LIST: ProductionId = ProductionId(10);
pub const ID_DEFINITION: ProductionId = ProductionId(11);
pub const ID_MODULE_DCL: ProductionId = ProductionId(12);

// §7.4.1.4 Type Decls (T3.3).
pub const ID_TYPE_DCL: ProductionId = ProductionId(13);
pub const ID_TYPE_SPEC: ProductionId = ProductionId(14);
pub const ID_SIMPLE_TYPE_SPEC: ProductionId = ProductionId(15);
pub const ID_SCOPED_NAME: ProductionId = ProductionId(16);
pub const ID_SCOPED_NAME_TAIL: ProductionId = ProductionId(17);

// Enum Defs (T3.7).
pub const ID_ENUM_DCL: ProductionId = ProductionId(42);
pub const ID_ENUMERATOR_LIST: ProductionId = ProductionId(43);
pub const ID_ENUMERATOR: ProductionId = ProductionId(44);

// Const Decls (T3.8).
pub const ID_CONST_DCL: ProductionId = ProductionId(45);
pub const ID_CONST_TYPE: ProductionId = ProductionId(46);
pub const ID_CONST_EXPR: ProductionId = ProductionId(47);
pub const ID_LITERAL: ProductionId = ProductionId(48);

// Except Decl (T3.10).
pub const ID_EXCEPT_DCL: ProductionId = ProductionId(56);

// Interface (T-LIM-5).
pub const ID_INTERFACE_DCL: ProductionId = ProductionId(74);
pub const ID_INTERFACE_DEF: ProductionId = ProductionId(75);
pub const ID_INTERFACE_FORWARD_DCL: ProductionId = ProductionId(76);
pub const ID_INTERFACE_HEADER: ProductionId = ProductionId(77);
pub const ID_INTERFACE_KIND: ProductionId = ProductionId(78);
pub const ID_INTERFACE_INHERITANCE_SPEC: ProductionId = ProductionId(79);
pub const ID_INTERFACE_NAME_LIST: ProductionId = ProductionId(80);
pub const ID_INTERFACE_BODY: ProductionId = ProductionId(81);
pub const ID_EXPORT_LIST: ProductionId = ProductionId(82);
pub const ID_EXPORT: ProductionId = ProductionId(83);
pub const ID_OP_DCL: ProductionId = ProductionId(84);
pub const ID_OP_TYPE_SPEC: ProductionId = ProductionId(85);
pub const ID_PARAMETER_DCLS: ProductionId = ProductionId(86);
pub const ID_PARAM_DCL_LIST: ProductionId = ProductionId(87);
pub const ID_PARAM_DCL: ProductionId = ProductionId(88);
pub const ID_PARAM_ATTRIBUTE: ProductionId = ProductionId(89);
pub const ID_RAISES_EXPR: ProductionId = ProductionId(90);
pub const ID_SCOPED_NAME_LIST: ProductionId = ProductionId(91);
pub const ID_ATTR_DCL: ProductionId = ProductionId(92);

// Valuetype minimal (T-LIM-5).
pub const ID_VALUE_BOX_DCL: ProductionId = ProductionId(93);
pub const ID_VALUE_FORWARD_DCL: ProductionId = ProductionId(94);

// Annotations §8.3 (T4.4).
pub const ID_ANNOTATION_APPL_SEQ: ProductionId = ProductionId(95);
pub const ID_ANNOTATION_APPL: ProductionId = ProductionId(96);
pub const ID_ANNOTATION_APPL_PARAMS: ProductionId = ProductionId(97);
pub const ID_ANNOTATION_APPL_PARAM_LIST: ProductionId = ProductionId(98);
pub const ID_ANNOTATION_APPL_PARAM: ProductionId = ProductionId(99);

// §7.4.3.1 Rules 90-97 — multi-name attr + raises (R-blocker RPC).
pub const ID_READONLY_ATTR_SPEC: ProductionId = ProductionId(108);
pub const ID_READONLY_ATTR_DECLARATOR: ProductionId = ProductionId(109);
pub const ID_ATTR_SPEC: ProductionId = ProductionId(110);
pub const ID_ATTR_DECLARATOR: ProductionId = ProductionId(111);
pub const ID_ATTR_RAISES_EXPR: ProductionId = ProductionId(112);
pub const ID_GET_EXCEP_EXPR: ProductionId = ProductionId(113);
pub const ID_SET_EXCEP_EXPR: ProductionId = ProductionId(114);
pub const ID_EXCEPTION_LIST: ProductionId = ProductionId(115);

// §7.4.15 Rules 218-223 — User-Defined @annotation Declarations.
pub const ID_ANNOTATION_DCL: ProductionId = ProductionId(116);
pub const ID_ANNOTATION_HEADER: ProductionId = ProductionId(117);
pub const ID_ANNOTATION_BODY: ProductionId = ProductionId(118);
pub const ID_ANNOTATION_BODY_MEMBER: ProductionId = ProductionId(119);
pub const ID_ANNOTATION_MEMBER: ProductionId = ProductionId(120);

// §7.4.1.3 Rule 61 — Native-Type-Decl.
pub const ID_NATIVE_DCL: ProductionId = ProductionId(121);

// §7.4.5 Rules 100-109 — Value Types Full.
pub const ID_VALUE_DEF: ProductionId = ProductionId(122);
pub const ID_VALUE_HEADER: ProductionId = ProductionId(123);
pub const ID_VALUE_INHERITANCE_SPEC: ProductionId = ProductionId(124);
pub const ID_VALUE_ELEMENT: ProductionId = ProductionId(125);
pub const ID_STATE_MEMBER: ProductionId = ProductionId(126);
pub const ID_INIT_DCL: ProductionId = ProductionId(127);
pub const ID_INIT_PARAM_DCLS: ProductionId = ProductionId(128);
pub const ID_INIT_PARAM_DCL: ProductionId = ProductionId(129);
pub const ID_VALUE_INHERITANCE_NAME_LIST: ProductionId = ProductionId(130);

// §7.4.6 Rules 111-117 — Repository-IDs / Typeprefix / Import.
pub const ID_TYPE_ID_DCL: ProductionId = ProductionId(131);
pub const ID_TYPE_PREFIX_DCL: ProductionId = ProductionId(132);
pub const ID_IMPORT_DCL: ProductionId = ProductionId(133);
pub const ID_IMPORTED_SCOPE: ProductionId = ProductionId(134);

// §7.4.9 Components / Homes / Ports / Eventtypes (CCM).
pub const ID_COMPONENT_DCL: ProductionId = ProductionId(135);
pub const ID_COMPONENT_FORWARD_DCL: ProductionId = ProductionId(136);
pub const ID_COMPONENT_DEF: ProductionId = ProductionId(137);
pub const ID_COMPONENT_HEADER: ProductionId = ProductionId(138);
pub const ID_COMPONENT_INHERITANCE_SPEC: ProductionId = ProductionId(139);
pub const ID_SUPPORTED_INTERFACE_SPEC: ProductionId = ProductionId(140);
pub const ID_COMPONENT_BODY: ProductionId = ProductionId(141);
pub const ID_COMPONENT_EXPORT: ProductionId = ProductionId(142);
pub const ID_PROVIDES_DCL: ProductionId = ProductionId(143);
pub const ID_USES_DCL: ProductionId = ProductionId(144);
pub const ID_INTERFACE_TYPE: ProductionId = ProductionId(145);
pub const ID_HOME_DCL: ProductionId = ProductionId(146);
pub const ID_HOME_HEADER: ProductionId = ProductionId(147);
pub const ID_HOME_INHERITANCE_SPEC: ProductionId = ProductionId(148);
pub const ID_PRIMARY_KEY_SPEC: ProductionId = ProductionId(149);
pub const ID_HOME_BODY: ProductionId = ProductionId(150);
pub const ID_HOME_EXPORT: ProductionId = ProductionId(151);
pub const ID_FACTORY_DCL: ProductionId = ProductionId(152);
pub const ID_FACTORY_PARAM_DCLS: ProductionId = ProductionId(153);
pub const ID_FINDER_DCL: ProductionId = ProductionId(154);
pub const ID_EVENT_DCL: ProductionId = ProductionId(155);
pub const ID_EVENT_FORWARD_DCL: ProductionId = ProductionId(156);
pub const ID_EVENT_DEF: ProductionId = ProductionId(157);
pub const ID_EVENT_HEADER: ProductionId = ProductionId(158);
pub const ID_EMITS_DCL: ProductionId = ProductionId(159);
pub const ID_PUBLISHES_DCL: ProductionId = ProductionId(160);
pub const ID_CONSUMES_DCL: ProductionId = ProductionId(161);
pub const ID_PORTTYPE_DCL: ProductionId = ProductionId(162);
pub const ID_PORT_BODY: ProductionId = ProductionId(163);
pub const ID_PORT_REF: ProductionId = ProductionId(164);
pub const ID_PORT_EXPORT: ProductionId = ProductionId(165);
pub const ID_PORT_DCL: ProductionId = ProductionId(166);
pub const ID_CONNECTOR_DCL: ProductionId = ProductionId(167);
pub const ID_CONNECTOR_HEADER: ProductionId = ProductionId(168);
pub const ID_CONNECTOR_INHERIT_SPEC: ProductionId = ProductionId(169);
pub const ID_CONNECTOR_EXPORT: ProductionId = ProductionId(170);

// §7.4.12 Template Modules (CORBA, Rules 184-194).
pub const ID_TEMPLATE_MODULE_DCL: ProductionId = ProductionId(171);
pub const ID_FORMAL_PARAMETERS: ProductionId = ProductionId(172);
pub const ID_FORMAL_PARAMETER: ProductionId = ProductionId(173);
pub const ID_FORMAL_PARAMETER_TYPE: ProductionId = ProductionId(174);
pub const ID_TPL_DEFINITION: ProductionId = ProductionId(175);
pub const ID_TEMPLATE_MODULE_INST: ProductionId = ProductionId(176);
pub const ID_ACTUAL_PARAMETERS: ProductionId = ProductionId(177);
pub const ID_ACTUAL_PARAMETER: ProductionId = ProductionId(178);
pub const ID_TEMPLATE_MODULE_REF: ProductionId = ProductionId(179);
pub const ID_FORMAL_PARAMETER_NAMES: ProductionId = ProductionId(180);
pub const ID_OP_WITH_CONTEXT: ProductionId = ProductionId(181);
pub const ID_CONTEXT_EXPR: ProductionId = ProductionId(182);
pub const ID_CONTEXT_STRING_LIST: ProductionId = ProductionId(183);
pub const ID_VALUE_BASE_TYPE: ProductionId = ProductionId(184);

// Map / Bitset / Bitmask / Any (T4.6).
pub const ID_MAP_TYPE: ProductionId = ProductionId(100);
pub const ID_BITSET_DCL: ProductionId = ProductionId(101);
pub const ID_BITFIELD_LIST: ProductionId = ProductionId(102);
pub const ID_BITFIELD: ProductionId = ProductionId(103);
pub const ID_BITFIELD_SPEC: ProductionId = ProductionId(104);
pub const ID_BITMASK_DCL: ProductionId = ProductionId(105);
pub const ID_BIT_VALUE_LIST: ProductionId = ProductionId(106);
pub const ID_ANY_TYPE: ProductionId = ProductionId(107);

// Union (T-LIM-4).
pub const ID_UNION_DCL: ProductionId = ProductionId(65);
pub const ID_UNION_DEF: ProductionId = ProductionId(66);
pub const ID_UNION_FORWARD_DCL: ProductionId = ProductionId(67);
pub const ID_SWITCH_TYPE_SPEC: ProductionId = ProductionId(68);
pub const ID_SWITCH_BODY: ProductionId = ProductionId(69);
pub const ID_CASE: ProductionId = ProductionId(70);
pub const ID_CASE_LABELS: ProductionId = ProductionId(71);
pub const ID_CASE_LABEL: ProductionId = ProductionId(72);
pub const ID_ELEMENT_SPEC: ProductionId = ProductionId(73);

// const_expr Operator-Stack (T-LIM-3). Praezedenz von oben nach unten:
// or → xor → and → shift → add → mult → unary → primary.
pub const ID_OR_EXPR: ProductionId = ProductionId(57);
pub const ID_XOR_EXPR: ProductionId = ProductionId(58);
pub const ID_AND_EXPR: ProductionId = ProductionId(59);
pub const ID_SHIFT_EXPR: ProductionId = ProductionId(60);
pub const ID_ADD_EXPR: ProductionId = ProductionId(61);
pub const ID_MULT_EXPR: ProductionId = ProductionId(62);
pub const ID_UNARY_EXPR: ProductionId = ProductionId(63);
pub const ID_PRIMARY_EXPR: ProductionId = ProductionId(64);

// Typedef + Array-Declarators (T3.9).
pub const ID_TYPEDEF_DCL: ProductionId = ProductionId(49);
pub const ID_TYPE_DECLARATOR: ProductionId = ProductionId(50);
pub const ID_ANY_DECLARATORS: ProductionId = ProductionId(51);
pub const ID_ANY_DECLARATOR: ProductionId = ProductionId(52);
pub const ID_ARRAY_DECLARATOR: ProductionId = ProductionId(53);
pub const ID_FIXED_ARRAY_SIZES: ProductionId = ProductionId(54);
pub const ID_FIXED_ARRAY_SIZE: ProductionId = ProductionId(55);

// Struct Defs (T3.6).
pub const ID_CONSTR_TYPE_DCL: ProductionId = ProductionId(33);
pub const ID_STRUCT_DCL: ProductionId = ProductionId(34);
pub const ID_STRUCT_DEF: ProductionId = ProductionId(35);
pub const ID_STRUCT_FORWARD_DCL: ProductionId = ProductionId(36);
pub const ID_MEMBER_LIST: ProductionId = ProductionId(37);
pub const ID_MEMBER: ProductionId = ProductionId(38);
pub const ID_DECLARATORS: ProductionId = ProductionId(39);
pub const ID_DECLARATOR: ProductionId = ProductionId(40);
pub const ID_SIMPLE_DECLARATOR: ProductionId = ProductionId(41);

// Template Types (T3.5).
pub const ID_TEMPLATE_TYPE_SPEC: ProductionId = ProductionId(27);
pub const ID_SEQUENCE_TYPE: ProductionId = ProductionId(28);
pub const ID_STRING_TYPE: ProductionId = ProductionId(29);
pub const ID_WIDE_STRING_TYPE: ProductionId = ProductionId(30);
pub const ID_FIXED_PT_TYPE: ProductionId = ProductionId(31);
pub const ID_POSITIVE_INT_CONST: ProductionId = ProductionId(32);

// Primitive Types (T3.4) — Forward-Declared, Productions folgen unten.
pub const ID_BASE_TYPE_SPEC: ProductionId = ProductionId(18);
pub const ID_INTEGER_TYPE: ProductionId = ProductionId(19);
pub const ID_SIGNED_INT: ProductionId = ProductionId(20);
pub const ID_UNSIGNED_INT: ProductionId = ProductionId(21);
pub const ID_FLOATING_PT_TYPE: ProductionId = ProductionId(22);
pub const ID_CHAR_TYPE: ProductionId = ProductionId(23);
pub const ID_WIDE_CHAR_TYPE: ProductionId = ProductionId(24);
pub const ID_BOOLEAN_TYPE: ProductionId = ProductionId(25);
pub const ID_OCTET_TYPE: ProductionId = ProductionId(26);

// ============================================================================
// §7.2 Lexical Conventions (T3.1)
// ============================================================================
// Trivialer 1:1-Wrapper pro Lexical-Token-Klasse. Diese Productions
// erlauben es, in syntaktischen Productions per Nonterminal-Referenz auf
// Lexical-Tokens zu verweisen — z.B. `<member> ::= <type_spec> <identifier>`
// statt `<member> ::= <type_spec> Ident-Terminal`.

pub const PROD_IDENTIFIER: Production = Production {
    id: ID_IDENTIFIER,
    name: "identifier",
    spec_ref: spec("7.2.4"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Ident)])],
    ast_hint: None,
};

pub const PROD_INTEGER_LITERAL: Production = Production {
    id: ID_INTEGER_LITERAL,
    name: "integer_literal",
    spec_ref: spec("7.2.6.1"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::IntegerLiteral)])],
    ast_hint: None,
};

pub const PROD_FLOATING_PT_LITERAL: Production = Production {
    id: ID_FLOATING_PT_LITERAL,
    name: "floating_pt_literal",
    spec_ref: spec("7.2.6.4"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::FloatLiteral)])],
    ast_hint: None,
};

pub const PROD_FIXED_PT_LITERAL: Production = Production {
    id: ID_FIXED_PT_LITERAL,
    name: "fixed_pt_literal",
    spec_ref: spec("7.2.6.6"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::FixedPtLiteral)])],
    ast_hint: None,
};

pub const PROD_CHARACTER_LITERAL: Production = Production {
    id: ID_CHARACTER_LITERAL,
    name: "character_literal",
    spec_ref: spec("7.2.6.2"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::CharLiteral)])],
    ast_hint: None,
};

pub const PROD_WIDE_CHARACTER_LITERAL: Production = Production {
    id: ID_WIDE_CHARACTER_LITERAL,
    name: "wide_character_literal",
    spec_ref: spec("7.2.6.3"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::WideCharLiteral)])],
    ast_hint: None,
};

pub const PROD_STRING_LITERAL: Production = Production {
    id: ID_STRING_LITERAL,
    name: "string_literal",
    spec_ref: spec("7.2.6.5"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::StringLiteral)])],
    ast_hint: None,
};

pub const PROD_WIDE_STRING_LITERAL: Production = Production {
    id: ID_WIDE_STRING_LITERAL,
    name: "wide_string_literal",
    spec_ref: spec("7.2.6.5"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::WideStringLiteral)])],
    ast_hint: None,
};

pub const PROD_BOOLEAN_LITERAL: Production = Production {
    id: ID_BOOLEAN_LITERAL,
    name: "boolean_literal",
    spec_ref: spec("7.2.6.7"),
    alternatives: &[
        named_alt("true", &[Symbol::Terminal(TokenKind::Keyword("TRUE"))]),
        named_alt("false", &[Symbol::Terminal(TokenKind::Keyword("FALSE"))]),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.1–3 Specification, Definition, Module (T3.2)
// ============================================================================
// BNF aus IDL 4.2 §7.4.1.1:
//   <specification> ::= <definition> +
// Desugared zu rekursiver Liste:
//   <specification>     ::= <definition_list>
//   <definition_list>   ::= <definition>
//                        | <definition_list> <definition>

pub const PROD_SPECIFICATION: Production = Production {
    id: ID_SPECIFICATION,
    name: "specification",
    spec_ref: spec("7.4.1.1"),
    alternatives: &[alt(&[Symbol::Nonterminal(ID_DEFINITION_LIST)])],
    ast_hint: None,
};

// Spec-Abweichung: leere Definition-Liste wird erlaubt (`module M {}`).
// IDL 4.2 §7.4.1.3 fordert strikt `<definition>+`, aber praktisch alle
// DDS-Vendoren akzeptieren leere Module — Migration aus Bestands-IDL
// wuerde sonst scheitern. Dokumentiert per Note.
pub const PROD_DEFINITION_LIST: Production = Production {
    id: ID_DEFINITION_LIST,
    name: "definition_list",
    spec_ref: spec("7.4.1.1"),
    alternatives: &[
        Alternative {
            name: Some("empty"),
            symbols: &[],
            note: Some("Spec fordert <definition>+, hier zugelassen fuer Vendor-Kompatibilitaet."),
        },
        named_alt("single", &[Symbol::Nonterminal(ID_DEFINITION)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_DEFINITION_LIST),
                Symbol::Nonterminal(ID_DEFINITION),
            ],
        ),
    ],
    ast_hint: None,
};

// BNF aus IDL 4.2 §7.4.1.2:
//   <definition> ::= <module_dcl> ";"
//                  | <const_dcl> ";"
//                  | <type_dcl> ";"
//                  | <except_dcl> ";"
//                  | ...
// T3.2 hatte nur <module_dcl>; T3.3 ergaenzt <type_dcl>, weitere
// Definitionen folgen in T3.7-T3.10.
pub const PROD_DEFINITION: Production = Production {
    id: ID_DEFINITION,
    name: "definition",
    spec_ref: spec("7.4.1.2"),
    alternatives: &[
        named_alt(
            "module",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_MODULE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "type",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_TYPE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "const",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_CONST_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "except",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_EXCEPT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "interface",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_INTERFACE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "value_box",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_VALUE_BOX_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "value_forward",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_VALUE_FORWARD_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // §7.4.5 Rule 101 — Value Types Full (CORBA-Profil, gated via
        // `corba_value_types_full` feature).
        named_alt(
            "value_def",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_VALUE_DEF),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // §7.4.6 Rules 111/112/115 — Repository-IDs / Typeprefix / Import
        // (CORBA-Profil, gated via `corba_repository_ids` / `corba_import`).
        named_alt(
            "type_id",
            &[
                Symbol::Nonterminal(ID_TYPE_ID_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "type_prefix",
            &[
                Symbol::Nonterminal(ID_TYPE_PREFIX_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "import",
            &[
                Symbol::Nonterminal(ID_IMPORT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // §7.4.9 Components/Homes/Eventtypes/Ports/Connectors (CCM,
        // gated via corba_components/corba_homes/corba_eventtypes/
        // corba_ports).
        named_alt(
            "component",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_COMPONENT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "home",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_HOME_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "event",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_EVENT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "porttype",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_PORTTYPE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "connector",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_CONNECTOR_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // §7.4.12 Template Modules (CORBA, gated via corba_template_modules).
        named_alt(
            "template_module",
            &[
                Symbol::Nonterminal(ID_TEMPLATE_MODULE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "template_inst",
            &[
                Symbol::Nonterminal(ID_TEMPLATE_MODULE_INST),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // §7.4.15 Rule 218 — User-Defined Annotation Declarations.
        // Hinweis: KEIN annotation_appl_seq-Prefix; Annotation-Defs
        // selbst koennen nicht annotiert werden (Spec-konform).
        named_alt(
            "annotation_dcl",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

// BNF aus IDL 4.2 §7.4.1.3:
//   <module_dcl> ::= "module" <identifier> "{" <definition_list> "}"
pub const PROD_MODULE_DCL: Production = Production {
    id: ID_MODULE_DCL,
    name: "module_dcl",
    spec_ref: spec("7.4.1.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("module")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_DEFINITION_LIST),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4 Type Declarations / Type-Specs (T3.3)
// ============================================================================
// BNF aus IDL 4.2 §7.4.1.4:
//   <type_dcl> ::= <constr_type_dcl> | <native_dcl> | <typedef_dcl>
// T3.3 macht <type_dcl> zu einem Forward-Hub; konkrete Alternativen
// BNF §7.4.1.4:
//   <type_dcl> ::= <constr_type_dcl> | <native_dcl> | <typedef_dcl>
pub const PROD_TYPE_DCL: Production = Production {
    id: ID_TYPE_DCL,
    name: "type_dcl",
    spec_ref: spec("7.4.1.4"),
    alternatives: &[
        named_alt("typedef", &[Symbol::Nonterminal(ID_TYPEDEF_DCL)]),
        named_alt("constr_type", &[Symbol::Nonterminal(ID_CONSTR_TYPE_DCL)]),
        named_alt("native", &[Symbol::Nonterminal(ID_NATIVE_DCL)]),
    ],
    ast_hint: None,
};

// §7.4.1.3 Rule 61: <native_dcl> ::= "native" <simple_declarator>
//
// Spec-Hinweis: native deklariert einen plattform-spezifischen Type
// ohne IDL-Definition. CORBA-Konstrukt; in DDS selten genutzt, aber
// fuer Repository-/Migration-Use-Cases relevant.
pub const PROD_NATIVE_DCL: Production = Production {
    id: ID_NATIVE_DCL,
    name: "native_dcl",
    spec_ref: spec("7.4.1.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("native")),
        Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
    ])],
    ast_hint: None,
};

// BNF §7.4.1.4.1:
//   <type_spec> ::= <simple_type_spec> | <template_type_spec>
pub const PROD_TYPE_SPEC: Production = Production {
    id: ID_TYPE_SPEC,
    name: "type_spec",
    spec_ref: spec("7.4.1.4.1"),
    alternatives: &[
        named_alt("simple", &[Symbol::Nonterminal(ID_SIMPLE_TYPE_SPEC)]),
        named_alt("template", &[Symbol::Nonterminal(ID_TEMPLATE_TYPE_SPEC)]),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.2:
//   <simple_type_spec> ::= <base_type_spec> | <scoped_name>
// <base_type_spec> kommt in T3.4.
pub const PROD_SIMPLE_TYPE_SPEC: Production = Production {
    id: ID_SIMPLE_TYPE_SPEC,
    name: "simple_type_spec",
    spec_ref: spec("7.4.1.4.2"),
    alternatives: &[
        named_alt("base", &[Symbol::Nonterminal(ID_BASE_TYPE_SPEC)]),
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.2.x:
//   <scoped_name> ::= <identifier>
//                  | "::" <identifier> <scoped_name_tail>
//                  | <identifier> <scoped_name_tail>
//   <scoped_name_tail> ::= "::" <identifier>
//                       | "::" <identifier> <scoped_name_tail>
// Ermoeglicht z.B. `Foo`, `::Foo`, `Foo::Bar::Baz`.
pub const PROD_SCOPED_NAME: Production = Production {
    id: ID_SCOPED_NAME,
    name: "scoped_name",
    spec_ref: spec("7.4.1.4.2"),
    alternatives: &[
        named_alt("plain", &[Symbol::Nonterminal(ID_IDENTIFIER)]),
        named_alt(
            "absolute",
            &[
                Symbol::Terminal(TokenKind::Punct("::")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_SCOPED_NAME_TAIL),
            ],
        ),
        named_alt(
            "qualified",
            &[
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_SCOPED_NAME_TAIL),
            ],
        ),
        named_alt(
            "absolute_short",
            &[
                Symbol::Terminal(TokenKind::Punct("::")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_SCOPED_NAME_TAIL: Production = Production {
    id: ID_SCOPED_NAME_TAIL,
    name: "scoped_name_tail",
    spec_ref: spec("7.4.1.4.2"),
    alternatives: &[
        named_alt(
            "single",
            &[
                Symbol::Terminal(TokenKind::Punct("::")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "cons",
            &[
                Symbol::Terminal(TokenKind::Punct("::")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_SCOPED_NAME_TAIL),
            ],
        ),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.1 Primitive Base Types (T3.4)
// ============================================================================
// BNF §7.4.1.4.4.1.x:
//   <base_type_spec> ::= <integer_type> | <floating_pt_type>
//                      | <char_type> | <wide_char_type>
//                      | <boolean_type> | <octet_type>

pub const PROD_BASE_TYPE_SPEC: Production = Production {
    id: ID_BASE_TYPE_SPEC,
    name: "base_type_spec",
    spec_ref: spec("7.4.1.4.4.1"),
    alternatives: &[
        named_alt("integer", &[Symbol::Nonterminal(ID_INTEGER_TYPE)]),
        named_alt("floating", &[Symbol::Nonterminal(ID_FLOATING_PT_TYPE)]),
        named_alt("char", &[Symbol::Nonterminal(ID_CHAR_TYPE)]),
        named_alt("wide_char", &[Symbol::Nonterminal(ID_WIDE_CHAR_TYPE)]),
        named_alt("boolean", &[Symbol::Nonterminal(ID_BOOLEAN_TYPE)]),
        named_alt("octet", &[Symbol::Nonterminal(ID_OCTET_TYPE)]),
        // CORBA Any (T4.6) — wird in DDS-IDL selten genutzt, aber von
        // CORBA-Migrationen erwartet.
        named_alt("any", &[Symbol::Nonterminal(ID_ANY_TYPE)]),
        // Spec §7.4.6.3 Rule (117): <base_type_spec> ::+ <object_type>.
        // Inline `Object`-Keyword statt separater Production —
        // semantisch identisch zur `<object_type>`-Rule (118).
        named_alt("object", &[Symbol::Terminal(TokenKind::Keyword("Object"))]),
        // Spec §7.4.7.3 Rule (131): <base_type_spec> ::+ <value_base_type>.
        named_alt("value_base", &[Symbol::Nonterminal(ID_VALUE_BASE_TYPE)]),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.4.1.1:
//   <integer_type> ::= <signed_int> | <unsigned_int>
//   <signed_int>   ::= "short" | "long" | "long" "long"
//                    | "int8" | "int16" | "int32" | "int64"
//   <unsigned_int> ::= "unsigned" "short" | "unsigned" "long"
//                    | "unsigned" "long" "long"
//                    | "uint8" | "uint16" | "uint32" | "uint64"
pub const PROD_INTEGER_TYPE: Production = Production {
    id: ID_INTEGER_TYPE,
    name: "integer_type",
    spec_ref: spec("7.4.1.4.4.1.1"),
    alternatives: &[
        named_alt("signed", &[Symbol::Nonterminal(ID_SIGNED_INT)]),
        named_alt("unsigned", &[Symbol::Nonterminal(ID_UNSIGNED_INT)]),
    ],
    ast_hint: None,
};

pub const PROD_SIGNED_INT: Production = Production {
    id: ID_SIGNED_INT,
    name: "signed_int",
    spec_ref: spec("7.4.1.4.4.1.1"),
    alternatives: &[
        named_alt("short", &[Symbol::Terminal(TokenKind::Keyword("short"))]),
        named_alt(
            "long_long",
            &[
                Symbol::Terminal(TokenKind::Keyword("long")),
                Symbol::Terminal(TokenKind::Keyword("long")),
            ],
        ),
        named_alt("long", &[Symbol::Terminal(TokenKind::Keyword("long"))]),
        named_alt("int8", &[Symbol::Terminal(TokenKind::Keyword("int8"))]),
        named_alt("int16", &[Symbol::Terminal(TokenKind::Keyword("int16"))]),
        named_alt("int32", &[Symbol::Terminal(TokenKind::Keyword("int32"))]),
        named_alt("int64", &[Symbol::Terminal(TokenKind::Keyword("int64"))]),
    ],
    ast_hint: None,
};

pub const PROD_UNSIGNED_INT: Production = Production {
    id: ID_UNSIGNED_INT,
    name: "unsigned_int",
    spec_ref: spec("7.4.1.4.4.1.1"),
    alternatives: &[
        named_alt(
            "u_short",
            &[
                Symbol::Terminal(TokenKind::Keyword("unsigned")),
                Symbol::Terminal(TokenKind::Keyword("short")),
            ],
        ),
        named_alt(
            "u_long_long",
            &[
                Symbol::Terminal(TokenKind::Keyword("unsigned")),
                Symbol::Terminal(TokenKind::Keyword("long")),
                Symbol::Terminal(TokenKind::Keyword("long")),
            ],
        ),
        named_alt(
            "u_long",
            &[
                Symbol::Terminal(TokenKind::Keyword("unsigned")),
                Symbol::Terminal(TokenKind::Keyword("long")),
            ],
        ),
        named_alt("uint8", &[Symbol::Terminal(TokenKind::Keyword("uint8"))]),
        named_alt("uint16", &[Symbol::Terminal(TokenKind::Keyword("uint16"))]),
        named_alt("uint32", &[Symbol::Terminal(TokenKind::Keyword("uint32"))]),
        named_alt("uint64", &[Symbol::Terminal(TokenKind::Keyword("uint64"))]),
    ],
    ast_hint: None,
};

pub const PROD_FLOATING_PT_TYPE: Production = Production {
    id: ID_FLOATING_PT_TYPE,
    name: "floating_pt_type",
    spec_ref: spec("7.4.1.4.4.1.2"),
    alternatives: &[
        named_alt("float", &[Symbol::Terminal(TokenKind::Keyword("float"))]),
        named_alt(
            "long_double",
            &[
                Symbol::Terminal(TokenKind::Keyword("long")),
                Symbol::Terminal(TokenKind::Keyword("double")),
            ],
        ),
        named_alt("double", &[Symbol::Terminal(TokenKind::Keyword("double"))]),
    ],
    ast_hint: None,
};

pub const PROD_CHAR_TYPE: Production = Production {
    id: ID_CHAR_TYPE,
    name: "char_type",
    spec_ref: spec("7.4.1.4.4.1.3"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("char"))])],
    ast_hint: None,
};

pub const PROD_WIDE_CHAR_TYPE: Production = Production {
    id: ID_WIDE_CHAR_TYPE,
    name: "wide_char_type",
    spec_ref: spec("7.4.1.4.4.1.4"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("wchar"))])],
    ast_hint: None,
};

pub const PROD_BOOLEAN_TYPE: Production = Production {
    id: ID_BOOLEAN_TYPE,
    name: "boolean_type",
    spec_ref: spec("7.4.1.4.4.1.5"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("boolean"))])],
    ast_hint: None,
};

pub const PROD_OCTET_TYPE: Production = Production {
    id: ID_OCTET_TYPE,
    name: "octet_type",
    spec_ref: spec("7.4.1.4.4.1.6"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("octet"))])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.3 Template Types (T3.5)
// ============================================================================
// BNF §7.4.1.4.3:
//   <template_type_spec> ::= <sequence_type> | <string_type>
//                         | <wide_string_type> | <fixed_pt_type>

pub const PROD_TEMPLATE_TYPE_SPEC: Production = Production {
    id: ID_TEMPLATE_TYPE_SPEC,
    name: "template_type_spec",
    spec_ref: spec("7.4.1.4.3"),
    alternatives: &[
        named_alt("sequence", &[Symbol::Nonterminal(ID_SEQUENCE_TYPE)]),
        named_alt("string", &[Symbol::Nonterminal(ID_STRING_TYPE)]),
        named_alt("wstring", &[Symbol::Nonterminal(ID_WIDE_STRING_TYPE)]),
        named_alt("fixed", &[Symbol::Nonterminal(ID_FIXED_PT_TYPE)]),
        // map<K,V> — Extended Data Types BB §7.4.13 (T4.6).
        named_alt("map", &[Symbol::Nonterminal(ID_MAP_TYPE)]),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.3.1:
//   <sequence_type> ::= "sequence" "<" <type_spec> "," <positive_int_const> ">"
//                    | "sequence" "<" <type_spec> ">"
pub const PROD_SEQUENCE_TYPE: Production = Production {
    id: ID_SEQUENCE_TYPE,
    name: "sequence_type",
    spec_ref: spec("7.4.1.4.3.1"),
    alternatives: &[
        named_alt(
            "bounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("sequence")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
        named_alt(
            "unbounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("sequence")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.3.2:
//   <string_type> ::= "string" "<" <positive_int_const> ">"
//                  | "string"
pub const PROD_STRING_TYPE: Production = Production {
    id: ID_STRING_TYPE,
    name: "string_type",
    spec_ref: spec("7.4.1.4.3.2"),
    alternatives: &[
        named_alt(
            "bounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("string")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
        named_alt(
            "unbounded",
            &[Symbol::Terminal(TokenKind::Keyword("string"))],
        ),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.3.3:
//   <wide_string_type> ::= "wstring" "<" <positive_int_const> ">"
//                       | "wstring"
pub const PROD_WIDE_STRING_TYPE: Production = Production {
    id: ID_WIDE_STRING_TYPE,
    name: "wide_string_type",
    spec_ref: spec("7.4.1.4.3.3"),
    alternatives: &[
        named_alt(
            "bounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("wstring")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
        named_alt(
            "unbounded",
            &[Symbol::Terminal(TokenKind::Keyword("wstring"))],
        ),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.3.4:
//   <fixed_pt_type> ::= "fixed" "<" <positive_int_const> "," <positive_int_const> ">"
pub const PROD_FIXED_PT_TYPE: Production = Production {
    id: ID_FIXED_PT_TYPE,
    name: "fixed_pt_type",
    spec_ref: spec("7.4.1.4.3.4"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("fixed")),
        Symbol::Terminal(TokenKind::Punct("<")),
        Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
        Symbol::Terminal(TokenKind::Punct(",")),
        Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
        Symbol::Terminal(TokenKind::Punct(">")),
    ])],
    ast_hint: None,
};

// BNF §7.4.1.4.4.5 (Vorgriff aus T3.8):
//   <positive_int_const> ::= <integer_literal>
// Die echte Definition <const_expr> kommt in T3.8 mit Konstanten-
// Ausdruecken (Arithmetik, Scoped-Names). Vorerst nur Integer-Literal —
// reicht fuer alle bounded sequences/strings.
pub const PROD_POSITIVE_INT_CONST: Production = Production {
    id: ID_POSITIVE_INT_CONST,
    name: "positive_int_const",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[alt(&[Symbol::Nonterminal(ID_INTEGER_LITERAL)])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4 Constructed Types — Struct (T3.6)
// ============================================================================
// BNF §7.4.1.4.4:
//   <constr_type_dcl> ::= <struct_dcl> | <union_dcl> | <enum_dcl>
pub const PROD_CONSTR_TYPE_DCL: Production = Production {
    id: ID_CONSTR_TYPE_DCL,
    name: "constr_type_dcl",
    spec_ref: spec("7.4.1.4.4"),
    alternatives: &[
        named_alt("struct", &[Symbol::Nonterminal(ID_STRUCT_DCL)]),
        named_alt("union", &[Symbol::Nonterminal(ID_UNION_DCL)]),
        named_alt("enum", &[Symbol::Nonterminal(ID_ENUM_DCL)]),
        // Extended Data Types BB §7.4.13 (T4.6).
        named_alt("bitset", &[Symbol::Nonterminal(ID_BITSET_DCL)]),
        named_alt("bitmask", &[Symbol::Nonterminal(ID_BITMASK_DCL)]),
    ],
    ast_hint: None,
};

// BNF §7.4.1.4.4.2:
//   <struct_dcl> ::= <struct_def> | <struct_forward_dcl>
//   <struct_def> ::= "struct" <identifier> "{" <member_list> "}"
//   <struct_forward_dcl> ::= "struct" <identifier>
pub const PROD_STRUCT_DCL: Production = Production {
    id: ID_STRUCT_DCL,
    name: "struct_dcl",
    spec_ref: spec("7.4.1.4.4.2"),
    alternatives: &[
        named_alt("def", &[Symbol::Nonterminal(ID_STRUCT_DEF)]),
        named_alt("forward", &[Symbol::Nonterminal(ID_STRUCT_FORWARD_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_STRUCT_DEF: Production = Production {
    id: ID_STRUCT_DEF,
    name: "struct_def",
    spec_ref: spec("7.4.1.4.4.2"),
    alternatives: &[
        // Inheritance-Variante zuerst — Extended Data Types BB (§7.4.13)
        // erweitert struct um ":" <scoped_name>. Praktisch verwendet von
        // RTI/OpenSplice fuer DDS-XTypes-Vererbung.
        named_alt(
            "with_base",
            &[
                Symbol::Terminal(TokenKind::Keyword("struct")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct(":")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Nonterminal(ID_MEMBER_LIST),
                Symbol::Terminal(TokenKind::Punct("}")),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("struct")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Nonterminal(ID_MEMBER_LIST),
                Symbol::Terminal(TokenKind::Punct("}")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_STRUCT_FORWARD_DCL: Production = Production {
    id: ID_STRUCT_FORWARD_DCL,
    name: "struct_forward_dcl",
    spec_ref: spec("7.4.1.4.4.2"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("struct")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

// BNF §7.4.1.4.4.2:
//   <member_list> ::= <member> +
//   <member>      ::= <type_spec> <declarators> ";"
// Pragmatik: leere member_list zugelassen (analog definition_list,
// fuer Vendor-Kompatibilitaet "struct Empty {};").
pub const PROD_MEMBER_LIST: Production = Production {
    id: ID_MEMBER_LIST,
    name: "member_list",
    spec_ref: spec("7.4.1.4.4.2"),
    alternatives: &[
        Alternative {
            name: Some("empty"),
            symbols: &[],
            note: Some("Spec fordert <member>+, hier zugelassen fuer Vendor-Kompatibilitaet."),
        },
        named_alt("single", &[Symbol::Nonterminal(ID_MEMBER)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_MEMBER_LIST),
                Symbol::Nonterminal(ID_MEMBER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_MEMBER: Production = Production {
    id: ID_MEMBER,
    name: "member",
    spec_ref: spec("7.4.1.4.4.2"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_DECLARATORS),
        Symbol::Terminal(TokenKind::Punct(";")),
    ])],
    ast_hint: None,
};

// BNF §7.4.1.4.4.3:
//   <declarators> ::= <declarator>
//                  | <declarators> "," <declarator>
//   <declarator>  ::= <simple_declarator> | <complex_declarator>
//   <simple_declarator> ::= <identifier>
// <complex_declarator> (Arrays) folgt in T3.9 mit Typedef.
pub const PROD_DECLARATORS: Production = Production {
    id: ID_DECLARATORS,
    name: "declarators",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_DECLARATOR)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_DECLARATORS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_DECLARATOR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_DECLARATOR: Production = Production {
    id: ID_DECLARATOR,
    name: "declarator",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        // complex_declarator zuerst — Earley-Disambiguation greift bei
        // ident gefolgt von "[". OMG-Spec erlaubt array_declarator als
        // complex_declarator auch im struct-member-Kontext.
        named_alt("complex", &[Symbol::Nonterminal(ID_ARRAY_DECLARATOR)]),
        named_alt("simple", &[Symbol::Nonterminal(ID_SIMPLE_DECLARATOR)]),
    ],
    ast_hint: None,
};

pub const PROD_SIMPLE_DECLARATOR: Production = Production {
    id: ID_SIMPLE_DECLARATOR,
    name: "simple_declarator",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[alt(&[Symbol::Nonterminal(ID_IDENTIFIER)])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.3 Union (T-LIM-4)
// ============================================================================
// BNF §7.4.1.4.4.3:
//   <union_dcl> ::= <union_def> | <union_forward_dcl>
//   <union_def> ::= "union" <identifier> "switch" "(" <switch_type_spec> ")"
//                   "{" <switch_body> "}"
//   <union_forward_dcl> ::= "union" <identifier>
//   <switch_type_spec> ::= <integer_type> | <char_type> | <wide_char_type>
//                       | <boolean_type> | <enum_dcl> | <scoped_name>
//   <switch_body> ::= <case>+   (manuell desugart als rekursive Liste)
//   <case>        ::= <case_labels> <element_spec> ";"
//   <case_labels> ::= <case_label>
//                  | <case_labels> <case_label>
//   <case_label>  ::= "case" <const_expr> ":" | "default" ":"
//   <element_spec> ::= <type_spec> <declarator>

pub const PROD_UNION_DCL: Production = Production {
    id: ID_UNION_DCL,
    name: "union_dcl",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt("def", &[Symbol::Nonterminal(ID_UNION_DEF)]),
        named_alt("forward", &[Symbol::Nonterminal(ID_UNION_FORWARD_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_UNION_DEF: Production = Production {
    id: ID_UNION_DEF,
    name: "union_def",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("union")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Keyword("switch")),
        Symbol::Terminal(TokenKind::Punct("(")),
        Symbol::Nonterminal(ID_SWITCH_TYPE_SPEC),
        Symbol::Terminal(TokenKind::Punct(")")),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_SWITCH_BODY),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_UNION_FORWARD_DCL: Production = Production {
    id: ID_UNION_FORWARD_DCL,
    name: "union_forward_dcl",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("union")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_SWITCH_TYPE_SPEC: Production = Production {
    id: ID_SWITCH_TYPE_SPEC,
    name: "switch_type_spec",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt("integer", &[Symbol::Nonterminal(ID_INTEGER_TYPE)]),
        named_alt("char", &[Symbol::Nonterminal(ID_CHAR_TYPE)]),
        named_alt("wide_char", &[Symbol::Nonterminal(ID_WIDE_CHAR_TYPE)]),
        named_alt("boolean", &[Symbol::Nonterminal(ID_BOOLEAN_TYPE)]),
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        // Spec §7.4.13.3 Rule (196): <switch_type_spec> ::+
        // <wide_char_type> | <octet_type>. wide_char ist oben schon
        // dabei; octet wird hier ergänzt.
        named_alt("octet", &[Symbol::Nonterminal(ID_OCTET_TYPE)]),
    ],
    ast_hint: None,
};

pub const PROD_SWITCH_BODY: Production = Production {
    id: ID_SWITCH_BODY,
    name: "switch_body",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_CASE)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_SWITCH_BODY),
                Symbol::Nonterminal(ID_CASE),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_CASE: Production = Production {
    id: ID_CASE,
    name: "case",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
        Symbol::Nonterminal(ID_CASE_LABELS),
        Symbol::Nonterminal(ID_ELEMENT_SPEC),
        Symbol::Terminal(TokenKind::Punct(";")),
    ])],
    ast_hint: None,
};

// element_spec mit Annotation-Prefix (T4.4): erlaubt sowohl
// `case 1: @optional long val;` (Annotation auf Element) als auch
// `@optional case 1: long val;` (Annotation auf Case).

pub const PROD_CASE_LABELS: Production = Production {
    id: ID_CASE_LABELS,
    name: "case_labels",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_CASE_LABEL)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_CASE_LABELS),
                Symbol::Nonterminal(ID_CASE_LABEL),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_CASE_LABEL: Production = Production {
    id: ID_CASE_LABEL,
    name: "case_label",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[
        named_alt(
            "case",
            &[
                Symbol::Terminal(TokenKind::Keyword("case")),
                Symbol::Nonterminal(ID_CONST_EXPR),
                Symbol::Terminal(TokenKind::Punct(":")),
            ],
        ),
        named_alt(
            "default",
            &[
                Symbol::Terminal(TokenKind::Keyword("default")),
                Symbol::Terminal(TokenKind::Punct(":")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ELEMENT_SPEC: Production = Production {
    id: ID_ELEMENT_SPEC,
    name: "element_spec",
    spec_ref: spec("7.4.1.4.4.3"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_DECLARATOR),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.4 Enum (T3.7)
// ============================================================================
// BNF §7.4.1.4.4.4:
//   <enum_dcl> ::= "enum" <identifier> "{" <enumerator_list> "}"
//   <enumerator_list> ::= <enumerator>
//                      | <enumerator_list> "," <enumerator>
//   <enumerator> ::= <identifier>
// Enum erfordert mindestens 1 enumerator (keine Vendor-Pragmatik fuer
// leere Enums — die wuerden semantisch keinen Wert haben).

pub const PROD_ENUM_DCL: Production = Production {
    id: ID_ENUM_DCL,
    name: "enum_dcl",
    spec_ref: spec("7.4.1.4.4.4"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("enum")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_ENUMERATOR_LIST),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_ENUMERATOR_LIST: Production = Production {
    id: ID_ENUMERATOR_LIST,
    name: "enumerator_list",
    spec_ref: spec("7.4.1.4.4.4"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_ENUMERATOR)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_ENUMERATOR_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_ENUMERATOR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ENUMERATOR: Production = Production {
    id: ID_ENUMERATOR,
    name: "enumerator",
    spec_ref: spec("7.4.1.4.4.4"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.5 Const Decl (T3.8)
// ============================================================================
// BNF §7.4.1.4.4.5:
//   <const_dcl> ::= "const" <const_type> <identifier> "=" <const_expr>
//   <const_type> ::= <integer_type> | <floating_pt_type>
//                  | <fixed_pt_const_type> | <char_type> | <wide_char_type>
//                  | <boolean_type> | <octet_type> | <string_type>
//                  | <wide_string_type> | <scoped_name>
//   <const_expr> ::= <or_expr> | ...
//
// T3.8 vereinfacht <const_expr> zu Literal-only (analog T3.5
// positive_int_const). Volle Arithmetik (or/xor/and/shift/add/mul/unary)
// läuft im AST-Builder.

pub const PROD_CONST_DCL: Production = Production {
    id: ID_CONST_DCL,
    name: "const_dcl",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("const")),
        Symbol::Nonterminal(ID_CONST_TYPE),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("=")),
        Symbol::Nonterminal(ID_CONST_EXPR),
    ])],
    ast_hint: None,
};

pub const PROD_CONST_TYPE: Production = Production {
    id: ID_CONST_TYPE,
    name: "const_type",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("integer", &[Symbol::Nonterminal(ID_INTEGER_TYPE)]),
        named_alt("floating", &[Symbol::Nonterminal(ID_FLOATING_PT_TYPE)]),
        named_alt("char", &[Symbol::Nonterminal(ID_CHAR_TYPE)]),
        named_alt("wide_char", &[Symbol::Nonterminal(ID_WIDE_CHAR_TYPE)]),
        named_alt("boolean", &[Symbol::Nonterminal(ID_BOOLEAN_TYPE)]),
        named_alt("octet", &[Symbol::Nonterminal(ID_OCTET_TYPE)]),
        named_alt("string", &[Symbol::Nonterminal(ID_STRING_TYPE)]),
        named_alt("wide_string", &[Symbol::Nonterminal(ID_WIDE_STRING_TYPE)]),
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        // Spec §7.4.1.4.3 Rule (43): <fixed_pt_const_type> ::= "fixed".
        // Inline-Keyword genuegt — Production hat keinen weiteren Inhalt.
        named_alt(
            "fixed_pt_const",
            &[Symbol::Terminal(TokenKind::Keyword("fixed"))],
        ),
    ],
    ast_hint: None,
};

// const_expr ist der Top-Level-Wrapper; die echte Operator-Hierarchie
// folgt in T-LIM-3 (or → xor → and → shift → add → mult → unary → primary).
pub const PROD_CONST_EXPR: Production = Production {
    id: ID_CONST_EXPR,
    name: "const_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[alt(&[Symbol::Nonterminal(ID_OR_EXPR)])],
    ast_hint: None,
};

// <literal> ::= <integer_literal> | <floating_pt_literal>
//            | <fixed_pt_literal> | <character_literal>
//            | <wide_character_literal> | <boolean_literal>
//            | <string_literal> | <wide_string_literal>
pub const PROD_LITERAL: Production = Production {
    id: ID_LITERAL,
    name: "literal",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("integer", &[Symbol::Nonterminal(ID_INTEGER_LITERAL)]),
        named_alt("floating", &[Symbol::Nonterminal(ID_FLOATING_PT_LITERAL)]),
        named_alt("fixed", &[Symbol::Nonterminal(ID_FIXED_PT_LITERAL)]),
        named_alt("char", &[Symbol::Nonterminal(ID_CHARACTER_LITERAL)]),
        named_alt(
            "wide_char",
            &[Symbol::Nonterminal(ID_WIDE_CHARACTER_LITERAL)],
        ),
        named_alt("boolean", &[Symbol::Nonterminal(ID_BOOLEAN_LITERAL)]),
        named_alt("string", &[Symbol::Nonterminal(ID_STRING_LITERAL)]),
        named_alt(
            "wide_string",
            &[Symbol::Nonterminal(ID_WIDE_STRING_LITERAL)],
        ),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.5 const_expr Operator-Stack (T-LIM-3)
// ============================================================================
// BNF aus IDL 4.2 §7.4.1.4.4.5 (verkuerzt, klassische Praezedenz):
//   <or_expr>     ::= <xor_expr>   | <or_expr> "|" <xor_expr>
//   <xor_expr>    ::= <and_expr>   | <xor_expr> "^" <and_expr>
//   <and_expr>    ::= <shift_expr> | <and_expr> "&" <shift_expr>
//   <shift_expr>  ::= <add_expr>   | <shift_expr> "<<" <add_expr>
//                                  | <shift_expr> ">>" <add_expr>
//   <add_expr>    ::= <mult_expr>  | <add_expr> "+" <mult_expr>
//                                  | <add_expr> "-" <mult_expr>
//   <mult_expr>   ::= <unary_expr> | <mult_expr> "*" <unary_expr>
//                                  | <mult_expr> "/" <unary_expr>
//                                  | <mult_expr> "%" <unary_expr>
//   <unary_expr>  ::= "-" <primary_expr> | "+" <primary_expr>
//                  | "~" <primary_expr> | <primary_expr>
//   <primary_expr> ::= <scoped_name> | <literal>
//                   | "(" <const_expr> ")"
// Alle linksrekursiv, Engine handelt das via Earley.

pub const PROD_OR_EXPR: Production = Production {
    id: ID_OR_EXPR,
    name: "or_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_xor", &[Symbol::Nonterminal(ID_XOR_EXPR)]),
        named_alt(
            "or",
            &[
                Symbol::Nonterminal(ID_OR_EXPR),
                Symbol::Terminal(TokenKind::Punct("|")),
                Symbol::Nonterminal(ID_XOR_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_XOR_EXPR: Production = Production {
    id: ID_XOR_EXPR,
    name: "xor_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_and", &[Symbol::Nonterminal(ID_AND_EXPR)]),
        named_alt(
            "xor",
            &[
                Symbol::Nonterminal(ID_XOR_EXPR),
                Symbol::Terminal(TokenKind::Punct("^")),
                Symbol::Nonterminal(ID_AND_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_AND_EXPR: Production = Production {
    id: ID_AND_EXPR,
    name: "and_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_shift", &[Symbol::Nonterminal(ID_SHIFT_EXPR)]),
        named_alt(
            "and",
            &[
                Symbol::Nonterminal(ID_AND_EXPR),
                Symbol::Terminal(TokenKind::Punct("&")),
                Symbol::Nonterminal(ID_SHIFT_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

// HINWEIS: `>>` wird im Lexer NICHT als Multi-Char-Punct gefuehrt — es
// kollidiert mit nested template-Closures wie `sequence<sequence<long>>`.
// Stattdessen besteht der Right-Shift hier aus zwei einzelnen `>`-Tokens
// (klassischer C++-Workaround vor C++11). Der Lexer-Tokenizer wird deshalb
// kein `>>`-Token erzeugen, weil keine Production diese Punct-Regel nutzt.
pub const PROD_SHIFT_EXPR: Production = Production {
    id: ID_SHIFT_EXPR,
    name: "shift_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_add", &[Symbol::Nonterminal(ID_ADD_EXPR)]),
        named_alt(
            "shl",
            &[
                Symbol::Nonterminal(ID_SHIFT_EXPR),
                Symbol::Terminal(TokenKind::Punct("<<")),
                Symbol::Nonterminal(ID_ADD_EXPR),
            ],
        ),
        named_alt(
            "shr",
            &[
                Symbol::Nonterminal(ID_SHIFT_EXPR),
                Symbol::Terminal(TokenKind::Punct(">")),
                Symbol::Terminal(TokenKind::Punct(">")),
                Symbol::Nonterminal(ID_ADD_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ADD_EXPR: Production = Production {
    id: ID_ADD_EXPR,
    name: "add_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_mult", &[Symbol::Nonterminal(ID_MULT_EXPR)]),
        named_alt(
            "add",
            &[
                Symbol::Nonterminal(ID_ADD_EXPR),
                Symbol::Terminal(TokenKind::Punct("+")),
                Symbol::Nonterminal(ID_MULT_EXPR),
            ],
        ),
        named_alt(
            "sub",
            &[
                Symbol::Nonterminal(ID_ADD_EXPR),
                Symbol::Terminal(TokenKind::Punct("-")),
                Symbol::Nonterminal(ID_MULT_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_MULT_EXPR: Production = Production {
    id: ID_MULT_EXPR,
    name: "mult_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("just_unary", &[Symbol::Nonterminal(ID_UNARY_EXPR)]),
        named_alt(
            "mul",
            &[
                Symbol::Nonterminal(ID_MULT_EXPR),
                Symbol::Terminal(TokenKind::Punct("*")),
                Symbol::Nonterminal(ID_UNARY_EXPR),
            ],
        ),
        named_alt(
            "div",
            &[
                Symbol::Nonterminal(ID_MULT_EXPR),
                Symbol::Terminal(TokenKind::Punct("/")),
                Symbol::Nonterminal(ID_UNARY_EXPR),
            ],
        ),
        named_alt(
            "mod",
            &[
                Symbol::Nonterminal(ID_MULT_EXPR),
                Symbol::Terminal(TokenKind::Punct("%")),
                Symbol::Nonterminal(ID_UNARY_EXPR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_UNARY_EXPR: Production = Production {
    id: ID_UNARY_EXPR,
    name: "unary_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt(
            "neg",
            &[
                Symbol::Terminal(TokenKind::Punct("-")),
                Symbol::Nonterminal(ID_PRIMARY_EXPR),
            ],
        ),
        named_alt(
            "pos",
            &[
                Symbol::Terminal(TokenKind::Punct("+")),
                Symbol::Nonterminal(ID_PRIMARY_EXPR),
            ],
        ),
        named_alt(
            "bitnot",
            &[
                Symbol::Terminal(TokenKind::Punct("~")),
                Symbol::Nonterminal(ID_PRIMARY_EXPR),
            ],
        ),
        named_alt("just_primary", &[Symbol::Nonterminal(ID_PRIMARY_EXPR)]),
    ],
    ast_hint: None,
};

pub const PROD_PRIMARY_EXPR: Production = Production {
    id: ID_PRIMARY_EXPR,
    name: "primary_expr",
    spec_ref: spec("7.4.1.4.4.5"),
    alternatives: &[
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt("literal", &[Symbol::Nonterminal(ID_LITERAL)]),
        named_alt(
            "paren",
            &[
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Nonterminal(ID_CONST_EXPR),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.6 Typedef + Array-Declarators (T3.9)
// ============================================================================
// BNF §7.4.1.4.4.6:
//   <typedef_dcl> ::= "typedef" <type_declarator>
//   <type_declarator> ::= <simple_type_spec> <any_declarators>
//                      | <template_type_spec> <any_declarators>
//                      | <constr_type_dcl> <any_declarators>
//   <any_declarators> ::= <any_declarator>
//                      | <any_declarators> "," <any_declarator>
//   <any_declarator>  ::= <simple_declarator> | <array_declarator>
//   <array_declarator> ::= <identifier> <fixed_array_sizes>
//   <fixed_array_sizes> ::= <fixed_array_size>
//                        | <fixed_array_sizes> <fixed_array_size>
//   <fixed_array_size>  ::= "[" <positive_int_const> "]"

pub const PROD_TYPEDEF_DCL: Production = Production {
    id: ID_TYPEDEF_DCL,
    name: "typedef_dcl",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("typedef")),
        Symbol::Nonterminal(ID_TYPE_DECLARATOR),
    ])],
    ast_hint: None,
};

pub const PROD_TYPE_DECLARATOR: Production = Production {
    id: ID_TYPE_DECLARATOR,
    name: "type_declarator",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[
        named_alt(
            "simple",
            &[
                Symbol::Nonterminal(ID_SIMPLE_TYPE_SPEC),
                Symbol::Nonterminal(ID_ANY_DECLARATORS),
            ],
        ),
        named_alt(
            "template",
            &[
                Symbol::Nonterminal(ID_TEMPLATE_TYPE_SPEC),
                Symbol::Nonterminal(ID_ANY_DECLARATORS),
            ],
        ),
        // Spec §7.4.1.3 Rule (64): <type_declarator> ::= { <simple_type_spec>
        // | <template_type_spec> | <constr_type_dcl> } <any_declarators>.
        // Beispiel: typedef struct { long x; } S;
        named_alt(
            "constr",
            &[
                Symbol::Nonterminal(ID_CONSTR_TYPE_DCL),
                Symbol::Nonterminal(ID_ANY_DECLARATORS),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANY_DECLARATORS: Production = Production {
    id: ID_ANY_DECLARATORS,
    name: "any_declarators",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_ANY_DECLARATOR)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_ANY_DECLARATORS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_ANY_DECLARATOR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANY_DECLARATOR: Production = Production {
    id: ID_ANY_DECLARATOR,
    name: "any_declarator",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[
        named_alt("simple", &[Symbol::Nonterminal(ID_SIMPLE_DECLARATOR)]),
        named_alt("array", &[Symbol::Nonterminal(ID_ARRAY_DECLARATOR)]),
    ],
    ast_hint: None,
};

pub const PROD_ARRAY_DECLARATOR: Production = Production {
    id: ID_ARRAY_DECLARATOR,
    name: "array_declarator",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Nonterminal(ID_FIXED_ARRAY_SIZES),
    ])],
    ast_hint: None,
};

pub const PROD_FIXED_ARRAY_SIZES: Production = Production {
    id: ID_FIXED_ARRAY_SIZES,
    name: "fixed_array_sizes",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_FIXED_ARRAY_SIZE)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_FIXED_ARRAY_SIZES),
                Symbol::Nonterminal(ID_FIXED_ARRAY_SIZE),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_FIXED_ARRAY_SIZE: Production = Production {
    id: ID_FIXED_ARRAY_SIZE,
    name: "fixed_array_size",
    spec_ref: spec("7.4.1.4.4.6"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct("[")),
        Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
        Symbol::Terminal(TokenKind::Punct("]")),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.1.4.4.7 Except Decl (T3.10)
// ============================================================================
// BNF §7.4.1.4.4.7:
//   <except_dcl> ::= "exception" <identifier> "{" <member_list> "}"
// Identische Struktur wie struct_def, aber mit "exception"-Keyword.

pub const PROD_EXCEPT_DCL: Production = Production {
    id: ID_EXCEPT_DCL,
    name: "except_dcl",
    spec_ref: spec("7.4.1.4.4.7"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("exception")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_MEMBER_LIST),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.2 Interface (T-LIM-5)
// ============================================================================
// BNF §7.4.2:
//   <interface_dcl> ::= <interface_def> | <interface_forward_dcl>
//   <interface_def> ::= <interface_header> "{" <interface_body> "}"
//   <interface_header> ::= [<interface_kind>] "interface" <identifier>
//                          [<interface_inheritance_spec>]
//   <interface_kind> ::= "abstract" | "local"
//   <interface_inheritance_spec> ::= ":" <interface_name_list>
//   <interface_name_list> ::= <scoped_name>
//                          | <interface_name_list> "," <scoped_name>
//   <interface_body> ::= <export_list>?
//   <export_list> ::= <export>
//                  | <export_list> <export>
//   <export> ::= <op_dcl> ";" | <attr_dcl> ";"
//             | <type_dcl> ";" | <const_dcl> ";" | <except_dcl> ";"
//   <op_dcl> ::= ["oneway"] <op_type_spec> <identifier>
//                <parameter_dcls> [<raises_expr>]
//   <op_type_spec> ::= <type_spec> | "void"
//   <parameter_dcls> ::= "(" [<param_dcl_list>] ")"
//   <param_dcl_list> ::= <param_dcl>
//                     | <param_dcl_list> "," <param_dcl>
//   <param_dcl> ::= <param_attribute> <type_spec> <simple_declarator>
//   <param_attribute> ::= "in" | "out" | "inout"
//   <raises_expr> ::= "raises" "(" <scoped_name_list> ")"
//   <attr_dcl> ::= ["readonly"] "attribute" <type_spec> <identifier>

pub const PROD_INTERFACE_DCL: Production = Production {
    id: ID_INTERFACE_DCL,
    name: "interface_dcl",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("def", &[Symbol::Nonterminal(ID_INTERFACE_DEF)]),
        named_alt("forward", &[Symbol::Nonterminal(ID_INTERFACE_FORWARD_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_DEF: Production = Production {
    id: ID_INTERFACE_DEF,
    name: "interface_def",
    spec_ref: spec("7.4.2"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_INTERFACE_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_INTERFACE_BODY),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_INTERFACE_FORWARD_DCL: Production = Production {
    id: ID_INTERFACE_FORWARD_DCL,
    name: "interface_forward_dcl",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "with_kind",
            &[
                Symbol::Nonterminal(ID_INTERFACE_KIND),
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_HEADER: Production = Production {
    id: ID_INTERFACE_HEADER,
    name: "interface_header",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "kind_inherit",
            &[
                Symbol::Nonterminal(ID_INTERFACE_KIND),
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_INTERFACE_INHERITANCE_SPEC),
            ],
        ),
        named_alt(
            "kind_only",
            &[
                Symbol::Nonterminal(ID_INTERFACE_KIND),
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "inherit_only",
            &[
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_INTERFACE_INHERITANCE_SPEC),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_KIND: Production = Production {
    id: ID_INTERFACE_KIND,
    name: "interface_kind",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "abstract",
            &[Symbol::Terminal(TokenKind::Keyword("abstract"))],
        ),
        named_alt("local", &[Symbol::Terminal(TokenKind::Keyword("local"))]),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_INHERITANCE_SPEC: Production = Production {
    id: ID_INTERFACE_INHERITANCE_SPEC,
    name: "interface_inheritance_spec",
    spec_ref: spec("7.4.2"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct(":")),
        Symbol::Nonterminal(ID_INTERFACE_NAME_LIST),
    ])],
    ast_hint: None,
};

pub const PROD_INTERFACE_NAME_LIST: Production = Production {
    id: ID_INTERFACE_NAME_LIST,
    name: "interface_name_list",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_INTERFACE_NAME_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_BODY: Production = Production {
    id: ID_INTERFACE_BODY,
    name: "interface_body",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        Alternative {
            name: Some("empty"),
            symbols: &[],
            note: None,
        },
        named_alt("with_exports", &[Symbol::Nonterminal(ID_EXPORT_LIST)]),
    ],
    ast_hint: None,
};

pub const PROD_EXPORT_LIST: Production = Production {
    id: ID_EXPORT_LIST,
    name: "export_list",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_EXPORT)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_EXPORT_LIST),
                Symbol::Nonterminal(ID_EXPORT),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_EXPORT: Production = Production {
    id: ID_EXPORT,
    name: "export",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "op",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_OP_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "attr",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_ATTR_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "type",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_TYPE_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "const",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_CONST_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "except",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_EXCEPT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // Spec §7.4.6.3 Rule (112): <export> ::+ <type_id_dcl> ";"
        // | <type_prefix_dcl> ";" | <import_dcl> ";".
        named_alt(
            "type_id",
            &[
                Symbol::Nonterminal(ID_TYPE_ID_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "type_prefix",
            &[
                Symbol::Nonterminal(ID_TYPE_PREFIX_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "import",
            &[
                Symbol::Nonterminal(ID_IMPORT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // Rule (123): <op_with_context> als Export-Variante.
        named_alt(
            "op_with_context",
            &[
                Symbol::Nonterminal(ID_OP_WITH_CONTEXT),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_OP_DCL: Production = Production {
    id: ID_OP_DCL,
    name: "op_dcl",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "oneway_with_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("oneway")),
                Symbol::Nonterminal(ID_OP_TYPE_SPEC),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_PARAMETER_DCLS),
                Symbol::Nonterminal(ID_RAISES_EXPR),
            ],
        ),
        named_alt(
            "oneway_no_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("oneway")),
                Symbol::Nonterminal(ID_OP_TYPE_SPEC),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_PARAMETER_DCLS),
            ],
        ),
        named_alt(
            "with_raises",
            &[
                Symbol::Nonterminal(ID_OP_TYPE_SPEC),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_PARAMETER_DCLS),
                Symbol::Nonterminal(ID_RAISES_EXPR),
            ],
        ),
        named_alt(
            "no_raises",
            &[
                Symbol::Nonterminal(ID_OP_TYPE_SPEC),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Nonterminal(ID_PARAMETER_DCLS),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_OP_TYPE_SPEC: Production = Production {
    id: ID_OP_TYPE_SPEC,
    name: "op_type_spec",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("type", &[Symbol::Nonterminal(ID_TYPE_SPEC)]),
        named_alt("void", &[Symbol::Terminal(TokenKind::Keyword("void"))]),
    ],
    ast_hint: None,
};

pub const PROD_PARAMETER_DCLS: Production = Production {
    id: ID_PARAMETER_DCLS,
    name: "parameter_dcls",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt(
            "empty",
            &[
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
        named_alt(
            "with_params",
            &[
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Nonterminal(ID_PARAM_DCL_LIST),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PARAM_DCL_LIST: Production = Production {
    id: ID_PARAM_DCL_LIST,
    name: "param_dcl_list",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_PARAM_DCL)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_PARAM_DCL_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_PARAM_DCL),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PARAM_DCL: Production = Production {
    id: ID_PARAM_DCL,
    name: "param_dcl",
    spec_ref: spec("7.4.2"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
        Symbol::Nonterminal(ID_PARAM_ATTRIBUTE),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
    ])],
    ast_hint: None,
};

pub const PROD_PARAM_ATTRIBUTE: Production = Production {
    id: ID_PARAM_ATTRIBUTE,
    name: "param_attribute",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("in", &[Symbol::Terminal(TokenKind::Keyword("in"))]),
        named_alt("out", &[Symbol::Terminal(TokenKind::Keyword("out"))]),
        named_alt("inout", &[Symbol::Terminal(TokenKind::Keyword("inout"))]),
    ],
    ast_hint: None,
};

pub const PROD_RAISES_EXPR: Production = Production {
    id: ID_RAISES_EXPR,
    name: "raises_expr",
    spec_ref: spec("7.4.2"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("raises")),
        Symbol::Terminal(TokenKind::Punct("(")),
        Symbol::Nonterminal(ID_SCOPED_NAME_LIST),
        Symbol::Terminal(TokenKind::Punct(")")),
    ])],
    ast_hint: None,
};

pub const PROD_SCOPED_NAME_LIST: Production = Production {
    id: ID_SCOPED_NAME_LIST,
    name: "scoped_name_list",
    spec_ref: spec("7.4.2"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_SCOPED_NAME_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
            ],
        ),
    ],
    ast_hint: None,
};

// §7.4.3.1 Rules 89-97 — Attribute Declarations.
//
// Rule 89: <attr_dcl> ::= <readonly_attr_spec> | <attr_spec>
// Rule 90: <readonly_attr_spec> ::= "readonly" "attribute" <type_spec>
//                                     <readonly_attr_declarator>
// Rule 91: <readonly_attr_declarator> ::=
//            <simple_declarator> <raises_expr>
//          | <simple_declarator> { "," <simple_declarator> }*
// Rule 92: <attr_spec> ::= "attribute" <type_spec> <attr_declarator>
// Rule 93: <attr_declarator> ::=
//            <simple_declarator> <attr_raises_expr>
//          | <simple_declarator> { "," <simple_declarator> }*
// Rule 94: <attr_raises_expr> ::=
//            <get_excep_expr> [ <set_excep_expr> ]
//          | <set_excep_expr>
// Rule 95: <get_excep_expr> ::= "getraises" <exception_list>
// Rule 96: <set_excep_expr> ::= "setraises" <exception_list>
// Rule 97: <exception_list> ::= "(" <scoped_name> { "," <scoped_name> }* ")"
//
// Anmerkung: <simple_declarator> ist hier `identifier` (Rule 41 in §7.4.1).

pub const PROD_ATTR_DCL: Production = Production {
    id: ID_ATTR_DCL,
    name: "attr_dcl",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[
        named_alt("readonly", &[Symbol::Nonterminal(ID_READONLY_ATTR_SPEC)]),
        named_alt("writable", &[Symbol::Nonterminal(ID_ATTR_SPEC)]),
    ],
    ast_hint: None,
};

pub const PROD_READONLY_ATTR_SPEC: Production = Production {
    id: ID_READONLY_ATTR_SPEC,
    name: "readonly_attr_spec",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("readonly")),
        Symbol::Terminal(TokenKind::Keyword("attribute")),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_READONLY_ATTR_DECLARATOR),
    ])],
    ast_hint: None,
};

pub const PROD_READONLY_ATTR_DECLARATOR: Production = Production {
    id: ID_READONLY_ATTR_DECLARATOR,
    name: "readonly_attr_declarator",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[
        named_alt(
            "with_raises",
            &[
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Nonterminal(ID_RAISES_EXPR),
            ],
        ),
        named_alt(
            "list",
            &[
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[
                        Symbol::Terminal(TokenKind::Punct(",")),
                        Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                    ],
                ),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ATTR_SPEC: Production = Production {
    id: ID_ATTR_SPEC,
    name: "attr_spec",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("attribute")),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_ATTR_DECLARATOR),
    ])],
    ast_hint: None,
};

pub const PROD_ATTR_DECLARATOR: Production = Production {
    id: ID_ATTR_DECLARATOR,
    name: "attr_declarator",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[
        named_alt(
            "with_raises",
            &[
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Nonterminal(ID_ATTR_RAISES_EXPR),
            ],
        ),
        named_alt(
            "list",
            &[
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[
                        Symbol::Terminal(TokenKind::Punct(",")),
                        Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                    ],
                ),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ATTR_RAISES_EXPR: Production = Production {
    id: ID_ATTR_RAISES_EXPR,
    name: "attr_raises_expr",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[
        named_alt(
            "get_then_set",
            &[
                Symbol::Nonterminal(ID_GET_EXCEP_EXPR),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_SET_EXCEP_EXPR)],
                ),
            ],
        ),
        named_alt("set_only", &[Symbol::Nonterminal(ID_SET_EXCEP_EXPR)]),
    ],
    ast_hint: None,
};

pub const PROD_GET_EXCEP_EXPR: Production = Production {
    id: ID_GET_EXCEP_EXPR,
    name: "get_excep_expr",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("getraises")),
        Symbol::Nonterminal(ID_EXCEPTION_LIST),
    ])],
    ast_hint: None,
};

pub const PROD_SET_EXCEP_EXPR: Production = Production {
    id: ID_SET_EXCEP_EXPR,
    name: "set_excep_expr",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("setraises")),
        Symbol::Nonterminal(ID_EXCEPTION_LIST),
    ])],
    ast_hint: None,
};

pub const PROD_EXCEPTION_LIST: Production = Production {
    id: ID_EXCEPTION_LIST,
    name: "exception_list",
    spec_ref: spec("7.4.3.1"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct("(")),
        Symbol::Nonterminal(ID_SCOPED_NAME_LIST),
        Symbol::Terminal(TokenKind::Punct(")")),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.15 Rules 218-223 — User-Defined Annotation Declarations
// ============================================================================
//
// Rule 218: <annotation_dcl> ::= <annotation_header> "{" <annotation_body> "}"
// Rule 219: <annotation_header> ::= "@annotation" <identifier>
// Rule 220: <annotation_body> ::= { (<annotation_member>
//                                    | <enum_dcl>";"
//                                    | <const_dcl>";"
//                                    | <typedef_dcl>";") }*
// Rule 221: <annotation_member> ::= <const_type> <simple_declarator>
//                                   [ "default" <const_expr> ] ";"
// Rules 222/223: reserved (n/a).
//
// Hinweis: `annotation` wird Keyword (Tab 7-14 reserved); der Lexer leitet
// das automatisch aus der Production ab.

pub const PROD_ANNOTATION_DCL: Production = Production {
    id: ID_ANNOTATION_DCL,
    name: "annotation_dcl",
    spec_ref: spec("7.4.15"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_ANNOTATION_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_ANNOTATION_BODY),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_ANNOTATION_HEADER: Production = Production {
    id: ID_ANNOTATION_HEADER,
    name: "annotation_header",
    spec_ref: spec("7.4.15"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct("@")),
        Symbol::Terminal(TokenKind::Keyword("annotation")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_ANNOTATION_BODY: Production = Production {
    id: ID_ANNOTATION_BODY,
    name: "annotation_body",
    spec_ref: spec("7.4.15"),
    alternatives: &[alt(&[Symbol::Repeat(
        RepeatKind::ZeroOrMore,
        &[Symbol::Nonterminal(ID_ANNOTATION_BODY_MEMBER)],
    )])],
    ast_hint: None,
};

pub const PROD_ANNOTATION_BODY_MEMBER: Production = Production {
    id: ID_ANNOTATION_BODY_MEMBER,
    name: "annotation_body_member",
    spec_ref: spec("7.4.15"),
    alternatives: &[
        // <annotation_member> endet selbst mit ";" (Rule 221).
        named_alt("member", &[Symbol::Nonterminal(ID_ANNOTATION_MEMBER)]),
        named_alt(
            "enum",
            &[
                Symbol::Nonterminal(ID_ENUM_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "const",
            &[
                Symbol::Nonterminal(ID_CONST_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "typedef",
            &[
                Symbol::Nonterminal(ID_TYPEDEF_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANNOTATION_MEMBER: Production = Production {
    id: ID_ANNOTATION_MEMBER,
    name: "annotation_member",
    spec_ref: spec("7.4.15"),
    alternatives: &[
        named_alt(
            "with_default",
            &[
                Symbol::Nonterminal(ID_CONST_TYPE),
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Terminal(TokenKind::Keyword("default")),
                Symbol::Nonterminal(ID_CONST_EXPR),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "no_default",
            &[
                Symbol::Nonterminal(ID_CONST_TYPE),
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.3 Valuetype (T-LIM-5, minimal)
// ============================================================================
// Vollstaendige <value_dcl>-Spec ist umfangreich (custom, abstract,
// truncatable, init, factory). Spike-Scope: nur <value_box_dcl> und
// <value_forward_dcl>, weil das das ist, was DDS-IDLs in der Praxis
// nutzen.
//   <value_box_dcl>     ::= "valuetype" <identifier> <type_spec>
//   <value_forward_dcl> ::= "valuetype" <identifier>

pub const PROD_VALUE_BOX_DCL: Production = Production {
    id: ID_VALUE_BOX_DCL,
    name: "value_box_dcl",
    spec_ref: spec("7.4.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("valuetype")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Nonterminal(ID_TYPE_SPEC),
    ])],
    ast_hint: None,
};

pub const PROD_VALUE_FORWARD_DCL: Production = Production {
    id: ID_VALUE_FORWARD_DCL,
    name: "value_forward_dcl",
    spec_ref: spec("7.4.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("valuetype")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.5 Rules 100-109 — Value Types Full (CORBA-Profil)
// ============================================================================
//
// Rule 100: <value_dcl> ::= <value_def> | <value_forward_dcl>
//                         | <value_box_dcl> | <value_abs_def>
// Rule 101: <value_def> ::= <value_header> "{" <value_element>* "}"
// Rule 102: <value_header> ::= ["custom"] "valuetype" <identifier>
//                              [<value_inheritance_spec>]
// Rule 103: <value_inheritance_spec> ::=
//             [":" ["truncatable"] <value_name> { "," <value_name> }*]
//             ["supports" <interface_name> { "," <interface_name> }*]
// Rule 104: <value_name> ::= <scoped_name>
// Rule 105: <value_element> ::= <export> | <state_member> | <init_dcl>
// Rule 106: <state_member> ::= ("public" | "private")
//                              <type_spec> <declarators> ";"
// Rule 107: <init_dcl> ::= "factory" <identifier>
//                          "(" [<init_param_decls>] ")"
//                          [<raises_expr>] ";"
// Rule 108: <init_param_decls> ::= <init_param_decl>
//                                  { "," <init_param_decl> }*
// Rule 109: <init_param_decl> ::= "in" <type_spec> <simple_declarator>

pub const PROD_VALUE_DEF: Production = Production {
    id: ID_VALUE_DEF,
    name: "value_def",
    spec_ref: spec("7.4.5"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_VALUE_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Repeat(
            RepeatKind::ZeroOrMore,
            &[Symbol::Nonterminal(ID_VALUE_ELEMENT)],
        ),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_VALUE_HEADER: Production = Production {
    id: ID_VALUE_HEADER,
    name: "value_header",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        // Alt 0: `custom valuetype` (Rule 102, custom-Variante)
        named_alt(
            "custom",
            &[
                Symbol::Terminal(TokenKind::Keyword("custom")),
                Symbol::Terminal(TokenKind::Keyword("valuetype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
        // Alt 1: `valuetype` (plain).
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("valuetype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
        // Alt 2: `abstract valuetype` (Rule 125 — value_abs_def-Header).
        named_alt(
            "abstract",
            &[
                Symbol::Terminal(TokenKind::Keyword("abstract")),
                Symbol::Terminal(TokenKind::Keyword("valuetype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_VALUE_INHERITANCE_SPEC: Production = Production {
    id: ID_VALUE_INHERITANCE_SPEC,
    name: "value_inheritance_spec",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt(
            "extends_truncatable",
            &[
                Symbol::Terminal(TokenKind::Punct(":")),
                Symbol::Terminal(TokenKind::Keyword("truncatable")),
                Symbol::Nonterminal(ID_VALUE_INHERITANCE_NAME_LIST),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[
                        Symbol::Terminal(TokenKind::Keyword("supports")),
                        Symbol::Nonterminal(ID_INTERFACE_NAME_LIST),
                    ],
                ),
            ],
        ),
        named_alt(
            "extends",
            &[
                Symbol::Terminal(TokenKind::Punct(":")),
                Symbol::Nonterminal(ID_VALUE_INHERITANCE_NAME_LIST),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[
                        Symbol::Terminal(TokenKind::Keyword("supports")),
                        Symbol::Nonterminal(ID_INTERFACE_NAME_LIST),
                    ],
                ),
            ],
        ),
        named_alt(
            "supports_only",
            &[
                Symbol::Terminal(TokenKind::Keyword("supports")),
                Symbol::Nonterminal(ID_INTERFACE_NAME_LIST),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_VALUE_INHERITANCE_NAME_LIST: Production = Production {
    id: ID_VALUE_INHERITANCE_NAME_LIST,
    name: "value_inheritance_name_list",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_VALUE_INHERITANCE_NAME_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_VALUE_ELEMENT: Production = Production {
    id: ID_VALUE_ELEMENT,
    name: "value_element",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt("export", &[Symbol::Nonterminal(ID_EXPORT)]),
        named_alt("state_member", &[Symbol::Nonterminal(ID_STATE_MEMBER)]),
        named_alt("init", &[Symbol::Nonterminal(ID_INIT_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_STATE_MEMBER: Production = Production {
    id: ID_STATE_MEMBER,
    name: "state_member",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt(
            "public",
            &[
                Symbol::Terminal(TokenKind::Keyword("public")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Nonterminal(ID_DECLARATORS),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "private",
            &[
                Symbol::Terminal(TokenKind::Keyword("private")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Nonterminal(ID_DECLARATORS),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INIT_DCL: Production = Production {
    id: ID_INIT_DCL,
    name: "init_dcl",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt(
            "with_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("factory")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_INIT_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
                Symbol::Nonterminal(ID_RAISES_EXPR),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "no_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("factory")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_INIT_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INIT_PARAM_DCLS: Production = Production {
    id: ID_INIT_PARAM_DCLS,
    name: "init_param_dcls",
    spec_ref: spec("7.4.5"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_INIT_PARAM_DCL)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_INIT_PARAM_DCLS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_INIT_PARAM_DCL),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INIT_PARAM_DCL: Production = Production {
    id: ID_INIT_PARAM_DCL,
    name: "init_param_dcl",
    spec_ref: spec("7.4.5"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("in")),
        Symbol::Nonterminal(ID_TYPE_SPEC),
        Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
    ])],
    ast_hint: None,
};

// ============================================================================
// §7.4.6 Rules 111-117 — Repository-IDs / Typeprefix / Import (CORBA)
// ============================================================================
//
// Rule 111: <type_id_dcl> ::= "typeid" <scoped_name> <string_literal>
// Rule 112: <type_prefix_dcl> ::= "typeprefix" <scoped_name> <string_literal>
// Rule 113-114: (reserved variants in older Specs)
// Rule 115: <import_dcl> ::= "import" <imported_scope>
// Rule 116: <imported_scope> ::= <scoped_name> | <string_literal>
// Rule 117: (reserved)

pub const PROD_TYPE_ID_DCL: Production = Production {
    id: ID_TYPE_ID_DCL,
    name: "type_id_dcl",
    spec_ref: spec("7.4.6"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("typeid")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Nonterminal(ID_STRING_LITERAL),
    ])],
    ast_hint: None,
};

pub const PROD_TYPE_PREFIX_DCL: Production = Production {
    id: ID_TYPE_PREFIX_DCL,
    name: "type_prefix_dcl",
    spec_ref: spec("7.4.6"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("typeprefix")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Nonterminal(ID_STRING_LITERAL),
    ])],
    ast_hint: None,
};

pub const PROD_IMPORT_DCL: Production = Production {
    id: ID_IMPORT_DCL,
    name: "import_dcl",
    spec_ref: spec("7.4.6"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("import")),
        Symbol::Nonterminal(ID_IMPORTED_SCOPE),
    ])],
    ast_hint: None,
};

pub const PROD_IMPORTED_SCOPE: Production = Production {
    id: ID_IMPORTED_SCOPE,
    name: "imported_scope",
    spec_ref: spec("7.4.6"),
    alternatives: &[
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt("string", &[Symbol::Nonterminal(ID_STRING_LITERAL)]),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.9 Components / Homes / Ports / Eventtypes (CCM, Rules 133-183)
// ============================================================================

pub const PROD_COMPONENT_DCL: Production = Production {
    id: ID_COMPONENT_DCL,
    name: "component_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt("def", &[Symbol::Nonterminal(ID_COMPONENT_DEF)]),
        named_alt("forward", &[Symbol::Nonterminal(ID_COMPONENT_FORWARD_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_COMPONENT_FORWARD_DCL: Production = Production {
    id: ID_COMPONENT_FORWARD_DCL,
    name: "component_forward_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("component")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_COMPONENT_DEF: Production = Production {
    id: ID_COMPONENT_DEF,
    name: "component_def",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_COMPONENT_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_COMPONENT_BODY),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_COMPONENT_HEADER: Production = Production {
    id: ID_COMPONENT_HEADER,
    name: "component_header",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("component")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_COMPONENT_INHERITANCE_SPEC)],
        ),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_SUPPORTED_INTERFACE_SPEC)],
        ),
    ])],
    ast_hint: None,
};

pub const PROD_COMPONENT_INHERITANCE_SPEC: Production = Production {
    id: ID_COMPONENT_INHERITANCE_SPEC,
    name: "component_inheritance_spec",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct(":")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
    ])],
    ast_hint: None,
};

pub const PROD_SUPPORTED_INTERFACE_SPEC: Production = Production {
    id: ID_SUPPORTED_INTERFACE_SPEC,
    name: "supported_interface_spec",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("supports")),
        Symbol::Nonterminal(ID_SCOPED_NAME_LIST),
    ])],
    ast_hint: None,
};

pub const PROD_COMPONENT_BODY: Production = Production {
    id: ID_COMPONENT_BODY,
    name: "component_body",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[Symbol::Repeat(
        RepeatKind::ZeroOrMore,
        &[Symbol::Nonterminal(ID_COMPONENT_EXPORT)],
    )])],
    ast_hint: None,
};

pub const PROD_COMPONENT_EXPORT: Production = Production {
    id: ID_COMPONENT_EXPORT,
    name: "component_export",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "provides",
            &[
                Symbol::Nonterminal(ID_PROVIDES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "uses",
            &[
                Symbol::Nonterminal(ID_USES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "attr",
            &[
                Symbol::Nonterminal(ID_ATTR_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "emits",
            &[
                Symbol::Nonterminal(ID_EMITS_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "publishes",
            &[
                Symbol::Nonterminal(ID_PUBLISHES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "consumes",
            &[
                Symbol::Nonterminal(ID_CONSUMES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "port",
            &[
                Symbol::Nonterminal(ID_PORT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PROVIDES_DCL: Production = Production {
    id: ID_PROVIDES_DCL,
    name: "provides_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("provides")),
        Symbol::Nonterminal(ID_INTERFACE_TYPE),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_USES_DCL: Production = Production {
    id: ID_USES_DCL,
    name: "uses_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "multiple",
            &[
                Symbol::Terminal(TokenKind::Keyword("uses")),
                Symbol::Terminal(TokenKind::Keyword("multiple")),
                Symbol::Nonterminal(ID_INTERFACE_TYPE),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "single",
            &[
                Symbol::Terminal(TokenKind::Keyword("uses")),
                Symbol::Nonterminal(ID_INTERFACE_TYPE),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_INTERFACE_TYPE: Production = Production {
    id: ID_INTERFACE_TYPE,
    name: "interface_type",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt("scoped", &[Symbol::Nonterminal(ID_SCOPED_NAME)]),
        named_alt("object", &[Symbol::Terminal(TokenKind::Keyword("Object"))]),
    ],
    ast_hint: None,
};

pub const PROD_HOME_DCL: Production = Production {
    id: ID_HOME_DCL,
    name: "home_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_HOME_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_HOME_BODY),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_HOME_HEADER: Production = Production {
    id: ID_HOME_HEADER,
    name: "home_header",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("home")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_HOME_INHERITANCE_SPEC)],
        ),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_SUPPORTED_INTERFACE_SPEC)],
        ),
        Symbol::Terminal(TokenKind::Keyword("manages")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_PRIMARY_KEY_SPEC)],
        ),
    ])],
    ast_hint: None,
};

pub const PROD_HOME_INHERITANCE_SPEC: Production = Production {
    id: ID_HOME_INHERITANCE_SPEC,
    name: "home_inheritance_spec",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct(":")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
    ])],
    ast_hint: None,
};

pub const PROD_PRIMARY_KEY_SPEC: Production = Production {
    id: ID_PRIMARY_KEY_SPEC,
    name: "primary_key_spec",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("primarykey")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
    ])],
    ast_hint: None,
};

pub const PROD_HOME_BODY: Production = Production {
    id: ID_HOME_BODY,
    name: "home_body",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[Symbol::Repeat(
        RepeatKind::ZeroOrMore,
        &[Symbol::Nonterminal(ID_HOME_EXPORT)],
    )])],
    ast_hint: None,
};

pub const PROD_HOME_EXPORT: Production = Production {
    id: ID_HOME_EXPORT,
    name: "home_export",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "factory",
            &[
                Symbol::Nonterminal(ID_FACTORY_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "finder",
            &[
                Symbol::Nonterminal(ID_FINDER_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt("export", &[Symbol::Nonterminal(ID_EXPORT)]),
    ],
    ast_hint: None,
};

pub const PROD_FACTORY_DCL: Production = Production {
    id: ID_FACTORY_DCL,
    name: "home_factory_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "with_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("factory")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_FACTORY_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
                Symbol::Nonterminal(ID_RAISES_EXPR),
            ],
        ),
        named_alt(
            "no_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("factory")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_FACTORY_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_FACTORY_PARAM_DCLS: Production = Production {
    id: ID_FACTORY_PARAM_DCLS,
    name: "factory_param_dcls",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "single",
            &[
                Symbol::Terminal(TokenKind::Keyword("in")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
            ],
        ),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_FACTORY_PARAM_DCLS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Terminal(TokenKind::Keyword("in")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Nonterminal(ID_SIMPLE_DECLARATOR),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_FINDER_DCL: Production = Production {
    id: ID_FINDER_DCL,
    name: "finder_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "with_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("finder")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_FACTORY_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
                Symbol::Nonterminal(ID_RAISES_EXPR),
            ],
        ),
        named_alt(
            "no_raises",
            &[
                Symbol::Terminal(TokenKind::Keyword("finder")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_FACTORY_PARAM_DCLS)],
                ),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_EVENT_DCL: Production = Production {
    id: ID_EVENT_DCL,
    name: "event_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt("def", &[Symbol::Nonterminal(ID_EVENT_DEF)]),
        named_alt("forward", &[Symbol::Nonterminal(ID_EVENT_FORWARD_DCL)]),
    ],
    ast_hint: None,
};

pub const PROD_EVENT_FORWARD_DCL: Production = Production {
    id: ID_EVENT_FORWARD_DCL,
    name: "event_forward_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "abstract",
            &[
                Symbol::Terminal(TokenKind::Keyword("abstract")),
                Symbol::Terminal(TokenKind::Keyword("eventtype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("eventtype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_EVENT_DEF: Production = Production {
    id: ID_EVENT_DEF,
    name: "event_def",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_EVENT_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Repeat(
            RepeatKind::ZeroOrMore,
            &[Symbol::Nonterminal(ID_VALUE_ELEMENT)],
        ),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_EVENT_HEADER: Production = Production {
    id: ID_EVENT_HEADER,
    name: "event_header",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "custom",
            &[
                Symbol::Terminal(TokenKind::Keyword("custom")),
                Symbol::Terminal(TokenKind::Keyword("eventtype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
        named_alt(
            "abstract",
            &[
                Symbol::Terminal(TokenKind::Keyword("abstract")),
                Symbol::Terminal(TokenKind::Keyword("eventtype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("eventtype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Nonterminal(ID_VALUE_INHERITANCE_SPEC)],
                ),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_EMITS_DCL: Production = Production {
    id: ID_EMITS_DCL,
    name: "emits_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("emits")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_PUBLISHES_DCL: Production = Production {
    id: ID_PUBLISHES_DCL,
    name: "publishes_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("publishes")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_CONSUMES_DCL: Production = Production {
    id: ID_CONSUMES_DCL,
    name: "consumes_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("consumes")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_PORTTYPE_DCL: Production = Production {
    id: ID_PORTTYPE_DCL,
    name: "porttype_dcl",
    spec_ref: spec("7.4.11.3"),
    alternatives: &[
        // Spec §7.4.11.3 Rule (172): <porttype_def>
        named_alt(
            "porttype_def",
            &[
                Symbol::Terminal(TokenKind::Keyword("porttype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Nonterminal(ID_PORT_BODY),
                Symbol::Terminal(TokenKind::Punct("}")),
            ],
        ),
        // Spec §7.4.11.3 Rule (173): <porttype_forward_dcl> ::=
        // "porttype" <identifier>.
        named_alt(
            "porttype_forward",
            &[
                Symbol::Terminal(TokenKind::Keyword("porttype")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PORT_BODY: Production = Production {
    id: ID_PORT_BODY,
    name: "port_body",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_PORT_REF),
        Symbol::Repeat(
            RepeatKind::ZeroOrMore,
            &[Symbol::Nonterminal(ID_PORT_EXPORT)],
        ),
    ])],
    ast_hint: None,
};

pub const PROD_PORT_REF: Production = Production {
    id: ID_PORT_REF,
    name: "port_ref",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "provides",
            &[
                Symbol::Nonterminal(ID_PROVIDES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "uses",
            &[
                Symbol::Nonterminal(ID_USES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "port",
            &[
                Symbol::Nonterminal(ID_PORT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PORT_EXPORT: Production = Production {
    id: ID_PORT_EXPORT,
    name: "port_export",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt("ref", &[Symbol::Nonterminal(ID_PORT_REF)]),
        named_alt(
            "attr",
            &[
                Symbol::Nonterminal(ID_ATTR_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_PORT_DCL: Production = Production {
    id: ID_PORT_DCL,
    name: "port_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "port",
            &[
                Symbol::Terminal(TokenKind::Keyword("port")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "mirrorport",
            &[
                Symbol::Terminal(TokenKind::Keyword("mirrorport")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_CONNECTOR_DCL: Production = Production {
    id: ID_CONNECTOR_DCL,
    name: "connector_dcl",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_CONNECTOR_HEADER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Repeat(
            RepeatKind::ZeroOrMore,
            &[Symbol::Nonterminal(ID_CONNECTOR_EXPORT)],
        ),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_CONNECTOR_HEADER: Production = Production {
    id: ID_CONNECTOR_HEADER,
    name: "connector_header",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("connector")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Repeat(
            RepeatKind::Optional,
            &[Symbol::Nonterminal(ID_CONNECTOR_INHERIT_SPEC)],
        ),
    ])],
    ast_hint: None,
};

pub const PROD_CONNECTOR_INHERIT_SPEC: Production = Production {
    id: ID_CONNECTOR_INHERIT_SPEC,
    name: "connector_inherit_spec",
    spec_ref: spec("7.4.9"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Punct(":")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
    ])],
    ast_hint: None,
};

pub const PROD_CONNECTOR_EXPORT: Production = Production {
    id: ID_CONNECTOR_EXPORT,
    name: "connector_export",
    spec_ref: spec("7.4.9"),
    alternatives: &[
        named_alt(
            "provides",
            &[
                Symbol::Nonterminal(ID_PROVIDES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "uses",
            &[
                Symbol::Nonterminal(ID_USES_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "port",
            &[
                Symbol::Nonterminal(ID_PORT_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        named_alt(
            "attr",
            &[
                Symbol::Nonterminal(ID_ATTR_DCL),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

// ============================================================================
// §7.4.12 Template Modules (CORBA, Rules 184-194)
// ============================================================================
//
// Rule 184: <template_module_dcl> ::= "module" <identifier>
//                                     "<" <formal_parameters> ">"
//                                     "{" <definition>* "}"
// Rule 185: <formal_parameters> ::= <formal_parameter>
//                                   { "," <formal_parameter> }*
// Rule 186: <formal_parameter> ::= <formal_parameter_type> <identifier>
// Rule 187: <formal_parameter_type> ::=
//             "typename" | "interface" | "valuetype" | "eventtype"
//           | "struct"   | "union"     | "exception" | "enum"
//           | "sequence" | "const" <const_type>
// Rule 188: <tpl_definition> ::= <definition> | <template_module_ref>
// Rule 189: <template_module_inst> ::= "module" <scoped_name>
//                                      "<" <actual_parameters> ">"
//                                      <identifier>
// Rule 190: <actual_parameters> ::= <actual_parameter>
//                                   { "," <actual_parameter> }*
// Rule 191: <actual_parameter> ::= <type_spec> | <const_expr>

pub const PROD_TEMPLATE_MODULE_DCL: Production = Production {
    id: ID_TEMPLATE_MODULE_DCL,
    name: "template_module_dcl",
    spec_ref: spec("7.4.12"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("module")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("<")),
        Symbol::Nonterminal(ID_FORMAL_PARAMETERS),
        Symbol::Terminal(TokenKind::Punct(">")),
        Symbol::Terminal(TokenKind::Punct("{")),
        // Spec §7.4.12 Rule 184 verlangt `<tpl_definition>+` im Body —
        // Rule 188 erweitert <definition> um <template_module_ref> ";"
        // (alias-Hook). ZeroOrMore ist Vendor-kompatible Lockerung
        // analog PROD_DEFINITION_LIST.
        Symbol::Repeat(
            RepeatKind::ZeroOrMore,
            &[Symbol::Nonterminal(ID_TPL_DEFINITION)],
        ),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

// Spec §7.4.12 Rule (188): <tpl_definition> ::= <definition>
//                                              | <template_module_ref> ";"
pub const PROD_TPL_DEFINITION: Production = Production {
    id: ID_TPL_DEFINITION,
    name: "tpl_definition",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt("definition", &[Symbol::Nonterminal(ID_DEFINITION)]),
        named_alt(
            "template_module_ref",
            &[
                Symbol::Nonterminal(ID_TEMPLATE_MODULE_REF),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_FORMAL_PARAMETERS: Production = Production {
    id: ID_FORMAL_PARAMETERS,
    name: "formal_parameters",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_FORMAL_PARAMETER)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_FORMAL_PARAMETERS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_FORMAL_PARAMETER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_FORMAL_PARAMETER: Production = Production {
    id: ID_FORMAL_PARAMETER,
    name: "formal_parameter",
    spec_ref: spec("7.4.12"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_FORMAL_PARAMETER_TYPE),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_FORMAL_PARAMETER_TYPE: Production = Production {
    id: ID_FORMAL_PARAMETER_TYPE,
    name: "formal_parameter_type",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt(
            "typename",
            &[Symbol::Terminal(TokenKind::Keyword("typename"))],
        ),
        named_alt(
            "interface",
            &[Symbol::Terminal(TokenKind::Keyword("interface"))],
        ),
        named_alt(
            "valuetype",
            &[Symbol::Terminal(TokenKind::Keyword("valuetype"))],
        ),
        named_alt(
            "eventtype",
            &[Symbol::Terminal(TokenKind::Keyword("eventtype"))],
        ),
        named_alt("struct", &[Symbol::Terminal(TokenKind::Keyword("struct"))]),
        named_alt("union", &[Symbol::Terminal(TokenKind::Keyword("union"))]),
        named_alt(
            "exception",
            &[Symbol::Terminal(TokenKind::Keyword("exception"))],
        ),
        named_alt("enum", &[Symbol::Terminal(TokenKind::Keyword("enum"))]),
        named_alt(
            "sequence",
            &[Symbol::Terminal(TokenKind::Keyword("sequence"))],
        ),
        named_alt(
            "const_typed",
            &[
                Symbol::Terminal(TokenKind::Keyword("const")),
                Symbol::Nonterminal(ID_CONST_TYPE),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_TEMPLATE_MODULE_INST: Production = Production {
    id: ID_TEMPLATE_MODULE_INST,
    name: "template_module_inst",
    spec_ref: spec("7.4.12"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("module")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Terminal(TokenKind::Punct("<")),
        Symbol::Nonterminal(ID_ACTUAL_PARAMETERS),
        Symbol::Terminal(TokenKind::Punct(">")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

pub const PROD_ACTUAL_PARAMETERS: Production = Production {
    id: ID_ACTUAL_PARAMETERS,
    name: "actual_parameters",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_ACTUAL_PARAMETER)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_ACTUAL_PARAMETERS),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_ACTUAL_PARAMETER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ACTUAL_PARAMETER: Production = Production {
    id: ID_ACTUAL_PARAMETER,
    name: "actual_parameter",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt("type", &[Symbol::Nonterminal(ID_TYPE_SPEC)]),
        named_alt("expr", &[Symbol::Nonterminal(ID_CONST_EXPR)]),
    ],
    ast_hint: None,
};

// Rule 193: <template_module_ref> ::= "alias" <scoped_name>
//                                     "<" <formal_parameter_names> ">"
//                                     <identifier>
pub const PROD_TEMPLATE_MODULE_REF: Production = Production {
    id: ID_TEMPLATE_MODULE_REF,
    name: "template_module_ref",
    spec_ref: spec("7.4.12"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("alias")),
        Symbol::Nonterminal(ID_SCOPED_NAME),
        Symbol::Terminal(TokenKind::Punct("<")),
        Symbol::Nonterminal(ID_FORMAL_PARAMETER_NAMES),
        Symbol::Terminal(TokenKind::Punct(">")),
        Symbol::Nonterminal(ID_IDENTIFIER),
    ])],
    ast_hint: None,
};

// Rule 194: <formal_parameter_names> ::= <identifier>
//                                        { "," <identifier> }*
pub const PROD_FORMAL_PARAMETER_NAMES: Production = Production {
    id: ID_FORMAL_PARAMETER_NAMES,
    name: "formal_parameter_names",
    spec_ref: spec("7.4.12"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_IDENTIFIER)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_FORMAL_PARAMETER_NAMES),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

// Rule 123 (§7.4.6 CORBA-Specific): <op_with_context> ::= {<op_dcl>
//                                                          | <op_oneway_dcl>}
//                                                          <context_expr>
pub const PROD_OP_WITH_CONTEXT: Production = Production {
    id: ID_OP_WITH_CONTEXT,
    name: "op_with_context",
    spec_ref: spec("7.4.6.3"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_OP_DCL),
        Symbol::Nonterminal(ID_CONTEXT_EXPR),
    ])],
    ast_hint: None,
};

// Rule 124: <context_expr> ::= "context" "(" <string_literal>
//                              { "," <string_literal> }* ")"
pub const PROD_CONTEXT_EXPR: Production = Production {
    id: ID_CONTEXT_EXPR,
    name: "context_expr",
    spec_ref: spec("7.4.6.3"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("context")),
        Symbol::Terminal(TokenKind::Punct("(")),
        Symbol::Nonterminal(ID_CONTEXT_STRING_LIST),
        Symbol::Terminal(TokenKind::Punct(")")),
    ])],
    ast_hint: None,
};

pub const PROD_CONTEXT_STRING_LIST: Production = Production {
    id: ID_CONTEXT_STRING_LIST,
    name: "context_string_list",
    spec_ref: spec("7.4.6.3"),
    alternatives: &[
        named_alt("single", &[Symbol::Terminal(TokenKind::StringLiteral)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_CONTEXT_STRING_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Terminal(TokenKind::StringLiteral),
            ],
        ),
    ],
    ast_hint: None,
};

// Rule 132 (§7.4.7 CORBA-Specific Value Types):
//   <value_base_type> ::= "ValueBase"
pub const PROD_VALUE_BASE_TYPE: Production = Production {
    id: ID_VALUE_BASE_TYPE,
    name: "value_base_type",
    spec_ref: spec("7.4.7.3"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("ValueBase"))])],
    ast_hint: None,
};

// ============================================================================
// §7.4.13 Extended Data Types BB — Map / Bitset / Bitmask / Any (T4.6)
// ============================================================================
// BNF aus IDL 4.2 §7.4.13:
//   <map_type>    ::= "map" "<" <type_spec> "," <type_spec>
//                       "," <positive_int_const> ">"
//                  | "map" "<" <type_spec> "," <type_spec> ">"
//   <bitset_dcl>  ::= "bitset" <identifier> [":" <scoped_name>]
//                       "{" <bitfield>* "}"
//   <bitfield>    ::= <bitfield_spec> <identifier>* ";"
//   <bitfield_spec> ::= "bitfield" "<" <positive_int_const>
//                         ["," <destination_type>] ">"
//   <bitmask_dcl> ::= "bitmask" <identifier>
//                       "{" <identifier> { "," <identifier> }* "}"
//   <any_type>    ::= "any"

pub const PROD_MAP_TYPE: Production = Production {
    id: ID_MAP_TYPE,
    name: "map_type",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        named_alt(
            "bounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("map")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
        named_alt(
            "unbounded",
            &[
                Symbol::Terminal(TokenKind::Keyword("map")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_BITSET_DCL: Production = Production {
    id: ID_BITSET_DCL,
    name: "bitset_dcl",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        named_alt(
            "with_base",
            &[
                Symbol::Terminal(TokenKind::Keyword("bitset")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct(":")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Nonterminal(ID_BITFIELD_LIST),
                Symbol::Terminal(TokenKind::Punct("}")),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("bitset")),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Nonterminal(ID_BITFIELD_LIST),
                Symbol::Terminal(TokenKind::Punct("}")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_BITFIELD_LIST: Production = Production {
    id: ID_BITFIELD_LIST,
    name: "bitfield_list",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        Alternative {
            name: Some("empty"),
            symbols: &[],
            note: None,
        },
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_BITFIELD_LIST),
                Symbol::Nonterminal(ID_BITFIELD),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_BITFIELD: Production = Production {
    id: ID_BITFIELD,
    name: "bitfield",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        // Mit optionalem Annotations-Prefix.
        named_alt(
            "named",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_BITFIELD_SPEC),
                Symbol::Nonterminal(ID_IDENTIFIER),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
        // Anonymous bitfield (Padding) — nur Spec, keinen Identifier.
        named_alt(
            "anonymous",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_BITFIELD_SPEC),
                Symbol::Terminal(TokenKind::Punct(";")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_BITFIELD_SPEC: Production = Production {
    id: ID_BITFIELD_SPEC,
    name: "bitfield_spec",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        // destination_type per Spec: boolean | integer_type | octet_type.
        // base_type_spec deckt alle drei (plus weitere primitives) ab —
        // das ist permissiv aber praktisch, da AST-Builder sowieso
        // semantisch validiert.
        named_alt(
            "with_dest",
            &[
                Symbol::Terminal(TokenKind::Keyword("bitfield")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_BASE_TYPE_SPEC),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
        named_alt(
            "plain",
            &[
                Symbol::Terminal(TokenKind::Keyword("bitfield")),
                Symbol::Terminal(TokenKind::Punct("<")),
                Symbol::Nonterminal(ID_POSITIVE_INT_CONST),
                Symbol::Terminal(TokenKind::Punct(">")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_BITMASK_DCL: Production = Production {
    id: ID_BITMASK_DCL,
    name: "bitmask_dcl",
    spec_ref: spec("7.4.13"),
    alternatives: &[alt(&[
        Symbol::Terminal(TokenKind::Keyword("bitmask")),
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("{")),
        Symbol::Nonterminal(ID_BIT_VALUE_LIST),
        Symbol::Terminal(TokenKind::Punct("}")),
    ])],
    ast_hint: None,
};

pub const PROD_BIT_VALUE_LIST: Production = Production {
    id: ID_BIT_VALUE_LIST,
    name: "bit_value_list",
    spec_ref: spec("7.4.13"),
    alternatives: &[
        // bit_value entspricht annotated identifier (analog enumerator).
        named_alt(
            "single",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_BIT_VALUE_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_IDENTIFIER),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANY_TYPE: Production = Production {
    id: ID_ANY_TYPE,
    name: "any_type",
    spec_ref: spec("7.4.1.4.4.1"),
    alternatives: &[alt(&[Symbol::Terminal(TokenKind::Keyword("any"))])],
    ast_hint: None,
};

// ============================================================================
// §8.3 Annotations (T4.4)
// ============================================================================
// BNF aus IDL 4.2 §8.3:
//   <annotation_appl>        ::= "@" <scoped_name>
//                                [ "(" <annotation_appl_params> ")" ]
//   <annotation_appl_params> ::= <const_expr>
//                              | <annotation_appl_param>
//                                { "," <annotation_appl_param> }*
//   <annotation_appl_param>  ::= <identifier> "=" <const_expr>
//
// Standard-Annotations (~25 in §8.3.x): @id, @autoid, @optional, @position,
// @value, @extensibility, @final, @appendable, @mutable, @key,
// @must_understand, @default_literal, @default, @range, @min, @max, @unit,
// @bit_bound, @external, @nested, @verbatim, @service, @oneway, @ami,
// @topic, @hashid, @ignore_literal_names, @try_construct.
//
// Wichtig: NICHT als Keywords registriert. Annotation-Namen sind
// <scoped_name> nach "@" — ein normaler Identifier-Pfad. Das erlaubt es,
// auch Vendor-spezifische Annotations (z.B. @rti::optional) zu parsen,
// ohne Grammar-Aenderung. Die semantische Behandlung der Standard-Namen
// passiert spaeter im AST-Builder.
//
// Hook-Punkte (annotation_appl_seq als Prefix): <definition>, <member>,
// <case>, <enumerator>, <op_dcl>, <attr_dcl>, <param_dcl>, <export>.
//
// `<annotation_appl_params>`: vereinfachte Disambiguation. Spec erlaubt
// "single const_expr" oder "named param list". Ein einzelner const_expr
// mit nur einem Identifier waere mehrdeutig (Identifier = sowohl
// scoped_name als auch named-param-LHS). Wir handhaben das so:
// - Wenn das Token nach "(" ein Ident ist gefolgt von "=", dann
//   named-param-list. Earley-Engine resolved das automatisch ueber die
//   Alternativen.

pub const PROD_ANNOTATION_APPL_SEQ: Production = Production {
    id: ID_ANNOTATION_APPL_SEQ,
    name: "annotation_appl_seq",
    spec_ref: spec("8.3"),
    alternatives: &[
        Alternative {
            name: Some("empty"),
            symbols: &[],
            note: None,
        },
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_SEQ),
                Symbol::Nonterminal(ID_ANNOTATION_APPL),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANNOTATION_APPL: Production = Production {
    id: ID_ANNOTATION_APPL,
    name: "annotation_appl",
    spec_ref: spec("8.3"),
    alternatives: &[
        named_alt(
            "with_params",
            &[
                Symbol::Terminal(TokenKind::Punct("@")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Nonterminal(ID_ANNOTATION_APPL_PARAMS),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
        named_alt(
            "no_params",
            &[
                Symbol::Terminal(TokenKind::Punct("@")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
            ],
        ),
        named_alt(
            "empty_params",
            &[
                Symbol::Terminal(TokenKind::Punct("@")),
                Symbol::Nonterminal(ID_SCOPED_NAME),
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANNOTATION_APPL_PARAMS: Production = Production {
    id: ID_ANNOTATION_APPL_PARAMS,
    name: "annotation_appl_params",
    spec_ref: spec("8.3"),
    alternatives: &[
        // Named-param-list zuerst — Earley waehlt korrekte Alternative
        // ueber Lookahead auf das "=" nach dem Identifier.
        named_alt(
            "named",
            &[Symbol::Nonterminal(ID_ANNOTATION_APPL_PARAM_LIST)],
        ),
        named_alt("single_expr", &[Symbol::Nonterminal(ID_CONST_EXPR)]),
    ],
    ast_hint: None,
};

pub const PROD_ANNOTATION_APPL_PARAM_LIST: Production = Production {
    id: ID_ANNOTATION_APPL_PARAM_LIST,
    name: "annotation_appl_param_list",
    spec_ref: spec("8.3"),
    alternatives: &[
        named_alt("single", &[Symbol::Nonterminal(ID_ANNOTATION_APPL_PARAM)]),
        named_alt(
            "cons",
            &[
                Symbol::Nonterminal(ID_ANNOTATION_APPL_PARAM_LIST),
                Symbol::Terminal(TokenKind::Punct(",")),
                Symbol::Nonterminal(ID_ANNOTATION_APPL_PARAM),
            ],
        ),
    ],
    ast_hint: None,
};

pub const PROD_ANNOTATION_APPL_PARAM: Production = Production {
    id: ID_ANNOTATION_APPL_PARAM,
    name: "annotation_appl_param",
    spec_ref: spec("8.3"),
    alternatives: &[alt(&[
        Symbol::Nonterminal(ID_IDENTIFIER),
        Symbol::Terminal(TokenKind::Punct("=")),
        Symbol::Nonterminal(ID_CONST_EXPR),
    ])],
    ast_hint: None,
};

// ============================================================================
// IDL_42 Grammar
// ============================================================================
// Die `productions`-Liste muss in derselben Reihenfolge wie die
// ID_*-Konstanten stehen, damit `Grammar::production(id)` ein O(1)-
// Slice-Index-Lookup ist.
//
// Start-Production ist `<specification>` (T3.2). Bis dahin ist die
// Grammar als Stub mit ID_IDENTIFIER als Start markiert — funktional
// nicht akzeptierend fuer komplette IDL-Files, aber lexikal korrekt.

pub const IDL_42: Grammar = Grammar {
    name: "OMG IDL 4.2",
    version: IdlVersion::V4_2,
    productions: &[
        // §7.2 Lexical (T3.1)
        PROD_IDENTIFIER,
        PROD_INTEGER_LITERAL,
        PROD_FLOATING_PT_LITERAL,
        PROD_FIXED_PT_LITERAL,
        PROD_CHARACTER_LITERAL,
        PROD_WIDE_CHARACTER_LITERAL,
        PROD_STRING_LITERAL,
        PROD_WIDE_STRING_LITERAL,
        PROD_BOOLEAN_LITERAL,
        // §7.4.1.1–3 Specification/Definition/Module (T3.2)
        PROD_SPECIFICATION,
        PROD_DEFINITION_LIST,
        PROD_DEFINITION,
        PROD_MODULE_DCL,
        // §7.4.1.4 Type Decls + Specs (T3.3)
        PROD_TYPE_DCL,
        PROD_TYPE_SPEC,
        PROD_SIMPLE_TYPE_SPEC,
        PROD_SCOPED_NAME,
        PROD_SCOPED_NAME_TAIL,
        // §7.4.1.4.4.1 Primitive Types (T3.4)
        PROD_BASE_TYPE_SPEC,
        PROD_INTEGER_TYPE,
        PROD_SIGNED_INT,
        PROD_UNSIGNED_INT,
        PROD_FLOATING_PT_TYPE,
        PROD_CHAR_TYPE,
        PROD_WIDE_CHAR_TYPE,
        PROD_BOOLEAN_TYPE,
        PROD_OCTET_TYPE,
        // §7.4.1.4.3 Template Types (T3.5)
        PROD_TEMPLATE_TYPE_SPEC,
        PROD_SEQUENCE_TYPE,
        PROD_STRING_TYPE,
        PROD_WIDE_STRING_TYPE,
        PROD_FIXED_PT_TYPE,
        PROD_POSITIVE_INT_CONST,
        // §7.4.1.4.4 Constructed Types — Struct (T3.6)
        PROD_CONSTR_TYPE_DCL,
        PROD_STRUCT_DCL,
        PROD_STRUCT_DEF,
        PROD_STRUCT_FORWARD_DCL,
        PROD_MEMBER_LIST,
        PROD_MEMBER,
        PROD_DECLARATORS,
        PROD_DECLARATOR,
        PROD_SIMPLE_DECLARATOR,
        // §7.4.1.4.4.4 Enum (T3.7)
        PROD_ENUM_DCL,
        PROD_ENUMERATOR_LIST,
        PROD_ENUMERATOR,
        // §7.4.1.4.4.5 Const Decl (T3.8)
        PROD_CONST_DCL,
        PROD_CONST_TYPE,
        PROD_CONST_EXPR,
        PROD_LITERAL,
        // §7.4.1.4.4.6 Typedef + Array-Declarators (T3.9)
        PROD_TYPEDEF_DCL,
        PROD_TYPE_DECLARATOR,
        PROD_ANY_DECLARATORS,
        PROD_ANY_DECLARATOR,
        PROD_ARRAY_DECLARATOR,
        PROD_FIXED_ARRAY_SIZES,
        PROD_FIXED_ARRAY_SIZE,
        // §7.4.1.4.4.7 Except Decl (T3.10)
        PROD_EXCEPT_DCL,
        // §7.4.1.4.4.5 const_expr Operator-Stack (T-LIM-3)
        PROD_OR_EXPR,
        PROD_XOR_EXPR,
        PROD_AND_EXPR,
        PROD_SHIFT_EXPR,
        PROD_ADD_EXPR,
        PROD_MULT_EXPR,
        PROD_UNARY_EXPR,
        PROD_PRIMARY_EXPR,
        // §7.4.1.4.4.3 Union (T-LIM-4)
        PROD_UNION_DCL,
        PROD_UNION_DEF,
        PROD_UNION_FORWARD_DCL,
        PROD_SWITCH_TYPE_SPEC,
        PROD_SWITCH_BODY,
        PROD_CASE,
        PROD_CASE_LABELS,
        PROD_CASE_LABEL,
        PROD_ELEMENT_SPEC,
        // §7.4.2 Interface (T-LIM-5)
        PROD_INTERFACE_DCL,
        PROD_INTERFACE_DEF,
        PROD_INTERFACE_FORWARD_DCL,
        PROD_INTERFACE_HEADER,
        PROD_INTERFACE_KIND,
        PROD_INTERFACE_INHERITANCE_SPEC,
        PROD_INTERFACE_NAME_LIST,
        PROD_INTERFACE_BODY,
        PROD_EXPORT_LIST,
        PROD_EXPORT,
        PROD_OP_DCL,
        PROD_OP_TYPE_SPEC,
        PROD_PARAMETER_DCLS,
        PROD_PARAM_DCL_LIST,
        PROD_PARAM_DCL,
        PROD_PARAM_ATTRIBUTE,
        PROD_RAISES_EXPR,
        PROD_SCOPED_NAME_LIST,
        PROD_ATTR_DCL,
        // §7.4.3.1 Rules 90-97 — Multi-Name + Raises (RPC-blocker)
        PROD_READONLY_ATTR_SPEC,
        PROD_READONLY_ATTR_DECLARATOR,
        PROD_ATTR_SPEC,
        PROD_ATTR_DECLARATOR,
        PROD_ATTR_RAISES_EXPR,
        PROD_GET_EXCEP_EXPR,
        PROD_SET_EXCEP_EXPR,
        PROD_EXCEPTION_LIST,
        // §7.4.15 Rules 218-223 — User-Defined Annotation Declarations
        PROD_ANNOTATION_DCL,
        PROD_ANNOTATION_HEADER,
        PROD_ANNOTATION_BODY,
        PROD_ANNOTATION_BODY_MEMBER,
        PROD_ANNOTATION_MEMBER,
        // §7.4.1.3 Rule 61 — Native-Type-Decl
        PROD_NATIVE_DCL,
        // §7.4.3 Valuetype (T-LIM-5, minimal)
        PROD_VALUE_BOX_DCL,
        PROD_VALUE_FORWARD_DCL,
        // §7.4.5 Rules 100-109 — Value Types Full
        PROD_VALUE_DEF,
        PROD_VALUE_HEADER,
        PROD_VALUE_INHERITANCE_SPEC,
        PROD_VALUE_INHERITANCE_NAME_LIST,
        PROD_VALUE_ELEMENT,
        PROD_STATE_MEMBER,
        PROD_INIT_DCL,
        PROD_INIT_PARAM_DCLS,
        PROD_INIT_PARAM_DCL,
        // §7.4.6 Rules 111-117 — Repository-IDs / Typeprefix / Import
        PROD_TYPE_ID_DCL,
        PROD_TYPE_PREFIX_DCL,
        PROD_IMPORT_DCL,
        PROD_IMPORTED_SCOPE,
        // §7.4.9 Components / Homes / Eventtypes / Ports / Connectors
        PROD_COMPONENT_DCL,
        PROD_COMPONENT_FORWARD_DCL,
        PROD_COMPONENT_DEF,
        PROD_COMPONENT_HEADER,
        PROD_COMPONENT_INHERITANCE_SPEC,
        PROD_SUPPORTED_INTERFACE_SPEC,
        PROD_COMPONENT_BODY,
        PROD_COMPONENT_EXPORT,
        PROD_PROVIDES_DCL,
        PROD_USES_DCL,
        PROD_INTERFACE_TYPE,
        PROD_HOME_DCL,
        PROD_HOME_HEADER,
        PROD_HOME_INHERITANCE_SPEC,
        PROD_PRIMARY_KEY_SPEC,
        PROD_HOME_BODY,
        PROD_HOME_EXPORT,
        PROD_FACTORY_DCL,
        PROD_FACTORY_PARAM_DCLS,
        PROD_FINDER_DCL,
        PROD_EVENT_DCL,
        PROD_EVENT_FORWARD_DCL,
        PROD_EVENT_DEF,
        PROD_EVENT_HEADER,
        PROD_EMITS_DCL,
        PROD_PUBLISHES_DCL,
        PROD_CONSUMES_DCL,
        PROD_PORTTYPE_DCL,
        PROD_PORT_BODY,
        PROD_PORT_REF,
        PROD_PORT_EXPORT,
        PROD_PORT_DCL,
        PROD_CONNECTOR_DCL,
        PROD_CONNECTOR_HEADER,
        PROD_CONNECTOR_INHERIT_SPEC,
        PROD_CONNECTOR_EXPORT,
        // §7.4.12 Template Modules
        PROD_TEMPLATE_MODULE_DCL,
        PROD_TPL_DEFINITION,
        PROD_FORMAL_PARAMETERS,
        PROD_FORMAL_PARAMETER,
        PROD_FORMAL_PARAMETER_TYPE,
        PROD_TEMPLATE_MODULE_INST,
        PROD_ACTUAL_PARAMETERS,
        PROD_ACTUAL_PARAMETER,
        PROD_TEMPLATE_MODULE_REF,
        PROD_FORMAL_PARAMETER_NAMES,
        PROD_OP_WITH_CONTEXT,
        PROD_CONTEXT_EXPR,
        PROD_CONTEXT_STRING_LIST,
        PROD_VALUE_BASE_TYPE,
        // §8.3 Annotations (T4.4)
        PROD_ANNOTATION_APPL_SEQ,
        PROD_ANNOTATION_APPL,
        PROD_ANNOTATION_APPL_PARAMS,
        PROD_ANNOTATION_APPL_PARAM_LIST,
        PROD_ANNOTATION_APPL_PARAM,
        // §7.4.13 Extended Data Types BB — Map/Bitset/Bitmask/Any (T4.6)
        PROD_MAP_TYPE,
        PROD_BITSET_DCL,
        PROD_BITFIELD_LIST,
        PROD_BITFIELD,
        PROD_BITFIELD_SPEC,
        PROD_BITMASK_DCL,
        PROD_BIT_VALUE_LIST,
        PROD_ANY_TYPE,
    ],
    start: ID_SPECIFICATION,
    token_rules: &[],
};

// `AltRef` ist re-exportiert, damit Test-Module nicht zwei super-Pfade brauchen.
#[allow(dead_code)]
const _ALT_REF_TYPE: Option<AltRef> = None;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::validate::validate;

    #[test]
    fn idl42_grammar_validates_at_lexical_layer() {
        let report = validate(&IDL_42);
        // Erlaubt sind Warnungen (z.B. UnusedProduction, weil Lexical-
        // Productions noch keine Verbraucher haben). Keine Errors.
        assert!(
            !report.has_errors(),
            "Errors: {:?}",
            report.errors().collect::<Vec<_>>()
        );
    }

    #[test]
    fn idl42_full_grammar_validation_sweep() {
        // T4.9 — explizites Audit: keine InvalidStart/Dangling-Errors,
        // Warnings aufgelistet als Audit-Trail (Left-Recursion erwartet
        // bei Operator-Stack/List-Productions; UnusedProduction bei
        // Lexical-Stubs).
        use crate::grammar::validate::ValidationIssue;
        let report = validate(&IDL_42);
        assert!(
            !report.has_errors(),
            "Errors: {:?}",
            report.errors().collect::<Vec<_>>()
        );
        // Audit: kategorisiere Warnings.
        let mut left_rec = 0;
        let mut unused = 0;
        let mut first_first = 0;
        let mut other = 0;
        for w in report.warnings() {
            match w {
                ValidationIssue::LeftRecursion { .. } => left_rec += 1,
                ValidationIssue::UnusedProduction { .. } => unused += 1,
                ValidationIssue::FirstFirstConflict { .. } => first_first += 1,
                _ => other += 1,
            }
        }
        // FirstFirstConflicts sind in EBNF-roher Form normal
        // (annotation_appl_seq vor mehreren Alternativen). Sie sind
        // keine Korrektheits-Probleme — der Earley-Parser handhabt
        // alle Konflikte korrekt; FF-Warnings markieren nur potenzielle
        // Slow-Path-Stellen fuer LL/LALR-Backends.
        // Erwartete Baseline (Audit-Trail): 22 LeftRecursion (Operator-
        // Stack + *_list-Productions), <= 6 UnusedProduction (
        // Audit-Stubs aus Tab. 7-6: alias/context/ValueBase, deren
        // Composer-Hooks erst in Phase 5 des Action-Plans aktiviert
        // werden), ~90 FirstFirstConflict (annotation_appl_seq-Hooks).
        assert_eq!(other, 0, "unexpected warning kinds detected");
        assert!(
            unused <= 6,
            "unused productions: {unused} (max 6 für \
             Tab.7-6-Keyword-Stubs: template_module_ref, \
             formal_parameter_names, op_with_context, context_expr, \
             context_string_list, value_base_type)"
        );
        assert!(left_rec > 0, "operator stack expected to flag LR");
        assert!(first_first > 0, "annotation hooks expected to flag FF");
    }

    #[test]
    fn production_count_grows_with_each_section() {
        // 9+4+5+9+6+9+3+4+7+1+8+9 = 74 (T-LIM-4 +9 union)
        // + 19 interface + 2 valuetype = 95 (T-LIM-5)
        // + 5 annotation = 100 (T4.4)
        // + 8 map/bitset/bitmask/any = 108 (T4.6)
        // + 8 §7.4.3.1 Rules 90-97 (multi-name attr + raises) = 116
        // + 5 §7.4.15 Rules 218-223 (user-defined annotation_dcl) = 121
        // + 1 §7.4.1.3 Rule 61 (native_dcl) = 122
        // + 9 §7.4.5 Rules 100-109 (value_def + state_member + init_dcl
        //   + value_inheritance) = 131
        // + 4 §7.4.6 Rules 111-117 (typeid + typeprefix + import +
        //   imported_scope) = 135
        // + 36 §7.4.9 (components/homes/eventtypes/ports/connectors) = 171
        // + 7 §7.4.12 (template_module_dcl/_inst, formal/actual_parameters) = 178.
        // + 2 §7.4.12 Rules (193)/(194) (template_module_ref +
        //   formal_parameter_names) = 180 (Audit nachgezogen,
        //   damit `alias`-Keyword aus Tab. 7-6 vom Lexer erkannt wird).
        // + 3 §7.4.6 Rules (123)/(124) (op_with_context, context_expr,
        //   context_string_list) = 183 (`context`-Keyword).
        // + 1 §7.4.7 Rule (132) (value_base_type) = 184 (
        // `ValueBase`-Keyword).
        // + 1 §7.4.12 Rule (188) (tpl_definition) = 185 (
        // alias-Ref-Hook in template_module_dcl-Body).
        assert_eq!(IDL_42.production_count(), 185);
    }

    #[test]
    fn production_ids_are_stable() {
        // Regression: Konsumenten verlassen sich auf ID-Konstanten.
        assert_eq!(
            IDL_42.production(ID_IDENTIFIER).map(|p| p.name),
            Some("identifier")
        );
        assert_eq!(
            IDL_42.production(ID_INTEGER_LITERAL).map(|p| p.name),
            Some("integer_literal")
        );
        assert_eq!(
            IDL_42.production(ID_BOOLEAN_LITERAL).map(|p| p.name),
            Some("boolean_literal")
        );
    }

    #[test]
    fn boolean_literal_has_two_named_alternatives() {
        let prod = IDL_42.production(ID_BOOLEAN_LITERAL).expect("must exist");
        assert_eq!(prod.alternatives.len(), 2);
        assert_eq!(prod.alternatives[0].name, Some("true"));
        assert_eq!(prod.alternatives[1].name, Some("false"));
    }

    // -----------------------------------------------------------------
    // T3.2 — Specification/Definition/Module
    // -----------------------------------------------------------------

    #[test]
    fn specification_is_start_production() {
        assert_eq!(IDL_42.start, ID_SPECIFICATION);
    }

    #[test]
    fn module_dcl_has_five_symbols() {
        let m = IDL_42.production(ID_MODULE_DCL).expect("must exist");
        assert_eq!(m.alternatives.len(), 1);
        assert_eq!(m.alternatives[0].symbols.len(), 5);
    }

    #[test]
    fn parses_empty_module() {
        use crate::engine::parse;
        use crate::lexer::Tokenizer;

        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer.tokenize("module M {};").expect("tokenize");
        let result = parse(&IDL_42, stream.tokens());
        assert!(result.is_ok(), "parse failed: {result:?}");
    }

    #[test]
    fn parses_nested_modules() {
        use crate::engine::parse;
        use crate::lexer::Tokenizer;

        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer
            .tokenize("module Outer { module Inner {}; };")
            .expect("tokenize");
        let result = parse(&IDL_42, stream.tokens());
        assert!(result.is_ok(), "parse failed: {result:?}");
    }

    #[test]
    fn parses_multiple_top_level_modules() {
        use crate::engine::parse;
        use crate::lexer::Tokenizer;

        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer
            .tokenize("module A {}; module B {};")
            .expect("tokenize");
        let result = parse(&IDL_42, stream.tokens());
        assert!(result.is_ok(), "parse failed: {result:?}");
    }

    #[test]
    fn rejects_module_without_braces() {
        use crate::engine::parse;
        use crate::lexer::Tokenizer;

        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer.tokenize("module M;").expect("tokenize");
        let result = parse(&IDL_42, stream.tokens());
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------
    // T3.3 + T3.4 — Types
    // -----------------------------------------------------------------

    fn try_parse(src: &str) -> Result<(), String> {
        use crate::engine::parse;
        use crate::lexer::Tokenizer;
        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer.tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
        parse(&IDL_42, stream.tokens()).map_err(|e| format!("parse: {e:?}"))?;
        Ok(())
    }

    #[test]
    fn parses_typedef_with_primitive_types() {
        // Stub-typedef ohne abschliessenden Identifier-Suffix (Arrays etc.
        // kommen mit T3.9). T3.3-Stub: typedef <type> <ident>
        for src in [
            "typedef short A;",
            "typedef long B;",
            "typedef long long C;",
            "typedef unsigned short D;",
            "typedef unsigned long long E;",
            "typedef int8 F;",
            "typedef uint64 G;",
            "typedef float H;",
            "typedef double I;",
            "typedef long double J;",
            "typedef char K;",
            "typedef wchar L;",
            "typedef boolean M;",
            "typedef octet N;",
        ] {
            assert!(try_parse(src).is_ok(), "expected ok: {src}");
        }
    }

    #[test]
    fn parses_typedef_with_scoped_name() {
        // typedef Foo Alias; und typedef ::Foo::Bar Alias2;
        assert!(try_parse("typedef Foo Alias;").is_ok());
        assert!(try_parse("typedef ::Foo::Bar Alias2;").is_ok());
        assert!(try_parse("typedef Outer::Inner::Type X;").is_ok());
    }

    #[test]
    fn parses_typedef_inside_module() {
        let src = r"
            module Pkg {
                typedef long Counter;
                typedef boolean Flag;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T3.5 — Template Types
    // -----------------------------------------------------------------

    #[test]
    fn parses_unbounded_string_typedef() {
        assert!(try_parse("typedef string Name;").is_ok());
        assert!(try_parse("typedef wstring WName;").is_ok());
    }

    #[test]
    fn parses_bounded_string_typedef() {
        assert!(try_parse("typedef string<32> Short;").is_ok());
        assert!(try_parse("typedef wstring<128> Title;").is_ok());
    }

    #[test]
    fn parses_unbounded_sequence_typedef() {
        assert!(try_parse("typedef sequence<long> Numbers;").is_ok());
        assert!(try_parse("typedef sequence<Foo> Items;").is_ok());
    }

    #[test]
    fn parses_bounded_sequence_typedef() {
        assert!(try_parse("typedef sequence<long, 100> SmallNumbers;").is_ok());
    }

    #[test]
    fn parses_nested_sequence_typedef() {
        assert!(try_parse("typedef sequence<sequence<long>> Matrix;").is_ok());
    }

    #[test]
    fn parses_fixed_pt_typedef() {
        assert!(try_parse("typedef fixed<10, 2> Money;").is_ok());
    }

    #[test]
    fn rejects_string_with_invalid_bound() {
        // String braucht positive int, nicht string-literal.
        assert!(try_parse(r#"typedef string<"big"> Bad;"#).is_err());
    }

    // -----------------------------------------------------------------
    // T3.6 — Struct
    // -----------------------------------------------------------------

    #[test]
    fn parses_empty_struct() {
        assert!(try_parse("struct Empty {};").is_ok());
    }

    #[test]
    fn parses_struct_with_single_member() {
        assert!(try_parse("struct Point { long x; };").is_ok());
    }

    #[test]
    fn parses_struct_with_multiple_members() {
        let src = r"
            struct Point3D {
                long x;
                long y;
                long z;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_struct_with_multiple_declarators_in_one_member() {
        // long x, y, z; ist eine member-Definition mit 3 declarators.
        assert!(try_parse("struct Vec { long x, y, z; };").is_ok());
    }

    #[test]
    fn parses_struct_with_template_type_member() {
        let src = r"
            struct Container {
                sequence<long> values;
                string<32> name;
                string description;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_struct_with_scoped_name_member() {
        assert!(try_parse("struct Foo { Pkg::Bar b; };").is_ok());
    }

    #[test]
    fn parses_struct_forward_declaration() {
        assert!(try_parse("struct Future;").is_ok());
    }

    #[test]
    fn parses_struct_inside_module() {
        let src = r"
            module Geo {
                struct Point { long x; long y; };
                struct Polygon { sequence<Point> vertices; };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T3.7 — Enum
    // -----------------------------------------------------------------

    #[test]
    fn parses_single_enumerator_enum() {
        assert!(try_parse("enum Color { RED };").is_ok());
    }

    #[test]
    fn parses_multi_enumerator_enum() {
        assert!(try_parse("enum Color { RED, GREEN, BLUE };").is_ok());
    }

    #[test]
    fn rejects_enum_without_enumerator() {
        // Enum braucht mindestens 1 enumerator.
        assert!(try_parse("enum Empty {};").is_err());
    }

    #[test]
    fn parses_enum_inside_module_and_struct_using_it() {
        let src = r"
            module Geo {
                enum Direction { NORTH, EAST, SOUTH, WEST };
                struct Vector { Direction dir; long magnitude; };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T3.8 — Const Decl
    // -----------------------------------------------------------------

    #[test]
    fn parses_int_const() {
        assert!(try_parse("const long MAX = 100;").is_ok());
        assert!(try_parse("const unsigned long PI_DIGITS = 3;").is_ok());
    }

    #[test]
    fn parses_float_const() {
        assert!(try_parse("const float PI = 3.14;").is_ok());
        assert!(try_parse("const double E = 2.71828;").is_ok());
    }

    #[test]
    fn parses_string_const() {
        assert!(try_parse(r#"const string GREETING = "hello";"#).is_ok());
        assert!(try_parse(r#"const string<32> SHORT = "abc";"#).is_ok());
    }

    #[test]
    fn parses_boolean_const() {
        assert!(try_parse("const boolean ENABLED = TRUE;").is_ok());
        assert!(try_parse("const boolean DISABLED = FALSE;").is_ok());
    }

    #[test]
    fn parses_const_with_scoped_name_value() {
        assert!(try_parse("const long ALIAS = Limits::MAX;").is_ok());
    }

    #[test]
    fn parses_const_in_module() {
        let src = r#"
            module Constants {
                const long MAX = 1024;
                const float RATIO = 0.5;
                const string NAME = "module";
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T3.9 — Typedef + Arrays
    // -----------------------------------------------------------------

    #[test]
    fn parses_typedef_simple_array() {
        assert!(try_parse("typedef long Buffer[10];").is_ok());
    }

    #[test]
    fn parses_typedef_multi_dim_array() {
        assert!(try_parse("typedef long Matrix[10][10];").is_ok());
    }

    #[test]
    fn parses_typedef_with_multiple_declarators() {
        assert!(try_parse("typedef long Foo, Bar, Baz;").is_ok());
    }

    #[test]
    fn parses_typedef_mixed_simple_and_array() {
        assert!(try_parse("typedef long X, Y[10], Z;").is_ok());
    }

    #[test]
    fn parses_typedef_template_with_array() {
        assert!(try_parse("typedef sequence<long> Items[5];").is_ok());
    }

    // -----------------------------------------------------------------
    // T3.10 — Except
    // -----------------------------------------------------------------

    #[test]
    fn parses_empty_exception() {
        assert!(try_parse("exception NotFound {};").is_ok());
    }

    #[test]
    fn parses_exception_with_members() {
        let src = r#"
            exception InvalidArgument {
                string reason;
                long code;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T-LIM-3 — const_expr Operator-Stack
    // -----------------------------------------------------------------

    #[test]
    fn parses_const_arithmetic() {
        assert!(try_parse("const long N = 1 + 2;").is_ok());
        assert!(try_parse("const long N = 10 - 5;").is_ok());
        assert!(try_parse("const long N = 4 * 8;").is_ok());
        assert!(try_parse("const long N = 100 / 4;").is_ok());
        assert!(try_parse("const long N = 7 % 3;").is_ok());
    }

    #[test]
    fn parses_const_bitwise() {
        assert!(try_parse("const long N = 0xFF | 0x0F;").is_ok());
        assert!(try_parse("const long N = 0xFF & 0x0F;").is_ok());
        assert!(try_parse("const long N = 0xFF ^ 0x0F;").is_ok());
        assert!(try_parse("const long N = ~0xFF;").is_ok());
    }

    #[test]
    fn parses_const_shift() {
        assert!(try_parse("const long N = 1 << 8;").is_ok());
        assert!(try_parse("const long N = 256 >> 4;").is_ok());
    }

    #[test]
    fn parses_const_unary() {
        assert!(try_parse("const long N = -42;").is_ok());
        assert!(try_parse("const long N = +1;").is_ok());
        assert!(try_parse("const long N = ~0;").is_ok());
    }

    #[test]
    fn parses_const_with_parens_and_precedence() {
        // (1 + 2) * 3
        assert!(try_parse("const long N = (1 + 2) * 3;").is_ok());
        // 1 + 2 * 3 (Praezedenz: * vor +)
        assert!(try_parse("const long N = 1 + 2 * 3;").is_ok());
    }

    #[test]
    fn parses_const_with_scoped_name_and_arithmetic() {
        assert!(try_parse("const long N = MAX + 10;").is_ok());
        assert!(try_parse("const long N = Limits::MAX * 2;").is_ok());
    }

    // -----------------------------------------------------------------
    // T-LIM-4 — Union
    // -----------------------------------------------------------------

    #[test]
    fn parses_union_with_integer_discriminator() {
        let src = r#"
            union Variant switch (long) {
                case 1: long int_value;
                case 2: string str_value;
                default: boolean fallback;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_union_with_boolean_discriminator() {
        let src = r#"
            union Maybe switch (boolean) {
                case TRUE: long value;
                case FALSE: string error;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_union_with_multiple_labels_per_case() {
        // case 1: case 2: long both;
        let src = r#"
            union Multi switch (long) {
                case 1: case 2: long both;
                default: string fallback;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_union_forward_declaration() {
        assert!(try_parse("union Future;").is_ok());
    }

    #[test]
    fn parses_union_in_module() {
        let src = r#"
            module Variants {
                union Result switch (boolean) {
                    case TRUE: long value;
                    case FALSE: string error;
                };
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_exception_in_module() {
        let src = r"
            module Errors {
                exception NotFound {};
                exception Forbidden { string user; };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T-LIM-5 — Interface
    // -----------------------------------------------------------------

    #[test]
    fn parses_empty_interface() {
        assert!(try_parse("interface Foo {};").is_ok());
    }

    #[test]
    fn parses_interface_forward_declaration() {
        assert!(try_parse("interface Foo;").is_ok());
    }

    #[test]
    fn parses_abstract_interface() {
        assert!(try_parse("abstract interface Foo {};").is_ok());
    }

    #[test]
    fn parses_local_interface() {
        assert!(try_parse("local interface Foo {};").is_ok());
    }

    #[test]
    fn parses_interface_with_inheritance() {
        let src = "interface Derived : Base {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_multiple_inheritance() {
        let src = "interface Derived : Base1, Base2, ::ns::Base3 {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_void_op() {
        let src = r"
            interface Service {
                void shutdown();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_typed_op_and_params() {
        let src = r"
            interface Calculator {
                long add(in long a, in long b);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_oneway_operation() {
        let src = r"
            interface Logger {
                oneway void log(in string msg);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_operation_with_raises() {
        let src = r"
            interface Repo {
                long lookup(in string key) raises (NotFound);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_operation_with_inout_and_out_params() {
        let src = r"
            interface Pipe {
                void transfer(in long src, out long dst, inout long acc);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_attribute() {
        let src = r"
            interface Counter {
                attribute long value;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_readonly_attribute() {
        let src = r"
            interface Counter {
                readonly attribute long value;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // §7.4.3.1 Rules 90/92-96 — multi-name attr + raises (R-blocker RPC)
    // -----------------------------------------------------------------

    #[test]
    fn parses_attr_multi_name() {
        let src = r"
            interface I {
                attribute long a, b, c;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_readonly_attr_multi_name() {
        let src = r"
            interface I {
                readonly attribute long x, y;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_attr_with_getraises() {
        let src = r"
            interface I {
                exception Err {};
                attribute long val getraises (Err);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_attr_with_setraises() {
        let src = r"
            interface I {
                exception Err {};
                attribute long val setraises (Err);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_attr_with_get_and_setraises() {
        let src = r"
            interface I {
                exception E1 {};
                exception E2 {};
                attribute long val getraises (E1) setraises (E2);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_readonly_attr_with_raises() {
        let src = r"
            interface I {
                exception Err {};
                readonly attribute long val raises (Err);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_attr_exception_list_multi() {
        let src = r"
            interface I {
                exception E1 {};
                exception E2 {};
                exception E3 {};
                attribute long val getraises (E1, E2, E3);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn rejects_attr_setraises_before_getraises() {
        // Spec: attr_raises_expr ::= get_excep_expr [set_excep_expr] | set_excep_expr
        // also: setraises allein ok, getraises allein ok, beide nur in Reihenfolge get→set.
        // setraises FOLGEND get ist erlaubt; das Gegenteil (set vor get) NICHT.
        let src = r"
            interface I {
                exception E1 {};
                exception E2 {};
                attribute long val setraises (E2) getraises (E1);
            };
        ";
        assert!(try_parse(src).is_err(), "expected err: {src}");
    }

    #[test]
    fn parses_interface_with_nested_type_and_const() {
        let src = r"
            interface Mixed {
                const long MAX = 100;
                typedef sequence<long> Buffer;
                long size();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // §7.4.4 Rule 97 — Interfaces-Full Embedded Decls
    // (R-blocker fuer DDS-RPC: Inner-Types in Service-Interfaces)
    // -----------------------------------------------------------------

    #[test]
    fn parses_interface_with_embedded_exception() {
        let src = r"
            interface Service {
                exception NotFound {};
                long lookup(in long key) raises (NotFound);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_embedded_struct() {
        let src = r"
            interface Service {
                struct Header { long ts; long seq; };
                Header next();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_embedded_enum() {
        let src = r"
            interface Service {
                enum State { READY, BUSY, DOWN };
                State status();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_embedded_union() {
        let src = r"
            interface Service {
                union Result switch (long) {
                    case 0: long ok;
                    default: string err;
                };
                Result poll();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_full_interface_all_export_kinds() {
        // §7.4.4 Rule 97: interface body kann op/attr/type/const/except.
        let src = r"
            interface Full {
                exception Bad {};
                const long DEFAULT_TTL = 60;
                typedef sequence<long> IdList;
                struct Header { long ts; };
                attribute long counter;
                readonly attribute Header last;
                IdList query(in long limit) raises (Bad);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_with_inner_type_used_in_op() {
        // Realistisches DDS-RPC-Pattern: Inner-Type wird Return-Type der Op.
        // (`port` ist seit B5 Keyword fuer ccm-port-Decl — Field heisst port_num.)
        let src = r"
            interface Locator {
                struct Endpoint {
                    string host;
                    long port_num;
                };
                Endpoint resolve(in string name);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_interface_in_module() {
        let src = r"
            module svc {
                interface Service {
                    void ping();
                };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn rejects_interface_without_braces() {
        assert!(try_parse("interface Foo;").is_ok());
        assert!(try_parse("interface Foo").is_err());
    }

    // -----------------------------------------------------------------
    // T-LIM-5 — Valuetype (minimal)
    // -----------------------------------------------------------------

    #[test]
    fn parses_value_box_dcl() {
        assert!(try_parse("valuetype MyBox long;").is_ok());
    }

    #[test]
    fn parses_value_box_with_string() {
        assert!(try_parse("valuetype Name string;").is_ok());
    }

    #[test]
    fn parses_value_forward_dcl() {
        assert!(try_parse("valuetype MyValue;").is_ok());
    }

    // -----------------------------------------------------------------
    // T4.4 — Annotations (§8.3)
    // -----------------------------------------------------------------

    #[test]
    fn parses_annotation_no_params_on_struct() {
        assert!(try_parse("@nested struct Inner { long x; };").is_ok());
    }

    #[test]
    fn parses_annotation_empty_params() {
        assert!(try_parse("@nested() struct Inner { long x; };").is_ok());
    }

    #[test]
    fn parses_annotation_on_member() {
        let src = r"
            struct Topic {
                @key long id;
                string payload;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_multiple_annotations_on_member() {
        let src = r"
            struct Topic {
                @key @id(1) long id;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_with_single_const_expr() {
        let src = r"
            struct Foo {
                @id(7) long bar;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_with_named_param() {
        let src = r"
            struct Foo {
                @range(min=0, max=100) long bar;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_with_string_value() {
        let src = r#"
            struct Foo {
                @unit("meters") long distance;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_extensibility_appendable() {
        assert!(try_parse("@appendable struct Foo { long x; };").is_ok());
    }

    #[test]
    fn parses_annotation_mutable() {
        assert!(try_parse("@mutable struct Foo { long x; };").is_ok());
    }

    #[test]
    fn parses_annotation_topic() {
        assert!(try_parse("@topic struct Sensor { @key long id; };").is_ok());
    }

    #[test]
    fn parses_annotation_on_union_case() {
        let src = r"
            union Variant switch (long) {
                case 1: @optional long val;
                default: string fallback;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_on_enumerator() {
        // OMG-Standard ist `@default_literal` (nicht `@default`, weil
        // `default` ein Union-Keyword ist und damit Lexer-Konflikt
        // erzeugt — mit Annotations vermischt waere das mehrdeutig).
        let src = r"
            enum Color {
                RED,
                @default_literal GREEN,
                BLUE
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_on_op_param() {
        let src = r"
            interface Service {
                void put(@key in long id, in string payload);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_on_attribute() {
        let src = r"
            interface Counter {
                @key readonly attribute long value;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_on_interface_op() {
        // `oneway` als Annotation kollidiert mit Op-Keyword. Stattdessen
        // realistisches Beispiel mit `@ami` (asynchronous method
        // invocation, OMG CORBA AMI).
        let src = r"
            interface Service {
                @ami void shutdown();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotation_with_scoped_name() {
        // Vendor-specific: @rti::optional — Annotation-Name ist scoped_name.
        assert!(try_parse("@rti::optional struct Foo { long x; };").is_ok());
    }

    #[test]
    fn parses_annotation_at_top_level_module() {
        assert!(try_parse("@verbatim module svc { };").is_ok());
    }

    #[test]
    fn parses_annotation_on_typedef() {
        assert!(try_parse("@key typedef long Id;").is_ok());
    }

    #[test]
    fn parses_annotation_on_const() {
        assert!(try_parse("@verbatim const long MAX = 100;").is_ok());
    }

    #[test]
    fn parses_annotation_on_exception() {
        assert!(try_parse("@verbatim exception Err {};").is_ok());
    }

    #[test]
    fn parses_complex_annotated_struct() {
        // Realistisches DDS-Topic-Beispiel.
        let src = r#"
            @topic
            @appendable
            struct SensorReading {
                @key long sensor_id;
                @id(2) double value;
                @unit("celsius") double temperature;
                @optional string label;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn rejects_annotation_without_name() {
        assert!(try_parse("@ struct Foo { long x; };").is_err());
    }

    // -----------------------------------------------------------------
    // §7.4.15 Rules 218-223 — User-Defined @annotation Declarations
    // (R-correctness fuer XTypes-konforme IDLs mit Custom-Annotations)
    // -----------------------------------------------------------------

    #[test]
    fn parses_user_annotation_empty() {
        assert!(try_parse("@annotation Foo {};").is_ok());
    }

    #[test]
    fn parses_user_annotation_with_member() {
        assert!(try_parse("@annotation Foo { string name; };").is_ok());
    }

    #[test]
    fn parses_user_annotation_with_default() {
        assert!(try_parse("@annotation Foo { long val default 42; };").is_ok());
    }

    #[test]
    fn parses_user_annotation_multi_members() {
        let src = r#"@annotation Range {
            long min default 0;
            long max default 100;
            string unit default "1";
        };"#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_user_annotation_in_module() {
        let src = r"module svc { @annotation Inner {}; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_user_annotation_with_const() {
        let src = r"@annotation Limited { const long MAX_LEN = 64; long len default 0; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_user_annotation_with_enum() {
        let src = r"@annotation Mode { enum Kind { READ, WRITE }; Kind kind default READ; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_user_annotation_then_application() {
        // Custom-Annotation definieren, dann anwenden.
        let src = r"
            @annotation MyKey {};
            @MyKey struct Topic { long id; };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn rejects_user_annotation_without_brace() {
        assert!(try_parse("@annotation Foo;").is_err());
    }

    // -----------------------------------------------------------------
    // §7.2.3.2 — Escape-Identifier `_AnIdentifier` (§1.1 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn parses_escape_identifier_module_as_typename() {
        // _module ist Identifier (Keyword `module` waere ohne _ illegal).
        assert!(try_parse("typedef long _module;").is_ok());
    }

    #[test]
    fn parses_escape_identifier_struct_as_member() {
        let src = "struct X { long _struct; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_escape_identifier_with_non_keyword_prefix() {
        // _Foo: Underscore + Non-Keyword bleibt valid Identifier.
        assert!(try_parse("struct _Foo { long y; };").is_ok());
    }

    #[test]
    fn parses_escape_identifier_interface_keyword() {
        let src = "module svc { interface _interface { void op(); }; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // §7.4.1.3 Rule 61 — Native-Type-Decl (§3.1 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn parses_native_dcl_top_level() {
        assert!(try_parse("native Handle;").is_ok());
    }

    #[test]
    fn parses_native_dcl_in_module() {
        assert!(try_parse("module sys { native Mutex; };").is_ok());
    }

    #[test]
    fn parses_native_dcl_in_interface() {
        let src = "interface Resource { native Handle; void close(in Handle h); };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // §7.4.5 Rules 100-109 — Value Types Full
    // (Recognizer akzeptiert immer; Feature-Gate enforct in `parse()`)
    // -----------------------------------------------------------------

    #[test]
    fn parses_value_def_empty() {
        assert!(try_parse("valuetype V {};").is_ok());
    }

    #[test]
    fn parses_value_def_with_state_members() {
        let src = "valuetype V { public long x; private string s; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_inheritance() {
        let src = "valuetype Derived : Base {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_truncatable_inheritance() {
        let src = "valuetype Derived : truncatable Base {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_supports() {
        let src = "valuetype V supports IFace {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_inheritance_and_supports() {
        let src = "valuetype V : Base supports IFace, IFace2 {};";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_factory() {
        let src = "valuetype V { factory create(); };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_factory_args() {
        let src = "valuetype V { factory create(in long id, in string name); };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_with_factory_raises() {
        let src = "valuetype V { exception E {}; factory create() raises (E); };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_value_def_custom() {
        assert!(try_parse("custom valuetype V {};").is_ok());
    }

    #[test]
    fn parses_value_def_with_export_op() {
        let src = "valuetype V { void op(); };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // §7.4.6 Rules 111-117 — Repository-IDs / Typeprefix / Import
    // -----------------------------------------------------------------

    #[test]
    fn parses_typeid_dcl() {
        assert!(try_parse(r#"typeid Foo "IDL:foo:1.0";"#).is_ok());
    }

    #[test]
    fn parses_typeid_with_scoped_name() {
        assert!(try_parse(r#"typeid M::Foo "IDL:M/Foo:1.0";"#).is_ok());
    }

    #[test]
    fn parses_typeprefix_dcl() {
        assert!(try_parse(r#"typeprefix Bar "company.com";"#).is_ok());
    }

    #[test]
    fn parses_import_dcl_simple() {
        assert!(try_parse("import Foo;").is_ok());
    }

    #[test]
    fn parses_import_dcl_scoped() {
        assert!(try_parse("import ::A::B::C;").is_ok());
    }

    // -----------------------------------------------------------------
    // §7.4.9 Components / Homes / Eventtypes / Ports / Connectors
    // -----------------------------------------------------------------

    #[test]
    fn parses_component_def_minimal() {
        assert!(try_parse("component C {};").is_ok());
    }

    #[test]
    fn parses_component_with_provides_uses_attr() {
        let src = r"
            component C : Base supports IFace, IFace2 {
                provides IService srv;
                uses multiple ISrc src;
                attribute long counter;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_component_forward_dcl() {
        assert!(try_parse("component C;").is_ok());
    }

    #[test]
    fn parses_home_minimal() {
        assert!(try_parse("home H manages T {};").is_ok());
    }

    #[test]
    fn parses_home_with_factory_finder() {
        let src = r"
            home H : BaseHome supports IAdmin manages MyComp primarykey K {
                factory create(in long id, in string name) raises (Bad);
                finder find(in long id);
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_eventtype_minimal() {
        assert!(try_parse("eventtype E {};").is_ok());
    }

    #[test]
    fn parses_eventtype_abstract_custom() {
        assert!(try_parse("abstract eventtype E {};").is_ok());
        assert!(try_parse("custom eventtype E { public long x; };").is_ok());
    }

    #[test]
    fn parses_porttype_with_provides_uses() {
        let src = r"
            porttype P {
                provides IService srv;
                uses ISrc src;
                port IExt ext;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_connector_with_ports() {
        let src = r"
            connector Conn : Base {
                provides IService srv;
                uses ISrc src;
                port IExt ext;
                mirrorport IMirror mir;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_template_module_typename() {
        let src = r"
            module Pair<typename T> {
                struct P { T first; T second; };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_template_module_multi_params() {
        let src = r"
            module Map<typename K, typename V, const long N> {
                struct Entry { K key; V value; };
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_template_module_typename_with_const_param() {
        // Multi-Parameter mit `const`-Form (Rule 187 alt-10).
        let src = r"
            module Buffer<typename T, const long Capacity> {
                typedef T Element;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_template_module_with_struct_param_typedef() {
        let src = r"
            module Holder<struct S> {
                typedef S Inner;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_component_with_emits_publishes_consumes() {
        let src = r"
            component Sensor {
                emits Reading evt_reading;
                publishes Alert evt_alert;
                consumes Command cmd;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_import_dcl_string() {
        assert!(try_parse(r#"import "IDL:M/Foo:1.0";"#).is_ok());
    }

    #[test]
    fn parses_value_def_full_combined() {
        let src = r"
            custom valuetype V : truncatable Base supports IFace {
                public long counter;
                private string secret;
                exception Bad {};
                factory create(in long initial) raises (Bad);
                long get_counter();
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn rejects_annotation_with_unclosed_params() {
        assert!(try_parse("@id(1 struct Foo { long x; };").is_err());
    }

    // -----------------------------------------------------------------
    // T4.7 — Struct-Inheritance (Extended Data Types BB §7.4.13)
    // -----------------------------------------------------------------

    #[test]
    fn parses_struct_with_inheritance() {
        let src = "struct Derived : Base { long x; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_struct_with_scoped_base() {
        let src = "struct Derived : ::ns::Base { long x; };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_empty_struct_with_inheritance() {
        let src = "struct Derived : Base { };";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_annotated_struct_with_inheritance() {
        // Realistisches DDS-XTypes-Beispiel.
        let src = r"
            @appendable
            struct ExtendedSensor : SensorReading {
                @id(10) double calibration_offset;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // T4.6 — Map / Bitset / Bitmask / Any
    // -----------------------------------------------------------------

    #[test]
    fn parses_map_unbounded_typedef() {
        assert!(try_parse("typedef map<string, long> Index;").is_ok());
    }

    #[test]
    fn parses_map_bounded_typedef() {
        assert!(try_parse("typedef map<string, long, 256> BoundedIndex;").is_ok());
    }

    #[test]
    fn parses_map_in_struct_member() {
        let src = r"
            struct Cache {
                map<string, long> entries;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_nested_map_without_workaround() {
        // §7.4.14.4 Note — `>>` ohne WS muss in nested templates gehen.
        // Spec-Check 2.0 Iter 6: Tokenizer split `>>` in zwei `>` weil
        // `>>` nicht in der Punct-Liste der Grammar ist.
        let src = "typedef map<string, map<long, double>> Nested;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_nested_sequence_without_workaround() {
        let src = "typedef sequence<sequence<long>> Matrix;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_triple_nested_template_without_workaround() {
        // map<K1, map<K2, sequence<long>>> — drei `>` am Stueck.
        let src = "typedef map<string, map<long, sequence<double>>> Cube;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_nested_map() {
        // map<K, map<K2,V>> — nested templates trigger >> Workaround.
        let src = "typedef map<string, map<long, double> > Nested;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_empty_bitset() {
        assert!(try_parse("bitset Flags {};").is_ok());
    }

    #[test]
    fn parses_bitset_with_bitfield() {
        let src = r"
            bitset Status {
                bitfield<3> level;
                bitfield<1> active;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_bitset_with_typed_bitfield() {
        let src = r"
            bitset Status {
                bitfield<8, octet> kind;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_bitset_with_inheritance() {
        let src = r"
            bitset Extended : Base {
                bitfield<2> extra;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_bitset_with_anonymous_padding() {
        let src = r"
            bitset Padded {
                bitfield<3> level;
                bitfield<5>;
                bitfield<1> active;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_bitmask_single_value() {
        assert!(try_parse("bitmask Permissions { READ };").is_ok());
    }

    #[test]
    fn parses_bitmask_multiple_values() {
        let src = r"
            bitmask Permissions {
                READ,
                WRITE,
                EXECUTE
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_bitmask_with_annotated_value() {
        let src = r"
            bitmask Permissions {
                @position(0) READ,
                @position(1) WRITE
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_any_in_struct_member() {
        let src = r"
            struct Generic {
                any payload;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_any_in_typedef() {
        assert!(try_parse("typedef any AnyAlias;").is_ok());
    }

    // -----------------------------------------------------------------
    // T4.5 — Extended Data Types BB Coverage-Smoke
    // §7.4.13 ist hauptsaechlich durch T4.4 (Annotations), T4.6 (Map/
    // Bitset/Bitmask) und T4.7 (Struct-Inheritance) abgedeckt. Diese
    // Tests sichern Coverage-Restpunkte.
    // -----------------------------------------------------------------

    #[test]
    fn parses_anonymous_sequence_as_struct_member() {
        // Anonymous bounded sequence direkt als type_spec eines Members.
        let src = r"
            struct Buffer {
                sequence<long, 100> samples;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_anonymous_string_as_struct_member() {
        let src = r"
            struct Person {
                string<64> name;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_extensibility_annotation_with_kind() {
        let src = r"
            @extensibility(MUTABLE)
            struct Evolvable {
                @id(1) long version;
            };
        ";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_dds_xtypes_combined() {
        // Realistisches DDS-XTypes-Pattern: extensibility + key + nested
        // struct + map + bitmask in einer Datei.
        let src = r#"
            @bit_bound(8) bitmask Permission { READ, WRITE };

            @appendable
            struct Acl {
                @key string user;
                Permission perms;
            };

            @mutable
            struct Resource {
                @key string id;
                map<string, Acl> entries;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // -----------------------------------------------------------------
    // Recognizer-Tests für alle Productions, die in
    // Phase 1 als Stubs vorgezogen wurden (alias/context/ValueBase),
    // plus Spec-Lücken-Tests fuer §7.4.1.3, §7.4.6.3, §7.4.7.3, §7.4.8.3,
    // §7.4.9.3, §7.4.10.3, §7.4.11.3, §7.4.12.3, §7.4.13.3, §7.4.14.3,
    // §7.4.15.3.
    // -----------------------------------------------------------------

    // Phase 3.1 — §7.4.1.3 Rule (6) Const-Type-Varianten

    #[test]
    fn parses_octet_const() {
        assert!(try_parse("const octet O = 42;").is_ok());
    }

    #[test]
    fn parses_char_const() {
        assert!(try_parse("const char C = 'A';").is_ok());
    }

    #[test]
    fn parses_wchar_const() {
        assert!(try_parse("const wchar W = L'A';").is_ok());
    }

    #[test]
    fn parses_wstring_const() {
        assert!(try_parse(r#"const wstring S = L"hello";"#).is_ok());
    }

    #[test]
    fn parses_fixed_const() {
        assert!(try_parse("const fixed F = 1.5d;").is_ok());
    }

    // Phase 3.2 — §7.4.1.3 Rule (13) Modulo-Operator

    #[test]
    fn parses_const_modulo() {
        assert!(try_parse("const long X = 7 % 3;").is_ok());
    }

    // Phase 3.3 — §7.4.1.3 Rule (15) Unary-+

    #[test]
    fn parses_const_unary_plus() {
        assert!(try_parse("const long X = +5;").is_ok());
    }

    // Phase 3.4 — §7.4.1.3 Rule (17) Literal-Klassen-Tests

    #[test]
    fn parses_const_with_fixed_pt_literal() {
        assert!(try_parse("const fixed F = 1.5d;").is_ok());
    }

    #[test]
    fn parses_const_with_char_literal() {
        assert!(try_parse("const char C = 'A';").is_ok());
    }

    #[test]
    fn parses_const_with_wide_char_literal() {
        assert!(try_parse("const wchar W = L'A';").is_ok());
    }

    #[test]
    fn parses_const_with_wide_string_literal() {
        assert!(try_parse(r#"const wstring W = L"x";"#).is_ok());
    }

    // Phase 3.5 — §7.4.1.3 Rule (24) long double

    #[test]
    fn parses_long_double_typedef() {
        assert!(try_parse("typedef long double LD;").is_ok());
    }

    // Phase 3.6 — §7.4.1.3 Rule (38) wstring als Template

    #[test]
    fn parses_wide_string_typedef() {
        assert!(try_parse("typedef wstring<10> WS;").is_ok());
    }

    // Phase 3.7 — §7.4.1.3 Rule (51) Scoped-Name als Switch-Type

    #[test]
    fn parses_union_with_scoped_name_discriminator() {
        let src = r#"
            enum E { A, B };
            union U switch (E) {
                case A: long x;
                case B: long y;
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // Phase 3.8 — §7.4.1.3 Rule (64) Inline-Constr-Type-Typedef

    #[test]
    fn parses_typedef_with_inline_struct() {
        // Spec §7.4.1.3 Rule 64: <type_declarator> ::= { <simple_type_spec>
        // | <template_type_spec> | <constr_type_dcl> } <any_declarators>.
        // constr_type_dcl = struct_dcl | union_dcl | enum_dcl; struct_dcl
        // verlangt Bezeichner (Foo) — anonymous struct ist nicht
        // spec-konform.
        let src = "typedef struct Foo { long x; } Alias;";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r64: typedef-mit-inline-struct nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.9 — §7.4.6.3 Rule (112) typeid/typeprefix/import als
    // Export im Interface

    #[test]
    fn parses_typeid_inside_interface() {
        let src = r#"
            interface I {
                typeid I "IDL:I:1.0";
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r112: typeid-als-export nicht akzeptiert: {r:?}"
        );
    }

    #[test]
    fn parses_typeprefix_inside_module_with_interface() {
        let src = r#"
            module M {
                interface I {};
                typeprefix M "example.com";
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r112: typeprefix-im-module nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.10 — §7.4.6.3 Rule (117) Object-als-Type

    #[test]
    fn parses_typedef_object_type() {
        let src = "typedef Object MyObj;";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r117: Object-als-base_type_spec nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.11 — §7.4.7.3 Rule (127) Abstract Value Type

    #[test]
    fn parses_abstract_value_type() {
        let src = r#"
            abstract valuetype AV {
                void op();
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r127: abstract valuetype nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.12 — §7.4.8.3 Rule (138) Component-Inheritance

    #[test]
    fn parses_component_with_inheritance() {
        let src = r#"
            component A {};
            component B : A {};
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // Phase 3.13 — §7.4.9.3 Rule (147) Home-Inheritance

    #[test]
    fn parses_home_with_inheritance() {
        let src = r#"
            component C {};
            home A manages C {};
            home B : A manages C {};
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // Phase 3.14 — §7.4.10.3 Rule (154) Component-Supports

    #[test]
    fn parses_component_with_supports() {
        let src = r#"
            interface I {};
            component C supports I {};
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r154: component-supports nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.15 — §7.4.10.3 Rule (157) Object-iface-type

    #[test]
    fn parses_component_provides_object() {
        let src = r#"
            component C {
                provides Object myObj;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r157: provides-Object nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.16 — §7.4.10.3 Rule (158) uses multiple

    #[test]
    fn parses_component_with_multiple_uses() {
        let src = r#"
            interface I {};
            component C {
                uses multiple I rec;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r158: uses-multiple nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.17 — §7.4.10.3 Rule (162) Home-Supports+PrimaryKey

    #[test]
    fn parses_home_with_supports_and_primary_key() {
        let src = r#"
            interface I {};
            valuetype K { };
            component C {};
            home H supports I manages C primarykey K {};
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r162: home-supports+primarykey nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.18 — §7.4.10.3 Rule (167) Event-Forward

    #[test]
    fn parses_eventtype_forward_dcl() {
        let src = r#"
            eventtype E;
            eventtype E {};
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r167: eventtype forward nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.19 — §7.4.11.3 Rule (173) Porttype-Forward

    #[test]
    fn parses_porttype_forward_dcl() {
        let src = r#"
            interface I {};
            porttype P;
            porttype P {
                provides I p;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r173: porttype forward nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.20 — §7.4.11.3 Rule (178) Port/Mirrorport

    #[test]
    fn parses_component_with_port_dcl() {
        let src = r#"
            interface I {};
            porttype P {
                provides I p;
            };
            component C {
                port P prt;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r178: port-dcl nicht akzeptiert: {r:?}"
        );
    }

    #[test]
    fn parses_component_with_mirrorport_dcl() {
        let src = r#"
            interface I {};
            porttype P {
                provides I p;
            };
            component C {
                mirrorport P prt;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r178: mirrorport-dcl nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.21 — §7.4.11.3 Rule (182) Connector-Inheritance

    #[test]
    fn parses_connector_with_inheritance() {
        let src = r#"
            interface I {};
            connector A {
                provides I p1;
            };
            connector B : A {
                provides I p2;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r182: connector-inheritance nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.22 — §7.4.12.3 Rule (188) formal_param_type-Varianten

    #[test]
    fn parses_template_module_with_interface_param() {
        let src = "module M<interface T> { typedef T A; };";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r188: interface-formal-param: {r:?}"
        );
    }

    #[test]
    fn parses_template_module_with_valuetype_param() {
        let src = "module M<valuetype T> { typedef T A; };";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r188: valuetype-formal-param: {r:?}"
        );
    }

    #[test]
    fn parses_template_module_with_eventtype_param() {
        let src = "module M<eventtype T> { typedef T A; };";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r188: eventtype-formal-param: {r:?}"
        );
    }

    #[test]
    fn parses_template_module_with_union_param() {
        let src = "module M<union T> { typedef T A; };";
        let r = try_parse(src);
        assert!(r.is_ok(), "Erkannte Lücke r188: union-formal-param: {r:?}");
    }

    #[test]
    fn parses_template_module_with_exception_param() {
        let src = "module M<exception T> { typedef T A; };";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r188: exception-formal-param: {r:?}"
        );
    }

    #[test]
    fn parses_template_module_with_enum_param() {
        let src = "module M<enum T> { typedef T A; };";
        let r = try_parse(src);
        assert!(r.is_ok(), "Erkannte Lücke r188: enum-formal-param: {r:?}");
    }

    #[test]
    fn parses_template_module_with_sequence_param() {
        let src = "module M<sequence T> { typedef T A; };";
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r188: sequence-formal-param: {r:?}"
        );
    }

    // Phase 3.23 — §7.4.12.3 Rule (190) Template-Module-Inst

    #[test]
    fn parses_template_module_instantiation() {
        let src = r#"
            module M<typename T> { typedef T A; };
            module N { module M<long> M_long; };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r190: template_module_inst nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.24 — §7.4.12.3 Rule (193) Template-Module-Alias

    #[test]
    fn parses_template_module_with_alias_ref() {
        // Spec §7.4.12 Rule 187: <formal_parameter_type> akzeptiert nur
        // typename/interface/valuetype/eventtype/struct/union/exception/
        // enum/sequence/const <const_type> — `long` allein ist NICHT
        // erlaubt (muss "const long" sein).
        // Rule 193: <template_module_ref> ::= "alias" <scoped_name>
        // "<" <formal_parameter_names> ">" <identifier>.
        let src = r#"
            module Inner<typename T, struct S, const long n> {
                typedef T Alias;
            };
            module Outer<typename T1, typename T2, struct S1, struct S2, const long m> {
                alias Inner<T2, S2, m> InnerAlias;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r193: alias-ref nicht akzeptiert (Composer-Hook fehlt): {r:?}"
        );
    }

    // Phase 3.25 — §7.4.13.3 Rule (195) Struct-Inheritance/Empty-Body

    #[test]
    fn parses_struct_with_single_inheritance() {
        let src = r#"
            struct Base { long x; };
            struct Derived : Base { long y; };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_empty_struct_with_void_body() {
        // Spec §7.4.13.3 (195): "struct" <identifier> "{" "}".
        let src = "struct Empty {};";
        // Wird via OneOrMore-Constraint potentiell rejected; mit
        // Extended-Data-Types-Composer-Erweiterung erlaubt.
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r195: empty-struct nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.26 — §7.4.13.3 Rule (196) Wchar/Octet Switch

    #[test]
    fn parses_union_with_wchar_discriminator() {
        let src = r#"
            union U switch (wchar) {
                case L'A': long x;
                default: long y;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r196: wchar-discriminator nicht akzeptiert: {r:?}"
        );
    }

    #[test]
    fn parses_union_with_octet_discriminator() {
        let src = r#"
            union U switch (octet) {
                case 1: long x;
                default: long y;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r196: octet-discriminator nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.27 — §7.4.14.3 Rule (217) Anonymous-Array-In-Struct

    #[test]
    fn parses_anonymous_array_as_struct_member() {
        let src = r#"
            struct S { long arr[10]; };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r217: anonymous-array-in-struct nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.28 — §7.4.15.3 Rule (218) Custom-Annotation-Decl

    #[test]
    fn parses_custom_annotation_dcl() {
        let src = r#"
            @annotation MyAnno {
                string value default "";
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke r218: custom-annotation-decl nicht akzeptiert: {r:?}"
        );
    }

    // Phase 3.29 — §7.4.3.4.3.3.2 Native-Attr

    #[test]
    fn parses_attr_with_native_type() {
        let src = r#"
            interface I {
                native MyHandle;
                attribute MyHandle h_attr;
            };
        "#;
        let r = try_parse(src);
        assert!(
            r.is_ok(),
            "Erkannte Lücke 7.4.3.4.3.3.2: native-attr nicht akzeptiert: {r:?}"
        );
    }

    // Phase 5 — §7.4.6.3 Rule (117) Object-Type
    #[test]
    fn parses_object_as_base_type_spec() {
        // Spec §7.4.6.3 (117): `<base_type_spec> ::+ <object_type>`
        // Object ist ein gueltiger base_type_spec im CORBA-Profil.
        let src = "typedef Object MyObj;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // Phase 5 — §7.4.7.3 Rule (131) ValueBase als base_type_spec
    #[test]
    fn parses_value_base_as_base_type_spec() {
        // Spec §7.4.7.3 (131): `<base_type_spec> ::+ <value_base_type>`
        // ValueBase ist ein gueltiger base_type_spec im CORBA-Profil.
        let src = "typedef ValueBase MyVal;";
        assert!(try_parse(src).is_ok(), "{src}");
    }

    // K1.9 Coverage-Audit — §7.4.6.3 r123/r124 op_with_context / context_expr
    #[test]
    fn parses_op_with_context() {
        // Spec §7.4.6.3 (123): `<op_with_context> ::= {<op_dcl> |
        // <op_oneway_dcl>} <context_expr>`. Plus Rule (124):
        // `<context_expr> ::= "context" "(" <string_literal>
        // {"," <string_literal>}* ")"`.
        let src = r#"
            interface I {
                void op() context ("X");
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_op_with_context_multiple_strings() {
        // Multiple context-strings.
        let src = r#"
            interface I {
                long op(in long arg) context ("CTX1", "CTX2", "CTX3.*");
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }

    #[test]
    fn parses_oneway_op_with_context() {
        // oneway op + context.
        let src = r#"
            interface I {
                oneway void notify() context ("topic");
            };
        "#;
        assert!(try_parse(src).is_ok(), "{src}");
    }
}
