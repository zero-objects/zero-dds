// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 Annex B: the syntax of DLRL queries and filters, checked
//! against a class and evaluated over its objects; the `content_filter`
//! of an `ObjectHome` (§8.1.6.3.7) between two Caches.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::collections::BTreeMap;

use zerodds_dlrl::criterion::{QueryExpression, QueryGrammar};
use zerodds_dlrl::error::DlrlException;
use zerodds_dlrl::model::{
    AttributeValue, BasicItem, BasicType, ClassModel, CollectionBasis, EnumerationType, Model,
    OBJECT_ROOT, ObjectRef, Relation, SequenceValue, SimpleStructType, SimpleType, Value,
};
use zerodds_dlrl::rows::ObjectData;
use zerodds_dlrl::types::DlrlOid;

fn basic(b: BasicType) -> SimpleType {
    SimpleType::Basic(b)
}

fn color() -> EnumerationType {
    EnumerationType::new("Color", &["RED", "GREEN", "BLUE"])
}

fn model() -> Model {
    Model::new(vec![
        ClassModel::new("Radar")
            .base(OBJECT_ROOT)
            .mono("name", basic(BasicType::String))
            .mono("range", basic(BasicType::Double)),
        ClassModel::new("Track")
            .base(OBJECT_ROOT)
            .mono("id", basic(BasicType::Long))
            .mono("small", basic(BasicType::Short))
            .mono("flags", basic(BasicType::Octet))
            .mono("speed", basic(BasicType::Real))
            .mono("x", basic(BasicType::Double))
            .mono("y", basic(BasicType::Double))
            .mono("class_code", basic(BasicType::Char))
            .mono("label", basic(BasicType::String))
            .mono("color", SimpleType::Enumeration(color()))
            .mono(
                "at",
                SimpleType::Struct(SimpleStructType::new(
                    "Position",
                    &[
                        ("lat", basic(BasicType::Double)),
                        ("lon", basic(BasicType::Double)),
                    ],
                )),
            )
            .mono("history", basic(BasicType::Sequence(BasicItem::Long)))
            .multi("marks", CollectionBasis::List, basic(BasicType::Long))
            .multi(
                "comments",
                CollectionBasis::StrMap,
                basic(BasicType::String),
            )
            .multi("levels", CollectionBasis::IntMap, basic(BasicType::Double))
            .multi("tags", CollectionBasis::Set, basic(BasicType::String))
            .relation(Relation::to_one("a_radar", "Radar"))
            .relation(Relation::to_many("seen_by", "Radar", CollectionBasis::List))
            .local("note"),
    ])
    .unwrap()
}

fn oid(n: i32) -> DlrlOid {
    DlrlOid::new(0, 1, n)
}

fn mono(v: Value) -> AttributeValue {
    AttributeValue::Mono(v)
}

fn radar(n: i32, name: &str, range: f64) -> ObjectData {
    ObjectData::new("Radar", oid(n))
        .with("name", mono(Value::String(name.into())))
        .with("range", mono(Value::Double(range)))
}

fn track() -> ObjectData {
    ObjectData::new("Track", oid(1))
        .with("id", mono(Value::Long(7)))
        .with("small", mono(Value::Short(-3)))
        .with("flags", mono(Value::Octet(0x10)))
        .with("speed", mono(Value::Real(2.5)))
        .with("x", mono(Value::Double(10.0)))
        .with("y", mono(Value::Double(20.0)))
        .with("class_code", mono(Value::Char('A')))
        .with("label", mono(Value::String("TRACK-7".into())))
        .with(
            "color",
            mono(Value::Enum {
                type_name: "Color".into(),
                enumerator: "GREEN".into(),
            }),
        )
        .with(
            "at",
            mono(Value::Struct {
                type_name: "Position".into(),
                fields: vec![
                    ("lat".into(), Value::Double(48.1)),
                    ("lon".into(), Value::Double(11.5)),
                ],
            }),
        )
        .with(
            "history",
            mono(Value::Sequence(SequenceValue::Long(vec![1, 2, 3]))),
        )
        .with(
            "marks",
            AttributeValue::List(vec![Value::Long(5), Value::Long(6)]),
        )
        .with(
            "comments",
            AttributeValue::StrMap(BTreeMap::from([(
                "it is".to_string(),
                Value::String("ok".into()),
            )])),
        )
        .with(
            "levels",
            AttributeValue::IntMap(BTreeMap::from([(-1, Value::Double(0.5))])),
        )
        .with("tags", AttributeValue::Set(vec![Value::String("a".into())]))
        .with(
            "a_radar",
            mono(Value::Ref(Some(ObjectRef::new("Radar", oid(10))))),
        )
        .with(
            "seen_by",
            AttributeValue::List(vec![
                Value::Ref(Some(ObjectRef::new("Radar", oid(10)))),
                Value::Ref(Some(ObjectRef::new("Radar", oid(11)))),
            ]),
        )
}

fn resolve(r: &ObjectRef) -> Option<ObjectData> {
    match r.oid {
        o if o == oid(10) => Some(radar(10, "north", 120.0)),
        o if o == oid(11) => Some(radar(11, "south", 80.0)),
        _ => None,
    }
}

fn params(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_string()).collect()
}

fn query(expr: &str, p: &[&str]) -> Result<QueryExpression, DlrlException> {
    QueryExpression::new(&model(), "Track", expr, &params(p))
}

fn passes(expr: &str, p: &[&str]) -> bool {
    query(expr, p)
        .unwrap_or_else(|e| panic!("{expr}: {e}"))
        .check(&track(), &resolve)
}

fn sql_error(expr: &str, p: &[&str]) {
    match query(expr, p) {
        Err(DlrlException::SqlError { message }) => assert!(!message.is_empty()),
        other => panic!("{expr}: expected SQLError, got {other:?}"),
    }
}

/// B.1: every production of the grammar parses and evaluates —
/// Condition with AND, OR, NOT and parentheses, the three forms of
/// ComparisonPredicate, BETWEEN and NOT BETWEEN with a Range, the six
/// RelOps.
#[test]
fn dlrl_b_1_every_production_parses_and_evaluates() {
    // ComparisonPredicate: FIELDNAME RelOp Parameter, each RelOp.
    for (e, want) in [
        ("id = 7", true),
        ("id = 8", false),
        ("id > 6", true),
        ("id > 7", false),
        ("id >= 7", true),
        ("id >= 8", false),
        ("id < 8", true),
        ("id < 7", false),
        ("id <= 7", true),
        ("id <= 6", false),
        ("id <> 8", true),
        ("id <> 7", false),
    ] {
        assert_eq!(passes(e, &[]), want, "{e}");
    }
    // Parameter RelOp FIELDNAME and FIELDNAME RelOp FIELDNAME.
    assert!(passes("6 < id", &[]));
    assert!(!passes("8 < id", &[]));
    assert!(passes("x < y", &[]));
    assert!(!passes("x = y", &[]));
    assert!(passes("id < x", &[]));
    // BetweenPredicate with a Range of two Parameters.
    assert!(passes("x BETWEEN 10 AND 11", &[]));
    assert!(!passes("x BETWEEN 10.5 AND 11", &[]));
    assert!(passes("x NOT BETWEEN 11 AND 12", &[]));
    assert!(!passes("x NOT BETWEEN 9 AND 10", &[]));
    // Condition: AND, OR, NOT, parentheses.
    assert!(passes("id = 7 AND x = 10", &[]));
    assert!(!passes("id = 7 AND x = 11", &[]));
    assert!(passes("id = 8 OR x = 10", &[]));
    assert!(!passes("id = 8 OR x = 11", &[]));
    assert!(passes("NOT id = 8", &[]));
    assert!(!passes("NOT (id = 7 OR x = 11)", &[]));
    assert!(passes("(id = 8 OR id = 7) AND NOT (x > 100)", &[]));
    // Keywords in any case.
    assert!(passes("id = 7 and not x between 0 and 1", &[]));
}

/// B.1: what Annex B does not have is SQLError — LIKE, ORDER BY, '!=',
/// a comparison without a field, a field as a bound of BETWEEN.
#[test]
fn dlrl_b_1_like_and_order_by_are_sql_error() {
    for e in [
        "label LIKE 'TRACK%'",
        "id = 7 ORDER BY x",
        "id != 7",
        "1 = 1",
        "x BETWEEN y AND 3",
        "id =",
        "(id = 7",
        "id = TRUE",
    ] {
        sql_error(e, &[]);
    }
    // The DDS 1.4 grammar is a named option, with LIKE.
    let q = QueryExpression::with_grammar(
        &model(),
        "Track",
        "label LIKE 'TRACK-%' AND id != 8",
        &[],
        QueryGrammar::Dcps14,
    )
    .unwrap();
    assert_eq!(q.grammar(), QueryGrammar::Dcps14);
    assert!(q.check(&track(), &resolve));
    // LIKE needs a string field there too.
    assert!(matches!(
        QueryExpression::with_grammar(&model(), "Track", "id LIKE '7'", &[], QueryGrammar::Dcps14),
        Err(DlrlException::SqlError { .. })
    ));
}

/// B.2 FIELDNAME: '.' into a structure and across a to-1 relation,
/// '[n]' into a List, a sequence and a to-many relation, '[n]' with a
/// negative key into an IntMap, "['key']" into a StrMap.
#[test]
fn dlrl_b_2_fieldname_navigates_structure_collection_and_relation() {
    assert!(passes("at.lat > 48 AND at.lon < 12", &[]));
    assert!(passes("a_radar.name = 'north'", &[]));
    assert!(passes("a_radar.range >= 120", &[]));
    assert!(passes("marks[1] = 6", &[]));
    assert!(passes("history[2] = 3", &[]));
    assert!(passes("seen_by[1].name = 'south'", &[]));
    assert!(passes("levels[-1] = 0.5", &[]));
    assert!(passes("comments['it is'] = 'ok'", &[]));
    assert!(passes("comments[`it is'] = 'ok'", &[]));
    // An element that is absent makes the predicate unknown: neither the
    // predicate nor its negation passes.
    assert!(!passes("marks[5] = 6", &[]));
    assert!(!passes("NOT marks[5] = 6", &[]));
    assert!(!passes("comments['none'] = 'ok'", &[]));
    assert!(!passes("seen_by[2].name = 'south'", &[]));
    // Unknown OR TRUE is TRUE.
    assert!(passes("marks[5] = 6 OR id = 7", &[]));
    // A relation that designates an object that is not available.
    let mut t = track();
    t.values.insert(
        "a_radar".into(),
        mono(Value::Ref(Some(ObjectRef::new("Radar", oid(99))))),
    );
    let q = query("a_radar.name = 'north'", &[]).unwrap();
    assert!(!q.check(&t, &resolve));
    t.values.insert("a_radar".into(), mono(Value::Ref(None)));
    assert!(!q.check(&t, &resolve));
    // Fields the class does not have, or that designate no value.
    for e in [
        "nothing = 1",
        "at.alt = 1",
        "at = 1",
        "a_radar = 1",
        "a_radar.size = 1",
        "marks = 1",
        "marks['a'] = 1",
        "comments[1] = 'ok'",
        "tags[0] = 'a'",
        "history = 1",
        "id.part = 1",
        "id[0] = 1",
        "note = 1",
        "seen_by.name = 'x'",
        // The key is a STRING of B.2: no '' escape inside it.
        "comments['it''s'] = 'ok'",
    ] {
        sql_error(e, &[]);
    }
}

/// B.2 PARAMETER: %n with n smaller than 100, the n+1th argument. The
/// DDS 1.4 grammar has the same bound (DDS 1.4 Annex B): %100 is
/// refused there too, also with 101 parameters, and so is %4294967295,
/// whose count n + 1 overflows on a 32-bit target.
#[test]
fn dlrl_b_2_parameter_100_is_sql_error() {
    let mut p = vec!["0"; 100];
    p[99] = "7";
    assert!(passes("id = %99", &p));
    sql_error("id = %100", &[]);
    let dcps14 = |expr: &str, p: &[&str]| {
        QueryExpression::with_grammar(&model(), "Track", expr, &params(p), QueryGrammar::Dcps14)
    };
    assert!(dcps14("id = %99", &p).unwrap().check(&track(), &resolve));
    let mut p101 = p.clone();
    p101.push("7");
    for (expr, args) in [
        ("id = %100", &p101[..]),
        ("id = %4294967295", &[][..]),
        ("id = %4294967295", &p[..1]),
    ] {
        match dcps14(expr, args) {
            Err(DlrlException::SqlError { message }) => {
                assert!(message.contains("smaller than 100"), "{expr}: {message}");
            }
            other => panic!("{expr}: expected SQLError, got {other:?}"),
        }
    }
    let mut q = query("id BETWEEN %0 AND %1", &["5", "9"]).unwrap();
    assert_eq!(q.required_parameters(), 2);
    assert!(q.check(&track(), &resolve));
    q.set_parameters(&params(&["8", "9"])).unwrap();
    assert_eq!(q.parameters(), ["8", "9"]);
    assert!(!q.check(&track(), &resolve));
    // %n refers to the n+1th argument.
    assert!(passes("id = %1 AND x = %0", &["10", "7"]));
}

/// B.2: INTEGERVALUE with sign and 0x, CHARVALUE, FLOATVALUE with an
/// exponent, STRING opened by either quote, ENUMERATEDVALUE.
#[test]
fn dlrl_b_2_hexadecimal_char_float_string_and_enumerated_values() {
    assert!(passes("flags = 0x10", &[]));
    assert!(passes("small = -3", &[]));
    assert!(passes("id = +7", &[]));
    assert!(passes("small < -0x2", &[]));
    assert!(passes("class_code = 'A'", &[]));
    assert!(passes("class_code < 'B'", &[]));
    assert!(passes("speed = 2.5", &[]));
    assert!(passes("x = 1.0e1", &[]));
    assert!(passes("y = 2e+1", &[]));
    assert!(passes("label = 'TRACK-7'", &[]));
    assert!(passes("label = `TRACK-7'", &[]));
    assert!(passes("color = 'GREEN'", &[]));
    // Enumerations compare by the position of their enumerators.
    assert!(passes("color > 'RED' AND color < 'BLUE'", &[]));
    assert!(passes("color BETWEEN 'RED' AND 'GREEN'", &[]));
    // The same values as parameters.
    assert!(passes(
        "flags = %0 AND class_code = %1 AND x = %2 AND label = %3 AND color = %4",
        &["0x10", "'A'", "1e1", "`TRACK-7'", "'GREEN'"],
    ));
    // Constants that do not fit their field.
    for e in [
        "class_code = 'AB'",
        "color = 'PURPLE'",
        "id = 'seven'",
        "label = 7",
        "x = label",
        "color = id",
        "label = 'a\nb'",
    ] {
        sql_error(e, &[]);
    }
    // A parameter that is not one Parameter token, or does not fit.
    sql_error("id = %0", &["seven"]);
    sql_error("id = %0", &["7 OR id = 8"]);
    sql_error("color = %0", &["'PURPLE'"]);
    sql_error("class_code = %0", &["'AB'"]);
}

/// §8.1.6.3.12: the number of parameters must fit the expression; a
/// failed set_parameters keeps the parameters it had.
#[test]
fn dlrl_b_2_too_few_parameters_are_sql_error() {
    sql_error("id = %0", &[]);
    sql_error("id = %1", &["7"]);
    sql_error("id = 7", &["7"]);
    // A parameter whose %n the expression does not use is still one
    // Parameter token.
    sql_error("id = %1", &["garbage", "7"]);
    sql_error("id = %1", &["1 OR id = 2", "7"]);
    let mut gap = query("id = %1", &["'x'", "7"]).unwrap();
    assert!(gap.check(&track(), &resolve));
    assert!(matches!(
        gap.set_parameters(&params(&["%0", "7"])),
        Err(DlrlException::SqlError { .. })
    ));
    assert_eq!(gap.parameters(), ["'x'", "7"]);
    let mut q = query("id = %0", &["7"]).unwrap();
    assert!(matches!(
        q.set_parameters(&params(&["7", "8"])),
        Err(DlrlException::SqlError { .. })
    ));
    assert!(matches!(
        q.set_parameters(&params(&["'x'"])),
        Err(DlrlException::SqlError { .. })
    ));
    assert_eq!(q.parameters(), ["7"]);
    assert!(q.check(&track(), &resolve));
    // An unknown class.
    assert!(matches!(
        QueryExpression::new(&model(), "Nothing", "id = 1", &[]),
        Err(DlrlException::SqlError { .. })
    ));
}

#[cfg(feature = "dcps")]
#[path = "support/dds.rs"]
mod dds;

/// The FIELDNAMEs of an expression that go across a relation, to-1 or
/// to-many; a structure, a collection of values and a sequence do not.
#[test]
fn dlrl_b_2_fields_across_relations_are_named() {
    let q = query(
        "a_radar.name = 'north' AND seen_by[1].range > 1 AND at.lat > 1 AND marks[0] = 5 \
         AND comments['it is'] = 'ok' AND history[0] = 1",
        &[],
    )
    .unwrap();
    assert_eq!(
        q.fields_across_relations(),
        ["a_radar.name", "seen_by[1].range"]
    );
    assert!(
        query("x > 1", &[])
            .unwrap()
            .fields_across_relations()
            .is_empty()
    );
}

/// §8.1.6.3.7: a content filter "is intended to be mapped on the
/// underlying DCPS infrastructure"; it decides on the data of the
/// incoming object, so a FIELDNAME across a relation is SQLError, to-1
/// and to-many, also beside a field of the class itself, and the home
/// keeps no filter.
#[cfg(feature = "dcps")]
#[test]
fn dlrl_8_1_6_3_7_a_content_filter_across_a_relation_is_sql_error() {
    use std::sync::Arc;

    use zerodds_dlrl::cache::ObjectHome;

    let m = dds::mapping();
    let tracks = ObjectHome::new(Arc::clone(&m), "Track").unwrap();
    for e in [
        "a_radar.name = 'north'",
        "x > 5 AND a_radar.name <> 'south'",
    ] {
        match tracks.set_content_filter(e) {
            Err(DlrlException::SqlError { message }) => {
                assert!(message.contains("a_radar.name"), "{message}");
            }
            other => panic!("{e}: expected SQLError, got {other:?}"),
        }
        assert_eq!(tracks.content_filter(), None);
    }
    let sensors = ObjectHome::new(Arc::clone(&m), "Sensor").unwrap();
    assert!(matches!(
        sensors.set_content_filter("parts[0].serial = 1"),
        Err(DlrlException::SqlError { .. })
    ));
    assert_eq!(sensors.content_filter(), None);
    // A field of the class itself, also in a collection of values.
    tracks
        .set_content_filter("x > 5 AND comments['k'] = 'v'")
        .unwrap();
    sensors.set_content_filter("label = 's'").unwrap();
}

/// §8.1.6.3.7: the content filter of a home is set before registration
/// only, an invalid one is SQLError, and only the incoming objects that
/// pass it are created in the Cache; an update that no longer passes is
/// not applied.
#[cfg(feature = "dcps")]
#[test]
fn dlrl_8_1_6_3_7_content_filter_lets_only_passing_objects_into_the_cache() {
    use std::sync::Arc;
    use std::time::Duration;

    use dds::{Bus, dbl, delete, enabled, mapping, stays, wait_until};
    use zerodds_dlrl::cache::{CacheBase, CacheDescription, CacheFactory, CacheUsage, ObjectHome};

    let bus = Bus::new();
    let m = mapping();
    let classes = ["Radar", "Track", "Part", "Sensor"];
    let participant = bus.participant();
    let cache = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadOnly,
            CacheDescription {
                name: format!("annex-b-filter-{}", std::process::id()),
                domain: participant.clone(),
            },
        )
        .unwrap();
    let homes: Vec<ObjectHome> = classes
        .iter()
        .map(|c| ObjectHome::new(Arc::clone(&m), c).unwrap())
        .collect();
    let tracks = homes[1].clone();
    assert_eq!(tracks.content_filter(), None);
    assert!(matches!(
        tracks.set_content_filter("x >"),
        Err(DlrlException::SqlError { .. })
    ));
    assert!(matches!(
        tracks.set_content_filter("z > 1"),
        Err(DlrlException::SqlError { .. })
    ));
    assert!(matches!(
        tracks.set_content_filter("x > %0"),
        Err(DlrlException::SqlError { .. })
    ));
    assert_eq!(tracks.content_filter(), None);
    tracks.set_content_filter("x > 5").unwrap();
    assert_eq!(tracks.content_filter().as_deref(), Some("x > 5"));
    for h in &homes {
        cache.register_home(h).unwrap();
    }
    assert!(matches!(
        tracks.set_content_filter("x > 6"),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert_eq!(tracks.content_filter().as_deref(), Some("x > 5"));
    cache.register_all_for_pubsub().unwrap();
    cache.enable_all_for_pubsub().unwrap();

    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &classes);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let new_track = |x: f64| {
        let t = publ.home("Track").create_object(&access).unwrap();
        t.set("x", dds::mono(Value::Double(x))).unwrap();
        t.set("y", dds::mono(Value::Double(0.0))).unwrap();
        t.set("a_radar", dds::mono(Value::Ref(None))).unwrap();
        t
    };
    let low = new_track(1.0);
    let high = new_track(9.0);
    access.write().unwrap();
    let base = CacheBase::Cache(cache.clone());
    wait_until("the passing track", || {
        tracks.find_object(high.oid(), &base).is_ok()
    });
    stays(
        "the filtered track stays out",
        Duration::from_millis(500),
        || tracks.find_object(low.oid(), &base).is_err(),
    );
    let got = tracks.find_object(high.oid(), &base).unwrap();
    assert_eq!(dbl(&got, "x"), 9.0);
    // An update that no longer passes is not applied; one that passes is.
    high.set("x", dds::mono(Value::Double(2.0))).unwrap();
    access.write().unwrap();
    stays("the old state", Duration::from_millis(500), || {
        dbl(&got, "x") == 9.0
    });
    high.set("x", dds::mono(Value::Double(7.0))).unwrap();
    // The low track now passes and enters the Cache.
    low.set("x", dds::mono(Value::Double(6.0))).unwrap();
    access.write().unwrap();
    wait_until("the passing update", || dbl(&got, "x") == 7.0);
    wait_until("the low track once it passes", || {
        tracks.find_object(low.oid(), &base).is_ok()
    });
    delete(publ);
    CacheFactory::get_instance().delete_cache(&cache).unwrap();
}
