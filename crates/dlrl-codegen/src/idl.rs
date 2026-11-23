// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The implied IDL of an application class, DDS 1.2 §8.2.1.2.2.
//!
//! For an application class `Foo` (a valuetype deriving from
//! `DDS::ObjectRoot`, §8.2.2.2) the specification implies thirteen
//! declarations: `FooSeq`, `FooListener`, `FooListenerSeq`,
//! `FooSelectionListener`, `FooFilter`, `FooQuery`, `FooSelection`,
//! `FooSelectionSeq`, `FooHome`, `FooList`, `FooSet`, `FooStrMap` and
//! `FooIntMap`. [`generate_implied_idl`] writes them for one class, in the
//! modules of the class, against the generic IDL of `crates/dlrl/idl/
//! dds_dlrl.idl`.
//!
//! The text follows the printed implied IDL with these changes:
//!
//! * `CacheBase` is written `DDS::CacheBase`: unscoped it does not resolve
//!   outside module `DDS`.
//! * The operations of `FooListener` return `boolean`, as the table of
//!   §8.1.6.3.8 and the generic IDL say (TRUE: the event need not reach
//!   the listeners of the parent classes, §8.1.6.4.2); the printed implied
//!   IDL has `void`.
//! * `FooHome::create_selection` takes `concerns_contained_objects`, as
//!   the table of §8.1.6.3.7 and the generic IDL do, and raises
//!   `DDS::PreconditionNotMet` (a Cache in the DCPS state INITIAL,
//!   §8.1.6.3.7) beside the printed `DDS::BadParameter`.
//! * `FooHome::find_object` raises `DDS::NotFound`, as the generic IDL's
//!   `find_object` does.
//! * `FooSet::remove` returns `boolean` (the printed IDL has `void`), as
//!   the `remove` of `List`, `StrMap` and `IntMap` in the generic IDL of
//!   this tree (its change 9): FALSE when there was nothing to remove,
//!   the warning of §8.2.1.1 as a return value.
//!
//! The listeners' redefinitions of `on_object_created`,
//! `on_object_deleted` and `on_object_out` are legal because the generic
//! IDL of this tree has them only in its comment blocks (its change 3).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The implied IDL of the application class `class` (a scoped name such
/// as `demo::Track`, or a name at global scope), to be compiled after
/// `#include "dds_dlrl.idl"` and the class's own valuetype.
#[must_use]
pub fn generate_implied_idl(class: &str) -> String {
    let segments: Vec<&str> = class.split("::").filter(|s| !s.is_empty()).collect();
    let (modules, simple) = match segments.split_last() {
        Some((last, modules)) => (modules.to_vec(), *last),
        None => (Vec::new(), class),
    };
    let depth = modules.len();
    let pad = "    ".repeat(depth);
    let mut body = String::new();
    let mut line = |text: &str| {
        if text.is_empty() {
            body.push('\n');
        } else {
            body.push_str(&pad);
            body.push_str(text);
            body.push('\n');
        }
    };
    line(&format!(
        "// The implied IDL of {class} (DDS 1.2 §8.2.1.2.2)."
    ));
    line(&format!("typedef sequence<{simple}> {simple}Seq;"));
    line("");
    line(&format!(
        "local interface {simple}Listener : DDS::ObjectListener {{"
    ));
    for op in [
        "on_object_created",
        "on_object_modified",
        "on_object_deleted",
    ] {
        line(&format!("    boolean {op}(in {simple} the_object);"));
    }
    line("};");
    line(&format!(
        "typedef sequence<{simple}Listener> {simple}ListenerSeq;"
    ));
    line("");
    line(&format!(
        "local interface {simple}SelectionListener : DDS::SelectionListener {{"
    ));
    for op in ["on_object_in", "on_object_modified", "on_object_out"] {
        line(&format!("    void {op}(in {simple} the_object);"));
    }
    line("};");
    line("");
    line(&format!(
        "local interface {simple}Filter : DDS::FilterCriterion {{"
    ));
    line(&format!(
        "    boolean check_object(in {simple} an_object, in DDS::MembershipState membership_state);"
    ));
    line("};");
    line("");
    line(&format!(
        "local interface {simple}Query : DDS::QueryCriterion, {simple}Filter {{"
    ));
    line("};");
    line("");
    line(&format!(
        "local interface {simple}Selection : DDS::Selection {{"
    ));
    line(&format!("    readonly attribute {simple}Filter filter;"));
    line(&format!("    readonly attribute {simple}Seq members;"));
    line(&format!(
        "    readonly attribute {simple}SelectionListener listener;"
    ));
    line(&format!(
        "    {simple}SelectionListener set_listener(in {simple}SelectionListener listener);"
    ));
    line("};");
    line(&format!(
        "typedef sequence<{simple}Selection> {simple}SelectionSeq;"
    ));
    line("");
    line(&format!(
        "local interface {simple}Home : DDS::ObjectHome {{"
    ));
    line(&format!(
        "    readonly attribute {simple}SelectionSeq selections;"
    ));
    line(&format!(
        "    readonly attribute {simple}ListenerSeq listeners;"
    ));
    line(&format!(
        "    void attach_listener(in {simple}Listener listener, in boolean concerns_contained_objects);"
    ));
    line(&format!(
        "    void detach_listener(in {simple}Listener listener);"
    ));
    line(&format!(
        "    {simple}Selection create_selection(in {simple}Filter filter, in boolean auto_refresh, \
         in boolean concerns_contained_objects)"
    ));
    line("        raises (DDS::PreconditionNotMet, DDS::BadParameter);");
    line(&format!(
        "    void delete_selection(in {simple}Selection a_selection) raises (DDS::PreconditionNotMet);"
    ));
    line(&format!(
        "    {simple} create_object(in DDS::CacheAccess access) raises (DDS::PreconditionNotMet);"
    ));
    line(&format!(
        "    {simple} create_unregistered_object(in DDS::CacheAccess access) \
         raises (DDS::PreconditionNotMet);"
    ));
    line(&format!(
        "    void register_object(in {simple} unregistered_object)"
    ));
    line("        raises (DDS::AlreadyExisting, DDS::PreconditionNotMet);");
    line(&format!(
        "    {simple} find_object_in_access(in DDS::DLRLOid oid, in DDS::CacheAccess access) \
         raises (DDS::NotFound);"
    ));
    line(&format!(
        "    {simple} find_object(in DDS::DLRLOid oid) raises (DDS::NotFound);"
    ));
    for op in [
        "get_objects",
        "get_created_objects",
        "get_modified_objects",
        "get_deleted_objects",
    ] {
        line(&format!("    {simple}Seq {op}(in DDS::CacheBase source);"));
    }
    line("};");
    line("");
    line(&format!("valuetype {simple}List : DDS::List {{"));
    line(&format!("    readonly attribute {simple}Seq values;"));
    line(&format!("    void add(in {simple} value);"));
    line(&format!("    void put(in long key, in {simple} value);"));
    line(&format!("    {simple} get(in long key);"));
    line("};");
    line("");
    line(&format!("valuetype {simple}Set : DDS::Set {{"));
    line(&format!("    {simple}Seq values();"));
    line(&format!("    {simple}Seq added_elements();"));
    line(&format!("    {simple}Seq removed_elements();"));
    line(&format!("    boolean contains(in {simple} value);"));
    line(&format!("    void add(in {simple} value);"));
    // boolean as in the generic IDL of this tree (its change 9): the
    // warning of §8.2.1.1 as a return value.
    line(&format!("    boolean remove(in {simple} value);"));
    line("};");
    line("");
    line(&format!("valuetype {simple}StrMap : DDS::StrMap {{"));
    line(&format!("    readonly attribute {simple}Seq values;"));
    line(&format!("    void put(in string key, in {simple} value);"));
    line(&format!("    {simple} get(in string key);"));
    line("};");
    line("");
    line(&format!("valuetype {simple}IntMap : DDS::IntMap {{"));
    line(&format!("    readonly attribute {simple}Seq values;"));
    line(&format!("    void put(in long key, in {simple} value);"));
    line(&format!("    {simple} get(in long key);"));
    line("};");

    let mut out = String::new();
    for (level, m) in modules.iter().enumerate() {
        out.push_str(&"    ".repeat(level));
        out.push_str(&format!("module {m} {{\n"));
    }
    out.push_str(&body);
    for level in (0..depth).rev() {
        out.push_str(&"    ".repeat(level));
        out.push_str("};\n");
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    /// The thirteen constructs of §8.2.1.2.2, in the module of the class.
    #[test]
    fn dlrl_8_2_1_2_implied_idl_declares_every_derived_construct() {
        let text = generate_implied_idl("demo::Track");
        assert!(text.starts_with("module demo {\n"));
        assert!(text.ends_with("};\n"));
        for declaration in [
            "typedef sequence<Track> TrackSeq;",
            "local interface TrackListener : DDS::ObjectListener {",
            "typedef sequence<TrackListener> TrackListenerSeq;",
            "local interface TrackSelectionListener : DDS::SelectionListener {",
            "local interface TrackFilter : DDS::FilterCriterion {",
            "local interface TrackQuery : DDS::QueryCriterion, TrackFilter {",
            "local interface TrackSelection : DDS::Selection {",
            "typedef sequence<TrackSelection> TrackSelectionSeq;",
            "local interface TrackHome : DDS::ObjectHome {",
            "valuetype TrackList : DDS::List {",
            "valuetype TrackSet : DDS::Set {",
            "valuetype TrackStrMap : DDS::StrMap {",
            "valuetype TrackIntMap : DDS::IntMap {",
        ] {
            assert!(text.contains(declaration), "{declaration}\n{text}");
        }
        assert!(text.contains("boolean on_object_created(in Track the_object);"));
        assert!(text.contains("boolean remove(in Track value);"));
        assert!(text.contains("TrackSeq get_objects(in DDS::CacheBase source);"));
        assert!(!text.contains("(in CacheBase"));
        let global = generate_implied_idl("Foo");
        assert!(global.starts_with("// The implied IDL of Foo"));
        assert!(global.contains("local interface FooHome : DDS::ObjectHome {"));
    }
}
