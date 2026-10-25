//! Round and game rules as Capi's engine implements them at the pinned commit
//! (packages/engine/src; docs/RULES.md lists each rule with its file:line).
//! M0 status: written from the digest's rule list and a reading of the code;
//! no parity with Capi is claimed until the M1 checks pass.

use crate::rng::Rng;
use crate::tile::{ALL, SUIT, TILE_COUNT, Tile, mask_pips, tiles};

/// Points of each mid-round bonus and of capicua (scoring.ts:100-104).
pub const BONUS: u16 = 25;

/// Marks "no seat" in `State::last_played_by`.
pub const NO_SEAT: u8 = u8::MAX;

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

    /// The next seat in turn order: n, e, s, w in 2v2 and n, s in 1v1
    /// (types.ts:103-110).
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

/// The team of a seat: n and s are team 0 and e and w team 1 in 2v2; n is
/// team 0 and s team 1 in 1v1 (types.ts:116-119). Seat indices make both
/// cases `seat % 2`.
pub const fn team(seat: u8) -> usize {
    (seat % 2) as usize
}

/// A deal in Capi's layout: seven tiles per seat in seat order, then the
/// boneyard (1v1 only), whose last tile is drawn first (reducer.ts:120-133,
/// 385-394).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Deal {
    pub tiles: [Tile; TILE_COUNT],
}

impl Deal {
    /// A uniformly shuffled deal. Uses DominoFish's own RNG: parity tests
    /// inject Capi's deals instead of reproducing Capi's RNG.
    pub fn shuffled(rng: &mut Rng) -> Deal {
        let mut tiles = core::array::from_fn(|i| Tile::from_index(i as u8));
        for i in (1..TILE_COUNT).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            tiles.swap(i, j);
        }
        Deal { tiles }
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

/// A move. On an empty board `end` sets the orientation instead of a side:
/// Left places the tile low pip left, as dealt (Capi's in-hand orientation),
/// and Right places it flipped. docs/RULES.md defines the canonical moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    Play { tile: Tile, end: End },
    Draw,
    Pass,
}

/// What a move caused, as typed events; Capi's callouts map to these only in
/// the parity harness.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// The opener's partner played on the lone opening tile after one pass.
    PaseSalida { team: usize },
    /// Three passes in a row after a play (2v2).
    PaseCorrido { team: usize },
    /// A seat emptied its hand; `points` is every pip left in every hand.
    Domino { seat: u8, points: u16 },
    /// A domino whose last tile fit both ends; `points` includes the bonus.
    Capicua { seat: u8, points: u16 },
    /// The board locked after a play that did not empty the hand.
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

/// A full game state, every hand included. Only the referee, the simulator
/// inside sampled worlds, and the omniscient bot may hold one.
#[derive(Clone, Copy, Debug)]
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
    /// Passes since the last play (one counter; Capi keeps two identical ones).
    pub passes: u8,
    pub phase: Phase,
    /// The seat that leads the next round, set when a round ends.
    pub round_winner: u8,
}

impl State {
    /// Round 1. The opener is the seat with the highest double (seat order,
    /// strict >); with no double (1v1 only) the highest pip sum, ties to the
    /// earlier seat, then the first such tile in dealt order. That tile is
    /// placed as dealt, low pip left (reducer.ts:90-109, 135-185). Returns the
    /// state and the automatic tile.
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
        state.hands[opener as usize] &= !tile.bit();
        state.board = tile.bit();
        state.left = tile.low();
        state.right = tile.high();
        state.turn = mode.next(opener);
        state.last_played_by = opener;
        (state, tile)
    }

    /// The next round: a fresh deal, an empty board, and the previous round's
    /// winning seat to lead any tile; scores carry (reducer.ts:520-564).
    pub fn next_round(&self, deal: &Deal) -> State {
        assert_eq!(self.phase, Phase::RoundOver);
        State::dealt(self.mode, self.target, self.scores, deal, self.round_winner)
    }

    fn dealt(mode: Mode, target: u16, scores: [u16; 2], deal: &Deal, leader: u8) -> State {
        let mut hands = [0u32; 4];
        for (seat, hand) in hands.iter_mut().enumerate().take(mode.seats() as usize) {
            *hand = deal.hand(seat as u8).iter().fold(0, |m, t| m | t.bit());
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
        }
    }

    pub fn boneyard(&self) -> &[Tile] {
        &self.boneyard[..self.boneyard_len as usize]
    }

    /// The canonical legal moves of the seat on turn: one per tile on an
    /// empty board (as dealt), one per tile and end otherwise, and one end
    /// only when both ends show the same pip. With no play: draw in 1v1 while
    /// the boneyard has tiles, else pass (validate.ts:82-115).
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

    fn fits(&self, mask: u32) -> bool {
        self.board == 0 || mask & (SUIT[self.left as usize] | SUIT[self.right as usize]) != 0
    }

    fn all_hand_pips(&self) -> u16 {
        mask_pips(
            self.hands[..self.mode.seats() as usize]
                .iter()
                .fold(0, |m, h| m | h),
        ) as u16
    }

    /// Only the round-winning side scores; reaching the target finishes the
    /// game (reducer.ts:190-218, without its legacy branches).
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

    /// A mid-round bonus pays only while it leaves the side strictly below
    /// the target (reducer.ts:349, 431).
    fn pay_bonus(&mut self, team: usize) -> bool {
        if self.scores[team] + BONUS < self.target {
            self.scores[team] += BONUS;
            true
        } else {
            false
        }
    }

    /// Applies a legal move of the seat on turn. The move must come from
    /// `legal_moves` or be a raw Capi play that `legal_moves` maps to one.
    pub fn play(&mut self, mv: Move) -> Option<Event> {
        debug_assert_eq!(self.phase, Phase::Playing);
        let seat = self.turn;
        match mv {
            Move::Play { tile, end } => self.place(seat, tile, end),
            Move::Draw => {
                // Pop from the end until a tile fits or the pile is empty; the
                // turn and the pass counter stay (reducer.ts:385-405).
                while self.boneyard_len > 0 {
                    self.boneyard_len -= 1;
                    let drawn = self.boneyard[self.boneyard_len as usize];
                    self.hands[seat as usize] |= drawn.bit();
                    if self.fits(drawn.bit()) {
                        break;
                    }
                }
                None
            }
            Move::Pass => {
                // Pase corrido: the third pass since the last play hands the
                // turn back to the player who made it (2v2 only;
                // reducer.ts:408-458).
                self.passes += 1;
                let next = self.mode.next(seat);
                self.turn = next;
                if self.mode == Mode::TwoVsTwo
                    && self.last_played_by != NO_SEAT
                    && next == self.last_played_by
                    && self.passes == 3
                {
                    let t = team(self.last_played_by);
                    if self.pay_bonus(t) {
                        return Some(Event::PaseCorrido { team: t });
                    }
                }
                None
            }
        }
    }

    fn place(&mut self, seat: u8, tile: Tile, end: End) -> Option<Event> {
        let s = seat as usize;
        let was_empty = self.board == 0;
        let lone_tile = self.board.count_ones() == 1;
        let (left_before, right_before) = (self.left, self.right);
        let played_before = self.last_played_by;
        let passes_before = self.passes;

        self.hands[s] &= !tile.bit();
        self.board |= tile.bit();
        match (was_empty, end) {
            (true, End::Left) => (self.left, self.right) = (tile.low(), tile.high()),
            (true, End::Right) => (self.left, self.right) = (tile.high(), tile.low()),
            (false, End::Left) => self.left = tile.other(self.left),
            (false, End::Right) => self.right = tile.other(self.right),
        }
        self.last_played_by = seat;

        // Going out is a domino even when the same tile locks the board
        // (reducer.ts:306-316); every hand's pips score, the boneyard's never
        // (scoring.ts:35-43). Capicua: the last tile fit both ends as they
        // were before it (scoring.ts:91-98).
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

        // Tranque: no tile outside the board fits either end
        // (validate.ts:45-51). Blocker against the next seat; equal pips go
        // to the round's opener (scoring.ts:63-82).
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

        // Pase de salida: the opener's partner plays on the lone opening tile
        // after the seat between them passed (2v2; reducer.ts:334-373).
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays random canonical moves to the end of a game and checks that
    /// every tile stays in exactly one place. Not a parity check.
    fn random_game(mode: Mode, seed: u64) -> State {
        let mut rng = Rng::new(seed);
        let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
        let mut moves = MoveList::new();
        loop {
            match state.phase {
                Phase::Finished => return state,
                Phase::RoundOver => state = state.next_round(&Deal::shuffled(&mut rng)),
                Phase::Playing => {
                    state.legal_moves(&mut moves);
                    let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                    state.play(mv);
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
            }
        }
    }

    #[test]
    fn random_games_finish_and_conserve_tiles() {
        for seed in 0..200 {
            for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
                let state = random_game(mode, seed);
                assert!(state.scores.iter().any(|&s| s >= 100));
            }
        }
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
}
