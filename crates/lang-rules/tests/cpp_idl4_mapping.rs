// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C++ Language Mapping 1.0 (OMG formal/25-03-03), clauses 7 and
//! 8, on the specification's own examples.
//!
//! Each test generates the C++ header of an IDL file, compiles it with
//! the `omg::types` runtime (`runtime/cpp/include/omg/types.hpp`) and a
//! check program whose expectations are the clause's text — the types,
//! signatures and names its examples print, the exceptions its sentences
//! name — under `-std=c++11` and `-std=c++17` with every warning an
//! error, and runs it.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "cpp-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, cpp, header};
use zerodds_lang_rules::emit::idl4_cpp::Options;

const BOTH: &[&str] = &["c++11", "c++17"];

fn run(tag: &str, idl: &str, declarations: &str, body: &str) {
    let files = cpp(idl, &Options::default());
    let check = check_program(&files[0].path, declarations, body);
    compile_and_run(tag, &files, &check, BOTH);
}

/// §7.2.2 and §7.2.3: a namespace per module, `constexpr` constants,
/// `omg::types::string_view` for a string, `L` before a wide one.
#[test]
fn idl4cpp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_are_constexpr() {
    let idl = r#"module my_math {
            const string my_string = "My String Value";
            const double PI = 3.141592;
            const wstring ws = L"Hello World";
            const float F = 1.5;
            const long long LL_MIN = -9223372036854775807 - 1;
            const unsigned long long ULL_MAX = 18446744073709551615;
            const long L_MIN = -2147483647 - 1;
            const char C = 'x';
            const wchar W = L'y';
            const boolean B = TRUE;
            const octet O = 255;
            const int8 I8 = -128;
            enum Color { red, green };
            const Color FAVOURITE = green;
            const fixed F52 = 123.45d;
        };
        const long GLOBAL = 7;"#;
    run(
        "constants",
        idl,
        r#"static_assert(my_math::PI > 3.14 && my_math::PI < 3.15, "constexpr double");
static_assert(my_math::my_string.size() == 15, "constexpr string_view");
static_assert(::GLOBAL == 7, "a declaration outside any module is in the global scope");
static_assert(my_math::FAVOURITE == my_math::Color::green, "an enumerator constant");"#,
        r#"    SAME(decltype(my_math::PI), const double);
    SAME(decltype(my_math::my_string), const omg::types::string_view);
    SAME(decltype(my_math::ws), const omg::types::wstring_view);
    ok(std::string(my_math::my_string.data(), my_math::my_string.size()) == "My String Value", "my_string");
    ok(std::wstring(my_math::ws.data(), my_math::ws.size()) == L"Hello World", "ws");
    SAME(decltype(my_math::F), const float);
    ok(my_math::F == 1.5f, "F");
    ok(my_math::LL_MIN == INT64_MIN && my_math::ULL_MAX == UINT64_MAX, "the 64-bit extremes");
    ok(my_math::L_MIN == INT32_MIN, "the 32-bit minimum");
    ok(my_math::C == 'x' && my_math::W == L'y' && my_math::B, "char, wchar, boolean");
    SAME(decltype(my_math::O), const std::uint8_t);
    ok(my_math::O == 255 && my_math::I8 == -128, "octet and int8");
    ok(my_math::F52.to_string() == "123.45", "a fixed constant");"#,
    );
}

/// §7.2.4.1, §7.14.4, §7.14.5: Tables 7.2, 7.3, 7.13 and 7.14, and the
/// default value 0 of each.
#[test]
fn idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14() {
    let idl = "struct Basic {
            short s; unsigned short us; long l; unsigned long ul;
            long long ll; unsigned long long ull;
            float f; double d; long double ld;
            char c; wchar w; boolean b; octet o;
            int8 i8; uint8 u8; int16 i16; uint16 u16; int32 i32; uint32 u32;
            int64 i64; uint64 u64;
        };";
    run(
        "basic",
        idl,
        "",
        r#"    SAME(decltype(Basic::s), std::int16_t);
    SAME(decltype(Basic::us), std::uint16_t);
    SAME(decltype(Basic::l), std::int32_t);
    SAME(decltype(Basic::ul), std::uint32_t);
    SAME(decltype(Basic::ll), std::int64_t);
    SAME(decltype(Basic::ull), std::uint64_t);
    SAME(decltype(Basic::f), float);
    SAME(decltype(Basic::d), double);
    SAME(decltype(Basic::ld), long double);
    SAME(decltype(Basic::c), char);
    SAME(decltype(Basic::w), wchar_t);
    SAME(decltype(Basic::b), bool);
    SAME(decltype(Basic::o), std::uint8_t);
    SAME(decltype(Basic::i8), std::int8_t);
    SAME(decltype(Basic::u8), std::uint8_t);
    SAME(decltype(Basic::i16), std::int16_t);
    SAME(decltype(Basic::u16), std::uint16_t);
    SAME(decltype(Basic::i32), std::int32_t);
    SAME(decltype(Basic::u32), std::uint32_t);
    SAME(decltype(Basic::i64), std::int64_t);
    SAME(decltype(Basic::u64), std::uint64_t);
    Basic b;
    ok(b.s == 0 && b.ul == 0 && b.ull == 0 && b.f == 0 && b.ld == 0 && b.c == 0 && b.w == 0 && !b.b && b.o == 0 && b.i8 == 0,
       "every basic member starts at 0");"#,
    );
}

/// §7.2.4.2 and §7.14.3.1: sequences, strings, wide strings and maps,
/// bounded and unbounded, with the traits of Tables 7.4, 7.5, 7.6, 7.10
/// and 7.11, and the bound check of the NOTEs.
#[test]
fn idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits() {
    let idl = "struct T { long x; };
        typedef sequence<T> V1;
        typedef sequence<T, 3> V2;
        typedef sequence<V1> V3;
        typedef string S1;
        typedef string<5> S2;
        typedef wstring W1;
        typedef wstring<4> W2;
        typedef map<unsigned long, T> M1;
        typedef map<string, T, 2> M2;";
    run(
        "templates",
        idl,
        "",
        r#"    SAME(V1, omg::types::sequence<T>);
    SAME(V1, std::vector<T>);
    SAME(V2, omg::types::bounded_sequence<T, 3>);
    SAME(V3, omg::types::sequence<V1>);
    SAME(S1, std::string);
    SAME(S2, omg::types::bounded_string<5>);
    SAME(W1, std::wstring);
    SAME(W2, omg::types::bounded_wstring<4>);
    SAME(M1, std::map<std::uint32_t, T>);
    SAME(M2, omg::types::bounded_map<std::string, T, 2>);
    ok(!omg::types::is_bounded<V1>::value && omg::types::is_bounded<V2>::value, "is_bounded of a sequence");
    ok(omg::types::bound<V1>::value == std::numeric_limits<std::size_t>::max() && omg::types::bound<V2>::value == 3,
       "bound of a sequence");
    ok(!omg::types::is_bounded<S1>::value && omg::types::bound<S2>::value == 5, "string traits");
    ok(!omg::types::is_bounded<W1>::value && omg::types::bound<W2>::value == 4, "wstring traits");
    ok(!omg::types::is_bounded<M1>::value && omg::types::bound<M2>::value == 2, "map traits");
    SAME(omg::types::key<M2>::type, std::string);
    SAME(omg::types::elements<M1>::type, T);
    V2 v2;
    v2.push_back(T());
    v2.push_back(T());
    v2.push_back(T());
    throws<std::length_error>([&] { v2.push_back(T()); }, "a fourth element in sequence<T, 3>");
    ok(v2.size() == 3, "the refused element left the sequence as it was");
    std::vector<T> plain = v2;
    ok(plain.size() == 3, "a bounded sequence converts to std::vector");
    throws<std::length_error>([&] { V2 four(std::vector<T>(4)); }, "a std::vector of four into sequence<T, 3>");
    S2 s2("hello");
    throws<std::length_error>([&] { s2 += "!"; }, "a sixth character in string<5>");
    ok(std::string(s2) == "hello", "a bounded string converts to std::string");
    W2 w2(L"abcd");
    throws<std::length_error>([&] { w2.push_back(L'e'); }, "a fifth character in wstring<4>");
    M2 m2;
    T t;
    t.x = 1;
    m2["a"] = t;
    m2["b"] = t;
    m2["a"] = t;
    throws<std::length_error>([&] { m2["c"] = T(); }, "a third key in map<string, T, 2>");
    ok(m2.size() == 2, "a replaced key is no new entry");"#,
    );
}

/// §7.2.4.2.4: `omg::types::fixed`, its operators and conversions, and
/// the traits of Table 7.7 for a `fixed<5,2>`.
#[test]
fn idl4cpp_7_2_4_2_4_fixed_is_omg_types_fixed_with_digits_and_scale() {
    let idl = "typedef fixed<5,2> F;
        interface AnInterface { void op(in F arg); };";
    run(
        "fixed",
        idl,
        "",
        r#"    ok(omg::types::digits<F>::value == 5 && omg::types::scale<F>::value == 2, "digits and scale");
    SAME(decltype(&AnInterface::op), void (AnInterface::*)(const F&));
    F a("123.45");
    F b(omg::types::fixed("1.5"));
    ok(a.fixed_digits() == 5 && a.fixed_scale() == 2, "a fixed<5,2> value");
    ok(b.to_string() == "1.50", "1.5 as a fixed<5,2> is 1.50");
    omg::types::fixed sum = a + b;
    ok(sum.to_string() == "124.95", "123.45 + 1.50");
    ok((a - b).to_string() == "121.95", "123.45 - 1.50");
    ok((a * b).to_string() == "185.1750", "123.45 * 1.50");
    ok((omg::types::fixed("1") / omg::types::fixed("4")).to_string() == "0.25", "1 / 4");
    ok((omg::types::fixed("1") / omg::types::fixed("3")).fixed_digits() == 31, "1 / 3 keeps 31 digits");
    ok(a > b && b < a && a != b && a == F("123.45") && a >= a && b <= b, "comparisons");
    ok(static_cast<std::int64_t>(a) == 123, "the conversion to int64_t truncates");
    ok(static_cast<long double>(b) == 1.5L, "the conversion to long double");
    ok(omg::types::fixed("2.345").round(2).to_string() == "2.35", "round");
    ok(omg::types::fixed("2.345").truncate(2).to_string() == "2.34", "truncate");
    ok(omg::types::fixed("-0.5").to_string() == "-0.5" && (-omg::types::fixed("0.5")).to_string() == "-0.5", "a negative value");
    omg::types::fixed c("9.99");
    ++c;
    ok(c.to_string() == "10.99", "++");
    ok(!omg::types::fixed("0.00") && static_cast<bool>(c), "operator bool");
    ok(omg::types::fixed(42).to_string() == "42" && omg::types::fixed(2.5).to_string() == "2.5", "from an integer and a double");
    throws<std::range_error>([] { F too_big("1234.5"); }, "four integer digits in a fixed<5,2>");
    std::ostringstream out;
    out << a;
    ok(out.str() == "123.45", "operator<<");
    std::istringstream in("7.25");
    omg::types::fixed read;
    in >> read;
    ok(read.to_string() == "7.25", "operator>>");
    omg::types::fixed x("1"), y("2");
    swap(x, y);
    ok(x.to_string() == "2" && y.to_string() == "1", "swap");"#,
    );
}

/// §7.2.4.3.1 and §7.14.1: the structure of the clause's example, its
/// default values, copies, comparison and `swap`, a derived structure.
#[test]
fn idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap() {
    let idl = r#"enum Color { @value(5) red, @value(9) green };
        struct MyStruct {
            long a_long;
            short a_short;
            string a_string;
            ::Color color;
            @default(42) long answer;
            sequence<long> numbers;
            long matrix[2][3];
        };
        struct ChildStruct : MyStruct {
            float a_float;
        };"#;
    run(
        "struct",
        idl,
        "",
        r#"    SAME(decltype(MyStruct::a_long), std::int32_t);
    SAME(decltype(MyStruct::a_short), std::int16_t);
    SAME(decltype(MyStruct::matrix), omg::types::array<omg::types::array<std::int32_t, 3>, 2>);
    MyStruct m;
    ok(m.a_long == 0 && m.a_short == 0 && m.a_string.empty(), "built-in members start at 0");
    ok(m.color == Color::red, "an enumeration member starts at its first enumerator");
    ok(m.answer == 42, "@default(42)");
    m.a_long = 7;
    m.a_string = "seven";
    m.numbers.push_back(1);
    m.matrix[1][2] = 3;
    MyStruct copy(m);
    ok(copy == m && !(copy != m), "a copy is equal");
    copy.numbers.push_back(2);
    ok(copy != m && m.numbers.size() == 1, "a deep copy");
    MyStruct moved(std::move(copy));
    ok(moved.numbers.size() == 2, "the move constructor");
    MyStruct other;
    swap(m, other);
    ok(other.a_long == 7 && m.a_long == 0, "swap in the namespace of the struct");
    ChildStruct c;
    ok(std::is_base_of<MyStruct, ChildStruct>::value, "ChildStruct derives from MyStruct");
    c.a_long = 1;
    c.a_float = 2.5f;
    ChildStruct d;
    swap(c, d);
    ok(d.a_long == 1 && d.a_float == 2.5f && c.a_long == 0, "swap swaps the inherited members too");
    ChildStruct e(d);
    ok(e == d, "a derived structure compares its base");
    e.a_long = 2;
    ok(e != d, "and a difference in the base makes it unequal");"#,
    );
}

/// §7.2.4.3.2 and §7.14.2: the two unions of the clause's examples, and
/// discriminators of the types §7.14.2 adds.
#[test]
fn idl4cpp_7_2_4_3_2_a_union_is_a_class_with_d_accessors_and_modifiers() {
    let idl = r#"struct AStruct { long a; };
        module with_default {
            union AUnion switch (octet) {
                case 1: long a_long;
                case 2: case 3: short a_short;
                case 4: AStruct a_struct;
                default: octet a_byte_default;
            };
        };
        module without_default {
            union AUnion switch (octet) {
                case 1: long a_long;
                case 2: case 3: short a_short;
            };
        };
        enum Kind { ONE, TWO, THREE };
        union ByEnum switch (Kind) { case ONE: string text; case TWO: case THREE: double number; };
        union ByChar switch (char) { case 'a': long x; };
        union ByWChar switch (wchar) { case L'a': long x; default: short y; };
        union ByBool switch (boolean) { case TRUE: long yes; };"#;
    run(
        "union",
        idl,
        "",
        r#"    using with_default::AUnion;
    AUnion u;
    SAME(decltype(static_cast<const AUnion&>(u)._d()), std::uint8_t);
    ok(u._d() != 1 && u._d() != 2 && u._d() != 3 && u._d() != 4, "the default case: a value no label takes");
    ok(u.a_byte_default() == 0, "and its member, default-initialized");
    u.a_long(42);
    ok(u._d() == 1 && u.a_long() == 42, "a_long(42) selects a_long");
    SAME(decltype(static_cast<const AUnion&>(u).a_long()), std::int32_t);
    u.a_short(7);
    ok(u._d() == 2 && u.a_short() == 7, "a_short(7) takes the first label");
    u.a_short(8, 3);
    ok(u._d() == 3 && u.a_short() == 8, "a_short(8, 3): a member with two labels");
    throws<std::invalid_argument>([&] { u.a_short(9, 1); }, "a discriminator of another member");
    u._d(2);
    ok(u._d() == 2 && u.a_short() == 8, "_d(2) keeps the selected member");
    throws<std::invalid_argument>([&] { u._d(1); }, "_d to a value of another member");
    throws<std::logic_error>([&] { (void)u.a_long(); }, "the accessor of a member not selected");
    AStruct s;
    s.a = 5;
    u.a_struct(s);
    ok(u._d() == 4 && u.a_struct().a == 5, "a_struct");
    u.a_struct().a = 6;
    ok(u.a_struct().a == 6, "the accessor of a struct member returns a reference");
    AUnion copy(u);
    ok(copy == u, "a copy is equal");
    copy.a_long(1);
    ok(copy != u && u.a_struct().a == 6, "and independent");
    AUnion other;
    swap(u, other);
    ok(other._d() == 4 && other.a_struct().a == 6, "swap");
    u.a_byte_default(9, 200);
    ok(u._d() == 200 && u.a_byte_default() == 9, "the default member with a discriminator");
    without_default::AUnion v;
    ok(v._d() == 1 && v.a_long() == 0, "no default case: the first discriminant value");
    v._default();
    ok(v._d() != 1 && v._d() != 2 && v._d() != 3, "_default() sets a legal default value");
    ByEnum e;
    ok(e._d() == Kind::ONE && e.text().empty(), "an enumeration discriminator");
    e.number(1.5, Kind::THREE);
    ok(e._d() == Kind::THREE && e.number() == 1.5, "an enumeration label");
    ByChar c;
    ok(c._d() == 'a', "a char discriminator");
    ByWChar w;
    ok(w._d() != L'a' && w.y() == 0, "a wchar discriminator (7.14.2)");
    ByBool b;
    ok(b._d() && b.yes() == 0, "a boolean discriminator");
    b._default();
    ok(!b._d(), "_default() on a boolean union is FALSE");"#,
    );
}

/// §7.2.4.3.3 and Table 7.18: an `enum class`, `@value`, and under
/// `@bit_bound` the underlying type and the traits of Table 7.8.
#[test]
fn idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound() {
    let idl = "enum AnEnum { zero, one, two };
        enum Color { @value(1) red, @value(2) green, @value(3) blue };
        @bit_bound(6) enum ABoundEnum { @value(1) one_, @value(2) two_ };
        @bit_bound(12) enum AShort { s };
        @bit_bound(20) enum AnInt { i };";
    run(
        "enum",
        idl,
        "",
        r#"    ok(std::is_enum<AnEnum>::value && !std::is_convertible<AnEnum, int>::value, "a scoped enum class");
    ok(static_cast<int>(AnEnum::two) == 2, "AnEnum::two");
    ok(static_cast<int>(Color::red) == 1 && static_cast<int>(Color::blue) == 3, "@value");
    SAME(std::underlying_type<ABoundEnum>::type, std::int8_t);
    SAME(std::underlying_type<AShort>::type, std::int16_t);
    SAME(std::underlying_type<AnInt>::type, std::int32_t);
    ok(omg::types::bit_bound<ABoundEnum>::value == 6, "the bit_bound trait");
    SAME(omg::types::underlying_type<ABoundEnum>::type, std::int8_t);"#,
    );
}

/// §7.2.4.3.4: a structure that holds a sequence of itself, declared
/// forward ahead of the sequence.
#[test]
fn idl4cpp_7_2_4_3_4_a_recursive_type_maps_directly() {
    let idl = "struct Node;
        typedef sequence<Node> NodeSeq;
        struct Node { long value; NodeSeq children; };";
    run(
        "recursive",
        idl,
        "",
        r#"    Node root;
    root.children.push_back(Node());
    root.children[0].value = 3;
    root.children[0].children.push_back(Node());
    Node copy = root;
    ok(copy == root && copy.children[0].children.size() == 1, "a tree of nodes");"#,
    );
}

/// §7.2.4.4 and §7.2.4.6: the clause's array typedefs, nested
/// `std::array`, the `dimensions` trait, and typedefs as aliases.
#[test]
fn idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases() {
    let idl = "typedef long long_array[100];
        typedef string string_array[1][2];
        typedef long Length;
        struct MyType { Length my_type_length; };";
    run(
        "arrays",
        idl,
        "",
        r#"    SAME(long_array, std::array<std::int32_t, 100>);
    SAME(string_array, std::array<std::array<std::string, 2>, 1>);
    ok(omg::types::dimensions<long_array>::value == 1 && omg::types::dimensions<string_array>::value == 2, "dimensions");
    SAME(Length, std::int32_t);
    SAME(decltype(MyType::my_type_length), Length);"#,
    );
}

/// §7.3: `omg::types::Any` holds a value and its type.
#[test]
fn idl4cpp_7_3_any_is_omg_types_any() {
    let idl = "struct Holder { any value; };";
    run(
        "any",
        idl,
        "",
        r#"    SAME(decltype(Holder::value), omg::types::Any);
    Holder h;
    ok(!h.value.has_value() && h.value.type() == typeid(void), "an empty any");
    h.value <<= std::int32_t(5);
    std::int32_t five = 0;
    ok((h.value >>= five) && five == 5, "insert and extract");
    std::string text;
    ok(!(h.value >>= text), "the extraction of another type fails");
    ok(h.value.type() == typeid(std::int32_t), "the actual type");
    Holder same(h);
    ok(same == h, "a copy of a structure with an any member is equal");
    same.value <<= std::int32_t(6);
    ok(same != h, "another value of the type is not");
    same.value <<= std::string("5");
    ok(same != h, "nor a value of another type");
    ok(Holder() == Holder() && Holder() != h, "two empty anys are equal, an empty and a full one not");"#,
    );
}

/// §7.4, §7.4.2: the interface of the clause's example, an interface
/// parameter as `omg::types::ref_type<T>`, a forward declaration, a
/// derived interface.
#[test]
fn idl4cpp_7_4_an_interface_is_a_class_of_pure_virtual_methods() {
    let idl = "struct MyStruct { long a; };
        interface Foo;
        interface AnInterface {
            attribute long attr;
            readonly attribute long ro_attr;
            attribute MyStruct s_attr;
            void op1(in long i_param, in MyStruct si_param, inout long io_param,
                     inout MyStruct ios_param, out long o_param, out MyStruct so_param);
            Foo make(in Foo other);
        };
        interface Foo { long value(); };
        interface Derived : AnInterface { void more(); };";
    run(
        "interface",
        idl,
        r#"class Impl : public Derived {
public:
    std::int32_t attr_ = 0;
    void attr(std::int32_t value) override { attr_ = value; }
    std::int32_t attr() const override { return attr_; }
    std::int32_t ro_attr() const override { return 9; }
    void s_attr(const MyStruct&) override {}
    MyStruct s_attr() const override { return MyStruct(); }
    void op1(std::int32_t i, const MyStruct& si, std::int32_t& io, MyStruct& ios, std::int32_t& o, MyStruct& so) override {
        io += i;
        ios.a = si.a;
        o = 3;
        so.a = 4;
    }
    omg::types::ref_type<Foo> make(const omg::types::ref_type<Foo>& other) override { return other; }
    void more() override {}
};"#,
        r#"    SAME(decltype(&AnInterface::op1), void (AnInterface::*)(std::int32_t, const MyStruct&, std::int32_t&, MyStruct&, std::int32_t&, MyStruct&));
    SAME(decltype(static_cast<std::int32_t (AnInterface::*)() const>(&AnInterface::attr)), std::int32_t (AnInterface::*)() const);
    SAME(decltype(static_cast<void (AnInterface::*)(std::int32_t)>(&AnInterface::attr)), void (AnInterface::*)(std::int32_t));
    SAME(decltype(static_cast<void (AnInterface::*)(const MyStruct&)>(&AnInterface::s_attr)), void (AnInterface::*)(const MyStruct&));
    SAME(decltype(&AnInterface::make), omg::types::ref_type<Foo> (AnInterface::*)(const omg::types::ref_type<Foo>&));
    ok(std::is_abstract<AnInterface>::value && std::is_polymorphic<AnInterface>::value, "an abstract class");
    ok(std::is_base_of<AnInterface, Derived>::value, "a derived interface extends the base's class");
    Impl impl;
    AnInterface& i = impl;
    i.attr(5);
    std::int32_t io = 1, o = 0;
    MyStruct si, ios, so;
    si.a = 2;
    i.op1(10, si, io, ios, o, so);
    ok(i.attr() == 5 && io == 11 && ios.a == 2 && o == 3 && so.a == 4, "in, inout and out");
    omg::types::ref_type<Foo> none;
    ok(!i.make(none), "a nil reference");"#,
    );
}

/// §7.4.1: an exception class on `std::exception`, its constructors,
/// accessors and modifiers, and `what()`.
#[test]
fn idl4cpp_7_4_1_an_exception_extends_std_exception() {
    let idl = "exception AnException { long error_code; string reason; };
        exception Empty {};
        interface MyInterfaceException { void op1(in long in_param) raises (AnException); };";
    run(
        "exception",
        idl,
        "",
        r#"    ok(std::is_base_of<std::exception, AnException>::value, "AnException : std::exception");
    AnException e;
    ok(e.error_code() == 0 && e.reason().empty(), "members start at their default value");
    AnException f(7, "seven", "explained");
    ok(f.error_code() == 7 && f.reason() == "seven" && std::string(f.what()) == "explained", "the explicit constructor");
    f.error_code(8);
    ok(f.error_code() == 8, "the modifier");
    AnException g(f);
    ok(g.error_code() == 8, "the copy constructor");
    try {
        throw f;
    } catch (const std::exception& caught) {
        ok(std::string(caught.what()) == "explained", "caught as std::exception");
    }
    Empty empty("nothing");
    ok(std::string(empty.what()) == "nothing", "an exception without members");
    SAME(decltype(&MyInterfaceException::op1), void (MyInterfaceException::*)(std::int32_t));"#,
    );
}

/// §7.5: the declarations of an interface body in its class, constants
/// `static constexpr`.
#[test]
fn idl4cpp_7_5_an_interface_body_declares_in_its_class() {
    let idl = "interface FullInterface {
            struct S { long a_long; };
            union U switch (long) { case 1: long x; };
            exception E { long code; };
            typedef sequence<S> SSeq;
            const double PI = 3.14;
            void op1(in S s_in);
            attribute long an_attribute;
        };";
    run(
        "full",
        idl,
        r#"static_assert(FullInterface::PI == 3.14, "static constexpr");"#,
        r#"    FullInterface::S s;
    s.a_long = 1;
    FullInterface::S t(s);
    ok(t == s, "the nested structure has its operators");
    FullInterface::S u;
    swap(t, u);
    ok(u.a_long == 1, "and its swap");
    FullInterface::U un;
    ok(un.x() == 0, "a nested union");
    FullInterface::SSeq seq(2);
    ok(seq.size() == 2, "a nested typedef");
    FullInterface::E e(3, "three");
    ok(e.code() == 3, "a nested exception");
    SAME(decltype(&FullInterface::op1), void (FullInterface::*)(const FullInterface::S&));"#,
    );
}

/// §7.6: the value types of the clause's example, their factory, and a
/// derived value type supporting an interface.
#[test]
fn idl4cpp_7_6_a_value_type_is_an_abstract_class_with_its_factory() {
    let idl = "valuetype VT1 {
            attribute long a_long_attr;
            void vt_op(in long p_long);
            public long a_public_long;
            private long a_private_long;
            factory vt_factory(in long a_long, in short a_short);
        };
        interface MyInterface { void op(); };
        valuetype VT2 : VT1 supports MyInterface { public long third_long; };";
    run(
        "valuetype",
        idl,
        r#"class VT2Impl : public VT2 {
public:
    std::int32_t a = 0, pub = 0, priv = 0, third = 0;
    void a_long_attr(std::int32_t value) override { a = value; }
    std::int32_t a_long_attr() const override { return a; }
    void vt_op(std::int32_t p) override { a = p; }
    void a_public_long(std::int32_t value) override { pub = value; }
    std::int32_t a_public_long() const override { return pub; }
    std::int32_t& a_public_long() override { return pub; }
    void op() override { third = 99; }
    void third_long(std::int32_t value) override { third = value; }
    std::int32_t third_long() const override { return third; }
    std::int32_t& third_long() override { return third; }
    std::int32_t private_value() { return a_private_long(); }

protected:
    void a_private_long(std::int32_t value) override { priv = value; }
    std::int32_t a_private_long() const override { return priv; }
    std::int32_t& a_private_long() override { return priv; }
};

class Factory : public VT1_factory {
public:
    omg::types::ref_type<VT1> vt_factory(std::int32_t a_long, std::int16_t) override {
        auto made = std::make_shared<VT2Impl>();
        made->a_public_long(a_long);
        return made;
    }
};"#,
        r#"    ok(std::is_abstract<VT1>::value && std::is_abstract<VT2>::value, "abstract classes");
    ok(std::is_base_of<VT1, VT2>::value, "VT2 : VT1");
    ok(!std::is_base_of<MyInterface, VT2>::value, "a supported interface's operations, not its class");
    ok(!std::is_constructible<VT1_factory>::value && !std::is_destructible<VT1_factory>::value,
       "the factory's constructor and destructor are protected");
    SAME(decltype(&VT1_factory::vt_factory), omg::types::ref_type<VT1> (VT1_factory::*)(std::int32_t, std::int16_t));
    Factory factory;
    omg::types::ref_type<VT1> made = factory.vt_factory(5, 1);
    ok(made->a_public_long() == 5, "the factory makes the value");
    VT2Impl impl;
    impl.op();
    impl.a_public_long() = 3;
    ok(impl.third_long() == 99 && impl.a_public_long() == 3 && impl.private_value() == 0, "the accessors");"#,
    );
}

/// §7.14.3.2 and §7.14.3.3: the clause's bit sets and bit mask.
#[test]
fn idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags() {
    let idl =
        "bitset BitSet1 { bitfield<1> bit0; bitfield<1>; bitfield<2, unsigned short> bits2_3; };
        bitset BitSet2 : BitSet1 { bitfield<3>; bitfield<1> bit7; };
        @bit_bound(32) bitmask MyBitMask { @position(0) flag0, @position(1) flag1 };
        @bit_bound(40) bitmask Wide { @position(39) top };
        bitmask Plain { a, b };";
    run(
        "bits",
        idl,
        "",
        r#"    SAME(decltype(BitSet1::bit0), bool);
    SAME(decltype(BitSet1::bits2_3), std::uint16_t);
    ok(std::is_base_of<BitSet1, BitSet2>::value, "BitSet2 : BitSet1");
    BitSet2 b = BitSet2();
    b.bit0 = true;
    b.bits2_3 = 3;
    b.bit7 = true;
    ok(b.bit0 && b.bits2_3 == 3 && b.bit7, "the bit fields");
    MyBitMask m;
    ok(m() == 0, "a default-constructed bit mask is 0");
    m |= MyBitMask::flag1;
    ok(m() == 2 && MyBitMask::flag0 == 1, "operator|= and the _flags values");
    m ^= MyBitMask::flag0;
    ok(m() == 3, "operator^=");
    m &= MyBitMask::flag0;
    ok(m() == 1, "operator&=");
    MyBitMask copy(m);
    ok(copy == m, "the copy constructor");
    SAME(std::underlying_type<MyBitMask::_flags>::type, std::uint32_t);
    SAME(MyBitMask::MyBitMaskBits, MyBitMask::_flags);
    ok(omg::types::bit_bound<MyBitMask>::value == 32, "bit_bound of a bit mask");
    SAME(omg::types::underlying_type<MyBitMask>::type, std::uint32_t);
    SAME(omg::types::underlying_type<Wide>::type, std::uint64_t);
    ok(Wide::top == 0x8000000000ULL, "a 64-bit flag");
    ok(omg::types::bit_bound<Plain>::value == 32, "the default bit_bound");"#,
    );
}

/// §7.17: `@optional` (Table 7.15), `@external` (Table 7.18), `@range`,
/// `@min`, `@max`, `@default` (Table 7.17), `@default_literal` (Table
/// 7.16), and the annotations without impact (`@id`, `@autoid`,
/// `@extensibility`, `@final`, `@mutable`, `@appendable`, `@key`,
/// `@must_understand`, `@unit`, `@nested`).
#[test]
fn idl4cpp_7_17_the_standardized_annotations_shape_the_struct() {
    let idl = r#"enum Level { LOW, @default_literal MID, HIGH };
        struct Inner { long v; };
        @final @nested struct Annotated {
            @key @id(5) long id;
            @optional long maybe;
            @optional ::Inner inner;
            @external ::Inner shared;
            @range(min = -10, max = 10) long x;
            @min(0) short positive;
            @max(100) unsigned long ceiling;
            @range(min = -1.5, max = 2.5) double ratio;
            @default(7) short seven;
            @unit("m") @must_understand double metres;
            ::Level level;
        };
        @mutable @autoid(HASH) struct Mutable { long a; string b; };
        @appendable @autoid(SEQUENTIAL) struct Appendable { long a; string b; };
        @extensibility(FINAL) struct Final { long a; string b; };
        struct Bare { long a; string b; };"#;
    run(
        "annotations",
        idl,
        "",
        r#"    Annotated a;
#if __cplusplus >= 201703L
    SAME(decltype(Annotated::maybe), std::optional<std::int32_t>);
#endif
    SAME(decltype(Annotated::maybe), omg::types::optional<std::int32_t>);
    SAME(decltype(Annotated::shared), omg::types::ref_type<Inner>);
    SAME(decltype(Annotated::x), omg::types::ranged<std::int32_t, -10, 10>);
    SAME(decltype(Annotated::id), std::int32_t);
    SAME(decltype(Annotated::metres), double);
    ok(!a.maybe.has_value() && !a.inner.has_value(), "an optional member starts without a value");
    a.maybe = 4;
    ok(a.maybe.has_value() && *a.maybe == 4, "and takes one");
    a.shared = std::make_shared<Inner>();
    a.shared->v = 1;
    Annotated b(a);
    b.shared->v = 2;
    ok(a.shared->v == 1, "the copy constructor copies an external member deeply");
    ok(!(a == b), "and compares what it points to");
    a.x = 10;
    throws<std::out_of_range>([&] { a.x = 11; }, "@range(max = 10)");
    throws<std::out_of_range>([&] { a.positive = -1; }, "@min(0)");
    throws<std::out_of_range>([&] { a.ceiling = 101; }, "@max(100)");
    a.ratio = 2.5;
    throws<std::out_of_range>([&] { a.ratio = 2.6; }, "@range on a double");
    ok(a.x == 10 && a.ratio == 2.5, "the checked members keep the last good value");
    ok(a.seven == 7, "@default(7)");
    ok(a.level == Level::MID, "@default_literal");
    // Table 7.15: @autoid, @extensibility, @final, @mutable and @appendable have no impact
    SAME(decltype(Mutable::a), decltype(Bare::a));
    SAME(decltype(Mutable::b), decltype(Bare::b));
    SAME(decltype(Appendable::a), decltype(Bare::a));
    SAME(decltype(Appendable::b), decltype(Bare::b));
    SAME(decltype(Final::a), decltype(Bare::a));
    SAME(decltype(Final::b), decltype(Bare::b));
    ok(sizeof(Mutable) == sizeof(Bare) && sizeof(Appendable) == sizeof(Bare) && sizeof(Final) == sizeof(Bare),
       "an annotated structure is the structure without the annotation");"#,
    );
}

/// Table 7.19: `@verbatim` text for `*`, `c++`, `cpp`, `cc` and `cxx` at
/// each of the six placements, and none for another language; §7.16: user
/// annotations are not propagated.
#[test]
fn idl4cpp_7_17_5_verbatim_text_is_copied_for_its_languages() {
    let idl = r#"@annotation Tagged { string note; };
        @verbatim(language = "c++", placement = BEFORE_DECLARATION, text = "// one-cpp")
        @verbatim(language = "cc", placement = BEFORE_DECLARATION, text = "// two-cc")
        @verbatim(language = "cxx", placement = BEGIN_DECLARATION, text = "int three_cxx = 3;")
        @verbatim(language = "*", placement = END_DECLARATION, text = "int four_star = 4;")
        @verbatim(language = "java", placement = BEFORE_DECLARATION, text = "// java-only")
        @verbatim(language = "c++", placement = BEGIN_FILE, text = "// file-begin")
        @verbatim(language = "c++", placement = AFTER_DECLARATION, text = "// after-declaration")
        @verbatim(language = "c++", placement = END_FILE, text = "// file-end")
        @Tagged(note = "n")
        struct Copied { long x; };"#;
    let files = cpp(idl, &Options::default());
    let text = header(&files);
    for expected in [
        "// one-cpp",
        "// two-cc",
        "int three_cxx = 3;",
        "int four_star = 4;",
    ] {
        assert!(text.contains(expected), "{expected} missing:\n{text}");
    }
    assert!(!text.contains("java-only"), "{text}");
    // The six placements of Table 7.19, each at its position.
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{text}"))
    };
    let order = [
        "// file-begin",
        "// one-cpp",
        "struct Copied {",
        "int three_cxx = 3;",
        "int four_star = 4;",
        "// after-declaration",
        "// file-end",
    ];
    for pair in order.windows(2) {
        assert!(
            at(pair[0]) < at(pair[1]),
            "{} stands behind {}:\n{text}",
            pair[0],
            pair[1]
        );
    }
    assert!(
        !text.contains("Tagged"),
        "a user annotation reached the C++:\n{text}"
    );
    let check = check_program(
        &files[0].path,
        "",
        r#"    Copied c;
    ok(c.three_cxx == 3 && c.four_star == 4, "the verbatim members");"#,
    );
    compile_and_run("verbatim", &files, &check, BOTH);
}

/// §8.1.1: `@cpp_mapping(struct_mapping =
/// CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS)` on the clause's example.
#[test]
fn idl4cpp_8_1_1_class_with_public_accessors_and_modifiers() {
    let idl = "@cpp_mapping(struct_mapping = CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS)
        struct MyStruct { long a_long; short a_short; string a_string; };";
    run(
        "accessors",
        idl,
        "",
        r#"    ok(std::is_class<MyStruct>::value, "a class");
    MyStruct m(1, 2, "three");
    ok(m.a_long() == 1 && m.a_short() == 2 && m.a_string() == "three", "the all-members constructor");
    SAME(decltype(static_cast<const MyStruct&>(m).a_string()), std::string);
    SAME(decltype(m.a_string()), std::string&);
    m.a_long(5);
    m.a_long() += 1;
    m.a_string(std::string("four"));
    std::string moved = "five";
    m.a_string(std::move(moved));
    ok(m.a_long() == 6 && m.a_string() == "five", "modifiers and reference accessors");
    MyStruct n;
    ok(n.a_long() == 0 && n != m, "the default constructor");
    swap(m, n);
    ok(n.a_long() == 6, "swap");"#,
    );
}

/// §7.1.4: the traits of Table 7.1 for a built-in type, a structure, an
/// interface, and their `_t` aliases.
#[test]
fn idl4cpp_7_1_4_the_type_traits_give_value_in_out_and_inout_types() {
    let idl = "struct S { long a; }; interface I { void op(); }; enum E { A };";
    run(
        "traits",
        idl,
        "",
        r#"    SAME(omg::types::value_type<std::int32_t>::type, std::int32_t);
    SAME(omg::types::in_type<std::int32_t>::type, std::int32_t);
    SAME(omg::types::in_type<E>::type, E);
    SAME(omg::types::in_type<S>::type, const S&);
    SAME(omg::types::out_type<S>::type, S&);
    SAME(omg::types::inout_type<S>::type, S&);
    SAME(omg::types::value_type<I>::type, omg::types::ref_type<I>);
    SAME(omg::types::in_type<I>::type, const omg::types::ref_type<I>&);
    SAME(omg::types::in_type_t<std::string>, const std::string&);
    SAME(omg::types::value_type_t<S>, S);
#if __cplusplus >= 201402L
    ok(omg::types::is_bounded_v<omg::types::bounded_sequence<S, 2>> && omg::types::bound_v<omg::types::bounded_string<3>> == 3,
       "the _v aliases (C++14 variable templates)");
#endif"#,
    );
}

/// §7.1.2: a name of the reserved list gets an underscore.
#[test]
fn idl4cpp_7_1_2_a_reserved_name_gets_an_underscore() {
    let idl = "struct _class { long _new; long delete_; }; enum _register { _static };";
    run(
        "reserved",
        idl,
        "",
        r#"    _class c;
    c._new = 1;
    ok(c._new == 1 && c.delete_ == 0, "_class::_new");
    ok(_register::_static == _register::_static, "_register::_static");"#,
    );
}

/// Annex C: the compatibility rules for C++98 and C++03 — `const`
/// constants and `std::string` (C.2.1.1), `omg::types::int32_t` and the
/// rest (Tables C.1 to C.3), no move operations (C.2.1.2.2), a plain
/// `enum` with `enum_prefix` and `enum_suffix` (C.2.1.2.2.1, C.3.1),
/// C arrays (C.2.1.2.3), `typedef` (C.2.1.2.4), `T*` for an interface
/// parameter (C.2.2), an optional and an external member (Tables C.4,
/// C.5), and the bit mask of C.2.4.1.1 — compiled with `-std=c++03`.
#[test]
fn idl4cpp_c_the_cxx03_mapping_follows_annex_c() {
    use zerodds_lang_rules::emit::idl4_cpp::Standard;
    let idl = r#"module my_math {
            const string my_string = "My String Value";
            const double PI = 3.141592;
            const wstring ws = L"Hello World";
        };
        @cpp_mapping(enum_prefix = "color_") enum Colors { red, green, blue };
        @cpp_mapping(enum_suffix = "_shade") @bit_bound(8) enum Shades { light, dark };
        typedef long long_array[100];
        typedef string string_array[1][2];
        typedef long Length;
        struct Inner { long v; };
        struct MyType {
            Length my_type_length;
            short grid[2][3];
            int8 small;
            uint64 big;
            Colors color;
            @optional float age;
            @external Inner shared;
        };
        union U switch (long) { case 1: string text; case 2: long number; };
        interface Peer { void ping(in Peer other, out Peer back); };
        @bit_bound(32) bitmask MyBitMask { @position(0) flag0, @position(1) flag1 };"#;
    let files = cpp(
        idl,
        &Options {
            standard: Standard::Cxx03,
            ..Options::default()
        },
    );
    let text = header(&files);
    // `&&` behind a type's last character is an rvalue reference; between
    // two operands of a comparison it stands after a blank.
    let rvalue_reference = text.match_indices("&&").any(|(at, _)| {
        text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '>')
    });
    assert!(
        !rvalue_reference && !text.contains("std::move") && !text.contains("noexcept"),
        "a move operation in the C++03 mapping:\n{text}"
    );
    assert!(
        !text.contains("constexpr"),
        "constexpr in the C++03 mapping:\n{text}"
    );
    assert!(
        !text.contains("enum class"),
        "an enum class in the C++03 mapping:\n{text}"
    );
    // `using std::swap;` in a swap function is a using-declaration.
    assert!(
        !text
            .lines()
            .any(|line| line.trim_start().starts_with("using ") && line.contains('=')),
        "an alias declaration in the C++03 mapping:\n{text}"
    );
    let check = check_program(
        &files[0].path,
        r#"class PeerImpl : public Peer {
public:
    void ping(Peer* other, Peer*& back) { back = other; }
};"#,
        r#"    ok(std::string(my_math::my_string) == "My String Value", "a std::string constant");
    ok(my_math::PI > 3.14 && my_math::ws == std::wstring(L"Hello World"), "double and wstring constants");
    Colors c = color_green;
    ok(c == 1 && color_blue == 2, "enum_prefix");
    ok(light_shade == 0 && dark_shade == 1, "enum_suffix");
    ok(sizeof(long_array) == 100 * sizeof(omg::types::int32_t), "a C array typedef");
    string_array names;
    names[0][1] = "x";
    ok(names[0][1] == "x", "a two-dimensional C array of strings");
    MyType m;
    ok(m.my_type_length == 0 && m.grid[1][2] == 0 && m.small == 0 && m.big == 0 && m.color == color_red,
       "members start at their default value");
    ok(m.age == 0 && m.shared == 0, "an optional and an external member are null pointers");
    float age = 3.5f;
    m.age = &age;
    m.grid[1][2] = 4;
    MyType n(m);
    ok(n == m && n.grid[1][2] == 4 && *n.age == 3.5f, "copy and comparison of C array members");
    MyType o;
    swap(n, o);
    ok(o.grid[1][2] == 4 && n.grid[1][2] == 0, "swap of C array members");
    ok(sizeof(m.small) == 1 && sizeof(m.big) == 8, "Tables C.2 and C.3");
    U u;
    u.text("hello");
    U v(u);
    ok(v.text() == "hello" && v._d() == 1, "a union holding a string");
    v.number(5);
    ok(v.number() == 5 && u.text() == "hello", "and its copy is independent");
    PeerImpl p;
    Peer* back = 0;
    p.ping(&p, back);
    ok(back == &p, "an interface parameter is a pointer");
    MyBitMask mask;
    mask |= MyBitMask::flag1;
    ok(mask() == 2, "the bit mask of C.2.4.1.1");"#,
    );
    compile_and_run("cxx03", &files, &check, &["c++03"]);
}
