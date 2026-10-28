// Compile and link only: DynamicType and DynamicData, with an enumeration, an alias and a bit set.
#include <dds/core/xtypes/DynamicData.hpp>
#include <dds/core/xtypes/EnumTypes.hpp>
#include <cstdio>

int main() {
    using namespace dds::core::xtypes;
    EnumerationType<uint32_t> color("m::Color", {Enumerator("RED"), Enumerator("GREEN", 5)});
    AliasType label("m::Label", StringType<char>(20));
    BitSetType flags("m::Flags", {BitField("low", 3), BitField("high", 5)});
    StructType s("m::Track", StructType(),
                 {MemberType("id", PrimitiveType<int32_t>(), annotation::Key()),
                  MemberType("label", label), MemberType("n", PrimitiveType<int64_t>()),
                  MemberType("color", color), MemberType("flags", flags)});
    DynamicData d(s);
    d.value("id", int32_t(7));
    d.value("label", std::string("x"));
    d.value("n", int64_t(5));
    d.value("color", std::string("GREEN"));
    d.value("flags", std::map<std::string, uint64_t>{{"low", 3}, {"high", 9}});
    auto b = d.encode();
    DynamicData e(s);
    e.decode(b);
    return e == d && e.value<int32_t>("id") == 7 &&
                   e.value<std::string>("color") == "GREEN" &&
                   e.value<std::map<std::string, uint64_t>>("flags").at("high") == 9 && e.member_id("n") == 2
               ? 0
               : 1;
}

// TypeProvider: the types at a URI become DynamicTypes (compile and link only).
#include <dds/core/xtypes/TypeProvider.hpp>
int type_provider_probe() {
    using dds::core::xtypes::TypeProvider;
    const auto first = TypeProvider::load_type("data:text/plain,struct%20T%20%7B%20long%20x%3B%20%7D%3B");
    const auto all = TypeProvider::load_types("file:///tmp/types.idl");
    const auto named = TypeProvider::load_type("file:///tmp/types.idl", "m::Track");
    return static_cast<int>(all.size()) + static_cast<int>(first.name().size()) +
           static_cast<int>(named.name().size());
}
