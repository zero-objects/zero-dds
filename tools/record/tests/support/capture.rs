// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// What the e2e tests read back from a capture `record` wrote (`#[path]`
// include).

use std::collections::BTreeMap;
use std::path::Path;

use zerodds_recorder::reader::RecordReader;

/// How often each payload occurs in the capture `capture`; frames without
/// a payload are left out.
pub fn payload_counts(capture: &Path) -> BTreeMap<Vec<u8>, usize> {
    let bytes = std::fs::read(capture).expect("the capture");
    let mut reader = RecordReader::new(&bytes);
    reader.parse_header().expect("the header");
    let mut seen: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    while let Some(frame) = reader.next_frame().expect("a frame") {
        if !frame.payload.is_empty() {
            *seen.entry(frame.payload).or_default() += 1;
        }
    }
    seen
}
