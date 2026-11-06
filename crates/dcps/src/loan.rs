// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Samples taken with a loan (DDS 1.4 §2.2.2.5.3.5 `take`, §2.2.2.5.3.20
//! `return_loan`): the octets a DataReader received, lent to the
//! application instead of decoded into a value of its own.
//!
//! [`crate::DataReader::take_loan`] takes the samples out of the reader's
//! cache as they arrived — the serialized body, a range of the datagram it
//! came in — with their [`SampleInfo`]. A type whose fields borrow
//! (`&'a str`, `&'a [u8]`) is read from [`LoanedSample::body`] and borrows
//! from it (`zerodds-codec`). Dropping the [`Loan`] returns it.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sample_bytes::SampleBytes;
use crate::sample_info::SampleInfo;

/// One sample of a [`Loan`]: its [`SampleInfo`] and the octets it arrived
/// in.
#[derive(Debug, Clone)]
pub struct LoanedSample {
    /// What a `take` reports with the sample (§2.2.2.5.1).
    pub info: SampleInfo,
    pub(crate) bytes: Option<SampleBytes>,
    pub(crate) representation: u8,
    pub(crate) big_endian: bool,
}

impl LoanedSample {
    /// The serialized body behind the encapsulation header, lent out of
    /// the received datagram; `None` for a sample without data
    /// (`info.valid_data == false`, a dispose or an unregister).
    #[must_use]
    pub fn body(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    /// The body as a [`zerodds_rtps::payload::Payload`], sharing the
    /// buffer it arrived in.
    #[must_use]
    pub fn payload(&self) -> Option<&zerodds_rtps::payload::Payload> {
        self.bytes.as_ref().map(SampleBytes::payload)
    }

    /// Whether the body is XCDR1 (classic CDR) and not XCDR2, as its
    /// encapsulation header said (DDSI-RTPS 2.5 §10.5).
    #[must_use]
    pub fn is_xcdr1(&self) -> bool {
        self.representation == 0
    }

    /// Whether the body is big-endian, as its encapsulation header said.
    #[must_use]
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }
}

/// Samples a DataReader took and lends: [`crate::DataReader::take_loan`].
///
/// The octets stay where the reader received them for as long as the loan
/// lives; dropping it, or [`crate::DataReader::return_loan_of`], returns
/// it (§2.2.2.5.3.20).
#[derive(Debug)]
pub struct Loan {
    pub(crate) type_name: &'static str,
    pub(crate) samples: Vec<LoanedSample>,
    /// The lending reader's count of outstanding loans while this one is
    /// outstanding; `None` once it is returned.
    pub(crate) outstanding: Option<Arc<AtomicUsize>>,
}

impl Loan {
    /// Lets the samples go and counts the loan off its reader: afterwards
    /// the loan holds no sample and no loan (§2.2.2.5.3.20, "max_len=0").
    pub(crate) fn release(&mut self) {
        self.samples = Vec::new();
        if let Some(lender) = self.outstanding.take() {
            lender.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Whether this still holds a loan of its DataReader, that is, it was
    /// neither returned ([`crate::DataReader::return_loan_of`]) nor dropped.
    #[must_use]
    pub fn is_outstanding(&self) -> bool {
        self.outstanding.is_some()
    }

    /// The topic type the samples are of, as its DataReader's type names
    /// it ([`crate::DdsType::TYPE_NAME`]).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Number of samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the loan holds no sample.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The sample at `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&LoanedSample> {
        self.samples.get(index)
    }

    /// The samples, in the order `take` gives them.
    pub fn iter(&self) -> core::slice::Iter<'_, LoanedSample> {
        self.samples.iter()
    }
}

impl Drop for Loan {
    fn drop(&mut self) {
        self.release();
    }
}

impl<'a> IntoIterator for &'a Loan {
    type Item = &'a LoanedSample;
    type IntoIter = core::slice::Iter<'a, LoanedSample>;

    fn into_iter(self) -> Self::IntoIter {
        self.samples.iter()
    }
}
