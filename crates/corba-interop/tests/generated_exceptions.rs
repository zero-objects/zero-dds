// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CORBA 3.3 Part 1 §8.12: the system exceptions the generated skeletons
//! and stubs of `zerodds-corba-rust` raise, called on the code
//! `build.rs` generates for `CosNaming::NamingContext`.
//!
//! - Arguments that do not unmarshal: `MARSHAL`, `COMPLETED_NO` (the
//!   operation never ran).
//! - A user exception the operation does not declare: `UNKNOWN` with
//!   standard minor code 1, `COMPLETED_YES` (§8.12.3.1; Annex A.6, "Unlisted
//!   user exception received by client").
//! - A reply that does not unmarshal in the stub: `MARSHAL`,
//!   `COMPLETED_YES` (the operation ran), for an operation with and one
//!   without a raises clause.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unused_imports
)]

use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));

use corba_gen::CosNaming::{
    AlreadyBound, CosName, NamingContext, NamingContextError, NamingContextStub,
    dispatch_namingcontext,
};
use zerodds_corba_rt::OMGVMCID;
use zerodds_corba_rust::{
    CompletionStatus, CorbaConnection, CorbaException, ObjectReference, SkeletonResult,
    SystemException, SystemExceptionKind,
};
use zerodds_wire::Endianness;

/// `resolve` raises `AlreadyBound`, which only `bind` and
/// `bind_new_context` declare.
struct Servant;

impl NamingContext for Servant {
    fn bind(&self, _n: CosName, _obj: ObjectReference) -> Result<(), NamingContextError> {
        Ok(())
    }
    fn rebind(&self, _n: CosName, _obj: ObjectReference) -> Result<(), NamingContextError> {
        Ok(())
    }
    fn resolve(&self, _n: CosName) -> Result<ObjectReference, NamingContextError> {
        Err(NamingContextError::AlreadyBound(AlreadyBound {}))
    }
    fn unbind(&self, _n: CosName) -> Result<(), NamingContextError> {
        Ok(())
    }
    fn new_context(&self) -> Result<ObjectReference, CorbaException> {
        Ok(ObjectReference::default())
    }
    fn bind_new_context(&self, _n: CosName) -> Result<ObjectReference, NamingContextError> {
        Ok(ObjectReference::default())
    }
    fn destroy(&self) -> Result<(), NamingContextError> {
        Ok(())
    }
}

fn system_of(result: SkeletonResult) -> SystemException {
    match result {
        SkeletonResult::Exception(CorbaException::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}

#[test]
fn corba_p1_8_12_2_the_skeleton_raises_marshal_completed_no_for_bad_arguments() {
    // `resolve(in CosName n)` without the sequence length.
    let e = system_of(dispatch_namingcontext(
        &Servant,
        "resolve",
        &zerodds_corba_rust::AlignedBody::new(&[0, 0], 0),
        Endianness::Big,
    ));
    assert_eq!(
        e,
        SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
    );
}

#[test]
fn corba_p1_8_12_3_1_an_unlisted_user_exception_is_unknown_minor_1_completed_yes() {
    // An empty CosName: the arguments unmarshal, the servant runs.
    let e = system_of(dispatch_namingcontext(
        &Servant,
        "resolve",
        &zerodds_corba_rust::AlignedBody::new(&[0, 0, 0, 0], 0),
        Endianness::Big,
    ));
    assert_eq!(
        e,
        SystemException::new(
            SystemExceptionKind::Unknown,
            OMGVMCID | 1,
            CompletionStatus::Yes
        )
    );
    // A declared exception still crosses as the user exception.
    let mut w = zerodds_wire::Writer::new(Endianness::Big);
    w.write_u32(0);
    ObjectReference::default().encode(&mut w);
    match dispatch_namingcontext(
        &Servant,
        "bind",
        &zerodds_corba_rust::AlignedBody::new(&w.finish(), 0),
        Endianness::Big,
    ) {
        SkeletonResult::Reply(_) => {}
        other => panic!("bind: {other:?}"),
    }
}

/// A connection whose replies carry one octet: no reply body of the
/// NamingContext operations decodes from it.
struct TruncatedReplies;

impl CorbaConnection for TruncatedReplies {
    fn invoke(
        &self,
        _target: &ObjectReference,
        _operation: &str,
        e: Endianness,
        _arguments: &zerodds_corba_rust::MarshalArgs<'_>,
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        Ok((vec![0xff], e))
    }
    fn invoke_oneway(
        &self,
        _target: &ObjectReference,
        _operation: &str,
        _e: Endianness,
        _arguments: &zerodds_corba_rust::MarshalArgs<'_>,
    ) -> Result<(), CorbaException> {
        Ok(())
    }
}

#[test]
fn corba_p1_8_12_2_the_stub_raises_marshal_completed_yes_for_a_bad_reply() {
    let stub = NamingContextStub::new(ObjectReference::default(), Arc::new(TruncatedReplies));
    let marshal_yes = SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::Yes);
    // An operation with a raises clause: the error enum's System variant.
    match stub.resolve(Vec::new()) {
        Err(NamingContextError::System(CorbaException::System(e))) => assert_eq!(e, marshal_yes),
        other => panic!("resolve: {other:?}"),
    }
    // An operation without one: the CorbaException itself.
    assert_eq!(
        stub.new_context().unwrap_err(),
        CorbaException::System(marshal_yes)
    );
}
