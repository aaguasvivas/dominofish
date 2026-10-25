//! The referee: holds a game's full state, takes Capi-ordered deals
//! (docs/RULES.md R3), and accepts only the moves Capi accepts.

use crate::rules::{Deal, Event, Mode, Move, Phase, State};
use crate::tile::Tile;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefereeError {
    /// Capi would reject this move from the seat on turn.
    IllegalMove(Move),
    /// A new round was dealt while the round was not over.
    RoundNotOver,
}

#[derive(Clone, Debug)]
pub struct Referee {
    state: State,
}

impl Referee {
    /// A game at round 1, with its automatic opening tile.
    pub fn new(mode: Mode, target: u16, deal: &Deal) -> (Referee, Tile) {
        let (state, tile) = State::first_round(mode, target, deal);
        (Referee { state }, tile)
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn play(&mut self, mv: Move) -> Result<Option<Event>, RefereeError> {
        if !self.state.is_legal(mv) {
            return Err(RefereeError::IllegalMove(mv));
        }
        Ok(self.state.play(mv).event)
    }

    pub fn next_round(&mut self, deal: &Deal) -> Result<(), RefereeError> {
        if self.state.phase != Phase::RoundOver {
            return Err(RefereeError::RoundNotOver);
        }
        self.state = self.state.next_round(deal);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::End;

    fn deal_in_order() -> Deal {
        Deal::from_tiles(core::array::from_fn(|i| Tile::from_index(i as u8))).unwrap()
    }

    #[test]
    fn rejects_what_capi_rejects() {
        // Tiles in index order: n holds 0-0..0-6, e 1-1..1-6 and 2-2, s
        // 2-3..2-6 and 3-3..3-5, w 3-6 and 4-4..6-6, so w opens with 6-6.
        let (mut referee, opening) = Referee::new(Mode::TwoVsTwo, 100, &deal_in_order());
        assert_eq!(opening, Tile::new(6, 6));
        assert_eq!(referee.state().turn, 0);
        let play = |a, b| Move::Play {
            tile: Tile::new(a, b),
            end: End::Left,
        };
        assert_eq!(
            referee.play(Move::Draw),
            Err(RefereeError::IllegalMove(Move::Draw))
        );
        assert_eq!(
            referee.play(play(1, 6)),
            Err(RefereeError::IllegalMove(play(1, 6)))
        );
        assert_eq!(
            referee.play(Move::Pass),
            Err(RefereeError::IllegalMove(Move::Pass))
        );
        assert_eq!(referee.play(play(0, 6)), Ok(None));
        assert_eq!(
            referee.next_round(&deal_in_order()),
            Err(RefereeError::RoundNotOver)
        );
        assert_eq!(referee.state().board.count_ones(), 2);
    }
}
