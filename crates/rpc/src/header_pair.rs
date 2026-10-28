// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A service defined by a pair of types that carry their own header
//! (Spec §7.3.1.4, Basic Service Mapping).
//!
//! "In the Basic Service Mapping, the types annotated as
//! `@RPCRequestType` shall have a member named `header` of type
//! `dds::rpc::RequestHeader` and the types annotated as `@RPCReplyType`
//! shall have a member named `header`." The request and reply types are
//! then the topic types themselves: the sample is the encoding of the
//! user's structure, header first, and the runtime puts the identity of the
//! request in its header and reads it back from the leading octets
//! (§7.5.1.1.1 fixes the header's layout, so the leading octets of any such
//! sample are a [`RequestHeader`] or a [`ReplyHeader`]).
//!
//! The two traits are what a structure of that shape implements; the
//! generated Rust binding (`zerodds_lang_rules::emit::type_pair_rust`)
//! implements them. [`crate::requester::Requester::with_pair`] and
//! [`crate::replier::Replier::with_pair`] are the endpoints that use them.

extern crate alloc;

use zerodds_dcps::dds_type::DdsType;

use crate::common_types::{ReplyHeader, RequestHeader};
use crate::error::RpcResult;

/// A request type with its own `header` member.
pub trait RequestWithHeader: DdsType + Clone {
    /// A copy of `self` whose `header` is `header`: `requestId` and
    /// `instanceName` set, the other members as they are.
    ///
    /// # Errors
    /// `RpcError::Codec` when the `header` member cannot take the value
    /// (its type is not the `dds::rpc::RequestHeader` of §7.5.1.1.1).
    fn with_request_header(&self, header: &RequestHeader) -> RpcResult<Self>;
}

/// A reply type with its own `header` member.
pub trait ReplyWithHeader: DdsType + Clone + Default {
    /// A copy of `self` whose `header` is `header`: `relatedRequestId` and
    /// `remoteEx` set.
    ///
    /// # Errors
    /// As [`RequestWithHeader::with_request_header`].
    fn with_reply_header(&self, header: &ReplyHeader) -> RpcResult<Self>;
}
