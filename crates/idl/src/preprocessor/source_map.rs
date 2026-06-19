// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Source map: maps expanded output positions to
//! `(file, original offset)` (T6.3).
//!
//! After a preprocessor run, the lexer points at bytes of the `expanded`
//! string. Diagnostics, however, must point at the original source file
//! (a line from `inc.idl` is at a completely different position in the
//! expanded output).
//!
//! [`SourceMap`] collects segments: each segment is an output
//! range `[output_start, output_start + length)` that stems from a
//! file `file_id` starting at `original_offset`. `lookup(output_pos)`
//! finds the matching segment and computes the faithful original
//! position.

/// Stable identifier for a file (local within a
/// SourceMap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Original source position after lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub file_id: FileId,
    pub byte_offset: usize,
}

/// Source-Map.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<String>,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    output_start: usize,
    length: usize,
    file_id: FileId,
    original_offset: usize,
}

impl SourceMap {
    /// Constructs an empty SourceMap.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a file and returns its stable [`FileId`].
    pub fn add_file(&mut self, name: &str) -> FileId {
        if let Some(idx) = self.files.iter().position(|f| f == name) {
            return FileId(idx as u32);
        }
        let id = FileId(self.files.len() as u32);
        self.files.push(name.to_string());
        id
    }

    /// Records a segment. `output_start` and `length` refer to the
    /// expanded string; `original_offset` points to the
    /// first byte position in the `file_id` source.
    pub fn record_segment(
        &mut self,
        output_start: usize,
        length: usize,
        file_id: FileId,
        original_offset: usize,
    ) {
        if length == 0 {
            return;
        }
        self.segments.push(Segment {
            output_start,
            length,
            file_id,
            original_offset,
        });
    }

    /// Resolves an expanded position to the original (file, offset).
    /// Returns `None` if the position does not lie in a registered
    /// segment (e.g. between two segments or past the
    /// last one).
    #[must_use]
    pub fn lookup(&self, output_pos: usize) -> Option<SourceLocation> {
        for s in &self.segments {
            if output_pos >= s.output_start && output_pos < s.output_start + s.length {
                let delta = output_pos - s.output_start;
                return Some(SourceLocation {
                    file_id: s.file_id,
                    byte_offset: s.original_offset + delta,
                });
            }
        }
        None
    }

    /// File name for a [`FileId`].
    #[must_use]
    pub fn file_name(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0 as usize).map(String::as_str)
    }

    /// Number of registered segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Number of registered files.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn add_file_returns_stable_id_for_same_name() {
        let mut m = SourceMap::new();
        let a = m.add_file("foo.idl");
        let b = m.add_file("foo.idl");
        assert_eq!(a, b);
    }

    #[test]
    fn add_file_returns_distinct_ids_for_different_names() {
        let mut m = SourceMap::new();
        let a = m.add_file("a.idl");
        let b = m.add_file("b.idl");
        assert_ne!(a, b);
    }

    #[test]
    fn lookup_returns_none_for_empty_map() {
        let m = SourceMap::new();
        assert!(m.lookup(0).is_none());
    }

    #[test]
    fn lookup_finds_position_in_recorded_segment() {
        let mut m = SourceMap::new();
        let id = m.add_file("a.idl");
        m.record_segment(10, 5, id, 100);
        let loc = m.lookup(12).expect("in segment");
        assert_eq!(loc.file_id, id);
        assert_eq!(loc.byte_offset, 102);
    }

    #[test]
    fn lookup_beyond_last_segment_is_none() {
        let mut m = SourceMap::new();
        let id = m.add_file("a.idl");
        m.record_segment(0, 5, id, 0);
        assert!(m.lookup(100).is_none());
    }

    #[test]
    fn record_segment_with_zero_length_is_noop() {
        let mut m = SourceMap::new();
        let id = m.add_file("a.idl");
        m.record_segment(0, 0, id, 0);
        assert_eq!(m.segment_count(), 0);
    }

    #[test]
    fn file_name_resolves_back() {
        let mut m = SourceMap::new();
        let id = m.add_file("foo.idl");
        assert_eq!(m.file_name(id), Some("foo.idl"));
    }

    #[test]
    fn file_count_tracks_unique_files() {
        let mut m = SourceMap::new();
        m.add_file("a");
        m.add_file("b");
        m.add_file("a"); // duplicate
        assert_eq!(m.file_count(), 2);
    }

    #[test]
    fn lookup_works_across_multiple_segments() {
        let mut m = SourceMap::new();
        let a = m.add_file("a");
        let b = m.add_file("b");
        m.record_segment(0, 10, a, 0);
        m.record_segment(10, 20, b, 50);
        let l1 = m.lookup(5).expect("in a");
        assert_eq!(l1.file_id, a);
        assert_eq!(l1.byte_offset, 5);
        let l2 = m.lookup(15).expect("in b");
        assert_eq!(l2.file_id, b);
        assert_eq!(l2.byte_offset, 55);
    }
}
