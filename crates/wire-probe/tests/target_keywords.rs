// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The spellings three IDL compilers give an identifier their target
//! language reserves.
//!
//! Every assertion below is a measurement and not a rule anybody
//! derived. All three were taken 2026-08-11 on the Linux test host, from
//! each generator's own output for
//! `tests/interop-corpus/identifier-target-keywords/type.idl` — the
//! corpus case whose module is called `package`, whose enumeration is
//! called `class` and whose members are called `new`, `delete`,
//! `operator`, `static`, `impl`, `lambda`, `def` and `async`.
//!
//! The negative cases carry the weight. That `new` is renamed by the C++
//! front ends and not by RTI's C one is the reason there are three
//! functions and not one, and a test that only checked the renamings
//! would pass just as well against a single shared list that renamed
//! `new` for everybody and put a member the vendor's own header does not
//! have into the probe.

use zerodds_wire_probe::target_keywords::{c, cxx, java};

/// omniidl 4.3.3 and `tao_idl` 2.5.24, measured: `_cxx_new`,
/// `_cxx_delete`, `_cxx_operator`, `_cxx_static`, and
/// `enum _cxx_class { _cxx_template, _cxx_virtual, _cxx_friend }`.
#[test]
fn the_cpp_front_ends_prefix_a_cpp_keyword() {
    assert_eq!(cxx("new"), "_cxx_new");
    assert_eq!(cxx("delete"), "_cxx_delete");
    assert_eq!(cxx("operator"), "_cxx_operator");
    assert_eq!(cxx("static"), "_cxx_static");
    assert_eq!(cxx("class"), "_cxx_class");
    assert_eq!(cxx("template"), "_cxx_template");
    assert_eq!(cxx("virtual"), "_cxx_virtual");
    assert_eq!(cxx("friend"), "_cxx_friend");
}

/// The same headers, measured: `package`, `type`, `impl`, `lambda`,
/// `def` and `async` appear verbatim. They are keywords in Java, Rust or
/// Python and none in C++, and a front end that renamed them would be a
/// different measurement.
#[test]
fn the_cpp_front_ends_leave_a_non_keyword_alone() {
    assert_eq!(cxx("lambda"), "lambda");
    assert_eq!(cxx("package"), "package");
    assert_eq!(cxx("type"), "type");
    assert_eq!(cxx("impl"), "impl");
    assert_eq!(cxx("def"), "def");
    assert_eq!(cxx("async"), "async");
}

/// `rtiddsgen -language C`, Connext 7.7, measured: `DDS_Long _c_static;`
/// beside `DDS_Long new;` and `DDS_Double delete;`. `new` and `delete`
/// are C++ keywords and C has neither, so the C generator renames one of
/// the three members and leaves two.
#[test]
fn the_c_generator_prefixes_a_c_keyword_and_only_a_c_keyword() {
    assert_eq!(c("static"), "_c_static");
    assert_eq!(c("new"), "new");
    assert_eq!(c("delete"), "delete");
    assert_eq!(c("operator"), "operator");
    assert_eq!(c("class"), "class");
    assert_eq!(c("template"), "template");
}

/// JacORB 3.9, measured: the IDL module `package` becomes the Java
/// package `_package` and the enumeration `class` the class `_class`,
/// while the enumerators `template`, `virtual` and `friend` keep their
/// spelling — Java reserves none of the three.
#[test]
fn jacorb_prefixes_a_java_keyword_with_one_underscore() {
    assert_eq!(java("package"), "_package");
    assert_eq!(java("class"), "_class");
    assert_eq!(java("new"), "_new");
    assert_eq!(java("static"), "_static");
    assert_eq!(java("virtual"), "virtual");
    assert_eq!(java("template"), "template");
    assert_eq!(java("friend"), "friend");
    assert_eq!(java("operator"), "operator");
    assert_eq!(java("delete"), "delete");
}

/// The three prefixes are three generators' and are not
/// interchangeable. A member the C++ front ends call `_cxx_static` is
/// `_c_static` in RTI's header and `_static` in JacORB's class, and a
/// probe that used the wrong one names a field that is not there.
#[test]
fn one_name_takes_three_spellings() {
    assert_eq!(cxx("static"), "_cxx_static");
    assert_eq!(c("static"), "_c_static");
    assert_eq!(java("static"), "_static");
}
