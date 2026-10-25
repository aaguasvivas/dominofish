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

    /// A referee for a round already dealt, such as `State::dealt` with
    /// scores carried in.
    pub fn from_state(state: State) -> Referee {
        Referee { state }
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

/// Plays one game of bots in process (`bots[seat]`), giving each bot only
/// the InfoSet this referee builds for its seat from the record so far.
/// Round r deals with derive(deal_seed, r); a seat's bot draws its
/// tie-breaks from derive(derive(deal_seed, r), 100 + seat), keyed by the
/// deal and the seat, so the same bot in the same seat of the same deal
/// makes the same choices.
pub fn play_bots(
    mode: crate::rules::Mode,
    target: u16,
    bots: [crate::bots::Bot; 4],
    deal_seed: u64,
) -> crate::record::GameRecord {
    use crate::infoset::InfoSet;
    use crate::record::{GameRecord, RoundRecord};
    use crate::rng::{Rng, derive};

    let deal_for = |r: u64| Deal::shuffled(&mut Rng::new(derive(deal_seed, r)));
    let rngs_for = |r: u64| {
        core::array::from_fn::<Rng, 4, _>(|s| {
            Rng::new(derive(derive(deal_seed, r), 100 + s as u64))
        })
    };
    let mut record = GameRecord {
        mode,
        target,
        rounds: vec![RoundRecord {
            deal: deal_for(0),
            moves: vec![],
        }],
    };
    let (mut state, _) = State::first_round(mode, target, &record.rounds[0].deal);
    let mut rngs = rngs_for(0);
    loop {
        match state.phase {
            Phase::Finished => return record,
            Phase::RoundOver => {
                let r = record.rounds.len();
                let deal = deal_for(r as u64);
                state = state.next_round(&deal);
                record.rounds.push(RoundRecord {
                    deal,
                    moves: vec![],
                });
                rngs = rngs_for(r as u64);
            }
            Phase::Playing => {
                let r = record.rounds.len() - 1;
                let seat = state.turn;
                let ply = record.rounds[r].moves.len();
                let info = InfoSet::new(&record, r, ply, seat).expect("the record so far is valid");
                let mv = bots[seat as usize].decide(&info, &mut rngs[seat as usize]);
                assert!(
                    state.is_legal(mv),
                    "{:?} chose an illegal move {mv:?}",
                    bots[seat as usize]
                );
                state.play(mv);
                record.rounds[r].moves.push(mv);
            }
        }
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
