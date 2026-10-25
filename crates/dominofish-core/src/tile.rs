//! Tiles in Capi's order (reducer.ts:27-35): index 0 is 0-0, then a = 0..6,
//! b = a..6, so index 27 is 6-6. A set of tiles is a 28-bit mask over these
//! indices.

use core::fmt;

pub const TILE_COUNT: usize = 28;

/// Every tile.
pub const ALL: u32 = (1 << TILE_COUNT) - 1;

const fn index_of(a: u8, b: u8) -> u8 {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    a * 7 - a * (a.saturating_sub(1)) / 2 + (b - a)
}

const PAIRS: [(u8, u8); TILE_COUNT] = {
    let mut out = [(0, 0); TILE_COUNT];
    let mut i = 0;
    let mut a = 0;
    while a <= 6 {
        let mut b = a;
        while b <= 6 {
            out[i] = (a, b);
            i += 1;
            b += 1;
        }
        a += 1;
    }
    out
};

/// SUIT[k] holds the seven tiles that show pip k.
pub const SUIT: [u32; 7] = {
    let mut out = [0u32; 7];
    let mut i = 0;
    while i < TILE_COUNT {
        let (a, b) = PAIRS[i];
        out[a as usize] |= 1 << i;
        out[b as usize] |= 1 << i;
        i += 1;
    }
    out
};

/// One domino tile, stored as its index in Capi's order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Tile(u8);

impl Tile {
    /// The tile with pips a and b, in either order.
    pub const fn new(a: u8, b: u8) -> Tile {
        assert!(a <= 6 && b <= 6);
        Tile(index_of(a, b))
    }

    pub const fn from_index(i: u8) -> Tile {
        assert!((i as usize) < TILE_COUNT);
        Tile(i)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn bit(self) -> u32 {
        1 << self.0
    }

    pub const fn low(self) -> u8 {
        PAIRS[self.0 as usize].0
    }

    pub const fn high(self) -> u8 {
        PAIRS[self.0 as usize].1
    }

    pub const fn pips(self) -> u32 {
        (self.low() + self.high()) as u32
    }

    pub const fn is_double(self) -> bool {
        self.low() == self.high()
    }

    pub const fn has(self, pip: u8) -> bool {
        self.low() == pip || self.high() == pip
    }

    /// The pip on the far side when this tile is joined to `pip`.
    pub const fn other(self, pip: u8) -> u8 {
        if self.low() == pip {
            self.high()
        } else {
            self.low()
        }
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.low(), self.high())
    }
}

/// The tiles of a mask, lowest index first.
pub fn tiles(mut mask: u32) -> impl Iterator<Item = Tile> {
    core::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let i = mask.trailing_zeros() as u8;
        mask &= mask - 1;
        Some(Tile(i))
    })
}

/// Total pips of the tiles in a mask.
pub fn mask_pips(mask: u32) -> u32 {
    tiles(mask).map(Tile::pips).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_follows_capi_order() {
        let mut i = 0;
        for a in 0..=6u8 {
            for b in a..=6u8 {
                let t = Tile::new(a, b);
                assert_eq!(t.index(), i);
                assert_eq!(Tile::new(b, a), t);
                assert_eq!((t.low(), t.high()), (a, b));
                i += 1;
            }
        }
        assert_eq!(i, TILE_COUNT);
    }

    #[test]
    fn suits_have_seven_tiles_and_168_pips_in_total() {
        for (k, suit) in SUIT.iter().enumerate() {
            assert_eq!(suit.count_ones(), 7);
            assert!(tiles(*suit).all(|t| t.has(k as u8)));
        }
        assert_eq!(mask_pips(ALL), 168);
    }
}
