// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to Java Language Mapping 1.0 §7.9, §7.10, §7.12 and §7.13:
//! components, homes, ports, connectors and template module instances
//! "shall be mapped to intermediate IDL, as specified in [IDL4], and mapped
//! to Java accordingly".
//!
//! The intermediate IDL of a component, a home and an event type is the
//! equivalent IDL of CCM 4.0 §6; the expectations below are that IDL's
//! operations (§6.4.3.2, §6.5.2, §6.6.3 to §6.6.6, §6.7.2 to §6.7.4)
//! written in Java by §7.4 and §7.6, for the clauses' own example shapes.
//! The `Components` module the equivalent IDL names is CCM 4.0's,
//! transcribed in `java-idl4/Components.idl`; it names `Object`, so the
//! platform is CORBA.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "java-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, corba_idl, corba_module, file, java};
use zerodds_lang_rules::emit::idl4_java::{JavaFile, Options, Platform};

fn components_idl() -> String {
    let components = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/java-idl4/Components.idl"),
    )
    .expect("tests/java-idl4/Components.idl");
    format!("{}\n{components}\n", corba_idl())
}

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

fn shop() -> Vec<JavaFile> {
    let options = Options {
        platform: Platform::Corba,
        ..Options::default()
    };
    let mut files = corba_module();
    files.extend(java(&format!("{}{SHOP}", components_idl()), &options));
    files
}

/// §7.9 and §7.12: a component is its equivalent interface — CCMObject
/// for a base, an operation per port named as CCM 4.0 §6.4 to §6.6 name
/// it, an extended port as the basic ports of its port type
/// (`<port>_<basic port>`, OMG IDL 4.2 §7.4.11.4.1) — and an event type its
/// value type and consumer interface.
#[test]
fn idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java() {
    let files = shop();
    file(&files, "Shop/Cashier.java");
    let check = check_program(
        r#"
        Class<?> c = Shop.Cashier.class;
        Class<?> pricing = Shop.Pricing.class;
        Class<?> cookie = Components.Cookie.class;
        Class<?> consumer = Shop.SoldConsumer.class;
        check(c.isInterface(), "a component is an interface");
        check(java.util.Arrays.asList(c.getInterfaces()).contains(Components.CCMObject.class),
            "Cashier extends Components.CCMObject (§6.3.2)");
        // §6.4.3.2: provides
        returns(c, "provide_prices", pricing);
        // §6.5.2.2: uses multiple
        returns(c, "connect_others", cookie, pricing);
        returns(c, "disconnect_others", pricing, cookie);
        returns(c, "get_connections_others", java.util.List.class);
        returns(Shop.Cashier.othersConnection.class, "get_objref", pricing);
        returns(Shop.Cashier.othersConnection.class, "get_ck", cookie);
        // §6.6.5: publishes; §6.6.4: emits; §6.6.6: consumes
        returns(c, "subscribe_sales", cookie, consumer);
        returns(c, "unsubscribe_sales", consumer, cookie);
        returns(c, "connect_alarms", void.class, consumer);
        returns(c, "disconnect_alarms", consumer);
        returns(c, "get_consumer_returns", consumer);
        // the extended port: the basic ports of Till
        returns(c, "provide_till_pricing", pricing);
        returns(c, "connect_till_fallback", void.class, pricing);
        returns(c, "disconnect_till_fallback", pricing);
        returns(c, "get_connection_till_fallback", pricing);
        absent(c, "provide_till");
        // the attribute, §7.4
        returns(c, "get_lane", int.class);
        returns(c, "set_lane", void.class, int.class);
        // §6.6.1: the event type and its consumer
        check(java.util.Arrays.asList(consumer.getInterfaces()).contains(Components.EventConsumerBase.class),
            "SoldConsumer extends Components.EventConsumerBase");
        returns(consumer, "push_Sold", void.class, Shop.Sold.class);
        check(Shop.SoldAbstract.class.isAssignableFrom(Shop.Sold.class), "Sold is its value type's class");
        check(Shop.Sold.class.getField("amount").getType() == int.class, "the public state of Sold is a public field (§7.6)");
"#,
    );
    compile_and_run("components", &files, &check);
}

/// §7.10: a home is its explicit, implicit and equivalent interfaces (CCM
/// 4.0 §6.7.2 to §6.7.4), a factory an operation of the explicit one that
/// returns the component and raises `CreateFailure`.
#[test]
fn idl4java_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces() {
    let files = shop();
    let check = check_program(
        r#"
        Class<?> explicit = Shop.CashierHomeExplicit.class;
        Class<?> implicit = Shop.CashierHomeImplicit.class;
        Class<?> home = Shop.CashierHome.class;
        check(java.util.Arrays.asList(explicit.getInterfaces()).contains(Components.CCMHome.class),
            "the explicit interface extends Components.CCMHome");
        check(java.util.Arrays.asList(implicit.getInterfaces()).contains(Components.KeylessCCMHome.class),
            "the implicit interface of a keyless home extends Components.KeylessCCMHome");
        check(java.util.Arrays.asList(home.getInterfaces()).containsAll(
            java.util.Arrays.asList(explicit, implicit)), "the equivalent interface extends both");
        returns(explicit, "open", Shop.Cashier.class, int.class);
        Class<?>[] thrown = method(explicit, "open", int.class).getExceptionTypes();
        check(thrown.length == 1 && thrown[0] == Components.CreateFailure.class, "open raises CreateFailure");
        returns(implicit, "create", Shop.Cashier.class);
"#,
    );
    compile_and_run("homes", &files, &check);
}

/// §7.12: a connector maps through its ports as a component does, a
/// mirror port with the facets and receptacles of its port type swapped
/// (OMG IDL 4.2 §7.4.11.4.2), and its attributes as an interface's.
#[test]
fn idl4java_7_12_a_connector_maps_through_its_ports() {
    let files = shop();
    let check = check_program(
        r#"
        Class<?> link = Shop.Link.class;
        Class<?> pricing = Shop.Pricing.class;
        returns(link, "provide_out_price", pricing);
        // mirrorport: Till's facet is a receptacle here, its receptacle a facet
        returns(link, "connect_other_pricing", void.class, pricing);
        returns(link, "get_connection_other_pricing", pricing);
        returns(link, "provide_other_fallback", pricing);
        absent(link, "provide_other_pricing");
        returns(link, "get_route", String.class);
        returns(link, "set_route", void.class, String.class);
"#,
    );
    compile_and_run("connectors", &files, &check);
}

/// §7.13: a template module instance is the module its template declares
/// with the actual parameters in place of the formal ones (OMG IDL 4.2
/// §7.4.12.4).
#[test]
fn idl4java_7_13_a_template_module_instance_is_a_package_of_its_own() {
    let files = shop();
    let slot = file(&files, "Shop/LongBuffer/Slot.java");
    assert!(slot.contains("package Shop.LongBuffer;"), "{slot}");
    assert!(
        files.iter().all(|f| !f.path.starts_with("Shop/Buffer/")),
        "the template itself declares nothing"
    );
    let check = check_program(
        r#"
        returns(Shop.LongBuffer.Slot.class, "get_value", int.class);
        check(Shop.LongBuffer.Size.value == 4, "Size is the actual parameter");
"#,
    );
    compile_and_run("template-modules", &files, &check);
}
