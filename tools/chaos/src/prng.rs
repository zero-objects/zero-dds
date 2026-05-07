// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! xorshift64 — minimal seedable PRNG fuer reproducible Chaos.

/// xorshift64-PRNG. Period 2^64-1, ausreichend fuer Test-Workloads.
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    /// Erzeugt einen Generator mit `seed`. Seed=0 wird auf 1 angehoben
    /// (xorshift mit 0 ist degenerate).
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Liefert das naechste `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Bernoulli-Sample mit Wahrscheinlichkeit `p` ∈ [0, 1].
    pub fn bernoulli(&mut self, p: f64) -> bool {
        if p <= 0.0 {
            return false;
        }
        if p >= 1.0 {
            return true;
        }
        // u64 → [0,1): mantisse-only, vermeidet Bias bei naive
        // f64-Cast (53-bit-mantisse).
        let bits = self.next_u64() >> 11;
        (bits as f64) / ((1u64 << 53) as f64) < p
    }

    /// Uniforme `u64` in [0, n). `n=0` returnt 0.
    pub fn range_u64(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        // Lemire's debiased range — fast genug fuer Test-Tool.
        let r = self.next_u64();
        ((r as u128 * n as u128) >> 64) as u64
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn deterministic_with_same_seed() {
        let mut a = Xorshift64::new(42);
        let mut b = Xorshift64::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn bernoulli_extremes() {
        let mut g = Xorshift64::new(1);
        assert!(!g.bernoulli(0.0));
        assert!(g.bernoulli(1.0));
        assert!(!g.bernoulli(-0.5));
        assert!(g.bernoulli(2.0));
    }

    #[test]
    fn bernoulli_distribution_close_to_p() {
        let mut g = Xorshift64::new(7);
        let n = 100_000;
        let hits: usize = (0..n).filter(|_| g.bernoulli(0.3)).count();
        let p = hits as f64 / n as f64;
        // 3-sigma window for n=100k, p=0.3 ≈ ±0.006
        assert!((0.29..0.31).contains(&p), "p={p}");
    }

    #[test]
    fn range_zero_is_zero() {
        let mut g = Xorshift64::new(1);
        assert_eq!(g.range_u64(0), 0);
    }

    #[test]
    fn range_within_bounds() {
        let mut g = Xorshift64::new(1);
        for _ in 0..1000 {
            let v = g.range_u64(100);
            assert!(v < 100);
        }
    }
}
