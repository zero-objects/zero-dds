// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA StatusCodes (Part 4 §7.38, numeric values from the normative
//! `Opc.Ua.StatusCodes` CSV referenced by Part 6 Annex A.2).
//!
//! Only the codes the services in this crate can actually return are listed;
//! each one is used by at least one service arm in [`crate::server`].

/// `Good` — the operation succeeded.
pub const GOOD: u32 = 0x0000_0000;
/// `Good_SubscriptionTransferred`.
pub const GOOD_SUBSCRIPTION_TRANSFERRED: u32 = 0x002D_0000;

/// `Bad_ServiceUnsupported`.
pub const BAD_SERVICE_UNSUPPORTED: u32 = 0x800B_0000;
/// `Bad_NothingToDo`.
pub const BAD_NOTHING_TO_DO: u32 = 0x800F_0000;
/// `Bad_TooManyOperations`.
pub const BAD_TOO_MANY_OPERATIONS: u32 = 0x8010_0000;
/// `Bad_UserAccessDenied`.
pub const BAD_USER_ACCESS_DENIED: u32 = 0x801F_0000;
/// `Bad_IdentityTokenInvalid`.
pub const BAD_IDENTITY_TOKEN_INVALID: u32 = 0x8020_0000;
/// `Bad_IdentityTokenRejected`.
pub const BAD_IDENTITY_TOKEN_REJECTED: u32 = 0x8021_0000;
/// `Bad_SecureChannelIdInvalid`.
pub const BAD_SECURE_CHANNEL_ID_INVALID: u32 = 0x8022_0000;
/// `Bad_InvalidTimestamp`.
pub const BAD_INVALID_TIMESTAMP: u32 = 0x8023_0000;
/// `Bad_SessionIdInvalid`.
pub const BAD_SESSION_ID_INVALID: u32 = 0x8025_0000;
/// `Bad_SessionClosed`.
pub const BAD_SESSION_CLOSED: u32 = 0x8026_0000;
/// `Bad_SessionNotActivated`.
pub const BAD_SESSION_NOT_ACTIVATED: u32 = 0x8027_0000;
/// `Bad_SubscriptionIdInvalid`.
pub const BAD_SUBSCRIPTION_ID_INVALID: u32 = 0x8028_0000;
/// `Bad_TimestampsToReturnInvalid`.
pub const BAD_TIMESTAMPS_TO_RETURN_INVALID: u32 = 0x802B_0000;
/// `Bad_WaitingForInitialData`.
pub const BAD_WAITING_FOR_INITIAL_DATA: u32 = 0x8032_0000;
/// `Bad_NodeIdUnknown`.
pub const BAD_NODE_ID_UNKNOWN: u32 = 0x8034_0000;
/// `Bad_AttributeIdInvalid`.
pub const BAD_ATTRIBUTE_ID_INVALID: u32 = 0x8035_0000;
/// `Bad_IndexRangeInvalid`.
pub const BAD_INDEX_RANGE_INVALID: u32 = 0x8036_0000;
/// `Bad_IndexRangeNoData`.
pub const BAD_INDEX_RANGE_NO_DATA: u32 = 0x8037_0000;
/// `Bad_DataEncodingInvalid`.
pub const BAD_DATA_ENCODING_INVALID: u32 = 0x8038_0000;
/// `Bad_NotReadable`.
pub const BAD_NOT_READABLE: u32 = 0x803A_0000;
/// `Bad_NotWritable`.
pub const BAD_NOT_WRITABLE: u32 = 0x803B_0000;
/// `Bad_MonitoringModeInvalid`.
pub const BAD_MONITORING_MODE_INVALID: u32 = 0x8041_0000;
/// `Bad_MonitoredItemIdInvalid`.
pub const BAD_MONITORED_ITEM_ID_INVALID: u32 = 0x8042_0000;
/// `Bad_MonitoredItemFilterUnsupported`.
pub const BAD_MONITORED_ITEM_FILTER_UNSUPPORTED: u32 = 0x8044_0000;
/// `Bad_ContinuationPointInvalid`.
pub const BAD_CONTINUATION_POINT_INVALID: u32 = 0x804A_0000;
/// `Bad_NoContinuationPoints`.
pub const BAD_NO_CONTINUATION_POINTS: u32 = 0x804B_0000;
/// `Bad_BrowseDirectionInvalid`.
pub const BAD_BROWSE_DIRECTION_INVALID: u32 = 0x804D_0000;
/// `Bad_RequestTypeInvalid` — an OpenSecureChannel `requestType` that does not
/// match the channel state (Part 4 §5.6.2).
pub const BAD_REQUEST_TYPE_INVALID: u32 = 0x8053_0000;
/// `Bad_UserSignatureInvalid`.
pub const BAD_USER_SIGNATURE_INVALID: u32 = 0x8057_0000;
/// `Bad_MaxAgeInvalid`.
pub const BAD_MAX_AGE_INVALID: u32 = 0x8070_0000;
/// `Bad_WriteNotSupported`.
pub const BAD_WRITE_NOT_SUPPORTED: u32 = 0x8073_0000;
/// `Bad_TypeMismatch`.
pub const BAD_TYPE_MISMATCH: u32 = 0x8074_0000;
/// `Bad_MethodInvalid`.
pub const BAD_METHOD_INVALID: u32 = 0x8075_0000;
/// `Bad_TooManyPublishRequests`.
pub const BAD_TOO_MANY_PUBLISH_REQUESTS: u32 = 0x8078_0000;
/// `Bad_NoSubscription`.
pub const BAD_NO_SUBSCRIPTION: u32 = 0x8079_0000;
/// `Bad_SequenceNumberUnknown`.
pub const BAD_SEQUENCE_NUMBER_UNKNOWN: u32 = 0x807A_0000;
/// `Bad_MessageNotAvailable`.
pub const BAD_MESSAGE_NOT_AVAILABLE: u32 = 0x807B_0000;
/// `Bad_HistoryOperationInvalid` — "The history details parameter is not
/// valid." (Part 11 §6.4.3/§6.4.4's own field constraints, e.g. an empty
/// `req_times` on `ReadAtTimeDetails`.)
pub const BAD_HISTORY_OPERATION_INVALID: u32 = 0x8071_0000;
/// `Bad_HistoryOperationUnsupported` — "The server does not support the
/// requested operation." Used for `ReadRawModifiedDetails.is_read_modified
/// == true` (Part 11 §6.4.3: Modified-value history is not implemented).
pub const BAD_HISTORY_OPERATION_UNSUPPORTED: u32 = 0x8072_0000;
/// `Bad_NoData` — "No data exists for the requested time range or event
/// filter." (Part 11 §6.4: a historizing node whose retained window holds
/// no sample in the requested range — distinct from
/// [`BAD_NODE_ID_UNKNOWN`]'s "not historizing at all".)
pub const BAD_NO_DATA: u32 = 0x809B_0000;
