// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C++ Language Mapping 1.0 §7.9, §7.10, §7.12 and §7.13:
//! components, homes, ports, connectors and template module instances
//! are "mapped to equivalent IDL, as specified in [OMG-CORBA-COMP]" and
//! "to intermediate IDL, as specified in [OMG-IDL4]", and from there to
//! C++ by the clauses for interfaces and value types.
//!
//! The intermediate IDL of a component, a home and an event type is the
//! equivalent IDL of CCM 4.0 §6; the expectations below are that IDL's
//! operations (§6.4.3.2, §6.5.2, §6.6.3 to §6.6.6, §6.7.2 to §6.7.4)
//! written in C++ by §7.4 and §7.6. The `Components` module the
//! equivalent IDL names is CCM 4.0's, transcribed in
//! `java-idl4/Components.idl`; it names `Object`, so the platform is CORBA
//! and the header includes `omg/corba.hpp`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "cpp-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, corba_idl, cpp, header};
use zerodds_lang_rules::emit::idl4_cpp::{CppFile, Options, Platform};

const BOTH: &[&str] = &["c++11", "c++17"];

const SHOP: &str = r"
module Shop {
    interface Pricing { long price(in string item); };
    eventtype Sold { public long amount; };
    porttype Till {
        provides Pricing pricing;
        uses Pricing fallback;
    };
    component Cashier {
        provides Pricing prices;
        uses multiple Pricing others;
        publishes Sold sales;
        emits Sold alarms;
        consumes Sold returns;
        port Till till;
        attribute long lane;
    };
    home CashierHome manages Cashier {
        factory open(in long lane);
    };
    connector Link {
        provides Pricing out_price;
        mirrorport Till other;
        attribute string route;
    };
    module Buffer<typename T, const long Capacity> {
        struct Slot { T value; };
        const long Size = Capacity;
    };
    module Buffer<long, 4> LongBuffer;
};
";

/// Whether `T` has a member of a name, for the operations the equivalent
/// IDL does not have.
const ABSENT: &str = r"
#define HAS_MEMBER(NAME) \
    template <typename T, typename = void> struct has_##NAME : std::false_type {}; \
    template <typename T> struct has_##NAME<T, decltype(void(&T::NAME))> : std::true_type {}
HAS_MEMBER(provide_prices);
HAS_MEMBER(provide_till);
HAS_MEMBER(provide_other_pricing);

typedef omg::types::ref_type<Shop::Pricing> PricingRef;
typedef omg::types::ref_type<Components::Cookie> CookieRef;
typedef omg::types::ref_type<Shop::SoldConsumer> ConsumerRef;
";

fn shop() -> Vec<CppFile> {
    let components = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/java-idl4/Components.idl"),
    )
    .expect("tests/java-idl4/Components.idl");
    let options = Options {
        platform: Platform::Corba,
        ..Options::default()
    };
    cpp(&format!("{}\n{components}\n{SHOP}", corba_idl()), &options)
}

fn run(tag: &str, body: &str) {
    let files = shop();
    let check = check_program(&files[0].path, ABSENT, body);
    compile_and_run(tag, &files, &check, BOTH);
}

/// §7.9 and §7.12: a component is its equivalent interface — CCMObject
/// for a base, an operation per port named as CCM 4.0 §6.4 to §6.6 name
/// it, an extended port as the basic ports of its port type
/// (`<port>_<basic port>`, OMG IDL 4.2 §7.4.11.4.1) — and an event type its
/// value type and consumer interface. The port type declares nothing of
/// its own.
#[test]
fn idl4cpp_7_9_and_7_12_a_component_is_its_equivalent_interface_in_cpp() {
    let files = shop();
    let text = header(&files);
    assert!(
        !text.contains("class Till"),
        "the port type is a class of its own:\n{text}"
    );
    run(
        "components",
        r#"    using Shop::Cashier;
    ok(std::is_abstract<Cashier>::value && has_provide_prices<Cashier>::value, "a component is an abstract class");
    ok(std::is_base_of<Components::CCMObject, Cashier>::value, "Cashier extends Components::CCMObject (6.3.2)");
    // 6.4.3.2: provides
    SAME(decltype(&Cashier::provide_prices), PricingRef (Cashier::*)());
    // 6.5.2.2: uses multiple
    SAME(decltype(&Cashier::connect_others), CookieRef (Cashier::*)(PricingRef));
    SAME(decltype(&Cashier::disconnect_others), PricingRef (Cashier::*)(CookieRef));
    SAME(decltype(&Cashier::get_connections_others), Cashier::othersConnections (Cashier::*)());
    SAME(Cashier::othersConnections, omg::types::sequence<Cashier::othersConnection>);
    SAME(decltype(Cashier::othersConnection::objref), PricingRef);
    SAME(decltype(Cashier::othersConnection::ck), CookieRef);
    // 6.6.5: publishes; 6.6.4: emits; 6.6.6: consumes
    SAME(decltype(&Cashier::subscribe_sales), CookieRef (Cashier::*)(ConsumerRef));
    SAME(decltype(&Cashier::unsubscribe_sales), ConsumerRef (Cashier::*)(CookieRef));
    SAME(decltype(&Cashier::connect_alarms), void (Cashier::*)(ConsumerRef));
    SAME(decltype(&Cashier::disconnect_alarms), ConsumerRef (Cashier::*)());
    SAME(decltype(&Cashier::get_consumer_returns), ConsumerRef (Cashier::*)());
    // the extended port: the basic ports of Till
    SAME(decltype(&Cashier::provide_till_pricing), PricingRef (Cashier::*)());
    SAME(decltype(&Cashier::connect_till_fallback), void (Cashier::*)(PricingRef));
    SAME(decltype(&Cashier::disconnect_till_fallback), PricingRef (Cashier::*)());
    SAME(decltype(&Cashier::get_connection_till_fallback), PricingRef (Cashier::*)());
    ok(!has_provide_till<Cashier>::value, "the extended port itself is no operation");
    // the attribute, 7.4
    SAME(decltype(static_cast<std::int32_t (Cashier::*)() const>(&Cashier::lane)), std::int32_t (Cashier::*)() const);
    SAME(decltype(static_cast<void (Cashier::*)(std::int32_t)>(&Cashier::lane)), void (Cashier::*)(std::int32_t));
    // 6.6.1: the event type and its consumer
    ok(std::is_base_of<Components::EventConsumerBase, Shop::SoldConsumer>::value,
       "SoldConsumer extends Components::EventConsumerBase");
    SAME(decltype(&Shop::SoldConsumer::push_Sold), void (Shop::SoldConsumer::*)(omg::types::ref_type<Shop::Sold>));
    ok(std::is_base_of<Components::EventBase, Shop::Sold>::value && std::is_abstract<Shop::Sold>::value,
       "Sold is a value type on Components::EventBase");
    SAME(decltype(static_cast<std::int32_t (Shop::Sold::*)() const>(&Shop::Sold::amount)), std::int32_t (Shop::Sold::*)() const);"#,
    );
}

/// §7.10: a home is its explicit, implicit and equivalent interfaces (CCM
/// 4.0 §6.7.2 to §6.7.4), a factory an operation of the explicit one that
/// returns the component.
#[test]
fn idl4cpp_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces() {
    run(
        "homes",
        r#"    using Shop::CashierHome;
    using Shop::CashierHomeExplicit;
    using Shop::CashierHomeImplicit;
    ok(std::is_base_of<Components::CCMHome, CashierHomeExplicit>::value,
       "the explicit interface extends Components::CCMHome");
    ok(std::is_base_of<Components::KeylessCCMHome, CashierHomeImplicit>::value,
       "the implicit interface of a keyless home extends Components::KeylessCCMHome");
    ok(std::is_base_of<CashierHomeExplicit, CashierHome>::value && std::is_base_of<CashierHomeImplicit, CashierHome>::value,
       "the equivalent interface extends both");
    SAME(decltype(&CashierHomeExplicit::open), omg::types::ref_type<Shop::Cashier> (CashierHomeExplicit::*)(std::int32_t));
    SAME(decltype(&CashierHomeImplicit::create), omg::types::ref_type<Shop::Cashier> (CashierHomeImplicit::*)());
    ok(std::is_base_of<CORBA::UserException, Components::CreateFailure>::value,
       "CreateFailure, which a factory raises, is a user exception");"#,
    );
}

/// §7.12: a connector maps through its ports as a component does, a
/// mirror port with the facets and receptacles of its port type swapped
/// (OMG IDL 4.2 §7.4.11.4.2), and its attributes as an interface's.
#[test]
fn idl4cpp_7_12_a_connector_maps_through_its_ports() {
    run(
        "connectors",
        r#"    using Shop::Link;
    SAME(decltype(&Link::provide_out_price), PricingRef (Link::*)());
    // mirrorport: Till's facet is a receptacle here, its receptacle a facet
    SAME(decltype(&Link::connect_other_pricing), void (Link::*)(PricingRef));
    SAME(decltype(&Link::get_connection_other_pricing), PricingRef (Link::*)());
    SAME(decltype(&Link::provide_other_fallback), PricingRef (Link::*)());
    ok(!has_provide_other_pricing<Link>::value, "the mirrored facet is no facet");
    SAME(decltype(static_cast<std::string (Link::*)() const>(&Link::route)), std::string (Link::*)() const);
    SAME(decltype(static_cast<void (Link::*)(const std::string&)>(&Link::route)), void (Link::*)(const std::string&));"#,
    );
}

/// §7.13: a template module instance is the module its template declares
/// with the actual parameters in place of the formal ones (OMG IDL 4.2
/// §7.4.12.4); the template itself declares nothing.
#[test]
fn idl4cpp_7_13_a_template_module_instance_is_a_namespace_of_its_own() {
    let files = shop();
    let text = header(&files);
    assert!(
        !text.contains("namespace Buffer"),
        "the template itself declares a namespace:\n{text}"
    );
    run(
        "template-modules",
        r#"    SAME(decltype(Shop::LongBuffer::Slot::value), std::int32_t);
    static_assert(Shop::LongBuffer::Size == 4, "Size is the actual parameter");
    Shop::LongBuffer::Slot slot;
    ok(slot.value == 0, "the instance's structure");"#,
    );
}
