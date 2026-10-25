//! Game records: each round's deal in Capi's layout and its moves in order
//! (docs/FORMATS.md gives the text form from M2-02). A record holds every
//! hidden tile; only the referee and record tools may read it directly.

use crate::rules::{Deal, Mode, Move, Phase, State};
use crate::tile::Tile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundRecord {
    pub deal: Deal,
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    pub mode: Mode,
    pub target: u16,
    pub rounds: Vec<RoundRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The record has no such round or ply.
    NoSuchPosition,
    /// A move the referee rejects.
    IllegalMove { round: usize, ply: usize },
    /// A round's moves end before the round does, or run past its end.
    RoundNotOver { round: usize },
}

impl GameRecord {
    /// The state at the start of `round`, after replaying every earlier
    /// round in full, with round 1's automatic tile.
    pub fn round_start(&self, round: usize) -> Result<(State, Option<Tile>), RecordError> {
        let first = self.rounds.first().ok_or(RecordError::NoSuchPosition)?;
        let (mut state, tile) = State::first_round(self.mode, self.target, &first.deal);
        if round == 0 {
            return Ok((state, Some(tile)));
        }
        for (r, rec) in self.rounds.iter().enumerate() {
            if r == round {
                return Ok((state, None));
            }
            for (ply, &mv) in rec.moves.iter().enumerate() {
                if !state.is_legal(mv) {
                    return Err(RecordError::IllegalMove { round: r, ply });
                }
                state.play(mv);
            }
            if state.phase != Phase::RoundOver {
                return Err(RecordError::RoundNotOver { round: r });
            }
            let next = self.rounds.get(r + 1).ok_or(RecordError::NoSuchPosition)?;
            state = state.next_round(&next.deal);
        }
        Err(RecordError::NoSuchPosition)
    }
}
