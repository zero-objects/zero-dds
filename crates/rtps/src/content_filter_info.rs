// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ContentFilterInfo_t` (DDSI-RTPS 2.5 §8.7.3.3 Table 8.87, §9.3.2.1) and
//! its in-line QoS parameter `PID_CONTENT_FILTER_INFO` (§9.6.4.1): what a
//! writer that filters on its side tells the readers about a sample it
//! does send, namely which filters it applied and which of them the sample
//! passed.

use alloc::vec::Vec;

use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::subscription_data::ContentFilterProperty;

/// `FilterSignature_t` (§9.3.2.1 `long[4]`): the 128-bit MD5 checksum of
/// the strings of a filter's `ContentFilterProperty_t` (§9.6.4.1), in the
/// order of its octets.
pub type FilterSignature = [u8; 16];

/// The signature of `filter` (§9.6.4.1): "the 128-bit MD5 checksum of all
/// strings in the filter's ContentFilterProperty_t", combined as
/// `[contentFilteredTopicName relatedTopicName filterClassName
/// filterExpression expressionParameters[0] …]`, "where each individual
/// string includes its NULL termination character".
#[must_use]
pub fn filter_signature(filter: &ContentFilterProperty) -> FilterSignature {
    let mut characters = Vec::new();
    let strings = [
        filter.content_filtered_topic_name.as_str(),
        filter.related_topic_name.as_str(),
        filter.filter_class_name.as_str(),
        filter.filter_expression.as_str(),
    ];
    for string in strings
        .into_iter()
        .chain(filter.expression_parameters.iter().map(|p| p.as_str()))
    {
        characters.extend_from_slice(string.as_bytes());
        characters.push(0);
    }
    zerodds_foundation::md5(&characters)
}

/// What a writer's filter made of one change for the readers with one
/// filter signature (§8.7.3.3, §8.7.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterOutcome {
    /// The filter that was applied.
    pub signature: FilterSignature,
    /// Whether the change passed it.
    pub passed: bool,
    /// For a change that did not pass: the previous sample of its instance
    /// passed, so the readers are told of this one by a DATA without a
    /// payload and with the FilteredFlag (§8.7.3.4: "the Writer must still
    /// send a Data submessage unless the previous sample for that Instance
    /// also did not pass the content filter"); `false` has it announced by
    /// a GAP.
    pub announce: bool,
}

/// `ContentFilterInfo_t`: the filters applied to a sample and, for each,
/// whether the sample passed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentFilterInfo {
    /// `filterSignatures`, with `filterResult` beside each: `true` is
    /// Table 9.21's 1, "filtered by the corresponding filter and passed".
    pub filters: Vec<(FilterSignature, bool)>,
}

impl ContentFilterInfo {
    /// The info of the outcomes a change carries.
    #[must_use]
    pub fn of(outcomes: &[FilterOutcome]) -> Self {
        Self {
            filters: outcomes.iter().map(|o| (o.signature, o.passed)).collect(),
        }
    }

    /// What the writer's filter `signature` made of the sample; `None`
    /// when the writer did not apply that filter (§8.7.3.3: "the Reader
    /// will ignore the filter results and instead apply its local
    /// filter").
    #[must_use]
    pub fn result_for(&self, signature: &FilterSignature) -> Option<bool> {
        self.filters
            .iter()
            .find(|(s, _)| s == signature)
            .map(|(_, passed)| *passed)
    }

    /// The wire form of §9.6.4.1: `numBitmaps`, the bitmaps (bit 0, the
    /// most significant, is the first signature), `numSignatures`, the
    /// signatures.
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> Vec<u8> {
        let put = |out: &mut Vec<u8>, v: u32| {
            out.extend_from_slice(&if little_endian {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            });
        };
        let bitmaps = self.filters.len().div_ceil(32);
        let mut out = Vec::with_capacity(8 + 4 * bitmaps + 16 * self.filters.len());
        put(&mut out, bitmaps as u32);
        for chunk in self.filters.chunks(32) {
            let mut bitmap = 0u32;
            for (bit, (_, passed)) in chunk.iter().enumerate() {
                if *passed {
                    bitmap |= 0x8000_0000 >> bit;
                }
            }
            put(&mut out, bitmap);
        }
        put(&mut out, self.filters.len() as u32);
        for (signature, _) in &self.filters {
            out.extend_from_slice(signature);
        }
        out
    }

    /// Reads the wire form. §9.6.4.1: "The content filter info in-line QoS
    /// is invalid unless numBitmaps == ([numSignatures/32] +
    /// (numSignatures%32 ? 1 : 0))".
    ///
    /// # Errors
    /// `ValueOutOfRange` for a value that ends early or whose counts do
    /// not agree.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        const INVALID: WireError = WireError::ValueOutOfRange {
            message: "PID_CONTENT_FILTER_INFO: the value ends early or its counts disagree",
        };
        let word = |at: usize| -> Result<u32, WireError> {
            let octets: [u8; 4] = bytes
                .get(at..at + 4)
                .and_then(|s| s.try_into().ok())
                .ok_or(INVALID)?;
            Ok(if little_endian {
                u32::from_le_bytes(octets)
            } else {
                u32::from_be_bytes(octets)
            })
        };
        let bitmaps = word(0)? as usize;
        // Bounded by the value's own length before anything is allocated.
        if bitmaps > bytes.len() / 4 {
            return Err(INVALID);
        }
        let signatures_at = 4 + 4 * bitmaps;
        let signatures = word(signatures_at)? as usize;
        if bitmaps != signatures.div_ceil(32) {
            return Err(INVALID);
        }
        let mut filters = Vec::with_capacity(signatures);
        for index in 0..signatures {
            let at = signatures_at + 4 + 16 * index;
            let signature: FilterSignature = bytes
                .get(at..at + 16)
                .and_then(|s| s.try_into().ok())
                .ok_or(INVALID)?;
            let bitmap = word(4 + 4 * (index / 32))?;
            filters.push((signature, bitmap & (0x8000_0000 >> (index % 32)) != 0));
        }
        Ok(Self { filters })
    }
}

/// `PID_CONTENT_FILTER_INFO` with `info`.
#[must_use]
pub fn content_filter_info_param(info: &ContentFilterInfo, little_endian: bool) -> Parameter {
    Parameter::new(pid::CONTENT_FILTER_INFO, info.to_bytes(little_endian))
}

/// The `PID_CONTENT_FILTER_INFO` of an in-line QoS list; `None` without
/// one or with one that is invalid, which a reader treats alike: it
/// applies its own filter.
#[must_use]
pub fn find_content_filter_info(
    list: &ParameterList,
    little_endian: bool,
) -> Option<ContentFilterInfo> {
    list.find(pid::CONTENT_FILTER_INFO)
        .and_then(|p| ContentFilterInfo::from_bytes(&p.value, little_endian).ok())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    fn property(parameters: &[&str]) -> ContentFilterProperty {
        ContentFilterProperty {
            content_filtered_topic_name: String::from("Cft"),
            related_topic_name: String::from("Topic"),
            filter_class_name: String::from("DDSSQL"),
            filter_expression: String::from("x > %0"),
            expression_parameters: parameters.iter().map(|p| String::from(*p)).collect(),
        }
    }

    /// §9.6.4.1: the MD5 of the strings one after another, each with its
    /// terminating NUL.
    #[test]
    fn rtps_9_6_4_1_the_signature_is_the_md5_of_the_strings_with_their_nul() {
        let signature = filter_signature(&property(&["5"]));
        assert_eq!(
            signature,
            zerodds_foundation::md5(b"Cft\0Topic\0DDSSQL\0x > %0\x005\0")
        );
        assert_ne!(
            signature,
            filter_signature(&property(&["6"])),
            "§8.7.3.3: the signature changes with the expression parameters"
        );
    }

    /// §9.6.4.1: bit 0 is the most significant bit and the first signature.
    #[test]
    fn rtps_9_6_4_1_the_wire_form_has_bit_zero_as_the_first_signature() {
        let info = ContentFilterInfo {
            filters: vec![([1; 16], true), ([2; 16], false), ([3; 16], true)],
        };
        let le = info.to_bytes(true);
        let mut expected = vec![1, 0, 0, 0, 0, 0, 0, 0xA0, 3, 0, 0, 0];
        expected.extend_from_slice(&[1; 16]);
        expected.extend_from_slice(&[2; 16]);
        expected.extend_from_slice(&[3; 16]);
        assert_eq!(le, expected);
        assert_eq!(ContentFilterInfo::from_bytes(&le, true).unwrap(), info);
        let be = info.to_bytes(false);
        assert_eq!(&be[..12], &[0, 0, 0, 1, 0xA0, 0, 0, 0, 0, 0, 0, 3]);
        assert_eq!(ContentFilterInfo::from_bytes(&be, false).unwrap(), info);
        assert_eq!(info.result_for(&[2; 16]), Some(false));
        assert_eq!(info.result_for(&[9; 16]), None);
    }

    /// 33 signatures take two bitmaps.
    #[test]
    fn rtps_9_6_4_1_thirty_three_signatures_take_two_bitmaps() {
        let info = ContentFilterInfo {
            filters: (0..33u8).map(|i| ([i; 16], i == 32)).collect(),
        };
        let bytes = info.to_bytes(true);
        assert_eq!(&bytes[..4], &[2, 0, 0, 0]);
        assert_eq!(&bytes[4..12], &[0, 0, 0, 0, 0, 0, 0, 0x80]);
        assert_eq!(ContentFilterInfo::from_bytes(&bytes, true).unwrap(), info);
    }

    /// §9.6.4.1: counts that do not agree make the parameter invalid.
    #[test]
    fn rtps_9_6_4_1_counts_that_disagree_are_invalid() {
        let mut bytes = ContentFilterInfo {
            filters: vec![([1; 16], true)],
        }
        .to_bytes(true);
        bytes[0] = 2;
        assert!(ContentFilterInfo::from_bytes(&bytes, true).is_err());
        assert!(ContentFilterInfo::from_bytes(&[1, 0, 0], true).is_err());
        let ended_early = ContentFilterInfo {
            filters: vec![([1; 16], true)],
        }
        .to_bytes(true);
        assert!(ContentFilterInfo::from_bytes(&ended_early[..20], true).is_err());
        let mut list = ParameterList::new();
        list.push(Parameter::new(pid::CONTENT_FILTER_INFO, bytes));
        assert_eq!(find_content_filter_info(&list, true), None);
    }
}
