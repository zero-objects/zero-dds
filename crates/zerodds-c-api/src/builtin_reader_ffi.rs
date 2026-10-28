// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DataReaders of the built-in subscriber (DDS 1.4 §2.2.5): `lookup_datareader` of the
//! built-in subscriber by topic name and `read`/`take` of their samples, for `DCPSParticipant`,
//! `DCPSTopic`, `DCPSPublication` and `DCPSSubscription`. The data of a sample is the C form
//! the `zerodds_dp_get_discovered_*` calls already use.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;

use zerodds_dcps::builtin_topics::{
    ParticipantBuiltinTopicData, PublicationBuiltinTopicData, SubscriptionBuiltinTopicData,
    TopicBuiltinTopicData,
};
use zerodds_dcps::sample::Sample;
use zerodds_dcps::sample_info::{InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind};
use zerodds_dcps::subscriber::DataReader;

use crate::ZeroDdsStatus;
use crate::builtin_ffi::{
    ZeroDdsParticipantBuiltinTopicData, ZeroDdsPublicationBuiltinTopicData,
    ZeroDdsSubscriptionBuiltinTopicData, ZeroDdsTopicBuiltinTopicData, publication_to_c,
    subscription_to_c, topic_to_c, zerodds_publication_builtin_topic_data_free,
    zerodds_subscription_builtin_topic_data_free, zerodds_topic_builtin_topic_data_free,
};
use crate::extra_ffi::ZeroDdsBuiltinSubscriber;
use crate::subscriber_ffi::ZeroDdsSampleInfo;

/// The kind of a built-in reader and of the sample it hands out.
pub const ZERODDS_BUILTIN_PARTICIPANT: u32 = 1;
/// `DCPSTopic`.
pub const ZERODDS_BUILTIN_TOPIC: u32 = 2;
/// `DCPSPublication`.
pub const ZERODDS_BUILTIN_PUBLICATION: u32 = 3;
/// `DCPSSubscription`.
pub const ZERODDS_BUILTIN_SUBSCRIPTION: u32 = 4;

enum Inner {
    Participant(DataReader<ParticipantBuiltinTopicData>),
    Topic(DataReader<TopicBuiltinTopicData>),
    Publication(DataReader<PublicationBuiltinTopicData>),
    Subscription(DataReader<SubscriptionBuiltinTopicData>),
}

/// A DataReader of the built-in subscriber.
pub struct ZeroDdsBuiltinReader {
    inner: Inner,
}

/// A sample of a built-in reader: `kind` names the member that holds the data; the other three
/// are zeroed. `info.valid_data` is false for a dispose, which carries the key only.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsBuiltinSample {
    /// `ZERODDS_BUILTIN_*`.
    pub kind: u32,
    /// The SampleInfo.
    pub info: ZeroDdsSampleInfo,
    /// `DCPSParticipant` data, for kind 1.
    pub participant: ZeroDdsParticipantBuiltinTopicData,
    /// `DCPSTopic` data, for kind 2.
    pub topic: ZeroDdsTopicBuiltinTopicData,
    /// `DCPSPublication` data, for kind 3.
    pub publication: ZeroDdsPublicationBuiltinTopicData,
    /// `DCPSSubscription` data, for kind 4.
    pub subscription: ZeroDdsSubscriptionBuiltinTopicData,
}

pub(crate) fn sample_info_to_c(info: &SampleInfo) -> ZeroDdsSampleInfo {
    ZeroDdsSampleInfo {
        sample_state: match info.sample_state {
            SampleStateKind::Read => 1,
            SampleStateKind::NotRead => 2,
        },
        view_state: match info.view_state {
            ViewStateKind::New => 1,
            ViewStateKind::NotNew => 2,
        },
        instance_state: match info.instance_state {
            InstanceStateKind::Alive => 1,
            InstanceStateKind::NotAliveDisposed => 2,
            InstanceStateKind::NotAliveNoWriters => 4,
        },
        disposed_generation_count: info.disposed_generation_count,
        no_writers_generation_count: info.no_writers_generation_count,
        sample_rank: info.sample_rank,
        generation_rank: info.generation_rank,
        absolute_generation_rank: info.absolute_generation_rank,
        source_timestamp_sec: info.source_timestamp.sec,
        source_timestamp_nanosec: info.source_timestamp.nanosec,
        instance_handle: info.instance_handle.as_raw(),
        publication_handle: info.publication_handle.as_raw(),
        valid_data: info.valid_data,
        representation: 0,
        big_endian: 0,
    }
}

fn zeroed_sample(kind: u32, info: &SampleInfo) -> ZeroDdsBuiltinSample {
    // SAFETY: every member is plain data (integers, byte arrays, raw pointers); all-zero is a
    // valid value of each, and means "nothing" for the pointers.
    let mut sample: ZeroDdsBuiltinSample = unsafe { core::mem::zeroed() };
    sample.kind = kind;
    sample.info = sample_info_to_c(info);
    sample
}

fn participant_sample(s: &Sample<ParticipantBuiltinTopicData>) -> ZeroDdsBuiltinSample {
    let mut out = zeroed_sample(ZERODDS_BUILTIN_PARTICIPANT, &s.info);
    out.participant.guid.copy_from_slice(&s.data.key.to_bytes());
    if !s.data.user_data.is_empty() {
        let bytes = s.data.user_data.clone().into_boxed_slice();
        out.participant.user_data_len = bytes.len();
        out.participant.user_data = Box::leak(bytes).as_ptr();
    }
    out
}

fn convert<T, F>(samples: Vec<Sample<T>>, kind: u32, fill: F) -> Option<Vec<ZeroDdsBuiltinSample>>
where
    F: Fn(&mut ZeroDdsBuiltinSample, &T) -> bool,
{
    let mut out: Vec<ZeroDdsBuiltinSample> = Vec::with_capacity(samples.len());
    for s in &samples {
        let mut sample = zeroed_sample(kind, &s.info);
        if !fill(&mut sample, &s.data) {
            for mut done in out {
                // SAFETY: filled above by this function.
                unsafe { zerodds_builtin_sample_free(&mut done) };
            }
            return None;
        }
        out.push(sample);
    }
    Some(out)
}

/// `lookup_datareader(topic_name)` of the built-in subscriber (DDS 1.4 §2.2.2.5.2.7): the reader
/// of `DCPSParticipant`, `DCPSTopic`, `DCPSPublication` or `DCPSSubscription`. NULL for another
/// name. The caller releases it with `zerodds_builtin_reader_destroy`.
///
/// # Safety
/// `bs` is a live built-in subscriber handle, `topic_name` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_subscriber_lookup_datareader(
    bs: *mut ZeroDdsBuiltinSubscriber,
    topic_name: *const c_char,
) -> *mut ZeroDdsBuiltinReader {
    if bs.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: non-null and live per the contract.
    let (subscriber, name) = unsafe { (&*bs, core::ffi::CStr::from_ptr(topic_name)) };
    let Ok(name) = name.to_str() else {
        return ptr::null_mut();
    };
    let inner = match name {
        "DCPSParticipant" => Inner::Participant(subscriber.inner.participant_reader()),
        "DCPSTopic" => Inner::Topic(subscriber.inner.topic_reader()),
        "DCPSPublication" => Inner::Publication(subscriber.inner.publication_reader()),
        "DCPSSubscription" => Inner::Subscription(subscriber.inner.subscription_reader()),
        _ => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(ZeroDdsBuiltinReader { inner }))
}

/// Releases a built-in reader.
///
/// # Safety
/// `r` is NULL or from `zerodds_builtin_subscriber_lookup_datareader`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_reader_destroy(r: *mut ZeroDdsBuiltinReader) {
    if !r.is_null() {
        // SAFETY: from `Box::into_raw` per the contract.
        drop(unsafe { Box::from_raw(r) });
    }
}

/// The kind (`ZERODDS_BUILTIN_*`) of the samples of `r`; 0 for NULL.
///
/// # Safety
/// `r` is NULL or a live built-in reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_reader_kind(r: *const ZeroDdsBuiltinReader) -> u32 {
    if r.is_null() {
        return 0;
    }
    // SAFETY: non-null and live per the contract.
    match unsafe { &(*r).inner } {
        Inner::Participant(_) => ZERODDS_BUILTIN_PARTICIPANT,
        Inner::Topic(_) => ZERODDS_BUILTIN_TOPIC,
        Inner::Publication(_) => ZERODDS_BUILTIN_PUBLICATION,
        Inner::Subscription(_) => ZERODDS_BUILTIN_SUBSCRIPTION,
    }
}

/// `read` (`take` = 0) or `take` (`take` != 0) of all samples of the reader, any state (DDS 1.4
/// §2.2.2.5.3.8/9). `*out` is an array of `*out_count` samples, allocated by the call (NULL for
/// none); the caller releases it with `zerodds_builtin_samples_free`.
///
/// # Safety
/// `r` is a live built-in reader, `out` and `out_count` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_reader_read(
    r: *mut ZeroDdsBuiltinReader,
    take: c_int,
    out: *mut *mut ZeroDdsBuiltinSample,
    out_count: *mut usize,
) -> c_int {
    if r.is_null() || out.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null and live per the contract.
    let reader = unsafe { &(*r).inner };
    let result = match reader {
        Inner::Participant(dr) => {
            let samples = if take != 0 {
                dr.take_with_info()
            } else {
                dr.read_with_info()
            };
            samples.map(|v| Some(v.iter().map(participant_sample).collect::<Vec<_>>()))
        }
        Inner::Topic(dr) => {
            let samples = if take != 0 {
                dr.take_with_info()
            } else {
                dr.read_with_info()
            };
            samples.map(|v| {
                convert(v, ZERODDS_BUILTIN_TOPIC, |s, d| match topic_to_c(d) {
                    Some(c) => {
                        s.topic = c;
                        true
                    }
                    None => false,
                })
            })
        }
        Inner::Publication(dr) => {
            let samples = if take != 0 {
                dr.take_with_info()
            } else {
                dr.read_with_info()
            };
            samples.map(|v| {
                convert(
                    v,
                    ZERODDS_BUILTIN_PUBLICATION,
                    |s, d| match publication_to_c(d) {
                        Some(c) => {
                            s.publication = c;
                            true
                        }
                        None => false,
                    },
                )
            })
        }
        Inner::Subscription(dr) => {
            let samples = if take != 0 {
                dr.take_with_info()
            } else {
                dr.read_with_info()
            };
            samples.map(|v| {
                convert(
                    v,
                    ZERODDS_BUILTIN_SUBSCRIPTION,
                    |s, d| match subscription_to_c(d) {
                        Some(c) => {
                            s.subscription = c;
                            true
                        }
                        None => false,
                    },
                )
            })
        }
    };
    let converted = match result {
        Ok(Some(v)) => v,
        Ok(None) => return ZeroDdsStatus::BadParameter as c_int,
        Err(zerodds_dcps::DdsError::NotEnabled) => return ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => return ZeroDdsStatus::Error as c_int,
    };
    let count = converted.len();
    let array = converted.into_boxed_slice();
    // SAFETY: `out` and `out_count` are writable per the contract.
    unsafe {
        *out_count = count;
        *out = if count == 0 {
            ptr::null_mut()
        } else {
            Box::into_raw(array).cast::<ZeroDdsBuiltinSample>()
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// Releases the array `zerodds_builtin_reader_read` returned, with the data of every sample.
///
/// # Safety
/// `samples[0..count]` is the array and count that call returned, not released before; or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_samples_free(
    samples: *mut ZeroDdsBuiltinSample,
    count: usize,
) {
    if samples.is_null() {
        return;
    }
    // SAFETY: the pointer and length of a boxed slice that call leaked.
    let mut array = unsafe { Box::from_raw(core::slice::from_raw_parts_mut(samples, count)) };
    for sample in array.iter_mut() {
        // SAFETY: filled by `zerodds_builtin_reader_read`.
        unsafe { zerodds_builtin_sample_free(sample) };
    }
}

/// Releases what a sample of `zerodds_builtin_reader_read` holds and zeroes the data.
///
/// # Safety
/// `s` is NULL or a sample that call filled and that was not released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_sample_free(s: *mut ZeroDdsBuiltinSample) {
    if s.is_null() {
        return;
    }
    // SAFETY: non-null and filled by `zerodds_builtin_reader_read` per the contract.
    unsafe {
        let sample = &mut *s;
        match sample.kind {
            ZERODDS_BUILTIN_PARTICIPANT => {
                crate::builtin_ffi::zerodds_builtin_userdata_free(
                    sample.participant.user_data,
                    sample.participant.user_data_len,
                );
                sample.participant.user_data = ptr::null();
                sample.participant.user_data_len = 0;
            }
            ZERODDS_BUILTIN_TOPIC => zerodds_topic_builtin_topic_data_free(&mut sample.topic),
            ZERODDS_BUILTIN_PUBLICATION => {
                zerodds_publication_builtin_topic_data_free(&mut sample.publication);
            }
            ZERODDS_BUILTIN_SUBSCRIPTION => {
                zerodds_subscription_builtin_topic_data_free(&mut sample.subscription);
            }
            _ => {}
        }
    }
}
