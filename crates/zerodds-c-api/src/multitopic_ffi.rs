// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MultiTopic (DDS 1.4 §2.2.2.3.4, Annex B.2): `create_multitopic` and `delete_multitopic` of the
//! participant, the expression parameters, and the DataReader on a MultiTopic with `read` and
//! `take`. The topics of the `FROM` clause are topics the application created and told their
//! type with `zerodds_topic_set_type_objects`; the samples of the result are bodies of the
//! result type (XCDR2, little-endian).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;

use zerodds_dcps::DdsError;
use zerodds_dcps::instance_handle::InstanceHandle;
use zerodds_dcps::qos::{EndpointQosArg, SubscriberQos};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::subscriber::{DataReader, ReadSelection, Subscriber};
use zerodds_dcps::topic::MultiTopic;
use zerodds_types::dynamic::DynamicSample;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDomainParticipant, ZeroDdsSubscriber, ZeroDdsTopic};
use crate::qos_ffi::{ZeroDdsDataReaderQos, dr_qos_from_c_checked};
use crate::subscriber_ffi::ZeroDdsSampleInfo;

/// A MultiTopic of a participant.
pub struct ZeroDdsMultiTopic {
    participant: *mut ZeroDdsDomainParticipant,
    multitopic: MultiTopic<DynamicSample>,
    support: Arc<SerializedTypeSupport>,
}

/// The DataReader on a MultiTopic.
pub struct ZeroDdsMultiTopicReader {
    reader: DataReader<DynamicSample>,
    support: Arc<SerializedTypeSupport>,
    // Keeps the subscriber of the reader alive.
    _subscriber: Subscriber,
}

/// A sample of a MultiTopic reader: the body of the result type and its SampleInfo.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsMultiTopicSample {
    /// The SampleInfo; `representation` is 1 (XCDR2), `big_endian` 0.
    pub info: ZeroDdsSampleInfo,
    /// The body, allocated by the call; NULL for a sample without data.
    pub data: *mut u8,
    /// Length of `data`.
    pub len: usize,
}

fn status_of(error: &DdsError) -> c_int {
    (match error {
        DdsError::BadParameter { .. } => ZeroDdsStatus::BadParameter,
        DdsError::PreconditionNotMet { .. } => ZeroDdsStatus::PreconditionNotMet,
        DdsError::AlreadyDeleted => ZeroDdsStatus::AlreadyDeleted,
        DdsError::NotEnabled => ZeroDdsStatus::NotEnabled,
        _ => ZeroDdsStatus::Error,
    }) as c_int
}

/// The strings `list[0..count]` names.
///
/// # Safety
/// `list[0..count]` is readable and holds NUL-terminated strings or NULL.
unsafe fn strings(list: *const *const c_char, count: usize) -> Option<Vec<String>> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        // SAFETY: `list[0..count]` is readable per the caller's contract.
        let p = unsafe { *list.add(i) };
        if p.is_null() {
            return None;
        }
        // SAFETY: NUL-terminated per the caller's contract.
        let s = unsafe { core::ffi::CStr::from_ptr(p) }.to_str().ok()?;
        out.push(s.to_string());
    }
    Some(out)
}

/// The type support the object arrays describe.
///
/// # Safety
/// `type_object[0..len]`, `dependencies[0..count]` and `dependency_lens[0..count]` are readable,
/// and each dependency readable for its length.
#[allow(clippy::too_many_arguments)]
unsafe fn support_from(
    type_name: &str,
    type_object: *const u8,
    len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    count: usize,
    is_keyed: c_int,
    extensibility: u32,
) -> Option<Arc<SerializedTypeSupport>> {
    if type_object.is_null()
        || len == 0
        || (count > 0 && (dependencies.is_null() || dependency_lens.is_null()))
    {
        return None;
    }
    let extensibility = u8::try_from(extensibility).ok()?;
    let mut deps: Vec<Vec<u8>> = Vec::with_capacity(count);
    for i in 0..count {
        // SAFETY: `dependencies[0..count]` and `dependency_lens[0..count]` are readable.
        let (p, n) = unsafe { (*dependencies.add(i), *dependency_lens.add(i)) };
        if p.is_null() || n == 0 {
            return None;
        }
        // SAFETY: `p[0..n]` is readable per the contract.
        deps.push(unsafe { core::slice::from_raw_parts(p, n) }.to_vec());
    }
    // SAFETY: `type_object[0..len]` is readable per the contract.
    let root = unsafe { core::slice::from_raw_parts(type_object, len) };
    SerializedTypeSupport::from_type_objects(type_name, root, &deps, is_keyed != 0, extensibility)
        .ok()
}

/// Tells `topic` its type: the `CompleteTypeObject` and its dependencies, as
/// `zerodds_dr_set_type_objects` takes them. A MultiTopic needs the types of the topics of its
/// `FROM` clause.
///
/// # Safety
/// `topic` is a live topic handle; the object and the dependencies are readable for the lengths
/// given.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_set_type_objects(
    topic: *mut ZeroDdsTopic,
    type_object: *const u8,
    len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    count: usize,
    is_keyed: c_int,
    extensibility: u32,
) -> c_int {
    if topic.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null and live per the contract.
    let topic = unsafe { &*topic };
    // SAFETY: the pointers are the caller's per the contract.
    let Some(support) = (unsafe {
        support_from(
            &topic.type_name,
            type_object,
            len,
            dependencies,
            dependency_lens,
            count,
            is_keyed,
            extensibility,
        )
    }) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    match topic.support.lock() {
        Ok(mut slot) => {
            *slot = Some(support);
            ZeroDdsStatus::Ok as c_int
        }
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// `create_multitopic` (DDS 1.4 §2.2.2.2.1.9): the MultiTopic `name` of the result type
/// `type_name` (its `CompleteTypeObject` and dependencies given), selected by the
/// `subscription_expression` of Annex B.2 with the `expression_parameters`. The topics of the
/// `FROM` clause are the participant's topics told their type with
/// `zerodds_topic_set_type_objects`. NULL on failure, with `*out_status` (if not NULL):
/// `ZERODDS_BAD_PARAMETER` for an expression, topics and result type that do not fit together.
///
/// # Safety
/// `p` is a live participant, the strings NUL-terminated, `parameters[0..parameter_count]` and
/// the type object arrays readable, `out_status` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_multitopic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
    type_name: *const c_char,
    subscription_expression: *const c_char,
    parameters: *const *const c_char,
    parameter_count: usize,
    type_object: *const u8,
    len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    dependency_count: usize,
    is_keyed: c_int,
    extensibility: u32,
    out_status: *mut c_int,
) -> *mut ZeroDdsMultiTopic {
    let fail = |code: ZeroDdsStatus| -> *mut ZeroDdsMultiTopic {
        if !out_status.is_null() {
            // SAFETY: writable per the contract.
            unsafe { *out_status = code as c_int };
        }
        ptr::null_mut()
    };
    if p.is_null()
        || name.is_null()
        || type_name.is_null()
        || subscription_expression.is_null()
        || (parameter_count > 0 && parameters.is_null())
    {
        return fail(ZeroDdsStatus::BadParameter);
    }
    // SAFETY: NUL-terminated per the contract.
    let (name, type_name, expression) = unsafe {
        let text = |c: *const c_char| core::ffi::CStr::from_ptr(c).to_str().ok();
        match (text(name), text(type_name), text(subscription_expression)) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return fail(ZeroDdsStatus::BadParameter),
        }
    };
    // SAFETY: `parameters[0..parameter_count]` readable per the contract.
    let Some(params) = (unsafe { strings(parameters, parameter_count) }) else {
        return fail(ZeroDdsStatus::BadParameter);
    };
    // SAFETY: the type object arrays are the caller's per the contract.
    let Some(support) = (unsafe {
        support_from(
            type_name,
            type_object,
            len,
            dependencies,
            dependency_lens,
            dependency_count,
            is_keyed,
            extensibility,
        )
    }) else {
        return fail(ZeroDdsStatus::BadParameter);
    };
    // SAFETY: live per the contract.
    let participant = unsafe { &*p };
    if let Err(error) = support.register_type(&participant.dp, None) {
        return fail_with(out_status, &error);
    }
    // The participant's topics that know their type are the topics of the Rust participant a
    // `FROM` clause names; one that exists there already stays as it is.
    if let Ok(topics) = participant.topics.lock() {
        for &t in topics.iter() {
            if t.is_null() {
                continue;
            }
            // SAFETY: the list holds live topic pointers.
            let topic = unsafe { &*t };
            let Some(topic_support) = topic.support.lock().ok().and_then(|s| s.clone()) else {
                continue;
            };
            let qos = topic.qos.lock().map(|q| q.clone()).unwrap_or_default();
            let _ = participant
                .dp
                .create_dynamic_topic(&topic.name, qos, topic_support);
        }
    }
    match participant
        .dp
        .create_dynamic_multitopic(name, type_name, expression, params)
    {
        Ok(multitopic) => {
            if !out_status.is_null() {
                // SAFETY: writable per the contract.
                unsafe { *out_status = ZeroDdsStatus::Ok as c_int };
            }
            Box::into_raw(Box::new(ZeroDdsMultiTopic {
                participant: p,
                multitopic,
                support,
            }))
        }
        Err(error) => fail_with(out_status, &error),
    }
}

fn fail_with(out_status: *mut c_int, error: &DdsError) -> *mut ZeroDdsMultiTopic {
    if !out_status.is_null() {
        // SAFETY: writable per the caller's contract.
        unsafe { *out_status = status_of(error) };
    }
    ptr::null_mut()
}

/// `delete_multitopic` (DDS 1.4 §2.2.2.2.1.10). `ZERODDS_PRECONDITION_NOT_MET` while a reader
/// created on it exists or when it belongs to another participant; the handle stays valid then.
///
/// # Safety
/// `p` and `mt` are live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_multitopic(
    p: *mut ZeroDdsDomainParticipant,
    mt: *mut ZeroDdsMultiTopic,
) -> c_int {
    if p.is_null() || mt.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: live per the contract.
    let (participant, multitopic) = unsafe { (&*p, &*mt) };
    if multitopic.participant != p {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    }
    match participant.dp.delete_multitopic(&multitopic.multitopic) {
        Ok(()) => {
            // SAFETY: from `Box::into_raw`; not used afterwards per the contract.
            drop(unsafe { Box::from_raw(mt) });
            ZeroDdsStatus::Ok as c_int
        }
        Err(error) => status_of(&error),
    }
}

/// Replaces the `expression_parameters` of the MultiTopic (DDS 1.4 §2.2.2.3.4).
///
/// # Safety
/// `mt` is live, `parameters[0..count]` readable NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_multitopic_set_parameters(
    mt: *mut ZeroDdsMultiTopic,
    parameters: *const *const c_char,
    count: usize,
) -> c_int {
    if mt.is_null() || (count > 0 && parameters.is_null()) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: readable per the contract.
    let Some(params) = (unsafe { strings(parameters, count) }) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: live per the contract.
    match unsafe { &*mt }.multitopic.set_expression_parameters(params) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(error) => status_of(&error),
    }
}

/// `create_datareader` on a MultiTopic (DDS 1.4 §2.2.2.5.2.5): a reader of the result type.
/// `qos` NULL is the subscriber's default QoS. NULL on failure.
///
/// # Safety
/// `sub` and `mt` are live handles of one participant, `qos` NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_multitopic_reader(
    sub: *mut ZeroDdsSubscriber,
    mt: *mut ZeroDdsMultiTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsMultiTopicReader {
    if sub.is_null() || mt.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: live per the contract.
    let (subscriber, multitopic) = unsafe { (&*sub, &*mt) };
    if subscriber.participant != multitopic.participant {
        return ptr::null_mut();
    }
    // SAFETY: live participant of the subscriber.
    let participant = unsafe { &*subscriber.participant };
    let dcps_subscriber = participant.dp.create_subscriber(SubscriberQos::default());
    let qos_arg = if qos.is_null() {
        EndpointQosArg::Default
    } else {
        // SAFETY: readable per the contract.
        match unsafe { dr_qos_from_c_checked(qos) } {
            Ok(value) => EndpointQosArg::Value(value),
            Err(_) => return ptr::null_mut(),
        }
    };
    match dcps_subscriber.create_datareader_with_multitopic(&multitopic.multitopic, qos_arg) {
        Ok(reader) => Box::into_raw(Box::new(ZeroDdsMultiTopicReader {
            reader,
            support: Arc::clone(&multitopic.support),
            _subscriber: dcps_subscriber,
        })),
        Err(_) => ptr::null_mut(),
    }
}

/// Deletes a MultiTopic reader.
///
/// # Safety
/// `r` is NULL or from `zerodds_sub_create_multitopic_reader`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_multitopic_reader_destroy(r: *mut ZeroDdsMultiTopicReader) {
    if !r.is_null() {
        // SAFETY: from `Box::into_raw` per the contract.
        drop(unsafe { Box::from_raw(r) });
    }
}

/// `read` (`take` = 0) or `take` (`take` != 0) of the samples of the MultiTopic reader (DDS 1.4
/// §2.2.2.5.3.8 to §2.2.2.5.3.17), in the states the three masks name, at most `max_samples`
/// (`-1` is no limit):
///
/// - `scope` 0: every instance; 1: the instance `instance`; 2: the instance after `instance`;
/// - `expression` (NULL for none) and its `%n` parameters: the samples that meet the query
///   (`read_w_condition`, `take_w_condition`); with a query only `scope` 0 is provided, any other
///   gives `ZERODDS_UNSUPPORTED`.
///
/// `*out` is an array of `*out_count` samples allocated by the call (NULL for none); release it
/// with `zerodds_multitopic_samples_free`.
///
/// # Safety
/// `r` is live, `expression` NULL or NUL-terminated, `parameters[0..parameter_count]` readable
/// strings, `out` and `out_count` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_multitopic_reader_read_ex(
    r: *mut ZeroDdsMultiTopicReader,
    take: c_int,
    scope: c_int,
    instance: u64,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    max_samples: i32,
    expression: *const c_char,
    parameters: *const *const c_char,
    parameter_count: usize,
    out: *mut *mut ZeroDdsMultiTopicSample,
    out_count: *mut usize,
) -> c_int {
    if r.is_null() || out.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: live per the contract.
    let reader = unsafe { &*r };
    let selection = ReadSelection {
        max_samples,
        sample_states,
        view_states,
        instance_states,
    };
    let take = take != 0;
    let handle = InstanceHandle::from_raw(instance);
    let result = if expression.is_null() {
        match (scope, take) {
            (0, false) => reader.reader.read_selected(&selection),
            (0, true) => reader.reader.take_selected(&selection),
            (1, false) => reader.reader.read_instance_selected(handle, &selection),
            (1, true) => reader.reader.take_instance_selected(handle, &selection),
            (2, false) => reader
                .reader
                .read_next_instance_selected(handle, &selection),
            (2, true) => reader
                .reader
                .take_next_instance_selected(handle, &selection),
            _ => return ZeroDdsStatus::BadParameter as c_int,
        }
    } else {
        if scope != 0 {
            return ZeroDdsStatus::Unsupported as c_int;
        }
        // SAFETY: NUL-terminated per the contract.
        let Ok(expression) = (unsafe { core::ffi::CStr::from_ptr(expression) }).to_str() else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        if parameter_count > 0 && parameters.is_null() {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        // SAFETY: readable per the contract.
        let Some(params) = (unsafe { strings(parameters, parameter_count) }) else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        match reader.reader.create_querycondition(
            sample_states,
            view_states,
            instance_states,
            expression,
            params,
        ) {
            Ok(condition) => {
                if take {
                    reader
                        .reader
                        .take_w_condition_selected(&condition, max_samples)
                } else {
                    reader
                        .reader
                        .read_w_condition_selected(&condition, max_samples)
                }
            }
            Err(error) => return status_of(&error),
        }
    };
    let samples = match result {
        Ok(s) => s,
        Err(DdsError::NoData) => Vec::new(),
        Err(error) => return status_of(&error),
    };
    let mut converted: Vec<ZeroDdsMultiTopicSample> = Vec::with_capacity(samples.len());
    for sample in &samples {
        let mut info = crate::builtin_reader_ffi::sample_info_to_c(&sample.info);
        info.representation = 1;
        info.big_endian = 0;
        let (data, len) = if sample.info.valid_data {
            let body = sample
                .data
                .snapshot()
                .ok()
                .and_then(|value| reader.support.from_dynamic_data(value).ok())
                .map(|s| s.body().to_vec());
            match body {
                Some(bytes) => {
                    let boxed = bytes.into_boxed_slice();
                    let len = boxed.len();
                    (Box::leak(boxed).as_mut_ptr(), len)
                }
                None => {
                    for done in converted {
                        // SAFETY: allocated above.
                        unsafe { free_body(done) };
                    }
                    return ZeroDdsStatus::Error as c_int;
                }
            }
        } else {
            (ptr::null_mut(), 0)
        };
        converted.push(ZeroDdsMultiTopicSample { info, data, len });
    }
    let count = converted.len();
    let array = converted.into_boxed_slice();
    // SAFETY: `out` and `out_count` are writable per the contract.
    unsafe {
        *out_count = count;
        *out = if count == 0 {
            ptr::null_mut()
        } else {
            Box::into_raw(array).cast::<ZeroDdsMultiTopicSample>()
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `read` (`take` = 0) or `take` (`take` != 0) of every sample of the MultiTopic reader, any
/// state: `zerodds_multitopic_reader_read_ex` without selection.
///
/// # Safety
/// `r` is live, `out` and `out_count` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_multitopic_reader_read(
    r: *mut ZeroDdsMultiTopicReader,
    take: c_int,
    out: *mut *mut ZeroDdsMultiTopicSample,
    out_count: *mut usize,
) -> c_int {
    // SAFETY: the contract of this function is that of the extended one.
    unsafe {
        zerodds_multitopic_reader_read_ex(
            r,
            take,
            0,
            0,
            0xffff,
            0xffff,
            0xffff,
            -1,
            ptr::null(),
            ptr::null(),
            0,
            out,
            out_count,
        )
    }
}

/// Releases the body of a sample.
///
/// # Safety
/// `sample` was filled by `zerodds_multitopic_reader_read` and its body is not released yet.
unsafe fn free_body(sample: ZeroDdsMultiTopicSample) {
    if !sample.data.is_null() {
        // SAFETY: a boxed slice of `len` octets that `zerodds_multitopic_reader_read` leaked.
        drop(unsafe { Box::from_raw(core::slice::from_raw_parts_mut(sample.data, sample.len)) });
    }
}

/// Releases the array `zerodds_multitopic_reader_read` returned, with every body.
///
/// # Safety
/// `samples[0..count]` is the array and count that call returned, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_multitopic_samples_free(
    samples: *mut ZeroDdsMultiTopicSample,
    count: usize,
) {
    if samples.is_null() {
        return;
    }
    // SAFETY: the pointer and length of the boxed slice that call leaked.
    let array = unsafe { Box::from_raw(core::slice::from_raw_parts_mut(samples, count)) };
    for sample in array.iter() {
        // SAFETY: allocated by that call.
        unsafe { free_body(*sample) };
    }
}
