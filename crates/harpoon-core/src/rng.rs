//! Deterministic xoshiro256** RNG with split-able child streams.
//!
//! The simulation owns one master stream; subsystems derive independent child
//! streams keyed by `(tick, entity, purpose)` so that command timing or scan
//! order can never perturb another subsystem's random sequence.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let mut s = [0u64; 4];
        for slot in s.iter_mut() {
            *slot = splitmix64(&mut sm);
        }
        Self { s }
    }

    /// Derive an independent child stream from a tag without advancing the
    /// parent (keeps `scope_rng` order-independent).
    pub fn derive(&self, tag: u64) -> Rng {
        let mixed = self
            .s
            .iter()
            .fold(tag.wrapping_mul(0xA076_1D64_78BD_642F), |acc, x| {
                (acc ^ *x).wrapping_mul(0x100_0000_01b3)
            });
        let mut sm = mixed;
        let mut s = [0u64; 4];
        for slot in s.iter_mut() {
            *slot = splitmix64(&mut sm);
        }
        Rng { s }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1]
            .wrapping_mul(5)
            .rotate_left(7)
            .wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform float in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform float in [a, b).
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next_f64()
    }

    /// Bernoulli trial.
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p.clamp(0.0, 1.0)
    }

    /// Roughly-normal variate in [a, b) via 3-uniform sum (Irwin–Hall).
    pub fn range_smooth(&mut self, a: f64, b: f64) -> f64 {
        let u = (self.next_f64() + self.next_f64() + self.next_f64()) / 3.0;
        a + (b - a) * u
    }

    /// True heading (0..360) spread around `center` by a normal-ish cone.
    pub fn cone_deg(&mut self, center: f64, spread_deg: f64) -> f64 {
        crate::geom::normalize_deg(center + self.range(-spread_deg, spread_deg))
    }

    pub fn hash_tag(tick: i64, entity: u32, purpose: &str) -> u64 {
        // FNV-1a over purpose mixed with tick/entity.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in purpose.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        h ^= (tick as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= entity as u64;
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn derived_streams_independent() {
        let mut m = Rng::new(7);
        let mut c1 = m.derive(1);
        let mut c2 = m.derive(2);
        let v1: Vec<u64> = (0..8).map(|_| c1.next_u64()).collect();
        let v2: Vec<u64> = (0..8).map(|_| c2.next_u64()).collect();
        assert_ne!(v1, v2);
    }

    #[test]
    fn stats_sane() {
        let mut r = Rng::new(99);
        let n = 200_000;
        let sum: f64 = (0..n).map(|_| r.next_f64()).sum();
        let mean = sum / n as f64;
        assert!(mean > 0.49 && mean < 0.51, "mean {mean}");
        let hits = (0..n).filter(|_| r.chance(0.3)).count();
        let rate = hits as f64 / n as f64;
        assert!(rate > 0.29 && rate < 0.31, "rate {rate}");
    }
}
