// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.2.1 Run-time Entities, the IDL PSM: the IDL of the generic
//! DLRL entities (`idl/dds_dlrl.idl`, §8.2.1.2.1) through the IDL
//! frontend, each of its declarations bound to the Rust item that
//! implements it, and the mapping rules of §8.2.1.1.
//!
//! Rust has no reflection: the binding is a table whose entries name the
//! Rust items as expressions and types, so that a Rust item that is
//! missing or renamed fails the build of this test, and the test requires
//! one entry for each declaration of the IDL and none beyond.

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::collections::BTreeSet;
use std::sync::Arc;

use dds::{Bus, Recorder, coherent_pair, delete, enabled, mapping, mono, vehicles, wait_until};
use zerodds_ast::{AnyId, Ast, InterfaceKind, PrimitiveKind, TypeKind, ValuetypeKind};
use zerodds_ast_core::ScopedName;
use zerodds_dlrl::cache::{
    Cache, CacheAccess, CacheBase, CacheDescription, CacheFactory, CacheKind, CacheListener,
    CacheUsage, Collection, Contract, DcpsState, FilterCriterion, IntMap, List, ObjectHome,
    ObjectListener, ObjectRoot, ObjectScope, ObjectState, QueryCriterion, Selection,
    SelectionCriterion, SelectionListener, Set, StrMap, UNLIMITED_RELATED_OBJECTS,
};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::{ObjectRef, Value};
use zerodds_dlrl::types::{
    CacheName, ClassName, CriterionKind, DlrlOid, LongSeq, MembershipState, ReferenceScope,
    RelatedObjectDepth, RelationDescription, RelationKind, RelationName,
};
use zerodds_idl_compose::{PathLoader, compose_source};

/// The model of `dds_dlrl.idl`.
fn model() -> Ast {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/idl");
    let composed = compose_source(
        &format!("{dir}/psm_test.idl"),
        "#include \"dds_dlrl.idl\"\n",
        &PathLoader::bare(),
    );
    assert!(
        composed.diagnostics.is_empty(),
        "dds_dlrl.idl does not compile: {:#?}",
        composed.diagnostics
    );
    composed.ast
}

/// The declarations of `dds_dcps.idl`, which are not DLRL entities.
const DCPS: [&str; 5] = [
    "DDS::StringSeq",
    "DDS::Entity",
    "DDS::DomainParticipant",
    "DDS::Publisher",
    "DDS::Subscriber",
];

fn name_of(ast: &Ast, any: AnyId) -> Option<String> {
    ast.scoped_name_of(any)
        .map(|n| n.segments().join("::"))
        .filter(|n| n.starts_with("DDS::") && !DCPS.contains(&n.as_str()))
}

/// Every declaration of the IDL by scoped name: the types (with the
/// enumerators of an enum, the members of a struct, the state members,
/// attributes and operations of a valuetype), the interfaces with their
/// attributes and operations, the exceptions with their members, the
/// constants.
fn idl_declarations(ast: &Ast) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (id, node) in ast.types() {
        let Some(name) = name_of(ast, AnyId::Type(id, node.node_id)) else {
            continue;
        };
        match &node.kind {
            TypeKind::Enumeration { .. } => {
                for e in ast.enumerators_of(id) {
                    out.insert(format!("{name}::{}", ast.enumerator(*e).unwrap().name));
                }
            }
            TypeKind::Structure { .. } => {
                for m in ast.members_of(id) {
                    out.insert(format!("{name}::{}", ast.member(*m).unwrap().name));
                }
            }
            TypeKind::Valuetype { state, .. } => {
                for m in state {
                    out.insert(format!("{name}::{}", ast.member(*m).unwrap().name));
                }
                for a in ast.attributes_of_value(id) {
                    out.insert(format!("{name}::{}", ast.attribute(a).unwrap().name));
                }
                for o in ast.operations_of_value(id) {
                    out.insert(format!("{name}::{}", ast.operation(o).unwrap().name));
                }
            }
            _ => {}
        }
        out.insert(name);
    }
    for (id, node) in ast.interfaces() {
        let Some(name) = name_of(ast, AnyId::Interface(id, node.node_id)) else {
            continue;
        };
        for a in &node.attributes {
            out.insert(format!("{name}::{}", ast.attribute(*a).unwrap().name));
        }
        for o in &node.operations {
            out.insert(format!("{name}::{}", ast.operation(*o).unwrap().name));
        }
        out.insert(name);
    }
    for (id, node) in ast.exceptions() {
        let Some(name) = name_of(ast, AnyId::Exception(id, node.node_id)) else {
            continue;
        };
        for m in &node.members {
            out.insert(format!("{name}::{}", ast.member(*m).unwrap().name));
        }
        out.insert(name);
    }
    for (id, node) in ast.constants() {
        if let Some(name) = name_of(ast, AnyId::Const(id, node.node_id)) {
            out.insert(name);
        }
    }
    out
}

/// The Rust item of each declaration of `dds_dlrl.idl`. Each entry names
/// its item as an expression or a type that this function uses, so that
/// the build fails when the item is missing.
#[allow(clippy::too_many_lines)]
fn binding() -> Vec<(&'static str, &'static str)> {
    let mut t: Vec<(&'static str, &'static str)> = Vec::new();
    macro_rules! item {
        ($idl:literal, $rust:expr) => {{
            let _ = $rust;
            t.push(($idl, stringify!($rust)));
        }};
    }
    macro_rules! ty {
        ($idl:literal, $rust:ty) => {{
            let _ = core::marker::PhantomData::<$rust>;
            t.push(($idl, stringify!($rust)));
        }};
    }

    ty!("DDS::ReferenceScope", ReferenceScope);
    item!(
        "DDS::ReferenceScope::SIMPLE_CONTENT_SCOPE",
        ReferenceScope::SimpleContent
    );
    item!(
        "DDS::ReferenceScope::REFERENCED_CONTENTS_SCOPE",
        ReferenceScope::ReferencedContents
    );
    ty!("DDS::ObjectScope", ObjectScope);
    item!(
        "DDS::ObjectScope::SIMPLE_OBJECT_SCOPE",
        ObjectScope::SimpleObject
    );
    item!(
        "DDS::ObjectScope::CONTAINED_OBJECTS_SCOPE",
        ObjectScope::ContainedObjects
    );
    item!(
        "DDS::ObjectScope::RELATED_OBJECTS_SCOPE",
        ObjectScope::RelatedObjects
    );
    ty!("DDS::DCPSState", DcpsState);
    item!("DDS::DCPSState::INITIAL", DcpsState::Initial);
    item!("DDS::DCPSState::REGISTERED", DcpsState::Registered);
    item!("DDS::DCPSState::ENABLED", DcpsState::Enabled);
    ty!("DDS::CacheUsage", CacheUsage);
    item!("DDS::CacheUsage::READ_ONLY", CacheUsage::ReadOnly);
    item!("DDS::CacheUsage::WRITE_ONLY", CacheUsage::WriteOnly);
    item!("DDS::CacheUsage::READ_WRITE", CacheUsage::ReadWrite);
    ty!("DDS::ObjectState", ObjectState);
    item!("DDS::ObjectState::OBJECT_VOID", ObjectState::Void);
    item!("DDS::ObjectState::OBJECT_NEW", ObjectState::New);
    item!(
        "DDS::ObjectState::OBJECT_NOT_MODIFIED",
        ObjectState::NotModified
    );
    item!("DDS::ObjectState::OBJECT_MODIFIED", ObjectState::Modified);
    item!("DDS::ObjectState::OBJECT_DELETED", ObjectState::Deleted);
    ty!("DDS::DLRLOid", DlrlOid);
    item!("DDS::DLRLOid::value", |o: DlrlOid| -> [i32; 3] { o.value });
    ty!("DDS::LongSeq", LongSeq);
    ty!("DDS::ClassName", ClassName);
    ty!("DDS::CacheName", CacheName);
    ty!("DDS::RelationName", RelationName);

    // The exceptions: the variants of DlrlException, each with message.
    macro_rules! exception {
        ($idl:literal, $member:literal, $variant:ident) => {{
            item!(
                $idl,
                DlrlException::$variant {
                    message: String::new()
                }
            );
            item!($member, DlrlException::message);
        }};
    }
    exception!("DDS::DCPSError", "DDS::DCPSError::message", DcpsError);
    exception!(
        "DDS::BadHomeDefinition",
        "DDS::BadHomeDefinition::message",
        BadHomeDefinition
    );
    exception!("DDS::NotFound", "DDS::NotFound::message", NotFound);
    exception!(
        "DDS::AlreadyExisting",
        "DDS::AlreadyExisting::message",
        AlreadyExisting
    );
    exception!(
        "DDS::AlreadyDeleted",
        "DDS::AlreadyDeleted::message",
        AlreadyDeleted
    );
    exception!(
        "DDS::PreconditionNotMet",
        "DDS::PreconditionNotMet::message",
        PreconditionNotMet
    );
    exception!(
        "DDS::NoSuchElement",
        "DDS::NoSuchElement::message",
        NoSuchElement
    );
    exception!("DDS::SQLError", "DDS::SQLError::message", SqlError);
    exception!(
        "DDS::ReadOnlyMode",
        "DDS::ReadOnlyMode::message",
        ReadOnlyMode
    );
    exception!(
        "DDS::ExpiredTimeOut",
        "DDS::ExpiredTimeOut::message",
        ExpiredTimeOut
    );
    exception!(
        "DDS::BadParameter",
        "DDS::BadParameter::message",
        BadParameter
    );

    // The sequences.
    ty!("DDS::ObjectRootSeq", Vec<ObjectRoot>);
    ty!("DDS::ObjectHomeSeq", Vec<ObjectHome>);
    ty!("DDS::ObjectListenerSeq", Vec<Arc<dyn ObjectListener>>);
    ty!("DDS::SelectionSeq", Vec<Selection>);
    ty!("DDS::CacheBaseSeq", Vec<CacheBase>);
    ty!("DDS::CacheAccessSeq", Vec<CacheAccess>);
    ty!("DDS::CacheListenerSeq", Vec<Arc<dyn CacheListener>>);
    ty!("DDS::ContractSeq", Vec<Contract>);
    ty!("DDS::RelationDescriptionSeq", Vec<RelationDescription>);

    // The listeners: their typed operations are generated (FooListener,
    // FooSelectionListener); the traits carry them for every class.
    ty!("DDS::ObjectListener", dyn ObjectListener);
    ty!("DDS::SelectionListener", dyn SelectionListener);
    ty!("DDS::CacheListener", dyn CacheListener);
    item!(
        "DDS::CacheListener::on_begin_updates",
        <dyn CacheListener as CacheListener>::on_begin_updates
    );
    item!(
        "DDS::CacheListener::on_end_updates",
        <dyn CacheListener as CacheListener>::on_end_updates
    );
    item!(
        "DDS::CacheListener::on_updates_enabled",
        <dyn CacheListener as CacheListener>::on_updates_enabled
    );
    item!(
        "DDS::CacheListener::on_updates_disabled",
        <dyn CacheListener as CacheListener>::on_updates_disabled
    );

    ty!("DDS::Contract", Contract);
    item!("DDS::Contract::depth", Contract::depth);
    item!("DDS::Contract::scope", Contract::scope);
    item!(
        "DDS::Contract::contracted_object",
        Contract::contracted_object
    );
    item!("DDS::Contract::set_depth", Contract::set_depth);
    item!("DDS::Contract::set_scope", Contract::set_scope);

    ty!("DDS::RelationKind", RelationKind);
    item!("DDS::RelationKind::REF_RELATION", RelationKind::RefRelation);
    item!(
        "DDS::RelationKind::LIST_RELATION",
        RelationKind::ListRelation
    );
    item!(
        "DDS::RelationKind::INT_MAP_RELATION",
        RelationKind::IntMapRelation
    );
    item!(
        "DDS::RelationKind::STR_MAP_RELATION",
        RelationKind::StrMapRelation
    );
    ty!("DDS::RelationDescription", RelationDescription);
    item!("DDS::RelationDescription::kind", RelationDescription::kind);
    item!("DDS::RelationDescription::name", RelationDescription::name);
    item!(
        "DDS::ListRelationDescription",
        |name: String, index: i32| RelationDescription::List { name, index }
    );
    item!(
        "DDS::ListRelationDescription::index",
        |d: &RelationDescription| match d {
            RelationDescription::List { index, .. } => Some(*index),
            _ => None,
        }
    );
    item!(
        "DDS::IntMapRelationDescription",
        |name: String, key: i32| RelationDescription::IntMap { name, key }
    );
    item!(
        "DDS::IntMapRelationDescription::key",
        |d: &RelationDescription| match d {
            RelationDescription::IntMap { key, .. } => Some(*key),
            _ => None,
        }
    );
    item!(
        "DDS::StrMapRelationDescription",
        |name: String, key: String| RelationDescription::StrMap { name, key }
    );
    item!(
        "DDS::StrMapRelationDescription::key",
        |d: &RelationDescription| match d {
            RelationDescription::StrMap { key, .. } => Some(key.clone()),
            _ => None,
        }
    );
    ty!("DDS::RelatedObjectDepth", RelatedObjectDepth);
    item!("DDS::UNLIMITED_RELATED_OBJECTS", UNLIMITED_RELATED_OBJECTS);

    ty!("DDS::ObjectRoot", ObjectRoot);
    // The private state is behind the attributes oid and class_name.
    item!("DDS::ObjectRoot::m_oid", ObjectRoot::oid);
    item!("DDS::ObjectRoot::m_class_name", ObjectRoot::class_name);
    item!("DDS::ObjectRoot::oid", ObjectRoot::oid);
    item!("DDS::ObjectRoot::read_state", ObjectRoot::read_state);
    item!("DDS::ObjectRoot::write_state", ObjectRoot::write_state);
    item!("DDS::ObjectRoot::object_home", ObjectRoot::object_home);
    item!("DDS::ObjectRoot::class_name", ObjectRoot::class_name);
    item!("DDS::ObjectRoot::owner", ObjectRoot::owner);
    item!("DDS::ObjectRoot::destroy", ObjectRoot::destroy);
    item!("DDS::ObjectRoot::is_modified", ObjectRoot::is_modified);
    item!(
        "DDS::ObjectRoot::which_contained_modified",
        ObjectRoot::which_contained_modified
    );

    ty!("DDS::CriterionKind", CriterionKind);
    item!("DDS::CriterionKind::QUERY", CriterionKind::Query);
    item!("DDS::CriterionKind::FILTER", CriterionKind::Filter);
    ty!("DDS::SelectionCriterion", SelectionCriterion);
    item!("DDS::SelectionCriterion::kind", SelectionCriterion::kind);
    ty!("DDS::MembershipState", MembershipState);
    item!(
        "DDS::MembershipState::UNDEFINED_MEMBERSHIP",
        MembershipState::UndefinedMembership
    );
    item!(
        "DDS::MembershipState::ALREADY_MEMBER",
        MembershipState::AlreadyMember
    );
    item!(
        "DDS::MembershipState::NOT_MEMBER",
        MembershipState::NotMember
    );
    ty!("DDS::FilterCriterion", dyn FilterCriterion);
    ty!("DDS::QueryCriterion", QueryCriterion);
    item!(
        "DDS::QueryCriterion::expression",
        QueryCriterion::expression
    );
    item!(
        "DDS::QueryCriterion::parameters",
        QueryCriterion::parameters
    );
    item!("DDS::QueryCriterion::set_query", QueryCriterion::set_query);
    item!(
        "DDS::QueryCriterion::set_parameters",
        QueryCriterion::set_parameters
    );

    ty!("DDS::Selection", Selection);
    item!("DDS::Selection::auto_refresh", Selection::auto_refresh);
    item!(
        "DDS::Selection::concerns_contained",
        Selection::concerns_contained
    );
    item!("DDS::Selection::refresh", Selection::refresh);

    ty!("DDS::ObjectHome", ObjectHome);
    item!("DDS::ObjectHome::name", ObjectHome::class_name);
    item!(
        "DDS::ObjectHome::content_filter",
        ObjectHome::content_filter
    );
    item!("DDS::ObjectHome::parent", ObjectHome::parent);
    item!("DDS::ObjectHome::children", ObjectHome::children);
    item!(
        "DDS::ObjectHome::registration_index",
        ObjectHome::registration_index
    );
    item!("DDS::ObjectHome::auto_deref", ObjectHome::auto_deref);
    item!(
        "DDS::ObjectHome::set_content_filter",
        ObjectHome::set_content_filter
    );
    item!(
        "DDS::ObjectHome::set_auto_deref",
        ObjectHome::set_auto_deref
    );
    item!("DDS::ObjectHome::deref_all", ObjectHome::deref_all);
    item!("DDS::ObjectHome::underef_all", ObjectHome::underef_all);
    item!(
        "DDS::ObjectHome::get_topic_name",
        ObjectHome::get_topic_name
    );
    item!(
        "DDS::ObjectHome::get_all_topic_names",
        ObjectHome::get_all_topic_names
    );

    ty!("DDS::Collection", dyn Collection);
    item!(
        "DDS::Collection::length",
        <dyn Collection as Collection>::length
    );
    ty!("DDS::List", List);
    item!("DDS::List::remove", List::remove);
    item!("DDS::List::added_elements", List::added_elements);
    item!("DDS::List::removed_elements", List::removed_elements);
    item!("DDS::List::modified_elements", List::modified_elements);
    ty!("DDS::Set", Set);
    ty!("DDS::StrMap", StrMap);
    item!("DDS::StrMap::keys", StrMap::keys);
    item!("DDS::StrMap::remove", StrMap::remove);
    item!("DDS::StrMap::added_elements", StrMap::added_elements);
    item!("DDS::StrMap::removed_elements", StrMap::removed_elements);
    item!("DDS::StrMap::modified_elements", StrMap::modified_elements);
    ty!("DDS::IntMap", IntMap);
    item!("DDS::IntMap::keys", IntMap::keys);
    item!("DDS::IntMap::remove", IntMap::remove);
    item!("DDS::IntMap::added_elements", IntMap::added_elements);
    item!("DDS::IntMap::removed_elements", IntMap::removed_elements);
    item!("DDS::IntMap::modified_elements", IntMap::modified_elements);

    ty!("DDS::CacheKind", CacheKind);
    item!("DDS::CacheKind::CACHE_KIND", CacheKind::Cache);
    item!("DDS::CacheKind::CACHEACCESS_KIND", CacheKind::CacheAccess);
    ty!("DDS::CacheBase", CacheBase);
    item!("DDS::CacheBase::cache_usage", CacheBase::cache_usage);
    item!("DDS::CacheBase::objects", CacheBase::objects);
    item!("DDS::CacheBase::kind", CacheBase::kind);
    item!("DDS::CacheBase::refresh", CacheBase::refresh);

    ty!("DDS::CacheAccess", CacheAccess);
    item!("DDS::CacheAccess::owner", CacheAccess::owner);
    item!("DDS::CacheAccess::contracts", CacheAccess::contracts);
    item!("DDS::CacheAccess::type_names", CacheAccess::type_names);
    item!("DDS::CacheAccess::write", CacheAccess::write);
    item!("DDS::CacheAccess::purge", CacheAccess::purge);
    item!(
        "DDS::CacheAccess::create_contract",
        CacheAccess::create_contract
    );
    item!(
        "DDS::CacheAccess::delete_contract",
        CacheAccess::delete_contract
    );

    ty!("DDS::Cache", Cache);
    item!("DDS::Cache::pubsub_state", Cache::pubsub_state);
    item!("DDS::Cache::the_publisher", Cache::the_publisher);
    item!("DDS::Cache::the_subscriber", Cache::the_subscriber);
    item!("DDS::Cache::updates_enabled", Cache::updates_enabled);
    item!("DDS::Cache::homes", Cache::homes);
    item!("DDS::Cache::sub_accesses", Cache::sub_accesses);
    item!("DDS::Cache::listeners", Cache::listeners);
    item!(
        "DDS::Cache::register_all_for_pubsub",
        Cache::register_all_for_pubsub
    );
    item!(
        "DDS::Cache::enable_all_for_pubsub",
        Cache::enable_all_for_pubsub
    );
    item!("DDS::Cache::register_home", Cache::register_home);
    item!("DDS::Cache::find_home_by_name", Cache::find_home_by_name);
    item!("DDS::Cache::find_home_by_index", Cache::find_home_by_index);
    item!("DDS::Cache::attach_listener", Cache::attach_listener);
    item!("DDS::Cache::detach_listener", Cache::detach_listener);
    item!("DDS::Cache::enable_updates", Cache::enable_updates);
    item!("DDS::Cache::disable_updates", Cache::disable_updates);
    item!("DDS::Cache::load", Cache::load);
    item!("DDS::Cache::create_access", Cache::create_access);
    item!("DDS::Cache::delete_access", Cache::delete_access);
    item!("DDS::Cache::lock", Cache::lock);
    item!("DDS::Cache::unlock", Cache::unlock);

    ty!("DDS::CacheDescription", CacheDescription);
    item!("DDS::CacheDescription::name", |d: &CacheDescription| d
        .name
        .clone());
    item!("DDS::CacheDescription::domain", |d: &CacheDescription| d
        .domain
        .clone());
    ty!("DDS::CacheFactory", CacheFactory);
    item!(
        "DDS::CacheFactory::create_cache",
        CacheFactory::create_cache
    );
    item!(
        "DDS::CacheFactory::find_cache_by_name",
        CacheFactory::find_cache_by_name
    );
    item!(
        "DDS::CacheFactory::delete_cache",
        CacheFactory::delete_cache
    );
    t
}

/// §8.2.1.2.1: the IDL of the generic DLRL entities compiles, with the
/// DCPS declarations it includes, and its declarations are those of the
/// specification (with the changes its header names).
#[test]
fn dlrl_8_2_1_2_1_generic_idl_parses() {
    let ast = model();
    let declarations = idl_declarations(&ast);
    for name in [
        "DDS::Cache::lock",
        "DDS::ObjectHome::deref_all",
        "DDS::ObjectRoot::which_contained_modified",
        "DDS::Contract::contracted_object",
        "DDS::ReadOnlyMode",
        "DDS::UNLIMITED_RELATED_OBJECTS",
    ] {
        assert!(declarations.contains(name), "{name}");
    }
    // The listeners' typed operations are not in the base interfaces
    // (change 3 of the file).
    assert!(!declarations.contains("DDS::ObjectListener::on_object_created"));
    assert!(!declarations.contains("DDS::SelectionListener::on_object_out"));
}

/// §8.2.1.2.1: each declaration of the IDL has its Rust item in the
/// table of [`binding`], and the table names nothing the IDL does not
/// declare.
#[test]
fn dlrl_8_2_1_2_1_every_idl_declaration_has_its_rust_item() {
    let ast = model();
    let idl = idl_declarations(&ast);
    let table = binding();
    let bound: BTreeSet<String> = table.iter().map(|(i, _)| (*i).to_string()).collect();
    assert_eq!(bound.len(), table.len(), "a declaration bound twice");
    let missing: Vec<&String> = idl.difference(&bound).collect();
    let beyond: Vec<&String> = bound.difference(&idl).collect();
    assert!(
        missing.is_empty(),
        "IDL declarations without Rust item: {missing:?}"
    );
    assert!(
        beyond.is_empty(),
        "table entries the IDL does not declare: {beyond:?}"
    );
}

fn interface_kind(ast: &Ast, name: &str) -> Option<InterfaceKind> {
    let path = ScopedName::absolute(name.split("::").map(ToString::to_string).collect());
    match ast.find_by_scoped_name(&path) {
        Ok(AnyId::Interface(id, _)) => Some(ast.interface(id).unwrap().kind),
        _ => None,
    }
}

fn valuetype_kind(ast: &Ast, name: &str) -> Option<ValuetypeKind> {
    let path = ScopedName::absolute(name.split("::").map(ToString::to_string).collect());
    match ast.find_by_scoped_name(&path) {
        Ok(AnyId::Type(id, _)) => match &ast.type_(id).unwrap().kind {
            TypeKind::Valuetype { kind, .. } => Some(*kind),
            _ => None,
        },
        _ => None,
    }
}

/// §8.2.1.1: the entities that are access points to servicing objects
/// are local interfaces, the application objects and their parts are
/// valuetypes; the oid's values have the native place-holder type
/// (`#define DLRL_OID_TYPE_NATIVE long`), three of them.
#[test]
fn dlrl_8_2_1_1_access_points_are_local_interfaces_and_objects_are_valuetypes() {
    let ast = model();
    for name in [
        "CacheFactory",
        "CacheBase",
        "Cache",
        "CacheAccess",
        "CacheListener",
        "Contract",
        "ObjectHome",
        "ObjectListener",
        "Selection",
        "SelectionCriterion",
        "FilterCriterion",
        "QueryCriterion",
        "SelectionListener",
    ] {
        assert_eq!(
            interface_kind(&ast, &format!("DDS::{name}")),
            Some(InterfaceKind::Local),
            "{name}"
        );
    }
    for (name, kind) in [
        ("ObjectRoot", ValuetypeKind::Regular),
        ("Collection", ValuetypeKind::Abstract),
        ("List", ValuetypeKind::Abstract),
        ("Set", ValuetypeKind::Regular),
        ("StrMap", ValuetypeKind::Abstract),
        ("IntMap", ValuetypeKind::Abstract),
        ("RelationDescription", ValuetypeKind::Regular),
        ("ListRelationDescription", ValuetypeKind::Regular),
        ("IntMapRelationDescription", ValuetypeKind::Regular),
        ("StrMapRelationDescription", ValuetypeKind::Regular),
    ] {
        assert_eq!(
            valuetype_kind(&ast, &format!("DDS::{name}")),
            Some(kind),
            "{name}"
        );
    }
    // CacheDescription is a struct (change 8 of the file): a valuetype
    // cannot hold the local DomainParticipant.
    assert_eq!(valuetype_kind(&ast, "DDS::CacheDescription"), None);
    // struct DLRLOid { DLRL_OID_TYPE_NATIVE value[3]; }
    let path = ScopedName::absolute(vec!["DDS".into(), "DLRLOid".into()]);
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path) else {
        panic!("DDS::DLRLOid");
    };
    let members = ast.members_of(id);
    assert_eq!(members.len(), 1);
    let value = ast.member(members[0]).unwrap();
    assert_eq!(value.name, "value");
    let ty = ast.type_(value.type_.resolved().unwrap()).unwrap();
    let TypeKind::Array { element, dims } = &ty.kind else {
        panic!("value is no array: {:?}", ty.kind);
    };
    assert_eq!(dims.len(), 1);
    assert_eq!(
        ast.eval_const(dims[0]),
        Ok(zerodds_ast::ConstValue::Integer(3))
    );
    let element = ast.type_(element.resolved().unwrap()).unwrap();
    assert_eq!(element.kind, TypeKind::Primitive(PrimitiveKind::Int32));
    let rust: [i32; 3] = DlrlOid::new(1, 2, 3).value;
    assert_eq!(rust, [1, 2, 3]);
}

/// §8.2.1.1: `get_instance` of CacheFactory is a static operation of the
/// language and not in the IDL; it gives the same factory at each call.
#[test]
fn dlrl_8_2_1_1_get_instance_is_not_in_the_idl_and_exists_in_rust() {
    let ast = model();
    let declarations = idl_declarations(&ast);
    assert!(declarations.contains("DDS::CacheFactory::create_cache"));
    assert!(!declarations.contains("DDS::CacheFactory::get_instance"));
    let a: *const CacheFactory = CacheFactory::get_instance();
    let b: *const CacheFactory = CacheFactory::get_instance();
    assert_eq!(a, b);
}

/// The return type of the operation `op` that the valuetype `name` of
/// the IDL declares; `None` for `void`.
fn value_operation_return(ast: &Ast, name: &str, op: &str) -> Option<TypeKind> {
    let path = ScopedName::absolute(name.split("::").map(ToString::to_string).collect());
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path) else {
        panic!("{name} is no type");
    };
    let o = ast
        .operations_of_value(id)
        .into_iter()
        .map(|o| ast.operation(o).unwrap())
        .find(|o| o.name == op)
        .unwrap_or_else(|| panic!("{name} has no operation {op}"));
    o.return_type
        .as_ref()
        .map(|t| ast.type_(t.resolved().unwrap()).unwrap().kind.clone())
}

/// The operations a valuetype of the IDL declares itself.
fn value_operations(ast: &Ast, name: &str) -> BTreeSet<String> {
    let path = ScopedName::absolute(name.split("::").map(ToString::to_string).collect());
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path) else {
        panic!("{name} is no type");
    };
    ast.operations_of_value(id)
        .into_iter()
        .map(|o| ast.operation(o).unwrap().name.clone())
        .collect()
}

/// §8.2.1.1: an error that affects the later behaviour is an exception
/// (a `get` of an element that is not there, `NoSuchElement`); a warning
/// that does not is a return value: the `remove` of each collection
/// returns FALSE when there is nothing to remove, and changes nothing.
/// The IDL declares these `remove` operations `boolean` (change 9 of the
/// file; the specification prints `void`, where the warning has no
/// place).
#[test]
fn dlrl_8_2_1_1_a_warning_is_a_return_value() {
    let ast = model();
    for name in ["DDS::List", "DDS::StrMap", "DDS::IntMap"] {
        assert_eq!(
            value_operation_return(&ast, name, "remove"),
            Some(TypeKind::Primitive(PrimitiveKind::Boolean)),
            "{name}::remove"
        );
    }
    let bus = Bus::new();
    let m = mapping();
    let side = enabled(
        &bus,
        CacheUsage::WriteOnly,
        &m,
        &["Radar", "Track", "Part", "Sensor"],
    );
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let sensor = side.home("Sensor").create_object(&access).unwrap();
    let parts = sensor.get_list("parts").unwrap();
    assert!(!parts.remove().unwrap(), "List::remove on an empty list");
    assert!(matches!(
        parts.get(0),
        Err(DlrlException::NoSuchElement { .. })
    ));
    let track = side.home("Track").create_object(&access).unwrap();
    let comments = track.get_str_map("comments").unwrap();
    comments.put("a", Value::String("x".into())).unwrap();
    assert!(!comments.remove("b").unwrap());
    assert_eq!(comments.keys(), ["a".to_string()]);
    assert!(comments.remove("a").unwrap());
    assert!(comments.keys().is_empty());
    assert!(matches!(
        comments.get("a"),
        Err(DlrlException::NoSuchElement { .. })
    ));
    delete(side);

    let v = vehicles();
    let side = enabled(&bus, CacheUsage::WriteOnly, &v, &["Vehicle", "Car"]);
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let car = side.home("Car").create_object(&access).unwrap();
    let tags = car.get_set("tags").unwrap();
    tags.add(Value::String("t".into())).unwrap();
    assert!(!tags.remove(&Value::String("u".into())).unwrap());
    assert_eq!(tags.length(), 1);
    let readings = car.get_int_map("readings").unwrap();
    assert!(!readings.remove(4).unwrap());
    readings.put(4, Value::Double(1.0)).unwrap();
    assert!(readings.remove(4).unwrap());
    delete(side);
}

/// Notes `$op` in `$called` and requires `AlreadyDeleted` from `$call`.
macro_rules! deleted {
    ($called:ident, $op:literal, $call:expr) => {{
        $called.insert($op.to_string());
        match $call {
            Err(DlrlException::AlreadyDeleted { message }) => {
                assert!(!message.is_empty(), "{}", $op);
            }
            other => panic!("{}: {:?}", $op, other.map(|_| ())),
        }
    }};
}

/// Requires the exception `$variant` from `$call`.
macro_rules! raises {
    ($variant:ident, $what:expr, $call:expr) => {{
        match $call {
            Err(DlrlException::$variant { .. }) => {}
            other => panic!("{}: {:?}", $what, other.map(|_| ())),
        }
    }};
}

/// §8.2.1.1 with §8.1.6.3.4: a `remove` that finds nothing to remove
/// gives the warning FALSE only on an object that may be changed. On an
/// object of the Cache, which is not modifiable, it raises
/// `PreconditionNotMet`, on a destroyed object `AlreadyDeleted`, before
/// it looks at the elements; so does `Set::add` of an element already in
/// the set.
#[test]
#[allow(clippy::too_many_lines)]
fn dlrl_8_2_1_1_remove_checks_the_object_before_the_warning() {
    let bus = Bus::new();
    let m = mapping();
    let classes = ["Radar", "Track", "Part", "Sensor"];
    let (sub, publ) = coherent_pair(&bus, &m, &classes);
    let writer = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let track = publ.home("Track").create_object(&writer).unwrap();
    track.set("x", mono(Value::Double(1.0))).unwrap();
    track.set("y", mono(Value::Double(2.0))).unwrap();
    track.set("a_radar", mono(Value::Ref(None))).unwrap();
    let sensor = publ.home("Sensor").create_object(&writer).unwrap();
    sensor
        .set("label", mono(Value::String("s".into())))
        .unwrap();
    writer.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the track and the sensor", || {
        sub.home("Track").find_object(track.oid(), &base).is_ok()
            && sub.home("Sensor").find_object(sensor.oid(), &base).is_ok()
    });
    let p_track = sub.home("Track").find_object(track.oid(), &base).unwrap();
    let p_sensor = sub.home("Sensor").find_object(sensor.oid(), &base).unwrap();
    let p_parts = p_sensor.get_list("parts").unwrap();
    assert_eq!(p_parts.length(), 0);
    raises!(
        PreconditionNotMet,
        "List::remove of an empty list of the Cache",
        p_parts.remove()
    );
    raises!(
        PreconditionNotMet,
        "StrMap::remove of an absent key of the Cache",
        p_track.get_str_map("comments").unwrap().remove("z")
    );
    let comments = track.get_str_map("comments").unwrap();
    let parts = sensor.get_list("parts").unwrap();
    assert!(!comments.remove("z").unwrap(), "writable: the warning");
    assert!(!parts.remove().unwrap(), "writable: the warning");
    track.destroy().unwrap();
    sensor.destroy().unwrap();
    raises!(AlreadyDeleted, "List::remove, destroyed", parts.remove());
    raises!(
        AlreadyDeleted,
        "StrMap::remove, destroyed",
        comments.remove("z")
    );
    delete(sub);
    delete(publ);

    let v = vehicles();
    let (sub, publ) = coherent_pair(&bus, &v, &["Vehicle", "Car"]);
    let writer = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let car = publ.home("Car").create_object(&writer).unwrap();
    car.set("name", mono(Value::String("c".into()))).unwrap();
    car.set("seats", mono(Value::Long(4))).unwrap();
    car.get_set("tags")
        .unwrap()
        .add(Value::String("t".into()))
        .unwrap();
    writer.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the car", || {
        sub.home("Car").find_object(car.oid(), &base).is_ok()
    });
    let p_car = sub.home("Car").find_object(car.oid(), &base).unwrap();
    let p_tags = p_car.get_set("tags").unwrap();
    assert!(p_tags.contains(&Value::String("t".into())).unwrap());
    raises!(
        PreconditionNotMet,
        "Set::remove of an absent element of the Cache",
        p_tags.remove(&Value::String("u".into()))
    );
    raises!(
        PreconditionNotMet,
        "Set::add of an element in the set of the Cache",
        p_tags.add(Value::String("t".into()))
    );
    raises!(
        PreconditionNotMet,
        "IntMap::remove of an absent key of the Cache",
        p_car.get_int_map("readings").unwrap().remove(4)
    );
    let tags = car.get_set("tags").unwrap();
    let readings = car.get_int_map("readings").unwrap();
    car.destroy().unwrap();
    raises!(
        AlreadyDeleted,
        "Set::remove, destroyed",
        tags.remove(&Value::String("u".into()))
    );
    raises!(
        AlreadyDeleted,
        "Set::add of an element in the set, destroyed",
        tags.add(Value::String("t".into()))
    );
    raises!(
        AlreadyDeleted,
        "IntMap::remove, destroyed",
        readings.remove(4)
    );
    delete(sub);
    delete(publ);
}

/// §8.1.6.2, §8.2.1.1: an ObjectRoot destroyed in a CacheAccess
/// (write_state DELETED) raises `AlreadyDeleted` from every operation:
/// those of the IDL of ObjectRoot, the generated `get_<attribute>`,
/// `set_<attribute>` and `is_<attribute>_modified`, the navigation, and
/// every operation of its collections, through handles taken before the
/// destruction; before and after the `write` that publishes the
/// destruction. Its attributes and those of its collections (`length`,
/// `values`, `keys`) keep their last value.
#[test]
#[allow(clippy::too_many_lines)]
fn dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_destroyed_object() {
    let ast = model();
    let bus = Bus::new();
    let m = mapping();
    let side = enabled(
        &bus,
        CacheUsage::WriteOnly,
        &m,
        &["Radar", "Track", "Part", "Sensor"],
    );
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = side.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(Value::String("r".into()))).unwrap();
    let track = side.home("Track").create_object(&access).unwrap();
    track.set("x", mono(Value::Double(1.0))).unwrap();
    track.set("y", mono(Value::Double(2.0))).unwrap();
    track.set_reference("a_radar", Some(&radar)).unwrap();
    let comments = track.get_str_map("comments").unwrap();
    comments.put("a", Value::String("c".into())).unwrap();
    let sensor = side.home("Sensor").create_object(&access).unwrap();
    sensor
        .set("label", mono(Value::String("s".into())))
        .unwrap();
    let part = side.home("Part").create_object(&access).unwrap();
    part.set("serial", mono(Value::Long(1))).unwrap();
    let parts = sensor.get_list("parts").unwrap();
    parts.add_object(&part).unwrap();
    access.write().unwrap();
    let oid = track.oid();

    track.destroy().unwrap();
    sensor.destroy().unwrap();
    assert_eq!(
        part.write_state(),
        ObjectState::Deleted,
        "the contained part is destroyed with the sensor"
    );
    let mut object = BTreeSet::new();
    deleted!(object, "destroy", track.destroy());
    deleted!(
        object,
        "is_modified",
        track.is_modified(ObjectScope::SimpleObject)
    );
    deleted!(
        object,
        "is_modified(CONTAINED_OBJECTS_SCOPE)",
        sensor.is_modified(ObjectScope::ContainedObjects)
    );
    deleted!(
        object,
        "is_modified(RELATED_OBJECTS_SCOPE)",
        track.is_modified(ObjectScope::RelatedObjects)
    );
    deleted!(
        object,
        "which_contained_modified",
        sensor.which_contained_modified()
    );
    deleted!(object, "get_x", track.get("x"));
    deleted!(object, "set_x", track.set("x", mono(Value::Double(3.0))));
    deleted!(object, "is_x_modified", track.is_attribute_modified("x"));
    deleted!(object, "get_a_radar", track.get_reference("a_radar"));
    deleted!(
        object,
        "set_a_radar",
        track.set_reference("a_radar", Some(&radar))
    );
    deleted!(object, "get_parts", sensor.get_references("parts"));
    deleted!(object, "get_parts as List", sensor.get_list("parts"));
    deleted!(
        object,
        "get_comments as StrMap",
        track.get_str_map("comments")
    );
    assert!(
        value_operations(&ast, "DDS::ObjectRoot").is_subset(&object),
        "{object:?}"
    );
    assert_eq!(track.oid(), oid);
    assert_eq!(track.class_name(), "Track");
    assert_eq!(track.write_state(), ObjectState::Deleted);
    assert!(matches!(track.owner(), Some(CacheBase::Access(_))));

    // List of the destroyed sensor, taken before the destruction.
    let mut list = BTreeSet::new();
    deleted!(list, "remove", parts.remove());
    deleted!(list, "added_elements", parts.added_elements());
    deleted!(list, "removed_elements", parts.removed_elements());
    deleted!(list, "modified_elements", parts.modified_elements());
    deleted!(list, "add", parts.add(Value::Ref(None)));
    deleted!(list, "put", parts.put(0, Value::Ref(None)));
    deleted!(list, "get", parts.get(0));
    deleted!(list, "add_object", parts.add_object(&radar));
    deleted!(list, "put_object", parts.put_object(0, &radar));
    deleted!(list, "get_object", parts.get_object(0));
    assert!(
        value_operations(&ast, "DDS::List").is_subset(&list),
        "{list:?}"
    );
    assert_eq!(parts.length(), 1);
    assert_eq!(parts.values().len(), 1);

    // StrMap of the destroyed track.
    let mut str_map = BTreeSet::new();
    deleted!(str_map, "remove", comments.remove("a"));
    deleted!(str_map, "added_elements", comments.added_elements());
    deleted!(str_map, "removed_elements", comments.removed_elements());
    deleted!(str_map, "modified_elements", comments.modified_elements());
    deleted!(str_map, "put", comments.put("b", Value::String("d".into())));
    deleted!(str_map, "get", comments.get("a"));
    deleted!(str_map, "put_object", comments.put_object("b", &radar));
    deleted!(str_map, "get_object", comments.get_object("a"));
    assert!(
        value_operations(&ast, "DDS::StrMap").is_subset(&str_map),
        "{str_map:?}"
    );
    assert_eq!(comments.length(), 1);
    assert_eq!(comments.keys(), ["a".to_string()]);
    assert_eq!(comments.values(), [Value::String("c".into())]);

    // After the write that publishes the destruction.
    access.write().unwrap();
    assert!(!access.objects().contains(&track));
    let mut after = BTreeSet::new();
    deleted!(after, "get_x", track.get("x"));
    deleted!(
        after,
        "is_modified",
        track.is_modified(ObjectScope::SimpleObject)
    );
    deleted!(
        after,
        "which_contained_modified",
        sensor.which_contained_modified()
    );
    deleted!(after, "StrMap::get", comments.get("a"));
    deleted!(after, "List::get", parts.get(0));
    delete(side);

    // Set and IntMap of a destroyed object.
    let v = vehicles();
    let side = enabled(&bus, CacheUsage::WriteOnly, &v, &["Vehicle", "Car"]);
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let car = side.home("Car").create_object(&access).unwrap();
    let other = side.home("Car").create_object(&access).unwrap();
    car.set("name", mono(Value::String("c".into()))).unwrap();
    car.set("seats", mono(Value::Long(4))).unwrap();
    let tags = car.get_set("tags").unwrap();
    tags.add(Value::String("t".into())).unwrap();
    let readings = car.get_int_map("readings").unwrap();
    readings.put(1, Value::Double(1.5)).unwrap();
    car.destroy().unwrap();
    deleted!(object, "get_tags as Set", car.get_set("tags"));
    deleted!(
        object,
        "get_readings as IntMap",
        car.get_int_map("readings")
    );
    let mut set = BTreeSet::new();
    let t = Value::String("t".into());
    deleted!(set, "contains", tags.contains(&t));
    deleted!(set, "add", tags.add(Value::String("u".into())));
    deleted!(set, "remove", tags.remove(&t));
    deleted!(set, "added_elements", tags.added_elements());
    deleted!(set, "removed_elements", tags.removed_elements());
    deleted!(set, "contains_object", tags.contains_object(&other));
    deleted!(set, "add_object", tags.add_object(&other));
    deleted!(set, "remove_object", tags.remove_object(&other));
    deleted!(set, "objects", tags.objects());
    assert!(
        value_operations(&ast, "DDS::Set").is_subset(&set),
        "{set:?}"
    );
    assert_eq!(tags.length(), 1);
    let mut int_map = BTreeSet::new();
    deleted!(int_map, "remove", readings.remove(1));
    deleted!(int_map, "added_elements", readings.added_elements());
    deleted!(int_map, "removed_elements", readings.removed_elements());
    deleted!(int_map, "modified_elements", readings.modified_elements());
    deleted!(int_map, "put", readings.put(2, Value::Double(2.0)));
    deleted!(int_map, "get", readings.get(1));
    deleted!(int_map, "put_object", readings.put_object(2, &other));
    deleted!(int_map, "get_object", readings.get_object(1));
    assert!(
        value_operations(&ast, "DDS::IntMap").is_subset(&int_map),
        "{int_map:?}"
    );
    assert_eq!(readings.keys(), [1]);
    assert_eq!(readings.length(), 1);
    delete(side);
}

/// The operations an interface of the IDL has, those of its bases
/// included (IDL 4.2 §7.4.3).
fn operations_of(ast: &Ast, name: &str) -> BTreeSet<String> {
    let path = ScopedName::absolute(name.split("::").map(ToString::to_string).collect());
    let Ok(AnyId::Interface(id, _)) = ast.find_by_scoped_name(&path) else {
        panic!("{name} is no interface");
    };
    ast.all_operations_of(id)
        .into_iter()
        .map(|o| ast.operation(o).unwrap().name.clone())
        .collect()
}

/// §8.1.6.2, §8.2.1.1: an operation invoked on an entity that has been
/// deleted raises `AlreadyDeleted`, which the IDL does not name in the
/// raises clauses. For each entity with a delete operation (Cache by
/// `delete_cache`, CacheAccess by `delete_access` and with its Cache,
/// Contract by `delete_contract` and `purge`, Selection by
/// `delete_selection`, its QueryCriterion with it) every operation the
/// IDL gives the interface is called, and the operations called are
/// exactly those of the IDL. The delete operations raise it for an entity
/// deleted already. A destroyed ObjectRoot raises it from `destroy` and
/// `set`, also after the `write` that published the destruction and
/// after a `purge` (every operation of a destroyed object: the next
/// test); an object of the Cache deleted by another participant is not
/// destroyed and reads its last state. The attributes keep their last
/// value.
#[test]
#[allow(clippy::too_many_lines)]
fn dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_deleted_entity() {
    let ast = model();
    let bus = Bus::new();
    let m = mapping();
    let classes = ["Radar", "Track", "Part", "Sensor"];
    let (sub, publ) = coherent_pair(&bus, &m, &classes);
    let writer = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&writer).unwrap();
    radar.set("name", mono(Value::String("r".into()))).unwrap();
    writer.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the radar", || {
        sub.home("Radar").find_object(radar.oid(), &base).is_ok()
    });
    let primary = sub.home("Radar").find_object(radar.oid(), &base).unwrap();
    let radars = sub.home("Radar");

    // Contract, deleted by delete_contract.
    let view = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    let contract = view
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    view.delete_contract(&contract).unwrap();
    assert!(contract.is_deleted());
    let mut called = BTreeSet::new();
    deleted!(called, "set_depth", contract.set_depth(2));
    deleted!(
        called,
        "set_scope",
        contract.set_scope(ObjectScope::RelatedObjects)
    );
    assert_eq!(called, operations_of(&ast, "DDS::Contract"));
    assert_eq!(contract.depth(), 0);
    assert_eq!(contract.scope(), ObjectScope::SimpleObject);
    let mut extra = BTreeSet::new();
    deleted!(extra, "delete_contract", view.delete_contract(&contract));
    // purge detaches all contracts: they are deleted.
    let detached = view
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    view.purge().unwrap();
    assert!(detached.is_deleted());
    deleted!(extra, "set_depth", detached.set_depth(1));

    // Selection and its QueryCriterion, deleted by delete_selection.
    let q = QueryCriterion::new(radars, "name = 'r'", &[]).unwrap();
    let sel = radars
        .create_selection(SelectionCriterion::Query(q.clone()), false, false)
        .unwrap();
    assert_eq!(sel.members(), vec![primary.clone()]);
    radars.delete_selection(&sel).unwrap();
    assert!(sel.is_deleted());
    let mut called = BTreeSet::new();
    deleted!(called, "refresh", sel.refresh());
    assert_eq!(called, operations_of(&ast, "DDS::Selection"));
    deleted!(extra, "set_listener", sel.set_listener(None));
    deleted!(extra, "delete_selection", radars.delete_selection(&sel));
    let mut called = BTreeSet::new();
    deleted!(called, "set_query", q.set_query("name = 'x'", &[]));
    deleted!(called, "set_parameters", q.set_parameters(&[]));
    assert_eq!(called, operations_of(&ast, "DDS::QueryCriterion"));
    assert_eq!(q.expression(), "name = 'r'");

    // CacheAccess, deleted by delete_access.
    let other = view
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    view.refresh().unwrap();
    assert_eq!(view.objects().len(), 1);
    sub.cache.delete_access(&view).unwrap();
    assert!(view.is_deleted());
    assert!(other.is_deleted());
    assert!(view.objects().is_empty());
    assert!(!sub.cache.sub_accesses().contains(&view));
    let mut called = BTreeSet::new();
    deleted!(called, "write", view.write());
    deleted!(called, "purge", view.purge());
    deleted!(
        called,
        "create_contract",
        view.create_contract(&primary, ObjectScope::SimpleObject, 0)
    );
    deleted!(called, "delete_contract", view.delete_contract(&other));
    deleted!(called, "refresh", view.refresh());
    assert_eq!(called, operations_of(&ast, "DDS::CacheAccess"));
    deleted!(extra, "delete_access", sub.cache.delete_access(&view));
    // A deleted CacheAccess given to the home: AlreadyDeleted.
    let gone = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let held = publ.home("Radar").create_object(&gone).unwrap();
    let pending = publ
        .home("Radar")
        .create_unregistered_object(&gone)
        .unwrap();
    publ.cache.delete_access(&gone).unwrap();
    deleted!(
        extra,
        "register_object",
        publ.home("Radar").register_object(&pending)
    );
    deleted!(
        extra,
        "create_object",
        publ.home("Radar").create_object(&gone)
    );
    deleted!(
        extra,
        "create_unregistered_object",
        publ.home("Radar").create_unregistered_object(&gone)
    );
    // ... and its object is detached: destroy and set raise it.
    deleted!(extra, "destroy", held.destroy());
    deleted!(
        extra,
        "set",
        held.set("name", mono(Value::String("h".into())))
    );

    // ObjectRoot, destroyed, then published.
    radar.destroy().unwrap();
    deleted!(extra, "destroy", radar.destroy());
    deleted!(
        extra,
        "set",
        radar.set("name", mono(Value::String("s".into())))
    );
    writer.write().unwrap();
    assert!(!writer.objects().contains(&radar));
    deleted!(extra, "destroy", radar.destroy());
    deleted!(
        extra,
        "set",
        radar.set("name", mono(Value::String("t".into())))
    );
    // The reading Cache's object of the radar, deleted by the other
    // participant: read_state DELETED. It is not destroyed: its operations
    // read its last state, as the listeners of a deletion do (§8.1.6.4).
    wait_until("the deletion in the reading Cache", || {
        primary.read_state() == ObjectState::Deleted
    });
    assert_eq!(
        primary.get("name").unwrap(),
        mono(Value::String("r".into()))
    );
    assert!(primary.is_modified(ObjectScope::SimpleObject).is_ok());
    assert!(primary.is_attribute_modified("name").is_ok());

    // Cache, deleted by delete_cache, with its CacheAccess.
    let side = enabled(&bus, CacheUsage::ReadWrite, &m, &classes);
    let cache = side.cache.clone();
    let listener: Arc<dyn CacheListener> = Arc::new(Recorder::default());
    cache.attach_listener(Arc::clone(&listener)).unwrap();
    let access = cache.create_access(CacheUsage::ReadWrite).unwrap();
    let home = side.home("Radar").clone();
    let q = QueryCriterion::new(&home, "name = 'r'", &[]).unwrap();
    let sel = home
        .create_selection(SelectionCriterion::Query(q.clone()), true, false)
        .unwrap();
    delete(side);
    assert!(cache.is_deleted());
    assert!(access.is_deleted());
    deleted!(extra, "refresh", access.refresh());
    // A Selection of the deleted Cache, its QueryCriterion (which
    // refreshes the auto_refresh Selection) and the home's operations
    // with the deleted Cache or CacheAccess as source.
    deleted!(extra, "Selection::refresh", sel.refresh());
    deleted!(
        extra,
        "QueryCriterion::set_parameters",
        q.set_parameters(&[])
    );
    for source in [
        CacheBase::Cache(cache.clone()),
        CacheBase::Access(access.clone()),
    ] {
        deleted!(extra, "get_objects", home.get_objects(&source));
        deleted!(
            extra,
            "get_created_objects",
            home.get_created_objects(&source)
        );
        deleted!(
            extra,
            "get_modified_objects",
            home.get_modified_objects(&source)
        );
        deleted!(
            extra,
            "get_deleted_objects",
            home.get_deleted_objects(&source)
        );
        deleted!(
            extra,
            "find_object",
            home.find_object(primary.oid(), &source)
        );
    }
    let mut called = BTreeSet::new();
    let fresh = ObjectHome::new(Arc::clone(&m), "Radar").unwrap();
    deleted!(called, "register_home", cache.register_home(&fresh));
    deleted!(
        called,
        "find_home_by_name",
        cache.find_home_by_name("Radar")
    );
    deleted!(called, "find_home_by_index", cache.find_home_by_index(0));
    deleted!(
        called,
        "register_all_for_pubsub",
        cache.register_all_for_pubsub()
    );
    deleted!(
        called,
        "enable_all_for_pubsub",
        cache.enable_all_for_pubsub()
    );
    deleted!(
        called,
        "attach_listener",
        cache.attach_listener(Arc::clone(&listener))
    );
    deleted!(called, "detach_listener", cache.detach_listener(&listener));
    deleted!(called, "enable_updates", cache.enable_updates());
    deleted!(called, "disable_updates", cache.disable_updates());
    deleted!(called, "load", cache.load());
    deleted!(
        called,
        "create_access",
        cache.create_access(CacheUsage::ReadOnly)
    );
    deleted!(called, "delete_access", cache.delete_access(&access));
    deleted!(called, "lock", cache.lock(0));
    deleted!(called, "unlock", cache.unlock());
    deleted!(called, "refresh", cache.refresh());
    assert_eq!(called, operations_of(&ast, "DDS::Cache"));
    deleted!(
        extra,
        "deref_reference",
        cache.deref_reference(&ObjectRef::new("Radar", primary.oid()))
    );
    deleted!(
        extra,
        "delete_cache",
        CacheFactory::get_instance().delete_cache(&cache)
    );
    assert!(fresh.registration_index().is_none());
    assert_eq!(cache.homes().len(), classes.len());
    assert_eq!(cache.cache_usage(), CacheUsage::ReadWrite);
    assert!(
        CacheFactory::get_instance()
            .find_cache_by_name(cache.name())
            .is_none()
    );
    for op in [
        "delete_contract",
        "set_depth",
        "set_listener",
        "delete_selection",
        "delete_access",
        "create_object",
        "create_unregistered_object",
        "register_object",
        "destroy",
        "set",
        "refresh",
        "deref_reference",
        "delete_cache",
    ] {
        assert!(extra.contains(op), "{op}");
    }
    delete(sub);
    delete(publ);
}
