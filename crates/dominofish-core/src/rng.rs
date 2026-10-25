//! Seeded randomness. SplitMix64 (Steele, Lea, and Flood 2014) with Lemire's
//! exact bounded integers (Lemire 2019). Written here instead of taken from a
//! crate so that a dependency upgrade can never change a recorded seed's
//! stream (docs/PLAN.md, Dependencies).

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seed for an independent stream, derived from a parent seed and a stream
/// number, so per-round deals do not depend on move choices.
pub fn derive(seed: u64, stream: u64) -> u64 {
    mix(seed.wrapping_add(mix(stream.wrapping_add(GOLDEN))))
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN);
        mix(self.state)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A uniform integer in 0..n, exactly (no modulo bias). n must be > 0.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0);
        let mut m = u64::from(self.next_u32()) * u64::from(n);
        let mut low = m as u32;
        if low < n {
            let threshold = n.wrapping_neg() % n;
            while low < threshold {
                m = u64::from(self.next_u32()) * u64::from(n);
                low = m as u32;
            }
        }
        (m >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_stays_in_range_and_hits_every_value() {
        let mut rng = Rng::new(1);
        let mut seen = [false; 28];
        for _ in 0..10_000 {
            let v = rng.below(28) as usize;
            seen[v] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
