// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 Package Quality Categories — REP-2004.

/// REP-2004 — Package Quality Levels.
///
/// Spec quote: "ROS 2 packages are sorted into quality levels using a
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
    /// Numeric level (1-5; lower = higher quality).
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

    /// Constructs from a numeric level.
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

    /// REP-2004 §"Package Quality Categories": Quality Level 1 to 5,
    /// level 1 the highest; no other level.
    #[test]
    fn rep2004_the_quality_levels_are_1_to_5_with_1_the_highest() {
        for (level, number) in [
            (QualityLevel::Q1, 1),
            (QualityLevel::Q2, 2),
            (QualityLevel::Q3, 3),
            (QualityLevel::Q4, 4),
            (QualityLevel::Q5, 5),
        ] {
            assert_eq!(level.numeric(), number, "{level:?}");
            assert_eq!(QualityLevel::from_numeric(number), Some(level));
        }
        assert!(QualityLevel::Q1 > QualityLevel::Q2);
        assert!(QualityLevel::Q4 > QualityLevel::Q5);
        assert_eq!(QualityLevel::from_numeric(0), None);
        assert_eq!(QualityLevel::from_numeric(6), None);
    }
}
