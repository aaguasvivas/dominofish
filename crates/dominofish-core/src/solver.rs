//! The exact perfect-information solver (M3, docs/PLAN.md D15). The value of
//! a known deal is the rest of the round's point margin, team 0 minus team
//! 1, under perfect play by every seat: bonuses paid from here on plus the
//! round's points. Alpha-beta search with a transposition table of bound
//! entries, move ordering, and a node cap.
//!
//! The value depends on the scores only through the bonus cap (R25): a team
//! with score s can still receive (target - 1 - s) / 25 bonuses this round.
//! The table key holds that count per team instead of the scores, so it
//! covers everything the value depends on. A table serves one deal at a
//! time: in 1v1 the key holds the boneyard position but not its order, so
//! `Table::clear` starts every new deal.
//!
//! With the win objective (M6, D21) a round's end is valued instead by team
//! 0's probability to win the game, from the equity table W in millionths.
//! Within a round played from one start the scores at a node are the
//! start's plus the bonuses paid on the way, which the key's bonus counts
//! fix, so the key still covers the value; with this objective a table
//! serves one deal at one start, and one objective, at a time.
//!
//! The search allocates nothing: moves live in fixed-size lists and the
//! table is allocated once.

use crate::equity::Equity;
use crate::rules::{BONUS, Mode, Move, MoveList, Phase, State, team};

/// The node count of a search with no cap.
pub const NO_CAP: u64 = u64::MAX;

/// Above any value: any margin a round can reach (every pip, a capicua, and
/// every bonus stay far below 10,000) and any win value (at most
/// 1,000,000).
const INF: i32 = 2_000_000;

/// A win value of 1: the game is won.
pub const WIN: i32 = 1_000_000;

/// What a solve maximizes for team 0 (D21).
#[derive(Clone, Copy, Debug)]
pub enum Objective<'a> {
    /// The rest of the round's margin (D15).
    Points,
    /// The probability in millionths to win the game, by W at the round's
    /// end (the table's target must be the state's).
    Win(&'a Equity),
}

/// Team 0's probability in millionths to win the game at the end of a round
/// (phase RoundOver or Finished): 1 or 0 when the round's winner reached the
/// target, else W with the round's winner leading the next round (R6).
pub fn win_value(w: &Equity, end: &State) -> i32 {
    debug_assert_eq!(w.target, end.target);
    let ours = team(end.round_winner) == 0;
    match end.phase {
        Phase::Finished => {
            if ours {
                WIN
            } else {
                0
            }
        }
        _ => (w.win(end.scores[0], end.scores[1], ours) * f64::from(WIN)).round() as i32,
    }
}

const EXACT: u8 = 1;
const LOWER: u8 = 2;
const UPPER: u8 = 3;

/// Keys for the bonuses each team can still receive: at most
/// (MAX_TARGET - 1) / 25 = 11.
const BONUS_KEYS: [[u64; 12]; 2] = {
    let mut keys = [[0u64; 12]; 2];
    let mut s = 0x5017_E500_0000_0003u64;
    let mut t = 0;
    while t < 2 {
        let mut n = 0;
        while n < 12 {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            keys[t][n] = z ^ (z >> 31);
            n += 1;
        }
        t += 1;
    }
    keys
};

/// How many 25-point bonuses a team can still receive this round (R25).
fn bonuses_left(state: &State, t: usize) -> usize {
    let s = state.scores[t];
    if state.mode == Mode::OneVsOne || s >= state.target {
        0
    } else {
        ((state.target - 1 - s) / BONUS) as usize
    }
}

/// The table key of a state: its position hash and the bonuses each team
/// can still receive.
pub fn key(state: &State) -> u64 {
    state.position_hash()
        ^ BONUS_KEYS[0][bonuses_left(state, 0)]
        ^ BONUS_KEYS[1][bonuses_left(state, 1)]
}

fn move_code(mv: Move) -> u8 {
    match mv {
        Move::Play { tile, end } => 1 + 2 * tile.index() as u8 + end as u8,
        Move::Draw => 57,
        Move::Pass => 58,
    }
}

#[derive(Clone, Copy, Default)]
struct Entry {
    key: u64,
    value: i32,
    bound: u8,
    /// The best move's code (`move_code`), 0 for none.
    mv: u8,
    generation: u8,
}

/// The transposition table: 2^bits entries of 16 bytes, always replaced.
pub struct Table {
    entries: Vec<Entry>,
    generation: u8,
}

impl Table {
    pub fn new(bits: u32) -> Table {
        Table {
            entries: vec![Entry::default(); 1 << bits],
            generation: 1,
        }
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * core::mem::size_of::<Entry>()
    }

    /// Starts a new deal: entries stored before no longer match.
    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.entries.fill(Entry::default());
            self.generation = 1;
        }
    }

    fn probe(&self, key: u64) -> Option<Entry> {
        let e = self.entries[key as usize & (self.entries.len() - 1)];
        (e.generation == self.generation && e.key == key).then_some(e)
    }

    fn store(&mut self, key: u64, value: i32, bound: u8, mv: Move) {
        let i = key as usize & (self.entries.len() - 1);
        self.entries[i] = Entry {
            key,
            value,
            bound,
            mv: move_code(mv),
            generation: self.generation,
        };
    }
}

/// The result of a solve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Solution {
    /// The rest of the round's margin, team 0 minus team 1, or with the win
    /// objective team 0's probability in millionths to win the game. Exact
    /// unless `capped`.
    pub value: i32,
    /// The best move of the seat on turn.
    pub best: Move,
    /// Search nodes visited.
    pub nodes: u64,
    /// The search hit its node cap; `value` and `best` come from the
    /// fallback (`greedy`).
    pub capped: bool,
}

/// The ordering score of a move: the table's move first, then plays of
/// heavy tiles and doubles (docs/PLAN.md D15).
fn order_score(mv: Move, table_move: u8) -> i32 {
    if move_code(mv) == table_move {
        return 1000;
    }
    match mv {
        Move::Play { tile, .. } => 2 * tile.pips() as i32 + if tile.is_double() { 3 } else { 0 },
        Move::Draw | Move::Pass => 0,
    }
}

/// The legal moves of `state`, best ordering score first; ties keep the
/// generator's order.
fn ordered(state: &State, table_move: u8, out: &mut MoveList) {
    state.legal_moves(out);
    let mut scored = [(0i32, Move::Pass); 16];
    let n = out.len();
    for (slot, &mv) in scored.iter_mut().zip(out.as_slice()) {
        *slot = (order_score(mv, table_move), mv);
    }
    for i in 1..n {
        let mut j = i;
        while j > 0 && scored[j - 1].0 < scored[j].0 {
            scored.swap(j - 1, j);
            j -= 1;
        }
    }
    out.clear();
    for &(_, mv) in &scored[..n] {
        out.push(mv);
    }
}

/// The points a move scored: team 0's gain minus team 1's.
fn gain(before: [u16; 2], after: [u16; 2]) -> i32 {
    (i32::from(after[0]) - i32::from(before[0])) - (i32::from(after[1]) - i32::from(before[1]))
}

struct Search<'a> {
    table: Option<&'a mut Table>,
    objective: Objective<'a>,
    cap: u64,
    nodes: u64,
    aborted: bool,
}

impl Search<'_> {
    /// Fail-soft alpha-beta in team 0's terms: a seat of team 0 maximizes,
    /// a seat of team 1 minimizes. Table bounds only cut off; they never
    /// narrow the window, so each stored bound is one the searched window
    /// proves.
    /// The root never returns from the table: its caller needs the move.
    fn search(
        &mut self,
        state: &mut State,
        mut alpha: i32,
        mut beta: i32,
        root: bool,
    ) -> (i32, Move) {
        if self.nodes >= self.cap {
            self.aborted = true;
            return (0, Move::Pass);
        }
        self.nodes += 1;
        let key = key(state);
        let mut table_move = 0;
        if let Some(e) = self.table.as_deref().and_then(|t| t.probe(key)) {
            let v = e.value;
            let cut = match e.bound {
                EXACT => true,
                LOWER => v >= beta,
                _ => v <= alpha,
            };
            if cut && !root {
                return (v, Move::Pass);
            }
            table_move = e.mv;
        }
        let (alpha0, beta0) = (alpha, beta);
        let maximize = team(state.turn) == 0;
        let mut moves = MoveList::new();
        ordered(state, table_move, &mut moves);
        let mut best = if maximize { i32::MIN } else { i32::MAX };
        let mut best_move = moves.as_slice()[0];
        for &mv in moves.as_slice() {
            let before = state.scores;
            let played = state.play(mv);
            let v = match self.objective {
                Objective::Points => {
                    let g = gain(before, state.scores);
                    if state.phase == Phase::Playing {
                        g + self.search(state, alpha - g, beta - g, false).0
                    } else {
                        g
                    }
                }
                Objective::Win(w) => {
                    if state.phase == Phase::Playing {
                        self.search(state, alpha, beta, false).0
                    } else {
                        win_value(w, state)
                    }
                }
            };
            state.undo(mv, played.undo);
            if self.aborted {
                return (0, Move::Pass);
            }
            if maximize {
                if v > best {
                    (best, best_move) = (v, mv);
                }
                alpha = alpha.max(v);
            } else {
                if v < best {
                    (best, best_move) = (v, mv);
                }
                beta = beta.min(v);
            }
            if alpha >= beta {
                break;
            }
        }
        if let Some(t) = self.table.as_deref_mut() {
            let bound = if best <= alpha0 {
                UPPER
            } else if best >= beta0 {
                LOWER
            } else {
                EXACT
            };
            t.store(key, best, bound, best_move);
        }
        (best, best_move)
    }
}

/// Solves `state` (phase Playing) for the round margin with at most `cap`
/// nodes, using `table` if given (cleared by the caller for each new deal).
/// `state` is unchanged on return. Past the cap, the value and move come
/// from `greedy`.
pub fn solve(state: &mut State, table: Option<&mut Table>, cap: u64) -> Solution {
    solve_with(state, table, cap, Objective::Points)
}

/// `solve` for an objective (D21).
pub fn solve_with(
    state: &mut State,
    table: Option<&mut Table>,
    cap: u64,
    objective: Objective,
) -> Solution {
    assert_eq!(state.phase, Phase::Playing);
    let mut s = Search {
        table,
        objective,
        cap,
        nodes: 0,
        aborted: false,
    };
    let (value, best) = s.search(state, -INF, INF, true);
    if !s.aborted {
        return Solution {
            value,
            best,
            nodes: s.nodes,
            capped: false,
        };
    }
    let maximize = team(state.turn) == 0;
    let mut moves = MoveList::new();
    ordered(state, 0, &mut moves);
    let mut pick = (
        if maximize { i32::MIN } else { i32::MAX },
        moves.as_slice()[0],
    );
    for &mv in moves.as_slice() {
        let before = state.scores;
        let played = state.play(mv);
        let v = match objective {
            Objective::Points => gain(before, state.scores) + greedy(state),
            Objective::Win(w) => greedy_win(w, state),
        };
        state.undo(mv, played.undo);
        if (maximize && v > pick.0) || (!maximize && v < pick.0) {
            pick = (v, mv);
        }
    }
    Solution {
        value: pick.0,
        best: pick.1,
        nodes: s.nodes,
        capped: true,
    }
}

/// The fallback past the node cap: the margin when every seat plays the
/// first move of the search's ordering (`order_score`: the heaviest tile,
/// with a double counted 3 pips heavier; ties to the generator's order) to
/// the end of the round.
pub fn greedy(state: &mut State) -> i32 {
    let start = state.scores;
    playout(state, |end| gain(start, end.scores))
}

/// `greedy` with the win objective: the win value at the end of the same
/// play.
pub fn greedy_win(w: &Equity, state: &mut State) -> i32 {
    playout(state, |end| win_value(w, end))
}

/// `value` of the round's end when every seat plays the first move of the
/// search's ordering; `state` is unchanged on return.
fn playout(state: &mut State, value: impl Fn(&State) -> i32) -> i32 {
    let mut moves = MoveList::new();
    let mut trail = [(Move::Pass, None); 128];
    let mut n = 0;
    while state.phase == Phase::Playing {
        ordered(state, 0, &mut moves);
        let mv = moves.as_slice()[0];
        trail[n] = (mv, Some(state.play(mv).undo));
        n += 1;
    }
    let v = value(state);
    while n > 0 {
        n -= 1;
        if let (mv, Some(undo)) = trail[n] {
            state.undo(mv, undo);
        }
    }
    v
}

/// Plain minimax, with no pruning and no table: the reference the solver
/// must equal (M3). None past `cap` nodes.
pub fn minimax(state: &mut State, cap: u64, nodes: &mut u64) -> Option<i32> {
    minimax_with(state, cap, nodes, Objective::Points)
}

/// `minimax` for an objective (D21).
pub fn minimax_with(
    state: &mut State,
    cap: u64,
    nodes: &mut u64,
    objective: Objective,
) -> Option<i32> {
    if *nodes >= cap {
        return None;
    }
    *nodes += 1;
    let maximize = team(state.turn) == 0;
    let mut moves = MoveList::new();
    state.legal_moves(&mut moves);
    let mut best = if maximize { i32::MIN } else { i32::MAX };
    for &mv in moves.as_slice() {
        let before = state.scores;
        let played = state.play(mv);
        let v = match objective {
            Objective::Points => {
                let g = gain(before, state.scores);
                if state.phase == Phase::Playing {
                    minimax_with(state, cap, nodes, objective).map(|c| g + c)
                } else {
                    Some(g)
                }
            }
            Objective::Win(w) => {
                if state.phase == Phase::Playing {
                    minimax_with(state, cap, nodes, objective)
                } else {
                    Some(win_value(w, state))
                }
            }
        };
        state.undo(mv, played.undo);
        let v = v?;
        best = if maximize { best.max(v) } else { best.min(v) };
    }
    Some(best)
}

/// Tiles not on the board (in hands or the boneyard): the depth measure of
/// the solver's checks and statistics.
pub fn tiles_left(state: &State) -> u32 {
    28 - state.board.count_ones()
}
