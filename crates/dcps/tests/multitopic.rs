// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `MultiTopic` and a DataReader on it (DDS 1.4 §2.2.2.3.4, Annex B.2).
//!
//! The expressions, topics and result types are the ones of the clause: a
//! natural join of the topics of the `FROM` clause, the fields of the
//! `SELECT` list under their aliases, `WHERE` over the joined row, and a
//! result instance whose state follows its constituents.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use zerodds_dcps::{
    DdsError, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, RawBytes, TopicQos,
};

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

fn bad_parameter<T: core::fmt::Debug>(r: Result<T, DdsError>) -> bool {
    matches!(r, Err(DdsError::BadParameter { .. }))
}

/// A participant with the topics `Order` and `Customer`, and the result types
/// the tests below ask for registered.
fn participant_with_topics() -> DomainParticipant {
    let p = offline();
    p.create_dynamic_topic(
        "Order",
        TopicQos::default(),
        common::dynamic_support(common::ORDER_IDL, "Order", true),
    )
    .unwrap();
    p.create_dynamic_topic(
        "Customer",
        TopicQos::default(),
        common::dynamic_support(common::CUSTOMER_IDL, "Customer", true),
    )
    .unwrap();
    for (idl, name) in [
        (common::SALE_IDL, "Sale"),
        (common::ORDER_COPY_IDL, "OrderCopy"),
        (
            "@final struct Origins { @key long id; string<16> origin; };",
            "Origins",
        ),
        (
            "@final struct Totals { @key long id; double total; };",
            "Totals",
        ),
        (
            "@final struct WrongType { @key long id; long amount; };",
            "WrongType",
        ),
    ] {
        common::dynamic_support(idl, name, true)
            .register_type(&p, None)
            .unwrap();
    }
    p
}

/// Annex B.2: the subscription expression is a `TopicExpression`; a
/// `Condition`, an `ORDER BY` and the rest are refused, never ignored.
#[test]
fn a_subscription_expression_must_be_a_topic_expression() {
    let p = participant_with_topics();
    for bad in [
        "id > 0",
        "SELECT * FROM Order ORDER BY id",
        "SELECT * FROM Order GROUP BY id",
        "SELECT * FROM Order LEFT NATURAL JOIN Customer",
        "SELECT * FROM Order, Customer",
        "SELECT FROM Order",
        "SELECT * FROM Order WHERE",
        "SELECT * FROM",
    ] {
        assert!(
            bad_parameter(p.create_dynamic_multitopic("MT", "Sale", bad, Vec::new())),
            "{bad}"
        );
    }
    // The three spellings of the join and a parenthesized join item.
    for join in ["INNER NATURAL JOIN", "NATURAL JOIN", "NATURAL INNER JOIN"] {
        let mt = p
            .create_dynamic_multitopic(
                "MT",
                "Sale",
                &format!("SELECT * FROM Order {join} Customer;"),
                Vec::new(),
            )
            .unwrap();
        assert_eq!(
            mt.get_related_topic_names(),
            ["Order".to_string(), "Customer".to_string()]
        );
        p.delete_multitopic(&mt).unwrap();
    }
}

/// DDS 1.4 §2.2.2.2.1.12: `lookup_topicdescription` "may be used to locate
/// any locally-created Topic, ContentFilteredTopic, and MultiTopic object";
/// a deleted one is not found.
#[test]
fn lookup_topicdescription_finds_a_multitopic() {
    let p = participant_with_topics();
    let mt = p
        .create_dynamic_multitopic(
            "Joined",
            "Sale",
            "SELECT * FROM Order NATURAL JOIN Customer",
            Vec::new(),
        )
        .unwrap();
    let handle = p.lookup_topicdescription("Joined").expect("found");
    assert_eq!(
        handle.kind(),
        zerodds_dcps::TopicDescriptionKind::MultiTopic
    );
    let again = handle
        .multitopic::<zerodds_types::dynamic::DynamicSample>()
        .unwrap();
    assert_eq!(
        again.get_subscription_expression(),
        "SELECT * FROM Order NATURAL JOIN Customer"
    );
    drop(again);
    p.delete_multitopic(&mt).unwrap();
    assert!(p.lookup_topicdescription("Joined").is_none());
}

/// The topics of the `FROM` clause are topics of the participant, and their
/// members are named by their `TypeObject`.
#[test]
fn the_topics_of_the_from_clause_must_exist_and_have_a_type_object() {
    let p = participant_with_topics();
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT * FROM Missing",
        Vec::new()
    )));
    p.create_topic::<RawBytes>("Raw", TopicQos::default())
        .unwrap();
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT * FROM Raw",
        Vec::new()
    )));
}

/// §2.2.2.3.4: "the type of the fields used for the NATURAL JOIN operation
/// must be the same".
#[test]
fn topics_sharing_a_member_name_need_the_same_type() {
    let p = participant_with_topics();
    p.create_dynamic_topic(
        "Clash",
        TopicQos::default(),
        common::dynamic_support("@final struct Clash { @key string<8> id; };", "Clash", true),
    )
    .unwrap();
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "Sale",
        "SELECT * FROM Order NATURAL JOIN Clash",
        Vec::new()
    )));
}

/// The result type holds the fields of the `SELECT` list: a member for each
/// field, of its type, and nothing left without a field.
#[test]
fn the_result_type_must_hold_the_fields_of_the_select_list() {
    let p = participant_with_topics();
    // OrderCopy has `amount`, which the list leaves out.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT id FROM Order",
        Vec::new()
    )));
    // No member `total` in OrderCopy.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT id, amount AS total FROM Order",
        Vec::new()
    )));
    // `amount` is a double, WrongType.amount a long.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "WrongType",
        "SELECT * FROM Order",
        Vec::new()
    )));
    // A field no topic has.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "Totals",
        "SELECT id, nothing AS total FROM Order",
        Vec::new()
    )));
    // A type nobody registered.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "Unregistered",
        "SELECT * FROM Order",
        Vec::new()
    )));
    // The aliased field fits Totals.
    p.create_dynamic_multitopic(
        "MT",
        "Totals",
        "SELECT id, amount AS total FROM Order",
        Vec::new(),
    )
    .unwrap();
}

/// `WHERE` reads the members of the joined topics and the aliases of the
/// `SELECT` list, and nothing else.
#[test]
fn where_names_columns_and_aliases_only() {
    let p = participant_with_topics();
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "Totals",
        "SELECT id, amount AS total FROM Order WHERE nothing > 1",
        Vec::new()
    )));
    // A column that is not selected, and an alias.
    p.create_dynamic_multitopic(
        "MT",
        "Totals",
        "SELECT id, amount AS total FROM Order WHERE id > 0 AND total > 1.5",
        Vec::new(),
    )
    .unwrap();
}

/// IDL 4.2 struct inheritance: the members of a base structure are fields of
/// the structure (Annex B.2 FIELDNAME refers to fields at any depth), in the
/// row, in the join, in `SELECT` and `WHERE`.
#[test]
fn inherited_members_are_fields_of_the_structure() {
    let p = participant_with_topics();
    p.create_dynamic_topic(
        "Derived",
        TopicQos::default(),
        common::dynamic_support_with_bases(
            common::DERIVED_ORDER_IDL,
            "DerivedOrder",
            &["OrderBase"],
            true,
        ),
    )
    .unwrap();
    // `id` is the base's: the join with Customer is over it, SELECT * has it
    // and `amount`, and WHERE may name it.
    let mt = p
        .create_dynamic_multitopic(
            "MT",
            "Sale",
            "SELECT * FROM Derived NATURAL JOIN Customer WHERE id > 0",
            Vec::new(),
        )
        .unwrap();
    p.delete_multitopic(&mt).unwrap();
    p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT id, amount FROM Derived",
        Vec::new(),
    )
    .unwrap();
    // A field that is neither the base's nor the structure's.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT2",
        "OrderCopy",
        "SELECT id, nothing AS amount FROM Derived",
        Vec::new()
    )));
}

#[test]
fn the_expression_parameters_are_checked_and_can_be_set() {
    let p = participant_with_topics();
    // %1 with one parameter.
    assert!(bad_parameter(p.create_dynamic_multitopic(
        "MT",
        "OrderCopy",
        "SELECT * FROM Order WHERE amount > %1",
        vec!["1".to_string()]
    )));
    let mt = p
        .create_dynamic_multitopic(
            "MT",
            "OrderCopy",
            "SELECT * FROM Order WHERE amount > %0 OR id = %1",
            vec!["1".to_string(), "2".to_string()],
        )
        .unwrap();
    assert_eq!(
        mt.get_subscription_expression(),
        "SELECT * FROM Order WHERE amount > %0 OR id = %1"
    );
    assert_eq!(
        mt.get_expression_parameters(),
        vec!["1".to_string(), "2".to_string()]
    );
    assert!(bad_parameter(
        mt.set_expression_parameters(vec!["only_zero".to_string()])
    ));
    let other_handle = mt.clone();
    mt.set_expression_parameters(vec!["3".to_string(), "4".to_string()])
        .unwrap();
    assert_eq!(
        other_handle.get_expression_parameters(),
        vec!["3".to_string(), "4".to_string()]
    );
}

#[test]
fn a_multitopic_of_a_foreign_participant_is_not_deleted() {
    let p = participant_with_topics();
    let q = offline();
    let mt = p
        .create_dynamic_multitopic("MT", "OrderCopy", "SELECT * FROM Order", Vec::new())
        .unwrap();
    assert!(matches!(
        q.delete_multitopic(&mt).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
}

#[cfg(target_os = "linux")]
mod live {
    use std::time::{Duration, Instant};

    use zerodds_dcps::listener::DataReaderListener;
    use zerodds_dcps::psm_constants::status;
    use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
    use zerodds_dcps::sample::Sample;
    use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter};
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
        HANDLE_NIL, InstanceHandle, InstanceStateKind, PublisherQos, SubscriberQos, TopicQos,
        ViewStateKind, instance_state_mask, sample_state_mask, view_state_mask,
    };
    use zerodds_types::dynamic::{DynamicSample, DynamicType};

    use super::common;

    fn reliable_keep_all() -> (DataWriterQos, DataReaderQos) {
        let history = HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        };
        let reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..Default::default()
        };
        (
            DataWriterQos {
                reliability,
                history,
                ..Default::default()
            },
            DataReaderQos {
                reliability,
                history,
                ..Default::default()
            },
        )
    }

    fn order(type_: &DynamicType, id: i32, amount: f64) -> DynamicSample {
        let mut s = DynamicSample::new(type_).unwrap();
        let ids = (
            s.get_member_id_by_name("id").unwrap(),
            s.get_member_id_by_name("amount").unwrap(),
        );
        s.set_int32_value(ids.0, id).unwrap();
        s.set_float64_value(ids.1, amount).unwrap();
        s
    }

    fn customer(type_: &DynamicType, id: i32, country: &str) -> DynamicSample {
        let mut s = DynamicSample::new(type_).unwrap();
        let ids = (
            s.get_member_id_by_name("id").unwrap(),
            s.get_member_id_by_name("country").unwrap(),
        );
        s.set_int32_value(ids.0, id).unwrap();
        s.set_string_value(ids.1, country).unwrap();
        s
    }

    fn int32(sample: &DynamicSample, name: &str) -> i32 {
        let mut s = sample.share();
        let id = s.get_member_id_by_name(name).unwrap();
        s.get_int32_value(id).unwrap()
    }

    fn float64(sample: &DynamicSample, name: &str) -> f64 {
        let mut s = sample.share();
        let id = s.get_member_id_by_name(name).unwrap();
        s.get_float64_value(id).unwrap()
    }

    fn string(sample: &DynamicSample, name: &str) -> String {
        let mut s = sample.share();
        let id = s.get_member_id_by_name(name).unwrap();
        s.get_string_value(id).unwrap()
    }

    #[derive(Default)]
    struct Heard(std::sync::atomic::AtomicU32);

    impl DataReaderListener for Heard {
        fn on_data_available(&self, _reader: InstanceHandle) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Takes from `reader` until `count` samples came or five seconds passed.
    fn take_until(reader: &DynamicDataReader, count: usize) -> Vec<Sample<DynamicSample>> {
        let mut got = Vec::new();
        let until = Instant::now() + Duration::from_secs(5);
        while got.len() < count && Instant::now() < until {
            got.extend(reader.take_with_info().unwrap());
            std::thread::sleep(Duration::from_millis(10));
        }
        got
    }

    /// Two participants: A writes `Order` and `Customer`, B reads the
    /// MultiTopic `expression` of the result type `result`.
    struct Rig {
        _pa: common::ParticipantGuard,
        pb: common::ParticipantGuard,
        order_type: DynamicType,
        customer_type: DynamicType,
        orders: DynamicDataWriter,
        customers: DynamicDataWriter,
        multitopic: zerodds_dcps::MultiTopic<DynamicSample>,
        subscriber: zerodds_dcps::Subscriber,
        reader: DynamicDataReader,
        // Last: fields drop in declaration order, and the domain goes back to
        // the pool only after the participants on it are gone.
        _lease: common::DomainLease,
    }

    fn rig(expression: &str, result: (&str, &str), parameters: Vec<String>) -> Rig {
        rig_with(expression, result, parameters, true)
    }

    /// [`rig`], with `autodispose_unregistered_instances` of the writer of
    /// `Order` chosen.
    fn rig_with(
        expression: &str,
        result: (&str, &str),
        parameters: Vec<String>,
        autodispose: bool,
    ) -> Rig {
        rig_full(
            expression,
            result,
            parameters,
            autodispose,
            (common::ORDER_IDL, "Order", &[]),
        )
    }

    /// [`rig_with`], with the type of the topic `Order` chosen: its IDL, its
    /// name and the base structures it extends.
    fn rig_full(
        expression: &str,
        result: (&str, &str),
        parameters: Vec<String>,
        autodispose: bool,
        order: (&str, &str, &[&str]),
    ) -> Rig {
        let lease = common::lease_domain();
        let cfg = common::isolated_cfg();
        let factory = DomainParticipantFactory::instance();
        let (wq, rq) = reliable_keep_all();
        let order_writer_qos = DataWriterQos {
            writer_data_lifecycle: zerodds_qos::WriterDataLifecycleQosPolicy {
                autodispose_unregistered_instances: autodispose,
            },
            ..wq.clone()
        };
        let order_support = common::dynamic_support_with_bases(order.0, order.1, order.2, true);
        let customer_support = common::dynamic_support(common::CUSTOMER_IDL, "Customer", true);
        let pa = common::ParticipantGuard::new(
            factory
                .create_participant_with_config(
                    lease.id(),
                    DomainParticipantQos::default(),
                    cfg.clone(),
                )
                .unwrap(),
        );
        let pb = common::ParticipantGuard::new(
            factory
                .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
                .unwrap(),
        );
        let publisher = pa.create_publisher(PublisherQos::default());
        let orders = publisher
            .create_datawriter(
                &pa.create_dynamic_topic("Order", TopicQos::default(), order_support.clone())
                    .unwrap(),
                order_writer_qos,
            )
            .unwrap();
        let customers = publisher
            .create_datawriter(
                &pa.create_dynamic_topic("Customer", TopicQos::default(), customer_support.clone())
                    .unwrap(),
                wq,
            )
            .unwrap();
        pb.create_dynamic_topic("Order", TopicQos::default(), order_support.clone())
            .unwrap();
        pb.create_dynamic_topic("Customer", TopicQos::default(), customer_support.clone())
            .unwrap();
        common::dynamic_support(result.1, result.0, true)
            .register_type(&pb, None)
            .unwrap();
        let multitopic = pb
            .create_dynamic_multitopic("Joined", result.0, expression, parameters)
            .unwrap();
        let subscriber = pb.create_subscriber(SubscriberQos::default());
        let reader = subscriber
            .create_datareader_with_multitopic(&multitopic, rq)
            .unwrap();
        orders
            .wait_for_matched_subscription(1, common::match_timeout())
            .expect("the reader of Order is matched");
        customers
            .wait_for_matched_subscription(1, common::match_timeout())
            .expect("the reader of Customer is matched");
        Rig {
            _lease: lease,
            _pa: pa,
            pb,
            order_type: order_support.get_type().unwrap(),
            customer_type: customer_support.get_type().unwrap(),
            orders,
            customers,
            multitopic,
            subscriber,
            reader,
        }
    }

    /// Annex B.2 `NATURAL JOIN` over the members the topics share: each
    /// order is joined with the customer of its `id`, as the customer and
    /// the order arrive; an order with no customer waits for one.
    #[test]
    fn a_reader_delivers_the_natural_join_of_two_topics() {
        let r = rig(
            "SELECT * FROM Order NATURAL JOIN Customer",
            ("Sale", common::SALE_IDL),
            Vec::new(),
        );
        // DDS 1.4 §2.2.2.3.4: the reader of a MultiTopic is alerted of data
        // "by the usual Listener or Condition mechanisms".
        let heard = std::sync::Arc::new(Heard::default());
        r.reader
            .set_listener(Some(heard.clone()), status::DATA_AVAILABLE);
        r.customers
            .write(&customer(&r.customer_type, 1, "DE"))
            .unwrap();
        r.orders.write(&order(&r.order_type, 1, 50.0)).unwrap();
        r.orders.write(&order(&r.order_type, 2, 70.0)).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while heard.0.load(std::sync::atomic::Ordering::SeqCst) == 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            heard.0.load(std::sync::atomic::Ordering::SeqCst) > 0,
            "on_data_available was not called for the join"
        );
        let first = take_until(&r.reader, 1);
        assert_eq!(first.len(), 1, "only order 1 has a customer");
        assert_eq!(int32(&first[0].data, "id"), 1);
        assert_eq!(float64(&first[0].data, "amount"), 50.0);
        assert_eq!(string(&first[0].data, "country"), "DE");
        assert!(first[0].info.valid_data);

        r.customers
            .write(&customer(&r.customer_type, 2, "US"))
            .unwrap();
        let second = take_until(&r.reader, 1);
        assert_eq!(second.len(), 1, "order 2 found its customer");
        assert_eq!(int32(&second[0].data, "id"), 2);
        assert_eq!(float64(&second[0].data, "amount"), 70.0);
        assert_eq!(string(&second[0].data, "country"), "US");
        // Nothing else is there.
        std::thread::sleep(Duration::from_millis(200));
        assert!(r.reader.take_with_info().unwrap().is_empty());
    }

    /// The `SELECT` list renames, `WHERE` reads a column that is not selected
    /// and an alias, and an expression parameter is a literal.
    #[test]
    fn projection_alias_where_and_parameter() {
        let r = rig(
            "SELECT id, country AS origin FROM Order NATURAL JOIN Customer \
             WHERE amount > %0 AND origin = 'DE'",
            (
                "Origins",
                "@final struct Origins { @key long id; string<16> origin; };",
            ),
            vec!["60.0".to_string()],
        );
        r.customers
            .write(&customer(&r.customer_type, 1, "DE"))
            .unwrap();
        r.customers
            .write(&customer(&r.customer_type, 2, "DE"))
            .unwrap();
        r.customers
            .write(&customer(&r.customer_type, 3, "US"))
            .unwrap();
        r.orders.write(&order(&r.order_type, 1, 50.0)).unwrap(); // amount too low
        r.orders.write(&order(&r.order_type, 2, 70.0)).unwrap(); // passes
        r.orders.write(&order(&r.order_type, 3, 80.0)).unwrap(); // not from DE
        let got = take_until(&r.reader, 1);
        assert_eq!(got.len(), 1);
        assert_eq!(int32(&got[0].data, "id"), 2);
        assert_eq!(string(&got[0].data, "origin"), "DE");
        std::thread::sleep(Duration::from_millis(200));
        assert!(r.reader.take_with_info().unwrap().is_empty());

        // set_expression_parameters reaches the join.
        r.multitopic
            .set_expression_parameters(vec!["10.0".to_string()])
            .unwrap();
        r.orders.write(&order(&r.order_type, 1, 50.0)).unwrap();
        let again = take_until(&r.reader, 1);
        assert_eq!(again.len(), 1);
        assert_eq!(int32(&again[0].data, "id"), 1);
    }

    /// §2.2.2.3.4: the `instance_state` of the result instance is
    /// NOT_ALIVE_DISPOSED once a constituting instance is disposed.
    #[test]
    fn a_disposed_constituent_disposes_the_result_instance() {
        let r = rig(
            "SELECT * FROM Order NATURAL JOIN Customer",
            ("Sale", common::SALE_IDL),
            Vec::new(),
        );
        let customer_one = customer(&r.customer_type, 1, "DE");
        let order_one = order(&r.order_type, 1, 50.0);
        r.customers.write(&customer_one).unwrap();
        r.orders.write(&order_one).unwrap();
        let joined = take_until(&r.reader, 1);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].info.instance_state, InstanceStateKind::Alive);
        let handle = joined[0].info.instance_handle;

        r.orders.dispose(&order_one, HANDLE_NIL).unwrap();
        let disposed = take_until(&r.reader, 1);
        assert_eq!(disposed.len(), 1);
        assert!(!disposed[0].info.valid_data);
        assert_eq!(
            disposed[0].info.instance_state,
            InstanceStateKind::NotAliveDisposed
        );
        assert_eq!(disposed[0].info.instance_handle, handle);
    }

    /// IDL 4.2 struct inheritance: `id` is a field of `DerivedOrder` that its
    /// base declares, and it is what the join with `Customer` runs over.
    #[test]
    fn the_join_runs_over_an_inherited_member() {
        let r = rig_full(
            "SELECT * FROM Order NATURAL JOIN Customer WHERE id = 1",
            ("Sale", common::SALE_IDL),
            Vec::new(),
            true,
            (common::DERIVED_ORDER_IDL, "DerivedOrder", &["OrderBase"]),
        );
        r.customers
            .write(&customer(&r.customer_type, 1, "DE"))
            .unwrap();
        r.customers
            .write(&customer(&r.customer_type, 2, "US"))
            .unwrap();
        r.orders.write(&order(&r.order_type, 1, 50.0)).unwrap();
        r.orders.write(&order(&r.order_type, 2, 70.0)).unwrap();
        let got = take_until(&r.reader, 1);
        assert_eq!(got.len(), 1, "WHERE id = 1 on the inherited member");
        assert_eq!(int32(&got[0].data, "id"), 1);
        assert_eq!(float64(&got[0].data, "amount"), 50.0);
        assert_eq!(string(&got[0].data, "country"), "DE");
    }

    /// §2.2.2.3.4: "The view_state of the MultiTopic instance is NEW if at
    /// least one of the constituting instances has view_state = NEW,
    /// otherwise it will be NOT_NEW." Order instances (id, line) map to the
    /// result instance of their id: a second order instance of an id the
    /// application has read is a NEW constituent, and makes the result
    /// instance NEW again; a new sample of a known one does not.
    #[test]
    fn a_new_constituent_makes_the_result_instance_new() {
        let r = rig_full(
            "SELECT id, amount, country FROM Order NATURAL JOIN Customer",
            (
                "Billed",
                "@final struct Billed { @key long id; double amount; string<16> country; };",
            ),
            Vec::new(),
            true,
            (
                "@final struct LineOrder { @key long id; @key long line; double amount; };",
                "LineOrder",
                &[],
            ),
        );
        let line = |id: i32, line: i32, amount: f64| {
            let mut s = DynamicSample::new(&r.order_type).unwrap();
            for (name, value) in [("id", id), ("line", line)] {
                let member = s.get_member_id_by_name(name).unwrap();
                s.set_int32_value(member, value).unwrap();
            }
            let member = s.get_member_id_by_name("amount").unwrap();
            s.set_float64_value(member, amount).unwrap();
            s
        };
        r.customers
            .write(&customer(&r.customer_type, 1, "DE"))
            .unwrap();
        r.orders.write(&line(1, 1, 10.0)).unwrap();
        let first = take_until(&r.reader, 1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].info.view_state, ViewStateKind::New);

        // The same constituents again: the instance has been seen.
        r.orders.write(&line(1, 1, 11.0)).unwrap();
        let again = take_until(&r.reader, 1);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].info.view_state, ViewStateKind::NotNew);

        // A second order instance of the same id: a NEW constituent.
        r.orders.write(&line(1, 2, 20.0)).unwrap();
        let second = take_until(&r.reader, 1);
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].info.instance_handle,
            first[0].info.instance_handle
        );
        assert_eq!(second[0].info.view_state, ViewStateKind::New);
    }

    /// §2.2.2.3.4: NOT_ALIVE_NO_WRITERS when a constituting instance has no
    /// writers and none is disposed.
    #[test]
    fn an_unregistered_constituent_leaves_the_result_instance_without_writers() {
        let r = rig_with(
            "SELECT * FROM Order NATURAL JOIN Customer",
            ("Sale", common::SALE_IDL),
            Vec::new(),
            false,
        );
        let order_one = order(&r.order_type, 1, 50.0);
        r.customers
            .write(&customer(&r.customer_type, 1, "DE"))
            .unwrap();
        r.orders.write(&order_one).unwrap();
        let joined = take_until(&r.reader, 1);
        assert_eq!(joined.len(), 1);

        r.orders
            .unregister_instance(&order_one, HANDLE_NIL)
            .unwrap();
        let gone = take_until(&r.reader, 1);
        assert_eq!(gone.len(), 1);
        assert!(!gone[0].info.valid_data);
        assert_eq!(
            gone[0].info.instance_state,
            InstanceStateKind::NotAliveNoWriters
        );
        assert_eq!(gone[0].info.instance_handle, joined[0].info.instance_handle);
    }

    #[derive(Default)]
    struct Told(std::sync::atomic::AtomicU32);

    impl zerodds_dcps::listener::SubscriberListener for Told {
        fn on_data_on_readers(&self, _subscriber: InstanceHandle) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl zerodds_dcps::listener::DomainParticipantListener for Told {
        fn on_subscription_matched(
            &self,
            _reader: InstanceHandle,
            _status: zerodds_dcps::status::SubscriptionMatchedStatus,
        ) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// The readers of the topics of a MultiTopic reader are its own: the
    /// application finds only the reader of the MultiTopic, hears nothing
    /// of them as DATA_ON_READERS of the subscriber or as a status at the
    /// participant, and they count as the users of the topics they are.
    #[test]
    fn the_readers_of_the_topics_are_not_visible_to_the_application() {
        let r = rig(
            "SELECT * FROM Order NATURAL JOIN Customer",
            ("Sale", common::SALE_IDL),
            Vec::new(),
        );
        // Not found by their topics or by their type.
        assert!(
            r.subscriber
                .lookup_datareader::<zerodds_dcps::serialized_type::SerializedSample>("Order")
                .is_none()
        );
        assert!(
            r.subscriber
                .lookup_datareader::<DynamicSample>("Order")
                .is_none()
        );
        assert!(
            r.subscriber
                .lookup_datareader::<DynamicSample>("Customer")
                .is_none()
        );
        // The reader of the MultiTopic is.
        assert_eq!(
            r.subscriber
                .lookup_datareader::<DynamicSample>("Joined")
                .unwrap()
                .instance_handle(),
            r.reader.instance_handle()
        );

        // Data for the topics, none for the join: nothing reaches the
        // subscriber's or the participant's listener.
        let told = std::sync::Arc::new(Told::default());
        r.subscriber
            .set_listener(Some(told.clone()), status::DATA_ON_READERS);
        r.pb.set_listener(Some(told.clone()), status::SUBSCRIPTION_MATCHED);
        r.orders.write(&order(&r.order_type, 5, 1.0)).unwrap();
        r.customers
            .write(&customer(&r.customer_type, 6, "DE"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(
            told.0.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the readers of the topics told a listener above them"
        );
        assert!(r.reader.take_with_info().unwrap().is_empty());

        // They are the users of their topics until the reader of the
        // MultiTopic goes (DDS 1.4 §2.2.2.2.1.6), and all go with
        // `delete_contained_entities`.
        assert_eq!(r.pb.topic_users("Order").unwrap().data_readers, 1);
        r.subscriber.delete_contained_entities().unwrap();
        assert_eq!(r.pb.topic_users("Order").unwrap().data_readers, 0);
        assert_eq!(r.pb.topic_users("Customer").unwrap().data_readers, 0);
        assert!(!r.subscriber.has_datareaders());
    }

    /// §2.2.2.2.1.10: a MultiTopic with a DataReader attached is not
    /// deleted; the readers of its topics end with the reader.
    #[test]
    fn a_multitopic_with_a_reader_is_not_deleted_and_its_topic_readers_end_with_it() {
        let r = rig(
            "SELECT * FROM Order NATURAL JOIN Customer",
            ("Sale", common::SALE_IDL),
            Vec::new(),
        );
        assert!(matches!(
            r.pb.delete_multitopic(&r.multitopic).unwrap_err(),
            DdsError::PreconditionNotMet { .. }
        ));
        let before = r
            .subscriber
            .get_datareaders(
                sample_state_mask::ANY,
                view_state_mask::ANY,
                instance_state_mask::ANY,
            )
            .unwrap()
            .len();
        assert_eq!(before, 0, "no reader holds samples yet");
        assert!(r.subscriber.has_datareaders());
        r.subscriber.delete_datareader(&r.reader).unwrap();
        assert!(
            !r.subscriber.has_datareaders(),
            "the readers of Order and Customer went with it"
        );
        r.pb.delete_multitopic(&r.multitopic).unwrap();
    }
}
