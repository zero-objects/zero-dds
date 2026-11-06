// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rules of OMG IDL 4.2 (formal/18-01-05) that the front end enforces, each
//! held to the example or the sentence of the clause it names.
//!
//! Every rule has a source that trips it and one that does not: a check
//! that fires on everything is as useless as one that never fires.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, ParamDirection};
use zerodds_idl_lower::lower;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, parse};

fn lowered(source: &str) -> (Ast, Vec<Diagnostic>) {
    let parsed = parse(source);
    let mut diagnostics = parsed.diagnostics().to_vec();
    let lowered = lower(&parsed.syntax(), "test.idl");
    diagnostics.extend(lowered.diagnostics);
    (lowered.ast, diagnostics)
}

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    lowered(source).1
}

fn assert_trips(source: &str, code: DiagnosticCode) -> Vec<Diagnostic> {
    let found = diagnostics(source);
    assert!(
        found.iter().any(|d| d.code == code),
        "{source:?} did not trip {}: {found:?}",
        code.as_str()
    );
    found
}

fn assert_clean(source: &str) {
    let found = diagnostics(source);
    assert!(found.is_empty(), "{source:?} reported {found:?}");
}

// -- §7.4.13.4.3.3 Bit Masks ------------------------------------------------

/// The clause's own example: `flagx` follows `flag3` at position 3 and so
/// takes position 4, which `flag4` already has — "ERROR, should be 0x01 <<4
/// but duplicates flag4". The same values without `flagx` are unordered and
/// legal ("annotated bit values can be declared unordered").
#[test]
fn idl_7_4_13_4_3_3_a_bit_position_taken_twice_is_refused() {
    let found = assert_trips(
        "bitmask MyBitMask {
             @position(0) flag0,
             flag1,
             @position(4) flag4,
             @position(2) flag2,
             flag3,
             flagx
         };",
        DiagnosticCode::DuplicateMemberId,
    );
    let duplicate = found
        .iter()
        .find(|d| d.code == DiagnosticCode::DuplicateMemberId)
        .unwrap();
    assert!(
        duplicate.message.contains("flagx") && duplicate.message.contains("flag4"),
        "{duplicate:?}"
    );
    assert_trips(
        "bitmask B { @position(1) A, @position(1) C };",
        DiagnosticCode::DuplicateMemberId,
    );
    assert_clean(
        "bitmask MyBitMask {
             @position(0) flag0,
             flag1,
             @position(4) flag4,
             @position(2) flag2,
             flag3
         };",
    );
}

/// "Possible positions range from 0 ... up to (size – 1)": a value that
/// follows the last position of an `@bit_bound(8)` mask has none.
#[test]
fn idl_7_4_13_4_3_3_a_following_bit_value_past_the_bound_is_refused() {
    assert_trips(
        "@bit_bound(8) bitmask B { @position(7) last, beyond };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("@bit_bound(8) bitmask B { @position(6) a, b };");
}

// -- §7.4.1.4.3 Constants, Table 7-26 ----------------------------------------

/// The two examples the clause gives — `const short s = 655592;` is too
/// large, `const octet o = -54;` is inappropriate — and the ends of the
/// ranges Table 7-26 states for each integer type.
#[test]
fn idl_7_4_1_4_3_a_constant_outside_its_types_range_is_refused() {
    for source in [
        "const short s = 655592;",
        "const octet o = -54;",
        "const octet o = 300;",
        "const int8 i = 200;",
        "const int8 i = -129;",
        "const uint8 u = 256;",
        "const unsigned short u = 65536;",
        "const unsigned short u = -1;",
        "const long l = 2147483648;",
        "const unsigned long u = 4294967296;",
        "const long long l = 9223372036854775808;",
        "const unsigned long long u = -1;",
        "typedef short Small; const Small s = 40000;",
        "const long base = 30000; const short s = base * 2;",
    ] {
        assert_trips(source, DiagnosticCode::TypeMismatch);
    }
    for source in [
        "const short s = -32768;",
        "const short s = 32767;",
        "const octet o = 255;",
        "const octet o = 0;",
        "const int8 i = -128;",
        "const int8 i = 127;",
        "const uint8 u = 255;",
        "const unsigned short u = 65535;",
        "const long l = -2147483648;",
        "const unsigned long u = 4294967295;",
        "const long long l = -9223372036854775808;",
        "const unsigned long long u = 18446744073709551615;",
        "typedef short Small; const Small s = 32767;",
    ] {
        assert_clean(source);
    }
}

// -- §8.3.3.2 @range --------------------------------------------------------

/// "The max value must be greater or equal to the min value."
#[test]
fn idl_8_3_3_2_a_range_whose_max_is_below_its_min_is_refused() {
    assert_trips(
        "struct S { @range(min = 5, max = 1) long a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "struct S { @range(5, 1) long a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "struct S { @min(2.5) @max(1.5) double a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "@range(min = 20, max = 10) typedef long MyLong;",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("struct S { @range(min = 1, max = 1) long a; };");
    assert_clean("struct S { @range(min = 10, max = 20) long a; };");
    assert_clean("@range(min = 10, max = 20) typedef long MyLong; struct Foo { MyLong bar2; };");
}

// -- §8.3.6.2 @oneway --------------------------------------------------------

/// "This annotation may only concern operations without any return value
/// (void return type) and without any out or inout parameters", and
/// `boolean value default TRUE`: `@oneway(FALSE)` leaves an operation
/// two-way.
#[test]
fn idl_8_3_6_2_a_oneway_operation_returns_nothing_and_takes_in_parameters_only() {
    let (ast, found) = lowered("interface I { @oneway void notify(in long x); };");
    assert!(found.is_empty(), "{found:?}");
    let (_, notify) = ast.operations().next().unwrap();
    assert!(notify.oneway, "the annotation form sets the flag");
    assert_eq!(
        ast.param(notify.params[0]).unwrap().direction,
        ParamDirection::In
    );

    let (ast, found) = lowered("interface I { @oneway(FALSE) void call(in long x); };");
    assert!(found.is_empty(), "{found:?}");
    assert!(!ast.operations().next().unwrap().1.oneway);

    assert_trips(
        "interface I { @oneway long ask(in long x); };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "interface I { @oneway void fill(out long x); };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "interface I { @oneway void bump(inout long x); };",
        DiagnosticCode::InvalidAnnotation,
    );
}

// -- §7.4.3.4.3.2.1 Interface inheritance rules --------------------------------

/// Figure 7-1's diamond and `E: A, B` are legal; naming `A` twice as a direct
/// base is not, nor redefining `make_it_so` (the clause's own example), nor
/// inheriting two different operations of one name.
#[test]
fn idl_7_4_3_4_3_2_1_a_direct_base_is_named_once_and_no_operation_is_redefined() {
    assert_clean(
        "interface A { void op(); };
         interface B: A { };
         interface C: A { };
         interface D: B, C { };
         interface E: A, B { };",
    );
    assert_trips(
        "interface A { }; interface F: A, A { };",
        DiagnosticCode::InvalidInheritance,
    );
    assert_trips(
        "interface A { void make_it_so(); };
         interface B: A { short make_it_so(in long times); };",
        DiagnosticCode::InvalidInheritance,
    );
    assert_trips(
        "interface A { attribute long level; };
         interface B: A { void level(); };",
        DiagnosticCode::InvalidInheritance,
    );
    assert_trips(
        "interface P { void op(); };
         interface Q { void op(); };
         interface R: P, Q { };",
        DiagnosticCode::InvalidInheritance,
    );
}

// -- §7.4.4.4 / §7.5.1 Ambiguity ----------------------------------------------

/// §7.4.4.4's example: in `C: B, A` the unqualified `L1` is ambiguous,
/// `A::L1` and `B::L1` are not, and each resolves to its own declaration.
#[test]
fn idl_7_4_4_4_a_name_declared_in_two_bases_is_ambiguous_unless_qualified() {
    let source = "interface A {
                      typedef long L1;
                      short opA (in L1 l_1);
                  };
                  interface B {
                      typedef short L1;
                      L1 opB (in long l);
                  };
                  interface C: B, A {
                      typedef L1 L2;
                      typedef A::L1 L3;
                      B::L1 opC (in L3 l_3);
                  };";
    let (ast, found) = lowered(source);
    let ambiguous: Vec<_> = found
        .iter()
        .filter(|d| d.code == DiagnosticCode::AmbiguousName)
        .collect();
    assert_eq!(ambiguous.len(), 1, "{found:?}");
    assert!(ambiguous[0].message.contains("L1"), "{ambiguous:?}");
    assert_eq!(found.len(), 1, "only the one ambiguity: {found:?}");
    assert_eq!(alias_target(&ast, "L3"), Some("long"));
    let (_, op_c) = ast.operations().find(|(_, op)| op.name == "opC").unwrap();
    let returned = op_c
        .return_type
        .as_ref()
        .and_then(|r| r.resolved())
        .unwrap();
    assert_eq!(primitive_through_aliases(&ast, returned), Some("short"));
}

/// §7.5.1: `attribute string_t Title;` in `C: A, B` is ambiguous;
/// `A::string_t` and `B::string_t` are each fine.
#[test]
fn idl_7_5_1_an_attribute_typed_by_an_ambiguous_name_is_an_error() {
    let found = diagnostics(
        "interface A { typedef string<128> string_t; };
         interface B { typedef string<256> string_t; };
         interface C: A, B {
             attribute string_t   Title;
             attribute A::string_t Name;
             attribute B::string_t City;
         };",
    );
    let ambiguous = found
        .iter()
        .filter(|d| d.code == DiagnosticCode::AmbiguousName)
        .count();
    assert_eq!(ambiguous, 1, "{found:?}");
    assert_eq!(found.len(), 1, "{found:?}");
}

/// §7.5.1: "Inheritance causes all identifiers defined in base interfaces,
/// both direct and indirect, to be visible in derived interfaces";
/// "Multiple paths to the same original identifier … do not conflict"; and
/// the exception `E` of `A` is known by `::A::E` and `::B::E`.
#[test]
fn idl_7_5_1_an_inherited_identifier_is_visible_under_every_global_name() {
    let (ast, found) = lowered(
        "interface A {
             exception E { long L; };
             void f () raises(E);
         };
         interface B: A {
             void g () raises(E);
             void h () raises(::B::E);
         };
         interface C: A { };
         interface D: B, C {
             void k () raises(E);
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (e, _) = ast.exceptions().find(|(_, node)| node.name == "E").unwrap();
    for name in ["f", "g", "h", "k"] {
        let (_, op) = ast.operations().find(|(_, op)| op.name == name).unwrap();
        assert_eq!(op.raises.len(), 1, "{name}");
        assert_eq!(op.raises[0].resolved(), Some(e), "{name} raises A::E");
    }
}

// -- §7.5.2 Scoping rules and name resolution ---------------------------------

/// The clause's example: `ArgType` on line l5, in `N::Y : M::B`, is found in
/// step 2, the inherited scope `M::B`, and is `string` — not the `char` of
/// `N` (step 3). On line l2, `AType` is `long` and `ArgType` is `string`.
#[test]
fn idl_7_5_2_a_name_is_searched_in_the_inherited_scope_before_the_enclosing_one() {
    let (ast, found) = lowered(
        "module M {
             typedef long ArgType;
             typedef ArgType AType;
             interface B {
                 typedef string ArgType;
                 ArgType opb(in AType i);
             };
         };
         module N {
             typedef char ArgType;
             interface Y : M::B {
                 void opy(in ArgType i);
             };
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let param_kind = |op: &str| {
        let (_, node) = ast.operations().find(|(_, o)| o.name == op).unwrap();
        let param = ast.param(node.params[0]).unwrap();
        type_word(&ast, param.type_.resolved().unwrap())
    };
    assert_eq!(param_kind("opy"), "string", "line l5");
    assert_eq!(param_kind("opb"), "long", "AType on line l2");
    let (_, opb) = ast.operations().find(|(_, o)| o.name == "opb").unwrap();
    let returned = opb.return_type.as_ref().and_then(|r| r.resolved()).unwrap();
    assert_eq!(type_word(&ast, returned), "string", "ArgType on line l2");
}

/// "An identifier from a surrounding scope is introduced into a scope if it
/// is used in that scope. An identifier is not introduced into a scope by
/// merely being visible in that scope." The clause's example, both ways.
#[test]
fn idl_7_5_2_a_used_identifier_is_introduced_and_a_visible_one_is_not() {
    assert_clean(
        "module M {
             module Inner1 { typedef string S1; };
             module Inner2 { typedef string inner1; };
         };",
    );
    let found = assert_trips(
        "module M {
             module Inner1 { typedef string S1; };
             module Inner2 {
                 typedef Inner1::S1 S2;
                 typedef string inner1;
                 typedef string S1;
             };
         };",
        DiagnosticCode::DuplicateName,
    );
    let duplicates: Vec<_> = found
        .iter()
        .filter(|d| d.code == DiagnosticCode::DuplicateName)
        .collect();
    assert_eq!(
        duplicates.len(),
        1,
        "only inner1, S1 is not introduced: {found:?}"
    );
    assert!(duplicates[0].message.contains("inner1"), "{duplicates:?}");
}

/// "Only the first identifier in a qualified name is introduced into the
/// current scope … A qualified name of the form ::X::Y::Z does not cause X
/// to be introduced, but a qualified name of the form X::Y::Z does."
#[test]
fn idl_7_5_2_only_the_first_identifier_of_a_relative_name_is_introduced() {
    assert_clean(
        "module X { module Y { typedef long Z; }; };
         module W {
             typedef ::X::Y::Z A;
             typedef string x;
         };",
    );
    assert_trips(
        "module X { module Y { typedef long Z; }; };
         module W {
             typedef X::Y::Z A;
             typedef string x;
         };",
        DiagnosticCode::DuplicateName,
    );
    assert_clean(
        "module X { module Y { typedef long Z; }; };
         module W {
             typedef X::Y::Z A;
             typedef string y;
             typedef string z;
         };",
    );
}

// -- §7.5.3 Special scoping rules for type names -------------------------------

/// "Once a type has been defined anywhere within the scope of a module,
/// interface or value type, it may not be redefined except within the scope
/// of a nested module, interface or value type, or within the scope of a
/// derived interface or value type." The derived interface `B` of the
/// clause's example redefines `ArgType` and `S`, and `A::ArgType` still
/// names the base's.
#[test]
fn idl_7_5_3_a_derived_interface_may_redefine_a_type_of_its_base() {
    let (ast, found) = lowered(
        "module M {
             interface A {
                 typedef string ArgType;
                 struct S { long m; };
             };
             interface B : A {
                 typedef long ArgType;
                 struct S {
                     ArgType x;
                     A::ArgType y;
                 };
             };
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast
        .types()
        .filter(|(_, t)| t.name == "S")
        .find(|(id, _)| ast.members_of(*id).len() == 2)
        .unwrap();
    let members = ast.members_of(s);
    let x = ast.member(members[0]).unwrap().type_.resolved().unwrap();
    let y = ast.member(members[1]).unwrap().type_.resolved().unwrap();
    assert_eq!(type_word(&ast, x), "long", "x is a long");
    assert_eq!(type_word(&ast, y), "string", "y is a string");
    // Redefining in the same scope stays an error.
    assert_trips(
        "module M { typedef long T; typedef short T; };",
        DiagnosticCode::DuplicateName,
    );
}

/// The potential scope: `ArgType` and `I` used in `M::A::S::T` are
/// introduced into `T`, `S` and `A`, so redefining either in `A` is an
/// error; `typedef short Y;` is fine (the member `y` uses nothing), and so is
/// `long I;` as a member of `U`, which is not a type name. The clause's last
/// note: a redefinition after a use inside a structure in a module is fine.
#[test]
fn idl_7_5_3_a_type_may_not_be_redefined_within_its_potential_scope() {
    let found = assert_trips(
        "module M {
             typedef long ArgType;
             const long I = 10;
             typedef short Y;
             interface A {
                 struct S {
                     ArgType x[I];
                     long y;
                 };
                 typedef string ArgType;
                 enum I { I1, I2 };
                 typedef short Y;
             };
         };",
        DiagnosticCode::DuplicateName,
    );
    let duplicates: Vec<_> = found
        .iter()
        .filter(|d| d.code == DiagnosticCode::DuplicateName)
        .collect();
    assert_eq!(duplicates.len(), 2, "ArgType and I: {found:?}");
    assert!(duplicates.iter().any(|d| d.message.contains("ArgType")));
    assert!(duplicates.iter().any(|d| d.message.contains("`I`")));
    assert_clean(
        "module M {
             typedef long ArgType;
             const long I = 10;
             interface A {
                 struct S { ArgType x[I]; };
                 struct U { long I; };
             };
         };",
    );
    let (ast, found) = lowered(
        "typedef long ArgType;
         module M {
             struct S { ArgType x; };
             typedef string ArgType;
             struct T { ArgType y; };
         };",
    );
    assert!(
        found.is_empty(),
        "redefinition after use in a module is OK: {found:?}"
    );
    let member_word = |name: &str| {
        let (id, _) = ast.types().find(|(_, t)| t.name == name).unwrap();
        let member = ast.member(ast.members_of(id)[0]).unwrap();
        type_word(&ast, member.type_.resolved().unwrap())
    };
    assert_eq!(member_word("S"), "long", "x is a long");
    assert_eq!(member_word("T"), "string", "y is a string");
}

// -- helpers ------------------------------------------------------------------

/// The keyword of the type an alias chain ends at: a primitive's, or
/// `string`.
fn type_word(ast: &Ast, id: zerodds_ast_core::Id<zerodds_ast::TypeNode>) -> &'static str {
    match primitive_through_aliases(ast, id) {
        Some(word) => word,
        None => "string",
    }
}

fn primitive_through_aliases(
    ast: &Ast,
    mut id: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
) -> Option<&'static str> {
    for _ in 0..16 {
        match &ast.type_(id).ok()?.kind {
            zerodds_ast::TypeKind::Alias { aliased } => id = aliased.resolved()?,
            zerodds_ast::TypeKind::Primitive(kind) => return Some(kind.as_str()),
            zerodds_ast::TypeKind::String { .. } => return None,
            _ => return Some("other"),
        }
    }
    None
}

fn alias_target(ast: &Ast, name: &str) -> Option<&'static str> {
    let (id, _) = ast.types().find(|(_, t)| t.name == name)?;
    primitive_through_aliases(ast, id)
}

// -- §7.4.7.4.3 Value inheritance rules, Table 7-19 -----------------------------

/// The clause's example — `V3: V1, V2 supports I3` is legal and `V4: V1
/// supports I2` is not, because `I2` does not derive from `I1`, which `V4`
/// supports through `V1` — and the other cells of Table 7-19.
#[test]
fn idl_7_4_7_4_3_a_value_type_has_one_concrete_base_first_and_supports_one_interface() {
    let example = "interface I1 { };
                   interface I2 { };
                   interface I3: I1, I2 { };
                   abstract valuetype V1 supports I1 { };
                   abstract valuetype V2 supports I2 { };
                   valuetype V3: V1, V2 supports I3 { };";
    assert_clean(example);
    assert_trips(
        &format!("{example} valuetype V4: V1 supports I2 {{ }};"),
        DiagnosticCode::InvalidInheritance,
    );
    for illegal in [
        // a single concrete base
        "valuetype A { public long state_a; }; valuetype B { public long state_b; };
         valuetype C : A, B { public long state_c; };",
        // and it comes first
        "abstract valuetype X { }; valuetype A { public long state_a; };
         valuetype C : X, A { public long state_c; };",
        // an abstract value type inherits abstract value types only
        "valuetype A { public long state_a; }; abstract valuetype X : A { };",
        // one interface that is not abstract
        "interface I { }; interface J { }; valuetype V supports I, J { public long state_a; };",
        // a boxed value type is no base
        "valuetype VB long; valuetype V : VB { public long state_a; };",
        // an abstract interface inherits abstract interfaces only
        "interface I { }; abstract interface J : I { };",
    ] {
        assert_trips(illegal, DiagnosticCode::InvalidInheritance);
    }
    for legal in [
        "abstract interface I { }; abstract interface J { };
         valuetype V supports I, J { public long state_a; };",
        "abstract valuetype X { }; valuetype A { public long state_a; };
         valuetype C : A, X { public long state_c; };",
        "abstract interface I { }; interface J : I { };",
    ] {
        assert_clean(legal);
    }
}

/// "truncatable may not be specified for a custom value type"; truncation
/// is for "A stateful value that derives from another stateful value"; and
/// "Non-custom value types may not (transitively) inherit from custom value
/// types."
#[test]
fn idl_7_4_7_4_5_truncatable_needs_a_stateful_base_and_custom_stays_custom() {
    assert_clean(
        "valuetype A { public long state_a; }; valuetype B : truncatable A { public long state_b; };",
    );
    assert_clean(
        "custom valuetype A { public long state_a; }; custom valuetype B : A { public long state_b; };",
    );
    for illegal in [
        "valuetype A { public long state_a; };
         custom valuetype B : truncatable A { public long state_b; };",
        "abstract valuetype X { }; valuetype B : truncatable X { public long state_b; };",
        "custom valuetype A { public long state_a; }; valuetype B : A { public long state_b; };",
        "custom valuetype A { public long state_a; }; custom valuetype B : A { public long state_b; };
         valuetype C : B { public long state_c; };",
    ] {
        assert_trips(illegal, DiagnosticCode::InvalidInheritance);
    }
}

// -- §7.2.6.2.2 Escape sequences ----------------------------------------------

/// "The \u escape is valid only with wchar and wstring types … Attempts to
/// set a char type to a \u defined literal or a string type to a sequence of
/// \u literals result in an error."
#[test]
fn idl_7_2_6_2_2_a_unicode_escape_is_refused_in_a_narrow_literal() {
    assert_trips("const char c = '\\u0041';", DiagnosticCode::TypeMismatch);
    assert_trips(
        "const string s = \"\\u0041\\u0042\";",
        DiagnosticCode::TypeMismatch,
    );
    assert_clean("const wchar c = L'\\u0041';");
    assert_clean("const wstring s = L\"\\u0041\\u0042\";");
    // An escaped backslash before a `u` is no escape of the letter.
    assert_clean("const string s = \"\\\\u0041\";");
}

// -- §7.4.1.3, §7.4.1.4.4.6 Native types --------------------------------------

/// Rule (61) declares a native type; "This declaration defines a new type
/// with the specified name", which then stands wherever a type is written,
/// by that name: as a member, through a `typedef`, as a parameter of a local
/// interface.
#[test]
fn idl_7_4_1_4_4_6_a_native_type_is_used_by_its_name_wherever_a_type_is() {
    let (ast, found) = lowered(
        "native Handle;
         typedef Handle Wrapped;
         struct S { Handle h; Wrapped w; };
         local interface L { Handle make(in Handle from); };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast.types().find(|(_, t)| t.name == "S").unwrap();
    for member in ast.members_of(s) {
        let target = ast.member(*member).unwrap().type_.resolved().unwrap();
        let mut kind = &ast.type_(target).unwrap().kind;
        if let zerodds_ast::TypeKind::Alias { aliased } = kind {
            kind = &ast.type_(aliased.resolved().unwrap()).unwrap().kind;
        }
        assert!(matches!(kind, zerodds_ast::TypeKind::Native), "{kind:?}");
    }
}

/// §7.4.6.4.4: "native type parameters are not permitted in operations of
/// remote interfaces"; a local interface (§7.4.6.4.3) is not remote.
#[test]
fn idl_7_4_6_4_4_a_native_parameter_is_refused_in_a_remote_interface() {
    assert_clean("native H; local interface L { void use(in H handle); };");
    assert_trips(
        "native H; interface R { void use(in H handle); };",
        DiagnosticCode::TypeMismatch,
    );
    assert_trips(
        "native H; typedef H Wrapped; abstract interface R { void use(in Wrapped handle); };",
        DiagnosticCode::TypeMismatch,
    );
}

// -- §7.4.3.4.3.3 Interface body -----------------------------------------------

/// Rules (82) to (87) through the front end: the three parameter directions,
/// a `raises` clause naming an exception, and the `context` clause of
/// §7.4.6.4.6 with its strings in order.
#[test]
fn idl_7_4_3_4_3_3_1_an_operation_keeps_its_directions_raises_and_context() {
    let (ast, found) = lowered(
        "interface I {
             exception Failed { long code; };
             long work(in long a, out short b, inout string c)
                 raises (Failed) context (\"user\", \"sys*\");
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (_, work) = ast.operations().find(|(_, o)| o.name == "work").unwrap();
    let directions: Vec<ParamDirection> = work
        .params
        .iter()
        .map(|p| ast.param(*p).unwrap().direction)
        .collect();
    assert_eq!(
        directions,
        [
            ParamDirection::In,
            ParamDirection::Out,
            ParamDirection::InOut
        ]
    );
    let (failed, _) = ast.exceptions().find(|(_, e)| e.name == "Failed").unwrap();
    assert_eq!(work.raises[0].resolved(), Some(failed));
    assert_eq!(work.contexts, ["user", "sys*"]);
}

/// §7.4.6.4.6: "Each <string_literal> is a non-empty string. If the
/// character * appears in a <string_literal>, it must appear only once, as
/// the last character of the <string_literal>, and must be preceded by one
/// or more characters other than *."
#[test]
fn idl_7_4_6_4_6_a_context_string_is_not_empty_and_ends_in_at_most_one_star() {
    assert_clean("interface I { void f() context (\"a\", \"b*\", \"CORBA.x*\"); };");
    for bad in ["\"\"", "\"*\"", "\"a**\"", "\"a*b\"", "\"*a\""] {
        assert_trips(
            &format!("interface I {{ void f() context ({bad}); }};"),
            DiagnosticCode::UnexpectedToken,
        );
    }
}

/// §7.4.6.4.5: one-way operations "Cannot trigger exceptions."
#[test]
fn idl_7_4_6_4_5_a_oneway_operation_has_no_raises_clause() {
    assert_clean("interface I { oneway void tell(in long x); };");
    assert_trips(
        "interface I { exception E { }; oneway void tell(in long x) raises (E); };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "interface I { exception E { }; @oneway void tell(in long x) raises (E); };",
        DiagnosticCode::InvalidAnnotation,
    );
}

/// Rules (85) to (90): `getraises` and `setraises` of an attribute, and
/// `raises` of a `readonly` one, each resolve to the exceptions they name.
#[test]
fn idl_7_4_3_4_3_3_2_an_attribute_keeps_its_getraises_and_setraises() {
    let (ast, found) = lowered(
        "interface I {
             exception GetFailed { };
             exception SetFailed { };
             attribute long level getraises (GetFailed) setraises (SetFailed);
             readonly attribute long count raises (GetFailed);
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let exception = |name: &str| ast.exceptions().find(|(_, e)| e.name == name).unwrap().0;
    let attribute = |name: &str| {
        let interface = ast.interfaces().next().unwrap().0;
        ast.attributes_of(interface)
            .iter()
            .map(|a| ast.attribute(*a).unwrap())
            .find(|a| a.name == name)
            .unwrap()
            .clone()
    };
    let level = attribute("level");
    assert!(!level.readonly);
    assert_eq!(level.get_raises[0].resolved(), Some(exception("GetFailed")));
    assert_eq!(level.set_raises[0].resolved(), Some(exception("SetFailed")));
    let count = attribute("count");
    assert!(count.readonly);
    assert_eq!(count.get_raises[0].resolved(), Some(exception("GetFailed")));
    assert!(count.set_raises.is_empty());
}

/// §7.4.4.4 and §7.2.3.1: attributes and operations share the one
/// namespace of the interface.
#[test]
fn idl_7_4_4_4_an_attribute_and_an_operation_of_one_name_collide() {
    assert_trips(
        "interface I { attribute long level; void level(); };",
        DiagnosticCode::DuplicateName,
    );
    assert_trips(
        "interface I { void Level(); readonly attribute long level; };",
        DiagnosticCode::DuplicateName,
    );
    assert_clean("interface I { attribute long level; void set_level(in long v); };");
}

// -- §7.4.5.4.1.3.2 Initializers ------------------------------------------------

/// Rule (107): `factory <identifier> ( <init_param_dcls> ) [ <raises_expr> ]`
/// declares an initializer of the value type, with its `in` parameters.
#[test]
fn idl_7_4_5_4_1_3_2_a_factory_is_an_initializer_of_its_value_type() {
    let (ast, found) = lowered(
        "valuetype V {
             public long state;
             exception Bad { };
             factory create(in long initial, in string name) raises (Bad);
             factory empty();
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (_, v) = ast.types().find(|(_, t)| t.name == "V").unwrap();
    let zerodds_ast::TypeKind::Valuetype {
        initializers,
        state,
        ..
    } = &v.kind
    else {
        panic!("{v:?}");
    };
    assert_eq!(state.len(), 1);
    let names: Vec<String> = initializers
        .iter()
        .map(|op| ast.operation(*op).unwrap().name.clone())
        .collect();
    assert_eq!(names, ["create", "empty"]);
    let create = ast.operation(initializers[0]).unwrap();
    assert_eq!(create.params.len(), 2);
    assert!(
        create
            .params
            .iter()
            .all(|p| ast.param(*p).unwrap().direction == ParamDirection::In)
    );
    assert_eq!(create.raises.len(), 1);
}

// -- §7.4.6.4.1.3 Repository Id Conflict -----------------------------------------

/// "both mechanisms must return the same repository id for the same IDL
/// element otherwise an error should be raised."
#[test]
fn idl_7_4_6_4_1_3_a_pragma_and_a_typeid_disagreeing_on_one_element_conflict() {
    assert_trips(
        "struct S { long x; };\n#pragma ID S \"IDL:A/S:1.0\"\ntypeid S \"IDL:B/S:1.0\";",
        DiagnosticCode::ConflictingAnnotations,
    );
    assert_clean("struct S { long x; };\n#pragma ID S \"IDL:A/S:1.0\"\ntypeid S \"IDL:A/S:1.0\";");
    assert_trips(
        "module M {\n#pragma prefix \"a.com\"\n struct S { long x; }; };\ntypeprefix M \"b.com\";",
        DiagnosticCode::ConflictingAnnotations,
    );
    assert_clean(
        "module M {\n#pragma prefix \"a.com\"\n struct S { long x; }; };\ntypeprefix M \"a.com\";",
    );
}

// -- §7.4.14.3 Anonymous types -----------------------------------------------------

/// Rule (216), `<type_spec> ::+ <template_type_spec>`: a sequence, a bounded
/// string, a wide string, a fixed-point type and a map written right at the
/// member, with no `typedef`.
#[test]
fn idl_7_4_14_3_rule_216_a_template_type_stands_anonymous_at_a_member() {
    let (ast, found) = lowered(
        "struct S {
             sequence<long> seq;
             sequence<long, 5> bounded;
             string<8> name;
             wstring<4> wide;
             fixed<5, 2> amount;
             map<long, string> table;
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast.types().find(|(_, t)| t.name == "S").unwrap();
    let kinds: Vec<&'static str> = ast
        .members_of(s)
        .iter()
        .map(|m| {
            let target = ast.member(*m).unwrap().type_.resolved().unwrap();
            let node = ast.type_(target).unwrap();
            assert!(node.name.is_empty(), "anonymous: {node:?}");
            node.kind.as_str()
        })
        .collect();
    assert_eq!(
        kinds,
        ["sequence", "sequence", "string", "string", "fixed", "map"]
    );
}

/// Rule (217), `<declarator> ::+ <array_declarator>`: `long arr[3][4];` and a
/// declarator list mixing an array and a plain name, with no `typedef`.
#[test]
fn idl_7_4_14_3_rule_217_an_array_declarator_stands_at_a_member() {
    let (ast, found) = lowered("struct S { long arr[3][4]; long plain, row[2]; };");
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast.types().find(|(_, t)| t.name == "S").unwrap();
    let dims: Vec<Option<Vec<i128>>> = ast
        .members_of(s)
        .iter()
        .map(|m| {
            let target = ast.member(*m).unwrap().type_.resolved().unwrap();
            match &ast.type_(target).unwrap().kind {
                zerodds_ast::TypeKind::Array { dims, .. } => Some(
                    dims.iter()
                        .map(|d| ast.eval_integral(*d).unwrap())
                        .collect(),
                ),
                _ => None,
            }
        })
        .collect();
    assert_eq!(dims, [Some(vec![3, 4]), None, Some(vec![2])]);
}

/// §7.4.14.4: "white space must be used to separate the two > tokens ending
/// the declaration so they are not parsed as a single >> token." The
/// spelling the clause asks for is read as two closing brackets, the run-on
/// spelling as well, and `>>` in an expression stays a shift.
#[test]
fn idl_7_4_14_4_nested_templates_close_with_two_brackets() {
    let (ast, found) = lowered(
        "typedef sequence<sequence<long> > Spaced;
         typedef sequence<sequence<long>> RunOn;
         const long Shifted = 16 >> 2;",
    );
    assert!(found.is_empty(), "{found:?}");
    for name in ["Spaced", "RunOn"] {
        let (alias, _) = ast.types().find(|(_, t)| t.name == name).unwrap();
        let zerodds_ast::TypeKind::Alias { aliased } = &ast.type_(alias).unwrap().kind else {
            panic!("{name}");
        };
        let outer = ast.type_(aliased.resolved().unwrap()).unwrap();
        let zerodds_ast::TypeKind::Sequence { element, .. } = &outer.kind else {
            panic!("{name}: {outer:?}");
        };
        let inner = ast.type_(element.resolved().unwrap()).unwrap();
        assert!(
            matches!(inner.kind, zerodds_ast::TypeKind::Sequence { .. }),
            "{name}: {inner:?}"
        );
    }
    let (_, shifted) = ast.constants().find(|(_, c)| c.name == "Shifted").unwrap();
    assert_eq!(ast.eval_integral(shifted.expr), Ok(4));
}

// -- §8.3.2.2 @must_understand, §8.3.3.1 @default -------------------------------

/// `@annotation must_understand { boolean value default TRUE; };`: the
/// compact form sets the flag, `FALSE` clears it, and a member without the
/// annotation is not must-understand ("It does not mean that by default …
/// an element is of that kind").
#[test]
fn idl_8_3_2_2_must_understand_is_true_by_its_compact_form_and_false_without_it() {
    let (ast, found) = lowered(
        "@mutable struct S {
             @must_understand long a;
             @must_understand(FALSE) long b;
             @must_understand(TRUE) long c;
             long d;
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast.types().find(|(_, t)| t.name == "S").unwrap();
    let flags: Vec<bool> = ast
        .members_of(s)
        .iter()
        .map(|m| ast.member(*m).unwrap().facets.is_must_understand())
        .collect();
    assert_eq!(flags, [true, false, true, false]);
}

/// `@annotation default { any value; };`: the value reaches the element,
/// and "The provided value must be compliant with the type of the annotated
/// element."
#[test]
fn idl_8_3_3_1_a_default_is_kept_and_must_fit_the_element() {
    let (ast, found) = lowered(
        "@default(7) typedef long Seven;
         struct S { @default(5) long a; @default(\"x\") string b; @default(2.5) double c; };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (s, _) = ast.types().find(|(_, t)| t.name == "S").unwrap();
    let a = ast.member(ast.members_of(s)[0]).unwrap();
    let value = a
        .facets
        .xtypes
        .as_ref()
        .and_then(|x| x.default_value)
        .unwrap();
    assert_eq!(ast.eval_integral(value), Ok(5));
    let (seven, _) = ast.types().find(|(_, t)| t.name == "Seven").unwrap();
    let value = ast
        .type_(seven)
        .unwrap()
        .facets
        .xtypes
        .as_ref()
        .and_then(|x| x.default_value)
        .unwrap();
    assert_eq!(ast.eval_integral(value), Ok(7));
    assert_trips(
        "struct S { @default(\"x\") long a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "struct S { @default(300) octet a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "struct S { @range(min = -1, max = 10) unsigned short a; };",
        DiagnosticCode::InvalidAnnotation,
    );
}

// -- §7.4.8 to §7.4.11 The component model ----------------------------------------

/// §7.4.8.4.2.1 and §7.4.8.4.2.2: a facet and a receptacle name "a
/// previously-defined non-component interface type", and the facet's name
/// reaches its role in the model; rule (157) adds `Object`, the root of all
/// interfaces (§7.4.10.4.4.2), which the port holds as the generic object
/// reference of rule (117).
#[test]
fn idl_7_4_8_4_2_1_a_facet_or_receptacle_names_an_interface_or_object() {
    let (ast, found) = lowered(
        "interface Service { };
         component C {
             provides Service facet;
             uses Service receptacle;
             provides Object root;
             uses multiple Object many;
         };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (c, _) = ast.interfaces().find(|(_, i)| i.name == "C").unwrap();
    let (service, _) = ast.interfaces().find(|(_, i)| i.name == "Service").unwrap();
    let roles: Vec<(String, Option<zerodds_ast::ComponentRole>, String)> = ast
        .attributes_of(c)
        .iter()
        .map(|a| {
            let node = ast.attribute(*a).unwrap();
            let kind = node
                .type_
                .resolved()
                .map_or("pending", |t| ast.type_(t).unwrap().kind.as_str());
            (
                node.name.clone(),
                node.facets
                    .corba
                    .as_ref()
                    .and_then(|c| c.component_role.clone()),
                kind.to_string(),
            )
        })
        .collect();
    assert!(matches!(
        &roles[0].1,
        Some(zerodds_ast::ComponentRole::Facet { name, interface }) if name == "facet" && interface.resolved() == Some(service)
    ));
    assert!(matches!(
        &roles[1].1,
        Some(zerodds_ast::ComponentRole::Receptacle { interface, multiple: false, .. }) if interface.resolved() == Some(service)
    ));
    assert!(matches!(
        &roles[2].1,
        Some(zerodds_ast::ComponentRole::Facet { .. })
    ));
    assert_eq!(
        roles[2].2, "object reference",
        "`provides Object` holds the root"
    );
    assert!(matches!(
        &roles[3].1,
        Some(zerodds_ast::ComponentRole::Receptacle { multiple: true, .. })
    ));
    assert_eq!(roles[3].2, "object reference");
    assert_trips(
        "component Other { }; component C { provides Other facet; };",
        DiagnosticCode::TypeMismatch,
    );
}

/// Rules (146) and (147): a home header names its single base home and the
/// component it manages, both "previously defined".
#[test]
fn idl_7_4_9_4_1_a_home_derives_from_a_home_and_manages_a_component() {
    let (ast, found) = lowered(
        "component C { };
         home Base manages C { };
         home Derived : Base manages C { };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (derived, node) = ast.interfaces().find(|(_, i)| i.name == "Derived").unwrap();
    let (base, _) = ast.interfaces().find(|(_, i)| i.name == "Base").unwrap();
    let (c, _) = ast.interfaces().find(|(_, i)| i.name == "C").unwrap();
    assert_eq!(node.bases[0].resolved(), Some(base));
    let manages = ast
        .interface(derived)
        .unwrap()
        .facets
        .corba
        .as_ref()
        .and_then(|corba| corba.manages.clone())
        .unwrap();
    assert_eq!(manages.component.resolved(), Some(c));
    assert_trips(
        "interface I { }; home H manages I { };",
        DiagnosticCode::TypeMismatch,
    );
    assert_trips(
        "component C { }; interface I { }; home H : I manages C { };",
        DiagnosticCode::InvalidInheritance,
    );
}

/// §7.4.11.4.1.1 and §7.4.11.4.2: a port type holds at least one facet,
/// receptacle or port; `port` and `mirrorport` name a port type and reach the
/// model as their two roles; "no cycles are allowed among port type
/// definitions".
#[test]
fn idl_7_4_11_4_1_1_a_port_type_holds_ports_and_does_not_embed_itself() {
    let (ast, found) = lowered(
        "interface Data { };
         porttype Inner { provides Data out_facet; };
         porttype Outer { port Inner nested; uses Data back; };
         component C { port Outer normal; mirrorport Outer reverse; };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (c, _) = ast.interfaces().find(|(_, i)| i.name == "C").unwrap();
    let roles: Vec<_> = ast
        .attributes_of(c)
        .iter()
        .map(|a| {
            ast.attribute(*a)
                .unwrap()
                .facets
                .corba
                .as_ref()
                .and_then(|c| c.component_role.clone())
        })
        .collect();
    assert_eq!(
        roles,
        [
            Some(zerodds_ast::ComponentRole::PortTypeRole),
            Some(zerodds_ast::ComponentRole::ConnectorRole)
        ]
    );
    assert_trips(
        "porttype Empty { attribute long a; };",
        DiagnosticCode::UnexpectedToken,
    );
    assert_trips(
        "interface Data { }; component C { port Data p; };",
        DiagnosticCode::TypeMismatch,
    );
    assert_trips(
        "interface Data { };
         porttype A;
         porttype B { port A a; provides Data d; };
         porttype A { port B b; };",
        DiagnosticCode::InvalidInheritance,
    );
}

/// §7.4.11.4.3: a connector holds "At least one port", may derive from "an
/// existing connector", and its `port` and `mirrorport` members keep their
/// roles.
#[test]
fn idl_7_4_11_4_3_a_connector_has_exports_and_derives_from_a_connector() {
    let (ast, found) = lowered(
        "interface Data { };
         porttype P { provides Data d; };
         connector Base { port P p; };
         connector Derived : Base { mirrorport P m; };",
    );
    assert!(found.is_empty(), "{found:?}");
    let (derived, node) = ast.interfaces().find(|(_, i)| i.name == "Derived").unwrap();
    assert_eq!(node.kind, zerodds_ast::InterfaceKind::Connector);
    let role = ast
        .attribute(ast.attributes_of(derived)[0])
        .unwrap()
        .facets
        .corba
        .as_ref()
        .and_then(|c| c.component_role.clone());
    assert_eq!(role, Some(zerodds_ast::ComponentRole::ConnectorRole));
    // Rule (183): an attribute is a connector export as much as a port is;
    // rule (180) asks for at least one export.
    assert_clean("connector Attributes { attribute long a; };");
    assert_trips("connector Empty { };", DiagnosticCode::UnexpectedToken);
    assert_trips(
        "interface Data { }; porttype P { provides Data d; };
         component K { }; connector X : K { port P p; };",
        DiagnosticCode::InvalidInheritance,
    );
}

// -- §8.2.2 Rules for using standardized annotations ---------------------------

/// §8.2.2: a specification that uses a standardized annotation "Shall respect
/// totally the standardized annotation syntax" and "Shall indicate the
/// default behavior when no annotation is placed on an element". Each of the
/// twenty-four annotations of §8.3 is read in the syntax its `@annotation`
/// declaration gives it — every parameter by name, and the compact form where
/// every parameter has a default — and an element without one keeps the
/// default the using specification states (DDS-XTypes 1.3 §7.3.1.2.1): not a
/// key, not optional, not external, no range, no unit, no default value.
#[test]
fn idl_8_2_2_every_standardized_annotation_is_read_in_its_declared_syntax() {
    let named = "module M {
        @extensibility(value = MUTABLE) @autoid(value = SEQUENTIAL) @nested(value = FALSE)
        @verbatim(language = \"*\", placement = BEFORE_DECLARATION, text = \"// x\")
        struct S {
            @id(value = 3) @key(value = TRUE) long a;
            @optional(value = TRUE) @must_understand(value = TRUE) long b;
            @default(value = 4) @range(min = 0, max = 9) @unit(value = \"m\") long c;
            @min(value = 1) @max(value = 2) @external(value = TRUE) long d;
            long plain;
        };
        @bit_bound(value = 8) bitmask B { @position(value = 3) flag };
        enum E { @value(value = 7) ONE, @default_literal TWO };
        @final struct F { long x; };
        @appendable struct A { long x; };
        @mutable struct U { long x; };
        @service(platform = \"DDS\") @ami(value = TRUE) interface I {
            @oneway(value = TRUE) void tell(in long x);
        };
    };";
    let (ast, found) = lowered(named);
    assert!(found.is_empty(), "{found:?}");
    let compact = "module N {
        @nested struct S {
            @key long a;
            @optional @must_understand @external long b;
        };
        @service @ami interface I { @oneway void tell(in long x); };
    };";
    assert_clean(compact);

    let (s, _) = ast.types().find(|(_, t)| t.name == "S").expect("S");
    let member = |index: usize| ast.member(ast.members_of(s)[index]).expect("member");
    let x = |index: usize| member(index).facets.xtypes.clone().unwrap_or_default();
    assert!(x(0).is_key);
    assert_eq!(member(0).facets.effective_member_id(), Some(3));
    assert!(x(1).is_optional && x(1).is_must_understand);
    assert!(x(2).default_value.is_some() && x(2).range.is_some());
    assert_eq!(x(2).unit.as_deref(), Some("m"));
    assert!(x(3).is_external);
    // The element without an annotation keeps the defaults.
    let plain = x(4);
    assert!(!plain.is_key && !plain.is_optional && !plain.is_external);
    assert!(!plain.is_must_understand);
    assert!(plain.range.is_none() && plain.unit.is_none() && plain.default_value.is_none());
}

/// §7.4.15.4.2: "An annotation may be applied to any IDL constructs or
/// sub-constructs. Applying an annotation consists in prefixing the
/// element under annotation" — the type of a `typedef` too, which IDL4 to
/// Java 1.0 §7.2.4.6 writes as `typedef @max(100) long Length;`. The
/// annotation lands on what each declarator declares.
#[test]
fn idl_7_4_15_4_2_an_annotation_prefixing_a_typedefs_type_annotates_the_typedef() {
    let (ast, found) = lowered("typedef @max(100) @unit(\"m\") long Length, Width;");
    assert!(found.is_empty(), "{found:?}");
    for name in ["Length", "Width"] {
        let (_, node) = ast.types().find(|(_, t)| t.name == name).unwrap();
        let xtypes = node.facets.xtypes.as_ref().expect("the annotations' facet");
        let max = xtypes.range.and_then(|r| r.max).expect("@max");
        assert_eq!(
            ast.eval_const(max),
            Ok(zerodds_ast::ConstValue::Integer(100))
        );
        assert_eq!(xtypes.unit.as_deref(), Some("m"));
    }
}
