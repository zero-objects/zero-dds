// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Source-Map: mappt expandierte Output-Positionen auf
//! `(Datei, Original-Offset)` (T6.3).
//!
//! Nach Preprocessor-Lauf zeigt der Lexer auf Bytes des `expanded`-
//! Strings. Diagnostiken muessen aber auf die Original-Quelldatei
//! zeigen (eine Zeile aus `inc.idl` ist im expanded-Output an einer
//! voellig anderen Position).
//!
//! [`SourceMap`] sammelt Segmente: jedes Segment ist ein Output-
//! Bereich `[output_start, output_start + length)`, der aus einer
//! Datei `file_id` ab `original_offset` stammt. `lookup(output_pos)`
//! findet das passende Segment und rechnet die ursprungsgetreue
//! Position aus.

/// Stable Identifier fuer eine Datei (lokal innerhalb einer
/// SourceMap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Originale Quell-Position nach Lookup.
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
    /// Konstruiert eine leere SourceMap.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert eine Datei und liefert ihre stabile [`FileId`].
    pub fn add_file(&mut self, name: &str) -> FileId {
        if let Some(idx) = self.files.iter().position(|f| f == name) {
            return FileId(idx as u32);
        }
        let id = FileId(self.files.len() as u32);
        self.files.push(name.to_string());
        id
    }

    /// Schreibt ein Segment ein. `output_start` und `length` beziehen
    /// sich auf den expanded-String; `original_offset` zeigt auf die
    /// erste Byte-Position in der `file_id`-Source.
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

    /// Loest eine expanded-Position auf die Original-(Datei,Offset).
    /// Liefert `None`, wenn die Position nicht in einem registrierten
    /// Segment liegt (z.B. zwischen zwei Segmenten oder hinter dem
    /// letzten).
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

    /// Datei-Name fuer eine [`FileId`].
    #[must_use]
    pub fn file_name(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0 as usize).map(String::as_str)
    }

    /// Anzahl registrierter Segmente.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Anzahl registrierter Dateien.
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
