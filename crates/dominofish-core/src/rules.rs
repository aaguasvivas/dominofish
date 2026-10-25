//! Round and game rules as Capi's engine implements them at the pinned commit
//! (packages/engine/src). docs/RULES.md states each rule (R1 to R28) with its
//! Capi file:line; parity with Capi is checked by the M1 features, never by
//! this file's comments.

use crate::hash::KEYS;
use crate::rng::Rng;
use crate::tile::{ALL, SUIT, TILE_COUNT, Tile, mask_pips, tiles};

/// Points of each mid-round bonus and of capicua (R18, R23 to R25).
pub const BONUS: u16 = 25;

/// Marks "no seat" in `State::last_played_by` and `State::round_winner`.
pub const NO_SEAT: u8 = 4;

/// The highest target the score hash supports; Capi offers 100 or 200.
pub const MAX_TARGET: u16 = 300;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    OneVsOne,
    TwoVsTwo,
}

impl Mode {
    pub const fn seats(self) -> u8 {
        match self {
            Mode::OneVsOne => 2,
            Mode::TwoVsTwo => 4,
        }
    }

    /// The next seat in turn order (R7).
    pub const fn next(self, seat: u8) -> u8 {
        (seat + 1) % self.seats()
    }

    pub const fn seat_name(self, seat: u8) -> &'static str {
        match (self, seat) {
            (_, 0) => "n",
            (Mode::OneVsOne, 1) => "s",
            (Mode::TwoVsTwo, 1) => "e",
            (Mode::TwoVsTwo, 2) => "s",
            (Mode::TwoVsTwo, 3) => "w",
            _ => "?",
        }
    }

    pub const fn team_name(self, team: usize) -> &'static str {
        match (self, team) {
            (Mode::OneVsOne, 0) => "n",
            (Mode::OneVsOne, _) => "s",
            (Mode::TwoVsTwo, 0) => "ns",
            (Mode::TwoVsTwo, _) => "ew",
        }
    }

    const fn boneyard_len(self) -> usize {
        match self {
            Mode::OneVsOne => 14,
            Mode::TwoVsTwo => 0,
        }
    }
}

/// The team of a seat (R3). Seat indices make both modes `seat % 2`.
pub const fn team(seat: u8) -> usize {
    (seat % 2) as usize
}

/// A deal in Capi's layout (R3): seven tiles per seat in seat order, then the
/// 1v1 boneyard, whose last tile is drawn first (R14).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Deal {
    tiles: [Tile; TILE_COUNT],
}

impl Deal {
    /// A uniformly shuffled deal from DominoFish's own RNG (R2: parity tests
    /// inject Capi's deals instead).
    pub fn shuffled(rng: &mut Rng) -> Deal {
        let mut tiles = core::array::from_fn(|i| Tile::from_index(i as u8));
        for i in (1..TILE_COUNT).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            tiles.swap(i, j);
        }
        Deal { tiles }
    }

    /// A deal from a Capi-ordered tile list; None unless it holds every tile
    /// exactly once.
    pub fn from_tiles(tiles: [Tile; TILE_COUNT]) -> Option<Deal> {
        let seen = tiles.iter().fold(0u32, |m, t| m | t.bit());
        (seen == ALL).then_some(Deal { tiles })
    }

    pub fn tiles(&self) -> &[Tile; TILE_COUNT] {
        &self.tiles
    }

    pub fn hand(&self, seat: u8) -> &[Tile] {
        let start = 7 * seat as usize;
        &self.tiles[start..start + 7]
    }

    pub fn boneyard(&self, mode: Mode) -> &[Tile] {
        &self.tiles[TILE_COUNT - mode.boneyard_len()..]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    Left,
    Right,
}

/// A move. On an empty board `end` sets the orientation (R10): Left places
/// the tile low pip left, as it sits in a hand; Right places it flipped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    Play { tile: Tile, end: End },
    Draw,
    Pass,
}

/// What a move caused. Capi's callouts map to these only in the parity
/// harness (docs/RULES.md, L4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// R24.
    PaseSalida { team: usize },
    /// R23.
    PaseCorrido { team: usize },
    /// R17: `points` is every pip left in every hand.
    Domino { seat: u8, points: u16 },
    /// R18: a domino whose last tile fit both ends; `points` includes the
    /// bonus.
    Capicua { seat: u8, points: u16 },
    /// R19.
    Tranque {
        blocker: u8,
        rival: u8,
        blocker_pips: u16,
        rival_pips: u16,
        winner: u8,
        points: u16,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Playing,
    RoundOver,
    Finished,
}

/// Up to 16 moves, enough for any position (at most 12 distinct plays).
#[derive(Clone, Copy, Debug)]
pub struct MoveList {
    moves: [Move; 16],
    len: u8,
}

impl MoveList {
    pub const fn new() -> MoveList {
        MoveList {
            moves: [Move::Pass; 16],
            len: 0,
        }
    }

    fn push(&mut self, mv: Move) {
        self.moves[self.len as usize] = mv;
        self.len += 1;
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for MoveList {
    fn default() -> MoveList {
        MoveList::new()
    }
}

/// What `State::undo` needs to reverse one move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Undo {
    left: u8,
    right: u8,
    turn: u8,
    last_played_by: u8,
    passes: u8,
    boneyard_len: u8,
    phase: Phase,
    round_winner: u8,
    scores: [u16; 2],
}

/// The result of `State::play`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Played {
    pub event: Option<Event>,
    pub undo: Undo,
}

/// A full game state, every hand included. Only the referee, the simulator
/// inside sampled worlds, and the omniscient bot may hold one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State {
    pub mode: Mode,
    pub target: u16,
    pub scores: [u16; 2],
    pub hands: [u32; 4],
    /// Tiles on the board.
    pub board: u32,
    /// Open ends; meaningful only while `board` is not empty.
    pub left: u8,
    pub right: u8,
    boneyard: [Tile; 14],
    boneyard_len: u8,
    pub turn: u8,
    /// The round's opener (la mano, starterThisRound).
    pub starter: u8,
    pub last_played_by: u8,
    /// Passes since the last play (one counter, L5).
    pub passes: u8,
    pub phase: Phase,
    /// The seat that leads the next round, set when a round ends (R6, R27).
    pub round_winner: u8,
    /// XOR of the Zobrist keys of every tile's place; see `hash`.
    tiles_hash: u64,
}

impl State {
    /// Round 1 (R4, R5). Returns the state and the automatic tile.
    pub fn first_round(mode: Mode, target: u16, deal: &Deal) -> (State, Tile) {
        let mut opener = 0;
        let mut opening: Option<Tile> = None;
        for seat in 0..mode.seats() {
            let double = (0..=6u8)
                .rev()
                .map(|d| Tile::new(d, d))
                .find(|d| deal.hand(seat).contains(d));
            if let Some(d) = double
                && opening.is_none_or(|o| d.low() > o.low())
            {
                opener = seat;
                opening = Some(d);
            }
        }
        if opening.is_none() {
            let mut best_sum = None;
            for seat in 0..mode.seats() {
                let mut best: Option<Tile> = None;
                for &t in deal.hand(seat) {
                    if best.is_none_or(|b| t.pips() > b.pips()) {
                        best = Some(t);
                    }
                }
                let t = best.expect("seven tiles per hand");
                if best_sum.is_none_or(|s| t.pips() > s) {
                    opener = seat;
                    opening = Some(t);
                    best_sum = Some(t.pips());
                }
            }
        }
        let tile = opening.expect("every hand has a tile");
        let mut state = State::dealt(mode, target, [0, 0], deal, opener);
        state.move_tile(tile, opener as usize);
        state.left = tile.low();
        state.right = tile.high();
        state.turn = mode.next(opener);
        state.last_played_by = opener;
        (state, tile)
    }

    /// The next round (R6): a fresh deal, an empty board, and the previous
    /// round's winning seat to lead; scores carry.
    pub fn next_round(&self, deal: &Deal) -> State {
        assert_eq!(self.phase, Phase::RoundOver);
        State::dealt(self.mode, self.target, self.scores, deal, self.round_winner)
    }

    /// A round dealt from `deal` with `leader` to lead on an empty board and
    /// the given scores carried in.
    pub fn dealt(mode: Mode, target: u16, scores: [u16; 2], deal: &Deal, leader: u8) -> State {
        assert!(target <= MAX_TARGET && scores.iter().all(|&s| s < target));
        let mut hands = [0u32; 4];
        let mut tiles_hash = 0;
        for (seat, hand) in hands.iter_mut().enumerate().take(mode.seats() as usize) {
            for t in deal.hand(seat as u8) {
                *hand |= t.bit();
                tiles_hash ^= KEYS.hand[seat][t.index()];
            }
        }
        let mut boneyard = [Tile::from_index(0); 14];
        let pile = deal.boneyard(mode);
        boneyard[..pile.len()].copy_from_slice(pile);
        State {
            mode,
            target,
            scores,
            hands,
            board: 0,
            left: 0,
            right: 0,
            boneyard,
            boneyard_len: pile.len() as u8,
            turn: leader,
            starter: leader,
            last_played_by: NO_SEAT,
            passes: 0,
            phase: Phase::Playing,
            round_winner: NO_SEAT,
            tiles_hash,
        }
    }

    /// The boneyard in order; the last tile is drawn first.
    pub fn boneyard(&self) -> &[Tile] {
        &self.boneyard[..self.boneyard_len as usize]
    }

    /// The canonical legal moves of the seat on turn (docs/RULES.md, Moves).
    pub fn legal_moves(&self, out: &mut MoveList) {
        out.len = 0;
        if self.phase != Phase::Playing {
            return;
        }
        let hand = self.hands[self.turn as usize];
        if self.board == 0 {
            for tile in tiles(hand) {
                out.push(Move::Play {
                    tile,
                    end: End::Left,
                });
            }
            return;
        }
        let fit_left = hand & SUIT[self.left as usize];
        let fit_right = if self.left == self.right {
            0
        } else {
            hand & SUIT[self.right as usize]
        };
        for tile in tiles(fit_left | fit_right) {
            if fit_left & tile.bit() != 0 {
                out.push(Move::Play {
                    tile,
                    end: End::Left,
                });
            }
            if fit_right & tile.bit() != 0 {
                out.push(Move::Play {
                    tile,
                    end: End::Right,
                });
            }
        }
        if out.is_empty() {
            out.push(if self.boneyard_len > 0 {
                Move::Draw
            } else {
                Move::Pass
            });
        }
    }

    /// Whether Capi accepts this move from the seat on turn (R8 to R16),
    /// including the non-canonical ones: a flipped lead and the right end
    /// when both ends show one pip.
    pub fn is_legal(&self, mv: Move) -> bool {
        if self.phase != Phase::Playing {
            return false;
        }
        let hand = self.hands[self.turn as usize];
        match mv {
            Move::Play { tile, end } => {
                hand & tile.bit() != 0
                    && (self.board == 0
                        || tile.has(match end {
                            End::Left => self.left,
                            End::Right => self.right,
                        }))
            }
            Move::Draw => self.boneyard_len > 0 && !self.fits(hand),
            Move::Pass => self.boneyard_len == 0 && !self.fits(hand),
        }
    }

    /// The canonical move that stands for `mv` (docs/RULES.md, Capi intents
    /// to moves): the right end becomes the left one on an empty board and
    /// when both ends show one pip.
    pub fn canonical(&self, mv: Move) -> Move {
        match mv {
            Move::Play {
                tile,
                end: End::Right,
            } if self.board == 0 || self.left == self.right => Move::Play {
                tile,
                end: End::Left,
            },
            other => other,
        }
    }

    fn fits(&self, mask: u32) -> bool {
        self.board == 0 || mask & (SUIT[self.left as usize] | SUIT[self.right as usize]) != 0
    }

    fn all_hand_pips(&self) -> u16 {
        mask_pips(self.hands.iter().fold(0, |m, h| m | h)) as u16
    }

    /// Moves a tile from a seat's hand onto the board, keeping the tile hash.
    fn move_tile(&mut self, tile: Tile, seat: usize) {
        self.hands[seat] &= !tile.bit();
        self.board |= tile.bit();
        self.tiles_hash ^= KEYS.hand[seat][tile.index()] ^ KEYS.board[tile.index()];
    }

    /// R21, without the legacy branches (L2).
    fn credit(&mut self, winner: u8, points: u16) {
        let t = team(winner);
        self.scores[t] += points;
        self.round_winner = winner;
        self.phase = if self.scores[t] >= self.target {
            Phase::Finished
        } else {
            Phase::RoundOver
        };
    }

    /// R25: a mid-round bonus pays only while it leaves the side strictly
    /// below the target.
    fn pay_bonus(&mut self, team: usize) -> bool {
        if self.scores[team] + BONUS < self.target {
            self.scores[team] += BONUS;
            true
        } else {
            false
        }
    }

    /// Applies a legal move (`is_legal`) of the seat on turn.
    pub fn play(&mut self, mv: Move) -> Played {
        debug_assert!(self.is_legal(mv), "illegal move {mv:?}");
        let undo = Undo {
            left: self.left,
            right: self.right,
            turn: self.turn,
            last_played_by: self.last_played_by,
            passes: self.passes,
            boneyard_len: self.boneyard_len,
            phase: self.phase,
            round_winner: self.round_winner,
            scores: self.scores,
        };
        let seat = self.turn;
        let event = match mv {
            Move::Play { tile, end } => self.place(seat, tile, end),
            Move::Draw => {
                // R14.
                let s = seat as usize;
                while self.boneyard_len > 0 {
                    self.boneyard_len -= 1;
                    let drawn = self.boneyard[self.boneyard_len as usize];
                    self.hands[s] |= drawn.bit();
                    self.tiles_hash ^= KEYS.hand[s][drawn.index()];
                    if self.fits(drawn.bit()) {
                        break;
                    }
                }
                None
            }
            Move::Pass => {
                // R16, R23.
                self.passes += 1;
                let next = self.mode.next(seat);
                self.turn = next;
                if self.mode == Mode::TwoVsTwo
                    && self.last_played_by != NO_SEAT
                    && next == self.last_played_by
                    && self.passes == 3
                {
                    let t = team(self.last_played_by);
                    self.pay_bonus(t).then_some(Event::PaseCorrido { team: t })
                } else {
                    None
                }
            }
        };
        Played { event, undo }
    }

    /// Reverses `play(mv)`, given the `Undo` it returned.
    pub fn undo(&mut self, mv: Move, undo: Undo) {
        let seat = undo.turn as usize;
        match mv {
            Move::Play { tile, .. } => {
                self.hands[seat] |= tile.bit();
                self.board &= !tile.bit();
                self.tiles_hash ^= KEYS.hand[seat][tile.index()] ^ KEYS.board[tile.index()];
            }
            Move::Draw => {
                for i in self.boneyard_len..undo.boneyard_len {
                    let t = self.boneyard[i as usize];
                    self.hands[seat] &= !t.bit();
                    self.tiles_hash ^= KEYS.hand[seat][t.index()];
                }
            }
            Move::Pass => {}
        }
        self.left = undo.left;
        self.right = undo.right;
        self.turn = undo.turn;
        self.last_played_by = undo.last_played_by;
        self.passes = undo.passes;
        self.boneyard_len = undo.boneyard_len;
        self.phase = undo.phase;
        self.round_winner = undo.round_winner;
        self.scores = undo.scores;
    }

    fn place(&mut self, seat: u8, tile: Tile, end: End) -> Option<Event> {
        let s = seat as usize;
        let was_empty = self.board == 0;
        let lone_tile = self.board.count_ones() == 1;
        let (left_before, right_before) = (self.left, self.right);
        let played_before = self.last_played_by;
        let passes_before = self.passes;

        self.move_tile(tile, s);
        match (was_empty, end) {
            (true, End::Left) => (self.left, self.right) = (tile.low(), tile.high()),
            (true, End::Right) => (self.left, self.right) = (tile.high(), tile.low()),
            (false, End::Left) => self.left = tile.other(self.left),
            (false, End::Right) => self.right = tile.other(self.right),
        }
        self.last_played_by = seat;

        // R17 (checked before the lock), R18.
        if self.hands[s] == 0 {
            let mut points = self.all_hand_pips();
            let capicua = !was_empty && tile.has(left_before) && tile.has(right_before);
            if capicua {
                points += BONUS;
            }
            self.credit(seat, points);
            return Some(if capicua {
                Event::Capicua { seat, points }
            } else {
                Event::Domino { seat, points }
            });
        }

        // R19: no tile outside the board shows either end (docs/PLAN.md, D8).
        let open = SUIT[self.left as usize] | SUIT[self.right as usize];
        if ALL & !self.board & open == 0 {
            let rival = self.mode.next(seat);
            let blocker_pips = mask_pips(self.hands[s]) as u16;
            let rival_pips = mask_pips(self.hands[rival as usize]) as u16;
            let winner = if blocker_pips < rival_pips {
                seat
            } else if rival_pips < blocker_pips {
                rival
            } else {
                self.starter
            };
            let points = self.all_hand_pips();
            self.credit(winner, points);
            return Some(Event::Tranque {
                blocker: seat,
                rival,
                blocker_pips,
                rival_pips,
                winner,
                points,
            });
        }

        self.turn = self.mode.next(seat);
        self.passes = 0;

        // R24.
        if self.mode == Mode::TwoVsTwo
            && lone_tile
            && played_before == self.starter
            && passes_before == 1
            && seat == self.mode.next(self.mode.next(self.starter))
        {
            let t = team(self.starter);
            if self.pay_bonus(t) {
                return Some(Event::PaseSalida { team: t });
            }
        }
        None
    }

    /// A 64-bit Zobrist hash of everything a later move or score depends on:
    /// every tile's place, the ends, turn, opener, last player, passes,
    /// boneyard position, scores, phase, and the next leader.
    pub fn hash(&self) -> u64 {
        self.tiles_hash ^ self.scalar_hash()
    }

    /// The same hash computed from scratch, for tests.
    pub fn recomputed_hash(&self) -> u64 {
        let mut h = 0;
        for seat in 0..self.mode.seats() as usize {
            for t in tiles(self.hands[seat]) {
                h ^= KEYS.hand[seat][t.index()];
            }
        }
        for t in tiles(self.board) {
            h ^= KEYS.board[t.index()];
        }
        h ^ self.scalar_hash()
    }

    fn scalar_hash(&self) -> u64 {
        let mut h = KEYS.turn[self.turn as usize]
            ^ KEYS.starter[self.starter as usize]
            ^ KEYS.last[self.last_played_by as usize]
            ^ KEYS.passes[self.passes as usize]
            ^ KEYS.pile[self.boneyard_len as usize]
            ^ KEYS.score[0][self.scores[0] as usize]
            ^ KEYS.score[1][self.scores[1] as usize]
            ^ KEYS.phase[self.phase as usize]
            ^ KEYS.winner[self.round_winner as usize];
        if self.board != 0 {
            h ^= KEYS.left[self.left as usize] ^ KEYS.right[self.right as usize];
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tile is in exactly one place.
    fn assert_conserved(state: &State) {
        let pile = state.boneyard().iter().fold(0, |m, t| m | t.bit());
        let mut seen = state.board | pile;
        let mut count = state.board.count_ones() + pile.count_ones();
        for h in &state.hands {
            assert_eq!(seen & h, 0);
            seen |= h;
            count += h.count_ones();
        }
        assert_eq!((seen, count), (ALL, 28));
    }

    /// The raw moves Capi accepts here: the canonical ones plus a flipped
    /// lead and the right end on equal ends.
    fn raw_moves(state: &State, canonical: &MoveList, out: &mut [Move; 32]) -> usize {
        let mut n = 0;
        for &mv in canonical.as_slice() {
            out[n] = mv;
            n += 1;
            if let Move::Play {
                tile,
                end: End::Left,
            } = mv
            {
                let right = Move::Play {
                    tile,
                    end: End::Right,
                };
                if (state.board == 0 && !tile.is_double()) || state.left == state.right {
                    out[n] = right;
                    n += 1;
                }
            }
        }
        n
    }

    /// Plays raw moves at random to the end of a game. After each move: tile
    /// conservation, the incremental hash equals the recomputed one, and undo
    /// restores the previous state exactly. Returns the final state and the
    /// number of moves.
    pub(crate) fn random_game_checked(mode: Mode, seed: u64) -> (State, u64) {
        let mut rng = Rng::new(seed);
        let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
        let mut moves = MoveList::new();
        let mut raw = [Move::Pass; 32];
        let mut count = 0;
        loop {
            match state.phase {
                Phase::Finished => return (state, count),
                Phase::RoundOver => state = state.next_round(&Deal::shuffled(&mut rng)),
                Phase::Playing => {
                    state.legal_moves(&mut moves);
                    let n = raw_moves(&state, &moves, &mut raw);
                    for &mv in &raw[..n] {
                        assert!(state.is_legal(mv), "{mv:?}");
                        assert!(moves.as_slice().contains(&state.canonical(mv)));
                    }
                    let mv = raw[rng.below(n as u32) as usize];
                    let before = state;
                    let played = state.play(mv);
                    count += 1;
                    assert_conserved(&state);
                    assert_eq!(state.hash(), state.recomputed_hash(), "seed {seed}");
                    let mut back = state;
                    back.undo(mv, played.undo);
                    assert_eq!(back, before, "seed {seed} undo of {mv:?}");
                    assert_eq!(back.hash(), before.hash());
                }
            }
        }
    }

    #[test]
    fn play_undo_round_trip_and_hash_in_both_modes() {
        let (mut games, mut moves) = (0, 0);
        for seed in 0..1000 {
            for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
                let (state, n) = random_game_checked(mode, seed);
                assert!(state.scores.iter().any(|&s| s >= 100));
                games += 1;
                moves += n;
            }
        }
        println!("play-undo: games {games} moves {moves} ok");
    }

    #[test]
    fn opener_holds_six_six_in_2v2() {
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let deal = Deal::shuffled(&mut rng);
            let (state, tile) = State::first_round(Mode::TwoVsTwo, 100, &deal);
            assert_eq!(tile, Tile::new(6, 6));
            assert!(deal.hand(state.starter).contains(&tile));
        }
    }

    #[test]
    fn from_tiles_rejects_a_repeated_tile() {
        let mut tiles = *Deal::shuffled(&mut Rng::new(1)).tiles();
        assert!(Deal::from_tiles(tiles).is_some());
        tiles[0] = tiles[1];
        assert!(Deal::from_tiles(tiles).is_none());
    }
}
