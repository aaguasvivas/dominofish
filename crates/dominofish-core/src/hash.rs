//! Zobrist keys for `State::hash`. Keys for tile places (each hand, the
//! board) are XORed into a running value as tiles move; the scalar fields
//! are looked up at each call. Mode and target are constant within a game and
//! are not hashed.

pub(crate) struct Keys {
    pub hand: [[u64; 28]; 4],
    pub board: [u64; 28],
    pub left: [u64; 7],
    pub right: [u64; 7],
    pub turn: [u64; 4],
    pub starter: [u64; 4],
    /// Index 4 is "no seat".
    pub last: [u64; 5],
    pub passes: [u64; 4],
    pub pile: [u64; 15],
    pub score: [[u64; 512]; 2],
    pub phase: [u64; 3],
    /// Index 4 is "no seat".
    pub winner: [u64; 5],
}

const fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

macro_rules! fill {
    ($arr:expr, $s:ident) => {{
        let mut i = 0;
        while i < $arr.len() {
            $arr[i] = next(&mut $s);
            i += 1;
        }
    }};
}

pub(crate) const KEYS: Keys = {
    let mut s = 0x0D0F_15C0_0000_0001u64;
    let mut k = Keys {
        hand: [[0; 28]; 4],
        board: [0; 28],
        left: [0; 7],
        right: [0; 7],
        turn: [0; 4],
        starter: [0; 4],
        last: [0; 5],
        passes: [0; 4],
        pile: [0; 15],
        score: [[0; 512]; 2],
        phase: [0; 3],
        winner: [0; 5],
    };
    fill!(k.hand[0], s);
    fill!(k.hand[1], s);
    fill!(k.hand[2], s);
    fill!(k.hand[3], s);
    fill!(k.board, s);
    fill!(k.left, s);
    fill!(k.right, s);
    fill!(k.turn, s);
    fill!(k.starter, s);
    fill!(k.last, s);
    fill!(k.passes, s);
    fill!(k.pile, s);
    fill!(k.score[0], s);
    fill!(k.score[1], s);
    fill!(k.phase, s);
    fill!(k.winner, s);
    k
};
