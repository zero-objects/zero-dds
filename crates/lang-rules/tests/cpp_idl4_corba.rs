// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C++ Language Mapping 1.0 (OMG formal/25-03-03), Annex A.1: the
//! CORBA-specific mappings, over `runtime/cpp/include/omg/corba.hpp`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "cpp-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, corba_idl, cpp};
use zerodds_lang_rules::emit::idl4_cpp::{CppFile, Options, Platform};

const BOTH: &[&str] = &["c++11", "c++17"];

fn corba() -> Options {
    Options {
        platform: Platform::Corba,
        ..Options::default()
    }
}

/// The C++ of the CORBA module, as `omg/corba_module.hpp` holds it.
fn corba_module() -> Vec<CppFile> {
    cpp(
        &corba_idl(),
        &Options {
            corba_module: true,
            header: "corba_module.hpp".to_string(),
            ..corba()
        },
    )
}

/// A.1.3: "Except Any ... and TypeCode, all types used in the declaration
/// of TypeCode shall be derived from their IDL definition in
/// [OMG-CORBA-IFC] following the mapping rules defined in Chapter 7. The
/// resulting C++ definitions shall be placed in the CORBA namespace"; A.1.4
/// and A.1.5 say the same of `ORB` and of what `Object` names.
/// `omg/corba_module.hpp` is that C++, generated from `orb.idl` and
/// `runtime/corba/CORBA_PIDL.idl`; the file in the repository is what the
/// generator writes today, and it compiles on its own.
///
/// `ZERODDS_CORBA_MODULE_REGENERATE=1` writes the file.
#[test]
fn idl4cpp_a_1_3_the_corba_module_header_is_the_cpp_of_its_idl() {
    let files = corba_module();
    assert_eq!(files.len(), 1);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/cpp/include/omg/corba_module.hpp");
    // The file in the repository carries the licence lines of every
    // runtime header ahead of what the generator writes.
    let expected = format!(
        "// SPDX-License-Identifier: Apache-2.0\n// Copyright 2026 ZeroDDS Contributors\n{}",
        files[0].text
    );
    if std::env::var("ZERODDS_CORBA_MODULE_REGENERATE").is_ok() {
        std::fs::write(&path, &expected).expect("omg/corba_module.hpp");
    }
    let committed = std::fs::read_to_string(&path).expect("omg/corba_module.hpp");
    assert!(
        committed == expected,
        "runtime/cpp/include/omg/corba_module.hpp is behind the generator; run this test with \
         ZERODDS_CORBA_MODULE_REGENERATE=1"
    );
    let check = check_program(
        "omg/corba_module.hpp",
        "",
        r#"    ok(std::is_abstract<CORBA::ORB>::value, "ORB is the class of a local interface (A.1.4)");
    SAME(CORBA::RepositoryId, std::string);
    SAME(CORBA::OctetSeq, omg::types::sequence<std::uint8_t>);"#,
    );
    compile_and_run("corba-module", &[], &check, BOTH);
}

const SHAPES: &str = r"
module M {
    struct S { long a; string b; };
    union U switch (long) { case 1: long x; case 2: string y; };
    enum E { ONE, TWO };
    typedef string<8> BStr;
    typedef sequence<long> LSeq;
    typedef sequence<S, 4> BSeq;
    typedef long Grid[2][3];
    typedef map<string, long> Dict;
    typedef map<string, long, 5> BDict;
    @bit_bound(8) bitmask Flags { @position(0) f0, @position(3) f3 };
    exception Failed { long code; string why; };
    interface Foo {
        long op(in S s_in, out U u_out, inout E e_io) raises (Failed);
        attribute long count;
    };
    local interface Loc { void here(); };
    abstract interface Abs { void anywhere(); };
    interface Bar : Foo { Foo peer(in Foo other); };
    valuetype Val {
        public long amount;
        private string note;
        void bump();
        factory make(in long amount);
    };
    abstract valuetype AbsVal { void touch(); };
    valuetype Box long;
    struct Holder { any value; Foo ref; };
};
";

/// An implementation of `M::Bar`, for the references the checks hold.
const BAR_IMPL: &str = r#"
class BarImpl : public M::Bar {
public:
    std::int32_t count_ = 0;
    std::int32_t op(const M::S& s_in, M::U& u_out, M::E& e_io) override {
        u_out.x(s_in.a);
        e_io = M::E::TWO;
        if (s_in.a < 0) throw M::Failed(s_in.a, s_in.b, "negative");
        return s_in.a + 1;
    }
    void count(std::int32_t value) override { count_ = value; }
    std::int32_t count() const override { return count_; }
    omg::types::ref_type<M::Foo> peer(omg::types::ref_type<M::Foo> other) override { return other; }
};
"#;

/// A.1.1: `CORBA::traits<T>` for every IDL-defined type — the common
/// members of Table A.1, the additional ones of Tables A.2 to A.12, the
/// parameter passing modes of Tables A.13 and A.14 in an operation's
/// signature, and the helper methods of A.1.1.10.
#[test]
fn idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14() {
    let files = cpp(SHAPES, &corba());
    let check = check_program(
        &files[0].path,
        BAR_IMPL,
        r#"    using CORBA::traits;
    // Table A.1
    SAME(traits<M::S>::value_type, M::S);
    SAME(traits<M::S>::in_type, const M::S&);
    SAME(traits<M::S>::out_type, M::S&);
    SAME(traits<M::S>::inout_type, M::S&);
    SAME(traits<M::U>::in_type, const M::U&);
    SAME(traits<std::int32_t>::in_type, std::int32_t);
    SAME(traits<M::E>::in_type, M::E);
    SAME(traits<M::E>::out_type, M::E&);
    // Table A.2
    SAME(traits<M::Foo>::is_local, std::false_type);
    SAME(traits<M::Foo>::is_abstract, std::false_type);
    SAME(traits<M::Loc>::is_local, std::true_type);
    SAME(traits<M::Abs>::is_abstract, std::true_type);
    SAME(traits<M::Foo>::ref_type, omg::types::ref_type<M::Foo>);
    SAME(traits<M::Foo>::weak_ref_type, omg::types::weak_ref_type<M::Foo>);
    // Tables A.3 and A.4
    SAME(traits<std::string>::element_traits, traits<char>);
    SAME(traits<std::wstring>::element_traits, traits<wchar_t>);
    SAME(traits<std::string>::is_bounded, std::false_type);
    SAME(traits<M::BStr>::element_traits, traits<char>);
    SAME(traits<M::BStr>::is_bounded, std::true_type);
    SAME(traits<M::BStr>::bound::value_type, std::uint32_t);
    ok(traits<M::BStr>::bound::value == 8, "the bound of string<8>");
    // Tables A.5 and A.6
    SAME(traits<M::LSeq>::element_traits, traits<std::int32_t>);
    SAME(traits<M::LSeq>::is_bounded, std::false_type);
    SAME(traits<M::BSeq>::element_traits, traits<M::S>);
    SAME(traits<M::BSeq>::is_bounded, std::true_type);
    SAME(traits<M::BSeq>::bound::value_type, std::uint32_t);
    ok(traits<M::BSeq>::bound::value == 4, "the bound of sequence<S, 4>");
    // Table A.7: "The element_traits for a multidimensional array shall
    // be the type trait for the core type."
    SAME(traits<M::Grid>::element_traits, traits<std::int32_t>);
    SAME(traits<M::Grid>::dimensions::value_type, std::uint32_t);
    ok(traits<M::Grid>::dimensions::value == 2, "the dimensions of long[2][3]");
    // Table A.8 and A.1.8.9.2
    SAME(traits<M::Val>::is_abstract, std::false_type);
    SAME(traits<M::Val>::is_truncatable, std::false_type);
    SAME(traits<M::AbsVal>::is_abstract, std::true_type);
    SAME(traits<M::Val>::factory_type, M::Val_factory);
    ok(std::is_base_of<M::Val, traits<M::Val>::obv_type>::value, "obv_type is the OBV class of Val");
    // Table A.9
    SAME(traits<M::Box>::boxed_traits, traits<std::int32_t>);
    // Tables A.10 and A.11
    SAME(traits<M::Dict>::key_traits, traits<std::string>);
    SAME(traits<M::Dict>::value_traits, traits<std::int32_t>);
    SAME(traits<M::Dict>::is_bounded, std::false_type);
    SAME(traits<M::BDict>::is_bounded, std::true_type);
    ok(traits<M::BDict>::bound::value == 5, "the bound of map<string, long, 5>");
    // Table A.12: "the traits template is specialized on the <Bitmask>Bits type"
    SAME(traits<M::Flags::FlagsBits>::bit_bound::value_type, std::uint32_t);
    ok(traits<M::Flags::FlagsBits>::bit_bound::value == 8, "the bit bound of Flags");
    SAME(traits<M::Flags::FlagsBits>::underlying_type, std::uint8_t);
    // Table A.13: interfaces, value types, TypeCode, enums and basic types
    SAME(traits<M::Foo>::in_type, omg::types::ref_type<M::Foo>);
    SAME(traits<M::Foo>::out_type, omg::types::ref_type<M::Foo>&);
    SAME(traits<M::Foo>::inout_type, omg::types::ref_type<M::Foo>&);
    SAME(traits<M::Val>::in_type, omg::types::ref_type<M::Val>);
    SAME(traits<CORBA::TypeCode>::in_type, omg::types::ref_type<CORBA::TypeCode>);
    SAME(decltype(&M::Bar::peer), omg::types::ref_type<M::Foo> (M::Bar::*)(omg::types::ref_type<M::Foo>));
    // Table A.14: structured types, sequences and strings
    SAME(decltype(&M::Foo::op), std::int32_t (M::Foo::*)(const M::S&, M::U&, M::E&));
    // A.1.1.10
    omg::types::ref_type<BarImpl> bar = CORBA::make_reference<BarImpl>();
    omg::types::ref_type<CORBA::Object> object = bar;
    ok(traits<M::Foo>::narrow(object) != nullptr, "narrow to a base interface of the object");
    ok(traits<M::Bar>::narrow(object) == bar, "narrow to its interface is the object");
    ok(traits<M::Loc>::narrow(object) == nullptr, "narrow to another interface is a nil reference");
    ok(traits<M::Foo>::narrow(nullptr) == nullptr, "narrow of a nil reference is nil");
    omg::types::ref_type<M::Foo> nil;
    ok(!nil, "a default-constructed reference is nil");
    M::S s;
    s.a = 4;
    M::U u;
    M::E e = M::E::ONE;
    ok(bar->op(s, u, e) == 5 && u.x() == 4 && e == M::E::TWO, "in, out and inout");"#,
    );
    compile_and_run("corba-traits", &files, &check, BOTH);
}

/// A.1.2: an exception is a class on `CORBA::UserException`; the
/// `Exception` and `SystemException` classes as the clause declares them;
/// `UnknownUserException` (A.1.2.1); insertion into and extraction from an
/// `Any` for user and system exceptions, insertion for `Exception` and
/// the generic extraction of a system exception (A.1.2.2);
/// `CORBA::BAD_PARAM` for a union member that is not selected (A.1.2.3)
/// and for an exceeded bound (A.1.2.4).
#[test]
fn idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause() {
    let files = cpp(SHAPES, &corba());
    let check = check_program(
        &files[0].path,
        "",
        r#"    using CORBA::CompletionStatus;
    // A.1.2
    ok(std::is_base_of<CORBA::UserException, M::Failed>::value, "an IDL exception derives from CORBA::UserException");
    ok(std::is_base_of<CORBA::Exception, CORBA::UserException>::value
           && std::is_base_of<CORBA::Exception, CORBA::SystemException>::value
           && std::is_base_of<std::exception, CORBA::Exception>::value,
       "UserException and SystemException derive from Exception, Exception from std::exception");
    ok(std::is_abstract<CORBA::Exception>::value && std::is_abstract<CORBA::SystemException>::value,
       "the Exception base class is abstract");
    ok(!std::is_default_constructible<CORBA::Exception>::value, "its constructors are protected");
    SAME(decltype(&CORBA::Exception::raise), void (CORBA::Exception::*)() const);
    SAME(decltype(&CORBA::Exception::_name), const char* (CORBA::Exception::*)() const);
    SAME(decltype(&CORBA::Exception::_rep_id), const char* (CORBA::Exception::*)() const);
    M::Failed f;
    ok(f.code() == 0 && f.why().empty(), "the default constructor initializes the members to their default values");
    M::Failed g(7, "seven", "explained");
    ok(g.code() == 7 && g.why() == "seven" && std::string(g.what()) == "explained", "the explicit constructor");
    g.code(8);
    ok(g.code() == 8, "the modifier");
    ok(std::string(g._name()) == "Failed", "_name is the unqualified name");
    ok(std::string(g._rep_id()) == "IDL:M/Failed:1.0", "_rep_id is the repository id");
    M::Failed copy(g);
    M::Failed moved(std::move(copy));
    f = moved;
    ok(f.code() == 8 && f.why() == "seven", "copy, move and assignment");
    try {
        const CORBA::Exception& base = g;
        base.raise();
        ok(false, "raise() throws");
    } catch (const M::Failed& caught) {
        ok(caught.code() == 8, "raise() throws the derived type");
    }
    try {
        throw g;
    } catch (const CORBA::UserException&) {
    } catch (...) {
        ok(false, "a user exception is caught as UserException");
    }
    try {
        throw CORBA::BAD_PARAM();
    } catch (const CORBA::UserException&) {
        ok(false, "a system exception is no user exception");
    } catch (const CORBA::SystemException& se) {
        ok(se.minor() == 0 && se.completed() == CompletionStatus::COMPLETED_NO,
           "the default constructor: minor 0, COMPLETED_NO");
        ok(std::string(se._name()) == "BAD_PARAM", "the name of a system exception");
    }
    try {
        throw CORBA::NO_MEMORY();
    } catch (const std::exception& e) {
        ok(e.what() != nullptr && e.what()[0] != 0, "what() of a system exception, caught as std::exception");
    }
    SAME(std::underlying_type<CompletionStatus>::type, std::uint32_t);
    ok(static_cast<int>(CompletionStatus::COMPLETED_YES) == 0 && static_cast<int>(CompletionStatus::COMPLETED_MAYBE) == 2,
       "COMPLETED_YES, COMPLETED_NO, COMPLETED_MAYBE");
    CORBA::BAD_PARAM bp(3, CompletionStatus::COMPLETED_YES);
    bp.minor(4);
    bp.completed(CompletionStatus::COMPLETED_MAYBE);
    ok(bp.minor() == 4 && bp.completed() == CompletionStatus::COMPLETED_MAYBE, "minor and completed");
    ok(std::is_base_of<CORBA::SystemException, CORBA::UNKNOWN>::value
           && std::is_base_of<CORBA::SystemException, CORBA::TRANSACTION_MODE>::value,
       "each standard exception derives from SystemException");
#if __cplusplus >= 201402L
    ok(std::is_final<CORBA::UNKNOWN>::value && std::is_final<CORBA::UnknownUserException>::value, "final classes");
#endif
    // A.1.2.1
    ok(std::is_base_of<CORBA::UserException, CORBA::UnknownUserException>::value,
       "UnknownUserException derives from UserException");
    SAME(decltype(std::declval<const CORBA::UnknownUserException&>().exception()), const CORBA::Any&);
    // A.1.2.2
    CORBA::Any any;
    any <<= g;
    M::Failed out;
    ok((any >>= out) && out.code() == 8 && out.why() == "seven", "a user exception through an any, copied in");
    any <<= M::Failed(1, "one", "moved in");
    ok((any >>= out) && out.code() == 1, "a user exception through an any, moved in");
    ok(any.type()->equal(M::_tc_Failed), "the any holds the exception's TypeCode");
    CORBA::Any generic;
    {
        const CORBA::Exception& base = g;
        generic <<= base;
    }
    M::Failed derived;
    ok((generic >>= derived) && derived.code() == 8, "inserted as Exception, the value is the derived exception");
    ok(generic.type()->equal(M::_tc_Failed), "and the TypeCode the derived exception's");
    CORBA::Any system;
    system <<= CORBA::BAD_PARAM(9, CompletionStatus::COMPLETED_YES);
    CORBA::BAD_PARAM typed;
    ok((system >>= typed) && typed.minor() == 9 && typed.completed() == CompletionStatus::COMPLETED_YES,
       "a system exception through an any");
    CORBA::BAD_PARAM held;
    CORBA::SystemException& se = held;
    ok((system >>= se) && held.minor() == 9, "the generic extraction of a system exception");
    CORBA::NO_MEMORY other;
    ok(!(system >>= other), "the extraction of another system exception fails");
    CORBA::Any number;
    number <<= std::int32_t(5);
    ok(!(number >>= se) && held.minor() == 9, "no system exception in the any: false, the value unchanged");
    // A.1.2.3
    M::U u;
    u.x(1);
    throws<CORBA::BAD_PARAM>([&] { (void)u.y(); }, "a union member that is not selected");
    // A.1.2.4
    M::BStr text("12345678");
    throws<CORBA::BAD_PARAM>([&] { text += "9"; }, "a ninth character in string<8>");
    M::BSeq seq(4);
    throws<CORBA::BAD_PARAM>([&] { seq.push_back(M::S()); }, "a fifth element in sequence<S, 4>");
    M::BDict dict;
    for (int i = 0; i < 5; ++i) dict[std::to_string(i)] = i;
    throws<CORBA::BAD_PARAM>([&] { dict["sixth"] = 6; }, "a sixth key in map<string, long, 5>");"#,
    );
    compile_and_run("corba-exceptions", &files, &check, BOTH);
}

/// A.1.3: a `_tc_<type>` constant of type
/// `omg::types::ref_type<CORBA::TypeCode>` at the scoping level of each
/// defined type, and the `CORBA::TypeCode` class of the clause, its
/// operations with the behaviour of CORBA 3.3 part 1 §8.11.1.
#[test]
fn idl4cpp_a_1_3_every_type_has_its_tc_constant_and_typecode_is_the_clauses_class() {
    let files = cpp(SHAPES, &corba());
    let check = check_program(
        &files[0].path,
        "",
        r#"    using CORBA::TCKind;
    using CORBA::TypeCode;
    typedef omg::types::ref_type<TypeCode> Ref;
    SAME(decltype(M::_tc_S), const Ref);
    // the declaration of the class
    SAME(decltype(&TypeCode::equal), bool (TypeCode::*)(Ref) const);
    SAME(decltype(&TypeCode::equivalent), bool (TypeCode::*)(Ref) const);
    SAME(decltype(&TypeCode::kind), TCKind (TypeCode::*)() const);
    SAME(decltype(&TypeCode::get_compact_typecode), Ref (TypeCode::*)() const);
    SAME(decltype(&TypeCode::id), const std::string& (TypeCode::*)() const);
    SAME(decltype(&TypeCode::name), const std::string& (TypeCode::*)() const);
    SAME(decltype(&TypeCode::member_count), std::uint32_t (TypeCode::*)() const);
    SAME(decltype(&TypeCode::member_name), const std::string& (TypeCode::*)(std::uint32_t) const);
    SAME(decltype(&TypeCode::member_type), Ref (TypeCode::*)(std::uint32_t) const);
    SAME(decltype(&TypeCode::member_label), const CORBA::Any& (TypeCode::*)(std::uint32_t) const);
    SAME(decltype(&TypeCode::discriminator_type), Ref (TypeCode::*)() const);
    SAME(decltype(&TypeCode::default_index), std::int32_t (TypeCode::*)() const);
    SAME(decltype(&TypeCode::length), std::uint32_t (TypeCode::*)() const);
    SAME(decltype(&TypeCode::content_type), Ref (TypeCode::*)() const);
    SAME(decltype(&TypeCode::fixed_digits), std::uint16_t (TypeCode::*)() const);
    SAME(decltype(&TypeCode::fixed_scale), std::int16_t (TypeCode::*)() const);
    SAME(decltype(&TypeCode::member_visibility), CORBA::Visibility (TypeCode::*)(std::uint32_t) const);
    SAME(decltype(&TypeCode::type_modifier), CORBA::ValueModifier (TypeCode::*)() const);
    SAME(decltype(&TypeCode::concrete_base_type), Ref (TypeCode::*)() const);
    ok(std::is_base_of<CORBA::UserException, TypeCode::Bounds>::value
           && std::is_base_of<CORBA::UserException, TypeCode::BadKind>::value,
       "Bounds and BadKind are user exceptions");
    ok(!std::is_default_constructible<TypeCode>::value, "no public constructors");
    // a structure
    ok(M::_tc_S->kind() == TCKind::tk_struct && M::_tc_S->id() == "IDL:M/S:1.0" && M::_tc_S->name() == "S", "_tc_S");
    ok(M::_tc_S->member_count() == 2 && M::_tc_S->member_name(1) == "b", "its members");
    ok(M::_tc_S->member_type(0)->equal(CORBA::_tc_long) && M::_tc_S->member_type(1)->kind() == TCKind::tk_string,
       "and their types");
    throws<TypeCode::Bounds>([] { (void)M::_tc_S->member_name(2); }, "Bounds at member_count");
    throws<TypeCode::BadKind>([] { (void)M::_tc_S->length(); }, "BadKind for the length of a structure");
    throws<TypeCode::BadKind>([] { (void)CORBA::_tc_long->id(); }, "BadKind for the id of long");
    // a union
    ok(M::_tc_U->kind() == TCKind::tk_union && M::_tc_U->member_count() == 2, "_tc_U");
    ok(M::_tc_U->discriminator_type()->equal(CORBA::_tc_long) && M::_tc_U->default_index() == -1,
       "its discriminator, and no default member");
    std::int32_t label = 0;
    ok((M::_tc_U->member_label(1) >>= label) && label == 2, "member_label");
    // an enumeration, an exception
    ok(M::_tc_E->kind() == TCKind::tk_enum && M::_tc_E->member_count() == 2 && M::_tc_E->member_name(1) == "TWO", "_tc_E");
    ok(M::_tc_Failed->kind() == TCKind::tk_except && M::_tc_Failed->member_name(0) == "code", "_tc_Failed");
    // typedefs: an alias of the template type
    ok(M::_tc_BStr->kind() == TCKind::tk_alias && M::_tc_BStr->name() == "BStr", "_tc_BStr is an alias");
    ok(M::_tc_BStr->content_type()->kind() == TCKind::tk_string && M::_tc_BStr->content_type()->length() == 8,
       "of string<8>");
    ok(M::_tc_LSeq->content_type()->kind() == TCKind::tk_sequence && M::_tc_LSeq->content_type()->length() == 0,
       "an unbounded sequence has the length 0");
    ok(M::_tc_BSeq->content_type()->length() == 4 && M::_tc_BSeq->content_type()->content_type()->equal(M::_tc_S),
       "sequence<S, 4>");
    Ref grid = M::_tc_Grid->content_type();
    ok(grid->kind() == TCKind::tk_array && grid->length() == 2 && grid->content_type()->kind() == TCKind::tk_array
           && grid->content_type()->length() == 3 && grid->content_type()->content_type()->equal(CORBA::_tc_long),
       "long[2][3] is an array of 2 arrays of 3");
    ok(M::_tc_BStr->equivalent(M::_tc_BStr->content_type()) && !M::_tc_BStr->equal(M::_tc_BStr->content_type()),
       "an alias is equivalent to its type, not equal");
    // interfaces
    ok(M::_tc_Foo->kind() == TCKind::tk_objref && M::_tc_Foo->id() == "IDL:M/Foo:1.0", "_tc_Foo");
    ok(M::_tc_Loc->kind() == TCKind::tk_local_interface && M::_tc_Abs->kind() == TCKind::tk_abstract_interface,
       "a local and an abstract interface");
    // value types
    ok(M::_tc_Val->kind() == TCKind::tk_value && M::_tc_Val->member_count() == 2, "_tc_Val");
    ok(M::_tc_Val->member_visibility(0) == CORBA::PUBLIC_MEMBER && M::_tc_Val->member_visibility(1) == CORBA::PRIVATE_MEMBER,
       "the visibility of its members");
    ok(M::_tc_Val->type_modifier() == CORBA::VM_NONE && M::_tc_AbsVal->type_modifier() == CORBA::VM_ABSTRACT,
       "the modifier of a value type");
    ok(!M::_tc_Val->concrete_base_type(), "no concrete base is a nil TypeCode");
    ok(M::_tc_Box->kind() == TCKind::tk_value_box && M::_tc_Box->content_type()->equal(CORBA::_tc_long), "_tc_Box");
    // the structure that names an any and an interface
    ok(M::_tc_Holder->member_type(0)->kind() == TCKind::tk_any && M::_tc_Holder->member_type(1)->equal(M::_tc_Foo),
       "any and an object reference as members");
    Ref compact = M::_tc_S->get_compact_typecode();
    ok(compact->name().empty() && compact->member_name(0).empty() && compact->id() == "IDL:M/S:1.0"
           && compact->equivalent(M::_tc_S),
       "the compact TypeCode has no names and is equivalent");"#,
    );
    compile_and_run("corba-typecode", &files, &check, BOTH);
}

const OBJECTS: &str = r"
module N {
    interface A { long fa(); };
    interface B : A { long fb(); };
    interface C : B { long fc(); };
    interface D : C { long fd(); };
    interface E : A { long fe(); };
    local interface LocalIF { void an_op(in long an_arg); };
};
";

/// A.1.5: `CORBA::Object` with the operations of the Object interface
/// under a leading underscore, implicit widening (A.1.5.1, A.1.5.2), the
/// nil reference and `INV_OBJREF` (A.1.5.3), narrowing with the two
/// examples of A.1.5.4. A.1.6: `LocalObject`, the class of a local
/// interface as `traits<>::base_type`, and the example of the clause.
#[test]
fn idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil() {
    let files = cpp(OBJECTS, &corba());
    let check = check_program(
        &files[0].path,
        r"
class CImpl : public N::C {
public:
    std::int32_t fa() override { return 1; }
    std::int32_t fb() override { return 2; }
    std::int32_t fc() override { return 3; }
};
class MyLocalIF : public N::LocalIF {
public:
    std::int32_t seen = 0;
    void an_op(std::int32_t an_arg) override { seen = an_arg; }
};
",
        r##"    using omg::types::ref_type;
    using CORBA::traits;
    using CORBA::Object;
    // A.1.5: the operations of the Object interface, with a leading underscore
    SAME(decltype(&Object::_get_interface), ref_type<CORBA::InterfaceDef> (Object::*)());
    SAME(decltype(&Object::_is_nil), bool (Object::*)());
    SAME(decltype(&Object::_duplicate), ref_type<Object> (Object::*)());
    SAME(decltype(&Object::_release), void (Object::*)());
    SAME(decltype(&Object::_is_a), bool (Object::*)(const std::string&));
    SAME(decltype(&Object::_non_existent), bool (Object::*)());
    SAME(decltype(&Object::_is_equivalent), bool (Object::*)(ref_type<Object>));
    SAME(decltype(&Object::_hash), std::uint32_t (Object::*)(std::uint32_t));
    SAME(decltype(&Object::_create_request),
         void (Object::*)(ref_type<CORBA::Context>, const std::string&, ref_type<CORBA::NVList>, CORBA::NamedValue&,
                          ref_type<CORBA::Request>&, std::uint32_t));
    SAME(decltype(&Object::_get_policy), ref_type<CORBA::Policy> (Object::*)(std::uint32_t));
    SAME(decltype(&Object::_get_domain_managers), std::vector<ref_type<CORBA::DomainManager>> (Object::*)());
    SAME(decltype(&Object::_set_policy_overrides),
         ref_type<Object> (Object::*)(const std::vector<ref_type<CORBA::Policy>>&, CORBA::SetOverrideType));
    SAME(decltype(&Object::_get_client_policy), ref_type<CORBA::Policy> (Object::*)(std::uint32_t));
    SAME(decltype(&Object::_get_policy_overrides),
         std::vector<ref_type<CORBA::Policy>> (Object::*)(const std::vector<std::uint32_t>&));
    SAME(decltype(&Object::_validate_connection), bool (Object::*)(std::vector<ref_type<CORBA::Policy>>&));
    SAME(decltype(&Object::_repository_id), std::string (Object::*)());
    SAME(decltype(&Object::_get_component), ref_type<Object> (Object::*)());
    SAME(decltype(&Object::_get_orb), ref_type<CORBA::ORB> (Object::*)());
    ok(std::is_base_of<Object, N::A>::value, "the class of an interface derives from CORBA::Object");

    // A.1.5.1, A.1.5.2: implicit widening
    ref_type<N::C> cp = CORBA::make_reference<CImpl>();
    ref_type<N::B> bp = cp;
    ref_type<N::A> ap = bp;
    ref_type<Object> objp = bp;
    objp = ap;
    ok(std::is_convertible<ref_type<N::B>, ref_type<N::A>>::value
           && std::is_convertible<ref_type<N::B>, ref_type<Object>>::value,
       "B widens implicitly to A and to Object");
    ok(!std::is_convertible<ref_type<N::A>, ref_type<N::B>>::value, "A does not narrow implicitly");
    ok(objp == cp && ap->fa() == 1 && bp->fb() == 2, "the widened references name the object");

    // A.1.5.3: the nil reference
    ref_type<N::A> nil;
    ref_type<N::A> from_nullptr = nullptr;
    ok(nil == nullptr && from_nullptr == nullptr && !nil, "a default-constructed reference is nil, nil is nullptr");
    throws<CORBA::INV_OBJREF>([&] { nil->fa(); }, "an operation through a nil reference raises INV_OBJREF");
    throws<CORBA::INV_OBJREF>([&] { (*nil).fa(); }, "so does one through the dereferenced nil reference");

    // A.1.5.4: narrowing, the first example (D : C : B : A, a C widened to A)
    ok(traits<N::A>::narrow(ap) != nullptr && traits<N::B>::narrow(ap) != nullptr
           && traits<N::C>::narrow(ap) != nullptr,
       "narrowing to A, B and C returns a valid reference");
    ok(traits<N::D>::narrow(ap) == nullptr, "narrowing to D returns a nil reference");
    // the second example (C : B, B and E : A)
    ok(traits<N::E>::narrow(ap) == nullptr, "narrowing to the unrelated interface returns a nil reference");
    ok(traits<N::C>::narrow(nullptr) == nullptr, "narrowing a nil reference returns a nil reference");
    ref_type<N::C> narrowed = traits<N::C>::narrow(ap);
    ok(narrowed->fc() == 3 && ap != nullptr && narrowed == ap, "narrow does not change the given reference");

    // the operations of Object on an object of this process
    ok(cp->_is_a("IDL:N/C:1.0") && cp->_is_a("IDL:N/B:1.0") && cp->_is_a("IDL:N/A:1.0")
           && cp->_is_a("IDL:omg.org/CORBA/Object:1.0") && !cp->_is_a("IDL:N/D:1.0"),
       "_is_a answers for the interface, its bases and Object");
    ok(cp->_repository_id() == "IDL:N/C:1.0", "_repository_id");
    ok(!cp->_non_existent() && !cp->_is_nil(), "_non_existent and _is_nil");
    ok(cp->_is_equivalent(objp) && !cp->_is_equivalent(CORBA::make_reference<CImpl>()), "_is_equivalent");
    ok(cp->_hash(15) <= 15 && cp->_hash(15) == objp->_hash(15), "_hash stays below the maximum");
    ok(cp->_duplicate() == objp, "_duplicate");
    ok(cp->_get_domain_managers().empty(), "_get_domain_managers");
    std::vector<ref_type<CORBA::Policy>> inconsistent;
    ok(cp->_validate_connection(inconsistent) && inconsistent.empty(), "_validate_connection");
    throws<CORBA::INITIALIZE>([&] { cp->_get_orb(); }, "_get_orb before ORB_init raises INITIALIZE");

    // A.1.6
    ok(std::is_base_of<Object, CORBA::LocalObject>::value, "LocalObject derives from Object");
    ok(std::is_base_of<CORBA::LocalObject, N::LocalIF>::value, "the class of a local interface derives from LocalObject");
    SAME(traits<N::LocalIF>::base_type, N::LocalIF);
    SAME(traits<N::A>::base_type, N::A);
    ok(traits<N::LocalIF>::is_local::value && !traits<N::A>::is_local::value, "is_local");
    ref_type<N::LocalIF> myref = CORBA::make_reference<MyLocalIF>();
    myref->an_op(5);
    ok(std::dynamic_pointer_cast<MyLocalIF>(myref)->seen == 5, "the example of the clause");
    ref_type<CORBA::LocalObject> local = myref;
    ok(traits<CORBA::LocalObject>::narrow(myref) == local && traits<CORBA::LocalObject>::narrow(cp) == nullptr,
       "narrowing to LocalObject");
    ok(traits<N::LocalIF>::narrow(local) == myref && myref->_repository_id() == "IDL:N/LocalIF:1.0",
       "narrowing to the local interface");"##,
    );
    compile_and_run("corba-objects", &files, &check, BOTH);
}

const ANYS: &str = r"
module N {
    typedef long LongType;
    struct MyStruct { long lmem; short smem; };
    enum Color { red, green, blue };
    union Pick switch (long) { case 1: long x; case 2: string y; };
    typedef sequence<MyStruct> Structs;
    typedef sequence<long, 3> Few;
    typedef long Row[4];
    typedef string<8> Brief;
    exception Oops { long code; };
    interface A { long fa(); };
    interface B : A { long fb(); };
    abstract interface Shape { long sides(); };
    interface Drawing : Shape { void draw(); };
    valuetype Square supports Shape { public long edge; };
    valuetype Plain { public long n; };
};
";

/// A.1.7: `CORBA::Any` — insertion (A.1.7.2) and extraction (A.1.7.3) for
/// every kind of IDL type, what an object, abstract and value reference is
/// extractable as, and the replacement of the TypeCode (A.1.7.4).
#[test]
fn idl4cpp_a_1_7_any_inserts_and_extracts_every_type() {
    let files = cpp(ANYS, &corba());
    let check = check_program(
        &files[0].path,
        r"
class BImpl : public N::B {
public:
    std::int32_t fa() override { return 1; }
    std::int32_t fb() override { return 2; }
};
class DrawingImpl : public N::Drawing {
public:
    std::int32_t sides() override { return 0; }
    void draw() override {}
};
class SquareImpl : public N::OBV_Square {
public:
    SquareImpl() {}
    std::int32_t sides() override { return 4; }
};
class PlainImpl : public N::OBV_Plain {
public:
    PlainImpl() {}
};
#define ROUND(T, V, K)                                                                          \
    {                                                                                           \
        typedef T round_type;                                                                   \
        CORBA::Any x;                                                                           \
        x <<= static_cast<round_type>(V);                                                       \
        round_type back = round_type();                                                         \
        ok((x >>= back) && back == static_cast<round_type>(V) && x.type()->kind() == CORBA::TCKind::K, #T); \
    }
",
        r##"    using omg::types::ref_type;
    using CORBA::Any;
    using CORBA::TCKind;
    // A.1.7.2, A.1.7.3: the example of the clause
    std::int32_t value = 42;
    Any a;
    a <<= value;
    std::int32_t out = 0;
    ok((a >>= out) && out == 42 && a.type()->kind() == TCKind::tk_long, "the value and the TypeCode are set");
    std::int16_t other = 7;
    ok(!(a >>= other) && other == 7, "another type: false, and the variable is not changed");
    // the types normally passed by value
    ROUND(std::int16_t, -3, tk_short)
    ROUND(std::int32_t, -4, tk_long)
    ROUND(std::int64_t, -5, tk_longlong)
    ROUND(std::uint16_t, 3, tk_ushort)
    ROUND(std::uint32_t, 4, tk_ulong)
    ROUND(std::uint64_t, 5, tk_ulonglong)
    ROUND(float, 1.5f, tk_float)
    ROUND(double, 2.5, tk_double)
    ROUND(long double, 3.5L, tk_longdouble)
    ROUND(char, 'c', tk_char)
    ROUND(wchar_t, L'w', tk_wchar)
    ROUND(bool, true, tk_boolean)
    ROUND(std::uint8_t, 200, tk_octet)
    ROUND(N::Color, N::Color::blue, tk_enum)
    Any t;
    t <<= N::_tc_MyStruct;
    ref_type<CORBA::TypeCode> tc;
    ok((t >>= tc) && tc->equal(N::_tc_MyStruct) && t.type()->kind() == TCKind::tk_TypeCode, "a TypeCode reference");
    // the types too large to be passed by value: copying and move insert
    N::MyStruct s;
    s.lmem = 3;
    s.smem = 4;
    Any sa;
    sa <<= s;
    N::MyStruct sb;
    ok((sa >>= sb) && sb.lmem == 3 && sb.smem == 4 && sa.type()->equal(N::_tc_MyStruct), "a struct, copied in");
    Any sm;
    sm <<= std::move(s);
    N::MyStruct sc;
    ok((sm >>= sc) && sc.smem == 4, "a struct, moved in");
    N::Pick p;
    p.y("two");
    Any u;
    u <<= p;
    N::Pick q;
    ok((u >>= q) && q.y() == "two" && u.type()->equal(N::_tc_Pick), "a union");
    Any str;
    str <<= std::string("text");
    std::string text;
    ok((str >>= text) && text == "text" && str.type()->kind() == TCKind::tk_string, "a string");
    Any wide;
    wide <<= std::wstring(L"wide");
    std::wstring wtext;
    ok((wide >>= wtext) && wtext == L"wide" && wide.type()->kind() == TCKind::tk_wstring, "a wstring");
    Any nested;
    nested <<= a;
    Any inner;
    ok((nested >>= inner) && inner == a && nested.type()->kind() == TCKind::tk_any, "an any");
    N::Oops oops;
    oops.code(9);
    Any e;
    e <<= oops;
    N::Oops caught;
    ok((e >>= caught) && caught.code() == 9 && e.type()->kind() == TCKind::tk_except, "an exception");
    N::Structs seq(2);
    seq[1].lmem = 5;
    Any sq;
    sq <<= seq;
    N::Structs seq_back;
    ok((sq >>= seq_back) && seq_back.size() == 2 && seq_back[1].lmem == 5, "a sequence");
    ok(sq.type()->kind() == TCKind::tk_sequence && sq.type()->length() == 0
           && sq.type()->content_type()->equal(N::_tc_MyStruct),
       "its TypeCode is the unbounded sequence of the element's");
    N::Few few;
    few.push_back(1);
    Any fw;
    fw <<= few;
    N::Few few_back;
    ok((fw >>= few_back) && few_back.size() == 1 && fw.type()->kind() == TCKind::tk_sequence
           && fw.type()->length() == 3 && fw.type()->content_type()->equal(CORBA::_tc_long),
       "a bounded sequence carries its bound");
    ok(!(fw >>= seq_back) && seq_back.size() == 2, "a sequence of another type is not extracted");
    N::Row row = {{1, 2, 3, 4}};
    Any rw;
    rw <<= row;
    N::Row row_back = {{0, 0, 0, 0}};
    ok((rw >>= row_back) && row_back[3] == 4 && rw.type()->kind() == TCKind::tk_array && rw.type()->length() == 4,
       "an array");
    N::Brief brief("abc");
    Any bs;
    bs <<= brief;
    N::Brief brief_back;
    ok((bs >>= brief_back) && brief_back == brief && bs.type()->kind() == TCKind::tk_string
           && bs.type()->length() == 8,
       "a bounded string carries its bound");
    // references
    ref_type<N::B> b = CORBA::make_reference<BImpl>();
    Any ob;
    ob <<= b;
    ref_type<N::B> b_back;
    ok((ob >>= b_back) && b_back == b && ob.type()->equal(N::_tc_B), "an object reference");
    ref_type<CORBA::Object> base;
    ok((ob >>= base) && base == b, "any object reference is extractable as a base object reference");
    ref_type<N::A> as_a;
    ok((ob >>= as_a) && as_a == b, "and as the reference of a base interface");
    ref_type<N::Plain> plain = CORBA::make_reference<PlainImpl>();
    plain->n(4);
    Any va;
    va <<= plain;
    ref_type<N::Plain> plain_back;
    ok((va >>= plain_back) && plain_back == plain && va.type()->equal(N::_tc_Plain), "a valuetype reference");
    ref_type<CORBA::ValueBase> value_base;
    ok((va >>= value_base) && value_base == plain,
       "any valuetype reference is extractable as a base valuetype reference");
    ok(!(va >>= base), "a valuetype reference is no object reference");
    ref_type<N::Shape> shape = CORBA::make_reference<DrawingImpl>();
    Any ab;
    ab <<= shape;
    ref_type<N::Shape> shape_back;
    ok((ab >>= shape_back) && shape_back == shape && ab.type()->equal(N::_tc_Shape), "an abstract reference");
    ok((ab >>= base) && base != nullptr && base->_is_a("IDL:N/Drawing:1.0"),
       "an abstract reference is extractable as a base object reference");
    ref_type<N::Shape> square = CORBA::make_reference<SquareImpl>();
    Any sv;
    sv <<= square;
    ok((sv >>= value_base) && CORBA::traits<N::Square>::narrow(value_base) != nullptr,
       "or as a base valuetype reference");

    // A.1.7.4
    SAME(decltype(std::declval<const Any&>().type()), ref_type<CORBA::TypeCode>);
    Any any;
    N::LongType val = 1234;
    any <<= val;
    ok(any.type()->kind() == TCKind::tk_long, "an alias is inserted as its type");
    any.type(N::_tc_LongType);
    ok(any.type()->kind() == TCKind::tk_alias, "the type modifier replaces the TypeCode with an equivalent one");
    std::int32_t still = 0;
    ok((any >>= still) && still == 1234, "the value stays");
    throws<CORBA::BAD_TYPECODE>([&] { any.type(CORBA::_tc_short); }, "a TypeCode that is not equivalent raises BAD_TYPECODE");"##,
    );
    compile_and_run("corba-any", &files, &check, BOTH);
}

const VALUES: &str = r"
module V {
    typedef octet Bytes[64];
    struct S { long a; };
    interface A { void op(); };
    valuetype Val {
        public Val t;
        private long v;
        public Bytes w;
        public string x;
        private S y;
        private A z;
    };
    valuetype Example {
        short op1();
        long op2(in Example x);
        private short val1;
        public long val2;
        private string val3;
        private Example val5;
    };
    enum Color { red, green, blue };
    valuetype ColorValue Color;
    valuetype StringValue string;
    interface X { void op(out string s); };
    abstract valuetype Touchable { void touch(); };
    valuetype Base { public long b1; private string b2; };
    valuetype Derived : Base, Touchable { public short d1; };
    valuetype Trunc : truncatable Base { public long extra; };
    valuetype B supports A { public short data; };
    valuetype Made {
        factory create_bool(in boolean b);
        factory create_other(in short s, in string p);
        public long n;
    };
    valuetype Bare { public long n; };
    valuetype WithOp { void run(); public long n; };
};
";

/// A.1.8: value types — the abstract base class and the OBV class with
/// the accessors of A.1.8.1, the constructors and destructors of A.1.8.2,
/// operations and `narrow` (A.1.8.3), the example of A.1.8.4, `ValueBase`
/// (A.1.8.5), value boxes (A.1.8.6), abstract value types (A.1.8.7),
/// inheritance (A.1.8.8) and the factories of A.1.8.9.
#[test]
fn idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause() {
    let files = cpp(VALUES, &corba());
    let check = check_program(
        &files[0].path,
        r"
using omg::types::ref_type;
#define PUBLIC_MEMBER(NAME)                                                                     \
    template <typename T, typename = void>                                                      \
    struct public_##NAME : std::false_type {};                                                  \
    template <typename T>                                                                       \
    struct public_##NAME<T, decltype(void(std::declval<const T&>().NAME()))> : std::true_type {};
PUBLIC_MEMBER(v)
PUBLIC_MEMBER(x)
PUBLIC_MEMBER(val1)
PUBLIC_MEMBER(val2)
template <typename T, typename = void>
struct has_obv : std::false_type {};
template <typename T>
struct has_obv<T, typename std::conditional<true, void, typename CORBA::traits<T>::obv_type>::type>
    : std::true_type {};

// The protected accessors of A.1.8.1, named from a derived class.
struct ValProbe : V::Val {
    static void check() {
        (void)static_cast<std::int32_t (V::Val::*)() const>(&ValProbe::v);
        (void)static_cast<std::int32_t& (V::Val::*)()>(&ValProbe::v);
        (void)static_cast<void (V::Val::*)(std::int32_t)>(&ValProbe::v);
        (void)static_cast<const V::S& (V::Val::*)() const>(&ValProbe::y);
        (void)static_cast<V::S& (V::Val::*)()>(&ValProbe::y);
        (void)static_cast<void (V::Val::*)(V::S&&)>(&ValProbe::y);
        (void)static_cast<void (V::Val::*)(const V::S&)>(&ValProbe::y);
        (void)static_cast<ref_type<V::A> (V::Val::*)() const>(&ValProbe::z);
        (void)static_cast<ref_type<V::A>& (V::Val::*)()>(&ValProbe::z);
        (void)static_cast<void (V::Val::*)(ref_type<V::A>)>(&ValProbe::z);
    }
};
class ValImpl : public V::OBV_Val {
public:
    ValImpl() {}
    std::int32_t private_v() const { return v(); }
    bool z_is_nil() const { return z() == nullptr; }
};
class ExampleImpl : public V::OBV_Example {
public:
    ExampleImpl() {}
    ExampleImpl(std::int16_t a, std::int32_t b, const std::string& c, ref_type<V::Example> d)
        : V::OBV_Example(a, b, c, d) {}
    std::int16_t op1() override { return val1(); }
    std::int32_t op2(ref_type<V::Example> x) override { return x ? x->val2() : -1; }
    std::string third() const { return val3(); }
    bool fourth_is(const ref_type<V::Example>& other) const { return val5() == other; }
};
class XImpl : public V::X {
public:
    void op(std::string& s) override { s = std::string(1, 'n') + std::string(1, 'e') + std::string(1, 'w'); }
};
class DerivedImpl : public V::OBV_Derived {
public:
    DerivedImpl(std::int32_t b1, const std::string& b2, std::int16_t d1) : V::OBV_Derived(b1, b2, d1) {}
    void touch() override {}
    std::string second() const { return b2(); }
};
class MadeImpl : public V::OBV_Made {
public:
    MadeImpl() {}
    explicit MadeImpl(std::int32_t n) : V::OBV_Made(n) {}
};
class MadeFactory : public V::Made_factory {
public:
    MadeFactory() {}
    ref_type<V::Made> create_bool(bool b) override { return CORBA::make_reference<MadeImpl>(b ? 1 : 0); }
    ref_type<V::Made> create_other(std::int16_t s, const std::string& p) override {
        return CORBA::make_reference<MadeImpl>(s + static_cast<std::int32_t>(p.size()));
    }

private:
    ref_type<CORBA::ValueBase> create_for_unmarshal() override { return CORBA::make_reference<MadeImpl>(); }
};
",
        r##"    using CORBA::traits;
    // A.1.8: the abstract base class, the OBV class, the references
    SAME(traits<V::Example>::base_type, V::Example);
    SAME(traits<V::Example>::obv_type, V::OBV_Example);
    SAME(traits<V::Example>::ref_type, ref_type<V::Example>);
    SAME(traits<V::Example>::weak_ref_type, omg::types::weak_ref_type<V::Example>);
    ok(std::is_abstract<V::Example>::value && std::is_abstract<V::OBV_Example>::value,
       "an abstract base class; the OBV class leaves the operations to the application");
    ok(std::is_base_of<CORBA::ValueBase, V::Example>::value && std::is_base_of<V::Example, V::OBV_Example>::value,
       "the base class derives from ValueBase, the OBV class from the base class");

    // A.1.8.1: public state members, the example of the clause
    (void)static_cast<ref_type<V::Val> (V::Val::*)() const>(&V::Val::t);
    (void)static_cast<ref_type<V::Val>& (V::Val::*)()>(&V::Val::t);
    (void)static_cast<void (V::Val::*)(ref_type<V::Val>)>(&V::Val::t);
    (void)static_cast<const V::Bytes& (V::Val::*)() const>(&V::Val::w);
    (void)static_cast<V::Bytes& (V::Val::*)()>(&V::Val::w);
    (void)static_cast<void (V::Val::*)(const V::Bytes&)>(&V::Val::w);
    (void)static_cast<void (V::Val::*)(V::Bytes&&)>(&V::Val::w);
    (void)static_cast<const std::string& (V::Val::*)() const>(&V::Val::x);
    (void)static_cast<std::string& (V::Val::*)()>(&V::Val::x);
    (void)static_cast<void (V::Val::*)(const std::string&)>(&V::Val::x);
    (void)static_cast<void (V::Val::*)(std::string&&)>(&V::Val::x);
    ValProbe::check();
    ok(public_x<V::Val>::value && !public_v<V::Val>::value, "a private state member has protected accessors");
    ok(public_val2<V::OBV_Example>::value && !public_val1<V::OBV_Example>::value, "in the OBV class too");

    // A.1.8.2
    ok(!std::is_destructible<V::Example>::value && !std::is_destructible<V::OBV_Example>::value
           && std::has_virtual_destructor<V::Example>::value,
       "the destructors are protected and virtual");
    ok(!std::is_copy_assignable<V::Example>::value && !std::is_move_assignable<V::Example>::value,
       "the assignment operators are deleted");
    ref_type<ValImpl> val = CORBA::make_reference<ValImpl>();
    ok(val->t() == nullptr && val->z_is_nil() && val->private_v() == 0 && val->x().empty() && val->w()[63] == 0,
       "the default constructor: nil references, the default values of the basic types");
    val->t(CORBA::make_reference<ValImpl>());
    val->x("set");
    val->w()[0] = 7;
    ok(val->t() != nullptr && val->x() == "set" && val->w()[0] == 7, "the modifiers");
    ref_type<ExampleImpl> empty = CORBA::make_reference<ExampleImpl>();
    ref_type<ExampleImpl> full = CORBA::make_reference<ExampleImpl>(std::int16_t(1), 2, "three", empty);
    ok(full->op1() == 1 && full->val2() == 2 && full->third() == "three" && full->fourth_is(empty),
       "the explicit constructor takes the members in the order of the IDL, private ones included");

    // A.1.8.3, A.1.8.4
    SAME(decltype(&V::Example::op1), std::int16_t (V::Example::*)());
    SAME(decltype(&V::Example::op2), std::int32_t (V::Example::*)(ref_type<V::Example>));
    ok(full->op2(full) == 2 && full->op2(nullptr) == -1, "an operation takes a valuetype reference");
    full->val2() = 9;
    ok(full->val2() == 9, "the accessor that returns a reference");
    ref_type<CORBA::ValueBase> base = full;
    ok(traits<V::Example>::narrow(base) == full && traits<V::Val>::narrow(base) == nullptr
           && traits<V::Example>::narrow(nullptr) == nullptr,
       "narrow: the instance's class, another class, a nil reference");

    // A.1.8.5
    SAME(decltype(&CORBA::ValueBase::_copy_value), ref_type<CORBA::ValueBase> (CORBA::ValueBase::*)());
    ok(base->_copy_value() == nullptr, "_copy_value returns a nil reference by default");
    ok(!std::is_destructible<CORBA::ValueBase>::value && !std::is_copy_assignable<CORBA::ValueBase>::value
           && !std::is_move_assignable<CORBA::ValueBase>::value,
       "ValueBase: a protected destructor, deleted assignment");

    // A.1.8.6
    ok(std::is_base_of<CORBA::ValueBase, V::ColorValue>::value && !std::is_abstract<V::ColorValue>::value,
       "a value box is a concrete class on ValueBase");
#if __cplusplus >= 201402L
    ok(std::is_final<V::ColorValue>::value, "and final");
#endif
    ok(!std::is_constructible<V::ColorValue, V::Color>::value && !std::is_destructible<V::ColorValue>::value,
       "its constructors and its destructor are protected");
    ok(!std::is_copy_assignable<V::ColorValue>::value && !std::is_move_assignable<V::ColorValue>::value,
       "its assignment operators are deleted");
    (void)static_cast<V::Color (V::ColorValue::*)() const>(&V::ColorValue::_value);
    (void)static_cast<V::Color& (V::ColorValue::*)()>(&V::ColorValue::_value);
    (void)static_cast<void (V::ColorValue::*)(V::Color)>(&V::ColorValue::_value);
    (void)static_cast<const std::string& (V::StringValue::*)() const>(&V::StringValue::_value);
    (void)static_cast<std::string& (V::StringValue::*)()>(&V::StringValue::_value);
    (void)static_cast<void (V::StringValue::*)(const std::string&)>(&V::StringValue::_value);
    (void)static_cast<void (V::StringValue::*)(std::string&&)>(&V::StringValue::_value);
    ref_type<V::ColorValue> color = CORBA::make_reference<V::ColorValue>(V::Color::green);
    ref_type<V::ColorValue> first = CORBA::make_reference<V::ColorValue>();
    ok(color->_value() == V::Color::green && first->_value() == V::Color::red, "made by make_reference");
    color->_value(V::Color::blue);
    ok(color->_value() == V::Color::blue, "the modifier");
    ref_type<V::StringValue> sval = CORBA::make_reference<V::StringValue>("string val");
    ref_type<V::X> x = CORBA::make_reference<XImpl>();
    x->op(sval->_value());
    ok(sval->_value() == "new", "the boxed string is replaced by the invocation of op()");
    SAME(traits<V::StringValue>::boxed_traits, traits<std::string>);
    ref_type<CORBA::ValueBase> boxed = sval;
    ok(boxed != nullptr, "a value box widens to ValueBase");

    // A.1.8.7
    ok(has_obv<V::Example>::value && !has_obv<V::Touchable>::value, "no OBV trait for an abstract valuetype");
    ok(traits<V::Touchable>::is_abstract::value && !traits<V::Example>::is_abstract::value, "is_abstract");

    // A.1.8.8
    ok(std::is_base_of<V::Base, V::Derived>::value && std::is_base_of<V::Touchable, V::Derived>::value,
       "the concrete and the abstract value base are base classes");
    ok(std::is_base_of<V::OBV_Base, V::OBV_Derived>::value && std::is_base_of<V::Derived, V::OBV_Derived>::value,
       "ladder style: the OBV class derives from its base's OBV class");
    ref_type<DerivedImpl> derived = CORBA::make_reference<DerivedImpl>(1, "two", std::int16_t(3));
    ok(derived->b1() == 1 && derived->second() == "two" && derived->d1() == 3,
       "the members of the inherited valuetype appear first in the constructor");
    ref_type<V::Base> as_base = derived;
    ref_type<V::Touchable> as_touchable = derived;
    ok(traits<V::Derived>::narrow(as_base) == derived && traits<V::Derived>::narrow(as_touchable) == derived,
       "widening to the bases, narrowing back");
    ok(traits<V::Trunc>::is_truncatable::value && !traits<V::Derived>::is_truncatable::value, "is_truncatable");
    ok(!std::is_base_of<V::A, V::B>::value, "the class of a supported interface is not inherited");
    SAME(decltype(&V::B::op), void (V::B::*)());

    // A.1.8.9
    ok(std::is_abstract<CORBA::ValueFactoryBase>::value
           && !std::is_copy_assignable<CORBA::ValueFactoryBase>::value
           && !std::is_destructible<CORBA::ValueFactoryBase>::value,
       "ValueFactoryBase");
    SAME(traits<V::Made>::factory_type, V::Made_factory);
    ok(std::is_base_of<CORBA::ValueFactoryBase, V::Made_factory>::value && std::is_abstract<V::Made_factory>::value
           && !std::is_destructible<V::Made_factory>::value,
       "the factory class of the initializers");
    SAME(decltype(&V::Made_factory::create_bool), ref_type<V::Made> (V::Made_factory::*)(bool));
    SAME(decltype(&V::Made_factory::create_other),
         ref_type<V::Made> (V::Made_factory::*)(std::int16_t, const std::string&));
    ref_type<V::Made_factory> factory = CORBA::make_reference<MadeFactory>();
    ok(factory->create_other(2, "abc")->n() == 5 && factory->create_bool(true)->n() == 1,
       "an application's factory makes the values");
    ref_type<CORBA::ValueBase> unmarshalled = CORBA::detail::factory_access::create_for_unmarshal(*factory);
    ok(traits<V::Made>::narrow(unmarshalled) != nullptr, "create_for_unmarshal, called by the implementation");
    SAME(traits<V::Bare>::factory_type, V::Bare_factory);
    ok(!std::is_abstract<V::Bare_factory>::value, "a valuetype without operations and initializers has a concrete factory");
    ref_type<V::Bare_factory> bare_factory = CORBA::make_reference<V::Bare_factory>();
    ref_type<V::Bare> bare = traits<V::Bare>::narrow(CORBA::detail::factory_access::create_for_unmarshal(*bare_factory));
    ok(bare != nullptr && bare->n() == 0, "which constructs an instance of the OBV class");
    SAME(traits<V::WithOp>::factory_type, CORBA::ValueFactoryBase);"##,
    );
    compile_and_run("corba-values", &files, &check, BOTH);

    // A.1.8.10: "The C++ mappings for the IDL CORBA::CustomerMarshal,
    // CORBA::DataOutputStream, and CORBA::DataInputStream types follow
    // normal C++ valuetype mapping rules."
    let custom = check_program(
        "omg/corba_module.hpp",
        "",
        r#"    using omg::types::ref_type;
    ok(std::is_base_of<CORBA::ValueBase, CORBA::CustomMarshal>::value && std::is_abstract<CORBA::CustomMarshal>::value,
       "CustomMarshal is the class of an abstract valuetype");
    SAME(decltype(&CORBA::CustomMarshal::marshal), void (CORBA::CustomMarshal::*)(ref_type<CORBA::DataOutputStream>));
    SAME(decltype(&CORBA::CustomMarshal::unmarshal), void (CORBA::CustomMarshal::*)(ref_type<CORBA::DataInputStream>));
    ok(std::is_base_of<CORBA::ValueBase, CORBA::DataOutputStream>::value
           && std::is_base_of<CORBA::ValueBase, CORBA::DataInputStream>::value,
       "the streams are the classes of abstract valuetypes");
    SAME(decltype(&CORBA::DataOutputStream::write_long), void (CORBA::DataOutputStream::*)(std::int32_t));
    SAME(decltype(&CORBA::DataInputStream::read_long), std::int32_t (CORBA::DataInputStream::*)());
    ok(CORBA::traits<CORBA::CustomMarshal>::is_abstract::value, "with their traits");"#,
    );
    compile_and_run("corba-custom", &[], &custom, BOTH);
}

const ABSTRACTS: &str = r"
module W {
    abstract interface Shape { long sides(); };
    abstract interface Named : Shape { string name(); };
    interface Drawing : Named { void draw(); };
    interface Plain { void fp(); };
    interface Both : Plain, Shape { };
    valuetype Square supports Named { public long edge; };
    valuetype Cube : Square { public long depth; };
};
";

/// A.1.9: abstract interfaces — `CORBA::AbstractBase` (A.1.9.1), and the
/// differences of the client side mapping to an interface's (A.1.9.2):
/// the base, the protected constructors and destructor, virtual
/// inheritance, the Any operators and the implicit widening of derived
/// interfaces and supporting value types.
#[test]
fn idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base() {
    let files = cpp(ABSTRACTS, &corba());
    let check = check_program(
        &files[0].path,
        r#"
using omg::types::ref_type;
class DrawingImpl : public W::Drawing {
public:
    std::int32_t sides() override { return 0; }
    std::string name() override { return "drawing"; }
    void draw() override {}
};
class BothImpl : public W::Both {
public:
    std::int32_t sides() override { return 2; }
    void fp() override {}
};
class SquareImpl : public W::OBV_Square {
public:
    SquareImpl() {}
    std::int32_t sides() override { return 4; }
    std::string name() override { return "square"; }
};
"#,
        r##"    using CORBA::traits;
    using CORBA::AbstractBase;
    // A.1.9.1
    SAME(traits<W::Shape>::ref_type, ref_type<W::Shape>);
    SAME(traits<W::Shape>::weak_ref_type, omg::types::weak_ref_type<W::Shape>);
    SAME(traits<W::Shape>::base_type, W::Shape);
    ok(traits<W::Shape>::is_abstract::value && !traits<W::Drawing>::is_abstract::value, "is_abstract");
    ok(!std::is_base_of<CORBA::Object, W::Shape>::value && std::is_base_of<AbstractBase, W::Shape>::value,
       "the class of an abstract interface derives from AbstractBase, not from Object");
    ok(std::is_base_of<W::Shape, W::Named>::value && !std::is_base_of<CORBA::Object, W::Named>::value,
       "a derived abstract interface derives from its base");
    SAME(decltype(&AbstractBase::_to_object), ref_type<CORBA::Object> (AbstractBase::*)());
    SAME(decltype(&AbstractBase::_to_value), ref_type<CORBA::ValueBase> (AbstractBase::*)());
    ok(!std::is_destructible<AbstractBase>::value && std::has_virtual_destructor<AbstractBase>::value,
       "AbstractBase: a protected virtual destructor");
    // A.1.9.2
    ok(!std::is_destructible<W::Shape>::value && std::has_virtual_destructor<W::Shape>::value,
       "an abstract interface class has a protected destructor");
    ok(std::is_base_of<CORBA::Object, W::Drawing>::value && std::is_base_of<W::Named, W::Drawing>::value,
       "an interface derived from an abstract interface derives from it and from Object");
    ok(std::is_base_of<W::Plain, W::Both>::value && std::is_base_of<W::Shape, W::Both>::value,
       "from an interface and an abstract interface");
    ok(std::is_base_of<W::Named, W::Square>::value && std::is_base_of<CORBA::ValueBase, W::Square>::value,
       "a valuetype that supports an abstract interface derives from it");
    ref_type<W::Drawing> drawing = CORBA::make_reference<DrawingImpl>();
    ref_type<W::Named> named = drawing;
    ref_type<W::Shape> shape = drawing;
    ref_type<W::Square> square = CORBA::make_reference<SquareImpl>();
    ref_type<W::Named> named_value = square;
    ref_type<W::Shape> shape_value = square;
    ref_type<W::Shape> shape_both = CORBA::make_reference<BothImpl>();
    ok(shape->sides() == 0 && shape_value->sides() == 4 && shape_both->sides() == 2 && named->name() == "drawing"
           && named_value->name() == "square",
       "implicit widening to each abstract interface base, the operations through it");
    ok(shape->_to_object() == drawing && shape->_to_value() == nullptr,
       "_to_object returns the object, _to_value a nil reference");
    ok(shape_value->_to_value() == square && shape_value->_to_object() == nullptr,
       "_to_value returns the valuetype, _to_object a nil reference");
    ok(shape_both->_to_object() != nullptr, "an interface with a concrete and an abstract base");
    ref_type<AbstractBase> base = shape;
    ref_type<AbstractBase> base_value = shape_value;
    ok(traits<W::Named>::narrow(base) == named && traits<W::Shape>::narrow(base) == shape,
       "narrowing to a derived abstract interface");
    ok(traits<W::Drawing>::narrow(base) == drawing && traits<W::Both>::narrow(base) == nullptr,
       "narrowing to a derived interface");
    ok(traits<W::Square>::narrow(base_value) == square && traits<W::Square>::narrow(base) == nullptr
           && traits<W::Cube>::narrow(base_value) == nullptr,
       "narrowing to a derived valuetype");
    ok(drawing->_is_a("IDL:W/Named:1.0") && drawing->_is_a("IDL:W/Shape:1.0")
           && drawing->_is_a("IDL:omg.org/CORBA/Object:1.0") && !drawing->_is_a("IDL:W/Plain:1.0"),
       "_is_a answers for the abstract bases");
    // "Normal Any insertion and extraction operators are generated for abstract interfaces."
    CORBA::Any any;
    any <<= shape;
    ref_type<W::Shape> back;
    ok((any >>= back) && back == shape && any.type()->equal(W::_tc_Shape), "the Any operators");
    ref_type<W::Named> named_back;
    ok((any >>= named_back) && named_back == named, "extraction as a derived abstract interface");"##,
    );
    compile_and_run("corba-abstract", &files, &check, BOTH);
}
