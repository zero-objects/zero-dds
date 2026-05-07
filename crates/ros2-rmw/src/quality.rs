// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 Package Quality Categories — REP-2004.

/// REP-2004 — Package Quality Levels.
///
/// Spec-Zitat: "ROS 2 packages are sorted into quality levels using a
/// formal, well-defined system. The category each package falls into
/// depends on the development practices that have been used to
/// produce that package."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QualityLevel {
    /// `Quality Level 5` — Demo / Tutorial / Toy.
    Q5,
    /// `Quality Level 4` — Mature stand-alone package, not yet
    /// guaranteed to be part of a release.
    Q4,
    /// `Quality Level 3` — Released package, with some level of
    /// review.
    Q3,
    /// `Quality Level 2` — Released package with documented quality
    /// practices.
    Q2,
    /// `Quality Level 1` — Released package with the highest level
    /// of quality practices, peer-reviewed, fully tested, etc.
    Q1,
}

impl QualityLevel {
    /// Numerischer Level (1-5; lower = higher quality).
    #[must_use]
    pub const fn numeric(self) -> u8 {
        match self {
            Self::Q1 => 1,
            Self::Q2 => 2,
            Self::Q3 => 3,
            Self::Q4 => 4,
            Self::Q5 => 5,
        }
    }

    /// Konstruiert aus numerischem Level.
    #[must_use]
    pub const fn from_numeric(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Q1),
            2 => Some(Self::Q2),
            3 => Some(Self::Q3),
            4 => Some(Self::Q4),
            5 => Some(Self::Q5),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_level_numeric_round_trip() {
        for q in [
            QualityLevel::Q1,
            QualityLevel::Q2,
            QualityLevel::Q3,
            QualityLevel::Q4,
            QualityLevel::Q5,
        ] {
            assert_eq!(QualityLevel::from_numeric(q.numeric()), Some(q));
        }
    }

    #[test]
    fn quality_level_ordering_q1_is_highest() {
        // Q1 < Q5 in der Enum-Ordering ist falsch, weil wir vom
        // niedrigeren-Level zum hoeheren ordnen. Aber numerically
        // Q1=1 < Q5=5.
        assert!(QualityLevel::Q1.numeric() < QualityLevel::Q5.numeric());
    }

    #[test]
    fn quality_level_from_numeric_rejects_out_of_range() {
        assert_eq!(QualityLevel::from_numeric(0), None);
        assert_eq!(QualityLevel::from_numeric(6), None);
    }
}
